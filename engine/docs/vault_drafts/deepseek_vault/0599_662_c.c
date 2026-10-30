// ============================================================================
// system_core_v2.c
//
// HIERARCHICAL CONSTRAINT SYSTEM v2.0
// Inspired by POLER[n] cyclic architecture and RPN hierarchical processing
// ============================================================================

#include "system_core_v2.h"
#include <string.h>
#include <math.h>
#include <stdio.h>

// ------------------------------------------------
// Initialize system
// ------------------------------------------------
void sys_v2_init(SystemV2* sys, int element_count, int link_capacity) {
memset(sys, 0, sizeof(SystemV2));

sys->element_count = element_count;
sys->link_capacity = link_capacity;
sys->link_count = 0;

sys->elements = (ElementV2*)calloc(element_count, sizeof(ElementV2));
sys->links = (LinkV2*)malloc(link_capacity * sizeof(LinkV2));

sys->current_pulses = (float*)calloc(element_count, sizeof(float));
sys->next_pulses = (float*)calloc(element_count, sizeof(float));
sys->input_pulses = (float*)calloc(element_count, sizeof(float));

sys->zone_count = 0;
sys->constraint_matrix = NULL;
sys->constraint_strength = 0.3f;
sys->system_time = 0.0f;
sys->energy_budget = 1.0f;
sys->debug_mode = 0;

for (int i = 0; i < element_count; i++) {
sys->elements[i].state = 0.0f;
sys->elements[i].baseline = 0.0f;
sys->elements[i].zone_id = -1;
sys->elements[i].last_spike = -100.0f;
}
}

// ------------------------------------------------
// Add zone (hierarchical level)
// ------------------------------------------------
void sys_v2_add_zone(SystemV2* sys, const char* name, int start, int count,
float threshold, float decay) {
if (sys->zone_count >= MAX_ZONES) {
printf("ERROR: Max zones reached\n");
return;
}

Zone* z = &sys->zones[sys->zone_count];
strncpy(z->name, name, 31);
z->name[31] = '\0';
z->start = start;
z->count = count;
z->threshold = threshold;
z->decay = decay;

// Mark elements as belonging to this zone
for (int i = start; i < start + count && i < sys->element_count; i++) {
sys->elements[i].zone_id = sys->zone_count;
}

printf("Zone %d: [%s] [%d-%d], threshold=%.2f, decay=%.3f\n",
sys->zone_count, name, start, start + count - 1, threshold, decay);

sys->zone_count++;
}

// ------------------------------------------------
// Add link
// ------------------------------------------------
void sys_v2_add_link(SystemV2* sys, int src, int dst, float weight, int delay) {
if (sys->link_count >= sys->link_capacity) {
sys->link_capacity *= 2;
sys->links = (LinkV2*)realloc(sys->links, sys->link_capacity * sizeof(LinkV2));
}

LinkV2* ln = &sys->links[sys->link_count++];
ln->src = src;
ln->dst = dst;
ln->weight = weight;
ln->delay = delay;
ln->head = 0;
ln->buffer = (float*)calloc(delay, sizeof(float));
ln->eligibility = 0.0f;
}

// ------------------------------------------------
// Phase 1: Decay all states
// ------------------------------------------------
void sys_v2_phase_decay(SystemV2* sys) {
for (int z = 0; z < sys->zone_count; z++) {
Zone* zone = &sys->zones[z];
for (int i = zone->start; i < zone->start + zone->count; i++) {
sys->elements[i].state *= zone->decay;
}
}
}

// ------------------------------------------------
// Phase 2: Activate input zone from external input
// ------------------------------------------------
void sys_v2_phase_input_activation(SystemV2* sys) {
if (sys->zone_count == 0) return;

Zone* input_zone = &sys->zones[0];  // Zone 0 is input

for (int i = input_zone->start; i < input_zone->start + input_zone->count; i++) {
if (sys->input_pulses[i] > 0.0f) {
// Input zone directly passes input to current_pulses
sys->current_pulses[i] = sys->input_pulses[i];
sys->elements[i].state = sys->input_pulses[i];
}
}

// Clear external input
memset(sys->input_pulses, 0, sys->element_count * sizeof(float));
}

