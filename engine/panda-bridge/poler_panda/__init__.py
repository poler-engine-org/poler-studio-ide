#!/usr/bin/env python3
"""
poler_panda — мост POLER ENGINE (физика) → Panda3D (рендер).

Цикл W «Симбиоз» (v0.59.0). Вода считается НЕ в Panda3D и не в Python:
она живёт в libpoler_ffi.so (Rust, спектральная гидродинамика GF(3)),
а Python только переносит числа в геометрию кадра.

    Panda3D (Python)                     POLER ENGINE (Rust)
    ─────────────────                    ───────────────────
    GeomVertexData 128×128   ◀────────   polerf_water_frame (тик + FFT)
    GLSL: Френель/пена/Блики             полерf_water_surface_at (нормали)
    камера, небо, ассеты                 polerf_water_flow_at (течения)
"""

import ctypes
import os
import platform
from pathlib import Path

_SO_NAMES = {
    "Linux": "libpoler_ffi.so",
    "Darwin": "libpoler_ffi.dylib",
    "Windows": "poler_ffi.dll",
}

# Порядок поиска библиотек (первый найденный выигрывает)
def _candidate_paths():
    name = _SO_NAMES.get(platform.system(), "libpoler_ffi.so")
    here = Path(__file__).resolve().parent
    roots = []
    if env := os.environ.get("POLER_FFI_LIB"):
        roots.append(Path(env))
    # 1. рядом с пакетом (раскладка релиза: panda-bridge/ + lib/)
    roots += [
        here / name,
        here.parent / name,
        here.parent / "lib" / name,
        here.parent.parent / "lib" / name,
    ]
    # 2. сборка в дереве исходников poler-engine
    for base in (here.parent.parent, here.parent, Path.cwd()):
        roots += [
            base / "target" / "release" / name,
            base / "target" / "debug" / name,
        ]
    # 3. системный поиск dlopen
    roots.append(Path(name))
    return roots


def load_library() -> ctypes.CDLL:
    """Найти и загрузить libpoler_ffi с рукопожатием ABI."""
    errors = []
    for p in _candidate_paths():
        try:
            lib = ctypes.CDLL(str(p))
        except OSError as e:
            errors.append(f"{p}: {e}")
            continue
        lib.polerf_version.restype = ctypes.c_char_p
        lib.polerf_abi.restype = ctypes.c_uint32
        abi = lib.polerf_abi()
        if abi != 1:
            raise RuntimeError(f"POLER FFI ABI {abi} != 1 (ожидался 1)")
        lib._poler_path = str(p)  # type: ignore[attr-defined]
        lib._poler_version = lib.polerf_version().decode()  # type: ignore[attr-defined]
        return lib
    raise RuntimeError(
        "libpoler_ffi не найдена. Пробовали:\n  "
        + "\n  ".join(errors)
        + "\nСоберите: cargo build --release -p poler-ffi (в poler-engine)"
    )


