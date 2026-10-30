find . -maxdepth 1 -type f -exec head -n 20 {} \;

Или сначала проверить тип файла и показывать только текстовые:

bash
