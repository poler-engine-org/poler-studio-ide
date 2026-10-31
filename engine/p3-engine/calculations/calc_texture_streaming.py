#!/usr/bin/env python3
"""
P3 Engine — Texture Streaming calculations (mipmap λ, anisotropic ellipse).
"""
import json
import numpy as np
import sympy as sp
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from pathlib import Path

OUT_DIR = Path('/home/z/P3_Engine/calculations/texture_streaming')
OUT_DIR.mkdir(parents=True, exist_ok=True)

# 1. MIPMAP LEVEL DERIVATION (closed form)
# λ = log2(max(|∂u/∂x|, |∂v/∂x|, |∂u/∂y|, |∂v/∂y|) * width_in_texels)
# Or simplified (square footprint): λ = log2(ρ) where ρ is pixel/texel ratio

du_dx, dv_dx, du_dy, dv_dy, w_tex = sp.symbols('du_dx dv_dx du_dy dv_dy w_tex', real=True, positive=True)
rho = sp.Max(sp.Abs(du_dx), sp.Abs(dv_dx), sp.Abs(du_dy), sp.Abs(dv_dy)) * w_tex
lambda_mip = sp.log(rho, 2)
print(f"Mipmap level: λ = log2(ρ), ρ = max(|∂u/∂x|, |∂v/∂x|, |∂u/∂y|, |∂v/∂y|) × W_tex")
print(f"  λ(ρ=1, no scale)         = {lambda_mip.subs([(du_dx, 1), (dv_dx, 1), (du_dy, 0), (dv_dy, 0), (w_tex, 1)])}")
print(f"  λ(ρ=2, half-res)         = {lambda_mip.subs([(du_dx, 2), (dv_dx, 2), (du_dy, 0), (dv_dy, 0), (w_tex, 1)])}")
print(f"  λ(ρ=4, quarter-res)      = {lambda_mip.subs([(du_dx, 4), (dv_dx, 4), (du_dy, 0), (dv_dy, 0), (w_tex, 1)])}")
print(f"  λ(ρ=0.5, magnify)        = {lambda_mip.subs([(du_dx, 0.5), (dv_dx, 0.5), (du_dy, 0), (dv_dy, 0), (w_tex, 1)])}")

# 2. MIPMAP PYRAMID SIZE
# Level 0: W × H
# Level i: W/2^i × H/2^i (until min dimension = 1)
# Total memory: sum = (4/3) × W × H (geometric series, ratio 1/4)

W, H = 1024, 1024
total = W * H * (1 + 1/4 + 1/16 + 1/64 + 1/256 + 1/1024)  # approx 4/3
exact = W * H * 4 / 3
print(f"\nTexture {W}x{H}:")
print(f"  Level 0 (base):    {W * H:,} texels")
print(f"  Total mipmap chain: {exact:,.0f} texels (overhead: +33%)")
print(f"  Bytes (RGBA8): {exact * 4 / 1024 / 1024:.2f} MB")
print(f"  Bytes (BC7 compressed, 1 byte/texel): {exact / 1024 / 1024:.2f} MB")

# 3. ANISOTROPIC FILTERING ELLIPSE
# When viewing at grazing angle, the pixel footprint becomes an ellipse
# Length-to-width ratio = anisotropy (e.g., 16x)
# Standard: sample N lines along the long axis, each at min(mip_long, mip_short + log2(anisotropy))
fig, axes = plt.subplots(1, 2, figsize=(12, 5))
axes[0].set_aspect('equal')

# Draw ellipse for various anisotropy
for aniso in [1, 2, 4, 8, 16]:
    theta = np.linspace(0, 2*np.pi, 100)
    a, b = aniso, 1  # semi-major, semi-minor
    x = a * np.cos(theta)
    y = b * np.sin(theta)
    axes[0].plot(x, y, label=f'Aniso {aniso}:1')
axes[0].set_xlim(-18, 18)
axes[0].set_ylim(-2, 2)
axes[0].set_title('Anisotropic Filtering Ellipse\n(grazing angle footprint)')
axes[0].legend()
axes[0].grid(True, alpha=0.3)

# 4. COMPRESSION RATIO TABLE
compression_data = [
    ["Format", "Bytes/texel", "BC7 1024² (KB)", "ASTC 4×4 1024² (KB)", "Uncompressed (KB)"],
    ["RGBA8 (raw)", 4, "—", "—", f"{W*H*4/1024:.0f}"],
    ["BC7 (8 bytes/4×4 block)", 0.5, f"{W*H*0.5/1024:.0f}", "—", "—"],
    ["BC1 (8 bytes/4×4, RGB)", 0.5, f"{W*H*0.5/1024:.0f}", "—", "—"],
    ["ASTC 4×4 (16 bytes/block)", 1.0, "—", f"{W*H*1.0/1024:.0f}", "—"],
    ["ASTC 6×6 (16 bytes/block)", 0.44, "—", f"{W*H*0.44/1024:.0f}", "—"],
]
axes[1].axis('off')
table = axes[1].table(cellText=compression_data[1:], colLabels=compression_data[0],
                     loc='center', cellLoc='center')
