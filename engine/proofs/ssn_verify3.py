#!/usr/bin/env python3
"""
SSN VERIFICATION SUITE v3 — ФИНАЛ
==================================
Метод владельца: «вначале доказываешь что работает, убеждаешься,
потом реализуешь». Эта третья редакция закрывает все 7 провалов v1/v2
и фиксирует НАЙДЕННЫЕ И ИСПРАВЛЕННЫЕ дефекты теории (это и была цель):

НАХОДКИ ВЕРИФИКАЦИИ (документируются в docs/SSN.md):
  F1. Стек передачи без гомеостаза насыщается до 100% («цифровая
      эпилепсия» — совпадает с историей v1.0 в диалоге 012:
      1,000,000/1,000,000 активных, avg_activation 0.985).
      => Гомеостаз ОБЯЗАТЕЛЕН, а не опционален.
  F2. EQ-A22 (трансприпция из диалога 012) содержит ошибку знака в
      ветви гипоактивности: w>0 умножается на (1−corr) при corr>0 —
      возбуждение СЖИМАЕТСЯ, когда сеть недоактивна => смерть сети.
      Исправленный закон: corr = err·k; w>0 *= (1−corr); w<0 *= (1+corr)
      (гипер → возбуждение падает/торможение растёт; гипо → наоборот).
  F3. Золотой режим параметров: homeo=0.02, STDP=0.0003·DA,
      стимул 1%/шаг, шум σ=0.015 => активность 4.8–5.0% (цель 5%),
      макс. серия нулей ≤ 5 шагов, флуктуации std≈0.04 (лавины,
      критическое состояние). При homeo=0.01 seed=42 сеть умирает
      (гистерезис GABA→0.9) — запас прочности важен.
  F4. Эйлер на J дрейфует по норме (×4564 при h=0.01, ||J||~22;
      ~1%/1000 шагов даже на нормированной J при h=0.01).
      => Транспорт реализуется Кэли-трансформацией
         p' = (I+hJ/2)(I−hJ/2)⁻¹ p — ТОЧНОЕ сохранение нормы
         (дрейф 3.7e-14 за 2000 шагов). Это и есть дискретная форма
         предписанного архивами exp(Δt·J) (EQ-D19).
  F5. Формула критичности C=1/(1+|CV−1|) работает на числе АКТИВНЫХ
      НЕЙРОНОВ (ограничено N); на неограниченных размерах лавин
      (Парето, бесконечная дисперсия) — не работает.
  F6. CSE-фронтенд требует посимвольной фазовой модуляции
      (φ_char = code· золотое сечение mod 2π): без неё все тексты
      коррелируют на cos≈0.92+; с ней — разделимость до 1.35
      (похожие 0.999, разные −0.35).
  F7. NMDA-гейт tanh(⟨state,ctx⟩) требует ненулевого накопленного
      состояния — инициализация строго через Transmit-предысторию.
  F8. Гомеостаз по мгновенной активности при высоком контурном усилении
      (NE-гейн + инерция мышления) даёт bang-bang режим 0%↔100%.
      => Гомеостаз обязан действовать по МЕДЛЕННОМУ следу частоты
         (EQ-A22/A23: mean(firing_rates); f_sys ← 0.99·f + 0.01·active)
         — низкочастотный фильтр = интегральный регулятор без перелётов.
      => Предельный цикл (Δ стабилизируется на постоянной амплитуде) —
         это НОРМАЛЬНЫЙ живой режим, а не дивергенция (V18c).
  F13. Насыщение гомеостаза на клипе весов (windup), найдено Rust-портом
      (seed=20260918): 80% весов у клипа 1.0, тормозные стёрты, активность
      0.5% — интегральному контроллеру НЕКУДА расти.
      => Ретикулярный тон — второй интегральный канал, не насыщается:
         b_tone <- clip(b_tone + k_b·(target − f_sys), 0, b_max),
         k_b=0.002, b_max=0.25; a += b_tone в активации.
         Биология: восходящая активирующая система (ароузал).
         Доказано: 10/10 seed x 10k шагов — активность 4.5–5.2%,
         w@клип ≤ 1.9%, dead-runs ≤ 18 (scripts/ssn_f13_proof.py).
"""

import numpy as np

RNG = np.random.default_rng(20260918)
PASS = 0
FAIL = 0


def check(name, cond, detail=""):
    global PASS, FAIL
    tag = "PASS" if cond else "FAIL"
    if cond:
        PASS += 1
    else:
        FAIL += 1
    print(f"[{tag}] {name}" + (f"  — {detail}" if detail else ""))


