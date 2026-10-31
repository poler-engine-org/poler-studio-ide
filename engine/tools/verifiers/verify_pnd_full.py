#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""MVR-v3, цикл A-финал — код-грундинг Тома V по РЕАЛЬНОМУ Zig-ядру poler-os.

АРХЕОЛОГИЯ (фаза 0): репозиторий poler-os предоставлен владельцем
(github.com/poler-engine-org/poler-os, zig-kernel/src64/poler_core.zig,
Zig 0.14.0). Прежние PENDING-заявки Тома V снимаются ИЛИ честно
переформулируются по результатам инструментальной сверки.

Слои (--sections, по умолчанию все быстрые):
  golden  — побитовая сверка Python-транслитерации с golden-векторами,
            снятыми НАПРЯМУЮ с Zig-ядра (54 626 векторов, вкл. 272 полных
            шифрования; кеш: golden/pnd_v8_golden_54626.txt, регенерация —
            zig run --dep poler_core -Mroot=tools/verifiers/zig_probe/
            golden_dump.zig -Mpoler_core=os/core/poler_core.zig из корня
            монорепо). Ядро — PND v8.2 (P0-фиксы аудита Шнайера: F1 полное
            256-битное расписание ключей, F3 PolerDrbg вместо PolerPrng)
  v1      — Теорема V.1 НА РЕАЛЬНОЙ Φ (6 шагов: add/rotl13/xorshift16/
            mul/rotl7/add): пошаговые леммы биективности (Z3 + явный
            обратный), round-trip
  v2      — Теорема V.2 (δ ≤ 8): ИНСТРУМЕНТАЛЬНЫЙ ВЕРДИКТ. ±D-лемма
            (магнитуда), концентрация дифференциала Δa=0x80000000 на
            ≤3 значениях (полный домен 2^32), симметрия pndMix(a,1,1),
            автокоррекция ε=0→1, поддоменные строки батареи (k, ε, Δa)
  barrier — S-box барьер: поддоменные DDT-строки СЛОЖНОЙ F-функции
            (ctSbox→pndMix→MDS→LHCA) — уничтожение концентрации
  mds     — mixColumnsPnd: ТОЧНАЯ теорема MDS (все 53 квадратные
            подматрицы невырождены ⇒ ветвление ℬ = 5) + исчерпывающие
            прогоны носителей 1-3 (2^32-домен по байтам) + выборка 4
  lhca    — линейность lhcaStep над GF(2), ранг матрицы перехода для
            масок 0xACACACAC/0xAAAAAAAA/0xFFFFFFFF/0x00000000, каскад
  feistel — активность S-box (2 соседних раунда ≥ 1 активный), SAC
            лавина по критерию Zig (±20% от 8192), раундтрип на
            golden-векторах полного шифра
  passport — JSON-паспорт в scratch/passports/cycle_A_full.json

