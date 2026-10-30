# POLER Edit — text editing core without file-size limits

**One binary. Any file size. Constant RAM.**

POLER Edit is the sovereign editing core of
[poler-engine](https://github.com/poler-engine-org/poler-engine) v0.38.0.
It answers a question every heavy-duty text user eventually asks:

> *Why does my editor die on a multi-gigabyte log?*

Kate/KTextEditor, VS Code, gedit — all of them materialize per-line heap
objects for the whole document (`QList<KateLineLayout>` and friends).
A 5 GB log becomes 20+ GB of RAM, then an OOM kill. POLER Edit removes
that ceiling structurally: the file is never loaded into the heap at all.

## Measured numbers (v0.38.0, 2 vCPU sandbox container)

| Operation | 304 MiB text, 2.5 M lines | 100 GiB sparse file |
|---|---|---|
| Open | **0.03 ms** | **0.12 ms** |
| Full line index (parallel SIMD) | 37 ms — **8.1 GiB/s** | 2.1 GiB/s (page-fault bound) |
| Search, full scan (rare word) | 58 ms — **5.2 GiB/s** | — |
| Search, 10 000 hits early-exit | 12 ms — **25.8 GiB/s** | 23 ms |
| Peak RSS of the whole process | **33 MiB** | **40 MiB** |

Reproduce on any file:

```bash
poler-engine --edit-bench /path/to/any/file --edit-bench-query needle
```

Reference point on the same machine: `wc -l` on the 304 MiB file takes
66 ms. POLER Edit indexes the same file (counting every line, building
the full line map, in parallel) in **37 ms** — faster than `wc`, while
also staying responsive for editing.

## Why the limits disappear

1. **Zero-copy piece table.** The original file is memory-mapped and
   never copied. It is sliced into 16 MiB *metadata* pieces at open time
   — creating 6 400 descriptors for a 100 GiB file costs ~0.1 ms and
   touches zero bytes of content. Edits append to immutable buffers;
   pieces reference either the mmap or the edit log (classic piece
   table, production-proven since the early 90s).
2. **Lazy parallel SIMD line index.** Line numbers require counting
   newlines — unavoidable. We count them per piece with `memchr`
   (SIMD), in parallel (rayon), cache the count inside the piece
   descriptor, and stop there. The viewport renders instantly from the
   top; a jump to line 1 000 000 extends the index only along the way.
3. **Kernel page hygiene.** After scanning a piece we `madvise(MADV_DONTNEED)`
   it (the same pattern as our disk harvester): RSS stays at tens of
   MiB regardless of file size, and the page cache still serves re-reads.
4. **Search that never blocks.** Aho-Corasick (the engine's multi-pattern
   SIMD matcher) scans pieces directly over the mmap. Matches that span
   piece boundaries — including boundaries between original bytes and
   recent edits — are found via a seam carry. Long searches are
   cancellable mid-scan (Esc) and report progress.

## Protocol: LSP-style, one process, many documents

The core exposes a JSON-lines protocol over stdio — the same shape the
editor world already knows from LSP:

```json
→ {"id":1,"cmd":"open","path":"huge.log"}
← {"id":1,"ok":true,"doc":1,"bytes":107374182400,"pieces":6400,"lines":null}
→ {"id":2,"cmd":"viewport","doc":1,"line":0,"count":100}
← {"id":2,"ok":true,"lines":[{"line":0,"text":"...","truncated":false}]}
→ {"id":3,"cmd":"insert_at","doc":1,"line":5,"col":3,"text":"hello"}
→ {"id":4,"cmd":"search","doc":1,"query":"ERROR","case_sensitive":true,"limit":1000}
← {"id":4,"ok":true,"hits":[{"byte":9103421,"line":80213,"col":4,"len":5}],...}
→ {"id":5,"cmd":"index","doc":1}
← {"ev":"progress","op":"index","doc":1,"done_bytes":4831838208,"total_bytes":107374182400,"gbps":2.3}
← {"id":5,"ok":true,"lines":400002,"completed":true}
```

Commands: `open close stats viewport goto linecol insert_at delete search
index save save_as undo redo cancel quit`. A single `poler-engine
--edit-serve` process serves any number of documents (GUI tabs).

## The Qt client: Kate's look, none of Kate's weight

`integrations/poler-edit-qt` — a **pure Qt6 Widgets** application
(zero KF6/KParts dependencies): menus, toolbar, tabs, status bar with
line/col and live indexing speed, dark Breeze-style theme, vi-style
command line (`:w`, `:q`, `:wq`, `:123`), Ctrl+F search over files of
any size. The custom viewport paints only the visible lines fetched
from the core; generations discard stale async blocks, so scrolling a
100 GiB file feels like scrolling a 1 KB one.

The older sibling integration — the KTextEditor plugin
(`integrations/kate-poler`) — embeds the engine's search, crystal and
motor into stock Kate. POLER Edit is the complementary bet: a full
editor experience where the *core itself* has no size limits.

## Engineering guarantees

- **13 unit tests**, including a 160-step randomized property test that
  diffs the piece table against a naive `String` reference model after
  every operation (content, byte length, line numbers, line starts).
- **2 end-to-end tests** drive the real `poler-engine --edit-serve`
  binary through the protocol: open, viewport, edit, search, save,
  undo, multi-document, error paths.
- Atomic saving: temp file + fsync + rename; permissions preserved.
- UTF-8 safety: edit boundaries snap to character boundaries
  (Cyrillic text cannot be split); searches are case-insensitive for
  Cyrillic via three-case pattern expansion.
- Undo/redo are snapshots of piece *metadata* (tens of KB), not text
  copies — undo history does not scale with file size.

## Honest limits (v1)

- No syntax highlighting yet (planned: KSyntaxHighlighting XML or
  tree-sitter, compiled in).
- Cold-index speed on huge files is page-fault bound (~2 GiB/s per
  couple of vCPUs in our container); warm re-scans run at memory
  bandwidth. More cores scale it linearly.
- A file replaced on disk underneath an open mmap is not yet detected
  (planned: stat polling).
- The GUI client is young: v1 covers viewing, editing, search, undo,
  save, tabs, command line — not yet split view, sessions or projects.

## Try it

```bash
git clone https://github.com/poler-engine-org/poler-engine
cd poler-engine
cargo build --release
# цифры:
./target/release/poler-engine --edit-bench some_huge_file --edit-bench-query needle
# GUI (Arch):
cd integrations/poler-edit-qt && cmake -B build && cmake --build build && ./build/poler-edit
```

POLER Edit is the editor-shaped face of the poler-engine stack: the same
SIMD scanning machinery that powers its disk harvester (5 747 files in
3.3 s) and its triune retrieval core, now wrapped in a piece table.
