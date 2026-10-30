# Создаем конфигурацию ALSA
sudo tee /etc/asound.conf << 'EOF'
defaults.pcm.card 1
defaults.pcm.device 3
defaults.ctl.card 1

pcm.!default {
type plug
slave.pcm "hdmi"
}

pcm.hdmi {
type hw
card 1
device 3
rate 48000
channels 2
}

ctl.!default {
type hw
card 1
}
EOF

# Теперь настройте приложения использовать ALSA напрямую
