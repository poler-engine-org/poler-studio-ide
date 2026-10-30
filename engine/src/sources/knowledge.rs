//! # Суверенный Гиппокамп — библиотека POLER в нативном индексе (v0.29.0)
//!
//! Замыкает конвейер «корпус → нативный движок → агент»: внешняя библиотека
//! (`POLER_ALL_GENERATED_DOCS`, 6 слоёв) инжестится в собственные форматы
//! poler-engine — ни внешнего RAG, ни облачных API, ни ключей.
//!
//! ```text
//! 01_TECHNICAL_SPECS_300/PTS-*.md ──┐  секция 4 → первоисточник ×1.0
//! 02_THEMATIC_VOLUMES_8/*.md ───────┤  синтез → нарратив ×0.7
//! 03_MATHEMATICAL_TREATISE_I_VI/ ───┤  тома I–V → MVR-доказано ×1.5 (VI → нарратив)
//! 04_CIPHER_CORE_AND_PROOFS/ ───────┤  спека+код шифра → первоисточник ×1.0
//! 05_APPLIED_CRYPTOGRAPHY_UA/*.md ──┤  Шнайер (укр) → первоисточник ×1.0
//! 06_CHOPOCOMPIK_DATABASE_194/ ─────┘  транскрипты (SQLite) → первоисточник ×1.0
//!            │  scan_library (детерминированный порядок, дедуп)
//!            ▼
//!   chunk::chunk_document — чанки с якорями (byte range, строки, breadcrumb)
//!            │  провенанс наследуется от секции (чанк не пересекает границу)
//!            ▼
//!   web::index::WebIndex  ── BM25 + WebRank + Semantic Bridge (ru↔en)
//!   + knowledge_chunks    ── сайдкар: якоря, провенанс, цитаты
//!            │
//!            ▼ (опционально)
//!   vectors: PqwEmbedder (.pqw) | HashEmbedder → RaBitQ 144 Б/вектор + HNSW
//!```
//!
//! ## Эпистемический контракт
//!
//! Ранжирование остаётся детерминированным: агент получает не «мнение»,
//! а взвешенную выдачу с ЯВНОЙ меткой происхождения каждого урывка
//! ([`Provenance`]). Фильтр `min_provenance` отсекает недостоверное,
//! коэффициент ×1.5/×1.0/×0.7 применяется к гибридному скору ПОСЛЕ
//! слияния русел — доказанный факт весит больше пересказа, пересказ —
//! больше необоснованного нарратива.
//!
//! ## Гибридное ранжирование
//!
//! * русло BM25: `search_with_bridge` (кросс-языковое расширение запроса,
//!   WHY-объяснение обязательно);
//! * русло векторов: HNSW-обход по RaBitQ-кодам, ADC-косинус запроса;
//! * слияние: `W_LEXICAL·norm(BM25) + W_VECTOR·cos` → `× coefficient(p)`.
//!
//! Инварианты: `text == file[byte_start..byte_end]` (агент верифицирует
//! цитату по диапазону); повторный инжест идемпотентен (URL стабильны);
//! векторный слой воспроизводим (сид вращения фиксирован в файле PRBQ).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use rusqlite::{Connection, OpenFlags, params};
use serde::Serialize;

use crate::retrieval::chunk::DEFAULT_MIN_TOKENS;
use crate::retrieval::{
    chunk_document, ChunkConfig, ChunkFormat, Provenance, SemanticBridge, DEFAULT_OVERLAP_TOKENS,
    DEFAULT_TARGET_TOKENS,
};
use crate::vectors::{
    CodeSource, Embedder, HashEmbedder, HnswConfig, HnswIndex, PqwEmbedder, QuantizedStore,
    QuantizedStoreView,
};
use crate::web::index::{content_hash, WebDoc, WebIndex};

/// Префикс URL чанков библиотеки в pages (отличаем от крауленых страниц).
pub const URL_PREFIX: &str = "poler://lib/";
/// Вес лексического русла (BM25/WebRank) в гибридном скоре.
pub const W_LEXICAL: f64 = 0.65;
/// Вес векторного русла (ADC-косинус) в гибридном скоре.
pub const W_VECTOR: f64 = 0.35;
/// Сид вращения Адамара для RaBitQ-субстрата (детерминизм сборки).
pub const VECTOR_ROT_SEED: u64 = 0x5EED_C0DE_0000_0001;
/// Размерность хэш-проекции (инструментальной, не ИИ) для `--knowledge-embedder hash`.
pub const HASH_DIM: usize = 512;
/// Сид хэш-проекции.
pub const HASH_SEED: u64 = 0x9E37_79B9_7F4A_7C15;
/// Минимум токенов в чанке (микро-осколки не индексируем).
const MIN_CHUNK_TOKENS: usize = 4;
/// Прогресс-лог инжеста: каждые N чанков.
const PROGRESS_EVERY: usize = 2000;

/// БД знаний по умолчанию: `POLER_KNOWLEDGE_DB` | ~/.local/share/poler-engine/knowledge.db.
pub fn default_db_path() -> PathBuf {
    if let Ok(p) = std::env::var("POLER_KNOWLEDGE_DB") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local/share/poler-engine/knowledge.db")
}

/// Путь RaBitQ-хранилища векторов для данной БД.
pub fn vectors_path(db: &Path) -> PathBuf {
    let mut s = db.as_os_str().to_os_string();
    s.push(".vectors");
    PathBuf::from(s)
}

/// Путь HNSW-графа для данной БД.
pub fn graph_path(db: &Path) -> PathBuf {
    let mut s = db.as_os_str().to_os_string();
    s.push(".graph");
    PathBuf::from(s)
}

// ---------------------------------------------------------------------------
// Скан библиотеки: слои → секции с провенансом
// ---------------------------------------------------------------------------

/// Секция документа библиотеки — атом провенанса. Внутри секции статус
/// однороден; чанки наследуют его (никогда не пересекают границу).
#[derive(Debug, Clone)]
pub struct LibrarySection {
    /// Стабильный ключ документа (PTS-074, TREATISE-V, VIDEO-xyz).
    pub doc_key: String,
    /// Относительный путь в библиотеке (или db#id для транскриптов).
    pub source_path: String,
    /// Исходный URL (транскрипты) — для цитирования.
    pub origin_url: Option<String>,
    /// Заголовок документа (H1 / имя файла / название видео).
    pub title: String,
    /// Метка секции («4. ПОЛНЫЙ ТЕХНИЧЕСКИЙ ТЕКСТ…»; пусто для целых файлов).
    pub section_label: String,
    /// Эпистемический статус секции.
    pub provenance: Provenance,
    /// Код языка (эвристика по слою; хранится в индексе).
    pub lang: &'static str,
    /// Порядковый номер секции внутри документа (стабильные URL).
    pub ordinal: usize,
    /// Начало секции в байтах исходного файла (включительно).
    pub byte_start: usize,
    /// Конец секции в байтах исходного файла (исключительно).
    pub byte_end: usize,
    /// Первая строка секции, 1-базная (для якорей чанков).
    pub line_start: usize,
    /// Текст секции — точный срез исходника.
    pub text: String,
    /// Формат для чанкера.
    pub format: ChunkFormat,
}

/// Итог скана библиотеки.
#[derive(Debug)]
pub struct ScanOutcome {
    /// Секции в детерминированном порядке (слои 01→06, файлы по имени).
    pub sections: Vec<LibrarySection>,
    /// Число просмотренных файлов.
    pub files: usize,
    /// Нераспознанные элементы корня (максимум 8, для диагностики).
    pub warnings: Vec<String>,
}

/// Скан корня библиотеки. Слои распознаются по префиксу имени каталога
/// (`01_`…`06_`); неизвестные каталоги попадают в warnings, не в панику.
pub fn scan_library(root: &Path) -> Result<ScanOutcome, String> {
    if !root.is_dir() {
        return Err(format!("корень библиотеки не каталог: {}", root.display()));
    }
    let mut out = ScanOutcome { sections: Vec::new(), files: 0, warnings: Vec::new() };
    for e in sorted_entries(root)? {
        let path = root.join(&e);
        if !path.is_dir() {
            continue; // README.md и пр. — не индексируем
        }
        if e.starts_with("01_") {
            scan_pts_dir(&path, e.clone(), &mut out)?;
        } else if e.starts_with("02_") {
            scan_md_dir(&path, "THEME", Provenance::Narrative, "ru", &mut out)?;
        } else if e.starts_with("03_") {
            scan_treatise_dir(&path, e.clone(), &mut out)?;
        } else if e.starts_with("04_") {
            scan_cipher_dir(&path, e.clone(), &mut out)?;
        } else if e.starts_with("05_") {
            scan_md_dir(&path, "SCHNEIER", Provenance::SourceDocument, "uk", &mut out)?;
        } else if e.starts_with("06_") {
            scan_transcript_db(&path, e.clone(), &mut out)?;
        } else if out.warnings.len() < 8 {
            out.warnings.push(format!("нераспознанный каталог: {e}"));
        }
    }
    Ok(out)
}

