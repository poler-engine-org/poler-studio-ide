// =============================================================================
// P³ UI DRAW v1.0 — ZIG
// =============================================================================
//
// 2D drawing abstraction (из O3DE IDraw2d).
//
// В O3DE: IDraw2d — интерфейс для отрисовки UI-примитивов:
//   DrawImage, DrawLine, DrawText, DrawQuad
//   Deferred rendering queue, blend modes, pixel rounding
//
// В P³ Engine:
//   - Тот же набор примитивов
//   - Blend mode enum
//   - Vertex struct (Pos + Color + UV)
//   - Deferred draw queue (sort by draw_order, then batch)
//   - Backend: абстрактный — пользователь подключает renderer
//
// Портировано из O3DE/Gems/LyShine/Code/Include/LyShine/IDraw2d.h
// Адаптировано для Zig 0.14.0 + P³ Engine API.
//
// Архитектор: Kotokvit (математик), Super Z (исполнение)
// =============================================================================

const std = @import("std");
const math = std.math;
const ui_transform = @import("p3_ui_transform.zig");

pub const Vec2 = ui_transform.Vec2;
pub const Rect = ui_transform.Rect;

// =============================================================================
// 1. COLOR
// =============================================================================

/// RGBA цвет (f32 для точной интерполяции)
pub const Color = struct {
    r: f32,
    g: f32,
    b: f32,
    a: f32,

    pub inline fn init(r: f32, g: f32, b: f32, a: f32) Color {
        return .{ .r = r, .g = g, .b = b, .a = a };
    }

    pub inline fn white() Color {
        return .{ .r = 1, .g = 1, .b = 1, .a = 1 };
    }

    pub inline fn black() Color {
        return .{ .r = 0, .g = 0, .b = 0, .a = 1 };
    }

    pub inline fn transparent() Color {
        return .{ .r = 0, .g = 0, .b = 0, .a = 0 };
    }

    pub inline fn lerp(a: Color, b: Color, t: f32) Color {
        return .{
            .r = a.r + t * (b.r - a.r),
            .g = a.g + t * (b.g - a.g),
            .b = a.b + t * (b.b - a.b),
            .a = a.a + t * (b.a - a.a),
        };
    }
};

// =============================================================================
// 2. BLEND MODE
// =============================================================================

/// Режим смешивания
pub const BlendMode = enum(u3) {
    alpha = 0, // Standard alpha blend
    additive = 1, // Additive (for glow/particles)
    multiply = 2, // Multiply (for shadows)
    premultiplied = 3, // Pre-multiplied alpha
};

// =============================================================================
// 3. VERTEX
// =============================================================================

/// Вершина для UI-рендера: Position + Color + UV
pub const UiVertex = struct {
    x: f32,
    y: f32,
    u: f32,
    v: f32,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
};

// =============================================================================
// 4. DRAW PRIMITIVE TYPES
// =============================================================================

/// Тип draw-примитива
pub const DrawPrimitiveType = enum(u3) {
    quad = 0, // Текстурированный/цветной прямоугольник
    line = 1, // Отрезок линии
    text = 2, // Текст
    triangle = 3, // Треугольник
    nine_slice = 4, // 9-slice sprite
};

// =============================================================================
// 5. DRAW COMMAND
// =============================================================================

