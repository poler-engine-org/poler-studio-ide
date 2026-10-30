# Changelog

## v1.1.0 (2026-08-27) — RQ2: dynamic continuous quantization

The first half of the compressor vision (docs/dynamic-quantization.md)
ships in the Python reference core. Defaults reproduce v1.0.0 exactly;
both new features are opt-in.

### Added

* `poler_quantum.core.purification` — McWeeny purification
  `P_new = 3P² − 2P³`:
  - `mcweeny_purify` / `mcweeny_step` / `idempotency_error` /
    `PurificationResult` (error trace, convergence flag);
  - `quantize_entries` — static entry-grid corruption (the baseline that
    breaks `P² = P`);
  - `symmetric_noise`, `projector_from_constraints`, `subspace_error`;
  - measured: 8-bit grid `6.3e-3 → 9.4e-9` in two iterations (quadratic
    contraction), rank preserved, eigenvalues → {0, 1};
  - honest limits tested: subspace drift ~ corruption size; corruption
    crossing the spectral gap rotates the subspace (repair ≠ identity).
* `poler_quantum.quantum.compression` — POLER phase quantization:
  - `phase_grid` / `phase_quantize` / `phase_quantization_error` /
    `effective_bits` — the angle grid on `[0, π]`: 2 levels = sign bit,
    3 = trits `{+1, 0, −1}`, `2^b` = b-bit phase grid;
    error bound `|p_q − p| ≤ π/(levels−1)`;
  - `AdaptiveDepth` — the ε̂ → levels policy (exponential interpolation
    in bits, monotone, clamped to `[levels_min, levels_max]`).
* `QuantumConfig` gains `adaptive_depth`, `levels_min`, `levels_max`,
  `eps_lo`, `eps_hi`, `purify_projector`, `projector_levels`,
  `purify_iters` (+ `validate_quantum`).
* `QuantumStepResult` gains `levels`, `eff_bits`,
  `pi_idempotency_before`, `pi_idempotency_after`.
* `QuantumPolerEngine`: per-step phase compression of the proposal
  encoding + in-loop McWeeny repair of the compressed projector.
* CLI: `poler-quantum compress` — phase grids, McWeeny repair table,
  adaptive-depth engine run (RMSE full 0.0740 vs compressed 0.0739 at a
  mean depth of 2.23 bits).
* Docs: `docs/dynamic-quantization.md` status table updated; honest
  correction of the convergence order (quadratic, not cubic).
* 56 new tests → **128 total** (was 72).

### Fixed

* CI: `poler-quantum compress` added to the demo matrix.

### Compatibility

* v1.0.0 behaviour is the default and bit-exact preserved (compression
  and purification are opt-in flags).
* `QuantumPolerEngine.quantum_proposal()` gained an optional `p`
  argument (backward compatible).

## v1.0.0 (2026-08-27)

* Complete POLER[n] reference implementation: classical core (Ω, F, ε,
  R[n], Π_Λ, S(p)), quantum-sampled engine (ansatz modes A/B/C, Born
  exploration), tracking benchmark with honest numbers, 72 tests, CI,
  full docs, MIT license.
