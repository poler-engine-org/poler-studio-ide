# Проверим, создан ли диск D: в Wine
ls -la "/run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559/GenshinWine/dosdevices/"

# Если нет ссылки d:, создадим её
export WINEPREFIX="/run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559/GenshinWine"
ln -sf "/run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559" "$WINEPREFIX/dosdevices/d:"

# Создадим папку на диске D:
mkdir -p "/run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559/Games/Genshin Impact"

# В установщике выберите: D:\Games\Genshin Impact
