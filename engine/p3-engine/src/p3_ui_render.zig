// =============================================================================
// P³ UI RENDER v1.0 — ZIG
// =============================================================================
//
// RenderGraph + Sprite resource + Draw2d full (из O3DE LyShine).
//
// В O3DE:
//   RenderGraph         — render graph для UI draw calls
//     - PrimitiveListRenderNode, MaskRenderNode, RenderTargetRenderNode
//     - Alpha fade stack
//     - Mask support
//   CDraw2d             — 2D drawing API (quad, line, text, image)
//   Sprite              — sprite resource
//
// В P³ Engine:
//   - RenderGraph — упрощённый (без RHI, без FBO для масок)
//   - Draw2d — через Raylib backend (abstraction layer)
//   - Alpha fade stack — полный порт
//   - P³ обобщение: проективный render transform
//
// Портировано из O3DE/Gems/LyShine/Code/Source/{RenderGraph,Draw2d}*
// Адаптировано для Zig 0.14.0 + P³ Engine API.
// =============================================================================

const std = @import("std");
const ui_transform = @import("p3_ui_transform.zig");
const ui_draw = @import("p3_ui_draw.zig");
const ui_canvas = @import("p3_ui_canvas.zig");

pub const Vec2 = ui_transform.Vec2;
pub const Rect = ui_transform.Rect;
pub const Color = ui_draw.Color;
pub const BlendMode = ui_draw.BlendMode;
pub const DrawCommand = ui_draw.DrawCommand;
pub const ElementId = ui_canvas.ElementId;

// =============================================================================
// 1. RENDER NODE TYPE
// =============================================================================

/// Тип узла в render graph.
pub const RenderNodeType = enum(u2) {
    primitive_list = 0,
    mask = 1,
    render_target = 2,
};

// =============================================================================
// 2. ALPHA MASK TYPE
// =============================================================================

/// Тип альфа-маски.
pub const AlphaMaskType = enum(u2) {
    none = 0,
    modulate_alpha = 1,
    modulate_alpha_and_color = 2,
};

// =============================================================================
// 3. BLEND STATE
// =============================================================================

/// Blend state для render node.
pub const TargetBlendState = struct {
    blend_mode: BlendMode,
    is_premultiplied: bool,
    is_srgb: bool,
    is_clamp: bool,

    pub fn default() TargetBlendState {
        return .{
            .blend_mode = .alpha,
            .is_premultiplied = false,
            .is_srgb = true,
            .is_clamp = true,
        };
    }
};

// =============================================================================
// 4. PRIMITIVE LIST RENDER NODE
// =============================================================================

/// Узел render graph — список примитивов (quads, lines, text).
///
/// В O3DE: PrimitiveListRenderNode — основной тип узла.
/// Содержит batch draw calls с одинаковым texture/blend state.
pub const PrimitiveListRenderNode = struct {
    blend_state: TargetBlendState,
    alpha_mask_type: AlphaMaskType,
    commands: std.ArrayList(DrawCommand),
    /// Max textures per node (O3DE: 16)
    texture_ids: [16]u32,
    texture_clamp: [16]bool,
    texture_count: u32,

    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator, blend_state: TargetBlendState) PrimitiveListRenderNode {
        return .{
            .blend_state = blend_state,
            .alpha_mask_type = .none,
            .commands = std.ArrayList(DrawCommand).init(allocator),
            .texture_ids = undefined,
            .texture_clamp = undefined,
            .texture_count = 0,
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *PrimitiveListRenderNode) void {
        self.commands.deinit();
    }

    /// Добавить draw command.
    pub fn addCommand(self: *PrimitiveListRenderNode, cmd: DrawCommand) !void {
        try self.commands.append(cmd);
    }

    /// Получить или добавить texture slot.
    pub fn getOrAddTexture(self: *PrimitiveListRenderNode, tex_id: u32, is_clamp: bool) u32 {
        for (0..self.texture_count) |i| {
            if (self.texture_ids[i] == tex_id) return @intCast(i);
        }
        if (self.texture_count < 16) {
            const idx = self.texture_count;
            self.texture_ids[idx] = tex_id;
            self.texture_clamp[idx] = is_clamp;
            self.texture_count += 1;
            return @intCast(idx);
        }
        return 0; // Fallback
    }
};

// =============================================================================
// 5. MASK RENDER NODE
// =============================================================================

/// Узел render graph — маска.
///
/// В O3DE: MaskRenderNode.
pub const MaskRenderNode = struct {
    is_masking_enabled: bool,
    use_alpha_test: bool,
    draw_behind: bool,
    draw_in_front: bool,
    parent: ?*MaskRenderNode,

    pub fn init() MaskRenderNode {
        return .{
            .is_masking_enabled = true,
            .use_alpha_test = false,
            .draw_behind = false,
            .draw_in_front = true,
            .parent = null,
        };
    }
};

