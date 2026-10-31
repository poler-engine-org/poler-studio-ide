// =============================================================================
// P³ TENSOR v1.0 — ZIG
// =============================================================================
//
// Тензорная алгебра для проективного пространства P³.
//
// В P³ Engine тензоры нужны для:
//   - Метрический тензор g_ij (внутреннее произведение на кривом многообразии)
//   - Тензор Римана R^i_{jkl} (кривизна пространства)
//   - Тензор энергии-импульса T_ij (физика)
//   - Символы Кристоффеля Γ^i_{jk} (связность)
//   - Pullback/pushforward тензоров под проективными отображениями
//
// РАНГИ:
//   (0,0) — скаляр (f32)
//   (1,0) — контравариантный вектор (Vec4)
//   (0,1) — ковариантный вектор / ковектор (Covec4)
//   (1,1) — линейный оператор (Mat4x4)
//   (0,2) — билинейная форма (MetricTensor)
//   (1,2) — связность (ChristoffelSymbols)
//   (1,3) — кривизна (RiemannTensor)
//
// P³ ОБОБЩЕНИЯ:
//   - Тензоры на P³ — сечения расслоений, не глобальные функции
//   - Метрика g_ij проективная:.signature (3,1) для лоренцевой,
//     (4,0) для евклидовой
//   - Pullback под f ∈ PGL(4) сохраняет cross-ratio
//   - Связность — проективная связность Вейля
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const math = std.math;

// =============================================================================
// 1. DIMENSIONS
// =============================================================================

/// Размерность проективного пространства P³ = 4 однородных координаты
pub const DIM: usize = 4;

/// Количество компонент тензора ранга (r,s): N^(r+s) где N = DIM
pub fn tensorComponents(r: usize, s: usize) usize {
    const total = r + s;
    var result: usize = 1;
    for (0..total) |_| {
        result *= DIM;
    }
    return result;
}

// =============================================================================
// 2. SCALAR — тензор ранга (0,0)
// =============================================================================

/// Скаляр — инвариант под PGL(4) только если он projective invariant
/// (как cross-ratio). Обычные числа — НЕ инварианты!
pub const Scalar = f32;

// =============================================================================
// 3. VEC4 — контравариантный вектор ранга (1,0)
// =============================================================================

/// Контравариантный вектор в P³ — однородная координата [x₀:x₁:x₂:x₃].
/// Под действием A ∈ PGL(4): v' = A·v
pub const Vec4 = [DIM]f32;

/// Ковектор (ковариантный вектор) ранга (0,1) — линейная форма.
/// Под действием A ∈ PGL(4): w' = w·A^{-1}
pub const Covec4 = [DIM]f32;

/// Создать Vec4.
pub fn vec4(x0: f32, x1: f32, x2: f32, x3: f32) Vec4 {
    return .{ x0, x1, x2, x3 };
}

/// Нулевой вектор.
pub fn vec4Zero() Vec4 {
    return .{ 0, 0, 0, 0 };
}

/// Сложение векторов.
pub fn vec4Add(a: Vec4, b: Vec4) Vec4 {
    return .{ a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3] };
}

/// Вычитание векторов.
pub fn vec4Sub(a: Vec4, b: Vec4) Vec4 {
    return .{ a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3] };
}

/// Умножение на скаляр.
pub fn vec4Scale(v: Vec4, s: f32) Vec4 {
    return .{ v[0] * s, v[1] * s, v[2] * s, v[3] * s };
}

/// Скалярное произведение (евклидово — для ковектора и вектора).
pub fn vec4Dot(w: Covec4, v: Vec4) f32 {
    return w[0] * v[0] + w[1] * v[1] + w[2] * v[2] + w[3] * v[3];
}

/// Внешнее произведение (wedge): Vec4 ∧ Vec4 → Bivector (ранга (2,0)).
/// 6 компонент в 4D: e01, e02, e03, e12, e13, e23
pub const Bivector6 = [6]f32;

