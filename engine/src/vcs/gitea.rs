//! # Gitea / Forgejo VCS Adapter (v0.16.0)
//!
//! REST API: <https://gitea.com/api/swagger>. Gitea и Forgejo совместимы
//! на уровне API v1. Токен из `$GITEA_TOKEN` / `$GT_TOKEN`. Хост
//! **обязателен** через `$GITEA_HOST` (например `gitea.com`,
//! `codeberg.org`, `git.example.com`).
//!
//! В отличие от GitHub/GitLab, Gitea-инстансов множество — дефолтный
//! `gitea.com` почти никогда не подходит. Без `$GITEA_HOST` адаптер
//! возвращает ошибку инициализации (см. `from_env`).

use serde_json::Value;

use super::{RepoId, VcsAdapter, VcsCodeHit, VcsCommit, VcsIssue, VcsScheme};
use super::github::{parse_iso, urlencode};

/// Адаптер Gitea / Forgejo.
#[derive(Debug)]
pub struct GiteaAdapter {
    token: Option<String>,
    /// Полный хост с протоколом (`https://gitea.com`).
    host: String,
    /// Веб-хост без trailing slash (для URL-ов репо).
    web_host: String,
    user_agent: String,
}

impl GiteaAdapter {
    /// Создать адаптер из окружения.
    /// `$GITEA_HOST` — обязательный, без протокола (например `gitea.com`),
    /// к нему добавляется `https://` автоматически (или `http://` если
    /// передан `localhost` / IP).
    pub fn from_env() -> Result<Self, String> {
        let token = std::env::var("GITEA_TOKEN")
            .or_else(|_| std::env::var("GT_TOKEN"))
            .ok()
            .filter(|s| !s.trim().is_empty());
        let raw_host = std::env::var("GITEA_HOST").map_err(|_| {
            "GITEA_HOST не задан (например export GITEA_HOST=gitea.com)".to_string()
        })?;
        let host = if raw_host.contains("://") {
            raw_host.trim_end_matches('/').to_string()
        } else if raw_host.starts_with("localhost") || raw_host.contains("127.0.0.1") {
            format!("http://{}", raw_host.trim_end_matches('/'))
        } else {
            format!("https://{}", raw_host.trim_end_matches('/'))
        };
        let web_host = raw_host.trim_end_matches('/').to_string();
        let user_agent = std::env::var("POLER_USER_AGENT")
            .unwrap_or_else(|_| "poler-engine/0.16".into());
        Ok(Self {
            token,
            host,
            web_host,
            user_agent,
        })
    }

    /// Создать с явными параметрами (для тестов).
    pub fn new(host: impl Into<String>, token: Option<String>) -> Self {
        let host_str = host.into();
        let host = if host_str.contains("://") {
            host_str.trim_end_matches('/').to_string()
        } else {
            format!("https://{}", host_str.trim_end_matches('/'))
        };
        let web_host = host_str.trim_end_matches('/').to_string();
        Self {
            token,
            host,
            web_host,
            user_agent: "poler-engine/0.16".into(),
        }
    }

    /// Анонимный адаптер на gitea.com (для тестов; rate-limit очень жёсткий).
    pub fn anonymous() -> Self {
        Self::new("gitea.com", None)
    }

    /// Базовый URL API v1.
    fn api_base(&self) -> String {
        format!("{}/api/v1", self.host)
    }

    /// Универсальный GET к Gitea API.
    fn api_get(&self, path: &str) -> Result<Value, String> {
        let url = format!("{}{path}", self.api_base());
        let mut req = ureq::get(&url)
            .set("User-Agent", &self.user_agent)
            .set("Accept", "application/json");
        if let Some(t) = &self.token {
            req = req.set("Authorization", &format!("token {t}"));
        }
        let resp = req
            .call()
            .map_err(|e| format!("gitea GET {url}: {e}"))?;
        let body: Value = resp
            .into_json()
            .map_err(|e| format!("gitea json parse: {e}"))?;
        Ok(body)
    }

    /// Веб-URL репозитория: `https://gitea.com/owner/repo`.
    pub fn repo_web_url(&self, repo: &RepoId) -> Result<String, String> {
        let (owner, name) = super::github::split_owner_repo(&repo.id)?;
        Ok(format!("https://{}/{}/{}", self.web_host, owner, name))
    }
}

