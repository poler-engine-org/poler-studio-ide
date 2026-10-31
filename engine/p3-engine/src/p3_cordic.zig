// =============================================================================
// P³ CORDIC v1.0 — ZIG
// =============================================================================
//
// CORDIC (COordinate Rotation DIgital Computer) — shift-and-add алгоритм
// для вычисления тригонометрических, гиперболических, экспоненциальных
// и логарифмических функций через только бит-сдвиги и сложения.
//
// В контексте P³ Engine:
//   - Проективные преобразования требуют sin/cos (вращения)
//   - Нормализация однородных координат требует деления (через обратное)
//   - Cross-ratio требует деления
//   - На GPU/embedded: CORDIC даёт hardware-independent результаты
//   - P³ обобщение: вращение в RP³ — четверка CORDIC для Sim(3)
//
// РЕЖИМЫ:
//   1. Circular (m = +1):  sin, cos, atan, atan2, asin, acos  [CORDIC]
//   2. Linear   (m =  0):  mul, div                            [CORDIC]
//   3. Hyperbolic (m = -1): sinh, cosh, exp, ln, sqrt          [ряды/Newton]
//
// ИТЕРАЦИЯ CORDIC (circular):
//   x' = x - σ·2^{-i}·y,   y' = y + σ·2^{-i}·x,   z' = z - σ·α_i
//   где α_i = atan(2^{-i})
//
// СХОДИМОСТЬ:
//   Circular: |z| ≤ π/2 ≈ 1.5708
//
// ТОЧНОСТЬ: f32 → 24 итерации (24 бита мантиссы)
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const math = std.math;

// =============================================================================
// 1. CORDIC CONSTANTS
// =============================================================================

/// Количество итераций для f32 точности
pub const ITERS_F32: usize = 24;

/// Gain factor K_n для circular CORDIC (24 итерации).
/// K_n = ∏ √(1 + 2^{-2i}) ≈ 1.646760258121
pub const CIRCULAR_GAIN_F32: f32 = 1.646760258121;

/// 1/K — константа для коррекции gain
pub const CIRCULAR_GAIN_INV_F32: f32 = 1.0 / CIRCULAR_GAIN_F32;

/// Конвергентная зона circular CORDIC: |z| ≤ π/2
pub const CIRCULAR_CONV_ZONE: f32 = math.pi / 2.0;

// =============================================================================
// 2. LOOKUP TABLES — atan(2^{-i})
// =============================================================================

/// atan(2^{-i}) для i = 0..23 — circular CORDIC micro-rotations
pub const ATAN_TABLE: [ITERS_F32]f32 = .{
    7.8539816e-01,  // atan(1) = π/4
    4.6364761e-01,  // atan(1/2)
    2.4497866e-01,  // atan(1/4)
    1.2435499e-01,
    6.2418810e-02,
    3.1239833e-02,
    1.5623729e-02,
    7.8123411e-03,
    3.9062301e-03,
    1.9531225e-03,
    9.7656219e-04,
    4.8828121e-04,
    2.4414062e-04,
    1.2207031e-04,
    6.1035156e-05,
    3.0517578e-05,
    1.5258789e-05,
    7.6293945e-06,
    3.8146973e-06,
    1.9073486e-06,
    9.5367432e-07,
    4.7683716e-07,
    2.3841858e-07,
    1.1920929e-07,
};

/// Степени двойки 2^{-i} как f32
pub const POW2_NEG: [ITERS_F32]f32 = .{
    1.0,        0.5,        0.25,       0.125,
    0.0625,     0.03125,    0.015625,   0.0078125,
    0.00390625, 0.00195313, 0.00097656, 0.00048828,
    0.00024414, 0.00012207, 6.1035e-05, 3.0518e-05,
    1.5259e-05, 7.6294e-06, 3.8147e-06, 1.9073e-06,
    9.5367e-07, 4.7684e-07, 2.3842e-07, 1.1921e-07,
};

// =============================================================================
// 3. CIRCULAR CORDIC — sin, cos, atan, atan2
// =============================================================================