pub fn vec4Wedge(a: Vec4, b: Vec4) Bivector6 {
    return .{
        a[0] * b[1] - a[1] * b[0], // e01
        a[0] * b[2] - a[2] * b[0], // e02
        a[0] * b[3] - a[3] * b[0], // e03
        a[1] * b[2] - a[2] * b[1], // e12
        a[1] * b[3] - a[3] * b[1], // e13
        a[2] * b[3] - a[3] * b[2], // e23
    };
}

// =============================================================================
// 4. MAT4X4 — тензор ранга (1,1), линейный оператор
// =============================================================================

/// Матрица 4×4 — элемент End(R⁴) или представление PGL(4).
/// Row-major: data[i*4+j] = элемент в строке i, столбце j.
pub const Mat4x4 = [16]f32;

/// Единичная матрица.
pub fn mat4Identity() Mat4x4 {
    return .{
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
        0, 0, 0, 1,
    };
}

/// Нулевая матрица.
pub fn mat4Zero() Mat4x4 {
    return .{ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 };
}

/// Доступ к элементу матрицы.
pub fn mat4Get(m: Mat4x4, i: usize, j: usize) f32 {
    return m[i * DIM + j];
}

/// Установка элемента матрицы (возвращает новую матрицу).
pub fn mat4Set(m: Mat4x4, i: usize, j: usize, val: f32) Mat4x4 {
    var result = m;
    result[i * DIM + j] = val;
    return result;
}

/// Умножение матриц: C = A · B.
pub fn mat4Mul(a: Mat4x4, b: Mat4x4) Mat4x4 {
    var result = mat4Zero();
    for (0..DIM) |i| {
        for (0..DIM) |j| {
            var sum: f32 = 0;
            for (0..DIM) |k| {
                sum += a[i * DIM + k] * b[k * DIM + j];
            }
            result[i * DIM + j] = sum;
        }
    }
    return result;
}

/// Матрица · вектор: y = A · x.
pub fn mat4MulVec(m: Mat4x4, v: Vec4) Vec4 {
    var result = vec4Zero();
    for (0..DIM) |i| {
        var sum: f32 = 0;
        for (0..DIM) |j| {
            sum += m[i * DIM + j] * v[j];
        }
        result[i] = sum;
    }
    return result;
}

/// Ковектор · матрица: w' = w · A.
pub fn cov4MulMat(w: Covec4, m: Mat4x4) Covec4 {
    var result: Covec4 = .{ 0, 0, 0, 0 };
    for (0..DIM) |j| {
        var sum: f32 = 0;
        for (0..DIM) |i| {
            sum += w[i] * m[i * DIM + j];
        }
        result[j] = sum;
    }
    return result;
}

/// Транспонирование.
pub fn mat4Transpose(m: Mat4x4) Mat4x4 {
    var result = mat4Zero();
    for (0..DIM) |i| {
        for (0..DIM) |j| {
            result[i * DIM + j] = m[j * DIM + i];
        }
    }
    return result;
}

/// След матрицы: tr(A) = Σ A_ii.
pub fn mat4Trace(m: Mat4x4) f32 {
    return m[0] + m[5] + m[10] + m[15];
}

/// Определитель 4×4 через разложение по строке (Laplace expansion).
pub fn mat4Det(m: Mat4x4) f32 {
    // Разложение по первой строке
    const m01 = m[1]; const m02 = m[2]; const m03 = m[3];
    const m10 = m[4]; const m11 = m[5]; const m12 = m[6]; const m13 = m[7];
    const m20 = m[8]; const m21 = m[9]; const m22 = m[10]; const m23 = m[11];
    const m30 = m[12]; const m31 = m[13]; const m32 = m[14]; const m33 = m[15];

    // 3×3 minors
    const a = m11 * (m22 * m33 - m23 * m32) - m12 * (m21 * m33 - m23 * m31) + m13 * (m21 * m32 - m22 * m31);
    const b = m10 * (m22 * m33 - m23 * m32) - m12 * (m20 * m33 - m23 * m30) + m13 * (m20 * m32 - m22 * m30);
    const c = m10 * (m21 * m33 - m23 * m31) - m11 * (m20 * m33 - m23 * m30) + m13 * (m20 * m31 - m21 * m30);
    const d = m10 * (m21 * m32 - m22 * m31) - m11 * (m20 * m32 - m22 * m30) + m12 * (m20 * m31 - m21 * m30);

    return m[0] * a - m01 * b + m02 * c - m03 * d;
}

