// ИНТЕГРАЦИОННЫЙ СЛОЙ - соединяет все компоненты

typedef struct {
SemanticEncoder    encoder;      // Layer 0
ArchetypeLibrary   archetypes;   // Layer 1
SystemV3           vgl_core;     // Layer 2 (БЕЗ ИЗМЕНЕНИЙ!)

SVPConfig          config;
SVPMode            mode;         // TRAINING / INFERENCE

SemanticSequence*  input_sequence;
SemanticSequence*  output_sequence;
SVPStats           stats;
} SemanticVectorProcessor;

// КОНФИГУРАЦИЯ
typedef struct {
int   hidden_size;          // 64-128
float learning_rate;        // 0.001-0.01
int   semantic_dim;         // 256
int   max_context_vectors;  // 4-16
SVPLanguage default_lang;   // RU/EN
} SVPConfig;

// ГЛАВНЫЕ ФУНКЦИИ:
void svp_init(svp, config);     // Инициализация
void svp_process(svp, in, out); // Inference
float svp_train_step(svp, in, target); // Training
void svp_generate(svp, prompt, out);   // Generation

// ВНУТРЕННИЕ (inline):
// svp_vectors_to_vgl_input()   - векторы → VGL input
// svp_vgl_output_to_vectors()  - VGL output → векторы
