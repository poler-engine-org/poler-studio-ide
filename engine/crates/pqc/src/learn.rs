//! Петля фазового обучения Born (RQ5): замкнутый цикл
//! «измерение → суррогатный градиент → фазовый шаг» на чистом Rust —
//! без Python, PyTorch и каких-либо внешних фреймворков.
//!
//! ## Математика
//!
//! Трек `p_i = cos(θ_i)` кодируется кубитом через `θ_i = arccos(p_i)`;
//! Born-измерение даёт несмещённую оценку параметра:
//!
//! ```text
//! p_emp,i = 1 − 2·N₁,i / N_shots,     E[p_emp,i] = p_i
//! ```
//!
//! Градиент целевого функционала `F(p, o)` по фазе — параметрический сдвиг
//! через цепное правило `dp/dθ = −sin θ = −√(1 − p²)`:
//!
//! ```text
//! ∇_θ,i F = ∇_p,i F · (−√(max(0, 1 − p_i²)))
//! ```
//!
//! Канонический шаг потока (тяжёлый шар + проектор причинности Π_Λ,
//! ср. гиперпараметры заголовка `dp/dt = −η·Π_Λ(∇F − γ∇ε)`):
//!
//! ```text
//! v ← γ·v + Π_Λ[∇_θ F]
//! θ ← arccos(p) − η·v,      p ← cos(θ) ∈ [−1, 1]
//! ```
//!
//! Каждые `K` итераций применяется McWeeny-пурификация
//! `p ← 2(3λ² − 2λ³) − 1, λ = (1+p)/2` — заострение к чистым тритам
//! (восстановление `P² = P` без переобучения).
//!
//! ## Полюсы и трение
//!
//! На чистых тритах `|p| = 1` производная `dp/dθ = −sin θ` вырождается
//! в нуль — параметрический сдвиг физически не может сдвинуть фазу с
//! полюса. Сильный момент (γ ≥ 0.8) при резкой смене цели способен
//! «занести» фазу точно на полюс и залипнуть там; для отслеживания
//! regime-jump'ов (инверсия цели) следует умерять трение (γ ≈ 0.5).
//! Пурификация, напротив, целенаправленно сгоняет фазы НА полюсы —
//! это режим сходимости к тритам, а не баг.
//!
//! ## Проектор причинности Π_Λ
//!
//! `Π_Λ` замораживает дуги с `|p| < ε` (порог LENS-значимости): фоновые
//! монеты не участвуют в потоке. Для product-движка носитель дуг уже
//! разрежен структурно — фон физически отсутствует в контейнере, поэтому
//! Π_Λ = (носитель дуг) ∩ {|p| ≥ ε}.
//!
//! ## Пример: сходимость за 10 шагов
//!
//! ```
//! use pqc::{Ansatz, BornOptimizer, LoadOptions};
//!
//! // Цель — сильные фазы, старт — слабые сигналы.
//! let target = vec![0.9, -0.9, 0.8, -0.8];
//! let start  = vec![0.1, -0.1, 0.2, -0.2];
//! let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();
//! let mut opt = BornOptimizer::new(0.25, 0.9, 4096).with_seed(42);
//!
//! let reports = opt
//!     .run(&mut ansatz, |p_emp| {
//!         p_emp.iter().zip(&target).map(|(e, t)| e - t).collect()
//!     }, 10)
//!     .unwrap();
//! assert_eq!(reports.len(), 10);
//!
//! // Параметрическая потеря ½‖p − target‖² упала более чем в 20 раз.
//! let p = BornOptimizer::parameters(&ansatz).unwrap();
//! let loss = 0.5 * p.iter().zip(&target).map(|(a, b)| (a - b).powi(2)).sum::<f64>();
//! assert!(loss < 0.06, "loss = {loss}");
//! ```

use pqw::mcweeny::purify_p;

use crate::ansatz::Ansatz;
use crate::born::BornSampler;
use crate::coherence::binary_entropy;
use crate::error::{PqcError, Result};
use crate::rng::Rng;
use crate::statevector::Statevector;

/// Оптимизатор фазового потока по Born-измерениям.
///
/// Владеет собственным ГПСЧ-потоком (xoshiro256++): один и тот же сид
/// даёт побитово воспроизводимую траекторию обучения.
pub struct BornOptimizer {
    /// Шаг потока η (learning rate).
    eta: f64,
    /// Трение потока γ (momentum, тяжёлый шар).
    gamma: f64,
    /// Число Born-выстрелов на одно измерение маргинал.
    shots: u64,
    /// Период McWeeny-пурификации; 0 = выключена.
    purify_every: usize,
    /// Порог проектора Π_Λ: дуги с `|p| < ε` заморожены.
    epsilon: f64,
    /// Счётчик сделанных шагов.
    steps: usize,
    /// Буфер момента в θ-пространстве.
    velocity: Vec<f64>,
    /// ГПСЧ измерений.
    rng: Rng,
}

/// Отчёт об одном шаге петли обучения.
#[derive(Clone, Debug, PartialEq)]
pub struct StepReport {
    /// Номер шага (с единицы).
    pub step: usize,
    /// Выстрелов на измерение.
    pub shots: u64,
    /// Суррогатная потеря `½‖Π_Λ g‖²` — для канонической квадратичной
    /// цели это `F(p_emp)` в точке измерения.
    pub surrogate_loss: f64,
    /// Норма спроецированного градиента `‖Π_Λ ∇_θ F‖₂`.
    pub grad_norm: f64,
    /// Максимальное изменение фазы `max |Δp|` за шаг.
    pub max_dp: f64,
    /// Средний шум измерения `mean |p_emp − p|` по активным дугам.
    pub measurement_mad: f64,
    /// Применялась ли McWeeny-пурификация на этом шаге.
    pub purified: bool,
    /// Средний `|p|` по активным дугам — LENS-плотность потока.
    pub mean_abs_p: f64,
}

impl BornOptimizer {
    /// Новый оптимизатор: `eta > 0`, `gamma ∈ [0, 1)`, `shots ≥ 1`.
    ///
    /// Сид ГПСЧ по умолчанию 0 (полная воспроизводимость);
    /// см. [`BornOptimizer::with_seed`].
    pub fn new(eta: f64, gamma: f64, shots: u64) -> BornOptimizer {
        BornOptimizer {
            eta,
            gamma,
            shots: shots.max(1),
            purify_every: 0,
            epsilon: 0.0,
            steps: 0,
            velocity: Vec::new(),
            rng: Rng::seed_from_u64(0),
        }
    }

