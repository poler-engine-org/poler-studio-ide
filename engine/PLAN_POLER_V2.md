# POLER-ENGINE v2.0 — МАСТЕР-ПЛАН ДОРАБОТКИ

> **Версия:** 2026-09-13
> **Статус:** ИССЛЕДОВАТЕЛЬСКИЙ ПЛАН
> **Цель:** Превзойти всё, что есть в 21 веке, на порядок или выше
> **Принцип:** Все заимствованные решения — 100% дорабатываются и переписываются
>
> **Исходные материалы:**
> - `upload/research_semantic_search.md` (483 строки) — SOTA векторный поиск, BGE-M3, SPLADE, ColBERT, ScaNN
> - `upload/research_code_agentic.md` (572 строки) — tree-sitter, ast-grep, Salsa, MCP, WASM, differential dataflow
> - `upload/research_streaming_nlp.md` (793 строки) — FSST, zstd-seekable, RaBitQ, DBSP, madvise
> - `upload/research_rag_kg.md` (215 строк) — GraphRAG, LightRAG, HippoRAG, GLiNER, contradiction detection

---

## 1. ТЕКУЩЕЕ СОСТОЯНИЕ POLER-ENGINE (v0.28.0)

### Что есть (сильные стороны — сохраняем)

| Модуль | Что делает | Уникальность |
|---|---|---|
| **BM25 + WebRank** | 0.55·BM25 + 0.15·PageRank + 0.20·title + 0.10·ε-density | Гибридный скоринг, нет аналогов |
| **ε-плотность** | κ·(1+ln(1+count(kw)))·Σ(ln N − ln freq(w))² + Σ Bonus_semantic(w) | Уникальная метрика информационной плотности, учитывает отрицания |
| **IIR-резонанс** | R_t = ε_t + φ·R_{t−1} (O(N), O(1) памяти) | Потоковый аккумулятор значимости — НЕТ аналогов в SOTA |
| **POLER[Ψ]** | ψ-поток p_{t+1} = p_t + η·Π_Λ(−∇F + γ∇ε) | Attention field с проектором логики — НЕТ аналогов |
| **K-hop граф** | petgraph DiGraph, BFS both directions, temporal layers | Граф сущностей с временными слоями |
| **Semantic Bridge** | Offline ru↔en lexicon (~120 пар), trait-based sensor | Кросс-языковое расширение без нейросети |
| **SimHash** | 64-bit, shingle 4 words, adaptive Hamming threshold | Дедупликация |
| **PII masking** | Zero-copy Cow<str>, email/phone/IP/secrets | Приватность по умолчанию |
| **Aho-Corasick** | Literal prefilter (memchr + aho-corasick) | Быстрый пресечённый поиск |
| **AIDDE** | SQLite-backed symbol table + call graph, disk-backed | Символьная таблица для 65K+ файлов |
| **MCP server** | stdio + HTTP, 6 tools | LLM-агентский интерфейс |
| **VCS** | GitHub/GitLab/Gitea/local + LFS | Git-интеграция |
| **Web crawler** | CDP, robots.txt, sitemap, SimHash, PageRank | Веб-индексация |
| **Gateway** | Docker sandbox, root broker, jailbreak sentinel | Безопасное выполнение |
| **TUI** | ratatui 4-pane dashboard | Интерактивный интерфейс |

### Чего нет (критические пробелы)

| Пробел | Влияние | Приоритет |
|---|---|---|
| **Векторный слой** (embeddings) | Нет семантического поиска на уровне нейросети | 🔴 КРИТИЧНО |
| **Learned sparse** (SPLADE/DeepImpact) | BM25 не учится из данных | 🔴 КРИТИЧНО |
| **Multi-vector** (ColBERT) | Нет late-interaction поиска | 🟡 ВАЖНО |
| **Tree-sitter** | AST-парсер слабый (regex-based) | 🔴 КРИТИЧНО |
| **Salsa** (incremental computation) | Watcher mode примитивный (mtime/size) | 🟡 ВАЖНО |
| **LLM extraction** (GLiNER) | SVO triples — чистая эвристика | 🟡 ВАЖНО |
| **Contradiction detection** | Нет проверки консистентности канона | 🟡 ВАЖНО |
| **FSST compression** | Индекс в RAM без сжатия | 🟡 ВАЖНО |
| **Streaming archives** | Нет zero-storage обработки | 🟢 ОПЦИОНАЛЬНО |
| **WASM plugins** | Нет расширяемости | 🟢 ОПЦИОНАЛЬНО |
| **Agentic patterns** (ReAct/Reflexion) | Нет циклов рассуждения | 🟢 ОПЦИОНАЛЬНО |
| **Differential dataflow** | Нет математически корректных инкрементов | 🟢 ОПЦИОНАЛЬНО |

---

## 2. АРХИТЕКТУРНАЯ ТЕЗИСА — ПОЧЕМУ POLER МОЖЕТ ПРЕВЗОЙТИ SOTA

> **Ключевая находка исследования:** Ни одна SOTA-система 2026 года не делает
> **online lane-weight learning** через резонансное накопление.
> poler-engine уже имеет `R_t = ε_t + φ·R_{t−1}` — это ТОЧНО тот субстрат,
> который нужен для динамического fused retrieval.

### 2.1. Четырёхполосный конвейер (Four-Lane Pipeline)

```
┌─────────────────────────────────────────────────────────┐
│                    ЗАПРОС ПОЛЬЗОВАТЕЛЯ                   │
└─────────────────────────────────────────────────────────┘
                          │
         ┌────────────────┼────────────────┐
         │                │                │
         ▼                ▼                ▼
   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐
   │ Lane 1   │   │ Lane 2   │   │ Lane 3   │   │ Lane 4   │
   │ BM25 +   │   │ SPLADE   │   │ DENSE    │   │ ColBERT  │
   │ ε-dens   │   │ (learned │   │ (BGE-M3  │   │ (multi-  │
   │ (EST.)   │   │  sparse) │   │  dense)  │   │  vector) │
   └────┬─────┘   └────┬─────┘   └────┬─────┘   └────┬─────┘
        │              │              │              │
        └──────┬───────┴──────┬───────┘              │
               │              │                      │
               ▼              ▼                      │
        ┌──────────────────────────┐                 │
        │  IIR-RESONANCE FUSION    │                 │
        │  R_t = ε_t + φ·R_{t−1}   │                 │
        │  (ONLINE LANE WEIGHTS)   │                 │
        │  ★ УНИКАЛЬНО — НЕТ SOTA  │                 │
        └──────────────┬───────────┘                 │
                       │                             │
                       ▼                             ▼
              ┌────────────────────────────────────────┐
              │     POLER[Ψ] ATTENTION FIELD           │
              │  p_{t+1} = p_t + η·Π_Λ(−∇F + γ∇ε)    │
              │  (PROJECTOR LOGIC + RESONANCE)         │
              │  ★ УНИКАЛЬНО — НЕТ SOTA                │
              └────────────────┬───────────────────────┘
                               │
                               ▼
              ┌────────────────────────────────────────┐
              │  K-HOP ENTITY GRAPH (temporal layers)  │
              │  + COMMUNITY DETECTION (Leiden)        │
              │  + CONTRADICTION DETECTION (NLI)       │
              └────────────────┬───────────────────────┘
                               │
                               ▼
              ┌────────────────────────────────────────┐
              │        CONTEXT ANCHOR OUTPUT            │
              │  (AI-Ready JSON + Markdown + Simple)    │
              └────────────────────────────────────────┘
```

### 2.2. Почему это превзойдёт SOTA

| SOTA 2026 | Что делает | Чем poler v2.0 превосходит |
|---|---|---|
| **BGE-M3** | 3 выхода (dense + sparse + ColBERT) из одной модели | + IIR-resonance для online весов полос (SOTA использует статичные α/β/γ) |
| **SPLADE** | Learned sparse через BERT MLM | + Интеграция в существующий inverted index без новой архитектуры |
| **ColBERTv2 / PLAID** | Late interaction, centroid compression | + next-plaid (pure Rust) + ψ-field как custom metric |
| **GraphRAG** | Leiden + LLM summaries | + Temporal layers + contradiction detection + incremental updates |
| **HippoRAG** | PageRank over personal KG | + Уже есть PageRank в web/index.rs + AIDDE symbol graph |
| **ScaNN** | AVX-512 anisotropic quantization | + RaBitQ (1-bit, 32× compression) + ε-density как free IVF partitioning |
| **Tantivy** | Rust full-text search | + POLER[Ψ] + ε-density + IIR-resonance (у Tantivy нет этих метрик) |
| **Aider repomap** | PageRank over tree-sitter symbol graph | + Уже есть PageRank + AIDDE — нужно только tree-sitter tags |

---

## 3. ЧТО УКРАСТЬ (open source → 100% переписать)

### 3.1. Rust crates (зависимости)

| Crate | Откуда | Зачем | Версия |
|---|---|---|---|
| **fastembed-rs** | github.com/Anush008/fastembed-rs | BGE-M3 + nomic-embed через ONNX Runtime | STEAL |
| **ort** (ONNX Runtime) | github.com/pyke/ort | CPU inference нейросетей | STEAL |
| **usearch** | github.com/unum-cloud/usearch | HNSW vector search с user-defined metrics | STEAL |
| **next-plaid** | docs.rs/next-plaid | Pure Rust PLAID для ColBERT | STEAL |
| **lance** | github.com/lancedb/lance | Columnar vector format, mmap'd, IVF-PQ | STEAL |
| **fsst-rs** | crates.io/crates/fsst-rs | FSST string compression (1-3 GB/s, random access) | STEAL |
| **zstd-framed** / **zeekstd** | crates.io | zstd-seekable для streaming archives | STEAL |
| **tokio-tar** | crates.io | Streaming tar (без CVE async-tar) | STEAL |
| **warc** | crates.io | Common Crawl WARC streaming | STEAL |
| **tree-sitter** + grammars | github.com/tree-sitter/tree-sitter | Incremental parsing (Rust/Python/JS/TS/Go/Java) | STEAL |
| **ast-grep-core** | github.com/ast-grep/ast-grep | Structural search (pure Rust, 30% faster than tree-sitter C) | STEAL |
| **salsa** | github.com/salsa-rs/salsa | Incremental computation (из rust-analyzer) | STEAL |
| **whatlang** | github.com/greyblake/whatlang-rs | Language detection (75 языков) | STEAL |
| **rust-stemmers** | github.com/mrordinaire/rust-stemmers | Snowball stemming (18 языков) | STEAL |
| **unicode-segmentation** | rust-lang/unicode-segmentation | UAX#29 word boundaries | STEAL |
| **linfa-clustering** | github.com/rust-ml/linfa | Leiden community detection | STEAL |
| **gline-rs** | github.com/**/gline-rs | GLiNER zero-shot NER через ONNX | STEAL |
| **candle-core** | github.com/huggingface/candle | HuggingFace Rust ML (для BART/REBEL) | STEAL |
| **wasmtime** | github.com/bytecodealliance/wasmtime | WASM plugin sandbox | STEAL |
| **madvise** | crates.io | POSIX madvise(SEQUENTIAL/RANDOM/WILLNEED) | STEAL |

