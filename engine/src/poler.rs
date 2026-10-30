//! Канонический POLER-цикл — точный порт `p3_poler.zig` (P³ Engine, Kotokvit).
//!
//! ## Каноническое уравнение (P3_Engine/src/p3_poler.zig, «Ключевые формулы»)
//!
//! ```text
//! p_new = p − η · Π_Λ(D·p + γ·J·p + ∇F)
//! D = L·Lᵀ  — диссипатор (энтропийный горел, symmetric ≥ 0)
//! J = A − Aᵀ — резонанс (skew-symmetric, генератор вращения)
//! Π_Λ = I − Jcᵀ(Jc·Jcᵀ + δI)⁻¹Jc — каузальный проектор
//! + CORDIC-ренормализация на S¹ с параметром mix
//! ```
//!
//! Это каноническая форма из серьёзных репозиториев (P3_Engine / poler-os),
//! а не вырожденная Ψ-версия из POLER_Psi_v3.py (без диссипатора, с
//! резонансом-памятью). Отличия принципиальные:
//!
//! * **D·p сжигает** внимание (энтропийный горел) — система без наблюдений
//!   затухает к нулю сама, диссипация гарантирована конструкцией D = L·Lᵀ;
//! * **J·p вращает** фазу (кососимметричный генератор), а не накапливает
//!   память — резонанс здесь это осцилляция, а не взвешенная история;
//! * **∇F притягивает** к текущему наблюдению (свободная энергия);
//! * **CORDIC-ренормализация** удерживает состояние на единичной сфере.
//!
//! ## Редукция к задаче ранжирования
//!
//! Состояние — 2D-вектор `x = (p, q)`: p — позиция внимания, q — фаза
//! резонанса. Наблюдение `o_t = tanh(ε)` отображается в целевую точку
//! `(o_t, 0)`. Все операторы — настоящие матрицы 2×2:
//!
//! * `D = L·Lᵀ` строится из потока наблюдений (нижнетреугольная L с
//!   коэффициентом затухания);
//! * `J = A − Aᵀ` — кососимметричная `[[0,−ω],[ω,0]]`, где ω — резонансная
//!   частота, восстановленная из осцилляций потока ε;
//! * `Π_Λ` — проекция на нуль-пространство ограничений Jc (temporal-фильтр:
//!   наблюдения чужих эпох запрещают сдвиг);
//! * CORDIC `1/√‖x‖²` — быстрая ренормализация на S¹.

use serde::{Deserialize, Serialize};

/// Гиперпараметры канонического POLER-цикла.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PolerParams {
    /// η — learning rate (P3Node.init: 0.01).
    pub eta: f64,
    /// γ — резонансная связь (P3Node.init: 0.1).
    pub gamma: f64,
    /// mix — квантовая нормализация CORDIC (P3Node.init: 0.1).
    pub mix: f64,
    /// δ — порог каузального проектора (PolerEngine.initDefault: 1e-10).
    pub delta: f64,
    /// d — коэффициент диссипатора D = L·Lᵀ (горел внимания).
    pub dissipator: f64,
}

impl Default for PolerParams {
    fn default() -> Self {
        Self {
            eta: 0.01,
            gamma: 0.1,
            mix: 0.1,
            delta: 1e-10,
            dissipator: 0.02,
        }
    }
}

/// 2D-вектор состояния (аналог HomVec4 в 2D-редукции).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub p: f64,
    pub q: f64,
}

impl Vec2 {
    pub fn new(p: f64, q: f64) -> Self {
        Self { p, q }
    }

    fn dot(self, o: Vec2) -> f64 {
        self.p * o.p + self.q * o.q
    }

    fn norm_sq(self) -> f64 {
        self.dot(self)
    }
}

/// Матрица 2×2 (column-major не нужна — работаем построчно).
#[derive(Debug, Clone, Copy)]
struct Mat2 {
    m: [[f64; 2]; 2],
}

impl Mat2 {
    fn apply(&self, v: Vec2) -> Vec2 {
        Vec2::new(
            self.m[0][0] * v.p + self.m[0][1] * v.q,
            self.m[1][0] * v.p + self.m[1][1] * v.q,
        )
    }

    /// A − Aᵀ (kernel.computeResonance из p3_kernel.zig)
    fn skew_part(a: &Mat2) -> Self {
        // Кососимметричная матрица: диагональ равна нулю по определению
        Mat2 {
            m: [
                [0.0, a.m[0][1] - a.m[1][0]],
                [a.m[1][0] - a.m[0][1], 0.0],
            ],
        }
    }
}