/// Результат circular CORDIC: (cos θ, sin θ) одновременно.
pub const SinCos = struct {
    cos: f32,
    sin: f32,
};

/// Circular CORDIC rotation mode: вычисляет (cos θ, sin θ).
pub fn circularRotation(theta: f32) SinCos {
    var angle = theta;
    var sign_flip = false;

    if (angle > CIRCULAR_CONV_ZONE) {
        angle = math.pi - angle;
        sign_flip = true;
    } else if (angle < -CIRCULAR_CONV_ZONE) {
        angle = -math.pi - angle;
        sign_flip = true;
    }

    var x: f32 = CIRCULAR_GAIN_INV_F32;
    var y: f32 = 0.0;
    var z: f32 = angle;

    for (0..ITERS_F32) |i| {
        const sigma: f32 = if (z >= 0) 1.0 else -1.0;
        const x_new = x - sigma * POW2_NEG[i] * y;
        const y_new = y + sigma * POW2_NEG[i] * x;
        const z_new = z - sigma * ATAN_TABLE[i];
        x = x_new;
        y = y_new;
        z = z_new;
    }

    if (sign_flip) {
        return .{ .cos = -x, .sin = y };
    }
    return .{ .cos = x, .sin = y };
}

/// sin(θ) через CORDIC.
pub fn sin(theta: f32) f32 {
    var a = @mod(theta, 2.0 * math.pi);
    if (a > math.pi) a -= 2.0 * math.pi;
    if (a < -math.pi) a += 2.0 * math.pi;
    return circularRotation(a).sin;
}

/// cos(θ) через CORDIC.
pub fn cos(theta: f32) f32 {
    var a = @mod(theta, 2.0 * math.pi);
    if (a > math.pi) a -= 2.0 * math.pi;
    if (a < -math.pi) a += 2.0 * math.pi;
    return circularRotation(a).cos;
}

/// Результат CORDIC atan2: (angle, magnitude).
pub const PolarF32 = struct {
    angle: f32,
    magnitude: f32,
};

/// Circular CORDIC vectoring mode: atan2(y, x) и √(x² + y²).
pub fn circularVectoring(y_in: f32, x_in: f32) PolarF32 {
    if (x_in == 0 and y_in == 0) return .{ .angle = 0, .magnitude = 0 };
    if (x_in == 0) {
        if (y_in > 0) return .{ .angle = math.pi / 2.0, .magnitude = @abs(y_in) };
        return .{ .angle = -math.pi / 2.0, .magnitude = @abs(y_in) };
    }
    if (y_in == 0) {
        if (x_in > 0) return .{ .angle = 0, .magnitude = x_in };
        return .{ .angle = math.pi, .magnitude = -x_in };
    }

    var x: f32 = x_in;
    var y: f32 = y_in;
    var angle_offset: f32 = 0;

    if (x < 0) {
        x = -x;
        y = -y;
        angle_offset = math.pi;
    }

    var z: f32 = 0;

    for (0..ITERS_F32) |i| {
        const sigma: f32 = if (y <= 0) 1.0 else -1.0;
        const x_new = x - sigma * POW2_NEG[i] * y;
        const y_new = y + sigma * POW2_NEG[i] * x;
        const z_new = z - sigma * ATAN_TABLE[i];
        x = x_new;
        y = y_new;
        z = z_new;
    }

    return .{
        .angle = z + angle_offset,
        .magnitude = x * CIRCULAR_GAIN_INV_F32,
    };
}

/// atan(y) через CORDIC.
pub fn atan(y: f32) f32 {
    return circularVectoring(y, 1.0).angle;
}

/// atan2(y, x) через CORDIC.
pub fn atan2(y: f32, x: f32) f32 {
    return circularVectoring(y, x).angle;
}

/// asin(x) через CORDIC.
pub fn asin(x: f32) f32 {
    if (@abs(x) > 1.0) return if (x > 0) math.pi / 2.0 else -math.pi / 2.0;
    return atan2(x, @sqrt(1.0 - x * x));
}

