"""Significance energy epsilon.

    eps_t = kappa * dx^T G(p) dx,   dx = Omega(o_t) - Omega(o_{t-1})

Epsilon is the "emotional response" of POLER: a quadratic form measuring
how *significant* the change in perception is, weighted by the metric G.
A large epsilon means the world just shifted -- and POLER answers with an
attention spike: the effective learning rate is modulated (always
positively) as

    eta_eff = eta * exp(beta * tanh(eps_hat))

where ``eps_hat`` is epsilon normalised by an exponential running average
of its own magnitude (so the response is scale-free).
"""

from __future__ import annotations

import numpy as np


class Epsilon:
    """Significance energy with a running normaliser."""

    def __init__(self, dim: int, kappa: float = 1.0,
                 G: np.ndarray | None = None,
                 ema_beta: float = 0.9) -> None:
        self.dim = dim
        self.kappa = float(kappa)
        self.G = np.eye(dim) if G is None else np.asarray(G, dtype=float)
        if self.G.shape != (dim, dim):
            raise ValueError(f"G must be ({dim}, {dim}), got {self.G.shape}")
        self.ema_beta = float(ema_beta)
        # Running mean of |eps| for scale-free normalisation.
        self.ema = 0.0
        self.seen = 0

    def significance(self, dx: np.ndarray) -> float:
        """Raw significance energy of a perception change ``dx``."""
        dx = np.asarray(dx, dtype=float)
        return self.kappa * float(dx @ self.G @ dx)

    def update(self, dx: np.ndarray) -> tuple[float, float]:
        """Compute eps for ``dx`` and fold it into the running average.

        Returns ``(eps, eps_hat)`` where ``eps_hat`` is the normalised
        significance used for attention modulation. The running average is
        initialised lazily from the first *non-zero* sample, so that the
        zero change of the very first step does not fake an infinitely
        significant world afterwards.
        """
        eps = self.significance(dx)
        if self.seen == 0:
            if eps > 0.0:
                self.ema = eps
                self.seen = 1
            scale = eps if eps > 0.0 else 1.0
        else:
            self.ema = self.ema_beta * self.ema + (1.0 - self.ema_beta) * eps
            self.seen += 1
            scale = max(self.ema, 1e-12)
        eps_hat = eps / scale - 1.0  # >0 means "more significant than usual"
        return eps, eps_hat

    def reset(self) -> None:
        self.ema = 0.0
        self.seen = 0
