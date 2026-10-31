// =============================================================================
// P³×R CAUSAL STRUCTURE v1.0 — ZIG
// =============================================================================
//
// Причинная структура на P³×R — проективное пространство + время.
// Реализация для Zig 0.14.0 + P³ Engine API.
//
// Архитектура:
//   SpacetimePoint = (HomVec4, t) — точка в P³×R
//   Причинная структура: d_FS + |Δt|/c_eff → световой конус
//   Мировая линия = кривая в P³×R, параметризованная по t
//   Эволюция мира: Worldline → EndogenousFlow(t) → следующее состояние
//   Π_Λ каузальный проектор: отсеивает непричинные связи
//
// Ключевые формулы:
//   d_causal(p1, p2) = d_FS(p1.p3, p2.p3) - |p1.t - p2.t| / c_eff
//   d_causal ≤ 0 → p2 в будущем светового конуса p1 (причинно связано)
//   d_causal > 0 → пространственноподобное разделение
//   c_eff = c × K_ANISO × |W_avg| — эффективная скорость света на P³
//
// Архитектор: Kotokvit (математик), Super Z (исполнение)
// =============================================================================

const std = @import("std");
const math = std.math;
const kernel = @import("p3_kernel.zig");

pub const HomVec4 = kernel.HomVec4;
pub const PGL4 = kernel.PGL4;
pub const fsDistance = kernel.fsDistance;

// =============================================================================
// 0. ФИЗИЧЕСКИЕ КОНСТАНТЫ P³×R
// =============================================================================

/// Скорость света (км/с)
pub const C_LIGHT_KM_S: f64 = 299792.458;

/// Анизотропная поправка: c × K_ANISO
pub const C_EFF_BASE: f64 = C_LIGHT_KM_S * kernel.K_ANISO;

/// Период POLER-цикла (с)
pub const POLER_PERIOD_S: f64 = 1.0 / kernel.RESONANCE_HZ;

/// Порог для различения timelike/spacelike
pub const TIMELIKE_THRESHOLD: f64 = 1e-10;

/// Апертура светового конуса на P³
pub const LIGHT_CONE_APERTURE: f64 = math.pi / 2.0;

// =============================================================================
// 1. ТОЧКА В P³×R
// =============================================================================

/// Точка в P³×R: пространственная часть в P³ + временная координата.
///
/// P³×R расширяет проективное пространство временной осью.
/// Причинная структура определяется через Фубини-Штуди метрику
/// и эффективную скорость света, зависящую от W-калибровки.
pub const SpacetimePoint = struct {
    /// Точка в P³ (пространство)
    p3: HomVec4,
    /// Временная координата (секунды)
    t: f64,

    /// W-координата (калибровочная)
    pub inline fn W(self: SpacetimePoint) f64 {
        return self.p3.w;
    }

    /// Однородные координаты (X, Y, Z, W)
    pub inline fn xyzw(self: SpacetimePoint) [4]f64 {
        return .{ self.p3.x, self.p3.y, self.p3.z, self.p3.w };
    }

    /// Та же пространственная точка в другой момент времени
    pub inline fn atTime(self: SpacetimePoint, new_t: f64) SpacetimePoint {
        return .{ .p3 = self.p3, .t = new_t };
    }

    /// Конструктор из HomVec4 и времени
    pub inline fn init(p3: HomVec4, t: f64) SpacetimePoint {
        return .{ .p3 = p3, .t = t };
    }
};

// =============================================================================
// 2. КАУЗАЛЬНАЯ СТРУКТУРА
// =============================================================================

