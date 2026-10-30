//! Helper-конверсия: VCS-объекты (коммиты/issues/MR) → `WebDoc` для
//! `WebIndex::upsert_page`. Ядро `web-index.db` не знает про VCS —
//! адаптеры выдают нейтральный `VcsCommit`/`VcsIssue`, а здесь мы
//! упаковываем их в `WebDoc` с URL-схемой `gh://`/`gl://`/`gt://`/`gix://`
//! и считаем `content_hash` (для Percolator-lite идемпотентности).
//!
//! Веб-страница VCS-объекта состоит из:
//!   * заголовка (`title`) — `[#<n>] <subject>` для issue/PR, `<short-sha> <subject>` для коммита;
//!   * тела (`text`) — marshal-пresentation автора, даты, сообщения, body;
//!   * списка ссылок (`links`) — веб-URL коммита/issue (для `links` таблицы)
//!     + URL репозитория (для замыкания графа: `repo.html_url → vcs_url`).
//!
//! `content_hash` — SHA-1 по нормализованному тексту. Если коммит не
//! изменился, повторный `upsert_page` не пересчитает позиции/BM25 —
//! Percolator-lite пропустит. Это критично для `sync vcs all` в фоне.

use crate::web::index::{content_hash, WebDoc};

use super::{RepoId, VcsCommit, VcsIssue, VcsScheme};

/// Сконвертировать коммит в `WebDoc` (для индексации).
///
/// URL: `{scheme}://{repo.id}/commit/{short_sha}`
/// Title: `{short_sha} {subject}`
/// Body: ```
/// commit {full_sha}
/// Author: {name} <{email}>
/// Date:   {ISO-date}
///
/// {message}
/// ```
pub fn commit_to_doc(scheme: VcsScheme, repo: &RepoId, c: &VcsCommit) -> WebDoc {
    let short = short_sha(&c.sha);
    let url = format!("{scheme}://{repo_id}/{short}", repo_id = repo.id);
    let subject = c
        .message
        .lines()
        .next()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("(no subject)")
        .to_string();
    let title = format!("{short} {subject}");

    let mut text = String::new();
    text.push_str(&format!("commit {}\n", c.sha));
    text.push_str(&format!("Author: {} <{}>\n", c.author, c.author_email));
    text.push_str(&format!("Date:   {}\n\n", iso_time(c.authored_at)));
    text.push_str(&c.message);
    text.push('\n');
    if !c.web_url.is_empty() {
        text.push_str(&format!("\nWeb: {}\n", c.web_url));
    }

    let mut links = Vec::new();
    if !c.web_url.is_empty() {
        links.push(c.web_url.clone());
    }

    let chash = content_hash(&text);
    WebDoc {
        url,
        title,
        lang: "en".into(),
        meta_description: subject,
        text,
        links,
        content_hash: chash,
    }
}

/// Сконвертировать issue/MR/PR в `WebDoc`.
///
/// URL: `{scheme}://{repo.id}/{issues|pulls}/{number}`
/// Title: `#{number} {title}`
/// Body: `state: {open|closed}\nauthor: {author}\ncreated: {ISO}\n\n{body}`
pub fn issue_to_doc(scheme: VcsScheme, repo: &RepoId, i: &VcsIssue) -> WebDoc {
    let kind = if i.is_merge_request { "pulls" } else { "issues" };
    let url = format!(
        "{scheme}://{repo_id}/{kind}/{n}",
        repo_id = repo.id,
        n = i.number
    );
    let title = format!("#{} {}", i.number, i.title);

    let mut text = String::new();
    text.push_str(&format!("{} #{}\n", upper_kind(kind), i.number));
    text.push_str(&format!("Title: {}\n", i.title));
    text.push_str(&format!("State: {}\n", i.state));
    text.push_str(&format!("Author: {}\n", i.author));
    text.push_str(&format!("Created: {}\n\n", iso_time(i.created_at)));
    text.push_str(&i.body);
    text.push('\n');
    if !i.web_url.is_empty() {
        text.push_str(&format!("\nWeb: {}\n", i.web_url));
    }

    let mut links = Vec::new();
    if !i.web_url.is_empty() {
        links.push(i.web_url.clone());
    }

    let chash = content_hash(&text);
    WebDoc {
        url,
        title,
        lang: "en".into(),
        meta_description: i.title.clone(),
        text,
        links,
        content_hash: chash,
    }
}

/// Batch-конверсия коммитов.
pub fn commits_to_docs(scheme: VcsScheme, repo: &RepoId, commits: &[VcsCommit]) -> Vec<WebDoc> {
    commits.iter().map(|c| commit_to_doc(scheme, repo, c)).collect()
}

