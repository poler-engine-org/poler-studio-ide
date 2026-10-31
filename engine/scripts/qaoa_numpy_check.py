#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Независимая numpy-верификация QAOA-чисел капсулы (истинный граф K4-ядра).
Нейрон j = трит j СТАРШИЙ (x // 3^(5-j)) % 3 — грабли сессии 5 (kron arg1).
Ansatz: |ψ⟩ = M(β)·C(γ)·|flat⟩ (как в cost_expr/mixer_expr движка):
  C(γ): по каждому ребру e=(u,v,w): компоненты с x_u=x_v БЕЗ фазы,
        остальные × e^{iγw}  (exp(γwi)·base + (1−exp(γwi))·EQU·base)
  M(β): ⨂₆ expm(−iβG3), G3 = X3 + X3†
E[cut] = Σ_e w_e·(1 − P(x_u = x_v)).
"""
import numpy as np
from scipy.linalg import expm

EDGES = [(0,1,735/1496),(0,2,24/1496),(0,4,1.0),(0,5,1222/1496),
         (1,2,251/1496),(1,3,231/1496),(1,4,206/1496),(1,5,194/1496),
         (2,4,40/1496),(2,5,28/1496),(4,5,8/1496)]

X3 = np.array([[0,0,1],[1,0,0],[0,1,0]], complex)
G3 = X3 + X3.conj().T

def digits(x):
    """x ∈ [0,729) → [d0..d5], dj = трит нейрона j (старший — j=0)."""
    d = []
    for j in range(5, -1, -1):
        d.append((x // 3**j) % 3)
    return d  # d[0] = самый старший = нейрон 0

# предвычислим таблицу цифр
DIG = np.array([digits(x) for x in range(729)])  # 729×6

def qaoa_state(layers):
    """layers = [(γ1,β1),...] — |ψ⟩ = M·C·…·M·C·flat."""
    psi = np.ones(729, complex) / np.sqrt(729)
    for g, b in layers:
        # cost
        psi = np.exp(1j*g*0) * psi  # фаза не-EQU компонент применяется посложно
        # посрочно: exp(gw i) на не-EQU, 1 на EQU → эквивалент: общий exp(gw i)
        # с EQU-компонентами, делёнными обратно... делаем честно по рёбрам:
        for (u, v, w) in EDGES:
            eq = DIG[:, u] == DIG[:, v]
            ph = np.exp(1j * g * w)
            psi = ph * psi + (1 - ph) * np.where(eq, psi, 0)
        # mixer
        M = expm(-1j * b * G3)
        MIX = np.array([[1]], complex)
        for _ in range(6):
            MIX = np.kron(MIX, M)
        psi = MIX @ psi
    return psi

def ecut(psi):
    e = 0.0
    for (u, v, w) in EDGES:
        peq = float(np.sum(np.abs(psi[DIG[:, u] == DIG[:, v]])**2))
        e += w * (1 - peq)
    return e

if __name__ == '__main__':
    print("── numpy-верификация QAOA капсулы (истинный граф) ──")
    p1 = qaoa_state([(0.9, 0.9)])
    print(f"p=1 (0.9,0.9):            E[cut] = {ecut(p1):.7f}  "
          f"({ecut(p1)*1496:.1f} син. из 4435)")
    p2 = qaoa_state([(0.9, 0.9), (1.8, 2.0)])
    print(f"p=2 (0.9,.9)(1.8,2.0):    E[cut] = {ecut(p2):.7f}  "
          f"({ecut(p2)*1496:.1f} син.)")
    p3 = qaoa_state([(0.3, 0.6), (1.5, 2.0), (0.3, 2.0)])
    print(f"p=3 (.3,.6)(1.5,2)(.3,2): E[cut] = {ecut(p3):.7f}  "
          f"({ecut(p3)*1496:.1f} син.)")
    print(f"Σw = {sum(w for _,_,w in EDGES):.7f} (4435/1496); "
          f"оптимум 4403/4435 = 0.992785")