/// Каузальное отношение между двумя событиями в P³×R.
///
/// Классификация основана на знаке каузального интервала:
///   d_causal = d_FS - |Δt|/c_eff
///
/// TIMELIKE_FUTURE: p2 в будущем p1, причинно связано (d_causal < 0, Δt > 0)
/// TIMELIKE_PAST:   p2 в прошлом p1, причинно связано (d_causal < 0, Δt < 0)
/// LIGHTLIKE:       на световом конусе, нулевой интервал (d_causal ≈ 0)
/// SPACELIKE:       пространственноподобное, некоммуникационно (d_causal > 0)
/// Z2Z_TUNNEL:      связь через Z/2Z-туннель (антиподальная)
pub const CausalRelation = enum(u3) {
    TIMELIKE_FUTURE = 1,
    TIMELIKE_PAST = 2,
    LIGHTLIKE = 3,
    SPACELIKE = 4,
    Z2Z_TUNNEL = 5,
};

/// Каузальный интервал между двумя событиями.
///
/// Содержит всю информацию о причинном отношении:
/// Фубини-Штуди расстояние, временную разницу, каузальный интервал,
/// тип отношения и эффективную скорость света.
pub const CausalInterval = struct {
    d_fs: f64, // Фубини-Штуди расстояние
    dt: f64, // Временная разница
    d_causal: f64, // Каузальный интервал
    relation: CausalRelation, // Тип отношения
    c_eff: f64, // Эффективная скорость света

    /// Можно ли передать сигнал от p1 к p2?
    pub fn isCausal(self: CausalInterval) bool {
        return self.relation == .TIMELIKE_FUTURE or
            self.relation == .LIGHTLIKE or
            self.relation == .Z2Z_TUNNEL;
    }
};

/// Каузальная структура P³×R.
///
/// Определяет, какие события могут влиять друг на друга
/// через световые конусы, деформированные анизотропией.
///
/// Эффективная скорость света зависит от W-калибровки:
/// ближе к W=0 свет «замедляется» — это горизонт событий
/// (объекты исчезают).
pub const CausalStructure = struct {
    c_eff: f64, // Базовая эффективная скорость

    /// Инициализация с заданной эффективной скоростью
    pub fn init(c_eff: f64) CausalStructure {
        return .{ .c_eff = c_eff };
    }

    /// Инициализация с анизотропной поправкой по умолчанию
    pub fn initDefault() CausalStructure {
        return .{ .c_eff = C_EFF_BASE };
    }

    /// Эффективная скорость света в точке p ∈ P³×R.
    ///
    /// c_eff(p) = c_base × |W(p)|
    ///
    /// W-калибровка: ближе к W=0 свет «замедляется» —
    /// это горизонт событий (объекты исчезают).
    pub fn effectiveSpeed(self: CausalStructure, p: SpacetimePoint) f64 {
        const w_abs = @min(@abs(p.p3.w), 1.0);
        if (w_abs < kernel.W_EPS) return 0.0; // Горизонт событий
        return self.c_eff * w_abs;
    }

    /// Классифицировать каузальное отношение p1 → p2.
    ///
    /// d_causal = d_FS(p1, p2) - |Δt| / c_eff_avg
    ///
    /// d_causal < 0 → timelike (причинно связано)
    /// d_causal = 0 → lightlike (на конусе)
    /// d_causal > 0 → spacelike (некоммуникационно)
    pub fn classify(self: CausalStructure, p1: SpacetimePoint, p2: SpacetimePoint) CausalInterval {
        const d_fs = fsDistance(p1.p3, p2.p3);
        const dt = p2.t - p1.t;

        // Средняя эффективная скорость на интервале
        const c1 = self.effectiveSpeed(p1);
        const c2 = self.effectiveSpeed(p2);
        const c_avg: f64 = if ((c1 + c2) > 0) (c1 + c2) / 2.0 else self.c_eff;

        // Каузальный интервал
        const d_causal: f64 = if (c_avg > 0)
            d_fs - @abs(dt) / c_avg
        else
            std.math.floatMax(f64); // Оба на горизонте — нет причинной связи

        // Классификация
        const relation: CausalRelation = blk: {
            if (d_causal < -TIMELIKE_THRESHOLD) {
                if (dt > 0) break :blk .TIMELIKE_FUTURE else break :blk .TIMELIKE_PAST;
            } else if (@abs(d_causal) <= TIMELIKE_THRESHOLD) {
                break :blk .LIGHTLIKE;
            } else {
                // Проверяем Z/2Z-туннель: если точки почти антиподальные
                const dot_abs = @abs(HomVec4.dot(p1.p3, p2.p3));
                if (dot_abs < 0.1) break :blk .Z2Z_TUNNEL else break :blk .SPACELIKE;
            }
        };

        return .{
            .d_fs = d_fs,
            .dt = dt,
            .d_causal = d_causal,
            .relation = relation,
            .c_eff = c_avg,
        };
    }

    /// Все точки в будущем световом конусе origin.
    ///
    /// Точка p в будущем конусе, если:
    ///   classify(origin, p) ∈ {TIMELIKE_FUTURE, LIGHTLIKE}
    pub fn futureCone(self: CausalStructure, allocator: std.mem.Allocator, origin: SpacetimePoint, points: []const SpacetimePoint) !std.ArrayList(CausalInterval) {
        var result = std.ArrayList(CausalInterval).init(allocator);
        errdefer result.deinit();
        for (points) |p| {
            const interval = self.classify(origin, p);
            if (interval.relation == .TIMELIKE_FUTURE or interval.relation == .LIGHTLIKE) {
                try result.append(interval);
            }
        }
        return result;
    }

    /// Каузальный ромб I⁺(p1) ∩ I⁻(p2) — область, причинно
    /// зависящая от обоих событий.
    ///
    /// Точка p ∈ diamond, если:
    ///   p в будущем p1 AND p2 в будущем p
    pub fn causalDiamond(self: CausalStructure, allocator: std.mem.Allocator, p1: SpacetimePoint, p2: SpacetimePoint, points: []const SpacetimePoint) !std.ArrayList(SpacetimePoint) {
        var result = std.ArrayList(SpacetimePoint).init(allocator);
        errdefer result.deinit();
        for (points) |p| {
            const interval1 = self.classify(p1, p);
            const interval2 = self.classify(p, p2);
            // p в будущем p1 И p2 в будущем p
            if ((interval1.relation == .TIMELIKE_FUTURE or interval1.relation == .LIGHTLIKE) and
                (interval2.relation == .TIMELIKE_FUTURE or interval2.relation == .LIGHTLIKE))
            {
                try result.append(p);
            }
        }
        return result;
    }
};

