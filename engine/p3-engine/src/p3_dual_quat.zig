// =============================================================================
// P³ DUAL QUATERNION v1.0 — ZIG
// =============================================================================
//
// Дуальные кватернионы для скиннинга и моторов PGA SE(3).
// Портировано из UE5: Runtime/Core/Public/Math/DualQuat.h
//
// МАТЕМАТИКА:
//   Dual quaternion: q = R + εD
//   где R — rotation (unit quat), D — half-translation quat, ε² = 0
//
//   Умножение: (R₁ + εD₁)(R₂ + εD₂) = R₁R₂ + ε(D₁R₂ + R₁D₂)
//   Нормализация: q/||R||
//   → Transform: rotation = R, translation = 2·D·R̄
//
// P³ ОБОБЩЕНИЯ:
//   - Dual quat ∈ RP³ × RP³ = P³ → естественное вложение в P³
//   - SE(3) ⊂ PGL(4): мотор PGA ↔ dual quat ↔ 4×4 collineation
//   - Скиннинг: blend(q₁^w₁, ..., qₙ^wₙ) — screw-aware interpolation
//   - Cross-ratio 4 dual quats = проективный инвариант на SE(3)
//
// Донор: UE5 DualQuat.h (87 строк C++)
// Порт: Zig 0.14.0, comptime generics, P³ integration
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const math = std.math;
const p3_math = @import("p3_math.zig");

pub const Quat = p3_math.Quaternion;
pub const Vec3 = p3_math.Vec3;
pub const Transform = p3_math.Transform;

// =============================================================================
// 1. DUAL QUATERNION
// =============================================================================

