// =============================================================================
// P³ ENGINE — O3DE NATIVE COMPATIBILITY SHARED LIBRARY (libP3_O3DE_Bridge.so)
// =============================================================================

const std = @import("std");
pub const spawnable = @import("p3_o3de_spawnable.zig");
pub const asset_sys = @import("p3_o3de_asset_system.zig");
pub const serialize = @import("p3_o3de_serialize.zig");
pub const cursor_fix = @import("p3_o3de_cursor_fix.zig");
pub const launcher_native = @import("p3_o3de_launcher_native.zig");
pub const camera = @import("p3_o3de_camera.zig");
pub const scene_sys = @import("p3_o3de_scene_system.zig");
pub const cursor_system = @import("p3_o3de_cursor_system.zig");
pub const transform = @import("p3_o3de_transform.zig");
pub const entity = @import("p3_o3de_entity.zig");
pub const mesh = @import("p3_o3de_mesh.zig");

// Принудительное включение экспортов зависимых модулей
comptime {
    _ = spawnable;
    _ = asset_sys;
    _ = serialize;
    _ = cursor_fix;
    _ = launcher_native;
    _ = camera;
    _ = scene_sys;
    _ = cursor_system;
    _ = transform;
    _ = entity;
    _ = mesh;
}

export fn P3_GetBridgeVersion() [*:0]const u8 {
    return "P3_O3DE_Bridge_v1.0.0";
}
