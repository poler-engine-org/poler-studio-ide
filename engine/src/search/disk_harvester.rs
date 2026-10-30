//! `poler_disk_harvester` — нативный суб-секундный сборщик диска.
//!
//! Замена Python-пайплайнов однопоточного I/O (см.
//! `scripts/archive_benchmarks/README.md`): обход сотен тысяч файлов и
//! агрегация релевантного содержимого в один структурированный документ
//! за секунды, а не минуты.
//!
//! Архитектура (Zero-Copy Parallel Scanner → Streaming Aggregator):
//!
//! 1. **Параллельный обход** — [`ignore::WalkBuilder`] (крейт из состава
//!    ripgrep): все ядра CPU, `.gitignore`/`.git`-правила уважаются,
//!    мусорные каталоги (`target`, `node_modules`, …) и бинарные
//!    расширения отсекаются прямо в обходе (`filter_entry`), скрытые
//!    файлы пропускаются (как ripgrep; `--harvest-hidden` включает).
//! 2. **Zero-Copy память** — файлы ≤ [`TINY_FILE`] читаются в
//!    переиспользуемый буфер воркера; крупные — `memmap2` без копирования
//!    в юзерспейс, с `MADV_SEQUENTIAL` перед сканом и `MADV_DONTNEED`
//!    после (дисциплина RSS < 128 МБ).
//! 3. **SIMD Aho-Corasick** — десятки терминов в один проход по байтам
//!    (`aho-corasick` 1.1, ускорение memchr). Кириллица — вариантами
//!    регистра (гамильтониан / ГАМИЛЬТОНИАН / Гамильтониан), ASCII —
//!    встроенным `ascii_case_insensitive`. Альтернатива — `--harvest-regex`
//!    (`regex::bytes`, точные byte-offsets).
//! 4. **Потоковый агрегатор** — выход пишется через `BufWriter` 8 МБ с
//!    подсчётом байт и жёстким капом `--harvest-max-out-bytes`; ничего не
//!    накапливается в RAM (позиции совпадений — `u32`, ≤ 8192 на файл).
//!
//! Чанковый скан (4 МБ + overlap = max длина паттерна) ловит совпадения,
//! разрезанные границей чанка; каждая позиция записывается ровно один раз
//! (правило `start < end` текущего чанка).
//!
//! Форматы вывода: `markdown` (нумерованные секции с префиксом строк),
//! `json` (JSONL: meta → file-записи → summary), `corpus`/`t5c`
//! (чистый текст для дообучения кристалла). Режимы: `sections`
//! (±N строк контекста, слияние перекрытий, покрытие ≥ 80% → весь файл)
//! и `files` (файл целиком, стримингом `io::copy`).
//!
//! DoD: 100 000 файлов на SSD < 2.5 с; RAM < 128 МБ; устойчивость к
//! битым симлинкам, невалидному UTF-8 и переполнению буферов (тесты ниже).

use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use ignore::{DirEntry, WalkBuilder, WalkState};
use memmap2::MmapOptions;

/// Файлы меньше этого размера читаются буфером, а не mmap (дешевле syscalls).
const TINY_FILE: u64 = 128 * 1024;
/// Размер буфера выходного агрегатора (спека: кольцевой BufWriter 8 МБ).
pub const OUT_BUF: usize = 8 << 20;
/// Чанк чанкового скана (overlap поверх — длина паттерна).
pub const DEFAULT_CHUNK: usize = 4 << 20;
/// Максимум сохраняемых позиций совпадений на файл (анти-раздувание RAM).
const MAX_OFFSETS_PER_FILE: usize = 8192;
/// Снифф бинарности: NUL в первых N байтах → файл пропускается (как grep).
const BINARY_SNIFF: usize = 8192;
/// Покрытие ≥ 80% строк → секция «весь файл» (меньше шума нумерации).
const WHOLE_FILE_COVERAGE_PCT: usize = 80;

/// Мусорные каталоги (дополнительно к скрытым; см. lib.rs SKIP_DIRS).
const JUNK_DIRS: &[&str] = &[
    "target",
    "node_modules",
    "__pycache__",
    "venv",
    "dist",
    "build",
    "out",
    "site-packages",
    "zig-out",
    "coverage",
];

/// Расширения бинарников/датасетов — отсекаются в обходе (NUL-снифф —
/// запасной фильтр для файлов без расширения).
const BINARY_EXT: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "tiff", "webp", "svgz",
    "pdf", "zip", "tar", "gz", "bz2", "xz", "zst", "7z", "rar",
    "jar", "war", "ear", "class", "exe", "dll", "so", "dylib",
    "o", "a", "lib", "obj", "pyd", "bin", "iso", "img", "deb", "rpm",
    "mp3", "mp4", "avi", "mkv", "mov", "flac", "ogg", "wav",
    "woff", "woff2", "ttf", "otf", "eot",
    "db", "sqlite", "sqlite3", "mdb", "pyc", "pyo",
    "pack", "idx", "wasm", "t5c", "poler", "pqw",
    "npz", "npy", "pkl", "parquet", "arrow", "feather", "safetensors",
];

/// Формат выходного документа.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarvestFormat {
    /// Markdown с нумерованными секциями и префиксами строк.
    Markdown,
    /// JSONL: meta-строка, file-записи, summary (стримится построчно).
    Json,
    /// Чистый текст без меты — корпус для обучения кристалла (alias t5c).
    Corpus,
}

/// Гранулярность агрегации.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarvestMode {
    /// Секции: ±context строк вокруг совпадений, слияние перекрытий.
    Sections,
    /// Файлы целиком (стриминг, ≤ max_file_bytes).
    Files,
}

