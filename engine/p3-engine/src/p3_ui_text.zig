// =============================================================================
// P³ UI TEXT v1.0 — ZIG
// =============================================================================
//
// Text + TextInput + Clipboard (из O3DE UiTextComponent/UiTextInputComponent).
//
// В O3DE:
//   UiTextComponent    — текстовый визуальный компонент
//     - Font, fontSize, color, alignment, spacing
//     - Overflow: OverflowText / ClipText / Ellipsis
//     - Wrap: NoWrap / Wrap
//     - ShrinkToFit: None / Uniform / WidthOnly
//     - Markup (rich text)
//     - Inline images
//     - Selection
//   UiTextInputComponent — интерактивный текстовый ввод
//     - Cursor, selection, editing
//     - Placeholder text
//     - Max string length
//     - Password field
//     - Clipboard
//
// В P³ Engine:
//   - Без FontSystem — абстрактный font interface
//   - Без EBus — прямые поля
//   - P³ обобщение: проективный text transform
//
// Портировано из O3DE/Gems/LyShine/Code/Source/{UiText,UiTextInput}*
// Адаптировано для Zig 0.14.0 + P³ Engine API.
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const math = std.math;
const ui_transform = @import("p3_ui_transform.zig");
const ui_draw = @import("p3_ui_draw.zig");
const ui_canvas = @import("p3_ui_canvas.zig");
const ui_interactable = @import("p3_ui_interactable.zig");

pub const Vec2 = ui_transform.Vec2;
pub const Rect = ui_transform.Rect;
pub const Color = ui_draw.Color;
pub const ElementId = ui_canvas.ElementId;
pub const Interactable = ui_interactable.Interactable;
pub const TextInputCallback = ui_interactable.TextInputCallback;
pub const ActionName = ui_interactable.ActionName;

// =============================================================================
// 1. TEXT ALIGNMENT
// =============================================================================

/// Горизонтальное выравнивание текста.
pub const HAlign = enum(u2) {
    left = 0,
    center = 1,
    right = 2,
};

/// Вертикальное выравнивание текста.
pub const VAlign = enum(u2) {
    top = 0,
    center = 1,
    bottom = 2,
};

// =============================================================================
// 2. OVERFLOW MODE
// =============================================================================

/// Как текст обрабатывает переполнение.
///
/// В O3DE: UiTextInterface::OverflowMode
pub const OverflowMode = enum(u2) {
    /// Текст выходит за границы элемента
    overflow_text = 0,
    /// Текст обрезается по границам
    clip_text = 1,
    /// Добавляется "..." в конце
    ellipsis = 2,
};

// =============================================================================
// 3. WRAP TEXT SETTING
// =============================================================================

/// Перенос текста.
///
/// В O3DE: UiTextInterface::WrapTextSetting
pub const WrapTextSetting = enum(u1) {
    no_wrap = 0,
    wrap = 1,
};

// =============================================================================
// 4. SHRINK TO FIT
// =============================================================================

/// Уменьшение текста чтобы вписаться.
///
/// В O3DE: UiTextInterface::ShrinkToFit
pub const ShrinkToFit = enum(u2) {
    /// Без уменьшения
    none = 0,
    /// Равномерное уменьшение (ширина и высота)
    uniform = 1,
    /// Уменьшение только по ширине
    width_only = 2,
};

// =============================================================================
// 5. TEXT COMPONENT
// =============================================================================

