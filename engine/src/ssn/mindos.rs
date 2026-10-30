//! Операторы MindOS — нейроморфные примитивы поверх векторов состояний.
//!
//! Доказано (V9, proofs/ssn_verify3.py):
//! - **NMDA-гейт**: coincidence-детектор — гейт tanh(⟨state, ctx⟩) открыт
//!   только при совпадении состояния с контекстом (0.995 для align,
//!   0.000 для ортогонального). Обучение «через ворот» — как настоящие
//!   NMDA-рецепторы: пластичность только при ко-активации.
//! - **WTA** (winner-take-all): ровно k активных компонент, норма 1 —
//!   разреженный код, сжимающий состояние до k лидеров.
//! - **Bind**: интерполяция A·g + B·(1−g) — комбинирование смыслов
//!   с гарантией ‖bound‖ ≤ g‖A‖ + (1−g)‖B‖ (неравенство треугольника).
//! - **Модуляция с затуханием**: α_t = α₀·e^(−t/τ) — вес сходится,
//!   а не убегает (стабильное обучение).

/// NMDA-гейт: возвращает открытие ворота g = tanh(⟨state, ctx⟩)
/// и применяет обучение state += x·w·g (сквозь ворота).
pub fn nmda(state: &mut [f64], x: &[f64], ctx: &[f64], w: f64) -> f64 {
    let gate = dot(state, ctx).tanh();
    for (s, xi) in state.iter_mut().zip(x) {
        *s += xi * w * gate;
    }
    gate
}

/// Только значение гейта (без изменения состояния).
pub fn nmda_gate(state: &[f64], ctx: &[f64]) -> f64 {
    dot(state, ctx).tanh()
}

/// WTA: оставить ровно `k` наибольших компонент, нормировать к 1.
/// in-place. При k = 0 — обнуление.
pub fn wta(state: &mut [f64], k: usize) {
    let n = state.len();
    if k == 0 || n == 0 {
        state.fill(0.0);
        return;
    }
    let mut idx: Vec<usize> = (0..n).collect();
    // топ-k по значению; при равенстве — меньший индекс (детерминизм)
    idx.sort_by(|&a, &b| {
        state[b].partial_cmp(&state[a]).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b))
    });
    let mut keep = vec![false; n];
    for &i in &idx[..k.min(n)] {
        keep[i] = true;
    }
    for (s, kept) in state.iter_mut().zip(keep) {
        if !kept {
            *s = 0.0;
        }
    }
    let nrm = crate::ssn::cse::norm(state);
    if nrm > 0.0 {
        for s in state.iter_mut() {
            *s /= nrm;
        }
    }
}

/// Bind: интерполяция A·g + B·(1−g). Гарантия: ‖bound‖ ≤ g‖A‖+(1−g)‖B‖.
pub fn bind(a: &[f64], b: &[f64], g: f64) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| x * g + y * (1.0 - g)).collect()
}

/// Модуляция с экспоненциальным затуханием: w += α₀·e^(−t/τ)·rate.
/// Сходится к w₀ + α₀·τ·rate (конечная сумма геометрического ряда).
pub fn modulate(w: f64, t: usize, alpha0: f64, tau: f64, rate: f64) -> f64 {
    w + alpha0 * (-(t as f64) / tau).exp() * rate
}

#[inline]
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssn::cse::norm;

    fn e0e1(d: usize) -> (Vec<f64>, Vec<f64>) {
        let mut e0 = vec![0.0; d];
        let mut e1 = vec![0.0; d];
        e0[0] = 3.0;
        e1[1] = 3.0; // ортогональные ненулевые состояния
        (e0, e1)
    }

    #[test]
    fn v9a_nmda_gate_align_vs_ortho() {
        let d = 64;
        let (e0, e1) = e0e1(d);
        let mut ctx = vec![0.0; d];
        ctx[0] = 1.0;
        let g_align = nmda_gate(&e0, &ctx);
        let g_ortho = nmda_gate(&e1, &ctx);
        assert!(g_align > 0.9, "g_align={g_align:.3}");
        assert!(g_ortho.abs() < 0.05, "g_ortho={g_ortho:.3}");
    }

    #[test]
    fn v9b_v9c_wta_count_and_norm() {
        let d = 64;
        let mut state: Vec<f64> = (0..d).map(|i| ((i * 7 + 3) % 13) as f64 * 0.1 - 0.6).collect();
        state[3] = 5.0;
        state[17] = 4.0;
        wta(&mut state, 10);
        let active = state.iter().filter(|x| x.abs() > 1e-12).count();
        assert_eq!(active, 10, "ровно 10 активных");
        assert!((norm(&state) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn v9d_composite_dynamics_bounded() {
        // 1000 шагов: состояние + вход, NMDA-фильтрация, периодический WTA, tanh
        let d = 64;
        let mut rng = crate::ssn::rng::Rng::new(20260918);
        let ctx = vec![1.0 / (d as f64).sqrt(); d];
        let mut state = vec![0.0; d];
        for t in 0..1000 {
            let x: Vec<f64> = (0..d).map(|_| rng.normal(0.0, 1.0).tanh()).collect();
            for (s, xi) in state.iter_mut().zip(&x) {
                *s += 0.05 * xi;
            }
            nmda(&mut state, &x, &ctx, 0.05);
            if t % 10 == 0 {
                wta(&mut state, 10);
            }
            for s in state.iter_mut() {
                *s = s.tanh();
            }
        }
        assert!(norm(&state) < (d as f64).sqrt(), "norm={}", norm(&state));
    }

    #[test]
    fn v9e_modulation_converges() {
        let mut w = 0.5;
        for t in 0..100 {
            w = modulate(w, t, 0.01, 50.0, 0.1);
        }
        assert!((0.5..0.6).contains(&w), "w={w:.4}");
    }

    #[test]
    fn v9f_bind_triangle_inequality() {
        let d = 64;
        let mut rng = crate::ssn::rng::Rng::new(555);
        let a: Vec<f64> = (0..d).map(|_| rng.normal(0.0, 1.0)).collect();
        let b: Vec<f64> = (0..d).map(|_| rng.normal(0.0, 1.0)).collect();
        let bound = bind(&a, &b, 0.7);
        assert!(norm(&bound) <= 0.7 * norm(&a) + 0.3 * norm(&b) + 1e-9);
    }
}
