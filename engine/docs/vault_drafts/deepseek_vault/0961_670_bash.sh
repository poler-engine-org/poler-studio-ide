# Временное переключение на прямой вывод ALSA
systemctl --user stop pipewire
speaker-test -D plughw:1,3 -c 2 -t sine -f 1000

Если в этом режиме звук стабилен - проблема именно в PipeWire.

❯ pactl info | grep -i "default sink"
Connection failure: Connection refused
pa_context_connect() failed: Connection refused

~
❯
❯ pw-cli ls Node | grep -A 20 "ASUS"
node.nick = "ASUS VK222H"
media.class = "Audio/Sink"
id 52, type PipeWire:Interface:Node/3
object.serial = "54"
object.path = "alsa:acp:PCH:4:playback"
factory.id = "19"
client.id = "42"
device.id = "44"
priority.session = "1009"
priority.driver = "1009"
 		node.description = "Вбудоване аудіо Аналогове стерео"
node.name = "alsa_output.pci-0000_00_1b.0.analog-stereo"
node.nick = "ALC887-VD Analog"
media.class = "Audio/Sink"
id 53, type PipeWire:Interface:Node/3
object.serial = "55"
object.path = "alsa:acp:PCH:0:capture"
factory.id = "19"
client.id = "42"
device.id = "44"
priority.session = "2009"

~
❯
 у мня вообче звук после конфигов пропал
Thought for 33 seconds
