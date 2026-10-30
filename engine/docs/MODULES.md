# Справочник модулей `src/`

> Статус: актуален на v2.0 (67 266 LOC, 100 файлов, 29 модулей, ~1 059
> тестов). Каждый модуль имеет русскоязычную `//!`-шапку в коде — этот файл
> даёт навигационную выжимку с публичным API. Правило поддержки: изменение
> pub-API → правка соответствующей строки здесь (см. INDEX.md).

Условные обозначения: **LOC** — строк кода; **ядро** — критично для
основного поиска; **интерфейс** — способы доставки результата наружу;
**субстрат** — то, на чём стоят другие модули.

---

## Карта зависимостей (упрощённо)

```text
main.rs ─→ engine ─→ streaming ─→ tokenizer ─→ compression (TermFreqs)
   │           │         │
   │           │         └─→ parser ──→ graph ──→ aidde
   │           ├─→ resonance ←──────┘ (ε, R(t) над TermFreqs)
   │           ├─→ poler.rs / psi.rs (циклы поверх частот)
   │           └─→ output (ContextAnchor)
   ├─→ retrieval (grep/teddy/chunker/semantic-bridge)
   ├─→ vectors (RaBitQ/HNSW/PRBQ) ←─ pqc (эмбеддер) ←─ .pqw
   ├─→ llm / ner / quantum (потребители pqc)
   ├─→ web / vcs / notes / sources (внешние данные)
   └─→ shell / gateway / mcp / mcp_http / reader (интерфейсы)
```

---

## Ядро поиска

### `engine.rs` — 579 LOC
Оркестратор: полный прогон корпуса и watcher-режим (`--watch`).
`Engine`, `WatchEvent`, `WatchState` (VocabArena + PostingsStore —
инкрементальные обновления без пересборки).

### `streaming.rs` — 1 266 LOC
Двухпроходный конвейер индексации. `pass1_file` — токены/лексемы;
`pass2_file` — контекст, сцены (`SceneInfo`), отношения; литеральный
предфильтр (быстрый reject по редким термам); файлы-гиганты обрабатываются
параллельно чанками. `HitRecord` — сырой хит с byte-offset.

### `tokenizer/` — 392 LOC
Инвертированный индекс (`InvertedIndex`), zero-copy PII-очистка
(`PiiCleaner`, `PiiMode::{Off,Mask}`), UAX#29-сегментация.

### `resonance/` — 561 LOC
Математика ранжирования: `calculate_epsilon` (generic по `TermFreqs`),
`SlidingEpsilon` (оконная ε для сцен), `IirFilter` (R(t), режимы
hits/field/psi/poler). Теория — THEORY.md §2–3.

### `poler.rs` — 372 LOC
Канонический POLER-цикл (порт P3_Engine): `PolerParams`, `PolerCycle`,
`poler_resonances`, `cordic_inv_sqrt` (обратный корень без FPU-деления).

### `psi.rs` — 329 LOC
POLER[Ψ]-поле внимания: `PsiParams`, `PsiField`, `psi_resonances`,
`ResonanceMemory` (кольцевой буфер состояний поля).

### `output/` — 198 LOC
AI-Ready контракт: `ContextAnchor` (byte-offset, скоуп, сцена, метрики),
`SearchResult`, рендеры `render_markdown/simple`, формат `ai-json`.

## Точный поиск и пассажи

### `retrieval/` — 3 582 LOC
- `grep.rs` — слой 0: ВСЕ совпадения, exit-коды grep (0/1/2), флаги
  -A/-B/-c/-l/-L, `--grep-json` с byte-offsets; parity с GNU grep.
- `teddy.rs` — SIMD-предфильтр множественного сопоставления (pshufb-решёто,
  адаптивные якоря, фолд кириллицы, LeftmostLongest = AC-эквивалент).
- `chunk.rs` — слой B: секция→абзац→предложение, byte-range якоря
  (`--chunk --chunk-json`).
- `semantic_bridge.rs` — офлайн ru↔en расширение запроса (BM25-сенсор,
  `--semantic-expand` диагностика).

