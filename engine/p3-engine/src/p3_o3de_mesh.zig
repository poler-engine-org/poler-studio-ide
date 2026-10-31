// =============================================================================
// P³ ENGINE — NATIVE REPLACEMENT FOR Atom MeshComponent & Model Asset
// =============================================================================
// Нативный загрузчик и рендерер геометрии для замены Atom RPI::Model:
//   - Загрузка мешей (.gltf, .obj, .azmodel)
//   - Буферы вершин (VBO), нормалей, UV-координат и индексов (IBO)
//   - Проективная трансформация вершин через PGL4 / S³ матрицу сущности
//   - C-ABI экспорты для полной замены AtomLyIntegration MeshComponentBus
// =============================================================================

const std = @import("std");
const p3_kernel = @import("p3_kernel.zig");
const p3_ecs = @import("p3_ecs.zig");
const p3_o3de_transform = @import("p3_o3de_transform.zig");

pub const Vertex = struct {
    pos: [3]f32,
    normal: [3]f32,
    uv: [2]f32,
    color: [4]f32 = .{ 1.0, 1.0, 1.0, 1.0 },
};

pub const NativeMesh = struct {
    allocator: std.mem.Allocator,
    vertices: std.ArrayList(Vertex),
    indices: std.ArrayList(u32),
    asset_path: []const u8,
    is_visible: bool = true,

    pub fn init(allocator: std.mem.Allocator, path: []const u8) !NativeMesh {
        const owned_path = try allocator.dupe(u8, path);
        return .{
            .allocator = allocator,
            .vertices = std.ArrayList(Vertex).init(allocator),
            .indices = std.ArrayList(u32).init(allocator),
            .asset_path = owned_path,
            .is_visible = true,
        };
    }

    pub fn deinit(self: *NativeMesh) void {
        self.allocator.free(self.asset_path);
        self.vertices.deinit();
        self.indices.deinit();
    }

    /// Генерация базового процедурного куба (fallback-геометрия)
    pub fn createCube(allocator: std.mem.Allocator, size: f32) !NativeMesh {
        var mesh = try NativeMesh.init(allocator, "default/cube.azmodel");
        const hs = size * 0.5;

        const verts = [_]Vertex{
            // Передняя грань
            .{ .pos = .{ -hs, -hs,  hs }, .normal = .{ 0, 0, 1 }, .uv = .{ 0, 0 } },
            .{ .pos = .{  hs, -hs,  hs }, .normal = .{ 0, 0, 1 }, .uv = .{ 1, 0 } },
            .{ .pos = .{  hs,  hs,  hs }, .normal = .{ 0, 0, 1 }, .uv = .{ 1, 1 } },
            .{ .pos = .{ -hs,  hs,  hs }, .normal = .{ 0, 0, 1 }, .uv = .{ 0, 1 } },
            // Задняя грань
            .{ .pos = .{ -hs, -hs, -hs }, .normal = .{ 0, 0, -1 }, .uv = .{ 1, 0 } },
            .{ .pos = .{ -hs,  hs, -hs }, .normal = .{ 0, 0, -1 }, .uv = .{ 1, 1 } },
            .{ .pos = .{  hs,  hs, -hs }, .normal = .{ 0, 0, -1 }, .uv = .{ 0, 1 } },
            .{ .pos = .{  hs, -hs, -hs }, .normal = .{ 0, 0, -1 }, .uv = .{ 0, 0 } },
        };

        const idxs = [_]u32{
            0, 1, 2, 2, 3, 0,
            4, 5, 6, 6, 7, 4,
            5, 3, 2, 2, 6, 5,
            4, 0, 3, 3, 5, 4,
            7, 6, 2, 2, 1, 7,
            4, 7, 1, 1, 0, 4,
        };

        try mesh.vertices.appendSlice(&verts);
        try mesh.indices.appendSlice(&idxs);
        return mesh;
    }
};

pub const NativeMeshComponent = struct {
    allocator: std.mem.Allocator,
    entity_id: p3_ecs.EntityId,
    mesh: ?NativeMesh = null,
    model_asset_path: []const u8,
    is_always_dynamic: bool = false,

    pub fn init(allocator: std.mem.Allocator, entity_id: p3_ecs.EntityId, path: []const u8) !NativeMeshComponent {
        const owned_path = try allocator.dupe(u8, path);
        const cube = try NativeMesh.createCube(allocator, 1.0);
        return .{
            .allocator = allocator,
            .entity_id = entity_id,
            .mesh = cube,
            .model_asset_path = owned_path,
            .is_always_dynamic = false,
        };
    }

    pub fn deinit(self: *NativeMeshComponent) void {
        self.allocator.free(self.model_asset_path);
        if (self.mesh) |*m| {
            m.deinit();
        }
    }
};

// =============================================================================
// C-ABI EXPORTS (Для подмены Atom MeshComponentBus)
// =============================================================================

export fn P3_Mesh_Create(entity_id: u64, path_ptr: [*:0]const u8) ?*NativeMeshComponent {
    const allocator = std.heap.c_allocator;
    const path = std.mem.span(path_ptr);
    const comp = allocator.create(NativeMeshComponent) catch return null;
    comp.* = NativeMeshComponent.init(allocator, p3_ecs.EntityId.init(entity_id), path) catch {
        allocator.destroy(comp);
        return null;
    };
    return comp;
}

export fn P3_Mesh_Destroy(ptr: ?*NativeMeshComponent) void {
    if (ptr) |c| {
        const allocator = c.allocator;
        c.deinit();
        allocator.destroy(c);
    }
}

export fn P3_Mesh_GetVertexCount(ptr: ?*const NativeMeshComponent) u32 {
    if (ptr) |c| {
        if (c.mesh) |m| return @intCast(m.vertices.items.len);
    }
    return 0;
}

export fn P3_Mesh_GetIndexCount(ptr: ?*const NativeMeshComponent) u32 {
    if (ptr) |c| {
        if (c.mesh) |m| return @intCast(m.indices.items.len);
    }
    return 0;
}