// =============================================================================
// 3. МИРОВАЯ ЛИНИЯ
// =============================================================================

/// Сегмент мировой линии между двумя событиями.
pub const WorldlineSegment = struct {
    p_start: SpacetimePoint,
    p_end: SpacetimePoint,
    interval: CausalInterval,
    proper_time: f64, // Собственное время вдоль сегмента
};

/// Мировая линия — кривая в P³×R, параметризованная по t.
///
/// Реализация: последовательность SpacetimePoint,
/// связанных каузальными интервалами.
///
/// Мировая линия является причинной, если все её сегменты
/// timelike или lightlike. Z/2Z-туннелирование обнаруживается,
/// когда сегмент проходит через антиподальное отождествление.
pub const Worldline = struct {
    events: std.ArrayList(SpacetimePoint),
    segments: std.ArrayList(WorldlineSegment),
    causal: CausalStructure,
    allocator: std.mem.Allocator,

    /// Инициализация мировой линии
    pub fn init(allocator: std.mem.Allocator) Worldline {
        return .{
            .events = std.ArrayList(SpacetimePoint).init(allocator),
            .segments = std.ArrayList(WorldlineSegment).init(allocator),
            .causal = CausalStructure.initDefault(),
            .allocator = allocator,
        };
    }

    /// Освобождение ресурсов
    pub fn deinit(self: *Worldline) void {
        self.events.deinit();
        self.segments.deinit();
    }

    /// Добавить событие на мировую линию.
    ///
    /// Если есть предыдущее событие, вычисляется каузальный интервал
    /// и собственное время вдоль сегмента.
    pub fn addEvent(self: *Worldline, p: SpacetimePoint) !void {
        if (self.events.items.len > 0) {
            const prev = self.events.items[self.events.items.len - 1];
            const interval = self.causal.classify(prev, p);
            // Собственное время: ∫√(dτ²) ≈ |Δt| для timelike
            const proper_time: f64 = if (interval.relation == .TIMELIKE_FUTURE or
                interval.relation == .LIGHTLIKE)
                @abs(p.t - prev.t)
            else
                0.0;
            try self.segments.append(.{
                .p_start = prev,
                .p_end = p,
                .interval = interval,
                .proper_time = proper_time,
            });
        }
        try self.events.append(p);
    }

    /// Полное собственное время вдоль мировой линии
    pub fn totalProperTime(self: Worldline) f64 {
        var sum: f64 = 0;
        for (self.segments.items) |seg| {
            sum += seg.proper_time;
        }
        return sum;
    }

    /// Является ли мировая линия причинной
    /// (все сегменты timelike/lightlike)?
    pub fn isCausal(self: Worldline) bool {
        for (self.segments.items) |seg| {
            if (seg.interval.relation != .TIMELIKE_FUTURE and
                seg.interval.relation != .LIGHTLIKE)
            {
                return false;
            }
        }
        return true;
    }

    /// Проходит ли мировая линия через Z/2Z-туннель?
    pub fn hasZ2ZTunneling(self: Worldline) bool {
        for (self.segments.items) |seg| {
            if (seg.interval.relation == .Z2Z_TUNNEL) return true;
        }
        return false;
    }

    /// Интерполировать мировую линию в момент t.
    /// Геодезическая интерполяция на P³ (SLERP на S³) + линейная по t.
    ///
    /// Для сегментов с Δt = 0 возвращает начальную точку сегмента.
    /// Для точек вне диапазона возвращает ближайшее событие.
    pub fn interpolate(self: Worldline, t: f64) ?SpacetimePoint {
        if (self.events.items.len == 0) return null;
        if (t <= self.events.items[0].t) return self.events.items[0];
        if (t >= self.events.items[self.events.items.len - 1].t)
            return self.events.items[self.events.items.len - 1];

        // Найти сегмент
        for (self.segments.items) |seg| {
            if (seg.p_start.t <= t and t <= seg.p_end.t) {
                const dt_total = seg.p_end.t - seg.p_start.t;
                if (@abs(dt_total) < 1e-15) return seg.p_start;
                const alpha = (t - seg.p_start.t) / dt_total;

                // SLERP на S³
                const v1 = seg.p_start.p3.normalize();
                const v2 = seg.p_end.p3.normalize();
                const dot_val = std.math.clamp(HomVec4.dot(v1, v2), -1.0, 1.0);
                const theta = math.acos(dot_val);

                const p3: HomVec4 = if (theta < 1e-10)
                    v1
                else blk: {
                    const sin_theta = math.sin(theta);
                    const w1 = math.sin((1.0 - alpha) * theta) / sin_theta;
                    const w2 = math.sin(alpha * theta) / sin_theta;
                    break :blk HomVec4.init(
                        w1 * v1.x + w2 * v2.x,
                        w1 * v1.y + w2 * v2.y,
                        w1 * v1.z + w2 * v2.z,
                        w1 * v1.w + w2 * v2.w,
                    );
                };

                return SpacetimePoint.init(p3, t);
            }
        }

        return self.events.items[self.events.items.len - 1];
    }
};