/// acos(x) через CORDIC.
pub fn acos(x: f32) f32 {
    if (@abs(x) > 1.0) return if (x > 0) 0 else math.pi;
    return atan2(@sqrt(1.0 - x * x), x);
}

// =============================================================================
// 4. LINEAR CORDIC — умножение и деление (binary decomposition)
// =============================================================================

/// Умножение через binary decomposition (CORDIC linear mode).
///
/// Разлагаем z = Σ σ_i · 2^{-i} и накапливаем y = x · z.
/// Работает для любого z (нормализация через декомпозицию).
pub fn linearRotation(x: f32, z: f32) f32 {
    // Для |z| > 1: разбиваем на целую и дробную часть
    // z = n + f, где n — целая, f — дробная
    // x·z = x·n + x·f
    if (@abs(z) <= 1.0) {
        return linearRotationCore(x, z);
    }

    const sign: f32 = if (z >= 0) 1.0 else -1.0;
    const az = @abs(z);
    const int_part: f32 = @floor(az);
    const frac_part: f32 = az - int_part;

    const int_result = x * int_part; // целая часть — просто умножение
    const frac_result = linearRotationCore(x, frac_part);

    return sign * (int_result + frac_result);
}

/// Core linear CORDIC: x · z для |z| ≤ 1.
fn linearRotationCore(x: f32, z: f32) f32 {
    var y: f32 = 0;
    var zv = z;

    for (0..ITERS_F32) |i| {
        const sigma: f32 = if (zv >= 0) 1.0 else -1.0;
        y += sigma * POW2_NEG[i] * x;
        zv -= sigma * POW2_NEG[i];
    }

    return y;
}

/// Деление через binary decomposition (CORDIC linear vectoring mode).
///
/// Накапливаем z = y/x, управляя y к нулю.
/// Работает для любого отношения (нормализация).
pub fn linearVectoring(y_in: f32, x_in: f32) f32 {
    if (@abs(x_in) < 1e-30) return if (y_in >= 0) math.floatMax(f32) else -math.floatMax(f32);

    // Нормализация: если |y/x| > 1, разбиваем
    const ratio = @abs(y_in / x_in);
    if (ratio <= 1.0) {
        return linearVectoringCore(y_in, x_in);
    }

    // |y| > |x|: вычисляем x/y и берём обратное
    const inv = linearVectoringCore(x_in, y_in);
    if (@abs(inv) < 1e-30) return math.floatMax(f32);
    return 1.0 / inv;
}

/// Core linear CORDIC: y/x для |y/x| ≤ 1.
fn linearVectoringCore(y_in: f32, x_in: f32) f32 {
    const x: f32 = x_in;
    var y: f32 = y_in;
    var z: f32 = 0;

    for (0..ITERS_F32) |i| {
        const sigma: f32 = if (y <= 0) 1.0 else -1.0;
        y += sigma * POW2_NEG[i] * x;
        z -= sigma * POW2_NEG[i];
    }

    return z;
}

// =============================================================================
// 5. HYPERBOLIC FUNCTIONS — sinh, cosh, atanh, exp, ln, sqrt
// =============================================================================
//
// Реализация через ряды Тейлора и итеративные методы.
// Это даёт hardware-independent результаты (как CORDIC),
// но с гарантированной сходимостью для всех входных диапазонов.
//
// Альтернатива hyperbolic CORDIC: ряды Тейлора сходятся быстрее
// и не требуют специальной обработки повторных итераций.
// =============================================================================

/// Результат: (cosh z, sinh z) одновременно.
pub const SinhCosh = struct {
    cosh: f32,
    sinh: f32,
};

