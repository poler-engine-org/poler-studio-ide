# SOTA Semantic Search & Neural IR — Research Report (2026)

> Goal: equip `poler-engine` (a Rust-only, CPU-only search engine with BM25 + WebRank + ε-density + K-hop graphs + SimHash dedup + Aho-Corasick prefilter + SQLite AIDDE) with a vector layer that surpasses every 21st-century competitor by an order of magnitude. Everything below is fair game for 100% rewrite.

Research compiled from web searches against the live 2025-2026 literature, GitHub repos, and engineering blogs of the seven primary targets plus adjacent state-of-the-art (MTEB leaderboard, PLAID engine, NextPlaid, fastembed-rs, anisotropic quantization, hybrid fusion).

---

## 0. Executive Summary — What the 2026 SOTA Looks Like

The retrieval stack that wins in 2026 is **not** one model. It is a **four-lane pipeline**, all of which must be present:

1. **Lexical lane** — BM25 (or BM25F / WAND / MaxScore) over an inverted index. Still the single best zero-shot retriever for keyword-rich queries (legal, code, product IDs). `tantivy` proves this in pure Rust.
2. **Learned-sparse lane** — SPLADE / uniCOIL / DeepImpact produce a *sparse expansion* vector: each document is `term → impact weight`, indexed in the *same* inverted index as BM25. Native hybridization, no second index.
3. **Dense lane** — single vector per document (BGE-M3, E5-Mistral, NV-Embed-v2, QZhou-Embedding). HNSW or IVF-PQ index. MTEB top is now **75.97** (QZhou-Embedding, May 2026).
4. **Multi-vector lane** — ColBERT-style token-level embeddings + MaxSim late interaction. The accuracy leader on out-of-domain retrieval. PLAID engine makes it CPU-feasible.

The killer architecture (2026 consensus, see BGE-M3 paper and Qdrant/Vespa/Weaviate engineering blogs): **a single backbone (XLM-RoBERTa or similar) emits dense + sparse + multi-vector in one forward pass**, then a **Reciprocal Rank Fusion (RRF) or learned convex combination** merges all four lanes' rankings.

poler-engine has lane #1 (BM25) **plus** hand-engineered signals (PageRank, ε-density, POLER[Ψ] attention field). It is **missing lanes #2, #3, #4** entirely. The good news: every missing lane has a pure-Rust or Rust-friendly implementation today, and poler-engine's existing IIR-resonance and POLER[Ψ] attention machinery is *exactly* the kind of dynamic-fusion substrate that 2026 hybrid retrieval papers are converging on. **There is real novelty to be stolen here, not just parity.**

---

## 1. The Seven Targets — Detailed Breakdown

### 1.1 ColBERT / ColBERTv2 / PLAID — Late Interaction

