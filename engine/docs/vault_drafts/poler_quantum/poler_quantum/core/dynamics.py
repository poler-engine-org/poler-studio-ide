"""Dynamics operator S(p).

    S(p) = Pi_Lambda (J - D) Pi_Lambda p

The *free* part of POLER cognition, living inside the constraint subspace:

* ``J`` is antisymmetric (``J^T = -J``): a rotation generator -- pure
  *creativity*. It moves the state along level sets of the energy without
  dissipating it (norm-preserving flow).
* ``D`` is symmetric positive semidefinite: a dissipative *stabilisation*
  term that contracts the state toward the origin of the subspace.

Together they make the state *explore* (J) while remaining *bound* (D),
before perception and resonance gradients are even applied.
"""

from __future__ import annotations

import numpy as np


class DynamicsOperator:
    """S(p) = Pi (J - D) Pi p with creativity J and stabiliser D."""

    def __init__(self, dim: int, J: np.ndarray | None = None,
                 D: np.ndarray | None = None, seed: int | None = None) -> None:
        self.dim = dim
        if J is None:
            rng = np.random.default_rng(seed)
            A = rng.standard_normal((dim, dim))
            J = (A - A.T) / 2.0  # random antisymmetric
        if D is None:
            D = np.diag(np.full(dim, 0.1))
        self.J = np.asarray(J, dtype=float)
        self.D = np.asarray(D, dtype=float)
        if self.J.shape != (dim, dim):
            raise ValueError(f"J must be ({dim}, {dim}), got {self.J.shape}")
        if self.D.shape != (dim, dim):
            raise ValueError(f"D must be ({dim}, {dim}), got {self.D.shape}")

    # -- validation helpers ----------------------------------------------------

    def is_creative(self) -> bool:
        """True if J is (numerically) antisymmetric."""
        return bool(np.allclose(self.J, -self.J.T, atol=1e-12))

    def is_stabilising(self, atol: float = 1e-10) -> bool:
        """True if D is symmetric positive semidefinite."""
        sym = np.allclose(self.D, self.D.T, atol=atol)
        psd = np.all(np.linalg.eigvalsh((self.D + self.D.T) / 2.0) >= -atol)
        return bool(sym and psd)

    # -- math -------------------------------------------------------------------

    def drift(self, p: np.ndarray, Pi: np.ndarray | None = None) -> np.ndarray:
        """Free dynamics S(p) = Pi (J - D) Pi p."""
        p = np.asarray(p, dtype=float)
        if Pi is not None:
            return Pi @ ((self.J - self.D) @ (Pi @ p))
        return (self.J - self.D) @ p

    def dissipation_rate(self, p: np.ndarray) -> float:
        """Instantaneous dissipation ``-d/dt ||p||^2 / 2`` contributed by D."""
        p = np.asarray(p, dtype=float)
        return float(p @ self.D @ p)
