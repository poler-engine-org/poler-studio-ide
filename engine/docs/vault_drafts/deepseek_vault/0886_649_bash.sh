# Подивитися, скільки місця займають логи
journalctl --disk-usage

# Очистити логи старіші, ніж, наприклад, 2 дні
sudo journalctl --vacuum-time=2d

# Або обмежити максимальний розмір (наприклад, 100 МБ)
sudo journalctl --vacuum-size=100M
