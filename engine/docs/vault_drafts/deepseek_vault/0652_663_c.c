static float compute_reward(LanguageCore* lang, unsigned char target) {
int target_idx = lang->symtab.decode_map[target];
float target_state = lang->base.elements[target_idx].state;

// softmax по всем выходным элементам
float max_state = target_state;
for (int ch = 0; ch < SYMBOL_SET_SIZE; ch++) {
int idx = lang->symtab.decode_map[ch];
if (lang->base.elements[idx].state > max_state)
max_state = lang->base.elements[idx].state;
}

// Смещаем, чтобы избежать экспоненциального взрыва
float sum_exp = 0.0f;
for (int ch = 0; ch < SYMBOL_SET_SIZE; ch++) {
int idx = lang->symtab.decode_map[ch];
sum_exp += expf(lang->base.elements[idx].state - max_state);
}
float prob = expf(target_state - max_state) / sum_exp;

// Награда в диапазоне [-0.5, 0.5]
return (prob - 0.5f) * 2.0f;  // -> [-1, 1]
}

В lang_learn заменить:

c
