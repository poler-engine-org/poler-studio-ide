# P³ Engine — Implementation Roadmap (Phase 2)

**Дата:** 2026-08-20
**Статус:** После math verification (commits 5324148, f9ab95d) — все 5 категорий инструментов проверены.

---

## 📊 Текущее состояние движка

### Что уже реализовано (Зиг-модули в `src/`):
- `p3_math.zig` — Vec3/Vec4/Mat4x4/Quaternion, transforms, projections (~2000 LOC)
- `p3_kernel.zig` — HomVec4, PGL4, projective primitives
- `p3_vision.zig` — VisualFrameBuffer (triple: RGB+depth+seg), ProjectiveRasterizer, **CV analyzer** (analyzeFrameBuffer, VisibleEntity with track_id/velocity), **Tracker** (temporal), **SceneGraph** (parent/child/neighbors), **serializeObservationJson** (~1234 LOC)
- `p3_vehicle_physics.zig` — Pacejka tire, suspension, transmission, differential, integrateVehicle (~786 LOC)
- `p3_obj_loader.zig` — Wavefront .obj parser (Blender integration)
- `p3_race_bench.zig` — Headless race renderer (trefoil-knot track, cyber-bolide, 30 buildings)
- `p3_arena_bench.zig` — Asteroid field benchmark
- `p3_closed_loop.zig` — LLM agent driver (high-level actions: aim_at, approach_entity, orbit_entity, follow_entity)
- 50+ other modules (algebra, ECS, physics, rendering, UI, GPU RT, etc.)

### Размер кодовой базы:
- 57 Zig-модулей в `src/`
- 33 137 строк кода
- 73 vehicle_physics теста, 8 vision теста, 68 obj_loader теста — все PASS
- 1600+ общих тестов движка

### Что уже в `calculations/`:
- 8 Python-скриптов с пред-расчётами (formulas.txt + results.json + PNG)
- `brdf_lut.bin` (32 KB) — готов к загрузке в Zig
- `verify_pacejka_multi_tool.py` — SymPy+NumPy+matplotlib проверка
- `verify_cgal_mujoco_character.py` — MuJoCo ground truth для character physics
- `verify_pga_clifford.py` — Clifford PGA (=наш P³) verification

---

## 🎯 План реализации Phase 2 (по приоритету)

### 📦 Module 1: Texture Streaming (`p3_texture.zig`, ~200 LOC)

**Зачем:** Нужен для всех остальных модулей (PBR нуждается в textures, race-bench в деталях трассы).

**Что реализовать:**
```zig
pub const MipLevel = u8;
pub const TextureFormat = enum { rgba8, bc7, astc_4x4, astc_6x6 };

pub const TextureMip = struct {
    width: u32,
    height: u32,
    format: TextureFormat,
    data: []const u8,
};

pub const Texture = struct {
    mips: []TextureMip,
    allocator: std.mem.Allocator,

    pub fn init(allocator, path) !Texture  // load .png / .tga
    pub fn deinit(self) void
    pub fn selectMipLevel(self, du_dx: f32, dv_dy: f32) MipLevel
    pub fn sampleBilinear(self, u: f32, v: f32, mip: MipLevel) PixelColor
};
```

**Алгоритмы (из `calc_texture_streaming.py`):**
- `λ = log2(max(|∂u/∂x|, |∂v/∂x|, |∂u/∂y|, |∂v/∂y|) × W_tex)` — mipmap level
- `total = (4/3) × W × H` — memory overhead
- Bilinear sampling: 4 texels, weighted by (u-{u0}), (v-{v0})

**Зависимости:** нет
**Файлы:** `src/p3_texture.zig`, добавить `texture` в `build.zig` modules, target `test-texture`
**Тесты:** load test PNG, verify mipmap count, sample at known UV, compare to ground truth

---

### 📦 Module 2: Character Physics (`p3_character_physics.zig`, ~600 LOC)

**Зачем:** Для любой игры с людьми (ходьба, прыжки, плавание, подъём по лестницам).