### `parser/` — 2 187 LOC
`CodeLang`/`CodeScope` — AST-lite скоупы для кода; markdown-сцены;
`Triple`/`extract_triples` — SVO-тройки для графа.

## Связи и анализ

### `graph/` — 853 LOC
`EntityGraph`, `KnowledgeNode`, `RelationEdge`, temporal-слои, K-hop BFS
(`--k-hop`), `IdentityPolicy` (политики слияния сущностей), экспорт
(`--graph-export`).

### `aidde/` — 1 294 LOC
AI-Interpreted Dependency & Impact Engine: `SymbolStore` (таблица символов)
→ call graph → `impact_analysis` (upstream/downstream паспорт символа,
`--impact --impact-depth`), `triage_scan` (быстрый аудит незнакомого кода),
кэш `--impact-cache`.

## Плотность памяти

### `compression/` — 2 098 LOC
`VocabArena` (словарь корпуса, 4.1× против HashMap), FSST-порт
(compress-probe, побитово детерминированная сериализация), `PostingsStore`
(lz4-парковка), `DocStoreCodec` (zstd, 26×), трейт `TermFreqs` —
инвариант «ε одинакова во всех представлениях». `GlobalStats`/`StatsRef` —
idf-статистики.

## Векторный субстрат

### `vectors/` — 2 308 LOC
`rabitq.rs`: `Encoder` (вращение Адамара + 1 бит), `QueryPrep`, оценки
sym (arcsin-MLE/popcount) и ADC (центрированный, несмещённый).
`hnsw.rs`: полер-нативный HNSW (детерминированная сборка).
`store.rs`: `QuantizedStore` (mmap PRBQ) + RAM-сборщик, трейт `CodeSource`.
`pqw_bridge.rs`: `PqwEmbedder` → трейт `Embedder`. `mod.rs`: `Embedder`,
`HashEmbedder` (тест-двойник). Формат — formats/PRBQ_FORMAT.md.

## Суверенный инференс

### `pqc/` — 6 636 LOC
- `tensor.rs` — AVX2 weight-only кернелы int8/int4/f32; LayerNorm/RMSNorm/
  GELU/SiLU/softmax/RoPE.
- `pqw.rs` — формат .pqw v2 (маппинг XLM-R/BERT/GLiNER/ChatGLM → конвенция,
  model_type + флаги спин).
- `tokenizer.rs` — XLM-R Unigram-Viterbi + Metaspace + NFKC-таблицы
  (nfc_tables.rs), профили нормализации v2, byte-fallback; 40/40 golden =
  HF tokenizers.
- `encoder.rs` — BERT/XLM-R-спина (BGE-M3); `deberta.rs` — DeBERTa-v2/v3
  disentangled attention (одна log-бакет-таблица c2p/p2c, rel-LN,
  staged-forward).
- `sha256.rs` — свой FIPS 180-4; `selftest.rs` — автономный цикл 6/6.
- Формат — formats/PQW_FORMAT.md.

### `llm/` — 1 112 LOC
`glm_engine.rs`: `GlmModel` — GLM-декодер из .pqw: RoPE, MQA/GQA,
SwiGLU, KV-арена (`KvArena`), MoE-роутер топ-k (`moe_route`), сэмплирование
`Sampling` (greedy/temperature/top-p), UAX#29-детокенизатор; `synth_glm`
для тестов.

### `ner/` — 750 LOC
`native_gliner.rs`: `RealGlinerModel` — BiLSTM-голова (гейты [i,f,g,o]
torch-конвенции), SpanMarker-MLP, prompt-проекция, жадный не-оверлап;
zero-shot метки через `--ner-labels`; `GlinerModel`/`synth_gliner`.

### `quantum/` — 2 271 LOC
См. [quantum-eri.md](quantum-eri.md). `mod.rs` — `QuantumMind` (L5);
`crystallizer.rs` — `SafetensorsHeader`, `R1CSGate`, `WeightCircuitBuilder`,
`execute_circuit_direct`; `meta_compiler.rs` — `VectorGate8`, `CSEOptimizer`,
`MetaPipeline`, `crystallize_to_flat_simd_rust`, `ternarize_mean_abs`,
`meta_compile_safetensors_tensor`.

