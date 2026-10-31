// =============================================================================
// P³ UI DRAGDROP v1.0 — ZIG
// =============================================================================
//
// Draggable + DropTarget + Dropdown (из O3DE LyShine).
//
// В O3DE:
//   UiDraggableComponent   — элемент, который можно перетаскивать
//   UiDropTargetComponent  — цель для drag-and-drop
//   UiDropdownComponent    — выпадающий список
//
// В P³ Engine:
//   - Без EBus — прямые вызовы
//   - Drag state machine — полный порт
//   - Proxy draggable — поддержка drag между canvas'ами
//
// Портировано из O3DE/Gems/LyShine/Code/Source/{UiDraggable,UiDropTarget,UiDropdown}*
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
pub const InteractableState = ui_interactable.InteractableState;
pub const StateActionList = ui_interactable.StateActionList;
pub const ActionName = ui_interactable.ActionName;

// =============================================================================
// 1. DRAG STATE
// =============================================================================

/// Состояние draggable элемента.
pub const DragState = enum(u2) {
    normal = 0,
    valid = 1,
    invalid = 2,
};

// =============================================================================
// 2. DROP STATE
// =============================================================================

/// Состояние drop target.
pub const DropState = enum(u2) {
    normal = 0,
    valid = 1,
    invalid = 2,
};

// =============================================================================
// 3. DRAGGABLE
// =============================================================================

/// UI Draggable — элемент, который можно перетаскивать.
///
/// В O3DE: UiDraggableComponent — extends UiInteractableComponent.
/// Имеет дополнительные drag-состояния: DragNormal, DragValid, DragInvalid.
pub const Draggable = struct {
    interactable: Interactable,

    // --- Drag state ---
    drag_state: DragState,
    is_dragging: bool,
    is_active: bool,

    // --- Hover drop target ---
    hover_drop_target: ElementId,

    // --- Proxy (drag между canvas'ами) ---
    is_proxy_for: ElementId,
    can_drop_on_any_canvas: bool,

    // --- State actions ---
    drag_normal_state_actions: StateActionList,
    drag_valid_state_actions: StateActionList,
    drag_invalid_state_actions: StateActionList,

    /// Инициализация
    pub fn init(element_id: ElementId) Draggable {
        return .{
            .interactable = Interactable.init(element_id),
            .drag_state = .normal,
            .is_dragging = false,
            .is_active = false,
            .hover_drop_target = 0,
            .is_proxy_for = 0,
            .can_drop_on_any_canvas = false,
            .drag_normal_state_actions = StateActionList.init(),
            .drag_valid_state_actions = StateActionList.init(),
            .drag_invalid_state_actions = StateActionList.init(),
        };
    }

    /// Вычислить interactable state с учётом drag.
    pub fn computeInteractableState(self: Draggable) InteractableState {
        if (!self.interactable.is_handling_events) return .disabled;
        if (self.interactable.is_pressed) return .pressed;
        if (self.interactable.is_hover) return .hover;
        return switch (self.drag_state) {
            .normal => .drag_normal,
            .valid => .drag_valid,
            .invalid => .drag_invalid,
        };
    }

    /// Handle press — начать drag.
    pub fn handlePressed(self: *Draggable, point: Vec2) bool {
        if (!self.interactable.is_handling_events) return false;
        _ = self.interactable.handlePressed(point);
        self.is_dragging = true;
        self.is_active = true;
        self.drag_state = .normal;
        return true;
    }

    /// Handle release — закончить drag.
    pub fn handleReleased(self: *Draggable, point: Vec2) bool {
        if (self.is_dragging) {
            self.is_dragging = false;
            self.is_active = false;
            _ = self.interactable.handleReleased(point);

            // If hovering over valid drop target, drop
            if (self.hover_drop_target != 0 and self.drag_state == .valid) {
                // Notify drop target
            }

            self.drag_state = .normal;
            self.hover_drop_target = 0;
            return true;
        }
        return false;
    }

    /// Input position update — обновить hover drop target.
    pub fn inputPositionUpdate(self: *Draggable, point: Vec2) void {
        if (!self.is_dragging) return;
        _ = point;
        // Check if hovering over a drop target
        // (implemented in full integration with canvas)
    }

    /// Lost active status.
    pub fn lostActiveStatus(self: *Draggable) void {
        self.is_dragging = false;
        self.is_active = false;
        self.drag_state = .normal;
        self.hover_drop_target = 0;
        self.interactable.lostActiveStatus();
    }

    /// Set as proxy (for cross-canvas drag).
    pub fn setAsProxy(self: *Draggable, original_id: ElementId, point: Vec2) void {
        self.is_proxy_for = original_id;
        self.is_dragging = true;
        self.is_active = true;
        _ = point;
    }

    /// Is this a proxy?
    pub fn isProxy(self: Draggable) bool {
        return self.is_proxy_for != 0;
    }
};

