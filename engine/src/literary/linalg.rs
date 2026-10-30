//! Минимальная плотная линейная алгебра f32 для Литературного Двигателя.
//!
//! Ноль внешних зависимостей (философия репо): матрицы row-major f32,
//! разреженность не нужна — фазовое пространство ≤ 256 осей, роторный
//! подтекст касты ≤ 64 нейронов. Точные решатели (Холецкого, Гаусса—
//! Жордана) работают во внутренних f64 и конвертируют результат назад:
//! устойчивость псевдообращения проектора важнее тактовой экономии.
//!
//! - [`Mat::matvec`] / [`Mat::matmul`] — прямые произведения;
//! - [`cholesky_factor`] — разложение SPD-матрицы (нижний треугольник L
//!   для стабилизатора Ляпунова D = L·Lᵀ);
//! - [`gauss_jordan_inverse`] — обращение с частичным ведущим элементом
//!   (для (J_cJ_cᵀ)⁻¹ проектора причинности).

/// Плотная матрица f32, row-major: `data[r * cols + c]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Mat {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f32>,
}

impl Mat {
    /// Нулевая матрица.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![0.0; rows * cols],
        }
    }

    /// Единичная (только квадратные).
    pub fn identity(n: usize) -> Self {
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m.data[i * n + i] = 1.0;
        }
        m
    }

    /// Диагональная из вектора.
    pub fn diagonal(d: &[f32]) -> Self {
        let n = d.len();
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m.data[i * n + i] = d[i];
        }
        m
    }

    /// Владение сырыми данными (проверка размеров — debug_assert).
    pub fn from_raw(rows: usize, cols: usize, data: Vec<f32>) -> Self {
        debug_assert_eq!(rows * cols, data.len());
        Self { rows, cols, data }
    }

    #[inline]
    pub fn at(&self, r: usize, c: usize) -> f32 {
        self.data[r * self.cols + c]
    }

    #[inline]
    pub fn set(&mut self, r: usize, c: usize, v: f32) {
        self.data[r * self.cols + c] = v;
    }

    /// Строка как срез.
    pub fn row(&self, r: usize) -> &[f32] {
        &self.data[r * self.cols..(r + 1) * self.cols]
    }

    /// Транспонирование.
    pub fn transpose(&self) -> Self {
        let mut t = Self::zeros(self.cols, self.rows);
        for r in 0..self.rows {
            for c in 0..self.cols {
                t.data[c * self.rows + r] = self.data[r * self.cols + c];
            }
        }
        t
    }

    /// Умножение на вектор: A·x.
    pub fn matvec(&self, x: &[f32]) -> Vec<f32> {
        debug_assert_eq!(self.cols, x.len());
        let mut out = vec![0.0f32; self.rows];
        for r in 0..self.rows {
            let row = self.row(r);
            let mut acc = 0.0f32;
            for (a, b) in row.iter().zip(x) {
                acc += a * b;
            }
            out[r] = acc;
        }
        out
    }

    /// Произведение матриц: self·other.
    pub fn matmul(&self, other: &Mat) -> Mat {
        debug_assert_eq!(self.cols, other.rows);
        let mut out = Mat::zeros(self.rows, other.cols);
        let t = other.transpose();
        for r in 0..self.rows {
            let a_row = self.row(r);
            for c in 0..other.cols {
                let b_row = t.row(c);
                let mut acc = 0.0f32;
                for (&a, &b) in a_row.iter().zip(b_row) {
                    acc += a * b;
                }
                out.set(r, c, acc);
            }
        }
        out
    }

    /// Поэлементное сложение (self + other).
    pub fn add(&self, other: &Mat) -> Mat {
        debug_assert_eq!((self.rows, self.cols), (other.rows, other.cols));
        let data = self
            .data
            .iter()
            .zip(&other.data)
            .map(|(&a, &b)| a + b)
            .collect();
        Mat::from_raw(self.rows, self.cols, data)
    }

    /// Масштабирование на скаляр.
    pub fn scaled(&self, k: f32) -> Mat {
        Mat::from_raw(self.rows, self.cols, self.data.iter().map(|&v| v * k).collect())
    }

    /// Максимум |элемента| (диагностика сходимости).
    pub fn abs_max(&self) -> f32 {
        self.data.iter().fold(0.0f32, |m, &v| m.max(v.abs()))
    }

    /// Норма Фробениуса.
    pub fn frobenius(&self) -> f32 {
        self.data.iter().map(|v| v * v).sum::<f32>().sqrt()
    }
}

