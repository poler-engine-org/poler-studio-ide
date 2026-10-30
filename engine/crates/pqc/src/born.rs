//! Born-сэмплирование: правило Борна `P(x) = |⟨x|ψ⟩|²` через инверсию
//! кумулятивного распределения — бинарный поиск по префиксной сумме,
//! O(log 2^n) на выстрел после O(2^n) подготовки.

use std::collections::BTreeMap;

use crate::error::{PqcError, Result};
use crate::rng::Rng;
use crate::statevector::Statevector;

/// Сэмплирование распределения Борна заданного состояния.
pub struct BornSampler {
    /// Кумулятивные вероятности: `cdf[i] = P(x ≤ i)`, последний элемент = 1.
    cdf: Vec<f64>,
}

impl BornSampler {
    /// Строит CDF по нормированному состоянию
    /// (`|‖ψ‖ − 1| ≤ 1e-9`, иначе [`PqcError::NotNormalized`]).
    pub fn new(sv: &Statevector) -> Result<BornSampler> {
        let norm = sv.norm();
        if (norm - 1.0).abs() > 1e-9 {
            return Err(PqcError::NotNormalized { norm });
        }
        let probs = sv.probabilities();
        let mut cdf = Vec::with_capacity(probs.len());
        let mut acc = 0.0;
        for p in probs {
            acc += p;
            cdf.push(acc);
        }
        // Точная единица в последнем элементе — защита от округления суммы.
        if let Some(last) = cdf.last_mut() {
            *last = 1.0;
        }
        Ok(BornSampler { cdf })
    }

    /// Размерность пространства исходов (2^n).
    pub fn dim(&self) -> usize {
        self.cdf.len()
    }

    /// Один выстрел: `x ~ P(x) = |⟨x|ψ⟩|²`.
    pub fn sample(&self, rng: &mut Rng) -> u64 {
        let u = rng.next_f64();
        // Первый индекс с cdf[i] > u.
        let i = self.cdf.partition_point(|&c| c <= u);
        i.min(self.cdf.len() - 1) as u64
    }

    /// `shots` независимых выстрелов.
    pub fn sample_n(&self, rng: &mut Rng, shots: u64) -> Vec<u64> {
        let mut out = Vec::with_capacity(shots as usize);
        for _ in 0..shots {
            out.push(self.sample(rng));
        }
        out
    }

    /// Счётчики исходов, отсортированные по убыванию частоты
    /// (при равенстве — по возрастанию исхода).
    pub fn sample_counts(&self, rng: &mut Rng, shots: u64) -> Vec<(u64, u64)> {
        counts_from(self.sample_n(rng, shots))
    }

    /// Топ-K исходов по частоте.
    pub fn top_k(&self, rng: &mut Rng, shots: u64, k: usize) -> Vec<(u64, u64)> {
        let mut c = self.sample_counts(rng, shots);
        c.truncate(k);
        c
    }

    /// Теоретическая вероятность исхода: `cdf[x] − cdf[x−1]`.
    pub fn outcome_probability(&self, outcome: u64) -> f64 {
        let i = outcome as usize;
        if i >= self.cdf.len() {
            return 0.0;
        }
        let hi = self.cdf[i];
        let lo = if i == 0 { 0.0 } else { self.cdf[i - 1] };
        (hi - lo).max(0.0)
    }
}

/// Счётчики исходов из списка выстрелов.
pub(crate) fn counts_from(outcomes: Vec<u64>) -> Vec<(u64, u64)> {
    let mut map = BTreeMap::new();
    for o in outcomes {
        *map.entry(o).or_insert(0u64) += 1;
    }
    let mut v: Vec<(u64, u64)> = map.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unnormalized_state() {
        let mut sv = Statevector::from_phases(&[0.0]).unwrap();
        for a in sv.amplitudes_mut() {
            *a = a.scale(2.0);
        }
        assert!(matches!(
            BornSampler::new(&sv),
            Err(PqcError::NotNormalized { .. })
        ));
    }

    #[test]
    fn cdf_ends_at_exact_one() {
        let sv = Statevector::from_phases(&[0.0, 0.0, 0.0]).unwrap();
        let s = BornSampler::new(&sv).unwrap();
        assert_eq!(s.dim(), 8);
    }

    #[test]
    fn outcome_probability_edges() {
        let sv = Statevector::from_phases(&[0.6, -0.2]).unwrap();
        let s = BornSampler::new(&sv).unwrap();
        assert_eq!(s.outcome_probability(999), 0.0);
        let total: f64 = (0..s.dim() as u64).map(|x| s.outcome_probability(x)).sum();
        assert!((total - 1.0).abs() < 1e-12);
    }
}
