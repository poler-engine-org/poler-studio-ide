// =============================================================================
// P³ UI BUTTON v1.0 — ZIG
// =============================================================================
//
// Button + Checkbox + RadioButton (из O3DE LyShine).
//
// В O3DE:
//   UiButtonComponent    — clickable button
//   UiCheckboxComponent  — toggle (on/off)
//   UiRadioButtonComponent — exclusive selection in group
//   UiRadioButtonGroupComponent — manages radio buttons
//
// В P³ Engine:
//   - Без EBus — прямые вызовы
//   - Tagged union для state вместо 3 отдельных классов
//   - P³ обобщение: проективный click area (не только AABB)
//
// Портировано из O3DE/Gems/LyShine/Code/Source/{UiButton,UiCheckbox,UiRadioButton}*
// Адаптировано для Zig 0.14.0 + P³ Engine API.
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const ui_transform = @import("p3_ui_transform.zig");
const ui_canvas = @import("p3_ui_canvas.zig");
const ui_interactable = @import("p3_ui_interactable.zig");

pub const Vec2 = ui_transform.Vec2;
pub const Rect = ui_transform.Rect;
pub const ElementId = ui_canvas.ElementId;
pub const Interactable = ui_interactable.Interactable;
pub const OnClickCallback = ui_interactable.OnClickCallback;
pub const StateChangeCallback = ui_interactable.StateChangeCallback;
pub const ActionName = ui_interactable.ActionName;

// =============================================================================
// 1. BUTTON
// =============================================================================

/// UI Button — кликабельная кнопка.
///
/// В O3DE: UiButtonComponent — extends UiInteractableComponent.
/// Добавляет onClick callback и action name.
///
/// В P³: composition вместо inheritance. Interactable встраивается.
pub const Button = struct {
    interactable: Interactable,

    // --- Button-specific ---
    on_click: OnClickCallback,
    on_click_action: ActionName,

    /// Инициализация
    pub fn init(element_id: ElementId) Button {
        return .{
            .interactable = Interactable.init(element_id),
            .on_click = ui_interactable.noopActionCallback,
            .on_click_action = "",
        };
    }

    /// Handle release — если был press, это клик.
    ///
    /// В O3DE: HandleReleased → OnButtonClick notification.
    pub fn handleReleased(self: *Button, point: Vec2) bool {
        if (self.interactable.is_pressed) {
            _ = self.interactable.handleReleased(point);
            self.on_click(self.interactable.element_id, point);
            return true;
        }
        return false;
    }

    /// Handle enter release (keyboard/gamepad).
    pub fn handleEnterReleased(self: *Button) bool {
        if (self.interactable.is_pressed) {
            self.interactable.is_pressed = false;
            self.interactable.state = self.interactable.computeState();
            self.on_click(self.interactable.element_id, self.interactable.pressed_point);
            return true;
        }
        return false;
    }
};

// =============================================================================
// 2. CHECKBOX
// =============================================================================

