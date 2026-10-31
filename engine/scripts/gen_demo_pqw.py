#!/usr/bin/env python3
"""Генератор демонстрационных .pqw v2 моделей (полир Engine, Part E).

Независимая от Rust-реализации запись формата — заодно дифференциал
спецификации docs/PQW_FORMAT.md: если питон-писатель и Rust-читатель
согласны, формат описан однозначно.

Модели (синтетические веса, детерминированный PRNG):
  demo_encoder.pqw  — BERT-класс int8 (для --semantic dense)
  demo_glm.pqw      — GLM-декодер int8 dense (для --llm local)
  demo_gliner.pqw   — GLiNER span-голова (для --ner gliner)
"""
import hashlib
import os
import random
import struct
import sys

PAGE = 4096
HEADER = 128
MAGIC = b"PQW2NN\0\0"
VERSION = 2

# dtype
F32, I8, I4, RAW = 0, 1, 2, 3
# model_type
ENC, DEC, NER = 0, 1, 2
# quant
Q_F32, Q_I8, Q_I4 = 0, 1, 2

OUT = sys.argv[1] if len(sys.argv) > 1 else "/home/z/my-project/scripts"


class Rng(random.Random):
    """Тот же интерфейс, что SynthRng (только для синтетики)."""


def quant_i8_row(w):
    m = max(abs(v) for v in w) or 1.0
    sc = m / 127.0
    q = [max(-127, min(127, round(v / sc))) for v in w]
    return q, sc


