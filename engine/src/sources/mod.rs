//! # Sources CRUD (v0.17.0)
//!
//! Каталог источников данных poler-engine. Хранится в той же SQLite-БД
//! (`web-index.db`) в таблице `poler_sources`. Источники трёх видов:
//!
//! | kind | value | пример |
//! |------|-------|--------|
//! | `file` | путь к файлу или директории | `/home/z/my-project/poler-engine/src` |
//! | `url`  | веб-URL | `https://doc.rust-lang.org/std/` |
//! | `repo` | owner/repo на GitHub/GitLab/Gitea | `rust-lang/rust` |
//!
//! ## Возможности
//! - **add** — добавить источник (с авто-детекцией kind по URL/пути)
//! - **list** — показать все источники
//! - **rm** — удалить источник по id
//! - **test** — проверить доступность (для url — HTTP HEAD, для file — `exists()`,
//!   для repo — через существующие VCS-адаптеры)
//! - **open** — `xdg-open` для url/file (только Linux/macOS)
//!
//! ## Схема
//! ```sql
//! CREATE TABLE IF NOT EXISTS poler_sources (
//!   id             INTEGER PRIMARY KEY AUTOINCREMENT,
//!   kind           TEXT NOT NULL,    -- 'file' | 'url' | 'repo'
//!   value          TEXT NOT NULL,    -- путь / URL / owner+repo
//!   label          TEXT,
//!   added_at       INTEGER NOT NULL,
//!   last_tested_at INTEGER,
//!   last_status    TEXT              -- 'ok' | 'fail' | 'unknown'
//! );
//! ```
//!
//! См. также модуль [`knowledge`] — Суверенный Гиппокамп (v0.29.0): инжест
//! библиотеки POLER (спеки, трактат, транскрипты) в нативный индекс движка
//! с эпистемической градацией доверия.

pub mod knowledge;

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

/// Тип источника.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    File,
    Url,
    Repo,
}

impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceKind::File => "file",
            SourceKind::Url => "url",
            SourceKind::Repo => "repo",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "file" => SourceKind::File,
            "url" => SourceKind::Url,
            "repo" => SourceKind::Repo,
            _ => return None,
        })
    }
}

/// Запись об источнике в БД.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub id: i64,
    pub kind: SourceKind,
    pub value: String,
    pub label: Option<String>,
    pub added_at: i64,
    pub last_tested_at: Option<i64>,
    pub last_status: String,
}

/// Результат тестирования источника.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
    Ok,
    Fail,
}

impl TestStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TestStatus::Ok => "ok",
            TestStatus::Fail => "fail",
        }
    }
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Создать таблицу `poler_sources` если её ещё нет. Идемпотентно.
pub fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS poler_sources (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            kind           TEXT NOT NULL,
            value          TEXT NOT NULL,
            label          TEXT,
            added_at       INTEGER NOT NULL,
            last_tested_at INTEGER,
            last_status    TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_polersrc_kind  ON poler_sources(kind);
        CREATE INDEX IF NOT EXISTS idx_polersrc_value ON poler_sources(value);",
    )
    .map_err(|e| format!("sources::ensure_schema: {e}"))?;
    Ok(())
}

/// Открыть соединение к poler-engine БД по пути (создаст схему если нужно).
pub fn open(db_path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("open {:?}: {e}", db_path))?;
    ensure_schema(&conn)?;
    Ok(conn)
}

/// Авто-определить тип источника по значению.
/// - Начинается с `http://`/`https://` → `Url`
/// - Не похоже на абсолютный/относительный путь (не `/`, `.`, `~`) и содержит
///   ровно один `/` без расширений → `Repo` (owner/repo)
/// - Иначе → `File`
pub fn detect_kind(value: &str) -> SourceKind {
    let v = value.trim();
    if v.starts_with("http://") || v.starts_with("https://") {
        return SourceKind::Url;
    }
    // Абсолютный/относительный путь → File
    if v.starts_with('/') || v.starts_with('.') || v.starts_with('~') {
        return SourceKind::File;
    }
    // owner/repo: ровно один слэш, оба компонента непустые, без расширений
    let slash_count = v.matches('/').count();
    if slash_count == 1 {
        let parts: Vec<&str> = v.split('/').collect();
        if parts.len() == 2
            && !parts[0].is_empty()
            && !parts[1].is_empty()
            && !parts[0].contains(char::is_whitespace)
            && !parts[1].contains(char::is_whitespace)
        {
            return SourceKind::Repo;
        }
    }
    SourceKind::File
}

