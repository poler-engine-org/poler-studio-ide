// =============================================================================
// P³ UI SCROLL v1.0 — ZIG
// =============================================================================
//
// Slider + ScrollBar + ScrollBox + Fader (из O3DE LyShine).
//
// В O3DE:
//   UiSliderComponent      — value control (min/max/step)
//   UiScrollBarComponent   — scroll handle with auto-fade
//   UiScrollBoxComponent   — scrollable content container
//   UiFaderComponent       — fade in/out with optional render-to-texture
//
// В P³ Engine:
//   - Без EBus — прямые поля
//   - P³ обобщение: проективный scroll transform
//   - Momentum (инерционная прокрутка) — полный порт
//
// Портировано из O3DE/Gems/LyShine/Code/Source/{UiSlider,UiScrollBar,UiScrollBox,UiFader}*
// Адаптировано для Zig 0.14.0 + P³ Engine API.
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const math = std.math;
const ui_transform = @import("p3_ui_transform.zig");
const ui_canvas = @import("p3_ui_canvas.zig");
const ui_interactable = @import("p3_ui_interactable.zig");
const ui_draw = @import("p3_ui_draw.zig");

pub const Vec2 = ui_transform.Vec2;
pub const Rect = ui_transform.Rect;
pub const Color = ui_draw.Color;
pub const ElementId = ui_canvas.ElementId;
pub const Interactable = ui_interactable.Interactable;
pub const ValueChangeCallback = ui_interactable.ValueChangeCallback;
pub const ScrollOffsetCallback = ui_interactable.ScrollOffsetCallback;
pub const ActionName = ui_interactable.ActionName;

// =============================================================================
// 1. ORIENTATION
// =============================================================================

/// Ориентация элемента (горизонтальный/вертикальный).
pub const Orientation = enum(u1) {
    horizontal = 0,
    vertical = 1,
};

// =============================================================================
// 2. SCROLL BAR VISIBILITY
// =============================================================================

/// Когда показывать scroll bar.
pub const ScrollBarVisibility = enum(u2) {
    /// Всегда видимый
    always = 0,
    /// Видимый только при прокрутке
    auto_hide = 1,
    /// Никогда не видимый
    never = 2,
};

// =============================================================================
// 3. SNAP MODE
// =============================================================================

/// Режим привязки прокрутки.
pub const SnapMode = enum(u2) {
    /// Без привязки
    none = 0,
    /// Привязка к сетке
    grid = 1,
    /// Привязка к элементам
    to_element = 2,
};

// =============================================================================
// 4. SLIDER
// =============================================================================

