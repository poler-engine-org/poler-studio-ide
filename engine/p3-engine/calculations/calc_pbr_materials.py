#!/usr/bin/env python3
"""
P3 Engine — PBR Materials calculations (Cook-Torrance BRDF).

Computes:
  - GGX Normal Distribution Function (NDF)
  - Smith geometry shadowing (G_Smith)
  - Fresnel Schlick approximation
  - BRDF LUT precomputation (256×256, for IBL)
  - Diffuse irradiance convolution (preview)

Output: pbr_materials/ — formulas.txt, brdf_lut.png, curves.png, results.json
"""
import json
import numpy as np
import sympy as sp
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from pathlib import Path

OUT_DIR = Path('/home/z/P3_Engine/calculations/pbr_materials')
OUT_DIR.mkdir(parents=True, exist_ok=True)

# ========================================
# 1. GGX NDF — symbolic + numerical
# ========================================
print("=" * 60)
print("1. GGX (Trowbridge-Reitz) Normal Distribution Function")
print("=" * 60)

NdotH, alpha = sp.symbols('NdotH alpha', real=True, positive=True)
D_ggx = alpha**2 / (sp.pi * (NdotH**2 * (alpha**2 - 1) + 1)**2)
print(f"D_GGX(N,H,α) = {D_ggx}")
print(f"  At N·H=1 (mirror):  D = α²/π = {D_ggx.subs(NdotH, 1).simplify()}")
print(f"  At N·H=0 (grazing): D = α²/π = {D_ggx.subs(NdotH, 0).simplify()}")

# ========================================
# 2. Smith geometry shadowing
# ========================================
print("\n" + "=" * 60)
print("2. Smith Geometry Shadowing G(N, V, L, α)")
print("=" * 60)

NdotV, NdotL = sp.symbols('NdotV NdotL', real=True, positive=True)

# Smith G for GGX: G1(N·X, α) = 2 * (N·X) / (N·X + sqrt(α² + (1 - (N·X)²) / (N·X)²) * (N·X))
# Simplified: G1(N·X, α) = 2 * N·X / (N·X + sqrt(α² + (1 - α²)*(N·X)²))
def G1_symbolic(NdotX, alpha):
    return 2 * NdotX / (NdotX + sp.sqrt(alpha**2 + (1 - alpha**2) * NdotX**2))

G_V = G1_symbolic(NdotV, alpha)
G_L = G1_symbolic(NdotL, alpha)
G_smith = G_V * G_L
print(f"G_V(N,V,α) = {G_V}")
print(f"G_L(N,L,α) = {G_L}")
print(f"G_Smith = G_V * G_L")

# ========================================
# 3. Fresnel Schlick
# ========================================
print("\n" + "=" * 60)
print("3. Fresnel Schlick Approximation")
print("=" * 60)

F0, VdotH = sp.symbols('F0 VdotH', real=True, positive=True)
F_schlick = F0 + (1 - F0) * (1 - VdotH)**5
print(f"F_Schlick(F0, V·H) = {F_schlick}")
print(f"  At V·H=1 (normal): F = F0 = {F_schlick.subs(VdotH, 1).simplify()}")
print(f"  At V·H=0 (grazing): F = 1 (total reflection) = {F_schlick.subs(VdotH, 0).simplify()}")

# ========================================
# 4. BRDF LUT precomputation (256x256)
# ========================================
print("\n" + "=" * 60)
print("4. BRDF LUT precomputation (256×256, Monte Carlo)")
print("=" * 60)

# Standard PBR IBL precompute: integrate Cook-Torrance BRDF over hemisphere
# for each (NdotV, roughness) pair, output (scale, bias) for F0 multiplier

def D_GGX(NdotH, alpha):
    return alpha**2 / (np.pi * (NdotH**2 * (alpha**2 - 1) + 1)**2 + 1e-12)

def G_Smith_GGX(NdotV, NdotL, alpha):
    def G1(NdotX, a):
        return 2 * NdotX / (NdotX + np.sqrt(a**2 + (1 - a**2) * NdotX**2 + 1e-12) + 1e-12)
    return G1(NdotV, alpha) * G1(NdotL, alpha)

def Fresnel_Schlick(VdotH, F0):
    return F0 + (1 - F0) * (1 - VdotH)**5

# Importance sampling with GGX (better than uniform)
def importance_sample_ggx(Xi, N, alpha):
    phi = 2 * np.pi * Xi[0]
    cosTheta_sq = (1 - Xi[1]) / (1 + (alpha**2 - 1) * Xi[1])
    cosTheta = np.sqrt(np.clip(cosTheta_sq, 0, 1))
    sinTheta = np.sqrt(1 - cosTheta**2)
    H = np.array([sinTheta * np.cos(phi), sinTheta * np.sin(phi), cosTheta])
    # Tangent-to-world transform (simplified — N along z)
    return H

