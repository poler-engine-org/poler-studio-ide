# P³ Engine

**Проективный 3-мерный геометрический движок на Zig.**

Переписываем O3DE (Open 3D Engine, 1.5M строк C++) с нуля — не «легковесный движок», а полный AAA-движок, где математика и типы переписаны через проективную геометрию P³.

> Принцип: **убивать и жрать и рождать новое.**

---

## Что здесь есть

59 модулей, 36 455 строк Zig, 627+ тестов — все проходят на Zig 0.14.0.

```
src/
├── p3_kernel.zig      (902L)  HomVec4, PGL4, FS-метрика, аффинные карты
├── p3_idempotent.zig  (511L)  Идемпотенты P²=P, спектральное разложение, POLER
├── p3_geodesic.zig    (482L)  Геодезические на S³, RK4, exp/log, параллельный перенос
├── p3_bridge.zig      (344L)  P³→R³ мост, деhomогенизация, переключение карт
├── p3_crossratio.zig  (332L)  Перекрестное отношение, гармонические четвёрки, PGL-инвариант
├── p3_safety.zig      (558L)  NonZeroHomVec4, NonSingularPGL4, phantom-типы
├── p3_gpu.zig         (678L)  WGSL шейдеры (FS-distance, PGL, RK4, идемпотент), CPU-эквиваленты
├── p3_raylib.zig      (558L)  raylib C ABI мост, @cImport
├── p3_invariant.zig   (593L)  comptime инварианты, Local/World разделение
├── p3_algebra.zig     (515L)  Внешняя алгебра Грассмана, координаты Плюккера, meet/join, Hodge
├── p3_ecs.zig         (802L)  Entity-Component-System, EventBus
├── p3_scene.zig       (638L)  Scene Graph на S³, FS-bounding spheres, frustum culling
├── p3_physics.zig     (763L)  P3Body, FS-гравитация, Christoffel, Riemann, Ricci, скалярная кривизна
├── p3_serial.zig      (330L)  JSON сериализация через @typeInfo, бинарный формат с FS-целостностью
├── p3_io.zig          (337L)  VFS, asset packs (P3PACK), типы файлов comptime
├── p3_jobs.zig        (330L)  Job scheduler, пакетные вычисления FS-distance/PGL/dehom
├── p3_rhi.zig         (722L)  Render Hardware Interface, comptime backend, FrameGraph
├── p3_renderer.zig    (983L)  P³ Camera, P³ Frustum, FS-depth, Forward+/Deferred pipeline
├── p3_gpu_rt.zig      (700L)  Реальный GPU pipeline через zgpu/Dawn/WebGPU ← НОВЫЙ
├── p3_input.zig       (370L)  Key, MouseState, InputEvent (tagged union), InputState ← НОВЫЙ
└── p3_app.zig         (500L)  pub fn main, P3Camera на S³, P3App lifecycle ← НОВЫЙ
```

---

## Ключевая идея

| Евклидов движок (O3DE/Unity/Unreal) | P³ Engine |
|---|---|
| R³, векторы, Matrix4x4 | P³, HomVec4, PGL4 |
| Гравитация = сила (F = mg) | Гравитация = кривизна (геодезическое уравнение) |
| Перспективная матрица (не группа!) | Камера = PGL4 элемент (строго группа) |
| 6 плоскостей frustum | 4 проективных гиперплоскости |
| Z-buffer, z-fighting | FS-depth = d_FS(observer, point), нет z-fighting |
| AABB bounding box | FS-сфера на S³, PGL-инвариантная |
| Virtual dispatch, RTTI | Tagged unions, comptime |
| C++, 1.5M строк | Zig 0.14.0, 12K строк |

---

## Математика

### Проективное пространство P³

```
P³ = (R⁴ \ {0}) / ~,  (x,y,z,w) ~ (λx, λy, λz, λw), λ ≠ 0
PGL(4,R) — проективная линейная группа (все обратимые 4×4, по модулю скаляра)
```

### Метрика Фубини-Штуди

