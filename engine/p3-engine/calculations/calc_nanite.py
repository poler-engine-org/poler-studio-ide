#!/usr/bin/env python3
"""P3 Engine — Nanite Virtualized Geometry (QEM, cluster bounds)."""
import json, numpy as np, sympy as sp
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from pathlib import Path

OUT = Path('/home/z/P3_Engine/calculations/nanite'); OUT.mkdir(parents=True, exist_ok=True)

# 1. SCREEN-SPACE ERROR FORMULA (Nanite LOD selection)
# ε_screen = ρ * r / d  where:
#   ρ = pixel density (texels/meter on screen)
#   r = bounding sphere radius of cluster (meters)
#   d = distance from camera to cluster (meters)
# If ε_screen > threshold, switch to lower-detail LOD
print("1. Screen-space error formula:")
rho, r, d = sp.symbols('rho r d', real=True, positive=True)
epsilon_screen = rho * r / d
print(f"  ε_screen = ρ × r / d")
print(f"  ρ = pixel density (texels/m)")
print(f"  r = cluster bounding sphere radius (m)")
print(f"  d = camera-to-cluster distance (m)")

# Sample: cluster radius 0.5m at various distances
distances = np.linspace(1, 100, 100)
radii = [0.5, 1.0, 2.0, 5.0]
fig, ax = plt.subplots(figsize=(8, 5))
for r_val in radii:
    error = 100 * r_val / distances  # rho = 100 pixels/m (typical)
    ax.plot(distances, error, linewidth=2, label=f'r = {r_val}m')
ax.axhline(y=4, color='r', linestyle='--', alpha=0.7, label='Threshold (4 pixels)')
ax.set_xlabel('Distance from camera (m)')
ax.set_ylabel('Screen-space error (pixels)')
ax.set_title('Nanite LOD Selection\n(switch when ε > 4 px)')
ax.legend(); ax.grid(True, alpha=0.3)
ax.set_yscale('log')
plt.tight_layout()
plt.savefig(OUT / 'nanite_lod_selection.png', dpi=100)
print(f"Saved: {OUT / 'nanite_lod_selection.png'}")

# 2. QEM (Quadric Error Metrics) — for mesh simplification
# Each vertex has a quadric matrix Q = Σ (planes adjacent to vertex)
# Error of moving vertex v to new position v': Q(v') = v'ᵀ Q v'
# Minimize: take gradient, solve for optimal v'
# For vertex on plane n·v + d = 0, the plane quadric is K = [n; d][n;d]ᵀ

print("\n2. QEM (Quadric Error Metrics) derivation:")
n_x, n_y, n_z, d_plane = sp.symbols('n_x n_y n_z d_plane', real=True)
v_x, v_y, v_z = sp.symbols('v_x v_y v_z', real=True)
v = sp.Matrix([v_x, v_y, v_z, 1])  # homogeneous
plane = sp.Matrix([n_x, n_y, n_z, d_plane])
# K = plane outer plane (4x4 symmetric matrix)
K = plane * plane.T
print(f"  Plane: n·v + d = 0, where n = (n_x, n_y, n_z)")
print(f"  Quadric K = (n, d)ᵀ(n, d) (4×4 symmetric)")
print(f"  Error(v') = v'ᵀ K v'")
# Minimization: d/dv' = 2 K v' = 0 → v' = K⁻¹ (last column, with [0,0,0,1])
print(f"  Optimal v' = argmin v'ᵀ K v'  →  solve K v' = [0,0,0,1]ᵀ")

# 3. CLUSTER HIERARCHY (DAG)
# Each cluster = 128 triangles (Nanite default)
# Build binary DAG: parent cluster = simplified union of 2 children
# At each level, parent has half the triangles of children combined
# Total memory for n triangles:
#   Level 0 (base): n triangles
#   Level 1: n/2 triangles (simplified parent)
#   Level 2: n/4
#   ...
#   Total: ~2n triangles (geometric series sum 1 + 1/2 + 1/4 + ... = 2)

n_tri = 1_000_000  # 1M triangles base
levels = 0
total = n_tri
while n_tri > 128:
    n_tri //= 2
    total += n_tri
    levels += 1
print(f"\n3. Cluster hierarchy (1M triangles base):")
print(f"  Levels: {levels}")
print(f"  Total triangles stored: {total:,} ({total/1_000_000:.2f}M)")
print(f"  Memory overhead vs base: +{(total-n_tri)*100/n_tri:.1f}% (≈2x)")

# 4. Visualization: cluster DAG
fig, ax = plt.subplots(figsize=(8, 5))
levels_show = 6
nodes_per_level = [1, 2, 4, 8, 16, 32]
y_positions = list(range(levels_show))
for level, count in zip(y_positions, nodes_per_level):
    for i in range(min(count, 8)):  # show up to 8 per level
        x = (i - count/2) * 1.5
        ax.scatter(x, -level, s=100, c='blue', zorder=3)
