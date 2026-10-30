# Для .venv
.venv/bin/pip list --format=freeze > .venv_packages.txt

# Для venv
venv/bin/pip list --format=freeze > venv_packages.txt

# Для cass_sim
cass_sim/bin/pip list --format=freeze > cass_sim_packages.txt

После этого вы можете сравнить полученные файлы:

bash
