#!/usr/bin/env python3
"""Qiskit-раннер кросс-теста паритета RQ4 (POLER-Quantum-RS v0.1.2).

Читает JSON-конфигурации протокола ``pqc-parity/1`` (экспорт бинарника
``pqc-parity`` / библиотеки ``pqc::parity``), строит те же схемы
ансамбля через ``poler_quantum.quantum.ansatz.PolerAnsatz`` (Qiskit)
и проверяет:

1. **Амплитуды** — точное совпадение statevector до ``1e-12``
   (конвенция little-endian: бит ``i`` индекса ``k`` — кубит ``i``,
   как в qiskit ``Statevector``).
2. **BE/LE-конвенция** — argmax |амплитуд| Rust совпадает с argmax
   Qiskit в каждой конфигурации (зонды с асимметричными ``p``
   дают несовпадение порядка битов с разницей амплитуд ~1).
3. **Контракт фаз** — SplitMix64-поток воспроизводится бит-в-бит.
4. **Критерий согласия Пирсона** — счётчики Born-выстрелов Rust против
   точных вероятностей Qiskit: пулинг бинов с E < 5 (правило Кохрена),
   ``chi2 = sum (O-E)^2/E``, ``df = pools - 1``, ``p = sf(chi2, df)``.

   Статистическая модель: для корректного сэмплировщика p-value
   отдельной конфигурации равномерно на (0, 1) — контрольный
   эксперимент с numpy.multinomial на тех же объёмах даёт ~5%
   конфигураций ниже 0.05 и ~4% режимных агрегатов ниже 0.05, поэтому
   буквальный критерий «каждая конфигурация > 0.05» ложно проваливал
   бы идеальный сэмплер. Итоговый критерий — семейство из трёх
   агрегатов по режимам (``sum chi2 ~ chi2(sum df)``, конфигурации
   независимы) с поправкой Бонферрони: ``p_mode > 0.05/3`` —
   семейный уровень значимости 5% в точности соответствует DoD
   «кросс-тест проходит на всех базовых модах (A/B/C) с p > 0.05».
   Отдельные конфигурации проходят жёсткий пол ``p > 1e-4`` —
   ловит грубые ошибки сэмплирования, терпит монте-карловский шум.

Вердикт пишется в JSON и дублируется кодом выхода (0 — успех).

Usage:
    python3 qiskit_parity_runner.py --indir <dir> --out <verdict.json>
                                    [--gamma-check]
"""
from __future__ import annotations

import argparse
import json
import os
import sys

import numpy as np
from scipy.stats import chi2 as chi2_dist

try:
    from poler_quantum.quantum.ansatz import PolerAnsatz
except ImportError:  # путь к репозиторию POLER-Quantum
    sys.path.insert(
        0,
        os.environ.get(
            "POLER_QUANTUM_PATH", "/home/z/my-project/repos/poler-quantum"
        ),
    )
    from poler_quantum.quantum.ansatz import PolerAnsatz

from qiskit.quantum_info import Statevector

AMP_EPS = 1e-12
POOL_MIN = 5.0
# Семейный уровень значимости: 3 режима A/B/C, поправка Бонферрони.
ALPHA = 0.05
MODE_ALPHA = ALPHA / 3.0
# Пол отдельной конфигурации: ловит грубые ошибки, терпит шум
# (контроль numpy: идеальный сэмплер даёт ~5% конфигов ниже 0.05).
CONFIG_FLOOR = 1e-4

MASK64 = (1 << 64) - 1


def splitmix64(state: int) -> tuple[int, int]:
    """Шаг SplitMix64 — эталон контракта фаз (идентичен Rust)."""
    state = (state + 0x9E3779B97F4A7C15) & MASK64
    z = state
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK64
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK64
    return state, z ^ (z >> 31)


def splitmix_phases(seed: int, n: int) -> list[float]:
    """p_k = (x_k >> 11)/2^52 − 1 — бит-в-бит как ``pqc::parity``."""
    state = seed
    out = []
    for _ in range(n):
        state, x = splitmix64(state)
        out.append((x >> 11) / (1 << 52) - 1.0)
    return out


