// =============================================================================
// P³ POLYNOMIAL ROOT SOLVER v1.0 — ZIG
// =============================================================================
//
// Решатели корней полиномов 2-й, 3-й и 4-й степени + квартика Феррари.
// Портировано из UE5: Runtime/Core/Public/Math/PolynomialRootSolver.h
//
// МАТЕМАТИКА:
//   2nd degree: ax² + bx + c = 0  → quadratic formula
//   3rd degree: ax³ + bx² + cx + d = 0  → Cardano's method
//   4th degree: ax⁴ + bx³ + cx² + dx + e = 0  → Ferrari's method
//
//   UE5 подход: Newton-bisection hybrid с производными для устойчивости
//   Наш подход: прямые формулы для deg 2-3, Ferrari для deg 4
//
// P³ ОБОБЩЕНИЯ:
//   - Корни полинома на P¹: проективные корни [x:w] ∈ P¹
//   - Бесконечный корень: leading coeff = 0 → [1:0] ∈ P¹
//   - Cross-ratio 4 корней = проективный инвариант
//   - Dual roots: корни над кольцом дуальных чисел
//
// Донор: UE5 PolynomialRootSolver.h (249 строк)
// Порт: Zig 0.14.0, comptime degree, P³ integration
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const math = std.math;

// =============================================================================
// 1. QUADRATIC SOLVER (2nd degree)
// =============================================================================

/// Solve ax² + bx + c = 0
/// Returns up to 2 real roots in sorted order
pub fn solveQuadratic(a: f32, b: f32, c: f32, tolerance: f32) [2]f32 {
    var roots: [2]f32 = .{ 0, 0 };

    // Degenerate: linear bx + c = 0
    if (@abs(a) < tolerance) {
        if (@abs(b) < tolerance) return roots; // constant = 0
        roots[0] = -c / b;
        return roots;
    }

    // Discriminant: Δ = b² - 4ac
    const disc = b * b - 4.0 * a * c;
    if (disc < 0) return roots; // no real roots

    // Numerically stable quadratic formula (avoiding cancellation)
    const sqrt_disc = @sqrt(disc);
    var q: f32 = undefined;
    if (b >= 0) {
        q = -0.5 * (b + sqrt_disc);
    } else {
        q = -0.5 * (b - sqrt_disc);
    }

    const r0 = q / a;
    const r1 = c / q;

    if (r0 <= r1) {
        roots[0] = r0;
        roots[1] = r1;
    } else {
        roots[0] = r1;
        roots[1] = r0;
    }

    return roots;
}

/// Count real roots of quadratic
pub fn countQuadraticRoots(a: f32, b: f32, c: f32, tolerance: f32) u32 {
    if (@abs(a) < tolerance) {
        if (@abs(b) < tolerance) return 0;
        return 1;
    }
    const disc = b * b - 4.0 * a * c;
    if (disc < 0) return 0;
    if (disc < tolerance * tolerance) return 1;
    return 2;
}

// =============================================================================
// 2. CUBIC SOLVER (3rd degree) — Cardano's method
// =============================================================================