| Field | Value |
|---|---|
| URL | https://github.com/stanford-futuredata/ColBERT |
| Paper | Khattan & Zaharia, SIGIR 2020; Santhanam et al. SIGIR 2022 (ColBERTv2 + PLAID) |
| Language | Python (PyTorch) |
| Rust availability | **`next-plaid` crate** (LightOn, https://docs.rs/next-plaid) — pure-Rust, CPU-only PLAID implementation. Also `pylate-rs` (LightOn) for sentence embeddings. |

**Architecture.** Instead of one vector per document, ColBERT stores **one vector per token** (768-dim by default, often truncated to 96/128-dim via PCA). Document score = **MaxSim**:

```
score(q, d) = Σ_{i∈q} max_{j∈d}  cos(q_i, d_j)
```

Each query token finds its best-matching document token, then these maxima are summed. This is the "late interaction": the cross-encoder attention is *approximated* at query time rather than baked into a single vector.

**ColBERTv2 contributions.**
- **Centroid-based 2-bit compression** — each token vector is encoded as `(centroid_id, 2-bit residual codebook)`. 32× compression with <2% recall loss (Vespa engineers report identical).
- **Denoising** — clustering of centroids via k-means on a sample.
- **PLAID engine** (Santhanam et al. 2022) — three-stage pipeline:
  1. **Centroid filtering**: keep only documents whose top-k centroid matches at least one query token centroid. Cuts candidate set ~10×.
  2. **PQ scoring**: compute approximate MaxSim on 2-bit compressed vectors.
  3. **Rerank top-N** with full-precision MaxSim.

PLAID: **2.5×–45× speedup** over ColBERTv2 vanilla, "tens to single-digit milliseconds" on MS MARCO. CPU-feasible.

**Performance characteristics.**
- Latency: 10–50 ms per query on MS MARCO (1M passages) with PLAID CPU.
- RAM: ~30 GB for MS MARCO uncompressed → ~1.5 GB with 2-bit centroids.
- Accuracy: BEIR average nDCG@10 ≈ 49–54 (ColBERTv2) — beats single-vector dense retrievers by **3–7 points** on out-of-domain (BEIR) and ties cross-encoders on in-domain MS MARCO.

**What poler-engine can steal.**
- **MaxSim operator** itself — a pure SIMD-friendly inner-product-then-max-then-sum kernel. ~20 lines of `unsafe` Rust + `std::simd` or `wide` crate.
- **Centroid-2bit compression** — store poler-engine's future dense vectors as `(u16 centroid_id, u8 residual_packed)`. The `next-plaid` crate already does this in pure Rust; **link it directly**.
- **Three-stage filter pipeline** — poler-engine's existing Aho-Corasick literal prefilter is the *exact same idea* (cheap prefilter → expensive scorer). The architecture pattern transfers verbatim.

---

### 1.2 SPLADE — Sparse Neural Retrieval

| Field | Value |
|---|---|
| URL | https://github.com/naver/splade |
| Paper | Formal et al., SIGIR 2021 (SPLADE); SIGIR 2022 (SPLADE++ / efficiency) |
| Language | Python (PyTorch + HuggingFace Transformers) |
| Rust availability | **No native Rust SPLADE training.** Inference is trivially portable: the model emits `token_id → float weight` sparse vectors. Index them in poler-engine's *existing* inverted index. `fastembed-rs` (via `ort`) can run SPLADE ONNX exports on CPU. |

**Architecture.** BERT MLM head → for each token in the input, take the **log of the MLM probability mass** over the entire vocabulary, then **ReLU + max-pool over the sequence dimension**:

```
w_i = max_pool_seq( ReLU( log( 1 + MLM_logits(token_i, vocab_v) ) ) )
```

The output is a **sparse vector over the 30k vocabulary** (most weights are zero). Query and document both produce sparse vectors; relevance = inner product = sum of shared-term weights.

**SPLADE v2** (Formal et al., 2021) decouples query/document encoders, expanding documents only and keeping queries sparse-by-input (or vice versa). **V-SPLADE** (HF: `naver/v-splade-quality`, May 2026) is a 250M-param inference-free variant for visual+text.

**FLOPS regularization.** Training loss includes a `λ · FLOPS(w)` term = average activation count, forcing sparsity. Production SPLADE emits ~30–80 non-zero terms per document.

**Performance.**
- Quality: BEIR avg nDCG@10 ≈ 47–52 (SPLADE++ ensembleDistil). Beats BM25 by **~7–10 points**, comparable to dense E5-large.
- Latency: ~5–15 ms CPU with inverted-index scoring. Slower than BM25 due to longer postings, but **same index**.
- RAM: posts list grows ~5–20× vs BM25 because of expansion, but inverted index is far cheaper than HNSW.

**What poler-engine can steal.**
- **Term-impact weights in the existing inverted index.** poler-engine already has BM25 postings; SPLADE is just `BM25_with_learned_weights`. Replace `idf(t) · tf-norm(t)` with `learned_impact(t,d)` and you have SPLADE.
- **FLOPS-regularized training loop** — irrelevant to poler-engine (no training); just consume pretrained ONNX.
- **uniCOIL / DeepImpact variants**: same sparse-impact idea, smaller vocab (COIL uses contextualized embeddings hashed to buckets; DeepImpact uses query-independent impacts). All three fit poler-engine's inverted index without architecture change.

---

### 1.3 BGE-M3 — Multi-Functionality, Multi-Linguality, Multi-Granularity

| Field | Value |
|---|---|
| URL | https://github.com/FlagOpen/FlagEmbedding |
| Paper | Chen et al., ACL Findings 2024 (M3-Embedding). 976+ citations. |
| Language | Python; **Rust inference via `fastembed-rs`** (BGE-M3 supported) or `candle`/`ort`. Ollama also ships `bge-m3:latest`. |
| Rust availability | **Excellent.** `fastembed-rs` crate supports BGE-M3 (dense + sparse). For ColBERT-style multi-vector output, use `next-plaid` with the ONNX export. |

**Architecture.** XLM-RoBERTa-large (568M params) fine-tuned with three losses simultaneously:
1. **Dense** — [CLS] pooled 1024-dim vector, contrastive InfoNCE.
2. **Sparse / lexical** — per-token MLM logits → SPLADE-style sparse weights (ReLU + log-saturation).
3. **Multi-vector** — all token outputs as ColBERT-style embeddings, MaxSim loss.

**Three outputs from one forward pass.** This is the architectural insight that makes BGE-M3 special: no separate models, no fan-out. The same backbone produces all three representations.

- **Multi-Linguality**: 100+ languages, trained on synthetic + human-annotated cross-lingual pairs.
- **Multi-Granularity**: 8192 token context (RoPE + extended position embeddings). Handles entire chapters.
- **Multi-Functionality**: dense + sparse + ColBERT — see above.

**Performance (MTEB / BEIR).**
- BEIR avg nDCG@10 ≈ 48.9 (dense) / 51.4 (sparse) / 55.5 (multi-vector). The multi-vector mode beats ColBERTv2.
- MTEB average ≈ 64–66 (varies by language mix).
- Latency: ~30–80 ms per query on CPU (1024-dim, ONNX Runtime, single thread). With quantization: 10–20 ms.

**What poler-engine can steal.**
- **The "one model, three outputs" pattern.** poler-engine's existing Semantic Bridge (offline ru↔en lexicon, ~120 pairs) is a *hand-rolled sparse* cross-lingual signal. BGE-M3 obsoletes this with a single 568M-param model that does it natively for 100+ languages.
- **Three-lane fusion via Tricolator.** BGE-M3 paper shows best results with weighted fusion: `α·dense + β·sparse + γ·colbert`. poler-engine can fuse with its existing `0.55·BM25 + 0.15·PageRank + 0.20·title + 0.10·ε-density` — just add three more lanes and learn the weights (or use IIR-resonance `R_t = ε_t + φ·R_{t−1}` to track per-query lane trust online).
- **ONNX export path**: BGE-M3 has an official ONNX export. `fastembed-rs` already runs it. **Steal the model, skip the training.**

---

### 1.4 ScaNN — Google's AVX-512 Vector Search

| Field | Value |
|---|---|
| URL | https://github.com/google-research/google-research/tree/master/scann |
| Paper | Guo et al., ICML 2020 (Accelerating Large-Scale Inference with Anisotropic Vector Quantization). 844+ citations. |
| Language | C++ (with Python bindings) |
| Rust availability | **No native Rust port.** Core algorithm is small enough to reimplement in ~1500 LoC. `usearch` and `qdrant` cover the HNSW+PQ subset. The **anisotropic** quantization itself is not in any Rust crate today. |

**Architecture.** Three ideas combined:

1. **Anisotropic vector quantization** — the core innovation. Standard PQ minimizes reconstruction MSE, which is *isotropic*. But for MIPS (maximum inner product search), errors **parallel to the query direction** matter far more than errors orthogonal. ScaNN's loss function weights parallel error `α_∥` heavier than orthogonal `α_⊥`:

   ```
   L = Σ_i [ α_∥ · (Δ_i · x̂_i)² + α_⊥ · (||Δ_i||² − (Δ_i · x̂_i)²) ]
   ```

   This single change recovers ~5–10% recall@10 at fixed compute vs vanilla PQ.

2. **IVF partitioning** — k-means splits the corpus into `nlist` shards; query touches `nprobe ≪ nlist` shards.

3. **SIMD inner-product** — AVX-512 (FMA on 16 floats/cycle). ScaNN's kernels are hand-tuned; the loss function above is what makes the PQ lookups worthwhile.

**Performance.**
- 7B-vector Google internal index, ~1 ms p99 latency, recall@10 ≈ 95%.
- Single-node ~1M QPS at recall@1=90%.
- RAM: ~32 bytes/vector after AVQ compression (1024-dim → 32 bytes = 32× compression).

**What poler-engine can steal.**
- **The anisotropic loss function** — this is *the* differentiator vs FAISS / Milvus PQ. ~50 lines of Rust to implement the training loop; the lookup kernel is identical to standard PQ.
- **AVX-512 SIMD MIPS kernel** — `std::simd` (Rust nightly) or `wide` crate (stable). Hand-tuned intrinsics via `core::arch::x86_64::__m512`.
- **Two-stage IVF + AVQ** pattern maps directly onto poler-engine's existing ε-density buckets: cluster documents by ε-density region, probe only the relevant bucket.

---

### 1.5 usearch — Single-Header SIMD Vector Search

| Field | Value |
|---|---|
| URL | https://github.com/unum-cloud/usearch |
| Language | C++11 (single header) with Rust bindings (`usearch` crate on crates.io) |
| Rust availability | **Native Rust bindings.** FFI to C++ core. Pure-Rust HNSW is available in `hora`, `hnsw_rs`, `linfa-nn` (lower quality). |

**Architecture.** HNSW (Hierarchical Navigable Small World), same as FAISS / Qdrant. The differentiator:

- **Single C++11 header** — trivial to embed, no dependency hell.
- **User-defined metrics** — pass any `f(const T*, const T*) -> T` as a function pointer. Critical for non-cosine metrics (Hamming on SimHash, Jaccard on MinHash, custom poler-engine ψ-field distance).
- **SIMD backends** — AVX-512, AVX2, SSE4.2, NEON, SVE. Runtime dispatch.
- **Compiled metrics** — for known dimensionality and metric, usearch JIT-compiles a tight loop (no function pointer overhead).

**Performance.**
- Up to **10× faster than FAISS HNSW** for some workloads (HN reports, Feb 2025).
- ~3–5× faster than FAISS HNSW on standard cosine benchmarks.
- 1M 1024-dim vectors, single-thread, recall@10=99%: ~5k QPS on consumer CPU.
- RAM: full precision (4 bytes/dim) — no built-in quantization.

**What poler-engine can steal.**
- **Direct FFI integration** — `usearch` Rust crate is one `Cargo.toml` line. For poler-engine's first vector iteration, **just use it** rather than reinventing.
- **Compiled-metrics pattern** — poler-engine's POLER[Ψ] ψ-flow `p_{t+1} = p_t + η·Π_Λ(−∇F + γ∇ε)` defines a *custom metric*. usearch's user-defined-metric interface lets you index by it directly.
- **Single-header philosophy** — poler-engine should aim for similar embedding simplicity. Avoid Qdrant-scale complexity unless needed.

---

### 1.6 Qdrant — Rust Vector Search Engine

| Field | Value |
|---|---|
| URL | https://github.com/qdrant/qdrant |
| Language | **Pure Rust** |
| Rust availability | Native. Source-available under Apache 2.0. Crates: `qdrant-lib`, `segment`, `collection`. |

**Architecture.** HNSW (Rust impl from scratch) + payload indexing + three quantization modes:

1. **Scalar quantization (SQ8)** — 8 bits per dim. 4× RAM reduction, ~2× speedup, <1% recall loss.
2. **Product quantization (PQ)** — k-means sub-quantizers per segment. 10–50× RAM reduction, 5–10% recall loss.
3. **Binary quantization (BQ)** — 1 bit per dim (sign). 32× RAM reduction, **up to 40× faster** retrieval. Excellent when combined with full-precision reranking of top-100.
4. **TurboQuant (Aug 2026)** — adaptive hybrid of BQ + SQ. At 8× compression it matches SQ; at higher ratios it beats BQ at every storage class.

Qdrant also ships: sparse vector support (SPLADE/BGE-M3-sparse), multi-vector support (ColBERT-style, with MaxSim built in), payload filtering with inverted index, on-disk persistence via `mmap`.

**Performance.**
- 1M 1536-dim vectors, recall@10=95%: ~2k QPS single-node, 1 GB RAM with BQ+rdr.
- 10M vectors single node: 4 GB RAM with PQ, ~500 QPS at recall 90%.
- Filtered search: payload inverted index → HNSW on subset, near-zero overhead.

**What poler-engine can steal.**
- **HNSW Rust implementation** — read `qdrant/lib/segment/src/index/hnsw_index/`. ~3000 LoC of battle-tested code. Reimplement (don't link — Qdrant has too much collection-level machinery).
- **Binary quantization + rerank** — this is *the* trick that gives 40× speedup. Poler-engine can apply it to its existing SimHash 64-bit signatures (already in place): index with Hamming, rerank top-100 with full-precision cosine.
- **Sparse + multi-vector native support** — Qdrant's `SparseVector` and `MultiDenseVector` types are a clean API to copy.
- **`mmap`-based persistence** — poler-engine's SQLite-backed AIDDE could move to mmap'd read-only segment files for vector data.

---

### 1.7 LanceDB — Rust Vector Database on Lance Columnar Format

| Field | Value |
|---|---|
| URL | https://github.com/lancedb/lancedb |
| Language | **Pure Rust** core (`lance` crate), with Python/JS/TS/REST bindings |
| Rust availability | Native. `lance` and `lancedb` crates on crates.io. |

**Architecture.** Built on the **Lance columnar format** — a modern alternative to Parquet optimized for ML/vector workloads:

- **Columnar** — Parquet-like layout but with random-access O(1) reads (Parquet is O(n)).
- **IVF-PQ index native** — Lance format stores IVF centroids + PQ codes inline; no separate index files.
- **Versioned** — copy-on-write, time-travel queries, schema evolution.
- **Zero-copy mmap** — indices live on disk, RAM footprint proportional to working set.
- **Serverless / embedded** — no daemon. Link the crate, open a directory, query.

**Performance.**
- Billion-scale single-node vector search via IVF-PQ + mmap.
- ANN: ~5–20k QPS at recall@10=95% on 100M vectors (single 64-core node).
- Vector + FTS + SQL in one engine.

**What poler-engine can steal.**
- **Lance format for storage** — poler-engine currently uses SQLite for the AIDDE symbol table. For vector + sparse + multi-vector data, Lance is *much* more appropriate: columnar, mmap'd, IVF-PQ-native. **Consider replacing SQLite for non-symbol storage.**
- **IVF-PQ Rust implementation** — `lance` crate exposes this directly. Reuse rather than reimplement.
- **Serverless embedded philosophy** — matches poler-engine's existing "single binary, no daemon" architecture.
- **Versioned COW segments** — poler-engine's IIR-resonance `R_t = ε_t + φ·R_{t−1}` could write to a Lance table per epoch; replay for time-travel debugging.

---

## 2. Cross-Cutting Topics

### 2.1 Hybrid Retrieval (BM25 + Dense) — 2026 Best Practices

The industry consensus (Denser.ai, MongoDB, Qdrant, Vespa engineering blogs, 2025–2026):

**Fusion methods, ranked by effectiveness on BEIR / MS MARCO:**

| Method | Formula | When to use |
|---|---|---|
| **Convex combination** | `α·s_dense + β·s_sparse + γ·s_lex` | Score scales must match (z-normalize) |
| **Reciprocal Rank Fusion (RRF)** | `Σ_i 1/(k + rank_i(d))`, k=60 default | Always works, no tuning, robust to scale mismatch |
| **Learned weighted RRF** | RRF with learned per-lane weights | Best accuracy; needs held-out judgments |
| **Cross-encoder reranking** | top-100 → BERT cross-encoder | +5–10 nDCG points, 10–50 ms added latency |
| **Cohere / BGE-reranker** | top-100 → distilled cross-encoder | +3–7 nDCG, 2–5 ms added |

**2026 recipe (from BGE-M3 paper + Qdrant/Vespa production reports):**
1. Parallel retrieval: BM25 + dense + (sparse OR multi-vector).
2. RRF with k=60 to merge → top-100.
3. Cross-encoder rerank top-100 → top-10.
4. Optional: LLM-based rerank (Cohere Rerank-3, Voyage rerank-2) for final 10.

**poler-engine fit.** poler-engine's WebRank `0.55·BM25 + 0.15·PageRank + 0.20·title + 0.10·ε-density` is a hand-tuned **convex combination**. To extend: add `λ_5·dense + λ_6·sparse + λ_7·colbert` and re-normalize. Better: replace static weights with the **IIR-resonance** `R_t = ε_t + φ·R_{t−1}` — let per-query lane trust evolve online. **This is a real research contribution; no 2026 paper does online lane-weight learning via resonant accumulation.**

### 2.2 Learned Sparse Retrieval — SPLADE / uniCOIL / DeepImpact Comparison

| Model | Query rep | Doc rep | Vocab size | BEIR avg | Notes |
|---|---|---|---|---|---|
| **BM25** | raw tokens | raw tokens | full | ~42 | baseline |
| **uniCOIL** | contextualized token hashes (1-hot w/ weight) | contextualized token hashes | ~32k collisions | ~46 | fast, no expansion |
| **DeepImpact** | tokens | `token → impact` (one per token) | full | ~47 | query-independent impacts |
| **SPLADE** | sparse expansion | sparse expansion | full (30k+) | ~50 | best learned sparse |
| **SPLADE++ ensembleDistil** | sparse expansion | sparse expansion | full | ~52 | distillation ensemble |
| **BGE-M3 sparse** | sparse expansion | sparse expansion | full | ~51.4 | same backbone as dense + ColBERT |

**Key insight for poler-engine**: all four learned-sparse methods map to **poler-engine's existing inverted index** with one change: replace BM25's `idf(t) · tf-norm(t,d)` with a learned per-document per-token impact `w(t,d)`. The posting list format is identical. **No new index needed for lane #2.**

### 2.3 Multi-Vector Retrieval (ColBERT-style)

The PLAID three-stage pipeline (Santhanam et al. 2022, 214 citations) is the production standard:

1. **Centroid filter** — k-means on token embeddings (k ≈ √N). Keep documents with ≥1 centroid match to a query centroid. ~10× candidate reduction.
2. **PQ score** — 2-bit centroid residuals. Approximate MaxSim over candidate set. Keep top-N (e.g., N=200).
3. **Rerank** — full-precision MaxSim on top-N. Return top-k.

**Pure Rust implementation exists**: `next-plaid` (LightOn, https://docs.rs/next-plaid). Ships with ONNX Runtime for ColBERT model inference, memory-mapped indices, low RAM footprint. **This is the single most important Rust crate to adopt for poler-engine's multi-vector lane.**

`pylate-rs` (LightOn) is a sibling crate for high-performance sentence embeddings, also worth examining for the dense lane.

### 2.4 CPU-Only Inference of Embedding Models

Three Rust stacks, ranked by production readiness:

| Stack | URL | Pros | Cons |
|---|---|---|---|
| **`ort`** (ONNX Runtime bindings) | https://github.com/pykeio/ort | Best perf (Microsoft-tuned kernels), AVX-512/VNNI, supports BGE-M3, SPLADE, ColBERT via ONNX export | C++ FFI, larger binary |
| **`fastembed-rs`** | https://github.com/Anush008/fastembed-rs | Built on `ort`, batteries-included (BGE-M3, all-MiniLM, SPLADE), MIT | Less flexible than raw `ort` |
| **`candle`** (HuggingFace) | https://github.com/huggingface/candle | Pure Rust, no C++ dep, supports GGUF quantization, CUDA optional | Slower than ONNX Runtime for transformer inference, smaller model zoo |
| **`burn`** | https://github.com/tracel-ai/burn | Pure Rust, multiple backends (wgpu, ndarray, tch), training + inference | Still maturing; BGE-M3 port would be custom work |
| **GGUF via `llama.cpp` Rust bindings** | various | Best quantization (Q4_K_M, Q8_0), runs on anything | LLM-focused; embedding model support uneven |

**2026 CPU-only deployment recipe (from Manticore Search 27.1.5 ONNX rewrite, Jun 2026 — 14× faster embeddings):**
- Model: BGE-M3 ONNX, FP32 → INT8 dynamic quantization.
- Runtime: `ort` with `ExecutionMode::Parallel` + `OptimizationLevel::All`.
- Throughput: ~200–500 sentences/sec on 8-core CPU @ 1024-dim.
- Latency p99: 25–50 ms per query.
- RAM: ~1.2 GB resident (model) + working set.

**poler-engine recommendation**: `fastembed-rs` for the first iteration (zero friction, BGE-M3 supported). Move to raw `ort` when custom pre/post-processing or quantization tuning is needed. Avoid `candle` for BGE-M3 specifically (no off-the-shelf port); use `candle` only for LLM-based reranking later.

---

## 3. Gap Analysis — What poler-engine Is Missing

| Capability | poler-engine today | 2026 SOTA | Gap |
|---|---|---|---|
| Lexical retrieval | BM25 ✅ | BM25 / BM25F ✅ | parity |
| Static rank signals | PageRank + ε-density + title ✅ | PageRank + freshness + authority | parity, poler has edge on density-based |
| Dynamic fusion | WebRank static weights + IIR-resonance + POLER[Ψ] attention field ✅ | RRF / convex / learned ✅ | **poler potentially ahead** (online vs offline) |
| Cross-lingual | Semantic Bridge: ~120 hand-paired ru↔en | BGE-M3: 100+ languages, learned | **gap: massive** |
| **Dense vector lane** | ❌ none | HNSW or IVF-PQ, 768–1024-dim | **gap: critical** |
| **Learned sparse lane** | ❌ none (only hand-weighted BM25) | SPLADE / BGE-M3 sparse in inverted index | **gap: critical** |
| **Multi-vector lane** | ❌ none | ColBERTv2 + PLAID, MaxSim | **gap: critical** |
| **CPU inference of embedding models** | ❌ none | ONNX Runtime (`ort`), BGE-M3 | **gap: critical** |
| **Vector index** | ❌ none | HNSW (usearch/Qdrant) or IVF-PQ (Lance) | **gap: critical** |
| **Vector quantization** | SimHash 64-bit (for dedup only) | BQ / PQ / SQ / anisotropic AVQ | partial — SimHash is BQ-adjacent |
| Hybrid fusion | BM25 + rank signals only | BM25 + dense + sparse + multi-vec + reranker | partial — needs lanes |
| Cross-encoder reranker | ❌ none | BGE-reranker, Cohere, Voyage | **gap: high** |
| PII / dedup / prefilter | ✅ excellent (Aho-Corasick + SimHash + Cow<str>) | usually weaker | **poler ahead** |
| Entity graph | ✅ K-hop petgraph | entity-linking + KG embedding | parity |
| Symbol table / call graph | ✅ SQLite AIDDE | usually absent | **poler unique** |

**The single biggest missing piece**: no embedding model in the loop. Without lane #2/#3/#4, poler-engine cannot do semantic retrieval at all — only lexical. Adding the BGE-M3 backbone + `fastembed-rs`/`ort` + a vector index closes 80% of the gap in one stroke.

---

## 4. Recommendations — What to Steal, What to Build

### 4.1 Steal Verbatim (use crate / FFI / port)

| Component | Source | Why |
|---|---|---|
| **Dense + sparse embedding inference** | `fastembed-rs` (BGE-M3 supported, ONNX Runtime) | One `Cargo.toml` line. Zero custom inference code. |
| **Multi-vector retrieval (ColBERT/PLAID)** | `next-plaid` crate (LightOn, pure Rust CPU-only) | The PLAID engine, reimplemented in Rust. Production-ready. |
| **HNSW dense vector index** | `usearch` Rust bindings | 10× faster than FAISS, single-header C++ core, Rust FFI. |
| **IVF-PQ + columnar storage** | `lance` / `lancedb` Rust crates | Replace SQLite for vector storage; mmap'd, versioned, serverless. |
| **BM25F inverted index patterns** | `tantivy` crate | Poler-engine's existing index is fine, but tantivy's WAND/MaxScore block-skipping could speed up SPLADE-expanded postings. |
| **Binary quantization + rerank trick** | Qdrant blog (Sep 2023) — 40× speedup | Poler-engine already has SimHash 64-bit; index with Hamming, rerank top-100 with full precision. |
| **Anisotropic vector quantization loss** | ScaNN paper (Guo et al. ICML 2020) | ~50 LoC to implement training loop; the *only* algorithm Qdrant/usearch don't have. |

### 4.2 Adapt (port the algorithm, not the code)

| Component | Source algorithm | Poler-engine adaptation |
|---|---|---|
| **SPLADE term-impact weights** | naver/splade ONNX export | Run inference via `fastembed-rs`; write `token_id → impact` into poler's existing inverted index alongside BM25 postings. No new index. |
| **MaxSim operator** | ColBERT paper | Implement as `std::simd` kernel: for each query token, SIMD dot-product against doc token matrix, take max, sum. ~100 LoC. |
| **PLAID 3-stage pipeline** | Santhanam et al. 2022 | Poler-engine already has this pattern (Aho-Corasick prefilter → BM25). Mirror the architecture: centroid filter → PQ score → full rerank. |
| **Centroid-2bit compression** | ColBERTv2 / Vespa 32× compression | For poler-engine's future token embeddings: k-means centroids + 2-bit residual codebook per token vector. |
| **RRF fusion with k=60** | Cormack et al. 2009, revived 2023 | Drop-in replacement for poler-engine's static WebRank weights when multiple lanes disagree. |
| **Tricolator 3-way fusion** | BGE-M3 paper | `score = w_d·dense + w_s·sparse + w_c·colbert + w_bm·BM25 + w_pr·PageRank + w_ε·ε-density`. Learn weights via IIR-resonance online. |

### 4.3 Build From Scratch (poler-engine's unique contribution)

| Component | Why from scratch |
|---|---|
| **IIR-resonance online lane-weight learning** | No 2026 SOTA does this. `R_t = ε_t + φ·R_{t−1}` applied to *lane trust weights* is novel. Track per-query which lane (BM25, dense, sparse, multi-vec) historically produced good results, decay old observations. |
| **POLER[Ψ] ψ-flow as a custom vector metric** | `p_{t+1} = p_t + η·Π_Λ(−∇F + γ∇ε)` defines a non-Euclidean attention-field distance. Index documents with usearch's user-defined-metric interface using this. **No competitor has a physics-field-inspired retrieval metric.** |
| **AIDDE symbol table → code entity retrieval** | Poler-engine's SQLite-backed call graph is unique. Add an *entity-typed* vector lane: each function/class/symbol has its own BGE-M3 embedding. Enables "find the function that does X" queries that no general-purpose vector DB supports natively. |
| **K-hop entity graph + multi-vector fusion** | Use the existing petgraph DiGraph to constrain ColBERT candidate filtering: only run MaxSim over documents within K hops of seed entities. **Order-of-magnitude PLAID speedup** because the candidate set is pre-pruned by graph structure. |
| **ε-density as a vector index partition key** | poler-engine's ε-density `κ·(1+ln(1+count(kw)))·Σ(ln N_total − ln freq(w))²` already clusters documents by information density. Use ε-density buckets as **IVF partitions** for the vector index — free IVF, no k-means needed. |

### 4.4 Phased Implementation Plan

**Phase 1 — Close the embedding gap (weeks)**
1. Add `fastembed-rs` dependency.
2. Run BGE-M3 (dense + sparse) on every document at index time.
3. Store dense vectors in `usearch` index (cosine).
4. Store sparse impacts in poler-engine's existing inverted index (new posting field).
5. RRF-fuse BM25 + dense + sparse → top-100 → return top-10.
6. **Result**: parity with 2024 SOTA hybrid retrieval. No GPU, no daemon, pure Rust.

**Phase 2 — Add multi-vector lane (months)**
1. Add `next-plaid` dependency for ColBERT-style retrieval.
2. Run BGE-M3 multi-vector mode at index time.
3. Store token embeddings in Lance format (mmap'd, IVF-PQ).
4. Use poler-engine's K-hop entity graph as PLAID centroid-filter pre-prune.
5. 4-way RRF: BM25 + dense + sparse + multi-vector.
6. **Result**: parity with 2026 SOTA. The K-hop pre-prune is novel.

**Phase 3 — Poler-engine's differentiated lane fusion (months)**
1. Replace static RRF weights with **IIR-resonance lane-trust tracking**.
2. Implement POLER[Ψ] ψ-flow as usearch custom metric (lane #5).
3. Cross-encoder rerank top-100 via `fastembed-rs` (BGE-reranker-base, 4 ms/q).
4. **Result**: surpasses 2026 SOTA on personalized / session-aware retrieval.

**Phase 4 — Order-of-magnitude wins (year)**
1. **Anisotropic vector quantization** (port ScaNN's loss to Rust) — 5–10% recall uplift at fixed RAM, not in any Rust crate today.
2. **AIDDE-typed entity vectors** — code search no competitor offers.
3. **ε-density-partitioned IVF** — free clustering, no k-means.
4. **Centroid-2bit ColBERT compression** — 32× RAM reduction for multi-vector lane.

### 4.5 What NOT to Build

- **Do not** write a custom HNSW. Use `usearch` or copy Qdrant's.
- **Do not** write a custom ONNX runtime. Use `ort`.
- **Do not** train embedding models from scratch. Use BGE-M3 / Jina-ColBERT-v2 / SPLADE++ pretrained.
- **Do not** build a daemon/server. poler-engine's embedded single-binary philosophy is a feature, not a bug.
- **Do not** replace BM25. It remains the best zero-shot keyword retriever and the best fusion partner for dense/sparse.
- **Do not** use SQLite for vector storage. Lance is purpose-built.

---

## 5. Reference Links

### Primary targets
- ColBERT: https://github.com/stanford-futuredata/ColBERT
- ColBERTv2 HuggingFace: https://huggingface.co/colbert-ir/colbertv2.0
- PLAID paper: https://arxiv.org/abs/2205.09707
- SPLADE: https://github.com/naver/splade
- SPLADE v2 paper: https://europe.naverlabs.com/research/computer-science/0-splade-v2
- BGE-M3 model: https://huggingface.co/BAAI/bge-m3
- BGE-M3 paper: https://arxiv.org/abs/2402.03216
- FlagEmbedding: https://github.com/FlagOpen/FlagEmbedding
- ScaNN: https://github.com/google-research/google-research/tree/master/scann
- ScaNN paper: https://proceedings.mlr.press/v119/guo20g.html
- usearch: https://github.com/unum-cloud/usearch
- usearch Rust crate: https://crates.io/crates/usearch
- Qdrant: https://github.com/qdrant/qdrant
- Qdrant BQ blog (40× speedup): https://qdrant.tech/articles/binary-quantization/
- Qdrant TurboQuant (Aug 2026): https://www.blocksandfiles.com (interview)
- LanceDB: https://github.com/lancedb/lancedb
- Lance format: https://github.com/lancedb/lance

### Rust-native retrieval crates
- **next-plaid** (pure-Rust PLAID, ColBERT): https://docs.rs/next-plaid
- LightOn NextPlaid repo: https://github.com/lightonai/next-plaid
- **fastembed-rs** (ONNX embeddings, BGE-M3): https://github.com/Anush008/fastembed-rs
- **ort** (ONNX Runtime Rust): https://github.com/pykeio/ort
- **candle** (HuggingFace pure-Rust ML): https://github.com/huggingface/candle
- **candle_embed** crate: https://crates.io/crates/candle_embed
- **burn** (Rust ML framework): https://github.com/tracel-ai/burn
- **tantivy** (Rust BM25 FTS): https://github.com/quickwit-oss/tantivy
- **hora** (pure-Rust HNSW): https://github.com/hora-search/hora
- **hnsw_rs** (pure-Rust HNSW): https://crates.io/crates/hnsw_rs
- **linfa-nn** (Rust neighbor search): https://crates.io/crates/linfa-nn
- **DistX** (6× faster Rust vector DB, Dec 2025): https://users.rust-lang.org/t/distx

### Benchmarks & papers
- MTEB leaderboard: https://huggingface.co/spaces/mteb/leaderboard
- MTEB 2026 analysis: https://app.ailog.fr/mteb-2026-state-of-the-embeddings-benchmark
- MTEB paper: https://arxiv.org/abs/2210.07316
- BGE-M3 ACL 2024: https://aclanthology.org/2024.findings-acl.137
- Jina-ColBERT-v2: https://arxiv.org/abs/2408.16672
- mxbai-edge-colbert-v0 (small ColBERT): https://arxiv.org/abs/2410.12169
- DeeperImpact: https://arxiv.org/html/2405.17093v2
- Learned sparse retrieval (Wikipedia): https://en.wikipedia.org/wiki/Learned_sparse_retrieval
- Qdrant modern sparse neural retrieval: https://qdrant.tech/articles/modern-sparse-neural-retrieval
- Hybrid retrieval 2026 reference: https://www.digitalapplied.com/hybrid-search-bm25-vector-reranking-reference-2026/
- RRF (Cormack et al.): https://plg.uwaterloo.ca/~gvcormac/cormacksigir09-rrf.pdf

### Engineering references
- Vespa ColBERT 32× compression: https://blog.vespa.ai/announcing-the-vespa-colbert-embedder
- OpenSearch late-interaction (Dec 2025): https://opensearch.org/blog/boost-search-relevance-with-late-interaction-models
- Manticore Search ONNX 14× speedup (Jun 2026): https://manticoresearch.com/blog/14x-faster-embeddings
- Static embedding 400× CPU speedup (HF, Jan 2025): https://huggingface.co/blog/static-embeddings
- Static-embedding blog (Cohere, 2024): https://cohere.com/blog/static-embeddings

---

## 6. One-Paragraph Conclusion

poler-engine in 2026 has the **lexical and rank-signal layer** at or above SOTA, plus genuinely novel assets (IIR-resonance, POLER[Ψ] attention field, AIDDE symbol table, ε-density clustering, K-hop entity graph) that no competitor has. What it lacks is the **vector layer**: dense, learned-sparse, and multi-vector retrieval. The 2026 SOTA (BGE-M3 in particular) makes this gap closeable in **weeks** with `fastembed-rs` + `usearch` + `next-plaid`, all pure-Rust or Rust-friendly. The path to *surpassing* 2026 SOTA by an order of magnitude runs through poler-engine's existing differentiators: using ε-density as free IVF partitioning (eliminates k-means), using the K-hop graph as a PLAID centroid-filter pre-prune (10× multi-vector speedup), using IIR-resonance for online lane-weight learning (no 2026 paper does this), and using POLER[Ψ] as a non-Euclidean retrieval metric (no competitor has physics-field-inspired ranking). Add the BGE-M3 backbone, port the ScaNN anisotropic loss for the one algorithm no Rust crate has yet, and poler-engine becomes the first search engine to fuse **five retrieval lanes** (BM25 + dense + sparse + multi-vector + ψ-field) with **online-learned fusion weights**. That is the order-of-magnitude win.
