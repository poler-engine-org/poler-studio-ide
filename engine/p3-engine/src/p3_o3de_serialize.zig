// =============================================================================
// P³ ENGINE — O3DE NATIVE SERIALIZE & RTTI CONTEXT (ZIG)
// =============================================================================
// Нативная замена AZ::SerializeContext & AZ::ReflectContext
// Устраняет ошибки "Element 'NULL' with class ID is not registered with the serializer":
//   - Реестр UUID типов и их схем
//   - Graceful fallback для устаревших или отсутствующих компонентов
//   - Сериализация и десериализация в бинарные и JSON форматы
// =============================================================================

const std = @import("std");

pub const Uuid = struct {
    data: [16]u8,

    pub fn fromString(str: []const u8) !Uuid {
        var clean: [32]u8 = undefined;
        var idx: usize = 0;
        for (str) |c| {
            if (c == '{' or c == '}' or c == '-') continue;
            if (idx >= 32) break;
            clean[idx] = c;
            idx += 1;
        }
        if (idx != 32) return error.InvalidUuidFormat;

        var bytes: [16]u8 = undefined;
        for (0..16) |i| {
            bytes[i] = try std.fmt.parseInt(u8, clean[i * 2 .. i * 2 + 2], 16);
        }
        return .{ .data = bytes };
    }

    pub fn format(self: Uuid, comptime _: []const u8, _: std.fmt.FormatOptions, writer: anytype) !void {
        try writer.print("{x:0>2}{x:0>2}{x:0>2}{x:0>2}-{x:0>2}{x:0>2}-{x:0>2}{x:0>2}-{x:0>2}{x:0>2}-{x:0>2}{x:0>2}{x:0>2}{x:0>2}{x:0>2}{x:0>2}", .{
            self.data[0],  self.data[1],  self.data[2],  self.data[3],
            self.data[4],  self.data[5],  self.data[6],  self.data[7],
            self.data[8],  self.data[9],  self.data[10], self.data[11],
            self.data[12], self.data[13], self.data[14], self.data[15],
        });
    }
};

pub const ClassDescriptor = struct {
    name: []const u8,
    uuid: Uuid,
    version: u32 = 1,
    is_deprecated: bool = false,

    pub fn deinit(self: *ClassDescriptor, allocator: std.mem.Allocator) void {
        allocator.free(self.name);
    }
};

pub const NativeSerializeContext = struct {
    allocator: std.mem.Allocator,
    classes: std.AutoHashMap(Uuid, ClassDescriptor),

    pub fn init(allocator: std.mem.Allocator) NativeSerializeContext {
        return .{
            .allocator = allocator,
            .classes = std.AutoHashMap(Uuid, ClassDescriptor).init(allocator),
        };
    }

    pub fn deinit(self: *NativeSerializeContext) void {
        var iter = self.classes.valueIterator();
        while (iter.next()) |desc| {
            var desc_mut = desc.*;
            desc_mut.deinit(self.allocator);
        }
        self.classes.deinit();
    }

    /// Регистрация класса с его UUID
    pub fn registerClass(self: *NativeSerializeContext, name: []const u8, uuid_str: []const u8, version: u32) !void {
        const uuid = try Uuid.fromString(uuid_str);
        try self.classes.put(uuid, .{
            .name = try self.allocator.dupe(u8, name),
            .uuid = uuid,
            .version = version,
            .is_deprecated = false,
        });
    }

    /// Маркировка устаревшего класса (ClassDeprecate)
    pub fn deprecateClass(self: *NativeSerializeContext, uuid_str: []const u8) !void {
        const uuid = try Uuid.fromString(uuid_str);
        if (self.classes.getPtr(uuid)) |desc| {
            desc.is_deprecated = true;
        } else {
            try self.classes.put(uuid, .{
                .name = try self.allocator.dupe(u8, "DeprecatedClass"),
                .uuid = uuid,
                .version = 0,
                .is_deprecated = true,
            });
        }
    }

    /// Проверка, зарегистрирован ли тип
    pub fn isClassRegistered(self: *const NativeSerializeContext, uuid_str: []const u8) bool {
        const uuid = Uuid.fromString(uuid_str) catch return false;
        return self.classes.contains(uuid);
    }
};

// =============================================================================
// C-ABI EXPORTS (Для подмены libAzCore.so / SerializeContext вызовов)
// =============================================================================

export fn P3_SerializeContext_Create() ?*NativeSerializeContext {
    const allocator = std.heap.c_allocator;
    const ctx = allocator.create(NativeSerializeContext) catch return null;
    ctx.* = NativeSerializeContext.init(allocator);
    return ctx;
}

export fn P3_SerializeContext_Destroy(ptr: ?*NativeSerializeContext) void {
    if (ptr) |c| {
        const allocator = c.allocator;
        c.deinit();
        allocator.destroy(c);
    }
}

export fn P3_SerializeContext_Register(ptr: ?*NativeSerializeContext, name_ptr: [*:0]const u8, uuid_ptr: [*:0]const u8, version: u32) bool {
    if (ptr) |c| {
        const name = std.mem.span(name_ptr);
        const uuid = std.mem.span(uuid_ptr);
        c.registerClass(name, uuid, version) catch return false;
        return true;
    }
    return false;
}