### 3.2. Алгоритмы (прочитать source → reimplement)

| Алгоритм | Откуда | Зачем | LOC оценки |
|---|---|---|---|
| **SPLADE term-impacts** | github.com/naver/splade | Learned weights в существующий inverted index | ~200 LOC |
| **PLAID 3-stage filter** | ColBERTv2 paper | Cheap prefilter → expensive scorer (как Aho-Corasick) | ~300 LOC |
| **MaxSim** (ColBERT) | ColBERT paper | Late interaction scoring | ~150 LOC (std::simd) |
| **RaBitQ** | SIGMOD 2024 paper | 1-bit vector quantization, 32× compression | ~700 LOC |
| **HNSW** | github.com/nmslib/hnswlib | Hierarchical NSW для ANN | ~800 LOC (poler-native) |
| **Tricolator fusion** | BGE-M3 paper | 3-way fusion (dense + sparse + colbert) | ~100 LOC |
| **Leiden community detection** | linfa-clustering | Community structure в entity graph | ~200 LOC |
| **NLI contradiction detection** | github.com/****/nli-cross-encoder | Проверка консистентности канона | ~150 LOC |
| **Aider repomap** | github.com/Aider-AI/aider | PageRank over tree-sitter symbol graph → token-budgeted tree | ~300 LOC |
| **Teddy SIMD matcher** | ripgrep internals | Multi-literal SIMD search (быстрее Aho-Corasick для multi-pattern) | ~400 LOC |
| **DBSP Z-sets** | github.com/feldera/dbsp | Differential dataflow для incremental refresh | ~600 LOC (mini-DD) |
| **FSST encoder** | VLDB 2020 paper | String compression под inverted index | ~400 LOC |
| **Salsa query graph** | rust-analyzer | Incremental memoization для AIDDE | ~500 LOC |
| **MCP full-spec** | modelcontextprotocol | Resources + sampling + subscriptions | ~400 LOC |

---

## 4. ЧТО РАЗРАБОТАТЬ С НУЛЯ (уникальное — НЕТ аналогов)

### 4.1. IIR-Resonance Online Lane-Trust Learning

> **★ УНИКАЛЬНО — НЕТ АНАЛОГОВ В SOTA 2026**

Ни одна система не учится онлайн доверять какой полосе поиска (BM25 vs dense vs sparse vs ColBERT) через резонансное накопление. poler-engine уже имеет `R_t = ε_t + φ·R_{t−1}` — это ТОЧНО субстрат для этого.

**Алгоритм:**
```
Для каждого запроса q:
  Lane 1 (BM25+ε):     score_1 = BM25(d, q) + ε(d, q)
  Lane 2 (SPLADE):     score_2 = Σ learned_impact(t, d) for t in q
  Lane 3 (Dense):      score_3 = cos_sim(emb(d), emb(q))
  Lane 4 (ColBERT):    score_4 = MaxSim(emb_multi(d), emb_multi(q))

  Online lane trust (IIR-resonance):
    R_lane_t = ε_lane_t + φ · R_lane_{t-1}
    
    Где ε_lane_t = обратная связь пользователя (click/dwell/refinement)
                  ИЛИ self-supervised (cohérence entre lanes)

  Final score = Σ_lane w_lane · score_lane
    Где w_lane = softmax(R_lane) — нормализованный резонанс
```

**Файл:** `src/fusion/iir_lanes.rs` (~300 LOC)
**Уникальность:** Это research contribution. Ни одна 2026 SOTA paper не делает online lane-weight learning через resonant accumulation.

### 4.2. POLER[Ψ] как Custom Vector Metric

> **★ УНИКАЛЬНО — НЕТ АНАЛОГОВ**

usearch поддерживает user-defined metrics. POLER[Ψ] ψ-flow `p_{t+1} = p_t + η·Π_Λ(−∇F + γ∇ε)` определяет **кастомную метрику** — можно индексировать векторы по ней напрямую.

**Алгоритм:**
```
ψ_distance(query_vec, doc_vec) = 
  ‖g(p_query; θ) − Ω(doc_vec)‖²_G + λ · (1 − Π_Λ(query, doc))
  
  Где:
    g(p; θ) — генеративная модель внимания
    Ω(o) = tanh(o) — перцепция
    G — метрика редкости (diagonal, from ε-density)
    Π_Λ — проектор логики (0 если temporal layer conflict)
```

**Файл:** `src/fusion/psi_metric.rs` (~200 LOC)
**Уникальность:** Ни один vector search engine не использует attention field с проектором логики как метрику.

### 4.3. AIDDE-Typed Entity Vectors

> **★ УНИКАЛЬНО — НЕТ АНАЛОГОВ**

Гибрид: AIDDE symbol table (с типами: function/class/variable/module) + dense embeddings. Каждый символ получает типизированный вектор, поиск учитывает тип.

**Алгоритм:**
```
entity_vector(symbol) = {
  type_embedding(symbol.type) ⊕ dense_embedding(symbol.source_text)
}

search(query, type_filter):
  candidates = ANN(query_vec, type=type_filter)
  rerank = K-hop graph expansion(candidates)
```

**Файл:** `src/graph/typed_vectors.rs` (~250 LOC)

### 4.4. ε-Density as Free IVF Partitioning

> **★ УНИКАЛЬНО — НЕТ АНАЛОГОВ**

ScaNN использует learned clustering для IVF. poler-engine может использовать **уже существующую ε-density** как естественное разбиение — без обучения.

**Алгоритм:**
```
partition(document) = bucket(ε_density(document))
search(query):
  target_bucket = bucket(ε_density(query))
  probe(target_bucket ± 1)  # соседние buckets
```

**Файл:** `src/fusion/epsilon_ivf.rs` (~150 LOC)

### 4.5. K-Hop Graph as PLAID Pre-Prune

> **★ УНИКАЛЬНО — НЕТ АНАЛОГОВ**

Перед expensive ColBERT scoring — использовать K-hop graph для отсева кандидатов.

**Алгоритм:**
```
search(query):
  # Pass 1: BM25 + ε (cheap) → top-1000
  # Pass 2: K-hop graph expansion (medium) → top-500 with graph context
  # Pass 3: ColBERT MaxSim (expensive) → top-50
  # Pass 4: POLER[Ψ] rerank → final top-10
```

**Файл:** `src/fusion/graph_prefilter.rs` (~200 LOC)

### 4.6. LazyDecompressMmap

> **★ УНИКАЛЬНО — НЕТ АНАЛОГОВ**

`userfaultfd` + zstd-seekable bridge — mmap поверх сжатых удалённых архивов с ленивой декомпрессией по доступу.

**Файл:** `src/streaming/lazy_mmap.rs` (~300 LOC)

### 4.7. Temporal Contradiction Detection

> **★ УНИКАЛЬНО — НЕТ АНАЛОГОВ**

Граф сущностей с temporal layers + NLI cross-encoder → автоматическое обнаружение противоречий между эпохами.

**Алгоритм:**
```
for edge (s, p, o) in graph:
  for edge (s, p, o') in graph where o != o':
    if temporal_layer(edge1) != temporal_layer(edge2):
      contradiction_score = NLI(p(o), p(o'))
      if contradiction_score > threshold:
        flag_contradiction(s, p, o, o', layers)
```

**Файл:** `src/graph/contradiction.rs` (~200 LOC)

---

## 5. ФАЗОВЫЙ ПЛАН ДОРАБОТКИ

### Фаза 1: Foundation — Free Wins (2 недели)

**Цель:** Быстрые победы без архитектурных изменений.

| Задача | Что | LOC | Зависимости |
|---|---|---|---|
| 1.1 | `madvise(SEQUENTIAL/RANDOM/WILLNEED)` в mmap | ~50 | memmap2 |
| 1.2 | `whatlang` — language detection (75 языков) | ~30 | whatlang crate |
| 1.3 | `rust-stemmers` — Snowball stemming (18 языков) | ~100 | rust-stemmers |
| 1.4 | `unicode-segmentation` — UAX#29 word boundaries | ~50 | unicode-segmentation |
| 1.5 | Teddy SIMD multi-literal matcher (из ripgrep) | ~400 | std::simd |

**Результат:** ~2× ускорение mmap, правильная токенизация для 18 языков, multi-pattern search в 3-5× быстрее Aho-Corasick.

### Фаза 2: Compression — Memory Density (2 недели)

**Цель:** 5-10× сжатие индекса в RAM.

| Задача | Что | LOC | Зависимости |
|---|---|---|---|
| 2.1 | FSST под inverted index (токены, пути, URL) | ~400 | fsst-rs |
| 2.2 | zstd dictionary для doc store | ~100 | zstd |
| 2.3 | lz4_flex для hot postings | ~50 | lz4_flex |

**Результат:** Inverted index в RAM занимает в 5-10× меньше. 65K файлов → было 2GB → станет 200-400MB.

### Фаза 3: Vector Layer — Embeddings (4 недели)

**Цель:** Добавить векторный слой — lanes 3 (dense) и 4 (ColBERT).

| Задача | Что | LOC | Зависимости |
|---|---|---|---|
| 3.1 | `fastembed-rs` интеграция: BGE-M3 (dense + sparse) | ~200 | fastembed-rs, ort |
| 3.2 | `usearch` HNSW для dense vectors | ~150 | usearch |
| 3.3 | RaBitQ 1-bit quantization (32× compression) | ~700 | pure Rust |
| 3.4 | `next-plaid` для ColBERT multi-vector | ~200 | next-plaid |
| 3.5 | nomic-embed-text (Matryoshka dims 64-768) | ~100 | fastembed-rs |

