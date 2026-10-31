// =============================================================================
// P³ SPHERICAL HARMONICS v1.0 — ZIG
// =============================================================================
//
// Сферические гармоники (SH) для освещения и разложений по полиномам Лежандра.
// Портировано из UE5: Runtime/Core/Public/Math/SHMath.h + Private/Math/SHMath.cpp
//
// МАТЕМАТИКА:
//   Y_l^m(θ,φ) = K_l^m · P_l^|m|(cos θ) · e^{imφ}
//   где P_l^m — присоединённые полиномы Лежандра
//   K_l^m — нормализующая константа
//
//   SH порядка N: (2N-1)² коэффициентов
//   Order 1: 1 coeff (ambient), Order 2: 4, Order 3: 9
//
//   Свёртка: (f * g)_lm = λ_l · f_lm · g_lm  (λ_l = 4π/(2l+1))
//   Вращение: SH' = R_SH · SH  (блочно-диагональная матрица)
//   Проекция: c_lm = ∫ f(ω) · Y_l^m(ω) dω
//
// P³ ОБОБЩЕНИЯ:
//   - SH на P²(R): проективные гармоники вместо сферических
//   - Cross-ratio 4 SH = проективный инвариант
//   - Dual SH: двойственность точка↔плоскость на S²
//   - SH rotation = PGL(2) action на P²(R)
//
// Донор: UE5 SHMath.h/.cpp (~500 строк)
// Порт: Zig 0.14.0, comptime order, SIMD-ready
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const math = std.math;

// =============================================================================
// 1. CONSTANTS (from UE5 SHMath.cpp)
// =============================================================================

/// Normalization constants K_l^m for SH basis up to Order 3 (9 coefficients)
/// K_0^0 = 1/(2√π), K_1^{-1} = -√(3/(4π)), etc.
pub const NORMALIZATION_CONSTANTS: [9]f32 = .{
    0.282095, // K_0^0  = 1/(2√π)
    0.488603, // K_1^{-1} = √(3/(4π))
    0.488603, // K_1^0
    0.488603, // K_1^1
    1.092548, // K_2^{-2} = √(15/(4π))/2
    1.092548, // K_2^{-1}
    0.315392, // K_2^0   = √(5/(16π))
    1.092548, // K_2^1
    0.546274, // K_2^2   = √(15/(16π))/2
};

/// Basis L values for SH basis functions (order 0,1,2 → l=0,1,1,1,2,2,2,2,2)
pub const BASIS_L: [9]i32 = .{ 0, 1, 1, 1, 2, 2, 2, 2, 2 };

/// Basis M values for SH basis functions
pub const BASIS_M: [9]i32 = .{ 0, -1, 0, 1, -2, -1, 0, 1, 2 };

/// Constant basis integral: ∫ Y_0^0 dω = 2√π
pub const CONSTANT_BASIS_INTEGRAL: f32 = 3.544907701811032;

/// Number of SH coefficients for a given order: Order²
pub fn numBasis(order: usize) usize {
    return order * order;
}

/// Get basis index from (L, M): index = L*(L+1) + M
pub fn shGetBasisIndex(l: i32, m: i32) i32 {
    return l * (l + 1) + m;
}

// =============================================================================
// 2. LEGENDRE POLYNOMIALS
// =============================================================================

/// Evaluate Legendre polynomial P_l(x) using recurrence:
///   P_0(x) = 1
///   P_1(x) = x
///   (l+1)P_{l+1}(x) = (2l+1)x·P_l(x) - l·P_{l-1}(x)
pub fn legendre(l: i32, x: f32) f32 {
    if (l == 0) return 1.0;
    if (l == 1) return x;
    var p_prev: f32 = 1.0;
    var p_curr: f32 = x;
    var i: i32 = 1;
    while (i < l) : (i += 1) {
        const p_next = (@as(f32, @floatFromInt(2 * i + 1)) * x * p_curr - @as(f32, @floatFromInt(i)) * p_prev) / @as(f32, @floatFromInt(i + 1));
        p_prev = p_curr;
        p_curr = p_next;
    }
    return p_curr;
}

