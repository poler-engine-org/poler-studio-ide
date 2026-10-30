# Проверим свободное место на sdc3 (195 ГБ Windows раздел)
df -h /dev/sdc3

# Создадим префикс на NTFS разделе
export WINEPREFIX="/run/media/vitalij/A052966852964348/GenshinWine"
export WINEARCH=win64
winecfg

# Установим необходимые библиотеки
winetricks -q corefonts vcrun2019 d3dcompiler_47
3. Если на sdc3 больше места, запустим установщик оттуда:
bash