/// Обратная матрица (Gauss-Jordan elimination).
pub fn mat4Inverse(m: Mat4x4) ?Mat4x4 {
    // Augmented matrix [M | I]
    var aug: [4][8]f32 = undefined;
    for (0..4) |i| {
        for (0..4) |j| {
            aug[i][j] = m[i * 4 + j];
            aug[i][j + 4] = if (i == j) 1.0 else 0.0;
        }
    }

    // Forward elimination (partial pivoting)
    for (0..4) |col| {
        // Find pivot
        var max_val = @abs(aug[col][col]);
        var max_row: usize = col;
        for (col + 1..4) |row| {
            if (@abs(aug[row][col]) > max_val) {
                max_val = @abs(aug[row][col]);
                max_row = row;
            }
        }
        if (max_val < 1e-10) return null; // Singular

        // Swap rows
        if (max_row != col) {
            for (0..8) |j| {
                const tmp = aug[col][j];
                aug[col][j] = aug[max_row][j];
                aug[max_row][j] = tmp;
            }
        }

        // Scale pivot row
        const pivot = aug[col][col];
        for (0..8) |j| {
            aug[col][j] /= pivot;
        }

        // Eliminate column
        for (0..4) |row| {
            if (row == col) continue;
            const factor = aug[row][col];
            for (0..8) |j| {
                aug[row][j] -= factor * aug[col][j];
            }
        }
    }

    // Extract inverse from augmented part
    var result = mat4Zero();
    for (0..4) |i| {
        for (0..4) |j| {
            result[i * 4 + j] = aug[i][j + 4];
        }
    }
    return result;
}

/// Масштабирование матрицы.
pub fn mat4Scale(m: Mat4x4, s: f32) Mat4x4 {
    var result: Mat4x4 = undefined;
    for (0..16) |i| {
        result[i] = m[i] * s;
    }
    return result;
}

// =============================================================================
// 5. METRIC TENSOR — ранг (0,2), симметричная билинейная форма
// =============================================================================

/// Метрический тензор g_ij — симметричная 4×4 матрица.
///
/// В P³:
///   - Евклидова сигнатура: (+,+,+,+) — diag(1,1,1,1)
///   - Лоренцева сигнатура: (-,+,+,+) — diag(-1,1,1,1)
///   - Проективная сигнатура: зависит от аффинной карты
pub const MetricTensor = struct {
    /// g_ij в row-major, симметричная: g_ij = g_ji
    components: Mat4x4,

    pub fn euclidean() MetricTensor {
        return .{ .components = mat4Identity() };
    }

    pub fn lorentzian() MetricTensor {
        return .{ .components = .{
            -1, 0, 0, 0,
            0,  1, 0, 0,
            0,  0, 1, 0,
            0,  0, 0, 1,
        } };
    }

    /// Проективная метрика: diag(1,1,1,-1) для S³ ⊂ R⁴
    pub fn projective() MetricTensor {
        return .{ .components = .{
            1, 0, 0,  0,
            0, 1, 0,  0,
            0, 0, 1,  0,
            0, 0, 0, -1,
        } };
    }

    pub fn fromMatrix(m: Mat4x4) MetricTensor {
        return .{ .components = m };
    }

    /// Внутреннее произведение: g(v, w) = v^i g_ij w^j
    pub fn inner(self: MetricTensor, v: Vec4, w: Vec4) f32 {
        // g_ij v^i w^j
        var result: f32 = 0;
        for (0..DIM) |i| {
            for (0..DIM) |j| {
                result += v[i] * self.components[i * DIM + j] * w[j];
            }
        }
        return result;
    }

    /// Квадрат нормы: |v|² = g(v, v)
    pub fn normSq(self: MetricTensor, v: Vec4) f32 {
        return self.inner(v, v);
    }

    /// Опускание индекса: v^i → v_i = g_ij v^j
    pub fn lower(self: MetricTensor, v: Vec4) Covec4 {
        var result: Covec4 = .{ 0, 0, 0, 0 };
        for (0..DIM) |i| {
            for (0..DIM) |j| {
                result[i] += self.components[i * DIM + j] * v[j];
            }
        }
        return result;
    }

    /// Обратная метрика g^{ij} (контравариантная)
    pub fn inverse(self: MetricTensor) ?MetricTensor {
        const inv = mat4Inverse(self.components) orelse return null;
        return .{ .components = inv };
    }

    /// Поднятие индекса: v_i → v^i = g^{ij} v_j
    pub fn raise(self: MetricTensor, w: Covec4) ?Vec4 {
        const inv = self.inverse() orelse return null;
        var result = vec4Zero();
        for (0..DIM) |i| {
            for (0..DIM) |j| {
                result[i] += inv.components[i * DIM + j] * w[j];
            }
        }
        return result;
    }

    /// Определитель метрики.
    pub fn det(self: MetricTensor) f32 {
        return mat4Det(self.components);
    }
};

