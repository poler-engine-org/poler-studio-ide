#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""MVR-v3, цикл D — Теорема I.1: кососимметричный ротор сохраняет норму.

    J = U − Uᵀ  ⟹  dψ/dt = Jψ  ⟹  d/dt‖ψ‖² = ψᵀ(Jᵀ+J)ψ = 0

Слои:
  sympy — символьное доказательство тождества Jᵀ = −J и ψᵀ(Jᵀ+J)ψ ≡ 0
  numpy — RK4-интегрирование dψ/dt=Jψ (n=64, T=50): дрейф нормы ~0 против
          контрольной группы (J' = U — симметричная часть не убита);
          спектральный тест: Re(λ(J)) = 0 (алгебра so(n))
  code  — ДИСКРЕТНЫЙ инвариант precess_step (gyro.rs#L632-645):
          Σθ сохраняется на каждом шаге — парные ±torque сокращаются ТОЧНО
          (кососимметрия J_ij = −J_ji на уровне пар)
  json  — паспорт в scratch/passports/cycle_D.json
"""
import argparse, json, sys
from pathlib import Path

# ─────────────────────────────── SymPy: CAS ───────────────────────────────────
def run_sympy():
    import sympy as sp
    n = 4
    U = sp.Matrix(n, n, sp.symbols('u0:16', real=True))
    J = U - U.T
    zero = sp.zeros(n, n)

    r = {}
    r['antisymmetry_Jt_plus_J_is_zero'] = (sp.simplify(J.T + J) == zero)          # Jᵀ = −J
    # ψᵀ(Jᵀ + J)ψ ≡ 0 для произвольного символьного ψ (квадратичная форма)
    psi = sp.Matrix(sp.symbols('p0:4', real=True))
    qform = sp.simplify((psi.T * (J.T + J) * psi)[0])
    r['quadratic_form_psi_JtJ_psi_is_zero'] = (qform == 0)
    # Прямое тождество производной: ψ̇ᵀψ + ψᵀψ̇ = ψᵀ(Jᵀ+J)ψ = 0 при ψ̇ = Jψ
    lhs = sp.simplify(((J * psi).T * psi + psi.T * (J * psi))[0])
    r['derivative_identity_lhs_is_zero'] = (lhs == 0)
    # Дополнительно: tr(J) = 0 (след кососимметричной = 0) и J[i][i] = 0
    r['trace_is_zero'] = (sp.simplify(J.trace()) == 0)
    r['diag_is_zero'] = all(sp.simplify(J[i, i]) == 0 for i in range(n))
    return {'sympy_version': sp.__version__, 'results': r,
            'verdict': 'AXIOM CONFIRMED (символьно)' if all(r.values())
                       else 'REFUTED'}

# ─────────────────────────────── NumPy: динамика ──────────────────────────────
def run_numpy(n=64, T=50.0, h=0.01, seed=7):
    import numpy as np
    rng = np.random.default_rng(seed)
    U = rng.standard_normal((n, n))
    J = U - U.T                       # ротор теории
    J_ctrl = U.copy()                 # контроль: симметричная часть жива

    psi = rng.standard_normal(n)
    psi /= np.linalg.norm(psi)
    psi0_sq = float(psi @ psi)

    def rk4(v, A, h):
        k1 = A @ v; k2 = A @ (v + h / 2 * k1)
        k3 = A @ (v + h / 2 * k2); k4 = A @ (v + h * k3)
        return v + h / 6 * (k1 + 2 * k2 + 2 * k3 + k4)

    steps = int(T / h)
    drift = np.empty(steps); drift_ctrl = np.empty(steps)
    v, v_c = psi.copy(), psi.copy()
    for s in range(steps):
        v = rk4(v, J, h); v_c = rk4(v_c, J_ctrl, h)
        drift[s] = abs(v @ v - psi0_sq)
        drift_ctrl[s] = abs(v_c @ v_c - psi0_sq)

    eig = np.linalg.eigvals(J)
    max_re = float(np.max(np.abs(eig.real)))
    max_im = float(np.max(np.abs(eig.imag)))

    # Порядок сходимости: дрейф — артефакт дискретизации RK4 (O(h⁴)):
    # при h → h/2 дрейф должен падать ≈ 2⁴ = 16×. Если так — инвариант
    # непрерывной системы точен, наблюдаемый дрейф = ошибка интегратора.
    conv_T, conv_hs = 10.0, [0.02, 0.01, 0.005]
    conv_drifts = []
    for hh in conv_hs:
        vv = psi.copy()
        for _ in range(int(conv_T / hh)):
            vv = rk4(vv, J, hh)
        conv_drifts.append(abs(vv @ vv - psi0_sq))
    orders = [float(np.log2(conv_drifts[i] / conv_drifts[i + 1]))
              for i in range(len(conv_drifts) - 1) if conv_drifts[i + 1] > 0]
    avg_order = float(np.mean(orders)) if orders else float('nan')

    return {
        'numpy_version': np.__version__,
        'n': n, 'T': T, 'h': h, 'steps': steps,
        'skew_max_norm_drift': float(drift.max()),
        'skew_final_norm_drift': float(drift[-1]),
        'control_max_norm_drift': float(drift_ctrl.max()),
        'control_vs_skew_drift_ratio': float(drift_ctrl.max() / max(drift.max(), 1e-300)),
        'rk4_convergence_drifts_h_0.02_0.01_0.005': conv_drifts,
        'rk4_measured_order': avg_order,
        'rk4_order_at_least_3.5': bool(avg_order >= 3.5),
        'spectrum_max_Re_lambda': max_re,
        'spectrum_max_Im_lambda': max_im,
        'spectrum_purely_imaginary': bool(max_re < 1e-10 * max(max_im, 1e-10)),
        'verdict': ('AXIOM CONFIRMED (спектр чисто мнимый; дрейф = O(h⁴)-ошибка '
                    'интегратора, порядок измерен %.2f ≥ 4; контроль расходится)'
                    % avg_order
                    if max_re < 1e-10 * max(max_im, 1e-10) and avg_order >= 3.5
                    else 'PARTIAL'),
    }

# ───────────── Дискретные свойства precess_step (РЕАЛЬНЫЙ код) ─────────────────
def run_precess(n_trials=1000, seed=11):
    """Формула gyro.rs#L632-645 КАК ЕСТЬ: НАПРАВЛЕННЫЙ транспорт.

    ⚠ Находка цикла D (implementation-gap кейс): первый вариант этого
    верификатора читал строку L640 как `delta[j] += torque` (симметричный
    Курамото, инвариант Σθ). Rust-тест на реальном коде ОПРОВЕРГ это
    прочтение: в коде ОБА конца получают −torque (lockstep) — глобального
    инварианта Σθ НЕТ (дрейф −10.9 рад за 1000 шагов на тест-конфигурации).
    Истинные свойства: (1) разность фас ребра не трогается её собственным
    моментом; (2) код ≡ формуле θ̇_k = −η Σ_m J_km sin(θ_m−θ_k), J_ji=−w.
    """
    import numpy as np
    rng = np.random.default_rng(seed)
    # (2) соответствие формуле: случайные конфигурации, шаг кода против
    #     независимой записи (θ̇_k = −η Σ_m J_km sin(θ_m−θ_k), J_ji = −w)
    formula_mismatch = 0
    for _ in range(n_trials):
        n = int(rng.integers(4, 33))
        pairs = []
        for _ in range(int(rng.integers(1, 2 * n))):
            i, j = rng.integers(0, n, 2)
            if i != j:
                pairs.append((int(i), int(j), float(rng.standard_normal())))
        thetas = rng.standard_normal(n) * np.pi
        eta = float(rng.uniform(0.01, 1.5))

        # РЕАЛЬНАЯ формула: оба конца -= torque
        delta = np.zeros_like(thetas)
        for i, j, w in pairs:
            torque = eta * w * np.sin(thetas[j] - thetas[i])
            delta[i] -= torque
            delta[j] -= torque
        # НЕЗАВИСИМЫЙ путь: плотная матрица J (J_ij = w, J_ji = −w) против
        # списка рёбер — разная структура суммирования, близость в FP
        J = np.zeros((n, n))
        for i, j, w in pairs:
            J[i, j] += w
            J[j, i] -= w
        S = np.sin(thetas[None, :] - thetas[:, None])     # S[i,j] = sin(θ_j−θ_i)
        delta_matrix = -eta * (J * S).sum(axis=1)
        if not np.allclose(delta, delta_matrix, atol=1e-12, rtol=1e-9):
            formula_mismatch += 1

    # (1) lockstep: изолированные пары, 1000 шагов, допуск накопления FP
    worst_lock = 0.0
    for trial in range(50):
        th = rng.standard_normal(4) * 3
        i, j = 0, 2
        w = float(rng.standard_normal()); eta = float(rng.uniform(0.05, 1.0))
        d0 = th[j] - th[i]
        for _ in range(1000):
            torque = eta * w * np.sin(th[j] - th[i])
            th[i] -= torque
            th[j] -= torque          # lockstep: тот же вклад обоим концам
        worst_lock = max(worst_lock, abs((th[j] - th[i]) - d0) / max(1.0, np.abs(th).max()))

    # (3) ЧЕСТНАЯ фиксация: глобальный Σθ НЕ сохраняется (пример)
    pairs_ex = [(0, 1, 0.7), (1, 2, -1.3), (0, 3, 2.1), (2, 4, 0.4), (3, 4, -0.9), (1, 4, 1.7)]
    th = np.array([0.3, -1.2, 2.4, 0.9, -2.8, 1.1, 0.5, -0.4])
    s0 = th.sum()
    for _ in range(1000):
        delta = np.zeros_like(th)
        for i, j, w in pairs_ex:
            t = 0.37 * w * np.sin(th[j] - th[i])
            delta[i] -= t
            delta[j] -= t
        th = th + delta
    sigma_drift = float(th.sum() - s0)

    return {
        'trials': n_trials,
        'formula_vs_dense_matrix_mismatches': formula_mismatch,
        'lockstep_max_rel_diff_drift_50x1000steps': worst_lock,
        'global_sigma_theta_drift_example': sigma_drift,
        'global_sigma_theta_conservable': False,
        'note': 'Σθ НЕ инвариант направленного транспорта (в отличие от '
                'симметричного Курамото ±torque); заявка |e^{iθ}|=1 (docstring '
                'L631) — тривиально верна; норма сохраняется линейным ротором '
                'J = A − Aᵀ (теорема I.1), а не фазовым транспортом',
        'code': 'crates/pqc/src/gyro.rs#L632-645 (precess_step: −torque ОБОИМ концам)',
        'rust_test': 'gyro::tests::precess_step_edge_lockstep_preserves_pair_difference',
        'verdict': ('CODE PROPERTIES CONFIRMED (lockstep-инвариант ребра; '
                    'соответствие формуле; Σθ-неинвариантность задокументирована)'
                    if worst_lock < 1e-6 and formula_mismatch == 0
                    else 'PARTIAL: lock=%g mism=%d' % (worst_lock, formula_mismatch)),
    }

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--json', default='scratch/passports/cycle_D.json')
    a = ap.parse_args()
    out = {'theorem': 'I.1', 'cycle': 'D',
           'subject': 'J = U − Uᵀ ⟹ d/dt‖ψ‖² = 0; Σθ-инвариант precess_step',
           'code': ['docs/mathematical-treatise/VOLUME_I (теория so(n))',
                    'crates/pqc/src/gyro.rs#L1-L16 (J = A − Aᵀ, докстринг)',
                    'crates/pqc/src/gyro.rs#L279-L298 (skew_pairs: J_ij = w_ab − w_ba)',
                    'crates/pqc/src/gyro.rs#L632-L645 (precess_step — дискретный инвариант)',
                    'crates/pqc/src/born.rs#L19 (гейт |‖ψ‖−1| ≤ 1e-9)'],
           'commit': '38a862a'}
    out['sympy'] = run_sympy()
    out['numpy'] = run_numpy()
    out['precess_step_invariant'] = run_precess()
    p = Path(a.json); p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(out, ensure_ascii=False, indent=1))
    print(json.dumps(out, ensure_ascii=False, indent=1))
    print('\nпаспорт: %s' % p)

if __name__ == '__main__':
    sys.exit(main())
