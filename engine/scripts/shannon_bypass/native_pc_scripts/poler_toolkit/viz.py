"""
poler_toolkit.viz — стандартные графики (matplotlib).

Все функции принимают данные + выходной путь, пишут PNG.
Единый стиль: шрифты, цвета, DPI.

Графики:
  - chapter_matrix         heatmap глав (cosine similarity)
  - character_map          карта персонажа по строкам/символам
  - transition_smoothness  cosine соседних абзацев (smooth vs sharp)
  - pca_scatter            PCA-проекция абзацев
  - entropy_per_chapter    Shannon entropy по главам
  - cluster_sizes          размеры POLER-кластеров
  - themes_per_chapter     тематические профили (stacked bar)
  - comparison_dashboard   две книги бок-о-бок (6 метрик)
"""

from __future__ import annotations

import re
import logging
from pathlib import Path
from typing import Optional, List, Dict, Any, Sequence, Tuple

import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.font_manager as fm

from .errors import VisualizationError

log = logging.getLogger("poler_toolkit.viz")


# ============================================================
# Стиль
# ============================================================
def _setup_fonts():
    for f in [
        "/usr/share/fonts/truetype/chinese/NotoSansSC-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    ]:
        if Path(f).exists():
            try:
                fm.fontManager.addfont(f)
            except Exception:
                pass
    plt.rcParams["font.sans-serif"] = ["DejaVu Sans", "Noto Sans SC"]
    plt.rcParams["axes.unicode_minus"] = False


_setup_fonts()

# Палитра
COLOR_PRIMARY = "#2c3e50"
COLOR_ACCENT = "#e74c3c"
COLOR_SECONDARY = "#3498db"
COLOR_OK = "#27ae60"
COLOR_WARN = "#f39c12"
COLOR_BAD = "#c0392b"
COLOR_NEUTRAL = "#95a5a6"

DPI = 130


# ============================================================
# Утилиты
# ============================================================
def _short_label(title: str) -> str:
    """Превращает 'Глава 12 (Робоча назва): Уламок' в '12'."""
    m = re.search(r"\d+", title)
    return m.group(0) if m else title[:6]


def _safe_savefig(fig, path: Path):
    try:
        fig.savefig(path, dpi=DPI)
        log.info(f"Saved: {path}")
    except Exception as e:
        raise VisualizationError(f"savefig failed: {e}")
    finally:
        plt.close(fig)


# ============================================================
# Графики
# ============================================================
def chapter_matrix(
    sim_matrix: np.ndarray,
    chapter_titles: List[str],
    out_path: Path,
    title: str = "Chapter similarity matrix",
    cmap: str = "viridis",
):
    """Heatmap cosine similarity глав."""
    n = len(chapter_titles)
    fig, ax = plt.subplots(figsize=(max(8, n * 0.4), max(7, n * 0.4)),
                           constrained_layout=True)
    im = ax.imshow(sim_matrix, cmap=cmap, vmin=0, vmax=1)
    ax.set_title(title, fontsize=13, pad=10)
    labels = [_short_label(t) for t in chapter_titles]
    ax.set_xticks(range(n))
    ax.set_yticks(range(n))
    ax.set_xticklabels(labels, rotation=90, fontsize=max(5, 9 - n // 20))
    ax.set_yticklabels(labels, fontsize=max(5, 9 - n // 20))
    ax.set_xlabel("Глава")
    ax.set_ylabel("Глава")
    fig.colorbar(im, ax=ax, fraction=0.046, pad=0.02, label="cosine")
    _safe_savefig(fig, out_path)


def character_map(
    characters: List[str],
    positions: Dict[str, List[int]],
    counts: Dict[str, int],
    out_path: Path,
    title: str = "Character map",
):
    """Карта персонажей по строкам/позициям."""
    fig, ax = plt.subplots(figsize=(13, 6), constrained_layout=True)
    colors = plt.cm.tab10(np.linspace(0, 1, len(characters)))
    for i, ch in enumerate(characters):
        pos = positions.get(ch, [])
        if not pos:
            continue
        ax.scatter(pos, [i] * len(pos), color=colors[i], s=18, alpha=0.65,
                   edgecolors="none")
    ax.set_yticks(range(len(characters)))
    ax.set_yticklabels([f"{ch} ({counts.get(ch, 0)})" for ch in characters],
                       fontsize=9)
    ax.set_xlabel("Позиция в файле (строка)")
    ax.set_title(title, fontsize=13, pad=10)
    ax.grid(alpha=0.25, axis="x")
    ax.legend(
        [f"{ch} ({counts.get(ch, 0)})" for ch in characters],
        loc="upper right", fontsize=7, ncol=3,
    )
    _safe_savefig(fig, out_path)


def transition_smoothness(
    sims: np.ndarray,
    out_path: Path,
    window: int = 5,
    title: str = "Transition smoothness",
    chapter_ticks: Optional[List[Tuple[int, str]]] = None,
):
    """Cosine соседних абзацев + згладжування."""
    fig, ax = plt.subplots(figsize=(13, 5), constrained_layout=True)
    x = np.arange(len(sims))
    ax.plot(x, sims, color=COLOR_NEUTRAL, alpha=0.35, linewidth=0.6,
            label="cos(susidнiй абзац)")
    if len(sims) >= window:
        smoothed = np.convolve(sims, np.ones(window) / window, mode="valid")
        ax.plot(np.arange(window - 1, window - 1 + len(smoothed)),
                smoothed, color=COLOR_ACCENT, linewidth=1.6,
                label=f"згладжено (вiкно {window})")
    ax.axhline(0.10, color=COLOR_OK, linestyle="--", alpha=0.7, linewidth=1,
               label="порiг «рвивка» 0.10")
    ax.axhline(0.20, color=COLOR_SECONDARY, linestyle="--", alpha=0.7,
               linewidth=1, label="порiг «спорiдненостi» 0.20")
    if chapter_ticks:
        for pos, lbl in chapter_ticks:
            ax.axvline(pos, color="#bbb", alpha=0.2, linewidth=0.5)
    ax.set_xlabel("Номер абзацу")
    ax.set_ylabel("cosine similarity")
    ax.set_title(title, fontsize=13, pad=10)
    ax.legend(loc="upper right", fontsize=8)
    ax.grid(alpha=0.25)
    _safe_savefig(fig, out_path)


def pca_scatter(
    coords: np.ndarray,
    cluster_labels: Optional[np.ndarray],
    chapter_idx: Optional[List[int]] = None,
    chapter_centroids: Optional[np.ndarray] = None,
    explained_variance: Optional[Tuple[float, float]] = None,
    out_path: Path = None,
    title: str = "PCA projection",
):
    """PCA-проекция абзацев."""
    fig, ax = plt.subplots(figsize=(11, 8), constrained_layout=True)
    if cluster_labels is not None:
        scatter = ax.scatter(coords[:, 0], coords[:, 1], c=cluster_labels,
                            cmap="tab10", s=10, alpha=0.55, edgecolors="none")
    elif chapter_idx is not None:
        scatter = ax.scatter(coords[:, 0], coords[:, 1], c=chapter_idx,
                            cmap="tab20", s=10, alpha=0.55, edgecolors="none")
    else:
        scatter = ax.scatter(coords[:, 0], coords[:, 1], s=10, alpha=0.55,
                            edgecolors="none", color=COLOR_PRIMARY)
    if chapter_centroids is not None and chapter_idx is not None:
        for ci in set(chapter_idx):
            mask = [i for i, c in enumerate(chapter_idx) if c == ci]
            if not mask:
                continue
            cx = coords[mask, 0].mean()
            cy = coords[mask, 1].mean()
            ax.scatter(cx, cy, marker="x", c="black", s=60, linewidths=1.2)
            ax.text(cx + 0.01, cy + 0.01, str(ci), fontsize=7, color="black",
                    alpha=0.85)
    if explained_variance:
        ax.set_xlabel(f"PC1 ({explained_variance[0]:.1%})")
        ax.set_ylabel(f"PC2 ({explained_variance[1]:.1%})")
    else:
        ax.set_xlabel("PC1")
        ax.set_ylabel("PC2")
    ax.set_title(title, fontsize=13, pad=10)
    ax.grid(alpha=0.25)
    _safe_savefig(fig, out_path)


def entropy_per_chapter(
    entropies: np.ndarray,
    chapter_titles: List[str],
    out_path: Path,
    title: str = "Shannon entropy per chapter",
    reference_lines: Optional[Dict[str, float]] = None,
):
    """Энтропия по главам."""
    fig, ax = plt.subplots(figsize=(13, 5), constrained_layout=True)
    xs = np.arange(len(entropies))
    ax.bar(xs, entropies, color=COLOR_PRIMARY, alpha=0.85)
    m = entropies.mean()
    s = entropies.std()
    ax.axhline(m, color=COLOR_ACCENT, linestyle="--", linewidth=1.5,
               label=f"середн║={m:.2f}")
    if s > 0:
        ax.axhline(m - s, color=COLOR_WARN, linestyle=":", linewidth=1,
                   label=f"−1σ={m-s:.2f}")
        ax.axhline(m + s, color=COLOR_OK, linestyle=":", linewidth=1,
                   label=f"+1σ={m+s:.2f}")
    if reference_lines:
        for lbl, val in reference_lines.items():
            ax.axhline(val, color=COLOR_SECONDARY, linestyle="-.",
                       linewidth=1, alpha=0.6, label=lbl)
    labels = [_short_label(t) for t in chapter_titles]
    ax.set_xticks(xs)
    ax.set_xticklabels(labels, rotation=90, fontsize=7)
    ax.set_xlabel("Глава")
    ax.set_ylabel("Entropy (bits/token)")
    ax.set_title(title, fontsize=13, pad=10)
    ax.legend(loc="lower right", fontsize=8)
    ax.grid(alpha=0.25, axis="y")
    _safe_savefig(fig, out_path)


def cluster_sizes_bar(
    cluster_sizes: List[int],
    out_path: Path,
    title: str = "POLER cluster sizes",
):
    """Размеры кластеров POLER."""
    if not cluster_sizes:
        cluster_sizes = [0]
    fig, ax = plt.subplots(figsize=(11, 5), constrained_layout=True)
    xs = np.arange(1, len(cluster_sizes) + 1)
    ax.bar(xs, cluster_sizes, color="#8e44ad", alpha=0.85)
    if len(cluster_sizes) > 0:
        m = float(np.mean(cluster_sizes))
        ax.axhline(m, color=COLOR_ACCENT, linestyle="--",
                   label=f"середнiй={m:.1f}")
    ax.set_xlabel("Номер кластеру")
    ax.set_ylabel("Кiлькiсть фрагментiв")
    ax.set_title(title, fontsize=13, pad=10)
    ax.legend(fontsize=9)
    ax.grid(alpha=0.25, axis="y")
    _safe_savefig(fig, out_path)


def themes_per_chapter(
    ch_marker_matrix: np.ndarray,
    markers: List[str],
    chapter_titles: List[str],
    out_path: Path,
    top_n: int = 15,
    title: str = "Themes per chapter",
):
    """Тематические профили глав (stacked bar)."""
    # Топ-N самых частых маркеров
    top_idx = np.argsort(-ch_marker_matrix.sum(axis=0))[:top_n]
    fig, ax = plt.subplots(figsize=(13, 6), constrained_layout=True)
    bottom = np.zeros(ch_marker_matrix.shape[0])
    colors = plt.cm.viridis(np.linspace(0, 1, len(top_idx)))
    # Нормируем на длину главы
    row_sums = ch_marker_matrix.sum(axis=1, keepdims=True) + 1
    norm = ch_marker_matrix / row_sums
    for k, mi in enumerate(top_idx):
        ax.bar(range(ch_marker_matrix.shape[0]), norm[:, mi], bottom=bottom,
               color=colors[k], label=markers[mi])
        bottom += norm[:, mi]
    labels = [_short_label(t) for t in chapter_titles]
    ax.set_xticks(range(len(labels)))
    ax.set_xticklabels(labels, rotation=90, fontsize=7)
    ax.set_xlabel("Глава")
    ax.set_ylabel("Густина маркерiв")
    ax.set_title(title, fontsize=13, pad=10)
    ax.legend(loc="upper right", fontsize=7, ncol=2)
    ax.grid(alpha=0.25, axis="y")
    _safe_savefig(fig, out_path)


def comparison_dashboard(
    metric_names: List[str],
    values_a: List[float],
    values_b: List[float],
    label_a: str,
    label_b: str,
    out_path: Path,
    title: str = "Comparison dashboard",
    color_a: str = COLOR_BAD,
    color_b: str = COLOR_OK,
):
    """Сравнение двух текстов по 6 метрикам."""
    n = len(metric_names)
    nrows = (n + 2) // 3
    ncols = min(3, n)
    fig, axes = plt.subplots(nrows, ncols, figsize=(5 * ncols, 4 * nrows),
                              constrained_layout=True)
    axes = np.atleast_1d(axes).flatten()
    for i, (ax, name, va, vb) in enumerate(zip(axes, metric_names, values_a, values_b)):
        bars = ax.bar([label_a, label_b], [va, vb],
                      color=[color_a, color_b], alpha=0.85,
                      edgecolor="black", linewidth=0.5)
        for b, v in zip(bars, [va, vb]):
            fmt = f"{v:.3f}" if isinstance(v, float) and abs(v) < 10 else f"{int(v)}"
            ax.text(b.get_x() + b.get_width() / 2, v + 0.005 * max(va, vb, 1),
                    fmt, ha="center", va="bottom", fontsize=10, fontweight="bold")
        ax.set_title(name, fontsize=11)
        ax.grid(alpha=0.25, axis="y")
        mx = max(va, vb, 0.01)
        ax.set_ylim(0, mx * 1.25 + 0.01)
    # Скрыть лишние
    for j in range(len(metric_names), len(axes)):
        axes[j].set_visible(False)
    fig.suptitle(title, fontsize=14, fontweight="bold")
    _safe_savefig(fig, out_path)


def comparison_matrices(
    sim_a: np.ndarray,
    sim_b: np.ndarray,
    titles_a: List[str],
    titles_b: List[str],
    label_a: str,
    label_b: str,
    out_path: Path,
    title: str = "Chapter matrices",
    cmap: str = "viridis",
):
    """Две матрицы глав бок-о-бок."""
    fig, axes = plt.subplots(1, 2, figsize=(16, 7), constrained_layout=True)
    for ax, sim, label, titles in [
        (axes[0], sim_a, label_a, titles_a),
        (axes[1], sim_b, label_b, titles_b),
    ]:
        im = ax.imshow(sim, cmap=cmap, vmin=0, vmax=1)
        ax.set_title(label, fontsize=12)
        labels = [_short_label(t) for t in titles]
        ax.set_xticks(range(len(labels)))
        ax.set_yticks(range(len(labels)))
        ax.set_xticklabels(labels, rotation=90, fontsize=6)
        ax.set_yticklabels(labels, fontsize=6)
        fig.colorbar(im, ax=ax, fraction=0.046, pad=0.02)
    fig.suptitle(title, fontsize=14, fontweight="bold")
    _safe_savefig(fig, out_path)


def cooccurrence_matrix(
    cooc: np.ndarray,
    names: List[str],
    out_path: Path,
    title: str = "Co-occurrence matrix",
):
    """Матрица совместной встречаемости персонажей."""
    n = len(names)
    fig, ax = plt.subplots(figsize=(9, 7), constrained_layout=True)
    im = ax.imshow(cooc, cmap="YlOrRd")
    ax.set_xticks(range(n))
    ax.set_yticks(range(n))
    ax.set_xticklabels(names, rotation=45, ha="right", fontsize=9)
    ax.set_yticklabels(names, fontsize=9)
    ax.set_title(title, fontsize=13, pad=10)
    vmax = cooc.max() if cooc.size > 0 else 1
    for i in range(n):
        for j in range(n):
            if cooc[i, j] > 0:
                color = "black" if cooc[i, j] < vmax / 2 else "white"
                ax.text(j, i, str(int(cooc[i, j])), ha="center", va="center",
                        fontsize=8, color=color)
    fig.colorbar(im, ax=ax, fraction=0.046, pad=0.02)
    _safe_savefig(fig, out_path)


__all__ = [
    "chapter_matrix",
    "character_map",
    "transition_smoothness",
    "pca_scatter",
    "entropy_per_chapter",
    "cluster_sizes_bar",
    "themes_per_chapter",
    "comparison_dashboard",
    "comparison_matrices",
    "cooccurrence_matrix",
]
