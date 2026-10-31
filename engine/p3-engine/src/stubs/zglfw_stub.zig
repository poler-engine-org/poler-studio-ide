// =============================================================================
// STUB: zglfw — минимальный интерфейс для тестов без GLFW
// =============================================================================
// Тестам p3_input.zig нужен @import("zglfw") для типов.
// Этот стаб предоставляет пустые типы — тесты проверяют логику, не GLFW.

const std = @import("std");

pub const Window = struct {
    pub fn shouldClose(self: *Window) bool {
        _ = self;
        return false;
    }
    pub fn getKey(self: *Window, key: Key) Action {
        _ = self;
        _ = key;
        return .release;
    }
    pub fn getMouseButton(self: *Window, button: MouseButton) Action {
        _ = self;
        _ = button;
        return .release;
    }
    pub fn getCursorPos(self: *Window) [2]f64 {
        _ = self;
        return .{ 0, 0 };
    }
    pub fn getFramebufferSize(self: *Window) [2]c_int {
        _ = self;
        return .{ 1280, 720 };
    }
    pub fn create(width: u32, height: u32, title: [:0]const u8, monitor: ?*Monitor, share: ?*Window) !*Window {
        _ = width;
        _ = height;
        _ = title;
        _ = monitor;
        _ = share;
        return undefined;
    }
    pub fn destroy(self: *Window) void {
        _ = self;
    }
};

pub const Key = enum(c_int) {
    a = 65,
    w = 87,
    s = 83,
    d = 68,
    q = 81,
    e = 69,
    r = 82,
    F1 = 290,
    F2 = 291,
    F12 = 301,
    space = 32,
    escape = 256,
    left_shift = 340,
    right_shift = 344,
    left_control = 341,
    right_control = 345,
    left_alt = 342,
    right_alt = 346,
    _,
};

pub const Action = enum(c_int) {
    release = 0,
    press = 1,
    repeat = 2,
};

pub const MouseButton = enum(c_int) {
    left = 0,
    right = 1,
    middle = 2,
    _,
};

pub const Monitor = opaque {};

pub fn init() !void {}
pub fn terminate() void {}
pub fn pollEvents() void {}
pub fn getTime() f64 { return 0; }
pub fn windowHint(hint: WindowHint, value: anytype) void {
    _ = hint;
    _ = value;
}

pub fn createWindow(width: c_int, height: c_int, title: [:0]const u8, monitor: ?*Monitor) !*Window {
    _ = width;
    _ = height;
    _ = title;
    _ = monitor;
    const dummy = try std.heap.page_allocator.create(Window);
    return dummy;
}

pub const WindowHint = enum(c_int) {
    client_api = 0x22001,
    resizable = 0x20003,
    no_api = 0,
    _,
};

// Platform-specific getters (stubs)
pub fn getWin32Window(window: *Window) ?*anyopaque { _ = window; return null; }
pub fn getX11Display() ?*anyopaque { return null; }
pub fn getX11Window(window: *Window) ?*anyopaque { _ = window; return null; }
pub fn getWaylandDisplay() ?*anyopaque { return null; }
pub fn getWaylandWindow(window: *Window) ?*anyopaque { _ = window; return null; }
pub fn getCocoaWindow(window: *Window) ?*anyopaque { _ = window; return null; }
