//! Disk-backed таблица символов AIDDE на SQLite.
//!
//! Лечение OOM-границы: ин-мемори `SymbolTable` на 65K файлах Linux
//! Kernel (~5 млн вызовов) требует 5–8 ГБ RAM. SQLite-хранилище
//! держит те же данные на диске с B-Tree индексами по callee/caller:
//! RAM при построении — O(пачка), при impact-анализе — O(уровень BFS).
//!
//! Схема (стиль LSP-серверов ccls/rust-analyzer):
//!
//! ```sql
//! CREATE TABLE defs    (symbol TEXT, kind TEXT, file TEXT, line INT, byte INT);
//! CREATE TABLE calls   (caller TEXT, callee TEXT, file TEXT, line INT);
//! CREATE INDEX idx_calls_callee ON calls(callee);  -- upstream BFS
//! CREATE INDEX idx_calls_caller ON calls(caller);  -- downstream BFS
//! CREATE INDEX idx_defs_symbol   ON defs(symbol);  -- resolve
//! ```

use rayon::prelude::*;
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::aidde::symbols::{scan_one_file, Definition};

/// Дисковое хранилище символов и вызовов.
pub struct SymbolStore {
    conn: Connection,
}

impl SymbolStore {
    /// Открывает базу без сброса (reuse-режим): если таблицы уже
    /// заполнены — возвращает Ok(false) и построение пропускается.
    pub fn open_existing(db_path: &Path) -> rusqlite::Result<(Self, bool)> {
        let conn = Connection::open(db_path)?;
        let calls: i64 = conn
            .query_row("SELECT COUNT(*) FROM calls", [], |r| r.get(0))
            .unwrap_or(0);
        let has_schema = calls > 0;
        Ok((Self { conn }, has_schema))
    }

