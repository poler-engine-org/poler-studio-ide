# ПРОМПТ ДЛЯ GLM (AGENT MODE) — ДОРАБОТКА POLER-ENGINE v2.0

> **Версия:** 2026-09-13
> **Назначение:** Промпт для GLM в agent mode через GitHub
> **Цель:** Доработать poler-engine до v2.0 — превзойти SOTA на порядок
> **Принцип:** Инструмент не понимает. Инструмент отдаёт. ИИ — понимает. Автор — творит. Инструмент — служит.

---

## 0. КОНТЕКСТ ПРОЕКТА

Ты работаешь над **poler-engine** — поисково-аналитическим движком на Rust. Это НЕ искусственный интеллект. Это **инструмент для ИИ** — гиппокамп, который даёт LLM-агентам глаза, память и верификацию.

**Автор проекта:** Виталий Коток (Kotokvit)
**Репозиторий движка:** https://github.com/poler-engine-org/poler-engine
**Репозиторий романа (тестовый корпус):** https://github.com/Kotokvit/Eteryya
**Текущая версия:** v0.28.0
**Целевая версия:** v2.0

**Главная цель:** Превзойти всё, что есть в 21 веке, на порядок или выше. Все заимствованные решения — 100% дорабатываются и переписываются.

**Критический принцип (НЕ НАРУШАТЬ):**
> Инструмент НЕ понимает контент. Инструмент НЕ генерирует текст. Инструмент НЕ принимает решений.
> Инструмент индексирует. Находит. Отдаёт. Связывает по метаданным.
> ИИ (GLM/Claude/GPT) — понимает. Автор — творит. Инструмент — служит.
> Если фаза доработки требует от poler-engine «понимать» контент — она нарушает принцип.
> Если фаза требует «индексировать и отдавать» — она соответствует принципу.

---

## 1. УСТАНОВКА И НАСТРОЙКА

### 1.1. Клонирование репозитория

```bash
# Клонировать poler-engine в skills/
cd ~/my-project/skills/
git clone https://github.com/poler-engine-org/poler-engine.git poler-engine
cd poler-engine

# Проверить, что Rust установлен
rustc --version  # нужно 1.80+
cargo --version

# Если Rust нет — установить
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
source ~/.cargo/env
```

### 1.2. Сборка

```bash
# Сборка в release-режиме (используй -j1 если RAM < 8GB, иначе OOM)
cargo build --release -j1

# Проверка
./target/release/poler-engine --version
# Должно вывести: poler-engine 0.28.0

# Установка в PATH
cp target/release/poler-engine ~/.local/bin/
ln -sf ~/.local/bin/poler-engine /usr/local/bin/poler-engine
```

### 1.3. ОТВЯЗКА ОТ GOOGLE И NOTEBOOKLM (КРИТИЧНО)

**Задача:** Полностью отвязать poler-engine от Google-сервисов и NotebookLM. Эти интеграции нарушают принцип суверенного стека.

**Что нужно удалить/отключить:**

```bash
# 1. Удалить модуль google/ (OAuth, Gmail, Drive, NotebookLM, CDP browser)
rm -rf src/google/

# 2. Удалить модуль nlm* (NotebookLM)
# Найти все ссылки на google/nlm в коде:
grep -rn "google\|nlm\|oauth\|gmail\|drive\|notebooklm" src/ --include="*.rs" | grep -v "//.*google"

# 3. Удалить из Cargo.toml зависимости, которые ТОЛЬКО для google:
# (gix — оставить, нужен для VCS; ureq — оставить, нужен для web)
# Удалить: ed25519-dalek (license gate), arboard (clipboard — опционально)

# 4. Удалить CLI-флаги google/nlm из src/main.rs:
# --google-auth, --google-gmail, --google-drive, --google-status,
# --google-browse, --google-fetch, --google-scopes,
# --import-browser-session, --auth-ui,
# --nlm-notebooks, --nlm-source, --nlm-notes, --nlm-artifacts,
# --nlm-account, --nlm-chat, --nlm-media, --nlm-shot, --nlm-sync,
# --gcp-auth, --license-import

# 5. Удалить dev-stand/ (весь Google-аутентификационный стенд)
rm -rf dev-stand/

# 6. Обновить src/lib.rs — убрать `pub mod google;`

# 7. Обновить src/main.rs — убрать все match-ветки для google/nlm флагов

# 8. Проверить, что сборка проходит БЕЗ google:
cargo build --release -j1 2>&1 | grep -i "error\|warning" | head -20

# 9. Обновить SKILL.md — убрать все упоминания google/nlm
```

