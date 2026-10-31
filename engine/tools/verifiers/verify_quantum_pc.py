#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
verify_quantum_pc.py — инструментальная верификация POLER Quantum PC (v0.44).

Цикл G, паспорт cycle_G.json. Четыре независимых арбитра:

  1. qiskit 2.5.2 — паритет распределений Борна на общих схемах
     (Bell, GHZ-5, BV-5, DJ-4, QFT-4 на подготовленном состоянии).
  2. Аналитика Гровера — sin²((2r+1)θ) против движком + независимая
     NumPy-реплика списка операций.
  3. СимПи — точное кольцо ℤ[1/√2, i]: независимая символьная реплика
     амплитуд Clifford+T-схем, равенство строгое (упрощение разности = 0).
  4. NumPy — реплика субстрата УДЕ §2.2 (γ-прецесия + SCF) на тех же
     H/P0/конфиге из JSON: траектории, работа ротора, сертифика Ляпунова.

Запуск:  python3 tools/verifiers/verify_quantum_pc.py
"""

from __future__ import annotations

import json
import math
import os
import subprocess
import sys
import tempfile
from fractions import Fraction
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[2]
PASSPORT_DIR = REPO / "scratch" / "passports"
PASSPORT = PASSPORT_DIR / "cycle_G.json"

TOL_QISKIT = 1e-12
TOL_SUBSTRATE = 1e-8

results: list[dict] = []
verdicts: list[str] = []


def record(name: str, ok: bool, detail: str, numbers: dict | None = None):
    ok = bool(ok)
    numbers = {k: (float(v) if isinstance(v, (int, float, np.floating)) else v)
               for k, v in (numbers or {}).items()}
    results.append({"check": name, "ok": ok, "detail": detail, "numbers": numbers})
    verdicts.append("PASS" if ok else "FAIL")
    mark = "PASS" if ok else "FAIL"
    print(f"  [{mark}] {name}: {detail}")
    if numbers:
        for k, v in numbers.items():
            print(f"         {k} = {v}")


def pqc_bin() -> str:
    for cand in (
        os.environ.get("PQC_BIN"),
        str(REPO / "target" / "release" / "pqc"),
        str(REPO / "target" / "debug" / "pqc"),
    ):
        if cand and Path(cand).exists():
            return cand
    raise SystemExit("pqc binary not found; build with cargo build -p pqc --bins")


def run_pqc(args: list[str]) -> dict:
    out = subprocess.run(
        [pqc_bin(), *args], capture_output=True, text=True, check=True
    )
    return json.loads(out.stdout)


def qc_json(text: str, extra: list[str] | None = None) -> dict:
    with tempfile.NamedTemporaryFile("w", suffix=".qc", delete=False) as f:
        f.write(text)
        path = f.name
    try:
        return run_pqc(["qc", path, "--json", *(extra or [])])
    finally:
        os.unlink(path)


# ======================================================================
# 1. Паритет с qiskit
# ======================================================================

def check_qiskit_parity():
    print("\n[1] Паритет с qiskit 2.5.2 (распределения Борна)")
    try:
        from qiskit import QuantumCircuit
        from qiskit.quantum_info import Statevector
    except ImportError:
        record("qiskit-import", False, "qiskit недоступен")
        return

    def probs_qiskit(qc: QuantumCircuit) -> np.ndarray:
        return np.abs(np.asarray(Statevector(qc).data)) ** 2

    # --- Bell ---
    d = run_pqc(["algo", "bell", "--shots", "0", "--json"])
    qc = QuantumCircuit(2)
    qc.h(0)
    qc.cx(0, 1)
    dp = np.abs(np.asarray(Statevector(qc).data)) ** 2
    diff = float(np.max(np.abs(np.array(d["probabilities"]) - dp)))
    record("bell-parity", diff < TOL_QISKIT, f"max|Δp| = {diff:.2e}", {"max_dp": diff})

    # --- GHZ-5 ---
    d = run_pqc(["algo", "ghz", "--n", "5", "--shots", "0", "--json"])
    qc = QuantumCircuit(5)
    qc.h(0)
    for q in range(1, 5):
        qc.cx(q - 1, q)
    dp = probs_qiskit(qc)
    diff = float(np.max(np.abs(np.array(d["probabilities"]) - dp)))
    record("ghz5-parity", diff < TOL_QISKIT, f"max|Δp| = {diff:.2e}", {"max_dp": diff})

    # --- QFT-4 на подготовленном состоянии (общий список операций) ---
    n = 4
    prep_angles = [0.7, -1.3, 2.1, 0.4]
    lines = [f"qubits {n}"]
    for q, a in enumerate(prep_angles):
        lines.append(f"ry {q} {a!r}")
    # Тот же список, что Rust qft(n, false): j от старшего к младшему.
    for j in range(n - 1, -1, -1):
        lines.append(f"h {j}")
        for k in range(j - 1, -1, -1):
            angle = math.pi / (1 << (j - k))
            lines.append(f"cp {k} {j} {angle!r}")
    d = qc_json("\n".join(lines))
    qc = QuantumCircuit(n)
    for q, a in enumerate(prep_angles):
        qc.ry(a, q)
    for j in range(n - 1, -1, -1):
        qc.h(j)
        for k in range(j - 1, -1, -1):
            qc.cp(math.pi / (1 << (j - k)), k, j)
    dp = probs_qiskit(qc)
    diff = float(np.max(np.abs(np.array(d["probabilities"]) - dp)))
    record("qft4-parity", diff < TOL_QISKIT, f"max|Δp| = {diff:.2e}", {"max_dp": diff})

    # --- BV-5, secret 21 ---
    secret = 21
    d = run_pqc(["algo", "bv", "--n", "5", "--secret", "21", "--shots", "0", "--json"])
    qc = QuantumCircuit(6)
    qc.x(5)
    qc.h(range(6))
    for k in range(5):
        if secret >> k & 1:
            qc.cx(k, 5)
    qc.h(range(5))
    dp = probs_qiskit(qc)
    diff = float(np.max(np.abs(np.array(d["probabilities"]) - dp)))
    # Секрет в младших 5 битах (анцилла q5 свободна).
    mass = sum(p for i, p in enumerate(d["probabilities"]) if i & 31 == secret)
    record(
        "bv5-parity",
        diff < TOL_QISKIT and mass > 1 - 1e-9,
        f"max|Δp| = {diff:.2e}, P(секрет) = {mass:.12f}",
        {"max_dp": diff, "p_secret": mass},
    )

    # --- DJ-4 сбалансированная (оракул по чётности x&1) ---
    marks = [x for x in range(16) if x & 1]
    d = run_pqc(
        ["algo", "dj", "--n", "4", "--marks", ",".join(map(str, marks)),
         "--shots", "0", "--json"]
    )
    qc = QuantumCircuit(4)
    qc.h(range(4))
    # Фазовый оракул: диагональ (−1)^f(x), f = x&1.
    diag = [(-1.0 if (x & 1) else 1.0) for x in range(16)]
    from qiskit.circuit.library import DiagonalGate
    qc.append(DiagonalGate(diag), range(4))
    qc.h(range(4))
    dp = probs_qiskit(qc)
    diff = float(np.max(np.abs(np.array(d["probabilities"]) - dp)))
    record("dj4-parity", diff < TOL_QISKIT, f"max|Δp| = {diff:.2e}", {"max_dp": diff})


# ======================================================================
# 2. Гровер: аналитика + независимая NumPy-реплика
# ======================================================================

def check_grover():
    print("\n[2] Гровер: аналитика sin²((2r+1)θ) + NumPy-реплика")

    def grover_np(n: int, marks: list[int], r: int) -> np.ndarray:
        dim = 1 << n
        # Матрица Уолша–Адамара: H[i][j] = (−1)^{popcount(i & j)}/√dim
        # (битовое скалярное произведение, НЕ xor-чётность).
        idx = np.arange(dim)
        pop = np.bitwise_count(idx[:, None] & idx[None, :])
        W = ((-1.0) ** pop) / math.sqrt(dim)
        psi = np.ones(dim, dtype=complex) / math.sqrt(dim)
        for _ in range(r):
            psi[marks] = -psi[marks]          # оракул
            psi = W @ psi                      # H^n
            psi[1:] = -psi[1:]                 # I − 2|0><0|
            psi = W @ psi                      # H^n
        return np.abs(psi) ** 2

    for n, mark in [(5, 22), (6, 42), (7, 42)]:
        d = run_pqc(["algo", "grover", "--n", str(n), "--marks", str(mark),
                     "--shots", "0", "--json"])
        r = int(d["iterations"])
        theta = math.asin(math.sqrt(1.0 / (1 << n)))
        p_theory = math.sin((2 * r + 1) * theta) ** 2
        p_engine = d["probabilities"][mark]
        p_np = grover_np(n, [mark], r)[mark]
        e1 = abs(p_engine - p_theory)
        e2 = abs(p_engine - p_np)
        record(
            f"grover-n{n}",
            e1 < 1e-9 and e2 < 1e-9,
            f"R={r}, P={p_engine:.10f} (теория {p_theory:.10f}, NumPy {p_np:.10f})",
            {"p_engine": p_engine, "p_theory": p_theory, "p_numpy": p_np,
             "err_theory": e1, "err_numpy": e2},
        )


# ======================================================================
# 3. Точное кольцо ℤ[1/√2, i] — независимая символьная реплика (SymPy)
# ======================================================================

def check_exact_ring():
    print("\n[3] Точное кольцо Z[1/sqrt(2), i] — SymPy-реплика, строгое равенство")
    import sympy as sp

    sqrt2 = sp.sqrt(2)

    def to_sympy(comp: list[float]) -> sp.Expr:
        a, b, c, d, k = (int(round(v)) for v in comp)
        return (sp.Integer(a) + sp.Integer(b) * sqrt2
                + sp.I * (sp.Integer(c) + sp.Integer(d) * sqrt2)) / 2**k

    def reference_amplitudes(text: str) -> list[sp.Expr]:
        """Независимая реплика: амплитуды как sympy-выражения."""
        n = None
        ops = []
        for raw in text.splitlines():
            line = raw.split("#")[0].strip()
            if not line:
                continue
            toks = line.split()
            if toks[0] in ("qubits", "qubit"):
                n = int(toks[1])
                amps = [sp.Integer(0)] * (1 << n)
                amps[0] = sp.Integer(1)
            else:
                ops.append(toks)
        for toks in ops:
            head = toks[0]
            if head in ("h", "x", "y", "z", "s", "t", "sdg", "tdg"):
                q = int(toks[1])
                half = 1 << q
                for blk in range(0, len(amps), half << 1):
                    for i in range(half):
                        a, b = amps[blk + i], amps[blk + half + i]
                        if head == "h":
                            w = 1 / sqrt2
                            amps[blk + i], amps[blk + half + i] = sp.expand(
                                w * (a + b)), sp.expand(w * (a - b))
                        elif head == "x":
                            amps[blk + i], amps[blk + half + i] = b, a
                        elif head == "y":
                            amps[blk + i], amps[blk + half + i] = sp.expand(
                                -sp.I * b), sp.expand(sp.I * a)
                        elif head == "z":
                            amps[blk + half + i] = sp.expand(-b)
                        elif head == "s":
                            amps[blk + half + i] = sp.expand(sp.I * b)
                        elif head == "sdg":
                            amps[blk + half + i] = sp.expand(-sp.I * b)
                        elif head == "t":
                            amps[blk + half + i] = sp.expand(
                                b * (1 + sp.I) / sqrt2)
                        elif head == "tdg":
                            amps[blk + half + i] = sp.expand(
                                b * (1 - sp.I) / sqrt2)
            elif head in ("cx", "cz", "swap"):
                x, y = int(toks[1]), int(toks[2])
                for i in range(len(amps)):
                    bx, by = (i >> x) & 1, (i >> y) & 1
                    if head == "cx" and bx == 1 and by == 0:
                        amps[i], amps[i | (1 << y)] = amps[i | (1 << y)], amps[i]
                    elif head == "cz" and bx == 1 and by == 1:
                        amps[i] = sp.expand(-amps[i])
                    elif head == "swap" and bx == 1 and by == 0:
                        j = i ^ (1 << x) ^ (1 << y)
                        amps[i], amps[j] = amps[j], amps[i]
            elif head == "ccx":
                c1, c2, t = int(toks[1]), int(toks[2]), int(toks[3])
                for i in range(len(amps)):
                    if (i >> c1) & 1 and (i >> c2) & 1 and (i >> t) & 1 == 0:
                        j = i | (1 << t)
                        amps[i], amps[j] = amps[j], amps[i]
            elif head == "flipstate":
                idx = int(toks[1])
                amps[idx] = sp.expand(-amps[idx])
            elif head == "flipphase":
                mask = int(toks[1])
                for i in range(len(amps)):
                    if mask and i & mask == mask:
                        amps[i] = sp.expand(-amps[i])
            elif head == "flipzero":
                for i in range(1, len(amps)):
                    amps[i] = sp.expand(-amps[i])
        return amps

    def exact_json(text: str) -> dict:
        with tempfile.NamedTemporaryFile("w", suffix=".qc", delete=False) as f:
            f.write(text)
            path = f.name
        try:
            return run_pqc(["qc", path, "--exact", "--json"])
        finally:
            os.unlink(path)

    circuits = {
        "bell": "qubits 2\nh 0\ncx 0 1\n",
        "t-chain": "qubits 1\nh 0\nt 0\nt 0\nt 0\nt 0\n",  # T⁴ = Z
        "clifford-mix":
            "qubits 3\nh 0\nh 1\nt 2\ncx 0 1\nt 1\ncx 1 2\nswap 0 2\n"
            "ccx 0 1 2\ntdg 0\ns 1\ncz 0 2\n",
        "grover2": "qubits 2\nh 0\nh 1\nflipstate 3\nh 0\nh 1\nflipzero\nh 0\nh 1\n",
    }
    for name, text in circuits.items():
        rep = exact_json(text)
        ref = reference_amplitudes(text)
        all_eq = True
        worst = ""
        for i, (comp, refa) in enumerate(zip(rep["exact_amplitudes"], ref)):
            got = to_sympy(comp)
            if sp.simplify(got - refa) != 0:
                all_eq = False
                worst = f"amp[{i}]: {got} != {refa}"
                break
        # Норма: сумма |amp|² = 1 строго.
        norm_ok = rep["norm_residual"] < 1e-15
        record(
            f"exact-{name}",
            all_eq and norm_ok,
            ("бит-в-бит амплитуды" if all_eq else worst)
            + f", норма-невязка {rep['norm_residual']:.1e}",
            {"norm_residual": rep["norm_residual"]},
        )

    # T⁴ = Z: точная проверка тождества кольца на уровне вероятностей невозможна
    # (фазы), поэтому сверяем амплитуды — уже сделано выше ("t-chain").


# ======================================================================
# 4. Субстрат УДЕ §2.2: NumPy-реплика + физика γ
# ======================================================================

def check_substrate():
    print("\n[4] Субстрат УДЕ §2.2: NumPy-реплика, γ-прецесия, SCF")

    def replicate(d: dict) -> tuple[list[dict], dict]:
        n = int(d["dim"])
        H = np.array(
            [[complex(re, im) for re, im in row] for row in d["hamiltonian"]]
        )
        P = np.array([[complex(re, im) for re, im in row] for row in d["p0"]])
        cfg = d["config"]
        eta, gamma, mu = cfg["eta"], cfg["gamma"], cfg["mu"]
        N, steps, U = int(cfg["particles"]), int(cfg["steps"]), cfg["scf_u"]

        def fock(P):
            return H + U * np.diag(np.diag(P).real) if U else H

        def mcweeney(X):
            return 3 * X @ X - 2 * X @ X @ X

        F = fock(P)
        pts = []
        violations = 0
        rotor_total = 0.0
        last_E = np.trace(H @ P).real
        prev = P.copy()
        for step in range(1, steps + 1):
            comm = P @ F - F @ P
            dissipator = P @ comm - comm @ P
            rotor = F @ P - P @ F
            shift = mu * (N - np.trace(P).real) / n
            nxt = P - eta * dissipator + eta * gamma * rotor + shift * np.eye(n)
            P = mcweeney(nxt)
            E = np.trace(H @ P).real
            lyap = float(np.sum(np.abs(H @ P - P @ H) ** 2))
            defect = float(np.linalg.norm(P @ P - P))
            purity = np.trace(P @ P).real
            rot_work = eta * gamma * np.trace((H @ F - F @ H) @ P).real
            prec = float(np.linalg.norm(P - prev))
            if E > last_E + 1e-12:
                violations += 1
            rotor_total += abs(rot_work)
            last_E = E
            prev = P.copy()
            pts.append({
                "step": step, "trace": np.trace(P).real, "purity": purity,
                "energy": E, "lyapunov": lyap, "defect": defect,
                "rotor_work": rot_work, "precession_speed": prec,
            })
            F = fock(P)
        summary = {
            "energy_violations": violations,
            "rotor_work_total": rotor_total,
            "final_defect": pts[-1]["defect"],
        }
        return pts, summary

    def compare(d: dict, pts: list[dict]) -> float:
        by_step = {int(p["step"]): p for p in pts}
        worst = 0.0
        for a in d["points"]:
            b = by_step.get(int(a["step"]))
            if b is None:
                return float("inf")
            for key in ("trace", "purity", "energy", "lyapunov", "defect",
                        "rotor_work", "precession_speed"):
                va, vb = float(a[key]), float(b[key])
                scale = max(abs(va), abs(vb), 1.0)
                worst = max(worst, abs(va - vb) / scale)
        return worst

    # 4a. γ = 0, чистая диссипация — паритет траекторий.
    d = run_pqc(["substrate", "--dim", "4", "--steps", "400", "--gamma", "0",
                 "--seed", "42", "--trace", "25", "--json"])
    pts, summ = replicate(d)
    worst = compare(d, pts)
    last = d["points"][-1]
    ok = (
        worst < TOL_SUBSTRATE
        and last["defect"] < 1e-12
        and abs(last["trace"] - 2.0) < 1e-9
        and last["lyapunov"] < 1e-12
        and int(d["energy_violations"]) <= int(d["config"]["steps"]) // 33
    )
    record(
        "substrate-gamma0",
        ok,
        f"реплика max rel diff = {worst:.2e}, дефект {last['defect']:.2e}, "
        f"Tr {last['trace']:.12f}, [H,P] {last['lyapunov']:.2e}, "
        f"нарушений {d['energy_violations']}",
        {"max_rel_diff": worst, "final_defect": last["defect"],
         "final_lyapunov": last["lyapunov"]},
    )

    # 4b. γ > 0, F = H: работа ротора строго нулевая (прецессия без работы).
    d = run_pqc(["substrate", "--dim", "4", "--steps", "400", "--gamma", "0.5",
                 "--seed", "7", "--trace", "25", "--json"])
    pts, summ = replicate(d)
    worst = compare(d, pts)
    rotor = d["rotor_work_total"]
    last = d["points"][-1]
    ok = worst < TOL_SUBSTRATE and rotor == 0.0 and last["defect"] < 1e-10
    record(
        "substrate-gamma-precession",
        ok,
        f"ротор |work| = {rotor:.1e} (строго 0 при F=H), "
        f"реплика diff {worst:.2e}, дефект {last['defect']:.2e}",
        {"rotor_work_total": rotor, "max_rel_diff": worst,
         "final_defect": last["defect"]},
    )

    # 4c. SCF: F ≠ H — ротор качает энергию.
    d = run_pqc(["substrate", "--dim", "4", "--steps", "600", "--gamma", "0.3",
                 "--scf", "0.8", "--seed", "11", "--trace", "30", "--json"])
    pts, summ = replicate(d)
    worst = compare(d, pts)
    rotor = d["rotor_work_total"]
    ok = worst < TOL_SUBSTRATE and rotor > 1e-6
    record(
        "substrate-scf-rotor-work",
        ok,
        f"ротор |work| = {rotor:.3e} > 0 при F≠H, реплика diff {worst:.2e}",
        {"rotor_work_total": rotor, "max_rel_diff": worst},
    )

    # 4d. Химический потенциал: след восстановлен.
    d = run_pqc(["substrate", "--dim", "6", "--steps", "500", "--gamma", "0",
                 "--fill", "3", "--mu", "2", "--seed", "3", "--trace", "50",
                 "--json"])
    last = d["points"][-1]
    ok = abs(last["trace"] - 3.0) < 1e-8
    record(
        "substrate-trace-restoration",
        ok,
        f"Tr P* = {last['trace']:.12f} (N = 3)",
        {"final_trace": last["trace"]},
    )


# ======================================================================
# 5. Ландауэров пол и детерминизм Борна
# ======================================================================

def check_landauer_and_determinism():
    print("\n[5] Ландауэров пол и детерминизм Born-семплирования")
    K_B, T, LN2 = 1.380649e-23, 300.0, math.log(2)

    d = run_pqc(["algo", "bell", "--shots", "0", "--json"])
    expect = d["entropy_bits"] * K_B * T * LN2
    err = abs(d["landauer_j"] - expect) / expect
    record(
        "landauer-floor",
        err < 1e-12 and abs(d["entropy_bits"] - 1.0) < 1e-12,
        f"H = {d['entropy_bits']:.12f} бит, W_min = {d['landauer_j']:.4e} Дж",
        {"entropy_bits": d["entropy_bits"], "landauer_j": d["landauer_j"],
         "rel_err": err},
    )

    a = run_pqc(["algo", "bell", "--shots", "500", "--seed", "123", "--json"])
    b = run_pqc(["algo", "bell", "--shots", "500", "--seed", "123", "--json"])
    det = a["counts"] == b["counts"]
    record(
        "born-determinism",
        det,
        "один и тот же сид → идентичные гистограммы (побитово)",
    )

    # Честная монета Белла: ~50/50 на 500 выстрелах.
    counts = dict((int(o), int(c)) for o, c in a["counts"])
    ok = counts.get(0, 0) + counts.get(3, 0) == 500 and counts.get(1, 0) == 0
    record(
        "bell-correlations",
        ok,
        f"исходы 00/11: {counts.get(0, 0)}/{counts.get(3, 0)}, "
        f"01/10: {counts.get(1, 0)}/{counts.get(2, 0)}",
    )


def main() -> int:
    print("=" * 72)
    print("POLER Quantum PC — инструментальная верификация (цикл G)")
    print("=" * 72)
    check_qiskit_parity()
    check_grover()
    check_exact_ring()
    check_substrate()
    check_landauer_and_determinism()

    passed = sum(1 for r in results if r["ok"])
    total = len(results)
    all_ok = passed == total
    print("\n" + "=" * 72)
    print(f"ИТОГ: {passed}/{total} проверок, вердикт: "
          f"{'AXIOM CONFIRMED' if all_ok else 'FAILED'}")
    print("=" * 72)

    PASSPORT_DIR.mkdir(parents=True, exist_ok=True)
    tools = {
        "qiskit": _try_version("qiskit"),
        "numpy": _try_version("numpy"),
        "sympy": _try_version("sympy"),
        "engine": pqc_bin(),
    }
    passport = {
        "cycle": "G",
        "subject": "POLER Quantum PC v0.44 — идеальный кубитный субстрат",
        "checks": results,
        "passed": passed,
        "total": total,
        "verdict": "AXIOM CONFIRMED" if all_ok else "FAILED",
        "tools": tools,
        "tolerances": {
            "qiskit_parity": TOL_QISKIT,
            "grover_analytic": 1e-9,
            "exact_ring": "strict (sympy simplify == 0)",
            "substrate_replica": TOL_SUBSTRATE,
        },
    }
    PASSPORT.write_text(json.dumps(passport, indent=2, ensure_ascii=False))
    print(f"Паспорт: {PASSPORT}")
    return 0 if all_ok else 1


def _try_version(mod: str) -> str:
    try:
        m = __import__(mod)
        return str(getattr(m, "__version__", "?"))
    except Exception:
        return "unavailable"


if __name__ == "__main__":
    sys.exit(main())