**Что реализовать (нативный порт UE CharacterMovementComponent):**
```zig
pub const MoveMode = enum { none, walking, falling, swimming, flying };

pub const Capsule = struct {
    radius: f32 = 0.4,
    height: f32 = 1.8,
    // cylinder half-length = (height - 2*radius) / 2
};

pub const CharacterState = struct {
    position: Vec3,
    velocity: Vec3,
    orientation: Quaternion,
    capsule: Capsule,
    mass: f32 = 80.0,
    move_mode: MoveMode = .walking,
    // ... etc.
};

pub const CharacterConfig = struct {
    max_step_height: f32 = 0.20,  // MAX_STEP_SIDE_Z (UE default)
    max_simulation_iterations: u8 = 8,
    ground_friction: f32 = 8.0,
    gravity: f32 = 9.81,
    max_walk_speed: f32 = 6.0,    // m/s
    jump_velocity: f32 = 4.5,
    swim_buoyancy: f32 = 1.0,
    // ... etc.
};

// === PhysWalking (нативный порт UE PhysWalking из CharacterMovementComponent.cpp:5653) ===
pub fn physWalking(state, config, world, dt) void

// === PhysFalling (нативный порт UE PhysFalling из CharacterMovementComponent.cpp:4887) ===
pub fn physFalling(state, config, world, dt) void

// === PhysSwimming (нативный порт UE PhysSwimming из CharacterMovementComponent.cpp:4605) ===
pub fn physSwimming(state, config, world, dt) void

// === StepUp (нативный порт UE StepUp из CharacterMovementComponent.cpp:7561) ===
pub fn stepUp(state, config, world, grav_dir, delta, hit) bool

// === MaintainHorizontalGroundVelocity (UE:5635) ===
pub fn maintainHorizontalGroundVelocity(state) void

// === CalcVelocity (UE:3863) — acceleration, friction, braking ===
pub fn calcVelocity(state, config, dt) void

// === ComputeFloorDist (UE:7058) ===
pub fn computeFloorDist(state, world, capsule_location) FloorResult

// === Capsule-vs-Triangle distance (из calc_character_physics.py) ===
pub fn capsuleVsTriangleDistance(capsule, triangle) f32

// === Buoyancy (Archimedes) ===
pub fn computeBuoyancy(state, water_level, dt) void

// === Main update ===
pub fn updateCharacter(state, config, world, input, dt) void
```

**Алгоритмы (из `calc_character_physics.py` + UE source):**
- `smoothstep(t) = 3t² - 2t³` — step-up smoothing (нулевая скорость границ)
- `V_capsule = π·R²·L + (4/3)·π·R³` — объём для плавучести
- `F = ρ·g·V_sub` — Архимед
- `t* = -((A-P0)·n̂) / ((B-A)·n̂)` — параметр ближайшей точки на сегменте к плоскости треугольника

**Зависимости:** `p3_math.zig` (Vec3, Quaternion, Mat4x4)
**Файлы:** `src/p3_character_physics.zig`, добавить `character_physics` в `build.zig`
**Тесты:** 
- ground raycast hits floor at correct distance
- step-up: capsule clears 0.20m stair
- buoyancy: 80kg character floats with positive net force
- PhysWalking: capsule moves horizontally with input acceleration

---

### 📦 Module 3: PBR Materials (`p3_pbr.zig`, ~400 LOC)

**Зачем:** Визуальное качество (Cook-Torrance BRDF — золотой стандарт PBR).

**Что реализовать:**
```zig
pub const Material = struct {
    base_color: PixelColor,       // RGB albedo
    metallic: f32 = 0.0,          // 0 = dielectric, 1 = metal
    roughness: f32 = 0.5,         // 0 = mirror, 1 = diffuse
    F0: Vec3,                     // fresnel at normal incidence (computed from metallic)
};

pub const BrdfLut = struct {
    scale: []f32,  // 64×64 or 256×256 (loaded from brdf_lut.bin)
    bias: []f32,
    size: u32,

    pub fn loadFromFile(path) !BrdfLut  // load binary blob
    pub fn sample(self, NdotV: f32, roughness: f32) struct { scale: f32, bias: f32 }
};

// === GGX NDF (Disney BRDF notes, B.2 Eq 13) ===
pub fn ggxNdf(N, H, alpha: f32) f32
//   D = α² / (π * ((N·H)² * (α²-1) + 1)²)

// === Smith geometry (separable, GGX-matched) ===
pub fn smithG(N, V, L, alpha: f32) f32
//   G1(X, α) = 2X / (X + sqrt(α² + (1-α²)·X²))

// === Fresnel Schlick ===
pub fn fresnelSchlick(VdotH: f32, F0: Vec3) Vec3
//   F = F0 + (1-F0) * (1-VdotH)⁵

// === Cook-Torrance BRDF ===
pub fn cookTorranceBrdf(N, V, L, H, material: Material) PixelColor
//   f_r = (1-F) * albedo/π + (D * G * F) / (4 * (N·V) * (N·L))

// === IBL lookup ===
pub fn imageBasedLighting(N, V, material, brdf_lut, irradiance_cubemap) PixelColor
//   indirect_diffuse = irradiance_cubemap.sample(N) * albedo * (1 - metallic)
//   indirect_specular = prefiltered_env.sample(R) * (F0 * lut.scale + lut.bias)
```