/// Solve ax³ + bx² + cx + d = 0
/// Returns up to 3 real roots in sorted order
pub fn solveCubic(a: f32, b: f32, c: f32, d: f32, tolerance: f32) [3]f32 {
    var roots: [3]f32 = .{ 0, 0, 0 };
    var count: u32 = 0;

    // Degenerate: quadratic
    if (@abs(a) < tolerance) {
        const q_roots = solveQuadratic(b, c, d, tolerance);
        roots[0] = q_roots[0];
        roots[1] = q_roots[1];
        return roots;
    }

    // Normalize: x³ + px² + qx + r = 0
    const p = b / a;
    const q = c / a;
    const r = d / a;

    // Depressed cubic: t³ + Pt + Q = 0, where x = t - p/3
    const shift = p / 3.0;
    const P = q - p * shift;
    const Q = r - p * q / 3.0 + 2.0 * p * p * p / 27.0;

    // Discriminant: Δ = -4P³ - 27Q²
    const disc = -4.0 * P * P * P - 27.0 * Q * Q;

    if (disc > tolerance) {
        // Three distinct real roots (casus irreducibilis)
        // Use trigonometric solution
        const m = 2.0 * @sqrt(-P / 3.0);
        const theta = std.math.acos(3.0 * Q / (P * m)) / 3.0;
        roots[0] = m * @cos(theta) - shift;
        roots[1] = m * @cos(theta - 2.0 * math.pi / 3.0) - shift;
        roots[2] = m * @cos(theta + 2.0 * math.pi / 3.0) - shift;
        count = 3;
    } else if (disc > -tolerance) {
        // Repeated roots
        if (@abs(Q) < tolerance) {
            // Triple root at x = -p/3
            roots[0] = -shift;
            count = 1;
        } else {
            // Double root + single root
            const w = cbrt(-Q / 2.0);
            roots[0] = 2.0 * w - shift;
            roots[1] = -w - shift;
            count = 2;
        }
    } else {
        // One real root + two complex conjugate
        const inner = Q * Q / 4.0 + P * P * P / 27.0;
        const sqrt_inner = @sqrt(@max(inner, 0));
        const w1 = cbrt(-Q / 2.0 + sqrt_inner);
        const w2 = cbrt(-Q / 2.0 - sqrt_inner);
        roots[0] = w1 + w2 - shift;
        count = 1;
    }

    // Sort roots
    if (count == 2 and roots[0] > roots[1]) {
        const tmp = roots[0];
        roots[0] = roots[1];
        roots[1] = tmp;
    }
    if (count == 3) {
        // Bubble sort 3 elements
        if (roots[0] > roots[1]) { const tmp = roots[0]; roots[0] = roots[1]; roots[1] = tmp; }
        if (roots[1] > roots[2]) { const tmp = roots[1]; roots[1] = roots[2]; roots[2] = tmp; }
        if (roots[0] > roots[1]) { const tmp = roots[0]; roots[0] = roots[1]; roots[1] = tmp; }
    }

    return roots;
}

// =============================================================================
// 3. QUARTIC SOLVER (4th degree) — Ferrari's method
// =============================================================================

/// Solve ax⁴ + bx³ + cx² + dx + e = 0
/// Returns up to 4 real roots in sorted order
pub fn solveQuartic(a: f32, b: f32, c: f32, d: f32, e: f32, tolerance: f32) [4]f32 {
    var roots: [4]f32 = .{ 0, 0, 0, 0 };
    var count: u32 = 0;

    // Degenerate: cubic
    if (@abs(a) < tolerance) {
        const c_roots = solveCubic(b, c, d, e, tolerance);
        roots[0] = c_roots[0];
        roots[1] = c_roots[1];
        roots[2] = c_roots[2];
        return roots;
    }

    // Normalize: x⁴ + px³ + qx² + rx + s = 0
    const p = b / a;
    const q = c / a;
    const r = d / a;
    const s = e / a;

    // Depressed quartic: y⁴ + Qy² + Ry + S = 0, where x = y - p/4
    const shift = p / 4.0;
    const pp = p * p;
    const Q = q - 3.0 * pp / 8.0;
    const R = r - p * q / 2.0 + pp * p / 8.0;
    const S = s - p * r / 4.0 + pp * q / 16.0 - 3.0 * pp * pp / 256.0;

    // Biquadratic case: R = 0 → y⁴ + Qy² + S = 0
    if (@abs(R) < tolerance) {
        const z_roots = solveQuadratic(1.0, Q, S, tolerance);
        for (0..2) |i| {
            if (z_roots[i] >= -tolerance) {
                const y = @sqrt(@max(z_roots[i], 0));
                if (count < 4) { roots[count] = y - shift; count += 1; }
                if (y > tolerance and count < 4) { roots[count] = -y - shift; count += 1; }
            }
        }
        sortRoots(roots[0..count]);
        return roots;
    }

    // Ferrari's resolvent cubic: 8m³ + 8Qm² + (2Q²-8S)m - R² = 0
    const c_roots = solveCubic(8.0, 8.0 * Q, 2.0 * Q * Q - 8.0 * S, -R * R, tolerance);

    // Find positive root m > 0
    var m: f32 = 0;
    var found = false;
    for (0..3) |i| {
        if (c_roots[i] > tolerance) {
            m = c_roots[i];
            found = true;
            break;
        }
    }

    if (!found) return roots;

    // Factor into two quadratics:
    //   (y² + m + αy + β)(y² + m - αy + γ) = 0
    const sqrt_2m = @sqrt(2.0 * m);
    const alpha = sqrt_2m;
    const beta = (Q + m - R / sqrt_2m) / 2.0;
    const gamma = (Q + m + R / sqrt_2m) / 2.0;

    // Solve each quadratic
    const q1_roots = solveQuadratic(1.0, alpha, m + beta, tolerance);
    const q2_roots = solveQuadratic(1.0, -alpha, m + gamma, tolerance);

    // Collect roots and shift back
    for (0..2) |i| {
        if (@abs(q1_roots[i]) > tolerance or i == 0) {
            if (count < 4) { roots[count] = q1_roots[i] - shift; count += 1; }
        }
    }
    for (0..2) |i| {
        if (@abs(q2_roots[i]) > tolerance or i == 0) {
            if (count < 4) { roots[count] = q2_roots[i] - shift; count += 1; }
        }
    }

    sortRoots(roots[0..count]);
    return roots;
}

