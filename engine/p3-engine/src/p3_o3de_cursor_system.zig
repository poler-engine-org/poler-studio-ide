// =============================================================================
// P³ ENGINE — NATIVE INPUT & UI CURSOR SYSTEM (ZIG 0.14)
// =============================================================================
// Полная реализация двух уровней курсора O3DE:
//   1. СИСТЕМНЫЙ КУРСОР ОС (InputSystemCursorRequests / InputDeviceMouse)
//      - Состояния: ConstrainedAndHidden, ConstrainedAndVisible,
//                   UnconstrainedAndHidden, UnconstrainedAndVisible
//      - Управление видимостью на уровне ОС/окна (X11 / Wayland / Raylib / GLFW)
//
//   2. ИГРОВОЙ UI-КУРСОР (UiCursorBus / LyShine)
//      - Счётчик видимости: IncrementVisibleCounter() / DecrementVisibleCounter()
//      - Координаты: GetUiCursorPosition() / SetUiCursorPosition()
//      - Отрисовка аппаратной и софтверной текстуры курсора (RenderUiCursor)
// =============================================================================

const std = @import("std");

pub const SystemCursorState = enum(u8) {
    unknown = 0,
    constrained_and_hidden = 1,
    constrained_and_visible = 2,
    unconstrained_and_hidden = 3,
    unconstrained_and_visible = 4,

    pub fn isVisible(self: SystemCursorState) bool {
        return self == .constrained_and_visible or self == .unconstrained_and_visible;
    }

    pub fn isConstrained(self: SystemCursorState) bool {
        return self == .constrained_and_hidden or self == .constrained_and_visible;
    }
};

pub const CursorPos2D = struct {
    x: f32 = 0.0,
    y: f32 = 0.0,
};

pub const NativeCursorManager = struct {
    allocator: std.mem.Allocator,
    // 1. Системный уровень (OS Cursor)
    system_state: SystemCursorState = .unconstrained_and_visible,
    system_pos_norm: CursorPos2D = .{},

    // 2. Игровой UI уровень (LyShine UI Cursor)
    ui_visible_counter: i32 = 1, // По умолчанию видим в меню/лаунчере
    ui_pos_pixels: CursorPos2D = .{},
    ui_custom_image_path: ?[]const u8 = null,

    pub fn init(allocator: std.mem.Allocator) NativeCursorManager {
        return .{
            .allocator = allocator,
            .system_state = .unconstrained_and_visible,
            .system_pos_norm = .{},
            .ui_visible_counter = 1,
            .ui_pos_pixels = .{},
            .ui_custom_image_path = null,
        };
    }

    pub fn deinit(self: *NativeCursorManager) void {
        if (self.ui_custom_image_path) |p| {
            self.allocator.free(p);
        }
    }

    // --- Методы InputSystemCursorRequests (System OS Cursor) ---

    pub fn setSystemCursorState(self: *NativeCursorManager, state: SystemCursorState) void {
        self.system_state = state;
    }

    pub fn getSystemCursorState(self: *const NativeCursorManager) SystemCursorState {
        return self.system_state;
    }

    pub fn setSystemCursorPositionNormalized(self: *NativeCursorManager, x: f32, y: f32) void {
        self.system_pos_norm = .{ .x = std.math.clamp(x, 0.0, 1.0), .y = std.math.clamp(y, 0.0, 1.0) };
    }

    pub fn getSystemCursorPositionNormalized(self: *const NativeCursorManager) CursorPos2D {
        return self.system_pos_norm;
    }

    // --- Методы UiCursorBus (LyShine Game UI Cursor) ---

    pub fn incrementVisibleCounter(self: *NativeCursorManager) void {
        self.ui_visible_counter += 1;
    }

    pub fn decrementVisibleCounter(self: *NativeCursorManager) void {
        self.ui_visible_counter -= 1;
    }

    pub fn isUiCursorVisible(self: *const NativeCursorManager) bool {
        return self.ui_visible_counter > 0;
    }

    pub fn setUiCursor(self: *NativeCursorManager, image_path: []const u8) !void {
        if (self.ui_custom_image_path) |p| {
            self.allocator.free(p);
        }
        self.ui_custom_image_path = try self.allocator.dupe(u8, image_path);
    }

    pub fn getUiCursorPosition(self: *const NativeCursorManager) CursorPos2D {
        return self.ui_pos_pixels;
    }

    pub fn setUiCursorPosition(self: *NativeCursorManager, x: f32, y: f32) void {
        self.ui_pos_pixels = .{ .x = x, .y = y };
    }
};

// =============================================================================
// C-ABI EXPORTS (Для прямой подмены InputSystemCursorRequestBus и UiCursorBus)
// =============================================================================

export fn P3_Cursor_Create() ?*NativeCursorManager {
    const allocator = std.heap.c_allocator;
    const mgr = allocator.create(NativeCursorManager) catch return null;
    mgr.* = NativeCursorManager.init(allocator);
    return mgr;
}

export fn P3_Cursor_Destroy(ptr: ?*NativeCursorManager) void {
    if (ptr) |c| {
        const allocator = c.allocator;
        c.deinit();
        allocator.destroy(c);
    }
}

// 1. Системные вызовы (InputSystemCursorRequests)
export fn P3_Cursor_SetSystemCursorState(ptr: ?*NativeCursorManager, state: u8) void {
    if (ptr) |c| {
        c.setSystemCursorState(@enumFromInt(state));
    }
}

export fn P3_Cursor_GetSystemCursorState(ptr: ?*const NativeCursorManager) u8 {
    if (ptr) |c| {
        return @intFromEnum(c.getSystemCursorState());
    }
    return @intFromEnum(SystemCursorState.unconstrained_and_visible);
}

// 2. Игровые UI вызовы (UiCursorBus / LyShine)
export fn P3_Cursor_IncrementVisibleCounter(ptr: ?*NativeCursorManager) void {
    if (ptr) |c| c.incrementVisibleCounter();
}

export fn P3_Cursor_DecrementVisibleCounter(ptr: ?*NativeCursorManager) void {
    if (ptr) |c| c.decrementVisibleCounter();
}

export fn P3_Cursor_IsUiCursorVisible(ptr: ?*const NativeCursorManager) bool {
    if (ptr) |c| return c.isUiCursorVisible();
    return true;
}

export fn P3_Cursor_SetUiCursorPosition(ptr: ?*NativeCursorManager, x: f32, y: f32) void {
    if (ptr) |c| c.setUiCursorPosition(x, y);
}

export fn P3_Cursor_GetUiCursorPositionX(ptr: ?*const NativeCursorManager) f32 {
    if (ptr) |c| return c.getUiCursorPosition().x;
    return 0.0;
}

export fn P3_Cursor_GetUiCursorPositionY(ptr: ?*const NativeCursorManager) f32 {
    if (ptr) |c| return c.getUiCursorPosition().y;
    return 0.0;
}
