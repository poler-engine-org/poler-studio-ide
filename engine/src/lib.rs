#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! # POLER-Engine
//!
//! AI-Native поисково-аналитический движок, вытесняющий `grep`/`ripgrep`
//! и слепой векторный RAG из архитектуры LLM-агентов.
//!
//! ## Что устраняется
//!
//! | Проблема grep/RAG | Механизм POLER |
//! |---|---|
//! | Graph Blindness | Enclosing Scope + K-hop подграф связей |
//! | Cosine Collapse / Negation Blindness | Exact Lexical Anchors + маркеры отрицаний в ε |
//! | Chunk Fragmentation | Сцены/функции целиком как границы окон |
//! | BM25/TF-IDF частотная ловушка | Информационная плотность ε на локальной энтропии |
//! | Temporal Blindness | Temporal Metric Tagging + фильтрация графа |
//! | Single-Fault Blindness (интерпретатор) | AIDDE: call graph + Impact Passport |
//!
//! ## Потоковый пайплайн (v0.3, ограниченная память)
//!
//! ```text
//! files ──► [Проход 1, rayon] mmap → литеральный предфильтр
//!              │   (Teddy SIMD, класс kwset/Teddy: решёто якорных байтов
//!              │    pshufb + адаптивные якоря; ASCII CI / быстрый фолд
//!              │    D0/D1 для кириллицы)
//!              │    ├─ нет литерала → streaming counts (без индекса)
//!              │    └─ есть литерал → временный FileTokens (zero-copy &str)
//!              │         → глобальные частоты + позиции совпадений
//!              │         (временные структуры освобождаются сразу)
//!              ▼
//!        merge global stats (N_total, freq) — только словарь корпуса
//!              (v2.0 Compression: FSST-арена — термы сжаты, ID-стабильны;
//!               пер-файловые словари — пары ID 8Б, постинги — lz4-парковка)
//!              ▼
//!        [Проход 2, rayon] только hit-файлы:
//!              окна W_t → ε (+semantic bonus) → IIR R_t = ε_t + φ·R_{t-1}
//!              → лёгкая локализация сцен (без клонов текста)
//!              → тройки уникальных сцен
//!              ▼
//!        EntityGraph (бюджет рёбер) → K-hop BFS от корневой сущности
//!              ▼
//!        temporal-фильтр → сортировка по R → top-N
//!              ▼
//!        [Проход 3] материализация сцен только для top-N якорей
//! ```
//!
//! Гигантские файлы (>= 8 МБ) обрабатываются строго последовательно.
//! Никакие индексы не удерживаются между фазами: пиковое потребление
//! памяти ограничено словарём корпуса + одним временным индексом.

pub mod aidde;
pub mod archive;
pub mod bench;
// poler-box — Linux-only: namespaces/pivot_root/seccomp/memfd —
// примитивы ядра Linux. На других ОС — заглушка с честной ошибкой
// (архитектура — src/boxenv/mod.rs).
#[cfg(target_os = "linux")]
pub mod boxenv;
#[cfg(not(target_os = "linux"))]
pub mod boxenv {
    use std::path::PathBuf;

    #[derive(Debug, Clone)]
    pub struct BoxSpec {
        pub archive: PathBuf,
        pub entry: String,
        pub args: Vec<String>,
        pub maps: Vec<(String, String)>,
        pub rss_mb: u64,
        pub cpu_s: u64,
        pub tmpfs_mb: u64,
        pub isolate: bool,
    }

    impl Default for BoxSpec {
        fn default() -> Self {
            Self {
                archive: PathBuf::new(),
                entry: String::new(),
                args: Vec::new(),
                maps: vec![("rootfs/".to_string(), "/".to_string())],
                rss_mb: 512,
                cpu_s: 60,
                tmpfs_mb: 512,
                isolate: true,
            }
        }
    }

    #[derive(Debug, Clone, serde::Serialize)]
    pub struct BoxReport {
        pub entry: String,
        pub exit_code: Option<i32>,
        pub signal: Option<i32>,
        pub kill_reason: Option<&'static str>,
        pub peak_tree_rss_kb: u64,
        pub cpu_seconds: f64,
        pub wall_seconds: f64,
        pub isolation: IsolationSummary,
    }

    #[derive(Debug, Clone, serde::Serialize)]
    pub struct IsolationSummary {
        pub userns_via_helper: bool,
        pub namespaces: &'static str,
        pub pivot_root: bool,
        pub seccomp: String,
        pub notes: Vec<String>,
    }

