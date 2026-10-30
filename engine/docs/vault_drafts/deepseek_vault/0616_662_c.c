typedef struct {
// Массивы солитонов и взаимодействий
Soliton* solitons;
int soliton_capacity;
int soliton_count;

Interaction* interactions;
int interaction_capacity;
int interaction_count;

// Иерархические уровни
HierarchyLevel levels[MAX_LEVELS];
int level_count;

// Латентное пространство
int latent_dim;
    float* metric;      // когнитивная метрика (LATENT_DIM x LATENT_DIM)

// Внешнее поле (вход)
float* external_field; // [LATENT_DIM]

// Эмерджентное время
float emergent_time;
float prev_amplitudes[]; // для вычисления ΔΣ (flexible array)

// Энергетический бюджет (скалярная кривизна)
float curvature;

// Статистика
int epoch;
float avg_reward;
} SystemV3;

Функции шага:

c
