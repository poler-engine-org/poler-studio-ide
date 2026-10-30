//! Персистентный веб-индекс: «Bigtable» poler-engine на SQLite.
//!
//! Украденные технологии в одном модуле:
//! * инвертированный индекс (Tantivy/Lucene): словарь термов → postings
//!   `(term, page_id, tf, title_tf, positions)` с delta-varint-позициями
//!   (Lucene .prx) — запрос — scatter по термам → gather в аккумуляторы
//!   → Top-K; фразы «"..."» проверяются по смежности позиций;
//! * BM25 (Robertson–Spärck Jones / Okapi) — tf×idf-ранжирование;
//! * PageRank (Page & Brin 1998): итерации по таблице links, d=0.85;
//! * Percolator-lite: повторный обход индексирует только изменившиеся
//!   страницы (сравнение content_hash);
//! * POLER WebRank v1 — гибрид: 0.55·BM25 + 0.15·PageRank + 0.20·заголовок
//!   + 0.10·ε-плотность (hits/doclen — информационная плотность совпадений).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection};

use super::extract::{clean_text, snippet_for, title_from_text};
use super::phrase::{decode_positions, encode_positions, parse_query, phrase_occurrences};
use super::stem::tokenize_stem;
use super::simhash::{near_duplicate, simhash};
use crate::compression::DocStoreCodec;
use crate::retrieval::semantic_bridge::{QueryExpansion, SemanticBridge};

/// Веса POLER WebRank v1.
pub const W_BM25: f64 = 0.55;
pub const W_PAGERANK: f64 = 0.15;
pub const W_TITLE: f64 = 0.20;
pub const W_DENSITY: f64 = 0.10;

/// BM25-константы (Okapi).
const K1: f64 = 1.2;
const B: f64 = 0.75;
/// Дампинговый фактор PageRank.
const DAMPING: f64 = 0.85;
/// Proximity-бонус за фразовое вхождение: к BM25 добавляется
/// `PHRASE_BONUS · Σidf(термов фразы) · min(occ, 8)` — фразовое совпадение
/// весомее разрозненных слов (как phraseFreq в Lucene PhraseScorer).
const PHRASE_BONUS: f64 = 0.5;
/// Разрыв между концом тела и началом заголовка в позиционном потоке:
/// фраза не должна сшивать последнее слово текста с первым словом title.
const TITLE_GAP: u32 = 8;

/// Документ на запись в индекс.
pub struct WebDoc {
    pub url: String,
    pub title: String,
    pub lang: String,
    pub meta_description: String,
    pub text: String,
    pub links: Vec<String>,
    pub content_hash: String,
}

/// Попадание поиска.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WebHit {
    pub url: String,
    pub title: String,
    pub lang: String,
    pub score: f64,
    pub bm25: f64,
    pub pagerank: f64,
    pub title_frac: f64,
    pub density: f64,
    /// Сколько вхождений фраз запроса (в кавычках) найдено по смежности.
    pub phrase_occ: usize,
    pub snippet: String,
    pub doclen: usize,
    pub fetched_at: i64,
}

/// Статистика индекса.
#[derive(Debug, serde::Serialize)]
pub struct IndexStats {
    pub pages: u64,
    pub terms: u64,
    pub postings: u64,
    pub links: u64,
    pub duplicates: u64,
    pub db_bytes: u64,
}

fn zstd_err(e: std::io::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
}

