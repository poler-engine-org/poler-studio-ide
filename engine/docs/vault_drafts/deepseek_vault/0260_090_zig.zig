// Пример build.zig для новой структуры
const std = @import("std");

pub fn build(b: *std.Build) void {
const target = b.standardTargetOptions(.{});
const optimize = b.standardOptimizeOption(.{});

const exe = b.addExecutable(.{
.name = "p3_engine",
.root_source_file = .{ .path = "src/main.zig" },
.target = target,
.optimize = optimize,
});

// Добавляем все модули
exe.addModule("p3_core", .{ .path = "src/core/p3_homogeneous.zig" });
exe.addModule("p3_pgl4", .{ .path = "src/core/p3_pgl4.zig" });
exe.addModule("p3_fs", .{ .path = "src/core/p3_fubini_study.zig" });
exe.addModule("p3_resonance", .{ .path = "src/core/p3_resonance.zig" });
exe.addModule("p3_poler", .{ .path = "src/core/p3_poler.zig" });
exe.addModule("p3_ecs", .{ .path = "src/ecs/p3_world.zig" });
exe.addModule("p3_scene", .{ .path = "src/scene/p3_node.zig" });
exe.addModule("p3_physics", .{ .path = "src/physics/p3_rigid_body.zig" });
exe.addModule("p3_render", .{ .path = "src/render/p3_camera.zig" });

b.installArtifact(exe);
}
