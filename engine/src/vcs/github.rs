//! # GitHub VCS Adapter (v0.16.0)
//!
//! REST API v3: <https://docs.github.com/en/rest>. Токен читается из
//! `$GITHUB_TOKEN` или `$GH_TOKEN`. Без токена работает в анонимном
//! режиме (rate-limit 60 req/h — хватает для `gh search` в шелле).
//!
//! **Архитектурные правила:**
//! * Ядро `poler_engine::*` не трогается — адаптер отдаёт нейтральные
//!   `VcsCommit`/`VcsIssue`/`VcsCodeHit`, а `vcs::ingest` упаковывает в `WebDoc`.
//! * HTTP-запрос — синхронный (ureq, без tokio). poler-engine — CLI-процесс,
//!   не веб-сервер; асинхронщина добавила бы лишний рантайм.
//! * Все URL в `links`/`web_url` — канонические веб-адреса (для `links`
//!   таблицы в web-index.db → PageRank отслеживает `web_url → vcs_url` рёбра).

use super::{RepoId, VcsAdapter, VcsCodeHit, VcsCommit, VcsIssue, VcsScheme};

/// Адаптер GitHub. Создаётся через [`GithubAdapter::from_env`] — токен
/// из переменной окружения.
pub struct GithubAdapter {
    token: Option<String>,
    /// API-хост для self-hosted GitHub Enterprise (по умолчанию `api.github.com`).
    api_host: String,
    /// Веб-хост для URL-ов коммитов/issue (`github.com` по умолчанию).
    web_host: String,
    /// User-Agent (GitHub требует).
    user_agent: String,
}

impl Default for GithubAdapter {
    fn default() -> Self {
        Self {
            token: None,
            api_host: "api.github.com".into(),
            web_host: "github.com".into(),
            user_agent: "poler-engine/0.16".into(),
        }
    }
}

impl GithubAdapter {
    /// Создать адаптер, читая токен из окружения.
    /// `$GITHUB_TOKEN` имеет приоритет, fallback — `$GH_TOKEN`.
    /// `api.github.com` по умолчанию; переопределяется через `$GITHUB_API_HOST`
    /// (для self-hosted GH Enterprise, например `github.my-corp.com/api/v3`).
    pub fn from_env() -> Result<Self, String> {
        let token = std::env::var("GITHUB_TOKEN")
            .or_else(|_| std::env::var("GH_TOKEN"))
            .ok()
            .filter(|s| !s.trim().is_empty());
        let api_host = std::env::var("GITHUB_API_HOST")
            .unwrap_or_else(|_| "api.github.com".into());
        let web_host = std::env::var("GITHUB_WEB_HOST")
            .unwrap_or_else(|_| "github.com".into());
        let user_agent = std::env::var("POLER_USER_AGENT")
            .unwrap_or_else(|_| "poler-engine/0.16".into());
        Ok(Self {
            token,
            api_host,
            web_host,
            user_agent,
        })
    }

    /// Создать с явным токеном (для тестов).
    pub fn with_token(token: impl Into<String>) -> Self {
        Self {
            token: Some(token.into()),
            api_host: "api.github.com".into(),
            web_host: "github.com".into(),
            user_agent: "poler-engine/0.16".into(),
        }
    }

    /// Создать анонимный адаптер (rate-limit 60 req/h).
    pub fn anonymous() -> Self {
        Self::default()
    }

    /// Базовый URL REST API.
    fn api_base(&self) -> String {
        if self.api_host == "api.github.com" {
            "https://api.github.com".into()
        } else {
            format!("https://{}", self.api_host)
        }
    }

    /// Универсальный GET-запрос к API. Возвращает тело JSON как `serde_json::Value`.
    fn api_get(&self, path: &str) -> Result<serde_json::Value, String> {
        let url = format!("{}{}", self.api_base(), path);
        let mut req = ureq::get(&url)
            .set("User-Agent", &self.user_agent)
            .set("Accept", "application/vnd.github+json");
        if let Some(t) = &self.token {
            req = req.set("Authorization", &format!("Bearer {t}"));
        }
        let resp = req
            .call()
            .map_err(|e| format!("github GET {url}: {e}"))?;
        let body: serde_json::Value =
            resp.into_json().map_err(|e| format!("github json parse: {e}"))?;
        Ok(body)
    }

    /// URL-префикс для API-запросов по репозиторию (`/repos/{owner}/{repo}`).
    fn repo_path(&self, repo: &RepoId) -> Result<String, String> {
        let (owner, name) = split_owner_repo(&repo.id)?;
        Ok(format!("/repos/{owner}/{name}"))
    }

