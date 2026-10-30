// ============================================================================
// language_core_v2.c
//
// Language Processing Core v2.0
// Using hierarchical constraint system with proper signal flow
// ============================================================================

#include "language_core_v2.h"
#include <string.h>
#include <math.h>
#include <stdio.h>
#include <alloca.h>

// Forward declarations
static void init_symbol_table(LanguageCoreV2* lang);
static void create_links(LanguageCoreV2* lang, int input_density,
int recurrent_density, int output_density);
static void update_traces(LanguageCoreV2* lang);
static void stdp_update(LanguageCoreV2* lang, float reward);

// ------------------------------------------------
// Initialize language core v2
// ------------------------------------------------
void lang_v2_init(LanguageCoreV2* lang,
int hidden_size,
int input_density,
int recurrent_density,
int output_density,
float learning_rate) {

memset(lang, 0, sizeof(LanguageCoreV2));

// Calculate total elements
int input_count = SYMBOL_SET_SIZE * ENCODE_GROUP_SIZE;
int output_count = SYMBOL_SET_SIZE;
int total_elements = input_count + hidden_size + output_count;

// Estimate links
int estimated_links =
(input_count * hidden_size * input_density / 100)
+ (hidden_size * hidden_size * recurrent_density / 100)
+ (hidden_size * output_count * output_density / 100);

printf("\n=== Initializing Language Core v2 ===\n");
printf("Elements: %d (input=%d, hidden=%d, output=%d)\n",
total_elements, input_count, hidden_size, output_count);
printf("Estimated links: %d\n", estimated_links);

// Initialize system
sys_v2_init(&lang->base, total_elements, estimated_links * 2);

// Add zones (hierarchical levels)
// Zone 0: Input - passes external signals, no spike generation
sys_v2_add_zone(&lang->base, "input", 0, input_count, 1.0f, 1.0f);
lang->input_zone_id = 0;

// Zone 1: Hidden - processes signals, generates spikes
sys_v2_add_zone(&lang->base, "hidden", input_count, hidden_size, 0.15f, 0.99f);
lang->hidden_zone_id = 1;

// Zone 2: Output - receives hidden signals, generates predictions
sys_v2_add_zone(&lang->base, "output", input_count + hidden_size, output_count, 0.1f, 0.98f);
lang->output_zone_id = 2;

// Initialize symbol table
init_symbol_table(lang);

// Create links
create_links(lang, input_density, recurrent_density, output_density);

// Initialize plasticity
lang->plast.learning_rate = learning_rate;
lang->plast.modulation = 0.0f;
lang->plast.total_ltp = 0;
lang->plast.total_ltd = 0;

lang->plast.pre_trace = (float**)malloc(total_elements * sizeof(float*));
for (int i = 0; i < total_elements; i++) {
lang->plast.pre_trace[i] = (float*)calloc(TRACE_WINDOW, sizeof(float));
}
lang->plast.trace_ptr = (int*)calloc(total_elements, sizeof(int));

// Stats
lang->epoch = 0;
lang->last_reward = 0.0f;
lang->avg_reward = 0.0f;
lang->correct_predictions = 0;
lang->total_predictions = 0;

printf("Language Core v2 initialized: %d elements, %d links\n",
lang->base.element_count, lang->base.link_count);
}

// ------------------------------------------------
// Initialize symbol table
// ------------------------------------------------
static void init_symbol_table(LanguageCoreV2* lang) {
lang->symtab.encode_map = (int*)malloc(SYMBOL_SET_SIZE * ENCODE_GROUP_SIZE * sizeof(int));
lang->symtab.decode_map = (int*)malloc(SYMBOL_SET_SIZE * sizeof(int));

int input_start = lang->base.zones[lang->input_zone_id].start;
int output_start = lang->base.zones[lang->output_zone_id].start;

for (int ch = 0; ch < SYMBOL_SET_SIZE; ch++) {
for (int i = 0; i < ENCODE_GROUP_SIZE; i++) {
lang->symtab.encode_map[ch * ENCODE_GROUP_SIZE + i] =
input_start + ch * ENCODE_GROUP_SIZE + i;
}
lang->symtab.decode_map[ch] = output_start + ch;
}
}