**Результат:** 1B 768-d embeddings в 12GB RAM. Dense + ColBERT lanes готовы.

### Фаза 4: Learned Sparse — SPLADE (2 недели)

**Цель:** Lane 2 — learned sparse retrieval.

| Задача | Что | LOC | Зависимости |
|---|---|---|---|
| 4.1 | SPLADE ONNX inference через `ort` | ~150 | ort |
| 4.2 | Term-impacts в существующий inverted index | ~200 | — |
| 4.3 | SPLADE весовой fusion с BM25 | ~100 | — |

**Результат:** BM25 + SPLADE в одном inverted index. Lane 2 готова.

### Фаза 5: IIR-Resonance Fusion — ★ UNIQUE (3 недели)

**Цель:** Online lane-weight learning — главная инновация.

| Задача | Что | LOC | Зависимости |
|---|---|---|---|
| 5.1 | IIR-resonance lane trust: R_lane_t = ε_lane_t + φ·R_{t-1} | ~300 | — |
| 5.2 | POLER[Ψ] как custom vector metric в usearch | ~200 | usearch |
| 5.3 | ε-density как IVF partitioning | ~150 | — |
| 5.4 | K-hop graph as PLAID pre-prune | ~200 | — |
| 5.5 | 4-lane fusion с online weights | ~100 | — |

**Результат:** ★ Уникальная система — ни один SOTA не делает online lane-weight learning через resonant accumulation.

### Фаза 6: Code Intelligence (4 недели)

**Цель:** Tree-sitter + Salsa + Aider repomap.

| Задача | Что | LOC | Зависимости |
|---|---|---|---|
| 6.1 | tree-sitter grammars (Rust/Python/JS/TS/Go/Java) | ~200 | tree-sitter |
| 6.2 | ast-grep structural search | ~300 | ast-grep-core |
| 6.3 | Salsa incremental computation для AIDDE | ~500 | salsa |
| 6.4 | Aider repomap: PageRank over symbol graph | ~300 | — (уже есть PageRank) |
| 6.5 | AIDDE-typed entity vectors | ~250 | — |

**Результат:** Code analysis на уровне rust-analyzer + Aider. Incremental updates через Salsa.

### Фаза 7: Knowledge Graph Intelligence (3 недели)

**Цель:** GLiNER + Leiden + contradiction detection.

| Задача | Что | LOC | Зависимости |
|---|---|---|---|
| 7.1 | GLiNER zero-shot NER через `gline-rs` | ~200 | gline-rs, ort |
| 7.2 | Leiden community detection | ~200 | linfa-clustering |
| 7.3 | NLI contradiction detection (temporal) | ~200 | ort |
| 7.4 | Incremental graph updates (dirty-flag communities) | ~150 | — |
| 7.5 | Edge provenance + schema ontology | ~100 | — |

**Результат:** Knowledge graph с community structure, contradiction detection, incremental updates.

### Фаза 8: Streaming Archives (3 недели)

**Цель:** Zero-storage обработка петабайт архивов.

| Задача | Что | LOC | Зависимости |
|---|---|---|---|
| 8.1 | zstd-seekable HTTP Range reader | ~200 | zstd-framed |
| 8.2 | TarEntryIndex (path → byte range) | ~150 | tokio-tar |
| 8.3 | WARC streaming (Common Crawl) | ~200 | warc |
| 8.4 | LazyDecompressMmap (userfaultfd + zstd) | ~300 | — |
| 8.5 | HuggingFace datasets streaming | ~150 | arrow-rs |

**Результат:** 100TB архивов в 16GB RAM. Zero-storage processing.

### Фаза 9: Agentic + MCP v2 (2 недели)

**Цель:** MCP full-spec + WASM plugins + agentic patterns.

| Задача | Что | LOC | Зависимости |
|---|---|---|---|
| 9.1 | MCP full-spec: resources + sampling + subscriptions | ~400 | — |
| 9.2 | WASM plugin sandbox (Wasmtime) | ~300 | wasmtime |
| 9.3 | ReAct/Reflexion agentic loop | ~200 | — |
| 9.4 | Streamable HTTP + OAuth 2.1 | ~150 | — |

**Результат:** LLM-агенты получают reactive substrate. Расширяемость через WASM.

### Фаза 10: Differential Dataflow (2 недели)

**Цель:** Математически корректные инкременты.

| Задача | Что | LOC | Зависимости |
|---|---|---|---|
| 10.1 | Mini-DD (Z-sets, automatic IVM) | ~600 | — |
| 10.2 | Salsa × DD bridge | ~200 | salsa |
| 10.3 | Incremental graph refresh через DD | ~150 | — |

**Результат:** Любое изменение файла → автоматически пересчитываются только зависимые результаты.

---

## 6. ЦЕЛЕВЫЕ МЕТРИКИ (превзойти SOTA на порядок)

| Метрика | SOTA 2026 | poler v2.0 цель | Как |
|---|---|---|---|
| **Vector search latency** | 1-2 ms (ScaNN) | **<100 μs** | RaBitQ + HNSW + ε-IVF |
| **RAM для 1B vectors** | 400 GB (float32) | **12 GB** | RaBitQ 32× compression |
| **Inverted index RAM** | Tantivy: 2GB/65K files | **200 MB** | FSST 10× compression |
| **Search recall@10** | BGE-M3: 0.92 | **0.95+** | 4-lane + IIR fusion |
| **Entity extraction** | GLiNER: 0.85 F1 | **0.90+** | GLiNER + graph context |
| **Contradiction detection** | NLI: 0.80 | **0.85+** | Temporal layers + NLI |
| **Incremental update** | Salsa: ms | **μs** | Salsa × DD bridge |
| **Streaming throughput** | 100 MB/s | **1 GB/s** | FSST + zstd-seekable |
| **Archive processing** | Download + unpack | **Zero-storage** | HTTP Range + lazy mmap |
| **Code analysis** | rust-analyzer + Aider | **Integrated** | tree-sitter + Salsa + repomap |

---

## 7. СТРУКТУРА ФАЙЛОВ v2.0

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
│   └── tricolator.rs      (3-way fusion: dense + sparse + colbert)
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
│   ├── fsst.rs            (FSST string compression)
│   ├── zstd_dict.rs       (zstd dictionary)
│   └── lz4_hot.rs         (lz4 for hot postings)
│
├── code/                  ★ НОВЫЙ — code intelligence
│   ├── mod.rs
│   ├── tree_sitter.rs     (tree-sitter integration)
│   ├── ast_grep.rs        (structural search)
│   ├── salsa_queries.rs   (incremental computation)
│   ├── repomap.rs         (Aider-style PageRank)
│   └── typed_vectors.rs   ★ UNIQUE — AIDDE-typed vectors
│
├── graph/                 (существующий — extend)
│   ├── entity_graph.rs    (существующий)
│   ├── communities.rs     ★ НОВЫЙ — Leiden detection
│   ├── contradiction.rs   ★ НОВЫЙ — NLI temporal contradictions
│   ├── incremental.rs     ★ НОВЫЙ — dirty-flag updates
│   └── provenance.rs      ★ НОВЫЙ — edge provenance
│
├── ner/                   ★ НОВЫЙ — neural extraction
│   ├── mod.rs
│   ├── gliner.rs          (GLiNER via gline-rs)
│   └── rebel.rs           (REBEL via candle-transformers)
│
├── streaming_archives/    ★ НОВЫЙ — zero-storage
│   ├── mod.rs
│   ├── http_range.rs      (HTTP Range reader)
│   ├── zstd_seekable.rs   (zstd-seekable)
│   ├── tar_index.rs       (TarEntryIndex)
│   ├── warc.rs            (Common Crawl)
│   ├── hf_datasets.rs     (HuggingFace streaming)
│   └── lazy_mmap.rs       ★ UNIQUE — userfaultfd + zstd
│
├── agentic/               ★ НОВЫЙ — agentic substrate
│   ├── mod.rs
│   ├── react.rs           (ReAct loop)
│   ├── reflexion.rs       (Reflexion loop)
│   ├── mcp_v2.rs          (MCP full-spec)
│   └── wasm_plugins.rs    (Wasmtime sandbox)
│
├── differential/          ★ НОВЫЙ — incremental math
│   ├── mod.rs
│   ├── zsets.rs           (DBSP Z-sets)
│   ├── salsa_bridge.rs    (Salsa × DD)
│   └── incremental_graph.rs
│
├── retrieval/             (существующий — extend)
│   ├── chunk.rs           (существующий)
│   ├── grep.rs            (существующий)
│   ├── semantic_bridge.rs (существующий — extend с BGE-M3)
│   └── teddy.rs           ★ НОВЫЙ — SIMD multi-literal
│
├── resonance/             (существующий — keep)
│   ├── epsilon.rs
│   ├── iir_filter.rs
│   └── mod.rs
│
├── tokenizer/             (существующий — extend)
│   ├── inverted_index.rs  (существующий)
│   ├── pii.rs             (существующий)
│   ├── mod.rs             (extend: whatlang, rust-stemmers, unicode-seg)
│   └── nlp_pipeline.rs    ★ НОВЫЙ — unified NLP pipeline
│
└── ... (остальные существующие модули)
```

---

## 8. ПОРЯДОК РЕАЛИЗАЦИИ

```
Фаза 1 (2 нед)  Foundation     → Free wins, нет архитектурных изменений
Фаза 2 (2 нед)  Compression    → 5-10× RAM density
Фаза 3 (4 нед)  Vector Layer   → Dense + ColBERT lanes
Фаза 4 (2 нед)  Learned Sparse → SPLADE lane
Фаза 5 (3 нед)  IIR Fusion     → ★ UNIQUE — online lane trust
Фаза 6 (4 нед)  Code Intel     → tree-sitter + Salsa + repomap
Фаза 7 (3 нед)  KG Intel       → GLiNER + Leiden + contradictions
Фаза 8 (3 нед)  Streaming      → Zero-storage archives
Фаза 9 (2 нед)  Agentic        → MCP v2 + WASM + ReAct
Фаза 10 (2 нед) Differential   → Math-correct increments