**Что ОСТАВЛЯЕМ:**
- `--web`, `--crawl`, `--web-search` (веб-краулинг через CDP — это НЕ google)
- `--mcp`, `--mcp-http` (MCP-сервер — это НЕ google)
- `--grep`, `-q` (поиск — ядро)
- `--chunk` (RAG-чанки)
- `--tui`, `--shell` (интерфейсы)
- `--gateway` (Docker sandbox — это НЕ google)
- `--semantic-expand` (кросс-языковое расширение)

---

## 2. ЧТО ИЗУЧИТЬ (ОБЯЗАТЕЛЬНО ПРОЧИТАТЬ)

Перед началом работы ПРОЧТИ следующие файлы в репозитории poler-engine:

### 2.1. Мастер-план (КРИТИЧНО)

```
PLAN_POLER_V2.md                    — мастер-план (1001 строка)
```

Этот файл содержит:
- Часть A: Текущее состояние + 10 фаз доработки + что украсть + что разработать
- Часть B: Архитектурный тезис «инструмент, не ИИ» (КРИТЕРИЙ для всех решений)
- Часть C: Сводные выжимки из 4 research-отчётов
- Часть D: Референс на диалог «инструмент vs ИИ»

### 2.2. Research-отчёты (в docs/research/)

```
docs/research/research_semantic_search.md   — 483 строки, SOTA векторный поиск
docs/research/research_code_agentic.md      — 572 строки, tree-sitter/Salsa/MCP/WASM
docs/research/research_streaming_nlp.md     — 793 строки, FSST/zstd/RaBitQ/DBSP
docs/research/research_rag_kg.md            — 215 строк, GraphRAG/GLiNER/contradictions
docs/research/dialogue_tool_vs_ai.md        — 1192 строки, диалог «инструмент vs ИИ»
```

### 2.3. Существующий код (для понимания архитектуры)

```
src/engine.rs                    — ядро движка
src/psi.rs                       — POLER[Ψ] уравнение внимания
src/resonance/epsilon.rs         — ε-плотность
src/resonance/iir_filter.rs      — IIR-резонанс R_t = ε_t + φ·R_{t-1}
src/graph/entity_graph.rs        — K-hop граф сущностей
src/retrieval/semantic_bridge.rs — кросс-языковое расширение
src/retrieval/grep.rs            — grep-режим
src/retrieval/chunk.rs           — RAG-чанки
src/tokenizer/inverted_index.rs  — инвертированный индекс
src/tokenizer/pii.rs             — PII-маскирование
src/web/simhash.rs               — SimHash дедупликация
src/web/index.rs                 — BM25 + PageRank
src/aidde/                       — AIDDE symbol table (SQLite)
src/parser/triples.rs            — SVO triple extraction
src/parser/markdown_scenes.rs    — парсер сцен
src/streaming.rs                 — потоковый конвейер
src/mcp.rs / src/mcp_http.rs     — MCP-сервер
src/shell/                       — TUI/REPL
```

### 2.4. Документация автора

```
README.md                         — полное описание (русский)
FUTURE_ROADMAP.md                 — roadmap автора («превзойти Google»)
docs/future-streaming-archives.md — план zero-storage архивов
docs/native-retrieval-analysis.md — анализ retrieval-слоя
Cargo.toml                        — зависимости
```

---

## 3. ПРИОРИТЕТЫ ДОРАБОТКИ (ПОРЯДОК ВЫПОЛНЕНИЯ)

> **Принцип приоритизации:** Сначала быстрые победы без архитектурных изменений. Потом критические пробелы. Потом уникальные инновации.

### ПРИОРИТЕТ 1: Отвязка от Google/NotebookLM (1-2 дня)

**Задача:** Удалить весь google/ и nlm-код. Движок должен работать БЕЗ каких-либо внешних облачных сервисов.

**Критерий успеха:** `cargo build --release -j1` проходит. `poler-engine --version` работает. `poler-engine ~/eteryya -q "Алексей"` работает. Никаких google/nlm флагов в `--help`.

