// ============================================================================
// semantic_encoder.h - Lossless Semantic Autoencoder
// ============================================================================

#define SEMANTIC_DIM 256        // Размерность семантических векторов
#define MAX_SEQUENCE 512        // Максимальная длина последовательности
#define CHUNK_SIZE 8            // Байтов на один вектор

// Семантический вектор
typedef struct {
float v[SEMANTIC_DIM];              // Компоненты вектора
int   byte_len;                     // Длина исходных байтов (1-8)
unsigned char original[CHUNK_SIZE]; // Для lossless восстановления
} SemanticVector;

// Последовательность векторов
typedef struct {
SemanticVector* vectors;
int   count;
int   capacity;
float total_bytes;
float compression_ratio;            // bytes/vectors
} SemanticSequence;

// Энкодер
typedef struct {
float* byte_embeddings;     // [256 * SEMANTIC_DIM] - эмбеддинги байтов
float* position_encoding;   // [MAX_SEQUENCE * SEMANTIC_DIM]
float* pattern_dict;        // [1000 * SEMANTIC_DIM]
int    pattern_count;
} SemanticEncoder;

// Inline функции:
// - semantic_vector_zero()    - обнулить вектор
// - semantic_vector_add()     - сложить векторы
// - semantic_vector_scale()   - масштабировать
// - semantic_vector_dot()     - скалярное произведение
// - semantic_vector_cosine()  - косинусная близость