/// Отсортированные имена записей каталога (детерминизм обхода).
fn sorted_entries(dir: &Path) -> Result<Vec<String>, String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| format!("read_dir {}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    v.sort();
    Ok(v)
}

/// Номер строки (1-базный) байтового смещения.
fn line_at(text: &str, byte: usize) -> usize {
    let mut line = 1usize;
    for (i, b) in text.as_bytes().iter().enumerate() {
        if i >= byte {
            break;
        }
        if *b == b'\n' {
            line += 1;
        }
    }
    line
}

/// FNV-1a — быстрый детерминированный хэш (дедуп секций).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Секции уровня `## `: (заголовок, начало, конец). Преамбула до первого
/// заголовка — секция с пустым заголовком. Подзаголовки `###` не делят
/// (их обрабатывает чанкер с breadcrumbs).
fn split_h2(text: &str) -> Vec<(String, usize, usize)> {
    let mut out: Vec<(String, usize, usize)> = Vec::new();
    let mut header = String::new();
    let mut start = 0usize;
    let mut off = 0usize;
    for line in text.split_inclusive('\n') {
        if let Some(rest) = line.strip_prefix("## ") {
            if off > start || !out.is_empty() || off > 0 {
                out.push((std::mem::take(&mut header), start, off));
            }
            header = rest.trim().to_string();
            start = off;
        }
        off += line.len();
    }
    out.push((header, start, text.len()));
    out.retain(|(_, s, e)| e > s);
    out
}

/// Номер секции из заголовка («4. ПОЛНЫЙ ТЕКСТ» → Some(4)).
fn section_number(header: &str) -> Option<u32> {
    header
        .split('.')
        .next()
        .and_then(|w| w.trim().parse::<u32>().ok())
}

/// Слой 01: PTS-спеки. Секция 4 = полный текст первоисточника (×1.0),
/// остальные секции = машинный нарратив Гемини (×0.7).
fn scan_pts_dir(dir: &Path, layer: String, out: &mut ScanOutcome) -> Result<(), String> {
    for name in sorted_entries(dir)? {
        if !name.ends_with(".md") {
            continue;
        }
        out.files += 1;
        let path = dir.join(&name);
        let Ok(text) = std::fs::read_to_string(&path) else {
            out.warnings.push(format!("не читается: {layer}/{name}"));
            continue;
        };
        let doc_key = name.trim_end_matches(".md").to_string();
        let title = text
            .lines()
            .next()
            .map(|l| l.trim().trim_start_matches('#').trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| doc_key.clone());
        for (ordinal, (header, s, e)) in split_h2(&text).into_iter().enumerate() {
            let primary = section_number(&header) == Some(4);
            let (provenance, lang) = if primary {
                (Provenance::SourceDocument, "en")
            } else {
                (Provenance::Narrative, "ru")
            };
            out.sections.push(LibrarySection {
                doc_key: doc_key.clone(),
                source_path: format!("{layer}/{name}"),
                origin_url: None,
                title: title.clone(),
                section_label: header,
                provenance,
                lang,
                ordinal,
                byte_start: s,
                byte_end: e,
                line_start: line_at(&text, s),
                text: text[s..e].to_string(),
                format: ChunkFormat::Markdown,
            });
        }
    }
    Ok(())
}

/// Слой 03: математический трактат. Тома I–V машинно доказаны (MVR-паспорта,
/// ×1.5); том VI — манифест без верификации (×0.7).
fn scan_treatise_dir(dir: &Path, layer: String, out: &mut ScanOutcome) -> Result<(), String> {
    for name in sorted_entries(dir)? {
        if !name.ends_with(".md") {
            continue;
        }
        out.files += 1;
        let path = dir.join(&name);
        let Ok(text) = std::fs::read_to_string(&path) else {
            out.warnings.push(format!("не читается: {layer}/{name}"));
            continue;
        };
        let roman = name
            .strip_prefix("VOLUME_")
            .and_then(|r| r.split('_').next())
            .unwrap_or("VI")
            .to_string();
        let provenance = match roman.as_str() {
            "I" | "II" | "III" | "IV" | "V" => Provenance::MvrVerified,
            _ => Provenance::Narrative,
        };
        let title = text
            .lines()
            .next()
            .map(|l| l.trim().trim_start_matches('#').trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| name.clone());
        out.sections.push(LibrarySection {
            doc_key: format!("TREATISE-{roman}"),
            source_path: format!("{layer}/{name}"),
            origin_url: None,
            title,
            section_label: String::new(),
            provenance,
            lang: "ru",
            ordinal: 0,
            byte_start: 0,
            byte_end: text.len(),
            line_start: 1,
            text,
            format: ChunkFormat::Markdown,
        });
    }
    Ok(())
}

/// Слой 04: ядро шифра и пруфы. Спецификация и код — первоисточники (×1.0).
fn scan_cipher_dir(dir: &Path, layer: String, out: &mut ScanOutcome) -> Result<(), String> {
    for name in sorted_entries(dir)? {
        let path = dir.join(&name);
        if path.is_dir() {
            continue;
        }
        let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
        let format = match ext.as_str() {
            "md" => ChunkFormat::Markdown,
            "zig" | "py" | "rs" | "c" | "h" => ChunkFormat::Code,
            _ => continue,
        };
        out.files += 1;
        let Ok(text) = std::fs::read_to_string(&path) else {
            out.warnings.push(format!("не читается: {layer}/{name}"));
            continue;
        };
        let dot = format!(".{ext}");
        let stem = name.strip_suffix(dot.as_str()).unwrap_or(name.as_str());
        let lang = if format == ChunkFormat::Code { "en" } else { "ru" };
        out.sections.push(LibrarySection {
            doc_key: format!("CIPHER-{stem}"),
            source_path: format!("{layer}/{name}"),
            origin_url: None,
            title: name.clone(),
            section_label: String::new(),
            provenance: Provenance::SourceDocument,
            lang,
            ordinal: 0,
            byte_start: 0,
            byte_end: text.len(),
            line_start: 1,
            text,
            format,
        });
    }
    Ok(())
}

/// Универсальный слой .md-файлов с единым провенансом (02-тематика, 05-Шнайер).
fn scan_md_dir(
    dir: &Path,
    key_prefix: &str,
    provenance: Provenance,
    lang: &'static str,
    out: &mut ScanOutcome,
) -> Result<(), String> {
    for name in sorted_entries(dir)? {
        if !name.ends_with(".md") {
            continue;
        }
        out.files += 1;
        let path = dir.join(&name);
        let Ok(text) = std::fs::read_to_string(&path) else {
            out.warnings.push(format!("не читается: {}", path.display()));
            continue;
        };
        let stem = name.trim_end_matches(".md");
        let title = text
            .lines()
            .next()
            .map(|l| l.trim().trim_start_matches('#').trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| stem.to_string());
        let layer = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        out.sections.push(LibrarySection {
            doc_key: format!("{key_prefix}-{stem}"),
            source_path: format!("{layer}/{name}"),
            origin_url: None,
            title,
            section_label: String::new(),
            provenance,
            lang,
            ordinal: 0,
            byte_start: 0,
            byte_end: text.len(),
            line_start: 1,
            text,
            format: ChunkFormat::Markdown,
        });
    }
    Ok(())
}

