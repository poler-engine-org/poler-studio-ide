#!/usr/bin/env python3
"""
P3 Engine — Character Physics calculations (capsule vs triangle, step-up, buoyancy).

Output:
  - character_physics_formulas.txt  — symbolic derivations
  - step_up_curve.png               — Hermite smoothing visualization
  - buoyancy_curve.png              — Archimedes vs depth
  - capsule_triangle_distance.png   — geometry diagram
  - character_physics_results.json  — numerical constants
"""
import json
import numpy as np
import sympy as sp
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from pathlib import Path

OUT_DIR = Path('/home/z/P3_Engine/calculations/character_physics')
OUT_DIR.mkdir(parents=True, exist_ok=True)

# ========================================
# 1. CAPSULE vs TRIANGLE — closed-form distance
# ========================================
print("=" * 60)
print("1. CAPSULE vs TRIANGLE distance (closed form)")
print("=" * 60)

# Symbols
seg_t = sp.Symbol('t', real=True)  # parameter along segment [0,1]
seg_a = sp.Matrix([sp.Symbol('ax'), sp.Symbol('ay'), sp.Symbol('az')])  # segment start
seg_b = sp.Matrix([sp.Symbol('bx'), sp.Symbol('by'), sp.Symbol('bz')])  # segment end
tri_p0 = sp.Matrix([sp.Symbol('p0x'), sp.Symbol('p0y'), sp.Symbol('p0z')])
tri_p1 = sp.Matrix([sp.Symbol('p1x'), sp.Symbol('p1y'), sp.Symbol('p1z')])
tri_p2 = sp.Matrix([sp.Symbol('p2x'), sp.Symbol('p2y'), sp.Symbol('p2z')])

# Point on segment
P_seg = seg_a + (seg_b - seg_a) * seg_t

# Distance from segment point to triangle plane:
# d = |(P_seg - p0) . n|  where n = normalize((p1-p0) x (p2-p0))
n = (tri_p1 - tri_p0).cross(tri_p2 - tri_p0)
n_norm = sp.sqrt(n.dot(n))
n_hat = n / n_norm

distance_to_plane = (P_seg - tri_p0).dot(n_hat)
print(f"Distance from capsule axis point to triangle plane:")
print(f"  d(t) = |((A + t*(B-A)) - P0) . n_hat|")
print(f"  n_hat = ((P1-P0) x (P2-P0)) / ||...||")

# Capsule radius R — contact when distance <= R
# Min over t in [0,1] gives closest point on segment
# d/dt of distance² = 0 gives t* (analytical optimum)
print(f"\nContact condition: min_t |d(t)| <= R (capsule radius)")
print(f"For step-up: capsule axis tilted forward, R must clear stair step height h_step")

# Numerical verification: capsule at height 1.0, axis pointing forward-down at 45°
R = 0.4  # capsule radius (m)
height = 1.8  # total capsule height (m)
half_height = (height - 2 * R) / 2  # cylinder half-length
print(f"\nCapsule defaults:")
print(f"  Radius:        {R} m")
print(f"  Total height:  {height} m")
print(f"  Cylinder half: {half_height:.2f} m (the segment length)")
print(f"  Stair step:    0.20 m (UE default MAX_STEP_SIDE_Z)")

# ========================================
# 2. STEP-UP SMOOTHING (Hermite cubic)
# ========================================
print("\n" + "=" * 60)
print("2. STEP-UP HEIGHT SMOOTHING (cubic Hermite)")
print("=" * 60)

