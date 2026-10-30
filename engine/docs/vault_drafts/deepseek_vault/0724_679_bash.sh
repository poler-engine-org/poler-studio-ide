# Проверьте, есть ли уникальные пакеты в .venv (кроме общих)
comm -23 <(sort .venv_packages.txt) <(sort venv_packages.txt)
# Проверьте, есть ли уникальные пакеты в venv
comm -13 <(sort .venv_packages.txt) <(sort venv_packages.txt)