/// Слой 06: SQLite-база транскриптов (таблица `videos` Гемини-пайплайна).
fn scan_transcript_db(dir: &Path, layer: String, out: &mut ScanOutcome) -> Result<(), String> {
    for name in sorted_entries(dir)? {
        if !name.ends_with(".db") {
            continue;
        }
        let path = dir.join(&name);
        out.files += 1;
        let conn = match Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
            Ok(c) => c,
            Err(e) => {
                out.warnings.push(format!("не открывается {layer}/{name}: {e}"));
                continue;
            }
        };
        let has_videos: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='videos'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false);
        if !has_videos {
            out.warnings.push(format!("{layer}/{name}: таблицы videos нет — пропуск"));
            continue;
        }
        let mut stmt = match conn.prepare(
            "SELECT id, title, url, full_text FROM videos ORDER BY rowid",
        ) {
            Ok(s) => s,
            Err(e) => {
                out.warnings.push(format!("{layer}/{name}: {e}"));
                continue;
            }
        };
        let rows: Vec<(String, String, String, String)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                ))
            })
            .map_err(|e| format!("{layer}/{name}: {e}"))?
            .flatten()
            .collect();
        for (vid, title, url, full_text) in rows {
            if full_text.trim().is_empty() {
                continue;
            }
            let safe_id: String = vid
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect();
            let origin = if url.is_empty() { None } else { Some(url) };
            out.sections.push(LibrarySection {
                doc_key: format!("VIDEO-{safe_id}"),
                source_path: format!("{layer}/{name}#{vid}"),
                origin_url: origin,
                title: if title.is_empty() { format!("transcript {vid}") } else { title },
                section_label: String::new(),
                provenance: Provenance::SourceDocument,
                lang: "ru",
                ordinal: 0,
                byte_start: 0,
                byte_end: full_text.len(),
                line_start: 1,
                text: full_text,
                format: ChunkFormat::Plain,
            });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Эмбеддер векторного русла
// ---------------------------------------------------------------------------

/// Источник эмбеддингов для векторного слоя гиппокампа.
///
/// * `None` — только BM25/WebRank (быстрый инжест, честный режим без ИИ);
/// * `Hash` — детерминированная хэш-проекция 512-d: полный гибридный
///   конвейер (RaBitQ+HNSW) без модели — инструментальная проекция,
///   НЕ семантика (одинаковые слова сближают, синонимы — нет);
/// * `Pqw` — нативный энкодер из `.pqw` (BGE-M3/XLM-R класс, mmap+Sha256).
pub enum KnowledgeEmbedder {
    /// Векторное русло отключено.
    None,
    /// Хэш-проекция фиксированной размерности.
    Hash(HashEmbedder),
    /// Нативный `.pqw`-энкодер.
    Pqw(PqwEmbedder),
}

impl KnowledgeEmbedder {
    /// Разбор режима CLI: none | hash | pqw (pqw требует --model).
    pub fn from_mode(mode: &str, model: Option<&Path>) -> Result<Self, String> {
        match mode.trim().to_lowercase().as_str() {
            "none" => Ok(Self::None),
            "hash" => Ok(Self::Hash(HashEmbedder::new(HASH_DIM, HASH_SEED))),
            "pqw" => {
                let p = model.ok_or(
                    "--knowledge-embedder pqw требует --model <path.pqw> \
                     (суверенные веса, без сети)",
                )?;
                Ok(Self::Pqw(PqwEmbedder::open(p)?))
            }
            other => Err(format!(
                "неизвестный эмбеддер {other:?} (доступны: none | hash | pqw)"
            )),
        }
    }

    /// Имя для knowledge_meta (воспроизведение при запросе).
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Hash(_) => "hash-projection-512",
            Self::Pqw(_) => "pqw-native-encoder",
        }
    }

    /// Размерность (0 для None).
    pub fn dim(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Hash(h) => h.dim(),
            Self::Pqw(p) => p.dim(),
        }
    }

    /// Эмбеддинг корпуса (rayon для .pqw; хэш — мгновенно).
    pub fn embed_corpus(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        match self {
            Self::None => Err("векторное русло отключено (none)".into()),
            Self::Hash(h) => {
                let mut hc = h.clone();
                let mut out = Vec::with_capacity(texts.len());
                for t in texts {
                    out.extend(hc.embed_batch(&[t.as_str()])?);
                }
                Ok(out)
            }
            Self::Pqw(p) => {
                use rayon::prelude::*;
                let hdr = p.model().view().header();
                let cap = hdr.max_pos
                    - if hdr.xlmr_positions() { 2 } else { 0 };
                let done = std::sync::atomic::AtomicUsize::new(0);
                texts
                    .par_iter()
                    .map(|t| {
                        let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        if n % PROGRESS_EVERY == 0 {
                            eprintln!("  эмбеддинг: {n}/{}", texts.len());
                        }
                        let mut ids = p.token_ids(t);
                        if ids.len() > cap {
                            ids.truncate(cap);
                        }
                        p.model().embed(&ids)
                    })
                    .collect()
            }
        }
    }

    /// Эмбеддинг одного текста (запрос).
    pub fn embed_query(&mut self, text: &str) -> Result<Vec<f32>, String> {
        match self {
            Self::None => Err("векторное русло отключено (none)".into()),
            Self::Hash(h) => {
                Ok(h.embed_batch(&[text])?.into_iter().next().unwrap())
            }
            Self::Pqw(p) => {
                Ok(p.embed_batch(&[text])?.into_iter().next().unwrap())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Инжест
// ---------------------------------------------------------------------------

/// Настройки инжеста.
#[derive(Debug, Clone)]
pub struct IngestOptions {
    /// Целевой размер чанка (токены POLER).
    pub target_tokens: usize,
    /// Перекрытие соседних чанков.
    pub overlap_tokens: usize,
    /// Минимум токенов хвостового чанка.
    pub min_tokens: usize,
    /// Дедуп байт-идентичных секций (секции 3/5 PTS совпадают по 300 спекам).
    pub dedupe: bool,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            target_tokens: DEFAULT_TARGET_TOKENS,
            overlap_tokens: DEFAULT_OVERLAP_TOKENS,
            min_tokens: DEFAULT_MIN_TOKENS,
            dedupe: true,
        }
    }
}

/// Отчёт инжеста (человекочитаемый рендер — [`IngestReport::render_text`]).
#[derive(Debug, Clone, Serialize)]
pub struct IngestReport {
    /// Корень библиотеки.
    pub library_root: String,
    /// Файлов просмотрено.
    pub files: usize,
    /// Секций найдено.
    pub sections: usize,
    /// Секций пропущено дедупом (байт-идентичны ранее встреченным).
    pub sections_deduped: usize,
    /// Чанков записано.
    pub chunks: usize,
    /// Суммарные токены POLER.
    pub tokens: usize,
    /// Чанки по провенансу (ключи — as_str).
    pub by_provenance: BTreeMap<String, usize>,
    /// Эмбеддер векторного русла.
    pub embedder: String,
    /// Векторов построено (0 — русло отключено).
    pub vectors: usize,
    /// Байты RaBitQ-хранилища.
    pub vector_bytes: u64,
    /// Байты HNSW-графа.
    pub graph_bytes: u64,
    /// Байты БД знаний.
    pub db_bytes: u64,
    /// Путь к БД знаний.
    pub db_path: String,
    /// Время инжеста, мс.
    pub elapsed_ms: u128,
}

impl IngestReport {
    /// Человекочитаемый отчёт (CLI).
    pub fn render_text(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "инжест завершён · {:.1} с\n",
            self.elapsed_ms as f64 / 1000.0
        ));
        s.push_str(&format!(
            "библиотека: {} · {} файлов · {} секций (дедуп: {})\n",
            self.library_root, self.files, self.sections, self.sections_deduped
        ));
        s.push_str(&format!(
            "чанков: {} · {} токенов\n",
            self.chunks,
            fmt_thousands(self.tokens)
        ));
        for (k, v) in &self.by_provenance {
            let badge = Provenance::parse(k)
                .map(|p| p.badge())
                .unwrap_or(k.as_str());
            s.push_str(&format!("  {:<18} {:>8}\n", badge, fmt_thousands(*v)));
        }
        s.push_str(&format!("БД: {} ({:.1} МБ)\n", self.db_path, self.db_bytes as f64 / 1e6));
        if self.vectors > 0 {
            s.push_str(&format!(
                "векторный слой: {} · {} векторов · RaBitQ {:.1} МБ + HNSW {:.1} МБ\n",
                self.embedder,
                fmt_thousands(self.vectors),
                self.vector_bytes as f64 / 1e6,
                self.graph_bytes as f64 / 1e6
            ));
        } else {
            s.push_str("векторный слой: отключён (--knowledge-embedder hash|pqw включает)\n");
        }
        s
    }
}

/// 11432 → «11 432» (тонкие пробелы).
fn fmt_thousands(n: usize) -> String {
    let raw = n.to_string();
    let mut out = String::with_capacity(raw.len() + raw.len() / 3);
    let bytes = raw.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(*b as char);
    }
    out
}

