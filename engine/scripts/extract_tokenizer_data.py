#!/usr/bin/env python3
"""Экстрактор данных XLM-R-токенизатора из tokenizer.json (HF fast).

Выход (коммитится в репозиторий рядом с конвертером):
  scripts/xlmr_norm_table.json  — per-codepoint таблица нормализации
                                  (точная семантика Precompiled charsmap:
                                  1 codepoint → строка-замена)
  tests/fixtures/tokenizer_golden.json — золотые тексты → id (эталон:
                                  библиотека `tokenizers`, Rust-реализация
                                  HF; дифференциал для нативного Rust)

Требует: pip install tokenizers numpy (только здесь, не в рантайме poler).
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))

GOLDEN_TEXTS = [
    "Проклятые княжества Нокс: магия и код",
    "функция grep ищет текст в файлах",
    "The quick brown fox jumps over the lazy dog",
    "semantic search engine for code and documents",
    "Київ — столиця України",
    "Poler Engine — AI-Native Search",
    "Rust компилируется в нативный бинарник без зависимостей",
    "int8 квантование весов, mmap zero-copy, SHA-256 верификация",
    "Два  пробела   подряд\tи табуляция\nперевод строки",
    "numbers: 3.14159, 2.718281828, 1e-9, 0xDEADBEEF, 42",
    "snake_case_identifier and CamelCaseClassName and CONST_VALUE",
    "fn main() { println!(\"hello, world\"); }",
    "SELECT * FROM users WHERE id = 17 AND name LIKE '%nox%';",
    "имя файла: Княжества_Нокс_глава_3.txt",
    "α β γ λ μ σ π Δ Σ Ω — греческий алфавит",
    "ⰀⰁⰂⰃ глаголица — редкая письменность",
    "中文文本测试 — китайские иероглифы",
    "日本語のテキスト処理",
    "emoji 🚀🔥 and symbols ©®™ §¶†‡",
    "ligature: ﬁle ﬂow ofﬁce — NFKC декомпозиция",
    "fullwidth: ＡＢＣ ａｂｃ １２３ ＧＬＭ",
    "non-breaking space and　ideographic space",
    "х — неизвестный символ: ⟒⊑⊓⌬⌭",
    "CXVIII, MDCCCLXXXVIII, Ⅻ римские цифры",
    "½ ¼ ¾ ⅓ ⅔ дроби и ²³ верхние индексы",
    "zero-width​space внутри слова",
    "  leading and trailing spaces  ",
    "single",
    "",
    "a",
    "▁literal replacement char in text",
    "Київ та Львів, Одеса і Харків — міста України",
    "vector embeddings cosine similarity HNSW RaBitQ",
    "struct Encoder { hidden: usize, layers: usize }",
    "x = (y * z) / (w + v) - q ^ r",
    "The\n    indented\n        code\n    block",
    "многоточие… и тире — и кавычки «ёлочки»",
    "naïve café résumé Zürich façade",
    "АБВГДЕЁЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯ",
    "абвгдеёжзийклмнопрстуфхцчшщъыьэюя",
]


def main():
    tok_path = sys.argv[1] if len(sys.argv) > 1 else os.environ.get(
        "TOKENIZER_JSON", "/home/z/my-project/hf/bge-m3/tokenizer.json")
    out_dir = sys.argv[2] if len(sys.argv) > 2 else HERE

    from tokenizers import Tokenizer
    tk = Tokenizer.from_file(tok_path)
    norm = tk.normalizer

    # 1. Таблица нормализации: все codepoints, где normalize(c) != c.
    table = {}
    for cp in range(0x110000):
        if 0xD800 <= cp <= 0xDFFF:
            continue
        ch = chr(cp)
        out = norm.normalize_str(ch)
        if out != ch:
            table[str(cp)] = out
    with open(os.path.join(out_dir, "xlmr_norm_table.json"), "w", encoding="utf-8") as f:
        json.dump(table, f, ensure_ascii=False, separators=(",", ":"))
    print(f"norm table: {len(table)} codepoints")

    # 2. Золотые тексты → ids (+ tokens для отладки).
    golden = []
    for text in GOLDEN_TEXTS:
        enc = tk.encode(text)
        golden.append({"text": text, "ids": enc.ids})
    fixture_path = os.path.join(ROOT, "tests", "fixtures", "tokenizer_golden.json")
    os.makedirs(os.path.dirname(fixture_path), exist_ok=True)
    with open(fixture_path, "w", encoding="utf-8") as f:
        json.dump(golden, f, ensure_ascii=False, indent=1)
    print(f"golden: {len(golden)} текстов → {fixture_path}")

    # 3. Метаданные модели Unigram (для проверки конвертера).
    tj = json.load(open(tok_path, encoding="utf-8"))
    m = tj["model"]
    meta = {
        "type": m["type"],
        "unk_id": m.get("unk_id"),
        "byte_fallback": m.get("byte_fallback"),
        "vocab_size": len(m["vocab"]),
        "pre_tokenizer": tj["pre_tokenizer"],
        "bos": 0, "eos": 2, "pad": 1, "mask": 250001,
    }
    with open(os.path.join(out_dir, "xlmr_tok_meta.json"), "w", encoding="utf-8") as f:
        json.dump(meta, f, ensure_ascii=False, indent=1)
    print("meta:", meta)


if __name__ == "__main__":
    main()
