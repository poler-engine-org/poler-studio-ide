if file --mime-type "$file" | grep -qE 'application/|binary'; then
    echo "⚠️ Пропускаємо бінарний файл: $file" >&2
continue
fi