// ------------------------------------------------
// Create links between zones
// ------------------------------------------------
static void create_links(LanguageCoreV2* lang, int input_density,
int recurrent_density, int output_density) {

SystemV2* sys = &lang->base;
int i_start = sys->zones[lang->input_zone_id].start;
int i_count = sys->zones[lang->input_zone_id].count;
int h_start = sys->zones[lang->hidden_zone_id].start;
int h_count = sys->zones[lang->hidden_zone_id].count;
int o_start = sys->zones[lang->output_zone_id].start;
int o_count = sys->zones[lang->output_zone_id].count;

int links_created = 0;

// 1. Input -> Hidden (direct, strong)
printf("Creating Input->Hidden links...\n");
for (int i = 0; i < i_count; i++) {
for (int h = 0; h < h_count; h++) {
if ((rand() % 100) < input_density) {
float w = 0.5f + (rand() / (float)RAND_MAX) * 0.5f;  // 0.5-1.0
if (rand() % 4 == 0) w = -w;  // 25% inhibitory
sys_v2_add_link(sys, i_start + i, h_start + h, w, 1);
links_created++;
}
}
}

// 2. Hidden -> Hidden (recurrent)
printf("Creating Hidden->Hidden links...\n");
for (int h1 = 0; h1 < h_count; h1++) {
for (int h2 = 0; h2 < h_count; h2++) {
if (h1 != h2 && (rand() % 100) < recurrent_density) {
float w = 0.3f + (rand() / (float)RAND_MAX) * 0.7f;  // 0.3-1.0
if (rand() % 3 == 0) w = -w;
int d = 1 + rand() % 5;
sys_v2_add_link(sys, h_start + h1, h_start + h2, w, d);
links_created++;
}
}
}

// 3. Hidden -> Output (direct, medium)
printf("Creating Hidden->Output links...\n");
for (int h = 0; h < h_count; h++) {
for (int o = 0; o < o_count; o++) {
if ((rand() % 100) < output_density) {
float w = 0.2f + (rand() / (float)RAND_MAX) * 0.3f;  // 0.2-0.5
if (rand() % 5 == 0) w = -w;  // 20% inhibitory
sys_v2_add_link(sys, h_start + h, o_start + o, w, 1);
links_created++;
}
}
}

// 4. Output -> Output (Winner-Take-All inhibition)
printf("Creating Output->Output links (WTA)...\n");
for (int o1 = 0; o1 < o_count; o1++) {
for (int o2 = 0; o2 < o_count; o2++) {
if (o1 != o2 && (rand() % 100) < 15) {
sys_v2_add_link(sys, o_start + o1, o_start + o2, -0.05f, 1);
links_created++;
}
}
}

printf("Total links created: %d\n", links_created);
}

// ------------------------------------------------
// Encode character to input zone
// ------------------------------------------------
static void encode_char(LanguageCoreV2* lang, unsigned char ch) {
int* map = lang->symtab.encode_map;
for (int i = 0; i < ENCODE_GROUP_SIZE; i++) {
int idx = map[ch * ENCODE_GROUP_SIZE + i];
lang->base.input_pulses[idx] = 3.0f;  // Strong input signal
}
}

// ------------------------------------------------
// Decode output zone to character
// ------------------------------------------------
unsigned char lang_v2_decode(LanguageCoreV2* lang) {
int o_start = lang->base.zones[lang->output_zone_id].start;

int best_idx = -1;
float best_val = -1e9f;

for (int ch = 0; ch < SYMBOL_SET_SIZE; ch++) {
int idx = lang->symtab.decode_map[ch];
if (idx < lang->base.element_count) {
float val = lang->base.elements[idx].state;
if (val > best_val) {
best_val = val;
best_idx = ch;
}
}
}

// Only return if above threshold
return (best_val > 0.1f) ? (unsigned char)best_idx : 0;
}

// ------------------------------------------------
// Compute reward (softmax-based)
// ------------------------------------------------
float lang_v2_compute_reward(LanguageCoreV2* lang, unsigned char target) {
int target_idx = lang->symtab.decode_map[target];
float target_state = lang->base.elements[target_idx].state;

// Find max for numerical stability
float max_state = target_state;
for (int ch = 0; ch < SYMBOL_SET_SIZE; ch++) {
int idx = lang->symtab.decode_map[ch];
float s = lang->base.elements[idx].state;
if (s > max_state) max_state = s;
}

// Compute softmax
float sum_exp = 0.0f;
for (int ch = 0; ch < SYMBOL_SET_SIZE; ch++) {
int idx = lang->symtab.decode_map[ch];
sum_exp += expf(lang->base.elements[idx].state - max_state);
}

float prob = expf(target_state - max_state) / sum_exp;

// Reward in [-1, 1]
return (prob - 0.5f) * 2.0f;
}

