# Активируйте .venv и попробуйте запустить простой импорт
source .venv/bin/activate && python -c "import numba; print(numba.__version__)" && deactivate
# Сделайте то же для venv
source venv/bin/activate && python -c "import numba; print(numba.__version__)" && deactivate
