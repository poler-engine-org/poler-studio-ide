#!/bin/bash
OUTPUT="POLER_DYNAMIS_full_for_NotebookLM.md"
> "$OUTPUT"
echo "# POLER-DYNAMIS v5 — повний текст (усі файли)" >> "$OUTPUT"
echo "" >> "$OUTPUT"

find . -type f \( \
-name "*.py" -o -name "*.rs" -o -name "*.toml" -o \
-name "*.md" -o -name "*.sh" -o -name "*.h" -o \
-name "*.c" -o -name "*.dat" -o -name "*.lock" -o \
-name "*.json" -o -name "*.yaml" -o -name "*.yml" \
\) | sort | while read -r file; do
rel="${file#./}"
echo "## 🗂️ \`$rel\`" >> "$OUTPUT"
echo '```' >> "$OUTPUT"
cat "$file" >> "$OUTPUT"
echo >> "$OUTPUT"
echo '```' >> "$OUTPUT"
echo "" >> "$OUTPUT"
done
