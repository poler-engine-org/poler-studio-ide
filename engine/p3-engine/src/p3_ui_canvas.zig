// =============================================================================
// P³ UI CANVAS v1.0 — ZIG
// =============================================================================
//
// Canvas + Element hierarchy + input routing (из O3DE LyShine).
//
// В O3DE: UiCanvasComponent — корень UI-дерева, управляет:
//   - Деревом элементов (UiElementComponent)
//   - Маршрутизацией ввода (click, hover, drag)
//   - Draw order (z-сортировка)
//   - Render-to-texture
//
// В P³ Engine:
//   - Element hierarchy — обобщённая, без Qt-зависимости
//   - Input routing — через function pointers, не EBus
//   - Canvas → viewport transform — проективная (Mat3)
//
// Архитектор: Kotokvit (математик), Super Z (исполнение)
// =============================================================================

const std = @import("std");
const ui_transform = @import("p3_ui_transform.zig");

pub const Vec2 = ui_transform.Vec2;
pub const Rect = ui_transform.Rect;
pub const UiTransform2d = ui_transform.UiTransform2d;

// =============================================================================
// 1. ELEMENT ID
// =============================================================================

/// ID элемента в canvas.
pub const ElementId = u64;

// =============================================================================
// 2. ELEMENT STATE
// =============================================================================

/// Визуальное состояние элемента (для interactable).
pub const ElementState = enum(u3) {
    normal = 0,
    hover = 1,
    pressed = 2,
    disabled = 3,
    active = 4,
};

// =============================================================================
// 3. UI ELEMENT
// =============================================================================

/// UI Element — узел в иерархии canvas.
///
/// Каждый элемент имеет:
///   - Трансформ (позиция, размер, анкоры)
///   - Визуальное состояние
///   - Флаги (visible, enabled, interactable)
///   - Список детей
///   - Ссылку на родителя
pub const UiElement = struct {
    id: ElementId,
    name: []const u8,
    transform: UiTransform2d,
    state: ElementState,
    visible: bool,
    enabled: bool,
    interactable: bool,
    /// Комputed rect (после layout)
    computed_rect: Rect,
    /// Z-order для рендера
    draw_order: i32,
    /// Parent index (0 = root)
    parent_id: ElementId,
    /// User data (generic)
    user_data: u64,

    /// Инициализация элемента
    pub fn init(id: ElementId, name: []const u8) UiElement {
        return .{
            .id = id,
            .name = name,
            .transform = UiTransform2d.initDefault(),
            .state = .normal,
            .visible = true,
            .enabled = true,
            .interactable = false,
            .computed_rect = Rect.init(0, 0, 0, 0),
            .draw_order = 0,
            .parent_id = 0,
            .user_data = 0,
        };
    }

    /// Hit test: находится ли точка внутри элемента?
    pub fn hitTest(self: UiElement, point: Vec2) bool {
        if (!self.visible or !self.enabled or !self.interactable) return false;
        return point.x >= self.computed_rect.left and
            point.x <= self.computed_rect.right and
            point.y >= self.computed_rect.top and
            point.y <= self.computed_rect.bottom;
    }
};

// =============================================================================
// 4. INPUT EVENT
// =============================================================================

/// Тип события ввода
pub const InputEventType = enum(u3) {
    mouse_move = 0,
    mouse_down = 1,
    mouse_up = 2,
    key_down = 3,
    key_up = 4,
    scroll = 5,
    touch = 6,
};

/// Событие ввода
pub const InputEvent = struct {
    event_type: InputEventType,
    position: Vec2,
    button: u32,
    key: u32,
    delta: f64, // scroll delta
};

// =============================================================================
// 5. INPUT HANDLER (FUNCTION POINTER — НЕ EBUS)
// =============================================================================

/// Тип обработчика ввода.
///
/// В O3DE: EBus с Single handler policy.
/// В P³: function pointer — zero-cost, без vtable.
pub const InputHandler = *const fn (element: *UiElement, event: InputEvent) bool;

