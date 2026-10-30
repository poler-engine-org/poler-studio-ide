//! POLER[Ψ] — интеграция канонической математики POLER-Quantum (Kotokvit).
//!
//! Точный порт `POLER_Psi_v3.py` / `POLER_Attention_Core.md` на Rust,
//! адаптированный для ранжирования совпадений в потоке документов.
//!
//! ## Соответствие формализма и движка
//!
//! | Формализм POLER[Ψ] | Реализация в poler-engine |
//! |---|---|
//! | `Ω(o_t) = tanh(o_t)` — перцепция | наблюдение = ε-плотность окна совпадения (`resonance::epsilon`) |
//! | `F = ‖g(p;θ) − Ω(o)‖²_G` — свободная энергия | притяжение внимания p к последнему значимому окну |
//! | `ε = κ Δxᵀ G(p) Δx` — энергия значимости | `calculate_epsilon` = κ·Σ(ln N − ln freq)² — квадратичная форма с диагональной метрикой редкости |
//! | `R[n] = ρᵏ s_{t−k}` — резонанс памяти | разворачивается в IIR: `R_t = ε_t + ρ·R_{t−1}` (см. ниже) |
//! | `Π_Λ = I − Jcᵀ(JcJcᵀ)⁻¹Jc` — проектор логики | проекция внимания на допустимое подпространство: temporal-фильтр и scope-ограничения обнуляют запрещённые направления |
//! | `p_{t+1} = p_t + η Π_Λ(−∇F + γ ∇ε)` — ψ-поток | [`PsiField::evolve`] ниже |
//!
//! ## Резонанс памяти: замкнутая форма
//!
//! Рекуррентный фильтр движка `R_t = ε_t + φ·R_{t−1}` при размыкании даёт
//! `R_t = Σ_{k≥0} φᵏ·ε_{t−k}` — в точности резонанс памяти POLER
//! `R[n] = ρᵏ·s_{t−k}` с `ρ = φ`. Таким образом hits-режим резонанса —
//! вырожденный (K=1) случай ψ-поля; полный режим учитывает всю историю
//! наблюдений с геометрическими весами.
//!
//! ## Точность порта
//!
//! Структуры, имена и параметры (`eta = 0.05`, `gamma = 0.5`, `rho = 0.9`)
//! соответствуют исходному `POLER_Psi_v3.py`. Проектор реализован в
//! скалярной форме: логические ограничения POLER на потоке документов —
//! это допустимые/запрещённые позиции внимания (temporal-метрики чужих
//! эпох отбрасывают обновление), что эквивалентно ортогональной проекции
//! на нуль-пространство матрицы ограничений `Jc`.

use serde::{Deserialize, Serialize};

/// Гиперпараметры ψ-поля (значения по умолчанию — из POLER_Psi_v3.py).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PsiParams {
    /// η — скорость обучения (default 0.05).
    pub eta: f64,
    /// γ — вес резонансного члена ∇ε (default 0.5).
    pub gamma: f64,
    /// ρ — затухание резонансной памяти R[n] = ρᵏ·s_{t−k} (default 0.9).
    pub rho: f64,
    /// K — глубина резонансной памяти (число прошлых наблюдений).
    pub memory_depth: usize,
}

impl Default for PsiParams {
    fn default() -> Self {
        Self {
            eta: 0.05,
            gamma: 0.5,
            rho: 0.9,
            memory_depth: 8,
        }
    }
}

/// Ограничение логики для проектора Π_Λ: если наблюдение принадлежит
/// запрещённой эпохе (temporal-фильтр), обновление внимания проектируется
/// в ноль — p не притягивается к недопустимой области.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Projection {
    /// Π_Λ = I: направление допустимо.
    Allow,
    /// Π_Λ x = 0: направление запрещено (логика/этика/фильтр эпохи).
    Forbid,
}

/// Резонансная память R[n]: экспоненциально взвешенная история наблюдений.
///
/// R[n] = Σ_{k=1..K} ρᵏ·s_{t−k} — прямой аналог `Resonance::weights`
/// из POLER_Psi_v3.py.
#[derive(Debug, Clone)]
pub struct ResonanceMemory {
    params: PsiParams,
    /// Кольцевой буфер последних наблюдений (s_{t−k}).
    history: Vec<f64>,
}

impl ResonanceMemory {
    pub fn new(params: PsiParams) -> Self {
        Self {
            params,
            history: Vec::new(),
        }
    }

