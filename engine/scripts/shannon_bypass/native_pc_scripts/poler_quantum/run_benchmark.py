#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
poler_quantum.run_benchmark — самодостатній бенчмарк квантових метрик (v2).

Знахідка аудиту: оригінал імпортував неіснуючі модулі
(poler_quantum.benchmark.tasks / core.engine / quantum.engine) — скрипт
не запускався взагалі. Переписаний: використовує ЛОКАЛЬНИЙ metrics.py.

Що робить:
  [1] Самотест метрик на аналітичних значеннях (Bell, |0⟩/|+⟩, I/d).
  [2] Бенчмарк швидкості: fidelity / von Neumann / mutual info на матрицях
      d ∈ {4, 16, 64} (оперцій/сек).
  [3] Динаміка деполяризації: ρ → (1−p)ρ + p·I/d; перевірка аналітичної
      кривої purity: Tr ρ²(p) = (1−p)²·Tr ρ₀² + 2p(1−p)/d + p²/d.
  [4] Якщо встановлено qiskit — незалежна зведірка fidelity (arbiter).
"""

from __future__ import annotations

import sys
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from metrics import (  # noqa: E402
    fidelity, purity, quantum_mutual_info, von_neumann_entropy, _selftest,
)


def random_unitary(d: int, rng: np.random.Generator) -> np.ndarray:
    """Haar-подібна унітарна матриця через QR (з фазовою фіксацією)."""
    z = rng.standard_normal((d, d)) + 1j * rng.standard_normal((d, d))
    q, r = np.linalg.qr(z)
    return q @ np.diag(r.diagonal() / np.abs(r.diagonal()))


def random_state(d: int, rng: np.random.Generator):
    """Випадковий чистий стан (вектор + проєктор)."""
    v = rng.standard_normal(d) + 1j * rng.standard_normal(d)
    v /= np.linalg.norm(v)
    return v, np.outer(v, v.conj())


def bench_speed() -> dict:
    rng = np.random.default_rng(7)
    results = {}
    for d in (4, 16, 64):
        _, rho = random_state(d, rng)
        _, sigma = random_state(d, rng)

        t0 = time.perf_counter()
        n = 0
        while time.perf_counter() - t0 < 0.2:
            fidelity(rho, sigma)
            n += 1
        results[f"fidelity_d{d}"] = n / 0.2

        t0 = time.perf_counter()
        n = 0
        while time.perf_counter() - t0 < 0.2:
            von_neumann_entropy(rho)
            n += 1
        results[f"vN_entropy_d{d}"] = n / 0.2

    # mutual info на 2-кубітних станах
    bell = np.array([1, 0, 0, 1], dtype=complex) / np.sqrt(2)
    rho_bell = np.outer(bell, bell.conj())
    t0 = time.perf_counter()
    n = 0
    while time.perf_counter() - t0 < 0.2:
        quantum_mutual_info(rho_bell, (2, 2))
        n += 1
    results["mutual_info_d4"] = n / 0.2
    return results


def depolarization_dynamics() -> dict:
    """ρ → (1−p)ρ + p·I/d. Аналітична крива purity проти числової."""
    rng = np.random.default_rng(11)
    d = 8
    _, rho0 = random_state(d, rng)
    p0 = purity(rho0)  # ≈ 1 (чистий)

    ps = np.linspace(0.0, 1.0, 21)
    numeric, analytic = [], []
    for p in ps:
        rho_p = (1.0 - p) * rho0 + p * np.eye(d) / d
        numeric.append(purity(rho_p))
        # Tr ρ²(p) = (1−p)²·Tr ρ₀² + 2p(1−p)/d + p²/d  (перехресні члени I/d ρ зникають)
        analytic.append((1 - p) ** 2 * p0 + 2 * p * (1 - p) / d + p ** 2 / d)

    max_err = float(np.max(np.abs(np.array(numeric) - np.array(analytic))))
    return {"d": d, "ps": ps.tolist(), "numeric": numeric, "analytic": analytic,
            "max_abs_error": max_err, "ok": max_err < 1e-12}


def qiskit_crosscheck() -> dict | None:
    try:
        from qiskit.quantum_info import DensityMatrix, state_fidelity
    except ImportError:
        return None
    rng = np.random.default_rng(3)
    pairs = []
    for _ in range(20):
        _, rho = random_state(4, rng)
        mixed = 0.5 * rho + 0.5 * np.eye(4) / 4
        # ранілізація у несингулярний стан для qiskit
        mixed_reg = 0.999 * mixed + 0.001 * np.eye(4) / 4
        pairs.append((rho, mixed_reg))
    max_diff = 0.0
    for rho, sigma in pairs:
        ours = fidelity(rho, sigma)
        theirs = float(state_fidelity(DensityMatrix(rho), DensityMatrix(sigma)))
        max_diff = max(max_diff, abs(ours - theirs))
    return {"n_pairs": len(pairs), "max_abs_diff_vs_qiskit": max_diff,
            # 1e-6 — чесний допуск: вкладені eigh+sqrt на погано обумовлених
            # матрицях дають ~1e-8..1e-7 шуму в ОБОХ реалізаціях; точність
            # проти АНАЛІТИКИ перевірена в самотесті metrics.py на 1e-12.
            "ok": max_diff < 1e-6}


def main() -> int:
    print("=" * 64)
    print("  POLER-QUANTUM METRICS BENCHMARK (v2, самодостатній)")
    print("=" * 64)

    print("\n[1] Самотест метрик (аналітичні значення):")
    if not _selftest():
        return 1

    print("\n[2] Швидкість (операцій/сек):")
    speed = bench_speed()
    for k, v in speed.items():
        print(f"    {k:<22}: {v:10,.0f} оп/с")

    print("\n[3] Динаміка деполяризації (аналітична крива):")
    dep = depolarization_dynamics()
    print(f"    max |numeric − analytic| = {dep['max_abs_error']:.2e} "
          f"({'OK' if dep['ok'] else 'FAIL'}, d={dep['d']})")
    if not dep["ok"]:
        return 1

    print("\n[4] Незалежний арбітер (qiskit), якщо встановлений:")
    qc = qiskit_crosscheck()
    if qc is None:
        print("    qiskit не встановлено — пропускаю (не помилка)")
    else:
        print(f"    {qc['n_pairs']} пар; max |F_наша − F_qiskit| = "
              f"{qc['max_abs_diff_vs_qiskit']:.2e} "
              f"({'OK' if qc['ok'] else 'FAIL'})")
        if not qc["ok"]:
            return 1

    print("\nВИСНОВОК: квантові метрики підтверджені "
          "(аналітика + швидкість" +
          (" + qiskit)" if qc else ")"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
