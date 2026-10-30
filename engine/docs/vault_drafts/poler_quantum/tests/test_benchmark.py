"""Tests of the benchmark harness (tasks, runners, metrics)."""

import numpy as np
import pytest

from poler_quantum.benchmark.tasks import tracking_task
from poler_quantum.benchmark.runners import (
    run_all, run_baseline_gd, run_baseline_ema, run_poler, run_quantum,
    summarise, format_table, make_plots, save_summary,
)
from poler_quantum.core.engine import PolerConfig
from poler_quantum.quantum.engine import QuantumConfig
from poler_quantum.metrics import rmse, recovery_steps, path_smoothness


@pytest.fixture(scope="module")
def small_task():
    return tracking_task(T=120, dim=4, switches=(50,), seed=3)


class TestTask:
    def test_reproducible_with_seed(self):
        a = tracking_task(T=100, dim=3, seed=5)
        b = tracking_task(T=100, dim=3, seed=5)
        assert np.array_equal(a.target, b.target)
        assert np.array_equal(a.observations, b.observations)

    def test_shapes(self):
        task = tracking_task(T=100, dim=5, seed=0)
        assert task.target.shape == (100, 5)
        assert task.observations.shape == (100, 5)

    def test_target_bounded(self):
        task = tracking_task(T=150, dim=4, seed=1)
        assert np.all(np.abs(task.target) <= 0.95)

    def test_switch_actually_jumps(self):
        task = tracking_task(T=150, dim=4, switches=(70,), seed=2, jump=0.9)
        pre = np.linalg.norm(task.target[65:70] - task.target[60:65])
        post = np.linalg.norm(task.target[70:75] - task.target[65:70])
        assert post > 5 * (pre + 1e-6)

    def test_rejects_switch_at_boundary(self):
        with pytest.raises(ValueError):
            tracking_task(T=100, switches=(95,))


class TestRunners:
    def test_gd_baseline_shape(self, small_task):
        res = run_baseline_gd(small_task)
        assert res["traj"].shape == (120, 4)

    def test_ema_baseline_shape(self, small_task):
        res = run_baseline_ema(small_task)
        assert res["traj"].shape == (120, 4)

    def test_poler_beats_gd_on_stationary_rmse(self):
        # sanity: with a calm world, full POLER must at least match GD
        task = tracking_task(T=150, dim=4, switches=(), seed=6, noise=0.05)
        gd = run_baseline_gd(task, eta=0.1)
        poler = run_poler(task, PolerConfig(dim=4, seed=0, eta=0.1,
                                             gamma_res=0.05, tau=0.02))
        assert rmse(poler["traj"], task.target) < rmse(gd["traj"], task.target) + 0.05

    def test_run_all_keys(self, small_task):
        results = run_all(small_task,
                          poler_cfg=PolerConfig(dim=4, seed=0),
                          quantum_cfg=QuantumConfig(dim=4, seed=0, q_seed=1))
        assert set(results.keys()) == {"GD", "EMA", "POLER", "POLER-Quantum"}
        for res in results.values():
            assert res["traj"].shape == (120, 4)

    def test_quantum_runner_reports_entropy(self, small_task):
        res = run_quantum(small_task, QuantumConfig(dim=4, seed=0, q_seed=2))
        assert "born_entropy" in res
        assert np.isfinite(res["born_entropy"]).all()


class TestMetrics:
    def test_rmse_zero_for_perfect_tracking(self):
        traj = np.tile(np.array([0.1, 0.2]), (50, 1))
        assert rmse(traj, traj) == pytest.approx(0.0)

    def test_recovery_fast_when_already_locked(self):
        target = np.zeros((60, 2))
        traj = target.copy()
        assert recovery_steps(traj, target, 30) <= 2

    def test_recovery_capped_at_window(self):
        target = np.zeros((60, 2))
        # locked on before the switch, hopelessly off after it
        traj = np.zeros((60, 2))
        traj[30:] = 1.0
        assert recovery_steps(traj, target, 30, window=25) == 25

    def test_smoothness_zero_for_frozen_path(self):
        traj = np.tile(np.array([0.5, -0.5]), (30, 1))
        assert path_smoothness(traj) == pytest.approx(0.0)


class TestSummariesAndPlots:
    def test_summarise_and_table(self, small_task, tmp_path):
        results = run_all(small_task,
                          poler_cfg=PolerConfig(dim=4, seed=0),
                          quantum_cfg=QuantumConfig(dim=4, seed=0, q_seed=1))
        summary = summarise(small_task, results)
        assert set(summary.keys()) == set(results.keys())
        for m in summary.values():
            assert np.isfinite(m["rmse"])
            assert np.isfinite(m["smoothness"])
            assert len(m["recovery_steps"]) == 1
        table = format_table(summary)
        assert "POLER" in table and "RMSE" in table

    def test_plots_and_json_written(self, small_task, tmp_path):
        results = run_all(small_task,
                          poler_cfg=PolerConfig(dim=4, seed=0),
                          quantum_cfg=QuantumConfig(dim=4, seed=0, q_seed=1))
        summary = summarise(small_task, results)
        plots = make_plots(small_task, results, summary, outdir=tmp_path)
        names = {p.name for p in plots}
        assert {"tracking.png", "metrics.png", "epsilon.png",
                "quantum_entropy.png"} <= names
        jpath = save_summary(summary, small_task, outdir=tmp_path)
        assert jpath.exists()
        import json
        payload = json.loads(jpath.read_text())
        assert payload["task"]["T"] == 120
        assert "POLER" in payload["metrics"]
