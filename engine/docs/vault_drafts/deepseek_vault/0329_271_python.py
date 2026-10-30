import os
import json
import struct
from pathlib import Path

# 1. Открываем файл в бинарном режиме
file_path = "model.safetensors"
output_dir = "./extracted_tensors"

# Создаём папку для разобранных частей
Path(output_dir).mkdir(exist_ok=True)

with open(file_path, "rb") as f:
# --- Читаем заголовок ---
# Первые 8 байт - это длина JSON-заголовка (в байтах)
# struct.unpack('<Q', ...) читает 8 байт как 64-битное целое (little-endian)
header_len = struct.unpack('<Q', f.read(8))[0]
    print(f"Длина JSON-заголовка: {header_len} байт")

# Читаем сам JSON
header_bytes = f.read(header_len)
header_json = json.loads(header_bytes.decode('utf-8'))

# --- Смотрим, что внутри ---
# Убираем служебные ключи (типа __metadata__), если они есть
tensor_meta = {k: v for k, v in header_json.items() if not k.startswith('__')}
    print(f"Найдено тензоров: {len(tensor_meta)}")

# --- Вырезаем каждый тензор в отдельный файл ---
for tensor_name, meta in tensor_meta.items():
# Где лежит этот тензор в файле?
offsets = meta['data_offsets']
start_byte = offsets[0]
        end_byte = offsets[1]   # не включительно
size_bytes = end_byte - start_byte

# Информация о форме и типе
shape = meta['shape']
dtype = meta['dtype']
        print(f"  Извлечение: {tensor_name} | форма: {shape} | тип: {dtype} | размер: {size_bytes:,} байт")

# Перемещаем курсор в начало этого тензора
f.seek(start_byte)

# Читаем ровно столько байт, сколько занимает тензор
raw_data = f.read(size_bytes)

# Сохраняем сырые данные в отдельный файл
# Заменяем "/" и "." в имени, чтобы создать нормальную папку/файл
safe_name = tensor_name.replace("/", "_").replace(".", "_")
out_path = Path(output_dir) / f"{safe_name}.raw"

with open(out_path, "wb") as out_f:
out_f.write(raw_data)

# Дополнительно сохраняем мета-информацию (shape, dtype) в JSON
meta_path = Path(output_dir) / f"{safe_name}.meta.json"
with open(meta_path, "w") as mf:
json.dump({"name": tensor_name, "shape": shape, "dtype": dtype}, mf, indent=2)
