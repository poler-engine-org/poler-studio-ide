#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! CLI poler-engine: AI-Native Topographical, Resonant and Graph Search Engine.
//!
//! Режимы:
//! * поиск: `poler-engine <PATH> -q <QUERY> [--watch]`
//! * impact-анализ (AIDDE): `poler-engine <PATH> --impact <SYMBOL>`
//! * веб-поиск для AI: `poler-engine --web-search <QUERY>` (по веб-индексу)
//! * краулинг: `poler-engine <URL> --crawl [--crawl-depth N --crawl-max M]`
//!
//! v2.0 (sovereign stack): Google/NotebookLM/Gmail/Drive/OAuth-интеграции
//! удалены. Движок полностью локален; внешний мир — только краулинг через
//! CDP и MCP-инструменты для агентов.
//!
//! Коды выхода (grep-совместимые): 0 — есть совпадения, 1 — совпадений нет,
//! 2 — ошибка.

use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use poler_engine::aidde::{impact_analysis, SymbolTable};
use poler_engine::{
    collect_files, render_markdown, render_simple, Engine, EngineConfig, PiiMode, ResonanceMode,
    CodeLang, DEFAULT_EXTENSIONS, SearchResult,
};

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
enum Format {
    /// Самодостаточный AI-Ready JSON (Context Anchor).
    AiJson,
    /// Markdown с секциями для человека.
    Md,
    /// Одна строка на совпадение.
    Simple,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
enum StreamTierArg {
    /// Только zstd-3: максимум скорости записи.
    Fast,
    /// Только zstd-15: максимум плотности (медленнее).
    Deep,
    /// Авто: zstd-3, глубокий ярус для хорошо сжимаемых чанков
    /// (дефолт директивы).
    Auto,
}

/// Парсер размеров с суффиксами: "100M", "4GiB", "2T", "512K".
fn parse_byte_size(s: &str) -> Result<u64, String> {
    let t = s.trim();
    let (num, mult) = if let Some(v) = t.strip_suffix("GiB").or_else(|| t.strip_suffix("G")) {
        (v, 1024u64.pow(3))
    } else if let Some(v) = t.strip_suffix("MiB").or_else(|| t.strip_suffix("M")) {
        (v, 1024u64.pow(2))
    } else if let Some(v) = t.strip_suffix("KiB").or_else(|| t.strip_suffix("K")) {
        (v, 1024u64)
    } else if let Some(v) = t.strip_suffix("TiB").or_else(|| t.strip_suffix("T")) {
        (v, 1024u64.pow(4))
    } else if let Some(v) = t.strip_suffix("B") {
        (v, 1)
    } else {
        (t, 1)
    };
    let n: u64 = num
        .trim()
        .parse()
        .map_err(|_| format!("не число: {s:?}"))?;
    n.checked_mul(mult).ok_or_else(|| format!("переполнение: {s:?}"))
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
enum PiiArg {
    /// PII-маскирование выключено.
    Off,
    /// Email/телефоны/IP/секреты заменяются маркерами (по умолчанию).
    Mask,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
enum ResonanceArg {
    /// IIR-резонанс по последовательности совпадений (R[n], K=1).
    Hits,
    /// Поле резонанса по всему документу, строго O(N).
    Field,
    /// POLER[Ψ]: уравнение внимания из POLER_Psi_v3.py.
    Psi,
    /// Канонический POLER-цикл из P3_Engine (p3_poler.zig):
    /// p −= η·Π_Λ(D·p + γ·J·p + ∇F), CORDIC-ренормализация.
    Poler,
}

/// v0.82.0: режим intent-ранжирования — код ↔ проза (симметрично).
#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
enum IntentArg {
    /// Автодетект: `func Default`/`express()` → код; кириллица/вопросы → проза.
    Auto,
    /// Принудительно: приоритет исходникам, документация — вниз.
    Code,
    /// Принудительно: приоритет документации, код — вниз.
    Prose,
    /// Без intent-тиров (ранжирование чисто по резонансу, как в v0.81).
    Off,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
enum KnowledgeEmbedderArg {
    /// Без векторного слоя (только BM25/WebRank) — быстрый инжест.
    None,
    /// Детерминированная хэш-проекция 512-d: полный гибридный конвейер
    /// (RaBitQ+HNSW) без модели — инструментальная проекция, НЕ семантика.
    Hash,
    /// Нативный энкодер из --model <path.pqw> (BGE-M3/XLM-R класс).
    Pqw,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
enum HarvestFormatArg {
    /// Markdown: секции с номерами строк (для человека/агента).
    Markdown,
    /// JSONL: meta → file-записи → summary (стримится построчно).
    Json,
    /// Чистый текст-корпус для обучения кристалла (alias: t5c).
    #[value(alias = "t5c")]
    Corpus,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
enum MolModeArg {
    /// Шаростержневой: сферы ковалентных радиусов + связи.
    Ball,
    /// Space-filling: ван-дер-ваальсовы сферы (реальный объём).
    Vdw,
    /// Каркас: только связи.
    Wire,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
enum PdbModeArg {
    /// Паспорт макромолекулы: цепочки, состав, вторичная структура.
    Info,
    /// Поиск карманов связывания (лигандный + слепой grid-burial).
    Pocket,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
enum JitModeArg {
    /// Бенчмарк JIT vs reference: бит-в-бит IEEE-754 + время.
    Bench,
    /// Человекочитаемый asm-листинг машинного кода ядра.
    Asm,
    /// Печать DataflowGraph: слои, активации, вакуум.
    Graph,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
enum HarvestModeArg {
    /// Секции: ±context строк вокруг совпадений, слияние перекрытий.
    Sections,
    /// Файлы целиком (стриминг, ≤ max-file-bytes).
    Files,
}

#[derive(Parser, Debug)]
#[command(
    name = "poler-engine",
    version,
    author = "POLER Engineering Core",
    about = "AI-Native Topographical, Resonant and Graph Search Engine",
    long_about = "Поисково-аналитический движок для LLM-агентов: полный логический скоуп \
                  (сцена/функция целиком), информационная плотность ε, резонанс R(t), \
                  K-hop подграф связей и AIDDE impact-анализ вместо изолированных строк grep."
)]
struct Cli {
    /// Путь к файлу или корню репозитория.
    /// С --web: трактуется как URL (https://...) — рендер через Chromium CDP.
    /// С --crawl: seed-URL для обхода.
    path: Option<PathBuf>,

    /// Рендер веб-страницы через Chromium CDP перед поиском
    /// (патч интерпретирует PATH как URL).
    #[arg(long)]
    web: bool,

    /// Порт Chromium DevTools (с --web) [default: 9222].
    #[arg(long = "cdp-port", default_value_t = 9222)]
    cdp_port: u16,

    /// Пауза после load на дочерние XHR, мс (с --web) [default: 1200].
    #[arg(long = "web-wait-ms", default_value_t = 1200)]
    web_wait_ms: u64,

    /// ВЕБ-ПОИСК: поиск по локальному веб-индексу (краулер --crawl).
    /// v0.21: запрос автоматически расширяется Semantic Bridge (офлайн
    /// сенсор ru↔en) — рус запрос находит англ корпус и обратно.
    #[arg(long = "web-search", conflicts_with_all = ["web", "crawl"])]
    web_search: Option<String>,

    /// SEMANTIC BRIDGE (v0.21): показать кросс-языковое расширение запроса
    /// офлайн-сенсором (кандидаты ru↔en + WHY + веса BM25) — без поиска.
    #[arg(
        long = "semantic-expand",
        value_name = "QUERY",
        conflicts_with_all = [
            "web_search", "crawl", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui",
            "impact", "browser_index", "web_lens", "web_lens_install", "grep", "chunk",
            "benchmark"
        ]
    )]
    semantic_expand: Option<String>,

    /// BENCHMARK (v0.21, Задача 4): автоматический бенчмарк-раннер —
    /// Exact Retrieval (POLER grep vs ripgrep/grep, полнота + parity),
    /// Explainable Lexical (BM25/WebRank + Semantic Bridge, golden top-1),
    /// Passage (чанкер vs naive splitter, целостность предложений),
    /// latency (мс) и RAM (VmHWM/VmRSS). Golden-регрессии — в cargo test.
    #[arg(
        long = "benchmark",
        conflicts_with_all = [
            "web_search", "crawl", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui",
            "impact", "browser_index", "web_lens", "web_lens_install", "grep", "chunk",
            "semantic_expand"
        ]
    )]
    benchmark: bool,

    /// Сохранить JSON-отчёт бенчмарка в файл (с --benchmark).
    #[arg(long = "benchmark-json", value_name = "PATH", requires = "benchmark")]
    benchmark_json: Option<PathBuf>,

    /// VAULT-ASM (v0.87.0): отчёт о микроядрах x86_64 сокровищницы —
    /// CPUID-детекция (AVX2/FMA/F16C/AVX-512/POPCNT) и карта ядер
    /// (FEP/LENS/SSN/CORDIC/STDP → точки движка).
    #[arg(
        long = "asm-info",
        conflicts_with_all = [
            "web_search", "crawl", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui",
            "impact", "browser_index", "web_lens", "web_lens_install", "grep", "chunk",
            "semantic_expand", "benchmark"
        ]
    )]
    asm_info: bool,

    /// VAULT-ASM (v0.87.0): бенчмарк микроядер сокровищницы — пропускная
    /// способность (M синапсов/с, GB/s) + сверка каждого ядра со
    /// скалярным эталоном (PASS/FAIL).
    #[arg(
        long = "asm-bench",
        conflicts_with_all = [
            "web_search", "crawl", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui",
            "impact", "browser_index", "web_lens", "web_lens_install", "grep", "chunk",
            "semantic_expand", "benchmark", "asm_info"
        ]
    )]
    asm_bench: bool,

    /// Число синапсов SSN-бенча (с --asm-bench) [default: 10_000_000].
    #[arg(long = "asm-synapses", default_value_t = 10_000_000)]
    asm_synapses: usize,

    /// Число циклов SSN/STDP-бенча (с --asm-bench) [default: 10].
    #[arg(long = "asm-cycles", default_value_t = 10)]
    asm_cycles: usize,

    /// Краулинг: PATH трактуется как seed-URL, страницы индексируются
    /// в веб-индекс (robots.txt, sitemap, SimHash-дедуп, PageRank).
    #[arg(long, requires = "path")]
    crawl: bool,

    /// Путь к базе веб-индекса [default: ~/.local/share/poler-engine/web-index.db].
    #[arg(long = "web-db")]
    web_db: Option<PathBuf>,

    /// Статистика веб-индекса (JSON в stdout).
    #[arg(long = "web-stats")]
    web_stats: bool,

    /// Глубина краулинга от seed [default: 2].
    #[arg(long = "crawl-depth", default_value_t = 2)]
    crawl_depth: usize,

    /// Максимум страниц за один обход [default: 25].
    #[arg(long = "crawl-max", default_value_t = 25)]
    crawl_max: usize,

    /// Минимальная пауза между запросами к одному хосту, мс [default: 1000].
    #[arg(long = "crawl-delay-ms", default_value_t = 1000)]
    crawl_delay_ms: u64,

    /// Пер-страничный таймаут загрузки в краулинге, мс [default: 45000].
    /// JS-тяжёлые сайты (бесконечные XHR/стриминг) не зависят дольше лимита.
    #[arg(long = "crawl-page-timeout-ms", default_value_t = 45_000)]
    crawl_page_timeout_ms: u64,

    /// ИНДЕКСАЦИЯ ОДНОЙ СТРАНИЦЫ: URL → рендер (Chromium CDP) → веб-индекс.
    /// Явная команда пользователя: robots.txt не блокирует (но фиксируется
    /// в notes). После — доступен --web-search по общему индексу.
    #[arg(long = "browser-index", value_name = "URL", conflicts_with_all = ["web", "crawl", "web_search", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui", "impact"])]
    browser_index: Option<String>,

    /// БРАУЗЕРНЫЙ РЕЖИМ: материализует WebLens (расширение MV3), запускает
    /// оконный Chromium с уже установленным WebLens и держит MCP-сервер
    /// на 127.0.0.1:8765 (BIND как у --mcp-http, или только порт).
    #[arg(long = "web-lens", value_name = "BIND", num_args = 0..=1, default_missing_value = "127.0.0.1:8765", conflicts_with_all = ["web", "crawl", "web_search", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens_install"])]
    web_lens: Option<String>,

    /// Установить WebLens в ЕЖЕДНЕВНЫЙ браузер: материализует файлы
    /// и печатает шаги «Load unpacked» (chrome://-страницы автоматизировать
    /// нельзя — это защита браузера; управляемый движком браузер ставит
    /// WebLens сам через --web-lens).
    #[arg(long = "web-lens-install", conflicts_with_all = ["web_lens", "mcp_http", "mcp", "shell", "exec", "tui", "crawl", "web_search", "browser_index"])]
    web_lens_install: bool,

    // ---------- Native Retrieval: grep-режим + RAG-чанки (v0.20.0) ----------

    /// ТОЧНЫЙ ПОИСК (grep-режим): ВСЕ совпадения PATTERN по файлам
    /// (PATH или текущий каталог), без индекса, гарантия полноты.
    /// Exit-коды как у grep: 0 — найдено, 1 — пусто, 2 — ошибка.
    #[arg(long = "grep", value_name = "PATTERN", conflicts_with_all = ["web", "crawl", "web_search", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens", "web_lens_install", "chunk"])]
    grep: Option<String>,

    /// Регулярное выражение вместо фиксированной строки (grep -E).
    #[arg(long = "grep-regex", requires = "grep")]
    grep_regex: bool,

    /// Регистронезависимость, Unicode-fold (grep -i).
    #[arg(long = "grep-i", requires = "grep")]
    grep_ignore_case: bool,

    /// Строк контекста ПОСЛЕ совпадения (grep -A NUM).
    #[arg(long = "grep-after", requires = "grep", default_value_t = 0)]
    grep_after: usize,

    /// Строк контекста ДО совпадения (grep -B NUM).
    #[arg(long = "grep-before", requires = "grep", default_value_t = 0)]
    grep_before: usize,

    /// Только счётчик совпадений на файл (grep -c).
    #[arg(long = "grep-count", requires = "grep")]
    grep_count: bool,

    /// Только пути файлов с совпадениями (grep -l).
    #[arg(long = "grep-list", requires = "grep")]
    grep_list: bool,

    /// Только пути файлов БЕЗ совпадений (grep -L).
    #[arg(long = "grep-list-nonmatching", requires = "grep")]
    grep_list_nonmatching: bool,

    /// Останов после N совпавших строк на файл (grep -m NUM).
    #[arg(long = "grep-max-count", value_name = "NUM", requires = "grep")]
    grep_max_count: Option<usize>,

    /// Включить скрытые файлы (по умолчанию пропускаются, как ripgrep).
    #[arg(long = "grep-hidden", requires = "grep")]
    grep_hidden: bool,

    /// Машинно-читаемый JSON вместо текста — byte offsets, диапазоны
    /// вхождений, статистика (для ИИ-агента).
    #[arg(long = "grep-json", requires = "grep")]
    grep_json: bool,

    /// Дополнительный корень grep-поиска (multi-root workspace IDE).
    /// Повторяемый: `poler-engine root1 --grep root2 --grep-root root3`.
    /// Все корни сканируются в ОДНОМ процессе параллельно через rayon —
    /// без N процессов на N корней и без общего пула потоков на каждый.
    #[arg(long = "grep-root", value_name = "PATH", requires = "grep", action = clap::ArgAction::Append)]
    grep_roots: Vec<PathBuf>,

    // ---------- poler_disk_harvester: суб-секундный сбор диска ----------

    /// HARVEST: корни обхода (1+; каталоги, репозитории или файлы).
    /// Параллельный ripgrep-обход + mmap + Aho-Corasick → один документ.
    #[arg(
        long = "harvest-disk",
        value_name = "ROOT_PATH",
        requires = "harvest_query",
        conflicts_with_all = [
            "web", "web_search", "crawl", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui",
            "impact", "browser_index", "web_lens", "web_lens_install", "grep", "chunk",
            "benchmark", "semantic_expand"
        ]
    )]
    harvest_disk: Vec<PathBuf>,

    /// Термы через пробел/запятую (OR; кириллица — вариантами регистра)
    /// или единый regex при --harvest-regex.
    #[arg(long = "harvest-query", value_name = "PATTERN")]
    harvest_query: Option<String>,

    /// Куда писать результат [default: stdout; "-" = stdout].
    #[arg(long = "harvest-out", value_name = "OUT_FILE")]
    harvest_out: Option<PathBuf>,

    /// Формат: markdown | json | corpus (t5c-корпус для обучения).
    #[arg(long = "harvest-format", value_enum, default_value = "markdown")]
    harvest_format: HarvestFormatArg,

    /// Гранулярность: sections (±context строк) | files (целиком).
    #[arg(long = "harvest-mode", value_enum, default_value = "sections")]
    harvest_mode: HarvestModeArg,

    /// Строк контекста вокруг совпадения (sections).
    #[arg(long = "harvest-context", default_value_t = 6)]
    harvest_context: usize,

    /// Кап файлов в выдаче (ранжирование по числу совпадений).
    #[arg(long = "harvest-max-files", default_value_t = 20_000)]
    harvest_max_files: usize,

    /// Минимум совпадений на файл для попадания в выдачу.
    #[arg(long = "harvest-min-matches", default_value_t = 1)]
    harvest_min_matches: usize,

    /// Скан первых N байт файла (гиганты — частично).
    #[arg(long = "harvest-max-file-bytes", default_value_t = 8 * 1024 * 1024)]
    harvest_max_file_bytes: u64,

    /// Кап размера выходного документа.
    #[arg(long = "harvest-max-out-bytes", default_value_t = 4294967296_u64)]
    harvest_max_out_bytes: u64,

    /// Число воркеров (0 = все ядра).
    #[arg(long = "harvest-threads", default_value_t = 0)]
    harvest_threads: usize,

    /// Включить скрытые файлы/каталоги (по умолчанию как ripgrep — нет).
    #[arg(long = "harvest-hidden")]
    harvest_hidden: bool,

    /// Игнорировать .gitignore (собрать ВСЁ, включая игнорируемое).
    #[arg(long = "harvest-no-ignore")]
    harvest_no_ignore: bool,

    /// Трактовать --harvest-query как regex (regex::bytes).
    #[arg(long = "harvest-regex")]
    harvest_regex: bool,

    // ---------- v0.38.0: POLER Sovereign Browser ----------

    /// СУВЕРЕННЫЙ БРАУЗЕР POLER: загрузить страницу, отфильтровать рекламу
    /// по квантовой ε-плотности и извлечь чистый семантический DOM/текст.
    #[arg(long = "browser-url", value_name = "URL")]
    browser_url: Option<String>,

    /// Сохранить чистый текст и структуру веб-страницы в файл.
    #[arg(long = "browser-out", value_name = "OUT_FILE", requires = "browser_url")]
    browser_out: Option<PathBuf>,

    // ---------- v0.38.0: poler-edit — суверенный редактор без лимитов ----------

    /// EDITOR SERVER: JSON-протокол поверх stdio (LSP-стиль) для GUI-клиентов
    /// (integrations/poler-edit-qt). Один процесс — много документов (вкладки),
    /// события прогресса индексации/поиска, отмена длинных операций.
    #[arg(
        long = "edit-serve",
        conflicts_with_all = [
            "web", "web_search", "crawl", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui",
            "impact", "browser_index", "web_lens", "web_lens_install", "grep", "chunk",
            "benchmark", "semantic_expand", "harvest_disk", "edit_bench", "browser_url"
        ]
    )]
    edit_serve: bool,

    /// EDITOR BENCH: открыть FILE (любого размера — mmap zero-copy),
    /// проиндексировать строки и выполнить поиск; печатает открытые цифры
    /// (open ms, index GiB/s, search GiB/s, peak RSS) — см. docs/POLER_EDIT.md.
    #[arg(
        long = "edit-bench",
        value_name = "FILE",
        conflicts_with_all = [
            "web", "web_search", "crawl", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui",
            "impact", "browser_index", "web_lens", "web_lens_install", "grep", "chunk",
            "benchmark", "semantic_expand", "harvest_disk", "edit_serve", "browser_url"
        ]
    )]
    edit_bench: Option<PathBuf>,

    /// Искать эту подстроку в --edit-bench [default: e].
    #[arg(long = "edit-bench-query", default_value = "e")]
    edit_bench_query: String,

    // ---------- v0.28.1: Архивы без распаковки ----------

    /// Скан архивов без распаковки (с --grep): записи zip/tar/tar.gz/
    /// tar.zst/gz/zst внутри PATH читаются напрямую из контейнера
    /// виртуальными файлами «архив::запись». Ничего не пишется на диск.
    #[arg(long = "archives", requires = "grep")]
    archives: bool,

    /// Пароль зашифрованных архивов (ZipCrypto/AES). Вводится прямо
    /// в CLI человеком или ИИ-агентом. Виден в истории shell — для
    /// секретности используйте POLER_ARCHIVE_KEY или TTY-промпт.
    #[arg(long = "archive-password", value_name = "PASS")]
    archive_password: Option<String>,

    /// Лимит несжатой записи архива, МиБ [default: 64] — защита от
    /// zip-бомб: запись свыше лимита пропускается с ошибкой в stats.
    #[arg(long = "archive-max-entry-mb", value_name = "NUM", default_value_t = 64)]
    archive_max_entry_mb: u64,

    /// ЛИСТИНГ АРХИВА: записи контейнера (имя/размеры/шифрование)
    /// без распаковки и без пароля — осмотр перед вскрытием.
    #[arg(long = "archive-list", value_name = "ARCHIVE", conflicts_with_all = ["grep", "chunk", "web", "crawl", "web_search", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens", "web_lens_install"])]
    archive_list: Option<PathBuf>,

    /// JSON-вывод листинга архива (с --archive-list) — для агента.
    #[arg(long = "archive-json", requires = "archive_list")]
    archive_json: bool,

    /// RAG-ЧАНКИ: нарезать документ PATH на фрагменты с якорями
    /// (byte range, номера строк, breadcrumb заголовков) —
    /// passage-уровень для агента вместо чтения документа целиком.
    #[arg(long = "chunk", requires = "path", conflicts_with_all = ["web", "crawl", "web_search", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens", "web_lens_install", "grep"])]
    chunk: bool,

    /// Целевой размер чанка в токенах POLER [default: 384].
    #[arg(long = "chunk-size", requires = "chunk", default_value_t = poler_engine::retrieval::DEFAULT_TARGET_TOKENS)]
    chunk_size: usize,

    /// Перекрытие соседних чанков в токенах [default: 48].
    #[arg(long = "chunk-overlap", requires = "chunk", default_value_t = poler_engine::retrieval::DEFAULT_OVERLAP_TOKENS)]
    chunk_overlap: usize,

    /// Машинно-читаемый JSON вместо текста (для ИИ-агента).
    #[arg(long = "chunk-json", requires = "chunk")]
    chunk_json: bool,

    /// MCP-СЕРВЕР (Model Context Protocol): poler-engine как нативный
    /// инструмент LLM-агентов поверх stdio JSON-RPC.
    /// 32+ инструмента: поиск/греп/чанки (poler_search/grep/chunk), веб (crawl/fetch),
    /// Калькулятор Всего (poler_calc), квантовый мост (poler_quantum: qft/grover/
    /// teleport/qaoa/bloch/verify), живая нейросеть (poler_triune_speak/state),
    /// мозг мухи FLYCSR1 (poler_fly_*: 10 инструментов), SSN-вихрь (poler_ssn_*),
    /// литературный POLER[Psi] (poler_literary_*), sandbox (poler_box_*).
    /// ПОЛНЫЙ живой список: `poler-engine --mcp` + tools/list, или `--schema`.
    #[arg(long = "mcp", conflicts_with_all = ["web_search", "crawl", "web_stats", "impact", "mcp_http", "mcp_token"])]
    mcp: bool,

    /// MCP-СЕРВЕР ПО HTTP (Streamable HTTP): тот же набор инструментов,
    /// что и --mcp, но для УДАЛЁННОГО агента — через туннель (например
    /// `cloudflared tunnel --url http://127.0.0.1:8765`). POST / или /mcp,
    /// заголовок Authorization: Bearer <токен>.
    /// BIND = «127.0.0.1:8765» (по умолчанию) или просто порт «8765».
    #[arg(long = "mcp-http", value_name = "BIND", num_args = 0..=1, default_missing_value = "127.0.0.1:8765", conflicts_with_all = ["mcp", "shell", "exec", "tui", "web_search", "crawl", "web_stats", "impact"])]
    mcp_http: Option<String>,

    /// Токен доступа для --mcp-http (или env POLER_MCP_TOKEN; без него
    /// генерируется при старте и печатается в stderr).
    #[arg(long = "mcp-token", value_name = "TOKEN", requires = "mcp_http")]
    mcp_token: Option<String>,

    /// M6: ШИФРОПОТОК ЖУРНАЛА операций MCP-сервера (--mcp/--mcp-http)
    /// в Vault .pvt — стриминг логов: каждое JSON-RPC-сообщение дописывается
    /// зашифованной строкой (ts, method, tool, латентность мкс, ok) без
    /// пере-печати файла. Читается как обычный --memory-open (на любом
    /// коммите файл валиден). Фраза: --vault-log-pass | env POLER_VAULT_PASS
    /// | интерактивный вопрос (как у --memory-seal).
    #[arg(long = "vault-log", value_name = "FILE.pvt")]
    vault_log: Option<std::path::PathBuf>,

    /// Парольная фраза для --vault-log (иначе env POLER_VAULT_PASS/stdin).
    #[arg(long = "vault-log-pass", value_name = "PASS", requires = "vault_log")]
    vault_log_pass: Option<String>,

    /// M6: RAM-бюджет резидентного файл-кэша grep (МиБ; 0 = безлимит).
    #[arg(long = "mcp-ram-budget", value_name = "MIB", default_value_t = 48)]
    mcp_ram_budget: usize,

    /// M6: БЕНЧМАРК РЕЗИДЕНТНОСТИ: N итераций poler_grep и
    /// query_poler_knowledge холодный-против-тёплого (p50/p95/p99),
    /// RAM кэша и проверка бюджета <5 мс. Без --path/--knowledge-db
    /// строит временный корпус (самодостаточная верификация M6).
    #[arg(long = "mcp-bench", value_name = "N", num_args = 0..=1, default_missing_value = "200", conflicts_with_all = ["shell", "exec", "tui", "mcp", "mcp_http", "web_search", "crawl", "web_stats", "impact", "grep", "chunk", "benchmark", "web_lens", "web_lens_install", "browser_index", "license", "semantic_expand"])]
    mcp_bench: Option<usize>,

    // ---------- poler-shell: интерактивный терминал v0.15.0 ----------

    /// TERMINAL GATEWAY (v0.22.0): единый терминальный шлюз — двойной
    /// контур исполнения (engine-native приоритет + sandboxed host proxy),
    /// конвейеры host↔engine, service/attach управление нижним слоем.
    /// Верхний уровень управления на Linux/macOS.
    #[arg(long, conflicts_with_all = ["shell", "exec", "tui", "mcp", "mcp_http", "web_search", "crawl", "web_stats", "impact", "grep", "chunk", "benchmark", "web_lens", "web_lens_install", "browser_index", "license", "semantic_expand"])]
    gateway: bool,

    /// v0.23.0 DANGER OVERRIDE: полностью отключить sandbox Terminal
    /// Gateway. Красный баннер при старте; блокировки и подтверждения
    /// НЕ применяются — вся ответственность за безопасность хоста
    /// ложится на оператора. Требует --gateway.
    #[arg(long = "dangerously-allow-all", requires = "gateway")]
    dangerously_allow_all: bool,

    /// Интерактивный REPL (poler> search/crawl/notes/sync vcs...).
    /// База web-index.db открывается лениво и переиспользуется
    /// между командами (история в ~/.cache/poler-engine/shell-history.txt).
    #[arg(long = "shell", conflicts_with_all = ["tui", "mcp", "web_search", "crawl", "web_stats", "impact"])]
    shell: bool,

    /// v0.47.0: одноразовое исполнение команды poler-shell без баннера
    /// и промптов — интерфейс для ИИ-агентов и скриптов.
    /// Примеры: --exec 'search "квант" --top 3'; --exec 'dir'; --exec 'sysinfo';
    /// v0.48.0: --exec 'calc 2^10' --json; --exec '= 5 km to mi';
    ///          --exec 'calc solve x^2 - 4 = 0'; --exec 'hw --json'.
    /// Поддерживает полный словарь: Linux-команды, Windows-команды (WinCompat),
    /// команды движка и пути к .poler-контейнерам.
    #[arg(long = "exec", value_name = "CMD", conflicts_with_all = ["shell", "tui", "mcp", "mcp_http", "web_search", "crawl", "web_stats", "impact"])]
    exec: Option<String>,

    /// v0.47.0: вывести результат --exec одной JSON-строкой
    /// {cmd, ok, exit_code, duration_ms, output} — машиночитаемый конверт.
    #[arg(long = "json", requires = "exec")]
    exec_json: bool,

    /// v0.74.0: ИНТЕРАКТИВНЫЙ 3D/4D-ВИЗОР МОЛЕКУЛ — SMILES или имя
    /// (кофеин, аспирин, дофамин, серотонин…). Живая орбитальная камера:
    /// ←→↑↓/hjkl вращение, +/- зум, m режим (шаростержневой/ВдВ/каркас),
    /// t — 4D-кручение σ-связей, [ ] скорость, q выход. Не-TTY — кадр.
    /// Примеры: --view-mol "CC(=O)Oc1ccccc1C(=O)O"; --view-mol кофеин
    #[arg(
        long = "view-mol",
        value_name = "SMILES|ИМЯ",
        conflicts_with_all = ["shell", "tui", "mcp", "mcp_http", "web_search", "crawl", "web_stats", "impact", "exec"]
    )]
    view_mol: Option<String>,

    /// Начальный режим --view-mol: ball | vdw | wire [default: ball].
    #[arg(long = "mol-mode", value_enum, default_value_t = MolModeArg::Ball, requires = "view_mol")]
    mol_mode: MolModeArg,

    /// v0.76.0: ВНУТРЕННИЙ КОМПИЛЯТОР (контур A). bench — JIT vs reference
    /// на случайном графе (бит-в-бит, время); asm — человекочитаемый листинг
    /// машинного кода ядра; graph — печать DataflowGraph с вакуумом.
    #[arg(
        long = "jit",
        value_name = "bench|asm|graph",
        conflicts_with_all = ["shell", "tui", "mcp", "mcp_http", "web_search", "crawl", "web_stats", "impact", "exec", "view_mol", "pdb_file", "dock", "view_complex"]
    )]
    jit_mode: Option<JitModeArg>,

    /// Размер matvec для --jit bench: строки [default: 64].
    #[arg(long = "jit-rows", default_value_t = 64, requires = "jit_mode")]
    jit_rows: usize,

    /// Размер matvec для --jit bench: столбцы [default: 48].
    #[arg(long = "jit-cols", default_value_t = 48, requires = "jit_mode")]
    jit_cols: usize,

    /// v0.75.0: PDB-МАКРОМОЛЕКУЛЫ — паспорт или поиск карманов.
    /// Примеры: --pdb 3ptb.pdb; --pdb 1crn.pdb --pdb-mode pocket
    #[arg(
        long = "pdb",
        value_name = "FILE",
        conflicts_with_all = ["shell", "tui", "mcp", "mcp_http", "web_search", "crawl", "web_stats", "impact", "exec", "view_mol", "dock", "view_complex"]
    )]
    pdb_file: Option<String>,

    /// Режим --pdb: info | pocket [default: info].
    #[arg(long = "pdb-mode", value_enum, default_value_t = PdbModeArg::Info, requires = "pdb_file")]
    pdb_mode: PdbModeArg,

    /// v0.75.0: ГИБКИЙ ДОКИНГ — SMILES-лиганд в PDB-белок.
    /// Примеры: --dock "NC(=N)c1ccccc1" 3ptb.pdb;
    ///          --dock "NC(=N)c1ccccc1" apo.pdb --pocket blind;
    ///          --dock кофеин x.pdb --pocket 12.3,4.5,-6.7 --dock-out pose.pdb
    #[arg(
        long = "dock",
        value_name = "SMILES FILE",
        num_args = 2,
        conflicts_with_all = ["shell", "tui", "mcp", "mcp_http", "web_search", "crawl", "web_stats", "impact", "exec", "view_mol", "pdb_file", "view_complex", "screen"]
    )]
    dock: Option<Vec<String>>,

    /// Карман докинга/скрининга: auto (по кристаллическому лиганду,
    /// redocking) | blind (слепой grid-burial) | x,y,z (точка, Å)
    /// [default: auto].
    #[arg(long = "pocket", value_name = "auto|blind|x,y,z")]
    pocket: Option<String>,

    /// Записать лучшую позу как PDB (HETATM LIG) — для --view-complex.
    #[arg(long = "dock-out", value_name = "FILE", requires = "dock")]
    dock_out: Option<String>,

    /// Прогонов MC/SA [default: 12].
    #[arg(long = "dock-runs", default_value_t = 12, requires = "dock")]
    dock_runs: usize,

    /// Шагов Метрополиса на прогон [default: 2200].
    #[arg(long = "dock-steps", default_value_t = 2200, requires = "dock")]
    dock_steps: usize,

    /// v0.78.0: INDUCED FIT — гибкость боковых цепей кармана при
    /// докинге (контур C): раунд жёсткий докинг → MC-релаксация χ
    /// (трит-секторы 13⅓°) → редокинг; ≤3 раунда.
    #[arg(long = "induced", default_value_t = false, requires = "dock")]
    dock_induced: bool,

    /// v0.79.0: ЗАДАЧНИК «CURRICULUM LEARNING» (контур D) — школьная
    /// химия 5–9 класса как бенчмарк: 187 задач с эталонными ответами
    /// независимой Python-реализации (двойная запись MVR). Без значения —
    /// все классы; --curriculum 8 — только 8-й.
    /// Примеры: --curriculum; --curriculum 8; --curriculum --curriculum-text
    #[arg(
        long = "curriculum",
        value_name = "GRADE",
        num_args = 0..=1,
        default_missing_value = "0",
        conflicts_with_all = ["shell", "tui", "mcp", "mcp_http", "web_search", "crawl", "web_stats", "impact", "exec", "view_mol", "pdb_file", "view_complex", "dock", "screen", "jit_mode"]
    )]
    curriculum: Option<String>,

    /// Режим SciVoice: задачи решаются ИЗ РУССКОГО ТЕКСТА формулировки
    /// (разбор → структурированный ввод → решение), а не из готового
    /// JSON-ввода. Требует --curriculum.
    #[arg(long = "curriculum-text", requires = "curriculum")]
    curriculum_text: bool,

    /// v0.77.0: УЛЬТРА-СКРИНИНГ БИБЛИОТЕК (контур B) — SDF/.smi-библиотека
    /// в активный карман: каскад пре-фильтры → быстрый MC (JIT-ядро A3) →
    /// полный докинг топ-N. Карман строится один раз на все лиганды.
    /// Примеры: --screen lib.sdf 3ptb.pdb;
    ///          --screen lib.smi x.pdb --pocket blind --screen-top 5 --workers 8
    #[arg(
        long = "screen",
        value_name = "LIB FILE",
        num_args = 2,
        conflicts_with_all = ["shell", "tui", "mcp", "mcp_http", "web_search", "crawl", "web_stats", "impact", "exec", "view_mol", "pdb_file", "view_complex", "jit_mode"]
    )]
    screen: Option<Vec<String>>,

    /// Топ-N лигандов в полный докинг (стадия 3) и в таблицу
    /// [default: 10% библиотеки, минимум 1]. (--top занят поисковым
    /// слоем движка — флаг называется --screen-top).
    #[arg(long = "screen-top", default_value_t = 0, requires = "screen")]
    screen_top: usize,

    /// Воркеров скрининга [default: все ядра].
    #[arg(long = "workers", default_value_t = 0, requires = "screen")]
    screen_workers: usize,

    /// JSON-отчёт скрининга (hits.json).
    #[arg(long = "screen-out", value_name = "FILE", requires = "screen")]
    screen_out: Option<String>,

    /// v0.75.0: ИНТЕРАКТИВНЫЙ 3D-ВИЗОР БЕЛОК-ЛИГАНДНОГО КОМПЛЕКСА —
    /// CA-трейс по классам остатков, боковые цепи кармана, лиганд CPK,
    /// ВдВ-поверхность. Пример: --view-complex 3ptb.pdb
    #[arg(
        long = "view-complex",
        value_name = "FILE",
        conflicts_with_all = ["shell", "tui", "mcp", "mcp_http", "web_search", "crawl", "web_stats", "impact", "exec", "view_mol", "pdb_file", "dock"]
    )]
    view_complex: Option<String>,

    /// TUI Dashboard на ratatui (панели: chat + ввод | notes + sources).
    /// Tab — смена фокуса, Esc — выход. Команды как в --shell.
    #[arg(long = "tui", conflicts_with_all = ["shell", "exec", "mcp", "web_search", "crawl", "web_stats", "impact"])]
    tui: bool,

    // ---------- License / EULA (v2.0: статус модели, без гейта) ----------

    /// Статус лицензии и модель распространения (Source-Available EULA).
    /// Локальный поиск и все функции движка — всегда без ограничений;
    /// v2.0 не содержит ключей и гейтов.
    #[arg(long = "license", conflicts_with_all = ["shell", "exec", "tui", "mcp", "mcp_http"])]
    license: bool,

    /// Самодокументирующийся JSON-манифест возможностей движка (AI-API-реформа):
    /// домены CLI, MCP-инструменты, честные границы вычислимости, примеры.
    /// Одна команда — и любая модель/LLM/человек видит весь движок целиком,
    /// без чтения 947 строк --help.
    #[arg(long = "schema", conflicts_with_all = ["shell", "exec", "tui", "mcp", "mcp_http", "license"])]
    schema: bool,
    /// Разрешить краулеру переход на другие хосты.
    #[arg(long = "cross-site", default_value_t = false)]
    cross_site: bool,

    /// Поисковый запрос: слово или фраза (в кавычках).
    #[arg(short, long)]
    query: Option<String>,

    // ---------- v2.0 Part E/F: суверенный ML-инференс (.pqw через pqc) ----------

    /// Путь к .pqw-модели (для --semantic dense / --llm local / --ner gliner).
    #[arg(long = "model", value_name = "PQW")]
    model: Option<PathBuf>,

    /// Семантический режим: dense = нативный энкодер из .pqw (через pqc,
    /// не ONNX). Требует --model и -q. С --semantic-corpus — живой поиск
    /// по каталогу/файлу (чанки → эмбеддинги → косинус → топ-N).
    #[arg(long = "semantic", value_name = "MODE")]
    semantic: Option<String>,

    /// Корпус для --semantic dense или --llm quantum (каталог или файл): чанкируются
    /// или насыщают русла квантовой циркуляции J.
    #[arg(long = "corpus", value_name = "PATH")]
    corpus: Option<PathBuf>,

    /// Корпус для --semantic dense (каталог или файл): чанкируются,
    /// эмбеддятся нативным энкодером и ранжируются косинусом.
    #[arg(long = "semantic-corpus", value_name = "PATH", requires = "semantic")]
    semantic_corpus: Option<PathBuf>,

    /// Сколько верхних результатов печатать в --semantic-corpus поиске.
    #[arg(long = "semantic-limit", default_value_t = 5)]
    semantic_limit: usize,

    /// Потолок чанков для --semantic-corpus (равномерная выборка по корпусу).
    #[arg(long = "semantic-max-chunks", default_value_t = 512)]
    semantic_max_chunks: usize,

    /// LLM-генерация (Part F): local = нативный GLM-декодер из .pqw.
    /// remote/auto — мост к серверному GLM (Фаза 12.9, заглушка).
    #[arg(long = "llm", value_name = "MODE")]
    llm: Option<String>,

    /// Извлечение сущностей: gliner = нативная span-голова из .pqw.
    #[arg(long = "ner", value_name = "MODEL")]
    ner: Option<String>,

    /// Метки сущностей для --ner gliner (через запятую; zero-shot).
    /// Для реальных чекпойнтов GLiNER обязательно: «--ner-labels людина,місто».
    #[arg(long = "ner-labels", value_name = "LIST")]
    ner_labels: Option<String>,

    /// Самопроверка суверенного стека: синтетические .pqw-модели →
    /// полный цикл инференса (энкодер + GLiNER + GLM + MoE + Sha256)
    /// без сети, без внешних библиотек.
    #[arg(long = "pqw-selftest")]
    pqw_selftest: bool,

    /// Обратный In-Place компилятор: живая демонстрация переписывания
    /// тритов в .t5q без декомпрессии (STDP + фазовый ротор + commit).
    /// Без аргумента — синтетический поток; с путём — ваш .t5q-файл.
    #[arg(long = "t5q-compile", value_name = "FILE", num_args = 0..=1, default_missing_value = "")]
    t5q_compile: Option<String>,

    /// JIT-контур: обученные веса → машинный код x86_64 (ваги вшиты в
    /// інструкції як immediate) → виконання → Hebb-пластичність →
    /// перекомпіляція. Без аргумента — синтетический слой 32×32.
    #[arg(long = "jit-loop", value_name = "FILE", num_args = 0..=1, default_missing_value = "")]
    jit_loop: Option<String>,

    // ---------- Суверенный Гиппокамп: библиотека POLER → нативный индекс (v0.29) ----------

    /// ИНЖЕСТ БИБЛИОТЕКИ ЗНАНИЙ: PATH = корень POLER_ALL_GENERATED_DOCS
    /// (слои 01…06 распознаются автоматически). Секции → чанки с якорями
    /// и эпистемической разметкой (MVR ×1.5 / первоисточник ×1.0 /
    /// нарратив ×0.7) → полнотекстовый индекс (BM25/WebRank + Semantic
    /// Bridge) + опционально векторный слой (RaBitQ 144 Б/вектор + HNSW).
    #[arg(
        long = "knowledge-ingest",
        value_name = "PATH",
        conflicts_with_all = [
            "web", "crawl", "web_search", "web_stats", "mcp", "mcp_http", "shell", "exec",
            "tui", "impact", "grep", "chunk", "benchmark", "semantic", "license",
            "knowledge_search", "knowledge_stats"
        ]
    )]
    knowledge_ingest: Option<PathBuf>,

    /// Векторный слой инжеста: none (по умолчанию) | hash | pqw.
    /// pqw требует --model <path.pqw> (суверенные веса).
    #[arg(
        long = "knowledge-embedder",
        value_enum,
        default_value_t = KnowledgeEmbedderArg::None,
        requires = "knowledge_ingest"
    )]
    knowledge_embedder: KnowledgeEmbedderArg,

    /// ПОИСК ПО БИБЛИОТЕКЕ ЗНАНИЙ: гибрид BM25/WebRank + векторы (если
    /// индекс построен с векторным слоем), эпистемическая градация и
    /// фильтр достоверности. Хиты несут файл/строки/байты + прямую цитату.
    #[arg(
        long = "knowledge-search",
        value_name = "QUERY",
        conflicts_with_all = [
            "web", "crawl", "web_search", "web_stats", "mcp", "mcp_http", "shell", "exec",
            "tui", "impact", "grep", "chunk", "benchmark", "semantic", "license",
            "knowledge_ingest", "knowledge_stats"
        ]
    )]
    knowledge_search: Option<String>,

    /// Минимальный эпистемический статус хитов (с --knowledge-search):
    /// mvr | source | narrative.
    #[arg(long = "min-provenance", value_name = "LEVEL", requires = "knowledge_search")]
    min_provenance: Option<String>,

    /// Путь к БД знаний [default: ~/.local/share/poler-engine/knowledge.db].
    #[arg(long = "knowledge-db", value_name = "PATH")]
    knowledge_db: Option<PathBuf>,

    /// Статистика индекса знаний (JSON в stdout).
    #[arg(
        long = "knowledge-stats",
        conflicts_with_all = [
            "web_search", "crawl", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui",
            "impact", "grep", "chunk", "benchmark", "semantic", "license",
            "knowledge_ingest", "knowledge_search"
        ]
    )]
    knowledge_stats: bool,

    // ---------- Крипто-слой данных: POLER Vault (M4.5 CDL, фича pnd-ffi) ----------

    /// ЗАПЕЧАТАТЬ ДАННЫЕ «НАСМЕРТЬ» В ЗАШИФРОВАННЫЙ КОНТЕЙНЕР:
    /// PATH → PATH.pvt. Крипто-ядро PND v8.2 (CBC, 256-битный ключ из
    /// парольной фразы, ланцюговий MAC + внешний SHA-256). Контейнер
    /// синхронизируется через git/любой транспорт: без ключа — нечитаем,
    /// но проверяем на целостность (--memory-verify). Память O(1):
    /// файлы от 500 КБ до сотен ГБ.
    #[cfg(feature = "pnd-ffi")]
    #[arg(
        long = "memory-seal",
        value_name = "PATH",
        conflicts_with_all = [
            "web_search", "crawl", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui",
            "impact", "grep", "chunk", "benchmark", "semantic", "license",
            "knowledge_ingest", "knowledge_search", "knowledge_stats",
            "memory_open", "memory_verify", "memory_info"
        ]
    )]
    memory_seal: Option<PathBuf>,

    /// ВСКРЫТЬ КОНТЕЙНЕР: VAULT.pvt → открытый текст (проверка MAC
    /// и транспортного SHA-256 обязательна; неверный ключ = отказ).
    #[cfg(feature = "pnd-ffi")]
    #[arg(
        long = "memory-open",
        value_name = "VAULT",
        conflicts_with_all = [
            "web_search", "crawl", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui",
            "impact", "grep", "chunk", "benchmark", "semantic", "license",
            "knowledge_ingest", "knowledge_search", "knowledge_stats",
            "memory_seal", "memory_verify", "memory_info"
        ]
    )]
    memory_open: Option<PathBuf>,

    /// ПРОВЕРИТЬ КОНТЕЙНЕР БЕЗ КЛЮЧА: заголовок (FNV-1a64) + внешний
    /// SHA-256 шифротекста — битый git-sync/диск виден до расшифровки.
    #[cfg(feature = "pnd-ffi")]
    #[arg(
        long = "memory-verify",
        value_name = "VAULT",
        conflicts_with_all = [
            "web_search", "crawl", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui",
            "impact", "grep", "chunk", "benchmark", "semantic", "license",
            "knowledge_ingest", "knowledge_search", "knowledge_stats",
            "memory_seal", "memory_open", "memory_info"
        ]
    )]
    memory_verify: Option<PathBuf>,

    /// МЕТАДАННЫЕ КОНТЕЙНЕРА БЕЗ КЛЮЧА: версия, размеры, страницы,
    /// итерации KDF, флаги (для человека и агента).
    #[cfg(feature = "pnd-ffi")]
    #[arg(
        long = "memory-info",
        value_name = "VAULT",
        conflicts_with_all = [
            "web_search", "crawl", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui",
            "impact", "grep", "chunk", "benchmark", "semantic", "license",
            "knowledge_ingest", "knowledge_search", "knowledge_stats",
            "memory_seal", "memory_open", "memory_verify"
        ]
    )]
    memory_info: Option<PathBuf>,

    /// Выходной путь (с --memory-seal: PATH.pvt по умолчанию;
    /// с --memory-open: VAULT без .pvt / с суффиксом .out).
    #[cfg(feature = "pnd-ffi")]
    #[arg(long = "memory-out", value_name = "PATH")]
    memory_out: Option<PathBuf>,

    /// Имя переменной окружения с парольной фразой
    /// [default: POLER_VAULT_KEY]. Если переменной нет — одна строка
    /// читается из stdin (не попадает в историю shell).
    #[cfg(feature = "pnd-ffi")]
    #[arg(long = "memory-key-env", value_name = "VAR", default_value = "POLER_VAULT_KEY")]
    memory_key_env: String,

    /// Записать публичный контент-хеш открытого текста в заголовок
    /// (дедупликация/адресация между контейнерами; по умолчанию
    /// отключено — приватность: хеш позволяет сверять догадки о содержимом).
    #[cfg(feature = "pnd-ffi")]
    #[arg(long = "memory-content-id", requires = "memory_seal")]
    memory_content_id: bool,

    /// Итерации KDF при печати [default: 100000] (минимум 10000).
    #[cfg(feature = "pnd-ffi")]
    #[arg(
        long = "memory-kdf-iters",
        value_name = "N",
        default_value_t = poler_engine::crypto::kdf::DEFAULT_ITERATIONS,
        requires = "memory_seal"
    )]
    memory_kdf_iters: u32,

    /// AIDDE impact-анализ символа (call graph + upstream/downstream паспорт).
    #[arg(long)]
    impact: Option<String>,

    // ---------- E1/v0.31.0: полер-исполнитель команд (фича pnd-ffi) ----------

    /// ИДЕАЛЬНЫЙ ИСПОЛНИТЕЛЬ КОМАНД (ядро os/core/poler_exec.zig, raw-syscalls):
    /// запустить CMD с аргументами, жёстким таймаутом, лимитом захвата вывода
    /// (хвост) и гарантией отсутствия зомби. Вывод ребёнка — на наши stdout/stderr,
    /// код выхода — код ребёнка; таймаут — 124 (конвенция GNU timeout);
    /// не найдено — 127. Без шелл-парсинга — инъекции невозможны.
    /// ВАЖНО: все флаги (--exec-timeout-ms и др.) — ДО --exec; после --exec
    /// всё до конца строки — команда и её аргументы (включая -флаги команды).
    #[cfg(feature = "pnd-ffi")]
    #[arg(
        long = "exec-raw",
        value_name = "CMD ARGS...",
        num_args = 1..,
        allow_hyphen_values = true,
        conflicts_with_all = [
            "web_search", "crawl", "web_stats", "mcp", "mcp_http", "shell", "tui",
            "impact", "grep", "chunk", "benchmark", "semantic", "license",
            "knowledge_ingest", "knowledge_search", "knowledge_stats",
            "memory_seal", "memory_open", "memory_verify", "memory_info"
        ]
    )]
    /// (v0.62: флаг переименован --exec → --exec-raw: в v0.47 каноном
    /// стал полер-shell `--exec '<команда>'`; дубликат поля не собирался
    /// с фичей pnd-ffi с v0.47 — CI не доходил до этой сборки.)
    exec_raw: Vec<String>,

    /// Жёсткий таймаут команды --exec, мс [default: 30000].
    /// По истечении: SIGTERM группе → grace → SIGKILL.
    #[cfg(feature = "pnd-ffi")]
    #[arg(long = "exec-timeout-ms", value_name = "MS", default_value_t = 30_000)]
    exec_timeout_ms: u64,

    /// Grace между SIGTERM и SIGKILL при таймауте --exec, мс.
    #[cfg(feature = "pnd-ffi")]
    #[arg(long = "exec-grace-ms", value_name = "MS", default_value_t = 100)]
    exec_grace_ms: u64,

    /// Лимит захвата вывода НА ПОТОК для --exec, байт [default: 256 КиБ].
    /// Удерживается ХВОСТ вывода (кольцевой буфер), превышение — флаг
    /// truncated и заметка в stderr; O(1) памяти при любом объёме вывода.
    #[cfg(feature = "pnd-ffi")]
    #[arg(long = "exec-max-out", value_name = "BYTES", default_value_t = 262_144)]
    exec_max_out: usize,

    /// Данные в stdin команды --exec (строка целиком).
    #[cfg(feature = "pnd-ffi")]
    #[arg(long = "exec-stdin", value_name = "DATA")]
    exec_stdin: Option<String>,

    /// E2: рабочий каталог команды --exec (chdir в бутстрапе ребёнка;
    /// провал → код 125). Родитель многопоточен — сам chdir не делает.
    #[cfg(feature = "pnd-ffi")]
    #[arg(long = "exec-cwd", value_name = "DIR")]
    exec_cwd: Option<String>,

    /// E2: переменная окружения ребёнка KEY=VALUE (повторяемый флаг;
    /// без него наследуется окружение процесса).
    #[cfg(feature = "pnd-ffi")]
    #[arg(long = "exec-env", value_name = "KEY=VALUE")]
    exec_env: Vec<String>,

    /// E2: запустить под псевдотерминалом 200x50 (sudo/fzf/htop, isatty);
    /// stdout и stderr сливаются в один поток (природа PTY).
    #[cfg(feature = "pnd-ffi")]
    #[arg(long = "exec-pty", default_value_t = false)]
    exec_pty: bool,

    /// E2: режим захвата при переполнении --exec-max-out: tail — только
    /// хвост; head_tail — первые B/2 + маркер «dropped N» + последние B/2
    /// (стек-трейс в начале огромного лога больше не теряется).
    #[cfg(feature = "pnd-ffi")]
    #[arg(long = "exec-capture", value_name = "MODE", default_value = "tail")]
    exec_capture: String,

    /// Disk-backed таблица символов (SQLite) для AIDDE на гигантских
    /// кодовых базах: RAM ограничен пачками записи, BFS — индексами.
    #[arg(long = "impact-cache")]
    impact_cache: Option<PathBuf>,

    /// Переиспользовать существующую --impact-cache базу без
    /// перестройки (мгновенные повторные impact-запросы).
    #[arg(long = "impact-reuse", default_value_t = false)]
    impact_reuse: bool,

    /// Глубина BFS impact-анализа.
    #[arg(long = "impact-depth", default_value_t = 3)]
    impact_depth: usize,

    /// Коннектом FLYCSR1 (FlyWire v783): мозг мухи как матрица A —
    /// node/edge/khop/impact-запросы прямо из zstd-артефакта.
    #[arg(
        long = "connectome",
        value_name = "CSR_ZST",
        conflicts_with_all = [
            "grep", "chunk", "web", "crawl", "web_search", "web_stats", "mcp",
            "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens",
            "web_lens_install", "archive_list"
        ]
    )]
    connectome: Option<PathBuf>,

    /// Таблица нейронов (flywire_v783_nodes.bin): имена root_id в выводе
    /// и поиск нейрона по root_id (не только по индексу).
    #[arg(long = "connectome-nodes", value_name = "NODES_BIN", requires = "connectome")]
    connectome_nodes: Option<PathBuf>,

    /// Паспорт нейрона: степени, синаптическая масса, топ-рёбра.
    #[arg(long = "connectome-node", value_name = "IDX_OR_ROOT_ID", requires = "connectome")]
    connectome_node: Option<String>,

    /// Ребро U:V — вес, медиатор, знак и ротор J = A − Aᵀ пары.
    #[arg(long = "connectome-edge", value_name = "U:V", requires = "connectome")]
    connectome_edge: Option<String>,

    /// K-hop BFS потока сигнала от нейрона (глубина: --k-hop).
    #[arg(long = "connectome-khop", value_name = "IDX_OR_ROOT_ID", requires = "connectome")]
    connectome_khop: Option<String>,

    /// Входящие связи нейрона (CSC): «кто управляет» — impact-слой AIDDE.
    #[arg(long = "connectome-impact", value_name = "IDX_OR_ROOT_ID", requires = "connectome")]
    connectome_impact: Option<String>,

    /// Полный список партнёров нейрона (по весу; направление: --connectome-dir).
    #[arg(long = "connectome-neighbors", value_name = "IDX_OR_ROOT_ID", requires = "connectome")]
    connectome_neighbors: Option<String>,

    /// Кратчайший путь сигнала FROM:TO (BFS с цепочкой прыжков).
    #[arg(long = "connectome-path", value_name = "FROM:TO", requires = "connectome")]
    connectome_path: Option<String>,

    /// Общие партнёры набора нейронов, csv: a,b,c (направление: --connectome-dir).
    #[arg(long = "connectome-common", value_name = "A,B,...", requires = "connectome")]
    connectome_common: Option<String>,

    /// Хабы: топ степеней + PageRank (сколько: --connectome-top).
    #[arg(long = "connectome-centrality", requires = "connectome")]
    connectome_centrality: bool,

    /// Глобальный топ пар по циркуляции J = A − Aᵀ (K пар, порог: --connectome-min-abs).
    #[arg(long = "connectome-rotor-top", value_name = "K", requires = "connectome")]
    connectome_rotor_top: Option<usize>,

    /// Мотивы вокруг нейрона: реципрокные, feedforward, feedback.
    #[arg(long = "connectome-motifs", value_name = "IDX_OR_ROOT_ID", requires = "connectome")]
    connectome_motifs: Option<String>,

    /// Симуляция распространения сигнала от нейронов, csv-семена.
    #[arg(long = "connectome-propagate", value_name = "SEEDS", requires = "connectome")]
    connectome_propagate: Option<String>,

    /// Направление для --connectome-neighbors (out|in) и --connectome-common (down|up).
    #[arg(long = "connectome-dir", value_name = "DIR", default_value = "out", requires = "connectome")]
    connectome_dir: String,

    /// Лимит списков (--connectome-neighbors/--connectome-common).
    #[arg(long = "connectome-limit", value_name = "N", default_value_t = 20, requires = "connectome")]
    connectome_limit: usize,

    /// Размер топов (--connectome-centrality, --connectome-propagate).
    #[arg(long = "connectome-top", value_name = "N", default_value_t = 10, requires = "connectome")]
    connectome_top: usize,

    /// Порог циркуляции для --connectome-rotor-top.
    #[arg(long = "connectome-min-abs", value_name = "J", default_value_t = 1, requires = "connectome")]
    connectome_min_abs: i32,

    /// Шагов симуляции --connectome-propagate.
    #[arg(long = "connectome-steps", value_name = "N", default_value_t = 4, requires = "connectome")]
    connectome_steps: usize,

    /// Гейн симуляции (γ: разгон возбуждения).
    #[arg(long = "connectome-gamma", value_name = "G", default_value_t = 0.05, requires = "connectome")]
    connectome_gamma: f64,

    /// Утечка симуляции (leak: память состояния).
    #[arg(long = "connectome-leak", value_name = "L", default_value_t = 0.8, requires = "connectome")]
    connectome_leak: f64,

    /// Фильтр знака K-hop: all | exc | inh (поток по возбуждающим/тормозным).
    #[arg(long = "connectome-sign", value_name = "FILTER", default_value = "all", requires = "connectome")]
    connectome_sign: String,

    /// JSON-вывод режима --connectome (для агентов).
    #[arg(long = "connectome-json", requires = "connectome")]
    connectome_json: bool,

    // ── L1/v0.34.0: Литературный Двигатель POLER[Ψ] ────────────────

    /// Анализ поля интенции: замысел → Ω(o) + архетипы (фазы ℘→O).
    #[arg(
        long = "literary-field",
        value_name = "TEXT",
        conflicts_with_all = [
            "literary_generate", "grep", "chunk", "web", "crawl", "web_search",
            "web_stats", "mcp", "mcp_http", "shell", "exec", "tui", "impact",
            "browser_index", "web_lens", "web_lens_install", "archive_list",
            "connectome"
        ]
    )]
    literary_field: Option<String>,

    /// Генерация нарратива: полный прогон до H^Ψ = 0 (+ текст-разметка).
    #[arg(
        long = "literary-generate",
        value_name = "TEXT",
        conflicts_with_all = [
            "grep", "chunk", "web", "crawl", "web_search", "web_stats", "mcp",
            "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens",
            "web_lens_install", "archive_list", "connectome"
        ]
    )]
    literary_generate: Option<String>,

    /// Коннектом FLYCSR1 — мушиная калибровка двигателя (каста от семян:
    /// ротор J = A − Aᵀ закручивает нарратив, D = L·Lᵀ гасит пертурбации).
    #[arg(long = "literary-csr", value_name = "CSR_ZST")]
    literary_csr: Option<PathBuf>,

    /// Таблица root_id для --literary-csr (семена по root_id).
    #[arg(long = "literary-nodes", value_name = "NODES_BIN", requires = "literary_csr")]
    literary_nodes: Option<PathBuf>,

    /// Семена касты, csv: индексы или root_id [default: 0].
    #[arg(long = "literary-seeds", value_name = "A,B,...", default_value = "0")]
    literary_seeds: String,

    /// K-hop BFS касты [default: 2].
    #[arg(long = "literary-khop", value_name = "N", default_value_t = 2)]
    literary_khop: usize,

    /// Максимум нейронов в касте [default: 48, max 64].
    #[arg(long = "literary-max-cast", value_name = "N", default_value_t = 48)]
    literary_max_cast: usize,

    /// Осей фазового пространства [default: 64].
    #[arg(long = "literary-dims", value_name = "N", default_value_t = 64)]
    literary_dims: usize,

    /// Шагов генерации --literary-generate [default: 48].
    #[arg(long = "literary-steps", value_name = "N", default_value_t = 48)]
    literary_steps: usize,

    /// η — шаг интегратора [default: 0.1].
    #[arg(long = "literary-eta", value_name = "F", default_value_t = 0.1)]
    literary_eta: f64,

    /// η_r — резонансный шаг [default: 0.05].
    #[arg(long = "literary-eta-r", value_name = "F", default_value_t = 0.05)]
    literary_eta_r: f64,

    /// ρ — затухание темпорального эха R[n] [default: 0.9].
    #[arg(long = "literary-rho", value_name = "F", default_value_t = 0.9)]
    literary_rho: f64,

    /// κ — масштаб энергии значимости ε [default: 1.2].
    #[arg(long = "literary-kappa", value_name = "F", default_value_t = 1.2)]
    literary_kappa: f64,

    /// γ — баланс циркуляции J (мушиный ротор) [default: 1.0].
    #[arg(long = "literary-gamma", value_name = "F", default_value_t = 1.0)]
    literary_gamma: f64,

    /// λ — вес логической регуляризации [default: 0.01].
    #[arg(long = "literary-lambda", value_name = "F", default_value_t = 0.01)]
    literary_lambda: f64,

    /// No-Mul: Trit5-квантование латентного состояния p_t ({−1,0,+1},
    /// SIMD-скалярные произведения без f32-математики).
    #[arg(long = "literary-no-mul", default_value_t = false)]
    literary_no_mul: bool,

    /// JSON-вывод режима --literary-* (для агентов).
    #[arg(long = "literary-json", default_value_t = false)]
    literary_json: bool,

    // ── S1/v0.35.0: Синаптический Вихрь SSN ────────────────────────

    /// Прогон живого мозга: полный доказанный стек вихря, телеметрия.
    #[arg(long = "ssn-demo", default_value_t = false)]
    ssn_demo: bool,

    /// CSE-кодирование текста (с --ssn-encode-b — сходство пары).
    #[arg(long = "ssn-encode", value_name = "TEXT")]
    ssn_encode: Option<String>,

    /// Второй текст для сравнения с --ssn-encode.
    #[arg(long = "ssn-encode-b", value_name = "TEXT", requires = "ssn_encode")]
    ssn_encode_b: Option<String>,

    /// Сенсорная инъекция текста в живой мозг + прогон --ssn-steps.
    #[arg(long = "ssn-inject", value_name = "TEXT")]
    ssn_inject: Option<String>,

    /// Нейронов в мозге [default: 600].
    #[arg(long = "ssn-n", value_name = "N", default_value_t = 600)]
    ssn_n: usize,

    /// Виртуальных синаптических полей на нейрон [default: 16].
    #[arg(long = "ssn-fields", value_name = "N", default_value_t = 16)]
    ssn_fields: usize,

    /// Seed траектории мозга [default: 777].
    #[arg(long = "ssn-seed", value_name = "N", default_value_t = 777)]
    ssn_seed: u64,

    /// Размерность CSE-вектора сенсорики [default: 128].
    #[arg(long = "ssn-dims", value_name = "N", default_value_t = 128)]
    ssn_dims: usize,

    /// Шагов для --ssn-demo/--ssn-inject [default: 10000].
    #[arg(long = "ssn-steps", value_name = "N", default_value_t = 10_000)]
    ssn_steps: usize,

    /// Readout: топ-K активных нейронов [default: 10].
    #[arg(long = "ssn-readout", value_name = "K", default_value_t = 10)]
    ssn_readout: usize,

    /// JSON-вывод режима --ssn-* (для агентов).
    #[arg(long = "ssn-json", default_value_t = false)]
    ssn_json: bool,

    // ── S2/v0.36.0: Триединая Архитектура (муха + вихрь + кристалл) ──

    /// Витрина Триединства: муха + вихрь + кристалл → живая речь +
    /// моторные интенты + телеметрия всех трёх опор.
    #[arg(long = "triune-demo", default_value_t = false)]
    triune_demo: bool,

    /// Попросить Триединство говорить от промпта.
    #[arg(long = "triune-speak", value_name = "TEXT")]
    triune_speak: Option<String>,

    /// Токенов речи [default: 24].
    #[arg(long = "triune-tokens", value_name = "N", default_value_t = 24)]
    triune_tokens: usize,

    /// Seed Триединства [default: 777].
    #[arg(long = "triune-seed", value_name = "N", default_value_t = 777)]
    triune_seed: u64,

    /// Усиление ротора мухи γ (0 = муха спит) [default: 0.8].
    #[arg(long = "triune-gamma", value_name = "F", default_value_t = 0.8)]
    triune_gamma: f64,

    /// Настоящий мозг мухи: артефакт коннектома FLYCSR1 (.csr.zst).
    #[arg(long = "triune-connectome", value_name = "CSR_ZST")]
    triune_connectome: Option<PathBuf>,

    /// Семена касты для --triune-connectome (CSV индексов) [default: 1000,5000,9000].
    #[arg(long = "triune-seeds", value_name = "CSV", default_value = "1000,5000,9000")]
    triune_seeds: String,

    /// Внешний кристалл .t5c (по умолчанию — зашитый в бинарник).
    #[arg(long = "triune-crystal", value_name = "T5C")]
    triune_crystal: Option<PathBuf>,

    /// Сохранить кристалл ПОСЛЕ речи (синапсы, обученные в сессии) [v0.38.0].
    #[arg(long = "triune-out", value_name = "T5C")]
    triune_out: Option<PathBuf>,

    /// Выключить Хеббовскую пластичность во время речи [v0.38.0].
    #[arg(long = "triune-no-learn", default_value_t = false)]
    triune_no_learn: bool,

    /// Автономне безперервне навчання та еволюція пам'яті (секунди, наприклад: 300 = 5 хв).
    #[arg(long = "triune-auto-evolve", value_name = "SECONDS")]
    triune_auto_evolve: Option<u64>,

    /// Інспекція кристала пам'яті: статистика, токени, активні синапси.
    #[arg(long = "triune-crystal-info")]
    triune_crystal_info: bool,

    /// Інспекція синаптичних зв'язків конкретного слова в пам'яті (+1 / -1).
    #[arg(long = "triune-crystal-inspect", value_name = "WORD")]
    triune_crystal_inspect: Option<String>,

    /// JSON-вывод режима --triune-* (для агентов).
    #[arg(long = "triune-json", default_value_t = false)]
    triune_json: bool,

    // ── S2→E2/v0.38.1: моторный мост (MotorIntent → исполнение) ──

    /// ИСПОЛНЯТЬ моторные интенты речи: R1 (чтение/статус) — авто,
    /// M2 (сборка/пуш/GUI) — подтверждение [y/N] или --motor-yes.
    #[arg(long = "motor-act", default_value_t = false)]
    motor_act: bool,

    /// Авто-подтверждение мутаций M2 без вопросов (для неинтерактивных
    /// прогонов; ответственность на операторе — красный баннер при старте).
    #[arg(long = "motor-yes", default_value_t = false)]
    motor_yes: bool,

    /// Таймаут моторной команды, мс [default: 30000].
    #[arg(long = "motor-timeout-ms", value_name = "N", default_value_t = 30_000)]
    motor_timeout_ms: u64,

    /// Лимит вывода моторной команды, байт (голова+хвост) [default: 65536].
    #[arg(long = "motor-max-out", value_name = "N", default_value_t = 65_536)]
    motor_max_out: usize,

    /// Собрать кристалл знаний .t5c из корпуса.
    #[arg(long = "crystal-build", value_name = "CORPUS_TXT")]
    crystal_build: Option<PathBuf>,

    /// Потокова інгестия директорії з текстами/кодом в кристалл .t5c.
    #[arg(long = "crystal-ingest-dir", value_name = "DIR")]
    crystal_ingest_dir: Option<PathBuf>,

    /// Автогенерація розгорнутого x86_64 асемблерного алгоритму матриці архетипів (50 000+ рядків).
    #[arg(long = "gen-archetype-asm", value_name = "OUT_ASM_PATH")]
    gen_archetype_asm: Option<PathBuf>,

    /// Мінімальна кількість рядків асемблера для --gen-archetype-asm [default: 50000].
    #[arg(long = "gen-archetype-lines", value_name = "N", default_value_t = 50000)]
    gen_archetype_lines: usize,

    /// АВТОНОМНЫЙ ИНТЕРНЕТ-ИНЖЕКТОР ПАМЯТИ: краулить URL (глубина и
    /// лимиты наследуются из --crawl-*) и потоково выучить кристалл .t5c.
    #[arg(
        long = "learn-web",
        value_name = "URL",
        conflicts_with_all = [
            "crawl", "web_search", "web_stats", "browser_index", "learn_dir",
            "mcp", "mcp_http", "shell", "exec", "tui", "impact", "web_lens",
            "web_lens_install", "crystal_build", "crystal_ingest_dir",
        ]
    )]
    learn_web: Option<String>,

    /// Автономный инжектор памяти: рекурсивно выучить кристалл из папки
    /// (текстовые расширения, бинарники пропускаются).
    #[arg(
        long = "learn-dir",
        value_name = "DIR",
        conflicts_with_all = [
            "crawl", "web_search", "web_stats", "browser_index", "learn_web",
            "mcp", "mcp_http", "shell", "exec", "tui", "impact", "web_lens",
            "web_lens_install", "crystal_build", "crystal_ingest_dir",
        ]
    )]
    learn_dir: Option<PathBuf>,

    /// Размер чанка потокового чтения при обучении, байт [default: 65536].
    #[arg(long = "learn-chunk", value_name = "BYTES", default_value_t = 65_536)]
    learn_chunk: usize,

    /// Лимит уникальных слов в RAM при обучении (защита от OOM) [default: 262144].
    #[arg(long = "learn-word-cap", value_name = "N", default_value_t = 262_144)]
    learn_word_cap: usize,

    /// Лимит биграммных пар в RAM при обучении [default: 2097152].
    #[arg(long = "learn-bigram-cap", value_name = "N", default_value_t = 2_097_152)]
    learn_bigram_cap: usize,

    /// Словарь кристалла для --crystal-build / --crystal-ingest-dir /
    /// --learn-web / --learn-dir [default: 384; для обучения рекомендуем
    /// 4096..65536 — при 384 режимы learn автоматически поднимают до 4096].
    #[arg(long = "crystal-vocab", value_name = "N", default_value_t = 384)]
    crystal_vocab: usize,

    /// Выходной путь .t5c для --crystal-build / --learn-* [default: <корпус>.t5c / memory.t5c].
    #[arg(long = "crystal-out", value_name = "T5C")]
    crystal_out: Option<PathBuf>,

    // ---------- v0.39.0: Zero-Disk Streaming Ingestion Pipeline ----------

    /// Потоковое скачивание URL (HTTP/HTTPS) → сжатый .poler БЕЗ сырой
    /// выгрузки на диск (директива DIRECTIVE_STREAMING_INGESTION_PIPELINE).
    /// v0.65.0: контейнеры (tar.gz/tar.zst/gz/zst) распаковываются НА ЛЕТУ —
    /// исходники ложатся с пофайловой таблицей и zstd-сжатием (ансамбль).
    #[arg(
        long = "stream-download",
        value_name = "URL",
        conflicts_with_all = [
            "stream_file", "stream_bench", "browser_crawl", "archive_to_crystal",
            "poler_list", "poler_verify", "poler_extract",
            "grep", "chunk", "web", "crawl", "web_search", "web_stats", "mcp",
            "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens",
            "web_lens_install", "crystal_build", "crystal_ingest_dir", "learn_web",
            "learn_dir", "stream_quant",
        ]
    )]
    stream_download: Option<String>,

    /// Потоковое чтение локального файла (или '-' = stdin) → .poler.
    /// Тот же конвейер, что и --stream-download, без сети.
    #[arg(
        long = "stream-file",
        value_name = "FILE|-",
        conflicts_with_all = [
            "stream_download", "stream_bench", "browser_crawl", "archive_to_crystal",
            "poler_list", "poler_verify", "poler_extract",
            "grep", "chunk", "web", "crawl", "web_search", "web_stats", "mcp",
            "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens",
            "web_lens_install", "crystal_build", "crystal_ingest_dir", "learn_web",
            "learn_dir", "stream_quant",
        ]
    )]
    stream_file: Option<String>,

    /// Внутренний синтетический поток N байт (суффиксы K/M/G/T) → .poler:
    /// бенчмарк приёмочных критериев без занятия диска сырьём.
    #[arg(
        long = "stream-bench",
        value_name = "BYTES",
        value_parser = parse_byte_size,
        conflicts_with_all = [
            "stream_download", "stream_file", "browser_crawl", "archive_to_crystal",
            "poler_list", "poler_verify", "poler_extract",
            "grep", "chunk", "web", "crawl", "web_search", "web_stats", "mcp",
            "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens",
            "web_lens_install", "crystal_build", "crystal_ingest_dir", "learn_web",
            "learn_dir", "stream_quant",
        ]
    )]
    stream_bench: Option<u64>,

    /// v0.65.0 ПОСТОЯННЫЙ АРХИВАТОР: упаковка каталога в .poler с
    /// пофайловой таблицей (FastCDC + BLAKE3-дедуп между файлами + zstd).
    /// Сборки poler-engine пакуются этим путём (scripts/poler_build.sh):
    /// бинарник/артефакты/доки — в один сжатый .poler, сырые копии
    /// удаляются. Права 0o755 сохраняются — poler-box запускает
    /// бинарники прямо из архива (циклический ансамбль).
    /// WYSIWYG: пакуется ВСЁ содержимое каталога (вкл. скрытые);
    /// --pack-gitignore включает фильтр .gitignore+hidden (репо-чекаут).
    #[arg(
        long = "pack",
        value_name = "DIR",
        conflicts_with_all = [
            "stream_download", "stream_file", "stream_bench", "browser_crawl", "archive_to_crystal",
            "poler_list", "poler_verify", "poler_extract",
            "grep", "chunk", "web", "crawl", "web_search", "web_stats", "mcp",
            "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens",
            "web_lens_install", "crystal_build", "crystal_ingest_dir", "learn_web",
            "learn_dir", "stream_quant",
        ]
    )]
    pack: Option<PathBuf>,

    /// С --pack: уважать .gitignore и пропускать скрытые (режим
    /// репо-чекаута). По умолчанию WYSIWYG — всё содержимое каталога.
    #[arg(long = "pack-gitignore", default_value_t = false)]
    pack_gitignore: bool,

    /// v0.91.0 CORPUS-TRIT: КОРПУСНОЙ ПАКЕР — 150 ГБ текстов (код +
    /// литература) → один .poler. Профиль плотности: CDC 128К/512К/1М,
    /// Deep zstd-19, BLAKE3-дедуп между файлами; сжатие чанков БАТАМИ
    /// на rayon (--pack-jobs); классификация текстов (код/литература/
    /// данные/прочее) в статистике. Формат .poler v1 не меняется.
    /// WYSIWYG: пакуется ВСЁ содержимое; --pack-gitignore включает
    /// фильтр .gitignore+hidden.
    #[arg(
        long = "corpus-pack",
        value_name = "DIR",
        conflicts_with_all = [
            "stream_download", "stream_file", "stream_bench", "browser_crawl",
            "archive_to_crystal", "poler_list", "poler_verify", "poler_extract",
            "poler_stats", "pack", "grep", "chunk", "web", "crawl", "web_search",
            "web_stats", "mcp", "mcp_http", "shell", "exec", "tui", "impact",
            "browser_index", "web_lens", "web_lens_install", "crystal_build",
            "crystal_ingest_dir", "learn_web", "learn_dir", "stream_quant",
        ]
    )]
    corpus_pack: Option<PathBuf>,

    /// Потоки сжатия для --pack/--corpus-pack [default: 0 = все ядра].
    #[arg(long = "pack-jobs", value_name = "N", default_value_t = 0)]
    pack_jobs: usize,

    /// v0.92.0 STREAM-CORPUS: манифест юнитов (URL http(s) и/или локальные
    /// пути, по одному в строке, '#' — комментарий) → ОДИН .poler.
    /// Параллельный фетч (--stream-jobs), распаковка контейнеров на лету,
    /// пофайловая таблица; чекпоинты каждые 2000 юнитов / 210 с (валидный
    /// архив + state-sidecar); резюм по `.units.done` — повторный запуск
    /// докатывает только недостающее; доклейка НА МЕСТЕ (пик диска =
    /// архив, не 2×). Байты зависят только от порядка манифеста.
    #[arg(
        long = "stream-manifest",
        value_name = "FILE",
        conflicts_with_all = [
            "stream_download", "stream_file", "stream_bench", "pack", "corpus_pack",
            "poler_stats", "archive_to_crystal", "poler_list", "poler_verify",
            "poler_extract", "browser_crawl", "grep", "chunk", "web", "crawl",
            "web_search", "web_stats", "mcp", "mcp_http", "shell", "exec", "tui",
            "impact", "browser_index", "web_lens", "web_lens_install",
            "crystal_build", "crystal_ingest_dir", "learn_web", "learn_dir",
            "stream_quant",
        ]
    )]
    stream_manifest: Option<PathBuf>,

    /// Фетч-воркеры --stream-manifest (слот-машина: каждый держит ≤1 юнит)
    /// [default: 4]. На байты архива не влияет — только на скорость.
    #[arg(long = "stream-jobs", value_name = "N", default_value_t = 4)]
    stream_jobs: usize,

    /// Порог атомарности юнита --stream-manifest: ≤ — буферизуется целиком
    /// (сбой = чистый скип), > — стримится телом записи с zero-pad
    /// [default: 8 MiB].
    #[arg(
        long = "stream-unit-max",
        value_name = "BYTES",
        value_parser = parse_byte_size,
        default_value_t = 8 * 1024 * 1024
    )]
    stream_unit_max: u64,

    /// v0.91.0: манифест .poler-корпуса БЕЗ декомпрессии — таблица
    /// файлов, классы текстов (код/литература/данные), гистограмма
    /// расширений, дедуп, топ-файлы. Только метаданные контейнера.
    #[arg(
        long = "poler-stats",
        value_name = "POLER",
        conflicts_with_all = [
            "stream_download", "stream_file", "stream_bench", "browser_crawl",
            "archive_to_crystal", "poler_list", "poler_verify", "poler_extract",
            "corpus_pack", "pack", "grep", "chunk", "web", "crawl", "web_search",
            "web_stats", "mcp", "mcp_http", "shell", "exec", "tui", "impact",
            "browser_index", "web_lens", "web_lens_install", "crystal_build",
            "crystal_ingest_dir", "learn_web", "learn_dir", "stream_quant",
        ]
    )]
    poler_stats: Option<PathBuf>,

    /// Выходной .poler-архив для --stream-download/--stream-file/
    /// --stream-bench/--pack/--browser-crawl.
    #[arg(long = "output-archive", value_name = "POLER")]
    output_archive: Option<PathBuf>,

    /// Отключить BLAKE3-дедупликацию чанков (по умолчанию включена).
    #[arg(long = "no-dedup", default_value_t = false)]
    no_dedup: bool,

    /// Ярус сжатия чанков: fast (zstd-3) / deep (zstd-15) / auto [default: auto].
    #[arg(long = "stream-tier", value_enum, default_value_t = StreamTierArg::Auto)]
    stream_tier: StreamTierArg,

    /// HTTP/HTTPS-краул БЕЗ Chromium: страницы → ε-фильтр → кристалл .t5c
    /// (--ingest-to-crystal) и/или .poler (--output-archive).
    /// Глубина/лимиты наследуются из --crawl-* флагов.
    #[arg(
        long = "browser-crawl",
        value_name = "URL",
        conflicts_with_all = [
            "stream_download", "stream_file", "stream_bench", "archive_to_crystal",
            "poler_list", "poler_verify", "poler_extract",
            "grep", "chunk", "web", "crawl", "web_search", "web_stats", "mcp",
            "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens",
            "web_lens_install", "crystal_build", "crystal_ingest_dir", "learn_web",
            "learn_dir", "stream_quant",
        ]
    )]
    browser_crawl: Option<String>,

    /// Кристалл .t5c для --browser-crawl (потоковое обучение на лету).
    #[arg(long = "ingest-to-crystal", value_name = "T5C")]
    ingest_to_crystal: Option<PathBuf>,

    /// .poler → потоковое обучение кристалла .t5c без распаковки.
    #[arg(
        long = "archive-to-crystal",
        value_name = "POLER",
        conflicts_with_all = [
            "stream_download", "stream_file", "stream_bench", "browser_crawl",
            "poler_list", "poler_verify", "poler_extract",
            "grep", "chunk", "web", "crawl", "web_search", "web_stats", "mcp",
            "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens",
            "web_lens_install", "crystal_build", "crystal_ingest_dir", "learn_web",
            "learn_dir", "stream_quant",
        ]
    )]
    archive_to_crystal: Option<PathBuf>,

    /// Выходной .t5c для --archive-to-crystal [default: memory.t5c].
    #[arg(long = "crystal", value_name = "T5C")]
    crystal: Option<PathBuf>,

    /// Листинг .poler-контейнера (JSON: чанки/файлы/компрессия).
    #[arg(
        long = "poler-list",
        value_name = "POLER",
        conflicts_with_all = [
            "stream_download", "stream_file", "stream_bench", "browser_crawl",
            "archive_to_crystal", "poler_verify", "poler_extract",
            "grep", "chunk", "web", "crawl", "web_search", "web_stats", "mcp",
            "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens",
            "web_lens_install", "crystal_build", "crystal_ingest_dir", "learn_web",
            "learn_dir", "stream_quant",
        ]
    )]
    poler_list: Option<PathBuf>,

    /// Верификация .poler: SHA256 потока + каждой записи.
    #[arg(
        long = "poler-verify",
        value_name = "POLER",
        conflicts_with_all = [
            "stream_download", "stream_file", "stream_bench", "browser_crawl",
            "archive_to_crystal", "poler_list", "poler_extract",
            "grep", "chunk", "web", "crawl", "web_search", "web_stats", "mcp",
            "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens",
            "web_lens_install", "crystal_build", "crystal_ingest_dir", "learn_web",
            "learn_dir", "stream_quant",
        ]
    )]
    poler_verify: Option<PathBuf>,

    /// Распаковка .poler в каталог (--extract-dir) с sha256-контролем.
    #[arg(
        long = "poler-extract",
        value_name = "POLER",
        conflicts_with_all = [
            "stream_download", "stream_file", "stream_bench", "browser_crawl",
            "archive_to_crystal", "poler_list", "poler_verify",
            "grep", "chunk", "web", "crawl", "web_search", "web_stats", "mcp",
            "mcp_http", "shell", "exec", "tui", "impact", "browser_index", "web_lens",
            "web_lens_install", "crystal_build", "crystal_ingest_dir", "learn_web",
            "learn_dir", "stream_quant",
        ]
    )]
    poler_extract: Option<PathBuf>,

    /// Каталог распаковки для --poler-extract [default: poler-extracted].
    #[arg(long = "extract-dir", value_name = "DIR", default_value = "poler-extracted")]
    extract_dir: PathBuf,

    /// Запись .poler в stdout (байты) — для пайпов в grep/sed.
    #[arg(
        long = "poler-cat",
        value_name = "POLER",
        conflicts_with_all = [
            "stream_download", "stream_file", "stream_bench", "browser_crawl",
            "archive_to_crystal", "poler_list", "poler_verify", "poler_extract",
            "poler_patch", "poler_rollback", "poler_remux",
        ]
    )]
    poler_cat: Option<PathBuf>,

    /// In-place CoW-патч .poler по JSON-манифесту (--manifest).
    #[arg(
        long = "poler-patch",
        value_name = "POLER",
        conflicts_with_all = [
            "stream_download", "stream_file", "stream_bench", "browser_crawl",
            "archive_to_crystal", "poler_list", "poler_verify", "poler_extract",
            "poler_cat", "poler_rollback", "poler_remux",
        ]
    )]
    poler_patch: Option<PathBuf>,

    /// Откат последнего --poler-patch по .polerbak.
    #[arg(
        long = "poler-rollback",
        value_name = "POLER",
        conflicts_with_all = [
            "stream_download", "stream_file", "stream_bench", "browser_crawl",
            "archive_to_crystal", "poler_list", "poler_verify", "poler_extract",
            "poler_cat", "poler_patch", "poler_remux",
        ]
    )]
    poler_rollback: Option<PathBuf>,

    /// Ремукс .poler (tar.gz-блоб) в .poler с файловой таблицей (--output-archive).
    #[arg(
        long = "poler-remux",
        value_name = "POLER",
        conflicts_with_all = [
            "stream_download", "stream_file", "stream_bench", "browser_crawl",
            "archive_to_crystal", "poler_list", "poler_verify", "poler_extract",
            "poler_cat", "poler_patch", "poler_rollback",
        ]
    )]
    poler_remux: Option<PathBuf>,

    /// Имя записи для --poler-cat.
    #[arg(long = "file", value_name = "NAME")]
    poler_file_name: Option<String>,

    /// v0.41.0 poler-box: запуск запису .poler в ізольованій коробці без ОС
    /// (userns/mntns/pidns/netns + pivot_root + seccomp + губернатор RSS/CPU).
    #[arg(
        long = "poler-box",
        value_name = "POLER",
        conflicts_with_all = [
            "stream_download", "stream_file", "stream_bench", "browser_crawl",
            "archive_to_crystal", "poler_list", "poler_verify", "poler_extract",
            "poler_cat", "poler_patch", "poler_rollback", "poler_remux",
        ]
    )]
    poler_box: Option<PathBuf>,

    /// v0.42.0 winpe: виконати Windows PE32+ (AMD64) бінарник нативно (без Wine/VM).
    #[arg(long = "winexec", value_name = "FILE", conflicts_with_all = [
        "stream_download", "stream_file", "stream_bench", "browser_crawl",
        "archive_to_crystal", "poler_list", "poler_verify", "poler_extract",
        "poler_cat", "poler_patch", "poler_rollback", "poler_remux", "poler_box",
    ])]
    winexec: Option<PathBuf>,

    /// Запис архіву, що стає процесом коробки (для --poler-box).
    #[arg(long = "box-entry", value_name = "NAME")]
    box_entry: Option<String>,

    /// Аргументи payload (повторюваний, для --poler-box).
    #[arg(long = "box-arg", value_name = "ARG")]
    box_arg: Vec<String>,

    /// Мапа записів архіву в каталог коробки: PREFIX:DIR (повторюваний; за замовчуванням rootfs/:/).
    #[arg(long = "box-map", value_name = "PREFIX:DIR")]
    box_map: Vec<String>,

    /// Ліміт RSS дерева процесів, МБ (губернатор) [default: 512].
    #[arg(long = "box-rss-mb", value_name = "N", default_value_t = 512)]
    box_rss_mb: u64,

    /// Ліміт CPU-часу дерева, с (губернатор) [default: 60].
    #[arg(long = "box-cpu-s", value_name = "N", default_value_t = 60)]
    box_cpu_s: u64,

    /// Розмір tmpfs rootfs коробки, МБ [default: 512].
    #[arg(long = "box-tmpfs-mb", value_name = "N", default_value_t = 512)]
    box_tmpfs_mb: u64,

    /// Debug: без просторів імен/seccomp — лише губернатор.
    #[arg(long = "box-no-isolate")]
    box_no_isolate: bool,

    /// JSON-манифест для --poler-patch: {"replace":[{name,data|file}],
    /// "add":[...], "delete":[{name}]}.
    #[arg(long = "manifest", value_name = "JSON")]
    poler_manifest: Option<PathBuf>,

    /// Перезаписать существующий .polerbak при --poler-patch.
    #[arg(long = "force-bak")]
    force_bak: bool,

    /// Порог ε для --browser-crawl: ниже — больше шума [default: 1.0].
    #[arg(long = "min-epsilon", value_name = "F", default_value_t = 1.0)]
    min_epsilon: f32,

    /// Потоковое квантование весов на лету: сырец → .t5q (70B на диске 4 ГБ).
    #[arg(long = "stream-quant", value_name = "FILE|-")]
    stream_quant: Option<PathBuf>,

    /// Выходной путь .t5q [default: <вход>.t5q].
    #[arg(long = "stream-quant-out", value_name = "T5Q")]
    stream_quant_out: Option<PathBuf>,

    /// Значений на блок [default: 512].
    #[arg(long = "stream-quant-block", value_name = "N", default_value_t = 512)]
    stream_quant_block: usize,

    /// Доля топ-|w| тритов на блок (вакуум при <1) [default: 1.0].
    #[arg(long = "stream-quant-keep", value_name = "F", default_value_t = 1.0)]
    stream_quant_keep: f64,

    /// Вход — f16-LE (иначе f32-LE).
    #[arg(long = "stream-quant-f16", default_value_t = false)]
    stream_quant_f16: bool,

    /// Watcher-режим: инкрементальный рескан по mtime/size.
    #[arg(long)]
    watch: bool,

    /// Интервал watcher-опроса в секундах.
    #[arg(long = "interval-secs", default_value_t = 2)]
    interval: u64,

    /// Дифф-режим watcher: печатать только новые/появившиеся якоря.
    #[arg(long, default_value_t = false)]
    diff: bool,

    /// Число топ-результатов.
    #[arg(short = 't', long, default_value_t = 10)]
    top: usize,

    /// Коэффициент затухания IIR-резонанса φ ∈ [0.75, 0.90].
    #[arg(long, default_value_t = 0.85)]
    phi: f64,

    /// Масштабный коэффициент калибровки ε.
    #[arg(long, default_value_t = 1.0)]
    kappa: f64,

    /// Радиус токенного окна.
    #[arg(short = 'w', long, default_value_t = 40)]
    window: usize,

    /// Глубина K-hop обхода графа сущностей.
    #[arg(short = 'k', long = "k-hop", default_value_t = 2)]
    k_hop: usize,

    /// Временной фильтр графа (например: Т-23).
    #[arg(long)]
    metric: Option<String>,

    /// Формат вывода.
    #[arg(long, value_enum, default_value_t = Format::AiJson)]
    format: Format,

    /// Режим очистки PII.
    #[arg(long, value_enum, default_value_t = PiiArg::Mask)]
    pii: PiiArg,

    /// Режим накопления резонанса.
    #[arg(long = "resonance-mode", value_enum, default_value_t = ResonanceArg::Hits)]
    resonance: ResonanceArg,

    /// Intent-ранжирование код ↔ проза (v0.82.0).
    #[arg(long, value_enum, default_value_t = IntentArg::Auto)]
    intent: IntentArg,

    /// POLER[Ψ]: η — скорость обучения внимания [default: 0.05].
    #[arg(long = "psi-eta", default_value_t = 0.05)]
    psi_eta: f64,

    /// POLER[Ψ]: γ — вес резонансного члена ∇ε [default: 0.5].
    #[arg(long = "psi-gamma", default_value_t = 0.5)]
    psi_gamma: f64,

    /// POLER[Ψ]: ρ — затухание резонансной памяти [default: 0.9].
    #[arg(long = "psi-rho", default_value_t = 0.9)]
    psi_rho: f64,

    /// POLER[Ψ]: K — глубина резонансной памяти [default: 8].
    #[arg(long = "psi-depth", default_value_t = 8)]
    psi_depth: usize,

    /// POLER-цикл (P3_Engine): η — learning rate [default: 0.01].
    #[arg(long = "poler-eta", default_value_t = 0.01)]
    poler_eta: f64,

    /// POLER-цикл: γ — резонансная связь [default: 0.1].
    #[arg(long = "poler-gamma", default_value_t = 0.1)]
    poler_gamma: f64,

    /// POLER-цикл: mix — CORDIC-квантовая нормализация [default: 0.1].
    #[arg(long = "poler-mix", default_value_t = 0.1)]
    poler_mix: f64,

    /// POLER-цикл: d — диссипатор D=LLᵀ (энтропийный горел) [default: 0.02].
    #[arg(long = "poler-dissipator", default_value_t = 0.02)]
    poler_dissipator: f64,

    /// ε по статистикам файла вместо корпуса.
    #[arg(long, default_value_t = false)]
    local_stats: bool,

    /// Сканируемые расширения (через запятую).
    #[arg(long, default_value = DEFAULT_EXTENSIONS)]
    extensions: String,

    /// Пропускать файлы больше N мегабайт.
    #[arg(long = "max-file-size", default_value_t = 64)]
    max_file_mb: u64,

    /// Максимум байт enclosing_scope в якоре.
    #[arg(long = "max-scope", default_value_t = 16384)]
    max_scope: usize,

    /// Максимум K-hop отношений на якорь.
    #[arg(long = "max-relations", default_value_t = 64)]
    max_relations: usize,

    /// Бюджет рёбер графа сущностей.
    #[arg(long = "max-graph-triples", default_value_t = 200_000)]
    max_graph_triples: usize,

    /// Показывать скрытые файлы/директории (rg --hidden).
    #[arg(long, default_value_t = false)]
    hidden: bool,

    /// Экспорт графа сущностей в SQL-файл (схема super-z memory_graph).
    #[arg(long = "graph-export")]
    graph_export: Option<PathBuf>,

    /// Число потоков rayon (по умолчанию — все ядра).
    #[arg(long)]
    threads: Option<usize>,

    /// Подробная статистика прогона в stderr.
    #[arg(short = 'v', long)]
    verbose: bool,
}

fn print_result(res: &SearchResult, format: Format) {
    match format {
        Format::AiJson => {
            println!("{}", serde_json::to_string_pretty(res).unwrap_or_default());
        }
        Format::Md => print!("{}", render_markdown(res)),
        Format::Simple => print!("{}", render_simple(res)),
    }
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

/// Человекочитаемый отчёт Benchmark Suite (v0.21, Задача 4).
fn print_bench_report(res: &poler_engine::bench::BenchResults) {
    print!("{}", poler_engine::bench::report_text(res));
}

/// Вывод результатов веб-поиска в трёх форматах.
fn print_web_hits(
    hits: &[poler_engine::web::WebHit],
    query: &str,
    format: Format,
    expansion: Option<&poler_engine::retrieval::QueryExpansion>,
) {
    use std::io::Write;
    match format {
        Format::AiJson => {
            let mut out = serde_json::json!({
                "engine": "poler-engine",
                "mode": "web-search",
                "rank": "POLER WebRank v1 (0.55·BM25 + 0.15·PageRank + 0.20·title + 0.10·ε-density)",
                "query": query,
                "total": hits.len(),
                "results": hits,
            });
            if let Some(exp) = expansion {
                if !exp.is_empty() {
                    out["semantic_bridge"] = serde_json::to_value(exp).unwrap_or_default();
                }
            }
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        }
        Format::Simple => {
            // WHY — в stderr: stdout остаётся чистым для пайпа
            if let Some(exp) = expansion {
                if !exp.is_empty() {
                    eprintln!("{}", exp.why());
                }
            }
            for h in hits {
                println!("{:.4}  {}  {}", h.score, h.url, h.title);
            }
        }
        Format::Md => {
            println!("# Веб-поиск: «{query}»\n");
            if let Some(exp) = expansion {
                if !exp.is_empty() {
                    println!("## Semantic Bridge (WHY)\n");
                    for l in exp.why_lines() {
                        println!("- {l}");
                    }
                    println!();
                }
            }
            for (i, h) in hits.iter().enumerate() {
                println!("## {}. {}\n", i + 1, if h.title.is_empty() { &h.url } else { &h.title });
                println!("- URL: {}", h.url);
                let phrase_note = if h.phrase_occ > 0 {
                    format!(", фраз=·{}", h.phrase_occ)
                } else {
                    String::new()
                };
                println!("- Score: {:.4} (bm25={:.3}, pagerank={:.5}, title={:.2}, ε={:.5}{})", h.score, h.bm25, h.pagerank, h.title_frac, h.density, phrase_note);
                println!("- Язык: {}, токенов: {}\n", if h.lang.is_empty() { "-" } else { &h.lang }, h.doclen);
                println!("> {}\n", h.snippet);
            }
        }
    }
    let _ = std::io::stdout().flush();
}

/// v0.38.0: Автономный запуск браузерного ядра POLER
fn run_browser_url(url: &str, out_file: Option<&std::path::Path>) -> i32 {
    use poler_engine::browser::{BrowserConfig, BrowserWindow};
    use std::time::Instant;

    let t0 = Instant::now();
    println!("poler-browser: загрузка {}", url);

    let html_content = if url.starts_with("http://") || url.starts_with("https://") {
        match poler_engine::web::cdp_fetcher_with_timeout(9222, 500, 10000) {
            Ok(mut fetcher) => {
                use poler_engine::web::PageFetcher;
                match fetcher.fetch_raw(url) {
                    Ok((_code, body)) => body,
                    Err(_) => match fetcher.fetch(url) {
                        Ok(page) => page.text,
                        Err(e) => {
                            eprintln!("poler-browser: ошибка CDP fetch: {e}");
                            return 2;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("poler-browser: не удалось подключиться к веб-рантайму: {e}");
                return 2;
            }
        }
    } else if std::path::Path::new(url).exists() {
        match std::fs::read_to_string(url) {
            Ok(body) => body,
            Err(e) => {
                eprintln!("poler-browser: ошибка чтения локального файла: {e}");
                return 2;
            }
        }
    } else {
        eprintln!("poler-browser: неподдерживаемый протокол или путь: {url}");
        return 2;
    };


    let mut window = BrowserWindow::new(BrowserConfig::default());
    window.open_tab(url);
    window.load_html(&html_content);

    let active_text = window.get_active_text();
    let title = &window.tabs[window.active_tab_idx].title;

    println!("poler-browser: [Заголовок] {}", title);
    println!("poler-browser: [Чистый текст] {} символов (обработано за {} мс)", active_text.len(), t0.elapsed().as_millis());

    if let Some(out) = out_file {
        if let Err(e) = std::fs::write(out, active_text) {
            eprintln!("poler-browser: ошибка сохранения: {e}");
            return 2;
        }
        println!("poler-browser: сохранено в {}", out.display());
    } else {
        println!("\n--- Содержимое (ε-очищенное) ---\n{}", active_text.lines().take(30).collect::<Vec<_>>().join("\n"));
        if active_text.lines().count() > 30 {
            println!("\n... [остальной текст скрыт, используй --browser-out для сохранения]");
        }
    }

    0
}

/// v0.18.0 → v2.0: человекочитаемый статус лицензии (`--license`).
fn print_license_status() -> ExitCode {
    print!("{}", poler_engine::license::status_text());
    ExitCode::SUCCESS
}

/// AI-API-реформа: самодокументирующийся JSON-манифест (`--schema`).
/// Одна команда раскрывает движок целиком: домены CLI, MCP-инструменты,
/// честные границы вычислимости, копируемые примеры. Скилл-доки и --help
/// исторически занижали поверхность (6–8 инструментов против 32 живых) —
/// манифест закрывает этот разрыв навсегда: модель читает один JSON.
fn print_schema_manifest() -> ExitCode {
    println!(
        r#"{{"engine":"poler-engine","flag":"--schema","purpose":"самодокументирующийся манифест для LLM/агентов",
"cli_domains":[
 {{"name":"search","call":"poler-engine <путь> -q '<запрос>' --format simple|md|ai-json","description":"топографический резонансный поиск: полные сцены вместо строк, метрики плотности eps и резонанса R"}},
 {{"name":"grep","call":"poler-engine <путь> --grep '<паттерн>' [--grep-regex] [--grep-i] [--grep-json]","description":"точный grep-режим: ВСЕ совпадения, контекст -B/-A, без индекса"}},
 {{"name":"calc","call":"poler-engine --exec 'calc <выражение>' --json","description":"Калькулятор Всего: арифметика, единицы (to), solve, матрицы det/inv/expm/eigen/eigen_sturm, числа, CODATA-константы"}},
 {{"name":"quantum","call":"poler-engine --mcp -> tools/call poler_quantum","description":"квантовый мост: bell|ghz|qft|iqft|grover|bv|dj|period|teleport + qaoa/bloch/verify, n<=26, шумовые пресеты ibm-heron/google-willow"}},
 {{"name":"neural","call":"poler-engine --triune-speak '<текст>' [--triune-tokens N]","description":"живая нейросеть Триединства: муха + синусоидный вихрь + троичный кристалл; STDP-обучение, критичность ~0.55"}},
 {{"name":"connectome","call":"poler-engine --connectome <CSR_ZST> ...","description":"живой мозг мухи FLYCSR1: паспорта нейронов, K-hop, пути сигнала, PageRank-хабы, ротор J=A-A^T"}},
 {{"name":"mcp","call":"poler-engine --mcp","description":"MCP-сервер stdio JSON-RPC 2.0: 32+ инструментов (живой список: tools/list)"}},
 {{"name":"web","call":"poler-engine --crawl <URL> / --web-search '<запрос>'","description":"локальный веб-индекс: CDP-краулинг real-Chromium, PageRank, SimHash-дедуп (офлайн, без Google)"}},
 {{"name":"rag","call":"poler-engine <файл> --chunk --chunk-size 500 --chunk-json","description":"нарезка на чанки с уважением границ предложений/абзацев"}},
 {{"name":"shell","call":"poler-engine --shell / --tui","description":"REPL и TUI-дашборд"}}
],
"mcp":{{"transport":"stdio JSON-RPC 2.0 (Model Context Protocol)","tools_count":"32+ (живой список через tools/list)","highlights":["poler_calc — Калькулятор Всего","poler_quantum — квантовый мост (9 алгоритмов + qaoa/teleport/bloch/verify)","poler_triune_speak/state — живая нейросеть","poler_fly_* — коннектом мухи (10 инструментов)","poler_ssn_* — живой SSN-мозг","poler_literary_* — литературный POLER[Psi]","poler_search/grep/chunk — retrieval","poler_box_exec — Docker-jail"]}},
"boundaries":{{"eigen":"надёжен n<=24; n>=28 — взрыв Уилкинсона (коэффициенты charpoly O(n!))","eigen_sturm":"НОВОЕ: Штурм-бисекция, симметричные вещественные, машинная точность ~3e-14 при n=256 (tridiag(2,-1,256): prod(lam)=det=257)","quantum":"n<=26 (плотный statevector 2^N x 16 байт; при RAM 3 ГБ — n<=24)","tridiag_builder":"n<=64 (компактный литерал); det/eigen_sturm матрицами — без лимита"}},
"quickstart":[
 "poler-engine --exec 'calc 2^10' --json",
 "poler-engine --exec 'calc det(tridiag(2,-1,64))' --json",
 "poler-engine --exec 'calc eigen_sturm(tridiag(2,-1,256))' --json",
 "poler-engine --triune-speak 'привет' --triune-tokens 24",
 "poler-engine ~/my-repo -q 'резонанс' --format ai-json",
 "poler-engine --schema | jq .boundaries"
]
}}"#
    );
    ExitCode::SUCCESS
}

fn main() -> ExitCode {

    // v0.41.0 poler-box: stage2-вхід — викликається exec-хелпером
    // /usr/bin/unshare -Ur -- <self>. МАЄ стояти до будь-якого парсингу argv.
    if std::env::var_os("POLER_BOX_STAGE2").is_some() {
        return ExitCode::from(poler_engine::boxenv::stage2_main() as u8);
    }

    // v0.24.0: hidden-вход PATH-shim медиации агентов — перехват ДО clap:
    // shim-обёртки (~/.poler-engine/shim/bash) вызывают именно его.
    // `poler-engine __gateway-shim <shell> [args…]` → судит payload тем же
    // sandbox-гейтом и либо exec реальный shell, либо отказ 126.
    let mut argv = std::env::args();
    let _prog = argv.next();
    if argv.next().as_deref() == Some("__gateway-shim") {
        let rest: Vec<String> = argv.collect();
        return ExitCode::from(poler_engine::gateway::shim::shim_main(&rest) as u8);
    }
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> ExitCode {

    if let Some(threads) = cli.threads {
        if let Err(e) = rayon::ThreadPoolBuilder::new()
            .num_threads(threads.max(1))
            .build_global()
        {
            eprintln!("poler-engine: не удалось настроить пул потоков: {e}");
            return ExitCode::from(2);
        }
    }

    // ---------- v0.38.0 poler-edit: сервер редактора и бенчмарк ядра ----------
    if cli.edit_serve {
        return ExitCode::from(poler_engine::editor::protocol::run_edit_server() as u8);
    }
    if let Some(file) = &cli.edit_bench {
        return ExitCode::from(poler_engine::editor::bench::run_edit_bench(
            file,
            &cli.edit_bench_query,
        ) as u8);
    }

    // ---------- v2.0 Part E/F: суверенный ML-инференс через pqc ----------
    // Никакого ONNX Runtime: энкодер/NER/LLM читают .pqw mmap'ом и
    // считаются нативными SIMD-кернелами внутри этого бинарника.
    if cli.pqw_selftest {
        return ExitCode::from(poler_engine::pqc::selftest::run_selftest() as u8);
    }
    if let Some(target) = &cli.t5q_compile {
        return ExitCode::from(run_t5q_compile(target) as u8);
    }
    if let Some(target) = &cli.jit_loop {
        return ExitCode::from(run_jit_loop(target) as u8);
    }
    if let Some(mode) = cli.semantic.as_deref() {
        return ExitCode::from(run_semantic_native(mode, &cli) as u8);
    }
    if let Some(mode) = cli.llm.as_deref() {
        return ExitCode::from(run_llm(mode, &cli) as u8);
    }
    if let Some(what) = cli.ner.as_deref() {
        return ExitCode::from(run_ner(what, &cli) as u8);
    }
    // ---------- v0.38.0: POLER Sovereign Browser ----------
    if let Some(url) = &cli.browser_url {
        return ExitCode::from(run_browser_url(url, cli.browser_out.as_deref()) as u8);
    }


    // ---------- Суверенный Гиппокамп: библиотека знаний POLER (v0.29) ----------

    if cli.knowledge_stats {
        return ExitCode::from(run_knowledge_stats(&cli) as u8);
    }
    if let Some(root) = &cli.knowledge_ingest {
        return ExitCode::from(run_knowledge_ingest(&cli, root) as u8);
    }
    if let Some(query) = &cli.knowledge_search {
        return ExitCode::from(run_knowledge_search(&cli, query) as u8);
    }

    // ---------- Крипто-слой данных: POLER Vault (M4.5 CDL) ----------
    #[cfg(feature = "pnd-ffi")]
    {
        if let Some(input) = &cli.memory_seal {
            return ExitCode::from(run_memory_seal(&cli, input) as u8);
        }
        if let Some(vault) = &cli.memory_open {
            return ExitCode::from(run_memory_open(&cli, vault) as u8);
        }
        if let Some(vault) = &cli.memory_verify {
            return ExitCode::from(run_memory_verify(&cli, vault) as u8);
        }
        if let Some(vault) = &cli.memory_info {
            return ExitCode::from(run_memory_info(&cli, vault) as u8);
        }
    }

    // ---------- E1/v0.31.0: полер-исполнитель команд ----------
    #[cfg(feature = "pnd-ffi")]
    if !cli.exec_raw.is_empty() {
        return ExitCode::from(run_exec(&cli) as u8);
    }

    // ---------- MCP-сервер: stdio JSON-RPC для LLM-агентов ----------
    if let Some(n) = cli.mcp_bench {
        return ExitCode::from(run_mcp_bench(&cli, n) as u8);
    }

    // ---------- M6: шифропоток журнала (--vault-log с --mcp/--mcp-http) ----------
    if cli.vault_log.is_some() && !cli.mcp && cli.mcp_http.is_none() {
        eprintln!("poler-vault-log: --vault-log работает вместе с --mcp или --mcp-http");
        return ExitCode::from(2);
    }

    if cli.mcp {
        let db = cli.web_db.clone().unwrap_or_else(poler_engine::web::default_db_path);
        let code = poler_engine::mcp::run(
            cli.cdp_port,
            cli.web_wait_ms,
            db,
            cli.knowledge_db.clone(),
            mcp_server_options(&cli),
        );
        return ExitCode::from(code as u8);
    }

    // ---------- v0.19.0: WebLens — браузерный режим движка ----------
    // Материализует расширение MV3 (вшито в бинарник), запускает оконный
    // Chromium с уже установленным WebLens (--load-extension — автоустановка)
    // и держит MCP-сервер на localhost: Ctrl+C останавливает демона,
    // окно браузера живёт своей жизнью.
    if let Some(bind) = cli.web_lens.clone() {
        let db = cli.web_db.clone().unwrap_or_else(poler_engine::web::default_db_path);
        let token = match poler_engine::web::weblens::weblens_token() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("poler-weblens: токен: {e}");
                return ExitCode::from(2);
            }
        };
        // «8765» → «127.0.0.1:8765» (тот же канон, что у --mcp-http)
        let bind_addr = match bind.parse::<u16>() {
            Ok(port) => format!("127.0.0.1:{port}"),
            Err(_) => bind.clone(),
        };
        let endpoint = format!("http://{bind_addr}/");
        let ext_dir = match poler_engine::web::weblens::materialize(&endpoint, &token) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("poler-weblens: материализация: {e}");
                return ExitCode::from(2);
            }
        };
        eprintln!("poler-weblens: расширение готово: {}", ext_dir.display());
        // оконный браузер с WebLens; неудача (нет дисплея/браузера) —
        // не фатально: демон продолжает serve, расширение можно поставить
        // в свой браузер (--web-lens-install)
        match poler_engine::web::weblens::spawn_windowed_browser(&ext_dir, "about:blank") {
            Ok(_child) => {
                eprintln!("poler-weblens: браузер запущен с WebLens — Alt+P открывает панель");
            }
            Err(e) => {
                eprintln!("poler-weblens: оконный браузер не запущен: {e}");
            }
        }
        let code = poler_engine::mcp_http::run_http(&bind, &token, cli.cdp_port, cli.web_wait_ms, db, cli.knowledge_db.clone(), mcp_server_options(&cli));
        return ExitCode::from(code as u8);
    }

    // ---------- --web-lens-install: WebLens в ЕЖЕДНЕВНЫЙ браузер ----------
    // chrome://-страницы автоматизировать нельзя (защита браузера) —
    // движок делает всё, что можно: файлы + конфиг + инструкция.
    if cli.web_lens_install {
        let token = match poler_engine::web::weblens::weblens_token() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("poler-weblens: токен: {e}");
                return ExitCode::from(2);
            }
        };
        let endpoint = "http://127.0.0.1:8765/".to_string();
        let dir = match poler_engine::web::weblens::materialize(&endpoint, &token) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("poler-weblens: {e}");
                return ExitCode::from(2);
            }
        };
        println!("WebLens материализован: {}", dir.display());
        println!();
        println!("Установка в свой браузер (один раз, ~30 секунд):");
        println!("  1. Откройте chrome://extensions (или edge://extensions, brave://extensions)");
        println!("  2. Включите «Режим разработчика» (переключатель справа сверху)");
        println!("  3. «Загрузить распакованное расширение» → выберите каталог:");
        println!("     {}", dir.display());
        println!("  4. Alt+P (или иконка POLER на панели) — боковая панель поиска");
        println!();
        println!("Демон движка (панель работает, пока он жив):");
        println!("  poler-engine --web-lens            # рекомендуемый режим: свой браузер + демон");
        println!("  poler-engine --mcp-http 8765 --mcp-token <токен>   # только демон");
        println!();
        println!("Токен уже вписан в config.json расширения: {endpoint}");
        return ExitCode::SUCCESS;
    }

    // ---------- MCP-сервер по HTTP: удалённый агент через туннель ----------
    if let Some(bind) = cli.mcp_http.clone() {
        let db = cli.web_db.clone().unwrap_or_else(poler_engine::web::default_db_path);
        let token = cli
            .mcp_token
            .clone()
            .or_else(|| std::env::var("POLER_MCP_TOKEN").ok())
            .unwrap_or_else(poler_engine::mcp_http::generate_token);
        let code = poler_engine::mcp_http::run_http(&bind, &token, cli.cdp_port, cli.web_wait_ms, db, cli.knowledge_db.clone(), poler_engine::mcp::McpServerOptions::default());
        return ExitCode::from(code as u8);
    }

    // ---------- v0.28.1: Листинг архива без распаковки ----------
    if let Some(archive) = cli.archive_list.clone() {
        return ExitCode::from(run_archive_list(&cli, &archive) as u8);
    }

    // ---------- v0.30.0: Коннектом FLYCSR1 — мозг мухи как матрица A ----------
    if let Some(csr) = cli.connectome.clone() {
        return ExitCode::from(run_connectome(&cli, &csr) as u8);
    }

    // ---------- L1/v0.34.0: Литературный Двигатель POLER[Ψ] ----------
    if cli.literary_field.is_some() || cli.literary_generate.is_some() {
        return ExitCode::from(run_literary(&cli) as u8);
    }

    // ---------- S1/v0.35.0: Синаптический Вихрь SSN ----------
    if cli.ssn_demo || cli.ssn_encode.is_some() || cli.ssn_inject.is_some() {
        return ExitCode::from(run_ssn(&cli) as u8);
    }

    // ---------- S2/v0.36.0: Триединая Архитектура ----------
    if cli.triune_crystal_info || cli.triune_crystal_inspect.is_some() {
        return ExitCode::from(run_triune_crystal_inspect(&cli) as u8);
    }
    if cli.triune_auto_evolve.is_some() || cli.triune_demo || cli.triune_speak.is_some() {
        return ExitCode::from(run_triune(&cli) as u8);
    }
    if cli.crystal_build.is_some() {
        return ExitCode::from(run_crystal_build(&cli) as u8);
    }
    if cli.crystal_ingest_dir.is_some() {
        return ExitCode::from(run_crystal_ingest_dir(&cli) as u8);
    }
    if cli.gen_archetype_asm.is_some() {
        return ExitCode::from(run_gen_archetype_asm(&cli) as u8);
    }
    if cli.learn_web.is_some() || cli.learn_dir.is_some() {
        return ExitCode::from(run_learn(&cli) as u8);
    }
    // ---------- v0.39.0: Zero-Disk Streaming Ingestion Pipeline ----------
    // v0.65.0: + --pack (постоянный архиватор сборки)
    // v0.91.0: + --corpus-pack (корпусной пакер CORPUS-TRIT)
    // v0.92.0: + --stream-manifest (много-URL стриминг STREAM-CORPUS)
    if cli.stream_download.is_some()
        || cli.stream_file.is_some()
        || cli.stream_bench.is_some()
        || cli.pack.is_some()
        || cli.corpus_pack.is_some()
        || cli.stream_manifest.is_some()
    {
        return ExitCode::from(run_stream_ingest(&cli) as u8);
    }
    if cli.browser_crawl.is_some() {
        return ExitCode::from(run_browser_crawl(&cli) as u8);
    }
    if cli.archive_to_crystal.is_some() {
        return ExitCode::from(run_archive_to_crystal(&cli) as u8);
    }
    if cli.poler_stats.is_some() {
        return ExitCode::from(run_poler_stats(&cli) as u8);
    }
    if cli.poler_list.is_some() || cli.poler_verify.is_some() || cli.poler_extract.is_some() {
        return ExitCode::from(run_poler_ops(&cli) as u8);
    }
    if let Some(arch) = cli.poler_cat.clone() {
        return ExitCode::from(run_poler_cat(&cli, &arch) as u8);
    }

    // ---------- v0.41.0 poler-box: циклічна обгортка виконання ----------
    // v0.42.0 winpe: прямий запуск Windows PE32+ на хості
    if let Some(pe_path) = cli.winexec.clone() {
        let args = cli.box_arg.clone();
        let code = match poler_engine::winpe::winexec_file(&pe_path, &args) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("winexec: {e}");
                126
            }
        };
        return ExitCode::from(code as u8);
    }

    if let Some(arch) = cli.poler_box.clone() {
        return ExitCode::from(run_poler_box(&cli, &arch) as u8);
    }
    if let Some(arch) = cli.poler_patch.clone() {
        return ExitCode::from(run_poler_patch(&cli, &arch) as u8);
    }
    if let Some(arch) = cli.poler_rollback.clone() {
        return match poler_engine::archive::patcher::rollback_archive(&arch) {
            Ok(rep) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&rep).unwrap_or_default()
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("poler-rollback: {e}");
                ExitCode::from(2)
            }
        };
    }
    if let Some(arch) = cli.poler_remux.clone() {
        return ExitCode::from(run_poler_remux(&cli, &arch) as u8);
    }
    if cli.stream_quant.is_some() {
        return ExitCode::from(run_stream_quant(&cli) as u8);
    }

    // ---------- v0.20.0: Native Retrieval — grep-режим (слой 0) ----------
    if let Some(pattern) = cli.grep.clone() {
        use poler_engine::retrieval as nr;
        let output = if cli.grep_count {
            nr::GrepOutput::Count
        } else if cli.grep_list {
            nr::GrepOutput::ListMatching
        } else if cli.grep_list_nonmatching {
            nr::GrepOutput::ListNonMatching
        } else {
            nr::GrepOutput::Content
        };
        // Корни поиска: позиционный PATH + повторяемые --grep-root
        // (multi-root workspace IDE: один процесс, rayon-параллелизм
        // по всем корням). PATH отсутствует → первый --grep-root или ".".
        let mut roots: Vec<std::path::PathBuf> = Vec::with_capacity(1 + cli.grep_roots.len());
        if let Some(p) = cli.path.clone() {
            roots.push(p);
        }
        for extra in &cli.grep_roots {
            roots.push(extra.clone());
        }
        if roots.is_empty() {
            roots.push(std::path::PathBuf::from("."));
        }
        // Дедупликация канонических корней: повторный корень не сканируется
        // дважды (IDE может добавить и корень, и его подпапку-подпроект).
        let mut seen: Vec<std::path::PathBuf> = Vec::with_capacity(roots.len());
        for r in roots {
            let canonical = std::fs::canonicalize(&r).unwrap_or(r);
            if !seen.contains(&canonical) {
                seen.push(canonical);
            }
        }
        let roots = seen;
        // Архивы: пароль резолвится ДО параллельного прогона — промпт
        // на TTY нельзя звать из rayon-воркеров.
        let archive_password = if cli.archives {
            match resolve_archive_password(&cli, &roots, cli.grep_hidden) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("poler-archive: {e}");
                    return ExitCode::from(2);
                }
            }
        } else {
            None
        };
        let config = nr::GrepConfig {
            pattern,
            mode: if cli.grep_regex { nr::GrepMode::Regex } else { nr::GrepMode::Literal },
            case_insensitive: cli.grep_ignore_case,
            before: cli.grep_before,
            after: cli.grep_after,
            max_count: cli.grep_max_count,
            output,
            include_hidden: cli.grep_hidden,
            respect_ignore: true,
            scan_archives: cli.archives,
            archive_password,
            archive_max_entry_bytes: cli.archive_max_entry_mb.saturating_mul(1024 * 1024),
        };
        match nr::grep_run(&roots, &config) {
            Ok(report) => {
                if cli.grep_json {
                    match serde_json::to_string_pretty(&report) {
                        Ok(json) => println!("{json}"),
                        Err(e) => {
                            eprintln!("poler-grep: сериализация JSON: {e}");
                            return ExitCode::from(2);
                        }
                    }
                } else {
                    print!("{}", nr::render_text(&report, nr::stdout_is_tty()));
                }
                for err in &report.stats.errors {
                    eprintln!("poler-grep: {err}");
                }
                // Ошибки обхода не меняют код (как у grep: trouble=2
                // только для фатальных). Совпадения решают 0/1.
                return ExitCode::from(report.exit_code() as u8);
            }
            Err(e) => {
                eprintln!("poler-grep: {e}");
                return ExitCode::from(2);
            }
        }
    }

    // ---------- v0.20.0: Native Retrieval — RAG-чанки (слой B) ----------
    if cli.chunk {
        use poler_engine::retrieval as nr;
        let path = cli.path.clone().unwrap_or_default();
        // v0.28.1: селектор «архив::запись» — чанки записи БЕЗ распаковки
        // (читается только указанная запись, bounded-буфер в памяти).
        if let Some((archive, entry)) =
            poler_engine::archive::split_virtual(&path.to_string_lossy())
        {
            if archive.exists() && poler_engine::archive::is_archive(&archive) {
                let limits = poler_engine::archive::ReadLimits {
                    max_entry_bytes: cli.archive_max_entry_mb.saturating_mul(1024 * 1024),
                };
                let roots = vec![archive.clone()];
                let password = match resolve_archive_password(&cli, &roots, false) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("poler-chunk: {e}");
                        return ExitCode::from(2);
                    }
                };
                let text = match poler_engine::archive::read_entry_text(
                    &archive,
                    &entry,
                    password.as_deref(),
                    &limits,
                ) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("poler-chunk: {e}");
                        return ExitCode::from(2);
                    }
                };
                let config = nr::ChunkConfig {
                    target_tokens: cli.chunk_size,
                    overlap_tokens: cli.chunk_overlap,
                    ..Default::default()
                };
                // Формат — по расширению записи (не архива).
                let format = nr::ChunkFormat::detect(std::path::Path::new(&entry));
                let report = nr::chunk_document(&text, format, &config);
                let display = poler_engine::archive::virtual_name(&archive, &entry);
                if cli.chunk_json {
                    match serde_json::to_string_pretty(&report) {
                        Ok(json) => println!("{json}"),
                        Err(e) => {
                            eprintln!("poler-chunk: сериализация JSON: {e}");
                            return ExitCode::from(2);
                        }
                    }
                } else {
                    print!("{}", nr::render_chunks_text(&report, &display));
                }
                return ExitCode::SUCCESS;
            }
            // «::» в имени, но левая часть — не архив: трактуем как
            // обычный путь (фолбэк, файл с «::» в имени — экзотика).
        }
        if !path.is_file() {
            eprintln!("poler-chunk: путь не файл (или не найден): {}", path.display());
            return ExitCode::from(2);
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("poler-chunk: прочитать {}: {e}", path.display());
                return ExitCode::from(2);
            }
        };
        let config = nr::ChunkConfig {
            target_tokens: cli.chunk_size,
            overlap_tokens: cli.chunk_overlap,
            ..Default::default()
        };
        let format = nr::ChunkFormat::detect(&path);
        let report = nr::chunk_document(&text, format, &config);
        if cli.chunk_json {
            match serde_json::to_string_pretty(&report) {
                Ok(json) => println!("{json}"),
                Err(e) => {
                    eprintln!("poler-chunk: сериализация JSON: {e}");
                    return ExitCode::from(2);
                }
            }
        } else {
            print!("{}", nr::render_chunks_text(&report, &path.display().to_string()));
        }
        return ExitCode::SUCCESS;
    }

    // ---------- Terminal Gateway: единый терминальный шлюз v0.22.0 ----------
    if cli.gateway {
        let db_path = cli.web_db.clone().unwrap_or_else(poler_engine::web::default_db_path);
        return poler_engine::gateway::run_gateway(db_path, cli.dangerously_allow_all);
    }

    // ---------- poler-shell: интерактивный терминал v0.15.0 ----------
    // ---------- v0.47.0: one-shot --exec для ИИ-агентов ----------
    // v0.74.0: интерактивный 3D/4D-визор молекул
    if let Some(mol) = cli.view_mol.as_deref() {
        return view_mol_command(mol, cli.mol_mode);
    }

    // v0.75.0: PDB-макромолекулы / докинг / визор комплекса
    if let Some(f) = cli.pdb_file.as_deref() {
        return pdb_command(f, cli.pdb_mode);
    }
    // ─── v0.76.0: --jit — внутренний компилятор (контур A) ─────────────────
    if let Some(mode) = cli.jit_mode {
        return jit_command(mode, cli.jit_rows, cli.jit_cols);
    }

    if let Some(args) = cli.dock.as_deref() {
        let [smi, file] = [args[0].as_str(), args[1].as_str()];
        return dock_command(
            smi,
            file,
            cli.pocket.as_deref().unwrap_or("auto"),
            cli.dock_out.as_deref(),
            cli.dock_runs,
            cli.dock_steps,
            cli.dock_induced,
        );
    }

    // ─── v0.77.0: --screen — ультра-скрининг библиотек (контур B) ────────
    if let Some(args) = cli.screen.as_deref() {
        let [lib, file] = [args[0].as_str(), args[1].as_str()];
        return screen_command(
            lib,
            file,
            cli.pocket.as_deref().unwrap_or("auto"),
            cli.screen_top,
            cli.screen_workers,
            cli.screen_out.as_deref(),
        );
    }

    // ─── v0.79.0: --curriculum — задачник Curriculum Learning (контур D) ─
    if let Some(g) = cli.curriculum.as_deref() {
        let grade = g.trim().parse::<u8>().ok();
        let grade = match grade {
            Some(n @ 5..=9) => Some(n),
            Some(0) => None,
            _ => {
                eprintln!("--curriculum: класс должен быть 5–9 (или пусто = все)");
                return ExitCode::FAILURE;
            }
        };
        return curriculum_command(grade, cli.curriculum_text);
    }
    // --pocket без --dock/--screen — явная ошибка (раньше требовал dock)
    if cli.pocket.is_some() && cli.dock.is_none() && cli.screen.is_none() {
        eprintln!("--pocket: работает вместе с --dock или --screen");
        return ExitCode::FAILURE;
    }
    if let Some(f) = cli.view_complex.as_deref() {
        return view_complex_command(f);
    }

    if let Some(cmd) = cli.exec.as_deref() {
        use std::time::Instant;
        let db_path =
            cli.web_db.clone().unwrap_or_else(poler_engine::web::default_db_path);
        let t0 = Instant::now();
        let mut state = poler_engine::shell::ShellState::new(db_path);
        let result = poler_engine::shell::dispatch(&mut state, cmd);
        let dt = t0.elapsed().as_millis();
        match result {
            poler_engine::shell::CmdResult::Quit => {
                if cli.exec_json {
                    println!(
                        "{}",
                        poler_engine::shell::agentenv::json_envelope(cmd, true, 0, dt, "")
                    );
                }
                return ExitCode::SUCCESS;
            }
            poler_engine::shell::CmdResult::Empty => {
                if cli.exec_json {
                    println!(
                        "{}",
                        poler_engine::shell::agentenv::json_envelope(cmd, true, 0, dt, "")
                    );
                }
                return ExitCode::SUCCESS;
            }
            poler_engine::shell::CmdResult::Done(out) => {
                if cli.exec_json {
                    // Критерий ошибки: маркер ❌ в начале вывода команды
                    let ok = !out.starts_with('❌') && !out.contains("неизвестная команда");
                    let code: u8 = if ok { 0 } else { 1 };
                    println!(
                        "{}",
                        poler_engine::shell::agentenv::json_envelope(cmd, ok, code, dt, &out)
                    );
                    return if ok { ExitCode::SUCCESS } else { ExitCode::from(1) };
                }
                if !out.is_empty() {
                    println!("{out}");
                }
                return ExitCode::SUCCESS;
            }
        }
    }

    if cli.shell {
        let db_path = cli.web_db.clone().unwrap_or_else(poler_engine::web::default_db_path);
        return poler_engine::shell::run_shell(db_path);
    }
    if cli.tui {
        let db_path = cli.web_db.clone().unwrap_or_else(poler_engine::web::default_db_path);
        return poler_engine::shell::run_tui(db_path);
    }

    // ---------- License / EULA: статус модели ----------
    if cli.license {
        return print_license_status();
    }

    // ---------- AI-API: самодокументирующийся манифест ----------
    if cli.schema {
        return print_schema_manifest();
    }

    // ---------- VAULT-ASM: микроядра сокровищницы (v0.87.0) ----------
    if cli.asm_info {
        poler_engine::asm::print_info();
        return ExitCode::SUCCESS;
    }
    if cli.asm_bench {
        return poler_engine::asm::run_bench(cli.asm_synapses, cli.asm_cycles);
    }

    // ---------- Веб-индекс: статистика ----------
    if cli.web_stats {
        let db = cli.web_db.clone().unwrap_or_else(poler_engine::web::default_db_path);
        return match poler_engine::web::WebIndex::open(&db) {
            Ok(ix) => {
                let mut st = match ix.stats() {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("poler-engine: {e}");
                        return ExitCode::from(2);
                    }
                };
                st.db_bytes = std::fs::metadata(&db).map(|m| m.len()).unwrap_or(0);
                println!("{}", serde_json::to_string_pretty(&st).unwrap_or_default());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("poler-engine: web-индекс {db:?}: {e}");
                ExitCode::from(2)
            }
        };
    }

    // ---------- Benchmark & Regression Suite (v0.21, Задача 4) ----------
    if cli.benchmark {
        let opts = poler_engine::bench::BenchOpts::default();
        match poler_engine::bench::run_suite(&opts) {
            Ok(res) => {
                print_bench_report(&res);
                if let Some(path) = &cli.benchmark_json {
                    match serde_json::to_string_pretty(&res) {
                        Ok(json) => match std::fs::write(path, json) {
                            Ok(()) => eprintln!("poler-engine: бенчмарк-отчёт → {}", path.display()),
                            Err(e) => {
                                eprintln!("poler-engine: запись {}: {e}", path.display());
                                return ExitCode::from(2);
                            }
                        },
                        Err(e) => {
                            eprintln!("poler-engine: сериализация отчёта: {e}");
                            return ExitCode::from(2);
                        }
                    }
                }
                // exit-код: golden-проверки полноты и моста обязаны быть зелёными
                return if res.exact.completeness_ok && res.lexical.golden_ok {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                };
            }
            Err(e) => {
                eprintln!("poler-engine benchmark: {e}");
                return ExitCode::from(2);
            }
        }
    }

    // ---------- poler_disk_harvester: суб-секундный сбор диска ----------
    if !cli.harvest_disk.is_empty() {
        use poler_engine::search::disk_harvester as dh;
        let Some(query) = cli.harvest_query.clone() else {
            eprintln!("poler-engine: --harvest-disk требует --harvest-query <PATTERN>");
            return ExitCode::from(2);
        };
        let cfg = dh::HarvestConfig {
            roots: cli.harvest_disk.clone(),
            query,
            regex_mode: cli.harvest_regex,
            out_path: cli
                .harvest_out
                .clone()
                .unwrap_or_else(|| std::path::PathBuf::from("-")),
            format: match cli.harvest_format {
                HarvestFormatArg::Markdown => dh::HarvestFormat::Markdown,
                HarvestFormatArg::Json => dh::HarvestFormat::Json,
                HarvestFormatArg::Corpus => dh::HarvestFormat::Corpus,
            },
            mode: match cli.harvest_mode {
                HarvestModeArg::Sections => dh::HarvestMode::Sections,
                HarvestModeArg::Files => dh::HarvestMode::Files,
            },
            context_lines: cli.harvest_context,
            max_files: cli.harvest_max_files,
            min_matches: cli.harvest_min_matches,
            max_file_bytes: cli.harvest_max_file_bytes,
            max_out_bytes: cli.harvest_max_out_bytes,
            threads: cli.harvest_threads,
            include_hidden: cli.harvest_hidden,
            no_ignore: cli.harvest_no_ignore,
            chunk_bytes: dh::DEFAULT_CHUNK,
        };
        match dh::run_harvest(&cfg) {
            Ok(stats) => {
                eprintln!("{}", stats.summary_line());
                if stats.read_errors > 0 {
                    eprintln!(
                        "poler_disk_harvester: ошибок чтения: {}",
                        stats.read_errors
                    );
                }
                if stats.files_selected == 0 {
                    return ExitCode::from(1);
                }
            }
            Err(e) => {
                eprintln!("poler-engine: {e}");
                return ExitCode::from(2);
            }
        }
        return ExitCode::SUCCESS;
    }

    // ---------- Semantic Bridge: диагностика расширения запроса ----------
    if let Some(query) = cli.semantic_expand.clone() {
        if query.trim().is_empty() {
            eprintln!("poler-engine: пустой --semantic-expand");
            return ExitCode::from(2);
        }
        let bridge = poler_engine::retrieval::SemanticBridge::offline();
        let terms = poler_engine::web::stem::tokenize_stem(&query);
        let expansion = bridge.expand(&terms);
        println!("Semantic Bridge — офлайн-сенсор кросс-языковых запросов (v0.21)");
        println!("Запрос: «{query}»");
        println!("Термы (стем-форма): {}", terms.join(", "));
        println!();
        if expansion.is_empty() {
            println!("Расширений нет: сенсор не знает этих термов —");
            println!("ранжирование останется чистым WebRank без кандидатов.");
        } else {
            println!("Расширения ({}):", expansion.expansions.len());
            for line in expansion.why_lines() {
                println!("  {line}");
            }
            println!();
            println!("{}", expansion.why());
        }
        return ExitCode::SUCCESS;
    }

    // ---------- Веб-поиск: по локальному веб-индексу ----------
    if let Some(query) = cli.web_search.clone() {
        if query.trim().is_empty() {
            eprintln!("poler-engine: пустой --web-search");
            return ExitCode::from(2);
        }
        let db = cli.web_db.clone().unwrap_or_else(poler_engine::web::default_db_path);
        let mut ix = match poler_engine::web::WebIndex::open(&db) {
            Ok(ix) => ix,
            Err(e) => {
                eprintln!("poler-engine: web-индекс {db:?}: {e}");
                return ExitCode::from(2);
            }
        };
        let bridge = poler_engine::retrieval::SemanticBridge::offline();
        let (hits, expansion) = match ix.search_with_bridge(&query, cli.top.max(1), &bridge) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("poler-engine: web-search: {e}");
                return ExitCode::from(2);
            }
        };
        if cli.verbose {
            eprintln!(
                "poler-engine web-search: «{query}» — {} результатов из {} страниц{}",
                hits.len(),
                ix.page_count(),
                if expansion.is_empty() {
                    String::new()
                } else {
                    format!(", bridge: +{} терма-кандидата", expansion.extra_terms().len())
                }
            );
        }
        print_web_hits(&hits, &query, cli.format, Some(&expansion));
        return if hits.is_empty() {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        };
    }

    // ---------- Краулинг: seed URL → веб-индекс ----------
    if cli.crawl {
        let Some(seed_path) = cli.path.clone() else {
            eprintln!("poler-engine: --crawl требует seed URL как PATH");
            return ExitCode::from(2);
        };
        let seed = seed_path.to_string_lossy().to_string();
        if !seed.starts_with("http://") && !seed.starts_with("https://") {
            eprintln!("poler-engine: --crawl ожидает URL (http(s)://...), получено: {seed}");
            return ExitCode::from(2);
        }
        let db = cli.web_db.clone().unwrap_or_else(poler_engine::web::default_db_path);
        let mut ix = match poler_engine::web::WebIndex::open(&db) {
            Ok(ix) => ix,
            Err(e) => {
                eprintln!("poler-engine: web-индекс {db:?}: {e}");
                return ExitCode::from(2);
            }
        };
        let mut fetcher = match poler_engine::web::cdp_fetcher_with_timeout(
            cli.cdp_port,
            cli.web_wait_ms,
            cli.crawl_page_timeout_ms,
        ) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("poler-engine: Chromium CDP (порт {}): {e}", cli.cdp_port);
                eprintln!("  автозапуск не удался: установите POLER_CHROME_BIN или запустите вручную:");
                eprintln!("  chrome --headless --remote-debugging-port={} --no-sandbox", cli.cdp_port);
                return ExitCode::from(2);
            }
        };
        let cfg = poler_engine::web::CrawlConfig {
            max_pages: cli.crawl_max.max(1),
            max_depth: cli.crawl_depth,
            delay_ms: cli.crawl_delay_ms,
            cross_site: cli.cross_site,
            wait_ms: cli.web_wait_ms,
            page_timeout_ms: cli.crawl_page_timeout_ms,
            respect_robots: true,
        };
        eprintln!("poler-crawl: seed {seed}, глубина ≤ {}, до {} страниц, база {db:?}", cfg.max_depth, cfg.max_pages);
        let stats = match poler_engine::web::crawl::crawl(&mut ix, &mut fetcher, &seed, &cfg, cli.verbose) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("poler-crawl: {e}");
                return ExitCode::from(2);
            }
        };
        println!("{}", serde_json::to_string_pretty(&stats).unwrap_or_default());
        eprintln!(
            "poler-crawl: готово — {} загружено, {} проиндексировано, {} без изменений, {} дубликатов, {} robots-запретов, {} ошибок, {} мс",
            stats.fetched,
            stats.indexed,
            stats.unchanged,
            stats.duplicates,
            stats.skipped_robots,
            stats.errors,
            stats.elapsed_ms
        );
        return ExitCode::SUCCESS;
    }

    // ---------- Индексация одной страницы: --browser-index <URL> ----------
    if let Some(url) = cli.browser_index.clone() {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            eprintln!("poler-engine: --browser-index ожидает URL (http(s)://...), получено: {url}");
            return ExitCode::from(2);
        }
        let db = cli.web_db.clone().unwrap_or_else(poler_engine::web::default_db_path);
        let mut ix = match poler_engine::web::WebIndex::open(&db) {
            Ok(ix) => ix,
            Err(e) => {
                eprintln!("poler-engine: web-индекс {db:?}: {e}");
                return ExitCode::from(2);
            }
        };
        let mut fetcher = match poler_engine::web::cdp_fetcher_with_timeout(
            cli.cdp_port,
            cli.web_wait_ms,
            cli.crawl_page_timeout_ms,
        ) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("poler-engine: Chromium CDP (порт {}): {e}", cli.cdp_port);
                return ExitCode::from(2);
            }
        };
        // одиночная страница: глубина 0; явная команда пользователя —
        // robots не блокирует, но честно фиксируется в notes
        let cfg = poler_engine::web::CrawlConfig {
            max_pages: 1,
            max_depth: 0,
            delay_ms: cli.crawl_delay_ms,
            cross_site: false,
            wait_ms: cli.web_wait_ms,
            page_timeout_ms: cli.crawl_page_timeout_ms,
            respect_robots: false,
        };
        let stats = match poler_engine::web::crawl::crawl(&mut ix, &mut fetcher, &url, &cfg, false) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("poler-browser-index: {e}");
                return ExitCode::from(2);
            }
        };
        println!("{}", serde_json::to_string_pretty(&stats).unwrap_or_default());
        for n in stats.notes.iter().take(5) {
            eprintln!("  • {n}");
        }
        if stats.indexed == 1 {
            eprintln!("poler-browser-index: страница в индексе — poler-engine --web-search \"запрос\"");
        } else if stats.unchanged == 1 {
            eprintln!("poler-browser-index: уже в индексе, контент не менялся (Percolator-lite)");
        } else if stats.duplicates == 1 {
            eprintln!("poler-browser-index: near-дубликат уже известной страницы (SimHash)");
        } else {
            eprintln!(
                "poler-browser-index: страница НЕ попала в индекс (ошибок: {}) — см. notes выше",
                stats.errors
            );
            return ExitCode::from(1);
        }
        return ExitCode::SUCCESS;
    }

    let Some(path_arg) = cli.path.clone() else {
        // Интерактивный терминал (и stdin, и stdout — TTY) → сразу TUI-дашборд.
        // Если хоть один поток пайп/редирект — честная справка (скрипты, CI, docker).
        let interactive = std::io::IsTerminal::is_terminal(&std::io::stdin())
            && std::io::IsTerminal::is_terminal(&std::io::stdout());
        if interactive {
            let db_path = cli.web_db.clone().unwrap_or_else(poler_engine::web::default_db_path);
            return poler_engine::shell::run_tui(db_path);
        }
        eprintln!("poler-engine: укажите PATH, --tui, --web-search <QUERY> или --crawl с seed URL");
        return ExitCode::from(2);
    };

    // ---------- Web-Native режим: рендер через Chromium CDP ----------
    let scan_target: PathBuf = if cli.web {
        let url = path_arg.to_string_lossy().to_string();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            eprintln!("poler-engine: --web ожидает URL (http(s)://...), получено: {url}");
            return ExitCode::from(2);
        }
        if let Err(e) = poler_engine::web::ensure_chromium(cli.cdp_port) {
            eprintln!("poler-engine web: {e}");
            return ExitCode::from(2);
        }
        match poler_engine::web::ingest_url(&url, cli.cdp_port, cli.web_wait_ms) {
            Ok(res) => {
                if cli.verbose {
                    eprintln!(
                        "poler-engine web: «{}» — {} байт текста, {} JSON API перехвачено",
                        res.title,
                        res.text_len,
                        res.json_files.len()
                    );
                }
                // директория кэша: сканируем текст страницы + все JSON
                poler_engine::web::web_cache_dir()
            }
            Err(e) => {
                eprintln!("poler-engine web: {e}");
                return ExitCode::from(2);
            }
        }
    } else {
        path_arg
    };

    if !cli.web && !scan_target.exists() {
        eprintln!("poler-engine: путь не найден: {}", scan_target.display());
        return ExitCode::from(2);
    }

    let config = EngineConfig {
        window_radius: cli.window,
        phi_decay: cli.phi,
        kappa: cli.kappa,
        top_n: cli.top,
        k_hop_depth: cli.k_hop,
        pii_mode: match cli.pii {
            PiiArg::Off => PiiMode::Off,
            PiiArg::Mask => PiiMode::Mask,
        },
        resonance_mode: match cli.resonance {
            ResonanceArg::Hits => ResonanceMode::Hits,
            ResonanceArg::Field => ResonanceMode::Field,
            ResonanceArg::Psi => ResonanceMode::Psi,
            ResonanceArg::Poler => ResonanceMode::Poler,
        },
        temporal_filter: cli.metric.clone(),
        local_stats: cli.local_stats,
        extensions: cli
            .extensions
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect(),
        max_file_bytes: cli.max_file_mb.saturating_mul(1024 * 1024),
        max_scope_bytes: cli.max_scope,
        max_relations: cli.max_relations,
        include_hidden: cli.hidden,
        graph_export: cli.graph_export.clone(),
        max_graph_triples: cli.max_graph_triples,
        psi_params: poler_engine::psi::PsiParams {
            eta: cli.psi_eta,
            gamma: cli.psi_gamma,
            rho: cli.psi_rho,
            memory_depth: cli.psi_depth,
        },
        poler_params: poler_engine::poler::PolerParams {
            eta: cli.poler_eta,
            gamma: cli.poler_gamma,
            mix: cli.poler_mix,
            dissipator: cli.poler_dissipator,
            ..poler_engine::poler::PolerParams::default()
        },
        intent_mode: match cli.intent {
            IntentArg::Auto => poler_engine::search::intent::IntentMode::Auto,
            IntentArg::Code => poler_engine::search::intent::IntentMode::Code,
            IntentArg::Prose => poler_engine::search::intent::IntentMode::Prose,
            IntentArg::Off => poler_engine::search::intent::IntentMode::Off,
        },
    };

    // ---------- Режим AIDDE: Impact Passport ----------
    if let Some(symbol) = &cli.impact {
        if cli.query.is_some() {
            eprintln!("poler-engine: --impact и --query взаимоисключающие");
            return ExitCode::from(2);
        }
        let files: Vec<PathBuf> = collect_files(&scan_target, &config)
            .into_iter()
            .filter(|p| poler_engine::detect_lang(p) != CodeLang::Plain)
            .collect();
        if cli.verbose {
            eprintln!("poler-engine AIDDE: кодовых файлов: {}", files.len());
        }
        // Два режима: ин-мемори (по умолчанию) или SQLite-хранилище
        // (--impact-cache path.db — для кодовых баз 65K+ файлов).
        if let Some(db_path) = &cli.impact_cache {
            // reuse-режим: существующая база не перестраивается
            let store = if cli.impact_reuse && db_path.exists() {
                match poler_engine::aidde::SymbolStore::open_existing(db_path) {
                    Ok((s, has_schema)) => {
                        if has_schema {
                            if cli.verbose {
                                eprintln!("poler-engine AIDDE: reuse базы {db_path:?}");
                            }
                            Some(s)
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                }
            } else {
                None
            };
            let store = match store {
                Some(s) => s,
                None => {
                    let mut s = match poler_engine::aidde::SymbolStore::open(db_path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("poler-engine: не удалось открыть базу {db_path:?}: {e}");
                            return ExitCode::from(2);
                        }
                    };
                    if let Err(e) = s.build(&files, config.max_file_bytes) {
                        eprintln!("poler-engine: ошибка построения таблицы: {e}");
                        return ExitCode::from(2);
                    }
                    s
                }
            };
            if cli.verbose {
                let (d, c) = store.stats();
                eprintln!("poler-engine AIDDE(sqlite): defs={d}, calls={c}");
            }
            match poler_engine::aidde::impact_analysis_sqlite(
                &store,
                symbol,
                cli.impact_depth,
                200,
            ) {
                Some(report) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).unwrap_or_default()
                    );
                    ExitCode::SUCCESS
                }
                None => {
                    eprintln!("poler-engine: символ не найден: {symbol}");
                    ExitCode::from(1)
                }
            }
        } else {
            let table = SymbolTable::build(&files, config.max_file_bytes);
            match impact_analysis(&table, symbol, cli.impact_depth, 200) {
                Some(report) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).unwrap_or_default()
                    );
                    ExitCode::SUCCESS
                }
                None => {
                    eprintln!("poler-engine: символ не найден: {symbol}");
                    ExitCode::from(1)
                }
            }
        }
    } else {
        // ---------- Режим поиска ----------
        let Some(query) = cli.query.clone() else {
            eprintln!("poler-engine: укажите --query <QUERY> или --impact <SYMBOL>");
            return ExitCode::from(2);
        };
        if query.trim().is_empty() {
            eprintln!("poler-engine: пустой запрос");
            return ExitCode::from(2);
        }

        if cli.watch {
            return watch_mode(cli, config, query, scan_target);
        }

        let mut engine = Engine::new(config, false);
        let (result, stats) = engine.scan(&scan_target, &query);
        if cli.verbose {
            eprintln!(
                "poler-engine: файлов просканировано={}, с совпадениями={}, токенов={}, \
                 хитов={}, узлов графа={}, рёбер={}, время={}мс",
                stats.files_scanned,
                stats.files_with_hits,
                stats.total_tokens,
                stats.total_hits,
                stats.graph_nodes,
                stats.graph_edges,
                stats.elapsed_ms
            );
        }
        // v0.83.0: честный флаг мягкого fallback — на stderr (stdout-контракт
        // JSON/simple/md не загрязняется); JSON несёт soft_fallback:true.
        // v0.85.0: две разновидности — префикс идентификатора (скобка,
        // v0.83.0) и гэп макро-атрибутов объявления типа (ГРАБЛЯ 43).
        // v0.86.0: третья — template-гэп (параметрический блок <...>
        // между template и class-key, ГРАБЛЯ 45, Boost.Spirit).
        if result.soft_fallback {
            match result.soft_fallback_kind.as_deref() {
                Some("type_decl_gap") => eprintln!(
                    "poler-engine: определение типа не найдено строгой фразой \
                     (макро-атрибут между class/struct и именем) — мягкий \
                     fallback с пропуском макро-атрибута"
                ),
                Some("template_decl_gap") => eprintln!(
                    "poler-engine: объявление шаблона не найдено строгой фразой \
                     (параметрический блок <...> между template и class-key) — \
                     мягкий fallback с пропуском блока"
                ),
                _ => eprintln!(
                    "poler-engine: точная сигнатура не найдена — мягкий fallback \
                     на префикс идентификатора без скобки"
                ),
            }
        }
        print_result(&result, cli.format);
        if result.total_hits == 0 {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        }
    }
}

/// Watcher-режим: первичный полный скан, затем инкрементальные rescans
/// по mtime/size; выход по Ctrl-C (SIGINT).
fn watch_mode(cli: Cli, config: EngineConfig, query: String, scan_target: PathBuf) -> ExitCode {
    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        let _ = ctrlc::set_handler(move || stop.store(true, Ordering::SeqCst));
    }

    let mut engine = Engine::new(config, true).with_diff(cli.diff);
    let (result, stats) = engine.scan(&scan_target, &query);
    if cli.verbose {
        eprintln!(
            "poler-engine watch: начальный скан — файлов={}, хитов={}, время={}мс",
            stats.files_scanned, stats.total_hits, stats.elapsed_ms
        );
    }
    print_result(&result, cli.format);

    let interval = cli.interval.max(1);
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_secs(interval));
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let (event, result, stats) = engine.rescan(&scan_target, &query);
        if event.is_empty() {
            continue;
        }
        eprintln!(
            "poler-engine watch: добавлено={}, изменено={}, удалено={} (файлов={}, хитов={}, время={}мс)",
            event.added.len(),
            event.changed.len(),
            event.removed.len(),
            stats.files_scanned,
            stats.total_hits,
            stats.elapsed_ms
        );
        print_result(&result, cli.format);
    }
    eprintln!("poler-engine watch: остановлено");
    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------
// v2.0 Part E/F: обработчики суверенного ML-инференса (pqc)
// ---------------------------------------------------------------------------

/// `--semantic dense --model X.pqw -q "…"`: нативный энкодер (BGE-M3-класс)
/// через pqc — mmap + Sha256 + weight-only int8/int4, без ONNX.
/// С `--semantic-corpus PATH` — живой семантический поиск: чанки корпуса
/// эмбеддятся тем же энкодером, ранжирование косинусом.
fn run_semantic_native(mode: &str, cli: &Cli) -> i32 {
    if mode != "dense" {
        eprintln!(
            "poler-engine: неизвестный --semantic режим {mode:?} (доступен: dense)"
        );
        return 2;
    }
    let Some(model_path) = &cli.model else {
        eprintln!(
            "poler-engine: --semantic dense требует --model <path.pqw>\n  \
             конвертер реальных весов: python3 scripts/convert_hf_to_pqw.py \
             --hf-dir <BGE-M3> --out models/bge-m3.pqw;\n  \
             проверить стек сейчас: poler-engine --pqw-selftest"
        );
        return 2;
    };
    let Some(query) = &cli.query else {
        eprintln!("poler-engine: --semantic dense требует -q <текст>");
        return 2;
    };
    use poler_engine::vectors::Embedder as _;
    let mut embedder = match poler_engine::vectors::pqw_bridge::PqwEmbedder::open(model_path) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("poler-engine: {e}");
            return 2;
        }
    };

    if let Some(corpus) = &cli.semantic_corpus {
        return run_semantic_corpus_search(
            &mut embedder,
            corpus,
            query,
            cli.semantic_limit,
            cli.semantic_max_chunks,
        );
    }

    let t0 = std::time::Instant::now();
    match embedder.embed_batch(&[query.as_str()]) {
        Ok(vs) => {
            let v = &vs[0];
            let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            let head: Vec<String> = v.iter().take(8).map(|x| format!("{x:.4}")).collect();
            let tok_info = match embedder.tokenizer() {
                Some(tk) => format!("unigram-токенизатор: {} кусков", tk.vocab_size()),
                None => "хэш-фолбэк (демо-модель без __tokenizer__)".to_string(),
            };
            println!(
                "pqw-native dense · dim={} · L2={norm:.4} · [{}] …",
                v.len(),
                head.join(" ")
            );
            println!(
                "модель: {} · {:.1} мс · {tok_info}",
                embedder.name(),
                t0.elapsed().as_secs_f64() * 1000.0
            );
            0
        }
        Err(e) => {
            eprintln!("poler-engine: {e}");
            2
        }
    }
}

/// Живой семантический поиск: корпус → чанки → эмбеддинги → косинус.
fn run_semantic_corpus_search(
    embedder: &mut poler_engine::vectors::pqw_bridge::PqwEmbedder,
    corpus: &std::path::Path,
    query: &str,
    limit: usize,
    max_chunks: usize,
) -> i32 {
    use poler_engine::vectors::Embedder as _;

    let t0 = std::time::Instant::now();
    let mut chunks: Vec<(String, String)> = Vec::new(); // (file, text)
    if poler_engine::archive::kind_of(corpus) == Some(poler_engine::archive::ArchiveKind::Poler) {
        // v0.91.0 CORPUS-TRIT: живой корпус — .poler-контейнер. Чанки
        // читаются read_range (декомпрессия чанков по требованию), БЕЗ
        // распаковки на диск; NUL-снифф отсеивает бинарные записи,
        // выборка — резервуар Vitter R (равномерно, RAM O(max_chunks)).
        match poler_archive_chunks(corpus, max_chunks, &mut chunks) {
            Ok((text_files, binary_files)) => {
                eprintln!(
                    "корпус: .poler {} → {} текстовых записей ({} бинарных пропущено) → {} чанков (резервуар ≤{})",
                    corpus.display(),
                    text_files,
                    binary_files,
                    chunks.len(),
                    max_chunks
                );
            }
            Err(e) => {
                eprintln!("poler-engine: {e}");
                return 2;
            }
        }
    } else {
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        if !collect_text_files(corpus, &mut files, 4096) {
            return 2;
        }
        if files.is_empty() {
            eprintln!("poler-engine: корпус пуст (текстовых файлов не найдено)");
            return 2;
        }

        // Чанки: абзацы склеиваются до ~400 символов, длинные режутся.
        for f in &files {
            let Ok(text) = std::fs::read_to_string(f) else { continue };
            for piece in text.split("\n\n") {
                let p = piece.trim();
                if p.chars().count() < 40 {
                    continue; // мусорные осколки пропускаем
                }
                if p.chars().count() <= 380 {
                    chunks.push((f.display().to_string(), p.to_string()));
                } else {
                    // жёсткая нарезка длинных абзацев (~150 токенов на чанк)
                    let mut start = 0usize;
                    let cs: Vec<char> = p.chars().collect();
                    while start < cs.len() {
                        let end = (start + 340).min(cs.len());
                        let s: String = cs[start..end].iter().collect();
                        chunks.push((f.display().to_string(), s));
                        start = end;
                    }
                }
            }
        }
        // Равномерная выборка до max_chunks (корпус может быть огромным).
        if chunks.len() > max_chunks {
            let step = chunks.len() as f64 / max_chunks as f64;
            let picked: Vec<(String, String)> = (0..max_chunks)
                .map(|i| chunks[(i as f64 * step) as usize].clone())
                .collect();
            eprintln!(
                "poler-engine: {} чанков → равномерная выборка {} (--semantic-max-chunks)",
                chunks.len(),
                max_chunks
            );
            chunks = picked;
        }
        eprintln!(
            "корпус: {} файлов → {} чанков · эмбеддинг…",
            files.len(),
            chunks.len()
        );
    }
    if chunks.is_empty() {
        eprintln!("poler-engine: в корпусе нет абзацев достаточной длины");
        return 2;
    }
    // Параллельно по чанкам (rayon): токенизация + forward — оба &self,
    // mmap-веса разделяются всеми потоками.
    let emb: &poler_engine::vectors::pqw_bridge::PqwEmbedder = &*embedder;
    let cap = emb.model().view().header().max_pos as usize;
    let cap = cap.saturating_sub(if emb.model().view().header().xlmr_positions() {
        2
    } else {
        0
    });
    let vectors: Vec<Vec<f32>> = {
        use rayon::prelude::*;
        let done = std::sync::atomic::AtomicUsize::new(0);
        let total = chunks.len();
        let r = chunks
            .par_iter()
            .map(|(_, t)| {
                let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if n % 25 == 0 {
                    eprintln!("  эмбеддинг: {n}/{total}");
                }
                let mut ids = emb.token_ids(t);
                if ids.len() > cap {
                    ids.truncate(cap);
                }
                emb.model().embed(&ids)
            })
            .collect::<Result<Vec<_>, String>>();
        match r {
            Ok(v) => v,
            Err(e) => {
                eprintln!("poler-engine: {e}");
                return 2;
            }
        }
    };
    let qv = match embedder.embed_batch(&[query]) {
        Ok(v) => v.into_iter().next().unwrap(),
        Err(e) => {
            eprintln!("poler-engine: {e}");
            return 2;
        }
    };

    let mut ranked: Vec<(usize, f32)> = vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (i, v.iter().zip(&qv).map(|(a, b)| a * b).sum::<f32>()))
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let dt = t0.elapsed().as_secs_f64();
    println!(
        "\nсемантический поиск · «{query}» · {} чанков за {dt:.1} с ({:.1} чанков/с)",
        chunks.len(),
        chunks.len() as f64 / dt.max(1e-9)
    );
    for (rank, &(i, score)) in ranked.iter().take(limit.max(1)).enumerate() {
        let (file, text) = &chunks[i];
        let snippet: String = text.chars().take(140).collect();
        println!(
            "\n{}. [cos {:.4}] {}",
            rank + 1,
            score,
            file
        );
        println!("   {snippet}…");
    }
    0
}

/// v0.91.0 CORPUS-TRIT: чанки текста ПРЯМО из .poler-архива — без
/// распаковки на диск. ГРАБЛЯ, пойманная тестом: raw_region режет
/// ФИЗИЧЕСКИЙ mmap архива, а raw_off/raw_len записей — координаты
/// ЛОГИЧЕСКОГО потока; правильный путь — read_range (декомпрессия
/// чанков по требованию, MADV_DONTNEED после — RSS не копится).
/// NUL-снифф первых 4 КиБ отсеивает бинарные записи; длинные записи
/// читаются первыми 2 МиБ. Выборка — резервуар Vitter R: равномерная
/// по всему корпусу, RAM O(max_chunks), один проход.
/// Возвращает (текстовые записи, бинарные записи).
fn poler_archive_chunks(
    archive: &std::path::Path,
    max_chunks: usize,
    out: &mut Vec<(String, String)>,
) -> Result<(usize, usize), String> {
    use poler_engine::archive::reader::PolerReader;
    let reader = PolerReader::open(archive)?;
    const PER_FILE_CAP: u64 = 2 * 1024 * 1024;
    let prefix = archive
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "corpus.poler".to_string());
    let mut text_files = 0usize;
    let mut binary_files = 0usize;
    let mut seen: u64 = 0;
    // детерминированный xorshift64 — резервуар воспроизводим
    let mut rng: u64 = 0x504F_4C45_5253_4543; // "POLERSEC"
    let mut scratch: Vec<u8> = Vec::new();
    for f in reader.files() {
        if f.raw_len == 0 {
            continue;
        }
        let take = f.raw_len.min(PER_FILE_CAP) as usize;
        scratch.clear();
        if reader.read_range(f.raw_off, take, &mut scratch).is_err() {
            continue;
        }
        let bytes: &[u8] = &scratch[..];
        let probe = &bytes[..bytes.len().min(4096)];
        if probe.contains(&0) {
            binary_files += 1;
            continue;
        }
        text_files += 1;
        let name = format!("{prefix}::{}", f.name);
        let text = String::from_utf8_lossy(bytes);
        for piece in text.split("\n\n") {
            let p = piece.trim();
            if p.chars().count() < 40 {
                continue; // мусорные осколки пропускаем
            }
            let pieces: Vec<String> = if p.chars().count() <= 380 {
                vec![p.to_string()]
            } else {
                // жёсткая нарезка длинных абзацев (~150 токенов на чанк)
                let mut v = Vec::new();
                let mut start = 0usize;
                let cs: Vec<char> = p.chars().collect();
                while start < cs.len() {
                    let end = (start + 340).min(cs.len());
                    v.push(cs[start..end].iter().collect());
                    start = end;
                }
                v
            };
            for s in pieces {
                seen += 1;
                if (out.len() as u64) < max_chunks as u64 {
                    out.push((name.clone(), s));
                } else {
                    // резервуар Vitter R: замена с вероятностью max/seen
                    rng ^= rng << 13;
                    rng ^= rng >> 7;
                    rng ^= rng << 17;
                    let idx = (rng % seen) as usize;
                    if idx < max_chunks {
                        out[idx] = (name.clone(), s);
                    }
                }
            }
        }
    }
    Ok((text_files, binary_files))
}

// ---------------------------------------------------------------------------
// Суверенный Гиппокамп (v0.29): раннеры CLI
// ---------------------------------------------------------------------------

/// БД знаний: --knowledge-db | POLER_KNOWLEDGE_DB | ~/.local/share/…/knowledge.db.
fn knowledge_db_of(cli: &Cli) -> std::path::PathBuf {
    cli.knowledge_db
        .clone()
        .unwrap_or_else(poler_engine::sources::knowledge::default_db_path)
}

/// `--knowledge-ingest <PATH>`: библиотека POLER → нативный индекс.
fn run_knowledge_ingest(cli: &Cli, root: &std::path::Path) -> i32 {
    use poler_engine::sources::knowledge::{self, IngestOptions, KnowledgeEmbedder};

    let mode = match cli.knowledge_embedder {
        KnowledgeEmbedderArg::None => "none",
        KnowledgeEmbedderArg::Hash => "hash",
        KnowledgeEmbedderArg::Pqw => "pqw",
    };
    let mut embedder = match KnowledgeEmbedder::from_mode(mode, cli.model.as_deref()) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("poler-engine: {e}");
            return 2;
        }
    };
    let db = knowledge_db_of(cli);
    eprintln!(
        "poler-knowledge: инжест {:?} → {:?} (эмбеддер: {mode})",
        root, db
    );
    match knowledge::ingest(root, &db, &mut embedder, &IngestOptions::default()) {
        Ok(rep) => {
            print!("{}", rep.render_text());
            eprintln!(
                "поиск: poler-engine --knowledge-search \"…\" [--min-provenance mvr] \
                 [--knowledge-db {:?}]",
                db
            );
            0
        }
        Err(e) => {
            eprintln!("poler-engine: {e}");
            2
        }
    }
}

/// `--knowledge-search <QUERY>`: гибридный поиск с эпистемической градацией.
fn run_knowledge_search(cli: &Cli, query: &str) -> i32 {
    use poler_engine::retrieval::Provenance;
    use poler_engine::sources::knowledge::{self, QueryOptions};

    let db = knowledge_db_of(cli);
    let min_prov = match cli.min_provenance.as_deref() {
        None => None,
        Some(s) => match Provenance::parse(s) {
            Some(p) => Some(p),
            None => {
                eprintln!(
                    "poler-engine: неизвестный --min-provenance {s:?} \
                     (доступно: mvr | source | narrative)"
                );
                return 2;
            }
        },
    };
    // Векторное русло: восстанавливаем эмбеддер по мета индекса; для .pqw
    // нужен --model — без него ищем чистым BM25 (честная деградация).
    let mut embedder = match knowledge::query_embedder(&db, cli.model.as_deref()) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("poler-engine: {e}");
            None
        }
    };
    let opts = QueryOptions { top: cli.top.max(1), min_provenance: min_prov, ..Default::default() };
    match knowledge::query(&db, query, &opts, embedder.as_mut()) {
        Ok(out) => {
            print!("{}", knowledge::render_query_text(query, &out));
            if out.hits.is_empty() {
                1
            } else {
                0
            }
        }
        Err(e) => {
            eprintln!("poler-engine: {e}");
            2
        }
    }
}

/// `--knowledge-stats`: JSON-статистика индекса знаний.
fn run_knowledge_stats(cli: &Cli) -> i32 {
    let db = knowledge_db_of(cli);
    match poler_engine::sources::knowledge::stats(&db) {
        Ok(s) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&s).unwrap_or_else(|_| "{}".into())
            );
            0
        }
        Err(e) => {
            eprintln!("poler-engine: {e}");
            2
        }
    }
}

// ── Крипто-слой данных: POLER Vault (M4.5 CDL, фича pnd-ffi) ────────────────

// ---------------------------------------------------------------------------
// v0.28.1: Архивы без распаковки — CLI-хелперы
// ---------------------------------------------------------------------------

/// `--archive-list <ARCHIVE>`: листинг записей контейнера без распаковки
/// и без пароля (метаданные читаются raw из центрального каталога /
/// заголовков tar). `--archive-json` — машинно-читаемая форма для агента.
fn run_archive_list(cli: &Cli, archive: &std::path::Path) -> i32 {
    use poler_engine::archive;

    let info = match archive::open_info(archive) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("poler-archive: {e}");
            return 2;
        }
    };
    let files = info.entries.iter().filter(|e| !e.is_dir).count();
    let dirs = info.entries.len() - files;
    let encrypted = info.entries.iter().filter(|e| e.encrypted).count();
    if cli.archive_json {
        let listing = serde_json::json!({
            "archive": info.path.display().to_string(),
            "kind": info.kind.as_str(),
            "entries_total": info.entries.len(),
            "entries_files": files,
            "entries_dirs": dirs,
            "entries_encrypted": encrypted,
            "entries": info.entries.iter().map(|e| serde_json::json!({
                "name": e.name,
                "size": e.size,
                "compressed": e.compressed,
                "is_dir": e.is_dir,
                "encrypted": e.encrypted,
            })).collect::<Vec<_>>(),
        });
        match serde_json::to_string_pretty(&listing) {
            Ok(json) => println!("{json}"),
            Err(e) => {
                eprintln!("poler-archive: сериализация JSON: {e}");
                return 2;
            }
        }
    } else {
        println!("Архив:  {}", info.path.display());
        println!(
            "Тип:    {} · записей: {} (файлов: {}, каталогов: {}) · зашифрованных: {}",
            info.kind.as_str(),
            info.entries.len(),
            files,
            dirs,
            encrypted
        );
        if encrypted > 0 {
            println!(
                "Пароль: --archive-password <PASS> либо env POLER_ARCHIVE_KEY \
                 (выводится промптом на TTY)"
            );
        }
        println!("{:>12}  {:>12}  {:<4}  {}", "размер", "сжатие", "шифр", "имя");
        for e in &info.entries {
            println!(
                "{:>12}  {:>12}  {:<4}  {}{}",
                e.size,
                if e.compressed > 0 {
                    e.compressed.to_string()
                } else {
                    "-".to_string()
                },
                if e.encrypted { "да" } else { "-" },
                e.name,
                if e.is_dir { "/" } else { "" }
            );
        }
    }
    0
}

// ---------- v0.30.0: Коннектом FLYCSR1 — мозг мухи как матрица A ----------

/// Разделение разрядов: 54492922 → «54 492 922».
fn thou(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    let bytes = s.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(*b as char);
    }
    out
}

/// Парсинг направления для режимов --connectome-neighbors/--connectome-common.
fn ct_dir(s: &str) -> Result<poler_engine::graph::flyops::Direction, String> {
    poler_engine::graph::flyops::Direction::parse(s)
}

/// Строка ребра для человека: «79529 (root ...) w=17 gaba (-1)».
fn fmt_con_edge(
    e: &poler_engine::graph::connectome::Edge,
    nodes: &Option<poler_engine::graph::connectome::ConnectomeNodes>,
) -> String {
    let who = match nodes {
        Some(ns) => match ns.root_id(e.target as usize) {
            Some(r) => format!("{} (root {})", e.target, r),
            None => e.target.to_string(),
        },
        None => e.target.to_string(),
    };
    let sign = match e.sign() {
        1 => "+1",
        -1 => "-1",
        _ => "0",
    };
    format!("{who} w={} {} ({sign})", e.weight, e.nt_name())
}

/// JSON-объект ребра (цель/источник + вес/медиатор/знак).
fn con_edge_json(
    e: &poler_engine::graph::connectome::Edge,
    nodes: &Option<poler_engine::graph::connectome::ConnectomeNodes>,
) -> serde_json::Value {
    serde_json::json!({
        "other": e.target,
        "other_root_id": nodes.as_ref().and_then(|n| n.root_id(e.target as usize)),
        "weight": e.weight,
        "nt": e.nt_name(),
        "sign": e.sign(),
        "signed_weight": e.signed_weight(),
    })
}

fn run_connectome(cli: &Cli, csr_path: &std::path::Path) -> i32 {
    use poler_engine::graph::connectome as ct;

    let t0 = std::time::Instant::now();
    let con = match ct::Connectome::load(csr_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("poler-connectome: {e}");
            return 2;
        }
    };
    let load_ms = t0.elapsed().as_millis();

    let nodes = match &cli.connectome_nodes {
        Some(p) => match ct::ConnectomeNodes::load(p) {
            Ok(n) => {
                if n.len() != con.n_nodes() {
                    eprintln!(
                        "poler-connectome: {} содержит {} root_id, а коннектом ждёт {} узлов",
                        p.display(),
                        n.len(),
                        con.n_nodes()
                    );
                    return 2;
                }
                Some(n)
            }
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 2;
            }
        },
        None => None,
    };

    // Режимы взаимно исключают друг друга.
    let modes = [
        cli.connectome_node.is_some(),
        cli.connectome_edge.is_some(),
        cli.connectome_khop.is_some(),
        cli.connectome_impact.is_some(),
        cli.connectome_neighbors.is_some(),
        cli.connectome_path.is_some(),
        cli.connectome_common.is_some(),
        cli.connectome_centrality,
        cli.connectome_rotor_top.is_some(),
        cli.connectome_motifs.is_some(),
        cli.connectome_propagate.is_some(),
    ];
    if modes.iter().filter(|&&b| b).count() > 1 {
        eprintln!(
            "poler-connectome: укажите один режим (--connectome-node | --connectome-edge | \
             --connectome-khop | --connectome-impact | --connectome-neighbors | \
             --connectome-path | --connectome-common | --connectome-centrality | \
             --connectome-rotor-top | --connectome-motifs | --connectome-propagate); \
             без режима — сводка"
        );
        return 2;
    }

    // Резолв нейрона: индекс (< n_nodes) либо root_id (с --connectome-nodes).
    let resolve = |spec: &str| -> Result<usize, String> {
        if let Ok(idx) = spec.parse::<usize>() {
            if idx < con.n_nodes() {
                return Ok(idx);
            }
        }
        if let Some(ns) = &nodes {
            if let Ok(rid) = spec.parse::<u64>() {
                if let Some(i) = ns.idx_of(rid) {
                    return Ok(i);
                }
            }
        }
        Err(format!(
            "нейрон «{spec}» не найден (узлов: {}; формат: индекс 0..{} либо root_id с --connectome-nodes)",
            con.n_nodes(),
            con.n_nodes().saturating_sub(1)
        ))
    };

    let json = cli.connectome_json;

    // ---------- Режим: паспорт нейрона ----------
    if let Some(spec) = &cli.connectome_node {
        let idx = match resolve(spec) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 1;
            }
        };
        let outs: Vec<ct::Edge> = con.out_edges(idx).unwrap().collect();
        let out_mass: u64 = outs.iter().map(|e| e.weight as u64).sum();
        let csc = con.build_in_edges();
        let ins: Vec<ct::Edge> = csc.in_edges(&con, idx).unwrap().collect();
        let in_mass: u64 = ins.iter().map(|e| e.weight as u64).sum();
        let mut top_out = outs.clone();
        top_out.sort_by_key(|e| std::cmp::Reverse(e.weight));
        top_out.truncate(5);
        let mut top_in = ins.clone();
        top_in.sort_by_key(|e| std::cmp::Reverse(e.weight));
        top_in.truncate(5);
        if json {
            let j = serde_json::json!({
                "mode": "node",
                "artifact": csr_path.display().to_string(),
                "core": con.is_core(),
                "neuron": idx,
                "root_id": nodes.as_ref().and_then(|n| n.root_id(idx)),
                "out_degree": outs.len(),
                "out_mass": out_mass,
                "in_degree": ins.len(),
                "in_mass": in_mass,
                "top_out": top_out.iter().map(|e| con_edge_json(e, &nodes)).collect::<Vec<_>>(),
                "top_in": top_in.iter().map(|e| con_edge_json(e, &nodes)).collect::<Vec<_>>(),
            });
            match serde_json::to_string_pretty(&j) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("poler-connectome: сериализация JSON: {e}");
                    return 2;
                }
            }
        } else {
            println!("Нейрон {}:", idx);
            println!(
                "  исходящих: {} (масса {}) · входящих: {} (масса {})",
                outs.len(),
                thou(out_mass),
                ins.len(),
                thou(in_mass)
            );
            if !top_out.is_empty() {
                let list = top_out
                    .iter()
                    .map(|e| fmt_con_edge(e, &nodes))
                    .collect::<Vec<_>>()
                    .join(" · ");
                println!("  топ исходящие: {list}");
            }
            if !top_in.is_empty() {
                let list = top_in
                    .iter()
                    .map(|e| fmt_con_edge(e, &nodes))
                    .collect::<Vec<_>>()
                    .join(" · ");
                println!("  топ входящие:  {list}");
            }
        }
        return 0;
    }

    // ---------- Режим: ребро U:V + ротор J = A − Aᵀ ----------
    if let Some(spec) = &cli.connectome_edge {
        let (us, vs) = match spec.split_once(':') {
            Some(pair) => pair,
            None => {
                eprintln!(
                    "poler-connectome: --connectome-edge ждёт формат U:V, получено «{spec}»"
                );
                return 2;
            }
        };
        let u = match resolve(us) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 1;
            }
        };
        let v = match resolve(vs) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 1;
            }
        };
        let fwd = con.edge(u, v);
        let bwd = con.edge(v, u);
        if fwd.is_none() && bwd.is_none() {
            if json {
                let j = serde_json::json!({
                    "mode": "edge", "artifact": csr_path.display().to_string(),
                    "u": u, "v": v, "found": false,
                    "forward": serde_json::Value::Null,
                    "backward": serde_json::Value::Null,
                    "rotor": 0,
                });
                println!("{}", serde_json::to_string_pretty(&j).unwrap_or_default());
            } else {
                eprintln!("poler-connectome: связи {u} -> {v} нет (в обоих направлениях)");
            }
            return 1;
        }
        let rotor = con.rotor(u, v);
        if json {
            let j = serde_json::json!({
                "mode": "edge",
                "artifact": csr_path.display().to_string(),
                "core": con.is_core(),
                "u": u,
                "u_root_id": nodes.as_ref().and_then(|n| n.root_id(u)),
                "v": v,
                "v_root_id": nodes.as_ref().and_then(|n| n.root_id(v)),
                "found": true,
                "forward": fwd.map(|e| con_edge_json(&e, &nodes)),
                "backward": bwd.map(|e| con_edge_json(&e, &nodes)),
                "rotor_Juu": rotor, // J[u][v] = A(u,v) − A(v,u); J[v][u] = −rotor
            });
            match serde_json::to_string_pretty(&j) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("poler-connectome: сериализация JSON: {e}");
                    return 2;
                }
            }
        } else {
            match fwd {
                Some(e) => println!("Ребро {u} -> {v}: {}", fmt_con_edge(&e, &nodes)),
                None => println!("Ребро {u} -> {v}: связи нет"),
            }
            match bwd {
                Some(e) => println!("Обратное {v} -> {u}: {}", fmt_con_edge(&e, &nodes)),
                None => println!("Обратное {v} -> {u}: связи нет"),
            }
            match (fwd, bwd) {
                (Some(_), Some(_)) => println!(
                    "J = A − Aᵀ: J[{u}][{v}] = {rotor:+} (реципрокная пара, циркуляция {rotor})"
                ),
                _ => println!(
                    "J = A − Aᵀ: J[{u}][{v}] = {rotor:+}, J[{v}][{u}] = {:-} — однонаправленный поток",
                    -rotor
                ),
            }
        }
        return 0;
    }

    // ---------- Режим: K-hop BFS потока сигнала ----------
    if let Some(spec) = &cli.connectome_khop {
        let idx = match resolve(spec) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 1;
            }
        };
        let filter = match ct::SignFilter::parse(&cli.connectome_sign) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 2;
            }
        };
        let r = con.k_hop(idx, cli.k_hop, filter);
        let frontiers = r
            .frontier_sizes
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        if json {
            let j = serde_json::json!({
                "mode": "khop",
                "artifact": csr_path.display().to_string(),
                "core": con.is_core(),
                "start": idx,
                "start_root_id": nodes.as_ref().and_then(|n| n.root_id(idx)),
                "depth": cli.k_hop,
                "sign_filter": cli.connectome_sign,
                "frontier_sizes": r.frontier_sizes,
                "visited": r.visited,
            });
            match serde_json::to_string_pretty(&j) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("poler-connectome: сериализация JSON: {e}");
                    return 2;
                }
            }
        } else {
            let label = match filter {
                ct::SignFilter::All => "все связи".to_string(),
                ct::SignFilter::Excitatory => "только возбуждающие (+1)".to_string(),
                ct::SignFilter::Inhibitory => "только тормозные (-1)".to_string(),
            };
            println!(
                "K-hop от {idx} (глубина {}): фронты [{frontiers}], достигнуто {} нейронов (сигнал: {label})",
                cli.k_hop,
                r.visited
            );
        }
        return 0;
    }

    // ---------- Режим: impact — «кто управляет нейроном» (CSC) ----------
    if let Some(spec) = &cli.connectome_impact {
        let idx = match resolve(spec) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 1;
            }
        };
        let t1 = std::time::Instant::now();
        let csc = con.build_in_edges();
        let csc_ms = t1.elapsed().as_millis();
        let outs: Vec<ct::Edge> = con.out_edges(idx).unwrap().collect();
        let ins: Vec<ct::Edge> = csc.in_edges(&con, idx).unwrap().collect();
        if ins.is_empty() && outs.is_empty() {
            if json {
                let j = serde_json::json!({
                    "mode": "impact", "artifact": csr_path.display().to_string(),
                    "neuron": idx, "in_degree": 0, "out_degree": 0, "found": false,
                });
                println!("{}", serde_json::to_string_pretty(&j).unwrap_or_default());
            } else {
                eprintln!("poler-connectome: нейрон {idx} изолирован (ни входящих, ни исходящих)");
            }
            return 1;
        }
        let in_mass: u64 = ins.iter().map(|e| e.weight as u64).sum();
        let inh_mass: u64 = ins
            .iter()
            .filter(|e| e.sign() == -1)
            .map(|e| e.weight as u64)
            .sum();
        let exc_mass: u64 = ins
            .iter()
            .filter(|e| e.sign() == 1)
            .map(|e| e.weight as u64)
            .sum();
        let mut top_in = ins.clone();
        top_in.sort_by_key(|e| std::cmp::Reverse(e.weight));
        top_in.truncate(10);
        if json {
            let j = serde_json::json!({
                "mode": "impact",
                "artifact": csr_path.display().to_string(),
                "core": con.is_core(),
                "neuron": idx,
                "root_id": nodes.as_ref().and_then(|n| n.root_id(idx)),
                "in_degree": ins.len(),
                "in_mass": in_mass,
                "in_exc_mass": exc_mass,
                "in_inh_mass": inh_mass,
                "out_degree": outs.len(),
                "csc_build_ms": csc_ms,
                "top_sources": top_in.iter().map(|e| con_edge_json(e, &nodes)).collect::<Vec<_>>(),
            });
            match serde_json::to_string_pretty(&j) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("poler-connectome: сериализация JSON: {e}");
                    return 2;
                }
            }
        } else {
            println!("Impact нейрона {idx} (кто управляет):");
            println!(
                "  входящих: {} (масса {}) · возбуждающая масса {} / тормозная {} · CSC за {} мс",
                ins.len(),
                thou(in_mass),
                thou(exc_mass),
                thou(inh_mass),
                csc_ms
            );
            println!("  исходящих: {} — куда управляет он сам", outs.len());
            for e in &top_in {
                println!("    {}", fmt_con_edge(e, &nodes));
            }
        }
        return 0;
    }

    // ---------- Режим: соседи (полный список по весу) ----------
    if let Some(spec) = &cli.connectome_neighbors {
        let idx = match resolve(spec) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 1;
            }
        };
        let dir = match ct_dir(&cli.connectome_dir) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 2;
            }
        };
        let filter = match ct::SignFilter::parse(&cli.connectome_sign) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 2;
            }
        };
        let csc = con.build_in_edges();
        let list = match con.neighbors(idx, dir, filter, cli.connectome_limit, &csc) {
            Some(l) => l,
            None => {
                eprintln!("poler-connectome: нейрон {idx} вне диапазона");
                return 1;
            }
        };
        let dir_name = match dir {
            poler_engine::graph::flyops::Direction::Out => "исходящие (мишени)",
            poler_engine::graph::flyops::Direction::In => "входящие (источники)",
        };
        let mass: u64 = list.iter().map(|e| e.weight as u64).sum();
        if json {
            let j = serde_json::json!({
                "mode": "neighbors",
                "artifact": csr_path.display().to_string(),
                "neuron": idx,
                "root_id": nodes.as_ref().and_then(|n| n.root_id(idx)),
                "direction": cli.connectome_dir,
                "sign_filter": cli.connectome_sign,
                "limit": cli.connectome_limit,
                "count": list.len(),
                "mass": mass,
                "edges": list.iter().map(|e| con_edge_json(e, &nodes)).collect::<Vec<_>>(),
            });
            match serde_json::to_string_pretty(&j) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("poler-connectome: сериализация JSON: {e}");
                    return 2;
                }
            }
        } else {
            println!("Соседи нейрона {idx} — {dir_name} (фильтр {}):", cli.connectome_sign);
            println!("  партнёров: {} (масса {})", list.len(), thou(mass));
            for e in &list {
                println!("  {}", fmt_con_edge(e, &nodes));
            }
        }
        return 0;
    }

    // ---------- Режим: кратчайший путь FROM:TO ----------
    if let Some(spec) = &cli.connectome_path {
        let (fs, ts) = match spec.split_once(':') {
            Some(pair) => pair,
            None => {
                eprintln!(
                    "poler-connectome: --connectome-path ждёт формат FROM:TO, получено «{spec}»"
                );
                return 2;
            }
        };
        let from = match resolve(fs) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 1;
            }
        };
        let to = match resolve(ts) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 1;
            }
        };
        let filter = match ct::SignFilter::parse(&cli.connectome_sign) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 2;
            }
        };
        let r = con.shortest_path(from, to, filter);
        if json {
            let j = serde_json::json!({
                "mode": "path",
                "artifact": csr_path.display().to_string(),
                "from": from,
                "from_root_id": nodes.as_ref().and_then(|n| n.root_id(from)),
                "to": to,
                "to_root_id": nodes.as_ref().and_then(|n| n.root_id(to)),
                "sign_filter": cli.connectome_sign,
                "found": r.found,
                "length": r.length(),
                "total_weight": r.total_weight(),
                "hops": r.hops.iter().map(|h| serde_json::json!({
                    "from": h.from,
                    "to": h.to,
                    "weight": h.edge.weight,
                    "nt": h.edge.nt_name(),
                    "sign": h.edge.sign(),
                    "signed_weight": h.edge.signed_weight(),
                })).collect::<Vec<_>>(),
            });
            match serde_json::to_string_pretty(&j) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("poler-connectome: сериализация JSON: {e}");
                    return 2;
                }
            }
        } else if !r.found {
            eprintln!(
                "poler-connectome: путь {from} -> {to} не найден (фильтр {})",
                cli.connectome_sign
            );
            return 1;
        } else {
            println!(
                "Кратчайший путь {from} -> {to}: {} прыжков, масса {}",
                r.length(),
                thou(r.total_weight())
            );
            for (i, h) in r.hops.iter().enumerate() {
                println!(
                    "  {:>2}. {} -> {}: w={} {} ({:+})",
                    i + 1,
                    h.from,
                    h.to,
                    h.edge.weight,
                    h.edge.nt_name(),
                    h.edge.signed_weight()
                );
            }
        }
        return 0;
    }

    // ---------- Режим: общие партнёры набора ----------
    if let Some(spec) = &cli.connectome_common {
        let idxs: Vec<usize> = match spec
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| resolve(s))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(v) if !v.is_empty() => v,
            Ok(_) => {
                eprintln!("poler-connectome: --connectome-common: пустой набор нейронов");
                return 2;
            }
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 1;
            }
        };
        let dir = match ct_dir(&cli.connectome_dir) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 2;
            }
        };
        let filter = match ct::SignFilter::parse(&cli.connectome_sign) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 2;
            }
        };
        let csc = con.build_in_edges();
        let mut partners = match con.common_partners(&idxs, dir, filter, &csc) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 2;
            }
        };
        let count = partners.len();
        partners.truncate(cli.connectome_limit);
        let dir_name = match dir {
            poler_engine::graph::flyops::Direction::Out => "общие мишени (вниз по потоку)",
            poler_engine::graph::flyops::Direction::In => "общие источники (вверх по потоку)",
        };
        if json {
            let j = serde_json::json!({
                "mode": "common",
                "artifact": csr_path.display().to_string(),
                "neurons": idxs,
                "direction": cli.connectome_dir,
                "sign_filter": cli.connectome_sign,
                "count": count,
                "partners": partners.iter().map(|p| serde_json::json!({
                    "node": p.node,
                    "root_id": nodes.as_ref().and_then(|n| n.root_id(p.node)),
                    "total_weight": p.total_weight,
                    "links": p.members.iter().map(|(q, e)| serde_json::json!({
                        "query": q,
                        "edge": con_edge_json(e, &nodes),
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            });
            match serde_json::to_string_pretty(&j) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("poler-connectome: сериализация JSON: {e}");
                    return 2;
                }
            }
        } else {
            println!("Набор {:?} — {}:", idxs, dir_name);
            println!("  общих партнёров: {}", count);
            for p in partners.iter().take(cli.connectome_limit) {
                println!(
                    "  {} (масса {})",
                    match nodes.as_ref().and_then(|n| n.root_id(p.node)) {
                        Some(r) => format!("{} (root {})", p.node, r),
                        None => p.node.to_string(),
                    },
                    thou(p.total_weight)
                );
            }
        }
        return 0;
    }

    // ---------- Режим: центральность (хабы + PageRank) ----------
    if cli.connectome_centrality {
        let filter = match ct::SignFilter::parse(&cli.connectome_sign) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 2;
            }
        };
        let csc = con.build_in_edges();
        let top_out = con.degree_ranking(
            poler_engine::graph::flyops::Direction::Out,
            cli.connectome_top,
            &csc,
        );
        let top_in = con.degree_ranking(
            poler_engine::graph::flyops::Direction::In,
            cli.connectome_top,
            &csc,
        );
        let t1 = std::time::Instant::now();
        let pr = con.pagerank(filter, 0.85, 30, 1e-9, cli.connectome_top);
        let pr_ms = t1.elapsed().as_millis();
        if json {
            let j = serde_json::json!({
                "mode": "centrality",
                "artifact": csr_path.display().to_string(),
                "sign_filter": cli.connectome_sign,
                "top": cli.connectome_top,
                "top_out_degree": top_out.iter().map(|&(u, d)| serde_json::json!({
                    "neuron": u,
                    "root_id": nodes.as_ref().and_then(|n| n.root_id(u)),
                    "out_degree": d,
                })).collect::<Vec<_>>(),
                "top_in_degree": top_in.iter().map(|&(v, d)| serde_json::json!({
                    "neuron": v,
                    "root_id": nodes.as_ref().and_then(|n| n.root_id(v)),
                    "in_degree": d,
                })).collect::<Vec<_>>(),
                "pagerank": {
                    "iterations": pr.iterations,
                    "converged": pr.converged,
                    "took_ms": pr_ms,
                    "top": pr.top.iter().map(|&(u, r)| serde_json::json!({
                        "neuron": u,
                        "root_id": nodes.as_ref().and_then(|n| n.root_id(u)),
                        "rank": r,
                    })).collect::<Vec<_>>(),
                },
            });
            match serde_json::to_string_pretty(&j) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("poler-connectome: сериализация JSON: {e}");
                    return 2;
                }
            }
        } else {
            println!("Центральность (топ {}):", cli.connectome_top);
            println!("  исходящие хабы (куда раздают):");
            for &(u, d) in &top_out {
                println!("    нейрон {u}: {d} рёбер");
            }
            println!("  входящие хабы (кого бомбардируют):");
            for &(v, d) in &top_in {
                println!("    нейрон {v}: {d} рёбер");
            }
            println!(
                "  PageRank ({} итераций, {}):",
                pr.iterations,
                if pr.converged { "сошёлся" } else { "потолок итераций" }
            );
            for &(u, r) in &pr.top {
                println!("    нейрон {u}: ранг {r:.9}");
            }
            println!("  PageRank занял {} мс", pr_ms);
        }
        return 0;
    }

    // ---------- Режим: глобальный топ циркуляции J ----------
    if let Some(k) = cli.connectome_rotor_top {
        let t1 = std::time::Instant::now();
        let pairs = con.rotor_top(k, cli.connectome_min_abs);
        let took_ms = t1.elapsed().as_millis();
        let count = pairs.len();
        if json {
            let j = serde_json::json!({
                "mode": "rotor_top",
                "artifact": csr_path.display().to_string(),
                "top": k,
                "min_abs": cli.connectome_min_abs,
                "count": count,
                "took_ms": took_ms,
                "pairs": pairs.iter().map(|p| serde_json::json!({
                    "u": p.u,
                    "u_root_id": nodes.as_ref().and_then(|n| n.root_id(p.u)),
                    "v": p.v,
                    "v_root_id": nodes.as_ref().and_then(|n| n.root_id(p.v)),
                    "J_uv": p.j,
                    "J_vu": -p.j,
                    "forward_signed": p.forward,
                    "backward_signed": p.backward,
                })).collect::<Vec<_>>(),
            });
            match serde_json::to_string_pretty(&j) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("poler-connectome: сериализация JSON: {e}");
                    return 2;
                }
            }
        } else {
            println!(
                "Топ-{} циркуляции J = A − Aᵀ (порог |J| ≥ {}, найдено {}, {} мс):",
                k, cli.connectome_min_abs, count, took_ms
            );
            for p in &pairs {
                println!(
                    "  J[{}][{}] = {:+5} — u доминирует (fwd {:?}, bwd {:?})",
                    p.u, p.v, p.j, p.forward, p.backward
                );
            }
        }
        return 0;
    }

    // ---------- Режим: мотивы вокруг нейрона ----------
    if let Some(spec) = &cli.connectome_motifs {
        let idx = match resolve(spec) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 1;
            }
        };
        let csc = con.build_in_edges();
        let m = match con.motif_census(idx, &csc, 5) {
            Some(m) => m,
            None => {
                eprintln!("poler-connectome: нейрон {idx} вне диапазона");
                return 1;
            }
        };
        if json {
            let j = serde_json::json!({
                "mode": "motifs",
                "artifact": csr_path.display().to_string(),
                "neuron": idx,
                "root_id": nodes.as_ref().and_then(|n| n.root_id(idx)),
                "out_degree": m.out_degree,
                "in_degree": m.in_degree,
                "reciprocal_count": m.reciprocal.len(),
                "reciprocal": m.reciprocal.iter().map(|(fwd, bwd)| serde_json::json!({
                    "partner": fwd.target,
                    "partner_root_id": nodes.as_ref().and_then(|n| n.root_id(fwd.target as usize)),
                    "u_to_partner": {"weight": fwd.weight, "nt": fwd.nt_name(), "sign": fwd.sign()},
                    "partner_to_u": {"weight": bwd.weight, "nt": bwd.nt_name(), "sign": bwd.sign()},
                })).collect::<Vec<_>>(),
                "feedforward": m.feedforward,
                "feedback3": m.feedback3,
                "ff_examples": m.ff_examples,
            });
            match serde_json::to_string_pretty(&j) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("poler-connectome: сериализация JSON: {e}");
                    return 2;
                }
            }
        } else {
            println!("Мотивы вокруг нейрона {idx}:",);
            println!(
                "  степень: {} исходящих / {} входящих",
                m.out_degree, m.in_degree
            );
            println!("  реципрокных пар (u⇄v): {}", m.reciprocal.len());
            for (fwd, bwd) in m.reciprocal.iter().take(5) {
                println!(
                    "    ⇄ {}: туда w={} {} / обратно w={} {}",
                    fwd.target,
                    fwd.weight,
                    fwd.nt_name(),
                    bwd.weight,
                    bwd.nt_name()
                );
            }
            println!("  feedforward-треугольников (u→v→w + u→w): {}", thou(m.feedforward as u64));
            println!("  feedback-циклов (u→v→w→u): {}", thou(m.feedback3 as u64));
            if !m.ff_examples.is_empty() {
                let ex = m
                    .ff_examples
                    .iter()
                    .map(|(v, w)| format!("{v}→{w}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("  примеры: {ex}");
            }
        }
        return 0;
    }

    // ---------- Режим: симуляция распространения сигнала ----------
    if let Some(spec) = &cli.connectome_propagate {
        let seeds: Vec<usize> = match spec
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| resolve(s))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(v) if !v.is_empty() => v,
            Ok(_) => {
                eprintln!("poler-connectome: --connectome-propagate: пустой набор семян");
                return 2;
            }
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 1;
            }
        };
        let filter = match ct::SignFilter::parse(&cli.connectome_sign) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 2;
            }
        };
        let r = match con.propagate(
            &seeds,
            cli.connectome_steps,
            cli.connectome_gamma,
            cli.connectome_leak,
            filter,
            cli.connectome_top,
            0.01,
        ) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("poler-connectome: {e}");
                return 2;
            }
        };
        if json {
            let round6 = |x: f64| (x * 1e6).round() / 1e6;
            let j = serde_json::json!({
                "mode": "propagate",
                "artifact": csr_path.display().to_string(),
                "seeds": r.seeds,
                "params": {
                    "steps": cli.connectome_steps,
                    "gamma": cli.connectome_gamma,
                    "leak": cli.connectome_leak,
                    "sign_filter": cli.connectome_sign,
                    "theta": 0.01,
                },
                "timeline": r.steps.iter().map(|s| serde_json::json!({
                    "step": s.step,
                    "active": s.active,
                    "positive_mass": round6(s.positive_mass),
                    "negative_mass": round6(s.negative_mass),
                })).collect::<Vec<_>>(),
                "top": r.top.iter().map(|&(v, x)| serde_json::json!({
                    "neuron": v,
                    "root_id": nodes.as_ref().and_then(|n| n.root_id(v)),
                    "potential": round6(x),
                })).collect::<Vec<_>>(),
            });
            match serde_json::to_string_pretty(&j) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("poler-connectome: сериализация JSON: {e}");
                    return 2;
                }
            }
        } else {
            println!(
                "Симуляция от {:?}: {} шагов, γ = {}, leak = {} (фильтр {}):",
                r.seeds, cli.connectome_steps, cli.connectome_gamma, cli.connectome_leak,
                cli.connectome_sign
            );
            for s in &r.steps {
                println!(
                    "  шаг {}: активных {}, масса +{:.3} / {:.3}",
                    s.step, s.active, s.positive_mass, s.negative_mass
                );
            }
            println!("  топ возбуждённых:");
            for &(v, x) in r.top.iter().take(cli.connectome_top) {
                println!("    нейрон {v}: потенциал {x:+.6}");
            }
        }
        return 0;
    }

    // ---------- Сводка (режим по умолчанию) ----------
    let m = con.mass_by_nt();
    let exc = m[1].0 + m[2].0;
    let inh = m[0].0;
    let modm = m[3].0 + m[4].0 + m[5].0;
    let tot = con.total_mass();
    let pct = |x: u64| format!("{:.1}%", x as f64 / tot as f64 * 100.0);
    // топ нейронов по исходящей степени
    let mut top_deg: Vec<(u32, usize)> = (0..con.n_nodes())
        .map(|u| (con.out_degree(u).unwrap_or(0), u))
        .collect();
    top_deg.sort_by(|a, b| b.cmp(a));
    top_deg.truncate(5);
    if json {
        let j = serde_json::json!({
            "mode": "summary",
            "artifact": csr_path.display().to_string(),
            "format": "FLYCSR1",
            "core": con.is_core(),
            "core_min_synapses": if con.is_core() { serde_json::json!(ct::CORE_MIN_SYNAPSES) } else { serde_json::json!(null) },
            "n_nodes": con.n_nodes(),
            "n_edges": con.n_edges(),
            "total_mass": tot,
            "load_ms": load_ms,
            "nt": (0..6).map(|c| serde_json::json!({
                "name": ct::NT_NAMES[c],
                "edges": m[c].1,
                "mass": m[c].0,
                "sign": ct::nt_sign(c as u8),
            })).collect::<Vec<_>>(),
            "mass_balance": {
                "excitatory": exc, "inhibitory": inh, "modulatory": modm,
                "excitatory_pct": pct(exc), "inhibitory_pct": pct(inh), "modulatory_pct": pct(modm),
            },
            "top_out_degree": top_deg.iter().map(|&(d, u)| serde_json::json!({
                "neuron": u,
                "root_id": nodes.as_ref().and_then(|n| n.root_id(u)),
                "out_degree": d,
            })).collect::<Vec<_>>(),
        });
        match serde_json::to_string_pretty(&j) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("poler-connectome: сериализация JSON: {e}");
                return 2;
            }
        }
    } else {
        println!("Коннектом: {}", csr_path.display());
        println!(
            "Формат: FLYCSR1 · {} · загрузка {} мс (zstd -> CSR в RAM)",
            if con.is_core() {
                format!("ядро (рёбра >= {} синапсов)", ct::CORE_MIN_SYNAPSES)
            } else {
                "полный граф".to_string()
            },
            load_ms
        );
        println!(
            "Нейронов: {} · рёбер: {} · синаптическая масса: {}",
            thou(con.n_nodes() as u64),
            thou(con.n_edges() as u64),
            thou(tot)
        );
        println!("{:<8} {:>12} {:>12}   знак", "медиатор", "рёбра", "масса");
        for c in 0..6 {
            let s = ct::nt_sign(c as u8);
            println!(
                "{:<8} {:>12} {:>12}   {}",
                ct::NT_NAMES[c],
                thou(m[c].1),
                thou(m[c].0),
                match s {
                    1 => "+1".to_string(),
                    -1 => "-1".to_string(),
                    _ => "0".to_string(),
                }
            );
        }
        println!(
            "Баланс массы: {} возб / {} торм / {} мод",
            pct(exc),
            pct(inh),
            pct(modm)
        );
        let tops = top_deg
            .iter()
            .map(|&(d, u)| match &nodes {
                Some(ns) => match ns.root_id(u) {
                    Some(r) => format!("{u} (root {r}) — {d}"),
                    None => format!("{u} — {d}"),
                },
                None => format!("{u} — {d}"),
            })
            .collect::<Vec<_>>()
            .join(" · ");
        println!("Топ по исходящей степени: {tops}");
        println!(
            "Запросы: --connectome-node | --connectome-edge U:V | --connectome-khop | \
             --connectome-impact (аргумент: индекс или root_id с --connectome-nodes)"
        );
    }
    0
}

/// Резолв пароля архивов ДО параллельного прогона: флаг
/// `--archive-password` (человек/агент вводит прямо в CLI) → env
/// `POLER_ARCHIVE_KEY` → TTY-промпт (только если среди найденных архивов
/// есть зашифрованные; промпт нельзя звать из rayon-воркеров).
fn resolve_archive_password(
    cli: &Cli,
    roots: &[std::path::PathBuf],
    include_hidden: bool,
) -> Result<Option<String>, String> {
    // 1. Явный флаг — высший приоритет.
    if let Some(p) = &cli.archive_password {
        return Ok(Some(p.clone()));
    }
    // 2. Окружение (не попадает в историю shell).
    if let Ok(p) = std::env::var("POLER_ARCHIVE_KEY") {
        if !p.trim().is_empty() {
            return Ok(Some(p));
        }
    }
    // 3. Пароль нужен только если хоть один архив зашифрован.
    let archives = poler_engine::retrieval::collect_archives(roots, include_hidden, true);
    let mut encrypted: Vec<std::path::PathBuf> = Vec::new();
    for a in &archives {
        match poler_engine::archive::open_info(a) {
            Ok(info) if info.encrypted() => encrypted.push(a.clone()),
            Ok(_) => {}
            // Листинг не удался — не блокируем поиск: сама запись
            // выдам ошибку в stats прогона.
            Err(_) => {}
        }
    }
    if encrypted.is_empty() {
        return Ok(None);
    }
    // 4. Интерактивный ввод (как у Vault: stdin, не история shell).
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() {
        eprint!(
            "poler-archive: пароль для {} (stdin, не попадёт в историю shell): ",
            encrypted[0].display()
        );
        use std::io::Write as _;
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .map_err(|e| format!("stdin: {e}"))?;
        let pw = line.trim_end_matches(['\r', '\n']).to_string();
        if pw.is_empty() {
            return Err(format!(
                "пароль пуст; архивы зашифрованы: {} (передайте --archive-password \
                 или POLER_ARCHIVE_KEY)",
                encrypted
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        return Ok(Some(pw));
    }
    Err(format!(
        "архив(ы) зашифрованы: {} — передайте --archive-password <PASS> \
         или env POLER_ARCHIVE_KEY (в интерактивном терминале движок \
         спросит пароль сам)",
        encrypted
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Парольная фраза: env (по умолчанию POLER_VAULT_KEY) → stdin.
/// Ключ никогда не пишется в историю shell: env или пайп.
#[cfg(feature = "pnd-ffi")]
fn vault_passphrase(key_env: &str) -> Result<String, String> {
    if let Ok(k) = std::env::var(key_env) {
        if !k.is_empty() {
            return Ok(k);
        }
    }
    eprint!("poler-vault: парольная фраза (stdin, не попадёт в историю shell): ");
    use std::io::BufRead;
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| format!("stdin: {e}"))?;
    let phrase = line.trim_end_matches(['\n', '\r']).to_string();
    if phrase.is_empty() {
        return Err("пустая парольная фраза — отказ (защита от случайной печати без ключа)".into());
    }
    Ok(phrase)
}

/// Выходной путь по умолчанию для печати: PATH + ".pvt".
#[cfg(feature = "pnd-ffi")]
fn vault_default_seal_output(input: &std::path::Path) -> std::path::PathBuf {
    let mut s = input.as_os_str().to_os_string();
    s.push(".pvt");
    std::path::PathBuf::from(s)
}

// ── M6: опции резидентного MCP + стриминг логов ────────────────────────────

/// Собрать опции резидентного сервера из CLI (--mcp-ram-budget, --vault-log).
/// Ошибка создания шифропотока — фатальна ДО старта сервера (не молчим).
fn mcp_server_options(cli: &Cli) -> poler_engine::mcp::McpServerOptions {
    // mut нужен только при pnd-ffi (стриминг --vault-log ниже);
    // без фичи конфигурация неизменяема после конструирования.
    #[cfg_attr(not(feature = "pnd-ffi"), allow(unused_mut))]
    let mut opts = poler_engine::mcp::McpServerOptions {
        ram_budget: cli.mcp_ram_budget.saturating_mul(1024 * 1024),
        ..Default::default()
    };
    #[cfg(feature = "pnd-ffi")]
    if let Some(vl) = &cli.vault_log {
        let phrase = match vault_log_passphrase(cli) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("poler-vault-log: {e}");
                std::process::exit(2);
            }
        };
        let appender = match poler_engine::crypto::vault::VaultAppender::create(
            vl,
            &phrase,
            &poler_engine::crypto::vault::SealOptions::default(),
        ) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("poler-vault-log: {}: {e}", vl.display());
                std::process::exit(2);
            }
        };
        eprintln!(
            "poler-vault-log: журнал операций шифропотоком → {} (стрим, коммит на каждое событие)",
            vl.display()
        );
        opts.vault_log = Some(appender);
    }
    opts
}

/// Фраза шифропотока журнала: --vault-log-pass | env POLER_VAULT_PASS | stdin.
#[cfg(feature = "pnd-ffi")]
fn vault_log_passphrase(cli: &Cli) -> Result<String, String> {
    if let Some(p) = &cli.vault_log_pass {
        if !p.is_empty() {
            return Ok(p.clone());
        }
    }
    if let Ok(k) = std::env::var("POLER_VAULT_PASS") {
        if !k.is_empty() {
            return Ok(k);
        }
    }
    eprint!("poler-vault-log: парольная фраза журнала (stdin): ");
    use std::io::BufRead;
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| format!("stdin: {e}"))?;
    let phrase = line.trim_end_matches(['\n', '\r']).to_string();
    if phrase.is_empty() {
        return Err("пустая парольная фраза — отказ".into());
    }
    Ok(phrase)
}

/// M6: бенчмарк резидентности — холодный против тёплого.
///
/// Строит (или берёт по --path/--knowledge-db) корпус, поднимает
/// in-process McpServer и гонит N итераций poler_grep +
/// query_poler_knowledge, замеряя латентность каждой dispatch.
/// Отчёт: cold (итерация 1) и warm p50/p95/p99; бюджет <5 мс —
/// критерий приёмки M6 (exit 0/1).
fn run_mcp_bench(cli: &Cli, iterations: usize) -> i32 {
    use poler_engine::mcp::McpServer;
    use serde_json::json;

    let tmp = std::env::temp_dir().join(format!("poler-mcp-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // Корпус: --path пользователя или временный (60 файлов × ~8 КиБ).
    let corpus: std::path::PathBuf = cli
        .path
        .clone()
        .unwrap_or_else(|| {
            eprintln!("poler-bench: --path не задан — строю временный корпус 60×8 КиБ");
            let dir = tmp.join("corpus");
            std::fs::create_dir_all(&dir).unwrap();
            for i in 0..60 {
                let body = format!(
                    "раздел {i}: каноническое уравнение dp/dt и резонансный аттрактор H Psi\n\
                     строка шума {i} для объёма и правдоподобия корпуса\n"
                );
                std::fs::write(dir.join(format!("doc{i:03}.md")), body.repeat(64)).unwrap();
            }
            dir
        })
        .into();

    // Гиппокамп: --knowledge-db пользователя или временный инжест.
    let kdb: std::path::PathBuf = cli
        .knowledge_db
        .clone()
        .unwrap_or_else(|| {
            eprintln!("poler-bench: --knowledge-db не задан — строю временную библиотеку");
            let dir = tmp.join("lib");
            let spec = dir.join("01_SPECS");
            std::fs::create_dir_all(&spec).unwrap();
            std::fs::write(
                spec.join("PTS-BENCH.md"),
                "# PTS-BENCH\n\n## 4. ПОЛНЫЙ ТЕХНИЧЕСКИЙ ТЕКСТ\nрезонансный аттрактор канона: dp/dt = -eta Pi Lambda D p.\n",
            )
            .unwrap();
            let db = tmp.join("bench-knowledge.db");
            let mut emb = poler_engine::sources::knowledge::KnowledgeEmbedder::None;
            poler_engine::sources::knowledge::ingest(
                &dir,
                &db,
                &mut emb,
                &poler_engine::sources::knowledge::IngestOptions::default(),
            )
            .unwrap();
            db
        })
        .into();

    // path-guard: разрешаем корпус (временный или пользовательский).
    std::env::set_var("POLER_MCP_EXTRA_ROOTS", &corpus);

    let server = McpServer::new(9222, 10, tmp.join("no-web.db"))
        .with_knowledge_db(kdb)
        .with_ram_budget(cli.mcp_ram_budget.saturating_mul(1024 * 1024));

    let dispatch_timed = |msg: serde_json::Value| -> (u128, serde_json::Value) {
        let t0 = std::time::Instant::now();
        let r = server.dispatch(&msg).unwrap_or(serde_json::Value::Null);
        (t0.elapsed().as_micros(), r)
    };

    let grep_msg = |q: &str| {
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": "poler_grep",
                       "arguments": {"pattern": q, "path": corpus.to_str().unwrap(), "output": "count"}}
        })
    };
    let know_msg = |q: &str| {
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {"name": "query_poler_knowledge", "arguments": {"query": q}}
        })
    };

    let print_report = |title: &str, samples: &[u128]| -> bool {
        let mut s = samples.to_vec();
        s.sort_unstable();
        let pick = |p: f64| -> u128 {
            if s.is_empty() { 0 } else { s[((s.len() as f64 - 1.0) * p).round() as usize] }
        };
        let p50 = pick(0.50);
        let p95 = pick(0.95);
        let p99 = pick(0.99);
        let ok = p99 < 5_000;
        println!(
            "{title}: n={} · cold={} мкс · warm p50={} p95={} p99={} мкс · бюджет <5000 мкс: {}",
            samples.len(),
            samples.first().copied().unwrap_or(0),
            p50, p95, p99,
            if ok { "ДА" } else { "НЕТ" }
        );
        ok
    };

    println!("poler-mcp-bench (M6 резидентность): итераций = {iterations}, бюджет RAM = {} МиБ", cli.mcp_ram_budget);

    // Серия grep: итерация 0 = холодная (чтение диска), остальные тёплые.
    let mut grep_samples = Vec::with_capacity(iterations);
    for i in 0..iterations {
        let (us, r) = dispatch_timed(grep_msg(if i % 2 == 0 { "резонансный" } else { "dp/dt" }));
        let ok = serde_json::to_string(&r).unwrap().contains("poler-grep");
        if !ok {
            eprintln!("poler-bench: grep-вызов не прошёл: {r}");
            let _ = std::fs::remove_dir_all(&tmp);
            return 2;
        }
        grep_samples.push(us);
    }
    let grep_ok = print_report("poler_grep        ", &grep_samples);

    // Серия знаний: итерация 0 = холодная (открытие SQLite/векторов).
    let mut know_samples = Vec::with_capacity(iterations);
    for i in 0..iterations {
        let (us, r) = dispatch_timed(know_msg(if i % 2 == 0 { "резонансный аттрактор" } else { "dp/dt канон" }));
        let txt = serde_json::to_string(&r).unwrap();
        if !txt.contains("poler knowledge") && !txt.contains("не проиндексирована") {
            eprintln!("poler-bench: knowledge-вызов не прошёл: {r}");
            let _ = std::fs::remove_dir_all(&tmp);
            return 2;
        }
        know_samples.push(us);
    }
    let know_ok = print_report("knowledge (warm)  ", &know_samples);

    // RAM-кэш и тёплые хэндлы.
    let stats = serde_json::to_string_pretty(&server.resident_stats()).unwrap();
    println!("резидентное состояние: {stats}");

    let _ = std::fs::remove_dir_all(&tmp);
    if grep_ok && know_ok {
        println!("M6: ПРИЁМКА ПРОЙДЕНА — тёплые p99 в бюджете <5 мс");
        0
    } else {
        eprintln!("M6: ПРИЁМКА ПРОВАЛЕНА — тёплые p99 выше 5 мс");
        1
    }
}

/// Выходной путь по умолчанию для вскрытия: снять .pvt, иначе суффикс .out.
#[cfg(feature = "pnd-ffi")]
fn vault_default_open_output(vault: &std::path::Path) -> std::path::PathBuf {
    match vault.extension().and_then(|e| e.to_str()) {
        Some("pvt") => vault.with_extension(""),
        _ => {
            let mut s = vault.as_os_str().to_os_string();
            s.push(".out");
            std::path::PathBuf::from(s)
        }
    }
}

/// E1/v0.31.0 + E2/v0.32.0: диспетчер --exec. Вывод ребёнка — в наши потоки
/// как есть (байты, без перекодировки), код выхода — ребёнка; таймаут — 124
/// (конвенция GNU timeout), не найдено — 127, отказ права — 126, плохой
/// cwd — 125. E2: --exec-cwd/--exec-env/--exec-pty/--exec-capture.
#[cfg(feature = "pnd-ffi")]
fn run_exec(cli: &Cli) -> i32 {
    use poler_engine::exec::{self, CaptureMode, ExecSpec};
    use std::io::Write;

    let mut parts = cli.exec_raw.iter();
    let program = parts.next().unwrap().clone();
    let args: Vec<String> = parts.cloned().collect();

    // --exec-env KEY=VALUE (повторяемый): явное окружение вместо наследования.
    let env = if cli.exec_env.is_empty() {
        None
    } else {
        let mut pairs = Vec::with_capacity(cli.exec_env.len());
        for kv in &cli.exec_env {
            match kv.split_once('=') {
                Some((k, v)) => pairs.push((k.to_string(), v.to_string())),
                None => {
                    eprintln!("poler-exec: --exec-env ожидает KEY=VALUE, получено {kv:?}");
                    return 2;
                }
            }
        }
        Some(pairs)
    };

    let capture = match CaptureMode::parse(&cli.exec_capture) {
        Some(c) => c,
        None => {
            eprintln!(
                "poler-exec: неизвестный --exec-capture {:?} (доступно: tail | head_tail)",
                cli.exec_capture
            );
            return 2;
        }
    };

    let spec = ExecSpec {
        program,
        args,
        env,
        timeout_ms: cli.exec_timeout_ms,
        grace_ms: cli.exec_grace_ms,
        max_out_bytes: cli.exec_max_out,
        stdin_data: cli.exec_stdin.as_ref().map(|s| s.as_bytes().to_vec()),
        cwd: cli.exec_cwd.clone(),
        pty: cli.exec_pty,
        capture,
        cancel: None,
    };

    match exec::run(&spec) {
        Ok(out) => {
            let mut so = std::io::stdout();
            let _ = so.write_all(&out.stdout);
            let _ = so.flush();
            let mut se = std::io::stderr();
            if !out.pty {
                // PTY: stderr уже слит в stdout — не дублируем
                let _ = se.write_all(&out.stderr);
            }
            if out.truncated {
                let _ = writeln!(
                    se,
                    "poler-exec: вывод превышал {} байт — удержан {}",
                    cli.exec_max_out,
                    match capture {
                        CaptureMode::Tail => "хвост".to_string(),
                        CaptureMode::HeadTail => "голова+маркер+хвост".to_string(),
                    }
                );
            }
            let _ = se.flush();
            if out.cancelled {
                eprintln!(
                    "poler-exec: запуск отменён — pid {} убит (TERM→KILL), {} мкс",
                    out.pid, out.duration_us
                );
                return 130; // 128+SIGINT-конвенция отмены
            }
            if out.timed_out {
                eprintln!(
                    "poler-exec: таймаут {} мс — pid {} убит (TERM→KILL), {} мкс",
                    cli.exec_timeout_ms, out.pid, out.duration_us
                );
                return 124;
            }
            if let Some(sig) = out.signal {
                eprintln!("poler-exec: процесс убит сигналом {sig}");
                return 128 + sig;
            }
            out.exit_code.unwrap_or(1)
        }
        Err(e) => {
            eprintln!("poler-exec: {e}");
            e.exit_code()
        }
    }
}

#[cfg(feature = "pnd-ffi")]
fn run_memory_seal(cli: &Cli, input: &std::path::Path) -> i32 {
    use poler_engine::crypto::vault::{self, SealOptions};

    let output = cli.memory_out.clone().unwrap_or_else(|| vault_default_seal_output(input));
    if output.exists() {
        eprintln!(
            "poler-vault: выход уже существует: {} (передайте --memory-out или удалите)",
            output.display()
        );
        return 2;
    }
    let passphrase = match vault_passphrase(&cli.memory_key_env) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("poler-vault: {e}");
            return 2;
        }
    };
    let opts = SealOptions {
        iterations: cli.memory_kdf_iters,
        content_id: cli.memory_content_id,
        salt: None,
    };
    match vault::seal(input, &output, &passphrase, &opts) {
        Ok(rep) => {
            println!("запечатано: {}", rep.output.display());
            println!(
                "  источник: {} ({} байт, {} стр. по 4096)",
                rep.input.display(),
                rep.original_len,
                rep.pages
            );
            println!("  контейнер: {} байт", rep.vault_len);
            println!("  время: {} мс ({:.1} МБ/с), KDF {} итераций", rep.duration_ms, rep.mb_per_s, opts.iterations);
            if let Some(cid) = rep.content_id {
                println!("  content-id: {}", poler_engine::crypto::hasher::PndHasher::hex(&cid));
            }
            println!("  проверка без ключа: poler-engine --memory-verify {}", rep.output.display());
            0
        }
        Err(e) => {
            eprintln!("poler-vault: {e}");
            2
        }
    }
}

#[cfg(feature = "pnd-ffi")]
fn run_memory_open(cli: &Cli, vault_path: &std::path::Path) -> i32 {
    use poler_engine::crypto::vault;

    let output = cli.memory_out.clone().unwrap_or_else(|| vault_default_open_output(vault_path));
    if output.exists() {
        eprintln!(
            "poler-vault: выход уже существует: {} (передайте --memory-out или удалите)",
            output.display()
        );
        return 2;
    }
    let passphrase = match vault_passphrase(&cli.memory_key_env) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("poler-vault: {e}");
            return 2;
        }
    };
    match vault::open(vault_path, &output, &passphrase) {
        Ok(rep) => {
            println!("вскрыто: {}", rep.output.display());
            println!(
                "  контейнер: {} ({} байт, {} стр.)",
                rep.input.display(),
                rep.original_len,
                rep.pages
            );
            println!(
                "  транспорт: OK (SHA-256), аутентичность: OK (MAC), {} мс ({:.1} МБ/с)",
                rep.duration_ms, rep.mb_per_s
            );
            0
        }
        Err(e) => {
            eprintln!("poler-vault: {e}");
            eprintln!("  подсказка: неверная фраза ИЛИ подмена — сравните с --memory-verify (без ключа)");
            2
        }
    }
}

#[cfg(feature = "pnd-ffi")]
fn run_memory_verify(_cli: &Cli, vault_path: &std::path::Path) -> i32 {
    use poler_engine::crypto::vault;

    match vault::verify(vault_path) {
        Ok(rep) => {
            println!("контейнер цел: {}", rep.path.display());
            println!("  заголовок: OK (FNV-1a64), транспорт: OK (SHA-256 шифротекста)");
            println!("  страниц: {}, размер: {} байт", rep.pages, rep.vault_len);
            0
        }
        Err(e) => {
            eprintln!("poler-vault: {e}");
            2
        }
    }
}

#[cfg(feature = "pnd-ffi")]
fn run_memory_info(_cli: &Cli, vault_path: &std::path::Path) -> i32 {
    use poler_engine::crypto::vault;

    match vault::info(vault_path) {
        Ok(h) => {
            println!("контейнер: {}", vault_path.display());
            println!("  формат: v{} (magic POLERVLT), flags: {:#x}", h.format_version, h.flags);
            println!(
                "  данные: {} байт в {} стр. по 4096 (контейнер: {} байт)",
                h.original_len,
                h.page_count,
                4096 + h.page_count * 4096
            );
            println!("  KDF: {} итераций, epsilon: {:#x}", h.kdf_iterations, h.epsilon);
            if h.flags & poler_engine::crypto::vault::FLAG_CONTENT_ID != 0 {
                println!("  content-id: {}", poler_engine::crypto::hasher::PndHasher::hex(&h.content_id));
            } else {
                println!("  content-id: (не сохранён — приватный режим)");
            }
            0
        }
        Err(e) => {
            eprintln!("poler-vault: {e}");
            2
        }
    }
}


/// Рекурсивный сбор текстовых файлов (без бинарных, ≤2 МБ).
fn collect_text_files(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>, max: usize) -> bool {
    let meta = match std::fs::metadata(root) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("poler-engine: корпус недоступен: {e}");
            return false;
        }
    };
    if meta.is_file() {
        out.push(root.to_path_buf());
        return true;
    }
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>, max: usize) {
        if out.len() >= max {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            match std::fs::metadata(&p) {
                Ok(m) if m.is_dir() => walk(&p, out, max),
                Ok(m) if m.len() <= 2_000_000 && m.len() > 0 => {
                    // пропускаем бинарные: NUL-байт в первой тысяче
                    let mut head = [0u8; 1024];
                    if let Ok(mut fh) = std::fs::File::open(&p) {
                        use std::io::Read;
                        if let Ok(n) = fh.read(&mut head) {
                            if !head[..n].contains(&0) {
                                out.push(p);
                            }
                        }
                    }
                }
                _ => {}
            }
            if out.len() >= max {
                return;
            }
        }
    }
    walk(root, out, max);
    true
}

/// `--llm quantum [--model X.poler] -q "…"`: квантово-фазовый L5-разум
/// (русла циркуляции J, сфера Блоха, Born-лотерея по аттракторам).
fn run_quantum_llm(cli: &Cli) -> i32 {
    let Some(prompt) = &cli.query else {
        eprintln!("poler-engine: --llm quantum требует -q <промпт>");
        return 2;
    };
    let mut mind = if let Some(path) = &cli.model {
        match poler_engine::quantum::QuantumMind::open(path) {
            Ok(mut m) => {
                let corpus_opt = cli.corpus.as_ref().or(cli.semantic_corpus.as_ref());
                if let Some(corpus_path) = corpus_opt {
                    let mut files = Vec::new();
                    if collect_text_files(corpus_path, &mut files, 256) {
                        for f in files {
                            if let Ok(text) = std::fs::read_to_string(f) {
                                let _ = m.ingest(&text, 0);
                            }
                        }
                    }
                }
                let _ = m.ingest(prompt, 0);
                m
            }
            Err(e) => {
                eprintln!("poler-engine: {e}");
                return 2;
            }
        }
    } else {
        match poler_engine::quantum::QuantumMind::new(1024, 42) {
            Ok(mut m) => {
                let corpus_opt = cli.corpus.as_ref().or(cli.semantic_corpus.as_ref());
                if let Some(corpus_path) = corpus_opt {
                    let mut files = Vec::new();
                    if collect_text_files(corpus_path, &mut files, 256) {
                        for f in files {
                            if let Ok(text) = std::fs::read_to_string(f) {
                                let _ = m.ingest(&text, 0);
                            }
                        }
                    }
                }
                let _ = m.ingest(prompt, 0);
                m
            }
            Err(e) => {
                eprintln!("poler-engine: {e}");
                return 2;
            }
        }
    };

    use std::io::Write;
    print!("квантовый ответ: ");
    let _ = std::io::stdout().flush();
    let t0 = std::time::Instant::now();
    let res = mind.generate_stream(prompt, 64, |token| {
        print!("{token} ");
        let _ = std::io::stdout().flush();
        true
    });
    println!();
    match res {
        Ok(rep) => {
            let dt = t0.elapsed().as_secs_f64();
            println!(
                "\nquantum-L5 · {} шагов за {dt:.3} с = {:.1} шаг/с · Born-лотерея по руслам J (релевантность: {:.1}%)",
                rep.steps.len(),
                rep.steps.len() as f64 / dt.max(1e-9),
                rep.relevance * 100.0
            );
            0
        }
        Err(e) => {
            eprintln!("poler-engine quantum: {e}");
            2
        }
    }
}

/// `--jit-loop [FILE]`: замкнутий контур «ваги → машинний код →
/// виконання → пластичність → перекомпіляція».
///
/// Кожен вага шару вшивається в машинний код x86_64 як immediate
/// (`mov eax, <біти f32>`), код виконується прямо з R|X-сторінки,
/// після чого Hebb-правило переписує трити в .t5q і шар
/// перекомпілюється — модель навчается, залишаючись стисненою.
fn run_jit_loop(target: &str) -> i32 {
    use poler_engine::triune::compiler::PlasticityConfig;
    use poler_engine::triune::jit_loop::JitLoop;
    use poler_engine::triune::stream_quant::{stream_quantize, StreamQuantConfig};

    let path: std::path::PathBuf = if target.is_empty() {
        // Синтетичний шар: 4096 LCG-значень → .t5q у тимчасовій папці.
        let mut s = 0x91F100Du64;
        let mut data = Vec::with_capacity(4096 * 4);
        for _ in 0..4096 {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let v = ((s >> 33) as i32 as f64 / i32::MAX as f64) as f32;
            data.extend_from_slice(&v.to_le_bytes());
        }
        let mut out = Vec::new();
        if let Err(e) = stream_quantize(&data[..], &mut out, &StreamQuantConfig::default()) {
            eprintln!("poler-engine: синтетичний шар: {e}");
            return 2;
        }
        let dir = std::env::temp_dir().join("poler_jit_loop_demo");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("layer.t5q");
        if let Err(e) = std::fs::write(&p, &out) {
            eprintln!("poler-engine: тимчасовий файл: {e}");
            return 2;
        }
        p
    } else {
        target.into()
    };

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  JIT LOOP · ВЕСЫ = МАШИННЫЙ КОД · ЗАМКНУТЫЙ КОНТУР       ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!("файл: {}", path.display());

    let mut lp = match JitLoop::open(&path, PlasticityConfig::default(), 0, 32, 32) {
        Ok(lp) => lp,
        Err(e) => {
            eprintln!("poler-engine: {e}");
            return 2;
        }
    };
    println!(
        "слой: {}×{} · машинный код: {} Б · инструкций: {} · плотность: {:.1} Б/вес",
        lp.rows(),
        lp.cols(),
        lp.code_len(),
        lp.instruction_count(),
        lp.code_len() as f32 / (lp.rows() * lp.cols()) as f32
    );
    println!(
        "блоков в .t5q: {} · значений: {}",
        lp.view().block_count(),
        lp.view().values()
    );

    // Фрагмент листинга — ваги видно прямо в інструкціях.
    println!("\nлистинг (первые строки — ваги як immediate у потоці коду):");
    for line in lp.asm_listing().lines().take(14) {
        println!("  {line}");
    }

    // Три цикли контуру з різними фазами входу.
    for cycle in 1..=3u32 {
        let phase = cycle as f32 * 0.35;
        let x: Vec<f32> = (0..32)
            .map(|j| {
                let v = (j as f32 * 0.21 + phase).sin() * 0.9 + 0.05 * ((j % 3) as f32 - 1.0);
                v.clamp(-1.0, 1.0)
            })
            .collect();
        let rep = match lp.cycle(&x) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("poler-engine: цикл {cycle}: {e}");
                return 2;
            }
        };
        let y_norm = rep.y_before.iter().map(|v| v * v).sum::<f32>().sqrt();
        println!(
            "\n─ цикл {cycle} ─ вход ||x||≈{:.2}",
            x.iter().map(|v| v * v).sum::<f32>().sqrt()
        );
        println!(
            "  forward машинним кодом: ||y|| = {y_norm:.4} ({} выходов)",
            rep.y_before.len()
        );
        println!(
            "  Hebb: {} импульсов · in-place переписано тритов: {}",
            rep.impulses, rep.flips
        );
        println!(
            "  commit: вакуум {} → {} · {} мс · sha256 пересчитан",
            rep.zeros_before, rep.zeros_after, rep.commit_ms
        );
        println!(
            "  перекомпіляція: {} Б нового машинного кода ({} инструкций)",
            rep.code_bytes, lp.instruction_count()
        );
        println!(
            "  следующий forward на новых весах: ||Δy|| = {:.4} — сигнал обучения",
            rep.y_delta_norm()
        );
    }

    // Цілісність файлу після трьох циклів мутацій.
    match lp.view().verify() {
        Ok(()) => println!("\nsha256: OK — файл валиден после 3 циклов самомодификации"),
        Err(e) => {
            eprintln!("poler-engine: verify: {e}");
            return 2;
        }
    }
    if target.is_empty() {
        let _ = std::fs::remove_file(&path);
        println!("(демо-файл удалён; для работы с реальными весами: --jit-loop layer.t5q)");
    }
    0
}

/// `--t5q-compile [FILE]`: обратный In-Place компилятор — модель
/// переписывает собственные триты в .t5q без декомпрессии.
/// Без FILE — синтетическая демонстрация полного цикла.
fn run_t5q_compile(target: &str) -> i32 {
    use poler_engine::triune::compiler::{PlasticityCompiler, PlasticityConfig};
    use poler_engine::triune::stream_quant::{stream_quantize, StreamQuantConfig};

    let mut demo_path: Option<std::path::PathBuf> = None;
    let path: std::path::PathBuf = if target.is_empty() {
        // Синтетический поток: 4096 значений LCG → .t5q во временной папке.
        let mut s = 0xC0FFEEu64;
        let mut data = Vec::with_capacity(4096 * 4);
        for _ in 0..4096 {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let v = ((s >> 33) as i32 as f64 / i32::MAX as f64) as f32;
            data.extend_from_slice(&v.to_le_bytes());
        }
        let mut out = Vec::new();
        if let Err(e) = stream_quantize(&data[..], &mut out, &StreamQuantConfig::default()) {
            eprintln!("poler-engine: синтетический поток: {e}");
            return 2;
        }
        let dir = std::env::temp_dir().join("poler_t5q_compile_demo");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("demo.t5q");
        if let Err(e) = std::fs::write(&p, &out) {
            eprintln!("poler-engine: временный файл: {e}");
            return 2;
        }
        demo_path = Some(p.clone());
        p
    } else {
        target.into()
    };

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║   INVERSE IN-PLACE COMPILER · триты переписывают себя    ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!("файл: {}", path.display());

    let mut compiler = match PlasticityCompiler::open(&path, PlasticityConfig::default()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("poler-engine: {e}");
            return 2;
        }
    };
    let blocks = compiler.view().block_count();
    let values = compiler.view().values();
    println!("блоков: {blocks} · значений: {values} · вакуум: пересчитывается при commit");

    // 1. Хеббовские импульсы: пара активаций пре/пост.
    let pre = vec![0.9f32, -0.2, 0.05, 0.0, 0.7, 0.0, -0.6, 0.1];
    let post = vec![0.8f32, 0.0, -0.9, 0.05, 0.0, 0.6, 0.0, -0.05];
    let hebb = match compiler.hebbian_impulses(0, 8, 8, &pre, &post) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("poler-engine: хебб: {e}");
            return 2;
        }
    };
    let mut impulses = hebb;

    // 2. Фазовый ротор J = A − Aᵀ: доминирующий поток ↔ встречный канал.
    let mut a = vec![0.0f32; 64];
    a[1 * 8 + 0] = 0.9;
    a[1] = -0.4; // a[0][1] row-major 8×8 — встречный канал
    match compiler.phase_rotor_impulses(0, &a, 8) {
        Ok(mut v) => impulses.append(&mut v),
        Err(e) => {
            eprintln!("poler-engine: фазовый ротор: {e}");
            return 2;
        }
    }
    println!("импульсов мутаций: {} (STDP + фазовый ротор)", impulses.len());

    // 3. In-place применение (без распаковки в FP16).
    match compiler.apply(&impulses) {
        Ok(changed) => println!("тритов переписано: {changed}"),
        Err(e) => {
            eprintln!("poler-engine: apply: {e}");
            return 2;
        }
    }

    // 4. Адаптация масштаба первого блока (STDP-пластичность масштаба).
    let _ = compiler.adapt_scale(0, 1.1);

    // 5. Атомарная фиксация: пересчёт вакуума + sha256 + msync.
    match compiler.commit() {
        Ok(stats) => {
            println!(
                "commit: {} флипов · {} масштабов · вакуум {} → {} · {} мс",
                stats.flips, stats.scale_updates, stats.zeros_before, stats.zeros_after, stats.ms
            );
        }
        Err(e) => {
            eprintln!("poler-engine: commit: {e}");
            return 2;
        }
    }

    // 6. Финальная валидация.
    if let Err(e) = compiler.view().verify() {
        eprintln!("poler-engine: файл невалиден после commit: {e}");
        return 2;
    }
    println!("sha256: OK — файл валиден, мутации зафиксированы на диске");

    if let Some(p) = demo_path {
        let dir = p.parent().unwrap().to_path_buf();
        let _ = std::fs::remove_file(&p);
        let _ = std::fs::remove_dir(&dir);
        println!("(демо-файл удалён; для работы с реальными весами: --t5q-compile model.t5q)");
    }
    0
}

/// `--llm local --model X.pqw -q "…"`: нативный GLM-декодер (RoPE + MQA +
/// SwiGLU + MoE) через pqc. remote/auto — мост Фазы 12.9 (заглушка).
fn run_llm(mode: &str, cli: &Cli) -> i32 {
    match mode {
        "quantum" => return run_quantum_llm(cli),
        "local" => {}
        "remote" | "auto" => {
            eprintln!(
                "poler-engine: --llm {mode} — серверный GLM-мост это Фаза 12.9 (не встроен);\n  \
                 сейчас доступны автономные режимы: --llm quantum, --llm local --model glm.pqw"
            );
            return 2;
        }
        _ => {
            eprintln!(
                "poler-engine: неизвестный --llm режим {mode:?} (доступны: quantum, local, remote, auto)"
            );
            return 2;
        }
    }
    let Some(model_path) = &cli.model else {
        eprintln!(
            "poler-engine: --llm local требует --model <glm.pqw>\n  \
             конвертер PyTorch GLM → .pqw (int4) — Фаза 12.7;\n  \
             проверить декодер сейчас: poler-engine --pqw-selftest"
        );
        return 2;
    };
    let Some(prompt) = &cli.query else {
        eprintln!("poler-engine: --llm local требует -q <промпт>");
        return 2;
    };
    let model = match poler_engine::llm::glm_engine::GlmModel::open(model_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("poler-engine: {e}");
            return 2;
        }
    };
    // _has_real_tok: признак «реальный токенизатор vs hash-фолбэк» —
    // диагностика; поведение генерации от него не ветвится.
    let (prompt_ids, _has_real_tok) = if let Some(tok) = model.tokenizer() {
        let formatted = if prompt.contains("[gMASK]") || prompt.contains("<|user|>") {
            prompt.clone()
        } else {
            format!("[gMASK]sop<|user|>\n{prompt}<|assistant|>")
        };
        let mut ids = tok.encode(&formatted);
        // If encode added bos (id=0 or 1) and eos (id=2), remove them if [gMASK]/sop are present
        if ids.first() == Some(&0) || ids.first() == Some(&1) {
            ids.remove(0);
        }
        if ids.last() == Some(&2) {
            ids.pop();
        }
        (ids, true)
    } else {
        (poler_engine::pqc::hash_token_ids(prompt, model.vocab() as u32), false)
    };
    if prompt_ids.is_empty() {
        eprintln!("poler-engine: пустой промпт — нечего генерировать");
        return 2;
    }
    let max_new = 128usize;
    let t0 = std::time::Instant::now();
    use std::io::Write;
    print!("ответ: ");
    let _ = std::io::stdout().flush();
    let out = match model.generate_stream(
        &prompt_ids,
        max_new,
        &poler_engine::llm::glm_engine::Sampling::Greedy,
        0,
        |_tok, piece| {
            print!("{piece}");
            let _ = std::io::stdout().flush();
            true
        },
    ) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("\npoler-engine: {e}");
            return 2;
        }
    };
    println!();
    let dt = t0.elapsed().as_secs_f64();
    println!(
        "\nglm-local · {} токенов за {dt:.3} с = {:.1} ток/с · greedy",
        out.len(),
        out.len() as f64 / dt.max(1e-9)
    );
    0
}

/// `--ner gliner --model X.pqw -q "…"`: нативная span-голова GLiNER через pqc.
///
/// model_type=Gliner (реальные чекпойнты, mdeberta-спина): zero-shot метки
/// через `--ner-labels`, пословленная токенизация из секции __tokenizer__.
/// model_type=SpanNer (синтетика/selftest): метки фиксированы в __labels__.
fn run_ner(what: &str, cli: &Cli) -> i32 {
    if what != "gliner" {
        eprintln!("poler-engine: неизвестный --ner {what:?} (доступен: gliner)");
        return 2;
    }
    let Some(model_path) = &cli.model else {
        eprintln!(
            "poler-engine: --ner gliner требует --model <gliner.pqw>\n  \
             конвертер реальных весов: scripts/convert_gliner_to_pqw.py;\n  \
             проверить голову сейчас: poler-engine --pqw-selftest"
        );
        return 2;
    };
    let Some(text) = &cli.query else {
        eprintln!("poler-engine: --ner gliner требует -q <текст>");
        return 2;
    };
    // Маршрутизация по model_type из заголовка .pqw.
    let real = match poler_engine::pqc::pqw::QuantizedWeightsView::open(model_path) {
        Ok(v) => matches!(v.header().model_type, poler_engine::pqc::pqw::ModelType::Gliner),
        Err(e) => {
            eprintln!("poler-engine: {e}");
            return 2;
        }
    };
    if real {
        let labels: Vec<&str> = match &cli.ner_labels {
            Some(l) => l.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect(),
            None => {
                eprintln!(
                    "poler-engine: --ner gliner (реальный чекпойнт) требует \n  \
                     --ner-labels \"метка1,метка2,…\" (zero-shot список типов сущностей)"
                );
                return 2;
            }
        };
        let model = match poler_engine::ner::RealGlinerModel::open(model_path) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("poler-engine: {e}");
                return 2;
            }
        };
        let t0 = std::time::Instant::now();
        let entities = match model.predict(text, &labels, 0.5) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("poler-engine: {e}");
                return 2;
            }
        };
        println!(
            "gliner-native · {} сущностей · метки [{}] · {:.1} мс",
            entities.len(),
            labels.join(", "),
            t0.elapsed().as_secs_f64() * 1000.0
        );
        for e in entities.iter().take(20) {
            println!("  {}:{} [{:.2}] {}..{}", e.label, e.text, e.score, e.start, e.end);
        }
        if entities.len() > 20 {
            println!("  … и ещё {}", entities.len() - 20);
        }
        return 0;
    }
    let model = match poler_engine::ner::native_gliner::GlinerModel::open(model_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("poler-engine: {e}");
            return 2;
        }
    };
    use unicode_segmentation::UnicodeSegmentation;
    let words: Vec<String> = text.unicode_words().map(|w| w.to_string()).collect();
    if words.is_empty() {
        eprintln!("poler-engine: текст без UAX#29-слов");
        return 2;
    }
    let vocab = model.vocab() as u32;
    let ids: Vec<u32> = words
        .iter()
        .map(|w| {
            poler_engine::pqc::hash_token_ids(w, vocab)
                .first()
                .copied()
                .unwrap_or(0)
        })
        .collect();
    let word_refs: Vec<&str> = words.iter().map(|s| s.as_str()).collect();
    let t0 = std::time::Instant::now();
    let entities = match model.extract(&word_refs, &ids, 0.5) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("poler-engine: {e}");
            return 2;
        }
    };
    println!(
        "gliner-native · {} сущностей · метки [{}] · {:.1} мс",
        entities.len(),
        model.labels().join(", "),
        t0.elapsed().as_secs_f64() * 1000.0
    );
    for e in entities.iter().take(20) {
        println!("  {}:{} [{:.2}] {}..{}", e.label, e.text, e.score, e.start, e.end);
    }
    if entities.len() > 20 {
        println!("  … и ещё {}", entities.len() - 20);
    }
    0
}

// ══════════════════════════════════════════════════════════════════
// L1/v0.34.0: Литературный Двигатель POLER[Ψ]
// ══════════════════════════════════════════════════════════════════

/// Режим CLI Литературного Двигателя.
// S1/v0.35.0: Синаптический Вихрь SSN — доказанное до реализации ядро
// (65→67 проверок Python-сьюта, 10 seed × 10k шагов Rust full-fidelity).
fn run_ssn(cli: &Cli) -> i32 {
    use poler_engine::ssn;
    use serde_json::json;

    let cfg = ssn::VortexConfig {
        n: cli.ssn_n,
        fields: cli.ssn_fields,
        ..Default::default()
    };

    // ── Режим 1: --ssn-encode [+ --ssn-encode-b] — сенсорная мера CSE ──
    if let Some(text) = cli.ssn_encode.clone() {
        let v = ssn::cse::encode(&text, cli.ssn_dims);
        match cli.ssn_encode_b.clone() {
            Some(other) => {
                let w = ssn::cse::encode(&other, cli.ssn_dims);
                let cos = ssn::cse::cos_sim(&v, &w);
                let sin = ssn::cse::sin_corrected(&v, &w);
                if cli.ssn_json {
                    println!(
                        "{}",
                        json!({
                            "mode": "ssn-encode-pair",
                            "a": text, "b": other, "dims": cli.ssn_dims,
                            "cos": (cos * 1e4).round() / 1e4,
                            "sin_corrected": (sin * 1e4).round() / 1e4,
                            "verdict": if cos > 0.5 { "похожи" } else if cos < 0.0 { "непохожи" } else { "нейтральны" },
                        })
                    );
                } else {
                    println!("CSE-сходство (D={}): cos = {:+.4}, sin-коррекция = {:+.4}", cli.ssn_dims, cos, sin);
                    println!("вердикт: {}", if cos > 0.5 { "похожи" } else if cos < 0.0 { "непохожи" } else { "нейтральны" });
                }
            }
            None => {
                if cli.ssn_json {
                    let head: Vec<f64> = v.iter().take(8).map(|x| (x * 1e4).round() / 1e4).collect();
                    println!(
                        "{}",
                        json!({
                            "mode": "ssn-encode", "dims": cli.ssn_dims,
                            "chars": text.chars().count(),
                            "norm": ssn::cse::norm(&v),
                            "head": head,
                        })
                    );
                } else {
                    println!("CSE-вектор (D={}, {} символов, норма {:.6}):", cli.ssn_dims, text.chars().count(), ssn::cse::norm(&v));
                    for (i, x) in v.iter().take(8).enumerate() {
                        println!("  v[{i:3}] = {x:+.6}");
                    }
                }
            }
        }
        return 0;
    }

    // ── Режим 2: --ssn-demo / --ssn-inject — живой мозг ──
    let t0 = std::time::Instant::now();
    let mut eng = ssn::SsnEngine::new(cfg, cli.ssn_seed, cli.ssn_dims);

    let injected = cli.ssn_inject.clone().map(|text| {
        let (chars, touched) = eng.inject_text(&text);
        (text, chars, touched)
    });

    let steps = cli.ssn_steps.max(1);
    let checkpoint = (steps / 10).max(1);
    let mut traj: Vec<(usize, f64)> = Vec::new();
    if !cli.ssn_json {
        println!(
            "Синаптический Вихрь SSN: N={} нейронов × {} полей = {} синапсов (виртуальная топология, 0 RAM на граф)",
            cli.ssn_n,
            cli.ssn_fields,
            cli.ssn_n * cli.ssn_fields
        );
        if let Some((text, chars, touched)) = &injected {
            println!("сенсорная инъекция: «{text}» — {chars} символов → {touched} нейронов");
        }
        println!("прогон {steps} шагов (доказанный режим: цель 5%, порог спайка 0.1):");
        println!("  шаг    акт  f_sys  b_тон    S     C   E/I  GABA  DA   5HT   NE");
    }
    for i in 0..steps {
        let act = eng.step();
        let done = i + 1;
        if done % checkpoint == 0 || done == steps {
            traj.push((done, act));
            if !cli.ssn_json {
                let t = eng.telemetry();
                println!(
                    "{:>6} {:>6.4} {:>6.4} {:>6.3} {:>5.2} {:>5.2} {:>5.2} {:>5.2} {:>4.2} {:>5.2} {:>5.2}",
                    done, t.activity, t.f_sys, t.b_tone, t.synchrony, t.criticality, t.ei, t.gaba, t.da, t.ht, t.ne
                );
            }
        }
    }

    let t = eng.telemetry();
    let readout = eng.readout(cli.ssn_readout);
    let elapsed = t0.elapsed();
    let sps = steps as f64 / elapsed.as_secs_f64();

    if cli.ssn_json {
        let traj_json: Vec<serde_json::Value> = traj
            .iter()
            .map(|(s, a)| json!({"step": s, "activity": (a * 1e4).round() / 1e4}))
            .collect();
        let readout_json: Vec<serde_json::Value> = readout
            .iter()
            .map(|(i, v)| json!({"neuron": i, "activation": (v * 1e4).round() / 1e4}))
            .collect();
        println!(
            "{}",
            json!({
                "mode": if injected.is_some() { "ssn-inject" } else { "ssn-demo" },
                "n": cli.ssn_n, "fields": cli.ssn_fields, "seed": cli.ssn_seed,
                "steps": steps,
                "injected_text": injected.as_ref().map(|(t, _, _)| t.clone()),
                "telemetry": {
                    "activity": (t.activity * 1e4).round() / 1e4,
                    "f_sys": (t.f_sys * 1e4).round() / 1e4,
                    "b_tone": (t.b_tone * 1e4).round() / 1e4,
                    "synchrony": (t.synchrony * 1e4).round() / 1e4,
                    "criticality": (t.criticality * 1e4).round() / 1e4,
                    "ei": (t.ei * 1e2).round() / 1e2,
                    "gaba": t.gaba, "glut": t.glut, "da": t.da, "ht_5ht": t.ht, "ne": t.ne,
                    "w_min": (t.w_min * 1e3).round() / 1e3, "w_max": (t.w_max * 1e3).round() / 1e3,
                    "w_at_clip": (t.w_at_clip * 1e4).round() / 1e4,
                    "active_count": t.active_count,
                },
                "readout": readout_json,
                "trajectory": traj_json,
                "elapsed_ms": elapsed.as_millis() as u64,
                "steps_per_sec": sps as u64,
            })
        );
    } else {
        println!("\nздоровье мозга: активность {:.1}% (цель 5%), S={:.2} (эпилепсия <0.8), C={:.2} (критичность), E/I={:.1} (норма ~4)",
            t.activity * 100.0, t.synchrony, t.criticality, t.ei);
        println!("веса: [{:+.3}, {:+.3}], у клипа {:.1}% | тон={:.3} | DA={:.2} 5HT={:.2} NE={:.2} | GABA={:.2} глут={:.2}",
            t.w_min, t.w_max, t.w_at_clip * 100.0, t.b_tone, t.da, t.ht, t.ne, t.gaba, t.glut);
        if !readout.is_empty() {
            print!("readout (топ-{}): ", readout.len());
            for (i, v) in &readout {
                print!("[{i}] {v:.3}  ");
            }
            println!();
        }
        println!("время: {} мс ({:.0} шагов/с)", elapsed.as_millis(), sps);
    }
    0
}

// ---------- S2/v0.36.0: Триединая Архитектура (муха + вихрь + кристалл) ----------

/// Загрузка кристалла: внешний .t5c, постоянная память или зашитый в бинарник.
fn triune_load_crystal(cli: &Cli) -> Result<poler_engine::triune::Crystal, String> {
    if let Some(path) = &cli.triune_crystal {
        let bytes = std::fs::read(path).map_err(|e| format!("чтение {path:?}: {e}"))?;
        return poler_engine::triune::Crystal::load(&bytes, poler_engine::triune::crystal::DEFAULT_DIMS);
    }
    // Автоматический поиск постоянной обученной памяти в глобальных и локальных путях
    let mut search_paths = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        search_paths.push(std::path::PathBuf::from(home).join(".poler").join("permanent_memory.t5c"));
    }
    search_paths.push(std::path::PathBuf::from("permanent_memory.t5c"));
    search_paths.push(std::path::PathBuf::from("mega_corpus.t5c"));
    search_paths.push(std::path::PathBuf::from("/home/vitalij/Стільниця/poler-engine/mega_corpus.t5c"));

    for p in search_paths {
        if p.exists() {
            if let Ok(bytes) = std::fs::read(&p) {
                if let Ok(c) = poler_engine::triune::Crystal::load(&bytes, poler_engine::triune::crystal::DEFAULT_DIMS) {
                    return Ok(c);
                }
            }
        }
    }
    poler_engine::triune::Crystal::embedded()
}

/// Пульс мухи: настоящая каста из коннектома или виртуальная (seed).
fn triune_load_fly(cli: &Cli) -> Result<poler_engine::triune::FlyPulse, String> {
    match &cli.triune_connectome {
        Some(csr) => {
            let con = poler_engine::graph::connectome::Connectome::load(csr)
                .map_err(|e| format!("коннектом {csr:?}: {e}"))?;
            let seeds: Vec<usize> = cli
                .triune_seeds
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if seeds.is_empty() {
                return Err("--triune-seeds: укажите CSV индексов нейронов".into());
            }
            let cast = poler_engine::triune::flypulse::cast_from_connectome(&con, &seeds, 2)?;
            Ok(poler_engine::triune::FlyPulse::from_cast(&cast, cli.triune_gamma))
        }
        None => Ok(poler_engine::triune::FlyPulse::synthetic(cli.triune_seed, cli.triune_gamma)),
    }
}

/// Телеметрия вихря → JSON (общая для run_triune и MCP).
fn triune_telemetry_json(t: &poler_engine::ssn::VortexTelemetry) -> serde_json::Value {
    let r = |x: f64| (x * 1e6).round() / 1e6;
    serde_json::json!({
        "step": t.step, "activity": r(t.activity), "f_sys": r(t.f_sys),
        "b_tone": r(t.b_tone), "synchrony": r(t.synchrony),
        "criticality": r(t.criticality), "ei": r(t.ei),
        "gaba": r(t.gaba), "glut": r(t.glut),
        "da": r(t.da), "ht_5": r(t.ht), "ne": r(t.ne),
        "w_min": r(t.w_min), "w_max": r(t.w_max), "w_at_clip": r(t.w_at_clip),
        "active_count": t.active_count,
    })
}

/// Высказывание → JSON (агентам: полный провенанс каждого токена).
fn triune_utterance_json(u: &poler_engine::triune::Utterance) -> serde_json::Value {
    use poler_engine::triune::PulseOrigin;
    let r = |x: f64| (x * 1e4).round() / 1e4;
    let trace: Vec<serde_json::Value> = u
        .trace
        .iter()
        .map(|t| {
            serde_json::json!({
                "token": t.token, "idx": t.idx,
                "sem": r(t.sem), "gate": r(t.gate), "syn": t.syn,
                "fly": r(t.fly), "rep": r(t.rep), "score": r(t.score),
                "tau": r(t.tau), "activity": r(t.activity), "act": r(t.act),
            })
        })
        .collect();
    let intents: Vec<serde_json::Value> = u
        .intents
        .iter()
        .map(|i| {
            serde_json::json!({
                "verb": i.verb, "op": i.op.as_str(), "object": i.object,
                "proposal": i.proposal, "allowed": i.allowed, "reason": i.reason,
            })
        })
        .collect();
    let fly = match &u.fly_origin {
        PulseOrigin::Connectome { members, top_rotor } => serde_json::json!({
            "kind": "connectome", "members": members, "top_rotor": top_rotor
        }),
        PulseOrigin::Synthetic { seed } => serde_json::json!({
            "kind": "synthetic", "seed": seed
        }),
    };
    serde_json::json!({
        "text": u.text,
        "tokens": u.trace.len(),
        "crystal_vocab": u.crystal_vocab,
        "fly": fly,
        "fly_drift": u.fly_drift.iter().map(|v| r(*v as f64)).collect::<Vec<_>>(),
        "telemetry": triune_telemetry_json(&u.telemetry),
        "trace": trace,
        "motor_intents": intents,
    })
}

fn run_triune(cli: &Cli) -> i32 {
    use poler_engine::triune::{TriuneConfig, TriuneCore};
    use serde_json::json;

    let t0 = std::time::Instant::now();
    let crystal = match triune_load_crystal(cli) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("triune: {e}");
            return 2;
        }
    };
    let fly = match triune_load_fly(cli) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("triune: {e}");
            return 2;
        }
    };
    let mut cfg = TriuneConfig::default();
    if cli.triune_no_learn {
        cfg.neurons.learn = false;
    }
    let mut core = TriuneCore::new(crystal, cfg, fly, cli.triune_seed);

    // S2→E2/v0.38.1: моторный мост — двухуровневый контур исполнения.
    let bridge = poler_engine::triune::motor_bridge::BridgeConfig {
        act: cli.motor_act,
        yes: cli.motor_yes,
        timeout_ms: cli.motor_timeout_ms,
        max_out: cli.motor_max_out,
    };
    if bridge.act {
        // Баннер в stderr: stdout остаётся чистым для --triune-json.
        eprintln!("┌─ МОТОРНЫЙ МОСТ АКТИВЕН (S2→E2) ────────────────────────────");
        eprintln!("│ R1 ReadOnly: авто-исполнение (статус, логи, чтение файлов)");
        if bridge.yes {
            eprintln!("│ M2 Mutating: АВТО (--motor-yes; ответственность на операторе!)");
        } else {
            eprintln!("│ M2 Mutating: подтверждение [y/N] (нет tty → отказ; --motor-yes для авто)");
        }
        eprintln!("│ таймаут {} мс (SIGTERM→2с→SIGKILL), вывод ≤ {} байт (head+tail)",
            bridge.timeout_ms, bridge.max_out);
        eprintln!("└────────────────────────────────────────────────────────────");
    }
    let mut motor_log: Vec<(String, serde_json::Value)> = Vec::new();

    let mut utterances = Vec::new();

    if let Some(evolve_secs) = cli.triune_auto_evolve {
        println!("╔════════════════════════════════════════════════════════════╗");
        println!("║  АВТОНОМНА ЕВОЛЮЦІЯ ТА НЕПРЕРВНЕ НАВЧАННЯ ТРИЄДНОСТІ ({} с) ║", evolve_secs);
        println!("╚════════════════════════════════════════════════════════════╝");
        println!("тривалість: {} секунд | кристалл: {} токенів | STDP: активний\n",
            evolve_secs, core.crystal.vocab());

        let mut cycle = 0;
        let mut cur_prompt = cli.triune_speak.clone().unwrap_or_else(|| "квантовий ротор пам'яті".into());
        let deadline = t0 + std::time::Duration::from_secs(evolve_secs);
        let mut recent_tokens_history: std::collections::VecDeque<String> = std::collections::VecDeque::with_capacity(30);

        let exploration_seeds = [
            "квантова редукція фази",
            "топологічний інваріант поля",
            "синаптична пластичність і пам'ять",
            "ентропійний градієнт вихору",
            "динаміка збудження та гальмування",
            "фрактальний атрактор свідомості",
            "нейромедіаторний баланс дофаміну",
            "когерентний синтез інформації",
        ];

        while std::time::Instant::now() < deadline {
            cycle += 1;
            let u = core.speak(&cur_prompt, cli.triune_tokens.min(24));
            let speech_text = u.text.clone();
            let words: Vec<&str> = speech_text.split_whitespace().collect();

            // Перевірка на цифрову епілепсію / граничний цикл (Limit-Cycle Attractor Trap)
            let mut repeat_count = 0;
            for w in &words {
                if recent_tokens_history.iter().any(|past| past == *w) {
                    repeat_count += 1;
                }
            }
            let is_epileptic_loop = words.is_empty() || (words.len() > 3 && repeat_count * 100 / words.len() > 50);

            for w in &words {
                if recent_tokens_history.len() >= 30 {
                    recent_tokens_history.pop_front();
                }
                recent_tokens_history.push_back((*w).to_string());
            }

            if is_epileptic_loop {
                // Анти-епілептичний зрив резонансу: введення свіжого семантичного імпульсу
                let seed_idx = (cycle + (u.telemetry.step as usize)) % exploration_seeds.len();
                cur_prompt = exploration_seeds[seed_idx].to_string();
            } else {
                // Вільний потік асоціативного ланцюжка
                let next_words: Vec<&str> = speech_text.split_whitespace().rev().take(3).collect();
                cur_prompt = if !next_words.is_empty() {
                    next_words.into_iter().rev().collect::<Vec<_>>().join(" ")
                } else {
                    "синтез структури системи".into()
                };
            }

            let rem = deadline.saturating_duration_since(std::time::Instant::now()).as_secs();
            let b = &u.telemetry;
            let loop_marker = if is_epileptic_loop { " [⚡зрив петлі]" } else { "" };
            println!("[Цикл {:03} | лишилось {:02}хв {:02}с] синапсів: {} | DA={:.2} C={:.2}{} | «{}»",
                cycle, rem / 60, rem % 60, core.crystal.bigram_nonzeros, b.da, b.criticality, loop_marker, speech_text);

            // S2→E2: исполнение моторных интентов цикла (двухуровневый контур).
            // Сканируем и ПРОМПТ (директива пользователя), и речь кристалла:
            // команда может прийти извне или родиться внутри речи.
            if bridge.act {
                let mut all_intents = poler_engine::triune::motor::scan(&cur_prompt);
                all_intents.extend(u.intents.iter().cloned());
                let events = poler_engine::triune::motor_bridge::act_on_intents(&all_intents, &bridge);
                if !cli.triune_json {
                    for ev in &events {
                        eprintln!("  ⚙ мотор [{}] {} → {}: {}",
                            ev["level"].as_str().unwrap_or("-"),
                            ev["verb"].as_str().unwrap_or("?"),
                            ev["program"].as_str().unwrap_or("—"),
                            ev["status"].as_str().unwrap_or("?"));
                    }
                }
                for ev in events {
                    motor_log.push((format!("цикл-{cycle}"), ev));
                }
            }

            utterances.push((format!("цикл-{cycle}"), u));
            std::thread::sleep(std::time::Duration::from_millis(400));
        }

        // Автозбереження навченого стану в постійну пам'ять
        // v0.38.1: глобальний шлях — через $HOME (портабельно; на машині
        // розробника це той самий /home/vitalij/.poler/), а не хардкод.
        if let Some(home) = std::env::var_os("HOME") {
            let global = std::path::Path::new(&home).join(".poler").join("permanent_memory.t5c");
            if let Some(dir) = global.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = core.crystal.save(&global);
        }
        let _ = core.crystal.save(std::path::Path::new("permanent_memory.t5c"));
    } else {
        let prompts: Vec<String> = if cli.triune_demo {
            vec![
                "живой мозг говорит".into(),
                "система слушает сенсорный вход".into(),
                "открой терминал и покажи статус".into(),
            ]
        } else {
            vec![cli.triune_speak.clone().unwrap_or_default()]
        };

        for prompt in &prompts {
            let u = core.speak(prompt, cli.triune_tokens);
            // S2→E2: исполнение моторных интентов. Промпт — директивный канал
            // пользователя («покажи статус»), речь — канал кристалла (если
            // он сам скажет «запусти сборку» — тоже исполнится по правилам).
            if bridge.act {
                let mut all_intents = poler_engine::triune::motor::scan(prompt);
                all_intents.extend(u.intents.iter().cloned());
                let events = poler_engine::triune::motor_bridge::act_on_intents(&all_intents, &bridge);
                if !cli.triune_json {
                    for ev in &events {
                        eprintln!("⚙ мотор [{}] {} → {}: {}",
                            ev["level"].as_str().unwrap_or("-"),
                            ev["verb"].as_str().unwrap_or("?"),
                            ev["program"].as_str().unwrap_or("—"),
                            ev["status"].as_str().unwrap_or("?"));
                    }
                }
                for ev in events {
                    motor_log.push((prompt.clone(), ev));
                }
            }
            utterances.push((prompt.clone(), u));
        }
    }

    if cli.triune_json {
        let arr: Vec<serde_json::Value> = utterances
            .iter()
            .map(|(p, u)| {
                json!({"prompt": p, "seed": cli.triune_seed, "gamma": cli.triune_gamma,
                       "utterance": triune_utterance_json(u)})
            })
            .collect();
        let mut payload = json!({"mode": "triune", "elapsed_ms": t0.elapsed().as_millis(), "speech": arr});
        if bridge.act {
            payload["motor_exec"] = json!(motor_log.iter()
                .map(|(src, ev)| json!({"source": src, "event": ev}))
                .collect::<Vec<_>>());
        }
        println!("{payload}");
        return 0;
    }

    // Человекочитаемая витрина.
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   ТРИЕДИНАЯ АРХИТЕКТУРА: муха + вихрь + кристалл → речь    ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    let fly_desc = match utterances.first().map(|(_, u)| &u.fly_origin) {
        Some(poler_engine::triune::PulseOrigin::Connectome { members, top_rotor }) => {
            format!("настоящая каста FLYCSR1: {members} нейронов, топ-ротор {top_rotor}")
        }
        Some(poler_engine::triune::PulseOrigin::Synthetic { seed }) => {
            format!("виртуальная муха (seed {seed}; подключи --triune-connectome для настоящей)")
        }
        None => "—".into(),
    };
    println!("муха:    {fly_desc}");
    println!("вихрь:   SSN v0.35.0, гомеостаз 5%, критичность на краю хаоса");
    println!("кристалл: {} токенов, Trit5 (5 тритов/байт), зашит в бинарник",
        utterances.first().map(|(_, u)| u.crystal_vocab).unwrap_or(0));
    println!("нейроны: v0.38.0, {} живых, ε={:.2}, S={:.2}, Хебб-обновлений: {}",
        utterances.last().map(|(_, u)| u.neurons.active_k).unwrap_or(0),
        utterances.last().map(|(_, u)| u.neurons.energy).unwrap_or(0.0),
        utterances.last().map(|(_, u)| u.neurons.entropy).unwrap_or(0.0),
        utterances.last().map(|(_, u)| u.neurons.hebb_updates).unwrap_or(0));
    for (prompt, u) in &utterances {
        println!("\n─ промпт: «{prompt}» ─────────────────────────────────");
        println!("речь: {text}", text = u.text);
        let t = &u.telemetry;
        println!("мозг: активность {:.1}%, S={:.2}, C={:.2}, DA={:.2} 5HT={:.2} NE={:.2}",
            t.activity * 100.0, t.synchrony, t.criticality, t.da, t.ht, t.ne);
        if let Some(first) = u.trace.first() {
            println!("первый токен: sem={:+.3} gate={:.3} syn={:+} fly={:+.3} act={:.3} τ={:.2}",
                first.sem, first.gate, first.syn, first.fly, first.act, first.tau);
        }
        if !u.intents.is_empty() {
            println!("моторный слой (S2):");
            for i in &u.intents {
                if i.allowed {
                    println!("  [допущено] {} → {}", i.verb, i.proposal);
                } else {
                    println!("  [отклонено] {} — {}", i.verb, i.reason);
                }
            }
        }
    }
    let (inj, spoken, _) = core.state_summary();
    println!("\nитог: {} фраз, {} токенов, {} сенсорных инъекций, {} мс",
        utterances.len(), spoken, inj, t0.elapsed().as_millis());
    if bridge.act {
        let count = |st: &str| motor_log.iter().filter(|(_, e)| e["status"] == st).count();
        println!("мост S2→E2: {} моторных событий (ok={}, refused={}, unresolved={}, error/timeout={})",
            motor_log.len(), count("ok"), count("refused"), count("unresolved"),
            motor_log.len() - count("ok") - count("refused") - count("unresolved"));
    }
    // v0.38.0: сохранение синапсов, обученных в сессии (непрерывное обучение).
    if !cli.triune_no_learn {
        let save_target = if let Some(out) = &cli.triune_out {
            Some(out.clone())
        } else if let Some(c_path) = &cli.triune_crystal {
            Some(c_path.clone())
        } else if std::path::Path::new("mega_corpus.t5c").exists() {
            Some(std::path::PathBuf::from("mega_corpus.t5c"))
        } else if std::path::Path::new("permanent_memory.t5c").exists() {
            Some(std::path::PathBuf::from("permanent_memory.t5c"))
        } else {
            None
        };

        if let Some(target) = save_target {
            match core.crystal.save(&target) {
                Ok(()) => println!("память обновлена (онлайн-обучение): {} ({} токенов, {} синапсов)",
                    target.display(), core.crystal.vocab(), core.crystal.bigram_nonzeros),
                Err(e) => eprintln!("triune auto-save: {e}"),
            }
        }
    }
    0
}

fn run_triune_crystal_inspect(cli: &Cli) -> i32 {
    let crystal = match triune_load_crystal(cli) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("помилка завантаження кристала: {e}");
            return 2;
        }
    };

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║         ІНСПЕКЦІЯ ТРІЙКОВОГО КРИСТАЛА ПАМ'ЯТІ (Trit5)       ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!("• Розмірність словника: {} унікальних токенів", crystal.vocab());
    println!("• Активних ненульових синапсів: {} зв'язків", crystal.bigram_nonzeros);
    println!("• Опрацьовано корпусу: {} слів ({} символів)", crystal.corpus_words, crystal.corpus_chars);
    println!("• Поріг зв'язку θ_hi: {:.3}", crystal.theta_hi());

    if let Some(target_word) = &cli.triune_crystal_inspect {
        let query = target_word.to_lowercase();
        let target_id = crystal.id_of(&query);

        println!("\n── ДЕТАЛЬНИЙ АНАЛІЗ ТОКЕНА «{}» ────────────────────────", target_word);
        match target_id {
            Some(tid) => {
                println!("• Внутрішній ID: {}", tid);
                let mut exc_synapses = Vec::new();
                let mut inh_synapses = Vec::new();

                for other_id in 0..(crystal.vocab() as u32) {
                    if other_id == tid {
                        continue;
                    }
                    let trit = crystal.bigram_trit(tid, other_id);
                    if trit > 0 {
                        exc_synapses.push(crystal.tokens[other_id as usize].clone());
                    } else if trit < 0 {
                        inh_synapses.push(crystal.tokens[other_id as usize].clone());
                    }
                }

                println!("• Збуджувальні синапси (+1, притягує): {} токенів", exc_synapses.len());
                if !exc_synapses.is_empty() {
                    println!("  → Топ ланцюжок: {}", exc_synapses.iter().take(20).cloned().collect::<Vec<_>>().join(", "));
                }

                println!("• Гальмівні синапси (-1, відштовхує): {} токенів", inh_synapses.len());
                if !inh_synapses.is_empty() {
                    println!("  → Топ гальмування: {}", inh_synapses.iter().take(20).cloned().collect::<Vec<_>>().join(", "));
                }
            }
            None => {
                println!("⚠ Токен «{}» відсутній у поточному словнику кристала.", target_word);
                let similar: Vec<&str> = crystal.tokens.iter()
                    .filter(|t| t.contains(&query) || query.contains(t.as_str()))
                    .take(8)
                    .map(|s| s.as_str())
                    .collect();
                if !similar.is_empty() {
                    println!("  Можливо, ви мали на увазі: {}", similar.join(", "));
                }
            }
        }
    } else {
        println!("\n── ТОП-32 НАЙБІЛЬШ ВЖИВАНИХ ТОКЕНІВ ПАМ'ЯТІ ─────────────");
        for (i, t) in crystal.tokens.iter().take(32).enumerate() {
            print!("{:<16}", format!("{}. {}", i + 1, t));
            if (i + 1) % 4 == 0 {
                println!();
            }
        }
        println!("\n\n💡 Для аналізу конкретного слова запустіть:\n  poler-engine --triune-crystal-inspect \"<СЛОВО>\"");
    }
    0
}

fn run_crystal_build(cli: &Cli) -> i32 {
    use poler_engine::triune::Crystal;
    let corpus_path = cli.crystal_build.as_ref().unwrap();
    let corpus = match std::fs::read_to_string(corpus_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("crystal-build: чтение {corpus_path:?}: {e}");
            return 2;
        }
    };
    let crystal = match Crystal::build(
        &corpus,
        cli.crystal_vocab,
        poler_engine::triune::crystal::DEFAULT_DIMS,
        poler_engine::triune::crystal::DEFAULT_THETA_HI,
        poler_engine::triune::crystal::DEFAULT_THETA_LO,
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("crystal-build: {e}");
            return 2;
        }
    };
    let out = cli.crystal_out.clone().unwrap_or_else(|| {
        let mut p = corpus_path.clone();
        p.set_extension("t5c");
        p
    });
    let bytes = crystal.to_bytes();
    if let Err(e) = crystal.save(&out) {
        eprintln!("crystal-build: {e}");
        return 2;
    }
    println!("кристалл собран: {}", out.display());
    println!("  словарь: {} токенов (порог включения θ_hi={})",
        crystal.vocab(), crystal.theta_hi());
    println!("  корпус: {} слов, {} символов", crystal.corpus_words, crystal.corpus_chars);
    println!("  биграммы: {} ненулевых тритов (Trit5, 5 тритов/байт)", crystal.bigram_nonzeros);
    println!("  файл: {} байт, sha256 защищён", bytes.len());
    0
}

fn run_crystal_ingest_dir(cli: &Cli) -> i32 {
    use poler_engine::triune::{IngestConfig, StreamCrystalBuilder};
    let dir = cli.crystal_ingest_dir.as_ref().unwrap();
    let mut cfg = IngestConfig::default();
    cfg.vocab = cli.crystal_vocab;
    let mut builder = StreamCrystalBuilder::new(cfg);
    println!("потоковая ингестия текстов из: {}", dir.display());
    match builder.feed_dir(dir) {
        Ok(count) => {
            println!("  прочитано файлов: {count}");
        }
        Err(e) => {
            eprintln!("crystal-ingest-dir: ошибка чтения директории {dir:?}: {e}");
            return 2;
        }
    }
    let (crystal, stats) = match builder.finalize() {
        Ok(res) => res,
        Err(e) => {
            eprintln!("crystal-ingest-dir: ошибка финализации: {e}");
            return 2;
        }
    };
    let out = cli.crystal_out.clone().unwrap_or_else(|| {
        let mut p = dir.clone();
        p.set_extension("t5c");
        p
    });
    if let Err(e) = crystal.save(&out) {
        eprintln!("crystal-ingest-dir: {e}");
        return 2;
    }
    println!("кристалл успешно скомпилирован: {}", out.display());
    println!("  словарь: {} токенов", stats.crystal_vocab);
    println!("  корпус: {} слов, {} символов", stats.total_words, stats.total_chars);
    println!("  размер: {} байт (Trit5, 5 тритов/байт, No-Mul)", stats.crystal_bytes);
    println!("  sha256: {}", stats.sha256_hex);
    0
}

fn run_gen_archetype_asm(cli: &Cli) -> i32 {
    use poler_engine::universal_archetype_asm::ArchetypeAsmGenerator;
    let out_path = cli.gen_archetype_asm.as_ref().unwrap();
    let min_lines = cli.gen_archetype_lines;
    println!("генерація розгорнутого x86_64 асемблерного алгоритму матриці архетипів...");
    println!("  цільовий файл: {}", out_path.display());
    println!("  мінімальна кількість рядків: {min_lines}");

    let gen = ArchetypeAsmGenerator::new();
    let t0 = std::time::Instant::now();
    match gen.write_unrolled_asm_to_file(out_path, min_lines) {
        Ok(count) => {
            let elapsed = t0.elapsed();
            println!("  успішно згенеровано рядків: {count}");
            println!("  час генерації: {:.2?}", elapsed);
            0
        }
        Err(e) => {
            eprintln!("gen-archetype-asm: помилка запису файлу: {e}");
            2
        }
    }
}

/// `--learn-web <URL>` / `--learn-dir <DIR>`: автономный инжектор памяти —
/// потоковое обучение Кристалла Знаний без удержания корпуса в RAM.
///
/// Веб-режим повторяет конвейер `--crawl` (robots, politeness, дедупликация
/// в SQLite-индекс), но каждая викачанная страница параллельно льётся в
/// [`StreamCrystalBuilder`] через декоратор [`IngestingFetcher`].
fn run_learn(cli: &Cli) -> i32 {
    use poler_engine::triune::{IngestConfig, IngestingFetcher, StreamCrystalBuilder};
    let t0 = std::time::Instant::now();

    // Словарь: 384 (дефолт флага) для обучения мал — поднимаем до 4096.
    let mut vocab = cli.crystal_vocab;
    if vocab == 384 {
        vocab = 4096;
        eprintln!(
            "learn: --crystal-vocab не задан, поднят 384 → 4096 \
             (для больших дампов укажите до 65536)"
        );
    }
    let vocab = vocab.clamp(32, 65_536);
    if vocab > 16_384 {
        let dense_mb = vocab as f64 * ((vocab as f64 + 4.0) / 5.0) / 1e6;
        eprintln!(
            "learn: ВНИМАНИЕ: плотная биграммная матрица {vocab}×{} ≈ {dense_mb:.0} МБ",
            (vocab + 4) / 5
        );
    }
    let cfg = IngestConfig {
        vocab,
        dims: poler_engine::triune::crystal::DEFAULT_DIMS,
        theta_hi: poler_engine::triune::crystal::DEFAULT_THETA_HI,
        theta_lo: poler_engine::triune::crystal::DEFAULT_THETA_LO,
        chunk_bytes: cli.learn_chunk.max(1024),
        word_cap: cli.learn_word_cap.max(1024),
        bigram_cap: cli.learn_bigram_cap.max(1024),
    };
    let mut builder = StreamCrystalBuilder::new(cfg);

    // ── Источник 1: локальная папка ──
    if let Some(dir) = &cli.learn_dir {
        eprintln!("learn-dir: потоковое чтение {}", dir.display());
        match builder.feed_dir(dir) {
            Ok(n) => eprintln!("learn-dir: прочитано файлов: {n}"),
            Err(e) => {
                eprintln!("learn-dir: {e}");
                return 2;
            }
        }
    }

    // ── Источник 2: веб-краул (страницы → кристалл на лету) ──
    if let Some(seed) = &cli.learn_web {
        if !seed.starts_with("http://") && !seed.starts_with("https://") {
            eprintln!("learn-web: ожидается URL (http(s)://...), получено: {seed}");
            return 2;
        }
        let db = cli.web_db.clone().unwrap_or_else(poler_engine::web::default_db_path);
        let mut ix = match poler_engine::web::WebIndex::open(&db) {
            Ok(ix) => ix,
            Err(e) => {
                eprintln!("learn-web: веб-индекс {db:?}: {e}");
                return 2;
            }
        };
        let fetcher = match poler_engine::web::cdp_fetcher_with_timeout(
            cli.cdp_port,
            cli.web_wait_ms,
            cli.crawl_page_timeout_ms,
        ) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("learn-web: Chromium CDP (порт {}): {e}", cli.cdp_port);
                eprintln!("  автозапуск не удался: установите POLER_CHROME_BIN или запустите вручную:");
                eprintln!("  chrome --headless --remote-debugging-port={} --no-sandbox", cli.cdp_port);
                return 2;
            }
        };
        let crawl_cfg = poler_engine::web::CrawlConfig {
            max_pages: cli.crawl_max.max(1),
            max_depth: cli.crawl_depth,
            delay_ms: cli.crawl_delay_ms,
            cross_site: cli.cross_site,
            wait_ms: cli.web_wait_ms,
            page_timeout_ms: cli.crawl_page_timeout_ms,
            respect_robots: true,
        };
        eprintln!(
            "learn-web: seed {seed}, глубина ≤ {}, до {} страниц, база {db:?}",
            crawl_cfg.max_depth, crawl_cfg.max_pages
        );
        let mut ing = IngestingFetcher::new(Box::new(fetcher), &mut builder);
        match poler_engine::web::crawl::crawl(&mut ix, &mut ing, seed, &crawl_cfg, cli.verbose) {
            Ok(s) => {
                eprintln!(
                    "learn-web: {} загружено, {} проиндексировано, {} без изменений, \
                     {} дубликатов, {} robots-запретов, {} ошибок",
                    s.fetched, s.indexed, s.unchanged, s.duplicates, s.skipped_robots, s.errors
                );
                eprintln!("learn-web: {} страниц скормлено в кристалл", ing.pages);
            }
            Err(e) => {
                eprintln!("learn-web: {e}");
                return 2;
            }
        }
    }

    // ── Сборка кристалла ──
    let (crystal, stats) = match builder.finalize() {
        Ok(res) => res,
        Err(e) => {
            eprintln!("learn: {e}");
            return 2;
        }
    };
    let out = cli
        .crystal_out
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("memory.t5c"));
    if let Err(e) = crystal.save(&out) {
        eprintln!("learn: {e}");
        return 2;
    }
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   АВТОНОМНЫЙ ИНТЕРНЕТ-ИНЖЕКТОР ПАМЯТИ: кристалл готов       ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!("артефакт:  {}", out.display());
    println!("словарь:   {} токенов (Trit5, 5 тритов/байт)", stats.crystal_vocab);
    println!("корпус:    {} слов, {} символов, {} источников", stats.total_words, stats.total_chars, stats.total_sources);
    println!("топология: {} ненулевых биграмм, sha256 {}", stats.bigram_nonzeros, &stats.sha256_hex[..16.min(64)]);
    println!(
        "RAM:       пик текстового буфера {} Б (лимит {} + 3), эвакуаций: {} слов / {} биграмм",
        stats.peak_text_buffer, cli.learn_chunk, stats.words_evicted, stats.bigrams_evicted
    );
    if stats.files_skipped_binary > 0 {
        println!("фильтр:    {} бинарных файлов пропущено", stats.files_skipped_binary);
    }
    println!("время:     {} мс", t0.elapsed().as_millis());
    println!();
    println!("говорить выученными словами:");
    println!("  poler-engine --triune-speak \"живой мозг\" --triune-crystal {}", out.display());
    0
}

// ─────────── v0.39.0: Zero-Disk Streaming Ingestion Pipeline ───────────

/// Общая конфигурация StreamWriter из CLI-флагов.
fn stream_write_cfg(cli: &Cli) -> poler_engine::archive::StreamWriteConfig {
    poler_engine::archive::StreamWriteConfig {
        dedup: !cli.no_dedup,
        tier: match cli.stream_tier {
            StreamTierArg::Fast => poler_engine::archive::CompressTier::Fast,
            StreamTierArg::Deep => poler_engine::archive::CompressTier::Deep,
            StreamTierArg::Auto => poler_engine::archive::CompressTier::Auto,
        },
        ..Default::default()
    }
}

/// Имя файла из URL (для одиночной записи не-tar потока).
fn url_hint(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("stream")
        .split('?')
        .next()
        .unwrap_or("stream")
        .to_string()
}

/// --stream-download / --stream-file / --stream-bench / --pack → .poler.
fn run_stream_ingest(cli: &Cli) -> i32 {
    use poler_engine::archive::{write_stream, SyntheticStream};
    let cfg = stream_write_cfg(cli);
    let out = match cli.output_archive.clone() {
        Some(p) => p,
        None => {
            eprintln!("stream: укажите --output-archive <out.poler>");
            return 2;
        }
    };
    // v0.91.0 CORPUS-TRIT: корпусной пакер — свой профиль плотности,
    // батч-сжатие на rayon, классификация текстов и честный отчёт.
    if let Some(dir) = &cli.corpus_pack {
        return run_corpus_pack(cli, dir, &out);
    }
    // v0.92.0 STREAM-CORPUS: манифест юнитов (URL/пути) → один .poler —
    // нативный много-URL стриминг с чекпоинтами и резюмом.
    if let Some(mf) = &cli.stream_manifest {
        return run_stream_manifest(cli, mf, &out);
    }
    let stats = if let Some(url) = &cli.stream_download {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            eprintln!("stream-download: ожидается http(s) URL: {url}");
            return 2;
        }
        eprintln!("stream-download: {url} → {}", out.display());
        let agent = ureq::AgentBuilder::new()
            .user_agent("poler-engine/0.65 (zero-disk streaming + on-the-fly unpack)")
            .build();
        let resp = match agent.get(url).call() {
            Ok(r) => r,
            Err(ureq::Error::Status(code, _)) => {
                eprintln!("stream-download: HTTP {code}");
                return 2;
            }
            Err(e) => {
                eprintln!("stream-download: {e}");
                return 2;
            }
        };
        // v0.65.0 ансамбль: тот же IngestPipeline, что и у --stream-file —
        // сниффинг магии стрима, gzip/zstd-декодер НА ЛЕТУ, TarObserver
        // разводит пофайловую таблицу. Сырой tar.gz на диск НЕ пишется.
        match poler_engine::archive::ingest_reader(
            resp.into_reader(),
            &out,
            cfg,
            poler_engine::archive::IngestMode::Auto,
            &url_hint(url),
        ) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("stream-download: {e}");
                return 2;
            }
        }
    } else if let Some(path) = &cli.stream_file {
        if path == "-" {
            eprintln!("stream-file: stdin → {}", out.display());
            // v0.65.0: stdin — тот же инжест-конвейер (tar.gz из пайпа
            // раскладывается в пофайловую таблицу на лету)
            match poler_engine::archive::ingest_reader(
                std::io::stdin(),
                &out,
                cfg,
                poler_engine::archive::IngestMode::Auto,
                "stdin",
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("stream-file: {e}");
                    return 2;
                }
            }
        } else {
            let p = std::path::Path::new(path);
            eprintln!("stream-file (ingest): {} → {}", p.display(), out.display());
            match poler_engine::archive::ingest_file(p, &out, cfg, poler_engine::archive::IngestMode::Auto) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("stream-file: {path}: {e}");
                    return 2;
                }
            }
        }
    } else if let Some(dir) = &cli.pack {
        // v0.65.0 ПОСТОЯННЫЙ АРХИВАТОР: каталог → .poler с пофайловой таблицей.
        // WYSIWYG по умолчанию (всё, вкл. скрытые); --pack-gitignore — режим
        // репо-чекаута (фильтр .gitignore + пропуск скрытых, как grep-слой).
        eprintln!("pack: {} → {}", dir.display(), out.display());
        let (hidden, ignore) = if cli.pack_gitignore {
            (false, true)
        } else {
            (true, false)
        };
        match poler_engine::archive::pack_dir(dir, &out, cfg, hidden, ignore) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("pack: {e}");
                return 2;
            }
        }
    } else {
        let total = cli.stream_bench.unwrap_or(1024 * 1024 * 1024);
        eprintln!("stream-bench: {} синтетики → {}", poler_engine::archive::fmt_bytes(total), out.display());
        match write_stream(SyntheticStream::new(total, 0x504F_4C45), &out, cfg, "synthetic.bin") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("stream-bench: {e}");
                return 2;
            }
        }
    };
    println!("{}", serde_json::to_string_pretty(&stats).unwrap_or_default());
    if stats.peak_rss_kb > 48 * 1024 {
        eprintln!(
            "stream: ВНИМАНИЕ: пик RSS {} КиБ превысил бюджет 48 МиБ",
            stats.peak_rss_kb
        );
    }
    0
}

/// v0.91.0 CORPUS-TRIT: --corpus-pack — сотни гигабайт текстов
/// (код + литература) → один .poler. Профиль плотности CDC 128К/512К/1М
/// + Deep zstd-19 + BLAKE3-дедуп + батч-сжатие на rayon.
fn run_corpus_pack(cli: &Cli, dir: &std::path::Path, out: &std::path::Path) -> i32 {
    use poler_engine::archive::fmt_bytes;
    use poler_engine::archive::{corpus_config, corpus_pack};
    let jobs = if cli.pack_jobs == 0 {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    } else {
        cli.pack_jobs
    };
    let mut cfg = corpus_config(jobs);
    if cli.no_dedup {
        cfg.dedup = false;
    }
    let (hidden, ignore) = if cli.pack_gitignore {
        (false, true)
    } else {
        (true, false)
    };
    eprintln!(
        "corpus-pack: {} → {}\n  профиль: CDC 128К/512К/1М · Deep zstd-{} · дедуп {} · батч-сжатие ×{}",
        dir.display(),
        out.display(),
        cfg.deep_level,
        if cfg.dedup { "вкл" } else { "выкл" },
        jobs
    );
    let t0 = std::time::Instant::now();
    match corpus_pack(dir, out, cfg, hidden, ignore) {
        Ok((stats, classes)) => {
            println!("{}", serde_json::to_string_pretty(&stats).unwrap_or_default());
            println!("{}", serde_json::to_string_pretty(&classes).unwrap_or_default());
            let total_files =
                classes.code_files + classes.prose_files + classes.data_files + classes.other_files;
            eprintln!(
                "corpus-pack: {} файлов (код {} · литература {} · данные {} · прочее {})",
                total_files, classes.code_files, classes.prose_files, classes.data_files,
                classes.other_files
            );
            eprintln!(
                "corpus-pack: {} → {} (сжатие ×{:.2}), дедуп сэкономил {}, {} мс, {} МиБ/с",
                fmt_bytes(stats.total_raw),
                fmt_bytes(stats.total_stored),
                if stats.total_raw > 0 {
                    stats.total_raw as f64 / stats.total_stored as f64
                } else {
                    0.0
                },
                fmt_bytes(stats.dedup_bytes),
                t0.elapsed().as_millis(),
                stats.throughput_mbs
            );
            if stats.peak_rss_kb > 512 * 1024 {
                eprintln!(
                    "corpus-pack: пик RSS {} КиБ (батч-сжатие ×{})",
                    stats.peak_rss_kb, jobs
                );
            }
            0
        }
        Err(e) => {
            eprintln!("corpus-pack: {e}");
            2
        }
    }
}

/// v0.92.0 STREAM-CORPUS: --stream-manifest — список юнитов (URL/пути)
/// → ОДИН .poler. Нативный конвейер: параллельный фетч слот-машиной
/// (байты = порядок манифеста, --stream-jobs влияет только на скорость),
/// распаковка tar.gz/tar.zst/gz/zst НА ЛЕТУ, zip пофайлово, BLAKE3-дедуп,
/// Deep zstd-19, чекпоинты (валидный архив + state-sidecar каждые
/// 2000 юнитов / 210 с), резюм по .units.done, доклейка НА МЕСТЕ
/// (пик диска = архив, не 2×). Падение любого масштаба откатывается
/// к последнему чекпоинту ремонтом по sidecar.
fn run_stream_manifest(cli: &Cli, manifest: &std::path::Path, out: &std::path::Path) -> i32 {
    use poler_engine::archive::fmt_bytes;
    use poler_engine::archive::manifest_pack::{manifest_pack, ManifestOptions};
    use poler_engine::archive::corpus_config;
    let jobs = if cli.pack_jobs == 0 {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    } else {
        cli.pack_jobs
    };
    let mut cfg = corpus_config(jobs);
    if cli.no_dedup {
        cfg.dedup = false;
    }
    let opts = ManifestOptions {
        fetch_jobs: cli.stream_jobs.max(1),
        unit_buf_max: cli.stream_unit_max as usize,
        ..Default::default()
    };
    let resumed = out.exists();
    eprintln!(
        "stream-manifest: {} → {}{}\n  профиль: CDC 128К/512К/1М · Deep zstd-{} · дедуп {} · сжатие ×{} · фетч ×{} · чекпоинт каждые {} юнитов / {} с",
        manifest.display(),
        out.display(),
        if resumed { " (доклейка/резюм)" } else { "" },
        cfg.deep_level,
        if cfg.dedup { "вкл" } else { "выкл" },
        jobs,
        opts.fetch_jobs,
        opts.checkpoint_units,
        opts.checkpoint_secs,
    );
    let t0 = std::time::Instant::now();
    match manifest_pack(manifest, out, cfg, opts) {
        Ok(stats) => {
            println!("{}", serde_json::to_string_pretty(&stats).unwrap_or_default());
            eprintln!(
                "stream-manifest: юниты {}/{} (докатано {}, пропущено готовых {}, провалов {}), чекпоинтов {}",
                stats.units_packed + stats.units_skipped_done,
                stats.units_total,
                stats.units_packed,
                stats.units_skipped_done,
                stats.units_failed,
                stats.checkpoints,
            );
            eprintln!(
                "stream-manifest: {} raw → {} stored (×{:.2}), дедуп {}, {} файлов, {} мс",
                fmt_bytes(stats.total_raw),
                fmt_bytes(stats.total_stored),
                if stats.total_raw > 0 {
                    stats.total_raw as f64 / stats.total_stored as f64
                } else {
                    0.0
                },
                fmt_bytes(stats.dedup_bytes),
                stats.files,
                t0.elapsed().as_millis(),
            );
            0
        }
        Err(e) => {
            eprintln!("stream-manifest: {e}");
            2
        }
    }
}

/// v0.91.0: --poler-stats — манифест .poler-корпуса БЕЗ декомпрессии:
/// классы текстов по именам записей, гистограмма расширений, дедуп,
/// топ-файлы. Только метаданные контейнера (mmap + таблица файлов).
fn run_poler_stats(cli: &Cli) -> i32 {
    use poler_engine::archive::fmt_bytes;
    use poler_engine::archive::ingest::{CorpusClass, CorpusClassStats};
    use poler_engine::archive::reader::PolerReader;
    use std::collections::HashMap;
    let path = cli.poler_stats.as_ref().unwrap();
    let reader = match PolerReader::open(path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("poler-stats: {e}");
            return 2;
        }
    };
    let info = reader.info().clone();
    let mut classes = CorpusClassStats::default();
    let mut ext_hist: HashMap<String, (u64, u64)> = HashMap::new();
    let mut top_files: Vec<(String, u64)> = Vec::new();
    for f in reader.files() {
        top_files.push((f.name.clone(), f.raw_len));
        let ext = std::path::Path::new(&f.name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        classes.bump(CorpusClass::of_ext(&ext), f.raw_len);
        let key = if ext.is_empty() { "(none)".to_string() } else { ext };
        let slot = ext_hist.entry(key).or_insert((0, 0));
        slot.0 += 1;
        slot.1 += f.raw_len;
    }
    let mut top_ext: Vec<(String, u64, u64)> =
        ext_hist.into_iter().map(|(k, (f, b))| (k, f, b)).collect();
    top_ext.sort_by(|a, b| b.1.cmp(&a.1));
    top_ext.truncate(16);
    classes.top_ext = top_ext;
    top_files.sort_by(|a, b| b.1.cmp(&a.1));
    top_files.truncate(16);

    let dedup_saved = info.logical_chunks.saturating_sub(info.physical_chunks);
    let manifest = serde_json::json!({
        "archive": path.display().to_string(),
        "info": info,
        "classes": classes,
        "top_files": top_files,
        "dedup_chunks_saved": dedup_saved,
    });
    println!("{}", serde_json::to_string_pretty(&manifest).unwrap_or_default());
    eprintln!(
        "poler-stats: {} записей · {} логических / {} физических чанков (дедуп −{}) · {} → {} (×{:.2})",
        reader.files().len(),
        info.logical_chunks,
        info.physical_chunks,
        dedup_saved,
        fmt_bytes(info.total_raw),
        fmt_bytes(info.total_stored),
        if info.total_raw > 0 {
            info.total_raw as f64 / info.total_stored as f64
        } else {
            0.0
        }
    );
    0
}

/// --browser-crawl: HTTP-краул без Chromium → ε-фильтр → кристалл/.poler.
fn run_browser_crawl(cli: &Cli) -> i32 {
    use poler_engine::browser::StreamingFetcher;
    use poler_engine::triune::{IngestConfig, StreamCrystalBuilder};
    use poler_engine::web::crawl::CrawlConfig;

    let seed = cli.browser_crawl.as_ref().unwrap().clone();
    if !seed.starts_with("http://") && !seed.starts_with("https://") {
        eprintln!("browser-crawl: ожидается http(s) URL: {seed}");
        return 2;
    }
    let crystal_path = cli.ingest_to_crystal.clone();
    let archive_path = cli.output_archive.clone();
    if crystal_path.is_none() && archive_path.is_none() {
        eprintln!("browser-crawl: не задан ни --ingest-to-crystal, ни --output-archive — нечего делать");
        return 2;
    }

    let crawl_cfg = CrawlConfig {
        max_pages: cli.crawl_max.max(1),
        max_depth: cli.crawl_depth,
        delay_ms: cli.crawl_delay_ms,
        cross_site: cli.cross_site,
        wait_ms: 0, // нет JS-рендера — пауза не нужна
        page_timeout_ms: cli.crawl_page_timeout_ms,
        respect_robots: true,
    };
    eprintln!(
        "browser-crawl: seed {seed}, глубина ≤ {}, до {} страниц, ε ≥ {}",
        crawl_cfg.max_depth, crawl_cfg.max_pages, cli.min_epsilon
    );

    // индекс в памяти: обход без дисковых side-эффектов (zero-disk)
    let mut ix = match poler_engine::web::WebIndex::open_memory() {
        Ok(ix) => ix,
        Err(e) => {
            eprintln!("browser-crawl: индекс в памяти: {e}");
            return 2;
        }
    };

    let t0 = std::time::Instant::now();
    let mut builder = StreamCrystalBuilder::new(IngestConfig {
        vocab: {
            let v = if cli.crystal_vocab == 384 { 4096 } else { cli.crystal_vocab };
            v.clamp(32, 65_536)
        },
        dims: poler_engine::triune::crystal::DEFAULT_DIMS,
        theta_hi: poler_engine::triune::crystal::DEFAULT_THETA_HI,
        theta_lo: poler_engine::triune::crystal::DEFAULT_THETA_LO,
        chunk_bytes: cli.learn_chunk.max(1024),
        word_cap: cli.learn_word_cap.max(1024),
        bigram_cap: cli.learn_bigram_cap.max(1024),
    });
    let mut archive_writer = match &archive_path {
        Some(p) => match poler_engine::archive::StreamWriter::open(p, stream_write_cfg(cli)) {
            Ok(w) => Some(w),
            Err(e) => {
                eprintln!("browser-crawl: {p:?}: {e}");
                return 2;
            }
        },
        None => None,
    };

    // одна обёртка — два стока: каждая страница течёт и в кристалл,
    // и в .poler (ε-очищенная семантика; сырой HTML не храним)
    struct CrawlSink<'a> {
        inner: StreamingFetcher,
        builder: &'a mut StreamCrystalBuilder,
        writer: Option<&'a mut poler_engine::archive::StreamWriter>,
        pages: usize,
    }
    impl poler_engine::web::crawl::PageFetcher for CrawlSink<'_> {
        fn fetch(
            &mut self,
            url: &str,
        ) -> Result<poler_engine::web::crawl::FetchedPage, String> {
            let page = self.inner.fetch(url)?;
            let mut text = String::with_capacity(page.text.len() + page.title.len() + 2);
            if !page.title.is_empty() {
                text.push_str(&page.title);
                text.push('\n');
            }
            text.push_str(&page.text);
            if let Some(w) = self.writer.as_mut() {
                if let Err(e) = w.push_bytes(text.as_bytes()) {
                    return Err(format!("архив: {e}"));
                }
            }
            self.builder.push_str(&text);
            self.pages += 1;
            Ok(page)
        }
        fn fetch_raw(&mut self, url: &str) -> Result<(u16, String), String> {
            self.inner.fetch_raw(url)
        }
    }

    let fetcher = StreamingFetcher::new(cli.crawl_page_timeout_ms, cli.min_epsilon);
    let mut sink = CrawlSink {
        inner: fetcher,
        builder: &mut builder,
        writer: archive_writer.as_mut(),
        pages: 0,
    };
    let stats =
        match poler_engine::web::crawl::crawl(&mut ix, &mut sink, &seed, &crawl_cfg, cli.verbose) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("browser-crawl: {e}");
                return 2;
            }
        };
    let pages = sink.pages;

    // финализация .poler ДО потребления билдера
    let mut archive_stats = None;
    if let Some(w) = archive_writer {
        match w.finish("crawl-pages.txt") {
            Ok(s) => archive_stats = Some(s),
            Err(e) => {
                eprintln!("browser-crawl: архив: {e}");
                return 2;
            }
        }
    }

    eprintln!(
        "browser-crawl: {} страниц, {} robots-запретов, {} ошибок, {} в кристалл, за {} мс",
        stats.fetched, stats.skipped_robots, stats.errors, pages,
        t0.elapsed().as_millis()
    );

    match crystal_path {
        Some(path) => {
            let (crystal, cstats) = match builder.finalize() {
                Ok(res) => res,
                Err(e) => {
                    eprintln!("browser-crawl: кристалл: {e}");
                    return 2;
                }
            };
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = crystal.save(&path) {
                eprintln!("browser-crawl: сохранение {}: {e}", path.display());
                return 2;
            }
            eprintln!(
                "browser-crawl: кристалл {} (словарь {}, {} слов корпуса)",
                path.display(), cstats.crystal_vocab, cstats.total_words
            );
        }
        None => {
            // архив без кристалла: билдер больше не нужен
            let _ = builder.finalize();
        }
    }
    if let Some(s) = archive_stats {
        println!("{}", serde_json::to_string_pretty(&s).unwrap_or_default());
    }
    0
}

/// RAM-дисциплина директивы (пик ≤ 48 МиБ): капы билдера выше этих
/// значений не сэкономить — карты слов/биграмм доминируют в RSS
/// (запас: vocab 4096 требует лишь ≥4096 живых слов, 131K капа
/// оставляет 32× запас на статистику Ципфа).
const DIRECTIVE_BIGRAM_CAP: usize = 131_072;
const DIRECTIVE_WORD_CAP: usize = 131_072;

/// --archive-to-crystal: .poler → потоковое обучение .t5c без распаковки.
fn run_archive_to_crystal(cli: &Cli) -> i32 {
    use poler_engine::archive::reader::PolerReader;
    use poler_engine::triune::{IngestConfig, StreamCrystalBuilder};
    let poler_path = cli.archive_to_crystal.as_ref().unwrap();
    let reader = match PolerReader::open(poler_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("archive-to-crystal: {e}");
            return 2;
        }
    };
    let out = cli.crystal.clone().unwrap_or_else(|| std::path::PathBuf::from("memory.t5c"));
    let t0 = std::time::Instant::now();
    let vocab = {
        let v = if cli.crystal_vocab == 384 { 4096 } else { cli.crystal_vocab };
        v.clamp(32, 65_536)
    };
    let (word_cap, bigram_cap) = if cli.learn_word_cap > DIRECTIVE_WORD_CAP
        || cli.learn_bigram_cap > DIRECTIVE_BIGRAM_CAP
    {
        eprintln!(
            "archive-to-crystal: RAM-дисциплина 48 МиБ: word_cap {} → {}, bigram_cap {} → {} \
             (явные --learn-word-cap/--learn-bigram-cap ниже порога не трогаются)",
            cli.learn_word_cap,
            cli.learn_word_cap.min(DIRECTIVE_WORD_CAP),
            cli.learn_bigram_cap,
            cli.learn_bigram_cap.min(DIRECTIVE_BIGRAM_CAP)
        );
        (
            cli.learn_word_cap.min(DIRECTIVE_WORD_CAP),
            cli.learn_bigram_cap.min(DIRECTIVE_BIGRAM_CAP),
        )
    } else {
        (cli.learn_word_cap, cli.learn_bigram_cap)
    };
    let mut builder = StreamCrystalBuilder::new(IngestConfig {
        vocab,
        dims: poler_engine::triune::crystal::DEFAULT_DIMS,
        theta_hi: poler_engine::triune::crystal::DEFAULT_THETA_HI,
        theta_lo: poler_engine::triune::crystal::DEFAULT_THETA_LO,
        chunk_bytes: cli.learn_chunk.max(1024),
        word_cap: word_cap.max(1024),
        bigram_cap: bigram_cap.max(1024),
    });
    let info = reader.info().clone();
    eprintln!(
        "archive-to-crystal: {} ({} сырых, {} записей, дедуп {}) → {}",
        poler_path.display(),
        poler_engine::archive::fmt_bytes(info.total_raw),
        info.files,
        info.logical_chunks - info.physical_chunks,
        out.display()
    );
    let fed = match builder.feed_poler(&reader) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("archive-to-crystal: {e}");
            return 2;
        }
    };
    drop(reader);
    // вернуть освобождённые арены аллокатора до финализации: пик RSS
    // не должен накладывать карты инжеста на матрицу кристалла
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    unsafe {
        libc::malloc_trim(0);
    }
    let (crystal, stats) = match builder.finalize() {
        Ok(res) => res,
        Err(e) => {
            eprintln!("archive-to-crystal: {e}");
            return 2;
        }
    };
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = crystal.save(&out) {
        eprintln!("archive-to-crystal: {e}", );
        return 2;
    }
    let throughput = if t0.elapsed().as_millis() > 0 {
        info.total_raw as f64 / 1024.0 / 1024.0 / (t0.elapsed().as_millis() as f64 / 1000.0)
    } else {
        0.0
    };
    println!(
        "{}",
        serde_json::json!({
            "archive": poler_path.display().to_string(),
            "crystal": out.display().to_string(),
            "entries_fed": fed,
            "entries_total": info.files,
            "total_words": stats.total_words,
            "crystal_vocab": stats.crystal_vocab,
            "skipped_binary": stats.files_skipped_binary,
            "crystal_bytes": stats.crystal_bytes,
            "sha256": stats.sha256_hex,
            "elapsed_ms": t0.elapsed().as_millis(),
            "throughput_mbs": throughput,
            "peak_rss_kb": poler_engine::archive::peak_rss_kb(),
        })
    );
    0
}

/// `--poler-cat <POLER> --file <NAME>`: байты записи в stdout (для
/// пайпов `| grep`, `| sed`, `| wc -c` — движок вместо bash-распаковки).
fn run_poler_cat(cli: &Cli, arch: &std::path::Path) -> i32 {
    use poler_engine::archive::patcher::cat_file;
    let Some(name) = &cli.poler_file_name else {
        eprintln!("poler-cat: требуется --file <NAME> (имя записи; см. --poler-list)");
        return 2;
    };
    let mut buf = Vec::new();
    match cat_file(arch, name, &mut buf) {
        Ok(n) => {
            use std::io::Write;
            let mut out = std::io::stdout().lock();
            if out.write_all(&buf).is_err() {
                eprintln!("poler-cat: stdout закрыт (пайп?)");
                return 2;
            }
            let _ = out.flush();
            if n != buf.len() as u64 {
                eprintln!("poler-cat: прочитано {} из {}", buf.len(), n);
                return 2;
            }
            0
        }
        Err(e) => {
            eprintln!("poler-cat: {e}");
            2
        }
    }
}

/// `--poler-box <POLER> --box-entry <NAME>`: нативна «заміна Docker без ОС».
/// Див. poler_engine::boxenv — архітектуру трипроцесного ланцюжка.
fn run_poler_box(cli: &Cli, arch: &std::path::Path) -> i32 {
    use poler_engine::boxenv::{run_box, BoxSpec};
    let Some(entry) = &cli.box_entry else {
        eprintln!("poler-box: потрібен --box-entry <ім'я запису> (див. --poler-list)");
        return 2;
    };
    let mut maps: Vec<(String, String)> = Vec::new();
    for m in &cli.box_map {
        match m.split_once(':') {
            // порожній префікс (":/ ") = увесь архів; інакше PREFIX:DIR
            Some((p, d)) if !d.is_empty() => {
                maps.push((p.to_string(), d.to_string()));
            }
            _ => {
                eprintln!("poler-box: --box-map очікує PREFIX:DIR, отримано '{m}'");
                return 2;
            }
        }
    }
    if maps.is_empty() {
        maps.push(("rootfs/".to_string(), "/".to_string()));
    }
    let spec = BoxSpec {
        archive: arch.to_path_buf(),
        entry: entry.clone(),
        args: cli.box_arg.clone(),
        maps,
        rss_mb: cli.box_rss_mb,
        cpu_s: cli.box_cpu_s,
        tmpfs_mb: cli.box_tmpfs_mb,
        isolate: !cli.box_no_isolate,
    };
    match run_box(&spec) {
        Ok(report) => {
            let json = serde_json::to_string_pretty(&report).unwrap_or_default();
            eprintln!("--- poler-box report ---
{json}");
            report.exit_code.unwrap_or(137)
        }
        Err(e) => {
            eprintln!("poler-box: {e}");
            2
        }
    }
}

/// `--poler-patch <POLER> --manifest <JSON>`: in-place CoW-патч.
/// Манифест: {"replace":[{"name":..,"data":..|"file":..}],
/// "add":[..], "delete":[{"name":..}]}.
fn run_poler_patch(cli: &Cli, arch: &std::path::Path) -> i32 {
    use poler_engine::archive::patcher::{patch_archive, PatchOp, PatchOptions};
    let Some(manifest) = &cli.poler_manifest else {
        eprintln!("poler-patch: требуется --manifest <JSON>");
        return 2;
    };
    let text = match std::fs::read_to_string(manifest) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("poler-patch: {} {e}", manifest.display());
            return 2;
        }
    };
    let root: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("poler-patch: JSON: {e}");
            return 2;
        }
    };
    let mut ops: Vec<PatchOp> = Vec::new();
    let mut parse = |section: &str,
                     make: &dyn Fn(String, Vec<u8>) -> PatchOp|
     -> Result<(), String> {
        let Some(arr) = root.get(section) else {
            return Ok(());
        };
        let serde_json::Value::Array(arr) = arr else {
            return Err(format!("манифест: {section} не массив"));
        };
        for (i, item) in arr.iter().enumerate() {
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("{section}[{i}]: нет поля name"))?
                .to_string();
            let data = if let Some(f) = item.get("file").and_then(|v| v.as_str()) {
                std::fs::read(f).map_err(|e| format!("{section}[{i}] file {f}: {e}"))?
            } else if let Some(d) = item.get("data").and_then(|v| v.as_str()) {
                d.as_bytes().to_vec()
            } else {
                return Err(format!("{section}[{i}]: нужно поле data или file"));
            };
            ops.push(make(name, data));
        }
        Ok(())
    };
    if let Err(e) = parse("replace", &PatchOp::replace) {
        eprintln!("poler-patch: {e}");
        return 2;
    }
    if let Err(e) = parse("add", &PatchOp::add) {
        eprintln!("poler-patch: {e}");
        return 2;
    }
    // delete: без данных
    if let Some(arr) = root.get("delete").and_then(|v| v.as_array()) {
        for (i, item) in arr.iter().enumerate() {
            let Some(name) = item.get("name").and_then(|v| v.as_str()) else {
                eprintln!("poler-patch: delete[{i}]: нет поля name");
                return 2;
            };
            ops.push(PatchOp::delete(name));
        }
    }
    let opts = PatchOptions { force_bak: cli.force_bak, ..Default::default() };
    match patch_archive(arch, &ops, &opts) {
        Ok(rep) => {
            println!("{}", serde_json::to_string_pretty(&rep).unwrap_or_default());
            0
        }
        Err(e) => {
            eprintln!("poler-patch: {e}");
            2
        }
    }
}

/// `--poler-remux <POLER> --output-archive <OUT>`: tar.gz-блоб →
/// файловая таблица (zero-disk стриминг через gzip-декодер).
fn run_poler_remux(cli: &Cli, arch: &std::path::Path) -> i32 {
    use poler_engine::archive::patcher::remux_archive;
    use poler_engine::archive::stream_writer::StreamWriteConfig;
    let Some(out) = &cli.output_archive else {
        eprintln!("poler-remux: требуется --output-archive <POLER>");
        return 2;
    };
    let cfg = StreamWriteConfig {
        tier: match cli.stream_tier {
            StreamTierArg::Fast => poler_engine::archive::CompressTier::Fast,
            StreamTierArg::Deep => poler_engine::archive::CompressTier::Deep,
            StreamTierArg::Auto => poler_engine::archive::CompressTier::Auto,
        },
        dedup: !cli.no_dedup,
        ..Default::default()
    };
    match remux_archive(arch, out, cfg) {
        Ok(stats) => {
            println!("{}", serde_json::to_string_pretty(&stats).unwrap_or_default());
            0
        }
        Err(e) => {
            eprintln!("poler-remux: {e}");
            2
        }
    }
}

fn run_poler_ops(cli: &Cli) -> i32 {
    use poler_engine::archive::reader::PolerReader;
    if let Some(path) = &cli.poler_list {
        let r = match PolerReader::open(path) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("poler-list: {e}");
                return 2;
            }
        };
        println!("{}", serde_json::to_string_pretty(&r.list_json()).unwrap_or_default());
    }
    if let Some(path) = &cli.poler_verify {
        let r = match PolerReader::open(path) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("poler-verify: {e}");
                return 2;
            }
        };
        let report = match r.verify() {
            Ok(rep) => rep,
            Err(e) => {
                eprintln!("poler-verify: {e}");
                return 2;
            }
        };
        println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
        if !report.all_ok {
            return 1;
        }
    }
    if let Some(path) = &cli.poler_extract {
        let r = match PolerReader::open(path) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("poler-extract: {e}");
                return 2;
            }
        };
        let dir = &cli.extract_dir;
        let report = match r.extract_all(dir) {
            Ok(rep) => rep,
            Err(e) => {
                eprintln!("poler-extract: {e}");
                return 2;
            }
        };
        println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
        if !report.files_bad.is_empty() {
            return 1;
        }
    }
    0
}

fn run_stream_quant(cli: &Cli) -> i32 {
    use poler_engine::triune::{stream_quantize, verify_t5q, StreamQuantConfig};
    let input = cli.stream_quant.as_ref().unwrap();
    let out_path = cli.stream_quant_out.clone().unwrap_or_else(|| {
        let mut p = input.clone();
        p.set_extension("t5q");
        p
    });
    let cfg = StreamQuantConfig {
        block: cli.stream_quant_block,
        theta: 0.05,
        keep: cli.stream_quant_keep as f32,
        f16: cli.stream_quant_f16,
    };
    let t0 = std::time::Instant::now();
    let stats = {
        let reader: Box<dyn std::io::Read> = if input.as_os_str() == "-" {
            Box::new(std::io::stdin().lock())
        } else {
            let f = match std::fs::File::open(input) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("stream-quant: открытие {input:?}: {e}");
                    return 2;
                }
            };
            Box::new(f)
        };
        let mut out = match std::fs::File::create(&out_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("stream-quant: создание {out_path:?}: {e}");
                return 2;
            }
        };
        match stream_quantize(reader, &mut out, &cfg) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("stream-quant: {e}");
                return 2;
            }
        }
    };
    // Верификация записанного (честность формата).
    let written = std::fs::read(&out_path).unwrap_or_default();
    let ok = verify_t5q(&written).is_ok();
    println!("потоковое квантование: {} → {}", input.display(), out_path.display());
    println!("  значений: {} ({} блоков по {}), вход {} байт",
        stats.values, stats.blocks, cfg.block, stats.in_bytes);
    println!("  выход: {} байт = {:.3} бита/вес, сжатие {:.1}×",
        stats.out_bytes, stats.bits_per_value(),
        stats.in_bytes as f64 / stats.out_bytes.max(1) as f64);
    println!("  вакуум: {:.1}% нулевых тритов (keep = {})",
        stats.vacuum_frac() * 100.0, cfg.keep);
    println!("  пик RAM: {} байт (сырец на диск НЕ писался)", stats.peak_buffer);
    println!("  sha256: {} | время {} мс",
        if ok { "сходится" } else { "ОШИБКА" }, t0.elapsed().as_millis());
    if !ok {
        return 3;
    }
    // Математика масштабирования (честная): 70B FP16 = 140 ГБ →
    // 70e9 весов × bpv бит / 8 = N ГБ (1 ГБ = 1e9 Б).
    let bpv = stats.bits_per_value();
    let gb_70b = 70.0e9 * bpv / 8.0 / 1e9;
    println!("  масштаб 70B: 140 ГБ FP16 → {:.1} ГБ в этом режиме", gb_70b);
    0
}

fn run_literary(cli: &Cli) -> i32 {
    use poler_engine::literary as lit;

    // Гиперпараметры из флагов.
    let params = lit::LiteraryParams {
        eta: cli.literary_eta as f32,
        eta_r: cli.literary_eta_r as f32,
        rho: cli.literary_rho as f32,
        kappa: cli.literary_kappa as f32,
        gamma: cli.literary_gamma as f32,
        lambda_rl: cli.literary_lambda as f32,
        dims: cli.literary_dims,
        max_steps: cli.literary_steps,
        no_mul: cli.literary_no_mul,
        ..Default::default()
    };

    // Мушиная калибровка: коннектом + семена → каста.
    let cast = match &cli.literary_csr {
        Some(csr) => {
            let t0 = std::time::Instant::now();
            let con = match poler_engine::graph::connectome::Connectome::load(csr) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("poler-literary: {e}");
                    return 1;
                }
            };
            let nodes = cli
                .literary_nodes
                .as_ref()
                .map(|p| match poler_engine::graph::connectome::ConnectomeNodes::load(p) {
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("poler-literary: {e}");
                        std::process::exit(1);
                    }
                });
            // Резолв семян: индекс либо root_id.
            let mut seeds = Vec::new();
            for spec in cli.literary_seeds.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                let resolved = spec
                    .parse::<usize>()
                    .ok()
                    .filter(|&i| i < con.n_nodes())
                    .or_else(|| {
                        nodes
                            .as_ref()
                            .and_then(|n| spec.parse::<u64>().ok().and_then(|r| n.idx_of(r)))
                    });
                match resolved {
                    Some(i) => seeds.push(i),
                    None => {
                        eprintln!("poler-literary: нейрон-семя «{spec}» не найден");
                        return 1;
                    }
                }
            }
            if seeds.is_empty() {
                eprintln!("poler-literary: семена пусты (--literary-seeds)");
                return 1;
            }
            match lit::flybridge::build_cast(&con, &seeds, cli.literary_khop, cli.literary_max_cast)
            {
                Ok(c) => {
                    eprintln!(
                        "poler-literary: каста мухи — {} нейронов (BFS достиг {}), {} внутренних \
                         рёбер, масса {}, за {} мс",
                        c.members.len(),
                        c.stats.reached,
                        c.stats.inner_edges,
                        c.stats.inner_abs_mass as u64,
                        t0.elapsed().as_millis()
                    );
                    Some(c)
                }
                Err(e) => {
                    eprintln!("poler-literary: {e}");
                    return 1;
                }
            }
        }
        None => None,
    };

    let mode = if cli.literary_field.is_some() {
        "field"
    } else {
        "generate"
    };
    let text = cli
        .literary_field
        .clone()
        .or_else(|| cli.literary_generate.clone())
        .expect("режим literary проверен выше");

    // ── Режим: анализ поля (℘→O) ────────────────────────────────────
    if mode == "field" {
        let eng = match lit::LiteraryEngine::new(params, &text, &[], cast) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("poler-literary: {e}");
                return 1;
            }
        };
        let top = lit::qualia::cosine_topology(&eng.field);
        if cli.literary_json {
            let mut report = serde_json::json!({
                "mode": "field",
                "dims": eng.params.dims,
                "tokens": eng.field.n_tokens,
                "terms": eng.field.n_terms,
                "intent_terms": eng.field.term_mass.iter().map(|(t, w)| serde_json::json!({
                    "term": t, "mass": (w * 1e4).round() / 1e4,
                })).collect::<Vec<_>>(),
                "archetypes": top.iter().take(6).map(|&(i, w)| serde_json::json!({
                    "name": lit::qualia::archetype_name(i),
                    "cosine": (w * 1e4).round() / 1e4,
                })).collect::<Vec<_>>(),
                "f_initial": (eng.free_energy() * 1e6).round() / 1e6,
            });
            if let Some(c) = &eng.cast {
                report["fly"] = serde_json::json!({
                    "members": c.members.len(),
                    "reached": c.stats.reached,
                    "inner_edges": c.stats.inner_edges,
                    "inner_abs_mass": c.stats.inner_abs_mass as i64,
                    "top_rotor": c.stats.top_rotor,
                });
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into())
            );
            return 0;
        }
        println!("ПОЛЕ ЗАМЫСЛА (℘→O)");
        println!(
            "  Токенов: {}, уникальных термов: {}, осей: {}",
            eng.field.n_tokens, eng.field.n_terms, eng.params.dims
        );
        println!("  Инвариант (топ термов):");
        for (t, w) in eng.field.term_mass.iter().take(8) {
            println!("    {t:<16} {w:.3}", w = *w as f64);
        }
        println!("  Архетипическая топология:");
        for &(i, w) in top.iter().take(6) {
            println!("    {:<22} {:>+7.4}", lit::qualia::archetype_name(i), w);
        }
        println!(
            "  F₀ = {:.4} (‖Ω(o)‖² = 1), причинный замок: p₀ − p₁ = 0",
            eng.free_energy()
        );
        if let Some(c) = &eng.cast {
            println!(
                "  МУХА: каста {} нейронов, {} внутренних рёбер, топ-ротор {:?}",
                c.members.len(),
                c.stats.inner_edges,
                c.stats.top_rotor
            );
        }
        return 0;
    }

    // ── Режим: генерация нарратива ─────────────────────────────────
    let mut eng = match lit::LiteraryEngine::new(params, &text, &[], cast) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("poler-literary: {e}");
            return 1;
        }
    };
    let t0 = std::time::Instant::now();
    let report = eng.generate();
    if cli.literary_json {
        let mut jr = serde_json::json!({
            "mode": "generate",
            "steps": report.steps,
            "converged": report.converged,
            "f_final": (report.final_f * 1e6).round() / 1e6,
            "h_psi_final": (report.final_h_psi * 1e6).round() / 1e6,
            "causality_clean": report.causality_clean,
            "refractions": report.refractions,
            "elapsed_ms": t0.elapsed().as_millis() as u64,
            "acts": report.acts.iter().map(|a| serde_json::json!({
                "steps": [a.steps.0, a.steps.1],
                "f": [(a.f_start * 1e4).round() / 1e4, (a.f_end * 1e4).round() / 1e4],
                "dominants": a.dominants.iter().map(|d| serde_json::json!({
                    "archetype": d.name, "weight": (d.weight * 1e4).round() / 1e4,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "dramatic_pairs": report.dramatic_pairs.iter().map(|p| serde_json::json!({
                "u": p.u, "v": p.v, "j": (p.j * 1e4).round() / 1e4,
                "u_archetype": p.u_archetype, "v_archetype": p.v_archetype,
            })).collect::<Vec<_>>(),
        });
        jr["text"] = serde_json::json!(report.text);
        println!(
            "{}",
            serde_json::to_string_pretty(&jr).unwrap_or_else(|_| "{}".into())
        );
        return 0;
    }
    println!("{}", report.text);
    println!(
        "  Время: {} мс ({} шагов)",
        t0.elapsed().as_millis(),
        report.steps
    );
    0
}


// ─── v0.74.0: --view-mol — интерактивный 3D/4D-визор молекул ───────────

fn view_mol_command(input: &str, mode: MolModeArg) -> ExitCode {
    use poler_engine::chem::{self, view::ViewMode};
    let g = match chem::resolve_input(input) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("--view-mol: {e}");
            return ExitCode::FAILURE;
        }
    };
    let conf = match chem::embed(&g) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("--view-mol: 3D-укладка не удалась: {e}");
            return ExitCode::FAILURE;
        }
    };
    let vmode = match mode {
        MolModeArg::Ball => ViewMode::BallStick,
        MolModeArg::Vdw => ViewMode::Vdw,
        MolModeArg::Wire => ViewMode::Wire,
    };
    match poler_engine::chem::view::run_interactive(&conf, &g, &g.hill_formula(), vmode) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("--view-mol: {e}");
            ExitCode::FAILURE
        }
    }
}

// ─── v0.75.0: PDB-макромолекулы, гибкий докинг, визор комплекса ─────────

fn pdb_command(file: &str, mode: PdbModeArg) -> ExitCode {
    use poler_engine::chem::pdb::{macro_passport, pocket_blind, pocket_from_ligand, MacroMol};
    let mm = match MacroMol::from_file(file) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("--pdb: {e}");
            return ExitCode::FAILURE;
        }
    };
    match mode {
        PdbModeArg::Info => {
            println!("{}", macro_passport(&mm, file));
            ExitCode::SUCCESS
        }
        PdbModeArg::Pocket => {
            // 1) карманы по лигандам
            for lri in mm.ligand_residues() {
                let p = pocket_from_ligand(&mm, lri);
                let r = &mm.residues[lri];
                println!(
                    "Карман по лиганду {}{} ({}): центр ({:.2}, {:.2}, {:.2}) Å, радиус {:.1} Å, {} атомов рецептора.",
                    r.name_str(),
                    if r.chain == 0 { '_' } else { r.chain as char },
                    (r.atoms.1 - r.atoms.0),
                    p.center[0],
                    p.center[1],
                    p.center[2],
                    p.radius,
                    p.atoms.len()
                );
                let names: Vec<String> = p
                    .residues
                    .iter()
                    .map(|&ri| {
                        let rr = &mm.residues[ri];
                        format!(
                            "{}{}{}",
                            rr.name_str(),
                            if rr.chain == 0 { '_' } else { rr.chain as char },
                            rr.seq
                        )
                    })
                    .collect();
                println!("  Остатки: {}", names.join(", "));
            }
            // 2) слепой карман
            match pocket_blind(&mm) {
                Ok(p) => {
                    println!(
                        "Слепой карман (grid-burial 1.5 Å): центр ({:.2}, {:.2}, {:.2}) Å, {} атомов, {} остатков.",
                        p.center[0],
                        p.center[1],
                        p.center[2],
                        p.atoms.len(),
                        p.residues.len()
                    );
                    let names: Vec<String> = p
                        .residues
                        .iter()
                        .map(|&ri| {
                            let rr = &mm.residues[ri];
                            format!(
                                "{}{}{}",
                                rr.name_str(),
                                if rr.chain == 0 { '_' } else { rr.chain as char },
                                rr.seq
                            )
                        })
                        .collect();
                    println!("  Остатки: {}", names.join(", "));
                }
                Err(e) => println!("Слепой поиск: {e}"),
            }
            ExitCode::SUCCESS
        }
    }
}

fn dock_command(
    smiles: &str,
    file: &str,
    pocket: &str,
    dock_out: Option<&str>,
    runs: usize,
    steps: usize,
    induced: bool,
) -> ExitCode {
    use poler_engine::chem::dock::{dock_text, write_dock_pdb, DockParams, PocketSpec};
    use poler_engine::chem::pdb::MacroMol;
    let mm = match MacroMol::from_file(file) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("--dock: {e}");
            return ExitCode::FAILURE;
        }
    };
    let spec = match pocket.trim() {
        "auto" | "" => PocketSpec::Auto,
        "blind" => PocketSpec::Blind,
        xyz => {
            let parts: Vec<f64> = xyz
                .split(',')
                .map(|s| s.trim().parse::<f64>())
                .collect::<Result<Vec<_>, _>>()
                .ok()
                .filter(|v| v.len() == 3)
                .unwrap_or_default();
            if parts.is_empty() {
                eprintln!(
                    "--pocket: «{xyz}» не распознан (ожидается auto | blind | x,y,z)"
                );
                return ExitCode::FAILURE;
            }
            PocketSpec::Point([parts[0], parts[1], parts[2]], 10.0)
        }
    };
    let params = DockParams {
        runs: runs.max(1).min(64),
        steps: steps.max(100).min(50_000),
        ..Default::default()
    };
    let t0 = std::time::Instant::now();
    // v0.78.0: induced fit (контур C) — гибкость боковых цепей кармана
    if induced {
        use poler_engine::chem::induced_fit::{induced_fit, induced_fit_text, InducedFitParams};
        let ip = InducedFitParams {
            dock: params,
            ..Default::default()
        };
        return match induced_fit(smiles, &mm, &ip, &spec) {
            Ok(res) => {
                println!("{}", induced_fit_text(&res, smiles));
                if let Some(out) = dock_out {
                    match poler_engine::chem::dock::prepare_ligand(smiles) {
                        Ok(lig) => {
                            match write_dock_pdb(out, &lig, &res.final_res.best_positions, &res.final_res)
                            {
                                Ok(()) => {
                                    println!("\nПоза записана: {out} (см. --view-complex {out})")
                                }
                                Err(e) => eprintln!("--dock-out: {e}"),
                            }
                        }
                        Err(e) => eprintln!("--dock-out: {e}"),
                    }
                }
                let _ = t0;
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("--dock --induced: {e}");
                ExitCode::FAILURE
            }
        };
    }
    match poler_engine::chem::dock::dock(smiles, &mm, &params, &spec) {
        Ok(res) => {
            println!("{}", dock_text(&res, smiles));
            if let Some(out) = dock_out {
                match poler_engine::chem::dock::prepare_ligand(smiles) {
                    Ok(lig) => match write_dock_pdb(out, &lig, &res.best_positions, &res) {
                        Ok(()) => println!("\nПоза записана: {out} (см. --view-complex {out})"),
                        Err(e) => eprintln!("--dock-out: {e}"),
                    },
                    Err(e) => eprintln!("--dock-out: {e}"),
                }
            }
            let _ = t0;
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("--dock: {e}");
            ExitCode::FAILURE
        }
    }
}

fn view_complex_command(file: &str) -> ExitCode {
    use poler_engine::chem::pdb::MacroMol;
    use poler_engine::chem::view::{build_complex_scene, run_complex_interactive};
    let mm = match MacroMol::from_file(file) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("--view-complex: {e}");
            return ExitCode::FAILURE;
        }
    };
    let sc = build_complex_scene(&mm);
    match run_complex_interactive(&sc) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("--view-complex: {e}");
            ExitCode::FAILURE
        }
    }
}

// ─── v0.79.0: --curriculum — задачник Curriculum Learning (контур D) ──────

/// `--curriculum [GRADE] [--curriculum-text]`: прогон школьного задачника
/// 5–9 класса (187 задач, двойная запись MVR: эталоны посчитаны независимой
/// Python-реализацией). `--curriculum-text` — режим SciVoice: задача
/// решается из русского текста формулировки, а не из готового JSON-ввода.
/// Код возврата: 0 — все решены, 1 — есть провалы (CI-семантика).
fn curriculum_command(grade: Option<u8>, from_text: bool) -> ExitCode {
    use poler_engine::chem::curriculum::{report_text, run_grade};

    let t0 = std::time::Instant::now();
    let reps = run_grade(grade, from_text);
    let n: usize = reps.iter().map(|r| r.total).sum();
    let p: usize = reps.iter().map(|r| r.passed).sum();
    let dt = t0.elapsed().as_millis();
    print!("{}", report_text(&reps, from_text));
    println!("═══ Время: {dt} мс · {n} задач · решено {p} ═══");
    if reps.is_empty() {
        eprintln!("--curriculum: класс не найден в корпусе");
        ExitCode::FAILURE
    } else if p == n {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

// ─── v0.77.0: --screen — ультра-скрининг библиотек (контур B) ────────────

/// `--screen LIB.sdf|LIB.smi PROTEIN.pdb [--pocket auto|blind|x,y,z]
/// [--top N] [--workers K] [--screen-out hits.json]`:
/// каскад пре-фильтры → быстрый MC (JIT) → полный докинг топ-N.
fn screen_command(
    lib: &str,
    file: &str,
    pocket: &str,
    top: usize,
    workers: usize,
    out: Option<&str>,
) -> ExitCode {
    use poler_engine::chem::dock::PocketSpec;
    use poler_engine::chem::pdb::MacroMol;
    use poler_engine::chem::screen::{screen_json, screen_library, screen_text, ScreenOptions};
    use poler_engine::chem::sdf::read_library;

    let t0 = std::time::Instant::now();
    let (entries, skipped) = match read_library(lib) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("--screen: {e}");
            return ExitCode::FAILURE;
        }
    };
    if entries.is_empty() {
        eprintln!("--screen: библиотека «{lib}» пуста (0 валидных записей)");
        return ExitCode::FAILURE;
    }
    for s in skipped.iter().take(10) {
        eprintln!("пропуск: {s}");
    }
    if skipped.len() > 10 {
        eprintln!("… и ещё {} пропущенных записей", skipped.len() - 10);
    }
    let mm = match MacroMol::from_file(file) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("--screen: {e}");
            return ExitCode::FAILURE;
        }
    };
    let spec = match pocket.trim() {
        "auto" | "" => PocketSpec::Auto,
        "blind" => PocketSpec::Blind,
        xyz => {
            let parts: Vec<f64> = xyz
                .split(',')
                .map(|s| s.trim().parse::<f64>())
                .collect::<Result<Vec<_>, _>>()
                .ok()
                .filter(|v| v.len() == 3)
                .unwrap_or_default();
            if parts.is_empty() {
                eprintln!(
                    "--pocket: «{xyz}» не распознан (ожидается auto | blind | x,y,z)"
                );
                return ExitCode::FAILURE;
            }
            PocketSpec::Point([parts[0], parts[1], parts[2]], 10.0)
        }
    };
    let opts = ScreenOptions {
        workers,
        top,
        ..Default::default()
    };
    eprintln!(
        "--screen: {} записей · {} · {} карман",
        entries.len(),
        lib,
        pocket.trim()
    );
    match screen_library(&entries, &mm, &spec, &opts) {
        Ok(res) => {
            println!("{}", screen_text(&res, top));
            for e in res.errors.iter().take(10) {
                eprintln!("ошибка: {e}");
            }
            if let Some(path) = out {
                match std::fs::write(path, screen_json(&res)) {
                    Ok(()) => println!("\nJSON-отчёт записан: {path}"),
                    Err(e) => eprintln!("--screen-out: {e}"),
                }
            }
            let _ = t0;
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("--screen: {e}");
            ExitCode::FAILURE
        }
    }
}

// ─── v0.76.0: --jit — внутренний компилятор (контур A) ────────────────────

/// `--jit bench|asm|graph [--jit-rows N --jit-cols N]`:
/// «голос» компилятора наружу — бенчмарк, листинг, граф.
fn jit_command(mode: JitModeArg, rows: usize, cols: usize) -> ExitCode {
    use poler_engine::graph::graph_asm::{
        DataflowCompiler, DataflowGraph, NodeAct, Reducer, WeightKind,
    };
    use poler_engine::triune::jit_loop::ExecutableKernel;

    // детерминированный случайный matvec-граф (сид-детерминизм канона)
    let lcg = |s: &mut u64| {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*s >> 33) as i32 as f32 / i32::MAX as f32)
    };
    let mut g = DataflowGraph::new(cols, 0.02);
    g.add_layer(rows, NodeAct::Identity, Reducer::Sum);
    let mut s = 0xC0FFEEu64;
    let mut w = Vec::with_capacity(rows * cols);
    for _ in 0..rows * cols {
        w.push(lcg(&mut s));
    }
    for i in 0..rows {
        for j in 0..cols {
            g.add_edge(0, j, i, WeightKind::F32(w[i * cols + j]));
        }
    }
    let compiled = match DataflowCompiler::compile_x86_64(&g, "jit_bench_graph") {
        Ok(c) => c,
        Err(e) => {
            println!("компиляция не удалась: {e}");
            return ExitCode::FAILURE;
        }
    };
    let x: Vec<f32> = (0..cols).map(|_| lcg(&mut s)).collect();

    match mode {
        JitModeArg::Bench => {
            let kern = match ExecutableKernel::load(&compiled.machine_bytes) {
                Ok(k) => k,
                Err(e) => {
                    println!("загрузка RX: {e}");
                    return ExitCode::FAILURE;
                }
            };
            // бит-в-бит проверка
            let ref_y = poler_engine::graph::graph_asm::dataflow_reference_eval(&g, &x).unwrap();
            let mut y = vec![0.0f32; rows];
            unsafe { kern.call_raw(x.as_ptr(), y.as_mut_ptr()) };
            let mut mismatches = 0usize;
            for (a, b) in ref_y.iter().zip(&y) {
                if a.to_bits() != b.to_bits() {
                    mismatches += 1;
                }
            }
            println!("граф {cols}→{rows}: {} рёбер, вакуум {:.1}%", g.active_edges(), g.sparsity_ratio() * 100.0);
            println!("инструкций: {}; код: {} Б ({} Б/весу)", compiled.instruction_count, compiled.machine_bytes.len(),
                compiled.machine_bytes.len() as f32 / g.active_edges().max(1) as f32);
            println!("бит-в-бит IEEE-754: {}", if mismatches == 0 { "OK ✓".to_string() } else { format!("{mismatches} расхождений!") });
            // бенчмарк: JIT vs наивный цикл
            let n = 100_000usize;
            let t0 = std::time::Instant::now();
            let mut acc = 0.0f32;
            for _ in 0..n {
                unsafe { kern.call_raw(x.as_ptr(), y.as_mut_ptr()) };
                acc += y[0];
            }
            let jit_us = t0.elapsed().as_secs_f64() * 1e6 / n as f64;
            let t0 = std::time::Instant::now();
            for _ in 0..n {
                for i in 0..rows {
                    let mut a = 0.0f32;
                    for j in 0..cols {
                        a += x[j] * w[i * cols + j];
                    }
                    y[i] = a;
                }
                acc += y[0];
            }
            let naive_us = t0.elapsed().as_secs_f64() * 1e6 / n as f64;
            println!("JIT: {jit_us:.2} мкс/вызов vs наивный цикл: {naive_us:.2} мкс → {:.2}× (sink {acc:.4})",
                naive_us / jit_us.max(1e-9));
            println!("W^X: код на PROT_READ|PROT_EXEC странице (запись → исполнение, никогда вместе)");
            ExitCode::SUCCESS
        }
        JitModeArg::Asm => {
            println!("{}", compiled.asm_listing);
            ExitCode::SUCCESS
        }
        JitModeArg::Graph => {
            println!("DataflowGraph «jit_bench_graph»");
            println!("  входов: {cols}, слоёв: {}", g.layers.len());
            for (li, l) in g.layers.iter().enumerate() {
                println!("  слой {}: ширина {}, активация {:?}, редьюсер {:?}", li + 1, l.width, l.act, l.reduce);
            }
            println!("  рёбер: {} (вакуум {:.1}%)", g.active_edges(), g.sparsity_ratio() * 100.0);
            println!("  машинный код: {} инструкций, {} Б", compiled.instruction_count, compiled.machine_bytes.len());
            ExitCode::SUCCESS
        }
    }
}

// ────────────────────────── тесты v0.91.0 CORPUS-TRIT ──────────────────────────

#[cfg(test)]
mod corpus_tests {
    use super::poler_archive_chunks;
    use poler_engine::archive::ingest::{corpus_config, corpus_pack};
    use poler_engine::archive::reader::PolerReader;
    use poler_engine::archive::stream_writer::StreamWriteConfig;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("poler-main-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    /// .poler-корпус → чанки без распаковки: текстовые записи читаются,
    /// бинарные отсеяны NUL-сниффом, резервуар держит потолок max_chunks.
    #[test]
    fn poler_archive_chunks_reservoir_and_binary_sniff() {
        let d = std::env::temp_dir().join(format!(
            "poler-semcorpus-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        // проза с двумя абзацами (>= 40 символов каждый)
        std::fs::write(
            d.join("story.md"),
            "Троичный кристалл помнит весь корпус без единой распаковки на диск.\n\nСуверенный архиватор льёт поток прямо в кристалл из mmap-региона.",
        )
        .unwrap();
        // короткий текст — не даёт чанков (осколок < 40)
        std::fs::write(d.join("tiny.txt"), "слишком коротко").unwrap();
        // бинарная запись — NUL-снифф обязан отсеять
        std::fs::write(d.join("blob.bin"), [0u8, 1, 2, 0, 255, 254].repeat(1024)).unwrap();

        let p = tmp("semcorpus.poler");
        let cfg = StreamWriteConfig {
            cdc: poler_engine::archive::dedup::CdcParams::new(4096, 16 * 1024, 64 * 1024),
            ..corpus_config(2)
        };
        let (stats, _) = corpus_pack(&d, &p, cfg, false, false).unwrap();
        assert_eq!(stats.files, 3);

        // резервуар потолком 1: сколько бы чанков ни было — ровно ≤ 1
        let mut chunks = Vec::new();
        let (text_files, binary_files) = poler_archive_chunks(&p, 1, &mut chunks).unwrap();
        assert_eq!(text_files, 2, "story.md + tiny.txt (текстовые)");
        assert_eq!(binary_files, 1, "blob.bin отсеян NUL-сниффом");
        assert_eq!(chunks.len(), 1, "резервуар держит потолок");
        assert!(chunks[0].0.starts_with("semcorpus.poler::"), "имя архив::запись: {}", chunks[0].0);
        assert!(chunks[0].1.contains("кристалл"));

        // без потолка (4096): оба абзаца story.md среди чанков
        let mut all = Vec::new();
        let _ = poler_archive_chunks(&p, 4096, &mut all).unwrap();
        assert!(all.len() >= 2, "два абзаца: {}", all.len());
        assert!(all.iter().any(|(_, t)| t.contains("распаковки")));
        assert!(all.iter().any(|(_, t)| t.contains("mmap-региона")));

        // lossless-verify попутно
        let r = PolerReader::open(&p).unwrap();
        let rep = r.verify().unwrap();
        assert!(rep.all_ok, "verify: {:?}", rep.files_bad);
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_file(&p);
    }
}
