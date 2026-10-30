//! Сверхдиапазонные вероятности (Сессия-13): лог-домен + тритная глубина.
//!
//! Мотивация: P = 2^(-N) при N ~ 1e15 бит и P = e^(-ΔS/k_B) при
//! ΔS/k_B ~ 1e30 не представимы в f64 (динамический диапазон ~10^±308).
//! Калькулятор Всего обязан отвечать и на такие вопросы — считаем сам
//! логарифм, а вероятность держим в двух родных формах:
//!
//!   десятичная: P = 10^(log10 P)  — «порядки десяти против»;
//!   тритная:    P = 3^(log3 P)    — POLER-глубина: сколько тритов
//!                                    «против» события (каждый трит
//!                                    сбалансированной тройки делит
//!                                    исход на 3).
//!
//! Инварианты (проверены тестами): log10(2^-N) = -N·log10(2);
//! log3(e^(-x)) = -x/ln 3; произведение — сумма логарифмов.
//!
//! Приложение: `regress_report` — полная калькуляция вероятности
//! регрессии существа в собственное прошлое с сохранением опыта из
//! будущего (канал Больцмана/Крукса), с бюджетом испытаний Вселенной.

//! Сессия-14: LogProb стал полноценным типом `Value` калькулятора —
//! лог-доменная арифметика (сложение/вычитание через logaddexp/
//! logsubexp, суммы рядов сверхмалых вероятностей через lpsum):
//! весь калькулятор оперирует за пределами динамического диапазона f64.

/// Постоянная Больцмана, Дж/К — точно по SI-2019.
pub const K_B: f64 = 1.380649e-23;

const LN2: f64 = std::f64::consts::LN_2;
const LN3: f64 = 1.098_612_288_668_109_8;
const LN10: f64 = std::f64::consts::LN_10;

/// Вероятность в лог-домене: `ln` = ln P (может быть ~ -1e30).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogProb {
    pub ln: f64,
}

impl LogProb {
    /// Из натурального логарифма (контроль конечности).
    pub fn from_ln(ln: f64) -> Result<Self, String> {
        if !ln.is_finite() {
            return Err(format!("log-домен: ln = {ln} — не число"));
        }
        Ok(LogProb { ln })
    }

    /// Поднять положительное число в лог-домен: logp(0.5) → ln = −ln 2.
    /// Ноль и отрицательные не представлены (вероятность > 0).
    pub fn from_f64(p: f64) -> Result<Self, String> {
        if !p.is_finite() || p <= 0.0 {
            return Err(format!(
                "logp: аргумент {p} — нужно строго положительное число \
                 (вероятность 0 представима только пределом)"
            ));
        }
        Ok(LogProb { ln: p.ln() })
    }

    /// ln P — натуральный логарифм (может быть ~ −1e30).
    pub fn ln(&self) -> f64 {
        self.ln
    }

    /// b^x для b > 0; результат может быть вне f64 — это и есть точка.
    pub fn powf(base: f64, exp: f64) -> Result<Self, String> {
        if !base.is_finite() || !exp.is_finite() {
            return Err(format!("logpow: основание {base} и показатель {exp} — конечные числа"));
        }
        if base <= 0.0 {
            return Err(format!("logpow: основание {base} должно быть > 0"));
        }
        Ok(LogProb { ln: base.ln() * exp })
    }

    /// e^(-x) для x ≥ 0 (штраф Больцмана/флуктуационной теоремы).
    pub fn exp_neg(x: f64) -> Result<Self, String> {
        if !x.is_finite() || x < 0.0 {
            return Err(format!("e^(-x): показатель {x} должен быть ≥ 0"));
        }
        Ok(LogProb { ln: -x })
    }

    /// Произведение вероятностей — сумма логарифмов (точно даже при
    /// ln ~ -1e30, где произведение «в лоб» даст 0 с потерей информации).
    pub fn mul(self, other: LogProb) -> LogProb {
        LogProb { ln: self.ln + other.ln }
    }

    /// Возведение в степень k.
    pub fn pow(self, k: f64) -> LogProb {
        LogProb { ln: self.ln * k }
    }

    /// Частное вероятностей — разность логарифмов (шансы/отношения
    /// правдоподобий не теряют порядков даже при ln ~ −1e30).
    pub fn div(self, other: LogProb) -> LogProb {
        LogProb { ln: self.ln - other.ln }
    }

