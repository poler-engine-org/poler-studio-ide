"""Memory resonance R[n].

    R[n] = sum_k rho^k * s_{t-k},   k = 1..n

Past perceived states ``s_{t-k}``` enter the present with exponentially
decaying weights ``rho^k``. POLER offers two readings of the resonance
gradient

    grad_p E_res = sum_k rho^k (p - s_{t-k})

* ``mode="novelty"`` (default, canonical): the update uses ``+gamma * grad``
  which *repels* the state from its own past -- an anti-habituation force.
  The agent refuses to loop on the same thought.
* ``mode="habit"``: the update uses ``-gamma * grad`` which *attracts* the
  state toward the resonant memory -- habit formation.
"""

from __future__ import annotations

from collections import deque

import numpy as np


class Resonance:
    """Exponentially weighted episodic memory of perceived states."""

    def __init__(self, dim: int, rho: float = 0.9, depth: int = 8,
                 mode: str = "novelty") -> None:
        if not 0.0 <= rho <= 1.0:
            raise ValueError(f"rho must be in [0, 1], got {rho}")
        if mode not in ("novelty", "habit"):
            raise ValueError(f"unknown resonance mode: {mode!r}")
        self.dim = dim
        self.rho = float(rho)
        self.depth = int(depth)
        self.mode = mode
        self._memory: deque[np.ndarray] = deque(maxlen=self.depth)

    # -- state ----------------------------------------------------------------

    def push(self, s: np.ndarray) -> None:
        """Store a perceived state in the resonance memory."""
        self._memory.append(np.asarray(s, dtype=float).copy())

    def __len__(self) -> int:
        return len(self._memory)

    def clear(self) -> None:
        self._memory.clear()

    # -- math -----------------------------------------------------------------

    def weights(self, n: int | None = None) -> np.ndarray:
        """Weights ``rho^k`` for k = 1..n (most recent state first)."""
        n = len(self._memory) if n is None else n
        return np.array([self.rho ** k for k in range(1, n + 1)])

    def gradient(self, p: np.ndarray) -> np.ndarray:
        """Gradient of the resonance energy at state ``p``.

        Indexing convention: ``self._memory[-k]`` is ``s_{t-k+1}``, i.e.
        the most recent entry has weight ``rho^1``.
        """
        p = np.asarray(p, dtype=float)
        grad = np.zeros_like(p)
        m = len(self._memory)
        for j, s in enumerate(reversed(self._memory)):  # j = 0 -> most recent
            w = self.rho ** (j + 1)
            grad += w * (p - s)
        return grad

    def contribution(self, p: np.ndarray, gamma: float) -> np.ndarray:
        """Signed resonance term added to the update (mode-dependent)."""
        grad = self.gradient(p)
        return (gamma if self.mode == "novelty" else -gamma) * grad

    # Resonance memory is not part of the determinism-sensitive state that
    # needs explicit seeding; it is fully determined by pushed states.