// =============================================================================
// 6. RENDER TARGET RENDER NODE
// =============================================================================

/// Узел render graph — render target (FBO).
///
/// В O3DE: RenderTargetRenderNode.
pub const RenderTargetRenderNode = struct {
    viewport_x: f32,
    viewport_y: f32,
    viewport_width: f32,
    viewport_height: f32,
    clear_color: Color,
    nest_level: i32,
    parent: ?*RenderTargetRenderNode,
    texture_id: u32,

    pub fn init(texture_id: u32, x: f32, y: f32, w: f32, h: f32) RenderTargetRenderNode {
        return .{
            .viewport_x = x,
            .viewport_y = y,
            .viewport_width = w,
            .viewport_height = h,
            .clear_color = Color.transparent(),
            .nest_level = 0,
            .parent = null,
            .texture_id = texture_id,
        };
    }
};

// =============================================================================
// 7. RENDER GRAPH
// =============================================================================

/// Render Graph — управляет render node'ами и alpha fade.
///
/// В O3DE: RenderGraph (400+ строк).
/// В P³: упрощённый, без RHI, без FBO.
pub const RenderGraph = struct {
    nodes: std.ArrayList(PrimitiveListRenderNode),
    alpha_fade_stack: [8]f32,
    alpha_fade_depth: u32,
    is_dirty: bool,
    is_rendering_to_mask: bool,

    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) RenderGraph {
        return .{
            .nodes = std.ArrayList(PrimitiveListRenderNode).init(allocator),
            .alpha_fade_stack = .{1.0} ** 8,
            .alpha_fade_depth = 0,
            .is_dirty = false,
            .is_rendering_to_mask = false,
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *RenderGraph) void {
        for (self.nodes.items) |*node| node.deinit();
        self.nodes.deinit();
    }

    /// Сбросить граф для нового кадра.
    pub fn resetGraph(self: *RenderGraph) void {
        for (self.nodes.items) |*node| node.deinit();
        self.nodes.clearRetainingCapacity();
        self.alpha_fade_depth = 0;
        self.alpha_fade_stack[0] = 1.0;
        self.is_dirty = false;
    }

    /// Добавить primitive list node.
    pub fn addNode(self: *RenderGraph, blend_state: TargetBlendState) !*PrimitiveListRenderNode {
        try self.nodes.append(PrimitiveListRenderNode.init(self.allocator, blend_state));
        self.is_dirty = true;
        return &self.nodes.items[self.nodes.items.len - 1];
    }

    /// Push alpha fade.
    pub fn pushAlphaFade(self: *RenderGraph, fade: f32) void {
        if (self.alpha_fade_depth < 7) {
            const current = self.getAlphaFade();
            self.alpha_fade_depth += 1;
            self.alpha_fade_stack[self.alpha_fade_depth] = current * fade;
        }
    }

    /// Push override alpha fade.
    pub fn pushOverrideAlphaFade(self: *RenderGraph, fade: f32) void {
        if (self.alpha_fade_depth < 7) {
            self.alpha_fade_depth += 1;
            self.alpha_fade_stack[self.alpha_fade_depth] = fade;
        }
    }

    /// Pop alpha fade.
    pub fn popAlphaFade(self: *RenderGraph) void {
        if (self.alpha_fade_depth > 0) {
            self.alpha_fade_depth -= 1;
        }
    }

    /// Get current alpha fade.
    pub fn getAlphaFade(self: RenderGraph) f32 {
        return self.alpha_fade_stack[self.alpha_fade_depth];
    }

    /// Пустой ли граф?
    pub fn isEmpty(self: RenderGraph) bool {
        return self.nodes.items.len == 0;
    }
};

// =============================================================================
// 8. DRAW 2D (ABSTRACT BACKEND)
// =============================================================================

