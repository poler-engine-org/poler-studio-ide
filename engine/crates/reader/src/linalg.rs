//! Мини-линейная алгебра 6×6 без внешних зависимостей.
//!
//! Точность — приоритет №1: экспонента кососимметричной матрицы обязана
//! быть ортогональной до ~1e-14 (инвариант нормы ротора, теорема I.1).
//! Поэтому expm использует агрессивный скейлинг (‖·‖ ≤ 1/8) и длинный
//! ряд Тейлора (16 членов), а возведение в квадрат — всего 3 шага.

pub const N: usize = 6;

pub type Mat = [[f64; N]; N];
pub type Vec6 = [f64; N];

/// Нулевая матрица.
pub fn zeros() -> Mat {
    [[0.0; N]; N]
}

/// Единичная матрица.
pub fn eye() -> Mat {
    let mut m = zeros();
    for i in 0..N {
        m[i][i] = 1.0;
    }
    m
}

/// C = A·B.
pub fn mat_mul(a: &Mat, b: &Mat) -> Mat {
    let mut c = zeros();
    for i in 0..N {
        for k in 0..N {
            let aik = a[i][k];
            if aik == 0.0 {
                continue;
            }
            for j in 0..N {
                c[i][j] += aik * b[k][j];
            }
        }
    }
    c
}

/// y = A·x.
pub fn mat_vec(a: &Mat, x: &Vec6) -> Vec6 {
    let mut y = [0.0; N];
    for i in 0..N {
        let mut s = 0.0;
        for j in 0..N {
            s += a[i][j] * x[j];
        }
        y[i] = s;
    }
    y
}

/// A += B.
pub fn mat_add_assign(a: &mut Mat, b: &Mat) {
    for i in 0..N {
        for j in 0..N {
            a[i][j] += b[i][j];
        }
    }
}

/// A += s·B.
pub fn mat_add_scaled_assign(a: &mut Mat, b: &Mat, s: f64) {
    for i in 0..N {
        for j in 0..N {
            a[i][j] += s * b[i][j];
        }
    }
}

/// A − Aᵀ (кососимметризация — ядро ротора J).
pub fn skew(a: &Mat) -> Mat {
    let mut j = zeros();
    for i in 0..N {
        for k in 0..N {
            j[i][k] = a[i][k] - a[k][i];
        }
    }
    j
}

/// ∞-норма матрицы.
pub fn norm_inf(m: &Mat) -> f64 {
    m.iter()
        .map(|row| row.iter().fold(0.0f64, |acc, &v| acc + v.abs()))
        .fold(0.0, f64::max)
}

/// Евклидова норма вектора.
pub fn vec_norm(x: &Vec6) -> f64 {
    x.iter().map(|&v| v * v).sum::<f64>().sqrt()
}

/// Решить A·x = b (гаусс с частичным выбором ведущего элемента).
/// `None` — вырожденная система.
pub fn solve(a: &Mat, b: &Vec6) -> Option<Vec6> {
    let mut m = *a;
    let mut x = *b;
    for col in 0..N {
        // выбор ведущего элемента
        let mut piv = col;
        let mut best = m[col][col].abs();
        for r in col + 1..N {
            let v = m[r][col].abs();
            if v > best {
                best = v;
                piv = r;
            }
        }
        if best < 1e-300 {
            return None;
        }
        if piv != col {
            m.swap(col, piv);
            x.swap(col, piv);
        }
        let d = m[col][col];
        for r in col + 1..N {
            let f = m[r][col] / d;
            if f == 0.0 {
                continue;
            }
            for c in col..N {
                m[r][c] -= f * m[col][c];
            }
            x[r] -= f * x[col];
        }
    }
    // обратный ход
    for r in (0..N).rev() {
        let mut s = x[r];
        for c in r + 1..N {
            s -= m[r][c] * x[c];
        }
        x[r] = s / m[r][r];
    }
    Some(x)
}

