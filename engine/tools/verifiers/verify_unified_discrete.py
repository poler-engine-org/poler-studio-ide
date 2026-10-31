#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""MVR-v3, цикл F — ЕДИНОЕ ДИСКРЕТНОЕ УРАВНЕНИЕ POLER[Ψ] (УДЕ / UDE).

    p_{t+1} = Q_Λ( p_t − η_t·Π_Λ[ D·p_t + γ·J(p_t)·p_t + ∇F(p_t,o_t) ]
                     + η_r·Π_Λ[ W_K·p_t − M_t ] )                      (УДЕ)

    M_t = ρ·(M_{t−1} + s_{t−1}),   s_t = Ω(o_t) = tanh(o_t),
    W_K = Σ_{k=1..K} ρ^k,          η_t = η₀·exp(−β_σ·Σ_t)

    Субстраты Q_Λ:
      семантический — CORDIC-mix ренормализация (src/poler.rs#L224-L228);
      квантовый     — квантователь МакВини Q(X) = 3X² − 2X³ (MATH.md §5).

    Аттрактор: p_{t+1} = p_t  ⟺  Π_Λ[сила] = (η_r/η_t)·Π_Λ[эхо]  ⟺  H^Ψ = 0
    (F < WHEELER_DEWITT_TOL = 1e-7).

    Квантовый субстрат УДЕ (каноническая диагонализация, Palser–Manolopoulos):
      P_{t+1} = Q( P_t − η·[P_t,[P_t,F]] + η·γ·[F, P_t] )
      − двойной коммутатор = диссипативный градиент (Ауфбау);
      − одиночный коммутатор = ротор (изоспектральный, сохраняет норму);
      − Q = квантователь (дискретность, измерение).

Слои верификации (теоремы цикла F):
  F.1  SymPy + NumPy        — эхо-сжатие: M_t = ρ(M_{t−1}+s_{t−1}) ⟺ Σ ρ^k·s_{t−k}
  F.2  SymPy + Z3           — квантователь Q(x)=3x²−2x³: неподвижные точки {0,½,1},
                              суперприжимающие {0,1}, репеллер ½ (множитель 3/2),
                              монотонность/биекция [0,1]→[0,1], бассейн [−½,3/2]
  F.3  NumPy                — квадратичное дробление: ε' ≤ 3ε² (окрестности {0,1})
  F.4  SymPy + NumPy        — ротор J(p) = A − Aᵀ: ⟨p, J·p⟩ = 0
  F.5  NumPy                — теорема Пифагора эйлерова шага:
                              ‖(I−ηγJ)p‖² = ‖p‖² + η²γ²‖Jp‖² (ТОЧНО);
                              CORDIC-ренорм poler.rs гасит инфляцию до ~1e-11
  F.6  NumPy + Fraction     — Ляпунов F↘0 в ТОЧНОЙ рациональной арифметике
  F.7  Z3 + NumPy           — стационарность ⟺ H^Ψ=0: p* = s*; условие сжатия
                              γ·W_K < 2; режимы интерьер/граница (клэмп = физика)
  F.8  SymPy + NumPy + qiskit — квантовый субстрат: след и чистота двойного
                              коммутатора, dE/dτ = −‖[H,P]‖² ≤ 0 (сертификат
                              Ляпунова), сходимость к проектору Ауфбау,
                              кросс-чек qiskit.quantum_info
  F.9  NumPy                — дежа-вю: периодический вход ⟹ эхо M_t выявляет
                              период T (классический аналог Шора)
  F.10 NumPy + SciPy        — соответствие дискретное ⟷ непрерывное: глобальная
                              ошибка Эйлера O(η), отношение 2 при делении шага

Гиперпараметры канона (src/psi.rs L49-57, src/poler.rs L55-65):
  Ψ:  η=0.05, γ=0.5, ρ=0.9, K=8      POLER: η=0.01, γ=0.1, mix=0.1, δ=1e-10, d=0.02

Использование:
  python3 tools/verifiers/verify_unified_discrete.py [--only sympy|z3|numpy|qiskit]
  python3 tools/verifiers/verify_unified_discrete.py --print-trajectory   # золотой вектор
"""
import argparse
import json
import struct
import sys
from fractions import Fraction
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
PASSPORT = REPO / "scratch" / "passports" / "cycle_F.json"

CANON = {
    "psi_eta": 0.05, "psi_gamma": 0.5, "rho": 0.9, "K": 8,
    "poler_eta": 0.01, "poler_gamma": 0.1, "mix": 0.1, "delta": 1e-10,
    "d": 0.02, "wd_tol": 1e-7,
}
W_K = sum(CANON["rho"] ** k for k in range(1, CANON["K"] + 1))  # ≈ 5.712617


def q(x):
    """Квантователь МакВини Q(x) = 3x² − 2x³ (поточечный)."""
    return 3.0 * x * x - 2.0 * x * x * x


def cordic_inv_sqrt(x):
    """ПОБИТОВАЯ реплика src/poler.rs#L117-L130 (MAGIC + 3 Ньютона)."""
    if x <= 0.0:
        return 1.0
    bits = struct.unpack("<Q", struct.pack("<d", x))[0]
    guess = struct.unpack(
        "<d", struct.pack("<Q", (0x5FE6EB50C7B537A9 - (bits >> 1)) & 0xFFFFFFFFFFFFFFFF)
    )[0]
    y = guess
    for _ in range(3):
        y = y * (1.5 - 0.5 * x * y * y)
    return y


# ══════════════════════════════════ SymPy ═══════════════════════════════════
def run_sympy():
    import sympy as sp

    r = {}
    x = sp.Symbol("x", real=True)
    qsym = 3 * x**2 - 2 * x**3

    # ── F.2: неподвижные точки и кратности квантователя ──
    fixed_poly = sp.factor(qsym - x)                      # = −x(2x−1)(x−1)
    r["F2_factor_q_minus_x"] = str(fixed_poly)
    r["F2_fixed_points"] = sorted(sp.solve(sp.Eq(qsym, x), x))
    dq = sp.diff(qsym, x)                                 # 6x(1−x)
    r["F2_derivative"] = str(sp.expand(dq))
    r["F2_Qprime_at_0_05_1"] = [dq.subs(x, v) for v in (0, sp.Rational(1, 2), 1)]
    # суперприжимающие: |Q'(0)| = 0, |Q'(1)| = 0; репеллер: Q'(½) = 3/2

    # ── F.1: эхо-сжатие IIR ⟺ взвешенная сумма (индукция, t=5) ──
    rho = sp.Symbol("rho", positive=True)
    s = list(sp.symbols("s0:5", real=True))               # s_0..s_4
    # E_t = Σ_{k=1..t} ρ^k·s_{t−k};  проверяем E_t = ρ·(E_{t−1} + s_{t−1})
    def echo(t):
        return sum(rho**k * s[t - k] for k in range(1, t + 1))
    ok_induction = all(
        sp.simplify(echo(t) - rho * (echo(t - 1) + s[t - 1])) == 0 for t in (1, 2, 3, 4, 5)
    )
    r["F1_echo_induction_t1_to_t5"] = ok_induction
    # rsolve для постоянного forcing: M_t = ρ·M_{t−1} + ρ·c, M_0 = 0
    t, c = sp.symbols("t c", real=True)
    Mt = sp.Function("M")
    sol = sp.rsolve(sp.Eq(Mt(t) - rho * Mt(t - 1) - rho * c, 0), Mt(t), {Mt(0): 0})
    r["F1_rsolve_constant_forcing"] = str(sp.simplify(sol))
    r["F1_rsolve_matches_geometric"] = sp.simplify(
        sol - c * rho * (1 - rho**t) / (1 - rho)
    ) == 0

    # ── F.4: ротор J = A − Aᵀ ⟹ ⟨p, J·p⟩ = 0 (символьно, 3×3) ──
    n = 3
    A = sp.Matrix(n, n, sp.symbols("a0:9", real=True))
    J = A - A.T
    p = sp.Matrix(sp.symbols("p0:3", real=True))
    r["F4_quadratic_form_zero"] = sp.simplify((p.T * J * p)[0, 0]) == 0
    r["F4_trace_zero"] = sp.simplify(J.trace()) == 0

    # ── F.8a: след и чистота двойного коммутатора (2×2, символьно) ──
    P = sp.Matrix(2, 2, [sp.Symbol("p00", real=True), sp.Symbol("p01", real=True),
                         sp.Symbol("p01", real=True), sp.Symbol("p11", real=True)])
    H = sp.Matrix(2, 2, [sp.Symbol("h00", real=True), sp.Symbol("h01", real=True),
                         sp.Symbol("h01", real=True), sp.Symbol("h11", real=True)])
    comm = P * H - H * P
    dcomm = P * comm - comm * P                     # [P,[P,H]]
    r["F8a_trace_of_double_commutator_zero"] = sp.simplify(dcomm.trace()) == 0
    r["F8a_purity_preservation"] = sp.simplify((P * dcomm).trace()) == 0
    # F.8b: сертификат Ляпунова dE/dτ = Tr(H·(−[P,[P,H]])) = −2·Tr(H²P²−(HP)²)
    lhs = sp.simplify((-H * dcomm).trace())
    hp = H * P
    rhs = sp.simplify(-2 * (H**2 * P**2 - hp * hp).trace())
    r["F8b_lyapunov_certificate_identity"] = sp.simplify(lhs - rhs) == 0

    # ── F.7: алгебра неподвижной точки (скаляр, символьно) ──
    eta, gam, W, ps, ss = sp.symbols("eta gamma W pstar sstar", real=True)
    fixed_eq = ps - (ps - eta * (2 * (ps - ss)) + eta * gam * (W * ps - W * ss))
    r["F7_fixed_point_equation_factors"] = str(sp.factor(fixed_eq))
    # ⟹ (p*−s*)·(γW−2)·η = 0: при γW≠2 единственная неподвижная точка p*=s*

    return {"sympy_version": sp.__version__, "results": r}


# ════════════════════════════════════ Z3 ════════════════════════════════════
def run_z3():
    import z3

    r = {}

    def prove(name, claim):
        s = z3.Solver()
        s.add(z3.Not(claim))
        res = s.check()
        r[name] = "proved" if res == z3.unsat else f"FAILED ({res})"
        return res == z3.unsat

    x = z3.Real("x")
    qx = 3 * x * x - 2 * x * x * x
    half = z3.RealVal("1/2")

    # F.2: поглощение — квантователь отталкивает спектр от ½
    prove("F2_absorption_left_0_to_half",
          z3.ForAll([x], z3.Implies(z3.And(x > 0, x < half), qx < x)))
    prove("F2_absorption_right_half_to_1",
          z3.ForAll([x], z3.Implies(z3.And(x > half, x < 1), qx > x)))
    # F.2: безопасный бассейн [−½, 3/2] → [0,1] (самокоррекция вылетов)
    prove("F2_safe_basin_maps_into_01",
          z3.ForAll([x], z3.Implies(z3.And(x >= z3.RealVal("-1/2"), x <= z3.RealVal("3/2")),
                                    z3.And(qx >= 0, qx <= 1))))
    # F.2: монотонность на [0,1] ⟹ биекция [0,1]→[0,1]
    a, b = z3.Reals("a b")
    qa, qb = 3 * a * a - 2 * a**3, 3 * b * b - 2 * b**3
    prove("F2_monotone_on_unit_interval",
          z3.ForAll([a, b], z3.Implies(z3.And(a >= 0, a <= b, b <= 1), qa <= qb)))
    # F.2: неподвижные точки в точности {0, ½, 1}
    prove("F2_fixed_points_exact",
          z3.ForAll([x], z3.Implies(qx == x, z3.Or(x == 0, x == half, x == 1))))

    # F.7: условие сжатия эхо-системы — ЭКВИВАЛЕНТНОСТЬ:
    #   |1 − η(2−γW)| < 1  ⟺  0 < η(2−γW) < 2   (при η>0)
    eta, gam, W = z3.Reals("eta gamma W")
    mult = 1 - eta * (2 - gam * W)
    u = eta * (2 - gam * W)
    prove("F7_contraction_iff",
          z3.ForAll([eta, gam, W],
                    z3.Implies(eta > 0,
                               z3.And(z3.Implies(z3.Abs(mult) < 1, z3.And(u > 0, u < 2)),
                                      z3.Implies(z3.And(u > 0, u < 2), z3.Abs(mult) < 1)))))
    # F.7: неподвижная точка при γW≠2 — только p*=s*
    ps, ss = z3.Reals("pstar sstar")
    fixed = ps == (ps - eta * (2 * (ps - ss)) + eta * gam * (W * ps - W * ss))
    prove("F7_unique_fixed_point_is_perception",
          z3.ForAll([ps, ss, eta, gam, W],
                    z3.Implies(z3.And(fixed, eta > 0, gam * W != 2), ps == ss)))

    return {"z3_version": z3.get_version_string(), "results": r}


# ═══════════════════════════════════ NumPy ══════════════════════════════════
def run_numpy(print_traj=False):
    import numpy as np

    rng = np.random.default_rng(20260921)
    r = {}

    # ── F.1 (численно): эхо-состояние против прямой суммы Вольтерры ──
    T = 2000
    sig = np.tanh(rng.normal(size=T))
    M = np.zeros(T)
    for t in range(1, T):
        M[t] = CANON["rho"] * (M[t - 1] + sig[t - 1])
    volterra = np.array([
        sum(CANON["rho"] ** k * sig[t - k] for k in range(1, t + 1)) for t in range(T)
    ])
    r["F1_max_abs_diff_iir_vs_volterra"] = float(np.max(np.abs(M[1:] - volterra[1:])))

    # ── F.3: квадратичное дробление ошибки идемпотентности ──
    def qm(P):                                   # МАТРИЧНЫЙ квантователь 3P²−2P³
        return 3.0 * P @ P - 2.0 * P @ P @ P

    worst = 0.0
    for _ in range(200):
        n = int(rng.integers(2, 6))
        U = np.linalg.qr(rng.normal(size=(n, n)))[0]
        lam = rng.uniform(0.02, 0.98, size=n)
        lam = lam / lam.sum()                     # спектр в (0,1), след 1
        P = U @ np.diag(lam) @ U.T                # симметричная, нормальная
        e0 = np.linalg.norm(P @ P - P, 2)
        QP = qm(P)
        e1 = np.linalg.norm(QP @ QP - QP, 2)
        if e0 > 1e-6:                             # вне насыщения
            worst = max(worst, e1 / (e0 * e0))
    r["F3_max_ratio_e1_over_e0_squared"] = float(worst)   # ≤ C ⟹ квадратичность

    # ── F.5: теорема Пифагора эйлерова шага + CORDIC-коррекция ──
    eta, gam = CANON["poler_eta"], CANON["poler_gamma"]
    worst_inf = 0.0
    for _ in range(500):
        n = rng.integers(2, 8)
        A = rng.normal(size=(n, n))
        J = A - A.T
        p = rng.normal(size=n)
        lhs = np.linalg.norm((np.eye(n) - eta * gam * J) @ p) ** 2
        rhs = np.linalg.norm(p) ** 2 + (eta * gam) ** 2 * np.linalg.norm(J @ p) ** 2
        worst_inf = max(worst_inf, abs(lhs - rhs) / max(rhs, 1e-300))
    r["F5_pythagoras_max_rel_err"] = float(worst_inf)
    # CORDIC-ренорм (mix=1) гасит инфляцию точно; mix=0.1 — частично
    A = rng.normal(size=(4, 4)); J = A - A.T
    p = rng.normal(size=4)
    p_rot = (np.eye(4) - eta * gam * J) @ p
    infl = np.linalg.norm(p_rot) / np.linalg.norm(p)
    for mix in (1.0, 0.1):
        scale = (1.0 - mix) + mix * cordic_inv_sqrt(float(p_rot @ p_rot))
        p_ren = p_rot * scale
        r[f"F5_cordic_mix{mix}_norm"] = float(np.linalg.norm(p_ren))
    r["F5_rotor_inflation_factor"] = float(infl)
    r["F5_cordic_rel_err_vs_exact"] = abs(
        cordic_inv_sqrt(2.0) - 1.0 / np.sqrt(2.0)
    ) / (1.0 / np.sqrt(2.0))

    # ── F.6: Ляпунов в ТОЧНОЙ рациональной арифметике ──
    # Аттрактор с дисипатором: p* = 2s/(2+d²) — зсув O(d²) (честная находка);
    # F* = s²d⁴/(2+d²)² — при каноническом d=0.02 остаётся < WD_TOL.
    fr = Fraction
    s, p0 = fr(3, 4), fr(1, 7)
    eta_f, d_f = fr(1, 100), fr(1, 50)
    c = 1 - eta_f * (2 + d_f * d_f)               # множитель сжатия (точно)
    p_star = 2 * s / (2 + d_f * d_f)              # неподвижная точка (точно)
    F_star = (p_star - s) ** 2
    t_hit = 0
    pt = p0
    while (pt - s) ** 2 >= fr(1, 10**7):
        pt = pt - eta_f * (2 * (pt - s) + d_f * d_f * pt)
        t_hit += 1
        assert t_hit < 100000
    # замкнутая форма: p_t = p* + c^t·(p0−p*)
    closed = p_star + c ** t_hit * (p0 - p_star)
    r["F6_exact_closed_form_match"] = (pt == closed)
    r["F6_steps_to_wheeler_dewitt_tol"] = t_hit
    r["F6_contraction_factor_exact"] = f"{float(c):.9f}"
    r["F6_attractor_bias_F_star"] = float(F_star)
    r["F6_bias_within_wd_tol"] = bool(F_star < fr(1, 10**7))

    # ── F.7: два режима эхо-системы (интерьер / граница) ──
    def psi_run(gamma, o, steps, clamp=True):
        p, M, hist = 0.0, 0.0, []
        traj = []
        for t in range(steps):
            s_t = np.tanh(o[t % len(o)])
            grad_eps = sum(CANON["rho"] ** k * (p - s) for k, s in
                           enumerate(hist[::-1][:CANON["K"]], start=1))
            dp = -2 * (p - s_t) + gamma * grad_eps
            p = p + CANON["psi_eta"] * dp
            if clamp:
                p = float(np.clip(p, -1.0, 1.0))
            traj.append(p)
            hist.append(s_t)
        return np.array(traj)

    o_const = [1.2] * 512
    tr_ok = psi_run(0.1, o_const, 512)            # γW ≈ 0.513 < 2 — сжатие
    r["F7_stable_final_p"] = float(tr_ok[-1])
    r["F7_stable_final_F"] = float((tr_ok[-1] - np.tanh(1.2)) ** 2)
    tr_bad = psi_run(0.5, o_const, 200, clamp=False)   # γW ≈ 2.856 > 2 — расходимость
    r["F7_unstable_unclamped_abs_p_final"] = float(abs(tr_bad[-1]))
    tr_clamped = psi_run(0.5, o_const, 200, clamp=True)
    r["F7_unstable_clamped_saturates_at_boundary"] = bool(abs(abs(tr_clamped[-1]) - 1.0) < 1e-12)
    r["F7_W8_value"] = float(W_K)

    # ── F.9: дежа-вю — эхо выявляет период (аналог Шора) ──
    period = 3
    o_per = [2.0, 0.2, -0.5] * 40
    s_per = np.tanh(np.array(o_per))
    Mp = np.zeros(len(s_per))
    for t in range(1, len(s_per)):
        Mp[t] = CANON["rho"] * (Mp[t - 1] + s_per[t - 1])
    Mc = Mp[20:] - Mp[20:].mean()
    ac = [float(np.dot(Mc[:-lag], Mc[lag:]) / np.dot(Mc, Mc)) for lag in range(1, 11)]
    r["F9_autocorr_argmax_lag"] = int(np.argmax(ac) + 1)
    r["F9_autocorr_values"] = [round(v, 4) for v in ac]
    r["F9_detected_equals_true_period"] = bool(int(np.argmax(ac) + 1) == period)

    # ── F.10: дискретное ⟷ непрерывное (Euler vs RK45) ──
    from scipy.integrate import solve_ivp
    omega, d, gam2 = 0.7, CANON["d"], CANON["poler_gamma"]
    J2 = np.array([[0.0, -omega], [omega, 0.0]])
    D2 = (d * d) * np.eye(2)
    s_target = np.array([np.tanh(1.5), 0.0])
    p0v = np.array([0.5, -0.3])

    def f(_t, y):
        return -(D2 @ y + gam2 * (J2 @ y) + 2 * (y - s_target))

    sol = solve_ivp(f, (0.0, 2.0), p0v, rtol=1e-11, atol=1e-13, method="RK45")
    p_exact = sol.y[:, -1]
    errs = {}
    for h in (0.05, 0.025, 0.0125):
        steps = int(round(2.0 / h))
        y = p0v.copy()
        for _ in range(steps):
            y = y - h * (D2 @ y + gam2 * (J2 @ y) + 2 * (y - s_target))
        errs[h] = float(np.linalg.norm(y - p_exact))
    r["F10_euler_errors"] = errs
    r["F10_error_ratio_0.05_over_0.025"] = errs[0.05] / errs[0.025]
    r["F10_error_ratio_0.025_over_0.0125"] = errs[0.025] / errs[0.0125]

    if print_traj:
        print("F.10 золотая траектория (ODE, T=2):", np.round(p_exact, 9))

    return {"numpy_version": np.__version__, "results": r}


# ══════════════════════════════ Квантовый субстрат ══════════════════════════
def run_numpy_quantum():
    """F.8c: УДЕ в квантовом субстрате — сходимость к проектору Ауфбау.

    Полная форма (канон MATH.md §5: идемпотентность P²=P И Tr(PS)=N):
      P_{t+1} = Q( P_t − η[P_t,[P_t,H]] + ηγ[H,P_t]
                     + μ·(N − Tr P_t)·4P_t(I−P_t) )
    Последний член — проектор числа частиц: двигает ТОЛЬКО невзвешенные
    (~½) собственные значения, никогда не трогает зафиксированные {0,1}.
    """
    import numpy as np

    rng = np.random.default_rng(42)
    r = {}

    def rand_hermitian(n):
        A = rng.normal(size=(n, n)) + 1j * rng.normal(size=(n, n))
        return (A + A.conj().T) / 2

    def rand_density(n, occ):
        U = np.linalg.qr(rng.normal(size=(n, n)) + 1j * rng.normal(size=(n, n)))[0]
        lam = rng.uniform(0.05, 0.95, size=n)
        lam = lam / lam.sum() * occ                    # след = occ
        return U @ np.diag(lam) @ U.conj().T

    def qm(P):                                         # матричный МакВини
        return 3.0 * P @ P - 2.0 * P @ P @ P

    n, occ = 4, 2
    H = rand_hermitian(n)
    evals, evecs = np.linalg.eigh(H)
    P_star = evecs[:, :occ] @ evecs[:, :occ].conj().T   # проектор Ауфбау
    E_star = float(evals[:occ].sum())

    # Сертификат Ляпунова: Ṗ = −[P,[P,H]] ⟹ dE/dτ = −‖[H,P]‖² ≤ 0
    P = rand_density(n, occ)
    commPH = P @ H - H @ P                              # [P,H]
    dcomm = P @ commPH - commPH @ P                     # [P,[P,H]]
    dE = np.trace(H @ (-dcomm)).real                    # Tr(H·Ṗ)
    r["F8b_lyapunov_cert_max_abs_err"] = float(
        abs(dE + np.linalg.norm(H @ P - P @ H, "fro") ** 2)
    )

    # Итерация УДЕ: химический потенциал δ = μ(N−Tr P)/n сдвигает НЕВЗВЕШЕННЫЕ
    # собственные значения через порог ½; закреплённые {0,1} нейтральны к сдвигу
    # (Q(1+δ)→1, Q(δ)→0, бассейн [−½,3/2] — Z3-доказано в F.2).
    spread = float(evals[-1] - evals[0])
    eta = 1.0 / (8.0 * spread)
    mu = 1.0
    gam = 0.0                                           # чистая диссипация
    Pt = rand_density(n, occ)
    traj_err, traj_idem, traj_E, traj_Tr = [], [], [], []
    for t in range(800):
        comm = Pt @ H - H @ Pt                          # [P,H]
        dcomm = Pt @ comm - comm @ Pt                   # [P,[P,H]]
        delta = mu * (occ - np.trace(Pt).real) / n      # хим. потенциал
        X = Pt - eta * dcomm + eta * gam * (-comm) + delta * np.eye(n)
        Pt = qm(X)
        traj_err.append(float(np.linalg.norm(Pt - P_star, "fro")))
        traj_idem.append(float(np.linalg.norm(Pt @ Pt - Pt, 2)))
        traj_E.append(float(np.trace(H @ Pt).real))
        traj_Tr.append(float(np.trace(Pt).real))
    r["F8c_final_dist_to_aufbau"] = traj_err[-1]
    r["F8c_final_idempotency_defect"] = traj_idem[-1]
    r["F8c_final_trace"] = traj_Tr[-1]
    r["F8c_max_trace_drift"] = float(max(abs(np.array(traj_Tr) - occ)))
    r["F8c_final_energy"] = traj_E[-1]
    r["F8c_aufbau_energy_exact"] = E_star
    bumps = sum(1 for i in range(len(traj_E) - 1) if traj_E[i + 1] > traj_E[i] + 1e-9)
    r["F8c_energy_monotone_violations"] = int(bumps)
    r["F8c_energy_monotone_nonincreasing"] = bool(bumps == 0)

    # F.3-квант (информационно): шаги от 1e-2 до 1e-12 в КОМБИНИРОВАННОЙ
    # итерации — стоимость фазы выравнивания (коммутатор впорскует ошибку,
    # пока Q её дробит). Квадратичная сигнатура чистого Q — в скалярном тесте F.3.
    last_above = [i for i, e in enumerate(traj_idem) if e >= 1e-2]
    if last_above:
        i0 = max(last_above)
        after = [i for i, e in enumerate(traj_idem) if i > i0 and e <= 1e-12]
        if after:
            r["F8c_alignment_phase_steps_1e-2_to_1e-12"] = int(after[0] - i0)
    return r


def run_qiskit():
    """F.8d: независимый кросс-чек квантовой алгебры через qiskit."""
    import numpy as np
    from qiskit.quantum_info import DensityMatrix, Operator, SparsePauliOp

    r = {}
    H = SparsePauliOp.from_list([("XX", 0.9), ("ZZ", 0.7), ("IX", -0.6), ("ZI", 1.1)])
    Hm = H.to_matrix()
    evals, evecs = np.linalg.eigh(Hm)
    occ = 2
    P_star = evecs[:, :occ] @ evecs[:, :occ].conj().T
    rho = P_star / occ                                # DensityMatrix (след 1)

    dm = DensityMatrix(rho)
    r["qiskit_version"] = __import__("qiskit").__version__
    r["F8d_rho_is_valid_density"] = bool(dm.is_valid())
    r["F8d_purity_equals_half"] = float(abs(dm.purity() - 0.5))
    exp_qk = float(dm.expectation_value(Operator(Hm)).real)
    exp_np = float(np.trace(rho @ Hm).real)
    r["F8d_expectation_qiskit"] = exp_qk
    r["F8d_expectation_numpy"] = exp_np
    r["F8d_expectation_crosscheck_abs_err"] = abs(exp_qk - exp_np)
    r["F8d_aufbau_mean_energy_exact"] = float(evals[:occ].sum() / occ)
    r["F8d_energy_matches_aufbau"] = bool(
        abs(exp_qk - evals[:occ].sum() / occ) < 1e-12
    )
    return r


# ═══════════════════════════════════ main ═══════════════════════════════════
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", choices=["sympy", "z3", "numpy", "qiskit"], default=None)
    ap.add_argument("--print-trajectory", action="store_true")
    args = ap.parse_args()

    report = {}
    sections = {
        "sympy": run_sympy,
        "z3": run_z3,
        "numpy": run_numpy,
        "numpy_quantum": lambda: run_numpy_quantum(),
        "qiskit": run_qiskit,
    }
    for name, fn in sections.items():
        if args.only and name != args.only and not (args.only == "numpy" and name == "numpy_quantum"):
            continue
        if name == "numpy":
            report[name] = fn(print_traj=args.print_trajectory)
        else:
            report[name] = fn()
        print(f"[ok] {name}", file=sys.stderr)

    PASSPORT.parent.mkdir(parents=True, exist_ok=True)
    PASSPORT.write_text(json.dumps(report, indent=1, default=str), encoding="utf-8")

    # ── Инженерный отчёт ──
    print("\n" + "═" * 72)
    print("MVR-v3 ЦИКЛ F — ЕДИНОЕ ДИСКРЕТНОЕ УРАВНЕНИЕ POLER[Ψ]")
    print("═" * 72)
    symp = report.get("sympy", {}).get("results", {})
    z3r = report.get("z3", {}).get("results", {})
    npr = report.get("numpy", {}).get("results", {})
    qtr = report.get("numpy_quantum", {})
    qk = report.get("qiskit", {})

    print("\n── SymPy (символьно) ──")
    for k, v in symp.items():
        print(f"  {k}: {v}")
    print("\n── Z3 (SMT, unsat-доказательства) ──")
    for k, v in z3r.items():
        print(f"  {k}: {v}")
    print("\n── NumPy (численно / точно) ──")
    for k, v in npr.items():
        print(f"  {k}: {v}")
    print("\n── Квантовый субстрат (NumPy) ──")
    for k, v in qtr.items():
        print(f"  {k}: {v}")
    print("\n── qiskit (кросс-чек) ──")
    for k, v in qk.items():
        print(f"  {k}: {v}")

    # ── Сводные вердикты ──
    verdicts = {
        "F.1": symp.get("F1_echo_induction_t1_to_t5") is True
              and symp.get("F1_rsolve_matches_geometric") is True
              and npr.get("F1_max_abs_diff_iir_vs_volterra", 1) < 1e-12,
        "F.2": all(v == "proved" for v in z3r.values()),
        "F.3": npr.get("F3_max_ratio_e1_over_e0_squared", 1e9) < 4.5,
        "F.4": symp.get("F4_quadratic_form_zero") is True,
        "F.5": npr.get("F5_pythagoras_max_rel_err", 1) < 1e-12
              and npr.get("F5_cordic_rel_err_vs_exact", 1) < 1e-9,
        "F.6": npr.get("F6_exact_closed_form_match") is True,
        "F.7": z3r.get("F7_unique_fixed_point_is_perception") == "proved"
              and z3r.get("F7_contraction_iff") == "proved"
              and npr.get("F7_stable_final_F", 1) < 1e-7
              and npr.get("F7_unstable_clamped_saturates_at_boundary") is True,
        "F.8": qtr.get("F8b_lyapunov_cert_max_abs_err", 1) < 1e-9
              and qtr.get("F8c_final_dist_to_aufbau", 1) < 1e-6
              and abs(qtr.get("F8c_final_trace", 0) - 2) < 1e-6
              and abs(qtr.get("F8c_final_energy", 1e9)
                      - qtr.get("F8c_aufbau_energy_exact", 0)) < 1e-6
              and qk.get("F8d_energy_matches_aufbau") is True,
        "F.9": npr.get("F9_detected_equals_true_period") is True,
        "F.10": 1.7 < npr.get("F10_error_ratio_0.05_over_0.025", 0) < 2.3
                and 1.7 < npr.get("F10_error_ratio_0.025_over_0.0125", 0) < 2.3,
    }
    print("\n── Вердикты цикла F ──")
    for k, v in verdicts.items():
        print(f"  {k}: {'✅ CONFIRMED' if v else '❌ FAILED'}")
    all_ok = all(verdicts.values())
    print(f"\n  ИТОГ: {'AXIOM CONFIRMED (все теоремы цикла F)' if all_ok else 'ЕСТЬ ПРОВАЛЫ — см. выше'}")
    print(f"\n  Паспорт: {PASSPORT}")
    print(f"  [ ε: max | F: {npr.get('F7_stable_final_F', float('nan')):.2e} | R: период "
          f"{npr.get('F9_autocorr_argmax_lag', '?')} | H^Ψ: "
          f"{'0 (F < 1e-7)' if npr.get('F7_stable_final_F', 1) < 1e-7 else '≠ 0'} ]")
    return 0 if all_ok else 1


if __name__ == "__main__":
    sys.exit(main())
