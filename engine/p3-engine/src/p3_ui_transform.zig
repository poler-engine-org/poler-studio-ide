// =============================================================================
// P³ UI TRANSFORM v1.0 — ZIG
// =============================================================================
//
// 2D UI-трансформ с anchor+offset системой (из O3DE UiTransform2dComponent).
//
// В O3DE LyShine элемент позиционируется относительно родителя через:
//   - Anchors (0-1 нормализованные внутри родителя)
//   - Offsets (пиксельные смещения от анкоров)
//   - Pivot (точка вращения/масштаба внутри элемента)
//   - Rotation (вокруг pivot)
//   - Scale (от pivot)
//
// В P³ Engine мы обобщаем:
//   - Anchors → проективные якоря (может быть > 1.0 для перспективы)
//   - ScaleToDeviceMode → единый проективный трансформ canvas→viewport
//   - Matrix4x4 → Mat3 (проективный 2D, 8 DOF)
//
// Ключевая формула O3DE:
//   position = lerp(parent_corners, anchor) + offset
//
// В P³:
//   position = projectiveLerp(parent_corners, anchor) + projectiveOffset
//
// Портировано из O3DE/Gems/LyShine/Code/Source/UiTransform2dComponent
// Адаптировано для Zig 0.14.0 + P³ Engine API.
//
// Архитектор: Kotokvit (математик), Super Z (исполнение)
// =============================================================================

const std = @import("std");
const math = std.math;

// =============================================================================
// 1. БАЗОВЫЕ 2D ТИПЫ
// =============================================================================

/// 2D вектор
pub const Vec2 = struct {
    x: f64,
    y: f64,

    pub inline fn init(x: f64, y: f64) Vec2 {
        return .{ .x = x, .y = y };
    }

    pub inline fn zero() Vec2 {
        return .{ .x = 0, .y = 0 };
    }

    pub inline fn add(a: Vec2, b: Vec2) Vec2 {
        return .{ .x = a.x + b.x, .y = a.y + b.y };
    }

    pub inline fn sub(a: Vec2, b: Vec2) Vec2 {
        return .{ .x = a.x - b.x, .y = a.y - b.y };
    }

    pub inline fn scale(v: Vec2, s: f64) Vec2 {
        return .{ .x = v.x * s, .y = v.y * s };
    }

    pub inline fn lerp(a: Vec2, b: Vec2, t: f64) Vec2 {
        return .{
            .x = a.x + t * (b.x - a.x),
            .y = a.y + t * (b.y - a.y),
        };
    }
};

/// 2D прямоугольник (axis-aligned)
pub const Rect = struct {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,

    pub inline fn init(l: f64, t: f64, r: f64, b: f64) Rect {
        return .{ .left = l, .top = t, .right = r, .bottom = b };
    }

    pub inline fn fromSize(x: f64, y: f64, w: f64, h: f64) Rect {
        return .{ .left = x, .top = y, .right = x + w, .bottom = y + h };
    }

    pub inline fn width(self: Rect) f64 {
        return self.right - self.left;
    }

    pub inline fn height(self: Rect) f64 {
        return self.bottom - self.top;
    }

    pub inline fn center(self: Rect) Vec2 {
        return Vec2.init(
            (self.left + self.right) / 2.0,
            (self.top + self.bottom) / 2.0,
        );
    }
};

/// 4 угла прямоугольника (после трансформа)
pub const RectPoints = struct {
    top_left: Vec2,
    top_right: Vec2,
    bottom_left: Vec2,
    bottom_right: Vec2,
};

// =============================================================================
// 2. ANCHORS
// =============================================================================

/// Анкоры — нормализованные [0,1] позиции внутри родителя.
///
/// В O3DE: анкоры определяют «резиновую» привязку элемента
/// к углам/сторонам родителя. Анкор (0,0) = левый-верхний
/// угол родителя, (1,1) = правый-нижний.
///
/// В P³: анкоры могут выходить за [0,1] для проективных
/// эффектов (перспективное позиционирование).
pub const Anchors = struct {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,

    /// Все анкоры в одной точке
    pub inline fn all(v: f64) Anchors {
        return .{ .left = v, .top = v, .right = v, .bottom = v };
    }

    /// По умолчанию: левый-верхний угол
    pub inline fn default() Anchors {
        return .{ .left = 0, .top = 0, .right = 0, .bottom = 0 };
    }

    /// Растянуть на весь родитель
    pub inline fn stretch() Anchors {
        return .{ .left = 0, .top = 0, .right = 1, .bottom = 1 };
    }

    /// Центр родителя
    pub inline fn center() Anchors {
        return .{ .left = 0.5, .top = 0.5, .right = 0.5, .bottom = 0.5 };
    }

    /// Горизонтальный центрированный растяг
    pub inline fn horizontalStretch() Anchors {
        return .{ .left = 0, .top = 0.5, .right = 1, .bottom = 0.5 };
    }

    /// Вертикальный центрированный растяг
    pub inline fn verticalStretch() Anchors {
        return .{ .left = 0.5, .top = 0, .right = 0.5, .bottom = 1 };
    }
};