    /// Сид ГПСЧ-потока измерений (builder).
    pub fn with_seed(mut self, seed: u64) -> BornOptimizer {
        self.rng = Rng::seed_from_u64(seed);
        self
    }

    /// Подключить готовый ГПСЧ (builder).
    pub fn with_rng(mut self, rng: Rng) -> BornOptimizer {
        self.rng = rng;
        self
    }

    /// Период McWeeny-пурификации `K`; 0 = выключена (builder).
    pub fn with_purify_every(mut self, every: usize) -> BornOptimizer {
        self.purify_every = every;
        self
    }

    /// Порог проектора причинности Π_Λ (builder): дуги `|p| < ε` заморожены.
    pub fn with_epsilon(mut self, epsilon: f64) -> BornOptimizer {
        self.epsilon = epsilon;
        self
    }

    /// Шаг потока η.
    pub fn eta(&self) -> f64 {
        self.eta
    }

    /// Трение потока γ.
    pub fn gamma(&self) -> f64 {
        self.gamma
    }

    /// Выстрелов на измерение.
    pub fn shots(&self) -> u64 {
        self.shots
    }

    /// Число сделанных шагов.
    pub fn steps(&self) -> usize {
        self.steps
    }

    /// Плотный вектор параметров `p` анзаца.
    ///
    /// * statevector-движок: эффективные маргиналы `p_q = 1 − 2·P(b_q = 1)`
    ///   (для чистого Ry-слоя — в точности углы кодирования);
    /// * product-движок: фазы хранимых дуг, фон — нули.
    pub fn parameters(ansatz: &Ansatz) -> Result<Vec<f64>> {
        parameters_of(ansatz)
    }

    /// Плотная запись параметров: statevector пересобирается как Ry-слой
    /// (цикл «измерение → обновление → перекодировка»), product обновляет
    /// фазы хранимых дуг — фон остаётся честными монетами (Π_Λ).
    pub fn set_parameters(ansatz: &mut Ansatz, ps: &[f64]) -> Result<()> {
        set_parameters(ansatz, ps)
    }

    /// Born-измерение маргинал: `p_emp = 1 − 2·P̄(b = 1)` по `shots` выстрелам.
    ///
    /// Для product-движка фон не сэмплируется покоординатно: его ожидание
    /// равно нулю и в плотном векторе он занимает `p_emp = 0`.
    pub fn measure(&mut self, ansatz: &Ansatz) -> Result<Vec<f64>> {
        measure(ansatz, &mut self.rng, self.shots)
    }

    /// Один шаг петли: измерить → оценить градиент → фазовый шаг.
    ///
    /// Замыкание `grad_at` получает плотный вектор измерения `p_emp`
    /// (несмещённую оценку текущих параметров) и возвращает градиент
    /// `∇_p F` той же длины — суррогат Борна для параметрического сдвига.
    pub fn step<F>(&mut self, ansatz: &mut Ansatz, grad_at: F) -> Result<StepReport>
    where
        F: FnOnce(&[f64]) -> Vec<f64>,
    {
        // 1. Параметры и структурная маска Π_Λ (носитель дуг).
        let p = parameters_of(ansatz)?;
        let d = p.len();
        let support = support_of(ansatz, d);

        // 2. Born-измерение: p_emp = 1 − 2·P̄(b=1).
        let p_emp = measure(ansatz, &mut self.rng, self.shots)?;
        debug_assert_eq!(p_emp.len(), d);

        // 3. Суррогатный градиент в точке измерения.
        let g = grad_at(&p_emp);
        if g.len() != d {
            return Err(PqcError::LengthMismatch {
                expected: d,
                actual: g.len(),
            });
        }

        // 4. Проекция Π_Λ и цепное правило dp/dθ = −√(1−p²).
        let mut grad_theta = vec![0.0_f64; d];
        let mut sq_norm = 0.0_f64;
        let mut mad_sum = 0.0_f64;
        let mut mad_n = 0u64;
        let mut abs_p_sum = 0.0_f64;
        for i in 0..d {
            let active = support[i] && p[i].abs() >= self.epsilon;
            if support[i] {
                mad_sum += (p_emp[i] - p[i]).abs();
                abs_p_sum += p[i].abs();
                mad_n += 1;
            }
            if !active {
                continue;
            }
            let chain = -(1.0 - p[i] * p[i]).max(0.0).sqrt();
            let gt = chain * g[i];
            grad_theta[i] = gt;
            sq_norm += gt * gt;
        }
        let surrogate_loss = 0.5 * sq_norm;
        let grad_norm = sq_norm.sqrt();
        let measurement_mad = if mad_n > 0 {
            mad_sum / mad_n as f64
        } else {
            0.0
        };
        let mean_abs_p = if mad_n > 0 {
            abs_p_sum / mad_n as f64
        } else {
            0.0
        };

        // 5. Момент и фазовый шаг: θ ← arccos(p) − η·v, p ← cos θ.
        if self.velocity.len() != d {
            self.velocity = vec![0.0_f64; d];
        }
        let mut p_new = vec![0.0_f64; d];
        let mut max_dp = 0.0_f64;
        for i in 0..d {
            self.velocity[i] = self.gamma * self.velocity[i] + grad_theta[i];
            let theta = p[i].acos();
            let theta_new = theta - self.eta * self.velocity[i];
            let pn = theta_new.cos().clamp(-1.0, 1.0);
            max_dp = max_dp.max((pn - p[i]).abs());
            p_new[i] = pn;
        }

        // 6. McWeeny-пурификация каждые K шагов.
        // Около полюсов ±1 полином 3λ²−2λ³ может перескочить границу
        // на ошибке округления (например, 1 + 4e−16) — возвращаем в [−1, 1].
        self.steps += 1;
        let purified = self.purify_every > 0 && self.steps.is_multiple_of(self.purify_every);
        if purified {
            for v in p_new.iter_mut() {
                *v = purify_p(*v).clamp(-1.0, 1.0);
            }
        }

        // 7. Перекодировка анзаца.
        set_parameters(ansatz, &p_new)?;

        Ok(StepReport {
            step: self.steps,
            shots: self.shots,
            surrogate_loss,
            grad_norm,
            max_dp,
            measurement_mad,
            purified,
            mean_abs_p,
        })
    }

