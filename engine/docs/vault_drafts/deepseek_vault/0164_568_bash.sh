# подивитись поточний розмір
journalctl --disk-usage

# залишити логи тільки за останні 3 дні
sudo journalctl --vacuum-time=3d

# або обмежити максимальний розмір (наприклад, 200 МБ)
sudo journalctl --vacuum-size=200M
5️⃣ Очищення тимчасових файлів
bash
