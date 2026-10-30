"""Non-stationary perception tracking task.

A target signal lives in [-1, 1]^dim and evolves as an Ornstein-Uhlenbeck
drift. At given switch times the target *jumps* (a regime change: new
preference, new context) and continues drifting afterwards. The agent only
ever sees a noisy observation of the target and must track it.

This task stresses exactly the properties POLER claims:

* steady-state accuracy   <- free-energy descent
* reaction to switches    <- epsilon-driven attention spikes
* structured exploration  <- resonance + quantum proposals
* constraint compliance   <- Pi_Lambda projection (constrained variant)
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np


@dataclass
class TaskSpec:
    """Specification of a tracking task instance."""

    T: int = 300
    dim: int = 8
    drift: float = 0.02          # OU drift scale
    theta: float = 0.05          # OU mean reversion
    noise: float = 0.10          # observation noise
    jump: float = 0.9            # regime jump amplitude
    switches: tuple | None = None  # switch times; None = auto (T/3, 2T/3)
    seed: int = 7

    def __post_init__(self) -> None:
        if self.switches is None:
            self.switches = (self.T // 3, 2 * self.T // 3)

    def validate(self) -> None:
        if self.T < 50:
            raise ValueError("T must be >= 50")
        if self.dim < 1:
            raise ValueError("dim must be >= 1")
        if any(not (0 < s < self.T - 10) for s in self.switches):
            raise ValueError("switches must lie strictly inside (0, T-10)")


@dataclass
class TaskInstance:
    """A concrete realisation of the task."""

    spec: TaskSpec
    target: np.ndarray = field(init=False)     # (T, dim) latent target
    observations: np.ndarray = field(init=False)  # (T, dim) noisy views

    def __post_init__(self) -> None:
        rng = np.random.default_rng(self.spec.seed)
        s = self.spec
        target = np.zeros((s.T, s.dim))
        x = rng.uniform(-0.3, 0.3, size=s.dim)
        for t in range(s.T):
            if t in s.switches:
                # regime jump: a fresh random direction, then renormalise
                direction = rng.standard_normal(s.dim)
                direction /= (np.linalg.norm(direction) + 1e-12)
                x = x + s.jump * direction
                x = np.clip(x, -0.95, 0.95)
            # OU drift
            x = x - s.theta * x + s.drift * rng.standard_normal(s.dim)
            x = np.clip(x, -0.95, 0.95)
            target[t] = x
        noise = s.noise * rng.standard_normal((s.T, s.dim))
        obs = np.clip(target + noise, -3.0, 3.0)  # raw observation, unbounded
        self.target = target
        self.observations = obs


def tracking_task(spec: TaskSpec | None = None, **overrides) -> TaskInstance:
    """Generate a tracking task instance (keyword args override the spec)."""
    spec = spec or TaskSpec()
    params = {**spec.__dict__, **overrides}
    if "T" in overrides and "switches" not in overrides:
        # the auto-resolved switch times were computed for the old horizon:
        # drop them so they are recomputed against the new T
        params["switches"] = None
    spec = TaskSpec(**params)
    spec.validate()
    return TaskInstance(spec=spec)
