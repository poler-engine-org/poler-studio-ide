"""
poler_toolkit.paths — розміщення вихідних даних та політики sys.path
(реконструкція за аудитом 2026-09-21).

get_output_root():
    $POLER_TOOLKIT_OUTPUT → XDG_DATA_HOME → ~/.local/share/poler_toolkit
ensure_poler_v6_on_path():
    Додає каталог самого пакета poler_toolkit у sys.path, щоб
    `import poler_v6` знаходив локальний шим poler_v6.py
    (канонічні лексикони POLER без залежності від litgraph-desktop).
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

_PKG_DIR = Path(__file__).resolve().parent


def get_output_root() -> Path:
    """Корінь вихідних директорій (env-aware)."""
    env = os.environ.get("POLER_TOOLKIT_OUTPUT")
    if env:
        root = Path(env)
    elif os.environ.get("XDG_DATA_HOME"):
        root = Path(os.environ["XDG_DATA_HOME"]) / "poler_toolkit"
    else:
        root = Path.home() / ".local" / "share" / "poler_toolkit"
    root.mkdir(parents=True, exist_ok=True)
    return root


def ensure_poler_v6_on_path() -> bool:
    """Гарантує, що `import poler_v6` резолвиться в локальний шим пакета."""
    pkg = str(_PKG_DIR)
    if pkg not in sys.path:
        sys.path.insert(0, pkg)
    return (_PKG_DIR / "poler_v6.py").exists()


__all__ = ["get_output_root", "ensure_poler_v6_on_path"]