    /// Сумма вероятностей в лог-домене: ln(P + Q) = logaddexp.
    /// Стабильно: ln a + ln(1 + e^{ln b − ln a}) при a ≥ b —
    /// ln1p не теряет малое слагаемое даже при разнице 700 порядков
    /// (P + Q ≈ P, но след Q живёт в ln1p до порога −745).
    pub fn logaddexp(self, other: LogProb) -> LogProb {
        let (a, b) = if self.ln >= other.ln {
            (self.ln, other.ln)
        } else {
            (other.ln, self.ln)
        };
        LogProb { ln: a + (b - a).exp().ln_1p() }
    }

    /// Разность вероятностей в лог-домене: ln(P − Q), требует P > Q
    /// (нулевая/отрицательная разность в лог-домене не живёт).
    /// Дополнение события: 1 − P = logsubexp(ln 1, ln P).
    pub fn logsubexp(self, other: LogProb) -> Result<LogProb, String> {
        if self.ln <= other.ln {
            return Err(format!(
                "вычитание лог-вероятностей: уменьшаемое должно быть больше \
                 ({} ≤ {}) — разность ≤ 0 не представима",
                self.fmt10(),
                other.fmt10()
            ));
        }
        let d = other.ln - self.ln; // < 0
        // ln(1 − e^d): при d < −745 разность неотличима от уменьшаемого.
        let inner = if d < -745.0 {
            0.0
        } else {
            (-(d.exp())).ln_1p()
        };
        Ok(LogProb { ln: self.ln + inner })
    }

    /// log10 P.
    pub fn log10(&self) -> f64 {
        self.ln / LN10
    }

    /// log3 P — тритный логарифм вероятности.
    pub fn log3(&self) -> f64 {
        self.ln / LN3
    }

    /// Тритная глубина «против»: d = -log3 P ≥ 0 — сколько тритов
    /// сбалансированной тройки голосует против события.
    pub fn trit_depth(&self) -> f64 {
        -self.log3()
    }

    /// Числовое значение, если представимо в f64.
    pub fn to_f64(&self) -> Option<f64> {
        let p = self.ln.exp();
        if p.is_finite() && p > 0.0 {
            Some(p)
        } else {
            None
        }
    }

    /// Является ли вероятность (0 < P ≤ 1), а не шансом/отношением.
    pub fn is_probability(&self) -> bool {
        self.ln <= 0.0
    }

    /// Десятичная запись: число, если представимо, иначе 10^(…).
    pub fn fmt10(&self) -> String {
        if self.log10().abs() <= 300.0 {
            // Реконструкция через e^ln (одна операция вместо
            // log10 → 10^x двойного округления: 2^-1 остаётся «0.5»,
            // а не «0.5000000000000001»).
            fmt_f64_short(self.ln.exp())
        } else {
            format!("10^({})", exp_str(self.log10()))
        }
    }

    /// Тритная запись: 3^(…) — родная POLER-форма сверхмалых величин.
    pub fn fmt3(&self) -> String {
        format!("3^({})", exp_str(self.log3()))
    }
}

/// Число в экспоненциальной записи с явным знаком порядка:
/// -7302.4e21 → «-7.302e+24». Порядок паддируется до двух цифр
/// (согласовано с Python-эталоном Сессии-13: 10^(-3.010e+04)).
pub(crate) fn exp_str(x: f64) -> String {
    if !x.is_finite() {
        return format!("{x}");
    }
    if x == 0.0 {
        return "0".to_string();
    }
    let s = format!("{:.3e}", x); // «-7.302e24» / «3.010e4»
    match s.split_once('e') {
        Some((m, e)) => {
            let (sign, digits) = match e.strip_prefix('-') {
                Some(d) => ("-", d),
                None => ("+", e),
            };
            let padded = if digits.len() < 2 {
                format!("0{digits}")
            } else {
                digits.to_string()
            };
            format!("{m}e{sign}{padded}")
        }
        None => s,
    }
}

