#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Structural Parity Verifier between Original and Translated Technical Markdown."""

import re
import sys
import argparse
from pathlib import Path

def analyze_structure(text: str) -> dict:
    code_fences = len(re.findall(r'```[\s\S]*?```', text))
    inline_code = len(re.findall(r'`[^`\n]+`', text))
    math_blocks = len(re.findall(r'\$\$[\s\S]*?\$\$', text))
    inline_math = len(re.findall(r'\$[^\$\n]+\$', text))
    links = len(re.findall(r'\[([^\]]+)\]\(([^)]+)\)', text))
    headers = len(re.findall(r'^#{1,6}\s+', text, re.MULTILINE))
    tables = len(re.findall(r'^\|.*\|$', text, re.MULTILINE))
    
    return {
        "code_fences": code_fences,
        "inline_code": inline_code,
        "math_blocks": math_blocks,
        "inline_math": inline_math,
        "links": links,
        "headers": headers,
        "table_rows": tables,
    }

def main():
    parser = argparse.ArgumentParser(description="Verify structural parity of technical translation")
    parser.add_argument("--orig", required=True, help="Original Markdown file")
    parser.add_argument("--trans", required=True, help="Translated Markdown file")
    args = parser.parse_args()

    orig_text = Path(args.orig).read_text(encoding="utf-8")
    trans_text = Path(args.trans).read_text(encoding="utf-8")

    s_orig = analyze_structure(orig_text)
    s_trans = analyze_structure(trans_text)

    print("═════════════════════════════════════════════════════════════")
    print(" 📊 Сравнение структурного паритета оригинала и перевода")
    print("═════════════════════════════════════════════════════════════")
    all_ok = True
    for key, count_orig in s_orig.items():
        count_trans = s_trans.get(key, 0)
        status = "✅ OK" if count_orig == count_trans else f"⚠️ DIFF (orig={count_orig}, trans={count_trans})"
        if count_orig != count_trans:
            all_ok = False
        print(f" {key:<15} : {status}")
    print("═════════════════════════════════════════════════════════════")
    if all_ok:
        print("🎉 100% Структурная целостность формул, ссылок и кода соблюдена!")
    else:
        print("⚠️ Внимание: есть расхождения в количестве блоков разметки.")

if __name__ == "__main__":
    main()
