// =============================================================================
// P³ ENGINE — NATIVE REPLACEMENT FOR AzFramework::Scene & SceneSystem
// =============================================================================
// Нативная реализация ISceneSystem и Scene O3DE (UUID {DB449BB3-...}):
//   - Реестр сцен (Main, EditorMain, Subscenes)
//   - Иерархия сцен (Parent/Child)
//   - Связь с проективным графом P³ SceneGraph (p3_scene.zig) и P³ ECS (p3_ecs.zig)
//   - C-ABI совместимость для прямой замены libAzFramework.so
// =============================================================================

const std = @import("std");
const p3_scene = @import("p3_scene.zig");
const p3_ecs = @import("p3_ecs.zig");
const p3_o3de_spawnable = @import("p3_o3de_spawnable.zig");

pub const SceneId = u64;

pub const NativeScene = struct {
    allocator: std.mem.Allocator,
    id: SceneId,
    name: []const u8,
    is_alive: bool = true,
    world: p3_ecs.World,
    scene_graph: p3_scene.SceneGraph,
    parent: ?*NativeScene = null,

    pub fn init(allocator: std.mem.Allocator, id: SceneId, name: []const u8, parent: ?*NativeScene) !NativeScene {
        const owned_name = try allocator.dupe(u8, name);
        return .{
            .allocator = allocator,
            .id = id,
            .name = owned_name,
            .is_alive = true,
            .world = p3_ecs.World.init(allocator),
            .scene_graph = p3_scene.SceneGraph.init(allocator),
            .parent = parent,
        };
    }

    pub fn deinit(self: *NativeScene) void {
        self.allocator.free(self.name);
        self.world.deinit();
        self.scene_graph.deinit();
    }

    /// Загрузка уровня O3DE (.spawnable) прямо в сцену P³
    pub fn loadSpawnable(self: *NativeScene, path: []const u8) !void {
        var spawnable = p3_o3de_spawnable.Spawnable.init(self.allocator);
        defer spawnable.deinit();

        try spawnable.loadFromFile(path);
        try spawnable.spawnIntoP3World(&self.world);
    }
};

pub const NativeSceneSystem = struct {
    allocator: std.mem.Allocator,
    scenes: std.StringHashMap(*NativeScene),
    next_id: SceneId = 1,

    pub fn init(allocator: std.mem.Allocator) NativeSceneSystem {
        return .{
            .allocator = allocator,
            .scenes = std.StringHashMap(*NativeScene).init(allocator),
            .next_id = 1,
        };
    }

    pub fn deinit(self: *NativeSceneSystem) void {
        var it = self.scenes.valueIterator();
        while (it.next()) |scene_ptr| {
            scene_ptr.*.deinit();
            self.allocator.destroy(scene_ptr.*);
        }
        self.scenes.deinit();
    }

    pub fn createScene(self: *NativeSceneSystem, name: []const u8, parent_name: ?[]const u8) !*NativeScene {
        if (self.scenes.contains(name)) {
            return self.scenes.get(name).?;
        }

        var parent_ptr: ?*NativeScene = null;
        if (parent_name) |pname| {
            parent_ptr = self.scenes.get(pname);
        }

        const scene_ptr = try self.allocator.create(NativeScene);
        scene_ptr.* = try NativeScene.init(self.allocator, self.next_id, name, parent_ptr);
        self.next_id += 1;

        try self.scenes.put(scene_ptr.name, scene_ptr);
        return scene_ptr;
    }

    pub fn getScene(self: *const NativeSceneSystem, name: []const u8) ?*NativeScene {
        return self.scenes.get(name);
    }
};

// =============================================================================
// C-ABI EXPORTS (Для подмены libAzFramework.so / ISceneSystem)
// =============================================================================

export fn P3_SceneSystem_Create() ?*NativeSceneSystem {
    const allocator = std.heap.c_allocator;
    const sys = allocator.create(NativeSceneSystem) catch return null;
    sys.* = NativeSceneSystem.init(allocator);
    return sys;
}

export fn P3_SceneSystem_Destroy(ptr: ?*NativeSceneSystem) void {
    if (ptr) |s| {
        const allocator = s.allocator;
        s.deinit();
        allocator.destroy(s);
    }
}

export fn P3_SceneSystem_CreateScene(ptr: ?*NativeSceneSystem, name_ptr: [*:0]const u8) ?*NativeScene {
    if (ptr) |s| {
        const name = std.mem.span(name_ptr);
        return s.createScene(name, null) catch return null;
    }
    return null;
}

export fn P3_SceneSystem_GetScene(ptr: ?*const NativeSceneSystem, name_ptr: [*:0]const u8) ?*NativeScene {
    if (ptr) |s| {
        const name = std.mem.span(name_ptr);
        return s.getScene(name);
    }
    return null;
}
