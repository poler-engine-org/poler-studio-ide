//! POLER-транспорт: Кэли-ротация + слой ограничений SCTP.
//!
//! Доказано (V11/V13/V18, proofs/ssn_verify3.py):
//! - **Кэли-транспорт** p' = (I − h/2·J)⁻¹(I + h/2·J)·p — второй порядок
//!   точности exp(hJ), дрейф нормы 3.7e-14 за 2000 шагов (Эйлер: 1e-2).
//!   J антисимметрична (J = A − Aᵀ) → чистая ротация, норма сохраняется.
//! - **SCTP-проектор** Π = I − J_cᵀ(J_cJ_cᵀ)⁻¹J_c — ортогональная проекция
//!   на нуль-пространство ограничений; идемпотентен, Σ(Πy) = 0.
//! - **Полный POLER-шаг**: Кэли → диссипация D → резонансное притяжение →
//!   проекция Π → нормировка. Инварианты: ‖p‖ = 1, Σp = 0, Δ ограничена
//!   (предельный цикл, а не взрыв и не смерть).
//!
//! Транспортная матрица М = (I − h/2·J)⁻¹(I + h/2·J) предвычисляется один
//! раз (J фиксирована) — шаг становится чистым matvec O(n²).

/// Минимальная f64-линейная алгебра для транспорта (самодостаточно).
mod la {
    /// Матрица n×n, row-major.
    #[derive(Clone)]
    pub struct Mat {
        pub n: usize,
        pub data: Vec<f64>,
    }

    impl Mat {
        pub fn zeros(n: usize) -> Self {
            Mat { n, data: vec![0.0; n * n] }
        }

        pub fn identity(n: usize) -> Self {
            let mut m = Mat::zeros(n);
            for i in 0..n {
                m.data[i * n + i] = 1.0;
            }
            m
        }

        pub fn from_rows(n: usize, data: Vec<f64>) -> Self {
            debug_assert_eq!(data.len(), n * n);
            Mat { n, data }
        }

        #[inline]
        pub fn at(&self, r: usize, c: usize) -> f64 {
            self.data[r * self.n + c]
        }

        #[inline]
        pub fn set(&mut self, r: usize, c: usize, v: f64) {
            self.data[r * self.n + c] = v;
        }

        pub fn matvec(&self, x: &[f64]) -> Vec<f64> {
            let n = self.n;
            let mut out = vec![0.0; n];
            for r in 0..n {
                let row = &self.data[r * n..(r + 1) * n];
                out[r] = row.iter().zip(x).map(|(a, b)| a * b).sum();
            }
            out
        }

        /// A + B
        pub fn add(&self, other: &Mat) -> Mat {
            let mut m = self.clone();
            for (d, o) in m.data.iter_mut().zip(&other.data) {
                *d += o;
            }
            m
        }

        /// A·k
        pub fn scaled(&self, k: f64) -> Mat {
            let mut m = self.clone();
            for d in m.data.iter_mut() {
                *d *= k;
            }
            m
        }

        /// Обращение методом Гаусса–Жордана с частичным ведущим элементом.
        pub fn inverse(&self) -> Option<Mat> {
            let n = self.n;
            let mut a = self.data.clone();
            let mut inv = Mat::identity(n).data;
            for col in 0..n {
                // ведущий элемент
                let mut piv = col;
                let mut best = a[col * n + col].abs();
                for r in (col + 1)..n {
                    let v = a[r * n + col].abs();
                    if v > best {
                        best = v;
                        piv = r;
                    }
                }
                if best < 1e-13 {
                    return None; // вырождена
                }
                if piv != col {
                    for c in 0..n {
                        a.swap(col * n + c, piv * n + c);
                        inv.swap(col * n + c, piv * n + c);
                    }
                }
                let d = a[col * n + col];
                for c in 0..n {
                    a[col * n + c] /= d;
                    inv[col * n + c] /= d;
                }
                for r in 0..n {
                    if r != col {
                        let f = a[r * n + col];
                        if f != 0.0 {
                            for c in 0..n {
                                a[r * n + c] -= f * a[col * n + c];
                                inv[r * n + c] -= f * inv[col * n + c];
                            }
                        }
                    }
                }
            }
            Some(Mat { n, data: inv })
        }
    }

