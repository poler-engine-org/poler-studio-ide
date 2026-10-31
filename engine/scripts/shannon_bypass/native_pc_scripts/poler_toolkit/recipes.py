"""
poler_toolkit.recipes — реєстр доступних рецептів (реконструкція аудиту 2026-09-21).

Оригінальний runner.py імпортував `from . import recipes`, але самого
модуля в коміті 7375791 не було. Відновлено мінімальний реєстр з тим, що
реально присутнє: recipes_ext.theme_evolution (контракт Stage 2).
"""

from __future__ import annotations

from typing import Any, Dict

from .recipes_ext.theme_evolution import recipe_theme_evolution

RECIPES: Dict[str, Dict[str, Any]] = {
    "theme_evolution": {
        "fn": recipe_theme_evolution,
        "n_files": 1,
        "description": "Evolution of EMOTIONAL_MARKERS across chapters",
        "options": ["chapter_pattern", "lang", "top_n"],
    },
}

__all__ = ["RECIPES"]
