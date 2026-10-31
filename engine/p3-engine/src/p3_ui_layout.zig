// =============================================================================
// P³ UI LAYOUT v1.0 — ZIG
// =============================================================================
//
// Layout manager для UI (из O3DE UiLayoutManager + Layout Components).
//
// В O3DE LyShine:
//   UiLayoutManager — dirty-marking, parent-before-child traversal
//   UiLayoutRowComponent — горизонтальный row
//   UiLayoutColumnComponent — вертикальный column
//   UiLayoutGridComponent — сетка
//   UiLayoutFitterComponent — auto-fit
//   UiLayoutCellComponent — per-element size hints
//
// В P³ Engine:
//   - Сохраняем dirty-marking (zero-cost when clean)
//   - Row/Column/Grid — прямые порты
//   - Добавляем ProjectiveLayout — сохраняет cross-ratio
//
// Портировано из O3DE/Gems/LyShine/Code/Source/UiLayoutManager
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
// 1. LAYOUT DIRTY FLAGS
// =============================================================================

/// Флаги загрязнённости layout.
///
/// Dirty-marking позволяет избежать лишних пересчётов:
/// если ничего не изменилось, layout не пересчитывается.
pub const LayoutDirtyFlags = packed struct {
    size: bool = false, // Размер элемента изменился
    child_order: bool = false, // Порядок детей изменился
    child_size: bool = false, // Размер ребёнка изменился
    anchors: bool = false, // Анкоры изменились
    padding: bool = false, // Padding изменился
    spacing: bool = false, // Spacing изменился
};

// =============================================================================
// 2. LAYOUT ALIGNMENT
// =============================================================================

/// Выравнивание элемента внутри layout ячейки.
pub const LayoutAlignment = enum(u2) {
    left_top = 0,
    center = 1,
    right_bottom = 2,
};

// =============================================================================
// 3. LAYOUT PADDING
// =============================================================================

/// Padding вокруг layout контента.
pub const LayoutPadding = struct {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,

    pub inline fn zero() LayoutPadding {
        return .{ .left = 0, .top = 0, .right = 0, .bottom = 0 };
    }

    pub inline fn uniform(v: f64) LayoutPadding {
        return .{ .left = v, .top = v, .right = v, .bottom = v };
    }

    pub inline fn horizontal(self: LayoutPadding) f64 {
        return self.left + self.right;
    }

    pub inline fn vertical(self: LayoutPadding) f64 {
        return self.top + self.bottom;
    }
};

// =============================================================================
// 4. LAYOUT CELL (SIZE HINTS)
// =============================================================================

/// Размерные подсказки для layout ячейки.
///
/// В O3DE: min+target+max позволяет layout'у
/// выбирать оптимальный размер в заданном диапазоне.
pub const LayoutCell = struct {
    min_width: f64,
    min_height: f64,
    target_width: f64,
    target_height: f64,
    max_width: f64,
    max_height: f64,

    pub inline fn fixed(w: f64, h: f64) LayoutCell {
        return .{
            .min_width = w,
            .min_height = h,
            .target_width = w,
            .target_height = h,
            .max_width = w,
            .max_height = h,
        };
    }

    pub inline fn range(min_w: f64, min_h: f64, max_w: f64, max_h: f64) LayoutCell {
        return .{
            .min_width = min_w,
            .min_height = min_h,
            .target_width = (min_w + max_w) / 2.0,
            .target_height = (min_h + max_h) / 2.0,
            .max_width = max_w,
            .max_height = max_h,
        };
    }

    pub inline fn flexible(min_w: f64, min_h: f64) LayoutCell {
        return .{
            .min_width = min_w,
            .min_height = min_h,
            .target_width = min_w,
            .target_height = min_h,
            .max_width = std.math.floatMax(f64),
            .max_height = std.math.floatMax(f64),
        };
    }
};

// =============================================================================
// 5. LAYOUT CHILD ENTRY
// =============================================================================

