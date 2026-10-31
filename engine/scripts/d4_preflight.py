#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Сессия 7 — ПРЕДПОЛЁТ d=4 (CGLMP на двух квквартах), numpy-кроссчек.

Методология Сессии 6: прежде чем считать на Калькуляторе движка,
устанавливаем эталонные числа numpy:
  1) Локальный предел I4 по ВСЕМ 256 детерминированным стратегиям = 2
  2) Максимально-запутанное состояние, фазы статьи Collins et al.:
     I4 = (2/3)(sqrt(2)+sqrt(10-sqrt(2))) = 2.896244677  (аналитика)
     + вероятности q_c = 1/(2 d^2 sin^2(pi(c+1/4)/d))
  3) DE по 12 DOF (3 амплитуды + 9 фаз) -> некомпактный оптимум d=4
  4) Оператор Белла B4 (16x16): lambda_max == DE-результат
  5) Шумовой порог eta* = 2/I4*
"""
import numpy as np

rng = np.random.default_rng(20261004)
d = 4
j = np.arange(d)

# ── операторы ────────────────────────────────────────────────────────────
F4 = np.exp(2j * np.pi * np.outer(j, j) / d) / 2.0          # QFT квкварта
F4dag = F4.conj().T
FF = np.kron(F4, F4dag)                                     # F ⊗ F† (грабли сессии 4)

def diagp(ph):
    return np.diag(np.exp(1j * np.asarray(ph, dtype=float)))

# проекторы классов c = (A - B) mod 4 в 16-мерии; kron: Алиса = старший
Pc = {}
for c in range(d):
    P = np.zeros((16, 16))
    for a in range(d):
        for b in range(d):
            if (a - b) % d == c:
                P[4 * a + b, 4 * a + b] = 1.0
    Pc[c] = P

def kron(A, B):
    return np.kron(A, B)

def M(A, B):
    return kron(A, B)

# ── контур I4 по формуле статьи (Id), конвенция движка ──────────────────
# k=0 (w=1):   +[P11(0)+P21(3)+P22(0)+P12(0)] - [P11(3)+P21(0)+P22(3)+P12(1)]
# k=1 (w=1/3): +[P11(1)+P21(2)+P22(1)+P12(3)] - [P11(2)+P21(1)+P22(2)+P12(2)]
def I4_value(psi, A2, B1, B2):
    s11 = FF @ M(np.eye(d), B1) @ psi
    s12 = FF @ M(np.eye(d), B2) @ psi
    s21 = FF @ M(A2, B1) @ psi
    s22 = FF @ M(A2, B2) @ psi
    def P(c, s):
        return float(np.real(np.vdot(s, Pc[c] @ s)))
    v = (P(0, s11) + P(3, s21) + P(0, s22) + P(0, s12)
         - P(3, s11) - P(0, s21) - P(3, s22) - P(1, s12))
    v += (1.0 / 3.0) * (P(1, s11) + P(2, s21) + P(1, s22) + P(3, s12)
                        - P(2, s11) - P(1, s21) - P(2, s22) - P(2, s12))
    return v

def bell_operator(A2, B1, B2):
    """B4 = сумма +/- w * M† FF† Pc FF M  (эрмитов 16x16)."""
    mats = [(np.eye(d), B1, 0, +1.0), (np.eye(d), B2, 0, +1.0),
            (A2, B1, 0, +1.0), (A2, B2, 0, +1.0)]
    # аккуратно собираем по членам формулы
    terms = []
    def add(A, B, c, sgn, w):
        Mi = M(A, B)
        terms.append(sgn * w * (Mi.conj().T @ FF.conj().T @ Pc[c] @ FF @ Mi))
    # k=0
    for (A, B) in [(np.eye(d), B1), (A2, B1), (A2, B2), (np.eye(d), B2)]:
        pass
    add(np.eye(d), B1, 0, +1, 1.0); add(A2, B1, 3, +1, 1.0)
    add(A2, B2, 0, +1, 1.0);       add(np.eye(d), B2, 0, +1, 1.0)
    add(np.eye(d), B1, 3, -1, 1.0); add(A2, B1, 0, -1, 1.0)
    add(A2, B2, 3, -1, 1.0);       add(np.eye(d), B2, 1, -1, 1.0)
    # k=1, w=1/3
    w = 1.0 / 3.0
    add(np.eye(d), B1, 1, +1, w); add(A2, B1, 2, +1, w)
    add(A2, B2, 1, +1, w);       add(np.eye(d), B2, 3, +1, w)
    add(np.eye(d), B1, 2, -1, w); add(A2, B1, 1, -1, w)
    add(A2, B2, 2, -1, w);       add(np.eye(d), B2, 2, -1, w)
    Bop = np.zeros((16, 16), dtype=complex)
    for t in terms:
        Bop += t
    return Bop

# ── 1) ЛОКАЛЬНЫЙ ПРЕДЕЛ: брутфорс 256 детерминированных стратегий ───────
def I4_det(a1, a2, b1, b2):
    """Детерминированные вероятности: P_ij(c) = [a_i - b_j == c mod 4]."""
    def Pij(ai, bj, c):
        return 1.0 if (ai - bj) % d == c else 0.0
    P11 = lambda c: Pij(a1, b1, c)
    P12 = lambda c: Pij(a1, b2, c)
    P21 = lambda c: Pij(a2, b1, c)
    P22 = lambda c: Pij(a2, b2, c)
    v = (P11(0) + P21(3) + P22(0) + P12(0) - P11(3) - P21(0) - P22(3) - P12(1))
    v += (1.0 / 3.0) * (P11(1) + P21(2) + P22(1) + P12(3)
                        - P11(2) - P21(1) - P22(2) - P12(2))
    return v

best = -10.0
for a1 in range(d):
    for a2 in range(d):
        for b1 in range(d):
            for b2 in range(d):
                best = max(best, I4_det(a1, a2, b1, b2))
print(f"[1] ЛОКАЛЬНЫЙ ПРЕДЕЛ (брутфорс 256 стратегий): max I4 = {best!r}")
assert abs(best - 2.0) < 1e-12, "локальный предел должен быть ровно 2!"

# ── 2) МАКСИМАЛЬНО ЗАПУТАННОЕ СОСТОЯНИЕ + фазы статьи ───────────────────
A2p = diagp(np.pi * j / 4.0)        # alpha2 = 1/2: phi = 2pi/4 * j/2
B1p = diagp(np.pi * j / 8.0)        # beta1  = 1/4: phi = 2pi/4 * j/4
B2p = diagp(-np.pi * j / 8.0)       # beta2  = -1/4
psiB = np.zeros(16, dtype=complex)
for k in range(d):
    psiB[4 * k + k] = 1.0
psiB /= 2.0

i4_me = I4_value(psiB, A2p, B1p, B2p)
i4_lit = (2.0 / 3.0) * (np.sqrt(2) + np.sqrt(10 - np.sqrt(2)))
print(f"[2] I4(max-ent, фазы статьи) = {i4_me:.10f}")
print(f"    литература (2/3)(√2+√(10−√2)) = {i4_lit:.10f}")
assert abs(i4_me - i4_lit) < 1e-9, "max-ent не совпал со статьёй!"

# аналитические q_c
qc = lambda c: 1.0 / (2 * d * d * np.sin(np.pi * (c + 0.25) / d) ** 2)
s11 = FF @ M(np.eye(d), B1p) @ psiB
q0_eng = float(np.real(np.vdot(s11, Pc[0] @ s11)))
print(f"    q0 движок = {q0_eng:.10f};  1/(2d²sin²(π/16)) = {qc(0):.10f}")
assert abs(q0_eng - qc(0)) < 1e-10

# шумовой порог max-ent
eta_me = 2.0 / i4_lit
print(f"    p_min(4) = 2/I4 = {eta_me:.6f}  (статья: 0.69055)")

# ── 3) DE: 12 DOF = (a1,a2,a3) амплитуды + 9 фаз лестниц ────────────────
def unpack(x):
    a = np.array([1.0, x[0], x[1], x[2]])
    psi = np.zeros(16, dtype=complex)
    for k in range(d):
        psi[4 * k + k] = a[k]
    psi = psi / np.linalg.norm(psi)
    A2 = diagp([0, x[3], x[4], x[5]])
    B1 = diagp([0, x[6], x[7], x[8]])
    B2 = diagp([0, x[9], x[10], x[11]])
    return psi, A2, B1, B2

def objective(x):
    psi, A2, B1, B2 = unpack(x)
    return I4_value(psi, A2, B1, B2)

NP, NG, F, CR = 80, 400, 0.7, 0.9
best_global, best_x = -10.0, None
for run in range(6):
    lb = np.array([-1.0, -1.0, -1.0] + [0.0] * 9)
    ub = np.array([1.0, 1.0, 1.0] + [2 * np.pi] * 9)
    pop = lb + (ub - lb) * rng.random((NP, 12))
    if run == 0:  # затравка: max-ent + фазы статьи
        pop[0] = np.array([1.0, 1.0, 1.0] + list(np.pi * j[1:] / 4)
                          + list(np.pi * j[1:] / 8) + list(-np.pi * j[1:] / 8))
    if run == 1:  # затравка: паландром (1,γ,γ,1), γ из d=3
        g = (np.sqrt(11) - np.sqrt(3)) / 2
        pop[0] = np.array([g, g, 1.0] + list(np.pi * j[1:] / 4)
                          + list(np.pi * j[1:] / 8) + list(-np.pi * j[1:] / 8))
    fit = np.array([objective(ind) for ind in pop])
    for gen in range(NG):
        for i in range(NP):
            idxs = [k for k in range(NP) if k != i]
            r1, r2, r3 = pop[rng.choice(idxs, 3, replace=False)]
            mutant = pop[i] + F * (r1 - r2 + r3 - pop[i])
            cross = rng.random(12) < CR
            if not cross.any():
                cross[rng.integers(12)] = True
            trial = np.where(cross, mutant, pop[i])
            trial = np.clip(trial, lb, ub)
            f = objective(trial)
            if f >= fit[i]:
                pop[i], fit[i] = trial, f
    k = int(np.argmax(fit))
    if fit[k] > best_global:
        best_global, best_x = float(fit[k]), pop[k].copy()
    print(f"[3] DE прогон {run + 1}: I4 = {fit[k]:.9f}")

psi, A2, B1, B2 = unpack(best_x)
print(f"    ЛУЧШИЙ DE: I4* = {best_global:.10f}")

# ── 3b) УСИЛЕНИЕ: чередование DE <-> собственный вектор B4 ─────────────
for amp in range(8):
    Bop = bell_operator(A2, B1, B2)
    evals, evecs = np.linalg.eigh(Bop)
    lam_max = float(evals[-1])
    psi = evecs[:, -1].real if np.abs(evecs[:, -1].imag).max() < 1e-9 else evecs[:, -1]
    # затравка фаз из текущего решения, состояние = top-собственный вектор
    amp_vec = np.array([abs(psi[4 * k + k]) for k in range(d)])
    x0 = list(amp_vec[1:] / amp_vec[0])
    # фазы: пересчитать DE только по фазам (состояние = eigenvector на лету)
    def obj_phases(ph):
        A2t = diagp([0, ph[0], ph[1], ph[2]])
        B1t = diagp([0, ph[3], ph[4], ph[5]])
        B2t = diagp([0, ph[6], ph[7], ph[8]])
        Bt = bell_operator(A2t, B1t, B2t)
        return float(np.linalg.eigvalsh(Bt)[-1])
    lb = np.zeros(9); ub = np.full(9, 2 * np.pi)
    pop = lb + (ub - lb) * rng.random((60, 9))
    pop[0] = [best_x[3 + i] for i in range(9)]
    fit = np.array([obj_phases(p) for p in pop])
    for gen in range(250):
        for i in range(60):
            idxs = [k for k in range(60) if k != i]
            r1, r2, r3 = pop[rng.choice(idxs, 3, replace=False)]
            mutant = pop[i] + F * (r1 - r2 + r3 - pop[i])
            cross = rng.random(9) < CR
            if not cross.any():
                cross[rng.integers(9)] = True
            trial = np.clip(np.where(cross, mutant, pop[i]), lb, ub)
            f = obj_phases(trial)
            if f >= fit[i]:
                pop[i], fit[i] = trial, f
    k = int(np.argmax(fit))
    ph = pop[k]
    A2 = diagp([0, ph[0], ph[1], ph[2]])
    B1 = diagp([0, ph[3], ph[4], ph[5]])
    B2 = diagp([0, ph[6], ph[7], ph[8]])
    Bop = bell_operator(A2, B1, B2)
    lam_max = float(np.linalg.eigvalsh(Bop)[-1])
    if lam_max > best_global:
        best_global = lam_max
    print(f"    [усиление {amp}] lambda_max(B4) = {lam_max:.10f}")

# ── 4) ОПЕРАТОР БЕЛЛА: lambda_max == итог, состояние = top-собственный ──
Bop = bell_operator(A2, B1, B2)
herm = np.abs(Bop - Bop.conj().T).max()
evals, evecs = np.linalg.eigh(Bop)
lam_max = float(evals[-1])
psi = evecs[:, -1]
if np.abs(psi.imag).max() < 1e-9:
    psi = psi.real.astype(complex)
rayleigh = float(np.real(np.vdot(psi, Bop @ psi)))
resid = float(np.linalg.norm(Bop @ psi - lam_max * psi))
i4_contour = I4_value(psi, A2, B1, B2)
print(f"[4] B4: эрмитовость |B-B†| = {herm:.2e};  lambda_max = {lam_max:.10f}")
print(f"    <Psi|B|Psi> = {rayleigh:.10f};  ||B Psi - lam Psi|| = {resid:.2e}")
print(f"    контур I4(Psi) = {i4_contour:.10f}")
assert abs(lam_max - rayleigh) < 1e-9
assert abs(i4_contour - lam_max) < 1e-9, "контур != оператор!"
amp_vec = np.array([abs(psi[4 * k + k]) for k in range(d)])
print(f"    амплитуды |c_k| = {np.round(amp_vec / amp_vec[0], 8)}")

# ── 5) ШУМОВОЙ ПОРОГ некомпактного оптимума ─────────────────────────────
eta_star = 2.0 / lam_max
rho = np.outer(psi, psi.conj())
rho1 = eta_star * rho + (1 - eta_star) * np.eye(16) / 16.0
def I4_rho(rho, A2, B1, B2):
    tot = 0.0
    for (A, B, c, sgn, w) in [
        (np.eye(d), B1, 0, +1, 1.0), (A2, B1, 3, +1, 1.0), (A2, B2, 0, +1, 1.0), (np.eye(d), B2, 0, +1, 1.0),
        (np.eye(d), B1, 3, -1, 1.0), (A2, B1, 0, -1, 1.0), (A2, B2, 3, -1, 1.0), (np.eye(d), B2, 1, -1, 1.0),
        (np.eye(d), B1, 1, +1, 1/3), (A2, B1, 2, +1, 1/3), (A2, B2, 1, +1, 1/3), (np.eye(d), B2, 3, +1, 1/3),
        (np.eye(d), B1, 2, -1, 1/3), (A2, B1, 1, -1, 1/3), (A2, B2, 2, -1, 1/3), (np.eye(d), B2, 2, -1, 1/3),
    ]:
        Mi = M(A, B)
        E = np.trace(FF.conj().T @ Pc[c] @ FF @ Mi @ rho1 @ Mi.conj().T)
        tot += sgn * w * float(np.real(E))
    return tot
i4_noise = I4_rho(rho1, A2, B1, B2)
print(f"[5] eta* = 2/lambda_max = {eta_star:.6f};  I4(eta*) = {i4_noise:.10f}")
print(f"    сравнение: eta*(d=3, Ацин) = 0.686141;  eta*(d=3, Белл) = 0.696152;  eta*(d=4, max-ent) = {eta_me:.6f}")

print("\n═══ СВОДКА d=4 ═══")
print(f"  локальный предел      = 2 (брутфорс 256)")
print(f"  max-ent               = {i4_lit:.9f}  (статья: 2.896244677)")
print(f"  некомпактный DE+eig   = {lam_max:.9f}")
print(f"  прирост к max-ent     = {100*(lam_max/i4_lit-1):.3f}%")
print(f"  прирост к d=3 Ацину   = {100*(lam_max/2.914854216-1):.3f}%")
print(f"  фазы A2 = {np.round([0, *np.angle(np.diag(A2))[1:]], 6)}")
print(f"  фазы B1 = {np.round([0, *np.angle(np.diag(B1))[1:]], 6)}")
print(f"  фазы B2 = {np.round([0, *np.angle(np.diag(B2))[1:]], 6)}")
# сохраняем эталон для калькуляторной сессии
np.savez('/home/z/my-project/scripts/session7/d4_optimum.npz',
         psi=psi, A2=np.diag(A2), B1=np.diag(B1), B2=np.diag(B2),
         lam_max=lam_max, i4_maxent=i4_lit)
print("  эталон сохранён: d4_optimum.npz")
