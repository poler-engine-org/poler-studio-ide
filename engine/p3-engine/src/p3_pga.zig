// =============================================================================
// P³ PGA (PROJECTIVE GEOMETRIC ALGEBRA) v1.0 — ZIG
// =============================================================================
//
// PGA Cl(3,0,1) — проективная геометрическая алгебра для P³.
// Реализация для Zig 0.14.0 + P³ Engine API.
//
// PGA Cl(3,0,1) — это алгебра Клиффорда с сигнатурой (3,0,1),
// идеально подходящая для работы с точками, прямыми и плоскостями
// в проективном 3-мерном пространстве.
//
// Базис Cl(3,0,1):
//   Скаляр:     1                (1 элемент)
//   Вектор:     e1, e2, e3, e0  (4 элемента)
//   Бивектор:   e12, e13, e1e0, e23, e2e0, e3e0  (6 элементов)
//   Тривектор:  e123, e12e0, e13e0, e23e0  (4 элемента)
//   Псевдоскаляр: e123e0  (1 элемент)
//   Итого: 16 элементов
//
// Геометрическая интерпретация:
//   Скаляр    → скаляр (величина)
//   Вектор    → плоскость (dual: точка)
//   Бивектор  → прямая (Plücker)
//   Тривектор → точка (dual: плоскость)
//   Псевдо    → объём (псевдоскаляр)
//
// Ключевые операции:
//   wedge (∧) — meet (пересечение)
//   vee (∨)   — join (соединение)
//   dual (*)  — Hodge dual
//   inner (·) — скалярное произведение Клиффорда
//
// Архитектор: Kotokvit (математик), Super Z (исполнение)
// =============================================================================

const std = @import("std");
const math = std.math;
const kernel = @import("p3_kernel.zig");

pub const HomVec4 = kernel.HomVec4;

// =============================================================================
// 1. PGA МУЛЬТИВЕКТОР Cl(3,0,1)
// =============================================================================
//
// Полный мультивектор Cl(3,0,1) имеет 16 компонент.
// Мы храним только практически используемые:
//   - Скаляр (1): s
//   - Вектор (4): e1, e2, e3, e0 (→ плоскости)
//   - Бивектор (6): e12, e13, e1e0, e23, e2e0, e3e0 (→ прямые)
//   - Тривектор (4): e123, e12e0, e13e0, e23e0 (→ точки)
//   - Псевдоскаляр (1): e123e0

/// Полный мультивектор Cl(3,0,1).
///
/// Хранение: 16 компонент в каноническом порядке.
/// Индексы:
///   [0]  = scalar 1
///   [1-4] = e1, e2, e3, e0
///   [5-10] = e12, e13, e1e0, e23, e2e0, e3e0
///   [11-14] = e123, e12e0, e13e0, e23e0
///   [15] = e123e0 (pseudoscalar)
pub const Multivector = struct {
    data: [16]f64,

    /// Нулевой мультивектор
    pub fn zero() Multivector {
        return .{ .data = .{0} ** 16 };
    }

    /// Скалярный мультивектор
    pub fn scalar(s: f64) Multivector {
        var m = zero();
        m.data[0] = s;
        return m;
    }

    /// Индексы компонент
    pub const Idx = enum(u5) {
        scalar = 0,
        e1 = 1,
        e2 = 2,
        e3 = 3,
        e0 = 4,
        e12 = 5,
        e13 = 6,
        e1e0 = 7,
        e23 = 8,
        e2e0 = 9,
        e3e0 = 10,
        e123 = 11,
        e12e0 = 12,
        e13e0 = 13,
        e23e0 = 14,
        e123e0 = 15,
    };

    /// Доступ по индексу
    pub inline fn get(self: Multivector, idx: Idx) f64 {
        return self.data[@intFromEnum(idx)];
    }

    /// Установка по индексу
    pub inline fn set(self: *Multivector, idx: Idx, val: f64) void {
        self.data[@intFromEnum(idx)] = val;
    }

    /// Норма мультивектора (евклидова всех компонент)
    pub fn norm(self: Multivector) f64 {
        var sum: f64 = 0;
        for (self.data) |c| {
            sum += c * c;
        }
        return @sqrt(sum);
    }
};