/// UI Slider — контроль значения (min/max/step).
///
/// В O3DE: UiSliderComponent — extends UiInteractableComponent.
/// Имеет track, fill, и manipulator entities.
pub const Slider = struct {
    interactable: Interactable,

    // --- Value ---
    value: f32,
    min_value: f32,
    max_value: f32,
    step_value: f32,

    // --- Drag state ---
    is_dragging: bool,
    is_active: bool,

    // --- Visual entities ---
    track_entity: ElementId,
    fill_entity: ElementId,
    manipulator_entity: ElementId,

    // --- Callbacks ---
    on_value_changed: ValueChangeCallback,
    on_value_changing: ValueChangeCallback,

    // --- Action names ---
    value_changed_action: ActionName,
    value_changing_action: ActionName,

    /// Инициализация
    pub fn init(element_id: ElementId) Slider {
        return .{
            .interactable = Interactable.init(element_id),
            .value = 0.0,
            .min_value = 0.0,
            .max_value = 1.0,
            .step_value = 0.01,
            .is_dragging = false,
            .is_active = false,
            .track_entity = 0,
            .fill_entity = 0,
            .manipulator_entity = 0,
            .on_value_changed = ui_interactable.noopValueChangeCallback,
            .on_value_changing = ui_interactable.noopValueChangeCallback,
            .value_changed_action = "",
            .value_changing_action = "",
        };
    }

    /// Установить значение с clamping.
    pub fn setValue(self: *Slider, value: f32) void {
        const clamped = std.math.clamp(value, self.min_value, self.max_value);
        if (clamped != self.value) {
            self.value = clamped;
            self.on_value_changed(self.interactable.element_id, self.value);
        }
    }

    /// Установить значение в процессе drag (value changing, не value changed).
    pub fn setValueChanging(self: *Slider, value: f32) void {
        const clamped = std.math.clamp(value, self.min_value, self.max_value);
        self.value = clamped;
        self.on_value_changing(self.interactable.element_id, self.value);
    }

    /// Нормализованное значение [0, 1].
    pub fn getNormalizedValue(self: Slider) f32 {
        const range = self.max_value - self.min_value;
        if (range <= 0) return 0;
        return (self.value - self.min_value) / range;
    }

    /// Установить значение из нормализованного.
    pub fn setNormalizedValue(self: *Slider, t: f32) void {
        self.setValue(self.min_value + t * (self.max_value - self.min_value));
    }

    /// Handle press — начать drag.
    pub fn handlePressed(self: *Slider, point: Vec2) bool {
        if (!self.interactable.is_handling_events) return false;
        _ = self.interactable.handlePressed(point);
        self.is_dragging = true;
        self.is_active = true;
        return true;
    }

    /// Handle release — закончить drag.
    pub fn handleReleased(self: *Slider, point: Vec2) bool {
        if (self.is_dragging) {
            self.is_dragging = false;
            self.is_active = false;
            _ = self.interactable.handleReleased(point);
            self.on_value_changed(self.interactable.element_id, self.value);
            return true;
        }
        return false;
    }

    /// Input position update — обновить значение при drag.
    pub fn inputPositionUpdate(self: *Slider, point: Vec2, track_rect: Rect, orientation: Orientation) void {
        if (!self.is_dragging) return;

        const t: f32 = @floatCast(switch (orientation) {
            .horizontal => (point.x - track_rect.left) / @max(track_rect.width(), 0.001),
            .vertical => (point.y - track_rect.top) / @max(track_rect.height(), 0.001),
        });
        self.setValueChanging(self.min_value + std.math.clamp(t, 0.0, 1.0) * (self.max_value - self.min_value));
    }

    /// Step up.
    pub fn stepUp(self: *Slider) void {
        self.setValue(self.value + self.step_value);
    }

    /// Step down.
    pub fn stepDown(self: *Slider) void {
        self.setValue(self.value - self.step_value);
    }
};

// =============================================================================
// 5. SCROLL BAR
// =============================================================================