    pub fn run_box(_spec: &BoxSpec) -> Result<BoxReport, String> {
        Err("poler-box: только Linux — namespaces/pivot_root/seccomp/memfd — примитивы ядра Linux".into())
    }

    pub fn stage2_main() -> i32 {
        // stage2-вход вызывается только Linux-хелпером unshare
        125
    }
}
pub mod browser;
pub mod calc;
pub mod compression;
pub mod crypto;
pub mod editor;
pub mod engine;
pub mod universal_archetype_asm;
// winpe — исполнение Windows PE: живёт НА Linux (memfd/ptrace/proc —
// примитивы ядра Linux). На других ОС — заглушка с честной ошибкой.
#[cfg(target_os = "linux")]
pub mod winpe;
#[cfg(not(target_os = "linux"))]
pub mod winpe {
    pub fn winexec_file(_path: &std::path::Path, _args: &[String]) -> Result<i32, String> {
        Err("winpe: исполнение Windows PE — только на Linux (memfd/ptrace/proc — примитивы ядра Linux)".into())
    }
}
pub mod universal_letters;
pub mod chem;
pub mod universal_chem;
pub mod universal_chem_asm;
/// FFI-мост к No-Mul .s-роторам (буквы мира + химия), собираемым build.rs
pub mod asm_rotors;

/// v0.87.0 VAULT-ASM: микроядра сокровищницы (FEP/LENS/SSN/CORDIC/STDP)
/// на чистом x86_64-ассемблере — docs/VAULT_ASM.md.
#[cfg(target_arch = "x86_64")]
pub mod asm;
/// Голос учёного: токенизатор → роторы → химия → триединое ядро
pub mod scivoice;

/// E1/v0.31.0: идеальный исполнитель команд — Zig-ядро
/// (os/core/poler_exec.zig, raw-syscall слой) + безопасная обвязка.
#[cfg(feature = "pnd-ffi")]
pub mod exec;

pub mod gateway;
pub mod game;
/// Сессия-12: GIS-ядро на тритных координатах — geo-types порт,
/// DE-9IM в 9-тритном коде, marching tetrahedra для изоповерхностей.
pub mod geo;
pub mod graph;
pub mod license;
pub mod literary;
pub mod ssn;
pub mod search;
pub mod triune;
pub mod llm;
pub mod mcp;
pub mod mcp_http;
pub mod ner;
pub mod notes;
pub mod output;
/// Цикл R (v0.53.0): P³-Мост — C-ABI конформанс Rust↔Zig и рендер
/// «кадр из гамильтониана» (libp3ffi.so поверх ядра P³ Engine).
pub mod p3;
pub mod parser;
pub mod poler;
pub mod psi;
pub mod pqc;
pub mod quantum;
pub mod reader;
pub mod resonance;
pub mod retrieval;
pub mod shell;
pub mod sources;
pub mod streaming;
pub mod tokenizer;
pub mod vcs;
pub mod vectors;
pub mod web;

use std::fs::File;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use memmap2::Mmap;

pub use aidde::{
    impact_analysis, triage_scan, Dependency, Dependent, ImpactReport, StructuralRelations,
    SymbolTable, TriageAlert, TriageCategory,
};
pub use engine::{Engine, WatchEvent};
pub use graph::{CodeSymbolRef, EntityGraph, IdentityPolicy};
pub use output::{render_markdown, render_simple, ContextAnchor, SearchResult};
pub use parser::{
    detect_lang, extract_code_triples, extract_enclosing_scope, extract_triples, CodeLang,
    CodeScope, SceneContext, Triple,
};
pub use poler::{cordic_inv_sqrt, poler_resonances, PolerCycle, PolerParams};
pub use psi::{psi_resonances, PsiField, PsiParams};
pub use resonance::{
    apply_iir_resonance, calculate_epsilon, semantic_bonus, IirFilter, SlidingEpsilon,
};
pub use compression::{GlobalStats, PostingsStore, TermFreqs, VocabArena};
pub use search::intent::{detect_query_intent, hit_tier, FileClass, IntentMode, QueryIntent, SignatureQuery, TypeDeclQuery};
pub use streaming::{FileTokens, GIANT_FILE_BYTES};
pub use tokenizer::{InvertedIndex, PiiCleaner, PiiMode};

/// Директории, исключаемые из обхода.
pub const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", ".svn", ".hg", "__pycache__"];

