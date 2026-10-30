# POLER Attention Core — the complete specification

> This document completes `POLER_Attention_Core_v3_original.md` (kept in this
> folder), which breaks off mid-sentence at the forward pass. The original
> Ukrainian text is preserved verbatim; below is the full, finished spec in
> English, matching the reference implementation in `poler_quantum/`.

## 1. System overview

POLER[n] is not a module but an **attention core** that operates between the
sensory layer Ω(o_t) and the decision p_{t+1}. It connects the physics of
data (free energy) with the logic of decisions (projection Π_Λ).

**Pipeline:**

```
Ω(o_t) → F(p,o;θ) → Π_Λ → ε → R[n] → S(p) → p_{t+1}
```

## 2. Core components

| Component | Formula | Function |
|:--|:--|:--|
| Ω(o_t) | `tanh(o)` | Perception (world → bounded internal state) |
| F(p,o;θ) | `‖g(p;θ) − Ω(o)‖²_G + λR_L(p)` | Variational free energy (model divergence) |
| ε | `κ · Δxᵀ G(p) Δx` | Significance energy (emotional response) |
| R[n] | `ρᵏ · s_{t−k}` | Memory resonance (influence of past states) |
| Π_Λ | `I − J_c⁺ J_c` | Logic / ethics projection |
| S(p) | `Π_Λ(J − D)Π_Λ` | Dynamics operator (J: creativity, D: stabilisation) |

### 2.1 Perception Ω

The hyperbolic tangent maps raw (unbounded) observations into the internal
state space `(-1, 1)^d`. All subsequent geometry (metrics, energies,
projections) lives on this compact domain.

### 2.2 Free energy F

With the identity generative model `g(p) = p`:

```
F(p, o)  = (p − Ω(o))ᵀ G (p − Ω(o)) + λ/2 · ‖p‖²
∇_p F    = 2 G (p − Ω(o)) + λ p
```

`G` is a metric tensor (identity by default) — the *precision* of each
perceptual channel in active-inference terms. `λ‖p‖²/2` is the state-cost
regulariser (an energy budget against saturation).

### 2.3 Significance ε — the emotional response

```
ε_t = κ · Δωᵀ G Δω,   Δω = Ω(o_t) − Ω(o_{t−1})
ε̂_t = ε_t / EMA(ε) − 1        (scale-free, running normaliser)
η_eff = η · exp(β · tanh(ε̂_t))  (always positive)
```

A large ε means the world *just changed*. POLER answers with an attention
spike: the effective learning rate is boosted multiplicatively, then decays
back as the running average absorbs the new regime. This is the mechanism
behind POLER's fast recovery after regime switches (see benchmark).

### 2.4 Memory resonance R[n]

```
∇_p E_res = Σ_k ρᵏ (p − s_{t−k}),   k = 1..n
```

* **novelty mode** (canonical): the update `+γ·∇E_res` *repels* the state
  from its own past — anti-habituation, refusal to loop on one thought.
* **habit mode**: the update `−γ·∇E_res` pulls the state toward the
  resonant past — habit formation.

### 2.5 Logic / ethics projection Π_Λ

For linear constraints `c(p) = J_c p = 0`:

```
Π_Λ = I − J_c⁺ J_c        (pseudo-inverse form; robust to rank loss)
```

Every update is projected: the trajectory never leaves the feasible
subspace. Decisions remain *creative inside the constraints*.

### 2.6 Dynamics operator S(p)

```
S(p) = Π_Λ (J − D) Π_Λ p
```

* `Jᵀ = −J` — antisymmetric creativity generator: norm-preserving rotation
  of the state inside the constraint subspace (exploration without
  dissipation).
* `D = Dᵀ ⪰ 0` — dissipative stabilisation: contracts the state toward the
  origin of the subspace.

## 3. Attention flows

**Forward pass (perception → decision):**

```
p_{t+1} = clip( p_t + η_eff · [ Π_Λ(−∇_p F + γ ∇_p E_res) + τ·S(p_t) ] )
```

**Attention flow (ε → η):** significance modulates the rate as in §2.3.

**Quantum flow (measurement):** the state is encoded into the POLER ansatz
(§4) and the exploration term is *Born-sampled* from it.

## 4. Quantum mapping

Each coordinate `p_i ∈ [−1, 1]` parameterises one qubit:

```
Ry(arccos p_i) on qubit i      →  P(measure |1⟩) = (1 − p_i)/2
CX / CZ chain over neighbours  →  resonance coupling R[n]
Rz(φ), φ = γ·e^{−|sin(κπ)|}   →  mode-C adaptive stabilisation
```

The update acquires a term `σ_q · Π_Λ s`, where `s ∈ {−1,+1}^d` is sampled
from the Born distribution of the ansatz. Because of the entanglement
layer, coordinates of `s` are correlated — exploration is history-shaped,
not white noise. In the unentangled limit `E[s_i] = p_i` exactly: the Born
mean *is* the encoded state.

**Modes** (historical): A = perception + resonance; B = A + Rz(γ);
C = A + adaptive Rz (original `POLER_modeC.py`).

## 5. The full cycle (v6)

```
1. ω_t    = tanh(o_t)                          perception
2. ε_t    = κ Δωᵀ G Δω;  η_eff = η e^{β tanh ε̂}  significance → attention
3. ∇F     = 2G(p − ω_t) + λ p                  free energy
4. ∇E_res = Σ ρᵏ (p − s_{t−k})                 resonance
5. dp     = Π_Λ(−∇F + γ∇E_res) + τ S(p)        projection + dynamics
6. p      = clip(p + η_eff · dp);  push(ω_t)   update + remember
```

## 6. Properties (all tested)

* State bounded in `[−1, 1]^d` at every step.
* Feasibility: `J_c p_t = 0` for all t (machine precision).
* Π_Λ idempotent, symmetric, annihilates constraints.
* J norm-preserving; D contracting.
* ε spikes exactly at perception discontinuities.
* Determinism: fixed seeds fully determine a run (classical and quantum).