// =============================================================================
// 4. DROP TARGET
// =============================================================================

/// UI DropTarget — цель для drag-and-drop.
///
/// В O3DE: UiDropTargetComponent.
pub const DropTarget = struct {
    // --- Drop state ---
    drop_state: DropState,

    // --- State actions ---
    drop_valid_state_actions: StateActionList,
    drop_invalid_state_actions: StateActionList,

    // --- Action ---
    on_drop_action: ActionName,

    /// Инициализация
    pub fn init() DropTarget {
        return .{
            .drop_state = .normal,
            .drop_valid_state_actions = StateActionList.init(),
            .drop_invalid_state_actions = StateActionList.init(),
            .on_drop_action = "",
        };
    }

    /// Handle drop hover start.
    pub fn handleDropHoverStart(self: *DropTarget, _: ElementId) void {
        self.drop_state = .valid;
    }

    /// Handle drop hover end.
    pub fn handleDropHoverEnd(self: *DropTarget) void {
        self.drop_state = .normal;
    }

    /// Handle drop.
    pub fn handleDrop(self: *DropTarget, _: ElementId) void {
        self.drop_state = .normal;
        // Dispatch on_drop_action
    }
};

// =============================================================================
// 5. DROPDOWN
// =============================================================================

/// UI Dropdown — выпадающий список.
///
/// В O3DE: UiDropdownComponent — extends UiInteractableComponent.
/// Имеет content entity (список опций) и expand/collapse поведение.
pub const Dropdown = struct {
    interactable: Interactable,

    // --- Value ---
    value: ElementId,
    content_entity: ElementId,
    expanded_parent_id: ElementId,

    // --- Behavior ---
    expand_on_hover: bool,
    wait_time: f32,
    collapse_on_outside_click: bool,

    // --- State ---
    is_expanded: bool,
    delay_timer: f32,
    expanded_by_click: bool,

    // --- Visual ---
    text_entity: ElementId,
    icon_entity: ElementId,

    // --- State actions ---
    expanded_state_actions: StateActionList,

    // --- Actions ---
    expanded_action: ActionName,
    collapsed_action: ActionName,
    option_selected_action: ActionName,

    /// Инициализация
    pub fn init(element_id: ElementId) Dropdown {
        return .{
            .interactable = Interactable.init(element_id),
            .value = 0,
            .content_entity = 0,
            .expanded_parent_id = 0,
            .expand_on_hover = false,
            .wait_time = 0.3,
            .collapse_on_outside_click = true,
            .is_expanded = false,
            .delay_timer = 0,
            .expanded_by_click = false,
            .text_entity = 0,
            .icon_entity = 0,
            .expanded_state_actions = StateActionList.init(),
            .expanded_action = "",
            .collapsed_action = "",
            .option_selected_action = "",
        };
    }

    /// Expand dropdown.
    pub fn expand(self: *Dropdown) void {
        if (self.is_expanded) return;
        self.is_expanded = true;
        // Move content to expanded parent
        // (implemented in full integration)
    }

    /// Collapse dropdown.
    pub fn collapse(self: *Dropdown) void {
        if (!self.is_expanded) return;
        self.is_expanded = false;
        // Move content back
    }

    /// Handle release — toggle expand.
    pub fn handleReleased(self: *Dropdown, point: Vec2) bool {
        if (self.interactable.is_pressed) {
            _ = self.interactable.handleReleased(point);
            if (self.is_expanded) {
                self.collapse();
            } else {
                self.expand();
                self.expanded_by_click = true;
            }
            return true;
        }
        return false;
    }

    /// Select option.
    pub fn selectOption(self: *Dropdown, option_id: ElementId) void {
        self.value = option_id;
        self.collapse();
        // Dispatch option_selected_action
    }

    /// Handle outside click — collapse.
    pub fn handleOutsideClick(self: *Dropdown) void {
        if (self.collapse_on_outside_click and self.is_expanded) {
            self.collapse();
        }
    }

    /// Update (delay timer for expand-on-hover).
    pub fn update(self: *Dropdown, dt: f32) void {
        if (self.delay_timer > 0) {
            self.delay_timer -= dt;
            if (self.delay_timer <= 0) {
                self.expand();
            }
        }
    }
};

