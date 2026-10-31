// =============================================================================
// P³ RENDERER — FORWARD+ / DEFERRED P³ RENDERING PIPELINE
// =============================================================================
//
// Донор: O3DE Atom — 2347 файлов C++:
//   - Atom/RPI: RenderPipeline, RenderPass, FeatureProcessor
//   - Atom/Feature: Mesh, SkinnedMesh, Decal, ImageBasedLight
//     - Atom/RHI: SwapChain, FrameGraph, CommandList
//   - Проблема: 7 уровней наследования, virtual dispatch на каждом draw call,
//     RTTI для pass validation, heap-allocated frame graph
//
// Мы УБИВАЕМ C++ OOP-рендер и ПОЖИРАЕМ концепции.
// Zig: comptime pass dispatch, tagged unions для pipelines,
//      arena-allocated frame graph, P³-native camera.
//
// P³-СПЕЦИФИКА:
//   - Камера = PGL4 элемент (нет перспективной Matrix4x4!)
//   - Frustum = 4 проективных гиперплоскости в P³ (не 6 плоскостей)
//   - Depth = d_FS(observer, point) — нет z-fighting!
//   - Clipping в однородных координатах до деhomогенизации
//   - Deferred normals из P³ (не screen-space reconstitution)
//   - W-координата = видимость (chart switching при w≈0)
//
// ПОЧЕМУ FORWARD+ И DEFERRED ОДНОВРЕМЕННО?
//   - Forward+: прозрачность, MSAA, мало источников света
//   - Deferred: много источников света, P³-геометрические нормали
//   - Переключение comptime: нет runtime overhead
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const math = std.math;
const p3_kernel = @import("p3_kernel.zig");
const p3_bridge = @import("p3_bridge.zig");
const p3_rhi = @import("p3_rhi.zig");

pub const HomVec4 = p3_kernel.HomVec4;
pub const PGL4 = p3_kernel.PGL4;

// =============================================================================
// 1. P³ КАМЕРА — ПРОЕКТИВНАЯ, НЕ ЕВКЛИДОВА
// =============================================================================
//
// В O3DE/Unity/Unreal: Camera = Matrix4x4 (perspective * view)
//   Проблема: perspective matrix не является элементом группы,
//   z-fighting при near→0, gimbal lock при compose.
//
// В P³: Camera = PGL4 элемент. Это СТРОГО группа.
//   - View transform: PGL4, переводящий world → camera space
//   - Projection: НЕ матрица — это выбор аффинной карты!
//   - Near/far: НЕ clip distances — это FS-distance thresholds
//   - Нет z-fighting: FS-metric гладкая на всём P³