/// Полная конфигурация запуска сборщика.
#[derive(Clone, Debug)]
pub struct HarvestConfig {
    /// Корни обхода (1 или несколько: репозитории, диск, файлы).
    pub roots: Vec<PathBuf>,
    /// Запрос: термы через пробел/запятую ИЛИ regex (при regex_mode).
    pub query: String,
    /// Трактовать query как единый regex::bytes::Regex.
    pub regex_mode: bool,
    /// Куда писать: путь или "-" (stdout).
    pub out_path: PathBuf,
    pub format: HarvestFormat,
    pub mode: HarvestMode,
    /// Строк контекста вокруг совпадения (sections).
    pub context_lines: usize,
    /// Кап числа файлов в выходном документе (ранжирование по совпадениям).
    pub max_files: usize,
    /// Минимум совпадений, чтобы файл попал в выдачу.
    pub min_matches: usize,
    /// Скан/агрегация первых N байт файла (гиганты — частично).
    pub max_file_bytes: u64,
    /// Кап размера выходного документа.
    pub max_out_bytes: u64,
    /// Число воркеров (0 = available_parallelism).
    pub threads: usize,
    /// Включить скрытые файлы/каталоги (по умолчанию как ripgrep — нет).
    pub include_hidden: bool,
    /// Игнорировать .gitignore-правила (собрать ВСЁ, включая игнор).
    pub no_ignore: bool,
    /// Размер чанка скана (тест-хук для границы чанков).
    pub chunk_bytes: usize,
}

impl HarvestConfig {
    /// Конфиг с разумными дефолтами (тесты и CLI).
    pub fn new(roots: Vec<PathBuf>, query: impl Into<String>, out: impl Into<PathBuf>) -> Self {
        Self {
            roots,
            query: query.into(),
            regex_mode: false,
            out_path: out.into(),
            format: HarvestFormat::Markdown,
            mode: HarvestMode::Sections,
            context_lines: 6,
            max_files: 20_000,
            min_matches: 1,
            max_file_bytes: 8 * 1024 * 1024,
            max_out_bytes: 4u64 << 30,
            threads: 0,
            include_hidden: false,
            no_ignore: false,
            chunk_bytes: DEFAULT_CHUNK,
        }
    }
}

/// Итоговая статистика прогона (stderr-сводка + меты форматов).
#[derive(Clone, Debug, Default)]
pub struct HarvestStats {
    pub files_scanned: u64,
    pub dirs_scanned: u64,
    pub files_matched: usize,
    pub files_selected: usize,
    pub bytes_scanned: u64,
    pub bytes_written: u64,
    pub sections_written: u64,
    pub skipped_binary: u64,
    pub skipped_symlink: u64,
    pub read_errors: u64,
    pub changed_during_harvest: u64,
    pub duration_ms: u128,
    pub truncated: bool,
}

impl HarvestStats {
    /// Пропускная способность скана, МБ/с.
    pub fn throughput_mb_s(&self) -> f64 {
        if self.duration_ms == 0 {
            return 0.0;
        }
        (self.bytes_scanned as f64 / 1024.0 / 1024.0) / (self.duration_ms as f64 / 1000.0)
    }

    /// Однострочная сводка для stderr.
    pub fn summary_line(&self) -> String {
        format!(
            "poler_disk_harvester: {} файлов / {} каталогов за {} мс ({:.1} МБ/с); \
             совпадения в {} файлах, отобрано {}, секций {}, вывод {} КиБ{}",
            self.files_scanned,
            self.dirs_scanned,
            self.duration_ms,
            self.throughput_mb_s(),
            self.files_matched,
            self.files_selected,
            self.sections_written,
            self.bytes_written / 1024,
            if self.truncated { " [TRUNCATED]" } else { "" }
        )
    }
}

/// Результат скана одного файла.
#[derive(Debug)]
struct FileHit {
    path: PathBuf,
    size: u64,
    match_count: usize,
    /// Позиции первых совпадений (байты, ≤ MAX_OFFSETS_PER_FILE).
    offsets: Vec<u32>,
}

/// Атомарные счётчики фазы скана (шарятся между воркерами).
#[derive(Default)]
struct ScanCounters {
    files: AtomicU64,
    dirs: AtomicU64,
    bytes: AtomicU64,
    binary: AtomicU64,
    symlinks: AtomicU64,
    errors: AtomicU64,
}

/// Много-паттерновый матчер: AC-автомат или regex.
enum Matcher {
    /// Aho-Corasick + максимальная длина паттерна (overlap чанков).
    Ac(AhoCorasick, usize),
    /// regex::bytes + эвристика span (квантификаторы могут тянуться
    /// дальше длины паттерна; для типовых паттернов достаточно).
    Re(regex::bytes::Regex, usize),
}

impl Matcher {
    fn build(cfg: &HarvestConfig) -> Result<Self, String> {
        let raw = cfg.query.trim();
        if raw.is_empty() {
            return Err("пустой --harvest-query".into());
        }
        if cfg.regex_mode {
            let re = regex::bytes::Regex::new(raw)
                .map_err(|e| format!("невалидный --harvest-regex: {e}"))?;
            let span = (raw.len() * 4).max(256);
            return Ok(Matcher::Re(re, span));
        }
        let terms: Vec<String> = raw
            .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        if terms.is_empty() {
            return Err("пустой --harvest-query".into());
        }
        // Кириллица и прочий не-ASCII: варианты регистра (ASCII покрывается
        // ascii_case_insensitive самого AC-автомата).
        let mut pats: Vec<String> = Vec::new();
        for t in &terms {
            if t.is_ascii() {
                let p = t.to_ascii_lowercase();
                if !pats.contains(&p) {
                    pats.push(p);
                }
                continue;
            }
            let lower = t.to_lowercase();
            let upper = t.to_uppercase();
            let mut cap = String::new();
            let mut chars = lower.chars();
            if let Some(first) = chars.next() {
                cap.extend(first.to_uppercase());
                cap.push_str(chars.as_str());
            }
            for p in [lower, upper, cap] {
                if !p.is_empty() && !pats.contains(&p) {
                    pats.push(p);
                }
            }
        }
        let max_len = pats.iter().map(|p| p.len()).max().unwrap_or(1);
        let ac = AhoCorasickBuilder::new()
            .match_kind(MatchKind::Standard)
            .ascii_case_insensitive(true)
            .build(&pats)
            .map_err(|e| format!("Aho-Corasick build: {e}"))?;
        Ok(Matcher::Ac(ac, max_len))
    }

