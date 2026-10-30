//! # Notes CRUD (v0.17.0)
//!
//! Локальная база заметок для poler-shell. Заметки сохраняются в той же
//! SQLite-БД (`web-index.db`) в отдельной таблице `poler_notes`, чтобы
//! не плодить файлы и не терять данные между сессиями.
//!
//! ## Возможности
//! - **Create**: `notes add` из CLI или `Ctrl+N` из TUI (встроенный редактор)
//! - **Read**: `notes list` / `notes show <id>`
//! - **Update**: `notes edit <id>` из TUI
//! - **Delete**: `notes rm <id>`
//!
//! v2.0: источник заметок `ai-reply` (бывший NLM-чат) и колонка notebook_id
//! сохраняются для обратной совместимости существующих БД; новые NLM-записи
//! не создаются (Google/NotebookLM удалены — суверенный стек).
//!
//! ## Схема
//! ```sql
//! CREATE TABLE IF NOT EXISTS poler_notes (
//!   id          INTEGER PRIMARY KEY AUTOINCREMENT,
//!   title       TEXT NOT NULL,
//!   body        TEXT NOT NULL,
//!   tags        TEXT,            -- CSV тегов: "ai-reply,notebook,prologue"
//!   source      TEXT,            -- 'manual' | 'ai-reply' | 'imported'
//!   notebook_id TEXT,            -- nullable, ссылка на NLM notebook UUID
//!   created_at  INTEGER NOT NULL,
//!   updated_at  INTEGER NOT NULL
//! );
//! ```

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

/// Запись о заметке в БД (используется в `list_notes` и `get_note`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub source: String,
    pub notebook_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Источник заметки: где она была создана.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteSource {
    Manual,
    AiReply,
    Imported,
    /// Пришла из NotebookLM при синке (nlm_notes_sync).
    Nlm,
}

impl NoteSource {
    pub fn as_str(self) -> &'static str {
        match self {
            NoteSource::Manual => "manual",
            NoteSource::AiReply => "ai-reply",
            NoteSource::Imported => "imported",
            NoteSource::Nlm => "nlm",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "ai-reply" => NoteSource::AiReply,
            "imported" => NoteSource::Imported,
            "nlm" => NoteSource::Nlm,
            _ => NoteSource::Manual,
        }
    }
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn split_tags(s: &str) -> Vec<String> {
    s.split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

fn join_tags(tags: &[String]) -> String {
    tags.join(",")
}

/// Создать таблицу `poler_notes` если её ещё нет. Идемпотентно.
pub fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS poler_notes (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            title       TEXT NOT NULL,
            body        TEXT NOT NULL,
            tags        TEXT,
            source      TEXT,
            notebook_id TEXT,
            created_at  INTEGER NOT NULL,
            updated_at  INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_polernote_tags ON poler_notes(tags);
        CREATE INDEX IF NOT EXISTS idx_polernote_source ON poler_notes(source);
        CREATE INDEX IF NOT EXISTS idx_polernote_nb ON poler_notes(notebook_id);",
    )
    .map_err(|e| format!("notes::ensure_schema: {e}"))?;
    Ok(())
}

/// Открыть соединение к poler-engine БД по пути (создаст схему если нужно).
pub fn open(db_path: &std::path::Path) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("open {:?}: {e}", db_path))?;
    ensure_schema(&conn)?;
    Ok(conn)
}

/// Создать новую заметку. Возвращает её id.
pub fn add_note(
    conn: &Connection,
    title: &str,
    body: &str,
    tags: &[String],
    source: NoteSource,
    notebook_id: Option<&str>,
) -> Result<i64, String> {
    let now = now_ts();
    conn.execute(
        "INSERT INTO poler_notes (title, body, tags, source, notebook_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        rusqlite::params![
            title,
            body,
            join_tags(tags),
            source.as_str(),
            notebook_id,
            now,
        ],
    )
    .map_err(|e| format!("notes::add_note: {e}"))?;
    Ok(conn.last_insert_rowid())
}

/// Получить все заметки (новейшие сверху).
pub fn list_notes(conn: &Connection, limit: usize) -> Result<Vec<Note>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, body, tags, source, notebook_id, created_at, updated_at
             FROM poler_notes ORDER BY updated_at DESC LIMIT ?1",
        )
        .map_err(|e| format!("notes::list_notes: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![limit as i64], |r| {
            let tags_str: String = r.get::<_, Option<String>>(3)?.unwrap_or_default();
            Ok(Note {
                id: r.get(0)?,
                title: r.get(1)?,
                body: r.get(2)?,
                tags: split_tags(&tags_str),
                source: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                notebook_id: r.get(5)?,
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
            })
        })
        .map_err(|e| format!("notes::list_notes query: {e}"))?;
    let mut out = Vec::new();
    for r in rows.flatten() {
        out.push(r);
    }
    Ok(out)
}

/// Получить одну заметку по id.
pub fn get_note(conn: &Connection, id: i64) -> Result<Option<Note>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, body, tags, source, notebook_id, created_at, updated_at
             FROM poler_notes WHERE id = ?1",
        )
        .map_err(|e| format!("notes::get_note: {e}"))?;
    let mut rows = stmt
        .query_map(rusqlite::params![id], |r| {
            let tags_str: String = r.get::<_, Option<String>>(3)?.unwrap_or_default();
            Ok(Note {
                id: r.get(0)?,
                title: r.get(1)?,
                body: r.get(2)?,
                tags: split_tags(&tags_str),
                source: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                notebook_id: r.get(5)?,
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
            })
        })
        .map_err(|e| format!("notes::get_note query: {e}"))?;
    match rows.next() {
        Some(Ok(n)) => Ok(Some(n)),
        Some(Err(e)) => Err(format!("notes::get_note row: {e}")),
        None => Ok(None),
    }
}