    /// `steps` шагов подряд; замыкание вызывается на каждом шаге
    /// (см. [`BornOptimizer::step`]).
    pub fn run<F>(
        &mut self,
        ansatz: &mut Ansatz,
        grad_at: F,
        steps: usize,
    ) -> Result<Vec<StepReport>>
    where
        F: Fn(&[f64]) -> Vec<f64>,
    {
        let mut reports = Vec::with_capacity(steps);
        for _ in 0..steps {
            reports.push(self.step(ansatz, |p| grad_at(p))?);
        }
        Ok(reports)
    }
}

/// Каноническая квадратичная цель: `F(p) = ½‖p − target‖²`, `∇F = p − target`.
///
/// Оценка градиента берётся в точке измерения `p_emp` — суррогат Борна
/// из симуляции владельца: `∂F/∂θᵢ = (p_emp − target)·(−√(1 − p²ᵢ))`.
pub fn quadratic_target(target: &[f64]) -> impl Fn(&[f64]) -> Vec<f64> + '_ {
    move |p_emp| p_emp.iter().zip(target).map(|(e, t)| e - t).collect()
}

// ============================================================================
// Active Inference (RQ6): непрерывное онлайн-обучение фаз из потока
// ============================================================================

/// Онлайновый оптимизатор Active Inference (RQ6): петля Борна с **адаптивным
/// шагом** по энтропийному сюрпризу потока — поверх той же математики
/// параметрического сдвига, что и [`BornOptimizer`].
///
/// ## Расписание шага
///
/// ```text
/// η(t) = η₀ · exp(−β · Σ(t)),   Σ(t) = (1/|S|) · Σ_{i∈S} h₂(P(bᵢ = 1))
/// ```
///
/// `Σ(t)` — сюрприз модели: нормированная Born-энтропия **активных дуг**
/// текущего состояния (фон в `Σ` не входит — структурная часть Π_Λ уже
/// заморожена). В хаосе (арки ≈ честные монеты, `Σ → 1`) шаг сжимается до
/// `η₀·e^(−β)` — термодинамическое трение `D = LLᵀ` гасит шум среды; у
/// стационара (`Σ → 0`, фазы ≈ триты) шаг возвращается к `η₀` — оператор
/// резонанса `J` держит нить смысла полной скоростью. Это уравнение
/// субъективного времени `T = dI/dΣ` из архитектурного манифеста:
/// при `ΔΣ → 0` система достигает стационарности `Ĥ_Ψ = 0`.
///
/// ## Защита от полюсов
///
/// Трение по умолчанию `γ = 0.5` — вывод анализа RQ5: при `γ ≥ 0.8`
/// импульс заносит фазу точно на полюс `|p| = 1`, где `dp/dθ = −sin θ`
/// вырождается, и параметрический сдвиг не может её сдвинуть (залипание
/// при regime-jump). `γ ≈ 0.5` отслеживает инверсию цели за 10 шагов.
///
/// McWeeny-пурификация каждые `K = 5` шагов ликвидирует дрейф идемпотентности
/// `P² = P` без переобучения.
///
/// ## Пример: адаптивная сходимость
///
/// ```
/// use pqc::{ActiveInference, Ansatz, LoadOptions};
///
/// let target = vec![0.9, -0.9, 0.8, -0.8];
/// let start  = vec![0.1, -0.1, 0.2, -0.2];
/// let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();
/// let mut ai = ActiveInference::new(0.25, 2.0, 4096).with_seed(42);
///
/// let reports = ai
///     .run(&mut ansatz, |p| {
///         p.iter().zip(&target).map(|(e, t)| e - t).collect()
///     }, 10)
///     .unwrap();
/// assert_eq!(reports.len(), 10);
///
/// // Расписание шага: η ≤ η₀ всегда, и растёт по мере падения сюрприза.
/// assert!(reports.iter().all(|r| r.eta <= 0.25 + 1e-12));
/// assert!(reports.last().unwrap().eta > reports.first().unwrap().eta);
/// ```
pub struct ActiveInference {
    /// Базовый шаг η₀ > 0.
    eta0: f64,
    /// Коэффициент затухания β ≥ 0 (чувствительность к сюрпризу).
    beta: f64,
    /// Трение γ ∈ [0, 1); по умолчанию 0.5 (защита полюсов, RQ5).
    gamma: f64,
    /// Born-выстрелов на измерение.
    shots: u64,
    /// Период McWeeny-пурификации K; 0 = выключена.
    purify_every: usize,
    /// Порог проектора Π_Λ.
    epsilon: f64,
    /// Счётчик шагов.
    steps: usize,
    /// Буфер момента в θ-пространстве.
    velocity: Vec<f64>,
    /// ГПСЧ измерений.
    rng: Rng,
    /// Сюрприз последнего шага Σ(t−1).
    last_surprise: f64,
    /// Шаг последнего применения η(t−1).
    last_eta: f64,
}

/// Отчёт об одном шаге Active Inference.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveStepReport {
    /// Номер шага (с единицы).
    pub step: usize,
    /// Выстрелов на измерение.
    pub shots: u64,
    /// Сюрприз Σ(t) — Born-энтропия активных дуг до шага.
    pub surprise: f64,
    /// Адаптивный шаг η(t) = η₀·e^(−βΣ).
    pub eta: f64,
    /// Суррогатная потеря `½‖Π_Λ g‖²`.
    pub surrogate_loss: f64,
    /// Норма спроецированного градиента `‖Π_Λ ∇_θ F‖₂`.
    pub grad_norm: f64,
    /// Максимальное изменение фазы `max |Δp|` за шаг.
    pub max_dp: f64,
    /// Средний шум измерения по активным дугам.
    pub measurement_mad: f64,
    /// Применялась ли McWeeny-пурификация.
    pub purified: bool,
    /// Средний `|p|` по активным дугам.
    pub mean_abs_p: f64,
}

impl ActiveInference {
    /// Новый оптимизатор: `eta0 > 0`, `beta ≥ 0`, `shots ≥ 1`.
    ///
    /// Значения по умолчанию: `γ = 0.5` (полюсная защита RQ5),
    /// McWeeny каждые `K = 5` шагов, `ε = 0`, сид ГПСЧ 0.
    pub fn new(eta0: f64, beta: f64, shots: u64) -> ActiveInference {
        ActiveInference {
            eta0,
            beta,
            gamma: 0.5,
            shots: shots.max(1),
            purify_every: 5,
            epsilon: 0.0,
            steps: 0,
            velocity: Vec::new(),
            rng: Rng::seed_from_u64(0),
            last_surprise: f64::NAN,
            last_eta: f64::NAN,
        }
    }