### ПРИОРИТЕТ 2: Foundation — Free Wins (1 неделя)

| Задача | Что | Зависимость | LOC |
|---|---|---|---|
| 2.1 | `madvise(SEQUENTIAL/RANDOM/WILLNEED)` в mmap | memmap2 (уже есть) | ~50 |
| 2.2 | `whatlang` — language detection (75 языков) | whatlang crate | ~30 |
| 2.3 | `rust-stemmers` — Snowball stemming (18 языков) | rust-stemmers crate | ~100 |
| 2.4 | `unicode-segmentation` — UAX#29 word boundaries | unicode-segmentation crate | ~50 |
| 2.5 | Teddy SIMD multi-literal matcher (из ripgrep internals) | std::simd | ~400 |

**Критерий успеха:** ~2× ускорение mmap. Правильная токенизация для 18 языков. Multi-pattern search в 3-5× быстрее Aho-Corasick.

### ПРИОРИТЕТ 3: Compression — Memory Density (1 неделя)

| Задача | Что | Зависимость | LOC |
|---|---|---|---|
| 3.1 | FSST под inverted index (токены, пути, URL) | fsst-rs crate | ~400 |
| 3.2 | zstd dictionary для doc store | zstd crate | ~100 |
| 3.3 | lz4_flex для hot postings | lz4_flex crate | ~50 |

**Критерий успеха:** Inverted index в RAM занимает в 5-10× меньше. 65K файлов → было 2GB → станет 200-400MB.

### ПРИОРИТЕТ 4: Vector Layer (2-3 недели) — КРИТИЧНО

> Без векторного слоя нечем соревноваться с SOTA. Это самая длинная фаза — начать раньше.

| Задача | Что | Зависимость | LOC |
|---|---|---|---|
| 4.1 | `fastembed-rs` интеграция: BGE-M3 (dense + sparse) | fastembed-rs, ort | ~200 |
| 4.2 | `usearch` HNSW для dense vectors | usearch crate | ~150 |
| 4.3 | RaBitQ 1-bit quantization (32× compression) | pure Rust | ~700 |
| 4.4 | `next-plaid` для ColBERT multi-vector | next-plaid crate | ~200 |
| 4.5 | nomic-embed-text (Matryoshka dims 64-768) | fastembed-rs | ~100 |

**Критерий успеха:** 1B 768-d embeddings в 12GB RAM. Dense + ColBERT lanes готовы. `poler-engine ~/eteryya -q "Алексей" --semantic dense` работает.

### ПРИОРИТЕТ 5: Learned Sparse — SPLADE (1 неделя)

| Задача | Что | Зависимость | LOC |
|---|---|---|---|
| 5.1 | SPLADE ONNX inference через `ort` | ort crate | ~150 |
| 5.2 | Term-impacts в существующий inverted index | — | ~200 |
| 5.3 | SPLADE весовой fusion с BM25 | — | ~100 |

**Критерий успеха:** BM25 + SPLADE в одном inverted index. `poler-engine ~/eteryya -q "Алексей" --semantic sparse` работает.

### ПРИОРИТЕТ 6: IIR-Resonance Fusion — ★ УНИКАЛЬНАЯ ИННОВАЦИЯ (2 недели)

> Главная инновация poler-engine. Ни одна SOTA-система 2026 не делает online lane-weight learning через резонансное накопление.

| Задача | Что | LOC |
|---|---|---|
| 6.1 | IIR-resonance lane trust: R_lane_t = ε_lane_t + φ·R_{t-1} | ~300 |
| 6.2 | POLER[Ψ] как custom vector metric в usearch | ~200 |
| 6.3 | ε-density как IVF partitioning | ~150 |
| 6.4 | K-hop graph as PLAID pre-prune | ~200 |
| 6.5 | 4-lane fusion с online weights | ~100 |

**Алгоритм:**
```
Для каждого запроса q:
  Lane 1 (BM25+ε):     score_1 = BM25(d, q) + ε(d, q)
  Lane 2 (SPLADE):     score_2 = Σ learned_impact(t, d) for t in q
  Lane 3 (Dense):      score_3 = cos_sim(emb(d), emb(q))
  Lane 4 (ColBERT):    score_4 = MaxSim(emb_multi(d), emb_multi(q))

  Online lane trust (IIR-resonance):
    R_lane_t = ε_lane_t + φ · R_lane_{t-1}

  Final score = Σ_lane w_lane · score_lane
    Где w_lane = softmax(R_lane) — нормализованный резонанс
```

