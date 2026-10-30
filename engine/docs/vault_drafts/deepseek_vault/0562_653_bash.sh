#!/bin/bash

# Выходной файл
OUTPUT="merged_code.txt"

# Очищаем выходной файл перед записью (или создаём новый)
> "$OUTPUT"

# Рекурсивно обходим все файлы (кроме самого скрипта и выходного файла)
find . -type f ! -name "$(basename "$0")" ! -name "$OUTPUT" -print0 | while IFS= read -r -d '' file; do
# Проверяем, текстовый ли файл (опционально)
if file "$file" | grep -q text; then
        echo "Добавляю: $file"
# Записываем разделитель с именем файла
echo -e "\n\n===== $file =====\n" >> "$OUTPUT"
# Добавляем содержимое файла
cat "$file" >> "$OUTPUT"
else
        echo "Пропускаю бинарный файл: $file"
fi
done
