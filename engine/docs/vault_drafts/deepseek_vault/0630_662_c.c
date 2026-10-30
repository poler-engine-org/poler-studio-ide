lang->learning_rate = 0.01f;
lang->momentum = 0.9f;
lang->weight_decay = 1e-4f;
lang->grad_norm_ema = 0.0f;
lang->cached_hidden = malloc(hidden_count * sizeof(float));
lang->cached_hidden_size = hidden_count;
lang->velocity_ho_size = hidden_count * output_count;
lang->velocity_ho = calloc(lang->velocity_ho_size, sizeof(float));
🔄 ОСНОВНОЙ ЦИКЛ ОБУЧЕНИЯ (ВМЕСТО lang_v2_learn)
c
