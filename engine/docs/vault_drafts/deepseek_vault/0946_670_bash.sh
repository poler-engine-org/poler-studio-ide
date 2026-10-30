context.modules = [
{ name = libpipewire-module-protocol-pulse
args = {
# Увеличиваем буферы для PulseAudio-совместимости
pulse.min.req = 1024/48000     # ~21 мс
pulse.default.req = 1024/48000
pulse.max.req = 1024/48000
pulse.min.frag = 1024/48000
pulse.default.frag = 1024/48000
pulse.max.frag = 1024/48000
# Отключаем некоторые оптимизации
server.address = [ "unix:native" ]
}
}
]
3. Перезапуск PipeWire
bash
