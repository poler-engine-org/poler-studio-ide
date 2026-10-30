//! Роторный резонатор с ПЕРЕМЕННЫМИ формантами (коартикуляция).
//!
//! Цикл K синтезировал одиночные гласные на фиксированных формантах.
//! Здесь резонатор один на всю книгу: состояние ψ непрерывно, а цели
//! F1/F2/F3 плывут от звука к звуку. Точная ZOH-дискретизация
//! ψ̇ = (J − D)ψ + g·u пересчитывается БЛОКАМИ (32 сэмпла ≈ 1.45 мс):
//! внутри блока матрицы постоянны, между блоками — новая геометрия
//! тракта. Это и есть физика коартикуляции: тракт деформируется
//! непрерывно, звук не «переключается».
//!
//! Порядок обхода семпла идентичен циклу K:
//! ψ ← Ad·ψ + u·bd;  y = ψ[0] + 0.9·ψ[2] + 1.3·ψ[4].

use crate::linalg::{self, Mat, Vec6};
use crate::rng::Xorshift64;

/// Размер блока пересчёта ZOH-матриц (сэмплов).
pub const BLOCK: usize = 32;

/// Геометрия тракта на блок: форманты + смуги.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tract {
    /// F1, F2, F3 (Гц) — цели уже сглаженного глайдами значения.
    pub formants: [f64; 3],
    /// BW1..BW3 (Гц).
    pub bandwidths: [f64; 3],
}

/// Кососимметричные вихревые связи (личность диктора, из семени).
/// Фиксированы на всю фразу: это геометрия, а не артикуляция.
pub type Couplings = Mat;

/// Сгенерировать связи из потока семени: симметричная ±3% матрица,
/// затем J = A − Aᵀ даст кососимметричные перекачки энергии.
pub fn couplings_from_seed(g: &mut Xorshift64) -> Couplings {
    let mut raw = linalg::zeros();
    for i in 0..linalg::N {
        for j in 0..linalg::N {
            raw[i][j] = 0.03 * g.sym_milli();
        }
    }
    // симметризация (как в эталоне: raw = 0.5·(raw + rawᵀ))
    let mut sym = linalg::zeros();
    for i in 0..linalg::N {
        for j in 0..linalg::N {
            sym[i][j] = 0.5 * (raw[i][j] + raw[j][i]);
        }
    }
    sym
}

/// Построить ZOH-матрицы для данной геометрии тракта:
/// Ad = expm((J − D)·dt), bd = (J−D)⁻¹·(Ad − I)·g.
pub fn zoh(tract: &Tract, couplings: &Couplings, fs: f64) -> (Mat, Vec6) {
    let dt = 1.0 / fs;
    let mut a = linalg::zeros();
    for (k, &f) in tract.formants.iter().enumerate() {
        let w = 2.0 * std::f64::consts::PI * f;
        // A−Aᵀ удваивает недиагональ → кладём ω/2
        a[2 * k][2 * k + 1] = 0.5 * w;
        a[2 * k + 1][2 * k] = -0.5 * w;
    }
    linalg::mat_add_assign(&mut a, couplings);
    let j = linalg::skew(&a);
    let mut fm = j;
    // D = diag(π·BW) — вычитаем из диагонали
    for k in 0..3 {
        let d = std::f64::consts::PI * tract.bandwidths[k];
        fm[2 * k][2 * k] -= d;
        fm[2 * k + 1][2 * k + 1] -= d;
    }
    // вход g: взвешенное возбуждение формант (как в эталоне)
    let wgt = [1.0, 0.9, 1.25];
    let m = 3.0f64.sqrt();
    let mut g = [0.0; 6];
    for k in 0..3 {
        g[2 * k] = wgt[k] / m;
        g[2 * k + 1] = 0.3 * wgt[k] / m;
    }
    // Fm·dt
    let mut fmdt = fm;
    for i in 0..linalg::N {
        for j2 in 0..linalg::N {
            fmdt[i][j2] *= dt;
        }
    }
    let ad = linalg::expm(&fmdt);
    // bd = Fm⁻¹·(Ad − I)·g  ⇔  Fm·x = (Ad − I)·g
    let mut am = ad;
    for i in 0..linalg::N {
        am[i][i] -= 1.0;
    }
    let rhs = linalg::mat_vec(&am, &g);
    let bd = linalg::solve(&fm, &rhs).unwrap_or([0.0; 6]);
    (ad, bd)
}