// ------------------------------------------------
// Phase 3: Propagate signals FROM a specific zone
// This implements the hierarchical flow from RPN
// ------------------------------------------------
void sys_v2_phase_propagate_zone(SystemV2* sys, int src_zone_id) {
if (src_zone_id < 0 || src_zone_id >= sys->zone_count) return;

Zone* src_zone = &sys->zones[src_zone_id];

// Process all links originating from this zone
for (int k = 0; k < sys->link_count; k++) {
LinkV2* ln = &sys->links[k];

// Check if link source is in the source zone
int src = ln->src;
if (src < src_zone->start || src >= src_zone->start + src_zone->count) {
continue;  // Skip links from other zones
}

// Get pulse from source (current_pulses has the spikes)
float pulse = sys->current_pulses[src];

// Also check delay buffer
if (pulse == 0.0f && ln->delay > 1) {
pulse = ln->buffer[ln->head];
}

if (pulse > 0.0f) {
// Propagate to destination element
sys->elements[ln->dst].state += ln->weight * pulse;

// Track energy
sys->energy_budget -= 0.001f * ln->weight * pulse;
}
}
}

// ------------------------------------------------
// Phase 4: Generate spikes in a specific zone
// Based on accumulated state, elements spike
// ------------------------------------------------
void sys_v2_phase_spike_generation(SystemV2* sys, int zone_id) {
if (zone_id < 0 || zone_id >= sys->zone_count) return;

Zone* zone = &sys->zones[zone_id];

// Skip input zone (doesn't generate spikes, only passes input)
if (zone_id == 0) return;

for (int i = zone->start; i < zone->start + zone->count; i++) {
float state = sys->elements[i].state;

// Spike if above threshold
if (state >= zone->threshold) {
float spike_strength = (state >= 1.0f) ? 1.0f : state;

// Generate spike
sys->next_pulses[i] = spike_strength;

// Consume state
sys->elements[i].state -= spike_strength;

// Track spike time (for STDP)
sys->elements[i].last_spike = sys->system_time;

// Write to delay buffers of links from this element
for (int k = 0; k < sys->link_count; k++) {
LinkV2* ln = &sys->links[k];
if (ln->src == i && ln->delay > 1) {
int write_pos = (ln->head + ln->delay - 1) % ln->delay;
ln->buffer[write_pos] = spike_strength;
}
}
}
}
}

// ------------------------------------------------
// Advance delay buffers
// ------------------------------------------------
static void advance_delay_buffers(SystemV2* sys) {
for (int k = 0; k < sys->link_count; k++) {
LinkV2* ln = &sys->links[k];
if (ln->delay > 1) {
ln->buffer[ln->head] = 0.0f;
ln->head = (ln->head + 1) % ln->delay;
}
}
}

// ------------------------------------------------
// MAIN STEP FUNCTION - Hierarchical Multi-Phase
//
// This is the core innovation inspired by POLER and RPN:
// Process zones in order, ensuring proper signal flow
// ------------------------------------------------
void sys_v2_step_hierarchical(SystemV2* sys) {

// Clear pulse buffers for new cycle
memset(sys->current_pulses, 0, sys->element_count * sizeof(float));
memset(sys->next_pulses, 0, sys->element_count * sizeof(float));

// === PHASE 1: DECAY ===
// All states decay (time passes)
sys_v2_phase_decay(sys);

// === PHASE 2: INPUT ACTIVATION ===
// External input activates input zone
sys_v2_phase_input_activation(sys);

// === PHASE 3: HIERARCHICAL PROPAGATION ===
// Process zones in order (like RPN's bottom-up)
// Zone 0 (input) -> Zone 1 (hidden) -> Zone 2 (output)

for (int z = 0; z < sys->zone_count; z++) {
if (sys->debug_mode) {
printf("Processing zone %d: %s\n", z, sys->zones[z].name);
}

// A) Propagate signals FROM this zone to all destinations
sys_v2_phase_propagate_zone(sys, z);

// B) Generate spikes in the NEXT zones (not current)
// Spikes happen in destination zones after they receive signals
if (z < sys->zone_count - 1) {
// Next zones spike based on their accumulated state
sys_v2_phase_spike_generation(sys, z + 1);
}

// C) Move next_pulses to current_pulses for next iteration
// (spikes generated become the pulses to propagate)
for (int i = 0; i < sys->element_count; i++) {
if (sys->next_pulses[i] > 0.0f) {
sys->current_pulses[i] = sys->next_pulses[i];
}
}
memset(sys->next_pulses, 0, sys->element_count * sizeof(float));
}

// === PHASE 4: ADVANCE TIME ===
advance_delay_buffers(sys);
sys->system_time += 1.0f;

// Regenerate energy
sys->energy_budget = 0.95f * sys->energy_budget + 0.05f * 1.0f;
}

