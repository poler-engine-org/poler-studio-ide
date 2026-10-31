#!/usr/bin/env python3
"""smoke_headless.py — бездисплейная проверка кадрового пути демо океана.

Валидирует БЕЗ видеокарты (окно не открывается):
  1. GeomVertexData V3n3 + GeomTriangles — numpy → setData (путь демо);
  2. один вызов C-ABI на кадр: geometries() → высоты+нормали;
  3. GLSL-шейдер демо парсится Shader.make (компиляция отложена до GPU);
  4. производительность кадрового пути.

Полный рендер с оптикой — на машине с GPU: python3 demo_ocean.py
"""
import os
import sys
import time

from panda3d.core import loadPrcFileData

loadPrcFileData("", "window-type none")
loadPrcFileData("", "audio-library-name null")

import importlib.util

import numpy as np
from panda3d.core import (
    Geom,
    GeomNode,
    GeomTriangles,
    GeomVertexData,
    GeomVertexFormat,
    Shader,
)

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from poler_panda import Water, version

GRID = 128
DOMAIN = 140.0
CELL = DOMAIN / (GRID - 1)

print(f"POLER FFI : {version()}")

# --- 1. море + один вызов геометрии ---
sea = Water(n=GRID, modes=160, seed=42, wind=8.0, domain=DOMAIN)
heights, normals = sea.geometries(1.0 / 60.0)
print(f"море      : {GRID}×{GRID}, H_s = {sea.significant_height:.3f} м")

# --- 2. геометрия Panda3D (путь demo_ocean.py) ---
fmt = GeomVertexFormat.getV3n3()
vdata = GeomVertexData("poler-ocean", fmt, Geom.UHDynamic)
vdata.setNumRows(GRID * GRID)
idx = np.arange(GRID * GRID, dtype=np.float32)
geom_buf = np.zeros((GRID * GRID, 6), dtype=np.float32)
geom_buf[:, 0] = (idx % GRID) * CELL - DOMAIN / 2
geom_buf[:, 1] = (idx // GRID) * CELL - DOMAIN / 2
vdata.modifyArray(0).modifyHandle().setData(geom_buf.tobytes())

tris = GeomTriangles(Geom.UHStatic)
quads = np.arange(GRID * GRID, dtype=np.uint32).reshape(GRID, GRID)[:-1, :-1]
a = quads.reshape(-1)
b, c, d = a + 1, a + GRID, a + GRID + 1
tri_idx = np.empty((a.size, 6), dtype=np.uint32)
tri_idx[:, 0], tri_idx[:, 1], tri_idx[:, 2] = a, c, b
tri_idx[:, 3], tri_idx[:, 4], tri_idx[:, 5] = b, c, d
tris.setIndexType(Geom.NTUint32)
tris.addNextVertices(a.size * 2)
tris.modifyVertices().modifyHandle().setData(tri_idx.tobytes())
geom = Geom(vdata)
geom.addPrimitive(tris)
node = GeomNode("poler-ocean")
node.addGeom(geom)
n_prims = node.getNumGeoms()
print(f"геометрия : {vdata.getNumRows()} вершин, {n_prims} Geom, "
      f"{tris.getNumVertices()} индексов")
assert vdata.getNumRows() == GRID * GRID
assert tris.getNumVertices() == (GRID - 1) * (GRID - 1) * 6

demo_path = os.path.join(os.path.dirname(__file__), "demo_ocean.py")
spec = importlib.util.spec_from_file_location("demo_ocean", demo_path)
demo = importlib.util.module_from_spec(spec)
sys.modules["demo_ocean"] = demo
spec.loader.exec_module(demo)
shader = Shader.make(Shader.SL_GLSL, vertex=demo.WATER_VERTEX, fragment=demo.WATER_FRAGMENT)
assert shader is not None
for marker in ("fres", "foam", "spec", "fog"):
    assert marker in demo.WATER_FRAGMENT, f"нет {marker} в шейдере"
print("шейдер   : Shader.make OK, оптика на месте (Френель/пена/блик/туман)")

# --- 4. кадровый цикл: физика + memcpy ---
t0 = time.perf_counter()
FRAMES = 240
for _ in range(FRAMES):
    heights, normals = sea.geometries(1.0 / 60.0)
    geom_buf[:, 2] = np.frombuffer(heights, dtype=np.float32)
    geom_buf[:, 3:] = np.frombuffer(normals, dtype=np.float32).reshape(GRID * GRID, 3)
    vdata.modifyArray(0).modifyHandle().setData(geom_buf.tobytes())
dt = time.perf_counter() - t0
fps = FRAMES / dt
print(f"кадры    : {FRAMES} кадров за {dt*1000:.0f} мс → {fps:.0f} кадров/с")
assert fps > 30, "кадровый путь медленнее 30 FPS"

zmin, zmax = geom_buf[:, 2].min(), geom_buf[:, 2].max()
nlen = np.sqrt((geom_buf[:, 3:6] ** 2).sum(axis=1))
print(f"волны    : z ∈ [{zmin:.2f}, {zmax:.2f}] м; |N| ∈ [{nlen.min():.4f}, {nlen.max():.4f}]")
assert zmax > zmin
assert abs(nlen.mean() - 1.0) < 1e-3, "нормали не единичные"

sea.close()
print("\nSMOKE OK: кадровый путь демо валиден (рендер — на GPU-машине)")
sys.exit(0)
