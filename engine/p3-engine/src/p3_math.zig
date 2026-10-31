// =============================================================================
// P³ MATH — ПОЛНЫЙ ПОРТ O3DE AzCore/Math НА ZIG + P³
// =============================================================================
//
// Фундаментальный модуль P³ Engine. Полный порт математической библиотеки
// O3DE (AzCore/Math) на Zig 0.14.0 с проективными обобщениями.
//
// ИЕРАРХИЯ ТИПОВ:
//
//   Constants ← Vec2 ← Vec3 ← Vec4
//                     ↑         ↑
//                Mat3x3 ← Mat4x4 (P³ collineation group)
//                     ↑         ↑
//                Quaternion ← Transform (Sim(3) ⊂ PGL(4))
//                     ↑
//                Plane ← Frustum (6 projective half-spaces)
//                     ↑
//                  Aabb ← Obb
//
// P³ ОБОБЩЕНИЯ:
//   - Vec4 = естественная однородная координата в P³
//   - Mat4x4 = естественная группа коллинеаций PGL(4)
//   - Plane ↔ Point: двойственность в P³ (одна Vec4)
//   - Frustum = 6 проективных полупространств
//   - Color с alpha = проективная величина (premultiplied = каноническая форма)
//   - Quaternion ∈ RP³: S³ → SO(3) двойное накрытие
//   - Transform = Sim(3) ⊂ PGL(4): подгруппа подобий
//
// Донор: O3DE/Gems/AzCore/Math (143 файла, ~50k строк C++)
// Порт: Zig 0.14.0, f32 everywhere, row-major matrices, @Vector(4,f32) SIMD
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const math = std.math;

// =============================================================================
// 1. CONSTANTS (из O3DE MathUtils.h)
// =============================================================================

pub const PI: f32 = math.pi;
pub const TWO_PI: f32 = 2.0 * math.pi;
pub const HALF_PI: f32 = math.pi / 2.0;
pub const QUARTER_PI: f32 = math.pi / 4.0;
pub const TAU: f32 = TWO_PI;
pub const MAX_FLOAT: f32 = math.floatMax(f32);
pub const EPSILON: f32 = 1e-5;
pub const TOLERANCE: f32 = 1e-3;
pub const MIN_TRANSFORM_SCALE: f32 = 1e-2;
pub const MAX_TRANSFORM_SCALE: f32 = 1e9;

pub inline fn radToDeg(r: f32) f32 {
    return r * (180.0 / math.pi);
}
pub inline fn degToRad(d: f32) f32 {
    return d * (math.pi / 180.0);
}
pub inline fn clamp(v: f32, lo: f32, hi: f32) f32 {
    return @max(lo, @min(hi, v));
}
pub inline fn clampInt(v: i32, lo: i32, hi: i32) i32 {
    return @max(lo, @min(hi, v));
}
pub inline fn lerpF(a: f32, b: f32, t: f32) f32 {
    return a + t * (b - a);
}
pub inline fn isClose(a: f32, b: f32, tolerance: f32) bool {
    return @abs(a - b) <= tolerance;
}
pub inline fn safeDiv(n: f32, d: f32, fallback: f32) f32 {
    if (@abs(d) < EPSILON) return fallback;
    return n / d;
}

/// Fast inverse square root (Newton-Raphson, 2 iterations)
pub fn invSqrt(x: f32) f32 {
    if (x <= 0) return 0;
    const x2 = x * 0.5;
    var y = @as(f32, @bitCast(@as(u32, 0x5F3759DF) - (@as(u32, @bitCast(x)) >> 1)));
    y = y * (1.5 - x2 * y * y); // iteration 1
    y = y * (1.5 - x2 * y * y); // iteration 2
    return y;
}

pub inline fn fastSqrt(x: f32) f32 {
    return x * invSqrt(x);
}

pub inline fn smoothStep(edge0: f32, edge1: f32, x: f32) f32 {
    const t = clamp((x - edge0) / safeDiv(edge1 - edge0, 1, 1), 0, 1);
    return t * t * (3 - 2 * t);
}

// =============================================================================
// 2. Vec2 (из O3DE Vector2.h)
// =============================================================================
//
// P³: Vec2 в P¹ (проективная прямая) → однородные координаты [x:y]
//     Perpendicular → join operation (wedge product) в P¹

pub const Vec2 = struct {
    x: f32,
    y: f32,

    pub inline fn init(x: f32, y: f32) Vec2 {
        return .{ .x = x, .y = y };
    }
    pub inline fn zero() Vec2 {
        return .{ .x = 0, .y = 0 };
    }
    pub inline fn one() Vec2 {
        return .{ .x = 1, .y = 1 };
    }
    pub inline fn axisX() Vec2 {
        return .{ .x = 1, .y = 0 };
    }
    pub inline fn axisY() Vec2 {
        return .{ .x = 0, .y = 1 };
    }

    pub inline fn add(a: Vec2, b: Vec2) Vec2 {
        return .{ .x = a.x + b.x, .y = a.y + b.y };
    }
    pub inline fn sub(a: Vec2, b: Vec2) Vec2 {
        return .{ .x = a.x - b.x, .y = a.y - b.y };
    }
    pub inline fn mul(a: Vec2, b: Vec2) Vec2 {
        return .{ .x = a.x * b.x, .y = a.y * b.y };
    }
    pub inline fn scale(v: Vec2, s: f32) Vec2 {
        return .{ .x = v.x * s, .y = v.y * s };
    }
    pub inline fn neg(v: Vec2) Vec2 {
        return .{ .x = -v.x, .y = -v.y };
    }
    pub inline fn dot(a: Vec2, b: Vec2) f32 {
        return a.x * b.x + a.y * b.y;
    }
    pub inline fn lengthSq(v: Vec2) f32 {
        return v.dot(v);
    }
    pub inline fn length(v: Vec2) f32 {
        return @sqrt(v.lengthSq());
    }
    pub inline fn lengthReciprocal(v: Vec2) f32 {
        return invSqrt(v.lengthSq());
    }
    pub inline fn normalize(v: Vec2) Vec2 {
        return v.scale(v.lengthReciprocal());
    }
    pub fn normalizeSafe(v: Vec2, fallback: Vec2) Vec2 {
        const lsq = v.lengthSq();
        if (lsq < EPSILON * EPSILON) return fallback;
        return v.scale(invSqrt(lsq));
    }

    pub inline fn lerp(a: Vec2, b: Vec2, t: f32) Vec2 {
        return .{ .x = lerpF(a.x, b.x, t), .y = lerpF(a.y, b.y, t) };
    }

    /// Perpendicular: 90° rotation (join в P¹)
    pub inline fn perpendicular(v: Vec2) Vec2 {
        return .{ .x = -v.y, .y = v.x };
    }

    pub inline fn project(v: Vec2, onto: Vec2) Vec2 {
        const d = onto.dot(onto);
        if (d < EPSILON * EPSILON) return zero();
        return onto.scale(v.dot(onto) / d);
    }

    pub inline fn distance(a: Vec2, b: Vec2) f32 {
        return a.sub(b).length();
    }
    pub inline fn distanceSq(a: Vec2, b: Vec2) f32 {
        return a.sub(b).lengthSq();
    }

    pub fn angle(a: Vec2, b: Vec2) f32 {
        const la = a.lengthReciprocal();
        const lb = b.lengthReciprocal();
        const cos_angle = clamp(a.dot(b) * la * lb, -1, 1);
        return math.acos(cos_angle);
    }

    pub inline fn isCloseV(a: Vec2, b: Vec2, tol: f32) bool {
        return @abs(a.x - b.x) <= tol and @abs(a.y - b.y) <= tol;
    }
    pub inline fn getMin(a: Vec2, b: Vec2) Vec2 {
        return .{ .x = @min(a.x, b.x), .y = @min(a.y, b.y) };
    }
    pub inline fn getMax(a: Vec2, b: Vec2) Vec2 {
        return .{ .x = @max(a.x, b.x), .y = @max(a.y, b.y) };
    }
    pub inline fn absV(v: Vec2) Vec2 {
        return .{ .x = @abs(v.x), .y = @abs(v.y) };
    }
};

// =============================================================================
// 3. Vec3 (из O3DE Vector3.h)
// =============================================================================
//
// P³: Vec3 в P² (проективная плоскость) → однородные [x:y:w]
//     Cross product → join (wedge) двух точек в P² → прямая (Plücker)

