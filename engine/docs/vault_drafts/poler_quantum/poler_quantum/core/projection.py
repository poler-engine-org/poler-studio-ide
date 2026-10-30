"""Logic / ethics projection Pi_Lambda.

    Pi_Lambda = I - J_c^T (J_c J_c^T)^{-1} J_c

For a set of linear constraints ``c(p) = J_c p = 0`` (the "logic" of the
agent: invariants it must never violate), ``Pi_Lambda`` is the orthogonal
projector onto the null space of ``J_c``. Every POLER update is passed
through it, so the state trajectory can never leave the feasible subspace
-- decisions are *creative inside the constraints*.

Properties (tested): idempotent (Pi^2 = Pi), symmetric, annihilates every
constraint row (J_c Pi = 0).
"""

from __future__ import annotations

import numpy as np


class LogicProjector:
    """Orthogonal projector onto the null space of the constraint set."""

    def __init__(self, Jc: np.ndarray | None = None, dim: int | None = None) -> None:
        if Jc is None:
            if dim is None:
                raise ValueError("either Jc or dim must be given")
            Jc = np.zeros((0, dim))
        self.Jc = np.atleast_2d(np.asarray(Jc, dtype=float))
        self.dim = self.Jc.shape[1]
        self._cached: np.ndarray | None = None

    # -- math -----------------------------------------------------------------

    def matrix(self) -> np.ndarray:
        """The projector ``Pi_Lambda`` as an explicit (dim, dim) matrix."""
        if self._cached is not None:
            return self._cached
        Jc = self.Jc
        if Jc.size == 0 or np.allclose(Jc, 0.0):
            Pi = np.eye(self.dim)
        else:
            # Pseudo-inverse based construction: robust to rank deficiency.
            # Pi = I - Jc^+ Jc  (equals the formula above when Jc has full row rank)
            Pi = np.eye(self.dim) - np.linalg.pinv(Jc) @ Jc
        self._cached = Pi
        return Pi

    def project(self, v: np.ndarray) -> np.ndarray:
        """Project a vector onto the constraint null space."""
        v = np.asarray(v, dtype=float)
        return self.matrix() @ v

    def feasible(self, p: np.ndarray, atol: float = 1e-9) -> bool:
        """Check whether a state satisfies all constraints."""
        if self.Jc.size == 0:
            return True
        return bool(np.all(np.abs(self.Jc @ np.asarray(p, dtype=float)) <= atol))

    def __call__(self, v: np.ndarray) -> np.ndarray:
        return self.project(v)
