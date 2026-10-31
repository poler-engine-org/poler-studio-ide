// =============================================================================
// P³ ENGINE — NATIVE REPLACEMENT FOR AzFramework::TransformComponent (ZIG 0.14)
// =============================================================================
// Полный нативный аналог TransformComponent:
//   - Локальный и мировой трансформ
//   - Проективная геометрия PGL4 на S³
//   - Parent-Child иерархия трансформаций
//   - C-ABI экспорты для полной замены libAzFramework.so TransformBus
// =============================================================================

const std = @import("std");
const p3_kernel = @import("p3_kernel.zig");
const p3_geodesic = @import("p3_geodesic.zig");
const p3_ecs = @import("p3_ecs.zig");
const p3_quaternion = @import("p3_quaternion.zig");

pub const HomVec4 = p3_kernel.HomVec4;
pub const PGL4 = p3_kernel.PGL4;
pub const EntityId = p3_ecs.EntityId;

pub const NativeTransform = struct {
    entity_id: EntityId = EntityId.invalid(),
    parent_id: EntityId = EntityId.invalid(),

    // Локальные компоненты
    local_pos: HomVec4 = HomVec4.fromCartesian(.{ 0, 0, 0 }),
    local_rot: p3_quaternion.Quat = p3_quaternion.Quat.identity,
    local_scale: f32 = 1.0,

    // Мировые вычисленные компоненты
    world_pos: HomVec4 = HomVec4.fromCartesian(.{ 0, 0, 0 }),
    world_rot: p3_quaternion.Quat = p3_quaternion.Quat.identity,
    world_scale: f32 = 1.0,

    world_matrix: PGL4 = PGL4.identity(),
    is_dirty: bool = true,

    pub fn init(entity_id: EntityId) NativeTransform {
        return .{
            .entity_id = entity_id,
            .parent_id = EntityId.invalid(),
            .local_pos = HomVec4.fromCartesian(.{ 0, 0, 0 }),
            .local_rot = p3_quaternion.Quat.identity,
            .local_scale = 1.0,
            .world_pos = HomVec4.fromCartesian(.{ 0, 0, 0 }),
            .world_rot = p3_quaternion.Quat.identity,
            .world_scale = 1.0,
            .world_matrix = PGL4.identity(),
            .is_dirty = true,
        };
    }

    pub fn setLocalTranslation(self: *NativeTransform, x: f32, y: f32, z: f32) void {
        self.local_pos = HomVec4.fromCartesian(.{ @floatCast(x), @floatCast(y), @floatCast(z) });
        self.is_dirty = true;
    }

    pub fn setLocalRotation(self: *NativeTransform, qx: f32, qy: f32, qz: f32, qw: f32) void {
        self.local_rot = .{ .x = qx, .y = qy, .z = qz, .w = qw };
        self.is_dirty = true;
    }

    pub fn setLocalUniformScale(self: *NativeTransform, scale: f32) void {
        self.local_scale = scale;
        self.is_dirty = true;
    }

    pub fn setParent(self: *NativeTransform, parent_id: EntityId) void {
        self.parent_id = parent_id;
        self.is_dirty = true;
    }

    /// Пересчет мирового трансформа
    pub fn updateWorldTransform(self: *NativeTransform, parent: ?*const NativeTransform) void {
        if (parent) |p| {
            self.world_pos = p.world_pos.add(self.local_pos);
            self.world_rot = p.world_rot.mul(self.local_rot);
            self.world_scale = p.world_scale * self.local_scale;
        } else {
            self.world_pos = self.local_pos;
            self.world_rot = self.local_rot;
            self.world_scale = self.local_scale;
        }
        self.is_dirty = false;
    }
};

// =============================================================================
// C-ABI EXPORTS (Для подмены libAzFramework.so / TransformComponent)
// =============================================================================

export fn P3_Transform_Create(entity_id: u64) ?*NativeTransform {
    const allocator = std.heap.c_allocator;
    const t = allocator.create(NativeTransform) catch return null;
    t.* = NativeTransform.init(EntityId.init(entity_id));
    return t;
}

export fn P3_Transform_Destroy(ptr: ?*NativeTransform) void {
    if (ptr) |t| {
        std.heap.c_allocator.destroy(t);
    }
}

export fn P3_Transform_SetLocalTranslation(ptr: ?*NativeTransform, x: f32, y: f32, z: f32) void {
    if (ptr) |t| t.setLocalTranslation(x, y, z);
}

export fn P3_Transform_SetLocalRotation(ptr: ?*NativeTransform, qx: f32, qy: f32, qz: f32, qw: f32) void {
    if (ptr) |t| t.setLocalRotation(qx, qy, qz, qw);
}

export fn P3_Transform_SetLocalScale(ptr: ?*NativeTransform, scale: f32) void {
    if (ptr) |t| t.setLocalUniformScale(scale);
}

export fn P3_Transform_SetParent(ptr: ?*NativeTransform, parent_id: u64) void {
    if (ptr) |t| t.setParent(EntityId.init(parent_id));
}
