// =============================================================================
// P³ APP — ЗАПУСКАЕМЫЙ ДВИЖОК (pub fn main)
// =============================================================================
//
// Это ФИНАЛЬНЫЙ шаг Фазы 6. P³ Engine — больше НЕ библиотека.
// Это ЗАПУСКАЕМЫЙ бинарник с pub fn main.
//
// Архитектура:
//
//   pub fn main()
//     → zglfw.init()          // Window system
//     → zgpu.GraphicsContext  // GPU device + swapchain
//     → P3GpuContext.init()   // P³ pipelines + buffers
//     → main loop:
//         input.update()
//         camera.update(input)
//         gpu_ctx.uploadPositions()
//         gpu_ctx.dispatchFSDistance()
//         gpu_ctx.renderFrame()
//         gpu_ctx.present()
//
// Донор: O3DE AzFramework::Application — 34 файла C++:
//   - ApplicationLifecycle, ComponentApplication, SystemComponent
//   - Проблема: 6 уровней наследования, virtual init/update/shutdown,
//     singleton Application, Gem dependency graph
//
// Мы УБИВАЕМ C++ OOP application lifecycle и ПОЖИРАЕМ концепции.
// Zig: flat main(), explicit init/deinit, arena per frame, no singletons.
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const zglfw = @import("zglfw");
const zgpu = @import("zgpu");
const p3_gpu_rt = @import("p3_gpu_rt.zig");
const p3_input = @import("p3_input.zig");
const p3_kernel = @import("p3_kernel.zig");
const p3_gpu = @import("p3_gpu.zig");

pub const HomVec4 = p3_kernel.HomVec4;
pub const PGL4 = p3_kernel.PGL4;

// =============================================================================
// 0. WINDOW PROVIDER WRAPPER — ZGLFW → ZGPU BRIDGE
// =============================================================================
//
// zglfw.Window.getFramebufferSize() returns [2]c_int, but
// zgpu.WindowProvider.fn_getFramebufferSize expects [2]u32.
// We need a wrapper function that converts between these types.

/// Wrapper for zgpu.WindowProvider.fn_getFramebufferSize
/// Converts zglfw's [2]c_int → [2]u32 (framebuffer sizes are always non-negative)
fn getFramebufferSizeForZgpu(window: *const anyopaque) [2]u32 {
    const w: *zglfw.Window = @constCast(@ptrCast(@alignCast(window)));
    const size = w.getFramebufferSize();
    return .{
        @intCast(size[0]),
        @intCast(size[1]),
    };
}

// =============================================================================
// 1. КОНФИГУРАЦИЯ ПРИЛОЖЕНИЯ
// =============================================================================

pub const AppConfig = struct {
    window_width: c_int = 1280,
    window_height: c_int = 720,
    window_title: [:0]const u8 = "P³ Engine — Projective Geometry",
    max_points: u32 = 100_000,
    max_transforms: u32 = 10_000,
    target_fps: f64 = 60.0,
    vsync: bool = true,
};

// =============================================================================
// 2. P³ CAMERA НА S³
// =============================================================================
//
// Камера — точка на S³ с направлением. Движение по геодезическим.
// НЕТ Euler angles. НЕТ perspective Matrix4x4. PGL4 — всё.

