void adapt_interactions(SystemV3* sys, float delta_information) {
    float eta = 0.01f; // скорость обучения
for (int k = 0; k < sys->interaction_count; k++) {
Interaction* in = &sys->interactions[k];
Soliton* src = &sys->solitons[in->src_soliton];
Soliton* dst = &sys->solitons[in->dst_soliton];

// Значение волновой функции источника в точке приёмника
float psi_src = soliton_influence(src, dst->center);
float psi_dst = dst->amplitude; // упрощение

// Комплексное обновление: Δc = η * ΔI * ψ_src * exp(i·phase_dst)
float complex update = eta * delta_information * psi_src * cexpf(I * dst->phase);
in->coeff += update;

// Ограничение модуля коэффициента
float mag = cabsf(in->coeff);
if (mag > 3.0f) in->coeff *= 3.0f / mag;
}
}