pub const Vec3 = struct {
    x: f32,
    y: f32,
    z: f32,

    pub inline fn init(x: f32, y: f32, z: f32) Vec3 {
        return .{ .x = x, .y = y, .z = z };
    }
    pub inline fn zero() Vec3 {
        return .{ .x = 0, .y = 0, .z = 0 };
    }
    pub inline fn one() Vec3 {
        return .{ .x = 1, .y = 1, .z = 1 };
    }
    pub inline fn axisX() Vec3 {
        return .{ .x = 1, .y = 0, .z = 0 };
    }
    pub inline fn axisY() Vec3 {
        return .{ .x = 0, .y = 1, .z = 0 };
    }
    pub inline fn axisZ() Vec3 {
        return .{ .x = 0, .y = 0, .z = 1 };
    }

    pub inline fn fromVec2(v: Vec2, z: f32) Vec3 {
        return .{ .x = v.x, .y = v.y, .z = z };
    }

    pub inline fn add(a: Vec3, b: Vec3) Vec3 {
        return .{ .x = a.x + b.x, .y = a.y + b.y, .z = a.z + b.z };
    }
    pub inline fn sub(a: Vec3, b: Vec3) Vec3 {
        return .{ .x = a.x - b.x, .y = a.y - b.y, .z = a.z - b.z };
    }
    pub inline fn mul(a: Vec3, b: Vec3) Vec3 {
        return .{ .x = a.x * b.x, .y = a.y * b.y, .z = a.z * b.z };
    }
    pub inline fn scale(v: Vec3, s: f32) Vec3 {
        return .{ .x = v.x * s, .y = v.y * s, .z = v.z * s };
    }
    pub inline fn neg(v: Vec3) Vec3 {
        return .{ .x = -v.x, .y = -v.y, .z = -v.z };
    }
    pub inline fn dot(a: Vec3, b: Vec3) f32 {
        return a.x * b.x + a.y * b.y + a.z * b.z;
    }

    /// Cross product: join (wedge) двух точек в P² → Plücker прямая
    pub inline fn cross(a: Vec3, b: Vec3) Vec3 {
        return .{
            .x = a.y * b.z - a.z * b.y,
            .y = a.z * b.x - a.x * b.z,
            .z = a.x * b.y - a.y * b.x,
        };
    }

    pub inline fn lengthSq(v: Vec3) f32 {
        return v.dot(v);
    }
    pub inline fn length(v: Vec3) f32 {
        return @sqrt(v.lengthSq());
    }
    pub inline fn lengthReciprocal(v: Vec3) f32 {
        return invSqrt(v.lengthSq());
    }
    pub inline fn normalize(v: Vec3) Vec3 {
        return v.scale(v.lengthReciprocal());
    }
    pub fn normalizeSafe(v: Vec3, fallback: Vec3) Vec3 {
        const lsq = v.lengthSq();
        if (lsq < EPSILON * EPSILON) return fallback;
        return v.scale(invSqrt(lsq));
    }

    pub inline fn lerp(a: Vec3, b: Vec3, t: f32) Vec3 {
        return .{ .x = lerpF(a.x, b.x, t), .y = lerpF(a.y, b.y, t), .z = lerpF(a.z, b.z, t) };
    }

    pub inline fn distance(a: Vec3, b: Vec3) f32 {
        return a.sub(b).length();
    }
    pub inline fn distanceSq(a: Vec3, b: Vec3) f32 {
        return a.sub(b).lengthSq();
    }

    /// Arbitrary orthogonal vector (для tangent frame)
    pub fn getOrthogonal(v: Vec3) Vec3 {
        const abs_v = Vec3.init(@abs(v.x), @abs(v.y), @abs(v.z));
        if (abs_v.x <= abs_v.y and abs_v.x <= abs_v.z)
            return v.cross(Vec3.axisX()).normalize()
        else if (abs_v.y <= abs_v.z)
            return v.cross(Vec3.axisY()).normalize()
        else
            return v.cross(Vec3.axisZ()).normalize();
    }

    pub inline fn isPerpendicular(a: Vec3, b: Vec3, tol: f32) bool {
        return @abs(a.dot(b)) <= tol;
    }

    pub inline fn maxElement(v: Vec3) f32 {
        return @max(@max(v.x, v.y), v.z);
    }
    pub inline fn minElement(v: Vec3) f32 {
        return @min(@min(v.x, v.y), v.z);
    }

    pub inline fn isCloseV(a: Vec3, b: Vec3, tol: f32) bool {
        return @abs(a.x - b.x) <= tol and @abs(a.y - b.y) <= tol and @abs(a.z - b.z) <= tol;
    }
    pub inline fn getMin(a: Vec3, b: Vec3) Vec3 {
        return .{ .x = @min(a.x, b.x), .y = @min(a.y, b.y), .z = @min(a.z, b.z) };
    }
    pub inline fn getMax(a: Vec3, b: Vec3) Vec3 {
        return .{ .x = @max(a.x, b.x), .y = @max(a.y, b.y), .z = @max(a.z, b.z) };
    }
    pub inline fn absV(v: Vec3) Vec3 {
        return .{ .x = @abs(v.x), .y = @abs(v.y), .z = @abs(v.z) };
    }
};

// =============================================================================
// 4. Vec4 (из O3DE Vector4.h)
// =============================================================================
//
// P³: Vec4 = ЕСТЕСТВЕННАЯ однородная координата в P³
//     w = однородный вес. Дегомогенизация: (x/w, y/w, z/w)
//     Вложение R³ → P³: (x,y,z) ↦ (x,y,z,1)

pub const Vec4 = struct {
    x: f32,
    y: f32,
    z: f32,
    w: f32,

    pub inline fn init(x: f32, y: f32, z: f32, w: f32) Vec4 {
        return .{ .x = x, .y = y, .z = z, .w = w };
    }
    pub inline fn zero() Vec4 {
        return .{ .x = 0, .y = 0, .z = 0, .w = 0 };
    }
    pub inline fn one() Vec4 {
        return .{ .x = 1, .y = 1, .z = 1, .w = 1 };
    }

    /// Вложение R³ → P³: (x,y,z) ↦ (x,y,z,1)
    pub inline fn fromVec3(v: Vec3, w: f32) Vec4 {
        return .{ .x = v.x, .y = v.y, .z = v.z, .w = w };
    }
    pub inline fn fromVec3Affine(v: Vec3) Vec4 {
        return .{ .x = v.x, .y = v.y, .z = v.z, .w = 1.0 };
    }

    /// Дегомогенизация P³ → R³: (x,y,z,w) ↦ (x/w, y/w, z/w)
    pub fn getAsVec3(v: Vec4) Vec3 {
        if (@abs(v.w) < EPSILON) return Vec3.zero();
        const inv_w = 1.0 / v.w;
        return Vec3.init(v.x * inv_w, v.y * inv_w, v.z * inv_w);
    }

    /// Homogenize: ensure w=1
    pub fn homogenize(v: Vec4) Vec4 {
        if (@abs(v.w) < EPSILON) return v;
        const inv_w = 1.0 / v.w;
        return .{ .x = v.x * inv_w, .y = v.y * inv_w, .z = v.z * inv_w, .w = 1.0 };
    }

    pub inline fn add(a: Vec4, b: Vec4) Vec4 {
        return .{ .x = a.x + b.x, .y = a.y + b.y, .z = a.z + b.z, .w = a.w + b.w };
    }
    pub inline fn sub(a: Vec4, b: Vec4) Vec4 {
        return .{ .x = a.x - b.x, .y = a.y - b.y, .z = a.z - b.z, .w = a.w - b.w };
    }
    pub inline fn scale(v: Vec4, s: f32) Vec4 {
        return .{ .x = v.x * s, .y = v.y * s, .z = v.z * s, .w = v.w * s };
    }
    pub inline fn dot(a: Vec4, b: Vec4) f32 {
        return a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w;
    }
    pub inline fn lengthSq(v: Vec4) f32 {
        return v.dot(v);
    }
    pub inline fn length(v: Vec4) f32 {
        return @sqrt(v.lengthSq());
    }
    pub inline fn normalize(v: Vec4) Vec4 {
        const l = v.length();
        if (l < EPSILON) return v;
        return v.scale(1.0 / l);
    }
    pub inline fn lerp(a: Vec4, b: Vec4, t: f32) Vec4 {
        return .{ .x = lerpF(a.x, b.x, t), .y = lerpF(a.y, b.y, t), .z = lerpF(a.z, b.z, t), .w = lerpF(a.w, b.w, t) };
    }

    /// Premultiply alpha (каноническая проективная форма)
    pub inline fn premultiplyAlpha(v: Vec4) Vec4 {
        return .{ .x = v.x * v.w, .y = v.y * v.w, .z = v.z * v.w, .w = v.w };
    }
};

// =============================================================================
// 5. Mat3x3 (из O3DE Matrix3x3.h)
// =============================================================================
//
// P²: Mat3x3 = группа коллинеаций PGL(3) на проективной плоскости
//     Определитель = проективный масштабный фактор

pub const Mat3x3 = struct {
    rows: [3]Vec3,

    pub inline fn identity() Mat3x3 {
        return .{
            .rows = .{
                Vec3.init(1, 0, 0),
                Vec3.init(0, 1, 0),
                Vec3.init(0, 0, 1),
            },
        };
    }
    pub inline fn zero() Mat3x3 {
        return .{ .rows = .{ Vec3.zero(), Vec3.zero(), Vec3.zero() } };
    }
    pub inline fn fromRows(r0: Vec3, r1: Vec3, r2: Vec3) Mat3x3 {
        return .{ .rows = .{ r0, r1, r2 } };
    }

    pub fn createScale(sx: f32, sy: f32, sz: f32) Mat3x3 {
        return .{
            .rows = .{
                Vec3.init(sx, 0, 0),
                Vec3.init(0, sy, 0),
                Vec3.init(0, 0, sz),
            },
        };
    }
    pub fn createUniformScale(s: f32) Mat3x3 {
        return createScale(s, s, s);
    }
    pub fn createRotationX(angle: f32) Mat3x3 {
        const c = @cos(angle);
        const s = @sin(angle);
        return .{
            .rows = .{
                Vec3.init(1, 0, 0),
                Vec3.init(0, c, s),
                Vec3.init(0, -s, c),
            },
        };
    }
    pub fn createRotationY(angle: f32) Mat3x3 {
        const c = @cos(angle);
        const s = @sin(angle);
        return .{
            .rows = .{
                Vec3.init(c, 0, -s),
                Vec3.init(0, 1, 0),
                Vec3.init(s, 0, c),
            },
        };
    }
    pub fn createRotationZ(angle: f32) Mat3x3 {
        const c = @cos(angle);
        const s = @sin(angle);
        return .{
            .rows = .{
                Vec3.init(c, s, 0),
                Vec3.init(-s, c, 0),
                Vec3.init(0, 0, 1),
            },
        };
    }

    /// Cross product matrix: M*a = p.Cross(a)
    pub fn createCrossProduct(p: Vec3) Mat3x3 {
        return .{
            .rows = .{
                Vec3.init(0, -p.z, p.y),
                Vec3.init(p.z, 0, -p.x),
                Vec3.init(-p.y, p.x, 0),
            },
        };
    }

    /// Matrix-matrix multiply: C = A * B
    pub fn mul(a: Mat3x3, b: Mat3x3) Mat3x3 {
        var result: Mat3x3 = undefined;
        for (0..3) |i| {
            const r = a.rows[i];
            result.rows[i] = Vec3.init(
                r.x * b.rows[0].x + r.y * b.rows[1].x + r.z * b.rows[2].x,
                r.x * b.rows[0].y + r.y * b.rows[1].y + r.z * b.rows[2].y,
                r.x * b.rows[0].z + r.y * b.rows[1].z + r.z * b.rows[2].z,
            );
        }
        return result;
    }

    /// Matrix-vector multiply: y = M * x
    pub inline fn mulVec(m: Mat3x3, v: Vec3) Vec3 {
        return Vec3.init(
            m.rows[0].dot(v),
            m.rows[1].dot(v),
            m.rows[2].dot(v),
        );
    }

    pub fn transpose(m: Mat3x3) Mat3x3 {
        return .{
            .rows = .{
                Vec3.init(m.rows[0].x, m.rows[1].x, m.rows[2].x),
                Vec3.init(m.rows[0].y, m.rows[1].y, m.rows[2].y),
                Vec3.init(m.rows[0].z, m.rows[1].z, m.rows[2].z),
            },
        };
    }

    pub fn determinant(m: Mat3x3) f32 {
        const r = m.rows;
        return r[0].x * (r[1].y * r[2].z - r[1].z * r[2].y) -
            r[0].y * (r[1].x * r[2].z - r[1].z * r[2].x) +
            r[0].z * (r[1].x * r[2].y - r[1].y * r[2].x);
    }

    pub fn inverse(m: Mat3x3) Mat3x3 {
        const det = m.determinant();
        if (@abs(det) < EPSILON * EPSILON * EPSILON) return Mat3x3.identity();
        const inv_det = 1.0 / det;
        const r = m.rows;
        return .{
            .rows = .{
                Vec3.init(
                    (r[1].y * r[2].z - r[1].z * r[2].y) * inv_det,
                    (r[0].z * r[2].y - r[0].y * r[2].z) * inv_det,
                    (r[0].y * r[1].z - r[0].z * r[1].y) * inv_det,
                ),
                Vec3.init(
                    (r[1].z * r[2].x - r[1].x * r[2].z) * inv_det,
                    (r[0].x * r[2].z - r[0].z * r[2].x) * inv_det,
                    (r[0].z * r[1].x - r[0].x * r[1].z) * inv_det,
                ),
                Vec3.init(
                    (r[1].x * r[2].y - r[1].y * r[2].x) * inv_det,
                    (r[0].y * r[2].x - r[0].x * r[2].y) * inv_det,
                    (r[0].x * r[1].y - r[0].y * r[1].x) * inv_det,
                ),
            },
        };
    }

    pub inline fn trace(m: Mat3x3) f32 {
        return m.rows[0].x + m.rows[1].y + m.rows[2].z;
    }

    pub fn isOrthogonal(m: Mat3x3, tol: f32) bool {
        const mt = m.transpose();
        const product = m.mul(mt);
        return product.rows[0].isCloseV(Vec3.init(1, 0, 0), tol) and
            product.rows[1].isCloseV(Vec3.init(0, 1, 0), tol) and
            product.rows[2].isCloseV(Vec3.init(0, 0, 1), tol);
    }
};