/// Экспонента матрицы: скейлинг-возведение в квадрат + ряд Тейлора.
///
/// Точность: масштабируем до ‖A·2^-s‖ ≤ 1/8, суммируем 16 членов ряда
/// (ошибка усечения ~ (1/8)^17/17! ≈ 1e-21), затем 3 возведения в квадрат.
/// Итоговая ошибка ~ несколько ε_маш — достаточно для инварианта
/// ортогональности expm(кососимметричная) на уровне 1e-14.
pub fn expm(a: &Mat) -> Mat {
    // s: минимальная степень двойки, дающая ‖A/2^s‖ ≤ 1/8
    let nrm = norm_inf(a);
    let mut s: u32 = 0;
    if nrm > 0.125 {
        s = (nrm / 0.125).log2().ceil() as u32;
    }
    let scale = 1.0 / (1u64 << s) as f64;
    let mut b = zeros();
    for i in 0..N {
        for j in 0..N {
            b[i][j] = a[i][j] * scale;
        }
    }
    // ряд Тейлора: E = I + B + B²/2! + ... + B¹⁶/16!
    // term хранит B^k/k! КУМУЛЯТИВНО (деление на k на каждом шаге) —
    // ошибка прошлой редакции (B^k/k) давала ортогональность лишь 2.5e-5.
    let mut term = eye();
    let mut e = eye();
    for k in 1..=16 {
        term = mat_mul(&term, &b);
        let kf = k as f64;
        for row in term.iter_mut() {
            for v in row.iter_mut() {
                *v /= kf;
            }
        }
        mat_add_assign(&mut e, &term);
    }
    // возведение в квадрат s раз
    for _ in 0..s {
        e = mat_mul(&e, &e);
    }
    e
}