```
d_FS(p, q) = arccos( |⟨p,q⟩| / (‖p‖ · ‖q‖) )  ∈ [0, π/2]
```

Нет сингулярностей. Конечна при d = 0. PGL-инвариантна.

### Геодезические на S³

```
γ(t) = cos(|v|·t)·p + sin(|v|·t)·v/|v|     (p ⊥ v, ‖p‖ = ‖v‖ = 1)
ẍ = −|v|²·x                                  (центростремительное!)
```

### Дифференциальная геометрия

```
Γ^i_{jk} = +x^i · δ_{jk} / |x|²             (символы Кристоффеля)
R^i_{jkl} = δ^i_k · g_{jl} − δ^i_l · g_{jk}  (тензор Римана)
K = 1/|x|²                                    (секционная кривизна, K=1 на S³)
R = 6                                         (скалярная кривизна S³(1))
Ric_{ij} = 2K · g_{ij}                        (тензор Риччи)
```

### Идемпотентная алгебра (архетипы)

```
P² = P                                        (проектор = архетип = наблюдатель)
dP/dt = [H, P] − γ(P² − P)                   (POLER динамика)
P_{k+1} = 3P²_k − 2P³_k                      (Newton проекция на многообразие)
```

---

## Сборка и тесты

```bash
# Требуется Zig 0.14.0
zig build test              # все тесты (627+, все проходят)
zig build test-kernel       # только p3_kernel
zig build test-physics      # только p3_physics
zig build test-renderer     # только p3_renderer
zig build test-gpu-rt       # только p3_gpu_rt
zig build test-input        # только p3_input
zig build test-app          # только p3_app
zig build test-vision       # CV analyzer + temporal tracker + scene graph
zig build test-vehicle_physics  # Pacejka tire + suspension + transmission
zig build test-character_physics  # capsule walking + step-up + buoyancy
zig build test-obj_loader   # Wavefront .obj parser (Blender integration)
zig build test-texture      # mipmap streaming + bilinear sampling
zig build test-pbr          # Cook-Torrance BRDF + IBL LUT loading
# ... и т.д. для каждого из 59 модулей
```

### Build targets (headless — no GPU/X11)

```bash
zig build p3            # build engine (stubs, headless)
zig build arena-bench   # headless asteroid field arena (P3 vs Pygame benchmark)
zig build closed-loop   # LLM vision closed-loop driver
zig build race-bench    # headless race renderer (trefoil-knot track + cyber-bolide)
zig build obj-demo      # Blender .obj loader + render demo
```

### GPU executable (после zig fetch)

```bash
# Добавить zgpu + zglfw зависимости:
zig fetch --save=zgpu https://github.com/zig-gamedev/zig-gamedev/archive/refs/heads/main.tar.gz
# Тогда раскомментировать GPU-блок в build.zig и:
zig build p3               # собрать запускаемый бинарник
```

---

## Python/Zig integration (math verification)

Движок использует **Zig для core-engine кода**, а **Python — для верификации математики** через символные/численные инструменты (SymPy, NumPy, SciPy, Z3, matplotlib, Mitsuba, MuJoCo, Clifford).

### Структура `calculations/`

```
calculations/
├── README.md                    ← детальный гайд по всем 8 модулям
├── calc_character_physics.py    ← capsule-vs-triangle, step-up, buoyancy
├── calc_pbr_materials.py         ← GGX, Smith, Fresnel, BRDF LUT precompute
├── calc_skeletal_animation.py   ← LBS vs DQS, FABRIK convergence
├── calc_particle_system.py      ← Bezier, curl noise, integration
├── calc_texture_streaming.py    ← mipmap λ, anisotropic, compression
├── calc_lumen_gi.py             ← SH L2 9 coeffs, SDF, ClampedCos
├── calc_nanite.py               ← screen-space error, QEM, cluster DAG
├── calc_network_replication.py  ← fixed-point, delta compression, GGPO
├── verify_pacejka_multi_tool.py ← SymPy+NumPy+matplotlib check of vehicle physics
├── z3_verify_pacejka.py         ← Z3 SMT for linear invariants
├── verify_cgal_mujoco_character.py  ← MuJoCo ground truth for character capsule
├── verify_pga_clifford.py       ← Clifford PGA (our P³ math equivalent)
└── <module_name>/               ← output dir per script: formulas.txt, results.json, PNGs
```