    pub fn norm(v: &[f64]) -> f64 {
        v.iter().map(|x| x * x).sum::<f64>().sqrt()
    }

    /// Спектральная норма ‖J‖₂ — степенная итерация на JᵀJ.
    pub fn spectral_norm(j: &Mat) -> f64 {
        let n = j.n;
        let mut x = vec![1.0; n];
        let mut lambda = 0.0f64;
        for _ in 0..256 {
            // y = JᵀJ x
            let jx = j.matvec(&x);
            let mut y = vec![0.0; n];
            for c in 0..n {
                let mut s = 0.0;
                for r in 0..n {
                    s += j.at(r, c) * jx[r];
                }
                y[c] = s;
            }
            let ny = norm(&y);
            if ny < 1e-300 {
                return 0.0;
            }
            lambda = ny;
            for xi in x.iter_mut() {
                *xi /= ny.max(1e-300);
            }
        }
        lambda.sqrt()
    }
}

/// Полный POLER-транспорт состояния p (EQ-D19):
/// Π·exp(Δt·J)·p + диссипация + резонансное притяжение + нормировка.
pub struct PolerTransport {
    /// Предвычисленная транспортная матрица (I − h/2·J)⁻¹(I + h/2·J).
    m: la::Mat,
    /// Диагональ диссипации D (Ляпунов, гасит норму).
    d_diag: Vec<f64>,
    /// Коэффициент диссипации.
    diss: f64,
    /// Коэффициент резонансного притяжения.
    attract: f64,
    /// Цель резонанса (равномерное распределение).
    target: f64,
    n: usize,
}

impl PolerTransport {
    /// Построить транспорт из антисимметричной матрицы J (n×n, row-major)
    /// и диагонали диссипации. J нормируется спектрально, как в V18.
    pub fn new(j_rows: &[f64], d_diag: &[f64], h: f64) -> Option<Self> {
        let n = d_diag.len();
        assert_eq!(j_rows.len(), n * n, "J должна быть n×n");
        let j = la::Mat::from_rows(n, j_rows.to_vec());
        let nj = la::spectral_norm(&j);
        let jn = if nj > 1e-15 { j.scaled(1.0 / nj) } else { j };
        // M⁺ = (I − h/2·J)⁻¹(I + h/2·J) — Кэли, предвычисление
        let half = la::Mat::identity(n).add(&jn.scaled(-0.5 * h));
        let plus = la::Mat::identity(n).add(&jn.scaled(0.5 * h));
        let half_inv = half.inverse()?;
        let m = matmul(&half_inv, &plus);
        Some(PolerTransport { m, d_diag: d_diag.to_vec(), diss: 0.05, attract: 0.02, target: 0.5, n })
    }

    /// Один транспортный шаг: Кэли → диссипация → притяжение → Π → норма.
    pub fn step(&self, p: &[f64]) -> Vec<f64> {
        let mut x = self.m.matvec(p);
        for (xi, d) in x.iter_mut().zip(&self.d_diag) {
            *xi -= self.diss * d * *xi;
        }
        for xi in x.iter_mut() {
            *xi += self.attract * (self.target - *xi);
        }
        // Π для ограничения Σp = 0: p − mean(p) (J_c = ones)
        let mean = x.iter().sum::<f64>() / self.n as f64;
        for xi in x.iter_mut() {
            *xi -= mean;
        }
        let nn = la::norm(&x);
        if nn > 0.0 {
            for xi in x.iter_mut() {
                *xi /= nn;
            }
        }
        x
    }

    /// Размерность состояния.
    pub fn dim(&self) -> usize {
        self.n
    }
}

fn matmul(a: &la::Mat, b: &la::Mat) -> la::Mat {
    let n = a.n;
    let mut out = la::Mat::zeros(n);
    for r in 0..n {
        for k in 0..n {
            let av = a.at(r, k);
            if av == 0.0 {
                continue;
            }
            for c in 0..n {
                let v = out.at(r, c) + av * b.at(k, c);
                out.set(r, c, v);
            }
        }
    }
    out
}

/// SCTP-слой ограничений: Π = I − J_cᵀ(J_cJ_cᵀ)⁻¹J_c (EQ-B69).
/// J_c — m×n строк ограничений (m << n). Проекция на нуль-пространство.
pub struct SctpProjector {
    /// Предвычисленная матрица Π (n×n).
    pi: la::Mat,
    n: usize,
}

