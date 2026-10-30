# Упаковываем всё в .tar
tar -cf my_weights.tar ./checkpoints/

# Разбиваем на части по 10 ГБ (можешь поставить 50ГБ, если разрешено)
split -b 10G my_weights.tar my_weights_part_

# Получаем файлы: my_weights_part_aa, ab, ac, ...
# Их можно заливать по одному через curl/rclone.