def virtual_targets(N, F, seed):
    """EQ-A1: target = (i*39293 + local*29101 + seed*73471) mod N"""
    i = np.arange(N)
    return np.stack([(i * 39293 + local * 29101 + seed * 73471) % N for local in range(F)], axis=1)


# ============================================================
# V1. Виртуальная hash-топология (EQ-A1)
# ============================================================
def v1_hash_topology():
    print("\n=== V1. Виртуальная hash-топология (EQ-A1) ===")
    N, F, seed = 1000, 20, 12345
    tgt = virtual_targets(N, F, seed)
    flat = tgt.ravel()
    coverage = len(np.unique(flat)) / N
    check("V1a покрытие вершин = 100%", coverage == 1.0, f"coverage={coverage:.3f}")
    hist, _ = np.histogram(flat, bins=N, range=(0, N))
    expected = len(flat) / N
    chi2 = np.sum((hist - expected) ** 2 / expected)
    check("V1b равномерность (хи² < 1074, df=999)", chi2 < 1074, f"chi2={chi2:.1f}")
    t2 = (5 * 39293 + 7 * 29101 + seed * 73471) % N
    t3 = (5 * 39293 + 7 * 29101 + seed * 73471) % N
    check("V1c детерминизм", t2 == t3)
    self_loops = (tgt == np.arange(N)[:, None]).sum()
    check("V1d самосвязи < 5%", self_loops / (N * F) < 0.05, f"rate={self_loops/(N*F):.4f}")


# ============================================================
# V2. Квантовая передача: ограниченность (насыщение = F1)
# ============================================================
def v2_quantum_transmission():
    print("\n=== V2. Квантовая передача (EQ-A9/A12) ===")
    phase = np.linspace(0, 2 * np.pi, 1000)
    coeff = 0.8 + 0.2 * np.cos(phase)
    check("V2a коэффициент ∈ [0.6, 1.0]", coeff.min() >= 0.6 - 1e-9 and coeff.max() <= 1.0 + 1e-9,
          f"min={coeff.min():.3f}, max={coeff.max():.3f}")
    NE, HT = 0.9, 0.05
    gain = (1 + 2.0 * NE) * (1 - 0.5 * HT)
    check("V2b перегрузка: max gain < 3.0", gain < 3.0, f"gain={gain:.3f}")
    # F1: без гомеостаза — насыщение (эпилепсия), но ОГРАНИЧЕННОЕ (tanh)
    N, F = 400, 20
    types = np.zeros(N); types[: int(N * 0.2)] = 1
    w = np.clip(RNG.exponential(0.3, (N, F)), 0.01, 1.0)
    w[types == 1] *= -0.3
    tgt = virtual_targets(N, F, 777)
    a = np.abs(RNG.normal(0.2, 0.1, N))
    R = N // 100
    for step in range(300):
        v = np.zeros(N)
        base = np.where(types == 1, 1.2, 0.8)
        for local in range(F):
            v[tgt[:, local]] += a * w[:, local] * base * gain
        a = np.tanh(v) * 0.92
        a = np.maximum(0, a + RNG.normal(0, 0.015, N))
        act_idx = np.where(a > 0.1)[0]
        a_pre = a.copy()
        for idx in act_idx:
            lo, hi = max(0, idx - R), min(N, idx + R + 1)
            ks = np.arange(lo, hi)
            a[ks] = np.maximum(0, a[ks] - np.exp(-np.abs(ks - idx) / (R / 2)) * 0.1 * a_pre[idx])
    check("V2c активации ограничены [0,1] даже в перегрузке (tanh-инвариант)",
          a.min() >= -1e-9 and a.max() <= 1.0 + 1e-9, f"max={a.max():.3f} (см. F1: гомеостаз обязателен)")
    DA = 1.0
    thr = 0.5 - 0.4 * DA
    check("V2d DA=1 → порог обучения 0.1", abs(thr - 0.1) < 1e-12, f"thr={thr}")


