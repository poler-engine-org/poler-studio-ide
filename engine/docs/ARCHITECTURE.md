# Архитектура POLER Engine v2.0

> Статус: актуален на v2.0 «Sovereign Stack» (сентябрь 2026).
> Карта всех документов — [INDEX.md](INDEX.md).

## 1. Система в одном абзаце

POLER Engine — однобинарный Rust-инструмент, который превращает локальные
данные (файлы, репозитории, веб-краулы, модели весов) в **поисково-аналитическую
память для ИИ-агентов**. Пять уровней доступа покрывают весь логический скоуп:
точный grep (слой 0) → лексический BM25 с ε-плотностью и резонансом R(t)
(слой 1) → RAG-чанки с byte-якорями (слой B) → плотный семантический поиск
(RaBitQ + HNSW, слой D) → сущностно-реляционный граф и K-hop рассуждения
(слой G). Поверх этого — нативный ML-инференс без ML-зависимостей (`pqc`):
энкодеры BGE-M3/GLiNER, декодер ChatGLM3 и квантово-фазовое ядро из
`.pqw`-контейнеров. Всё на CPU, всё офлайн, ноль облачных API.

## 2. Слои и потоки данных

```text
┌─────────────────────────────────────────────────────────────────────────┐
│  ИНТЕРФЕЙСЫ: CLI (~85 флагов) · --shell REPL · --tui (ratatui)          │
│  --gateway (Terminal Gateway, двойной контур) · --mcp / --mcp-http      │
│  MCP-инструменты: poler_search/grep/chunk/web_search/crawl/fetch/...    │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────────────────┐
│  УРОВЕНЬ ДОСТУПА К ДАННЫМ (пять слоёв полноты)                          │
│                                                                          │
│  Слой 0 — ТОЧНОСТЬ:  retrieval::grep — все совпадения, exit-коды grep,  │
│        Teddy SIMD-предфильтр (решёто якорных байтов pshufb)             │
│  Слой 1 — ЛЕКСИКА:   streaming (проходы 1/2) → InvertedIndex →          │
│        BM25/WebRank + resonance (ε-плотность, IIR R(t), сцены)           │
│  Слой B — ПАССАЖИ:   retrieval::chunker — секция→абзац→предложение,     │
│        byte-range якоря для цитирования                                 │
│  Слой D — СЕМАНТИКА: vectors — PqwEmbedder (нативный BGE-M3) →          │
│        RaBitQ 1-бит коды → HNSW-граф → mmap QuantizedStore (PRBQ)       │
│  Слой G — СВЯЗИ:     parser (SVO-тройки, сцены) → graph (EntityGraph,   │
│        K-hop BFS, temporal-слои) + aidde (call graph, impact-анализ)    │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────────────────┐
│  ПЛОТНОСТЬ ПАМЯТИ:  compression — FSST-арена строк, VocabArena,         │
│  lz4-постинги, zstd doc store; trait TermFreqs (ε побитово одинаков     │
│  во всех представлениях)                                                │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────────────────┐
│  СУВЕРЕННЫЙ ИНФЕРЕНС: pqc — AVX2 weight-only кернелы (int8/int4/f32),   │
│  LayerNorm/RMSNorm/GELU/SiLU/softmax/RoPE; .pqw v2 (mmap, SHA-256);     │
│  потребители: llm/glm_engine (декодер), ner/native_gliner (span-NER),   │
│  vectors/pqw_bridge (эмбеддер)                                          │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────────────────┐
│  КВАНТОВО-ФАЗОВЫЙ МОСТ: quantum — QuantumMind (L5-генератор, русла J),  │
│  crystallizer (R1CS-кристаллизатор весов), meta_compiler (Reverse       │
│  Meta-Compiler POLER-ERI v3.2.0: CircuitBuilder → CSE → 8×SIMD волны →  │
│  плоский Rust-кодоген)                                                  │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────────────────┐
│  ВНЕШНИЙ МИР: web — CDP-краулер (robots.txt, SimHash, PageRank),        │
│  vcs — Unified VCS Mesh (gh/gl/gt/gix), sources/notes — SQLite          │
└─────────────────────────────────────────────────────────────────────────┘
```