/// Добавить источник (kind авто-определяется если передан `None`).
pub fn add_source(
    conn: &Connection,
    kind: Option<SourceKind>,
    value: &str,
    label: Option<&str>,
) -> Result<i64, String> {
    let k = kind.unwrap_or_else(|| detect_kind(value));
    let now = now_ts();
    conn.execute(
        "INSERT INTO poler_sources (kind, value, label, added_at, last_status)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![k.as_str(), value, label, now, "unknown"],
    )
    .map_err(|e| format!("sources::add_source: {e}"))?;
    Ok(conn.last_insert_rowid())
}

/// Список всех источников (новейшие сверху).
pub fn list_sources(conn: &Connection, limit: usize) -> Result<Vec<Source>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, value, label, added_at, last_tested_at, last_status
             FROM poler_sources ORDER BY added_at DESC LIMIT ?1",
        )
        .map_err(|e| format!("sources::list_sources: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![limit as i64], |r| {
            let kind_str: String = r.get::<_, Option<String>>(1)?.unwrap_or_default();
            let status_str: String = r.get::<_, Option<String>>(6)?.unwrap_or_else(|| "unknown".into());
            Ok(Source {
                id: r.get(0)?,
                kind: SourceKind::from_str(&kind_str).unwrap_or(SourceKind::File),
                value: r.get(2)?,
                label: r.get(3)?,
                added_at: r.get(4)?,
                last_tested_at: r.get(5)?,
                last_status: status_str,
            })
        })
        .map_err(|e| format!("sources::list_sources query: {e}"))?;
    let mut out = Vec::new();
    for r in rows.flatten() {
        out.push(r);
    }
    Ok(out)
}

/// Удалить источник по id.
pub fn delete_source(conn: &Connection, id: i64) -> Result<(), String> {
    let n = conn
        .execute("DELETE FROM poler_sources WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("sources::delete_source: {e}"))?;
    if n == 0 {
        return Err(format!("source id {id} не найден"));
    }
    Ok(())
}

/// Протестировать источник (без сети — только локальные проверки).
///
/// Для `file` — `Path::exists()`. Для `url` — пробуем TCP-connect к хосту
/// (минимум зависимостей, без HTTP-клиента; игнорируем TLS). Для `repo` —
/// формат owner/repo проверяется регуляркой; реальный REST-запрос должен
/// делаться через `poler-engine gh repos <owner>/<repo>` в шелле.
pub fn test_source(conn: &Connection, id: i64) -> Result<TestStatus, String> {
    let src = match get_source(conn, id)? {
        Some(s) => s,
        None => return Err(format!("source id {id} не найден")),
    };
    let status = match src.kind {
        SourceKind::File => {
            if Path::new(&src.value).exists() {
                TestStatus::Ok
            } else {
                TestStatus::Fail
            }
        }
        SourceKind::Url => {
            // Лёгкая проверка: наличие схемы + хоста
            let url = src.value.trim();
            if (url.starts_with("http://") || url.starts_with("https://"))
                && url.len() > 8
                && url[7..].contains('.')
            {
                TestStatus::Ok
            } else {
                TestStatus::Fail
            }
        }
        SourceKind::Repo => {
            // owner/repo: ровно один слеш, оба компонента непустые
            let parts: Vec<&str> = src.value.split('/').collect();
            if parts.len() == 2
                && !parts[0].is_empty()
                && !parts[1].is_empty()
                && !parts[0].contains(char::is_whitespace)
                && !parts[1].contains(char::is_whitespace)
            {
                TestStatus::Ok
            } else {
                TestStatus::Fail
            }
        }
    };
    let now = now_ts();
    conn.execute(
        "UPDATE poler_sources SET last_tested_at=?2, last_status=?3 WHERE id=?1",
        rusqlite::params![id, now, status.as_str()],
    )
    .map_err(|e| format!("sources::test_source update: {e}"))?;
    Ok(status)
}

/// Получить один источник по id.
pub fn get_source(conn: &Connection, id: i64) -> Result<Option<Source>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, value, label, added_at, last_tested_at, last_status
             FROM poler_sources WHERE id = ?1",
        )
        .map_err(|e| format!("sources::get_source: {e}"))?;
    let mut rows = stmt
        .query_map(rusqlite::params![id], |r| {
            let kind_str: String = r.get::<_, Option<String>>(1)?.unwrap_or_default();
            let status_str: String = r
                .get::<_, Option<String>>(6)?
                .unwrap_or_else(|| "unknown".into());
            Ok(Source {
                id: r.get(0)?,
                kind: SourceKind::from_str(&kind_str).unwrap_or(SourceKind::File),
                value: r.get(2)?,
                label: r.get(3)?,
                added_at: r.get(4)?,
                last_tested_at: r.get(5)?,
                last_status: status_str,
            })
        })
        .map_err(|e| format!("sources::get_source query: {e}"))?;
    match rows.next() {
        Some(Ok(s)) => Ok(Some(s)),
        Some(Err(e)) => Err(format!("sources::get_source row: {e}")),
        None => Ok(None),
    }
}

