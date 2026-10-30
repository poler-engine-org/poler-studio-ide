// ИНИЦИАЛИЗАЦИЯ - Xavier + Box-Muller для нормального распределения
void semantic_encoder_init(SemanticEncoder* enc) {
float scale = sqrtf(2.0f / (256.0f + SEMANTIC_DIM));  // Xavier

for (int b = 0; b < 256; b++) {
for (int d = 0; d < SEMANTIC_DIM; d++) {
// Box-Muller transform для нормального распределения
float z = sqrtf(-2.0f * logf(u1)) * cosf(2π * u2);
enc->byte_embeddings[b * SEMANTIC_DIM + d] = z * scale;
}
}

// Sinusoidal position encoding (как в transformer)
for (pos, d) {
angle = pos / pow(10000, 2*d/SEMANTIC_DIM);
position[pos,d] = sin(angle) или cos(angle);
}
}

// КОДИРОВАНИЕ bytes → vectors
SemanticSequence* semantic_encode(bytes, len) {
num_vectors = (len + 7) / 8;  // 8 байтов на вектор

for each vector:
1. Суммируем эмбеддинги байтов
2. Усредняем по chunk_len
3. Добавляем position encoding
4. L2 нормализуем  ← КЛЮЧ! (исправило loss с 15M до 0.002)

return seq;
}

// ДЕКОДИРОВАНИЕ vectors → bytes (lossless)
int semantic_decode(seq, output) {
// ПОКА ЗАГЛУШКА - возвращает сохранённые original bytes
// TODO: обучить реальный декодер
for each vector:
output += sv->original[i];
}