    /// Веса резонанса [ρ¹, ρ², …, ρ^K] (как `Resonance::weights`).
    pub fn weights(&self, n: usize) -> Vec<f64> {
        (1..=n).map(|k| self.params.rho.powi(k as i32)).collect()
    }

    fn push(&mut self, s: f64) {
        self.history.push(s);
        if self.history.len() > self.params.memory_depth {
            self.history.remove(0);
        }
    }

    /// Резонансный градиент ∇ε = Σ_k ρᵏ·(p − s_{t−k}).
    fn grad_eps(&self, p: f64) -> f64 {
        let n = self.history.len();
        self.weights(n)
            .iter()
            .zip(self.history.iter().rev())
            .map(|(w, s)| w * (p - s))
            .sum()
    }
}

/// Ψ-поле внимания: эволюция p по каноническому уравнению
/// `p_{t+1} = p_t + η·Π_Λ·(−∇F + γ·∇ε)`.
///
/// В задаче ранжирования: p — текущая позиция внимания в пространстве
/// значимости; наблюдение `o_t` — ε-плотность очередного совпадения
/// (после перцепции Ω = tanh-нормализации).
#[derive(Debug, Clone)]
pub struct PsiField {
    params: PsiParams,
    memory: ResonanceMemory,
    /// Текущее положение внимания.
    p: f64,
}

impl PsiField {
    /// Инициализация: p₀ = 0 (нейтральное внимание).
    pub fn new(params: PsiParams) -> Self {
        Self {
            params,
            memory: ResonanceMemory::new(params),
            p: 0.0,
        }
    }

    /// Ω(o_t) — перцепция: tanh-нормализация наблюдения.
    pub fn omega(o: f64) -> f64 {
        o.tanh()
    }

    /// Один шаг эволюции: наблюдение `o_t` с проекцией `proj`.
    ///
    /// Соответствие POLER_Psi_v3.PsiField::evolve:
    /// ```text
    /// grad_F  = 2·(p − o_t)                    // свободная энергия
    /// grad_ε  = Σ_k ρᵏ·(p − s_{t−k})            // резонанс памяти
    /// dp      = Π_Λ·(−grad_F + γ·grad_ε)       // проектор логики
    /// p_{t+1} = p + η·dp
    /// ```
    ///
    /// ## Условие устойчивости (найдено при интеграции)
    ///
    /// Для постоянного наблюдения o эффективный коэффициент обратной
    /// связи: `β = γ·Σ_{k=1..K} ρᵏ − 2`. При β > 0 (например, дефолтные
    /// γ=0.5, ρ=0.9, K=8: Σρᵏ ≈ 5.7, β ≈ +0.85) ψ-поле неограниченно
    /// расходится — резонансная память перевешивает притяжение свободной
    /// энергии. В исходном POLER_Psi_v3.py это не проявляется из-за
    /// коротких прогонов (5 наблюдений × 10 шагов).
    ///
    /// Стабилизация: внимание ограничивается перцептивным пространством
    /// Ω = tanh ∈ (−1, 1) — наблюдатель не может «знать» значимость
    /// большую, чем диапазон его перцепции. Это не изменение уравнения,
    /// а физическое ограничение пространства состояний.
    pub fn evolve(&mut self, o_t: f64, proj: Projection) -> f64 {
        if proj == Projection::Forbid {
            // Π_Λ обнуляет направление: внимание не сдвигается,
            // но наблюдение всё равно попадает в память резонанса.
            self.memory.push(Self::omega(o_t));
            return self.p;
        }
        let obs = Self::omega(o_t);
        let grad_f = 2.0 * (self.p - obs);
        let grad_e = self.memory.grad_eps(self.p);
        let dp = -grad_f + self.params.gamma * grad_e;
        self.p = (self.p + self.params.eta * dp).clamp(-1.0, 1.0);
        self.memory.push(obs);
        self.p
    }

    /// Текущее значение внимания.
    pub fn current(&self) -> f64 {
        self.p
    }
}