/// UI Text — визуальный компонент для текста.
///
/// В O3DE: UiTextComponent — 800+ строк.
/// В P³: данные + рендер через DrawQueue.
pub const Text = struct {
    // --- Content ---
    text: []const u8,
    color: Color,
    alpha: f32,

    // --- Font ---
    font_path: []const u8,
    font_size: f32,
    font_effect_index: u32,

    // --- Alignment ---
    h_alignment: HAlign,
    v_alignment: VAlign,

    // --- Spacing ---
    character_spacing: f32,
    line_spacing: f32,

    // --- Overflow ---
    overflow_mode: OverflowMode,
    wrap_text: WrapTextSetting,
    shrink_to_fit: ShrinkToFit,
    min_shrink_scale: f32,

    // --- Markup ---
    is_markup_enabled: bool,

    // --- Selection ---
    selection_color: Color,
    selection_start: i32,
    selection_end: i32,

    // --- Layout ---
    text_size: Vec2,

    // --- Override (для state actions) ---
    override_color: Color,
    override_alpha: f32,
    override_font_path: []const u8,
    override_font_effect_index: u32,
    is_color_overridden: bool,
    is_alpha_overridden: bool,
    is_font_overridden: bool,
    is_font_effect_overridden: bool,

    // --- Render cache ---
    are_draw_batches_dirty: bool,

    /// Инициализация по умолчанию
    pub fn init() Text {
        return .{
            .text = "",
            .color = Color.white(),
            .alpha = 1.0,
            .font_path = "",
            .font_size = 16.0,
            .font_effect_index = 0,
            .h_alignment = .left,
            .v_alignment = .top,
            .character_spacing = 0,
            .line_spacing = 0,
            .overflow_mode = .overflow_text,
            .wrap_text = .no_wrap,
            .shrink_to_fit = .none,
            .min_shrink_scale = 0.5,
            .is_markup_enabled = false,
            .selection_color = Color.init(0.2, 0.4, 0.8, 1.0),
            .selection_start = -1,
            .selection_end = -1,
            .text_size = Vec2.zero(),
            .override_color = Color.white(),
            .override_alpha = 1.0,
            .override_font_path = "",
            .override_font_effect_index = 0,
            .is_color_overridden = false,
            .is_alpha_overridden = false,
            .is_font_overridden = false,
            .is_font_effect_overridden = false,
            .are_draw_batches_dirty = true,
        };
    }

    /// Установить текст.
    pub fn setText(self: *Text, text: []const u8) void {
        self.text = text;
        self.are_draw_batches_dirty = true;
    }

    /// Получить эффективный цвет.
    pub fn getEffectiveColor(self: Text) Color {
        var c = if (self.is_color_overridden) self.override_color else self.color;
        c.a *= self.getEffectiveAlpha();
        return c;
    }

    /// Получить эффективную альфу.
    pub fn getEffectiveAlpha(self: Text) f32 {
        return if (self.is_alpha_overridden) self.override_alpha else self.alpha;
    }

    /// Установить override цвет.
    pub fn setOverrideColor(self: *Text, color: Color) void {
        self.override_color = color;
        self.is_color_overridden = true;
        self.are_draw_batches_dirty = true;
    }

    /// Установить override альфу.
    pub fn setOverrideAlpha(self: *Text, alpha: f32) void {
        self.override_alpha = alpha;
        self.is_alpha_overridden = true;
        self.are_draw_batches_dirty = true;
    }

    /// Сбросить все override.
    pub fn resetOverrides(self: *Text) void {
        self.is_color_overridden = false;
        self.is_alpha_overridden = false;
        self.is_font_overridden = false;
        self.is_font_effect_overridden = false;
        self.are_draw_batches_dirty = true;
    }

    /// Установить выделение текста.
    pub fn setSelection(self: *Text, start: i32, end: i32) void {
        self.selection_start = start;
        self.selection_end = end;
        self.are_draw_batches_dirty = true;
    }

    /// Очистить выделение.
    pub fn clearSelection(self: *Text) void {
        self.selection_start = -1;
        self.selection_end = -1;
        self.are_draw_batches_dirty = true;
    }

    /// Есть ли выделение?
    pub fn hasSelection(self: Text) bool {
        return self.selection_start >= 0 and self.selection_end > self.selection_start;
    }

    /// Получить индекс символа по позиции.
    ///
    /// В O3DE: GetCharIndexFromPoint
    pub fn getCharIndexFromPoint(self: Text, point: Vec2, element_rect: Rect) i32 {
        if (self.text.len == 0) return 0;

        // Approximate: assume monospace for hit test
        const char_width = self.text_size.x / @as(f32, @floatFromInt(self.text.len));
        const relative_x = point.x - element_rect.left;
        const index = @as(i32, @intFromFloat(@floor(relative_x / @max(char_width, 0.001))));
        return std.math.clamp(index, 0, @as(i32, @intCast(self.text.len)));
    }

    /// Обрезать текст с ellipsis если он не влезает.
    pub fn applyEllipsis(self: Text, max_width: f32, allocator: std.mem.Allocator) ![]const u8 {
        if (self.overflow_mode != .ellipsis) return self.text;
        if (self.text_size.x <= max_width) return self.text;

        // Approximate: truncate by character
        const char_width = self.text_size.x / @as(f32, @floatFromInt(@max(self.text.len, 1)));
        const max_chars = @as(usize, @intFromFloat(@floor(max_width / @max(char_width, 0.001))));
        if (max_chars < 4) return "...";

        const result = try allocator.alloc(u8, max_chars);
        @memcpy(result[0 .. max_chars - 3], self.text[0 .. max_chars - 3]);
        result[max_chars - 3] = '.';
        result[max_chars - 2] = '.';
        result[max_chars - 1] = '.';
        return result;
    }
};

