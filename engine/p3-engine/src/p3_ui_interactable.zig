// =============================================================================
// P³ UI INTERACTABLE v1.0 — ZIG
// =============================================================================
//
// Interactable base + State actions (из O3DE UiInteractableComponent).
//
// В O3DE: UiInteractableComponent — базовый компонент для всех
// интерактивных элементов (Button, Checkbox, Slider, etc).
// Управляет:
//   - Состояниями: Normal, Hover, Pressed, Disabled
//   - State actions: цвет/альфа/спрайт/шрифт при смене состояния
//   - Action callbacks: hoverStart, hoverEnd, pressed, released
//   - Multi-touch
//   - Navigation (tab order, arrow keys)
//
// В P³ Engine:
//   - Без EBus — прямые вызовы и function pointers
//   - Без RTTI — Zig comptime
//   - State actions — tagged union вместо virtual dispatch
//   - Callbacks — fn pointers вместо AZStd::function
//
// Портировано из O3DE/Gems/LyShine/Code/Source/UiInteractableComponent
// Адаптировано для Zig 0.14.0 + P³ Engine API.
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const ui_transform = @import("p3_ui_transform.zig");
const ui_canvas = @import("p3_ui_canvas.zig");
const ui_draw = @import("p3_ui_draw.zig");

pub const Vec2 = ui_transform.Vec2;
pub const Rect = ui_transform.Rect;
pub const ElementId = ui_canvas.ElementId;
pub const ElementState = ui_canvas.ElementState;
pub const Color = ui_draw.Color;

// =============================================================================
// 1. INTERACTABLE STATE ENUM
// =============================================================================

/// Расширенное состояние interactable (больше чем ElementState).
///
/// В O3DE: UiInteractableStatesInterface::State
/// Normal = 0, Hover = 1, Pressed = 2, Disabled = 3
/// Плюс дополнительные состояния в подклассах (DragNormal, DragValid, etc).
pub const InteractableState = enum(u4) {
    normal = 0,
    hover = 1,
    pressed = 2,
    disabled = 3,
    // Draggable extensions
    drag_normal = 4,
    drag_valid = 5,
    drag_invalid = 6,
    // Dropdown extension
    expanded = 7,
    // DropTarget extensions
    drop_valid = 8,
    drop_invalid = 9,

    pub fn toElementState(self: InteractableState) ElementState {
        return switch (self) {
            .normal => .normal,
            .hover => .hover,
            .pressed => .pressed,
            .disabled => .disabled,
            else => .normal,
        };
    }
};

// =============================================================================
// 2. STATE ACTION (TAGGED UNION — вместо virtual dispatch)
// =============================================================================

/// State action — действие, которое применяется при переходе в состояние.
///
/// В O3DE: UiInteractableStateAction (virtual base) с подклассами:
///   - UiInteractableStateColor  → меняет цвет целевого элемента
///   - UiInteractableStateAlpha  → меняет альфа целевого элемента
///   - UiInteractableStateSprite → меняет спрайт целевого элемента
///   - UiInteractableStateFont   → меняет шрифт целевого элемента
///
/// В P³: tagged union — zero-cost, без vtable, без allocation.
pub const StateAction = union(enum) {
    /// Меняет цвет целевого элемента
    color: struct {
        target_id: ElementId,
        color: Color,
    },

    /// Меняет альфа целевого элемента
    alpha: struct {
        target_id: ElementId,
        alpha: f32,
    },

    /// Меняет спрайт целевого элемента
    sprite: struct {
        target_id: ElementId,
        sprite_path: []const u8,
        cell_index: u32,
    },

    /// Меняет шрифт целевого элемента
    font: struct {
        target_id: ElementId,
        font_path: []const u8,
        effect_index: u32,
    },

    /// Включает/выключает целевой элемент
    visibility: struct {
        target_id: ElementId,
        visible: bool,
    },

    /// P³ обобщение: проективный трансформ целевого элемента
    /// Сохраняет cross-ratio при смене состояния
    projective_transform: struct {
        target_id: ElementId,
        /// 3x3 проективный трансформ (8 DOF) в canvas space
        m: [3][3]f32,
    },
};

// =============================================================================
// 3. ACTION CALLBACK TYPES
// =============================================================================

