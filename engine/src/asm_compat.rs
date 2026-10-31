//! Архитектурно-независимое ядро проективной арифметики [N:D] и фасад
//! над VAULT-ASM (v0.93.0, fix macOS/aarch64 CI: E0433 × 13, 11 ранов).
//!
//! ПРОБЛЕМА: `crate::asm` целиком гейтован `cfg(target_arch = "x86_64")`
//! (микроядра — чистый x86_64-ассемблер), но чисто-Rust СЕМАНТИКА —
//! структура [`Proj`], канонические сравнения, безопасные деления,
//! LENS-барьер, FEP-энергия — нужна ВСЕМ архитектурам: `engine.rs`,
//! `entity_graph.rs` и `literary/engine.rs` используют её в негейтовом
//! коде. Каноническая схема движка («скалярный эталон всегда, SIMD-путь —
//! cfg», см. teddy.rs/meta_compiler.rs) здесь доведена до модуля:
//!
//! * чистые определения живут ЗДЕСЬ и компилируются везде;
//! * x86_64-специфика (extern-микроядра `poler_az_*` из `asm::zero_asm`,
//!   `asm::lens_asm::filter`, `asm::fep_asm::energy/grad`) — за cfg;
//! * `asm::zero_asm` реэкспортирует перенесённое — пути
//!   `crate::asm::zero_asm::*` на x86_64 НЕ меняются; канонический путь
//!   для кросс-платформенного кода — `crate::asm_compat::*`.
//!
//! Паритет asm/скаляр (Фаза 5 протокола AI-инженера) держат тесты внизу.

/// Код все-⊙ байта pack5: (3⁵−1)/2 = 121 — единственная кодировка
/// троичного нуля в 5-тритном байте (Калькулятор Всего).
pub const TRIT5_ZERO_CODE: u8 = 121;

/// Троичный радиус u64: 40 тритов = 8 pack5-байтов.
pub const TRITS_PER_U64: usize = 40;

/// (3⁴⁰−1)/2 = 6078832729528464400 < 2⁶³ — сбалансированный диапазон
/// 40 тритов влезает в i64 БЕЗ переполнения; |MIN| = |MAX| ТОЧНО
/// (Калькулятор Всего; контраст: 2⁶³ − (2⁶³−1) = 1 у дополнения до двух).
pub const TRIT_U64_RADIUS: i64 = 6078832729528464400;

/// Симметричный положительный полюс насыщения i64.
pub const POLE_I64_POS: i64 = i64::MAX;

/// Симметричный отрицательный полюс насыщения i64: −(2⁶³−1) — НЕ MIN,
/// чтобы полюс сам не был точкой-изгоем негации.
pub const POLE_I64_NEG: i64 = -i64::MAX;

/// ULP Q30 — шаг решётки CORDIC-ротора (Калькулятор Всего:
/// 2⁻³⁰ = 9.313225746154785e-10). Радиус изоляции машинного нуля.
pub const Q30_ULP_F64: f64 = 9.313225746154785e-10;

/// Класс точки [N:D] на проективной прямой ℝP¹.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjClass {
    /// Обыкновенная дробь N/D, D≠0, N≠0.
    Finite = 0,
    /// [0:D≠0] — южный полюс: истинный абсолютный ноль (изолирован,
    /// уникален, центр симметрии решётки).
    Zero = 1,
    /// [N≠0:0] — северный полюс: ∞. Проективно [1:0]≡[−1:0] — ОДНА точка
    /// (λ-инвариант однородных координат замыкает +∞ и −∞).
    Pole = 2,
    /// [0:0] — калибровочная неопределённость: единственный запрещённый
    /// класс, детектируется ОДНИМ тестом вместо NaN-отравления.
    Gauge = 3,
}

impl ProjClass {
    fn from_code(code: u64) -> Self {
        match code {
            1 => ProjClass::Zero,
            2 => ProjClass::Pole,
            3 => ProjClass::Gauge,
            _ => ProjClass::Finite,
        }
    }
}

/// Проективная дробь [N:D]: значение N/D; D=0 — полюс (∞), не сбой;
/// [0:0] — калибровка. Деления НЕТ: умножения и обмены.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Proj {
    pub n: i64,
    pub d: i64,
}