/// Усечение строки по границе символа (для кольца обучения словаря).
fn truncate_char_safe_str(s: &mut String, max_bytes: usize) {
    if s.len() <= max_bytes {
        return;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
}

pub struct WebIndex {
    conn: Connection,
    /// v2.0 (Приоритет 3.2): кодек doc store — pages.text хранится
    /// zstd-BLOB'ом (со словарём после обучения), старые строки читаются
    /// как плоский текст (ленивая миграция).
    docstore: DocStoreCodec,
    /// Кольцо свежих текстов для обучения zstd-словаря (ограничено).
    train_ring: Vec<String>,
}

impl WebIndex {
    /// Доступ к соединению (только чтение) — для инспекции статистик,
    /// прямых запросов `pages`/`terms`/`links` в тестах и клиентском коде.
    /// Запись — только через методы `upsert_page`/`recompute_pagerank`/
    /// `record_duplicate` (транзакционная целостность).
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
    /// Открытие (с созданием схемы) базы веб-индекса.
    pub fn open(db_path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = db_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS pages(
               id INTEGER PRIMARY KEY,
               url TEXT UNIQUE NOT NULL,
               title TEXT NOT NULL DEFAULT '',
               lang TEXT NOT NULL DEFAULT '',
               meta_desc TEXT NOT NULL DEFAULT '',
               text TEXT NOT NULL DEFAULT '',
               content_hash TEXT NOT NULL,
               simhash INTEGER NOT NULL,
               doclen INTEGER NOT NULL,
               rank REAL NOT NULL DEFAULT 1.0,
               dup_of TEXT NOT NULL DEFAULT '',
               fetched_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS terms(
               term TEXT NOT NULL,
               page_id INTEGER NOT NULL,
               tf INTEGER NOT NULL,
               title_tf INTEGER NOT NULL,
               positions BLOB,
               PRIMARY KEY(term, page_id)
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS links(
               src INTEGER NOT NULL,
               dst TEXT NOT NULL,
               PRIMARY KEY(src, dst)
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS hosts(
               host TEXT PRIMARY KEY,
               robots TEXT NOT NULL DEFAULT '',
               robots_at INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS meta(k TEXT PRIMARY KEY, v TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS idx_terms_page ON terms(page_id);
             CREATE INDEX IF NOT EXISTS idx_links_dst ON links(dst);",
        )?;
        Self::with_conn(conn)
    }

    /// In-memory база (тесты).
    pub fn open_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS pages(
               id INTEGER PRIMARY KEY, url TEXT UNIQUE NOT NULL,
               title TEXT NOT NULL DEFAULT '', lang TEXT NOT NULL DEFAULT '',
               meta_desc TEXT NOT NULL DEFAULT '', text TEXT NOT NULL DEFAULT '',
               content_hash TEXT NOT NULL, simhash INTEGER NOT NULL,
               doclen INTEGER NOT NULL, rank REAL NOT NULL DEFAULT 1.0,
               dup_of TEXT NOT NULL DEFAULT '', fetched_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS terms(
               term TEXT NOT NULL, page_id INTEGER NOT NULL,
               tf INTEGER NOT NULL, title_tf INTEGER NOT NULL,
               positions BLOB,
               PRIMARY KEY(term, page_id)) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS links(
               src INTEGER NOT NULL, dst TEXT NOT NULL, PRIMARY KEY(src, dst)) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS hosts(
               host TEXT PRIMARY KEY, robots TEXT NOT NULL DEFAULT '',
               robots_at INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS meta(k TEXT PRIMARY KEY, v TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS idx_terms_page ON terms(page_id);
             CREATE INDEX IF NOT EXISTS idx_links_dst ON links(dst);",
        )?;
        Self::with_conn(conn)
    }

    /// Общий конструктор: кодек doc store + миграции (позиции v0.11.0,
    /// затем zstd-колонка/словарь v2.0).
    fn with_conn(conn: Connection) -> rusqlite::Result<Self> {
        let codec = DocStoreCodec::new()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let mut ix = Self {
            conn,
            docstore: codec,
            train_ring: Vec::new(),
        };
        // Порядок важен: docstore-миграция добавляет колонку text_c,
        // которую читает позиционная миграция (старые строки — плоский
        // текст, новые — zstd-BLOB).
        ix.migrate_docstore_v1()?;
        ix.migrate_positions_v1()?;
        Ok(ix)
    }

    // ------------------------------------------------------------------
    // Миграция v2: doc store на zstd (v2.0, Приоритет 3.2)
    // ------------------------------------------------------------------

    /// Колонка `text_c` + словарь из meta (либо обучение по существующим
    /// страницам старой БД). Ленивая миграция: строки до v2.0 остаются в
    /// `text` и читаются как плоский текст.
    fn migrate_docstore_v1(&mut self) -> rusqlite::Result<()> {
        let has_text_c: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('pages') WHERE name = 'text_c'",
            [],
            |r| r.get(0),
        )?;
        if has_text_c == 0 {
            self.conn
                .execute_batch("ALTER TABLE pages ADD COLUMN text_c BLOB")?;
        }
        let dict: Option<Vec<u8>> = match self.conn.query_row(
            "SELECT v FROM meta WHERE k = 'zstd_dict'",
            [],
            |r| r.get::<_, Vec<u8>>(0),
        ) {
            Ok(d) => Some(d),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e),
        };
        if let Some(d) = dict {
            if !d.is_empty() {
                self.docstore.install_dict(d).map_err(zstd_err)?;
                return Ok(());
            }
        }
        // Старая БД без словаря, страниц достаточно — обучаем при открытии.
        let pages: i64 = self.conn.query_row("SELECT COUNT(*) FROM pages", [], |r| r.get(0))?;
        if pages >= 64 {
            self.train_docstore_dict()?;
        }
        Ok(())
    }

    /// Обучение zstd-словаря по свежим текстам (кольцо апсертов либо
    /// существующие страницы). Словарь иммутабелен после создания и
    /// хранится в `meta['zstd_dict']`.
    fn train_docstore_dict(&mut self) -> rusqlite::Result<()> {
        if self.docstore.has_dict() {
            return Ok(());
        }
        let ring: Vec<String> = std::mem::take(&mut self.train_ring);
        let texts: Vec<String> = if ring.len() >= crate::compression::zstd_dict::DICT_TRAIN_MIN_PAGES
        {
            ring
        } else {
            // Читаем свежие страницы (старые строки — плоский текст,
            // новые — разжимаем; цикл обучения видит обе формы).
            let mut out: Vec<String> = Vec::new();
            let mut stmt = self
                .conn
                .prepare("SELECT text, text_c FROM pages ORDER BY id DESC LIMIT 256")?;
            let rows = stmt.query_map([], |r| {
                let text: String = r.get(0)?;
                let text_c: Option<Vec<u8>> = r.get(1)?;
                match text_c {
                    Some(blob) if !blob.is_empty() => self
                        .docstore
                        .decompress(&blob)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e))),
                    _ => Ok(text),
                }
            })?;
            for row in rows {
                out.push(row?);
                if out.len() >= 256 {
                    break;
                }
            }
            out
        };
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        if let Some(dict) = DocStoreCodec::train_dict_from_texts(&refs) {
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(k, v) VALUES('zstd_dict', ?1)",
                params![dict],
            )?;
            self.docstore.install_dict(dict).map_err(zstd_err)?;
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Миграция v1 → v2: позиционный индекс (v0.11.0)
    // ------------------------------------------------------------------

    /// Старые БД (v0.9/v0.10) не имеют колонки `terms.positions`.
    /// Миграция: ALTER TABLE + пересчёт позиций из сохранённого text/title
    /// тем же токенизатором, что и upsert. content_hash не трогаем —
    /// Percolator-lite продолжит пропускать неизменившиеся страницы.
    /// Нюанс: у страниц с пустым исходным title в pages лежит title,
    /// выведенный из text (title_from_text) — его позиции попадут в
    /// title-зону потока и могут продублировать вхождения фразы
    /// (без ложных срабатываний: выведенный title — префикс текста).
    fn migrate_positions_v1(&self) -> rusqlite::Result<()> {
        let has_positions: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('terms') WHERE name = 'positions'",
            [],
            |r| r.get(0),
        )?;
        if has_positions > 0 {
            return Ok(()); // уже v2
        }
        self.conn
            .execute_batch("ALTER TABLE terms ADD COLUMN positions BLOB")?;
        let pages: Vec<(i64, String, String)> = {
            let mut stmt = self.conn.prepare("SELECT id, text, text_c, title FROM pages")?;
            let rows = stmt.query_map([], |r| {
                // v2.0: свежие строки держат текст в text_c (zstd),
                // старые — в text; миграция позиций видит обе формы.
                let plain: String = r.get(1)?;
                let blob: Option<Vec<u8>> = r.get(2)?;
                let text = match blob {
                    Some(b) if !b.is_empty() => self
                        .docstore
                        .decompress(&b)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                    _ => plain,
                };
                Ok((r.get::<_, i64>(0)?, text, r.get::<_, String>(3)?))
            })?;
            rows.collect::<Result<Vec<_>, rusqlite::Error>>()?
        };
        let tx = self.conn.unchecked_transaction()?;
        {
            // v2.0 Foundation (2.3): ПОЛНЫЙ переиндекс термов страницы, а не
            // только позиции. Snowball-стемминг латиницы меняет термы
            // («runtime» → «runtim»): старый UPDATE-подход оставлял строки
            // с не-стемами без позиций, а новые стемы не вставлял — фразовый
            // поиск по старым БД молча терял страницы. Путь идентичен upsert:
            // те же tokenize_stem → position_map → tf/title_tf.
            let mut del =
                tx.prepare("DELETE FROM terms WHERE page_id = ?1")?;
            let mut ins = tx.prepare(
                "INSERT INTO terms(term, page_id, tf, title_tf, positions)
                 VALUES(?1, ?2, ?3, ?4, ?5)",
            )?;
            for (id, text, title) in pages {
                let body = tokenize_stem(&text);
                let ttl = tokenize_stem(&title);
                let positions = position_map(&body, &ttl);
                let mut tf: HashMap<&str, i64> = HashMap::new();
                for t in &body {
                    *tf.entry(t.as_str()).or_insert(0) += 1;
                }
                let mut title_tf: HashMap<&str, i64> = HashMap::new();
                for t in &ttl {
                    *title_tf.entry(t.as_str()).or_insert(0) += 1;
                }
                del.execute(params![id])?;
                for (term, tf_v) in &tf {
                    let ttf_v = title_tf.get(term).copied().unwrap_or(0);
                    let blob = positions
                        .get(*term)
                        .map(|p| encode_positions(p))
                        .unwrap_or_default();
                    ins.execute(params![term, id, tf_v, ttf_v, blob])?;
                }
                // термы ТОЛЬКО заголовка (body пуст) — не терять
                for (term, ttf_v) in &title_tf {
                    if !tf.contains_key(*term) {
                        let blob = positions
                            .get(*term)
                            .map(|p| encode_positions(p))
                            .unwrap_or_default();
                        ins.execute(params![term, id, 0, ttf_v, blob])?;
                    }
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Запись
    // ------------------------------------------------------------------

    /// Upsert страницы + переиндексация термов + ссылки.
    /// Возвращает (page_id, была ли переиндексирована).
    pub fn upsert_page(&mut self, doc: &WebDoc) -> rusqlite::Result<(i64, bool)> {
        let text = clean_text(&doc.text, 256 * 1024);
        let body_tokens = tokenize_stem(&text);
        let title_tokens = tokenize_stem(&doc.title);
        // ЗАГОЛОВОК — ЧАСТЬ ДОКУМЕНТА: его термы индексируются тоже
        // (иначе запрос по слову из title не находит страницу).
        // Позиции: тело [0..len), затем разрыв TITLE_GAP и заголовок —
        // фраза не сшивается через границу тело→title.
        let positions = position_map(&body_tokens, &title_tokens);
        let mut tokens = body_tokens;
        tokens.extend(title_tokens.iter().cloned());
        let sim = simhash(&tokens) as i64;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let title = if doc.title.trim().is_empty() {
            title_from_text(&text)
        } else {
            doc.title.trim().to_string()
        };

        // существующая страница с тем же контентом — не переиндексируем
        let existing: Option<(i64, String)> = match self.conn.query_row(
            "SELECT id, content_hash FROM pages WHERE url = ?1",
            params![doc.url],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        ) {
            Ok(row) => Some(row),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e),
        };
        if let Some((id, hash)) = existing {
            if hash == doc.content_hash {
                self.conn.execute(
                    "UPDATE pages SET fetched_at = ?1 WHERE id = ?2",
                    params![now, id],
                )?;
                self.store_links(id, &doc.links)?;
                return Ok((id, false));
            }
        }

        // v2.0 (Приоритет 3.2): текст страницы хранится zstd-BLOB'ом;
        // `text` остаётся пустым (старые строки — плоский текст, лениво).
        let text_c = self.docstore.compress(&text).map_err(zstd_err)?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO pages(url, title, lang, meta_desc, text, text_c, content_hash, simhash, doclen, fetched_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(url) DO UPDATE SET
               title=?2, lang=?3, meta_desc=?4, text=?5, text_c=?6, content_hash=?7,
               simhash=?8, doclen=?9, fetched_at=?10, dup_of=''",
            params![
                doc.url,
                title,
                doc.lang,
                doc.meta_description,
                String::new(),
                text_c,
                doc.content_hash,
                sim,
                tokens.len() as i64,
                now
            ],
        )?;
        let id: i64 = tx.query_row(
            "SELECT id FROM pages WHERE url = ?1",
            params![doc.url],
            |r| r.get(0),
        )?;
        // полный переиндекс термов страницы (Percolator-lite: только изменённые)
        tx.execute("DELETE FROM terms WHERE page_id = ?1", params![id])?;
        tx.execute("DELETE FROM links WHERE src = ?1", params![id])?;
        {
            // tf — по телу, title_tf — по заголовку (честные частоты)
            let mut tf: HashMap<&str, i64> = HashMap::new();
            for t in &tokens {
                *tf.entry(t.as_str()).or_insert(0) += 1;
            }
            let mut title_tf: HashMap<&str, i64> = HashMap::new();
            for t in &title_tokens {
                *title_tf.entry(t.as_str()).or_insert(0) += 1;
            }
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO terms(term, page_id, tf, title_tf, positions)
                 VALUES(?1, ?2, ?3, ?4, ?5)",
            )?;
            for (term, tf_v) in &tf {
                let ttf_v = title_tf.get(term).copied().unwrap_or(0);
                let pos_blob = positions
                    .get(*term)
                    .map(|p| encode_positions(p))
                    .unwrap_or_default();
                stmt.execute(params![term, id, tf_v, ttf_v, pos_blob])?;
            }
        }
        {
            let mut stmt = tx.prepare("INSERT OR IGNORE INTO links(src, dst) VALUES(?1, ?2)")?;
            for dst in &doc.links {
                stmt.execute(params![id, dst])?;
            }
        }
        tx.commit()?;

        // Кольцо свежих текстов для обучения словаря (сжатая форма
        // непригодна для сэмплов): до 256 страниц по 8 КБ.
        if !self.docstore.has_dict() {
            let mut sample: String = text;
            truncate_char_safe_str(&mut sample, 8 * 1024);
            self.train_ring.push(sample);
            if self.train_ring.len() > 256 {
                self.train_ring.remove(0);
            }
            let min_pages = crate::compression::zstd_dict::DICT_TRAIN_MIN_PAGES;
            if self.train_ring.len() >= min_pages
                && self.train_ring.len() % min_pages == 0
            {
                self.train_docstore_dict()?;
            }
        }
        Ok((id, true))
    }

    fn store_links(&mut self, id: i64, links: &[String]) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM links WHERE src = ?1", params![id])?;
        let mut stmt = self
            .conn
            .prepare("INSERT OR IGNORE INTO links(src, dst) VALUES(?1, ?2)")?;
        for dst in links {
            stmt.execute(params![id, dst])?;
        }
        Ok(())
    }

    /// Записать страницу как дубликат (без термов) — SimHash-фильтр.
    pub fn record_duplicate(&mut self, url: &str, dup_of: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE pages SET dup_of = ?2 WHERE url = ?1",
            params![url, dup_of],
        )?;
        Ok(())
    }

    /// Поиск near-дубликата по SimHash (линейный скан; Manku-блоки — v0.10).
    pub fn find_duplicate(&self, url: &str, tokens: &[String]) -> Option<String> {
        if tokens.len() < 16 {
            return None; // короткие — не дедуплицируем
        }
        let sim = simhash(tokens);
        let mut stmt = self
            .conn
            .prepare("SELECT url, simhash, dup_of FROM pages WHERE simhash != 0")
            .ok()?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)? as u64,
                    r.get::<_, String>(2)?,
                ))
            })
            .ok()?;
        for row in rows.flatten() {
            if row.0 == url || !row.2.is_empty() {
                continue;
            }
            if near_duplicate(sim, row.1, tokens.len()) {
                return Some(row.0);
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // PageRank (Page & Brin 1998)
    // ------------------------------------------------------------------

    /// Пересчёт PageRank по таблице links: rank(p) = (1−d) + d·Σ rank(q)/out(q).
    pub fn recompute_pagerank(&mut self, iterations: usize) -> rusqlite::Result<()> {
        let ids: Vec<i64> = {
            let mut stmt = self.conn.prepare("SELECT id FROM pages")?;
            let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
            rows.collect::<Result<Vec<_>, rusqlite::Error>>()?
        };
        // рёбра только между проиндексированными страницами
        let edges: Vec<(i64, i64)> = {
            let mut stmt = self.conn.prepare(
                "SELECT l.src, p.id FROM links l JOIN pages p ON p.url = l.dst",
            )?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
            rows.collect::<Result<Vec<_>, rusqlite::Error>>()?
        };
        let n = ids.len().max(1) as f64;
        let mut rank: HashMap<i64, f64> = ids.iter().map(|&i| (i, 1.0)).collect();
        let mut outdeg: HashMap<i64, f64> = HashMap::new();
        let mut incoming: HashMap<i64, Vec<i64>> = HashMap::new();
        for &(s, d) in &edges {
            *outdeg.entry(s).or_insert(0.0) += 1.0;
            if s != d {
                incoming.entry(d).or_default().push(s);
            }
        }
        for _ in 0..iterations {
            let mut next: HashMap<i64, f64> = HashMap::with_capacity(ids.len());
            for &id in &ids {
                let mut sum = 0.0;
                if let Some(srcs) = incoming.get(&id) {
                    for &s in srcs {
                        let out = outdeg.get(&s).copied().unwrap_or(1.0).max(1.0);
                        sum += rank.get(&s).copied().unwrap_or(1.0) / out;
                    }
                }
                next.insert(id, (1.0 - DAMPING) / n + DAMPING * sum);
            }
            rank = next;
        }
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare("UPDATE pages SET rank = ?1 WHERE id = ?2")?;
            for (&id, r) in &rank {
                stmt.execute(params![r, id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Поиск: scatter по термам → gather → BM25 → WebRank
    // ------------------------------------------------------------------

    /// Поиск по веб-индексу. Возвращает Top-N отсортированных хитов.
    ///
    /// v0.11: сегменты запроса в кавычках (`"..."` / `«...»`) — ФРАЗЫ:
    /// документ обязан содержать их по смежности позиций (семантика точных
    /// цитат Google); вхождения дают proximity-бонус к BM25.
    pub fn search(&mut self, query: &str, top_n: usize) -> rusqlite::Result<Vec<WebHit>> {
        let qp = parse_query(query);
        let q_terms = qp.all_terms();
        if q_terms.is_empty() {
            return Ok(Vec::new());
        }
        let weighted: Vec<(String, f64)> = q_terms.iter().map(|t| (t.clone(), 1.0)).collect();
        self.search_core(&weighted, qp.phrases, top_n, q_terms.len(), &q_terms)
    }

    /// Поиск с Semantic Bridge (v0.21, Задача 3): кросс-языковый сенсор
    /// расширяет запрос (рус → англ корпус и обратно), кандидаты
    /// подмешиваются в BM25 с весом ≤ 0.85, ранжирование остаётся
    /// детерминированным WebRank. WHY-объяснение каждого расширения —
    /// в возвращаемой [`QueryExpansion`] (объяснимость обязательна).
    pub fn search_with_bridge(
        &mut self,
        query: &str,
        top_n: usize,
        bridge: &SemanticBridge,
    ) -> rusqlite::Result<(Vec<WebHit>, QueryExpansion)> {
        let qp = parse_query(query);
        let q_terms = qp.all_terms();
        if q_terms.is_empty() {
            return Ok((Vec::new(), QueryExpansion::default()));
        }
        let expansion = bridge.expand(&q_terms);
        let mut weighted: Vec<(String, f64)> =
            q_terms.iter().map(|t| (t.clone(), 1.0)).collect();
        for e in &expansion.expansions {
            if !weighted.iter().any(|(t, _)| *t == e.candidate.term) {
                weighted.push((e.candidate.term.clone(), e.bm25_weight));
            }
        }
        // сниппет подсвечивает и кандидатов сенсора — видно, ЧТО нашлось
        let mut snippet_terms = q_terms.clone();
        for t in expansion.extra_terms() {
            if !snippet_terms.contains(&t) {
                snippet_terms.push(t);
            }
        }
        let hits =
            self.search_core(&weighted, qp.phrases, top_n, q_terms.len(), &snippet_terms)?;
        Ok((hits, expansion))
    }

    /// Ядро поиска: scatter по ВЗВЕШЕННЫМ термам → gather → BM25 → WebRank.
    ///
    /// Родные термы запроса несут вес 1.0; сенсорные кандидаты Semantic
    /// Bridge — ≤ 0.85 (сигнал сенсора всегда слабее прямого совпадения).
    /// `phrases` — фразовые фильтры (только исходные термы, сенсор их не
    /// трогает); `title_denom` — число исходных термов (кандидаты не
    /// разбавляют title-буст); `snippet_terms` — подсветка сниппетов.
    fn search_core(
        &mut self,
        weighted: &[(String, f64)],
        phrases: Vec<Vec<String>>,
        top_n: usize,
        title_denom: usize,
        snippet_terms: &[String],
    ) -> rusqlite::Result<Vec<WebHit>> {
        if weighted.is_empty() {
            return Ok(Vec::new());
        }
        // N — все страницы с postings (дубликаты тоже, они фильтруются на gather)
        let n_pages: u64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM pages", [], |r| r.get(0))?;
        if n_pages == 0 {
            return Ok(Vec::new());
        }
        let sum_doclen: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(doclen), 0) FROM pages",
            [],
            |r| r.get(0),
        )?;
        let avgdl = (sum_doclen / n_pages as f64).max(1.0);

        // ---- scatter: postings каждого терма ----
        // doclen всех страниц — один префетч (не запрос на каждый posting)
        let doclens: HashMap<i64, f64> = {
            let mut stmt = self.conn.prepare("SELECT id, doclen FROM pages")?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?)))?;
            rows.collect::<Result<HashMap<_, _>, rusqlite::Error>>()?
        };
        // [0]=bm25, [1]=сумма tf (ε-плотность), [2]=термы запроса в заголовке
        let mut acc: HashMap<i64, [f64; 3]> = HashMap::new();

        // postings каждого терма — собираем ДО фильтрации: если стоп-ворд-фильтр
        // убил ВСЕ термы запроса, нужен откат к полному набору (иначе на моно-
        // корпусе «про nginx» запрос «gzip» даёт 0 результатов: предметный терм
        // встречается на каждой странице сайта и выглядит стоп-словом).
        // Для фразовых термов заодно тянем positions (delta-varint blob).
        /// (page_id, tf, title_tf) — строка postings одного терма.
        type RawPostings = Vec<(i64, i64, i64)>;
        /// page_id → positions-blob (только фразовые термы).
        type PosBlobs = HashMap<i64, Vec<u8>>;
        struct TermPostings {
            term: String,
            postings: Vec<(i64, i64, i64)>,
            /// позиции только у фразовых термов (память не тратим зря)
            pos_blobs: HashMap<i64, Vec<u8>>,
            dfv: u64,
            /// Вес терма: 1.0 — родной терм запроса, ≤ 0.85 — сенсорный
            /// кандидат Semantic Bridge (v0.21).
            weight: f64,
        }
        let phrase_terms: HashSet<&str> =
            phrases.iter().flatten().map(|s| s.as_str()).collect();
        let mut all_terms: Vec<TermPostings> = Vec::with_capacity(weighted.len());
        for (term, weight) in weighted {
            let need_pos = phrase_terms.contains(term.as_str());
            let (postings, pos_blobs): (RawPostings, PosBlobs) = if need_pos {
                    let mut stmt = self.conn.prepare(
                        "SELECT page_id, tf, title_tf, positions FROM terms WHERE term = ?1",
                    )?;
                    let rows = stmt.query_map(params![term], |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, Option<Vec<u8>>>(3)?,
                        ))
                    })?;
                    let mut postings = Vec::new();
                    let mut pos_blobs = HashMap::new();
                    for row in rows.flatten() {
                        let (pid, tf, ttf, blob) = row;
                        postings.push((pid, tf, ttf));
                        if let Some(b) = blob {
                            pos_blobs.insert(pid, b);
                        }
                    }
                    (postings, pos_blobs)
                } else {
                    let mut stmt = self.conn.prepare(
                        "SELECT page_id, tf, title_tf FROM terms WHERE term = ?1",
                    )?;
                    let rows = stmt.query_map(params![term], |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, i64>(2)?,
                        ))
                    })?;
                    let postings =
                        rows.collect::<Result<Vec<(i64, i64, i64)>, rusqlite::Error>>()?;
                    (postings, HashMap::new())
                };
            let dfv = postings.len() as u64;
            if dfv > 0 {
                all_terms.push(TermPostings {
                    term: term.clone(),
                    postings,
                    pos_blobs,
                    dfv,
                    weight: *weight,
                });
            }
        }

        // стоп-слово: терм почти во всех документах НЕМАЛЕНЬКОГО корпуса
        // (idf и так ≈ 0). Если после фильтра не осталось НИЧЕГО — считаем
        // корпус моно-тематическим и берём все термы: idf≈0 просто отдаст
        // ранжирование tf/title-совпадениям и PageRank (сигнал слабый, но
        // лучше, чем «ничего не найдено»).
        let mut kept: Vec<&TermPostings> = all_terms
            .iter()
            .filter(|t| !(n_pages >= 5 && t.dfv as f64 > n_pages as f64 * 0.95))
            .collect();
        if kept.is_empty() {
            kept = all_terms.iter().collect();
        }

        for t in kept {
            let idf = (n_pages.saturating_sub(t.dfv) as f64 + 0.5) / (t.dfv as f64 + 0.5);
            let idf = (1.0 + idf).ln();
            for &(pid, tf, title_tf) in &t.postings {
                let dl = doclens.get(&pid).copied().unwrap_or(1.0).max(1.0);
                let e = acc.entry(pid).or_insert([0.0; 3]);
                let denom = tf as f64 + K1 * (1.0 - B + B * dl / avgdl);
                e[0] += t.weight * idf * tf as f64 * (K1 + 1.0) / denom.max(1e-9);
                e[1] += tf as f64;
                e[2] += title_tf.min(1) as f64;
            }
        }

        // ---- фразовый фильтр: смежность позиций (v0.11) ----
        // Документ обязан содержать КАЖДУЮ фразу запроса целиком; страницы,
        // где слова фразы разбросаны по тексту, из выдачи исключаются —
        // семантика точных цитат. Вхождения дают proximity-бонус к BM25.
        let mut phrase_occ_total: HashMap<i64, usize> = HashMap::new();
        if !phrases.is_empty() {
            let by_term: HashMap<&str, &TermPostings> = all_terms
                .iter()
                .map(|t| (t.term.as_str(), t))
                .collect();
            let mut drop_pids: Vec<i64> = Vec::new();
            for (&pid, e) in &mut acc {
                let mut total = 0usize;
                let mut matched = true;
                'phrases: for phrase in &phrases {
                    let mut lists: Vec<Vec<u32>> = Vec::with_capacity(phrase.len());
                    let mut idf_sum = 0.0f64;
                    for t in phrase {
                        let Some(tp) = by_term.get(t.as_str()) else {
                            // терма нет ни в одном документе → фразы нет нигде
                            matched = false;
                            break 'phrases;
                        };
                        let list = match tp.pos_blobs.get(&pid) {
                            Some(blob) => decode_positions(blob),
                            None => Vec::new(),
                        };
                        if list.is_empty() {
                            matched = false; // в этом документе терма нет
                            break 'phrases;
                        }
                        let idf = (n_pages.saturating_sub(tp.dfv) as f64 + 0.5)
                            / (tp.dfv as f64 + 0.5);
                        idf_sum += (1.0 + idf).ln();
                        lists.push(list);
                    }
                    let refs: Vec<&[u32]> = lists.iter().map(|l| l.as_slice()).collect();
                    let occ = phrase_occurrences(&refs);
                    if occ == 0 {
                        matched = false; // слова есть, но не рядом
                        break 'phrases;
                    }
                    total += occ;
                    e[0] += PHRASE_BONUS * idf_sum * occ.min(8) as f64;
                }
                if matched {
                    phrase_occ_total.insert(pid, total);
                } else {
                    drop_pids.push(pid);
                }
            }
            for pid in drop_pids {
                acc.remove(&pid);
            }
            if acc.is_empty() {
                return Ok(Vec::new());
            }
        }

        // ---- gather: метаданные кандидатов ----
        struct Cand {
            e: [f64; 3],
            rank: f64,
            url: String,
            title: String,
            lang: String,
            text: String,
            fetched_at: i64,
            doclen: usize,
            phrase_occ: usize,
        }
        let mut candidates: Vec<Cand> = Vec::new();
        for (pid, e) in &acc {
            // v2.0: text_c (zstd) у свежих строк, text — у старых.
            let row = self.conn.query_row(
                "SELECT url, title, lang, text, text_c, rank, doclen, fetched_at, dup_of FROM pages WHERE id = ?1",
                params![pid],
                |r| {
                    let plain: String = r.get(3)?;
                    let blob: Option<Vec<u8>> = r.get(4)?;
                    let text = match blob {
                        Some(b) if !b.is_empty() => self
                            .docstore
                            .decompress(&b)
                            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                        _ => plain,
                    };
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        text,
                        r.get::<_, f64>(5)?,
                        r.get::<_, i64>(6)?,
                        r.get::<_, i64>(7)?,
                        r.get::<_, String>(8)?,
                    ))
                },
            );
            let Ok((url, title, lang, text, rank, doclen, fetched_at, dup_of)) = row else {
                continue;
            };
            if !dup_of.is_empty() {
                continue; // дубликаты не показываем
            }
            candidates.push(Cand {
                e: *e,
                rank,
                url,
                title,
                lang,
                text,
                fetched_at,
                doclen: doclen as usize,
                phrase_occ: phrase_occ_total.get(pid).copied().unwrap_or(0),
            });
        }
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        // ---- нормализация компонентов WebRank ----
        let max_bm25 = candidates.iter().map(|c| c.e[0]).fold(0.0_f64, f64::max).max(1e-9);
        let max_pr = candidates.iter().map(|c| c.rank).fold(0.0_f64, f64::max).max(1e-9);
        let max_eps = candidates
            .iter()
            .map(|c| if c.doclen > 0 { c.e[1] / c.doclen as f64 } else { 0.0 })
            .fold(0.0_f64, f64::max)
            .max(1e-9);

        let mut hits: Vec<WebHit> = candidates
            .into_iter()
            .map(|c| {
                let bm25n = c.e[0] / max_bm25;
                let prn = (1.0 + c.rank * 1000.0).ln() / (1.0 + max_pr * 1000.0).ln().max(1e-9);
                let title_frac = (c.e[2] / title_denom.max(1) as f64).min(1.0);
                let eps = if c.doclen > 0 { c.e[1] / c.doclen as f64 } else { 0.0 };
                let epsn = eps / max_eps;
                let score = W_BM25 * bm25n
                    + W_PAGERANK * prn
                    + W_TITLE * title_frac
                    + W_DENSITY * epsn;
                WebHit {
                    url: c.url,
                    title: c.title,
                    lang: c.lang,
                    score,
                    bm25: c.e[0],
                    pagerank: c.rank,
                    title_frac,
                    density: eps,
                    phrase_occ: c.phrase_occ,
                    snippet: snippet_for(&c.text, snippet_terms, 240),
                    doclen: c.doclen,
                    fetched_at: c.fetched_at,
                }
            })
            .collect();

        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(top_n);
        Ok(hits)
    }

    // ------------------------------------------------------------------
    // robots-кэш и статистика
    // ------------------------------------------------------------------

    /// Кэш robots.txt по хосту (TTL не проверяем — краулер решает сам).
    pub fn robots_for(&self, host: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT robots FROM hosts WHERE host = ?1",
                params![host],
                |r| r.get::<_, String>(0),
            )
            .ok()
    }

    pub fn save_robots(&mut self, host: &str, robots_body: &str) -> rusqlite::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.conn.execute(
            "INSERT INTO hosts(host, robots, robots_at) VALUES(?1, ?2, ?3)
             ON CONFLICT(host) DO UPDATE SET robots=?2, robots_at=?3",
            params![host, robots_body, now],
        )?;
        Ok(())
    }

    pub fn stats(&self) -> rusqlite::Result<IndexStats> {
        let pages = self.conn.query_row("SELECT COUNT(*) FROM pages", [], |r| r.get(0))?;
        let terms = self.conn.query_row("SELECT COUNT(DISTINCT term) FROM terms", [], |r| r.get(0))?;
        let postings = self.conn.query_row("SELECT COUNT(*) FROM terms", [], |r| r.get(0))?;
        let links = self.conn.query_row("SELECT COUNT(*) FROM links", [], |r| r.get(0))?;
        let duplicates =
            self.conn.query_row("SELECT COUNT(*) FROM pages WHERE dup_of != ''", [], |r| r.get(0))?;
        Ok(IndexStats {
            pages,
            terms,
            postings,
            links,
            duplicates,
            db_bytes: 0,
        })
    }

    /// Все страницы (для отчёта краулера).
    pub fn page_count(&self) -> u64 {
        self.conn
            .query_row("SELECT COUNT(*) FROM pages", [], |r| r.get(0))
            .unwrap_or(0)
    }

    /// Тестовый хелпер: есть ли термин в индексе.
    #[cfg(test)]
    pub fn postings_of(&self, term: &str) -> Vec<(i64, i64)> {
        let mut stmt = self
            .conn
            .prepare("SELECT page_id, tf FROM terms WHERE term = ?1")
            .unwrap();
        stmt.query_map(params![term], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    /// Тестовый хелпер: rank страницы по URL.
    #[cfg(test)]
    pub fn rank_of(&self, url: &str) -> f64 {
        self.conn
            .query_row("SELECT rank FROM pages WHERE url = ?1", params![url], |r| {
                r.get::<_, f64>(0)
            })
            .unwrap_or(0.0)
    }
}

/// Позиции термов в объединённом потоке `[тело | TITLE_GAP | заголовок]`.
/// Одинаковая функция для upsert и миграции — позиции восстанавливаются
/// байт-в-байт тем же путём.
fn position_map(body: &[String], title: &[String]) -> HashMap<String, Vec<u32>> {
    let mut map: HashMap<String, Vec<u32>> = HashMap::new();
    for (i, t) in body.iter().enumerate() {
        map.entry(t.clone()).or_default().push(i as u32);
    }
    let start = body.len() as u32 + TITLE_GAP;
    for (i, t) in title.iter().enumerate() {
        map.entry(t.clone()).or_default().push(start + i as u32);
    }
    map
}

/// FNV-1a 128-бит (два независимых 64) — content_hash для Percolator-lite.
pub fn content_hash(text: &str) -> String {
    let mut h1: u64 = 0xcbf29ce484222325;
    let mut h2: u64 = 0x84222325cbf29ce4;
    for b in text.as_bytes() {
        h1 ^= *b as u64;
        h1 = h1.wrapping_mul(0x100000001b3);
        h2 = h2.wrapping_add(*b as u64).rotate_left(13) ^ h2.wrapping_mul(0x9e3779b97f4a7c15);
    }
    format!("{h1:016x}{h2:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    // SimHash-дедуп: токены — как в upsert (tokenize_stem, v2.0)
    use crate::web::stem::tokenize_stem;

    fn doc(url: &str, title: &str, text: &str, links: Vec<&str>) -> WebDoc {
        WebDoc {
            url: url.to_string(),
            title: title.to_string(),
            lang: "en".to_string(),
            meta_description: String::new(),
            text: text.to_string(),
            links: links.into_iter().map(String::from).collect(),
            content_hash: content_hash(text),
        }
    }

    fn ix() -> WebIndex {
        WebIndex::open_memory().unwrap()
    }

    #[test]
    fn index_and_search_basic() {
        let mut ix = ix();
        ix.upsert_page(&doc(
            "https://a.io/rust",
            "Rust ownership",
            "rust ownership borrowing lifetimes memory safety",
            vec![],
        ))
        .unwrap();
        ix.upsert_page(&doc(
            "https://a.io/cooking",
            "Cooking pasta",
            "pasta recipe tomato sauce cheese delicious dinner",
            vec![],
        ))
        .unwrap();

        let hits = ix.search("rust ownership", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].url, "https://a.io/rust");
        assert!(hits[0].score > 0.5);
        assert!(!hits[0].snippet.is_empty() || hits[0].doclen > 0);

        let none = ix.search("quantum entanglement", 10).unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn title_match_beats_body_only() {
        let mut ix = ix();
        ix.upsert_page(&doc(
            "https://a.io/1",
            "mmap tutorial",
            "memory mapped files io performance kernel paging",
            vec![],
        ))
        .unwrap();
        ix.upsert_page(&doc(
            "https://a.io/2",
            "kernel guide",
            "mmap appears here only once in a long long long body of text about kernels",
            vec![],
        ))
        .unwrap();
        // третий документ без «mmap»: иначе df = N и терм станет стоп-словом
        ix.upsert_page(&doc(
            "https://a.io/3",
            "cooking",
            "pasta recipe with tomato sauce and cheese for dinner tonight",
            vec![],
        ))
        .unwrap();
        let hits = ix.search("mmap", 10).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].url, "https://a.io/1"); // title-буст
    }

    #[test]
    fn monothematic_corpus_subject_term_not_stopped_out() {
        // Регрессия v0.9.0: краулим один сайт про nginx — терм «gzip» есть
        // на КАЖДОЙ странице (N≥5, df=N) → стоп-ворд-фильтр убивал его и
        // поиск возвращал 0 результатов. Откат к полному набору термов,
        // когда фильтр опустошил запрос, обязан вернуть хиты.
        let mut ix = ix();
        for (i, title) in [
            "gzip module",
            "core directives",
            "http reference",
            "changes 1.0",
            "changes 1.10",
            "security advisories",
        ]
        .into_iter()
        .enumerate()
        {
            ix.upsert_page(&doc(
                &format!("https://nginx.org/p{i}"),
                title,
                "nginx http server gzip compression module directives config",
                vec![],
            ))
            .unwrap();
        }
        // «gzip» встречается в 6/6 документах: df == N → фильтр хотел бы
        // выкинуть его как стоп-слово, но это ЕДИНСТВЕННЫЙ терм запроса.
        let hits = ix.search("gzip", 10).unwrap();
        assert!(!hits.is_empty(), "моно-корпус: запрос из предметного терма не должен давать 0");
        // страница с «gzip» в заголовке должна побеждать по title-бусту
        assert_eq!(hits[0].url, "https://nginx.org/p0");
        // а в большом СМЕШАННОМ корпусе тот же фильтр обязан работать
        // (терм из 95%+ документов не должен забивать релевантные редкие)
        let mut ix2 = WebIndex::open_memory().unwrap();
        for i in 0..10 {
            let text = if i < 9 {
                "common boilerplate nav menu header footer gzip everywhere"
            } else {
                "quantum entanglement spins decoherence experiment"
            };
            ix2.upsert_page(&doc(&format!("https://m.io/{i}"), &format!("p{i}"), text, vec![]))
                .unwrap();
        }
        let hits2 = ix2.search("quantum entanglement", 10).unwrap();
        assert_eq!(hits2.len(), 1);
        assert_eq!(hits2[0].url, "https://m.io/9");
    }

    #[test]
    fn incremental_reindex_skips_unchanged() {
        let mut ix = ix();
        let d = doc("https://a.io/x", "T", "hello world content", vec![]);
        let (id1, indexed1) = ix.upsert_page(&d).unwrap();
        assert!(indexed1);
        let (id2, indexed2) = ix.upsert_page(&d).unwrap();
        assert_eq!(id1, id2);
        assert!(!indexed2); // тот же content_hash → Percolator-lite skip
        let mut changed = d;
        changed.text = "hello world content v2 new words".into();
        changed.content_hash = content_hash(&changed.text);
        let (_, indexed3) = ix.upsert_page(&changed).unwrap();
        assert!(indexed3);
    }

    #[test]
    fn pagerank_flow() {
        let mut ix = ix();
        // A ← B, A ← C, C ← B: у A максимум входящих
        ix.upsert_page(&doc(
            "https://a.io/hub",
            "hub",
            "hub page content words",
            vec![],
        ))
        .unwrap();
        ix.upsert_page(&doc(
            "https://a.io/b",
            "b",
            "b page content words",
            vec!["https://a.io/hub", "https://a.io/c"],
        ))
        .unwrap();
        ix.upsert_page(&doc(
            "https://a.io/c",
            "c",
            "c page content words",
            vec!["https://a.io/hub"],
        ))
        .unwrap();
        ix.recompute_pagerank(20).unwrap();
        let hub = ix.rank_of("https://a.io/hub");
        let b = ix.rank_of("https://a.io/b");
        assert!(hub > b, "hub={hub} b={b}");
        assert!(hub > 0.0);
    }

    #[test]
    fn simhash_duplicate_detection() {
        // v2.0: simhash в upsert считается по СТЕММИРОВАННЫМ токенам
        // (tokenize_stem, тот же путь что и термы) — тест использует его же.
        let mut ix = ix();
        let body = "the same long article body with many unique words like "
            .repeat(3);
        let text = format!("{body} alpha beta gamma delta epsilon zeta");
        ix.upsert_page(&doc("https://a.io/original", "Original", &text, vec![]))
            .unwrap();
        let toks = tokenize_stem(&text);
        assert!(ix.find_duplicate("https://a.io/copy", &toks).is_some());
        let other = tokenize_stem("completely different content about gardening tools");
        assert!(ix.find_duplicate("https://a.io/other", &other).is_none());
    }

    #[test]
    fn stopwords_downweighted() {
        let mut ix = ix();
        ix.upsert_page(&doc(
            "https://a.io/1",
            "one",
            "the the the the the the unique words here",
            vec![],
        ))
        .unwrap();
        ix.upsert_page(&doc(
            "https://a.io/2",
            "two",
            "the the the other things entirely unrelated anything",
            vec![],
        ))
        .unwrap();
        // df("unique") = 1 < N: ранжирует первая страница;
        // "the" (df = N = 2): N < 5 → не фильтруется жёстко, но idf≈0
        // не даёт ему перевесить осмысленный терм
        let hits = ix.search("the unique", 10).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].url, "https://a.io/1");
        assert!(hits[0].bm25.is_finite());
    }

    #[test]
    fn robots_cache_roundtrip() {
        let mut ix = ix();
        assert!(ix.robots_for("x.io").is_none());
        ix.save_robots("x.io", "User-agent: *\nDisallow: /tmp").unwrap();
        assert_eq!(ix.robots_for("x.io").unwrap(), "User-agent: *\nDisallow: /tmp");
    }

    #[test]
    fn stats_counts() {
        let mut ix = ix();
        ix.upsert_page(&doc("https://a.io/1", "t", "one two three", vec!["https://a.io/2"]))
            .unwrap();
        let s = ix.stats().unwrap();
        assert_eq!(s.pages, 1);
        assert_eq!(s.links, 1);
        assert!(s.postings >= 3);
    }

    #[test]
    fn content_hash_stable() {
        assert_eq!(content_hash("abc"), content_hash("abc"));
        assert_ne!(content_hash("abc"), content_hash("abd"));
    }

    #[test]
    fn russian_query_matches() {
        let mut ix = ix();
        ix.upsert_page(&doc(
            "https://a.io/ru",
            "Статья про память",
            "безопасность памяти без сборщика мусора владение заимствование",
            vec![],
        ))
        .unwrap();
        // «память» — терм из заголовка (заголовок индексируется)
        let hits = ix.search("память", 5).unwrap();
        assert_eq!(hits.len(), 1);
        // ё-нормализация: «тёмная ёлка» находится запросом «темная елка»
        ix.upsert_page(&doc("https://a.io/yo", "Ё", "тёмная ёлка зимой", vec![])).unwrap();
        let hits = ix.search("темная елка", 5).unwrap();
        assert!(hits.iter().any(|h| h.url == "https://a.io/yo"));
    }

    // ==================================================================
    // v0.11: фразовый поиск с позиционным индексом
    // ==================================================================

    #[test]
    fn phrase_search_exact_match() {
        let mut ix = ix();
        ix.upsert_page(&doc(
            "https://a.io/1",
            "async rust guide",
            "the rust async runtime is tokio; async io drives the reactor",
            vec![],
        ))
        .unwrap();
        ix.upsert_page(&doc(
            "https://a.io/2",
            "scattered",
            "rust is a language; much later we discuss async patterns and runtime details",
            vec![],
        ))
        .unwrap();
        // точная фраза: только страница, где слова стоят РЯДОМ
        let hits = ix.search("\"rust async\"", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].url, "https://a.io/1");
        assert!(hits[0].phrase_occ >= 1);
        // без кавычек — обе (мешок слов, прежняя семантика)
        let hits = ix.search("rust async", 10).unwrap();
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn phrase_search_stemmed_cyrillic() {
        let mut ix = ix();
        ix.upsert_page(&doc(
            "https://a.io/ru",
            "Владение памятью",
            "безопасное владение памятью без сборщика мусора",
            vec![],
        ))
        .unwrap();
        // падежи запроса отличаются от текста — стемминг унифицирует,
        // а смежность позиций подтверждает фразу
        let hits = ix.search("«владения память»", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].url, "https://a.io/ru");
        assert!(hits[0].phrase_occ >= 1);
    }

    #[test]
    fn phrase_not_across_title_boundary() {
        let mut ix = ix();
        // тело кончается на «memory safety», заголовок начинается с «Rust» —
        // фраза «safety rust» НЕ должна сшиваться через границу тело→title
        ix.upsert_page(&doc(
            "https://a.io/b",
            "Rust ownership",
            "lifetimes borrow checker memory safety",
            vec![],
        ))
        .unwrap();
        let hits = ix.search("\"safety rust\"", 10).unwrap();
        assert!(hits.is_empty(), "фраза через границу тело→title — не фраза");
        // а «memory safety» внутри тела — находится
        let hits = ix.search("\"memory safety\"", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].url, "https://a.io/b");
    }

    #[test]
    fn phrase_bonus_ranks_multiple_occurrences() {
        let mut ix = ix();
        ix.upsert_page(&doc("https://a.io/one", "one", "error handling here", vec![]))
            .unwrap();
        ix.upsert_page(&doc(
            "https://a.io/many",
            "many",
            "error handling again and again error handling everywhere",
            vec![],
        ))
        .unwrap();
        let hits = ix.search("\"error handling\"", 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].url, "https://a.io/many");
        assert!(hits[0].phrase_occ > hits[1].phrase_occ);
    }

    #[test]
    fn phrase_plus_free_terms() {
        let mut ix = ix();
        ix.upsert_page(&doc(
            "https://a.io/a",
            "a",
            "rust async runtime basics intro",
            vec![],
        ))
        .unwrap();
        ix.upsert_page(&doc(
            "https://a.io/b",
            "b",
            "rust async tokio scheduler deep dive",
            vec![],
        ))
        .unwrap();
        // обе содержат фразу; свободный терм «tokio» решает ранжирование
        let hits = ix.search("\"rust async\" tokio", 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].url, "https://a.io/b");
    }

    #[test]
    fn phrase_missing_everywhere_gives_zero() {
        let mut ix = ix();
        ix.upsert_page(&doc(
            "https://a.io/1",
            "t",
            "alpha beta gamma delta epsilon",
            vec![],
        ))
        .unwrap();
        // оба слова есть в документе, но НЕ рядом → честный ноль
        let hits = ix.search("\"gamma alpha\"", 10).unwrap();
        assert!(hits.is_empty());
        // и обратный порядок не матчится
        let hits = ix.search("\"beta gamma\" alpha", 10).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn positions_survive_reindex() {
        let mut ix = ix();
        let mut d = doc("https://a.io/x", "T", "hello world phrase here", vec![]);
        ix.upsert_page(&d).unwrap();
        d.text = "hello world phrase here updated tail".into();
        d.content_hash = content_hash(&d.text);
        ix.upsert_page(&d).unwrap();
        let hits = ix.search("\"world phrase\"", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].url, "https://a.io/x");
    }

    #[test]
    fn migration_v1_db_gets_positions() {
        // ручная v1-БД (v0.9/v0.10): terms БЕЗ колонки positions
        let dir = std::env::temp_dir().join(format!("poler-mig-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let db = dir.join("mig.db");
        let _ = std::fs::remove_file(&db);
        {
            let conn = Connection::open(&db).unwrap();
            conn.execute_batch(
                "CREATE TABLE pages(id INTEGER PRIMARY KEY, url TEXT UNIQUE NOT NULL,
                   title TEXT NOT NULL DEFAULT '', lang TEXT NOT NULL DEFAULT '',
                   meta_desc TEXT NOT NULL DEFAULT '', text TEXT NOT NULL DEFAULT '',
                   content_hash TEXT NOT NULL, simhash INTEGER NOT NULL,
                   doclen INTEGER NOT NULL, rank REAL NOT NULL DEFAULT 1.0,
                   dup_of TEXT NOT NULL DEFAULT '', fetched_at INTEGER NOT NULL);
                 CREATE TABLE terms(term TEXT NOT NULL, page_id INTEGER NOT NULL,
                   tf INTEGER NOT NULL, title_tf INTEGER NOT NULL,
                   PRIMARY KEY(term, page_id)) WITHOUT ROWID;
                 CREATE TABLE links(src INTEGER NOT NULL, dst TEXT NOT NULL,
                   PRIMARY KEY(src, dst)) WITHOUT ROWID;
                 CREATE TABLE hosts(host TEXT PRIMARY KEY, robots TEXT NOT NULL DEFAULT '',
                   robots_at INTEGER NOT NULL DEFAULT 0);
                 CREATE TABLE meta(k TEXT PRIMARY KEY, v TEXT NOT NULL);
                 INSERT INTO pages(url, title, lang, meta_desc, text, content_hash,
                   simhash, doclen, fetched_at)
                   VALUES('https://old.io/1', 'Old page', 'en', '',
                          'tokio rust async runtime scheduler', 'deadbeef', 0, 5, 100);
                 INSERT INTO terms(term, page_id, tf, title_tf) VALUES
                   ('tokio', 1, 1, 0), ('rust', 1, 1, 0), ('async', 1, 1, 0),
                   ('runtime', 1, 1, 0), ('scheduler', 1, 1, 0);",
            )
            .unwrap();
        }
        // открытие мигрирует: ALTER TABLE + пересчёт позиций из text
        let mut ix = WebIndex::open(&db).unwrap();
        let hits = ix.search("\"rust async\"", 10).unwrap();
        assert_eq!(hits.len(), 1, "после миграции фразовый поиск обязан работать");
        assert_eq!(hits[0].url, "https://old.io/1");
        assert!(hits[0].phrase_occ >= 1);
        // повторное открытие — колонка уже есть, миграция холостая
        drop(ix);
        let mut ix2 = WebIndex::open(&db).unwrap();
        let hits = ix2.search("\"async runtime\"", 10).unwrap();
        assert_eq!(hits.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn phrase_queries_via_plain_terms_unchanged() {
        // регрессия: запросы БЕЗ кавычек ведут себя как раньше (OR-семантика)
        let mut ix = ix();
        ix.upsert_page(&doc(
            "https://a.io/rust",
            "Rust ownership",
            "rust ownership borrowing lifetimes memory safety",
            vec![],
        ))
        .unwrap();
        ix.upsert_page(&doc(
            "https://a.io/cooking",
            "Cooking pasta",
            "pasta recipe tomato sauce cheese delicious dinner",
            vec![],
        ))
        .unwrap();
        let hits = ix.search("rust ownership", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].phrase_occ, 0);
        let none = ix.search("quantum entanglement", 10).unwrap();
        assert!(none.is_empty());
    }
}