**Алгоритмы (из `calc_pbr_materials.py`):**
- `brdf_lut.bin` загружается как `[]f32` scale + `[]f32` bias (total 32 KB)
- `α = roughness²` (perceptual linear)
- F0 for dielectrics: `((n-1)/(n+1))²` (typical 0.02-0.05)
- F0 for metals: RGB tint (gold = (1.0, 0.71, 0.29))

**Зависимости:** `p3_vision.zig` (PixelColor), `p3_math.zig` (Vec3)
**Файлы:** `src/p3_pbr.zig`, добавить `pbr` в `build.zig`
**Тесты:**
- brdf_lut.bin loads with correct size (64×64 × 2 × 4 bytes = 32 KB)
- GGX NDF: zero at NdotH=0, peak at NdotH=1
- Smith G: 1.0 at zero roughness (perfect mirror)
- Fresnel: F0 at VdotH=1, 1.0 at VdotH=0

---

### 📦 Module 4: Skeletal Animation (`p3_skeletal.zig`, ~500 LOC)

**Зачем:** Анимированные персонажи (бег, ходьба, атака, смерть).

**Что реализовать:**
```zig
pub const Bone = struct {
    name: []const u8,
    parent_index: i32,  // -1 = root
    bind_pose: Mat4x4,
    local_pose: Mat4x4,
};

pub const Skeleton = struct {
    bones: []Bone,
    pub fn computeObjectMatrices(self, out: []Mat4x4) void
};

pub const VertexWeight = struct {
    bone_indices: [4]u16,
    weights: [4]f32,
};

pub const SkinnedMesh = struct {
    vertices: []MeshVertex,
    weights: []VertexWeight,
    indices: []u16,
};

// === Linear Blend Skinning (LBS) — fast, candy wrapper artifact ===
pub fn linearBlendSkinning(mesh, skeleton, out_vertices) void
//   v' = Σ wᵢ Mᵢ v

// === Dual Quaternion Skinning (DQS) — preserves volume ===
pub const DualQuat = struct { rot: Quaternion, dual: Quaternion };
pub fn dualQuatSkinning(mesh, skeleton, out_vertices) void
//   v' = (Σ wᵢ dqᵢ normalized) · v

// === FABRIK IK solver (from calc_skeletal_animation.py) ===
pub fn solveFabrik(joints: []Vec3, target: Vec3, max_iter: u32) void
//   forward + backward pass, preserve bone lengths

// === SLERP for quaternions ===
pub fn slerp(q_a, q_b, t) Quaternion
//   q(t) = (q_A·sin((1-t)·θ/2) + q_B·sin(t·θ/2)) / sin(θ/2)
```

**Зависимости:** `p3_math.zig` (Mat4x4, Quaternion)
**Файлы:** `src/p3_skeletal.zig`
**Тесты:**
- LBS: identity bones → output = input
- DQS: vertex under 180° twist preserves volume (no candy wrapper)
- FABRIK: 3-bone chain converges to target within 20 iterations

---

### 📦 Module 5: Particle System (`p3_particles.zig`, ~300 LOC)

**Зачем:** Эффекты (искры, дым, exhaust).

**Что реализовать:**
```zig
pub const Particle = struct {
    position: Vec3,
    velocity: Vec3,
    age: f32,         // 0..1 (normalized lifetime)
    size: f32,
    color: PixelColor,
};

pub const Emitter = struct {
    particles: []Particle,
    spawn_rate: f32,        // particles/sec
    lifetime: f32,          // seconds
    gravity: Vec3,
    color_curve: [4]Vec2,  // cubic Bezier control points

    pub fn update(self, dt) void
    pub fn spawnParticle(self) void
};

pub fn sampleBezier(t: f32, control_points: [4]Vec2) f32
//   B(t) = (1-t)³ P0 + 3(1-t)² t P1 + 3(1-t) t² P2 + t³ P3

pub fn curlNoise(p: Vec3) Vec3
//   divergence-free velocity field (smoke/dust)
```

**Зависимости:** `p3_math.zig`, `p3_vision.zig` (PixelColor)
**Тесты:**
- Bezier(0) = P0, Bezier(1) = P3, Bezier(0.5) midpoint
- Curl noise: no divergence (volume-preserving)
- Particle lifetime cycles through 0→1

---

### 📦 Module 6: Network Replication (`p3_network.zig`, ~500 LOC)

**Зачем:** Multiplayer (later).

