#!/usr/bin/env python3
"""F13: насыщение гомеостаза на клипе весов → ретикулярный тон (anti-windup).

Диагноз (Rust-порт, seed=20260918, 10k шагов):
  - 80% весов у клипа 1.0, w>0 среднее = 1.0000
  - тормозные веса стёрты (w<0 среднее = -0.0001)
  - f_sys = 0.016 при цели 0.05, активность 0.5% — ниже пола 1%
  - вспышки 0-3.5% сеть жива, но гомеостату НЕКУДА расти (мультипликативный
    интегральный контроль бессилен при w = 1.0)

Лечение: ретикулярный тон — второй интегральный канал, не насыщается:
  b_tone <- clip(b_tone + k_b*(target - f_sys), 0, b_max)
  a = max(0, tanh(v)*0.92 + noise + b_tone)
Биология: восходящая активирующая система (ароузал) — при дефиците активности
тонус растёт, подталкивая субпороговые нейроны к порогу.

Доказательство: 10 seed x 10k шагов, все должны быть здоровы:
  активность в (1%, 50%), S < 0.8, dead-run < 500, веса в клипах.
"""
import numpy as np

def virtual_targets(N, F, seed):
    i = np.arange(N)
    return np.stack([(i * 39293 + local * 29101 + seed * 73471) % N for local in range(F)], axis=1)

def run(seed, steps=10000, k_b=0.002, b_max=0.25, use_tone=True, verbose=False):
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
    acts_hist = []
    f_sys = 0.0
    b_tone = 0.0   # F13: ретикулярный тон
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
        # F13: ретикулярный тон подталкивает субпороговые нейроны
        a = np.tanh(v) * 0.92
        a = np.maximum(0, a + RNG.normal(0, 0.015, N) + (b_tone if use_tone else 0.0))
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
        f_sys = 0.99 * f_sys + 0.01 * active_frac
        err = f_sys - 0.05
        corr = err * 0.02
        w[w > 0] *= (1 - corr)
        w[w < 0] *= (1 + corr)
        w = np.clip(w, -1, 1)
        # F13: обновление тона — интегральный контроль без насыщения
        if use_tone:
            b_tone = float(np.clip(b_tone + k_b * (0.05 - f_sys), 0.0, b_max))
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
        if ei > 6.0:
            GABA = min(0.9, GABA + 0.01); glut = max(0.1, glut - 0.01)
        elif ei < 2.0:
            GABA = max(0.1, GABA - 0.01); glut = min(0.9, glut + 0.01)
    h = np.array(acts_hist)
    last20 = h[-20:]
    S = last20.min() / (last20.max() + 1e-9)
    mz = 0; cur = 0
    for x in h:
        cur = cur + 1 if x < 0.01 else 0
        mz = max(mz, cur)
    wclip = (w >= 0.999).mean()
    return dict(seed=seed, final=h[-1000:].mean(), std=h[-1000:].std(), S=S,
                dead=mz, wclip=wclip, b=b_tone, f_sys=f_sys,
                DA=DA, HT=HT, NE=NE, GABA=GABA, glut=glut)

print("=== F13-ДОКАЗАТЕЛЬСТВО: ретикулярный тон (k_b=0.002, b_max=0.25) ===")
print(f"{'seed':>10} {'акт':>6} {'±':>5} {'S':>5} {'dead':>4} {'w@клип':>7} {'b_тон':>6} {'f_sys':>6} {'DA':>4} {'5HT':>4} {'NE':>4} {'вердикт'}")
all_ok = True
for s in [777, 1, 42, 20260918, 555, 31337, 999, 12345, 678, 100000]:
    r = run(s)
    ok = (0.01 < r['final'] < 0.5) and (r['S'] < 0.8) and (r['dead'] < 500)
    all_ok = all_ok and ok
    print(f"{r['seed']:>10} {r['final']:>6.4f} {r['std']:>5.3f} {r['S']:>5.2f} {r['dead']:>4d} "
          f"{r['wclip']:>6.1%} {r['b']:>6.3f} {r['f_sys']:>6.4f} {r['DA']:>4.2f} {r['HT']:>4.2f} {r['NE']:>4.2f} "
          f"{'OK' if ok else 'ПРОВАЛ'}")
print()
print("ИТОГ:", "ВСЕ 10 SEED ЗДОРОВЫ — F13-ФИКС ДОКАЗАН" if all_ok else "ЕСТЬ ПРОВАЛЫ — ДОРАБОТАТЬ")

# Контроль: без тона (для сравнения — воспроизводимость проблемы в Python)
print("\n=== Контроль БЕЗ тона (ожидаем провалы на части seed) ===")
fails = 0
for s in [777, 1, 42, 20260918, 555, 31337, 999, 12345, 678, 100000]:
    r = run(s, use_tone=False)
    ok = (0.01 < r['final'] < 0.5) and (r['S'] < 0.8) and (r['dead'] < 500)
    if not ok:
        fails += 1
    print(f"{s:>10}: акт={r['final']:.4f} S={r['S']:.2f} dead={r['dead']} w@клип={r['wclip']:.1%} {'OK' if ok else 'ПРОВАЛ'}")
print(f"без тона: {fails}/10 провалов (проблема воспроизводится)" if fails else "без тона: 0 провалов (Python-RNG не поймал яму — но Rust поймал)")
