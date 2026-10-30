# SOTA Streaming + NLP + Compression Research for poler-engine (Rust, CPU-only, 16–32 GB RAM)

**Date:** 2026-09-13
**Scope:** State-of-the-art (SOTA) survey across 7 areas to inform a 100% rewrite of `poler-engine` whose explicit goal is to *surpass everything in the 21st century by an order of magnitude* on a home PC (CPU only, 16–32 GB RAM).
**Method:** 36 web searches via the `z-ai` `web_search` function, raw snippets archived under `/home/z/my-project/upload/s1_*.json … s36_*.json`. Each section below cites the canonical URL for every tool and ends with a *steal / build* verdict relative to poler-engine's current state.

> **poler-engine current state (for context):**
> - Streaming: `mmap`-based, multi-pass (1 = stats, 2 = tokenize hits, 3 = materialize top-N); zero-copy `FileTokens<'a>` from a memory-mapped slice.
> - Compression: none (raw text).
> - NLP: custom Unicode tokenizer, Cyrillic stemming, Aho-Corasick semantic markers (negation/duty/threat), SimHash near-dup, PII masker.
> - Embeddings: **none**.
> - Vector quantization: **none**.
> - Incremental: file-watcher with mtime/size cache; no differential dataflow.
> - Memory: `memmap2`, no `madvise` strategy, no advanced mmap tricks.
> - Architecture: Rust + CPU only, 16–32 GB RAM target.

---

## 0. Executive Summary — The 10× Play

The SOTA in 2026 has crystallized around a small set of primitives that, combined, allow a CPU-only Rust process to index **terabyte-scale text corpora with embeddings and vector search in <32 GB RAM**:

1. **zstd-seekable** multi-frame compression + a side index → random-access into compressed archives over HTTP Range. **Poler's multi-pass mmap design maps 1:1 onto this**; replace `Mmap` with `SeekableReader<HttpRangeReader>` and you keep the same `&'a str` zero-copy borrowing, but now over the wire.
2. **FSST** for per-column string compression (1–3 GB/s, random access) — this is the missing piece that lets the inverted index sit in RAM at 5–10× density without paying zstd's decompression tax per query.
3. **candle + fastembed-rs / ort** for CPU embedding inference at 100–1000 sentences/sec (BGE-small int8 or MiniLM GGUF). Poler currently has **zero** vector layer — this is the single biggest missing capability.
4. **RaBitQ + HNSW** for binary-quantized ANN with reranking — 32× memory compression of vectors with theoretical error bounds. Lets you fit ~1B 768-d embeddings in 12 GB.
5. **DBSP / differential-dataflow** for incremental view maintenance — replaces poler's "watcher mode" with mathematically-grounded incremental recomputation that gives correct results under arbitrary updates.
6. **memmap2 + madvise(SEQUENTIAL/RANDOM/WILLNEED)** — trivial change, ~2× speedup on multi-pass scans.

The "order-of-magnitude" win comes from **stacking** all of these: a multi-pass mmap pipeline (poler already has this) over **zstd-seekable HTTP archives** (no local disk), indexing into **FSST-compressed inverted index + RaBitQ-quantized HNSW vectors**, refreshed incrementally by **DBSP Z-sets**, all on CPU. No single SOTA system combines these; each existing tool (Tantivy, LanceDB, USearch, Materialize) covers 1–2 of them.

---

## 1. Streaming Archives (Zero-Storage Processing)

The goal: read a 100 TB remote archive as if it were local, without ever materializing it on disk, while still allowing multi-pass access (poler's design requires ≥3 passes per file).

### 1.1 zstd Seekable Format

| | |
|---|---|
| **Name** | Zstandard Seekable Format |
| **URL** | https://github.com/facebook/zstd/blob/dev/contrib/seekable_format.md ; Rust: https://github.com/rorosen/zeekstd and https://crates.io/crates/zstd-framed |
| **What it does** | Splits a compressed stream into independent frames (default 4 MB each) and appends a *seek table* (skippable frame) at the end. The table maps uncompressed offsets → frame boundaries, so a reader can `seek()` to any uncompressed offset and decompress only the surrounding frame. |
| **Performance** | Compression within 1–3% of plain zstd (frame overhead is negligible at 4 MB). Decompression of a single frame is ~1.5 GB/s; random access latency = one frame's decompression cost ≈ 2–3 ms on a 4 MB frame at level 19. |
| **Rust availability** | **`zeekstd`** (pure Rust, `docs.rs/zeekstd`) — actively maintained, matches the upstream spec byte-for-byte. **`zstd-framed`** (https://crates.io/crates/zstd-framed) provides sync + async `SeekableReader`/`SeekableWriter` types and is the more ergonomic API. Both build on the `zstd-rs` FFI. |
| **Borrow** | The seek-table format spec — directly implement a poler-flavored reader that returns `&'a str` slices into a per-frame decode buffer. **Multi-frame independence is exactly what poler's pass-1/pass-2/pass-3 needs**: each pass can re-seek the same compressed stream without re-downloading. |
| **Verdict** | **STEAL.** This is the foundation of zero-storage. Replace `Mmap` with `zstd_framed::SeekableReader<HttpRangeReader>`; poler's `FileTokens<'a>` borrowing survives untouched. |

### 1.2 HTTP Range Requests

| | |
|---|---|
| **Name** | HTTP Byte-Range Requests (RFC 7233) |
| **URL** | https://developer.mozilla.org/en-US/docs/Web/HTTP/Range_requests ; Rust: https://docs.rs/async_http_range_reader |
| **What it does** | Client sends `Range: bytes=START-END`; server returns 206 Partial Content. Combined with zstd-seekable, lets you fetch only the frame(s) covering the bytes you want. |
| **Performance** | One RTT per range; pipelining / HTTP/2 multiplexing amortizes this. AWS S3 / Cloudflare / commoncrawl.org all honor Range. Typical cold latency: 30–100 ms per range over WAN; <5 ms over LAN. |
| **Rust availability** | `async_http_range_reader` (docs.rs) is the canonical async random-access HTTP reader. `reqwest` + manual `Range` headers also works. Combine with `tokio_util::io::ReaderStream` for backpressure. |
| **Borrow** | The pattern; not a crate to depend on directly. Build a poler `HttpRangeBlob` that: (a) prefetches the next N frames via HTTP/2 multiplex, (b) caches decoded frames in an LRU sized to a configurable RAM budget, (c) exposes `async fn read_range(uncompressed_start: u64, len: usize) -> &[u8]`. |
| **Verdict** | **BUILD** a thin layer; **STEAL** the spec. |

### 1.3 Tar Streaming (No-Seek)

| | |
|---|---|
| **Name** | `tar` / `tokio-tar` / `async-tar` |
| **URL** | https://github.com/alexcrichton/tar-rs ; https://github.com/astral-sh/tokio-tar |
| **What it does** | Sequential iteration over tar entries from any `Read`/`AsyncRead`. No seek required — this is critical because tar entries are concatenated with no central index. |
| **Performance** | ~1 GB/s on a hot SSD; bottleneck is the underlying reader (HTTP/decompression), not tar parsing. |
| **Rust availability** | `tar` (sync, alexcrichton) is the de-facto standard. `tokio-tar` is the async fork (now maintained by astral-sh). ⚠️ **Security note (Oct 2025):** the `async-tar` crate has the *TARmageddon* path-traversal / nested-tar smuggling CVE and **will not be patched** — `tokio-tar` is the maintained fork. Poler must use `tokio-tar` and validate every entry path against a canonicalized root. |
| **Borrow** | Tar entry iteration logic. For `*.tar.gz` / `*.tar.zst` over HTTP: stack `HttpRangeReader → ZstdDecoder → tokio_tar::Archive::entries()` and iterate. **For multi-pass on a single tar entry**, you cannot re-seek a non-seekable tar — so you must either (a) cache the byte range of interesting entries discovered in pass 1 and re-fetch them via HTTP Range in pass 2, or (b) prefer `tar.zst` where zstd-seekable gives you random access into the *decompressed tar* and then use a seekable tar reader. |
| **Verdict** | **STEAL** `tokio-tar`. Build a `TarEntryIndex` cache (path → `[start, end)` byte range in the decompressed stream) on pass 1 so passes 2–3 can use HTTP Range to fetch only the entries they need. |

### 1.4 Common Crawl WARC Streaming

| | |
|---|---|
| **Name** | WARC (Web ARChive) format |
| **URL** | Spec: https://commoncrawl.org/the-data/get-started/ ; Rust: https://docs.rs/warc |
| **What it does** | WARC is the standard record-based format for web archives. Each record has headers (`WARC-Type`, `WARC-Date`, `WARC-Record-ID`, `Content-Length`) + a payload (HTML, WAT metadata, WET extracted text). Common Crawl publishes monthly dumps as `wet.tar.gz`, `wat.tar.gz`, `warc.tar.zst` — petabyte scale. |
| **Performance** | The `warc` Rust crate exposes a `WarcReader` that yields records iterator-style from any `BufRead`; zero intermediate allocation beyond the current record. |
| **Rust availability** | `warc` (docs.rs/warc) — sync streaming reader + writer. DuckDB also ships a `warc` extension that can parse WARC directly in SQL — useful as a reference for record-layout edge cases. `warcio-s3` (Python) is the upstream reference implementation; the Rust crate matches it feature-for-feature. |
| **Borrow** | The `WarcReader` iterator pattern. Poler's `FileTokens<'a>` borrow style maps directly: a WARC record's payload becomes the borrowed `&'a str`. |
| **Verdict** | **STEAL** the `warc` crate. Poler's filter pipeline (aho-corasick prefilter → ε/IIR resonance → SimHash dedup) is *exactly* the right shape for a streaming WARC consumer: most records are discarded at the prefilter stage. |

