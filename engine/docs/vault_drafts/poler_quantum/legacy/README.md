# legacy/

The original POLER-Quantum scripts (2024–2025), preserved verbatim for
history. These are the notebooks/experiments the project grew from:

* `POLER_sim.py` — Mode A: the first 2-qubit quantum step (Ry + CX).
* `POLER_modeB.py` — Mode B: + Rz(γ) stabilisation layer.
* `POLER_modeC.py` — Mode C: adaptive stabilisation `γ·e^{−|sin(κπ)|}`.
* `POLER_Psi_v3.py` — the first classical Ψ-field prototype (Perception /
  LogicProjector / Resonance / PsiField classes).
* `qiskit_ansatz.py` — statevector ansatz demo.
* `save_results.py` — GitHub Actions results helper.
* `python_prototypes/` — `poler_v6.py` (reference cycle), stubs.
* `poler_toolkit/` — early text-analysis sketches on POLER[Ψ].

Everything here is superseded by the `poler_quantum/` package, which
implements the same ideas completely and with tests. The ansatz modes
A/B/C live on in `poler_quantum/quantum/ansatz.py`.