table.auto_set_font_size(False)
table.set_fontsize(9)
table.scale(1, 1.5)
axes[1].set_title('Texture Compression Ratios (1024×1024 texture)')

plt.tight_layout()
plt.savefig(OUT_DIR / 'texture_streaming.png', dpi=100)
print(f"\nSaved: {OUT_DIR / 'texture_streaming.png'}")

results = {
    "module": "texture_streaming",
    "formulas": {
        "mipmap_level": "λ = log2(max(|∂u/∂x|, |∂v/∂x|, |∂u/∂y|, |∂v/∂y|) × W_tex)",
        "total_mipmap_size": "Total = W × H × (1 + 1/4 + 1/16 + ...) = (4/3) × W × H",
        "anisotropic_sampling": "N lines along long axis, each at min(λ_long, λ_short + log2(aniso))",
    },
    "test_results": {
        "1024_squared_rgba8_mipmap_bytes": int(exact * 4),
        "1024_squared_bc7_mipmap_bytes": int(exact * 0.5),
        "1024_squared_astc_4x4_bytes": int(exact * 1.0),
        "compression_ratio_vs_uncompressed": "8x (BC7), 4x (ASTC 4x4), 9x (ASTC 6x6)",
    },
    "implementation_notes": [
        "Precompute mipmap chain at texture load time",
        "Per-pixel: compute partial derivatives ∂u/∂x etc. from neighbor UVs",
        "Select mipmap level λ, clamp to [0, max_level]",
        "Trilinear: blend adjacent mip levels for smooth transitions",
        "Anisotropic: sample multiple texels along footprint ellipse",
    ],
}
with open(OUT_DIR / 'texture_streaming_results.json', 'w') as f:
    json.dump(results, f, indent=2)

with open(OUT_DIR / 'texture_formulas.txt', 'w') as f:
    f.write("P3 ENGINE — TEXTURE STREAMING DERIVATIONS\n")
    f.write("=" * 60 + "\n\n")
    f.write("1. MIPMAP LEVEL DERIVATION\n")
    f.write("-" * 40 + "\n")
    f.write("Given pixel (x, y) with texture coords (u, v):\n")
    f.write("  ∂u/∂x, ∂v/∂x = partial derivatives of u,v wrt pixel x\n")
    f.write("  ∂u/∂y, ∂v/∂y = partial derivatives of u,v wrt pixel y\n")
    f.write("  W_tex = texture width in texels\n")
    f.write("\n  λ = log2(max(|∂u/∂x|, |∂v/∂x|, |∂u/∂y|, |∂v/∂y|) × W_tex)\n")
    f.write("\n  λ = 0: 1:1 texel/pixel ratio (no minification)\n")
    f.write("  λ = 1: 2:1 ratio (use 1/2 mip)\n")
    f.write("  λ = 2: 4:1 ratio (use 1/4 mip)\n")
    f.write("  λ < 0: magnification (need bilinear from level 0)\n\n")
    f.write("2. TOTAL MIPMAP MEMORY\n")
    f.write("-" * 40 + "\n")
    f.write("Total = W × H × (1 + 1/4 + 1/16 + 1/64 + ...) = (4/3) × W × H\n")
    f.write("  Overhead: +33% vs single-level texture\n")
    f.write("  For 1024×1024 RGBA8: 5.33 MB → 7.11 MB (with mip chain)\n\n")
    f.write("3. ANISOTROPIC FILTERING\n")
    f.write("-" * 40 + "\n")
    f.write("At grazing angles, pixel footprint becomes ellipse (length/width = aniso)\n")
    f.write("Standard: sample N lines along long axis (N = aniso level: 2, 4, 8, 16)\n")
    f.write("Each sample at: min(λ_long_axis, λ_short_axis + log2(aniso))\n")
    f.write("Result: sharp textures at grazing angles (no blur)\n\n")
    f.write("4. TEXTURE COMPRESSION FORMATS\n")
    f.write("-" * 40 + "\n")
    f.write("BC7 (8 bytes per 4×4 block = 0.5 byte/texel):\n")
    f.write("  Best quality, RGBA, used for color/albedo/normal maps\n")
    f.write("BC1 (8 bytes per 4×4 block = 0.5 byte/texel):\n")
    f.write("  RGB only (no alpha), 5:6:5 color, used for diffuse textures\n")
    f.write("ASTC 4×4 (16 bytes per 4×4 = 1.0 byte/texel):\n")
    f.write("  Better than BC7, configurable block size, mobile-friendly\n")

print(f"Texture streaming calculations COMPLETE")
