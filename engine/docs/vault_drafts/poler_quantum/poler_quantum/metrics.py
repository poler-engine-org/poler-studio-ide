"""Evaluation metrics for tracking benchmarks."""

from __future__ import annotations

import numpy as np


def rmse(traj: np.ndarray, target: np.ndarray, warmup: int = 20) -> float:
    """Root-mean-square tracking error after the warm-up phase."""
    traj = np.asarray(traj, dtype=float)[warmup:]
    target = np.asarray(target, dtype=float)[warmup:]
    return float(np.sqrt(np.mean((traj - target) ** 2)))


def recovery_steps(traj: np.ndarray, target: np.ndarray,
                   switch_t: int, window: int = 40,
                   pre_error: float | None = None) -> int:
    """Steps needed to re-lock onto the target after a regime switch.

    A runner is considered "re-locked" when its instantaneous error falls
    below ``1.5 * pre_error`` (the median error of the *same* runner in the
    30 steps preceding the switch). Returns ``window`` (the cap) if it
    never recovers.
    """
    traj = np.asarray(traj, dtype=float)
    target = np.asarray(target, dtype=float)
    if pre_error is None:
        lo = max(0, switch_t - 30)
        pre = np.linalg.norm(traj[lo:switch_t] - target[lo:switch_t], axis=-1)
        pre_error = float(np.median(pre)) if len(pre) else 0.0
    threshold = 1.5 * max(pre_error, 1e-9)
    for k in range(switch_t, min(len(traj), switch_t + window)):
        err = float(np.linalg.norm(traj[k] - target[k]))
        if err <= threshold:
            return k - switch_t
    return window


def path_smoothness(traj: np.ndarray, warmup: int = 20) -> float:
    """Mean step size of the trajectory (lower = smoother decisions)."""
    traj = np.asarray(traj, dtype=float)[warmup:]
    if len(traj) < 2:
        return 0.0
    deltas = np.linalg.norm(np.diff(traj, axis=0), axis=-1)
    return float(np.mean(deltas))


def mean_free_energy(values: np.ndarray, warmup: int = 20) -> float:
    """Time-average of the free energy after warm-up."""
    values = np.asarray(values, dtype=float)[warmup:]
    return float(np.mean(values)) if len(values) else float("nan")