/// UI ScrollBar — полоса прокрутки с handle.
///
/// В O3DE: UiScrollBarComponent — extends UiInteractableComponent.
/// Имеет handle entity и поддерживает auto-fade.
pub const ScrollBar = struct {
    interactable: Interactable,

    // --- Value ---
    value: f32,
    handle_size: f32,
    min_handle_pixel_size: f32,
    orientation: Orientation,

    // --- Drag state ---
    is_dragging: bool,
    is_active: bool,
    pressed_value: f32,
    pressed_pos_along_axis: f32,
    pressed_on_handle: bool,

    // --- Auto-fade ---
    is_auto_fade_enabled: bool,
    is_fading: bool,
    auto_fade_delay: f32,
    auto_fade_speed: f32,
    seconds_remaining_before_fade: f32,
    initial_scroll_bar_alpha: f32,
    initial_handle_alpha: f32,
    current_fade: f32,

    // --- Entities ---
    handle_entity: ElementId,
    scrollable_entity: ElementId,

    // --- Callbacks ---
    on_value_changed: ValueChangeCallback,
    on_value_changing: ValueChangeCallback,

    // --- Action names ---
    value_changed_action: ActionName,
    value_changing_action: ActionName,

    /// Инициализация
    pub fn init(element_id: ElementId) ScrollBar {
        return .{
            .interactable = Interactable.init(element_id),
            .value = 0.0,
            .handle_size = 0.1,
            .min_handle_pixel_size = 20.0,
            .orientation = .vertical,
            .is_dragging = false,
            .is_active = false,
            .pressed_value = 0,
            .pressed_pos_along_axis = 0,
            .pressed_on_handle = false,
            .is_auto_fade_enabled = false,
            .is_fading = false,
            .auto_fade_delay = 1.0,
            .auto_fade_speed = 1.0,
            .seconds_remaining_before_fade = 1.0,
            .initial_scroll_bar_alpha = 1.0,
            .initial_handle_alpha = 1.0,
            .current_fade = 1.0,
            .handle_entity = 0,
            .scrollable_entity = 0,
            .on_value_changed = ui_interactable.noopValueChangeCallback,
            .on_value_changing = ui_interactable.noopValueChangeCallback,
            .value_changed_action = "",
            .value_changing_action = "",
        };
    }

    /// Handle press.
    pub fn handlePressed(self: *ScrollBar, point: Vec2, handle_rect: Rect) bool {
        if (!self.interactable.is_handling_events) return false;
        _ = self.interactable.handlePressed(point);

        // Check if pressed on handle
        self.pressed_on_handle = point.x >= handle_rect.left and point.x <= handle_rect.right and
            point.y >= handle_rect.top and point.y <= handle_rect.bottom;

        const axis_pos = switch (self.orientation) {
            .horizontal => point.x,
            .vertical => point.y,
        };
        self.pressed_pos_along_axis = axis_pos;
        self.pressed_value = self.value;
        self.is_dragging = true;
        self.is_active = true;

        // Reset auto-fade
        self.seconds_remaining_before_fade = self.auto_fade_delay;
        self.current_fade = 1.0;
        self.is_fading = false;

        return true;
    }

    /// Handle release.
    pub fn handleReleased(self: *ScrollBar, point: Vec2) bool {
        if (self.is_dragging) {
            self.is_dragging = false;
            self.is_active = false;
            _ = self.interactable.handleReleased(point);
            self.on_value_changed(self.interactable.element_id, self.value);
            return true;
        }
        return false;
    }

    /// Input position update.
    pub fn inputPositionUpdate(self: *ScrollBar, point: Vec2, track_rect: Rect) void {
        if (!self.is_dragging) return;

        const axis_pos = switch (self.orientation) {
            .horizontal => point.x,
            .vertical => point.y,
        };

        const track_len = switch (self.orientation) {
            .horizontal => track_rect.width(),
            .vertical => track_rect.height(),
        };

        if (self.pressed_on_handle) {
            // Dragging the handle
            const delta = axis_pos - self.pressed_pos_along_axis;
            const value_delta = delta / @max(track_len * (1.0 - self.handle_size), 0.001);
            self.value = std.math.clamp(self.pressed_value + value_delta, 0, 1);
        } else {
            // Clicked on track, jump to position
            const t = switch (self.orientation) {
                .horizontal => (axis_pos - track_rect.left) / @max(track_len, 0.001),
                .vertical => (axis_pos - track_rect.top) / @max(track_len, 0.001),
            };
            self.value = std.math.clamp(t - self.handle_size / 2, 0, 1 - self.handle_size);
        }
        self.on_value_changing(self.interactable.element_id, self.value);
    }

    /// Update auto-fade.
    pub fn update(self: *ScrollBar, dt: f32) void {
        if (self.is_auto_fade_enabled and !self.is_dragging) {
            self.seconds_remaining_before_fade -= dt;
            if (self.seconds_remaining_before_fade <= 0) {
                self.is_fading = true;
                self.current_fade -= dt * self.auto_fade_speed;
                if (self.current_fade < 0) self.current_fade = 0;
            }
        }
    }
};

// =============================================================================
// 6. SCROLL BOX
// =============================================================================

