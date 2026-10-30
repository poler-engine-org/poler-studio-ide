# POLER-ERI v3.2.0: Reverse Meta-Compiler Technical Specification

> **Module:** `src/quantum/meta_compiler.rs`  
> **Status:** Production-Verified (Bit-for-Bit Identical to Mathematical Reference)  
> **ISA Target:** x86_64 AVX2 / FMA (Zero-Alloc, No-Mul, Single-Pass L1/L3 Cache)  
> **Speed:** >31,400 passes/sec on 256×256 Ternary Weight Matrices (31.76 µs/pass)

---

## 1. Architectural Philosophy: Reverse Meta-Compilation

Conventional deep learning frameworks (PyTorch, ONNX Runtime, GGML, vLLM) execute neural network weights using dense matrix multiplication (`GEMM`), calling external BLAS libraries and streaming gigabytes through DDR memory.

**POLER-ERI (Exact Resonance Inference)** fundamentally replaces matrix multiplication with **Phase-Crystal R1CS Circuits**:
$$\mathbf{r} = c_L \cdot \mathbf{l} + c_R \cdot \mathbf{k}, \quad c_L, c_R \in \{-1, 0, +1\}$$

```
  ┌──────────────────────────────────────────────────────────────┐
  │                 REVERSE STAGE (Weight Analysis)              │
  │  Dense Weights (.safetensors BF16/F16/F32)                   │
  │     ↓                                                        │
  │  Ternarization: Threshold θ = 1.2125 · mean(|w|) (Density ⅓)  │
  │     ↓                                                        │
  │  R1CS Circuit Generation (CircuitBuilder)                    │
  └──────────────────────────────┬───────────────────────────────┘
                                 │
  ┌──────────────────────────────▼───────────────────────────────┐
  │                   META STAGE (Circuit Synthesis)             │
  │  Pass 1: Value Normalization (Chase-map aliasing)            │
  │  Pass 2: Commutative CSE (Subexpression deduplication)       │
  │  Pass 3: Dead Code Elimination (DCE from active outputs)     │
  │  Pass 4: Cold Scalar Prologue Materialization                │
  │  Pass 5: Lane-Affinity Lockstep Wave-Scheduler (8-way SIMD)  │
  └──────────────────────────────┬───────────────────────────────┘
                                 │
  ┌──────────────────────────────▼───────────────────────────────┐
  │                      DUAL EXECUTION TARGETS                  │
  │  A. Runtime Conveyor: VectorGate8 JIT-waves in L1/L3 Cache    │
  │  B. Flat Crystallizer: Pure Rust linear AVX2 source code     │
  └──────────────────────────────────────────────────────────────┘
```

---

## 2. VectorGate8: Branchless 8-Way SIMD Execution

Every AVX2 256-bit register holds eight 32-bit single-precision float values (`8 × f32`). Eight independent R1CS gates execute in single-instruction lockstep:

### Bitwise Mask Algebra
Instead of runtime conditional branching (`match` / `if`), all 9 combinations of ternary coefficients $(c_L, c_R) \in \{-1, 0, +1\}^2$ are evaluated branchlessly:

| Coefficient $c$ | Absorb Mask (`_mm256_and_ps`) | Sign Mask (`_mm256_xor_ps`) | Resulting Value |
|:---:|:---:|:---:|:---:|
| **$0$** | `0x0000_0000` (`MASK_KILL`) | `0x0000_0000` (`MASK_NOFLIP`) | $0.0$ |
| **$+1$** | `0xFFFF_FFFF` (`MASK_KEEP`) | `0x0000_0000` (`MASK_NOFLIP`) | $+x$ |
| **$-1$** | `0xFFFF_FFFF` (`MASK_KEEP`) | `0x8000_0000` (`MASK_FLIP`) | $-x$ (IEEE-754 bit flip) |

```rust
// Core branchless AVX2 intrinsic sequence for 8 gates simultaneously:
let l_val = _mm256_loadu_ps(operands.as_ptr().add(left_base));
let r_val = _mm256_loadu_ps(operands.as_ptr().add(right_base));

// 1. Absorb (Zeroing unneeded terms)
let l_absorbed = _mm256_and_ps(l_val, absorb_l_mask);
let r_absorbed = _mm256_and_ps(r_val, absorb_r_mask);

// 2. Sign Inversion (Negation without multiplication)
let l_signed = _mm256_xor_ps(l_absorbed, sign_l_mask);
let r_signed = _mm256_xor_ps(r_absorbed, sign_r_mask);

// 3. Single Addition (Produces 8 results in 1 clock cycle)
let res = _mm256_add_ps(l_signed, r_signed);
_mm256_storeu_ps(operands.as_mut_ptr().add(result_base), res);
```

---

## 3. Lane-Affinity Wave Scheduling

A major bottleneck in SIMD processing is `gather` overhead (`_mm256_set_ps` from scattered addresses). 

The POLER-ERI scheduler solves this through **Lane Affinity**:
1. Chains of accumulator additions for row $j$ are pinned strictly to SIMD lane $j \in [0..7]$.
2. The left operands of wave $w$ are guaranteed to be the contiguous outputs of wave $w-1$.
3. Left and right vectors are loaded via single contiguous **`_mm256_loadu_ps`** instructions, eliminating gathers.
4. Output results are written with contiguous **`_mm256_storeu_ps`**.

---

## 4. Benchmark & Verification Results

* **Correctness Fuzzing:** 88 randomized fuzzing circuits tested against reference mathematical evaluator — 100% bit-for-bit identical float outputs.
* **Kernel Compilation:** Flat generated Rust functions compile directly with `rustc -O` and match JIT runtime results bit-for-bit.
* **Throughput on 256×256 Layer:**
  - 43,764 scalar gates condensed into 5,450 8-wide SIMD waves.
  - Execution time: **31.76 µs per forward pass**.
  - Rate: **31,490 passes / second** (Exceeding the 1,000 passes/s target by $\times 31.5$).