/// cosh(x) и sinh(x) через ряды Тейлора.
///
/// cosh(x) = 1 + x²/2! + x⁴/4! + x⁶/6! + ...
/// sinh(x) = x + x³/3! + x⁵/5! + x⁷/7! + ...
///
/// Для больших |x|: используем тождество
///   cosh(x) = (e^x + e^-x)/2, sinh(x) = (e^x - e^-x)/2
/// и вычисляем e^x через нормализованную экспоненту.
pub fn hyperbolicRotation(z: f32) SinhCosh {
    // Для |z| ≤ 1: прямые ряды Тейлора (быстрая сходимость)
    if (@abs(z) <= 1.0) {
        return sinhCoshSeries(z);
    }

    // Для |z| > 1: используем double-angle формулы
    // sinh(2a) = 2·sinh(a)·cosh(a)
    // cosh(2a) = 2·cosh²(a) - 1
    var half = z * 0.5;
    var result = sinhCoshSeries(half);

    // Повторяем double-angle пока не достигнем нужной величины
    while (@abs(half) > 1.0) {
        half *= 0.5;
        result = sinhCoshSeries(half);
    }

    // Сколько раз мы делили на 2?
    var n_doublings: u32 = 0;
    var v = z;
    while (@abs(v) > 1.0) {
        v *= 0.5;
        n_doublings += 1;
    }
    result = sinhCoshSeries(v);

    // Восстанавливаем через double-angle
    for (0..n_doublings) |_| {
        const new_sinh = 2.0 * result.sinh * result.cosh;
        const new_cosh = 2.0 * result.cosh * result.cosh - 1.0;
        result = .{ .cosh = new_cosh, .sinh = new_sinh };
    }

    return result;
}

/// Ряды Тейлора для sinh/cosh — |z| ≤ 1.
fn sinhCoshSeries(z: f32) SinhCosh {
    const x2 = z * z;

    // cosh(x) = 1 + x²/2! + x⁴/4! + x⁶/6! + ...
    // Рекуррентное соотношение: term_{k+1} = term_k · x²/((2k+1)(2k+2))
    var cosh_val: f32 = 1.0;
    var cterm: f32 = 1.0;
    cterm *= x2 / 2.0;    cosh_val += cterm;   // x²/2!
    cterm *= x2 / 12.0;   cosh_val += cterm;   // x⁴/4!
    cterm *= x2 / 30.0;   cosh_val += cterm;   // x⁶/6!
    cterm *= x2 / 56.0;   cosh_val += cterm;   // x⁸/8!
    cterm *= x2 / 90.0;   cosh_val += cterm;   // x¹⁰/10!
    cterm *= x2 / 132.0;  cosh_val += cterm;   // x¹²/12!

    // sinh(x) = x + x³/3! + x⁵/5! + x⁷/7! + ...
    // Рекуррентное: term_{k+1} = term_k · x²/((2k+2)(2k+3))
    var sinh_val: f32 = z;
    var sterm: f32 = z;
    sterm *= x2 / 6.0;    sinh_val += sterm;   // x³/3!
    sterm *= x2 / 20.0;   sinh_val += sterm;   // x⁵/5!
    sterm *= x2 / 42.0;   sinh_val += sterm;   // x⁷/7!
    sterm *= x2 / 72.0;   sinh_val += sterm;   // x⁹/9!
    sterm *= x2 / 110.0;  sinh_val += sterm;   // x¹¹/11!
    sterm *= x2 / 156.0;  sinh_val += sterm;   // x¹³/13!

    return .{ .cosh = cosh_val, .sinh = sinh_val };
}

/// sinh(z) через ряды + double-angle.
pub fn sinh(z: f32) f32 {
    return hyperbolicRotation(z).sinh;
}

/// cosh(z) через ряды + double-angle.
pub fn cosh(z: f32) f32 {
    return hyperbolicRotation(z).cosh;
}

/// tanh(z) = sinh(z)/cosh(z).
pub fn tanh(z: f32) f32 {
    const result = hyperbolicRotation(z);
    if (@abs(result.cosh) < 1e-30) return if (z >= 0) 1.0 else -1.0;
    return result.sinh / result.cosh;
}

