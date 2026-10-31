# P³ Engine — Master Calculation Guide

**Цель:** Подробный план расчётов для каждой из 8 подсистем движка. Здесь — что считать, зачем, какие инструменты, в каком порядке, что должно получиться на выходе. Ты локально запускаешь скрипты — я говорю что делать дальше.

---

## 📁 Структура каталога `calculations/`

```
calculations/
├── README.md                              ← этот файл
├── calc_character_physics.py              ← Module 1: Capsule + step-up + buoyancy
├── calc_pbr_materials.py                  ← Module 2: Cook-Torrance BRDF + LUT
├── calc_skeletal_animation.py            ← Module 3: LBS vs DQS + FABRIK
├── calc_particle_system.py               ← Module 4: Bezier + curl noise + gravity
├── calc_texture_streaming.py             ← Module 5: Mipmap λ + anisotropic
├── calc_lumen_gi.py                       ← Module 7: SH L2 + SDF
├── calc_nanite.py                         ← Module 8: QEM + cluster DAG
├── calc_network_replication.py           ← Module 6: Fixed-point + delta + rollback
├── verify_pacejka_multi_tool.py         ← Мульти-инструмент проверка (SymPy+NumPy+matplotlib)
│
├── character_physics/                     ← выводы каждого модуля
│   ├── character_physics_formulas.txt
│   ├── character_physics_results.json
│   ├── step_up_curve.png
│   ├── buoyancy_curve.png
│   └── capsule_triangle_distance.png
├── pbr_materials/
│   ├── pbr_formulas.txt
│   ├── pbr_materials_results.json
│   ├── brdf_curves_and_lut.png
│   └── brdf_lut.bin                       ← 32 KB, готово к загрузке в Zig
├── skeletal_animation/ ... (similar)
├── particle_system/ ...
├── texture_streaming/ ...
├── lumen_gi/ ...
├── nanite/ ...
└── network_replication/ ...
```

---

## 🛠 Какой инструмент для чего

| Задача | Инструмент | Установка (CachyOS) |
|---|---|---|
| Символьный вывод уравнений | SymPy | `pip install sympy` (есть) |
| Численная проверка формул | NumPy | `pip install numpy` (есть) |
| Численное интегрирование, сплайны | SciPy | `pip install scipy` (есть) |
| Визуализация результатов | matplotlib | `pip install matplotlib` (есть) |
| SMT-доказательства линейных инвариантов | Z3 | `pip install z3-solver` (есть) |
| Capsule-vs-triangle, geom алгоритмы | **CGAL** | `sudo pacman -S cgal` |
| Обработка мешей, IG LUT | **libigl** | `pip install igl` |
| 3D данные, ICP, SDF | **Open3D** | `pip install open3d` |
| Эталон физики (для сравнения) | **MuJoCo** | `pip install mujoco` |
| Дифференцируемая физика (для RL) | **JAX** | `pip install jax jaxlib` |
| Группы Ли (SO(3), SE(3)) | **Sophus** (C++) | build from github.com/strasdat/Sophus |
| Геом. алгебра (наш P³) | **Klein** | github.com/jeremyong/klein |
| Метриалы/BRDF референс | **Mitsuba 3** | `pip install mitsuba` |
| ODE солверы (RK4, Verlet) | **Boost.Odeint** | `sudo pacman -S boost` |
| SDF / GI | **OpenVDB** | `sudo pacman -S openvdb` |
| Мешлет-кластеризация | **meshoptimizer** | github.com/zeux/meshoptimizer |
| Метриалi референс | **MaterialX** | github.com/AcademySoftwareFoundation/MaterialX |
| IK solver (FABRIK ref) | **Caliko** | github.com/FedUni/caliko |

---

## 📋 Шаги для каждого модуля (делаешь локально)

### Module 1: Character Physics (capsule walking + step-up + swimming)