/// Запись о ребёнке в layout.
pub const LayoutChild = struct {
    /// Размерные подсказки
    cell: LayoutCell,
    /// Фактический размер (после layout)
    computed_rect: Rect,
    /// Выравнивание
    h_alignment: LayoutAlignment,
    v_alignment: LayoutAlignment,
};

// =============================================================================
// 6. ROW LAYOUT
// =============================================================================

/// Горизонтальный row layout.
///
/// Дети располагаются слева направо (или справа налево).
/// Spacing между детьми. Padding вокруг контента.
pub const RowLayout = struct {
    spacing: f64,
    padding: LayoutPadding,
    h_alignment: LayoutAlignment,
    v_alignment: LayoutAlignment,
    reverse: bool, // RTL

    pub fn init(spacing: f64, padding: LayoutPadding) RowLayout {
        return .{
            .spacing = spacing,
            .padding = padding,
            .h_alignment = .left_top,
            .v_alignment = .center,
            .reverse = false,
        };
    }

    /// Вычислить layout для детей.
    ///
    /// children: массив записей с cell-хинтами
    /// container: доступное пространство
    /// allocator: для временных вычислений
    ///
    /// Заполняет computed_rect в каждом child.
    pub fn layout(self: RowLayout, children: []LayoutChild, container: Rect) void {
        if (children.len == 0) return;

        // Доступное пространство
        const avail_w = container.width() - self.padding.horizontal();
        const avail_h = container.height() - self.padding.vertical();

        // Сумма target widths + spacing
        var total_target_w: f64 = 0;
        for (children, 0..) |child, i| {
            total_target_w += child.cell.target_width;
            if (i > 0) total_target_w += self.spacing;
        }

        // Если total > avail — масштабируем вниз
        const scale: f64 = if (total_target_w > avail_w and total_target_w > 0)
            avail_w / total_target_w
        else
            1.0;

        // Расставляем детей
        const x = container.left + self.padding.left;
        const y = container.top + self.padding.top;

        // RTL
        const start_x = if (self.reverse) container.right - self.padding.right else x;

        if (self.reverse) {
            var cx = start_x;
            for (children, 0..) |*child, i| {
                const w = child.cell.target_width * scale;
                const h = @min(child.cell.target_height, avail_h);

                // V-alignment
                const cy = switch (child.v_alignment) {
                    .left_top => y,
                    .center => y + (avail_h - h) / 2.0,
                    .right_bottom => y + avail_h - h,
                };

                child.computed_rect = Rect.fromSize(cx - w, cy, w, h);
                cx -= w;
                if (i < children.len - 1) cx -= self.spacing;
            }
        } else {
            var cx = start_x;
            for (children, 0..) |*child, i| {
                const w = child.cell.target_width * scale;
                const h = @min(child.cell.target_height, avail_h);

                // V-alignment
                const cy = switch (child.v_alignment) {
                    .left_top => y,
                    .center => y + (avail_h - h) / 2.0,
                    .right_bottom => y + avail_h - h,
                };

                child.computed_rect = Rect.fromSize(cx, cy, w, h);
                cx += w;
                if (i < children.len - 1) cx += self.spacing;
            }
        }
    }
};

// =============================================================================
// 7. COLUMN LAYOUT
// =============================================================================

