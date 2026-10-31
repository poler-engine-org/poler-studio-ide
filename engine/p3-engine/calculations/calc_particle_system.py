#!/usr/bin/env python3
"""
P3 Engine — Particle System calculations (Bezier curves, curl noise, gravity).
"""
import json
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from pathlib import Path

OUT_DIR = Path('/home/z/P3_Engine/calculations/particle_system')
OUT_DIR.mkdir(parents=True, exist_ok=True)

# 1. CUBIC BEZIER LIFE CURVE
# Particle lifetime parameterized by t ∈ [0, 1]
# Bezier control points: P0=(0,0), P1=(0.3, 0.5), P2=(0.7, 1.0), P3=(1.0, 0.0)
# (spawn low → ramp up → fade out)
P0, P1, P2, P3 = np.array([0, 0]), np.array([0.3, 0.5]), np.array([0.7, 1.0]), np.array([1.0, 0])

def bezier_cubic(t):
    return (1-t)**3 * P0 + 3*(1-t)**2 * t * P1 + 3*(1-t) * t**2 * P2 + t**3 * P3

t = np.linspace(0, 1, 100)
life_curve = np.array([bezier_cubic(ti) for ti in t])

fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(12, 5))
ax1.plot(life_curve[:, 0], life_curve[:, 1], 'b-', linewidth=3, label='Life curve (intensity vs t)')
ax1.plot([P0[0], P1[0], P2[0], P3[0]], [P0[1], P1[1], P2[1], P3[1]], 'ro--', markersize=10, label='Control points')
ax1.set_xlabel('Particle age (normalized)')
ax1.set_ylabel('Intensity (alpha multiplier)')
ax1.set_title('Cubic Bezier Life Curve\n(spawn low → ramp up → fade out)')
ax1.legend()
ax1.grid(True, alpha=0.3)

# 2. CURL NOISE (approximation: cross of 2D gradients of pseudo-potential fields)
def potential(x, y, freq=0.5, seed=0):
    # Simple Perlin-like potential
    return np.sin(x * freq + seed) * np.cos(y * freq + seed * 0.7)

def curl_2d(x, y, freq=0.5, eps=0.01):
    # curl(z-potential field in 2D) = (dP/dy, -dP/dx)
    dPdx = (potential(x + eps, y, freq) - potential(x - eps, y, freq)) / (2 * eps)
    dPdy = (potential(x, y + eps, freq) - potential(x, y - eps, freq)) / (2 * eps)
    return np.array([dPdy, -dPdx])

# Simulate 100 particles in curl field
np.random.seed(42)
particles = np.random.uniform(-5, 5, (100, 2))
velocities = np.zeros((100, 2))
positions_history = [particles.copy()]
for step in range(50):
    for i in range(len(particles)):
        v = curl_2d(particles[i, 0], particles[i, 1])
        velocities[i] = velocities[i] * 0.95 + v * 0.05
    particles += velocities * 0.3
    positions_history.append(particles.copy())

for i, hist in enumerate(positions_history[::5]):
    ax2.scatter(hist[:, 0], hist[:, 1], s=10, alpha=min(1.0, 0.3 + i * 0.1), c='blue')
ax2.set_xlabel('X')
ax2.set_ylabel('Y')
ax2.set_title('Curl Noise Field — Particle Trajectories\n(50 steps, 100 particles)')
ax2.set_aspect('equal')
ax2.grid(True, alpha=0.3)

plt.tight_layout()
plt.savefig(OUT_DIR / 'particle_curves.png', dpi=100)

# 3. GRAVITY INTEGRATION: semi-implicit Euler vs Verlet
def integrate_euler(p0, v0, g=9.81, dt=0.016, steps=60):
    p, v = p0.copy(), v0.copy()
    traj = [p.copy()]
    for _ in range(steps):
        v += np.array([0, -g, 0]) * dt  # semi-implicit Euler
        p += v * dt
        traj.append(p.copy())
    return np.array(traj)

def integrate_verlet(p0, v0, g=9.81, dt=0.016, steps=60):
    p, v = p0.copy(), v0.copy()
    p_prev = p - v * dt
    traj = [p.copy()]
    for _ in range(steps):
        a = np.array([0, -g, 0])
        p_new = 2 * p - p_prev + a * dt**2
        p_prev = p
        p = p_new
        traj.append(p.copy())
    return np.array(traj)