/// Нижнее треугольное разложение Холецкого SPD-матрицы A = L·Lᵀ.
/// Внутренние вычисления в f64. Ошибка — матрица не SPD (или вырождена).
pub fn cholesky_factor(a: &Mat) -> Result<Mat, String> {
    if a.rows != a.cols {
        return Err(format!("Холецкий: матрица {}×{} не квадратная", a.rows, a.cols));
    }
    let n = a.rows;
    let mut l = vec![0.0f64; n * n];
    for r in 0..n {
        for c in 0..=r {
            let mut sum = a.at(r, c) as f64;
            for k in 0..c {
                sum -= l[r * n + k] * l[c * n + k];
            }
            if r == c {
                if sum <= 1e-12 {
                    return Err(format!(
                        "Холецкий: ведущий элемент {sum:.3e} ≤ 0 — матрица не SPD"
                    ));
                }
                l[r * n + c] = sum.sqrt();
            } else {
                l[r * n + c] = sum / l[c * n + c];
            }
        }
    }
    Ok(Mat::from_raw(n, n, l.iter().map(|&v| v as f32).collect()))
}

/// Решение SPD-системы A·x = b через разложение Холецкого.
pub fn cholesky_solve(a: &Mat, b: &[f32]) -> Result<Vec<f32>, String> {
    let l = cholesky_factor(a)?;
    let n = l.rows;
    debug_assert_eq!(b.len(), n);
    // Ly = b (прямая подстановка)
    let mut y = vec![0.0f64; n];
    for r in 0..n {
        let mut sum = b[r] as f64;
        for k in 0..r {
            sum -= l.at(r, k) as f64 * y[k];
        }
        y[r] = sum / l.at(r, r) as f64;
    }
    // Lᵀx = y (обратная подстановка)
    let mut x = vec![0.0f64; n];
    for r in (0..n).rev() {
        let mut sum = y[r];
        for k in (r + 1)..n {
            sum -= l.at(k, r) as f64 * x[k];
        }
        x[r] = sum / l.at(r, r) as f64;
    }
    Ok(x.iter().map(|&v| v as f32).collect())
}

/// Обращение квадратной матрицы методом Гаусса—Жордана с частичным
/// ведущим элементом. Для (J_cJ_cᵀ)⁻¹ проектора: маленькие плотные
/// матрицы ≤ 32×32, устойчивость важнее скорости. Провал — вырожденность.
pub fn gauss_jordan_inverse(a: &Mat) -> Result<Mat, String> {
    if a.rows != a.cols {
        return Err(format!(
            "обращение: матрица {}×{} не квадратная",
            a.rows, a.cols
        ));
    }
    let n = a.rows;
    // Расширенная [A | I] во внутренних f64.
    let mut m = vec![vec![0.0f64; 2 * n]; n];
    for r in 0..n {
        for c in 0..n {
            m[r][c] = a.at(r, c) as f64;
        }
        m[r][n + r] = 1.0;
    }
    for col in 0..n {
        // Частичный ведущий: максимальный |элемент| в столбце.
        let mut piv = col;
        let mut best = m[col][col].abs();
        for r in (col + 1)..n {
            let v = m[r][col].abs();
            if v > best {
                best = v;
                piv = r;
            }
        }
        if best < 1e-12 {
            return Err(format!(
                "обращение: столбец {col} вырожден (ведущий {best:.3e})"
            ));
        }
        m.swap(col, piv);
        let inv = 1.0 / m[col][col];
        for c in col..2 * n {
            m[col][c] *= inv;
        }
        for r in 0..n {
            if r == col {
                continue;
            }
            let f = m[r][col];
            if f == 0.0 {
                continue;
            }
            for c in col..2 * n {
                m[r][c] -= f * m[col][c];
            }
        }
    }
    let mut out = Mat::zeros(n, n);
    for r in 0..n {
        for c in 0..n {
            out.set(r, c, m[r][n + c] as f32);
        }
    }
    Ok(out)
}

// ── Векторные помощники ──────────────────────────────────────────────

/// Скалярное произведение.
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(&x, &y)| x * y).sum()
}

/// Евклидова норма.
pub fn norm2(a: &[f32]) -> f32 {
    dot(a, a).sqrt()
}

