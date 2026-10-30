"""Classical POLER[n] core: perception, free energy, significance,
memory resonance, logic projection and the dynamics operator."""

from .perception import Perception  # noqa: F401
from .free_energy import FreeEnergy  # noqa: F401
from .epsilon import Epsilon  # noqa: F401
from .resonance import Resonance  # noqa: F401
from .projection import LogicProjector  # noqa: F401
from .dynamics import DynamicsOperator  # noqa: F401
from .purification import (mcweeny_purify, mcweeny_step,  # noqa: F401
                           idempotency_error, quantize_entries,
                           symmetric_noise, projector_from_constraints,
                           subspace_error, PurificationResult)
from .engine import PolerEngine, PolerConfig, StepResult  # noqa: F401

__all__ = [
    "Perception",
    "FreeEnergy",
    "Epsilon",
    "Resonance",
    "LogicProjector",
    "DynamicsOperator",
    "mcweeny_purify",
    "mcweeny_step",
    "idempotency_error",
    "quantize_entries",
    "symmetric_noise",
    "projector_from_constraints",
    "subspace_error",
    "PurificationResult",
    "PolerEngine",
    "PolerConfig",
    "StepResult",
]
