#!/usr/bin/env python3
"""Диагностика V17 после F12-фикса: почему E/I рельсован? Робастность по seed.
Вопросы:
  D1. E/I по спайкам (a>0.1): куда сходится? GABA/glut рельсы?
  D2. Частоты стрельбы по типам: f_E vs f_I (кто стреляет чаще и почему)
  D3. Робастность: 3 seed × 10k шагов → активность, S, E/I, рельсы
"""
import numpy as np

def virtual_targets(N, F, seed):
    i = np.arange(N)
    return np.stack([(i * 39293 + local * 29101 + seed * 73471) % N for local in range(F)], axis=1)

def run(seed, steps=10000, verbose=False):
    RNG = np.random.default_rng(seed)
    N, F = 600, 16
    types = np.zeros(N); types[: int(N * 0.2)] = 1
    w = np.clip(RNG.exponential(0.3, (N, F)), 0.01, 1.0)
    w[types == 1] *= -0.3
    phases = RNG.uniform(0, 2 * np.pi, (N, F))
    tgt = virtual_targets(N, F, 777)
    GABA, glut = 0.5, 0.5
    DA, HT, NE = 0.3, 0.3, 0.3
    theta_r, gamma_r = 0.0, 0.0
    a = np.zeros(N)
    acts_hist, ei_hist = [], []
    f_rate = np.zeros(N)  # EQ-A23: след частоты по нейронам
    f_sys = 0.0
    dt = 0.001
    for step in range(steps):
        theta_r = (theta_r + dt * 5.0) % (2 * np.pi)
        gamma_r = (gamma_r + dt * 40.0) % (2 * np.pi)
        theta_mod = np.sin(theta_r + 0.01 * np.arange(N))
        if RNG.random() < 0.05:
            a[RNG.random(N) < 0.01] += 0.5
        v = np.zeros(N)
        base = np.where(types == 1, GABA * 1.2, glut * 0.8)
        gain = (1 + 2.0 * NE) * (1 - 0.5 * HT)
        for local in range(F):
            q_eff = np.cos(phases[:, local] + 0.3 * theta_mod[tgt[:, local]])
            v[tgt[:, local]] += a * w[:, local] * base * gain * (0.8 + 0.2 * q_eff)
        v += a * 0.2 * NE
        a = np.tanh(v) * 0.92
        a = np.maximum(0, a + RNG.normal(0, 0.015, N))
        R = N // 100
        act_idx = np.where(a > 0.1)[0]
        a_pre = a.copy()
        for idx in act_idx:
            lo, hi = max(0, idx - R), min(N, idx + R + 1)
            ks = np.arange(lo, hi)
            a[ks] = np.maximum(0, a[ks] - np.exp(-np.abs(ks - idx) / (R / 2)) * 0.1 * a_pre[idx])
        for local in range(F):
            mask = (v[tgt[:, local]] * w[:, local] > 0.1) & (w[:, local] > 0)
            w[mask, local] = np.minimum(1.0, w[mask, local] + 0.0003 * DA)
        active_frac = (a > 0.1).mean()
        acts_hist.append(active_frac)
        # EQ-A23: след частоты
        firing = (a > 0.1).astype(float)
        f_rate = np.where(firing > 0, 0.9 * f_rate + 0.1, 0.99 * f_rate)
        f_sys = 0.99 * f_sys + 0.01 * active_frac
        err = f_sys - 0.05
        corr = err * 0.02
        w[w > 0] *= (1 - corr)
        w[w < 0] *= (1 + corr)
        w = np.clip(w, -1, 1)
        if len(acts_hist) >= 10:
            trend = np.polyfit(range(10), acts_hist[-10:], 1)[0]
            DA = float(np.clip(DA + 0.01 * (trend > 0) - 0.005 * (trend <= 0), 0.1, 0.9))
        sigma_a = np.std(a)
        HT = float(np.clip(0.995 * HT + 0.005 * (1 - min(sigma_a, 1)), 0.1, 0.9))
        CV = np.std(acts_hist[-5:]) / (np.mean(acts_hist[-5:]) + 1e-9)
        NE = float(np.clip(0.99 * NE + 0.01 * min(CV, 2.0), 0.1, 0.9))
        E = ((types == 0) & (a > 0.1)).sum()
        I = ((types == 1) & (a > 0.1)).sum()
        ei = E / max(I, 1)
        ei_hist.append(ei)
        if ei > 6.0:
            GABA = min(0.9, GABA + 0.01); glut = max(0.1, glut - 0.01)
        elif ei < 2.0:
            GABA = max(0.1, GABA - 0.01); glut = min(0.9, glut + 0.01)
    h = np.array(acts_hist)
    last20 = h[-20:]
    S = last20.min() / (last20.max() + 1e-9)
    fE = f_rate[types == 0].mean()
    fI = f_rate[types == 1].mean()
    wE_out = w[types == 0].mean()
    wI_out = w[types == 1].mean()
    return dict(
        seed=seed, final=h[-1000:].mean(), std=h[-1000:].std(), S=S,
        ei_last=np.mean(ei_hist[-1000:]), GABA=GABA, glut=glut,
        DA=DA, HT=HT, NE=NE, fE=fE, fI=fI, wE=wE_out, wI=wI_out,
        max_zero_run=max(len(list(g)) for k, g in __import__('itertools').groupby(h > 0.01) if not k),
    )

print("=== D1-D2: анатомия рельса E/I (seed=777 как в сьюте) ===")
r = run(777)
print(f"активность={r['final']:.4f} ± {r['std']:.4f}  S={r['S']:.3f}")
print(f"E/I(спайки)={r['ei_last']:.2f}  GABA={r['GABA']:.2f} (пол?)  glut={r['glut']:.2f} (потолок?)")
print(f"частоты: f_E={r['fE']:.4f}  f_I={r['fI']:.4f}  отношение f_E/f_I={r['fE']/max(r['fI'],1e-9):.2f}")
print(f"исходящие веса: w_E={r['wE']:.4f}  w_I={r['wI']:.4f} (отрицательные, |w_I| << w_E)")
print(f"макс. серия 'мертвых' шагов (акт<1%): {r['max_zero_run']}")
print(f"модуляторы: DA={r['DA']:.2f} 5HT={r['HT']:.2f} NE={r['NE']:.2f}")

print("\n=== D3: робастность по seed ===")
for s in [777, 1, 42, 20260918, 555]:
    r = run(s)
    rail = "GABA-пол" if r['GABA'] <= 0.1001 else ("GABA-потолок" if r['GABA'] >= 0.8999 else "в коридоре")
    ok = (0.01 < r['final'] < 0.5) and r['S'] < 0.8 and r['max_zero_run'] < 500
    print(f"seed={s:>9}: акт={r['final']:.3f}±{r['std']:.3f} S={r['S']:.2f} "
          f"E/I={r['ei_last']:.2f} {rail:14s} fE/fI={r['fE']/max(r['fI'],1e-9):.2f} "
          f"{'OK' if ok else 'ПРОВАЛ'}")