/// atanh(x) через ряд Тейлора: atanh(t) = t + t³/3 + t⁵/5 + ...
/// Для |t| < 1. Для |t| ≥ 1 — расходится (возвращаем ±∞).
pub fn atanh(x: f32) f32 {
    if (@abs(x) >= 1.0) return if (x > 0) math.floatMax(f32) else -math.floatMax(f32);
    if (@abs(x) < 1e-10) return x;

    // Для быстрой сходимости: atanh(t) = atanh(t_half) · 2 + ...
    // Или просто ряд для |t| < 0.5, иначе половинный аргумент
    if (@abs(x) < 0.5) {
        return atanhSeries(x);
    }

    // atanh(x) = atanh(x/(1+√(1-x²))) + atanh(√(1-x²)/(1+x/(1+√(1-x²))))
    // Проще: atanh(x) = 0.5·ln((1+x)/(1-x))
    const one_plus = 1.0 + x;
    const one_minus = 1.0 - x;
    if (@abs(one_minus) < 1e-30) return math.floatMax(f32);
    return 0.5 * ln(one_plus / one_minus);
}

/// Ряд Тейлора для atanh(t) — |t| < 0.5.
fn atanhSeries(t: f32) f32 {
    const t2 = t * t;
    var result: f32 = t;
    var term: f32 = t;
    // t + t³/3 + t⁵/5 + t⁷/7 + ...
    term *= t2; result += term / 3.0;
    term *= t2; result += term / 5.0;
    term *= t2; result += term / 7.0;
    term *= t2; result += term / 9.0;
    term *= t2; result += term / 11.0;
    term *= t2; result += term / 13.0;
    term *= t2; result += term / 15.0;
    term *= t2; result += term / 17.0;
    return result;
}

// =============================================================================
// 6. EXP, LN, SQRT
// =============================================================================

/// exp(x) через cosh + sinh: exp(x) = cosh(x) + sinh(x).
pub fn exp(x: f32) f32 {
    const result = hyperbolicRotation(x);
    return result.cosh + result.sinh;
}

/// ln(x) через atanh: ln(x) = 2·atanh((x-1)/(x+1)) для x > 0.
pub fn ln(x: f32) f32 {
    if (x <= 0) return -math.floatMax(f32);
    if (x == 1.0) return 0.0;

    // Нормализация: ln(x) = ln(x/2^k) + k·ln(2)
    const ln2: f32 = 0.6931471805599453;
    var k: i32 = 0;
    var v = x;
    while (v > 2.0) {
        v *= 0.5;
        k += 1;
    }
    while (v < 0.5) {
        v *= 2.0;
        k -= 1;
    }

    const t = (v - 1.0) / (v + 1.0);
    return 2.0 * atanhSeries(t) + @as(f32, @floatFromInt(k)) * ln2;
}

/// log2(x) через ln.
pub fn log2(x: f32) f32 {
    const ln2: f32 = 0.6931471805599453;
    return ln(x) / ln2;
}

/// log10(x) через ln.
pub fn log10(x: f32) f32 {
    const ln10: f32 = 2.302585092994046;
    return ln(x) / ln10;
}

/// √x через Newton-Raphson: x_{n+1} = (x_n + a/x_n) / 2.
pub fn sqrt(x: f32) f32 {
    if (x < 0) return std.math.nan(f32);
    if (x == 0) return 0;
    if (x == 1.0) return 1.0;

    // Нормализация: sqrt(x) = sqrt(x·4^k) / 2^k
    // Приводим v в диапазон [0.5, 2.0] для хорошего bit-hack приближения
    var v = x;
    var scale: f32 = 1.0;
    while (v < 0.5) {
        v *= 4.0;
        scale *= 0.5;
    }
    while (v > 2.0) {
        v *= 0.25;
        scale *= 2.0;
    }

    // Начальное приближение через bit hack: i = (i >> 1) + 0x1FC00000
    const half_bits: u32 = @as(u32, @bitCast(v)) >> 1;
    const sqrt_init: u32 = half_bits + @as(u32, 0x1FC00000);
    var y: f32 = @as(f32, @bitCast(sqrt_init));

    // Newton-Raphson: 4 итерации для f32 точности
    y = 0.5 * (y + v / y);
    y = 0.5 * (y + v / y);
    y = 0.5 * (y + v / y);
    y = 0.5 * (y + v / y);

    return y * scale;
}