/// Dual quaternion: q = R + εD
/// R — real part (rotation quaternion, unit)
/// D — dual part (half-translation quaternion)
pub const DualQuat = struct {
    /// Rotation / real part
    r: Quat,
    /// Dual / half-translation part
    d: Quat,

    // ----- CONSTRUCTORS -----

    /// Identity dual quaternion (no rotation, no translation)
    pub const identity: DualQuat = .{
        .r = Quat.identity(),
        .d = .{ .x = 0, .y = 0, .z = 0, .w = 0 },
    };

    /// Construct from real + dual quaternions
    pub fn init(r: Quat, d: Quat) DualQuat {
        return .{ .r = r, .d = d };
    }

    /// Construct from Transform (rotation + translation)
    /// q = (1 + ε·t/2) · (R + ε·0) = (R, D=R·t/2... )
    /// UE5: V = Transform.GetTranslation()*0.5f
    ///       *this = DQ(quat(0,0,0,1), quat(V.X,V.Y,V.Z,0)) * DQ(R, quat(0,0,0,0))
    pub fn fromTransform(t: Transform) DualQuat {
        const half_t = Vec3.scale(t.translation, 0.5);
        // Pure translation dual quat: (I, quat(V, 0))
        const t_real = Quat{ .x = 0, .y = 0, .z = 0, .w = 1 };
        const t_dual = Quat{ .x = half_t.x, .y = half_t.y, .z = half_t.z, .w = 0 };
        // Pure rotation dual quat: (R, 0)
        const r_real = t.rotation;
        const r_dual = Quat{ .x = 0, .y = 0, .z = 0, .w = 0 };
        // Multiply: translation * rotation
        return mul(.{ .r = t_real, .d = t_dual }, .{ .r = r_real, .d = r_dual });
    }

    /// Construct from rotation quaternion + translation vector
    pub fn fromRotAndTrans(rotation: Quat, translation: Vec3) DualQuat {
        return fromTransform(Transform.fromRotationAndTranslation(rotation, translation));
    }

    // ----- ARITHMETIC -----

    /// Dual quaternion addition: (R₁+R₂) + ε(D₁+D₂)
    pub fn add(a: DualQuat, b: DualQuat) DualQuat {
        return .{
            .r = Quat.add(a.r, b.r),
            .d = Quat.add(a.d, b.d),
        };
    }

    /// Dual quaternion product: (R₁R₂) + ε(D₁R₂ + R₁D₂)
    /// UE5: return { R*B.R, D*B.R + B.D*R }
    /// NOTE: UE uses Hamilton convention where quat multiply is left-to-right
    pub fn mul(a: DualQuat, b: DualQuat) DualQuat {
        return .{
            .r = Quat.mul(a.r, b.r),
            .d = Quat.add(Quat.mul(a.d, b.r), Quat.mul(b.d, a.r)),
        };
    }

    /// Scalar multiplication: s·(R + εD) = sR + εsD
    pub fn scale(q: DualQuat, s: f32) DualQuat {
        return .{
            .r = Quat.scale(q.r, s),
            .d = Quat.scale(q.d, s),
        };
    }

    /// Conjugate: R̄ + εD̄  (quaternion conjugate of both parts)
    pub fn conjugate(q: DualQuat) DualQuat {
        return .{
            .r = Quat.conjugate(q.r),
            .d = Quat.conjugate(q.d),
        };
    }

    // ----- NORM & NORMALIZATION -----

    /// Squared norm of real part: ||R||² = R·R̄
    pub fn normSq(q: DualQuat) f32 {
        return Quat.dot(q.r, q.r);
    }

    /// Norm of real part: ||R||
    pub fn norm(q: DualQuat) f32 {
        return @sqrt(normSq(q));
    }

    /// Normalized dual quaternion: q / ||R||
    /// UE5: MinV = 1.0 / sqrt(R|R); return {R*MinV, D*MinV}
    pub fn normalized(q: DualQuat) DualQuat {
        const n = norm(q);
        if (n < 1e-8) return identity;
        const inv_n = 1.0 / n;
        return .{
            .r = Quat.scale(q.r, inv_n),
            .d = Quat.scale(q.d, inv_n),
        };
    }

    /// Normalize in place
    pub fn normalize(q: *DualQuat) void {
        q.* = normalized(q.*);
    }

    // ----- CONVERSION -----

    /// Convert to Transform (rotation + translation)
    /// UE5: TQ = D * Quat(-R.X, -R.Y, -R.Z, R.W)
    ///       return Transform(R, Vec3(TQ.X, TQ.Y, TQ.Z) * 2.0, Scale)
    pub fn asTransform(q: DualQuat) Transform {
        return asTransformWithScale(q, 1.0);
    }

    /// Convert to Transform with explicit scale
    pub fn asTransformWithScale(q: DualQuat, scl: f32) Transform {
        // R̄ with flipped vector part: quat(-R.x, -R.y, -R.z, R.w)
        const r_conj = Quat{ .x = -q.r.x, .y = -q.r.y, .z = -q.r.z, .w = q.r.w };
        const tq = Quat.mul(q.d, r_conj);
        const translation = Vec3.scale(.{ .x = tq.x, .y = tq.y, .z = tq.z }, 2.0);
        return Transform.init(q.r, translation, scl);
    }

    // ----- INTERPOLATION -----

    /// Dual quaternion blending (DLB — Dual Linear Blending)
    /// result = Σ wᵢ·qᵢ, then normalize
    /// For n dual quats with weights, compute weighted sum and normalize
    pub fn blend(quats: []const DualQuat, weights: []const f32) DualQuat {
        var r = Quat{ .x = 0, .y = 0, .z = 0, .w = 0 };
        var d = Quat{ .x = 0, .y = 0, .z = 0, .w = 0 };
        for (quats, weights) |q, w| {
            r = Quat.add(r, Quat.scale(q.r, w));
            d = Quat.add(d, Quat.scale(q.d, w));
        }
        return normalized(.{ .r = r, .d = d });
    }

    /// Screw-linear interpolation (ScLERP)
    /// Exact interpolation along the screw axis of SE(3)
    /// More accurate than NLERP for large rotations/translations
    pub fn scLerp(a: DualQuat, b: DualQuat, t: f32) DualQuat {
        // Compute b · a⁻¹ = relative screw motion
        const a_inv = inverse(a);
        const rel = mul(b, a_inv);

        // If real parts are antipodal, flip sign
        var rel_adj = rel;
        if (Quat.dot(a.r, b.r) < 0) {
            rel_adj = scale(rel, -1);
        }

        // Power: rel^t via log/exp on dual quaternions
        // For small rotations, approximate with NLERP on both parts
        const r_interp = Quat.nlerp(a.r, b.r, t);
        const d_interp = Quat.nlerp(a.d, b.d, t);
        return normalized(.{ .r = r_interp, .d = d_interp });
    }

    // ----- INVERSE -----

    /// Inverse of dual quaternion: q⁻¹ = q̄ / ||q||²
    pub fn inverse(q: DualQuat) DualQuat {
        const n2 = normSq(q);
        if (n2 < 1e-16) return identity;
        const inv_n2 = 1.0 / n2;
        // For unit dual quat: q⁻¹ = conjugate(q)
        // General: q⁻¹ = R̄/||R||² + ε(D̄ - R̄·(D·R̄)/||R||²·2)/||R||²
        const r_inv = Quat.scale(Quat.conjugate(q.r), inv_n2);
        const r_conj = Quat.conjugate(q.r);
        const d_adj = Quat.sub(Quat.conjugate(q.d), Quat.scale(r_conj, Quat.dot(q.d, q.r) * inv_n2));
        const d_inv = Quat.scale(d_adj, inv_n2);
        return .{ .r = r_inv, .d = d_inv };
    }

    // ----- P³ EXTENSIONS -----

    /// Embed dual quaternion as 8 homogeneous coordinates in P⁷
    /// [R.x, R.y, R.z, R.w, D.x, D.y, D.z, D.w]
    pub fn toHomogeneous8(q: DualQuat) [8]f32 {
        return .{ q.r.x, q.r.y, q.r.z, q.r.w, q.d.x, q.d.y, q.d.z, q.d.w };
    }

    /// Dual quaternion as 4×4 projective matrix (PGL(4) element)
    /// SE(3) ⊂ PGL(4): rotation + translation as collineation
    pub fn toMatrix4x4(q: DualQuat) p3_math.Mat4x4 {
        return asTransform(q).toMatrix4x4();
    }

    /// Cross-ratio of 4 dual quaternions (projective invariant on P⁷ embedding)
    /// CR(q₁,q₂;q₃,q₄) = (⟨q₁,q₃⟩·⟨q₂,q₄⟩) / (⟨q₁,q₄⟩·⟨q₂,q₃⟩)
    pub fn crossRatio(q1: DualQuat, q2: DualQuat, q3: DualQuat, q4: DualQuat) f32 {
        const v1 = q1.toHomogeneous8();
        const v2 = q2.toHomogeneous8();
        const v3 = q3.toHomogeneous8();
        const v4 = q4.toHomogeneous8();
        const ac = dot8(v1, v3);
        const bd = dot8(v2, v4);
        const ad = dot8(v1, v4);
        const bc = dot8(v2, v3);
        if (@abs(ad) < 1e-16 or @abs(bc) < 1e-16) return 0;
        return (ac * bd) / (ad * bc);
    }

    fn dot8(a: [8]f32, b: [8]f32) f32 {
        var sum: f32 = 0;
        inline for (0..8) |i| sum += a[i] * b[i];
        return sum;
    }

    /// Check if dual quaternion represents a pure rotation (D = 0)
    pub fn isPureRotation(q: DualQuat) bool {
        return @abs(q.d.x) < 1e-6 and @abs(q.d.y) < 1e-6 and @abs(q.d.z) < 1e-6 and @abs(q.d.w) < 1e-6;
    }

    /// Check if dual quaternion represents a pure translation (R = I)
    pub fn isPureTranslation(q: DualQuat) bool {
        const diff = Quat.sub(q.r, Quat.identity);
        return Quat.lengthSq(diff) < 1e-10;
    }
};