**Что считать:**
1. **Capsule-vs-Triangle distance** — закрытая формула: `d(t*) = ((A + t*(B-A) - P0)·n̂)`, где `t* = -((A-P0)·n̂) / ((B-A)·n̂)`, clamp [0, 1].
2. **Step-up smoothing** — Hermite cubic `s(t) = 3t² - 2t³`, нулевая скорость на границах (нет snap).
3. **Buoyancy (Archimedes)** — `F = ρ·g·V_submerged`, V_capsule = `π·R²·L + (4/3)·π·R³`.

**Уже посчитано (запусти скрипт):**
```bash
python3 calculations/calc_character_physics.py
```

**Что на выходе:**
- `character_physics/character_physics_formulas.txt` — все формулы
- `character_physics/step_up_curve.png` — визуализация smoothing
- `character_physics/buoyancy_curve.png` — график плавучести
- `character_physics/results.json` — численные константы (R=0.4, h=1.8, V=0.77 m³, F_max=7560 N)

**Что тебе нужно сделать локально:**
1. Установить CGAL: `sudo pacman -S cgal`
2. Запустить скрипт, проверить графики
3. Дополнительные расчёты (если хочешь):
   - Протестировать capsule-vs-triangle с реальным triangle (не плоскостью) — используя CGAL
   - Сравнить нашу buoyancy с MuJoCo (если установлен)
4. Реализовать в Zig: `src/p3_character_physics.zig` (~600 строк)
   - `fn capsuleVsTriangleDistance(capsule, triangle) f32`
   - `fn applyStepUpSmoothing(state, step_height, t)`
   - `fn updateBuoyancy(character_state, water_level, dt)`

---

### Module 2: PBR Materials (Cook-Torrance BRDF + IBL)

**Что считать:**
1. **GGX NDF**: `D = α² / (π · ((N·H)² · (α²-1) + 1)²)`
2. **Smith Geometry**: `G = G1(N·V,α) · G1(N·L,α)`, `G1(X,α) = 2X / (X + √(α² + (1-α²)·X²))`
3. **Fresnel Schlick**: `F = F0 + (1-F0)·(1 - V·H)⁵`
4. **BRDF LUT** — 256×256 lookup table (scale, bias) for F0 multiplier

**Уже посчитано:**
```bash
python3 calculations/calc_pbr_materials.py
```
(64×64 LUT для скорости; продакшен = 256×256 с 1024 samples)

**Что на выходе:**
- `pbr_materials/pbr_formulas.txt` — все формулы
- `pbr_materials/brdf_curves_and_lut.png` — GGX NDF curve + LUT visualization
- `pbr_materials/brdf_lut.bin` — 32 KB (scale + bias, f32) — **готов к загрузке в Zig**
- `pbr_materials/pbr_materials_results.json` — F0 values for materials (water, skin, iron, gold, silver)

**Что тебе нужно сделать локально:**
1. Поставить Mitsuba 3: `pip install mitsuba` — для проверки BRDF ground truth
2. Увеличить LUT до 256×256 с 1024 samples в `calc_pbr_materials.py`:
   - `LUT_SIZE = 256`
   - `SAMPLE_COUNT = 1024`
   - Запуск ~30 минут
3. Дополнительные расчёты (опционально):
   - Diffuse irradiance cubemap convolution (IBL половина для diffuse)
   - Prefiltered environment map для specular
4. Реализовать в Zig: `src/p3_pbr_renderer.zig` (~400 строк)
   - `fn loadBrdfLut(path) BrdfLut` — читает bin файл
   - `fn cookTorranceBrdf(N, V, L, F0, roughness) Color`
   - Интегрировать в `p3_renderer.zig` (фрагмент-функция для каждого пикселя)

---

### Module 3: Skeletal Animation (LBS vs DQS + FABRIK)

