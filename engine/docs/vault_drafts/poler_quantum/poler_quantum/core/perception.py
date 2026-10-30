"""Perception Omega(o_t).

The sensory layer maps raw observations into a bounded internal state
space. The canonical POLER mapping is the hyperbolic tangent, which keeps
every perceived coordinate inside (-1, 1) so that the free-energy metric
G operates on a compact domain.

    Omega(o_t) = tanh(o_t)
"""

from __future__ import annotations

import numpy as np


class Perception:
    """Omega(o_t): embedding of sensory data into bounded state space.

    Parameters
    ----------
    kind:
        ``"tanh"`` (default, canonical POLER mapping) or ``"linear"`` --
        identity clipping to [-1, 1], useful for debugging and for
        baselines that need an unwarped perception.
    """

    def __init__(self, kind: str = "tanh") -> None:
        if kind not in ("tanh", "linear"):
            raise ValueError(f"unknown perception kind: {kind!r}")
        self.kind = kind

    def omega(self, o_t: np.ndarray) -> np.ndarray:
        """Embed a raw observation ``o_t`` into internal state space."""
        o_t = np.asarray(o_t, dtype=float)
        if self.kind == "tanh":
            return np.tanh(o_t)
        return np.clip(o_t, -1.0, 1.0)

    def __call__(self, o_t: np.ndarray) -> np.ndarray:
        return self.omega(o_t)

    def __repr__(self) -> str:  # pragma: no cover - trivial
        return f"Perception(kind={self.kind!r})"
