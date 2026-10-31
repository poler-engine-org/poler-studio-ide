#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
СЕССИЯ 6 — ИЗЛОМАННЫЕ ИЗМЕРЕНИЯ АЦИНА (CGLMP d=3, цель I₃ → 2.9249)
====================================================================
Теория: Acín et al. — теоретический максимум нарушения нелокальности
для кутритов I₃ = 2.924935 достигается НЕ равномерной фазовой лестницей
diag(1, e^{iφ}, e^{2iφ}), а ИЗЛОМАННЫМИ измерениями: индивидуальные
фазы α_{x,k}, β_{y,k} для каждого базисного состояния |k⟩ каждой
настройки (A1, A2, B1, B2). Плюс некомпактное состояние (c0,c1,c2).

Параметризация (совместима с Калькулятором движка, тест cglmp_...):
  psi  = (c0|00> + c1|11> + c2|22>)/N        — Шмидт, вещественные
  A1p  = diag(e^{i a1_0}, e^{i a1_1}, e^{i a1_2})   (стандарт: 1,1,1)
  A2p  = diag(...)                                   (стандарт: 0, π/3, 2π/3)
  B1p  = diag(...)                                   (стандарт: 0, π/6, π/3)
  B2p  = diag(...)                                   (стандарт: 0, −π/6, −π/3)
  FF   = F₃ ⊗ F₃†;  s_xy = FF · (A_x ⊗ B_y) · psi
  Классы c=(a−b) mod 3: Pc0 (совпадение), Pcp (+1), Pcm (−1)

I₃ (конвенция движка, якорь сессии 4 = 2.872934 = (4/9)(3+2√3)):
  I₃ = P0(11)+Pm(21)+P0(22)+P0(12) − Pm(11)−P0(21)−Pm(22)−Pp(12)

Этапы:
  1. ЯКОРЬ: стандартные фазы + psi Белла → 2.872934 (лит. значение)
  2. ЛОКАЛЬНЫЙ ПРЕДЕЛ: брутфорс 81 детерминированной стратегии → max = 2.0
  3. ОПТИМИЗАЦИЯ: дифференциальная эволюция (14 DOF) + Nelder-Mead полировка
