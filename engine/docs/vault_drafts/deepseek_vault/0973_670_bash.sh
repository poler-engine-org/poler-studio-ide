# Переименуем наши конфиги, чтобы они не мешали
mv ~/.config/pipewire ~/.config/pipewire_backup 2>/dev/null
mv ~/.config/wireplumber ~/.config/wireplumber_backup 2>/dev/null

# Восстанавливаем системные конфиги
rm -rf ~/.config/pipewire
cp -r /etc/pipewire ~/.config/

# Перезапускаем все
systemctl --user daemon-reload
systemctl --user restart pipewire pipewire-pulse wireplumber
systemctl --user --no-pager status pipewire
Проверяем, что всё запустилось:
bash
