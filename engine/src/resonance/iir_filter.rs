//! Линейный рекурсивный фильтр резонанса: `R_t = ε_t + φ·R_{t-1}`.
//!
//! Сложность — строго O(N) в один линейный проход: один FMA-подобный
//! аккумулятор без ветвлений. Коэффициент затухания φ рекомендуется
//! держать в диапазоне [0.75, 0.90]; значения вне [0, 1] безопасно
//! клампятся.

/// Клампит φ в физически осмысленный диапазон [0, 1].
/// NaN/∞ -> значение по умолчанию 0.85.
pub fn clamp_phi(phi: f64) -> f64 {
    if !phi.is_finite() {
        return 0.85;
    }
    phi.clamp(0.0, 1.0)
}

/// Батч-вариант IIR-фильтра: применяет `R_t = ε_t + φ·R_{t-1}` к последовательности.
pub fn apply_iir_resonance(epsilons: &[f64], phi_decay: f64) -> Vec<f64> {
    let phi = clamp_phi(phi_decay);
    let mut out = Vec::with_capacity(epsilons.len());
    let mut r = 0.0f64;
    for &e in epsilons {
        r = e + phi * r;
        out.push(r);
    }
    out
}

/// Потоковый IIR-аккумулятор (O(1) памяти на элемент).
pub struct IirFilter {
    phi: f64,
    state: f64,
}

impl IirFilter {
    /// Создаёт фильтр с коэффициентом затухания `phi`.
    pub fn new(phi: f64) -> Self {
        Self {
            phi: clamp_phi(phi),
            state: 0.0,
        }
    }

    /// Проталкивает очередное ε и возвращает текущий резонанс R_t.
    pub fn push(&mut self, eps: f64) -> f64 {
        self.state = eps + self.phi * self.state;
        self.state
    }

    /// Текущее состояние фильтра без обновления.
    pub fn current(&self) -> f64 {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hand_computed_sequence() {
        // R1 = 1; R2 = 1 + 0.5*1 = 1.5; R3 = 1 + 0.5*1.5 = 1.75
        let out = apply_iir_resonance(&[1.0, 1.0, 1.0], 0.5);
        assert!((out[0] - 1.0).abs() < 1e-12);
        assert!((out[1] - 1.5).abs() < 1e-12);
        assert!((out[2] - 1.75).abs() < 1e-12);
    }

    #[test]
    fn zero_phi_passes_through() {
        let out = apply_iir_resonance(&[3.0, 4.0, 5.0], 0.0);
        assert_eq!(out, vec![3.0, 4.0, 5.0]);
    }

    #[test]
    fn resonance_never_below_epsilon() {
        let out = apply_iir_resonance(&[2.0, 0.1, 7.0, 0.0], 0.9);
        for (e, r) in [2.0, 0.1, 7.0, 0.0].iter().zip(out.iter()) {
            assert!(*r >= *e - 1e-12, "r={r} < e={e}");
        }
    }

    #[test]
    fn streaming_matches_batch() {
        let eps = [0.5, 1.5, 0.0, 3.3, 2.2, 0.7];
        let batch = apply_iir_resonance(&eps, 0.82);
        let mut f = IirFilter::new(0.82);
        for (e, b) in eps.iter().zip(batch.iter()) {
            let s = f.push(*e);
            assert!((s - b).abs() < 1e-12);
        }
    }

    #[test]
    fn phi_out_of_range_is_clamped() {
        assert_eq!(clamp_phi(1.7), 1.0);
        assert_eq!(clamp_phi(-0.3), 0.0);
        assert_eq!(clamp_phi(f64::NAN), 0.85);
    }

    #[test]
    fn decay_converges_for_constant_input() {
        // Постоянное ε=1, φ=0.5: R -> 2.0
        let out = apply_iir_resonance(&vec![1.0; 60], 0.5);
        assert!((out[59] - 2.0).abs() < 1e-9);
    }
}
