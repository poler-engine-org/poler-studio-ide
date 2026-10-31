"""
poler_toolkit.recipes_ext.theme_evolution
=========================================

Recipe: track how ``EMOTIONAL_MARKERS`` from POLER v6 evolve across chapters
of a narrative text.

What the recipe does
--------------------
1. Read the file via ``core.read_any`` (supports .txt / .md / .epub).
2. Split the text into chapters via ``core.split_chapters``.
3. For each chapter, count occurrences of every marker in
   ``poler_v6.EMOTIONAL_MARKERS`` (a set of 60 multi-lingual terms:
   RU / UK / EN).
4. Normalize counts by chapter length (markers per 1000 tokens) so that
   long and short chapters are directly comparable.
5. Rank markers by total raw frequency and pick top-N (default 20) for the
   visualizations.
6. Render three PNG artifacts:
   - **Stacked area chart** — top-N marker *proportions* per chapter.
   - **Heatmap** — chapters (rows) × top-N markers (cols), normalized
     density on a ``magma`` colour scale.
   - **Line chart** — top-5 markers' evolution across chapters.
7. Emit a JSON ``report`` with the full chapters × markers matrix, the
   per-chapter dominant marker, and variance statistics.
8. Emit a Markdown ``verdict`` identifying:
   - Most stable marker (lowest variance of normalized density).
   - Most volatile marker (highest variance of normalized density).
   - Chapter with the highest emotional density.
   - Chapter with the lowest emotional density.

The function signature matches the Stage 2 contract so the main agent can
register it in ``recipes.RECIPES`` directly:

    "theme_evolution": {
        "fn": recipe_theme_evolution,
        "n_files": 1,
        "description": "Evolution of EMOTIONAL_MARKERS across chapters",
        "options": ["chapter_pattern", "lang", "top_n"],
    }
"""

from __future__ import annotations

import re
import logging
from pathlib import Path
from typing import Any, Dict, List, Optional
from collections import Counter

import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

# Recipe lives in poler_toolkit.recipes_ext, so parent-of-parent is
# poler_toolkit itself. Primary path: relative imports (matches Stage 2
# integration contract). Fallback: absolute imports, so the file can also
# be invoked directly as `python3 .../theme_evolution.py FILE` for testing.
try:
    from .. import core
    from .. import report
    from .. import viz
    from ..errors import RecipeError, ConfigurationError
except ImportError:  # pragma: no cover - script-mode fallback
    import os as _os
    import sys as _sys
    _here = _os.path.dirname(_os.path.abspath(__file__))
    _scripts = _os.path.dirname(_os.path.dirname(_here))
    if _scripts not in _sys.path:
        _sys.path.insert(0, _scripts)
    from poler_toolkit import core  # type: ignore
    from poler_toolkit import report  # type: ignore
    from poler_toolkit import viz  # type: ignore
    from poler_toolkit.errors import RecipeError, ConfigurationError  # type: ignore

log = logging.getLogger("poler_toolkit.recipes_ext.theme_evolution")


# ============================================================
# Local visualization helpers
# ------------------------------------------------------------
# Stage 2 constraint: do NOT modify viz.py. The existing viz.* functions
# cover heatmaps for cosine similarity (square matrix) and stacked BAR
# charts, but the task explicitly asks for stacked AREA + a non-square
# chapter×marker heatmap + a multi-series line chart. We implement those
# three locally so viz.py stays untouched for the main agent.
# ============================================================
def _short_label(title: str) -> str:
    """Collapse 'Глава 12 (Робоча назва): Уламок' -> '12'."""
    m = re.search(r"\d+", title)
    return m.group(0) if m else title[:10]


def _safe_savefig(fig, path: Path) -> None:
    """Save + close. Honours toolkit DPI."""
    try:
        fig.savefig(path, dpi=viz.DPI)
        log.info(f"Saved: {path}")
    except Exception as e:
        raise viz.VisualizationError(f"savefig failed: {e}") from e
    finally:
        plt.close(fig)