**Критерий успеха:** 4-lane fusion работает. Online веса адаптируются. `poler-engine ~/eteryya -q "Алексей" --semantic fused` работает. Benchmark vs single-lane показывает улучшение recall@10.

### ПРИОРИТЕТ 7: Code Intelligence (2-3 недели)

| Задача | Что | Зависимость | LOC |
|---|---|---|---|
| 7.1 | tree-sitter grammars (Rust/Python/JS/TS/Go/Java) | tree-sitter crates | ~200 |
| 7.2 | ast-grep structural search | ast-grep-core crate | ~300 |
| 7.3 | Salsa incremental computation для AIDDE | salsa crate | ~500 |
| 7.4 | Aider repomap: PageRank over symbol graph | — (уже есть PageRank) | ~300 |
| 7.5 | AIDDE-typed entity vectors | — | ~250 |

**Критерий успеха:** Code analysis на уровне rust-analyzer + Aider. Incremental updates через Salsa. `poler-engine ./src --grep "fn " --structural` работает.

### ПРИОРИТЕТ 8: Knowledge Graph Intelligence (2 недели)

| Задача | Что | Зависимость | LOC |
|---|---|---|---|
| 8.1 | GLiNER zero-shot NER через `gline-rs` | gline-rs, ort | ~200 |
| 8.2 | Leiden community detection | linfa-clustering | ~200 |
| 8.3 | NLI contradiction detection (temporal) | ort | ~200 |
| 8.4 | Incremental graph updates (dirty-flag communities) | — | ~150 |
| 8.5 | Edge provenance + schema ontology | — | ~100 |

**Критерий успеха:** Knowledge graph с community structure, contradiction detection. `poler-engine ~/eteryya -q "Алексей" --contradiction-check` работает.

### ПРИОРИТЕТ 9: Streaming Archives (2 недели)

| Задача | Что | Зависимость | LOC |
|---|---|---|---|
| 9.1 | zstd-seekable HTTP Range reader | zstd-framed | ~200 |
| 9.2 | TarEntryIndex (path → byte range) | tokio-tar | ~150 |
| 9.3 | WARC streaming (Common Crawl) | warc crate | ~200 |
| 9.4 | LazyDecompressMmap (userfaultfd + zstd) | — | ~300 |
| 9.5 | HuggingFace datasets streaming | arrow-rs | ~150 |

**Критерий успеха:** 100TB архивов в 16GB RAM. Zero-storage processing.

### ПРИОРИТЕТ 10: Agentic + MCP v2 (1 неделя)

| Задача | Что | Зависимость | LOC |
|---|---|---|---|
| 10.1 | MCP full-spec: resources + sampling + subscriptions | — | ~400 |
| 10.2 | WASM plugin sandbox (Wasmtime) | wasmtime | ~300 |
| 10.3 | ReAct/Reflexion agentic loop | — | ~200 |
| 10.4 | Streamable HTTP + OAuth 2.1 | — | ~150 |

**Критерий успеха:** LLM-агенты получают reactive substrate. Расширяемость через WASM.

### ПРИОРИТЕТ 11: Differential Dataflow (1 неделя)

| Задача | Что | LOC |
|---|---|---|
| 11.1 | Mini-DD (Z-sets, automatic IVM) | ~600 |
| 11.2 | Salsa × DD bridge | ~200 |
| 11.3 | Incremental graph refresh через DD | ~150 |

**Критерий успеха:** Любое изменение файла → автоматически пересчитываются только зависимые результаты.

---

## 4. ЧТО УКРАСТЬ (RUST CRATES — ЗАВИСИМОСТИ)

Добавить в `Cargo.toml` (по мере необходимости, НЕ все сразу):