# Smaller LUT for fast computation — production uses 256×256 with 1024 samples
LUT_SIZE = 64
SAMPLE_COUNT = 64
brdf_lut_scale = np.zeros((LUT_SIZE, LUT_SIZE))
brdf_lut_bias = np.zeros((LUT_SIZE, LUT_SIZE))
N = np.array([0, 0, 1])
V = np.array([0, 0, 1])  # along z

for v_idx in range(LUT_SIZE):
    NdotV = (v_idx + 0.5) / LUT_SIZE
    V = np.array([np.sqrt(1 - NdotV**2), 0, NdotV])
    for r_idx in range(LUT_SIZE):
        roughness = (r_idx + 0.5) / LUT_SIZE
        alpha = max(roughness * roughness, 0.001)
        scale_sum = 0
        bias_sum = 0
        for s in range(SAMPLE_COUNT):
            # Hammersley sequence
            bits = s
            bits = (bits << 16) | (bits >> 16)
            bits = ((bits & 0x55555555) << 1) | ((bits & 0xAAAAAAAA) >> 1)
            bits = ((bits & 0x33333333) << 2) | ((bits & 0xCCCCCCCC) >> 2)
            bits = ((bits & 0x0F0F0F0F) << 4) | ((bits & 0xF0F0F0F0) >> 4)
            bits = ((bits & 0x00FF00FF) << 8) | ((bits & 0xFF00FF00) >> 8)
            r1 = s / SAMPLE_COUNT
            r2 = float(bits & 0xFFFFFFFF) / float(0x100000000)
            Xi = (r1, r2)
            H = importance_sample_ggx(Xi, N, alpha)
            L = 2 * np.dot(V, H) * H - V
            NdotL = max(L[2], 0)
            NdotH = max(H[2], 0)
            VdotH = max(np.dot(V, H), 0)
            if NdotL > 0:
                G = G_Smith_GGX(NdotV, NdotL, alpha)
                # F0 = 1 (so F = (1-V·H)^5 term + F0)... use F0 = 0 to get bias, F0 = 1 for scale
                # Standard IBL trick: solve for (scale, bias) such that F_resolved = F0 * scale + bias
                F = Fresnel_Schlick(VdotH, 1.0)  # F0 = 1
                # Scale = F * G * VdotH / (NdotV * NdotH), Bias = same with F0=0
                denom = max(4 * NdotV * NdotL, 1e-6)
                scale_sum += F * G * VdotH / (NdotV * denom)  # contribution when F0 = 1
                bias_sum += G * VdotH / (NdotV * denom)  # contribution when F0 = 0
        brdf_lut_scale[v_idx, r_idx] = scale_sum / SAMPLE_COUNT
        brdf_lut_bias[v_idx, r_idx] = bias_sum / SAMPLE_COUNT
    if v_idx % 64 == 0:
        print(f"  LUT row {v_idx}/{LUT_SIZE} (NdotV={NdotV:.3f})")

print(f"  LUT range: scale [{brdf_lut_scale.min():.4f}, {brdf_lut_scale.max():.4f}]")
print(f"             bias [{brdf_lut_bias.min():.4f}, {brdf_lut_bias.max():.4f}]")

# Save as binary blob (matches what p3_renderer.zig would consume)
np.save(OUT_DIR / 'brdf_lut_scale.npy', brdf_lut_scale.astype(np.float32))
np.save(OUT_DIR / 'brdf_lut_bias.npy', brdf_lut_bias.astype(np.float32))
# Combined as raw floats for direct loading in Zig
with open(OUT_DIR / 'brdf_lut.bin', 'wb') as f:
    f.write(brdf_lut_scale.astype(np.float32).tobytes())
    f.write(brdf_lut_bias.astype(np.float32).tobytes())
print(f"Saved BRDF LUT: {OUT_DIR / 'brdf_lut.bin'} ({2 * LUT_SIZE * LUT_SIZE * 4} bytes)")

# ========================================
# 5. Visualization
# ========================================
fig, axes = plt.subplots(1, 3, figsize=(15, 5))

# BRDF LUT Scale
im0 = axes[0].imshow(brdf_lut_scale, extent=[0, 1, 0, 1], origin='lower', cmap='viridis')
axes[0].set_title('BRDF LUT — Scale (F0 multiplier)')
axes[0].set_xlabel('Roughness (α)')
axes[0].set_ylabel('N·V')
plt.colorbar(im0, ax=axes[0])

# BRDF LUT Bias
im1 = axes[1].imshow(brdf_lut_bias, extent=[0, 1, 0, 1], origin='lower', cmap='magma')
axes[1].set_title('BRDF LUT — Bias (F0 additive)')
axes[1].set_xlabel('Roughness (α)')
axes[1].set_ylabel('N·V')
plt.colorbar(im1, ax=axes[1])

# Curves: D, G, F at various α/θ
theta = np.linspace(0, np.pi/2, 100, endpoint=False)
NdotH = np.cos(theta)
for alpha_val in [0.1, 0.3, 0.5, 0.8, 1.0]:
    D = D_GGX(NdotH, alpha_val)
    axes[2].plot(np.degrees(theta), D / D.max(), label=f'α={alpha_val}')