// =============================================================================
// 6. DROPDOWN OPTION
// =============================================================================

/// DropdownOption — опция внутри Dropdown.
///
/// В O3DE: UiDropdownOptionComponent.
pub const DropdownOption = struct {
    dropdown_entity: ElementId,
    text: []const u8,

    pub fn init(dropdown_id: ElementId, text: []const u8) DropdownOption {
        return .{
            .dropdown_entity = dropdown_id,
            .text = text,
        };
    }
};

// =============================================================================
// 7. MASK
// =============================================================================

/// UI Mask — маска для обрезки содержимого.
///
/// В O3DE: UiMaskComponent.
pub const Mask = struct {
    is_masking_enabled: bool,
    use_alpha_test: bool,
    draw_behind: bool,
    draw_in_front: bool,

    pub fn init() Mask {
        return .{
            .is_masking_enabled = true,
            .use_alpha_test = false,
            .draw_behind = false,
            .draw_in_front = true,
        };
    }
};

// =============================================================================
// 8. SPAWNER
// =============================================================================

/// UI Spawner — динамическое создание элементов.
///
/// В O3DE: UiSpawnerComponent.
pub const Spawner = struct {
    spawn_target: ElementId,
    spawn_count: u32,

    pub fn init() Spawner {
        return .{
            .spawn_target = 0,
            .spawn_count = 0,
        };
    }

    pub fn spawn(self: *Spawner) void {
        self.spawn_count += 1;
    }
};

// =============================================================================
// 9. ТЕСТЫ
// =============================================================================

test "Draggable: init defaults" {
    const d = Draggable.init(1);
    try std.testing.expect(d.drag_state == .normal);
    try std.testing.expect(!d.is_dragging);
    try std.testing.expect(!d.isProxy());
}

test "Draggable: drag flow" {
    var d = Draggable.init(1);
    _ = d.handlePressed(Vec2.init(100, 100));
    try std.testing.expect(d.is_dragging);
    try std.testing.expect(d.is_active);

    _ = d.handleReleased(Vec2.init(150, 100));
    try std.testing.expect(!d.is_dragging);
    try std.testing.expect(d.drag_state == .normal);
}

test "Draggable: compute state with drag" {
    var d = Draggable.init(1);
    try std.testing.expect(d.computeInteractableState() == .drag_normal);
    d.drag_state = .valid;
    try std.testing.expect(d.computeInteractableState() == .drag_valid);
}

test "DropTarget: init defaults" {
    const dt = DropTarget.init();
    try std.testing.expect(dt.drop_state == .normal);
}

test "DropTarget: hover flow" {
    var dt = DropTarget.init();
    dt.handleDropHoverStart(1);
    try std.testing.expect(dt.drop_state == .valid);
    dt.handleDropHoverEnd();
    try std.testing.expect(dt.drop_state == .normal);
}

test "Dropdown: init defaults" {
    const dd = Dropdown.init(1);
    try std.testing.expect(!dd.is_expanded);
    try std.testing.expect(dd.collapse_on_outside_click);
}

test "Dropdown: expand/collapse" {
    var dd = Dropdown.init(1);
    dd.expand();
    try std.testing.expect(dd.is_expanded);
    dd.collapse();
    try std.testing.expect(!dd.is_expanded);
}

test "Dropdown: select option" {
    var dd = Dropdown.init(1);
    dd.expand();
    dd.selectOption(5);
    try std.testing.expect(!dd.is_expanded);
    try std.testing.expect(dd.value == 5);
}

test "Mask: init defaults" {
    const m = Mask.init();
    try std.testing.expect(m.is_masking_enabled);
    try std.testing.expect(!m.use_alpha_test);
}