// =============================================================================
// 3. OFFSETS
// =============================================================================

/// Офсеты — пиксельные смещения от анкорных позиций.
///
/// В O3DE: offsets добавляются к позициям, вычисленным
/// из анкоров. Это позволяет позиционировать элемент
/// с точностью до пикселя, даже если анкоры «резиновые».
pub const Offsets = struct {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,

    /// Нулевые офсеты
    pub inline fn zero() Offsets {
        return .{ .left = 0, .top = 0, .right = 0, .bottom = 0 };
    }

    /// Одинаковый отступ со всех сторон
    pub inline fn uniform(v: f64) Offsets {
        return .{ .left = v, .top = v, .right = -v, .bottom = -v };
    }

    /// Из ширины и высоты (центрированный)
    pub inline fn fromSize(w: f64, h: f64) Offsets {
        return .{ .left = -w / 2, .top = -h / 2, .right = w / 2, .bottom = h / 2 };
    }
};

// =============================================================================
// 4. PIVOT
// =============================================================================

/// Pivot — точка внутри элемента, вокруг которой вращение и масштаб.
///
/// (0,0) = левый-верхний угол элемента
/// (0.5, 0.5) = центр элемента
/// (1,1) = правый-нижний угол элемента
pub const Pivot = struct {
    x: f64,
    y: f64,

    pub inline fn init(x: f64, y: f64) Pivot {
        return .{ .x = x, .y = y };
    }

    /// Центр элемента
    pub inline fn center() Pivot {
        return .{ .x = 0.5, .y = 0.5 };
    }

    /// Левый-верхний
    pub inline fn topLeft() Pivot {
        return .{ .x = 0, .y = 0 };
    }
};

// =============================================================================
// 5. SCALE-TO-DEVICE MODE
// =============================================================================

/// Как элемент масштабируется при изменении размера viewport.
///
/// В O3DE: 7 режимов. В P³: унифицируем в проективный трансформ.
pub const ScaleToDeviceMode = enum(u3) {
    /// Без масштабирования
    none = 0,
    /// Равномерное масштабирование чтобы вписаться
    uniform_scale_to_fit = 1,
    /// Равномерное масштабирование с заполнением (может обрезать)
    uniform_scale_to_fill = 2,
    /// Равномерное масштабирование по ширине
    uniform_scale_to_fit_width = 3,
    /// Равномерное масштабирование по высоте
    uniform_scale_to_fit_height = 4,
    /// Неравномерное (растягивает)
    stretch_to_fit = 5,
    /// Проективное (P³ обобщение — сохраняет cross-ratio)
    projective = 6,
};

// =============================================================================
// 6. UI TRANSFORM 2D
// =============================================================================

