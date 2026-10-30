"""Tests of the quantum module: ansatz and quantum-sampled engine."""

import numpy as np
import pytest
from qiskit_aer import AerSimulator

from poler_quantum.quantum.ansatz import PolerAnsatz
from poler_quantum.quantum.engine import QuantumConfig, QuantumPolerEngine


@pytest.fixture(scope="module")
def simulator():
    return AerSimulator()


class TestAnsatz:
    def test_angles_mapping(self):
        ans = PolerAnsatz(3)
        theta = ans.angles(np.array([1.0, 0.0, -1.0]))
        assert np.allclose(theta, [0.0, np.pi / 2, np.pi])

    def test_angles_clip_out_of_range(self):
        ans = PolerAnsatz(2)
        theta = ans.angles(np.array([5.0, -5.0]))
        assert np.isfinite(theta).all()

    def test_statevector_is_normalised(self):
        ans = PolerAnsatz(5, mode="C")
        sv = ans.statevector(np.full(5, 0.3), gamma=0.4, kappa=0.8)
        assert np.linalg.norm(sv.data) == pytest.approx(1.0)

    def test_probabilities_sum_to_one(self):
        ans = PolerAnsatz(4, mode="B")
        probs = ans.born_probabilities(np.array([0.1, -0.4, 0.7, 0.0]))
        assert probs.sum() == pytest.approx(1.0)
        assert np.all(probs >= 0.0)

    def test_all_plus_state_measures_all_zeros(self):
        # p = +1 -> theta = 0 -> |0...0>; CX chain with all controls |0> is idle
        ans = PolerAnsatz(4)
        samples = ans.sample(AerSimulator(), np.ones(4), shots=32, seed=1)
        assert np.all(samples == 1.0)

    def test_sample_shape_and_values(self):
        ans = PolerAnsatz(4, mode="C")
        samples = ans.sample(AerSimulator(), np.zeros(4), shots=100, seed=2)
        assert samples.shape == (100, 4)
        assert set(np.unique(samples)) <= {-1.0, 1.0}

    def test_sampling_is_deterministic_with_seed(self):
        ans = PolerAnsatz(4)
        p = np.array([0.2, -0.6, 0.4, 0.0])
        a = ans.sample(AerSimulator(), p, shots=50, seed=3)
        b = ans.sample(AerSimulator(), p, shots=50, seed=3)
        assert np.array_equal(a, b)

    def test_mean_sign_of_unentangled_qubit_matches_state(self):
        # single-qubit ansatz has no entanglement: E[sign] = p exactly,
        # because P(measure 0) = (1 + p) / 2.
        ans = PolerAnsatz(1)
        p_val = 0.6
        samples = ans.sample(AerSimulator(), np.array([p_val]),
                             shots=4000, seed=5)
        assert samples.mean() == pytest.approx(p_val, abs=0.03)

    def test_born_entropy_bounds(self):
        ans = PolerAnsatz(3)
        n = 3
        uniform = ans.born_probabilities(np.zeros(n))
        assert PolerAnsatz.born_entropy(uniform) == pytest.approx(np.log2(2 ** n))
        peaked = ans.born_probabilities(np.ones(n))
        assert PolerAnsatz.born_entropy(peaked) == pytest.approx(0.0, abs=1e-9)

    def test_mode_c_phase_is_adaptive(self):
        ans = PolerAnsatz(2, mode="C")
        assert ans.stabilisation_phase(0.5, 0.8) != ans.stabilisation_phase(0.5, 0.3)

    def test_rejects_bad_args(self):
        with pytest.raises(ValueError):
            PolerAnsatz(0)
        with pytest.raises(ValueError):
            PolerAnsatz(2, mode="Z")
        with pytest.raises(ValueError):
            PolerAnsatz(2, entanglement="swap")


class TestQuantumEngine:
    def test_runs_and_stays_bounded(self):
        cfg = QuantumConfig(dim=4, seed=0, q_seed=11)
        engine = QuantumPolerEngine(cfg)
        obs = np.random.default_rng(8).standard_normal((50, 4))
        report = engine.run(obs)
        assert len(report.steps) == 50
        assert np.all(np.abs(report.states) <= 1.0 + 1e-12)

    def test_deterministic_with_fixed_qseed(self):
        obs = np.random.default_rng(4).standard_normal((30, 3))
        cfg = QuantumConfig(dim=3, seed=0, q_seed=7)
        a = QuantumPolerEngine(cfg).run(obs).states
        b = QuantumPolerEngine(QuantumConfig(dim=3, seed=0, q_seed=7)).run(obs).states
        assert np.allclose(a, b)

    def test_converges_on_stationary_target(self):
        rng = np.random.default_rng(1)
        obs = 0.6 + 0.02 * rng.standard_normal((80, 4))
        engine = QuantumPolerEngine(QuantumConfig(dim=4, seed=0, q_seed=3,
                                                  sigma_q=0.02))
        engine.reset()
        report = engine.run(np.tanh(obs))
        first = np.abs(report.states[0] - np.tanh(obs[0])).mean()
        last = np.abs(report.states[-1] - np.tanh(obs[-1])).mean()
        assert last < first
        assert last < 0.1

    def test_constraints_hold_with_quantum_exploration(self):
        Jc = np.array([[1.0, -1.0, 0.0]])
        obs = np.random.default_rng(2).standard_normal((60, 3))
        engine = QuantumPolerEngine(QuantumConfig(dim=3, seed=0, q_seed=5), Jc=Jc)
        report = engine.run(obs)
        assert np.max(np.abs(Jc @ report.states.T)) < 1e-8

    def test_step_result_carries_quantum_diagnostics(self):
        cfg = QuantumConfig(dim=3, seed=0, q_seed=1)
        engine = QuantumPolerEngine(cfg)
        res = engine.step(np.zeros(3))
        assert res.quantum_proposal is not None
        assert res.quantum_proposal.shape == (3,)
        assert 0.0 <= res.born_entropy <= np.log2(2 ** 3) + 1e-9
