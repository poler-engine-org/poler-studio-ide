// ============================================================================
// language_core.c
// РЕАЛИЗАЦИЯ ЯЗЫКОВОГО МОДУЛЯ
// ============================================================================

#include "language_core.h"
#include <string.h>
#include <math.h>

// ------------------------------------------------
// ПОСТРОЕНИЕ ТАБЛИЦЫ СИМВОЛОВ (разреженное кодирование)
// ------------------------------------------------
static void init_symbol_table(LanguageCore* lang, int input_start, int output_start) {
lang->input_zone_start = input_start;
lang->output_zone_start = output_start;
lang->output_zone_count = SYMBOL_SET_SIZE;

// Входная зона: ENCODE_GROUP_SIZE элементов на символ
lang->input_zone_count = SYMBOL_SET_SIZE * ENCODE_GROUP_SIZE;

// Выделяем память для карт
lang->symtab.encode_map = (int*)malloc(SYMBOL_SET_SIZE * ENCODE_GROUP_SIZE * sizeof(int));
lang->symtab.decode_map = (int*)malloc(SYMBOL_SET_SIZE * sizeof(int));

// Заполняем encode_map: для каждого символа выбираем ENCODE_GROUP_SIZE различных
// индексов в пределах входной зоны.
for (int ch = 0; ch < SYMBOL_SET_SIZE; ch++) {
// Просто берём подряд идущие элементы
for (int i = 0; i < ENCODE_GROUP_SIZE; i++) {
lang->symtab.encode_map[ch * ENCODE_GROUP_SIZE + i] =
lang->input_zone_start + ch * ENCODE_GROUP_SIZE + i;
}
// Выходная карта: каждый символ привязан к одному элементу в выходной зоне
lang->symtab.decode_map[ch] = lang->output_zone_start + ch;
}
}

// ------------------------------------------------
// СОЗДАНИЕ СВЯЗЕЙ МЕЖДУ ЗОНАМИ
// ------------------------------------------------
static void create_links(LanguageCore* lang,
int input_density, int recurrent_density, int output_density) {
System* sys = &lang->base;
int hidden_start = lang->hidden_zone_start;
int hidden_count = lang->hidden_zone_count;
int input_count = lang->input_zone_count;
int output_start = lang->output_zone_start;
int output_count = lang->output_zone_count;

// 1. Связи вход -> скрытый
for (int i = 0; i < input_count; i++) {
int src = lang->input_zone_start + i;
for (int h = 0; h < hidden_count; h++) {
if ((rand() % 100) < input_density) {
int dst = hidden_start + h;
float w = (rand() / (float)RAND_MAX) * 0.5f;
