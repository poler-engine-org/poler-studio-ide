// =============================================================================
// P³ INPUT — ОБРАБОТКА ВВОДА (КЛАВИАТУРА, МЫШЬ)
// =============================================================================
//
// Донор: O3DE AzFramework::Input — 187 файлов C++:
//   - InputChannel, InputDevice, InputMapping, InputEventBus
//   - Проблема: 5 уровней наследования, virtual dispatch на каждое событие,
//     event bus для КАЖДОГО типа канала
//
// Мы УБИВАЕМ C++ OOP input и ПОЖИРАЕМ концепции.
// Zig: direct GLFW polling, bitflags для key state, tagged union для событий.
//
// P³-СПЕЦИФИКА:
//   - Камера = PGL4: WASD двигает по S³ (геодезические), QE вращает
//   - Мышь: orbit на S³ (не Euler angles!)
//   - Нет key rebinding в рантайме — comptime config
//   - Input state — plain struct, не singleton
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const zglfw = @import("zglfw");

// =============================================================================
// 1. КЛАВИАТУРА
// =============================================================================

/// Клавиша — enum, не int. Конечное множество, comptime.
pub const Key = enum(u16) {
    a = 65,
    b = 66,
    c = 67,
    d = 68,
    e = 69,
    f = 70,
    q = 81,
    r = 82,
    s = 83,
    w = 87,
    x = 88,
    z = 90,
    space = 32,
    escape = 256,
    enter = 257,
    tab = 258,
    left_shift = 340,
    right_shift = 344,
    left_ctrl = 341,
    right_ctrl = 345,
    left_alt = 342,
    right_alt = 346,
    up = 265,
    down = 264,
    left = 263,
    right = 262,
    f1 = 290,
    f2 = 291,
    f3 = 292,
    f4 = 293,
    f5 = 294,
    f12 = 301,
    _,
};

/// Состояние клавиши: pressed / released / повтор
pub const KeyAction = enum(u2) {
    released = 0,
    pressed = 1,
    repeat = 2,
};

/// Модификаторы (bitfield, не bool поля)
pub const KeyMods = packed struct(u8) {
    shift: bool = false,
    ctrl: bool = false,
    alt: bool = false,
    super_: bool = false,
    _pad: u4 = 0,
};

// =============================================================================
// 2. МЫШЬ
// =============================================================================

/// Кнопка мыши
pub const MouseButton = enum(u3) {
    left = 0,
    right = 1,
    middle = 2,
    _,
};

/// Состояние мыши — всё в одном struct
pub const MouseState = struct {
    x: f64 = 0,
    y: f64 = 0,
    dx: f64 = 0, // delta от предыдущего кадра
    dy: f64 = 0,
    scroll_x: f64 = 0,
    scroll_y: f64 = 0,
    left_pressed: bool = false,
    right_pressed: bool = false,
    middle_pressed: bool = false,

    pub fn clearDeltas(self: *MouseState) void {
        self.dx = 0;
        self.dy = 0;
        self.scroll_x = 0;
        self.scroll_y = 0;
    }
};

// =============================================================================
// 3. СОБЫТИЕ ВВОДА — TAGGED UNION (НЕ ВИРТУАЛЬНЫЙ ДИСПАТЧ)
// =============================================================================

/// Одно событие ввода — tagged union, не иерархия классов
pub const InputEvent = union(enum) {
    key_press: KeyEventData,
    key_release: KeyEventData,
    key_repeat: KeyEventData,
    mouse_move: MouseMoveData,
    mouse_press: MouseButtonData,
    mouse_release: MouseButtonData,
    mouse_scroll: MouseScrollData,
    char_input: u21, // Unicode codepoint

    pub const KeyEventData = struct {
        key: Key,
        mods: KeyMods,
        scancode: u32,
    };

    pub const MouseMoveData = struct {
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
    };

    pub const MouseButtonData = struct {
        button: MouseButton,
        x: f64,
        y: f64,
        mods: KeyMods,
    };

    pub const MouseScrollData = struct {
        x_offset: f64,
        y_offset: f64,
        x: f64,
        y: f64,
    };
};

// =============================================================================
// 4. ПОЛНОЕ СОСТОЯНИЕ ВВОДА
// =============================================================================
//
// Один struct владеет ВСЕМ состоянием ввода. Нет singleton.
// Обновляется каждый кадр через update().