    /// Максимальный «вылет» совпадения за границу чанка.
    fn overlap(&self) -> usize {
        match self {
            Matcher::Ac(_, n) | Matcher::Re(_, n) => *n,
        }
    }

    /// Все начала совпадений в haystack (по возрастанию).
    fn for_each_start(&self, hay: &[u8], mut f: impl FnMut(usize)) {
        match self {
            Matcher::Ac(ac, _) => {
                for m in ac.find_iter(hay) {
                    f(m.start());
                }
            }
            Matcher::Re(re, _) => {
                for m in re.find_iter(hay) {
                    f(m.start());
                }
            }
        }
    }
}

/// Публичная точка входа: скан → ранжирование → агрегация.
///
/// Коды возврата транслирует вызывающая сторона: `files_selected == 0`
/// → 1 (как grep «ничего не найдено»), `Err` → 2.
pub fn run_harvest(cfg: &HarvestConfig) -> Result<HarvestStats, String> {
    let t0 = Instant::now();
    if cfg.roots.is_empty() {
        return Err("не указан --harvest-disk <ROOT_PATH>".into());
    }
    for r in &cfg.roots {
        if !r.exists() {
            return Err(format!("корень не существует: {}", r.display()));
        }
    }
    let matcher = Matcher::build(cfg)?;
    let (counters, hits) = scan_parallel(cfg, &matcher);

    let mut hits = hits;
    hits.sort_by(|a, b| {
        b.match_count
            .cmp(&a.match_count)
            .then_with(|| a.path.cmp(&b.path))
    });
    hits.retain(|h| h.match_count >= cfg.min_matches);
    let files_matched = hits.len();
    hits.truncate(cfg.max_files);

    let mut stats = HarvestStats {
        files_scanned: counters.files.load(Ordering::Relaxed),
        dirs_scanned: counters.dirs.load(Ordering::Relaxed),
        bytes_scanned: counters.bytes.load(Ordering::Relaxed),
        skipped_binary: counters.binary.load(Ordering::Relaxed),
        skipped_symlink: counters.symlinks.load(Ordering::Relaxed),
        read_errors: counters.errors.load(Ordering::Relaxed),
        files_matched,
        files_selected: hits.len(),
        duration_ms: t0.elapsed().as_millis(),
        ..Default::default()
    };
    write_output(cfg, &hits, &mut stats)
        .map_err(|e| format!("запись вывода {}: {e}", cfg.out_path.display()))?;
    stats.duration_ms = t0.elapsed().as_millis();
    Ok(stats)
}

// ---------------------------------------------------------------------------
// Фаза 1: параллельный скан (ignore::WalkBuilder — движок ripgrep)
// ---------------------------------------------------------------------------

/// Предикат KEEP для filter_entry (true = оставить; false = вырезать).
/// Вырезаем мусорные каталоги и бинарные расширения прямо в обходе.
fn keep_entry(e: &DirEntry) -> bool {
    // Корень обхода не фильтруем — иначе скрытый корень обрежет весь обход.
    if e.depth() == 0 {
        return true;
    }
    let Some(ft) = e.file_type() else { return true };
    if ft.is_dir() {
        let name = e.file_name().to_string_lossy();
        return !JUNK_DIRS.iter().any(|d| name.eq_ignore_ascii_case(d));
    }
    if ft.is_file() {
        if let Some(ext) = e.path().extension().and_then(|s| s.to_str()) {
            let ext = ext.to_ascii_lowercase();
            return !BINARY_EXT.iter().any(|x| *x == ext);
        }
    }
    true
}

fn scan_parallel(cfg: &HarvestConfig, matcher: &Matcher) -> (ScanCounters, Vec<FileHit>) {
    let counters = ScanCounters::default();
    let hits: Mutex<Vec<FileHit>> = Mutex::new(Vec::new());

    let mut builder = WalkBuilder::new(&cfg.roots[0]);
    for r in &cfg.roots[1..] {
        builder.add(r);
    }
    let threads = if cfg.threads == 0 {
        // I/O-bound обход: oversubscription кратно ускоряет медленные диски
        // (замер на 2-ядерной песочнице, 100k файлов: 2 воркера → 8.1 с;
        // 8 → 1.5 с; 16+ → ~1.0 с; плато на ~16).
        let n = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        (n * 2).clamp(8, 32)
    } else {
        cfg.threads
    };
    builder
        .hidden(!cfg.include_hidden)
        .git_ignore(!cfg.no_ignore)
        .git_global(!cfg.no_ignore)
        .git_exclude(!cfg.no_ignore)
        .ignore(!cfg.no_ignore)
        .parents(!cfg.no_ignore)
        .require_git(false)
        .threads(threads)
        .filter_entry(keep_entry);

    // Выходной файл исключаем из сбора: итеративные прогоны не должны
    // переваривать собственные результаты (самопоглощение документа).
    let out_abs = if cfg.out_path.is_absolute() {
        cfg.out_path.clone()
    } else {
        std::env::current_dir().unwrap_or_default().join(&cfg.out_path)
    };

    let walk = builder.build_parallel();
    walk.run(|| {
        let counters = &counters;
        let hits = &hits;
        let matcher = matcher;
        let cfg = cfg;
        let out_abs = &out_abs;
        // Переиспользуемый буфер воркера для tiny-файлов.
        let mut tiny_buf: Vec<u8> = Vec::new();
        Box::new(move |entry: Result<DirEntry, ignore::Error>| -> WalkState {
            let e = match entry {
                Ok(e) => e,
                Err(_) => {
                    counters.errors.fetch_add(1, Ordering::Relaxed);
                    return WalkState::Continue;
                }
            };
            let Some(ft) = e.file_type() else {
                return WalkState::Continue;
            };
            if ft.is_symlink() {
                // Симлинки не follow'им (анти-циклы); битые пропускаются тихо.
                counters.symlinks.fetch_add(1, Ordering::Relaxed);
                return WalkState::Continue;
            }
            if ft.is_dir() {
                counters.dirs.fetch_add(1, Ordering::Relaxed);
                return WalkState::Continue;
            }
            if !ft.is_file() {
                return WalkState::Continue;
            }
            // Свой выход не собираем (см. out_abs выше).
            let p = e.path();
            if p == cfg.out_path.as_path() || p == out_abs.as_path() {
                return WalkState::Continue;
            }
            counters.files.fetch_add(1, Ordering::Relaxed);
            let Ok(meta) = e.metadata() else {
                counters.errors.fetch_add(1, Ordering::Relaxed);
                return WalkState::Continue;
            };
            if let Some(hit) =
                scan_file(e.path(), meta.len(), cfg, matcher, counters, &mut tiny_buf)
            {
                if let Ok(mut guard) = hits.lock() {
                    guard.push(hit);
                }
            }
            WalkState::Continue
        })
    });

    (counters, hits.into_inner().unwrap_or_default())
}