// =============================================================================
// 2. PGA ПРИМИТИВЫ
// =============================================================================

/// PGA Точка: тривектор в Cl(3,0,1).
///
/// Точка в P³ представляется как тривектор:
///   P = x·e23e0 + y·e13e0 + z·e12e0 + w·e123
///
/// Dual: вектор плоскости e1, e2, e3, e0.
/// Это согласуется с HomVec4 = [X:Y:Z:W].
pub const PgaPoint = struct {
    x: f64,
    y: f64,
    z: f64,
    w: f64,

    /// Конструктор
    pub inline fn init(x: f64, y: f64, z: f64, w: f64) PgaPoint {
        return .{ .x = x, .y = y, .z = z, .w = w };
    }

    /// Из HomVec4
    pub inline fn fromHomVec4(v: HomVec4) PgaPoint {
        return .{ .x = v.x, .y = v.y, .z = v.z, .w = v.w };
    }

    /// В HomVec4
    pub inline fn toHomVec4(self: PgaPoint) HomVec4 {
        return HomVec4.init(self.x, self.y, self.z, self.w);
    }

    /// Как мультивектор Cl(3,0,1)
    pub fn toMultivector(self: PgaPoint) Multivector {
        var m = Multivector.zero();
        // P = x·e23e0 + y·e13e0 + z·e12e0 + w·e123
        // Внимание: знаки зависят от dual mapping
        m.data[@intFromEnum(Multivector.Idx.e23e0)] = self.x;
        m.data[@intFromEnum(Multivector.Idx.e13e0)] = self.y;
        m.data[@intFromEnum(Multivector.Idx.e12e0)] = self.z;
        m.data[@intFromEnum(Multivector.Idx.e123)] = self.w;
        return m;
    }

    /// Нормализация
    pub fn normalize(self: PgaPoint) PgaPoint {
        const n = @sqrt(self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w);
        if (n < 1e-15) return .{ .x = 0, .y = 0, .z = 0, .w = 1 };
        return .{ .x = self.x / n, .y = self.y / n, .z = self.z / n, .w = self.w / n };
    }
};

/// PGA Плоскость: вектор в Cl(3,0,1).
///
/// Плоскость в P³: a·e1 + b·e2 + c·e3 + d·e0
/// где (a,b,c) — нормаль, d — расстояние от начала.
pub const PgaPlane = struct {
    a: f64, // e1 component (normal x)
    b: f64, // e2 component (normal y)
    c: f64, // e3 component (normal z)
    d: f64, // e0 component (offset)

    /// Конструктор: плоскость aX + bY + cZ + dW = 0
    pub inline fn init(a: f64, b: f64, c: f64, d: f64) PgaPlane {
        return .{ .a = a, .b = b, .c = c, .d = d };
    }

    /// Из 4 коэффициентов (каноническая форма плоскости в P³)
    pub inline fn fromCoeffs(coeffs: [4]f64) PgaPlane {
        return .{ .a = coeffs[0], .b = coeffs[1], .c = coeffs[2], .d = coeffs[3] };
    }

    /// Как мультивектор
    pub fn toMultivector(self: PgaPlane) Multivector {
        var m = Multivector.zero();
        m.data[@intFromEnum(Multivector.Idx.e1)] = self.a;
        m.data[@intFromEnum(Multivector.Idx.e2)] = self.b;
        m.data[@intFromEnum(Multivector.Idx.e3)] = self.c;
        m.data[@intFromEnum(Multivector.Idx.e0)] = self.d;
        return m;
    }
};