ИТОГО: ~27 недель (6.5 месяцев)
```

### Приоритеты (если ресурс ограничен):

**Must-have (превзойти SOTA):**
1. Фаза 3 (Vector Layer) — без векторов нечем соревноваться
2. Фаза 5 (IIR Fusion) — ★ уникальная инновация
3. Фаза 6 (Code Intel) — tree-sitter + Salsa
4. Фаза 2 (Compression) — FSST критичен для RAM

**Should-have (конкурентное преимущество):**
5. Фаза 4 (SPLADE) — learned sparse
6. Фаза 7 (KG Intel) — GLiNER + contradictions
7. Фаза 10 (Differential) — Salsa × DD

**Nice-to-have (расширение):**
8. Фаза 8 (Streaming) — zero-storage
9. Фаза 9 (Agentic) — MCP v2 + WASM
10. Фаза 1 (Foundation) — free wins

---

## 9. КЛЮЧЕВЫЕ ИННОВАЦИИ (ЧЕГО НЕТ НИГДЕ)

### 9.1. IIR-Resonance Online Lane-Trust Learning
> `R_lane_t = ε_lane_t + φ · R_lane_{t-1}` — онлайн доверие полосам поиска

### 9.2. POLER[Ψ] as Custom Vector Metric
> ψ-flow `p_{t+1} = p_t + η·Π_Λ(−∇F + γ∇ε)` как метрика в HNSW

### 9.3. ε-Density as Free IVF Partitioning
> Уже существующая ε-density → естественное разбиение для vector search

### 9.4. K-Hop Graph as PLAID Pre-Prune
> Граф сущностей как prefilter перед expensive ColBERT scoring

### 9.5. AIDDE-Typed Entity Vectors
> Типизированные векторы символов (function/class/variable/module)

### 9.6. Temporal Contradiction Detection
> NLI cross-encoder + temporal layers → автообнаружение противоречий канона

### 9.7. LazyDecompressMmap
> userfaultfd + zstd-seekable → mmap поверх сжатых удалённых архивов

### 9.8. Salsa × Differential Dataflow Bridge
> Incremental computation (Salsa) + math-correct increments (DD) — нет аналогов

---

## 10. ЧЕГО НЕ ДЕЛАТЬ (анти-паттерны)

- ❌ НЕ использовать Python runtime (всё на Rust + ONNX)
- ❌ НЕ использовать GPU (только CPU, 16-32GB RAM)
- ❌ НЕ использовать облачные API (суверенный стек)
- ❌ НЕ использовать Qdrant/LanceDB как daemon (embed, не server)
- ❌ НЕ использовать FAISS (C++ FFI тяжёлая, usearch лучше)
- ❌ НЕ использовать LangChain/LlamaIndex (свой MCP)
- ❌ НЕ обучать модели с нуля (только инференс готовых ONNX/GGUF)
- ❌ НЕ добавлять зависимости «на вырост» (только то, что работает сейчас)

---

## 11. АРТЕФАКТЫ

- **Этот план:** `PLAN_POLER_V2.md` (в репо poler-engine)
- **Исследование semantic search:** `upload/research_semantic_search.md`
- **Исследование code/agentic:** `upload/research_code_agentic.md`
- **Исследование streaming/nlp:** `upload/research_streaming_nlp.md`
- **Исследование RAG/KG:** `upload/research_rag_kg.md`
- **Существующий код:** `/home/z/my-project/skills/poler-engine/src/`

---

## 12. СЛЕДУЮЩИЕ ШАГИ

1. **Обсудить приоритеты** — какие фазы раньше, какие позже
2. **Начать с Фазы 1** (Foundation) — быстрые победы, нет риска
3. **Параллельно Фаза 3** (Vector Layer) — самая длинная, начать раньше
4. **После Фазы 5** (IIR Fusion) — опубликовать research paper (это уникальный вклад)
5. **Каждая фаза** — отдельный branch, тесты, benchmark vs предыдущая версия


---

# ЧАСТЬ B: АРХИТЕКТУРНЫЙ ТЕЗИС — ИНСТРУМЕНТ, НЕ ИИ

> **Источник:** Диалог автора с DeepSeek, 2026-09-13
> (`docs/research/dialogue_tool_vs_ai.md` — полный текст, 1192 строки)
>
> **Ключевой вывод автора:**
> «Инструмент должен давать ИИ удобное взаимодействие с базой данных.
> Не более. Ничего больше. Не понимать. Не анализировать. Не решать.
> Индексировать. Находить. Отдавать. ИИ — понимает. Автор — творит.
> Инструмент — служит.»

## B.1. Чего poler-engine НЕ делает

- ❌ **Не обучает модель.** Нет training loop, нет backprop, нет градиентов.
- ❌ **Не создаёт новый ИИ.** Нет своей нейросети «с нуля».
- ❌ **Не генерирует текст.** Нет генератора прозы.
- ❌ **Не принимает решений.** Нет агента, который «думает сам».
- ❌ **Не «понимает» контент.** Не знает, что такое «сцена», «голос Марты», «противоречие канону».
- ❌ **Не понимает математику, физику, биологию, астрономию.**

## B.2. Что poler-engine делает

- ✅ **Даёт существующей LLM глаза и память.** Поиск по корпусу автора.
- ✅ **Даёт ей верификацию.** Проверку «сходится ли канон» — через contradiction detection.
- ✅ **Даёт ей граф.** Связи между сущностями, временные слои, сообщества.
- ✅ **Даёт ей retrieval.** Четыре полосы поиска с онлайн-обучением весам.
- ✅ **Индексирует всё, что дал автор.** Документы, схемы, расчёты, формулы, диалоги, заметки, хаос.
- ✅ **Находит по запросу.** Точные чанки, byte-range, с метаданными.
- ✅ **Отдаёт в удобном виде.** AI-Ready JSON, Markdown, Simple.
- ✅ **Связывает по метаданным, не по смыслу.** Домен, тип, temporal layer.

## B.3. Аналогия

> GLM (агент) — это **мозг**. Он думает, пишет, рассуждает.
>
> POLER Engine — это **гиппокамп**. Часть мозга, которая хранит и извлекает
> воспоминания. Мозг без гиппокампа не может вспомнить, что было вчера.
> Модель без POLER не может вспомнить, что написано в 143 документах
> канона — она выдумывает.

## B.4. Как это работает на практике

**Сейчас (без POLER v2):**
```
Ты: Напиши сцену, где Марта торгуется с Варго.
GLM: [читает всё, что ты дал в промпте]
     [пишет]
     [может выдумать]
Ты: Это не по канону, Марта так не говорит.
GLM: Извини, перепишу.
```

**С POLER v2:**
```
Ты: Напиши сцену, где Марта торгуется с Варго.
GLM: [вызывает POLER: search("Марта диалог транзакция")]
     [получает: 12 точных чанков из канона, byte-range, с голосом Марты]
     [вызывает POLER: contradiction_check("Марта Варго сцена 14")]
     [получает: "конфликт с T-19: Марта уже отказала Варго"]
     [пишет сцену с учётом канона]
