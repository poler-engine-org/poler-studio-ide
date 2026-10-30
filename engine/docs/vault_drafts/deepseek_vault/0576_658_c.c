// КЛЮЧЕВАЯ КОНЦЕПЦИЯ:
//   V = Σ αᵢ·Aᵢ + R
//   Любой вектор = сумма архетипов + остаток

// ТИПЫ АРХЕТИПОВ (семантические примитивы):
typedef enum {
ARCH_ACTION,     // transfer, pay, send, create, delete
ARCH_AMOUNT,     // числа, количества
ARCH_CURRENCY,   // USD, RUB, EUR
ARCH_RECIPIENT,  // имена, адреса
ARCH_SUBJECT,    // я, ты, он
ARCH_TIME,       // сегодня, завтра
ARCH_LOCATION,   // Москва, home
ARCH_OBJECT,     // деньги, файл
ARCH_ATTRIBUTE,  // быстро, срочно
ARCH_CONDITION,  // если, когда
ARCH_LANG_RU,    // русский язык
ARCH_LANG_EN     // английский язык
} ArchetypeType;

// АРХЕТИП
typedef struct {
char  name[32];
ArchetypeType type;
SemanticVector vector;    // Сам вектор архетипа
float frequency;
int   usage_count;
float gradient[256];      // Для обучения
} Archetype;

// РАЗЛОЖЕНИЕ
typedef struct {
int   archetype_idx[16];   // Индексы активных архетипов
float coefficients[16];    // Коэффициенты α
int   count;
SemanticVector residual;   // R = V - Σ αᵢ·Aᵢ
float residual_norm;
float reconstruction_error;
} ArchetypeDecomposition;

// БИБЛИОТЕКА АРХЕТИПОВ
typedef struct {
Archetype archetypes[32];
int       count;              // 12 по умолчанию
float     centroid[256];
float     learning_rate;
int       total_decompositions;
float     avg_archetypes_used;  // ~3-5 обычно
} ArchetypeLibrary;