# When character steps up a stair, capsule origin moves up smoothly
# from current y to current_y + step_height over duration T
# Using smoothstep (Hermite cubic): s(t) = 3t² - 2t³
t = sp.Symbol('t', real=True, positive=True)
smoothstep = 3 * t**2 - 2 * t**3
print(f"smoothstep(t) = 3t² - 2t³")
print(f"  smoothstep(0) = {smoothstep.subs(t, 0)}")
print(f"  smoothstep(1) = {smoothstep.subs(t, 1)}")
print(f"  smoothstep'(0) = {sp.diff(smoothstep, t).subs(t, 0)}  (zero velocity at start)")
print(f"  smoothstep'(1) = {sp.diff(smoothstep, t).subs(t, 1)}  (zero velocity at end) → no snap")

# Visualize
t_vals = np.linspace(0, 1, 100)
smooth_vals = 3 * t_vals**2 - 2 * t_vals**3
step_height = 0.20
fig, ax = plt.subplots(figsize=(8, 5))
ax.plot(t_vals, smooth_vals * step_height, 'b-', linewidth=3, label='Position (smoothstep)')
ax.plot(t_vals, np.gradient(smooth_vals * step_height, t_vals), 'r--', linewidth=2, label='Velocity (derivative)')
ax.axhline(y=0, color='k', alpha=0.3)
ax.set_xlabel('Normalized time (t/T)')
ax.set_ylabel('Height (m) / Velocity (m/s)')
ax.set_title('Character Step-Up Hermite Smoothing\n(Eliminates snap on stair ascent)')
ax.legend()
ax.grid(True, alpha=0.3)
plt.tight_layout()
plt.savefig(OUT_DIR / 'step_up_curve.png', dpi=100)
print(f"\nSaved: {OUT_DIR / 'step_up_curve.png'}")

# ========================================
# 3. BUOYANCY (Archimedes) — swimming physics
# ========================================
print("\n" + "=" * 60)
print("3. BUOYANCY — Archimedes for swimming")
print("=" * 60)

# Submerged volume V_sub of capsule in water
# Approximation: capsule vertical, water level at depth y_water from capsule origin (top)
# V_sub = (Capsule Volume) × (submerged fraction)

capsule_volume = np.pi * R**2 * (2 * half_height) + (4/3) * np.pi * R**3
print(f"Capsule volume: {capsule_volume:.4f} m³")
print(f"Water density:   1000 kg/m³")
print(f"Gravity:         9.81 m/s²")

# Buoyancy force (Archimedes)
rho_water = 1000.0
g = 9.81
F_buoyancy_max = rho_water * g * capsule_volume
print(f"\nMax buoyancy (fully submerged): {F_buoyancy_max:.2f} N")

# Character mass 80 kg → weight
mass = 80.0
weight = mass * g
print(f"Character weight (80kg):       {weight:.2f} N")
print(f"Net force (full submerge):     {F_buoyancy_max - weight:.2f} N  (upward)")

# Linear submersion fraction (simplified — for full capsule vertical)
submersion = np.linspace(0, 1, 100)  # 0 = out of water, 1 = fully submerged
F_buoy = rho_water * g * capsule_volume * submersion
F_net = F_buoy - weight
accel = F_net / mass

fig, ax = plt.subplots(figsize=(8, 5))
ax.plot(submersion * 100, F_buoy, 'b-', linewidth=2, label='Buoyancy force')
ax.axhline(y=weight, color='r', linestyle='--', label=f'Weight ({weight:.1f} N)')
ax.axhline(y=0, color='k', alpha=0.3)
ax.fill_between(submersion * 100, 0, F_buoy, where=(F_buoy > 0), alpha=0.2, color='blue')
ax.set_xlabel('Submersion (%)')
ax.set_ylabel('Force (N)')
ax.set_title('Buoyancy Force vs Submersion\n(Capsule volume = πR²L + (4/3)πR³)')
ax.legend()
ax.grid(True, alpha=0.3)
plt.tight_layout()
plt.savefig(OUT_DIR / 'buoyancy_curve.png', dpi=100)
print(f"\nSaved: {OUT_DIR / 'buoyancy_curve.png'}")

# ========================================
# 4. CAPSULE-TRIANGLE GEOMETRY DIAGRAM
# ========================================
fig, ax = plt.subplots(figsize=(8, 6))

