"""Variational free energy F(p, o; theta).

    F(p, o; theta) = || g(p; theta) - Omega(o) ||_G^2  +  lambda * R_L(p)

where

* ``g(p; theta)`` is the generative model -- the state the agent *predicts*
  the observation should have produced. The reference implementation uses
  the identity generative model ``g(p) = p`` (theta is empty), so the
  first term reduces to the G-weighted squared prediction error.
* ``R_L(p) = 0.5 * ||p||^2`` is a state-cost regulariser (an "energy
  budget"): without it the state is free to saturate at the clip bounds.

Gradients (identity generative model):

    grad_p ||p - omega||_G^2 = 2 G (p - omega)
    grad_p R_L(p)            = p
"""

from __future__ import annotations

import numpy as np


class FreeEnergy:
    """F(p, o; theta) with an identity generative model."""

    def __init__(self, dim: int, G: np.ndarray | None = None,
                 lam: float = 0.01) -> None:
        self.dim = dim
        self.G = np.eye(dim) if G is None else np.asarray(G, dtype=float)
        if self.G.shape != (dim, dim):
            raise ValueError(f"G must be ({dim}, {dim}), got {self.G.shape}")
        self.lam = float(lam)

    def prediction_error(self, p: np.ndarray, omega: np.ndarray) -> np.ndarray:
        """Residual of the generative model: ``g(p) - Omega(o)``."""
        return np.asarray(p, dtype=float) - np.asarray(omega, dtype=float)

    def value(self, p: np.ndarray, omega: np.ndarray) -> float:
        """Numerical value of the free energy."""
        r = self.prediction_error(p, omega)
        quadratic = float(r @ self.G @ r)
        state_cost = 0.5 * self.lam * float(p @ p)
        return quadratic + state_cost

    def grad(self, p: np.ndarray, omega: np.ndarray) -> np.ndarray:
        """Gradient of F with respect to the state ``p``."""
        r = self.prediction_error(p, omega)
        return 2.0 * (self.G @ r) + self.lam * np.asarray(p, dtype=float)