/// PGA Прямая: бивектор в Cl(3,0,1) (Plücker coords).
///
/// Прямая представляется 6 Plücker-координатами:
///   L = d1·e12 + d2·e13 + d3·e1e0 + d4·e23 + d5·e2e0 + d6·e3e0
///
/// Plücker-отношение: d1·d6 - d2·d5 + d3·d4 = 0
pub const PgaLine = struct {
    d1: f64, // e12
    d2: f64, // e13
    d3: f64, // e1e0
    d4: f64, // e23
    d5: f64, // e2e0
    d6: f64, // e3e0

    /// Конструктор из Plücker-координат
    pub inline fn init(d1: f64, d2: f64, d3: f64, d4: f64, d5: f64, d6: f64) PgaLine {
        return .{ .d1 = d1, .d2 = d2, .d3 = d3, .d4 = d4, .d5 = d5, .d6 = d6 };
    }

    /// Как мультивектор
    pub fn toMultivector(self: PgaLine) Multivector {
        var m = Multivector.zero();
        m.data[@intFromEnum(Multivector.Idx.e12)] = self.d1;
        m.data[@intFromEnum(Multivector.Idx.e13)] = self.d2;
        m.data[@intFromEnum(Multivector.Idx.e1e0)] = self.d3;
        m.data[@intFromEnum(Multivector.Idx.e23)] = self.d4;
        m.data[@intFromEnum(Multivector.Idx.e2e0)] = self.d5;
        m.data[@intFromEnum(Multivector.Idx.e3e0)] = self.d6;
        return m;
    }

    /// Plücker-условие: d1·d6 - d2·d5 + d3·d4 = 0
    pub fn pluckerResidual(self: PgaLine) f64 {
        return self.d1 * self.d6 - self.d2 * self.d5 + self.d3 * self.d4;
    }

    /// Прямая через две точки (PGA join).
    ///
    /// L = P1 ∨ P2 (dual wedge)
    pub fn fromTwoPoints(p1: PgaPoint, p2: PgaPoint) PgaLine {
        // Plücker-координаты из двух точек
        // d1 = p1.x*p2.y - p1.y*p2.x  (e12)
        // d2 = p1.x*p2.z - p1.z*p2.x  (e13)
        // d3 = p1.w*p2.x - p1.x*p2.w  (e1e0)
        // d4 = p1.y*p2.z - p1.z*p2.y  (e23)
        // d5 = p1.w*p2.y - p1.y*p2.w  (e2e0)
        // d6 = p1.w*p2.z - p1.z*p2.w  (e3e0)
        return .{
            .d1 = p1.x * p2.y - p1.y * p2.x,
            .d2 = p1.x * p2.z - p1.z * p2.x,
            .d3 = p1.w * p2.x - p1.x * p2.w,
            .d4 = p1.y * p2.z - p1.z * p2.y,
            .d5 = p1.w * p2.y - p1.y * p2.w,
            .d6 = p1.w * p2.z - p1.z * p2.w,
        };
    }

    /// Прямая через две плоскости (PGA meet).
    ///
    /// L = π1 ∧ π2
    pub fn fromTwoPlanes(pi1: PgaPlane, pi2: PgaPlane) PgaLine {
        // Бивектор из wedge двух векторов
        return .{
            .d1 = pi1.a * pi2.b - pi1.b * pi2.a,
            .d2 = pi1.a * pi2.c - pi1.c * pi2.a,
            .d3 = pi1.a * pi2.d - pi1.d * pi2.a,
            .d4 = pi1.b * pi2.c - pi1.c * pi2.b,
            .d5 = pi1.b * pi2.d - pi1.d * pi2.b,
            .d6 = pi1.c * pi2.d - pi1.d * pi2.c,
        };
    }
};

// =============================================================================
// 3. PGA ОПЕРАЦИИ
// =============================================================================