```toml
[dependencies]
# Существующие (оставить)
clap = { version = "4.5", features = ["derive"] }
rayon = "1.10"
memmap2 = "0.9"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
petgraph = { version = "0.6", features = ["serde-1"] }
regex = "1.10"
aho-corasick = "1.1"
ignore = "0.4"
rusqlite = { version = "0.31", features = ["bundled"] }
memchr = "2"
ratatui = { version = "0.29", default-features = false, features = ["crossterm"] }
crossterm = "0.28"
rustyline = "14"
ureq = { version = "2.10", features = ["json"] }
gix = { version = "0.66", default-features = false, features = ["blocking-http-transport-reqwest", "blocking-network-client", "worktree-mutation", "revision", "comfort"] }
tui-textarea = "0.7"
globset = "0.4"
tempfile = "3.10"

# НОВЫЕ — Phase 1 (Foundation)
whatlang = "0.16"
rust-stemmers = "1.2"
unicode-segmentation = "1.11"

# НОВЫЕ — Phase 2 (Compression)
fsst = "0.1"            # или fsst-rs
lz4_flex = "0.11"
zstd = "0.13"

# НОВЫЕ — Phase 3 (Vector Layer)
fastembed = "4"         # BGE-M3, nomic-embed
ort = { version = "2", features = ["download-binaries"] }
usearch = "0.8"         # HNSW

# НОВЫЕ — Phase 4 (Learned Sparse) — ort уже добавлен

# НОВЫЕ — Phase 6 (Code Intelligence)
tree-sitter = "0.22"
tree-sitter-rust = "0.21"
tree-sitter-python = "0.21"
tree-sitter-javascript = "0.21"
tree-sitter-typescript = "0.21"
tree-sitter-go = "0.21"
tree-sitter-java = "0.21"
ast-grep-core = "0.3"
salsa = "0.18"          # incremental computation

# НОВЫЕ — Phase 7 (KG Intelligence)
linfa-clustering = "0.7"  # Leiden

# НОВЫЕ — Phase 8 (Streaming)
tokio-tar = "0.3"
warc = "0.6"
zeekstd = "0.4"         # zstd-seekable

# НОВЫЕ — Phase 9 (Agentic)
wasmtime = "20"         # WASM sandbox

# УДАЛИТЬ (google/nlm отвязка)
# ed25519-dalek = "2"   — license gate, не нужен без google
# arboard = "3"         — clipboard, опционально
```

---

## 5. ЧТО РАЗРАБОТАТЬ С НУЛЯ (УНИКАЛЬНОЕ — НЕТ АНАЛОГОВ)

> Эти 8 инноваций — главная ценность poler-engine v2.0. Ни одна SOTA-система 2026 их не имеет.

### 5.1. IIR-Resonance Online Lane-Trust Learning
- **Файл:** `src/fusion/iir_lanes.rs` (~300 LOC)
- **Суть:** `R_lane_t = ε_lane_t + φ · R_{t-1}` — онлайн доверие полосам поиска
- **Уникальность:** Ни одна SOTA paper не делает online lane-weight learning через resonant accumulation

### 5.2. POLER[Ψ] как Custom Vector Metric
- **Файл:** `src/fusion/psi_metric.rs` (~200 LOC)
- **Суть:** ψ-flow `p_{t+1} = p_t + η·Π_Λ(−∇F + γ∇ε)` как метрика в HNSW
- **Уникальность:** Ни один vector search engine не использует attention field с проектором логики

### 5.3. ε-Density as Free IVF Partitioning
- **Файл:** `src/fusion/epsilon_ivf.rs` (~150 LOC)
- **Суть:** Уже существующая ε-density → естественное разбиение для vector search без обучения

### 5.4. K-Hop Graph as PLAID Pre-Prune
- **Файл:** `src/fusion/graph_prefilter.rs` (~200 LOC)
- **Суть:** Граф сущностей как prefilter перед expensive ColBERT scoring

### 5.5. AIDDE-Typed Entity Vectors
- **Файл:** `src/code/typed_vectors.rs` (~250 LOC)
- **Суть:** Типизированные векторы символов (function/class/variable/module)

### 5.6. Temporal Contradiction Detection
- **Файл:** `src/graph/contradiction.rs` (~200 LOC)
- **Суть:** NLI cross-encoder + temporal layers → автообнаружение противоречий канона

### 5.7. LazyDecompressMmap
- **Файл:** `src/streaming/lazy_mmap.rs` (~300 LOC)
- **Суть:** userfaultfd + zstd-seekable → mmap поверх сжатых удалённых архивов