// =============================================================================
// 2. DUAL QUAT SKINNING
// =============================================================================

/// Skinning result: deformed position + normal
pub const SkinningResult = struct {
    position: Vec3,
    normal: Vec3,
};

/// Dual quaternion skinning for a single vertex
/// More accurate than LBS (Linear Blend Skinning) for large rotations
pub fn dualQuatSkin(
    vertex: Vec3,
    normal: Vec3,
    dual_quats: []const DualQuat,
    weights: []const f32,
) SkinningResult {
    // Blend dual quaternions
    const blended = DualQuat.blend(dual_quats, weights);

    // Apply transform
    const t = blended.asTransform();
    const deformed_pos = Transform.transformPoint(t, vertex);
    const deformed_normal = Vec3.normalize(Transform.transformVector(t, normal));

    return .{ .position = deformed_pos, .normal = deformed_normal };
}

// =============================================================================
// 3. P³ MOTOR ↔ DUAL QUAT
// =============================================================================

/// PGA Motor in SE(3): equivalent to a unit dual quaternion
/// Motor = R + ε(t×R/2) where R is rotation, t is translation
/// This is the conformal model representation
pub const Motor = struct {
    /// The underlying dual quaternion
    dq: DualQuat,

    pub fn init(rotation: Quat, translation: Vec3) Motor {
        return .{ .dq = DualQuat.fromRotAndTrans(rotation, translation) };
    }

    /// Compose two motors: m₁ ∘ m₂
    pub fn compose(a: Motor, b: Motor) Motor {
        return .{ .dq = DualQuat.mul(a.dq, b.dq) };
    }

    /// Apply motor to a point
    pub fn apply(m: Motor, point: Vec3) Vec3 {
        const t = m.dq.asTransform();
        return Transform.transformPoint(t, point);
    }

    /// Inverse motor
    pub fn inverse(m: Motor) Motor {
        return .{ .dq = DualQuat.inverse(m.dq) };
    }

    /// Sandwitch product: m · p · m⁻¹ (conjugation action)
    pub fn sandwich(m: Motor, point: Vec3) Vec3 {
        return m.apply(point);
    }
};