p0, v0 = np.array([0.0, 5.0, 0.0]), np.array([3.0, 8.0, 0.0])
traj_euler = integrate_euler(p0, v0)
traj_verlet = integrate_verlet(p0, v0)
analytical_y = 5 + 8 * np.arange(61) * 0.016 - 0.5 * 9.81 * (np.arange(61) * 0.016)**2

fig, ax = plt.subplots(figsize=(8, 5))
ax.plot(traj_euler[:, 0], traj_euler[:, 1], 'b-', linewidth=2, label='Semi-implicit Euler')
ax.plot(traj_verlet[:, 0], traj_verlet[:, 1], 'g--', linewidth=2, label='Verlet')
ax.plot(traj_euler[:, 0], analytical_y, 'r:', linewidth=2, label='Analytical (ground truth)')
ax.set_xlabel('X (m)')
ax.set_ylabel('Y (m)')
ax.set_title('Gravity Integration: Semi-Implicit Euler vs Verlet\n(both match analytical for short time)')
ax.legend()
ax.grid(True, alpha=0.3)
plt.tight_layout()
plt.savefig(OUT_DIR / 'integration_comparison.png', dpi=100)

results = {
    "module": "particle_system",
    "formulas": {
        "cubic_bezier": "B(t) = (1-t)³ P0 + 3(1-t)² t P1 + 3(1-t) t² P2 + t³ P3",
        "curl_2d": "curl = (∂P/∂y, -∂P/∂x)  where P is potential field",
        "semi_implicit_euler": "v' = v + a*dt;  p' = p + v'*dt",
        "verlet": "p_new = 2*p - p_prev + a*dt²",
    },
    "life_curve_control_points": {
        "P0_spawn": [0.0, 0.0],
        "P1_ramp_up": [0.3, 0.5],
        "P2_peak": [0.7, 1.0],
        "P3_fade_out": [1.0, 0.0],
        "rationale": "particle fades in, peaks near mid-life, fades out",
    },
    "integration_test": {
        "initial_position": list(p0),
        "initial_velocity": list(v0),
        "max_error_euler_vs_analytical": float(np.max(np.abs(traj_euler[:, 1] - analytical_y))),
        "max_error_verlet_vs_analytical": float(np.max(np.abs(traj_verlet[:, 1] - analytical_y))),
        "conclusion": "both methods accurate for short time (60 frames = 1s). Verlet better for long simulations",
    },
    "implementation_notes": [
        "Use semi-implicit Euler for sparks (short-lived, <1s)",
        "Use Verlet for cloth (long simulations, energy conservation)",
        "Curl noise: precompute potential field at 16x16 grid, sample with bilinear interpolation",
        "Life curves: cubic Bezier with 4 control points per particle emitter (intensity, size, color)",
    ],
}
with open(OUT_DIR / 'particle_system_results.json', 'w') as f:
    json.dump(results, f, indent=2)

with open(OUT_DIR / 'particle_formulas.txt', 'w') as f:
    f.write("P3 ENGINE — PARTICLE SYSTEM DERIVATIONS\n")
    f.write("=" * 60 + "\n\n")
    f.write("1. CUBIC BEZIER LIFE CURVE\n")
    f.write("-" * 40 + "\n")
    f.write("B(t) = (1-t)³ P0 + 3(1-t)² t P1 + 3(1-t) t² P2 + t³ P3, t ∈ [0,1]\n")
    f.write("  Used for: intensity, size, color over particle lifetime\n")
    f.write("  Per-emitter: 4 control points × 3 properties = 12 floats\n\n")
    f.write("2. CURL NOISE (turbulence)\n")
    f.write("-" * 40 + "\n")
    f.write("curl(z) = (∂P/∂y, -∂P/∂x, 0)  where P is Perlin noise potential\n")
    f.write("  Provides divergence-free (volume-preserving) velocity field\n")
    f.write("  Used for: smoke, dust, fog particles\n\n")
    f.write("3. GRAVITY INTEGRATION\n")
    f.write("-" * 40 + "\n")
    f.write("Semi-implicit Euler:\n")
    f.write("  v' = v + a*dt;  p' = p + v'*dt\n")
    f.write("  Fast, symplectic for short simulations, OK for sparks\n\n")
    f.write("Verlet:\n")
    f.write("  p_new = 2*p - p_prev + a*dt²\n")
    f.write("  Energy-conserving, better for long simulations (cloth, hair)\n")

print(f"Saved all particle system outputs to {OUT_DIR}")
print(f"Particle system calculations COMPLETE")
