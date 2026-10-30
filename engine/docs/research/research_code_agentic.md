# SOTA Code Analysis + Agentic Infrastructure — 2026 Research

**Goal context:** poler-engine currently ships a regex/lexer-based AST-lite extractor (`src/parser/ast_code.rs`, 753 LOC, brace-depth + Python indent heuristic), a SQLite-backed symbol table + call graph AIDDE, a Docker-sandboxed gateway with root broker & jailbreak sentinel, a 6-tool MCP server (stdio+HTTP), a CDP crawler + SimHash + PageRank + BM25 web index, and a ripgrep-class native retrieval layer (Aho-Corasick + `regex`). It has **NO** tree-sitter, LSP integration, agentic loops, CPU LLM inference, WASM plugins, or differential dataflow. The plan is to rewrite everything 100% and surpass SOTA by an order of magnitude. This report is the intelligence brief.

Research sources: web search snapshots (Sept 2026) saved under `/home/z/research_tmp/*.json` (~36 queries) plus direct inspection of `poler-engine/src/`.

---

## 1. tree-sitter — incremental parsing
- **URL:** https://github.com/tree-sitter/tree-sitter · https://tree-sitter.github.io
- **What:** Parser generator + incremental parsing library. Concrete syntax tree (CST, not AST) per source file, reused across edits.
- **Core algorithm:** **GLR (Generalized LR)** with on-the-fly ambiguity resolution via dynamic precedence + a hand-written external scanner per language for context-sensitive tokens (heredocs, Python f-strings, JS regex literals). Incremental reparsing uses **Brics' "reduced subtree" trick**: on edit, walk up to the smallest reusable subtree whose byte-range is entirely before the edit and whose parent can re-accept it, then reparse only the affected window with a state-stack restored from the previous parse — *sub-millisecond* for keystroke edits.
- **Fast/unique:**
  - C core (~10k LOC), no runtime deps, runs in <1 ms per keystroke on 100K LOC files.
  - Lossless CST preserves comments + whitespace → round-trippable; ASTs are projections.
  - **Query DSL**: S-expression pattern language with captures, predicates (`#eq?`, `#match?`), field constraints (`name: (identifier)`) — compiled once via the Pratt-style pattern matcher, then streamed against the CST. This is the API Aider/ast-grep/Neovim/Helix/Zed all build on.
  - **Tags API**: lightweight symbol/def-ref extraction (what Aider's repomap uses) — much cheaper than full HIR.
  - Grammar DSL in JS (`.js`), generates C parser; the same grammar also compiles to WASM via `web-tree-sitter`.
- **Rust availability:**
  - `tree-sitter` crate — official Rust bindings (FFI to C core), under `lib/binding_rust/` of the main repo.
  - Per-language crates: `tree-sitter-rust`, `tree-sitter-python`, …, `tree-sitter-typescript`, etc. (~50 languages).
  - `rust-sitter` (Shadaj Laddad) — define grammar in pure Rust proc-macro; generates tree-sitter JSON → C. Good ergonomics, less mature.
  - `syntastica` — pure-Rust highlighting runtime built on tree-sitter; supports a `-c2rust` mode that transpiles the C grammars to Rust.
  - `tree-sitter-c2rust` — automated transpilation of the C-generated parser sources to safe Rust (no FFI). Used by `syntastica`. **This is the closest thing to "pure Rust tree-sitter" today.**
  - `tree-sitter-grep` crate — ripgrep-style front-end over tree-sitter queries.
- **Steal list:**
  1. **Query DSL** — the S-expression pattern language + capture system + predicate VM. Borrow verbatim, it's the lingua franca.
  2. **Incremental reparsing algorithm** (Brics subtree reuse + state-stack restoration) — paper-grade and well-documented. Reusable in pure Rust without the C core.
  3. **External-scanner contract** — the per-language `scan()` hook for context-sensitive lexing.
  4. **Tags API** — replace AIDDE's regex-based `Definition` extractor with a typed tree-sitter tag query.
  5. **Lossless CST with byte-ranges on every node** — perfect substrate for SimHash dedup at the node level and for AIDDE's byte-accurate call sites.

---

## 2. ast-grep — structural search & replace
- **URL:** https://github.com/ast-grep/ast-grep · https://ast-grep.github.io
- **What:** `grep` for AST patterns. Write a code snippet as the search pattern, get matches by structure not text. Supports YAML rule files, linting, refactoring, multi-language.
- **Core algorithm:** Built on tree-sitter for parsing. The pattern itself is *parsed as code* into a CST, then a **structural unification** algorithm walks target trees and tries to unify against the pattern CST, with metavariables (`$NAME`, `$$MULTI`) and "wildcard" ellipsis (`...`) matching arbitrary subtrees. Under the hood: pattern normalization, equivalence-class collapse (e.g., treat `x = x + 1` ≡ `x += 1`), then a backtracking matcher with on-the-fly pattern compilation.
- **Fast/unique:**
  - **In 2026 ast-grep published a Rust rewrite of tree-sitter** that runs ~30% faster than upstream C (`ast-grep.github.io`, "How ast-grep Rewrote Tree-sitter in Rust and Made It 30% Faster"). They claim: incremental parsing dropped from 5 ms → 400 µs on a 100-LOC edit; the rewrite is pure Rust, no FFI.
  - Pratt-parsing for pattern disambiguation (operator precedence in patterns).
  - Pattern → AST → matcher pipeline; matches carry byte ranges and captures.
  - Rule language: `pattern`, `pattern-not`, `inside`, `has`, `precedes`, `follows` — a small DSL of structural relations.
- **Rust availability:**
  - `ast-grep` itself is **pure Rust** (napi-rs for Node bindings). The `ast-grep-core` crate exposes the matcher engine.
  - Their tree-sitter fork is the gold standard reference if we want to skip upstream tree-sitter entirely.
- **Steal list:**
  1. **Rust tree-sitter rewrite** — port or vendor their fork instead of FFI to C tree-sitter. Removes a whole FFI + unsafe surface; ~30% speed-up claimed.
  2. **Structural unification matcher** with metavariables and `...`/`$$` quantifiers.
  3. **Rule relation DSL** (`inside`/`has`/`precedes`/`follows`) — much more expressive than tree-sitter's query predicates alone. Build this as a layer above tree-sitter queries.
  4. **Equivalence classes** — `+=`, `x = x + 1`, `++x` collapse. Critical for refactor correctness.
  5. **YAML rule format** — composable, distributable lint rules; could become poler-engine's policy format.

---

## 3. LSP (Language Server Protocol)
- **URL:** https://microsoft.github.io/language-server-protocol · spec at https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/
- **What:** JSON-RPC 2.0 protocol between editors (clients) and language tooling (servers). Decouples "IDE features" from "editor". Standardizes text-document sync, completion, hover, goto-definition, references, rename, code actions, diagnostics, semantic tokens, inlay hints, document symbols, workspace symbols, etc.
- **Core architecture:**
  - Transport: stdio / TCP / WebSocket. Framing: `Content-Length: N\r\n\r\n<json>` (HTTP-ish header).
  - Two-way JSON-RPC: requests, responses, notifications, and `$/cancelRequest`.
  - Lifecycle: `initialize` (capabilities handshake) → `initialized` → `textDocument/didOpen` → edits → `textDocument/didChange` → … → `shutdown` → `exit`.
  - **Capability negotiation** is the genius move: client and server each declare what they support. Extensible without breaking.
  - Semantic Tokens (relative-position encoding) — language-agnostic highlighting.
  - **Pull diagnostics** (`textDocument/diagnostic` + `workspace/diagnostic`) added in 3.17 — replaces push model.
- **Fast/unique:**
  - Capability negotiation = forward-compatible forever.
  - Stateless request IDs enable pipelining and cancellation.
  - Document state lives on server; client sends incremental `didChange` range edits.
- **Rust availability:**
  - `tower-lsp` (now `tower-lsp-server`, community fork) — Tower-based async LSP framework on tokio. The de-facto Rust choice.
  - `async-lsp` — alternative using tower `Layer` middleware stack (more flexible, less macro magic).
  - `lsp-types` — pure types crate, complete LSP type definitions, no runtime.
  - `lsp-server` — minimal request/response plumbing (rust-analyzer's own substrate).
- **Steal list:**
  1. **Capability negotiation pattern** — apply to MCP server & gateway too. Self-describing servers, forward-compat.
  2. **Pull-diagnostics model** — replace push-based AIDDE scan with on-demand diagnostics keyed by `(file, version)`.
  3. **Document versioning + range edits** — adopt for the editor bridge, so partial reparses don't re-scan whole files.
  4. **Semantic tokens relative encoding** — cheaper than full highlight spans, perfect for TUI rendering & MCP exposure.
  5. **Content-Length framing** for the gateway's binary protocol.

---

## 4. rust-analyzer — Rust LSP
- **URL:** https://github.com/rust-lang/rust-analyzer · https://rust-analyzer.github.io
- **What:** The reference LSP for Rust. Powers IDE Rust everywhere (VSCode, Neovim, Helix, Zed, IntelliJ-Rust). Lives next to rustc, sharing the trait solver.
- **Core architecture:**
  - **Salsa** incremental computation framework (rust-analyzer's killer feature). Every computation is a "query" keyed by inputs + a memoization DB. Inputs change → only downstream queries re-run. Internally: **ACID-ish memoization with green/red marking** (Adapton-style), MVCC storage, durable vs volatile inputs (filesystem = durable; clock = volatile). "Durable Incrementality" blog post is the deep-dive.
  - **HirDatabase**: layered IRs — `syntax` (rowan CST) → `hir` (name resolution, types) → `chalk` (trait solver). Each layer is a Salsa query group.
  - **Rowan**: red-green tree library (à-la Roslyn). Immutable, persistable, sharable across versions. Green nodes are deduplicated; red nodes are lazy views with parent pointers.
  - **Flycheck**: runs `cargo check` in the background, parses diagnostics.
  - **Chalk** (legacy) → **next-gen trait solver** (now shared with rustc, 2025-2026 migration). Models traits/lifetimes as logical inference rules; solves via **SLG resolution**.
  - **IDE pipeline**: `Text` → `File` → `SourceFile` (CST) → `HirFile` → analysis queries → LSP responses.
- **Fast/unique:**
  - Salsa is the **only production-grade incremental DB pattern** in the Rust ecosystem. Memoized, dependency-tracked, GC-able. This is what makes rust-analyzer usable on 500K-LOC workspaces.
  - Rowan's red-green tree = O(1) structural sharing between parse versions. Perfect for multi-version diff.
  - "Durable" vs "volatile" inputs cleanly separate file changes (which can re-execute queries) from volatile signals (which can't).
  - **Macro expansion** is a Salsa query → macro changes correctly invalidate downstream.
- **Rust availability:**
  - `salsa` (salsa-rs) — pure Rust, MIT. Production-ready; used outside rust-analyzer (bevy, rustc query system, dada, etc.). Version `salsa-2022` (= "salsa 0.17+" macro API) is the current stable.
  - `rowan` — pure Rust, MIT. Standalone; can be used by any CST tool.
- **Steal list:**
  1. **Salsa** — adopt verbatim as the incremental backbone of the new poler-engine AIDDE. Symbol table, call graph, impact analysis, web index, embeddings — all become Salsa queries. A file edit invalidates only downstream.
  2. **Rowan red-green trees** — substrate for tree-sitter CST output. Persisted in AIDDE; structural diff between commits is free.
  3. **Layered IRs** (syntax → name-resolved → typed) — replace the single-pass `ast_code.rs` extractor with multi-stage IRs.
  4. **Durable vs volatile input distinction** — for gateway file watchers vs time-based triggers.
  5. **Macro-expansion-as-query** — generalize to "any expansion (template, macro, codegen) is a Salsa query whose invalidation is tracked".

---

## 5. SearxNG — privacy metasearch
- **URL:** https://github.com/searxng/searxng · https://searxng.org
- **What:** Self-hostable metasearch that proxies 70+ upstream engines (Google, Bing, DuckDuckGo, Wikipedia, arXiv, …). Strips tracking, no profiling, supports Tor.
- **Core architecture:**
  - Python/Flask app. Each upstream is an **engine plugin** (~150 engines) implementing a common `request(query, params)` → `response(resp)` interface.
  - **Async parallel fan-out** via `httpx` + `asyncio`; results merged, deduplicated by URL normalization + title SimHash.
  - **Redis-backed limiter** (bot detection + per-IP rate limiting) using a Bloom-filter + token bucket.
  - Per-engine timeout + retry + circuit breaker; partial results returned.
  - Plugin pipeline: `pre_search` → engine fan-out → `post_search` → `on_result` hooks (custom rewrites, trackers stripping).
- **Fast/unique:**
  - **Engine plugin ABI** is the genius — adding a new source = ~80 LOC Python.
  - Limiter is the only thing keeping it alive against upstreams that ban scrapers.
  - Result merging by URL canonical + fuzzy title dedup.
  - Optional JSON API for machine consumption.
- **Rust availability:**
  - No first-party Rust port. Closest: `searxng-rs` (community client, not a server).
  - The architecture (parallel fan-out + plugin ABI + limiter) maps 1:1 to Rust + `reqwest` + `tokio` + `dashmap` for per-IP limiter. Worth reimplementing.
- **Steal list:**
  1. **Engine plugin ABI** — adapt for poler-engine's `sources/` module: each upstream (GitHub, GitLab, Drive, Gmail, web, MCP) is a trait object with `request`/`response`/`rate_limit`.
  2. **Redis-style token-bucket limiter** with Bloom dedup — fold into the gateway sentinel.
  3. **Result-merge pipeline** with URL-normalization + SimHash dedup — poler-engine already has SimHash for web; extend to all sources.
  4. **Circuit breaker per engine** — fail-fast on misbehaving upstreams.
  5. **Pre/post search plugin hooks** — let WASM plugins mutate queries/results.

---

## 6. MCP (Model Context Protocol)
- **URL:** https://github.com/modelcontextprotocol · https://modelcontextprotocol.io · spec at https://modelcontextprotocol.info
- **What:** Anthropic-open-sourced (Nov 2024) standard for "USB-C for LLMs" — a single protocol connecting LLM hosts/apps to arbitrary data sources & tools. JSON-RPC 2.0 over stdio/HTTP/SSE/WebSocket.
- **Core architecture (4 primitives + 2 transports + 1 negotiation):**
  1. **Tools** — server-exposed functions the LLM can call (model-initiated).
  2. **Prompts** — server-exposed prompt templates (user-initiated).
  3. **Resources** — file-like data the LLM can read (`file://`, `git://`, custom schemes).
  4. **Sampling** — *server* requests the *client* to do an LLM completion (reverse-direction; enables recursive agentic workflows).
  5. **Roots** — client scopes server access to specific URIs (filesystem roots, repos).
  6. **Notifications** — `notifications/progress`, `notifications/cancelled`, resource-subscription.
  - Capability negotiation at `initialize` — same pattern as LSP.
  - **2026 transport additions:** Streamable HTTP (replaces deprecated HTTP+SSE), OAuth 2.1 auth on the server side.
- **Fast/unique:**
  - **Sampling is the asymmetry** that turns MCP from "tool-use" into "true agentic substrate" — a tool can ask the host LLM for help mid-execution.
  - Roots make sandboxing first-class.
  - Resource subscriptions = reactive context for the LLM.
- **Rust availability:**
  - `rmcp` ( official-ish Rust SDK, modelcontextprotocol/rust-sdk) — async, tokio/serde, supports all 4 primitives + 3 transports.
  - `mcp-rust` and several community crates — varying maturity.
  - Poler-engine already ships an MCP server (6 tools). Missing: prompts, resources, sampling, roots, subscriptions.
- **Steal list:**
  1. **Sampling primitive** — adopt server→client LLM calls for agentic tool flows (e.g., a search tool that asks the host to reformulate the query mid-search).
  2. **Resources** as first-class URIs — expose AIDDE symbol table as `poler://symbol/$name`, web index as `poler://page/$hash`. Lets LLM fetch by URI instead of calling a tool.
  3. **Resource subscriptions** + Salsa → reactive context. File changes push `notifications/resources/updated`.
  4. **Roots** — make the jailbreak sentinel's sandbox model explicit in the protocol.
  5. **Streamable HTTP transport** + OAuth 2.1 — replace poler-engine's plain HTTP MCP.
  6. **Prompts** — codify common workflows (impact-analysis, repo-map, refactors) as named prompt templates the agent can browse.

---

## 7. Continue.dev — AI code assistant
- **URL:** https://github.com/continuedev/continue · https://continue.dev · https://docs.continue.dev
- **What:** Open-source AI code assistant (VSCode, JetBrains, Neovim). Was acquired by Cursor (May 2026). Config-driven: any LLM (hosted or local via Ollama/llama.cpp), any embedder, any reranker, any context provider.
- **Core architecture:**
  - **Config DSL** (`config.yaml` / `config.json`): models, embeddings, context providers, slash-commands, MCP servers, tools — all declarative.
  - **Context providers** are the plugin type: `file`, `code`, `openai`, `search`, `database`, `web`, `git`, `diff`, `problems`, `codebase` (RAG), `url`, custom MCP. Each returns `ContextItem[]` with name/dresentation/content.
  - **RAG stack**: chunker (tree-sitter aware) → embedder → LanceDB/`lancedb` vector store → top-k → optional reranker → LLM.
  - **Slash commands** (`/edit`, `/comment`, `/share`, …) written in Python or YAML.
  - **Tool use**: tool-call loop with Continue-defined tool schemas (parallel tool calls supported).
- **Fast/unique:**
  - Pure config DSL means users swap providers without code — strong productization lesson.
  - Tree-sitter-aware chunking (respects function boundaries) beats naive fixed-size chunks.
  - **Hub** for sharing configs/prompts/models.
- **Rust availability:** Continue is TypeScript. No official Rust port. But its patterns translate directly:
  - Context providers → trait objects in Rust.
  - Config DSL → serde + figment.
  - Vector store → `lancedb` has Rust bindings; or `qdrant` client; or roll HNSW (`hnsw_rs`, `hora`, `instant-distance`).
- **Steal list:**
  1. **Context-provider trait** — abstract "anything that yields `ContextItem`s"; implement 20+ providers (code, web, gmail, drive, MCP, AIDDE impact passport, …).
  2. **Tree-sitter-aware chunking** — chunks end at function/class boundaries; replace poler-engine's `retrieval/chunk.rs` if it's fixed-size.
  3. **Slash-command DSL** for the TUI; users compose workflows declaratively.
  4. **Reranker step** in retrieval — currently absent in poler-engine; cross-encoder rerank on top-k BM25.
  5. **Config-as-data** for everything model-related.

---

## 8. Aider — AI pair programming
- **URL:** https://github.com/Aider-AI/aider · https://aider.chat · blog https://aider.chat/docs/repomap.html
- **What:** Terminal AI pair programmer. Edits git-tracked files, commits with sensible messages, manages context automatically.
- **Core architecture — the **repo map** is the genius:**
  1. **Symbol extraction**: tree-sitter parses every file in the repo; extracts `def`/`class`/`fn`/`struct`/`method` definitions and call references using tree-sitter's **tags** queries. Produces a per-file list of symbols.
  2. **Symbol graph**: nodes = symbols, edges = call relations (file A references symbol B defined in file B). Edges weighted by co-occurrence.
  3. **PageRank**: run personalized PageRank with the **currently-open files as personalization vector**. Top-N symbols (≈ 1k tokens) form the repo map.
  4. **Token-budget truncation**: format as a tree of `filename:symbol:line`, drop low-rank symbols first.
  5. **Architect mode**: two-model workflow — one model plans, another edits.
- **Fast/unique:**
  - **PageRank over the symbol graph** is the single best idea in the entire space. It selects ~1k tokens that capture global project structure regardless of size.
  - Implicit feedback: every LLM edit is tracked; subsequent PageRank personalization biases toward edited files.
  - Tree-sitter for 40+ languages via community grammars; consistent tag queries.
- **Rust availability:**
  - Aider is Python. No Rust port.
  - The algorithm is fully reproducible in Rust with tree-sitter + a PageRank impl (poler-engine **already has PageRank** in `src/web/index.rs` for the web graph). Trivial to extend to the symbol graph.
- **Steal list:**
  1. **Repo map algorithm verbatim** — tree-sitter tags → symbol graph → personalized PageRank → token-budgeted tree output. poler-engine has 4/5 pieces already; the missing link is tree-sitter tags + the PageRank wiring from web→symbol graph.
  2. **Architect mode (two-model workflow)** — planner model + executor model; pair with CPU LLM for one and API LLM for the other.
  3. **Auto-commit semantics** — every agent edit becomes a git commit; rollbacks are trivial.
  4. **Token-budgeted truncation algorithm** (greedy by rank, drops subtrees when budget hit).

---

## 9. ripgrep — fast grep
- **URL:** https://github.com/BurntSushi/ripgrep · https://burntsushi.net
- **What:** Line-oriented recursive regex search. The de-facto "fast grep" on the planet.
- **Core architecture:**
  - **`ignore` crate**: parallel directory walk (`crossbeam`/`rayon`), respects `.gitignore`/`.ignore`/global ignores, skips hidden & binary files.
  - **`regex` crate** (BurntSushi) — the engine. Multiple internal strategies, picked by inspecting the pattern:
    - **Aho-Corasick** for literal prefixes (also the basis of GNU grep's kwset).
    - **Teddy** — SIMD-accelerated multi-literal matcher (Hyperscan-derived). SSE2/SSSE3/AVX2/AVX-512 variants; 16/32/64-byte masks, bloom-filter-like; processes 16/32/64 bytes per cycle.
    - **memchr** for single-byte/short-literal scanning (SIMD).
    - **Pike VM** for true regex when no literal prefix; uses `SparseSet` for state tracking.
    - **One-pass DFA construction** for simple patterns.
  - **mmap strategy**: *only* uses mmap for very large files searched serially; switches to `read()` with intermediate buffers for many-small-files (avoids page-table thrash — the insight from BurntSushi's "ripgrep is faster than…" blog post).
  - **Memory-bounded**: streams line-by-line; never loads whole files except for one-line pathological inputs.
- **Fast/unique:**
  - Per-pattern strategy selection (literal / multi-literal / regex) — best-of-all-worlds automatically.
  - SIMD Teddy multi-pattern matcher is the killer for "search many patterns at once" (ripgrep `-e P1 -e P2 …`).
  - The mmap heuristic (only for few files) is non-obvious and crucial.
- **Rust availability:**
  - Pure Rust already. `ripgrep`, `grep` (the crate split), `ignore`, `regex`, `aho-corasick`, `grep-matcher`, `grep-regex`, `grep-searcher` — all reusable.
  - poler-engine already uses `ignore::WalkBuilder` + `aho-corasick` + `regex` in `retrieval/grep.rs`.
- **Steal list:**
  1. **Teddy SIMD multi-literal matcher** — currently poler-engine uses single-pattern Aho-Corasick; upgrade to Teddy for the multi-pattern case (TUI search, multi-keyword web queries).
  2. **Strategy auto-selection** (`Regex::new` does this) — but build a higher-level matcher that picks Teddy vs AC vs Pike VM based on the query shape.
  3. **mmap heuristic** — adopt verbatim in `retrieval/grep.rs`.
  4. **`grep-searcher` streaming line reader** — never materialize whole files.

---

## 10. ugrep — feature-rich grep
- **URL:** https://github.com/Genivia/ugrep · https://www.genivia.com
- **What:** "User-friendly faster grep replacement" with Boolean search (AND/OR/NOT), fuzzy search, hexdump, archive search (tar/zip/cpio/pax), PDF/DOCX search, interactive TUI.
- **Core architecture:**
  - **Boyer-Moore-Horspool fast-skip** for literal search (single pattern).
  - **Adaptive matcher**: chooses BMH / DFA / NFA depending on pattern complexity.
  - **Boolean query parser** (`--bool`) — parses `apple AND (pie OR cake) NOT bread`, builds a tiny AST, evaluates per-line.
  - **Fuzzy search** (`-Z` + edit distance) via bounded Levenshtein automaton (Bitap for ASCII, Myhill-Nerode DFA for Unicode).
  - **Archive traversal** — libarchive-style detection; transparent decompression per-entry; doesn't extract to disk.
  - **Interactive TUI** with preview pane.
- **Fast/unique:**
  - **Archive search without extraction** is the unique feature; works on tar/zip/cpio/pax.
  - Boolean query DSL is rare in grep-clones.
  - Fuzzy matching built-in (ripgrep has `--regex-size-limit` but no fuzzy).
- **Rust availability:**
  - ugrep is C++. No Rust port.
  - Boolean query → trivial Rust AST + per-line eval.
  - Fuzzy → `nucleo` (Helix's fuzzy matcher, pure Rust, SIMD) or `greedy-fz` or `fuzzy-matcher`.
  - Archive search → `tar` + `flate2` + `zstd` crates (already in poler-engine's deps) + the streaming-archive patterns below.
- **Steal list:**
  1. **Boolean query DSL** — add to `retrieval/grep.rs`: `--bool "apple AND (pie OR cake) NOT bread"`. Builds AST, evaluates per match.
  2. **Fuzzy search** via `nucleo` (Helix's matcher, ultra-fast, SkimMatcher scoring).
  3. **Archive-transparent search** — combine with the streaming-archive section below. Search inside `.tar.zst` without extracting.
  4. **Hexdump mode** for binary inspection in the TUI.

---

## 11. Agentic search patterns

### ReAct (Reasoning + Acting)
- **Paper:** Yao et al., 2022, arxiv:2210.03629. Cited 15K+.
- **Loop:** `Thought → Action → Observation → Thought → …`. LLM produces a natural-language reasoning trace then a structured tool call; the tool returns an observation; loop until final answer.
- **Key insight:** pure reasoning (CoT) hallucinates; pure acting (tool-call) lacks planning. Interleaving yields both.
- **Implementation pattern:** a loop driver with a tool registry; the LLM is given a system prompt that demands `Thought: …\nAction: tool_name[args]` format; a parser extracts the action; dispatch; feed back as `Observation: …`.

### Reflexion
- **Paper:** Shinn et al., 2023, arxiv:2303.11366. Cited 7.5K+.
- **Loop:** `Actor → Evaluator → Self-Reflection → Episodic Memory → Actor (retry)`. After a failed attempt, the agent writes a verbal reflection ("I should have checked X first") and stores it in a memory buffer; the next attempt sees all prior reflections.
- **Key insight:** verbal self-critique as a stand-in for gradient-based RL; works with frozen LLMs.
- **Implementation pattern:** episodic memory buffer (list of reflections); evaluator (LLM-as-judge or test suite); short-term trajectory + long-term reflection store.

### Tree of Thoughts (ToT)
- **Paper:** Yao et al., 2023, arxiv:2305.10601. Cited 8.6K+.
- **Loop:** At each step, generate *multiple* candidate thoughts (branching factor b), evaluate each (LLM-as-judge or heuristic), BFS/DFS through the thought-tree to depth d, backtrack on failure.
- **Key insight:** single-chain CoT commits too early; ToT explores alternatives.
- **Implementation pattern:** tree node = (state, parent, thought, score); generator + evaluator LLM calls per node; search algorithm (BFS / DFS / beam search) over the tree.

### Plan-and-Solve / ReWOO
- **Plan-and-Solve:** (Wang et al. 2023) — first generate a full plan, then execute step-by-step. Decouples planning from execution.
- **ReWOO:** (Xu et al. 2023) — plan with *variable placeholders*, execute tools to fill placeholders, then a single solver pass synthesizes the answer. Minimizes LLM calls.

### Tool-use variants (2026 SOTA)
- **Parallel tool calls**: LLM emits multiple tool calls in one turn; runtime dispatches concurrently.
- **Constrained generation** via grammar (GBNF, llama.cpp's `--grammar`, outlines) — guarantees valid tool-call JSON.
- **Structured outputs** (OpenAI/Anthropic 2024+) — schema-constrained decoding at API level.
- **OpenAI Swarm / Anthropic Computer-Use / Claude Code (2025-26)** — lightweight agent handoffs; agent-as-routine.

### Recommendations for poler-engine
Build a **first-class agentic loop substrate** with:
1. **ReAct driver** as the default loop, with structured tool-call schema (use `serde_json::Value` + JSON-schema validation, optional grammar-constrained decoding when on CPU LLM).
2. **Reflexion layer** as an opt-in: a per-session episodic memory (SQLite table) keyed by `(session_id, attempt_n)`.
3. **ToT mode** for planning-heavy tasks: beam search over plan-tree with `b=3, d=4`.
4. **Plan-and-Solve** for multi-step refactors: AIDDE impact analysis produces the plan; executor applies.
5. **Parallel tool calls** — the gateway already sandbox-executes; expose `tools/invoke_parallel` in MCP.
6. **Grammar-constrained decoding** for CPU LLM — see §13.

---

## 12. Streaming archives — random access into compressed blobs

### HTTP Range requests
- Standardized in RFC 7233. Client sends `Range: bytes=start-end`; server replies `206 Partial Content` with `Content-Range` header.
- Used by every browser for video streaming; same pattern enables byte-range fetches into remote tarballs.
- Rust: `reqwest` supports `Range` headers; `tower-http::services::fs` for serving.

### zstd-seekable format
- **Format spec:** https://github.com/facebook/zstd/blob/dev/contrib/seekable_format.md
- Frames compressed independently; a **seek table** (offset → frame) at the end allows O(1) random access by uncompressed offset.
- Trade-off: smaller frames = faster seeks + worse compression ratio. Typical: 4 KB–64 KB frames.
- Rust: `zeekstd` (pure Rust, 2025) — read + write seekable zstd. `zstd-seekable` C bindings also available.
- Use case: store 65K+-file repos as a single `.tar.zst` seekable; random-access any file by `(offset, length)` from the seek table.

### tar streaming
- **ratarmount** (C++): pre-indexes tarball offsets; FUSE-mounts it as a filesystem; random file access without extraction.
- **Tar format**: 512-byte header per file with size + offset to next header; trivially streamable and indexable.
- Rust: `tar` crate streams entries; for random access, build `HashMap<Path, (header_offset, data_offset, size)>` in a single pass.

### Pattern: HTTP Range + zstd-seekable + tar = remote random access
- A remote `.tar.zst-seekable` blob served over HTTP supports:
  1. `GET /blob` with `Range: bytes=0-N` → fetch the zstd seek table (small, at end of file).
  2. Resolve `path/to/file.rs` → tar header offset → data offset → compressed frame(s) covering it.
  3. `GET /blob` with `Range: bytes=X-Y` for the compressed frame(s); decompress; read the tar entry.
- **Poler-engine application:** GitHub mirror as a single `.tar.zst-seekable` per repo; AIDDE symbol table points into byte ranges; remote agents fetch only the files they need. Same idea scales to web crawl archives (WARC + zstd-seekable).

### Other
- **FSST** (Fast Static Symbol Table) — symbol-based compression for short-string-heavy data (CSV, logs). Complements zstd.
- **WARC** — Web ARChive format; standard for crawl storage. `warc` Rust crate.
- **HuggingFace streaming datasets** — same Range + zstd-seekable pattern; `datasets` library streams shards.

### Steal list
1. **zstd-seekable** via `zeekstd` for all large persistent blobs (repo snapshots, web crawl archives, embeddings).
2. **Tar index** (`HashMap<Path, Range>`) in AIDDE — `aidde/files` table becomes a *pointer into a single seekable blob*, not a filesystem path.
3. **HTTP Range serving** — expose the poler-engine blobs over HTTP; remote agents/MCP clients get random access without SSH/git.
4. **ratarmount-style virtual FS** — FUSE-mount a remote archive as a local dir for tools that expect a filesystem.

---

## 13. CPU inference — local LLMs without a GPU

### llama.cpp + GGUF
- **URL:** https://github.com/ggml-org/llama.cpp
- **What:** C/C++ LLM inference engine. The reference for CPU (and CPU+GPU) LLM serving.
- **Core architecture:**
  - **GGUF** format (replaced GGML in 2023): single-file model container with metadata + tensor storage + optional tokenizer. Quantized tensors (Q4_0, Q4_1, Q5_0, Q5_1, Q8_0, Q2_K, Q3_K, Q4_K, Q5_K, Q6_K, IQ2_XXS, …). Each quant type trades bits/weight vs accuracy.
  - **ggml** tensor library: hand-written kernels per CPU arch (ARM NEON, x86 AVX2/AVX-512/AMX, RISC-V V). On Mac: Metal backend via `ggml-metal`.
  - **Weight-only quantization** — weights are quantized; activations stay FP16/BF16. Best throughput on CPU.
  - **k-quants** — per-group (super-block) quantization with mixed precision; better accuracy per bit.
  - **imatrix** (importance matrix) — calibrate quantization to activation distribution; near-FP16 quality at Q4_K.
  - **Batched decoding** + **paged KV cache** (v3) — supports long contexts with low memory.
  - **Grammar-constrained sampling** (`--grammar` with GBNF) — guarantees output schema; perfect for tool calls.
- **Rust availability:**
  - `llama-cpp-2` — maintained Rust bindings to llama.cpp.
  - `llm` (rustformers) — was a pure-Rust reimplementation; archived in 2024 in favor of llama.cpp.
  - `candle` (below) provides an alternative pure-Rust path.

### Candle (Hugging Face)
- **URL:** https://github.com/huggingface/candle
- **What:** Minimalist ML framework in pure Rust. Serverless-first (small binary, no runtime). CPU + CUDA + Metal + WASM backends.
- **Core architecture:**
  - Tensor library with reverse-mode autodiff (for training) and a no-grad inference mode.
  - Backend trait: `Cpu`, `Cuda`, `Metal`, `Wasm`. Same model code, multiple backends.
  - Quantized dtypes (F16, BF16, Q4_0, …) + matmul kernels per arch.
  - Model implementations: Llama, Mistral, Phi, Qwen, StarCoder2, Whisper, Bert, … — directly load HF safetensors / GGUF (partial).
- **Fast/unique:**
  - **Pure Rust, no C dependency** — compiles to WASM; embeddable in browser/plugins.
  - **Serverless-first** — small binary, fast cold-start; opposite of PyTorch.
  - Same code path for CPU/CUDA/Metal — switch at runtime.
- **Use case:** candle is the natural choice if poler-engine wants CPU LLM inference *without* a C FFI dependency. Slightly slower than llama.cpp's hand-tuned kernels but cleaner integration.

### ort (ONNX Runtime, Rust bindings)
- **URL:** https://github.com/pykeio/ort · https://crates.io/crates/ort
- **What:** Rust bindings to Microsoft's ONNX Runtime. Hardware-accelerated ML inference (CPU, CUDA, DirectML, CoreML, TensorRT, OpenVINO, XNNPACK, NNAPI, …).
- **Core architecture:**
  - ONNX = open standard for serialized models; huge ecosystem.
  - **Execution Providers** (EPs): per-hardware acceleration; CPU EP uses MLAS (Microsoft's hand-tuned kernels); XNNPACK for mobile.
  - **Pre-packed weights**, **graph optimizations** (constant folding, layout transformation, redundant-node elimination).
  - Session options: intra-op threads (parallelism within one op), inter-op threads (parallel ops).
- **Use case:** best for **embedding models** (MiniLM, BGE, Nomic-Embed) and **rerankers** (bge-reranker). These run great on CPU and ONNX is the standard export target. Not ideal for autoregressive LLMs (no KV-cache optimizations out of the box).

### Steal list / decision matrix for poler-engine
| Use case | Recommended crate | Why |
|---|---|---|
| Local chat LLM (3-8B) | `llama-cpp-2` (GGUF) | Best CPU kernels; imatrix Q4_K is gold; grammar-constrained sampling |
| Local embeddings (MiniLM/BGE/Nomic) | `ort` (ONNX) | Smaller models, CPU EP is excellent, no llama.cpp dependency |
| Embedding alternative (pure Rust) | `candle` (BERT/MiniLM) | No C FFI; WASM-embeddable; slightly slower |
| Whisper (speech) | `ort` or `candle` | Both have Whisper impls |
| Reranker | `ort` (bge-reranker-v2-micro ONNX) | 100 MB, fast on CPU |
| Grammar-constrained decoding | llama.cpp GBNF | For guaranteed-valid tool calls |
| Tokenizer | `tokenizers` (HF, pure Rust) | Already de-facto standard |

---

## 14. WebAssembly — sandboxed plugins

### Runtimes
- **Wasmtime** (Bytecode Alliance, Rust) — production-grade, WASI Preview 2 + Component Model, fastest cold-start in 2026.
- **Wasmer** (Rust) — older, also production; supports multiple backends (Cranelift, LLVM, Singlepass).
- **WasmEdge** (C++) — focused on cloud/serverless; supports Rust SDK.

### WASI Preview 2 + Component Model
- **Component Model** (2024-2026 stabilization) — typed interface imports/exports; language-agnostic ABI (WIT, Wasm Interface Types); composable without shared-nothing boundary pain.
- **WASI Preview 2** adds `wasi-sockets` (TCP/UDP), `wasi-http` (outbound HTTP), `wasi-cli`, `wasi-filesystem` (with capability-based paths).
- **Capability-based security** — host grants the component a `directory` handle or `socket` capability; the component cannot escape.
- **`wasm32-wasip2` target** in Rust — compile Rust plugins directly to components.

### Pattern: plugin host in Rust
1. Host (poler-engine) embeds `wasmtime::Engine`.
2. Plugins compile to `.wasm` components exposing a `poler-plugin` interface (defined in WIT).
3. Host instantiates per-request; passes a `ResourceTable` (capability handles) for files / network / sandboxed syscall.
4. Plugin calls host functions (imports) for sanctioned operations (read AIDDE symbol, fetch URL).
5. On crash: runtime traps, host catches, request fails cleanly — no host compromise.

### Steal list
1. **Replace Docker-only sandbox** with **WASM for plugins** (user-supplied linters, custom extractors, prompt transforms). Docker stays for *full-process* tools (compilers, interpreters); WASM is for in-process trusted-by-default plugins. ~1000× faster cold-start.
2. **WIT-defined `poler-plugin` interface** — stable ABI for third-party extensions.
3. **Capability-based file/network access** — formalize the jailbreak sentinel's policy as WASI capabilities.
4. **`wasm32-wasip2` compile target** for shipping the TUI's user-defined slash-commands as portable `.wasm`.

---

## 15. Differential dataflow (Frank McSherry)
- **URL:** http://www.frankmcsherry.org · https://github.com/TimelyDataflow/differential-dataflow · paper https://www.cidrdb.org/cidr2013/Papers/CIDR13_Paper111.pdf
- **What:** Incremental data-parallel computation model. Generalizes incremental view maintenance to *arbitrary* dataflow (joins, aggregates, iterations) with **time-versioned collections** and **differences** (collection deltas, not recomputes).
- **Core architecture:**
  - Built on **timely dataflow** (McSherry et al.) — a distributed dataflow runtime with **logical clocks** and **progress tracking** (frontiers). Worker model; scales to N processes.
  - **Differential collections**: `Collection<T, R>` where `R` is the difference type (usually `i64` multiplicity). Insert = +1, delete = -1.
  - **Operators**: `map`, `filter`, `join`, `group_arranged`, `distinct`, `iterate`, `count`, `topk` — all maintain their output *differentially*; new inputs produce only the new output differences.
  - **Iteration**: `iterate` operator handles fixed-point computation with nested timestamps; produces the difference between iterations.
  - **Arrangements** — shared, indexed, persisted operator outputs that downstream operators reuse. Avoids recomputation across queries.
- **Production users:**
  - **Materialize** — SQL streaming database built on differential dataflow. Maintains materialized views over Kafka/Postgres with sub-ms updates.
  - **DDlog** — Datalog-syntax language that compiles to differential dataflow.
- **Fast/unique:**
  - **Sub-millisecond incremental updates** for relational queries — orders of magnitude better than recompute-from-scratch.
  - **Iteration + differential** = correct incremental fixed-point. Nothing else in the ecosystem does this well.
  - **Arrangements** = O(1) reuse of prior computation across queries.
- **Rust availability:**
  - `differential-dataflow` crate (TimelyDataflow org) — pure Rust, MIT. Frank McSherry's own implementation.
  - `timely-dataflow` — the substrate.
  - `ddlog` — language + compiler to Rust.
- **Poler-engine applications:**
  1. **AIDDE call graph + impact analysis** as a differential dataflow: file edit → diff in definitions/calls → differential BFS produces *only* the impact changes. Currently AIDDE does a from-scratch BFS on every query — wasted compute at 65K+ files scale.
  2. **Web index**: page add/delete = difference; BM25 + PageRank incrementally maintained as new crawl data arrives.
  3. **SimHash dedup**: incremental — new pages join against existing sketches and only the collision-deltas propagate.
  4. **Cross-repo symbol graph**: when poler-engine indexes 1000s of repos, a global symbol graph as a differential collection scales to billions of edges.
- **Steal list:**
  1. **Adopt `differential-dataflow` as AIDDE's computation layer** — replaces ad-hoc SQLite tables + rayon parallelism with a maintained dataflow graph. Edit → diff → O(log n) recomputation.
  2. **Arrangements** for cached query results (impact passports, repomaps) — reuse across agent calls.
  3. **DDlog as a DSL** for declaring structural relations (call graph, type hierarchy, import graph) — gets incremental maintenance for free.

---

## 16. Cross-cutting — what's missing in poler-engine

| Capability | poler-engine today | SOTA does this | Gap |
|---|---|---|---|
| AST parsing | `ast_code.rs` lexer + brace-count + Python indent | tree-sitter CST (lossless, query DSL, 50+ langs) | **Critical** — replace |
| Structural search | regex via `retrieval/grep.rs` | ast-grep pattern matching | **Critical** — add |
| Symbol extraction | regex `SIGNATURE_RE` | tree-sitter tags API | **Critical** — replace |
| Repo map / context ranking | (none) | Aider PageRank repomap | **High** — add (we already have PageRank in web!) |
| Incremental recomputation | SQLite + rayon | Salsa (rust-analyzer) | **Critical** — adopt Salsa |
| Differential/incremental views | recompute from scratch | differential dataflow | **High** — adopt for AIDDE + web index |
| LSP integration | (none) | tower-lsp | **Medium** — add as a server |
| MCP primitives | 6 tools, stdio+HTTP | MCP full (tools+prompts+resources+sampling+roots+subscriptions+streamable HTTP) | **High** — extend |
| Agentic loops | CLI tools only | ReAct/Reflexion/ToT | **Critical** — add |
| CPU LLM inference | (none) | llama.cpp/candle/ort | **High** — add |
| Local embeddings | (none in codebase seen) | ort + BGE/MiniLM/Nomic | **Critical** — add for semantic search |
| Reranker | (none) | bge-reranker via ort | **High** — add |
| Plugin sandbox | Docker only | + WASM (Wasmtime/WIT) | **High** — add WASM tier |
| Streaming archives | (none) | zstd-seekable + tar + HTTP Range | **High** — add for remote blob access |
| Multi-literal SIMD search | Aho-Corasick (single) | Teddy (ripgrep) | **Medium** — add for multi-pattern |
| Boolean query DSL | (none) | ugrep `--bool` | **Low** — nice-to-have |
| Fuzzy search | (none) | nucleo (Helix) | **Medium** — add for TUI |
| Archive-transparent grep | (none) | ugrep + ratarmount | **Medium** — add |
| Grammar-constrained decoding | (none) | llama.cpp GBNF | **High** — add for tool-call safety |
| Semantic tokens / structured highlight | (none) | LSP semantic tokens | **Low** — add for TUI/WebLens |

---

## 17. Recommendations — what to steal vs build from scratch

### STEAL (vendor / adopt with minimal adaptation)
1. **`salsa`** crate — adopt verbatim as the incremental backbone. Rewrite AIDDE, web index, and embeddings as Salsa queries. **Highest-leverage single decision.**
2. **`rowan`** crate — red-green trees as the CST substrate; pair with tree-sitter output.
3. **`tree-sitter` + `ast-grep`'s Rust tree-sitter fork** — vendor ast-grep's pure-Rust tree-sitter (30% faster, no FFI). Use the official per-language grammars.
4. **ast-grep `ast-grep-core`** — vendor the matcher; build the policy/rule DSL on top.
5. **`tower-lsp-server`** + `lsp-types` — adopt for the LSP server side (expose AIDDE + retrieval to any editor).
6. **`rmcp`** MCP Rust SDK — extend the existing 6-tool server to the full MCP spec (resources, prompts, sampling, roots, subscriptions, streamable HTTP).
7. **`differential-dataflow`** — adopt for incremental impact analysis and web-index maintenance. Pair with Salsa (Salsa for query-level memoization; DD for cross-collection relational maintenance).
8. **`regex` + `aho-corasick` + `ignore`** — already in use; add Teddy via the regex crate (auto-selected) and `nucleo` for fuzzy.
9. **`zeekstd`** (zstd-seekable) — adopt for archive storage.
10. **`wasmtime`** + Component Model — adopt for the plugin tier; design `poler-plugin` WIT.
11. **`llama-cpp-2`** + **`ort`** — adopt for CPU LLM + embeddings/rerankers.
12. **`tokenizers`** (HF) — adopt; already the standard.

### BUILD FROM SCRATCH (no SOTA fits)
1. **The agentic loop substrate** — combine ReAct driver + Reflexion episodic memory + ToT planner + Plan-and-Solve executor + parallel tool calls. No existing framework does all four cleanly in Rust.
2. **The "poler-plugin" WIT interface** — domain-specific (AIDDE symbols, web pages, MCP resources). Define from scratch.
3. **The Salsa×differential-dataflow bridge** — Salsa for intra-query memoization, DD for cross-query relational maintenance. The composition is novel; need a thin adapter.
4. **The PageRank-over-symbol-graph repomap** — Aider's algorithm but in Rust, personalized by the agent's current focus (not just open files), with Salsa-backed invalidation when the symbol graph changes.
5. **The remote-archive virtual FS** — zstd-seekable + tar + HTTP Range + a Salsa-backed file-cache. No existing tool combines all four.
6. **The "differential impact passport"** — when a file changes, produce *only the delta* of the impact passport. AIDDE today recomputes from scratch; with DD, it's O(log n).
7. **The jailbreak sentinel × WASI capability model** — translate the existing sentinel policy into WASI capability grants; expose to plugins.
8. **The TUI's reactive rendering on top of LSP semantic tokens + Salsa-tracked state** — ratatui + semantic-token stream + Salsa-driven invalidation.
9. **The grammar-constrained tool-call decoder** for CPU LLM — bridge llama.cpp GBNF with the MCP tool schema.

### ARCHITECTURAL ORDERS OF MAGNITUDE — the path to "10× SOTA"
The SOTA tools are **siloed**: tree-sitter doesn't know about Salsa, ast-grep doesn't speak MCP, Aider doesn't use DD, Continue doesn't do CPU inference, etc. poler-engine's opportunity is **the integration**:

1. **Salsa as the universal memoization layer** ties tree-sitter parsing, AIDDE symbol graph, web index, embeddings, and agentic state together. A file edit invalidates exactly the right downstream queries across *all* subsystems. No existing tool does this.
2. **Differential dataflow as the relational maintenance layer** makes impact analysis, PageRank, BM25, and SimHash *incremental* on every edit/crawl. Currently each is recomputed from scratch.
3. **MCP full-spec (resources+sampling)** lets the agent *reactively* subscribe to AIDDE symbol changes — when a file is edited, the LLM's context updates without an explicit tool call.
4. **CPU LLM + grammar-constrained decoding** removes the API dependency for the agentic loop; the loop runs locally, sandboxed, with provably-valid tool calls.
5. **WASM plugins + capability model** turns the gateway from "Docker-only, 100 ms cold-start" into "Docker for processes + WASM for in-process plugins, 100 µs cold-start".
6. **zstd-seekable archives + HTTP Range + Salsa-backed cache** turns 65K+-file repos into a single remote-fetchable blob with O(1) per-file random access. Agents fetch only what they read.
7. **ast-grep matcher on top of Salsa-tracked tree-sitter CSTs** means structural search results are *cached and incrementally maintained*. Run a query once, get updates on edit for free. No tool does this today.

### Concrete rewrite order (high-leverage first)
1. **Salsa DB + rowan CST + tree-sitter (ast-grep fork)** — foundation. Replace `ast_code.rs`.
2. **AIDDE v2 as Salsa queries + differential dataflow for impact BFS** — replace SQLite-from-scratch scans.
3. **Aider-style repomap** (PageRank over symbol graph, personalized) — free win, we already have PageRank.
4. **ast-grep-style structural search** (vendored matcher) — replace regex-only retrieval for code.
5. **MCP full-spec** (resources + sampling + subscriptions + streamable HTTP) — extend existing server.
6. **CPU LLM + embeddings + reranker** (llama-cpp-2 + ort) — local-first agentic substrate.
7. **Agentic loop substrate** (ReAct + Reflexion + ToT + Plan-and-Solve + parallel tools).
8. **WASM plugin tier** (wasmtime + WIT) — second sandbox level.
9. **Streaming archives** (zeekstd + tar index + HTTP Range + Salsa cache).
10. **LSP server** (tower-lsp) — expose everything to editors.
11. **Teddy + nucleo + Boolean DSL** — retrieval polish.
12. **Grammar-constrained tool-call decoder** — agent safety.

---

## 18. Key references (selected)

- Tree-sitter: Brinkerink et al., "Tree-sitter: an incremental parsing system for programming tools" — https://github.com/tree-sitter/tree-sitter ; FOSDEM 2018 talk (algorithms behind tree-sitter, GLR + incremental).
- ast-grep Rust tree-sitter rewrite: https://ast-grep.github.io (Aug 2026) — "How ast-grep Rewrote Tree-sitter in Rust and Made It 30% Faster".
- Salsa: "Durable Incrementality" — https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html ; Adapton (Hammer et al.) the academic ancestor.
- Rowan: https://github.com/rust-analyzer/rowan — red-green tree docs.
- rust-analyzer / Chalk / next-gen trait solver: https://github.com/rust-lang/chalk , https://medium.com/@theopinionatedev/inside-chalk-the-next-gen-type-system-solver-for-rust (Oct 2025).
- Aider repomap: https://aider.chat/2023/10/22/repomap.html , https://aider.chat/docs/repomap.html , https://anishgandhi.com/aider-pagerank-codebase-ranking (Feb 2026).
- ReAct: Yao et al. 2022 — https://arxiv.org/abs/2210.03629.
- Reflexion: Shinn et al. 2023 — https://arxiv.org/abs/2303.11366.
- Tree of Thoughts: Yao et al. 2023 — https://arxiv.org/abs/2305.10601.
- Differential dataflow: McSherry 2015 — http://www.frankmcsherry.org/differential/dataflow/2015/04/07/differential.html ; CIDR 2013 paper; `differential-dataflow` crate.
- MCP spec: https://modelcontextprotocol.io ; Anthropic launch post Nov 2024.
- LSP spec: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/.
- Teddy SIMD matcher: jneem/teddy (Rust) + Hyperscan origin.
- zstd seekable format: https://github.com/facebook/zstd/blob/dev/contrib/seekable_format.md ; `zeekstd` Rust impl.
- ratarmount: https://github.com/mxmlnkn/ratarmount.
- llama.cpp + GGUF: https://github.com/ggml-org/llama.cpp ; "Which Quantization Should I Use?" arxiv (Jan 2026).
- Candle: https://github.com/huggingface/candle.
- ort: https://github.com/pykeio/ort.
- Wasmtime + Component Model: https://bytecodealliance.org , https://github.com/bytecodealliance/wasmtime.

---

*End of report. Total web-search snapshots: 36 queries under `/home/z/research_tmp/`. poler-engine codebase inspected at `/home/z/my-project/skills/poler-engine/src/`.*