/// Собственные значения симметричной матрицы (циклический метод Якоби).
/// Возвращает ОТСОРТИРОВАННЫЕ ПО УБЫВАНИЮ собственные значения.
pub fn eig_sym(m: &Mat) -> [f64; N] {
    // Работаем с полной симметричной матрицей N×N (у нас всегда 6×6).
    let mut a = *m;
    // симметризация на всякий случай (вход может иметь шум 1e-18)
    for i in 0..N {
        for j in 0..i {
            let v = 0.5 * (a[i][j] + a[j][i]);
            a[i][j] = v;
            a[j][i] = v;
        }
    }
    for sweep in 0..100 {
        // внедиагональная норма
        let mut off = 0.0;
        for i in 0..N {
            for j in 0..i {
                off += a[i][j] * a[i][j];
            }
        }
        if off < 1e-30 {
            break;
        }
        let _ = sweep;
        for p in 0..N {
            for q in (p + 1)..N {
                let apq = a[p][q];
                if apq.abs() < 1e-300 {
                    continue;
                }
                let theta = (a[q][q] - a[p][p]) / (2.0 * apq);
                // t = sign(θ)/(|θ|+√(θ²+1)) — устойчивая формула
                let t = if theta >= 0.0 {
                    1.0 / (theta + (theta * theta + 1.0).sqrt())
                } else {
                    -1.0 / (-theta + (theta * theta + 1.0).sqrt())
                };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                // вращение Гивенса строки/столбцов p, q
                for k in 0..N {
                    let akp = a[k][p];
                    let akq = a[k][q];
                    a[k][p] = c * akp - s * akq;
                    a[k][q] = s * akp + c * akq;
                }
                for k in 0..N {
                    let apk = a[p][k];
                    let aqk = a[q][k];
                    a[p][k] = c * apk - s * aqk;
                    a[q][k] = s * apk + c * aqk;
                }
            }
        }
    }
    let mut eig = [0.0; N];
    for i in 0..N {
        eig[i] = a[i][i];
    }
    eig.sort_by(|x, y| y.partial_cmp(x).unwrap_or(std::cmp::Ordering::Equal));
    eig
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn exp_of_zero_is_identity() {
        let e = expm(&zeros());
        for i in 0..N {
            for j in 0..N {
                let want = if i == j { 1.0 } else { 0.0 };
                assert!(near(e[i][j], want, 1e-15));
            }
        }
    }

    #[test]
    fn exp_of_skew_is_orthogonal() {
        // J = A − Aᵀ c типичными частотами формант (~15k рад/с)
        let mut a = zeros();
        let w1 = 2.0 * std::f64::consts::PI * 730.0;
        let w2 = 2.0 * std::f64::consts::PI * 2200.0;
        a[0][1] = 0.5 * w1;
        a[1][0] = -0.5 * w1;
        a[2][3] = 0.5 * w2;
        a[3][2] = -0.5 * w2;
        let j = skew(&a);
        let dt = 1.0 / 22_050.0;
        let mut jd = j;
        for i in 0..N {
            for k in 0..N {
                jd[i][k] *= dt;
            }
        }
        let e = expm(&jd);
        // EᵀE = I?
        let mut err: f64 = 0.0;
        for i in 0..N {
            for k in 0..N {
                let mut s = 0.0;
                for m in 0..N {
                    s += e[m][i] * e[m][k];
                }
                let want = if i == k { 1.0 } else { 0.0 };
                err = err.max((s - want).abs());
            }
        }
        assert!(err < 1e-12, "ортогональность expm(кососимм): {err:.3e}");
    }

    #[test]
    fn exp_diagonal_matches_scalar() {
        // диагональный случай: e^12 = 162754 — большая магнитуда теряет
        // абсолютную точность при возведении в квадрат (относительная
        // держится ~1e-14); наш рабочий режим — ‖A‖ ≤ 1 (точность 1e-15).
        let mut a = zeros();
        a[0][0] = -3.7;
        a[1][1] = 0.5;
        a[2][2] = 12.0;
        let e = expm(&a);
        assert!((e[0][0] - (-3.7f64).exp()).abs() / (-3.7f64).exp() < 1e-13);
        assert!((e[1][1] - (0.5f64).exp()).abs() / (0.5f64).exp() < 3e-13);
        assert!((e[2][2] - (12.0f64).exp()).abs() / (12.0f64).exp() < 1e-12);
        assert!(near(e[0][1], 0.0, 1e-15));
    }

    #[test]
    fn solve_linear_system() {
        let mut a = eye();
        a[0][1] = 2.0;
        a[2][0] = -1.0;
        a[4][5] = 0.5;
        let b = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let x = solve(&a, &b).unwrap();
        let y = mat_vec(&a, &x);
        for i in 0..N {
            assert!(near(y[i], b[i], 1e-12));
        }
    }

    #[test]
    fn solve_singular_returns_none() {
        let mut a = zeros();
        a[0][0] = 1.0;
        a[1][1] = 1.0;
        a[2][2] = 1.0;
        a[3][3] = 1.0;
        a[4][4] = 1.0;
        // строка 5 нулевая → вырожденность
        assert!(solve(&a, &[1.0; N]).is_none());
    }

    #[test]
    fn eig_sym_diagonal_trivial() {
        let mut a = zeros();
        a[0][0] = 3.0;
        a[1][1] = -1.0;
        a[2][2] = 7.0;
        a[3][3] = 0.5;
        a[4][4] = 2.0;
        a[5][5] = -4.0;
        let e = eig_sym(&a);
        let want = [7.0, 3.0, 2.0, 0.5, -1.0, -4.0];
        for i in 0..N {
            assert!(near(e[i], want[i], 1e-12));
        }
    }

    #[test]
    fn eig_sym_2x2_block() {
        // [[2,1],[1,2]] → {3,1}; остальное диагональ
        let mut a = zeros();
        a[0][0] = 2.0;
        a[0][1] = 1.0;
        a[1][0] = 1.0;
        a[1][1] = 2.0;
        a[2][2] = 0.0;
        a[3][3] = 0.0;
        a[4][4] = 0.0;
        a[5][5] = 0.0;
        let e = eig_sym(&a);
        assert!(near(e[0], 3.0, 1e-12));
        assert!(near(e[1], 1.0, 1e-12));
    }

    #[test]
    fn mat_mul_identity() {
        let a = eye();
        let mut b = zeros();
        b[0][2] = 5.0;
        b[3][1] = -2.0;
        let c = mat_mul(&a, &b);
        assert_eq!(c[0][2], 5.0);
        assert_eq!(c[3][1], -2.0);
    }

    #[test]
    fn norm_preserved_by_skew_expm() {
        // при применении expm(J·dt) к вектору норма сохраняется
        let mut a = zeros();
        let w = 2.0 * std::f64::consts::PI * 1090.0;
        a[0][1] = 0.5 * w;
        a[1][0] = -0.5 * w;
        a[2][3] = 0.25 * w;
        a[3][2] = -0.25 * w;
        a[4][5] = 0.75 * w;
        a[5][4] = -0.75 * w;
        let j = skew(&a);
        let dt = 1.0 / 22_050.0;
        let mut jd = j;
        for i in 0..N {
            for k in 0..N {
                jd[i][k] *= dt;
            }
        }
        let e = expm(&jd);
        let mut x = [1.0, 0.2, -0.4, 0.3, 0.0, -0.1];
        let e0 = vec_norm(&x);
        for _ in 0..20_000 {
            x = mat_vec(&e, &x);
        }
        let drift = (vec_norm(&x) - e0).abs();
        assert!(drift < 1e-9, "дрейф нормы за 20000 шагов: {drift:.3e}");
    }
}
