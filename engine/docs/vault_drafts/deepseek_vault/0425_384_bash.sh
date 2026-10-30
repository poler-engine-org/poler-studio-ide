# Перейдіть у базову директорію
cd "/home/vitalij/Документи/POLER-DYNAMIS-v5"

# Створіть один .md файл з усіма файлами з трьох папок
OUTPUT="all_files_combined.md"
echo "# Зведений документ всіх файлів проекту POLER-DYNAMIS-v5" > "$OUTPUT"
echo "" >> "$OUTPUT"
echo "Створено: $(date)" >> "$OUTPUT"
echo "" >> "$OUTPUT"

# Функція для додавання вмісту файлів з певної директорії
process_dir() {
local dir="$1"
if [ -d "$dir" ]; then
        echo "## Директорія: $dir" >> "$OUTPUT"
echo "" >> "$OUTPUT"
find "$dir" -type f -not -name "$OUTPUT" | sort | while read -r file; do
echo "### Файл: \`$file\`" >> "$OUTPUT"
echo '```' >> "$OUTPUT"
            cat "$file" >> "$OUTPUT" 2>/dev/null || echo "[Неможливо прочитати файл]" >> "$OUTPUT"
echo '```' >> "$OUTPUT"
echo "" >> "$OUTPUT"
done
else
        echo "## Директорія не знайдена: $dir" >> "$OUTPUT"
fi
}

# Обробка трьох шляхів
process_dir "/home/vitalij/Документи/POLER-DYNAMIS-v5"
process_dir "/home/vitalij/Документи/POLER-DYNAMIS-v5/poler_rust_core"
process_dir "/home/vitalij/Документи/POLER-DYNAMIS-v5/poler_rust_core/src"