impl Proj {
    /// Абсолютный ноль [0:1] — южный полюс ℝP¹.
    pub const ZERO: Proj = Proj { n: 0, d: 1 };
    /// Единица [1:1].
    pub const ONE: Proj = Proj { n: 1, d: 1 };
    /// Полюс [1:0] = ∞ (≡ [−1:0] — одна проективная точка).
    pub const INF: Proj = Proj { n: 1, d: 0 };
    /// Калибровочная неопределённость [0:0] — единственный запрещённый класс.
    pub const GAUGE: Proj = Proj { n: 0, d: 0 };

    /// Целое как дробь [v:1].
    pub fn from_int(v: i64) -> Self {
        Proj { n: v, d: 1 }
    }

    /// Класс точки: asm-ядро (sete+lea, без веток).
    pub fn classify(&self) -> ProjClass {
        #[cfg(target_arch = "x86_64")]
        {
            ProjClass::from_code(unsafe { poler_az_class(self.n, self.d) })
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            classify_scalar(self.n, self.d)
        }
    }

    /// Инверсия = ОБМЕН [D:N]: нуль ↔ полюс зеркальны; 1/x без деления.
    pub fn inv(&self) -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            let mut out = [0i64; 2];
            unsafe { poler_az_inv(self.n, self.d, out.as_mut_ptr()) };
            Proj { n: out[0], d: out[1] }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            // обмен [D:N] — та же семантика без ядра
            Proj { n: self.d, d: self.n }
        }
    }

    /// Умножение [N1·N2 : D1·D2]; переполнение — насыщение в полюс
    /// со знаком значения (asm-ядро).
    pub fn mul(&self, o: &Proj) -> Proj {
        #[cfg(target_arch = "x86_64")]
        {
            let mut out = [0i64; 2];
            unsafe { poler_az_proj_mul(self.n, self.d, o.n, o.d, out.as_mut_ptr()) };
            Proj { n: out[0], d: out[1] }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            proj_mul_scalar(self, o)
        }
    }

    /// ДЕЛЕНИЕ БЕЗ ДЕЛЕНИЯ: a/b = [N1·D2 : D1·N2] перекрёстным
    /// умножением (asm-ядро). b=0 → полюс (∞), 0/0 → калибровка [0:0],
    /// x/∞ → нуль, ∞/x → полюс — «деление на ноль» перестаёт быть
    /// событием: это координата на ℝP¹, а не исключение.
    pub fn div(&self, o: &Proj) -> Proj {
        #[cfg(target_arch = "x86_64")]
        {
            let mut out = [0i64; 2];
            unsafe { poler_az_proj_div(self.n, self.d, o.n, o.d, out.as_mut_ptr()) };
            Proj { n: out[0], d: out[1] }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            proj_div_scalar(self, o)
        }
    }

    /// Аддитивная негация [−N:D] — тотальна на троично-симметричной
    /// решётке (контраст: i64::MIN ломает её в дополнении до двух).
    pub fn neg(&self) -> Self {
        Proj { n: self.n.wrapping_neg(), d: self.d }
    }

    /// Сложение [N1·D2+N2·D1 : D1·D2] в i128-промежуточных — ТОЧНО
    /// (1/2+1/3 = [5:6] без всяких 0.833…4). Переполнение i64 — насыщение:
    /// |числитель|>|знаменатель| → полюс знака, иначе нуль.
    pub fn add(&self, o: &Proj) -> Proj {
        let n = self.n as i128 * o.d as i128 + o.n as i128 * self.d as i128;
        let d = self.d as i128 * o.d as i128;
        let fits = |v: i128| v >= i64::MIN as i128 && v <= i64::MAX as i128;
        if fits(n) && fits(d) {
            return Proj { n: n as i64, d: d as i64 };
        }
        // насыщение по величине: полюс знака либо нуль
        let sign = (n < 0) != (d < 0);
        if n.unsigned_abs() > d.unsigned_abs() {
            Proj { n: if sign { POLE_I64_NEG } else { POLE_I64_POS }, d: 0 }
        } else {
            Proj::ZERO
        }
    }

    /// Каноническая форма: НОД-редукция (бинарный gcd) + знаменатель > 0.
    /// [−1:0] → [1:0]: плюс- и минус-полюс — ОДНА проективная точка.
    pub fn canonicalize(&self) -> Proj {
        match self.classify() {
            ProjClass::Gauge => Proj::GAUGE,
            ProjClass::Zero => Proj::ZERO,
            ProjClass::Pole => Proj { n: 1, d: 0 },
            ProjClass::Finite => {
                let mut n = self.n;
                let mut d = self.d;
                if d < 0 {
                    n = -n;
                    d = -d;
                }
                let g = gcd64(n.unsigned_abs(), d.unsigned_abs()) as i64;
                if g > 1 {
                    n /= g;
                    d /= g;
                }
                Proj { n, d }
            }
        }
    }

    /// Приближённое значение (только для отображения — не для сравнений!).
    pub fn value_f64(&self) -> f64 {
        self.n as f64 / self.d as f64
    }
}