// =============================================================================
// 6. CANVAS
// =============================================================================

/// UI Canvas — корень UI-дерева.
///
/// Управляет:
///   - Коллекцией элементов
///   - Иерархией (parent/child)
///   - Маршрутизацией ввода
///   - Draw order
///   - Viewport size
///
/// В O3DE: UiCanvasComponent — 612 строк C++.
/// В P³: упрощённый, без Qt, без EBus, без animation system.
pub const UiCanvas = struct {
    elements: std.ArrayList(UiElement),
    viewport: Rect,
    next_id: ElementId,
    hovered_id: ElementId,
    pressed_id: ElementId,
    input_handler: ?InputHandler,
    allocator: std.mem.Allocator,

    /// Инициализация canvas
    pub fn init(allocator: std.mem.Allocator, viewport: Rect) UiCanvas {
        return .{
            .elements = std.ArrayList(UiElement).init(allocator),
            .viewport = viewport,
            .next_id = 1,
            .hovered_id = 0,
            .pressed_id = 0,
            .input_handler = null,
            .allocator = allocator,
        };
    }

    /// Встановити callback для подій, які canvas маршрутизує до елемента.
    pub fn setInputHandler(self: *UiCanvas, handler: ?InputHandler) void {
        self.input_handler = handler;
    }

    /// Освобождение
    pub fn deinit(self: *UiCanvas) void {
        self.elements.deinit();
    }

    /// Создать элемент и добавить в canvas
    pub fn createElement(self: *UiCanvas, name: []const u8) !ElementId {
        const id = self.next_id;
        self.next_id += 1;
        var elem = UiElement.init(id, name);
        // Root element has parent 0
        if (self.elements.items.len > 0) {
            elem.parent_id = 1; // Default to root
        }
        try self.elements.append(elem);
        return id;
    }

    /// Создать дочерний элемент
    pub fn createChild(self: *UiCanvas, parent_id: ElementId, name: []const u8) !ElementId {
        const id = self.next_id;
        self.next_id += 1;
        var elem = UiElement.init(id, name);
        elem.parent_id = parent_id;
        try self.elements.append(elem);
        return id;
    }

    /// Найти элемент по ID
    pub fn findElement(self: UiCanvas, id: ElementId) ?*UiElement {
        for (self.elements.items) |*elem| {
            if (elem.id == id) return elem;
        }
        return null;
    }

    fn findTopmostHit(self: *UiCanvas, point: Vec2) ?*UiElement {
        var i: usize = self.elements.items.len;
        while (i > 0) {
            i -= 1;
            const elem = &self.elements.items[i];
            if (elem.hitTest(point)) return elem;
        }
        return null;
    }

    /// Обработать событие ввода — маршрутизировать к элементам.
    ///
    /// В O3DE: input routing через EBus dispatch.
    /// В P³: прямой обход дерева, back-to-front (по draw_order).
    ///
    /// Возвращает: true если событие обработано (consumed)
    pub fn handleInput(self: *UiCanvas, event: InputEvent) bool {
        switch (event.event_type) {
            .mouse_move => {
                // Hit test: найти верхний interactable элемент
                var new_hovered: ElementId = 0;
                // Back-to-front: последний элемент с наибольшим draw_order
                if (self.findTopmostHit(event.position)) |elem| {
                    new_hovered = elem.id;
                    elem.state = .hover;
                    if (self.input_handler) |handler| {
                        _ = handler(elem, event);
                    }
                }
                // Unhover previous
                if (self.hovered_id != 0 and self.hovered_id != new_hovered) {
                    if (self.findElement(self.hovered_id)) |prev| {
                        if (prev.state == .hover) prev.state = .normal;
                    }
                }
                self.hovered_id = new_hovered;
                return new_hovered != 0;
            },
            .mouse_down => {
                if (self.findTopmostHit(event.position)) |elem| {
                    elem.state = .pressed;
                    self.hovered_id = elem.id;
                    self.pressed_id = elem.id;
                    if (self.input_handler) |handler| {
                        return handler(elem, event);
                    }
                    return true;
                }
                return false;
            },
            .mouse_up => {
                if (self.pressed_id != 0) {
                    if (self.findElement(self.pressed_id)) |elem| {
                        // Если всё ещё hovered → click
                        if (elem.hitTest(event.position)) {
                            elem.state = .hover;
                        } else {
                            elem.state = .normal;
                        }
                        const handled = if (self.input_handler) |handler| handler(elem, event) else true;
                        self.pressed_id = 0;
                        return handled;
                    }
                }
                self.pressed_id = 0;
                return false;
            },
            else => return false,
        }
    }

    /// Обновить computed_rect для всех элементов.
    ///
    /// Parent-before-child traversal (layout parent first).
    pub fn updateLayout(self: *UiCanvas) void {
        // Root element uses viewport
        for (self.elements.items) |*elem| {
            const parent_rect: Rect = if (elem.parent_id == 0)
                self.viewport
            else blk: {
                if (self.findElement(elem.parent_id)) |parent| {
                    break :blk parent.computed_rect;
                }
                break :blk self.viewport;
            };
            elem.computed_rect = elem.transform.computeLocalRect(parent_rect);
        }
    }

    /// Количество элементов
    pub inline fn size(self: UiCanvas) usize {
        return self.elements.items.len;
    }
};

