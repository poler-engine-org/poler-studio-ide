# SOTA RAG + Knowledge Graph Extraction Research (2026)

Scope: GraphRAG, LightRAG, HippoRAG, GLiNER, REBEL, plus temporal KGs,
contradiction detection, and incremental updates. Focus = what poler-engine
(Rust + CPU, heuristic SVO triples, petgraph K-hop, temporal tagging) can
borrow or must build.

---

## Tool Findings (1 paragraph each)

### GraphRAG (Microsoft, github.com/microsoft/graphrag)
Hierarchical RAG that builds a KG from unstructured text via LLM entity/relation
extraction, then runs **Hierarchical Leiden community detection** to partition
the graph recursively. Each community is summarized by an LLM into a text block.
Two retrieval modes: **local** (entity-neighborhood for specific questions) and
**global** (community-summaries map-reduced for "what are the main themes"
queries). Strength: global reasoning over whole corpus that naive vector RAG
fails at. Cost: very expensive indexing (many LLM calls per chunk + per
community). Rust availability: none — Python only, tightly coupled to LLM
backends. Algorithm to steal: Hierarchical Leiden + community-summary cache.

### LightRAG (HKUDS, github.com/HKUDS/LightRAG)
Lightweight KG-RAG with a **dual-level retrieval paradigm**: low-level
(entity + neighbors) and high-level (keyword-clustered themes). Combines graph
traversal with vector similarity in a single pass; incremental index updates
on new docs without full rebuild. Cheaper than GraphRAG (~10x less indexing
cost), comparable quality. Rust availability: none — Python. Steal: dual-level
retrieval pattern + incremental index design.

### HippoRAG (NeurIPS 2024)
Neurobiologically inspired — models **hippocampal indexing theory**:
pattern separation (distinguish similar memories) via OpenIE triple extraction
+ named entity recognition, pattern completion (retrieve from partial cue) via
PageRank over the personal knowledge graph (PG) combined with a recognition
memory module. Single-shot indexing of a full corpus, then online retrieval
without re-reading all docs. Outperforms RAG on multi-hop QA. Rust
availability: none — Python + LLM. Steal: PageRank-on-KG retrieval (algorithmic,
no LLM at query time).

### GLiNER (github.com/urchade/GLiNER)
Zero-shot NER using a **BERT-like bidirectional encoder**: given free-text
labels of desired entity types as input, it tags spans without fine-tuning.
Outperforms ChatGPT and fine-tuned LLMs on zero-shot NER benchmarks (NAACL
2024, ~371 citations). Tiny (~0.3B params) — runs on CPU. **GLiNER-Relex**
(May 2026) unifies NER + relation extraction in one encoder. Rust availability:
YES — `gline-rs` crate (ONNX-based inference engine for GLiNER models on
crates.io, Aug 2026). Direct drop-in for poler-engine via ONNX Runtime.

### REBEL (Babelscape, 2021, 650+ citations)
**Relation Extraction By End-to-end Language generation**: reframes RE as
seq2seq using BART. Linearizes triplets as text: `<triplet> subject <subj> relation
<obj> object`. Trained on ~200 relation types from Wikidata; can fine-tune for
custom schemas. SOTA on DocRED, NYT, NYTimes. Model: `Babelscape/rebel-large`
(~400M). Rust availability: not direct, but `candle-transformers` has BART
support — REBEL is a BART fine-tune, so port feasible via Candle or ONNX
Runtime. Heavier than GLiNER (~400M vs ~300M) and seq2seq is slower than
encoder-only on CPU.

### Temporal Knowledge Graphs (TKG)
TKG = multi-relational graph where edges carry timestamps:
`(subject, relation, object, time)`. Two research threads: **TKG
Completion** (predict missing edges at a given time; methods like T-GAP,
TransE-T) and **TKG Question Answering** (embedding-based or semantic-parsing).
Edge-validity windows `(t_start, t_end)` enable time-travel queries. Rust
availability: none — research code only. Build: store edges with
`(t_start, t_end)` in petgraph or a sidecar interval index; filter K-hop by
validity window.