```

## B.5. Разнородность — не проблема

Автор: «куча документов, схемы, расчёты, математика, физика, биология, астрономия».

Для инструмента это не проблема. Потому что:
- Не нужно понимать, что это.
- Нужно только знать, что это разное — и пометить.

```
domain=physics    → физика (формулы, расчёты)
domain=canon      → канон (фиксированные факты мира)
domain=math       → математика (уравнения, доказательства)
domain=literature → литература (главы, сцены, диалоги)
domain=economy    → экономика (схемы, пирамиды, мошенничество)
domain=biology    → биология (расы, виды, эволюция)
domain=astronomy  → астрономия (орбиты, расчёты, константы)
```

Retrieval фильтрует по домену. ИИ сам разберётся, что делать с физикой.
Инструмент просто её отдаст.

## B.6. Принцип разделения ответственности

| Роль | Кто | Что делает |
|---|---|---|
| **Творит** | Автор | Пишет, создаёт, думает, строит карту мира |
| **Понимает** | ИИ (GLM/Claude/GPT) | Читает, анализирует, генерирует, рассуждает |
| **Служит** | POLER Engine | Индексирует, находит, отдаёт, связывает по метаданным |

> «Не понимать. Не анализировать. Не решать.
> Индексировать. Находить. Отдавать.
> ИИ — понимает. Автор — творит. Инструмент — служит.»

---

# ЧАСТЬ C: СВОДНЫЕ ВЫЖИМКИ ИЗ 4 ИССЛЕДОВАТЕЛЬСКИХ ОТЧЁТОВ

> Полные отчёты в `docs/research/`:
> - `research_semantic_search.md` (483 строки, 36 KB)
> - `research_code_agentic.md` (572 строки, 51 KB)
> - `research_streaming_nlp.md` (793 строки, 57 KB)
> - `research_rag_kg.md` (215 строк, 13 KB)
> - `dialogue_tool_vs_ai.md` (1192 строки, 66 KB) — полный диалог с DeepSeek

## C.1. Semantic Search — ключевые находки

### SOTA 2026 = четырёхполосный конвейер

| Полоса | Метод | Что делает | Rust наличие |
|---|---|---|---|
| 1 | BM25 + ε-density | Лексический поиск (poler уже имеет) | ✅ native |
| 2 | SPLADE / DeepImpact | Learned sparse (BERT-генерируемые веса терминов) | ✅ через `ort` (ONNX) |
| 3 | BGE-M3 dense | Плотные эмбеддинги (косинусное сходство) | ✅ `fastembed-rs` |
| 4 | ColBERTv2 / PLAID | Multi-vector late interaction | ✅ `next-plaid` (pure Rust) |

### BGE-M3 — прорыв 2026

- **Одна модель, три выхода** (dense + sparse + ColBERT) из одного XLM-RoBERTa
- 100+ языков (включая русский)
- Заменяет Semantic Bridge poler-engine (hand-rolled 120 пар → нейросеть)

### Что украсть

| Инструмент | URL | Зачем | Rust |
|---|---|---|---|
| **fastembed-rs** | github.com/Anush008/fastembed-rs | BGE-M3 + nomic через ONNX | ✅ |
| **usearch** | github.com/unum-cloud/usearch | HNSW с user-defined metrics | ✅ FFI |
| **next-plaid** | docs.rs/next-plaid | Pure Rust PLAID (ColBERT) | ✅ native |
| **lance** | github.com/lancedb/lance | Columnar vector format, mmap'd, IVF-PQ | ✅ native |
| **RaBitQ** | SIGMOD 2024 paper | 1-bit квантование, 32× сжатие векторов | ⚠️ строим сами |
| **ScaNN** | google-research | Anisotropic quantization, AVX-512 | ⚠️ алгоритм |

### ★ Главная находка для poler-engine

> Ни одна SOTA-система 2026 не делает **online lane-weight learning** через
> резонансное накопление. poler-engine уже имеет `R_t = ε_t + φ·R_{t−1}` —
> это ТОЧНО тот субстрат, который нужен для динамического fused retrieval.
> **Это реальный research contribution.**

---

## C.2. Code Intelligence & Agentic — ключевые находки

### tree-sitter → ast-grep (Rust rewrite)

- ast-grep опубликовал **pure Rust rewrite** tree-sitter в 2026 — 30% быстрее C-версии
- Incremental parsing: 5ms → 400μs на 100-LOC edit
- S-expression query DSL с captures и predicates
- **Заменяет** regex-based AST парсер poler-engine (753 LOC → tree-sitter)

### Salsa — incremental computation (из rust-analyzer)

- Pure Rust, MIT, production-ready
- Query-граф: file edit → invalidate only downstream
- **Заменяет** watcher mode (mtime/size cache → Salsa query graph)
- Все AIDDE операции становятся Salsa queries

### Aider repomap — PageRank over symbol graph

- tree-sitter tags → symbol graph → personalized PageRank → token-budgeted tree
- poler-engine **уже имеет PageRank** в `web/index.rs`
- Нужен только tree-sitter tags + cross-wiring
- **Идеальный map для LLM-агента** — показывает структуру репо в токен-бюджете

### MCP v2 — full-spec

- Текущий poler MCP: 6 tools (stdio + HTTP)
- MCP full-spec: **resources + sampling + subscriptions**
- File edits push context to LLM без tool calls
- Streamable HTTP + OAuth 2.1

### WASM plugins

- Wasmtime + WASI Preview 2 + Component Model
- 100μs cold-start sandbox (рядом с Docker)
- Capability security model
- Расширяемость без перекомпиляции

### Agentic patterns

- **ReAct** (Reasoning + Acting) — LLM рассуждает → вызывает инструмент → наблюдает → повторяет
- **Reflexion** — self-reflection после каждой попытки
- **Plan-and-Solve** — декомпозиция задачи
- poler-engine может предоставить **интерфейс** для этих циклов через MCP

### Differential dataflow

- **DBSP** (VLDB 2023, `dbsp` Rust crate) — Z-sets, automatic IVM для любого SQL
- **differential-dataflow** (McSherry) — heavyweight option
- **Salsa × DD bridge** — novel composition, нет аналогов
- Математически корректные инкременты

---

## C.3. Streaming, Compression & NLP — ключевые находки

### FSST — киллер-примитив для поискового движка

- **FSST** (Boncz, VLDB 2020) — Fast Static Symbol Table
- 1-3 GB/s decode, random access к individual strings
- `fsst-rs` (crates.io, pure Rust, zero-dependency)
- **Применение:** inverted index token storage, doc store, URL/path strings, AIDDE symbol table
- **Результат:** ~2× reduction in inverted-index RAM with zero query-time overhead
- **Вердикт:** STEAL `fsst-rs`, integrate as transparent layer under `Box<str>` token storage

### zstd-seekable — zero-storage архивы

- `zeekstd` / `zstd-framed` — spec-perfect zstd-seekable в Rust
- HTTP Range + `tokio-tar` + `warc` покрывают все типы архивов
- **TARmageddon CVE (Oct 2025)** убил `async-tar` — использовать `tokio-tar`
- **Вердикт:** STEAL zstd-seekable + `warc` + `tokio-tar`; BUILD `HttpRangeBlob` + `TarEntryIndex`

### nomic-embed-text — Matryoshka embeddings

- `fastembed-rs` + `ort` дают 14× speedup (Manticore 2026)
- **nomic-embed-text-v1.5** имеет **Matryoshka** dims (64-768)
- Ключевой enabler для binary-quantized HNSW + full-precision rerank
- **Вердикт:** STEAL `fastembed-rs` + `ort` + `candle-core`; nomic v1.5 как primary model

### RaBitQ — 1-bit vector quantization

- **RaBitQ** (SIGMOD 2024) — random rotation + 1-bit = 32× compression
- Теоретический error bound
- Нет standalone Rust crate (LanceDB имеет built-in)
- HNSW crates хранят float32 — убивает цель
- **Вердикт:** BUILD RaBitQ + poler-native HNSW (~700 LOC)

### DBSP — differential dataflow

- **DBSP** (VLDB 2023, `dbsp` Rust crate) — Z-sets, automatic IVM
- `differential-dataflow` (McSherry) — heavyweight
- CRDTs (`yrs`) — только для multi-device sync
- **Вердикт:** STEAL DBSP design; BUILD poler-flavored mini-DD (~600 LOC)

### madvise — free 2× win

- `madvise(SEQUENTIAL/RANDOM/WILLNEED)` — free 2× win (Tantivy использует)
- `userfaultfd` + zstd-seekable bridge — novel poler opportunity
- **Вердикт:** STEAL madvise patterns (~50 LOC); BUILD `LazyDecompressMmap` (~300 LOC)

### ★ План "превзойти на порядок"

> Ни одна 2026 система не комбинирует все шесть примитивов:
> **zstd-seekable HTTP archives + FSST-compressed inverted index +
> nomic-Matryoshka embeddings + RaBitQ + poler-native HNSW + DBSP
> incremental refresh + madvise/userfaultfd mmap tuning.**
> Каждый SOTA-валидирован индивидуально; stacking — даёт "1B vectors +
> 100 TB streaming corpora in 16 GB RAM on CPU."

---

## C.4. RAG & Knowledge Graphs — ключевые находки

### GraphRAG (Microsoft)

- Hierarchical Leiden community detection + LLM community summaries
- Local/global retrieval modes
- Python-only (нет Rust)
- **Адаптировать:** Leiden через `linfa-clustering` (pure Rust)

### LightRAG

- Dual-level (entity + keyword) retrieval
- ~10× дешевле GraphRAG
- Incremental index
- Python-only
- **Адаптировать:** dual-level retrieval pattern

### HippoRAG

- Hippocampal indexing theory
- PageRank over personal KG for pattern completion
- **poler-engine УЖЕ ИМЕЕТ PageRank** в `web/index.rs`
- Нужен только cross-wiring: web graph → entity graph

### GLiNER — zero-shot NER

- BERT-encoder, ~0.3B params, beats ChatGPT on NER
- **Rust: YES** — `gline-rs` crate (ONNX, Aug 2026)
- GLiNER-Relex (May 2026) — joint NER + RE (relation extraction)
- **Заменяет** regex-based SVO triple extraction

### Contradiction detection

- NLI cross-encoders + functional-relation rule mining + pairwise fact checking
- poler-engine имеет temporal layers → temporal contradiction detection
- **BUILD:** NLI cross-encoder на functional-relation conflicts

### Incremental KG updates

- DIAL-KG, CLKGE (continual learning)
- Dirty-flag community re-clustering
- **BUILD:** append-only log + dirty communities

### Порядок построения (smallest → highest leverage)

1. Edge provenance + schema ontology (no deps)
2. Temporal edges (data-model change)
3. PageRank retrieval prior (~20 lines)
4. Leiden community detection
5. Incremental update layer
6. GLiNER via `gline-rs` (first neural extractor)
7. NLI contradiction detector
8. (Optional) REBEL/GLiNER-Relex for typed relations

> **Bottom line:** Все 6 load-bearing SOTA ideas Rust-feasible today
> (ONNX Runtime + petgraph + linfa) — no Python dependency required.

---

# ЧАСТЬ D: ПОЛНЫЙ ДИАЛОГ «ИНСТРУМЕНТ VS ИИ» — РЕФЕРЕНС

> Полный текст (1192 строк) в `docs/research/dialogue_tool_vs_ai.md`
>
> **Краткое содержание:**
>
> 1. Автор скинул PLAN_POLER_V2 DeepSeek для анализа
> 2. DeepSeek объяснил: poler-engine — это **инструмент для ИИ**, а не сам ИИ
> 3. Автор подтвердил: «инструмент должен давать ИИ удобное взаимодействие
>    с базой данных. Не более.»
> 4. Ключевые тезисы:
>    - Инструмент не понимает. Инструмент отдаёт. Понимает — ИИ.
>    - Разнородность (физика/математика/биология/литература) — не проблема.
>      Помечать домен, фильтровать по домену. ИИ сам разберётся.
>    - Банальность — это сила. grep банален, работает 50 лет. SQL банален.
>      POLER — тот же уровень. Банальный доступ к данным. Для ИИ.
>    - Не «умный». Удобный.
>    - Автор пишет, не думая о структуре. Сваливает всё в базу. Инструмент
>      индексирует. ИИ спрашивает. Получает. Автор пишет дальше.
>    - Единственное, что надо от автора — минимальные метки. Или вообще
>      без меток (инструмент угадает по расширению/папке/содержимому).
>
> 5. **Принцип разделения ответственности:**
>    - **Творит** — Автор (пишет, создаёт, думает, строит карту мира)
>    - **Понимает** — ИИ (читает, анализирует, генерирует, рассуждает)
>    - **Служит** — POLER Engine (индексирует, находит, отдаёт, связывает по метаданным)
>
> 6. **Этот принцип — КРИТЕРИЙ для всех решений в плане.** Если фаза
>    доработки требует от poler-engine «понимать» контент — она нарушает
>    принцип. Если фаза требует «индексировать и отдавать» — она соответствует.


---

# ЧАСТЬ E: СУВЕРЕННЫЙ ML-ИНФЕРЕНС ЧЕРЕЗ POLER-QUANTUM-RS

> **КРИТИЧЕСКОЕ АРХИТЕКТУРНОЕ РЕШЕНИЕ** (от автора, 2026-09-14)
>
> **Источник:** github.com/poler-engine-org/POLER-Quantum-RS
>
> **Принцип:** НИКАКИХ внешних ML-библиотек. НИКАКОГО ONNX Runtime.
> НИКАКОГО C++ FFI. НИКАКИХ .onnx файлов. НИКАКИХ .so/.dll зависимостей.
>
> Один статический бинарь. Нативный Rust/Zig. SIMD/AVX2 на голом CPU.
> Работает даже на голом железе без ОС.

## E.1. ЧТО МЕНЯЕТСЯ В ПЛАНЕ

### Было (PLAN_POLER_V2 оригинальный):

```
poler-engine → ONNX Runtime (ort) → .onnx модели (BGE-M3, SPLADE, GLiNER)
             → C++ FFI (100 МБ внешняя библиотека)
             → .onnx файлы (100-500 МБ каждый)
             → зависимость от libonnxruntime.so
