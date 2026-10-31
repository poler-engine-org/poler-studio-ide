"""
poler_toolkit — інструментарій аналізу наративних текстів на POLER-лексиконах.

Реконструкція за аудитом 2026-09-21: коміт 7375791 приніс core/report/runner/
viz/theme_evolution БЕЗ __init__/errors/paths/recipes/poler_v6 — пакет не
імпортувався. Відсутні модулі відновлені (див. README.md у каталозі пакета).

Швидкий старт:
    from poler_toolkit.runner import run
    out = run("theme_evolution", ["roman.md"])
"""

__version__ = "1.1.0-reconstructed"

from . import errors, paths  # noqa: F401

__all__ = ["errors", "paths", "__version__"]