/// Бинарный НОД (без деления — сдвиги и вычитания; «ноль Python»-стиль
/// железа). gcd(12,18)=6 — сверено с Калькулятором Всего.
pub fn gcd64(mut a: u64, mut b: u64) -> u64 {
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }
    let za = a.trailing_zeros();
    let zb = b.trailing_zeros();
    a >>= za;
    b >>= zb;
    // общая двойная степень: минимум валиций, фиксируется ОДИН раз;
    // нули, стрипаемые в цикле из b, НЕ входят в НОД (a нечётно)
    let shift = za.min(zb);
    loop {
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        b -= a;
        if b == 0 {
            return a << shift;
        }
        b >>= b.trailing_zeros();
    }
}

/// ТОЧНОЕ сравнение канонических дробей (d1>0, d2>0): 128-битные
/// произведения N1·D2 vs N2·D1 — БЕЗ деления. asm-ядро.
pub fn cmp_canonical(a: &Proj, b: &Proj) -> std::cmp::Ordering {
    debug_assert!(a.d > 0 && b.d > 0, "контракт канонической формы: знаменатель > 0");
    #[cfg(target_arch = "x86_64")]
    {
        let r = unsafe { poler_az_cmp(a.n, a.d, b.n, b.d) };
        r.cmp(&0)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        cmp_canonical_scalar(a, b)
    }
}