class Water:
    """Живое море POLER: состояние 1–2 КБ, ноль f32-дрейфа.

    Параметры конструктора — как у `game water`:
      n        — сетка синтеза (степень двойки, 16..1024)
      modes    — число активных волн
      seed     — сид детерминизма
      wind     — скорость ветра, м/с (H_s ≈ 0.21·v²/g)
      domain   — физический размер, м
      wind_dir — направление ветра, рад
    """

    def __init__(
        self,
        n: int = 128,
        modes: int = 160,
        seed: int = 42,
        wind: float = 8.0,
        domain: float = 120.0,
        wind_dir: float = 0.0,
        lib: ctypes.CDLL | None = None,
    ):
        self.lib = lib or load_library()
        self.lib.polerf_water_new_u64.restype = ctypes.c_void_p
        self.lib.polerf_water_new_u64.argtypes = [
            ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint64,
            ctypes.c_double, ctypes.c_double, ctypes.c_double,
        ]
        self.lib.polerf_water_free.argtypes = [ctypes.c_void_p]
        self.lib.polerf_water_step.argtypes = [ctypes.c_void_p, ctypes.c_double]
        self.lib.polerf_water_frame.restype = ctypes.c_uint32
        self.lib.polerf_water_frame.argtypes = [
            ctypes.c_void_p, ctypes.c_double,
            ctypes.POINTER(ctypes.c_float), ctypes.c_uint32,
        ]
        self.lib.polerf_water_geometries_f32.restype = ctypes.c_uint32
        self.lib.polerf_water_geometries_f32.argtypes = [
            ctypes.c_void_p, ctypes.c_double,
            ctypes.POINTER(ctypes.c_float), ctypes.POINTER(ctypes.c_float),
            ctypes.c_uint32,
        ]
        self.lib.polerf_water_surface_at.restype = ctypes.c_double
        self.lib.polerf_water_surface_at.argtypes = [
            ctypes.c_void_p, ctypes.c_double, ctypes.c_double,
            ctypes.POINTER(ctypes.c_double), ctypes.POINTER(ctypes.c_double),
        ]
        self.lib.polerf_water_flow_at.restype = ctypes.c_double
        self.lib.polerf_water_flow_at.argtypes = [
            ctypes.c_void_p, ctypes.c_double, ctypes.c_double,
            ctypes.POINTER(ctypes.c_double), ctypes.POINTER(ctypes.c_double),
        ]
        self.lib.polerf_water_significant_height.restype = ctypes.c_double
        self.lib.polerf_water_significant_height.argtypes = [ctypes.c_void_p]
        self.lib.polerf_water_hash.restype = ctypes.c_uint64
        self.lib.polerf_water_hash.argtypes = [ctypes.c_void_p]

        self.n = n
        h = self.lib.polerf_water_new_u64(n, modes, seed, wind, domain, wind_dir)
        if not h:
            raise ValueError(
                "poler_panda.Water: невалидные параметры "
                f"(n={n} — степень двойки 16..1024, modes={modes} 1..16384)"
            )
        self._h = ctypes.c_void_p(h)
        self._grid = (ctypes.c_float * (n * n))()
        self._normals = (ctypes.c_float * (n * n * 3))()

    # --- жизненный цикл -------------------------------------------------
    def close(self):
        if getattr(self, "_h", None):
            self.lib.polerf_water_free(self._h)
            self._h = ctypes.c_void_p(0)

    def __del__(self):
        try:
            self.close()
        except Exception:
            pass

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()

    # --- физика ---------------------------------------------------------
    def step(self, dt: float):
        """Шаг моря (секунды)."""
        self.lib.polerf_water_step(self._h, dt)

    def frame(self, dt: float):
        """Шаг + сетка высот. Возвращает ctypes-массив f32 n×n (row-major)."""
        written = self.lib.polerf_water_frame(self._h, dt, self._grid, self.n * self.n)
        if written != self.n * self.n:
            raise RuntimeError(f"polerf_water_frame вернул {written}")
        return self._grid

    def geometries(self, dt: float):
        """Шаг + высоты + нормали ОДНИМ вызовом (путь рендер-цикла).

        Возвращает (heights, normals): heights — ctypes n×n f32;
        normals — ctypes n×n×3 f32 (nx, ny, nz), нормированные,
        из точных градиентов спектра (не из разностей сетки).
        """
        written = self.lib.polerf_water_geometries_f32(
            self._h, dt, self._grid, self._normals, self.n * self.n
        )
        if written != self.n * self.n:
            raise RuntimeError(f"polerf_water_geometries_f32 вернул {written}")
        return self._grid, self._normals

    @property
    def grid(self):
        """Последняя сетка высот (без шага)."""
        return self._grid

    def surface_at(self, x: float, y: float):
        """(h, dh/dx, dh/dy) — аналитические наклоны для нормалей."""
        dhx, dhy = ctypes.c_double(), ctypes.c_double()
        h = self.lib.polerf_water_surface_at(
            self._h, x, y, ctypes.byref(dhx), ctypes.byref(dhy)
        )
        return h, dhx.value, dhy.value

    def flow_at(self, x: float, y: float):
        """(u, v, w) — безвихревое течение в точке."""
        vx, vy = ctypes.c_double(), ctypes.c_double()
        w = self.lib.polerf_water_flow_at(
            self._h, x, y, ctypes.byref(vx), ctypes.byref(vy)
        )
        return vx.value, vy.value, w

    @property
    def significant_height(self) -> float:
        """H_s, м (Пирсон–Московиц)."""
        return self.lib.polerf_water_significant_height(self._h)

    @property
    def state_hash(self) -> int:
        """Бит-в-бит хэш состояния (детерминизм)."""
        return self.lib.polerf_water_hash(self._h)


def version() -> str:
    """Строка версии libpoler_ffi."""
    return load_library()._poler_version  # type: ignore[attr-defined]
