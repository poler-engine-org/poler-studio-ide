static inline float soliton_influence(const Soliton* s, const float* point) {
float dx = point[0] - s->center[0];
// ... для всех LATENT_DIM координат
float r2 = dx*dx + ...;
return s->amplitude * expf(-r2 / (2 * s->width * s->width));
}
Зоны (Zone) → HierarchyLevel
c
