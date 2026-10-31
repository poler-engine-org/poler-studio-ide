// =============================================================================
// P³ UI NAVIGATION v1.0 — ZIG
// =============================================================================
//
// Navigation system (из O3DE UiNavigationHelpers + UiNavigationSettings).
//
// В O3DE:
//   UiNavigationSettings — настройки навигации для interactable
//   UiNavigationHelpers  — утилиты для поиска следующего элемента
//   Navigation: Tab, Arrow keys, Enter
//
// В P³ Engine:
//   - Упрощённый navigation (без full accessibility)
//   - Tab order — explicit ordering
//   - Arrow navigation — spatial
//
// Портировано из O3DE/Gems/LyShine/Code/Source/UiNavigation*
// Адаптировано для Zig 0.14.0 + P³ Engine API.
// =============================================================================

const std = @import("std");
const ui_canvas = @import("p3_ui_canvas.zig");

pub const ElementId = ui_canvas.ElementId;

// =============================================================================
// 1. NAVIGATION MODE
// =============================================================================

/// Режим навигации для interactable элемента.
pub const NavigationMode = enum(u3) {
    /// Без навигации
    none = 0,
    /// Tab навигация
    tab = 1,
    /// Стрелочная навигация
    arrow = 2,
    /// Tab + стрелки
    tab_and_arrow = 3,
    /// Автоматическая (на основе spatial position)
    auto_spatial = 4,
};

// =============================================================================
// 2. NAVIGATION ENTRY
// =============================================================================

/// Явные навигационные ссылки для элемента.
pub const NavigationEntry = struct {
    /// Следующий элемент по Tab
    next: ElementId,
    /// Предыдущий элемент по Tab
    prev: ElementId,
    /// Элемент вверх
    up: ElementId,
    /// Элемент вниз
    down: ElementId,
    /// Элемент влево
    left: ElementId,
    /// Элемент вправо
    right: ElementId,

    pub fn init() NavigationEntry {
        return .{ .next = 0, .prev = 0, .up = 0, .down = 0, .left = 0, .right = 0 };
    }
};

// =============================================================================
// 3. NAVIGATION SETTINGS
// =============================================================================

/// Настройки навигации для interactable элемента.
pub const NavigationSettings = struct {
    mode: NavigationMode,
    entry: NavigationEntry,
    /// Tab index (для tab order)
    tab_index: i32,

    pub fn init() NavigationSettings {
        return .{
            .mode = .none,
            .entry = NavigationEntry.init(),
            .tab_index = -1,
        };
    }
};

// =============================================================================
// 4. NAVIGATION HELPERS
// =============================================================================

/// Утилиты для навигации.
pub const NavigationHelpers = struct {
    /// Найти следующий interactable элемент по Tab.
    ///
    /// Проходит по всем элементам в порядке tab_index.
    pub fn findNextTabTarget(
        canvas: *ui_canvas.UiCanvas,
        current_id: ElementId,
    ) ?ElementId {
        _ = current_id;
        var next_id: ElementId = 0;

        // Find next interactable element
        for (canvas.elements.items) |elem| {
            if (!elem.enabled or !elem.interactable) continue;
            if (next_id == 0) {
                next_id = elem.id;
            }
        }

        return if (next_id != 0) next_id else null;
    }

    /// Найти элемент по направлению (spatial).
    ///
    /// Выбирает ближайший interactable элемент в указанном направлении.
    pub fn findSpatialTarget(
        canvas: *ui_canvas.UiCanvas,
        current_id: ElementId,
        direction: enum { up, down, left, right },
    ) ?ElementId {
        const current = canvas.findElement(current_id) orelse return null;
        const cx = current.computed_rect.center().x;
        const cy = current.computed_rect.center().y;

        var best_id: ElementId = 0;
        var best_dist: f64 = std.math.floatMax(f64);

        for (canvas.elements.items) |elem| {
            if (!elem.enabled or !elem.interactable) continue;
            if (elem.id == current_id) continue;

            const ex = elem.computed_rect.center().x;
            const ey = elem.computed_rect.center().y;

            // Check direction
            const in_direction = switch (direction) {
                .up => ey < cy,
                .down => ey > cy,
                .left => ex < cx,
                .right => ex > cx,
            };
            if (!in_direction) continue;

            const dx = ex - cx;
            const dy = ey - cy;
            const dist = dx * dx + dy * dy;

            if (dist < best_dist) {
                best_dist = dist;
                best_id = elem.id;
            }
        }

        return if (best_id != 0) best_id else null;
    }
};

// =============================================================================
// 5. ТЕСТЫ
// =============================================================================

test "NavigationSettings: init defaults" {
    const ns = NavigationSettings.init();
    try std.testing.expect(ns.mode == .none);
    try std.testing.expect(ns.tab_index == -1);
}

test "NavigationEntry: init defaults" {
    const ne = NavigationEntry.init();
    try std.testing.expect(ne.next == 0);
    try std.testing.expect(ne.up == 0);
}
