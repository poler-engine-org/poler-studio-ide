# Компиляция
gcc -O2 -o constraint_system_v2 system_core_v2.c language_core_v2.c main_v2.c -lm

# Запуск теста распространения сигнала
./constraint_system_v2

# Запуск эксперимента 1 (простой паттерн abcabc...)
./constraint_system_v2 1

# Запуск эксперимента 2 (сложный паттерн)
./constraint_system_v2 2

# Запуск эксперимента 3 (текст)
./constraint_system_v2 3