/// Безопасное деление i32 (Q-домен): НИКОГДА не поднимает #DE.
/// den==0 → симметричный полюс ±(2³¹−1) по знаку числителя; 0/0 → 0;
/// MIN/−1 → +MAX (асимметрия дополнения до двух излечена насыщением).
pub fn div_safe_i32(num: i32, den: i32) -> i32 {
    #[cfg(target_arch = "x86_64")]
    {
        unsafe { poler_az_div_safe_i32(num, den) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        div_safe_i32_scalar(num, den)
    }
}

/// Кутритная ноль-сумма: [N, D, −(N+D)] — сумма проекций ≡ 0 ПО
/// ПОСТРОЕНИЮ (A₂-плоскость, фазы 1+ω+ω²=0). asm-ядро: lea+neg.
pub fn qutrit3(n: i64, d: i64) -> [i64; 3] {
    #[cfg(target_arch = "x86_64")]
    {
        let mut out = [0i64; 3];
        unsafe { poler_az_qutrit3(n, d, out.as_mut_ptr()) };
        out
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        qutrit3_scalar(n, d)
    }
}

// ------------------------- скалярные зеркала -------------------------

/// Скалярное зеркало `poler_az_class` (референс тестов, не-x86_64).
pub fn classify_scalar(n: i64, d: i64) -> ProjClass {
    ProjClass::from_code((n == 0) as u64 + 2 * (d == 0) as u64)
}

/// Скалярное зеркало `poler_az_div_safe_i32`.
pub fn div_safe_i32_scalar(num: i32, den: i32) -> i32 {
    if den == 0 {
        return if num > 0 {
            i32::MAX
        } else if num < 0 {
            -i32::MAX
        } else {
            0
        };
    }
    if den == -1 && num == i32::MIN {
        return i32::MAX;
    }
    num / den
}

/// Скалярное зеркало `poler_az_trit_zero_count`.
pub fn trit_zero_count_scalar(packed: &[u8]) -> usize {
    packed.iter().filter(|&&b| b == TRIT5_ZERO_CODE).count()
}

/// Скалярное зеркало `poler_az_proj_mul` (checked-умножения).
pub fn proj_mul_scalar(a: &Proj, b: &Proj) -> Proj {
    match (a.n.checked_mul(b.n), a.d.checked_mul(b.d)) {
        (Some(n), Some(d)) => Proj { n, d },
        _ => {
            // знак произведения (n1/d1)*(n2/d2) — XOR четырёх знаков
            let negative = (a.n < 0) ^ (a.d < 0) ^ (b.n < 0) ^ (b.d < 0);
            Proj { n: if negative { POLE_I64_NEG } else { POLE_I64_POS }, d: 0 }
        }
    }
}

/// Скалярное зеркало `poler_az_proj_div` (checked, перекрёстное).
pub fn proj_div_scalar(a: &Proj, b: &Proj) -> Proj {
    match (a.n.checked_mul(b.d), a.d.checked_mul(b.n)) {
        (Some(n), Some(d)) => Proj { n, d },
        _ => {
            let negative = (a.n < 0) ^ (a.d < 0) ^ (b.n < 0) ^ (b.d < 0);
            Proj { n: if negative { POLE_I64_NEG } else { POLE_I64_POS }, d: 0 }
        }
    }
}

/// Скалярное зеркало `poler_az_cmp` (i128-произведения).
pub fn cmp_canonical_scalar(a: &Proj, b: &Proj) -> std::cmp::Ordering {
    debug_assert!(a.d > 0 && b.d > 0);
    let p1 = a.n as i128 * b.d as i128;
    let p2 = b.n as i128 * a.d as i128;
    p1.cmp(&p2)
}

/// Скалярное зеркало `poler_az_qutrit3`.
pub fn qutrit3_scalar(n: i64, d: i64) -> [i64; 3] {
    [n, d, -(n + d)]
}

/// Безопасное деление f32 — семантика АЗУ в плавающем домене:
/// * `den ≠ 0` — обычное деление;
/// * `den = 0, num ≠ 0` — ПОЛЮС: насыщение ±f32::MAX по знаку числителя
///   (∞ как предельная точка ℝP¹, без NaN-отравления конвейера);
/// * `0/0` — КАЛИБРОВКА [0:0] → 0.0 (принятое значение неопределённости).
///
/// Применение: нормировки FEP `x/(1+x)` (полюс x=−1), STDP-softmax
/// (Σ=0 → равномерные награды), любая статистика с делением на сумму.
pub fn div_safe_f32(num: f32, den: f32) -> f32 {
    if den != 0.0 {
        num / den
    } else if num == 0.0 {
        0.0
    } else if num > 0.0 {
        f32::MAX
    } else {
        f32::MIN
    }
}

#[cfg(test)]
mod div_safe_f32_tests {
    use super::div_safe_f32;

    #[test]
    fn div_safe_f32_pole_semantics() {
        assert_eq!(div_safe_f32(1.0, 2.0), 0.5);
        // полюс: насыщение по знаку числителя (НЕ NaN!)
        assert_eq!(div_safe_f32(1.0, 0.0), f32::MAX);
        assert_eq!(div_safe_f32(-2.0, 0.0), f32::MIN);
        // калибровка 0/0 → 0
        assert_eq!(div_safe_f32(0.0, 0.0), 0.0);
        // FEP-нормировка x/(1+x): полюс x = −1 больше не отравляет
        let norm = |x: f32| div_safe_f32(x, 1.0 + x);
        assert!(norm(-1.0).is_finite());
        assert_eq!(norm(1.0), 0.5);
        assert_eq!(norm(0.0), 0.0);
    }
}

// Микроядра x86_64 (символы определены global_asm! в asm::zero_asm —
// модуль существует только на x86_64, поэтому и ссылки — только там).
#[cfg(target_arch = "x86_64")]
extern "sysv64" {
    fn poler_az_class(n: i64, d: i64) -> u64;
    fn poler_az_inv(n: i64, d: i64, out: *mut i64);
    fn poler_az_proj_mul(n1: i64, d1: i64, n2: i64, d2: i64, out: *mut i64);
    fn poler_az_proj_div(n1: i64, d1: i64, n2: i64, d2: i64, out: *mut i64);
    fn poler_az_cmp(n1: i64, d1: i64, n2: i64, d2: i64) -> i64;
    fn poler_az_div_safe_i32(num: i32, den: i32) -> i32;
    fn poler_az_qutrit3(n: i64, d: i64, out: *mut i64);
}


// ═══════════════════ LENS-барьер (из asm/lens_asm.rs) ═══════════════════

/// Канонический порог сокровищницы (lens_index.rs: `weight > 0.05`).
/// Живёт здесь: барьер — СЕМАНТИКА, x86_64-фильтр — только ускорение.
pub const LENS_MIN_W: f32 = 0.05;

/// Скалярный эталон LENS-барьера (перенесён из lens_asm; тот реэкспортирует).
pub fn lens_filter_scalar(
    w: &[f32],
    flags: &[u64],
    out: &mut [u32],
    min_w: f32,
    require: u64,
    forbid: u64,
) -> usize {
    let mut count = 0;
    for i in 0..w.len().min(flags.len()).min(out.len()) {
        let ok = w[i] > min_w
            && (flags[i] & require) == require
            && (flags[i] & forbid) == 0;
        if ok {
            out[count] = i as u32;
            count += 1;
        }
    }
    count
}

/// No-Hits барьер: компакция индексов выживших рёбер (возвращает число
/// выживших; `out` вмещает `w.len()` индексов). x86_64 — SSE2-микроядро
/// `poler_lens_filter`, прочие архитектуры — скалярный эталон.
pub fn lens_filter(
    w: &[f32],
    flags: &[u64],
    out: &mut [u32],
    min_w: f32,
    require: u64,
    forbid: u64,
) -> usize {
    #[cfg(target_arch = "x86_64")]
    {
        crate::asm::lens_asm::filter(w, flags, out, min_w, require, forbid)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        lens_filter_scalar(w, flags, out, min_w, require, forbid)
    }
}

// ═══════════════════ FEP-энергия (из asm/fep_asm.rs) ═══════════════════

/// Скалярный эталон FEP-энергии (перенесён из fep_asm; тот реэкспортирует).
pub fn fep_energy_scalar(p: &[f32], o: &[f32], gw: Option<&[f32]>, lambda: f32) -> f32 {
    let mut f = 0.0f32;
    for i in 0..p.len().min(o.len()) {
        let r = p[i] - o[i];
        let w = gw.map_or(1.0f32, |g| g.get(i).copied().unwrap_or(1.0));
        f += w * r * r + 0.5 * lambda * p[i] * p[i];
    }
    f
}

/// Скалярный эталон FEP-градиента.
pub fn fep_grad_scalar(
    p: &[f32],
    o: &[f32],
    gw: Option<&[f32]>,
    lambda: f32,
    grad_out: &mut [f32],
) {
    for i in 0..p.len().min(o.len()).min(grad_out.len()) {
        let r = p[i] - o[i];
        let w = gw.map_or(1.0f32, |g| g.get(i).copied().unwrap_or(1.0));
        grad_out[i] = 2.0 * w * r + lambda * p[i];
    }
}

/// Скалярный эталон полного шага (перенесён из fep_asm; тот реэкспортирует).
pub fn fep_step_scalar(
    p: &mut [f32],
    o: &[f32],
    gw: &[f32],
    grad_out: &mut [f32],
    lambda: f32,
    eta: f32,
) -> f32 {
    let mut f = 0.0f32;
    for i in 0..p.len().min(o.len()).min(gw.len()).min(grad_out.len()) {
        let r = p[i] - o[i];
        f += gw[i] * r * r + 0.5 * lambda * p[i] * p[i];
        grad_out[i] = 2.0 * gw[i] * r + lambda * p[i];
        p[i] -= eta * grad_out[i];
    }
    f
}

/// FEP-энергия ‖p−o‖² (+λ-регуляризация): x86_64 — AVX2/FMA-микроядро
/// `poler_fep_energy` (с рантайм-фолбэком внутри), прочие — скалярный эталон.
pub fn fep_energy(p: &[f32], o: &[f32], gw: Option<&[f32]>, lambda: f32) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        crate::asm::fep_asm::energy(p, o, gw, lambda)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        fep_energy_scalar(p, o, gw, lambda)
    }
}