/// 1/√x через Newton-Raphson.
pub fn invSqrt(x: f32) f32 {
    if (x <= 0) return 0;

    // Quake fast inverse sqrt
    var y: f32 = @as(f32, @bitCast(@as(u32, 0x5F3759DF) - (@as(u32, @bitCast(x)) >> 1)));

    // Newton-Raphson: 3 итерации
    const x2 = x * 0.5;
    y *= 1.5 - x2 * y * y;
    y *= 1.5 - x2 * y * y;
    y *= 1.5 - x2 * y * y;

    return y;
}

/// x^y через exp(y·ln(x)).
pub fn pow(x: f32, y: f32) f32 {
    if (x <= 0) return 0;
    return exp(y * ln(x));
}

// =============================================================================
// 7. P³ PROJECTIVE CORDIC — вращение в RP³
// =============================================================================

/// Однородная четвёрка [x₀:x₁:x₂:x₃].
pub const HomVec4 = [4]f32;

/// Вращение в RP³ вокруг оси на угол θ (через CORDIC).
///
/// Rodrigues' formula: v_rot = v·cos θ + (k × v)·sin θ + k·(k·v)·(1 - cos θ)
/// Однородная координата w не меняется при вращении.
pub fn projectiveRotate(point: HomVec4, axis_x: f32, axis_y: f32, axis_z: f32, theta: f32) HomVec4 {
    const sc = circularRotation(theta);

    const axis_len = @sqrt(axis_x * axis_x + axis_y * axis_y + axis_z * axis_z);
    if (axis_len < 1e-10) return point;
    const ax = axis_x / axis_len;
    const ay = axis_y / axis_len;
    const az = axis_z / axis_len;

    const p0 = point[0];
    const p1 = point[1];
    const p2 = point[2];
    const p3 = point[3];

    const cross_x = ay * p2 - az * p1;
    const cross_y = az * p0 - ax * p2;
    const cross_z = ax * p1 - ay * p0;
    const dot = ax * p0 + ay * p1 + az * p2;
    const one_minus_cos = 1.0 - sc.cos;

    return .{
        p0 * sc.cos + cross_x * sc.sin + ax * dot * one_minus_cos,
        p1 * sc.cos + cross_y * sc.sin + ay * dot * one_minus_cos,
        p2 * sc.cos + cross_z * sc.sin + az * dot * one_minus_cos,
        p3,
    };
}

/// Cross-ratio через CORDIC division.
/// CR(A,B;C,D) = (AC·BD) / (AD·BC) — инвариант PGL(4).
pub fn crossRatio(a: f32, b: f32, c: f32, d: f32) f32 {
    const ac = a - c;
    const ad = a - d;
    const bc = b - c;
    const bd = b - d;
    if (@abs(ad) < 1e-30 or @abs(bc) < 1e-30) return math.floatMax(f32);
    return (ac * bd) / (ad * bc);
}

// =============================================================================
// 8. DUAL-NUMBER CORDIC — автоматическое дифференцирование
// =============================================================================

/// Dual number: a + ε·b, ε² = 0. Вычисляет f(x) и f'(x) одновременно.
pub const DualF32 = struct {
    val: f32,
    der: f32,

    pub fn init(val: f32, der: f32) DualF32 {
        return .{ .val = val, .der = der };
    }

    pub fn constant(val: f32) DualF32 {
        return .{ .val = val, .der = 0 };
    }

    pub fn variable(val: f32) DualF32 {
        return .{ .val = val, .der = 1 };
    }

    pub fn add(self: DualF32, other: DualF32) DualF32 {
        return .{ .val = self.val + other.val, .der = self.der + other.der };
    }

    pub fn mul(self: DualF32, other: DualF32) DualF32 {
        return .{
            .val = self.val * other.val,
            .der = self.val * other.der + self.der * other.val,
        };
    }

    pub fn div(self: DualF32, other: DualF32) DualF32 {
        const c2 = other.val * other.val;
        if (@abs(c2) < 1e-30) return .{ .val = math.floatMax(f32), .der = 0 };
        return .{
            .val = self.val / other.val,
            .der = (self.der * other.val - self.val * other.der) / c2,
        };
    }
};