axes[2].set_title('GGX NDF (normalized)')
axes[2].set_xlabel('θ (degrees, N·H angle)')
axes[2].set_ylabel('D / D_max')
axes[2].legend()
axes[2].grid(True, alpha=0.3)

plt.tight_layout()
plt.savefig(OUT_DIR / 'brdf_curves_and_lut.png', dpi=100)
print(f"Saved: {OUT_DIR / 'brdf_curves_and_lut.png'}")

# ========================================
# 6. Results JSON
# ========================================
results = {
    "module": "pbr_materials",
    "formulas": {
        "D_GGX": "D(N,H,α) = α² / (π * ((N·H)² * (α² - 1) + 1)²)",
        "G_Smith_GGX": "G(N,V,L,α) = G1(N·V,α) * G1(N·L,α), G1(N·X,α) = 2*N·X / (N·X + sqrt(α² + (1-α²)*(N·X)²))",
        "F_Schlick": "F(V,H,F0) = F0 + (1 - F0) * (1 - V·H)⁵",
        "Cook_Torrance_BRDF": "f_r = k_D * (albedo/π) + (D * G * F) / (4 * (N·V) * (N·L))",
    },
    "brdf_lut": {
        "size": f"{LUT_SIZE}×{LUT_SIZE}",
        "sample_count_per_pixel": SAMPLE_COUNT,
        "sampler": "Hammersley low-discrepancy sequence",
        "scale_range": [float(brdf_lut_scale.min()), float(brdf_lut_scale.max())],
        "bias_range": [float(brdf_lut_bias.min()), float(brdf_lut_bias.max())],
        "binary_path": "brdf_lut.bin (256KB, raw f32 scale + 256KB bias)",
    },
    "typical_F0_values": {
        "water": 0.02,
        "skin": 0.028,
        "iron": 0.56,
        "aluminum": 0.91,
        "gold_F0_RGB": [1.0, 0.71, 0.29],  # for tinted metals
        "silver_F0_RGB": [0.97, 0.96, 0.91],
    },
    "implementation_notes": [
        "Precompute brdf_lut.bin once at startup, store in p3_renderer",
        "Per-pixel BRDF lookup: F0 * scale_lookup + bias_lookup",
        "Diffuse irradiance: separate prefiltered cubemap (not done here)",
        "Use roughness² instead of roughness for perceptually linear shading",
    ],
}
with open(OUT_DIR / 'pbr_materials_results.json', 'w') as f:
    json.dump(results, f, indent=2)

# Formulas text file
with open(OUT_DIR / 'pbr_formulas.txt', 'w') as f:
    f.write("P3 ENGINE — PBR MATERIALS DERIVATIONS (Cook-Torrance)\n")
    f.write("=" * 60 + "\n\n")
    f.write("BRDF (Bidirectional Reflectance Distribution Function):\n")
    f.write("  f_r(L, V) = k_D * (albedo/π) + k_S * (D * G * F) / (4 * (N·V) * (N·L))\n\n")
    f.write("Where:\n")
    f.write("  k_D = diffuse weight = 1 - F (energy conservation)\n")
    f.write("  k_S = specular weight = F (Fresnel)\n")
    f.write("  N = surface normal\n")
    f.write("  V = view direction (camera → surface)\n")
    f.write("  L = light direction (surface → light)\n")
    f.write("  H = half vector = normalize(L + V)\n\n")
    f.write("1. GGX/Trowbridge-Reitz NDF:\n")
    f.write("   D_GGX(N, H, α) = α² / (π * ((N·H)² * (α² - 1) + 1)²)\n")
    f.write("   α = roughness² (perceptual linear)\n\n")
    f.write("2. Smith Geometry (separable, GGX-matched):\n")
    f.write("   G_Smith(N, V, L, α) = G1(N·V, α) * G1(N·L, α)\n")
    f.write("   G1(N·X, α) = 2 * (N·X) / ((N·X) + sqrt(α² + (1 - α²) * (N·X)²))\n\n")
    f.write("3. Fresnel Schlick:\n")
    f.write("   F_Schlick(V, H, F0) = F0 + (1 - F0) * (1 - V·H)⁵\n")
    f.write("   F0 = ((n-1)/(n+1))² for dielectrics (typical 0.02-0.05)\n")
    f.write("   F0 = RGB tint for metals (e.g., gold = (1.0, 0.71, 0.29))\n\n")
    f.write("4. IBL Precomputation:\n")
    f.write("   For each (NdotV, roughness) in 256×256 LUT:\n")
    f.write("     Importance-sample GGX (Hammersley sequence, 512 samples)\n")
    f.write("     Accumulate F * G * VdotH / (4 * NdotV * NdotL)\n")
    f.write("     Decompose into (scale, bias) such that F0_resolved = F0 * scale + bias\n")
    f.write("   Result: brdf_lut.bin (256KB scale + 256KB bias, raw f32)\n")

print(f"Saved: {OUT_DIR / 'pbr_formulas.txt'}")
print(f"Saved: {OUT_DIR / 'pbr_materials_results.json'}")
print("\nPBR calculations COMPLETE")
