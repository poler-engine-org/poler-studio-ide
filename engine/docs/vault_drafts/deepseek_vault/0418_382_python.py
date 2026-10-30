import ctypes

# Укажите полный путь к файлу
libcint = ctypes.CDLL("/home/vitalij/Завантажене/ппппп/pyscf-master/pyscf/geomopt/libcint/build/libcint.so")

# Проверьте, что библиотека загружена
print("libcint loaded successfully")

Если вы планируете использовать эту библиотеку с PySCF (без компиляции всего PySCF), нужно также указать путь в переменной окружения LD_LIBRARY_PATH:

fish