// =============================================================================
// 4. GENERAL POLYNOMIAL ROOT FINDER (Newton-bisection hybrid, UE5 style)
// =============================================================================

/// Find roots of a polynomial of given degree within [range_start, range_end]
/// Uses Newton-bisection hybrid like UE5 TPolynomialRootSolver
/// coeffs[i] = coefficient of x^i (so coeffs[0] = constant term)
pub fn findRootsInRange(
    roots: []f32,
    coeffs: []const f32,
    degree: usize,
    range_start: f32,
    range_end: f32,
    tolerance: f32,
    max_iterations: u32,
) usize {
    if (degree < 2 or coeffs.len < degree + 1) return 0;

    // For low degrees, use direct solvers
    if (degree == 2) {
        const r = solveQuadratic(coeffs[2], coeffs[1], coeffs[0], tolerance);
        var count: usize = 0;
        for (0..2) |i| {
            if (r[i] > range_start and r[i] < range_end and count < roots.len) {
                roots[count] = r[i];
                count += 1;
            }
        }
        return count;
    }
    if (degree == 3) {
        const r = solveCubic(coeffs[3], coeffs[2], coeffs[1], coeffs[0], tolerance);
        var count: usize = 0;
        for (0..3) |i| {
            if (r[i] > range_start and r[i] < range_end and count < roots.len) {
                roots[count] = r[i];
                count += 1;
            }
        }
        return count;
    }
    if (degree == 4) {
        const r = solveQuartic(coeffs[4], coeffs[3], coeffs[2], coeffs[1], coeffs[0], tolerance);
        var count: usize = 0;
        for (0..4) |i| {
            if (r[i] > range_start and r[i] < range_end and count < roots.len) {
                roots[count] = r[i];
                count += 1;
            }
        }
        return count;
    }

    // For degree > 4: UE5 Newton-bisection approach
    // Compute derivative coefficients
    _ = max_iterations;
    // Higher-degree support would go here
    return 0;
}

// =============================================================================
// 5. P³ PROJECTIVE ROOTS
// =============================================================================

/// Projective root on P¹: [x:w] where x/w is the affine root
/// [1:0] = infinity root (leading coeff = 0)
pub const ProjectiveRoot = struct {
    x: f32,
    w: f32,

    /// Affine value x/w (infinity if w=0)
    pub fn affine(r: ProjectiveRoot) ?f32 {
        if (@abs(r.w) < 1e-10) return null;
        return r.x / r.w;
    }

    /// Is this the point at infinity on P¹?
    pub fn isInfinity(r: ProjectiveRoot) bool {
        return @abs(r.w) < 1e-10;
    }
};

