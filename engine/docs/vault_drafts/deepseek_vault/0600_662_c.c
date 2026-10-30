// ============================================================================
// language_core_v2.h
//
// Language Processing Core v2.0
// Using hierarchical constraint system with proper signal flow
// ============================================================================

#ifndef LANGUAGE_CORE_V2_H
#define LANGUAGE_CORE_V2_H

#include "system_core_v2.h"

// ------------------------------------------------
// Constants
// ------------------------------------------------
#define SYMBOL_SET_SIZE   256
#define ENCODE_GROUP_SIZE   8
#define TRACE_WINDOW       20

// ------------------------------------------------
// Symbol table
// ------------------------------------------------
typedef struct {
int* encode_map;    // [SYMBOL_SET_SIZE * ENCODE_GROUP_SIZE]
int* decode_map;    // [SYMBOL_SET_SIZE]
} SymbolTableV2;

// ------------------------------------------------
// Plasticity (STDP learning)
// ------------------------------------------------
typedef struct {
float** pre_trace;      // [element_count][TRACE_WINDOW]
int*    trace_ptr;      // [element_count]
float   learning_rate;
float   modulation;

// Stats
int     total_ltp;
int     total_ltd;
} PlasticityV2;

// ------------------------------------------------
// Language Core v2
// ------------------------------------------------
typedef struct {
SystemV2       base;
SymbolTableV2  symtab;
PlasticityV2   plast;

// Zone IDs
int input_zone_id;
int hidden_zone_id;
int output_zone_id;

// Stats
int   epoch;
float last_reward;
float avg_reward;
int   correct_predictions;
int   total_predictions;
} LanguageCoreV2;

// ------------------------------------------------
// Functions
// ------------------------------------------------
void lang_v2_init(LanguageCoreV2* lang,
int hidden_size,
int input_density,
int recurrent_density,
int output_density,
float learning_rate);

void lang_v2_step(LanguageCoreV2* lang, unsigned char input_char);
void lang_v2_learn(LanguageCoreV2* lang, unsigned char target_char);
void lang_v2_reset(LanguageCoreV2* lang);
void lang_v2_free(LanguageCoreV2* lang);

unsigned char lang_v2_decode(LanguageCoreV2* lang);
float lang_v2_compute_reward(LanguageCoreV2* lang, unsigned char target);
void lang_v2_debug(LanguageCoreV2* lang, unsigned char target);
void lang_v2_print_stats(LanguageCoreV2* lang);

#endif