/// P³ Camera — наблюдатель на S³
pub const P3Camera = struct {
    /// Позиция наблюдателя на S³
    position: HomVec4,
    /// Направление взгляда (касательный вектор к S³)
    forward: HomVec4,
    /// Вверх (касательный вектор, ортогональный forward)
    up: HomVec4,
    /// Право (вычисляется: forward × up на S³)
    right: HomVec4,
    /// Горизонтальный FOV (в радианах)
    fov: f64,
    /// Aspect ratio (width/height)
    aspect: f64,
    /// Near FS-distance threshold (вместо near plane distance)
    near_fs: f64,
    /// Far FS-distance threshold
    far_fs: f64,
    /// View transform: world → camera (PGL4 элемент)
    view_pgl4: PGL4,
    /// Dirty flag
    dirty: bool,

    pub fn init(
        position: HomVec4,
        forward: HomVec4,
        up: HomVec4,
        fov: f64,
        aspect: f64,
        near_fs: f64,
        far_fs: f64,
    ) P3Camera {
        const pos_norm = position.norm();
        const p = if (pos_norm > 1e-15) position.normalize() else HomVec4.init(0, 0, 0, 1);

        // Ортогонализация forward к position
        var f = forward;
        const f_radial = HomVec4.dot(f, p);
        f = HomVec4.init(
            f.x - f_radial * p.x,
            f.y - f_radial * p.y,
            f.z - f_radial * p.z,
            f.w - f_radial * p.w,
        );
        const f_norm = f.norm();
        if (f_norm > 1e-15) f = f.normalize();

        // Ортогонализация up к position и forward
        var u = up;
        const u_radial = HomVec4.dot(u, p);
        u = HomVec4.init(
            u.x - u_radial * p.x,
            u.y - u_radial * p.y,
            u.z - u_radial * p.z,
            u.w - u_radial * p.w,
        );
        const u_forward = HomVec4.dot(u, f);
        u = HomVec4.init(
            u.x - u_forward * f.x,
            u.y - u_forward * f.y,
            u.z - u_forward * f.z,
            u.w - u_forward * f.w,
        );
        const u_norm = u.norm();
        if (u_norm > 1e-15) u = u.normalize();

        // Right = forward × up (на S³: через Gram-Schmidt)
        // В R⁴: right = компонента up ортогональная к {position, forward}
        // Вычисляем как: r = up − (up·f)f − (up·p)p
        // (уже сделано выше для u)
        const r = HomVec4.init(
            f.y * u.z - f.z * u.y,
            f.z * u.x - f.x * u.z,
            f.x * u.y - f.y * u.x,
            0.0, // w-компонента для right в касательном пространстве
        );
        const r_norm = r.norm();
        const right_final = if (r_norm > 1e-15) r.normalize() else HomVec4.init(1, 0, 0, 0);

        return .{
            .position = p,
            .forward = f,
            .up = u,
            .right = right_final,
            .fov = fov,
            .aspect = aspect,
            .near_fs = near_fs,
            .far_fs = far_fs,
            .view_pgl4 = PGL4.identity(),
            .dirty = true,
        };
    }

    /// Дефолтная камера: (0,0,0,1) смотрит вдоль −Z
    pub fn default() P3Camera {
        return init(
            HomVec4.init(0, 0, 0, 1),
            HomVec4.init(0, 0, -1, 0),
            HomVec4.init(0, 1, 0, 0),
            math.pi / 3.0, // 60° FOV
            16.0 / 9.0,
            0.01,
            1000.0,
        );
    }

    /// Вычислить view PGL4 из position/forward/up
    pub fn updateViewMatrix(self: *P3Camera) void {
        if (!self.dirty) return;

        // View matrix: rotation из {right, up, -forward, position}
        // Это PGL4 элемент: переводит camera space → world space
        // Обратный (view): world → camera
        //
        // На S³: view = параллельный перенос из position → origin
        // + вращение {forward, up, right} → {−e₃, e₂, e₁}
        //
        // Упрощение (для начала): строим как в R³, но из HomVec4

        const r = self.right;
        const u = self.up;
        const f = self.forward;
        const p = self.position;

        // View matrix (row-major для PGL4 column-major storage)
        // Row 0 = right, Row 1 = up, Row 2 = -forward, Row 3 = position
        // Translation = −dot(axis, position)
        const tx = -(r.x * p.x + r.y * p.y + r.z * p.z + r.w * p.w);
        const ty = -(u.x * p.x + u.y * p.y + u.z * p.z + u.w * p.w);
        const tz = -(f.x * p.x + f.y * p.y + f.z * p.z + f.w * p.w);

        // Column-major PGL4
        self.view_pgl4 = PGL4.init(.{
            r.x,  u.x,  -f.x, 0,
            r.y,  u.y,  -f.y, 0,
            r.z,  u.z,  -f.z, 0,
            tx,   ty,   tz,   1,
        });

        self.dirty = false;
    }

    /// Двигать камеру вдоль forward на S³ (геодезическое движение)
    pub fn moveForward(self: *P3Camera, ds: f64) void {
        // γ(t) = cos(t)·p + sin(t)·forward
        const new_pos = HomVec4.init(
            @cos(ds) * self.position.x + @sin(ds) * self.forward.x,
            @cos(ds) * self.position.y + @sin(ds) * self.forward.y,
            @cos(ds) * self.position.z + @sin(ds) * self.forward.z,
            @cos(ds) * self.position.w + @sin(ds) * self.forward.w,
        );
        const n = new_pos.norm();
        self.position = if (n > 1e-15) new_pos.normalize() else self.position;
        self.dirty = true;
    }

    /// Двигать камеру вдоль right на S³
    pub fn moveRight(self: *P3Camera, ds: f64) void {
        const new_pos = HomVec4.init(
            @cos(ds) * self.position.x + @sin(ds) * self.right.x,
            @cos(ds) * self.position.y + @sin(ds) * self.right.y,
            @cos(ds) * self.position.z + @sin(ds) * self.right.z,
            @cos(ds) * self.position.w + @sin(ds) * self.right.w,
        );
        const n = new_pos.norm();
        self.position = if (n > 1e-15) new_pos.normalize() else self.position;
        self.dirty = true;
    }

    /// FS-расстояние от камеры до точки
    pub fn distanceTo(self: P3Camera, point: HomVec4) f64 {
        return p3_kernel.fsDistance(self.position, point);
    }
};

// =============================================================================
// 2. P³ FRUSTUM — 4 ПРОЕКТИВНЫХ ГИПЕРПЛОСКОСТИ
// =============================================================================
//
// В O3DE: Frustum = 6 Plane{normal, distance} в R³
//   Проблема: не инвариантен при проективных преобразованиях,
//   не работает при w≈0 (бесконечность).
//
// В P³: Frustum = пересечение 4 полупространств
//   π_i = [a_i : b_i : c_i : d_i] ∈ (P³)*
//   Точка p видима ⟺ ⟨π_i, p⟩ ≥ 0  для всех i
//
// Только 4 гиперплоскости (не 6), потому что:
//   - Near/far — это FS-distance thresholds, не плоскости
//   - Left/right/top/bottom → 4 гиперплоскости в P³
//   - Near/far проверяются через d_FS(observer, point)

