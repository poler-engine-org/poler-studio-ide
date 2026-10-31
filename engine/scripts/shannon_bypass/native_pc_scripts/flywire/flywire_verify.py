#!/usr/bin/env python3
"""Верификация агрегированного CSR FlyWire v783.

Проверяет:
V1. Структура артефактов (заголовки, монотонность offsets, сортировка targets)
V2. Узлы (nodes.bin == 138 639)
V3. Сохранность массы и корректность распаковки графа
V4. Ядро (все веса >= 5, масса full == core + слабые)
V5. E/I-баланс (биологический баланс 80/20)
"""
import os
from pathlib import Path
import struct
import numpy as np
import zstandard as zstd

# scripts/shannon_bypass/native_pc_scripts/flywire/flywire_verify.py
# → repo root: flywire/ → native_pc_scripts/ → shannon_bypass/ → scripts/ → poler-engine/
REPO_ROOT = Path(__file__).resolve().parents[4]
RAW = os.environ.get("FLYWIRE_RAW", str(REPO_ROOT / "docs" / "flywire-connectome" / "raw"))
OUT = os.environ.get("FLYWIRE_OUT", str(REPO_ROOT / "docs" / "flywire-connectome"))
NT = ["gaba", "ach", "glut", "oct", "ser", "da"]
results = []


def ok(name, cond):
    results.append(bool(cond))
    print(f"  [{'OK' if cond else 'FAIL'}] {name}")


def load_csr(fname):
    blob = zstd.ZstdDecompressor().decompress(open(f"{OUT}/{fname}", "rb").read(), max_output_size=1 << 29)
    assert blob[:7] == b"FLYCSR1", "магия"
    core_flag = blob[7]
    n_nodes, n_edges = struct.unpack_from("<II", blob, 8)
    off = 16
    offsets = np.frombuffer(blob, dtype="<u4", count=n_nodes + 1, offset=off)
    off += (n_nodes + 1) * 4
    targets = np.frombuffer(blob, dtype="<u4", count=n_edges, offset=off)
    off += n_edges * 4
    weights = np.frombuffer(blob, dtype="<u2", count=n_edges, offset=off)
    off += n_edges * 2
    nt_class = np.frombuffer(blob, dtype="u1", count=n_edges, offset=off)
    return core_flag, n_nodes, n_edges, offsets, targets, weights, nt_class


print("V1: структура артефактов")
cf, nn, ne, offF, tF, wF, ntF = load_csr("flywire_v783_full.csr.zst")
ok(f"full: core_flag=0, n_nodes={nn}, n_edges={ne}", cf == 0)
ok("offsets монотонны, last==n_edges", bool(np.all(np.diff(offF) >= 0)) and offF[-1] == ne)
samp_rows = [range(offF[i], offF[i + 1]) for i in np.linspace(0, nn - 1, 400, dtype=int)]
ok("targets отсортированы внутри строк", all(np.all(np.diff(tF[r]) >= 0) for r in samp_rows if len(r) > 1))

print("V2: узлы")
nodes = np.fromfile(f"{OUT}/flywire_v783_nodes.bin", dtype="<i8")
ok(f"nodes.bin == n_nodes ({len(nodes)})", len(nodes) == nn)

raw_npy = Path(f"{RAW}/proofread_root_ids_783.npy")
if raw_npy.exists():
    ids = np.load(raw_npy).astype(np.int64)
    ok(f"nodes ⊆ proofread_ids; вне графа {len(ids) - len(np.intersect1d(nodes, ids))} изолированных",
       len(np.intersect1d(nodes, ids)) == len(nodes))

print("V3: масса синапсов")
tot_syn = int(wF.astype(np.uint64).sum())
ok(f"полная масса синапсов: {tot_syn} == 54 492 922", tot_syn == 54_492_922)

print("V4: ядро")
cc, nnc, nec, offC, tC, wC, ntC = load_csr("flywire_v783_core.csr.zst")
ok(f"core: flag=1, n_nodes={nnc}, n_edges={nec}", cc == 1 and nnc == nn)
ok("все веса core >= 5", bool(np.all(wC >= 5)))
ok("масса: full == core + слабые",
   tot_syn == int(wC.astype(np.uint64).sum()) + int(wF[wF < 5].astype(np.uint64).sum()))

print("V5: E/I-баланс")
w64 = wF.astype(np.uint64)
exc = int(w64[(ntF == 1) | (ntF == 2)].sum())
inh = int(w64[ntF == 0].sum())
mod = int(w64[(ntF == 3) | (ntF == 4) | (ntF == 5)].sum())
tot = int(w64.sum())
print(f"  возб: {exc/1e6:.1f}M ({100*exc/tot:.1f}%) | торм: {inh/1e6:.1f}M ({100*inh/tot:.1f}%) | модул: {mod/1e6:.1f}M ({100*mod/tot:.1f}%)")
ok("E:I в диапазоне 3–6 (≈80:20)", 3.0 < exc / inh < 6.0)

print()
print("ИТОГ:", "ВСЕ ПРОВЕРКИ ПРОЙДЕНЫ" if all(results) else "ЕСТЬ НЕУДАЧИ")
raise SystemExit(0 if all(results) else 1)
