#!/bin/bash
set -e

echo "🔧 Установка Skill System..."

# Проверяем зависимости
command -v python3 >/dev/null 2>&1 || { echo "❌ Python3 не найден"; exit 1; }
command -v tesseract >/dev/null 2>&1 || { echo "⚠️  Tesseract не найден, установи: sudo apt install tesseract-ocr"; }
command -v pdftoppm >/dev/null 2>&1 || { echo "⚠️  Poppler не найден, установи: sudo apt install poppler-utils"; }

# Устанавливаем Python-пакеты
pip3 install --user jsonschema reportlab pypdf pillow

# Копируем файлы
mkdir -p ~/.local/share/skill-system
cp -r skills orchestrator.py ~/.local/share/skill-system/

# Создаём алиас
echo 'alias skill="python3 ~/.local/share/skill-system/orchestrator.py"' >> ~/.bashrc
source ~/.bashrc