# Draw triangle (ground + step)
triangle = plt.Polygon([(0, 0), (3, 0), (3, 2), (1, 2), (1, 0.5), (0, 0.5)],
                       closed=True, fill=True, facecolor='lightgray', edgecolor='black')
ax.add_patch(triangle)

# Draw capsule (above step, mid-step-up)
capsule_y_base = 0.7
from matplotlib.patches import Circle, Rectangle
ax.add_patch(Rectangle((-0.3, capsule_y_base - 0.4 + 0.2), 0.6, 0.8, facecolor='cyan', alpha=0.5, edgecolor='blue'))
ax.add_patch(Circle((0, capsule_y_base - 0.4 + 0.2), 0.3, facecolor='cyan', alpha=0.5, edgecolor='blue'))
ax.add_patch(Circle((0, capsule_y_base + 0.4 + 0.2), 0.3, facecolor='cyan', alpha=0.5, edgecolor='blue'))

# Step arrow
ax.annotate('', xy=(1.0, 0.5), xytext=(1.0, 0.0),
            arrowprops=dict(arrowstyle='->', color='red', lw=2))
ax.text(1.05, 0.25, 'step height\n(0.20 m)', color='red', fontsize=10)

# Capsule axis ray
ax.plot([0, 0], [capsule_y_base - 0.4 + 0.2, capsule_y_base + 0.4 + 0.2], 'b-', linewidth=2)
ax.text(0.1, capsule_y_base, 'capsule axis\n(segment)', color='blue', fontsize=10)

ax.set_xlim(-1, 4)
ax.set_ylim(-0.5, 2.5)
ax.set_aspect('equal')
ax.set_title('Capsule vs Triangle Geometry (Step-Up)')
ax.grid(True, alpha=0.3)
plt.tight_layout()
plt.savefig(OUT_DIR / 'capsule_triangle_distance.png', dpi=100)
print(f"\nSaved: {OUT_DIR / 'capsule_triangle_distance.png'}")

# ========================================
# 5. RESULTS JSON
# ========================================
results = {
    "module": "character_physics",
    "capsule_defaults": {
        "radius_m": R,
        "total_height_m": height,
        "cylinder_half_length_m": half_height,
        "capsule_volume_m3": capsule_volume,
    },
    "physics_constants": {
        "water_density_kg_m3": rho_water,
        "gravity_m_s2": g,
        "character_mass_kg": mass,
        "character_weight_N": weight,
        "max_buoyancy_N": F_buoyancy_max,
        "net_force_full_submerge_N": F_buoyancy_max - weight,
    },
    "step_up": {
        "step_height_m": 0.20,
        "smoothing_function": "smoothstep(t) = 3t^2 - 2t^3",
        "smoothstep_at_0": 0,
        "smoothstep_at_1": 1,
        "smoothstep_derivative_at_0": 0,
        "smoothstep_derivative_at_1": 0,
        "rationale": "zero velocity at start/end eliminates snap",
    },
    "formulas": {
        "capsule_triangle_distance": "d(t) = |((A + t*(B-A)) - P0) . n_hat|  where n_hat = ((P1-P0) x (P2-P0)) / ||...||",
        "contact_condition": "min_t |d(t)| <= R",
        "buoyancy": "F = rho_water * g * V_submerged",
        "smoothstep": "s(t) = 3t^2 - 2t^3, s'(t) = 6t - 6t^2 = 6t(1-t)",
    },
    "next_steps_for_implementation": [
        "Implement capsule_vs_triangle_distance() in p3_character_physics.zig",
        "Use segment parameter t* from d/dt|d|²=0 closed-form",
        "Clamp t* to [0, 1] for capsule ends",
        "Step-up: if vertical separation < 0.20m, apply smoothstep over 0.2s",
        "Swimming: integrate buoyancy into integrateCharacter(dt)",
    ],
}
with open(OUT_DIR / 'character_physics_results.json', 'w') as f:
    json.dump(results, f, indent=2)