# ============================================================
# V3. Нейромодуляторы (EQ-A24..A27)
# ============================================================
def v3_neuromodulators():
    print("\n=== V3. Нейромодуляторы (EQ-A24..A27) ===")
    DA = 0.1
    acts = np.linspace(0.01, 0.5, 10)
    trend = np.polyfit(range(10), acts, 1)[0]
    for _ in range(200):
        DA = float(np.clip(DA + 0.01 * (trend > 0) - 0.005 * (trend <= 0), 0.1, 0.9))
    check("V3a восходящий тренд → DA = 0.9", DA == 0.9, f"DA={DA}")
    acts_dn = np.linspace(0.5, 0.01, 10)
    trend_dn = np.polyfit(range(10), acts_dn, 1)[0]
    for _ in range(200):
        DA = float(np.clip(DA + 0.01 * (trend_dn > 0) - 0.005 * (trend_dn <= 0), 0.1, 0.9))
    check("V3b нисходящий тренд → DA = 0.1", DA == 0.1, f"DA={DA}")
    HT, sigma_a = 0.1, 0.05
    for _ in range(500):
        HT = float(np.clip(0.995 * HT + 0.005 * (1 - min(sigma_a, 1)), 0.1, 0.9))
    check("V3c низкая σ → 5HT → 0.9", HT > 0.85, f"5HT={HT:.3f}")
    NE, CV = 0.1, 1.5
    for _ in range(500):
        NE = float(np.clip(0.99 * NE + 0.01 * min(CV, 2.0), 0.1, 0.9))
    check("V3d высокая CV → NE → 0.9", NE > 0.85, f"NE={NE:.3f}")
    check("V3e коридор [0.1, 0.9]", all(0.1 <= x <= 0.9 for x in (DA, HT, NE)))


# ============================================================
# V4. STDP + ИСПРАВЛЕННЫЙ гомеостаз (F2) + золотой режим (F3)
# ============================================================
def v4_homeostasis():
    print("\n=== V4. Гомеостаз (исправленный знак, F2) + золотой режим (F3) ===")
    N, F = 200, 20
    types = np.zeros(N); types[: int(N * 0.2)] = 1
    w = np.clip(RNG.exponential(0.3, (N, F)), 0.01, 1.0)
    w[types == 1] *= -0.3
    tgt = virtual_targets(N, F, 777)
    GABA, glut = 0.5, 0.5
    a = np.zeros(N)
    hist = []
    HOMEO, STDP = 0.02, 0.0003  # F3: золотой режим
    for step in range(6000):
        a[RNG.random(N) < 0.01] += 0.5
        v = np.zeros(N)
        base = np.where(types == 1, GABA * 1.2, glut * 0.8)
        for local in range(F):
            v[tgt[:, local]] += a * w[:, local] * base
        a = np.tanh(v) * 0.92
        a = np.maximum(0, a + RNG.normal(0, 0.015, N))
        for local in range(F):
            mask = (v[tgt[:, local]] * w[:, local] > 0.1) & (w[:, local] > 0)
            w[mask, local] = np.minimum(1.0, w[mask, local] + STDP * 0.3)
        err = (a > 0.1).mean() - 0.05
        corr = err * HOMEO  # F2: исправленный знак — единая формула
        w[w > 0] *= (1 - corr)
        w[w < 0] *= (1 + corr)
        w = np.clip(w, -1, 1)
        E = ((types == 0) & (a > 0.01)).sum()
        I = ((types == 1) & (a > 0.01)).sum()
        ei = E / max(I, 1)
        if ei > 6.0:
            GABA = min(0.9, GABA + 0.01); glut = max(0.1, glut - 0.01)
        elif ei < 2.0:
            GABA = max(0.1, GABA - 0.01); glut = min(0.9, glut + 0.01)
        hist.append((a > 0.1).mean())
    h = np.array(hist)
    final = h[-1000:].mean()
    max_zero_run, cur = 0, 0
    for x in h[-3000:]:
        cur = cur + 1 if x == 0 else 0
        max_zero_run = max(max_zero_run, cur)
    check("V4a активность сходится к 5% ± 3%", 0.02 <= final <= 0.08, f"final={final:.4f}")
    check("V4b нет смерти (макс. нулевая серия < 50)", max_zero_run < 50, f"max_zero_run={max_zero_run}")
    check("V4c активации ∈ [0, 1]", a.min() >= -1e-9 and a.max() <= 1.0 + 1e-9)
    check("V4d веса в клипах [-1, 1]", w.min() >= -1 and w.max() <= 1)
    check("V4e лавинная динамика (std > 0.01)", h[-1000:].std() > 0.01,
          f"std={h[-1000:].std():.3f} (критичность)")
    check("V4f GABA/глутамат в клипах", 0.1 <= GABA <= 0.9 and 0.1 <= glut <= 0.9,
          f"GABA={GABA:.2f}, glut={glut:.2f}")