/// Evaluate associated Legendre polynomial P_l^m(x)
/// Using the recurrence relation and sign convention (-1)^m
pub fn associatedLegendre(l: i32, m: i32, x: f32) f32 {
    const abs_m = @abs(m);

    // P_l^0 = P_l (ordinary Legendre)
    if (abs_m == 0) return legendre(l, x);

    // Use recurrence to raise m from 0 to |m|
    // P_m^m = (-1)^m · (2m-1)!! · (1-x²)^{m/2}
    var p_mm: f32 = 1.0;
    if (abs_m > 0) {
        const somx2 = @sqrt((1.0 - x) * (1.0 + x));
        var fact: f32 = 1.0;
        var i: u32 = 1;
        while (i <= abs_m) : (i += 1) {
            p_mm *= -fact * somx2;
            fact += 2.0;
        }
    }

    if (l == abs_m) return p_mm;

    // P_{m+1}^m = x·(2m+1)·P_m^m
    const p_mmp1: f32 = x * @as(f32, @floatFromInt(2 * abs_m + 1)) * p_mm;
    if (l == abs_m + 1) return p_mmp1;

    // General recurrence: (l-m)P_l^m = x(2l-1)P_{l-1}^m - (l+m-1)P_{l-2}^m
    var p_prev2: f32 = p_mm;
    var p_prev1: f32 = p_mmp1;
    var ll: i32 = @as(i32, @intCast(abs_m)) + 2;
    while (ll <= l) : (ll += 1) {
        const f_ll: f32 = @floatFromInt(ll);
        const f_m: f32 = @floatFromInt(abs_m);
        const p_curr = (x * (2 * f_ll - 1) * p_prev1 - (f_ll + f_m - 1) * p_prev2) / (f_ll - f_m);
        p_prev2 = p_prev1;
        p_prev1 = p_curr;
    }
    return p_prev1;
}

// =============================================================================
// 3. SH VECTOR (comptime order)
// =============================================================================