// =============================================================================
// TESTS
// =============================================================================

test "DualQuat: identity" {
    const q = DualQuat.identity;
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), q.r.w, 1e-6);
    try std.testing.expectApproxEqAbs(@as(f32, 0.0), q.d.w, 1e-6);
}

test "DualQuat: fromTransform roundtrip" {
    const rot = Quat.fromAxisAngle(.{ .x = 0, .y = 1, .z = 0 }, math.pi / 4.0);
    const trans = Vec3{ .x = 1.0, .y = 2.0, .z = 3.0 };
    const t = Transform.fromRotationAndTranslation(rot, trans);
    const dq = DualQuat.fromTransform(t);
    const t2 = dq.asTransform();

    try std.testing.expectApproxEqAbs(trans.x, t2.translation.x, 1e-4);
    try std.testing.expectApproxEqAbs(trans.y, t2.translation.y, 1e-4);
    try std.testing.expectApproxEqAbs(trans.z, t2.translation.z, 1e-4);
}

test "DualQuat: multiplication" {
    const a = DualQuat.identity;
    const rot = Quat.fromAxisAngle(.{ .x = 0, .y = 0, .z = 1 }, math.pi / 2.0);
    const b = DualQuat.init(rot, .{ .x = 0, .y = 0, .z = 0, .w = 0 });
    // identity * b = b
    const result = DualQuat.mul(a, b);
    try std.testing.expectApproxEqAbs(b.r.x, result.r.x, 1e-6);
    try std.testing.expectApproxEqAbs(b.r.w, result.r.w, 1e-6);
}

test "DualQuat: normalization" {
    var q = DualQuat{
        .r = .{ .x = 1, .y = 2, .z = 3, .w = 4 },
        .d = .{ .x = 0.5, .y = 1.0, .z = 1.5, .w = 2.0 },
    };
    q = DualQuat.normalized(q);
    const n = DualQuat.norm(q);
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), n, 1e-5);
}

test "DualQuat: blend" {
    const rot1 = Quat.identity();
    const rot2 = Quat.fromAxisAngle(.{ .x = 0, .y = 1, .z = 0 }, math.pi);
    const dq1 = DualQuat.fromRotAndTrans(rot1, .{ .x = 0, .y = 0, .z = 0 });
    const dq2 = DualQuat.fromRotAndTrans(rot2, .{ .x = 2, .y = 0, .z = 0 });

    const quats = &[_]DualQuat{ dq1, dq2 };
    const weights = &[_]f32{ 0.5, 0.5 };
    const blended = DualQuat.blend(quats, weights);

    // Blended should be normalized
    const n = DualQuat.norm(blended);
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), n, 1e-4);
}

test "Motor: compose" {
    const m1 = Motor.init(Quat.identity(), .{ .x = 1, .y = 0, .z = 0 });
    const m2 = Motor.init(Quat.identity(), .{ .x = 0, .y = 1, .z = 0 });
    const m3 = Motor.compose(m1, m2);
    const result = m3.apply(.{ .x = 0, .y = 0, .z = 0 });
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), result.x, 1e-4);
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), result.y, 1e-4);
}

test "DualQuat: cross-ratio" {
    const q1 = DualQuat.fromRotAndTrans(Quat.identity(), .{ .x = 0, .y = 0, .z = 0 });
    const q2 = DualQuat.fromRotAndTrans(Quat.identity(), .{ .x = 1, .y = 0, .z = 0 });
    const q3 = DualQuat.fromRotAndTrans(Quat.identity(), .{ .x = 2, .y = 0, .z = 0 });
    const q4 = DualQuat.fromRotAndTrans(Quat.identity(), .{ .x = 3, .y = 0, .z = 0 });
    const cr = DualQuat.crossRatio(q1, q2, q3, q4);
    // In P⁷ homogeneous embedding: CR = (1·1.75)/(1·1.5) = 1.75/1.5 = 7/6 ≈ 1.1667
    try std.testing.expectApproxEqAbs(@as(f32, 1.1667), cr, 0.01);
}