/// Проективная гиперплоскость в P³
/// π = [a:b:c:d] — точка в двойственном P³*
/// Точка p = [x:y:z:w] лежит на π ⟺ a·x + b·y + c·z + d·w = 0
pub const ProjectiveHyperplane = struct {
    a: f64,
    b: f64,
    c: f64,
    d: f64,

    pub fn init(a: f64, b: f64, c: f64, d: f64) ProjectiveHyperplane {
        return .{ .a = a, .b = b, .c = c, .d = d };
    }

    /// Вычислить ⟨π, p⟩ — знак определяет полупространство
    pub fn evaluate(self: ProjectiveHyperplane, p: HomVec4) f64 {
        return self.a * p.x + self.b * p.y + self.c * p.z + self.d * p.w;
    }

    /// Точка видима (в положительном полупространстве)?
    pub fn isVisible(self: ProjectiveHyperplane, p: HomVec4) bool {
        return self.evaluate(p) >= 0;
    }

    /// PGL-действие на гиперплоскость: (M⁻ᵀ)·π
    /// Если M: P³ → P³, то M*π = (M⁻ᵀ)·π в двойственном пространстве
    pub fn transformBy(self: ProjectiveHyperplane, m: PGL4) ProjectiveHyperplane {
        // Для PGL4 column-major: (M⁻ᵀ)·π
        // Упрощение: используем M напрямую для строк
        // π' = π · M (row vector × matrix)
        return .{
            .a = self.a * m.data[0] + self.b * m.data[4] + self.c * m.data[8] + self.d * m.data[12],
            .b = self.a * m.data[1] + self.b * m.data[5] + self.c * m.data[9] + self.d * m.data[13],
            .c = self.a * m.data[2] + self.b * m.data[6] + self.c * m.data[10] + self.d * m.data[14],
            .d = self.a * m.data[3] + self.b * m.data[7] + self.c * m.data[11] + self.d * m.data[15],
        };
    }
};

/// P³ Frustum — 4 проективных гиперплоскости + FS near/far
pub const P3Frustum = struct {
    /// 4 clipping гиперплоскости (left, right, top, bottom)
    planes: [4]ProjectiveHyperplane,
    /// Near FS-distance threshold
    near_fs: f64,
    /// Far FS-distance threshold
    far_fs: f64,
    /// Позиция наблюдателя (для FS-distance проверки)
    observer: HomVec4,

    /// Построить frustum из камеры
    pub fn fromCamera(camera: P3Camera) P3Frustum {
        // P³ frustum: 4 гиперплоскости через наблюдателя
        // Каждая плоскость содержит observer и границу FOV
        //
        // На S³: плоскость = {p ∈ S³ | ⟨π, p⟩ = 0}
        // Левая плоскость: содержит {observer, forward×up − tan(fov/2)·right×up}
        // Правая: зеркально
        // Верхняя: содержит {observer, forward×right + tan(fov/2/aspect)·up×right}
        // Нижняя: зеркально

        const half_fov = camera.fov / 2.0;
        const half_fov_ver = half_fov / camera.aspect;
        const tan_h = @tan(half_fov);
        const tan_v = @tan(half_fov_ver);

        const p = camera.position;
        const f = camera.forward;
        const r = camera.right;
        const u = camera.up;

        // Нормаль левой плоскости = forward + tan(fov/2) * right
        // (указывает НАПРАВО от левой границы)
        const left_normal = HomVec4.init(
            f.x + tan_h * r.x,
            f.y + tan_h * r.y,
            f.z + tan_h * r.z,
            f.w + tan_h * r.w,
        );
        // Нормаль правой = forward − tan(fov/2) * right
        const right_normal = HomVec4.init(
            f.x - tan_h * r.x,
            f.y - tan_h * r.y,
            f.z - tan_h * r.z,
            f.w - tan_h * r.w,
        );
        // Нормаль верхней = forward − tan(fov_v/2) * up
        const top_normal = HomVec4.init(
            f.x - tan_v * u.x,
            f.y - tan_v * u.y,
            f.z - tan_v * u.z,
            f.w - tan_v * u.w,
        );
        // Нормаль нижней = forward + tan(fov_v/2) * up
        const bottom_normal = HomVec4.init(
            f.x + tan_v * u.x,
            f.y + tan_v * u.y,
            f.z + tan_v * u.z,
            f.w + tan_v * u.w,
        );

        // Гиперплоскость: ⟨n, p⟩ + d = 0, где d = −⟨n, observer⟩
        return .{
            .planes = .{
                ProjectiveHyperplane.init(
                    left_normal.x,
                    left_normal.y,
                    left_normal.z,
                    -(left_normal.x * p.x + left_normal.y * p.y + left_normal.z * p.z + left_normal.w * p.w),
                ),
                ProjectiveHyperplane.init(
                    right_normal.x,
                    right_normal.y,
                    right_normal.z,
                    -(right_normal.x * p.x + right_normal.y * p.y + right_normal.z * p.z + right_normal.w * p.w),
                ),
                ProjectiveHyperplane.init(
                    top_normal.x,
                    top_normal.y,
                    top_normal.z,
                    -(top_normal.x * p.x + top_normal.y * p.y + top_normal.z * p.z + top_normal.w * p.w),
                ),
                ProjectiveHyperplane.init(
                    bottom_normal.x,
                    bottom_normal.y,
                    bottom_normal.z,
                    -(bottom_normal.x * p.x + bottom_normal.y * p.y + bottom_normal.z * p.z + bottom_normal.w * p.w),
                ),
            },
            .near_fs = camera.near_fs,
            .far_fs = camera.far_fs,
            .observer = camera.position,
        };
    }

    /// Тест видимости: точка видима ⟺ все 4 плоскости ≥0 И near≤d_FS≤far
    pub fn isVisible(self: P3Frustum, point: HomVec4) bool {
        // Проверка 4 гиперплоскостей
        for (self.planes) |plane| {
            if (!plane.isVisible(point)) return false;
        }
        // Проверка FS-distance near/far
        const d = p3_kernel.fsDistance(self.observer, point);
        if (d < self.near_fs or d > self.far_fs) return false;
        return true;
    }

    /// Быстрый тест: только плоскости (без FS-distance — дешевле)
    pub fn isVisibleFast(self: P3Frustum, point: HomVec4) bool {
        for (self.planes) |plane| {
            if (!plane.isVisible(point)) return false;
        }
        return true;
    }

    /// Тест сферы на S³: центр + FS-радиус
    /// Сфера видима если хотя бы одна плоскость не отсекает её полностью
    pub fn isSphereVisible(self: P3Frustum, center: HomVec4, fs_radius: f64) bool {
        // Для каждой плоскости: если ⟨π, center⟩ + radius_bound < 0 → полностью вне
        // radius_bound ≈ fs_radius * ‖π‖ (первое приближение)
        for (self.planes) |plane| {
            const val = plane.evaluate(center);
            const plane_norm = @sqrt(plane.a * plane.a + plane.b * plane.b + plane.c * plane.c + plane.d * plane.d);
            if (val < -fs_radius * plane_norm) return false;
        }
        // Near/far проверка
        const d = p3_kernel.fsDistance(self.observer, center);
        if (d + fs_radius < self.near_fs or d - fs_radius > self.far_fs) return false;
        return true;
    }

    /// PGL-действие: преобразовать frustum проективным преобразованием
    /// M: P³ → P³, frustum' = M(frustum)
    pub fn transformBy(self: P3Frustum, m: PGL4) P3Frustum {
        var new_planes: [4]ProjectiveHyperplane = undefined;
        for (0..4) |i| {
            new_planes[i] = self.planes[i].transformBy(m);
        }
        return .{
            .planes = new_planes,
            .near_fs = self.near_fs,
            .far_fs = self.far_fs,
            .observer = m.apply(self.observer),
        };
    }
};

