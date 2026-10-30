# Установка средств проверки
sudo pacman -S rkhunter chkrootkit

# Обновление баз rkhunter и проверка системы
sudo rkhunter --update
sudo rkhunter --check --skip-keypress

# Запуск chkrootkit
sudo chkrootkit