pub const P3Camera = struct {
    // Position on S³ (unit 4-vector)
    pos: [4]f64 = .{ 0, 0, 0, 1 }, // [0,0,0,1] = origin in UW chart
    // Forward direction (tangent to S³, perpendicular to pos)
    forward: [4]f64 = .{ 0, 0, -1, 0 },
    // Up direction
    up: [4]f64 = .{ 0, 1, 0, 0 },
    // Right direction
    right: [4]f64 = .{ 1, 0, 0, 0 },
    // Speed: units per second on S³
    speed: f64 = 2.0,
    // Orbit sensitivity
    orbit_sens: f64 = 0.003,
    // FS near/far for frustum
    fs_near: f64 = 0.01,
    fs_far: f64 = 3.14, // ≈ π (half of S³)

    /// Обновить камеру из ввода (dt в секундах)
    pub fn update(self: *P3Camera, input: *const p3_input.InputState, dt: f64) void {
        const move_dir = input.cameraMoveDir();
        const orbit = input.cameraOrbitDelta();
        const step = self.speed * dt;

        // --- Translation along geodesics ---
        // Small step: exponential map ≈ pos + step * tangent
        // With re-normalization to stay on S³
        const tangent = [4]f64{
            move_dir[0] * self.right[0] + move_dir[1] * self.up[0] + move_dir[2] * self.forward[0],
            move_dir[0] * self.right[1] + move_dir[1] * self.up[1] + move_dir[2] * self.forward[1],
            move_dir[0] * self.right[2] + move_dir[1] * self.up[2] + move_dir[2] * self.forward[2],
            move_dir[0] * self.right[3] + move_dir[1] * self.up[3] + move_dir[2] * self.forward[3],
        };

        // RK1 (Euler) step on S³: γ(t+dt) ≈ cos(|v|dt)·pos + sin(|v|dt)·v/|v|
        const v_len = @sqrt(tangent[0] * tangent[0] + tangent[1] * tangent[1] + tangent[2] * tangent[2] + tangent[3] * tangent[3]);
        if (v_len > 1e-10) {
            const angle = v_len * step;
            const cos_a = @cos(angle);
            const sin_a = @sin(angle);
            const inv_len = 1.0 / v_len;

            self.pos[0] = cos_a * self.pos[0] + sin_a * tangent[0] * inv_len;
            self.pos[1] = cos_a * self.pos[1] + sin_a * tangent[1] * inv_len;
            self.pos[2] = cos_a * self.pos[2] + sin_a * tangent[2] * inv_len;
            self.pos[3] = cos_a * self.pos[3] + sin_a * tangent[3] * inv_len;
        }

        // --- Orbit (mouse) ---
        // Rotate forward/up around right, forward/right around up
        const yaw = -orbit[0] * self.orbit_sens;
        const pitch = orbit[1] * self.orbit_sens;

        // Yaw: rotate forward and right around up
        if (@abs(yaw) > 1e-10) {
            const cos_y = @cos(yaw);
            const sin_y = @sin(yaw);
            var new_fwd = self.forward;
            var new_right = self.right;
            for (0..4) |i| {
                new_fwd[i] = cos_y * self.forward[i] + sin_y * self.right[i];
                new_right[i] = -sin_y * self.forward[i] + cos_y * self.right[i];
            }
            self.forward = new_fwd;
            self.right = new_right;
        }

        // Pitch: rotate forward and up around right
        if (@abs(pitch) > 1e-10) {
            const cos_p = @cos(pitch);
            const sin_p = @sin(pitch);
            var new_fwd = self.forward;
            var new_up = self.up;
            for (0..4) |i| {
                new_fwd[i] = cos_p * self.forward[i] + sin_p * self.up[i];
                new_up[i] = -sin_p * self.forward[i] + cos_p * self.up[i];
            }
            self.forward = new_fwd;
            self.up = new_up;
        }

        // Re-orthonormalize (Gram-Schmidt on S³)
        self.reorthonormalize();

        // Scroll → speed
        self.speed = @max(0.1, self.speed + input.mouse.scroll_y * 0.5);
    }

    /// Gram-Schmidt re-orthonormalization
    fn reorthonormalize(self: *P3Camera) void {
        // Normalize pos
        var pos_len: f64 = 0;
        for (0..4) |i| pos_len += self.pos[i] * self.pos[i];
        pos_len = @sqrt(pos_len);
        if (pos_len > 1e-10) {
            for (0..4) |i| self.pos[i] /= pos_len;
        }

        // Project out pos component from forward
        var dot_pf: f64 = 0;
        for (0..4) |i| dot_pf += self.pos[i] * self.forward[i];
        for (0..4) |i| self.forward[i] -= dot_pf * self.pos[i];

        // Normalize forward
        var fwd_len: f64 = 0;
        for (0..4) |i| fwd_len += self.forward[i] * self.forward[i];
        fwd_len = @sqrt(fwd_len);
        if (fwd_len > 1e-10) {
            for (0..4) |i| self.forward[i] /= fwd_len;
        }

        // right = forward × up (in 4D: project out pos and forward from up, then cross)
        // Simple approach: right = forward × up via Gram-Schmidt
        var dot_pu: f64 = 0;
        for (0..4) |i| dot_pu += self.pos[i] * self.up[i];
        for (0..4) |i| self.up[i] -= dot_pu * self.pos[i];
        var dot_fu: f64 = 0;
        for (0..4) |i| dot_fu += self.forward[i] * self.up[i];
        for (0..4) |i| self.up[i] -= dot_fu * self.forward[i];

        var up_len: f64 = 0;
        for (0..4) |i| up_len += self.up[i] * self.up[i];
        up_len = @sqrt(up_len);
        if (up_len > 1e-10) {
            for (0..4) |i| self.up[i] /= up_len;
        }

        // right = forward × up (3D part, w=0)
        self.right[0] = self.forward[1] * self.up[2] - self.forward[2] * self.up[1];
        self.right[1] = self.forward[2] * self.up[0] - self.forward[0] * self.up[2];
        self.right[2] = self.forward[0] * self.up[1] - self.forward[1] * self.up[0];
        self.right[3] = 0; // right is always tangent (w=0 for orbit)

        var right_len: f64 = 0;
        for (0..4) |i| right_len += self.right[i] * self.right[i];
        right_len = @sqrt(right_len);
        if (right_len > 1e-10) {
            for (0..4) |i| self.right[i] /= right_len;
        }
    }

    /// Получить view matrix как PGL4 (f64 column-major)
    pub fn viewPGL4(self: *const P3Camera) [16]f64 {
        // View matrix: [right | up | -forward | pos] as columns
        // This maps camera-local coords to world coords on S³
        var m = [_]f64{0} ** 16;
        // Column 0: right
        m[0] = self.right[0];
        m[1] = self.right[1];
        m[2] = self.right[2];
        m[3] = self.right[3];
        // Column 1: up
        m[4] = self.up[0];
        m[5] = self.up[1];
        m[6] = self.up[2];
        m[7] = self.up[3];
        // Column 2: -forward
        m[8] = -self.forward[0];
        m[9] = -self.forward[1];
        m[10] = -self.forward[2];
        m[11] = -self.forward[3];
        // Column 3: pos (translation)
        m[12] = self.pos[0];
        m[13] = self.pos[1];
        m[14] = self.pos[2];
        m[15] = self.pos[3];
        return m;
    }

    /// Получить MVP (f32 column-major) для GPU upload
    pub fn mvpGpu(self: *const P3Camera, aspect: f64) p3_gpu.GpuPGL4 {
        const view = self.viewPGL4();

        // Simple perspective projection on top of PGL4 view
        // Near = fs_near, Far = fs_far (in FS-distance)
        const near: f64 = self.fs_near;
        const far: f64 = self.fs_far;
        const fov: f64 = std.math.pi / 3.0; // 60°
        const f = 1.0 / @tan(fov / 2.0);

        var mvp = [_]f32{0} ** 16;
        // Column 0
        mvp[0] = @floatCast(f / aspect);
        // Column 1
        mvp[5] = @floatCast(f);
        // Column 2
        mvp[10] = @floatCast((far + near) / (near - far));
        mvp[11] = -1.0;
        // Column 3
        mvp[14] = @floatCast(2.0 * far * near / (near - far));

        // Multiply projection × view
        // For now, we compose view into MVP by multiplying
        var result = [_]f32{0} ** 16;
        for (0..4) |i| {
            for (0..4) |j| {
                var sum: f32 = 0;
                for (0..4) |k| {
                    const a = mvp[k * 4 + i];
                    const b: f32 = @floatCast(view[j * 4 + k]);
                    sum += a * b;
                }
                result[j * 4 + i] = sum;
            }
        }

        return .{ .data = result };
    }

    /// Reset to origin
    pub fn reset(self: *P3Camera) void {
        self.pos = .{ 0, 0, 0, 1 };
        self.forward = .{ 0, 0, -1, 0 };
        self.up = .{ 0, 1, 0, 0 };
        self.right = .{ 1, 0, 0, 0 };
    }
};

