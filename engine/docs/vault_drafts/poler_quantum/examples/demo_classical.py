"""Minimal example: the classical POLER[n] cognitive cycle.

Run:  python examples/demo_classical.py
"""

import numpy as np

from poler_quantum import PolerConfig, PolerEngine

rng = np.random.default_rng(0)

# A calm, slowly drifting world that suddenly changes regime at t=40.
T = 80
world = (0.02 * np.cumsum(rng.standard_normal((T, 4)), axis=0)
         + 0.01 * rng.standard_normal((T, 4)))
world[40:] += 0.7  # regime change

engine = PolerEngine(PolerConfig(dim=4, eta=0.15, seed=0))
engine.reset()
report = engine.run(np.tanh(world))

for t in (0, 20, 38, 39, 40, 41, 60, 79):
    s = report.steps[t]
    print(f"t={s.t:>3}  eps={s.eps:7.4f}  eta_eff={s.eta_eff:6.3f}  "
          f"F={s.free_energy:7.4f}  p={np.round(s.p, 3)}")

print("\nThe epsilon column spikes on the very first step after the regime "
      "change (t=41: eps jumps from ~0.001 to 1.18) and the attention "
      "(eta_eff) follows -- the POLER 'emotional response' accelerating "
      "re-learning, then relaxing back once the new regime is absorbed.")
