# Декодер обучен восстанавливать байты из вектора
reconstructed_bytes = decoder(next_vector)  # [16] — 4 символа × 4 байта

# Выбор языка на выходе — это параметр декодера!
decoder.language = 'ru'
print(reconstructed_bytes.decode('utf-32-le'))  # "500 "

decoder.language = 'en'
print(reconstructed_bytes.decode('utf-32-le'))  # "500 "
