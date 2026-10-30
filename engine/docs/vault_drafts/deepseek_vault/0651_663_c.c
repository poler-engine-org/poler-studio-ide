static void stdp_update(LanguageCore* lang, float reward) {
int window = TRACE_WINDOW;
for (int k = 0; k < lang->base.link_count; k++) {
Link* ln = &lang->base.links[k];

// Ищем последний спайк пре и пост в окне
int t_pre = -1, t_post = -1;
for (int t = 0; t < window; t++) {
if (lang->plast.pre_trace[ln->src][t] > 0) t_pre = t;
if (lang->plast.pre_trace[ln->dst][t] > 0) t_post = t;
}
if (t_pre == -1 || t_post == -1) continue;

        float dt = t_post - t_pre;  // разница времени (такты)
float delta;
if (dt > 0) {
delta = 0.01f * expf(-dt / 10.0f);   // LTP
} else {
delta = -0.005f * expf(dt / 10.0f);  // LTD
}

// След элиджибилити с затуханием
lang->plast.eligibility[k] = lang->plast.eligibility[k] * 0.9f + delta;

// Изменение веса под действием глобальной награды
ln->weight += lang->plast.learning_rate * reward * lang->plast.eligibility[k];

// Клиппинг
if (ln->weight > 2.0f) ln->weight = 2.0f;
if (ln->weight < -2.0f) ln->weight = -2.0f;
}
}
3. НЕПРЕРЫВНАЯ НАГРАДА (вместо бинарной)
c
