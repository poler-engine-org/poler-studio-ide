// encode_symbol — создаёт внешнее поле (гауссов пакет) в латентном пространстве
void soliton_encode(SolitonLang* lang, unsigned char ch) {
// Получаем центр для данного символа из таблицы
float* center = lang->symbol_centers[ch];
// Создаём внешнее поле как сумму гауссиан вокруг центра
for (int i = 0; i < LATENT_DIM; i++) {
        float dx = i - center[i]; // упрощённо
lang->sys.external_field[i] = 3.0f * expf(-dx*dx / 20.0f);
}
}

// decode — выбираем символ с максимальной амплитудой солитона в выходном уровне
unsigned char soliton_decode(SolitonLang* lang) {
int best = 0;
float max_amp = -1e9;
HierarchyLevel* out = &lang->sys.levels[OUTPUT_LEVEL];
for (int i = out->first; i < out->first + out->count; i++) {
if (lang->sys.solitons[i].amplitude > max_amp) {
max_amp = lang->sys.solitons[i].amplitude;
best = i - out->first;
}
}
return (unsigned char)best;
}

// шаг обработки
void soliton_step(SolitonLang* lang, unsigned char input_char) {
soliton_encode(lang, input_char);
sys_v3_step(&lang->sys);
}

// обучение — прямое усиление целевого солитона + адаптация взаимодействий
void soliton_learn(SolitonLang* lang, unsigned char target_char) {
HierarchyLevel* out = &lang->sys.levels[OUTPUT_LEVEL];
int target_idx = out->first + target_char;

// Усиливаем амплитуду целевого солитона (внешнее «учительское» поле)
lang->sys.solitons[target_idx].amplitude += 5.0f;

// Вычисляем информационный отклик (ΔI) — насколько улучшилась вероятность
float prob_before = ...; // softmax до усиления
float prob_after = ...;  // softmax после
float delta_I = prob_after - prob_before;

// Адаптируем взаимодействия
adapt_interactions(&lang->sys, delta_I);

// Сохраняем статистику
lang->avg_reward = lang->avg_reward * 0.99f + 0.01f * delta_I;
}
