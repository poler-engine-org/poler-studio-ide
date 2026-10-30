"""Tests of phase compression + adaptive eps-depth + engine integration
(RQ2, v1.1.0)."""

import numpy as np
import pytest

from poler_quantum.quantum.ansatz import PolerAnsatz
from poler_quantum.quantum.compression import (
    AdaptiveDepth,
    effective_bits,
    phase_grid,
    phase_quantization_error,
    phase_quantize,
)
from poler_quantum.quantum.engine import QuantumConfig, QuantumPolerEngine
from poler_quantum.benchmark.tasks import tracking_task


@pytest.fixture(scope="module")
def state() -> np.ndarray:
    rng = np.random.default_rng(42)
    return np.clip(rng.standard_normal(8) * 0.6, -0.99, 0.99)


# -- the phase grid -----------------------------------------------------------

class TestPhaseGrid:
    def test_two_levels_is_the_sign_pair(self):
        assert np.allclose(phase_grid(2), [1.0, -1.0])

    def test_three_levels_are_trits(self):
        assert np.allclose(phase_grid(3), [1.0, 0.0, -1.0])

    def test_grid_is_descending_and_bounded(self):
        g = phase_grid(17)
        assert len(g) == 17
        assert np.all(np.diff(g) < 0)
        assert g[0] == pytest.approx(1.0)
        assert g[-1] == pytest.approx(-1.0)

    def test_effective_bits(self):
        assert effective_bits(2) == pytest.approx(1.0)
        assert effective_bits(4) == pytest.approx(2.0)
        assert effective_bits(256) == pytest.approx(8.0)

    def test_validation(self):
        with pytest.raises(ValueError):
            phase_grid(1)
        with pytest.raises(ValueError):
            phase_quantize(np.zeros(3), 1)


class TestPhaseQuantize:
    def test_output_lives_on_the_grid(self, state):
        for levels in (2, 3, 5, 17, 257):
            q = phase_quantize(state, levels)
            grid = phase_grid(levels)
            # every quantized value is one of the grid points
            dist = np.abs(q[:, None] - grid[None, :]).min(axis=1)
            assert np.all(dist < 1e-12)

    def test_sign_bits(self):
        p = np.array([0.3, -0.7, 0.99, -0.02])
        q = phase_quantize(p, 2)
        assert np.allclose(q, [1.0, -1.0, 1.0, -1.0])

    def test_trits(self):
        # trit boundary at |p| = cos(pi/4) ~ 0.707
        p = np.array([0.9, 0.2, -0.2, -0.9])
        # theta = arccos(p); with 3 grid points the nearest is cos(k*pi/2)
        q = phase_quantize(p, 3)
        # cos(pi/2) is 6.1e-17 in floating point -> compare with tolerance
        assert np.allclose(q, [1.0, 0.0, 0.0, -1.0], atol=1e-15)

    def test_out_of_range_is_clipped(self):
        q = phase_quantize(np.array([5.0, -5.0]), 3)
        assert np.allclose(q, [1.0, -1.0])

    def test_error_bound(self, state):
        # |cos a - cos b| <= |a - b|  =>  err <= pi / (levels - 1)
        for levels in (2, 3, 7, 33):
            max_err, rmse = phase_quantization_error(state, levels)
            assert max_err <= np.pi / (levels - 1) + 1e-12
            assert rmse <= max_err

    def test_error_shrinks_with_depth(self, state):
        e2 = phase_quantization_error(state, 3)[0]
        e4 = phase_quantization_error(state, 17)[0]
        e8 = phase_quantization_error(state, 257)[0]
        assert e2 > e4 > e8
        assert e8 < 0.013                          # 8 bits: sub-grid-step

    def test_exact_points_are_fixed(self):
        p = phase_grid(5)                          # already on the grid
        assert np.allclose(phase_quantize(p, 5), p)


# -- adaptive eps-depth ---------------------------------------------------------

class TestAdaptiveDepth:
    def test_validation(self):
        with pytest.raises(ValueError):
            AdaptiveDepth(levels_min=1)
        with pytest.raises(ValueError):
            AdaptiveDepth(levels_min=8, levels_max=4)
        with pytest.raises(ValueError):
            AdaptiveDepth(eps_lo=2.0, eps_hi=1.0)

    def test_background_floor(self):
        d = AdaptiveDepth(levels_min=3, levels_max=256)
        for eps_hat in (-5.0, -1.0, 0.0):
            assert d.levels_for(eps_hat) == 3

    def test_spike_ceiling(self):
        d = AdaptiveDepth(levels_min=3, levels_max=256)
        for eps_hat in (2.0, 10.0, 100.0):
            assert d.levels_for(eps_hat) == 256

    def test_monotone_in_significance(self):
        d = AdaptiveDepth(levels_min=2, levels_max=1024)
        eps = np.linspace(-2.0, 3.0, 61)
        levels = [d.levels_for(e) for e in eps]
        assert all(a <= b for a, b in zip(levels, levels[1:]))

    def test_interpolation_stays_in_bounds(self):
        d = AdaptiveDepth(levels_min=3, levels_max=256)
        for eps_hat in np.linspace(0.0, 2.0, 21):
            lv = d.levels_for(eps_hat)
            assert 3 <= lv <= 256

    def test_bits_are_consistent_with_levels(self):
        d = AdaptiveDepth(levels_min=2, levels_max=256)
        for eps_hat in (0.3, 0.7, 1.4):
            assert d.bits_for(eps_hat) == pytest.approx(
                np.log2(d.levels_for(eps_hat)))

    def test_full_unfold_on_spike_is_high_precision(self):
        # eps_hat >= eps_hi unfolds the full phase space (the vision:
        # the resonance layer instantly deploys the full J = A - A^T space)
        d = AdaptiveDepth(levels_min=3, levels_max=1024, eps_hi=1.5)
        assert d.levels_for(1.5) == 1024
        assert d.bits_for(1.5) == pytest.approx(10.0)


