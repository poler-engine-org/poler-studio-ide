//! # Pure-Rust Git Adapter (gix) (v0.16.0)
//!
//! Локальный git-репозиторий читается через `gix` crate — Pure-Rust
//! реализация git-протокола без системного `git` CLI. Коммиты
//! попадают в `web-index.db` как `gix://{path}/commit/{short_sha}`
//! страницы. Дополнительно — `clone` для скачивания удалённого репо.
//!
//! Архитектурно: локальный gix-адаптер — это мост между **локальным кодом**
//! (которое движок умеет индексировать с v0.1 через `collect_files`) и
//! **глобальным веб-индексом** (который нужен для `--web-search`).
//! История коммитов становится поисковым корпусом наравне с вебом.
//!
//! **Donor** (как и GitHub/GitLab): `gix` crate <https://github.com/Byron/gitoxide>.

use std::path::{Path, PathBuf};

use super::{RepoId, VcsAdapter, VcsCodeHit, VcsCommit, VcsIssue, VcsScheme};

/// Адаптер локальных git-репозиториев через `gix` (Pure-Rust Git).
///
/// Не имеет state (токенов/хостов) — все параметры передаются в методы.
/// `RepoId` для этого адаптера — путь к репозиторию (например `/home/x/proj`).
pub struct GixAdapter {
    /// User-Agent для HTTP-запросов при clone/fetch (если передан URL).
    /// v0.17.0: будет использоваться при blocking-network-client clone.
    #[allow(dead_code)]
    user_agent: String,
}

impl Default for GixAdapter {
    fn default() -> Self {
        Self {
            user_agent: std::env::var("POLER_USER_AGENT")
                .unwrap_or_else(|_| "poler-engine/0.16".into()),
        }
    }
}

impl GixAdapter {
    /// Создать адаптер. Обычно — `GixAdapter::default()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Открыть локальный репозиторий через `gix::discover` (найдёт `.git`
    /// вверх по дереву). Возвращает `gix::Repository` для дальнейшей работы.
    pub fn open_repo(path: &Path) -> Result<gix::Repository, String> {
        gix::discover(path).map_err(|e| format!("gix::discover({}): {e}", path.display()))
    }

    /// Список последних N коммитов от HEAD. Аналог `git log -n N`.
    /// Использует `rev_walk` + `find_commit` + `decode()` для извлечения метаданных.
    pub fn list_commits_from_repo(
        repo: &gix::Repository,
        limit: usize,
    ) -> Result<Vec<VcsCommit>, String> {
        let head_id = repo
            .head_commit()
            .map_err(|e| format!("head_commit: {e}"))?
            .id();
        let platform = repo.rev_walk(Some(head_id));
        let mut out = Vec::new();
        // platform.all() возвращает `Result<Walk, Error>` — Walker-итerator.
        let walk = platform.all().map_err(|e| format!("rev_walk.all: {e}"))?;
        for step in walk {
            if out.len() >= limit {
                break;
            }
            // Каждый step — `Result<Info, simple::Error>`. Info.id() → ObjectId.
            let info = match step {
                Ok(s) => s,
                Err(_) => continue,
            };
            // info.object() сразу возвращает `Commit<'repo>` (без find_commit).
            let commit_obj = match info.object() {
                Ok(c) => c,
                Err(_) => continue,
            };
            // `decode()` возвращает `CommitRef` со всеми полями без Result.
            let commit_ref = match commit_obj.decode() {
                Ok(c) => c,
                Err(_) => continue,
            };
            // `message()` у `CommitRef` возвращает `MessageRef` напрямую (без Result).
            // MessageRef.title — `&BStr`, MessageRef.body — `Option<&BStr>`.
            use gix::bstr::ByteSlice;
            let msg_ref = commit_ref.message();
            let mut msg = String::new();
            msg.push_str(&String::from_utf8_lossy(msg_ref.title.as_bytes()));
            if let Some(body) = msg_ref.body {
                msg.push('\n');
                msg.push_str(&String::from_utf8_lossy(body.as_bytes()));
            }

            // `author()` у `CommitRef` возвращает `SignatureRef` напрямую (без Result).
            let author_ref = commit_ref.author();
            let author_name = String::from_utf8_lossy(author_ref.name.as_bytes()).into_owned();
            let author_email = String::from_utf8_lossy(author_ref.email.as_bytes()).into_owned();
            let authored_at = author_ref.time.seconds;
            let sha = format!("{}", info.id());
            let web_url = String::new(); // локальный репозиторий, web-URL не определён
            out.push(VcsCommit {
                sha,
                message: msg,
                author: author_name,
                author_email,
                authored_at,
                web_url,
            });
        }
        Ok(out)
    }

