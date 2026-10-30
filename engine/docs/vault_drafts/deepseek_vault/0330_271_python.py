import numpy as np
import struct

def read_raw_tensor(filepath, dtype='f4', shape=None):
# dtype: 'f4' = float32, 'f2' = float16, 'i4' = int32
with open(filepath, 'rb') as f:
data = f.read()
# Распаковываем в массив чисел
count = len(data) // 4  # для float32
numbers = struct.unpack(f'<{count}f', data)  # < = little-endian
if shape:
return np.array(numbers).reshape(shape)
return np.array(numbers)

# Пример: читаем первый попавшийся тензор
import glob
raw_files = glob.glob("./extracted_tensors/*.raw")
if raw_files:
tensor_data = read_raw_tensor(raw_files[0])
