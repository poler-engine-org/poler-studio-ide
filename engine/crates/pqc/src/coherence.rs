//! Метрики квантовой когерентности Born-распределения анзаца.
//!
//! ## Quantum Coherence Metric (QCM)
//!
//! Анзац `⊗ R_y(arccos p)` переводит фазовый вектор в распределение
//! Борна; «когерентность» здесь — степень уверенности (conviction)
//! состояния: насколько измерение предсказуемо относительно максимума
//! энтропии `d_pol` бит:
//!
//! ```text
//! QCM = 1 − H_born / d_pol,   H_born = Σ_q h₂(P(b_q = 1))
//! ```
//!
//! где `h₂` — двоичная энтропия. Для произведения состояний сумма
//! маргинальных энтропий равна энтропии Шеннона распределения (точное
//! равенство); при энтанглменте она является верхней оценкой
//! (субаддитивность) — метрика остаётся корректной мерой предсказуемости
//! измерения. Границы: `QCM = 0` — чистый шум (все честные монеты,
//! состояние LENS пустое или равномерное), `QCM = 1` — полная
//! определённость (все триты ±1).
//!
//! Отчёт содержит теоретическую и наблюдённую (по выстрелам) метрики:
//! их зазор — калибровка качества Born-сэмплирования.

use crate::ansatz::{Engine, SampleReport};

/// Двоичная энтропия `h₂(x) = −x log₂ x − (1−x) log₂(1−x)`.
///
/// Граничные случаи `x ∈ {0, 1}` дают 0; вход вне `[0, 1]` — NaN.
pub fn binary_entropy(x: f64) -> f64 {
    if !x.is_finite() || x < 0.0 || x > 1.0 {
        return f64::NAN;
    }
    let (p, q) = (x, 1.0 - x);
    let mut h = 0.0;
    if p > 0.0 {
        h -= p * p.log2();
    }
    if q > 0.0 {
        h -= q * q.log2();
    }
    h
}

/// Отчёт о когерентности Born-распределения.
#[derive(Clone, Debug)]
pub struct CoherenceReport {
    /// Размерность состояния.
    pub d_pol: u32,
    /// Число хранимых LENS-дуг.
    pub nnz: usize,
    /// Число выстрелов.
    pub shots: u64,
    /// Движок сэмплирования.
    pub engine: Engine,
    /// QCM теории: `1 − Σ h₂(P₁^теория)/d_pol` (фон = 1 бит на координату).
    pub qcm_theory: f64,
    /// QCM наблюдения: то же по маргиналам выстрелов.
    pub qcm_observed: f64,
    /// Энтропия Борна теории в битах (`Σ h₂ + фон`).
    pub born_entropy_bits: f64,
    /// Средняя абсолютная невязка `|наблюдение − теория|` по маргиналам —
    /// калибровка сэмплирования (стремится к 0 как O(1/√shots)).
    pub marginal_mad: f64,
}

impl CoherenceReport {
    /// Зазор между теорией и наблюдением `|QCM_т − QCM_н|`.
    pub fn qcm_gap(&self) -> f64 {
        (self.qcm_theory - self.qcm_observed).abs()
    }
}

/// Сумма маргинальных двоичных энтропий по отчёту сэмплирования
/// с учётом фоновых координат (по 1 биту каждая).
fn born_entropy(marginals: &[(u32, f64, f64)], d_pol: u32, observed: bool) -> f64 {
    let covered = marginals.len() as f64;
    let background = f64::from(d_pol) - covered;
    let mut h = background.max(0.0); // фон: честные монеты, 1 бит каждая
    for m in marginals {
        let v = if observed { m.2 } else { m.1 };
        h += binary_entropy(v);
    }
    h
}

/// Метрика когерентности из отчёта Born-сэмплирования.
pub fn coherence(sample: &SampleReport) -> CoherenceReport {
    let d_pol = sample.d_pol.max(1);
    let h_theory = born_entropy(&sample.marginals, d_pol, false);
    let h_observed = born_entropy(&sample.marginals, d_pol, true);
    let df = f64::from(d_pol);
    let mad = if sample.marginals.is_empty() {
        0.0
    } else {
        sample
            .marginals
            .iter()
            .map(|m| (m.2 - m.1).abs())
            .sum::<f64>()
            / sample.marginals.len() as f64
    };
    CoherenceReport {
        d_pol,
        nnz: sample.marginals.len(),
        shots: sample.shots,
        engine: sample.engine,
        qcm_theory: 1.0 - h_theory / df,
        qcm_observed: 1.0 - h_observed / df,
        born_entropy_bits: h_theory,
        marginal_mad: mad,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_entropy_edges() {
        assert!(binary_entropy(0.0).abs() < 1e-15);
        assert!(binary_entropy(1.0).abs() < 1e-15);
        assert!((binary_entropy(0.5) - 1.0).abs() < 1e-12);
        assert!((binary_entropy(0.25) - 0.8112781244591328).abs() < 1e-12);
        assert!(binary_entropy(-0.1).is_nan());
        assert!(binary_entropy(1.1).is_nan());
    }

    #[test]
    fn qcm_bounds_on_synthetic_reports() {
        // Всё — честные монеты: QCM = 0.
        let fair = mk_report(vec![(0, 0.5, 0.5), (1, 0.5, 0.5)], 2, 100);
        let c = coherence(&fair);
        assert!(c.qcm_theory.abs() < 1e-12);
        assert!(c.qcm_observed.abs() < 1e-12);

        // Всё детерминировано: QCM = 1.
        let det = mk_report(vec![(0, 0.0, 0.0), (1, 1.0, 1.0)], 2, 100);
        let c = coherence(&det);
        assert!((c.qcm_theory - 1.0).abs() < 1e-12);
        assert!((c.qcm_observed - 1.0).abs() < 1e-12);
        assert!(c.marginal_mad.abs() < 1e-15);

        // Половина определена, половина монеты: QCM = 0.5.
        let half = mk_report(vec![(0, 0.0, 0.0), (1, 0.5, 0.51)], 2, 10_000);
        let c = coherence(&half);
        assert!((c.qcm_theory - 0.5).abs() < 1e-12);
        assert!(c.qcm_gap() < 0.01);
    }

    #[test]
    fn background_counts_one_bit_each() {
        // Product-движок: маргиналы покрывают 1 дугу из d=8.
        let rep = mk_report(vec![(3, 0.0, 0.0)], 8, 100);
        let c = coherence(&rep);
        // H = 7 фоновых бит + 0 на дуге → QCM = 1 − 7/8.
        assert!((c.born_entropy_bits - 7.0).abs() < 1e-12);
        assert!((c.qcm_theory - 1.0 / 8.0).abs() < 1e-12);
        assert_eq!(c.nnz, 1);
    }

    fn mk_report(
        marginals: Vec<(u32, f64, f64)>,
        d_pol: u32,
        shots: u64,
    ) -> crate::ansatz::SampleReport {
        crate::ansatz::SampleReport {
            engine: crate::ansatz::Engine::Statevector,
            d_pol,
            shots,
            distinct: 0,
            top: Vec::new(),
            top_probs: Vec::new(),
            marginals,
            weight_mean: 0.0,
            weight_var: 0.0,
            expected_weight: 0.0,
        }
    }
}