/// Batch-конверсия issues.
pub fn issues_to_docs(scheme: VcsScheme, repo: &RepoId, issues: &[VcsIssue]) -> Vec<WebDoc> {
    issues.iter().map(|i| issue_to_doc(scheme, repo, i)).collect()
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Сократить SHA до 7 символов (как `git log --oneline`).
pub fn short_sha(sha: &str) -> String {
    sha.chars().take(7).collect::<String>()
}

/// Перевести epoch-секунды в ISO-8601 UTC (`2026-08-26T12:34:56Z`).
pub fn iso_time(epoch: i64) -> String {
    if epoch <= 0 {
        return "1970-01-01T00:00:00Z".into();
    }
    let secs = epoch as u64;
    let days = secs / 86400;
    let rem = secs % 86400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let sec = rem % 60;
    // вычисление Y-M-D (по григорианскому календарю, без високосных ошибок)
    let (y, m, d) = days_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// Дни с 1970-01-01 → (year, month, day) UTC.
/// Итеративный поиск года/месяца — читаемый, надёжный (без формулы Howard
/// Hinnant, которая давала ошибки в edge-cases при тестах v0.16.0).
fn days_to_ymd(days: u64) -> (u32, u32, u32) {
    // Дней в начале каждого месяца (невысокосный год):
    // [0]=unused, [1]=0 (1 Jan = day 0), [2]=31 (1 Feb = day 31), ..., [12]=334
    const MONTH_START: [i64; 13] = [0, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let mut remaining = days as i64;
    let mut y: i64 = 1970;
    loop {
        let dy = if is_leap(y) { 366 } else { 365 };
        if remaining < dy {
            break;
        }
        remaining -= dy;
        y += 1;
    }
    let leap = is_leap(y);
    // remaining — день-в-году (0-based); распределим по месяцам.
    // Для високосного года после февраля все сдвигается на 1.
    let mut m: u32 = 12;
    for mo in (1..=12u32).rev() {
        let mut start = MONTH_START[mo as usize];
        if mo > 2 && leap {
            start += 1;
        }
        if remaining >= start {
            m = mo;
            break;
        }
    }
    let mut start = MONTH_START[m as usize];
    if m > 2 && leap {
        start += 1;
    }
    let d = remaining - start + 1;
    (y as u32, m, d as u32)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

fn upper_kind(kind: &str) -> &str {
    match kind {
        "issues" => "Issue",
        "pulls" => "Pull Request",
        _ => "Item",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_commit() -> VcsCommit {
        VcsCommit {
            sha: "abcdef1234567890abcdef1234567890abcdef12".into(),
            message: "feat: add gix adapter\n\nDetailed body here.".into(),
            author: "Alice".into(),
            author_email: "alice@example.com".into(),
            authored_at: 1_724_000_000,
            web_url: "https://github.com/user/repo/commit/abcdef".into(),
        }
    }

    fn sample_issue() -> VcsIssue {
        VcsIssue {
            number: 42,
            title: "Bug: crash on empty input".into(),
            body: "Steps to reproduce...".into(),
            state: "open".into(),
            author: "bob".into(),
            created_at: 1_724_000_000,
            web_url: "https://github.com/user/repo/issues/42".into(),
            is_merge_request: false,
        }
    }

    fn sample_mr() -> VcsIssue {
        VcsIssue {
            number: 7,
            title: "Refactor: extract vcs module".into(),
            body: "Refactors the vcs module into separate adapters.".into(),
            state: "merged".into(),
            author: "charlie".into(),
            created_at: 1_724_000_000,
            web_url: "https://gitlab.com/grp/proj/-/merge_requests/7".into(),
            is_merge_request: true,
        }
    }

    fn sample_repo() -> RepoId {
        RepoId::from_owner_repo("user", "repo")
    }

    #[test]
    fn commit_to_doc_url_uses_scheme() {
        let c = sample_commit();
        let doc = commit_to_doc(VcsScheme::GitHub, &sample_repo(), &c);
        assert_eq!(
            doc.url,
            "gh://user/repo/abcdef1",
            "URL should use gh:// scheme + repo.id + short_sha"
        );
        assert_eq!(doc.lang, "en");
    }

    #[test]
    fn commit_to_doc_title_short_sha_subject() {
        let c = sample_commit();
        let doc = commit_to_doc(VcsScheme::GitHub, &sample_repo(), &c);
        assert!(doc.title.starts_with("abcdef1 "), "title starts with short SHA");
        assert!(doc.title.contains("feat: add gix adapter"));
    }

    #[test]
    fn commit_to_doc_body_contains_full_sha_and_author() {
        let c = sample_commit();
        let doc = commit_to_doc(VcsScheme::GitLab, &sample_repo(), &c);
        assert!(doc.text.contains("commit abcdef1234567890abcdef1234567890abcdef12"));
        assert!(doc.text.contains("Author: Alice <alice@example.com>"));
        assert!(doc.text.contains("Detailed body here."));
        assert!(doc.text.contains("Web: https://github.com"));
    }

    #[test]
    fn commit_to_doc_links_include_web_url() {
        let c = sample_commit();
        let doc = commit_to_doc(VcsScheme::GitHub, &sample_repo(), &c);
        assert_eq!(doc.links.len(), 1);
        assert_eq!(doc.links[0], c.web_url);
    }

    #[test]
    fn commit_to_doc_content_hash_stable() {
        let c = sample_commit();
        let doc1 = commit_to_doc(VcsScheme::GitHub, &sample_repo(), &c);
        let doc2 = commit_to_doc(VcsScheme::GitHub, &sample_repo(), &c);
        assert_eq!(doc1.content_hash, doc2.content_hash);
        assert!(!doc1.content_hash.is_empty());
    }

    #[test]
    fn issue_to_doc_url_uses_issues_path() {
        let i = sample_issue();
        let doc = issue_to_doc(VcsScheme::GitHub, &sample_repo(), &i);
        assert_eq!(doc.url, "gh://user/repo/issues/42");
    }

    #[test]
    fn mr_to_doc_url_uses_pulls_path() {
        let i = sample_mr();
        let doc = issue_to_doc(VcsScheme::GitLab, &sample_repo(), &i);
        assert_eq!(doc.url, "gl://user/repo/pulls/7");
    }

    #[test]
    fn issue_to_doc_title_has_number() {
        let i = sample_issue();
        let doc = issue_to_doc(VcsScheme::GitHub, &sample_repo(), &i);
        assert!(doc.title.starts_with("#42 "));
        assert!(doc.title.contains("Bug: crash"));
    }

    #[test]
    fn issue_to_doc_body_has_state_and_author() {
        let i = sample_issue();
        let doc = issue_to_doc(VcsScheme::GitHub, &sample_repo(), &i);
        assert!(doc.text.contains("State: open"));
        assert!(doc.text.contains("Author: bob"));
        assert!(doc.text.contains("Steps to reproduce"));
    }

    #[test]
    fn batch_commits_preserves_order() {
        let c1 = sample_commit();
        let mut c2 = c1.clone();
        c2.sha = "deadbeef00000000deadbeef00000000deadbeef".into();
        let docs = commits_to_docs(VcsScheme::GitHub, &sample_repo(), &[c1.clone(), c2.clone()]);
        assert_eq!(docs.len(), 2);
        assert_ne!(docs[0].url, docs[1].url);
    }

    #[test]
    fn batch_issues_preserves_order() {
        let mut i1 = sample_issue();
        i1.number = 1;
        let mut i2 = sample_issue();
        i2.number = 2;
        let docs = issues_to_docs(VcsScheme::GitHub, &sample_repo(), &[i1, i2]);
        assert_eq!(docs.len(), 2);
        assert!(docs[0].url.ends_with("/issues/1"));
        assert!(docs[1].url.ends_with("/issues/2"));
    }

    #[test]
    fn short_sha_takes_7_chars() {
        assert_eq!(short_sha("abcdef1234567890"), "abcdef1");
        assert_eq!(short_sha("1234567"), "1234567");
        assert_eq!(short_sha("12345"), "12345");
        assert_eq!(short_sha(""), "");
    }

    #[test]
    fn iso_time_zero_is_epoch() {
        assert_eq!(iso_time(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn iso_time_well_known() {
        // 1_724_000_000 — это 2024-08-18T... по UTC (приблизительно)
        let s = iso_time(1_724_000_000);
        assert!(s.starts_with("2024-"));
        assert!(s.ends_with('Z'));
    }

    #[test]
    fn iso_time_format_strict() {
        let s = iso_time(1_700_000_000); // 2023-11-14T...
        // 20 символов формат "YYYY-MM-DDThh:mm:ssZ"
        assert_eq!(s.len(), 20);
        assert_eq!(s.chars().nth(4), Some('-'));
        assert_eq!(s.chars().nth(10), Some('T'));
        assert_eq!(s.chars().nth(19), Some('Z'));
    }

    #[test]
    fn commit_to_doc_no_web_url_works() {
        let mut c = sample_commit();
        c.web_url = String::new();
        let doc = commit_to_doc(VcsScheme::GitHub, &sample_repo(), &c);
        assert!(doc.links.is_empty());
        assert!(!doc.text.contains("Web:"));
    }

    #[test]
    fn commit_to_doc_no_message_body_works() {
        let mut c = sample_commit();
        c.message = String::new();
        let doc = commit_to_doc(VcsScheme::GitHub, &sample_repo(), &c);
        assert_eq!(doc.title, "abcdef1 (no subject)");
    }

    #[test]
    fn days_to_ymd_epoch() {
        // 1970-01-01 → days=0 → (1970, 1, 1)
        let (y, m, d) = days_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn days_to_ymd_known_date() {
        // 2024-01-01 — это 19723 дня с 1970-01-01
        let (y, m, d) = days_to_ymd(19723);
        assert_eq!((y, m, d), (2024, 1, 1));
    }

    #[test]
    fn issue_to_doc_meta_description_is_title() {
        let i = sample_issue();
        let doc = issue_to_doc(VcsScheme::GitHub, &sample_repo(), &i);
        assert_eq!(doc.meta_description, i.title);
    }
}