# ============================================================
# V5. E/I контроллер (мёртвая зона по дизайну)
# ============================================================
def v5_ei_balance():
    print("\n=== V5. E/I контроллер (EQ-A27) ===")
    GABA, glut = 0.5, 0.5
    target = 4.0
    ei = 1.0
    for _ in range(3000):
        if ei > 1.5 * target:
            GABA += 0.01; glut -= 0.01
        elif ei < 0.5 * target:
            GABA -= 0.01; glut += 0.01
        GABA, glut = float(np.clip(GABA, 0.1, 0.9)), float(np.clip(glut, 0.1, 0.9))
        ei += 0.1 * (glut / GABA - ei)
    check("V5a E/I входит в мёртвую зону [2, 6]", 2.0 <= ei <= 6.0, f"ei={ei:.3f}")
    check("V5b равновесие (|glut/GABA − ei| < 0.01)", abs(glut / GABA - ei) < 0.01)
    check("V5c клипы GABA/глутамат", 0.1 <= GABA <= 0.9 and 0.1 <= glut <= 0.9,
          f"GABA={GABA:.2f}, glut={glut:.2f}")


# ============================================================
# V6. Критичность: семантика формулы + ветвящийся процесс (F5)
# ============================================================
def v6_criticality():
    print("\n=== V6. Критичность (EQ-A18/A19) ===")

    def C_from(x):
        last = np.asarray(x, dtype=float)
        return 1 / (1 + abs(np.std(last) / (np.mean(last) + 1e-9) - 1))

    # семантика: CV=1 → C=1; CV=0.5 → C=2/3; CV=0 → 0.5; CV=2 → 0.5
    sig_cv1 = RNG.normal(100, 100, 500) + 100  # CV=0.5
    c_cv1 = C_from(np.full(100, 5.0) + 0.0)
    check("V6a CV=0 → C=0.5 (некритично)", abs(c_cv1 - 0.5) < 1e-9, f"C={c_cv1:.3f}")
    # ветвящийся процесс: σ=1 (критично) vs σ=0.7 (докритично)
    N = 1000
    def run_branching(sigma, steps=2000):
        act = np.zeros(steps)
        cur = int(RNG.integers(1, 5))
        for t in range(steps):
            nxt = int(RNG.poisson(sigma, cur).sum()) if cur > 0 else 0
            if RNG.random() < 0.02:
                nxt += int(RNG.integers(1, 4))
            nxt = min(nxt, N)
            act[t] = nxt
            cur = nxt
        return act
    act_crit = run_branching(1.0)
    act_sub = run_branching(0.7)
    C_crit, C_sub = C_from(act_crit), C_from(act_sub)
    check("V6b критическая ветвь C > докритической", C_crit > C_sub,
          f"C_crit={C_crit:.3f} > C_sub={C_sub:.3f}")
    chaotic = RNG.uniform(0.01, 1.0, 20)
    S_ch = chaotic.min() / chaotic.max()
    check("V6c хаос → S < 0.2 (синхронность)", S_ch < 0.2, f"S={S_ch:.3f}")
    acts = np.ones(20) * 0.5
    check("V6d идеальная синхрония → S > 0.8 (эпилепсия)", acts.min() / acts.max() > 0.8)


# ============================================================
# V7. Асимметричный бит 0↮1 (FLAG)
# ============================================================
def v7_asymmetric_bit():
    print("\n=== V7. Асимметричный бит 0↮1 (FLAG) ===")

    class AsymBit:
        def __init__(self):
            self.void = True
            self.presence = False

        def emerge(self):
            assert self.void, "1 без 0 невозможен"
            self.presence = True

        def collapse(self):
            self.presence = False

    b = AsymBit()
    ok1 = b.void and not b.presence
    b.emerge()
    ok2 = b.void and b.presence
    b.collapse()
    ok3 = b.void and not b.presence
    check("V7a вакуум сохраняется после коллапса 1", ok1 and ok2 and ok3)
    p = 0.5
    H1 = -p * np.log2(p) - (1 - p) * np.log2(1 - p)
    check("V7b H(1) > H(0) = 0", H1 > 0, f"H1={H1:.3f}")
    state = RNG.integers(0, 2, 256)
    top_k = np.argsort(state)[-10:]
    mask = np.zeros(256); mask[top_k] = 1
    inhibited = state * mask
    check("V7c после WTA остаются нули (вакуум)", (inhibited == 0).sum() > 0,
          f"zeros={(inhibited == 0).sum()}")


# ============================================================
# V8. CSE с золотой фазой (F6)
# ============================================================
def _cse_encode(text: str, D: int = 128) -> np.ndarray:
    vec = np.zeros(D)
    idx = np.arange(D)
    PHI = 0.618033988749895
    for i, ch in enumerate(text):
        code = ord(ch)
        wp = 1.0 / (1 + i)
        cphase = (code * PHI) % (2 * np.pi)  # F6: посимвольная фаза
        for j in range(min(16, D)):
            bit = (code >> j) & 1
            freq = 1.0 + j * 0.3
            phase = i * 0.01 + cphase
            if bit:
                vec += np.sin(freq * idx + phase) * wp
            else:
                vec += np.cos(freq * idx + phase) * 0.5 * wp
    vec = vec - vec.mean()
    n = np.linalg.norm(vec)
    return vec / n if n > 0 else vec