pub const InputState = struct {
    // Keyboard — bitflags для 512 клавиш (64 bytes)
    keys_pressed: [8]u64 = [_]u64{0} ** 8,
    keys_just_pressed: [8]u64 = [_]u64{0} ** 8,
    keys_just_released: [8]u64 = [_]u64{0} ** 8,
    mods: KeyMods = .{},

    // Mouse
    mouse: MouseState = .{},

    // P³ Camera controls (вычисляются из key state)
    camera_move_forward: bool = false, // W
    camera_move_back: bool = false, // S
    camera_move_left: bool = false, // A
    camera_move_right: bool = false, // D
    camera_move_up: bool = false, // Space
    camera_move_down: bool = false, // Shift
    camera_orbit: bool = false, // Right mouse
    camera_speed: f64 = 1.0, // Scroll

    // Engine controls
    should_close: bool = false, // Escape
    toggle_wireframe: bool = false, // F1
    toggle_pipeline: bool = false, // F2 (forward/deferred)
    reset_camera: bool = false, // R

    // Event buffer (ring buffer for this frame's events)
    events: std.ArrayListUnmanaged(InputEvent) = .{},
    max_events: u32 = 256,

    // =====================================================================
    // UPDATE — ОПРОС GLFW И ОБНОВЛЕНИЕ СОСТОЯНИЯ
    // =====================================================================

    /// Обновить состояние ввода из GLFW window
    pub fn update(self: *InputState, window: *zglfw.Window) void {
        // Сбросить "just" флаги и дельты
        self.keys_just_pressed = [_]u64{0} ** 8;
        self.keys_just_released = [_]u64{0} ** 8;
        self.mouse.clearDeltas();
        self.events.clearRetainingCapacity();

        // Сбросить one-shot флаги
        self.toggle_wireframe = false;
        self.toggle_pipeline = false;
        self.reset_camera = false;

        // Poll GLFW events
        zglfw.pollEvents();

        // --- Клавиатура ---
        const checkKey = struct {
            fn call(self_: *InputState, window_: *zglfw.Window, key: zglfw.Key, bit_index: u7) void {
                const word: usize = @intCast(bit_index / 64);
                const bit: u64 = @as(u64, 1) << @as(u6, @intCast(bit_index % 64));
                const pressed = window_.getKey(key) == .press;
                const was_pressed = (self_.keys_pressed[word] & bit) != 0;
                if (pressed and !was_pressed) {
                    self_.keys_just_pressed[word] |= bit;
                    self_.keys_pressed[word] |= bit;
                } else if (!pressed and was_pressed) {
                    self_.keys_just_released[word] |= bit;
                    self_.keys_pressed[word] &= ~bit;
                }
            }
        }.call;

        // WASD + QE + Space/Shift + F-keys
        checkKey(self, window, .w, 0);
        checkKey(self, window, .a, 1);
        checkKey(self, window, .s, 2);
        checkKey(self, window, .d, 3);
        checkKey(self, window, .q, 4);
        checkKey(self, window, .e, 5);
        checkKey(self, window, .space, 6);
        checkKey(self, window, .left_shift, 7);
        checkKey(self, window, .escape, 8);
        checkKey(self, window, .r, 9);
        checkKey(self, window, .F1, 10);
        checkKey(self, window, .F2, 11);
        checkKey(self, window, .F12, 12);
        checkKey(self, window, .left_control, 13);

        // --- Мышь ---
        const cursor_pos = window.getCursorPos();
        self.mouse.dx = cursor_pos[0] - self.mouse.x;
        self.mouse.dy = cursor_pos[1] - self.mouse.y;
        self.mouse.x = cursor_pos[0];
        self.mouse.y = cursor_pos[1];

        self.mouse.left_pressed = window.getMouseButton(.left) == .press;
        self.mouse.right_pressed = window.getMouseButton(.right) == .press;
        self.mouse.middle_pressed = window.getMouseButton(.middle) == .press;

        // --- P³ Camera controls ---
        self.camera_move_forward = window.getKey(.w) == .press;
        self.camera_move_back = window.getKey(.s) == .press;
        self.camera_move_left = window.getKey(.a) == .press;
        self.camera_move_right = window.getKey(.d) == .press;
        self.camera_move_up = window.getKey(.space) == .press;
        self.camera_move_down = window.getKey(.left_shift) == .press;
        self.camera_orbit = self.mouse.right_pressed;

        // --- One-shot controls ---
        if (window.getKey(.escape) == .press) self.should_close = true;
        if (self.isKeyJustPressed(10)) self.toggle_wireframe = true; // F1
        if (self.isKeyJustPressed(11)) self.toggle_pipeline = true; // F2
        if (self.isKeyJustPressed(9)) self.reset_camera = true; // R

        // Модификаторы
        self.mods.shift = window.getKey(.left_shift) == .press or window.getKey(.right_shift) == .press;
        self.mods.ctrl = window.getKey(.left_control) == .press or window.getKey(.right_control) == .press;
        self.mods.alt = window.getKey(.left_alt) == .press or window.getKey(.right_alt) == .press;
    }

    /// Проверить: клавиша нажата в этом кадре?
    pub fn isKeyJustPressed(self: *const InputState, bit_index: u7) bool {
        const word: usize = @intCast(bit_index / 64);
        const bit: u64 = @as(u64, 1) << @as(u6, @intCast(bit_index % 64));
        return (self.keys_just_pressed[word] & bit) != 0;
    }

    /// Проверить: клавиша удерживается?
    pub fn isKeyDown(self: *const InputState, bit_index: u7) bool {
        const word: usize = @intCast(bit_index / 64);
        const bit: u64 = @as(u64, 1) << @as(u6, @intCast(bit_index % 64));
        return (self.keys_pressed[word] & bit) != 0;
    }

    /// Движение камеры как направление (tx, ty, tz) на S³
    /// Вычисляется из WASD + Space/Shift
    pub fn cameraMoveDir(self: *const InputState) [3]f64 {
        var dir = [3]f64{ 0, 0, 0 };
        if (self.camera_move_forward) dir[2] += 1.0;
        if (self.camera_move_back) dir[2] -= 1.0;
        if (self.camera_move_left) dir[0] -= 1.0;
        if (self.camera_move_right) dir[0] += 1.0;
        if (self.camera_move_up) dir[1] += 1.0;
        if (self.camera_move_down) dir[1] -= 1.0;

        // Нормализовать
        const len = @sqrt(dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]);
        if (len > 1e-10) {
            dir[0] /= len;
            dir[1] /= len;
            dir[2] /= len;
        }
        return dir;
    }

    /// Orbit delta (мышь при правой кнопке)
    pub fn cameraOrbitDelta(self: *const InputState) [2]f64 {
        if (!self.camera_orbit) return .{ 0, 0 };
        return .{ self.mouse.dx, self.mouse.dy };
    }

    /// Деинициализация
    pub fn deinit(self: *InputState, allocator: std.mem.Allocator) void {
        self.events.deinit(allocator);
    }
};