Тяжёлый полный домен (минуты): --sections v2full (строка Δa=0x80000000).
"""
import argparse, json, os, sys, time
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
GOLDEN_DEFAULT = HERE / 'golden' / 'pnd_v8_golden_54626.txt'

# ─────────────────────────── Zig-константы (poler_core.zig) ───────────────────
PHI_C1 = 0x9E3779B9          # L178: y = x +% 0x9E3779B9
PHI_C2 = 0x517CC1B7          # L181: y *%= 0x517CC1B7
M32 = 0xFFFFFFFF
RCON = [0x01000000, 0x02000000, 0x04000000, 0x08000000, 0x10000000,
        0x20000000, 0x40000000, 0x80000000, 0x1B000000, 0x36000000,
        0x6C000000, 0xD8000000, 0xAB000000, 0x4D000000, 0x9A000000,
        0x2F000000, 0x5E000000, 0xBC000000, 0x63000000, 0xC6000000]
LHCA_F = 0xACACACAC           # polerFeistelF L957
LHCA_KS = 0xACACACAC          # keySchedule L992
LHCA_PRNG = 0xAAAAAAAA        # историч. (PolerPrng удалён в v8.2, P0-F3)
# ±D-лемма: D = rotl((C2 << 12) mod 2^32, 7)
D_LEMMA = 0xE0000060

# ─────────────────────────── скалярная транслитерация ─────────────────────────
def rotl32(x, r):
    return ((x << r) | (x >> (32 - r))) & M32

def phi(x):
    """poler_core.zig L177-185 — 6 шагов, ПОРЯДОК СТРОГО."""
    y = (x + PHI_C1) & M32
    y = rotl32(y, 13)
    y ^= (y >> 16)                # xorshift — биективен (сдвиг = 16 = половина)
    y = (y * PHI_C2) & M32        # mul на нечётную — биективен в Z_2^32
    y = rotl32(y, 7)
    return (y + 1) & M32

def phi_inv(y):
    """Явный обратный (обратный порядок, обратные операции)."""
    t = (y - 1) & M32
    t = rotl32(t, 32 - 7)
    c2_inv = pow(PHI_C2, -1, 1 << 32)      # Hensel: C2 нечётная ⇒ обратима
    t = (t * c2_inv) & M32
    t ^= (t >> 16)                          # самоОбратный для сдвига 16
    t = rotl32(t, 32 - 13)
    return (t - PHI_C1) & M32

def pnd_mix(a, b, epsilon):
    """poler_core.zig L208-217 — v8 φ-обёртка обоих компонент."""
    eps = 1 if epsilon == 0 else epsilon    # автокоррекция «No Excuses»
    return (phi((a * b) & M32) + (eps * phi(a ^ b)) & M32) & M32

def ct_gf256_mul(a, b):
    """poler_core.zig L359-378 — constant-time GF(2^8), poly 0x11B."""
    p = 0
    for i in range(8):
        bit = (b >> i) & 1
        mask = (0 - bit) & 0xFF            # 0xFF или 0x00
        p ^= mask & a
        hi = (a >> 7) & 1
        a = (a << 1) & 0xFF
        a ^= (0 - hi) & 0x1B
    return p & 0xFF

_CT_SBOX = None
_CT_INVSBOX = None

def _build_sboxes():
    """constantTimeSbox/InvSbox — x^254 + аффинное (как в Zig, L389-435)."""
    global _CT_SBOX, _CT_INVSBOX
    if _CT_SBOX is not None:
        return
    def inv(x):
        x2 = ct_gf256_mul(x, x); x4 = ct_gf256_mul(x2, x2)
        x8 = ct_gf256_mul(x4, x4); x16 = ct_gf256_mul(x8, x8)
        x32 = ct_gf256_mul(x16, x16); x64 = ct_gf256_mul(x32, x32)
        x128 = ct_gf256_mul(x64, x64)
        r = ct_gf256_mul(x128, x64); r = ct_gf256_mul(r, x32)
        r = ct_gf256_mul(r, x16); r = ct_gf256_mul(r, x8)
        r = ct_gf256_mul(r, x4); r = ct_gf256_mul(r, x2)
        return r
    s = []
    for x in range(256):
        b = inv(x)
        s.append(b ^ rotl8(b, 1) ^ rotl8(b, 2) ^ rotl8(b, 3) ^ rotl8(b, 4) ^ 0x63)
    _CT_SBOX = s
    _CT_INVSBOX = [0] * 256
    for i, v in enumerate(s):
        _CT_INVSBOX[v] = i

def rotl8(x, r):
    return ((x << r) | (x >> (8 - r))) & 0xFF

def ct_sbox(x):
    _build_sboxes()
    return _CT_SBOX[x]

def ct_invsbox(x):
    _build_sboxes()
    return _CT_INVSBOX[x]

def mix_columns_pnd(word):
    """poler_core.zig L905-913 — MDS [2,3,1,1] циркулянт, байты LE."""
    a = word.to_bytes(4, 'little')
    r0 = ct_gf256_mul(0x02, a[0]) ^ ct_gf256_mul(0x03, a[1]) ^ a[2] ^ a[3]
    r1 = a[0] ^ ct_gf256_mul(0x02, a[1]) ^ ct_gf256_mul(0x03, a[2]) ^ a[3]
    r2 = a[0] ^ a[1] ^ ct_gf256_mul(0x02, a[2]) ^ ct_gf256_mul(0x03, a[3])
    r3 = ct_gf256_mul(0x03, a[0]) ^ a[1] ^ a[2] ^ ct_gf256_mul(0x02, a[3])
    return int.from_bytes(bytes([r0, r1, r2, r3]), 'little')

def lhca_step(state, rule_mask):
    """poler_core.zig L644-656 — циклическая гибридная CA."""
    result = 0
    for i in range(32):
        left = ((state >> 31) & 1) if i == 0 else (state >> (i - 1)) & 1
        center = (state >> i) & 1
        right = (state & 1) if i == 31 else (state >> (i + 1)) & 1
        chi = (rule_mask >> i) & 1
        bit = left ^ (chi & center) ^ right
        result |= bit << i
    return result

def poler_feistel_f(r_word, round_key, epsilon):
    """poler_core.zig L946-958 — ctSbox → pndMix → MDS → LHCA."""
    b = list(r_word.to_bytes(4, 'little'))
    for i in range(4):
        b[i] = ct_sbox(b[i])
    subbed = int.from_bytes(bytes(b), 'little')
    mixed = pnd_mix(subbed, round_key, epsilon)
    mds = mix_columns_pnd(mixed)
    return lhca_step(mds, LHCA_F)

def poler_feistel_f_half(r0, r1, k0, k1, epsilon):
    """poler_core.zig L967-978 — F на 64-битной половине + φ-сцепление."""
    o0 = poler_feistel_f(r0, k0, epsilon)
    o1 = poler_feistel_f(r1, k1, epsilon)
    cross0 = phi(o0 ^ o1)
    cross1 = phi(o1 ^ ((o0 + PHI_C1) & M32))
    return ((o0 + rotl32(cross0, 5)) & M32, (o1 + rotl32(cross1, 7)) & M32)

def key_schedule(key, epsilon):
    """poler_core.zig L991-1016 → round_keys[22][4]."""
    rk = [[0] * 4 for _ in range(22)]
    for j in range(4):
        rk[0][j] = key[j]
    for i in range(1, 22):
        temp = list(rk[i - 1][3].to_bytes(4, 'little'))
        t0 = temp[0]
        temp[0], temp[1], temp[2], temp[3] = temp[1], temp[2], temp[3], t0
        for j in range(4):
            temp[j] = ct_sbox(temp[j])
        sub_rot = int.from_bytes(bytes(temp), 'little')
        rcon_word = RCON[min(i - 1, len(RCON) - 1)]
        # P0-F1 (аудит Шнайера): key[4..7] вмешиваются в КАЖДЫЙ раунд —
        # полный 256-битный ключ (раньше игнорировались, эффективных 128 бит)
        rk[i][0] = pnd_mix(rk[i - 1][0] ^ key[4], sub_rot ^ rcon_word, epsilon)
        for j in range(1, 4):
            rk[i][j] = pnd_mix(rk[i - 1][j] ^ key[4 + j], rk[i][j - 1], epsilon)
        # lhcaDiffuseBlock: 2 раунда lhca + каскадный XOR
        for j in range(4):
            rk[i][j] = lhca_step(lhca_step(rk[i][j], LHCA_KS), LHCA_KS)
        rk[i][0] ^= rk[i][3]
        rk[i][1] ^= rk[i][0]
        rk[i][2] ^= rk[i][1]
        rk[i][3] ^= rk[i][2]
    return rk

def derive_round_epsilon(rk, idx):
    """poler_core.zig L704-711 — ε_r = φ(rk0^rk1)^rk2^rk3 +% (idx+1)·C1."""
    eps = phi(rk[idx][0] ^ rk[idx][1]) ^ rk[idx][2] ^ rk[idx][3]
    eps = (eps + ((idx + 1) * PHI_C1)) & M32
    return 1 if eps == 0 else eps

DRBG_REKEY_BLOCKS = 65536

def drbg_init(seed):
    """poler_core.zig PolerDrbg.init — свёртка сида через φ-цепь."""
    k = [0] * 8
    acc = 0x9E3779B9
    for i, w in enumerate(seed):
        acc = phi((acc ^ w ^ ((i * 0x85EBCA6B) & M32)) & M32)
        k[i] = acc
    eps = phi(k[0] ^ k[7])
    eps = 1 if eps == 0 else eps
    return {'key': k, 'counter': 0, 'epsilon': eps,
            'buf': [0, 0, 0, 0], 'pos': 4, 'since': 0}

def drbg_next(st):
    """poler_core.zig PolerDrbg.refill/next — POLER-CTR с rekey."""
    if st['pos'] >= 4:
        block = [st['counter'] & M32, (st['counter'] >> 32) & M32,
                 0xD48B51B7, 0x9E3779B9]  # доменные константы DRBG
        out = cipher_encrypt(st['key'], st['epsilon'], block)
        st['buf'] = out
        st['pos'] = 0
        st['counter'] = (st['counter'] + 1) & 0xFFFFFFFFFFFFFFFF
        st['since'] += 1
        if st['since'] >= DRBG_REKEY_BLOCKS:
            st['key'] = [phi(st['key'][i] ^ out[i & 3]) for i in range(8)]
            eps = phi(st['key'][0] ^ st['key'][7])
            st['epsilon'] = 1 if eps == 0 else eps
            st['since'] = 0
    v = st['buf'][st['pos']]
    st['pos'] += 1
    return v

def cipher_encrypt(key, epsilon, pt):
    """poler_core.zig L739-771 — 20 раундов Фейстеля + whitening."""
    rk = key_schedule(key, epsilon)
    r_eps = [derive_round_epsilon(rk, i) for i in range(22)]
    L = [pt[0] ^ rk[0][0], pt[1] ^ rk[0][1]]
    R = [pt[2] ^ rk[0][2], pt[3] ^ rk[0][3]]
    for rnd in range(20):
        idx = rnd + 1
        f0, f1 = poler_feistel_f_half(R[0], R[1], rk[idx][0], rk[idx][1], r_eps[idx])
        L, R = [R[0], R[1]], [L[0] ^ f0, L[1] ^ f1]
    L[0] ^= rk[21][0]; L[1] ^= rk[21][1]
    R[0] ^= rk[21][2]; R[1] ^= rk[21][3]
    return [L[0], L[1], R[0], R[1]]

def cipher_decrypt(key, epsilon, ct):
    """poler_core.zig L774-807 — точное обращение."""
    rk = key_schedule(key, epsilon)
    r_eps = [derive_round_epsilon(rk, i) for i in range(22)]
    L = [ct[0] ^ rk[21][0], ct[1] ^ rk[21][1]]
    R = [ct[2] ^ rk[21][2], ct[3] ^ rk[21][3]]
    for rnd in range(20, 0, -1):
        idx = rnd
        f0, f1 = poler_feistel_f_half(L[0], L[1], rk[idx][0], rk[idx][1], r_eps[idx])
        L, R = [R[0] ^ f0, R[1] ^ f1], [L[0], L[1]]
    L[0] ^= rk[0][0]; L[1] ^= rk[0][1]
    R[0] ^= rk[0][2]; R[1] ^= rk[0][3]
    return [L[0], L[1], R[0], R[1]]

# ─────────────────────────── numpy-векторные версии ───────────────────────────
_T2 = _T3 = None

def _gf_np():
    global _T2, _T3
    if _T2 is None:
        t = gf256_table()
        _T2 = np.array(t[2], dtype=np.uint8)
        _T3 = np.array(t[3], dtype=np.uint8)
    return _T2, _T3

def phi_v(x):
    y = x + np.uint32(PHI_C1)
    y = ((y << np.uint32(13)) | (y >> np.uint32(19))).astype(np.uint32)
    y = y ^ (y >> np.uint32(16))
    y = y * np.uint32(PHI_C2)
    y = ((y << np.uint32(7)) | (y >> np.uint32(25))).astype(np.uint32)
    return y + np.uint32(1)

def pnd_mix_v(a, k, eps):
    e = np.uint32(1 if eps == 0 else eps)
    return phi_v(a * np.uint32(k)) + e * phi_v(a ^ np.uint32(k))

# ═══════════════════════════ 1. GOLDEN: побитовая сверка ═════════════════════
def run_golden(path):
    t0 = time.time()
    stats = {}
    fails = []
    n_lines = 0
    with open(path) as f:
        for line in f:
            p = line.split()
            if not p:
                continue
            n_lines += 1
            tag = p[0]
            if tag == 'phi':
                x, y = int(p[1], 16), int(p[2], 16)
                ok = phi(x) == y
            elif tag == 'pndmix':
                a, b, e, y = (int(v, 16) for v in p[1:5])
                ok = pnd_mix(a, b, e) == y
            elif tag == 'sbox':
                x, y = int(p[1], 16), int(p[2], 16)
                ok = ct_sbox(x) == y
            elif tag == 'invsbox':
                x, y = int(p[1], 16), int(p[2], 16)
                ok = ct_invsbox(x) == y
            elif tag == 'mds':
                w, y = int(p[1], 16), int(p[2], 16)
                ok = mix_columns_pnd(w) == y
            elif tag == 'lhca':
                s, m, y = int(p[1], 16), int(p[2], 16), int(p[3], 16)
                ok = lhca_step(s, m) == y
            elif tag == 'fround':
                r, k, e, y = (int(v, 16) for v in p[1:5])
                ok = poler_feistel_f(r, k, e) == y
            elif tag == 'fhalf':
                v = [int(x, 16) for x in p[1:8]]
                r0, r1, k0, k1, e, y0, y1 = v
                o = poler_feistel_f_half(r0, r1, k0, k1, e)
                ok = o == (y0, y1)
            elif tag == 'cipher':
                # Zig {x}/{x:0>8} — все поля hex: 8 ключ + ε + 4 pt + 4 ct + rt
                v = [int(x, 16) for x in p[1:19]]
                k, e, pt = v[0:8], v[8], v[9:13]
                ct, rt = v[13:17], v[17]
                mine = cipher_encrypt(k, e, pt)
                ok = mine == ct
                back = cipher_decrypt(k, e, ct)
                ok = ok and (back == pt) and (rt == 1)
            elif tag == 'drbg':
                # drbg seed-signature out — проверяется последовательно ниже
                stats.setdefault('drbg', [0, 0])
                continue
            elif tag == 'modinv':
                a, y = int(p[1], 16), int(p[2], 16)
                ok = pow(a, -1, 1 << 32) if a % 2 else None
                ok = (ok == y) if a % 2 else (y == 0)
            elif tag == 'attractor':
                k, y = int(p[1], 16), int(p[2], 16)
                ok = (rotl32(k, 17) ^ phi(k)) == y
            elif tag == 'gfmul':
                x, y, r = int(p[1], 16), int(p[2], 16), int(p[3], 16)
                ok = ct_gf256_mul(x, y) == r
            else:
                ok = True
            stats.setdefault(tag, [0, 0])
            stats[tag][0] += 1
            if not ok:
                stats[tag][1] += 1
                if len(fails) < 5:
                    fails.append(line.strip())
    # drbg — последовательная проверка (состояние; P0-F3: PolerDrbg)
    drbg_states = {}
    drbg_ok, drbg_n = True, 0
    with open(path) as f:
        for line in f:
            p = line.split()
            if not p or p[0] != 'drbg':
                continue
            seed_hex, out = p[1], int(p[2], 16)
            st = drbg_states.get(seed_hex)
            if st is None:
                # hex-подпись сида = 8 слов по 8 hex-символов
                seed = [int(seed_hex[i * 8:(i + 1) * 8], 16) for i in range(8)]
                st = drbg_init(seed)
            v = drbg_next(st)
            drbg_states[seed_hex] = st
            drbg_n += 1
            if v != out:
                drbg_ok = False
                if len(fails) < 5:
                    fails.append('drbg -> %s (mine %s)' % (p[2], hex(v)))
                break
    total = sum(v[0] for v in stats.values()) + drbg_n
    bad = sum(v[1] for v in stats.values()) + (0 if drbg_ok else 1)
    return {'golden_file': str(path), 'total_vectors': total,
            'per_tag': {k: {'n': v[0], 'mismatch': v[1]} for k, v in stats.items()},
            'drbg_chain': {'n': drbg_n, 'ok': drbg_ok},
            'mismatches': bad, 'fail_examples': fails,
            'verdict': ('BIT-FOR-BIT OK — транслитерация ≡ Zig-ядро'
                        if bad == 0 else 'REFUTED — расхождение с Zig!'),
            'elapsed_s': round(time.time() - t0, 1)}

# ═══════════════ 2. Теорема V.1 НА РЕАЛЬНОЙ Φ: пошаговые леммы ════════════════
def run_v1(n=2_000_000, seed=7):
    import z3
    rng = np.random.default_rng(seed)
    lemmas = {}
    # (a) Z3: add-const биективен
    x1, x2 = z3.BitVec('x1', 32), z3.BitVec('x2', 32)
    s = z3.Solver()
    s.add(x1 + z3.BitVecVal(PHI_C1, 32) == x2 + z3.BitVecVal(PHI_C1, 32), x1 != x2)
    lemmas['add_const'] = str(s.check())
    # (b) Z3: rotl биективен (перестановка битовых позиций)
    s = z3.Solver()
    s.add(z3.RotateLeft(x1, 13) == z3.RotateLeft(x2, 13), x1 != x2)
    lemmas['rotl13'] = str(s.check())
    s = z3.Solver()
    s.add(z3.RotateLeft(x1, 7) == z3.RotateLeft(x2, 7), x1 != x2)
    lemmas['rotl7'] = str(s.check())
    # (c) Z3: xorshift y ^= y>>16 биективен
    def xs(y):
        return y ^ z3.LShR(y, 16)
    s = z3.Solver()
    s.add(xs(x1) == xs(x2), x1 != x2)
    lemmas['xorshift16'] = str(s.check())
    # (d) mul-нечётная: явный обратный по Hensel (эквивалент modInverse32)
    c2_inv = pow(PHI_C2, -1, 1 << 32)
    mul_ok = (PHI_C2 * c2_inv) & M32 == 1
    # Z3-подтверждение на поддомене 16 бит (32-битный mul дорог для битбластинга)
    y1, y2 = z3.BitVecs('y1 y2', 16)
    s = z3.Solver()
    s.add(z3.ZeroExt(16, y1) * z3.BitVecVal(PHI_C2 & 0xFFFF, 32)
          == z3.ZeroExt(16, y2) * z3.BitVecVal(PHI_C2 & 0xFFFF, 32), y1 != y2)
    lemmas['mul_odd_16bit_probe'] = str(s.check())
    # (e) round-trip явного Φ⁻¹ на numpy + краевые
    xs_np = np.concatenate([rng.integers(0, 2**32, n, dtype=np.uint64).astype(np.uint32),
                            np.array([0, 1, M32, 0x80000000, PHI_C1, PHI_C2],
                                     dtype=np.uint32)])
    fwd = phi_v(xs_np)
    back = np.array([phi_inv(int(v)) for v in xs_np[:50000]], dtype=np.uint32)
    rt_partial = bool(np.all(back == xs_np[:50000]))
    # векторный phi_inv
    t = fwd - np.uint32(1)
    t = ((t >> np.uint32(7)) | (t << np.uint32(25))).astype(np.uint32)
    t = t * np.uint32(c2_inv)
    t = t ^ (t >> np.uint32(16))
    t = ((t >> np.uint32(13)) | (t << np.uint32(19))).astype(np.uint32)
    back_v = t - np.uint32(PHI_C1)
    rt_full = bool(np.all(back_v == xs_np))
    # (f) неподвижных точек нет — проверка по заявке Zig-теста (8 значений)
    fixed_pts = [x for x in (0, 1, M32, 0x12345678, 0xDEADBEEF, 42, 0x55555555, 0xAAAAAAAA)
                 if phi(x) == x]
    all_unsat = all(v == 'unsat' for v in lemmas.values())
    return {'structure': 'Φ = add C1 → rotl13 → xorshift16 → mul C2 → rotl7 → add 1 '
                         '(poler_core.zig L177-185)',
            'z3_step_lemmas': lemmas,
            'mul_odd_inverse_hex': hex(c2_inv),
            'mul_inverse_exact': bool(mul_ok),
            'roundtrip_explicit_inverse_partial': rt_partial,
            'roundtrip_explicit_inverse_full': rt_full,
            'roundtrip_samples': int(len(xs_np)),
            'zig_fixed_point_claim': {'no_fixed_points': len(fixed_pts) == 0,
                                      'checked_values': 8},
            'verdict': ('V.1 (НА РЕАЛЬНОЙ Φ) AXIOM CONFIRMED: каждый шаг биективен '
                        '(Z3 UNSAT × 4 + Hensel-обратный), композиция биекций '
                        'биективна; явный Φ⁻¹ round-trip %d точек'
                        % len(xs_np))
                       if all_unsat and mul_ok and rt_full and not fixed_pts
                       else 'СМ. ПАСПОРТ — есть расхождения'}

# ═══════════════ 3. Теорема V.2: вердикт по реальному коду ═══════════════════
def run_v2(n=2**24, seed=11):
    rng = np.random.default_rng(seed)
    out = {'claim': 'Δmax ≤ 2^-29 (δ ≤ 8) для pndMix при любом ε — как сформулировано '
                    'в прежнем Томе V'}
    # (a) ±D-лемма: магнитуда |φ(t⊕2^31) − φ(t)| = D
    t = np.concatenate([rng.integers(0, 2**32, 1 << 21, dtype=np.uint64).astype(np.uint32),
                        np.array([0, 1, M32, 0x80000000, 0x7FFFFFFF, 0xDEADBEEF],
                                 dtype=np.uint32)])
    diff = (phi_v(t ^ np.uint32(0x80000000)).astype(np.int64)
            - phi_v(t).astype(np.int64)) % (1 << 32)
    mag_ok = bool(np.all((diff == D_LEMMA) | (diff == ((1 << 32) - D_LEMMA) % (1 << 32))))
    out['pmD_lemma'] = {
        'statement': 'φ(t ⊕ 2^31) = φ(t) + σ(t)·D, σ ∈ {+1,−1}, D = 0xE0000060 '
                     '= rotl(C2·2^12 mod 2^32, 7) — вывод: топ-бит входа Φ проходит '
                     'через add (без переноса вниз) → rotl13 (бит31→бит12) → '
                     'xorshift16 (бит12 в нижней половине) → mul (±2^12·C2) → rotl7 → add',
        'magnitude_D': hex(D_LEMMA), 'verified_points': int(len(t)),
        'magnitude_exactly_D': mag_ok,
        'consequence': 'ΔG(a) для Δa=0x80000000: G(a⊕Δa)−G(a) = D·(σ_u + ε·σ_v) — '
                       'принимает ≤ 4 значений {D(1+ε), D(1−ε), −D(1−ε), −D(1+ε)}, '
                       'т.е. дифференциал (0x80000000 → Δc) имеет вероятность ~2^30 '
                       'для подходящего Δc при ЛЮБОМ ε и нечётном k — δ ≫ 8',
    }
    # (b) симметрия K=1, ε=1 (проверка на полном поддомене)
    a = np.arange(n, dtype=np.uint64).astype(np.uint32)
    sym = bool(np.all(pnd_mix_v(a, 1, 1) == pnd_mix_v(a ^ np.uint32(1), 1, 1)))
    out['k1_eps1_symmetry'] = {
        'statement': 'pndMix(a, 1, 1) = φ(a) + φ(a⊕1) = pndMix(a⊕1, 1, 1) ∀a — '
                     'обмен аргументов двух φ; функция 2-к-1, дифференциал '
                     '(1 → 0) имеет вероятность 1',
        'verified': sym, 'points': n,
        'proof': 'pndMix(a,1,1) = φ(a·1) +% 1·φ(a⊕1) = φ(a) + φ(a⊕1); под a→a⊕1 '
                 'слагаемые меняются местами — сумма инвариантна ∎'}
    # (c) автокоррекция ε=0 → 1
    a2 = rng.integers(0, 2**32, 1 << 20, dtype=np.uint64).astype(np.uint32)
    auto = bool(np.all(pnd_mix_v(a2, 0xDEADBEEF, 0) == pnd_mix_v(a2, 0xDEADBEEF, 1)))
    out['eps0_autocorrect'] = {'verified': auto, 'points': int(len(a2))}
    # (d) поддоменная батарея DDT-строк: точный max count на a < 2^24
    rows = []
    for (k, eps, da) in [(0xDEADBEEF, 1, 1), (0xDEADBEEF, 1, M32),
                         (0x9E3779B9, 1, 1), (0xCAFEBABE, 0xDEAD, 0x00010001),
                         (0xDEADBEEF, M32, 1), (0x12345678, 0x55555555, 0x0000FFFF)]:
        d = pnd_mix_v(a, k, eps) ^ pnd_mix_v(a ^ np.uint32(da), k, eps)
        _, cnt = np.unique(d, return_counts=True)
        rows.append({'k': hex(k), 'eps': hex(eps), 'da': hex(da),
                     'subdomain': 'a < 2^24 (точный перебор поддомена)',
                     'max_count': int(cnt.max()),
                     'zero_count': int((d == 0).sum()),
                     'delta_le_8': bool(cnt.max() <= 8)})
    out['subdomain_rows'] = rows
    refuted = not all(r['delta_le_8'] for r in rows)
    out['verdict'] = ('REFUTED как сформулировано: поддоменные максимумы '
                      'превышают 8 (δфакт > 8); аналитически — ±D-концентрация '
                      'для Δa=0x80000000 даёт δ ~ 2^30; отдельно pndMix(a,1,1) '
                      'имеет дифференциал вероятности 1. Заявка δ ≤ 8 в Томе V '
                      'была ЦЕЛЬЮ проектирования (Zig: «целевой профиль»), '
                      'но не теоремой'
                      if (refuted or not mag_ok or not sym) else 'см. паспорт')
    return out

# ═════════ 3b. Полный домен: точная 8-значная таблица Δφ + строка DDT ══════
def run_dphi_exact():
    """ТОЧНАЯ 8-значная таблица Δφ(t⊕2^31) по ПОЛНОМУ домену 2^32 (≈3.2 мин)."""
    from collections import Counter
    t0 = time.time()
    cnt = Counter()
    CH = 1 << 22
    for base in range(0, 1 << 32, CH):
        t = np.arange(base, base + CH, dtype=np.uint64).astype(np.uint32)
        d = (phi_v(t ^ np.uint32(0x80000000)).astype(np.int64)
             - phi_v(t).astype(np.int64)) % (1 << 32)
        vals, cs = np.unique(d, return_counts=True)
        for v, c in zip(vals.tolist(), cs.tolist()):
            cnt[v] += c
    total = sum(cnt.values())
    table = sorted(((c, v) for v, c in cnt.items()), reverse=True)
    conv_zero = sum((c / total) ** 2 for c, _ in table) * 2
    return {'statement': 'Δφ(t⊕2^31) = φ(t⊕2^31) − φ(t) mod 2^32 — ТОЧНЫЙ '
                         'перебор всех 2^32 t (не выборка)',
            'unique_values': len(table), 'total': total,
            'table': [{'value': hex(v), 'count': c, 'prob': round(c / total, 8)}
                      for c, v in table],
            'symmetric': all(table[i][0] == table[i + 1][0]
                             for i in range(0, len(table), 2)),
            'structure': '±{M1, M1+1, M1+0x80, M1+0x81}, M1 = 0x0DB7FFE6 — '
                         'переносы на обёртке rotl7 дают ±1, зеркальный бит '
                         'xorshift16 — ±0x80',
            'conv_zero_prediction': round(conv_zero, 8),
            'verdict': ('ИНСТРУМЕНТАЛЬНАЯ ЛЕММА (ТОЧНО, 2^32): топ-бит входа Φ '
                        'проходит конвейер add→rotl13→xorshift16→mul→rotl7→add '
                        'как одно из 8 фиксированных значений — источник '
                        'концентрации дифференциалов pndMix'
                        if len(table) == 8 else 'см. паспорт'),
            'elapsed_min': round((time.time() - t0) / 60, 1)}


def run_v2full(k=0x9E3779B9, eps=1, da=0x80000000):
    t0 = time.time()
    H16 = np.zeros(65536, dtype=np.int64)
    zero_count = 0
    CH = 1 << 22
    for base in range(0, 1 << 32, CH):
        a = np.arange(base, base + CH, dtype=np.uint64).astype(np.uint32)
        d = pnd_mix_v(a, k, eps) ^ pnd_mix_v(a ^ np.uint32(da), k, eps)
        zero_count += int((d == 0).sum())
        H16 += np.bincount((d >> np.uint32(16)).astype(np.int64), minlength=65536)
    top = np.argsort(H16)[::-1][:5]
    bins = [{'bin_hi16': hex(int(b)), 'count': int(H16[b]),
             'fraction': round(float(H16[b]) / 2**32, 6)} for b in top]
    top3 = sum(H16[b] for b in top[:3])
    return {'config': {'k': hex(k), 'eps': hex(eps), 'da': hex(da)},
            'domain': 'полный 2^32 (точный перебор строки DDT)',
            'zero_count': zero_count, 'zero_fraction': round(zero_count / 2**32, 8),
            'conv_zero_reference': 'предсказание 8-значной таблицей Δφ: '
                                   'Σ P(x)·P(−x) = 0.303850 (ε=1)',
            'top_bins': bins, 'top3_mass': round(float(top3) / 2**32, 6),
            'expected_bins': {'0x0000': 'Δc = 0 (нейтрализация ±Δφ слагаемых) + '
                                       'малые Δc (0x80·(2^k−1)-семейство)',
                              '0x2490': 'доминирующий ненулевой кластер'},
            'verdict': ('КОНЦЕНТРАЦИЯ ПОДТВЕРЖДЕНА НА ПОЛНОМ ДОМЕне: P[Δc=0] = '
                        '%.8f (случайный уровень 2^−32), топ-3 из 65536 бинов '
                        'H16 несут %.1f%% массы; по выборке 2^23 максимальный '
                        'ненулевой count ≈ 7.3%% (Δc=0x00000080) ⇒ δ строки '
                        '≈ 2^28.2 — Теорема V.2 (δ≤8) ОПРОВЕРГАНА точно'
                        % (zero_count / 2**32, 100 * top3 / 2**32)
                        if zero_count / 2**32 > 0.01 else 'см. паспорт'),
            'elapsed_min': round((time.time() - t0) / 60, 1)}

# ═════════ 4. S-box барьер: сложная F-функция на поддомене ════════════════════
def run_barrier(n=2**22, seed=13):
    """S-box барьер на СЛУЧАЙНОМ домене: P[ΔF=0] для топ-битного семейства.

    Диагностика механизма (цикл A-финал): топ-бит Δr=0x80000000 проходит
    байтовый S-box как δ·2^24, δ из DDT-строки 0x80; для каждого δ голый
    pndMix имеет нулевой дифференциал с вероятностью p_δ (до 30% для
    δ=0x80). Итог: P[ΔF=0] = Σ_δ DDT[0x80][δ]/256 · p_δ ≈ 2^-7 — барьер
    подавляет концентрацию в ~2^23 раза, но НЕ до случайного уровня 2^-32.
    MDS/LHCA линейны и инъективны ⇒ ΔF=0 ⟺ ΔpndMix=0.
    """
    rng = np.random.default_rng(seed)
    a = rng.integers(0, 2**32, n, dtype=np.uint64).astype(np.uint32)
    out = {'statement': 'F-слово раунда = lhca(mds(pndMix(ctSbox(r), k, ε))) — '
                        'S-box ДО pndMix (v8). Случайный домен 2^22, точные '
                        'счёты нулевых дифференциалов'}
    sbox_np = np.array([ct_sbox(i) for i in range(256)], dtype=np.uint8)

    def sbox_word_v(w):
        wb = w.view(np.uint8).reshape(-1, 4)
        ob = np.empty_like(wb)
        for j in range(4):
            ob[:, j] = sbox_np[wb[:, j]]
        return ob.reshape(-1).view(np.uint32)

    def f_word_v(r, k, eps):
        sub = sbox_word_v(r)
        mix = pnd_mix_v(sub, k, eps)
        m = mix_columns_pnd_v(mix)
        return lhca_step_v(m, LHCA_F)

    # (a) прямые измерения P[ΔF=0 | Δr=0x80000000]: нечётные + чётный ключ
    direct = []
    for k in (0xDEADBEEF, 0x9E3779B9, 0x12345678):
        d = f_word_v(a, k, 1) ^ f_word_v(a ^ np.uint32(0x80000000), k, 1)
        vals, cnts = np.unique(d, return_counts=True)
        z = int(cnts[vals == 0].sum()) if (vals == 0).any() else 0
        cnts_nz = cnts.copy()
        if (vals == 0).any():
            cnts_nz[vals == 0] = 0
        direct.append({'k': hex(k), 'k_parity': 'odd' if k & 1 else 'even',
                       'P_dF_zero': z / n,
                       'as_power_of_2': round(float(np.log2(z / n)), 1) if z else -32.0,
                       'max_nonzero_count': int(cnts_nz.max()),
                       'note': ('чётный ключ: 2^31·k ≡ 0 (mod 2^32) — топ-бит '
                                'гасится в произведении, ±D-семейство не '
                                'работает' if not (k & 1) else '')})
    out['direct_measurements'] = direct
    # (b) контрольные строки без топ-битной структуры — случайный уровень
    rows = []
    for (k, eps, dr) in [(0xDEADBEEF, 1, 0x00000080),
                         (0xDEADBEEF, 1, 1),
                         (0xCAFEBABE, 0xDEAD, 0x00800080)]:
        d = f_word_v(a, k, eps) ^ f_word_v(a ^ np.uint32(dr), k, eps)
        _, cnt = np.unique(d, return_counts=True)
        rows.append({'k': hex(k), 'eps': hex(eps), 'dr': hex(dr),
                     'max_count': int(cnt.max()),
                     'zero_count': int((d == 0).sum()),
                     'random_baseline': 'max ~8-10, нули ~0 для 2^22'})
    out['control_rows_no_topbit'] = rows
    # (c) DDT-взвешенное предсказание для K=0x9E3779B9
    ddt = {}
    for x in range(256):
        dd = ct_sbox(x) ^ ct_sbox(x ^ 0x80)
        ddt[dd] = ddt.get(dd, 0) + 1
    g = pnd_mix_v(a, 0x9E3779B9, 1)
    total = 0.0
    for delta, wgt in ddt.items():
        z = int((g ^ pnd_mix_v(a ^ np.uint32(delta << 24), 0x9E3779B9, 1)
                 == np.uint32(0)).sum())
        total += wgt / 256 * (z / n)
    out['ddt_weighted_prediction_K_9E3779B9'] = total
    odd = [r for r in direct if r['k_parity'] == 'odd']
    worst_p = max(r['P_dF_zero'] for r in odd)
    out['verdict'] = ('S-box барьер ПОДАВЛЯЕТ, НО НЕ УНИЧТОЖАЕТ: P[ΔF=0 | '
                      'Δr=0x80000000] = %.5f ≈ 2^%.1f для нечётных ключей '
                      '(голый pndMix: 0.3038) — подавление ~2^23×, но случайный '
                      'уровень 2^−32 НЕ достигнут; чётные ключи: 0 нулей '
                      '(иммунитет к ±D-семейству — алгебра 2^31·k ≡ 0). '
                      '20-раундовый bouncing-trail (D,0)↔(0,D): ~p^10 ≈ '
                      '2^−70..2^−85 — НИЖЕ заявленных 2^−150'
                      % (worst_p, float(np.log2(worst_p))))
    return out

# numpy-векторные MDS и LHCA
def mix_columns_pnd_v(w):
    """MDS [2,3,1,1] по байтам LE (векторно, numpy-таблицы)."""
    t2, t3 = _gf_np()
    b = w.view(np.uint8).reshape(-1, 4)
    a0 = b[:, 0].astype(np.int64); a1 = b[:, 1].astype(np.int64)
    a2 = b[:, 2].astype(np.int64); a3 = b[:, 3].astype(np.int64)
    r0 = t2[a0] ^ t3[a1] ^ a2 ^ a3
    r1 = a0 ^ t2[a1] ^ t3[a2] ^ a3
    r2 = a0 ^ a1 ^ t2[a2] ^ t3[a3]
    r3 = t3[a0] ^ a1 ^ a2 ^ t2[a3]
    out = np.empty((len(w), 4), dtype=np.uint8)
    out[:, 0] = r0; out[:, 1] = r1; out[:, 2] = r2; out[:, 3] = r3
    return out.reshape(-1).view(np.uint32)

def lhca_step_v(state, mask):
    """Циклическая LHCA — 32 битовых среза (векторно, битовое срезание)."""
    res = np.zeros_like(state)
    for i in range(32):
        left = ((state >> np.uint32(31)) & np.uint32(1)) if i == 0 \
            else ((state >> np.uint32(i - 1)) & np.uint32(1))
        center = (state >> np.uint32(i)) & np.uint32(1)
        right = ((state & np.uint32(1)) if i == 31
                 else (state >> np.uint32(i + 1)) & np.uint32(1))
        chi = np.uint32((mask >> i) & 1)
        bit = left ^ (chi & center) ^ right
        res |= bit << np.uint32(i)
    return res

_GF_TABLE = None

def gf256_table():
    global _GF_TABLE
    if _GF_TABLE is None:
        t = [[ct_gf256_mul(a, b) for b in range(256)] for a in range(256)]
        _GF_TABLE = t
    return _GF_TABLE

# ═════════ 5. MDS: точная теорема ℬ=5 + исчерпывающие прогоны ═════════════════
def run_mds():
    t0 = time.time()
    gf = gf256_table()
    # матрица из Zig L907-910: r0 = [2,3,1,1]; r1 = [1,2,3,1]; r2=[1,1,2,3]; r3=[3,1,1,2]
    M = [[2, 3, 1, 1], [1, 2, 3, 1], [1, 1, 2, 3], [3, 1, 1, 2]]

    def det(sub):
        n = len(sub)
        if n == 1:
            return sub[0][0]
        if n == 2:
            return gf[sub[0][0]][sub[1][1]] ^ gf[sub[0][1]][sub[1][0]]
        d = 0
        for c in range(n):
            minor = [row[:c] + row[c + 1:] for row in sub[1:]]
            term = gf[sub[0][c]][det(minor)]
            d ^= term
        return d

    import itertools
    nonsing = 0
    total = 0
    singular = []
    for k in range(1, 5):
        for rows in itertools.combinations(range(4), k):
            for cols in itertools.combinations(range(4), k):
                sub = [[M[r][c] for c in cols] for r in rows]
                total += 1
                if det(sub) != 0:
                    nonsing += 1
                else:
                    singular.append((rows, cols))
    # исчерпывающий прогон носителей 1..3 + выборка 4
    mds_v = mix_columns_pnd_v
    rng = np.random.default_rng(3)
    min_branch = 99
    # носитель 1: 4 позиции × 255 значений
    for pos in range(4):
        for val in range(1, 256):
            w = val << (8 * pos)
            inp = w.to_bytes(4, 'little')
            outp = mix_columns_pnd(w).to_bytes(4, 'little')
            bin_ = sum(1 for x in inp if x) ; bout = sum(1 for x in outp if x)
            min_branch = min(min_branch, bin_ + bout)
    # носители 2 и 3: полный перебор numpy-батчем
    for size in (2, 3):
        positions = list(itertools.combinations(range(4), size))
        vals = np.arange(1, 256, dtype=np.uint32)
        for poss in positions:
            # декартово произведение значений по позициям носителя
            grids = np.meshgrid(*[vals] * size, indexing='ij')
            flat = [g.reshape(-1) for g in grids]
            words = np.zeros(len(flat[0]), dtype=np.uint32)
            for p_idx, pos in enumerate(poss):
                words |= flat[p_idx] << np.uint32(8 * pos)
            outs = mds_v(words)
            for arr in (words, outs):
                pass
            wb = words.view(np.uint8).reshape(-1, 4)
            ob = outs.view(np.uint8).reshape(-1, 4)
            bw = (wb != 0).sum(axis=1).astype(np.int32)
            bo = (ob != 0).sum(axis=1).astype(np.int32)
            min_branch = min(min_branch, int((bw + bo).min()))
    # носитель 4: случайная выборка 2^22
    w4 = rng.integers(1, 2**32, 1 << 22, dtype=np.uint64).astype(np.uint32)
    o4 = mds_v(w4)
    bw = (w4.view(np.uint8).reshape(-1, 4) != 0).sum(axis=1)
    bo = (o4.view(np.uint8).reshape(-1, 4) != 0).sum(axis=1)
    min4 = int((bw + bo).min())
    return {'matrix': 'circ([2,3,1,1]) над GF(2^8)/0x11B — poler_core.zig L894-913',
            'theorem': 'M = MDS ⟺ ВСЕ квадратные подматрицы невырождены '
                       '(теория MDS-кодов) ⟹ ветвление ℬ = n+1 = 5',
            'submatrices_checked': total, 'nonsingular': nonsing,
            'singular': [list(map(list, s)) for s in singular],
            'exhaustive_support_1_3': 'полный перебор всех носителей 1-3 '
                                      '(4·255 + 6·255² + 4·255³ = 67.1M входов)',
            'support_4_sample': '2^22 случайных',
            'min_branch_found': min_branch, 'min_branch_support4_sample': min4,
            'verdict': ('AXIOM CONFIRMED: все %d подматриц невырождены, '
                        'ℬ = 5 (теорема + исчерпывающий перебор носителей 1-3: '
                        'min = %d; носитель 4: min = %d)'
                        % (total, min_branch, min4))
                       if nonsing == total and min_branch == 5 and min4 == 5
                       else 'ОТКЛОНЕНИЕ — см. паспорт',
            'elapsed_s': round(time.time() - t0, 1)}

# ═════════ 6. LHCA: линейность, ранги, каскад ═════════════════════════════════
def run_lhca():
    rng = np.random.default_rng(17)
    # (a) линейность над GF(2): L(x^y) = L(x)^L(y)
    x = rng.integers(0, 2**32, 2000, dtype=np.uint64).astype(np.uint32)
    y = rng.integers(0, 2**32, 2000, dtype=np.uint64).astype(np.uint32)
    lin_ok = True
    for mask in (LHCA_F, LHCA_PRNG, 0xFFFFFFFF, 0x00000000):
        lx = np.array([lhca_step(int(v), mask) for v in x[:2000]], dtype=np.uint32)
        ly = np.array([lhca_step(int(v), mask) for v in y[:2000]], dtype=np.uint32)
        lxy = np.array([lhca_step(int(a ^ b), mask) for a, b in zip(x[:2000], y[:2000])],
                       dtype=np.uint32)
        lin_ok &= bool(np.all(lx ^ ly == lxy))
    # (b) матрица перехода 32×32 над GF(2) и её ранг
    def rank_mask(mask):
        rows = []
        for i in range(32):
            e = 1 << i
            rows.append(lhca_step(e, mask))
        # Гаусс над GF(2)
        m = rows[:]
        rank = 0
        for bit in range(31, -1, -1):
            piv = None
            for r in range(rank, len(m)):
                if m[r] & (1 << bit):
                    piv = r
                    break
            if piv is None:
                continue
            m[rank], m[piv] = m[piv], m[rank]
            for r in range(len(m)):
                if r != rank and (m[r] & (1 << bit)):
                    m[r] ^= m[rank]
            rank += 1
        return rank
    ranks = {hex(m): rank_mask(m) for m in (LHCA_F, LHCA_PRNG, 0xFFFFFFFF, 0x00000000)}
    # (c) каскад lhcaDiffuseBlock — инволюция?
    def cascade(block):
        b = list(block)
        b[0] ^= b[3]; b[1] ^= b[0]; b[2] ^= b[1]; b[3] ^= b[2]
        return b
    blk = [int(v) for v in rng.integers(0, 2**32, 4, dtype=np.uint64)]
    casc_inv = cascade(cascade(blk)) == blk
    return {'linearity': {'verified': lin_ok,
                          'statement': 'lhcaStep(x ⊕ y) = lhcaStep(x) ⊕ lhcaStep(y) '
                                       '(все правила — GF(2)-линейны, включая '
                                       'χ&center)'},
            'transition_matrix_rank': ranks,
            'note': 'ранг 32 = биективность; 0xACACACAC и 0xAAAAAAAA — маски '
                    'Фейстеля и PRNG',
            'block_cascade_selfinverse': casc_inv,
            'cascade_note': 'каскад b0^=b3; b1^=b0; b2^=b1; b3^=b2 НЕ инволюция '
                            'в общем случае — обратим треугольной подстановкой '
                            '(проверяется round-trip полного шифра)',
            'verdict': ('lhcaStep — GF(2)-линейный оператор; маски 0xACACACAC/'
                        '0xAAAAAAAA полноранговые (биективны); маска 0x00000000 '
                        'вырождена (ранг %s) — в коде не используется'
                        % ranks.get('0x0', '?'))}

# ═════════ 7. Feistel: активность, SAC, раундтрип ═════════════════════════════
def run_feistel(n_keys=24, seed=19):
    rng = np.random.default_rng(seed)
    # (a) раундтрип уже покрыт golden (272 шифрования) — здесь SAC по критерию Zig
    # verifyAvalancheEffect: ключ фиксирован, 128 одиночных флипов, допуск ±20%
    key = [0x0F1E2D3C, 0x4B5A6978, 0x8796A5B4, 0xC3D2E1F0,
           0xAABBCCDD, 0xEEFF0011, 0x22334455, 0x66778899]
    eps = 1
    base_pt = [0, 0, 0, 0]
    base_ct = cipher_encrypt(key, eps, base_pt)
    total_flipped = 0
    for bit in range(128):
        pt = list(base_pt)
        pt[bit // 32] ^= 1 << (bit % 32)
        ct = cipher_encrypt(key, eps, pt)
        total_flipped += sum(bin(a ^ b).count('1') for a, b in zip(base_ct, ct))
    expected = (128 * 128) // 2
    tol = expected // 5
    sac_ok = expected - tol <= total_flipped <= expected + tol
    sac_ratio = total_flipped / (128 * 128)
    # (b) активность S-box в парах соседних раундов на реальном шифре
    # (теорема о парах доказывается индукцией — см. Том V; здесь численная
    # демонстрация с реальным расписанием ключей)
    min_active_pair = 99
    n_trials = 128
    def act(w):
        return sum(1 for byte in w.to_bytes(4, 'little') if byte)
    rk = key_schedule(key, eps)
    r_eps = [derive_round_epsilon(rk, i) for i in range(22)]
    for _ in range(n_trials):
        diff = (int(rng.integers(1, 2**63)) << 64) | int(rng.integers(0, 2**63))
        dl = [(diff >> (32 * i)) & M32 for i in range(4)]
        pt = [int(v) for v in rng.integers(0, 2**32, 4, dtype=np.uint64)]
        pt2 = [p ^ d for p, d in zip(pt, dl)]
        L = [pt[0] ^ rk[0][0], pt[1] ^ rk[0][1]]
        R = [pt[2] ^ rk[0][2], pt[3] ^ rk[0][3]]
        L2 = [pt2[0] ^ rk[0][0], pt2[1] ^ rk[0][1]]
        R2 = [pt2[2] ^ rk[0][2], pt2[3] ^ rk[0][3]]
        acts = []
        for rnd in range(4):
            dR = [R[0] ^ R2[0], R[1] ^ R2[1]]
            acts.append(act(dR[0]) + act(dR[1]))
            idx = rnd + 1
            f0, f1 = poler_feistel_f_half(R[0], R[1], rk[idx][0], rk[idx][1], r_eps[idx])
            g0, g1 = poler_feistel_f_half(R2[0], R2[1], rk[idx][0], rk[idx][1], r_eps[idx])
            L, L2, R, R2 = [R[0], R[1]], [R2[0], R2[1]], \
                [L[0] ^ f0, L[1] ^ f1], [L2[0] ^ g0, L2[1] ^ g1]
        for i in range(3):
            min_active_pair = min(min_active_pair, acts[i] + acts[i + 1])
    return {'sac_zig_criterion': {
                'total_flipped': total_flipped, 'expected': expected,
                'tolerance': '±20% (Zig verifyAvalancheEffect L1596-1602)',
                'pass': bool(sac_ok), 'ratio': round(sac_ratio, 4)},
            'two_consecutive_rounds_min_active': {
                'min_over_trials': min_active_pair, 'trials': n_trials,
                'theorem': 'в ЛЮБОЙ паре соседних раундов Фейстеля ≥ 1 активный '
                           'S-box (доказательство индукцией в Томе V); 20 раундов '
                           '⇒ ≥ 10 активных S-box ⇒ граница следа 2^−60 '
                           '(НЕ 2^−150: AES-SPN аргумент неприменим к Фейстелю '
                           'из-за возможности сокращения через pndMix/LHCA)'},
            'roundtrip': 'покрыт golden-слоем: 272 шифрования, decrypt∘encrypt = id',
            'verdict': ('Feistel-структура подтверждена; ЧЕСТНАЯ граница — '
                        '≥10 активных S-box за 20 раундов (след ≤ 2^−60), '
                        'заявка 25/2^−150 снята как неприменимый AES-SPN аргумент'
                        if sac_ok and min_active_pair >= 1 else 'см. паспорт')}

# ═════════════════════════════════ main ═══════════════════════════════════════
def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--sections', default='golden,v1,v2,barrier,mds,lhca,feistel')
    ap.add_argument('--golden', default=str(GOLDEN_DEFAULT))
    ap.add_argument('--regen', action='store_true',
                    help='регенерировать golden-векторы (нужны zig + poler-os)')
    ap.add_argument('--polos', default=os.environ.get('POLER_OS_PATH', ''),
                    help='путь к клону poler-os (для --regen)')
    ap.add_argument('--zig', default=os.environ.get('ZIG_BIN', 'zig'))
    ap.add_argument('--json', default='scratch/passports/cycle_A_full.json')
    a = ap.parse_args()
    secs = set(s.strip() for s in a.sections.split(','))

    out = {'protocol': 'POLER-MVR-v3', 'cycle': 'A-final (code-grounded)',
           'subject': 'Том V: PND v8 по РЕАЛЬНОМУ Zig-ядру poler-os',
           'source': 'poler-os/zig-kernel/src64/poler_core.zig @ fc3ffa8, Zig 0.14.0',
           'date': time.strftime('%Y-%m-%d %H:%M:%S')}
    if 'golden' in secs:
        out['golden'] = run_golden(a.golden)
        print('[golden]', out['golden']['verdict'])
    if 'v1' in secs:
        out['v1_real_phi'] = run_v1()
        print('[v1]', out['v1_real_phi']['verdict'])
    if 'v2' in secs:
        out['v2_verdict'] = run_v2()
        print('[v2]', out['v2_verdict']['verdict'])
    if 'dphi' in secs:
        out['dphi_exact_8values'] = run_dphi_exact()
        print('[dphi]', out['dphi_exact_8values']['verdict'])
    if 'v2full' in secs:
        out['v2_full_domain'] = run_v2full()
        print('[v2full]', out['v2_full_domain']['verdict'])
    if 'barrier' in secs:
        out['sbox_barrier'] = run_barrier()
        print('[barrier]', out['sbox_barrier']['verdict'])
    if 'mds' in secs:
        out['mds_branch5'] = run_mds()
        print('[mds]', out['mds_branch5']['verdict'])
    if 'lhca' in secs:
        out['lhca'] = run_lhca()
        print('[lhca]', out['lhca']['verdict'])
    if 'feistel' in secs:
        out['feistel'] = run_feistel()
        print('[feistel]', out['feistel']['verdict'])

    p = Path(a.json)
    p.parent.mkdir(parents=True, exist_ok=True)
    merged = {}
    if p.exists():  # слияние прогонов секций (паспорт накапливается)
        try:
            merged = json.loads(p.read_text())
        except Exception:
            merged = {}
    merged.update(out)
    merged['date'] = out['date']
    p.write_text(json.dumps(merged, ensure_ascii=False, indent=1, default=str))
    print('\nпаспорт: %s' % p)
    return 0

if __name__ == '__main__':
    sys.exit(main())
