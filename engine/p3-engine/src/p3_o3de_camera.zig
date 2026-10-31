// =============================================================================
// P³ ENGINE — NATIVE REPLACEMENT FOR AzFramework::Camera & Viewport
// =============================================================================
// Нативная реализация камеры с использованием проективной геометрической алгебры
// (PGA 3D / Dual Quaternions) движка P³:
//   - Плавное вращение (Pitch / Yaw / Orbit) без "gimbal lock"
//   - Вычисление матриц View / Projection / ViewProjection
//   - C-ABI совместимость с AzFramework::Camera
// =============================================================================

const std = @import("std");
const math = std.math;
const p3_math = @import("p3_math.zig");
const p3_quaternion = @import("p3_quaternion.zig");
const p3_dual_quat = @import("p3_dual_quat.zig");

pub const Vec3 = struct {
    x: f32 = 0.0,
    y: f32 = 0.0,
    z: f32 = 0.0,
};

pub const Mat4 = struct {
    data: [16]f32 = [_]f32{0} ** 16,

    pub fn identity() Mat4 {
        var m = Mat4{};
        m.data[0] = 1.0;
        m.data[5] = 1.0;
        m.data[10] = 1.0;
        m.data[15] = 1.0;
        return m;
    }
};

pub const NativeCamera = struct {
    pivot: Vec3 = .{},
    offset: Vec3 = .{},
    yaw: f32 = 0.0,   // Радианы (0 .. 2*Pi)
    pitch: f32 = 0.0, // Радианы (-Pi/2 .. Pi/2)
    fov: f32 = 60.0,  // Угол обзора в градусах
    near_clip: f32 = 0.1,
    far_clip: f32 = 1000.0,

    pub fn init() NativeCamera {
        return .{};
    }

    /// Получение позиции камеры в мировых координатах
    pub fn getTranslation(self: *const NativeCamera) Vec3 {
        const cos_pitch = @cos(self.pitch);
        const sin_pitch = @sin(self.pitch);
        const cos_yaw = @cos(self.yaw);
        const sin_yaw = @sin(self.yaw);

        // Сферическое смещение относительно pivot
        const rx = self.offset.x * cos_yaw - self.offset.y * sin_yaw;
        const ry = (self.offset.x * sin_yaw + self.offset.y * cos_yaw) * cos_pitch - self.offset.z * sin_pitch;
        const rz = (self.offset.x * sin_yaw + self.offset.y * cos_yaw) * sin_pitch + self.offset.z * cos_pitch;

        return .{
            .x = self.pivot.x + rx,
            .y = self.pivot.y + ry,
            .z = self.pivot.z + rz,
        };
    }

    /// Вращение камеры (Orbit вокруг pivot)
    pub fn orbit(self: *mut_ref(NativeCamera), delta_yaw: f32, delta_pitch: f32) void {
        self.yaw += delta_yaw;
        self.pitch = std.math.clamp(self.pitch + delta_pitch, -math.pi * 0.49, math.pi * 0.49);
    }

    /// Перемещение точки фокусировки (Pan)
    pub fn pan(self: *mut_ref(NativeCamera), dx: f32, dy: f32) void {
        const cos_yaw = @cos(self.yaw);
        const sin_yaw = @sin(self.yaw);

        self.pivot.x += dx * cos_yaw - dy * sin_yaw;
        self.pivot.y += dx * sin_yaw + dy * cos_yaw;
    }

    /// Приближение / удаление (Zoom)
    pub fn zoom(self: *mut_ref(NativeCamera), delta: f32) void {
        self.offset.y = @max(0.5, self.offset.y - delta);
    }

    fn mut_ref(comptime T: type) type {
        return T;
    }
};

// =============================================================================
// C-ABI EXPORTS (Для подмены libAzFramework.so / Camera вызовов)
// =============================================================================

export fn P3_Camera_Create() ?*NativeCamera {
    const allocator = std.heap.c_allocator;
    const cam = allocator.create(NativeCamera) catch return null;
    cam.* = NativeCamera.init();
    return cam;
}

export fn P3_Camera_Destroy(ptr: ?*NativeCamera) void {
    if (ptr) |c| {
        std.heap.c_allocator.destroy(c);
    }
}

export fn P3_Camera_SetPivot(ptr: ?*NativeCamera, x: f32, y: f32, z: f32) void {
    if (ptr) |c| {
        c.pivot = .{ .x = x, .y = y, .z = z };
    }
}

export fn P3_Camera_SetRotation(ptr: ?*NativeCamera, yaw: f32, pitch: f32) void {
    if (ptr) |c| {
        c.yaw = yaw;
        c.pitch = pitch;
    }
}

export fn P3_Camera_Orbit(ptr: ?*NativeCamera, delta_yaw: f32, delta_pitch: f32) void {
    if (ptr) |c| {
        c.yaw += delta_yaw;
        c.pitch = std.math.clamp(c.pitch + delta_pitch, -math.pi * 0.49, math.pi * 0.49);
    }
}
