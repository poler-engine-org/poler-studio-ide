"""Minimal example: the quantum-sampled POLER engine.

Run:  python examples/demo_quantum.py
"""

import numpy as np

from poler_quantum import QuantumConfig, QuantumPolerEngine
from poler_quantum.quantum.ansatz import PolerAnsatz

rng = np.random.default_rng(1)
T = 60
world = 0.2 * rng.standard_normal((T, 6))

cfg = QuantumConfig(dim=6, seed=0, q_seed=42, sigma_q=0.05, q_shots=8)
engine = QuantumPolerEngine(cfg)
engine.reset()
report = engine.run(np.tanh(world))

entropies = np.array([s.born_entropy for s in report.steps])
print(f"ran {T} cognitive steps with Born-sampled exploration")
print(f"final state            : {np.round(engine.state, 3)}")
print(f"Born entropy (bits)    : mean={entropies.mean():.3f} "
      f"min={entropies.min():.3f} max={entropies.max():.3f} "
      f"(uniform would be {np.log2(64):.2f})")

# Inspect the ansatz of the final state.
ansatz = PolerAnsatz(6, mode=cfg.mode)
probs = ansatz.born_probabilities(engine.state)
top = np.argsort(probs)[::-1][:3]
print("top Born outcomes of the final ansatz:")
for idx in top:
    print(f"  |{format(idx, '06b')}>  p = {probs[idx]:.4f}")