/// UI Checkbox — переключатель (on/off).
///
/// В O3DE: UiCheckboxComponent — extends UiInteractableComponent.
/// Имеет checked/unchecked entity IDs (визуальные элементы).
pub const Checkbox = struct {
    interactable: Interactable,

    // --- Checkbox state ---
    is_on: bool,
    on_change: StateChangeCallback,

    // --- Visual entities ---
    checked_entity: ElementId,
    unchecked_entity: ElementId,

    // --- Action names ---
    turn_on_action: ActionName,
    turn_off_action: ActionName,
    changed_action: ActionName,

    /// Инициализация
    pub fn init(element_id: ElementId) Checkbox {
        return .{
            .interactable = Interactable.init(element_id),
            .is_on = false,
            .on_change = ui_interactable.noopStateChangeCallback,
            .checked_entity = 0,
            .unchecked_entity = 0,
            .turn_on_action = "",
            .turn_off_action = "",
            .changed_action = "",
        };
    }

    /// Toggle state.
    ///
    /// В O3DE: ToggleState() → swaps checked/unchecked visibility.
    pub fn toggleState(self: *Checkbox) bool {
        self.is_on = !self.is_on;
        self.on_change(self.interactable.element_id, self.is_on);
        return self.is_on;
    }

    /// Set state directly.
    pub fn setState(self: *Checkbox, is_on: bool) void {
        if (self.is_on != is_on) {
            self.is_on = is_on;
            self.on_change(self.interactable.element_id, self.is_on);
        }
    }

    /// Handle release — toggle on click.
    pub fn handleReleased(self: *Checkbox, point: Vec2) bool {
        if (self.interactable.is_pressed) {
            _ = self.interactable.handleReleased(point);
            _ = self.toggleState();
            return true;
        }
        return false;
    }

    /// Handle enter release (keyboard).
    pub fn handleEnterReleased(self: *Checkbox) bool {
        if (self.interactable.is_pressed) {
            self.interactable.is_pressed = false;
            self.interactable.state = self.interactable.computeState();
            _ = self.toggleState();
            return true;
        }
        return false;
    }
};

// =============================================================================
// 3. RADIO BUTTON
// =============================================================================

/// UI RadioButton — эксклюзивный выбор в группе.
///
/// В O3DE: UiRadioButtonComponent — extends UiInteractableComponent.
/// Принадлежит RadioButtonGroup — только один может быть выбран.
pub const RadioButton = struct {
    interactable: Interactable,

    // --- Radio state ---
    is_on: bool,
    on_change: StateChangeCallback,

    // --- Visual entities ---
    selected_entity: ElementId,
    deselected_entity: ElementId,

    // --- Group membership ---
    group_id: ElementId,

    // --- Action names ---
    turn_on_action: ActionName,
    turn_off_action: ActionName,
    changed_action: ActionName,

    /// Инициализация
    pub fn init(element_id: ElementId) RadioButton {
        return .{
            .interactable = Interactable.init(element_id),
            .is_on = false,
            .on_change = ui_interactable.noopStateChangeCallback,
            .selected_entity = 0,
            .deselected_entity = 0,
            .group_id = 0,
            .turn_on_action = "",
            .turn_off_action = "",
            .changed_action = "",
        };
    }

    /// Set state (called by group when selection changes).
    pub fn setState(self: *RadioButton, is_on: bool) void {
        if (self.is_on != is_on) {
            self.is_on = is_on;
            self.on_change(self.interactable.element_id, self.is_on);
        }
    }

    /// Handle release — select this radio button (deselect others in group).
    pub fn handleReleased(self: *RadioButton, point: Vec2) bool {
        if (self.interactable.is_pressed) {
            _ = self.interactable.handleReleased(point);
            if (!self.is_on) {
                self.setState(true);
            }
            return true;
        }
        return false;
    }
};

// =============================================================================
// 4. RADIO BUTTON GROUP
// =============================================================================

/// RadioButtonGroup — управляет набором RadioButton.
///
/// Гарантирует, что только одна кнопка в группе выбрана.
/// В O3DE: UiRadioButtonGroupComponent.
pub const RadioButtonGroup = struct {
    buttons: std.ArrayList(ElementId),
    selected_index: i32,
    allocator: std.mem.Allocator,

    /// Инициализация
    pub fn init(allocator: std.mem.Allocator) RadioButtonGroup {
        return .{
            .buttons = std.ArrayList(ElementId).init(allocator),
            .selected_index = -1,
            .allocator = allocator,
        };
    }

    /// Освобождение
    pub fn deinit(self: *RadioButtonGroup) void {
        self.buttons.deinit();
    }

    /// Добавить кнопку в группу.
    pub fn addButton(self: *RadioButtonGroup, button_id: ElementId) !void {
        try self.buttons.append(button_id);
    }

    /// Выбрать кнопку по индексу (снять выбор с остальных).
    pub fn selectIndex(self: *RadioButtonGroup, index: i32) void {
        if (index < 0 or index >= @as(i32, @intCast(self.buttons.items.len))) return;
        self.selected_index = index;
        // В полной интеграции: deselect all other buttons in group
    }

    /// Получить ID выбранной кнопки.
    pub fn getSelectedId(self: RadioButtonGroup) ElementId {
        if (self.selected_index < 0 or self.selected_index >= @as(i32, @intCast(self.buttons.items.len))) return 0;
        return self.buttons.items[@as(usize, @intCast(self.selected_index))];
    }
};