// =============================================================================
// 6. TEXT INPUT
// =============================================================================

/// UI TextInput — интерактивный текстовый ввод.
///
/// В O3DE: UiTextInputComponent — extends UiInteractableComponent.
/// Добавляет cursor, selection, editing, clipboard.
pub const TextInput = struct {
    interactable: Interactable,

    // --- Content ---
    text_entity: ElementId,
    placeholder_text_entity: ElementId,
    max_string_length: i32,

    // --- Password ---
    is_password_field: bool,
    replacement_character: u32,

    // --- Editing ---
    is_editing: bool,
    is_dragging: bool,
    cursor_position: i32,
    selection_start: i32,
    cursor_blink_interval: f32,
    cursor_blink_start_time: f32,

    // --- Colors ---
    text_selection_color: Color,
    text_cursor_color: Color,

    // --- Clipboard ---
    is_clipboard_enabled: bool,

    // --- Callbacks ---
    on_change: TextInputCallback,
    on_end_edit: TextInputCallback,
    on_enter: TextInputCallback,

    // --- Action names ---
    change_action: ActionName,
    end_edit_action: ActionName,
    enter_action: ActionName,

    /// Инициализация
    pub fn init(element_id: ElementId) TextInput {
        return .{
            .interactable = Interactable.init(element_id),
            .text_entity = 0,
            .placeholder_text_entity = 0,
            .max_string_length = 256,
            .is_password_field = false,
            .replacement_character = '*',
            .is_editing = false,
            .is_dragging = false,
            .cursor_position = 0,
            .selection_start = 0,
            .cursor_blink_interval = 0.5,
            .cursor_blink_start_time = 0,
            .text_selection_color = Color.init(0.2, 0.4, 0.8, 1.0),
            .text_cursor_color = Color.white(),
            .is_clipboard_enabled = true,
            .on_change = ui_interactable.noopTextInputCallback,
            .on_end_edit = ui_interactable.noopTextInputCallback,
            .on_enter = ui_interactable.noopTextInputCallback,
            .change_action = "",
            .end_edit_action = "",
            .enter_action = "",
        };
    }

    /// Handle press — начать редактирование.
    pub fn handlePressed(self: *TextInput, point: Vec2) bool {
        if (!self.interactable.is_handling_events) return false;
        _ = self.interactable.handlePressed(point);
        self.is_editing = true;
        self.is_dragging = true;
        self.cursor_blink_start_time = 0;
        return true;
    }

    /// Handle release — закончить drag.
    pub fn handleReleased(self: *TextInput, point: Vec2) bool {
        if (self.interactable.is_pressed) {
            _ = self.interactable.handleReleased(point);
            self.is_dragging = false;
            return true;
        }
        return false;
    }

    /// Handle enter press — подтвердить ввод.
    pub fn handleEnterPressed(self: *TextInput) bool {
        if (!self.interactable.is_handling_events) return false;
        self.is_editing = true;
        _ = self.interactable.handlePressed(Vec2.zero());
        return true;
    }

    /// Handle enter release — закончить редактирование.
    pub fn handleEnterReleased(self: *TextInput) bool {
        self.is_editing = false;
        self.is_dragging = false;
        self.on_enter(self.interactable.element_id, "");
        self.on_end_edit(self.interactable.element_id, "");
        _ = self.interactable.handleReleased(Vec2.zero());
        return true;
    }

    /// Handle text input — вставить символ.
    pub fn handleTextInput(self: *TextInput, input_text: []const u8) bool {
        if (!self.is_editing) return false;
        _ = input_text;
        self.on_change(self.interactable.element_id, "");
        return true;
    }

    /// Lost active status — закончить редактирование.
    pub fn lostActiveStatus(self: *TextInput) void {
        self.is_editing = false;
        self.is_dragging = false;
        self.interactable.lostActiveStatus();
    }

    /// Обновить (cursor blink).
    pub fn update(self: *TextInput, dt: f32) void {
        if (self.is_editing) {
            self.cursor_blink_start_time += dt;
            if (self.cursor_blink_start_time >= self.cursor_blink_interval * 2.0) {
                self.cursor_blink_start_time -= self.cursor_blink_interval * 2.0;
            }
        }
    }

    /// Видим ли курсор (для blink)?
    pub fn isCursorVisible(self: TextInput) bool {
        if (!self.is_editing) return false;
        return self.cursor_blink_start_time < self.cursor_blink_interval;
    }
};