/// Dual sin: sin(x) + ε·cos(x)·x'.
pub fn dualSin(x: DualF32) DualF32 {
    const s = sin(x.val);
    const c = cos(x.val);
    return .{ .val = s, .der = c * x.der };
}

/// Dual cos: cos(x) - ε·sin(x)·x'.
pub fn dualCos(x: DualF32) DualF32 {
    const s = sin(x.val);
    const c = cos(x.val);
    return .{ .val = c, .der = -s * x.der };
}

/// Dual exp: exp(x) + ε·exp(x)·x'.
pub fn dualExp(x: DualF32) DualF32 {
    const e = exp(x.val);
    return .{ .val = e, .der = e * x.der };
}

/// Dual ln: ln(x) + ε·x'/x.
pub fn dualLn(x: DualF32) DualF32 {
    if (@abs(x.val) < 1e-30) return .{ .val = -math.floatMax(f32), .der = 0 };
    return .{ .val = ln(x.val), .der = x.der / x.val };
}

/// Dual sqrt: √x + ε·x'/(2√x).
pub fn dualSqrt(x: DualF32) DualF32 {
    if (x.val < 0) return .{ .val = std.math.nan(f32), .der = 0 };
    const s = sqrt(x.val);
    if (@abs(s) < 1e-30) return .{ .val = 0, .der = math.floatMax(f32) };
    return .{ .val = s, .der = x.der / (2.0 * s) };
}

// =============================================================================
// 9. ТЕСТЫ
// =============================================================================

test "CORDIC: sin accuracy vs std" {
    const angles = [_]f32{ 0, 0.1, 0.5, 1.0, 1.5, math.pi / 4.0, math.pi / 3.0, math.pi / 2.0 };
    for (angles) |a| {
        const cval = sin(a);
        const sval = @sin(a);
        try std.testing.expectApproxEqAbs(cval, sval, 1e-4);
    }
}

test "CORDIC: cos accuracy vs std" {
    const angles = [_]f32{ 0, 0.1, 0.5, 1.0, 1.5, math.pi / 4.0, math.pi / 3.0, math.pi / 2.0 };
    for (angles) |a| {
        const cval = cos(a);
        const sval = @cos(a);
        try std.testing.expectApproxEqAbs(cval, sval, 1e-4);
    }
}

test "CORDIC: sin²+cos² = 1" {
    const angles = [_]f32{ 0, 0.3, 0.7, 1.0, 1.57, 3.14 };
    for (angles) |a| {
        const sc = circularRotation(a);
        try std.testing.expectApproxEqAbs(sc.sin * sc.sin + sc.cos * sc.cos, 1.0, 1e-4);
    }
}

test "CORDIC: atan2 accuracy" {
    try std.testing.expectApproxEqAbs(atan2(1.0, 1.0), math.pi / 4.0, 1e-4);
    try std.testing.expectApproxEqAbs(atan2(1.0, 0.0), math.pi / 2.0, 1e-4);
    try std.testing.expectApproxEqAbs(atan2(0.0, 1.0), 0.0, 1e-4);
    try std.testing.expectApproxEqAbs(atan2(-1.0, 1.0), -math.pi / 4.0, 1e-4);
}

test "CORDIC: atan accuracy" {
    try std.testing.expectApproxEqAbs(atan(1.0), math.pi / 4.0, 1e-4);
    try std.testing.expectApproxEqAbs(atan(0.0), 0.0, 1e-4);
}

test "CORDIC: exp accuracy" {
    try std.testing.expectApproxEqAbs(exp(0.0), 1.0, 1e-4);
    try std.testing.expectApproxEqAbs(exp(1.0), 2.718281828, 1e-4);
    try std.testing.expectApproxEqAbs(exp(-1.0), 0.367879441, 1e-4);
    try std.testing.expectApproxEqAbs(exp(0.5), 1.648721271, 1e-4);
}

test "CORDIC: ln accuracy" {
    try std.testing.expectApproxEqAbs(ln(1.0), 0.0, 1e-4);
    try std.testing.expectApproxEqAbs(ln(2.718281828), 1.0, 1e-3);
    try std.testing.expectApproxEqAbs(ln(0.5), -0.693147, 1e-4);
}