### 5.8. Salsa × Differential Dataflow Bridge
- **Файл:** `src/differential/salsa_bridge.rs` (~200 LOC)
- **Суть:** Incremental computation (Salsa) + math-correct increments (DD) — нет аналогов

---

## 6. ЦЕЛЕВЫЕ МЕТРИКИ

| Метрика | SOTA 2026 | poler v2.0 цель | Как |
|---|---|---|---|
| Vector search latency | 1-2 ms (ScaNN) | **<100 μs** | RaBitQ + HNSW + ε-IVF |
| RAM для 1B vectors | 400 GB (float32) | **12 GB** | RaBitQ 32× compression |
| Inverted index RAM | Tantivy: 2GB/65K files | **200 MB** | FSST 10× compression |
| Search recall@10 | BGE-M3: 0.92 | **0.95+** | 4-lane + IIR fusion |
| Entity extraction | GLiNER: 0.85 F1 | **0.90+** | GLiNER + graph context |
| Contradiction detection | NLI: 0.80 | **0.85+** | Temporal layers + NLI |
| Incremental update | Salsa: ms | **μs** | Salsa × DD bridge |
| Streaming throughput | 100 MB/s | **1 GB/s** | FSST + zstd-seekable |
| Archive processing | Download + unpack | **Zero-storage** | HTTP Range + lazy mmap |

---

## 7. ПРАВИЛА РАБОТЫ (НЕ НАРУШАТЬ)

### 7.1. Принцип «Инструмент, не ИИ»

- ✅ Индексировать, находить, отдавать, связывать по метаданным
- ❌ Понимать контент, генерировать текст, принимать решения
- ❌ Обучать модели (только инференс готовых ONNX/GGUF)
- ❌ Использовать GPU (только CPU, 16-32GB RAM)
- ❌ Использовать облачные API (суверенный стек)

### 7.2. Принцип «100% переписать»

- Все заимствованные решения (crates, алгоритмы) — дорабатываются и переписываются
- Не использовать как «чёрный ящик» — понимать, как работает, адаптировать под poler

### 7.3. Принцип «Банальность — это сила»

- grep банален, работает 50 лет. SQL банален. git банален.
- POLER — тот же уровень. Банальный доступ к данным. Для ИИ.
- Не «умный». Удобный.

### 7.4. Принцип «Не более»

> «Инструмент должен давать ИИ удобное взаимодействие с базой данных. Не более.»

- Не добавлять фичи «на вырост»
- Не усложнять без необходимости
- Каждая фича — закрывает конкретную боль

### 7.5. Принцип «No Google»

- Полная отвязка от Google/NotebookLM/Gmail/Drive/OAuth
- Суверенный стек — никаких внешних облачных API
- Веб-краулинг через CDP — ОК (это не Google)
- MCP-сервер — ОК (это не Google)

### 7.6. Принцип «Rust + CPU only»

- Только Rust. Никакого Python runtime.
- Только CPU. Никакого GPU.
- 16-32GB RAM target. Никаких серверных конфигураций.
- Инференс через ONNX Runtime (ort) или candle — НЕ Python.

### 7.7. Принцип «Каждый релиз — один кирпич»

- Каждый релиз закрывает ровно ОДНУ «хреновую» часть до конца
- С живым полевым тестом и регрессионным тестом
- Никаких зависимостей «на вырост» — только то, что работает в этот релиз

---

## 8. ПОРЯДОК ДЕЙСТВИЙ (ЧТО ДЕЛАТЬ ПРЯМО СЕЙЧАС)

### Шаг 1: Отвязка от Google (1-2 дня)

```bash
cd ~/my-project/skills/poler-engine

# 1. Прочитать PLAN_POLER_V2.md (Часть B — принцип «инструмент, не ИИ»)
# 2. Прочитать этот промпт полностью
# 3. Удалить src/google/
# 4. Удалить dev-stand/
# 5. Убрать google/nlm флаги из src/main.rs
# 6. Убрать `pub mod google;` из src/lib.rs
# 7. Убрать ed25519-dalek из Cargo.toml
# 8. cargo build --release -j1
# 9. poler-engine --version (должно работать)
# 10. poler-engine ~/eteryya -q "Алексей" (должно работать)
# 11. git commit -m "feat: remove google/nlm dependencies — sovereign stack"
# 12. git push
```

