# Удалим swap.img (4.1 ГБ) если ещё не удалили
sudo rm -f "/run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559/swap.img"

# Посмотрим, что можно удалить в папке q-e (3.8 ГБ)
ls -la "/run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559/q-e"

# Если там не важные данные, удаляем
sudo rm -rf "/run/media/vitalij/8464fd5c-cb5b-4dca-bb66-3e2f0f3cf559/q-e"
