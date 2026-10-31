#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
poler_quantum.metrics — КВАНТОВІ МЕТРИКИ (v2, аудит 2026-09-21).

Знахідка аудиту: коміт 7375791 обіцяв «Fidelity, ентропію фон Неймана та
квантову інформацію», але файл містив лише трекінг-метрики траєкторій
(RMSE, smoothness). Тепер метрики відповідають заявці; трекінг-метрики
перенесені (без змін) у tracking.py.

Реалізовано (numpy, без qiskit-залежності; збіги з qiskit — у run_benchmark):
  * purity(ρ)                 = Tr ρ²                       ∈ [1/d, 1]
  * fidelity(ρ, σ)            = (Tr √(√ρ σ √ρ))²            ∈ [0, 1]
  * von_neumann_entropy(ρ)    = −Tr ρ log₂ ρ                ∈ [0, log₂ d]
  * trace_distance(ρ, σ)      = ½ Tr|ρ−σ|                   ∈ [0, 1]
  * quantum_mutual_info(ρ_AB) = S(ρ_A)+S(ρ_B)−S(ρ_AB)       ≥ 0 (SSAR)
"""

from __future__ import annotations

import numpy as np


# ───────────────────────────── базові HELPERS ────────────────────────────────

def _validate_dm(rho: np.ndarray, name: str = "rho") -> np.ndarray:
    """Перевірка, що матриця — щільність (ермітова, PSD, слід 1)."""
    rho = np.asarray(rho, dtype=complex)
    if rho.ndim != 2 or rho.shape[0] != rho.shape[1]:
        raise ValueError(f"{name}: очікувана квадратна матриця, отримано {rho.shape}")
    if not np.allclose(rho, rho.conj().T, atol=1e-10):
        raise ValueError(f"{name}: матриця не ермітова")
    tr = float(np.real(np.trace(rho)))
    if abs(tr - 1.0) > 1e-8:
        raise ValueError(f"{name}: слід = {tr}, очікується 1")
    return rho


def _sqrtm_psd(a: np.ndarray) -> np.ndarray:
    """Головний квадратний корінь PSD матриці через власний розклад."""
    w, v = np.linalg.eigh(a)
    w = np.clip(w, 0.0, None)  # NUMA-безпечно: зсув мікроскопічних від'ємних
    return (v * np.sqrt(w)) @ v.conj().T


# ───────────────────────────── КВАНТОВІ МЕТРИКИ ─────────────────────────────

def purity(rho: np.ndarray) -> float:
    """Tr ρ². Чистий стан → 1; максимально змішаний → 1/d."""
    rho = _validate_dm(rho)
    return float(np.real(np.trace(rho @ rho)))


def fidelity(rho: np.ndarray, sigma: np.ndarray) -> float:
    """F(ρ,σ) = (Tr √(√ρ σ √ρ))² — Uhlmann fidelity, ∈ [0, 1]."""
    rho = _validate_dm(rho, "rho")
    sigma = _validate_dm(sigma, "sigma")
    sq = _sqrtm_psd(_sqrtm_psd(rho) @ sigma @ _sqrtm_psd(rho))
    f = float(np.real(np.trace(sq)))
    return min(max(f, 0.0), 1.0) ** 2


def von_neumann_entropy(rho: np.ndarray, base: float = 2.0) -> float:
    """S(ρ) = −Tr ρ log ρ (біт, якщо base=2). 0 для чистого стану."""
    rho = _validate_dm(rho)
    w = np.linalg.eigvalsh(rho)
    w = w[w > 1e-12]  # 0·log0 := 0
    return float(-np.sum(w * np.log(w) / np.log(base)))


def trace_distance(rho: np.ndarray, sigma: np.ndarray) -> float:
    """T(ρ,σ) = ½ Tr|ρ−σ| = ½ Σ|λᵢ(ρ−σ)|."""
    _validate_dm(rho, "rho")
    _validate_dm(sigma, "sigma")
    w = np.linalg.eigvalsh(rho - sigma)
    return float(0.5 * np.sum(np.abs(w)))


def quantum_mutual_info(rho_ab: np.ndarray, dims: tuple[int, int]) -> float:
    """I(A:B) = S(ρ_A) + S(ρ_B) − S(ρ_AB) — субадитивність (≥ 0)."""
    d_a, d_b = dims
    rho_ab = _validate_dm(rho_ab, "rho_ab")
    if rho_ab.shape != (d_a * d_b, d_a * d_b):
        raise ValueError(f"розмірність {rho_ab.shape} не відповідає dims {dims}")
    rho_a = np.trace(rho_ab.reshape(d_a, d_b, d_a, d_b), axis1=1, axis2=3)
    rho_b = np.trace(rho_ab.reshape(d_a, d_b, d_a, d_b), axis1=0, axis2=2)
    return float(von_neumann_entropy(rho_a) + von_neumann_entropy(rho_b)
                 - von_neumann_entropy(rho_ab))


# ───────────────────────────── САМОТЕСТ ──────────────────────────────────────

def _selftest() -> bool:
    rng = np.random.default_rng(42)
    checks = []

    # 1. Чистий стан: purity=1, S=0, F(ψ,ψ)=1
    v = rng.standard_normal(4) + 1j * rng.standard_normal(4)
    v /= np.linalg.norm(v)
    psi = np.outer(v, v.conj())
    checks.append(("purity(чистий) = 1", abs(purity(psi) - 1.0) < 1e-12))
    checks.append(("S(чистий) = 0", abs(von_neumann_entropy(psi)) < 1e-12))
    checks.append(("F(ψ,ψ) = 1", abs(fidelity(psi, psi) - 1.0) < 1e-12))

    # 2. Максимально змішаний: purity=1/d, S=log2(d)
    d = 4
    max_mixed = np.eye(d) / d
    checks.append(("purity(I/d) = 1/d", abs(purity(max_mixed) - 1.0 / d) < 1e-12))
    checks.append(("S(I/d) = log2(d)=2", abs(von_neumann_entropy(max_mixed) - 2.0) < 1e-12))

    # 3. Fidelity: аналітичне значення для |0⟩⟨0| vs |+⟩⟨+| = 1/2
    ket0 = np.array([1, 0], dtype=complex)
    ketp = np.array([1, 1], dtype=complex) / np.sqrt(2)
    rho0 = np.outer(ket0, ket0.conj())
    rhop = np.outer(ketp, ketp.conj())
    checks.append(("F(|0⟩,|+⟩) = 1/2", abs(fidelity(rho0, rhop) - 0.5) < 1e-12))

    # 4. Bell-станок: S(ρ_AB)=0, S(ρ_A)=1, I(A:B)=2 (максимальне заплутування)
    bell = np.array([1, 0, 0, 1], dtype=complex) / np.sqrt(2)
    rho_bell = np.outer(bell, bell.conj())
    checks.append(("Bell: S(ρ_AB)=0", abs(von_neumann_entropy(rho_bell)) < 1e-12))
    checks.append(("Bell: I(A:B)=2 біти",
                   abs(quantum_mutual_info(rho_bell, (2, 2)) - 2.0) < 1e-12))

    # 5. Продуктовий стан: I(A:B)=0
    prod = np.kron(rho0, rhop)
    checks.append(("product: I(A:B)=0", abs(quantum_mutual_info(prod, (2, 2))) < 1e-12))

    # 6. Fidelity змішаного з чистим: F(ρ,|ψ⟩) = ⟨ψ|ρ|ψ⟩ (аналітика)
    mixed = 0.5 * rho0 + 0.5 * rhop  # суміш некогерентна
    expected_f = 0.5 * abs(ket0 @ ket0.conj()) ** 2 + 0.5 * abs(ket0 @ ketp.conj()) ** 2
    checks.append(("F(суміш ½|0⟩+½|+⟩, |0⟩) = ¾",
                   abs(fidelity(mixed, rho0) - expected_f) < 1e-12))
    checks.append(("trace_distance(ρ,ρ) = 0", trace_distance(mixed, mixed) < 1e-12))
    # T(чистих) = √(1−|⟨ψ|φ⟩|²): для |0⟩,|+⟩ → 1/√2 ≈ 0.7071
    checks.append(("T(|0⟩,|+⟩) = 1/√2",
                   abs(trace_distance(rho0, rhop) - 1.0 / np.sqrt(2)) < 1e-12))

    print("poler_quantum.metrics selftest:")
    all_ok = True
    for name, cond in checks:
        print(f"  [{'OK' if cond else 'FAIL'}] {name}")
        all_ok = all_ok and cond
    return all_ok


if __name__ == "__main__":
    import sys
    sys.exit(0 if _selftest() else 1)