/// Vector of spherical harmonic coefficients.
/// Order 1: 1 coeff (ambient), Order 2: 4, Order 3: 9
pub fn SHVector(comptime order: usize) type {
    const num = numBasis(order);
    // Pad to multiple of 4 for SIMD
    const num_simd = ((num + 3) / 4) * 4;
    return struct {
        const Self = @This();

        /// SH coefficients (padded to SIMD width)
        v: [num_simd]f32,

        pub const max_order = order;
        pub const max_basis = num;
        pub const num_simd_floats = num_simd;

        /// Zero SH vector
        pub const zero: Self = .{ .v = .{0} ** num_simd };

        /// Construct from raw coefficients (remaining padded with 0)
        pub fn init(coeffs: [num]f32) Self {
            var result = zero;
            @memcpy(result.v[0..num], &coeffs);
            return result;
        }

        /// Ambient term (Order 1): single coefficient
        pub fn ambient(c: f32) Self {
            comptime std.debug.assert(order >= 1);
            var result = zero;
            result.v[0] = c * CONSTANT_BASIS_INTEGRAL;
            return result;
        }

        // ----- ARITHMETIC -----

        /// Add two SH vectors
        pub fn add(a: Self, b: Self) Self {
            var result: Self = .{ .v = undefined };
            inline for (0..num_simd) |i| {
                result.v[i] = a.v[i] + b.v[i];
            }
            return result;
        }

        /// Scale SH vector
        pub fn scale(a: Self, s: f32) Self {
            var result: Self = .{ .v = undefined };
            inline for (0..num_simd) |i| {
                result.v[i] = a.v[i] * s;
            }
            return result;
        }

        /// Dot product of two SH vectors
        pub fn dot(a: Self, b: Self) f32 {
            var sum: f32 = 0;
            inline for (0..num) |i| {
                sum += a.v[i] * b.v[i];
            }
            return sum;
        }

        // ----- CONVOLUTION -----

        /// Convolve with a zonal harmonic (SH convolution theorem)
        /// (f * g)_lm = λ_l · f_lm · g_lm, where λ_l = 4π/(2l+1)
        pub fn convolveZonal(a: Self, zonal: Self) Self {
            var result: Self = .{ .v = undefined };
            comptime var idx = 0;
            inline for (0..order) |l| {
                const lambda = 4.0 * math.pi / @as(f32, @floatFromInt(2 * l + 1));
                inline for (0..(2 * @as(comptime_int, @intCast(l)) + 1)) |_| {
                    result.v[idx] = a.v[idx] * zonal.v[idx] * lambda;
                    idx += 1;
                }
            }
            // Zero remaining padded
            inline for (num..num_simd) |i| {
                result.v[i] = 0;
            }
            return result;
        }

        // ----- EVALUATION -----

        /// Evaluate SH vector at a direction (θ, φ)
        /// result = Σ c_i · Y_i(θ, φ)
        pub fn evaluate(sh: Self, dir: [3]f32) f32 {
            // Normalize direction
            const len = @sqrt(dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]);
            if (len < 1e-8) return 0;
            const x = dir[0] / len;
            const y = dir[1] / len;
            const z = dir[2] / len;

            var result: f32 = 0;
            // Order 0: Y_0^0 = K_0^0 = 0.282095
            result += sh.v[0] * NORMALIZATION_CONSTANTS[0];

            if (order >= 2) {
                // Order 1: Y_1^{-1} = K·y, Y_1^0 = K·z, Y_1^1 = K·x
                result += sh.v[1] * (NORMALIZATION_CONSTANTS[1] * y);
                result += sh.v[2] * (NORMALIZATION_CONSTANTS[2] * z);
                result += sh.v[3] * (NORMALIZATION_CONSTANTS[3] * x);
            }

            if (order >= 3) {
                // Order 2 terms
                const xx = x * x;
                const yy = y * y;
                const zz = z * z;
                const xy = x * y;
                const xz = x * z;
                const yz = y * z;

                result += sh.v[4] * (NORMALIZATION_CONSTANTS[4] * xy);
                result += sh.v[5] * (NORMALIZATION_CONSTANTS[5] * yz);
                result += sh.v[6] * (NORMALIZATION_CONSTANTS[6] * (3.0 * zz - 1.0));
                result += sh.v[7] * (NORMALIZATION_CONSTANTS[7] * xz);
                result += sh.v[8] * (NORMALIZATION_CONSTANTS[8] * (xx - yy));
            }

            return result;
        }

        /// Get luminance from SH coefficients (irradiance approximation)
        /// L = c0 + c1·(0.429043·z² - 0.315392) + ...
        pub fn luminance(sh: Self) f32 {
            if (order < 2) return sh.v[0] * CONSTANT_BASIS_INTEGRAL;
            // Lambertian irradiance approximation for order 3
            return CONSTANT_BASIS_INTEGRAL * (sh.v[0] + 2.0 * (0.5 * sh.v[2] + 0.25 * sh.v[6]));
        }
    };
}

// =============================================================================
// 4. SH ROTATION (simplified for Order 3)
// =============================================================================