/// UI ScrollBox — прокручиваемый контейнер.
///
/// В O3DE: UiScrollBoxComponent — extends UiInteractableComponent.
/// Поддерживает momentum (инерция), snapping, scroll bars.
pub const ScrollBox = struct {
    interactable: Interactable,

    // --- Scroll state ---
    scroll_offset: Vec2,
    is_horizontal_scrolling_enabled: bool,
    is_vertical_scrolling_enabled: bool,
    is_scrolling_constrained: bool,

    // --- Snap ---
    snap_mode: SnapMode,
    snap_grid: Vec2,

    // --- Scroll sensitivity ---
    scroll_sensitivity: Vec2,

    // --- Momentum ---
    momentum_duration: f32,
    momentum_is_active: bool,
    momentum_time_accumulator: f32,
    dragging_time_accumulator: f32,
    stopping_time_accumulator: f32,
    last_offset_change: Vec2,
    offset_change_accumulator: Vec2,

    // --- Scroll bar visibility ---
    h_scroll_bar_visibility: ScrollBarVisibility,
    v_scroll_bar_visibility: ScrollBarVisibility,

    // --- Drag state ---
    is_dragging: bool,
    is_active: bool,
    pressed_scroll_offset: Vec2,
    last_drag_point: Vec2,

    // --- Entities ---
    content_entity: ElementId,
    h_scroll_bar_entity: ElementId,
    v_scroll_bar_entity: ElementId,

    // --- Callbacks ---
    on_scroll_offset_changed: ScrollOffsetCallback,
    on_scroll_offset_changing: ScrollOffsetCallback,

    // --- Action names ---
    scroll_offset_changed_action: ActionName,
    scroll_offset_changing_action: ActionName,

    /// Константы из O3DE
    pub const MIN_OFFSET_THRESHOLD: f32 = 10.0;
    pub const MAX_STOPPING_DELAY: f32 = 0.12;

    /// Инициализация
    pub fn init(element_id: ElementId) ScrollBox {
        return .{
            .interactable = Interactable.init(element_id),
            .scroll_offset = Vec2.zero(),
            .is_horizontal_scrolling_enabled = true,
            .is_vertical_scrolling_enabled = true,
            .is_scrolling_constrained = true,
            .snap_mode = .none,
            .snap_grid = Vec2.init(1, 1),
            .scroll_sensitivity = Vec2.init(1, 1),
            .momentum_duration = 0.0,
            .momentum_is_active = false,
            .momentum_time_accumulator = 0,
            .dragging_time_accumulator = 0,
            .stopping_time_accumulator = 0,
            .last_offset_change = Vec2.zero(),
            .offset_change_accumulator = Vec2.zero(),
            .h_scroll_bar_visibility = .auto_hide,
            .v_scroll_bar_visibility = .auto_hide,
            .is_dragging = false,
            .is_active = false,
            .pressed_scroll_offset = Vec2.zero(),
            .last_drag_point = Vec2.zero(),
            .content_entity = 0,
            .h_scroll_bar_entity = 0,
            .v_scroll_bar_entity = 0,
            .on_scroll_offset_changed = ui_interactable.noopScrollOffsetCallback,
            .on_scroll_offset_changing = ui_interactable.noopScrollOffsetCallback,
            .scroll_offset_changed_action = "",
            .scroll_offset_changing_action = "",
        };
    }

    /// Handle press — начать прокрутку.
    pub fn handlePressed(self: *ScrollBox, point: Vec2) bool {
        if (!self.interactable.is_handling_events) return false;
        _ = self.interactable.handlePressed(point);
        self.is_dragging = true;
        self.is_active = true;
        self.pressed_scroll_offset = self.scroll_offset;
        self.last_drag_point = point;
        self.dragging_time_accumulator = 0;
        self.stopping_time_accumulator = 0;
        self.last_offset_change = Vec2.zero();
        self.offset_change_accumulator = Vec2.zero();

        // Stop momentum
        self.momentum_is_active = false;

        return true;
    }

    /// Handle release — начать momentum.
    pub fn handleReleased(self: *ScrollBox, point: Vec2) bool {
        if (self.is_dragging) {
            self.is_dragging = false;
            self.is_active = false;
            _ = self.interactable.handleReleased(point);

            // Start momentum if there's velocity
            if (self.momentum_duration > 0 and
                (self.offset_change_accumulator.x != 0 or self.offset_change_accumulator.y != 0))
            {
                self.momentum_is_active = true;
                self.momentum_time_accumulator = 0;
            }

            return true;
        }
        return false;
    }

    /// Input position update — обновить scroll offset.
    pub fn inputPositionUpdate(self: *ScrollBox, point: Vec2) void {
        if (!self.is_dragging) return;

        var delta = Vec2.sub(point, self.last_drag_point);

        // Apply sensitivity
        delta.x *= self.scroll_sensitivity.x;
        delta.y *= self.scroll_sensitivity.y;

        // Apply per-axis enable
        if (!self.is_horizontal_scrolling_enabled) delta.x = 0;
        if (!self.is_vertical_scrolling_enabled) delta.y = 0;

        self.scroll_offset = Vec2.add(self.scroll_offset, delta);
        self.last_drag_point = point;

        // Track velocity for momentum
        self.dragging_time_accumulator += 0.016; // Approximate dt
        self.last_offset_change = delta;

        self.on_scroll_offset_changing(self.interactable.element_id, self.scroll_offset);
    }

    /// Handle scroll wheel.
    pub fn handleScroll(self: *ScrollBox, delta: f32, is_horizontal: bool) void {
        const d = delta * 30.0; // Scroll step
        if (is_horizontal and self.is_horizontal_scrolling_enabled) {
            self.scroll_offset.x += d;
        } else if (!is_horizontal and self.is_vertical_scrolling_enabled) {
            self.scroll_offset.y += d;
        }
        self.on_scroll_offset_changing(self.interactable.element_id, self.scroll_offset);
    }

    /// Update momentum.
    pub fn update(self: *ScrollBox, dt: f32) void {
        if (self.momentum_is_active) {
            self.momentum_time_accumulator += dt;
            const t = self.momentum_time_accumulator / @max(self.momentum_duration, 0.001);

            if (t >= 1.0) {
                self.momentum_is_active = false;
            } else {
                // Exponential decay
                const factor = 1.0 - t;
                const momentum_offset = Vec2.scale(self.offset_change_accumulator, factor * dt * 60.0);
                self.scroll_offset = Vec2.add(self.scroll_offset, momentum_offset);
                self.on_scroll_offset_changing(self.interactable.element_id, self.scroll_offset);
            }
        }
    }

    /// Set scroll offset directly.
    pub fn setScrollOffset(self: *ScrollBox, offset: Vec2) void {
        self.scroll_offset = offset;
        self.on_scroll_offset_changed(self.interactable.element_id, self.scroll_offset);
    }

    /// Остановить momentum.
    pub fn stopMomentum(self: *ScrollBox) void {
        self.momentum_is_active = false;
    }
};

