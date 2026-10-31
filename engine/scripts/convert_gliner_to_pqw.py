#!/usr/bin/env python3
"""Конвертер GLiNER (mdeberta-чекпойнты) → .pqw v2.

Принимает чекпойнт класса urchade/gliner_multi (torch-zip pytorch_model.bin
+ spm.model), пишет .pqw model_type=Gliner (флаг deberta):

  спина  DeBERTa-v2 (12 слоёв, disentangled attention, int8 weight-only)
  голова BiLSTM + SpanMarker + prompt-проекция
  секции __tokenizer__ v2 (unigram + профиль нормализации mdeberta)
         __gliner__ (max_width, ent_id, sep_id, flert_id)

Только stdlib + numpy (как convert_hf_to_pqw.py): spm-прото разбирается
вручную, torch-zip — стабами. Спец-токены gliner: [FLERT]/<<ENT>>/<<SEP>>
= ids 250102..250104 поверх base-словаря 250102 (spm 250101 + '▁').

Пример:
  python3 convert_gliner_to_pqw.py \
      --hf-dir ~/.cache/huggingface/hub/models--urchade--gliner_multi \
      --out models/gliner_multi.pqw --quant int8
"""
import argparse
import io
import json
import os
import struct
import sys
import zipfile

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from convert_hf_to_pqw import PqwWriter, load_tensor_meta, quant_rows_iter  # noqa: E402

# dtype-коды .pqw
F32, I8, I4, RAW = 0, 1, 2, 3
# model_type
GLINER = 3
# флаги: bias | deberta-rel-attn
FLAGS = 1 | 8

TORCH_DTYPES = {"float": ("<f4", 4)}


# ---------------------------------------------------------------------------
# spm.model → куски + счёты (мини-парсер sentencepiece-прото, stdlib)
# ---------------------------------------------------------------------------

def parse_spm(path):
    """[(piece, score, type)], unk_id. Прото: repeated SentencePiece pieces=1
    { string piece=1; float score=2; enum type=3 (1..6) }."""
    data = open(path, "rb").read()

    def varint(buf, i):
        v = 0
        shift = 0
        while True:
            b = buf[i]
            i += 1
            v |= (b & 0x7F) << shift
            if not b & 0x80:
                return v, i
            shift += 7

    pieces = []
    i = 0
    while i < len(data):
        tag, i = varint(data, i)
        field = tag >> 3
        wt = tag & 7
        if field == 1 and wt == 2:  # pieces
            ln, i = varint(data, i)
            sub = data[i : i + ln]
            i += ln
            piece = b""
            score = 0.0
            ptype = 1
            j = 0
            while j < len(sub):
                t2, j = varint(sub, j)
                f2, w2 = t2 >> 3, t2 & 7
                if f2 == 1 and w2 == 2:
                    l2, j = varint(sub, j)
                    piece = sub[j : j + l2]
                    j += l2
                elif f2 == 2 and w2 == 5:  # fixed32
                    score = struct.unpack("<f", sub[j : j + 4])[0]
                    j += 4
                elif f2 == 3 and w2 == 0:  # varint
                    ptype, j = varint(sub, j)
                elif w2 == 0:
                    _, j = varint(sub, j)
                elif w2 == 5:
                    j += 4
                elif w2 == 1:
                    l3, j = varint(sub, j)
                    j += l3
                else:
                    raise SystemExit(f"spm: неожиданный wire-тип {w2} (поле {f2})")
            pieces.append((piece, score, ptype))
        elif wt == 0:
            _, i = varint(data, i)
        elif wt == 5:
            i += 4
        elif wt == 1:
            ln, i = varint(data, i)
            i += ln
        elif wt == 2:
            ln, i = varint(data, i)
            i += ln
        else:
            raise SystemExit(f"spm: мусорный тег {tag} на {i}")
    unk = next((n for n, (p, _, t) in enumerate(pieces) if t == 2), 3)
    return pieces, unk


def build_tokenizer_section(spm_path):
    """Секция __tokenizer__ v2: unigram + профиль нормализации mdeberta.

    ВАЖНО: '▁' уже есть в spm-словаре как обычный кусок (id 260) со своим
    счётом — НЕ добавлять дубликат в vocab (score 0.0 перехватит Viterbi!).
    Добавленный HF-токен '▁' (id 250101) — raw-match на литеральный '▁'
    в тексте → попадает в specials, как в HF added_tokens."""
    pieces, unk_id = parse_spm(spm_path)
    base = [(p, s) for (p, s, _t) in pieces]  # 250101 кусок, '▁' = id 260
    added_replacement = len(pieces)  # 250101 — raw-match '▁'
    flert = added_replacement + 1  # 250102
    ent = added_replacement + 2  # 250103
    sep = added_replacement + 3  # 250104

    out = bytearray()
    out += b"TOKR"
    out += struct.pack("<H", 2)  # v2
    out.append(0)  # unigram
    out.append(1)  # add_prefix_space (metaspace prepend_scheme=always)
    for i in (unk_id, 1, 2, 0, 0xFFFFFFFF):  # unk bos eos pad mask
        out += struct.pack("<I", int(i))
    out += struct.pack("<I", len(base))
    for p, s in base:
        out += struct.pack("<H", len(p))
        out += p
        out += struct.pack("<f", float(s))
    specials = [
        (b"<<SEP>>", sep),
        (b"<<ENT>>", ent),
        (b"[FLERT]", flert),
        (b"\xe2\x96\x81", added_replacement),  # '▁' raw-match (HF added)
    ]
    out += struct.pack("<I", len(specials))
    for p, sid in specials:
        out += struct.pack("<H", len(p))
        out += p
        out += struct.pack("<I", int(sid))
    out.append(1)  # norm_profile=1: NFC + strip_right (mdeberta)
    return bytes(out), len(base), {"flert": flert, "ent": ent, "sep": sep}