print(f"\nSaved: {OUT_DIR / 'character_physics_results.json'}")

# ========================================
# 6. Write formulas to text file
# ========================================
with open(OUT_DIR / 'character_physics_formulas.txt', 'w') as f:
    f.write("P3 ENGINE — CHARACTER PHYSICS DERIVATIONS\n")
    f.write("=" * 60 + "\n\n")
    f.write("1. CAPSULE vs TRIANGLE DISTANCE\n")
    f.write("-" * 40 + "\n")
    f.write("Given:\n")
    f.write("  Capsule axis segment: A → B (A and B are sphere centers at each end)\n")
    f.write("  Capsule radius: R\n")
    f.write("  Triangle: P0, P1, P2\n\n")
    f.write("Distance from a point P on segment to triangle plane:\n")
    f.write("  P(t) = A + t*(B - A), t ∈ [0, 1]\n")
    f.write("  n = (P1 - P0) × (P2 - P0)\n")
    f.write("  n_hat = n / ||n||\n")
    f.write("  d(t) = |((P(t) - P0) . n_hat|\n\n")
    f.write("Closest point on segment to plane: t* from d|d|²/dt = 0\n")
    f.write("  d|d|²/dt = 2 * (P(t) - P0) . n_hat * (B - A) . n_hat\n")
    f.write("  => t* = -((A - P0) . n_hat) / ((B - A) . n_hat)\n")
    f.write("  Clamp t* to [0, 1]\n\n")
    f.write("Contact condition: |d(t*)| <= R\n")
    f.write("Barycentric test: project P(t*) onto triangle, check barycentric coords\n\n")
    f.write("=" * 60 + "\n\n")
    f.write("2. STEP-UP SMOOTHING (Hermite Cubic)\n")
    f.write("-" * 40 + "\n")
    f.write("smoothstep(t) = 3t² - 2t³, t ∈ [0, 1]\n")
    f.write("  smoothstep(0) = 0, smoothstep(1) = 1\n")
    f.write("  smoothstep'(0) = 0, smoothstep'(1) = 0\n")
    f.write("  → Zero velocity at boundaries eliminates snap\n\n")
    f.write("Step height h = 0.20 m (UE MAX_STEP_SIDE_Z default)\n")
    f.write("Step duration T = 0.2 s (gameplay feel)\n")
    f.write("Final position: y(t) = y_start + h * smoothstep(t/T)\n\n")
    f.write("=" * 60 + "\n\n")
    f.write("3. BUOYANCY (Archimedes)\n")
    f.write("-" * 40 + "\n")
    f.write("F_buoyancy = ρ_water * g * V_submerged\n")
    f.write("  ρ_water = 1000 kg/m³\n")
    f.write("  g = 9.81 m/s²\n")
    f.write("  V_submerged = V_capsule * submersion_fraction\n")
    f.write("  V_capsule = π * R² * (2 * L_half) + (4/3) * π * R³\n")
    f.write("    (cylinder + 2 hemispheres)\n\n")
    f.write("For 80kg character with R=0.4m, L_half=0.5m:\n")
    f.write(f"  V_capsule = {capsule_volume:.4f} m³\n")
    f.write(f"  F_max_buoyancy = {F_buoyancy_max:.2f} N\n")
    f.write(f"  Weight = {weight:.2f} N\n")
    f.write(f"  Net (submerged) = {F_buoyancy_max - weight:.2f} N upward\n")
    f.write("  → Character floats (positive buoyancy)\n")
print(f"Saved: {OUT_DIR / 'character_physics_formulas.txt'}")

print("\n" + "=" * 60)
print("Character physics calculations COMPLETE")
print("=" * 60)
print(f"Output dir: {OUT_DIR}")