    /// Открывает (создаёт) базу и схему. `:memory:` — ин-мемори режим
    /// (совместимость с тестами).
    pub fn open(db_path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = OFF;
             PRAGMA synchronous = OFF;
             PRAGMA cache_size = -64000; -- 64 МБ кэша страниц
             CREATE TABLE IF NOT EXISTS files (
                 id   INTEGER PRIMARY KEY,
                 path TEXT NOT NULL UNIQUE
             );
             CREATE TABLE IF NOT EXISTS defs (
                 symbol  TEXT NOT NULL,
                 kind    TEXT NOT NULL,
                 file_id INTEGER NOT NULL,
                 line    INTEGER NOT NULL,
                 byte    INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS calls (
                 caller  TEXT NOT NULL,
                 callee  TEXT NOT NULL,
                 file_id INTEGER NOT NULL,
                 line    INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_defs_symbol ON defs(symbol);
             CREATE INDEX IF NOT EXISTS idx_calls_callee ON calls(callee);
             CREATE INDEX IF NOT EXISTS idx_calls_caller ON calls(caller);
             DELETE FROM defs;
             DELETE FROM calls;
             DELETE FROM files;",
        )?;
        Ok(Self { conn })
    }

    /// Параллельное построение с потоковой записью: каждый rayon-воркер
    /// сканирует файлы чанками и сразу сливает результаты в SQLite под
    /// мьютексом — пик памяти ограничен размером чанка, а не корпусом
    /// (bug: collect() до записи раздувал RSS до гигабайт на 65K файлов).
    pub fn build(&mut self, files: &[PathBuf], max_file_bytes: u64) -> rusqlite::Result<()> {
        use std::sync::Mutex;

        const CHUNK: usize = 64; // файлов на чанк воркера
        let conn = Mutex::new(&mut self.conn);

        files
            .par_chunks(CHUNK)
            .map(|chunk| -> rusqlite::Result<()> {
                let mut local_defs: Vec<Definition> = Vec::new();
                let mut local_calls: Vec<(String, String, String, usize)> = Vec::new();
                for p in chunk {
                    let (defs, calls, _imports) = scan_one_file(p, max_file_bytes);
                    local_defs.extend(defs);
                    local_calls.extend(
                        calls
                            .into_iter()
                            .map(|c| (c.caller, c.callee, c.file, c.line)),
                    );
                }
                // Слить чанк в базу под мьютексом (короткая транзакция).
                // Пути нормализуются: files(id, path) — путь пишется один
                // раз, вызовы ссылаются по id (сжатие базы в ~5 раз).
                {
                    let conn = conn.lock().unwrap();
                    conn.execute("BEGIN", [])?;
                    // id путей для этого чанка
                    let mut file_ids: std::collections::HashMap<String, i64> =
                        std::collections::HashMap::new();
                    {
                        let mut ins = conn
                            .prepare("INSERT OR IGNORE INTO files (path) VALUES (?1)")?;
                        let mut sel =
                            conn.prepare("SELECT id FROM files WHERE path = ?1")?;
                        let mut paths: HashSet<&str> = HashSet::new();
                        for d in &local_defs {
                            paths.insert(d.file.as_str());
                        }
                        for (_, _, f, _) in &local_calls {
                            paths.insert(f.as_str());
                        }
                        for p in paths {
                            ins.execute(params![p])?;
                            let id: i64 = sel.query_row(params![p], |r| r.get(0))?;
                            file_ids.insert(p.to_string(), id);
                        }
                    }
                    {
                        let mut st = conn.prepare(
                            "INSERT INTO defs (symbol, kind, file_id, line, byte) VALUES (?1, ?2, ?3, ?4, ?5)",
                        )?;
                        for d in &local_defs {
                            let fid = file_ids.get(&d.file).copied().unwrap_or(0);
                            st.execute(params![
                                d.symbol,
                                d.kind,
                                fid,
                                d.line as i64,
                                d.byte as i64
                            ])?;
                        }
                    }
                    {
                        let mut st = conn.prepare(
                            "INSERT INTO calls (caller, callee, file_id, line) VALUES (?1, ?2, ?3, ?4)",
                        )?;
                        for (caller, callee, file, line) in &local_calls {
                            let fid = file_ids.get(file).copied().unwrap_or(0);
                            st.execute(params![caller, callee, fid, *line as i64])?;
                        }
                    }
                    conn.execute("COMMIT", [])?;
                }
                Ok(())
            })
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(())
    }

    /// Разрешение символа (аналог SymbolTable::resolve).
    pub fn resolve(&self, name: &str) -> Vec<Definition> {
        let key = last_seg(name);
        let mut stmt = match self
            .conn
            .prepare("SELECT d.symbol, d.kind, f.path, d.line, d.byte FROM defs d JOIN files f ON f.id = d.file_id WHERE d.symbol = ?1")
        {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map(params![key], |r| {
            Ok(Definition {
                symbol: r.get(0)?,
                kind: r.get(1)?,
                file: r.get(2)?,
                line: r.get::<_, i64>(3)? as usize,
                byte: r.get::<_, i64>(4)? as usize,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Вызовы по callee (для upstream BFS).
    pub fn calls_of_callee(&self, callee: &str) -> Vec<(String, String, usize)> {
        let mut stmt = match self
            .conn
            .prepare("SELECT c.caller, f.path, c.line FROM calls c JOIN files f ON f.id = c.file_id WHERE c.callee = ?1")
        {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map(params![callee], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)? as usize))
        });
        match rows {
            Ok(iter) => iter.filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Вызовы по caller (для downstream BFS).
    pub fn calls_of_caller(&self, caller: &str) -> Vec<(String, String, String)> {
        let mut stmt = match self
            .conn
            .prepare("SELECT c.callee, c.callee, f.path FROM calls c JOIN files f ON f.id = c.file_id WHERE c.caller = ?1")
        {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map(params![caller], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        });
        match rows {
            Ok(iter) => iter.filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Есть ли вообще вызовы с таким callee (для extern/macro fallback).
    pub fn has_calls_of_callee(&self, callee: &str) -> bool {
        !self.calls_of_callee(callee).is_empty()
    }

    pub fn stats(&self) -> (usize, usize) {
        let defs: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM defs", [], |r| r.get(0))
            .unwrap_or(0);
        let calls: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM calls", [], |r| r.get(0))
            .unwrap_or(0);
        (defs as usize, calls as usize)
    }
}

pub fn last_seg(s: &str) -> &str {
    s.rsplit("::")
        .next()
        .unwrap_or(s)
        .rsplit('.')
        .next()
        .unwrap_or(s)
}

/// Impact-анализ поверх SQLite-хранилища: BFS читает только нужные
/// строки по индексам, память ограничена текущим уровнем.
pub fn impact_analysis_sqlite(
    store: &SymbolStore,
    target: &str,
    depth: usize,
    max_items: usize,
) -> Option<crate::aidde::ImpactReport> {
    use crate::aidde::{Dependency, Dependent, ImpactReport};

    let external_def;
    let def = match store.resolve(target).first() {
        Some(d) => d.clone(),
        None => {
            if !store.has_calls_of_callee(target) {
                return None;
            }
            external_def = Definition {
                symbol: target.to_string(),
                kind: "extern/macro".to_string(),
                file: String::new(),
                line: 0,
                byte: 0,
            };
            external_def
        }
    };

    // Тело определения (triage-сигналы) — только для локальных defs.
    let (lines, triage_alerts) = if def.file.is_empty() {
        ("extern".to_string(), Vec::new())
    } else {
        let text = std::fs::read_to_string(&def.file).ok()?;
        let lang = crate::parser::detect_lang(Path::new(&def.file));
        let scope = crate::parser::extract_enclosing_scope(&text, def.byte, lang);
        (
            format!("{}-{}", scope.start_line, scope.end_line),
            crate::aidde::impact::triage_scan(&scope.text),
        )
    };

    // ---------- Upstream BFS по индексу idx_calls_callee ----------
    let mut upstream: Vec<Dependent> = Vec::new();
    let mut seen_calls: HashSet<(String, String, usize)> = HashSet::new();
    let mut level: HashSet<String> = HashSet::from([last_seg(target).to_string()]);
    let mut visited: HashSet<String> = level.clone();

    for _ in 0..depth.max(1) {
        let mut next: HashSet<String> = HashSet::new();
        for callee in &level {
            for (caller, file, line) in store.calls_of_callee(callee) {
                let key = (caller.clone(), file.clone(), line);
                if seen_calls.insert(key) {
                    upstream.push(Dependent {
                        caller: caller.clone(),
                        file: file.clone(),
                        line,
                    });
                }
                next.insert(last_seg(&caller).to_string());
            }
        }
        next.retain(|n| !visited.contains(n));
        if next.is_empty() || upstream.len() >= max_items {
            break;
        }
        visited.extend(next.iter().cloned());
        level = next;
    }
    upstream.truncate(max_items);

    // ---------- Downstream BFS по индексу idx_calls_caller ----------
    let mut downstream: Vec<Dependency> = Vec::new();
    let mut seen_dep: HashSet<String> = HashSet::new();
    let mut level: HashSet<String> = HashSet::from([last_seg(target).to_string()]);
    let mut visited: HashSet<String> = level.clone();

    for _ in 0..depth.max(1) {
        let mut next: HashSet<String> = HashSet::new();
        for caller in &level {
            for (callee, _dup, file) in store.calls_of_caller(caller) {
                if seen_dep.insert(callee.clone()) {
                    let dfile = store
                        .resolve(&callee)
                        .first()
                        .map(|d| d.file.clone())
                        .unwrap_or(file);
                    downstream.push(Dependency {
                        callee: callee.clone(),
                        file: dfile,
                    });
                }
                next.insert(last_seg(&callee).to_string());
            }
        }
        next.retain(|n| !visited.contains(n));
        if next.is_empty() || downstream.len() >= max_items {
            break;
        }
        visited.extend(next.iter().cloned());
        level = next;
    }
    downstream.truncate(max_items);

    // ---------- Danger level ----------
    let files: HashSet<&String> = upstream.iter().map(|d| &d.file).collect();
    let n = files.len();
    let danger = match n {
        0 => "LOW (прямых зависимых не найдено)".to_string(),
        1..=3 => format!("MEDIUM (затронет {n} файл)"),
        4..=10 => format!("HIGH (затронет {n} файлов)"),
        _ => format!("CRITICAL (затронет {n} файлов)"),
    };

    Some(ImpactReport {
        target_function: format!(
            "{}::{}",
            Path::new(&def.file)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            def.symbol
        ),
        file: def.file.clone(),
        lines,
        structural_relations: crate::aidde::impact::StructuralRelations {
            upstream_dependents: upstream,
            downstream_dependencies: downstream,
        },
        heuristic_triage_alerts: triage_alerts,
        danger_level_if_modified: danger,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_store_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("a.rs"),
            "pub fn core_fn(x: i32) -> i32 {\n    x + 1\n}\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("b.rs"), "pub fn mid() {\n    core_fn(1);\n}\n").unwrap();
        let files = vec![dir.path().join("a.rs"), dir.path().join("b.rs")];

        let db = dir.path().join("symbols.db");
        let mut store = SymbolStore::open(&db).unwrap();
        store.build(&files, 1024 * 1024).unwrap();

        let (ndefs, ncalls) = store.stats();
        assert!(ndefs >= 2, "defs={ndefs}");
        assert!(ncalls >= 1, "calls={ncalls}");
        assert_eq!(store.resolve("core_fn").len(), 1);
        let callers = store.calls_of_callee("core_fn");
        assert!(callers.iter().any(|(c, _, _)| c == "b::mid"));

        let report =
            impact_analysis_sqlite(&store, "core_fn", 2, 100).expect("impact через sqlite");
        assert!(report
            .structural_relations
            .upstream_dependents
            .iter()
            .any(|d| d.caller == "b::mid"));
    }

    #[test]
    fn sqlite_extern_macro_fallback() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("u.c"),
            "void user(void) {\n    printk(\"x\");\n}\n",
        )
        .unwrap();
        let files = vec![dir.path().join("u.c")];
        let db = dir.path().join("m.db");
        let mut store = SymbolStore::open(&db).unwrap();
        store.build(&files, 1024 * 1024).unwrap();
        // printk не определён — fallback по вызовам
        let report = impact_analysis_sqlite(&store, "printk", 2, 100);
        assert!(report.is_some(), "extern/macro fallback не сработал");
    }
}
