#!/usr/bin/env python3
import os
import sys

OUTPUT_FILE = "merged_code.txt"

def is_text_file(filepath):
    """Простая проверка на текстовый файл по расширению или содержимому"""
text_extensions = {'.txt', '.py', '.js', '.html', '.css', '.json', '.xml', '.md', '.c', '.cpp', '.h', '.java', '.sh', '.bat', '.ps1', '.yml', '.yaml', '.ini', '.cfg', '.conf', '.sql', '.php', '.rb', '.go', '.rs', '.swift', '.kt', '.dart'}
ext = os.path.splitext(filepath)[1].lower()
if ext in text_extensions:
return True
# Можно добавить проверку через 'file' команду, но для простоты оставим так
return False

def main():
# Если скрипт лежит не в целевой папке, можно передать путь аргументом
if len(sys.argv) > 1:
root_dir = sys.argv[1]
else:
root_dir = "."

with open(OUTPUT_FILE, 'w', encoding='utf-8') as out:
for root, dirs, files in os.walk(root_dir):
for file in files:
filepath = os.path.join(root, file)
# Пропускаем сам скрипт и выходной файл
if file == os.path.basename(__file__) or file == OUTPUT_FILE:
continue
if is_text_file(filepath):
                    print(f"Добавляю: {filepath}")
out.write(f"\n\n===== {filepath} =====\n\n")
try:
with open(filepath, 'r', encoding='utf-8') as f:
out.write(f.read())
except Exception as e:
                        out.write(f"[[[Ошибка чтения файла: {e}]]]\n")
else:
