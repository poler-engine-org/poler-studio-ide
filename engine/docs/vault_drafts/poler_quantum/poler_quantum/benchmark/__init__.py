"""Reproducible benchmark tasks and runner comparison."""

from .tasks import tracking_task, TaskSpec  # noqa: F401
from .runners import (  # noqa: F401
    run_baseline_gd,
    run_baseline_ema,
    run_poler,
    run_quantum,
    run_all,
    run_multiseed,
    summarise,
    format_table,
    format_aggregate_table,
    make_plots,
    make_aggregate_plot,
    save_summary,
)

__all__ = [
    "tracking_task",
    "TaskSpec",
    "run_baseline_gd",
    "run_baseline_ema",
    "run_poler",
    "run_quantum",
    "run_all",
    "run_multiseed",
    "summarise",
    "format_table",
    "format_aggregate_table",
    "make_plots",
    "make_aggregate_plot",
    "save_summary",
]