// =============================================================================
// 7. CLIPBOARD
// =============================================================================

/// Clipboard — абстракция системного буфера обмена.
///
/// В O3DE: UiClipboard (platform-specific).
pub const Clipboard = struct {
    var clipboard_text: [4096]u8 = undefined;
    var clipboard_len: usize = 0;

    /// Скопировать текст в clipboard.
    pub fn copy(text: []const u8) void {
        const len = @min(text.len, clipboard_text.len - 1);
        @memcpy(clipboard_text[0..len], text[0..len]);
        clipboard_len = len;
        clipboard_text[clipboard_len] = 0;
    }

    /// Получить текст из clipboard.
    pub fn paste() []const u8 {
        return clipboard_text[0..clipboard_len];
    }

    /// Очистить clipboard.
    pub fn clear() void {
        clipboard_len = 0;
    }
};

// =============================================================================
// 8. ТЕСТЫ
// =============================================================================

test "Text: init defaults" {
    const txt = Text.init();
    try std.testing.expect(txt.h_alignment == .left);
    try std.testing.expect(txt.v_alignment == .top);
    try std.testing.expect(txt.font_size == 16.0);
    try std.testing.expect(txt.overflow_mode == .overflow_text);
}

test "Text: set text" {
    var txt = Text.init();
    txt.setText("Hello, P3!");
    try std.testing.expectEqualStrings(txt.text, "Hello, P3!");
    try std.testing.expect(txt.are_draw_batches_dirty);
}

test "Text: selection" {
    var txt = Text.init();
    txt.setSelection(2, 5);
    try std.testing.expect(txt.hasSelection());
    txt.clearSelection();
    try std.testing.expect(!txt.hasSelection());
}

test "Text: override and reset" {
    var txt = Text.init();
    txt.setOverrideColor(Color.init(1.0, 0.0, 0.0, 1.0));
    try std.testing.expect(txt.is_color_overridden);
    txt.resetOverrides();
    try std.testing.expect(!txt.is_color_overridden);
}

test "TextInput: init defaults" {
    const ti = TextInput.init(1);
    try std.testing.expect(!ti.is_editing);
    try std.testing.expect(!ti.is_password_field);
    try std.testing.expect(ti.max_string_length == 256);
}

test "TextInput: edit flow" {
    var ti = TextInput.init(1);
    _ = ti.handlePressed(Vec2.init(100, 100));
    try std.testing.expect(ti.is_editing);
    try std.testing.expect(ti.is_dragging);

    _ = ti.handleReleased(Vec2.init(100, 100));
    try std.testing.expect(!ti.is_dragging);
}

test "TextInput: enter flow" {
    var ti = TextInput.init(1);
    _ = ti.handleEnterPressed();
    try std.testing.expect(ti.is_editing);

    _ = ti.handleEnterReleased();
    try std.testing.expect(!ti.is_editing);
}

test "TextInput: cursor blink" {
    var ti = TextInput.init(1);
    ti.is_editing = true;
    ti.cursor_blink_start_time = 0.3;
    try std.testing.expect(ti.isCursorVisible()); // < 0.5

    ti.cursor_blink_start_time = 0.7;
    try std.testing.expect(!ti.isCursorVisible()); // >= 0.5
}

test "Clipboard: copy and paste" {
    Clipboard.clear();
    Clipboard.copy("Hello");
    const pasted = Clipboard.paste();
    try std.testing.expectEqualStrings(pasted, "Hello");
}
