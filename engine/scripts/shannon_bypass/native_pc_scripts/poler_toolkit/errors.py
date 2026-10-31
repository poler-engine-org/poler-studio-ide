"""
poler_toolkit.errors — ієрархія помилок (реконструкція за аудитом 2026-09-21).

Оригінальний коміт 7375791 містив core/report/runner/viz/theme_evolution,
АЛЕ без errors.py/paths.py/recipes.py/__init__.py і залежності poler_v6 —
пакет не імпортувався взагалі. Сигнатури відновлені за викликами в коді:
  FileError(path, reason) · UnsupportedFormatError(path, ext)
  PolerEngineError(op, exc) · RecipeError(msg, hint=None)
  ConfigurationError(msg, hint=None) · OutputError(msg)
  VisualizationError(msg)
"""

from __future__ import annotations


class PolerToolkitError(Exception):
    """Базовий клас усіх помилок poler_toolkit."""


class PolerEngineError(PolerToolkitError):
    """Помилка виклику движка POLER (поламаний/відсутній backend)."""

    def __init__(self, op: str, exc: Exception | str):
        self.op = op
        self.exc = exc
        super().__init__(f"POLER engine call '{op}' failed: {exc}")


class FileError(PolerToolkitError):
    """Файл недоступний / читається з помилкою."""

    def __init__(self, path: str, reason: str):
        self.path = path
        self.reason = reason
        super().__init__(f"file '{path}': {reason}")


class UnsupportedFormatError(PolerToolkitError):
    """Непідтримуване розширення файлу."""

    def __init__(self, path: str, ext: str):
        self.path = path
        self.ext = ext
        super().__init__(f"unsupported format '{ext}' for '{path}' "
                         f"(підтримуються .txt / .md / .epub)")


class RecipeError(PolerToolkitError):
    """Помилка виконання рецепту (дані не підходять тощо)."""

    def __init__(self, msg: str, hint: str | None = None):
        self.hint = hint
        text = f"recipe error: {msg}"
        if hint:
            text += f" (підказка: {hint})"
        super().__init__(text)


class ConfigurationError(PolerToolkitError):
    """Невірна конфігурація виклику (кількість файлів, опції)."""

    def __init__(self, msg: str, hint: str | None = None):
        self.hint = hint
        text = f"configuration error: {msg}"
        if hint:
            text += f" (підказка: {hint})"
        super().__init__(text)


class OutputError(PolerToolkitError):
    """Не вдалося записати вихідний артефакт."""


class VisualizationError(PolerToolkitError):
    """Помилка рендерингу графіка (matplotlib)."""


__all__ = [
    "PolerToolkitError", "PolerEngineError", "FileError",
    "UnsupportedFormatError", "RecipeError", "ConfigurationError",
    "OutputError", "VisualizationError",
]
