#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""MVR-v3, цикл A — Теорема V.2′: примитивы S-box x^254 в GF(2⁸).

РЕДИЦИЯ 2 (после подключения poler-os): ARX-часть ВЫВЕДЕНА из оборота —
прежний arx_phi описывал ДРУГУЮ структуру (без mul/xorshift) и Z3 доказывал
легкую функцию. Биективность РЕАЛЬНОЙ Φ — в verify_pnd_full.py (секция v1:
пошаговые леммы + Hensel-обратный). Этот скрипт сохраняет свою уникальную
ценность: GF-представление-независимость и ПОЛНЫЕ таблицы DDT/LAT S-box.

Слои:
  gf    — x^254 == x⁻¹ в GF(2⁸) для ДВУХ неприводимых полиномов (0x11B AES,
          0x165) — представление-независимость (Ферма: x^255 = 1)
  sbox  — биективность S; ПОЛНЫЕ таблицы DDT (256×256) и LAT (256×256):
          δ_S (дифференциальная равномерность), max|W| (линейность), NL
  json  — паспорт в scratch/passports/cycle_A.json

Сверка с реальным ядром poler-os: значения constantTimeSbox — golden-вектора
в verify_pnd_full.py (все 256 значений совпали бит-в-бит).
"""
import argparse, json, sys
from pathlib import Path
import numpy as np

def gf_mul(a, b, poly):
    """Умножение в GF(2^8) (carry-less + модуль)."""
    r = 0
    while b:
        if b & 1:
            r ^= a
        b >>= 1
        a <<= 1
        if a & 0x100:
            a ^= poly
    return r & 0xFF

def build_sbox(poly):
    """S(x) = x^254 в GF(2^8)[poly]."""
    S = [0] * 256            # 0^254 = 0
    for x in range(1, 256):
        r = x
        for _ in range(253):            # x^254 = x * x^253
            r = gf_mul(r, x, poly)
        S[x] = r
    return S

# ───────────────────── 1. x^254 = x⁻¹ (представление-независимость) ────────────
def run_gf():
    out = {}
    for poly, name in [(0x11B, 'AES 0x11B'), (0x165, 'alt 0x165')]:
        S = build_sbox(poly)
        inverse_ok = all(S[x] == 0 for x in [0]) and True
        # проверка: S[x] * x == 1 для всех x ≠ 0
        inv_ok = all(gf_mul(S[x], x, poly) == 1 for x in range(1, 256))
        # и S взаимно однозначен
        bij = len(set(S)) == 256
        out[name] = {'x254_times_x_is_1': inv_ok, 'bijective': bij}
    fermat_ok = all(
        gf_mul(build_sbox(0x11B)[x], x, 0x11B) == 1 for x in range(1, 256))
    return {'polys': out,
            'fermat_note': 'x^255 = 1 в GF(2⁸)* ⇒ x^254 = x⁻¹ — не зависит от '
                           'выбора полинома (подтверждено на двух)',
            'verdict': ('AXIOM CONFIRMED (x^254 = x⁻¹, биективно, 0↦0)'
                        if all(v['x254_times_x_is_1'] and v['bijective']
                               for v in out.values()) else 'REFUTED')}

# ───────────────── 2. Полные DDT / LAT S-box (256×256, без пропусков) ─────────
def run_sbox(poly=0x11B):
    S = np.array(build_sbox(poly), dtype=np.uint8)
    x = np.arange(256, dtype=np.uint8)
    # DDT[a][b] = #{x : S(x)^S(x^a) == b}
    DDT = np.zeros((256, 256), dtype=np.int32)
    for a in range(256):
        diff_out = S[x] ^ S[x ^ np.uint8(a)]
        np.add.at(DDT, (np.full(256, a), diff_out), 1)
    delta_S = int(DDT[1:].max())          # дифф. равномерность (a ≠ 0)
    # LAT: W[a][b] = Σ_x (−1)^{a·x ⊕ b·S(x)}
    A = np.zeros((256, 256), dtype=np.int8)
    B = np.zeros((256, 256), dtype=np.int8)
    for i in range(256):
        A[i] = np.array([bin(i & j).count('1') & 1 for j in range(256)])
        B[i] = np.array([bin(i & int(S[j])).count('1') & 1 for j in range(256)])
    W = np.zeros((256, 256), dtype=np.int32)
    for a in range(256):
        for b in range(256):
            W[a, b] = int(((-1) ** (A[a].astype(np.int8) ^ B[b])).sum())
    mask = np.ones((256, 256), dtype=bool); mask[0, 0] = False
    max_W = int(np.abs(W[mask]).max())
    NL = 128 - max_W // 2
    return {'poly': hex(poly),
            'sbox_is_permutation': bool(len(set(S.tolist())) == 256),
            'DDT_shape': '256×256 (полный перебор 2^16)',
            'differential_uniformity_delta': delta_S,
            'LAT_shape': '256×256 (полный перебор)',
            'max_abs_walsh': max_W,
            'nonlinearity_NL': NL,
            'nyberg_bound': 'NL ≥ 2^7 − 2^4 = 112 для инверсии',
            'verdict': ('CONFIRMED: δ_S = 4 (каждый дифференциал ≤ 4/256 — '
                        'нелинейность диффузии); NL = %d (граница Ниберга '
                        'выполнена)' % NL
                        if delta_S == 4 and NL >= 112 else
                        'ЧАСТИЧНО: δ=%d, NL=%d — см. паспорт' % (delta_S, NL))}

# ───────────────── 3. Z3: биективность ARX Φ — ПЕРЕНЕСЕНО ───────────────────
# РЕДИЦИЯ 2: прямой запрос на реальную Φ (add→rotl13→χ16→mul→rotl7→add)
# не решается Z3 за разумное время (>300 c — mul байт-бластится тяжело).
# Доказательство реальной Φ — пошаговые леммы в verify_pnd_full.py::run_v1
# (Z3 UNSAT × 4 + Hensel-обратный для mul). Ниже — маркер для паспорта.
def run_z3():
    return {'status': 'ПЕРЕНЕСЕНО в verify_pnd_full.py (секция v1)',
            'reason': 'прошлая Z3-секция доказывала ДРУГУЮ структуру Φ '
                      '(без mul/xorshift) — легкую функцию; реальная доказана '
                      'пошаговыми леммами',
            'verdict': 'см. verify_pnd_full.py / Том V изд. 2 §2'}

# ───────────────────── 4. Явный Φ⁻¹ — ПЕРЕНЕСЕНО ─────────────────────────────
def run_roundtrip(n=10_000_000, seed=5):
    return {'status': 'ПЕРЕНЕСЕНО в verify_pnd_full.py (секция v1)',
            'verdict': 'Φ⁻¹ для реальной структуры + round-trip 2×10⁶ — там же'}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--json', default='scratch/passports/cycle_A.json')
    a = ap.parse_args()
    out = {'theorem': 'V.1 + V.2', 'cycle': 'A',
           'subject': 'PND v8: S-box x^254 (GF(2⁸)) DDT/LAT; ARX Φ биективность',
           'archaeology': {
               'source': 'архив 299: критика линейности = PND v6/v7 (устарело); '
                         'v8: pndMix = Φ(a·b) +% ε·Φ(a⊕b), S-box x^254 до умножения',
               'code_grounding': 'poler-os ПОДКЛЮЧЕН (@fc3ffa8) — полный '
                                 'код-грундинг в verify_pnd_full.py; этот '
                                 'скрипт — специалист S-box/GF'},
           'code': ['docs/mathematical-treatise/VOLUME_V (изд. 2)'],
           'commit': '36df975'}
    out['gf_inverse'] = run_gf()
    out['sbox_ddt_lat'] = run_sbox()
    out['z3_arx_bijective'] = run_z3()
    out['roundtrip'] = run_roundtrip()
    p = Path(a.json); p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(out, ensure_ascii=False, indent=1, default=str))
    print(json.dumps(out, ensure_ascii=False, indent=1, default=str))
    print('\nпаспорт: %s' % p)

if __name__ == '__main__':
    sys.exit(main())
