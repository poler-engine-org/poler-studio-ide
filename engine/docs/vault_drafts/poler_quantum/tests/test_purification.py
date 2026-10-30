"""Tests of McWeeny purification (RQ2, v1.1.0).

The invariant under repair: ``P^2 = P`` (idempotency) is restored exactly,
in 1-2 iterations, for any symmetric corruption smaller than half the
spectral gap. The subspace drifts by O(corruption); the rank is preserved
as long as no eigenvalue crosses the unstable fixed point 1/2.
"""

import numpy as np
import pytest

from poler_quantum.core.purification import (
    PurificationResult,
    idempotency_error,
    mcweeny_purify,
    mcweeny_step,
    projector_from_constraints,
    quantize_entries,
    subspace_error,
    symmetric_noise,
)


@pytest.fixture(scope="module")
def projector() -> np.ndarray:
    """A rank-(n-2) null-space projector from two random constraints."""
    rng = np.random.default_rng(7)
    Jc = rng.standard_normal((2, 6))
    return projector_from_constraints(Jc)


# -- the polynomial itself ---------------------------------------------------

class TestMcWeenyStep:
    def test_identity_and_zero_are_fixed_points(self):
        I3 = np.eye(3)
        assert np.allclose(mcweeny_step(I3), I3)
        assert np.allclose(mcweeny_step(np.zeros((3, 3))), np.zeros((3, 3)))

    def test_eigenvalue_mapping_near_one(self):
        # f(1 - d) = 1 - 3 d^2 + 2 d^3  -> quadratic contraction
        lam = 0.99
        expected = 3 * lam ** 2 - 2 * lam ** 3
        assert mcweeny_step(np.array([[lam]]))[0, 0] == pytest.approx(expected)

    def test_eigenvalue_mapping_near_zero(self):
        lam = 0.01
        expected = 3 * lam ** 2 - 2 * lam ** 3
        assert mcweeny_step(np.array([[lam]]))[0, 0] == pytest.approx(expected)

    def test_half_is_an_unstable_fixed_point(self):
        assert mcweeny_step(np.array([[0.5]]))[0, 0] == pytest.approx(0.5)

    def test_symmetric_matrix_stays_symmetric(self, projector):
        Q = quantize_entries(projector, 16)
        S = mcweeny_step(Q)
        assert np.allclose(S, S.T, atol=1e-12)


# -- the repair ---------------------------------------------------------------