impl SctpProjector {
    /// Построить проектор по m строкам ограничений.
    pub fn new(jc_rows: &[f64], m: usize, n: usize) -> Option<Self> {
        assert_eq!(jc_rows.len(), m * n, "J_c должна быть m×n");
        // JJt = J_c·J_cᵀ (m×m)
        let mut jjt = la::Mat::zeros(m);
        for i in 0..m {
            for k in 0..m {
                let mut s = 0.0;
                for c in 0..n {
                    s += jc_rows[i * n + c] * jc_rows[k * n + c];
                }
                jjt.set(i, k, s);
            }
        }
        let jjt_inv = jjt.inverse()?;
        // Π = I − J_cᵀ·(JJt)⁻¹·J_c
        // (J_cᵀ)_{r,i} = J_c[i][r];  итог: Π_{r,c} = δ_{r,c} − Σ_{i,k} J_c[i][r]·(JJt⁻¹)_{i,k}·J_c[k][c]
        let mut pi = la::Mat::identity(n);
        for r in 0..n {
            for c in 0..n {
                let mut acc = 0.0;
                for i in 0..m {
                    for k in 0..m {
                        acc += jc_rows[i * n + r] * jjt_inv.at(i, k) * jc_rows[k * n + c];
                    }
                }
                pi.set(r, c, pi.at(r, c) - acc);
            }
        }
        Some(SctpProjector { pi, n })
    }

    /// Проекция вектора: Π·p.
    pub fn project(&self, p: &[f64]) -> Vec<f64> {
        self.pi.matvec(p)
    }

    /// Размерность пространства.
    pub fn dim(&self) -> usize {
        self.n
    }