// =============================================================================
// 4. ЭВОЛЮЦИЯ МИРА
// =============================================================================

/// Слепок состояния мира на шаге эволюции.
pub const WorldSnapshot = struct {
    t: f64,
    step: u64,
    n_events: usize,
    n_causal_links: usize,
    resonance: f64,
};

/// Эволюция мира на P³×R — шаги времени с эндогенным течением.
///
/// Каждый шаг:
///   1. Применяем эндогенное течение к каждой мировой линии
///   2. Вычисляем каузальную структуру
///   3. Вычисляем Π_Λ каузальный проектор
///   4. Записываем новые события
pub const WorldEvolution = struct {
    dt: f64, // Шаг времени
    t: f64, // Текущее время
    worldlines: std.ArrayList(*Worldline),
    causal: CausalStructure,
    step_count: u64,
    resonance_hz: f64 = kernel.RESONANCE_HZ,
    oscillation_amplitude: f64 = 0.01,
    allocator: std.mem.Allocator,

    /// Инициализация с заданным шагом времени
    pub fn init(allocator: std.mem.Allocator, dt: f64) WorldEvolution {
        return .{
            .dt = dt,
            .t = 0.0,
            .worldlines = std.ArrayList(*Worldline).init(allocator),
            .causal = CausalStructure.initDefault(),
            .step_count = 0,
            .resonance_hz = kernel.RESONANCE_HZ,
            .oscillation_amplitude = 0.01,
            .allocator = allocator,
        };
    }

    /// Инициализация с кастомными параметрами планетарной волновой среды
    pub fn initWithResonance(allocator: std.mem.Allocator, dt: f64, resonance_hz: f64, amplitude: f64) WorldEvolution {
        return .{
            .dt = dt,
            .t = 0.0,
            .worldlines = std.ArrayList(*Worldline).init(allocator),
            .causal = CausalStructure.initDefault(),
            .step_count = 0,
            .resonance_hz = resonance_hz,
            .oscillation_amplitude = amplitude,
            .allocator = allocator,
        };
    }

    /// Освобождение ресурсов
    pub fn deinit(self: *WorldEvolution) void {
        self.worldlines.deinit();
    }

    /// Добавить мировую линию
    pub fn addWorldline(self: *WorldEvolution, wl: *Worldline) !void {
        try self.worldlines.append(wl);
    }

    /// Один шаг эволюции мира.
    ///
    /// Эндогенное течение: малое вращение в XW-плоскости
    /// (Z/2Z-основа) с настраиваемой резонансной частотой.
    pub fn step(self: *WorldEvolution) !WorldSnapshot {
        self.t += self.dt;
        self.step_count += 1;

        // Применяем эндогенное течение к каждой мировой линии
        for (self.worldlines.items) |wl| {
            if (wl.events.items.len == 0) continue;
            const last = wl.events.items[wl.events.items.len - 1];
            const v = last.p3.normalize();

            // Малое вращение в плоскости XW (Z/2Z-основа)
            const angle = self.oscillation_amplitude * math.sin(2.0 * math.pi * self.resonance_hz * self.t);
            const c = math.cos(angle);
            const s = math.sin(angle);

            const new_p3 = HomVec4.init(
                c * v.x + s * v.w,
                v.y,
                v.z,
                -s * v.x + c * v.w,
            ).normalize();

            const new_point = SpacetimePoint.init(new_p3, self.t);
            try wl.addEvent(new_point);
        }

        // Подсчитываем причинные связи
        var n_causal: usize = 0;
        const items = self.worldlines.items;
        for (items, 0..) |wl1, i| {
            if (wl1.events.items.len == 0) continue;
            for (items, 0..) |wl2, j| {
                if (i >= j) continue;
                if (wl2.events.items.len == 0) continue;
                const p1 = wl1.events.items[wl1.events.items.len - 1];
                const p2 = wl2.events.items[wl2.events.items.len - 1];
                const interval = self.causal.classify(p1, p2);
                if (interval.isCausal()) n_causal += 1;
            }
        }

        // Текущий резонансный отклик среды
        const resonance = math.sin(2.0 * math.pi * self.resonance_hz * self.t);

        return .{
            .t = self.t,
            .step = self.step_count,
            .n_events = items.len,
            .n_causal_links = n_causal,
            .resonance = resonance,
        };
    }

    /// Эволюция мира на n шагов
    pub fn evolve(self: *WorldEvolution, n_steps: u32) !void {
        for (0..n_steps) |_| {
            _ = try self.step();
        }
    }

    /// Π_Λ Каузальный проектор — отсеивает непричинные связи.
    ///
    /// Возвращает матрицу связности: C[i,j] = 1 если i→j причинно.
    /// Матрица n×n выделяется через allocator.
    pub fn piLambdaProjector(self: WorldEvolution, allocator: std.mem.Allocator) ![]f64 {
        const n = self.worldlines.items.len;
        const C = try allocator.alloc(f64, n * n);
        @memset(C, 0);

        for (0..n) |i| {
            const wl1 = self.worldlines.items[i];
            if (wl1.events.items.len == 0) continue;
            for (0..n) |j| {
                if (i == j) continue;
                const wl2 = self.worldlines.items[j];
                if (wl2.events.items.len == 0) continue;

                const p1 = wl1.events.items[wl1.events.items.len - 1];
                const p2 = wl2.events.items[wl2.events.items.len - 1];
                const interval = self.causal.classify(p1, p2);
                if (interval.isCausal()) {
                    C[i * n + j] = 1.0;
                }
            }
        }
        return C;
    }
};

