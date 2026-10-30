# Фиксируем устройство вывода
set-default-sink alsa_output.pci-0000_01_00.1.hdmi-stereo

# Увеличиваем размер буфера (помогает против пропадания)
load-module module-alsa-sink device=hw:1,3 fragments=4 fragment_size=1024

Сохраните и перезапустите PulseAudio:

bash