/// Callback при action (hover, press, release).
///
/// В O3DE: AZStd::function<void(AZ::EntityId, AZ::Vector2)>
/// В P³: function pointer — zero-cost, без allocation.
pub const ActionCallback = *const fn (element_id: ElementId, position: Vec2) void;

/// Callback при click (Button).
pub const OnClickCallback = *const fn (element_id: ElementId, position: Vec2) void;

/// Callback при state change (Checkbox).
pub const StateChangeCallback = *const fn (element_id: ElementId, new_state: bool) void;

/// Callback при value change (Slider, ScrollBar).
pub const ValueChangeCallback = *const fn (element_id: ElementId, new_value: f32) void;

/// Callback при scroll offset change (ScrollBox).
pub const ScrollOffsetCallback = *const fn (element_id: ElementId, offset: Vec2) void;

/// Callback при text input change.
pub const TextInputCallback = *const fn (element_id: ElementId, text: []const u8) void;

/// Пустой callback (no-op).
pub fn noopActionCallback(_: ElementId, _: Vec2) void {}
pub fn noopStateChangeCallback(_: ElementId, _: bool) void {}
pub fn noopValueChangeCallback(_: ElementId, _: f32) void {}
pub fn noopScrollOffsetCallback(_: ElementId, _: Vec2) void {}
pub fn noopTextInputCallback(_: ElementId, _: []const u8) void {}

// =============================================================================
// 4. ACTION NAME (для сериализации/событий)
// =============================================================================

/// Имя action для диспетчеризации событий.
///
/// В O3DE: LyShine::ActionName = AZStd::string
/// В P³: []const u8 — zero-cost slice, без allocation.
pub const ActionName = []const u8;

// =============================================================================
// 5. STATE ACTION LIST
// =============================================================================

/// Список state actions для одного состояния.
///
/// В O3DE: AZStd::vector<UiInteractableStateAction*>
/// В P³: массив фиксированного размера (inline, без allocation).
pub const StateActionList = struct {
    actions: [8]StateAction,
    count: u32,

    pub fn init() StateActionList {
        return .{
            .actions = undefined,
            .count = 0,
        };
    }

    pub fn add(self: *StateActionList, action: StateAction) void {
        if (self.count < 8) {
            self.actions[self.count] = action;
            self.count += 1;
        }
    }

    pub fn clear(self: *StateActionList) void {
        self.count = 0;
    }

    pub fn items(self: StateActionList) []const StateAction {
        return self.actions[0..self.count];
    }
};

// =============================================================================
// 6. INTERACTABLE
// =============================================================================