/// Строка сайдкар-таблицы knowledge_chunks.
#[derive(Debug, Clone)]
struct ChunkRow {
    chunk_id: i64,
    url: String,
    doc_key: String,
    source_path: String,
    origin_url: Option<String>,
    title: String,
    breadcrumb: String,
    provenance: Provenance,
    line_start: usize,
    line_end: usize,
    byte_start: usize,
    byte_end: usize,
    tokens: usize,
    text: String,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS knowledge_chunks (
    chunk_id    INTEGER PRIMARY KEY,
    url         TEXT NOT NULL UNIQUE,
    doc_key     TEXT NOT NULL,
    source_path TEXT NOT NULL,
    origin_url  TEXT,
    title       TEXT NOT NULL,
    breadcrumb  TEXT NOT NULL,
    provenance  TEXT NOT NULL,
    line_start  INTEGER NOT NULL,
    line_end    INTEGER NOT NULL,
    byte_start  INTEGER NOT NULL,
    byte_end    INTEGER NOT NULL,
    tokens      INTEGER NOT NULL,
    text        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_kchunk_prov ON knowledge_chunks(provenance);
CREATE INDEX IF NOT EXISTS idx_kchunk_doc  ON knowledge_chunks(doc_key);
CREATE TABLE IF NOT EXISTS knowledge_meta (
    k TEXT PRIMARY KEY,
    v TEXT NOT NULL
);
";

/// Инжест библиотеки в нативный индекс.
///
/// Идемпотентен: URL чанков стабильны (`poler://lib/<doc>/s<ord>/c<idx>`),
/// повторный прогон переиспользует upsert (не переиндексирует байто-идентичное).
/// Полнотекстовое русло — WebIndex (BM25+WebRank+zstd doc store), векторное —
/// RaBitQ+HNSW (файлы-спутники БД).
pub fn ingest(
    root: &Path,
    db_path: &Path,
    embedder: &mut KnowledgeEmbedder,
    opts: &IngestOptions,
) -> Result<IngestReport, String> {
    let t0 = Instant::now();
    let scan = scan_library(root)?;
    if scan.sections.is_empty() {
        return Err(format!(
            "в {:?} не найдено ни одной секции (ожидались слои 01_…06_)",
            root.display()
        ));
    }
    for w in &scan.warnings {
        eprintln!("poler-knowledge: предупреждение: {w}");
    }

    let mut ix = WebIndex::open(db_path)
        .map_err(|e| format!("open {}: {e}", db_path.display()))?;
    // Массовая сборка: WAL + нормальный синк — в разы быстрее дефолтного
    // journal+FULL на ~10⁴ транзакций upsert_page. Крах сборки → пересборка.
    ix.conn()
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
        .map_err(|e| format!("pragma: {e}"))?;
    // Свежие таблицы знаний + вычистка кусков предыдущего прогона.
    ix.conn()
        .execute_batch(
            "DROP TABLE IF EXISTS knowledge_chunks;
             DROP TABLE IF EXISTS knowledge_meta;",
        )
        .map_err(|e| format!("drop: {e}"))?;
    ix.conn()
        .execute_batch(SCHEMA)
        .map_err(|e| format!("schema: {e}"))?;
    ix.conn()
        .execute("DELETE FROM pages WHERE url LIKE 'poler://lib/%'", [])
        .map_err(|e| format!("чистка старых чанков: {e}"))?;

    let cfg = ChunkConfig {
        target_tokens: opts.target_tokens,
        overlap_tokens: opts.overlap_tokens,
        min_tokens: opts.min_tokens,
    };
    let mut seen: HashSet<u64> = HashSet::new();
    let mut rows: Vec<ChunkRow> = Vec::new();
    let mut deduped = 0usize;
    let mut total_tokens = 0usize;
    let mut by_provenance: BTreeMap<String, usize> = BTreeMap::new();

    for sec in &scan.sections {
        if sec.text.trim().is_empty() {
            continue;
        }
        if opts.dedupe {
            let h = fnv1a(sec.text.as_bytes());
            if !seen.insert(h) {
                deduped += 1;
                continue;
            }
        }
        let report = chunk_document(&sec.text, sec.format, &cfg);
        for ch in &report.chunks {
            if ch.tokens < MIN_CHUNK_TOKENS {
                continue;
            }
            let chunk_id = (rows.len() + 1) as i64;
            let url = format!(
                "{URL_PREFIX}{}/s{}/c{}",
                sec.doc_key, sec.ordinal, ch.index
            );
            let title = if sec.section_label.is_empty() {
                sec.title.clone()
            } else {
                format!("{} — {}", sec.title, sec.section_label)
            };
            let abs_byte_start = sec.byte_start + ch.byte_start;
            let abs_byte_end = sec.byte_start + ch.byte_end;
            let abs_line_start = sec.line_start + ch.line_start - 1;
            let abs_line_end = sec.line_start + ch.line_end - 1;
            // Преамбула/заголовок секции уже внутри текста чанка (срез
            // секции) — текст для эмбеддинга снабжаем титулом и следом
            // заголовков: вектор «знает», о чём кусок.
            let embed_text = if ch.breadcrumb.is_empty() {
                format!("{}\n{}", title, ch.text)
            } else {
                format!("{} › {}\n{}", title, ch.breadcrumb.join(" › "), ch.text)
            };
            rows.push(ChunkRow {
                chunk_id,
                url: url.clone(),
                doc_key: sec.doc_key.clone(),
                source_path: sec.source_path.clone(),
                origin_url: sec.origin_url.clone(),
                title,
                breadcrumb: ch.breadcrumb.join(" › "),
                provenance: sec.provenance,
                line_start: abs_line_start,
                line_end: abs_line_end,
                byte_start: abs_byte_start,
                byte_end: abs_byte_end,
                tokens: ch.tokens,
                text: embed_text,
            });
            total_tokens += ch.tokens;
            *by_provenance.entry(sec.provenance.as_str().to_string()).or_default() += 1;
        }
    }
    if rows.is_empty() {
        return Err("после чанкинга и дедупа не осталось ни одного чанка".into());
    }

    // ---- полнотекстовое русло: pages/terms через upsert_page ----
    eprintln!(
        "poler-knowledge: {} секций → {} чанков · индексация…",
        scan.sections.len(),
        rows.len()
    );
    for r in &rows {
        let doc = WebDoc {
            url: r.url.clone(),
            title: r.title.clone(),
            lang: "auto".to_string(),
            meta_description: r.breadcrumb.clone(),
            text: r.text.clone(),
            links: vec![],
            content_hash: content_hash(&r.text),
        };
        ix.upsert_page(&doc)
            .map_err(|e| format!("upsert {}: {e}", r.url))?;
        if rows.len() > PROGRESS_EVERY && (r.chunk_id as usize) % PROGRESS_EVERY == 0 {
            eprintln!("  индексация: {}/{}", r.chunk_id, rows.len());
        }
    }

    // ---- сайдкар: якоря, провенанс, цитаты ----
    ix.conn()
        .execute_batch("BEGIN")
        .map_err(|e| format!("begin: {e}"))?;
    {
        let mut stmt = ix
            .conn()
            .prepare(
                "INSERT OR REPLACE INTO knowledge_chunks
                 (chunk_id, url, doc_key, source_path, origin_url, title, breadcrumb,
                  provenance, line_start, line_end, byte_start, byte_end, tokens, text)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            )
            .map_err(|e| format!("prepare sidecar: {e}"))?;
        for r in &rows {
            stmt.execute(params![
                r.chunk_id,
                r.url,
                r.doc_key,
                r.source_path,
                r.origin_url,
                r.title,
                r.breadcrumb,
                r.provenance.as_str(),
                r.line_start as i64,
                r.line_end as i64,
                r.byte_start as i64,
                r.byte_end as i64,
                r.tokens as i64,
                r.text,
            ])
            .map_err(|e| format!("sidecar {}: {e}", r.chunk_id))?;
        }
    }
    ix.conn()
        .execute_batch("COMMIT")
        .map_err(|e| format!("commit: {e}"))?;

    // ---- векторное русло ----
    let mut vectors = 0usize;
    let mut vector_bytes = 0u64;
    let mut graph_bytes = 0u64;
    if !matches!(embedder, KnowledgeEmbedder::None) {
        eprintln!("poler-knowledge: векторный слой [{}]…", embedder.name());
        let texts: Vec<String> = rows.iter().map(|r| r.text.clone()).collect();
        let vecs = embedder.embed_corpus(&texts)?;
        let dim = vecs.first().map(|v| v.len()).unwrap_or(0);
        if dim == 0 {
            return Err("эмбеддер вернул пустые векторы".into());
        }
        let mut store = QuantizedStore::new(dim, VECTOR_ROT_SEED);
        let mut graph = HnswIndex::new(HnswConfig::default());
        for (i, v) in vecs.iter().enumerate() {
            let slot = store.push(rows[i].chunk_id as u32, v);
            graph.insert(&store, slot);
        }
        let vp = vectors_path(db_path);
        let gp = graph_path(db_path);
        store.save(&vp)?;
        graph.save(&gp)?;
        vectors = vecs.len();
        vector_bytes = std::fs::metadata(&vp).map(|m| m.len()).unwrap_or(0);
        graph_bytes = std::fs::metadata(&gp).map(|m| m.len()).unwrap_or(0);
    }

    // ---- мета ----
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let meta: Vec<(&str, String)> = vec![
        ("library_root", root.display().to_string()),
        ("built_at", now.to_string()),
        ("embedder", embedder.name().to_string()),
        ("dim", embedder.dim().to_string()),
        ("vectors", vectors.to_string()),
        ("chunk_count", rows.len().to_string()),
        ("section_count", scan.sections.len().to_string()),
        ("sections_deduped", deduped.to_string()),
        ("tokens", total_tokens.to_string()),
    ];
    {
        let mut stmt = ix
            .conn()
            .prepare("INSERT OR REPLACE INTO knowledge_meta(k, v) VALUES (?1, ?2)")
            .map_err(|e| format!("prepare meta: {e}"))?;
        for (k, v) in &meta {
            stmt.execute(params![k, v])
                .map_err(|e| format!("meta {k}: {e}"))?;
        }
    }

    let db_bytes = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
    Ok(IngestReport {
        library_root: root.display().to_string(),
        files: scan.files,
        sections: scan.sections.len(),
        sections_deduped: deduped,
        chunks: rows.len(),
        tokens: total_tokens,
        by_provenance,
        embedder: embedder.name().to_string(),
        vectors,
        vector_bytes,
        graph_bytes,
        db_bytes,
        db_path: db_path.display().to_string(),
        elapsed_ms: t0.elapsed().as_millis(),
    })
}

// ---------------------------------------------------------------------------
// Запрос: гибрид BM25 + векторы × провенанс
// ---------------------------------------------------------------------------

/// Настройки запроса.
#[derive(Debug, Clone)]
pub struct QueryOptions {
    /// Сколько хитов вернуть.
    pub top: usize,
    /// Минимальный эпистемический статус (None — без фильтра).
    pub min_provenance: Option<Provenance>,
    /// Пул кандидатов каждого русла (до слияния).
    pub candidate_pool: usize,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self { top: 10, min_provenance: None, candidate_pool: 64 }
    }
}

/// Хит выдачи гиппокампа.
#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeHit {
    pub chunk_id: i64,
    pub url: String,
    pub doc_key: String,
    pub source_path: String,
    pub origin_url: Option<String>,
    pub title: String,
    pub breadcrumb: String,
    pub provenance: Provenance,
    pub line_start: usize,
    pub line_end: usize,
    pub byte_start: usize,
    pub byte_end: usize,
    pub tokens: usize,
    /// Нормированный BM25/WebRank-скор русла (0…1; 0 — вектор-only хит).
    pub bm25: f64,
    /// ADC-косинус векторного русла (None — русло не участвовало/нет хита).
    pub vector: Option<f64>,
    /// Итог: (W_LX·bm25 + W_VEC·cos) × coefficient(provenance).
    pub final_score: f64,
    /// Подсвеченный сниппет WebIndex (если хит из BM25-русла).
    pub snippet: String,
    /// Прямая цитата (окно вокруг совпадения из текста чанка).
    pub quote: String,
}

