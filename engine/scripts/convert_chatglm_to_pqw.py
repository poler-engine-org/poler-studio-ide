#!/usr/bin/env python3
"""
ChatGLM-3-6B to POLER .pqw Streaming Converter (Pure Python + NumPy).

Streams safetensors shards directly from HuggingFace (or local dir),
quantizes weights to INT4 with per-row scales, extracts ChatGLM3 SentencePiece
tokenizer with byte-fallback, and produces a sovereign zero-dependency .pqw file.
"""

import sys
import os
import json
import struct
import hashlib
import urllib.request
import numpy as np

PAGE = 4096
MAGIC = b"POLERQW\x00"
VERSION = 2
HEADER_SIZE = 128

MODEL_TYPE_DECODER = 1
QUANT_INT4 = 2
QUANT_TRIT5 = 3

def parse_spm_tokenizer(path):
    with open(path, 'rb') as f:
        data = f.read()
    pos = 0
    pieces = []
    while pos < len(data):
        tag = 0
        shift = 0
        while True:
            b = data[pos]
            pos += 1
            tag |= (b & 0x7F) << shift
            if not (b & 0x80): break
            shift += 7
        fn = tag >> 3
        wt = tag & 7
        if wt == 0:
            v = 0
            shift = 0
            while True:
                b = data[pos]
                pos += 1
                v |= (b & 0x7F) << shift
                if not (b & 0x80): break
                shift += 7
        elif wt == 1: pos += 8
        elif wt == 2:
            length = 0
            shift = 0
            while True:
                b = data[pos]
                pos += 1
                length |= (b & 0x7F) << shift
                if not (b & 0x80): break
                shift += 7
            field_data = data[pos:pos+length]
            pos += length
            if fn == 1:
                ppos = 0
                ptext = b''
                pscore = 0.0
                ptype = 1
                while ppos < len(field_data):
                    ptag = 0
                    pshift = 0
                    while True:
                        pb = field_data[ppos]
                        ppos += 1
                        ptag |= (pb & 0x7F) << pshift
                        if not (pb & 0x80): break
                        pshift += 7
                    pfn = ptag >> 3
                    pwt = ptag & 7
                    if pwt == 0:
                        pv = 0
                        pshift = 0
                        while True:
                            pb = field_data[ppos]
                            ppos += 1
                            pv |= (pb & 0x7F) << pshift
                            if not (pb & 0x80): break
                            pshift += 7
                        if pfn == 3: ptype = pv
                    elif pwt == 1: ppos += 8
                    elif pwt == 2:
                        plen = 0
                        pshift = 0
                        while True:
                            pb = field_data[ppos]
                            ppos += 1
                            plen |= (pb & 0x7F) << pshift
                            if not (pb & 0x80): break
                            pshift += 7
                        ptext = field_data[ppos:ppos+plen]
                        ppos += plen
                    elif pwt == 5:
                        pscore = struct.unpack('<f', field_data[ppos:ppos+4])[0]
                        ppos += 4
                pieces.append((ptext, pscore, ptype))
        elif wt == 5: pos += 4
    return pieces

def build_tokenizer_section(pieces):
    out = bytearray(b"TOKR")
    out.extend(struct.pack("<H", 2))
    out.append(0)
    out.append(1)
    out.extend(struct.pack("<IIIII", 0, 1, 2, 3, 64789))
    out.extend(struct.pack("<I", len(pieces)))
    for ptext, pscore, _ in pieces:
        out.extend(struct.pack("<H", len(ptext)))
        out.extend(ptext)
        out.extend(struct.pack("<f", pscore))
    specials = [
        (b"[MASK]", 64789),
        (b"[gMASK]", 64790),
        (b"[sMASK]", 64791),
        (b"sop", 64792),
        (b"eop", 64793),
        (b"<|system|>", 64794),
        (b"<|user|>", 64795),
        (b"<|assistant|>", 64796),
        (b"<|observation|>", 64797),
    ]
    out.extend(struct.pack("<I", len(specials)))
    for stext, sid in specials:
        out.extend(struct.pack("<H", len(stext)))
        out.extend(stext)
        out.extend(struct.pack("<I", sid))
    out.append(2)
    return bytes(out)

