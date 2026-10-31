#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Сессия 7 — «ПАМЯТЬ ВЫБИРАЕТ НЕКОМПАКТНОСТЬ»: скан кристалла t5c.

Идея (пользователь): «прокормить фазы Ацина тритами кристалла — пусть
некомпактность γ выбирает сама память».

Механика: permanent_memory.t5c — биграммная тритная матрица памяти
(8198×8198 тритов, 67.2 млн). Каждое окно из 6 подряд идущих тритов —
кандидат γ = n/729 (n = сбалансированные триты как база-3 цифры).
Семейство Ацина (1,γ,1)/√(2+γ²) даёт (движок, сессия 6-7):

    I₃(γ) = 4(2√3·γ + 3) / (3(2 + γ²)),  γ* = (√11−√3)/2 ≈ 0.7922870.

Память «выбирает» argmax I₃ по СОБСТВЕННЫМ окнам. Скан побайтово-строчно
(8198 строк), bincount по 729 значениям окон. Выход: распределение γ,
выбранный γ_mem, где он живёт (какие строки-токены), счётчик.
"""
import struct
import sys
import numpy as np

PATH = sys.argv[1] if len(sys.argv) > 1 else \
    '/home/z/Стільниця/poler-engine/permanent_memory.t5c'

data = open(PATH, 'rb').read()
magic = data[:8].decode('ascii', 'replace')
V = struct.unpack_from('<I', data, 12)[0]
H = struct.unpack_from('<I', data, 16)[0]
token_off = struct.unpack_from('<I', data, 20)[0]
bigram_off = struct.unpack_from('<I', data, 24)[0]
row_w = (V + 4) // 5
print(f"кристалл: {PATH}")
print(f"  magic={magic!r} V={V} H={H} token_off={token_off} bigram_off={bigram_off}")
print(f"  размер={len(data)} байт; строка биграмм={row_w} байт")

# токены (для семантики: где живёт выбранный γ)
pos = token_off
tokens = []
for _ in range(V):
    ln = data[pos]; pos += 1
    tokens.append(data[pos:pos + ln].decode('utf-8', 'replace')); pos += ln
print(f"  токенов прочитано: {len(tokens)}; примеры: {tokens[:8]}")

# скан: скользящие 6-тритовые окна по ВСЕЙ матрице, побайтово
# КОНВЕНЦИЯ: окно читается в kron-порядке (как мост сессии 4: первый трит —
# старший разряд) => свёртка с обращённым ядром: первый трит × 243.
POW = np.array([1, 3, 9, 27, 81, 243], dtype=np.int32)
counts = np.zeros(729, dtype=np.int64)
row_best = {}   # n -> список (row, offset) лучших вхождений (для топ-n)
t0 = None
total_windows = 0
nonzero_rows = 0
for r in range(V):
    rb = np.frombuffer(data[bigram_off + r * row_w:
                            bigram_off + (r + 1) * row_w], dtype=np.uint8)
    v = np.minimum(rb.astype(np.int32), 243)
    # 5 тритов на байт, little-endian по разрядам (как в сессии 4)
    trits = np.empty(len(v) * 5, dtype=np.int8)
    vv = v.copy()
    for i in range(5):
        trits[i::5] = (vv % 3) - 1
        vv //= 3
    d = (trits.astype(np.int32) + 1)  # цифры 0..2
    if d.any():
        nonzero_rows += 1
    # окна: n = Σ d_{k+j} * 3^(5-j) — свёртка
    if len(d) >= 6:
        n = np.convolve(d, POW, 'valid')  # int32
        counts += np.bincount(n, minlength=729)
        total_windows += len(n)

print(f"\nскан: строк с ненулевыми тритами: {nonzero_rows}/{V}")
print(f"всего 6-тритовых окон: {total_windows:,}")

# γ → I₃(γ) — движковая аналитика семейства Ацина
def I3(g):
    return 4.0 * (2.0 * np.sqrt(3) * g + 3.0) / (3.0 * (2.0 + g * g))

gstar = (np.sqrt(11) - np.sqrt(3)) / 2
print(f"γ* (Ацин) = {gstar:.10f};  I₃(γ*) = {I3(gstar):.10f} (= 1+√(11/3) = {1+np.sqrt(11/3):.10f})")

present = counts > 0
print(f"различных значений окон присутствует: {present.sum()}/729")
gammas = np.arange(729) / 729.0
I3s = I3(gammas)
# выбор памяти: argmax I₃ среди ПРИСУТСТВУЮЩИХ значений
masked = np.where(present, I3s, -1.0)
n_star = int(np.argmax(masked))
g_mem = n_star / 729.0
print(f"\nВЫБОР ПАМЯТИ: n* = {n_star} (= {n_star} в базе 3, старший-первый: "
      f"{''.join(str((n_star // 3**j) % 3) for j in range(5, -1, -1))}₃)")
print(f"  триты окна (kron-порядок): "
      f"{[( (n_star // 3**j) % 3) - 1 for j in range(5, -1, -1)]}")
print(f"  γ_mem = {g_mem:.10f}  (Δ от γ* = {abs(g_mem - gstar):.2e})")
print(f"  I₃(γ_mem) = {I3s[n_star]:.10f}  ({100*I3s[n_star]/I3(gstar):.6f}% оптимума)")
print(f"  окон с этим значением в кристалле: {counts[n_star]:,}")

# топ-5 присутствующих значений по I₃
top = np.argsort(masked)[::-1][:5]
print("\nтоп-5 значений памяти по I₃:")
for n in top:
    if counts[n] == 0:
        continue
    print(f"  n={n:3d}  γ={n/729:.6f}  I₃={I3s[n]:.9f}  окон: {counts[n]:,}")

# ГДЕ живёт выбранный n* — какие строки-токены
if counts[n_star] > 0:
    print(f"\nгде живёт n*={n_star}: сканирую строки заново (до 200 вхождений)…")
    hits = []
    for r in range(V):
        rb = np.frombuffer(data[bigram_off + r * row_w:
                                bigram_off + (r + 1) * row_w], dtype=np.uint8)
        v = np.minimum(rb.astype(np.int32), 243)
        d = np.empty(len(v) * 5, dtype=np.int32)
        vv = v.copy()
        for i in range(5):
            d[i::5] = (vv % 3)
            vv //= 3
        if len(d) < 6:
            continue
        n = np.convolve(d, POW, 'valid')
        w = np.nonzero(n == n_star)[0]
        for off in w[:3]:  # до 3 на строку
            # триты окна (сбалансированные) для моста
            td = d[off:off + 6]
            trits = (td - 1).tolist()
            hits.append((r, tokens[r] if r < len(tokens) else '?', int(off), trits))
        if len(hits) >= 200:
            break
    print(f"  найдено вхождений (обрезано 200): {len(hits)}")
    tok_count = {}
    for r, tok, off, trits in hits:
        tok_count[tok] = tok_count.get(tok, 0) + 1
    print(f"  токены-строки, где живёт оптимум (топ-10):")
    for tok, c in sorted(tok_count.items(), key=lambda kv: -kv[1])[:10]:
        print(f"    {tok!r}: {c}")
    print(f"  примеры окон (триты): {[h[3] for h in hits[:4]]}")

# распределение γ по кристаллу
dens = counts / counts.sum()
g_mean = float((gammas * counts).sum() / counts.sum())
print(f"\nстатистика γ по окнам: среднее = {g_mean:.4f}; "
      f"медианное n = {int(np.argmax(np.cumsum(counts) >= counts.sum()/2))}")
print(f"  плотность на n*=578 (теор. оптимум округления): {counts[578]:,} окон")
if counts[578] > 0:
    print(f"  → округлённый оптимум Ацина 578/729 ПРИСУТСТВУЕТ в памяти ({counts[578]:,} раз)")

np.save('/home/z/my-project/scripts/session7/crystal_scan.npy',
        {'counts': counts, 'n_star': n_star, 'V': V})
print("\nсохранено: crystal_scan.npy")
