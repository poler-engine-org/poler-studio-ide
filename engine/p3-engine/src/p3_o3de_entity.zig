// =============================================================================
// P³ ENGINE — NATIVE REPLACEMENT FOR AZ::Entity & ComponentApplication (ZIG 0.14)
// =============================================================================
// Полный нативный аналог AZ::Entity (UUID {75651658-...}):
//   - Состояния жизненного цикла: Constructed, Initializing, Init, Activating, Active, Deactivating
//   - Добавление, инициализация, активация и деактивация компонентов
//   - Прямая интеграция с миром P³ ECS (p3_ecs.zig)
//   - C-ABI экспорты для подмены libAzCore.so / EntityBus
// =============================================================================

const std = @import("std");
const p3_ecs = @import("p3_ecs.zig");
const p3_o3de_transform = @import("p3_o3de_transform.zig");

pub const EntityState = enum(u8) {
    constructed = 0,
    initializing = 1,
    init = 2,
    activating = 3,
    active = 4,
    deactivating = 5,
};

pub const ComponentType = enum(u32) {
    transform = 1,
    mesh = 2,
    camera = 3,
    light = 4,
    script = 5,
    custom = 6,
};

pub const NativeComponent = struct {
    type_id: ComponentType,
    is_active: bool = false,
    data_ptr: ?*anyopaque = null,

    pub fn init(type_id: ComponentType, data: ?*anyopaque) NativeComponent {
        return .{
            .type_id = type_id,
            .is_active = false,
            .data_ptr = data,
        };
    }
};

pub const NativeEntity = struct {
    allocator: std.mem.Allocator,
    id: p3_ecs.EntityId,
    name: []const u8,
    state: EntityState = .constructed,
    components: std.ArrayList(NativeComponent),
    transform: p3_o3de_transform.NativeTransform,

    pub fn init(allocator: std.mem.Allocator, id_val: u64, name_str: []const u8) !NativeEntity {
        const owned_name = try allocator.dupe(u8, name_str);
        const ent_id = p3_ecs.EntityId.init(id_val);
        return .{
            .allocator = allocator,
            .id = ent_id,
            .name = owned_name,
            .state = .constructed,
            .components = std.ArrayList(NativeComponent).init(allocator),
            .transform = p3_o3de_transform.NativeTransform.init(ent_id),
        };
    }

    pub fn deinit(self: *NativeEntity) void {
        self.deactivate();
        self.allocator.free(self.name);
        self.components.deinit();
    }

    pub fn addComponent(self: *NativeEntity, type_id: ComponentType, data: ?*anyopaque) !void {
        if (self.state == .active) {
            // В O3DE нельзя добавлять компоненты в активную сущность без деактивации
            return error.EntityIsActive;
        }
        try self.components.append(NativeComponent.init(type_id, data));
    }

    pub fn initEntity(self: *NativeEntity) void {
        if (self.state == .constructed) {
            self.state = .initializing;
            // Инициализация компонентов
            self.state = .init;
        }
    }

    pub fn activate(self: *NativeEntity) void {
        if (self.state == .constructed) {
            self.initEntity();
        }
        if (self.state == .init) {
            self.state = .activating;
            for (self.components.items) |*comp| {
                comp.is_active = true;
            }
            self.state = .active;
        }
    }

    pub fn deactivate(self: *NativeEntity) void {
        if (self.state == .active) {
            self.state = .deactivating;
            for (self.components.items) |*comp| {
                comp.is_active = false;
            }
            self.state = .init;
        }
    }

    pub fn getState(self: *const NativeEntity) EntityState {
        return self.state;
    }
};

// =============================================================================
// C-ABI EXPORTS (Для подмены libAzCore.so / AZ::Entity)
// =============================================================================

export fn P3_Entity_Create(id: u64, name_ptr: [*:0]const u8) ?*NativeEntity {
    const allocator = std.heap.c_allocator;
    const name = std.mem.span(name_ptr);
    const ent = allocator.create(NativeEntity) catch return null;
    ent.* = NativeEntity.init(allocator, id, name) catch {
        allocator.destroy(ent);
        return null;
    };
    return ent;
}

export fn P3_Entity_Destroy(ptr: ?*NativeEntity) void {
    if (ptr) |e| {
        const allocator = e.allocator;
        e.deinit();
        allocator.destroy(e);
    }
}

export fn P3_Entity_Init(ptr: ?*NativeEntity) void {
    if (ptr) |e| e.initEntity();
}

export fn P3_Entity_Activate(ptr: ?*NativeEntity) void {
    if (ptr) |e| e.activate();
}

export fn P3_Entity_Deactivate(ptr: ?*NativeEntity) void {
    if (ptr) |e| e.deactivate();
}

export fn P3_Entity_GetState(ptr: ?*const NativeEntity) u8 {
    if (ptr) |e| return @intFromEnum(e.getState());
    return @intFromEnum(EntityState.constructed);
}