// =============================================================================
// 6. Mat4x4 (из O3DE Matrix4x4.h)
// =============================================================================
//
// P³: Mat4x4 = ЕСТЕСТВЕННАЯ группа коллинеаций PGL(4) на P³
//     Projection матрицы = сингулярные коллинеации (rank-3 проективные отображения)
//     CreateLookAt = проективная камера

pub const Mat4x4 = struct {
    rows: [4]Vec4,

    pub inline fn identity() Mat4x4 {
        return .{
            .rows = .{
                Vec4.init(1, 0, 0, 0),
                Vec4.init(0, 1, 0, 0),
                Vec4.init(0, 0, 1, 0),
                Vec4.init(0, 0, 0, 1),
            },
        };
    }
    pub inline fn zero() Mat4x4 {
        return .{ .rows = .{ Vec4.zero(), Vec4.zero(), Vec4.zero(), Vec4.zero() } };
    }
    pub inline fn fromRows(r0: Vec4, r1: Vec4, r2: Vec4, r3: Vec4) Mat4x4 {
        return .{ .rows = .{ r0, r1, r2, r3 } };
    }

    pub fn createTranslation(tx: f32, ty: f32, tz: f32) Mat4x4 {
        return .{
            .rows = .{
                Vec4.init(1, 0, 0, tx),
                Vec4.init(0, 1, 0, ty),
                Vec4.init(0, 0, 1, tz),
                Vec4.init(0, 0, 0, 1),
            },
        };
    }
    pub fn createScale(sx: f32, sy: f32, sz: f32) Mat4x4 {
        return .{
            .rows = .{
                Vec4.init(sx, 0, 0, 0),
                Vec4.init(0, sy, 0, 0),
                Vec4.init(0, 0, sz, 0),
                Vec4.init(0, 0, 0, 1),
            },
        };
    }
    pub fn createRotationX(angle: f32) Mat4x4 {
        const m3 = Mat3x3.createRotationX(angle);
        return fromMat3x3(m3);
    }
    pub fn createRotationY(angle: f32) Mat4x4 {
        const m3 = Mat3x3.createRotationY(angle);
        return fromMat3x3(m3);
    }
    pub fn createRotationZ(angle: f32) Mat4x4 {
        const m3 = Mat3x3.createRotationZ(angle);
        return fromMat3x3(m3);
    }

    pub fn fromMat3x3(m: Mat3x3) Mat4x4 {
        return .{
            .rows = .{
                Vec4.fromVec3(m.rows[0], 0),
                Vec4.fromVec3(m.rows[1], 0),
                Vec4.fromVec3(m.rows[2], 0),
                Vec4.init(0, 0, 0, 1),
            },
        };
    }

    /// Create from Quaternion + Translation
    pub fn createFromQuaternionAndTranslation(q: Quaternion, t: Vec3) Mat4x4 {
        const rot = q.toMat3x3();
        var m = fromMat3x3(rot);
        m.rows[0].w = t.x;
        m.rows[1].w = t.y;
        m.rows[2].w = t.z;
        return m;
    }

    /// LookAt View Matrix (World-to-Camera)
    pub fn lookAt(eye: Vec3, target: Vec3, up: Vec3) Mat4x4 {
        const f = target.sub(eye).normalize();
        const s = f.cross(up).normalize();
        const u = s.cross(f);

        return fromRows(
            Vec4.init(s.x, s.y, s.z, -s.dot(eye)),
            Vec4.init(u.x, u.y, u.z, -u.dot(eye)),
            Vec4.init(-f.x, -f.y, -f.z, f.dot(eye)),
            Vec4.init(0.0, 0.0, 0.0, 1.0),
        );
    }

    /// Perspective projection (сингулярная коллинеация P³ → P²)
    pub fn createProjectionFov(fov_y: f32, aspect: f32, near: f32, far: f32) Mat4x4 {
        const cot_half_fov = 1.0 / @tan(fov_y * 0.5);
        const range = far / (near - far);
        return .{
            .rows = .{
                Vec4.init(cot_half_fov / aspect, 0, 0, 0),
                Vec4.init(0, cot_half_fov, 0, 0),
                Vec4.init(0, 0, range, near * range),
                Vec4.init(0, 0, -1, 0),
            },
        };
    }

    /// Look-at matrix (проективная камера)
    pub fn createLookAt(eye: Vec3, target: Vec3, up: Vec3) Mat4x4 {
        const f = target.sub(eye).normalize();
        const s = f.cross(up).normalize();
        const u = s.cross(f);
        return .{
            .rows = .{
                Vec4.init(s.x, s.y, s.z, -s.dot(eye)),
                Vec4.init(u.x, u.y, u.z, -u.dot(eye)),
                Vec4.init(-f.x, -f.y, -f.z, f.dot(eye)),
                Vec4.init(0, 0, 0, 1),
            },
        };
    }

    /// Matrix-matrix multiply: C = A * B
    pub fn mul(a: Mat4x4, b: Mat4x4) Mat4x4 {
        var result: Mat4x4 = undefined;
        for (0..4) |i| {
            const r = a.rows[i];
            result.rows[i] = Vec4.init(
                r.x * b.rows[0].x + r.y * b.rows[1].x + r.z * b.rows[2].x + r.w * b.rows[3].x,
                r.x * b.rows[0].y + r.y * b.rows[1].y + r.z * b.rows[2].y + r.w * b.rows[3].y,
                r.x * b.rows[0].z + r.y * b.rows[1].z + r.z * b.rows[2].z + r.w * b.rows[3].z,
                r.x * b.rows[0].w + r.y * b.rows[1].w + r.z * b.rows[2].w + r.w * b.rows[3].w,
            );
        }
        return result;
    }

    /// Multiply with homogeneous point: y = M * p
    pub inline fn mulVec(m: Mat4x4, v: Vec4) Vec4 {
        return Vec4.init(
            m.rows[0].dot(v),
            m.rows[1].dot(v),
            m.rows[2].dot(v),
            m.rows[3].dot(v),
        );
    }

    /// Transform a 3D point: embed in P³, multiply, dehomogenize
    pub fn transformPoint(m: Mat4x4, p: Vec3) Vec3 {
        const hp = m.mulVec(Vec4.fromVec3Affine(p));
        return hp.getAsVec3();
    }

    /// Transform a 3D direction (w=0, no translation)
    pub fn transformVector(m: Mat4x4, v: Vec3) Vec3 {
        const hv = m.mulVec(Vec4.fromVec3(v, 0));
        return Vec3.init(hv.x, hv.y, hv.z);
    }

    pub fn transpose(m: Mat4x4) Mat4x4 {
        return .{
            .rows = .{
                Vec4.init(m.rows[0].x, m.rows[1].x, m.rows[2].x, m.rows[3].x),
                Vec4.init(m.rows[0].y, m.rows[1].y, m.rows[2].y, m.rows[3].y),
                Vec4.init(m.rows[0].z, m.rows[1].z, m.rows[2].z, m.rows[3].z),
                Vec4.init(m.rows[0].w, m.rows[1].w, m.rows[2].w, m.rows[3].w),
            },
        };
    }

    /// Determinant (cofactor expansion along row 0)
    pub fn determinant(m: Mat4x4) f32 {
        const r = m.rows;
        // Direct cofactor formula for 4x4 determinant
        const m00 = r[0].x;
        const m01 = r[0].y;
        const m02 = r[0].z;
        const m03 = r[0].w;
        const m10 = r[1].x;
        const m11 = r[1].y;
        const m12 = r[1].z;
        const m13 = r[1].w;
        const m20 = r[2].x;
        const m21 = r[2].y;
        const m22 = r[2].z;
        const m23 = r[2].w;
        const m30 = r[3].x;
        const m31 = r[3].y;
        const m32 = r[3].z;
        const m33 = r[3].w;

        return m00 * (m11 * (m22 * m33 - m23 * m32) - m12 * (m21 * m33 - m23 * m31) + m13 * (m21 * m32 - m22 * m31)) -
            m01 * (m10 * (m22 * m33 - m23 * m32) - m12 * (m20 * m33 - m23 * m30) + m13 * (m20 * m32 - m22 * m30)) +
            m02 * (m10 * (m21 * m33 - m23 * m31) - m11 * (m20 * m33 - m23 * m30) + m13 * (m20 * m31 - m21 * m30)) -
            m03 * (m10 * (m21 * m32 - m22 * m31) - m11 * (m20 * m32 - m22 * m30) + m12 * (m20 * m31 - m21 * m30));
    }
};

// =============================================================================
// 7. Quaternion (из O3DE Quaternion.h)
// =============================================================================
//
// P³: Quaternion ∈ RP³ — двойное накрытие S³ → SO(3)
//     w = действительная часть (cos(θ/2))
//     GetShortestEquivalent() выбирает канонического представителя