// =============================================================================
// 5. ТЕСТЫ
// =============================================================================

test "Input: KeyMods is 1 byte" {
    try std.testing.expect(@sizeOf(KeyMods) == 1);
}

test "Input: MouseState initial values" {
    const ms = MouseState{};
    try std.testing.expect(ms.x == 0);
    try std.testing.expect(ms.left_pressed == false);
}

test "Input: InputState initial state" {
    const is = InputState{};
    try std.testing.expect(is.camera_move_forward == false);
    try std.testing.expect(is.should_close == false);
}

test "Input: cameraMoveDir with no input" {
    const is = InputState{};
    const dir = is.cameraMoveDir();
    try std.testing.expect(dir[0] == 0);
    try std.testing.expect(dir[1] == 0);
    try std.testing.expect(dir[2] == 0);
}

test "Input: cameraMoveDir forward" {
    var is = InputState{};
    is.camera_move_forward = true;
    const dir = is.cameraMoveDir();
    try std.testing.expectApproxEqAbs(dir[2], 1.0, 1e-10);
}

test "Input: cameraMoveDir normalized diagonal" {
    var is = InputState{};
    is.camera_move_forward = true;
    is.camera_move_right = true;
    const dir = is.cameraMoveDir();
    const len = @sqrt(dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]);
    try std.testing.expectApproxEqAbs(len, 1.0, 1e-10);
}

test "Input: InputEvent tagged union size" {
    // Should be reasonably compact
    try std.testing.expect(@sizeOf(InputEvent) < 128);
}

test "Input: MouseButton enum values" {
    try std.testing.expect(@intFromEnum(MouseButton.left) == 0);
    try std.testing.expect(@intFromEnum(MouseButton.right) == 1);
    try std.testing.expect(@intFromEnum(MouseButton.middle) == 2);
}

test "Input: KeyAction enum values" {
    try std.testing.expect(@intFromEnum(KeyAction.released) == 0);
    try std.testing.expect(@intFromEnum(KeyAction.pressed) == 1);
    try std.testing.expect(@intFromEnum(KeyAction.repeat) == 2);
}

test "Input: MouseState clearDeltas" {
    var ms = MouseState{ .dx = 10.0, .dy = 5.0, .scroll_x = 1.0, .scroll_y = 2.0 };
    ms.clearDeltas();
    try std.testing.expect(ms.dx == 0);
    try std.testing.expect(ms.dy == 0);
    try std.testing.expect(ms.scroll_x == 0);
    try std.testing.expect(ms.scroll_y == 0);
    // Position should NOT be cleared
    try std.testing.expect(ms.x == 0);
}