**Что реализовать:**
```zig
pub const Fixed32 = struct {
    raw: i64,  // value × 2^32
    // Q32.32 fixed-point, precision 2.3e-10

    pub fn fromFloat(f) Fixed32
    pub fn toFloat(self) f32
    pub fn add(self, other) Fixed32
    pub fn mul(self, other) Fixed32
};

pub const SnapshotDelta = struct {
    changed_fields: u16,  // bitmask
    values: [10]f32,      // only changed ones
    pub fn encode(prev, curr) SnapshotDelta
    pub fn decode(self, prev) Snapshot
};

pub const RollbackBuffer = struct {
    states: [7]Snapshot,  // W=7 window
    inputs: [7]Input,

    pub fn applyCorrection(self, confirmed_frame: u32, server_state: Snapshot) void
    pub fn reSimulate(self, current_inputs: Input) void
};
```

**Тесты:**
- Fixed32 add/mul matches float32 within precision
- SnapshotDelta: only changed fields encoded
- Rollback: re-simulation produces same state as original (determinism)

---

### 📦 Module 7: Lumen GI (`p3_gi.zig`, ~2000 LOC)

**Зачем:** Real-time global illumination.

**Что реализовать:**
```zig
pub const SphericalHarmonicsL2 = struct {
    // 9 RGB coefficients = 27 floats total
    coeffs: [9][3]f32,

    pub fn projectFromIrradiance(irradiance: Cubemap) SphericalHarmonicsL2
    pub fn sample(self, normal: Vec3) PixelColor
};

pub const SDF = struct {
    data: []f32,  // 3D grid
    dimensions: Vec3,
    cell_size: f32,

    pub fn sample(self, p: Vec3) f32
    pub fn rayMarch(self, origin: Vec3, dir: Vec3) RayHit
};

pub const SurfaceCache = struct {
    radiance: []PixelColor,  // 512×512
    pub fn update(self, sdf: SDF, lights: []Light) void
};

pub fn computeIndirectDiffuse(sh: SphericalHarmonicsL2, normal: Vec3) PixelColor
pub fn computeIndirectSpecular(env_map, R: Vec3, roughness: f32) PixelColor
```

---

### 📦 Module 8: Nanite Virtualized Geometry (`p3_nanite.zig`, ~2000 LOC)

**Зачем:** Virtualized geometry для AAA meshes.

**Что реализовать:**
```zig
pub const Cluster = struct {
    triangles: [128]Triangle,
    bounding_sphere: Sphere,
    screen_error: f32,

    pub fn computeScreenError(self, camera: Camera) f32
    //   ε = ρ × r / d
};

pub const ClusterHierarchy = struct {
    nodes: []ClusterNode,  // DAG
    pub fn build(base_mesh: Mesh) ClusterHierarchy
    pub fn selectLod(self, camera: Camera) []Cluster
};

pub fn computeQEM(vertex: Vec3, planes: []Plane) f32
//   Q(v) = vᵀ K v  where K = Σ (n,d)ᵀ(n,d)
```

---

## 🚀 Порядок выполнения

1. **Сейчас: Texture Streaming** (200 LOC, ~30 минут)
2. **Дальше: Character Physics** (600 LOC, ~1.5 часа)
3. **Потом: PBR Materials** (400 LOC, ~1 час, с brdf_lut.bin loading)
4. **Skeletal Animation** (500 LOC, ~1.5 часа)
5. **Particle System** (300 LOC, ~30 минут)
6. **Network Replication** (500 LOC, ~1 час)
7. Lumen GI (2000 LOC, ~5 часов — очень сложно)
8. Nanite (2000 LOC, ~5 часов — очень сложно)

## 📦 Unreal Engine integration

**Что безопасно интегрировать из UE source (который у нас в `/home/z/research/giant/UnrealEngine-release/`):**
- ✅ **Алгоритмы** (Pacejka, Cook-Torrance, GGX, FABRIK, StepUp) — public domain math
- ✅ **Структуры данных** (FWheelStatus, CharacterState, Material) — концептуальные, реализуем нативно
- ✅ **MaterialX glsl shaders** — /Engine/Binaries/ThirdParty/MaterialX/libraries/pbrlib/ (Disney BRDF notes reference, public)
- ⚠️ **C++ код UE** — Epic license, не копируем напрямую. Берём архитектуру + переписываем на Zig

**Что НЕ интегрируем:**
- ❌ Chaos Physics (proprietary, нужен million LOC port — слишком много)
- ❌ Lumen/Nanite proprietary implementations (берём алгоритмы из публичных SIGGRAPH talks)
- ❌ Network replication с UE-specific netcode (используем GGPO public domain)

## 🎯 После завершения Phase 2

Движок будет иметь:
- Headless CPU rendering с structured CV observation (уже есть)
- Native vehicle physics (уже есть)
- Native character physics (новое)
- PBR materials (новое)
- Skeletal animation (новое)
- Particle system (новое)
- Texture streaming (новое)
- Network multiplayer (новое)
- Real-time GI (новое)
- Virtualized geometry (новое)

→ Полноценный AAA game engine с native P³ math, без костылей.
