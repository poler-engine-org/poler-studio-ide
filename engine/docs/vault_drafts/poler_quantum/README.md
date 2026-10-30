# POLER-Quantum

**POLER[n] — an attention core for a new kind of AI agent.**
Free-energy perception · significance-driven attention · memory resonance ·
logic-constrained creativity · quantum-sampled exploration.

```
Ω(o_t) → F(p,o;θ) → Π_Λ → ε → R[n] → S(p) → p_{t+1}
```

POLER[n] is a cognitive dynamics model that sits between a sensory layer
and a decision layer. It couples **the physics of data** (variational free
energy, as in active inference) with **the logic of decisions** (a
projection onto the constraint subspace), modulated by an **emotional
response** (significance energy ε) and an **episodic memory resonance**
(R[n]) — and it can run its exploration through a **quantum circuit**
(Qiskit/Aer), sampling cognitive proposals from the Born distribution.

This repository is the complete, tested reference implementation:

* `poler_quantum.core` — the classical engine (numpy)
* `poler_quantum.quantum` — the quantum-sampled engine (Qiskit Aer)
* `poler_quantum.benchmark` — reproducible benchmarks with figures
* 128 unit / integration tests, deterministic under fixed seeds

---

## The idea in one minute

An agent lives in a non-stationary world and maintains an internal state
`p` (its "belief/attitude", bounded in `[-1,1]^d`). Every step:

1. **Perceive** — `ω = tanh(o)`: the world enters a bounded inner space.
2. **Feel** — `ε = κ·ΔωᵀGΔω`: how *significant* was the change? A spike
   means "the world just shifted" → attention (learning rate) spikes:
   `η_eff = η·exp(β·tanh(ε̂))`.
3. **Model** — free energy `F = ‖p − ω‖²_G + λ‖p‖²/2` pulls the belief
   toward the perception (with an energy budget against saturation).
4. **Remember** — resonance `Σ ρᵏ(p − s_{t−k})` over the episodic memory:
   in *novelty* mode it repels the state from its own past
   (anti-habituation); in *habit* mode it attracts.
5. **Obey the logic** — every update is projected by
   `Π_Λ = I − J_c⁺J_c` onto the null space of the constraints: the agent
   is *creative inside its ethics*, never outside.
6. **Drift** — `S(p) = Π_Λ(J − D)Π_Λ p`: antisymmetric `J` rotates the
   state (norm-preserving creativity), dissipative `D` contracts it
   (stabilisation).
7. **(Quantum mode)** — the state is encoded into an ansatz
   (`Ry(arccos p_i)` + entanglement chain), measured on Aer, and the Born
   sample becomes the exploration term: **cognition as measurement**.

Full spec: [`docs/POLER_Attention_Core.md`](docs/POLER_Attention_Core.md).

## Vision: dynamic continuous quantization (v2.x line)

POLER-Quantum is not just a simulator — it is a **universal topological
compressor**. Instead of static grids (AWQ / GPTQ / SmoothQuant cut weights
onto fixed INT4/INT8/FP4 scales and break the phase structure of
non-transformer architectures — RWKV, Mamba, SNN, BitNet, optical chips),
it quantizes **continuously and dynamically**:

* **Phase encoding** — `θ = arccos(p)`, `|ψ⟩ = cos(θ/2)|0⟩ + sin(θ/2)|1⟩`;
  Born sampling gives `P(|0⟩) = (1+p)/2` exactly, with no rounding noise.
* **ε-adaptive bit depth** — background tokens compress to 1–2 qubits
  (trits), while an ε-spike unfolds the full `J = A − Aᵀ` phase space.
  Depth is dynamic in time and topology, not fixed over the weight grid.
* **McWeeny purification** — `P_new = 3P² − 2P³` restores idempotency of a
  quantized projector onto the Grassmann manifold in 1–2 iterations,
  without retraining.