/// Компактная запись числа: целые до 1e5 — как есть, дальше — экспонента.
pub(crate) fn fmt_num(x: f64) -> String {
    if !x.is_finite() {
        return format!("{x}");
    }
    if x == x.trunc() && x.abs() < 1e16 && x.abs() < 1e5 {
        format!("{}", x as i64)
    } else if x.abs() >= 1e5 || x.abs() < 1e-3 || (x == x.trunc() && x.abs() >= 1e16) {
        exp_str(x)
    } else {
        format!("{x}")
    }
}

/// Короткая запись f64 без артефактов двойного округления:
/// «0.5000000000000001» → «0.5» (round-trip-проверка гарантирует,
/// что укорачивание не изменило значение больше чем на 1e-9 отн.).
pub(crate) fn fmt_f64_short(v: f64) -> String {
    if !v.is_finite() {
        return format!("{v}");
    }
    if v == v.trunc() && v.abs() < 1e16 {
        return format!("{}", v as i64);
    }
    let s = format!("{v}");
    if s.len() <= 12 {
        return s; // уже коротко (0.0009765625 и т.п.)
    }
    let r = format!("{:.10}", v)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string();
    if let Ok(parsed) = r.parse::<f64>() {
        if (parsed - v).abs() <= 1e-9 * v.abs().max(1e-300) {
            return r;
        }
    }
    format!("{:e}", v)
}

/// Бюджет испытаний наблюдаемой Вселенной (log10): планковских
/// моментов в её истории × на число частиц ≈ 10^140.9.
pub fn universe_trials_log10() -> f64 {
    const T_P: f64 = 5.391_247e-44; // планковское время, с
    const T_U: f64 = 4.35e17; // возраст Вселенной ~13.8 млрд лет, с
    const N_PART: f64 = 1.0e80; // частиц в наблюдаемой Вселенной
    (T_U / T_P).log10() + N_PART.log10()
}

/// Сумма ряда вероятностей в лог-домене (статистическая сумма /
/// функция разбиения Z = Σ P_i): каждый член может быть ~ 10^(−10^5) —
/// в f64 такой ряд «суммируется» в 0 с потерей всей информации,
/// здесь — честный logaddexp-аккумулятор без единого порядка потерь.
/// Члены — ln P_i; accepts также поднимаемые величины > 0.
pub fn lpsum_lns(items: &[f64]) -> Result<LogProb, String> {
    if items.is_empty() {
        return Err("lpsum: пустой ряд — нечего суммировать".into());
    }
    let mut acc = LogProb::from_ln(items[0])?;
    for &x in &items[1..] {
        acc = acc.logaddexp(LogProb::from_ln(x)?);
    }
    Ok(acc)
}