/// 2D UI трансформ — полный набор данных для позиционирования элемента.
///
/// Вычисление позиции элемента относительно родителя:
///
///   1. Анкорные позиции = lerp(parent_corners, anchors)
///   2. Анкорный прямоугольник = анкорные_позиции + offsets
///   3. Pivot = точка внутри элемента для вращения/масштаба
///   4. Итоговая позиция = rotate(scale(anchor_rect, pivot), pivot)
///
/// Это прямой порт O3DE UiTransform2dComponent с проективным обобщением.
pub const UiTransform2d = struct {
    anchors: Anchors,
    offsets: Offsets,
    pivot: Pivot,
    rotation: f64, // радианы
    scale: Vec2,
    scale_to_device: ScaleToDeviceMode,

    /// Инициализация по умолчанию
    pub fn initDefault() UiTransform2d {
        return .{
            .anchors = Anchors.default(),
            .offsets = Offsets.zero(),
            .pivot = Pivot.topLeft(),
            .rotation = 0,
            .scale = Vec2.init(1, 1),
            .scale_to_device = .none,
        };
    }

    /// Инициализация с анкерами и офсетами
    pub fn init(anchors: Anchors, offsets: Offsets) UiTransform2d {
        return .{
            .anchors = anchors,
            .offsets = offsets,
            .pivot = Pivot.topLeft(),
            .rotation = 0,
            .scale = Vec2.init(1, 1),
            .scale_to_device = .none,
        };
    }

    /// Вычислить прямоугольник элемента в локальных координатах
    /// родителя на основе анкоров и офсетов.
    ///
    /// Формула O3DE:
    ///   left   = parent.left   + anchor.left   * parent.width  + offset.left
    ///   top    = parent.top    + anchor.top    * parent.height + offset.top
    ///   right  = parent.left   + anchor.right  * parent.width  + offset.right
    ///   bottom = parent.top    + anchor.bottom * parent.height + offset.bottom
    pub fn computeLocalRect(self: UiTransform2d, parent: Rect) Rect {
        const pw = parent.width();
        const ph = parent.height();
        return .{
            .left = parent.left + self.anchors.left * pw + self.offsets.left,
            .top = parent.top + self.anchors.top * ph + self.offsets.top,
            .right = parent.left + self.anchors.right * pw + self.offsets.right,
            .bottom = parent.top + self.anchors.bottom * ph + self.offsets.bottom,
        };
    }

    /// Вычислить 4 угла прямоугольника после вращения и масштаба.
    ///
    /// Шаги:
    ///   1. Вычислить anchor rect
    ///   2. Найти pivot point внутри rect
    ///   3. Масштабировать углы относительно pivot
    ///   4. Повернуть углы относительно pivot
    pub fn computeTransformedPoints(self: UiTransform2d, parent: Rect) RectPoints {
        const rect = self.computeLocalRect(parent);

        // Pivot point внутри rect
        const pivot_pt = Vec2.init(
            rect.left + self.pivot.x * rect.width(),
            rect.top + self.pivot.y * rect.height(),
        );

        // 4 угла
        var corners = [4]Vec2{
            Vec2.init(rect.left, rect.top), // TL
            Vec2.init(rect.right, rect.top), // TR
            Vec2.init(rect.left, rect.bottom), // BL
            Vec2.init(rect.right, rect.bottom), // BR
        };

        // Масштаб относительно pivot
        const cos_r = math.cos(self.rotation);
        const sin_r = math.sin(self.rotation);

        for (&corners) |*c| {
            // Сдвиг к pivot
            var dx = c.x - pivot_pt.x;
            var dy = c.y - pivot_pt.y;

            // Масштаб
            dx *= self.scale.x;
            dy *= self.scale.y;

            // Вращение
            const rx = dx * cos_r - dy * sin_r;
            const ry = dx * sin_r + dy * cos_r;

            // Сдвиг обратно
            c.x = pivot_pt.x + rx;
            c.y = pivot_pt.y + ry;
        }

        return .{
            .top_left = corners[0],
            .top_right = corners[1],
            .bottom_left = corners[2],
            .bottom_right = corners[3],
        };
    }

    /// Ширина элемента (до трансформа)
    pub inline fn width(self: UiTransform2d, parent: Rect) f64 {
        const rect = self.computeLocalRect(parent);
        return rect.width();
    }

    /// Высота элемента (до трансформа)
    pub inline fn height(self: UiTransform2d, parent: Rect) f64 {
        const rect = self.computeLocalRect(parent);
        return rect.height();
    }

    /// Применить ScaleToDeviceMode
    pub fn applyScaleToDevice(self: UiTransform2d, viewport: Rect, design_size: Vec2) UiTransform2d {
        switch (self.scale_to_device) {
            .none => return self,
            .stretch_to_fit => {
                const sx = viewport.width() / design_size.x;
                const sy = viewport.height() / design_size.y;
                var result = self;
                result.scale = Vec2.init(sx, sy);
                return result;
            },
            .uniform_scale_to_fit => {
                const sx = viewport.width() / design_size.x;
                const sy = viewport.height() / design_size.y;
                const s = @min(sx, sy);
                var result = self;
                result.scale = Vec2.init(s, s);
                return result;
            },
            .uniform_scale_to_fill => {
                const sx = viewport.width() / design_size.x;
                const sy = viewport.height() / design_size.y;
                const s = @max(sx, sy);
                var result = self;
                result.scale = Vec2.init(s, s);
                return result;
            },
            .uniform_scale_to_fit_width => {
                const s = viewport.width() / design_size.x;
                var result = self;
                result.scale = Vec2.init(s, s);
                return result;
            },
            .uniform_scale_to_fit_height => {
                const s = viewport.height() / design_size.y;
                var result = self;
                result.scale = Vec2.init(s, s);
                return result;
            },
            .projective => {
                // P³ обобщение: проективный трансформ canvas → viewport
                // Сохраняет cross-ratio вместо просто масштаба
                // Пока используем uniform as approximation
                const sx = viewport.width() / design_size.x;
                const sy = viewport.height() / design_size.y;
                const s = @min(sx, sy);
                var result = self;
                result.scale = Vec2.init(s, s);
                return result;
            },
        }
    }
};

