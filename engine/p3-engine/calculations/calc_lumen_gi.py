#!/usr/bin/env python3
"""P3 Engine — Lumen GI calculations (Spherical Harmonics, SDF)."""
import json, numpy as np, sympy as sp
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from pathlib import Path

OUT = Path('/home/z/P3_Engine/calculations/lumen_gi'); OUT.mkdir(parents=True, exist_ok=True)

# 1. SPHERICAL HARMONICS L2 (9 coefficients) — closed form
# Y_0^0  = 1/(2*sqrt(pi))
# Y_1^-1 = -sqrt(3/(4*pi)) * y/r
# Y_1^0  = sqrt(3/(4*pi)) * z/r
# Y_1^1  = -sqrt(3/(4*pi)) * x/r  (note sign convention)
# Y_2^-2 = sqrt(15/(4*pi)) * (xy/r²)/sqrt(3)  ... etc.

# Symbolic forms:
theta, phi = sp.symbols('theta phi', real=True)
Y00 = sp.sqrt(sp.Rational(1, 4) / sp.pi)  # = 1/(2*sqrt(pi))
Y1m1 = -sp.sqrt(sp.Rational(3, 4) / sp.pi) * sp.sin(theta) * sp.sin(phi)
Y10 = sp.sqrt(sp.Rational(3, 4) / sp.pi) * sp.cos(theta)
Y11 = sp.sqrt(sp.Rational(3, 4) / sp.pi) * sp.sin(theta) * sp.cos(phi)
Y2m2 = sp.sqrt(sp.Rational(15, 4) / sp.pi) * sp.sin(theta)**2 * sp.sin(phi) * sp.cos(phi) / sp.sqrt(3)
Y2m1 = -sp.sqrt(sp.Rational(15, 4) / sp.pi) * sp.sin(theta) * sp.cos(theta) * sp.sin(phi) / sp.sqrt(3)
Y20 = sp.sqrt(sp.Rational(5, 16) / sp.pi) * (3*sp.cos(theta)**2 - 1)
Y21 = sp.sqrt(sp.Rational(15, 4) / sp.pi) * sp.sin(theta) * sp.cos(theta) * sp.cos(phi) / sp.sqrt(3)
Y22 = sp.sqrt(sp.Rational(15, 16) / sp.pi) * sp.sin(theta)**2 * sp.cos(2*phi)

print("Spherical Harmonics L2 (9 coefficients):")
print(f"  Y_0^0  = {Y00}")
print(f"  Y_1^-1 = {Y1m1}")
print(f"  Y_1^0  = {Y10}")
print(f"  Y_1^1  = {Y11}")
print(f"  Y_2^-2 = {Y2m2}")
print(f"  Y_2^-1 = {Y2m1}")
print(f"  Y_2^0  = {Y20}")
print(f"  Y_2^1  = {Y21}")
print(f"  Y_2^2  = {Y22}")

# 2. SDF (Signed Distance Field) generation — Jump Flooding Algorithm concept
# For each texel/voxel, find nearest surface point
# Distance field: positive outside, negative inside, 0 on surface
# Used for ray-marching (Lumen traces rays against SDF, not actual geometry)

# Simple 2D example: circle of radius 0.3 at center of 64x64 grid
N = 64
xs = np.linspace(-1, 1, N)
ys = np.linspace(-1, 1, N)
X, Y = np.meshgrid(xs, ys)
sdf_circle = np.sqrt(X**2 + Y**2) - 0.3  # outside > 0, inside < 0, surface = 0

fig, axes = plt.subplots(1, 2, figsize=(12, 5))
im0 = axes[0].imshow(sdf_circle, cmap='RdBu_r', extent=[-1, 1, -1, 1])
axes[0].contour(X, Y, sdf_circle, levels=[0], colors='black', linewidths=2)  # surface
axes[0].set_title('SDF of a Circle\n(red=inside, blue=outside, black=surface)')
axes[0].set_xlabel('X'); axes[0].set_ylabel('Y')
plt.colorbar(im0, ax=axes[0])

# 3. SH-based irradiance approximation (3-band Lambert)
# E(n) ≈ c0 + c1*n + c2*(3*n_z²-1)/2 + c3*(n_x²-n_y²)
# For diffuse irradiance from a single light direction:
light_dir = np.array([0.5, 0.7, 0.3]) / np.sqrt(0.25 + 0.49 + 0.09)
dirs = np.linspace(0, 2*np.pi, 100)
cos_theta = np.cos(dirs)  # simplified 2D irradiance

# SH projection of cos(theta) (ClampedCos)
# SH coefficients: only first 3 are non-zero for ClampedCos
# c0 = sqrt(pi)/3, c1 = sqrt(pi/3), c2 = sqrt(pi)*sqrt(5)/(4*sqrt(5)) etc.
# Use literature values: ClampedCos L1 = (pi/3, pi/3 * sqrt(3), 0, 0)
sh_coeffs = np.array([np.pi/3, np.pi/3 * np.sqrt(3)/np.sqrt(3), 0, 0, 0, 0, 0, 0, 0])[:3]
# Reconstruct irradiance
def sh_reconstruct(theta_rad, coeffs):
    # L1 SH basis (simplified)
    return coeffs[0] + coeffs[1] * np.cos(theta_rad) + coeffs[2] * (3*np.cos(theta_rad)**2 - 1) / 2