def _stacked_area(
    matrix: np.ndarray,           # shape (chapters, n_markers)
    markers: List[str],
    chapter_titles: List[str],
    out_path: Path,
    title: str = "Marker proportions per chapter",
) -> None:
    """
    Stacked area chart of top-N marker *proportions* per chapter.

    Each chapter's row is normalized so that the visible top-N markers sum
    to 1.0 (residual 'other' mass, if any, is dropped — it would always be
    zero here because we sum over the same top-N subset).
    """
    n_chap, n_m = matrix.shape
    row_sums = matrix.sum(axis=1, keepdims=True)
    row_sums[row_sums == 0] = 1.0
    prop = matrix / row_sums

    fig, ax = plt.subplots(figsize=(13, 6), constrained_layout=True)
    x = np.arange(n_chap)
    colors = plt.cm.tab20(np.linspace(0, 1, max(n_m, 1)))
    stack = np.zeros(n_chap)
    for k in range(n_m):
        ax.fill_between(
            x, stack, stack + prop[:, k],
            color=colors[k], alpha=0.85, linewidth=0.4,
            label=markers[k],
        )
        stack = stack + prop[:, k]

    labels = [_short_label(t) for t in chapter_titles]
    ax.set_xticks(x)
    ax.set_xticklabels(
        labels, rotation=90, fontsize=max(5, 8 - n_chap // 30),
    )
    ax.set_xlabel("Глава")
    ax.set_ylabel("Доля маркера (в top-N)")
    ax.set_ylim(0, 1.0)
    ax.set_title(title, fontsize=13, pad=10)
    ax.legend(
        loc="center left", bbox_to_anchor=(1.01, 0.5),
        fontsize=7, ncol=1, framealpha=0.9,
    )
    ax.grid(alpha=0.25, axis="y")
    _safe_savefig(fig, out_path)


def _marker_heatmap(
    matrix: np.ndarray,           # shape (chapters, n_markers)
    markers: List[str],
    chapter_titles: List[str],
    out_path: Path,
    title: str = "Marker density heatmap",
    cmap: str = "magma",
) -> None:
    """
    Heatmap chapters × markers, normalized density (markers / 1000 tokens).

    Rows are markers (sorted by total frequency desc), columns are chapters
    in textual order. Cell colour encodes density.
    """
    n_chap, n_m = matrix.shape
    fig_w = max(10, n_chap * 0.32)
    fig_h = max(6, n_m * 0.40 + 2)
    fig, ax = plt.subplots(
        figsize=(fig_w, fig_h), constrained_layout=True,
    )
    display = matrix.T  # rows=markers, cols=chapters
    vmax = float(display.max()) if display.size and display.max() > 0 else 1.0
    im = ax.imshow(
        display, aspect="auto", cmap=cmap, vmin=0, vmax=vmax,
    )
    ax.set_xticks(range(n_chap))
    ax.set_yticks(range(n_m))
    chap_labels = [_short_label(t) for t in chapter_titles]
    ax.set_xticklabels(
        chap_labels, rotation=90, fontsize=max(5, 8 - n_chap // 30),
    )
    ax.set_yticklabels(markers, fontsize=max(7, 10 - n_m // 15))
    ax.set_xlabel("Глава")
    ax.set_ylabel("Маркер")
    ax.set_title(title, fontsize=13, pad=10)
    fig.colorbar(
        im, ax=ax, fraction=0.025, pad=0.02,
        label="маркеров / 1000 токенов",
    )
    _safe_savefig(fig, out_path)


def _top5_line_chart(
    matrix: np.ndarray,           # shape (chapters, k)  (k <= 5)
    markers: List[str],
    chapter_titles: List[str],
    out_path: Path,
    title: str = "Top-5 markers evolution",
) -> None:
    """Line chart of top-5 markers' normalized density across chapters."""
    n_chap, k = matrix.shape
    fig, ax = plt.subplots(figsize=(13, 6), constrained_layout=True)
    x = np.arange(n_chap)
    colors = plt.cm.tab10(np.linspace(0, 1, max(k, 1)))
    for j in range(k):
        ax.plot(
            x, matrix[:, j],
            marker="o", markersize=3.5, linewidth=1.7,
            color=colors[j], label=markers[j], alpha=0.9,
        )
    labels = [_short_label(t) for t in chapter_titles]
    ax.set_xticks(x)
    ax.set_xticklabels(
        labels, rotation=90, fontsize=max(5, 8 - n_chap // 30),
    )
    ax.set_xlabel("Глава")
    ax.set_ylabel("Маркеров на 1000 токенов")
    ax.set_title(title, fontsize=13, pad=10)
    ax.legend(loc="best", fontsize=9, ncol=min(k, 2), framealpha=0.9)
    ax.grid(alpha=0.3)
    _safe_savefig(fig, out_path)


# ============================================================
# Recipe
# ============================================================
def recipe_theme_evolution(
    filepaths: list,
    *,
    chapter_pattern: Optional[str] = None,
    lang: str = "auto",
    top_n: int = 20,
    out_dir: Path = None,
) -> dict:
    """
    Track how ``poler_v6.EMOTIONAL_MARKERS`` evolve across chapters.

    Args:
        filepaths: length-1 list with the input file path
            (.txt / .md / .epub supported via ``core.read_any``).
        chapter_pattern: optional regex for chapter headings. If ``None``,
            ``core.split_chapters`` auto-detects.
        lang: chapter-pattern language hint (``auto`` / ``ru`` / ``ua`` /
            ``en`` / ``inline_ru`` / ``inline_en``).
        top_n: how many top markers to keep for the heatmap + stacked area
            chart (default 20). Line chart always shows the top 5.
        out_dir: directory for PNG artifacts. If ``None``, no artifacts
            are written (useful when stacking recipes).

    Returns:
        ``{"report": dict, "verdict_lines": list, "artifacts": dict}``.
        The ``artifacts`` mapping contains at most three entries:
        ``stacked_area``, ``heatmap``, ``line_chart``.
    """
    # ----- 0. validate inputs -------------------------------------------
    if len(filepaths) != 1:
        raise ConfigurationError(
            "theme_evolution expects exactly 1 file",
            hint="Pass a single .txt / .md / .epub path.",
        )
    filepath = Path(filepaths[0])
    log.info(f"[theme_evolution] file={filepath}")

    # ----- 1. read + split chapters -------------------------------------
    text, fmt = core.read_any(filepath)
    log.info(f"  read: {len(text)} chars, format={fmt}")

    chapters = core.split_chapters(text, pattern=chapter_pattern, lang=lang)
    n_chap = len(chapters)
    log.info(f"  chapters: {n_chap}")
    if n_chap < 1:
        raise RecipeError(
            "no chapters detected",
            hint="Try --lang inline_ru or pass an explicit chapter_pattern.",
        )

    # ----- 2. load EMOTIONAL_MARKERS (sorted for stable column order) --
    poler = core.get_poler()
    markers = sorted(poler.EMOTIONAL_MARKERS)
    n_markers = len(markers)
    log.info(f"  markers: {n_markers}")

    # ----- 3. per-chapter counts + token counts -------------------------
    # NB: we deliberately use a local regex tokenizer here (matches what
    # core.auto_detect_keyword and recipes.recipe_analyze do) instead of
    # 60×N calls to core.safe_call('grep_search_file', ...) — the latter
    # would re-scan the file 60 times per chapter and is needlessly slow.
    # The pattern is word-aware (Cyrillic-safe via re.UNICODE default).
    token_re = re.compile(r"[\w’']+")
    raw_counts = np.zeros((n_chap, n_markers), dtype=np.int64)
    token_counts = np.zeros(n_chap, dtype=np.int64)
    for ci, ch in enumerate(chapters):
        body_lower = ch["body"].lower()
        tokens = token_re.findall(body_lower)
        token_counts[ci] = len(tokens)
        tc = Counter(tokens)
        for mi, m in enumerate(markers):
            raw_counts[ci, mi] = tc.get(m, 0)
    log.info(
        f"  total markers found: {int(raw_counts.sum())} "
        f"across {int((raw_counts.sum(axis=0) > 0).sum())}/{n_markers} active markers"
    )

    # ----- 4. normalize: markers per 1000 tokens ------------------------
    norm_matrix = np.zeros((n_chap, n_markers), dtype=np.float64)
    for ci in range(n_chap):
        if token_counts[ci] > 0:
            norm_matrix[ci] = raw_counts[ci] / token_counts[ci] * 1000.0

    # ----- 5. rank markers by total raw frequency -----------------------
    total_per_marker = raw_counts.sum(axis=0)
    top_n_actual = max(1, min(top_n, n_markers))
    # indices sorted by total count descending
    top_idx_list = list(np.argsort(-total_per_marker)[:top_n_actual])
    top_markers = [markers[i] for i in top_idx_list]
    top_norm = norm_matrix[:, top_idx_list]
    log.info(f"  top-{top_n_actual} markers: {top_markers[:5]} ...")

    # Top-5 for the line chart
    top5_idx = top_idx_list[:5]
    top5_markers = [markers[i] for i in top5_idx]
    top5_norm = norm_matrix[:, top5_idx]

    # ----- 6. per-chapter dominant marker -------------------------------
    dominant_per_chapter: List[Dict[str, Any]] = []
    for ci in range(n_chap):
        row = norm_matrix[ci]
        if row.sum() == 0:
            dominant_per_chapter.append({
                "chapter_index": ci,
                "title": chapters[ci]["title"],
                "marker": None,
                "density_per_1k": 0.0,
                "unique_markers": 0,
            })
        else:
            mi = int(np.argmax(row))
            dominant_per_chapter.append({
                "chapter_index": ci,
                "title": chapters[ci]["title"],
                "marker": markers[mi],
                "density_per_1k": float(row[mi]),
                "unique_markers": int((raw_counts[ci] > 0).sum()),
            })

    # ----- 7. variance per marker (on normalized values) ----------------
    # Stable = low variance of per-1000-tokens density across chapters.
    # Volatile = high variance.
    #
    # Two filters apply:
    #   - For 'most stable' we additionally require the marker to appear
    #     in >= min(max(2, n_chap // 4), n_chap) chapters. Without this
    #     filter, a marker that appears exactly once in one long chapter
    #     (giving a tiny density value) would have variance ≈ 0 and
    #     win 'most stable' — semantically wrong. We want 'stable' to
    #     mean 'consistently present at a similar rate'.
    #   - For 'most volatile' we only require appearance in >= 1 chapter
    #     (singletons can still spike).
    marker_variance = np.zeros(n_markers, dtype=np.float64)
    chapters_appearing = np.zeros(n_markers, dtype=np.int64)
    for mi in range(n_markers):
        if total_per_marker[mi] > 0:
            marker_variance[mi] = float(np.var(norm_matrix[:, mi]))
            chapters_appearing[mi] = int((raw_counts[:, mi] > 0).sum())
    appears_mask = total_per_marker > 0
    n_appearing = int(appears_mask.sum())

    # threshold for "stable": appear in at least 25% of chapters (>= 2)
    stable_min_chapters = max(2, min(n_chap, n_chap // 4))
    stable_mask = appears_mask & (chapters_appearing >= stable_min_chapters)
    if stable_mask.any():
        stable_idx_pool = np.where(stable_mask)[0]
        var_stable = marker_variance[stable_idx_pool]
        most_stable_idx = int(stable_idx_pool[int(np.argmin(var_stable))])
    elif n_appearing > 0:
        # fall back to all appearing markers if none is "consistent"
        appearing_idx = np.where(appears_mask)[0]
        var_among = marker_variance[appearing_idx]
        most_stable_idx = int(appearing_idx[int(np.argmin(var_among))])
    else:
        most_stable_idx = 0

    if n_appearing > 0:
        appearing_idx = np.where(appears_mask)[0]
        var_among = marker_variance[appearing_idx]
        most_volatile_idx = int(appearing_idx[int(np.argmax(var_among))])
    else:
        most_volatile_idx = 0

    # ----- 8. emotional density per chapter -----------------------------
    density_per_chapter = norm_matrix.sum(axis=1)
    if n_chap > 0 and density_per_chapter.sum() > 0:
        hi_ch = int(np.argmax(density_per_chapter))
        lo_ch = int(np.argmin(density_per_chapter))
    else:
        hi_ch = 0
        lo_ch = 0

    # ----- 9. render artifacts ------------------------------------------
    artifacts: Dict[str, Path] = {}
    if out_dir is not None:
        out_dir = Path(out_dir)
        out_dir.mkdir(parents=True, exist_ok=True)

        chapter_titles = [c["title"] for c in chapters]

        artifacts["stacked_area"] = out_dir / "01_stacked_area.png"
        _stacked_area(
            top_norm, top_markers, chapter_titles,
            artifacts["stacked_area"],
            title=f"{filepath.stem} — эволюция маркеров (stacked area)",
        )

        artifacts["heatmap"] = out_dir / "02_heatmap.png"
        _marker_heatmap(
            top_norm, top_markers, chapter_titles,
            artifacts["heatmap"],
            title=f"{filepath.stem} — плотность маркеров (на 1000 токенов)",
        )

        artifacts["line_chart"] = out_dir / "03_line_top5.png"
        _top5_line_chart(
            top5_norm, top5_markers, chapter_titles,
            artifacts["line_chart"],
            title=f"{filepath.stem} — топ-5 маркеров (на 1000 токенов)",
        )

    # ----- 10. build metrics dict for JSON ------------------------------
    stable = {
        "marker": markers[most_stable_idx],
        "variance": float(marker_variance[most_stable_idx]),
        "mean_density_per_1k": float(norm_matrix[:, most_stable_idx].mean()),
        "total_count": int(total_per_marker[most_stable_idx]),
        "chapters_appearing": int(chapters_appearing[most_stable_idx]),
    }
    volatile = {
        "marker": markers[most_volatile_idx],
        "variance": float(marker_variance[most_volatile_idx]),
        "mean_density_per_1k": float(norm_matrix[:, most_volatile_idx].mean()),
        "total_count": int(total_per_marker[most_volatile_idx]),
        "chapters_appearing": int(chapters_appearing[most_volatile_idx]),
    }
    hi = {
        "chapter_index": hi_ch,
        "title": chapters[hi_ch]["title"],
        "density_per_1k": float(density_per_chapter[hi_ch]),
        "tokens": int(token_counts[hi_ch]),
    }
    lo = {
        "chapter_index": lo_ch,
        "title": chapters[lo_ch]["title"],
        "density_per_1k": float(density_per_chapter[lo_ch]),
        "tokens": int(token_counts[lo_ch]),
    }

    metrics: Dict[str, Any] = {
        # input summary
        "file": str(filepath),
        "format": fmt,
        "total_chars": int(len(text)),
        "total_words": int(len(text.split())),
        "total_tokens_regex": int(token_counts.sum()),
        "total_chapters": int(n_chap),
        # markers summary
        "markers_total": int(n_markers),
        "markers_appearing": int(n_appearing),
        "top_n": int(top_n_actual),
        "top_markers": top_markers,
        "top5_markers": top5_markers,
        # per-chapter info
        "chapter_titles": [c["title"] for c in chapters],
        "chapter_token_counts": token_counts.tolist(),
        "chapter_emotional_density_per_1k": density_per_chapter.tolist(),
        "dominant_marker_per_chapter": dominant_per_chapter,
        # full matrix (chapters × all markers)
        "matrix_markers": markers,
        "matrix_norm_per_1k": norm_matrix.tolist(),
        "matrix_raw_counts": raw_counts.tolist(),
        # top-N matrix (subset, for the 2 charts)
        "top_matrix_markers": top_markers,
        "top_matrix_norm_per_1k": top_norm.tolist(),
        # variance stats
        "marker_variance": marker_variance.tolist(),
        "marker_chapters_appearing": chapters_appearing.tolist(),
        "stable_min_chapters_threshold": int(stable_min_chapters),
        "most_stable_marker": stable,
        "most_volatile_marker": volatile,
        "highest_density_chapter": hi,
        "lowest_density_chapter": lo,
    }

    # ----- 11. Markdown verdict -----------------------------------------
    verdict_text = (
        f"Эволюция маркеров: «{stable['marker']}» — самый стабильный "
        f"(var={stable['variance']:.4f}, mean={stable['mean_density_per_1k']:.2f}/1000, "
        f"в {stable['chapters_appearing']}/{n_chap} главах); "
        f"«{volatile['marker']}» — самый волатильный "
        f"(var={volatile['variance']:.4f}, mean={volatile['mean_density_per_1k']:.2f}/1000, "
        f"в {volatile['chapters_appearing']}/{n_chap} главах). "
        f"Пик эмоциональной плотности — глава «{hi['title']}» "
        f"({hi['density_per_1k']:.2f}/1000); "
        f"минимум — глава «{lo['title']}» ({lo['density_per_1k']:.2f}/1000). "
        f"Активных маркеров: {n_appearing}/{n_markers}."
    )

    verdict_lines = report.verdict_block(
        f"Theme Evolution: {filepath.name}",
        report.metric_table(
            ["Метрика", "Значение"],
            [
                ["Файл", str(filepath)],
                ["Слов (whitespace)", str(metrics["total_words"])],
                ["Токенов (regex)", str(metrics["total_tokens_regex"])],
                ["Глав", str(metrics["total_chapters"])],
                ["Маркеров всего", str(metrics["markers_total"])],
                ["Активных маркеров", str(metrics["markers_appearing"])],
                ["Top-N визуализации", str(metrics["top_n"])],
                ["Порог 'stable' (глав)", str(stable_min_chapters)],
                ["Самый стабильный", stable["marker"]],
                ["  var (stable)", f"{stable['variance']:.6f}"],
                ["  mean / 1000", f"{stable['mean_density_per_1k']:.3f}"],
                ["  глав с маркером", f"{stable['chapters_appearing']}/{n_chap}"],
                ["Самый волатильный", volatile["marker"]],
                ["  var (volatile)", f"{volatile['variance']:.6f}"],
                ["  mean / 1000", f"{volatile['mean_density_per_1k']:.3f}"],
                ["  глав с маркером", f"{volatile['chapters_appearing']}/{n_chap}"],
                ["Глава пик плотности",
                 f"#{hi['chapter_index'] + 1} «{hi['title']}»"],
                ["  density", f"{hi['density_per_1k']:.2f}/1000"],
                ["  tokens", str(hi["tokens"])],
                ["Глава минимум плотности",
                 f"#{lo['chapter_index'] + 1} «{lo['title']}»"],
                ["  density", f"{lo['density_per_1k']:.2f}/1000"],
                ["  tokens", str(lo["tokens"])],
            ],
        ),
        ["## Вердикт", "", f"**{verdict_text}**", ""],
    )

    # ----- 12. wrap into standard report bundle -------------------------
    full_report = report.wrap_report(
        task_name="theme_evolution",
        engine="POLER v6 EMOTIONAL_MARKERS",
        input_files=[str(filepath)],
        metrics=metrics,
        output_files={k: str(v) for k, v in artifacts.items()},
        verdict=verdict_text,
    )

    return {
        "report": full_report,
        "verdict_lines": verdict_lines,
        "artifacts": artifacts,
    }


__all__ = ["recipe_theme_evolution"]


# ============================================================
# Standalone test entry-point
# ------------------------------------------------------------
# Run:  python3 poler_toolkit/recipes_ext/theme_evolution.py [FILE]
# If no FILE is given, defaults to env $POLER_TOOLKIT_TEST_FILE or
# the Cassiopeia canon sample (if available).
# Output: env $POLER_TOOLKIT_OUTPUT/test_theme_evolution_<ts>/
# ============================================================
if __name__ == "__main__":
    import sys
    import os
    import json
    from datetime import datetime

    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s [%(name)s] %(levelname)s: %(message)s",
    )

    default_file = os.environ.get("POLER_TOOLKIT_TEST_FILE", "")
    if not default_file or not Path(default_file).exists():
        # Fallback: try common sample locations
        for cand in [
            "upload/Касіопея исп канон главы 1 -58 (1).txt",
            "upload/Касіопея исп канон главы 1 -58 (1).txt",
        ]:
            if Path(cand).exists():
                default_file = cand
                break
    test_file = sys.argv[1] if len(sys.argv) > 1 else default_file

    # Output dir via paths.py (env-aware)
    from .. import paths as _paths
    output_root = _paths.get_output_root()
    ts = datetime.now().strftime("%Y%m%d_%H%M%S")
    out_dir = output_root / f"test_theme_evolution_{ts}"
    out_dir.mkdir(parents=True, exist_ok=True)

    print(f"Running theme_evolution on: {test_file}")
    print(f"Output dir: {out_dir}")

    bundle = recipe_theme_evolution(
        [test_file],
        chapter_pattern=None,
        lang="auto",
        top_n=20,
        out_dir=out_dir,
    )

    # Write report.json + verdict.md (mirrors what runner.run() does)
    (out_dir / "report.json").write_text(
        json.dumps(
            bundle["report"], ensure_ascii=False, indent=2, default=str,
        ),
        encoding="utf-8",
    )
    (out_dir / "verdict.md").write_text(
        "\n".join(bundle["verdict_lines"]), encoding="utf-8",
    )

    print("\n=== DONE ===")
    print(f"Artifacts ({len(bundle['artifacts'])}):")
    for name, p in bundle["artifacts"].items():
        print(f"  {name:14s} -> {p}  (exists={p.exists()})")
    print(f"\nreport.json: {out_dir / 'report.json'}")
    print(f"verdict.md:  {out_dir / 'verdict.md'}")
    print(f"\nVerdict: {bundle['report']['verdict']}")