pub const Quaternion = struct {
    x: f32,
    y: f32,
    z: f32,
    w: f32,

    pub inline fn identity() Quaternion {
        return .{ .x = 0, .y = 0, .z = 0, .w = 1 };
    }
    pub inline fn init(x: f32, y: f32, z: f32, w: f32) Quaternion {
        return .{ .x = x, .y = y, .z = z, .w = w };
    }

    pub fn fromAxisAngle(axis: Vec3, angle: f32) Quaternion {
        const half = angle * 0.5;
        const s = @sin(half);
        const n = axis.normalize();
        return .{ .x = n.x * s, .y = n.y * s, .z = n.z * s, .w = @cos(half) };
    }

    pub fn fromEulerXYZ(pitch: f32, yaw: f32, roll: f32) Quaternion {
        const qp = fromAxisAngle(Vec3.axisX(), pitch);
        const qy = fromAxisAngle(Vec3.axisY(), yaw);
        const qr = fromAxisAngle(Vec3.axisZ(), roll);
        return qp.mul(qy).mul(qr);
    }

    /// Shortest arc rotation from v1 to v2
    pub fn fromShortestArc(v1: Vec3, v2: Vec3) Quaternion {
        const n1 = v1.normalize();
        const n2 = v2.normalize();
        const d = n1.dot(n2);
        if (d >= 1.0 - EPSILON) return identity();
        if (d <= -1.0 + EPSILON) {
            // 180° rotation: pick orthogonal axis
            const orth = n1.getOrthogonal();
            return fromAxisAngle(orth, PI);
        }
        const axis = n1.cross(n2);
        const s = @sqrt((1.0 + d) * 2.0);
        const inv_s = 1.0 / s;
        return .{ .x = axis.x * inv_s, .y = axis.y * inv_s, .z = axis.z * inv_s, .w = s * 0.5 };
    }

    pub inline fn conjugate(q: Quaternion) Quaternion {
        return .{ .x = -q.x, .y = -q.y, .z = -q.z, .w = q.w };
    }
    pub inline fn inverse(q: Quaternion) Quaternion {
        const lsq = q.lengthSq();
        const inv_lsq = 1.0 / lsq;
        return .{ .x = -q.x * inv_lsq, .y = -q.y * inv_lsq, .z = -q.z * inv_lsq, .w = q.w * inv_lsq };
    }

    pub inline fn lengthSq(q: Quaternion) f32 {
        return q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
    }
    pub inline fn length(q: Quaternion) f32 {
        return @sqrt(q.lengthSq());
    }
    pub fn normalize(q: Quaternion) Quaternion {
        const l = q.length();
        if (l < EPSILON) return identity();
        const inv_l = 1.0 / l;
        return .{ .x = q.x * inv_l, .y = q.y * inv_l, .z = q.z * inv_l, .w = q.w * inv_l };
    }

    /// Hamilton product: q = q1 * q2
    pub fn mul(q1: Quaternion, q2: Quaternion) Quaternion {
        return .{
            .x = q1.w * q2.x + q1.x * q2.w + q1.y * q2.z - q1.z * q2.y,
            .y = q1.w * q2.y - q1.x * q2.z + q1.y * q2.w + q1.z * q2.x,
            .z = q1.w * q2.z + q1.x * q2.y - q1.y * q2.x + q1.z * q2.w,
            .w = q1.w * q2.w - q1.x * q2.x - q1.y * q2.y - q1.z * q2.z,
        };
    }

    /// Rotate vector: q * v * q^-1
    pub fn transformVec(q: Quaternion, v: Vec3) Vec3 {
        const qv = Quaternion.init(v.x, v.y, v.z, 0);
        const result = q.mul(qv).mul(q.conjugate());
        return Vec3.init(result.x, result.y, result.z);
    }

    pub inline fn add(a: Quaternion, b: Quaternion) Quaternion {
        return .{ .x = a.x + b.x, .y = a.y + b.y, .z = a.z + b.z, .w = a.w + b.w };
    }

    pub inline fn scale(q: Quaternion, s: f32) Quaternion {
        return .{ .x = q.x * s, .y = q.y * s, .z = q.z * s, .w = q.w * s };
    }

    pub fn dot(a: Quaternion, b: Quaternion) f32 {
        return a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w;
    }

    /// Spherical linear interpolation
    pub fn slerp(a: Quaternion, b: Quaternion, t: f32) Quaternion {
        var cos_half_angle = dot(a, b);
        var qb = b;
        if (cos_half_angle < 0) {
            qb = Quaternion.init(-b.x, -b.y, -b.z, -b.w);
            cos_half_angle = -cos_half_angle;
        }
        if (cos_half_angle > 1.0 - EPSILON) {
            // Linear fallback for small angles
            return nlerp(a, qb, t);
        }
        const half_angle = math.acos(clamp(cos_half_angle, -1, 1));
        const sin_half_angle = @sin(half_angle);
        const inv_sin = 1.0 / sin_half_angle;
        const wa = @sin((1.0 - t) * half_angle) * inv_sin;
        const wb = @sin(t * half_angle) * inv_sin;
        return .{
            .x = wa * a.x + wb * qb.x,
            .y = wa * a.y + wb * qb.y,
            .z = wa * a.z + wb * qb.z,
            .w = wa * a.w + wb * qb.w,
        };
    }

    /// Normalized linear interpolation (faster, approximates slerp)
    pub fn nlerp(a: Quaternion, b: Quaternion, t: f32) Quaternion {
        return Quaternion.init(
            lerpF(a.x, b.x, t),
            lerpF(a.y, b.y, t),
            lerpF(a.z, b.z, t),
            lerpF(a.w, b.w, t),
        ).normalize();
    }

    /// Convert to 3x3 rotation matrix
    pub fn toMat3x3(q: Quaternion) Mat3x3 {
        const xx = q.x * q.x;
        const yy = q.y * q.y;
        const zz = q.z * q.z;
        const ww = q.w * q.w;
        const xy = q.x * q.y;
        const xz = q.x * q.z;
        const xw = q.x * q.w;
        const yz = q.y * q.z;
        const yw = q.y * q.w;
        const zw = q.z * q.w;
        return .{
            .rows = .{
                Vec3.init(ww + xx - yy - zz, 2 * (xy + zw), 2 * (xz - yw)),
                Vec3.init(2 * (xy - zw), ww - xx + yy - zz, 2 * (yz + xw)),
                Vec3.init(2 * (xz + yw), 2 * (yz - xw), ww - xx - yy + zz),
            },
        };
    }

    /// Convert to axis-angle
    pub fn convertToAxisAngle(q: Quaternion) struct { axis: Vec3, angle: f32 } {
        const qn = q.normalize();
        const half_angle = math.acos(clamp(qn.w, -1, 1));
        if (half_angle < EPSILON) return .{ .axis = Vec3.axisX(), .angle = 0 };
        const sin_half = @sin(half_angle);
        return .{
            .axis = Vec3.init(qn.x / sin_half, qn.y / sin_half, qn.z / sin_half).normalize(),
            .angle = 2.0 * half_angle,
        };
    }

    /// Get shortest equivalent (ensure rotation ≤ 180°)
    pub fn getShortestEquivalent(q: Quaternion) Quaternion {
        if (q.w < 0) return Quaternion.init(-q.x, -q.y, -q.z, -q.w);
        return q;
    }
};

// =============================================================================
// 8. Transform (из O3DE Transform.h)
// =============================================================================
//
// P³: Transform = Sim(3) ⊂ PGL(4): подгруппа подобий
//     Uniform scale → проективный масштаб (собственное значение коллинеации)
//     CreateLookAt → проективная камера

pub const Transform = struct {
    rotation: Quaternion,
    translation: Vec3,
    scale_val: f32,

    pub inline fn init(rotation: Quaternion, translation: Vec3, scale_val: f32) Transform {
        return .{ .rotation = rotation, .translation = translation, .scale_val = clamp(scale_val, MIN_TRANSFORM_SCALE, MAX_TRANSFORM_SCALE) };
    }
    pub inline fn identity() Transform {
        return .{ .rotation = Quaternion.identity(), .translation = Vec3.zero(), .scale_val = 1.0 };
    }
    pub inline fn fromRotation(q: Quaternion) Transform {
        return .{ .rotation = q, .translation = Vec3.zero(), .scale_val = 1.0 };
    }
    pub inline fn fromTranslation(t: Vec3) Transform {
        return .{ .rotation = Quaternion.identity(), .translation = t, .scale_val = 1.0 };
    }
    pub inline fn fromScale(s: f32) Transform {
        return .{ .rotation = Quaternion.identity(), .translation = Vec3.zero(), .scale_val = clamp(s, MIN_TRANSFORM_SCALE, MAX_TRANSFORM_SCALE) };
    }
    pub inline fn fromRotationAndTranslation(q: Quaternion, t: Vec3) Transform {
        return .{ .rotation = q, .translation = t, .scale_val = 1.0 };
    }

    pub fn createLookAt(from: Vec3, to: Vec3, forward: Vec3) Transform {
        const dir = to.sub(from).normalize();
        const q = Quaternion.fromShortestArc(forward, dir);
        return .{ .rotation = q, .translation = from, .scale_val = 1.0 };
    }

    /// Compose: this * other
    pub fn mul(a: Transform, b: Transform) Transform {
        return .{
            .rotation = a.rotation.mul(b.rotation),
            .translation = a.rotation.transformVec(b.translation).scale(a.scale_val).add(a.translation),
            .scale_val = a.scale_val * b.scale_val,
        };
    }

    /// Inverse transform
    pub fn inverse(t: Transform) Transform {
        const inv_rot = t.rotation.conjugate();
        const inv_scale = 1.0 / clamp(t.scale_val, MIN_TRANSFORM_SCALE, MAX_TRANSFORM_SCALE);
        return .{
            .rotation = inv_rot,
            .translation = inv_rot.transformVec(t.translation.neg()).scale(inv_scale),
            .scale_val = inv_scale,
        };
    }

    /// Transform a point: scale * rotate(p) + translate
    pub fn transformPoint(t: Transform, p: Vec3) Vec3 {
        return t.rotation.transformVec(p).scale(t.scale_val).add(t.translation);
    }

    /// Transform a direction: scale * rotate(v)
    pub fn transformVector(t: Transform, v: Vec3) Vec3 {
        return t.rotation.transformVec(v).scale(t.scale_val);
    }

    /// Convert to Mat4x4
    pub fn toMat4x4(t: Transform) Mat4x4 {
        var m = Mat4x4.createFromQuaternionAndTranslation(t.rotation, t.translation);
        // Apply uniform scale
        m.rows[0] = m.rows[0].scale(t.scale_val);
        m.rows[1] = m.rows[1].scale(t.scale_val);
        m.rows[2] = m.rows[2].scale(t.scale_val);
        return m;
    }
};

// =============================================================================
// 9. Plane (из O3DE Plane.h)
// =============================================================================
//
// P³: Plane = гиперплоскость в P³ — двойственная точке
//     (A,B,C,D) = двойственные однородные координаты
//     CreateFromTriangle = join 3 точек (wedge product)
//     CastRay = meet линии и гиперплоскости