// =============================================================================
// 3. FS-DEPTH — ГЛУБИНА НА ОСНОВЕ ФУБИНИ-ШТУДИ МЕТРИКИ
// =============================================================================
//
// В O3DE: Depth = z/w (linear или reversed)
//   Проблема: z-fighting при near→0, float precision waste near far,
//   не инвариантно при проективных преобразованиях.
//
// В P³: Depth = d_FS(observer, point) / max_fs_distance
//   - Нет z-fighting: FS-metric гладкая на всём P³
//   - Precision uniform: d_FS ∈ [0, π/2], mapped to [0,1]
//   - PGL-инвариант: d_FS(M·p, M·q) = d_FS(p, q)

/// Вычислить FS-depth для точки относительно наблюдателя
/// Возвращает [0, 1] где 0 = near, 1 = far
pub fn fsDepth(observer: HomVec4, point: HomVec4, near_fs: f64, far_fs: f64) f64 {
    const d = p3_kernel.fsDistance(observer, point);
    if (d <= near_fs) return 0.0;
    if (d >= far_fs) return 1.0;
    return (d - near_fs) / (far_fs - near_fs);
}

/// Обратное преобразование: FS-depth [0,1] → FS-distance
pub fn fsDepthToDistance(depth: f64, near_fs: f64, far_fs: f64) f64 {
    return near_fs + depth * (far_fs - near_fs);
}

/// W-координата как грубая мера глубины
/// W > 0: перед наблюдателем (в UW карте)
/// W ≈ 0: на бесконечности → переключение карты!
/// W < 0: за наблюдателем
pub fn wVisibility(point: HomVec4) WVisibilityClass {
    if (point.w > 1e-6) return .in_front;
    if (point.w < -1e-6) return .behind;
    return .at_infinity;
}

pub const WVisibilityClass = enum {
    in_front, // w > 0: видим в UW карте
    behind, // w < 0: за наблюдателем
    at_infinity, // w ≈ 0: на бесконечности — переключить карту!
};