/// Interactable — базовый компонент для интерактивных UI элементов.
///
/// Управляет:
///   - Состояниями (normal/hover/pressed/disabled)
///   - State actions (что происходит при смене состояния)
///   - Action callbacks (уведомления о действиях)
///   - Обработкой ввода
///   - Multi-touch
///
/// В O3DE: 612-строчный UiInteractableComponent.
/// В P³: упрощённый, без EBus, без RTTI, без сериализации.
pub const Interactable = struct {
    // --- Identity ---
    element_id: ElementId,

    // --- State ---
    state: InteractableState,
    is_hover: bool,
    is_pressed: bool,
    is_handling_events: bool,
    is_handling_multi_touch: bool,
    is_auto_activation_enabled: bool,

    // --- Pressed point ---
    pressed_point: Vec2,
    pressed_multi_touch_index: i32,

    // --- State Actions ---
    hover_state_actions: StateActionList,
    pressed_state_actions: StateActionList,
    disabled_state_actions: StateActionList,

    // --- Action Callbacks ---
    hover_start_callback: ActionCallback,
    hover_end_callback: ActionCallback,
    pressed_callback: ActionCallback,
    released_callback: ActionCallback,

    // --- Action Names (для диспетчеризации) ---
    hover_start_action: ActionName,
    hover_end_action: ActionName,
    pressed_action: ActionName,
    released_action: ActionName,
    outside_released_action: ActionName,

    /// Инициализация по умолчанию
    pub fn init(element_id: ElementId) Interactable {
        return .{
            .element_id = element_id,
            .state = .normal,
            .is_hover = false,
            .is_pressed = false,
            .is_handling_events = true,
            .is_handling_multi_touch = false,
            .is_auto_activation_enabled = true,
            .pressed_point = Vec2.zero(),
            .pressed_multi_touch_index = -1,
            .hover_state_actions = StateActionList.init(),
            .pressed_state_actions = StateActionList.init(),
            .disabled_state_actions = StateActionList.init(),
            .hover_start_callback = noopActionCallback,
            .hover_end_callback = noopActionCallback,
            .pressed_callback = noopActionCallback,
            .released_callback = noopActionCallback,
            .hover_start_action = "",
            .hover_end_action = "",
            .pressed_action = "",
            .released_action = "",
            .outside_released_action = "",
        };
    }

    /// Вычислить текущее состояние на основе флагов.
    ///
    /// В O3DE: ComputeInteractableState() — virtual.
    /// В P³: прямая функция.
    pub fn computeState(self: Interactable) InteractableState {
        if (!self.is_handling_events) return .disabled;
        if (self.is_pressed) return .pressed;
        if (self.is_hover) return .hover;
        return .normal;
    }

    /// Применить state actions для данного состояния.
    ///
    /// В O3DE: m_stateActionManager.SetInteractableState()
    /// В P³: прямой dispatch по tagged union.
    pub fn applyStateActions(self: Interactable, new_state: InteractableState) void {
        const actions = switch (new_state) {
            .hover => self.hover_state_actions.items(),
            .pressed => self.pressed_state_actions.items(),
            .disabled => self.disabled_state_actions.items(),
            else => &[_]StateAction{},
        };

        _ = actions;
        // State actions are applied to target elements via the canvas
        // (implementation depends on canvas having access to element properties)
    }

    /// Обработать hover start.
    pub fn handleHoverStart(self: *Interactable) void {
        self.is_hover = true;
        self.state = self.computeState();
        self.applyStateActions(self.state);
        self.hover_start_callback(self.element_id, self.pressed_point);
    }

    /// Обработать hover end.
    pub fn handleHoverEnd(self: *Interactable) void {
        self.is_hover = false;
        self.state = self.computeState();
        self.applyStateActions(self.state);
        self.hover_end_callback(self.element_id, self.pressed_point);
    }

    /// Обработать press.
    pub fn handlePressed(self: *Interactable, point: Vec2) bool {
        if (!self.is_handling_events) return false;
        self.is_pressed = true;
        self.pressed_point = point;
        self.state = self.computeState();
        self.applyStateActions(self.state);
        self.pressed_callback(self.element_id, point);
        return true;
    }

    /// Обработать release.
    /// Возвращает: true если это был клик (press + release на том же элементе).
    pub fn handleReleased(self: *Interactable, point: Vec2) bool {
        const was_pressed = self.is_pressed;
        self.is_pressed = false;
        self.state = self.computeState();
        self.applyStateActions(self.state);
        self.released_callback(self.element_id, point);
        return was_pressed;
    }

    /// Обработать lost active status (когда другой элемент перехватывает ввод).
    pub fn lostActiveStatus(self: *Interactable) void {
        self.is_pressed = false;
        self.is_hover = false;
        self.state = .normal;
        self.applyStateActions(.normal);
    }

    /// Can handle event at given point?
    pub fn canHandleEvent(self: Interactable) bool {
        return self.is_handling_events;
    }

    /// Пометить как disabled.
    pub fn setEnabled(self: *Interactable, enabled: bool) void {
        self.is_handling_events = enabled;
        if (!enabled) {
            self.is_pressed = false;
            self.is_hover = false;
            self.state = .disabled;
            self.applyStateActions(.disabled);
        } else {
            self.state = .normal;
            self.applyStateActions(.normal);
        }
    }
};

// =============================================================================
// 7. INTERACTABLE STATE ACTION APPLICATOR
// =============================================================================

/// Применяет StateAction к целевому элементу.
///
/// Это заменяет O3DE's виртуальный ApplyState() на прямой dispatch.
pub fn applyStateAction(action: StateAction, canvas: *ui_canvas.UiCanvas) void {
    switch (action) {
        .color => |c| {
            if (canvas.findElement(c.target_id)) |elem| {
                _ = elem; // В полной интеграции: elem.color = c.color
            }
        },
        .alpha => |a| {
            if (canvas.findElement(a.target_id)) |elem| {
                _ = elem; // elem.alpha = a.alpha
            }
        },
        .sprite => |s| {
            if (canvas.findElement(s.target_id)) |elem| {
                _ = elem; // elem.sprite = loadSprite(s.sprite_path, s.cell_index)
            }
        },
        .font => |f| {
            if (canvas.findElement(f.target_id)) |elem| {
                _ = elem; // elem.font = loadFont(f.font_path, f.effect_index)
            }
        },
        .visibility => |v| {
            if (canvas.findElement(v.target_id)) |elem| {
                elem.visible = v.visible;
            }
        },
        .projective_transform => |p| {
            if (canvas.findElement(p.target_id)) |elem| {
                _ = elem; // elem.projectiveTransform = p.m
            }
        },
    }
}