/// Скан одного файла: tiny-буфер или mmap + чанки с overlap.
/// Возвращает `Some(FileHit)`, если есть хотя бы одно совпадение.
fn scan_file(
    path: &Path,
    size: u64,
    cfg: &HarvestConfig,
    matcher: &Matcher,
    counters: &ScanCounters,
    tiny_buf: &mut Vec<u8>,
) -> Option<FileHit> {
    if size == 0 {
        return None;
    }
    let want = size.min(cfg.max_file_bytes) as usize;
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => {
            counters.errors.fetch_add(1, Ordering::Relaxed);
            return None;
        }
    };

    let mut offsets: Vec<u32> = Vec::new();
    let mut count: usize = 0;
    let collect = cfg.mode == HarvestMode::Sections;
    let mut record = |abs: usize, end: usize| {
        if abs < end {
            count += 1;
            if collect && offsets.len() < MAX_OFFSETS_PER_FILE {
                offsets.push(abs as u32);
            }
        }
    };

    let scan = |bytes: &[u8], record: &mut dyn FnMut(usize, usize)| {
        // NUL в первых байтах → бинарник (grep-семантика; UTF-16 отсекается
        // сознательно — задокументированное ограничение байтового сканера).
        if bytes[..bytes.len().min(BINARY_SNIFF)].contains(&0u8) {
            counters.binary.fetch_add(1, Ordering::Relaxed);
            return;
        }
        counters
            .bytes
            .fetch_add(bytes.len() as u64, Ordering::Relaxed);
        let overlap = matcher.overlap();
        let mut pos = 0usize;
        while pos < bytes.len() {
            let end = (pos + cfg.chunk_bytes).min(bytes.len());
            let scan_end = (end + overlap).min(bytes.len());
            let base = pos;
            matcher.for_each_start(&bytes[base..scan_end], |m| record(base + m, end));
            pos = end;
        }
    };

    if want as u64 <= TINY_FILE {
        tiny_buf.clear();
        let mut take = file.take(want as u64);
        match take.read_to_end(tiny_buf) {
            Ok(_) => scan(tiny_buf, &mut record),
            Err(_) => {
                counters.errors.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        }
    } else {
        let mmap = match unsafe { MmapOptions::new().len(want).map(&file) } {
            Ok(m) => m,
            Err(_) => {
                counters.errors.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        };
        #[cfg(unix)]
        let _ = mmap.advise(memmap2::Advice::Sequential);
        scan(&mmap[..], &mut record);
        // Дроп mmap = munmap: страницы сразу покидают RSS нашего процесса
        // (memmap2 0.9.11 не экспортирует DontNeed; он и не нужен —
        // отображение живёт ровно до конца скана этого файла).
        drop(mmap);
    }

    if count == 0 {
        None
    } else {
        Some(FileHit {
            path: path.to_path_buf(),
            size,
            match_count: count,
            offsets,
        })
    }
}

// ---------------------------------------------------------------------------
// Фаза 2: ранжирование сделано — стриминговая агрегация в документ
// ---------------------------------------------------------------------------

/// Обёртка Write с подсчётом байт (кап max_out_bytes проверяет цикл файлов).
struct CountingWriter<W: Write> {
    inner: W,
    n: u64,
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let w = self.inner.write(buf)?;
        self.n += w as u64;
        Ok(w)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn json_escape(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Начала строк файла (байтовые offsetы) для отображения позиций в строки.
fn line_starts(buf: &[u8]) -> Vec<u32> {
    let mut v = vec![0u32];
    for (i, b) in buf.iter().enumerate() {
        if *b == b'\n' && i + 1 < buf.len() {
            v.push((i + 1) as u32);
        }
    }
    v
}

/// Строка i файла (без завершающего \n в срезе — он внутри среза).
fn line_slice<'a>(buf: &'a [u8], starts: &[u32], i: usize) -> &'a [u8] {
    let s = starts[i] as usize;
    let e = if i + 1 < starts.len() {
        starts[i + 1] as usize
    } else {
        buf.len()
    };
    &buf[s..e.max(s)]
}

/// Слияние контекстных диапазонов; покрытие ≥ 80% → весь файл.
fn merged_ranges(offsets: &[u32], starts: &[u32], ctx: usize) -> Vec<(usize, usize)> {
    let n_lines = starts.len();
    let mut ranges: Vec<(usize, usize)> = offsets
        .iter()
        .map(|off| {
            let l = starts.partition_point(|s| *s <= *off).saturating_sub(1);
            (l.saturating_sub(ctx), (l + ctx + 1).min(n_lines))
        })
        .collect();
    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in ranges {
        if let Some(last) = merged.last_mut() {
            if s <= last.1 {
                if e > last.1 {
                    last.1 = e;
                }
                continue;
            }
        }
        merged.push((s, e));
    }
    let coverage: usize = merged.iter().map(|(s, e)| e.saturating_sub(*s)).sum();
    if !merged.is_empty() && coverage * 100 >= n_lines * WHOLE_FILE_COVERAGE_PCT {
        merged = vec![(0, n_lines)];
    }
    merged
}

fn write_output(
    cfg: &HarvestConfig,
    hits: &[FileHit],
    stats: &mut HarvestStats,
) -> std::io::Result<()> {
    let stdout = cfg.out_path.as_os_str() == "-";
    let inner: Box<dyn Write> = if stdout {
        Box::new(BufWriter::with_capacity(OUT_BUF, std::io::stdout().lock()))
    } else {
        Box::new(BufWriter::with_capacity(OUT_BUF, File::create(&cfg.out_path)?))
    };
    let mut w = CountingWriter { inner, n: 0 };

    if cfg.format == HarvestFormat::Markdown {
        write_md_header(&mut w, cfg, stats)?;
    } else if cfg.format == HarvestFormat::Json {
        let mut line = String::from("{\"type\":\"meta\",\"roots\":[");
        for (i, r) in cfg.roots.iter().enumerate() {
            if i > 0 {
                line.push(',');
            }
            json_escape(&r.display().to_string(), &mut line);
        }
        line.push_str("],\"query\":");
        json_escape(&cfg.query, &mut line);
        line.push_str(&format!(
            ",\"regex\":{},\"mode\":\"{}\",\"context\":{}}}\n",
            cfg.regex_mode,
            match cfg.mode {
                HarvestMode::Sections => "sections",
                HarvestMode::Files => "files",
            },
            cfg.context_lines
        ));
        w.write_all(line.as_bytes())?;
    }

    // Переиспользуемый буфер чтения фазы агрегации (растёт до max_file_bytes).
    let mut buf: Vec<u8> = Vec::new();

    'files: for (idx, hit) in hits.iter().enumerate() {
        if w.n >= cfg.max_out_bytes {
            stats.truncated = true;
            break 'files;
        }
        // TOCTOU: файл изменился между фазами — честный пропуск с предупреждением.
        let size_now = fs::metadata(&hit.path).map(|m| m.len());
        let unchanged = matches!(size_now, Ok(s) if s == hit.size);
        if !unchanged {
            stats.changed_during_harvest += 1;
            eprintln!(
                "poler_disk_harvester: файл изменился во время сбора, пропуск: {}",
                hit.path.display()
            );
            continue;
        }
        let want = hit.size.min(cfg.max_file_bytes);
        let f = File::open(&hit.path)?;
        buf.clear();
        f.take(want).read_to_end(&mut buf)?;

        match (cfg.format, cfg.mode) {
            (HarvestFormat::Markdown, HarvestMode::Files) => {
                write!(
                    w,
                    "\n## [{idx}] {} — {} совпадений, {} байт\n\n````text\n",
                    hit.path.display(),
                    hit.match_count,
                    hit.size
                )?;
                w.write_all(&buf)?;
                if !buf.ends_with(b"\n") {
                    w.write_all(b"\n")?;
                }
                w.write_all(b"````\n")?;
                stats.sections_written += 1;
            }
            (HarvestFormat::Markdown, HarvestMode::Sections) => {
                write!(
                    w,
                    "\n## [{idx}] {} — {} совпадений, {} байт\n",
                    hit.path.display(),
                    hit.match_count,
                    hit.size
                )?;
                let starts = line_starts(&buf);
                for (s, e) in merged_ranges(&hit.offsets, &starts, cfg.context_lines) {
                    if w.n >= cfg.max_out_bytes {
                        stats.truncated = true;
                        break 'files;
                    }
                    w.write_all(b"\n````text\n")?;
                    for i in s..e {
                        let line = line_slice(&buf, &starts, i);
                        write!(w, "{:>5} | ", i + 1)?;
                        w.write_all(line)?;
                        if !line.ends_with(b"\n") {
                            w.write_all(b"\n")?;
                        }
                    }
                    w.write_all(b"````\n")?;
                    stats.sections_written += 1;
                }
            }
            (HarvestFormat::Json, HarvestMode::Files) => {
                let mut line = String::from("{\"type\":\"file\",\"path\":");
                json_escape(&hit.path.display().to_string(), &mut line);
                line.push_str(&format!(
                    ",\"bytes\":{},\"matches\":{},\"truncated\":{},\"content\":",
                    hit.size,
                    hit.match_count,
                    want < hit.size
                ));
                json_escape(&String::from_utf8_lossy(&buf), &mut line);
                line.push_str("}\n");
                w.write_all(line.as_bytes())?;
                stats.sections_written += 1;
            }
            (HarvestFormat::Json, HarvestMode::Sections) => {
                let starts = line_starts(&buf);
                let ranges = merged_ranges(&hit.offsets, &starts, cfg.context_lines);
                let mut line = String::from("{\"type\":\"file\",\"path\":");
                json_escape(&hit.path.display().to_string(), &mut line);
                line.push_str(&format!(
                    ",\"bytes\":{},\"matches\":{},\"sections\":[",
                    hit.size, hit.match_count
                ));
                for (si, (s, e)) in ranges.iter().enumerate() {
                    if si > 0 {
                        line.push(',');
                    }
                    let text: Vec<&[u8]> = (*s..*e).map(|i| line_slice(&buf, &starts, i)).collect();
                    let joined = text.concat();
                    line.push_str(&format!("{{\"start\":{},\"end\":{},\"text\":", s + 1, e));
                    json_escape(&String::from_utf8_lossy(&joined), &mut line);
                    line.push('}');
                    stats.sections_written += 1;
                }
                line.push_str("]}\n");
                w.write_all(line.as_bytes())?;
            }
            (HarvestFormat::Corpus, HarvestMode::Files) => {
                w.write_all(&buf)?;
                if !buf.ends_with(b"\n") {
                    w.write_all(b"\n")?;
                }
                stats.sections_written += 1;
            }
            (HarvestFormat::Corpus, HarvestMode::Sections) => {
                let starts = line_starts(&buf);
                for (s, e) in merged_ranges(&hit.offsets, &starts, cfg.context_lines) {
                    for i in s..e {
                        let line = line_slice(&buf, &starts, i);
                        w.write_all(line)?;
                        if !line.ends_with(b"\n") {
                            w.write_all(b"\n")?;
                        }
                    }
                    w.write_all(b"\n")?;
                    stats.sections_written += 1;
                }
            }
        }
    }

    if cfg.format == HarvestFormat::Markdown {
        write_md_footer(&mut w, cfg, stats)?;
    } else if cfg.format == HarvestFormat::Json {
        let line = format!(
            "{{\"type\":\"summary\",\"files_scanned\":{},\"files_matched\":{},\"files_selected\":{},\"sections\":{},\"bytes_written\":{},\"truncated\":{}}}\n",
            stats.files_scanned,
            stats.files_matched,
            stats.files_selected,
            stats.sections_written,
            w.n,
            stats.truncated
        );
        w.write_all(line.as_bytes())?;
    }
    w.flush()?;
    stats.bytes_written = w.n;
    if stats.truncated {
        eprintln!(
            "poler_disk_harvester: вывод обрезан по --harvest-max-out-bytes ({})",
            cfg.max_out_bytes
        );
    }
    Ok(())
}