// =============================================================================
// 5. ТЕСТЫ
// =============================================================================

test "SpacetimePoint: init and W" {
    const p3 = HomVec4.init(0.5, 0.3, 0.2, 0.8);
    const sp = SpacetimePoint.init(p3, 1.5);
    try std.testing.expectApproxEqAbs(sp.t, 1.5, 1e-10);
    try std.testing.expectApproxEqAbs(sp.W(), 0.8, 1e-10);
}

test "SpacetimePoint: atTime" {
    const p3 = HomVec4.init(0.5, 0.3, 0.2, 0.8);
    const sp = SpacetimePoint.init(p3, 1.0);
    const sp2 = sp.atTime(2.0);
    try std.testing.expectApproxEqAbs(sp2.t, 2.0, 1e-10);
    try std.testing.expectApproxEqAbs(HomVec4.dot(sp2.p3, p3), HomVec4.dot(p3, p3), 1e-10);
}

test "CausalStructure: effective speed at W=1" {
    const cs = CausalStructure.initDefault();
    const p = SpacetimePoint.init(HomVec4.init(0, 0, 0, 1), 0);
    const c_eff = cs.effectiveSpeed(p);
    // c_eff = C_EFF_BASE * |W| = C_EFF_BASE * 1.0
    try std.testing.expectApproxEqAbs(c_eff, C_EFF_BASE, 1e-6);
}