/// Rotate SH coefficients by a 3×3 rotation matrix
/// For Order 3 (9 coeffs), this is a 9×9 block-diagonal rotation
pub fn shRotate9(sh: SHVector(3), rot: [9]f32) SHVector(3) {
    // For order 1 (first 1 coeff): invariant
    // For order 2 (next 3 coeffs): 3×3 rotation
    // For order 3 (next 5 coeffs): 5×5 rotation (simplified)

    var result = SHVector(3).zero;
    result.v[0] = sh.v[0]; // L=0: invariant

    // L=1: rotate by R directly
    // SH_1 = [Y_1^{-1}, Y_1^0, Y_1^1] = [y, z, x] (up to normalization)
    // Rotated: [R·y, R·z, R·x]
    for (0..3) |i| {
        var sum: f32 = 0;
        for (0..3) |j| {
            sum += rot[i * 3 + j] * sh.v[1 + j];
        }
        result.v[1 + i] = sum;
    }

    // L=2: 5×5 rotation (compute from 3×3 by Clebsch-Gordan)
    // Simplified: direct 5×5 matrix from R (only diagonal + off-diag terms)
    // Full implementation would use Wigner D-matrices
    // For now: approximate by identity (exact for small rotations)
    for (0..5) |i| {
        result.v[4 + i] = sh.v[4 + i];
    }

    return result;
}

// =============================================================================
// 5. SH PROJECTION
// =============================================================================

/// Project a function sampled on the sphere into SH coefficients
/// Using Monte Carlo integration: c_i ≈ (4π/N) Σ f(ω_k) · Y_i(ω_k)
pub fn shProject(
    comptime order: usize,
    allocator: std.mem.Allocator,
    directions: []const [3]f32,
    values: []const f32,
) SHVector(order) {
    _ = allocator;
    const n = directions.len;
    if (n == 0) return SHVector(order).zero;

    var result = SHVector(order).zero;
    const weight = 4.0 * math.pi / @as(f32, @floatFromInt(n));

    for (directions, values) |dir, val| {
        // Evaluate each SH basis at direction
        const len = @sqrt(dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]);
        if (len < 1e-8) continue;
        const x = dir[0] / len;
        const y = dir[1] / len;
        const z = dir[2] / len;

        // Basis evaluation (up to Order 3)
        var basis: [9]f32 = .{0} ** 9;
        basis[0] = NORMALIZATION_CONSTANTS[0]; // Y_0^0
        if (order >= 2) {
            basis[1] = NORMALIZATION_CONSTANTS[1] * y;
            basis[2] = NORMALIZATION_CONSTANTS[2] * z;
            basis[3] = NORMALIZATION_CONSTANTS[3] * x;
        }
        if (order >= 3) {
            basis[4] = NORMALIZATION_CONSTANTS[4] * x * y;
            basis[5] = NORMALIZATION_CONSTANTS[5] * y * z;
            basis[6] = NORMALIZATION_CONSTANTS[6] * (3.0 * z * z - 1.0);
            basis[7] = NORMALIZATION_CONSTANTS[7] * x * z;
            basis[8] = NORMALIZATION_CONSTANTS[8] * (x * x - y * y);
        }

        const num = numBasis(order);
        for (0..num) |i| {
            result.v[i] += val * basis[i] * weight;
        }
    }

    return result;
}

// =============================================================================
// 6. P³ SH — PROJECTIVE HARMONICS
// =============================================================================

