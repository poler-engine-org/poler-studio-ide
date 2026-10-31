#!/usr/bin/env python3
"""FlyWire v783 → CSR-ядро POLER, v3: потоковая агреграция (low-mem).

Агреграция дубликатов пар (разные нейропили):
    weight   = Σ syn_count
    nt_class = argmax_c Σ score_c · syn_count   (стриминг по классам)

Пиковая anon-память ~700 МБ (было >2.5 ГБ в v2).
"""
import json
import hashlib
import os
import struct
import numpy as np
import pyarrow.feather as feather
import zstandard as zstd

from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
RAW = os.environ.get("FLYWIRE_RAW", str(REPO_ROOT / "docs" / "flywire-connectome" / "raw"))
OUT = os.environ.get("FLYWIRE_OUT", str(REPO_ROOT / "docs" / "flywire-connectome"))
CORE_MIN = 5
CHUNK = 2_000_000
NT_NAMES = ["gaba", "ach", "glut", "oct", "ser", "da"]
MAGIC = b"FLYCSR1"


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for blk in iter(lambda: f.read(1 << 20), b""):
            h.update(blk)
    return h.hexdigest()


def main():
    src = f"{RAW}/proofread_connections_783.feather"
    print("[1/7] mmap исходной таблицы…")
    t = feather.read_table(src, memory_map=True)
    n = t.num_rows
    print(f"      строк (pre,post,neuropil): {n}")

    print("[2/7] уникальные нейроны + композитные ключи пар (chunked)…")
    uniq_parts, key_parts = [], []
    for s in range(0, n, CHUNK):
        e = min(s + CHUNK, n)
        pre = t.column("pre_pt_root_id")[s:e].to_numpy(zero_copy_only=False)
        post = t.column("post_pt_root_id")[s:e].to_numpy(zero_copy_only=False)
        uniq_parts.append(np.unique(np.concatenate([np.unique(pre), np.unique(post)])))
        key_parts.append((pre, post))
        del pre, post
    nodes = np.unique(np.concatenate(uniq_parts))
    del uniq_parts
    n_nodes = len(nodes)
    print(f"      узлов: {n_nodes}")
    for i, (pre, post) in enumerate(key_parts):
        pi = np.searchsorted(nodes, pre).astype(np.uint64)
        qi = np.searchsorted(nodes, post).astype(np.uint64)
        key_parts[i] = (pi << np.uint64(32)) | qi
    key = np.concatenate(key_parts)
    del key_parts

    print("[3/7] np.unique пар…")
    ukey, inv = np.unique(key, return_inverse=True)
    del key
    n_edges = len(ukey)
    print(f"      уникальных пар: {n_edges}")

    def chunk_ranges():
        return [(s, min(s + CHUNK, n)) for s in range(0, n, CHUNK)]

    print("[4/7] weight = Σ syn (стриминговый bincount)…")
    syn_sum = np.zeros(n_edges, dtype=np.float64)
    for s, e in chunk_ranges():
        syn = t.column("syn_count")[s:e].to_numpy(zero_copy_only=False).astype(np.float64)
        syn_sum += np.bincount(inv[s:e], weights=syn, minlength=n_edges)
        del syn

    print("[5/7] nt_class: потоковое голосование по 6 классам…")
    best = np.full(n_edges, -np.inf, dtype=np.float64)
    best_c = np.zeros(n_edges, dtype=np.uint8)
    for j, name in enumerate(NT_NAMES):
        acc = np.zeros(n_edges, dtype=np.float64)
        for s, e in chunk_ranges():
            sc = t.column(f"{name}_avg")[s:e].to_numpy(zero_copy_only=False).astype(np.float64)
            syn = t.column("syn_count")[s:e].to_numpy(zero_copy_only=False).astype(np.float64)
            sc *= syn
            acc += np.bincount(inv[s:e], weights=sc, minlength=n_edges)
            del sc, syn
        upd = acc > best
        best[upd] = acc[upd]
        best_c[upd] = j
        del acc, upd
        print(f"      {name}: {int((best_c == j).sum())} рёбер ведёт класс")
    del best, inv

    print("[6/7] CSR + запись артефактов…")
    pre_idx = (ukey >> np.uint64(32)).astype(np.uint32)
    post_idx = (ukey & np.uint64(0xFFFFFFFF)).astype(np.uint32)
    del ukey
    weights = np.minimum(syn_sum, 65535).astype(np.uint16)
    sat = int((syn_sum > 65535).sum())
    total_syn = int(syn_sum.sum())
    del syn_sum
    offsets = np.zeros(n_nodes + 1, dtype=np.uint32)
    offsets[1:] = np.cumsum(np.bincount(pre_idx, minlength=n_nodes).astype(np.uint64))
    assert offsets[-1] == n_edges
    nt_edge_counts = {name: int((best_c == j).sum()) for j, name in enumerate(NT_NAMES)}

    def write(is_core, fname):
        if is_core:
            keep = weights >= CORE_MIN
            cnt = np.bincount(pre_idx[keep], minlength=n_nodes).astype(np.uint64)
            o = np.zeros(n_nodes + 1, dtype=np.uint32)
            o[1:] = np.cumsum(cnt)
            n_e = int(keep.sum())
            payload = b"".join([MAGIC, bytes([1]), struct.pack("<II", n_nodes, n_e),
                                o.tobytes(), post_idx[keep].tobytes(),
                                weights[keep].tobytes(), best_c[keep].tobytes()])
        else:
            payload = b"".join([MAGIC, bytes([0]), struct.pack("<II", n_nodes, n_edges),
                                offsets.tobytes(), post_idx.tobytes(),
                                weights.tobytes(), best_c.tobytes()])
        blob = zstd.ZstdCompressor(level=19).compress(payload)
        with open(f"{OUT}/{fname}", "wb") as f:
            f.write(blob)

    write(False, "flywire_v783_full.csr.zst")
    core_mask = weights >= CORE_MIN
    n_core = int(core_mask.sum())
    syn_core = int(weights[core_mask].astype(np.uint64).sum())
    write(True, "flywire_v783_core.csr.zst")
    nodes.astype("<u8").tofile(f"{OUT}/flywire_v783_nodes.bin")

    print("[7/7] мета…")
    full_size = os.path.getsize(f"{OUT}/flywire_v783_full.csr.zst")
    core_size = os.path.getsize(f"{OUT}/flywire_v783_core.csr.zst")
    meta = {
        "title": "FlyWire Whole-brain Connectome v783 — компактное CSR-ядро POLER",
        "source": {
            "zenodo_record": 10676866,
            "doi": "10.5281/zenodo.10676866",
            "release": "v783 (взрослый женский мозг Drosophila melanogaster)",
            "paper": "Dorkenwald et al., Nature 634 (2024): Neuronal wiring diagram of an adult brain",
            "nt_paper": "Eckstein et al. 2024 — синаптические предсказания медиаторов",
            "files": {
                "proofread_connections_783.feather": {"bytes": 852022274, "sha256": sha256_file(src)},
                "proofread_root_ids_783.npy": {"bytes": 1114168},
                "per_neuron_neuropil_count_pre_783.feather": {"bytes": 16853770},
                "per_neuron_neuropil_count_post_783.feather": {"bytes": 233843050},
                "flywire_synapses_783.feather": {"bytes": 9493000000, "note": "полная синаптическая таблица (9.5 ГБ) вне ядра; тот же URL-шаблон Zenodo"},
            },
            "raw_local": "docs/flywire-connectome/raw/ (вне git; SHA-256 для воспроизведения)",
        },
        "aggregation": {
            "source_rows": int(n),
            "unique_pairs": int(n_edges),
            "rule": "пара (pre,post) из нескольких нейропилей склеивается: weight = Σ syn_count; nt_class = argmax_c Σ(score_c·syn_count)",
            "u16_saturation_edges": sat,
        },
        "graph": {
            "nodes": int(n_nodes),
            "edges_full": int(n_edges),
            "synapses_full": total_syn,
            "edges_core_ge5": n_core,
            "synapses_core_ge5": syn_core,
            "core_min_synapses": CORE_MIN,
            "isolated_proofread": int(len(np.load(f"{RAW}/proofread_root_ids_783.npy")) - n_nodes),
        },
        "neurotransmitter_edges_full": nt_edge_counts,
        "sign_convention": {
            "+1 возбуждающий": ["ach (ацетилхолин)", "glut (глутамат)"],
            "-1 тормозной": ["gaba"],
            "модуляторные": ["oct (октопамин)", "ser (серотонин)", "da (допамин)"],
        },
        "format": {
            "container": "zstd level=19 поверх сырого little-endian буфера",
            "layout": "MAGIC b'FLYCSR1' + u8 core_flag + u32 n_nodes + u32 n_edges + offsets u32[n_nodes+1] + targets u32[n_edges] + weights u16[n_edges] + nt_class u8[n_edges]",
            "nt_class_map": {str(i): nm for i, nm in enumerate(NT_NAMES)},
            "nodes_file": "flywire_v783_nodes.bin — отсортированные u64 LE root_id; targets — индексы в этом массиве",
        },
        "artifacts": {
            "flywire_v783_full.csr.zst": {"bytes": full_size},
            "flywire_v783_core.csr.zst": {"bytes": core_size},
            "flywire_v783_nodes.bin": {"bytes": int(n_nodes) * 8},
        },
        "male_cns_note": "мужской целый CNS раздаётся через codex.flywire.ai → Downloads (бесплатный Google-логин); формат feather/parquet тот же — конвертер совместим.",
    }
    with open(f"{OUT}/flywire_v783_meta.json", "w", encoding="utf-8") as f:
        json.dump(meta, f, ensure_ascii=False, indent=2)
    print(json.dumps({"graph": meta["graph"], "aggregation": meta["aggregation"]}, ensure_ascii=False, indent=1))
    print(f"РАЗМЕРЫ: full = {full_size/1e6:.1f} MB | core = {core_size/1e6:.1f} MB")


if __name__ == "__main__":
    main()
