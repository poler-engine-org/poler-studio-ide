# Создаем конфиг PulseAudio
mkdir -p ~/.config/pulse
cat > ~/.config/pulse/default.pa << 'EOF'
# Увеличиваем размер буфера
load-module module-udev-detect tsched_buffer_size=1024 tsched_buffer_watermark=512

# Явно указываем наше HDMI устройство
load-module module-alsa-sink device=hw:1,3 fragments=4 fragment_size=1024

# Отключаем приостановку устройств
load-module module-suspend-on-idle timeout=0
EOF

# Перезапускаем PulseAudio
pulseaudio -k
pulseaudio --start --log-level=debug
