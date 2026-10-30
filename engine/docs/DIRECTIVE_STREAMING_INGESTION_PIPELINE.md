# 🌐📦 STRATEGIC DIRECTIVE: SOVEREIGN INGESTION PIPELINE — STREAMING BROWSER + ZERO-DISK ARCHIVER + TRIT5 MEMORY FUSION

> [!IMPORTANT]
> **CRITICAL ARCHITECTURAL DIRECTIVE FOR CONTINUOUS AI INGESTION & TRAINING (v0.39.0+)**
> **TARGET OBJECTIVE**: Ingest web datasets, huge codebases, and massive archives (>100 GiB) on restricted disk systems (<10 GiB) by streaming directly from HTTP/sockets into `.poler` chunked containers and quantum Trit5 `.t5c` semantic memory crystals without intermediate uncompressed disk landing.

---

## 1. ARCHITECTURAL PARADIGM: ZERO-DISK LANDING STREAMING
Traditional ingestion models fail because downloading and extracting a 50 GiB dataset requires:
1. 50 GiB raw download file.
2. 100+ GiB uncompressed folder.
3. RAM spikes leading to OOM / disk full errors.

**The POLER Sovereign Pipeline eliminates disk landing completely:**
```
[ HTTP / Web / Git / Sockets ] (Chunked Network Stream)
             │
             ▼
  [ Sovereign Streaming Parser & Noise Filter ] (src/browser/filter.rs + dom_tree.rs)
             │
             ▼
   [ FastCDC Content-Defined Chunking & BLAKE3 Dedup ] (src/archive/dedup.rs)
             │
             ├──► [ Compressed Zero-Copy Chunk Store (.poler container) ] (5-10x compression on disk)
             │
             └──► [ StreamCrystalBuilder: Quantum Causal Graph (.t5c) ] (Real-time memory crystal ingestion)
```

---

## 2. KEY SUBSYSTEM REQUIREMENTS

### A. Sovereign Browser Streaming Ingestion (`src/browser/` + `src/triune/ingest.rs`)
- **Direct HTTP/HTTPS Chunked Reader**: Process raw HTML/JSON/Markdown streams on-the-fly without holding whole responses in RAM.
- **$\varepsilon$-Entropy Filter & Semantic Extraction**:
  - Automatically strip JavaScript bloat, ads, tracker tokens, and boilerplate CSS.
  - Retain pure text, code snippets, structured tables, and causal links.
- **Deep Recursive Crawler Discipline**:
  - Follow links with strict deduplication using in-memory Bloom filter / BLAKE3 URL hashes.
  - Stream parsed DOM nodes directly into `StreamCrystalBuilder`.

### B. Sovereign Archiver / Container Core (`src/archive/`)
- **Zero-Copy Random Access (`.poler` / `.t5z`)**:
  - Index header/jump-table with $O(1)$ sub-file offset lookup.
  - Multi-tier compression: Fast ZSTD streaming frames + Trit5 sparse matrix encoding.
- **Virtual Stream Ingestion (`--stream-ingest`)**:
  - Compress network stream on-the-fly into `.poler` archive chunks in 64 KB – 1 MB blocks.
  - A 100 GiB incoming stream occupies only ~5–8 GiB of compressed, deduplicated disk blocks.
- **In-Memory VFS Mounting**:
  - Ability for `poler-edit`, `kate-poler`, and LLM agents to read files inside `.poler` containers via `mmap` with $< 1\text{ ms}$ latency without unpacking.

---

## 3. UNIFIED CLI PROTOCOL & COMMAND SPECIFICATION

```bash
# 1. Download & Auto-Compress 100 GiB Stream directly to .poler without full uncompressed landing
poler-engine --stream-download "https://example.com/huge-corpus.tar" --output-archive corpus.poler --dedup

# 2. Web Crawl + Auto-Ingest into Trit5 Permanent AI Memory Crystal (.t5c)
poler-engine --browser-crawl "https://docs.kernel.org" --max-depth 3 --ingest-to-crystal ~/.poler/permanent_memory.t5c

# 3. Stream .poler archive directly into Trit5 neural memory
poler-engine --archive-to-crystal corpus.poler --crystal ~/.poler/permanent_memory.t5c --workers auto

# 4. Instant Zero-Copy Inspection of .poler container (O(1) header read)
poler-pack list corpus.poler --json

# 5. Open any file inside .poler directly in poler-edit GUI
poler-edit "poler://corpus.poler/src/kernel/main.rs"
```

---

## 4. CODEBASE STRUCTURE & IMPLEMENTATION TARGETS

Implement and wire the following modules in `poler-engine`:
- `src/archive/mod.rs` — Unified archive interface and `.poler` format definition.
- `src/archive/stream_writer.rs` — Streaming chunked compressor with bounded RSS ($\le 32\text{ MB}$).
- `src/archive/reader.rs` — Zero-copy Random Access `mmap` container reader.
- `src/archive/dedup.rs` — FastCDC content-defined chunking + BLAKE3 deduplication registry.
- `src/browser/stream_ingest.rs` — Streaming DOM tokenizer & $\varepsilon$-entropy filter pipe.
- `src/triune/ingest.rs` — Wire streaming parser output directly into `StreamCrystalBuilder` (`.t5c`).
- `tests/stream_ingest_e2e.rs` — End-to-end tests: stream 10 GiB synthetic data on <1 GiB free disk, verify 100% lossless extraction and valid memory crystal creation.

---

## 5. PERFORMANCE ACCEPTANCE CRITERIA
1. **Zero Intermediate Disk Space**: At no point shall uncompressed stream bytes touch the physical filesystem.
2. **Bounded RAM Footprint**: Peak RSS $\le 48\text{ MB}$ during multi-gigabyte continuous ingestion.
3. **Lossless Recovery**: Extracting any file from `.poler` must match original SHA256 exactly.
4. **Instant Ingestion**: Token extraction and causal graph nodes must be written into `.t5c` in real time with $> 100\text{ MB/s}$ throughput.