/// Проверка скрещенности двух прямых.
///
/// L1 ∨ L2 ≠ 0 → прямые скрещенные (не пересекаются).
///
/// В PGA: pseudoscalar extraction from L1 ∧ L2.
/// Если L1 и L2 пересекаются, их wedge = 0.
/// Если скрещенные — pseudoscalar ≠ 0.
pub fn skewLinesCheck(l1: PgaLine, l2: PgaLine) f64 {
    // Pseudoscalar component of L1 ∧ L2:
    // ps = l1.d1*l2.d6 - l1.d2*l2.d5 + l1.d3*l2.d4
    //    + l1.d4*l2.d3 - l1.d5*l2.d2 + l1.d6*l2.d1
    return l1.d1 * l2.d6 - l1.d2 * l2.d5 + l1.d3 * l2.d4 +
        l1.d4 * l2.d3 - l1.d5 * l2.d2 + l1.d6 * l2.d1;
}

/// Расстояние от точки до плоскости в PGA.
///
/// d(p, π) = π(p) / ‖π‖ = (a·x + b·y + c·z + d·w) / √(a²+b²+c²)
///
/// В PGA это внутреннее произведение:
///   d = (p · π) / |π|
pub fn distancePointPlane(p: PgaPoint, pi: PgaPlane) f64 {
    const inner = pi.a * p.x + pi.b * p.y + pi.c * p.z + pi.d * p.w;
    const norm_sq = pi.a * pi.a + pi.b * pi.b + pi.c * pi.c;
    if (norm_sq < 1e-30) return 0.0;
    return inner / @sqrt(norm_sq);
}

/// Принадлежность точки плоскости.
///
/// p ∈ π ⟺ π(p) = 0 ⟺ a·x + b·y + c·z + d·w = 0
pub fn pointOnPlane(p: PgaPoint, pi: PgaPlane, tol: f64) bool {
    return @abs(pi.a * p.x + pi.b * p.y + pi.c * p.z + pi.d * p.w) < tol;
}

/// Принадлежность точки прямой.
///
/// p ∈ L ⟺ L ∨ p = 0 (join = zero)
pub fn pointOnLine(p: PgaPoint, l: PgaLine, tol: f64) bool {
    // PgaLine ∨ PgaPoint: 6 условий из Plücker
    const c1 = l.d1 * p.z + l.d2 * p.y + l.d4 * p.x;
    const c2 = l.d1 * p.w + l.d3 * p.y + l.d5 * p.x;
    const c3 = l.d2 * p.w + l.d3 * p.z + l.d6 * p.x;
    const c4 = l.d4 * p.w + l.d5 * p.z + l.d6 * p.y;
    return @abs(c1) < tol and @abs(c2) < tol and @abs(c3) < tol and @abs(c4) < tol;
}

// =============================================================================
// 4. ТЕСТЫ
// =============================================================================

test "PgaPoint: from HomVec4 and back" {
    const v = HomVec4.init(1, 2, 3, 4);
    const p = PgaPoint.fromHomVec4(v);
    const v2 = p.toHomVec4();
    try std.testing.expectApproxEqAbs(v2.x, v.x, 1e-10);
    try std.testing.expectApproxEqAbs(v2.y, v.y, 1e-10);
    try std.testing.expectApproxEqAbs(v2.z, v.z, 1e-10);
    try std.testing.expectApproxEqAbs(v2.w, v.w, 1e-10);
}

test "PgaPoint: normalize" {
    const p = PgaPoint.init(3, 4, 0, 0);
    const pn = p.normalize();
    try std.testing.expectApproxEqAbs(pn.x, 0.6, 1e-10);
    try std.testing.expectApproxEqAbs(pn.y, 0.8, 1e-10);
}

test "PgaLine: Plücker condition for line through two points" {
    const p1 = PgaPoint.init(1, 0, 0, 1);
    const p2 = PgaPoint.init(0, 1, 0, 1);
    const l = PgaLine.fromTwoPoints(p1, p2);
    // Plücker: d1·d6 - d2·d5 + d3·d4 = 0
    try std.testing.expectApproxEqAbs(l.pluckerResidual(), 0.0, 1e-10);
}

