"""POLER phase quantization: continuous compression via angle encoding.

Instead of rounding a state coordinate ``p in [-1, 1]`` onto a *value*
grid (static INT4/FP4 quantization), POLER encodes it as a *phase*:

    theta = arccos(p),   |psi> = cos(theta/2)|0> + sin(theta/2)|1>

and quantizes the **angle** onto a uniform grid of ``levels`` points on
``[0, pi]``:

    p_q = cos(round(theta / step) * step),   step = pi / (levels - 1)

Because ``|cos a - cos b| <= |a - b|``, the reconstruction error is bounded
by the angular step alone -- the grid lives in the geometry of the state
(Born probabilities ``P(|0>) = (1 + p) / 2``), not in the algebra of the
weights. Useful levels:

    levels = 2   ->  p in {+1, -1}            (1 bit, the sign)
    levels = 3   ->  p in {+1, 0, -1}         (trits)
    levels = 2^b ->  uniform b-bit phase grid

The depth (number of levels) is chosen *per step* from the significance
energy epsilon: background perception compresses to 1-2 bits / trits,
an epsilon-spike unfolds the full phase space. See
:class:`AdaptiveDepth` -- this is the "dynamic in time and topology"
of docs/dynamic-quantization.md.
"""

from __future__ import annotations

import numpy as np


def phase_grid(levels: int) -> np.ndarray:
    """The reconstruction grid: ``cos(linspace(0, pi, levels))``.

    Descending from +1 to -1; ``levels`` points define a
    ``log2(levels)``-bit phase code.
    """
    if levels < 2:
        raise ValueError(f"levels must be >= 2, got {levels}")
    return np.cos(np.linspace(0.0, np.pi, levels))


def effective_bits(levels: int) -> float:
    """Depth of a ``levels``-point phase grid, in bits: ``log2(levels)``."""
    return float(np.log2(levels))


def phase_quantize(p: np.ndarray, levels: int) -> np.ndarray:
    """Compress state coordinates onto the ``levels``-point phase grid."""
    p = np.clip(np.asarray(p, dtype=float), -1.0, 1.0)
    if levels < 2:
        raise ValueError(f"levels must be >= 2, got {levels}")
    theta = np.arccos(p)
    step = np.pi / (levels - 1)
    theta_q = np.round(theta / step) * step
    return np.cos(theta_q)


def phase_quantization_error(p: np.ndarray, levels: int) -> tuple[float, float]:
    """(max abs error, RMSE) of the phase compression of ``p``."""
    q = phase_quantize(p, levels)
    err = np.abs(q - np.clip(np.asarray(p, dtype=float), -1.0, 1.0))
    return float(err.max()), float(np.sqrt(np.mean(err ** 2)))


class AdaptiveDepth:
    """Significance-driven phase depth: ``eps_hat -> levels``.

    The normalised significance ``eps_hat`` (from
    :class:`poler_quantum.core.epsilon.Epsilon`) is scale-free: ``> 0``
    means "more significant than the running average" (a spike), ``< 0``
    a calmer-than-usual world. The policy maps it monotonically onto the
    phase grid size:

    * ``eps_hat <= eps_lo``  (background)  ->  ``levels_min``  (1-2 bits /
      trits -- the minimum-dissipation regime);
    * ``eps_hat >= eps_hi``  (a spike)     ->  ``levels_max``  (the full
      phase space unfolds);
    * in between the depth grows **exponentially** (perceptually, bits --
      not levels -- are the natural scale), so the interpolation is done
      in ``log2(levels)``.

    Monotone by construction; the bounds are inclusive.
    """

    def __init__(self, levels_min: int = 3, levels_max: int = 256,
                 eps_lo: float = 0.0, eps_hi: float = 2.0) -> None:
        if levels_min < 2:
            raise ValueError(f"levels_min must be >= 2, got {levels_min}")
        if levels_max < levels_min:
            raise ValueError(
                f"levels_max ({levels_max}) must be >= levels_min ({levels_min})")
        if not eps_hi > eps_lo:
            raise ValueError(f"eps_hi ({eps_hi}) must be > eps_lo ({eps_lo})")
        self.levels_min = int(levels_min)
        self.levels_max = int(levels_max)
        self.eps_lo = float(eps_lo)
        self.eps_hi = float(eps_hi)

    def levels_for(self, eps_hat: float) -> int:
        """Phase grid size for the current normalised significance."""
        s = (float(eps_hat) - self.eps_lo) / (self.eps_hi - self.eps_lo)
        s = min(max(s, 0.0), 1.0)
        if s <= 0.0:
            return self.levels_min
        if s >= 1.0:
            return self.levels_max
        lo = np.log2(self.levels_min)
        hi = np.log2(self.levels_max)
        levels = 2.0 ** (lo + s * (hi - lo))
        return int(min(max(round(levels), self.levels_min), self.levels_max))

    def bits_for(self, eps_hat: float) -> float:
        """Same mapping in bits (diagnostic)."""
        return effective_bits(self.levels_for(eps_hat))
