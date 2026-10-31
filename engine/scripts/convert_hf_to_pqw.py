#!/usr/bin/env python3
"""Конвертер реальных весов HuggingFace → .pqw v2 (Part E, задача E.8).

Самодостаточен: stdlib + numpy. Читает torch-zip (pytorch_model.bin —
это обычный ZIP: data.pkl + data/<n>), мапит имена XLM-R на конвенцию
pqw-энкодера, квантует weight-only int8/int4 ПОСТРОЧНО потоково
(блоками строк — RAM не растёт с размером модели; тот же паттерн
пригодится для 70B), встраивает токенизатор (секция `__tokenizer__`:
Unigram + Metaspace + таблица нормализации), пишет таблицу тензоров
и SHA-256 ровно по спецификации docs/PQW_FORMAT.md.

Выход открывается `QuantizedWeightsView::open` (Rust) — кросс-языковая
проверка формата, как у demo-писателя.

Пример (BGE-M3):
  python3 scripts/convert_hf_to_pqw.py \
    --hf-dir /path/to/bge-m3 --out models/bge-m3.pqw --quant int8
"""
import argparse
import hashlib
import io
import json
import os
import pickle
import struct
import sys
import zipfile

import numpy as np

PAGE = 4096
HEADER = 128
MAGIC = b"PQW2NN\0\0"
VERSION = 2

# dtype
F32, I8, I4, RAW = 0, 1, 2, 3
# model_type
ENC = 0
# quant
Q_F32, Q_I8, Q_I4 = 0, 1, 2

TORCH_DTYPES = {
    "float32": ("<f4", 4), "float": ("<f4", 4),
    "float16": ("<f2", 2), "half": ("<f2", 2),
    "float64": ("<f8", 8), "double": ("<f8", 8),
}


# ---------------------------------------------------------------------------
# torch-zip: разбор data.pkl со стабами (без torch)
# ---------------------------------------------------------------------------

class _Stub:
    def __init__(self, name):
        self.name = name

    def __call__(self, *a, **kw):
        return _Stub(self.name + "()")

    def __repr__(self):
        return f"<stub {self.name}>"


def _rebuild_tensor_v2(storage, storage_offset, size, stride, *args):
    _marker, dtype, key, numel = storage
    return {
        "shape": tuple(int(s) for s in size),
        "dtype": dtype,
        "key": key,
        "offset": int(storage_offset),
        "numel": int(numel),
    }


class _TorchUnpickler(pickle.Unpickler):
    def find_class(self, module, name):
        if module == "collections" and name == "OrderedDict":
            import collections
            return collections.OrderedDict
        if module == "torch._utils" and name.startswith("_rebuild_tensor"):
            return _rebuild_tensor_v2
        return _Stub(f"{module}.{name}")

    def persistent_load(self, pid):
        _typ, storage, key, _loc, numel = pid
        dtype = storage.name.rsplit(".", 1)[-1].replace("Storage", "").lower()
        return ("storage", dtype, str(key), int(numel))


def load_tensor_meta(bin_path):
    """{имя: (shape, dtype, key, offset_элементов)} + префикс архива."""
    z = zipfile.ZipFile(bin_path)
    prefix = z.namelist()[0].split("/")[0]
    pkl = z.read(f"{prefix}/data.pkl")
    state = _TorchUnpickler(io.BytesIO(pkl)).load()
    tensors = {}
    for name, val in state.items():
        if isinstance(val, dict) and "key" in val:
            tensors[name] = val
    return tensors, prefix, z


# ---------------------------------------------------------------------------
# План конвертации: XLM-R → pqw-конвенция
# ---------------------------------------------------------------------------

