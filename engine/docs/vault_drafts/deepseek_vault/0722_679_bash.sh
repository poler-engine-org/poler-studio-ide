# Простой просмотр для быстрой оценки
cat .venv_packages.txt venv_packages.txt cass_sim_packages.txt | sort | uniq -d

# Более наглядное сравнение (нужно установить `diff-so-fancy`)
diff -u .venv_packages.txt venv_packages.txt | diff-so-fancy