### Contradiction Detection in KGs
Three approaches: (1) **NLI-based** — embed claim + KG triples, classify
entailment/contradiction/neutral with a small MiniLM-NLI model (GraphCheck,
FactCheck). (2) **Logical rule mining** — mine Horn rules, find positive and
negative evidential paths (Semantic Web Journal 2021). (3) **Pairwise fact
checking** (INSPECTOR) — compare two KG facts for semantic inconsistency.
Key signal for poler-engine: when inserting edge `(s, r, o')` and an edge
`(s, r, o)` already exists with `o' != o` and `r` is functional (e.g.
"born_in"), flag as contradiction. Rust availability: NLI models (e.g.
`cross-encoder/nli-deberta-v3-base`) load via ONNX Runtime Rust bindings.

### Incremental Graph Updates
Papers: **DIAL-KG** (schema-free autonomous incremental KG construction),
**CLKGE** (continual learning KGE avoiding catastrophic forgetting),
online KGE updates. Core problems: (1) new node embeddings without
re-embedding whole graph, (2) edge deletions/updates, (3) temporal decay.
For poler-engine's petgraph (in-memory): incremental = append-only edge
list with content-hash dedup + versioned node properties + lazy community
re-detection (mark affected communities dirty, re-cluster on next query).
No Rust crate does this directly — must build.

---

## What to Steal (table)

| Concept | Source | Rust path | Why for poler-engine |
|---|---|---|---|
| Hierarchical Leiden community detection | GraphRAG | `petgraph` + custom Leiden impl (or `linfa-clustering` has Leiden) | Theme-level summarization without LLM (rule-based cluster labels) |
| Dual-level retrieval (entity-neighborhood + keyword) | LightRAG | Build directly on existing K-hop | Multi-granularity queries cheap |
| PageRank-over-KG retrieval signal | HippoRAG | `petgraph::algo::page_rank` already feasible | Better multi-hop recall than K-hop alone |
| Zero-shot NER | GLiNER | `gline-rs` crate (ONNX) — drop-in | Replace regex NER; supports arbitrary entity types |
| Joint NER + RE | GLiNER-Relex | ONNX Runtime via `ort` crate | One model replaces regex SVO + manual entity rules |
| Triplet linearization format | REBEL | N/A (format, not lib) | Standardized `(<s>, <r>, <o>)` text output for downstream |
| NLI contradiction scoring | FactCheck/GraphCheck | `cross-encoder/nli-*` via `ort` | Functional-relation conflicts flagged on insert |
| TKG edge validity windows | TKG literature | Build on existing temporal tagging | Time-travel queries, edge expiry |
| Continual embedding update | CLKGE | Build custom (small scale) | Avoid full re-embed on doc append |
| Incremental community dirty-flag | DIAL-KG | Build on Leiden impl | Re-cluster only affected subgraphs |

---

## What to Build From Scratch (table)

| Component | Why from scratch | Sketch |
|---|---|---|
| Incremental petgraph update layer | No Rust crate; petgraph is immutable-friendly | Append-only edge log + versioned adjacency; hash-dedup edges; mark affected nodes/communities dirty |
| Contradiction detector on functional relations | NLI is generic; poler needs schema-aware | Tag relations as functional/non-functional; on insert, check existing `(s, r, *)` and score conflict via NLI only when ambiguous |
| Time-aware edge filter | petgraph has no temporal support | Store `(t_start, t_end, source_doc)` per edge; K-hop filters by `now ∈ [t_start, t_end]` |
| Rule-based community summarizer (no LLM) | poler-engine is CPU + no LLM dep | Top-N entities by PageRank in community + top relations by frequency + temporal span → text label |
| Hybrid retrieval ranker (PageRank × K-hop × vector) | Each existing impl is coupled | Fusion rank: `score = α·PR + β·hop_decay + γ·cos_sim`; tune on internal eval |
| Schema relation ontology (functional, temporal, hierarchical) | Needed for contradiction + temporal | Hand-authored YAML: `relations: {born_in: {functional: true, temporal: point}, employed_by: {functional: false, temporal: interval}}` |
| Edge provenance + confidence | KG trust without LLM verification | Each edge stores `(source_doc_id, span_offset, extractor, confidence)`; contradictions lower confidence |

---

## Gap Analysis: What poler-engine is Missing

### Critical gaps (block quality)
1. **No LLM/encoder extraction** — regex SVO misses entities without explicit
   verbs, misses typed relations, misses multi-token entities. **Fix:**
   swap regex NER for GLiNER via `gline-rs` (ONNX, CPU-OK, ~300M params,
   ~50-200ms/doc on CPU). Keep regex as fallback for offline/zero-dep builds.
