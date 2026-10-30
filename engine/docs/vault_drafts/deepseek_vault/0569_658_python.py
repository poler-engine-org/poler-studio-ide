# Пропускаем через encoder
latent_vectors_ru = encoder(input_bytes_ru)  # [4, 256]  — 4 вектора по 256 чисел
latent_vectors_en = encoder(input_bytes_en)  # [4, 256]  — тоже 4 вектора!

# Проверяем близость
cosine_similarity(latent_vectors_ru[0], latent_vectors_en[0])  # 0.97 — почти идентичны
АРХЕТИПЫ (обучаемые параметры модели):
python