    /// Сид ГПСЧ-потока измерений (builder).
    pub fn with_seed(mut self, seed: u64) -> ActiveInference {
        self.rng = Rng::seed_from_u64(seed);
        self
    }

    /// Подключить готовый ГПСЧ (builder).
    pub fn with_rng(mut self, rng: Rng) -> ActiveInference {
        self.rng = rng;
        self
    }

    /// Трение γ (builder); для отслеживания regime-jump держите γ ≈ 0.5.
    pub fn with_gamma(mut self, gamma: f64) -> ActiveInference {
        self.gamma = gamma;
        self
    }

    /// Период McWeeny-пурификации K; 0 = выключена (builder).
    pub fn with_purify_every(mut self, every: usize) -> ActiveInference {
        self.purify_every = every;
        self
    }

    /// Порог проектора причинности Π_Λ (builder).
    pub fn with_epsilon(mut self, epsilon: f64) -> ActiveInference {
        self.epsilon = epsilon;
        self
    }

    /// Базовый шаг η₀.
    pub fn eta0(&self) -> f64 {
        self.eta0
    }

    /// Коэффициент затухания β.
    pub fn beta(&self) -> f64 {
        self.beta
    }

    /// Трение γ.
    pub fn gamma(&self) -> f64 {
        self.gamma
    }

    /// Число сделанных шагов.
    pub fn steps(&self) -> usize {
        self.steps
    }

    /// Сюрприз последнего шага Σ(t−1) (NaN до первого шага).
    pub fn last_surprise(&self) -> f64 {
        self.last_surprise
    }

    /// Адаптивный шаг последнего применения η(t−1) (NaN до первого шага).
    pub fn last_eta(&self) -> f64 {
        self.last_eta
    }

    /// Расписание шага: `η(Σ) = η₀ · exp(−β·Σ)`, `Σ ∈ [0, 1]`.
    ///
    /// Монотонно убывает по сюрпризу: `η(0) = η₀`, `η(1) = η₀·e^(−β)`.
    pub fn eta_at(&self, surprise: f64) -> f64 {
        self.eta0 * (-self.beta * surprise.clamp(0.0, 1.0)).exp()
    }

    /// Сюрприз состояния Σ: нормированная Born-энтропия активных дуг.
    ///
    /// Считается по теоретическим маргиналам параметров (без шума
    /// измерения) — расписание шага детерминировано.
    pub fn surprise(ansatz: &Ansatz) -> f64 {
        match ansatz {
            Ansatz::Statevector(sv) => {
                let n = sv.n_qubits().max(1) as f64;
                sv.marginals()
                    .iter()
                    .map(|&m| binary_entropy(m))
                    .sum::<f64>()
                    / n
            }
            Ansatz::Product(pa) => {
                let n = pa.nnz().max(1) as f64;
                pa.arcs()
                    .iter()
                    .map(|&(_, p)| binary_entropy(0.5 * (1.0 - p)))
                    .sum::<f64>()
                    / n
            }
        }
    }

    /// Один шаг петли Active Inference: сюрприз → адаптивный η → измерение →
    /// суррогатный градиент → Π_Λ → момент → фазовый шаг → (K) пурификация.
    ///
    /// Замыкание `grad_at` получает плотный вектор измерения `p_emp`
    /// (как в [`BornOptimizer::step`]).
    pub fn step<F>(&mut self, ansatz: &mut Ansatz, grad_at: F) -> Result<ActiveStepReport>
    where
        F: FnOnce(&[f64]) -> Vec<f64>,
    {
        // 1. Сюрприз и адаптивный шаг ДО обновления: Σ(t) по текущим фазам.
        let surprise = ActiveInference::surprise(ansatz);
        let eta = self.eta_at(surprise);

        // 2. Параметры и структурная маска Π_Λ.
        let p = parameters_of(ansatz)?;
        let d = p.len();
        let support = support_of(ansatz, d);

        // 3. Born-измерение.
        let p_emp = measure(ansatz, &mut self.rng, self.shots)?;
        debug_assert_eq!(p_emp.len(), d);

        // 4. Суррогатный градиент в точке измерения.
        let g = grad_at(&p_emp);
        if g.len() != d {
            return Err(PqcError::LengthMismatch {
                expected: d,
                actual: g.len(),
            });
        }

        // 5. Проекция Π_Λ и цепное правило dp/dθ = −√(1−p²).
        // Π_Λ строгая (улучшение RQ6): у замороженных дуг обнуляется
        // и момент — фаза не дрейфует от инерции истории.
        if self.velocity.len() != d {
            self.velocity = vec![0.0_f64; d];
        }
        let mut grad_theta = vec![0.0_f64; d];
        let mut sq_norm = 0.0_f64;
        let mut mad_sum = 0.0_f64;
        let mut mad_n = 0u64;
        let mut abs_p_sum = 0.0_f64;
        for i in 0..d {
            let active = support[i] && p[i].abs() >= self.epsilon;
            if support[i] {
                mad_sum += (p_emp[i] - p[i]).abs();
                abs_p_sum += p[i].abs();
                mad_n += 1;
            }
            if !active {
                self.velocity[i] = 0.0;
                continue;
            }
            let chain = -(1.0 - p[i] * p[i]).max(0.0).sqrt();
            let gt = chain * g[i];
            grad_theta[i] = gt;
            sq_norm += gt * gt;
        }
        let surrogate_loss = 0.5 * sq_norm;
        let grad_norm = sq_norm.sqrt();
        let measurement_mad = if mad_n > 0 {
            mad_sum / mad_n as f64
        } else {
            0.0
        };
        let mean_abs_p = if mad_n > 0 {
            abs_p_sum / mad_n as f64
        } else {
            0.0
        };

        // 6. Момент и фазовый шаг с адаптивным η.
        let mut p_new = vec![0.0_f64; d];
        let mut max_dp = 0.0_f64;
        for i in 0..d {
            self.velocity[i] = self.gamma * self.velocity[i] + grad_theta[i];
            let theta = p[i].acos();
            let theta_new = theta - eta * self.velocity[i];
            let pn = theta_new.cos().clamp(-1.0, 1.0);
            max_dp = max_dp.max((pn - p[i]).abs());
            p_new[i] = pn;
        }

        // 7. McWeeny-пурификация каждые K шагов.
        self.steps += 1;
        let purified = self.purify_every > 0 && self.steps.is_multiple_of(self.purify_every);
        if purified {
            for v in p_new.iter_mut() {
                *v = purify_p(*v).clamp(-1.0, 1.0);
            }
        }

        // 8. Перекодировка анзаца и память расписания.
        set_parameters(ansatz, &p_new)?;
        self.last_surprise = surprise;
        self.last_eta = eta;

        Ok(ActiveStepReport {
            step: self.steps,
            shots: self.shots,
            surprise,
            eta,
            surrogate_loss,
            grad_norm,
            max_dp,
            measurement_mad,
            purified,
            mean_abs_p,
        })
    }

