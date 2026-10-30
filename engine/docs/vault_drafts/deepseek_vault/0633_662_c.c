unsigned char lang_v2_decode(LanguageCoreV2* lang) {
SystemV2* sys = &lang->base;
Zone* out = &sys->zones[lang->output_zone_id];

// Softmax
float max_val = -1e9f;
for (int i = 0; i < out->count; i++) {
float v = sys->elements[out->start + i].state;
if (v > max_val) max_val = v;
}
float sum_exp = 0.0f;
for (int i = 0; i < out->count; i++) {
float e = expf(sys->elements[out->start + i].state - max_val);
sum_exp += e;
}

int best = 0;
float best_prob = -1.0f;
for (int i = 0; i < out->count; i++) {
float prob = expf(sys->elements[out->start + i].state - max_val) / sum_exp;
if (prob > best_prob) {
best_prob = prob;
best = i;
}
}
return (unsigned char)best;
}