// =============================================================================
// 6. CHRISTOFFEL SYMBOLS — ранг (1,2), связность
// =============================================================================

/// Символы Кристоффеля Γ^i_{jk}.
///
/// 4 × 4 × 4 = 64 компоненты.
/// Симметрия: Γ^i_{jk} = Γ^i_{kj} (torsion-free) → 4 × 10 = 40 независимых.
///
/// В проективной геометрии — символы Вейля (проективная связность).
pub const ChristoffelSymbols = struct {
    /// Γ^i_{jk} — data[i][j][k]
    data: [DIM][DIM][DIM]f32,

    pub fn zero() ChristoffelSymbols {
        return .{ .data = .{.{.{0} ** DIM} ** DIM} ** DIM };
    }

    /// Из метрики (Levi-Civita связность):
    /// Γ^i_{jk} = ½ g^{il} (∂_j g_{lk} + ∂_k g_{lj} - ∂_l g_{jk})
    ///
    /// Для постоянной метрики: Γ = 0 (плоское пространство).
    pub fn fromConstantMetric(g: MetricTensor) ChristoffelSymbols {
        _ = g;
        // Для постоянной метрики все производные = 0, поэтому Γ = 0
        return zero();
    }

    /// Доступ к компоненте.
    pub fn get(self: ChristoffelSymbols, i: usize, j: usize, k: usize) f32 {
        return self.data[i][j][k];
    }

    /// Установка компоненты.
    pub fn set(self: *ChristoffelSymbols, i: usize, j: usize, k: usize, val: f32) void {
        self.data[i][j][k] = val;
        // Симметрия по нижним индексам
        self.data[i][k][j] = val;
    }

    /// Ковариантная производная вектора:
    /// ∇_j v^i = ∂_j v^i + Γ^i_{jk} v^k
    pub fn covDerivVec(self: ChristoffelSymbols, v: Vec4, deriv_dir: usize) Vec4 {
        var result = vec4Zero();
        for (0..DIM) |i| {
            for (0..DIM) |k| {
                result[i] += self.data[i][deriv_dir][k] * v[k];
            }
        }
        return result;
    }
};

// =============================================================================
// 7. RIEMANN TENSOR — ранг (1,3), кривизна
// =============================================================================

