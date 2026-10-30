# Создадим виртуальный диск VHD на HDD
cd "/run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559"
dd if=/dev/zero of=virtual_c_drive.img bs=1G count=120

# Примонтируем как loop устройство
sudo losetup /dev/loop1 virtual_c_drive.img
sudo mkfs.ext4 /dev/loop1

# Монтируем
sudo mkdir /mnt/virtual_c
sudo mount /dev/loop1 /mnt/virtual_c

# Переносим Wine префикс на виртуальный диск
sudo cp -r "/run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559/GenshinWine" /mnt/virtual_c/

# Обновляем префикс
export WINEPREFIX="/mnt/virtual_c/GenshinWine"