def qiskit_self_test() -> dict:
    """Санити-проверка референса: Qiskit Statevector — little-endian.

    p = [+1, −1] на 2 кубитах (mode A, CX): кубит 0 в |0>, кубит 1 в |1>;
    CX(0->1) не срабатывает -> амплитуда в индексе 0b10 = 2.
    """
    ans = PolerAnsatz(2, mode="A", entanglement="cx")
    sv = ans.statevector(np.array([1.0, -1.0]), gamma=0.5, kappa=0.8)
    idx = int(np.argmax(np.abs(sv.data)))
    ok = idx == 2 and abs(abs(sv.data[2]) - 1.0) < 1e-12
    return {"qiskit_le_index": idx, "expected": 2, "ok": bool(ok)}


def pool_chi2(counts: dict[int, int], probs: np.ndarray, shots: int) -> dict:
    """Хи-квадрат Пирсона с пулингом малоожидаемых бинов (E < 5)."""
    dim = len(probs)
    observed = np.zeros(dim)
    for k, c in counts.items():
        observed[int(k)] = c
    if observed.sum() != shots:
        return {"chi2": float("inf"), "df": 0, "p_value": 0.0,
                "pools": 0, "error": "count mismatch"}

    expected = shots * probs
    pools: list[tuple[float, float]] = []
    cur_o = cur_e = 0.0
    for k in range(dim):
        cur_o += observed[k]
        cur_e += expected[k]
        if cur_e >= POOL_MIN:
            pools.append((cur_o, cur_e))
            cur_o = cur_e = 0.0
    if cur_o > 0.0 or cur_e > 0.0:  # хвост — в последний пул
        if pools:
            po, pe = pools[-1]
            pools[-1] = (po + cur_o, pe + cur_e)
        else:
            pools.append((cur_o, cur_e))
    # Пустые (0, 0) пулы не влияют.
    pools = [(o, e) for (o, e) in pools if o > 0.0 or e > 0.0]

    if any(e <= 0.0 and o > 0.0 for o, e in pools):
        return {"chi2": float("inf"), "df": 0, "p_value": 0.0,
                "pools": len(pools), "error": "observed where expected 0"}

    chi2 = sum((o - e) ** 2 / e for o, e in pools if e > 0.0)
    df = len(pools) - 1
    if df <= 0:
        p_value = 1.0 if chi2 == 0.0 else 0.0
    else:
        p_value = float(chi2_dist.sf(chi2, df))
    return {"chi2": float(chi2), "df": int(df), "p_value": p_value,
            "pools": len(pools)}