// =============================================================================
// 3. P³ APP — ПОЛНОЕ СОСТОЯНИЕ ПРИЛОЖЕНИЯ
// =============================================================================

pub const P3App = struct {
    allocator: std.mem.Allocator,
    config: AppConfig,
    window: *zglfw.Window,
    gpu_ctx: *p3_gpu_rt.P3GpuContext,
    input: p3_input.InputState,
    camera: P3Camera,
    demo_vertices: []p3_gpu_rt.P3Vertex,
    frame_count: u64 = 0,
    last_time: f64 = 0,
    fps: f64 = 0,
    running: bool = true,

    // =====================================================================
    // INIT — СОЗДАЁМ ВСЁ
    // =====================================================================

    pub fn init(allocator: std.mem.Allocator, config: AppConfig) !*P3App {
        // --- 1. GLFW ---
        try zglfw.init();
        errdefer zglfw.terminate();

        // No OpenGL context — WebGPU only
        zglfw.windowHint(.client_api, .no_api);
        zglfw.windowHint(.resizable, true);

        const window = try zglfw.createWindow(
            config.window_width,
            config.window_height,
            config.window_title,
            null,
        );
        errdefer window.destroy();

        // --- 2. GPU CONTEXT ---
        // WindowProvider bridges zglfw → zgpu
        // We need wrapper functions that match the WindowProvider signatures
        const gctx = try zgpu.GraphicsContext.create(
            allocator,
            .{
                .window = @ptrCast(window),
                .fn_getTime = @ptrCast(&zglfw.getTime),
                .fn_getFramebufferSize = &getFramebufferSizeForZgpu,
                .fn_getWin32Window = @ptrCast(&zglfw.getWin32Window),
                .fn_getX11Display = @ptrCast(&zglfw.getX11Display),
                .fn_getX11Window = @ptrCast(&zglfw.getX11Window),
                .fn_getWaylandDisplay = @ptrCast(&zglfw.getWaylandDisplay),
                .fn_getWaylandSurface = @ptrCast(&zglfw.getWaylandWindow),
                .fn_getCocoaWindow = @ptrCast(&zglfw.getCocoaWindow),
            },
            .{},
        );
        errdefer gctx.destroy(allocator);

        // --- 3. P³ GPU PIPELINES ---
        const gpu_ctx = try p3_gpu_rt.P3GpuContext.init(
            allocator,
            gctx,
            config.max_points,
            config.max_transforms,
        );
        errdefer gpu_ctx.deinit(allocator);

        // --- 4. DEMO VERTICES ---
        const demo_vertices = try p3_gpu_rt.generateS3Points(
            allocator,
            1000, // 1000 points on S³
        );
        errdefer allocator.free(demo_vertices);

        // Upload demo vertices to GPU
        // Convert P3Vertex[] to GpuHomVec4[] for positions buffer
        var gpu_positions = try allocator.alloc(p3_gpu.GpuHomVec4, demo_vertices.len);
        defer allocator.free(gpu_positions);
        for (demo_vertices, 0..) |v, i| {
            gpu_positions[i] = .{ .x = v.px, .y = v.py, .z = v.pz, .w = v.pw };
        }

        // --- 5. APP STATE ---
        const app = try allocator.create(P3App);
        app.* = .{
            .allocator = allocator,
            .config = config,
            .window = window,
            .gpu_ctx = gpu_ctx,
            .input = .{},
            .camera = .{},
            .demo_vertices = demo_vertices,
            .last_time = zglfw.getTime(),
        };

        // Upload initial data
        app.gpu_ctx.uploadPositions(gpu_positions);
        const ref_point: p3_gpu.GpuHomVec4 = .{ .x = 0, .y = 0, .z = 0, .w = 1 }; // origin
        app.gpu_ctx.uploadReference(&ref_point);

        return app;
    }

    // =====================================================================
    // DEINIT — ОСВОБОЖДАЕМ ВСЁ
    // =====================================================================

    pub fn deinit(self: *P3App) void {
        self.input.deinit(self.allocator);
        self.allocator.free(self.demo_vertices);
        self.gpu_ctx.deinit(self.allocator);
        self.window.destroy();
        zglfw.terminate();
        self.allocator.destroy(self);
    }

    // =====================================================================
    // UPDATE — ЛОГИКА КАДРА
    // =====================================================================

    pub fn update(self: *P3App) void {
        const now = zglfw.getTime();
        const dt = now - self.last_time;
        self.last_time = now;

        // FPS counter
        if (dt > 0) {
            self.fps = 0.95 * self.fps + 0.05 * (1.0 / dt); // EMA
        }

        // --- Input ---
        self.input.update(self.window);

        // --- Camera ---
        self.camera.update(&self.input, dt);
        if (self.input.reset_camera) self.camera.reset();

        // --- Upload MVP ---
        const fb_size = self.window.getFramebufferSize();
        const fb_width = fb_size[0];
        const fb_height = fb_size[1];
        const aspect: f64 = if (fb_height > 0) @as(f64, @floatFromInt(fb_width)) / @as(f64, @floatFromInt(fb_height)) else 1.0;
        const mvp = self.camera.mvpGpu(aspect);
        self.gpu_ctx.uploadMVP(&mvp);

        // --- Dispatch compute shaders (demo: FS-distance) ---
        // Each frame, compute FS-distance from camera to all points
        const ref_as_gpu = p3_gpu.GpuHomVec4{
            .x = @floatCast(self.camera.pos[0]),
            .y = @floatCast(self.camera.pos[1]),
            .z = @floatCast(self.camera.pos[2]),
            .w = @floatCast(self.camera.pos[3]),
        };
        self.gpu_ctx.uploadReference(&ref_as_gpu);
        self.gpu_ctx.dispatchFSDistance(@intCast(self.demo_vertices.len));

        self.frame_count += 1;
    }

    // =====================================================================
    // RENDER — ОТРИСОВКА КАДРА
    // =====================================================================

    pub fn render(self: *P3App) void {
        // Render demo vertices
        self.gpu_ctx.renderFrame(
            @intCast(self.demo_vertices.len),
            1, // 1 instance
        );

        // Present
        if (self.gpu_ctx.present() == .swap_chain_resized) {
            self.gpu_ctx.recreateDepthTexture();
        }
    }

    // =====================================================================
    // RUN — ГЛАВНЫЙ ЦИКЛ
    // =====================================================================

    pub fn run(self: *P3App) void {
        while (self.running and !self.input.should_close and !self.window.shouldClose()) {
            self.update();
            self.render();
        }
    }
};

