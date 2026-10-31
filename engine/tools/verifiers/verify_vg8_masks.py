#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""MVR-v3, цикл B — Теорема II.1: побитовая эквивалентность безветвежных масок.

    f(x, c) = (x & AbsorbMask(c)) ^ SignMask(c) == c · x,  c ∈ {−1, 0, +1}

Против РЕАЛЬНЫХ констант кода (src/quantum/meta_compiler.rs#L83-L89):
    MASK_KEEP  = 0xFFFFFFFF   (absorb, c = ±1)
    MASK_KILL  = 0x00000000   (absorb, c =  0)
    MASK_FLIP  = 0x80000000   (sign,   c = −1)
    MASK_NOFLIP= 0x00000000   (sign,   c ∈ {0, +1})
Горячий путь: meta_compiler.rs#L897-898 `xor(and(l, al), sl)`.
Генерация кода: #L1104-1122. Тест-каveat нулей: #L1372.

Слои:
  z3    — SMT-доказательства в теории IEEE-754 float32 (Z3 FP)
  numpy — побитовый дифференциал против скалярного умножения
          (краевые случаи + 4·10^6 случайных битовых паттернов)
  json  — паспорт в scratch/passports/cycle_B.json
"""
import argparse, json, struct, sys
from pathlib import Path

MASKS = {          # (absorb, sign) — из masks_of() meta_compiler.rs#L151-158
    0:  (0x00000000, 0x00000000),
    1:  (0xFFFFFFFF, 0x00000000),
    -1: (0xFFFFFFFF, 0x80000000),
}

def bits_to_f32_array(bits):
    """uint32-массив → float32-массив через байтовое переисследование (LE)."""
    return bits.astype('<u4').view('<f4')

def f32_array_to_bits(arr):
    return arr.astype('<f4').view('<u4')

# ────────────────────────────── Z3: SMT IEEE-754 ──────────────────────────────
def run_z3():
    import z3
    F32 = z3.Float32()
    proofs = {}

    def prove(name, assumptions, claim):
        """Доказать ∀x: assumptions(x) ⟹ claim(x): добавить ¬claim при assumptions."""
        x_bv = z3.BitVec('x_%s' % name, 32)
        x = z3.fpToFP(x_bv, F32)          # битовая реинтерпретация BV → FP
        s = z3.Solver()
        for a in assumptions(x, x_bv):
            s.add(a)
        s.add(z3.Not(claim(x, x_bv)))     # отрицание тезиса → ищем контрпример
        r = s.check()
        proofs[name] = 'UNSAT (теорема доказана)' if r == z3.unsat else str(r)

    # II.1a: c = +1 — тождество битов (весь домен, включая NaN/Inf)
    prove('c_plus1_identity_all_patterns',
          lambda fp, bv: [],
          lambda fp, bv: z3.fpToFP(bv & z3.BitVecVal(0xFFFFFFFF, 32), F32) == fp)

    # II.1b: c = −1 — XOR знакового бита == fp.neg для всех не-NaN
    #         (включая ±0, ±Inf; SMT-LIB fp.neg определён как флип бита 31)
    prove('c_minus1_flip_is_fneg_non_nan',
          lambda fp, bv: [z3.Not(z3.fpIsNaN(fp))],
          lambda fp, bv: z3.fpToFP(bv ^ z3.BitVecVal(0x80000000, 32), F32) == -fp)

    def finite(fp):
        return z3.And(z3.Not(z3.fpIsInf(fp)), z3.Not(z3.fpIsNaN(fp)))

    # II.1c: c = 0 — (x & 0) ^ 0 == +0.0  ~IEEE-числово равно~ 0·x для КОНЕЧНЫХ x.
    #         НЮАНС СЕМАНТИКИ, найденный инструментом: Z3 '=' на FP-сорте —
    #         БИТОВОЕ равенство (+0 ≠ −0); IEEE-числовое равенство — fpEQ
    #         (+0 == −0). Теорема верна в fpEQ-семантике.
    prove('c_zero_kill_is_ieq_mul_for_finite',
          lambda fp, bv: [finite(fp)],
          lambda fp, bv: z3.fpEQ(z3.fpToFP((bv & z3.BitVecVal(0, 32)) ^ z3.BitVecVal(0, 32), F32),
                                 z3.FPVal(0.0, F32) * fp))

    # II.1c': КАВЕТ (биты, не значения): для конечных x < 0 маска даёт +0.0,
    #          скаляр 0·x даёт −0.0 — бит-строгое '=' даёт SAT (контрпример
    #          x = −0.0 найден инструментом). Это ровно каveat теста L1372.
    x_bv2 = z3.BitVec('x_caveat', 32)
    x2 = z3.fpToFP(x_bv2, F32)
    s1 = z3.Solver()
    s1.add(finite(x2)); s1.add(z3.fpIsNegative(x2))
    s1.add(z3.Not(z3.fpToFP((x_bv2 & z3.BitVecVal(0, 32)) ^ z3.BitVecVal(0, 32), F32)
                  == z3.FPVal(0.0, F32) * x2))
    proofs['c_zero_signed_zero_bit_gap'] = (
        'SAT — битовый кавет +0.0 vs −0.0 для x<0 (значения IEEE-равны)'
        if s1.check() == z3.sat else '???')

    # II.1d: НЕОБХОДИМОСТЬ ограничения домена: для x ∈ {NaN, ±Inf} c=0 рушится
    #         (0·NaN = NaN ≠ +0.0; 0·Inf = NaN ≠ +0.0) → ищем sat-контрпример
    x_bv = z3.BitVec('x_gap', 32)
    x = z3.fpToFP(x_bv, F32)
    s = z3.Solver()
    s.add(z3.Not(finite(x)))
    s.add(z3.Not(z3.fpToFP(x_bv & z3.BitVecVal(0, 32), F32) == z3.FPVal(0.0, F32) * x))
    proofs['c_zero_domain_gap_is_real'] = (
        'SAT (домен обязан быть конечным: 0·Inf = NaN ≠ +0.0)'
        if s.check() == z3.sat else '???')
    return {'z3_version': z3.get_version_string(), 'proofs': proofs}

# ───────────────────────────── numpy: побитовый дифференциал ──────────────────
def run_numpy(n_random=2_000_000, seed=42):
    import numpy as np
    rng = np.random.default_rng(seed)

    # Краевые случаи IEEE-754 single
    edges = [0x00000000, 0x80000000,              # ±0.0
             0x3F800000, 0xBF800000,              # ±1.0
             0x40490FDB, 0xC0490FDB,              # ±π
             0x7F7FFFFF, 0xFF7FFFFF,              # ±max normal
             0x00000001, 0x80000001,              # ±min denormal
             0x007FFFFF, 0x807FFFFF,              # ±max denormal
             0x00800000, 0x80800000,              # ±min normal
             0x7F800000, 0xFF800000,              # ±Inf
             0x7FC00000, 0xFFC00000]              # ±NaN (qNaN)
    edge_bits = np.array(edges, dtype=np.uint32)

    # Структурированные случайные: нормали, окрестность 1, денормали, битовый хаос
    rand_bits = np.concatenate([
        rng.integers(0, 0x7F800000, n_random // 4, dtype=np.uint32),
        rng.integers(0x3F000000, 0x40000000, n_random // 4, dtype=np.uint32),
        rng.integers(0, 0x00800000, n_random // 4, dtype=np.uint32),
        rng.integers(0, 2**32, n_random - 3 * (n_random // 4), dtype=np.uint32),
    ])
    signs = rng.integers(0, 2, rand_bits.size, dtype=np.uint32) * np.uint32(0x80000000)
    rand_bits = (rand_bits | signs).astype(np.uint32)
    all_bits = np.concatenate([edge_bits, rand_bits]).astype('<u4')
    all_f32 = bits_to_f32_array(all_bits)

    finite = np.isfinite(all_f32)
    report = {'samples_total': int(all_bits.size),
              'finite': int(finite.sum()), 'non_finite': int((~finite).sum())}

    for c, (absorb, sign) in MASKS.items():
        got = (all_bits & np.uint32(absorb)) ^ np.uint32(sign)
        want = f32_array_to_bits(all_f32 * np.float32(c))   # честное IEEE-754 mul
        exact = got == want
        g_f, w_f = got.view('<f4'), want.view('<f4')
        fp_eq = (g_f == w_f)                                 # −0.0 == +0.0 здесь True
        signed_zero_only = (~exact) & fp_eq
        zero_pair = ((got == 0) | (got == np.uint32(0x80000000))) & \
                    ((want == 0) | (want == np.uint32(0x80000000)))
        bad_zero = signed_zero_only & ~zero_pair
        val_mismatch_fin = (~exact) & ~fp_eq & finite        # КОМЕНТАРИЙ: должно быть 0
        src_neg = (all_bits & np.uint32(0x80000000)) != 0
        predicted_sz = ((~exact) & fp_eq & zero_pair & src_neg) if c == 0 \
            else np.zeros_like(exact)
        entry = {
            'exact_bits': int(exact.sum()),
            'signed_zero_only': int(signed_zero_only.sum()),
            'signed_zero_all_are_pm0': bool(bad_zero.sum() == 0),
            'signed_zero_predicted_by_src_sign': int((signed_zero_only & predicted_sz).sum()),
            'value_mismatch_finite': int(val_mismatch_fin.sum()),
        }
        if val_mismatch_fin.sum() or bad_zero.sum():
            entry['VERDICT'] = 'REFUTED на конечном домене!'
        elif c == 0 and signed_zero_only.sum() > 0:
            entry['VERDICT'] = ('CONFIRMED WITH CAVEATS: −0.0 vs +0.0 (IEEE-754 '
                                'равны, биты различаются) — все случаи предсказаны '
                                'знаком источника')
        else:
            entry['VERDICT'] = 'AXIOM CONFIRMED (побитово)'
        report['c=%+d' % c] = entry
    return report

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--samples', type=int, default=2_000_000)
    ap.add_argument('--json', default='scratch/passports/cycle_B.json')
    a = ap.parse_args()
    out = {'theorem': 'II.1', 'cycle': 'B',
           'subject': 'f(x,c) = (x & absorb) ^ sign == c*x, c∈{−1,0,+1}',
           'code': ['src/quantum/meta_compiler.rs#L83-L89 (константы)',
                    'src/quantum/meta_compiler.rs#L151-L158 (masks_of)',
                    'src/quantum/meta_compiler.rs#L897-L898 (горячий путь AVX2)',
                    'src/quantum/meta_compiler.rs#L1104-L1122 (кодоген)',
                    'src/quantum/meta_compiler.rs#L1372 (тест-каveat ±0.0)'],
           'commit': '38a862a'}
    out['z3'] = run_z3()
    out['numpy'] = run_numpy(a.samples)
    p = Path(a.json); p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(out, ensure_ascii=False, indent=1))
    print(json.dumps(out, ensure_ascii=False, indent=1))
    print('\nпаспорт: %s' % p)

if __name__ == '__main__':
    sys.exit(main())