2. **No relation typing** — `(subject, verb, object)` triples are untyped;
   no way to detect that "born_in" is functional (one birthplace per person)
   so contradictions impossible to flag. **Fix:** schema ontology YAML +
   either GLiNER-Relex (joint NER+RE) or REBEL (heavier, BART).
3. **No contradiction detection** — duplicate/conflicting triples silently
   accumulate. **Fix:** on edge insert, query existing `(s, r, *)`; if `r` is
   functional and `o' != o`, run NLI cross-encoder on `(o, o')`; flag if
   `contradiction > 0.7`; store both edges with degraded confidence + a
   `conflicts_with` edge between them.

### Important gaps (block scale)
4. **No incremental updates** — every new doc likely triggers full re-build
   of the graph. **Fix:** append-only edge log with hash-dedup; community
   dirty-flag (Leiden re-cluster only when a community's edge count grows
   > X%); node embeddings updated incrementally (CLKGE-style: only re-embed
   nodes within K hops of insertion).
5. **No community structure** — K-hop is purely local; no theme-level
   retrieval. **Fix:** add Leiden clustering (`linfa-clustering` or custom);
   build community-summary cache (rule-based, no LLM) for global queries.
6. **No semantic retrieval signal** — K-hop + temporal only; no vector
   similarity fuse. **Fix:** add `candle` or `ort` sentence-embedding model
   (`all-MiniLM-L6-v2`, ~22M, CPU-fast); hybrid-rank with existing signals.

### Minor gaps (polish)
7. **Temporal tagging exists but unused in graph** — tag is on the source
   doc, not the edge. **Fix:** propagate temporal tags to edges as
   `(t_start, t_end)` intervals; enable time-travel K-hop.
8. **No provenance** — can't tell which doc/span a triple came from.
   **Fix:** store `(doc_id, char_span, extractor, confidence)` per edge;
   required for any human-in-the-loop audit.
9. **No PageRank / centrality** — K-hop treats all neighbors equally.
   **Fix:** `petgraph::algo::page_rank` is ~20 lines; use as retrieval
   prior + community-summary entity ranking.

---

## Recommended Build Order (smallest → highest leverage)

1. **Edge provenance + schema ontology** (YAML, no new deps) — unblocks
   everything downstream.
2. **Temporal edges** — propagate existing tags to edges; interval-filter
   K-hop. Pure data-model change.
3. **PageRank retrieval prior** — one `petgraph` call, big retrieval win.
4. **Leiden community detection** — `linfa-clustering` or hand-rolled;
   enables theme queries + summarizer.
5. **Incremental update layer** — append-only log + dirty communities;
   unblocks streaming use cases.
6. **GLiNER via `gline-rs`** (ONNX) — first neural extractor; keep regex
   fallback. CPU-feasible.
7. **NLI contradiction detector** — `ort` + small NLI cross-encoder; only
   invoked on functional-relation conflicts (rare path).
8. **(Optional) REBEL or GLiNER-Relex** — typed relation extraction; heavier,
   only if schema ontology outgrows keyword matching.

---

## Open Questions / Risks

- **CPU latency of GLiNER/REBEL on Rust `ort`**: needs benchmark on target
  hardware. GLiNER-Paper ~0.3B params → expect 50-300ms/doc on consumer CPU.
  May need batched async pipeline.
- **Leiden in Rust**: `linfa-clustering` Leiden is functional but less mature
  than Python `igraph`/`graphtool`; may need custom impl for hierarchical
  variant.
- **NLI model choice**: `nli-deberta-v3-base` (~180M) is accurate but slow;
  `ms-marco-MiniLM-L-6-v2` + thresholding is faster but less calibrated.
- **Memory growth**: append-only edge log unbounded; need compaction strategy
  (merge equivalent edges, expire stale temporal edges).

---

## Summary

poler-engine has solid bones (petgraph K-hop, temporal tagging, SVO regex)
but is missing the **three load-bearing SOTA ideas**:

1. **Neural extraction** (GLiNER) — replace regex with zero-shot NER.
2. **Community structure** (Leiden à la GraphRAG) — enable theme queries.
3. **Contradiction detection** (NLI on functional relations) — keep the KG
   trustworthy as it grows.

Plus the **operational gap**: incremental updates + edge provenance, without
which the graph degrades silently under streaming load. All six are
Rust-feasible today (ONNX Runtime + petgraph + linfa); no Python dependency
required.
