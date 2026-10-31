// =============================================================================
// P³ QUATERNION — КВАТЕРНИОНЫ, SO(3), S³→P³
// =============================================================================
//
// КЛЮЧЕВАЯ ТЕОРЕМА: S³/{±1} ≅ SO(3) ≅ P³
//
//   Кватернион единичной нормы q ∈ S³ задает вращение:
//     R_q(x) = q · x · q⁻¹
//
//   Антиподная идентификация: q и −q задают ОДНО вращение.
//   Поэтому S³/{±1} ≅ SO(3).
//
//   По теореме: P³ ≅ SO(3) (через ориентацию репера в R³).
//
//   Структура:
//     S³  ──2:1──→  SO(3)  ──1:1──→  P³
//     q          ↦  R_q          ↦  [репер]
//
//   Это ДВОЙНОЕ НАКРЫТИЕ: один оборот в S³ = пол-оборота в SO(3).
//   Группа голономии π₁(P³) = ℤ/2ℤ.
//
// АЛГЕБРА:
//   q = w + xi + yj + zk
//   i² = j² = k² = ijk = −1
//   ij = k,  jk = i,  ki = j
//   q⁻¹ = q̄ / ‖q‖²  (q̄ = w − xi − yj − zk)
//
// СВЯЗЬ С P³ Engine:
//   - HomVec4(x,y,z,w) ↔ Quat(w,x,y,z) — один и тот же R⁴!
//   - Геодезические на S³ = большие круги = однопараметрические подгруппы
//   - Камера на S³ = кватернион вращения
//   - Gimbal lock НЕВОЗМОЖЕН — кватернионы глобальны
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const math = std.math;
const p3_kernel = @import("p3_kernel.zig");

pub const HomVec4 = p3_kernel.HomVec4;

// =============================================================================
// 1. КВАТЕРНИОН
// =============================================================================

