DEST=~/fpga_all_files_flat
mkdir -p "$DEST"

find . -type f \( ... \) | while read -r file; do
# все расширения из списка выше подставьте в скобки
newname=$(echo "$file" | sed 's|^\./||' | tr '/' '_')
cp "$file" "$DEST/$newname"
done