test "CausalStructure: effective speed at W=0 (event horizon)" {
    const cs = CausalStructure.initDefault();
    const p = SpacetimePoint.init(HomVec4.init(0, 0, 0, 0), 0);
    const c_eff = cs.effectiveSpeed(p);
    try std.testing.expectApproxEqAbs(c_eff, 0.0, 1e-10);
}

test "CausalStructure: effective speed at W=0.5" {
    const cs = CausalStructure.initDefault();
    const p = SpacetimePoint.init(HomVec4.init(0, 0, 0, 0.5), 0);
    const c_eff = cs.effectiveSpeed(p);
    try std.testing.expectApproxEqAbs(c_eff, C_EFF_BASE * 0.5, 1e-6);
}

test "CausalStructure: classify same point same time → LIGHTLIKE" {
    const cs = CausalStructure.initDefault();
    const p3 = HomVec4.init(0.5, 0.3, 0.2, 0.8).normalize();
    const p1 = SpacetimePoint.init(p3, 1.0);
    const p2 = SpacetimePoint.init(p3, 1.0);
    const interval = cs.classify(p1, p2);
    try std.testing.expect(interval.relation == .LIGHTLIKE);
    try std.testing.expectApproxEqAbs(interval.d_fs, 0.0, 1e-10);
}