/// Итог запроса.
#[derive(Debug, Clone)]
pub struct QueryOutcome {
    pub hits: Vec<KnowledgeHit>,
    /// WHY-строки Semantic Bridge (объяснимость расширения запроса).
    pub bridge_why: Vec<String>,
    /// Кандидатов BM25-русла после фильтра провенанса.
    pub bm25_candidates: usize,
    /// Кандидатов векторного русла.
    pub vector_candidates: usize,
    /// Работало ли векторное русло.
    pub vector_used: bool,
    /// Время запроса, мс.
    pub elapsed_ms: u128,
}

/// Мета чанка из сайдкара.
#[derive(Debug, Clone)]
struct ChunkMeta {
    chunk_id: i64,
    url: String,
    doc_key: String,
    source_path: String,
    origin_url: Option<String>,
    title: String,
    breadcrumb: String,
    provenance: Provenance,
    line_start: usize,
    line_end: usize,
    byte_start: usize,
    byte_end: usize,
    tokens: usize,
    text: String,
}

fn row_to_meta(r: &rusqlite::Row<'_>) -> rusqlite::Result<ChunkMeta> {
    let prov: String = r.get(7)?;
    Ok(ChunkMeta {
        chunk_id: r.get(0)?,
        url: r.get(1)?,
        doc_key: r.get(2)?,
        source_path: r.get(3)?,
        origin_url: r.get(4)?,
        title: r.get(5)?,
        breadcrumb: r.get(6)?,
        provenance: Provenance::parse(&prov).unwrap_or(Provenance::Narrative),
        line_start: r.get::<_, i64>(8)? as usize,
        line_end: r.get::<_, i64>(9)? as usize,
        byte_start: r.get::<_, i64>(10)? as usize,
        byte_end: r.get::<_, i64>(11)? as usize,
        tokens: r.get::<_, i64>(12)? as usize,
        text: r.get(13)?,
    })
}

const META_COLS: &str = "chunk_id, url, doc_key, source_path, origin_url, title, breadcrumb, \
                         provenance, line_start, line_end, byte_start, byte_end, tokens, text";

fn fetch_by_urls(conn: &Connection, urls: &[String]) -> Result<HashMap<String, ChunkMeta>, String> {
    let mut out = HashMap::new();
    if urls.is_empty() {
        return Ok(out);
    }
    let placeholders = urls.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT {META_COLS} FROM knowledge_chunks WHERE url IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("meta url: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(urls.iter()), |r| {
            let m = row_to_meta(r)?;
            Ok((m.url.clone(), m))
        })
        .map_err(|e| format!("meta url q: {e}"))?;
    for r in rows.flatten() {
        out.insert(r.0, r.1);
    }
    Ok(out)
}

fn fetch_by_ids(conn: &Connection, ids: &[i64]) -> Result<HashMap<i64, ChunkMeta>, String> {
    let mut out = HashMap::new();
    if ids.is_empty() {
        return Ok(out);
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT {META_COLS} FROM knowledge_chunks WHERE chunk_id IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("meta id: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            let m = row_to_meta(r)?;
            Ok((m.chunk_id, m))
        })
        .map_err(|e| format!("meta id q: {e}"))?;
    for r in rows.flatten() {
        out.insert(r.0, r.1);
    }
    Ok(out)
}

/// Окно-цитата: первое вхождение самого длинного слова запроса (без учёта
/// регистра), расширенное до `max_chars`; иначе — голова текста.
fn quote_window(text: &str, query: &str, max_chars: usize) -> String {
    let mut best: Option<usize> = None;
    let lower_text = text.to_lowercase();
    let mut words: Vec<&str> =
        query.split_whitespace().filter(|w| w.chars().count() >= 4).collect();
    words.sort_by_key(|w| std::cmp::Reverse(w.chars().count()));
    for w in &words {
        let wl = w.to_lowercase();
        let wl = wl.trim_matches(|c: char| !c.is_alphanumeric());
        if wl.is_empty() {
            continue;
        }
        if let Some(pos) = lower_text.find(wl) {
            best = Some(pos);
            break;
        }
    }
    let chars: Vec<char> = text.chars().collect();
    let (start, end) = match best {
        Some(byte_pos) => {
            let cp = text[..byte_pos].chars().count();
            let half = max_chars / 3;
            let s = cp.saturating_sub(half);
            let e = (cp + max_chars - half).min(chars.len());
            (s, e)
        }
        None => (0, max_chars.min(chars.len())),
    };
    let mut q: String = chars[start..end].iter().collect();
    q = q.trim().to_string();
    if q.chars().count() >= max_chars {
        q.push('…');
    }
    if start > 0 {
        q.insert(0, '…');
    }
    q.replace('\n', " ")
}

// ------------------------------------------------------------------
// M6: резидентный (тёплый) доступ без холодного старта на каждый запрос.
// `query_warm` переиспользует SQLite-соединение (кэш страниц FTS5),
// RaBitQ-хранилище и HNSW-граф.
// ------------------------------------------------------------------

/// Резидентное (тёплое) состояние Гиппокампа.
pub struct WarmKnowledge {
    /// Путь БД (для векторов/графа и диагностики).
    pub db_path: PathBuf,
    /// Открытый веб-индекс (SQLite, FTS5) — переиспользуется.
    pub ix: WebIndex,
    /// Квантованные векторы (открыты, если файлы присутствовали).
    pub view: Option<QuantizedStoreView>,
    /// HNSW-граф (открыт, если файл присутствовал).
    pub graph: Option<HnswIndex>,
    /// Эмбеддер запроса (hash-фолбэк или .pqw-модель) — держится
    /// открытым между запросами (mmap весов).
    pub embedder: Option<KnowledgeEmbedder>,
}

impl WarmKnowledge {
    /// Открыть все хранилища один раз. Ошибки векторов/графа не фатальны —
    /// русло деградирует до BM25 (та же семантика, что у холодного query).
    pub fn open(db_path: &Path, embedder: Option<KnowledgeEmbedder>) -> Result<Self, String> {
        if !db_path.exists() {
            return Err(format!(
                "БД знаний не найдена: {}. Сначала инжест: poler-engine --knowledge-ingest <корень POLER_ALL_GENERATED_DOCS>",
                db_path.display()
            ));
        }
        let ix = WebIndex::open(db_path)
            .map_err(|e| format!("open {}: {e}", db_path.display()))?;
        let vp = vectors_path(db_path);
        let gp = graph_path(db_path);
        let view = if vp.exists() { QuantizedStoreView::open(&vp).ok() } else { None };
        let graph = if gp.exists() { HnswIndex::open(&gp, HnswConfig::default()).ok() } else { None };
        Ok(Self { db_path: db_path.to_path_buf(), ix, view, graph, embedder })
    }
}

/// M6: запрос к Гиппокампу по тёплым хэндлам — без открытия SQLite и
/// загрузки векторных структур. Результат обязан совпадать с холодным
/// [`query`] (тест `warm_query_matches_cold`).
pub fn query_warm(
    wk: &mut WarmKnowledge,
    q: &str,
    opts: &QueryOptions,
) -> Result<QueryOutcome, String> {
    let embedder = wk.embedder.as_mut();
    query_core(&mut wk.ix, wk.view.as_ref(), wk.graph.as_ref(), &wk.db_path, q, opts, embedder)
}