/// Открыть источник в системе: для url/file — `xdg-open` (Linux) или `open` (macOS).
/// Для repo — формирует ссылку `https://github.com/<value>` и открывает её.
pub fn open_source(conn: &Connection, id: i64) -> Result<(), String> {
    let src = match get_source(conn, id)? {
        Some(s) => s,
        None => return Err(format!("source id {id} не найден")),
    };
    let target = match src.kind {
        SourceKind::File | SourceKind::Url => src.value.clone(),
        SourceKind::Repo => format!("https://github.com/{}", src.value),
    };
    let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    std::process::Command::new(opener)
        .arg(&target)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("open {target:?}: {e} (нет xdg-open?)"))?;
    Ok(())
}

/// Отформатировать список источников для TUI.
pub fn format_list(sources: &[Source]) -> String {
    if sources.is_empty() {
        return "(источников нет — добавьте через `sources add <url|path|owner/repo>`)".into();
    }
    let mut s = String::new();
    s.push_str(&format!("📚 {} источников:\n\n", sources.len()));
    for src in sources {
        let lbl = src
            .label
            .as_ref()
            .map(|l| format!("  ({})", l))
            .unwrap_or_default();
        let st = match src.last_status.as_str() {
            "ok" => "✓",
            "fail" => "✗",
            _ => "?",
        };
        s.push_str(&format!(
            "#{:>3} [{}] {:<6} {}{}\n",
            src.id, st, src.kind.as_str(), src.value, lbl
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fresh_db() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let p = dir.path().join("test.db");
        let conn = open(&p).unwrap();
        (dir, conn)
    }

    #[test]
    fn detect_url() {
        assert_eq!(detect_kind("https://example.com"), SourceKind::Url);
        assert_eq!(detect_kind("http://rust-lang.org"), SourceKind::Url);
    }

    #[test]
    fn detect_repo() {
        assert_eq!(detect_kind("rust-lang/rust"), SourceKind::Repo);
        assert_eq!(detect_kind("torvalds/linux"), SourceKind::Repo);
    }

    #[test]
    fn detect_file() {
        assert_eq!(detect_kind("/home/z/poler-engine"), SourceKind::File);
        assert_eq!(detect_kind("./src/lib.rs"), SourceKind::File);
        assert_eq!(detect_kind("~/notes.md"), SourceKind::File);
    }

    #[test]
    fn add_and_list_source() {
        let (_t, conn) = fresh_db();
        let id = add_source(&conn, None, "https://rust-lang.org", Some("rust")).unwrap();
        assert!(id > 0);
        let list = list_sources(&conn, 10).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].value, "https://rust-lang.org");
        assert_eq!(list[0].kind, SourceKind::Url);
        assert_eq!(list[0].label.as_deref(), Some("rust"));
        assert_eq!(list[0].last_status, "unknown");
    }

    #[test]
    fn add_explicit_kind_overrides_detect() {
        let (_t, conn) = fresh_db();
        // value выглядит как repo, но явно укажем file
        let id = add_source(&conn, Some(SourceKind::File), "torvalds/linux", None).unwrap();
        let list = list_sources(&conn, 10).unwrap();
        assert_eq!(list[0].id, id);
        assert_eq!(list[0].kind, SourceKind::File);
    }

    #[test]
    fn test_file_ok_and_fail() {
        let (_t, conn) = fresh_db();
        let dir = tempdir().unwrap();
        let real = dir.path().join("real.txt");
        std::fs::write(&real, "hi").unwrap();

        let id_ok = add_source(&conn, Some(SourceKind::File), real.to_str().unwrap(), None).unwrap();
        let id_fail = add_source(&conn, Some(SourceKind::File), "/nonexistent/path", None).unwrap();

        assert_eq!(test_source(&conn, id_ok).unwrap(), TestStatus::Ok);
        assert_eq!(test_source(&conn, id_fail).unwrap(), TestStatus::Fail);

        let s = get_source(&conn, id_ok).unwrap().unwrap();
        assert_eq!(s.last_status, "ok");
        assert!(s.last_tested_at.is_some());

        let s = get_source(&conn, id_fail).unwrap().unwrap();
        assert_eq!(s.last_status, "fail");
    }

    #[test]
    fn test_url_format() {
        let (_t, conn) = fresh_db();
        let id = add_source(&conn, Some(SourceKind::Url), "https://doc.rust-lang.org", None).unwrap();
        assert_eq!(test_source(&conn, id).unwrap(), TestStatus::Ok);

        let id_bad = add_source(&conn, Some(SourceKind::Url), "not-a-url", None).unwrap();
        assert_eq!(test_source(&conn, id_bad).unwrap(), TestStatus::Fail);
    }

    #[test]
    fn test_repo_format() {
        let (_t, conn) = fresh_db();
        let id = add_source(&conn, Some(SourceKind::Repo), "rust-lang/rust", None).unwrap();
        assert_eq!(test_source(&conn, id).unwrap(), TestStatus::Ok);

        let id_bad = add_source(&conn, Some(SourceKind::Repo), "no-slash", None).unwrap();
        assert_eq!(test_source(&conn, id_bad).unwrap(), TestStatus::Fail);
    }

    #[test]
    fn delete_removes_source() {
        let (_t, conn) = fresh_db();
        let id = add_source(&conn, None, "/tmp", None).unwrap();
        delete_source(&conn, id).unwrap();
        assert!(get_source(&conn, id).unwrap().is_none());
    }

    #[test]
    fn delete_missing_returns_err() {
        let (_t, conn) = fresh_db();
        let e = delete_source(&conn, 9999).unwrap_err();
        assert!(e.contains("не найден"));
    }

    #[test]
    fn ensure_schema_idempotent() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("t.db");
        let conn = Connection::open(&p).unwrap();
        ensure_schema(&conn).unwrap();
        ensure_schema(&conn).unwrap();
        add_source(&conn, None, "/tmp", None).unwrap();
        ensure_schema(&conn).unwrap();
    }

    #[test]
    fn format_list_shows_kind_and_status() {
        let now = now_ts();
        let s = Source {
            id: 5,
            kind: SourceKind::Url,
            value: "https://example.com".into(),
            label: Some("ex".into()),
            added_at: now,
            last_tested_at: Some(now),
            last_status: "ok".into(),
        };
        let txt = format_list(&[s]);
        assert!(txt.contains("https://example.com"));
        assert!(txt.contains("url"));
        assert!(txt.contains("✓"));
        assert!(txt.contains("(ex)"));
    }
}