# ---------------------------------------------------------------------------
# План конвертации
# ---------------------------------------------------------------------------

T = "token_rep_layer.bert_layer.model."


def gliner_plan(tensors, n_layers):
    """[(pqw_name, torch_name, quantized?)] — спина + голова."""
    plan = [
        ("word_embeddings", T + "embeddings.word_embeddings.weight", True),
        ("embeddings_ln_gamma", T + "embeddings.LayerNorm.weight", False),
        ("embeddings_ln_beta", T + "embeddings.LayerNorm.bias", False),
        ("rel_embeddings", T + "encoder.rel_embeddings.weight", True),
        ("rel_ln_gamma", T + "encoder.LayerNorm.weight", False),
        ("rel_ln_beta", T + "encoder.LayerNorm.bias", False),
    ]
    for i in range(n_layers):
        p, t = f"layers.{i}.", f"{T}encoder.layer.{i}."
        plan += [
            (p + "attn_q_w", t + "attention.self.query_proj.weight", True),
            (p + "attn_q_b", t + "attention.self.query_proj.bias", False),
            (p + "attn_k_w", t + "attention.self.key_proj.weight", True),
            (p + "attn_k_b", t + "attention.self.key_proj.bias", False),
            (p + "attn_v_w", t + "attention.self.value_proj.weight", True),
            (p + "attn_v_b", t + "attention.self.value_proj.bias", False),
            (p + "attn_o_w", t + "attention.output.dense.weight", True),
            (p + "attn_o_b", t + "attention.output.dense.bias", False),
            (p + "attn_ln_gamma", t + "attention.output.LayerNorm.weight", False),
            (p + "attn_ln_beta", t + "attention.output.LayerNorm.bias", False),
            (p + "ffn_up_w", t + "intermediate.dense.weight", True),
            (p + "ffn_up_b", t + "intermediate.dense.bias", False),
            (p + "ffn_down_w", t + "output.dense.weight", True),
            (p + "ffn_down_b", t + "output.dense.bias", False),
            (p + "ffn_ln_gamma", t + "output.LayerNorm.weight", False),
            (p + "ffn_ln_beta", t + "output.LayerNorm.bias", False),
        ]
    plan += [
        # BiLSTM (flair-голова)
        ("lstm_ih_w", "rnn.lstm.weight_ih_l0", True),
        ("lstm_ih_b", "rnn.lstm.bias_ih_l0", False),
        ("lstm_hh_w", "rnn.lstm.weight_hh_l0", True),
        ("lstm_hh_b", "rnn.lstm.bias_hh_l0", False),
        ("lstm_ih_w_r", "rnn.lstm.weight_ih_l0_reverse", True),
        ("lstm_ih_b_r", "rnn.lstm.bias_ih_l0_reverse", False),
        ("lstm_hh_w_r", "rnn.lstm.weight_hh_l0_reverse", True),
        ("lstm_hh_b_r", "rnn.lstm.bias_hh_l0_reverse", False),
        # SpanMarker: project_start/end (768→1536→768) + out (1536→768)
        ("span_start_0_w", "span_rep_layer.span_rep_layer.project_start.0.weight", True),
        ("span_start_0_b", "span_rep_layer.span_rep_layer.project_start.0.bias", False),
        ("span_start_3_w", "span_rep_layer.span_rep_layer.project_start.3.weight", True),
        ("span_start_3_b", "span_rep_layer.span_rep_layer.project_start.3.bias", False),
        ("span_end_0_w", "span_rep_layer.span_rep_layer.project_end.0.weight", True),
        ("span_end_0_b", "span_rep_layer.span_rep_layer.project_end.0.bias", False),
        ("span_end_3_w", "span_rep_layer.span_rep_layer.project_end.3.weight", True),
        ("span_end_3_b", "span_rep_layer.span_rep_layer.project_end.3.bias", False),
        ("span_out_w", "span_rep_layer.span_rep_layer.out_project.weight", True),
        ("span_out_b", "span_rep_layer.span_rep_layer.out_project.bias", False),
        # prompt-проекция (768→3072→768)
        ("prompt_0_w", "prompt_rep_layer.0.weight", True),
        ("prompt_0_b", "prompt_rep_layer.0.bias", False),
        ("prompt_3_w", "prompt_rep_layer.3.weight", True),
        ("prompt_3_b", "prompt_rep_layer.3.bias", False),
    ]
    return plan


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--hf-dir", required=True,
                    help="каталог чекпойнта (pytorch_model.bin, gliner_config.json, spm.model)")
    ap.add_argument("--out", required=True, help="выходной .pqw")
    ap.add_argument("--quant", choices=["int8", "int4"], default="int8")
    ap.add_argument("--spm", default=None, help="путь к spm.model (по умолчанию --hf-dir/spm.model)")
    args = ap.parse_args()

    cfg = json.load(open(os.path.join(args.hf_dir, "gliner_config.json"), encoding="utf-8"))
    n_layers = 12  # mdeberta-v3-base
    hidden = cfg.get("hidden_size", 768)
    max_width = cfg.get("max_width", 12)
    max_len = cfg.get("max_len", 384)
    print(f"архитектура: gliner {cfg.get('model_name')}, слоёв {n_layers}, hidden {hidden}, "
          f"max_width {max_width}, квант {args.quant}")

    bin_path = os.path.join(args.hf_dir, "pytorch_model.bin")
    tensors, prefix, z = load_tensor_meta(bin_path)
    plan = gliner_plan(tensors, n_layers)
    missing = [pqw_name for pqw_name, torch_name, _ in plan if torch_name not in tensors]
    if missing:
        raise SystemExit(f"в модели нет тензоров: {missing}")

    def make_reader(torch_name):
        meta = tensors[torch_name]
        shape, dtype, key, off_elems = meta["shape"], meta["dtype"], meta["key"], meta["offset"]
        if dtype not in TORCH_DTYPES:
            raise SystemExit(f"тензор {torch_name}: dtype {dtype} не поддерживается")
        fmt, item = TORCH_DTYPES[dtype]
        entry = f"{prefix}/data/{key}"
        if entry not in z.namelist():
            raise SystemExit(f"zip-запись {entry} не найдена")
        zinfo = z.getinfo(entry)
        if zinfo.compress_type != zipfile.ZIP_STORED:
            raise SystemExit(f"zip-запись {entry} сжата — ожидаем STORED")

        def reader(r0, r1):
            with z.open(entry) as fh:
                fh.seek((off_elems + r0 * cols) * item)
                return fh.read((r1 - r0) * cols * item)

        rows = shape[0] if len(shape) == 2 else 1
        cols = shape[1] if len(shape) == 2 else (shape[0] if shape else 1)
        return reader, rows, cols

    w = PqwWriter(args.out)
    qcode = {"int8": I8, "int4": I4}[args.quant]

    for pqw_name, torch_name, quantized in plan:
        reader, rows, cols = make_reader(torch_name)
        shape = tensors[torch_name]["shape"]
        if quantized and len(shape) == 2:
            gen, scales = quant_rows_iter(reader, rows, cols, args.quant)
            n = w.write_section(pqw_name, qcode, [rows, cols], gen, scales)
            print(f"  {pqw_name:24s} [{rows}, {cols}] → {args.quant} {n/1e6:.1f} МБ")
        else:
            numel = 1
            for s in shape:
                numel *= s
            gen = (reader(0, 1) if len(shape) == 2 else iter([reader(0, 1)]))
            # 1D-тензоры: cols=numel, одна строка
            def gen1(r=reader, n=numel, item=4):
                with z.open(f"{prefix}/data/{tensors[torch_name]['key']}") as fh:
                    fh.seek(tensors[torch_name]["offset"] * item)
                    yield fh.read(n * item)
            n = w.write_section(pqw_name, F32, [numel], gen1(), [])
            print(f"  {pqw_name:24s} [{numel}] → f32 {n} Б")

    # секция токенизатора v2
    spm_path = args.spm or os.path.join(args.hf_dir, "spm.model")
    section, vocab_n, ids = build_tokenizer_section(spm_path)
    w.write_section("__tokenizer__", RAW, [len(section)], iter([section]), [])
    print(f"  __tokenizer__           unigram {vocab_n} кусков (v2, mdeberta) {len(section)/1e6:.1f} МБ")

    meta_json = json.dumps({
        "max_width": max_width,
        "ent_id": ids["ent"],
        "sep_id": ids["sep"],
        "flert_id": ids["flert"],
    }, ensure_ascii=False).encode("utf-8")
    w.write_section("__gliner__", RAW, [len(meta_json)], iter([meta_json]), [])
    print(f"  __gliner__              {meta_json.decode()}")

    emb_rows = tensors[T + "embeddings.word_embeddings.weight"]["shape"][0]
    fields = dict(
        model_type=GLINER, quant=qcode, flags=FLAGS,
        layers=n_layers, hidden=hidden, intermediate=4 * hidden,
        heads=hidden // 64, head_dim=64, vocab=emb_rows, max_pos=max_len,
        experts=0, top_k=0, kv_heads=1, _path=args.out,
    )
    file_len, n_sections = w.finish(fields)
    print(f"OK: {args.out} — {file_len / 1e6:.1f} МБ, секций {n_sections}, "
          f"SHA-256 в заголовке (верифицируется при open в Rust)")


if __name__ == "__main__":
    main()
