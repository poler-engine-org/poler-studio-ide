context.properties = {
# Увеличиваем буфер для стабильности
default.clock.quantum = 1024
default.clock.min-quantum = 1024
default.clock.max-quantum = 1024
default.clock.rate = 48000
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
]

Перезапустите PipeWire:

bash