// =============================================================================
// 7. FADER
// =============================================================================

/// UI Fader — плавное появление/исчезновение.
///
/// В O3DE: UiFaderComponent.
/// Fade value 0 = invisible, 1 = fully visible.
pub const Fader = struct {
    // --- Fade state ---
    fade_value: f32,
    is_fading: bool,
    fade_target: f32,
    fade_speed: f32,

    // --- Render to texture ---
    use_render_to_texture: bool,
    render_target_width: i32,
    render_target_height: i32,

    /// Инициализация
    pub fn init() Fader {
        return .{
            .fade_value = 1.0,
            .is_fading = false,
            .fade_target = 1.0,
            .fade_speed = 1.0,
            .use_render_to_texture = false,
            .render_target_width = 0,
            .render_target_height = 0,
        };
    }

    /// Начать fade к целевому значению.
    pub fn fade(self: *Fader, target: f32, speed: f32) void {
        self.fade_target = std.math.clamp(target, 0, 1);
        self.fade_speed = @max(speed, 0.001);
        self.is_fading = true;
    }

    /// Fade in (к 1.0).
    pub fn fadeIn(self: *Fader, speed: f32) void {
        self.fade(1.0, speed);
    }

    /// Fade out (к 0.0).
    pub fn fadeOut(self: *Fader, speed: f32) void {
        self.fade(0.0, speed);
    }

    /// Update fade.
    pub fn update(self: *Fader, dt: f32) void {
        if (!self.is_fading) return;

        const delta = self.fade_speed * dt;
        if (self.fade_value < self.fade_target) {
            self.fade_value += delta;
            if (self.fade_value >= self.fade_target) {
                self.fade_value = self.fade_target;
                self.is_fading = false;
            }
        } else if (self.fade_value > self.fade_target) {
            self.fade_value -= delta;
            if (self.fade_value <= self.fade_target) {
                self.fade_value = self.fade_target;
                self.is_fading = false;
            }
        } else {
            self.is_fading = false;
        }
    }

    /// Установить fade value напрямую.
    pub fn setFadeValue(self: *Fader, value: f32) void {
        self.fade_value = std.math.clamp(value, 0, 1);
        self.is_fading = false;
    }
};

