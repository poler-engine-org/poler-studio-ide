#!/usr/bin/env python3
"""P3 Engine — Network Replication (fixed-point math, delta compression, rollback)."""
import json, numpy as np
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from pathlib import Path

OUT = Path('/home/z/P3_Engine/calculations/network_replication'); OUT.mkdir(parents=True, exist_ok=True)

# 1. FIXED-POINT MATH (for deterministic physics across machines)
# Floating point math is non-deterministic across CPUs (FMA, fused ops, etc.)
# Fixed-point: represent floats as integers × scale factor
# E.g., Q16.16: 32-bit integer with 16 fractional bits → precision = 1/65536

precision_q16 = 1.0 / (1 << 16)
precision_q24 = 1.0 / (1 << 24)
precision_q32 = 1.0 / (1 << 32)
print("1. Fixed-point math precision:")
print(f"  Q16.16 (16 int + 16 frac, 32-bit total): precision = {precision_q16:.10f}")
print(f"  Q24.8  (24 int + 8 frac,  32-bit total): precision = {1.0/256:.10f}")
print(f"  Q32.32 (32 int + 32 frac, 64-bit total): precision = {precision_q32:.15f}")
print(f"  Float32 epsilon ≈ 1.19e-7 (range-dependent)")
print(f"  → Q16.16 is too imprecise for physics; Q32.32 has more than float32 precision")

# 2. DELTA COMPRESSION (snapshot delta)
# State: 10 floats (position, velocity, etc.)
# Naive: 10 * 4 = 40 bytes per snapshot
# Delta: only send fields that changed > threshold
# Compression ratio: depends on change frequency

# Simulate: 1000 frames, only ~10% fields change per frame
np.random.seed(42)
n_fields = 10
n_frames = 1000
states = np.random.randn(n_frames, n_fields).astype(np.float32)
# Make most fields stay constant (slow motion)
for i in range(1, n_frames):
    # 90% of fields stay the same
    mask = np.random.random(n_fields) < 0.1
    states[i] = np.where(mask, np.random.randn(n_fields), states[i-1])

# Compute deltas
deltas = np.abs(states[1:] - states[:-1])
threshold = 0.01  # 1cm for positions
changed = deltas > threshold
change_rate = changed.mean()
naive_bytes = n_frames * n_fields * 4
delta_bytes = (changed.sum() * 4)  # only send changed fields
overhead_per_frame = 2  # 2 bytes for changed-fields bitmask
delta_bytes += n_frames * overhead_per_frame

print(f"\n2. Delta compression test (1000 frames × 10 fields):")
print(f"  Change rate: {change_rate*100:.1f}% of fields per frame")
print(f"  Naive:  {naive_bytes:,} bytes ({naive_bytes/1024:.1f} KB)")
print(f"  Delta:  {delta_bytes:,} bytes ({delta_bytes/1024:.1f} KB)")
print(f"  Compression ratio: {naive_bytes/delta_bytes:.2f}×")

# 3. ROLLBACK (GGPO-style)
# Client predicts N frames ahead with current input
# Server sends authoritative state
# Client rolls back to last confirmed frame, re-simulates from there
# Input delay: K frames (typical 2-4)
# Window: W frames of re-simulation

K = 2  # input delay
W = 7  # rollback window
print(f"\n3. GGPO Rollback config:")
print(f"  Input delay K = {K} frames")
print(f"  Rollback window W = {W} frames")
print(f"  Re-simulation cost per rollback: W * physics_step_time")
print(f"  At 60 FPS, W=7 frames = {W/60*1000:.0f}ms re-sim per correction")

# 4. Plot: change rate over time
fig, axes = plt.subplots(1, 2, figsize=(12, 5))
window = 50
rolling_change = []
for i in range(window, n_frames):
    rolling_change.append(changed[i-window:i].mean())
axes[0].plot(rolling_change, 'b-', linewidth=2)
axes[0].axhline(y=change_rate, color='r', linestyle='--', label=f'Avg: {change_rate*100:.1f}%')
axes[0].set_xlabel('Frame')
axes[0].set_ylabel('Change rate (rolling 50)')
axes[0].set_title('Snapshot Delta Compression\n(fields changed per frame)')
axes[0].legend(); axes[0].grid(True, alpha=0.3)

# 5. Bandwidth: naive vs delta
bandwidth_naive = 60 * n_fields * 4 / 1024  # KB/s at 60 FPS
bandwidth_delta = 60 * (changed.mean() * n_fields * 4 + overhead_per_frame) / 1024
axes[1].bar(['Naive (per frame)', 'Delta (per frame)'], 
           [n_fields * 4, changed.mean() * n_fields * 4 + overhead_per_frame],
           color=['red', 'green'])
