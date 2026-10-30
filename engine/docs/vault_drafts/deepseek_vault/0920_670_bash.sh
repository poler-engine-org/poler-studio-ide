pacmd list-sinks | grep -A 10 "alsa_output.pci" | grep -E "name:|device.description"

Создайте конфиг для PulseAudio:

bash
