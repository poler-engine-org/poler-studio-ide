#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Markdown Structure & LaTeX-safe Preprocessor for Technical Translation."""

import re
import sys
import argparse
from pathlib import Path

class MarkdownMasker:
    def __init__(self):
        self.placeholders = {}
        self.counter = 0

    def _store(self, match_str):
        key = f"__PROTECTED_BLOCK_{self.counter}__"
        self.placeholders[key] = match_str
        self.counter += 1
        return key

    def mask(self, text: str) -> str:
        # 1. Защита блоков кода (``` ... ```)
        text = re.sub(r'```[\s\S]*?```', lambda m: self._store(m.group(0)), text)
        
        # 2. Защита инлайн-кода (` ... `)
        text = re.sub(r'`[^`\n]+`', lambda m: self._store(m.group(0)), text)
        
        # 3. Защита блочных формул ($$ ... $$)
        text = re.sub(r'\$\$[\s\S]*?\$\$', lambda m: self._store(m.group(0)), text)
        
        # 4. Защита инлайн-формул ($ ... $)
        text = re.sub(r'\$[^\$\n]+\$', lambda m: self._store(m.group(0)), text)
        
        # 5. Защита URL и путей в ссылках [text](url)
        def mask_link(m):
            visible_text = m.group(1)
            url_part = m.group(2)
            masked_url = self._store(url_part)
            return f"[{visible_text}]({masked_url})"
        
        text = re.sub(r'\[([^\]]+)\]\(([^)]+)\)', mask_link, text)
        return text

    def unmask(self, text: str) -> str:
        for key, orig in reversed(list(self.placeholders.items())):
            text = text.replace(key, orig)
        return text

def main():
    parser = argparse.ArgumentParser(description="Markdown Masking/Unmasking Tool for Translation")
    parser.add_argument("--mode", choices=["mask", "unmask"], required=True)
    parser.add_argument("--input", required=True, help="Input markdown file")
    parser.add_argument("--output", required=True, help="Output file")
    parser.add_argument("--map-file", default="mask_map.json", help="JSON map for unmasking")
    args = parser.parse_args()

    input_path = Path(args.input)
    content = input_path.read_text(encoding="utf-8")

    masker = MarkdownMasker()
    if args.mode == "mask":
        masked = masker.mask(content)
        Path(args.output).write_text(masked, encoding="utf-8")
        import json
        Path(args.map_file).write_text(json.dumps(masker.placeholders, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"Masked {len(masker.placeholders)} technical blocks -> {args.output}")
    else:
        import json
        masker.placeholders = json.loads(Path(args.map_file).read_text(encoding="utf-8"))
        unmasked = masker.unmask(content)
        Path(args.output).write_text(unmasked, encoding="utf-8")
        print(f"Restored {len(masker.placeholders)} technical blocks -> {args.output}")

if __name__ == "__main__":
    main()