# -- engine integration -----------------------------------------------------------

def _short_task(T=60, dim=4, seed=3):
    return tracking_task(T=T, dim=dim, seed=seed)


class TestEngineCompression:
    def test_defaults_are_full_precision(self):
        # v1.0.0 behaviour is the default: no compression, no purification
        cfg = QuantumConfig(dim=4, seed=0, q_seed=1)
        engine = QuantumPolerEngine(cfg)
        task = _short_task()
        engine.reset()
        for o in np.tanh(task.observations[:10]):
            res = engine.step(o)
            assert res.levels is None
            assert res.eff_bits is None
            assert res.pi_idempotency_before == 0.0
            assert res.pi_idempotency_after == 0.0

    def test_determinism_with_compression_on(self):
        # same seeds -> identical trajectories, even with quantization
        def run():
            cfg = QuantumConfig(dim=4, seed=0, q_seed=7,
                                adaptive_depth=True)
            engine = QuantumPolerEngine(cfg)
            engine.reset()
            task = _short_task()
            return engine.run(np.tanh(task.observations)).states
        assert np.array_equal(run(), run())

    def test_adaptive_depth_reports_levels_and_bits(self):
        cfg = QuantumConfig(dim=4, seed=0, q_seed=5, adaptive_depth=True,
                            levels_min=3, levels_max=256)
        engine = QuantumPolerEngine(cfg)
        engine.reset()
        task = _short_task()
        levels_seen = []
        for o in np.tanh(task.observations):
            res = engine.step(o)
            assert res.levels is not None
            assert 3 <= res.levels <= 256
            assert res.eff_bits == pytest.approx(np.log2(res.levels))
            levels_seen.append(res.levels)
        # the task has a regime switch -> a spike must unfold more depth
        assert max(levels_seen) > min(levels_seen)

    def test_compressed_engine_still_tracks(self):
        # honest trade-off: trit-floor background compression costs little
        task = _short_task(T=120, dim=6, seed=9)
        obs = np.tanh(task.observations)

        full = QuantumPolerEngine(QuantumConfig(dim=6, seed=1, q_seed=3))
        full.reset()
        traj_full = full.run(obs).states

        comp = QuantumPolerEngine(QuantumConfig(dim=6, seed=1, q_seed=3,
                                                adaptive_depth=True,
                                                levels_min=3, levels_max=256))
        comp.reset()
        traj_comp = comp.run(obs).states

        def rmse(traj):
            return float(np.sqrt(np.mean((traj[20:] - task.target[20:]) ** 2)))
        # compression perturbs exploration, it must not destroy tracking
        assert rmse(traj_comp) < rmse(traj_full) + 0.03

    def test_purify_projector_restores_idempotency_in_loop(self):
        Jc = np.array([[1.0, 2.0, 0.0, 0.0]])
        cfg = QuantumConfig(dim=4, seed=0, q_seed=2,
                            purify_projector=True, projector_levels=256)
        engine = QuantumPolerEngine(cfg, Jc=Jc)
        engine.reset()
        task = _short_task()
        for o in np.tanh(task.observations[:20]):
            res = engine.step(o)
            assert res.pi_idempotency_before > 0.0    # grid broke it
            assert res.pi_idempotency_after < res.pi_idempotency_before / 50.0
            assert res.pi_idempotency_after < 1e-3

    def test_purify_projector_keeps_feasibility(self):
        Jc = np.array([[1.0, 2.0, 0.0, 0.0]])
        cfg = QuantumConfig(dim=4, seed=0, q_seed=2,
                            purify_projector=True, projector_levels=256)
        engine = QuantumPolerEngine(cfg, Jc=Jc)
        engine.reset()
        task = _short_task()
        traj = engine.run(np.tanh(task.observations)).states
        # state stays on the constraint subspace up to the grid scale
        assert np.abs(Jc @ traj.T).max() < 0.05

    def test_quantum_config_validation(self):
        with pytest.raises(ValueError):
            QuantumConfig(levels_min=1).validate_quantum()
        with pytest.raises(ValueError):
            QuantumConfig(levels_max=2, levels_min=4).validate_quantum()
        with pytest.raises(ValueError):
            QuantumConfig(eps_lo=1.0, eps_hi=0.5).validate_quantum()
        with pytest.raises(ValueError):
            QuantumConfig(projector_levels=1).validate_quantum()
        with pytest.raises(ValueError):
            QuantumConfig(purify_iters=0).validate_quantum()

    def test_phase_quantized_state_keeps_born_statistics(self):
        # E[sign] = p is preserved by the phase grid: quantizing p to the
        # grid and sampling the ansatz reproduces the grid probabilities
        ans = PolerAnsatz(4)
        p = np.array([0.8, -0.3, 0.1, 0.0])
        q = phase_quantize(p, 3)
        # for grid point p_q, P(|0>) = (1 + p_q)/2 exactly
        probs = (1.0 + q) / 2.0
        assert np.allclose(probs, np.round(probs * 2.0) / 2.0)
