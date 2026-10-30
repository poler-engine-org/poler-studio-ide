// Было:
lang_v2_step(lang, seq[i]);
lang_v2_learn(lang, seq[i+1]);

// Стало:
lang_v2_train_step(lang, seq[i], seq[i+1]);
