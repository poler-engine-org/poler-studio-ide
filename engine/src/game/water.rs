//! # Water — спектральная гидродинамика (цикл V1, v0.58.0)
//!
//! Разбор владельца «OpenAI решила задачу» (2026-09-24):
//!
//! ```text
//!   OpenAI (brute force)              POLER WATER (спектр)
//!   ─────────────────                 ────────────────────
//!   10 000 агентов, 88 часов,         состояние = K мод поля
//!   тераватты под столом              (байты, не гигабайты)
//!   UE/Unity: плотные сетки f32,      эволюция = целочисленные
//!   Re^(9/4) ≈ 10^14 точек,           трит-операции на кремнии
//!   дрейф округлений, «резина»        (счётчик фаз — фикс-точка)
//! ```
//!
//! Принцип владельца: «Компьютеру не нужно перебирать бесконечность
//! или плодить миллионы float-матриц. Плавающая точка — это кремний.
//! Нужно дать условия и инварианты, услышать уравнение через
//! вязкость и энергию».
//!
//! Физика (всё честно, без магии):
//!
//! ```text
//!   1. Состояние — спектр гравитационно-капиллярных волн:
//!      h(x,t) = Σ_k A_k · cos(k·x + φ_k(t)),
//!      дисперсия глубоководная: ω(k) = √(g·k + (σ/ρ)·k³).
//!   2. Равновесие — каскад Колмогорова: A(k) ∝ k^(−5/6)
//!      (E(k) ∝ k^(−5/3)), окно ветра вокруг k_p = g/v²
//!      (пик Пирсона–Московица), H_s ≈ 0.21·v²/g.
//!   3. Вязкость (Навье–Стокс, Ламб): A(t) = A₀·e^(−2νk²t) —
//!      в лог-уровнях амплитуды это линейный спуск.
//!   4. Ветер: релаксация к равновесию за τ_wind —
//!      стационарный баланс «накачка ↔ диссипация».
//! ```
//!
//! Кремний (главное): фаза живёт в **целочисленном счётчике**
//! микро-секторов 3^(p+m) с 32 дробными битами фикс-точки.
//! Шаг эволюции — одно целочисленное вычитание и один rem_euclid:
//! ошибка ≤ 2⁻³³ сектора на шаг и практически не накапливается
//! (дрейф f32-аккумулятора на тех же шагах — порядка радиан;
//! см. тест `f32_drifts_trit_counter_does_not`). Амплитуда —
//! тернарная лестница 3^t лог-уровней с фикс-точечным накопителем;
//! релаксация τ_wind сама демпфирует отклонения — устойчиво
//! по построению.
//!
//! Контейнер — тот же VRTX, что у кодека V0: спектр есть спектр.
//! `to_vortex()` раскладывает каждую волну в эрмитову пару ±k,
//! `vortex::synthesize` даёт поле высот; `from_vortex` собирает
//! пары обратно (физика мод восстанавливается из параметров,
//! равновесие принимается равным текущему уровню — контейнер
//! хранит видимый кадр спектра). Течения для геймплея —
//! аналитические орбитальные скорости линейной теории (точные
//! производные потенциала, без конечных разностей): потенциальное
//! течение, завихрённость нуль по построению (тест меряет curl
//! численно).
//!
//! # Цикл X «Обрушение» (v0.60.0): нелинейность и вихри
//!
//! То, что OpenAI решала тераваттами брут-форсом — здесь спектральная
//! нелинейная гидродинамика на целочисленном кремнии:
//!
//! ```text
//!   1. Стокс, 2-й порядок:  h₂ = (k·A²/2)·cos 2θ — гребень острее,
//!      ложбина площе; частотный сдвиг ω·(1 + (kA)²/2) — в том же
//!      целочисленном счётчике фаз (без дрейфа — по-прежнему).
//!   2. Лимит Мичелла: наклон моды k·A > s_break — снос гребня
//!      (спиллинг): срез уровня до 0.9·порога, энергия события —
//!      бухгалтерски: когерентный вихрь + тепло (тест закрывает).
//!   3. Белые барашки: наблюдательный триггер по Бофорту (первые
//!      барашки ≈ 9 м/с; как source-term диссипации океанских
//!      моделей — эмпирика триггера, физика вихря честная).
//!   4. Вихрь обрушения = Ламб–Озеен на фикс-точке: ядро расплывается
//!      r² = r₀² + 8νt, лагранжева адвекция полем волн (дрейф
//!      Стокса возникает сам), растяжение деформацией
//!      dΓ/dt = Γ·∂u/∂x с клипом (конечное время деформации).
//!   5. Поверхностный след барашка — горб напора скорости ядра:
//!      Δh = Γ²/(4π²·g·r²)·exp(−d²/2r²) — в height_field/surface_at.
//!   6. Завихренность ≠ 0: flow_full_at — первое РОТАЦИОННОЕ поле
//!      движка (циркуляция по контуру = Γ·(1−e^(−d²/r²)), теорема
//!      Стокса); волновая часть остаётся безвихревой (flow_at).
//! ```
//!
//! Контейнер VRTX хранит видимый кадр ЛИНЕЙНОГО спектра (эрмитовы
//! пары ±k — раунд-трип бит-в-бит); вторые гармоники Стокса живут в
//! собственной лог-шкале (они на 1–2 порядка слабее фундаментальных)
//! и сэмплируются в height_field вторым FFT-проходом; слой барашков —
//! состояние движка (сайдкар хэша детерминизма, не контейнер).
//! Верность квантования честно меряется PSNR против f64-эталона
//! с теми же уравнениями ([`WaterF64`]).

use std::collections::{HashMap, HashSet};
use std::f64::consts::{PI, TAU};
use super::vortex::{self, Rng};

/// Фикс-точка состояний: 32 дробных бита.
const FP: f64 = 4294967296.0; // 2^32
const FP_I: i64 = 1i64 << 32;

// ---------------------------------------------------------------------------
// Параметры
// ---------------------------------------------------------------------------

/// Параметры водной поверхности. Физические величины — в СИ.
#[derive(Debug, Clone)]
pub struct WaterParams {
    /// Сетка синтеза n×n (степень двойки).
    pub n: usize,
    /// Число активных мод (волн).
    pub modes: usize,
    /// Сид детерминизма.
    pub seed: u64,
    /// Скорость ветра, м/с — задаёт пик спектра k_p = g/v² и H_s.
    pub wind: f64,
    /// Время релаксации к равновесию, с (игровой масштаб развития моря).
    pub tau_wind: f64,
    /// Кинематическая вязкость, м²/с (пресная вода ≈ 1e-6).
    pub viscosity: f64,
    /// Ускорение свободного падения, м/с².
    pub gravity: f64,
    /// Капиллярность σ/ρ, м³/с² (вода ≈ 7.4e-5).
    pub capillary: f64,
    /// Физический размер домена, м.
    pub domain: f64,
    /// Направление ветра, рад (0 = +x).
    pub wind_dir: f64,
    /// Угловой разброс волн вокруг ветра, рад.
    pub spread: f64,
    /// Триты амплитуды: 3^t лог-уровней лестницы.
    pub amp_trits: u32,
    /// Триты фазы (сектора): 3^p на оборот.
    pub phase_trits: u32,
    /// Суб-секторные триты: счётчик живёт в 3^(p+m) микро-секторах.
    pub micro_trits: u32,
    /// Нелинейность: 2-я гармоника Стокса, сдвиг частоты, обрушение.
    pub nonlinear: bool,
    /// Предельный наклон моды k·A (спиллинг): угол Стокса 120° ≈ 0.58,
    /// Мичелл H/λ = 1/7; консервативный порог сноса гребня.
    pub steepness_break: f64,
    /// Ветер начала белых барашков, м/с (Бофорт 5: первые барашки).
    pub whitecap_onset: f64,
    /// Частота барашков при превышении порога на 6 м/с, 1/с
    /// (калибровка: шторм 20 м/с теряет ~15% энергии моря в барашки).
    pub whitecap_rate: f64,
    /// Доля циркуляции бара в когерентном вихре (остальное — тепло).
    pub breaker_alpha: f64,
    /// Доля энергии события обрушения, уходящая в тепло (не в вихрь).
    pub breaker_heat_frac: f64,
    /// Ёмкость слоя барашков (максимум живых вихрей).
    pub max_breakers: usize,
}

impl Default for WaterParams {
    fn default() -> Self {
        WaterParams {
            n: 256,
            modes: 192,
            seed: 42,
            wind: 8.0,
            tau_wind: 30.0,
            viscosity: 1e-6,
            gravity: 9.81,
            capillary: 7.4e-5,
            domain: 100.0,
            wind_dir: 0.0,
            spread: PI / 5.0,
            amp_trits: 6,
            phase_trits: 6,
            micro_trits: 3,
            nonlinear: true,
            steepness_break: 0.32,
            whitecap_onset: 9.0,
            whitecap_rate: 0.1,
            breaker_alpha: 0.3,
            breaker_heat_frac: 0.5,
            max_breakers: 24,
        }
    }
}