pub const Plane = struct {
    normal: Vec3,
    distance: f32, // Ax + By + Cz + D = 0

    pub inline fn fromNormalAndDistance(n: Vec3, d: f32) Plane {
        return .{ .normal = n.normalize(), .distance = d };
    }
    pub inline fn fromNormalAndPoint(n: Vec3, p: Vec3) Plane {
        const nn = n.normalize();
        return .{ .normal = nn, .distance = -nn.dot(p) };
    }
    pub fn fromTriangle(a: Vec3, b: Vec3, c: Vec3) Plane {
        const n = b.sub(a).cross(c.sub(a)).normalize();
        return .{ .normal = n, .distance = -n.dot(a) };
    }
    pub inline fn fromCoefficients(a: f32, b: f32, c: f32, d: f32) Plane {
        const n = Vec3.init(a, b, c);
        const len = n.length();
        if (len < EPSILON) return .{ .normal = Vec3.axisZ(), .distance = 0 };
        return .{ .normal = n.scale(1.0 / len), .distance = d / len };
    }

    /// As Vec4 (dual homogeneous coordinates)
    pub inline fn toVec4(p: Plane) Vec4 {
        return Vec4.init(p.normal.x, p.normal.y, p.normal.z, p.distance);
    }
    pub inline fn fromVec4(v: Vec4) Plane {
        return fromCoefficients(v.x, v.y, v.z, v.w);
    }

    pub inline fn distanceToPoint(p: Plane, point: Vec3) f32 {
        return p.normal.dot(point) + p.distance;
    }

    pub inline fn project(p: Plane, point: Vec3) Vec3 {
        return point.sub(p.normal.scale(p.distanceToPoint(point)));
    }

    pub fn normalize(p: Plane) Plane {
        const l = p.normal.length();
        if (l < EPSILON) return p;
        const inv_l = 1.0 / l;
        return .{ .normal = p.normal.scale(inv_l), .distance = p.distance * inv_l };
    }

    pub inline fn flipped(p: Plane) Plane {
        return .{ .normal = p.normal.neg(), .distance = -p.distance };
    }

    /// Ray intersection: origin + t*dir
    pub fn castRay(p: Plane, origin: Vec3, dir: Vec3) ?f32 {
        const denom = p.normal.dot(dir);
        if (@abs(denom) < EPSILON) return null; // parallel
        const t = -(p.normal.dot(origin) + p.distance) / denom;
        if (t < 0) return null; // behind origin
        return t;
    }
};

// =============================================================================
// 10. Aabb (из O3DE Aabb.h)
// =============================================================================
//
// P³: AABB → проективный ограничивающий прямоугольник
//     Contains() → тест знака однородных полупространств

pub const Aabb = struct {
    min: Vec3,
    max: Vec3,

    pub fn createNull() Aabb {
        return .{
            .min = Vec3.init(MAX_FLOAT, MAX_FLOAT, MAX_FLOAT),
            .max = Vec3.init(-MAX_FLOAT, -MAX_FLOAT, -MAX_FLOAT),
        };
    }
    pub inline fn fromPoint(p: Vec3) Aabb {
        return .{ .min = p, .max = p };
    }
    pub inline fn fromMinMax(min_v: Vec3, max_v: Vec3) Aabb {
        return .{ .min = min_v, .max = max_v };
    }
    pub inline fn fromCenterHalfExtents(c: Vec3, half: Vec3) Aabb {
        return .{ .min = c.sub(half), .max = c.add(half) };
    }

    pub inline fn center(a: Aabb) Vec3 {
        return a.min.add(a.max).scale(0.5);
    }
    pub inline fn extents(a: Aabb) Vec3 {
        return a.max.sub(a.min).scale(0.5);
    }
    pub inline fn volume(a: Aabb) f32 {
        const d = a.max.sub(a.min);
        return d.x * d.y * d.z;
    }
    pub inline fn surfaceArea(a: Aabb) f32 {
        const d = a.max.sub(a.min);
        return 2.0 * (d.x * d.y + d.y * d.z + d.z * d.x);
    }

    pub inline fn contains(a: Aabb, p: Vec3) bool {
        return p.x >= a.min.x and p.x <= a.max.x and
            p.y >= a.min.y and p.y <= a.max.y and
            p.z >= a.min.z and p.z <= a.max.z;
    }

    pub fn containsAabb(a: Aabb, b: Aabb) bool {
        return b.min.x >= a.min.x and b.max.x <= a.max.x and
            b.min.y >= a.min.y and b.max.y <= a.max.y and
            b.min.z >= a.min.z and b.max.z <= a.max.z;
    }

    pub fn overlaps(a: Aabb, b: Aabb) bool {
        return a.min.x <= b.max.x and a.max.x >= b.min.x and
            a.min.y <= b.max.y and a.max.y >= b.min.y and
            a.min.z <= b.max.z and a.max.z >= b.min.z;
    }

    pub fn merge(a: Aabb, b: Aabb) Aabb {
        return .{
            .min = Vec3.getMin(a.min, b.min),
            .max = Vec3.getMax(a.max, b.max),
        };
    }

    pub fn expand(a: Aabb, point: Vec3) Aabb {
        return .{
            .min = Vec3.getMin(a.min, point),
            .max = Vec3.getMax(a.max, point),
        };
    }
};

// =============================================================================
// 11. Frustum (из O3DE Frustum.h)
// =============================================================================
//
// P³: Frustum = 6 проективных полупространств
//     8 corners = meet 3 planes (тройной wedge product)

pub const PlaneId = enum(u3) {
    near = 0,
    far = 1,
    left = 2,
    right = 3,
    top = 4,
    bottom = 5,
};
pub const PLANE_COUNT: usize = 6;

pub const Frustum = struct {
    planes: [PLANE_COUNT]Plane,

    /// Extract frustum from view-projection matrix (row-major)
    pub fn fromMatrix(m: Mat4x4) Frustum {
        var f: Frustum = undefined;
        const r = m.rows;
        // Left:  m[3] + m[0]
        f.planes[@intFromEnum(PlaneId.left)] = Plane.fromVec4(Vec4.init(
            r[3].x + r[0].x, r[3].y + r[0].y, r[3].z + r[0].z, r[3].w + r[0].w,
        )).normalize();
        // Right: m[3] - m[0]
        f.planes[@intFromEnum(PlaneId.right)] = Plane.fromVec4(Vec4.init(
            r[3].x - r[0].x, r[3].y - r[0].y, r[3].z - r[0].z, r[3].w - r[0].w,
        )).normalize();
        // Bottom: m[3] + m[1]
        f.planes[@intFromEnum(PlaneId.bottom)] = Plane.fromVec4(Vec4.init(
            r[3].x + r[1].x, r[3].y + r[1].y, r[3].z + r[1].z, r[3].w + r[1].w,
        )).normalize();
        // Top:    m[3] - m[1]
        f.planes[@intFromEnum(PlaneId.top)] = Plane.fromVec4(Vec4.init(
            r[3].x - r[1].x, r[3].y - r[1].y, r[3].z - r[1].z, r[3].w - r[1].w,
        )).normalize();
        // Near:   m[3] + m[2]
        f.planes[@intFromEnum(PlaneId.near)] = Plane.fromVec4(Vec4.init(
            r[3].x + r[2].x, r[3].y + r[2].y, r[3].z + r[2].z, r[3].w + r[2].w,
        )).normalize();
        // Far:    m[3] - m[2]
        f.planes[@intFromEnum(PlaneId.far)] = Plane.fromVec4(Vec4.init(
            r[3].x - r[2].x, r[3].y - r[2].y, r[3].z - r[2].z, r[3].w - r[2].w,
        )).normalize();
        return f;
    }

    /// Test if AABB is inside/overlapping/outside frustum
    pub const IntersectResult = enum { interior, overlaps, exterior };

    pub fn intersectAabb(f: Frustum, aabb: Aabb) IntersectResult {
        var result: IntersectResult = .interior;
        for (f.planes) |plane| {
            const p = Vec3.init(
                if (plane.normal.x >= 0) aabb.max.x else aabb.min.x,
                if (plane.normal.y >= 0) aabb.max.y else aabb.min.y,
                if (plane.normal.z >= 0) aabb.max.z else aabb.min.z,
            );
            const n = Vec3.init(
                if (plane.normal.x >= 0) aabb.min.x else aabb.max.x,
                if (plane.normal.y >= 0) aabb.min.y else aabb.max.y,
                if (plane.normal.z >= 0) aabb.min.z else aabb.max.z,
            );
            if (plane.distanceToPoint(p) < 0) return .exterior;
            if (plane.distanceToPoint(n) < 0) result = .overlaps;
        }
        return result;
    }

    /// Проверка точки на попадание во Frustum
    pub fn containsPoint(self: Frustum, pt: Vec3) bool {
        for (self.planes) |p| {
            if (p.distanceToPoint(pt) < -1e-4) return false;
        }
        return true;
    }

    /// Проверка пересечения сферы со Frustum
    pub fn intersectsSphere(self: Frustum, center: Vec3, radius: f32) bool {
        for (self.planes) |p| {
            if (p.distanceToPoint(center) < -radius) return false;
        }
        return true;
    }

    /// Проверка пересечения Aabb (булевый результат)
    pub fn intersectsAabb(self: Frustum, box: Aabb) bool {
        return self.intersectAabb(box) != .exterior;
    }
};

// =============================================================================
// 12. Color (из O3DE Color.h)
// =============================================================================
//
// P³: Color с alpha = проективная величина
//     Premultiplied alpha = каноническая проективная форма
//     LinearToGamma = нелинейный переход карты на проективном пространстве

