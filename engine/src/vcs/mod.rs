//! # Unified VCS & Data Mesh (v0.16.0)
//!
//! Каждый VCS (GitHub, GitLab, Gitea) и Pure-Rust git (gix) становится
//! source-адаптером, вливающим коммиты/issues/PR/files в `web-index.db`
//! как страницы по своим URL-схемам (`gh://`, `gl://`, `gt://`, `gix://`).
//!
//! Архитектурный инвариант v0.16.0 (см. FUTURE_ROADMAP.md §6.4):
//! **ноль новых зависимостей в схеме web-index.db** — VCS-страницы
//! используют те же `WebDoc` + `links` + `content_hash` + `positions` +
//! PageRank, что веб и NLM. URL-схема — единственное отличие.
//!
//! ```text
//!                ┌─────────────────────────────────────────┐
//!                │   poler_engine::vcs (Unified VCS Mesh)   │
//!                └────────────────────┬────────────────────┘
//!                                     │ VcsAdapter trait
//!   ┌────────────┬────────────────────┼────────────────────┬────────────┐
//!   ▼            ▼                    ▼                    ▼            ▼
//! github.rs   gitlab.rs           gitea.rs            local.rs    ingest.rs
//! (REST)      (REST v4)           (REST)              (gix)       (→WebDoc)
//! gh://       gl://               gt://               gix://
//! ```

pub mod gitea;
pub mod github;
pub mod gitlab;
pub mod ingest;
pub mod local;
pub mod clone;
pub mod lfs;

use crate::web::WebIndex;

/// Собрать URL VCS-страницы: `{scheme}://{path}` (например `gh://user/repo/commit/<sha>`).
pub fn url(scheme: VcsScheme, path: &str) -> String {
    format!("{scheme}://{path}")
}

/// Идентификатор схемы VCS-страницы в `web-index.db`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VcsScheme {
    /// `gh://` — GitHub (commits, issues, PRs, files via REST API).
    GitHub,
    /// `gl://` — GitLab (commits, MRs, pipelines via REST v4).
    GitLab,
    /// `gt://` — Gitea/Forgejo (commits, issues, PRs via REST).
    Gitea,
    /// `gix://` — локальный репозиторий, читаемый через Pure-Rust `gix` crate
    /// (clone/checkout/commits без git CLI).
    Gix,
}

impl VcsScheme {
    /// Строковое имя схемы (`"gh"`, `"gl"`, `"gt"`, `"gix"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GitHub => "gh",
            Self::GitLab => "gl",
            Self::Gitea => "gt",
            Self::Gix => "gix",
        }
    }

    /// Парсинг строкового имени схемы (case-insensitive).
    /// Назван `parse` вместо `from_str`, чтобы не конфликтовать с
    /// `std::str::FromStr::from_str` (clippy lint `should_implement_trait`).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "gh" | "github" => Ok(Self::GitHub),
            "gl" | "gitlab" => Ok(Self::GitLab),
            "gt" | "gitea" | "forgejo" => Ok(Self::Gitea),
            "gix" | "local" => Ok(Self::Gix),
            other => Err(format!("неизвестная VCS-схема: {other} (gh|gl|gt|gix)")),
        }
    }

    /// Все схемы (для `sync vcs all`).
    pub fn all() -> &'static [VcsScheme] {
        &[
            VcsScheme::GitHub,
            VcsScheme::GitLab,
            VcsScheme::Gitea,
            VcsScheme::Gix,
        ]
    }
}

impl std::fmt::Display for VcsScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Общие типы VCS-объектов (нейтральны к платформе)
// ---------------------------------------------------------------------------

/// Идентификатор репозитория. Для GitHub/Gitea — `"owner/repo"`,
/// для GitLab — URL-encoded path (`"group%2Fproject"` или просто `"group/project"`),
/// для локального gix — путь к репозиторию.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepoId {
    /// Платформенно-нейтральный идентификатор репозитория.
    pub id: String,
    /// Человекочитаемое имя (например `"poler-engine/poler-engine"`).
    pub display: String,
}

impl RepoId {
    pub fn new(id: impl Into<String>, display: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display: display.into(),
        }
    }

    /// Создать из `owner/repo` формы (GitHub/Gitea).
    pub fn from_owner_repo(owner: &str, repo: &str) -> Self {
        let id = format!("{owner}/{repo}");
        let display = id.clone();
        Self::new(id, display)
    }

    /// Создать из пути (локальный gix-репозиторий).
    pub fn from_path(p: &std::path::Path) -> Self {
        let id = p.display().to_string();
        let display = id.clone();
        Self::new(id, display)
    }
}

/// Коммит VCS.
#[derive(Debug, Clone)]
pub struct VcsCommit {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub author_email: String,
    pub authored_at: i64,
    /// URL веб-страницы коммита (для отображения и рёбер `links`).
    pub web_url: String,
}