    /// `steps` шагов подряд; замыкание вызывается на каждом шаге.
    pub fn run<F>(
        &mut self,
        ansatz: &mut Ansatz,
        grad_at: F,
        steps: usize,
    ) -> Result<Vec<ActiveStepReport>>
    where
        F: Fn(&[f64]) -> Vec<f64>,
    {
        let mut reports = Vec::with_capacity(steps);
        for _ in 0..steps {
            reports.push(self.step(ansatz, |p| grad_at(p))?);
        }
        Ok(reports)
    }
}

/// Плотный вектор параметров анзаца (см. [`BornOptimizer::parameters`]).
fn parameters_of(ansatz: &Ansatz) -> Result<Vec<f64>> {
    match ansatz {
        Ansatz::Statevector(sv) => {
            // Эффективные маргиналы: для чистого Ry-слоя совпадают с углами.
            Ok(sv.marginals().iter().map(|&p1| 1.0 - 2.0 * p1).collect())
        }
        Ansatz::Product(pa) => {
            let d = pa.d_pol() as usize;
            let mut ps = vec![0.0_f64; d];
            for &(i, p) in pa.arcs() {
                ps[i as usize] = p;
            }
            Ok(ps)
        }
    }
}

/// Структурная часть Π_Λ: какие координаты вообще являются параметрами.
fn support_of(ansatz: &Ansatz, d: usize) -> Vec<bool> {
    match ansatz {
        Ansatz::Statevector(_) => vec![true; d],
        Ansatz::Product(pa) => {
            let mut sup = vec![false; d];
            for &(i, _) in pa.arcs() {
                sup[i as usize] = true;
            }
            sup
        }
    }
}

/// Born-измерение маргинал (общее для обоих движков).
fn measure(ansatz: &Ansatz, rng: &mut Rng, shots: u64) -> Result<Vec<f64>> {
    let n = shots.max(1) as f64;
    match ansatz {
        Ansatz::Statevector(sv) => {
            let sampler = BornSampler::new(sv)?;
            let outcomes = sampler.sample_n(rng, shots);
            let qn = sv.n_qubits();
            let mut ones = vec![0u64; qn];
            for o in outcomes {
                let mut rest = o;
                let mut q = 0;
                while rest != 0 && q < qn {
                    if rest & 1 == 1 {
                        ones[q] += 1;
                    }
                    rest >>= 1;
                    q += 1;
                }
            }
            Ok(ones.into_iter().map(|c| 1.0 - 2.0 * c as f64 / n).collect())
        }
        Ansatz::Product(pa) => {
            let st = pa.sample(rng, shots);
            let mut p_emp = vec![0.0_f64; pa.d_pol() as usize];
            for (k, &(i, _)) in pa.arcs().iter().enumerate() {
                p_emp[i as usize] = 1.0 - 2.0 * st.ones[k] as f64 / n;
            }
            Ok(p_emp)
        }
    }
}

/// Плотная запись параметров (см. [`BornOptimizer::set_parameters`]).
fn set_parameters(ansatz: &mut Ansatz, ps: &[f64]) -> Result<()> {
    match ansatz {
        Ansatz::Statevector(sv) => {
            // Цикл POLER: измеренное состояние перекодируется фазами заново.
            *sv = Statevector::from_phases(ps)?;
            Ok(())
        }
        Ansatz::Product(pa) => pa.set_arc_phases(ps),
    }
}

/// Параметрическая потеря `½‖p − target‖²` (общий тестовый хелпер).
#[cfg(test)]
fn param_loss(ansatz: &Ansatz, target: &[f64]) -> f64 {
    let p = parameters_of(ansatz).unwrap();
    0.5 * p
        .iter()
        .zip(target)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
}