/// Единичный кватернион q = w + xi + yj + zk
///
/// ИНВАРИАНТ: ‖q‖ = 1 (на S³)
/// Автонормировка в конструкторах.
pub const Quat = struct {
    w: f64, // scalar part
    x: f64, // i-component
    y: f64, // j-component
    z: f64, // k-component

    /// Единичный кватернион (identity rotation)
    pub const identity = Quat{ .w = 1.0, .x = 0.0, .y = 0.0, .z = 0.0 };

    /// Создать из компонентов (без нормировки)
    pub fn init(w: f64, x: f64, y: f64, z: f64) Quat {
        return .{ .w = w, .x = x, .y = y, .z = z };
    }

    /// Создать из углов Эйлера (ZYX convention: yaw→pitch→roll)
    /// ИСПОЛЬЗУЕТСЯ ТОЛЬКО для инициализации — потом работаем на S³
    pub fn fromEulerAngles(yaw: f64, pitch: f64, roll: f64) Quat {
        const cy = @cos(yaw * 0.5);
        const sy = @sin(yaw * 0.5);
        const cp = @cos(pitch * 0.5);
        const sp = @sin(pitch * 0.5);
        const cr = @cos(roll * 0.5);
        const sr = @sin(roll * 0.5);

        return Quat.init(
            cr * cp * cy + sr * sp * sy,
            sr * cp * cy - cr * sp * sy,
            cr * sp * cy + sr * cp * sy,
            cr * cp * sy - sr * sp * cy,
        ).normalize();
    }

    /// Создать из оси вращения + угол (рад)
    pub fn fromAxisAngle(axis: [3]f64, angle: f64) Quat {
        const half = angle * 0.5;
        const s = @sin(half);
        // Нормируем ось
        const len = @sqrt(axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]);
        if (len < 1e-15) return identity;
        const nx = axis[0] / len;
        const ny = axis[1] / len;
        const nz = axis[2] / len;
        return Quat.init(@cos(half), s * nx, s * ny, s * nz);
    }

    /// Конвертировать HomVec4 → Quat (один и тот же R⁴!)
    pub fn fromHomVec4(h: HomVec4) Quat {
        return Quat.init(h.w, h.x, h.y, h.z);
    }

    /// Конвертировать Quat → HomVec4
    pub fn toHomVec4(self: Quat) HomVec4 {
        return HomVec4.init(self.x, self.y, self.z, self.w);
    }

    // =================================================================
    // АЛГЕБРА
    // =================================================================

    /// Умножение кватернионов (гамильтоново произведение)
    pub fn mul(self: Quat, other: Quat) Quat {
        return Quat.init(
            self.w * other.w - self.x * other.x - self.y * other.y - self.z * other.z,
            self.w * other.x + self.x * other.w + self.y * other.z - self.z * other.y,
            self.w * other.y - self.x * other.z + self.y * other.w + self.z * other.x,
            self.w * other.z + self.x * other.y - self.y * other.x + self.z * other.w,
        );
    }

    /// Сопряжённый: q̄ = w − xi − yj − zk
    pub fn conjugate(self: Quat) Quat {
        return Quat.init(self.w, -self.x, -self.y, -self.z);
    }

    /// Антипод: −q (ТОТ ЖЕ элемент в P³, ДРУГОЙ на S³)
    /// Это голономия ℤ/2ℤ: один обход в S³ → антипод
    pub fn antipode(self: Quat) Quat {
        return Quat.init(-self.w, -self.x, -self.y, -self.z);
    }

    /// Норма: ‖q‖² = w² + x² + y² + z²
    pub fn normSq(self: Quat) f64 {
        return self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z;
    }

    /// Норма: ‖q‖
    pub fn norm(self: Quat) f64 {
        return @sqrt(self.normSq());
    }

    /// Нормировать на S³
    pub fn normalize(self: Quat) Quat {
        const n = self.norm();
        if (n < 1e-15) return identity;
        return Quat.init(self.w / n, self.x / n, self.y / n, self.z / n);
    }

    /// Обратный: q⁻¹ = q̄ / ‖q‖²
    pub fn inverse(self: Quat) Quat {
        const nsq = self.normSq();
        if (nsq < 1e-30) return identity;
        const c = self.conjugate();
        return Quat.init(c.w / nsq, c.x / nsq, c.y / nsq, c.z / nsq);
    }

    /// Скалярное произведение ⟨q₁, q₂⟩
    pub fn dot(self: Quat, other: Quat) f64 {
        return self.w * other.w + self.x * other.x + self.y * other.y + self.z * other.z;
    }

    /// Экспонента: exp(v) для чистого кватерниона v = 0 + xi + yj + zk
    /// exp(v) = cos(‖v‖) + sin(‖v‖)·v/‖v‖
    pub fn exp(pure: Quat) Quat {
        const v_len = @sqrt(pure.x * pure.x + pure.y * pure.y + pure.z * pure.z);
        if (v_len < 1e-15) return identity;
        const s = @sin(v_len) / v_len;
        return Quat.init(@cos(v_len), s * pure.x, s * pure.y, s * pure.z);
    }

    /// Логарифм: log(q) для единичного кватерниона
    /// log(q) = acos(w) · v/‖v‖  (где v = xi+yj+zk)
    pub fn log(self: Quat) Quat {
        const v_len = @sqrt(self.x * self.x + self.y * self.y + self.z * self.z);
        if (v_len < 1e-15) return Quat.init(0, 0, 0, 0);
        const angle = math.acos(std.math.clamp(self.w, -1.0, 1.0));
        const s = angle / v_len;
        return Quat.init(0, s * self.x, s * self.y, s * self.z);
    }

    /// SLERP: сферическая линейная интерполяция на S³
    /// КРИТИЧНО: выбирает КОРОТКИЙ путь (|⟨q₁,q₂⟩| для перехода через антипод)
    pub fn slerp(self: Quat, other: Quat, t: f64) Quat {
        var cos_half_angle = self.dot(other);

        // Если угол > π/2 — перейти через антипод (КОРОТКИЙ путь в P³)
        if (cos_half_angle < 0) {
            cos_half_angle = -cos_half_angle;
        }

        cos_half_angle = std.math.clamp(cos_half_angle, -1.0, 1.0);
        const half_angle = math.acos(cos_half_angle);

        if (@abs(half_angle) < 1e-6) {
            // Очень близко — линейная интерполяция
            return Quat.init(
                self.w + t * (other.w - self.w),
                self.x + t * (other.x - self.x),
                self.y + t * (other.y - self.y),
                self.z + t * (other.z - self.z),
            ).normalize();
        }

        const sin_half_angle = @sin(half_angle);
        const s1 = @sin((1.0 - t) * half_angle) / sin_half_angle;
        const s2 = @sin(t * half_angle) / sin_half_angle;

        return Quat.init(
            s1 * self.w + s2 * other.w,
            s1 * self.x + s2 * other.x,
            s1 * self.y + s2 * other.y,
            s1 * self.z + s2 * other.z,
        ).normalize();
    }

    // =================================================================
    // SO(3) ДЕЙСТВИЕ: R_q(v) = q · v · q⁻¹
    // =================================================================

    /// Вращение вектора через кватернион: R_q(v) = q · v · q⁻¹
    /// v ∈ R³ — чистый вектор (0, vx, vy, vz)
    pub fn rotateVector(self: Quat, v: [3]f64) [3]f64 {
        // q · v (v как чистый кватернион)
        const qv = Quat.init(
            -self.x * v[0] - self.y * v[1] - self.z * v[2],
            self.w * v[0] + self.y * v[2] - self.z * v[1],
            self.w * v[1] - self.x * v[2] + self.z * v[0],
            self.w * v[2] + self.x * v[1] - self.y * v[0],
        );
        // q·v · q⁻¹ = q·v · q̄ (для единичного q)
        const qbar = self.conjugate();
        const result = qv.mul(qbar);
        return .{ result.x, result.y, result.z };
    }

    /// Извлечь матрицу вращения SO(3) — 3×3 column-major
    pub fn toRotationMatrix(self: Quat) [9]f64 {
        const w = self.w;
        const x = self.x;
        const y = self.y;
        const z = self.z;
        return .{
            1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y),
            2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x),
            2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y),
        };
    }

    /// Извлечь ось вращения + угол
    pub const AxisAngleResult = struct { axis: [3]f64, angle: f64 };

    pub fn toAxisAngle(self: Quat) AxisAngleResult {
        const cos_half = std.math.clamp(self.w, -1.0, 1.0);
        const angle = 2.0 * math.acos(cos_half);
        const sin_half = @sqrt(1.0 - cos_half * cos_half);

        if (sin_half < 1e-10) {
            return .{ .axis = .{ 0, 0, 1 }, .angle = 0 };
        }

        return .{
            .axis = .{
                self.x / sin_half,
                self.y / sin_half,
                self.z / sin_half,
            },
            .angle = angle,
        };
    }

    // =================================================================
    // ℤ/2ℤ ГОЛОНОМИЯ
    // =================================================================

    /// Класс голономии: один обход в S³ → Hol = -1 (антипод)
    /// Два обхода → Hol² = +1 (возврат)
    /// Это π₁(P³) = ℤ/2ℤ
    pub const HolonomyClass = enum(u1) {
        identity = 0, // чётное число обходов (Hol² = +1)
        antipode = 1, // нечётное число обходов (Hol = -1)
    };

    /// Определить класс голономии по кватерниону
    /// Если q ≈ +1 → identity, если q ≈ −1 → antipode
    pub fn holonomyClass(self: Quat) HolonomyClass {
        // q близко к +1 (identity) или к -1 (antipode)?
        if (self.w >= 0) return .identity;
        return .antipode;
    }

    /// Проверить: два кватерниона задают один элемент в P³?
    /// (т.е. отличаются на знак: q₁ = ±q₂)
    pub fn equalInP3(self: Quat, other: Quat) bool {
        const d1 = self.dot(other);
        const d2 = self.dot(other.antipode());
        // Ближе к +1 или к -1?
        return @abs(d1) > 0.999999 or @abs(d2) > 0.999999;
    }
};

