// Минимальная нейросеть для предсказания следующего вектора
typedef struct {
float W[256 * 512];   // input → hidden
float b1[512];
float W2[512 * 256];  // hidden → output
float b2[256];
} VectorPredictor;

void predictor_forward(predictor, current_vector, predicted_next) {
h = tanh(current_vector * W + b1);
predicted_next = h * W2 + b2;
}