// =============================================================================
// 7. ТЕСТЫ
// =============================================================================

test "Vec2: lerp" {
    const a = Vec2.init(0, 0);
    const b = Vec2.init(10, 20);
    const mid = Vec2.lerp(a, b, 0.5);
    try std.testing.expectApproxEqAbs(mid.x, 5.0, 1e-10);
    try std.testing.expectApproxEqAbs(mid.y, 10.0, 1e-10);
}

test "Rect: width and height" {
    const r = Rect.fromSize(10, 20, 100, 200);
    try std.testing.expectApproxEqAbs(r.width(), 100.0, 1e-10);
    try std.testing.expectApproxEqAbs(r.height(), 200.0, 1e-10);
}

test "Anchors: stretch fills parent" {
    const transform = UiTransform2d.init(Anchors.stretch(), Offsets.zero());
    const parent = Rect.fromSize(0, 0, 800, 600);
    const rect = transform.computeLocalRect(parent);
    try std.testing.expectApproxEqAbs(rect.left, 0.0, 1e-10);
    try std.testing.expectApproxEqAbs(rect.top, 0.0, 1e-10);
    try std.testing.expectApproxEqAbs(rect.width(), 800.0, 1e-10);
    try std.testing.expectApproxEqAbs(rect.height(), 600.0, 1e-10);
}

test "Anchors: center positions at center" {
    const transform = UiTransform2d.init(Anchors.center(), Offsets.fromSize(100, 50));
    const parent = Rect.fromSize(0, 0, 800, 600);
    const rect = transform.computeLocalRect(parent);
    // Центр родителя: (400, 300), элемент 100×50 центрирован
    try std.testing.expectApproxEqAbs(rect.left, 350.0, 1e-10);
    try std.testing.expectApproxEqAbs(rect.top, 275.0, 1e-10);
    try std.testing.expectApproxEqAbs(rect.width(), 100.0, 1e-10);
    try std.testing.expectApproxEqAbs(rect.height(), 50.0, 1e-10);
}

test "UiTransform2d: anchor with offset" {
    var transform = UiTransform2d.init(
        Anchors{ .left = 0, .top = 0, .right = 0, .bottom = 0 },
        Offsets{ .left = 10, .top = 20, .right = 110, .bottom = 70 },
    );
    transform.pivot = Pivot.topLeft();
    const parent = Rect.fromSize(0, 0, 800, 600);
    const rect = transform.computeLocalRect(parent);
    try std.testing.expectApproxEqAbs(rect.left, 10.0, 1e-10);
    try std.testing.expectApproxEqAbs(rect.top, 20.0, 1e-10);
    try std.testing.expectApproxEqAbs(rect.width(), 100.0, 1e-10);
    try std.testing.expectApproxEqAbs(rect.height(), 50.0, 1e-10);
}

test "UiTransform2d: rotation preserves size" {
    var transform = UiTransform2d.init(Anchors.center(), Offsets.fromSize(100, 50));
    transform.rotation = math.pi / 4.0; // 45°
    const parent = Rect.fromSize(0, 0, 800, 600);
    const points = transform.computeTransformedPoints(parent);
    // После вращения диагональ должна сохраниться
    const dx = points.top_right.x - points.top_left.x;
    const dy = points.top_right.y - points.top_left.y;
    const top_len = @sqrt(dx * dx + dy * dy);
    try std.testing.expectApproxEqAbs(top_len, 100.0, 1e-6);
}

test "UiTransform2d: scale stretches" {
    var transform = UiTransform2d.init(Anchors.center(), Offsets.fromSize(100, 50));
    transform.scale = Vec2.init(2.0, 3.0);
    const parent = Rect.fromSize(0, 0, 800, 600);
    const points = transform.computeTransformedPoints(parent);
    // Ширина = 100 * 2 = 200
    const dx = points.top_right.x - points.top_left.x;
    try std.testing.expectApproxEqAbs(@abs(dx), 200.0, 1e-6);
}