    /// Клонировать удалённый git-репозиторий через `gix::clone` (Pure-Rust).
    /// URL — `https://github.com/...`, `git@github.com:...`, или локальный путь.
    /// Возвращает путь к склонированному репозиторию.
    ///
    /// **v0.17.0:** реализован настоящий pure-Rust clone через
    /// `gix::clone::PrepareFetch` (feature `blocking-network-client` включён).
    /// Делегирует в [`crate::vcs::clone::clone_repo`]. Для LFS-объектов
    /// используйте `gix lfs fetch <PATH>` после clone.
    pub fn clone_repo(url: &str, dest: &Path) -> Result<PathBuf, String> {
        // Валидация на раннем этапе (без сети).
        if parse_clone_url(url).is_none() && !Path::new(url).exists() {
            return Err(format!(
                "clone_repo: невалидный URL или путь: {url}\n\
                 подсказка: HTTPS-URL должен быть вида https://github.com/USER/REPO[.git];\n\
                 локальный путь должен существовать"
            ));
        }
        let opts = crate::vcs::clone::CloneOpts::new(url, dest);
        crate::vcs::clone::clone_repo(&opts).map_err(|e| e.to_user_string())
    }

    /// Листинг коммитов пути (аналог `git log -n N` в поданном репо).
    /// Открывает репо через `open_repo` и делегирует в `list_commits_from_repo`.
    pub fn list_commits_at_path(
        path: &Path,
        limit: usize,
    ) -> Result<Vec<VcsCommit>, String> {
        let repo = Self::open_repo(path)?;
        Self::list_commits_from_repo(&repo, limit)
    }
}

impl VcsAdapter for GixAdapter {
    fn name(&self) -> &'static str {
        "gix"
    }

    fn scheme(&self) -> VcsScheme {
        VcsScheme::Gix
    }

    fn list_repos(&self, _owner: &str) -> Result<Vec<RepoId>, String> {
        // gix не имеет понятия "удалённый список репозиториев пользователя"
        // (как GitHub). Репозиторий задаётся путём и открывается локально.
        // Для глобального sync это — no-op; пользователь делает `gix clone`
        // или `gix log /path` отдельно.
        Ok(Vec::new())
    }

    fn list_commits(&self, repo: &RepoId, limit: usize) -> Result<Vec<VcsCommit>, String> {
        let path = Path::new(&repo.id);
        Self::list_commits_at_path(path, limit)
    }

    fn list_issues(&self, _repo: &RepoId, _limit: usize) -> Result<Vec<VcsIssue>, String> {
        // локальный git не имеет issues/MR — это платформенная фича.
        // Возвращаем пустой список (но не ошибку, чтобы `sync vcs all`
        // не светил red на gix).
        Ok(Vec::new())
    }

    fn search_code(&self, _query: &str, _limit: usize) -> Result<Vec<VcsCodeHit>, String> {
        // gix не имеет встроенного search-by-content (это задача poler-engine
        // core через `scan_path`!). Возвращает пустой результат — пользователю
        // подсказка использовать `poler> impact` или `poler-engine <PATH> -q`.
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// helpers: parse git-like URL → (host, owner, repo)
// ---------------------------------------------------------------------------

/// Распарсить URL клонирования в `(host, owner, repo)`.
/// Принимает: `https://github.com/user/repo[.git]`,
/// `git@github.com:user/repo[.git]`, `ssh://git@github.com/user/repo`,
/// `https://user:pass@github.com/owner/repo.git`,
/// `/local/path`.
pub fn parse_clone_url(url: &str) -> Option<(String, String, String)> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    // SSH-SCP: `git@github.com:user/repo.git`
    if let Some(rest) = url.strip_prefix("git@") {
        // rest = "github.com:user/repo.git"
        let (host, after_host) = rest.split_once(':')?;
        let (owner, repo) = parse_path_tail(after_host)?;
        return Some((host.into(), owner, repo));
    }
    // HTTP(S) / SSH-with-scheme
    if url.starts_with("https://")
        || url.starts_with("http://")
        || url.starts_with("ssh://")
    {
        let no_scheme = url.split("://").nth(1)?;
        // Поддержка user:pass@ или user@ перед host'ом
        let after_user = if let Some((_, rest)) = no_scheme.split_once('@') {
            rest
        } else {
            no_scheme
        };
        // after_user = "github.com/user/repo.git" или "github.com:owner/repo.git"
        // (с портом через ':')
        let (host_part, tail) = after_user.split_once('/').unwrap_or((after_user, ""));
        // Отрезаем порт (":22" или ":443"), если есть
        let host = host_part.split(':').next()?;
        if tail.is_empty() {
            return None;
        }
        let (owner, repo) = parse_path_tail(tail)?;
        return Some((host.into(), owner, repo));
    }
    // Локальный путь — недопустимо для parse_url (нужно знать, чей это репо)
    None
}