/// FEP-градиент: x86_64 — микроядро, прочие — скалярный эталон.
pub fn fep_grad(
    p: &[f32],
    o: &[f32],
    gw: Option<&[f32]>,
    lambda: f32,
    grad_out: &mut [f32],
) {
    #[cfg(target_arch = "x86_64")]
    {
        crate::asm::fep_asm::grad(p, o, gw, lambda, grad_out)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        fep_grad_scalar(p, o, gw, lambda, grad_out)
    }
}

// ═══════════════════ Паритет asm/скаляр (Фаза 5 протокола) ═════════════

#[cfg(all(test, target_arch = "x86_64"))]
mod facade_parity_tests {
    use super::*;

    #[test]
    fn lens_filter_facade_matches_scalar() {
        let w = vec![0.0f32, 0.03, 0.05, 0.2, 0.9, -1.0, 0.5];
        let flags = vec![0u64, 0b0001, 0b0011, 0b0110, 0b1111, 0, 0b1000];
        let mut a = vec![0u32; w.len()];
        let mut b = vec![0u32; w.len()];
        let ka = lens_filter(&w, &flags, &mut a, LENS_MIN_W, 0b0010, 0b1000);
        let kb = lens_filter_scalar(&w, &flags, &mut b, LENS_MIN_W, 0b0010, 0b1000);
        assert_eq!(ka, kb, "число выживших");
        assert_eq!(&a[..ka], &b[..kb], "индексы выживших");
    }

