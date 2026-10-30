[Unit]
Description=Keep HDMI Audio awake
After=pipewire.service

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/bin/bash -c "while true; do amixer -c 1 sset 'HDMI/DP,pcm=3' 1%+ >/dev/null 2>&1; sleep 30; done"

[Install]
WantedBy=default.target

Включите и запустите:

bash