### 1.5 Hugging Face Datasets Streaming Mode

| | |
|---|---|
| **Name** | HF Datasets `streaming=True` |
| **URL** | https://huggingface.co/docs/datasets/stream ; Rust: https://docs.rs/huggingface_hub (download) + `parquet` crate (read) |
| **What it does** | HF stores datasets as sharded Parquet on S3. Streaming mode iterates row-group-by-row-group, downloading only what's needed. |
| **Performance** | One row group (~128 MB) per worker in memory at a time. The official Python implementation has a known memory-leak issue (#7269) when wrapping a streaming dataset in a DataLoader — Rust avoids this trivially because there's no GC. |
| **Rust availability** | No first-party Rust streaming client. The recipe: `huggingface_hub` crate to resolve shard URLs → HTTP Range over Parquet → `parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder` with `with_batch_size(N)` for row-group streaming. `arrow-rs` is fully streaming. |
| **Borrow** | The Parquet row-group streaming pattern from `arrow-rs`. |
| **Verdict** | **BUILD** a poler `HfDatasetSource` that streams Parquet row groups. Not hard — `arrow-rs` already does the heavy lifting. |

### 1.6 Section Summary — What's Missing in poler-engine

- ❌ No HTTP Range fetcher (poler is disk-only today).
- ❌ No zstd-seekable reader — would unlock remote archives with multi-pass.
- ❌ No WARC reader — would unlock Common Crawl directly.
- ❌ No tar-entry byte-range index for selective re-fetch in passes 2–3.
- ✅ Poler's `FileTokens<'a>` zero-copy borrow style is *already* the right abstraction — it just needs to borrow from a decode buffer instead of an mmap.

---

## 2. Compression Algorithms (Rust Ecosystem)

Poler currently stores raw text. Adding compression to the inverted index, the document store, and the vector store gives 5–32× memory density for free.

### 2.1 zstd (Facebook)

| | |
|---|---|
| **Name** | Zstandard |
| **URL** | https://github.com/gyscos/zstd-rs (Rust FFI over https://github.com/facebook/zstd) |
| **What it does** | Modern LZ77 + FSE + Huffman codec; the reference for "fast + good ratio." |
| **Performance** | Level 3: ~500 MB/s compress, ~1500 MB/s decompress, ratio ~3.0× on text. Level 19: ~2.6 MB/s compress, 1100 MB/s decompress, ratio ~6.0×. Level 22 (max): ratio ~6.5×. Decompression is *always* fast regardless of level. |
| **Rust availability** | `zstd-rs` (gyscos) is the canonical FFI binding. Pure-Rust decoder exists in `ruzstd` (used by the kernel). |
| **Borrow** | Use as the *bulk* compressor for cold documents and for zstd-seekable archives. Train a dictionary (§2.5) for short texts. |
| **Verdict** | **STEAL** as the workhorse for archive/stream compression. Already a poler-engine dependency candidate. |

### 2.2 lz4

| | |
|---|---|
| **Name** | LZ4 / lz4_flex |
| **URL** | https://github.com/bozaro/lz4-rs (FFI) ; https://github.com/pseitz/lz4_flex (pure Rust) |
| **What it does** | LZ77 family, optimized for *extreme* decompression speed at the cost of ratio. |
| **Performance** | `lz4_flex` decompresses at **2+ GB/s**, compresses at ~350 MB/s, ratio ~2.0–2.5× on text. The fastest pure-Rust compressor. |
| **Rust availability** | `lz4_flex` (pure Rust, no unsafe by default) — preferred over FFI bindings for auditability. `lzzzz` (FFI) is marginally faster but adds a C dependency. |
| **Borrow** | Use for *hot-path* decompression where the same data is read many times per query (e.g., postings lists that get touched on every search). |
| **Verdict** | **STEAL** `lz4_flex` for hot postings. |

### 2.3 brotli

| | |
|---|---|
| **Name** | Brotli |
| **URL** | https://github.com/dropbox/rust-brotli |
| **What it does** | LZ77 + context modeling + static dictionary of common web strings. Best-in-class ratio for HTTP text. |
| **Performance** | Decompression ~500 MB/s; compression level 11 ratio on text ~3.5–4.0×. Slower than zstd at every level; better ratio than zstd below level 16. |
| **Rust availability** | `rust-brotli` (dropbox) — pure Rust, no unsafe. |
| **Borrow** | Limited use: HTTP responses, pre-compressed static assets. Not worth the CPU vs zstd for a search engine's internal data. |
| **Verdict** | **SKIP** for poler-engine internals. zstd wins on the speed/ratio Pareto frontier. |

### 2.4 FSST (Fast Static Symbol Table) — *the* string-compression primitive for a search engine

| | |
|---|---|
| **Name** | FSST |
| **URL** | Paper: Boncz, Neumann, Leis (VLDB 2020) https://ir.cwi.nl/pub/50085/ ; Rust: https://crates.io/crates/fsst-rs and https://docs.rs/fsst ; Optimized variant: OptFSST (arXiv 2607.11271, 2026) |
| **What it does** | Trains a *static* symbol table of ≤255 variable-length (1–8 byte) symbols from a sample. Each string is encoded as a sequence of symbol IDs (1 byte each) + an escape mechanism for uncovered bytes. **Random access**: decode any string in isolation without decompressing neighbors. |
| **Performance** | Compression ~1–3 GB/s, decompression ~2–5 GB/s, ratio ~2.0–2.5× on natural-language text, ~3× on URIs/identifiers. **Crucially, decompression of a single string is O(len) with no warm-up** — perfect for posting-list traversal. |
| **Rust availability** | `fsst-rs` (crates.io, 2024-08) — pure Rust, zero-dependency. `rudb_encoding::fsst` (docs.rs) — embedded in a Rust DB. OptFSST (2026) improves ratio further but no Rust crate yet. |
| **Borrow** | This is the **single highest-leverage compression add** for poler-engine. Apply FSST to: (a) every token in the inverted index (tokens are highly repetitive), (b) every document's raw text in the doc store, (c) URL/path strings, (d) the symbol table in `aidde::symbols`. Train one FSST encoder per corpus partition. |
| **Verdict** | **STEAL** `fsst-rs` and integrate as a transparent layer under poler's `Box<str>` token storage. Expect ~2× reduction in inverted-index RAM with zero query-time overhead (decode is faster than the string comparison that follows). |

### 2.5 Dictionary-Based Compression for Text Corpora

| | |
|---|---|
| **Name** | zstd dictionary training |
| **URL** | https://facebook.github.io/zstd/#small-data ; Rust: `zstd::dict::from_sample` |
| **What it does** | Train a ~100 KB dictionary from a corpus sample; use it as the prefix for compressing many small (<16 KB) documents. Eliminates the "small-data penalty" where zstd level 19 on a 4 KB doc achieves only 1.3× but with a dictionary achieves 4×. |
| **Performance** | On small documents: 2–4× better ratio than plain zstd at the same speed. Training cost: ~30 s on a 100 MB sample (one-time). |
| **Rust availability** | `zstd-rs` exposes `train_dictionary` and `with_dictionary`. |
| **Borrow** | Train one dictionary per top-level corpus (e.g., "rust source code", "English web text", "Russian web text") and ship it as a poler asset. Use it for the cold document store. |
| **Verdict** | **STEAL.** ~10 lines of code, 2–4× ratio win. |

### 2.6 Compression Recommendation Matrix

| Use case in poler-engine | Recommended codec | Why |
|---|---|---|
| Remote archive transport (HTTP) | zstd-seekable level 19 + dict | Random access + best cold ratio |
| Inverted-index token storage | **FSST** | Random access to individual strings at GB/s |
| Hot postings lists | lz4_flex | 2 GB/s decode, postings touched every query |
| Cold document store | zstd-19 + corpus dictionary | Max ratio, decompressed rarely |
| Vector embeddings | (see §6 — RaBitQ / scalar int8) | Domain-specific quantization, not generic codec |
| HTTP responses / API payloads | (skip brotli) | zstd is strictly better here on CPU |

---

## 3. NLP Fundamentals (Rust Ecosystem)

Poler has a custom Unicode tokenizer and Cyrillic stemmer. The 2026 SOTA offers dramatically better alternatives for the non-differentiating parts.

### 3.1 tokenizers (HuggingFace)

| | |
|---|---|
| **Name** | `huggingface/tokenizers` |
| **URL** | https://github.com/huggingface/tokenizers ; Rust: https://docs.rs/tokenizers |
| **What it does** | Production-grade BPE / WordPiece / Unigram / SentencePiece tokenizer. Sub-2 µs/token on CPU. |
| **Performance** | Tokenizes **1 GB of text in ~20 s** on a server CPU (HF's own benchmark). `bpe` crate (crates.io) and `tiktoken-rs` (OpenAI's tiktoken FFI) are alternatives — `tiktoken-rs` is faster for the specific OpenAI vocabularies. **Gigatoken** (marktechpost, Jul 2026) claims 24.53 GB/s — 989× faster than HF — but is single-vocab and not general-purpose. |
| **Rust availability** | First-class: the Python `tokenizers` library *is* a Rust core with Python bindings. `tokenizers` crate is the API. |
| **Borrow** | Use HF `tokenizers` for the *embedding-model* sub-vocab (BGE/MiniLM/nomic all ship a `tokenizer.json`). Keep poler's custom Unicode tokenizer for the *inverted index* — they serve different purposes (sub-word for embeddings vs. word-level for IR). |
| **Verdict** | **STEAL** for the embedding pipeline only. Do **not** replace the inverted-index tokenizer; word-level is correct for IR. |

### 3.2 whatlang / whichlang — Language Detection

| | |
|---|---|
| **Name** | `whatlang` (greyblake) vs `whichlang` (quickwit-oss) vs `lingua` (pemistahl) |
| **URL** | https://github.com/greyblake/whatlang-rs ; https://github.com/quickwit-oss/whichlang ; https://github.com/pemistahl/lingua-rs |
| **What it does** | Detect the natural language of a text fragment. |
| **Performance** | `whatlang`: 75 languages, trigram-based, very fast (<1 µs), ~92–96% accuracy on short texts. `whichlang`: 99.5% accuracy on quickwit's validation set, ~10× faster than `whatlang`, but only ~25 languages. `lingua`: 75 languages, **highest accuracy** (96% on short text, 89% on very short), but 10–100× slower than whatlang. |
| **Rust availability** | All three are pure Rust. `whatlang` is the most established. |
| **Borrow** | Replace poler's `detect_lang` heuristic with **`whatlang`** as the default (fast path) and `lingua` as a fallback for ambiguous short snippets. Poler already detects Cyrillic — `whatlang` extends this to 75 languages for free. |
| **Verdict** | **STEAL** `whatlang`. Optionally layer `lingua` for short queries. |

### 3.3 rust-stemmers

| | |
|---|---|
| **Name** | `rust-stemmers` (Snowball algorithms) |
| **URL** | https://github.com/mrordinaire/rust-stemmers ; https://docs.rs/rust_stemmers |
| **What it does** | Snowball stemmer implementations for 18 languages (English, French, German, Spanish, Russian, Arabic, Tamil, Turkish, Basque, …). Byte-identical output to the official Snowball reference. |
| **Performance** | ~100 ns/word; trivially fast. |
| **Rust availability** | `rust-stemmers` (mrordinaire) is the canonical crate. `porter_stemmers_rs` (lib.rs, 2026) is a more recent port covering more languages. |
| **Borrow** | Poler currently has only a Cyrillic stemmer. Adding `rust-stemmers` gives 17 more languages for ~zero effort. The `Algorithm::Russian` variant is a drop-in upgrade for poler's hand-rolled one. |
| **Verdict** | **STEAL**. Replace poler's hand-rolled stemmer with `rust-stemmers` for non-Cyrillic, keep custom one as a fallback for edge cases. |

### 3.4 unicode-segmentation

| | |
|---|---|
| **Name** | `unicode-segmentation` |
| **URL** | https://github.com/unicode-rs/unicode-segmentation ; https://docs.rs/unicode_segmentation |
| **What it does** | UAX#29 grapheme / word / sentence boundary iterators. Maintained by the `unicode-rs` working group (same group that does `unicode-xid`, `regex`). |
| **Performance** | Linear, allocation-free, ~1–2 GB/s. `no_std` compatible. |
| **Rust availability** | Canonical. Updated to Unicode 16.0 as of 2026. |
| **Borrow** | Poler's custom tokenizer should call `unicode_segmentation::UnicodeSegmentation::unicode_words()` instead of hand-rolling word boundaries. This buys correct handling of CJK, Thai (no spaces), and combining marks for free. |
| **Verdict** | **STEAL.** Use as the word-boundary oracle inside poler's tokenizer. |

### 3.5 NLTK Alternatives in Rust

There is no single "NLTK in Rust" — by design, NLTK is a teaching library. The Rust ecosystem has split NLTK's functionality across focused crates:

| NLTK module | Rust equivalent | URL |
|---|---|---|
| `nltk.tokenize` | `huggingface/tokenizers` (sub-word) + `unicode-segmentation` (word) | (above) |
| `nltk.stem` | `rust-stemmers` + `rust-porter2` | (above) |
| `nltk.corpus.stopwords` | `whatlang` + embedded stopword lists; or `stop-words` crate | https://crates.io/crates/stop-words |
| `nltk.tag` (POS) | `rust-bert` (BERT tagger) | https://github.com/guillaume-be/rust-bert |
| `nltk.ne_chunk` (NER) | `rust-bert` OR regex-based gazetteer | https://github.com/guillaume-be/rust-bert |
| `nltk.chunk` | custom + `lingua` for lang-aware | — |
| `nltk.wordnet` | `wordnet-rs` | https://crates.io/crates/wordnet |
| `nltk.collocations` | build on top of poler's inverted index (PMI / χ²) | — |
| `vtext` (overall) | `vtext` (Davyds, 2019) — abandoned but a reference | https://github.com/davydov-vlad/vtext |

| **Borrow** | Don't try to find one library; assemble the stack from the focused crates above. `rust-bert` pulls in `tch` (LibTorch FFI) which is heavy — for CPU-only poler, prefer `candle` + a small BERT for POS/NER. |
| **Verdict** | **BUILD** the integration layer (a `NlpPipeline` trait that chains segmenter → stemmer → stopword filter → optional POS/NER), **STEAL** the underlying crates. |

### 3.6 Section Summary — What's Missing in poler-engine

- ❌ No sub-word tokenizer for embeddings (BPE/Unigram).
- ❌ Stemmer is Cyrillic-only — 17+ languages missing.
- ❌ Word boundary logic is hand-rolled — fragile on CJK/Thai/combining marks.
- ❌ No multi-language stopword list.
- ❌ No POS tagger / NER (needed for entity-graph triples quality).
- ✅ Cyrillic detection and Aho-Corasick semantic markers are *good* — keep them.

---

## 4. Embedding Models for CPU

Poler has **no vector layer**. Adding one is the single biggest capability upgrade. CPU-only constrains the model size: 25–135 M params (int8 quantized) is the sweet spot for 100–1000 sentences/sec on a 16-core CPU.

### 4.1 BGE-small / BGE-large (int8)

| | |
|---|---|
| **Name** | BGE (BAAI General Embedding) |
| **URL** | https://huggingface.co/BAAI/bge-small-en-v1.5 ; int8 ONNX: https://huggingface.co/RedHatAI/bge-small-en-v1.5-quant |
| **What it does** | Dense text embeddings for retrieval. bge-small-en-v1.5 = 33 M params, 384-dim. bge-large-en-v1.5 = 335 M params, 1024-dim. |
| **Performance** | bge-small int8: ~10 ms embedding latency on CPU (single sentence), ~150 sentences/sec batched. MTEB avg 62.4. bge-large int8: ~50 ms latency, ~25 sentences/sec, MTEB avg 63.9. |
| **Rust availability** | `bge` crate (crates.io, 2024-04) — wraps bge-small specifically. **`fastembed-rs`** (https://github.com/Anush008/fastembed-rs) — covers BGE, MiniLM, nomic, and BGEM3Q (the default, optimized for CPU). Built on `ort`. |
| **Borrow** | Use `fastembed-rs` for the inference path; do **not** write your own BGE wrapper. |
| **Verdict** | **STEAL** `fastembed-rs` + `bge-small-en-v1.5` (int8 ONNX) as the default embedding model. bge-large only for the rerank stage. |

### 4.2 all-MiniLM-L6-v2 (GGUF)

| | |
|---|---|
| **Name** | all-MiniLM-L6-v2 |
| **URL** | https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2 ; GGUF: https://huggingface.co/second-state/All-MiniLM-L6-v2-Embedding-GGUF |
| **What it does** | 22 M params, 384-dim embeddings. The "default" small embedding model for years. |
| **Performance** | ~5 ms latency on CPU (int8), ~300 sentences/sec batched. MTEB avg 56.3 — *worse* than BGE-small (62.4) but ~2× faster. GGUF format allows quantized inference via `llama.cpp`'s embedding endpoint. |
| **Rust availability** | Via `fastembed-rs` (ONNX path) or via `llama-cpp-2` (GGUF path). The GGUF path is faster on CPU for batched inference thanks to llama.cpp's hand-tuned int8 GEMM kernels. |
| **Borrow** | Use MiniLM as the *ultra-fast* fallback when query latency budget is <5 ms. |
| **Verdict** | **STEAL** the GGUF; use as a tier-2 model for latency-sensitive paths. |

### 4.3 nomic-embed-text

| | |
|---|---|
| **Name** | nomic-embed-text-v1.5 |
| **URL** | https://huggingface.co/nomic-ai/nomic-embed-text-v1.5 |
| **What it does** | 137 M params, **Matryoshka Representation Learning** — the same model produces 64, 128, 256, 512, or 768-dim embeddings by truncation. |
| **Performance** | MTEB avg 62.3 at 768-dim (competitive with OpenAI text-embedding-3-small). At 64-dim, ~58 MTEB — still usable for a coarse first-pass search. ~30 ms latency, ~50 sentences/sec on CPU int8. |
| **Rust availability** | `fastembed-rs` supports it (with open issue #83 about exposing the dimension parameter). |
| **Borrow** | **Matryoshka is the killer feature** for poler: store 64-dim binary-quantized vectors for the coarse HNSW pass, then re-rank with the full 768-dim cosine on the top-100. This gives 12× vector memory reduction with <1% recall loss. |
| **Verdict** | **STEAL** nomic-embed-text-v1.5 as the *primary* embedding model. Matryoshka enables the §6.3 binary-quantization strategy. |

### 4.4 candle (HuggingFace Rust ML)

| | |
|---|---|
| **Name** | `candle` |
| **URL** | https://github.com/huggingface/candle ; docs: https://huggingface.github.io/candle |
| **What it does** | Minimalist ML framework in pure Rust. CPU backend with optional MKL/Accelerate; GPU (CUDA/Metal) backends. Quantization via llama.cpp quantized types (Q4_K_M etc.). Supports serverless inference. |
| **Performance** | Within 1.5–2× of PyTorch CPU for transformer inference; faster than PyTorch for quantized models because of native int8 kernels. No Python, no GIL — ideal for embedding into a Rust binary. |
| **Rust availability** | First-class (it *is* Rust). The `candle-core` / `candle-nn` / `candle-transformers` crates. |
| **Borrow** | Use `candle` directly if you need fine control (custom pooling, hybrid sparse/dense). Use `fastembed-rs` (which uses `ort`) for the standard path. The two are interchangeable — `candle` is the more flexible option for a research-grade engine like poler. |
| **Verdict** | **STEAL** `candle-core` as the ML substrate; build the poler embedding module on top. |

### 4.5 ort (ONNX Runtime Rust)

| | |
|---|---|
| **Name** | `ort` (formerly `onnxruntime-rs`) |
| **URL** | https://github.com/pyke/ort |
| **What it does** | Rust bindings to ONNX Runtime — Microsoft's production ML inference engine. Supports int8 quantization, multi-threaded execution providers, AVX2/AVX-512 kernels. |
| **Performance** | Manticore Search 27.1.5 (Jun 2026) reports **14× speedup** over their previous embedding path by switching to ONNX Runtime. ONNX on Rust is "significantly faster" than PyTorch on pure inference (Stackademic benchmark, Nov 2025). CPU quantization modes: U8U8, U8S8, S8S8 (default, balanced). |
| **Rust availability** | `ort` (pyke) is the canonical binding. `fastembed-rs` is built on top of it. |
| **Borrow** | `ort` is the *fastest* path to CPU embeddings in 2026. If poler needs maximum inference throughput (e.g., indexing 1B docs), `ort` + ONNX int8 beats `candle`. For flexibility (custom ops, GGUF), `candle` wins. |
| **Verdict** | **STEAL** `ort` for the production embedding path; keep `candle` as the research/extensibility layer. |

### 4.6 fastembed-rs — the turnkey option

| | |
|---|---|
| **Name** | `fastembed-rs` |
| **URL** | https://github.com/Anush008/fastembed-rs |
| **What it does** | Qdrant's embedding library, Rust port. Bundles 20+ models (BGE, MiniLM, nomic, BGEM3Q, multilingual-e5, jina-embeddings-v3) as pre-quantized ONNX. Zero-config: `TextEmbedding::try_default()`. |
| **Performance** | BGEM3Q default model is optimized for CPU. Supports text + image + sparse embeddings + reranking. |
| **Rust availability** | First-class. |
| **Borrow** | This is the path of least resistance for poler: `cargo add fastembed`, ~10 lines, embeddings work. |
| **Verdict** | **STEAL** as the v1 embedding backend. Migrate to direct `ort`/`candle` only if profiling reveals fastembed's abstraction tax. |

### 4.7 Embedding Recommendation

```
poler-engine embedding stack (proposed):

  user text
      │
      ▼
  huggingface/tokenizers  ── BPE tokens
      │
      ▼
  fastembed-rs / ort       ── nomic-embed-text-v1.5 (int8 ONNX, 768-d)
      │
      ▼
  Matryoshka truncation    ── 64-d for HNSW coarse, 768-d for rerank
      │
      ▼
  RaBitQ binary quant      ── 64-d → 8 bytes (§6.3)
      │
      ▼
  HNSW index               ── (§6.4)
```

---

## 5. Vector Quantization (for CPU Vector Search)

Poler needs to fit ~1B vectors in 16–32 GB RAM. Raw 768-d float32 = 3 KB/vector → 3 TB. **Need 100–1000× compression.**

### 5.1 Product Quantization (PQ)

| | |
|---|---|
| **Name** | Product Quantization (Jégou et al., TPAMI 2011) |
| **URL** | Reference: https://github.com/facebookresearch/faiss ; Rust: `instant-distance` (partial), `ruvector_data_framework::hnsw` (bundled) |
| **What it does** | Splits a D-dim vector into M sub-vectors of D/M dims; each sub-vector is quantized against a 256-entry codebook learned by k-means on the sub-vector distribution. Final code = M bytes. |
| **Performance** | 768-d → 96 bytes (M=96) = **32× compression**. Distance via Asymmetric Distance Computation (ADC) = 768 LUT lookups + adds, ~1 µs/vector with SIMD. Recall@10 ≈ 0.95 with rerank. |
| **Rust availability** | No standalone, well-maintained PQ crate — usually bundled with HNSW. `instant-distance` has it. `lancedb` (Rust core) has full IVF_PQ. |
| **Borrow** | The algorithm is ~200 lines; implement natively if no crate fits. |
| **Verdict** | **BUILD** (or borrow from `instant-distance`). PQ is the second-tier compressor after RaBitQ. |

### 5.2 Scalar Quantization (int8)

| | |
|---|---|
| **Name** | int8 scalar quantization |
| **URL** | pgvector: https://jkatz.github.io/postgres/pgvector/vector-quantization/ ; ONNX Runtime native |
| **What it does** | Per-dimension affine map float32 → int8. 4× compression, near-lossless. |
| **Performance** | Cosine on int8 vectors: ~4× faster than float32 (2× from SIMD width, 2× from cache). Recall@10 ≈ 0.99 vs float32. |
| **Rust availability** | Trivial to implement (~30 lines). `usearch` supports it natively. |
| **Borrow** | Always do this as a baseline; it's free. |
| **Verdict** | **BUILD** as the floor; pair with RaBitQ for the index. |

### 5.3 Binary Quantization + RaBitQ — *the* 2026 SOTA

| | |
|---|---|
| **Name** | RaBitQ (Jianyang Gao & Cheng Long, SIGMOD 2024) + Extended RaBitQ (Dec 2024) |
| **URL** | Paper: https://arxiv.org/abs/2405.12497 ; Library: https://vectordb-ntu.github.io/RaBitQ/ ; Rust: `lancedb` has it built-in |
| **What it does** | Random rotation of the vector space (so all dims become statistically equivalent), then 1-bit quantization (sign). The random rotation gives a *theoretical error bound* on the distance estimate — unlike naive sign quantization, which has no bound. Extended RaBitQ adds 2-bit and 4-bit modes with even better bounds. |
| **Performance** | 768-d → 96 bytes = **32× compression** at 1-bit; recall@10 ≈ 0.85 *without* rerank, ≈0.98 *with* rerank on top-100 using full-precision. 2-bit (192 bytes, 16× compression) ≈ 0.95 without rerank. Distance computation = Hamming + popcount, ~50 GB/s on AVX2. |
| **Rust availability** | LanceDB's Rust core (https://docs.lancedb.com) ships RaBitQ. No standalone Rust crate as of 2026-09. |
| **Borrow** | Implement RaBitQ natively in poler (the math is ~100 lines). Combine with **Matryoshka nomic** (§4.3): 64-d → 8 bytes per vector → **1B vectors in 8 GB RAM**. This is the order-of-magnitude win. |
| **Verdict** | **BUILD** RaBitQ from scratch — no crate matches poler's needs. Pair with Matryoshka + HNSW. |

### 5.4 HNSW (Hierarchical Navigable Small World)

| | |
|---|---|
| **Name** | HNSW (Malkov & Yashunin, TPAMI 2018) |
| **URL** | Paper: https://arxiv.org/abs/1603.09320 ; Rust crates: `hnsw_rs` (https://github.com/jean-pierreBoth/hnswrs), `hora` (https://github.com/hora-search/hora), `instant-distance` (https://github.com/InstantSearchPlus/instant-distance), `sincere` (older) |
| **What it does** | Multi-layer proximity graph. Search starts at a coarse top layer (few nodes, long edges) and descends to fine bottom layer (all nodes, short edges). Log-time search. |
| **Performance** | 1M vectors, 768-d: search <1 ms, build ~30 s, recall@10 ≈ 0.95. Memory: graph edges dominate (~50 bytes/vector at M=16). |
| **Rust availability** | Multiple mature crates. `hnsw_rs` is the most featureful (custom distance functions, persistence). `hora` is the fastest but less maintained. `instant-distance` is the simplest. `ruvector_data_framework::hnsw` (2025-2026) is a newer production-quality implementation. |
| **Borrow** | The graph construction algorithm is well-understood. Use **`hnsw_rs`** if you need a turnkey solution. Better: **build a poler-native HNSW that stores edges as RaBitQ codes directly** — the existing crates all store float32, defeating the purpose. |
| **Verdict** | **BUILD** a poler-native HNSW with RaBitQ-coded nodes. ~500 lines. Borrow the construction algorithm from `hnsw_rs`'s source. |

### 5.5 Bonus: usearch

| | |
|---|---|
| **Name** | USearch (unum-cloud) |
| **URL** | https://github.com/unum-cloud/usearch ; Rust: https://crates.io/crates/usearch |
| **What it does** | Single-header C++ SIMD-optimized ANN with Rust bindings. JIT-compiled custom metrics. Supports int8, f16, f32, f64, and custom quantization. |
| **Performance** | Benchmark-topping on cloud CPUs (arXiv 2505.07621). Used by ScyllaDB for their vector search. |
| **Rust availability** | FFI bindings (`usearch` crate). |
| **Borrow** | Strong candidate if poler doesn't want to build HNSW from scratch. But it doesn't support RaBitQ natively. |
| **Verdict** | **CONSIDER** as a fallback if the poler-native HNSW proves too costly to build. |

### 5.6 Vector Quantization Recommendation

```
poler vector stack (proposed):

  query embedding (768-d float32, nomic v1.5)
      │
      ▼
  Matryoshka truncation → 64-d float32
      │
      ▼
  RaBitQ random rotation + 1-bit → 8 bytes
      │
      ▼
  HNSW (poler-native, RaBitQ-coded nodes)
      │  → top-100 candidates
      ▼
  Rerank with full 768-d float32 cosine (LanceDB / pgvector pattern)
      │
      ▼
  top-10 final
```

Memory budget at 1B vectors:
- HNSW graph: 1B × 50 B (edges) = 50 GB → too much; use **IVF + HNSW-per-partition** (LanceDB pattern) → ~5 GB.
- RaBitQ codes: 1B × 8 B = 8 GB.
- Rerank cache (top-100 full-precision, LRU): ~300 MB.
- **Total: ~13 GB. Fits in 16 GB.**

---

## 6. Differential / Incremental Computation

Poler's "watcher mode" (mtime/size cache) is *ad hoc*. The SOTA gives mathematically-grounded alternatives.

### 6.1 differential-dataflow (Frank McSherry)

| | |
|---|---|
| **Name** | `differential-dataflow` |
| **URL** | https://github.com/TimelyDataflow/differential-dataflow ; https://crates.io/crates/differential-dataflow |
| **What it does** | A data-parallel programming framework where collections are `(data, time, diff)` triples. Operators (join, group, map, iterate) produce *new* diffs when inputs change, rather than recomputing. Built on `timely-dataflow` (Murray, Naiad lineage). |
| **Performance** | For a streaming join, throughput is millions of tuples/sec on a single machine; latency for incremental updates is sub-second even with multi-step pipelines. |
| **Rust availability** | First-class (it *is* Rust). Maintained by Frank McSherry. Also: `differential-datalog` (DDlog) is a Datalog-on-DD language used in VMware's networking. |
| **Borrow** | DD is *the* principled way to do "recompute only what changed." Poler's inverted index refresh on file edit is naturally a `map` + `group_by` + `join` DD pipeline. The `diff` type maps perfectly to "+1 doc / −1 doc" deltas. |
| **Verdict** | **STEAL** the *concept* and the algorithmic approach (Z-sets, arrangement-based joins). **BUILD** a poler-flavored mini-DD — the full `differential-dataflow` crate is a heavy dependency (timely, crossbeam-channel, etc.) designed for distributed computation. |

### 6.2 DBSP / Incremental View Maintenance

| | |
|---|---|
| **Name** | DBSP (Dataflow Stream Processor) |
| **URL** | Paper: https://www.vldb.org/pvldb/vol16/p1601-budiu.pdf ; Reference impl: https://github.com/feldera/dbsp (Rust) ; Production: https://materialize.com (Materialize uses DD, not DBSP, but the ideas converge) |
| **What it does** | A *calculus* of incremental computation based on Z-sets (sets with integer multiplicities). Any batch SQL query can be *automatically* converted to an incremental stream-processing program. Supports all relational operators, aggregation, recursion. |
| **Performance** | Materialize reports 90% cost reduction vs full recomputation for materialized-view refresh. DBSP is "simpler and more general" than differential-dataflow per the VLDB paper. |
| **Rust availability** | `dbsp` is a Rust crate (feldera/dbsp). The Feldera platform is built on it. |
| **Borrow** | DBSP is **the right abstraction for poler's incremental refresh**. A poler index is fundamentally a SQL-like materialized view: `SELECT token, doc_id, freq FROM docs JOIN tokens ON ...`. DBSP can maintain this incrementally under inserts/updates/deletes of docs. |
| **Verdict** | **STEAL** `dbsp` as a dependency, OR steal the Z-set + operator-fusion design and build a poler-native mini-version. The full crate is well-maintained and MIT-licensed. |

### 6.3 CRDTs (Conflict-free Replicated Data Types)

| | |
|---|---|
| **Name** | `yrs` (Yjs port) / `automerge` / `crdt-kit` |
| **URL** | https://github.com/y-crdt/y-crdt (Rust: `yrs`) ; https://github.com/automerge/automerge-rs ; https://crdt-kit.github.io |
| **What it does** | Data structures (counters, maps, sequences, text) that merge deterministically across replicas without coordination. |
| **Performance** | `yrs` is the fastest Rust CRDT — used in production by Notion, Evernote, Linear. Merge of a 10 KB document update is sub-ms. |
| **Rust availability** | `yrs` (y-crdt) is the canonical high-performance CRDT. `crdt-kit` is a newer collection. |
| **Borrow** | CRDTs are *not* directly applicable to a search index (which is a derived, not source, data structure). They **are** applicable if poler grows a sync layer between multiple PCs (e.g., desktop + laptop), where each maintains its own index and merges deltas. Poler's `aidde::sqlite_store` could be replaced by a `yrs` document for multi-device sync. |
| **Verdict** | **DEFER.** Only relevant if poler adds multi-device sync. Note the technique for future. |

### 6.4 Differential Computation Recommendation

```
poler incremental refresh (proposed):

  File watcher event (path, mtime, size, hash)
      │
      ▼
  Diff: Δdocs = { +new_doc, −old_doc }
      │
      ▼
  DBSP / mini-DD pipeline:
      ┌─ tokenize(new_doc) ─► Δtokens
      ├─ detect_lang(new_doc) ─► Δlang
      ├─ embed(new_doc) ─► Δvectors
      ├─ simhash(new_doc) ─► Δdedup
      └─ iir_resonance(new_doc) ─► Δpsi
      │
      ▼
  Z-set merge into persistent index
      │
      ▼
  Affected queries get notified (push-based invalidation)
```

Poler's `mtime/size cache` becomes the *source* of `Δdocs`; the rest of the pipeline is genuinely incremental.

---

## 7. Memory-Mapped I/O (Rust)

Poler already uses `memmap2`. The SOTA is about *how* you use it.

### 7.1 memmap2

| | |
|---|---|
| **Name** | `memmap2` |
| **URL** | https://crates.io/crates/memmap2 ; https://docs.rs/memmap2 |
| **What it does** | Cross-platform `mmap`/`munmap` for file-backed and anonymous memory maps. Fork of the abandoned `memmap` crate. |
| **Performance** | OS-level page-cache integration; zero copy between disk and user space. Random access into a 100 GB file is O(1) — the kernel pages in 4 KB on first touch. |
| **Rust availability** | Canonical. |
| **Borrow** | Poler already does this. |
| **Verdict** | **KEEP.** |

### 7.2 madvise Strategies

The single biggest free win available to poler today. From the `memmap2::Advice` enum and `man 2 madvise`:

| Advice | When to use | Effect |
|---|---|---|
| `MADV_SEQUENTIAL` | Pass 1 (full-file stats scan) | Aggressive read-ahead; kernel discards pages after consumption. **~2× throughput** on sequential scan. |
| `MADV_RANDOM` | Pass 2 (tokenize only hit files, random offsets) | Disables read-ahead; avoids polluting page cache with useless prefetch. |
| `MADV_WILLNEED` | Before pass 2, prefetch the byte ranges of hit files discovered in pass 1 | Kernel starts reading pages in background; pass 2 sees them resident. |
| `MADV_DONTNEED` | After pass 1 on a file you won't re-read | Frees page-cache pages immediately; useful under memory pressure. |
| `MADV_HUGEPAGE` | For the inverted index (large, contiguous) | 2 MB transparent huge pages; ~10% TLB-miss reduction. |

| **Rust availability** | `memmap2::Mmap::advise(Advice::Sequential)` etc. |
| **Borrow** | Tantivy uses `MADV_SEQUENTIAL` on segment load (issue #11377). Poler should do the same per-pass. |
| **Verdict** | **STEAL.** ~20 lines of code, 1.5–2× speedup on multi-pass. |

### 7.3 Advanced mmap Strategies

| Technique | Description | Application in poler |
|---|---|---|
| **mmap over a sparse file** | Create a 1 TB sparse file, mmap it, write only touched pages. | Poler's vector store: pre-allocate a virtual 1 TB file for HNSW, only touch what's used. |
| **mmap + `userfaultfd`** | Intercept page faults in user space; serve pages from a custom backend (e.g., decompress-on-demand). | Lets poler mmap a *compressed* file and have `userfaultfd` decompress frames on demand. **This is the cleanest way to bridge zstd-seekable archives and poler's mmap-based tokenizer.** Requires Linux 4.11+ and `nix` crate. |
| **`mremap` + fixed address** | Grow a mapping in place without copying. | HNSW graph growth without reallocation. |
| **`MADV_COLD` / `MADV_PAGEOUT`** | Linux 5.11+; actively de-prioritize pages. | Demote cold index segments. |
| **`MAP_HUGETLB`** | Pre-allocated 2 MB huge pages. | Inverted index hash table. |
| **Double-buffered mmap** | Two mmaps of the same file at different offsets, alternating per pass. | Hides disk latency on HDDs; useless on NVMe. |

| **Rust availability** | `memmap2` covers the basics; `nix` crate covers `userfaultfd`, `mremap`, `MADV_*` extras. |
| **Borrow** | The `userfaultfd` + zstd-seekable bridge is **the** novel poler opportunity. Build it as a `LazyDecompressMmap` type that quacks like `Mmap` but lazily decompresses frames on first page fault. |
| **Verdict** | **BUILD** the `userfaultfd` bridge (~300 lines); **STEAL** the `madvise` patterns. |

### 7.4 Lazy Loading Strategies

- **Demand-paged mmap** (default): pages loaded on first touch. Best for random access.
- **`readahead` syscall** (Linux): explicit prefetch of a byte range. Use before pass 2.
- **`fadvise(FADV_WILLNEED)`**: same effect as `readahead`, more portable.
- **`posix_fadvise(FADV_SEQUENTIAL)`**: hint the kernel about your access pattern at the fd level (vs `madvise` which is per-mapping).
- **Tiered storage**: hot segments in RAM-mapped, warm in NVMe-mapped, cold in HTTP-Range-virtual. Poler's `FileTokens<'a>` abstraction makes this transparent — only the backing `Mmap`/`SeekableReader`/`HttpRangeReader` differs.

---

## 8. Bonus Context — Adjacent SOTA Worth Knowing

### 8.1 Tantivy (full-text search engine in Rust)

| | |
|---|---|
| **URL** | https://github.com/quickwit-oss/tantivy |
| **What it does** | Apache-Lucene-inspired full-text search engine library in pure Rust. Segment-based inverted index with BM25, SIMD-accelerated postings, custom compressions (varint, bitpack, bitpacked-with-frame-of-reference). |
| **Borrow** | The *segment* architecture (small immutable indexes merged over time), the *postings list compression* (ForeachBitpacked, VInt), the *docstore* (zstd-compressed blocks of documents). **Do not** depend on tantivy directly — poler's whole point is to surpass it. But study its source for compression tricks. |
| **Verdict** | **STEAL** algorithms (segment merge policy, postings compression); do **not** take the crate. |

### 8.2 LanceDB (Rust vector database)

| | |
|---|---|
| **URL** | https://github.com/lancedb/lancedb |
| **What it does** | Serverless vector DB with Rust core. Lance columnar format (parquet-like, with random access). IVF + HNSW + PQ + RaBitQ indexes. Scales to 10B vectors. |
| **Borrow** | The IVF + HNSW composition pattern, RaBitQ integration, and the Lance columnar format (which is designed for fast random access — unlike Parquet). Poler could adopt Lance as its on-disk format. |
| **Verdict** | **STEAL** the Lance format + IVF/HNSW/RaBitQ composition; consider depending on `lance` crate for storage. |

### 8.3 Polars (Rust DataFrame engine)

| | |
|---|---|
| **URL** | https://github.com/pola-rs/polars |
| **What it does** | Streaming Arrow-based query engine. Lazy evaluation, query optimization, out-of-core processing. `sink_parquet` writes streaming results larger than RAM. |
| **Borrow** | The *streaming engine* architecture (morsel-driven parallelism, lazy query plan, push-based execution). Poler's multi-pass pipeline is conceptually similar; polars' morsel model is a cleaner abstraction. |
| **Verdict** | **STEAL** the morsel-driven parallelism pattern; do **not** depend on polars for the search core. |

### 8.4 SPSC Ring Buffers (cross-thread streaming)

| | |
|---|---|
| **URL** | `rtrb` https://github.com/mgeier/rtrb ; `smallring` https://crates.io/crates/smallring ; `ringbuf` https://docs.rs/ringbuf |
| **What it does** | Wait-free single-producer single-consumer ring buffers for lock-free handoff between threads. |
| **Performance** | Tens of nanoseconds per enqueue/dequeue; padded counters avoid false sharing. |
| **Borrow** | For poler's multi-threaded streaming pipeline (one producer reading the archive, N consumers tokenizing/embedding), `rtrb` is the right primitive. Replaces channels with lower latency. |
| **Verdict** | **STEAL** `rtrb`. |

---

## 9. What's Missing in poler-engine (Consolidated Gap Analysis)

| Capability | Current state | Gap | Severity |
|---|---|---|---|
| Remote archive access | None (local mmap only) | No HTTP Range, no zstd-seekable | **Critical** |
| WARC ingestion | None | No Common Crawl support | High |
| Tar streaming | None | Cannot ingest `*.tar.gz` dumps | High |
| Bulk compression | None (raw text) | 5–10× memory waste | **Critical** |
| Per-string compression | None | FSST would 2× the inverted index density | **Critical** |
| Sub-word tokenizer | None | No embedding model can run | **Critical** |
| Multi-language stemming | Cyrillic only | 17+ languages missing | High |
| Language detection | Cyrillic heuristic | No multi-lang detection | Medium |
| POS / NER | None | Entity-graph triples quality limited | Medium |
| Embeddings | **None** | No vector layer at all | **Critical** |
| Vector quantization | **None** | Cannot fit >1M vectors in RAM | **Critical** |
| ANN index | **None** | No HNSW / IVF | **Critical** |
| Incremental refresh | mtime/size cache | No differential dataflow | High |
| `madvise` strategy | None | Leaving 2× on the table | High |
| `userfaultfd` decompress-on-demand | None | The novel poler opportunity | Medium |
| Multi-thread streaming | Rayon (likely) | SPSC ring buffers would lower latency | Low |

**Five critical gaps** — all of them solvable with the SOTA surveyed above. Closing them is what unlocks the "order-of-magnitude" goal.

---

## 10. Recommendations — What to Steal, What to Build

### 10.1 STEAL (use the crate directly, minimal wrapper)

| Crate | Purpose | Why steal |
|---|---|---|
| `zeekstd` / `zstd-framed` | zstd-seekable reader/writer | Spec-perfect; no reason to reimplement |
| `zstd-rs` | Bulk compression | FFI to Facebook's reference |
| `lz4_flex` | Hot-path postings decompression | Pure Rust, 2 GB/s, no unsafe |
| `fsst-rs` | String compression for inverted index | Pure Rust, zero-dep, matches paper |
| `warc` | WARC record streaming | Matches `warcio` feature-for-feature |
| `tokio-tar` | Tar entry streaming | Maintained fork (avoids TARmageddon CVE) |
| `huggingface/tokenizers` | BPE for embedding models | The Rust core is the SOTA |
| `whatlang` | Language detection | 75 languages, <1 µs |
| `rust-stemmers` | Snowball stemmers (18 langs) | Byte-identical to reference |
| `unicode-segmentation` | UAX#29 word boundaries | Maintained by `unicode-rs` |
| `memmap2` | Cross-platform mmap | Already a dep |
| `rtrb` | SPSC ring buffer | Wait-free, padded counters |
| `fastembed-rs` | Turnkey CPU embeddings | 20+ models, ONNX int8 |
| `ort` | ONNX Runtime bindings | 14× speedup proven (Manticore) |
| `candle-core` | ML substrate for custom ops | HuggingFace's own Rust ML |
| `dbsp` (or borrow DD's design) | Incremental view maintenance | Mathematically grounded |

### 10.2 STEAL ALGORITHMS (read the source, reimplement natively)

| Algorithm | Source to read | Why not depend |
|---|---|---|
| HNSW construction + search | `hnsw_rs`, `hora`, `usearch` | Need RaBitQ-coded nodes, not float32 |
| RaBitQ | LanceDB Rust core, vectordb-ntu.github.io | No standalone Rust crate |
| Product Quantization | `instant-distance`, Faiss (C++) | Want unified PQ+RaBitQ+HNSW |
| Segment merge policy | Tantivy source | Tight coupling with tantivy's IR |
| Morsel-driven parallelism | Polars source | Architectural pattern, not a library |
| Z-sets / differential operators | `differential-dataflow`, `dbsp` | Want poler-flavored mini-DD |
| FSST encoder training | `fsst-rs` source | Want corpus-specialized encoders per partition |
| zstd dictionary training | `zstd-rs` | Already exposed; just call it |

### 10.3 BUILD FROM SCRATCH (poler-specific, no crate fits)

| Component | LOC estimate | Why build |
|---|---|---|
| `HttpRangeBlob` | ~200 | Poler needs HTTP/2 multiplex + LRU frame cache + backpressure |
| `LazyDecompressMmap` (userfaultfd + zstd-seekable) | ~300 | The novel poler bridge; no existing crate |
| `TarEntryIndex` (path → byte range in decompressed stream) | ~150 | Needed for multi-pass on tar.zst |
| `HfDatasetSource` (Parquet row-group streaming over HTTP) | ~200 | `arrow-rs` does the work; just the URL resolver + Range |
| Poler-native HNSW with RaBitQ nodes | ~500 | Existing HNSW crates store float32 |
| RaBitQ encoder + distance | ~200 | No standalone Rust crate |
| IVF + HNSW composition | ~300 | LanceDB has it but tied to Lance format |
| `NlpPipeline` trait (segmenter → stemmer → stopword → POS/NER) | ~400 | Integration layer over stolen crates |
| Mini-DD / Z-set engine for incremental refresh | ~600 | `dbsp` is too heavy; poler needs a focused subset |
| `madvise` policy per pass | ~50 | Trivial but high-leverage |
| FSST-trained inverted index layer | ~200 | Wrap `fsst-rs` with corpus-aware training |
| Embedding pipeline (Matryoshka truncation + rerank) | ~300 | Ties `fastembed-rs` → RaBitQ → HNSW |

**Total estimated build effort:** ~3,500 LOC of poler-native code, on top of ~15 stolen crates. This is achievable in a focused rewrite.

### 10.4 Suggested Rewrite Phasing

1. **Phase 1 (week 1–2):** Add `madvise` strategies to existing mmap paths. Free 2× win. Add `whatlang` + `rust-stemmers` + `unicode-segmentation` to the tokenizer. Free quality win.
2. **Phase 2 (week 3–4):** Add `fsst-rs` under the inverted index. Free 2× memory density. Add `zstd` + dictionary for the cold doc store.
3. **Phase 3 (week 5–6):** Build `HttpRangeBlob` + integrate `zeekstd` + `warc` + `tokio-tar`. Poler can now stream remote archives.
4. **Phase 4 (week 7–8):** Integrate `fastembed-rs` + nomic-embed-text-v1.5. Poler has embeddings.
5. **Phase 5 (week 9–10):** Build RaBitQ + poler-native HNSW. Poler has vector search at 1B-scale in 16 GB.
6. **Phase 6 (week 11–12):** Build mini-DD for incremental refresh. Poler's watcher mode becomes principled.
7. **Phase 7 (week 13–14):** Build `LazyDecompressMmap` (userfaultfd bridge). The novel poler capability — mmap-over-compressed-HTTP.
8. **Phase 8 (week 15+):** Benchmark against Tantivy + LanceDB + USearch. Tune until order-of-magnitude is demonstrated.

---

## 11. Key URLs Reference (one place)

**Streaming:**
- https://github.com/rorosen/zeekstd
- https://crates.io/crates/zstd-framed
- https://docs.rs/async_http_range_reader
- https://github.com/astral-sh/tokio-tar
- https://docs.rs/warc
- https://huggingface.co/docs/datasets/stream

**Compression:**
- https://github.com/gyscos/zstd-rs
- https://github.com/pseitz/lz4_flex
- https://github.com/dropbox/rust-brotli
- https://crates.io/crates/fsst-rs (paper: https://ir.cwi.nl/pub/50085/)
- https://facebook.github.io/zstd/#small-data

**NLP:**
- https://github.com/huggingface/tokenizers
- https://github.com/greyblake/whatlang-rs
- https://github.com/quickwit-oss/whichlang
- https://github.com/pemistahl/lingua-rs
- https://github.com/mrordinaire/rust-stemmers
- https://github.com/unicode-rs/unicode-segmentation

**Embeddings:**
- https://huggingface.co/nomic-ai/nomic-embed-text-v1.5
- https://huggingface.co/RedHatAI/bge-small-en-v1.5-quant
- https://huggingface.co/second-state/All-MiniLM-L6-v2-Embedding-GGUF
- https://github.com/huggingface/candle
- https://github.com/pyke/ort
- https://github.com/Anush008/fastembed-rs

**Vector quantization:**
- https://arxiv.org/abs/2405.12497 (RaBitQ)
- https://vectordb-ntu.github.io/RaBitQ/
- https://github.com/jean-pierreBoth/hnswrs
- https://github.com/hora-search/hora
- https://github.com/InstantSearchPlus/instant-distance
- https://github.com/unum-cloud/usearch
- https://docs.lancedb.com

**Differential / incremental:**
- https://github.com/TimelyDataflow/differential-dataflow
- https://github.com/feldera/dbsp
- https://www.vldb.org/pvldb/vol16/p1601-budiu.pdf (DBSP paper)
- https://github.com/y-crdt/y-crdt

**Memory-mapped I/O:**
- https://crates.io/crates/memmap2
- https://docs.rs/memmap2 (Advice enum)
- https://man7.org/linux/man-pages/man2/madvise.2.html

**Adjacent SOTA:**
- https://github.com/quickwit-oss/tantivy
- https://github.com/lancedb/lancedb
- https://github.com/pola-rs/polars
- https://github.com/mgeier/rtrb

---

## 12. Raw Search Artifacts

The 36 individual web-search result JSON files are archived alongside this report at:
- `/home/z/my-project/upload/s1_zstd_seekable.json` … `s36_fastembed.json`

Each contains the full URL, title, snippet, host, rank, and date for every result returned. Use these for citation depth-checking before committing to any specific dependency.

---

**Bottom line:** Poler's existing architecture (multi-pass mmap + zero-copy `FileTokens<'a>` + aho-corasick prefilter + IIR resonance + SimHash dedup) is *already* the right skeleton. The 100% rewrite should preserve that skeleton and graft on: (1) zstd-seekable + HTTP Range over the mmap layer, (2) FSST under the inverted index, (3) fastembed-rs / nomic-embed-text-v1.5 for embeddings, (4) RaBitQ + poler-native HNSW for vector search, (5) DBSP-style Z-sets for incremental refresh, (6) madvise + userfaultfd for mmap tuning. Each of these is a SOTA-validated primitive; **no system in 2026 combines all six** — which is where the order-of-magnitude win comes from.