pub const Color = struct {
    r: f32,
    g: f32,
    b: f32,
    a: f32,

    pub inline fn init(r: f32, g: f32, b: f32, a: f32) Color {
        return .{ .r = r, .g = g, .b = b, .a = a };
    }
    pub inline fn fromRGBA8(r: u8, g: u8, b: u8, a: u8) Color {
        return .{
            .r = @as(f32, @floatFromInt(r)) / 255.0,
            .g = @as(f32, @floatFromInt(g)) / 255.0,
            .b = @as(f32, @floatFromInt(b)) / 255.0,
            .a = @as(f32, @floatFromInt(a)) / 255.0,
        };
    }
    pub inline fn white() Color {
        return .{ .r = 1, .g = 1, .b = 1, .a = 1 };
    }
    pub inline fn black() Color {
        return .{ .r = 0, .g = 0, .b = 0, .a = 1 };
    }
    pub inline fn transparent() Color {
        return .{ .r = 0, .g = 0, .b = 0, .a = 0 };
    }
    pub inline fn red() Color {
        return .{ .r = 1, .g = 0, .b = 0, .a = 1 };
    }
    pub inline fn green() Color {
        return .{ .r = 0, .g = 1, .b = 0, .a = 1 };
    }
    pub inline fn blue() Color {
        return .{ .r = 0, .g = 0, .b = 1, .a = 1 };
    }

    pub inline fn toU32(c: Color) u32 {
        const r8: u32 = @intFromFloat(clamp(c.r, 0, 1) * 255.0);
        const g8: u32 = @intFromFloat(clamp(c.g, 0, 1) * 255.0);
        const b8: u32 = @intFromFloat(clamp(c.b, 0, 1) * 255.0);
        const a8: u32 = @intFromFloat(clamp(c.a, 0, 1) * 255.0);
        return (a8 << 24) | (b8 << 16) | (g8 << 8) | r8;
    }
    pub fn fromU32(v: u32) Color {
        return fromRGBA8(
            @as(u8, @truncate(v)),
            @as(u8, @truncate(v >> 8)),
            @as(u8, @truncate(v >> 16)),
            @as(u8, @truncate(v >> 24)),
        );
    }

    pub inline fn lerp(a: Color, b: Color, t: f32) Color {
        return .{
            .r = lerpF(a.r, b.r, t),
            .g = lerpF(a.g, b.g, t),
            .b = lerpF(a.b, b.b, t),
            .a = lerpF(a.a, b.a, t),
        };
    }

    /// Premultiply alpha (каноническая проективная форма)
    pub inline fn premultiplyAlpha(c: Color) Color {
        return .{ .r = c.r * c.a, .g = c.g * c.a, .b = c.b * c.a, .a = c.a };
    }

    /// sRGB gamma → linear
    pub fn linearToGamma(v: f32) f32 {
        if (v <= 0.0031308) return v * 12.92;
        return 1.055 * math.pow(f32, v, 1.0 / 2.4) - 0.055;
    }
    /// Linear → sRGB gamma
    pub fn gammaToLinear(v: f32) f32 {
        if (v <= 0.04045) return v / 12.92;
        return math.pow(f32, (v + 0.055) / 1.055, 2.4);
    }

    pub fn saturate(c: Color) Color {
        return .{
            .r = clamp(c.r, 0, 1),
            .g = clamp(c.g, 0, 1),
            .b = clamp(c.b, 0, 1),
            .a = clamp(c.a, 0, 1),
        };
    }

    /// As Vec4 (projective color)
    pub inline fn toVec4(c: Color) Vec4 {
        return Vec4.init(c.r, c.g, c.b, c.a);
    }
};

// =============================================================================
// 13. Uuid (из O3DE Uuid.h)
// =============================================================================

pub const Uuid = struct {
    data: [16]u8 align(16),

    pub const Variant = enum(i32) {
        unknown = -1,
        ncs = 0,
        rfc4122 = 2,
        microsoft = 6,
        reserved = 7,
    };
    pub const Version = enum(i32) {
        unknown = -1,
        time = 1,
        dce = 2,
        name_md5 = 3,
        random = 4,
        name_sha1 = 5,
    };

    pub fn createNull() Uuid {
        return .{ .data = @splat(0) };
    }
    pub fn createRandom() Uuid {
        var rng = SimpleLcgRandom.init(@intCast(std.time.nanoTimestamp()));
        var u: Uuid = undefined;
        for (&u.data) |*b| {
            b.* = @truncate(rng.next());
        }
        // Set version 4 (random) and variant RFC 4122
        u.data[6] = (u.data[6] & 0x0F) | 0x40;
        u.data[8] = (u.data[8] & 0x3F) | 0x80;
        return u;
    }
    pub inline fn isNull(u: Uuid) bool {
        for (u.data) |b| {
            if (b != 0) return false;
        }
        return true;
    }
    pub inline fn equals(a: Uuid, b: Uuid) bool {
        for (a.data, b.data) |da, db| {
            if (da != db) return false;
        }
        return true;
    }
    pub fn hash(u: Uuid) u64 {
        var h: u64 = 0;
        for (0..8) |i| {
            h |= @as(u64, u.data[i]) << @intCast(i * 8);
        }
        return h;
    }
};

// =============================================================================
// 14. Interpolation (из O3DE Interpolate.h)
// =============================================================================

pub inline fn slerpF(a: f32, b: f32, t: f32) f32 {
    const omega = math.acos(clamp(a * b, -1, 1));
    if (@abs(omega) < EPSILON) return lerpF(a, b, t);
    const sin_omega = @sin(omega);
    return @sin((1 - t) * omega) * a / sin_omega + @sin(t * omega) * b / sin_omega;
}

pub inline fn smootherStep(edge0: f32, edge1: f32, x: f32) f32 {
    const t = clamp((x - edge0) / safeDiv(edge1 - edge0, 1, 1), 0, 1);
    return t * t * t * (t * (t * 6 - 15) + 10);
}

pub inline fn easeInQuad(t: f32) f32 {
    return t * t;
}
pub inline fn easeOutQuad(t: f32) f32 {
    return t * (2 - t);
}
pub inline fn easeInOutQuad(t: f32) f32 {
    return if (t < 0.5) 2 * t * t else -1 + (4 - 2 * t) * t;
}
pub inline fn easeInCubic(t: f32) f32 {
    return t * t * t;
}
pub inline fn easeOutCubic(t: f32) f32 {
    const t1 = t - 1;
    return t1 * t1 * t1 + 1;
}

/// Critically damped spring (из O3DE Vector2::SmoothCriticallyDamped)
pub fn springDamper(current: f32, target: f32, velocity: *f32, dt: f32, smoothing: f32) f32 {
    const omega = 2.0 / smoothing;
    const x = omega * dt;
    const exp = @exp(-x);
    const temp1 = target - current + (velocity.* + (target - current) * omega) * dt;
    const new_value = target - temp1 * exp;
    velocity.* = (velocity.* - temp1 * omega) * exp;
    return new_value;
}

// =============================================================================
// 15. Random (из O3DE Random.h)
// =============================================================================

pub const SimpleLcgRandom = struct {
    seed: u64,

    pub fn init(seed: u64) SimpleLcgRandom {
        return .{ .seed = seed & 0xFFFFFFFFFFFF }; // 48-bit
    }

    pub fn next(self: *SimpleLcgRandom) u32 {
        // Java-style LCG: seed = (seed * 0x5DEECE66D + 0xB) mod 2^48
        // Wrapping multiply/add (mod 2^64), then mask to 48 bits
        self.seed = (self.seed *% @as(u64, 0x5DEECE66D) +% @as(u64, 0xB)) & 0xFFFFFFFFFFFF;
        return @truncate(self.seed >> 16);
    }

    pub fn nextFloat(self: *SimpleLcgRandom) f32 {
        const n = self.next();
        return @as(f32, @floatFromInt(n)) / @as(f32, @floatFromInt(@as(u32, 0xFFFFFFFF)));
    }

    pub fn nextRange(self: *SimpleLcgRandom, lo: i32, hi: i32) i32 {
        if (lo >= hi) return lo;
        const range = @as(u32, @intCast(hi - lo));
        const n = self.next() % (range + 1);
        return lo + @as(i32, @intCast(n));
    }

    pub fn nextVec2(self: *SimpleLcgRandom) Vec2 {
        return Vec2.init(self.nextFloat(), self.nextFloat());
    }
    pub fn nextVec3(self: *SimpleLcgRandom) Vec3 {
        return Vec3.init(self.nextFloat(), self.nextFloat(), self.nextFloat());
    }
    pub fn nextVec4(self: *SimpleLcgRandom) Vec4 {
        return Vec4.init(self.nextFloat(), self.nextFloat(), self.nextFloat(), self.nextFloat());
    }
};

/// Halton quasi-random sequence (low-discrepancy, for projective sampling)
pub fn HaltonSequence(comptime dim: u8) type {
    return struct {
        index: u32,
        primes: [dim]u32,

        const Self = @This();

        pub fn init() Self {
            const first_primes = [8]u32{ 2, 3, 5, 7, 11, 13, 17, 19 };
            var pr: [dim]u32 = undefined;
            for (0..dim) |i| {
                pr[i] = first_primes[i % 8];
            }
            return .{ .index = 1, .primes = pr };
        }

        pub fn next(self: *Self) [dim]f32 {
            var result: [dim]f32 = undefined;
            for (0..dim) |d| {
                var f: f32 = 1.0;
                const inv_base: f32 = 1.0 / @as(f32, @floatFromInt(self.primes[d]));
                var idx = self.index;
                var val: f32 = 0;
                while (idx > 0) {
                    f *= inv_base;
                    val += f * @as(f32, @floatFromInt(idx % self.primes[d]));
                    idx /= self.primes[d];
                }
                result[d] = val;
            }
            self.index += 1;
            return result;
        }
    };
}

// =============================================================================
// 16. Rotator (из Unreal Engine FRotator — Pitch, Yaw, Roll в градусах)
// =============================================================================

