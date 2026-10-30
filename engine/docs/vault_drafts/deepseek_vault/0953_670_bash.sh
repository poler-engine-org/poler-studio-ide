#!/bin/bash
# Находим точное имя HDMI устройства
HDMI_SINK=$(pactl list short sinks | grep "alsa_output.pci-0000_01_00.1" | grep "hdmi" | cut -f2)

if [ -n "$HDMI_SINK" ]; then
    echo "Устанавливаю HDMI устройство: $HDMI_SINK"
pactl set-default-sink "$HDMI_SINK"

# Перемещаем все текущие потоки на HDMI
for input in $(pactl list short sink-inputs | cut -f1); do
pactl move-sink-input "$input" "$HDMI_SINK"
done

    echo "Готово. Текущий дефолтный выход:"
pactl info | grep "Default Sink"
else
