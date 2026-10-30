# Дадим полные права на папку префикса
sudo chown -R vitalij:vitalij "/run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559/GenshinWine"
sudo chmod -R 755 "/run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559/GenshinWine"

# Перезапустим Wine
wineserver -k