axes[1].set_ylabel('Bytes per frame')
axes[1].set_title(f'Per-Frame Bandwidth (60 FPS):\nNaive: {bandwidth_naive:.1f} KB/s | Delta: {bandwidth_delta:.1f} KB/s')
axes[1].grid(True, alpha=0.3)

plt.tight_layout()
plt.savefig(OUT / 'network_replication.png', dpi=100)
print(f"\nSaved: {OUT / 'network_replication.png'}")

results = {
    "module": "network_replication",
    "formulas": {
        "fixed_point_Q32_32": "x = (int32_t)(value * 2^32); precision = 1/2^32 ≈ 2.3e-10",
        "delta_compression": "send only fields where |new - prev| > threshold; bitmask selects changed fields",
        "ggpo_rollback": "client predicts K frames ahead, re-simulates W frames when server correction arrives",
        "snapshot_size": "sizeof(state) — minimize via field-level encoding",
    },
    "test_results": {
        "fixed_point_precision": {
            "Q16_16": float(precision_q16),
            "Q24_8": float(1.0/256),
            "Q32_32": float(precision_q32),
            "float32_epsilon": 1.19e-7,
            "recommendation": "Use Q32.32 for physics determinism (better than float32 precision)",
        },
        "delta_compression_test": {
            "frames": n_frames,
            "fields_per_frame": n_fields,
            "change_rate_percent": float(change_rate * 100),
            "naive_bytes": int(naive_bytes),
            "delta_bytes": int(delta_bytes),
            "compression_ratio": float(naive_bytes / delta_bytes),
        },
        "ggpo_rollback": {
            "input_delay_frames": K,
            "rollback_window_frames": W,
            "re_simulation_time_ms_at_60fps": W / 60 * 1000,
        },
    },
    "implementation_notes": [
        "Fixed-point math: replace f32 with Q32.32 in physics core",
        "Snapshot delta: serialize state diff per frame, send over UDP",
        "Bitmask: 2 bytes per frame = 16 fields max per snapshot",
        "GGPO: simulate client + server side-by-side for K frames ahead",
        "Re-simulation: store input buffer of size W, replay on correction",
        "Smoothing: snap to corrected state over 100ms to hide jitter",
    ],
}
with open(OUT / 'network_replication_results.json', 'w') as f:
    json.dump(results, f, indent=2)

with open(OUT / 'network_formulas.txt', 'w') as f:
    f.write("P3 ENGINE — NETWORK REPLICATION DERIVATIONS\n")
    f.write("=" * 60 + "\n\n")
    f.write("1. FIXED-POINT MATH (deterministic physics)\n")
    f.write("-" * 40 + "\n")
    f.write("Floating point math is non-deterministic across CPUs\n")
    f.write("(FMA, fused ops, rounding mode differences).\n")
    f.write("\nFixed-point: store value × 2^F as integer (F fractional bits):\n")
    f.write("  Q16.16: 32-bit total, 16 frac bits → precision = 1/65536 ≈ 1.5e-5\n")
    f.write("  Q32.32: 64-bit total, 32 frac bits → precision ≈ 2.3e-10\n")
    f.write("\n  Recommendation: Q32.32 for physics (better than float32 epsilon 1.19e-7)\n\n")
    f.write("2. DELTA COMPRESSION (snapshot diff)\n")
    f.write("-" * 40 + "\n")
    f.write("Each frame: state = N fields (positions, velocities, etc.)\n")
    f.write("Naive: send N * sizeof(f32) bytes per frame\n")
    f.write("\nDelta: only send fields where |new - prev| > threshold\n")
    f.write("  Bitmask: 2 bytes (16 fields) selects changed fields\n")
    f.write("  Per-field: 4 bytes if changed, 0 if not\n")
    f.write("\n  Compression ratio = 1 / change_rate\n")
    f.write("  Typical: 10x compression at 10% change rate\n\n")
    f.write("3. GGPO ROLLBACK (client-side prediction)\n")
    f.write("-" * 40 + "\n")
    f.write("Client predicts K frames ahead (K=2 typical):\n")
    f.write("  Buffer: last W=7 frames of input + state\n")
    f.write("  Display: show predicted frame K+1 (client is ahead by K)\n")
    f.write("\nOn server correction (frame n confirmed):\n")
    f.write("  If state[n] matches predicted: no action\n")
    f.write("  If mismatch: rollback to frame n, re-simulate n → n+W\n")
    f.write("  Cost: W * physics_step_time (e.g., 7 * 0.016 = 112ms re-sim)\n")
    f.write("\n  Smoothing: snap-to-corrected over 100ms to hide visual jitter\n")

print(f"Network replication calculations COMPLETE")
