# Экспортируем реестр Wine
export WINEPREFIX="/run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559/GenshinWine"

# Создадим reg-файл для настройки диска D:
cat > ~/setup_drive_d.reg << 'EOF'
REGEDIT4

[HKEY_CURRENT_USER\Software\Wine\Drives]
"D:"="z:\\run\\media\\vitalij\\8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559"

[HKEY_CURRENT_USER\Software\Wine\Drives\D:]
"Path"="z:\\run\\media\\vitalij\\8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559"
"Type"="hd"
"Serial"=hex:00,00,00,00
"Label"="Games"
EOF

# Применим reg-файл
wine regedit ~/setup_drive_d.reg
