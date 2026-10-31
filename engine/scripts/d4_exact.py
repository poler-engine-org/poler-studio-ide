#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Сессия 7 — ТОЧНАЯ алгебра оптимума d=4.

Фазы = линейные лестницы статьи (DE подтвердил), состояние палиндромное
c = (1, g, g, 1)/sqrt(2+2g^2). Тогда все вероятности P_ij(c) аналитичны:

  P_ij(c) = |sum_j c_j e^{i*pi*j*(4c+s)/8}|^2 / 16,  s — сдвиг настройки.

Для палиндрома |sum|^2 = 4[cos(3θ/2)+g·cos(θ/2)]^2, θ = π(4c+s)/8,
и после нормировки (2+2g²): P_ij(c) = [cos(3θ/2)+g·cos(θ/2)]²/(2+2g²)/2.
Отсюда I4(g) = P(g)/Q(g) — отношение квадратичных полиномов от g.
Условие dI4/dg = 0 даёт КУБИКУ с алгебраическими коэффициентами (√2-башня).
"""
import sympy as sp

g = sp.symbols('g', real=True)

def P(c, s):
    """P_ij(c) для палиндрома (1,g,g,1)/sqrt(2+2g^2).
    θ = π(4c+s)/8, s = 4(α+β): P11→s=1, P21→s=3, P22→s=1, P12→s=−1."""
    th = sp.pi * (4 * c + s) / 8
    return (sp.cos(3 * th / 2) + g * sp.cos(th / 2))**2 / (2 + 2 * g**2)

# формула I4 (конвенция движка == формула Id статьи):
# k=0 (w=1):   +[P11(0)+P21(-1)+P22(0)+P12(0)] - [P11(-1)+P21(0)+P22(-1)+P12(1)]
# k=1 (w=1/3): +[P11(1)+P21(-2)+P22(1)+P12(-1)] - [P11(-2)+P21(1)+P22(-2)+P12(2)]
I4 = (
    P(0, 1) + P(-1, 3) + P(0, 1) + P(0, -1)
    - P(-1, 1) - P(0, 3) - P(-1, 1) - P(1, -1)
    + sp.Rational(1, 3) * (P(1, 1) + P(-2, 3) + P(1, 1) + P(-1, -1)
                           - P(-2, 1) - P(1, 3) - P(-2, 1) - P(2, -1))
)
I4 = sp.simplify(sp.together(I4))
print("I4(g) =", I4, flush=True)

# проверка числом: g=1 -> max-ent статьи
val_one = float(I4.subs(g, 1))
lit = (sp.Rational(2, 3)) * (sp.sqrt(2) + sp.sqrt(10 - sp.sqrt(2)))
print(f"I4(g=1)    = {val_one:.12f}")
print(f"литература = {float(lit):.12f}   совпадение: {abs(val_one-float(lit)) < 1e-12}", flush=True)

# кубика: dI4/dg = 0 -> (P'Q - PQ') = 0 (кубика!)
dI4 = sp.diff(I4, g)
crit_num = sp.fraction(sp.together(dI4))[0]
cubic_expr = sp.expand(crit_num)
cubic_expr = sp.radsimp(sp.expand(cubic_expr.rewrite(sp.sqrt)))
cubic = sp.Poly(cubic_expr, g)
print("\nКУБИКА dI4/dg = 0 (после очистки знаменателя):")
print("  ", cubic.as_expr(), flush=True)
print("  старшая степень по g:", cubic.degree())

# численные корни (высокая точность)
roots = [r for r in sp.nroots(cubic, n=40) if abs(sp.im(r)) < sp.Float(1e-25)]
print("\nвещественные корни кубики:")
best_g, best_val = None, -10
for r in roots:
    rv = float(sp.re(r))
    iv = float(I4.subs(g, rv))
    print(f"  g = {rv:.15f}   I4 = {iv:.12f}")
    if 0 < rv < 1 and iv > best_val:
        best_g, best_val = sp.re(r), iv

print(f"\nγ4 = {float(best_g):.15f}")
I4_star_val = float(I4.subs(g, best_g))
print(f"I4* = {I4_star_val:.15f}")
print(f"сверка с DE+eig 2.9726982671: |Δ| = {abs(I4_star_val-2.9726982671):.2e}")

# вторая производная — максимум?
d2 = sp.diff(I4, g, 2)
print(f"d2I4/dg2(γ4) = {float(d2.subs(g, float(best_g))):.4f}  (< 0 => максимум)")

# радикальная форма (Кардано) — попытка
print("\nпопытка замкнутой формы γ4:")
for basis in ([sp.sqrt(2)], [sp.sqrt(2), sp.sqrt(3)],
              [sp.sqrt(2), sp.sqrt(3), sp.sqrt(5), sp.sqrt(7)]):
    try:
        guess = sp.nsimplify(sp.Float(float(best_g), 25), basis)
        if guess.is_number and abs(float(guess) - float(best_g)) < 1e-12:
            print(f"  nsimplify{basis} = {guess}")
            break
    except Exception as e:
        print(f"  {basis}: {e}")
else:
    print("  простой формы нет — γ4 алгебраичен над Q(√2-башней), степень > 1")

# γ4 как точный RootOf кубики + радикальные коэффициенты кубики
coeffs = cubic.all_coeffs()
print("\nкоэффициенты кубики (радикалы):")
for i, c in enumerate(coeffs):
    print(f"  [{3-i}] {sp.radsimp(c)}")

# η* и сравнения
eta = 2.0 / I4_star_val
print(f"\nη*(d=4) = 2/I4* = {eta:.9f}")
print(f"η*(d=3,Ацин) = {2/2.9148542156:.9f}  d=4 терпеливее: {2/2.9148542156 > eta}")

with open('/home/z/my-project/scripts/session7/d4_exact.txt', 'w') as f:
    f.write(f"кубика: {cubic.as_expr()}\n")
    f.write(f"γ4 = {float(best_g):.15f}\n")
    f.write(f"I4* = {I4_star_val:.15f}\n")
    f.write(f"η* = {eta:.12f}\n")
    for i, c in enumerate(coeffs):
        f.write(f"кубика a{3-i} = {sp.radsimp(c)}\n")
print("\nсохранено: d4_exact.txt")