# Draw edges (simplified — show pairs)
for level in range(levels_show - 1):
    for i in range(min(nodes_per_level[level], 4)):
        x_parent = (i - nodes_per_level[level]/2) * 1.5
        for j in range(2):
            x_child = (2*i + j - nodes_per_level[level+1]/2) * 1.5
            ax.plot([x_parent, x_child], [-level, -level-1], 'k-', alpha=0.3)
ax.set_xlabel('Cluster index')
ax.set_ylabel('Level (0=coarse, top=fine)')
ax.set_yticks(range(0, -levels_show, -1))
ax.set_yticklabels([f'L{abs(i)}' for i in range(0, -levels_show, -1)])
ax.set_title('Nanite Cluster DAG\n(each parent = simplified union of 2 children)')
ax.grid(True, alpha=0.3)
plt.tight_layout()
plt.savefig(OUT / 'nanite_cluster_dag.png', dpi=100)
print(f"Saved: {OUT / 'nanite_cluster_dag.png'}")

results = {
    "module": "nanite",
    "formulas": {
        "screen_space_error": "ε_screen = ρ × r / d  (pixel density × cluster radius / distance)",
        "lod_threshold": "ε_screen > 4 px → switch to lower-detail cluster",
        "qem_plane_quadric": "K = (n, d)ᵀ (n, d)  where plane is n·v + d = 0",
        "qem_error": "Error(v') = v'ᵀ K v', minimize → K v' = [0,0,0,1]ᵀ",
        "cluster_size": "128 triangles per cluster (UE Nanite default)",
        "dag_total_memory": "Total ≈ 2n triangles for n base triangles (geometric series)",
    },
    "test_results": {
        "base_triangles": 1_000_000,
        "levels_in_hierarchy": levels,
        "total_triangles_stored": total,
        "memory_overhead_percent": float((total - n_tri) * 100 / n_tri),
        "ratio_vs_base": float(total / n_tri),
    },
    "implementation_notes": [
        "Use meshoptimizer (OSS) for meshlet generation + cluster grouping",
        "Use METIS for k-way partitioning of triangle graph",
        "QEM simplification: build per-vertex quadrics from face normals",
        "DAG edges: parent cluster has half the triangles of 2 children combined",
        "Streaming: only load clusters visible at current ε_screen threshold",
        "Reference: UE5 Nanite SIGGRAPH 2021 talk, Texturing Mesh Shaders by AMD",
    ],
}
with open(OUT / 'nanite_results.json', 'w') as f:
    json.dump(results, f, indent=2)

with open(OUT / 'nanite_formulas.txt', 'w') as f:
    f.write("P3 ENGINE — NANITE VIRTUALIZED GEOMETRY DERIVATIONS\n")
    f.write("=" * 60 + "\n\n")
    f.write("1. SCREEN-SPACE ERROR (LOD selection)\n")
    f.write("-" * 40 + "\n")
    f.write("ε_screen = ρ × r / d\n")
    f.write("  ρ = pixel density (texels per meter on screen)\n")
    f.write("  r = cluster bounding sphere radius (m)\n")
    f.write("  d = camera-to-cluster distance (m)\n")
    f.write("Switch to lower LOD when ε_screen > threshold (4 pixels typical)\n\n")
    f.write("2. QEM (Quadric Error Metrics) for mesh simplification\n")
    f.write("-" * 40 + "\n")
    f.write("Each vertex accumulates a 4×4 symmetric quadric K:\n")
    f.write("  K = Σ planes (planes = face normals + offsets)\n")
    f.write("  For plane n·v + d = 0: K_plane = (n,d)ᵀ (n,d)\n")
    f.write("Error of moving vertex to v': Q(v') = v'ᵀ K v'\n")
    f.write("Optimal v' minimizes Q → solve K v' = [0, 0, 0, 1]ᵀ\n\n")
    f.write("3. CLUSTER HIERARCHY (DAG of clusters)\n")
    f.write("-" * 40 + "\n")
    f.write("Base mesh: n triangles\n")
    f.write("Level 0 (finest): n triangles, in clusters of 128\n")
    f.write("Level 1: n/2 triangles (parent = simplified union of 2 children)\n")
    f.write("Level k: n/2^k triangles\n")
    f.write("Until cluster size < 128: stop subdivision\n")
    f.write("Total memory: ≈ 2n triangles (sum of geometric series 1 + 1/2 + 1/4 + ...)\n\n")
    f.write("4. VISIBILITY CULLING (Hierarchical Z-Buffer)\n")
    f.write("-" * 40 + "\n")
    f.write("For each cluster: project bounding sphere to screen\n")
    f.write("If screen radius < 1 pixel: cull\n")
    f.write("If bounding sphere behind Hi-Z tile: occlusion cull\n")
    f.write("Surviving clusters: rasterize via mesh shaders (128 tris per draw call)\n")

print(f"Nanite calculations COMPLETE")
