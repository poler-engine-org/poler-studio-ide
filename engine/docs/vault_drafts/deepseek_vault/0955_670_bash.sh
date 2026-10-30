# Тестовый тон через PipeWire напрямую
pw-play -t 1000000 --channel-map=stereo --volume=0.5 /usr/share/sounds/alsa/Front_Center.wav

# Или через ALSA напрямую (в обход PipeWire)
speaker-test -D hw:1,3 -c 2 -t sine -f 1000 -l 0
