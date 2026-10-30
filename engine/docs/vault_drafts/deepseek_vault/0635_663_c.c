// ============================================================================
// system_core.c
// РЕАЛИЗАЦИЯ ЯДРА СИСТЕМЫ ОГРАНИЧЕНИЙ
// ============================================================================

#include "system_core.h"
#include <string.h>
#include <math.h>

// ------------------------------------------------
// БАЗОВАЯ ФУНКЦИЯ ОГРАНИЧЕНИЯ (линейная)
// ------------------------------------------------
static inline float apply_constraint(float dst_state, float weight, float pulse) {
    return dst_state + weight * pulse;   // знак weight определяет возбуждение/торможение
}

// ------------------------------------------------
// ИНИЦИАЛИЗАЦИЯ (случайные связи, нулевые состояния)
// ------------------------------------------------
void sys_init(System* sys, int element_count, int link_count) {
sys->element_count = element_count;
sys->link_count = link_count;

sys->elements = (Element*)calloc(element_count, sizeof(Element));
sys->links = (Link*)calloc(link_count, sizeof(Link));
sys->input_pulses = (float*)calloc(element_count, sizeof(float));
sys->output_pulses = (float*)calloc(element_count, sizeof(float));

for (int i = 0; i < element_count; i++)
sys->elements[i].state = 0.0f;

for (int k = 0; k < link_count; k++) {
sys->links[k].src = rand() % element_count;
sys->links[k].dst = rand() % element_count;
sys->links[k].weight = 0.1f + (rand() / (float)RAND_MAX) * 0.9f;
        if (rand() % 2) sys->links[k].weight *= -1.0f; // половина тормозных
sys->links[k].delay = 1 + rand() % 10;         // 1..10 тактов
sys->links[k].head = 0;
sys->links[k].buffer = (float*)calloc(sys->links[k].delay, sizeof(float));
}

sys->constraint_matrix = NULL;
sys->constraint_strength = 0.3f;
}

// ------------------------------------------------
// ОДИН ТАКТ РАБОТЫ СИСТЕМЫ
// ------------------------------------------------
void sys_step(System* sys) {
// 1. Затухание состояний (диссипация)
for (int i = 0; i < sys->element_count; i++)
sys->elements[i].state *= 0.95f;

// 2. Внешние импульсы от модулей
for (int i = 0; i < sys->element_count; i++)
sys->elements[i].state += sys->input_pulses[i];

// 3. Применение локальных связей (ограничений)
for (int k = 0; k < sys->link_count; k++) {
float pulse = sys->links[k].buffer[sys->links[k].head];
if (pulse > 0.0f) {
sys->elements[sys->links[k].dst].state = apply_constraint(
sys->elements[sys->links[k].dst].state,
sys->links[k].weight,
pulse
);
}
}

// 4. Глобальный оператор ограничения (если задан)
if (sys->constraint_matrix != NULL) {
float* tmp = (float*)alloca(sys->element_count * sizeof(float));
for (int i = 0; i < sys->element_count; i++) {
tmp[i] = 0.0f;
for (int j = 0; j < sys->element_count; j++) {
tmp[i] += sys->constraint_matrix[i * sys->element_count + j]
* sys->elements[j].state;
}
}
for (int i = 0; i < sys->element_count; i++) {
sys->elements[i].state += sys->constraint_strength * tmp[i];
}
}

// 5. Генерация выходных импульсов (пороговая активация)
for (int i = 0; i < sys->element_count; i++) {
if (sys->elements[i].state >= 1.0f) {
