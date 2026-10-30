"""Unit tests of the classical core components."""

import numpy as np
import pytest

from poler_quantum.core.perception import Perception
from poler_quantum.core.free_energy import FreeEnergy
from poler_quantum.core.epsilon import Epsilon
from poler_quantum.core.resonance import Resonance
from poler_quantum.core.projection import LogicProjector
from poler_quantum.core.dynamics import DynamicsOperator


# -- perception -----------------------------------------------------------------

class TestPerception:
    def test_tanh_bounded(self):
        perc = Perception()
        o = np.array([5.0, -5.0, 0.0])
        w = perc.omega(o)
        assert np.all(w > -1.0) and np.all(w < 1.0)
        assert w[2] == 0.0

    def test_linear_clips(self):
        perc = Perception("linear")
        w = perc.omega(np.array([5.0, -5.0]))
        assert np.allclose(w, [1.0, -1.0])

    def test_rejects_unknown_kind(self):
        with pytest.raises(ValueError):
            Perception("sigmoid")


# -- free energy ------------------------------------------------------------------

class TestFreeEnergy:
    def test_zero_at_matching_state(self):
        fe = FreeEnergy(3, lam=0.0)
        p = np.array([0.2, -0.5, 0.7])
        assert fe.value(p, p) == pytest.approx(0.0)

    def test_gradient_matches_finite_differences(self):
        rng = np.random.default_rng(3)
        dim = 4
        A = rng.standard_normal((dim, dim))
        G = A @ A.T + dim * np.eye(dim)  # SPD metric
        fe = FreeEnergy(dim, G=G, lam=0.13)
        p = rng.uniform(-0.5, 0.5, dim)
        omega = rng.uniform(-0.9, 0.9, dim)
        analytic = fe.grad(p, omega)
        num = np.zeros(dim)
        h = 1e-6
        for i in range(dim):
            e = np.zeros(dim); e[i] = h
            num[i] = (fe.value(p + e, omega) - fe.value(p - e, omega)) / (2 * h)
        assert np.allclose(analytic, num, rtol=1e-4, atol=1e-6)

    def test_state_cost_pulls_to_origin(self):
        fe = FreeEnergy(2, lam=1.0)
        g = fe.grad(np.array([1.0, 1.0]), np.array([1.0, 1.0]))
        assert np.allclose(g, [1.0, 1.0])  # only the regulariser remains

    def test_rejects_bad_metric_shape(self):
        with pytest.raises(ValueError):
            FreeEnergy(3, G=np.eye(4))


# -- epsilon ------------------------------------------------------------------------

class TestEpsilon:
    def test_quadratic_form(self):
        eps = Epsilon(2, kappa=2.0)
        dx = np.array([1.0, 1.0])
        # 2 * dx^T I dx = 2 * 2 = 4
        assert eps.significance(dx) == pytest.approx(4.0)

    def test_first_update_is_neutral(self):
        eps = Epsilon(3)
        _, eps_hat = eps.update(np.array([0.5, 0.5, 0.5]))
        assert eps_hat == pytest.approx(0.0)

    def test_spike_gives_positive_hat(self):
        eps = Epsilon(2)
        eps.update(np.array([0.01, 0.01]))   # calm world
        eps.update(np.array([0.01, -0.01]))
        _, eps_hat = eps.update(np.array([0.9, 0.9]))  # sudden change
        assert eps_hat > 0.5

    def test_metric_weighting(self):
        G = np.diag([10.0, 0.1])
        eps = Epsilon(2, G=G)
        big = eps.significance(np.array([1.0, 0.0]))
        small = eps.significance(np.array([0.0, 1.0]))
        assert big > 50 * small


# -- resonance -------------------------------------------------------------------------