Ключевое свойство потока: **одни и те же данные обслуживаются всеми слоями без
дублирования**. TermFreqs-трейт гарантирует, что ε-плотность считается
побитово одинаково из RAM-индекса, FSST-словаря и lz4-постингов — слой
сжатия прозрачен для математики.

## 3. Модульная карта (29 модулей, 67 K LOC)

Подробный справочник с pub-API — [MODULES.md](MODULES.md). Здесь — группировка
по ответственности.

### 3.1. Ядро поиска (то, ради чего всё)

| Модуль | LOC | Ответственность |
|---|---|---|
| `engine.rs` | 579 | Оркестрация: полный и watcher-прогоны, WatchState |
| `streaming.rs` | 1 266 | Проход 1 (токены/лексемы) и проход 2 (контекст), литеральный предфильтр, гиганты параллельно |
| `tokenizer/` | 392 | Инвертированный индекс, zero-copy PII-очистка |
| `resonance/` | 561 | ε-плотность (generic по TermFreqs), IIR-фильтр R(t) |
| `poler.rs` | 372 | Канонический POLER-цикл (порт P3_Engine, CORDIC) |
| `psi.rs` | 329 | POLER[Ψ] — поле внимания, ResonanceMemory |
| `output/` | 198 | AI-Ready контракт: ContextAnchor, SearchResult |

### 3.2. Точный поиск и пассажи

| Модуль | LOC | Ответственность |
|---|---|---|
| `retrieval/` | 3 582 | grep-режим (полнота + parity с GNU grep), чанкер, Semantic Bridge ru↔en, Teddy SIMD |
| `parser/` | 2 187 | AST-lite скоупы для кода, markdown-сцены, SVO-тройки |

### 3.3. Связи и анализ

| Модуль | LOC | Ответственность |
|---|---|---|
| `graph/` | 853 | EntityGraph, K-hop BFS, temporal-слои, IdentityPolicy |
| `aidde/` | 1 294 | AI-Interpreted Dependency & Impact Engine: таблица символов → call graph → impact-паспорт |

### 3.4. Плотность памяти

| Модуль | LOC | Ответственность |
|---|---|---|
| `compression/` | 2 098 | FSST-порт, VocabArena, lz4-постинги, zstd doc store |

### 3.5. Векторный субстрат

| Модуль | LOC | Ответственность |
|---|---|---|
| `vectors/` | 2 308 | RaBitQ-кодирование (вращение Адамара + 1 бит), оценки sym/ADC, HNSW, mmap-хранилище PRBQ, трейт Embedder |

### 3.6. Суверенный инференс

| Модуль | LOC | Ответственность |
|---|---|---|
| `pqc/` | 6 636 | Формат .pqw v2, AVX2-кернелы, BERT/DeBERTa-спины, SHA-256 FIPS 180-4, XLM-R токенизатор (Unigram-Viterbi) |
| `llm/` | 1 112 | GLM-декодер: RoPE, MQA/GQA, SwiGLU, MoE-роутер, KV-арена, сэмплирование |
| `ner/` | 750 | GLiNER span-NER: BiLSTM-голова, SpanMarker-MLP, zero-shot метки |
| `quantum/` | 2 271 | QuantumMind, crystallizer, meta_compiler (см. [quantum-eri.md](quantum-eri.md)) |

### 3.7. Внешний мир и интерфейсы