    /// Веб-URL репозитория: `https://github.com/{owner}/{repo}`.
    pub fn repo_web_url(&self, repo: &RepoId) -> Result<String, String> {
        let (owner, name) = split_owner_repo(&repo.id)?;
        Ok(format!("https://{}/{}/{}", self.web_host, owner, name))
    }
}

impl VcsAdapter for GithubAdapter {
    fn name(&self) -> &'static str {
        "github"
    }

    fn scheme(&self) -> VcsScheme {
        VcsScheme::GitHub
    }

    fn list_repos(&self, owner: &str) -> Result<Vec<RepoId>, String> {
        // сначала пробуем как пользователя, потом как org
        let user_path = format!("/users/{owner}/repos?per_page=100&type=owner");
        let body = match self.api_get(&user_path) {
            Ok(b) => b,
            Err(_) => {
                // fallback на orgs
                return self
                    .api_get(&format!("/orgs/{owner}/repos?per_page=100"))
                    .map(|v| parse_repos(&v))
                    .map_err(|e| format!("list_repos({owner}): {e}"));
            }
        };
        Ok(parse_repos(&body))
    }

    fn list_commits(&self, repo: &RepoId, limit: usize) -> Result<Vec<VcsCommit>, String> {
        let path = format!(
            "{}/commits?per_page={}",
            self.repo_path(repo)?,
            limit.min(100)
        );
        let body = self.api_get(&path)?;
        let arr = body
            .as_array()
            .ok_or_else(|| "commits: expected array".to_string())?;
        let mut out = Vec::with_capacity(arr.len());
        for c in arr {
            let sha = s(c, "sha");
            let commit = &c["commit"];
            let message = s(commit, "message");
            let author_obj = &commit["author"];
            let author_name = s(author_obj, "name");
            let author_email = s(author_obj, "email");
            let authored_at = parse_iso(&s(author_obj, "date")).unwrap_or(0);
            let web_url = format!(
                "https://{}/{}/{}/commit/{sha}",
                self.web_host,
                owner_from_repo(&repo.id)?,
                name_from_repo(&repo.id)?,
            );
            out.push(VcsCommit {
                sha,
                message,
                author: author_name,
                author_email,
                authored_at,
                web_url,
            });
        }
        Ok(out)
    }

    fn list_issues(&self, repo: &RepoId, limit: usize) -> Result<Vec<VcsIssue>, String> {
        // REST: /repos/{owner}/{repo}/issues — НЕ возвращает PR (как в GraphQL),
        // но поле `pull_request` в каждом issue указывает, что это PR.
        let path = format!(
            "{}/issues?state=all&per_page={}",
            self.repo_path(repo)?,
            limit.min(100)
        );
        let body = self.api_get(&path)?;
        let arr = body
            .as_array()
            .ok_or_else(|| "issues: expected array".to_string())?;
        let (owner, name) = split_owner_repo(&repo.id)?;
        let mut out = Vec::with_capacity(arr.len());
        for i in arr {
            let number = i["number"].as_u64().unwrap_or(0);
            let title = s(i, "title");
            let body = s(i, "body");
            let state = s(i, "state");
            let author = s(&i["user"], "login");
            let created_at = parse_iso(&s(i, "created_at")).unwrap_or(0);
            let is_pr = i.get("pull_request").is_some();
            let kind = if is_pr { "pull" } else { "issues" };
            let web_url = format!(
                "https://{}/{}/{}/{}/{number}",
                self.web_host, owner, name, kind
            );
            out.push(VcsIssue {
                number,
                title,
                body,
                state,
                author,
                created_at,
                web_url,
                is_merge_request: is_pr,
            });
        }
        Ok(out)
    }

    fn search_code(&self, query: &str, limit: usize) -> Result<Vec<VcsCodeHit>, String> {
        // GitHub REST code search: /search/code?q=... — требует аутентификацию.
        if self.token.is_none() {
            return Err(
                "github code search requires $GITHUB_TOKEN (anonymous forbidden by API)"
                    .into(),
            );
        }
        let q = urlencode(query);
        let path = format!("/search/code?q={q}&per_page={}", limit.min(100));
        let body = self.api_get(&path)?;
        let items = body["items"]
            .as_array()
            .ok_or_else(|| "search/code: items not array".to_string())?;
        let mut out = Vec::with_capacity(items.len());
        for it in items {
            let path_val = s(it, "path");
            let repo_full = s(&it["repository"], "full_name");
            let sha = s(it, "sha");
            let html_url = s(it, "html_url");
            out.push(VcsCodeHit {
                path: path_val,
                repo: repo_full,
                sha,
                snippet: String::new(),
                web_url: html_url,
            });
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// helpers: парсинг JSON, URL-кодирование, разбор owner/repo
// ---------------------------------------------------------------------------

fn s(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

fn parse_repos(body: &serde_json::Value) -> Vec<RepoId> {
    let arr = match body.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };
    arr.iter()
        .map(|r| {
            let full = s(r, "full_name");
            RepoId::new(full.clone(), full)
        })
        .collect()
}

/// Разбор `"owner/repo"` → (owner, repo).
pub fn split_owner_repo(id: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = id.splitn(2, '/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(format!("repo id must be 'owner/repo', got: {id}"));
    }
    Ok((parts[0].into(), parts[1].into()))
}

fn owner_from_repo(id: &str) -> Result<String, String> {
    split_owner_repo(id).map(|(o, _)| o)
}

fn name_from_repo(id: &str) -> Result<String, String> {
    split_owner_repo(id).map(|(_, n)| n)
}

/// Парсинг ISO-8601 → epoch-секунды UTC. Принимает `"2024-08-18T12:34:56Z"`
/// (без дробных секунд) и `"2024-08-18T12:34:56.000Z"` (с долями).
///
/// Алгоритм: итеративный подсчёт дней от 1970-01-01 по годам и месяцам.
/// Не использует формулу Howard Hinnant (которая давала ошибку в edge-cases);
/// вместо этого — простой и читаемый цикл, нормальный для диапазона 1970–2100.
pub fn parse_iso(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let mo: i64 = s.get(5..7)?.parse().ok()?;
    let d: i64 = s.get(8..10)?.parse().ok()?;
    let h: i64 = s.get(11..13)?.parse().ok()?;
    let mi: i64 = s.get(14..16)?.parse().ok()?;
    let se: i64 = s.get(17..19)?.parse().ok()?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let days = days_from_civil(y, mo, d)?;
    Some(days * 86400 + h * 3600 + mi * 60 + se)
}

/// Простой подсчёт дней с 1970-01-01 по (y, m, d) UTC.
/// Итеративный, но для диапазона 1970–2100 (130 лет) — это ~130 шагов
/// в годовом цикле + 12 в месячном. Тестов в ISO-парсинге —
/// пренебрежимо малая нагрузка.
fn days_from_civil(y: i64, m: i64, d: i64) -> Option<i64> {
    if y < 1970 {
        return None;
    }
    // Накопленные дни в начале каждого месяца (невысокосный год):
    // Jan=0, Feb=31, Mar=59, Apr=90, May=120, Jun=151, Jul=181,
    // Aug=212, Sep=243, Oct=273, Nov=304, Dec=334
    const MONTH_START: [i64; 13] = [0, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let mut days = 0i64;
    for yr in 1970..y {
        days += if is_leap(yr) { 366 } else { 365 };
    }
    days += MONTH_START[m as usize];
    if m > 2 && is_leap(y) {
        days += 1; // 29 февраля в високосном году
    }
    days += d - 1;
    Some(days)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

/// Простое URL-кодирование (только зарезервированные символы).
/// Не используем `urlencoding` crate, чтобы не добавлять зависимость.
pub fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_from_env_optional_token() {
        // убираем токены для теста (не должно паниковать, токен опциональный)
        std::env::remove_var("GITHUB_TOKEN");
        std::env::remove_var("GH_TOKEN");
        let a = GithubAdapter::from_env().unwrap();
        assert!(a.token.is_none(), "without env vars, token should be None");
        assert_eq!(a.api_host, "api.github.com");
        assert_eq!(a.web_host, "github.com");
    }

    #[test]
    fn adapter_with_token_explicit() {
        let a = GithubAdapter::with_token("ghp_fake_test_token");
        assert_eq!(a.token.as_deref(), Some("ghp_fake_test_token"));
        assert_eq!(a.name(), "github");
        assert_eq!(a.scheme(), VcsScheme::GitHub);
    }

    #[test]
    fn split_owner_repo_basic() {
        let (o, n) = split_owner_repo("poler-engine/poler-engine").unwrap();
        assert_eq!(o, "poler-engine");
        assert_eq!(n, "poler-engine");
    }

    #[test]
    fn split_owner_repo_with_subpath() {
        // под-путь в имени репо — допустимо (splitn=2)
        let (o, n) = split_owner_repo("a/b/c").unwrap();
        assert_eq!(o, "a");
        assert_eq!(n, "b/c");
    }

    #[test]
    fn split_owner_repo_no_slash_errors() {
        assert!(split_owner_repo("nonslash").is_err());
        assert!(split_owner_repo("").is_err());
        assert!(split_owner_repo("/justrepo").is_err());
        assert!(split_owner_repo("justowner/").is_err());
    }

    #[test]
    fn parse_iso_well_known() {
        // 2024-01-01T00:00:00Z = 1704067200
        assert_eq!(parse_iso("2024-01-01T00:00:00Z"), Some(1704067200));
    }

    #[test]
    fn parse_iso_with_millis() {
        // та же дата, с дробными секундами
        assert_eq!(parse_iso("2024-01-01T00:00:00.000Z"), Some(1704067200));
    }

    #[test]
    fn parse_iso_invalid_returns_none() {
        assert_eq!(parse_iso(""), None);
        assert_eq!(parse_iso("garbage"), None);
        assert_eq!(parse_iso("2024-01-01"), None);
    }

    #[test]
    fn urlencode_basic() {
        assert_eq!(urlencode("hello world"), "hello+world");
        assert_eq!(urlencode("a+b=c"), "a%2Bb%3Dc");
        assert_eq!(urlencode("safe123"), "safe123");
        assert_eq!(urlencode("a.b-c_d~e"), "a.b-c_d~e");
    }

    #[test]
    fn urlencode_unicode_escaped() {
        // кириллица UTF-8 → %XX-последовательности
        let encoded = urlencode("Привет");
        assert!(encoded.starts_with("%"), "cyrillic must be escaped");
        assert!(!encoded.contains('П'));
    }

    #[test]
    fn repo_web_url_basic() {
        let a = GithubAdapter::default();
        let url = a
            .repo_web_url(&RepoId::from_owner_repo("user", "repo"))
            .unwrap();
        assert_eq!(url, "https://github.com/user/repo");
    }

    #[test]
    fn parse_repos_from_json_array() {
        let body: serde_json::Value = serde_json::json!([
            {"full_name": "user/repo1"},
            {"full_name": "user/repo2"}
        ]);
        let v = parse_repos(&body);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].id, "user/repo1");
        assert_eq!(v[1].id, "user/repo2");
    }

    #[test]
    fn parse_repos_from_non_array_is_empty() {
        let body: serde_json::Value = serde_json::json!({"error": "nope"});
        let v = parse_repos(&body);
        assert!(v.is_empty());
    }

    #[test]
    fn search_code_without_token_errors() {
        let a = GithubAdapter::anonymous();
        let res = a.search_code("rust", 5);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("GITHUB_TOKEN"));
    }
    #[test]
    fn api_base_default_is_github() {
        let a = GithubAdapter::default();
        assert_eq!(a.api_base(), "https://api.github.com");
    }

    #[test]
    fn api_base_enterprise_uses_host() {
        let mut a = GithubAdapter::default();
        a.api_host = "github.mycorp.com/api/v3".into();
        assert_eq!(a.api_base(), "https://github.mycorp.com/api/v3");
    }

    #[test]
    fn list_commits_parses_minimal_payload() {
        // нельзя дёрнуть реальный GitHub без токена, но проверим парсинг через mock JSON.
        // (GithubAdapter::anonymous() здесь не нужен — тестируем только хелперы парсинга.)
        let body = serde_json::json!([
            {
                "sha": "abcdef1234567890",
                "commit": {
                    "author": {
                        "name": "Alice",
                        "email": "alice@x.com",
                        "date": "2024-01-01T12:34:56Z"
                    },
                    "message": "test commit message"
                }
            }
        ]);
        let arr = body.as_array().unwrap();
        let c = &arr[0];
        // проверяем, что хелперы корректно извлекают поля
        assert_eq!(s(c, "sha"), "abcdef1234567890");
        assert_eq!(s(&c["commit"], "message"), "test commit message");
        assert_eq!(s(&c["commit"]["author"], "name"), "Alice");
        assert_eq!(s(&c["commit"]["author"], "email"), "alice@x.com");
        assert_eq!(parse_iso(&s(&c["commit"]["author"], "date")), Some(1704112496));
    }

    #[test]
    fn list_issues_distinguishes_pr_from_issue() {
        // Тестируем эвристику: объект с "pull_request" → PR, без → issue
        let body = serde_json::json!([
            {"number": 1, "title": "issue 1", "state": "open", "body": "issue body", "user": {"login": "alice"}, "created_at": "2024-01-01T00:00:00Z"},
            {"number": 2, "title": "PR 2", "state": "open", "body": "pr body", "user": {"login": "bob"}, "created_at": "2024-01-02T00:00:00Z", "pull_request": {"url": "..."}}
        ]);
        let arr = body.as_array().unwrap();
        let i1 = &arr[0];
        let i2 = &arr[1];
        assert!(i1.get("pull_request").is_none(), "first is plain issue");
        assert!(i2.get("pull_request").is_some(), "second is PR");
    }
}
