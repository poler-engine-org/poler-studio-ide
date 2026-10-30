exe.addModule("p3_core", .{ .path = "src/core/p3_homogeneous.zig" });
exe.addModule("p3_ecs", .{ .path = "src/ecs/p3_world.zig" });
exe.addModule("p3_scene", .{ .path = "src/scene/p3_node.zig" });
exe.addModule("p3_physics", .{ .path = "src/physics/p3_rigid_body.zig" });
