"""
poler_toolkit.report — генерация отчётов.

Два выходных формата:
  - JSON (машинно-читаемый, для downstream обработки)
  - Markdown verdict (человекочитаемый, краткая сводка)

Все отчёты пишутся в одну выходную директорию с timestamp.
"""

from __future__ import annotations

import json
import logging
from pathlib import Path
from datetime import datetime
from typing import Any, Dict, Optional, List

from .errors import OutputError

log = logging.getLogger("poler_toolkit.report")


# ============================================================
# JSON-отчёт
# ============================================================
def save_json_report(
    data: Dict[str, Any],
    out_dir: Path,
    filename: str = "report.json",
) -> Path:
    """
    Сохраняет data как JSON в out_dir/filename.
    Бросает OutputError при падении.
    """
    out_path = out_dir / filename
    try:
        out_path.write_text(
            json.dumps(data, ensure_ascii=False, indent=2, default=str),
            encoding="utf-8",
        )
        log.info(f"JSON report saved: {out_path}")
        return out_path
    except Exception as e:
        raise OutputError(f"Cannot write {out_path}: {e}")


# ============================================================
# Markdown verdict
# ============================================================
def save_markdown_verdict(
    lines: List[str],
    out_dir: Path,
    filename: str = "verdict.md",
) -> Path:
    """Сохраняет verdict как Markdown."""
    out_path = out_dir / filename
    try:
        out_path.write_text("\n".join(lines), encoding="utf-8")
        log.info(f"Markdown verdict saved: {out_path}")
        return out_path
    except Exception as e:
        raise OutputError(f"Cannot write {out_path}: {e}")


# ============================================================
# Стандартная обёртка для всех отчётов
# ============================================================
def wrap_report(
    task_name: str,
    engine: str,
    input_files: List[str],
    metrics: Dict[str, Any],
    output_files: Dict[str, str],
    verdict: str,
    extra: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    """
    Стандартный шаблон отчёта.
    Все recipe-функции возвращают dict этой структуры.
    """
    report = {
        "meta": {
            "task": task_name,
            "engine": engine,
            "timestamp": datetime.now().isoformat(timespec="seconds"),
            "toolkit_version": "1.0.0",
        },
        "inputs": input_files,
        "metrics": metrics,
        "outputs": output_files,
        "verdict": verdict,
    }
    if extra:
        report["extra"] = extra
    return report


# ============================================================
# Helpers для verdict-ов
# ============================================================
def verdict_block(title: str, *line_groups: List[str]) -> List[str]:
    """
    Форматирует блок verdict-а.
    Принимает title + произвольное количество списков строк,
    склеивает их в один Markdown-блок.
    """
    out = [f"# {title}", ""]
    for lines in line_groups:
        out.extend(lines)
        if not lines or lines[-1] != "":
            out.append("")
    return out


def metric_table(headers: List[str], rows: List[List[str]]) -> List[str]:
    """Markdown-таблица метрик."""
    lines = [
        "| " + " | ".join(headers) + " |",
        "|" + "|".join(["---"] * len(headers)) + "|",
    ]
    for row in rows:
        lines.append("| " + " | ".join(row) + " |")
    lines.append("")
    return lines


def coherence_verdict(mean_cosine: float, silhouette: float) -> str:
    """Авто-вердикт по coherence + silhouette."""
    if mean_cosine < 0.10 and silhouette < 0.05:
        return "КЛАСТЕРИ НЕ СКЛАДАЮТЬСЯ — текст розпадається на iзольованi шматки."
    if mean_cosine < 0.15 and silhouette < 0.15:
        return "СЛАБКА СКЛАДАНIСТЬ — кластери формальнi, але зв'язок ламкий."
    if mean_cosine < 0.25 and silhouette < 0.30:
        return "СЕРЕДНЯ СКЛАДАНIСТЬ — структура ║, але з провалами."
    if mean_cosine >= 0.25 and silhouette >= 0.30:
        return "ВИСОКА СКЛАДАНIСТЬ — кластери тримаються, переходи плавнi."
    return "ЗМIШАНА КАРТИНА — окремi блоки тримаються, iншi розсипаються."


__all__ = [
    "save_json_report",
    "save_markdown_verdict",
    "wrap_report",
    "verdict_block",
    "metric_table",
    "coherence_verdict",
]
