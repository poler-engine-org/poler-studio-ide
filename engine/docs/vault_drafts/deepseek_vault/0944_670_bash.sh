context.properties = {
# Увеличиваем буфер в 8 раз от стандартного (стандарт 64/128)
default.clock.quantum = 1024
default.clock.min-quantum = 1024
default.clock.max-quantum = 1024
default.clock.rate = 48000
# Приоритет для вашего HDMI
default.priority.session = 2048
}

context.modules = [
{ name = libpipewire-module-rtkit
args = {