test "PgaLine: Plücker condition for line through two planes" {
    const pi1 = PgaPlane.init(1, 0, 0, 0); // X = 0
    const pi2 = PgaPlane.init(0, 1, 0, 0); // Y = 0
    const l = PgaLine.fromTwoPlanes(pi1, pi2);
    try std.testing.expectApproxEqAbs(l.pluckerResidual(), 0.0, 1e-10);
}

test "skewLinesCheck: intersecting lines → 0" {
    // Две прямые в плоскости XY, обе через (0,0,0,1)
    const p1 = PgaPoint.init(1, 0, 0, 1);
    const p2 = PgaPoint.init(0, 1, 0, 1);
    const p3 = PgaPoint.init(1, 1, 0, 1);
    const l1 = PgaLine.fromTwoPoints(p1, p2);
    const l2 = PgaLine.fromTwoPoints(p1, p3);
    // Пересекающиеся → skew = 0
    try std.testing.expectApproxEqAbs(skewLinesCheck(l1, l2), 0.0, 1e-10);
}

test "skewLinesCheck: truly skew lines → ≠ 0" {
    // Скрещенные прямые: одна через (0,0,0)→(1,0,0), другая через (0,1,1)→(0,1,2)
    const p1 = PgaPoint.init(0, 0, 0, 1);
    const p2 = PgaPoint.init(1, 0, 0, 1);
    const p3 = PgaPoint.init(0, 1, 1, 1);
    const p4 = PgaPoint.init(0, 1, 2, 1);
    const l1 = PgaLine.fromTwoPoints(p1, p2);
    const l2 = PgaLine.fromTwoPoints(p3, p4);
    const ps = skewLinesCheck(l1, l2);
    // Скрещенные → pseudoscalar ≠ 0
    try std.testing.expect(@abs(ps) > 1e-10);
}

test "distancePointPlane: point on plane" {
    const p = PgaPoint.init(0, 0, 0, 1); // origin
    const pi = PgaPlane.init(0, 0, 1, 0); // Z = 0
    try std.testing.expectApproxEqAbs(distancePointPlane(p, pi), 0.0, 1e-10);
}

test "distancePointPlane: point off plane" {
    const p = PgaPoint.init(0, 0, 5, 1); // z = 5
    const pi = PgaPlane.init(0, 0, 1, 0); // Z = 0
    try std.testing.expectApproxEqAbs(distancePointPlane(p, pi), 5.0, 1e-10);
}

test "distancePointPlane: point NOT on plane" {
    const p = PgaPoint.init(3, 4, 0, 1);
    const pi = PgaPlane.init(3.0 / 5.0, 4.0 / 5.0, 0, -5.0 / 5.0); // 0.6x + 0.8y - 1 = 0, normalized
    const d = distancePointPlane(p, pi);
    // inner = 0.6*3 + 0.8*4 + 0*0 + (-1)*1 = 1.8 + 3.2 - 1 = 4.0
    // norm = sqrt(0.6² + 0.8² + 0²) = 1.0
    // distance = 4.0 / 1.0 = 4.0
    try std.testing.expectApproxEqAbs(d, 4.0, 1e-10);
}

test "pointOnPlane: origin on Z=0" {
    const p = PgaPoint.init(0, 0, 0, 1);
    const pi = PgaPlane.init(0, 0, 1, 0);
    try std.testing.expect(pointOnPlane(p, pi, 1e-10));
}

test "pointOnPlane: point off plane" {
    const p = PgaPoint.init(0, 0, 1, 1);
    const pi = PgaPlane.init(0, 0, 1, 0);
    try std.testing.expect(!pointOnPlane(p, pi, 1e-10));
}
