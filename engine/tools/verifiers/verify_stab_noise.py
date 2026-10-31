#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
verify_stab_noise.py — цикл I: Gottesman–Knill и шумовые модели
(цикл G, строки 2–3) против независимых арбитров.

  1. qiskit 2.5.2 (Statevector + DensityMatrix + Kraus):
     [1] стабилизаторные ТОЧНЫЕ ⟨Z_S⟩ (флаг --expect; спектральная
         теорема: 0 или ±1) на GHZ и кластерном состоянии против
         точных ожиданий qiskit-стейтвектора — совпадение строгое;
     [2] эмпирические распределения исходов stab-движка против
         точных распределений qiskit (TVD в биномиальных пределах);
     [3] шум: MCWF-траектории против ТОЧНОЙ эволюции DensityMatrix
         с Kraus-каналами (деполяризация p2q на обоих кубитах Белла;
         амплитудное затухание) — трёхстороннее согласие
         движок ↔ формула ↔ qiskit.
  2. Пресеты железа: упорядочение деградации (ideal < железо < 90-е
     по TVD; пик идеала не падает).

Запуск:  python3 tools/verifiers/verify_stab_noise.py
"""

from __future__ import annotations

import json
import math
import subprocess
import sys
import time
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[2]
PASSPORT_DIR = REPO / "scratch" / "passports"
PASSPORT = PASSPORT_DIR / "cycle_I.json"

results: list[dict] = []
verdicts: list[str] = []


def record(name: str, ok: bool, detail: str, numbers: dict | None = None):
    ok = bool(ok)
    numbers = {k: (float(v) if isinstance(v, (int, float, np.floating)) else v)
               for k, v in (numbers or {}).items()}
    results.append({"check": name, "ok": ok, "detail": detail, "numbers": numbers})
    verdicts.append("PASS" if ok else "FAIL")
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}: {detail}")
    if numbers:
        for k, v in numbers.items():
            print(f"         {k} = {v}")


def pqc_bin() -> str:
    import os
    for cand in (
        os.environ.get("PQC_BIN"),
        str(REPO / "target" / "release" / "pqc"),
        str(REPO / "target" / "debug" / "pqc"),
    ):
        if cand and Path(cand).exists():
            return cand
    return "pqc"


def run_pqc(args: list[str], timeout: int = 600) -> dict:
    out = subprocess.run([pqc_bin()] + args, capture_output=True, text=True, timeout=timeout)
    if out.returncode != 0:
        raise RuntimeError(f"pqc failed: {out.stderr[:300]}")
    return json.loads(out.stdout)


print("=" * 72)
print("ЦИКЛ I — Gottesman–Knill и шумовые модели против qiskit")
print("=" * 72)
t_start = time.time()

import qiskit
from qiskit import QuantumCircuit
from qiskit.quantum_info import DensityMatrix, Kraus, Statevector

# ─────────────────────────────────────────────────────────────────────────────
print("\n[1] Стабилизатор: ТОЧНЫЕ ⟨Z_S⟩ против qiskit-стейтвектора")
# ─────────────────────────────────────────────────────────────────────────────

# GHZ-8: точные ожидания — qiskit эталон
n = 8
qc_ghz = QuantumCircuit(n)
qc_ghz.h(0)
for q in range(1, n):
    qc_ghz.cx(0, q)
psi_ghz = Statevector(qc_ghz)


def qiskit_z_expect(psi: Statevector, subset: list[int]) -> float:
    probs = np.abs(np.asarray(psi.data)) ** 2
    acc = 0.0
    for i, p in enumerate(probs):
        sign = 1.0
        for q in subset:
            if (i >> q) & 1:
                sign = -sign
        acc += sign * p
    return acc


subsets_ghz = [[0, 1], [3, 7], [0, 1, 2], [5], list(range(8)), [1, 2, 3, 4, 5]]
args = ["stab", "--n", str(n), "--preset", "ghz", "--shots", "4", "--json"]
for s in subsets_ghz:
    args += ["--expect", ",".join(map(str, s))]
d = run_pqc(args)
mismatch = 0
max_dev = 0.0
for (subset_bits, val), subset in zip(d["expect_z"], subsets_ghz):
    want = qiskit_z_expect(psi_ghz, subset)
    got = 0.0 if val == "null" or val is None else float(val)
    dev = abs(got - want)
    max_dev = max(max_dev, dev)
    mismatch += dev > 1e-12
record(
    "I.1-ghz8-exact-expectations",
    mismatch == 0,
    f"⟨Z_S⟩ GHZ-8: движок vs qiskit по {len(subsets_ghz)} срезам, max|Δ| = {max_dev:.1e}",
    {"subsets": len(subsets_ghz), "max_dev": max_dev},
)

# Кластер-10: H везде + CZ-линия; случайные срезы
n = 10
qc_cl = QuantumCircuit(n)
for q in range(n):
    qc_cl.h(q)
for q in range(n - 1):
    qc_cl.cz(q, q + 1)
psi_cl = Statevector(qc_cl)
rng = np.random.default_rng(2026)
subsets_cl = [sorted(rng.choice(n, size=k, replace=False).tolist())
              for k in (1, 2, 3, 5, 7, 10)] + [[0, 2, 4, 6, 8], [1, 3, 5, 7, 9]]
args = ["stab", "--n", str(n), "--preset", "cluster", "--shots", "4", "--json"]
for s in subsets_cl:
    args += ["--expect", ",".join(map(str, s))]
d = run_pqc(args)
mismatch = 0
max_dev = 0.0
nulls = 0
for (subset_bits, val), subset in zip(d["expect_z"], subsets_cl):
    want = qiskit_z_expect(psi_cl, subset)
    got = 0.0 if val == "null" or val is None else float(val)
    nulls += val == "null" or val is None
    dev = abs(got - want)
    max_dev = max(max_dev, dev)
    mismatch += dev > 1e-12
record(
    "I.2-cluster10-exact-expectations",
    mismatch == 0,
    f"⟨Z_S⟩ кластер-10 (случайные срезы, {nulls} нулей): max|Δ| vs qiskit = {max_dev:.1e}",
    {"subsets": len(subsets_cl), "max_dev": max_dev, "zeros": nulls},
)

# ─────────────────────────────────────────────────────────────────────────────
print("\n[2] Стабилизатор: эмпирические распределения против qiskit")
# ─────────────────────────────────────────────────────────────────────────────

# GHZ-8: 3000 выстрелов → TVD против точного {0^8: ½, 1^8: ½}
d = run_pqc(["stab", "--n", "8", "--preset", "ghz", "--shots", "3000", "--json"])
exact = np.zeros(256)
exact[0] = 0.5
exact[255] = 0.5
emp = np.zeros(256)
for bits, c in d["counts"]:
    idx = int("".join(str(int(b)) for b in bits), 2)
    emp[idx] = c / 3000.0
tvd = 0.5 * np.abs(emp - exact).sum()
bound = 8.0 * math.sqrt(2.0 / 3000.0)  # 2 ненулевых исхода
record(
    "I.3-ghz8-distribution-tvd",
    tvd < bound and d["distinct_outcomes"] == 2 and abs(d["outcome_entropy_bits"] - 1.0) < 0.01,
    f"GHZ-8: TVD(эмп, точн) = {tvd:.4f} < {bound:.4f}; исходов {d['distinct_outcomes']}, "
    f"энтропия {d['outcome_entropy_bits']:.3f} бит",
    {"tvd": tvd, "bound": bound},
)

# Кластер-10: маргиналы = 0.5 точно (стабилизатор X_i Z_nbrs ⟹ ⟨Z_i⟩ = 0)
d = run_pqc(["stab", "--n", "10", "--preset", "cluster", "--shots", "4000", "--json"])
marg = np.array(d["marginals_p1"])
worst = float(np.max(np.abs(marg - 0.5)))
sigma = 0.5 / math.sqrt(4000.0)
record(
    "I.4-cluster10-marginals-half",
    worst < 5.0 * sigma,
    f"кластер-10: max|⟨Z_i⟩ − ½| = {worst:.4f} < 5σ = {5 * sigma:.4f}",
    {"worst": worst, "sigma": sigma},
)

# Масштаб: GHZ-1024 — «за пределами 26 кубитов» на порядок
d = run_pqc(["stab", "--n", "1024", "--preset", "ghz", "--shots", "8", "--json"])
ok = d["distinct_outcomes"] == 2 and d["random_events_total"] == 8
record(
    "I.5-ghz1024-beyond-26",
    ok and d["total_ms"] < 30_000,
    f"GHZ-1024: {d['distinct_outcomes']} исхода, случайных событий {d['random_events_total']}, "
    f"время {d['total_ms']:.0f} мс (statevector: 2^1024 амплитуд — вне физической Вселенной)",
    {"total_ms": d["total_ms"], "distinct": d["distinct_outcomes"]},
)

# ─────────────────────────────────────────────────────────────────────────────
print("\n[3] Шум: MCWF-траектории против точных каналов (qiskit Kraus)")
# ─────────────────────────────────────────────────────────────────────────────

# Белл + деполяризация p2q = p на обоих кубитах после CX.
p = 0.15
shots = 60_000
d = run_pqc(["noise", "bell", "--p2q", str(p), "--shots", str(shots), "--json"])
# CLI noisy_peak = max по исходам: для Белла это P(00)+... — возьмём из
# гистограммы? В json только пики; используем формулу P(00) через пик? Нет:
# peak = max(P(00), P(11), P(01), P(10)) — для симметричной картины пик
# соответствует P(00) = P(11). Точная формула:
want_formula = 0.25 * (1.0 + (1.0 - 4.0 * p / 3.0) ** 2)
# qiskit: DensityMatrix + Kraus деполяризации на обоих кубитах
qc_bell = QuantumCircuit(2)
qc_bell.h(0)
qc_bell.cx(0, 1)
rho = DensityMatrix(qc_bell)
I2 = np.eye(2)
X = np.array([[0, 1], [1, 0]], dtype=complex)
Y = np.array([[0, -1j], [1j, 0]], dtype=complex)
Z = np.array([[1, 0], [0, -1]], dtype=complex)
kraus_ops = [math.sqrt(1 - p) * I2] + [math.sqrt(p / 3) * M for M in (X, Y, Z)]
K = Kraus(kraus_ops)
# применяем на каждый кубит (после CX): qiskit applies on qubits
rho = rho.evolve(K, qargs=[0])
rho = rho.evolve(K, qargs=[1])
probs_qiskit = np.real(np.diag(rho.data))
want_qiskit = float(probs_qiskit[0])
got = d["results"][0]["noisy_peak"]
# пик = max исход; из-за симметрии P(00)=P(11)=want — пик совпадает с P(00)
sigma = math.sqrt(want_formula * (1 - want_formula) / shots)
ok = abs(got - want_formula) < 6 * sigma and abs(want_qiskit - want_formula) < 1e-12
record(
    "I.6-depolarizing-bell-vs-qiskit-kraus",
    ok,
    f"P(00): движок {got:.4f}, формула {want_formula:.4f}, qiskit-Kraus {want_qiskit:.6f} "
    f"(6σ = {6 * sigma:.4f})",
    {"engine": got, "formula": want_formula, "qiskit": want_qiskit, "sigma": sigma},
)

# Амплитудное затухание: |+⟩ → демпинг γ → измерение: P(0) = ½(1+1−γ)... :
# ρ = (1−γ)|+⟩⟨+| + γ|0⟩⟨0| — измерение в Z: P(0) = (1−γ)·½ + γ·1 = 1 − γ/2.
# Схема: H (с своим γ_1q) затем readout=0: итог P(0) = ?
# Точная эволюция qiskit: |0⟩ →H→ |+⟩ → AD(γ) → measure.
gamma = 0.25
t1_us, tg_ns = 1.0, 287.682  # γ = 1 − e^{−t/T1}: подберём точно
# γ = 1 − exp(−tg_ns/(t1_us·1000))
gamma_actual = 1.0 - math.exp(-tg_ns / (t1_us * 1000.0))
d = run_pqc(["noise", "bell", "--t1", str(t1_us), "--tg1", str(tg_ns), "--tg2", str(tg_ns),
             "--shots", str(shots), "--json"])
# Bell: оба кубита после гейтов; P(00) точно:
# H(0): |+0⟩ с γ на q0 после H; CX: с γ на q0 и q1 после CX.
# Точная цепочка в qiskit:
rho = DensityMatrix(qc_bell)
K0 = np.array([[1.0, 0.0], [0.0, math.sqrt(1 - gamma_actual)]], dtype=complex)
K1 = np.array([[0.0, math.sqrt(gamma_actual)], [0.0, 0.0]], dtype=complex)
Kad = Kraus([K0, K1])
# порядок как в движке: H → damp(q0) → CX → damp(q0), damp(q1)
rho_h = DensityMatrix(QuantumCircuit(2))  # |00⟩
qc_step = QuantumCircuit(2)
qc_step.h(0)
rho = DensityMatrix(qc_step)
rho = rho.evolve(Kad, qargs=[0])
qc_step2 = QuantumCircuit(2)
qc_step2.cx(0, 1)
rho = rho.evolve(qc_step2)
rho = rho.evolve(Kad, qargs=[0])
rho = rho.evolve(Kad, qargs=[1])
probs = np.real(np.diag(rho.data))
want = float(probs[0])
# движок: пик = max(P) — для демпинга P(00) максимален
got = d["results"][0]["noisy_peak"]
sigma = math.sqrt(want * (1 - want) / shots)
record(
    "I.7-amplitude-damping-vs-qiskit-kraus",
    abs(got - want) < 6 * sigma,
    f"P(00) при T1-демпинге (γ = {gamma_actual:.4f}): движок {got:.4f}, "
    f"qiskit-Kraus {want:.4f} (6σ = {6 * sigma:.4f})",
    {"engine": got, "qiskit": want, "gamma": gamma_actual, "sigma": sigma},
)

# Пресеты железа: идеал чище железа, 90-е хуже всех
d = run_pqc(["noise", "grover", "--n", "6", "--compare", "--shots", "20000", "--json"])
by = {r["preset"]: r for r in d["results"]}
ok = (
    by["ideal"]["tvd"] < by["ibm-heron"]["tvd"]
    and by["ideal"]["tvd"] < by["google-willow"]["tvd"]
    and by["noisy-90s"]["tvd"] > by["ibm-heron"]["tvd"]
    and by["noisy-90s"]["tvd"] > by["google-willow"]["tvd"]
    and by["ideal"]["ideal_peak"] > 0.99
)
record(
    "I.8-hardware-presets-ordering",
    ok,
    f"Grover-6 TVD: ideal {by['ideal']['tvd']:.4f} < heron {by['ibm-heron']['tvd']:.4f}, "
    f"willow {by['google-willow']['tvd']:.4f} < 90s {by['noisy-90s']['tvd']:.4f}; "
    f"пик идеала {by['ideal']['ideal_peak']:.4f}",
    {k: v["tvd"] for k, v in by.items()},
)

# ─────────────────────────────────────────────────────────────────────────────
elapsed = time.time() - t_start
n_pass = sum(1 for v in verdicts if v == "PASS")
final = "AXIOM CONFIRMED" if n_pass == len(verdicts) else "REFUTED / INCOMPLETE"

passport = {
    "cycle": "I",
    "theorem": "VIII/G(2,3): Gottesman–Knill за пределами 26 кубитов + шумовые модели",
    "subject": "стабилизатор: точные ⟨Z_S⟩ и распределения против qiskit; шум: "
               "MCWF против DensityMatrix+Kraus (деполяризация, T1); пресеты железа",
    "environment": {
        "qiskit": qiskit.__version__,
        "numpy": np.__version__,
        "pqc_bin": pqc_bin(),
    },
    "results": results,
    "verdicts": verdicts,
    "n_pass": n_pass,
    "n_total": len(verdicts),
    "verdict": final,
    "elapsed_s": elapsed,
}
PASSPORT_DIR.mkdir(parents=True, exist_ok=True)
PASSPORT.write_text(json.dumps(passport, indent=2, ensure_ascii=False), encoding="utf-8")

print("\n" + "=" * 72)
print(f"ИТОГ: {n_pass}/{len(verdicts)} проверок, вердикт: {final}")
print(f"время: {elapsed:.1f}s | qiskit {qiskit.__version__}")
print("=" * 72)
print(f"Паспорт: {PASSPORT}")
sys.exit(0 if final == "AXIOM CONFIRMED" else 1)