test "CORDIC: sqrt accuracy" {
    try std.testing.expectApproxEqAbs(sqrt(1.0), @as(f32, 1.0), @as(f32, 1e-4));
    try std.testing.expectApproxEqAbs(sqrt(4.0), @as(f32, 2.0), @as(f32, 1e-4));
    try std.testing.expectApproxEqAbs(sqrt(2.0), @as(f32, 1.41421356), @as(f32, 1e-4));
    try std.testing.expectApproxEqAbs(sqrt(0.25), @as(f32, 0.5), @as(f32, 1e-4));
}

test "CORDIC: pow accuracy" {
    try std.testing.expectApproxEqAbs(pow(2.0, 3.0), 8.0, 1e-2);
    try std.testing.expectApproxEqAbs(pow(2.0, 0.5), 1.41421356, 1e-3);
    try std.testing.expectApproxEqAbs(pow(math.e, 1.0), 2.718281828, 1e-2);
}

test "CORDIC: cross-ratio invariance" {
    const cr1 = crossRatio(1.0, 2.0, 3.0, 4.0);
    const cr2 = crossRatio(2.0, 4.0, 6.0, 8.0);
    try std.testing.expectApproxEqAbs(cr1, cr2, 1e-4);
}

test "CORDIC: linear mul" {
    try std.testing.expectApproxEqAbs(linearRotation(3.0, 4.0), 12.0, 0.01);
    try std.testing.expectApproxEqAbs(linearRotation(2.5, 2.0), 5.0, 0.01);
    try std.testing.expectApproxEqAbs(linearRotation(3.0, 0.5), 1.5, 0.01);
}

test "CORDIC: linear div" {
    try std.testing.expectApproxEqAbs(linearVectoring(10.0, 2.0), 5.0, 0.01);
    try std.testing.expectApproxEqAbs(linearVectoring(7.0, 2.0), 3.5, 0.01);
    try std.testing.expectApproxEqAbs(linearVectoring(1.0, 4.0), 0.25, 0.01);
}

test "CORDIC: dual sin/cos derivatives" {
    const x = DualF32.variable(0.5);
    const ds = dualSin(x);
    try std.testing.expectApproxEqAbs(ds.val, @sin(0.5), 1e-4);
    try std.testing.expectApproxEqAbs(ds.der, @cos(0.5), 1e-4);

    const dc = dualCos(x);
    try std.testing.expectApproxEqAbs(dc.val, @cos(0.5), 1e-4);
    try std.testing.expectApproxEqAbs(dc.der, -@sin(0.5), 1e-4);
}

test "CORDIC: dual exp derivative" {
    const x = DualF32.variable(1.0);
    const de = dualExp(x);
    try std.testing.expectApproxEqAbs(de.val, 2.718281828, 1e-3);
    try std.testing.expectApproxEqAbs(de.der, 2.718281828, 1e-3);
}

test "CORDIC: projective rotation preserves homogeneous" {
    const point: HomVec4 = .{ 1.0, 0.0, 0.0, 1.0 };
    const rotated = projectiveRotate(point, 0.0, 1.0, 0.0, math.pi / 2.0);
    try std.testing.expectApproxEqAbs(rotated[0], 0.0, 1e-3);
    try std.testing.expectApproxEqAbs(rotated[2], -1.0, 1e-3);
    try std.testing.expectApproxEqAbs(rotated[3], 1.0, 1e-3);
}

test "CORDIC: sinh/cosh identity" {
    const z: f32 = 0.5;
    const result = hyperbolicRotation(z);
    try std.testing.expectApproxEqAbs(result.cosh * result.cosh - result.sinh * result.sinh, 1.0, 1e-4);
}

test "CORDIC: invSqrt accuracy" {
    try std.testing.expectApproxEqAbs(invSqrt(4.0), 0.5, 1e-3);
    try std.testing.expectApproxEqAbs(invSqrt(2.0) * 2.0, 1.41421356, 1e-3);
}
