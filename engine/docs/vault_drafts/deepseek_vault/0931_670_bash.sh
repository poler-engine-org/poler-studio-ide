context.properties = {
# Увеличиваем квант (размер буфера) для стабильности
default.clock.quantum = 1024
default.clock.min-quantum = 1024
default.clock.max-quantum = 1024
# Фиксируем частоту дискретизации (частота вашего монитора, обычно 48000 или 44100)
default.clock.rate = 48000
# Отключаем энергосбережение
default.power.save = 0
}

context.modules = [
{ name = libpipewire-module-rtkit
args = {
nice.level = -15
rt.prio = 88
rt.time.soft = 200000
rt.time.hard = 200000
}
flags = [ ifexists nofail ]
}
{ name = libpipewire-module-protocol-pulse
args = {
# Увеличиваем размер буфера PulseAudio-совместимого сервера
pulse.min.req = 1024/48000
pulse.default.req = 1024/48000
pulse.max.req = 1024/48000
pulse.min.frag = 1024/48000
pulse.default.frag = 1024/48000
pulse.max.frag = 1024/48000
# Отключаем энергосбережение в модуле PulseAudio
server.daemon_mode = false
server.realtime = true
}
}
]