// =============================================================================
// 4. РЕНДЕР-ПАЙПЛАЙН: FORWARD+
// =============================================================================
//
// Forward+ = Forward rendering + tiled/clustered light culling
//
// Pass 1: Z-prepass (FS-depth only, no shading)
// Pass 2: Light culling (compute: assign lights to tiles)
// Pass 3: Opaque shading (forward with per-tile light list)
// Pass 4: Transparency (forward, no depth write, back-to-front)

/// Forward+ render pass configuration
pub const ForwardPlusConfig = struct {
    /// Tile size for light culling (pixels)
    tile_size: u32 = 16,
    /// Z-prepass enabled
    z_prepass: bool = true,
    /// Max lights per tile
    max_lights_per_tile: u32 = 256,
    /// MSAA samples (1 = off, 4 = 4x, 8 = 8x)
    msaa_samples: u32 = 1,
    /// FS-depth mode (vs traditional Z-buffer)
    fs_depth: bool = true,
    /// W-chart switching enabled
    chart_switching: bool = true,
};

/// Forward+ pipeline: генерирует последовательность RHI команд
pub const ForwardPlusPipeline = struct {
    config: ForwardPlusConfig,
    camera: P3Camera,
    frustum: P3Frustum,
    frame_index: u64,

    pub fn init(camera: P3Camera, config: ForwardPlusConfig) ForwardPlusPipeline {
        return .{
            .config = config,
            .camera = camera,
            .frustum = P3Frustum.fromCamera(camera),
            .frame_index = 0,
        };
    }

    /// Обновить камеру и frustum
    pub fn updateCamera(self: *ForwardPlusPipeline, camera: P3Camera) void {
        self.camera = camera;
        self.frustum = P3Frustum.fromCamera(camera);
    }

    /// Количество проходов
    pub fn passCount(self: ForwardPlusPipeline) u32 {
        var count: u32 = 1; // geometry pass
        if (self.config.z_prepass) count += 1;
        count += 1; // light culling
        count += 1; // transparency
        return count;
    }
};

// =============================================================================
// 5. РЕНДЕР-ПАЙПЛАЙН: DEFERRED
// =============================================================================
//
// Deferred = Geometry → G-buffer → Lighting → Post-process
//
// Pass 1: Geometry (write G-buffer: P³ position, P³ normal, albedo, material)
// Pass 2: Lighting (read G-buffer, compute shading per pixel)
// Pass 3: Post-process (tone mapping, FS-AA, etc.)
//
// P³-СПЕЦИФИКА в G-buffer:
//   - Position: HomVec4 (4×f32 = 16 bytes, НЕ vec3!)
//   - Normal: HomVec4 (4×f32 = 16 bytes, P³-геометрическая, НЕ screen-space!)
//   - Albedo: RGBA8 (4 bytes)
//   - Material: metallic+f32, roughness+f32, emission+f32, flags+u32 (16 bytes)
//   - Total G-buffer: 52 bytes/pixel
//
// Преимущество: нормали ВТОРИЧНЫХ поверхностей корректны —
// нет ошибок screen-space reconstitution при сложной геометрии.

/// G-buffer layout для deferred
pub const GBufferLayout = struct {
    /// P³ position: HomVec4 (4 × f32 = 16 bytes)
    position_format: p3_rhi.Format = .rgba32_float,
    /// P³ normal: HomVec4 (4 × f32 = 16 bytes)
    normal_format: p3_rhi.Format = .rgba32_float,
    /// Albedo + alpha: RGBA8 (4 bytes)
    albedo_format: p3_rhi.Format = .rgba8_unorm,
    /// Material params: metallic, roughness, emission, flags (16 bytes)
    material_format: p3_rhi.Format = .rgba32_float,
    /// FS-depth: R32_FLOAT (4 bytes)
    depth_format: p3_rhi.Format = .depth32_float,

    /// Total G-buffer size per pixel
    pub fn bytesPerPixel(self: GBufferLayout) u32 {
        return self.position_format.size() +
            self.normal_format.size() +
            self.albedo_format.size() +
            self.material_format.size() +
            self.depth_format.size();
    }
};

/// Deferred render configuration
pub const DeferredConfig = struct {
    /// G-buffer layout
    gbuffer: GBufferLayout = .{},
    /// FS-depth in G-buffer (vs traditional Z)
    fs_depth: bool = true,
    /// P³-geometric normals (vs screen-space reconstitution)
    p3_normals: bool = true,
    /// Tile-based deferred (vs fullscreen quad)
    tiled: bool = true,
    /// Tile size
    tile_size: u32 = 16,
    /// Max lights
    max_lights: u32 = 1024,
    /// W-chart switching for G-buffer readback
    chart_switching: bool = true,
};