**Что считать:**
1. **LBS** — linear blend skinning (v' = Σ wᵢ Mᵢ v)
2. **DQS** — dual quaternion skinning (preserves volume, no candy wrapper)
3. **FABRIK** — iterative IK solver (forward + backward reaching)
4. **SLERP** — spherical linear interpolation for quaternions

**Уже посчитано:**
```bash
python3 calculations/calc_skeletal_animation.py
```

**Что на выходе:**
- `skeletal_animation/skeletal_formulas.txt`
- `skeletal_animation/lbs_vs_dqs.png` — trajectory vertex под 0°→180° twist
- `skeletal_animation/fabrik_convergence.png` — сходимость FABRIK vs target distance
- `skeletal_animation/skeletal_animation_results.json`

**Что тебе нужно сделать локально:**
1. Blender: экспортируй тестовый скелет (e.g., `armature.fbx`) с анимацией walk cycle
2. Проверь наш SLERP против Blender's quaternion interpolation
3. Установи Caliko для FABRIK reference
4. Реализовать в Zig: `src/p3_skeletal_animation.zig` (~500 строк)
   - `DualQuaternion` struct (8 floats)
   - `fn computeDualQuatSkinning(bones, weights) DualQuat`
   - `fn solveFabrik(joints, target, iterations) void`

---

### Module 4: Particle System (Bezier + curl noise + gravity)

**Что считать:**
1. **Cubic Bezier life curves** — 4 control points для intensity/size/color over lifetime
2. **Curl noise** — divergence-free velocity field (smoke/dust)
3. **Gravity integration** — semi-implicit Euler vs Verlet (сравнить с аналитическим решением)

**Уже посчитано:**
```bash
python3 calculations/calc_particle_system.py
```

**Что на выходе:**
- `particle_system/particle_formulas.txt`
- `particle_system/particle_curves.png` — Bezier + curl noise visualization
- `particle_system/integration_comparison.png` — Euler vs Verlet vs analytical
- `particle_system/particle_system_results.json`

**Что тебе нужно сделать локально:**
1. Протестировать curl noise с Perlin вместо нашего простого sin/cos
2. Сравнить с Niagara reference (если есть UE5)
3. Реализовать в Zig: `src/p3_particles.zig` (~300 строк)
   - `ParticleEmitter` struct (max particles, spawn rate, lifetime)
   - `fn sampleBezierCurve(control_points, t) f32`
   - `fn integrateCurlNoise(particle, dt)`

---

### Module 5: Texture Streaming (mipmap λ + anisotropic)

**Что считать:**
1. **Mipmap level**: `λ = log₂(max(|∂u/∂x|, |∂v/∂x|, |∂u/∂y|, |∂v/∂y|) × W_tex)`
2. **Total mipmap memory**: `(4/3) × W × H` (geometric series)
3. **Anisotropic filtering** — ellipse footprint, N samples along long axis
4. **Compression formats** — BC7, BC1, ASTC 4×4/6×6 (bytes/texel, ratio)

**Уже посчитано:**
```bash
python3 calculations/calc_texture_streaming.py
```

**Что на выходе:**
- `texture_streaming/texture_formulas.txt`
- `texture_streaming/texture_streaming.png` — anisotropic ellipses + compression table
- `texture_streaming/texture_streaming_results.json`

**Что тебе нужно сделать локально:**
1. Поставить ImageMagick: `sudo pacman -S imagemagick`
2. Сгенерировать mipmap chain на тестовой текстуре:
   ```bash
   convert input.png -define filter:filter=lanczos \
     -resize 50% -resize 25% -resize 12.5% mip_pyramid.png
   ```
3. Проверить наши формулы против libpng/stb_image при загрузке
4. Реализовать в Zig: `src/p3_texture.zig` (~200 строк)
   - `TextureMipChain` struct (levels: []TextureMip)
   - `fn selectMipLevel(du_dx, dv_dy, w_tex) u8`
   - `fn sampleTextureBilinear(tex, uv, mip_level) Color`

---

### Module 6: Network Replication (fixed-point + delta + rollback)

**Что считать:**
1. **Fixed-point math** — Q16.16 (precision 1.5e-5), Q32.32 (2.3e-10, лучше чем float32)
2. **Delta compression** — bitmask + changed fields, 10× compression at 10% change rate
3. **GGPO rollback** — K=2 input delay, W=7 rollback window, 117ms re-sim at 60FPS

**Уже посчитано:**
```bash
python3 calculations/calc_network_replication.py
```

**Что на выходе:**
- `network_replication/network_formulas.txt`
- `network_replication/network_replication.png` — delta compression rate + bandwidth chart
- `network_replication/network_replication_results.json`

**Что тебе нужно сделать локально:**
1. **Z3 verification**: доказать детерминизм fixed-point math (Q32.32) — без NaN, без overflow
2. Протестировать delta compression на реальном игровом трафике
3. Mock UDP сервер для проверки GGPO rollback
4. Реализовать в Zig: `src/p3_network.zig` (~500 строк)
   - `Fixed32` (Q32.32) struct с операторами
   - `fn packSnapshotDelta(prev, curr) bytes`
   - `fn applyRollback(buffer, confirmed_frame, current_inputs)`

---

### Module 7: Lumen GI (Spherical Harmonics L2 + SDF)

**Что считать:**
1. **SH L2 basis** — 9 coefficients (Y_0^0, Y_1^-1, ..., Y_2^2)
2. **SDF (Signed Distance Field)** — `f(p) = signed distance to nearest surface`
3. **Ray-marching against SDF** — `t += min(distance, ε)`
4. **ClampedCos SH projection** — для diffuse irradiance

**Уже посчитано:**
```bash
python3 calculations/calc_lumen_gi.py
```

**Что на выходе:**
- `lumen_gi/lumen_formulas.txt`
- `lumen_gi/lumen_sh_sdf.png` — SDF circle + SH irradiance approximation
- `lumen_gi/lumen_gi_results.json`

**Что тебе нужно сделать локально:**
1. Поставить OpenVDB: `sudo pacman -S openvdb`
2. Сгенерировать SDF на тестовой сцене через `vdb_tool`
3. Сравнить наши SH coefficients с scipy.special.sph_harm
4. Реализовать в Zig: `src/p3_gi.zig` (~2000 строк, очень сложно)
   - `SphericalHarmonicsL2` struct (9 RGB coeffs = 27 floats)
   - `fn rayMarchSdf(sdf, ray_origin, ray_dir) hit_result`
   - `fn computeDiffuseIrradiance(sh, normal) Color`

---

### Module 8: Nanite Virtualized Geometry (QEM + cluster DAG)

**Что считать:**
1. **Screen-space error**: `ε_screen = ρ · r / d` (pixel density × cluster radius / distance)
2. **QEM (Quadric Error Metrics)** — `K = (n,d)ᵀ(n,d)`, error = `v'ᵀ K v'`
3. **Cluster hierarchy DAG** — parent = simplified union of 2 children, 128 triangles per cluster
4. **Total memory** — geometric series ≈ 2× base mesh

**Уже посчитано:**
```bash
python3 calculations/calc_nanite.py
```

**Что на выходе:**
- `nanite/nanite_formulas.txt`
- `nanite/nanite_lod_selection.png` — screen error vs distance
- `nanite/nanite_cluster_dag.png` — DAG visualization
- `nanite/nanite_results.json` — 1M triangles → 13 levels, 2M total memory

**Что тебе нужно сделать локально:**
1. Поставить MeshLab: `sudo pacman -S meshlab`
2. Поставить Open3D: `pip install open3d` — для QEM simplification reference
3. Поставить meshoptimizer: github.com/zeux/meshoptimizer
4. Сгенерировать cluster DAG на тестовой сцене (e.g., Sponza)
5. Реализовать в Zig: `src/p3_nanite.zig` (~2000 строк, очень сложно)
   - `Cluster` struct (128 triangles + bounding sphere)
   - `fn computeScreenError(cluster, camera) f32`
   - `fn buildClusterDAG(base_mesh) ClusterHierarchy`

---

## 📊 Сводная таблица сложности и приоритета

| # | Модуль | LOC (Zig) | Сложность | Зависимости | Приоритет |
|---|---|---|---|---|---|
| 1 | Character Physics | 600 | средняя | (нет внешних) | **ВЫСОКИЙ** (нужно для любой игры с людьми) |
| 2 | PBR Materials | 400 | средняя | Module 5 (textures для IBL) | **ВЫСОКИЙ** (визуальный качество) |
| 3 | Skeletal Animation | 500 | сложная | Module 1 (для character skinning) | **ВЫСОКИЙ** (нужно для людей) |
| 4 | Particle System | 300 | средняя | (нет) | средний |
| 5 | Texture Streaming | 200 | простая | (нет) | **ВЫСОКИЙ** (нужно для textures в любом рендере) |
| 6 | Network Replication | 500 | сложная | Module 1 (physics must be deterministic) | средний (multiplayer позже) |
| 7 | Lumen GI | 2000 | очень сложная | Module 5 (SDF), Module 2 (materials) | низкий (после остального) |
| 8 | Nanite | 2000 | очень сложная | Module 3 (meshes) | низкий (после остального) |

---

## 🎯 Рекомендуемый порядок реализации

**Фаза 1 (foundation для gaming):**
1. Module 5 — Texture Streaming (быстрый, простой, нужен для всех остальных)
2. Module 1 — Character Physics (нужно для любой игры с людьми)
3. Module 3 — Skeletal Animation (нужно для анимированных персонажей)

**Фаза 2 (визуальное качество):**
4. Module 2 — PBR Materials (для красивого рендера)
5. Module 4 — Particle System (для эффектов)

**Фаза 3 (advanced):**
6. Module 6 — Network Replication (для multiplayer)
7. Module 7 — Lumen GI (для real-time GI)
8. Module 8 — Nanite (для virtualized geometry)

---

## 🔧 Перед пушем — чеклист

После расчётов сделай локально:
```bash
cd P3_Engine

# 1. Проверить что все расчёты прошли
python3 calculations/calc_*.py

# 2. Проверить все тесты проходят
zig build test --summary all

# 3. Закоммитить расчёты
git add calculations/
git commit -m "feat(calculations): add 7-module math derivations and visualizations

Pre-computed formulas, JSON results, and PNG visualizations for:
- Character Physics (capsule, step-up, buoyancy)
- PBR Materials (GGX, Smith, Fresnel, BRDF LUT 64x64 binary)
- Skeletal Animation (LBS vs DQS, FABRIK convergence)
- Particle System (Bezier, curl noise, integration)
- Texture Streaming (mipmap λ, anisotropic, compression)
- Lumen GI (SH L2 9 coeffs, SDF, ClampedCos)
- Nanite (screen error, QEM, cluster DAG)
- Network Replication (fixed-point Q32.32, delta compression, GGPO rollback)

All formulas derived in SymPy, validated with NumPy, visualized with
matplotlib. BRDF LUT (32 KB binary) ready for direct Zig loading."

# 4. Запушить
git push origin main
```

---

## 📖 Полезные ссылки (что читать при реализации)

- **Character Physics**: Game Physics Engine Development (Ian Millington)
- **PBR**: Physically Based Rendering: From Theory to Implementation (Pharr)
- **Skeletal**: Dual Quaternions paper (Kavan et al. 2007), FABRIK paper (Aristidou & Lasenby 2011)
- **Particles**: GPU Gems 3 Chapter 32 (Curl Noise)
- **Texture Streaming**: OpenGL SuperBible chapter on Texture Minification
- **Network**: GGPO protocol docs, Gaffer on Games series
- **Lumen**: Epic SIGGRAPH 2021 talk (Surface Cache + Final Gather)
- **Nanite**: Epic SIGGRAPH 2021 talk (Virtualized Geometry)

Все книги и papers — публично доступные. **Эпик не патентует математику.**
