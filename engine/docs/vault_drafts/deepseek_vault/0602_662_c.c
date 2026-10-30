// ============================================================================
// main_v2.c - Test Hierarchical Constraint System v2.0
//
// This version uses POLER/RPN-inspired hierarchical processing:
//   Phase 1: Input activation
//   Phase 2: Input -> Hidden propagation + Hidden spikes
//   Phase 3: Hidden -> Output propagation + Output activation
// ============================================================================

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "language_core_v2.h"

// Test sequences
const char* SEQ_SIMPLE = "abcabcabcabcabcabcabcabcabcabc";
const char* SEQ_COMPLEX = "abababacabababacabababacabababac";
const char* SEQ_TEXT = "the cat sat on the mat the dog ran in the park the cat sat on the mat the dog ran in the park";

// ------------------------------------------------
// Train on sequence
// ------------------------------------------------
float train_on_sequence_v2(LanguageCoreV2* lang, const char* seq,
int print_progress, int debug_first) {
int len = strlen(seq);
int correct = 0;

for (int i = 0; i < len - 1; i++) {
lang_v2_step(lang, (unsigned char)seq[i]);
lang_v2_learn(lang, (unsigned char)seq[i + 1]);

if (debug_first && i == 0) {
lang_v2_debug(lang, (unsigned char)seq[i + 1]);
}

unsigned char pred = lang_v2_decode(lang);
if (pred == (unsigned char)seq[i + 1]) correct++;
}

return (float)correct / (len - 1);
}

// ------------------------------------------------
// Generate sequence
// ------------------------------------------------
void generate_sequence_v2(LanguageCoreV2* lang, const char* prompt, int length) {
printf("Prompt: \"%s\"\n", prompt);
printf("Generated: \"");

for (int i = 0; i < strlen(prompt); i++) {
lang_v2_step(lang, (unsigned char)prompt[i]);
}

for (int i = 0; i < length; i++) {
unsigned char next = lang_v2_decode(lang);
if (next > 31 && next < 127) putchar(next);
else putchar('?');
fflush(stdout);
lang_v2_step(lang, next);
}
printf("\"\n");
}

// ------------------------------------------------
// Experiment 1: Simple pattern
// ------------------------------------------------
void experiment_simple_v2(void) {
printf("\n+============================================================+\n");
printf("|  EXPERIMENT 1 (v2): Simple pattern (abcabc...)             |\n");
printf("+============================================================+\n\n");

LanguageCoreV2 lang;
lang_v2_init(&lang, 50, 30, 15, 40, 0.3f);

printf("Training sequence: \"%s\"\n\n", SEQ_SIMPLE);

for (int epoch = 0; epoch < 50; epoch++) {
lang_v2_reset(&lang);
float accuracy = train_on_sequence_v2(&lang, SEQ_SIMPLE, 0, epoch == 0);
lang.epoch = epoch + 1;

printf("Epoch %2d: accuracy = %.1f%%, avg_reward = %.4f\n",
epoch + 1, accuracy * 100, lang.avg_reward);

if (accuracy > 0.95f) {
printf("Early stopping at epoch %d\n", epoch + 1);
break;
}
}

printf("\n--- Generation Test ---\n");
lang_v2_reset(&lang);
generate_sequence_v2(&lang, "abc", 12);

lang_v2_print_stats(&lang);
lang_v2_free(&lang);
}

// ------------------------------------------------
// Experiment 2: Complex pattern
// ------------------------------------------------
void experiment_complex_v2(void) {
printf("\n+============================================================+\n");
printf("|  EXPERIMENT 2 (v2): Complex pattern (abababac...)          |\n");
printf("+============================================================+\n\n");

LanguageCoreV2 lang;
lang_v2_init(&lang, 100, 40, 40, 40, 0.03f);

printf("Training sequence: \"%s\"\n\n", SEQ_COMPLEX);

for (int epoch = 0; epoch < 30; epoch++) {
lang_v2_reset(&lang);
float accuracy = train_on_sequence_v2(&lang, SEQ_COMPLEX, 0, 0);
lang.epoch = epoch + 1;

printf("Epoch %2d: accuracy = %.1f%%, avg_reward = %.4f\n",
epoch + 1, accuracy * 100, lang.avg_reward);

if (accuracy > 0.90f) {
printf("Early stopping at epoch %d\n", epoch + 1);
break;
}
}

printf("\n--- Generation Test ---\n");
lang_v2_reset(&lang);
generate_sequence_v2(&lang, "abab", 16);

lang_v2_print_stats(&lang);
lang_v2_free(&lang);
}