## Внешний мир

### `web/` — 5 583 LOC
CDP-клиент (Chromium DevTools Protocol), краулер (`--crawl`, robots.txt,
sitemap, SimHash-дедуп, PageRank), `WebIndex` (SQLite BM25+WebRank),
phrase-поиск, WebLens (MV3-расширение). Схема — formats/WEB_INDEX_FORMAT.md.

### `vcs/` — 3 456 LOC
`trait VcsAdapter`: GitHub/GitLab/Gitea REST + gix; LFS; `clone_repo`;
URL-схемы `gh://owner/repo@rev`, `gl://`, `gt://`, `gix://`.

### `notes/` + `sources/` — 888 LOC
SQLite CRUD: `poler_notes` (заметки TUI/REPL, notebook_id, теги) и
`poler_sources` (реестр источников, kind/value). Схемы — в `//!`-шапках
модулей.

### `reader/` — 853 LOC
LLM Reader Desktop: `Workspace`, `Document`, `Annotation`, `Action`,
`tool_schema` (MCP-совместимая схема инструментов чтения), ReadSet
(интервальное множество прочитанных зон + Unread-гэпы), позиционный
курсор, закладки. Потребляется `quantum::QuantumMind::generate_with_reader`.

## Интерфейсы

### `main.rs` — 1 910 LOC
CLI: `struct Cli` (clap derive, ~85 флагов), `Format::{AiJson,Md,Simple}`,
`PiiArg`, `ResonanceArg`. Референс — CLI.md.

### `shell/` — 7 647 LOC
REPL (`--shell`, rustyline + `PolerCompleter`) и TUI (`--tui`, ratatui:
chat|notes|sources), doc_browser (локальные источники), mouse-поддержка,
`tui-textarea`.

### `gateway/` — ~14 060 LOC
Terminal Gateway: двойной контур исполнения (sandbox ↔ host), `exec_line`,
`GatewayState`, bind-mount, broker-токены, root broker (парольный режим),
jailbreak sentinel, контейнерный jail, PTY, hunter. Детали —
terminal-gateway-architecture.md.

### `mcp.rs` + `mcp_http.rs` — 2 049 LOC
MCP-сервер (JSON-RPC): stdio и HTTP (Bearer `--mcp-token`), инструменты
`poler_web_search/crawl/fetch/search/grep/chunk/box_exec/box_status`,
path/URL-гарды, `generate_token`.

### `bench/` — 1 819 LOC
6 контуров: Teddy-предфильтр, grep+parity (vs ripgrep/grep), BM25-golden,
чанкер, vectors, compression/RSS; `BenchOpts`, JSON-отчёт
`--benchmark-json`.

### `license/` — 104 LOC
Только EULA-метаданные (`eula_notice`, `status_text`). Гейт удалён в v2.0.

---

## Каталоги вне `src/`

| Путь | Что |
|---|---|
| `tests/integration.rs` | Полный пайплайн на фикстурах: контракт, режимы резонанса, PII, детерминизм |
| `tests/gateway.rs` | Живой REPL через пайп + lifecycle MCP (401/200) |
| `tests/pqw_real_model.rs` | Дифференциал токенизатора против HF golden (skip без модели) |
| `tests/gliner_real_model.rs` | Дифференциал GLiNER на реальном чекпойнте (skip без модели) |
| `tests/meta_compiler_flat_codegen.rs` | Кодоген компилируется rustc, бит-в-бит с runtime |
| `tests/safetensors_crystallize_bench.rs` | Bench на живом safetensors (skip без файла) |
| `poler_canonical_modules/` | Lean4-доказательство стационарности, Julia-брюсселятор, FPGA-Verilog, Rust-SCTP |
| `bench_trit5/` | Zig-бенчмарк Trit5 No-Mul |
| `scripts/` | Конвертеры: convert_hf_to_pqw.py, convert_gliner_to_pqw.py, convert_chatglm_to_pqw.py, extract_tokenizer_data.py |
| `examples/sandbox_probe.rs` | Пример песочницы gateway |
| `weblens/` | MV3-расширение браузера |
| `license-tool/` | Отдельный крейт инструментов лицензии |
