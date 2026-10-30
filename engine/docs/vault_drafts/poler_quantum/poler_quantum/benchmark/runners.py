"""Benchmark runners: POLER vs classical baselines vs quantum POLER.

All agents perceive the world identically (``omega = tanh(observation)``)
and are evaluated against the *latent* target. The baselines are:

* **GD** -- plain gradient descent on the prediction error
  ``p += eta * (omega - p)``. No memory, no significance response.
* **EMA** -- exponential moving average of the perception. Memory of the
  *observations*, but no model, no free energy, no attention.
* **POLER** -- the full cognitive cycle.
* **POLER-Quantum** -- the cycle with a Born-sampled exploration term.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np

from ..core.engine import PolerConfig, PolerEngine
from ..quantum.engine import QuantumConfig, QuantumPolerEngine
from ..metrics import rmse, recovery_steps, path_smoothness, mean_free_energy
from .tasks import TaskInstance

# consistent colours across all figures
COLORS = {
    "target": "black",
    "GD": "#7f7f7f",
    "EMA": "#1f77b4",
    "POLER": "#d62728",
    "POLER-Quantum": "#9467bd",
}


# ---------------------------------------------------------------------------
# baselines
# ---------------------------------------------------------------------------

def run_baseline_gd(task: TaskInstance, eta: float = 0.10) -> dict:
    """Plain gradient descent on the prediction error."""
    obs = np.tanh(task.observations)
    T, dim = obs.shape
    p = np.zeros(dim)
    traj = np.empty((T, dim))
    for t in range(T):
        p = p + eta * (obs[t] - p)
        traj[t] = p
    return {"name": "GD", "traj": traj, "perception": obs}


def run_baseline_ema(task: TaskInstance, alpha: float = 0.10) -> dict:
    """Exponential moving average of the perception."""
    obs = np.tanh(task.observations)
    T, dim = obs.shape
    p = np.zeros(dim)
    traj = np.empty((T, dim))
    for t in range(T):
        p = alpha * obs[t] + (1.0 - alpha) * p
        traj[t] = p
    return {"name": "EMA", "traj": traj, "perception": obs}


# ---------------------------------------------------------------------------
# POLER
# ---------------------------------------------------------------------------

def run_poler(task: TaskInstance, config: PolerConfig | None = None,
              p0: np.ndarray | None = None) -> dict:
    """The classical POLER[n] engine."""
    obs = np.tanh(task.observations)
    cfg = config or PolerConfig(dim=obs.shape[1])
    cfg.dim = obs.shape[1]
    engine = PolerEngine(cfg)
    engine.reset(p0)
    report = engine.run(obs)
    traj = report.states
    return {
        "name": "POLER",
        "traj": traj,
        "perception": obs,
        "eps": report.eps,
        "eta_eff": report.eta_eff,
        "free_energy": report.free_energy,
    }


def run_quantum(task: TaskInstance, config: QuantumConfig | None = None,
                p0: np.ndarray | None = None) -> dict:
    """The quantum-sampled POLER engine."""
    obs = np.tanh(task.observations)
    cfg = config or QuantumConfig(dim=obs.shape[1])
    cfg.dim = obs.shape[1]
    engine = QuantumPolerEngine(cfg)
    engine.reset(p0)
    report = engine.run(obs)
    traj = report.states
    return {
        "name": "POLER-Quantum",
        "traj": traj,
        "perception": obs,
        "eps": report.eps,
        "eta_eff": report.eta_eff,
        "free_energy": report.free_energy,
        "born_entropy": np.array([s.born_entropy for s in report.steps]),
    }


# ---------------------------------------------------------------------------
# orchestration
# ---------------------------------------------------------------------------

def run_all(task: TaskInstance, poler_cfg: PolerConfig | None = None,
            quantum_cfg: QuantumConfig | None = None,
            ema_alpha: float = 0.3) -> dict[str, dict]:
    """Run every runner on the same task instance.

    Note: with ``alpha == eta`` the EMA update ``a*o + (1-a)*p`` is
    algebraically identical to the GD update ``p + a*(o - p)`` -- so EMA
    gets its own, conventionally tuned smoothing factor (0.3) instead of
    silently duplicating the GD baseline.
    """
    eta = (poler_cfg.eta if poler_cfg else QuantumConfig().eta)
    results = {
        "GD": run_baseline_gd(task, eta=eta),
        "EMA": run_baseline_ema(task, alpha=ema_alpha),
        "POLER": run_poler(task, poler_cfg),
        "POLER-Quantum": run_quantum(task, quantum_cfg),
    }
    return results


def _posthoc_free_energy(traj: np.ndarray, obs: np.ndarray, lam: float = 0.01) -> np.ndarray:
    """Free energy of an arbitrary trajectory (fair cross-method metric)."""
    diff = traj - obs
    return np.einsum("ti,ti->t", diff, diff) + 0.5 * lam * np.einsum("ti,ti->t", traj, traj)


def summarise(task: TaskInstance, results: dict[str, dict],
              warmup: int = 20) -> dict[str, dict]:
    """Compute all metrics for every runner."""
    target = task.target
    summary: dict[str, dict] = {}
    for name, res in results.items():
        traj = res["traj"]
        obs = res["perception"]
        rec = [recovery_steps(traj, target, int(s))
               for s in task.spec.switches]
        summary[name] = {
            "rmse": rmse(traj, target, warmup),
            "recovery_steps": rec,
            "recovery_mean": float(np.mean(rec)),
            "smoothness": path_smoothness(traj, warmup),
            "free_energy": mean_free_energy(_posthoc_free_energy(traj, obs), warmup),
        }
    return summary


def format_table(summary: dict[str, dict]) -> str:
    """Human-readable metric table."""
    header = (f"{'method':<16}{'RMSE':>10}{'recovery':>11}"
              f"{'smoothness':>12}{'free energy':>13}")
    lines = [header, "-" * len(header)]
    for name, m in summary.items():
        lines.append(
            f"{name:<16}{m['rmse']:>10.4f}{m['recovery_mean']:>11.1f}"
            f"{m['smoothness']:>12.4f}{m['free_energy']:>13.4f}")
    return "\n".join(lines)


def make_plots(task: TaskInstance, results: dict[str, dict],
               summary: dict[str, dict], outdir: str | Path = "results",
               dim_plot: int = 0) -> list[Path]:
    """Render the benchmark figures (PNG) into ``outdir``.

    Note: figure text is English because the repository is public and the
    benchmark is aimed at an international audience.
    """
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    outdir = Path(outdir)
    outdir.mkdir(parents=True, exist_ok=True)
    target = task.target
    switches = list(task.spec.switches)
    written: list[Path] = []

    # -- 1. tracking trajectories ------------------------------------------------
    fig, axes = plt.subplots(2, 1, figsize=(11, 7), sharex=True,
                             constrained_layout=True)
    ax = axes[0]
    ax.plot(target[:, dim_plot], color=COLORS["target"], lw=1.6,
            label="latent target", zorder=5)
    for name, res in results.items():
        ax.plot(res["traj"][:, dim_plot], color=COLORS.get(name, None),
                lw=1.0, alpha=0.85, label=name)
    for s in switches:
        ax.axvline(s, color="gray", ls=":", lw=0.8)
    ax.set_ylabel(f"state dim {dim_plot}")
    ax.set_title("POLER-Quantum benchmark: non-stationary tracking")
    ax.legend(loc="upper left", bbox_to_anchor=(1.01, 1.0), fontsize=9)

    ax = axes[1]
    for name, res in results.items():
        err = np.linalg.norm(res["traj"] - target, axis=-1)
        ax.plot(err, color=COLORS.get(name, None), lw=1.0, alpha=0.9, label=name)
    for s in switches:
        ax.axvline(s, color="gray", ls=":", lw=0.8)
    ax.set_yscale("log")
    ax.set_xlabel("time step")
    ax.set_ylabel("||p - target||")
    ax.legend(loc="upper left", bbox_to_anchor=(1.01, 1.0), fontsize=9)
    path = outdir / "tracking.png"
    fig.savefig(path, dpi=150)
    plt.close(fig)
    written.append(path)

    # -- 2. metric bars -------------------------------------------------------------
    fig, axes = plt.subplots(1, 3, figsize=(12, 3.6), constrained_layout=True)
    names = list(summary.keys())
    x = np.arange(len(names))
    for ax, key, label in zip(
            axes,
            ["rmse", "recovery_mean", "smoothness"],
            ["RMSE (lower is better)",
             "mean recovery after switch (steps, lower is better)",
             "path smoothness: mean step size (lower is better)"]):
        vals = [summary[n][key] for n in names]
        ax.bar(x, vals, color=[COLORS.get(n, "#333") for n in names])
        ax.set_xticks(x, names, rotation=20, fontsize=8)
        ax.set_title(label, fontsize=9)
        for i, v in enumerate(vals):
            ax.text(i, v, f"{v:.3f}", ha="center", va="bottom", fontsize=8)
    path = outdir / "metrics.png"
    fig.savefig(path, dpi=150)
    plt.close(fig)
    written.append(path)

    # -- 3. epsilon / attention ---------------------------------------------------
    poler = results.get("POLER")
    if poler is not None and "eps" in poler:
        fig, ax = plt.subplots(figsize=(11, 3.4), constrained_layout=True)
        ax.plot(poler["eps"], color="#d62728", lw=1.0, label=r"$\epsilon$ (significance energy)")
        ax2 = ax.twinx()
        ax2.plot(poler["eta_eff"], color="#1f77b4", lw=1.0, alpha=0.8,
                 label=r"$\eta_{eff}$ (attention)")
        for s in switches:
            ax.axvline(s, color="gray", ls=":", lw=0.8)
        ax.set_xlabel("time step")
        ax.set_ylabel(r"$\epsilon$", color="#d62728")
        ax2.set_ylabel(r"$\eta_{eff}$", color="#1f77b4")
        ax.set_title("POLER emotional response: significance spikes at regime switches")
        h1, l1 = ax.get_legend_handles_labels()
        h2, l2 = ax2.get_legend_handles_labels()
        ax.legend(h1 + h2, l1 + l2, loc="upper left", bbox_to_anchor=(1.08, 1.0), fontsize=9)
        path = outdir / "epsilon.png"
        fig.savefig(path, dpi=150)
        plt.close(fig)
        written.append(path)

    # -- 4. quantum diagnostics -----------------------------------------------------
    q = results.get("POLER-Quantum")
    if q is not None and "born_entropy" in q:
        fig, ax = plt.subplots(figsize=(11, 3.4), constrained_layout=True)
        ax.plot(q["born_entropy"], color="#9467bd", lw=1.0)
        for s in switches:
            ax.axvline(s, color="gray", ls=":", lw=0.8)
        ax.set_xlabel("time step")
        ax.set_ylabel("Born entropy [bits]")
        ax.set_title("POLER-Quantum: attention entropy of the ansatz")
        path = outdir / "quantum_entropy.png"
        fig.savefig(path, dpi=150)
        plt.close(fig)
        written.append(path)

    return written


def run_multiseed(n_seeds: int = 5, base_seed: int = 7,
                  poler_cfg: PolerConfig | None = None,
                  quantum_cfg: QuantumConfig | None = None,
                  **task_params) -> dict:
    """Run the full benchmark over several task seeds and aggregate.

    Returns a dict with:
      * ``aggregate``: {method: {metric: {"mean", "std"}}}
      * ``per_seed``: list of single-seed summaries
      * ``representative``: (task, results) of the *median* seed by
        POLER RMSE -- a fair, non-cherry-picked showcase for the plots.
    """
    from .tasks import tracking_task

    per_seed = []
    tasks_results = []
    for i in range(n_seeds):
        task = tracking_task(seed=base_seed + i, **task_params)
        results = run_all(task, poler_cfg=poler_cfg, quantum_cfg=quantum_cfg)
        per_seed.append(summarise(task, results))
        tasks_results.append((task, results))

    methods = list(per_seed[0].keys())
    metrics_keys = ("rmse", "recovery_mean", "smoothness", "free_energy")
    aggregate = {}
    for m in methods:
        aggregate[m] = {}
        for k in metrics_keys:
            vals = np.array([s[m][k] for s in per_seed], dtype=float)
            aggregate[m][k] = {"mean": float(vals.mean()),
                               "std": float(vals.std(ddof=1)) if n_seeds > 1 else 0.0}

    # representative seed: the one where POLER's RMSE is the median
    poler_rmse = [s["POLER"]["rmse"] for s in per_seed]
    order = np.argsort(poler_rmse)
    rep_idx = int(order[len(order) // 2])
    return {
        "aggregate": aggregate,
        "per_seed": per_seed,
        "representative": tasks_results[rep_idx],
        "seeds": [base_seed + i for i in range(n_seeds)],
    }


def format_aggregate_table(aggregate: dict) -> str:
    """Human-readable mean +/- std table."""
    header = (f"{'method':<16}{'RMSE':>16}{'recovery':>16}"
              f"{'smoothness':>16}{'free energy':>16}")
    lines = [header, "-" * len(header)]

    def cell(k):
        return f"{k['mean']:.4f}±{k['std']:.4f}"

    for name, m in aggregate.items():
        lines.append(
            f"{name:<16}{cell(m['rmse']):>16}{cell(m['recovery_mean']):>16}"
            f"{cell(m['smoothness']):>16}{cell(m['free_energy']):>16}")
    return "\n".join(lines)


def make_aggregate_plot(aggregate: dict, outdir: str | Path = "results") -> Path:
    """Bar chart of the aggregated metrics with std error bars."""
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    outdir = Path(outdir)
    outdir.mkdir(parents=True, exist_ok=True)
    names = list(aggregate.keys())
    x = np.arange(len(names))
    fig, axes = plt.subplots(1, 3, figsize=(12, 3.8), constrained_layout=True)
    for ax, key, label in zip(
            axes,
            ["rmse", "recovery_mean", "smoothness"],
            ["RMSE vs latent target (lower is better)",
             "mean recovery after switch (steps, lower is better)",
             "path smoothness: mean step size (lower is better)"]):
        means = [aggregate[n][key]["mean"] for n in names]
        stds = [aggregate[n][key]["std"] for n in names]
        ax.bar(x, means, yerr=stds, capsize=3,
               color=[COLORS.get(n, "#333") for n in names])
        ax.set_xticks(x, names, rotation=20, fontsize=8)
        ax.set_title(label, fontsize=9)
        for i, (mu, sd) in enumerate(zip(means, stds)):
            ax.text(i, mu + sd, f"{mu:.3f}", ha="center", va="bottom", fontsize=8)
    path = outdir / "metrics_multiseed.png"
    fig.savefig(path, dpi=150)
    plt.close(fig)
    return path


def save_summary(summary: dict[str, dict], task: TaskInstance,
                 outdir: str | Path = "results") -> Path:
    """Persist the metrics as JSON next to the figures."""
    outdir = Path(outdir)
    outdir.mkdir(parents=True, exist_ok=True)
    payload = {
        "task": {
            "T": task.spec.T,
            "dim": task.spec.dim,
            "switches": list(task.spec.switches),
            "seed": task.spec.seed,
            "noise": task.spec.noise,
            "jump": task.spec.jump,
        },
        "metrics": summary,
    }
    path = outdir / "metrics.json"
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    return path