/// Команда отрисовки (deferred queue entry).
///
/// В O3DE: RenderGraph node.
/// В P³: упрощённая команда для батчинга.
pub const DrawCommand = struct {
    primitive_type: DrawPrimitiveType,
    draw_order: i32,
    blend_mode: BlendMode,
    clip_rect: Rect,
    // Quad data
    position: Vec2,
    size: Vec2,
    color: Color,
    // Texture ID (0 = no texture, solid color)
    texture_id: u32,
    // UV coords (for textured quads)
    uv_min: Vec2,
    uv_max: Vec2,
    // Line data
    line_start: Vec2,
    line_end: Vec2,
    line_width: f64,
    // Text data
    text_ptr: ?[*]const u8,
    text_len: usize,
    font_size: f64,
    // 9-slice borders
    slice_left: f64,
    slice_top: f64,
    slice_right: f64,
    slice_bottom: f64,

    /// Создать команду для текстурированного квадрата
    pub fn drawQuad(
        pos: Vec2,
        sz: Vec2,
        clr: Color,
        tex_id: u32,
        uv_min_val: Vec2,
        uv_max_val: Vec2,
        order: i32,
        blend: BlendMode,
    ) DrawCommand {
        return .{
            .primitive_type = .quad,
            .draw_order = order,
            .blend_mode = blend,
            .clip_rect = Rect.init(-1e10, -1e10, 1e10, 1e10),
            .position = pos,
            .size = sz,
            .color = clr,
            .texture_id = tex_id,
            .uv_min = uv_min_val,
            .uv_max = uv_max_val,
            .line_start = Vec2.zero(),
            .line_end = Vec2.zero(),
            .line_width = 1,
            .text_ptr = null,
            .text_len = 0,
            .font_size = 16,
            .slice_left = 0,
            .slice_top = 0,
            .slice_right = 0,
            .slice_bottom = 0,
        };
    }

    /// Создать команду для линии
    pub fn drawLine(
        start: Vec2,
        end: Vec2,
        width: f64,
        clr: Color,
        order: i32,
    ) DrawCommand {
        return .{
            .primitive_type = .line,
            .draw_order = order,
            .blend_mode = .alpha,
            .clip_rect = Rect.init(-1e10, -1e10, 1e10, 1e10),
            .position = start,
            .size = Vec2.zero(),
            .color = clr,
            .texture_id = 0,
            .uv_min = Vec2.zero(),
            .uv_max = Vec2.init(1, 1),
            .line_start = start,
            .line_end = end,
            .line_width = width,
            .text_ptr = null,
            .text_len = 0,
            .font_size = 16,
            .slice_left = 0,
            .slice_top = 0,
            .slice_right = 0,
            .slice_bottom = 0,
        };
    }

    /// Создать команду для текста
    pub fn drawText(
        text: []const u8,
        pos: Vec2,
        clr: Color,
        font_sz: f64,
        order: i32,
    ) DrawCommand {
        return .{
            .primitive_type = .text,
            .draw_order = order,
            .blend_mode = .alpha,
            .clip_rect = Rect.init(-1e10, -1e10, 1e10, 1e10),
            .position = pos,
            .size = Vec2.zero(),
            .color = clr,
            .texture_id = 0,
            .uv_min = Vec2.zero(),
            .uv_max = Vec2.init(1, 1),
            .line_start = Vec2.zero(),
            .line_end = Vec2.zero(),
            .line_width = 1,
            .text_ptr = text.ptr,
            .text_len = text.len,
            .font_size = font_sz,
            .slice_left = 0,
            .slice_top = 0,
            .slice_right = 0,
            .slice_bottom = 0,
        };
    }
};

// =============================================================================
// 6. DRAW QUEUE (DEFERRED RENDERING)
// =============================================================================

/// Очередь отрисовки — собирает команды, сортирует по draw_order,
/// затем батчит и отправляет на renderer backend.
pub const DrawQueue = struct {
    commands: std.ArrayList(DrawCommand),
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) DrawQueue {
        return .{
            .commands = std.ArrayList(DrawCommand).init(allocator),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *DrawQueue) void {
        self.commands.deinit();
    }

    /// Добавить команду в очередь
    pub inline fn push(self: *DrawQueue, cmd: DrawCommand) !void {
        try self.commands.append(cmd);
    }

    /// Сортировать по draw_order (ascending = back-to-front)
    pub fn sort(self: *DrawQueue) void {
        const SortContext = struct {
            fn lessThan(_: void, a: DrawCommand, b: DrawCommand) bool {
                return a.draw_order < b.draw_order;
            }
        };
        std.mem.sort(DrawCommand, self.commands.items, {}, SortContext.lessThan);
    }

    /// Количество команд
    pub inline fn len(self: DrawQueue) usize {
        return self.commands.items.len;
    }

    /// Очистить очередь
    pub inline fn clear(self: *DrawQueue) void {
        self.commands.clearRetainingCapacity();
    }
};

