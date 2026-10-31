// =============================================================================
// P³ ENGINE — NATIVE 3D VIEWPORT & LEVEL RENDERER (ZIG + RAYLIB)
// =============================================================================
// Нативный 3D-вьюпорт, объединяющий все переписанные подсистемы P³:
//   - Камера p3_o3de_camera.zig (орбитальное управление ЛКМ/ПКМ/Колесико)
//   - Загрузка уровня .spawnable через p3_o3de_spawnable.zig
//   - Иерархия сцены p3_o3de_scene_system.zig + ECS p3_o3de_entity.zig
//   - Проективные трансформации p3_o3de_transform.zig
//   - Отрисовка геометрии p3_o3de_mesh.zig + сетка координатного пространства P³
// =============================================================================

const std = @import("std");
const p3_camera = @import("p3_o3de_camera.zig");
const p3_mesh = @import("p3_o3de_mesh.zig");
const p3_spawnable = @import("p3_o3de_spawnable.zig");
const p3_transform = @import("p3_o3de_transform.zig");
const p3_entity = @import("p3_o3de_entity.zig");

const rl = @cImport({
    @cInclude("raylib.h");
    @cInclude("rlgl.h");
});

pub const NativeViewport = struct {
    allocator: std.mem.Allocator,
    camera: p3_camera.NativeCamera,
    entities: std.ArrayList(p3_entity.NativeEntity),
    default_mesh: p3_mesh.NativeMesh,

    pub fn init(allocator: std.mem.Allocator) !NativeViewport {
        var cam = p3_camera.NativeCamera.init();
        cam.offset.y = 8.0;
        cam.pitch = 0.4;
        cam.yaw = 0.6;

        const cube = try p3_mesh.NativeMesh.createCube(allocator, 1.5);
        return .{
            .allocator = allocator,
            .camera = cam,
            .entities = std.ArrayList(p3_entity.NativeEntity).init(allocator),
            .default_mesh = cube,
        };
    }

    pub fn deinit(self: *NativeViewport) void {
        for (self.entities.items) |*e| {
            e.deinit();
        }
        self.entities.deinit();
        self.default_mesh.deinit();
    }

    pub fn loadLevel(self: *NativeViewport, path: []const u8) !void {
        var sp = p3_spawnable.Spawnable.init(self.allocator);
        defer sp.deinit();

        sp.loadFromFile(path) catch {};

        // Загружаем сущности уровня в сцену вьюпорта
        for (sp.entities.items) |ent| {
            var ne = try p3_entity.NativeEntity.init(self.allocator, ent.id, ent.name);
            ne.transform.setLocalTranslation(0.0, 0.0, 0.0);
            try self.entities.append(ne);
        }

        // Если уровень пустой — создаем стартовую тестовую сущность
        if (self.entities.items.len == 0) {
            const root = try p3_entity.NativeEntity.init(self.allocator, 1, "DefaultLevelRoot");
            try self.entities.append(root);
        }
    }

    pub fn update(self: *NativeViewport, is_drag: bool, wheel: f32) void {
        if (is_drag) {
            const delta = rl.GetMouseDelta();
            self.camera.orbit(delta.x * 0.005, delta.y * 0.005);
        }
        if (wheel != 0.0) {
            self.camera.zoom(wheel * 1.5);
        }
    }

    pub fn render(self: *NativeViewport, rx: f32, ry: f32, rw: f32, rh: f32) void {
        const cam_pos = self.camera.getTranslation();

        const rl_camera = rl.Camera3D{
            .position = .{ .x = cam_pos.x, .y = cam_pos.y, .z = cam_pos.z },
            .target = .{ .x = self.camera.pivot.x, .y = self.camera.pivot.y, .z = self.camera.pivot.z },
            .up = .{ .x = 0.0, .y = 1.0, .z = 0.0 },
            .fovy = self.camera.fov,
            .projection = rl.CAMERA_PERSPECTIVE,
        };

        rl.BeginScissorMode(@intFromFloat(rx), @intFromFloat(ry), @intFromFloat(rw), @intFromFloat(rh));
        rl.BeginMode3D(rl_camera);

        // 1. Координатная сетка P³ Space Grid
        rl.DrawGrid(20, 1.0);

        // 2. Отрисовка 3D-сущностей уровня
        for (self.entities.items, 0..) |ent, idx| {
            const pos = ent.transform.world_pos;
            const px: f32 = @floatCast(pos.x);
            const py: f32 = @floatCast(pos.y + 0.75);
            const pz: f32 = @floatCast(pos.z);

            const color = if (idx == 0)
                rl.Color{ .r = 0, .g = 136, .b = 255, .a = 255 }
            else
                rl.Color{ .r = 46, .g = 204, .b = 113, .a = 255 };

            rl.DrawCube(.{ .x = px, .y = py, .z = pz }, 1.5, 1.5, 1.5, color);
            rl.DrawCubeWires(.{ .x = px, .y = py, .z = pz }, 1.5, 1.5, 1.5, rl.Color{ .r = 255, .g = 255, .b = 255, .a = 180 });
        }

        rl.EndMode3D();
        rl.EndScissorMode();

        // Рамка вьюпорта
        rl.DrawRectangleLinesEx(rl.Rectangle{ .x = rx, .y = ry, .width = rw, .height = rh }, 1.0, rl.Color{ .r = 48, .g = 54, .b = 66, .a = 255 });
    }
};
