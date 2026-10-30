#!/bin/bash
OUTPUT="POLER_DYNAMIS_full.txt"
> "$OUTPUT"
find . -type f ! -name "*.so*" ! -name "*.zip" ! -name "*.tar" ! -name "*.jpg" ! -name "*.png" ! -name "*.pyc" ! -name "POLER_DYNAMIS_full.txt" | sort | while read -r f; do
echo "=== $f ===" >> "$OUTPUT"
cat "$f" >> "$OUTPUT"
echo >> "$OUTPUT"
done