/// Тензор Римана R^i_{jkl}.
///
/// 4 × 4 × 4 × 4 = 256 компонент.
/// Симметрии:
///   R^i_{jkl} = -R^i_{jlk}      (антисимметрия по k,l)
///   R^i_{jkl} + R^i_{klj} + R^i_{ljk} = 0  (тождество Бьянки)
///   → 20 независимых компонент в 4D
///
/// Для P³: тензор Римана описывает кривизну проективного пространства.
/// На S³ (единичная сфера в R⁴): R^i_{jkl} = δ^i_k g_{jl} - δ^i_l g_{jk}
pub const RiemannTensor = struct {
    /// R^i_{jkl} — data[i][j][k][l]
    data: [DIM][DIM][DIM][DIM]f32,

    pub fn zero() RiemannTensor {
        return .{ .data = .{.{.{.{0} ** DIM} ** DIM} ** DIM} ** DIM };
    }

    /// Доступ к компоненте.
    pub fn get(self: RiemannTensor, i: usize, j: usize, k: usize, l: usize) f32 {
        return self.data[i][j][k][l];
    }

    /// Тензор Риччи: R_{jl} = R^i_{jil} (свёртка по первому и третьему)
    pub fn ricci(self: RiemannTensor) MetricTensor {
        var components = mat4Zero();
        for (0..DIM) |j| {
            for (0..DIM) |l| {
                var sum: f32 = 0;
                for (0..DIM) |i| {
                    sum += self.data[i][j][i][l];
                }
                components[j * DIM + l] = sum;
            }
        }
        return .{ .components = components };
    }

    /// Скалярная кривизна: R = g^{jl} R_{jl}
    pub fn scalarCurvature(self: RiemannTensor, g: MetricTensor) ?f32 {
        const ricci_tensor = self.ricci();
        const g_inv = g.inverse() orelse return null;
        var result: f32 = 0;
        for (0..DIM) |j| {
            for (0..DIM) |l| {
                result += g_inv.components[j * DIM + l] * ricci_tensor.components[j * DIM + l];
            }
        }
        return result;
    }

    /// Тензор Римана для пространства постоянной кривизны K:
    /// R^i_{jkl} = K · (δ^i_k g_{jl} - δ^i_l g_{jk})
    pub fn constantCurvature(g: MetricTensor, K: f32) RiemannTensor {
        var result = zero();
        for (0..DIM) |i| {
            for (0..DIM) |j| {
                for (0..DIM) |k| {
                    for (0..DIM) |l| {
                        const delta_ik: f32 = if (i == k) 1.0 else 0.0;
                        const delta_il: f32 = if (i == l) 1.0 else 0.0;
                        const g_jl = g.components[j * DIM + l];
                        const g_jk = g.components[j * DIM + k];
                        result.data[i][j][k][l] = K * (delta_ik * g_jl - delta_il * g_jk);
                    }
                }
            }
        }
        return result;
    }
};

// =============================================================================
// 8. PULLBACK / PUSHFORWARD — действие PGL(4) на тензоры
// =============================================================================

/// Pullback тензора (0,2) под отображением f:
/// (f* g)_{ij} = g_{kl} · (∂x^k/∂y^i) · (∂x^l/∂y^j)
///
/// Для линейного отображения A: (f* g) = A^T · g · A
pub fn pullbackMetric(g: MetricTensor, a: Mat4x4) MetricTensor {
    const at = mat4Transpose(a);
    const at_g = mat4Mul(at, g.components);
    const result = mat4Mul(at_g, a);
    return .{ .components = result };
}

/// Pushforward вектора: f_* v = A · v
pub fn pushforwardVec(a: Mat4x4, v: Vec4) Vec4 {
    return mat4MulVec(a, v);
}

/// Pullback ковектора: f* w = w · A^{-1}
pub fn pullbackCovec(w: Covec4, a: Mat4x4) ?Covec4 {
    const a_inv = mat4Inverse(a) orelse return null;
    return cov4MulMat(w, a_inv);
}

// =============================================================================
// 9. ТЕНЗОР ЭНЕРГИИ-ИМПУЛЬСА — ранг (0,2), симметричный
// =============================================================================