// ------------------------------------------------
// Simple step (fallback, similar to original)
// ------------------------------------------------
void sys_v2_step_simple(SystemV2* sys) {
// Decay
sys_v2_phase_decay(sys);

// Process all links at once (original behavior)
for (int k = 0; k < sys->link_count; k++) {
LinkV2* ln = &sys->links[k];

float pulse = sys->input_pulses[ln->src];
if (pulse == 0.0f) pulse = sys->current_pulses[ln->src];
if (pulse == 0.0f && ln->delay > 1) pulse = ln->buffer[ln->head];

if (pulse > 0.0f) {
sys->elements[ln->dst].state += ln->weight * pulse;
}
}

// Generate spikes
for (int i = 0; i < sys->element_count; i++) {
if (sys->elements[i].zone_id > 0) {  // Not input zone
float state = sys->elements[i].state;
int zone_id = sys->elements[i].zone_id;
if (zone_id >= 0 && state >= sys->zones[zone_id].threshold) {
float spike = (state >= 1.0f) ? 1.0f : state;
sys->current_pulses[i] = spike;
sys->elements[i].state -= spike;
}
}
}

memset(sys->input_pulses, 0, sys->element_count * sizeof(float));
advance_delay_buffers(sys);
sys->system_time += 1.0f;
}

// ------------------------------------------------
// Debug: Print zone states
// ------------------------------------------------
void sys_v2_debug_zones(SystemV2* sys) {
printf("\n=== System State (t=%.1f) ===\n", sys->system_time);

for (int z = 0; z < sys->zone_count; z++) {
Zone* zone = &sys->zones[z];

int active = 0;
float max_state = 0.0f;
float sum_state = 0.0f;
int spikes = 0;

for (int i = zone->start; i < zone->start + zone->count; i++) {
if (sys->elements[i].state > 0.01f) active++;
if (sys->elements[i].state > max_state) max_state = sys->elements[i].state;
sum_state += sys->elements[i].state;
if (sys->current_pulses[i] > 0.0f) spikes++;
}

printf("Zone %d [%s]: active=%d, max=%.3f, avg=%.3f, spikes=%d\n",
z, zone->name, active, max_state,
sum_state / zone->count, spikes);
}

printf("Energy: %.3f\n", sys->energy_budget);
}

// ------------------------------------------------
// Reset system
// ------------------------------------------------
void sys_v2_reset(SystemV2* sys) {
for (int i = 0; i < sys->element_count; i++) {
sys->elements[i].state = 0.0f;
sys->elements[i].last_spike = -100.0f;
}

memset(sys->current_pulses, 0, sys->element_count * sizeof(float));
memset(sys->next_pulses, 0, sys->element_count * sizeof(float));
memset(sys->input_pulses, 0, sys->element_count * sizeof(float));

for (int k = 0; k < sys->link_count; k++) {
for (int d = 0; d < sys->links[k].delay; d++) {
sys->links[k].buffer[d] = 0.0f;
}
sys->links[k].head = 0;
sys->links[k].eligibility = 0.0f;
}

sys->system_time = 0.0f;
sys->energy_budget = 1.0f;
}

// ------------------------------------------------
// Free system
// ------------------------------------------------
void sys_v2_free(SystemV2* sys) {
free(sys->elements);
for (int k = 0; k < sys->link_count; k++) {
free(sys->links[k].buffer);
}
free(sys->links);
free(sys->current_pulses);
free(sys->next_pulses);
free(sys->input_pulses);
if (sys->constraint_matrix) free(sys->constraint_matrix);
}