// ------------------------------------------------
// Update pre-synaptic traces (for STDP)
// ------------------------------------------------
static void update_traces(LanguageCoreV2* lang) {
for (int i = 0; i < lang->base.element_count; i++) {
// Decay traces
for (int t = 0; t < TRACE_WINDOW; t++) {
lang->plast.pre_trace[i][t] *= 0.9f;
}

// Record new spike
if (lang->base.current_pulses[i] > 0.0f) {
int pos = lang->plast.trace_ptr[i];
lang->plast.pre_trace[i][pos] = 1.0f;
lang->plast.trace_ptr[i] = (pos + 1) % TRACE_WINDOW;
}
}
}

// ------------------------------------------------
// STDP update with reward modulation
// ------------------------------------------------
static void stdp_update(LanguageCoreV2* lang, float reward) {
float eta = lang->plast.learning_rate;
int window = TRACE_WINDOW;
int ltp_count = 0, ltd_count = 0;

// Get zone boundaries
int h_start = lang->base.zones[lang->hidden_zone_id].start;
int h_count = lang->base.zones[lang->hidden_zone_id].count;
int o_start = lang->base.zones[lang->output_zone_id].start;

// Find which hidden elements are currently active
float* hidden_activity = (float*)alloca(h_count * sizeof(float));
float hidden_sum = 0.0f;
for (int h = 0; h < h_count; h++) {
hidden_activity[h] = lang->base.elements[h_start + h].state;
if (hidden_activity[h] < 0) hidden_activity[h] = 0;
hidden_sum += hidden_activity[h];
}

// Normalize hidden activity
if (hidden_sum > 0) {
for (int h = 0; h < h_count; h++) {
hidden_activity[h] /= hidden_sum;
}
}

// Direct learning: adjust hidden->output weights based on reward
for (int k = 0; k < lang->base.link_count; k++) {
LinkV2* ln = &lang->base.links[k];

// Special handling for hidden->output connections
if (ln->src >= h_start && ln->src < h_start + h_count &&
ln->dst >= o_start) {

int h_idx = ln->src - h_start;
float h_act = hidden_activity[h_idx];

// Direct weight update based on reward and hidden activity
if (h_act > 0.01f) {
float dw = eta * reward * h_act * 0.5f;

if (reward > 0) {
ln->weight += dw * 2.0f;  // Stronger positive reinforcement
} else {
ln->weight += dw * 0.5f;  // Weaker negative adjustment
}

ltp_count++;
}
} else {
// Standard STDP for other connections
int t_pre = -1, t_post = -1;
for (int t = 0; t < window; t++) {
if (lang->plast.pre_trace[ln->src][t] > 0.5f) { t_pre = t; break; }
}
for (int t = 0; t < window; t++) {
if (lang->plast.pre_trace[ln->dst][t] > 0.5f) { t_post = t; break; }
}

float delta = 0.0f;

if (t_pre >= 0 && t_post >= 0) {
float dt = (float)(t_post - t_pre);
if (dt > 0) {
delta = 0.02f * expf(-dt / 10.0f);  // LTP
ltp_count++;
} else if (dt < 0) {
delta = -0.01f * expf(dt / 10.0f);  // LTD
ltd_count++;
}
}

// Update eligibility
ln->eligibility = ln->eligibility * 0.95f + delta;

// Update weight
float dw = eta * reward * ln->eligibility * 0.1f;
ln->weight += dw;
}

// Clamp weights
if (ln->weight > 3.0f) ln->weight = 3.0f;
if (ln->weight < -3.0f) ln->weight = -3.0f;
}

lang->plast.total_ltp += ltp_count;
lang->plast.total_ltd += ltd_count;
}

// ------------------------------------------------
// Step: process one character
// ------------------------------------------------
void lang_v2_step(LanguageCoreV2* lang, unsigned char input_char) {
// Encode input
encode_char(lang, input_char);

// Run hierarchical step (THE KEY FIX!)
// This ensures proper signal flow: Input -> Hidden -> Output
sys_v2_step_hierarchical(&lang->base);

// Update traces for STDP
update_traces(lang);

// Run a second step to let hidden->output propagate fully
sys_v2_step_hierarchical(&lang->base);
update_traces(lang);
}