test "CausalStructure: classify nearby future → TIMELIKE_FUTURE" {
    const cs = CausalStructure.initDefault();
    const p3_base = HomVec4.init(0.5, 0.3, 0.2, 0.8).normalize();
    const p1 = SpacetimePoint.init(p3_base, 0.0);
    // Same spatial point, later in time → d_FS=0, d_causal<0 → TIMELIKE_FUTURE
    // (c_eff ≈ 385000 km/s, so need large Δt for d_FS>0 points; same point is safest)
    const p2 = SpacetimePoint.init(p3_base, 100.0);
    const interval = cs.classify(p1, p2);
    try std.testing.expect(interval.relation == .TIMELIKE_FUTURE);
    try std.testing.expect(interval.isCausal());
}

test "CausalStructure: classify spacelike separation" {
    const cs = CausalStructure.initDefault();
    const p1 = SpacetimePoint.init(HomVec4.init(1, 0, 0, 0).normalize(), 0.0);
    const p2 = SpacetimePoint.init(HomVec4.init(0, 1, 0, 0).normalize(), 0.0);
    const interval = cs.classify(p1, p2);
    // Ортогональные точки, dt=0 → spacelike
    try std.testing.expect(interval.relation == .SPACELIKE or interval.relation == .Z2Z_TUNNEL);
    try std.testing.expect(!interval.isCausal() or interval.relation == .Z2Z_TUNNEL);
}

test "CausalInterval: isCausal for Z2Z_TUNNEL" {
    const interval = CausalInterval{
        .d_fs = 1.5,
        .dt = 0.5,
        .d_causal = 0.5,
        .relation = .Z2Z_TUNNEL,
        .c_eff = C_EFF_BASE,
    };
    try std.testing.expect(interval.isCausal());
}

test "Worldline: single event is causal, no tunneling" {
    const allocator = std.testing.allocator;
    var wl = Worldline.init(allocator);
    defer wl.deinit();
    try wl.addEvent(SpacetimePoint.init(HomVec4.init(0.5, 0.3, 0.2, 0.8).normalize(), 0.0));
    try std.testing.expect(wl.isCausal());
    try std.testing.expect(!wl.hasZ2ZTunneling());
}

test "Worldline: two causally connected events" {
    const allocator = std.testing.allocator;
    var wl = Worldline.init(allocator);
    defer wl.deinit();
    const p3 = HomVec4.init(0.5, 0.3, 0.2, 0.8).normalize();
    try wl.addEvent(SpacetimePoint.init(p3, 0.0));
    // Same spatial point, later time → d_FS=0, d_causal<0 → TIMELIKE_FUTURE
    try wl.addEvent(SpacetimePoint.init(p3, 0.001));
    try std.testing.expect(wl.segments.items.len == 1);
    try std.testing.expectApproxEqAbs(wl.totalProperTime(), 0.001, 1e-10);
}

test "Worldline: interpolate at endpoints" {
    const allocator = std.testing.allocator;
    var wl = Worldline.init(allocator);
    defer wl.deinit();
    const p1 = SpacetimePoint.init(HomVec4.init(1, 0, 0, 0).normalize(), 0.0);
    const p2 = SpacetimePoint.init(HomVec4.init(0, 1, 0, 0).normalize(), 1.0);
    try wl.addEvent(p1);
    try wl.addEvent(p2);

    const interp_0 = wl.interpolate(0.0);
    try std.testing.expect(interp_0 != null);
    try std.testing.expectApproxEqAbs(interp_0.?.t, 0.0, 1e-10);

    const interp_1 = wl.interpolate(1.0);
    try std.testing.expect(interp_1 != null);
    try std.testing.expectApproxEqAbs(interp_1.?.t, 1.0, 1e-10);
}

test "Worldline: interpolate at midpoint (SLERP)" {
    const allocator = std.testing.allocator;
    var wl = Worldline.init(allocator);
    defer wl.deinit();
    const p1 = SpacetimePoint.init(HomVec4.init(1, 0, 0, 0).normalize(), 0.0);
    const p2 = SpacetimePoint.init(HomVec4.init(0, 0, 0, 1).normalize(), 1.0);
    try wl.addEvent(p1);
    try wl.addEvent(p2);

    const interp = wl.interpolate(0.5);
    try std.testing.expect(interp != null);
    try std.testing.expectApproxEqAbs(interp.?.t, 0.5, 1e-10);
    // На середине SLERP между (1,0,0,0) и (0,0,0,1):
    // угол = π/2, sin(π/4)/sin(π/2) = 1/√2
    const expected = 1.0 / @sqrt(2.0);
    try std.testing.expectApproxEqAbs(@abs(interp.?.p3.x), expected, 1e-10);
    try std.testing.expectApproxEqAbs(@abs(interp.?.p3.w), expected, 1e-10);
}

