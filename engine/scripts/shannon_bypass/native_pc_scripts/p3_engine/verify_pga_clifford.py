#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Category 5 Verification (v2, аудит 2026-09-21 — повністю переписаний):
  - Projective Geometric Algebra (PGA P³ ~ Cl(3,0,1)) через бібліотеку clifford
  - Ротор обертання навколо осі Z на 90°: точка (1,0,0) → (0,1,0) —
    ПЕРЕВІРЯЄТЬСЯ за коефіцієнтами мультивектора (5 реальних перевірок).

Знахідка аудиту (чому переписаний):
  1. Оригінальний assert `abs(angle - math.pi/2) < 1e-6` перевіряв, що
     π/2 == π/2 — vacuous; результат перетворення ніколи не згадувався.
  2. Бібліотека clifford у Cl(3,0,1) робить NULL-базисом e1 (метрика
     diag[0,1,1,1]), а не e4. Отже ротор оригінала на лезі e12 —
     вироджена нуль-площина (e12² = 0): він нічого не обертає,
     і «точка» e123 + x·e234 + ... мала невірну структуру.

Правильна конвенція (перевірена числово):
  null-базис: e1 (= PGA e0); євклідові: e2→x, e3→y, e4→z.
  Точка:      P(x,y,z) = e234 − x·e134 + y·e124 − z·e123
  Ротор (навколо z на кут θ, площина e23): R = cos(θ/2) − sin(θ/2)·e23
"""
import math
import sys

import numpy as np

try:
    import clifford as cf
except ImportError:
    print("Бібліотека clifford не встановлена: pip install clifford")
    sys.exit(2)

print("============================================================")
print("1. Clifford Projective Geometric Algebra Cl(3,0,1) — PGA P³")
print("   (null-базис e1; євклідові e2,e3,e4; ротор у площині e23)")
print("============================================================")

layout, blades = cf.Cl(3, 0, 1)
E123 = blades["e123"]
E234 = blades["e234"]
E134 = blades["e134"]
E124 = blades["e124"]
E23 = blades["e23"]


def bidx(blade) -> int:
    """Індекс коефіцієнта леза у векторі значень мультивектора."""
    return int(np.argmax(np.abs(blade.value)))


I234, I134, I124, I123 = bidx(E234), bidx(E134), bidx(E124), bidx(E123)


def pga_point(x: float, y: float, z: float):
    """Однорідна PGA-точка: P = e234 − x·e134 + y·e124 − z·e123."""
    return E234 - x * E134 + y * E124 - z * E123


def point_coords(P):
    """(x, y, z) з нормуванням на лідо e234 (однорідна координата)."""
    v = P.value
    w = float(v[I234])
    if abs(w) < 1e-12:
        return None
    return (-float(v[I134]) / w, float(v[I124]) / w, -float(v[I123]) / w)


def grade_indices():
    return {int(i): int(layout.gradeList[i]) for i in range(len(layout.gradeList))}


# --- Ротор: обертання навколо осі Z (e4) на 90° у площині e23 ---
theta = math.pi / 2.0
rotor = math.cos(theta / 2.0) - math.sin(theta / 2.0) * E23
_rn = rotor * ~rotor  # унітарність з FP-допуском
rotor_unitary = abs(float(_rn.value[0]) - 1.0) < 1e-12 and \
    float(np.max(np.abs(_rn.value[1:]))) < 1e-12

pt = pga_point(1.0, 0.0, 0.0)
transformed = rotor * pt * ~rotor
coords = point_coords(transformed)

print(f"\nТочка (1, 0, 0), обернута ротором e23 на 90° навколо e4 (z):")
print(f"  декартові координати: ({coords[0]:+.2e}, {coords[1]:+.6f}, {coords[2]:+.2e})")
print(f"  норма ротора R·R̃ = 1: {'ТАК' if rotor_unitary else 'НІ'}")

# ===== РЕАЛЬНІ ПЕРЕВІРКИ =====
checks = [
    ("ротор унітарний (R·R̃ = 1, площина e23 не нульова)", rotor_unitary),
    ("точка (1,0,0) → (0,1,0) після повороту +90° навколо z",
     abs(coords[0]) < 1e-9 and abs(coords[1] - 1.0) < 1e-9 and abs(coords[2]) < 1e-9),
]

# 2. Зворотний ротор повертає точку назад
back = (~rotor) * transformed * rotor
cb = point_coords(back)
checks.append(("зворотний ротор R̃ відкатує перетворення: (0,1,0) → (1,0,0)",
               abs(cb[0] - 1.0) < 1e-9 and abs(cb[1]) < 1e-9 and abs(cb[2]) < 1e-9))

# 3. Знак ротора (2:1 накриття Spin→SO): (−R) дає ту саму дію
pt2 = (-rotor) * pga_point(1.0, 0.0, 0.0) * ~(-rotor)
c2 = point_coords(pt2)
checks.append(("(−R)·P·(−R̃) ≡ R·P·R̃ (подвійне накриття Spin(3)→SO(3))",
               abs(c2[0] - coords[0]) < 1e-12 and abs(c2[1] - coords[1]) < 1e-12))

# 4. Кут 0 — тотожність
idr = 1.0 - 0.0 * E23
c3 = point_coords(idr * pga_point(1.0, 0.0, 0.0) * ~idr)
checks.append(("кут 0: ротор тотожності зберігає точку",
               abs(c3[0] - 1.0) < 1e-12 and abs(c3[1]) < 1e-12 and abs(c3[2]) < 1e-12))

# 5. Ґрейд-чистота: сендвіч зберігає ґрейд точки (лише 3-вектори)
gidx = grade_indices()
coeffs = {i: float(transformed.value[i]) for i in range(len(transformed.value))
          if abs(transformed.value[i]) > 1e-12}
checks.append(("ґрейд точки (3) зберігається — без домішок скалярів/бівекторів",
               all(gidx[i] == 3 for i in coeffs)))

# 6. Однорідна координата e234 незмінна (=1)
checks.append(("однорідна координата e234 незмінна (=1)",
               abs(float(transformed.value[I234]) - 1.0) < 1e-12))

print()
all_ok = True
for name, cond in checks:
    print(f"  [{'OK' if cond else 'FAIL'}] {name}")
    all_ok = all_ok and cond

print()
print("Clifford PGA P³ rotor sandwich verification:",
      "PASS ✅" if all_ok else "FAIL ❌")
sys.exit(0 if all_ok else 1)
