//! R[n] Резонанс: темпоральное эхо смысловой памяти.
//!
//! Замкнутая форма резонанса POLER `P[n] = ∫ A(t)·e^{−λt} dt` в
//! дискретном времени — взвешенная сумма прошлых attract-состояний:
//!
//! `echo_t = (1−ρ)·Σ_{k=0..H−1} ρᵏ·p_{t−k}`  (нормировано на единицу)
//!
//! с коэффициентом затухания ρ = 0.9. Нить повествования удерживается
//! не объёмом контекстного окна, а интегралом состояний: вклад давних
//! аттракторов геометрически тает, но никогда не обрывается скачком —
//! на дистанции в тысячи шагов эхо остаётся непрерывной функцией
//! истории (скалярный предшественник — `crate::psi::ResonanceMemory`,
//! здесь — векторная форма для фазового пространства).
//!
//! Вторая продукция модуля — **энергия значимости** ε («смысловое
//! удивление»): расстояние между текущим состоянием и его эхом.
//! Высокое ε = система встретила новое; низкое = когнитивный покой.
//! Пластичность движка: `plasticity_rate = 0.2 · ε`.

use std::collections::VecDeque;

/// Темпоральное резонансное эхо R[n].
pub struct TemporalEcho {
    /// Коэффициент затухания ρ ∈ [0, 1).
    pub rho: f32,
    /// Глубина истории H (кольцевой буфер аттракторов).
    horizon: usize,
    /// История attract-состояний (свежие — в конце).
    history: VecDeque<Vec<f32>>,
    /// Число осей (проверка консистентности push).
    dims: usize,
    /// Резонансные веса w_k = ρᵏ·(1−ρ) (для Σ w_k ∇ε в интеграторе).
    resonance_weights: Vec<f32>,
}

impl TemporalEcho {
    /// Новое эхо: ρ (умолчание 0.9) и горизонт истории H.
    pub fn new(rho: f32, horizon: usize, dims: usize) -> Self {
        let rho = rho.clamp(0.0, 0.999);
        let horizon = horizon.clamp(2, 512);
        let resonance_weights: Vec<f32> = (0..horizon).map(|k| (1.0 - rho) * rho.powi(k as i32)).collect();
        Self {
            rho,
            horizon,
            history: VecDeque::with_capacity(horizon),
            dims,
            resonance_weights,
        }
    }

    /// Горизонт истории.
    pub fn horizon(&self) -> usize {
        self.horizon
    }

    /// Число состояний в памяти (≤ horizon).
    pub fn depth(&self) -> usize {
        self.history.len()
    }

    /// Резонансные веса w_k (k = 0 — самый свежий).
    pub fn weights(&self) -> &[f32] {
        &self.resonance_weights
    }

    /// Зафиксировать attract-состояние p_t в памяти.
    /// Вектор чужой размерности — ошибка (защита фазового пространства).
    pub fn push(&mut self, state: &[f32]) -> Result<(), String> {
        if state.len() != self.dims {
            return Err(format!(
                "эхо: состояние из {} осей, память на {}",
                state.len(),
                self.dims
            ));
        }
        self.history.push_back(state.to_vec());
        while self.history.len() > self.horizon {
            self.history.pop_front();
        }
        Ok(())
    }

    /// Текущее эхо: нормированное взвешенное среднее истории
    /// (аттрактор памяти, сравнимый по масштабу с p). Пустая память —
    /// нулевой вектор.
    pub fn echo(&self) -> Vec<f32> {
        let mut out = self.echo_sum();
        // Нормировка суммы весов → среднее.
        let mut wsum = 0.0f32;
        let n = self.history.len();
        for k in 0..n.min(self.horizon) {
            let w = self.resonance_weights[k];
            if w < 1e-9 {
                break;
            }
            wsum += w;
        }
        if wsum > 1e-9 {
            for v in out.iter_mut() {
                *v /= wsum;
            }
        }
        out
    }

    /// Замкнутая форма резонанса POLER: P[n] = Σ ρᵏ·p_{t−k} —
    /// НЕнормированная сумма (интеграл состояний; скалярный
    /// предшественник — `crate::psi` R_t = ε_t + ρ·R_{t−1}).
    pub fn echo_sum(&self) -> Vec<f32> {
        let mut out = vec![0.0f32; self.dims];
        if self.history.is_empty() {
            return out;
        }
        // Свежие состояния лежат в конце deque: k = 0 → history[len−1].
        let n = self.history.len();
        for k in 0..n.min(self.horizon) {
            let w = self.resonance_weights[k];
            if w < 1e-9 {
                break; // хвост ρᵏ пренебрежим
            }
            let s = &self.history[n - 1 - k];
            for (o, &v) in out.iter_mut().zip(s) {
                *o += w * v;
            }
        }
        out
    }

    /// Резонансная норма ‖echo‖₂ — магнитуда накопленной памяти.
    pub fn resonance_norm(&self) -> f32 {
        super::linalg::norm2(&self.echo())
    }