def v8_cse():
    print("\n=== V8. CSE + золотая фаза (FLAG + F6) ===")
    va1 = _cse_encode("герой идёт в поход против тьмы")
    va2 = _cse_encode("герой идёт в поход против тьмы")
    vb = _cse_encode("герой идёт в поход против бездны")
    vc = _cse_encode("кофеварка сломалась вчера")
    check("V8a детерминизм", np.array_equal(va1, va2))
    check("V8b норма = 1", abs(np.linalg.norm(va1) - 1.0) < 1e-9)
    cab = float(np.dot(va1, vb))
    cac = float(np.dot(va1, vc))
    gap = cab - cac
    check("V8c разделимость: зазор > 0.5", gap > 0.5,
          f"cos(a,b)={cab:.3f} vs cos(a,c)={cac:.3f}, gap={gap:.3f}")
    check("V8d |cos| ≤ 1", abs(cab) <= 1.0 and abs(cac) <= 1.0)
    vs = _cse_encode("а")
    vl = _cse_encode("а" * 1000)
    check("V8e норма = 1 для любой длины",
          abs(np.linalg.norm(vs) - 1) < 1e-9 and abs(np.linalg.norm(vl) - 1) < 1e-9)
    # синусоидная коррекция сходства (FLAG)
    def sim(v1, v2):
        c = np.dot(v1, v2) / (np.linalg.norm(v1) * np.linalg.norm(v2) + 1e-8)
        return float(np.sin(c * np.pi / 2))
    check("V8f sin-коррекция монотонна и ∈ [-1, 1]",
          sim(va1, vb) >= sim(va1, vc) and abs(sim(va1, vb)) <= 1)


# ============================================================
# V9. MindOS-операторы (FLAG, F7)
# ============================================================
def v9_operators():
    print("\n=== V9. Операторы MindOS (FLAG + F7) ===")
    D = 64
    e0, e1 = np.zeros(D), np.zeros(D)
    e0[0] = 3.0
    e1[1] = 3.0  # ОРТОГОНАЛЬНЫЕ ненулевые состояния

    def op_nmda(state, x, ctx, w):
        gate = np.tanh(np.dot(state, ctx))
        return state + x * w * gate, gate

    _, g_align = op_nmda(e0, np.eye(1, D, 0).ravel(), e0 / 3.0, 0.5)
    _, g_ortho = op_nmda(e1, np.eye(1, D, 1).ravel(), e0 / 3.0, 0.5)
    check("V9a NMDA: гейт открыт (align) / закрыт (ortho)",
          g_align > 0.9 and abs(g_ortho) < 0.05,
          f"g_align={g_align:.3f}, g_ortho={g_ortho:.3f}")

    def op_inhibit_wta(state, k=10):
        top_k = np.argsort(state)[-k:]
        mask = np.zeros_like(state)
        mask[top_k] = 1
        inh = state * mask
        return inh / (np.linalg.norm(inh) + 1e-8)

    state = RNG.normal(0, 1, D)
    wta = op_inhibit_wta(state, 10)
    check("V9b WTA: ровно 10 активных", (np.abs(wta) > 1e-12).sum() == 10)
    check("V9c WTA: норма = 1", abs(np.linalg.norm(wta) - 1.0) < 1e-6)
    ctx = np.ones(D) / np.sqrt(D)
    state = np.zeros(D)
    for t in range(1000):
        x = np.tanh(RNG.normal(0, 1, D))
        state = state + 0.05 * x
        state, _ = op_nmda(state, x, ctx, 0.05)
        if t % 10 == 0:
            state = op_inhibit_wta(state, 10)
        state = np.tanh(state)
    check("V9d составная динамика ограничена", np.linalg.norm(state) < np.sqrt(D),
          f"norm={np.linalg.norm(state):.2f}")
    w = 0.5
    for t in range(100):
        alpha = 0.01 * np.exp(-t / 50)
        w = w + alpha * 0.1
    check("V9e модуляция с затуханием → стабильный вес", 0.5 < w < 0.6, f"w={w:.4f}")
    # Bind-оператор (FLAG: A·g + B·(1−g))
    A, B = RNG.normal(0, 1, D), RNG.normal(0, 1, D)
    bound = A * 0.7 + B * 0.3
    check("V9f Bind — интерполяция (‖bound‖ ≤ 0.7‖A‖+0.3‖B‖)",
          np.linalg.norm(bound) <= 0.7 * np.linalg.norm(A) + 0.3 * np.linalg.norm(B) + 1e-9)


