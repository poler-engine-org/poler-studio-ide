"""
poler_toolkit.runner — оркестратор.

runner.run(recipe_name, filepaths, opts) -> Path (выходная директория)

Шаги:
  1. Проверить recipe_name в RECIPES
  2. Проверить количество файлов
  3. Создать выходную директорию с timestamp
  4. Вызвать recipe-функцию с out_dir
  5. Сохранить report.json + verdict.md
  6. Вернуть путь к выходной директории
"""

from __future__ import annotations

import logging
from pathlib import Path
from typing import Any, Dict, List, Optional

from . import core
from . import report
from . import recipes
from .errors import RecipeError, ConfigurationError

log = logging.getLogger("poler_toolkit.runner")


def list_recipes() -> Dict[str, Dict[str, Any]]:
    """Возвращает словарь доступных рецептов."""
    return recipes.RECIPES


def run(
    recipe_name: str,
    filepaths: List[str | Path],
    *,
    keyword: Optional[str] = None,
    chapter_pattern: Optional[str] = None,
    lang: str = "auto",
    top_n: int = 15,
    names: Optional[List[str]] = None,
    # ---- Stage 2 ext-recipe options (Agents A-E) ----
    plugin_path: Optional[str] = None,
    timeout: int = 300,
    plugin_opts: Optional[Dict[str, Any]] = None,
    extensions: Optional[List[str]] = None,
    recursive: bool = True,
    max_locations: int = 20,
    window_size: int = 3000,
    # ---- end ext-recipe options ----
    # ---- smart_toolkit options ----
    mode: str = "summary",
    query: Optional[str] = None,
    max_chars: int = 2000,
    context_chars: int = 500,
    pattern: Optional[str] = None,
    include: Optional[str] = None,
    exclude: Optional[str] = None,
    max_results: int = 50,
    context_lines: int = 2,
    ignore_case: bool = True,
    action: str = "get",
    cache_dir: Optional[str] = None,
    # ---- end smart_toolkit options ----
    output_root: Optional[Path] = None,
    task_label: Optional[str] = None,
) -> Path:
    """
    Запускает recipe на файлах.

    Args:
        recipe_name: 'analyze' / 'compare' / 'structure' / 'characters'
                     / 'theme_evolution' / 'themes_alpha' / 'diff_versions'
                     / 'batch_directory' / 'custom_plugin'
        filepaths: список файлов (1 для analyze/structure/characters/...,
                   2 для compare/diff_versions, 1 dir для batch_directory,
                   переменное число для custom_plugin)
        keyword: keyword для POLER (None = auto-detect)
        chapter_pattern: regex для глав (None = auto)
        lang: язык глав ('auto' / 'ru' / 'ua' / 'en' / 'inline_ru' / 'inline_en')
        top_n: top_n для POLER analyze (default 15)
        names: список персонажей для recipe='characters'
        plugin_path: путь к .py файлу плагина для recipe='custom_plugin'
        timeout: таймаут выполнения плагина в секундах (custom_plugin)
        plugin_opts: dict опций, передаваемых в плагин (custom_plugin)
        extensions: список расширений файлов для batch_directory
                    (default: ['.txt', '.md'])
        recursive: рекурсивный обход каталога для batch_directory
        max_locations: максимум source locations на слово для themes_alpha
        window_size: размер окна POLER для diff_versions
        output_root: корень для выходных файлов (default: env $POLER_TOOLKIT_OUTPUT
                     или ~/.local/share/poler_toolkit/, см. paths.py)
        task_label: метка задачи (default: recipe_name)

    Returns:
        Путь к созданной выходной директории.
    """
    # 1. Проверить recipe
    if recipe_name not in recipes.RECIPES:
        available = ", ".join(recipes.RECIPES.keys())
        raise RecipeError(
            f"unknown recipe '{recipe_name}'",
            hint=f"Available: {available}",
        )
    spec = recipes.RECIPES[recipe_name]
    fn = spec["fn"]
    expected_n = spec["n_files"]

    # 2. Проверить файлы (n_files=None → переменное число, пропуск проверки)
    if expected_n is not None and len(filepaths) != expected_n:
        raise ConfigurationError(
            f"recipe '{recipe_name}' expects {expected_n} file(s), got {len(filepaths)}",
            hint=f"Use --help {recipe_name} for usage.",
        )
    if expected_n is None and not filepaths:
        raise ConfigurationError(
            f"recipe '{recipe_name}' expects at least 1 file, got 0",
            hint=f"Use --help {recipe_name} for usage.",
        )

    # 3. Выходная директория
    label = task_label or recipe_name
    out_dir = core.make_output_dir(label, output_root=output_root)
    log.info(f"Output dir: {out_dir}")

    # 4. Логирование входа
    log.info(f"=== RUN recipe='{recipe_name}' ===")
    for i, fp in enumerate(filepaths):
        log.info(f"  input[{i}]: {fp}")
    log.info(f"  opts: keyword={keyword}, lang={lang}, top_n={top_n}, names={names}")

    # 5. Вызов recipe-функции
    common_kwargs = {"out_dir": out_dir}
    if "keyword" in spec["options"]:
        common_kwargs["keyword"] = keyword
    if "chapter_pattern" in spec["options"]:
        common_kwargs["chapter_pattern"] = chapter_pattern
    if "lang" in spec["options"]:
        common_kwargs["lang"] = lang
    if "top_n" in spec["options"]:
        common_kwargs["top_n"] = top_n
    if "names" in spec["options"] and names is not None:
        common_kwargs["names"] = names
    # ---- Stage 2 ext-recipe option forwarding ----
    if "plugin_path" in spec["options"] and plugin_path is not None:
        common_kwargs["plugin_path"] = plugin_path
    if "timeout" in spec["options"]:
        common_kwargs["timeout"] = timeout
    if "plugin_opts" in spec["options"] and plugin_opts is not None:
        common_kwargs["plugin_opts"] = plugin_opts
    if "extensions" in spec["options"] and extensions is not None:
        common_kwargs["extensions"] = extensions
    if "recursive" in spec["options"]:
        common_kwargs["recursive"] = recursive
    if "max_locations" in spec["options"]:
        common_kwargs["max_locations"] = max_locations
    if "window_size" in spec["options"]:
        common_kwargs["window_size"] = window_size
    # ---- smart_toolkit option forwarding ----
    if "mode" in spec["options"]:
        common_kwargs["mode"] = mode
    if "query" in spec["options"] and query is not None:
        common_kwargs["query"] = query
    if "max_chars" in spec["options"]:
        common_kwargs["max_chars"] = max_chars
    if "context_chars" in spec["options"]:
        common_kwargs["context_chars"] = context_chars
    if "pattern" in spec["options"] and pattern is not None:
        common_kwargs["pattern"] = pattern
    if "include" in spec["options"] and include is not None:
        common_kwargs["include"] = include
    if "exclude" in spec["options"] and exclude is not None:
        common_kwargs["exclude"] = exclude
    if "max_results" in spec["options"]:
        common_kwargs["max_results"] = max_results
    if "context_lines" in spec["options"]:
        common_kwargs["context_lines"] = context_lines
    if "ignore_case" in spec["options"]:
        common_kwargs["ignore_case"] = ignore_case
    if "action" in spec["options"]:
        common_kwargs["action"] = action
    if "cache_dir" in spec["options"] and cache_dir is not None:
        common_kwargs["cache_dir"] = cache_dir

    bundle = fn(filepaths, **common_kwargs)

    # 6. Сохранить JSON-отчёт
    json_path = report.save_json_report(
        bundle["report"], out_dir, filename="report.json",
    )
    log.info(f"JSON: {json_path}")

    # 7. Сохранить Markdown verdict
    md_path = report.save_markdown_verdict(
        bundle["verdict_lines"], out_dir, filename="verdict.md",
    )
    log.info(f"Markdown: {md_path}")

    # 8. Финальный лог
    log.info(f"=== DONE: {recipe_name} ===")
    log.info(f"  artifacts: {len(bundle['artifacts'])} PNG files")
    log.info(f"  verdict: {bundle['report']['verdict'][:120]}...")

    return out_dir


__all__ = ["run", "list_recipes"]