// =============================================================================
// 2. FS-РАССТОЯНИЕ ЧЕРЕЗ КВАТЕРНИОНЫ
// =============================================================================

/// FS-расстояние между двумя точками на S³ (как кватернионами)
/// d_FS(q₁, q₂) = 2 · arccos(|⟨q₁, q₂⟩|)
///
/// Множитель 2: потому что S³ — ДВОЙНОЕ накрытие P³.
/// Один обход в S³ = пол-оборота в SO(3).
pub fn fsDistanceQuat(q1: Quat, q2: Quat) f64 {
    const cos_angle = @abs(q1.dot(q2));
    return 2.0 * math.acos(std.math.clamp(cos_angle, 0.0, 1.0));
}

/// Угол вращения для кватерниона (в радианах, [0, 2π])
/// Один полный оборот в SO(3) = 2π
pub fn rotationAngle(q: Quat) f64 {
    const cos_half = std.math.clamp(@abs(q.w), 0.0, 1.0);
    return 2.0 * math.acos(cos_half);
}

// =============================================================================
// 3. ТЕСТЫ
// =============================================================================

test "Quat: identity norm" {
    const q = Quat.identity;
    try std.testing.expectApproxEqAbs(q.norm(), 1.0, 1e-10);
}

test "Quat: fromAxisAngle then normalize" {
    const q = Quat.fromAxisAngle(.{ 0, 1, 0 }, math.pi * 0.5);
    try std.testing.expectApproxEqAbs(q.norm(), 1.0, 1e-10);
}