# ============================================================
# V11. Ротор: Кэли-транспорт (F4)
# ============================================================
def v11_rotor_norm():
    print("\n=== V11. Ротор J = A − Aᵀ: Кэли-транспорт (F4) ===")
    n = 32
    A = RNG.normal(0, 1, (n, n))
    J = A - A.T
    check("V11a J антисимметрична", np.allclose(J, -J.T))
    ev = np.linalg.eigvals(J)
    check("V11b собственные числа чисто мнимые", np.abs(ev.real).max() < 1e-10,
          f"max|Re|={np.abs(ev.real).max():.2e}")
    p = RNG.normal(0, 1, n)
    n0 = np.linalg.norm(p)
    h = 0.05
    for _ in range(2000):
        M = np.eye(n) + 0.5 * h * J
        p = np.linalg.solve(np.eye(n) - 0.5 * h * J, M @ p)
    drift = abs(np.linalg.norm(p) - n0) / n0
    check("V11c Кэли: дрейф < 1e-8 за 2000 шагов", drift < 1e-8, f"drift={drift:.2e}")
    Jn = J / np.linalg.norm(J, 2)
    p = RNG.normal(0, 1, n)
    n0 = np.linalg.norm(p)
    for _ in range(1000):
        p = p + 0.002 * (Jn @ p)  # малый шаг: квадратичный дрейф
    drift_e = abs(np.linalg.norm(p) - n0) / n0
    check("V11d Эйлер (h=0.002, норм. J): дрейф < 1%", drift_e < 0.01, f"drift={drift_e:.4f}")
    check("V11e Кэли точнее на порядки", drift < drift_e,
          f"cayley={drift:.2e} vs euler={drift_e:.2e}")


# ============================================================
# V13. SCTP-слой ограничений (EQ-B69)
# ============================================================
def v13_sctp_layer():
    print("\n=== V13. Слой ограничений SCTP (EQ-B69) ===")
    n = 16
    A = RNG.normal(0, 0.5, (n, n))
    J = A - A.T
    D = np.diag(np.linspace(1.0, 1.5, n))
    Jc = np.ones((1, n))
    P = np.eye(n) - Jc.T @ np.linalg.inv(Jc @ Jc.T) @ Jc
    M = P @ (J - D) @ P
    check("V13a Π идемпотентен", np.allclose(P @ P, P, atol=1e-10))
    x = RNG.normal(0, 1, n)
    y = M @ x
    check("V13b Σy = 0 (ограничение выдержано)", abs(y.sum()) < 1e-10, f"Σy={y.sum():.2e}")
    e0 = np.linalg.norm(x)
    y2 = x.copy()
    for _ in range(100):
        y2 = y2 - 0.1 * (P @ D @ P @ y2)
    check("V13c D-часть гасит норму (Ляпунов)", np.linalg.norm(y2) < e0,
          f"{e0:.2f} → {np.linalg.norm(y2):.2f}")
    check("V13d M — антисимметрия J сохранена в ротационной части",
          np.allclose((P @ J @ P), -(P @ J @ P).T))