// ------------------------------------------------
// Experiment 3: Real text
// ------------------------------------------------
void experiment_text_v2(void) {
printf("\n+============================================================+\n");
printf("|  EXPERIMENT 3 (v2): Real text                              |\n");
printf("+============================================================+\n\n");

LanguageCoreV2 lang;
lang_v2_init(&lang, 150, 30, 30, 30, 0.02f);

printf("Training text: \"%s\"\n\n", SEQ_TEXT);

for (int epoch = 0; epoch < 40; epoch++) {
lang_v2_reset(&lang);
float accuracy = train_on_sequence_v2(&lang, SEQ_TEXT, 0, epoch == 0);
lang.epoch = epoch + 1;

if (epoch % 5 == 0 || accuracy > 0.5f) {
printf("Epoch %2d: accuracy = %.1f%%, avg_reward = %.4f\n",
epoch + 1, accuracy * 100, lang.avg_reward);
}

if (accuracy > 0.75f) {
printf("Early stopping at epoch %d\n", epoch + 1);
break;
}
}

printf("\n--- Generation Test ---\n");
lang_v2_reset(&lang);
generate_sequence_v2(&lang, "the cat ", 30);

lang_v2_print_stats(&lang);
lang_v2_free(&lang);
}

// ------------------------------------------------
// Debug test: verify signal propagation
// ------------------------------------------------
void test_signal_propagation(void) {
printf("\n+============================================================+\n");
printf("|  SIGNAL PROPAGATION TEST                                   |\n");
printf("+============================================================+\n\n");

LanguageCoreV2 lang;
lang_v2_init(&lang, 20, 20, 10, 30, 0.0f);

lang.base.debug_mode = 1;

printf("\n--- Testing single character 'a' ---\n");
lang_v2_step(&lang, 'a');

printf("\nAfter processing 'a':\n");
sys_v2_debug_zones(&lang.base);

// Check if output zone has any activity
int o_start = lang.base.zones[lang.output_zone_id].start;
float output_sum = 0.0f;
float output_max = 0.0f;
for (int i = o_start; i < o_start + 256; i++) {
output_sum += lang.base.elements[i].state;
if (lang.base.elements[i].state > output_max) {
output_max = lang.base.elements[i].state;
}
}

printf("\nOutput zone summary: sum=%.4f, max=%.4f\n", output_sum, output_max);

if (output_max > 0.01f) {
printf("\n[SUCCESS] Signal reached output zone!\n");
} else {
printf("\n[FAILURE] Output zone is empty. Signal propagation broken.\n");
}

lang_v2_free(&lang);
}

// ------------------------------------------------
// Main
// ------------------------------------------------
int main(int argc, char** argv) {
srand(time(NULL));

printf("\n====================================================================\n");
printf("    CONSTRAINT SYSTEM v2.0 - HIERARCHICAL PROCESSING                \n");
printf("  Inspired by POLER[n] cyclic architecture and RPN hierarchy        \n");
printf("                                                                    \n");
printf("  Key innovation: Multi-phase step with proper signal flow          \n");
printf("    Phase 1: Input activation                                       \n");
printf("    Phase 2: Input -> Hidden propagation                             \n");
printf("    Phase 3: Hidden spike generation                                \n");
printf("    Phase 4: Hidden -> Output propagation                            \n");
printf("====================================================================\n");

// Always run signal propagation test first
test_signal_propagation();

int experiment = 1;
if (argc > 1) experiment = atoi(argv[1]);

switch (experiment) {
case 1: experiment_simple_v2(); break;
case 2: experiment_complex_v2(); break;
case 3: experiment_text_v2(); break;
default:
experiment_simple_v2();
experiment_complex_v2();
experiment_text_v2();
break;
}

printf("\n====================================================================\n");
printf("  DONE - v2.0 with hierarchical signal propagation                  \n");
printf("====================================================================\n");

return 0;
}
