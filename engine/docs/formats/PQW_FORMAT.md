# .pqw v2 — POLER Quantum Weights (нейровеса)

> Формат из PLAN_POLER_V2 Part E.3. Заменяет `.onnx`: наш заголовок, наши
> секции, наш SHA-256, наш mmap. Реализация: `src/pqc/pqw.rs` (билдер +
> `QuantizedWeightsView`). Семейство контейнеров согласовано с крейтом
> `pqw` репозитория POLER-Quantum-RS (магия фазовых состояний v1 —
> `POLER_QW`; нейровеса v2 — `PQW2NN`).

## Зачем

| Параметр | ONNX Runtime (ort) | .pqw v2 (pqc) |
|---|---|---|
| Внешние зависимости | libonnxruntime.so (~100 МБ C++) | **0** — нативный Rust |
| Метаданные | избыточный protobuf | 128-байтовый заголовок + таблица |
| Проверка целостности | нет | **SHA-256 при открытии** |
| mmap | нет | **да** (ленивые страницы → 70B с NVMe) |
| Квантование | внешнее | **int8 / int4 нативно** |
| Выравнивание | не гарантировано | **секции по границе страниц 4096** |

## Заголовок (128 байт, little-endian)

```text
СМЕЩ  РАЗМ  ПОЛЕ
0x00  8     magic "PQW2NN\0\0"
0x08  4     version u32 = 2
0x0C  4     header_size u32 = 128
0x10  1     model_type: 0=encoder, 1=decoder, 2=span-ner
0x11  1     quant: 0=fp32, 1=int8, 2=int4 (режим весов Linear)
0x12  2     flags: bit0=bias, bit1=xlmr-позиции, bit2=MoE
0x14  4     num_layers u32
0x18  4     hidden u32
0x1C  4     intermediate u32
0x20  4     heads u32
0x24  4     head_dim u32 (= hidden/heads)
0x28  4     vocab u32
0x2C  4     max_pos u32
0x30  4     num_experts u32 (0 = плотный FFN)
0x34  4     top_k u32 (MoE-маршрутизация)
0x38  4     kv_heads u32 (1 = MQA, heads = MHA)
0x3C  4     reserved = 0
0x40  8     table_offset u64 (выравнен на 4096)
0x48  8     table_len u64
0x50  8     payload_len u64 (байты секций данных)
0x58  8     file_len u64
0x60  32    sha256([4096 .. table_offset+table_len)) — payload + таблица
```

## Макет файла

```text
0x0000  заголовок (128 Б)
0x0080  …паддинг до страницы…
0x1000  секции данных тензоров — каждая выравнена на 4096:
        [int8-коды | int4-nibble | f32 | raw] без заголовков внутри
…       таблица тензоров (тоже на границе страницы)
EOF
```

Выравнивание на страницу — это ноль-copy mmap: тензорные срезы
отдаются прямо из страниц файла, ядро подгружает их по касанию
(лениво). Для GLM-70B это и есть «SSD streaming»: в RAM живут только
затронутые страницы, MoE-роутер не читает невыбранных экспертов.

## Запись таблицы (последовательность записей)

```text
u16 name_len | name utf8 | u8 dtype | u8 ndims | u16 reserved
u32 scale_count | u64 dims[ndims] | u64 data_offset | u64 data_len
f32 scales[scale_count]        (только int8/int4; per-row)
```

dtype: `0` = f32, `1` = int8, `2` = int4, `3` = raw (метаданные).

## Квантование (симметричное, на строку выходных каналов)

- **int8**: `scale = max|w_row| / 127`, код ∈ [-127, 127].
  Матвектор: `out[r] = scale_r · Σ q[r,i]·x[i]` — int8-строка
  разворачивается в f32 прямо в AVX2-регистрах
  (`cvtepi8_epi32 → cvtepi32_ps → fmadd`); активации остаются fp32
  (weight-only). Плотность: 4 Б/вес → 1 Б/вес + 4 Б/строка.
- **int4**: `scale = max|w_row| / 7`, код ∈ [-7, 7], упаковка два
  nibble на байт (`lo = байт & 0xF`, `hi = байт >> 4`, значение =
  `nibble − 8`). Плотность: 0.5 Б/вес — режим GLM-декодера (6B ≈ 4 ГБ).

Погрешности (дифференциальные тесты, синтетика):
int8 — cos > 0.999; int4 — cos > 0.98 (физика 4-бит).

## Именование тензоров

Энкодер (BERT/XLM-R-класс — BGE-M3, SPLADE):