### Шаг 2: Foundation (1 неделя)

```bash
# 1. Добавить whatlang, rust-stemmers, unicode-segmentation в Cargo.toml
# 2. Интегрировать в src/tokenizer/mod.rs
# 3. Добавить madvise в src/streaming.rs (mmap)
# 4. Реализовать Teddy SIMD в src/retrieval/teddy.rs
# 5. Тесты: cargo test
# 6. Benchmark: poler-engine ~/eteryya -q "Алексей" --benchmark
# 7. git commit + push
```

### Шаг 3: Compression (1 неделя)

```bash
# 1. Добавить fsst, lz4_flex, zstd в Cargo.toml
# 2. Реализовать FSST layer в src/compression/fsst.rs
# 3. Применить к inverted index (src/tokenizer/inverted_index.rs)
# 4. Применить к doc store
# 5. Тесты + benchmark (RAM usage before/after)
# 6. git commit + push
```

### Шаг 4: Vector Layer (2-3 недели) — НАЧАТЬ РАНЬШЕ

```bash
# 1. Добавить fastembed, ort, usearch в Cargo.toml
# 2. Реализовать src/vectors/embeddings.rs (BGE-M3 через fastembed-rs)
# 3. Реализовать src/vectors/usearch_bridge.rs (HNSW)
# 4. Реализовать src/vectors/rabitq.rs (1-bit quantization — ~700 LOC)
# 5. Реализовать src/vectors/colbert.rs (next-plaid integration)
# 6. Добавить CLI флаг --semantic dense/sparse/colbert/fused
# 7. Тесты: embeddings генерируются, HNSW ищет, RaBitQ сжимает
# 8. git commit + push
```

### Шаг 5-11: Продолжать по приоритетам из §3

---

## 9. СТРУКТУРА ФАЙЛОВ v2.0 (ЦЕЛЕВАЯ)

```
src/
├── engine.rs              (существующий — refactor)
├── poler.rs               (существующий — refactor)
├── psi.rs                 (существующий — extend)
├── streaming.rs           (существующий — extend)
│
├── fusion/                ★ НОВЫЙ — 4-lane fusion
│   ├── mod.rs
│   ├── iir_lanes.rs       ★ UNIQUE — online lane trust
│   ├── psi_metric.rs      ★ UNIQUE — POLER[Ψ] as vector metric
│   ├── epsilon_ivf.rs     ★ UNIQUE — ε-density partitioning
│   ├── graph_prefilter.rs ★ UNIQUE — K-hop as PLAID pre-prune
│   └── tricolator.rs      (3-way fusion)
│
├── vectors/               ★ НОВЫЙ — vector layer
│   ├── mod.rs
│   ├── embeddings.rs      (fastembed-rs: BGE-M3, nomic)
│   ├── rabitq.rs          ★ UNIQUE — 1-bit quantization
│   ├── hnsw.rs            (poler-native HNSW)
│   ├── colbert.rs         (next-plaid integration)
│   └── usearch_bridge.rs  (usearch FFI)
│
├── sparse/                ★ НОВЫЙ — learned sparse
│   ├── mod.rs
│   ├── splade.rs          (ONNX inference)
│   └── term_impacts.rs    (в existing inverted index)
│
├── compression/           ★ НОВЫЙ — memory density
│   ├── mod.rs
│   ├── fsst.rs
│   ├── zstd_dict.rs
│   └── lz4_hot.rs
│
├── code/                  ★ НОВЫЙ — code intelligence
│   ├── mod.rs
│   ├── tree_sitter.rs
│   ├── ast_grep.rs
│   ├── salsa_queries.rs
│   ├── repomap.rs
│   └── typed_vectors.rs   ★ UNIQUE
│
├── graph/                 (существующий — extend)
│   ├── entity_graph.rs    (существующий)
│   ├── communities.rs     ★ НОВЫЙ — Leiden
│   ├── contradiction.rs   ★ НОВЫЙ — NLI temporal
│   ├── incremental.rs     ★ НОВЫЙ — dirty-flag
│   └── provenance.rs      ★ НОВЫЙ
│
├── ner/                   ★ НОВЫЙ — neural extraction
│   ├── mod.rs
│   ├── gliner.rs
│   └── rebel.rs
│
├── streaming_archives/    ★ НОВЫЙ — zero-storage
│   ├── mod.rs
│   ├── http_range.rs
│   ├── zstd_seekable.rs
│   ├── tar_index.rs
│   ├── warc.rs
│   ├── hf_datasets.rs
│   └── lazy_mmap.rs       ★ UNIQUE
│
├── agentic/               ★ НОВЫЙ — agentic substrate
│   ├── mod.rs
│   ├── react.rs
│   ├── reflexion.rs
│   ├── mcp_v2.rs
│   └── wasm_plugins.rs
│
├── differential/          ★ НОВЫЙ — incremental math
│   ├── mod.rs
│   ├── zsets.rs
│   ├── salsa_bridge.rs
│   └── incremental_graph.rs
│
├── retrieval/             (существующий — extend)
│   ├── chunk.rs
│   ├── grep.rs
│   ├── semantic_bridge.rs (extend с BGE-M3)
│   └── teddy.rs           ★ НОВЫЙ — SIMD
│
├── resonance/             (существующий — keep)
├── tokenizer/             (существующий — extend)
├── output/                (существующий — keep)
├── parser/                (существующий — extend с tree-sitter)
├── aidde/                 (существующий — extend с Salsa)
├── web/                   (существующий — keep, НЕ google)
├── vcs/                   (существующий — keep)
├── shell/                 (существующий — keep)
├── gateway/               (существующий — keep)
├── notes/                 (существующий — keep)
├── sources/               (существующий — keep)
├── license/               (существующий — keep или удалить)
└── bench/                 (существующий — keep)
```

