// Берём любой текст, кодируем, декодируем → должно совпасть
original = "тестовый текст";
vectors = encode(original);
reconstructed = decode(vectors);
assert(strcmp(original, reconstructed) == 0);

Тест 3: Генерация

c
