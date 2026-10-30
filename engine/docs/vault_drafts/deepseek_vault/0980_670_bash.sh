# Устанавливаем JACK
sudo pacman -S jack2 jack2-dbus

# Останавливаем PipeWire/PulseAudio
systemctl --user stop pipewire pulseaudio

# Запускаем JACK
jack_control start
jack_control ds alsa
jack_control dps device hw:1,3
jack_control dps rate 48000
jack_control dps nperiods 3
jack_control dps period 1024

# Проверяем
jack_lsp
Для подключения приложений к JACK:
bash
