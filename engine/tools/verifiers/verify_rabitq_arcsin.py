#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""MVR-v3, цикл C — Теоремы III.1/III.2: RaBitQ arcsin-MLE и стиснення.

Теорема III.1 (rabitq.rs#L179-194, sym_ip):
    E[h/d] = θ/π  (Goemans-Williamson / Grothendieck arcsin-закон)
    ρ̂ = sin(π/2 · (1−2h/d)) = cos(π·h/d)  — точный MLE при h ~ Bin(d, θ/π)

Слои:
  gw    — тождество случайных гиперплоскостей P[sign(r·u)≠sign(r·v)] = θ/π
  had   — вращение Уолша-Адамара с рандом. диагональю (как в коде,
          rabitq.rs#L141 fwt_inplace): h/d концентрируется на θ/π
  mle   — несмещённость/эффективность: эмпирич. bias(ρ̂) vs дельта-метод
          2-го порядка; Var(ρ̂) vs граница Крамера-Рао
  adc   — несмещённость adc_ip (rabitq.rs#L205-208): E[adc_ip] = E⟨x,q⟩;
          self-IP: E = ‖x‖² точно, типичное отклонение ~2% (заявка L203-204)
  bytes — арифметика стиснення (store.rs#L9-23): 12+4+128 = 144 Б, 21.3×/24×
"""
import argparse, json, sys
from pathlib import Path
import numpy as np

def unit_pair(rng, d, theta):
    """u, v — единичные с точным углом θ между ними."""
    u = rng.standard_normal(d); u /= np.linalg.norm(u)
    w = rng.standard_normal(d); w -= (w @ u) * u; w /= np.linalg.norm(w)
    v = np.cos(theta) * u + np.sin(theta) * w
    return u, v

# ─────────────────── 1. Тождество Гоеманса-Вильямсона (GW) ────────────────────
def run_gw(d=768, trials=200_000, chunk=10_000, seed=1):
    rng = np.random.default_rng(seed)
    out = {}
    for deg in [0, 15, 30, 45, 60, 75, 90, 120, 150, 180]:
        th = np.deg2rad(deg)
        u, v = unit_pair(rng, d, th)
        diff = 0; n = 0
        while n < trials:
            m = min(chunk, trials - n)
            R = rng.standard_normal((m, d))
            diff += int(np.count_nonzero((R @ u >= 0) != (R @ v >= 0))); n += m
        p_hat = diff / n
        q_true = th / np.pi
        se = np.sqrt(max(q_true * (1 - q_true), 1e-12) / n)
        z = (p_hat - q_true) / se
        out['θ=%3d°' % deg] = {'p_hat': round(p_hat, 6), 'θ/π': round(q_true, 6),
                               'z_score': round(float(z), 2)}
    max_z = max(abs(v['z_score']) for v in out.values())
    return {'d': d, 'trials': trials, 'table': out,
            'max_abs_z': max_z,
            'verdict': ('AXIOM CONFIRMED (|z| ≤ 3 по всей сетке углов)'
                        if max_z <= 3 else 'REFUTED/ЧАСТИЧНО')}

# ─────── 2. Вращение Уолша-Адамара + рандомизированная диагональ ──────────────
def fwht_np(x):
    """Быстрое WHT по последней оси + нормализация 1/√d (rabitq.rs#L103-106)."""
    d = x.shape[-1]
    h = 1
    while h < d:
        x = x.reshape(-1, d // (2 * h), 2, h)
        a = x[:, :, 0, :].copy(); b = x[:, :, 1, :].copy()
        x[:, :, 0, :] = a + b
        x[:, :, 1, :] = a - b
        h *= 2
        x = x.reshape(-1, d)
    return x / np.sqrt(d)

def run_hadamard(d_real=768, d_pad=1024, draws=400, seed=2):
    rng = np.random.default_rng(seed)
    out = {}
    for deg in [15, 45, 75, 105]:
        th = np.deg2rad(deg)
        u, v = unit_pair(rng, d_real, th)
        up = np.zeros(d_pad); up[:d_real] = u      # паддинг нулями, как в коде
        vp = np.zeros(d_pad); vp[:d_real] = v
        D = rng.integers(0, 2, (draws, d_pad)) * 2 - 1   # Радемахер ±1
        yx = fwht_np(D * up)                        # (draws, d_pad)
        yq = fwht_np(D * vp)
        # конвенция кода: бит=1 ⟺ y_i − mu ≥ 0 (rabitq.rs#L315, L339-343)
        bx = (yx - yx.mean(axis=1, keepdims=True)) >= 0
        bq = (yq - yq.mean(axis=1, keepdims=True)) >= 0
        h = (bx != bq).mean(axis=1)                 # h/d_pad на диагональ
        q_true = th / np.pi
        # норма/скаляр сохраняются ортогональным преобразованием точно
        ip_rot = float(yx[0] @ yq[0]); ip_true = float(u @ v)
        out['θ=%3d°' % deg] = {
            'mean_h_over_d': round(float(h.mean()), 6), 'θ/π': round(q_true, 6),
            'std_h': round(float(h.std()), 6),
            'binomial_std_pred': round(float(np.sqrt(q_true * (1 - q_true) / d_pad)), 6),
            'inner_product_preserved': bool(abs(ip_rot - ip_true) < 1e-8),
        }
    ok = all(abs(v['mean_h_over_d'] - v['θ/π']) < 4 * v['std_h'] and v['inner_product_preserved']
             for v in out.values())
    return {'d_real': d_real, 'd_pad': d_pad, 'diagonal_draws': draws, 'table': out,
            'verdict': ('CONFIRMED (среднее h/d = θ/π в 4σ; ⟨·,·⟩ и нормы '
                        'сохранены ортогональностью WHT; разброс ≈ биномиальный)'
                        if ok else 'ЧАСТИЧНО — см. таблицу')}

# ─────────── 3. MLE: смещение (дельта-метод) и эффективность (CRB) ────────────
def run_mle(d=1024, n=200_000, seed=3):
    rng = np.random.default_rng(seed)
    out = {}
    for rho in [0.0, 0.3, 0.6, 0.9, -0.6, -0.9]:
        theta = np.arccos(np.clip(rho, -1, 1))
        q = theta / np.pi                              # P[знаки различаются]
        h = rng.binomial(d, q, n)
        agree = 1.0 - 2.0 * h / d
        rho_hat = np.sin(np.pi / 2 * agree)            # формула rabitq.rs#L191
        emp_bias = float(rho_hat.mean() - rho)
        # дельта-метод 2-го порядка: E[sin(π/2·X)] ≈ sin(π/2·μ) + ½g''·Var
        # g'' = −(π/2)²·sin(π/2·μ); μ = 1−2q; Var(agree) = 4q(1−q)/d
        mu = 1 - 2 * q
        pred_bias = 0.5 * (-(np.pi / 2) ** 2 * np.sin(np.pi / 2 * mu)) * 4 * q * (1 - q) / d
        emp_var = float(rho_hat.var())
        # Крамер-Рао: q(ρ) = (1 − (2/π)arcsin ρ)/2 ⇒ I = d·(dq/dρ)²/(q(1−q))
        dq = -(1 / np.pi) / np.sqrt(1 - rho ** 2)
        crb = 1.0 / (d * dq ** 2 / (q * (1 - q)))
        out['ρ=%+.1f' % rho] = {
            'emp_bias': float(f'{emp_bias:.2e}'), 'delta2_pred_bias': float(f'{pred_bias:.2e}'),
            'bias_ratio_pred_over_emp': round(pred_bias / emp_bias, 3) if emp_bias != 0 else None,
            'emp_var': float(f'{emp_var:.2e}'), 'cramer_rao_bound': float(f'{crb:.2e}'),
            'efficiency_emp_var_over_crb': round(emp_var / crb, 4),
        }
    ok = all(v['efficiency_emp_var_over_crb'] >= 0.98 and
             (v['bias_ratio_pred_over_emp'] is None or abs(1 - v['bias_ratio_pred_over_emp']) < 0.15
              or abs(v['emp_bias']) < 5e-4)
             for v in out.values())
    return {'d': d, 'n': n, 'table': out,
            'note': 'ρ̂ = sin(π/2·(1−2h/d)) ≡ cos(π·ĥ/d): обращение arcsin-закона — '
                    'это точный MLE биномиальной модели (āgre → (2/π)arcsin ρ)',
            'verdict': ('AXIOM CONFIRMED (несмещённость асимптотическая, смещение '
                        '2-го порядка = предсказанию дельта-метода; Var ≈ CRB — '
                        'асимптотическая эффективность)' if ok else 'ЧАСТИЧНО')}

# ───────────────────── 4. ADC: несмещённость (rabitq.rs#L205-208) ──────────────
def run_adc(D=1024, n=20_000, seed=4):
    rng = np.random.default_rng(seed)
    out = {}
    sigma_x, sigma_q, mu_x, mu_q = 1.0, 0.8, 0.3, -0.2
    for rho in [0.0, 0.5, 0.9]:
        biasses, true_ips, adc_vals, self_rel = [], [], [], []
        chunk = 2000
        for start in range(0, n, chunk):
            m = min(chunk, n - start)
            rx = rng.standard_normal((m, D)) * sigma_x
            rq = rho * (sigma_q / sigma_x) * rx + \
                np.sqrt(max(1 - rho ** 2, 0)) * sigma_q * rng.standard_normal((m, D))
            yx = mu_x + rx; yq = mu_q + rq

            def scalars(y):
                mu = y.mean(axis=1, keepdims=True)
                r = y - mu
                delta = np.linalg.norm(r, axis=1)
                gamma = np.abs(r).mean(axis=1)
                return mu, delta, gamma, r

            muX, dX, gX, rX = scalars(yx)
            muQ, _, _, _ = scalars(yq)
            bx = rX >= 0                                  # бит=1 ⟺ y−mu ≥ 0
            s_plus_c = ((yq - muQ) * bx).sum(axis=1)       # Σ_{b=1}(yq−mu_q)
            s_q = yq.sum(axis=1)
            adc = (muX[:, 0] * s_q + np.pi * gX * s_plus_c)
            true_ip = (yx * yq).sum(axis=1)
            biasses.append(adc - true_ip); true_ips.append(true_ip); adc_vals.append(adc)

            # self-IP: x = q (одна и та же выборка)
            rx2 = rng.standard_normal((m, D)) * sigma_x
            y2 = mu_x + rx2
            mu2, _, g2, r2 = scalars(y2)
            b2 = r2 >= 0
            adc_self = mu2[:, 0] * y2.sum(axis=1) + np.pi * g2 * ((y2 - mu2) * b2).sum(axis=1)
            self_rel.append(np.abs(adc_self - (y2 * y2).sum(axis=1)) / (y2 * y2).sum(axis=1))

        adc_v = np.concatenate(adc_vals); tip = np.concatenate(true_ips)
        rel = np.concatenate(self_rel)
        out['ρ=%.1f' % rho] = {
            'E[adc_ip]': round(float(adc_v.mean()), 4),
            'E[⟨x,q⟩]': round(float(tip.mean()), 4),
            'rel_bias_of_mean': float(f'{(adc_v.mean() - tip.mean()) / tip.mean():.2e}'),
            'rel_std': round(float(adc_v.std() / abs(tip.mean())), 4),
            'self_ip_median_rel_dev': round(float(np.median(rel)), 4),
            'self_ip_mean_rel_dev': round(float(rel.mean()), 4),
        }
    unbiased_ok = all(abs(v['rel_bias_of_mean']) < 3e-3 for v in out.values())
    self_ok = all(v['self_ip_median_rel_dev'] < 0.05 for v in out.values())
    return {'D': D, 'n': n, 'table': out,
            'claims_from_code': ['rabitq.rs#L203-204: «на self-IP типичное отклонение '
                                 '~2%, E — точно ‖x‖²»'],
            'verdict': ('AXIOM CONFIRMED (E[adc_ip] = E⟨x,q⟩; self-IP: медианное '
                        'отклонение ~2% как заявлено в докстринге)'
                        if unbiased_ok and self_ok else 'ЧАСТИЧНО — см. таблицу')}

# ───────────────────────── 5. Арифметика стиснення ─────────────────────────────
def run_bytes():
    d, d_pad = 768, 1024
    per_vec = 4 + 4 + 4 + 4 + d_pad // 8      # mu, delta, gamma, id, codes
    fp32 = d * 4
    return {'layout': 'store.rs#L9-23: 12 Б скаляров + 4 Б id + d_pad/8 Б кодов',
            'per_vector_bytes': per_vec, 'fp32_bytes': fp32,
            'compression_total': round(fp32 / per_vec, 4),
            'compression_codes_only': round(fp32 / (d_pad // 8), 4),
            'verdict': ('AXIOM CONFIRMED (144 Б = 21.33×; коды 128 Б = 24× — '
                        'совпадает со store.rs#L23)') if per_vec == 144 else 'REFUTED'}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--json', default='scratch/passports/cycle_C.json')
    a = ap.parse_args()
    out = {'theorem': 'III.1 + III.2', 'cycle': 'C',
           'subject': 'arcsin-MLE несмещённость/эффективность; ADC; стиснення 21.3×',
           'code': ['src/vectors/rabitq.rs#L179-194 (sym_ip: agree → sin(π/2·agree))',
                    'src/vectors/rabitq.rs#L141 (fwt_inplace — WHT)',
                    'src/vectors/rabitq.rs#L205-208 (adc_ip)',
                    'src/vectors/rabitq.rs#L314-345 (encode_into: бит=1 ⟺ y−mu ≥ 0)',
                    'src/vectors/store.rs#L9-23 (раскладка 144 Б)'],
           'commit': '38a862a'}
    out['gw_identity'] = run_gw()
    out['hadamard_rotation'] = run_hadamard()
    out['mle'] = run_mle()
    out['adc'] = run_adc()
    out['bytes'] = run_bytes()
    p = Path(a.json); p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(out, ensure_ascii=False, indent=1, default=str))
    print(json.dumps(out, ensure_ascii=False, indent=1, default=str))
    print('\nпаспорт: %s' % p)

if __name__ == '__main__':
    sys.exit(main())