/// Гибридный запрос по библиотеке знаний (холодный: открывает БД на вызов;
/// для резидентного сервера — [`query_warm`]).
///
/// Порядок детерминирован: сортировка по `final_score` (total_cmp), при
/// равенстве — по `chunk_id`. Векторное русло включается, если индекс
/// построен с векторным слоем и передан согласующийся `embedder`.
pub fn query(
    db_path: &Path,
    q: &str,
    opts: &QueryOptions,
    embedder: Option<&mut KnowledgeEmbedder>,
) -> Result<QueryOutcome, String> {
    if !db_path.exists() {
        return Err(format!(
            "БД знаний не найдена: {}. Сначала инжест: poler-engine --knowledge-ingest <корень POLER_ALL_GENERATED_DOCS>",
            db_path.display()
        ));
    }
    let mut ix = WebIndex::open(db_path)
        .map_err(|e| format!("open {}: {e}", db_path.display()))?;
    let vp = vectors_path(db_path);
    let gp = graph_path(db_path);
    let (view, graph) = if vp.exists() && gp.exists() {
        (
            QuantizedStoreView::open(&vp).ok(),
            HnswIndex::open(&gp, HnswConfig::default()).ok(),
        )
    } else {
        (None, None)
    };
    query_core(&mut ix, view.as_ref(), graph.as_ref(), db_path, q, opts, embedder)
}

/// Общее тело поиска: два русла (BM25+мост, векторы) и слияние с
/// провенансом. `t0` фиксируется здесь — единая честная метрика.
#[allow(clippy::too_many_arguments)]
fn query_core(
    ix: &mut WebIndex,
    view: Option<&QuantizedStoreView>,
    graph: Option<&HnswIndex>,
    db_path: &Path,
    q: &str,
    opts: &QueryOptions,
    embedder: Option<&mut KnowledgeEmbedder>,
) -> Result<QueryOutcome, String> {
    let t0 = Instant::now();
    if !db_path.exists() {
        return Err(format!(
            "БД знаний не найдена: {}. Сначала инжест: poler-engine --knowledge-ingest <корень POLER_ALL_GENERATED_DOCS>",
            db_path.display()
        ));
    }
    let bridge = SemanticBridge::offline();
    let pool = opts.candidate_pool.max(opts.top * 4).min(256);

    // ---- русло 1: BM25/WebRank + кросс-языковое расширение ----
    let (raw, expansion) = ix
        .search_with_bridge(q, pool, &bridge)
        .map_err(|e| format!("поиск: {e}"))?;
    let mut bm25_by_url: HashMap<String, (f64, String)> = HashMap::new();
    for h in raw {
        if h.url.starts_with(URL_PREFIX) {
            bm25_by_url.insert(h.url.clone(), (h.score, h.snippet.clone()));
        }
    }
    let mut metas = fetch_by_urls(ix.conn(), &bm25_by_url.keys().cloned().collect::<Vec<_>>())?;
    if let Some(min) = opts.min_provenance {
        metas.retain(|_, m| m.provenance >= min);
    }

    // ---- русло 2: векторы (RaBitQ + HNSW) ----
    // M6: хранилище и граф приходят ИНЪЕКЦИЕЙ (тёплые хэндлы резидентного
    // сервера); файловые проверки остаются guard'ом против гонки удаления.
    let vp = vectors_path(db_path);
    let gp = graph_path(db_path);
    let mut vector_used = false;
    let mut vec_by_id: HashMap<i64, f64> = HashMap::new();
    if let (Some(view), Some(graph)) = (view, graph) {
        if vp.exists() && gp.exists() {
            if let Some(emb) = embedder {
                match emb.embed_query(q) {
                    Ok(qv) => {
                        if view.count() > 0 && view.dim() == qv.len() {
                            let prep = view.prepare_query(&qv);
                            let k = pool.min(view.count());
                            let ef = pool.max(64);
                            for (cos, slot) in graph.search(view, &prep, k, ef) {
                                vec_by_id.insert(view.id(slot) as i64, cos.clamp(0.0, 1.0));
                            }
                            vector_used = true;
                        }
                    }
                    Err(e) => eprintln!("poler-knowledge: векторное русло пропущено: {e}"),
                }
            }
        }
    }
    if let Some(min) = opts.min_provenance {
        // добираем мета векторных кандидатов и фильтруем их тем же статусом
        let missing: Vec<i64> = vec_by_id
            .keys()
            .copied()
            .filter(|id| !metas.values().any(|m| m.chunk_id == *id))
            .collect();
        let extra = fetch_by_ids(ix.conn(), &missing)?;
        for (_, m) in extra {
            if m.provenance >= min {
                metas.insert(m.url.clone(), m);
            }
        }
        vec_by_id.retain(|id, _| metas.values().any(|m| m.chunk_id == *id));
    } else {
        let missing: Vec<i64> = vec_by_id
            .keys()
            .copied()
            .filter(|id| !metas.values().any(|m| m.chunk_id == *id))
            .collect();
        let extra = fetch_by_ids(ix.conn(), &missing)?;
        for (_, m) in extra {
            metas.insert(m.url.clone(), m);
        }
    }

    // ---- слияние × провенанс ----
    // нормировка: только по хитам, прошедшим фильтр провенанса
    let kept_scores: Vec<f64> = bm25_by_url
        .iter()
        .filter(|(u, _)| metas.contains_key(*u))
        .map(|(_, (s, _))| *s)
        .collect();
    let max_bm25 = kept_scores.iter().cloned().fold(0.0f64, f64::max).max(1e-12);

    struct Cand {
        meta: ChunkMeta,
        bm25: f64,
        vector: Option<f64>,
        snippet: String,
    }
    let mut cands: Vec<Cand> = Vec::new();
    let mut seen_ids: HashSet<i64> = HashSet::new();
    for (url, m) in &metas {
        if !seen_ids.insert(m.chunk_id) {
            continue;
        }
        let (bm25_norm, snippet) = bm25_by_url
            .get(url)
            .map(|(s, sn)| (s / max_bm25, sn.clone()))
            .unwrap_or((0.0, String::new()));
        cands.push(Cand {
            meta: m.clone(),
            bm25: bm25_norm,
            vector: vec_by_id.get(&m.chunk_id).copied(),
            snippet,
        });
    }
    let mut hits: Vec<KnowledgeHit> = cands
        .into_iter()
        .map(|c| {
            let base = if vector_used {
                W_LEXICAL * c.bm25 + W_VECTOR * c.vector.unwrap_or(0.0)
            } else {
                c.bm25
            };
            let final_score = base * c.meta.provenance.coefficient();
            KnowledgeHit {
                chunk_id: c.meta.chunk_id,
                url: c.meta.url,
                doc_key: c.meta.doc_key,
                source_path: c.meta.source_path,
                origin_url: c.meta.origin_url,
                title: c.meta.title,
                breadcrumb: c.meta.breadcrumb,
                provenance: c.meta.provenance,
                line_start: c.meta.line_start,
                line_end: c.meta.line_end,
                byte_start: c.meta.byte_start,
                byte_end: c.meta.byte_end,
                tokens: c.meta.tokens,
                bm25: c.bm25,
                vector: c.vector,
                final_score,
                snippet: c.snippet.clone(),
                quote: quote_window(&c.meta.text, q, 360),
            }
        })
        .collect();
    hits.sort_by(|a, b| {
        b.final_score
            .total_cmp(&a.final_score)
            .then(a.chunk_id.cmp(&b.chunk_id))
    });
    hits.truncate(opts.top.max(1));

    Ok(QueryOutcome {
        hits,
        bridge_why: expansion.why_lines(),
        bm25_candidates: kept_scores.len(),
        vector_candidates: vec_by_id.len(),
        vector_used,
        elapsed_ms: t0.elapsed().as_millis(),
    })
}

/// Рендер выдачи (CLI и MCP используют один формат — агент-читаемость).
pub fn render_query_text(q: &str, out: &QueryOutcome) -> String {
    let mut s = String::new();
    let mode = if out.vector_used { "гибрид BM25+векторы" } else { "BM25/WebRank" };
    s.push_str(&format!(
        "poler knowledge «{q}» · {} хитов · {} мс · режим: {}\n",
        out.hits.len(),
        out.elapsed_ms,
        mode
    ));
    if !out.bridge_why.is_empty() {
        s.push_str("semantic bridge WHY:\n");
        for l in out.bridge_why.iter().take(4) {
            s.push_str(&format!("  {l}\n"));
        }
    }
    if out.hits.is_empty() {
        s.push_str(
            "ничего не найдено — попробуй другие термы, снимите --min-provenance \
             или переиндексируйте библиотеку (--knowledge-ingest)\n",
        );
        return s;
    }
    for (i, h) in out.hits.iter().enumerate() {
        s.push_str(&format!(
            "\n{}. [{:.4}] {}\n",
            i + 1,
            h.final_score,
            h.provenance.badge()
        ));
        s.push_str(&format!("   {} — {}\n", h.doc_key, h.title));
        s.push_str(&format!(
            "   {} : строки {}-{} · байты {}-{}",
            h.source_path, h.line_start, h.line_end, h.byte_start, h.byte_end
        ));
        if let Some(u) = &h.origin_url {
            s.push_str(&format!(" · {}", u));
        }
        s.push('\n');
        let vec = match h.vector {
            Some(v) => format!("{v:.4}"),
            None => "—".to_string(),
        };
        s.push_str(&format!("   bm25 {:.4} · vec {} · токенов {}\n", h.bm25, vec, h.tokens));
        s.push_str(&format!("   «{}»\n", h.quote));
    }
    s
}