/// Обновить заголовок и тело заметки (тэги можно отдельно через `set_tags`).
pub fn update_note(
    conn: &Connection,
    id: i64,
    title: &str,
    body: &str,
    tags: &[String],
) -> Result<(), String> {
    let now = now_ts();
    let n = conn
        .execute(
            "UPDATE poler_notes SET title=?2, body=?3, tags=?4, updated_at=?5 WHERE id=?1",
            rusqlite::params![id, title, body, join_tags(tags), now],
        )
        .map_err(|e| format!("notes::update_note: {e}"))?;
    if n == 0 {
        return Err(format!("note id {id} не найден"));
    }
    Ok(())
}

/// Удалить заметку по id.
pub fn delete_note(conn: &Connection, id: i64) -> Result<(), String> {
    let n = conn
        .execute("DELETE FROM poler_notes WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("notes::delete_note: {e}"))?;
    if n == 0 {
        return Err(format!("note id {id} не найден"));
    }
    Ok(())
}

/// Отформатировать список заметок в многострочный текст для TUI.
pub fn format_list(notes: &[Note]) -> String {
    if notes.is_empty() {
        return "(заметок нет — нажмите Ctrl+N чтобы создать)".into();
    }
    let mut s = String::new();
    s.push_str(&format!("📝 {} заметок:\n\n", notes.len()));
    for n in notes {
        let tags = if n.tags.is_empty() {
            String::new()
        } else {
            format!("  [{}]", n.tags.join(","))
        };
        let nb = n
            .notebook_id
            .as_ref()
            .map(|u| format!("  nb:{}", &u[..u.len().min(8)]))
            .unwrap_or_default();
        s.push_str(&format!(
            "#{}  {}{}{}\n      {}\n\n",
            n.id,
            n.title,
            tags,
            nb,
            // Первая строка тела как превью
            n.body.lines().next().unwrap_or("(пусто)")
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
    fn add_and_get_note_roundtrip() {
        let (_t, conn) = fresh_db();
        let id = add_note(
            &conn,
            "Test",
            "Body text",
            &["unit".into(), "demo".into()],
            NoteSource::Manual,
            None,
        )
        .unwrap();
        let n = get_note(&conn, id).unwrap().expect("must exist");
        assert_eq!(n.title, "Test");
        assert_eq!(n.body, "Body text");
        assert_eq!(n.tags, vec!["unit".to_string(), "demo".to_string()]);
        assert_eq!(n.source, "manual");
        assert_eq!(n.notebook_id, None);
        assert!(n.created_at > 0);
        assert!(n.updated_at >= n.created_at);
    }

    #[test]
    fn list_returns_newest_first() {
        let (_t, conn) = fresh_db();
        let id_a = add_note(&conn, "A", "a", &[], NoteSource::Manual, None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let id_b = add_note(&conn, "B", "b", &[], NoteSource::Manual, None).unwrap();
        let list = list_notes(&conn, 10).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, id_b, "newest first");
        assert_eq!(list[1].id, id_a);
    }

    #[test]
    fn update_changes_body_and_tags() {
        let (_t, conn) = fresh_db();
        let id = add_note(&conn, "Old", "old body", &[], NoteSource::Manual, None).unwrap();
        update_note(&conn, id, "New", "new body", &["x".into()]).unwrap();
        let n = get_note(&conn, id).unwrap().unwrap();
        assert_eq!(n.title, "New");
        assert_eq!(n.body, "new body");
        assert_eq!(n.tags, vec!["x".to_string()]);
    }

    #[test]
    fn delete_removes_note() {
        let (_t, conn) = fresh_db();
        let id = add_note(&conn, "Tmp", "tmp", &[], NoteSource::Manual, None).unwrap();
        delete_note(&conn, id).unwrap();
        assert!(get_note(&conn, id).unwrap().is_none());
    }

    #[test]
    fn delete_missing_returns_err() {
        let (_t, conn) = fresh_db();
        let e = delete_note(&conn, 9999).unwrap_err();
        assert!(e.contains("не найден"));
    }

    #[test]
    fn ai_reply_source_round_trips() {
        let (_t, conn) = fresh_db();
        let id = add_note(
            &conn,
            "AI reply",
            "answer",
            &["prologue".into()],
            NoteSource::AiReply,
            Some("704f2610-abcd"),
        )
        .unwrap();
        let n = get_note(&conn, id).unwrap().unwrap();
        assert_eq!(n.source, "ai-reply");
        assert_eq!(n.notebook_id.as_deref(), Some("704f2610-abcd"));
    }

    #[test]
    fn format_list_handles_empty() {
        let s = format_list(&[]);
        assert!(s.contains("заметок нет"));
    }

    #[test]
    fn format_list_shows_id_and_title() {
        let now = now_ts();
        let n = Note {
            id: 7,
            title: "Demo".into(),
            body: "first line\nsecond".into(),
            tags: vec!["x".into()],
            source: "manual".into(),
            notebook_id: Some("704f2610".into()),
            created_at: now,
            updated_at: now,
        };
        let s = format_list(&[n]);
        assert!(s.contains("#7"));
        assert!(s.contains("Demo"));
        assert!(s.contains("[x]"));
        assert!(s.contains("first line"));
    }

    #[test]
    fn ensure_schema_is_idempotent() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("t.db");
        let conn = Connection::open(&p).unwrap();
        ensure_schema(&conn).unwrap();
        ensure_schema(&conn).unwrap();
        ensure_schema(&conn).unwrap();
        // даже с данными
        add_note(&conn, "x", "y", &[], NoteSource::Manual, None).unwrap();
        ensure_schema(&conn).unwrap();
    }
}
