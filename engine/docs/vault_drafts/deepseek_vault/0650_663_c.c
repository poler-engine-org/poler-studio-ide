// В language_core.c:
static void create_links(LanguageCore* lang, ...) {
System* sys = &lang->base;
int h_start = lang->hidden_zone_start;
int h_cnt = lang->hidden_zone_count;
int i_start = lang->input_zone_start;
int i_cnt = lang->input_zone_count;
int o_start = lang->output_zone_start;
int o_cnt = lang->output_zone_count;

// Вход -> Скрытый
for (int i = 0; i < i_cnt; i++) {
for (int h = 0; h < h_cnt; h++) {
if ((rand() % 100) < input_density) {
float w = (rand() / (float)RAND_MAX) * 0.5f;
if (rand() % 2) w = -w;
sys_add_link(sys, i_start + i, h_start + h, w, 1 + rand() % 3);
}
}
}

// Скрытый -> Скрытый (рекуррентные)
for (int h1 = 0; h1 < h_cnt; h1++) {
for (int h2 = 0; h2 < h_cnt; h2++) {
if (h1 != h2 && (rand() % 100) < recurrent_density) {
float w = (rand() / (float)RAND_MAX) * 0.3f;
if (rand() % 2) w = -w;
sys_add_link(sys, h_start + h1, h_start + h2, w, 1 + rand() % MAX_DELAY);
}
}
}

// Скрытый -> Выход
for (int h = 0; h < h_cnt; h++) {
for (int o = 0; o < o_cnt; o++) {
if ((rand() % 100) < output_density) {
float w = (rand() / (float)RAND_MAX) * 0.4f;
if (rand() % 2) w = -w;
sys_add_link(sys, h_start + h, o_start + o, w, 1 + rand() % 3);
}
}
}

// Тормозные WTA внутри выходной зоны
for (int o1 = 0; o1 < o_cnt; o1++) {
for (int o2 = 0; o2 < o_cnt; o2++) {
if (o1 != o2 && (rand() % 100) < 30) {
sys_add_link(sys, o_start + o1, o_start + o2, -0.2f, 1);
}
}
}
}
2. НАСТОЯЩИЙ STDP С УЧЁТОМ ВРЕМЕНИ
c
