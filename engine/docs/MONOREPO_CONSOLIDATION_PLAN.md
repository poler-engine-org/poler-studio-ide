# POLER Sovereign Monorepo Consolidation Master Plan

> **Target Goal:** Unify `poler-engine`, `POLER-Quantum-RS`, and `poler-os` into a single, sovereign root monorepo.  
> **Eliminating:** Path-dependency fragility (`../POLER-Quantum-RS_repo`), external C++ / ONNX runtimes, duplicate code.  
> **Architecture:** Pure Rust / Zig Workspace with zero external dynamic library dependencies.
>
> **STATUS UPDATE (2026-09-16, M2 executed):** Phase 1, step 1 is DONE —
> `crates/pqc` and `crates/pqw` now live in this repository (merge `f5e53dc`
> + workspace wiring `33c3b48`), full RQ1–RQ23 commit history preserved
> (`git log f5e53dc^2`), CI builds the workspace on a bare checkout with
> **no tokens/secrets**. Note: the workspace is currently a root-package
> hybrid (`poler-engine` stays at repo root, per docs/MERGE_PLAN.md M2);
> the virtual-manifest layout below (with `crates/poler-engine`) is the M3
> target. Steps 2 (`crates/crystallizer` extraction) and Phase 2–3 remain open.

---

## 1. Unified Monorepo Tree Layout

```text
poler/
├── Cargo.toml                  # Root Cargo Workspace (Virtual Manifest)
├── Cargo.lock
├── README.md
├── build.zig                   # Unified Zig build for Bare-Metal / OS Kernel targets
│
├── crates/
│   ├── pqw/                    # POLER Quantum Weights format (encoder/decoder, mmap, BF16/Ternary)
│   │   ├── Cargo.toml
│   │   └── src/
│   │
│   ├── pqc/                    # POLER Quantum Core (AVX2 SIMD, Rotor J, Active Inference, Born stream)
│   │   ├── Cargo.toml
│   │   └── src/
│   │
│   ├── crystallizer/           # R1CS Circuit Builder, CSE, DCE, Meta-Compiler JIT
│   │   ├── Cargo.toml
│   │   └── src/
│   │
│   ├── poler-engine/           # Search, RAG, Inverted Index, K-hop Graph, AIDDE, Gateway, MCP
│   │   ├── Cargo.toml
│   │   └── src/
│   │
│   └── poler-gateway/          # Container Jail, Root Broker, Jailbreak Sentinel, PTY REPL
│       ├── Cargo.toml
│       └── src/
│
├── kernel/                     # poler-os sovereign microkernel & bare-metal runtime
│   ├── build.zig
│   ├── src/
│   │   ├── main.zig            # Ring-0 bootloader & memory manager
│   │   ├── gpu/                # VirtIO-GPU / DRM / KMS scanout
│   │   └── quantum_trap.zig    # Trap handler invoking pqc AVX2 kernels directly on CPU
│   └── include/
│
└── docs/
    ├── POLER_REVERSE_META_COMPILER.md
    ├── COGNITIVE_ARCHITECTURE_299_SOURCES_SYNTHESIS.md
    ├── MONOREPO_CONSOLIDATION_PLAN.md
    ├── PQW_FORMAT.md
    └── sources-archive/
```

---

## 2. Root `Cargo.toml` Workspace Configuration

```toml
[workspace]
resolver = "2"
members = [
    "crates/pqw",
    "crates/pqc",
    "crates/crystallizer",
    "crates/poler-engine",
    "crates/poler-gateway",
]

[workspace.package]
version = "2.0.0"
edition = "2021"
authors = ["POLER Authors"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/poler-engine-org/poler-engine"

[workspace.dependencies]
# Internal workspace crates with seamless zero-overhead linkage
pqw = { path = "crates/pqw", version = "2.0.0" }
pqc = { path = "crates/pqc", version = "2.0.0" }
crystallizer = { path = "crates/crystallizer", version = "2.0.0" }

# Unified external dependencies
memmap2 = "0.9"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

---

## 3. Migration Roadmap (3 Phases)

### Phase 1: Internal Crate Modularization (Zero Breaking Changes)
1. Move `crates/pqw` and `crates/pqc` from `POLER-Quantum-RS` into `crates/`.
2. Extract `src/quantum/meta_compiler.rs` and `crystallizer.rs` into `crates/crystallizer`.
3. Update `poler-engine/Cargo.toml` to depend on workspace paths `{ path = "../pqw" }` etc.

### Phase 2: Native Kernel Linkage (`poler-os`)
1. Create `kernel/` root with Zig 0.13+ toolchain.
2. Direct static linking of `libpqc.a` into `poler-os` kernel image (`target=x86_64-freestanding`).
3. Enable Ring-0 AVX2 state restoration via `xsave` / `xrstor` for quantum JIT forward passes.

### Phase 3: CI/CD & Automated Verification
1. Universal benchmark script running both user-space AVX2 and QEMU bare-metal kernel tests.
2. Verified single-binary build: `cargo build --release --bin poler-engine` yields fully self-contained binary.