/// Вертикальный column layout.
///
/// Дети располагаются сверху вниз (или снизу вверх).
pub const ColumnLayout = struct {
    spacing: f64,
    padding: LayoutPadding,
    h_alignment: LayoutAlignment,
    v_alignment: LayoutAlignment,
    reverse: bool, // bottom-to-top

    pub fn init(spacing: f64, padding: LayoutPadding) ColumnLayout {
        return .{
            .spacing = spacing,
            .padding = padding,
            .h_alignment = .center,
            .v_alignment = .left_top,
            .reverse = false,
        };
    }

    /// Вычислить column layout для детей.
    pub fn layout(self: ColumnLayout, children: []LayoutChild, container: Rect) void {
        if (children.len == 0) return;

        const avail_w = container.width() - self.padding.horizontal();
        const avail_h = container.height() - self.padding.vertical();

        var total_target_h: f64 = 0;
        for (children, 0..) |child, i| {
            total_target_h += child.cell.target_height;
            if (i > 0) total_target_h += self.spacing;
        }

        const scale: f64 = if (total_target_h > avail_h and total_target_h > 0)
            avail_h / total_target_h
        else
            1.0;

        const x = container.left + self.padding.left;
        const y = container.top + self.padding.top;

        if (self.reverse) {
            var cy = container.bottom - self.padding.bottom;
            for (children, 0..) |*child, i| {
                const w = @min(child.cell.target_width, avail_w);
                const h = child.cell.target_height * scale;

                const cx = switch (child.h_alignment) {
                    .left_top => x,
                    .center => x + (avail_w - w) / 2.0,
                    .right_bottom => x + avail_w - w,
                };

                child.computed_rect = Rect.fromSize(cx, cy - h, w, h);
                cy -= h;
                if (i < children.len - 1) cy -= self.spacing;
            }
        } else {
            var cy = y;
            for (children, 0..) |*child, i| {
                const w = @min(child.cell.target_width, avail_w);
                const h = child.cell.target_height * scale;

                const cx = switch (child.h_alignment) {
                    .left_top => x,
                    .center => x + (avail_w - w) / 2.0,
                    .right_bottom => x + avail_w - w,
                };

                child.computed_rect = Rect.fromSize(cx, cy, w, h);
                cy += h;
                if (i < children.len - 1) cy += self.spacing;
            }
        }
    }
};

// =============================================================================
// 8. GRID LAYOUT
// =============================================================================

/// Сетка с ячейками заданного размера.
pub const GridLayout = struct {
    cell_width: f64,
    cell_height: f64,
    spacing_x: f64,
    spacing_y: f64,
    padding: LayoutPadding,
    columns: u32, // 0 = auto

    pub fn init(cell_w: f64, cell_h: f64, cols: u32, padding: LayoutPadding) GridLayout {
        return .{
            .cell_width = cell_w,
            .cell_height = cell_h,
            .spacing_x = 2.0,
            .spacing_y = 2.0,
            .padding = padding,
            .columns = cols,
        };
    }

    /// Вычислить grid layout для детей.
    pub fn layout(self: GridLayout, children: []LayoutChild, container: Rect) void {
        if (children.len == 0) return;

        const avail_w = container.width() - self.padding.horizontal();

        // Определяем число колонок
        const cols: u32 = if (self.columns > 0)
            self.columns
        else blk: {
            // Auto: сколько ячеек влезает
            const step = self.cell_width + self.spacing_x;
            if (step < 1) break :blk 1;
            break :blk @max(1, @as(u32, @intFromFloat(@floor(avail_w / step))));
        };

        const x0 = container.left + self.padding.left;
        const y0 = container.top + self.padding.top;

        for (children, 0..) |*child, i| {
            const col: u32 = @intCast(i % cols);
            const row: u32 = @intCast(i / cols);

            const cx = x0 + @as(f64, @floatFromInt(col)) * (self.cell_width + self.spacing_x);
            const cy = y0 + @as(f64, @floatFromInt(row)) * (self.cell_height + self.spacing_y);

            // Масштабируем до cell size если cell hint позволяет
            const w = @min(child.cell.target_width, self.cell_width);
            const h = @min(child.cell.target_height, self.cell_height);

            // Центрируем в ячейке
            const ox = (self.cell_width - w) / 2.0;
            const oy = (self.cell_height - h) / 2.0;

            child.computed_rect = Rect.fromSize(cx + ox, cy + oy, w, h);
        }
    }
};

// =============================================================================
// 9. LAYOUT MANAGER (DIRTY-MARKING)
// =============================================================================