// =============================================================================
// 8. ТЕСТЫ
// =============================================================================

test "Interactable: init defaults" {
    const inter = Interactable.init(1);
    try std.testing.expect(inter.element_id == 1);
    try std.testing.expect(inter.state == .normal);
    try std.testing.expect(!inter.is_hover);
    try std.testing.expect(!inter.is_pressed);
    try std.testing.expect(inter.is_handling_events);
}

test "Interactable: handle hover start" {
    var inter = Interactable.init(1);
    inter.handleHoverStart();
    try std.testing.expect(inter.is_hover);
    try std.testing.expect(inter.state == .hover);
}

test "Interactable: handle hover end" {
    var inter = Interactable.init(1);
    inter.handleHoverStart();
    inter.handleHoverEnd();
    try std.testing.expect(!inter.is_hover);
    try std.testing.expect(inter.state == .normal);
}

test "Interactable: handle press" {
    var inter = Interactable.init(1);
    const handled = inter.handlePressed(Vec2.init(100, 200));
    try std.testing.expect(handled);
    try std.testing.expect(inter.is_pressed);
    try std.testing.expect(inter.state == .pressed);
    try std.testing.expectApproxEqAbs(inter.pressed_point.x, 100.0, 1e-10);
}

test "Interactable: handle release" {
    var inter = Interactable.init(1);
    _ = inter.handlePressed(Vec2.init(100, 200));
    const clicked = inter.handleReleased(Vec2.init(100, 200));
    try std.testing.expect(clicked);
    try std.testing.expect(!inter.is_pressed);
    try std.testing.expect(inter.state == .normal);
}

test "Interactable: lost active status" {
    var inter = Interactable.init(1);
    _ = inter.handlePressed(Vec2.init(100, 200));
    inter.lostActiveStatus();
    try std.testing.expect(!inter.is_pressed);
    try std.testing.expect(!inter.is_hover);
    try std.testing.expect(inter.state == .normal);
}

test "Interactable: disabled" {
    var inter = Interactable.init(1);
    inter.setEnabled(false);
    try std.testing.expect(!inter.is_handling_events);
    try std.testing.expect(inter.state == .disabled);

    const handled = inter.handlePressed(Vec2.init(100, 200));
    try std.testing.expect(!handled);
}

test "Interactable: compute state" {
    var inter = Interactable.init(1);
    try std.testing.expect(inter.computeState() == .normal);

    inter.is_hover = true;
    try std.testing.expect(inter.computeState() == .hover);

    inter.is_pressed = true;
    try std.testing.expect(inter.computeState() == .pressed);

    inter.is_handling_events = false;
    try std.testing.expect(inter.computeState() == .disabled);
}

test "StateActionList: add and iterate" {
    var list = StateActionList.init();
    list.add(.{ .color = .{ .target_id = 1, .color = Color.white() } });
    list.add(.{ .alpha = .{ .target_id = 2, .alpha = 0.5 } });
    try std.testing.expect(list.count == 2);

    const items = list.items();
    try std.testing.expect(items.len == 2);
    try std.testing.expect(items[0] == .color);
    try std.testing.expect(items[1] == .alpha);
}

test "StateActionList: overflow capped at 8" {
    var list = StateActionList.init();
    for (0..10) |_| {
        list.add(.{ .visibility = .{ .target_id = 0, .visible = true } });
    }
    try std.testing.expect(list.count == 8);
}

test "InteractableState: to ElementState" {
    try std.testing.expect(ElementState.normal == InteractableState.normal.toElementState());
    try std.testing.expect(ElementState.hover == InteractableState.hover.toElementState());
    try std.testing.expect(ElementState.pressed == InteractableState.pressed.toElementState());
    try std.testing.expect(ElementState.disabled == InteractableState.disabled.toElementState());
}