* **Substrate independence** — the same invariants `HΨ = 0` and operators
  `(D, J, Π_Λ)` map onto Mach-Zehnder phase shifters, memristive crossbars
  (`D = LLᵀ`) and multiplier-less FPGA shift-and-add (`R_t = ε_t + ρR_{t−1}`).

**Architecture decision (owner, 2026-08-27):** the production simulator is
pure **Rust** — qiskit and friends are tools for *developing the math*, not
runtime dependencies. Training runs **Born + Python when needed**, on data
streamed **fully from the internet** (zero-storage archives, poler-engine).
Full concept: [`docs/dynamic-quantization.md`](docs/dynamic-quantization.md);
plan: [`docs/rust-core-roadmap.md`](docs/rust-core-roadmap.md).

## Shipped in v1.1.0 (RQ2): McWeeny + adaptive ε-depth

The first half of the vision now runs in the Python reference core:

* `poler_quantum.core.purification` — McWeeny purification `3P² − 2P³`:
  entry-grid quantization of a projector breaks `P² = P`; the repair
  restores the invariant **quadratically** (8-bit grid: `6.3e-3 → 9.4e-9`
  in two iterations; eigenvalues return to {0, 1}; rank survives).
  Honest limits are tested too: the subspace drifts by ~the corruption
  size (repair, not identity), and corruption crossing the spectral gap
  can rotate the subspace — see the module docstring.
* `poler_quantum.quantum.compression` — **phase quantization**
  (`θ = arccos p`, grid on `[0, π]`): 2 levels = sign bit, 3 = trits
  `{+1, 0, −1}`, `2^b` = b-bit phase grid; error bounded by `π/(levels−1)`.
* `AdaptiveDepth` — the ε→depth policy: background perception runs on
  trits, an ε-spike unfolds the full phase space within one step.
  Wired into `QuantumPolerEngine` (`adaptive_depth=True`,
  `purify_projector=True`; both off by default — v1.0.0 behaviour
  is bit-exact preserved).

Measured (`poler-quantum compress`, tracking task dim 6, T 120):

| engine | RMSE | mean depth |
|:--|--:|--:|
| full precision | 0.0740 | 64 bit float |
| **ε-adaptive phase compression** | **0.0739** | **2.23 bit** |

Around the regime switch (t = 40, ε̂ = 3.41) the grid jumps from 6 to 256
levels in a single step and falls back to trits right after — dynamic in
time, not static. McWeeny repair table and phase-grid error curves: run
`poler-quantum compress`.

## Install

```bash
pip install -r requirements.txt        # or: pip install -e .
pytest                                  # 128 tests
```

Requires Python ≥ 3.10. Quantum parts need `qiskit` + `qiskit-aer`
(included in requirements).

## Quickstart

```bash
poler-quantum demo                     # classical cycle + constrained tracking
poler-quantum quantum                  # Born-sampled engine + ansatz stats
poler-quantum benchmark --seeds 5      # full benchmark, figures in ./results
poler-quantum spec                     # print the pipeline
```

Or from Python:

```python
import numpy as np
from poler_quantum import PolerEngine, PolerConfig

engine = PolerEngine(PolerConfig(dim=8, eta=0.1, seed=0))
for o in observations:          # (T, 8) array of raw sensory data
    engine.step(o)
print(engine.state)
```

The quantum engine is a drop-in replacement:

```python
from poler_quantum import QuantumPolerEngine, QuantumConfig

engine = QuantumPolerEngine(QuantumConfig(dim=8, sigma_q=0.05, q_seed=42))
```

## Benchmark (honest numbers)

Task: track a latent target (dim 8) that drifts smoothly and jumps at
t = 100 and t = 200; only noisy observations are visible (σ = 0.1).
Aggregate over 5 seeds (`poler-quantum benchmark --T 300 --dim 8 --seeds 5`):