sh_recon = sh_reconstruct(dirs, sh_coeffs)
cos_clamped = np.maximum(cos_theta, 0)

axes[1].plot(np.degrees(dirs), cos_clamped, 'b-', linewidth=3, label='Clamped cos(θ)')
axes[1].plot(np.degrees(dirs), sh_recon, 'r--', linewidth=2, label='SH L2 reconstruction')
axes[1].set_xlabel('θ (degrees)')
axes[1].set_ylabel('Irradiance')
axes[1].set_title('SH Irradiance Approximation\n(L2 captures dominant direction)')
axes[1].legend(); axes[1].grid(True, alpha=0.3)
plt.tight_layout()
plt.savefig(OUT / 'lumen_sh_sdf.png', dpi=100)
print(f"Saved: {OUT / 'lumen_sh_sdf.png'}")

# 4. Results JSON
results = {
    "module": "lumen_gi",
    "formulas": {
        "SH_L2_coefficients": "9 coefficients: Y_0^0, Y_1^-1..1, Y_2^-2..2",
        "irradiance_reconstruction": "E(n) ≈ c0 + c1*n + c2*(3n_z²-1)/2 + c3*(n_x²-n_y²)",
        "sdf_definition": "f(p) = signed distance from p to nearest surface; positive=outside, negative=inside",
        "ray_marching_sdf": "t += min(distance, max_step); stop when |f| < ε",
        "clamped_cos_sh": "ClampedCos projected onto L2: dominant direction L1=π/3 along n",
    },
    "lumen_pipeline_steps": [
        "1. Precompute SDF per mesh (per-vertex SDF, merged to global SDF atlas)",
        "2. Trace rays against SDF (not actual geometry) — much faster than BVH",
        "3. Hit points cached in surface cache (parameterized by UVs)",
        "4. Final gather: spherical harmonics at each surface point → diffuse irradiance",
        "5. Specular: radiance cache (roughness-indexed mip chain)",
        "6. Combine: direct lighting + indirect diffuse (SH) + indirect specular (radiance cache)",
    ],
    "memory_estimate": {
        "sdf_atlas_64x64_per_mesh": "16KB per mesh (f32 distances)",
        "surface_cache_512x512": "1MB per scene (RGBA8 radiance)",
        "sh_per_probe_9_coeffs": "144 bytes (9 × f32 RGB) per probe",
        "probe_grid_8x8x8": "73KB (512 probes × 144 bytes)",
    },
    "implementation_notes": [
        "Start with diffuse-only GI (skip specular cache — too complex)",
        "Use SH L2 (9 coeffs) — captures 1 bounce",
        "SDF generation: Jump Flooding Algorithm on GPU, or CPU per-mesh",
        "Surface cache update: dirty-rect scheme (only update changed cells)",
        "Lumen reference: Epic SIGGRAPH 2021 talk",
    ],
}
with open(OUT / 'lumen_gi_results.json', 'w') as f:
    json.dump(results, f, indent=2)

with open(OUT / 'lumen_formulas.txt', 'w') as f:
    f.write("P3 ENGINE — LUMEN GI DERIVATIONS\n")
    f.write("=" * 60 + "\n\n")
    f.write("1. SPHERICAL HARMONICS L2 (9 coefficients)\n")
    f.write("-" * 40 + "\n")
    f.write("Y_0^0  = 1/(2√π)\n")
    f.write("Y_1^-1 = -√(3/4π) · sin(θ)·sin(φ)\n")
    f.write("Y_1^0  =  √(3/4π) · cos(θ)\n")
    f.write("Y_1^1  =  √(3/4π) · sin(θ)·cos(φ)\n")
    f.write("Y_2^-2 =  √(15/4π)/√3 · sin²(θ)·sin(φ)·cos(φ)\n")
    f.write("Y_2^-1 = -√(15/4π)/√3 · sin(θ)·cos(θ)·sin(φ)\n")
    f.write("Y_2^0  =  √(5/16π) · (3cos²(θ) - 1)\n")
    f.write("Y_2^1  =  √(15/4π)/√3 · sin(θ)·cos(θ)·cos(φ)\n")
    f.write("Y_2^2  =  √(15/16π) · sin²(θ)·cos(2φ)\n\n")
    f.write("2. SIGNED DISTANCE FIELD (SDF)\n")
    f.write("-" * 40 + "\n")
    f.write("f(p) = signed distance from p to nearest surface\n")
    f.write("  f > 0: outside;  f < 0: inside;  f = 0: on surface\n")
    f.write("Ray march: t += max(min_distance, ε)\n")
    f.write("  Stop when |f(p)| < ε (hit) or t > max_t (miss)\n")
    f.write("  SDF allows large steps in empty space (vs fixed-step raycast)\n\n")
    f.write("3. CLAMPED COSINE SH PROJECTION\n")
    f.write("-" * 40 + "\n")
    f.write("max(cos(θ), 0) projected onto SH L1 (only first 3 coeffs non-zero):\n")
    f.write("  c0 = π/3\n")
    f.write("  c1 = π/3 * (3·cos(θ_light)·dir) → dominant direction\n")
    f.write("Reconstruction: E(n) ≈ c0 + c1·n + c2·(3n_z²-1)/2\n")

print(f"Lumen GI calculations COMPLETE")
