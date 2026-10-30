function switch_to_hdmi()
-- Ищем устройство с именем "ASUS VK222H"
for _, node in ipairs(wp_objects) do
if node.type == "Sink" and node.name == "alsa_output.pci-0000_01_00.1.hdmi-stereo" then
-- Устанавливаем это устройство как устройство по умолчанию
wp_call("set-default-node", { node.id })
break
end
end
end

-- Запускаем функцию при инициализации WirePlumber
switch_to_hdmi()
