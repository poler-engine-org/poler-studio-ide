# Отключаем PipeWire
systemctl --user disable --now pipewire pipewire-pulse wireplumber

# Включаем PulseAudio
systemctl --user enable --now pulseaudio pulseaudio.socket

# Проверяем
pactl info | grep "Server Name"
Б) Настройка PulseAudio (более стабильно для старых устройств):
bash