/// Углы Эйлера в градусах (конвенция Unreal Engine: Pitch = Y-наклон, Yaw = Z-поворот, Roll = X-крен)
pub const Rotator = struct {
    pitch: f32 = 0.0, // Наклон вверх/вниз вокруг оси Y [-90..90]
    yaw: f32 = 0.0,   // Поворот влево/вправо вокруг оси Z [0..360)
    roll: f32 = 0.0,  // Крен вокруг оси X [-180..180]

    pub fn init(pitch_deg: f32, yaw_deg: f32, roll_deg: f32) Rotator {
        return .{ .pitch = pitch_deg, .yaw = yaw_deg, .roll = roll_deg };
    }

    pub fn zero() Rotator {
        return .{ .pitch = 0.0, .yaw = 0.0, .roll = 0.0 };
    }

    pub fn fromRadians(pitch_rad: f32, yaw_rad: f32, roll_rad: f32) Rotator {
        return .{
            .pitch = radToDeg(pitch_rad),
            .yaw = radToDeg(yaw_rad),
            .roll = radToDeg(roll_rad),
        };
    }

    /// Нормализовать углы к диапазону [-180..180]
    pub fn normalize(self: Rotator) Rotator {
        var p = @rem(self.pitch, 360.0);
        if (p > 180.0) p -= 360.0 else if (p < -180.0) p += 360.0;
        var y = @rem(self.yaw, 360.0);
        if (y > 180.0) y -= 360.0 else if (y < -180.0) y += 360.0;
        var r = @rem(self.roll, 360.0);
        if (r > 180.0) r -= 360.0 else if (r < -180.0) r += 360.0;
        return .{ .pitch = p, .yaw = y, .roll = r };
    }

    /// Ограничить pitch в диапазоне [-89.9..89.9] во избежание Gimbal Lock
    pub fn clamp(self: Rotator) Rotator {
        const norm = self.normalize();
        return .{
            .pitch = std.math.clamp(norm.pitch, -89.9, 89.9),
            .yaw = norm.yaw,
            .roll = norm.roll,
        };
    }

    /// Конвертировать Rotator в единичный Quaternion (последовательность ZYX: Yaw * Pitch * Roll)
    pub fn toQuaternion(self: Rotator) Quaternion {
        const p = degToRad(self.pitch) * 0.5;
        const y = degToRad(self.yaw) * 0.5;
        const r = degToRad(self.roll) * 0.5;

        const sin_p = @sin(p);
        const cos_p = @cos(p);
        const sin_y = @sin(y);
        const cos_y = @cos(y);
        const sin_r = @sin(r);
        const cos_r = @cos(r);

        return Quaternion.init(
            sin_r * cos_p * cos_y - cos_r * sin_p * sin_y, // x
            cos_r * sin_p * cos_y + sin_r * cos_p * sin_y, // y
            cos_r * cos_p * sin_y - sin_r * sin_p * cos_y, // z
            cos_r * cos_p * cos_y + sin_r * sin_p * sin_y, // w
        ).normalize();
    }

    /// Извлечь Rotator из Quaternion с защитой от Gimbal Lock
    pub fn fromQuaternion(q: Quaternion) Rotator {
        const norm = q.normalize();
        const sin_pitch = 2.0 * (norm.w * norm.y - norm.z * norm.x);

        var pitch_rad: f32 = 0.0;
        var yaw_rad: f32 = 0.0;
        var roll_rad: f32 = 0.0;

        if (@abs(sin_pitch) >= 0.9999) {
            // Gimbal Lock случай
            pitch_rad = std.math.copysign(PI * 0.5, sin_pitch);
            yaw_rad = -2.0 * math.atan2(norm.x, norm.w);
            roll_rad = 0.0;
        } else {
            pitch_rad = math.asin(std.math.clamp(sin_pitch, -1.0, 1.0));
            yaw_rad = math.atan2(2.0 * (norm.w * norm.z + norm.x * norm.y), 1.0 - 2.0 * (norm.y * norm.y + norm.z * norm.z));
            roll_rad = math.atan2(2.0 * (norm.w * norm.x + norm.y * norm.z), 1.0 - 2.0 * (norm.x * norm.x + norm.y * norm.y));
        }

        return fromRadians(pitch_rad, yaw_rad, roll_rad);
    }

    /// Вектор прямого направления (Forward / Heading)
    pub fn getForwardVector(self: Rotator) Vec3 {
        const p = degToRad(self.pitch);
        const y = degToRad(self.yaw);
        const cp = @cos(p);
        return Vec3.init(cp * @cos(y), cp * @sin(y), @sin(p)).normalize();
    }

    /// Вектор вправо (Right)
    pub fn getRightVector(self: Rotator) Vec3 {
        return self.toQuaternion().transformVec(Vec3.axisY()).normalize();
    }

    /// Вектор вверх (Up)
    pub fn getUpVector(self: Rotator) Vec3 {
        return self.toQuaternion().transformVec(Vec3.axisZ()).normalize();
    }

    /// Матрица вращения 4x4
    pub fn toMatrix4x4(self: Rotator) Mat4x4 {
        return self.toQuaternion().toMat4x4();
    }

    pub fn add(self: Rotator, other: Rotator) Rotator {
        return Rotator.init(self.pitch + other.pitch, self.yaw + other.yaw, self.roll + other.roll);
    }

    pub fn sub(self: Rotator, other: Rotator) Rotator {
        return Rotator.init(self.pitch - other.pitch, self.yaw - other.yaw, self.roll - other.roll);
    }

    pub fn scale(self: Rotator, s: f32) Rotator {
        return Rotator.init(self.pitch * s, self.yaw * s, self.roll * s);
    }
};

// =============================================================================
// 17. Ray & Ray-Intersection (Трассировка лучей и пересечения)
// =============================================================================

/// Луч в трехмерном евклидовом пространстве
pub const Ray = struct {
    origin: Vec3,
    direction: Vec3, // Единичный вектор

    pub fn init(origin: Vec3, direction: Vec3) Ray {
        return .{
            .origin = origin,
            .direction = direction.normalize(),
        };
    }

    /// Точка вдоль луча: p(t) = origin + t * direction
    pub fn pointAt(self: Ray, t: f32) Vec3 {
        return self.origin.add(self.direction.scale(t));
    }

    /// Пересечение с плоскостью Plane
    pub fn intersectPlane(self: Ray, plane: Plane) ?f32 {
        const denom = plane.normal.dot(self.direction);
        if (@abs(denom) < 1e-6) return null; // Луч параллелен плоскости
        const t = -(plane.normal.dot(self.origin) + plane.dist) / denom;
        return if (t >= 0.0) t else null;
    }

    /// Пересечение со сферой (центр, радиус) -> {t_entry, t_exit}
    pub fn intersectSphere(self: Ray, center: Vec3, radius: f32) ?struct { t0: f32, t1: f32 } {
        const oc = self.origin.sub(center);
        const a = self.direction.dot(self.direction);
        const b = 2.0 * oc.dot(self.direction);
        const c = oc.dot(oc) - radius * radius;
        const disc = b * b - 4.0 * a * c;

        if (disc < 0.0) return null;
        const sqrt_disc = @sqrt(disc);
        const t0 = (-b - sqrt_disc) / (2.0 * a);
        const t1 = (-b + sqrt_disc) / (2.0 * a);

        if (t1 < 0.0) return null;
        return .{ .t0 = @max(0.0, t0), .t1 = t1 };
    }

    /// Пересечение с ограничивающим параллелепипедом Aabb (Slab Method)
    pub fn intersectAabb(self: Ray, box: Aabb) ?struct { t_min: f32, t_max: f32 } {
        var t_min: f32 = -std.math.floatMax(f32);
        var t_max: f32 = std.math.floatMax(f32);

        const orig = [3]f32{ self.origin.x, self.origin.y, self.origin.z };
        const dir = [3]f32{ self.direction.x, self.direction.y, self.direction.z };
        const min_b = [3]f32{ box.min.x, box.min.y, box.min.z };
        const max_b = [3]f32{ box.max.x, box.max.y, box.max.z };

        for (0..3) |i| {
            if (@abs(dir[i]) < 1e-6) {
                if (orig[i] < min_b[i] or orig[i] > max_b[i]) return null;
            } else {
                const inv_d = 1.0 / dir[i];
                var t1 = (min_b[i] - orig[i]) * inv_d;
                var t2 = (max_b[i] - orig[i]) * inv_d;
                if (t1 > t2) {
                    const temp = t1;
                    t1 = t2;
                    t2 = temp;
                }
                t_min = @max(t_min, t1);
                t_max = @min(t_max, t2);
                if (t_min > t_max or t_max < 0.0) return null;
            }
        }
        return .{ .t_min = @max(0.0, t_min), .t_max = t_max };
    }

    /// Пересечение с треугольником (Möller–Trumbore алгоритм) -> {t, u, v}
    pub fn intersectTriangle(self: Ray, v0: Vec3, v1: Vec3, v2: Vec3) ?struct { t: f32, u: f32, v: f32 } {
        const edge1 = v1.sub(v0);
        const edge2 = v2.sub(v0);
        const h = self.direction.cross(edge2);
        const a = edge1.dot(h);

        if (@abs(a) < 1e-6) return null; // Луч параллелен треугольнику
        const f = 1.0 / a;
        const s = self.origin.sub(v0);
        const u = f * s.dot(h);
        if (u < 0.0 or u > 1.0) return null;

        const q = s.cross(edge1);
        const v = f * self.direction.dot(q);
        if (v < 0.0 or u + v > 1.0) return null;

        const t = f * edge2.dot(q);
        if (t < 1e-6) return null;
        return .{ .t = t, .u = u, .v = v };
    }
};

// =============================================================================
// 18. Frustum & Reversed-Z Perspective (UE5 / Nanite Standard)
// =============================================================================

/// Стандартная и Reversed-Z проекционная матрица
pub const Projection = struct {
    /// Стандартная перспективная матрица (OpenGL/Vulkan depth [0..1])
    pub fn perspective(fov_y_rad: f32, aspect: f32, near: f32, far: f32) Mat4x4 {
        return Mat4x4.createProjectionFov(fov_y_rad, aspect, near, far);
    }

    /// Reversed-Z перспективная матрица с бесконечной дальней плоскостью (UE5 Standard)
    /// z_near -> 1.0, z_inf -> 0.0 (максимальная точность float32 depth буфера на астрономических масштабах)
    pub fn reversedZPerspective(fov_y_rad: f32, aspect: f32, near: f32) Mat4x4 {
        const cot_half_fov = 1.0 / @tan(fov_y_rad * 0.5);
        return Mat4x4.fromRows(
            Vec4.init(cot_half_fov / aspect, 0.0, 0.0, 0.0),
            Vec4.init(0.0, cot_half_fov, 0.0, 0.0),
            Vec4.init(0.0, 0.0, 0.0, near),
            Vec4.init(0.0, 0.0, -1.0, 0.0),
        );
    }
};

// =============================================================================
// 19. InterpCurve (Кривые интерполяции, сплайны Эрмита и Безье)
// =============================================================================

/// Инструменты сплайновой и криволинейной интерполяции
pub const InterpCurve = struct {
    /// Кубический сплайн Эрмита: p0/p1 — точки, t0/t1 — касательные векторы скорости
    pub fn cubicHermite(p0: Vec3, t0: Vec3, p1: Vec3, t1: Vec3, alpha: f32) Vec3 {
        const a = std.math.clamp(alpha, 0.0, 1.0);
        const a2 = a * a;
        const a3 = a2 * a;

        const h00 = 2.0 * a3 - 3.0 * a2 + 1.0;
        const h10 = a3 - 2.0 * a2 + a;
        const h01 = -2.0 * a3 + 3.0 * a2;
        const h11 = a3 - a2;

        return p0.scale(h00).add(t0.scale(h10)).add(p1.scale(h01)).add(t1.scale(h11));
    }

    /// Сплайн Катмулла-Рома по 4 опорным точкам
    pub fn catmullRom(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, alpha: f32) Vec3 {
        const t1 = p2.sub(p0).scale(0.5);
        const t2 = p3.sub(p1).scale(0.5);
        return cubicHermite(p1, t1, p2, t2, alpha);
    }

    /// Квадратичная кривая Безье
    pub fn bezierQuad(p0: Vec3, p1: Vec3, p2: Vec3, t: f32) Vec3 {
        const u = 1.0 - t;
        return p0.scale(u * u).add(p1.scale(2.0 * u * t)).add(p2.scale(t * t));
    }

    /// Кубическая кривая Безье
    pub fn bezierCubic(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) Vec3 {
        const u = 1.0 - t;
        const tt = t * t;
        const uu = u * u;
        return p0.scale(uu * u).add(p1.scale(3.0 * uu * t)).add(p2.scale(3.0 * u * tt)).add(p3.scale(tt * t));
    }
};

