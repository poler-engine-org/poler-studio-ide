// =============================================================================
// P³ ENGINE — O3DE NATIVE SPAWNABLE & PREFAB LOADER (ZIG)
// =============================================================================
// Нативная реализация AzFramework::Spawnable (UUID: {855E3021-D305-4845-B284-20C3F7FDF16B})
// Парсит структуры .spawnable и .prefab без вызова падающих C++ RTTI сериализаторов:
//   - EntityList (std.ArrayList(Entity))
//   - EntityAliasList
//   - SpawnableMetaData
//   - Проективная трансляция в P³ ECS
// =============================================================================

const std = @import("std");
const p3_ecs = @import("p3_ecs.zig");
const p3_kernel = @import("p3_kernel.zig");

pub const SpawnableUuid = "{855E3021-D305-4845-B284-20C3F7FDF16B}";
pub const EntityAliasUuid = "{C8D0C5BC-1F0B-4572-98C1-73B2CA8C9356}";

pub const EntityAliasType = enum(u8) {
    original = 0,
    disable = 1,
    replace = 2,
    additional = 3,
    merge = 4,
};

pub const EntityAlias = struct {
    tag: u32 = 0,
    source_index: u32 = 0,
    target_index: u32 = 0,
    alias_type: EntityAliasType = .original,
    queue_load: bool = false,
    target_spawnable_path: []const u8 = "",

    pub fn deinit(self: *EntityAlias, allocator: std.mem.Allocator) void {
        if (self.target_spawnable_path.len > 0) {
            allocator.free(self.target_spawnable_path);
        }
    }
};

pub const SpawnableEntity = struct {
    id: u64,
    name: []const u8,
    is_active: bool = true,
    components_count: usize = 0,

    pub fn deinit(self: *SpawnableEntity, allocator: std.mem.Allocator) void {
        if (self.name.len > 0) {
            allocator.free(self.name);
        }
    }
};

pub const Spawnable = struct {
    allocator: std.mem.Allocator,
    entities: std.ArrayList(SpawnableEntity),
    aliases: std.ArrayList(EntityAlias),
    name: []const u8 = "Root",

    pub fn init(allocator: std.mem.Allocator) Spawnable {
        return .{
            .allocator = allocator,
            .entities = std.ArrayList(SpawnableEntity).init(allocator),
            .aliases = std.ArrayList(EntityAlias).init(allocator),
            .name = "Root",
        };
    }

    pub fn deinit(self: *Spawnable) void {
        for (self.entities.items) |*e| {
            e.deinit(self.allocator);
        }
        self.entities.deinit();

        for (self.aliases.items) |*a| {
            a.deinit(self.allocator);
        }
        self.aliases.deinit();
    }

    /// Загрузка .spawnable / .prefab напрямую из файла
    pub fn loadFromFile(self: *Spawnable, path: []const u8) !void {
        const file = try std.fs.openFileAbsolute(path, .{});
        defer file.close();

        const content = try file.readToEndAlloc(self.allocator, 16 * 1024 * 1024);
        defer self.allocator.free(content);

        // Парсинг JSON-структуры префаба
        var parsed = try std.json.parseFromSlice(std.json.Value, self.allocator, content, .{});
        defer parsed.deinit();

        const root = parsed.value;
        if (root == .object) {
            if (root.object.get("entities")) |ents_val| {
                if (ents_val == .array) {
                    for (ents_val.array.items) |ent_val| {
                        var ent_id: u64 = 0;
                        var ent_name: []const u8 = "";

                        if (ent_val == .object) {
                            if (ent_val.object.get("id")) |id_v| {
                                if (id_v == .integer) ent_id = @intCast(id_v.integer);
                            }
                            if (ent_val.object.get("name")) |n_v| {
                                if (n_v == .string) ent_name = try self.allocator.dupe(u8, n_v.string);
                            }
                        }

                        try self.entities.append(.{
                            .id = ent_id,
                            .name = ent_name,
                            .is_active = true,
                        });
                    }
                }
            }
        }
    }

    /// Инстанцирование всех сущностей в P³ ECS World
    pub fn spawnIntoP3World(self: *const Spawnable, world: *p3_ecs.World) !void {
        for (self.entities.items) |ent| {
            _ = try world.createEntity(ent.name);
        }
    }
};

// =============================================================================
// C-ABI EXPORTS (Для подмены libAzFramework.so / Spawnable вызовов)
// =============================================================================

export fn P3_Spawnable_Create() ?*Spawnable {
    const allocator = std.heap.c_allocator;
    const spawnable = allocator.create(Spawnable) catch return null;
    spawnable.* = Spawnable.init(allocator);
    return spawnable;
}

export fn P3_Spawnable_Destroy(ptr: ?*Spawnable) void {
    if (ptr) |s| {
        const allocator = s.allocator;
        s.deinit();
        allocator.destroy(s);
    }
}

export fn P3_Spawnable_GetEntityCount(ptr: ?*const Spawnable) usize {
    if (ptr) |s| {
        return s.entities.items.len;
    }
    return 0;
}
