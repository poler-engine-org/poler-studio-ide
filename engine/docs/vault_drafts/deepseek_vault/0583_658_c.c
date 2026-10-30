typedef struct {
float weights[256 * 64];  // 256 → 64
float bias1[64];
float weights2[64 * 32];  // 64 → 32
float bias2[32];
    float weights3[32 * 64];  // 32 → 64 (8 байт * 8 бит)
float bias3[64];
} SemanticDecoder;

void decoder_forward(decoder, vector, output_bytes[8]) {
// h1 = relu(vector * W1 + b1)
// h2 = relu(h1 * W2 + b2)
// logits = h2 * W3 + b3  (64 нейрона)
// output_bytes[i] = argmax over 8 classes for each byte position
}

Вариант Б (рекуррентный, для вариативной длины):

c