/// Расширения, сканируемые по умолчанию.
///
/// v0.83.0: +`zig`,`s`,`asm`,`lua` — низкоуровневые ядра (poler-os)
/// и скрипты больше не отсекаются на обходе дерева. Матчинг расширений
/// НЕЗАВИСИМ ОТ РЕГИСТРА (v0.83.0): `.s` GNU as и `.S` после cpp —
/// оба покрываются записью `s` (конфиг приводится к нижнему регистру).
///
/// v0.84.0: +`hh`,`hxx`,`cxx`,`inc`,`inl` — современный C++ (Blender,
/// LLVM, Google) держит заголовки и inline-определения в `.hh`/`.hxx`/
/// `.inc`/`.inl`; без них `struct BMVert` в `bmesh_class.hh` требовал
/// ручного `--extensions hh` (грабля 42, стресс-тест Blender).
pub const DEFAULT_EXTENSIONS: &str =
    "md,markdown,txt,rst,rs,py,c,h,cpp,hpp,cc,hh,hxx,cxx,inc,inl,js,jsx,ts,tsx,java,go,kt,swift,cs,scala,dart,zig,s,asm,lua,json,toml,yaml,yml,sql,sh";

/// Режим накопления резонанса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResonanceMode {
    /// IIR по последовательности совпадений (ε окна каждого хита):
    /// вырожденный (K=1) случай резонанса памяти POLER R[n] = ρᵏ·s_{t−k}.
    Hits,
    /// Поле резонанса по всему документу, строго O(N), семплы на хитах.
    Field,
    /// POLER[Ψ]: уравнение внимания из POLER_Psi_v3.py (Kotokvit).
    Psi,
    /// Канонический POLER-цикл из P3_Engine (p3_poler.zig, Kotokvit):
    /// `p_new = p − η·Π_Λ(D·p + γ·J·p + ∇F)` — диссипатор D=LLᵀ,
    /// кососимметричный резонанс J=A−Aᵀ, каузальный проектор Π_Λ,
    /// CORDIC-ренормализация.
    Poler,
}

/// Конфигурация движка.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Радиус окна в токенах (полуокно).
    pub window_radius: usize,
    /// Коэффициент затухания IIR-резонанса φ ∈ [0.75, 0.90].
    pub phi_decay: f64,
    /// Масштабный коэффициент калибровки ε.
    pub kappa: f64,
    /// Число возвращаемых якорей.
    pub top_n: usize,
    /// Глубина K-hop обхода графа.
    pub k_hop_depth: usize,
    /// Режим очистки PII.
    pub pii_mode: PiiMode,
    /// Режим резонанса.
    pub resonance_mode: ResonanceMode,
    /// Temporal-фильтр (например, `Т-23`).
    pub temporal_filter: Option<String>,
    /// Считать ε по статистикам файла, а не корпуса.
    pub local_stats: bool,
    /// Сканируемые расширения (lowercase, без точек).
    pub extensions: Vec<String>,
    /// Максимальный размер файла в байтах.
    pub max_file_bytes: u64,
    /// Максимальная длина enclosing_scope в байтах.
    pub max_scope_bytes: usize,
    /// Максимум K-hop отношений на якорь.
    pub max_relations: usize,
    /// Показывать скрытые файлы (rg --hidden).
    pub include_hidden: bool,
    /// Дамп графа сущностей в SQL-файл (схема super-z memory_graph).
    pub graph_export: Option<PathBuf>,
    /// Бюджет рёбер графа сущностей (защита от неограниченного роста).
    pub max_graph_triples: usize,
    /// Гиперпараметры POLER[Ψ] (η, γ, ρ, K) — режим резонанса Psi.
    pub psi_params: crate::psi::PsiParams,
    /// Гиперпараметры канонического POLER-цикла (P3_Engine) — режим Poler.
    pub poler_params: crate::poler::PolerParams,
    /// Режим intent-детекта запроса: код ↔ проза (v0.82.0).
    pub intent_mode: IntentMode,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            window_radius: 40,
            phi_decay: 0.85,
            kappa: 1.0,
            top_n: 10,
            k_hop_depth: 2,
            pii_mode: PiiMode::Mask,
            resonance_mode: ResonanceMode::Hits,
            temporal_filter: None,
            local_stats: false,
            extensions: DEFAULT_EXTENSIONS
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .collect(),
            max_file_bytes: 64 * 1024 * 1024,
            max_scope_bytes: 16 * 1024,
            max_relations: 64,
            include_hidden: false,
            graph_export: None,
            max_graph_triples: 200_000,
            psi_params: crate::psi::PsiParams::default(),
            poler_params: crate::poler::PolerParams::default(),
            intent_mode: IntentMode::Auto,
        }
    }
}

