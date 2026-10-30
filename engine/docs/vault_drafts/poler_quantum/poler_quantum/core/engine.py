"""The POLER[n] engine: the complete cognitive cycle.

Pipeline (see docs/POLER_Attention_Core.md):

    Omega(o_t) -> F(p, o; theta) -> Pi_Lambda -> eps -> R[n] -> S(p) -> p_{t+1}

One step, in order:

1. **Perception**       ``omega_t = tanh(o_t)``
2. **Significance**     ``eps = kappa * d_omega^T G d_omega`` -- the emotional
   response to *change*; normalised against a running average it modulates
   the effective learning rate multiplicatively:
   ``eta_eff = eta * exp(beta * tanh(eps_hat))`` -- always positive, spiking
   when the world shifts and calming down when nothing new happens.
3. **Free energy**      ``grad_F = 2 G (p - omega) + lam * p``
4. **Resonance**        novelty/habit gradient over the episodic memory
   ``sum_k rho^k (p - s_{t-k})``.
5. **Projection + dynamics**:
   ``dp = Pi_Lambda (-grad_F + gamma_res * grad_res) + tau * S(p)``
6. **Update**: ``p_{t+1} = clip(p_t + eta_eff * dp)``

The state is clipped to [-1, 1]^dim to match the perception range.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np

from .dynamics import DynamicsOperator
from .epsilon import Epsilon
from .free_energy import FreeEnergy
from .perception import Perception
from .projection import LogicProjector
from .resonance import Resonance


@dataclass
class PolerConfig:
    """All knobs of the POLER[n] engine in one place."""

    dim: int = 8
    # learning
    eta: float = 0.10             # base learning rate
    lam: float = 0.01             # state-cost weight in F
    # significance (epsilon)
    kappa: float = 1.0            # significance sensitivity
    beta: float = 1.0             # attention modulation strength (log-scale)
    # resonance
    rho: float = 0.90             # memory decay
    gamma_res: float = 0.10       # resonance strength
    depth: int = 8                # episodic memory depth
    resonance_mode: str = "novelty"  # "novelty" | "habit"
    # dynamics
    tau: float = 0.10             # timescale of the free dynamics S(p)
    # misc
    clip: float = 1.0
    seed: int | None = 0

    def validate(self) -> None:
        if self.dim < 1:
            raise ValueError("dim must be >= 1")
        if not 0.0 < self.eta <= 1.0:
            raise ValueError("eta must be in (0, 1]")
        if not 0.0 <= self.rho <= 1.0:
            raise ValueError("rho must be in [0, 1]")
        if self.resonance_mode not in ("novelty", "habit"):
            raise ValueError("resonance_mode must be 'novelty' or 'habit'")


@dataclass
class StepResult:
    """Diagnostics of a single cognitive step."""

    t: int
    p: np.ndarray                 # new state p_{t+1}
    omega: np.ndarray             # perception of the observation
    eps: float                    # raw significance energy
    eps_hat: float                # normalised significance
    eta_eff: float                # attention-modulated learning rate
    free_energy: float            # F evaluated at the *old* state
    free_energy_next: float       # F evaluated at the new state
    grad_norm: float              # ||Pi(-grad_F + gamma*grad_res)||


@dataclass
class RunReport:
    """Aggregate diagnostics of a full run."""

    steps: list[StepResult] = field(default_factory=list)

    @property
    def states(self) -> np.ndarray:
        return np.array([s.p for s in self.steps])

    @property
    def eps(self) -> np.ndarray:
        return np.array([s.eps for s in self.steps])

    @property
    def eta_eff(self) -> np.ndarray:
        return np.array([s.eta_eff for s in self.steps])

    @property
    def free_energy(self) -> np.ndarray:
        return np.array([s.free_energy for s in self.steps])


class PolerEngine:
    """The classical POLER[n] attention core."""

    def __init__(self, config: PolerConfig | None = None,
                 Jc: np.ndarray | None = None,
                 G: np.ndarray | None = None,
                 J: np.ndarray | None = None,
                 D: np.ndarray | None = None) -> None:
        self.cfg = config or PolerConfig()
        self.cfg.validate()
        d = self.cfg.dim

        self.perception = Perception("tanh")
        self.free_energy = FreeEnergy(d, G=G, lam=self.cfg.lam)
        self.epsilon = Epsilon(d, kappa=self.cfg.kappa, G=G)
        self.resonance = Resonance(d, rho=self.cfg.rho, depth=self.cfg.depth,
                                   mode=self.cfg.resonance_mode)
        self.projector = LogicProjector(Jc=Jc, dim=d)
        self.dynamics = DynamicsOperator(d, J=J, D=D,
                                          seed=None if self.cfg.seed is None
                                          else self.cfg.seed + 1)

        # internal state
        self.t = 0
        self.p = np.zeros(d)
        self._last_omega: np.ndarray | None = None

    # -- lifecycle -------------------------------------------------------------

    def reset(self, p0: np.ndarray | None = None) -> None:
        """Reset the agent to an initial state."""
        p0 = np.zeros(self.cfg.dim) if p0 is None else np.asarray(p0, float)
        if p0.shape != (self.cfg.dim,):
            raise ValueError(f"p0 must have shape ({self.cfg.dim},)")
        # Start inside the feasible subspace.
        self.p = self.projector.project(np.clip(p0, -self.cfg.clip, self.cfg.clip))
        self.t = 0
        self._last_omega = None
        self.resonance.clear()
        self.epsilon.reset()

    def ensure_ready(self) -> None:
        if self._last_omega is None:
            # first call ever -> behave as if reset
            self.reset(self.p)

    # -- the cognitive step ------------------------------------------------------

    def step(self, o_t: np.ndarray) -> StepResult:
        """One full POLER cycle: perceive -> feel -> remember -> move."""
        self.ensure_ready()
        omega = self.perception.omega(o_t)

        # (2) significance -- emotional response to change
        d_omega = (omega - self._last_omega) if self._last_omega is not None \
            else np.zeros_like(omega)
        eps, eps_hat = self.epsilon.update(d_omega)
        eta_eff = self.cfg.eta * float(np.exp(self.cfg.beta * np.tanh(eps_hat)))

        # (3) free energy gradient
        f_before = self.free_energy.value(self.p, omega)
        grad_F = self.free_energy.grad(self.p, omega)

        # (4) memory resonance gradient
        grad_res = self.resonance.gradient(self.p)
        sign = 1.0 if self.cfg.resonance_mode == "novelty" else -1.0

        # (5) projected update + free dynamics
        Pi = self.projector.matrix()
        raw = Pi @ (-grad_F + sign * self.cfg.gamma_res * grad_res)
        drift = self.cfg.tau * self.dynamics.drift(self.p, Pi)
        dp = raw + drift

        # (6) attention-modulated update, clipped to the perception range
        p_next = np.clip(self.p + eta_eff * dp,
                         -self.cfg.clip, self.cfg.clip)
        f_after = self.free_energy.value(p_next, omega)

        # bookkeeping
        self.resonance.push(omega)
        self._last_omega = omega
        self.p = p_next
        self.t += 1

        return StepResult(
            t=self.t,
            p=p_next.copy(),
            omega=omega.copy(),
            eps=eps,
            eps_hat=float(eps_hat),
            eta_eff=float(eta_eff),
            free_energy=f_before,
            free_energy_next=f_after,
            grad_norm=float(np.linalg.norm(raw)),
        )

    def run(self, observations) -> RunReport:
        """Run the engine over a sequence of observations."""
        report = RunReport()
        for o_t in observations:
            report.steps.append(self.step(o_t))
        return report

    # -- introspection ------------------------------------------------------------

    @property
    def state(self) -> np.ndarray:
        return self.p.copy()

    def snapshot(self) -> dict:
        return {
            "t": self.t,
            "p": self.p.copy(),
            "memory_depth": len(self.resonance),
            "eps_ema": self.epsilon.ema,
            "feasible": self.projector.feasible(self.p, atol=1e-6),
        }
