#!/bin/bash
echo "Мониторинг звука HDMI. Нажмите Ctrl+C для остановки"
while true; do
echo "--- $(date) ---"
cat /proc/asound/card1/stream0 2>/dev/null | grep -A 5 "HDMI 0"
amixer -c 1 sget 'HDMI/DP,pcm=3' 2>/dev/null | grep -E "Playback|Mono:"
sleep 2
done
