DEST=~/fpga_consolidated_flat
mkdir -p "$DEST"

find . -type f \( -name "*.zig" -o -name "*.md" -o -name "*.pdf" \) | while read -r file; do
newname=$(echo "$file" | sed 's|^\./||' | tr '/' '_')
cp "$file" "$DEST/$newname"
done