```text
word_embeddings        [vocab, hidden]
position_embeddings    [max_pos, hidden]
token_type_embeddings  [2, hidden]            (опционально)
embeddings_ln_gamma|beta [hidden]
layers.{i}.attn_{q,k,v,o}_w  [out, in]
layers.{i}.attn_{q,k,v,o}_b  [out]            (опционально)
layers.{i}.attn_ln_gamma|beta
layers.{i}.ffn_up_w    [intermediate, hidden]
layers.{i}.ffn_down_w  [hidden, intermediate]
layers.{i}.ffn_{up,down}_b                   (опционально)
layers.{i}.ffn_ln_gamma|beta
final_ln_gamma|beta                         (опционально)
__tokenizer__        RAW — Unigram-токенизатор (см. ниже, BGE-M3)
```

Декодер (GLM-класс):

```text
word_embeddings        [vocab, hidden]
layers.{i}.attn_q_w    [heads·hd, hidden]
layers.{i}.attn_k_w    [kv_heads·hd, hidden]   (MQA: kv_heads=1)
layers.{i}.attn_v_w
layers.{i}.attn_o_w    [hidden, heads·hd]
layers.{i}.attn_norm_gamma [hidden]
layers.{i}.ffn_norm_gamma  [hidden]
layers.{i}.ffn_gate_w  [intermediate, hidden]  (SwiGLU gate)
layers.{i}.ffn_up_w    [intermediate, hidden]
layers.{i}.ffn_down_w  [hidden, intermediate]
# MoE-вариант FFN:
layers.{i}.moe_gate_w  [experts, hidden]       (f32, маленький)
layers.{i}.experts.{e}.ffn_{gate,up,down}_w
final_norm_gamma       [hidden]
lm_head_w              [vocab, hidden]         (нет → tied с эмбеддингами)
```

GLiNER (model_type=2) добавляет голову:

```text
span_width_emb  [max_width, hidden]
span_proj_w     [num_labels, 3·hidden]
__labels__      raw utf-8, метки через '\n'
```

## Верификация

`QuantizedWeightsView::open`:
1. магия + версия + длина файла;
2. разбор таблицы с проверкой границ каждой записи;
3. контроль выравнивания секций (4096) и размеров по dtype;
4. **SHA-256 по [4096 .. table_offset+table_len)** — подменённый байт
   ловится ДО инференса (тест `sha256_tamper_detected`, самотест [6/6]).

## Инструменты

- `scripts/convert_hf_to_pqw.py` — **конвертер реальных весов** (E.8):
  torch-zip `pytorch_model.bin` → `.pqw` int8/int4 потоково (блоки строк,
  RAM не растёт с моделью), XLM-R-маппинг имён, встраивание токенизатора.
  Требует numpy. Проверен на BAAI/bge-m3: 573 МБ, послойный дифференциал
  с fp32-эталоном cos ≥ 0.9999, семантика 0.75/0.29 (совпадает с fp32).
- `scripts/extract_tokenizer_data.py` — снятие таблицы нормализации и
  золотых токенизаций с `tokenizers` (HF, эталон).
- `scripts/gen_nfc_tables.py` — регенерация `src/pqc/nfc_tables.rs`.
- `PqwBuilder` (`src/pqc/pqw.rs`) — сборка файла в RAM (синтетика,
  тесты).
- `poler-engine --pqw-selftest` — полный автономный цикл: генерация
  синтетических моделей → mmap+SHA-256 → инференс → сверка с эталоном.

## Секция `__tokenizer__` (RAW, v1)

Модель самодостаточна — токенизатор живёт внутри `.pqw`:

```text
"TOKR" u16 version=1 u8 algo(0=unigram) u8 flags(bit0=add_prefix_space)
u32 unk_id bos_id eos_id pad_id mask_id (0xFFFFFFFF = нет)
u32 vocab_size;   ×N { u16 len, bytes, f32 score }
u32 specials_count; ×N { u16 len, bytes, u32 id }
u32 norm_count;   ×N { u32 codepoint, u16 len, bytes }
```

Конвейер кодирования (портирован с `tokenizers` v0.23.2, сверен на 40
текстах — 0 расхождений): raw-split по спец-токенам → нормализация
(per-codepoint таблица = точная семантика Precompiled charsmap, затем
NFC-композиция по `nfc_tables.rs`, затем коллапс `' {2,}'` → `' '`) →
метаспейс (`' '` → `▁`, префикс `▁`) → Unigram-Viterbi (DP по байтам,
unk = `min_score − 10` за один чар, `fuse_unk` — слияние подряд идущих
unk) → обёртка `[bos, …, eos]`.

## Статус интеграции с POLER-Quantum-RS

Ядро `pqc` (тензор/контейнер/энкодер) живёт в дереве poler-engine
(вендор-модель по канону FSST-кирпича: приватный репозиторий нельзя
git-зависимостью в публичный poler-engine). Вынос в
`crates/pqc-inference` workspace POLER-Quantum-RS — отдельный кирпич
после открытия репозитория.