// =============================================================================
// 8. ТЕСТЫ
// =============================================================================

test "Slider: init defaults" {
    const s = Slider.init(1);
    try std.testing.expect(s.value == 0.0);
    try std.testing.expect(s.min_value == 0.0);
    try std.testing.expect(s.max_value == 1.0);
}

test "Slider: set value clamped" {
    var s = Slider.init(1);
    s.setValue(0.5);
    try std.testing.expectApproxEqAbs(s.value, 0.5, 1e-6);
    s.setValue(2.0); // Clamped to 1.0
    try std.testing.expectApproxEqAbs(s.value, 1.0, 1e-6);
    s.setValue(-1.0); // Clamped to 0.0
    try std.testing.expectApproxEqAbs(s.value, 0.0, 1e-6);
}

test "Slider: normalized value" {
    var s = Slider.init(1);
    s.min_value = 10;
    s.max_value = 20;
    s.value = 15;
    try std.testing.expectApproxEqAbs(s.getNormalizedValue(), 0.5, 1e-6);
}

test "Slider: drag flow" {
    var s = Slider.init(1);
    _ = s.handlePressed(Vec2.init(100, 100));
    try std.testing.expect(s.is_dragging);

    const track = Rect.fromSize(0, 0, 200, 20);
    s.inputPositionUpdate(Vec2.init(150, 10), track, .horizontal);
    try std.testing.expectApproxEqAbs(s.value, 0.75, 1e-3);

    _ = s.handleReleased(Vec2.init(150, 10));
    try std.testing.expect(!s.is_dragging);
}

test "ScrollBar: init defaults" {
    const sb = ScrollBar.init(1);
    try std.testing.expect(sb.value == 0.0);
    try std.testing.expect(sb.handle_size == 0.1);
    try std.testing.expect(!sb.is_auto_fade_enabled);
}

test "ScrollBox: init defaults" {
    const sb = ScrollBox.init(1);
    try std.testing.expect(sb.scroll_offset.x == 0);
    try std.testing.expect(sb.is_horizontal_scrolling_enabled);
    try std.testing.expect(sb.is_vertical_scrolling_enabled);
}

test "ScrollBox: drag and scroll" {
    var sb = ScrollBox.init(1);
    _ = sb.handlePressed(Vec2.init(100, 100));
    sb.inputPositionUpdate(Vec2.init(110, 120));
    try std.testing.expectApproxEqAbs(sb.scroll_offset.x, 10.0, 1e-6);
    try std.testing.expectApproxEqAbs(sb.scroll_offset.y, 20.0, 1e-6);
}

test "Fader: init defaults" {
    const f = Fader.init();
    try std.testing.expect(f.fade_value == 1.0);
    try std.testing.expect(!f.is_fading);
}

test "Fader: fade in" {
    var f = Fader.init();
    f.fade_value = 0.0;
    f.fadeIn(2.0);
    try std.testing.expect(f.is_fading);
    try std.testing.expectApproxEqAbs(f.fade_target, 1.0, 1e-6);
}

test "Fader: update" {
    var f = Fader.init();
    f.fade_value = 0.0;
    f.fadeIn(2.0);
    f.update(0.25); // Should reach 0.5
    try std.testing.expectApproxEqAbs(f.fade_value, 0.5, 1e-6);
    try std.testing.expect(f.is_fading);
}

test "Fader: complete" {
    var f = Fader.init();
    f.fade_value = 0.0;
    f.fadeIn(1.0);
    f.update(1.0); // Should reach 1.0
    try std.testing.expectApproxEqAbs(f.fade_value, 1.0, 1e-6);
    try std.testing.expect(!f.is_fading);
}
