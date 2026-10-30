# 📦 DIRECTIVE: IMPLEMENT SOVEREIGN ARCHIVE & CONTAINER ENGINE (`poler-pack` / `src/archive/`)

> [!IMPORTANT]
> **CRITICAL ARCHITECTURAL DIRECTIVE — SOVEREIGN CONTAINER & ARCHIVE SYSTEM**
> **TARGET VERSION**: POLER Engine v0.39.0+
> **COMPATIBILITY**: Sovereign Browser, Piece-Tree Editor, Trit5 Continuous Ingestion Memory Crystal (`.t5c`).

---

## 1. PURPOSE & ARCHITECTURAL OBJECTIVE
Design and implement a next-generation high-performance archive & container format (`.poler` / `.t5z`) to completely replace outdated ZIP, TAR, and 7Z formats. 
The tool MUST exist in two forms:
1. **Core Module in POLER Engine (`src/archive/`)**: Native API integration with Piece-Tree buffer (`src/editor/`), Sovereign Browser (`src/browser/`), and Trit5 memory crystals (`src/triune/`).
2. **Standalone Standout CLI Tool (`poler-pack` & `poler-unpack`)**: High-speed, standalone Unix utility with zero runtime dependencies.

---

## 2. KEY ARCHITECTURAL REQUIREMENTS

### A. Zero-Copy Instant Random Access (No Full Decompression)
- Traditional `tar.gz` requires sequential decompression of the entire archive to read a single file.
- `poler-pack` MUST use a **Chunked Frame Table (Jump Table)** at the archive header/footer with $O(1)$ file lookups.
- Any sub-file inside a 100 GB `.poler` archive MUST be readable via `mmap` with $\le 1\text{ ms}$ latency without touching the rest of the archive.

### B. Content-Defined Chunking & Deduplication (CDC)
- Implement FastCDC / Rabin Fingerprinting / BLAKE3 chunk hashing (default chunk size: 64 KB – 1 MB).
- Identical code files, repeated assets, or multiple versions of repositories MUST be deduplicated globally across the entire archive.

### C. Multi-Tier Compression Pipeline
1. **Tier 0 (Raw / Zero-Copy)**: Direct slice mapping.
2. **Tier 1 (Ultra-Fast ZSTD / LZ4 Frame)**: Multi-threaded streaming block compression.
3. **Tier 2 (POLER Trit5 Semantic Packing)**: Sparse causal matrix quantization for AI memory and structured code.

### D. File-System & Virtual Mount Capabilities (FUSE / In-Memory VFS)
- Support streaming extraction (`--stream`), selective extraction, and read-only virtual mounting so editors (`poler-edit`, Kate plugin) can view/edit files inside archives without unpacking them to disk.

---

## 3. CLI SPECIFICATION & PROTOCOL

### Standalone CLI Commands (`poler-pack` / `poler-engine --pack`):
```bash
# 1. Create an archive with deduplication and ZSTD level 3-19
poler-pack create --input /path/to/data --output project.poler --threads auto --dedup

# 2. Ultra-fast list (O(1) header read)
poler-pack list project.poler --json

# 3. Extract single file or full directory
poler-pack extract project.poler --target src/main.rs --dest ./out/
poler-pack extract project.poler --all --dest ./unpacked/

# 4. In-Memory Random Read Benchmark
poler-pack bench-read project.poler --random-samples 10000

# 5. Integrate directly into POLER memory crystal (.t5c)
poler-engine --pack-to-crystal project.poler --crystal ~/.poler/permanent_memory.t5c
```

---

## 4. CODEBASE STRUCTURE & IMPLEMENTATION TARGETS

Create the following files in `poler-engine`:
- `src/archive/mod.rs` — Core archive interface, formats, and feature flags.
- `src/archive/header.rs` — Magic bytes (`0x50 0x4F 0x4C 0x5A` -> "POLZ"), metadata, chunk index tables, encryption/checksum markers.
- `src/archive/dedup.rs` — Fast Content-Defined Chunking (CDC) & BLAKE3 deduplication registry.
- `src/archive/compressor.rs` — Pluggable compression backends (ZSTD, LZ4, Trit5 Sparse).
- `src/archive/reader.rs` — Zero-copy Random Access mmap reader.
- `src/archive/writer.rs` — Streaming multi-threaded parallel archive builder.
- `src/bin/poler_pack.rs` — Standalone standalone binary entry point.
- `tests/archive_e2e.rs` — Property tests verifying roundtrip lossless extraction, 100% byte-for-byte fidelity, and large-file benchmarks.

---

## 5. PERFORMANCE ACCEPTANCE CRITERIA
1. **Lossless Guarantee**: Roundtrip SHA256 of extracted files must match original sources 100%.
2. **Open Latency**: Reading metadata and first byte of any file in a multi-gigabyte archive must take $< 1\text{ ms}$.
3. **Deduplication Ratio**: Repeating files/folders must compress with near-zero additional storage overhead.
4. **RAM Footprint**: Streaming compression/decompression must not exceed bounded memory (RSS $\le 64\text{ MB}$ regardless of archive size).
