DEST=~/fpga_consolidated_versions
mkdir -p "$DEST"

find . -type f \( -name "*.zig" -o -name "*.md" -o -name "*.pdf" \) | while read -r file; do
    version=$(echo "$file" | cut -d'/' -f2)   # берём первую подпапку как версию
basename=$(basename "$file")
cp "$file" "$DEST/${version}_${basename}"
done