test "Quat: multiplication is associative" {
    const a = Quat.fromAxisAngle(.{ 1, 0, 0 }, 0.3);
    const b = Quat.fromAxisAngle(.{ 0, 1, 0 }, 0.5);
    const c = Quat.fromAxisAngle(.{ 0, 0, 1 }, 0.7);
    const ab_c = a.mul(b).mul(c);
    const a_bc = a.mul(b.mul(c));
    try std.testing.expectApproxEqAbs(ab_c.w, a_bc.w, 1e-10);
    try std.testing.expectApproxEqAbs(ab_c.x, a_bc.x, 1e-10);
}

test "Quat: q · q⁻¹ = identity" {
    const q = Quat.fromAxisAngle(.{ 1, 2, 3 }, 1.23);
    const result = q.mul(q.inverse());
    try std.testing.expectApproxEqAbs(result.w, 1.0, 1e-8);
    try std.testing.expectApproxEqAbs(result.x, 0.0, 1e-8);
}

test "Quat: antipode is same in P³" {
    const q = Quat.fromAxisAngle(.{ 0, 1, 0 }, 0.5);
    try std.testing.expect(q.equalInP3(q.antipode()));
}

test "Quat: rotateVector preserves length" {
    const q = Quat.fromAxisAngle(.{ 1, 1, 1 }, 2.0);
    const v = [3]f64{ 3.0, 4.0, 0.0 };
    const rv = q.rotateVector(v);
    const len_orig = @sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    const len_rot = @sqrt(rv[0] * rv[0] + rv[1] * rv[1] + rv[2] * rv[2]);
    try std.testing.expectApproxEqAbs(len_orig, len_rot, 1e-10);
}

test "Quat: toRotationMatrix is SO(3)" {
    const q = Quat.fromAxisAngle(.{ 0, 1, 0 }, math.pi * 0.5);
    const m = q.toRotationMatrix();
    // det(R) should be 1
    const det = m[0] * (m[4] * m[8] - m[5] * m[7]) -
        m[1] * (m[3] * m[8] - m[5] * m[6]) +
        m[2] * (m[3] * m[7] - m[4] * m[6]);
    try std.testing.expectApproxEqAbs(det, 1.0, 1e-8);
}

test "Quat: slerp stays on S³" {
    const q1 = Quat.fromAxisAngle(.{ 1, 0, 0 }, 0.0);
    const q2 = Quat.fromAxisAngle(.{ 1, 0, 0 }, math.pi * 0.5);
    const mid = q1.slerp(q2, 0.5);
    try std.testing.expectApproxEqAbs(mid.norm(), 1.0, 1e-8);
}

test "Quat: holonomy of identity" {
    try std.testing.expect(Quat.identity.holonomyClass() == .identity);
}

test "Quat: holonomy of antipode" {
    const q = Quat.init(-1, 0, 0, 0); // antipode of identity
    try std.testing.expect(q.holonomyClass() == .antipode);
}

test "Quat: exp-log roundtrip" {
    const pure = Quat.init(0, 0.3, 0.5, 0.1);
    const q = Quat.exp(pure);
    const log_q = q.log();
    try std.testing.expectApproxEqAbs(log_q.x, pure.x, 1e-8);
    try std.testing.expectApproxEqAbs(log_q.y, pure.y, 1e-8);
    try std.testing.expectApproxEqAbs(log_q.z, pure.z, 1e-8);
}

test "Quat: fromHomVec4 / toHomVec4 roundtrip" {
    const h = HomVec4.fromCartesian(.{ 0.1, 0.2, 0.3 });
    const q = Quat.fromHomVec4(h.normalize());
    const h2 = q.toHomVec4();
    try std.testing.expectApproxEqAbs(h2.x, h.normalize().x, 1e-10);
}

test "fsDistanceQuat: to self is 0" {
    const q = Quat.fromAxisAngle(.{ 0, 1, 0 }, 0.7);
    try std.testing.expectApproxEqAbs(fsDistanceQuat(q, q), 0.0, 1e-10);
}

test "fsDistanceQuat: to antipode is π" {
    const q = Quat.fromAxisAngle(.{ 0, 1, 0 }, 0.3);
    const aq = q.antipode();
    // On S³: geodesic distance from q to -q is π
    // dot(q, -q) = -1, acos(-1) = π
    const s3_dist = math.acos(std.math.clamp(q.dot(aq), -1.0, 1.0));
    try std.testing.expectApproxEqAbs(s3_dist, math.pi, 1e-6);
    // In P³ (after identification): q ≡ -q, so distance = 0
    const p3_dist = fsDistanceQuat(q, aq);
    try std.testing.expectApproxEqAbs(p3_dist, 0.0, 1e-6);
}