def xlmr_plan(tensors, n_layers):
    """[(pqw_name, torch_name, quantized?)] в логическом порядке."""
    plan = []
    # эмбеддинги
    for torch_name, pqw_name in [
        ("embeddings.word_embeddings.weight", "word_embeddings"),
        ("embeddings.position_embeddings.weight", "position_embeddings"),
        ("embeddings.token_type_embeddings.weight", "token_type_embeddings"),
    ]:
        if torch_name in tensors:
            plan.append((pqw_name, torch_name, True))
    plan += [
        ("embeddings_ln_gamma", "embeddings.LayerNorm.weight", False),
        ("embeddings_ln_beta", "embeddings.LayerNorm.bias", False),
    ]
    for i in range(n_layers):
        p, t = f"layers.{i}.", f"encoder.layer.{i}."
        plan += [
            (p + "attn_q_w", t + "attention.self.query.weight", True),
            (p + "attn_q_b", t + "attention.self.query.bias", False),
            (p + "attn_k_w", t + "attention.self.key.weight", True),
            (p + "attn_k_b", t + "attention.self.key.bias", False),
            (p + "attn_v_w", t + "attention.self.value.weight", True),
            (p + "attn_v_b", t + "attention.self.value.bias", False),
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
    return plan


# ---------------------------------------------------------------------------
# Токенизатор: секция __tokenizer__ (unigram v1)
# ---------------------------------------------------------------------------

def build_tokenizer_section(tok_json_path, norm_table_path):
    tj = json.load(open(tok_json_path, encoding="utf-8"))
    model = tj["model"]
    if model.get("type") != "Unigram":
        raise SystemExit(f"не Unigram-токенизатор: {model.get('type')}")
    vocab = model["vocab"]

    added = {a["content"]: a["id"] for a in tj.get("added_tokens", [])}
    if not added:
        added = {"<s>": 0, "<pad>": 1, "</s>": 2, "<unk>": 3}
    unk_id = added.get("<unk>", model.get("unk_id", 3))
    bos_id = added.get("<s>", 0)
    eos_id = added.get("</s>", 2)
    pad_id = added.get("<pad>", 1)
    mask_id = added.get("<mask>", 0xFFFFFFFF)

    pre = tj.get("pre_tokenizer", {})
    add_prefix = bool(pre.get("add_prefix_space", True))

    norm_table = json.load(open(norm_table_path, encoding="utf-8"))

    out = bytearray()
    out += b"TOKR"
    out += struct.pack("<H", 1)
    out.append(0)  # unigram
    out.append(1 if add_prefix else 0)
    for i in (unk_id, bos_id, eos_id, pad_id, mask_id):
        out += struct.pack("<I", int(i))
    out += struct.pack("<I", len(vocab))
    for piece, score in vocab:
        b = piece.encode("utf-8")
        out += struct.pack("<H", len(b))
        out += b
        out += struct.pack("<f", float(score))
    specials = sorted(added.items(), key=lambda kv: -len(kv[0]))
    out += struct.pack("<I", len(specials))
    for content, sid in specials:
        b = content.encode("utf-8")
        out += struct.pack("<H", len(b))
        out += b
        out += struct.pack("<I", int(sid))
    out += struct.pack("<I", len(norm_table))
    for cp, rep in norm_table.items():
        b = rep.encode("utf-8")
        out += struct.pack("<I", int(cp))
        out += struct.pack("<H", len(b))
        out += b
    return bytes(out), len(vocab)


# ---------------------------------------------------------------------------
# Потоковый писатель .pqw
# ---------------------------------------------------------------------------

class PqwWriter:
    """Секции пишутся блоками, SHA-256 инкрементально по [4096..EOF)."""

    def __init__(self, path):
        self.path = path
        self.f = open(path, "wb")
        self.f.write(b"\0" * HEADER)
        self._pad_to_page(hash_it=False)
        self.sha = hashlib.sha256()
        self.entries = []  # (name, dtype, dims, scales, offset, length)

    def _pad_to_page(self, hash_it: bool):
        pos = self.f.tell()
        pad = (PAGE - pos % PAGE) % PAGE
        if pad:
            chunk = b"\0" * pad
            self.f.write(chunk)
            if hash_it:
                # паддинг между секциями входит в SHA-диапазон [4096..EOF)
                self.sha.update(chunk)

    def write_section(self, name, dtype, dims, data_iter, scales):
        """data_iter — генератор байтовых блоков (пишем потоково)."""
        self._pad_to_page(hash_it=True)
        off = self.f.tell()
        length = 0
        for chunk in data_iter:
            self.f.write(chunk)
            self.sha.update(chunk)
            length += len(chunk)
        self.entries.append((name, dtype, dims, scales, off, length))
        return length

    def finish(self, header_fields):
        # таблица тензоров (на границе страницы)
        self._pad_to_page(hash_it=True)
        table_off = self.f.tell()
        table = bytearray()
        for name, dtype, dims, scales, off, length in self.entries:
            nb = name.encode("utf-8")
            table += struct.pack("<H", len(nb)) + nb
            table.append(dtype)
            table.append(len(dims))
            table += struct.pack("<H", 0)
            table += struct.pack("<I", len(scales))
            for d in dims:
                table += struct.pack("<Q", d)
            table += struct.pack("<Q", off)
            table += struct.pack("<Q", length)
            for s in scales:
                table += struct.pack("<f", s)
        self.f.write(bytes(table))
        self.sha.update(bytes(table))
        table_len = len(table)
        file_len = self.f.tell()
        self.f.close()

        # заголовок по спецификации
        buf = bytearray(HEADER)
        buf[0:8] = MAGIC
        struct.pack_into("<I", buf, 8, VERSION)
        struct.pack_into("<I", buf, 12, HEADER)
        buf[16] = header_fields["model_type"]
        buf[17] = header_fields["quant"]
        struct.pack_into("<H", buf, 18, header_fields["flags"])
        for key, off in [("layers", 20), ("hidden", 24), ("intermediate", 28),
                         ("heads", 32), ("head_dim", 36), ("vocab", 40),
                         ("max_pos", 44), ("experts", 48), ("top_k", 52),
                         ("kv_heads", 56)]:
            struct.pack_into("<I", buf, off, header_fields[key])
        struct.pack_into("<I", buf, 60, 0)
        struct.pack_into("<Q", buf, 64, table_off)
        struct.pack_into("<Q", buf, 72, table_len)
        struct.pack_into("<Q", buf, 80, table_off - PAGE)
        struct.pack_into("<Q", buf, 88, file_len)
        buf[96:128] = self.sha.digest()

        with open(self.path, "r+b") as f:
            f.seek(0)
            f.write(bytes(buf))
        return file_len, len(self.entries)


def quant_rows_iter(reader, rows, cols, quant, block=8192):
    """Генератор квантованных блоков + копилка масштабов."""
    scales = np.zeros(rows, dtype=np.float32)

    def gen():
        for r0 in range(0, rows, block):
            r1 = min(r0 + block, rows)
            w = np.frombuffer(
                reader(r0, r1), dtype="<f4").astype(np.float32)
            w = w.reshape(r1 - r0, cols)
            amax = np.maximum(np.abs(w).max(axis=1), 1e-12)
            if quant == "int8":
                sc = amax / 127.0
                q = np.clip(np.rint(w / sc[:, None]), -127, 127).astype(np.int8)
                scales[r0:r1] = sc
                yield q.tobytes()
            else:  # int4: nibble −8..7 (чётный элемент — младший)
                sc = amax / 7.0
                q = (np.clip(np.rint(w / sc[:, None]), -8, 7) + 8).astype(np.uint8)
                scales[r0:r1] = sc
                even = q[:, 0::2]
                odd = q[:, 1::2]
                if odd.shape[1] < even.shape[1]:  # нечётный cols — добиваем нулём
                    odd = np.hstack([odd, np.zeros((odd.shape[0], 1), dtype=np.uint8)])
                packed = even | (odd << 4)
                yield packed.tobytes()

    return gen(), scales


def f32_iter(reader, rows, cols, block=65536):
    def gen():
        for r0 in range(0, rows, block):
            r1 = min(r0 + block, rows)
            yield reader(r0, r1)

    return gen(), None


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--hf-dir", required=True, help="каталог модели (pytorch_model.bin, config.json, tokenizer.json)")
    ap.add_argument("--out", required=True, help="выходной .pqw")
    ap.add_argument("--quant", choices=["int8", "int4"], default="int8")
    ap.add_argument("--norm-table", default=os.path.join(here, "xlmr_norm_table.json"))
    ap.add_argument("--no-tokenizer", action="store_true")
    args = ap.parse_args()

    cfg = json.load(open(os.path.join(args.hf_dir, "config.json"), encoding="utf-8"))
    n_layers = cfg["num_hidden_layers"]
    hidden = cfg["hidden_size"]
    intermediate = cfg["intermediate_size"]
    heads = cfg["num_attention_heads"]
    vocab = cfg["vocab_size"]
    max_pos = cfg["max_position_embeddings"]
    print(f"архитектура: слоёв {n_layers}, hidden {hidden}, голов {heads}, "
          f"vocab {vocab}, max_pos {max_pos}, квант {args.quant}")

    bin_path = os.path.join(args.hf_dir, "pytorch_model.bin")
    tensors, prefix, z = load_tensor_meta(bin_path)
    plan = xlmr_plan(tensors, n_layers)
    missing = [pqw_name for pqw_name, torch_name, _ in plan if torch_name not in tensors]
    if missing:
        raise SystemExit(f"в модели нет тензоров: {missing}")

    # читатель байтов тензора: блоки строк из zip-записи
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
                # ВАЖНО: смещение = storage_offset + номер строки × cols
                # (иначе каждый блок перечитывает первые строки!)
                fh.seek((off_elems + r0 * cols) * item)
                # 2D: блок строк [r1-r0, cols]; 1D: весь тензор (rows=1, cols=numel)
                return fh.read((r1 - r0) * cols * item)
        # для 1D: строка = весь тензор, cols = numel
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
            # ВАЖНО: scales (np-массив) мутируется генератором ВО ВРЕМЯ
            # записи секции — в таблицу попадает уже заполненный (в finish()).
            n = w.write_section(pqw_name, qcode, [rows, cols], gen, scales)
            print(f"  {pqw_name:42s} [{rows}, {cols}] → {args.quant} {n} Б")
        else:
            numel = shape[0] if shape else 1
            gen, _ = f32_iter(reader, rows, cols)
            n = w.write_section(pqw_name, F32, [numel], gen, [])
            print(f"  {pqw_name:42s} [{numel}] → f32 {n} Б")

    # секция токенизатора
    tok_path = os.path.join(args.hf_dir, "tokenizer.json")
    if not args.no_tokenizer and os.path.exists(tok_path):
        section, vocab_n = build_tokenizer_section(tok_path, args.norm_table)
        w.write_section("__tokenizer__", RAW, [len(section)], iter([section]), [])
        print(f"  __tokenizer__                          unigram {vocab_n} кусков, секция {len(section)} Б")

    fields = dict(
        model_type=ENC, quant={"int8": Q_I8, "int4": Q_I4}[args.quant],
        flags=1 | 2,  # bias + xlmr-позиции
        layers=n_layers, hidden=hidden, intermediate=intermediate, heads=heads,
        head_dim=hidden // heads, vocab=vocab, max_pos=max_pos,
        experts=0, top_k=0, kv_heads=1, _path=args.out,
    )
    file_len, n_sections = w.finish(fields)
    print(f"OK: {args.out} — {file_len / 1e6:.1f} МБ, секций {n_sections}, "
          f"SHA-256 в заголовке (верифицируется при open в Rust)")


if __name__ == "__main__":
    main()
