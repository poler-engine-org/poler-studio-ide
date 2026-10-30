// АЛГОРИТМ РАЗЛОЖЕНИЯ (iterative projection)
void archetype_decompose(lib, input, result) {

result->residual = input;  // R = V
result->count = 0;

for iter = 0 to MAX_COEFFICIENTS:
// Найти архетип с максимальной корреляцией
best_idx = argmax_i dot(A[i], residual)

// Порог: |dot| > 0.1
if |best_dot| < 0.1: break

// Добавить к разложению
result->archetype_idx[count] = best_idx
result->coefficients[count] = best_dot  // α = correlation
count++

// Обновить остаток: R = R - α·A
residual -= best_dot * A[best_idx]

result->residual_norm = ||R||
result->reconstruction_error = ||V - reconstruct(result)||
}

// РЕКОНСТРУКЦИЯ
void archetype_reconstruct(lib, decomp, output) {
output = 0
for i in decomp->count:
output += decomp->coefficients[i] * lib->archetypes[decomp->archetype_idx[i]].vector
}
