"""The quantum-sampled POLER engine.

Same cognitive cycle as :class:`poler_quantum.core.PolerEngine`, but the
update acquires an *exploration term sampled from the Born distribution*
of the POLER ansatz:

    dp = Pi_Lambda (-grad_F + gamma_res * grad_res)          # classical part
         + tau * S(p)                                        # free dynamics
         + sigma_q * (mean of q_shots Born samples)          # quantum part

The quantum proposal ``s in {-1,+1}^dim`` is drawn by preparing the ansatz
from the *current* state and measuring it. Because of the resonance
entanglement layer, the coordinates of ``s`` are correlated -- the agent
explores in a structured, history-shaped way rather than with white noise.

The quantum engine therefore demonstrates the core claim of
POLER-Quantum: **cognition as measurement**. Attention (the free-energy
gradient) proposes; the quantum resonance disposes.

v1.1.0 (RQ2) adds **dynamic continuous quantization** to the loop
(``docs/dynamic-quantization.md``):

* the state is *phase-quantized* before the ansatz is built, with the
  grid depth chosen per step from the significance energy (background ->
  trits / 1-2 bits, an eps-spike -> the full phase space) -- see
  :class:`poler_quantum.quantum.compression.AdaptiveDepth`;
* the logic projector ``Pi_Lambda`` can be run through simulated
  compressed storage (entry quantization) and *repaired in-loop* with
  McWeeny purification ``3P^2 - 2P^3``, restoring idempotency without
  retraining -- see :mod:`poler_quantum.core.purification`.

Both features are opt-in; the defaults reproduce v1.0.0 exactly.
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
from qiskit_aer import AerSimulator

from ..core.engine import PolerConfig, PolerEngine, StepResult
from ..core.purification import (idempotency_error, mcweeny_purify,
                                  quantize_entries)
from .ansatz import PolerAnsatz
from .compression import AdaptiveDepth, effective_bits, phase_quantize


@dataclass
class QuantumConfig(PolerConfig):
    """POLER engine configuration + quantum parameters.

    v1.1.0 (RQ2) -- dynamic continuous quantization knobs:

    * ``adaptive_depth``    -- compress the *proposal encoding* per step:
      the state is phase-quantized onto ``levels(eps_hat)`` grid points
      before the ansatz is built (background -> trits, spike -> full
      phase space). Off by default (exact v1.0.0 behaviour).
    * ``purify_projector``  -- simulate compressed storage of the logic
      projector: quantize ``Pi_Lambda`` entries to ``projector_levels``
      grid points, then repair idempotency with McWeeny
      (``3P^2 - 2P^3``, ``purify_iters`` iterations). Off by default.
    """

    mode: str = "A"              # ansatz mode: A | B | C
    entanglement: str = "cx"     # resonance gate: cx | cz
    sigma_q: float = 0.05        # strength of the quantum exploration term
    q_shots: int = 8             # Born samples averaged per step
    q_seed: int | None = 42      # simulator seed (reproducibility)
    # -- dynamic continuous quantization (RQ2, v1.1.0) --
    adaptive_depth: bool = False     # eps-driven phase compression of p
    levels_min: int = 3              # background grid: trits {+1, 0, -1}
    levels_max: int = 256            # spike grid: 8-bit phase space
    eps_lo: float = 0.0              # eps_hat at/below -> levels_min
    eps_hi: float = 2.0              # eps_hat at/above -> levels_max
    purify_projector: bool = False   # McWeeny-repair Pi_Lambda each step
    projector_levels: int = 16       # entry grid of the compressed Pi
    purify_iters: int = 2            # McWeeny iterations per repair

    def validate_quantum(self) -> None:
        if self.levels_min < 2:
            raise ValueError(f"levels_min must be >= 2, got {self.levels_min}")
        if self.levels_max < self.levels_min:
            raise ValueError("levels_max must be >= levels_min")
        if not self.eps_hi > self.eps_lo:
            raise ValueError("eps_hi must be > eps_lo")
        if self.projector_levels < 2:
            raise ValueError(
                f"projector_levels must be >= 2, got {self.projector_levels}")
        if self.purify_iters < 1:
            raise ValueError("purify_iters must be >= 1")


@dataclass
class QuantumStepResult(StepResult):
    """Step diagnostics + quantum-specific fields."""

    quantum_proposal: np.ndarray | None = None   # mean Born sample
    born_entropy: float = float("nan")           # attention entropy
    # -- v1.1.0 (RQ2): compression diagnostics --
    levels: int | None = None          # phase grid used this step (None = full)
    eff_bits: float | None = None      # log2(levels) (None = full precision)
    pi_idempotency_before: float = 0.0 # ||Q^2 - Q|| of the compressed Pi
    pi_idempotency_after: float = 0.0  # after McWeeny (0 if purification off)


class QuantumPolerEngine(PolerEngine):
    """POLER[n] engine with a quantum-sampled exploration term."""

    def __init__(self, config: QuantumConfig | None = None,
                 Jc: np.ndarray | None = None,
                 G: np.ndarray | None = None,
                 J: np.ndarray | None = None,
                 D: np.ndarray | None = None) -> None:
        if config is None:
            config = QuantumConfig()
        super().__init__(config=config, Jc=Jc, G=G, J=J, D=D)
        self.qcfg: QuantumConfig = config
        self.qcfg.validate_quantum()
        self.ansatz = PolerAnsatz(self.cfg.dim, mode=self.qcfg.mode,
                                  entanglement=self.qcfg.entanglement)
        self.simulator = AerSimulator()
        self._shot_counter = 0
        self.depth = AdaptiveDepth(levels_min=self.qcfg.levels_min,
                                   levels_max=self.qcfg.levels_max,
                                   eps_lo=self.qcfg.eps_lo,
                                   eps_hi=self.qcfg.eps_hi)

    # -- quantum proposal ----------------------------------------------------

    def quantum_proposal(self, p: np.ndarray | None = None) -> np.ndarray:
        """Sample ``q_shots`` Born proposals and average them.

        ``p`` defaults to the current state; the engine passes the
        (phase-quantized) compressed state when adaptive depth is on --
        exploration then runs at the significance-chosen resolution.
        """
        if p is None:
            p = self.p
        seed = None
        if self.qcfg.q_seed is not None:
            # deterministic per-step seed
            seed = self.qcfg.q_seed + self._shot_counter
            self._shot_counter += 1
        samples = self.ansatz.sample(
            self.simulator, p,
            gamma=self.cfg.gamma_res, kappa=self.cfg.kappa,
            shots=self.qcfg.q_shots, seed=seed)
        return samples.mean(axis=0)

    def attention_entropy(self) -> float:
        """Born entropy of the current ansatz (diagnostic)."""
        probs = self.ansatz.born_probabilities(
            self.p, gamma=self.cfg.gamma_res, kappa=self.cfg.kappa)
        return PolerAnsatz.born_entropy(probs)

    # -- the cognitive step ------------------------------------------------------

    def step(self, o_t: np.ndarray) -> QuantumStepResult:
        self.ensure_ready()
        omega = self.perception.omega(o_t)

        # significance (epsilon) -- attention modulation
        d_omega = (omega - self._last_omega) if self._last_omega is not None \
            else np.zeros_like(omega)
        eps, eps_hat = self.epsilon.update(d_omega)
        eta_eff = self.cfg.eta * float(np.exp(self.cfg.beta * np.tanh(eps_hat)))

        # classical cognition
        f_before = self.free_energy.value(self.p, omega)
        grad_F = self.free_energy.grad(self.p, omega)
        grad_res = self.resonance.gradient(self.p)
        sign = 1.0 if self.cfg.resonance_mode == "novelty" else -1.0

        # v1.1.0 (RQ2): dynamic continuous quantization of the encoding.
        # The phase grid size follows the significance: background runs on
        # trits / 1-2 bits, an eps-spike unfolds the full phase space.
        levels: int | None = None
        p_encoded = self.p
        if self.qcfg.adaptive_depth:
            levels = self.depth.levels_for(eps_hat)
            p_encoded = phase_quantize(self.p, levels)

        # quantum proposal (Born sampling of the encoded state)
        proposal = self.quantum_proposal(p_encoded)
        entropy = PolerAnsatz.born_entropy(self.ansatz.born_probabilities(
            p_encoded, gamma=self.cfg.gamma_res, kappa=self.cfg.kappa))

        # v1.1.0 (RQ2): compressed-storage projector + McWeeny repair.
        # Simulates Pi_Lambda stored on a low-bit grid: entry quantization
        # breaks idempotency, 3P^2 - 2P^3 restores it in 1-2 iterations.
        Pi = self.projector.matrix()
        pi_err_before = 0.0
        pi_err_after = 0.0
        if self.qcfg.purify_projector:
            Q = quantize_entries(Pi, self.qcfg.projector_levels)
            pi_err_before = idempotency_error(Q)
            repaired = mcweeny_purify(Q, max_iters=self.qcfg.purify_iters)
            Pi = repaired.matrix
            pi_err_after = idempotency_error(Pi)

        raw = Pi @ (-grad_F + sign * self.cfg.gamma_res * grad_res)
        drift = self.cfg.tau * self.dynamics.drift(self.p, Pi)
        quantum = self.qcfg.sigma_q * (Pi @ proposal)
        dp = raw + drift + quantum

        p_next = np.clip(self.p + eta_eff * dp,
                         -self.cfg.clip, self.cfg.clip)
        f_after = self.free_energy.value(p_next, omega)

        self.resonance.push(omega)
        self._last_omega = omega
        self.p = p_next
        self.t += 1

        return QuantumStepResult(
            t=self.t,
            p=p_next.copy(),
            omega=omega.copy(),
            eps=eps,
            eps_hat=float(eps_hat),
            eta_eff=float(eta_eff),
            free_energy=f_before,
            free_energy_next=f_after,
            grad_norm=float(np.linalg.norm(raw)),
            quantum_proposal=proposal.copy(),
            born_entropy=entropy,
            levels=levels,
            eff_bits=None if levels is None else effective_bits(levels),
            pi_idempotency_before=float(pi_err_before),
            pi_idempotency_after=float(pi_err_after),
        )
