"""Integration tests of the classical POLER[n] engine."""

import numpy as np
import pytest

from poler_quantum.core.engine import PolerConfig, PolerEngine


def constant_observations(dim=4, value=0.6, T=80, noise=0.02, seed=1):
    rng = np.random.default_rng(seed)
    return value + noise * rng.standard_normal((T, dim))


class TestEngineBasics:
    def test_state_stays_bounded(self):
        cfg = PolerConfig(dim=4, seed=0)
        engine = PolerEngine(cfg)
        obs = np.random.default_rng(2).standard_normal((60, 4)) * 5
        engine.run(obs)
        assert np.all(np.abs(engine.state) <= 1.0 + 1e-12)

    def test_converges_on_stationary_target(self):
        obs = constant_observations()
        engine = PolerEngine(PolerConfig(dim=4, seed=0, eta=0.15))
        engine.reset()
        report = engine.run(obs)
        first = np.abs(report.states[0] - np.tanh(obs[0])).mean()
        last = np.abs(report.states[-1] - np.tanh(obs[-1])).mean()
        assert last < first
        assert last < 0.05

    def test_free_energy_decreases_on_stationary_world(self):
        obs = constant_observations()
        engine = PolerEngine(PolerConfig(dim=4, seed=0))
        report = engine.run(obs)
        head = report.free_energy[:10].mean()
        tail = report.free_energy[-10:].mean()
        assert tail < head

    def test_deterministic_with_seed(self):
        obs = np.random.default_rng(9).standard_normal((40, 3))
        a = PolerEngine(PolerConfig(dim=3, seed=7)).run(obs).states
        b = PolerEngine(PolerConfig(dim=3, seed=7)).run(obs).states
        assert np.allclose(a, b)

    def test_reset_restarts_cycle(self):
        obs = constant_observations(dim=3)
        engine = PolerEngine(PolerConfig(dim=3, seed=0))
        engine.run(obs)
        engine.reset()
        assert engine.t == 0
        assert np.allclose(engine.state, 0.0)
        assert len(engine.resonance) == 0

    def test_bad_config_rejected(self):
        with pytest.raises(ValueError):
            PolerConfig(dim=0).validate()
        with pytest.raises(ValueError):
            PolerConfig(eta=2.0).validate()
        with pytest.raises(ValueError):
            PolerConfig(resonance_mode="weird").validate()


class TestEpsilonAttention:
    def test_attention_spikes_at_regime_switch(self):
        rng = np.random.default_rng(4)
        T = 120
        obs = 0.2 * rng.standard_normal((T, 4))
        switch = 60
        obs[switch:] += 0.8  # sudden world change
        engine = PolerEngine(PolerConfig(dim=4, seed=0, beta=2.0))
        report = engine.run(obs)
        eta = report.eta_eff
        # the largest attention spike must sit at (or right after) the switch
        assert int(np.argmax(eta)) in (switch, switch + 1)
        assert eta[switch] > 1.5 * np.median(eta)

    def test_calm_world_keeps_attention_near_baseline(self):
        obs = constant_observations(noise=0.005)
        engine = PolerEngine(PolerConfig(dim=4, seed=0))
        report = engine.run(obs)
        tail = report.eta_eff[30:]
        # chi-square fluctuations of eps give O(1) excursions of eps_hat,
        # but attention must stay within a factor ~3 of the baseline
        assert np.all(tail < 3.0 * engine.cfg.eta)
        assert np.median(tail) < 1.3 * engine.cfg.eta


class TestConstraints:
    def test_trajectory_never_leaves_feasible_subspace(self):
        Jc = np.array([[1.0, -1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 1.0]])
        obs = np.random.default_rng(3).standard_normal((100, 4))
        engine = PolerEngine(PolerConfig(dim=4, seed=0), Jc=Jc)
        report = engine.run(obs)
        traj = report.states
        assert np.max(np.abs(Jc @ traj.T)) < 1e-8

    def test_engine_reset_projects_initial_state(self):
        Jc = np.array([[1.0, -1.0]])
        engine = PolerEngine(PolerConfig(dim=2, seed=0), Jc=Jc)
        engine.reset(np.array([1.0, 0.0]))  # infeasible start
        assert engine.state[0] == pytest.approx(engine.state[1])


class TestResonanceModes:
    def test_novelty_and_habit_diverge(self):
        obs = np.random.default_rng(6).standard_normal((50, 3)) * 0.4
        a = PolerEngine(PolerConfig(dim=3, seed=0, resonance_mode="novelty")).run(obs).states
        b = PolerEngine(PolerConfig(dim=3, seed=0, resonance_mode="habit")).run(obs).states
        assert not np.allclose(a, b)

    def test_snapshot_reports_feasibility(self):
        engine = PolerEngine(PolerConfig(dim=3, seed=0))
        engine.run(constant_observations(dim=3))
        snap = engine.snapshot()
        assert snap["feasible"] is True
        assert snap["t"] == 80
