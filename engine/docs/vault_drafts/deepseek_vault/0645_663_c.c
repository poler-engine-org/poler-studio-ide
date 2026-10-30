// Лучше: градиентная награда по softmax
static float compute_reward(LanguageCore* lang, unsigned char target) {
float target_state = lang->base.elements[lang->symtab.decode_map[target]].state;
float sum_exp = 0.0f;

for (int ch = 0; ch < SYMBOL_SET_SIZE; ch++) {
float s = lang->base.elements[lang->symtab.decode_map[ch]].state;
sum_exp += expf(s);
}

float prob = expf(target_state) / sum_exp;  // вероятность правильного символа
return prob - 0.5f;  // награда от -0.5 до +0.5
}