| Модуль | LOC | Ответственность |
|---|---|---|
| `web/` | 5 583 | CDP-краулер, BM25+PageRank веб-индекс (SQLite), SimHash-дедуп, WebLens |
| `vcs/` | 3 456 | VCS Mesh: GitHub/GitLab/Gitea REST, gix, LFS, схемы `gh:// gl:// gt://` |
| `gateway/` | ~14 060 | Terminal Gateway: sandbox, root broker, sentinel, containers, PTY |
| `shell/` | 7 647 | REPL + TUI (ratatui), doc_browser, completer |
| `mcp.rs` + `mcp_http.rs` | 2 049 | MCP-сервер stdio/HTTP, токен-аутентификация |
| `main.rs` | 1 910 | CLI (clap derive) |
| `reader/` | 853 | LLM Reader: курсор, закладки, ReadSet, Active Inference |
| `notes/` + `sources/` | 888 | SQLite CRUD заметок и источников |
| `bench/` | 1 819 | Бенчмарк-сьют 6 контуров |
| `license/` | 104 | EULA-метаданные (гейт удалён в v2.0) |

## 4. Ключевые интерфейсы (контракты)

### 4.1. Трейт `TermFreqs` — прозрачность сжатия

```rust
pub trait TermFreqs {
    fn total_tokens(&self) -> u64;
    fn term_freq(&self, term: &str) -> u32;
    // + итерация по термам
}
```

Любое представление постинг-листов (RAM `HashMap`, FSST-словарь, lz4-блок)
реализует трейт, и `calculate_epsilon` даёт **побитово одинаковый** результат.
Это архитектурный инвариант: сжатие не имеет права менять математику.

### 4.2. Трейт `Embedder` — сменяемость моделей

```rust
pub trait Embedder: Send + Sync {
    fn embed(&self, texts: &[&str]) -> Vec<Vec<f32>>;
}
```

Точка подключения BGE-M3 (`PqwEmbedder` поверх .pqw) и тестовых двойников
(`HashEmbedder` — feature-hashing для конвейерных тестов без модели). Субстрат
(RaBitQ/HNSW/PRBQ) не знает, какой эмбеддер его кормит.

### 4.3. Трейт `CodeSource` — хранилище кодов

Слот → коды (`&[u64]`), скаляры (`mu/delta/gamma`), внешний `DocId`. HNSW и
оценщики sym/ADC работают через него: RAM-сборщик и mmap-представление
неразличимы для алгоритмов.

### 4.4. Трейт `VcsAdapter` — Unified VCS Mesh

Единый интерфейс GitHub/GitLab/Gitea REST + gix поверх одного и того же
клона. URL-схемы `gh://owner/repo@rev`, `gl://`, `gt://`, `gix://` —
программный поиск по репозиториям без ручного clone.

### 4.5. AI-Ready контракт вывода

`ContextAnchor` — каждый результат несёт byte-offset, скоуп, сцену, метрики
(ε, R, tf) и рендерится в `ai-json` / `md` / `simple`. Агент получает не
«топ-10 ссылок», а машиночитаемый якорь для цитирования и навигации.

## 5. Форматы данных

| Формат | Где | Спецификация |
|---|---|---|
| `.pqw` v2 | веса моделей (int8/int4/f32, SHA-256, `__tokenizer__`) | [formats/PQW_FORMAT.md](formats/PQW_FORMAT.md) |
| `.prbq` v1 | квантованные векторы (1 бит + 12 Б скаляров на вектор) | [formats/PRBQ_FORMAT.md](formats/PRBQ_FORMAT.md) |
| `web-index.db` | SQLite веб-краула (pages/terms/links/hosts/meta) | [formats/WEB_INDEX_FORMAT.md](formats/WEB_INDEX_FORMAT.md) |
| FSST-таблица | словарь строк индекса (побитово детерминированная сериализация) | compression/mod.rs `//!` |
| `.safetensors` | вход конвертеров и crystallizer (заголовок парсится вручную) | crystallizer.rs `//!` |

## 5.1. Зависимости и сборка

