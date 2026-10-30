// ============================================================================
// system_core_v2.h
//
// HIERARCHICAL CONSTRAINT SYSTEM v2.0
// Inspired by POLER[n] cyclic architecture and RPN hierarchical processing
//
// Key innovation: Multi-phase step function with proper signal flow:
//   Phase 1: Input zone activation
//   Phase 2: Input->Hidden propagation + Hidden spike generation
//   Phase 3: Hidden->Output propagation
//   Phase 4: Output activation
// ============================================================================

#ifndef SYSTEM_CORE_V2_H
#define SYSTEM_CORE_V2_H

#include <stdlib.h>
#include <stdio.h>

// ------------------------------------------------
// Constants
// ------------------------------------------------
#define MAX_ZONES 8

// ------------------------------------------------
// Element: holds state
// ------------------------------------------------
typedef struct {
float state;          // Current state
float baseline;       // Baseline activity (for homeostasis)
int   zone_id;        // Which zone this element belongs to
float last_spike;     // Time of last spike (for STDP)
} ElementV2;

// ------------------------------------------------
// Link: describes constraint between elements
// ------------------------------------------------
typedef struct {
int   src;            // Source element
int   dst;            // Destination element
float weight;         // Constraint strength
int   delay;          // Delay in timesteps
int   head;           // Buffer head position
float* buffer;        // Delay buffer
float eligibility;    // For STDP learning
} LinkV2;

// ------------------------------------------------
// Zone: group of elements (like POLER's stages)
// ------------------------------------------------
typedef struct {
int   start;          // First element index
int   count;          // Number of elements
char  name[32];       // Zone name
float threshold;      // Spike threshold for this zone
float decay;          // Decay rate for this zone
} Zone;

// ------------------------------------------------
// System v2: Hierarchical constraint system
// ------------------------------------------------
typedef struct {
// Elements
ElementV2* elements;
int        element_count;

// Links
LinkV2*    links;
int        link_count;
int        link_capacity;

// Zones (hierarchical levels)
Zone       zones[MAX_ZONES];
int        zone_count;

// Signal buffers (like POLER's perception, image, logic stages)
float*     current_pulses;    // Pulses being processed NOW
float*     next_pulses;       // Pulses for NEXT phase
float*     input_pulses;      // External input

// Constraint matrix (optional)
float*     constraint_matrix;
float      constraint_strength;

// Time tracking (emergent time from POLER)
float      system_time;
float      energy_budget;

// Debug flags
int        debug_mode;
} SystemV2;

// ------------------------------------------------
// Function declarations
// ------------------------------------------------

// Initialization
void sys_v2_init(SystemV2* sys, int element_count, int link_capacity);
void sys_v2_add_zone(SystemV2* sys, const char* name, int start, int count,
float threshold, float decay);
void sys_v2_add_link(SystemV2* sys, int src, int dst, float weight, int delay);

// Core processing (hierarchical)
void sys_v2_step_hierarchical(SystemV2* sys);  // Multi-phase step
void sys_v2_step_simple(SystemV2* sys);        // Simple fallback

// Phase functions (like POLER stages)
void sys_v2_phase_decay(SystemV2* sys);
void sys_v2_phase_input_activation(SystemV2* sys);
void sys_v2_phase_propagate_zone(SystemV2* sys, int zone_id);
void sys_v2_phase_spike_generation(SystemV2* sys, int zone_id);

// Utility
void sys_v2_free(SystemV2* sys);
void sys_v2_reset(SystemV2* sys);
void sys_v2_debug_zones(SystemV2* sys);

#endif