def quantize_int4(w):
    rows, cols = w.shape
    pc = cols + cols % 2
    scales = (np.abs(w).max(axis=1).astype(np.float32) / 7.0)
    scales[scales == 0] = 1.0
    packed = np.zeros((rows, pc // 2), dtype=np.uint8)
    CH = 4096
    for s in range(0, rows, CH):
        e = min(s + CH, rows)
        q = np.clip(np.round(w[s:e].astype(np.float32) / scales[s:e, None]), -7, 7).astype(np.int8)
        if cols % 2 != 0:
            q = np.pad(q, ((0, 0), (0, 1)), mode='constant')
        nib_lo = (q[:, 0::2] + 8).astype(np.uint8)
        nib_hi = (q[:, 1::2] + 8).astype(np.uint8)
        packed[s:e] = (nib_hi << 4) | nib_lo
    return packed.tobytes(), scales.astype('<f4').tobytes(), list(scales)

def quantize_trit5(w):
    """Троичное квантование per-row — бит-в-бит как quant_trit5_per_row в Rust:
    порог ±0.33 на нормированное значение, упаковка 5 тритов в байт
    (byte = sum((t_i+1)*3^i)), dtype тензора = 4 (Trit5).
    Работает чанками по строкам: RAM-пик ~50 МБ даже на матрицах 65024x4096."""
    rows, cols = w.shape
    n_bytes = (cols + 4) // 5  # байт на строку (5 тритов -> 1 байт)
    scales = np.abs(w).max(axis=1).astype(np.float32)
    scales[scales == 0] = 1.0
    packed = np.zeros((rows, n_bytes), dtype=np.uint8)
    CH = 4096  # строк за чанк
    for s in range(0, rows, CH):
        e = min(s + CH, rows)
        normed = w[s:e].astype(np.float32) / scales[s:e, None]
        trits = np.zeros((e - s, n_bytes * 5), dtype=np.int8)
        trits[:, :cols][normed > 0.33] = 1
        trits[:, :cols][normed < -0.33] = -1
        # 5 тритов -> 1 байт: byte = sum_k trit[5j+k] * 3^k
        t5 = (trits + 1).astype(np.uint16).reshape(e - s, n_bytes, 5)
        b = np.zeros((e - s, n_bytes), dtype=np.uint16)
        for k in range(5):
            b += t5[:, :, k] * (3 ** k)  # максимум 2*121=242 < 256, переполнения нет
        packed[s:e] = b.astype(np.uint8)
    return packed.tobytes(), scales.astype('<f4').tobytes(), list(scales)

def parse_safetensors_metadata(file_path):
    with open(file_path, 'rb') as f:
        header_len_bytes = f.read(8)
        if len(header_len_bytes) < 8:
            return None, 0
        header_len = struct.unpack('<Q', header_len_bytes)[0]
        header_json = f.read(header_len).decode('utf-8')
        metadata = json.loads(header_json)
        return metadata, 8 + header_len

def load_safetensors_tensor(f, offset_base, tensor_info):
    dtype_str = tensor_info['dtype']
    shape = tensor_info['shape']
    data_offsets = tensor_info['data_offsets']
    f.seek(offset_base + data_offsets[0])
    raw_data = f.read(data_offsets[1] - data_offsets[0])
    if dtype_str == 'F16':
        arr = np.frombuffer(raw_data, dtype=np.float16)  # f16 как есть; квантер сам конвертит чанками
    elif dtype_str == 'BF16':
        u16 = np.frombuffer(raw_data, dtype=np.uint16)
        u32 = u16.astype(np.uint32) << 16
        arr = u32.view(np.float32)
    elif dtype_str == 'F32':
        arr = np.frombuffer(raw_data, dtype=np.float32)
    else:
        raise ValueError(f"Unsupported dtype: {dtype_str}")
    return arr.reshape(shape)

class StreamingPqwWriter:
    def __init__(self, out_path, layers=28, hidden=4096, heads=32, intermediate=13696, vocab=65024, max_pos=8192, kv_heads=2,
                 qk_layer_scaling=True, rms_eps=1e-5, quant_dtype=4, quant_hdr=None):
        self.out_path = out_path
        self.ckpt_path = out_path + '.ckpt'
        # Возобновление после обрыва сессии: контрольная точка хранит
        # уже записанные тензоры и позицию в файле; hasher восстанавливаем
        # по уже записанным данным.
        import pickle, os
        if os.path.exists(self.ckpt_path) and os.path.exists(out_path):
            try:
                with open(self.ckpt_path, 'rb') as cf:
                    state = pickle.load(cf)
                fsize = os.path.getsize(out_path)
                if fsize >= state.get('size', 0):
                    # Файл длиннее ckpt => хвост — недописанный тензор, обрезаем.
                    if fsize > state['size']:
                        with open(out_path, 'r+b') as tf:
                            tf.truncate(state['size'])
                    self.entries = state['entries']
                    self.f = open(out_path, 'r+b')
                    self.f.seek(state['size'])
                    # SHA-256 по уже записанным данным (PAGE..size) —
                    # состояние хеша восстанавливаем перечитыванием.
                    self.f.seek(PAGE)
                    remaining = state['size'] - PAGE
                    self.hasher = hashlib.sha256()
                    while remaining > 0:
                        chunk = self.f.read(min(1 << 22, remaining))
                        if not chunk:
                            break
                        self.hasher.update(chunk)
                        remaining -= len(chunk)
                    self.f.seek(state['size'])
                    print(f"[Resume] .pqw: продолжаю с {state['size']} байт, тензоров: {len(self.entries)}")
                    self._init_fields(layers, hidden, heads, intermediate, vocab, max_pos, kv_heads,
                                      qk_layer_scaling, rms_eps, quant_dtype, quant_hdr)
                    return
                else:
                    print(f"[Resume] контрольная точка не совпала (ckpt={state.get('size')}, file={os.path.getsize(out_path)}) — с нуля")
            except Exception as e:
                print(f"[Resume] ошибка ckpt ({e}) — с нуля")
        self.f = open(out_path, 'wb')
        self.f.seek(PAGE)
        self.entries = []
        self.hasher = hashlib.sha256()
        self._init_fields(layers, hidden, heads, intermediate, vocab, max_pos, kv_heads,
                          qk_layer_scaling, rms_eps, quant_dtype, quant_hdr)

    def _init_fields(self, layers, hidden, heads, intermediate, vocab, max_pos, kv_heads,
                     qk_layer_scaling, rms_eps, quant_dtype, quant_hdr):
        self.quant_hdr = quant_hdr if quant_hdr is not None else quant_dtype
        self.layers = layers
        self.hidden = hidden
        self.heads = heads
        self.intermediate = intermediate
        self.vocab = vocab
        self.max_pos = max_pos
        self.kv_heads = kv_heads
        self.qk_layer_scaling = qk_layer_scaling
        self.rms_eps = rms_eps
        self.quant_dtype = quant_dtype

    def add_tensor(self, name, dtype, dims, scales, data_bytes):
        curr_pos = self.f.tell()
        aligned = (curr_pos + PAGE - 1) // PAGE * PAGE
        if aligned > curr_pos:
            pad = b'\x00' * (aligned - curr_pos)
            self.f.write(pad)
            self.hasher.update(pad)
        data_off = aligned
        self.f.write(data_bytes)
        self.hasher.update(data_bytes)
        self.entries.append({
            'name': name,
            'dtype': dtype,
            'dims': dims,
            'scales': scales,
            'data_off': data_off,
            'data_len': len(data_bytes)
        })
        self._save_ckpt()

    def _save_ckpt(self):
        import pickle
        tmp = self.ckpt_path + '.tmp'
        with open(tmp, 'wb') as cf:
            pickle.dump({'entries': self.entries, 'size': self.f.tell()}, cf)
        os.replace(tmp, self.ckpt_path)

    def finish(self):
        curr_pos = self.f.tell()
        table_offset = (curr_pos + PAGE - 1) // PAGE * PAGE
        if table_offset > curr_pos:
            pad = b'\x00' * (table_offset - curr_pos)
            self.f.write(pad)
            self.hasher.update(pad)
        table_start = self.f.tell()
        table_buf = bytearray()
        for t in self.entries:
            name_b = t['name'].encode('utf-8')
            table_buf.extend(struct.pack('<H', len(name_b)))
            table_buf.extend(name_b)
            table_buf.append(t['dtype'])
            table_buf.append(len(t['dims']))
            table_buf.extend(struct.pack('<H', 0))
            table_buf.extend(struct.pack('<I', len(t['scales'])))
            for d in t['dims']:
                table_buf.extend(struct.pack('<Q', d))
            table_buf.extend(struct.pack('<Q', t['data_off']))
            table_buf.extend(struct.pack('<Q', t['data_len']))
            for s in t['scales']:
                table_buf.extend(struct.pack('<f', s))
        self.f.write(table_buf)
        self.hasher.update(table_buf)
        table_len = len(table_buf)
        file_len = self.f.tell()
        digest = self.hasher.digest()
        head = bytearray(HEADER_SIZE)
        head[0:8] = MAGIC
        struct.pack_into('<I', head, 8, VERSION)
        struct.pack_into('<I', head, 12, HEADER_SIZE)
        head[16] = MODEL_TYPE_DECODER
        head[17] = self.quant_hdr  # основной режим квантования (Quant-enum)
        flags = 16 if self.qk_layer_scaling else 0  # бит 4 (0x10): послойный масштаб
        struct.pack_into('<H', head, 18, flags)
        struct.pack_into('<I', head, 20, self.layers)
        struct.pack_into('<I', head, 24, self.hidden)
        struct.pack_into('<I', head, 28, self.intermediate)
        struct.pack_into('<I', head, 32, self.heads)
        head_dim = self.hidden // self.heads
        struct.pack_into('<I', head, 36, head_dim)
        struct.pack_into('<I', head, 40, self.vocab)
        struct.pack_into('<I', head, 44, self.max_pos)
        struct.pack_into('<I', head, 48, 0)
        struct.pack_into('<I', head, 52, 0)
        struct.pack_into('<I', head, 56, self.kv_heads)
        # Ранее зарезервированное поле: eps RMSNorm (0.0 -> движок трактует как 1e-6).
        struct.pack_into('<f', head, 60, self.rms_eps)
        struct.pack_into('<Q', head, 64, table_offset)
        struct.pack_into('<Q', head, 72, table_len)
        struct.pack_into('<Q', head, 80, table_offset - PAGE)
        struct.pack_into('<Q', head, 88, file_len)
        head[96:128] = digest
        self.f.seek(0)
        self.f.write(head)
        self.f.close()
        print(f"\n[PQW] Successfully wrote {file_len / (1024*1024):.2f} MB to {self.out_path}")

def download_file(url, target_path):
    """Загрузка через curl: докачка (-C -) + ретраи + таймауты — устойчива к обрывам."""
    import subprocess
    print(f"[Download] {url} -> {target_path} (curl, resume+retry)")
    for attempt in range(1, 11):
        r = subprocess.run(
            ['curl', '-L', '-s', '-S', '--fail', '--retry', '20',
             '--retry-delay', '5', '--connect-timeout', '30',
             '--retry-all-errors', '--speed-time', '60', '--speed-limit', '10240',
             '-C', '-', '-o', target_path, url],
            env={**os.environ})
        if r.returncode == 0:
            if os.path.exists(target_path):
                print(f"  done: {os.path.getsize(target_path)} bytes")
            return
        print(f"  [retry {attempt}/10] curl exit={r.returncode}, повтор через 10с...")
        import time; time.sleep(10)
    raise RuntimeError(f"не удалось скачать {url} после 10 попыток")

def convert_chatglm3(model_dir_or_repo, out_pqw_path, quant='trit5'):
    quant_dtype = 4 if quant == 'trit5' else 2  # Dtype-enum: 4 = Trit5, 2 = Int4
    quant_hdr = 3 if quant == 'trit5' else 2    # Quant-enum: 3 = Trit5
    quantize = quantize_trit5 if quant == 'trit5' else quantize_int4
    print(f"[Quant] Режим квантования: {quant} (dtype={quant_dtype})")
    is_hf_repo = "/" in model_dir_or_repo and not os.path.exists(model_dir_or_repo)
    base_url = f"https://huggingface.co/{model_dir_or_repo}/resolve/main" if is_hf_repo else ""
    os.makedirs("tmp_dl", exist_ok=True)
    tok_model_path = os.path.join("tmp_dl", "chatglm3_tokenizer.model")
    if not os.path.exists(tok_model_path):
        if is_hf_repo:
            download_file(f"{base_url}/tokenizer.model", tok_model_path)
        else:
            tok_model_path = os.path.join(model_dir_or_repo, "tokenizer.model")
    print("[Tokenizer] Parsing SPM tokenizer...")
    spm_pieces = parse_spm_tokenizer(tok_model_path)
    tok_section_bytes = build_tokenizer_section(spm_pieces)
    print(f"[Tokenizer] Built section ({len(tok_section_bytes)} bytes, {len(spm_pieces)} pieces)")
    index_path = os.path.join("tmp_dl", "model.safetensors.index.json")
    if is_hf_repo:
        if not os.path.exists(index_path):
            download_file(f"{base_url}/model.safetensors.index.json", index_path)
        with open(index_path, 'r') as f:
            index_data = json.load(f)
    else:
        with open(os.path.join(model_dir_or_repo, "model.safetensors.index.json"), 'r') as f:
            index_data = json.load(f)
    weight_map = index_data.get('weight_map', {})
    all_shards = sorted(list(set(weight_map.values())))
    # Шарды, уже лежащие в tmp_dl целиком, обрабатываем ПЕРВЫМИ и удаляем —
    # в тесной песочнице (~5.5 ГБ бюджета) это единственный способ пройти
    # конвертацию без переполнения диска. Порядок тензоров в .pqw движку
    # неважен (таблица ищется по имени).
    def shard_bytes(s):
        p = os.path.join('tmp_dl', s)
        return os.path.getsize(p) if os.path.exists(p) else 0
    on_disk = [s for s in all_shards if shard_bytes(s) > 0]
    shards = on_disk + [s for s in all_shards if s not in on_disk]
    if on_disk:
        print(f"[Order] сначала локальные шарды ({', '.join(on_disk)}), затем докачка остальных")
    print(f"[Model] Found {len(shards)} safetensors shards.")
    # ChatGLM3: эффективный масштаб внимания = 1/sqrt(hd) — layer_number
    # в reference-реализации сокращается (alpha=1/(sqrt(hd)*L) затем *L),
    # путь SDPA (PyTorch>=2) тоже делит только на sqrt(hd).
    writer = StreamingPqwWriter(out_pqw_path, layers=28, hidden=4096, heads=32, intermediate=13696, vocab=65024, max_pos=8192, kv_heads=2,
                             qk_layer_scaling=False, rms_eps=1e-5, quant_dtype=quant_dtype, quant_hdr=quant_hdr)
    writer.add_tensor("__tokenizer__", 3, [len(tok_section_bytes)], [], tok_section_bytes)
    for s_idx, shard_name in enumerate(shards):
        print(f"\n[Shard {s_idx+1}/{len(shards)}] Processing {shard_name} ...")
        # Пропуск целиком обработанных шардов БЕЗ скачивания (по weight_map).
        def pqw_names_for_shard(shard):
            names = set()
            for k in weight_map:
                if weight_map[k] != shard:
                    continue
                if k == 'transformer.embedding.word_embeddings.weight':
                    names.add('word_embeddings')
                elif k == 'transformer.encoder.final_layernorm.weight':
                    names.add('final_norm_gamma')
                elif k == 'transformer.output_layer.weight':
                    names.add('lm_head_w')
                elif k.startswith('transformer.encoder.layers.'):
                    p = k.split('.')
                    li = int(p[3]); s = '.'.join(p[4:])
                    m = {'input_layernorm.weight': {f'layers.{li}.attn_norm_gamma'},
                         'post_attention_layernorm.weight': {f'layers.{li}.ffn_norm_gamma'},
                         'self_attention.query_key_value.weight': {f'layers.{li}.q_proj_w', f'layers.{li}.k_proj_w', f'layers.{li}.v_proj_w'},
                         'self_attention.query_key_value.bias': {f'layers.{li}.qkv_b'},
                         'self_attention.dense.weight': {f'layers.{li}.out_proj_w'},
                         'mlp.dense_h_to_4h.weight': {f'layers.{li}.ffn_gate_w', f'layers.{li}.ffn_up_w'},
                         'mlp.dense_4h_to_h.weight': {f'layers.{li}.ffn_down_w'}}
                    names |= m.get(s, set())
            return names

        shard_names = pqw_names_for_shard(shard_name)
        done = {e['name'] for e in writer.entries}
        if shard_names and shard_names <= done:
            print(f"[Skip] {shard_name}: все {len(shard_names)} тензоров уже в .pqw — не качаю")
            continue
        if is_hf_repo:
            shard_path = os.path.join("tmp_dl", shard_name)
            url = f"{base_url}/{shard_name}"
            # Проверка целостности: докачиваем, если размер не совпадает (curl -C -).
            import subprocess
            r = subprocess.run(['curl', '-sIL', url], capture_output=True, text=True)
            expected = 0
            for line in r.stdout.lower().split('\n'):
                if line.startswith('content-length:'):
                    expected = int(line.split(':')[1].strip())
            have = os.path.getsize(shard_path) if os.path.exists(shard_path) else 0
            if have != expected:
                print(f"  [Resume] частичный файл {have} != {expected}, докачиваю...")
                download_file(url, shard_path)
        else:
            shard_path = os.path.join(model_dir_or_repo, shard_name)
        meta, base_off = parse_safetensors_metadata(shard_path)
        with open(shard_path, 'rb') as f:
            for k, info in meta.items():
                if k == '__metadata__': continue
                # Целевое имя тензора в .pqw (для проверки докрученности).
                if k == 'transformer.embedding.word_embeddings.weight':
                    tgt = {'word_embeddings'}
                elif k == 'transformer.encoder.final_layernorm.weight':
                    tgt = {'final_norm_gamma'}
                elif k == 'transformer.output_layer.weight':
                    tgt = {'lm_head_w'}
                elif k.startswith('transformer.encoder.layers.'):
                    parts = k.split('.')
                    l_idx = int(parts[3])
                    sub = '.'.join(parts[4:])
                    m = {
                        'input_layernorm.weight': {f'layers.{l_idx}.attn_norm_gamma'},
                        'post_attention_layernorm.weight': {f'layers.{l_idx}.ffn_norm_gamma'},
                        'self_attention.query_key_value.weight': {f'layers.{l_idx}.q_proj_w', f'layers.{l_idx}.k_proj_w', f'layers.{l_idx}.v_proj_w'},
                        'self_attention.query_key_value.bias': {f'layers.{l_idx}.qkv_b'},
                        'self_attention.dense.weight': {f'layers.{l_idx}.out_proj_w'},
                        'mlp.dense_h_to_4h.weight': {f'layers.{l_idx}.ffn_gate_w', f'layers.{l_idx}.ffn_up_w'},
                        'mlp.dense_4h_to_h.weight': {f'layers.{l_idx}.ffn_down_w'},
                    }
                    tgt = m.get(sub, set())
                else:
                    tgt = set()
                if tgt and tgt <= done:
                    continue  # уже записано в предыдущем заходе
                if k == 'transformer.embedding.word_embeddings.weight':
                    print(f"  Quantizing {k} -> word_embeddings ...")
                    arr = load_safetensors_tensor(f, base_off, info)
                    packed, _, scales = quantize(arr)
                    writer.add_tensor("word_embeddings", quant_dtype, list(arr.shape), scales, packed)
                elif k == 'transformer.encoder.final_layernorm.weight':
                    print(f"  Converting {k} -> final_norm_gamma (f32) ...")
                    arr = load_safetensors_tensor(f, base_off, info)
                    writer.add_tensor("final_norm_gamma", 0, list(arr.shape), [], arr.astype('<f4').tobytes())
                elif k == 'transformer.output_layer.weight':
                    print(f"  Quantizing {k} -> lm_head_w ...")
                    arr = load_safetensors_tensor(f, base_off, info)
                    packed, _, scales = quantize(arr)
                    writer.add_tensor("lm_head_w", quant_dtype, list(arr.shape), scales, packed)
                elif k.startswith('transformer.encoder.layers.'):
                    parts = k.split('.')
                    l_idx = int(parts[3])
                    sub = '.'.join(parts[4:])
                    if sub == 'input_layernorm.weight':
                        arr = load_safetensors_tensor(f, base_off, info)
                        writer.add_tensor(f"layers.{l_idx}.attn_norm_gamma", 0, list(arr.shape), [], arr.astype('<f4').tobytes())
                    elif sub == 'post_attention_layernorm.weight':
                        arr = load_safetensors_tensor(f, base_off, info)
                        writer.add_tensor(f"layers.{l_idx}.ffn_norm_gamma", 0, list(arr.shape), [], arr.astype('<f4').tobytes())
                    elif sub == 'self_attention.query_key_value.weight':
                        arr = load_safetensors_tensor(f, base_off, info)
                        q_w = arr[0:4096, :]
                        k_w = arr[4096:4352, :]
                        v_w = arr[4352:4608, :]
                        p, _, sc = quantize(q_w)
                        writer.add_tensor(f"layers.{l_idx}.q_proj_w", quant_dtype, list(q_w.shape), sc, p)
                        p, _, sc = quantize(k_w)
                        writer.add_tensor(f"layers.{l_idx}.k_proj_w", quant_dtype, list(k_w.shape), sc, p)
                        p, _, sc = quantize(v_w)
                        writer.add_tensor(f"layers.{l_idx}.v_proj_w", quant_dtype, list(v_w.shape), sc, p)
                    elif sub == 'self_attention.query_key_value.bias':
                        # ChatGLM3: add_qkv_bias=true — bias терялся старым конвертером.
                        arr = load_safetensors_tensor(f, base_off, info)
                        writer.add_tensor(f"layers.{l_idx}.qkv_b", 0, list(arr.shape), [], arr.astype('<f4').tobytes())
                    elif sub == 'self_attention.dense.weight':
                        arr = load_safetensors_tensor(f, base_off, info)
                        p, _, sc = quantize(arr)
                        writer.add_tensor(f"layers.{l_idx}.out_proj_w", quant_dtype, list(arr.shape), sc, p)
                    elif sub == 'mlp.dense_h_to_4h.weight':
                        arr = load_safetensors_tensor(f, base_off, info)
                        gate_w = arr[0:13696, :]
                        up_w = arr[13696:27392, :]
                        p, _, sc = quantize(gate_w)
                        writer.add_tensor(f"layers.{l_idx}.ffn_gate_w", quant_dtype, list(gate_w.shape), sc, p)
                        p, _, sc = quantize(up_w)
                        writer.add_tensor(f"layers.{l_idx}.ffn_up_w", quant_dtype, list(up_w.shape), sc, p)
                    elif sub == 'mlp.dense_4h_to_h.weight':
                        arr = load_safetensors_tensor(f, base_off, info)
                        p, _, sc = quantize(arr)
                        writer.add_tensor(f"layers.{l_idx}.ffn_down_w", quant_dtype, list(arr.shape), sc, p)
        if is_hf_repo and os.path.exists(shard_path):
            os.remove(shard_path)
            print(f"  Cleaned up temporary shard {shard_path}")
    writer.finish()
    if os.path.exists(out_pqw_path + '.ckpt'):
        os.remove(out_pqw_path + '.ckpt')
        print('[Cleanup] контрольная точка удалена — конвертация завершена')

if __name__ == '__main__':
    import argparse
    ap = argparse.ArgumentParser(description='ChatGLM3-6B -> .pqw (Trit5/Int4) — эксперимент переписывания весов')
    ap.add_argument('src', nargs='?', default='THUDM/chatglm3-6b',
                    help='HF-репозиторий или локальная директория с safetensors')
    ap.add_argument('dst', nargs='?', default='models/chatglm3-6b.pqw',
                    help='выходной .pqw-файл')
    ap.add_argument('--quant', choices=['trit5', 'int4'], default='trit5',
                    help='режим квантования: trit5 (1.6 бита/вес, по умолчанию) или int4')
    args = ap.parse_args()
    os.makedirs(os.path.dirname(args.dst) or ".", exist_ok=True)
    convert_chatglm3(args.src, args.dst, args.quant)