class TestMcWeenyPurify:
    def test_exact_projector_is_returned_as_is(self, projector):
        res = mcweeny_purify(projector, max_iters=3)
        assert res.iterations == 0          # already idempotent
        assert np.allclose(res.matrix, projector, atol=1e-12)

    def test_repair_after_entry_quantization(self, projector):
        Q = quantize_entries(projector, 16)
        err_before = idempotency_error(Q)
        assert err_before > 1e-3            # quantization broke the invariant
        res = mcweeny_purify(Q, max_iters=2)
        assert idempotency_error(res.matrix) < err_before / 100.0
        assert res.converged or res.iterations == 2

    def test_two_iterations_repair_fine_grid(self, projector):
        # the "1-2 takta" claim at 8-bit grid scale:
        # measured 5.2e-03 -> 5.4e-09 in two iterations
        Q = quantize_entries(projector, 256)
        assert idempotency_error(Q) > 1e-4
        res = mcweeny_purify(Q, max_iters=2)
        assert idempotency_error(res.matrix) < 1e-7

    def test_coarse_grid_needs_a_few_more_iterations(self, projector):
        # 16-level grid: 1.2e-01 -> 2.5e-06 (3 iters) -> machine (5 iters)
        Q = quantize_entries(projector, 16)
        res = mcweeny_purify(Q, max_iters=3)
        assert idempotency_error(res.matrix) < 1e-5
        res = mcweeny_purify(Q, max_iters=6)
        assert idempotency_error(res.matrix) < 1e-12

    def test_error_trace_is_decreasing(self, projector):
        Q = quantize_entries(projector, 16) + symmetric_noise(6, 1e-3, seed=3)
        res = mcweeny_purify(Q, max_iters=6, tol=1e-14)
        for a, b in zip(res.error_trace, res.error_trace[1:]):
            assert b < a
        assert res.converged

    def test_convergence_is_quadratic(self, projector):
        # e_{k+1} ~ 3 e_k^2 (in spectral terms): ratio e_{k+1}/e_k^2 is bounded
        Q = quantize_entries(projector, 16)
        res = mcweeny_purify(Q, max_iters=4, tol=0.0)
        errs = [idempotency_error(Q)] + res.error_trace
        for e0, e1 in zip(errs, errs[1:]):
            if e0 > 1e-12:
                assert e1 / (e0 ** 2) < 50.0   # quadratic up to constants

    def test_rank_is_preserved_for_moderate_corruption(self, projector):
        Q = quantize_entries(projector, 32)
        res = mcweeny_purify(Q, max_iters=3)
        rank_before = int(round(np.trace(Q)))
        rank_after = int(round(np.trace(res.matrix)))
        assert rank_after == rank_before == 4   # 6 - 2 constraints

    def test_eigenvalues_pulled_to_the_poles(self, projector):
        Q = quantize_entries(projector, 16)
        res = mcweeny_purify(Q, max_iters=6, tol=1e-14)
        eig = np.linalg.eigvalsh(res.matrix)
        # every eigenvalue is now within eps of {0, 1}
        assert np.all(np.minimum(np.abs(eig), np.abs(eig - 1.0)) < 1e-10)
        assert eig.min() >= -1e-9
        assert eig.max() <= 1.0 + 1e-9

    def test_repair_after_symmetric_noise(self, projector):
        # measurement / estimation noise of norm 0.1 (a heavy corruption):
        # measured 8.8e-02 -> 3.8e-12 in four iterations
        Q = projector + symmetric_noise(6, 0.1, seed=11)
        res = mcweeny_purify(Q, max_iters=4)
        assert idempotency_error(res.matrix) < 1e-9

    def test_subspace_drift_is_proportional_to_corruption(self, projector):
        for scale in (0.02, 0.05):
            Q = projector + symmetric_noise(6, scale, seed=5)
            res = mcweeny_purify(Q, max_iters=3)
            assert subspace_error(res.matrix, projector) < 6.0 * scale

    def test_born_sampling_scale_noise_is_repaired(self, projector):
        # finite-shot statistics: noise ~ 1/sqrt(N) for N shots;
        # measured: N=16 (noise 0.25) -> 4.1e-11 in five iterations
        for shots in (16, 64):
            noise_level = 1.0 / np.sqrt(shots)
            Q = projector + symmetric_noise(6, noise_level, seed=shots)
            res = mcweeny_purify(Q, max_iters=6)
            assert idempotency_error(res.matrix) < 1e-8
            assert subspace_error(res.matrix, projector) < 8.0 * noise_level

    def test_coarse_grid_caveat_is_real(self, projector):
        # honest limit: a 4-level grid is a huge corruption -- the INVARIANT
        # is still repaired, but the subspace drifts far (quantization can
        # rotate eigenvectors across the spectral gap). Repair != identity.
        Q = quantize_entries(projector, 4)
        res = mcweeny_purify(Q, max_iters=12)
        assert idempotency_error(res.matrix) < 1e-6
        assert subspace_error(res.matrix, projector) > 0.1

    def test_non_square_input_raises(self):
        with pytest.raises(ValueError):
            mcweeny_purify(np.zeros((3, 4)))

    def test_result_fields(self, projector):
        Q = quantize_entries(projector, 16)
        res = mcweeny_purify(Q, max_iters=2)
        assert isinstance(res, PurificationResult)
        assert res.matrix.shape == (6, 6)
        assert len(res.error_trace) == res.iterations


# -- the corruption operators --------------------------------------------------

class TestQuantizeEntries:
    def test_two_levels_is_a_binary_grid(self):
        M = np.array([[0.0, 0.9], [-0.7, 0.3]])
        Q = quantize_entries(M, 2)
        values = np.unique(np.round(Q, 9))
        assert np.allclose(values, [-0.7, 0.9])

    def test_output_lives_on_the_grid(self, projector):
        Q = quantize_entries(projector, 16)
        lo, hi = projector.min(), projector.max()
        step = (hi - lo) / 15
        on_grid = np.abs((Q - lo) / step - np.round((Q - lo) / step))
        assert np.all(on_grid < 1e-9)

    def test_extremes_are_exact(self, projector):
        Q = quantize_entries(projector, 16)
        assert Q.min() == pytest.approx(projector.min())
        assert Q.max() == pytest.approx(projector.max())

    def test_constant_matrix_is_unchanged(self):
        M = np.full((3, 3), 0.42)
        assert np.array_equal(quantize_entries(M, 5), M)

    def test_levels_validation(self):
        with pytest.raises(ValueError):
            quantize_entries(np.eye(2), 1)

    def test_quantization_breaks_idempotency(self, projector):
        # the reason McWeeny exists: a static grid is NOT a projector
        Q = quantize_entries(projector, 16)
        assert idempotency_error(Q) > 1e-3
        assert not np.allclose(Q @ Q, Q)


class TestHelpers:
    def test_projector_from_constraints_matches_null_space(self):
        Jc = np.array([[1.0, -1.0, 0.0, 0.0]])
        Pi = projector_from_constraints(Jc)
        assert np.allclose(Jc @ Pi, 0.0, atol=1e-12)   # annihilates Jc
        assert idempotency_error(Pi) < 1e-12
        assert np.allclose(Pi, Pi.T)

    def test_idempotency_error_zero_for_exact_projector(self, projector):
        assert idempotency_error(projector) < 1e-12

    def test_symmetric_noise_has_requested_norm(self):
        E = symmetric_noise(5, 0.3, seed=1)
        assert np.linalg.norm(E, "fro") == pytest.approx(0.3)
        assert np.allclose(E, E.T)

    def test_subspace_error_zero_for_identical(self, projector):
        assert subspace_error(projector, projector) == 0.0