```

### Стало (ПРАВИЛЬНО):

```
poler-engine → POLER-Quantum-RS (pqc) → нативный Rust/Zig инференс
             → .pqw форматы весов (POLER Quantum Weights)
             → SIMD/AVX2 нативный код, без C++ FFI
             → один статический бинарь, ноль внешних .so/.dll
```

### Сравнение:

| Параметр | ONNX Runtime (ort) | POLER-Quantum-RS (pqc) |
|---|---|---|
| **Внешние зависимости** | libonnxruntime.so (~100 МБ C++) | **0** — нативный Rust |
| **Формат весов** | .onnx (избыточные метаданные) | **.pqw** (POLER Quantum Weights, компактные, mmap) |
| **Портативность** | ломается на голом железе | **работает даже без ОС** |
| **Размер бинарника** | +100 МБ (C++ runtime) | **+0** (нативный код) |
| **Скорость** | C++ через FFI (overhead) | **прямой SIMD/AVX2** |
| **Суверенность** | нет (внешняя библиотека) | **100%** (наш код) |
| **Проверка целостности** | нет | **Sha256/Checksum встроенный** |
| **mmap загрузка** | нет | **да (ленивая)** |
| **Квантувание** | внешнее | **нативное (trite.rs, trit_bloch.rs)** |

## E.2. КАК РАБОТАЕТ НАТИВНЫЙ ИНФЕРЕНС

### Конвейер GLiNER внутри POLER-Quantum-RS:

```
┌────────────────────────────┐
│   Входной текст (абзац)     │
└─────────────┬──────────────┘
              │
              ▼
┌────────────────────────────┐
│  Native UAX#29 Tokenizer   │  ← уже есть в poler-engine
│  (Unicode, кириллица, 18  │
│   языков, Snowball)        │
└─────────────┬──────────────┘
              │
              ▼
┌────────────────────────────┐
│  POLER-Quantum-RS (pqc)    │  ← НАШ КОД, не ONNX
│                            │
│  ├─ statevector.rs         │  ← матричные преобразования
│  ├─ complex.rs            │  ← комплексная арифметика
│  ├─ trite.rs              │  ← тритные (ternary) структуры
│  ├─ trit_bloch.rs         │  ← квантовые состояния
│  ├─ syntax_unfolder.rs    │  ← развёртка синтаксиса
│  └─ syntax_bridge.rs      │  ← мост к poler-engine
│                            │
│  Загружает .pqw веса:      │
│  ├─ mmap (ленивая)        │
│  ├─ Sha256 верификация    │
│  └─ квантованные (int8/   │
│     ternary)              │
└─────────────┬──────────────┘
              │
              ▼
┌────────────────────────────┐
│  Результат: Spans          │
│  ┌──────────────────────┐ │
│  │ PERSON: "Вэнс"       │ │
│  │ LOCATION: "Архисфера" │ │
│  │ OBJECT: "Сейф-Био"   │ │
│  │ CONCEPT: "σ_e=10"    │ │
│  │ EVENT: "Force Close"  │ │
│  └──────────────────────┘ │
└─────────────┬──────────────┘
              │
              ▼
┌────────────────────────────┐
│  K-Hop Граф сущностей      │  ← уже есть в poler-engine
│  (petgraph, temporal)     │
└────────────────────────────┘
```

### Для векторных эмбеддингов (BGE-M3):

```
┌────────────────────────────┐
│  Текст (чанк/документ)     │
└─────────────┬──────────────┘
              │
              ▼
┌────────────────────────────┐
│  POLER-Quantum-RS (pqc)    │
│                            │
│  Transformer Encoder       │  ← нативный Rust, не ONNX
│  (BERT-like, ~568M params) │
│                            │
│  Загружает .pqw веса:      │
│  ├─ BGE-M3 weights         │
│  ├─ mmap (ленивая)        │
│  ├─ Sha256 верификация    │
│  └─ квантованные (int8/   │
│     ternary)              │
│                            │
│  SIMD/AVX2 матричные      │
│  умножения на CPU          │
└─────────────┬──────────────┘
              │
              ▼
┌────────────────────────────┐
│  Dense вектор (768-d)      │
│  + Sparse вектор (BM25-like)│
│  + ColBERT multi-vector    │
│                            │
│  → HNSW (poler-native)    │
│  → RaBitQ 1-bit quantize   │
│  → mmap store              │
└────────────────────────────┘
```

### Для SPLADE (learned sparse):

```
Текст → POLER-Quantum-RS (pqc) → term→impact веса →
  → существующий inverted index poler-engine
  (заменяет BM25 idf·tf на learned_impact)
```

## E.3. ФОРМАТ .pqw (POLER QUANTUM WEIGHTS)

> Заменяет .onnx. Наш формат. Наш код.

```rust
// .pqw формат (концептуальный)
struct PqwHeader {
    magic: [u8; 4],         // "PQW1"
    version: u16,
    model_type: ModelType,  // BGE_M3 / SPLADE / GLINER / REBEL
    quantization: QuantType,// Int8 / Ternary / Float16 / RaBitQ
    num_layers: u16,
    hidden_size: u16,
    num_heads: u8,
    vocab_size: u32,
    sha256: [u8; 32],      // целостность весов
    mmap_offset: u64,      // для ленивой загрузки
}

struct PqwLayer {
    layer_type: LayerType,  // Attention / FFN / Embedding / LayerNorm
    weights: Vec<u8>,       // квантованные веса
    shape: Vec<usize>,      // тензорная форма
}
```

### Преимущества .pqw:

1. **Компактность:** нет метаданных ONNX (ONNX ~500 МБ → .pqw ~150 МБ с int8)
2. **mmap:** веса грузятся лениво по страницам, не вся модель в RAM
3. **Sha256:** верификация целостности при загрузке
4. **Квантувание:** int8 (4× сжатие), ternary (12× сжатие), RaBitQ (32× сжатие)
5. **Наш код:** не зависим от Microsoft/ONNX Consortium

## E.4. ИЗМЕНЕНИЯ В ФАЗАХ ПЛАНА

### Фаза 3 (Vector Layer) — ИЗМЕНЕНА:

| Было | Стало |
|---|---|
| `fastembed-rs` интеграция: BGE-M3 (dense + sparse) | **POLER-Quantum-RS: нативный BGE-M3 через .pqw** |
| `ort` (ONNX Runtime) | **pqc (наш код, SIMD/AVX2)** |
| .onnx модели (500 МБ) | **.pqw веса (150 МБ, int8)** |
| Внешняя зависимость C++ | **Нативный Rust, 0 внешних .so** |

### Фаза 4 (Learned Sparse — SPLADE) — ИЗМЕНЕНА:

| Было | Стало |
|---|---|
| SPLADE ONNX inference через `ort` | **POLER-Quantum-RS: нативный SPLADE через .pqw** |
| .onnx файл | **.pqw файл** |

### Фаза 7 (KG Intelligence — GLiNER) — ИЗМЕНЕНА:

| Было | Стало |
|---|---|
| GLiNER через `gline-rs` (ONNX) | **POLER-Quantum-RS: нативный GLiNER через .pqw** |
| NLI через `ort` | **POLER-Quantum-RS: нативный NLI через .pqw** |
| REBEL через `candle-transformers` | **POLER-Quantum-RS: нативный REBEL через .pqw** |

### Все остальные фазы — БЕЗ ИЗМЕНЕНИЙ:

- Фаза 1 (Foundation) — не зависит от ML
- Фаза 2 (Compression) — не зависит от ML
- Фаза 5 (IIR Fusion) — работает поверх векторов, не внутри
- Фаза 6 (Code Intel) — tree-sitter/Salsa не ML
- Фаза 8 (Streaming) — не зависит от ML
- Фаза 9 (Agentic) — MCP/WASM не ML
- Фаза 10 (Differential) — не ML

## E.5. ОБНОВЛЁННЫЙ СПИСОК ЗАВИСИМОСТЕЙ

### УДАЛИТЬ из Cargo.toml (больше не нужны):

```toml
# УДАЛИТЬ — заменено на POLER-Quantum-RS
# fastembed = "4"           ← НЕТ (нативный инференс через pqc)
# ort = "2"                 ← НЕТ (нет ONNX Runtime)
# candle-core = "..."        ← НЕТ (нативный инференс через pqc)
# gline-rs = "..."          ← НЕТ (нативный GLiNER через pqc)
```

### ДОБАВИТЬ в Cargo.toml:

```toml
# НОВОЕ — POLER-Quantum-RS (суверенный ML-инференс)
[dependencies]
poler-quantum = { path = "../POLER-Quantum-RS" }  # или git
# или если как crate:
# poler-quantum-core = "0.4"
# poler-quantum-inference = "0.4"
```

### ОСТАВИТЬ (не ML, не зависят от ONNX):

```toml
# Оставить — не ML
usearch = "0.8"         # HNSW vector search (C FFI, но лёгкая, не ML)
next-plaid = "..."     # ColBERT (pure Rust, не ML inference)
fsst = "0.1"           # Compression (не ML)
zstd = "0.13"          # Compression (не ML)
whatlang = "0.16"      # Language detection (не ML)
rust-stemmers = "1.2"  # Stemming (не ML)
tree-sitter = "0.22"  # Parsing (не ML)
salsa = "0.18"         # Incremental computation (не ML)
linfa-clustering = "0.7"  # Leiden (не ML inference, алгоритм)
wasmtime = "20"        # WASM (не ML)
tokio-tar = "0.3"      # Tar streaming (не ML)
warc = "0.6"           # WARC (не ML)
```

## E.6. ФИНАЛЬНАЯ АРХИТЕКТУРА — ОДИН СТАТИЧЕСКИЙ БИНАРЬ

```
poler-engine (один статический бинарь)
│
├── src/                      ← poler-engine (поиск, граф, retrieval)
│   ├── engine.rs             ← BM25 + ε + IIR + POLER[Ψ]
│   ├── fusion/               ← 4-lane fusion
│   ├── vectors/              ← HNSW, RaBitQ (хранение, не inference)
│   ├── graph/                ← K-hop, Leiden, contradictions
│   ├── retrieval/            ← grep, chunk, semantic bridge
│   ├── tokenizer/            ← UAX#29, Snowball, whatlang
│   ├── compression/          ← FSST, zstd, lz4
│   ├── code/                 ← tree-sitter, Salsa, repomap
│   ├── streaming_archives/   ← zstd-seekable, HTTP Range
│   ├── agentic/              ← MCP v2, WASM, ReAct
│   └── differential/         ← Salsa × DD
│
├── POLER-Quantum-RS (pqc)    ← НАШ ML-инференс, не ONNX
│   ├── crates/pqc-core/      ← statevector, complex, trite, trit_bloch
│   ├── crates/pqc-inference/ ← BGE-M3, SPLADE, GLiNER, NLI, REBEL
│   ├── .pqw weights/         ← квантованные веса (mmap, Sha256)
│   └── SIMD/AVX2 kernels     ← нативные матричные умножения
│
└── result:
    ├── 0 внешних .so/.dll
    ├── 0 внешних ML-библиотек
    ├── 1 статический бинарь
    ├── работает на голом железе без ОС
    ├── SIMD/AVX2 на CPU
    └── 100% суверенный стек