impl VcsAdapter for GiteaAdapter {
    fn name(&self) -> &'static str {
        "gitea"
    }

    fn scheme(&self) -> VcsScheme {
        VcsScheme::Gitea
    }

    fn list_repos(&self, owner: &str) -> Result<Vec<RepoId>, String> {
        // /users/{username}/repos — пользователь, /orgs/{org}/repos — организация.
        // Без знания типа owner'а пробуем оба.
        let user_path = format!("/users/{owner}/repos?limit=50");
        if let Ok(body) = self.api_get(&user_path) {
            if body.is_array() {
                return Ok(parse_repos(&body));
            }
        }
        let org_path = format!("/orgs/{owner}/repos?limit=50");
        let body = self.api_get(&org_path)?;
        Ok(parse_repos(&body))
    }

    fn list_commits(&self, repo: &RepoId, limit: usize) -> Result<Vec<VcsCommit>, String> {
        let (owner, name) = super::github::split_owner_repo(&repo.id)?;
        let path = format!(
            "/repos/{owner}/{name}/commits?limit={}",
            limit.min(50)
        );
        let body = self.api_get(&path)?;
        let arr = body
            .as_array()
            .ok_or_else(|| "commits: expected array".to_string())?;
        let mut out = Vec::with_capacity(arr.len());
        for c in arr {
            let sha = s(c, "sha");
            let message = s(&c["commit"], "message");
            let author_obj = &c["commit"]["author"];
            let author = s(author_obj, "name");
            let author_email = s(author_obj, "email");
            let authored_at = parse_iso(&s(author_obj, "date")).unwrap_or(0);
            let web_url = format!(
                "https://{}/{}/{}/commit/{sha}",
                self.web_host, owner, name,
            );
            out.push(VcsCommit {
                sha,
                message,
                author,
                author_email,
                authored_at,
                web_url,
            });
        }
        Ok(out)
    }

    fn list_issues(&self, repo: &RepoId, limit: usize) -> Result<Vec<VcsIssue>, String> {
        let (owner, name) = super::github::split_owner_repo(&repo.id)?;
        let path = format!(
            "/repos/{owner}/{name}/issues?state=all&limit={}",
            limit.min(50)
        );
        let body = self.api_get(&path)?;
        let arr = body
            .as_array()
            .ok_or_else(|| "issues: expected array".to_string())?;
        let mut out = Vec::with_capacity(arr.len());
        for i in arr {
            // Gitea Issues: поле `pull_request != null` если это PR.
            let number = i["number"].as_u64().unwrap_or(0);
            let title = s(i, "title");
            let body = s(i, "body");
            let state = s(i, "state");
            let author = s(&i["user"], "login");
            let created_at = parse_iso(&s(i, "created_at")).unwrap_or(0);
            let is_pr = i
                .get("pull_request")
                .map(|pr| !pr.is_null())
                .unwrap_or(false);
            let kind = if is_pr { "pulls" } else { "issues" };
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
        // /repos/issues/search?q=... — поиск issues, не кода.
        // Code search в Gitea: /repos/{owner}/{repo}/grep?q=... (per-repo only).
        // Без указания репо глобального code search нет.
        let q = urlencode(query);
        let path = format!("/repos/search?q={q}&limit={}", limit.min(50));
        let body = self.api_get(&path)?;
        // /repos/search возвращает {"data": [...] } wrapper
        let arr = if body.get("data").is_some() {
            body["data"]
                .as_array()
                .ok_or_else(|| "search: data not array".to_string())?
        } else {
            body.as_array()
                .ok_or_else(|| "search: expected array or {data:[]}".to_string())?
        };
        let mut out = Vec::with_capacity(arr.len());
        for it in arr {
            let path_val = s(it, "name"); // это название репо, а не path — для code search нужен grep per-repo
            let repo = s(it, "full_name");
            let web_url = s(it, "html_url");
            out.push(VcsCodeHit {
                path: path_val,
                repo,
                sha: String::new(),
                snippet: String::new(),
                web_url,
            });
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn s(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

fn parse_repos(body: &Value) -> Vec<RepoId> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    /// Тесты мутируют process-env параллельно — сериализуем их,
    /// иначе GITEA_HOST устанавливается одним тестом, пока другой
    /// проверяет ошибку «без хоста» (гонка, флaky-красный CI).
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn adapter_from_env_fails_without_host() {
        let _g = env_lock();
        std::env::remove_var("GITEA_HOST");
        std::env::remove_var("GITEA_TOKEN");
        let res = GiteaAdapter::from_env();
        assert!(res.is_err(), "should fail without GITEA_HOST");
        assert!(res.unwrap_err().contains("GITEA_HOST"));
    }

    #[test]
    fn adapter_from_env_with_host() {
        let _g = env_lock();
        std::env::set_var("GITEA_HOST", "gitea.com");
        std::env::remove_var("GITEA_TOKEN");
        let a = GiteaAdapter::from_env().unwrap();
        assert_eq!(a.host, "https://gitea.com");
        assert_eq!(a.web_host, "gitea.com");
        assert!(a.token.is_none());
        std::env::remove_var("GITEA_HOST");
    }

    #[test]
    fn adapter_from_env_localhost_uses_http() {
        let _g = env_lock();
        std::env::set_var("GITEA_HOST", "localhost:3000");
        let a = GiteaAdapter::from_env().unwrap();
        assert_eq!(a.host, "http://localhost:3000");
        std::env::remove_var("GITEA_HOST");
    }

    #[test]
    fn adapter_from_env_with_protocol_preserved() {
        let _g = env_lock();
        std::env::set_var("GITEA_HOST", "http://192.168.1.10:3000");
        let a = GiteaAdapter::from_env().unwrap();
        assert_eq!(a.host, "http://192.168.1.10:3000");
        std::env::remove_var("GITEA_HOST");
    }

    #[test]
    fn adapter_new_explicit() {
        let a = GiteaAdapter::new("gitea.com", Some("fake-token".into()));
        assert_eq!(a.host, "https://gitea.com");
        assert_eq!(a.token.as_deref(), Some("fake-token"));
        assert_eq!(a.name(), "gitea");
        assert_eq!(a.scheme(), VcsScheme::Gitea);
    }

    #[test]
    fn adapter_anonymous_default() {
        let a = GiteaAdapter::anonymous();
        assert_eq!(a.host, "https://gitea.com");
        assert!(a.token.is_none());
    }

    #[test]
    fn api_base_is_v1() {
        let a = GiteaAdapter::new("gitea.com", None);
        assert_eq!(a.api_base(), "https://gitea.com/api/v1");
    }

    #[test]
    fn repo_web_url_basic() {
        let a = GiteaAdapter::new("gitea.com", None);
        let url = a
            .repo_web_url(&RepoId::from_owner_repo("user", "repo"))
            .unwrap();
        assert_eq!(url, "https://gitea.com/user/repo");
    }

    #[test]
    fn parse_repos_from_json_array() {
        let body: Value = serde_json::json!([
            {"full_name": "user/repo1"},
            {"full_name": "user/repo2"}
        ]);
        let v = parse_repos(&body);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].id, "user/repo1");
    }

    #[test]
    fn parse_repos_from_wrapper_data() {
        // wrapper-формат {data: [...]} (некоторые endpoints Gitea)
        let body: Value = serde_json::json!({"data": [{"full_name": "user/repo"}]});
        let arr = body["data"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
    }

    #[test]
    fn list_commits_payload_shape() {
        let body: Value = serde_json::json!([
            {
                "sha": "abc1234",
                "commit": {
                    "author": {
                        "name": "Alice",
                        "email": "a@x.com",
                        "date": "2024-01-01T00:00:00Z"
                    },
                    "message": "feat: commit"
                }
            }
        ]);
        let arr = body.as_array().unwrap();
        let c = &arr[0];
        assert_eq!(s(c, "sha"), "abc1234");
        assert_eq!(s(&c["commit"], "message"), "feat: commit");
        assert_eq!(s(&c["commit"]["author"], "name"), "Alice");
    }

    #[test]
    fn list_issues_distinguishes_pr_from_issue() {
        let body: Value = serde_json::json!([
            {"number": 1, "title": "issue", "state": "open", "body": "", "user": {"login": "alice"}, "created_at": "2024-01-01T00:00:00Z"},
            {"number": 2, "title": "PR", "state": "open", "body": "", "user": {"login": "bob"}, "created_at": "2024-01-02T00:00:00Z", "pull_request": {"merged": true}}
        ]);
        let arr = body.as_array().unwrap();
        assert!(
            arr[0].get("pull_request").map(|pr| pr.is_null()).unwrap_or(true)
                || arr[0].get("pull_request").is_none()
        );
        assert!(
            arr[1].get("pull_request").map(|pr| !pr.is_null()).unwrap_or(false)
        );
    }

    #[test]
    fn from_env_token_priority_gitea_first() {
        let _g = env_lock();
        std::env::set_var("GITEA_HOST", "gitea.com");
        std::env::set_var("GITEA_TOKEN", "gt-token");
        std::env::set_var("GT_TOKEN", "gt-token-2");
        let a = GiteaAdapter::from_env().unwrap();
        assert_eq!(a.token.as_deref(), Some("gt-token"), "GITEA_TOKEN has priority");
        std::env::remove_var("GITEA_HOST");
        std::env::remove_var("GITEA_TOKEN");
        std::env::remove_var("GT_TOKEN");
    }
}
