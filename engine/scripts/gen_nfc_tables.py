#!/usr/bin/env python3
"""Генератор src/pqc/nfc_tables.rs — данные для канонической композиции.

NFC = канонический порядок (по ccc) + попарная композиция (starter+mark).
Таблицы извлекаются из unicodedata Python (Unicode 15.x) эмпирически:
композиционная пара (a,b)→c валидна ⟺ NFC(a+b) == c. Hangul — алгоритмически.
"""
import sys
import unicodedata

OUT = sys.argv[1] if len(sys.argv) > 1 else \
    "/home/z/my-project/poler-engine/src/pqc/nfc_tables.rs"

MAX_CP = 0x110000

# 1. Канонические комбинирующие классы (ccc != 0)
ccc = []
for cp in range(MAX_CP):
    if 0xD800 <= cp <= 0xDFFF:
        continue
    cl = unicodedata.combining(chr(cp))
    if cl != 0:
        ccc.append((cp, cl))

# 2. Композиционные пары: cp с канонической декомпозицией из 2 чаров,
#    где NFC(декомпозиция) == cp (не исключение из композиции)
comp = []
for cp in range(MAX_CP):
    if 0xD800 <= cp <= 0xDFFF:
        continue
    ch = chr(cp)
    d = unicodedata.decomposition(ch)
    if not d or d.startswith("<"):
        continue  # нет декомпозиции или compatibility (неканоническая)
    parts = d.split()
    if len(parts) != 2:
        continue
    a, b = int(parts[0], 16), int(parts[1], 16)
    if unicodedata.normalize("NFC", chr(a) + chr(b)) == ch:
        comp.append((a, b, cp))

# сортировка по (starter, mark) — под бинарный поиск в Rust
comp.sort(key=lambda t: (t[0], t[1]))

with open(OUT, "w", encoding="utf-8") as f:
    f.write("//! Сгенерировано scripts/gen_nfc_tables.py (unicodedata Python,\n")
    f.write("//! Unicode 15.x). НЕ редактировать руками — регенерация скриптом.\n")
    f.write("//! Данные канонической композиции для NFC-прохода токенизатора\n")
    f.write("//! (src/pqc/tokenizer.rs): таблица не зависит от модели.\n\n")
    f.write("/// Канонические комбинирующие классы: (codepoint, ccc), ccc != 0.\n")
    f.write("/// Отсортировано по codepoint — бинарный поиск.\n")
    f.write("pub static CCC: &[(u32, u8)] = &[\n")
    for cp, cl in ccc:
        f.write(f"    (0x{cp:X}, {cl}),\n")
    f.write("];\n\n")
    f.write("/// Композиционные пары (starter, mark) → composed.\n")
    f.write("/// Отсортировано по (starter, mark) — бинарный поиск.\n")
    f.write("pub static COMPOSITIONS: &[(u32, u32, u32)] = &[\n")
    for a, b, c in comp:
        f.write(f"    (0x{a:X}, 0x{b:X}, 0x{c:X}),\n")
    f.write("];\n")

print(f"ccc: {len(ccc)} записей, композиций: {len(comp)}")