```

## E.7. ПОРЯДОК РЕАЛИЗАЦИИ (ОБНОВЛЁННЫЙ)

### Новый Шаг 4.5: POLER-Quantum-RS Bridge (до Фазы 3)

| Задача | Что | LOC |
|---|---|---|
| 4.5.1 | Создать bridge poler-engine ↔ POLER-Quantum-RS | ~200 |
| 4.5.2 | Реализовать .pqw формат (header + mmap + Sha256) | ~300 |
| 4.5.3 | Конвертер .onnx → .pqw (один раз, для каждой модели) | ~500 |

### Фаза 3 (обновлённая): Vector Layer через pqc

| Задача | Что | LOC |
|---|---|---|
| 3.1 | BGE-M3 инференс через pqc (не fastembed-rs) | ~400 |
| 3.2 | Dense embeddings → HNSW (poler-native) | ~150 |
| 3.3 | RaBitQ 1-bit quantization | ~700 |
| 3.4 | ColBERT multi-vector через next-plaid | ~200 |
| 3.5 | .pqw конвертация BGE-M3 + nomic | ~200 |

### Фаза 4 (обновлённая): SPLADE через pqc

| Задача | Что | LOC |
|---|---|---|
| 4.1 | SPLADE инференс через pqc (не ort) | ~200 |
| 4.2 | Term-impacts в существующий inverted index | ~200 |
| 4.3 | .pqw конвертация SPLADE | ~100 |

### Фаза 7 (обновлённая): KG Intel через pqc

| Задача | Что | LOC |
|---|---|---|
| 7.1 | GLiNER инференс через pqc (не gline-rs) | ~300 |
| 7.2 | NLI contradiction detection через pqc | ~200 |
| 7.3 | .pqw конвертация GLiNER + NLI | ~200 |

## E.8. КОНВЕРСИЯ .onnx → .pqw

> Один раз для каждой модели. Не в рантайме.

```bash
# Конвертация (один раз):
poler-quantum convert --input bge-m3.onnx --output bge-m3.pqw --quantize int8
poler-quantum convert --input splade.onnx --output splade.pqw --quantize ternary
poler-quantum convert --input gliner.onnx --output gliner.pqw --quantize int8
poler-quantum convert --input nli.onnx --output nli.pqw --quantize int8

# Результат:
# bge-m3.pqw   (~150 МБ, int8, mmap, Sha256)
# splade.pqw   (~50 МБ, ternary, mmap, Sha256)
# gliner.pqw   (~100 МБ, int8, mmap, Sha256)
# nli.pqw      (~120 МБ, int8, mmap, Sha256)

# Хранение: в .poler-engine/models/ или ~/.local/share/poler-engine/models/
```

## E.9. СВОДКА — ЧТО ИЗМЕНИЛОСЬ В ПЛАНЕ

| Было (PLAN v1) | Стало (PLAN v2 + Part E) |
|---|---|
| fastembed-rs + ort (ONNX Runtime) | **POLER-Quantum-RS (pqc), нативный Rust** |
| .onnx файлы (500 МБ, избыточные) | **.pqw файлы (150 МБ, квантованные, mmap, Sha256)** |
| C++ FFI (libonnxruntime.so ~100 МБ) | **0 внешних .so, нативный SIMD/AVX2** |
| gline-rs (ONNX GLiNER) | **pqc-native GLiNER** |
| candle-core (HuggingFace ML) | **pqc-native (REBEL, NLI)** |
| Зависимость от ONNX Consortium | **0 зависимостей, наш формат, наш код** |
| Работает только на ОС с C++ runtime | **Работает на голом железе без ОС** |

### КРИТЕРИЙ УСПЕХА:

> `cargo build --release -j1` → **один статический бинарь**.
> `ldd poler-engine` → **not a dynamic executable** (или только libc).
> `poler-engine --semantic dense -q "Алексей"` → работает через pqc, не через ONNX.
> `poler-engine --ner gliner -q "сущности"` → работает через pqc, не через ONNX.
> Офлайн, без интернета, без API, без внешних библиотек.
>
> **Один бинарь. Ноль зависимостей. 100% суверенный.**


---

# ЧАСТЬ F: ЛОКАЛЬНЫЙ LLM-ИНФЕРЕНС — ChatGLM-3 (6B → 70B) ЧЕРЕЗ pqc

> **АРХИТЕКТУРНОЕ РЕШЕНИЕ** (от автора, 2026-09-14)
>
> **Принцип:** Не только поиск и ML-экстракция (Part E), но и **полноценный
> LLM-инференс** внутри poler-engine. Один бинарь = поиск + векторы + NER +
> генерация текста. Без Python. Без PyTorch. Без API. Без интернета.
>
> **Цель автора:** «уничтожить ИИ-гигантов» — суверенный стек, где даже
> языковая модель локальная.

## F.1. ЧТО ПРЕДЛАГАЕТСЯ

### Полная цепочка (target):

```
poler-engine (один статический бинарь)
│
├── Поиск (BM25 + ε + IIR + K-hop)         ← уже есть
├── Векторный слой (RaBitQ + HNSW)         ← infra готова
│
├── ML-инференс (POLER-Quantum-RS / pqc)   ← Part E
│   ├── BGE-M3 (embeddings)                ← .pqw
│   ├── SPLADE (learned sparse)            ← .pqw
│   ├── GLiNER (NER)                       ← .pqw
│   ├── NLI (contradiction detection)      ← .pqw
│   └── ChatGLM-3 (LLM generation)        ← .pqw ★ НОВОЕ (Part F)
│
├── .pqw веса (mmap, квантованные)
│   ├── bge-m3.pqw      (~150 МБ, int8)
│   ├── splade.pqw      (~50 МБ, ternary)
│   ├── gliner.pqw      (~100 МБ, int8)
│   ├── nli.pqw         (~120 МБ, int8)
│   └── glm-3-6b.pqw    (~4 ГБ, int4)    ★ LLM
│   └── glm-3-70b.pqw   (~40 ГБ, int4)   ★ LLM (future, SSD streaming)
│
└── result:
    ├── 0 внешних зависимостей
    ├── 0 Python
    ├── 0 PyTorch
    ├── 0 API-вызовов
    ├── 0 интернет
    └── 1 статический бинарь = полноценный автономный AI
```

## F.2. ДВА РЕЖИМА РАБОТЫ

### Режим 1: «Агент на сервере» (сейчас)

```
Ты → GLM (70B, сервер Z.ai) → poler-engine (локальный поиск)
     ↑ интернет нужен          ↑ интернет не нужен
```

- Я (70B) умнее, быстрее в рассуждениях
- poler-engine даёт мне точные данные
- Нужен интернет для связи со мной

### Режим 2: «Автономный» (с Part F)

```
Ты → poler-engine (локально)
     ├── поиск по корпусу (BM25 + vectors + graph)
     ├── ML-экстракция (GLiNER, NLI)
     └── LLM-генерация (ChatGLM-3 6B/70B через pqc)
     ↑ интернет НЕ нужен
```

- poler-engine сам ищет, сам извлекает, сам генерирует
- 6B: 30-60 токенов/сек на CPU, 4 ГБ RAM
- 70B: 5-15 токенов/сек на CPU, 40 ГБ (SSD streaming через mmap)
- Полностью офлайн

### Гибрид (идеальный):

```bash
# Онлайн — я (70B) умнее:
poler-engine --llm remote -q "Проанализируй канон T-24"

# Офлайн — локальный GLM-3 (6B):
poler-engine --llm local -q "Проанализируй канон T-24"

