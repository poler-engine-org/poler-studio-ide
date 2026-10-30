# Отключаем энергосбережение аудио
options snd_hda_intel power_save=0
options snd_hda_intel power_save_controller=N

# Явно указываем индекс HDMI-выхода (для вашего монитора ASUS VK222H на устройстве 3)
options snd_hda_intel enable=1,1,1,1  # Включаем все 4 HDMI-устройства на карте 1
options snd_hda_intel probe_mask=1,1,1,1

# Пробуем разные тайминги (может помочь с "пропаданием")
options snd_hda_intel position_fix=1
options snd_hda_intel bdl_pos_adj=32