/// Тензор энергии-импульса T_{μν}.
///
/// В контексте P³:
///   - T_00 = плотность энергии
///   - T_0i = плотность импульса
///   - T_ij = тензор напряжений
///
/// Сохранение: ∇_μ T^{μν} = 0
pub const StressEnergy = struct {
    components: Mat4x4,

    pub fn zero() StressEnergy {
        return .{ .components = mat4Zero() };
    }

    /// Идеальная жидкость: T_{μν} = (ρ+p)u_μ u_ν + p g_{μν}
    pub fn perfectFluid(rho: f32, pressure: f32, u: Vec4, g: MetricTensor) StressEnergy {
        var components = mat4Zero();
        const u_cov = g.lower(u);
        for (0..DIM) |mu| {
            for (0..DIM) |nu| {
                components[mu * DIM + nu] = (rho + pressure) * u_cov[mu] * u_cov[nu] + pressure * g.components[mu * DIM + nu];
            }
        }
        return .{ .components = components };
    }

    /// След: T = g^{μν} T_{μν}
    pub fn trace(self: StressEnergy, g: MetricTensor) ?f32 {
        const g_inv = g.inverse() orelse return null;
        var result: f32 = 0;
        for (0..DIM) |mu| {
            for (0..DIM) |nu| {
                result += g_inv.components[mu * DIM + nu] * self.components[mu * DIM + nu];
            }
        }
        return result;
    }
};

// =============================================================================
// 10. LEVI-CIVITA SYMBOL — полностью антисимметричный
// =============================================================================

/// Символ Леви-Чивиты ε_{ijkl} в 4D.
/// ε = 0 если есть повторяющиеся индексы
/// ε = +1 для чётной перестановки (0,1,2,3)
/// ε = -1 для нечётной перестановки
pub fn leviCivita4(i: usize, j: usize, k: usize, l: usize) i32 {
    // Проверка на повторяющиеся индексы
    if (i == j or i == k or i == l or j == k or j == l or k == l) return 0;

    // Подсчёт инверсий перестановки (i,j,k,l)
    const perm = [_]usize{ i, j, k, l };
    var inversions: i32 = 0;
    for (0..4) |a| {
        for (a + 1..4) |b| {
            if (perm[a] > perm[b]) inversions += 1;
        }
    }

    return if (@mod(inversions, 2) == 0) 1 else -1;
}

// =============================================================================
// 11. ТЕСТЫ
// =============================================================================

test "Vec4: add and scale" {
    const a = vec4(1, 2, 3, 4);
    const b = vec4(5, 6, 7, 8);
    const c = vec4Add(a, b);
    try std.testing.expect(c[0] == 6 and c[1] == 8 and c[2] == 10 and c[3] == 12);

    const d = vec4Scale(a, 2.0);
    try std.testing.expect(d[0] == 2 and d[1] == 4 and d[2] == 6 and d[3] == 8);
}

test "Vec4: dot product" {
    const a = vec4(1, 0, 0, 0);
    const b = vec4(0, 1, 0, 0);
    try std.testing.expect(vec4Dot(a, b) == 0);
    try std.testing.expect(vec4Dot(a, a) == 1);
}

test "Vec4: wedge product antisymmetry" {
    const a = vec4(1, 0, 0, 0);
    const b = vec4(0, 1, 0, 0);
    const ab = vec4Wedge(a, b);
    const ba = vec4Wedge(b, a);
    // a ∧ b = -(b ∧ a)
    try std.testing.expect(ab[0] == -ba[0]);
    try std.testing.expect(ab[0] == 1); // e01 component
}

test "Mat4x4: identity multiplication" {
    const id = mat4Identity();
    const a = mat4Set(mat4Zero(), 0, 1, 3.0);
    const result = mat4Mul(id, a);
    try std.testing.expect(mat4Get(result, 0, 1) == 3.0);
}

test "Mat4x4: determinant" {
    const id = mat4Identity();
    try std.testing.expect(mat4Det(id) == 1.0);

    // Scaling matrix: diag(2,2,2,2) → det = 16
    var scaled = mat4Zero();
    for (0..4) |i| scaled[i * 4 + i] = 2.0;
    try std.testing.expect(mat4Det(scaled) == 16.0);
}

test "Mat4x4: inverse" {
    const a = .{
        2, 1, 0, 0,
        1, 2, 1, 0,
        0, 1, 2, 1,
        0, 0, 1, 2,
    };
    const inv = mat4Inverse(a) orelse unreachable;
    const product = mat4Mul(a, inv);
    // Should be identity
    for (0..16) |i| {
        const expected: f32 = if (i % 5 == 0) 1.0 else 0.0;
        try std.testing.expectApproxEqAbs(product[i], expected, 1e-4);
    }
}

