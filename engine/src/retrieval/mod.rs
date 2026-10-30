//! Native Retrieval (v0.20.0): два недостающих слоя инструментария
//! ИИ-агента — точный поиск (grep) и passage-нарезка (RAG-чанки).
//!
//! Конвейер из четырёх независимых слоёв (детальнее —
//! `docs/native-retrieval-analysis.md`):
//!
//! * **Слой 0 — [`grep`]**: все точные совпадения, без индекса,
//!   гарантия полноты. Замена внешнему grep/ripgrep.
//! * **Слой 1 — BM25-ранжирование** (`web::index`, не тронут):
//!   релевантность по корпусу, объяснимый скор.
//! * **Слой B — [`chunk`]**: документ → фрагменты с якорями.
//!   Замена внешнему RAG-конвейеру chunking→retrieve.
//! * **Слой S — [`semantic_bridge`]** (v0.21): кросс-языковый сенсор
//!   запроса — рус запрос находит англ корпус и обратно; кандидаты
//!   подмешиваются в BM25 с пониженным весом, ранжирование и WHY
//!   остаются за детерминированным ядром.
//!
//! Ни одна новая зависимость не добавлена: `aho-corasick`, `regex`,
//! `ignore`, `memchr`, `rayon` уже были в дереве движка.

pub mod chunk;
pub mod filecache;
pub mod grep;
pub mod semantic_bridge;
pub mod teddy;

pub use chunk::{
    chunk_document, render_chunks_text, Chunk, ChunkConfig, ChunkFormat, ChunkReport, Provenance,
    DEFAULT_OVERLAP_TOKENS, DEFAULT_TARGET_TOKENS,
};
pub use grep::{
    collect_archives, grep_buffer, grep_run, grep_run_cached, render_text, stdout_is_tty,
    GrepConfig, GrepGroup, GrepLineOut, GrepMode, GrepOutput, GrepReport, GrepStats,
};
pub use semantic_bridge::{
    LexiconSensor, QueryExpansion, SemanticBridge, SemanticCandidate, SemanticSensor,
    TermExpansion, BRIDGE_TERM_WEIGHT,
};
pub use teddy::{Teddy, TeddyError, TeddyMatch, MAX_PATTERNS};