/// Роторная (без дисипации) ZOH-матрица для инвариант-проб норм.
pub fn zoh_rotor(tract: &Tract, couplings: &Couplings, fs: f64) -> Mat {
    let dt = 1.0 / fs;
    let mut a = linalg::zeros();
    for (k, &f) in tract.formants.iter().enumerate() {
        let w = 2.0 * std::f64::consts::PI * f;
        a[2 * k][2 * k + 1] = 0.5 * w;
        a[2 * k + 1][2 * k] = -0.5 * w;
    }
    linalg::mat_add_assign(&mut a, couplings);
    let j = linalg::skew(&a);
    let mut jdt = j;
    for i in 0..linalg::N {
        for j2 in 0..linalg::N {
            jdt[i][j2] *= dt;
        }
    }
    linalg::expm(&jdt)
}

/// Наблюдение: взвешенная сумма «косинусных» компонент формант
/// (высокие моди чувствительнее — площадь излучения).
#[inline]
pub fn observe(psi: &Vec6) -> f64 {
    psi[0] + 0.9 * psi[2] + 1.3 * psi[4]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tract() -> Tract {
        Tract {
            formants: [730.0, 1090.0, 2440.0],
            bandwidths: [90.0, 100.0, 130.0],
        }
    }

    #[test]
    fn zoh_dissipates_without_input() {
        // ψ ← Ad·ψ без входа: энергия строго убывает (D > 0)
        let mut g = Xorshift64::new(11);
        let c = couplings_from_seed(&mut g);
        let (ad, _) = zoh(&tract(), &c, 22_050.0);
        let mut psi = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut e_prev = linalg::vec_norm(&psi);
        for _ in 0..5000 {
            psi = linalg::mat_vec(&ad, &psi);
            let e = linalg::vec_norm(&psi);
            assert!(e <= e_prev + 1e-15, "дисипация нарушена: {e} > {e_prev}");
            e_prev = e;
        }
        assert!(e_prev < 0.01, "энергия должна уйти: {e_prev}");
    }

    #[test]
    fn zoh_bd_finite_and_nonzero() {
        let mut g = Xorshift64::new(22);
        let c = couplings_from_seed(&mut g);
        let (_, bd) = zoh(&tract(), &c, 22_050.0);
        assert!(bd.iter().all(|&v| v.is_finite()));
        assert!(bd.iter().any(|&v| v.abs() > 1e-9));
    }

    #[test]
    fn rotor_zoh_preserves_norm() {
        let mut g = Xorshift64::new(33);
        let c = couplings_from_seed(&mut g);
        let ad = zoh_rotor(&tract(), &c, 22_050.0);
        let mut psi = [0.3, -0.2, 0.5, 0.1, -0.4, 0.2];
        let e0 = linalg::vec_norm(&psi);
        for _ in 0..20_000 {
            psi = linalg::mat_vec(&ad, &psi);
        }
        let drift = (linalg::vec_norm(&psi) - e0).abs();
        assert!(drift < 1e-9, "дрейф нормы ротора: {drift:.3e}");
    }

    #[test]
    fn block_change_of_tract_is_smooth() {
        // при смене геометрии на соседнюю ZOH-матрицы близки:
        // это гарантирует непрерывность ψ при коартикуляции
        let mut g = Xorshift64::new(44);
        let c = couplings_from_seed(&mut g);
        let t1 = tract();
        let t2 = Tract { formants: [745.0, 1100.0, 2445.0], bandwidths: [90.0, 100.0, 130.0] };
        let (ad1, bd1) = zoh(&t1, &c, 22_050.0);
        let (ad2, bd2) = zoh(&t2, &c, 22_050.0);
        let mut dmax: f64 = 0.0;
        for i in 0..6 {
            for j in 0..6 {
                dmax = dmax.max((ad1[i][j] - ad2[i][j]).abs());
            }
        }
        assert!(dmax < 1e-2, "близкие тракты дают близкие Ad: {dmax}");
        let bdd = bd1.iter().zip(bd2.iter()).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        assert!(bdd < 1e-6, "bd непрерывен: {bdd}");
    }

    #[test]
    fn couplings_symmetric() {
        let mut g = Xorshift64::new(55);
        let c = couplings_from_seed(&mut g);
        for i in 0..6 {
            for j in 0..6 {
                assert!((c[i][j] - c[j][i]).abs() < 1e-15);
            }
        }
    }
}