    /// Энергия значимости ε: κ·‖p_t − echo‖₂² («смысловое удивление»
    /// относительно собственной истории). Пустая память → 0 (нечему
    /// удивляться — первого наблюдения ещё нет).
    pub fn surprise(&self, current: &[f32], kappa: f32) -> f32 {
        if self.history.is_empty() {
            return 0.0;
        }
        let e = self.echo();
        let d = super::linalg::sub(current, &e);
        kappa * super::linalg::dot(&d, &d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_form_matches_geometric_weights() {
        let mut e = TemporalEcho::new(0.9, 8, 2);
        // Константа: и среднее, и сумма-интеграл видят константу/её долю.
        for _ in 0..5 {
            e.push(&[3.0, -4.0]).unwrap();
        }
        let ec = e.echo();
        assert!((ec[0] - 3.0).abs() < 1e-4 && (ec[1] + 4.0).abs() < 1e-4);
        // Замкнутая форма: один импульс → P[n] = w_0·impulse = (1−ρ)·impulse
        let mut e2 = TemporalEcho::new(0.9, 8, 2);
        e2.push(&[10.0, 0.0]).unwrap();
        let s1 = e2.echo_sum();
        assert!((s1[0] - 1.0).abs() < 1e-5, "w_0·10 = 0.1·10 = 1: {s1:?}");
        // Два состояния [10,0] затем [0,10]:
        // P = w_0·[0,10] + w_1·[10,0] = [0.9, 1.0]
        e2.push(&[0.0, 10.0]).unwrap();
        let s2 = e2.echo_sum();
        assert!((s2[0] - 0.9).abs() < 1e-4, "s2 = {s2:?}");
        assert!((s2[1] - 1.0).abs() < 1e-4, "s2 = {s2:?}");
        // Среднее тех же данных: [0.9, 1.0]/0.19
        let m2 = e2.echo();
        assert!((m2[0] - 0.9 / 0.19).abs() < 1e-3, "m2 = {m2:?}");
    }

    #[test]
    fn horizon_evicts_and_weights_sum_to_one() {
        let mut e = TemporalEcho::new(0.5, 4, 1);
        for i in 0..10 {
            e.push(&[i as f32]).unwrap();
        }
        assert_eq!(e.depth(), 4, "горизонт 4 вытесняет древнее");
        // В памяти последние 4: 6,7,8,9. Веса свежести: 0.5,0.25,0.125,0.0625
        let ec = e.echo()[0];
        let expect = (0.5 * 9.0 + 0.25 * 8.0 + 0.125 * 7.0 + 0.0625 * 6.0) / 0.9375;
        assert!((ec - expect).abs() < 1e-4, "{ec} vs {expect}");
        // Сумма весов буфера ≤ 1, растёт с глубиной
        let w: f32 = e.weights().iter().sum();
        assert!(w <= 1.0 + 1e-6);
    }

    #[test]
    fn surprise_orders_novelty() {
        let mut e = TemporalEcho::new(0.9, 16, 3);
        for _ in 0..6 {
            e.push(&[1.0, 0.0, 0.0]).unwrap();
        }
        // Повтор прошлого — почти нулевое удивление
        let s_old = e.surprise(&[1.0, 0.0, 0.0], 1.2);
        // Резкая новизна — большое
        let s_new = e.surprise(&[0.0, 1.0, 0.0], 1.2);
        assert!(s_new > 10.0 * s_old, "новизна: {s_new} vs {s_old}");
        assert!(s_old < 1e-3, "покой почти нулевой: {s_old}");
        // κ масштабирует линейно
        let s_k = e.surprise(&[0.0, 1.0, 0.0], 2.4);
        assert!((s_k - 2.0 * s_new).abs() < 1e-4);
        // Пустая память — ноль
        let e0 = TemporalEcho::new(0.9, 4, 3);
        assert_eq!(e0.surprise(&[1.0, 1.0, 1.0], 1.2), 0.0);
        assert_eq!(e0.echo(), vec![0.0, 0.0, 0.0]);
        assert_eq!(e0.resonance_norm(), 0.0);
    }

    #[test]
    fn dimension_mismatch_is_error_and_clamps() {
        let mut e = TemporalEcho::new(0.9, 4, 2);
        assert!(e.push(&[1.0, 2.0, 3.0]).is_err());
        // ρ клампится в [0, 1): крайние значения не паникуют
        let e2 = TemporalEcho::new(1.5, 0, 2);
        assert!(e2.rho < 1.0);
        assert!(e2.horizon() >= 2);
        let e3 = TemporalEcho::new(-0.5, 99999, 2);
        assert_eq!(e3.rho, 0.0);
        assert!(e3.horizon() <= 512);
        let mut e3 = e3;
        e3.push(&[1.0, 1.0]).unwrap();
        e3.push(&[2.0, 2.0]).unwrap();
        // ρ=0: эхо = только самое свежее состояние
        assert!((e3.echo()[0] - 2.0).abs() < 1e-6);
    }
}
