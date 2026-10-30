# Добавляем канал conda-forge (если ещё не добавлен)
conda config --add channels conda-forge
conda config --set channel_priority strict

# Устанавливаем GTK3
conda install gtk3

Эта команда установит пакет gtk3 (версия 3.24.43 для Linux)-
18
-
. Также доступны пакеты gtk2 и gtk4-
.