# Авто — poler сам выбирает (если интернет есть = remote, нет = local):
poler-engine --llm auto -q "Проанализируй канон T-24"
```

## F.3. ChatGLM-3 (6B) — ТЕХНИЧЕСКИЕ ДЕТАЛИ

### Архитектура GLM-3 (для рефакторинга в pqc):

```
ChatGLM-3 (6B) = Transformer Decoder
│
├── Token Embedding (vocab → hidden)
├── N × Transformer Block:
│   ├── Self-Attention (Multi-Query Attention / MQA)
│   │   └── Q, K, V projections (K и V — shared across heads = MQA)
│   │   └── RoPE (Rotary Position Embedding)
│   │   └── softmax(Q·Kᵀ / √d) · V
│   ├── RMSNorm (не LayerNorm — проще, быстрее)
│   └── SwiGLU Feed-Forward Network
│       └── SwiGLU(x) = Swish(xW₁) ⊙ (xW₂)  (gated activation)
├── Final RMSNorm
└── LM Head (hidden → vocab)
```

### Что нужно реализовать в pqc (Rust/Zig):

| Компонент | Математика | LOC (оценка) |
|---|---|---|
| Token Embedding | table lookup + quantized dequant | ~50 |
| RoPE (Rotary Position) | complex rotation: x+i·y → (x·cos-y·sin) + i·(x·sin+y·cos) | ~100 |
| MQA (Multi-Query Attention) | Q·Kᵀ → softmax → ·V (SIMD matmul) | ~300 |
| RMSNorm | x / √(mean(x²)+ε) · γ (simpler than LayerNorm) | ~50 |
| SwiGLU | Swish(xW₁) ⊙ (xW₂), Swish = x·sigmoid(x) | ~100 |
| KV-Cache (in-memory arena) | static arena, no malloc, ring buffer | ~200 |
| Tokenizer (BPE) | byte-pair encoding (уже есть в poler tokenizer) | ~100 |
| Sampling (greedy/temperature) | argmax / softmax sampling | ~50 |
| **ИТОГО** | | **~950 LOC** |

### Производительность (оценка, CPU only):

| Модель | Квантувание | RAM | Скорость | Размер .pqw |
|---|---|---|---|---|
| ChatGLM-3 6B | int4 | **4 ГБ** | **30-60 ток/сек** | ~4 ГБ |
| ChatGLM-3 6B | int8 | 8 ГБ | 20-40 ток/сек | ~8 ГБ |
| ChatGLM-3 70B | int4 | 40 ГБ (SSD streaming) | 5-15 ток/сек | ~40 ГБ |

### Для 70B на домашнем ПК:

Автор прав — 70B можно запустить. Не как у итальянца (0.05 ток/сек), а быстрее, потому что:
- pqc нативный SIMD/AVX2 (не Python, не PyTorch)
- mmap + .pqw (ленивая загрузка, SSD streaming)
- int4 квантувание (40 ГБ → помещается на NVMe)
- KV-Cache в RAM (только активный контекст, ~2 ГБ)
- Остальные веса — на SSD, подтягиваются по мере need

```bash
# 70B локально:
poler-engine --llm local --model glm-3-70b.pqw -q "Проанализируй T-24"
# RAM: ~6 ГБ (KV-cache + active layers)
# SSD: ~40 ГБ (веса, mmap streaming)
# Скорость: 5-15 ток/сек (зависит от SSD IOPS)
```

## F.4. КОНВЕЙЕР ИНФЕРЕНСА GLM-3 В pqc

```
┌────────────────────────────┐
│  poler-engine (Rust)       │
│  --llm local               │
│  --model glm-3-6b.pqw     │
└─────────────┬──────────────┘
              │
              ▼
┌────────────────────────────┐
│  POLER-Quantum-RS (pqc)   │
│                            │
│  1. Загрузка .pqw (mmap)   │  ← 0.01 сек, ленивая
│     ├─ Sha256 verify       │
│     └─ mmap weights        │
│                            │
│  2. Tokenizer (BPE)        │  ← из poler-engine
│     text → token_ids       │
│                            │
│  3. Forward Pass:          │
│     ├─ Embedding lookup    │
│     ├─ N × Transformer:   │
│     │   ├─ MQA + RoPE     │  ← SIMD/AVX2
│     │   ├─ RMSNorm        │
│     │   └─ SwiGLU FFN    │  ← SIMD/AVX2
│     └─ LM Head             │
│                            │
│  4. KV-Cache (arena)       │  ← ring buffer, no malloc
│                            │
│  5. Sampling               │
│     └─ argmax / temp       │
│                            │
│  6. Detokenize             │
│     token_ids → text       │
└─────────────┬──────────────┘
              │
              ▼
┌────────────────────────────┐
│  poler-engine (Rust)       │
│  Результат: текст ответа   │
└────────────────────────────┘
```

## F.5. ИЗМЕНЕНИЯ В ПЛАНЕ

### Новая Фаза 12: Локальный LLM-инференс (4 недели)

| Задача | Что | LOC | Зависимость |
|---|---|---|---|
| 12.1 | RoPE (Rotary Position Embedding) в pqc | ~100 | pqc |
| 12.2 | MQA (Multi-Query Attention) SIMD | ~300 | pqc |
| 12.3 | RMSNorm + SwiGLU в pqc | ~150 | pqc |
| 12.4 | KV-Cache arena (ring buffer, no malloc) | ~200 | — |
| 12.5 | BPE tokenizer для GLM-3 (в poler tokenizer) | ~100 | — |
| 12.6 | Sampling (greedy/temperature/top-k) | ~50 | — |
| 12.7 | .pqw конвертер: PyTorch GLM-3 → .pqw (int4) | ~500 | — |
| 12.8 | CLI флаг --llm local/remote/auto | ~100 | — |
| 12.9 | Streaming output (token-by-token) | ~100 | — |
| 12.10 | 70B SSD streaming (mmap pages, LRU cache) | ~300 | — |

**ИТОГО:** ~1900 LOC

### Обновлённый список фаз (12 → 12):

```
Фаза 1-2:   Foundation + Compression          (3 нед)
Фаза 3-4:   Vector Layer + SPLADE              (6 нед) — через pqc
Фаза 5:     IIR-Resonance Fusion (★ UNIQUE)   (3 нед)
Фаза 6:     Code Intelligence (tree-sitter)    (4 нед)
Фаза 7:     KG Intelligence (GLiNER + NLI)     (3 нед) — через pqc
Фаза 8:     Streaming Archives                 (3 нед)
Фаза 9:     Agentic (MCP v2 + WASM)           (2 нед)
Фаза 10:    Differential Dataflow               (2 нед)
Фаза 11:    .pqw + pqc bridge (Part E)          (2 нед) — инфраструктура
Фаза 12:    Локальный LLM (GLM-3 6B/70B)      (4 нед) ★ НОВОЕ

ИТОГО: ~32 недели (~8 месяцев)
```

## F.6. ОБНОВЛЁННЫЙ PROMPT_FOR_GLM.md

### Новые команды для агента:

```bash
# Локальный LLM (офлайн):
poler-engine --llm local --model glm-3-6b.pqw   -q "Проанализируй сцены с Мартой в T-24"

# Онлайн LLM (сервер GLM):
poler-engine --llm remote   -q "Проанализируй сцены с Мартой в T-24"

# Авто (poler сам решает):
poler-engine --llm auto   -q "Проанализируй сцены с Мартой в T-24"

# 70B локально (SSD streaming):
poler-engine --llm local --model glm-3-70b.pqw   -q "Проанализируй весь канон T-24"
```

## F.7. ФИНАЛЬНАЯ АРХИТЕКТУРА — ПОЛНЫЙ СУВЕРЕННЫЙ AI

```
poler-engine (ОДИН статический бинарь)
│
├── ПОИСК + RETRIEVAL
│   ├── BM25 + ε-density + IIR-resonance     ← poler-engine core
│   ├── SPLADE (learned sparse)              ← pqc-native
│   ├── BGE-M3 (dense embeddings)           ← pqc-native
│   ├── ColBERT (multi-vector)              ← next-plaid (Rust)
│   ├── IIR-Resonance Fusion (★ UNIQUE)    ← poler innovation
│   └── POLER[Ψ] attention field            ← poler innovation
│
├── ГРАФ ЗНАНИЙ
│   ├── K-hop entity graph (temporal)        ← poler-engine core
│   ├── GLiNER NER                           ← pqc-native
│   ├── NLI contradiction detection          ← pqc-native
│   └── Leiden community detection           ← linfa (Rust)
│
├── CODE INTELLIGENCE
│   ├── tree-sitter (multi-language AST)     ← tree-sitter (Rust)
│   ├── Salsa (incremental computation)      ← salsa (Rust)
│   └── Aider repomap (PageRank + symbols)   ← poler innovation
│
├── COMPRESSION + STREAMING
│   ├── FSST (string compression)            ← fsst-rs
│   ├── zstd-seekable (archive streaming)    ← zeekstd
│   └── RaBitQ (1-bit vector quantization)   ← poler-native
│
├── AGENTIC SUBSTRATE
│   ├── MCP v2 (resources + sampling)        ← poler-engine
│   ├── WASM plugins                         ← wasmtime
│   └── ReAct/Reflexion loops                ← poler-engine
│
├── LLM INFERENCE (★ НОВОЕ — Part F)
│   ├── ChatGLM-3 6B (int4, 4 ГБ)           ← pqc-native
│   ├── ChatGLM-3 70B (int4, SSD streaming) ← pqc-native
│   ├── BGE-M3 (embeddings)                 ← pqc-native
│   ├── SPLADE (sparse)                     ← pqc-native
│   ├── GLiNER (NER)                        ← pqc-native
│   └── NLI (contradiction)                 ← pqc-native
│
└── ВСЁ В ОДНОМ БИНАРЕ:
    ├── 0 Python
    ├── 0 PyTorch
    ├── 0 ONNX Runtime
    ├── 0 C++ FFI (ML)
    ├── 0 внешних API
    ├── 0 интернет (для локального режима)
    ├── SIMD/AVX2 на CPU
    ├── mmap + .pqw (ленивая загрузка весов)
    └── Работает на голом железе без ОС

ПОЛНЫЙ СУВЕРЕННЫЙ AI.
ОДИН БИНАРЬ.
УНИЧТОЖАЕТ ЗАВИСИМОСТЬ ОТ ИИ-ГИГАНТОВ.
```

## F.8. ЦЕЛЕВЫЕ МЕТРИКИ (ОБНОВЛЁННЫЕ)

| Метрика | SOTA 2026 | poler v2.0 + Part F |
|---|---|---|
| LLM inference | API (онлайн, дорого) | **локально, бесплатно** |
| LLM скорость (6B) | 20-40 ток/сек (Python) | **30-60 ток/сек (pqc-native)** |
| LLM скорость (70B) | 0.05 ток/сек (Colibrì) | **5-15 ток/сек (pqc + mmap)** |
| LLM RAM (6B int4) | 6-8 ГБ (Python) | **4 ГБ (pqc, int4)** |
| Интернет | нужен | **не нужен** |
| Внешние зависимости | Python + PyTorch + CUDA | **0** |
| Бинарь | не один (runtime + model + deps) | **1 статический** |
| Стоимость | API/подписка | **0 (бесплатно)** |

## F.9. ПОРЯДОК РЕАЛИЗАЦИИ

### Шаг 1: pqc bridge + .pqw формат (Part E, Фаза 11)
### Шаг 2: GLM-3 6B инференс в pqc (Фаза 12.1-12.6)
### Шаг 3: Конвертер PyTorch → .pqw (Фаза 12.7)
### Шаг 4: CLI --llm local/remote/auto (Фаза 12.8)
### Шаг 5: Streaming output (Фаза 12.9)
### Шаг 6: 70B SSD streaming (Фаза 12.10)

### КРИТЕРИЙ УСПЕХА:

> `poler-engine --llm local --model glm-3-6b.pqw -q "Привет"`
> → **ответ за <2 сек, локально, без интернета**
>
> `ldd poler-engine` → **not a dynamic executable**
>
> **Один бинарь. Полный AI. Без гигантов.**