// =============================================================================
// 4. pub fn main — ТОЧКА ВХОДА
// =============================================================================
//
// P³ Engine — запускаемый бинарник. Больше НЕ библиотека.

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const config = AppConfig{
        .window_width = 1280,
        .window_height = 720,
        .window_title = "P³ Engine — Projective Geometry on S³",
        .max_points = 100_000,
        .max_transforms = 10_000,
    };

    const app = try P3App.init(allocator, config);
    defer app.deinit();

    app.run();
}

// =============================================================================
// 5. ТЕСТЫ (БЕЗ GPU — ЛОГИКА КАМЕРЫ И КОНФИГУРАЦИИ)
// =============================================================================

test "App: P3Camera initial position" {
    const cam = P3Camera{};
    try std.testing.expectApproxEqAbs(cam.pos[3], 1.0, 1e-10); // w = 1
    try std.testing.expectApproxEqAbs(cam.pos[0], 0.0, 1e-10); // x = 0
}

test "App: P3Camera position on S³" {
    const cam = P3Camera{};
    const norm_sq = cam.pos[0] * cam.pos[0] + cam.pos[1] * cam.pos[1] + cam.pos[2] * cam.pos[2] + cam.pos[3] * cam.pos[3];
    try std.testing.expectApproxEqAbs(norm_sq, 1.0, 1e-10);
}

