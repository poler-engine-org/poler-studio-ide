// В system_core.h
#define DECAY 0.99f        // вместо 0.95
#define THRESHOLD 0.5f     // вместо 1.0

// В language_core.c (create_links)
// Вход -> скрытый: увеличиваем вес
float w = 0.8f + (rand() / (float)RAND_MAX) * 1.2f;  // 0.8..2.0
if (rand() % 3 == 0) w = -w;  // только 30% тормозных

// Скрытый -> скрытый: увеличиваем вес
float w_rec = 0.3f + (rand() / (float)RAND_MAX) * 0.7f; // 0.3..1.0
if (rand() % 3 == 0) w_rec = -w_rec;

// Скрытый -> выход: увеличиваем вес
float w_out = 0.4f + (rand() / (float)RAND_MAX) * 0.8f; // 0.4..1.2
if (rand() % 3 == 0) w_out = -w_out;