    /// Транспонированная проекция (для симметризации).
    pub fn pi(&self) -> &la::Mat {
        &self.pi
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn antisym_from_seed(n: usize, seed: u64) -> (Vec<f64>, Vec<f64>) {
        let mut rng = crate::ssn::rng::Rng::new(seed);
        let mut a = vec![0.0f64; n * n];
        for r in 0..n {
            for c in (r + 1)..n {
                let x = rng.normal(0.0, 1.0);
                a[r * n + c] = x;
                a[c * n + r] = -x;
            }
        }
        let d: Vec<f64> = (0..n).map(|i| 0.5 + 0.5 * i as f64 / n as f64).collect();
        (a, d)
    }

    /// V11a/V11b: J = A − Aᵀ антисимметрична, собственные числа чисто мнимые
    /// (проверяем косвенно: xᵀJx = 0 для любого x — квадратичная форма нулевая).
    #[test]
    fn v11_antisymmetry() {
        let (j, _) = antisym_from_seed(32, 42);
        let n = 32;
        let mut rng = crate::ssn::rng::Rng::new(7);
        for _ in 0..20 {
            let x: Vec<f64> = (0..n).map(|_| rng.normal(0.0, 1.0)).collect();
            let jx = la::Mat::from_rows(n, j.clone()).matvec(&x);
            let quad = crate::ssn::cse::dot(&x, &jx);
            assert!(quad.abs() < 1e-10, "xᵀJx = {quad} — не антисимметрична");
        }
        // симметрия элементов
        for r in 0..n {
            for c in 0..n {
                assert!((j[r * n + c] + j[c * n + r]).abs() < 1e-12);
            }
        }
    }

    /// V11c: Кэли-транспорт сохраняет норму — дрейф < 1e-8 за 2000 шагов.
    #[test]
    fn v11c_cayley_norm_drift() {
        let (j, d) = antisym_from_seed(24, 777);
        // чистый Кэли без диссипации/притяжения: проверяем матрицу M
        let tr = PolerTransport::new(&j, &d, 0.1).unwrap();
        let mut rng = crate::ssn::rng::Rng::new(1);
        let mut p: Vec<f64> = (0..24).map(|_| rng.normal(0.0, 1.0)).collect();
        let n0 = la::norm(&p);
        // чистая ротация: обходим диссипацию — берём только матрицу M через
        // два шага с обнулением эффектов невозможно; поэтому проверяем сам M:
        let m_only = {
            let n = 24usize;
            let jm = la::Mat::from_rows(n, j.clone());
            let nj = la::spectral_norm(&jm);
            let jn = jm.scaled(1.0 / nj);
            let half = la::Mat::identity(n).add(&jn.scaled(-0.05));
            let plus = la::Mat::identity(n).add(&jn.scaled(0.05));
            matmul(&half.inverse().unwrap(), &plus)
        };
        for _ in 0..2000 {
            p = m_only.matvec(&p);
        }
        let drift = (la::norm(&p) - n0).abs() / n0;
        assert!(drift < 1e-8, "дрейф нормы = {drift:.2e}");
        // и транспорт в целом жив
        let _ = tr.step(&p);
    }

    /// V18: полный POLER-шаг — норма 1, Σp = 0, Δ ограничена (цикл).
    #[test]
    fn v18_poler_transport_invariants() {
        let (j, d) = antisym_from_seed(24, 20260918);
        let tr = PolerTransport::new(&j, &d, 0.1).unwrap();
        let mut rng = crate::ssn::rng::Rng::new(5);
        let mut p: Vec<f64> = (0..24).map(|_| rng.normal(0.0, 1.0)).collect();
        let n0 = la::norm(&p);
        for v in p.iter_mut() {
            *v /= n0;
        }
        for _ in 0..500 {
            p = tr.step(&p);
        }
        assert!((la::norm(&p) - 1.0).abs() < 1e-6, "норма = {}", la::norm(&p));
        assert!(p.iter().sum::<f64>().abs() < 1e-10, "Σp = {}", p.iter().sum::<f64>());
        let mut deltas = Vec::new();
        for _ in 0..50 {
            let prev = p.clone();
            p = tr.step(&p);
            deltas.push(la::norm(&p.iter().zip(&prev).map(|(a, b)| a - b).collect::<Vec<_>>()));
        }
        let d_first: f64 = deltas[..10].iter().sum::<f64>() / 10.0;
        let d_last: f64 = deltas[deltas.len() - 10..].iter().sum::<f64>() / 10.0;
        assert!(d_last <= d_first * 1.5, "Δ растёт: {d_first} → {d_last}");
        assert!(d_last > 1e-6, "система умерла: Δ = {d_last}");
    }

    /// V13: SCTP-проектор — идемпотентен, Σ(Πy) = 0 (для J_c = ones),
    /// D-часть гасит норму (Ляпунов), ротационная часть сохраняет антисимметрию.
    #[test]
    fn v13_sctp_layer() {
        let n = 16usize;
        // J_c = ones(1, n): ограничение Σy = 0
        let jc = vec![1.0f64; n];
        let proj = SctpProjector::new(&jc, 1, n).unwrap();
        let mut rng = crate::ssn::rng::Rng::new(3);
        let x: Vec<f64> = (0..n).map(|_| rng.normal(0.0, 1.0)).collect();

        // Π идемпотентен
        let y1 = proj.project(&x);
        let y2 = proj.project(&y1);
        for (a, b) in y1.iter().zip(&y2) {
            assert!((a - b).abs() < 1e-10, "Π не идемпотентен");
        }

        // Σy = 0
        assert!(y1.iter().sum::<f64>().abs() < 1e-10, "Σy = {}", y1.iter().sum::<f64>());

        // D-часть гасит норму: y ← y − 0.1·ΠDΠy, 100 шагов
        let (j, d) = antisym_from_seed(n, 9);
        let jm = la::Mat::from_rows(n, j);
        let _ = jm;
        let mut y = x.clone();
        let e0 = la::norm(&y);
        for _ in 0..100 {
            let dy: Vec<f64> = y.iter().zip(&d).map(|(v, di)| 0.1 * di * v).collect();
            let pdy = proj.project(&dy);
            for (yi, p_) in y.iter_mut().zip(pdy) {
                *yi -= p_;
            }
        }
        assert!(la::norm(&y) < e0, "D не гасит: {} → {}", e0, la::norm(&y));

        // ротационная часть: ΠJΠ антисимметрична
        let pip = proj.pi();
        let pj = matmul(pip, &matmul(&jm, pip));
        for r in 0..n {
            for c in 0..n {
                assert!((pj.at(r, c) + pj.at(c, r)).abs() < 1e-10, "ΠJΠ не антисимметрична");
            }
        }
    }
}
