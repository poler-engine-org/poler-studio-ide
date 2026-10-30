void level_evolve(SystemV3* sys, int level_idx) {
HierarchyLevel* lv = &sys->levels[level_idx];
for (int i = lv->first; i < lv->first + lv->count; i++) {
Soliton* s = &sys->solitons[i];

// Затухание (диссипация)
s->amplitude *= (1.0f - lv->dissipation);

// Суммарное влияние от всех взаимодействий (только от солитонов нижних уровней)
float sum_influence = 0.0f;
for (int k = 0; k < sys->interaction_count; k++) {
Interaction* in = &sys->interactions[k];
if (in->dst_soliton == i) {
Soliton* src = &sys->solitons[in->src_soliton];
// влияние = |coeff| * ψ_src(позиция dst)
float psi = soliton_influence(src, s->center);
sum_influence += crealf(in->coeff) * psi; // реальная часть
// фаза влияет на будущую фазу dst, но пока упростим
s->phase += cimagf(in->coeff) * psi;      // сдвиг фазы
}
}
s->amplitude += sum_influence;

// Ограничение амплитуды (неустойчивости)
if (s->amplitude > 1.0f) s->amplitude = 1.0f;
if (s->amplitude < 0.0f) s->amplitude = 0.0f;

// Вычисление энергии (E ∝ A²)
s->energy = s->amplitude * s->amplitude;
}
}

Активация (бывший «спайк») происходит, когда амплитуда превышает порог коллапса. Тогда солитон «излучает» возмущение в вышестоящий уровень.

c
