# Клонуємо репозиторій PKGBUILD
git clone https://aur.archlinux.org/rclone-ui-bin.git
cd rclone-ui-bin

# Перевіряємо вміст PKGBUILD (не обов'язково)
cat PKGBUILD

# Збираємо та встановлюємо пакет
makepkg -si