// =============================================================================
// 7. ТЕСТЫ
// =============================================================================

fn countInputEvent(element: *UiElement, _: InputEvent) bool {
    element.user_data += 1;
    return true;
}

test "UiElement: init" {
    const elem = UiElement.init(1, "button");
    try std.testing.expect(elem.id == 1);
    try std.testing.expect(elem.visible);
    try std.testing.expect(!elem.interactable);
}

test "UiElement: hit test inside" {
    var elem = UiElement.init(1, "button");
    elem.computed_rect = Rect.fromSize(10, 20, 100, 50);
    elem.interactable = true;
    try std.testing.expect(elem.hitTest(Vec2.init(50, 40)));
}

test "UiElement: hit test outside" {
    var elem = UiElement.init(1, "button");
    elem.computed_rect = Rect.fromSize(10, 20, 100, 50);
    elem.interactable = true;
    try std.testing.expect(!elem.hitTest(Vec2.init(200, 200)));
}

test "UiElement: hit test disabled" {
    var elem = UiElement.init(1, "button");
    elem.computed_rect = Rect.fromSize(10, 20, 100, 50);
    elem.interactable = true;
    elem.enabled = false;
    try std.testing.expect(!elem.hitTest(Vec2.init(50, 40)));
}

test "UiCanvas: create elements" {
    const allocator = std.testing.allocator;
    var canvas = UiCanvas.init(allocator, Rect.fromSize(0, 0, 800, 600));
    defer canvas.deinit();

    const id1 = try canvas.createElement("root");
    const id2 = try canvas.createChild(id1, "child");
    try std.testing.expect(id1 == 1);
    try std.testing.expect(id2 == 2);
    try std.testing.expect(canvas.size() == 2);
}

test "UiCanvas: find element" {
    const allocator = std.testing.allocator;
    var canvas = UiCanvas.init(allocator, Rect.fromSize(0, 0, 800, 600));
    defer canvas.deinit();

    _ = try canvas.createElement("root");
    const elem = canvas.findElement(1);
    try std.testing.expect(elem != null);
    try std.testing.expectEqualStrings(elem.?.name, "root");
}