    #[test]
    fn fep_energy_facade_matches_scalar() {
        let p = [0.5f32, -1.25, 2.0, 0.0, 3.75];
        let o = [0.25f32, 0.5, -1.0, 0.0, 1.5];
        let gw = [1.0f32, 2.0, 0.5, 1.0, 0.25];
        let a = fep_energy(&p, &o, Some(&gw), 0.1);
        let b = fep_energy_scalar(&p, &o, Some(&gw), 0.1);
        let c = fep_energy(&p, &o, None, 0.0);
        let d = fep_energy_scalar(&p, &o, None, 0.0);
        assert!((a - b).abs() <= 1e-4 * (1.0 + a.abs()), "FEP(gw,λ) {a} vs {b}");
        assert!((c - d).abs() <= 1e-4 * (1.0 + c.abs()), "FEP(plain) {c} vs {d}");
    }

    #[test]
    fn fep_grad_facade_matches_scalar() {
        let p = [0.5f32, -1.25, 2.0, 0.0, 3.75];
        let o = [0.25f32, 0.5, -1.0, 0.0, 1.5];
        let mut a = vec![0.0f32; 5];
        let mut b = vec![0.0f32; 5];
        fep_grad(&p, &o, None, 0.1, &mut a);
        fep_grad_scalar(&p, &o, None, 0.1, &mut b);
        for (x, y) in a.iter().zip(&b) {
            assert!((x - y).abs() <= 1e-5 * (1.0 + x.abs()), "{x} vs {y}");
        }
    }

    #[test]
    fn proj_ops_facade_match_scalar() {
        // classify/inv/mul/div: asm-путь против скалярных зеркал
        let pts = [
            Proj::ZERO, Proj::ONE, Proj::INF, Proj::GAUGE,
            Proj { n: 5, d: 2 }, Proj { n: -9, d: 4 }, Proj { n: 0, d: -3 },
            Proj { n: -7, d: 0 }, Proj { n: 12, d: 8 }, Proj { n: 3, d: 1 },
        ];
        for p in pts {
            assert_eq!(p.classify(), classify_scalar(p.n, p.d));
            assert_eq!(p.inv(), Proj { n: p.d, d: p.n });
            let m = p.mul(&Proj { n: 3, d: 4 });
            assert_eq!(m, proj_mul_scalar(&p, &Proj { n: 3, d: 4 }));
            let dv = p.div(&Proj { n: 3, d: 4 });
            assert_eq!(dv, proj_div_scalar(&p, &Proj { n: 3, d: 4 }));
        }
        // cmp_canonical: asm против i128-зеркала
        for a in pts {
            for b in pts {
                if a.d > 0 && b.d > 0 {
                    assert_eq!(cmp_canonical(&a, &b), cmp_canonical_scalar(&a, &b));
                }
            }
        }
    }
}