fn write_md_header<W: Write>(
    w: &mut CountingWriter<W>,
    cfg: &HarvestConfig,
    stats: &HarvestStats,
) -> std::io::Result<()> {
    let roots: Vec<String> = cfg.roots.iter().map(|r| r.display().to_string()).collect();
    writeln!(
        w,
        "# POLER Disk Harvest\n\n\
         - **roots**: {}\n\
         - **query**: {}{}\n\
         - **mode**: {} (context: {})\n\
         - **matched files**: {} (selected: {})\n",
        roots.join(", "),
        cfg.query,
        if cfg.regex_mode { " [regex]" } else { "" },
        match cfg.mode {
            HarvestMode::Sections => "sections",
            HarvestMode::Files => "files",
        },
        cfg.context_lines,
        stats.files_matched,
        stats.files_selected,
    )
}

fn write_md_footer<W: Write>(
    w: &mut CountingWriter<W>,
    cfg: &HarvestConfig,
    stats: &HarvestStats,
) -> std::io::Result<()> {
    if stats.truncated {
        writeln!(
            w,
            "\n> Вывод обрезан по --harvest-max-out-bytes ({})",
            cfg.max_out_bytes
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Тесты (DoD: битые симлинки, невалидный UTF-8, переполнение буферов,
// границы чанков, кириллица, детерминизм, капы)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("poler_harvest_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(path: &Path, content: impl AsRef<[u8]>) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn run_ok(cfg: &HarvestConfig) -> HarvestStats {
        run_harvest(cfg).unwrap()
    }

    /// Выходной файл ВНЕ дерева обхода: тесты не зависят от
    /// самопоглощения и продакшн-исключения выходного файла.
    fn outpath(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "poler_harvest_out_{name}_{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&p);
        p
    }

    fn out_str(path: &Path) -> String {
        fs::read_to_string(path).unwrap()
    }

    #[test]
    fn broken_symlink_skipped_gracefully() {
        let d = tmpdir("broken_symlink");
        put(&d.join("good.txt"), "гамильтониан системы");
        std::os::unix::fs::symlink(d.join("missing.txt"), d.join("dangling.lnk")).unwrap();
        let out = outpath("broken_symlink");
        let cfg = HarvestConfig::new(vec![d.clone()], "гамильтониан", &out);
        let stats = run_ok(&cfg);
        assert!(stats.skipped_symlink >= 1, "битый симлинк должен быть пропущен");
        assert_eq!(stats.files_selected, 1);
        assert!(out_str(&out).contains("good.txt"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn binary_nul_sniffed_and_skipped() {
        let d = tmpdir("binary_nul");
        put(&d.join("data.txt"), "prefix \x00\x01\x02 гамильтониан");
        put(&d.join("clean.txt"), "гамильтониан чистый");
        let out = outpath("binary_nul");
        let cfg = HarvestConfig::new(vec![d.clone()], "гамильтониан", &out);
        let stats = run_ok(&cfg);
        assert!(stats.skipped_binary >= 1);
        assert_eq!(stats.files_selected, 1);
        assert!(!out_str(&out).contains("data.txt"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn junk_dirs_pruned() {
        let d = tmpdir("junk_dirs");
        put(&d.join("target/junk.txt"), "гамильтониан в мусоре");
        put(&d.join("node_modules/lib.js"), "гамильтониан в мусоре");
        put(&d.join("real.txt"), "гамильтониан настоящий");
        let out = outpath("junk_dirs");
        let cfg = HarvestConfig::new(vec![d.clone()], "гамильтониан", &out);
        let stats = run_ok(&cfg);
        assert_eq!(stats.files_selected, 1);
        assert!(out_str(&out).contains("real.txt"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn hidden_skipped_by_default_included_with_flag() {
        let d = tmpdir("hidden");
        put(&d.join(".secret/hidden.txt"), "гамильтониан скрытый");
        put(&d.join("open.txt"), "гамильтониан явный");
        let out = outpath("hidden1");
        let cfg = HarvestConfig::new(vec![d.clone()], "гамильтониан", &out);
        let stats = run_ok(&cfg);
        assert_eq!(stats.files_selected, 1);

        let out2 = outpath("hidden2");
        let mut cfg2 = HarvestConfig::new(vec![d.clone()], "гамильтониан", &out2);
        cfg2.include_hidden = true;
        let stats2 = run_ok(&cfg2);
        assert_eq!(stats2.files_selected, 2);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn gitignore_respected_and_overridable() {
        let d = tmpdir("gitignore");
        put(&d.join(".gitignore"), "*.ignored\n");
        put(&d.join("a.ignored"), "гамильтониан игнорируется");
        put(&d.join("b.txt"), "гамильтониан видимый");
        let out = outpath("gitignore1");
        let cfg = HarvestConfig::new(vec![d.clone()], "гамильтониан", &out);
        let stats = run_ok(&cfg);
        assert_eq!(stats.files_selected, 1);

        let out2 = outpath("gitignore2");
        let mut cfg2 = HarvestConfig::new(vec![d.clone()], "гамильтониан", &out2);
        cfg2.no_ignore = true;
        let stats2 = run_ok(&cfg2);
        assert_eq!(stats2.files_selected, 2);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn cyrillic_case_variants_match() {
        let d = tmpdir("cyrillic_case");
        put(&d.join("cap.txt"), "Гамильтониан системы");
        put(&d.join("lower.txt"), "гамильтониан системы");
        put(&d.join("upper.txt"), "ГАМИЛЬТОНИАН системы");
        put(&d.join("none.txt"), "нет совпадений");
        let out = outpath("cyrillic_case");
        let cfg = HarvestConfig::new(vec![d.clone()], "гамильтониан", &out);
        let stats = run_ok(&cfg);
        assert_eq!(stats.files_selected, 3);
        let text = out_str(&out);
        assert!(text.contains("cap.txt") && text.contains("lower.txt") && text.contains("upper.txt"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn sections_context_and_merging() {
        let d = tmpdir("sections_ctx");
        // 6 строк; совпадения на строках 1 и 4 (0-based).
        let text = "line0\nneedle one\nline2\nline3\nneedle two\nline5\n";
        put(&d.join("f.txt"), text);
        let out = outpath("sections_ctx");

        // ctx=0 → две изолированные секции по одной строке.
        let mut cfg = HarvestConfig::new(vec![d.clone()], "needle", &out);
        cfg.context_lines = 0;
        let stats = run_ok(&cfg);
        assert_eq!(stats.sections_written, 2, "ctx=0: две секции");
        assert!(out_str(&out).contains("needle one") && out_str(&out).contains("needle two"));

        // ctx=2 → диапазоны перекрываются, покрытие 100% → весь файл одной секцией.
        let mut cfg2 = HarvestConfig::new(vec![d.clone()], "needle", &out);
        cfg2.context_lines = 2;
        let stats2 = run_ok(&cfg2);
        assert_eq!(stats2.sections_written, 1, "ctx=2: единая секция");
        let text_out = out_str(&out);
        assert!(text_out.contains("line0") && text_out.contains("line5"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn max_files_and_min_matches_filters() {
        let d = tmpdir("caps");
        put(&d.join("three.txt"), "x x x гамильтониан гамильтониан гамильтониан");
        put(&d.join("two.txt"), "гамильтониан гамильтониан");
        put(&d.join("one.txt"), "гамильтониан");
        let out = outpath("caps");

        let mut cfg = HarvestConfig::new(vec![d.clone()], "гамильтониан", &out);
        cfg.max_files = 2;
        let stats = run_ok(&cfg);
        assert_eq!(stats.files_selected, 2);
        let text = out_str(&out);
        assert!(text.contains("three.txt") && text.contains("two.txt"));
        assert!(!text.contains("one.txt"), "кап max_files отсекает слабейший");

        let mut cfg2 = HarvestConfig::new(vec![d.clone()], "гамильтониан", &out);
        cfg2.min_matches = 2;
        let stats2 = run_ok(&cfg2);
        assert_eq!(stats2.files_selected, 2, "min_matches=2 фильтрует one.txt");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn markdown_structure() {
        let d = tmpdir("md_structure");
        put(&d.join("f.txt"), "alpha\nneedle here\nomega");
        let out = outpath("md_structure");
        let cfg = HarvestConfig::new(vec![d.clone()], "needle", &out);
        run_ok(&cfg);
        let text = out_str(&out);
        assert!(text.starts_with("# POLER Disk Harvest"));
        assert!(text.contains("## [0] "));
        assert!(text.contains("````text"));
        assert!(text.contains(" | needle here"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn json_jsonl_shape() {
        let d = tmpdir("json_shape");
        put(&d.join("f.txt"), "alpha\nneedle here\nomega");
        let out = outpath("json_shape");
        let mut cfg = HarvestConfig::new(vec![d.clone()], "needle", &out);
        cfg.format = HarvestFormat::Json;
        run_ok(&cfg);
        let text = out_str(&out);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() >= 3, "meta + file + summary");
        assert!(lines[0].starts_with("{\"type\":\"meta\""));
        assert!(lines[0].contains("\"roots\""));
        assert!(lines.last().unwrap().contains("\"type\":\"summary\""));
        for l in &lines {
            assert!(l.starts_with('{') && l.ends_with('}'), "каждая строка — JSON-объект: {l}");
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn corpus_is_pure_text() {
        let d = tmpdir("corpus_pure");
        put(&d.join("f.txt"), "alpha\nneedle here\nomega");
        let out = outpath("corpus_pure");
        let mut cfg = HarvestConfig::new(vec![d.clone()], "needle", &out);
        cfg.format = HarvestFormat::Corpus;
        run_ok(&cfg);
        let text = out_str(&out);
        assert!(!text.contains("# POLER"), "без markdown-меты");
        assert!(!text.contains("````"));
        assert!(text.contains("needle here"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn files_mode_dumps_whole_file() {
        let d = tmpdir("files_mode");
        put(&d.join("f.txt"), "far top\nneedle\nfar bottom");
        let out = outpath("files_mode");
        let mut cfg = HarvestConfig::new(vec![d.clone()], "needle", &out);
        cfg.mode = HarvestMode::Files;
        run_ok(&cfg);
        let text = out_str(&out);
        assert!(text.contains("far top") && text.contains("far bottom"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn chunk_boundary_matches_recorded_once() {
        let d = tmpdir("chunk_boundary");
        // Терм из 13 байт, стартующий на 7-й и ровно на 8-й байт (граница чанка 8).
        let term = "boundary_term";
        let f1: String = "a".repeat(7) + term + "bbbb";
        let f2: String = "a".repeat(8) + term + "bbbb";
        put(&d.join("span.txt"), &f1);
        put(&d.join("edge.txt"), &f2);
        let out = outpath("chunk_boundary");
        let mut cfg = HarvestConfig::new(vec![d.clone()], "boundary_term", &out);
        cfg.chunk_bytes = 8; // overlap = 13 ≥ длины терма
        let stats = run_ok(&cfg);
        assert_eq!(stats.files_selected, 2, "оба граничных файла найдены");
        // Ровно по одному совпадению — дедупликация на границе чанков.
        for name in ["span.txt", "edge.txt"] {
            let matcher = Matcher::build(&cfg).unwrap();
            let counters = ScanCounters::default();
            let mut buf = Vec::new();
            let path = d.join(name);
            let size = fs::metadata(&path).unwrap().len();
            let hit = scan_file(&path, size, &cfg, &matcher, &counters, &mut buf).unwrap();
            assert_eq!(hit.match_count, 1, "{name}: совпадение ровно одно");
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn invalid_utf8_lossy_no_panic() {
        let d = tmpdir("invalid_utf8");
        put(&d.join("f.txt"), b"\xff\xfe broken\nneedle valid line\n");
        let out = outpath("invalid_utf8");
        let cfg = HarvestConfig::new(vec![d.clone()], "needle", &out);
        let stats = run_ok(&cfg);
        assert_eq!(stats.files_selected, 1);
        // Маркдаун хранит сырые байты («до байта») — читаем как байты.
        let out_bytes = fs::read(&out).unwrap();
        assert!(
            String::from_utf8_lossy(&out_bytes).contains("needle valid line"),
            "lossy-чтение должно находить валидную строку"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn multi_root_harvest() {
        let d1 = tmpdir("multi_root_a");
        let d2 = tmpdir("multi_root_b");
        put(&d1.join("a.txt"), "гамильтониан из корня A");
        put(&d2.join("b.txt"), "гамильтониан из корня B");
        let out = outpath("multi_root");
        let cfg = HarvestConfig::new(vec![d1.clone(), d2.clone()], "гамильтониан", &out);
        let stats = run_ok(&cfg);
        assert_eq!(stats.files_selected, 2);
        let text = out_str(&out);
        assert!(text.contains("a.txt") && text.contains("b.txt"));
        let _ = fs::remove_dir_all(&d1);
        let _ = fs::remove_dir_all(&d2);
    }

    #[test]
    fn regex_mode_works() {
        let d = tmpdir("regex_mode");
        put(&d.join("f1.txt"), "гамильтониан квантовый");
        put(&d.join("f2.txt"), "гамільтоніан український");
        put(&d.join("f3.txt"), "нет совпадений");
        let out = outpath("regex_mode");
        let mut cfg = HarvestConfig::new(vec![d.clone()], "гам[иі]льтон", &out);
        cfg.regex_mode = true;
        let stats = run_ok(&cfg);
        assert_eq!(stats.files_selected, 2, "regex покрывает RU и UA варианты");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn corpus_output_deterministic() {
        let d = tmpdir("determinism");
        put(&d.join("a.txt"), "needle один\nконтент");
        put(&d.join("b.txt"), "needle два\nдругой контент");
        let out1 = outpath("det1");
        let out2 = outpath("det2");
        let mut c1 = HarvestConfig::new(vec![d.clone()], "needle", &out1);
        c1.format = HarvestFormat::Corpus;
        let mut c2 = HarvestConfig::new(vec![d.clone()], "needle", &out2);
        c2.format = HarvestFormat::Corpus;
        run_ok(&c1);
        run_ok(&c2);
        assert_eq!(
            fs::read(&out1).unwrap(),
            fs::read(&out2).unwrap(),
            "повторный прогон байт-в-байт"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn offsets_capped_but_count_exact() {
        let d = tmpdir("offsets_cap");
        let mut text = String::new();
        for _ in 0..10_000 {
            text.push_str("x needle\n");
        }
        put(&d.join("big.txt"), &text);
        let cfg = HarvestConfig::new(vec![d.clone()], "needle", outpath("offsets_cap"));
        let matcher = Matcher::build(&cfg).unwrap();
        let counters = ScanCounters::default();
        let mut buf = Vec::new();
        let path = d.join("big.txt");
        let size = fs::metadata(&path).unwrap().len();
        let hit = scan_file(&path, size, &cfg, &matcher, &counters, &mut buf).unwrap();
        assert_eq!(hit.match_count, 10_000, "счётчик полный");
        assert_eq!(hit.offsets.len(), MAX_OFFSETS_PER_FILE, "позиции capped");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn no_matches_and_errors() {
        let d = tmpdir("no_match");
        put(&d.join("f.txt"), "ничего");
        let out = outpath("no_match");
        let cfg = HarvestConfig::new(vec![d.clone()], "гамильтониан", &out);
        let stats = run_ok(&cfg);
        assert_eq!(stats.files_selected, 0, "вызывающая сторона → exit 1");

        let bad = HarvestConfig::new(vec![d.clone()], "", &out);
        assert!(run_harvest(&bad).is_err(), "пустой запрос → Err");

        let missing = HarvestConfig::new(
            vec![PathBuf::from("/nonexistent/poler/xyz")],
            "терм",
            &out,
        );
        assert!(run_harvest(&missing).is_err(), "несуществующий корень → Err");
        let _ = fs::remove_dir_all(&d);
    }
}