/// Layout Manager — координирует пересчёт layout'ов.
///
/// Dirty-marking: если элемент не изменился, его layout
/// не пересчитывается. Это zero-cost когда всё чисто.
///
/// Traversal: parent-before-child (сначала layout
/// родителя, чтобы дети знали своё доступное пространство).
pub const LayoutManager = struct {
    dirty: LayoutDirtyFlags,

    pub fn init() LayoutManager {
        return .{ .dirty = .{ .size = true, .child_order = true, .child_size = true, .anchors = true, .padding = true, .spacing = true } };
    }

    /// Пометить всё как грязное
    pub fn markAllDirty(self: *LayoutManager) void {
        self.dirty = .{ .size = true, .child_order = true, .child_size = true, .anchors = true, .padding = true, .spacing = true };
    }

    /// Пометить размер как грязный
    pub fn markSizeDirty(self: *LayoutManager) void {
        self.dirty.size = true;
        self.dirty.child_size = true;
    }

    /// Пометить порядок детей как грязный
    pub fn markChildOrderDirty(self: *LayoutManager) void {
        self.dirty.child_order = true;
    }

    /// Нужен ли пересчёт?
    pub inline fn isDirty(self: LayoutManager) bool {
        return self.dirty.size or self.dirty.child_order or
            self.dirty.child_size or self.dirty.anchors or
            self.dirty.padding or self.dirty.spacing;
    }

    /// Пометить как чистый (после пересчёта)
    pub inline fn markClean(self: *LayoutManager) void {
        self.dirty = .{};
    }
};

// =============================================================================
// 10. ТЕСТЫ
// =============================================================================

test "LayoutDirtyFlags: initially clean" {
    const flags = LayoutDirtyFlags{};
    try std.testing.expect(!flags.size);
    try std.testing.expect(!flags.child_order);
}

test "LayoutPadding: horizontal and vertical" {
    const p = LayoutPadding.uniform(10);
    try std.testing.expectApproxEqAbs(p.horizontal(), 20.0, 1e-10);
    try std.testing.expectApproxEqAbs(p.vertical(), 20.0, 1e-10);
}

test "LayoutCell: fixed" {
    const cell = LayoutCell.fixed(100, 50);
    try std.testing.expectApproxEqAbs(cell.min_width, 100.0, 1e-10);
    try std.testing.expectApproxEqAbs(cell.max_width, 100.0, 1e-10);
    try std.testing.expectApproxEqAbs(cell.target_height, 50.0, 1e-10);
}

test "RowLayout: three children" {
    const row = RowLayout.init(5.0, LayoutPadding.zero());
    var children = [_]LayoutChild{
        .{ .cell = LayoutCell.fixed(100, 30), .computed_rect = Rect.init(0, 0, 0, 0), .h_alignment = .left_top, .v_alignment = .center },
        .{ .cell = LayoutCell.fixed(200, 30), .computed_rect = Rect.init(0, 0, 0, 0), .h_alignment = .left_top, .v_alignment = .center },
        .{ .cell = LayoutCell.fixed(150, 30), .computed_rect = Rect.init(0, 0, 0, 0), .h_alignment = .left_top, .v_alignment = .center },
    };
    const container = Rect.fromSize(0, 0, 800, 600);
    row.layout(&children, container);

    // Первый ребёнок: x=0, w=100
    try std.testing.expectApproxEqAbs(children[0].computed_rect.left, 0.0, 1e-10);
    try std.testing.expectApproxEqAbs(children[0].computed_rect.width(), 100.0, 1e-10);

    // Второй ребёнок: x=100+5=105
    try std.testing.expectApproxEqAbs(children[1].computed_rect.left, 105.0, 1e-10);
    try std.testing.expectApproxEqAbs(children[1].computed_rect.width(), 200.0, 1e-10);

    // Третий ребёнок: x=105+200+5=310
    try std.testing.expectApproxEqAbs(children[2].computed_rect.left, 310.0, 1e-10);
    try std.testing.expectApproxEqAbs(children[2].computed_rect.width(), 150.0, 1e-10);
}

