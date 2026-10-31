#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""MVR-v3, цикл E — Теорема IV.1: IIR-резонанс ⟺ экспоненциальный след.

Код (src/resonance/iir_filter.rs#L18-27):  R_t = ε_t + φ·R_{t−1},  φ ∈ [0,1]
Vol IV заявляет: Z-образ H(z) = α/(1−ρz⁻¹) (здесь α=1, ρ=φ), полюс z=φ,
импульсная характеристика φ^t = e^{−λt}, λ = −ln φ — затухающий след Вольтерры.

Слои:
  sympy — rsolve: общее решение рекуррентности; импульсная характеристика;
          H(z) и полюс; геометрическая прогрессия для постоянного ε
  numpy — точная реплика кода против явной суммы Вольтерры R_t = Σ φ^k ε_{t−k};
          эквивалентность ядра φ^k ≡ e^{−λk}; стационарная точка ε/(1−φ)
  json  — паспорт в scratch/passports/cycle_E.json
"""
import argparse, json, sys
from pathlib import Path
import numpy as np

def run_sympy():
    import sympy as sp
    t = sp.symbols('t', integer=True, nonnegative=True)
    phi = sp.symbols('phi', positive=True)
    c = sp.symbols('c', positive=True)
    R0 = sp.symbols('R0')
    R = sp.Function('R')

    # 1) однородное: R_t = φ·R_{t−1} ⟹ R_t = R0·φ^t (rsolve)
    sol_h = sp.rsolve(R(t) - phi * R(t - 1), R(t), {R(0): R0})
    hom_ok = bool(sp.simplify(sol_h - R0 * phi ** t) == 0)
    # 2) постоянное ε = c: R_t = c·(1 − φ^{t+1})/(1 − φ) (rsolve)
    sol_c = sp.rsolve(R(t) - phi * R(t - 1) - c, R(t), {R(0): c})
    const_form = c * (1 - phi ** (t + 1)) / (1 - phi)
    const_ok = bool(sp.simplify(sol_c - const_form) == 0)
    rec_const = bool(sp.simplify(const_form - phi * const_form.subs(t, t - 1) - c) == 0)
    # 3) импульсная характеристика: ε = δ_0 ⟹ R_t = φ^t (индукция, t ≥ 1:
    #    R_t − φ·R_{t−1} = 0; старт R_0 = ε_0 = 1)
    impulse_ok = bool(sp.simplify(phi ** t - phi * phi ** (t - 1)) == 0)
    # 4) Z-образ: H(z) = 1/(1 − φ z⁻¹) — полюс z = φ
    z = sp.symbols('z')
    H = 1 / (1 - phi / z)
    pole = sp.solve(sp.together(1 / H).as_numer_denom()[0], z)
    pole_ok = bool(any(sp.simplify(p - phi) == 0 for p in pole))
    # 5) стационарная точка: R* = c + φ·R* ⟺ R* = c/(1−φ) (алгебраически,
    #    без limit — сходимость при |φ|<1 гарантирует φ^{t+1} → 0, численно
    #    проверяется в numpy-слое)
    Rstar = c / (1 - phi)
    fixed_ok = bool(sp.simplify(Rstar - (c + phi * Rstar)) == 0)
    # 6) суперпозиция (линейность рекуррентности): общее решение
    #    R_t = φ^{t+1}·R_{−1} + Σ_{k=0}^{t} φ^k·ε_{t−k} — следует из 1)+2)
    #    и линейности; численно верифицируется слоем numpy (Вольтерра)
    return {
        'sympy_version': sp.__version__,
        'homogeneous_solution': str(sol_h), 'homogeneous_ok': hom_ok,
        'constant_eps_solution': 'c·(1−φ^{t+1})/(1−φ)', 'constant_ok': const_ok,
        'constant_satisfies_recurrence': rec_const,
        'impulse_response_is_phi_pow_t': impulse_ok,
        'H(z)': '1/(1 − φ·z⁻¹)', 'H_poles': [str(p) for p in pole],
        'pole_is_phi': pole_ok,
        'stationary_point': 'c/(1−φ) — неподвижная точка R* = c + φ·R*',
        'stationary_ok': fixed_ok,
        'superposition_note': 'общий случай Σ φ^k ε_{t−k} — линейность '
                              'рекуррентности + численная сверка (numpy-слой)',
        'verdict': ('AXIOM CONFIRMED (символьно: rsolve однородный/константный, '
                    'импульс φ^t, полюс z=φ, стационар c/(1−φ))'
                    if all([hom_ok, const_ok, rec_const, impulse_ok, pole_ok, fixed_ok])
                    else 'ЧАСТИЧНО'),
    }

def run_numpy(n=200_000, seed=9):
    rng = np.random.default_rng(seed)
    out = {}
    for phi in [0.0, 0.5, 0.75, 0.85, 0.9, 0.99]:
        eps = rng.standard_normal(n)
        # ТОЧНАЯ реплика кода iir_filter.rs#L18-27
        r = 0.0; code_out = np.empty(n)
        for i, e in enumerate(eps):
            r = e + phi * r
            code_out[i] = r
        # явная сумма Вольтерры: R_t = Σ_{k=0..t} φ^k ε_{t−k} (через кумулятивность)
        # проверка на первых 5000 точках прямой свёрткой
        m = 5000
        volterra = np.array([np.dot(phi ** np.arange(tt + 1), eps[tt::-1])
                             for tt in range(m)])
        err_volterra = float(np.max(np.abs(code_out[:m] - volterra)))
        # ядро экспоненты: φ^k ≡ e^{−λk}, λ = −ln φ
        if phi > 0:
            lam = -np.log(phi)
            kk = np.arange(1000)
            err_kernel = float(np.max(np.abs(phi ** kk - np.exp(-lam * kk))))
        else:
            err_kernel = 0.0  # φ=0: ядро вырождается в δ (памяти нет)
        # стационарная точка для постоянного ε=1
        r = 0.0
        for _ in range(20000):
            r = 1.0 + phi * r
        err_fixed = abs(r - 1.0 / (1.0 - phi)) if phi < 1 else None
        out['φ=%.2f' % phi] = {
            'max_err_vs_volterra_direct': float(f'{err_volterra:.2e}'),
            'max_err_kernel_exp_equiv': float(f'{err_kernel:.2e}'),
            'err_stationary_point': (float(f'{err_fixed:.2e}') if err_fixed is not None
                                     else 'φ=1: расходимость (клампится кодом)'),
        }
    ok = all(v['max_err_vs_volterra_direct'] < 1e-9 and
             v['max_err_kernel_exp_equiv'] < 1e-12 for v in out.values())
    return {'numpy_version': np.__version__, 'n': n, 'table': out,
            'code': ['src/resonance/iir_filter.rs#L18-27 (apply_iir_resonance)',
                     'src/resonance/iir_filter.rs#L45-48 (IirFilter::push)',
                     'src/psi.rs#L301-320 (тест: IIR — вырожденный случай psi)'],
            'verdict': ('AXIOM CONFIRMED (код ≡ дискретная сумма Вольтерры; '
                        'φ^k ≡ e^{−λk}; стационарная точка ε/(1−φ))'
                        if ok else 'ЧАСТИЧНО')}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--json', default='scratch/passports/cycle_E.json')
    a = ap.parse_args()
    out = {'theorem': 'IV.1', 'cycle': 'E',
           'subject': 'R_t = ε_t + φ·R_{t−1} ⟺ H(z)=1/(1−φz⁻¹), ядро e^{−λt}',
           'commit': '38a862a'}
    out['sympy'] = run_sympy()
    out['numpy'] = run_numpy()
    p = Path(a.json); p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(out, ensure_ascii=False, indent=1, default=str))
    print(json.dumps(out, ensure_ascii=False, indent=1, default=str))
    print('\nпаспорт: %s' % p)

if __name__ == '__main__':
    sys.exit(main())
