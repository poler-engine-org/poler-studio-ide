# Проверить квант (размер буфера)
pw-top

# Проверить устройство вывода
pactl info | grep -i "default sink"

# Проверить состояние вашего HDMI устройства
pw-cli ls Node | grep -A 20 "ASUS"