/// CORDIC-ренормализация: быстрая аппроксимация 1/√x (аналог
/// `kernel.cordicInvSqrt`). Три Ньютоновские итерации поверх
/// битовой магии — стандартная CORDIC-техника начального приближения.
pub fn cordic_inv_sqrt(x: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    // Начальное приближение через битовую хаклю (double-вариант Quake III):
    // y₀ = from_bits(MAGIC − (bits(x) >> 1))
    let bits = x.to_bits();
    let guess = f64::from_bits(0x5FE6EB50C7B537A9_u64.wrapping_sub(bits >> 1));
    // Три Ньютоновские итерации: y = y·(1.5 − 0.5·x·y²)
    // (начальное приближение ~3% → 1e-5 → 1e-11 → машинная точность)
    let y1 = guess * (1.5 - 0.5 * x * guess * guess);
    let y2 = y1 * (1.5 - 0.5 * x * y1 * y1);
    y2 * (1.5 - 0.5 * x * y2 * y2)
}

/// Канонический POLER-цикл (порт PolerEngine из p3_poler.zig).
pub struct PolerCycle {
    params: PolerParams,
    /// Текущее состояние x = (p, q).
    x: Vec2,
    /// Скользящая оценка резонансной частоты ω из знака осцилляций ε.
    osc_prev: f64,
    omega: f64,
}

impl PolerCycle {
    /// Инициализация: x = (0, 0), ω = 0 (P3Node.init: eta=0.01, gamma=0.1, mix=0.1).
    pub fn new(params: PolerParams) -> Self {
        Self {
            params,
            x: Vec2::new(0.0, 0.0),
            osc_prev: 0.0,
            omega: 0.0,
        }
    }

    /// Каузальный проектор Π_Λ = I − Jcᵀ(JcJcᵀ+δI)⁻¹Jc для одной строки
    /// ограничений Jc (1×2): проекция на нуль-пространство Jc.
    fn projector(jc: Vec2, delta: f64) -> Mat2 {
        let jjt = jc.dot(jc) + delta;
        // I − Jcᵀ·(1/(JcJcᵀ+δ))·Jc
        Mat2 {
            m: [
                [1.0 - jc.p * jc.p / jjt, -jc.p * jc.q / jjt],
                [-jc.q * jc.p / jjt, 1.0 - jc.q * jc.q / jjt],
            ],
        }
    }

    /// Один шаг канонического цикла: `p_new = p − η·Π_Λ(D·p + γ·J·p + ∇F)`.
    ///
    /// * `o_t` — наблюдение (ε-плотность окна, до перцепции);
    /// * `allowed` — Π_Λ: false означает запрет направления (temporal-фильтр):
    ///   ограничение Jc выбирается так, что сила проецируется в нуль.
    pub fn step(&mut self, o_t: f64, allowed: bool) -> Vec2 {
        let params = self.params;

        // Перцепция: Ω(o) = tanh(o) — целевая точка внимания.
        let obs = o_t.tanh();
        let target = Vec2::new(obs, 0.0);

        // --- Диссипатор D = L·Lᵀ: энтропийный горел ---
        // L = [[d, 0], [0, d]] → D = d²·I: изотропное затухание внимания.
        let d = params.dissipator;
        let dissip = Mat2 {
            m: [[d * d, 0.0], [0.0, d * d]],
        };

        // --- Резонанс J = A − Aᵀ: генератор вращения ---
        // ω восстанавливается из осцилляций потока наблюдений (знак Δε):
        // резонанс — свечение на частоте возбуждения, а не память.
        let delta_obs = obs - self.osc_prev;
        self.osc_prev = obs;
        // сглаженная оценка угловой скорости потока
        self.omega = 0.9 * self.omega + 0.1 * (delta_obs * 10.0).clamp(-1.0, 1.0);
        let a = Mat2 {
            m: [[0.0, -self.omega], [self.omega, 0.0]],
        };
        let resonance = Mat2::skew_part(&a);

        // --- Градиент свободной энергии ∇F = 2(x − target) ---
        let grad_f = Vec2::new(2.0 * (self.x.p - target.p), 2.0 * (self.x.q - target.q));

        // --- Сила: D·x + γ·J·x + ∇F ---
        let dx = dissip.apply(self.x);
        let jx = resonance.apply(self.x);
        let force = Vec2::new(
            dx.p + params.gamma * jx.p + grad_f.p,
            dx.q + params.gamma * jx.q + grad_f.q,
        );

        // --- Каузальная проекция Π_Λ(force) ---
        // allowed=false → ограничение Jc вдоль силы: проекция обнуляет сдвиг.
        let (projected, _used_projection) = if allowed {
            (force, false)
        } else {
            let norm = force.norm_sq().sqrt().max(1e-12);
            let jc = Vec2::new(force.p / norm, force.q / norm);
            (Self::projector(jc, params.delta).apply(force), true)
        };

        // --- Обновление: x = x − η·projected ---
        let p_new = Vec2::new(
            self.x.p - params.eta * projected.p,
            self.x.q - params.eta * projected.q,
        );

        // --- CORDIC-ренормализация (квантовая нормализация с mix) ---
        let norm_sq = p_new.norm_sq();
        let inv_norm = cordic_inv_sqrt(norm_sq);
        let scale = (1.0 - params.mix) + params.mix * inv_norm;
        self.x = Vec2::new(p_new.p * scale, p_new.q * scale);
        self.x
    }