### Как связаны Zig и Python тесты?

**Связь:** Python скрипты в `calculations/` верифицируют математические формулы, которые потом **реализуются в Zig-модулях**. Если SymPy нашёл что-то в формуле Pacejka, это повлияет на `src/p3_vehicle_physics.zig`.

| Python verification | Zig implementation |
|---|---|
| `verify_pacejka_multi_tool.py` | `src/p3_vehicle_physics.zig` |
| `calc_character_physics.py` (capsule, buoyancy) | `src/p3_character_physics.zig` |
| `calc_pbr_materials.py` (GGX, Fresnel) | `src/p3_pbr.zig` |
| `calc_skeletal_animation.py` (FABRIK) | `src/p3_dual_quat.zig` (existing) |
| `calc_texture_streaming.py` (mipmap λ) | `src/p3_texture.zig` |
| `calc_lumen_gi.py` (SH L2) | (TODO: `src/p3_gi.zig`) |
| `calc_nanite.py` (QEM) | (TODO: `src/p3_nanite.zig`) |
| `calc_network_replication.py` (GGPO) | (TODO: `src/p3_network.zig`) |

### Запуск верификаций локально

```bash
# Установить Python dependencies (CachyOS/Arch Linux):
sudo pacman -S --needed python-numpy python-scipy python-sympy python-matplotlib z3

# Или через pip:
pip install sympy numpy scipy matplotlib z3-solver

# Запустить все верификации:
python3 calculations/verify_pacejka_multi_tool.py    # multi-tool check
python3 calculations/calc_character_physics.py
python3 calculations/calc_pbr_materials.py            # генерирует brdf_lut.bin (32 KB)
python3 calculations/calc_skeletal_animation.py
python3 calculations/calc_particle_system.py
python3 calculations/calc_texture_streaming.py
python3 calculations/calc_lumen_gi.py
python3 calculations/calc_nanite.py
python3 calculations/calc_network_replication.py
```

### CI/CD (automated on every push/PR)

GitHub Actions workflow `.github/workflows/ci.yml`:
- **Zig tests job**: builds headless targets, runs `zig build test` + all module-specific tests
- **Python verification job**: installs SymPy/NumPy/SciPy/Z3, runs all 8 calculation scripts + pacejka verification, uploads PNG artifacts + BRDF LUT

Локально CI можно проверить через `act`:
```bash
act -W  # запустить workflow как GitHub Actions
```

---


## Дорожная карта

### Фаза 1–5 — ЗАВЕРШЕНЫ ✅

| Фаза | Модули | Что сделано |
|---|---|---|
| 1 | kernel, idempotent, geodesic, bridge, crossratio, safety | Ядро P³, FS-метрика, геодезические, типобезопасность |
| 2 | gpu, raylib, invariant, algebra | WGSL шейдеры, raylib мост, внешняя алгебра |
| 3 | ecs, scene, physics | ECS, Scene Graph на S³, физика с кривизной |
| 4 | serial, io, jobs | Сериализация, VFS, параллельные вычисления, дифгеом |
| 5 | rhi, renderer | RHI абстракция, Forward+/Deferred pipeline, P³ Camera/Frustum |

### Фаза 6 — GPU + App — ЗАВЕРШЕНА ✅

| Задача | Из O3DE | В P³ |
|---|---|---|
| GPU runtime | Atom RHI (245 файлов C++) | `p3_gpu_rt.zig`, zgpu/Dawn/WebGPU |
| Compute pipelines | AZSL→SPIR-V (runtime) | 4 pipeline: FS-distance, PGL, RK4, Idempotent |
| Render pipeline | Atom Forward+/Deferred | dehomogenize vertex + P³ fragment shader |
| Input | AzFramework::Input (event bus) | `p3_input.zig`, tagged union InputEvent |
| Camera | AzFramework::Camera (Euler) | `P3Camera` на S³, геодезическое движение |
| Application | AzFramework::Application (virtual) | `p3_app.zig`, pub fn main, comptime loop |

