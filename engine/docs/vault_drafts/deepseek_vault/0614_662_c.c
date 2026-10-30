// ΔΣ — изменение структуры (сумма квадратов изменений амплитуд)
float delta_structure = 0.0f;
for (int i = 0; i < sys->soliton_count; i++) {
float diff = sys->solitons[i].amplitude - sys->prev_amplitudes[i];
delta_structure += diff * diff;
}
// ΔI — изменение свободной энергии (можно взять разницу логарифма вероятности правильного ответа)
float delta_information = ... ; // вычисляется из метрики качества

float T = (fabsf(delta_information) > 1e-6) ? delta_structure / delta_information : 1.0f;
sys->emergent_time += T;