/// Issue / Merge-Request / PR.
#[derive(Debug, Clone)]
pub struct VcsIssue {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: String,
    pub author: String,
    pub created_at: i64,
    pub web_url: String,
    /// `true` = MR/PR, `false` = issue.
    pub is_merge_request: bool,
}

/// Результат поиска по коду.
#[derive(Debug, Clone)]
pub struct VcsCodeHit {
    pub path: String,
    pub repo: String,
    pub sha: String,
    pub snippet: String,
    pub web_url: String,
}

/// Статистика синхронизации одного адаптера.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct VcsSyncStats {
    pub scheme: String,
    pub repos_synced: usize,
    pub commits_indexed: usize,
    pub issues_indexed: usize,
    pub unchanged: usize,
    pub errors: usize,
    pub elapsed_ms: u128,
}

// ---------------------------------------------------------------------------
// VcsAdapter — трейт, который реализует каждый адаптер
// ---------------------------------------------------------------------------

/// Source-адаптер для одной VCS-платформы.
///
/// Реализации: [`github::GithubAdapter`], [`gitlab::GitlabAdapter`],
/// [`gitea::GiteaAdapter`], [`local::GixAdapter`].
///
/// Каждый адаптер знает свой API-токен, базовый URL, и умеет листать
/// страницы VCS-объектов. Влитие в `web-index.db` делается единым хелпером
/// [`ingest::ingest_objects`] — адаптеры только отдают данные.
pub trait VcsAdapter {
    /// Имя адаптера (`"github"`, `"gitlab"`, `"gitea"`, `"gix"`).
    fn name(&self) -> &'static str;

    /// Схема URL, под которой адаптер кладёт страницы.
    fn scheme(&self) -> VcsScheme;

    /// Листинг репозиториев пользователя/группы (для `sync vcs <scheme>`).
    fn list_repos(&self, owner: &str) -> Result<Vec<RepoId>, String>;

    /// Последние N коммитов репозитория (для индексации в web-index.db).
    fn list_commits(&self, repo: &RepoId, limit: usize) -> Result<Vec<VcsCommit>, String>;

    /// Открытые issues/MR репозитория (для индексации).
    fn list_issues(&self, repo: &RepoId, limit: usize) -> Result<Vec<VcsIssue>, String>;

    /// Поиск кода в репозиториях (если платформа поддерживает).
    fn search_code(&self, query: &str, limit: usize) -> Result<Vec<VcsCodeHit>, String>;
}

// ---------------------------------------------------------------------------
// Фабрики адаптеров (с токенами из окружения)
// ---------------------------------------------------------------------------

/// Создать GitHub-адаптер, читая токен из `$GITHUB_TOKEN` / `$GH_TOKEN`.
pub fn github_adapter() -> Result<github::GithubAdapter, String> {
    github::GithubAdapter::from_env()
}

/// Создать GitLab-адаптер, читая токен из `$GITLAB_TOKEN` / `$GL_TOKEN`.
pub fn gitlab_adapter() -> Result<gitlab::GitlabAdapter, String> {
    gitlab::GitlabAdapter::from_env()
}

/// Создать Gitea-адаптер, читая токен из `$GITEA_TOKEN` / `$GT_TOKEN`.
pub fn gitea_adapter() -> Result<gitea::GiteaAdapter, String> {
    gitea::GiteaAdapter::from_env()
}

/// Создать gix-адаптер (без токена — работает с локальными путями).
pub fn gix_adapter() -> local::GixAdapter {
    local::GixAdapter::default()
}

// ---------------------------------------------------------------------------
// sync_vcs — диспетч синхронизации всех VCS в web-index.db
// ---------------------------------------------------------------------------

/// Запустить синхронизацию одной или всех VCS-площадок. Каждый коммит/issue
/// становится страницей в `web-index.db` (URL-схема `gh://`/`gl://`/`gt://`).
/// Повторный синк идемпотентен: `content_hash` пропускает неизменившиеся.
pub fn sync_vcs(
    ix: &mut WebIndex,
    scheme: Option<VcsScheme>,
    owner: Option<&str>,
    limit: usize,
) -> Vec<VcsSyncStats> {
    let mut out = Vec::new();
    let schemes = match scheme {
        Some(s) => vec![s],
        None => VcsScheme::all().to_vec(),
    };
    for s in schemes {
        let t0 = std::time::Instant::now();
        let mut st = VcsSyncStats {
            scheme: s.to_string(),
            ..Default::default()
        };
        let owner_ref = owner.unwrap_or("");
        let res = match s {
            VcsScheme::GitHub => github_adapter()
                .and_then(|a| sync_one_adapter(ix, &a, owner_ref, limit, &mut st)),
            VcsScheme::GitLab => gitlab_adapter()
                .and_then(|a| sync_one_adapter(ix, &a, owner_ref, limit, &mut st)),
            VcsScheme::Gitea => gitea_adapter()
                .and_then(|a| sync_one_adapter(ix, &a, owner_ref, limit, &mut st)),
            VcsScheme::Gix => {
                // gix работает с локальными путями, не с owner'ом.
                // Здесь — no-op для глобального sync; пользователь делает
                // `poler> gix clone <URL> <PATH>` отдельно.
                st.elapsed_ms = t0.elapsed().as_millis();
                out.push(st);
                continue;
            }
        };
        if let Err(e) = res {
            st.errors += 1;
            st.elapsed_ms = t0.elapsed().as_millis();
            eprintln!("vcs::sync_vcs: {s}: {e}");
        }
        st.elapsed_ms = t0.elapsed().as_millis();
        out.push(st);
    }
    out
}

