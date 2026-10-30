"""Run the full benchmark and write figures + metrics into ./results.

Run:  python examples/run_benchmark.py
"""

from poler_quantum.benchmark.tasks import tracking_task
from poler_quantum.benchmark.runners import (
    run_all, run_multiseed, summarise, format_table,
    format_aggregate_table, make_plots, make_aggregate_plot,
    save_summary,
)
from poler_quantum.core.engine import PolerConfig
from poler_quantum.quantum.engine import QuantumConfig

# -- single-seed run with figures -------------------------------------------
task = tracking_task(T=300, dim=8, seed=7)
results = run_all(
    task,
    poler_cfg=PolerConfig(dim=8, seed=7),
    quantum_cfg=QuantumConfig(dim=8, seed=7, q_seed=42),
)
summary = summarise(task, results)
print(format_table(summary))
make_plots(task, results, summary, outdir="results")
save_summary(summary, task, outdir="results")

# -- statistically robust multi-seed run ------------------------------------
bundle = run_multiseed(
    n_seeds=5, base_seed=7,
    poler_cfg=PolerConfig(dim=8),
    quantum_cfg=QuantumConfig(dim=8),
    T=300, dim=8,
)
print()
print(format_aggregate_table(bundle["aggregate"]))
make_aggregate_plot(bundle["aggregate"], outdir="results")