test "UiCanvas: input routing mouse move" {
    const allocator = std.testing.allocator;
    var canvas = UiCanvas.init(allocator, Rect.fromSize(0, 0, 800, 600));
    defer canvas.deinit();

    _ = try canvas.createElement("button");
    var elem = canvas.findElement(1).?;
    elem.computed_rect = Rect.fromSize(100, 100, 200, 50);
    elem.interactable = true;

    const handled = canvas.handleInput(.{
        .event_type = .mouse_move,
        .position = Vec2.init(150, 120),
        .button = 0,
        .key = 0,
        .delta = 0,
    });
    try std.testing.expect(handled);
    try std.testing.expect(canvas.hovered_id == 1);
}

test "UiCanvas: input routing click" {
    const allocator = std.testing.allocator;
    var canvas = UiCanvas.init(allocator, Rect.fromSize(0, 0, 800, 600));
    defer canvas.deinit();

    _ = try canvas.createElement("button");
    var elem = canvas.findElement(1).?;
    elem.computed_rect = Rect.fromSize(100, 100, 200, 50);
    elem.interactable = true;

    // Hover
    _ = canvas.handleInput(.{ .event_type = .mouse_move, .position = Vec2.init(150, 120), .button = 0, .key = 0, .delta = 0 });
    // Press
    const pressed = canvas.handleInput(.{ .event_type = .mouse_down, .position = Vec2.init(150, 120), .button = 0, .key = 0, .delta = 0 });
    try std.testing.expect(pressed);
    try std.testing.expect(canvas.pressed_id == 1);

    // Release
    const released = canvas.handleInput(.{ .event_type = .mouse_up, .position = Vec2.init(150, 120), .button = 0, .key = 0, .delta = 0 });
    try std.testing.expect(released);
    try std.testing.expect(canvas.pressed_id == 0);
}

test "UiCanvas: mouse down hit tests current position" {
    const allocator = std.testing.allocator;
    var canvas = UiCanvas.init(allocator, Rect.fromSize(0, 0, 800, 600));
    defer canvas.deinit();

    _ = try canvas.createElement("button");
    var elem = canvas.findElement(1).?;
    elem.computed_rect = Rect.fromSize(100, 100, 200, 50);
    elem.interactable = true;

    const pressed = canvas.handleInput(.{
        .event_type = .mouse_down,
        .position = Vec2.init(150, 120),
        .button = 0,
        .key = 0,
        .delta = 0,
    });
    try std.testing.expect(pressed);
    try std.testing.expectEqual(@as(ElementId, 1), canvas.pressed_id);
}

test "UiCanvas: dispatches input callback" {
    const allocator = std.testing.allocator;
    var canvas = UiCanvas.init(allocator, Rect.fromSize(0, 0, 800, 600));
    defer canvas.deinit();

    _ = try canvas.createElement("button");
    var elem = canvas.findElement(1).?;
    elem.computed_rect = Rect.fromSize(100, 100, 200, 50);
    elem.interactable = true;
    canvas.setInputHandler(countInputEvent);

    const position = Vec2.init(150, 120);
    _ = canvas.handleInput(.{ .event_type = .mouse_move, .position = position, .button = 0, .key = 0, .delta = 0 });
    _ = canvas.handleInput(.{ .event_type = .mouse_down, .position = position, .button = 0, .key = 0, .delta = 0 });
    _ = canvas.handleInput(.{ .event_type = .mouse_up, .position = position, .button = 0, .key = 0, .delta = 0 });

    try std.testing.expectEqual(@as(u64, 3), elem.user_data);
}

test "UiCanvas: update layout" {
    const allocator = std.testing.allocator;
    var canvas = UiCanvas.init(allocator, Rect.fromSize(0, 0, 800, 600));
    defer canvas.deinit();

    _ = try canvas.createElement("root");
    canvas.updateLayout();
    // Root layout computed from viewport
    const elem = canvas.findElement(1).?;
    // Default transform: anchors = (0,0,0,0), offsets = (0,0,0,0)
    // → rect = (0, 0, 0, 0) — zero-size at top-left
    try std.testing.expectApproxEqAbs(elem.computed_rect.left, 0.0, 1e-10);
}