test "App: P3Camera forward perpendicular to position" {
    const cam = P3Camera{};
    var dot: f64 = 0;
    for (0..4) |i| dot += cam.pos[i] * cam.forward[i];
    try std.testing.expectApproxEqAbs(dot, 0.0, 1e-10);
}

test "App: P3Camera reset" {
    var cam = P3Camera{};
    cam.pos = .{ 1, 0, 0, 0 };
    cam.forward = .{ 0, 1, 0, 0 };
    cam.reset();
    try std.testing.expectApproxEqAbs(cam.pos[3], 1.0, 1e-10);
    try std.testing.expectApproxEqAbs(cam.forward[2], -1.0, 1e-10);
}

test "App: P3Camera viewPGL4 is 16 elements" {
    const cam = P3Camera{};
    const view = cam.viewPGL4();
    try std.testing.expect(view.len == 16);
}

test "App: P3Camera reorthonormalize preserves S³" {
    var cam = P3Camera{};
    // Perturb slightly
    cam.pos[0] = 0.01;
    cam.reorthonormalize();
    const norm_sq = cam.pos[0] * cam.pos[0] + cam.pos[1] * cam.pos[1] + cam.pos[2] * cam.pos[2] + cam.pos[3] * cam.pos[3];
    try std.testing.expectApproxEqAbs(norm_sq, 1.0, 1e-10);
}

test "App: P3Camera forward stays perpendicular after update" {
    var cam = P3Camera{};
    var input = p3_input.InputState{};
    input.camera_move_forward = true;

    // Simulate 100 frames
    for (0..100) |_| {
        cam.update(&input, 0.016); // ~60fps
    }

    var dot: f64 = 0;
    for (0..4) |i| dot += cam.pos[i] * cam.forward[i];
    try std.testing.expectApproxEqAbs(dot, 0.0, 1e-6); // Should stay perpendicular
}

test "App: AppConfig defaults" {
    const config = AppConfig{};
    try std.testing.expect(config.window_width == 1280);
    try std.testing.expect(config.window_height == 720);
    try std.testing.expect(config.max_points == 100_000);
    try std.testing.expect(config.vsync == true);
}

test "App: P3Camera update with zero input" {
    var cam = P3Camera{};
    const input = p3_input.InputState{};
    cam.update(&input, 0.016);
    // Should stay at origin
    try std.testing.expectApproxEqAbs(cam.pos[3], 1.0, 1e-6);
}

test "App: mvpGpu produces valid PGL4" {
    const cam = P3Camera{};
    const mvp = cam.mvpGpu(16.0 / 9.0);
    // Should have non-zero elements
    var has_nonzero = false;
    for (0..16) |i| {
        if (@abs(mvp.data[i]) > 1e-10) has_nonzero = true;
    }
    try std.testing.expect(has_nonzero);
}