// ---------------------------------------------------------------------------
// Статистика и восстановление эмбеддера для запроса
// ---------------------------------------------------------------------------

/// Статистика индекса знаний (--knowledge-stats, JSON).
#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeStats {
    pub db_path: String,
    pub library_root: String,
    pub built_at_unix: i64,
    pub chunks: usize,
    pub sections: usize,
    pub tokens: usize,
    pub by_provenance: BTreeMap<String, usize>,
    pub embedder: String,
    pub vector_dim: usize,
    pub vectors: usize,
    pub vector_bytes: u64,
    pub graph_bytes: u64,
    pub db_bytes: u64,
}

/// Статистика индекса.
pub fn stats(db_path: &Path) -> Result<KnowledgeStats, String> {
    if !db_path.exists() {
        return Err(format!(
            "БД знаний не найдена: {} — сначала --knowledge-ingest",
            db_path.display()
        ));
    }
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("open: {e}"))?;
    let mut by_provenance = BTreeMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT provenance, COUNT(*) FROM knowledge_chunks GROUP BY provenance")
            .map_err(|e| format!("stats: {e}"))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize)))
            .map_err(|e| format!("stats q: {e}"))?;
        for (k, v) in rows.flatten() {
            by_provenance.insert(k, v);
        }
    }
    let get = |k: &str| -> String {
        conn.query_row(
            "SELECT v FROM knowledge_meta WHERE k = ?1",
            params![k],
            |r| r.get::<_, String>(0),
        )
        .unwrap_or_default()
    };
    let num = |k: &str| -> usize { get(k).parse().unwrap_or(0) };
    Ok(KnowledgeStats {
        db_path: db_path.display().to_string(),
        library_root: get("library_root"),
        built_at_unix: get("built_at").parse().unwrap_or(0),
        chunks: num("chunk_count"),
        sections: num("section_count"),
        tokens: num("tokens"),
        by_provenance,
        embedder: get("embedder"),
        vector_dim: num("dim"),
        vectors: num("vectors"),
        vector_bytes: std::fs::metadata(vectors_path(db_path)).map(|m| m.len()).unwrap_or(0),
        graph_bytes: std::fs::metadata(graph_path(db_path)).map(|m| m.len()).unwrap_or(0),
        db_bytes: std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0),
    })
}

