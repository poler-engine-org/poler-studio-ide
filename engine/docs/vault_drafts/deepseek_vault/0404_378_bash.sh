#!/bin/bash
OUTPUT="POLER_DYNAMIS_full.md"
echo "# POLER-DYNAMIS v5 повний текст" > "$OUTPUT"
find . -type f \( -name "*.py" -o -name "*.rs" -o -name "*.toml" -o -name "*.md" -o -name "*.sh" -o -name "*.h" -o -name "*.c" -o -name "*.dat" -o -name "*.lock" \) | sort | while read -r f; do
echo "## $f" >> "$OUTPUT"
echo '```' >> "$OUTPUT"
cat "$f" >> "$OUTPUT"
echo >> "$OUTPUT"
echo '```' >> "$OUTPUT"
echo >> "$OUTPUT"
done