test "RowLayout: overflow scales down" {
    const row = RowLayout.init(0, LayoutPadding.zero());
    var children = [_]LayoutChild{
        .{ .cell = LayoutCell.fixed(500, 30), .computed_rect = Rect.init(0, 0, 0, 0), .h_alignment = .left_top, .v_alignment = .center },
        .{ .cell = LayoutCell.fixed(500, 30), .computed_rect = Rect.init(0, 0, 0, 0), .h_alignment = .left_top, .v_alignment = .center },
    };
    const container = Rect.fromSize(0, 0, 400, 600); // Too narrow!
    row.layout(&children, container);

    // Both should be scaled to 200 each
    try std.testing.expectApproxEqAbs(children[0].computed_rect.width(), 200.0, 1e-6);
    try std.testing.expectApproxEqAbs(children[1].computed_rect.width(), 200.0, 1e-6);
}

test "ColumnLayout: three children" {
    const col = ColumnLayout.init(5.0, LayoutPadding.zero());
    var children = [_]LayoutChild{
        .{ .cell = LayoutCell.fixed(200, 50), .computed_rect = Rect.init(0, 0, 0, 0), .h_alignment = .center, .v_alignment = .left_top },
        .{ .cell = LayoutCell.fixed(200, 100), .computed_rect = Rect.init(0, 0, 0, 0), .h_alignment = .center, .v_alignment = .left_top },
        .{ .cell = LayoutCell.fixed(200, 75), .computed_rect = Rect.init(0, 0, 0, 0), .h_alignment = .center, .v_alignment = .left_top },
    };
    const container = Rect.fromSize(0, 0, 800, 600);
    col.layout(&children, container);

    try std.testing.expectApproxEqAbs(children[0].computed_rect.top, 0.0, 1e-10);
    try std.testing.expectApproxEqAbs(children[1].computed_rect.top, 55.0, 1e-10); // 50+5
    try std.testing.expectApproxEqAbs(children[2].computed_rect.top, 160.0, 1e-10); // 55+100+5
}

test "GridLayout: 2x2 grid" {
    const grid = GridLayout.init(100, 100, 2, LayoutPadding.zero());
    var children = [_]LayoutChild{
        .{ .cell = LayoutCell.fixed(80, 80), .computed_rect = Rect.init(0, 0, 0, 0), .h_alignment = .center, .v_alignment = .center },
        .{ .cell = LayoutCell.fixed(80, 80), .computed_rect = Rect.init(0, 0, 0, 0), .h_alignment = .center, .v_alignment = .center },
        .{ .cell = LayoutCell.fixed(80, 80), .computed_rect = Rect.init(0, 0, 0, 0), .h_alignment = .center, .v_alignment = .center },
        .{ .cell = LayoutCell.fixed(80, 80), .computed_rect = Rect.init(0, 0, 0, 0), .h_alignment = .center, .v_alignment = .center },
    };
    const container = Rect.fromSize(0, 0, 800, 600);
    grid.layout(&children, container);

    // (0,0): centered in cell
    try std.testing.expectApproxEqAbs(children[0].computed_rect.width(), 80.0, 1e-10);
    try std.testing.expectApproxEqAbs(children[0].computed_rect.height(), 80.0, 1e-10);

    // (1,0): second column
    try std.testing.expect(children[1].computed_rect.left > children[0].computed_rect.left);

    // (0,1): second row
    try std.testing.expect(children[2].computed_rect.top > children[0].computed_rect.top);
}

test "LayoutManager: dirty marking" {
    var mgr = LayoutManager.init();
    try std.testing.expect(mgr.isDirty());
    mgr.markClean();
    try std.testing.expect(!mgr.isDirty());
    mgr.markSizeDirty();
    try std.testing.expect(mgr.isDirty());
}