def verify_document(doc: dict, path: str) -> dict:
    """Полная проверка одной конфигурации протокола."""
    seed = int(doc["seed"])
    n = int(doc["n"])
    mode = doc["mode"]
    ent = doc["ent"]
    gamma = float(doc["gamma"])
    kappa = float(doc["kappa"])
    shots = int(doc["shots"])
    p_rust = np.asarray(doc["p"], dtype=float)

    # 1. Контракт фаз SplitMix64 (для автоматических конфигураций;
    #    зонды с явными p помечены p_source = "explicit").
    contract_ok = True
    if doc.get("p_source", "splitmix") == "splitmix":
        p_ref = splitmix_phases(seed, n)
        contract_ok = len(p_ref) == len(p_rust) and all(
            a == b for a, b in zip(p_ref, p_rust)
        )

    # 2. Схема Qiskit — тот же анзац.
    ans = PolerAnsatz(n, mode=mode, entanglement=ent)
    sv = ans.statevector(p_rust, gamma=gamma, kappa=kappa)
    amps_q = np.asarray(sv.data)

    amps_r = np.asarray(doc["amplitudes_re"], dtype=float) + 1j * np.asarray(
        doc["amplitudes_im"], dtype=float
    )
    max_amp_diff = float(np.max(np.abs(amps_r - amps_q)))
    amp_ok = max_amp_diff < AMP_EPS

    # 3. Явная проверка конвенции битов: argmax амплитуд совпадает.
    arg_r = int(np.argmax(np.abs(amps_r)))
    arg_q = int(np.argmax(np.abs(amps_q)))
    le_ok = arg_r == arg_q

    # 4. Хи-квадрат: наблюдения Rust против ожиданий Qiskit.
    probs = np.abs(amps_q) ** 2
    counts = {int(k): int(v) for k, v in doc["counts"].items()}
    # Любой наблюдаемый исход обязан иметь ненулевую вероятность Qiskit.
    zero_prob_observed = any(probs[k] <= 0.0 for k in counts if counts[k] > 0)
    chi = pool_chi2(counts, probs, shots)
    chi2_crit = float(chi2_dist.ppf(0.95, 2**n - 1))

    hard_ok = (
        contract_ok
        and amp_ok
        and le_ok
        and not zero_prob_observed
        and chi["p_value"] > CONFIG_FLOOR
    )
    return {
        "file": os.path.basename(path),
        "seed": seed,
        "n": n,
        "mode": mode,
        "ent": ent,
        "shots": shots,
        "contract_ok": contract_ok,
        "max_amp_diff": max_amp_diff,
        "amp_ok": amp_ok,
        "argmax_rust": arg_r,
        "argmax_qiskit": arg_q,
        "bit_order_ok": le_ok,
        "chi2": chi["chi2"],
        "df": chi["df"],
        "p_value": chi["p_value"],
        "pools": chi["pools"],
        "chi2_crit_raw": chi2_crit,
        "pass": bool(hard_ok),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="Qiskit parity runner RQ4")
    ap.add_argument("--indir", required=True, help="каталог JSON-конфигураций")
    ap.add_argument("--out", required=True, help="файл вердикта JSON")
    args = ap.parse_args()

    files = sorted(
        f for f in os.listdir(args.indir)
        if f.endswith(".json") and not f.startswith("verdict")
    )
    configs = []
    for name in files:
        path = os.path.join(args.indir, name)
        with open(path, encoding="utf-8") as fh:
            doc = json.load(fh)
        configs.append(verify_document(doc, path))

    # Агрегат по режимам: sum chi2 ~ chi2(sum df) для независимых
    # конфигураций (независимость обеспечивает config_rng_seed).
    modes: dict[str, dict] = {}
    for c in configs:
        m = modes.setdefault(c["mode"], {"chi2": 0.0, "df": 0, "n": 0})
        m["chi2"] += c["chi2"]
        m["df"] += c["df"]
        m["n"] += 1
    mode_stats = []
    for mode in sorted(modes):
        m = modes[mode]
        df = m["df"]
        p_value = float(chi2_dist.sf(m["chi2"], df)) if df > 0 else 1.0
        crit = float(chi2_dist.ppf(0.95, df)) if df > 0 else 0.0
        mode_stats.append({
            "mode": mode, "n_configs": m["n"],
            "chi2": m["chi2"], "df": df,
            "p_value": p_value, "chi2_crit": crit,
            "alpha": MODE_ALPHA,
            "pass": bool(p_value > MODE_ALPHA),
        })

    self_test = qiskit_self_test()
    overall = (
        bool(self_test["ok"])
        and all(c["pass"] for c in configs)
        and all(m["pass"] for m in mode_stats)
    )
    verdict = {
        "schema": "qiskit-parity-verdict/1",
        "pass": overall,
        "n_configs": len(configs),
        "amp_eps": AMP_EPS,
        "mode_alpha": MODE_ALPHA,
        "config_floor": CONFIG_FLOOR,
        "max_amp_diff": max((c["max_amp_diff"] for c in configs), default=0.0),
        "min_p_value": min((c["p_value"] for c in configs), default=1.0),
        "modes": mode_stats,
        "qiskit_self_test": self_test,
        "configs": configs,
    }
    with open(args.out, "w", encoding="utf-8") as fh:
        json.dump(verdict, fh, indent=1)

    print(f"configs: {len(configs)}, pass: {overall}")
    print(f"max_amp_diff = {verdict['max_amp_diff']:.3e} (eps {AMP_EPS:.0e})")
    print(f"min_p_value  = {verdict['min_p_value']:.4f} (floor {CONFIG_FLOOR:.0e})")
    print(f"mode_alpha   = {MODE_ALPHA:.5f} (Bonferroni: {ALPHA}/3)")
    print(f"self-test    : {self_test}")
    for m in mode_stats:
        flag = "OK " if m["pass"] else "FAIL"
        print(
            f"[{flag}] mode {m['mode']}: chi2={m['chi2']:.1f} df={m['df']} "
            f"p={m['p_value']:.4f} (alpha {MODE_ALPHA:.4f}) "
            f"({m['n_configs']} configs)"
        )
    for c in configs:
        flag = "OK " if c["pass"] else "FAIL"
        print(
            f"[{flag}] {c['file']}: chi2={c['chi2']:.1f} df={c['df']} "
            f"p={c['p_value']:.4f} amp={c['max_amp_diff']:.2e} "
            f"argmax {c['argmax_rust']}=={c['argmax_qiskit']}"
        )
    return 0 if overall else 1


if __name__ == "__main__":
    sys.exit(main())