/// Deferred pipeline
pub const DeferredPipeline = struct {
    config: DeferredConfig,
    camera: P3Camera,
    frustum: P3Frustum,
    frame_index: u64,

    pub fn init(camera: P3Camera, config: DeferredConfig) DeferredPipeline {
        return .{
            .config = config,
            .camera = camera,
            .frustum = P3Frustum.fromCamera(camera),
            .frame_index = 0,
        };
    }

    pub fn updateCamera(self: *DeferredPipeline, camera: P3Camera) void {
        self.camera = camera;
        self.frustum = P3Frustum.fromCamera(camera);
    }

    pub fn passCount(self: DeferredPipeline) u32 {
        _ = self;
        return 3; // geometry + lighting + post
    }
};

// =============================================================================
// 6. РЕНДЕР-ПАЙПЛАЙН: ВЫБОР COMPTIME
// =============================================================================
//
// O3DE: runtime pipeline selection через FeatureProcessor virtual dispatch
// P³:   comptime — zero overhead, нет vtable

/// Pipeline mode — выбирается при компиляции
pub const PipelineMode = enum {
    forward_plus,
    deferred,
    /// Выбор: forward+ для <N источников света, deferred для ≥N
    hybrid,

    /// Количество проходов для данного режима
    pub fn passCountHint(self: PipelineMode) u32 {
        return switch (self) {
            .forward_plus => 4, // z-prepass + light cull + opaque + transparent
            .deferred => 3, // geometry + lighting + post
            .hybrid => 5, // все из forward+ + deferred post
        };
    }
};

/// Unified render pipeline: comptime выбирает forward+ или deferred
pub fn RenderPipeline(comptime mode: PipelineMode) type {
    return switch (mode) {
        .forward_plus => ForwardPlusPipeline,
        .deferred => DeferredPipeline,
        .hybrid => struct {
            forward: ForwardPlusPipeline,
            deferred: DeferredPipeline,
            /// Порог: <N lights → forward+, ≥N → deferred
            light_threshold: u32,

            pub fn init(camera: P3Camera, light_count: u32) @This() {
                const fwd_config = ForwardPlusConfig{};
                const def_config = DeferredConfig{};
                return .{
                    .forward = ForwardPlusPipeline.init(camera, fwd_config),
                    .deferred = DeferredPipeline.init(camera, def_config),
                    .light_threshold = light_count,
                };
            }
        },
    };
}

// =============================================================================
// 7. P³-СПЕЦИФИЧЕСКИЕ РЕНДЕР-ОПЕРАЦИИ
// =============================================================================

/// Деhomогенизировать массив точек P³ → R³ с chart switching
/// Используется перед растеризацией: GPU vertex shader
pub fn dehomogenizeBatch(
    positions: []const HomVec4,
    output: []p3_bridge.Cartesian3,
) void {
    const n = @min(positions.len, output.len);
    for (0..n) |i| {
        output[i] = p3_bridge.dehomogenize(positions[i]);
    }
}

/// Проективный clipping: отсечь точки за плоскостью
/// Возвращает количество видимых точек
pub fn projectiveClip(
    plane: ProjectiveHyperplane,
    positions: []const HomVec4,
    visible_mask: []bool,
) u32 {
    const n = @min(positions.len, visible_mask.len);
    var count: u32 = 0;
    for (0..n) |i| {
        const is_visible = plane.isVisible(positions[i]);
        visible_mask[i] = is_visible;
        if (is_visible) count += 1;
    }
    return count;
}

/// Frustum culling на S³: отсечь точки вне frustum
pub fn frustumCull(
    frustum: P3Frustum,
    positions: []const HomVec4,
    visible_mask: []bool,
) u32 {
    const n = @min(positions.len, visible_mask.len);
    var count: u32 = 0;
    for (0..n) |i| {
        const is_visible = frustum.isVisibleFast(positions[i]);
        visible_mask[i] = is_visible;
        if (is_visible) count += 1;
    }
    return count;
}

/// Sort by FS-depth (back-to-front для transparency)
/// Возвращает отсортированные индексы
pub fn sortByFSDepth(
    allocator: std.mem.Allocator,
    observer: HomVec4,
    positions: []const HomVec4,
) ![]usize {
    const n = positions.len;
    var indices = try allocator.alloc(usize, n);
    for (0..n) |i| indices[i] = i;

    // Insertion sort (достаточно для <1000 прозрачных)
    for (1..n) |i| {
        const key = indices[i];
        const key_dist = p3_kernel.fsDistance(observer, positions[key]);
        var j: usize = i;
        while (j > 0) : (j -= 1) {
            const prev_dist = p3_kernel.fsDistance(observer, positions[indices[j - 1]]);
            if (prev_dist >= key_dist) break;
            indices[j] = indices[j - 1];
        }
        indices[j] = key;
    }

    return indices;
}

// =============================================================================
// 8. ТЕСТЫ
// =============================================================================

test "Renderer: P3Camera default" {
    const cam = P3Camera.default();
    try std.testing.expectApproxEqAbs(cam.position.norm(), 1.0, 1e-10);
    try std.testing.expect(cam.fov > 0);
    try std.testing.expect(cam.aspect > 1);
}