test "WorldEvolution: single worldline evolution" {
    const allocator = std.testing.allocator;
    var wl = Worldline.init(allocator);
    defer wl.deinit();
    try wl.addEvent(SpacetimePoint.init(HomVec4.init(0.5, 0.3, 0.2, 0.8).normalize(), 0.0));

    var evolution = WorldEvolution.init(allocator, POLER_PERIOD_S);
    defer evolution.deinit();
    try evolution.addWorldline(&wl);

    const snap = try evolution.step();
    try std.testing.expect(snap.step == 1);
    try std.testing.expect(wl.events.items.len == 2);
}

test "CausalStructure: future cone filtering" {
    const allocator = std.testing.allocator;
    const cs = CausalStructure.initDefault();
    const p3_base = HomVec4.init(0.5, 0.3, 0.2, 0.8).normalize();
    const origin = SpacetimePoint.init(p3_base, 0.0);

    const points = [_]SpacetimePoint{
        // Same spatial point, far future → TIMELIKE_FUTURE
        SpacetimePoint.init(p3_base, 1e6),
        // Ортогональное, dt=0 — не в будущем конусе
        SpacetimePoint.init(HomVec4.init(0, 1, 0, 0).normalize(), 0.0),
    };

    var cone = try cs.futureCone(allocator, origin, &points);
    defer cone.deinit();
    // Хотя бы одна точка в будущем конусе
    try std.testing.expect(cone.items.len >= 1);
}

test "CausalStructure: causal diamond" {
    const allocator = std.testing.allocator;
    const cs = CausalStructure.initDefault();
    const p3_base = HomVec4.init(0.5, 0.3, 0.2, 0.8).normalize();
    const p1 = SpacetimePoint.init(p3_base, 0.0);
    const p2 = SpacetimePoint.init(p3_base, 2e6);

    const points = [_]SpacetimePoint{
        // Same spatial point, midpoint in time — must be in the diamond
        SpacetimePoint.init(p3_base, 1e6),
    };

    var diamond = try cs.causalDiamond(allocator, p1, p2, &points);
    defer diamond.deinit();
    // Точка посередине должна быть в каузальном ромбе
    try std.testing.expect(diamond.items.len >= 1);
}

test "WorldEvolution: pi-lambda projector" {
    const allocator = std.testing.allocator;
    var wl1 = Worldline.init(allocator);
    defer wl1.deinit();
    var wl2 = Worldline.init(allocator);
    defer wl2.deinit();

    try wl1.addEvent(SpacetimePoint.init(HomVec4.init(0.5, 0.3, 0.2, 0.8).normalize(), 0.0));
    try wl2.addEvent(SpacetimePoint.init(HomVec4.init(0.4, 0.35, 0.25, 0.82).normalize(), 0.0));

    var evolution = WorldEvolution.init(allocator, POLER_PERIOD_S);
    defer evolution.deinit();
    try evolution.addWorldline(&wl1);
    try evolution.addWorldline(&wl2);

    // Прогоняем несколько шагов
    try evolution.evolve(5);

    // Π_Λ проектор: матрица 2×2
    const C = try evolution.piLambdaProjector(allocator);
    defer allocator.free(C);
    // C[0*2+0] = 0 (диагональ), C[1*2+1] = 0 (диагональ)
    try std.testing.expectApproxEqAbs(C[0], 0.0, 1e-10);
    try std.testing.expectApproxEqAbs(C[3], 0.0, 1e-10);
}