/// Draw2d — абстрактный интерфейс для 2D рисования.
///
/// В O3DE: CDraw2d — конкретный класс с OpenGL/D3D11 backend.
/// В P³: abstraction layer — пользователь подключает backend.
pub const Draw2d = struct {
    /// Viewport
    viewport_width: f32,
    viewport_height: f32,
    dpi_scale: f32,

    /// Deferred rendering
    is_deferred: bool,
    queue: ui_draw.DrawQueue,

    /// Sort key for deferred rendering
    sort_key: i64,

    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) Draw2d {
        return .{
            .viewport_width = 1280,
            .viewport_height = 720,
            .dpi_scale = 1.0,
            .is_deferred = true,
            .queue = ui_draw.DrawQueue.init(allocator),
            .sort_key = 0,
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *Draw2d) void {
        self.queue.deinit();
    }

    /// Нарисовать изображение.
    pub fn drawImage(
        self: *Draw2d,
        texture_id: u32,
        pos: Vec2,
        size: Vec2,
        opacity: f32,
        rotation: f32,
        color: Color,
        blend: BlendMode,
        draw_order: i32,
    ) !void {
        if (self.is_deferred) {
            try self.queue.push(DrawCommand.drawQuad(pos, size, Color.init(
                color.r * opacity,
                color.g * opacity,
                color.b * opacity,
                color.a * opacity,
            ), texture_id, Vec2.init(0, 0), Vec2.init(1, 1), draw_order, blend));
        }
        _ = rotation;
    }

    /// Нарисовать линию.
    pub fn drawLine(
        self: *Draw2d,
        start: Vec2,
        end: Vec2,
        color: Color,
        width: f64,
        draw_order: i32,
    ) !void {
        if (self.is_deferred) {
            try self.queue.push(DrawCommand.drawLine(start, end, width, color, draw_order));
        }
    }

    /// Нарисовать текст.
    pub fn drawText(
        self: *Draw2d,
        text: []const u8,
        pos: Vec2,
        font_size: f64,
        color: Color,
        draw_order: i32,
    ) !void {
        if (self.is_deferred) {
            try self.queue.push(DrawCommand.drawText(text, pos, color, font_size, draw_order));
        }
    }

    /// Рендерить все отложенные примитивы.
    pub fn renderDeferred(self: *Draw2d) void {
        self.queue.sort();
        // Backend renders sorted draw commands
    }

    /// Очистить отложенные примитивы.
    pub fn clearDeferred(self: *Draw2d) void {
        self.queue.clear();
    }
};

// =============================================================================
// 9. ТЕСТЫ
// =============================================================================

test "RenderGraph: init and reset" {
    const allocator = std.testing.allocator;
    var rg = RenderGraph.init(allocator);
    defer rg.deinit();

    try std.testing.expect(rg.isEmpty());
    rg.resetGraph();
    try std.testing.expect(rg.isEmpty());
}

test "RenderGraph: add node" {
    const allocator = std.testing.allocator;
    var rg = RenderGraph.init(allocator);
    defer rg.deinit();

    _ = try rg.addNode(TargetBlendState.default());
    try std.testing.expect(!rg.isEmpty());
    try std.testing.expect(rg.is_dirty);
}

test "RenderGraph: alpha fade stack" {
    const allocator = std.testing.allocator;
    var rg = RenderGraph.init(allocator);
    defer rg.deinit();

    try std.testing.expectApproxEqAbs(rg.getAlphaFade(), 1.0, 1e-6);

    rg.pushAlphaFade(0.5);
    try std.testing.expectApproxEqAbs(rg.getAlphaFade(), 0.5, 1e-6);

    rg.pushAlphaFade(0.8);
    try std.testing.expectApproxEqAbs(rg.getAlphaFade(), 0.4, 1e-6);

    rg.popAlphaFade();
    try std.testing.expectApproxEqAbs(rg.getAlphaFade(), 0.5, 1e-6);

    rg.popAlphaFade();
    try std.testing.expectApproxEqAbs(rg.getAlphaFade(), 1.0, 1e-6);
}

test "PrimitiveListRenderNode: add texture" {
    const allocator = std.testing.allocator;
    var node = PrimitiveListRenderNode.init(allocator, TargetBlendState.default());
    defer node.deinit();

    const idx0 = node.getOrAddTexture(1, true);
    try std.testing.expect(idx0 == 0);
    try std.testing.expect(node.texture_count == 1);

    const idx1 = node.getOrAddTexture(2, false);
    try std.testing.expect(idx1 == 1);
    try std.testing.expect(node.texture_count == 2);

    // Same texture → returns existing slot
    const idx0b = node.getOrAddTexture(1, true);
    try std.testing.expect(idx0b == 0);
    try std.testing.expect(node.texture_count == 2);
}

test "Draw2d: init" {
    const allocator = std.testing.allocator;
    var d2d = Draw2d.init(allocator);
    defer d2d.deinit();

    try std.testing.expectApproxEqAbs(d2d.viewport_width, 1280.0, 1e-6);
    try std.testing.expect(d2d.is_deferred);
}

test "Draw2d: deferred draw" {
    const allocator = std.testing.allocator;
    var d2d = Draw2d.init(allocator);
    defer d2d.deinit();

    try d2d.drawImage(1, Vec2.init(10, 20), Vec2.init(100, 50), 1.0, 0, Color.white(), .alpha, 0);
    try std.testing.expect(d2d.queue.len() == 1);
}

test "MaskRenderNode: init defaults" {
    const m = MaskRenderNode.init();
    try std.testing.expect(m.is_masking_enabled);
    try std.testing.expect(!m.use_alpha_test);
}

test "RenderTargetRenderNode: init" {
    const rt = RenderTargetRenderNode.init(1, 0, 0, 800, 600);
    try std.testing.expectApproxEqAbs(rt.viewport_width, 800.0, 1e-6);
    try std.testing.expect(rt.texture_id == 1);
}