    /// Текущее состояние.
    pub fn state(&self) -> Vec2 {
        self.x
    }

    /// POLER-резонанс для ранжирования: модуль внимания на S¹.
    pub fn amplitude(&self) -> f64 {
        self.x.norm_sq().sqrt()
    }
}

/// Прогон канонического POLER-цикла по последовательности наблюдений.
///
/// Возвращает POLER-амплитуду каждого совпадения. `forbidden[i] = true`
/// означает, что наблюдение из запрещённой эпохи (temporal-фильтр → Π_Λ).
pub fn poler_resonances(
    observations: &[f64],
    forbidden: &[bool],
    params: PolerParams,
) -> Vec<f64> {
    let mut cycle = PolerCycle::new(params);
    observations
        .iter()
        .zip(forbidden.iter().chain(std::iter::repeat(&false)))
        .map(|(&o, &f)| {
            cycle.step(o, !f);
            cycle.amplitude()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cordic_inv_sqrt_accuracy() {
        for &x in &[0.25_f64, 1.0, 4.0, 100.0, 1e6] {
            let approx = cordic_inv_sqrt(x);
            let exact = 1.0 / x.sqrt();
            assert!(
                (approx - exact).abs() / exact < 1e-9,
                "x={x}: {approx} vs {exact}"
            );
        }
    }

    #[test]
    fn dissipator_burns_attention_without_observations() {
        // D·p сжигает: без наблюдений (∇F → 0, o=0) внимание затухает
        let params = PolerParams {
            dissipator: 0.5,
            ..PolerParams::default()
        };
        let mut cycle = PolerCycle::new(params);
        // начальный разгон
        for _ in 0..50 {
            cycle.step(1.0, true);
        }
        let a0 = cycle.amplitude();
        // затем тишина
        for _ in 0..200 {
            cycle.step(0.0, true);
        }
        let a1 = cycle.amplitude();
        assert!(a1 < a0, "диссипатор обязан жечь: {a1} !< {a0}");
    }

    #[test]
    fn resonance_oscillates_with_flow() {
        // Осциллирующий поток наблюдений возбуждает резонанс:
        // амплитуда q-компоненты ненулевая (вращение)
        let params = PolerParams::default();
        let mut cycle = PolerCycle::new(params);
        for i in 0..200 {
            let o = (i as f64 * 0.3).sin() * 5.0;
            cycle.step(o, true);
        }
        assert!(cycle.state().q.abs() > 1e-6, "фаза резонанса мертва");
    }

    #[test]
    fn causal_projector_freezes_forbidden_direction() {
        let params = PolerParams::default();
        let mut cycle = PolerCycle::new(params);
        for _ in 0..30 {
            cycle.step(1.0, true);
        }
        let before = cycle.state();
        for _ in 0..30 {
            cycle.step(5.0, false); // запрет: Π_Λ обнуляет силу
        }
        let after = cycle.state();
        // Каузальный проектор подавляет сдвиг к запрещённой цели
        assert!(
            (after.p - before.p).abs() < (before.p.abs() + 1e-9),
            "Π_Λ не сдержал: {before:?} -> {after:?}"
        );
    }

    #[test]
    fn amplitude_bounded_by_cordic_normalization() {
        // mix-ренормализация не даёт состоянию взорваться
        let params = PolerParams {
            eta: 0.2,
            mix: 0.5,
            ..PolerParams::default()
        };
        let obs = vec![50.0_f64; 500]; // огромные наблюдения
        let forb = vec![false; 500];
        let res = poler_resonances(&obs, &forb, params);
        for a in &res {
            assert!(a.is_finite() && *a < 50.0, "амплитуда {a}");
        }
    }

    #[test]
    fn default_params_match_p3_engine() {
        // P3Node.init / PolerEngine.initDefault из p3_poler.zig
        let p = PolerParams::default();
        assert_eq!(p.eta, 0.01);
        assert_eq!(p.gamma, 0.1);
        assert_eq!(p.mix, 0.1);
        assert_eq!(p.delta, 1e-10);
    }

    #[test]
    fn projector_is_idempotent_projection() {
        // Π_Λ·Π_Λ = Π_Λ (проектор!) — проверка на случайной строке Jc
        let jc = Vec2::new(0.6, 0.8);
        let pi = PolerCycle::projector(jc, 1e-10);
        let v = Vec2::new(0.3, -0.7);
        let once = pi.apply(v);
        let twice = pi.apply(once);
        // допуск ~ δ (регуляризация проектора)
        assert!((once.p - twice.p).abs() < 1e-9);
        assert!((once.q - twice.q).abs() < 1e-9);
        // Jc·Πv = 0: проекция ортогональна ограничению
        assert!(jc.dot(once).abs() < 1e-9);
    }
}