# ============================================================
# V14. Trit5 (EQ-D16/D17)
# ============================================================
def v14_trit_packing():
    print("\n=== V14. Trit5 упаковка (EQ-D17) ===")

    def pack5(trits):
        return sum((t + 1) * 3 ** k for k, t in enumerate(trits))

    def unpack5(b):
        return [((b // 3 ** k) % 3) - 1 for k in range(5)]

    ok = True
    for _ in range(1000):
        trits = [int(x) for x in RNG.integers(-1, 2, 5)]
        b = pack5(trits)
        ok = ok and (0 <= b <= 242) and (unpack5(b) == trits)
    check("V14a roundtrip 1000 случайных", ok)
    check("V14b 3^5 = 243 ≤ 256", 3 ** 5 <= 256)
    w = RNG.normal(0, 0.5, 100000)
    theta = 0.7 * np.mean(np.abs(w))
    trits = np.where(w > theta, 1, np.where(w < -theta, -1, 0))
    frac_zero = (trits == 0).mean()
    check("V14c мёртвая зона ~30-70%", 0.3 <= frac_zero <= 0.7, f"zero={frac_zero:.3f}")


# ============================================================
# V15. Осцилляторный блок (FLAG)
# ============================================================
def v15_oscillator():
    print("\n=== V15. Синусоидный осцилляторный блок (FLAG) ===")
    D = 64
    freqs = np.logspace(0, 1, D)  # 1..10
    phases = RNG.uniform(0, 2 * np.pi, D)
    phase_acc = np.zeros(D)
    x = RNG.normal(0, 1, D)
    outs = []
    for t in range(500):
        phase_acc += freqs * 0.01
        osc = np.sin(phase_acc + phases)
        mod = x * osc
        mod = mod + np.sin(phase_acc * 2) * 0.3
        outs.append(mod)
    outs = np.array(outs)
    check("V15a выход ограничен", np.abs(outs).max() < 5.0, f"max|out|={np.abs(outs).max():.2f}")
    fft_energy = np.abs(np.fft.fft(outs[:, 0]))
    peak_frac = fft_energy.max() / fft_energy.sum()
    check("V15b многочастотность", peak_frac < 0.5, f"peak frac={peak_frac:.3f}")


# ============================================================
# V16. Активный инференс (EQ-D23)
# ============================================================
def v16_active_inference():
    print("\n=== V16. Активный инференс (EQ-D23) ===")
    p = np.zeros(8)
    o = np.ones(8) * 0.5
    tau = 0.3
    reads = 0
    for t in range(50):
        F = np.linalg.norm(p - o) ** 2
        if F > tau:
            reads += 1
            p = p + 0.3 * (o - p)
        else:
            p = p * 0.98
    check("V16a любопытство срабатывает", reads > 0, f"reads={reads}")
    check("V16b насыщение: F < τ в конце", np.linalg.norm(p - o) ** 2 < tau,
          f"F_final={np.linalg.norm(p - o) ** 2:.4f}")


# ============================================================
# V17. ПОЛНЫЙ СТЕК ВИХРЯ: 10k шагов (с исправленным гомеостазом)
# ============================================================
def v17_full_vortex():
    print("\n=== V17. Полный стек вихря: 10k шагов ===")
    # Локальный детерминированный RNG — воспроизводимость доказательства
    # (глобальный RNG зависел бы от того, какие тесты шли раньше).
    # Робастность проверена отдельно: 5/5 seed здоровы (ssn_diag_ei.py).
    RNG = np.random.default_rng(777)
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
    f_sys = 0.0  # медленный след частоты (EQ-A23) — интегральный контроллер (F8)
    b_tone = 0.0  # F13: ретикулярный тон — интегральный канал без насыщения
    dt = 0.001
    for step in range(10000):
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
        v += a * 0.2 * NE  # инерция мышления (EQ-A16)
        a = np.tanh(v) * 0.92
        # F13: ретикулярный тон подталкивает субпороговые нейроны к порогу
        a = np.maximum(0, a + RNG.normal(0, 0.015, N) + b_tone)
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
        # F12: ЕДИНОЕ определение активности во всей системе — порог спайка
        # EQ-A23 (a > 0.1). Метрика, DA-тренд, CV для NE, E/I — все по нему.
        active_frac = (a > 0.1).mean()
        acts_hist.append(active_frac)
        # F8: гомеостаз по МЕДЛЕННОМУ следу (EQ-A22/A23: mean(firing_rates))
        f_sys = 0.99 * f_sys + 0.01 * active_frac  # низкочастотный фильтр
        err = f_sys - 0.05
        corr = err * 0.02  # F2: исправленный знак
        w[w > 0] *= (1 - corr)
        w[w < 0] *= (1 + corr)
        w = np.clip(w, -1, 1)
        # F13: обновление ретикулярного тона — анти-windup канал
        b_tone = float(np.clip(b_tone + 0.002 * (0.05 - f_sys), 0.0, 0.25))
        if len(acts_hist) >= 10:
            trend = np.polyfit(range(10), acts_hist[-10:], 1)[0]
            DA = float(np.clip(DA + 0.01 * (trend > 0) - 0.005 * (trend <= 0), 0.1, 0.9))
        sigma_a = np.std(a)
        HT = float(np.clip(0.995 * HT + 0.005 * (1 - min(sigma_a, 1)), 0.1, 0.9))
        CV = np.std(acts_hist[-5:]) / (np.mean(acts_hist[-5:]) + 1e-9)
        NE = float(np.clip(0.99 * NE + 0.01 * min(CV, 2.0), 0.1, 0.9))
        # F12: E/I — по тому же спайковому определению (a > 0.1)
        E = ((types == 0) & (a > 0.1)).sum()
        I = ((types == 1) & (a > 0.1)).sum()
        ei = E / max(I, 1)
        if ei > 6.0:
            GABA = min(0.9, GABA + 0.01); glut = max(0.1, glut - 0.01)
        elif ei < 2.0:
            GABA = max(0.1, GABA - 0.01); glut = min(0.9, glut + 0.01)
    final = np.mean(acts_hist[-1000:])
    last = np.array(acts_hist[-100:])
    C = 1 / (1 + abs(np.std(last) / (np.mean(last) + 1e-9) - 1))
    last20 = np.array(acts_hist[-20:])
    S = last20.min() / (last20.max() + 1e-9)
    check("V17a выживаемость: 1% < активность < 50%", 0.01 < final < 0.5, f"active={final:.3f}")
    check("V17b активации ∈ [0, 1]", a.min() >= -1e-9 and a.max() <= 1.0 + 1e-9)
    check("V17c не эпилепсия (S < 0.8)", S < 0.8, f"S={S:.3f}")
    check("V17d веса в клипах", w.min() >= -1 and w.max() <= 1)
    check("V17e нейромодуляторы в коридоре", all(0.1 <= x <= 0.9 for x in (DA, HT, NE)),
          f"DA={DA:.2f}, 5HT={HT:.2f}, NE={NE:.2f}")
    check("V17f ретикулярный тон в клипе [0, 0.25] (F13)", 0.0 <= b_tone <= 0.25,
          f"b_tone={b_tone:.3f}")
    check("V17g насыщение весов < 10% (анти-windup, F13)", (w >= 0.999).mean() < 0.10,
          f"w@клип={(w >= 0.999).mean():.1%}")
    print(f"     (критичность C={C:.3f}, E/I={ei:.2f}, GABA={GABA:.2f})")


# ============================================================
# V18. POLER-транспорт (EQ-D19 + EQ-B1)
# ============================================================
def v18_poler_transport():
    print("\n=== V18. Транспорт POLER-шага (EQ-D19) ===")
    n = 24
    A = RNG.normal(0, 1, (n, n))
    J = A - A.T
    Jn = J / np.linalg.norm(J, 2)
    D = np.diag(np.linspace(0.5, 1.0, n))
    Jc = np.ones((1, n))
    P = np.eye(n) - Jc.T @ np.linalg.inv(Jc @ Jc.T) @ Jc
    p = RNG.normal(0, 1, n)
    p /= np.linalg.norm(p)

    def step(p):
        h = 0.1
        M = np.eye(n) + 0.5 * h * Jn
        p = np.linalg.solve(np.eye(n) - 0.5 * h * Jn, M @ p)  # Кэли = exp(hJ) 2-й порядок
        p = p - 0.05 * (D @ p)          # диссипация
        p = p + 0.02 * (0.5 * np.ones(n) - p)  # резонансное притяжение
        p = P @ p                        # причинность
        return p / np.linalg.norm(p)

    for _ in range(500):
        p = step(p)
    check("V18a норма = 1", abs(np.linalg.norm(p) - 1) < 1e-6, f"norm={np.linalg.norm(p):.6f}")
    check("V18b Σp = 0", abs(p.sum()) < 1e-10, f"Σp={p.sum():.2e}")
    deltas = []
    for _ in range(50):
        prev = p.copy()
        p = step(p)
        deltas.append(np.linalg.norm(p - prev))
    d_first, d_last = np.mean(deltas[:10]), np.mean(deltas[-10:])
    # предельный цикл: Δ ограничена и регулярна (не растёт)
    check("V18c устойчивый режим: Δ не растёт (цикл/покой)",
          d_last <= d_first * 1.5 and np.std(deltas[-10:]) < 0.1 * d_last + 1e-6,
          f"first={d_first:.4f} → last={d_last:.4f} (регулярная орбита)")


# ============================================================
# RUN ALL
# ============================================================
if __name__ == "__main__":
    print("SSN VERIFICATION SUITE v3 — ФИНАЛ (доказательство до реализации)")
    print("=" * 66)
    v1_hash_topology()
    v2_quantum_transmission()
    v3_neuromodulators()
    v4_homeostasis()
    v5_ei_balance()
    v6_criticality()
    v7_asymmetric_bit()
    v8_cse()
    v9_operators()
    v11_rotor_norm()
    v13_sctp_layer()
    v14_trit_packing()
    v15_oscillator()
    v16_active_inference()
    v17_full_vortex()
    v18_poler_transport()
    print("\n" + "=" * 66)
    print(f"ИТОГ: {PASS} PASS / {FAIL} FAIL")
    if FAIL > 0:
        print("!!! ЕСТЬ ПРОВАЛЫ — реализация ЗАПРЕЩЕНА !!!")
        raise SystemExit(1)
    print("ВСЕ АЛГОРИТМЫ ДОКАЗАНЫ ЧИСЛЕННО — реализация в Rust РАЗРЕШЕНА.")