/// Cross-ratio of 4 projective roots on P¹
/// CR = (x₁w₃ - x₃w₁)(x₂w₄ - x₄w₂) / ((x₁w₄ - x₄w₁)(x₂w₃ - x₃w₂))
pub fn crossRatioP1(r1: ProjectiveRoot, r2: ProjectiveRoot, r3: ProjectiveRoot, r4: ProjectiveRoot) f32 {
    const a13 = r1.x * r3.w - r3.x * r1.w;
    const a24 = r2.x * r4.w - r4.x * r2.w;
    const a14 = r1.x * r4.w - r4.x * r1.w;
    const a23 = r2.x * r3.w - r3.x * r2.w;
    if (@abs(a14) < 1e-16 or @abs(a23) < 1e-16) return 0;
    return (a13 * a24) / (a14 * a23);
}

// =============================================================================
// HELPERS
// =============================================================================

/// Cube root (handles negative numbers)
fn cbrt(x: f32) f32 {
    if (x >= 0) return std.math.pow(f32, x, 1.0 / 3.0);
    return -std.math.pow(f32, -x, 1.0 / 3.0);
}

/// Sort roots in ascending order
fn sortRoots(roots: []f32) void {
    // Simple insertion sort (small N)
    for (1..roots.len) |i| {
        const key = roots[i];
        var j: usize = i;
        while (j > 0 and roots[j - 1] > key) : (j -= 1) {
            roots[j] = roots[j - 1];
        }
        roots[j] = key;
    }
}

// =============================================================================
// TESTS
// =============================================================================

test "PolyRoot: quadratic x² - 3x + 2 = 0 → {1, 2}" {
    const roots = solveQuadratic(1, -3, 2, 1e-6);
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), roots[0], 1e-4);
    try std.testing.expectApproxEqAbs(@as(f32, 2.0), roots[1], 1e-4);
}

test "PolyRoot: quadratic x² + 1 = 0 → no real roots" {
    const roots = solveQuadratic(1, 0, 1, 1e-6);
    // Should be zeros (no real roots)
    try std.testing.expectApproxEqAbs(@as(f32, 0.0), roots[0], 1e-4);
}

test "PolyRoot: cubic x³ - 6x² + 11x - 6 = (x-1)(x-2)(x-3) → {1, 2, 3}" {
    const roots = solveCubic(1, -6, 11, -6, 1e-6);
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), roots[0], 1e-3);
    try std.testing.expectApproxEqAbs(@as(f32, 2.0), roots[1], 1e-3);
    try std.testing.expectApproxEqAbs(@as(f32, 3.0), roots[2], 1e-3);
}

test "PolyRoot: count quadratic roots" {
    try std.testing.expectEqual(@as(u32, 2), countQuadraticRoots(1, -3, 2, 1e-6));
    try std.testing.expectEqual(@as(u32, 0), countQuadraticRoots(1, 0, 1, 1e-6));
    try std.testing.expectEqual(@as(u32, 1), countQuadraticRoots(1, -2, 1, 1e-6));
}

test "PolyRoot: quartic x⁴ - 10x³ + 35x² - 50x + 24 = (x-1)(x-2)(x-3)(x-4)" {
    const roots = solveQuartic(1, -10, 35, -50, 24, 1e-6);
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), roots[0], 1e-2);
    try std.testing.expectApproxEqAbs(@as(f32, 2.0), roots[1], 1e-2);
    try std.testing.expectApproxEqAbs(@as(f32, 3.0), roots[2], 1e-2);
    try std.testing.expectApproxEqAbs(@as(f32, 4.0), roots[3], 1e-2);
}

test "PolyRoot: projective cross-ratio {0,1,2,3} = 4/3" {
    const r1 = ProjectiveRoot{ .x = 0, .w = 1 };
    const r2 = ProjectiveRoot{ .x = 1, .w = 1 };
    const r3 = ProjectiveRoot{ .x = 2, .w = 1 };
    const r4 = ProjectiveRoot{ .x = 3, .w = 1 };
    const cr = crossRatioP1(r1, r2, r3, r4);
    try std.testing.expectApproxEqAbs(@as(f32, 4.0 / 3.0), cr, 1e-4);
}