class Model:
    def __init__(self, model_type, quant, layers, hidden, heads, intermediate,
                 vocab, max_pos, kv_heads=1, experts=0, top_k=0, xlmr=None):
        self.model_type = model_type
        self.quant = quant
        self.layers = layers
        self.hidden = hidden
        self.heads = heads
        self.intermediate = intermediate
        self.vocab = vocab
        self.max_pos = max_pos
        self.kv_heads = kv_heads
        self.experts = experts
        self.top_k = top_k
        self.xlmr = xlmr if xlmr is not None else model_type in (ENC, NER)
        self.tensors = []  # (name, dtype, dims, scales, data bytes)

    def add_f32(self, name, dims, values):
        data = b"".join(struct.pack("<f", v) for v in values)
        self.tensors.append((name, F32, dims, [], data))

    def add_i8(self, name, dims, values):
        rows, cols = dims
        codes, scales = [], []
        for r in range(rows):
            q, sc = quant_i8_row(values[r * cols:(r + 1) * cols])
            codes.extend(q)
            scales.append(sc)
        data = bytes((c & 0xFF) for c in codes)
        self.tensors.append((name, I8, dims, scales, data))

    def add_raw(self, name, text):
        self.tensors.append((name, RAW, [len(text)], [], text.encode("utf-8")))

    def write(self, path):
        buf = bytearray(HEADER)
        offsets = []
        for name, dtype, dims, scales, data in self.tensors:
            aligned = (len(buf) + PAGE - 1) // PAGE * PAGE
            buf.extend(b"\0" * (aligned - len(buf)))
            offsets.append(aligned)
            buf.extend(data)
        table_off = (len(buf) + PAGE - 1) // PAGE * PAGE
        buf.extend(b"\0" * (table_off - len(buf)))
        table_start = len(buf)
        for (name, dtype, dims, scales, data), off in zip(self.tensors, offsets):
            nb = name.encode("utf-8")
            buf.extend(struct.pack("<H", len(nb)))
            buf.extend(nb)
            buf.append(dtype)
            buf.append(len(dims))
            buf.extend(struct.pack("<H", 0))
            buf.extend(struct.pack("<I", len(scales)))
            for d in dims:
                buf.extend(struct.pack("<Q", d))
            buf.extend(struct.pack("<Q", off))
            buf.extend(struct.pack("<Q", len(data)))
            for s in scales:
                buf.extend(struct.pack("<f", s))
        table_len = len(buf) - table_start

        digest = hashlib.sha256(bytes(buf[PAGE:])).digest()

        # Заголовок заполняем по полям (см. docs/PQW_FORMAT.md).
        buf = bytearray(HEADER) + bytearray(buf[HEADER:])
        buf[0:8] = MAGIC
        struct.pack_into("<I", buf, 8, VERSION)
        struct.pack_into("<I", buf, 12, HEADER)
        buf[16] = self.model_type
        buf[17] = self.quant
        flags = 0
        if any(n.endswith("_b") for n, *_ in self.tensors):
            flags |= 1
        if self.xlmr:
            flags |= 2
        if self.experts > 0:
            flags |= 4
        struct.pack_into("<H", buf, 18, flags)
        struct.pack_into("<I", buf, 20, self.layers)
        struct.pack_into("<I", buf, 24, self.hidden)
        struct.pack_into("<I", buf, 28, self.intermediate)
        struct.pack_into("<I", buf, 32, self.heads)
        struct.pack_into("<I", buf, 36, self.hidden // self.heads)
        struct.pack_into("<I", buf, 40, self.vocab)
        struct.pack_into("<I", buf, 44, self.max_pos)
        struct.pack_into("<I", buf, 48, self.experts)
        struct.pack_into("<I", buf, 52, self.top_k)
        struct.pack_into("<I", buf, 56, self.kv_heads)
        struct.pack_into("<I", buf, 60, 0)
        struct.pack_into("<Q", buf, 64, table_off)
        struct.pack_into("<Q", buf, 72, table_len)
        struct.pack_into("<Q", buf, 80, table_off - PAGE)
        struct.pack_into("<Q", buf, 88, len(buf))
        buf[96:128] = digest

        with open(path, "wb") as f:
            f.write(bytes(buf))
        print(f"{path}: {len(buf)} байт, тензоров {len(self.tensors)}")


def synth_encoder(seed, layers, hidden, heads, intermediate, vocab, max_pos):
    rng = Rng(seed)

    def w(rows, cols, lo=-0.2, hi=0.2):
        return [rng.uniform(lo, hi) for _ in range(rows * cols)]

    m = Model(ENC, Q_I8, layers, hidden, heads, intermediate, vocab, max_pos)
    m.add_i8("word_embeddings", [vocab, hidden], w(vocab, hidden))
    m.add_i8("position_embeddings", [max_pos, hidden], w(max_pos, hidden))
    m.add_i8("token_type_embeddings", [2, hidden], w(2, hidden))
    m.add_f32("embeddings_ln_gamma", [hidden], [rng.uniform(0.9, 1.1) for _ in range(hidden)])
    m.add_f32("embeddings_ln_beta", [hidden], [rng.uniform(-0.05, 0.05) for _ in range(hidden)])
    for i in range(layers):
        p = f"layers.{i}."
        for x in ("q", "k", "v", "o"):
            m.add_i8(p + f"attn_{x}_w", [hidden, hidden], w(hidden, hidden))
            m.add_f32(p + f"attn_{x}_b", [hidden], [rng.uniform(-0.05, 0.05) for _ in range(hidden)])
        m.add_f32(p + "attn_ln_gamma", [hidden], [rng.uniform(0.9, 1.1) for _ in range(hidden)])
        m.add_f32(p + "attn_ln_beta", [hidden], [rng.uniform(-0.05, 0.05) for _ in range(hidden)])
        m.add_i8(p + "ffn_up_w", [intermediate, hidden], w(intermediate, hidden))
        m.add_f32(p + "ffn_up_b", [intermediate], [rng.uniform(-0.05, 0.05) for _ in range(intermediate)])
        m.add_i8(p + "ffn_down_w", [hidden, intermediate], w(hidden, intermediate))
        m.add_f32(p + "ffn_down_b", [hidden], [rng.uniform(-0.05, 0.05) for _ in range(hidden)])
        m.add_f32(p + "ffn_ln_gamma", [hidden], [rng.uniform(0.9, 1.1) for _ in range(hidden)])
        m.add_f32(p + "ffn_ln_beta", [hidden], [rng.uniform(-0.05, 0.05) for _ in range(hidden)])
    m.add_f32("final_ln_gamma", [hidden], [rng.uniform(0.9, 1.1) for _ in range(hidden)])
    m.add_f32("final_ln_beta", [hidden], [rng.uniform(-0.05, 0.05) for _ in range(hidden)])
    return m


def synth_glm(seed, layers, hidden, heads, kv_heads, intermediate, vocab, max_pos):
    rng = Rng(seed)
    hd = hidden // heads
    q_dim, kv_dim = heads * hd, kv_heads * hd

    def w(rows, cols, lo=-0.15, hi=0.15):
        return [rng.uniform(lo, hi) for _ in range(rows * cols)]

    m = Model(DEC, Q_I8, layers, hidden, heads, intermediate, vocab, max_pos,
              kv_heads=kv_heads)
    m.add_i8("word_embeddings", [vocab, hidden], w(vocab, hidden))
    for i in range(layers):
        p = f"layers.{i}."
        m.add_i8(p + "attn_q_w", [q_dim, hidden], w(q_dim, hidden))
        m.add_i8(p + "attn_k_w", [kv_dim, hidden], w(kv_dim, hidden))
        m.add_i8(p + "attn_v_w", [kv_dim, hidden], w(kv_dim, hidden))
        m.add_i8(p + "attn_o_w", [hidden, q_dim], w(hidden, q_dim))
        m.add_f32(p + "attn_norm_gamma", [hidden], [rng.uniform(0.9, 1.1) for _ in range(hidden)])
        m.add_f32(p + "ffn_norm_gamma", [hidden], [rng.uniform(0.9, 1.1) for _ in range(hidden)])
        m.add_i8(p + "ffn_gate_w", [intermediate, hidden], w(intermediate, hidden))
        m.add_i8(p + "ffn_up_w", [intermediate, hidden], w(intermediate, hidden))
        m.add_i8(p + "ffn_down_w", [hidden, intermediate], w(hidden, intermediate))
    m.add_f32("final_norm_gamma", [hidden], [rng.uniform(0.9, 1.1) for _ in range(hidden)])
    m.add_i8("lm_head_w", [vocab, hidden], w(vocab, hidden))
    return m


def synth_gliner(seed, layers, hidden, heads, intermediate, vocab, max_pos, labels):
    m = synth_encoder(seed, layers, hidden, heads, intermediate, vocab, max_pos)
    m.model_type = NER
    rng = Rng(seed + 12345)
    max_width = 16
    n_labels = len(labels)
    w1 = [rng.uniform(-0.2, 0.2) for _ in range(max_width * hidden)]
    w2 = [rng.uniform(-0.15, 0.15) for _ in range(n_labels * 3 * hidden)]
    m.add_i8("span_width_emb", [max_width, hidden], w1)
    m.add_i8("span_proj_w", [n_labels, 3 * hidden], w2)
    m.add_raw("__labels__", "\n".join(labels))
    return m


os.makedirs(OUT, exist_ok=True)
synth_encoder(42, 2, 64, 4, 128, 96, 64).write(os.path.join(OUT, "demo_encoder.pqw"))
synth_glm(9, 2, 64, 4, 1, 96, 128, 512).write(os.path.join(OUT, "demo_glm.pqw"))
synth_gliner(7, 1, 32, 4, 64, 64, 64, ["PERSON", "LOCATION", "OBJECT"]).write(
    os.path.join(OUT, "demo_gliner.pqw"))
print("OK")
