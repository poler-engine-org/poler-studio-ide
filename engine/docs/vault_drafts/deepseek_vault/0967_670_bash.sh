context.properties = {
# Устанавливаем стандартную частоту дискретизации, которую поддерживает старый монитор
default.clock.rate = 44100
# Увеличиваем буфер для стабильности
default.clock.quantum = 1024
default.clock.min-quantum = 1024
default.clock.max-quantum = 1024
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

4.3. Перезапустим PipeWire:

bash