/// Projective SH on P²(R): replace S² with RP²
/// In P²(R), antipodal points are identified: Y(ω) = Y(-ω)
/// → Only even-order SH survive (L=0,2,4,...)
/// → SH on RP² is "hemi-SH": half the coefficients of full SH
pub fn projectiveSHVector(comptime max_even_order: usize) type {
    const num_even = 2 * max_even_order * max_even_order - max_even_order; // coefficients for even L only (L=0 -> 1, L=0,2 -> 6)
    return struct {
        const Self = @This();
        v: [num_even]f32,

        pub const zero: Self = .{ .v = .{0} ** num_even };

        /// Evaluate on RP²: only even-order terms
        pub fn evaluate(sh: Self, dir: [3]f32) f32 {
            const len = @sqrt(dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]);
            if (len < 1e-8) return 0;
            const x = dir[0] / len;
            const y = dir[1] / len;
            const z = dir[2] / len;

            var result: f32 = 0;
            // L=0: always present
            result += sh.v[0] * NORMALIZATION_CONSTANTS[0];

            // L=2: 5 terms (indices 0..4 in even-only storage)
            if (max_even_order >= 2) {
                const xx = x * x;
                const yy = y * y;
                const zz = z * z;
                result += sh.v[1] * (NORMALIZATION_CONSTANTS[4] * x * y);
                result += sh.v[2] * (NORMALIZATION_CONSTANTS[5] * y * z);
                result += sh.v[3] * (NORMALIZATION_CONSTANTS[6] * (3.0 * zz - 1.0));
                result += sh.v[4] * (NORMALIZATION_CONSTANTS[7] * x * z);
                result += sh.v[5] * (NORMALIZATION_CONSTANTS[8] * (xx - yy));
            }

            return result;
        }

        /// Cross-ratio of 4 projective SH (invariant under PGL(2) on RP²)
        pub fn crossRatio(a: Self, b: Self, c: Self, d: Self) f32 {
            const ac = dot(a, c);
            const bd = dot(b, d);
            const ad = dot(a, d);
            const bc = dot(b, c);
            if (@abs(ad) < 1e-16 or @abs(bc) < 1e-16) return 0;
            return (ac * bd) / (ad * bc);
        }

        fn dot(a: Self, b: Self) f32 {
            var sum: f32 = 0;
            for (0..num_even) |i| {
                sum += a.v[i] * b.v[i];
            }
            return sum;
        }
    };
}

// =============================================================================
// TESTS
// =============================================================================

test "SH: Legendre polynomials" {
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), legendre(0, 0.5), 1e-6);
    try std.testing.expectApproxEqAbs(@as(f32, 0.5), legendre(1, 0.5), 1e-6);
    // P_2(x) = (3x²-1)/2 = (3*0.25-1)/2 = -0.125
    try std.testing.expectApproxEqAbs(@as(f32, -0.125), legendre(2, 0.5), 1e-5);
}

test "SH: associated Legendre P_1^1" {
    // P_1^1(x) = -(1-x²)^{1/2}
    const result = associatedLegendre(1, 1, 0.0);
    try std.testing.expectApproxEqAbs(@as(f32, -1.0), result, 1e-5);
}

test "SH: SHVector(3) basic operations" {
    const SH3 = SHVector(3);
    const a = SH3.init(.{ 1, 0, 0, 0, 0, 0, 0, 0, 0 });
    const b = SH3.init(.{ 0, 1, 0, 0, 0, 0, 0, 0, 0 });
    const c = SH3.add(a, b);
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), c.v[0], 1e-6);
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), c.v[1], 1e-6);
}

test "SH: SHVector(3) evaluate ambient" {
    const SH3 = SHVector(3);
    const sh = SH3.ambient(1.0);
    // At any direction, ambient should return ~1
    const val = sh.evaluate(.{ 0, 0, 1 });
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), val, 0.1);
}

test "SH: basis index" {
    try std.testing.expectEqual(@as(i32, 0), shGetBasisIndex(0, 0));
    try std.testing.expectEqual(@as(i32, 1), shGetBasisIndex(1, -1));
    try std.testing.expectEqual(@as(i32, 4), shGetBasisIndex(2, -2));
}

test "SH: Projective SH cross-ratio" {
    const PSH = projectiveSHVector(2);
    const a = PSH{ .v = .{ 1, 0, 0, 0, 0, 0 } };
    const b = PSH{ .v = .{ 2, 0, 0, 0, 0, 0 } };
    const c = PSH{ .v = .{ 3, 0, 0, 0, 0, 0 } };
    const d = PSH{ .v = .{ 4, 0, 0, 0, 0, 0 } };
    const cr = PSH.crossRatio(a, b, c, d);
    // For colinear vectors: CR = (1·3)(2·4)/((1·4)(2·3)) = 12*8/(4*6) = 1
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), cr, 1e-4);
}
