# Смотрим ошибки
journalctl --user -u pipewire -n 50 --no-pager
journalctl --user -u wireplumber -n 50 --no-pager

# Запускаем вручную для отладки
pipewire --version
pipewire-pulse --version
