#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
СЕССИЯ 6: КАРТА ВСЕХ CGLMP-ПОДОБНЫХ НЕРАВЕНСТВ d=3.
Перебор 6⁴ = 1296 паттернов (для каждой из 4 настроек — упорядоченная пара
(положительный класс, отрицательный класс) из {0,+1,−1}).
Фильтры валидности:
  F1: локальный предел (брутфорс 81 стратегии) = РОВНО 2
  F2: значение на стандартной точке (Белл + фазовые лестницы CGLMP)
      = 2.872934 = (4/9)(3+2√3) — литературный якорь
Выжившие → быстрая DE-оптимизация диагонального семейства → кто даёт 2.9249?
"""
import itertools
import numpy as np
from scipy.optimize import differential_evolution, minimize

w3 = np.exp(2j * np.pi / 3)
F3 = np.array([[1, 1, 1], [1, w3, w3**2], [1, w3**2, w3]]) / np.sqrt(3)
FF = np.kron(F3, F3.conj().T)
LIT = (4.0/9.0)*(3.0 + 2.0*np.sqrt(3.0))

def q(s, kind):
    v = 0.0
    for a in range(3):
        for b in range(3):
            if (a - b) % 3 == kind % 3:
                v += abs(s[3*a + b])**2
    return v

def all_probs(psi, U1, U2, V1, V2):
    """12 вероятностей: q[setting][class], setting∈{11,12,21,22}, class∈{0,1,2(≡−1)}."""
    out = {}
    for nm, (U, V) in {'11': (U1, V1), '12': (U1, V2),
                       '21': (U2, V1), '22': (U2, V2)}.items():
        s = FF @ np.kron(U, V) @ psi
        out[nm] = (q(s, 0), q(s, 1), q(s, 2))
    return out

# стандартная точка
psi_std = np.zeros(9, complex); psi_std[0] = psi_std[4] = psi_std[8] = 1/np.sqrt(3)
A2 = np.diag([1, np.exp(1j*np.pi/3), np.exp(2j*np.pi/3)])
B1 = np.diag([1, np.exp(1j*np.pi/6), np.exp(1j*np.pi/3)])
B2 = np.diag([1, np.exp(-1j*np.pi/6), np.exp(-1j*np.pi/3)])
STD = all_probs(psi_std, np.eye(3), A2, B1, B2)

CLASSES = (0, 1, 2)  # 0, +1, −1(≡2)
PAIRS = [(p, n) for p in CLASSES for n in CLASSES if p != n]  # 6 упорядоченных

def expr_value(probs, pattern):
    """pattern: кортеж 4 пар (pos,neg) для настроек 11,12,21,22."""
    order = ['11', '12', '21', '22']
    v = 0.0
    for nm, (p, n) in zip(order, pattern):
        v += probs[nm][p] - probs[nm][n]
    return v

def local_bound(pattern):
    best = -np.inf
    for a1 in range(3):
        for a2 in range(3):
            for b1 in range(3):
                for b2 in range(3):
                    det = {'11': (a1, b1), '12': (a1, b2),
                           '21': (a2, b1), '22': (a2, b2)}
                    pr = {nm: tuple(1.0 if (a - b) % 3 == c else 0.0 for c in CLASSES)
                          for nm, (a, b) in det.items()}
                    best = max(best, expr_value(pr, pattern))
    return best

# ── перебор с фильтрами ──────────────────────────────────────────────────
print("Перебор 1296 паттернов: локальный предел=2 И стандарт=2.872934 …")
survivors = []
for pattern in itertools.product(PAIRS, repeat=4):
    lb = local_bound(pattern)
    if abs(lb - 2.0) > 1e-12:
        continue
    sv = expr_value(STD, pattern)
    if abs(sv - LIT) > 1e-9:
        continue
    survivors.append((pattern, lb, sv))

print(f"Выживших после F1+F2: {len(survivors)}")
for i, (pat, lb, sv) in enumerate(survivors):
    desc = '  '.join(f"{nm}:{['0','+','−'][p]}−{['0','+','−'][n]}"
                     for nm, (p, n) in zip(['11','12','21','22'], pat))
    print(f"  #{i}: {desc}")

# ── быстрая DE-оптимизация выживших (диагональное семейство) ─────────────
def i3_diag(x, pattern):
    c = x[:3]
    ph = x[3:].reshape(4, 3)
    psi = np.zeros(9, complex)
    psi[0], psi[4], psi[8] = c[0], c[1], c[2]
    n = np.linalg.norm(psi)
    if n == 0:
        return -99.0
    psi /= n
    U1 = np.diag(np.exp(1j*ph[0])); U2 = np.diag(np.exp(1j*ph[1]))
    V1 = np.diag(np.exp(1j*ph[2])); V2 = np.diag(np.exp(1j*ph[3]))
    return expr_value(all_probs(psi, U1, U2, V1, V2), pattern)

print("\nDE-оптимизация выживших (диагональное семейство, 6 сидов):")
results = []
for i, (pat, lb, sv) in enumerate(survivors):
    best_v, best_x = -np.inf, None
    for seed in range(6):
        de = differential_evolution(
            lambda x, pat=pat: -i3_diag(x, pat),
            [(-1.5, 1.5)]*3 + [(-np.pi, np.pi)]*12,
            seed=seed, maxiter=180, popsize=28, tol=1e-12,
            mutation=(0.3, 1.0), recombination=0.9, polish=True,
            init='sobol', updating='immediate')
        if -de.fun > best_v:
            best_v, best_x = -de.fun, de.x
    nm = minimize(lambda x, pat=pat: -i3_diag(x, pat), best_x,
                  method='Nelder-Mead',
                  options={'xatol': 1e-13, 'fatol': 1e-13,
                           'maxiter': 40000, 'maxfev': 40000})
    if -nm.fun > best_v:
        best_v, best_x = -nm.fun, nm.x
    results.append((best_v, best_x, pat))
    print(f"  #{i}: max I₃ = {best_v:.6f}   "
          f"(до 2.924935: {2.924935 - best_v:+.6f})")

results.sort(key=lambda r: -r[0])
print(f"\nЛУЧШИЙ паттерн: I₃ = {results[0][0]:.6f}")
pat = results[0][2]
desc = '  '.join(f"{nm}:{['0','+','−'][p]}−{['0','+','−'][n]}"
                 for nm, (p, n) in zip(['11','12','21','22'], pat))
print(f"  {desc}")
x = results[0][1]
print(f"  состояние c = ({x[0]:.4f}, {x[1]:.4f}, {x[2]:.4f})")
print(f"  фазы: A1={np.round(x[3:6],4)} A2={np.round(x[6:9],4)} "
      f"B1={np.round(x[9:12],4)} B2={np.round(x[12:15],4)}")

import json
json.dump({'pattern': [[int(p), int(n)] for p, n in pat],
           'I3': results[0][0], 'c': x[:3].tolist(),
           'phases': x[3:].reshape(4,3).tolist()},
          open('/home/z/my-project/scripts/acin_best_pattern.json', 'w'), indent=2)
print("\n[сохранено] acin_best_pattern.json")
