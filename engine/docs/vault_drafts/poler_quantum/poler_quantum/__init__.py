"""POLER-Quantum: reference implementation of the POLER[n] attention core.

POLER[n] is a cognitive dynamics model that sits between a sensory layer
and a decision layer. It couples the physics of data (variational free
energy) with the logic of decisions (constraint projection), modulated by
a significance response (epsilon) and a memory resonance (R[n]):

    Omega(o_t) -> F(p, o; theta) -> Pi_Lambda -> eps -> R[n] -> S(p) -> p_{t+1}

This package provides:

* ``poler_quantum.core``    -- the classical POLER[n] engine (numpy)
* ``poler_quantum.quantum`` -- a quantum-sampled engine on top of Qiskit/Aer
* ``poler_quantum.benchmark``-- reproducible tracking benchmarks and plots

Version: 1.1.0
"""

__version__ = "1.1.0"

from .core.engine import PolerEngine, PolerConfig, StepResult  # noqa: F401
from .quantum.engine import QuantumPolerEngine, QuantumConfig  # noqa: F401

__all__ = [
    "PolerEngine",
    "PolerConfig",
    "StepResult",
    "QuantumPolerEngine",
    "QuantumConfig",
    "__version__",
]