22 внешние зависимости, ноль ML-фреймворков. С фазы **M2** (merge `f5e53dc`
+ workspace `33c3b48`, 2026-09-16) квантовые крейты `pqc`/`pqw` живут
**в самом репозитории** (`crates/`, единый Cargo Workspace, история
RQ1–RQ23 сохранена) — свежий `git clone` + `cargo build` работает без
соседних клонов и секретов (провенанс — `crates/README.md`; план —
[MERGE_PLAN.md](MERGE_PLAN.md)). Release-профиль общий на весь workspace:
LTO fat, codegen-units=1, panic=abort, strip — поэтому при RAM < 8 ГБ
собирать с `-j1`.

CI (`.github/workflows/ci.yml`) с фазы M2 гоняет `cargo test --workspace`
(clippy-гейт + весь сьют трёх пакетов) на голом checkout без токенов;
до M2 опечатка `branches: ain` маскировала неразрешимость path-депов
(CI физически не собирал репозиторий на чистом checkout).

## 6. Режимы исполнения и границы доверия

```text
--shell / --tui          локальные интерфейсы, полный доступ к своим данным
--gateway                 Terminal Gateway: двойной контур исполнения
                          (sandbox-контур по умолчанию, host-контур за
                          явным разрешением), bind-mount, broker-токены
--mcp / --mcp-http        MCP-сервер для LLM-агентов: 8 инструментов,
                          Bearer-токен, path/URL-гарды
--dangerously-allow-all   аварийный люк (см. docs/terminal-gateway-architecture.md)
```

Философия границ: «рут — тоже привилегия хоста». Sentinel отслеживает
побеги из sandbox, root broker требует пароль для эскалации, jail-контейнеры
изолируют файловую систему. Приватность — продуктовое свойство, не опция.

## 7. Что сознательно отсутствует (v2.0)

- Облачные API: Google/NotebookLM/Gmail/Drive/OAuth — удалены (Шаг 1
  суверенного стека). Веб-краулинг через CDP и публичные REST — разрешены:
  это не облачная зависимость.
- Python runtime, ONNX Runtime, GPU-бэкенды — инференс только нативный
  через `pqc` (см. PLAN_POLER_V2 Part E).
- Фоновые сервисы-демоны — всё запускается явной командой и умирает вместе
  с ней; персистентность только в файлах (индексы, SQLite, .pqw).
- «ИИ-решения»: движок не ранжирует за пользователя смыслы и не делает
  выводов — он отдаёт плотности, якоря и графы; интерпретация принадлежит
  вызывающему агенту (тезис «инструмент, не ИИ», research/dialogue_tool_vs_ai.md).

## 8. Точки расширения (ближайшие по плану)

1. **ColBERT/Matryoshka** (кирпич 3 векторов): поздняя интеракция поверх
   того же PRBQ-субстрата.
2. **SPLADE**: разреженные семантические веса в терминах FSST-словаря.
3. **IIR-Resonance Fusion**: слияние 4 сигналов (Lexical + Dense + Sparse +
   Graph) в единое топографическое ранжирование — уникальная инновация
   проекта, математика в THEORY.md §4.
4. **Код-интеллект**: tree-sitter/Salsa поверх aidde.
5. **Leiden-кластеризация KG** на GLiNER-весах.
6. **Локальный ChatGLM3-6B** (Part F): конвертер готов, конвертация на
   железе владельца (15 ГБ RAM → int4 ~3.2 ГБ).

## 9. Как проверить, что архитектура не сломана

- `cargo test` — ~1059 юнит+интеграционных тестов, включая дифференциальные
  против эталонов (numpy, HF tokenizers, GNU grep parity, побитовый
  детерминизм). Детали — [TESTING.md](TESTING.md).
- `--benchmark` — 6 контуров (Teddy/grep/BM25-golden/chunker/vectors/
  compression) с JSON-отчётом.
- `--pqw-selftest` — автономный цикл формата весов 6/6.
- Интеграционный тест кодогена: сгенерированный meta_compiler'ом Rust-файл
  компилируется настоящим rustc и совпадает с runtime-конвейером побитово.