/// y += α·x (на месте).
pub fn axpy(y: &mut [f32], alpha: f32, x: &[f32]) {
    debug_assert_eq!(y.len(), x.len());
    for (yv, &xv) in y.iter_mut().zip(x) {
        *yv += alpha * xv;
    }
}

/// Разность векторов.
pub fn sub(a: &[f32], b: &[f32]) -> Vec<f32> {
    debug_assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(&x, &y)| x - y).collect()
}

/// L2-нормализация (нулевой вектор остаётся нулевым).
pub fn normalize(a: &[f32]) -> Vec<f32> {
    let n = norm2(a);
    if n < 1e-12 {
        return vec![0.0; a.len()];
    }
    a.iter().map(|&v| v / n).collect()
}

/// Косинусная близость (0 при нулевом аргументе).
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let d = dot(a, b);
    let (na, nb) = (norm2(a), norm2(b));
    if na < 1e-12 || nb < 1e-12 {
        return 0.0;
    }
    d / (na * nb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn matvec_matmul_basics() {
        // A = [[1,2],[3,4]], x = [5,6] → A·x = [17, 39]
        let a = Mat::from_raw(2, 2, vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(a.matvec(&[5.0, 6.0]), vec![17.0, 39.0]);
        // A·Aᵀ = [[5,11],[11,25]]
        let aat = a.matmul(&a.transpose());
        assert_eq!(aat.data, vec![5.0, 11.0, 11.0, 25.0]);
        // тождество: I·x = x
        let i = Mat::identity(3);
        assert_eq!(i.matvec(&[1.5, -2.0, 3.0]), vec![1.5, -2.0, 3.0]);
    }

    #[test]
    fn cholesky_roundtrip_and_solve() {
        // SPD: [[4,2],[2,3]]
        let a = Mat::from_raw(2, 2, vec![4.0, 2.0, 2.0, 3.0]);
        let l = cholesky_factor(&a).unwrap();
        // L·Lᵀ = A
        let back = l.matmul(&l.transpose());
        for (x, y) in back.data.iter().zip(&a.data) {
            assert!(close(*x, *y, 1e-5));
        }
        // A·x = [2,1] → x = A⁻¹·[2,1]
        let x = cholesky_solve(&a, &[2.0, 1.0]).unwrap();
        let ax = a.matvec(&x);
        assert!(close(ax[0], 2.0, 1e-5) && close(ax[1], 1.0, 1e-5));
        // Не SPD → ошибка
        let bad = Mat::from_raw(2, 2, vec![1.0, 2.0, 2.0, 1.0]);
        assert!(cholesky_factor(&bad).is_err());
        // Неквадратная → ошибка
        let rect = Mat::zeros(2, 3);
        assert!(cholesky_factor(&rect).is_err());
    }

    #[test]
    fn gauss_jordan_inverse_roundtrip() {
        let a = Mat::from_raw(3, 3, vec![2.0, 1.0, 1.0, 1.0, 3.0, 2.0, 1.0, 0.0, 0.0]);
        let inv = gauss_jordan_inverse(&a).unwrap();
        let prod = a.matmul(&inv);
        for (i, &v) in prod.data.iter().enumerate() {
            let expect = if i % 4 == 0 { 1.0 } else { 0.0 };
            assert!(close(v, expect, 1e-4), "prod[{i}] = {v}");
        }
        // Вырожденная матрица → ошибка
        let sing = Mat::from_raw(2, 2, vec![1.0, 2.0, 2.0, 4.0]);
        assert!(gauss_jordan_inverse(&sing).is_err());
    }

    #[test]
    fn vector_helpers() {
        assert!(close(dot(&[1.0, 2.0], &[3.0, 4.0]), 11.0, 1e-6));
        assert!(close(norm2(&[3.0, 4.0]), 5.0, 1e-6));
        let mut y = vec![1.0, 1.0];
        axpy(&mut y, 2.0, &[1.0, -1.0]);
        assert_eq!(y, vec![3.0, -1.0]);
        assert_eq!(sub(&[5.0, 7.0], &[2.0, 3.0]), vec![3.0, 4.0]);
        assert!(close(norm2(&normalize(&[3.0, 4.0])), 1.0, 1e-6));
        assert_eq!(normalize(&[0.0, 0.0]), vec![0.0, 0.0]);
        assert!(close(cosine(&[1.0, 0.0], &[1.0, 0.0]), 1.0, 1e-6));
        assert!(close(cosine(&[1.0, 0.0], &[0.0, 1.0]), 0.0, 1e-6));
    }
}