/// Восстановить эмбеддер запроса по мета индекса.
///
/// * `none` → None (поиск остаётся чисто BM25);
/// * `hash-projection-*` → HashEmbedder той же размерности (сид фиксирован);
/// * `.pqw` → требует `--model`; без него вернётся ошибка-подсказка
///   (вызывающий может продолжить без векторного русла).
pub fn query_embedder(
    db_path: &Path,
    model: Option<&Path>,
) -> Result<Option<KnowledgeEmbedder>, String> {
    if !db_path.exists() {
        return Err(format!(
            "БД знаний не найдена: {} — сначала --knowledge-ingest",
            db_path.display()
        ));
    }
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("open: {e}"))?;
    let name: String = conn
        .query_row(
            "SELECT v FROM knowledge_meta WHERE k = 'embedder'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "none".to_string());
    let dim: usize = conn
        .query_row(
            "SELECT v FROM knowledge_meta WHERE k = 'dim'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(HASH_DIM);
    match name.as_str() {
        "none" => Ok(None),
        n if n.starts_with("hash") => {
            Ok(Some(KnowledgeEmbedder::Hash(HashEmbedder::new(dim.max(8), HASH_SEED))))
        }
        _ => {
            let p = model.ok_or(
                "индекс построен с .pqw-энкодером: укажи --model <path.pqw> \
                 (поиск продолжится без векторного русла)",
            )?;
            Ok(Some(KnowledgeEmbedder::Pqw(PqwEmbedder::open(p)?)))
        }
    }
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Мини-библиотека со всеми распознаваемыми слоями.
    fn fixture_library(dir: &Path) {
        let d1 = dir.join("01_SPECS");
        std::fs::create_dir_all(&d1).unwrap();
        std::fs::write(
            d1.join("PTS-001.md"),
            "# PTS-001: Первая спека\n\n## 1. ОБЗОР\nквантовый обзор зизлова.\n\n\
             ## 4. ПОЛНЫЙ ТЕХНИЧЕСКИЙ ТЕКСТ\nalpha secret words про зизлов механизм.\n\n\
             ## 5. РЕЕСТР\nшаблонный реестр бла бла.\n",
        )
        .unwrap();
        std::fs::write(
            d1.join("PTS-002.md"),
            "# PTS-002: Вторая спека\n\n## 1. ОБЗОР\nдругой обзор резонанса.\n\n\
             ## 4. ПОЛНЫЙ ТЕХНИЧЕСКИЙ ТЕКСТ\nbeta original text про резонанс поля.\n\n\
             ## 5. РЕЕСТР\nшаблонный реестр бла бла.\n",
        )
        .unwrap();
        let d2 = dir.join("02_THEMES");
        std::fs::create_dir_all(&d2).unwrap();
        std::fs::write(
            d2.join("07_CORE.md"),
            "# Тематический том\n\n## Синтез\nпересказ концепции зизлова без пруфов.\n",
        )
        .unwrap();
        let d3 = dir.join("03_TREATISE");
        std::fs::create_dir_all(&d3).unwrap();
        std::fs::write(
            d3.join("VOLUME_I_QUANTUM.md"),
            "# Том I\n\n## Теорема A\nзизлов принцип доказан машинно golden vector.\n",
        )
        .unwrap();
        std::fs::write(
            d3.join("VOLUME_VI_MANIFEST.md"),
            "# Том VI\n\n## Манифест\nзизлов принцип как метафора без верификации.\n",
        )
        .unwrap();
        let d4 = dir.join("04_CIPHER");
        std::fs::create_dir_all(&d4).unwrap();
        std::fs::write(
            d4.join("kernel.zig"),
            "const std = @import(\"std\");\npub fn mix(x: u32) u32 {\n    return x ^ 0x9E3779B9;\n}\n",
        )
        .unwrap();
        let d6 = dir.join("06_DB");
        std::fs::create_dir_all(&d6).unwrap();
        let conn = Connection::open(d6.join("chopocompik.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE videos(id TEXT, type TEXT, title TEXT, url TEXT, full_text TEXT, file_path TEXT);
             INSERT INTO videos VALUES ('v1','video','Доклад','https://youtu.be/v1',
               'транскрипт доклада про зизлов и alpha secret','/t/v1.md');",
        )
        .unwrap();
    }

    #[test]
    fn provenance_parse_order_coefficients() {
        assert!(Provenance::MvrVerified > Provenance::SourceDocument);
        assert!(Provenance::SourceDocument > Provenance::Narrative);
        assert_eq!(Provenance::MvrVerified.coefficient(), 1.5);
        assert_eq!(Provenance::SourceDocument.coefficient(), 1.0);
        assert_eq!(Provenance::Narrative.coefficient(), 0.7);
        assert_eq!(Provenance::parse("mvr"), Some(Provenance::MvrVerified));
        assert_eq!(
            Provenance::parse("source_document"),
            Some(Provenance::SourceDocument)
        );
        assert_eq!(Provenance::parse("NARRATIVE"), Some(Provenance::Narrative));
        assert_eq!(Provenance::parse("мусор"), None);
        // serde-имена совпадают с as_str
        let j = serde_json::to_string(&Provenance::MvrVerified).unwrap();
        assert!(j.contains("mvr_verified"), "{j}");
    }

    #[test]
    fn scan_maps_layers_to_provenance() {
        let dir = tempfile::tempdir().unwrap();
        fixture_library(dir.path());
        let out = scan_library(dir.path()).unwrap();
        assert!(out.files >= 7, "файлов: {}", out.files);
        let get = |doc: &str| {
            out.sections
                .iter()
                .filter(|s| s.doc_key == doc)
                .map(|s| (s.ordinal, s.provenance, s.section_label.clone()))
                .collect::<Vec<_>>()
        };
        // PTS: секция 4 — первоисточник, остальные — нарратив
        let pts = get("PTS-001");
        assert_eq!(pts.len(), 4, "секции PTS-001: {pts:?}");
        let sec4 = pts.iter().find(|(o, _, _)| *o == 2).unwrap();
        assert_eq!(sec4.1, Provenance::SourceDocument);
        assert!(sec4.2.contains("ПОЛНЫЙ"));
        assert_eq!(pts[0].1, Provenance::Narrative);
        // трактат: I — MVR, VI — нарратив
        assert_eq!(get("TREATISE-I")[0].1, Provenance::MvrVerified);
        assert_eq!(get("TREATISE-VI")[0].1, Provenance::Narrative);
        // тематический том — нарратив; код шифра и транскрипт — первоисточники
        assert_eq!(get("THEME-07_CORE")[0].1, Provenance::Narrative);
        assert_eq!(get("CIPHER-kernel")[0].1, Provenance::SourceDocument);
        let vid = get("VIDEO-v1");
        assert_eq!(vid.len(), 1);
        assert_eq!(vid[0].1, Provenance::SourceDocument);
        let vid_sec = out.sections.iter().find(|s| s.doc_key == "VIDEO-v1").unwrap();
        assert_eq!(vid_sec.origin_url.as_deref(), Some("https://youtu.be/v1"));
    }

    #[test]
    fn ingest_dedups_identical_sections() {
        let dir = tempfile::tempdir().unwrap();
        fixture_library(dir.path());
        let mut emb = KnowledgeEmbedder::None;
        let db = dir.path().join("k.db");
        let rep = ingest(dir.path(), &db, &mut emb, &IngestOptions::default()).unwrap();
        // секции «5. РЕЕСТР» байт-идентичны у PTS-001/PTS-002 → один дедуп
        assert_eq!(rep.sections_deduped, 1, "дедуп: {}", rep.sections_deduped);
        assert!(rep.chunks >= 7, "чанков: {}", rep.chunks);
        assert!(rep.by_provenance.contains_key("mvr_verified"));
        assert!(rep.by_provenance.contains_key("source_document"));
        assert!(rep.by_provenance.contains_key("narrative"));
        assert_eq!(rep.vectors, 0, "без эмбеддера векторов нет");
    }

    #[test]
    fn ingest_query_roundtrip_with_hash_vectors() {
        let dir = tempfile::tempdir().unwrap();
        fixture_library(dir.path());
        let mut emb = KnowledgeEmbedder::from_mode("hash", None).unwrap();
        let db = dir.path().join("k.db");
        let rep = ingest(dir.path(), &db, &mut emb, &IngestOptions::default()).unwrap();
        assert_eq!(rep.vectors, rep.chunks);
        assert!(vectors_path(&db).exists());
        assert!(graph_path(&db).exists());

        // запрос с хэш-векторами: полное гибридное русло
        let mut qe = query_embedder(&db, None).unwrap().unwrap();
        let out = query(
            &db,
            "alpha secret",
            &QueryOptions { top: 5, ..Default::default() },
            Some(&mut qe),
        )
        .unwrap();
        assert!(out.vector_used, "векторное русло обязано работать");
        assert!(!out.hits.is_empty());
        // Ранг №1 для хэш-проекции не гарантирован (короткий транскрипт
        // легитимно выигрывает по косинусу) — контракт roundtrip'а:
        // хит PTS-001 присутствует и несёт верные якоря/провенанс/вектор.
        let pts = out
            .hits
            .iter()
            .find(|h| h.doc_key == "PTS-001")
            .expect("PTS-001 обязан находиться в top-5");
        assert_eq!(pts.provenance, Provenance::SourceDocument);
        assert!(pts.url.starts_with("poler://lib/PTS-001/s2/"), "url: {}", pts.url);
        assert!(pts.line_start >= 1);
        assert!(pts.quote.to_lowercase().contains("alpha") || pts.quote.contains("зизлов"));
        assert!(pts.final_score > 0.0);
        assert!(pts.vector.is_some(), "векторное русло дало оценку PTS-001");

        // транскрипт тоже находится (русло БД chopocompik)
        let out2 = query(
            &db,
            "транскрипт доклада",
            &QueryOptions { top: 10, ..Default::default() },
            Some(&mut qe),
        )
        .unwrap();
        assert!(
            out2.hits.iter().any(|h| h.doc_key == "VIDEO-v1"),
            "транскрипт не найден: {:?}",
            out2.hits.iter().map(|h| h.doc_key.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn min_provenance_filters_narrative() {
        let dir = tempfile::tempdir().unwrap();
        fixture_library(dir.path());
        let mut emb = KnowledgeEmbedder::None;
        let db = dir.path().join("k.db");
        ingest(dir.path(), &db, &mut emb, &IngestOptions::default()).unwrap();

        let out = query(
            &db,
            "зизлов",
            &QueryOptions {
                top: 10,
                min_provenance: Some(Provenance::MvrVerified),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        assert!(!out.hits.is_empty(), "MVR-хиты обязаны быть (том I)");
        assert!(
            out.hits.iter().all(|h| h.provenance >= Provenance::MvrVerified),
            "фильтр пропустил нарратив"
        );
        assert!(out.hits.iter().all(|h| h.doc_key.starts_with("TREATISE-")));
        // без фильтра — и нарратив находится
        let all = query(&db, "зизлов", &QueryOptions { top: 10, ..Default::default() }, None)
            .unwrap();
        assert!(all.hits.len() > out.hits.len());
    }

    #[test]
    fn mvr_boost_ranks_verified_above_narrative() {
        let dir = tempfile::tempdir().unwrap();
        // два документа с ОДИНАКОВЫМ телом: MVR-том и нарративный том VI
        let d = dir.path().join("03_TREATISE");
        std::fs::create_dir_all(&d).unwrap();
        let body = "# T\n\n## S\nзизлов механизм резонанса квантовый обзор.\n";
        std::fs::write(d.join("VOLUME_I_A.md"), body).unwrap();
        std::fs::write(d.join("VOLUME_VI_B.md"), body).unwrap();

        let mut emb = KnowledgeEmbedder::None;
        let db = dir.path().join("k.db");
        // dedupe=false: тела намеренно байт-идентичны (равный BM25) —
        // дедуп съел бы второй документ и тест буста потерял бы смысл.
        let opts = IngestOptions { dedupe: false, ..Default::default() };
        ingest(dir.path(), &db, &mut emb, &opts).unwrap();
        let out = query(
            &db,
            "зизлов механизм",
            &QueryOptions { top: 2, ..Default::default() },
            None,
        )
        .unwrap();
        assert_eq!(out.hits.len(), 2);
        assert_eq!(out.hits[0].provenance, Provenance::MvrVerified, "MVR должен быть первым");
        assert_eq!(out.hits[0].doc_key, "TREATISE-I");
        assert!(out.hits[0].final_score > out.hits[1].final_score);
    }

    #[test]
    fn missing_db_gives_actionable_error() {
        let e = query(
            Path::new("/nonexistent/knowledge.db"),
            "x",
            &QueryOptions::default(),
            None,
        )
        .unwrap_err();
        assert!(e.contains("--knowledge-ingest"), "подсказка инжеста: {e}");
    }
}




#[cfg(test)]
mod m6_tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("poler-wk-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// M6: тёплый запрос обязан давать ТОТ ЖЕ результат, что холодный
    /// (оба русла, слияние, провенанс), без пере-открытия хранилищ.
    #[test]
    fn warm_query_matches_cold() {
        let dir = tmpdir("same");
        let spec = dir.join("01_SPECS");
        std::fs::create_dir_all(&spec).unwrap();
        std::fs::write(
            spec.join("PTS-900.md"),
            "# PTS-900\n\n## 4. ПОЛНЫЙ ТЕХНИЧЕСКИЙ ТЕКСТ\nрезонансный контур pndMix в золотом векторе.\n",
        )
        .unwrap();
        let kdb = dir.join("knowledge.db");
        let mut emb = KnowledgeEmbedder::None;
        ingest(&dir, &kdb, &mut emb, &IngestOptions::default()).unwrap();

        let opts = QueryOptions { top: 5, ..QueryOptions::default() };
        let cold = query(&kdb, "резонансный контур pndMix", &opts, None).unwrap();

        let mut warm = WarmKnowledge::open(&kdb, None).unwrap();
        let w1 = query_warm(&mut warm, "резонансный контур pndMix", &opts).unwrap();
        assert_eq!(cold.hits.len(), w1.hits.len(), "число хитов совпадает");
        for (c, w) in cold.hits.iter().zip(w1.hits.iter()) {
            assert_eq!(c.chunk_id, w.chunk_id);
            assert_eq!(c.final_score.to_bits(), w.final_score.to_bits(), "скор побитово");
            assert_eq!(c.quote, w.quote, "цитаты совпадают");
        }
        assert_eq!(cold.bridge_why, w1.bridge_why);

        // Повторный тёплый запрос по тому же хэндлу стабилен.
        let w2 = query_warm(&mut warm, "резонансный контур pndMix", &opts).unwrap();
        assert_eq!(w1.hits.len(), w2.hits.len());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// M6: WarmKnowledge честно ошибается на отсутствующей БД.
    #[test]
    fn warm_open_missing_db_errors() {
        let e = match WarmKnowledge::open(Path::new("/nonexistent/warm.db"), None) {
            Err(e) => e,
            Ok(_) => panic!("отсутствующая БД обязана давать ошибку, не Ok"),
        };
        assert!(e.contains("--knowledge-ingest"), "подсказка: {e}");
    }
}