// ------------------------------------------------
// Learn: adjust weights based on target
// ------------------------------------------------
void lang_v2_learn(LanguageCoreV2* lang, unsigned char target_char) {
// SUPERVISED LEARNING: Directly activate target output
int target_idx = lang->symtab.decode_map[target_char];
float target_boost = 10.0f;  // Strong teacher signal

// Boost target output
lang->base.elements[target_idx].state += target_boost;
lang->base.current_pulses[target_idx] = target_boost;

// Compute reward (should be positive since we boosted target)
float reward = lang_v2_compute_reward(lang, target_char);
lang->last_reward = reward;

// Update average reward
lang->avg_reward = 0.99f * lang->avg_reward + 0.01f * reward;

// Track accuracy (before teacher signal)
unsigned char predicted = lang_v2_decode(lang);
lang->total_predictions++;
if (predicted == target_char) {
lang->correct_predictions++;
}

// STDP update with the boosted reward
stdp_update(lang, reward);
}

// ------------------------------------------------
// Debug output
// ------------------------------------------------
void lang_v2_debug(LanguageCoreV2* lang, unsigned char target) {
printf("\n--- Language Core v2 Debug ---\n");

// Show zone states
sys_v2_debug_zones(&lang->base);

// Show target
int target_idx = lang->symtab.decode_map[target];
printf("Target: '%c' (idx=%d, state=%.4f)\n",
target > 31 ? target : '?', target_idx,
lang->base.elements[target_idx].state);

// Top predictions
printf("Top predictions:\n");
float states[SYMBOL_SET_SIZE];
for (int ch = 0; ch < SYMBOL_SET_SIZE; ch++) {
states[ch] = lang->base.elements[lang->symtab.decode_map[ch]].state;
}

for (int rank = 0; rank < 5; rank++) {
int best = 0;
float max_val = states[0];
for (int ch = 1; ch < SYMBOL_SET_SIZE; ch++) {
if (states[ch] > max_val) {
max_val = states[ch];
best = ch;
}
}
printf("  %d: '%c' (%.4f)\n", rank + 1, best > 31 ? best : '?', max_val);
states[best] = -999;
}
}

// ------------------------------------------------
// Print statistics
// ------------------------------------------------
void lang_v2_print_stats(LanguageCoreV2* lang) {
printf("\n=== Language Core v2 Statistics ===\n");
printf("Epoch: %d\n", lang->epoch);
printf("Accuracy: %.2f%% (%d/%d)\n",
100.0f * lang->correct_predictions /
(lang->total_predictions > 0 ? lang->total_predictions : 1),
lang->correct_predictions, lang->total_predictions);
printf("Reward: last=%.4f, avg=%.4f\n", lang->last_reward, lang->avg_reward);
printf("LTP: %d, LTD: %d\n", lang->plast.total_ltp, lang->plast.total_ltd);

// Weight distribution
int pos = 0, neg = 0;
float pos_sum = 0.0f, neg_sum = 0.0f;
for (int k = 0; k < lang->base.link_count; k++) {
float w = lang->base.links[k].weight;
if (w > 0.01f) { pos++; pos_sum += w; }
else if (w < -0.01f) { neg++; neg_sum += w; }
}
printf("Weights: pos=%d (avg=%.3f), neg=%d (avg=%.3f)\n",
pos, pos_sum / (pos > 0 ? pos : 1),
neg, neg_sum / (neg > 0 ? neg : 1));
}

// ------------------------------------------------
// Reset
// ------------------------------------------------
void lang_v2_reset(LanguageCoreV2* lang) {
sys_v2_reset(&lang->base);

for (int i = 0; i < lang->base.element_count; i++) {
for (int t = 0; t < TRACE_WINDOW; t++) {
lang->plast.pre_trace[i][t] = 0.0f;
}
lang->plast.trace_ptr[i] = 0;
}
}

// ------------------------------------------------
// Free
// ------------------------------------------------
void lang_v2_free(LanguageCoreV2* lang) {
sys_v2_free(&lang->base);
free(lang->symtab.encode_map);
free(lang->symtab.decode_map);
for (int i = 0; i < lang->base.element_count; i++) {
free(lang->plast.pre_trace[i]);
}
free(lang->plast.pre_trace);
free(lang->plast.trace_ptr);
}