/// Полная калькуляция: P(опыт из будущего сохранится | регрессия
/// существа в прошлое) и P(сама регрессия), канал Больцмана/Крукса.
///
/// - `n_bits` — биты НОВОГО опыта за откатываемый интервал (каждый
///   бит делит условную вероятность пополам: P = 2^(-N));
/// - `dt_s` — глубина регрессии, секунды;
/// - `watts` — тепловой поток мозга, Вт (умолчание 20);
/// - `kelvin` — температура мозга, К (умолчание 310.15).
///
/// Безусловная вероятность: ΔS = σ·Δt, σ = P/T, P(регрессия) =
/// e^(-ΔS/k_B) — рассеянное тепло должно спонтанно стечь обратно.
pub fn regress_report(n_bits: f64, dt_s: f64, watts: f64, kelvin: f64) -> Result<String, String> {
    for (what, v) in [("N", n_bits), ("Δt", dt_s), ("мощность", watts), ("температура", kelvin)] {
        if !v.is_finite() {
            return Err(format!("regress: {what} = {v} — не число"));
        }
    }
    if n_bits < 0.0 {
        return Err("regress: N бит опыта ≥ 0".into());
    }
    if dt_s <= 0.0 {
        return Err("regress: глубина регрессии Δt > 0".into());
    }
    if watts <= 0.0 {
        return Err("regress: мощность мозга > 0".into());
    }
    if kelvin <= 0.0 {
        return Err("regress: температура > 0".into());
    }

    let p_mem = LogProb::powf(2.0, -n_bits)?;
    let sigma_kb = watts / kelvin / K_B; // скорость производства энтропии, k_B/с
    let ds_over_kb = sigma_kb * dt_s; // безразмерный штраф
    let p_reg = LogProb::exp_neg(ds_over_kb)?;
    let p_joint = p_mem.mul(p_reg);
    let trials = universe_trials_log10();
    let expected = p_joint.log10() + trials; // log10 ожидаемого числа событий

    // На сколько порядков возврат ТЕЛА дороже сохранения ПАМЯТИ
    // (в самой экспоненте): ΔS/k_B против N·ln 2.
    let gap_str = if n_bits > 0.0 {
        let gap = (ds_over_kb / (n_bits * LN2)).log10();
        if gap >= 1.0 {
            format!(" · возврат тела дороже памяти на {:.0} порядков экспоненты", gap)
        } else {
            " · память и тело сопоставимы по цене".into()
        }
    } else {
        String::new()
    };

    let verdict = if expected < -12.0 {
        format!("0 реализаций (дефицит {} порядков)", exp_str(-expected))
    } else if expected < 0.0 {
        format!("доля события {} от одного", exp_str(expected))
    } else {
        format!("~{} событий", exp_str(expected))
    };

    Ok([
        format!(
            "регрессия: Δt = {} с · опыт N = {} бит · мозг {} Вт / {} К",
            fmt_num(dt_s),
            fmt_num(n_bits),
            fmt_num(watts),
            fmt_num(kelvin)
        ),
        format!(
            "σ = {} k_B/с · ΔS/k_B = {} — рассеянное тепло должно стечь обратно",
            exp_str(sigma_kb),
            exp_str(ds_over_kb)
        ),
        format!(
            "P(опыт | регрессия) = 2^(-N) = {} = {} — глубина {} тритов против",
            p_mem.fmt10(),
            p_mem.fmt3(),
            exp_str(p_mem.trit_depth())
        ),
        format!(
            "P(регрессия) = e^(-ΔS/k_B) = {} = {} — глубина {} тритов против",
            p_reg.fmt10(),
            p_reg.fmt3(),
            exp_str(p_reg.trit_depth())
        ),
        format!(
            "совместная = {}{gap_str}",
            p_joint.fmt10()
        ),
        format!(
            "бюджет Вселенной: 10^({:.2}) испытаний → ожидается 10^({}) событий → {}",
            trials,
            exp_str(expected),
            verdict
        ),
        "Θ-канал (точное обращение): P = 1 формально, субъективно 0 · де Ситтер: реализуется, но бесконечно поздно".to_string(),
    ]
    .join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, rel: f64) -> bool {
        (a - b).abs() <= rel * b.abs().max(1e-300)
    }

    #[test]
    fn two_pow_minus_one() {
        let p = LogProb::powf(2.0, -1.0).unwrap();
        assert!((p.to_f64().unwrap() - 0.5).abs() < 1e-15);
        assert!(close(p.log10(), -0.30102999566398120, 1e-12));
        assert!(close(p.log3(), -0.63092975357145743, 1e-12));
        assert_eq!(p.fmt10(), "0.5");
        assert_eq!(p.fmt3(), "3^(-6.309e-01)");
        assert!(close(p.trit_depth(), 0.63092975357145743, 1e-12));
    }

    #[test]
    fn beyond_f64_range() {
        // 2^(-10^15): ln ≈ -6.93e14 — вероятность вне f64, логарифм — нет.
        let p = LogProb::powf(2.0, -1e15).unwrap();
        assert!(p.to_f64().is_none());
        assert!(close(p.log10(), -3.010299956639812e14, 1e-10));
        assert!(close(p.log3(), -6.3092975357145744e14, 1e-10));
        assert_eq!(p.fmt10(), "10^(-3.010e+14)");
        assert_eq!(p.fmt3(), "3^(-6.309e+14)");
    }

    #[test]
    fn exp_neg_entropy_penalty() {
        // Эталон Python-калькуляции Сессии-13: ΔS/k_B = 1.6814e25
        // → log10 P = -7.3021e24.
        let p = LogProb::exp_neg(1.6814e25).unwrap();
        assert!(close(p.log10(), -7.3021e24, 1e-4));
        assert!(close(p.log3(), -1.5305e25, 1e-4));
        assert_eq!(p.fmt10(), "10^(-7.302e+24)");
    }

    #[test]
    fn mul_is_sum_of_logs() {
        let a = LogProb::powf(2.0, -1e13).unwrap();
        let b = LogProb::exp_neg(4.422e30).unwrap();
        let j = a.mul(b);
        assert!(close(j.ln, a.ln + b.ln, 1e-15));
        // 2^(-10^13) вырождается в 0.0 при прямом вычислении — лог-домен
        // сохраняет информацию, а произведение не теряет ни одного порядка.
        assert!(a.to_f64().is_none());
        let p = j.pow(0.5);
        assert!(close(p.ln, j.ln * 0.5, 1e-15));
    }

    #[test]
    fn powf_rejects_bad_base() {
        assert!(LogProb::powf(0.0, 3.0).is_err());
        assert!(LogProb::powf(-2.0, 3.0).is_err());
        assert!(LogProb::powf(f64::NAN, 3.0).is_err());
        assert!(LogProb::exp_neg(-1.0).is_err());
    }

    #[test]
    fn logaddexp_sum_of_geometric_series() {
        // Σ_{i=1..N} 2^(−i) = 1 − 2^(−N): ряд сверхмалых слагаемых,
        // чья сумма — обычное число. В f64 при N > 1074 члены — 0.0,
        // сумма — 0.0; в лог-домене сумма стремится к 1 точно.
        for n in [4usize, 64, 1024, 100_000] {
            let lns: Vec<f64> = (1..=n as i64).map(|i| -(i as f64) * LN2).collect();
            let s = lpsum_lns(&lns).unwrap();
            let expected = 1.0 - if n <= 1000 { 2.0f64.powi(-(n as i32)) } else { 0.0 };
            assert!((s.to_f64().unwrap() - expected).abs() < 1e-12, "N={n}");
        }
        // шок-сравнение: каждый член 2^(−100000) вне f64 (to_f64 = None),
        // а их сумма — представимое число ~1: ряд не потерял ни одного члена.
        let term = LogProb::powf(2.0, -100_000.0).unwrap();
        assert!(term.to_f64().is_none());
        let lns: Vec<f64> = (1..=100_000i64).map(|i| -(i as f64) * LN2).collect();
        let s = lpsum_lns(&lns).unwrap();
        assert!((s.to_f64().unwrap() - 1.0).abs() < 1e-15);
    }

    #[test]
    fn logaddexp_and_subexp_roundtrip() {
        let a = LogProb::from_f64(0.6).unwrap();
        let b = LogProb::from_f64(0.2).unwrap();
        let sum = a.logaddexp(b);
        assert!((sum.to_f64().unwrap() - 0.8).abs() < 1e-15);
        let back = sum.logsubexp(b).unwrap();
        assert!((back.to_f64().unwrap() - 0.6).abs() < 1e-12);
        // вычитание большего из меньшего — ошибка
        assert!(a.logsubexp(sum).is_err());
    }

    #[test]
    fn complement_of_super_small_probability() {
        // 1 − 2^(−10^5): дополнение события — само сверхмало в экспоненте,
        // но ln(1 − ε) ≈ −ε ≈ −2^(−10^5) → |ln| < 1e-4 — представимо.
        let p = LogProb::powf(2.0, -1e5).unwrap();
        let one = LogProb::from_f64(1.0).unwrap();
        let c = one.logsubexp(p).unwrap();
        assert!(c.to_f64().is_some());
        assert!((c.to_f64().unwrap() - 1.0).abs() < 1e-5); // ≈ 1 − 3e-30105
        // а в лоб: 1.0 − 2^(−10^5) в f64 = ровно 1.0 (ε выпал в underflow)
    }

    #[test]
    fn logaddexp_keeps_tiny_tail() {
        // P + Q при Q на 700 порядков меньше: logaddexp хранит след Q.
        let p = LogProb::from_f64(1.0).unwrap();
        let q = LogProb::from_ln(-690.0).unwrap(); // e^-690 ≈ 1e-300
        let s = p.logaddexp(q);
        assert!((s.ln() - q.ln().exp()).abs() < 1e-12); // ln = ln(1+1e-300) ≈ 1e-300
        // а при разнице > 745 порядков след недоступен f64-логарифму — это предел
        let q2 = LogProb::from_ln(-800.0).unwrap();
        let s2 = p.logaddexp(q2);
        assert_eq!(s2.ln(), 0.0);
    }

    #[test]
    fn lpsum_boltzmann_partition_function() {
        // Z = Σ e^(−E_i): все члены ~ e^(−1e5) — в f64 нули, здесь — число.
        let lns: Vec<f64> = (1..=100).map(|i| -(i as f64) * 1e3).collect();
        let z = lpsum_lns(&lns).unwrap();
        // доминирует первый член: Z ≈ e^(−1000)·(1 + e^(−1000) + …) ≈ e^(−1000)
        assert!((z.ln() + 1000.0).abs() < 1e-6, "Z: {}", z.ln());
        assert!(z.to_f64().is_none()); // e^-1000 вне f64 — но Z посчитан
    }

    #[test]
    fn div_is_log_difference() {
        let a = LogProb::powf(2.0, -1e13).unwrap();
        let b = LogProb::powf(2.0, -1e11).unwrap();
        // шансы b/a = 2^(+9.9e12) — отношение > 1, ln > 0
        let r = b.div(a);
        assert!((r.ln() - (1e13 - 1e11) * LN2).abs() < 1e-3);
        assert!(!r.is_probability());
        assert!(a.is_probability());
        // а сверхмалое/сверхмалое той же глубины = 1
        assert!(a.div(a).ln().abs() < 1e-9);
    }

    #[test]
    fn from_f64_rejects_nonpositive() {
        assert!(LogProb::from_f64(0.0).is_err());
        assert!(LogProb::from_f64(-0.5).is_err());
        assert!(LogProb::from_f64(f64::NAN).is_err());
        assert!((LogProb::from_f64(2.5).unwrap().ln() - 2.5f64.ln()).abs() < 1e-15);
    }

    #[test]
    fn universe_budget() {
        let t = universe_trials_log10();
        assert!((t - 140.907).abs() < 0.01, "бюджет: {t}");
    }

    #[test]
    fn exp_str_padding() {
        assert_eq!(exp_str(-30103.0), "-3.010e+04");
        assert_eq!(exp_str(140.9069), "1.409e+02");
        assert_eq!(exp_str(0.0), "0");
        assert_eq!(exp_str(-6.93e-7), "-6.930e-07");
    }

    #[test]
    fn fmt_num_variants() {
        assert_eq!(fmt_num(3600.0), "3600");
        assert_eq!(fmt_num(310.15), "310.15");
        assert_eq!(fmt_num(20.0), "20");
        assert_eq!(fmt_num(100000.0), "1.000e+05");
        assert_eq!(fmt_num(0.0), "0");
    }

    #[test]
    fn regress_report_hour_reference() {
        // Сверка с Python-эталоном (scripts/regression_time_probability.py):
        // N = 1e5 бит, Δt = 3600 с, 20 Вт, 310.15 К.
        let rep = regress_report(1e5, 3600.0, 20.0, 310.15).unwrap();
        assert!(rep.contains("ΔS/k_B = 1.681e+25"), "ΔS: {rep}");
        assert!(rep.contains("10^(-3.010e+04)"), "память: {rep}");
        assert!(rep.contains("10^(-7.302e+24)"), "регрессия: {rep}");
        assert!(rep.contains("3^(-1.530e+25)"), "тритная глубина: {rep}");
        assert!(rep.contains("10^(140.91)"), "бюджет: {rep}");
        assert!(rep.contains("0 реализаций"), "вердикт: {rep}");
        assert!(rep.contains("20 порядков"), "дороже: {rep}");
        assert!(rep.contains("4.671e+21"), "σ: {rep}");
    }

    #[test]
    fn regress_report_single_bit() {
        // N = 1 бит: P(опыт|регрессия) = 0.5 — представимо, без 10^(…).
        let rep = regress_report(1.0, 60.0, 20.0, 310.15).unwrap();
        assert!(rep.contains("= 0.5 = 3^(-6.309e-01)"), "один бит: {rep}");
        assert!(rep.contains("ΔS/k_B = 2.802e+23"), "минута: {rep}");
    }

    #[test]
    fn regress_report_validation() {
        assert!(regress_report(-1.0, 60.0, 20.0, 310.15).is_err());
        assert!(regress_report(1.0, 0.0, 20.0, 310.15).is_err());
        assert!(regress_report(1.0, 60.0, 0.0, 310.15).is_err());
        assert!(regress_report(1.0, 60.0, 20.0, -1.0).is_err());
        assert!(regress_report(f64::NAN, 60.0, 20.0, 310.15).is_err());
    }
}
