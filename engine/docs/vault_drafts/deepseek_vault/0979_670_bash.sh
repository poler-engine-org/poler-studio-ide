# Удаляем старые конфиги
rm -rf ~/.config/pipewire ~/.config/wireplumber

# Создаем папки
mkdir -p ~/.config/pipewire/pipewire.conf.d
mkdir -p ~/.config/wireplumber

# Минимальный конфиг PipeWire
cat > ~/.config/pipewire/pipewire.conf.d/99-simple.conf << 'EOF'
context.properties = {
default.clock.quantum = 1024
default.clock.rate = 48000
}
EOF

# Минимальный конфиг WirePlumber
cat > ~/.config/wireplumber/main.lua.d/99-simple.lua << 'EOF'
rule = {
matches = {
{
{ "node.nick", "equals", "ASUS VK222H" },
},
},
apply_properties = {
["priority.driver"] = 1000,
["node.pause-on-idle"] = false,
},
}
EOF

# Перезапускаем
systemctl --user restart pipewire wireplumber