impl WaterParams {
    /// Валидация параметров (общая для либы и shell).
    pub fn validate(&self) -> Result<(), String> {
        if !(16..=1024).contains(&self.n) || !self.n.is_power_of_two() {
            return Err("water: n — степень двойки 16..=1024".into());
        }
        if !(1..=16384).contains(&self.modes) {
            return Err("water: modes 1..=16384".into());
        }
        if !(0.5..=60.0).contains(&self.wind) {
            return Err("water: wind 0.5..=60 м/с".into());
        }
        if !(0.5..=3600.0).contains(&self.tau_wind) {
            return Err("water: tau-wind 0.5..=3600 с".into());
        }
        if !(1e-8..=1e-1).contains(&self.viscosity) {
            return Err("water: viscosity 1e-8..=1e-1 м²/с".into());
        }
        if !(0.1..=1000.0).contains(&self.domain) {
            return Err("water: domain 0.1..=1000 м".into());
        }
        if !(2..=8).contains(&self.amp_trits) {
            return Err("water: amp-trits 2..=8".into());
        }
        if !(2..=10).contains(&self.phase_trits) {
            return Err("water: phase-trits 2..=10".into());
        }
        if self.micro_trits != 3 {
            return Err("water: micro-trits = 3 (фикс-точка счётчика)".into());
        }
        if !(0.05..=0.60).contains(&self.steepness_break) {
            return Err("water: steepness-break 0.05..=0.60".into());
        }
        if !(0.0..=40.0).contains(&self.whitecap_onset) {
            return Err("water: whitecap-onset 0..=40 м/с".into());
        }
        if !(0.0..=20.0).contains(&self.whitecap_rate) {
            return Err("water: whitecap-rate 0..=20 1/с".into());
        }
        if !(0.05..=2.0).contains(&self.breaker_alpha) {
            return Err("water: breaker-alpha 0.05..=2.0".into());
        }
        if !(0.0..=0.95).contains(&self.breaker_heat_frac) {
            return Err("water: breaker-heat-frac 0..=0.95".into());
        }
        if self.max_breakers > 64 {
            return Err("water: max-breakers 0..=64".into());
        }
        if !(self.gravity > 0.0) || !(self.capillary >= 0.0) || !(self.spread > 0.0) {
            return Err("water: физические константы должны быть положительными".into());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Состояние: целочисленный спектр
// ---------------------------------------------------------------------------

/// Одна волна спектра. Состояние — только целые числа;
/// физика моды прекомпьютедна (f64-константы, не состояние).
#[derive(Debug, Clone)]
pub struct WaterMode {
    /// Бин Фурье по x, 1..=n/2 (канонический представитель пары ±k).
    pub kx: i32,
    /// Бин Фурье по y, −n/2..=n/2.
    pub ky: i32,
    /// Фикс-точечный накопитель уровня амплитуды (единица = 2⁻³² лестницы).
    pub acc: i64,
    /// Фикс-точечный счётчик фазы: микро-сектора·2³²; убывает — волна
    /// бежит по +k (по ветру).
    pub phase_fp: i64,
    /// Физическое волновое число |k|, рад/м.
    pub k: f64,
    /// Физическая проекция k на ось x, рад/м.
    pub kx_phys: f64,
    /// Физическая проекция k на ось y, рад/м.
    pub ky_phys: f64,
    /// Частота по дисперсионному соотношению, рад/с.
    pub omega: f64,
    /// Равновесный уровень лестницы (окно ветра × K41).
    pub u_eq: f64,
    /// Вязкий спуск по лестнице, уровней/с (константа ≥ 0).
    pub decay_rate: f64,
    /// Кулдаун после сноса гребня, фикс-точка секунд (3 периода моды).
    pub cooldown_fp: i64,
}

/// Когерентный вихрь обрушения (барашек). Состояние — только целые
/// числа фикс-точки 2³²; физика — Ламб–Озеен:
///
/// ```text
///   ядро:       r²(t) = r₀² + 8νt          (вязкое расплывание)
///   адвекция:   лагранжева, полем волн      (дрейф Стокса сам)
///   растяжение: dΓ/dt = Γ·∂u/∂x, клип ±0.25/шаг, потолок 2·Γ₀
///   энергия:    E = Γ²/(8π) на единицу площади (деление ρ опущено)
///   след:       Δh = Γ²/(4π²·g·r²)·e^(−d²/2r²) (напор скорости ядра)
/// ```
///
/// Знак Γ — детерминированная конвенция «верх бара с волной» (− для
/// волн по +x); физика завихренности от знака не зависит.
#[derive(Debug, Clone)]
pub struct BreakerVortex {
    /// Позиция x, фикс-точка метров.
    pub x_fp: i64,
    /// Позиция y, фикс-точка метров.
    pub y_fp: i64,
    /// Циркуляция Γ (знаковая), фикс-точка м²/с.
    pub gamma_fp: i64,
    /// Циркуляция рождения |Γ₀| — потолок растяжения, фикс-точка.
    pub gamma0_fp: i64,
    /// Радиус ядра r, фикс-точка метров.
    pub r_fp: i64,
    /// Возраст, фикс-точка секунд.
    pub age_fp: i64,
    /// Время жизни (5 периодов волны-источника), фикс-точка секунд.
    pub ttl_fp: i64,
}

impl BreakerVortex {
    /// Позиция центра, м.
    pub fn pos(&self) -> (f64, f64) {
        (self.x_fp as f64 / FP, self.y_fp as f64 / FP)
    }

    /// Циркуляция Γ, м²/с (знаковая).
    pub fn gamma(&self) -> f64 {
        self.gamma_fp as f64 / FP
    }

    /// Радиус ядра, м.
    pub fn radius(&self) -> f64 {
        self.r_fp as f64 / FP
    }

    /// Возраст, с.
    pub fn age(&self) -> f64 {
        self.age_fp as f64 / FP
    }

    /// Время жизни, с.
    pub fn ttl(&self) -> f64 {
        self.ttl_fp as f64 / FP
    }

    /// Энергия вихря на единицу площади (деление на ρ опущено), м³/с².
    pub fn energy(&self) -> f64 {
        let g = self.gamma();
        g * g / (8.0 * PI)
    }
}

/// Водная поверхность как спектр волн: состояние в целых числах.
#[derive(Debug, Clone)]
pub struct WaterSpectrum {
    pub n: usize,
    pub amp_trits: u32,
    pub phase_trits: u32,
    /// Микро-секторов на оборот: 3^(p+m).
    pub micro_total: i64,
    /// Границы лог-окна амплитуд в Place-единицах (бины Фурье: A·n²/2).
    pub a_min: f64,
    pub a_max: f64,
    /// Уровней в лестнице: 3^t − 1.
    pub level_max: i64,
    /// Натуральных логарифмов на уровень лестницы.
    pub ln_per_level: f64,
    /// Время релаксации к равновесию, с.
    pub tau_wind: f64,
    /// Физический размер домена, м.
    pub domain: f64,
    pub modes: Vec<WaterMode>,
    /// Целочисленный счётчик шагов.
    pub steps: u64,
    /// Целевая дисперсия высот, м² (нормировка спектра по ветру).
    pub variance_target: f64,
    /// Слой барашков (состояние движка, не контейнера).
    pub breakers: Vec<BreakerVortex>,
    /// RNG барашков — продолжение сида генерации (детерминизм событий).
    pub rng: Rng,
    /// Кумулятивная диссипация в тепло, фикс-точка (энергия/ρ на площадь).
    pub heat_fp: i64,
    /// Аккумулятор долей событий белых барашков, фикс-точка.
    pub whitecap_acc_fp: i64,
    /// Счётчик рождённых барашков за всё время.
    pub spawned_total: u64,
    // Физические константы (не состояние):
    /// Гравитация, м/с².
    pub gravity: f64,
    /// Кинематическая вязкость, м²/с.
    pub viscosity: f64,
    /// Скорость ветра, м/с (триггер барашков по Бофорту).
    pub wind: f64,
    /// Нелинейность включена.
    pub nonlinear: bool,
    /// Предельный наклон моды (Мичелл).
    pub steepness_break: f64,
    /// Ветер начала белых барашков, м/с.
    pub whitecap_onset: f64,
    /// Частота барашков при превышении на 6 м/с, 1/с.
    pub whitecap_rate: f64,
    /// Доля циркуляции бара в когерентном вихре.
    pub breaker_alpha: f64,
    /// Доля энергии события в тепло.
    pub breaker_heat_frac: f64,
    /// Ёмкость слоя барашков.
    pub max_breakers: usize,
    /// Статические пропуски вторых гармоник (DC/Найквист/коллизии).
    pub second_skip: Vec<bool>,
}

impl WaterSpectrum {
    /// Видимый уровень лестницы амплитуды.
    pub fn level(&self, m: &WaterMode) -> i64 {
        (m.acc >> 32).clamp(0, self.level_max)
    }

    /// Амплитуда волны в Place-единицах (значение бина Фурье): A·n²/2.
    fn amp_place(&self, m: &WaterMode) -> f64 {
        self.amp_place_at_level(self.level(m))
    }

    /// Амплитуда в Place-единицах по уровню лестницы.
    fn amp_place_at_level(&self, u: i64) -> f64 {
        let u = u.clamp(0, self.level_max) as f64;
        self.a_min * (self.a_max / self.a_min).powf(u / self.level_max as f64)
    }

    /// Физическая амплитуда волны, м.
    pub fn amplitude(&self, m: &WaterMode) -> f64 {
        2.0 * self.amp_place(m) / (self.n * self.n) as f64
    }

    /// Фаза волны, рад — в точности та, что видит контейнер и синтез
    /// (микро-сектор фикс-точки, дробные биты — только для эволюции).
    pub fn phase(&self, m: &WaterMode) -> f64 {
        let s = (m.phase_fp >> 32).rem_euclid(self.micro_total);
        TAU * s as f64 / self.micro_total as f64
    }

    /// Дисперсия высот поверхности, м².
    pub fn variance(&self) -> f64 {
        self.modes
            .iter()
            .map(|m| {
                let a = self.amplitude(m);
                a * a / 2.0
            })
            .sum()
    }

    /// Значимая высота волн H_s = 4σ, м.
    pub fn significant_height(&self) -> f64 {
        4.0 * self.variance().sqrt()
    }

    /// Один шаг эволюции. Фаза — целочисленно (без дрейфа), амплитуда —
    /// фикс-точечная лестница с релаксацией к равновесию и вязким спуском.
    /// Цикл X: сдвиг частоты Стокса, снос гребней (Мичелл), белые барашки
    /// (Бофорт), эволюция вихрей Ламб–Озеена (адвекция/растяжение/вязкость).
    pub fn step(&mut self, dt: f64) {
        assert!(dt.is_finite() && dt > 0.0, "water: dt должен быть конечным > 0");
        let wrap = self.micro_total * FP_I;
        let top = self.level_max * FP_I;
        let dt_fp = (dt * FP).round() as i64;
        // Лестница амплитуд как замыкание (копии констант — без borrow):
        // то же выражение, что amp_at_acc — биты совпадают.
        let (a_min, a_max, level_max, n2) =
            (self.a_min, self.a_max, self.level_max, (self.n * self.n) as f64);
        let amp_at = move |acc: i64| -> f64 {
            let u = (acc >> 32).clamp(0, level_max) as f64;
            2.0 * a_min * (a_max / a_min).powf(u / level_max as f64) / n2
        };
        let mut slopes = vec![0.0f64; self.modes.len()];
        for (i, m) in self.modes.iter_mut().enumerate() {
            // Фаза: вычитаем ω·dt в микро-секторах фикс-точки.
            // round() константы — единственный источник ошибки (≤ 2⁻³³
            // сектора на шаг); суммирование — точные целочисленные операции.
            // Цикл X: нелинейный сдвиг Стокса ω·(1 + (kA)²/2) — в том же
            // счётчике, та же целочисленность (сдвиг ограничен: kA < 0.6).
            let a = amp_at(m.acc);
            let ka = m.k * a;
            let shift = if self.nonlinear { 1.0 + 0.5 * ka * ka } else { 1.0 };
            let adv =
                (m.omega * shift * dt * self.micro_total as f64 / TAU * FP).round() as i64;
            m.phase_fp = (m.phase_fp - adv).rem_euclid(wrap);
            // Амплитуда: du/dt = (u_eq − u)/τ_wind − 2νk²/L.
            let u = m.acc as f64 / FP;
            let du = (m.u_eq - u) / self.tau_wind - m.decay_rate;
            m.acc = (m.acc + (du * dt * FP).round() as i64).clamp(0, top);
            // Кулдаун сноса гребня.
            if m.cooldown_fp > 0 {
                m.cooldown_fp = (m.cooldown_fp - dt_fp).max(0);
            }
            slopes[i] = ka;
        }
        if self.nonlinear {
            // Снос сверхкрутых гребней (Мичелл) — события с кулдауном.
            self.michell_events(&slopes);
            // Наклоны после сносов — для барашков и бухгалтерии.
            for (i, m) in self.modes.iter().enumerate() {
                slopes[i] = m.k * self.amp_at_acc(m.acc);
            }
            // Белые барашки: наблюдательный триггер по Бофорту.
            self.whitecap_events(dt, &slopes);
        }
        // Вихри: адвекция, растяжение, вязкость, ретировка.
        self.evolve_breakers(dt);
        self.steps += 1;
    }

    // -----------------------------------------------------------------------
    // Цикл X: обрушение гребней и вихри Ламб–Озеена
    // -----------------------------------------------------------------------

    /// Физическая амплитуда по аккумулятору уровня, м.
    fn amp_at_acc(&self, acc: i64) -> f64 {
        2.0 * self.amp_place_at_level(acc >> 32) / (self.n * self.n) as f64
    }

    /// Снос сверхкрутых гребней (лимит Мичелла): срез уровня до 0.9·порога,
    /// энергия события — в когерентный вихрь + тепло; кулдаун 3 периода.
    fn michell_events(&mut self, slopes: &[f64]) {
        let n2 = (self.n * self.n) as f64 / 2.0;
        for i in 0..self.modes.len() {
            if slopes[i] <= self.steepness_break || self.modes[i].cooldown_fp > 0 {
                continue;
            }
            let m = self.modes[i].clone();
            // Цель: наклон 0.9·порога — спиллинг уводит под предел.
            let place_target = 0.9 * self.steepness_break / m.k * n2;
            let u_target = if place_target > self.a_min {
                ((place_target / self.a_min).ln() / self.ln_per_level)
                    .clamp(0.0, self.level_max as f64)
            } else {
                0.0
            };
            let acc_before = self.modes[i].acc;
            if acc_before == 0 {
                continue; // ниже пола лестницы — нечего сносить
            }
            // Срез минимум на один уровень (лестница грубая внизу).
            let acc_after = ((u_target * FP).round() as i64).min(acc_before - 1).max(0);
            let a_before = self.amp_at_acc(acc_before);
            let a_after = self.amp_at_acc(acc_after);
            let de = 0.5 * self.gravity * (a_before * a_before - a_after * a_after);
            self.modes[i].acc = acc_after;
            let period = TAU / m.omega;
            self.modes[i].cooldown_fp = (3.0 * period * FP).round() as i64;
            // Бухгалтерия события: вихрь + тепло.
            self.spawn_breaker(&m, de);
        }
    }

    /// Белые барашки: триггер по Бофорту (первые барашки ≈ 9 м/с),
    /// частота растёт квадратично с превышением ветра. Фикс-точечный
    /// аккумулятор долей событий — дискретизация честная и детерминированная.
    fn whitecap_events(&mut self, dt: f64, slopes: &[f64]) {
        if self.max_breakers == 0 || !(self.wind > self.whitecap_onset) {
            return;
        }
        let over = (self.wind - self.whitecap_onset) / 6.0;
        let rate = self.whitecap_rate * over * over;
        self.whitecap_acc_fp += (rate * dt * FP).round() as i64;
        while self.whitecap_acc_fp >= FP_I {
            self.whitecap_acc_fp -= FP_I;
            self.whitecap_event(slopes);
        }
    }

    /// Одно событие белого барашка: размер — доля H_s (наблюдательно:
    /// зрелые барашки шторма ~0.3–0.45·H_s, редкие ~0.1·H_s), позиция
    /// равномерна по домену, источник — доминирующая мода. Цикл X:
    /// событие берёт ровно столько энергии, сколько реально собрали
    /// с мод (лестница может не покрыть цель — барашек честно
    /// уменьшается, энергия не берётся из ниоткуда).
    fn whitecap_event(&mut self, slopes: &[f64]) {
        let over = ((self.wind - self.whitecap_onset) / 6.0).clamp(0.0, 3.0);
        let u = self.rng.next_f64();
        let hs = self.significant_height();
        if !(hs > 0.0) {
            return; // море ещё не построилось — событие пустое
        }
        let h_b =
            (hs * (0.08 + 0.30 * over) * (0.6 + 0.8 * u)).clamp(hs * 0.03, hs * 0.45);
        let gamma_target = self.breaker_alpha * (self.gravity * h_b * h_b * h_b).sqrt();
        let e_target = gamma_target * gamma_target / (8.0 * PI) / (1.0 - self.breaker_heat_frac);
        // Оплата: фактический сбор с уровней лестницы.
        let paid = self.pay_from_modes(e_target, slopes);
        if !(paid > 0.0) {
            return; // платить нечем — события нет
        }
        let e_v = paid * (1.0 - self.breaker_heat_frac);
        self.heat_fp += ((paid - e_v) * FP).round() as i64;
        let gamma = (8.0 * PI * e_v).sqrt();
        // Позиция и источник.
        let x = self.rng.next_f64() * self.domain;
        let y = self.rng.next_f64() * self.domain;
        let src = self
            .modes
            .iter()
            .enumerate()
            .max_by(|a, b| {
                let aa = self.amp_place(a.1);
                let bb = self.amp_place(b.1);
                aa.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        let m = self.modes[src].clone();
        let v = self.make_breaker(x, y, gamma, &m);
        self.push_breaker(v);
    }

    /// Съём энергии с мод пропорционально наклону² (dE = 2·E·ln_per_level·du
    /// на уровнях лестницы — то же соотношение, что в f64-эталоне).
    /// Возвращает ФАКТИЧЕСКИ уплаченное: измеримо по тем же уровням
    /// лестницы, что читает variance() — бухгалтерия события закрывается
    /// точно (ceil-сбор невозможен: clamp аккумулятора на нуле).
    fn pay_from_modes(&mut self, e_total: f64, slopes: &[f64]) -> f64 {
        let w: f64 = slopes.iter().map(|s| s * s).sum();
        if !(w > 0.0) {
            return 0.0; // мёртвое море — платить нечем
        }
        let mut paid = 0.0f64;
        for (i, s) in slopes.iter().enumerate() {
            let share = e_total * s * s / w;
            let m = &self.modes[i];
            let a = self.amp_at_acc(m.acc);
            let e_i = 0.5 * self.gravity * a * a;
            if !(e_i > 0.0) {
                continue;
            }
            let du = share / (2.0 * e_i * self.ln_per_level);
            let acc_b = self.modes[i].acc;
            let acc_a = (acc_b - (du * FP).round() as i64).max(0);
            // Измеримо: ровно столько энергии отдали уровни лестницы.
            let a_b = self.amp_at_acc(acc_b);
            let a_a = self.amp_at_acc(acc_a);
            paid += 0.5 * self.gravity * (a_b * a_b - a_a * a_a);
            self.modes[i].acc = acc_a;
        }
        paid
    }

    /// Рождение вихря обрушения из энергии события (Мичелл):
    /// Γ из E = Γ²/8π; позиция — гребень моды (θ ≡ 0).
    fn spawn_breaker(&mut self, m: &WaterMode, de: f64) {
        let e_v = de * (1.0 - self.breaker_heat_frac);
        self.heat_fp += (de * self.breaker_heat_frac * FP).round() as i64;
        if !(e_v > 0.0) || self.max_breakers == 0 {
            return;
        }
        let gamma = (8.0 * PI * e_v).sqrt();
        let phi = self.phase(m);
        let (x, y) = self.crest_point(m, phi);
        let v = self.make_breaker(x, y, gamma, m);
        self.push_breaker(v);
    }

    /// Конструктор вихря: радиус ядра ~ 0.75·h_b (но не меньше ячейки
    /// сетки — след обязан быть видимым), жизнь 5 периодов источника.
    fn make_breaker(&self, x: f64, y: f64, gamma: f64, m: &WaterMode) -> BreakerVortex {
        let h_b = (gamma / self.breaker_alpha).powf(2.0 / 3.0) / self.gravity.powf(1.0 / 3.0);
        let r0 = (0.75 * h_b).max(1.5 * self.domain / self.n as f64);
        let ttl = 5.0 * TAU / m.omega;
        // Знак Γ — конвенция «верх бара с волной» (− для волн по +x).
        let sign = if m.kx_phys >= 0.0 { -1.0 } else { 1.0 };
        BreakerVortex {
            x_fp: (x * FP).round() as i64,
            y_fp: (y * FP).round() as i64,
            gamma_fp: (sign * gamma * FP).round() as i64,
            gamma0_fp: (gamma * FP).round() as i64,
            r_fp: (r0 * FP).round() as i64,
            age_fp: 0,
            ttl_fp: (ttl * FP).round() as i64,
        }
    }

    /// Точка гребня моды (θ ≡ 0): младшая ось фиксируется у центра
    /// домена, старшая подтягивается к центру выбором ближайшего
    /// периода гребня (домен периодичен — условие сохраняется точно).
    fn crest_point(&self, m: &WaterMode, phi: f64) -> (f64, f64) {
        let d = self.domain;
        let near = |mut v: f64, per: f64| -> f64 {
            while v - 0.5 * d > 0.5 * per {
                v -= per;
            }
            while 0.5 * d - v > 0.5 * per {
                v += per;
            }
            v.clamp(0.0, d)
        };
        if m.kx_phys.abs() >= m.ky_phys.abs() {
            let y = 0.5 * d;
            let per = TAU / m.kx_phys.abs();
            let x = near((-(phi + m.ky_phys * y) / m.kx_phys).rem_euclid(d), per);
            (x, y)
        } else {
            let x = 0.5 * d;
            let per = TAU / m.ky_phys.abs();
            let y = near((-(phi + m.kx_phys * x) / m.ky_phys).rem_euclid(d), per);
            (x, y)
        }
    }

    /// Слой барашков ограничен: при переполнении старейший уходит в тепло.
    fn push_breaker(&mut self, v: BreakerVortex) {
        self.spawned_total += 1;
        if self.breakers.len() < self.max_breakers {
            self.breakers.push(v);
            return;
        }
        if let Some(idx) = self
            .breakers
            .iter()
            .enumerate()
            .min_by_key(|(_, b)| b.age_fp)
            .map(|(i, _)| i)
        {
            let old = self.breakers.swap_remove(idx);
            self.heat_fp += (old.energy() * FP).round() as i64;
        }
        self.breakers.push(v);
    }

    /// Эволюция вихрей: лагранжева адвекция полем волн (дрейф Стокса
    /// возникает сам — частица движется с полем), растяжение деформацией
    /// (клип на шаг и потолок 2·Γ₀), вязкое расплывание ядра, возраст.
    fn evolve_breakers(&mut self, dt: f64) {
        let dt_fp = (dt * FP).round() as i64;
        let dom_fp = (self.domain * FP).round() as i64;
        for vi in 0..self.breakers.len() {
            let (x, y) = self.breakers[vi].pos();
            // 1) Адвекция: орбитальное течение волн в центре ядра.
            let (ux, uy, _) = self.flow_at(x, y);
            // 2) Деформация волнового поля в точке.
            let s = self.strain_xx_at(x, y);
            let v = &mut self.breakers[vi];
            v.x_fp = (v.x_fp + (ux * dt * FP).round() as i64).rem_euclid(dom_fp);
            v.y_fp = (v.y_fp + (uy * dt * FP).round() as i64).rem_euclid(dom_fp);
            // Растяжение: dΓ/dt = Γ·∂u/∂x.
            let gamma = v.gamma();
            let mut g2 = gamma * (1.0 + (s * dt).clamp(-0.25, 0.25));
            let cap = 2.0 * (v.gamma0_fp as f64 / FP);
            if g2.abs() > cap {
                g2 = cap * g2.signum();
            }
            v.gamma_fp = (g2 * FP).round() as i64;
            // Вязкость: r² += 8νt (Ламб–Озеен).
            let r = v.radius();
            let r2 = r * r + 8.0 * self.viscosity * dt;
            v.r_fp = (r2.sqrt() * FP).round() as i64;
            // Возраст.
            v.age_fp += dt_fp;
        }
        // Ретировка: время жизни истекло — остаток энергии в тепло.
        let mut heat = 0.0f64;
        let mut i = 0;
        while i < self.breakers.len() {
            if self.breakers[i].age_fp >= self.breakers[i].ttl_fp {
                let old = self.breakers.swap_remove(i);
                heat += old.energy();
            } else {
                i += 1;
            }
        }
        self.heat_fp += (heat * FP).round() as i64;
    }

    /// Деформация волнового поля ∂u_x/∂x в точке (только волны).
    pub fn strain_xx_at(&self, x: f64, y: f64) -> f64 {
        let mut s = 0.0;
        for m in &self.modes {
            let a = self.amplitude(m);
            let theta = m.kx_phys * x + m.ky_phys * y + self.phase(m);
            // u_x = Σ A·ω·(kx/k)·cos θ → ∂u_x/∂x = −Σ A·ω·(kx²/k)·sin θ
            s -= a * m.omega * (m.kx_phys * m.kx_phys / m.k) * theta.sin();
        }
        s
    }

    /// Высота в точке сетки (x, y), м. O(K) — запросы геймплея.
    /// Тот же путь, что surface_at: фундаменталы + Стокс + барашки.
    pub fn height_at(&self, x: usize, y: usize) -> f64 {
        let cell = self.domain / self.n as f64;
        self.surface_at(x as f64 * cell, y as f64 * cell).0
    }

    /// Поле высот n×n — через вихревой синтез (тот же FFT-путь, что у
    /// кодека). Цикл X: + второй FFT-проход вторых гармоник Стокса +
    /// штампы горбов барашков (тороидальные гауссианы).
    pub fn height_field(&self) -> Vec<f64> {
        let mut f = vortex::synthesize(&self.to_vortex());
        if self.nonlinear {
            if let Some(spec2) = self.second_harmonic_spectrum() {
                let f2 = vortex::synthesize(&spec2);
                for (a, b) in f.iter_mut().zip(f2.iter()) {
                    *a += b;
                }
            }
            if !self.breakers.is_empty() {
                self.stamp_breakers(&mut f);
            }
        }
        f
    }

    /// Поверхность в физической точке (x, y — метры от угла домена):
    /// (h, ∂h/∂x, ∂h/∂y) — высота и точные градиенты, м и безразмерные.
    /// Цикл X: + вторая гармоника Стокса (каноничная, окно вторых
    /// гармоник) + горбы барашков. Один проход по модам — для шейдинга.
    pub fn surface_at(&self, x: f64, y: f64) -> (f64, f64, f64) {
        let win = if self.nonlinear { self.second_window() } else { None };
        let mut h = 0.0;
        let mut gx = 0.0;
        let mut gy = 0.0;
        for (i, m) in self.modes.iter().enumerate() {
            let a = self.amplitude(m);
            let theta = m.kx_phys * x + m.ky_phys * y + self.phase(m);
            let (s, c) = theta.sin_cos();
            h += a * c;
            gx -= a * m.kx_phys * s;
            gy -= a * m.ky_phys * s;
            if let Some(w) = win {
                if let Some((a2, phi2)) = self.second_of(i, w) {
                    let th2 = 2.0 * (m.kx_phys * x + m.ky_phys * y) + phi2;
                    let (s2, c2) = th2.sin_cos();
                    h += a2 * c2;
                    gx -= 2.0 * a2 * m.kx_phys * s2;
                    gy -= 2.0 * a2 * m.ky_phys * s2;
                }
            }
        }
        // Горбы барашков (напор скорости ядра) и их градиенты.
        for v in &self.breakers {
            let (dx, dy) = self.torus_delta(x, y, v.pos());
            let b = self.bump_at(dx, dy, v);
            if b == 0.0 {
                continue;
            }
            h += b;
            let r2 = v.radius() * v.radius();
            gx -= b * dx / r2;
            gy -= b * dy / r2;
        }
        (h, gx, gy)
    }

    /// Орбитальное течение в физической точке (x, y — метры):
    /// (u_x, u_y, w) — горизонтальный снос и вертикальная скорость, м/с.
    /// Линейная теория глубоководных волн: u = ∇Φ, z = 0.
    /// Только волны — безвихревая часть (тест curl).
    pub fn flow_at(&self, x: f64, y: f64) -> (f64, f64, f64) {
        let mut ux = 0.0;
        let mut uy = 0.0;
        let mut w = 0.0;
        for m in &self.modes {
            let a = self.amplitude(m);
            let theta = m.kx_phys * x + m.ky_phys * y + self.phase(m);
            let (s, c) = theta.sin_cos();
            let aw = a * m.omega;
            ux += aw * (m.kx_phys / m.k) * c;
            uy += aw * (m.ky_phys / m.k) * c;
            w += aw * s;
        }
        (ux, uy, w)
    }

    /// Течение с барашками: (u_x, u_y, w) — волны (безвихревые) +
    /// когерентные вихри Ламб–Озеена. Первое РОТАЦИОННОЕ поле движка:
    /// циркуляция по контуру вокруг ядра = Γ·(1 − e^(−d²/r²)).
    pub fn flow_full_at(&self, x: f64, y: f64) -> (f64, f64, f64) {
        let (mut ux, mut uy, w) = self.flow_at(x, y);
        for v in &self.breakers {
            let (dx, dy) = self.torus_delta(x, y, v.pos());
            let d2 = dx * dx + dy * dy;
            let r = v.radius();
            if d2 >= 36.0 * r * r {
                continue; // за 6 ядрами вклад ничтожен
            }
            let d = d2.sqrt();
            if d < 1e-9 {
                continue; // центр ядра: скорость → 0 (регулярно)
            }
            let g = v.gamma();
            // u_θ = Γ/(2πd)·(1 − e^(−d²/r²)); тангенс ẑ×r̂ = (−dy, dx)/d.
            let ut = g / (TAU * d) * (1.0 - (-d2 / (r * r)).exp());
            ux += -ut * dy / d;
            uy += ut * dx / d;
        }
        (ux, uy, w)
    }

    /// Тороидальная разность точек (домен периодичен).
    fn torus_delta(&self, x: f64, y: f64, pos: (f64, f64)) -> (f64, f64) {
        let d = self.domain;
        let wrap = |mut v: f64| -> f64 {
            if v > 0.5 * d {
                v -= d;
            } else if v < -0.5 * d {
                v += d;
            }
            v
        };
        (wrap(x - pos.0), wrap(y - pos.1))
    }

    /// Вклад барашка в высоту на расстоянии (dx, dy) от центра:
    /// Δh = Γ²/(4π²·g·r²)·e^(−d²/2r²) с огибающей созревания/затухания.
    /// Обрезка на 3.5σ (0.2% горба) — штамп конечен.
    fn bump_at(&self, dx: f64, dy: f64, v: &BreakerVortex) -> f64 {
        let r = v.radius();
        let d2 = dx * dx + dy * dy;
        if d2 >= 12.25 * r * r {
            return 0.0;
        }
        let g = v.gamma();
        let amp = g * g / (4.0 * PI * PI * self.gravity * r * r);
        let age = v.age();
        let rise = (v.ttl() / 8.0).max(1e-9);
        let env = (age / rise).min(1.0) * ((v.ttl() - age) / rise).clamp(0.0, 1.0);
        amp * env * (-d2 / (2.0 * r * r)).exp()
    }

    /// Живые барашки для геймплея/пены: (x, y, высота горба) в метрах.
    pub fn breakers_info(&self) -> Vec<(f64, f64, f64)> {
        self.breakers
            .iter()
            .map(|v| {
                let r = v.radius();
                let g = v.gamma();
                let amp = g * g / (4.0 * PI * PI * self.gravity * r * r);
                let rise = (v.ttl() / 8.0).max(1e-9);
                let env =
                    (v.age() / rise).min(1.0) * ((v.ttl() - v.age()) / rise).clamp(0.0, 1.0);
                let (x, y) = v.pos();
                (x, y, amp * env)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Генерация спектра: K41 × окно ветра × нормировка Пирсона–Московица
// ---------------------------------------------------------------------------

/// Сгенерировать развитое море: моды лог-равномерно по k с направленным
/// разбросом вокруг ветра, амплитуды — равновесие K41 в окне ветра,
/// нормировка на H_s ≈ 0.21·v²/g. Всё детерминированно от сида.
pub fn generate(p: &WaterParams) -> WaterSpectrum {
    p.validate().expect("water: валидные параметры");
    let mut rng = Rng::new(p.seed);
    let n = p.n;
    let half = (n / 2) as i64;
    let k_min = TAU / p.domain; // бин 1
    let k_peak = (p.gravity / (p.wind * p.wind)).max(k_min);
    let k_nyq = PI * n as f64 / p.domain;
    let k_max = (8.0 * k_peak).min(k_nyq).max(2.0 * k_min);
    let sigma_w = 1.2; // ширина окна ветра (лог-нормальное)

    let mut seen = HashSet::new();
    let mut waves: Vec<(i32, i32, f64)> = Vec::with_capacity(p.modes);
    let mut attempts = 0usize;
    while waves.len() < p.modes && attempts < p.modes * 64 + 256 {
        attempts += 1;
        // Лог-равномерный радиус: каскад от инерционных к мелким масштабам.
        let kk = k_min * (k_max / k_min).powf(rng.next_f64());
        let theta = (p.wind_dir + rng.next_gaussian() * p.spread)
            .clamp(p.wind_dir - PI * 0.45, p.wind_dir + PI * 0.45);
        let kx = (kk * theta.cos() * p.domain / TAU).round() as i64;
        let ky = (kk * theta.sin() * p.domain / TAU).round() as i64;
        // Частоты Найквиста (|бин| = n/2) — самозеркальные: запрещаем,
        // чтобы эрмитова пара ±k всегда была двумя разными бинами.
        if !(1..half).contains(&kx) || ky.abs() >= half {
            continue;
        }
        if !seen.insert((kx, ky)) {
            continue;
        }
        let kbin = ((kx * kx + ky * ky) as f64).sqrt();
        waves.push((kx as i32, ky as i32, TAU * kbin / p.domain));
    }
    assert!(!waves.is_empty(), "water: пустой спектр — увеличь n или domain");

    // Равновесные амплитуды: K41 A ∝ k^(−5/6) в окне ветра,
    // нормировка Σ A²/2 = σ_h², H_s = 4σ_h ≈ 0.21·v²/g (Пирсон–Московиц).
    let hs = 0.21 * p.wind * p.wind / p.gravity;
    let var_target = (hs / 4.0) * (hs / 4.0);
    let raw: Vec<f64> = waves
        .iter()
        .map(|&(_, _, k)| {
            let x = (k / k_peak).ln() / sigma_w;
            (k / k_peak).powf(-5.0 / 6.0) * (-0.5 * x * x).exp()
        })
        .collect();
    let raw_sq: f64 = raw.iter().map(|v| v * v).sum();
    let c = if raw_sq > 0.0 { (2.0 * var_target / raw_sq).sqrt() } else { 0.0 };

    // Place-единицы: бин Фурье несёт A·n²/2 (см. тождество синтеза).
    let np2 = (n * n) as f64 / 2.0;
    let a_place_eq: Vec<f64> = raw.iter().map(|&r| c * r * np2).collect();
    let mut a_lo = f64::INFINITY;
    let mut a_hi = 0.0f64;
    for &a in &a_place_eq {
        a_lo = a_lo.min(a);
        a_hi = a_hi.max(a);
    }
    if !(a_hi > 0.0) {
        a_lo = 1.0;
        a_hi = 1.0;
    }
    let a_min = a_lo / 1.6;
    let a_max = a_hi * 1.6;
    let level_max = 3i64.pow(p.amp_trits) - 1;
    let ln_per_level = (a_max / a_min).ln() / level_max as f64;
    let micro_total = 3i64.pow(p.phase_trits + p.micro_trits);

    let modes: Vec<WaterMode> = waves
        .iter()
        .zip(a_place_eq.iter())
        .map(|(&(kx, ky, k), &a_eq)| {
            let omega = (p.gravity * k + p.capillary * k * k * k).sqrt();
            let u_eq = ((a_eq / a_min).ln() / ln_per_level).clamp(0.0, level_max as f64);
            let decay_rate =
                if ln_per_level > 0.0 { 2.0 * p.viscosity * k * k / ln_per_level } else { 0.0 };
            let phase_fp = (rng.next_f64() * micro_total as f64 * FP).round() as i64;
            WaterMode {
                kx,
                ky,
                acc: (u_eq * FP).round() as i64,
                phase_fp: phase_fp.clamp(0, micro_total * FP_I - 1),
                k,
                kx_phys: TAU * kx as f64 / p.domain,
                ky_phys: TAU * ky as f64 / p.domain,
                omega,
                u_eq,
                decay_rate,
                cooldown_fp: 0,
            }
        })
        .collect();

    let second_skip = WaterSpectrum::build_second_skip(&modes, n);
    WaterSpectrum {
        n,
        amp_trits: p.amp_trits,
        phase_trits: p.phase_trits,
        micro_total,
        a_min,
        a_max,
        level_max,
        ln_per_level,
        tau_wind: p.tau_wind,
        domain: p.domain,
        modes,
        steps: 0,
        variance_target: var_target,
        breakers: Vec::new(),
        rng, // продвинут генерацией — события продолжают детерминизм
        heat_fp: 0,
        whitecap_acc_fp: 0,
        spawned_total: 0,
        gravity: p.gravity,
        viscosity: p.viscosity,
        wind: p.wind,
        nonlinear: p.nonlinear,
        steepness_break: p.steepness_break,
        whitecap_onset: p.whitecap_onset,
        whitecap_rate: p.whitecap_rate,
        breaker_alpha: p.breaker_alpha,
        breaker_heat_frac: p.breaker_heat_frac,
        max_breakers: p.max_breakers,
        second_skip,
    }
}

// ---------------------------------------------------------------------------
// Контейнер VRTX: вода ↔ кодек (спектр есть спектр)
// ---------------------------------------------------------------------------

impl WaterSpectrum {
    /// Спектр воды как спектр кодека: каждая волна раскладывается в
    /// эрмитову пару ±k (вещественное поле). Фаза контейнера — микро-сектор
    /// 3^(p+m): тот же `synthesize`, что у кодека, даёт поле высот.
    pub fn to_vortex(&self) -> vortex::VortexSpectrum {
        let n = self.n as u32;
        let mut harmonics = Vec::with_capacity(self.modes.len() * 2);
        for m in &self.modes {
            let a = self.amp_place(m);
            let q = vortex::amp_quant(a, self.a_min, self.a_max, self.amp_trits);
            let s = (m.phase_fp >> 32).rem_euclid(self.micro_total) as u64;
            let s_neg = (self.micro_total - s as i64).rem_euclid(self.micro_total) as u64;
            let kx = m.kx as u32;
            let ky = m.ky.rem_euclid(n as i32) as u32;
            let kxn = (n - kx) % n;
            let kyn = (n - ky) % n;
            harmonics.push(vortex::Harmonic { kx, ky, q, s });
            harmonics.push(vortex::Harmonic { kx: kxn, ky: kyn, q, s: s_neg });
        }
        vortex::VortexSpectrum {
            n: self.n as u32,
            amp_trits: self.amp_trits,
            phase_trits: self.phase_trits + 3, // p + m микро-секторов
            a_min: self.a_min,
            a_max: self.a_max,
            harmonics,
            energy_total: 0.0,
            energy_kept: 0.0,
        }
    }

    /// Хеш детерминизма: FNV-1a по байтам VRTX-контейнера + сайдкар
    /// слоя барашков (состояние движка) — штормы воспроизводимы бит-в-бит.
    pub fn water_hash(&self) -> u64 {
        let mut bytes = vortex::encode(&self.to_vortex());
        bytes.extend_from_slice(&(self.breakers.len() as u64).to_le_bytes());
        for v in &self.breakers {
            bytes.extend_from_slice(&v.x_fp.to_le_bytes());
            bytes.extend_from_slice(&v.y_fp.to_le_bytes());
            bytes.extend_from_slice(&v.gamma_fp.to_le_bytes());
            bytes.extend_from_slice(&v.r_fp.to_le_bytes());
            bytes.extend_from_slice(&v.age_fp.to_le_bytes());
            bytes.extend_from_slice(&v.ttl_fp.to_le_bytes());
        }
        bytes.extend_from_slice(&self.heat_fp.to_le_bytes());
        bytes.extend_from_slice(&self.whitecap_acc_fp.to_le_bytes());
        bytes.extend_from_slice(&self.rng.state().to_le_bytes());
        vortex::fnv1a_u8(&bytes)
    }

    // -------------------------------------------------------------------
    // Цикл X: вторые гармоники Стокса (собственная лог-шкала)
    // -------------------------------------------------------------------

    /// Статические пропуски вторых гармоник: DC/Найквист-самозеркала и
    /// коллизии бинов ±2k (ky-пары через n/2) — поправка субпроцентная,
    /// честнее пропустить, чем квантовать сумму нетождественно.
    fn build_second_skip(modes: &[WaterMode], n: usize) -> Vec<bool> {
        let ni = n as i64;
        let mut seen = HashSet::new();
        let mut skip = Vec::with_capacity(modes.len());
        for m in modes {
            let k2x = (2 * m.kx as i64).rem_euclid(ni);
            let k2y = (2 * m.ky as i64).rem_euclid(ni);
            let bad = k2x == 0 || k2x == ni / 2 || k2y == 0 || k2y == ni / 2;
            skip.push(bad || !seen.insert((k2x, k2y)));
        }
        skip
    }

    /// Бин второй гармоники моды (или пропуск).
    fn second_bin(&self, i: usize) -> Option<(u32, u32)> {
        if !*self.second_skip.get(i)? {
            let m = &self.modes[i];
            let n = self.n as i64;
            Some((
                (2 * m.kx as i64).rem_euclid(n) as u32,
                (2 * m.ky as i64).rem_euclid(n) as u32,
            ))
        } else {
            None
        }
    }

    /// Лог-окно вторых гармоник [lo, hi] (Place-единицы) по живым модам.
    pub fn second_window(&self) -> Option<(f64, f64)> {
        let n2 = (self.n * self.n) as f64;
        let mut lo = f64::INFINITY;
        let mut hi = 0.0f64;
        for (i, m) in self.modes.iter().enumerate() {
            if self.second_bin(i).is_none() {
                continue;
            }
            let a_place = self.amp_place(m);
            let a2 = m.k * a_place * a_place / n2;
            if a2 <= 0.0 {
                continue;
            }
            lo = lo.min(a2);
            hi = hi.max(a2);
        }
        if !(hi > 0.0) {
            return None;
        }
        if !lo.is_finite() {
            lo = hi;
        }
        Some((lo / 1.6, hi * 1.6))
    }

    /// Каноничная вторая гармоника моды: (A₂, 2φ) — физические единицы,
    /// та же квантовка, что у FFT-пути (окно вторых гармоник).
    pub fn second_of(&self, i: usize, win: (f64, f64)) -> Option<(f64, f64)> {
        self.second_bin(i)?;
        let m = &self.modes[i];
        let n2 = (self.n * self.n) as f64;
        let a_place = self.amp_place(m);
        let a2_place = m.k * a_place * a_place / n2;
        let q = vortex::amp_quant(a2_place, win.0, win.1, self.amp_trits);
        let a2p = vortex::amp_dequant(q, win.0, win.1, self.amp_trits);
        let p3 = self.phase_trits + 3;
        let s = vortex::phase_quant(2.0 * self.phase(m), p3);
        let phi2 = vortex::phase_dequant(s, p3);
        Some((2.0 * a2p / n2, phi2))
    }

    /// Спектр вторых гармоник для FFT-пути height_field: эрмитовы пары
    /// ±2k в собственной лог-шкале (квант-тождество: second_of ↔ бин;
    /// n² — степень четвёрки, переход физ ↔ Place точен в fp).
    fn second_harmonic_spectrum(&self) -> Option<vortex::VortexSpectrum> {
        let win = self.second_window()?;
        let n = self.n as u32;
        let p3 = self.phase_trits + 3;
        let micro = 3u64.pow(p3);
        let mut harmonics = Vec::new();
        for (i, _m) in self.modes.iter().enumerate() {
            let (k2x, k2y) = match self.second_bin(i) {
                Some(b) => b,
                None => continue,
            };
            let (a2, phi2) = match self.second_of(i, win) {
                Some(v) => v,
                None => continue,
            };
            // Обратно в Place-единицы: a2·n²/2 == a2p (точно).
            let a2_place = a2 * (self.n * self.n) as f64 / 2.0;
            let q = vortex::amp_quant(a2_place, win.0, win.1, self.amp_trits);
            let s = vortex::phase_quant(phi2, p3);
            let s_neg = (micro as i64 - s as i64).rem_euclid(micro as i64) as u64;
            harmonics.push(vortex::Harmonic { kx: k2x, ky: k2y, q, s });
            harmonics.push(vortex::Harmonic {
                kx: (n - k2x) % n,
                ky: (n - k2y) % n,
                q,
                s: s_neg,
            });
        }
        if harmonics.is_empty() {
            return None;
        }
        Some(vortex::VortexSpectrum {
            n,
            amp_trits: self.amp_trits,
            phase_trits: p3,
            a_min: win.0,
            a_max: win.1,
            harmonics,
            energy_total: 0.0,
            energy_kept: 0.0,
        })
    }

    /// Штампы горбов барашков на сетке (тороидальные, через bump_at —
    /// тот же след, что в surface_at; бокс 3.5σ вокруг центра).
    fn stamp_breakers(&self, f: &mut [f64]) {
        let n = self.n as isize;
        let cell = self.domain / self.n as f64;
        for v in &self.breakers {
            let (vx, vy) = v.pos();
            let r = v.radius();
            let rad = (3.5 * r).max(1.5 * cell);
            let i0 = ((vx - rad) / cell).floor() as isize;
            let i1 = ((vx + rad) / cell).ceil() as isize;
            let j0 = ((vy - rad) / cell).floor() as isize;
            let j1 = ((vy + rad) / cell).ceil() as isize;
            for j in j0..=j1 {
                let y = j as f64 * cell;
                for i in i0..=i1 {
                    let x = i as f64 * cell;
                    let (dx, dy) = self.torus_delta(x, y, (vx, vy));
                    let b = self.bump_at(dx, dy, v);
                    if b == 0.0 {
                        continue;
                    }
                    let idx = (j.rem_euclid(n) * n + i.rem_euclid(n)) as usize;
                    f[idx] += b;
                }
            }
        }
    }

    /// Восстановление из контейнера. Физика мод пересчитывается из
    /// параметров (домен, дисперсия, вязкость); равновесие принимается
    /// равным текущему уровню — контейнер хранит видимый кадр спектра.
    pub fn from_vortex(
        spec: &vortex::VortexSpectrum,
        p: &WaterParams,
    ) -> Result<WaterSpectrum, String> {
        let n = spec.n as usize;
        if !n.is_power_of_two() || n < 16 {
            return Err("water: n контейнера — степень двойки ≥ 16".into());
        }
        if spec.phase_trits <= 3 {
            return Err("water: контейнер без микро-секторов фазы (нужны p+3 трита)".into());
        }
        let micro_total = 3i64.pow(spec.phase_trits);
        let half = (n / 2) as i64;
        let level_max = 3i64.pow(spec.amp_trits) - 1;
        let ln_per_level = (spec.a_max / spec.a_min).ln() / level_max as f64;
        if !(ln_per_level > 0.0) || !(spec.a_min > 0.0) {
            return Err("water: битое лог-окно амплитуд".into());
        }

        let mut bins: HashMap<(u32, u32), (u64, u64)> = HashMap::new();
        for h in &spec.harmonics {
            if bins.insert((h.kx, h.ky), (h.q, h.s)).is_some() {
                return Err("water: дубликат бина в контейнере".into());
            }
        }
        let mut modes = Vec::with_capacity(spec.harmonics.len() / 2);
        let mut used = HashSet::new();
        for h in &spec.harmonics {
            let wrap = |v: u32| -> i64 {
                let s = v as i64;
                if s > n as i64 / 2 {
                    s - n as i64
                } else {
                    s
                }
            };
            let kx_s = wrap(h.kx);
            let ky_s = wrap(h.ky);
            if !(1..half).contains(&kx_s) || ky_s.abs() >= half {
                // Неканонический представитель (−k) или частота Найквиста:
                // волну обработает пара с kx ∈ [1, n/2); сироты и самозеркала
                // поймает финальная проверка счёта бинов.
                continue;
            }
            if used.contains(&(h.kx, h.ky)) {
                continue;
            }
            let pkx = ((n as u32) - h.kx) % n as u32;
            let pky = ((n as u32) - h.ky) % n as u32;
            let partner = bins
                .get(&(pkx, pky))
                .ok_or_else(|| "water: нет эрмитовой пары ±k".to_string())?;
            if partner.0 != h.q {
                return Err("water: амплитуда эрмитовой пары не совпала".into());
            }
            let s_neg = (micro_total - h.s as i64).rem_euclid(micro_total);
            if partner.1 as i64 != s_neg {
                return Err("water: фаза эрмитовой пары не совпала".into());
            }
            used.insert((h.kx, h.ky));
            used.insert((pkx, pky));

            let kx = kx_s as i32;
            let ky = ky_s as i32;
            let k = TAU * ((kx_s * kx_s + ky_s * ky_s) as f64).sqrt() / p.domain;
            let omega = (p.gravity * k + p.capillary * k * k * k).sqrt();
            let u = (h.q as i64).clamp(0, level_max) as f64;
            let decay_rate =
                if ln_per_level > 0.0 { 2.0 * p.viscosity * k * k / ln_per_level } else { 0.0 };
            modes.push(WaterMode {
                kx,
                ky,
                acc: (h.q as i64).clamp(0, level_max) << 32,
                phase_fp: (h.s as i64 % micro_total) << 32,
                k,
                kx_phys: TAU * kx as f64 / p.domain,
                ky_phys: TAU * ky as f64 / p.domain,
                omega,
                u_eq: u, // равновесие = текущий уровень (кадр)
                decay_rate,
                cooldown_fp: 0,
            });
        }
        if used.len() != spec.harmonics.len() {
            return Err("water: непарные бины в контейнере (сироты ±k)".into());
        }
        let var_target = {
            let hs = 0.21 * p.wind * p.wind / p.gravity;
            (hs / 4.0) * (hs / 4.0)
        };
        // Слой барашков не живёт в контейнере (кадр линейного спектра):
        // восстановление начинается со спокойного слоя; RNG сеется
        // детерминированно от байтов контейнера.
        let rng = Rng::new(vortex::fnv1a_u8(&vortex::encode(spec)));
        let second_skip = WaterSpectrum::build_second_skip(&modes, n);
        Ok(WaterSpectrum {
            n,
            amp_trits: spec.amp_trits,
            phase_trits: spec.phase_trits - 3,
            micro_total,
            a_min: spec.a_min,
            a_max: spec.a_max,
            level_max,
            ln_per_level,
            tau_wind: p.tau_wind,
            domain: p.domain,
            modes,
            steps: 0,
            variance_target: var_target,
            breakers: Vec::new(),
            rng,
            heat_fp: 0,
            whitecap_acc_fp: 0,
            spawned_total: 0,
            gravity: p.gravity,
            viscosity: p.viscosity,
            wind: p.wind,
            nonlinear: p.nonlinear,
            steepness_break: p.steepness_break,
            whitecap_onset: p.whitecap_onset,
            whitecap_rate: p.whitecap_rate,
            breaker_alpha: p.breaker_alpha,
            breaker_heat_frac: p.breaker_heat_frac,
            max_breakers: p.max_breakers,
            second_skip,
        })
    }
}

// ---------------------------------------------------------------------------
// f64-эталон: те же уравнения, ноль квантования
// ---------------------------------------------------------------------------

/// Мода эталона: непрерывные амплитуда и фаза (f64).
#[derive(Debug, Clone)]
struct WaterModeF64 {
    kx: i32,
    ky: i32,
    /// Волновое число |k|, рад/м.
    k: f64,
    /// Логарифм амплитуды в Place-единицах (непрерывный).
    ln_a_place: f64,
    /// Фаза, рад (f64, с завёрткой).
    phi: f64,
    omega: f64,
    /// Равновесный логарифм (окно ветра × K41).
    ln_eq: f64,
    decay_ln: f64,
}

/// Эталонная вода: та же физика без трит-квантования. Меряется PSNR
/// трит-воды против неё — честная цена квантования на кремнии.
/// Цикл X: зеркалит нелинейность — сдвиг Стокса, спиллинг Мичелла
/// (непрерывная релаксация) и съём энергии барашков (средний размер,
/// непрерывный); горбов барашков в эталоне нет — это визуальный слой
/// движка (позиции событий хаотичны по существу, PSNR меряет волны).
#[derive(Debug, Clone)]
pub struct WaterF64 {
    n: usize,
    tau_wind: f64,
    modes: Vec<WaterModeF64>,
    steps: u64,
    nonlinear: bool,
    steepness_break: f64,
    whitecap_onset: f64,
    whitecap_rate: f64,
    breaker_alpha: f64,
    breaker_heat_frac: f64,
    gravity: f64,
    wind: f64,
}

impl WaterF64 {
    /// Снять непрерывную копию с трит-спектра: начальные амплитуда и фаза
    /// совпадают с внутренним непрерывным состоянием трит-воды, так что
    /// расхождение дальше — только цена лестницы и секторов.
    pub fn from(s: &WaterSpectrum) -> Self {
        let modes = s
            .modes
            .iter()
            .map(|m| {
                let u_cont = (m.acc as f64 / FP).clamp(0.0, s.level_max as f64);
                WaterModeF64 {
                    kx: m.kx,
                    ky: m.ky,
                    k: m.k,
                    ln_a_place: s.a_min.ln() + u_cont * s.ln_per_level,
                    phi: s.phase(m),
                    omega: m.omega,
                    ln_eq: s.a_min.ln() + m.u_eq * s.ln_per_level,
                    decay_ln: m.decay_rate * s.ln_per_level,
                }
            })
            .collect();
        WaterF64 {
            n: s.n,
            tau_wind: s.tau_wind,
            modes,
            steps: s.steps,
            nonlinear: s.nonlinear,
            steepness_break: s.steepness_break,
            whitecap_onset: s.whitecap_onset,
            whitecap_rate: s.whitecap_rate,
            breaker_alpha: s.breaker_alpha,
            breaker_heat_frac: s.breaker_heat_frac,
            gravity: s.gravity,
            wind: s.wind,
        }
    }

    /// Шаг эталона: f32 НЕ используется даже здесь — честный f64.
    pub fn step(&mut self, dt: f64) {
        for m in &mut self.modes {
            let a = 2.0 * m.ln_a_place.exp() / (self.n * self.n) as f64;
            let ka = m.k * a;
            // Сдвиг частоты Стокса — непрерывный.
            let shift = if self.nonlinear { 1.0 + 0.5 * ka * ka } else { 1.0 };
            m.phi = (m.phi - m.omega * shift * dt).rem_euclid(TAU);
            let mut dln = (m.ln_eq - m.ln_a_place) / self.tau_wind - m.decay_ln;
            // Спиллинг Мичелла: непрерывная релаксация к 0.9·порога.
            if self.nonlinear && ka > self.steepness_break {
                let ln_t =
                    (0.9 * self.steepness_break / m.k * (self.n * self.n) as f64 / 2.0).ln();
                let tau_b = 0.25 * TAU / m.omega;
                dln += (ln_t - m.ln_a_place) / tau_b;
            }
            m.ln_a_place += dln * dt;
        }
        // Белые барашки: непрерывное зеркало трит-движка (средний размер).
        if self.nonlinear && self.wind > self.whitecap_onset {
            let over = ((self.wind - self.whitecap_onset) / 6.0).clamp(0.0, 3.0);
            let rate = self.whitecap_rate * over * over;
            let var: f64 = self.variance();
            let hs = 4.0 * var.sqrt();
            if hs > 0.0 {
                let h_b = (hs * (0.08 + 0.30 * over)).clamp(hs * 0.03, hs * 0.45);
                let gamma = self.breaker_alpha * (self.gravity * h_b * h_b * h_b).sqrt();
                let e_rate =
                    rate * gamma * gamma / (8.0 * PI) / (1.0 - self.breaker_heat_frac);
                // Съём энергии пропорционально наклону² мод.
                let mut w = 0.0;
                let slopes: Vec<f64> = self
                    .modes
                    .iter()
                    .map(|m| {
                        let a = 2.0 * m.ln_a_place.exp() / (self.n * self.n) as f64;
                        let s = m.k * a;
                        w += s * s;
                        s
                    })
                    .collect();
                if w > 0.0 {
                    for (m, &s) in self.modes.iter_mut().zip(slopes.iter()) {
                        let a = 2.0 * m.ln_a_place.exp() / (self.n * self.n) as f64;
                        let e_i = 0.5 * self.gravity * a * a;
                        if e_i <= 0.0 {
                            continue;
                        }
                        let de = e_rate * dt * s * s / w;
                        m.ln_a_place -= de / (2.0 * e_i);
                    }
                }
            }
        }
        self.steps += 1;
    }

    fn amp_full(&self, m: &WaterModeF64) -> f64 {
        2.0 * m.ln_a_place.exp() / (self.n * self.n) as f64
    }

    /// Поле высот эталона (прямая сумма — без FFT, независимый путь).
    /// Цикл X: + вторая гармоника Стокса (непрерывная, без квантования).
    pub fn height_field(&self) -> Vec<f64> {
        let n = self.n;
        let nf = n as f64;
        let mut f = vec![0.0f64; n * n];
        for m in &self.modes {
            let a = self.amp_full(m);
            let a2 = if self.nonlinear { 0.5 * m.k * a * a } else { 0.0 };
            for y in 0..n {
                let base = TAU * m.ky as f64 * y as f64 / nf + m.phi;
                let row = y * n;
                for x in 0..n {
                    let arg = TAU * m.kx as f64 * x as f64 / nf + base;
                    f[row + x] += a * arg.cos() + a2 * (2.0 * arg).cos();
                }
            }
        }
        f
    }

    /// Дисперсия высот эталона, м².
    pub fn variance(&self) -> f64 {
        self.modes
            .iter()
            .map(|m| {
                let a = self.amp_full(m);
                a * a / 2.0
            })
            .sum()
    }
}

// ---------------------------------------------------------------------------
// Метрики честности
// ---------------------------------------------------------------------------

/// Поле высот (метры) → u8: окно ±2.5σ целевой дисперсии.
/// Одна шкала для трит-воды и эталона — PSNR видит только разницу физики.
pub fn field_to_u8(field: &[f64], sigma: f64) -> Vec<u8> {
    let scale = if sigma > 1e-300 { 2.5 * sigma } else { 1.0 };
    field
        .iter()
        .map(|&h| ((h / scale + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8)
        .collect()
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn small(seed: u64, modes: usize) -> WaterParams {
        WaterParams { n: 64, modes, seed, ..Default::default() }
    }

    #[test]
    fn generation_modes_unique_directed() {
        let p = WaterParams { n: 256, modes: 192, seed: 42, ..Default::default() };
        let w = generate(&p);
        assert_eq!(w.modes.len(), p.modes, "все моды разместились");
        let mut uniq = HashSet::new();
        for m in &w.modes {
            assert!((1..128).contains(&m.kx), "kx={} вне канона", m.kx);
            assert!(m.ky.abs() < 128, "ky={} вне канона", m.ky);
            assert!(uniq.insert((m.kx, m.ky)), "дубликат бина");
            assert!(m.omega > 0.0 && m.k > 0.0 && m.u_eq >= 0.0 && m.decay_rate >= 0.0);
        }
        // Спектр при генерации уже нормирован на цель (квантование лестницей).
        let var = w.variance() / w.variance_target;
        assert!((0.8..=1.25).contains(&var), "дисперсия/цель = {var}");
        // Значимая высота согласуется с оценкой Пирсона–Московица.
        let hs_pm = 0.21 * p.wind * p.wind / p.gravity;
        let hs = w.significant_height();
        assert!((0.6..=1.4).contains(&(hs / hs_pm)), "H_s={hs} против PM {hs_pm}");
    }

    #[test]
    fn phase_counter_dispersion_exact() {
        // Одномодовое море, ЛИНЕЙНАЯ ветка (nonlinear = false): счётчик
        // фазы обязан отщелкать ровно ω·dt за шаг — с точностью округления
        // константы (без накопления). Нелинейный сдвиг Стокса — отдельный
        // тест ниже (stokes_counter_shift_integer_exact).
        let p = WaterParams {
            n: 64, modes: 1, seed: 3, wind: 6.0,
            nonlinear: false, ..Default::default()
        };
        let mut w = generate(&p);
        let m0 = w.modes[0].clone();
        let dt = 1.0 / 60.0;
        let steps = 1000i64;
        let fp0 = m0.phase_fp;
        for _ in 0..steps as usize {
            w.step(dt);
        }
        let adv = (m0.omega * dt * w.micro_total as f64 / TAU * FP).round() as i64;
        // Целочисленная аддитивность: ровно steps·adv, без дрейфа.
        let wrap = w.micro_total * FP_I;
        let expect = (fp0 - steps * adv).rem_euclid(wrap);
        assert_eq!(w.modes[0].phase_fp, expect, "счётчик дрейфанул");
        // Физика: видимая частота совпадает с дисперсионной
        // (adv живёт в фикс-точке микро-секторов — делим на 2³²).
        let omega_apparent = adv as f64 / FP * TAU / (w.micro_total as f64 * dt);
        let rel = ((omega_apparent - m0.omega) / m0.omega).abs();
        assert!(rel < 1e-8, "относительная ошибка частоты {rel}");
    }

    #[test]
    fn crest_moves_at_phase_speed() {
        // Гребень бегит по ветру. Вдоль y=0 косая волна движется со
        // скоростью ω/kx (фазовая скорость по оси x) — её и меряем.
        // У чистой косинусоиды гребни равновысоки: argmax ищем окном
        // ± полпериода от прошлого положения, без глобального шума.
        let p =
            WaterParams { n: 256, modes: 1, seed: 11, wind: 5.0, domain: 100.0, ..Default::default() };
        let mut w = generate(&p);
        let m = w.modes[0].clone();
        let c = m.omega / m.kx_phys; // скорость гребня вдоль оси x, м/с
        let dt = 0.1;
        let cell = p.domain / p.n as f64;
        let period_cells = p.n as f64 / m.kx as f64;
        let win = (period_cells / 2.0).ceil() as i64;

        let crest_global = |w: &WaterSpectrum| -> f64 {
            let n = w.n as i64;
            let mut best = 0i64;
            let mut bh = f64::NEG_INFINITY;
            for x in 0..n {
                let h = w.height_at(x as usize, 0);
                if h > bh {
                    bh = h;
                    best = x;
                }
            }
            best as f64
        };
        let crest_near = |w: &WaterSpectrum, prev: f64| -> f64 {
            let n = w.n as i64;
            let center = prev.round() as i64;
            let mut best = center;
            let mut bh = f64::NEG_INFINITY;
            for di in -win..=win {
                let x = (center + di).rem_euclid(n) as usize;
                let h = w.height_at(x, 0);
                if h > bh {
                    bh = h;
                    best = (center + di).rem_euclid(n);
                }
            }
            // Параболическая интерполяция — субпиксельная точность.
            let at = |i: i64| w.height_at(i.rem_euclid(n) as usize, 0);
            let bh2 = at(best);
            let hm = at(best - 1);
            let hp = at(best + 1);
            let denom = hm - 2.0 * bh2 + hp;
            let shift = if denom.abs() > 1e-300 { 0.5 * (hm - hp) / denom } else { 0.0 };
            best as f64 + shift
        };

        let mut pos = crest_global(&w);
        let mut dist = 0.0f64;
        let n_steps = 30;
        for _ in 0..n_steps {
            w.step(dt);
            let np = crest_near(&w, pos);
            let mut d = (np - pos) * cell;
            if d < -p.domain / 2.0 {
                d += p.domain;
            }
            if d > p.domain / 2.0 {
                d -= p.domain;
            }
            dist += d;
            pos = np;
        }
        let c_est = dist / (n_steps as f64 * dt);
        assert!(c_est > 0.0, "волна побежала против ветра: {c_est}");
        assert!((c_est - c).abs() / c < 0.15, "c_est={c_est} против c={c}");
    }

    #[test]
    fn viscous_decay_matches_lamb() {
        // Затухание Ламба A ∝ e^(−2νk²t): в лог-уровнях — линейный спуск.
        let p = WaterParams {
            n: 64,
            modes: 1,
            seed: 5,
            wind: 4.0,
            viscosity: 0.05,
            tau_wind: 3600.0, // ветер почти выключен
            ..Default::default()
        };
        let mut w = generate(&p);
        let m = w.modes[0].clone();
        assert!(m.decay_rate > 0.0, "для этого k вязкость должна действовать");
        let target_drop = 4.0f64; // уровней лестницы
        let t_total = target_drop / m.decay_rate;
        let n_steps = 200;
        let dt = t_total / n_steps as f64;
        let u0 = w.modes[0].acc >> 32;
        for _ in 0..n_steps {
            w.step(dt);
        }
        let drop = (u0 - (w.modes[0].acc >> 32)) as f64;
        // Лестница + округления: допуск полтора уровня.
        assert!((drop - target_drop).abs() <= 1.5, "спуск {drop} против {target_drop}");
    }

    #[test]
    fn wind_builds_equilibrium_band() {
        // Из штиля ветер накачивает море; после 6τ — стационарная полоса.
        let p =
            WaterParams { n: 64, modes: 96, seed: 7, wind: 8.0, tau_wind: 5.0, ..Default::default() };
        let mut w = generate(&p);
        for m in &mut w.modes {
            m.acc = (m.acc / 20).max(1); // штиль: ~5% уровней
        }
        assert!(w.variance() / w.variance_target < 0.1, "это ещё штиль");
        let dt = 1.0 / 60.0;
        for _ in 0..(6.0 * p.tau_wind / dt) as usize {
            w.step(dt);
        }
        let var = w.variance() / w.variance_target;
        assert!((0.4..=1.6).contains(&var), "море не развилось: {var}");
        // Ещё 6τ — полоса держится (нет ни разгона, ни затухания).
        for _ in 0..(6.0 * p.tau_wind / dt) as usize {
            w.step(dt);
        }
        let var2 = w.variance() / w.variance_target;
        assert!((0.4..=1.6).contains(&var2), "полоса не держится: {var2}");
    }

    #[test]
    fn f32_drifts_trit_counter_does_not() {
        // Главная демонрация кремния: одна и та же динамика фазы —
        // f32-аккумулятор (как в сеточных движках) против целочисленного
        // счётчика фикс-точки. 3 млн шагов.
        let omega = 2.0f64;
        let dt = 1.0f64 / 60.0;
        let micro = 19683i64; // 3^9
        let steps = 3_000_000i64;
        let sector = TAU / micro as f64;

        // f32: погрешность округления растёт вместе с величиной фазы.
        let step32 = (omega * dt) as f32;
        let mut ph32 = 0.0f32;
        for _ in 0..steps {
            ph32 -= step32;
        }
        let exact = omega * dt * steps as f64;
        let err32 = (ph32 as f64 + exact).abs();

        // Трит-счётчик: целочисленные вычитания точны по построению;
        // вся «цена» — округление константы шага (≤ 2⁻³³ сектора).
        let adv = (omega * dt * micro as f64 / TAU * FP).round() as i64;
        let wrap = micro * FP_I;
        let mut fp = 0i64;
        for _ in 0..steps {
            fp = (fp - adv).rem_euclid(wrap);
        }
        let expect = (-(steps) * adv).rem_euclid(wrap);
        assert_eq!(fp, expect, "целочисленная аддитивность нарушена?!");
        // Ошибка квантования константы, набежавшая за 3 млн шагов:
        let trit_err_micro = (steps as f64 * (adv as f64 / FP
            - omega * dt * micro as f64 / TAU)).abs();
        let trit_err_rad = trit_err_micro * TAU / micro as f64;

        assert!(trit_err_rad < 1e-4, "дрейф трита {trit_err_rad} рад");
        assert!(err32 > 100.0 * sector, "f32 дрейф {err32} рад мал сверх ожидания");
        assert!(err32 > 1e4 * trit_err_rad, "разрыв не убедителен: {err32} против {trit_err_rad}");
    }

    #[test]
    fn determinism_bit_to_bit() {
        let run = |seed: u64| {
            let p = WaterParams { n: 64, modes: 48, seed, ..Default::default() };
            let mut w = generate(&p);
            for _ in 0..100 {
                w.step(1.0 / 60.0);
            }
            let f = w.height_field();
            (w.water_hash(), vortex::fnv1a_u8(&field_to_u8(&f, w.variance_target.sqrt())))
        };
        let a = run(21);
        let b = run(21);
        let c = run(22);
        assert_eq!(a, b, "бит-в-бит детерминизм");
        assert_ne!(a, c, "разные сиды — разные моря");
    }

    #[test]
    fn vrtx_roundtrip_exact() {
        let p = small(9, 24);
        let mut w = generate(&p);
        for _ in 0..50 {
            w.step(1.0 / 60.0);
        }
        let bytes = vortex::encode(&w.to_vortex());
        let spec = vortex::decode(&bytes).expect("контейнер воды декодируется");
        let w2 = WaterSpectrum::from_vortex(&spec, &p).expect("пары ±k собираются");
        assert_eq!(w2.modes.len(), w.modes.len());
        let f1 = w.height_field();
        let f2 = w2.height_field();
        for (a, b) in f1.iter().zip(f2.iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "восстановление не бит-в-бит");
        }
    }

    #[test]
    fn height_at_matches_height_field() {
        let w = generate(&small(13, 24));
        let f = w.height_field();
        let n = 64;
        for y in (0..n).step_by(7) {
            for x in (0..n).step_by(5) {
                let d = (w.height_at(x, y) - f[y * n + x]).abs();
                assert!(d < 1e-9, "({x},{y}): расхождение {d}");
            }
        }
    }

    #[test]
    fn flow_field_is_irrotational() {
        // Линейная глубоководная вода — потенциальное течение:
        // ротор горизонтального поля обязан быть нулём.
        let w = generate(&small(17, 24));
        let h = 0.05; // шаг центральной разности, м
        let mut max_curl = 0.0f64;
        let mut max_u = 0.0f64;
        for i in 0..10 {
            for j in 0..10 {
                let x = 5.0 + i as f64 * 9.0;
                let y = 5.0 + j as f64 * 9.0;
                let (_, uy1, _) = w.flow_at(x - h, y);
                let (_, uy2, _) = w.flow_at(x + h, y);
                let (ux1, _, _) = w.flow_at(x, y - h);
                let (ux2, _, _) = w.flow_at(x, y + h);
                let curl = (uy2 - uy1) / (2.0 * h) - (ux2 - ux1) / (2.0 * h);
                max_curl = max_curl.max(curl.abs());
                let (ux, uy, w_) = w.flow_at(x, y);
                max_u = max_u.max(ux.abs().max(uy.abs()).max(w_.abs()));
            }
        }
        assert!(max_u > 0.0, "течение нулевое — тест слепой");
        assert!(max_curl < 0.05 * max_u, "curl {max_curl} при скорости {max_u}");
    }

    #[test]
    fn psnr_against_f64_reference() {
        // Честная цена квантования: трит-вода против f64-эталона
        // с теми же уравнениями. 4 секунды модельного времени.
        let p = WaterParams { n: 64, modes: 48, seed: 42, ..Default::default() };
        let mut w = generate(&p);
        let mut r = WaterF64::from(&w);
        let dt = 1.0 / 60.0;
        for _ in 0..240 {
            w.step(dt);
            r.step(dt);
        }
        let sigma = w.variance_target.sqrt();
        let fa = field_to_u8(&w.height_field(), sigma);
        let fb = field_to_u8(&r.height_field(), sigma);
        let psnr = vortex::psnr_u8(&fa, &fb);
        assert!(psnr > 35.0, "PSNR трит-воды против эталона: {psnr}");
        // Дисперсии согласуются — энергия не потерялась и не приплыла.
        let rel = ((w.variance() - r.variance()) / r.variance()).abs();
        assert!(rel < 0.05, "дисперсии разошлись: {rel}");
    }

    #[test]
    fn state_memory_small_vs_grid() {
        // Состояние в байтах против f32-сетки той же детализации.
        let w = generate(&WaterParams { n: 256, modes: 192, ..Default::default() });
        let bytes = vortex::encode(&w.to_vortex()).len();
        let grid = 256 * 256 * 4;
        assert!(bytes * 100 < grid, "состояние {bytes} Б против сетки {grid} Б");
    }

    // -------------------------------------------------------------------
    // Цикл X «Обрушение»: нелинейность и вихри
    // -------------------------------------------------------------------

    #[test]
    fn stokes_counter_shift_integer_exact() {
        // Цикл X: счётчик фазы несёт стоксов сдвиг ω·(1 + (kA)²/2) —
        // по-прежнему целочисленно: каждый шаг ровно round(константы),
        // без накопления. Проверяем на живой (эволюционирующей) амплитуде.
        let p = WaterParams {
            n: 64, modes: 1, seed: 3, wind: 10.0,
            tau_wind: 3600.0, viscosity: 1e-8,
            nonlinear: true, ..Default::default()
        };
        let mut w = generate(&p);
        let m0 = w.modes[0].clone();
        let dt = 1.0 / 60.0;
        let wrap = w.micro_total * FP_I;
        let mut shift_seen = 0.0f64;
        for _ in 0..600 {
            let a = w.amplitude(&w.modes[0]);
            let ka = m0.k * a;
            let shift = 1.0 + 0.5 * ka * ka;
            shift_seen = shift_seen.max(shift - 1.0);
            // Тот же порядок операций, что в step() — биты совпадают.
            let adv = (m0.omega * shift * dt * w.micro_total as f64 / TAU * FP).round() as i64;
            let before = w.modes[0].phase_fp;
            w.step(dt);
            let moved = (before - w.modes[0].phase_fp).rem_euclid(wrap);
            assert_eq!(moved, adv, "шаг счётчика ≠ round(ω·(1+(kA)²/2)·dt)");
        }
        assert!(shift_seen > 5e-4, "стоксов сдвиг не проявился: {shift_seen}");
    }

    #[test]
    fn stokes_crest_sharper_trough_flatter() {
        // 2-й порядок Стокса: гребень острее, ложбина площе —
        // h_max + h_min = k·A²/2 > 0 (у линейной волны ровно 0).
        let probe = |nonlinear: bool| -> (f64, f64, f64, f64) {
            // Сид с живым бином 2k (DC/Найквист/коллизии пропускаются).
            let seed = (0u64..64)
                .find(|&s| {
                    let w = generate(&WaterParams {
                        n: 64, modes: 1, seed: s, wind: 12.0,
                        tau_wind: 3600.0, viscosity: 1e-8, nonlinear: true,
                        ..Default::default()
                    });
                    !w.second_skip[0]
                })
                .expect("есть сид с живым 2k-бином");
            let p = WaterParams {
                n: 64, modes: 1, seed, wind: 12.0,
                tau_wind: 3600.0, viscosity: 1e-8, nonlinear,
                ..Default::default()
            };
            let w = generate(&p);
            let m = &w.modes[0];
            let a = w.amplitude(m);
            let lambda = TAU / m.k;
            let kx = m.kx_phys / m.k;
            let ky = m.ky_phys / m.k;
            let (mut hmax, mut hmin) = (f64::NEG_INFINITY, f64::INFINITY);
            let n = 4000;
            for i in 0..n {
                let t = i as f64 * lambda / n as f64;
                let h = w.surface_at(50.0 + t * kx, 50.0 + t * ky).0;
                hmax = hmax.max(h);
                hmin = hmin.min(h);
            }
            (hmax, hmin, a, m.k)
        };
        let (hmax, hmin, a, k) = probe(true);
        let skew = k * a * a / 2.0;
        assert!(skew > 1e-3, "2-я гармоника мала для замера: {skew}");
        assert!(
            hmax + hmin > 0.25 * skew,
            "гребень не заострился: сумма {:+.5} против k·A²/2 = {skew:.5}",
            hmax + hmin
        );
        // Контроль: линейная волна симметрична.
        let (hmax0, hmin0, a0, _) = probe(false);
        assert!((hmax0 + hmin0).abs() < 0.01 * a0, "линейная волна несимметрична");
    }

    #[test]
    fn michell_slope_corridor() {
        // Снос гребней (Мичелл): склон моды живёт в коридоре — срез до
        // 0.9·порога, отрастание за кулдаун (3 периода) ограничено
        // равновесием: bound = 0.9·s_b + (s_eq − 0.9·s_b)·(1 − e^(−3T/τ)).
        // Зонд: при ветре 20 м/с и 32 модах максимальный наклон ≈ 0.106 —
        // порог 0.08 достижим, сносы реально работают.
        let p = WaterParams {
            n: 64, modes: 32, seed: 42, wind: 20.0,
            steepness_break: 0.08, whitecap_onset: 40.0,
            viscosity: 1e-8, tau_wind: 30.0,
            nonlinear: true, ..Default::default()
        };
        let mut w = generate(&p);
        let bounds: Vec<f64> = w
            .modes
            .iter()
            .map(|m| {
                let amp_at = |u: f64| {
                    let uu = u.clamp(0.0, w.level_max as f64);
                    2.0 * w.a_min * (w.a_max / w.a_min).powf(uu / w.level_max as f64)
                        / (w.n * w.n) as f64
                };
                let s0 = m.k * amp_at(m.acc as f64 / FP);
                let seq = m.k * amp_at(m.u_eq);
                let base = 0.9 * w.steepness_break;
                let cool = 3.0 * TAU / m.omega; // кулдаун
                let regrow = base + (seq.max(s0) - base) * (1.0 - (-cool / w.tau_wind).exp());
                s0.max(seq).max(regrow) * 1.06
            })
            .collect();
        let dt = 1.0 / 60.0;
        for step in 1..=3600 {
            w.step(dt);
            if step % 15 == 0 {
                for (i, m) in w.modes.iter().enumerate() {
                    let s = m.k * w.amplitude(m);
                    assert!(
                        s <= bounds[i] + 1e-12,
                        "мода {i}: склон {s:.4} покинул коридор {:.4}",
                        bounds[i]
                    );
                }
            }
        }
        assert!(w.spawned_total >= 1, "сносы Мичелла не сработали");
    }

    #[test]
    fn michell_event_energy_conserved() {
        // Бухгалтерия сноса: ΔE мод == энергия когерентного вихря + тепло.
        // Первое событие на пустом слое — чистый транзит (без растяжения
        // уже живых вихрей), одномоментная проверка тождества.
        let p = WaterParams {
            n: 64, modes: 4, seed: 42, wind: 20.0,
            steepness_break: 0.10, whitecap_onset: 40.0,
            tau_wind: 3600.0, viscosity: 1e-8,
            nonlinear: true, ..Default::default()
        };
        let mut w = generate(&p);
        let dt = 1.0 / 60.0;
        let (var_b, heat_b) = {
            let mut it = 0usize;
            loop {
                it += 1;
                assert!(it < 100_000, "снос Мичелла не наступил");
                let vb = w.variance();
                let hb = w.heat_fp;
                w.step(dt);
                if w.spawned_total > 0 {
                    break (vb, hb);
                }
            }
        };
        let transferred = w.gravity * (var_b - w.variance());
        let e_breakers: f64 = w.breakers.iter().map(|v| v.energy()).sum();
        let heat = (w.heat_fp - heat_b) as f64 / FP;
        assert!(transferred > 0.0, "снос не перенёс энергию");
        let res = (transferred - (e_breakers + heat)).abs();
        assert!(
            res < 1e-2 * transferred + 1e-15,
            "бухгалтерия сноса: перенос {transferred:.6} против книг {:.6}",
            e_breakers + heat
        );
    }

    #[test]
    fn whitecap_beaufort_gate() {
        // Наблюдательный триггер: до 9 м/с барашков нет вовсе;
        // при 15 м/с — стабильный поток событий.
        let base = WaterParams {
            n: 64, modes: 48, seed: 42, wind: 8.0,
            steepness_break: 0.60, // Мичелл выключен — изолируем барашки
            nonlinear: true, ..Default::default()
        };
        let mut calm = generate(&base);
        for _ in 0..7200 {
            calm.step(1.0 / 60.0);
        }
        assert_eq!(calm.spawned_total, 0, "барашки до порога Бофорта");
        assert!(calm.breakers.is_empty());

        let mut breeze = generate(&WaterParams { wind: 15.0, ..base.clone() });
        for _ in 0..7200 {
            breeze.step(1.0 / 60.0);
        }
        assert!(
            breeze.spawned_total >= 5,
            "барашки не пошли: {}",
            breeze.spawned_total
        );
        assert!(!breeze.breakers.is_empty());
    }

    #[test]
    fn whitecap_event_energy_conserved() {
        // Бухгалтерия барашка: оплата с мод == вихрь + тепло (первое
        // событие, одномоментное тождество — как у Мичелла).
        let p = WaterParams {
            n: 64, modes: 48, seed: 42, wind: 15.0,
            steepness_break: 0.60, tau_wind: 3600.0, viscosity: 1e-8,
            nonlinear: true, ..Default::default()
        };
        let mut w = generate(&p);
        let dt = 1.0 / 60.0;
        let (var_b, heat_b) = {
            let mut it = 0usize;
            loop {
                it += 1;
                assert!(it < 100_000, "белый барашек не наступил");
                let vb = w.variance();
                let hb = w.heat_fp;
                w.step(dt);
                if w.spawned_total > 0 {
                    break (vb, hb);
                }
            }
        };
        let transferred = w.gravity * (var_b - w.variance());
        let e_b: f64 = w.breakers.iter().map(|v| v.energy()).sum();
        let heat = (w.heat_fp - heat_b) as f64 / FP;
        assert!(transferred > 0.0);
        let res = (transferred - (e_b + heat)).abs();
        assert!(
            res < 2e-2 * transferred + 1e-15,
            "бухгалтерия барашка: {transferred:.6} против книг {:.6}",
            e_b + heat
        );
    }

    #[test]
    fn lamb_oseen_core_diffusion() {
        // Ядро Ламб–Озеена: r²(t) = r₀² + 8νt — линейный рост площади
        // ядра (вязкое расплывание), на живых вихрях шторма.
        let p = WaterParams {
            n: 64, modes: 48, seed: 42, wind: 20.0,
            viscosity: 1e-4, nonlinear: true, ..Default::default()
        };
        let mut w = generate(&p);
        let dt = 1.0 / 60.0;
        for _ in 0..900 {
            w.step(dt); // накапливаем барашки
        }
        assert!(!w.breakers.is_empty(), "нет живых вихрей для замера");
        let snap: Vec<BreakerVortex> = w.breakers.clone();
        let k = 120usize;
        for _ in 0..k {
            w.step(dt);
        }
        let dt_fp = (dt * FP).round() as i64;
        let t = k as f64 * dt;
        let mut checked = 0usize;
        for v0 in &snap {
            let age_target = v0.age_fp + k as i64 * dt_fp;
            if age_target >= v0.ttl_fp {
                continue; // уйдёт на пенсию внутри окна — пропускаем
            }
            // Личность: ttl и Γ₀ — константы рождения; r²-траектория
            // зависит только от r₀ и t, дубликаты не ломают замер.
            let v = match w
                .breakers
                .iter()
                .find(|v| v.ttl_fp == v0.ttl_fp && v.gamma0_fp == v0.gamma0_fp)
            {
                Some(v) => v,
                None => continue,
            };
            let r2_0 = v0.radius() * v0.radius();
            let r2_1 = v.radius() * v.radius();
            let want = r2_0 + 8.0 * w.viscosity * t;
            assert!(
                (r2_1 - want).abs() < 1e-6,
                "ядро: r² = {r2_1:.8}, Ламб–Озеен {want:.8}"
            );
            checked += 1;
        }
        assert!(checked >= 1, "ни одного вихря не прожило замер");
    }

    #[test]
    fn stokes_theorem_circulation() {
        // Первое РОТАЦИОННОЕ поле движка: циркуляция Ламб–Озеена по
        // контуру = Γ·(1 − e^(−R²/r²)) — теорема Стокса. Контраст:
        // волновая часть безвихревая — её циркуляция нуль.
        let p = WaterParams {
            n: 64, modes: 1, seed: 5, wind: 5.0,
            nonlinear: true, ..Default::default()
        };
        let mut w = generate(&p);
        let gamma = 0.5f64;
        w.breakers.push(BreakerVortex {
            x_fp: (50.0 * FP).round() as i64,
            y_fp: (50.0 * FP).round() as i64,
            gamma_fp: (gamma * FP).round() as i64,
            gamma0_fp: (gamma * FP).round() as i64,
            r_fp: (1.0 * FP).round() as i64,
            age_fp: (10.0 * FP).round() as i64, // зрелый: огибающая = 1
            ttl_fp: (1e6 * FP).round() as i64,
        });
        let circ = |radius: f64, full: bool| -> f64 {
            let n = 720usize;
            let mut s = 0.0;
            for i in 0..n {
                let t = TAU * (i as f64 + 0.5) / n as f64;
                let (x, y) = (50.0 + radius * t.cos(), 50.0 + radius * t.sin());
                let (ux, uy, _) = if full { w.flow_full_at(x, y) } else { w.flow_at(x, y) };
                // dl = R·t̂·dθ, обход против часовой стрелки.
                s += (ux * (-t.sin()) + uy * t.cos()) * radius * TAU / n as f64;
            }
            s
        };
        let r = 1.0f64;
        // Далёкий контур (R = 5r): почти вся циркуляция Γ.
        let c_far = circ(5.0 * r, true);
        let want_far = gamma * (1.0 - (-(25.0f64)).exp());
        assert!(
            (c_far - want_far).abs() < 0.02 * gamma,
            "циркуляция R=5r: {c_far:.6} против Γ·(1−e⁻²⁵)={want_far:.6}"
        );
        // Контур в ядре (R = r): профиль Ламб–Озеена Γ·(1−e⁻¹).
        let c_core = circ(r, true);
        let want_core = gamma * (1.0 - (-1.0f64).exp());
        assert!(
            (c_core - want_core).abs() < 0.04 * gamma,
            "профиль ядра: {c_core:.6} против Γ(1−e⁻¹)={want_core:.6}"
        );
        // Контраст: чистые волны — циркуляция нуль (безвихревость).
        let c_wave = circ(5.0 * r, false);
        assert!(c_wave.abs() < 0.01 * gamma, "волны завихрились: {c_wave}");
    }

    #[test]
    fn stokes_drift_emerges_from_advection() {
        // Дрейф Стокса возникает сам: лагранжева частица в орбитальном
        // поле волн уходит вперёд по волне со скоростью ω·A²·k/2 —
        // никакого отдельного члена «дрейф» в движке нет.
        let p = WaterParams {
            n: 64, modes: 1, seed: 11, wind: 6.0,
            tau_wind: 3600.0, viscosity: 1e-8,
            nonlinear: true, ..Default::default()
        };
        let mut w = generate(&p);
        let m = w.modes[0].clone();
        let a = w.amplitude(&m);
        let period = TAU / m.omega;
        let n_per = 40usize;
        let total = n_per as f64 * period;
        let dt = 1.0 / 60.0;
        let steps = (total / dt).round() as usize;
        w.breakers.push(BreakerVortex {
            x_fp: (50.0 * FP).round() as i64,
            y_fp: (50.0 * FP).round() as i64,
            gamma_fp: 0,
            gamma0_fp: 0,
            r_fp: (0.05 * FP).round() as i64,
            age_fp: 0,
            ttl_fp: (1e7 * FP).round() as i64,
        });
        for _ in 0..steps {
            w.step(dt);
        }
        let (x, y) = w.breakers[0].pos();
        let (dx, dy) = (x - 50.0, y - 50.0);
        // Прогноз: u_s = ω·A²/2 вдоль вектора k.
        let us = 0.5 * m.omega * a * a;
        let ex = us * m.kx_phys * total;
        let ey = us * m.ky_phys * total;
        let got = (dx * dx + dy * dy).sqrt();
        let want = (ex * ex + ey * ey).sqrt();
        assert!(want > 0.5, "дрейф слишком мал для замера: {want} м");
        assert!(dx * ex + dy * ey > 0.0, "частица ушла против волны");
        assert!(
            (got - want).abs() < 0.2 * want,
            "дрейф {got:.4} м против Стокса {want:.4} м"
        );
    }

    #[test]
    fn gamma_stretch_coupling_and_cap() {
        // Связка «растяжение ↔ деформация»: за шаг Γ' = Γ·(1 + ∂u/∂x·dt)
        // ровно (клип ±0.25/шаг, читаем деформацию до шага — как движок);
        // в шторме |Γ| никогда не выше потолка 2·Γ₀.
        let p = WaterParams {
            n: 64, modes: 1, seed: 5, wind: 5.0,
            nonlinear: true, ..Default::default()
        };
        let mut w = generate(&p);
        let (bx, by) = (37.0f64, 53.0f64);
        let gamma = 0.3f64;
        w.breakers.push(BreakerVortex {
            x_fp: (bx * FP).round() as i64,
            y_fp: (by * FP).round() as i64,
            gamma_fp: (gamma * FP).round() as i64,
            gamma0_fp: (gamma * FP).round() as i64,
            r_fp: (1.0 * FP).round() as i64,
            age_fp: (10.0 * FP).round() as i64,
            ttl_fp: (1e6 * FP).round() as i64,
        });
        let dt = 1.0 / 60.0;
        w.step(dt);
        // Деформация, которую видел движок: поле на СТАРОЙ позиции с
        // финальным состоянием шага (фаза уже убежала, амплитуда обновлена;
        // в этом тесте ни Мичелл, ни барашки не мешают — ветер 5 м/с).
        let s_after = w.strain_xx_at(bx, by);
        let expect = gamma * (1.0 + (s_after * dt).clamp(-0.25, 0.25));
        let got = w.breakers[0].gamma();
        assert!((got - expect).abs() < 1e-9, "растяжение: {got} против {expect}");

        // Потолок циркуляции в живом шторме.
        let ps = WaterParams {
            n: 64, modes: 96, seed: 42, wind: 20.0,
            nonlinear: true, ..Default::default()
        };
        let mut storm = generate(&ps);
        for i in 1..=3000 {
            storm.step(dt);
            if i % 100 == 0 {
                for v in &storm.breakers {
                    assert!(
                        v.gamma_fp.abs() <= 2 * v.gamma0_fp + 1,
                        "Γ вылетел за потолок 2·Γ₀"
                    );
                }
            }
        }
    }

    #[test]
    fn breaker_bump_rises_and_retires() {
        // Жизнь барашка: горб напора вырастает (огибающая созревания),
        // живёт и к ttl уходит в тепло целиком.
        let p = WaterParams {
            n: 64, modes: 48, seed: 42, wind: 15.0,
            steepness_break: 0.60, nonlinear: true, ..Default::default()
        };
        let mut w = generate(&p);
        let dt = 1.0 / 60.0;
        loop {
            w.step(dt);
            if !w.breakers.is_empty() {
                break;
            }
        }
        let born = w.breakers[0].clone();
        let key = |v: &BreakerVortex| (v.ttl_fp, v.gamma0_fp);
        let mut amp_first = 0.0f64;
        let mut amp_max = 0.0f64;
        let mut last_energy = 0.0f64;
        let mut heat_at_last = w.heat_fp;
        let mut retired = false;
        for _ in 0..(born.ttl() / dt) as usize + 120 {
            let info = w.breakers_info();
            match w.breakers.iter().position(|v| key(v) == key(&born)) {
                Some(i) => {
                    let amp = info[i].2;
                    if amp_first == 0.0 {
                        amp_first = amp;
                    }
                    amp_max = amp_max.max(amp);
                    last_energy = w.breakers[i].energy();
                    heat_at_last = w.heat_fp;
                }
                None => {
                    retired = true;
                    break;
                }
            }
            w.step(dt);
        }
        assert!(retired, "барашек не ушёл на пенсию");
        assert!(amp_first < amp_max, "горб не вырос: {amp_first} → {amp_max}");
        assert!(amp_max > 0.0, "горб не проявился");
        // Пенсия: остаток энергии вихря — в тепло (тепло только растёт).
        let heat_gain = (w.heat_fp - heat_at_last) as f64 / FP;
        assert!(
            heat_gain >= 0.5 * last_energy,
            "тепло пенсии {heat_gain:.6} < энергии вихря {last_energy:.6}"
        );
    }

    #[test]
    fn height_field_surface_at_agree_in_storm() {
        // В шторме с живыми барашками оба пути поверхности — FFT
        // (контейнер + вторые гармоники + штампы горбов) и аналитика
        // surface_at — согласены точка в точку.
        let p = WaterParams {
            n: 64, modes: 96, seed: 42, wind: 20.0,
            nonlinear: true, ..Default::default()
        };
        let mut w = generate(&p);
        for _ in 0..2400 {
            w.step(1.0 / 60.0);
        }
        assert!(w.breakers.len() >= 2, "шторм без барашков");
        let f = w.height_field();
        let cell = w.domain / w.n as f64;
        let mut maxdiff = 0.0f64;
        for y in 0..w.n {
            for x in 0..w.n {
                let h = w.surface_at(x as f64 * cell, y as f64 * cell).0;
                let d = (h - f[y * w.n + x]).abs();
                maxdiff = maxdiff.max(d);
            }
        }
        assert!(maxdiff < 5e-9, "FFT и аналитика разошлись: {maxdiff:.3e}");
    }

    #[test]
    fn container_second_harmonics_consistent() {
        // Вторые гармоники Стокса в контейнере: живые бины ±2k уникальны
        // и не бьют в DC/Найквист; мультимножество эрмитово (±2k пары
        // сопряжены по фазе, амплитуды равны); квантование фаз
        // идемпотентно — FFT-путь и аналитический путь неотличимы.
        let p = WaterParams {
            n: 64, modes: 96, seed: 42, wind: 10.0,
            nonlinear: true, ..Default::default()
        };
        let w = generate(&p);
        // 1) Живые бины: канон.
        let mut uniq = HashSet::new();
        let ni = w.n as i64;
        for (i, m) in w.modes.iter().enumerate() {
            if w.second_skip[i] {
                continue;
            }
            let k2x = (2 * m.kx as i64).rem_euclid(ni);
            let k2y = (2 * m.ky as i64).rem_euclid(ni);
            assert!(
                k2x != 0 && k2x != ni / 2 && k2y != 0 && k2y != ni / 2,
                "живой бин 2k в DC/Найквисте"
            );
            assert!(uniq.insert((k2x, k2y)), "коллизия живых бинов ±2k");
        }
        assert!(!uniq.is_empty(), "нет живых вторых гармоник");
        // 2) Эрмитовость: мультимножество (bin, q, s) закрыто сопряжением.
        let spec2 = w.second_harmonic_spectrum().expect("спектр 2-гармоник");
        let n = w.n as u32;
        let micro = 3u64.pow(w.phase_trits + 3);
        let mut counts: HashMap<(u32, u32, u64, u64), usize> = HashMap::new();
        for h in &spec2.harmonics {
            *counts.entry((h.kx, h.ky, h.q, h.s)).or_insert(0) += 1;
        }
        for (&(kx, ky, q, s), &c) in counts.iter() {
            let neg = ((n - kx) % n, (n - ky) % n);
            let s_neg = (micro as i64 - s as i64).rem_euclid(micro as i64) as u64;
            let c_neg = counts.get(&(neg.0, neg.1, q, s_neg)).copied().unwrap_or(0);
            assert_eq!(c, c_neg, "эрмитова пара ±2k не сопряжена");
        }
        // 3) Идемпотентность фазового квантования (2φ → s → 2φ → s).
        let p3 = w.phase_trits + 3;
        for m in &w.modes {
            let phi2 = 2.0 * w.phase(m);
            let s = vortex::phase_quant(phi2, p3);
            let back = vortex::phase_dequant(s, p3);
            assert_eq!(
                vortex::phase_quant(back, p3),
                s,
                "квантование фазы не идемпотентно"
            );
        }
    }

    #[test]
    fn determinism_storm_bit_to_bit() {
        // Шторм с барашками воспроизводим бит-в-бит: хеш покрывает
        // контейнер + сайдкар вихрей + тепло + аккумулятор + RNG.
        let p = WaterParams {
            n: 64, modes: 96, seed: 42, wind: 20.0,
            nonlinear: true, ..Default::default()
        };
        let run = || {
            let mut w = generate(&p);
            for _ in 0..5400 {
                w.step(1.0 / 60.0);
            }
            (w.water_hash(), w.spawned_total, w.breakers.len(), w.variance())
        };
        let a = run();
        let b = run();
        assert_eq!(a.0, b.0, "хеш шторма разошёлся");
        assert_eq!(a.1, b.1, "число событий разошлось");
        assert!(a.2 > 0, "шторм без живых барашков");
        assert_eq!(a.3, b.3, "дисперсия разошлась");
    }
}