| method | RMSE ↓ | recovery after switch (steps) ↓ | smoothness (step size) ↓ |
|:--|--:|--:|--:|
| GD (η = 0.1) | 0.0608 ± 0.0024 | 9.0 ± 0.9 | **0.0315 ± 0.0003** |
| EMA (α = 0.3) | 0.0520 ± 0.0019 | **3.9 ± 0.7** | 0.0928 ± 0.0013 |
| **POLER** | **0.0519 ± 0.0021** | 5.0 ± 1.1 | 0.0676 ± 0.0010 |
| POLER-Quantum (σ_q = 0.05) | 0.0520 ± 0.0022 | 5.1 ± 1.0 | 0.0678 ± 0.0012 |

Reading (no cherry-picking):

* **POLER has the lowest steady-state error** — statistically tied with the
  aggressively tuned EMA, while running **27 % smoother** (0.0676 vs 0.0928):
  EMA hugs the noisy observation, POLER denoises.
* **POLER recovers from regime switches 44 % faster than GD** (5.0 vs 9.0
  steps) — the ε-attention spike does exactly what it is designed to do.
* **The quantum-sampled variant matches classical accuracy** while keeping
  its exploration alive: the Born samples are correlated by the resonance
  entanglement layer, i.e. the exploration is history-shaped, not white
  noise.
* GD is the smoothest but the slowest and least accurate: no memory, no
  attention.

Figures live in [`docs/benchmark/`](docs/benchmark/) (committed, exactly the
run behind the table above): `tracking.png`, `metrics.png`,
`epsilon.png`, `quantum_entropy.png`, `metrics_multiseed.png`; raw numbers
in `docs/benchmark/metrics_multiseed.json`. A fresh run writes the same
set into `results/`.

> Note on the free-energy column: a post-hoc free energy against the *noisy
> observation* rewards overfitting the noise — EMA "wins" it by jittering.
> The honest measure of tracking quality is the RMSE against the *latent*
> target, which is why the headline table shows RMSE.

## Repository layout

```
poler_quantum/            the package
  core/                   Ω, F, ε, R[n], Π_Λ, S(p), engine
  quantum/                ansatz (modes A/B/C) + quantum engine
  benchmark/              task generator, runners, plots
tests/                    128 tests (pytest)
examples/                 runnable demos
docs/                     the completed spec, the original 2024 draft,
                          dynamic-quantization.md, rust-core-roadmap.md
CHANGELOG.md              release history
legacy/                   original scripts (POLER_modeB/C, Ψ_v3, ...)
archive/                  2025 multi-language restoration fragments
```

## Status & roadmap

* [x] v1.0.0 — complete classical core, quantum-sampled engine, benchmark,
      tests, CI, docs.
* [ ] **RQ1** `poler-quantum-rs`: phase encoder + statevector + Born
      sampling in pure Rust, zero quantum dependencies (parity vs Aer).
* [x] **RQ2** McWeeny purification `3P² − 2P³` + ε-adaptive phase depth —
      **shipped in Python v1.1.0** (`core.purification`,
      `quantum.compression`); Rust port rides with RQ1.
* [ ] **RQ3** ε-adaptive depth by *topology*: per-coordinate / per-module
      grids and generator-level (not entry-level) matrix compression of
      `J = A − Aᵀ`, `D`, `Π_Λ` (state-vector phase depth shipped in v1.1.0).
* [ ] **RQ4** Rust ↔ Python/qiskit cross-validation (< 1e-12 on small n);
      qiskit demoted to a math-development tool.
* [ ] **RQ5** training loop: Born + Python (gym interface over the Rust core).
* [ ] **RQ6** training data fully from the internet: zero-storage streaming
      archives (poler-engine integration).
* [ ] richer worlds: control tasks (act on the world, not only perceive),
  multi-agent resonance coupling.
* [ ] hardware ansatz (mode B/C on real backends via Qiskit Runtime).
* [ ] text / time-series perception modules beyond `tanh` embeddings.

## License

MIT.
