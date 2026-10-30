"""The POLER ansatz: state -> circuit.

Encoding of the cognitive state into qubits:

* **Perception layer** -- every coordinate ``p_i in [-1, 1]`` of the state
  becomes a Ry rotation on qubit ``i`` with angle
  ``theta_i = arccos(clip(p_i, -1, 1))``. The probability of measuring
  ``|1>`` on qubit i is then ``sin^2(theta_i / 2) = (1 - p_i) / 2`` --
  the state literally parameterises the Born distribution.

* **Resonance layer** -- a chain of entangling gates (CX by default, CZ in
  the "B" mode) between neighbouring qubits mirrors the memory resonance
  R[n]: the coordinates stop being independent and the measurement
  distribution develops correlations, exactly like the episodic memory
  couples past states.

* **Stabilisation layer** -- an Rz rotation on every qubit with the
  adaptive phase ``phi = gamma * exp(-|sin(kappa * pi)|)`` (mode C of the
  original notebooks): a phase-only nudge that leaves measurement
  probabilities untouched in the computational basis but shifts the
  interference structure -- the "inner life" of the state.
"""

from __future__ import annotations

import numpy as np
from qiskit import QuantumCircuit, transpile
from qiskit.quantum_info import Statevector

#: Modes:
#:   "A" -- perception + resonance entanglement (CX chain)
#:   "B" -- mode A + Rz(gamma) stabilisation layer
#:   "C" -- mode A + Rz(adaptive gamma) layer (original POLER_modeC)
MODES = ("A", "B", "C")


def _parse_counts(counts: dict[str, int], num_qubits: int) -> np.ndarray:
    """Convert Qiskit counts to an array of per-shot sign vectors.

    Qiskit bitstrings are big-endian (leftmost char = highest classical
    bit = highest qubit index). Bit ``b_i`` of qubit i maps to the sign
    ``s_i = 1 - 2*b_i`` so that ``p_i = +1`` (state at +1) favours ``|0>``.
    """
    shots = sum(counts.values())
    samples = np.empty((shots, num_qubits), dtype=float)
    row = 0
    for bitstring, count in counts.items():
        bits = bitstring.replace(" ", "")
        # little-endian per-qubit order: qubit i is bits[num_qubits - 1 - i]
        signs = np.array([1.0 - 2.0 * float(bits[num_qubits - 1 - i])
                          for i in range(num_qubits)])
        samples[row:row + count] = signs
        row += count
    return samples


class PolerAnsatz:
    """Builder of the POLER quantum circuit."""

    def __init__(self, num_qubits: int, mode: str = "A",
                 entanglement: str = "cx") -> None:
        if num_qubits < 1:
            raise ValueError("num_qubits must be >= 1")
        if mode not in MODES:
            raise ValueError(f"mode must be one of {MODES}, got {mode!r}")
        if entanglement not in ("cx", "cz"):
            raise ValueError("entanglement must be 'cx' or 'cz'")
        self.num_qubits = num_qubits
        self.mode = mode
        self.entanglement = entanglement

    # -- circuit construction ---------------------------------------------------

    def angles(self, p: np.ndarray) -> np.ndarray:
        """State coordinates -> Ry angles (perception layer)."""
        p = np.clip(np.asarray(p, dtype=float), -1.0, 1.0)
        if p.shape != (self.num_qubits,):
            raise ValueError(
                f"p must have shape ({self.num_qubits},), got {p.shape}")
        return np.arccos(p)

    def stabilisation_phase(self, gamma: float, kappa: float) -> float:
        """Adaptive stabilisation phase (mode C)."""
        return float(gamma * np.exp(-abs(np.sin(kappa * np.pi))))

    def build(self, p: np.ndarray, gamma: float = 0.5,
              kappa: float = 0.8) -> QuantumCircuit:
        """Assemble the POLER circuit for state ``p``."""
        theta = self.angles(p)
        qc = QuantumCircuit(self.num_qubits)

        # perception layer
        for i in range(self.num_qubits):
            qc.ry(theta[i], i)

        # resonance layer -- entangle neighbours
        entangler = qc.cx if self.entanglement == "cx" else qc.cz
        for i in range(self.num_qubits - 1):
            entangler(i, i + 1)

        # stabilisation layer (modes B and C)
        if self.mode == "B":
            for i in range(self.num_qubits):
                qc.rz(gamma, i)
        elif self.mode == "C":
            phi = self.stabilisation_phase(gamma, kappa)
            for i in range(self.num_qubits):
                qc.rz(phi, i)

        return qc

    # -- simulation ---------------------------------------------------------------

    def statevector(self, p: np.ndarray, gamma: float = 0.5,
                    kappa: float = 0.8) -> Statevector:
        """Exact statevector of the ansatz (no measurement)."""
        return Statevector.from_instruction(self.build(p, gamma, kappa))

    def sample(self, simulator, p: np.ndarray, gamma: float = 0.5,
               kappa: float = 0.8, shots: int = 64,
               seed: int | None = None) -> np.ndarray:
        """Born-sample the ansatz.

        Returns an array of shape ``(shots, num_qubits)`` with entries in
        {-1, +1}: one cognitive "proposal" per shot.
        """
        qc = self.build(p, gamma, kappa)
        qc.measure_all()
        compiled = transpile(qc, simulator)
        if seed is not None:
            result = simulator.run(compiled, shots=shots,
                                   seed_simulator=seed).result()
        else:
            result = simulator.run(compiled, shots=shots).result()
        counts = result.get_counts()
        return _parse_counts(counts, self.num_qubits)

    def born_probabilities(self, p: np.ndarray, gamma: float = 0.5,
                           kappa: float = 0.8) -> np.ndarray:
        """Exact Born probabilities over the computational basis."""
        return np.abs(self.statevector(p, gamma, kappa).data) ** 2

    # -- diagnostics ---------------------------------------------------------------

    @staticmethod
    def born_entropy(probs: np.ndarray) -> float:
        """Shannon entropy of the Born distribution (attention entropy)."""
        probs = np.asarray(probs, dtype=float)
        probs = probs[probs > 1e-15]
        return float(-(probs * np.log2(probs)).sum())