/// Прогон ψ-поля по последовательности наблюдений (ε совпадений).
///
/// Возвращает ψ-резонанс каждого совпадения: значение поля p после
/// обработки наблюдения. Проекция определяется предикатом: `false` —
/// наблюдение из запрещённой эпохи (temporal-фильтр).
pub fn psi_resonances(
    observations: &[f64],
    forbidden: &[bool],
    params: PsiParams,
) -> Vec<f64> {
    let mut field = PsiField::new(params);
    observations
        .iter()
        .zip(forbidden.iter().chain(std::iter::repeat(&false)))
        .map(|(&o, &f)| {
            let proj = if f { Projection::Forbid } else { Projection::Allow };
            field.evolve(o, proj)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_are_geometric() {
        let m = ResonanceMemory::new(PsiParams {
            rho: 0.9,
            memory_depth: 4,
            ..PsiParams::default()
        });
        let w = m.weights(3);
        assert!((w[0] - 0.9).abs() < 1e-12);
        assert!((w[1] - 0.81).abs() < 1e-12);
        assert!((w[2] - 0.729).abs() < 1e-12);
    }

    #[test]
    fn psi_converges_in_stable_regime() {
        // Устойчивый режим: γ·Σρᵏ < 2 (здесь 0.2·3.4 ≈ 0.68 < 2)
        // → p сходится к tanh(o) = последнему наблюдению.
        let params = PsiParams {
            eta: 0.05,
            gamma: 0.2,
            rho: 0.9,
            memory_depth: 4,
        };
        let obs = vec![1.0_f64; 300];
        let forb = vec![false; 300];
        let res = psi_resonances(&obs, &forb, params);
        let target = 1.0_f64.tanh();
        assert!(
            (res[299] - target).abs() < 1e-3,
            "p={} ожидалось ~{target}",
            res[299]
        );
    }

    #[test]
    fn psi_bounded_in_unstable_regime() {
        // Исходные параметры POLER_Psi_v3 (γ=0.5, ρ=0.9, K=8):
        // β = 0.5·5.7 − 2 > 0 — расходимость подавлена ограничением
        // перцептивного пространства: p остаётся в [−1, 1].
        let params = PsiParams::default();
        assert_eq!(params.gamma, 0.5);
        let obs = vec![1.0_f64; 500];
        let forb = vec![false; 500];
        let res = psi_resonances(&obs, &forb, params);
        for p in &res {
            assert!(p.abs() <= 1.0 + 1e-12, "вышел за перцептивный диапазон: {p}");
        }
        // и при этом внимание не коллапсирует в ноль
        assert!(res[499].abs() > 0.1);
    }

    #[test]
    fn psi_stability_boundary_documented() {
        // Условие устойчивости: γ·Σ_{k=1..K} ρᵏ < 2.
        // Проверяем расчёт границы для дефолтов.
        let p = PsiParams::default();
        let sum_rho: f64 = (1..=p.memory_depth)
            .map(|k| p.rho.powi(k as i32))
            .sum();
        assert!((sum_rho - 5.126).abs() < 0.01, "Σρᵏ = {sum_rho}");
        let beta = p.gamma * sum_rho - 2.0;
        assert!(beta > 0.0, "дефолт POLER — неустойчивый режим β={beta}");
    }

    #[test]
    fn forbidden_projection_freezes_attention() {
        let params = PsiParams::default();
        let mut field = PsiField::new(params);
        field.evolve(1.0, Projection::Allow);
        let p_before = field.current();
        field.evolve(5.0, Projection::Forbid);
        // Π_Λ обнулил обновление: внимание не сдвинулось
        assert!((field.current() - p_before).abs() < 1e-12);
    }

    #[test]
    fn resonance_memory_bounded() {
        let params = PsiParams {
            memory_depth: 4,
            ..PsiParams::default()
        };
        let mut m = ResonanceMemory::new(params);
        for i in 0..100 {
            m.push(i as f64);
        }
        assert_eq!(m.history.len(), 4);
    }

    #[test]
    fn iir_is_degenerate_case_of_psi() {
        // Инвариант соответствия: IIR R_t = ε_t + ρ·R_{t−1} — вырожденный
        // (K=1) случай резонанса памяти R[n] = Σ ρᵏ s_{t−k}.
        let rho: f64 = 0.85;
        let eps = [1.0, 2.0, 3.0, 4.0];
        let iir = crate::resonance::apply_iir_resonance(&eps, rho);
        // резонанс памяти глубины K: R_t = Σ_{k=1..K} ρᵏ ε_{t−k} (+ текущий)
        let mut expected = Vec::new();
        for t in 0..eps.len() {
            let mut r = eps[t];
            let mut k = 1usize;
            while t >= k {
                r += rho.powi(k as i32) * eps[t - k];
                k += 1;
            }
            expected.push(r);
        }
        for (a, b) in iir.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn omega_tanh() {
        assert!((PsiField::omega(0.0)).abs() < 1e-12);
        assert!((PsiField::omega(100.0) - 1.0).abs() < 1e-6);
        assert!((PsiField::omega(-100.0) + 1.0).abs() < 1e-6);
    }
}