/// Статистика прогона (для `--verbose`).
#[derive(Debug, Clone, Default)]
pub struct ScanStats {
    pub files_scanned: usize,
    pub files_with_hits: usize,
    pub total_tokens: u64,
    pub total_hits: usize,
    pub graph_nodes: usize,
    pub graph_edges: usize,
    pub elapsed_ms: u128,
}

// ---------------------------------------------------------------------------
// Чтение файлов: mmap + UTF-8 + PII
// ---------------------------------------------------------------------------

/// Открывает файл через mmap и вызывает `f(text)`.
/// Невалидный UTF-8 конвертируется lossy; пустые файлы дают `""`.
///
/// v2.0 Foundation (2.1): подсказка ядру `madvise(SEQUENTIAL)` — чтение
/// файла предсказуемо последовательное (токенизация/стемминг всего
/// буфера), ядро включает readahead и кэш-страницы не вытесняются
/// случайными обходами других файлов.
pub(crate) fn with_text<T>(path: &Path, max_bytes: u64, f: impl FnOnce(&str) -> T) -> Option<T> {
    let file = File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() > max_bytes {
        return None;
    }
    if meta.len() == 0 {
        return Some(f(""));
    }
    let mmap = unsafe { Mmap::map(&file) }.ok()?;
    // Foundation 2.1: последовательный доступ — ядро начинает readahead,
    // страницы маппинга читаются кластерами, а не по требованию.
    // Ошибка не фатальна (некоторые ФС не поддерживают) — тихо продолжаем.
    #[cfg(unix)]
    let _ = mmap.advise(memmap2::Advice::Sequential);
    let bytes = &mmap[..];
    let lossy;
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => {
            lossy = String::from_utf8_lossy(bytes).into_owned();
            lossy.as_str()
        }
    };
    Some(f(text))
}

/// Сбор файлов: WalkBuilder из крейта `ignore` (ripgrep) — .gitignore,
/// скрытые файлы, фильтр нежелательных директорий.
pub fn collect_files(target: &Path, config: &EngineConfig) -> Vec<PathBuf> {
    if target.is_file() {
        return vec![target.to_path_buf()];
    }
    let exts: Vec<String> = config.extensions.clone();
    WalkBuilder::new(target)
        .hidden(!config.include_hidden)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .filter_entry(move |e| {
            e.file_type().map_or(true, |t| !t.is_dir())
                || !SKIP_DIRS.contains(&e.file_name().to_string_lossy().as_ref())
        })
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                // v0.83.0: регистронезависимо — `.S` (после cpp) матчится
                // на конфиг-запись `s`; конфиг и так фолдится в нижний регистр
                .map(|s| exts.iter().any(|x| x.eq_ignore_ascii_case(s)))
                .unwrap_or(false)
        })
        .map(|e| e.into_path())
        .collect()
}

// ---------------------------------------------------------------------------
// Публичное API (совместимость с v0.2)
// ---------------------------------------------------------------------------

/// Синхронное сканирование пути. См. [`scan_path_with_stats`].
pub fn scan_path(target: &Path, query: &str, config: &EngineConfig) -> SearchResult {
    scan_path_with_stats(target, query, config).0
}

/// Полный прогон движка (без watcher-состояния).
pub fn scan_path_with_stats(
    target: &Path,
    query: &str,
    config: &EngineConfig,
) -> (SearchResult, ScanStats) {
    let mut engine = Engine::new(config.clone(), false);
    engine.scan(target, query)
}

/// Вспомогательная функция для CLI: проверка существования пути.
pub fn ensure_target_exists(target: &Path) -> std::io::Result<()> {
    if target.exists() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("путь не найден: {}", target.display()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_sane() {
        let c = EngineConfig::default();
        assert!(c.window_radius > 0);
        assert!((0.75..=0.90).contains(&c.phi_decay));
        assert!(c.top_n > 0);
        assert!(c.k_hop_depth >= 2);
        assert!(c.extensions.contains(&"md".to_string()));
        assert!(c.max_graph_triples > 0);
        // v0.84.0: современный C++ в дефолте (Blender-грабля 42 —
        // `struct BMVert` в bmesh_class.hh не находился без --extensions hh)
        for ext in ["hh", "hxx", "cxx", "inc", "inl"] {
            assert!(
                c.extensions.contains(&ext.to_string()),
                "нет .{ext} в дефолтных расширениях"
            );
        }
    }

    #[test]
    fn empty_query_returns_empty() {
        let res = scan_path(Path::new("/nonexistent"), "", &EngineConfig::default());
        assert_eq!(res.total_hits, 0);
        let res2 = scan_path(Path::new("/nonexistent"), "   ", &EngineConfig::default());
        assert_eq!(res2.total_hits, 0);
    }
}
