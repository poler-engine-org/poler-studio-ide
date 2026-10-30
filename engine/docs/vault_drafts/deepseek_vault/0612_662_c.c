void level_collapse(SystemV3* sys, int level_idx) {
HierarchyLevel* lv = &sys->levels[level_idx];
for (int i = lv->first; i < lv->first + lv->count; i++) {
Soliton* s = &sys->solitons[i];
if (s->amplitude >= lv->collapse_threshold) {
// Создаём солитон на следующем уровне (или усиливаем существующий)
// Этот процесс — аналог композиции паттернов.
// Для простоты: просто передаём энергию в связанные солитоны верхнего уровня.
for (int k = 0; k < sys->interaction_count; k++) {
Interaction* in = &sys->interactions[k];
if (in->src_soliton == i && sys->solitons[in->dst_soliton].level == level_idx+1) {
// усиление амплитуды целевого солитона
sys->solitons[in->dst_soliton].amplitude += crealf(in->coeff) * s->amplitude;
}
}
// Сбрасываем амплитуду (коллапс)
s->amplitude = 0.0f;
}
}
}