class TestResonance:
    def test_weights_decay(self):
        res = Resonance(2, rho=0.5)
        w = res.weights(4)
        assert np.allclose(w, [0.5, 0.25, 0.125, 0.0625])

    def test_empty_memory_zero_gradient(self):
        res = Resonance(3)
        assert np.allclose(res.gradient(np.ones(3)), 0.0)

    def test_most_recent_state_dominates(self):
        res = Resonance(2, rho=0.1)
        res.push(np.array([0.0, 0.0]))        # old
        res.push(np.array([1.0, 1.0]))        # recent
        grad = res.gradient(np.zeros(2))
        # grad = sum_k rho^k (p - s_{t-k}); the recent memory (weight 0.1)
        # dominates the old one (weight 0.01)
        assert np.allclose(grad, [0.1 * (0.0 - 1.0), 0.1 * (0.0 - 1.0)])

    def test_novelty_repels_habit_attracts(self):
        # memory sits at +1, the state starts at 0:
        # novelty mode pushes the state AWAY from the memory (negative
        # contribution), habit mode pulls it TOWARD the memory (positive).
        for mode, expected_sign in (("novelty", -1), ("habit", +1)):
            res = Resonance(2, rho=0.9, mode=mode)
            res.push(np.array([1.0, 0.0]))
            contrib = res.contribution(np.zeros(2), gamma=1.0)
            assert np.sign(contrib[0]) == expected_sign

    def test_depth_is_bounded(self):
        res = Resonance(2, depth=4)
        for _ in range(20):
            res.push(np.ones(2))
        assert len(res) == 4

    def test_rejects_bad_rho(self):
        with pytest.raises(ValueError):
            Resonance(2, rho=1.5)


# -- projection --------------------------------------------------------------------------

class TestLogicProjector:
    def test_identity_without_constraints(self):
        pi = LogicProjector(dim=4)
        assert np.allclose(pi.matrix(), np.eye(4))

    def test_projector_is_idempotent_and_symmetric(self):
        Jc = np.array([[1.0, -1.0, 0.0, 0.0], [0.0, 1.0, 1.0, 1.0]])
        pi = LogicProjector(Jc=Jc)
        P = pi.matrix()
        assert np.allclose(P @ P, P)
        assert np.allclose(P, P.T)

    def test_constraints_are_annihilated(self):
        Jc = np.array([[1.0, -1.0, 0.0], [0.5, 0.5, -1.0]])
        pi = LogicProjector(Jc=Jc)
        assert np.allclose(Jc @ pi.matrix(), 0.0, atol=1e-10)

    def test_projection_preserves_feasible_points(self):
        Jc = np.array([[1.0, -1.0, 0.0]])
        pi = LogicProjector(Jc=Jc)
        feasible = np.array([0.3, 0.3, -0.7])
        assert np.allclose(pi.project(feasible), feasible, atol=1e-10)

    def test_projection_moves_infeasible_to_subspace(self):
        Jc = np.array([[1.0, -1.0, 0.0]])
        pi = LogicProjector(Jc=Jc)
        moved = pi.project(np.array([1.0, 0.0, 0.0]))
        assert moved[0] == pytest.approx(moved[1])

    def test_rank_deficient_constraints(self):
        # two proportional rows -- pinv-based construction must survive
        Jc = np.array([[1.0, -1.0], [2.0, -2.0]])
        pi = LogicProjector(Jc=Jc)
        assert np.allclose(Jc @ pi.matrix(), 0.0, atol=1e-10)


# -- dynamics ------------------------------------------------------------------------------

class TestDynamicsOperator:
    def test_default_J_is_antisymmetric(self):
        dyn = DynamicsOperator(5, seed=11)
        assert dyn.is_creative()

    def test_default_D_is_dissipative(self):
        dyn = DynamicsOperator(5, seed=11)
        assert dyn.is_stabilising()

    def test_pure_creativity_preserves_norm(self):
        # J alone: the flow p' = J p is a rotation -> norm preserved
        rng = np.random.default_rng(5)
        A = rng.standard_normal((4, 4))
        J = (A - A.T) / 2.0
        dyn = DynamicsOperator(4, J=J, D=np.zeros((4, 4)))
        p = rng.standard_normal(4)
        dt = 1e-3
        p_next = p + dt * dyn.drift(p)
        assert np.linalg.norm(p_next) == pytest.approx(np.linalg.norm(p), rel=1e-4)

    def test_pure_dissipation_shrinks_norm(self):
        dyn = DynamicsOperator(4, J=np.zeros((4, 4)), D=np.diag([0.5] * 4))
        p = np.array([1.0, 1.0, 1.0, 1.0])
        p_next = p + 0.1 * dyn.drift(p)
        assert np.linalg.norm(p_next) < np.linalg.norm(p)

    def test_projected_drift_stays_in_subspace(self):
        Jc = np.array([[1.0, -1.0, 0.0]])
        pi = LogicProjector(Jc=Jc)
        dyn = DynamicsOperator(3, seed=2)
        drift = dyn.drift(np.array([0.2, 0.2, 0.5]), pi.matrix())
        assert (Jc @ drift)[0] == pytest.approx(0.0, abs=1e-10)