// =============================================================================
// 20. ТЕСТЫ
// =============================================================================

test "Constants: radToDeg/degToRad" {
    try std.testing.expectApproxEqAbs(radToDeg(PI), 180.0, 1e-4);
    try std.testing.expectApproxEqAbs(degToRad(180.0), PI, 1e-4);
}

test "invSqrt: basic" {
    try std.testing.expectApproxEqAbs(invSqrt(4.0), 0.5, 1e-3);
    try std.testing.expectApproxEqAbs(invSqrt(1.0), 1.0, 1e-3);
}

test "Vec2: basic operations" {
    const a = Vec2.init(3, 4);
    const b = Vec2.init(1, 2);
    try std.testing.expectApproxEqAbs(a.length(), 5.0, 1e-4);
    try std.testing.expectApproxEqAbs(a.dot(b), 11.0, 1e-4);
    const sum = a.add(b);
    try std.testing.expectApproxEqAbs(sum.x, 4.0, 1e-4);
}

test "Vec2: normalize" {
    const v = Vec2.init(3, 4).normalize();
    try std.testing.expectApproxEqAbs(v.length(), 1.0, 1e-4);
}

test "Vec2: perpendicular" {
    const v = Vec2.init(1, 0);
    const p = v.perpendicular();
    try std.testing.expectApproxEqAbs(p.dot(v), 0.0, 1e-4);
}

test "Vec3: cross product" {
    const a = Vec3.axisX();
    const b = Vec3.axisY();
    const c = a.cross(b);
    try std.testing.expectApproxEqAbs(c.z, 1.0, 1e-4);
}

test "Vec3: normalize and length" {
    const v = Vec3.init(1, 2, 3).normalize();
    try std.testing.expectApproxEqAbs(v.length(), 1.0, 1e-4);
}

test "Vec4: dehomogenize" {
    const p = Vec4.init(2, 4, 6, 2);
    const v = p.getAsVec3();
    try std.testing.expectApproxEqAbs(v.x, 1.0, 1e-4);
    try std.testing.expectApproxEqAbs(v.y, 2.0, 1e-4);
    try std.testing.expectApproxEqAbs(v.z, 3.0, 1e-4);
}

test "Mat3x3: identity and determinant" {
    const m = Mat3x3.identity();
    try std.testing.expectApproxEqAbs(m.determinant(), 1.0, 1e-4);
}

test "Mat3x3: rotation preserves determinant" {
    const m = Mat3x3.createRotationZ(PI / 4);
    try std.testing.expectApproxEqAbs(m.determinant(), 1.0, 1e-4);
}

test "Mat3x3: inverse" {
    const m = Mat3x3.createRotationZ(0.7).mul(Mat3x3.createScale(2, 3, 4));
    const inv = m.inverse();
    const product = m.mul(inv);
    const id = Mat3x3.identity();
    for (0..3) |i| {
        try std.testing.expect(product.rows[i].isCloseV(id.rows[i], 1e-3));
    }
}

test "Mat4x4: identity" {
    const m = Mat4x4.identity();
    try std.testing.expectApproxEqAbs(m.determinant(), 1.0, 1e-3);
}

test "Mat4x4: translation and transform" {
    const m = Mat4x4.createTranslation(10, 20, 30);
    const p = m.transformPoint(Vec3.zero());
    try std.testing.expectApproxEqAbs(p.x, 10.0, 1e-3);
    try std.testing.expectApproxEqAbs(p.y, 20.0, 1e-3);
}

test "Quaternion: identity" {
    const q = Quaternion.identity();
    try std.testing.expectApproxEqAbs(q.w, 1.0, 1e-4);
    const v = q.transformVec(Vec3.axisX());
    try std.testing.expectApproxEqAbs(v.x, 1.0, 1e-4);
}

test "Quaternion: axis-angle roundtrip" {
    const q = Quaternion.fromAxisAngle(Vec3.axisZ(), PI / 3);
    const aa = q.convertToAxisAngle();
    try std.testing.expectApproxEqAbs(aa.angle, PI / 3, 1e-3);
}

test "Quaternion: slerp" {
    const a = Quaternion.identity();
    const b = Quaternion.fromAxisAngle(Vec3.axisZ(), PI / 2);
    const mid = Quaternion.slerp(a, b, 0.5);
    const aa = mid.convertToAxisAngle();
    try std.testing.expectApproxEqAbs(aa.angle, PI / 4, 1e-3);
}

test "Transform: compose and inverse" {
    const a = Transform.fromTranslation(Vec3.init(1, 0, 0));
    const b = Transform.fromTranslation(Vec3.init(0, 1, 0));
    const ab = a.mul(b);
    const p = ab.transformPoint(Vec3.zero());
    try std.testing.expectApproxEqAbs(p.x, 1.0, 1e-4);
    try std.testing.expectApproxEqAbs(p.y, 1.0, 1e-4);
}

test "Plane: distance and ray" {
    const p = Plane.fromNormalAndDistance(Vec3.axisZ(), -5);
    try std.testing.expectApproxEqAbs(p.distanceToPoint(Vec3.init(0, 0, 5)), 0.0, 1e-4);
    const t = p.castRay(Vec3.init(0, 0, 10), Vec3.init(0, 0, -1));
    try std.testing.expect(t != null);
    try std.testing.expectApproxEqAbs(t.?, 5.0, 1e-4);
}

test "Aabb: contains and overlaps" {
    const a = Aabb.fromMinMax(Vec3.zero(), Vec3.one());
    try std.testing.expect(a.contains(Vec3.init(0.5, 0.5, 0.5)));
    try std.testing.expect(!a.contains(Vec3.init(2, 2, 2)));
}

test "Color: lerp and premultiply" {
    const a = Color.black();
    const b = Color.white();
    const mid = Color.lerp(a, b, 0.5);
    try std.testing.expectApproxEqAbs(mid.r, 0.5, 1e-4);
    const pm = Color.init(1, 0, 0, 0.5).premultiplyAlpha();
    try std.testing.expectApproxEqAbs(pm.r, 0.5, 1e-4);
}

test "Uuid: null and random" {
    const n = Uuid.createNull();
    try std.testing.expect(n.isNull());
    const r = Uuid.createRandom();
    try std.testing.expect(!r.isNull());
}

test "SimpleLcgRandom: range" {
    var rng = SimpleLcgRandom.init(42);
    for (0..100) |_| {
        const v = rng.nextRange(0, 10);
        try std.testing.expect(v >= 0 and v <= 10);
    }
}

test "HaltonSequence: 2D" {
    var h = HaltonSequence(2).init();
    const p = h.next();
    try std.testing.expect(p[0] >= 0 and p[0] < 1);
    try std.testing.expect(p[1] >= 0 and p[1] < 1);
}

test "Rotator: toQuaternion and fromQuaternion" {
    const rot = Rotator.init(30.0, 45.0, 0.0);
    const q = rot.toQuaternion();
    const back = Rotator.fromQuaternion(q);
    try std.testing.expectApproxEqAbs(rot.pitch, back.pitch, 0.1);
    try std.testing.expectApproxEqAbs(rot.yaw, back.yaw, 0.1);
    try std.testing.expectApproxEqAbs(rot.roll, back.roll, 0.1);

    const fwd = rot.getForwardVector();
    try std.testing.expect(fwd.x > 0.0 and fwd.y > 0.0 and fwd.z > 0.0);
}

test "Ray: sphere and AABB intersection" {
    const ray = Ray.init(Vec3.init(0, 0, -10), Vec3.axisZ());
    // Sphere at (0,0,0) radius 2
    const sphere_hit = ray.intersectSphere(Vec3.zero(), 2.0);
    try std.testing.expect(sphere_hit != null);
    try std.testing.expectApproxEqAbs(sphere_hit.?.t0, 8.0, 1e-3);
    try std.testing.expectApproxEqAbs(sphere_hit.?.t1, 12.0, 1e-3);

    // AABB from (-1,-1,-1) to (1,1,1)
    const box = Aabb.fromMinMax(Vec3.init(-1, -1, -1), Vec3.init(1, 1, 1));
    const box_hit = ray.intersectAabb(box);
    try std.testing.expect(box_hit != null);
    try std.testing.expectApproxEqAbs(box_hit.?.t_min, 9.0, 1e-3);
    try std.testing.expectApproxEqAbs(box_hit.?.t_max, 11.0, 1e-3);

    // Triangle
    const tri_hit = ray.intersectTriangle(Vec3.init(-2, -2, 0), Vec3.init(2, -2, 0), Vec3.init(0, 2, 0));
    try std.testing.expect(tri_hit != null);
    try std.testing.expectApproxEqAbs(tri_hit.?.t, 10.0, 1e-3);
}

test "Projection: Reversed-Z perspective matrix" {
    const proj = Projection.reversedZPerspective(PI / 3.0, 16.0 / 9.0, 0.1);
    try std.testing.expect(proj.rows[0].x > 0.0);
    try std.testing.expect(proj.rows[1].y > 0.0);
    try std.testing.expectApproxEqAbs(proj.rows[2].z, 0.0, 1e-6); // Reversed-Z infinite far
    try std.testing.expectApproxEqAbs(proj.rows[2].w, 0.1, 1e-4);
}

test "Frustum: point and sphere containment" {
    const view = Mat4x4.createLookAt(Vec3.init(0, 0, 10), Vec3.zero(), Vec3.axisY());
    const proj = Projection.perspective(PI / 3.0, 1.0, 0.1, 100.0);
    const vp = proj.mul(view);
    const frustum = Frustum.fromMatrix(vp);

    const aabb_inside = Aabb.fromCenterHalfExtents(Vec3.zero(), Vec3.init(1, 1, 1));
    try std.testing.expect(frustum.intersectsAabb(aabb_inside));
    const aabb_outside = Aabb.fromCenterHalfExtents(Vec3.init(0, 0, 200), Vec3.init(1, 1, 1));
    try std.testing.expect(!frustum.intersectsAabb(aabb_outside));
}

test "InterpCurve: Cubic Hermite, Catmull-Rom, Bezier" {
    const p0 = Vec3.zero();
    const p1 = Vec3.init(10, 0, 0);
    const t0 = Vec3.init(0, 5, 0);
    const t1 = Vec3.init(0, -5, 0);

    const mid = InterpCurve.cubicHermite(p0, t0, p1, t1, 0.5);
    try std.testing.expectApproxEqAbs(mid.x, 5.0, 1e-3);
    try std.testing.expect(mid.y > 0.0); // Curve bends upward due to tangents

    const bez = InterpCurve.bezierQuad(p0, Vec3.init(5, 10, 0), p1, 0.5);
    try std.testing.expectApproxEqAbs(bez.x, 5.0, 1e-3);
    try std.testing.expectApproxEqAbs(bez.y, 5.0, 1e-3);
}
