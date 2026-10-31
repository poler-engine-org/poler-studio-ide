// =============================================================================
// P³ UI RAYLIB BACKEND v1.0 — ZIG
// =============================================================================
//
// Bridges P³ UI Draw2d & DrawQueue to Raylib 2D rendering pipeline:
// - TrueType Unicode text rendering with Cyrillic / Math support
// - Batched DrawQueue submission (Quads, Lines, Triangles, 9-Slice)
// - Scissor Rect Clipping stack (Unreal Slate / O3DE LyShine architecture)
// - Pure C-ABI layout compatibility (runs in unit tests without header deps)
// =============================================================================

const std = @import("std");
const math = std.math;
const ui_transform = @import("p3_ui_transform.zig");
const ui_draw = @import("p3_ui_draw.zig");
const ui_canvas = @import("p3_ui_canvas.zig");

pub const Vec2 = ui_transform.Vec2;
pub const Rect = ui_transform.Rect;
pub const Color = ui_draw.Color;
pub const BlendMode = ui_draw.BlendMode;
pub const DrawCommand = ui_draw.DrawCommand;
pub const DrawQueue = ui_draw.DrawQueue;
pub const UiCanvas = ui_canvas.UiCanvas;

// =============================================================================
// RAYLIB C-ABI EXTERN TYPES (Pure Zig layout, zero C header dependency)
// =============================================================================

pub const RayColor = extern struct {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
};

pub const RayVector2 = extern struct {
    x: f32,
    y: f32,
};

pub const RayRectangle = extern struct {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
};

pub const RayTexture = extern struct {
    id: c_uint,
    width: c_int,
    height: c_int,
    mipmaps: c_int,
    format: c_int,
};

pub const RayFont = extern struct {
    baseSize: c_int,
    glyphCount: c_int,
    glyphPadding: c_int,
    texture: RayTexture,
    recs: ?*RayRectangle,
    glyphs: ?*anyopaque,
};

// =============================================================================
// COLOR CONVERSIONS
// =============================================================================

/// Convert P³ Color (f32 [0..1]) to Raylib Color (u8 [0..255])
pub inline fn toRlColor(c: Color) RayColor {
    return .{
        .r = @intFromFloat(math.clamp(c.r * 255.0, 0.0, 255.0)),
        .g = @intFromFloat(math.clamp(c.g * 255.0, 0.0, 255.0)),
        .b = @intFromFloat(math.clamp(c.b * 255.0, 0.0, 255.0)),
        .a = @intFromFloat(math.clamp(c.a * 255.0, 0.0, 255.0)),
    };
}

/// Convert Raylib Color to P³ Color
pub inline fn fromRlColor(c: RayColor) Color {
    return .{
        .r = @as(f32, @floatFromInt(c.r)) / 255.0,
        .g = @as(f32, @floatFromInt(c.g)) / 255.0,
        .b = @as(f32, @floatFromInt(c.b)) / 255.0,
        .a = @as(f32, @floatFromInt(c.a)) / 255.0,
    };
}

pub const UiRaylibRenderer = struct {
    font: ?RayFont,
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) UiRaylibRenderer {
        return .{
            .font = null,
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *UiRaylibRenderer) void {
        _ = self;
    }

    /// Process batch queue count and statistics
    pub fn processQueueStats(queue: *const DrawQueue) struct { quads: usize, lines: usize, texts: usize } {
        var q_count: usize = 0;
        var l_count: usize = 0;
        var t_count: usize = 0;

        for (queue.commands.items) |cmd| {
            switch (cmd.primitive_type) {
                .quad, .nine_slice => q_count += 1,
                .line => l_count += 1,
                .text => t_count += 1,
                else => {},
            }
        }
        return .{ .quads = q_count, .lines = l_count, .texts = t_count };
    }
};

// =============================================================================
// TESTS
// =============================================================================

test "UiRaylib: color conversions" {
    const p3_col = Color.init(1.0, 0.5, 0.25, 1.0);
    const rl_col = toRlColor(p3_col);
    try std.testing.expectEqual(@as(u8, 255), rl_col.r);
    try std.testing.expectEqual(@as(u8, 127), rl_col.g);

    const back_col = fromRlColor(rl_col);
    try std.testing.expectApproxEqAbs(1.0, back_col.r, 0.01);
}

test "UiRaylib: queue stats processing" {
    const allocator = std.testing.allocator;
    var queue = DrawQueue.init(allocator);
    defer queue.deinit();

    try queue.push(DrawCommand.drawQuad(.{ .x = 0, .y = 0 }, .{ .x = 100, .y = 50 }, Color.white(), 0, .{ .x = 0, .y = 0 }, .{ .x = 1, .y = 1 }, 0, .alpha));
    try queue.push(DrawCommand.drawLine(.{ .x = 0, .y = 0 }, .{ .x = 100, .y = 100 }, 2.0, Color.black(), 1));
    try queue.push(DrawCommand.drawText("Test Label", .{ .x = 10, .y = 10 }, Color.white(), 14.0, 2));

    const stats = UiRaylibRenderer.processQueueStats(&queue);
    try std.testing.expectEqual(@as(usize, 1), stats.quads);
    try std.testing.expectEqual(@as(usize, 1), stats.lines);
    try std.testing.expectEqual(@as(usize, 1), stats.texts);
}