/// Синхронизация одного адаптера: листинг репозиториев → коммиты → issues
/// → влитие в web-index.db через `ingest_objects`.
fn sync_one_adapter(
    ix: &mut WebIndex,
    adapter: &dyn VcsAdapter,
    owner: &str,
    limit: usize,
    st: &mut VcsSyncStats,
) -> Result<(), String> {
    if owner.is_empty() {
        return Err(
            "vcs::sync: укажите owner (например `sync vcs github poler-engine`)".to_string(),
        );
    }
    let repos = adapter.list_repos(owner)?;
    st.repos_synced = repos.len();
    for repo in &repos {
        // коммиты
        match adapter.list_commits(repo, limit) {
            Ok(commits) => {
                let docs = ingest::commits_to_docs(adapter.scheme(), repo, &commits);
                let mut idx = 0;
                let mut unc = 0;
                for doc in &docs {
                    match ix.upsert_page(doc) {
                        Ok((_, was_new)) => {
                            if was_new {
                                idx += 1;
                            } else {
                                unc += 1;
                            }
                        }
                        Err(_) => st.errors += 1,
                    }
                }
                st.commits_indexed += idx;
                st.unchanged += unc;
            }
            Err(e) => {
                st.errors += 1;
                eprintln!("vcs::sync: {} commits {}: {e}", adapter.name(), repo.display);
            }
        }
        // issues/MR
        match adapter.list_issues(repo, limit) {
            Ok(issues) => {
                let docs = ingest::issues_to_docs(adapter.scheme(), repo, &issues);
                let mut idx = 0;
                let mut unc = 0;
                for doc in &docs {
                    match ix.upsert_page(doc) {
                        Ok((_, was_new)) => {
                            if was_new {
                                idx += 1;
                            } else {
                                unc += 1;
                            }
                        }
                        Err(_) => st.errors += 1,
                    }
                }
                st.issues_indexed += idx;
                st.unchanged += unc;
            }
            Err(e) => {
                st.errors += 1;
                eprintln!("vcs::sync: {} issues {}: {e}", adapter.name(), repo.display);
            }
        }
    }
    // после влития — переcчёт PageRank по всем рёбрам (вкл. новые vcs://)
    let _ = ix.recompute_pagerank(20);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vcs_scheme_roundtrip() {
        for s in VcsScheme::all() {
            let name = s.to_string();
            assert_eq!(VcsScheme::parse(&name).unwrap(), *s);
        }
    }

    #[test]
    fn vcs_scheme_from_long_name() {
        assert_eq!(VcsScheme::parse("github").unwrap(), VcsScheme::GitHub);
        assert_eq!(VcsScheme::parse("GitLab").unwrap(), VcsScheme::GitLab);
        assert_eq!(VcsScheme::parse("forgejo").unwrap(), VcsScheme::Gitea);
        assert!(VcsScheme::parse("bogus").is_err());
    }

    #[test]
    fn url_format() {
        assert_eq!(
            url(VcsScheme::GitHub, "user/repo/commit/abc"),
            "gh://user/repo/commit/abc"
        );
        assert_eq!(
            url(VcsScheme::GitLab, "grp/proj/issues/42"),
            "gl://grp/proj/issues/42"
        );
        assert_eq!(
            url(VcsScheme::Gitea, "owner/name/pulls/7"),
            "gt://owner/name/pulls/7"
        );
        assert_eq!(
            url(VcsScheme::Gix, "/home/x/repo/commit/def"),
            "gix:///home/x/repo/commit/def"
        );
    }

    #[test]
    fn repo_id_from_owner_repo() {
        let r = RepoId::from_owner_repo("poler-engine", "poler-engine");
        assert_eq!(r.id, "poler-engine/poler-engine");
        assert_eq!(r.display, "poler-engine/poler-engine");
    }

    #[test]
    fn repo_id_from_path() {
        let r = RepoId::from_path(std::path::Path::new("/home/x/repo"));
        assert_eq!(r.id, "/home/x/repo");
        assert_eq!(r.display, "/home/x/repo");
    }

    #[test]
    fn all_schemes_complete() {
        let all = VcsScheme::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&VcsScheme::GitHub));
        assert!(all.contains(&VcsScheme::GitLab));
        assert!(all.contains(&VcsScheme::Gitea));
        assert!(all.contains(&VcsScheme::Gix));
    }
}
