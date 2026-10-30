# Встановити Weston та Waydroid
sudo pacman -S weston waydroid

# Завантажити модулі ядра (якщо ще не)
sudo modprobe binder_linux devices="binder,hwbinder,vndbinder"
sudo modprobe ashmem_linux

# Ініціалізувати Waydroid (завантажить образи Android)
sudo waydroid init

# Запустити Weston (відкриється нове вікно)
weston --socket=waydroid-weston &

Після запуску Weston у новому терміналі всередині Weston виконайте:

bash