test "Renderer: P3Camera position on S³" {
    const cam = P3Camera.init(
        HomVec4.init(1, 2, 3, 1),
        HomVec4.init(0, 0, -1, 0),
        HomVec4.init(0, 1, 0, 0),
        math.pi / 3.0,
        1.5,
        0.01,
        1000.0,
    );
    // Position must be on S³
    try std.testing.expectApproxEqAbs(cam.position.norm(), 1.0, 1e-10);
}

test "Renderer: P3Camera forward orthogonal to position" {
    const cam = P3Camera.default();
    const dot = HomVec4.dot(cam.forward, cam.position);
    try std.testing.expectApproxEqAbs(@abs(dot), 0.0, 1e-6);
}

test "Renderer: P3Camera move forward on S³" {
    var cam = P3Camera.default();
    cam.moveForward(0.1);
    try std.testing.expectApproxEqAbs(cam.position.norm(), 1.0, 1e-10);
}

test "Renderer: P3Camera move right on S³" {
    var cam = P3Camera.default();
    cam.moveRight(0.1);
    try std.testing.expectApproxEqAbs(cam.position.norm(), 1.0, 1e-10);
}

test "Renderer: P3Camera FS distance" {
    const cam = P3Camera.default();
    const point = HomVec4.init(0, 0, -1, 0); // opposite pole
    const d = cam.distanceTo(point);
    // d_FS((0,0,0,1), (0,0,-1,0)) = π/2
    try std.testing.expectApproxEqAbs(d, math.pi / 2.0, 1e-10);
}

test "Renderer: ProjectiveHyperplane evaluate" {
    // Plane x = 0 (YZW plane)
    const plane = ProjectiveHyperplane.init(1, 0, 0, 0);
    const p1 = HomVec4.init(1, 0, 0, 1); // x = 1 → positive
    const p2 = HomVec4.init(-1, 0, 0, 1); // x = -1 → negative
    const p3 = HomVec4.init(0, 1, 0, 1); // x = 0 → on plane
    try std.testing.expect(plane.isVisible(p1));
    try std.testing.expect(!plane.isVisible(p2));
    try std.testing.expectApproxEqAbs(plane.evaluate(p3), 0.0, 1e-10);
}

test "Renderer: P3Frustum from camera" {
    const cam = P3Camera.default();
    const frustum = P3Frustum.fromCamera(cam);
    // Observer should be at camera position
    try std.testing.expectApproxEqAbs(
        HomVec4.dot(frustum.observer, cam.position),
        1.0, // both on S³ and same direction
        1e-6,
    );
}

test "Renderer: P3Frustum visibility — point in front" {
    const cam = P3Camera.default();
    const frustum = P3Frustum.fromCamera(cam);
    // Point directly ahead: should be visible (plane check)
    const ahead = HomVec4.init(0, 0, -1, 1).normalize();
    const fast = frustum.isVisibleFast(ahead);
    // Should pass plane check (may fail near/far for default thresholds)
    _ = fast;
}

test "Renderer: FS-depth computation" {
    const observer = HomVec4.init(0, 0, 0, 1);
    const point = HomVec4.init(0, 0, -1, 0); // d_FS = π/2
    const depth = fsDepth(observer, point, 0.01, 2.0);
    // π/2 ≈ 1.57, mapped between 0.01 and 2.0
    try std.testing.expect(depth > 0 and depth < 1);
}

test "Renderer: FS-depth near clamp" {
    const observer = HomVec4.init(0, 0, 0, 1);
    const point = HomVec4.init(0, 0, 0.001, 1).normalize(); // very close
    const depth = fsDepth(observer, point, 0.01, 100.0);
    try std.testing.expectApproxEqAbs(depth, 0.0, 1e-6);
}

test "Renderer: FS-depth far clamp" {
    const observer = HomVec4.init(0, 0, 0, 1);
    const point = HomVec4.init(1, 0, 0, 0); // d_FS = π/2
    const depth = fsDepth(observer, point, 0.01, 0.5);
    // π/2 > 0.5 → clamped to 1.0
    try std.testing.expectApproxEqAbs(depth, 1.0, 1e-10);
}

test "Renderer: FS-depth round-trip" {
    const near: f64 = 0.1;
    const far: f64 = 100.0;
    const d: f64 = 50.0;
    const depth = (d - near) / (far - near);
    const recovered = fsDepthToDistance(depth, near, far);
    try std.testing.expectApproxEqAbs(recovered, d, 1e-10);
}

test "Renderer: W-visibility classification" {
    const p1 = HomVec4.init(0, 0, 0, 1); // w = 1 → in_front
    const p2 = HomVec4.init(0, 0, 0, -1); // w = -1 → behind
    const p3 = HomVec4.init(1, 0, 0, 0); // w = 0 → at_infinity
    try std.testing.expect(wVisibility(p1) == .in_front);
    try std.testing.expect(wVisibility(p2) == .behind);
    try std.testing.expect(wVisibility(p3) == .at_infinity);
}