**P³ Engine — теперь ЗАПУСКАЕМЫЙ бинарник.** `pub fn main` реализует полный цикл: input → camera → upload MVP → dispatch FS-distance → render → present.

### Фаза 7 — Vulkan бэкенд ⬜

| Задача | Из O3DE | В P³ |
|---|---|---|
| Vulkan backend | RHI.Vulkan (245 файлов C++) | `p3_rhi_vulkan.zig`, прямые Vk* вызовы |
| Shader compilation | AZSL → SPIR-V (runtime) | WGSL → SPIR-V (naga, comptime) |
| SwapChain | RHI::SwapChain (virtual) | comptime backend: wgpu/vulkan/null |

### Фаза 8 — Ассеты и терраин ⬜

| Задача | Из O3DE | В P³ |
|---|---|---|
| Mesh pipeline | Atom::MeshFeatureProcessor | `p3_mesh.zig`, HomVec4 vertex format |
| Texture | Atom::Image (DDS/KTX/ASTC) | `p3_texture.zig`, KTX2 + BCn decode на GPU |
| Terrain | Landscape + Vegetation (Gems) | `p3_terrain.zig`, геодезический LOD на S³ |
| Material | Material::Type (SRG, shader variant) | `p3_material.zig`, comptime material graph |

### Фаза 9 — Мультиплеер и звук ⬜

| Задача | Из O3DE | В P³ |
|---|---|---|
| Networking | AzNetworking (TCP/UDP, encryption) | `p3_net.zig`, UDP + PGL4 state sync |
| Audio | AudioSystem (Wwise integration) | `p3_audio.zig`, miniaudio @cImport |
| Replication | GridMate (replica, RPC) | `p3_repl.zig`, FS-distance interest management |

### Фаза 10 — Инструменты и редактор ⬜

| Задача | Из O3DE | В P³ |
|---|---|---|
| Asset processor | AssetProcessor (job queue, 100K files) | `p3_ap.zig`, Zig comptime asset pipeline |
| Scene inspector | Editor Qt widgets | `p3_inspector.zig`, Mach sysgui / Dear ImGui |
| Prefab | PrefabSystem (nested composition) | `p3_prefab.zig`, idempotent composition |
| Profiler | RAD Telemetry / Tracy | `!Tracy @cImport, FS-integrity timeline |

---

## 18 слабых мест O3DE — и как P³ их решает

Полный анализ: [`O3DE_WEAKNESS_ANALYSIS.md`](O3DE_WEAKNESS_ANALYSIS.md)

**6 КРИТИЧЕСКИХ:**

| # | Баг O3DE | Решение P³ |
|---|---|---|
| 1 | `Normalize()` без проверки нуля → NaN | `NonZeroHomVec4` phantom-тип, error union `!Vec3` |
| 2 | `Homogenize()` делит на w без проверки | `pickBestCard()` — 4 аффинные карты, w=0 не проблема |
| 3 | Gimbal lock в Euler-конверсии | Нет Euler angles. PGL4 — всё. |
| 4 | Singleton SystemAllocator | Zig `Allocator` — параметр, не singleton |
| 5 | Нет геометрических инвариантов | Cross-ratio, FS-distance — comptime типы |
| 6 | RTTI для 38 типов | `@typeName` + `@typeInfo` — comptime, zero overhead |

---

## Зависимости

| Что | Версия | Зачем |
|---|---|---|
| Zig | 0.14.0 | Основной язык |
| zgpu | (опционально, через zig fetch) | WebGPU/Dawn бэкенд для GPU |
| zglfw | (опционально, через zig fetch) | GLFW bindings для окна и ввода |
| raylib | 5.x (опционально) | C ABI мост для 2D/GUI |
| naga | (будущее) | WGSL → SPIR-V компиляция |

---

## Лицензия

MIT

---

> «Всё есть наблюдатель, всё есть архетип» — P² = P