---

## 10. АНТИ-ПАТТЕРНЫ (ЧЕГО НЕ ДЕЛАТЬ)

- ❌ НЕ использовать Python runtime (всё на Rust + ONNX)
- ❌ НЕ использовать GPU (только CPU)
- ❌ НЕ использовать облачные API (суверенный стек)
- ❌ НЕ использовать Google/NotebookLM/Gmail/Drive/OAuth
- ❌ НЕ использовать Qdrant/LanceDB как daemon (embed, не server)
- ❌ НЕ использовать FAISS (C++ FFI тяжёлая, usearch лучше)
- ❌ НЕ использовать LangChain/LlamaIndex (свой MCP)
- ❌ НЕ обучать модели с нуля (только инференс готовых ONNX/GGUF)
- ❌ НЕ добавлять зависимости «на вырост»
- ❌ НЕ «понимать» контент (инструмент отдаёт, ИИ понимает)
- ❌ НЕ генерировать текст (инструмент ищет, ИИ пишет)
- ❌ НЕ принимать решений (инструмент даёт данные, ИИ решает)

---

## 11. КЛЮЧЕВЫЕ ДОКУМЕНТЫ (СПИСОК)

Перед началом работы ПРОЧТИ:

1. **`PLAN_POLER_V2.md`** (1001 строка) — мастер-план (Части A-D)
2. **`docs/research/research_semantic_search.md`** (483 строки) — SOTA векторный поиск
3. **`docs/research/research_code_agentic.md`** (572 строки) — tree-sitter/Salsa/MCP/WASM
4. **`docs/research/research_streaming_nlp.md`** (793 строки) — FSST/zstd/RaBitQ/DBSP
5. **`docs/research/research_rag_kg.md`** (215 строк) — GraphRAG/GLiNER/contradictions
6. **`docs/research/dialogue_tool_vs_ai.md`** (1192 строки) — диалог «инструмент vs ИИ»
7. **`README.md`** — описание движка от автора
8. **`FUTURE_ROADMAP.md`** — roadmap автора
9. **`Cargo.toml`** — текущие зависимости
10. **`src/engine.rs`** — ядро движка
11. **`src/psi.rs`** — POLER[Ψ] математика

---

## 12. ФИНАЛЬНЫЙ ПРИНЦИП

> «Инструмент должен давать ИИ удобное взаимодействие с базой данных. Не более.
>
> Не понимать. Не анализировать. Не решать.
> Индексировать. Находить. Отдавать.
>
> ИИ — понимает. Автор — творит. Инструмент — служит.»

**Начинай с Шага 1: отвязка от Google. Потом — по приоритетам.**