// =============================================================================
// 7. PIXEL ROUNDING
// =============================================================================

/// Pixel rounding — выравнивание координат до пикселей.
///
/// В O3DE: опция для чёткого текста при масштабе.
pub fn roundToPixel(coord: f64, scale: f64) f64 {
    return @round(coord * scale) / scale;
}

/// Round Vec2 to pixel boundaries
pub fn roundVec2ToPixel(v: Vec2, scale: f64) Vec2 {
    return Vec2.init(roundToPixel(v.x, scale), roundToPixel(v.y, scale));
}

// =============================================================================
// 8. ТЕСТЫ
// =============================================================================

test "Color: lerp" {
    const a = Color.black();
    const b = Color.white();
    const mid = Color.lerp(a, b, 0.5);
    try std.testing.expectApproxEqAbs(mid.r, 0.5, 1e-6);
    try std.testing.expectApproxEqAbs(mid.g, 0.5, 1e-6);
    try std.testing.expectApproxEqAbs(mid.a, 1.0, 1e-6);
}

test "Color: transparent" {
    const c = Color.transparent();
    try std.testing.expectApproxEqAbs(c.a, 0.0, 1e-6);
}

test "DrawCommand: drawQuad" {
    const cmd = DrawCommand.drawQuad(
        Vec2.init(10, 20),
        Vec2.init(100, 50),
        Color.white(),
        1, // texture
        Vec2.init(0, 0),
        Vec2.init(1, 1),
        0, // draw order
        .alpha,
    );
    try std.testing.expect(cmd.primitive_type == .quad);
    try std.testing.expectApproxEqAbs(cmd.position.x, 10.0, 1e-10);
    try std.testing.expectApproxEqAbs(cmd.size.x, 100.0, 1e-10);
}

test "DrawCommand: drawLine" {
    const cmd = DrawCommand.drawLine(
        Vec2.init(0, 0),
        Vec2.init(100, 100),
        2.0,
        Color.white(),
        0,
    );
    try std.testing.expect(cmd.primitive_type == .line);
    try std.testing.expectApproxEqAbs(cmd.line_width, 2.0, 1e-10);
}

test "DrawQueue: push and sort" {
    const allocator = std.testing.allocator;
    var queue = DrawQueue.init(allocator);
    defer queue.deinit();

    try queue.push(DrawCommand.drawLine(Vec2.zero(), Vec2.init(1, 1), 1, Color.white(), 10));
    try queue.push(DrawCommand.drawLine(Vec2.zero(), Vec2.init(1, 1), 1, Color.white(), 0));
    try queue.push(DrawCommand.drawLine(Vec2.zero(), Vec2.init(1, 1), 1, Color.white(), 5));

    try std.testing.expect(queue.len() == 3);
    queue.sort();

    // After sort: order 0, 5, 10
    try std.testing.expect(queue.commands.items[0].draw_order == 0);
    try std.testing.expect(queue.commands.items[1].draw_order == 5);
    try std.testing.expect(queue.commands.items[2].draw_order == 10);
}

test "DrawQueue: clear" {
    const allocator = std.testing.allocator;
    var queue = DrawQueue.init(allocator);
    defer queue.deinit();

    try queue.push(DrawCommand.drawLine(Vec2.zero(), Vec2.init(1, 1), 1, Color.white(), 0));
    queue.clear();
    try std.testing.expect(queue.len() == 0);
}

test "roundToPixel: identity at scale=1" {
    try std.testing.expectApproxEqAbs(roundToPixel(5.0, 1.0), 5.0, 1e-10);
}

test "roundToPixel: rounds at scale=2" {
    try std.testing.expectApproxEqAbs(roundToPixel(5.3, 2.0), 5.5, 1e-10);
}