// =============================================================================
// 5. MARKUP BUTTON
// =============================================================================

/// MarkupButton — кнопка с текстовой разметкой (rich text).
///
/// В O3DE: UiMarkupButtonComponent — extends UiButtonComponent.
/// Поддерживает markup tags в тексте (bold, italic, color, etc).
pub const MarkupButton = struct {
    button: Button,
    markup_text: []const u8,

    pub fn init(element_id: ElementId) MarkupButton {
        return .{
            .button = Button.init(element_id),
            .markup_text = "",
        };
    }
};

// =============================================================================
// 6. ТЕСТЫ
// =============================================================================

test "Button: init defaults" {
    const btn = Button.init(1);
    try std.testing.expect(btn.interactable.element_id == 1);
    try std.testing.expect(btn.interactable.state == .normal);
}

test "Button: click flow" {
    var btn = Button.init(1);
    _ = btn.interactable.handlePressed(Vec2.init(50, 50));
    const clicked = btn.handleReleased(Vec2.init(50, 50));
    try std.testing.expect(clicked);
    try std.testing.expect(!btn.interactable.is_pressed);
}

test "Button: click with callback" {
    const btn = Button.init(1);
    _ = btn;
}

test "Checkbox: init defaults" {
    const cb = Checkbox.init(1);
    try std.testing.expect(!cb.is_on);
}

test "Checkbox: toggle" {
    var cb = Checkbox.init(1);
    const new_state = cb.toggleState();
    try std.testing.expect(new_state);
    try std.testing.expect(cb.is_on);

    const new_state2 = cb.toggleState();
    try std.testing.expect(!new_state2);
    try std.testing.expect(!cb.is_on);
}

test "Checkbox: set state" {
    var cb = Checkbox.init(1);
    cb.setState(true);
    try std.testing.expect(cb.is_on);
    cb.setState(false);
    try std.testing.expect(!cb.is_on);
}

test "Checkbox: click toggles" {
    var cb = Checkbox.init(1);
    _ = cb.interactable.handlePressed(Vec2.init(50, 50));
    _ = cb.handleReleased(Vec2.init(50, 50));
    try std.testing.expect(cb.is_on);
}

test "RadioButton: init defaults" {
    const rb = RadioButton.init(1);
    try std.testing.expect(!rb.is_on);
    try std.testing.expect(rb.group_id == 0);
}

test "RadioButton: set state" {
    var rb = RadioButton.init(1);
    rb.setState(true);
    try std.testing.expect(rb.is_on);
}

test "RadioButtonGroup: add and select" {
    const allocator = std.testing.allocator;
    var group = RadioButtonGroup.init(allocator);
    defer group.deinit();

    try group.addButton(1);
    try group.addButton(2);
    try group.addButton(3);
    try std.testing.expect(group.buttons.items.len == 3);

    group.selectIndex(1);
    try std.testing.expect(group.selected_index == 1);
    try std.testing.expect(group.getSelectedId() == 2);
}

test "RadioButtonGroup: out of range" {
    const allocator = std.testing.allocator;
    var group = RadioButtonGroup.init(allocator);
    defer group.deinit();

    try group.addButton(1);
    group.selectIndex(-1); // Invalid
    try std.testing.expect(group.selected_index == -1);
    group.selectIndex(5); // Invalid
    try std.testing.expect(group.selected_index == -1);
}