test "Metric: euclidean inner product" {
    const g = MetricTensor.euclidean();
    const v = vec4(1, 2, 3, 4);
    try std.testing.expect(g.inner(v, v) == 30.0); // 1+4+9+16
}

test "Metric: lorentzian norm" {
    const g = MetricTensor.lorentzian();
    const v = vec4(1, 0, 0, 0); // time-like
    try std.testing.expect(g.normSq(v) == -1.0);

    const w = vec4(0, 1, 0, 0); // space-like
    try std.testing.expect(g.normSq(w) == 1.0);
}

test "Metric: raise and lower" {
    const g = MetricTensor.euclidean();
    const v = vec4(1, 2, 3, 4);
    const v_low = g.lower(v);
    // Euclidean: lowering doesn't change
    for (0..4) |i| {
        try std.testing.expect(v_low[i] == v[i]);
    }
}

test "Pullback: preserves metric type" {
    const g = MetricTensor.euclidean();
    // Orthogonal matrix (rotation by 90° in xy-plane)
    const rot = .{
        0, 1, 0, 0,
        -1, 0, 0, 0,
        0, 0, 1, 0,
        0, 0, 0, 1,
    };
    const g_pull = pullbackMetric(g, rot);
    // Pullback of euclidean metric under rotation = euclidean
    for (0..16) |i| {
        const expected: f32 = if (i % 5 == 0) 1.0 else 0.0;
        try std.testing.expectApproxEqAbs(g_pull.components[i], expected, 1e-4);
    }
}

test "Riemann: constant curvature sphere" {
    const g = MetricTensor.euclidean();
    const R = RiemannTensor.constantCurvature(g, 1.0);
    // On S³ with K=1: R^0_{101} = g_{11} = 1
    try std.testing.expect(R.get(0, 1, 0, 1) == 1.0);
    // R^0_{110} = -g_{11} = -1 (antisymmetry)
    try std.testing.expect(R.get(0, 1, 1, 0) == -1.0);
}

test "Riemann: scalar curvature of S³" {
    const g = MetricTensor.euclidean();
    const R = RiemannTensor.constantCurvature(g, 1.0);
    const sc = R.scalarCurvature(g) orelse unreachable;
    // S³ with K=1: scalar curvature = K·n·(n-1) = 1·4·3 = 12
    try std.testing.expectApproxEqAbs(sc, 12.0, 1e-3);
}

test "Levi-Civita: basic values" {
    try std.testing.expect(leviCivita4(0, 1, 2, 3) == 1);
    try std.testing.expect(leviCivita4(1, 0, 2, 3) == -1);
    try std.testing.expect(leviCivita4(0, 0, 2, 3) == 0);
}

test "Christoffel: zero for constant metric" {
    const g = MetricTensor.euclidean();
    const gamma = ChristoffelSymbols.fromConstantMetric(g);
    try std.testing.expect(gamma.get(0, 0, 0) == 0);
    try std.testing.expect(gamma.get(1, 2, 3) == 0);
}

test "StressEnergy: perfect fluid trace" {
    const g = MetricTensor.lorentzian();
    const u = vec4(1, 0, 0, 0); // Rest frame
    const T = StressEnergy.perfectFluid(1.0, 0.5, u, g);
    // Trace = T^μ_μ = -ρ + 3p for Lorentzian
    const tr = T.trace(g) orelse unreachable;
    try std.testing.expectApproxEqAbs(tr, -1.0 + 3.0 * 0.5, 1e-3);
}

test "Mat4x4: transpose" {
    const a = mat4Set(mat4Set(mat4Zero(), 0, 1, 3.0), 2, 0, 5.0);
    const at = mat4Transpose(a);
    try std.testing.expect(mat4Get(at, 1, 0) == 3.0);
    try std.testing.expect(mat4Get(at, 0, 2) == 5.0);
}
