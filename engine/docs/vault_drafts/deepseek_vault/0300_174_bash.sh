# Установка
pip install ruff

# Запуск анализа (вывод всех ошибок)
ruff check my_script.py

# Автоматическое исправление всех возможных ошибок
ruff check --fix my_script.py

# Автоформатирование кода по стандарту PEP 8 (как Black)
ruff format my_script.py
