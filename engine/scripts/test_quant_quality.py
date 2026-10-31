#!/usr/bin/env python3
"""
Численный тест деструктивности квантования на РЕАЛЬНЫХ весах ChatGLM3-6B.

Для каждой тестовой матрицы из локального шарда safetensors:
  1. Trit5  (1.6 бита/вес)  — нативный формат poler-engine
  2. Int4   (4+32 бита/строку) — контрольный формат
Считаем косинус W->W~, относительную ошибку Фробениуса и долю нулей.
Это объясняет ПОЧЕМУ генерация на Trit5-весах даёт мусор (или нет).
"""
import sys, os, struct, json
import numpy as np

PAGE = 4096

def parse_meta(fp):
    with open(fp, 'rb') as f:
        hl = struct.unpack('<Q', f.read(8))[0]
        meta = json.loads(f.read(hl).decode())
    return meta, 8 + hl

def load_tensor(fp, meta, base, name, row_limit=None):
    info = meta[name]
    off0, off1 = info['data_offsets']
    shape = info['shape']
    with open(fp, 'rb') as f:
        if row_limit and len(shape) == 2:
            rows = min(shape[0], row_limit)
            nbytes = (off1 - off0) * rows // shape[0]
            f.seek(base + off0)
            raw = f.read(nbytes)
            arr = np.frombuffer(raw, dtype=np.float16).reshape(rows, shape[1]).copy()
        else:
            f.seek(base + off0)
            raw = f.read(off1 - off0)
            arr = np.frombuffer(raw, dtype=np.float16).reshape(shape).copy()
    return arr

def dequant_int4(w):
    """Квантуем и разворачиваем назад — как движок (scale*(nib-8))."""
    rows, cols = w.shape
    scales = np.abs(w).max(axis=1).astype(np.float32) / 7.0
    scales[scales == 0] = 1.0
    q = np.clip(np.round(w.astype(np.float32) / scales[:, None]), -7, 7)
    return q * scales[:, None], scales

def dequant_trit5(w):
    rows, cols = w.shape
    scales = np.abs(w).max(axis=1).astype(np.float32)
    scales[scales == 0] = 1.0
    normed = w.astype(np.float32) / scales[:, None]
    t = np.zeros_like(normed, dtype=np.int8)
    t[normed > 0.33] = 1
    t[normed < -0.33] = -1
    return t.astype(np.float32) * scales[:, None], scales, t

def stats(w, wq):
    a = w.astype(np.float64).ravel()
    b = wq.astype(np.float64).ravel()
    dot = float(np.dot(a, b))
    na, nb = float(np.linalg.norm(a)), float(np.linalg.norm(b))
    cos = dot / (na * nb) if na > 0 and nb > 0 else 0.0
    rel = float(np.linalg.norm(a - b) / (na if na > 0 else 1))
    return cos, rel

def zeros_share(t):
    return float((t == 0).mean())

TESTS = [
    # (шард, имя тензора, метка, row_limit)
    ('model-00004-of-00007.safetensors',
     'transformer.encoder.layers.13.self_attention.query_key_value.weight', 'L13 qkv_w [4608x4096]', None),
    ('model-00004-of-00007.safetensors',
     'transformer.encoder.layers.13.self_attention.dense.weight', 'L13 out_proj_w [4096x4096]', None),
    ('model-00004-of-00007.safetensors',
     'transformer.encoder.layers.13.mlp.dense_h_to_4h.weight', 'L13 ffn_gate+up [27392x4096]', None),
    ('model-00004-of-00007.safetensors',
     'transformer.encoder.layers.13.mlp.dense_4h_to_h.weight', 'L13 ffn_down [4096x13696]', None),
    ('model-00006-of-00007.safetensors',
     'transformer.encoder.layers.23.self_attention.query_key_value.weight', 'L23 qkv_w [4608x4096]', None),
    ('model-00006-of-00007.safetensors',
     'transformer.encoder.layers.23.mlp.dense_h_to_4h.weight', 'L23 ffn_gate+up [27392x4096]', None),
]

def main():
    base = 'tmp_dl'
    print(f"{'матрица':32s} {'формат':6s} {'косинус':>8s} {'rel.err':>8s} {'%нулей':>7s}")
    print('-' * 66)
    results = {}
    for shard, tname, label, rlim in TESTS:
        path = os.path.join(base, shard)
        if not os.path.exists(path):
            print(f"{label:32s} НЕТ ШАРДА {shard}")
            continue
        meta, boff = parse_meta(path)
        if tname not in meta:
            print(f"{label:32s} нет тензора")
            continue
        w = load_tensor(path, meta, boff, tname, rlim)
        for mode in ('trit5', 'int4'):
            if mode == 'trit5':
                wq, _, t = dequant_trit5(w)
                z = zeros_share(t)
            else:
                wq, _ = dequant_int4(w)
                z = zeros_share(np.round(wq / (np.abs(w).max(axis=1, keepdims=True) / 7.0)))
                z = 0.0  # int4 почти не обнуляет веса; считать по q не нужно
            cos, rel = stats(w, wq)
            results[(label, mode)] = (cos, rel, z)
            print(f"{label:32s} {mode:6s} {cos:8.4f} {rel:8.4f} {z*100:6.1f}%")
            del wq
        del w
    # Итог
    print('-' * 66)
    for mode in ('trit5', 'int4'):
        cs = [v[0] for (l, m), v in results.items() if m == mode]
        if cs:
            print(f"СРЕДНИЙ косинус {mode:5s}: {np.mean(cs):.4f}  (мин {np.min(cs):.4f})")

if __name__ == '__main__':
    main()