/// Дрейф-цель: сильные фазы на каждой второй координате (общий хелпер).
#[cfg(test)]
fn drift_setup(d: usize) -> (Vec<f64>, Vec<f64>) {
    let target: Vec<f64> = (0..d)
        .map(|i| if i % 2 == 0 { 0.9 } else { -0.8 })
        .collect();
    let start: Vec<f64> = (0..d).map(|i| 0.15 * (i as f64 % 3.0 - 1.0)).collect();
    (start, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ansatz::LoadOptions;
    use crate::PhaseAnsatz;

    #[test]
    fn gradient_chain_rule_formula_is_exact() {
        // ∇_θ = −√(1−p²)·g: ручная сверка одной координаты.
        // p = 0.6, g = 0.3, η = 0.5, γ = 0:
        //   ∇_θ = −0.8·0.3 = −0.24; θ' = arccos(0.6) + 0.12; p' = cos(θ').
        let mut opt = BornOptimizer::new(0.5, 0.0, 1).with_seed(1);
        let mut ansatz = Ansatz::from_phases(&[0.6], &LoadOptions::default()).unwrap();
        let rep = opt.step(&mut ansatz, |_p_emp| vec![0.3]).unwrap();
        let expected = (0.6_f64.acos() + 0.12).cos();
        let p = parameters_of(&ansatz).unwrap();
        assert!((p[0] - expected).abs() < 1e-15);
        assert!((rep.grad_norm - 0.24).abs() < 1e-15);
        assert!((rep.surrogate_loss - 0.5 * 0.24 * 0.24).abs() < 1e-15);
        assert!(!rep.purified);
    }

    #[test]
    fn momentum_accumulates_velocity() {
        // γ = 0.5: второй шаг несёт половину скорости первого.
        // Шаг 1: v = −0.24, Δθ = η·0.24; шаг 2 (тот же градиент):
        //   v = 0.5·(−0.24) + (−0.24) = −0.36 → Δθ₂ = η·0.36.
        let mut opt = BornOptimizer::new(0.1, 0.5, 1).with_seed(1);
        let mut ansatz = Ansatz::from_phases(&[0.6], &LoadOptions::default()).unwrap();
        opt.step(&mut ansatz, |_| vec![0.3]).unwrap();
        let p1 = parameters_of(&ansatz).unwrap()[0];
        opt.step(&mut ansatz, |_| vec![0.3]).unwrap();
        let p2 = parameters_of(&ansatz).unwrap()[0];
        // Оба шага уменьшают p (g > 0), второй — сильнее.
        assert!(p1 < 0.6 && p2 < p1);
        let step1 = 0.6 - p1;
        let step2 = p1 - p2;
        assert!(
            step2 > step1,
            "шаг 2 ({step2}) должен превзойти шаг 1 ({step1})"
        );
    }

    #[test]
    fn closure_length_is_validated() {
        let mut opt = BornOptimizer::new(0.1, 0.0, 8);
        let mut ansatz = Ansatz::from_phases(&[0.1, 0.2], &LoadOptions::default()).unwrap();
        assert!(matches!(
            opt.step(&mut ansatz, |_| vec![0.0]),
            Err(PqcError::LengthMismatch { expected: 2, .. })
        ));
    }

    #[test]
    fn measure_is_unbiased() {
        // E[p_emp] = p: при 100k выстрелов уклонение < 0.02 на координату.
        let ps = vec![0.6, -0.4, 0.0, 0.9, -0.9, 0.25, -0.25, 0.75];
        let ansatz = Ansatz::from_phases(&ps, &LoadOptions::default()).unwrap();
        let mut opt = BornOptimizer::new(0.1, 0.0, 100_000).with_seed(7);
        let p_emp = opt.measure(&ansatz).unwrap();
        for (i, (&e, &p)) in p_emp.iter().zip(&ps).enumerate() {
            assert!((e - p).abs() < 0.02, "coord {i}: emp {e} vs p {p}");
        }
    }

    /// DoD RQ5: сходимость за ≤ 10 шагов на Born-сэмплах (product-движок).
    #[test]
    fn converges_within_ten_steps_product() {
        let (start, target) = drift_setup(512);
        let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();
        assert_eq!(ansatz.engine().name(), "product");
        let l0 = param_loss(&ansatz, &target);

        let mut opt = BornOptimizer::new(0.25, 0.9, 4096).with_seed(42);
        let reports = opt.run(&mut ansatz, quadratic_target(&target), 10).unwrap();
        assert_eq!(reports.len(), 10);

        let l10 = param_loss(&ansatz, &target);
        assert!(
            l10 < l0 / 20.0,
            "потеря {l0:.4} → {l10:.4} — сходимость за 10 шагов недостаточна"
        );
        // Финальные фазы близки к целям покоординатно.
        let p = parameters_of(&ansatz).unwrap();
        for (i, (&pv, &t)) in p.iter().zip(&target).enumerate() {
            assert!((pv - t).abs() < 0.25, "coord {i}: p = {pv}, цель {t}");
        }
    }

    /// DoD RQ5: то же для statevector-движка.
    #[test]
    fn converges_within_ten_steps_statevector() {
        let target = vec![0.9, -0.9, 0.8, -0.8, 0.7, -0.7];
        let start = vec![0.1, -0.1, 0.2, -0.2, 0.0, 0.0];
        let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();
        assert_eq!(ansatz.engine().name(), "statevector");
        let l0 = param_loss(&ansatz, &target);

        let mut opt = BornOptimizer::new(0.25, 0.9, 8192).with_seed(3);
        opt.run(&mut ansatz, quadratic_target(&target), 10).unwrap();

        let l10 = param_loss(&ansatz, &target);
        assert!(
            l10 < l0 / 8.0,
            "потеря {l0:.4} → {l10:.4} — сходимость за 10 шагов недостаточна"
        );
        assert_eq!(ansatz.engine().name(), "statevector");
    }

    /// Монотонная сходимость параметрической потери (малый η, крупный батч).
    #[test]
    fn loss_decreases_monotonically() {
        let (start, target) = drift_setup(64);
        let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();
        let mut opt = BornOptimizer::new(0.2, 0.0, 16_384).with_seed(11);

        let mut prev = param_loss(&ansatz, &target);
        for step in 0..10 {
            opt.step(&mut ansatz, quadratic_target(&target)).unwrap();
            let li = param_loss(&ansatz, &target);
            assert!(
                li <= prev + 1e-12,
                "шаг {step}: потеря выросла {prev:.6} → {li:.6}"
            );
            prev = li;
        }
        assert!(prev < 1.0, "финальная потеря {prev:.4}");
    }

    /// Отслеживание regime-jump: цель разворачивается на полпути.
    ///
    /// Трение γ = 0.5: при сильном моменте (γ ≥ 0.8) фаза может «залипнуть»
    /// на полюсе p = ±1 — там dp/dθ = −sin θ = 0 и параметрический сдвиг
    /// вырождается (см. замечание о полюсах в документации модуля).
    #[test]
    fn regime_jump_is_tracked() {
        let start: Vec<f64> = vec![0.3; 32];
        let target_a = vec![0.9; 32];
        let target_b = vec![-0.9; 32];

        let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();
        let mut opt = BornOptimizer::new(0.25, 0.5, 4096).with_seed(5);

        opt.run(&mut ansatz, quadratic_target(&target_a), 5)
            .unwrap();
        let la = param_loss(&ansatz, &target_a);
        assert!(la < 0.2, "до прыжка: {la:.4}");

        // Скачок режима: цель инвертировалась.
        let lb0 = param_loss(&ansatz, &target_b);
        opt.run(&mut ansatz, quadratic_target(&target_b), 10)
            .unwrap();
        let lb = param_loss(&ansatz, &target_b);
        assert!(lb < lb0 / 100.0, "после прыжка: {lb0:.4} → {lb:.4}");
    }

    /// McWeeny-пурификация загоняет треки на чистые триты (P² = P).
    #[test]
    fn purify_every_k_sharpens_to_trits() {
        let target: Vec<f64> = (0..64)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let start: Vec<f64> = vec![0.3; 64];
        let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();

        let mut opt = BornOptimizer::new(0.2, 0.9, 4096)
            .with_seed(9)
            .with_purify_every(2);
        opt.run(&mut ansatz, quadratic_target(&target), 8).unwrap();

        let p = parameters_of(&ansatz).unwrap();
        for (i, &pv) in p.iter().enumerate() {
            assert!(pv.abs() > 0.999, "coord {i}: |p| = {pv:.6} — не трит");
        }
        // Инвариант идемпотентности: max |λ² − λ| → 0.
        let residual = pqw::mcweeny::idempotency_residual(p.iter().copied());
        assert!(residual < 1e-6, "residual = {residual:.2e}");
    }

    /// Π_Λ: дуги ниже порога ε заморожены (фон не учится).
    #[test]
    fn causality_projector_freezes_weak_arcs() {
        // Слабая дуга 0.05 < ε = 0.2 — не должна сдвинуться;
        // сильная 0.6 — учится.
        let start = vec![0.05, 0.6];
        let target = vec![0.9, 0.9];
        let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();
        let mut opt = BornOptimizer::new(0.3, 0.9, 8192)
            .with_seed(2)
            .with_epsilon(0.2);
        opt.run(&mut ansatz, quadratic_target(&target), 6).unwrap();

        let p = parameters_of(&ansatz).unwrap();
        assert!((p[0] - 0.05).abs() < 1e-12, "слабая дуга сдвинулась: {p:?}");
        assert!((p[1] - 0.9).abs() < 0.1, "сильная дуга не сошлась: {p:?}");
    }

    /// Побитовая воспроизводимость при фиксированном сиде.
    #[test]
    fn learning_is_bit_reproducible() {
        let (start, target) = drift_setup(128);
        let run_once = || {
            let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();
            let mut opt = BornOptimizer::new(0.2, 0.9, 2048).with_seed(2026);
            let reports = opt.run(&mut ansatz, quadratic_target(&target), 8).unwrap();
            (reports, parameters_of(&ansatz).unwrap())
        };
        let (rep_a, p_a) = run_once();
        let (rep_b, p_b) = run_once();
        assert_eq!(rep_a, rep_b);
        assert_eq!(p_a, p_b);
    }

    #[test]
    fn quadratic_target_gradient() {
        let f = quadratic_target(&[0.9, -0.9]);
        let g = f(&[0.4, -0.4]);
        assert!((g[0] - (-0.5)).abs() < 1e-15);
        assert!((g[1] - 0.5).abs() < 1e-15);
    }

    /// Пурификация не разрушает фон: координаты без дуг заморожены
    /// структурно (Π_Λ product-движка), пурификация — неподвижная точка 0.
    #[test]
    fn purify_keeps_background_fixed() {
        let pa = PhaseAnsatz::new(4, vec![(0, 0.4), (2, -0.4)]).unwrap();
        let mut ansatz = Ansatz::Product(pa);
        let target = vec![1.0, 0.0, -1.0, 0.0];
        let mut opt = BornOptimizer::new(0.2, 0.5, 4096)
            .with_seed(4)
            .with_purify_every(1);
        opt.run(&mut ansatz, quadratic_target(&target), 6).unwrap();
        let p = parameters_of(&ansatz).unwrap();
        assert_eq!(p[1], 0.0, "фон сдвинулся: {p:?}");
        assert_eq!(p[3], 0.0, "фон сдвинулся: {p:?}");
        assert!(p[0] > 0.99 && p[2] < -0.99, "дуги не очистились: {p:?}");
    }
}

#[cfg(test)]
mod active_inference_tests {
    use super::*;
    use crate::ansatz::LoadOptions;
    use crate::PhaseAnsatz;

    /// η(Σ) = η₀·e^(−βΣ): граничные значения и монотонность.
    #[test]
    fn eta_schedule_math_is_exact() {
        let ai = ActiveInference::new(0.3, 2.0, 64);
        assert!((ai.eta_at(0.0) - 0.3).abs() < 1e-15);
        assert!((ai.eta_at(1.0) - 0.3 * (-2.0_f64).exp()).abs() < 1e-15);
        assert!((ai.eta_at(0.5) - 0.3 * (-1.0_f64).exp()).abs() < 1e-15);
        // Монотонность по сюрпризу.
        assert!(ai.eta_at(0.2) > ai.eta_at(0.8));
        // Выход за [0, 1] клампится, а не NaN.
        assert!((ai.eta_at(-3.0) - 0.3).abs() < 1e-15);
        assert!((ai.eta_at(7.0) - 0.3 * (-2.0_f64).exp()).abs() < 1e-15);
        // β = 0 — постоянный шаг.
        let flat = ActiveInference::new(0.25, 0.0, 64);
        assert!((flat.eta_at(0.0) - flat.eta_at(1.0)).abs() < 1e-15);
    }

    /// Сюрприз: честные монеты дают Σ = 1, чистые триты — Σ = 0.
    #[test]
    fn surprise_spans_coin_to_trit() {
        let coins = Ansatz::from_phases(&[0.0, 0.0, 0.0, 0.0], &LoadOptions::default()).unwrap();
        assert!((ActiveInference::surprise(&coins) - 1.0).abs() < 1e-12);

        let trits = Ansatz::from_phases(&[1.0, -1.0, 1.0], &LoadOptions::default()).unwrap();
        assert!(ActiveInference::surprise(&trits).abs() < 1e-12);

        let mixed = Ansatz::from_phases(&[0.6, 0.0], &LoadOptions::default()).unwrap();
        let s = ActiveInference::surprise(&mixed);
        assert!(s > 0.0 && s < 1.0);

        // Фон product-движка не входит в Σ: одна сильная дуга на d = 64.
        let pa = PhaseAnsatz::new(64, vec![(7, 0.8)]).unwrap();
        let sparse = Ansatz::Product(pa);
        let s_sp = ActiveInference::surprise(&sparse);
        assert!(s_sp < 0.5, "sparse surprise = {s_sp}");
    }

    /// DoD RQ6: адаптивная петля сходится за ≤ 10 шагов (product, d = 512).
    #[test]
    fn adaptive_converges_within_ten_steps() {
        let (start, target) = drift_setup(512);
        let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();
        let l0 = param_loss(&ansatz, &target);

        let mut ai = ActiveInference::new(0.25, 2.0, 4096).with_seed(42);
        let reports = ai.run(&mut ansatz, quadratic_target(&target), 10).unwrap();
        assert_eq!(reports.len(), 10);

        let l10 = param_loss(&ansatz, &target);
        assert!(
            l10 < l0 / 20.0,
            "потеря {l0:.4} → {l10:.4} — адаптивная сходимость недостаточна"
        );
        // Расписание: η ≤ η₀ и растёт по мере падения сюрприза.
        assert!(reports.iter().all(|r| r.eta <= 0.25 + 1e-12));
        assert!(reports.last().unwrap().eta > reports.first().unwrap().eta);
        assert!(reports.last().unwrap().surprise < reports.first().unwrap().surprise);
    }

    /// Регламент RQ5: γ = 0.5 по умолчанию отслеживает инверсию цели.
    #[test]
    fn regime_jump_tracked_at_gamma_half() {
        let start: Vec<f64> = vec![0.3; 32];
        let target_a = vec![0.9; 32];
        let target_b = vec![-0.9; 32];

        let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();
        let mut ai = ActiveInference::new(0.25, 1.0, 4096).with_seed(5);
        assert!((ai.gamma() - 0.5).abs() < 1e-12, "γ по умолчанию не 0.5");

        ai.run(&mut ansatz, quadratic_target(&target_a), 5).unwrap();
        assert!(param_loss(&ansatz, &target_a) < 0.2);

        let lb0 = param_loss(&ansatz, &target_b);
        ai.run(&mut ansatz, quadratic_target(&target_b), 10)
            .unwrap();
        let lb = param_loss(&ansatz, &target_b);
        assert!(lb < lb0 / 100.0, "после прыжка: {lb0:.4} → {lb:.4}");
        // Фазы не залипли на полюсах: все координаты перешли знак.
        let p = parameters_of(&ansatz).unwrap();
        assert!(
            p.iter().all(|&v| v < -0.5),
            "инверсия не отслежена: {}",
            p.iter().filter(|v| **v > 0.0).count()
        );
    }

    /// McWeeny каждые K = 5 шагов: флаги purification и чистые триты.
    ///
    /// γ = 0.5 (регламент RQ6): сильный момент (γ ≥ 0.8) осциллирует
    /// у полюса |p| = 1 и не даёт пурификации дожать треки до
    /// residual < 1e−6 за разумное число шагов.
    #[test]
    fn purify_every_five_steps() {
        let target: Vec<f64> = (0..64)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let mut ansatz = Ansatz::from_phases(&[0.3; 64], &LoadOptions::default()).unwrap();
        // γ = 0.5 по умолчанию, β = 0: тест про период K = 5.
        let mut ai = ActiveInference::new(0.25, 0.0, 4096).with_seed(9);

        let reports = ai.run(&mut ansatz, quadratic_target(&target), 20).unwrap();
        // Шаги 5, 10, 15, 20 — пурифицированные, остальные нет.
        assert_eq!(reports.len(), 20);
        assert!((ai.gamma() - 0.5).abs() < 1e-12);
        for r in &reports {
            let expect = r.step % 5 == 0;
            assert_eq!(r.purified, expect, "шаг {}", r.step);
        }
        let p = parameters_of(&ansatz).unwrap();
        for (i, &pv) in p.iter().enumerate() {
            assert!(pv.abs() > 0.999, "coord {i}: |p| = {pv:.6} — не трит");
        }
        let residual = pqw::mcweeny::idempotency_residual(p.iter().copied());
        assert!(residual < 1e-6, "residual = {residual:.2e}");
    }

    /// Π_Λ: слабые дуги заморожены, сильные учатся (как в BornOptimizer).
    #[test]
    fn epsilon_freezes_weak_arcs() {
        let start = vec![0.05, 0.6];
        let target = vec![0.9, 0.9];
        let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();
        // β = 0: плоское расписание — тест про Π_Λ, а не про адаптацию.
        let mut ai = ActiveInference::new(0.3, 0.0, 8192)
            .with_seed(2)
            .with_epsilon(0.2)
            .with_purify_every(0)
            .with_gamma(0.9);
        ai.run(&mut ansatz, quadratic_target(&target), 6).unwrap();
        let p = parameters_of(&ansatz).unwrap();
        assert!((p[0] - 0.05).abs() < 1e-12, "слабая дуга сдвинулась: {p:?}");
        assert!((p[1] - 0.9).abs() < 0.1, "сильная дуга не сошлась: {p:?}");
    }

    /// Побитовая воспроизводимость адаптивной траектории.
    #[test]
    fn adaptive_is_bit_reproducible() {
        let (start, target) = drift_setup(128);
        let run_once = || {
            let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();
            let mut ai = ActiveInference::new(0.2, 2.0, 2048).with_seed(2026);
            let reports = ai.run(&mut ansatz, quadratic_target(&target), 8).unwrap();
            (reports, parameters_of(&ansatz).unwrap())
        };
        let (rep_a, p_a) = run_once();
        let (rep_b, p_b) = run_once();
        assert_eq!(rep_a, rep_b);
        assert_eq!(p_a, p_b);
        // Память расписания синхронна с последним отчётом.
        let mut ai = ActiveInference::new(0.2, 2.0, 2048).with_seed(2026);
        let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();
        ai.step(&mut ansatz, quadratic_target(&target)).unwrap();
        assert!((ai.last_surprise() - rep_a[0].surprise).abs() < 1e-15);
        assert!((ai.last_eta() - rep_a[0].eta).abs() < 1e-15);
    }

    /// Длина замыкания валидируется (паритет с BornOptimizer).
    #[test]
    fn closure_length_is_validated() {
        let mut ai = ActiveInference::new(0.1, 1.0, 8);
        let mut ansatz = Ansatz::from_phases(&[0.1, 0.2], &LoadOptions::default()).unwrap();
        assert!(matches!(
            ai.step(&mut ansatz, |_| vec![0.0]),
            Err(PqcError::LengthMismatch { expected: 2, .. })
        ));
    }

    /// В хаосе шаг мал, у стационара — полный: β реально управляет трением.
    #[test]
    fn eta_shrinks_with_surprise_in_live_loop() {
        // Старт из честных монет: Σ = 1 → η = η₀·e^(−β) строго меньше η₀.
        let mut ansatz = Ansatz::from_phases(&[0.0; 16], &LoadOptions::default()).unwrap();
        let mut ai = ActiveInference::new(0.25, 3.0, 4096).with_seed(1);
        let r = ai
            .step(&mut ansatz, quadratic_target(&vec![0.9; 16]))
            .unwrap();
        assert!((r.surprise - 1.0).abs() < 1e-12);
        assert!((r.eta - 0.25 * (-3.0_f64).exp()).abs() < 1e-12);

        // Старт из чистых тритов: Σ = 0 → η = η₀ (полная скорость у стационара).
        let mut trit_ansatz =
            Ansatz::from_phases(&[1.0, -1.0, 1.0, -1.0], &LoadOptions::default()).unwrap();
        let mut ai2 = ActiveInference::new(0.25, 3.0, 4096).with_seed(1);
        let r2 = ai2
            .step(&mut trit_ansatz, quadratic_target(&[1.0, -1.0, 1.0, -1.0]))
            .unwrap();
        assert!(r2.surprise.abs() < 1e-12);
        assert!((r2.eta - 0.25).abs() < 1e-15);
    }
}
