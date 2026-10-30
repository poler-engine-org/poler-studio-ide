void lang_v2_train_step(LanguageCoreV2* lang, unsigned char input_char, unsigned char target_char) {
SystemV2* sys = &lang->base;

// ------------------------------------------------------------
// 1. ПРЯМОЙ ПРОХОД (БЕЗ TEACHER FORCING)
// ------------------------------------------------------------
// Кодируем вход
encode_char(lang, input_char);
// Запускаем полный иерархический цикл
sys_v2_step_hierarchical(sys);

// ------------------------------------------------------------
// 2. СОХРАНЯЕМ СОСТОЯНИЯ ДЛЯ ГРАДИЕНТА
// ------------------------------------------------------------
Zone* hidden_zone = &sys->zones[lang->hidden_zone_id];
Zone* output_zone = &sys->zones[lang->output_zone_id];

// Копируем скрытый слой (нормализованный)
float sum_h = 0.0f;
for (int i = 0; i < hidden_zone->count; i++) {
int idx = hidden_zone->start + i;
lang->cached_hidden[i] = sys->elements[idx].state;
if (lang->cached_hidden[i] < 0) lang->cached_hidden[i] = 0;
sum_h += lang->cached_hidden[i];
}
// Нормализация (важно для стабильности градиентов)
if (sum_h > 1e-6f) {
for (int i = 0; i < hidden_zone->count; i++)
lang->cached_hidden[i] /= sum_h;
}

// ------------------------------------------------------------
// 3. ВЫЧИСЛЯЕМ SOFTMAX И КРОСС-ЭНТРОПИЮ
// ------------------------------------------------------------
// Находим максимальное значение в выходном слое (для численной стабильности)
float max_val = -1e9f;
for (int i = 0; i < output_zone->count; i++) {
int idx = output_zone->start + i;
if (sys->elements[idx].state > max_val)
max_val = sys->elements[idx].state;
}

// Вычисляем экспоненты и сумму
float sum_exp = 0.0f;
float exp_vals[256]; // SYMBOL_SET_SIZE
for (int i = 0; i < output_zone->count; i++) {
int idx = output_zone->start + i;
exp_vals[i] = expf(sys->elements[idx].state - max_val);
sum_exp += exp_vals[i];
}

// Вероятности (softmax)
float probs[256];
for (int i = 0; i < output_zone->count; i++)
probs[i] = exp_vals[i] / sum_exp;

// Целевой one-hot
int target_idx = lang->symtab.decode_map[target_char] - output_zone->start;

// Кросс-энтропия (только для мониторинга)
float loss = -logf(probs[target_idx] + 1e-10f);

// ------------------------------------------------------------
// 4. ВЫЧИСЛЯЕМ ГРАДИЕНТ ПО ВЫХОДНОМУ СЛОЮ
// ------------------------------------------------------------
// dL/dy_i = probs[i] - (i == target_idx ? 1 : 0)
float grad_output[256];
for (int i = 0; i < output_zone->count; i++) {
grad_output[i] = probs[i] - (i == target_idx ? 1.0f : 0.0f);
}

// ------------------------------------------------------------
// 5. ОБНОВЛЯЕМ ВЕСА HIDDEN → OUTPUT
// ------------------------------------------------------------
int ho_links = 0;
float grad_norm = 0.0f;

for (int k = 0; k < sys->link_count; k++) {
LinkV2* ln = &sys->links[k];

// Проверяем, что связь идёт из hidden в output
int src_in_hidden = (ln->src >= hidden_zone->start &&
ln->src < hidden_zone->start + hidden_zone->count);
int dst_in_output = (ln->dst >= output_zone->start &&
ln->dst < output_zone->start + output_zone->count);

if (src_in_hidden && dst_in_output) {
int h_idx = ln->src - hidden_zone->start;
int o_idx = ln->dst - output_zone->start;

// Градиент: ∂L/∂w = grad_output[o_idx] * cached_hidden[h_idx]
float grad = grad_output[o_idx] * lang->cached_hidden[h_idx];

// Добавляем L2-регуляризацию
grad += lang->weight_decay * ln->weight;

// Momentum
            int vel_idx = ho_links; // можно использовать индекс связи
lang->velocity_ho[vel_idx] = lang->momentum * lang->velocity_ho[vel_idx] +
lang->learning_rate * grad;

// Обновление веса
ln->weight -= lang->velocity_ho[vel_idx];

// Накопление нормы градиента (для адаптивной скорости)
grad_norm += grad * grad;

ho_links++;
}
}

// ------------------------------------------------------------
// 6. АДАПТИВНАЯ НОРМАЛИЗАЦИЯ ГРАДИЕНТА (опционально)
// ------------------------------------------------------------
if (ho_links > 0) {
grad_norm = sqrtf(grad_norm / ho_links);
// Скользящее среднее
lang->grad_norm_ema = 0.99f * lang->grad_norm_ema + 0.01f * grad_norm;

// Если норма слишком мала/велика, корректируем learning rate
if (lang->grad_norm_ema > 0.1f) {
            lang->learning_rate *= 0.99f;  // плавно уменьшаем
} else if (lang->grad_norm_ema < 0.01f) {
            lang->learning_rate *= 1.01f;   // плавно увеличиваем
}

// Ограничиваем
if (lang->learning_rate > 0.1f) lang->learning_rate = 0.1f;
if (lang->learning_rate < 1e-5f) lang->learning_rate = 1e-5f;
}

// ------------------------------------------------------------
// 7. ОБНОВЛЯЕМ ВЕСА INPUT → HIDDEN (опционально, для автоэнкодера)
// ------------------------------------------------------------
// Здесь можно добавить аналогичный проход для обучения входных весов,
// если мы хотим, чтобы скрытый слой обучался полезным признакам.
// Один из способов: реконструировать вход с помощью декодера.
// Для простоты пока пропускаем.

// ------------------------------------------------------------
// 8. СБРАСЫВАЕМ TEACHER FORCING (если применяли)
// ------------------------------------------------------------
// В данном алгоритме мы НЕ используем teacher forcing, поэтому не требуется.
}
📊 ПРЕИМУЩЕСТВА ПЕРЕД STDP И TEACHER FORCING
Критерий	STDP + Teacher boost	Vector Gradient Learning (VGL)