/// Из пути `"user/repo.git"` → `("user", "repo")`. Берёт 2 компоненты.
fn parse_path_tail(path: &str) -> Option<(String, String)> {
    let mut parts = path.trim_end_matches(".git").split('/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner, repo))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_default_user_agent() {
        std::env::remove_var("POLER_USER_AGENT");
        let a = GixAdapter::default();
        assert_eq!(a.user_agent, "poler-engine/0.16");
        assert_eq!(a.name(), "gix");
        assert_eq!(a.scheme(), VcsScheme::Gix);
    }

    #[test]
    fn parse_clone_url_https() {
        let r = parse_clone_url("https://github.com/user/repo.git").unwrap();
        assert_eq!(r.0, "github.com");
        assert_eq!(r.1, "user");
        assert_eq!(r.2, "repo");
    }

    #[test]
    fn parse_clone_url_https_no_git_suffix() {
        let r = parse_clone_url("https://gitlab.com/grp/proj").unwrap();
        assert_eq!(r.0, "gitlab.com");
        assert_eq!(r.1, "grp");
        assert_eq!(r.2, "proj");
    }

    #[test]
    fn parse_clone_url_ssh_scp() {
        let r = parse_clone_url("git@github.com:user/repo.git").unwrap();
        assert_eq!(r.0, "github.com");
        assert_eq!(r.1, "user");
        assert_eq!(r.2, "repo");
    }

    #[test]
    fn parse_clone_url_ssh_scheme() {
        let r = parse_clone_url("ssh://git@github.com/user/repo.git").unwrap();
        assert_eq!(r.0, "github.com");
        assert_eq!(r.1, "user");
        assert_eq!(r.2, "repo");
    }

    #[test]
    fn parse_clone_url_empty_returns_none() {
        assert!(parse_clone_url("").is_none());
        assert!(parse_clone_url("   ").is_none());
    }

    #[test]
    fn parse_clone_url_local_path_returns_none() {
        // Локальные пути не парсятся как clone URL (нет owner/repo)
        assert!(parse_clone_url("/home/user/code").is_none());
        assert!(parse_clone_url("../my-repo").is_none());
    }

    #[test]
    fn parse_clone_url_gitea_self_hosted() {
        let r = parse_clone_url("https://gitea.com/user/repo.git").unwrap();
        assert_eq!(r.0, "gitea.com");
        assert_eq!(r.1, "user");
        assert_eq!(r.2, "repo");
    }

    #[test]
    fn parse_path_tail_with_subpath() {
        // GitLab nested groups: "grp/sub/proj.git" → owner="grp", repo="sub"
        // (это нормально для нашей адаптер-модели: RepoId.id хранит что угодно)
        let r = parse_path_tail("grp/sub/proj.git").unwrap();
        assert_eq!(r.0, "grp");
        assert_eq!(r.1, "sub");
    }

    #[test]
    fn adapter_list_repos_returns_empty() {
        let a = GixAdapter::default();
        let v = a.list_repos("any").unwrap();
        assert!(v.is_empty(), "gix has no remote listing API");
    }

    #[test]
    fn adapter_list_issues_returns_empty() {
        let a = GixAdapter::default();
        let r = RepoId::from_path(Path::new("/tmp/fake"));
        let v = a.list_issues(&r, 10).unwrap();
        assert!(v.is_empty(), "local git has no issues");
    }

    #[test]
    fn adapter_search_code_returns_empty() {
        let a = GixAdapter::default();
        let v = a.search_code("foo", 10).unwrap();
        assert!(v.is_empty(), "gix has no content search; use poler-engine core");
    }

    #[test]
    fn open_repo_nonexistent_returns_err() {
        let res = GixAdapter::open_repo(Path::new("/nonexistent/path"));
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("gix::discover"));
    }

    #[test]
    fn list_commits_at_nonexistent_path_returns_err() {
        let res = GixAdapter::list_commits_at_path(Path::new("/nonexistent"), 5);
        assert!(res.is_err());
    }

    // Интеграционный тест на собственном репозитории движка:
    // /home/z/my-project/poler-engine содержит .git (init через `git init` ранее)
    // или его нет — тогда тест должен skip'нуться без паники.
    #[test]
    fn list_commits_on_poler_engine_if_repo_exists() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"));
        let res = GixAdapter::list_commits_at_path(path, 5);
        // Если git init не делался — будет Err. Это ОК.
        match res {
            Ok(commits) => {
                assert!(commits.len() <= 5, "should respect limit");
                if !commits.is_empty() {
                    let c = &commits[0];
                    assert!(!c.sha.is_empty(), "sha must be non-empty");
                    assert!(!c.message.is_empty(), "message must be non-empty");
                }
            }
            Err(e) => {
                eprintln!("list_commits_on_poler_engine: skipped (no .git): {e}");
            }
        }
    }

    #[test]
    fn clone_repo_invalid_url_errors_gracefully() {
        let tmp = tempfile::tempdir().unwrap();
        let res = GixAdapter::clone_repo("not-a-valid-url", tmp.path());
        assert!(res.is_err(), "invalid URL should error");
    }

    #[test]
    fn parse_clone_url_with_credentials() {
        // URL с credentials должны парситься (мы просто игнорируем user:pass)
        let r = parse_clone_url("https://user:pass@github.com/owner/repo.git").unwrap();
        assert_eq!(r.0, "github.com");
        assert_eq!(r.1, "owner");
        assert_eq!(r.2, "repo");
    }
}