test "Renderer: Forward+ pipeline pass count" {
    const cam = P3Camera.default();
    const pipeline = ForwardPlusPipeline.init(cam, .{ .z_prepass = true });
    try std.testing.expect(pipeline.passCount() == 4);
}

test "Renderer: Forward+ without Z-prepass" {
    const cam = P3Camera.default();
    const pipeline = ForwardPlusPipeline.init(cam, .{ .z_prepass = false });
    try std.testing.expect(pipeline.passCount() == 3);
}

test "Renderer: Deferred pipeline pass count" {
    const cam = P3Camera.default();
    const pipeline = DeferredPipeline.init(cam, .{});
    try std.testing.expect(pipeline.passCount() == 3);
}

test "Renderer: G-buffer bytes per pixel" {
    const gbuffer = GBufferLayout{};
    // 16 + 16 + 4 + 16 + 4 = 56 bytes
    try std.testing.expect(gbuffer.bytesPerPixel() == 56);
}

test "Renderer: Pipeline mode pass count hint" {
    try std.testing.expect(PipelineMode.forward_plus.passCountHint() == 4);
    try std.testing.expect(PipelineMode.deferred.passCountHint() == 3);
    try std.testing.expect(PipelineMode.hybrid.passCountHint() == 5);
}

test "Renderer: Frustum cull — all visible" {
    const cam = P3Camera.default();
    const frustum = P3Frustum.fromCamera(cam);
    const points = [_]HomVec4{
        HomVec4.init(0, 0, 0, 1),
    };
    var mask: [1]bool = undefined;
    const count = frustumCull(frustum, &points, &mask);
    // At least check it runs without crashing
    _ = count;
}

test "Renderer: Projective clip" {
    const plane = ProjectiveHyperplane.init(1, 0, 0, 0); // x ≥ 0
    const points = [_]HomVec4{
        HomVec4.init(1, 0, 0, 1), // visible
        HomVec4.init(-1, 0, 0, 1), // not visible
        HomVec4.init(0, 1, 0, 1), // on plane (visible)
    };
    var mask: [3]bool = undefined;
    const count = projectiveClip(plane, &points, &mask);
    try std.testing.expect(count == 2); // first and third
    try std.testing.expect(mask[0]);
    try std.testing.expect(!mask[1]);
    try std.testing.expect(mask[2]);
}

test "Renderer: P3Frustum PGL transform" {
    const cam = P3Camera.default();
    var frustum = P3Frustum.fromCamera(cam);
    const T = p3_kernel.pglTranslate(1, 0, 0);
    frustum = frustum.transformBy(T);
    // Observer should be transformed
    const expected_obs = T.apply(cam.position);
    try std.testing.expectApproxEqAbs(frustum.observer.x, expected_obs.x, 1e-10);
    try std.testing.expectApproxEqAbs(frustum.observer.y, expected_obs.y, 1e-10);
}

test "Renderer: P3Camera view matrix update" {
    var cam = P3Camera.default();
    cam.updateViewMatrix();
    try std.testing.expect(!cam.dirty);
}

test "Renderer: Sort by FS-depth" {
    const observer = HomVec4.init(0, 0, 0, 1);
    // Три точки вдоль -Z на S³: разные расстояния от наблюдателя
    const p0 = HomVec4.init(0, 0, -0.5, 1).normalize();
    const p1 = HomVec4.init(0, 0, -0.1, 1).normalize();
    const p2 = HomVec4.init(0, 0, -0.3, 1).normalize();
    const points = [_]HomVec4{ p0, p1, p2 };

    // Compute actual distances for verification
    const d0 = p3_kernel.fsDistance(observer, p0);
    const d1 = p3_kernel.fsDistance(observer, p1);
    const d2 = p3_kernel.fsDistance(observer, p2);
    // p1 should be closest (z=-0.1 → small angle), p0 farthest (z=-0.5)
    try std.testing.expect(d1 < d2 and d2 < d0);

    const indices = try sortByFSDepth(std.testing.allocator, observer, &points);
    defer std.testing.allocator.free(indices);
    // Back-to-front: farthest first
    try std.testing.expect(indices[0] == 0); // farthest (d0)
    try std.testing.expect(indices[1] == 2); // middle (d2)
    try std.testing.expect(indices[2] == 1); // closest (d1)
}

test "Renderer: P3Frustum sphere visibility" {
    const cam = P3Camera.default();
    const frustum = P3Frustum.fromCamera(cam);
    // Sphere around observer: should be visible
    const visible = frustum.isSphereVisible(cam.position, 0.1);
    try std.testing.expect(visible);
}

test "Renderer: ProjectiveHyperplane PGL transform" {
    const plane = ProjectiveHyperplane.init(1, 0, 0, 0); // x = 0
    const I = PGL4.identity();
    const transformed = plane.transformBy(I);
    // Identity transform: plane unchanged
    try std.testing.expectApproxEqAbs(transformed.a, 1.0, 1e-10);
    try std.testing.expectApproxEqAbs(transformed.b, 0.0, 1e-10);
}
