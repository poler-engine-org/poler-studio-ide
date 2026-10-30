# Удалим временный swap.img если ещё не удалили
sudo rm -f "/run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559/swap.img"

# Проверим реальное свободное место
df -h /dev/sdc6

# Создадим папку для игры на корне раздела
mkdir -p "/run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559/GenshinImpact"

# В установщике выберите путь:
# Z:\run\media\vitalij\8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559\GenshinImpact
3. Альтернатива: используем диск D: который мы создавали
bash