"""
import json
import numpy as np
from scipy.optimize import differential_evolution, minimize

w3 = np.exp(2j * np.pi / 3)
F3 = np.array([[1, 1, 1],
               [1, w3, w3**2],
               [1, w3**2, w3]]) / np.sqrt(3)
FF = np.kron(F3, F3.conj().T)
I3E = np.eye(3)

# Классы c = (a−b) mod 3 в вычислительном базисе (после FF)
def class_op(kind):
    """kind: 0 (a=b), +1 (a−b=+1), −1 (a−b=−1≡2)"""
    P = np.zeros((9, 9), complex)
    for a in range(3):
        for b in range(3):
            if (a - b) % 3 == kind % 3:
                P[3*a + b, 3*a + b] = 1.0
    return P

PC0, PCP, PCM = class_op(0), class_op(1), class_op(-1)

def psi_schmidt(c):
    c = np.asarray(c, float)
    psi = np.zeros(9, complex)
    psi[0], psi[4], psi[8] = c[0], c[1], c[2]
    return psi / np.linalg.norm(psi)

def i3(c, ph, want_terms=False):
    """I₃ для состояния c (3 амплитуды) и фаз ph (4×3: A1,A2,B1,B2).
    Отрицательные амплитуды допустимы (знак = фаза π)."""
    psi = psi_schmidt(c)
    A1 = np.diag(np.exp(1j * ph[0]))
    A2 = np.diag(np.exp(1j * ph[1]))
    B1 = np.diag(np.exp(1j * ph[2]))
    B2 = np.diag(np.exp(1j * ph[3]))
    s11 = FF @ np.kron(I3E, B1) @ psi
    s12 = FF @ np.kron(I3E, B2) @ psi
    s21 = FF @ np.kron(A2, B1) @ psi
    s22 = FF @ np.kron(A2, B2) @ psi
    q = lambda s, P: float(np.abs(P @ s.conj()).sum() ** 0) if False else \
        float(np.real(np.vdot(P @ s, P @ s)))
    t = {
        'q0_11': q(s11, PC0), 'qm_11': q(s11, PCM),
        'qm_21': q(s21, PCM), 'q0_21': q(s21, PC0),
        'q0_22': q(s22, PC0), 'qm_22': q(s22, PCM),
        'q0_12': q(s12, PC0), 'qp_12': q(s12, PCP),
    }
    val = (t['q0_11'] + t['qm_21'] + t['q0_22'] + t['q0_12']
           - t['qm_11'] - t['q0_21'] - t['qm_22'] - t['qp_12'])
    return (val, t) if want_terms else val

# ── 1. ЯКОРЬ: стандартная настройка CGLMP ────────────────────────────────
STD_PH = np.array([
    [0.0, 0.0, 0.0],                 # A1 (тождественная)
    [0.0, np.pi/3, 2*np.pi/3],       # A2
    [0.0, np.pi/6, np.pi/3],         # B1
    [0.0, -np.pi/6, -np.pi/3],       # B2
])
anchor, t_anchor = i3([1, 1, 1], STD_PH, want_terms=True)
lit = (4.0/9.0) * (3.0 + 2.0*np.sqrt(3.0))
print(f"[ЯКОРЬ] стандарт CGLMP: I₃ = {anchor:.6f}  (лит. {lit:.6f}, "
      f"Δ = {abs(anchor-lit):.2e})")
assert abs(anchor - lit) < 1e-9, "якорь не сходится — модель сломана"
print(f"        q0 = {t_anchor['q0_11']:.6f} (лит. (4+2√3)/9), "
      f"q₋ = {t_anchor['qm_11']:.6f} (лит. 1/9)")

# ── 2. ЛОКАЛЬНЫЙ ПРЕДЕЛ: брутфорс 81 стратегии (для ЭТОЙ конвенции) ─────
def local_bound_bruteforce():
    """Детерминированная стратегия: a1,a2,b1,b2 ∈ {0,1,2} → 3⁴ = 81.
    Вероятности вырождены: P(c|xy) = 1 если c = (a_x − b_y) mod 3."""
    best = -np.inf
    for a1 in range(3):
        for a2 in range(3):
            for b1 in range(3):
                for b2 in range(3):
                    q0_11 = 1.0 if (a1 - b1) % 3 == 0 else 0.0
                    qm_11 = 1.0 if (a1 - b1) % 3 == 2 else 0.0
                    qm_21 = 1.0 if (a2 - b1) % 3 == 2 else 0.0
                    q0_21 = 1.0 if (a2 - b1) % 3 == 0 else 0.0
                    q0_22 = 1.0 if (a2 - b2) % 3 == 0 else 0.0
                    qm_22 = 1.0 if (a2 - b2) % 3 == 2 else 0.0
                    q0_12 = 1.0 if (a1 - b2) % 3 == 0 else 0.0
                    qp_12 = 1.0 if (a1 - b2) % 3 == 1 else 0.0
                    v = (q0_11 + qm_21 + q0_22 + q0_12
                         - qm_11 - q0_21 - qm_22 - qp_12)
                    best = max(best, v)
    return best

LB = local_bound_bruteforce()
print(f"[ЛОКАЛЬНЫЙ ПРЕДЕЛ] брутфорс 81 стратегии: max I₃ = {LB:.1f}  "
      f"(локальный реализм ≤ 2 ✓)")

# ── 3. ОПТИМИЗАЦИЯ: 3 амплитуды + 12 фаз (глобальные фазы = калибровка) ──
# Знак амплитуды ≡ фаза π, поэтому c ∈ ℝ³ свободно; фазы A-стороны
# поглощают фазы Шмидта (диагональные фазы коммутируют с семейством).
def unpack(x):
    return x[:3], x[3:].reshape(4, 3)

def neg(x):
    c, ph = unpack(x)
    return -i3(c, ph)

bounds = [(-1.5, 1.5)] * 3 + [(-np.pi, np.pi)] * 12
DE = differential_evolution(
    neg, bounds, seed=42, maxiter=400, popsize=40, tol=1e-12,
    mutation=(0.3, 1.0), recombination=0.9, polish=True,
    init='sobol', updating='deferred', workers=-1,
)
c0, ph0 = unpack(DE.x)
v0 = -DE.fun
print(f"\n[DE]        I₃ = {v0:.6f}  (состояние c = {np.round(c0,4)})")

# Двухсторонняя полировка Nelder-Mead (фазы ф mod 2π)
res = minimize(neg, DE.x, method='Nelder-Mead',
               options={'xatol': 1e-13, 'fatol': 1e-13, 'maxiter': 40000,
                        'maxfev': 40000})
c1, ph1 = unpack(res.x)
v1 = -res.fun
print(f"[NM]        I₃ = {v1:.6f}  (состояние c = {np.round(c1,4)})")

# Зеркальный запуск из «литературной» догадки: (1, γ, γ), γ≈0.79
guess = np.concatenate([[1.0, 0.7924, 0.7924], STD_PH.flatten()])
guess[3:6] = [0.0, 0.25, 0.55]   # слегка изломанный A2 для затравки
res2 = minimize(neg, guess, method='Nelder-Mead',
                options={'xatol': 1e-13, 'fatol': 1e-13, 'maxiter': 40000,
                         'maxfev': 40000})
c2, ph2 = unpack(res2.x)
v2 = -res2.fun
print(f"[NM-затравка] I₃ = {v2:.6f}  (состояние c = {np.round(c2,4)})")

# Лучший результат
if v1 >= v2:
    best_c, best_ph, best_v = c1, ph1, v1
else:
    best_c, best_ph, best_v = c2, ph2, v2

# Нормализация отчёта: фиксируем c0 > 0, c1,c2 ≥ 0 (фазы → измерения),
# фазы приводим к (−π, π], первую фазу каждой строки к 0 — НО это
# калибровка только ДЛЯ ОТЧЁТА; движок получит сырые значения.
def report(c, ph):
    c = np.asarray(c, float)
    m = np.max(np.abs(c))
    c = c / m
    ph = np.mod(np.asarray(ph), 2*np.pi)
    ph[ph > np.pi] -= 2*np.pi
    return c, ph

rc, rph = report(best_c, best_ph)
val_fin, t_fin = i3(rc, rph, want_terms=True)
print(f"\n{'═'*66}")
print(f"[ОПТИМУМ]   I₃ = {val_fin:.6f}"
      f"   (цель литературы: 2.924935, Δ = {abs(val_fin-2.924935):.2e})")
print(f"           превышение над компактным Беллом 2.872935: "
      f"+{val_fin - lit:.6f} (+{100*(val_fin-lit)/lit:.2f}%)")
print(f"           нарушение локального реализма: {val_fin/LB:.4f}×")
print(f"  Состояние Шмидта: ({rc[0]:.6f})|00⟩ + ({rc[1]:.6f})|11⟩ + "
      f"({rc[2]:.6f})|22⟩  (норма на max)")
print("  Изломанные фазы (радианы):")
for i, nm in enumerate(['A1', 'A2', 'B1', 'B2']):
    row = '  '.join(f"{rph[i,j]:+9.6f}" for j in range(3))
    print(f"    {nm}: [{row}]   (шаги: {', '.join(f'{np.diff(np.sort(rph[i]))[k]:+.4f}' for k in range(2))})")
print(f"  Вероятности: q0(11)={t_fin['q0_11']:.6f} q₋(11)={t_fin['qm_11']:.6f} "
      f"q₋(21)={t_fin['qm_21']:.6f} q0(21)={t_fin['q0_21']:.6f}")
print(f"               q0(22)={t_fin['q0_22']:.6f} q₋(22)={t_fin['qm_22']:.6f} "
      f"q0(12)={t_fin['q0_12']:.6f} q₊(12)={t_fin['qp_12']:.6f}")

out = {
    'anchor_std': anchor,
    'local_bound': LB,
    'i3_opt': val_fin,
    'c': rc.tolist(),
    'phases': rph.tolist(),
    'terms': {k: float(v) for k, v in t_fin.items()},
}
with open('/home/z/my-project/scripts/acin_broken_opt.json', 'w') as f:
    json.dump(out, f, indent=2)
print("\n[сохранено] scripts/acin_broken_opt.json")
