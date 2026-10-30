# подивитись, скільки місця займають логи
journalctl --disk-usage
# залишити логи тільки за останні 3 дні
sudo journalctl --vacuum-time=3d
🗑 Очищення тимчасових файлів
bash
