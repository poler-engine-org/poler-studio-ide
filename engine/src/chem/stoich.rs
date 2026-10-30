//! Стехиометрия: балансировка уравнений реакций (рациональный Гаусс,
//! бит-в-бит зеркально Python-генератору задачника) и расчёты
//! «масса ↔ количество ↔ объём ↔ число частиц» по уравнениям.
//!
//! Контур D (v0.79.0). Школьные константы: N_A = 6.02·10²³, V_m = 22.4
//! л/моль (н. у.) — канон учебника; ответы задачника в допуске 0.8%.

use super::formula::{parse_formula, Formula};

/// Число Авогадро, моль⁻¹ (школьный канон).
pub const NA: f64 = 6.02e23;
/// Молярный объём газа при н. у., л/моль (школьный канон).
pub const VM: f64 = 22.4;

// ─── Рациональная арифметика для балансировщика ───────────────────────────

/// Рациональное число num/den, den > 0, НОД(num, den) = 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rat {
    pub num: i64,
    pub den: i64,
}

impl Rat {
    pub fn new(mut num: i64, mut den: i64) -> Rat {
        assert!(den != 0, "Rat: нулевой знаменатель");
        if den < 0 {
            num = -num;
            den = -den;
        }
        let g = gcd(num.unsigned_abs() as u64, den as u64) as i64;
        if g > 1 {
            num /= g;
            den /= g;
        }
        Rat { num, den }
    }

    pub fn from_i(v: i64) -> Rat {
        Rat { num: v, den: 1 }
    }

    pub fn zero() -> Rat {
        Rat { num: 0, den: 1 }
    }

    pub fn is_zero(&self) -> bool {
        self.num == 0
    }

    pub fn add(self, o: Rat) -> Rat {
        Rat::new(self.num * o.den + o.num * self.den, self.den * o.den)
    }

    pub fn sub(self, o: Rat) -> Rat {
        Rat::new(self.num * o.den - o.num * self.den, self.den * o.den)
    }

    pub fn mul(self, o: Rat) -> Rat {
        Rat::new(self.num * o.num, self.den * o.den)
    }

    pub fn div(self, o: Rat) -> Rat {
        assert!(!o.is_zero(), "Rat: деление на ноль");
        Rat::new(self.num * o.den, self.den * o.num)
    }

    pub fn neg(self) -> Rat {
        Rat { num: -self.num, den: self.den }
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

// ─── Уравнение реакции ────────────────────────────────────────────────────

/// Уравнение: вещества в порядке следования + коэффициенты.
#[derive(Debug, Clone)]
pub struct Equation {
    /// Вещества (реагенты первыми).
    pub species: Vec<Formula>,
    /// Число реагентов (левая часть).
    pub n_left: usize,
    /// Сбалансированные минимальные целые коэффициенты.
    pub coefs: Vec<i64>,
}

impl Equation {
    /// Молярная масса вещества по индексу.
    pub fn molar_mass(&self, i: usize) -> f64 {
        self.species[i].molar_mass()
    }

    /// Индекс вещества по тексту формулы («Fe2O3»).
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.species.iter().position(|f| f.text == name)
    }

    /// Строка с коэффициентами: «2 6 2 3».
    pub fn coefs_str(&self) -> String {
        self.coefs.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ")
    }

    /// Канонический вывод: «2Al + 6HCl -> 2AlCl3 + 3H2».
    pub fn to_string_pretty(&self) -> String {
        let mut parts: Vec<String> = Vec::with_capacity(self.species.len());
        for (i, f) in self.species.iter().enumerate() {
            let c = self.coefs[i];
            parts.push(if c == 1 {
                f.text.clone()
            } else {
                format!("{c}{}", f.text)
            });
        }
        let left = parts[..self.n_left].join(" + ");
        let right = parts[self.n_left..].join(" + ");
        format!("{left} -> {right}")
    }

    /// Проверка закона сохранения массы по всем элементам.
    pub fn mass_balanced(&self) -> bool {
        let mut els: Vec<u8> = Vec::new();
        for f in &self.species {
            for &(z, _) in &f.atoms {
                if !els.contains(&z) {
                    els.push(z);
                }
            }
        }
        els.iter().all(|&z| {
            let left: i64 = self.species[..self.n_left]
                .iter()
                .zip(&self.coefs[..self.n_left])
                .map(|(f, c)| c * f.count_of(z) as i64)
                .sum();
            let right: i64 = self.species[self.n_left..]
                .iter()
                .zip(&self.coefs[self.n_left..])
                .map(|(f, c)| c * f.count_of(z) as i64)
                .sum();
            left == right
        })
    }

    /// Моль вещества `to`, получаемые из `qty` (г — если `is_mass`,
    /// иначе моль) вещества `from`.
    pub fn convert(&self, i_from: usize, qty: f64, is_mass: bool, i_to: usize) -> f64 {
        let n_from = if is_mass {
            qty / self.molar_mass(i_from)
        } else {
            qty
        };
        n_from * self.coefs[i_to] as f64 / self.coefs[i_from] as f64
    }
}

/// Разбор уравнения «Fe2O3 + 3CO -> 2Fe + 3CO2» (коэффициенты
/// допускаются и отбрасываются — баланс пересчитывается). Принимаются
/// разделители «->», «=», «=>», «→».
pub fn parse_equation(s: &str) -> Result<Equation, String> {
    let norm = s.trim();
    let (lhs, rhs) = split_arrow(norm)
        .ok_or_else(|| format!("«{norm}»: нет разделителя реагенты/продукты (->)"))?;
    let left = split_terms(lhs);
    let right = split_terms(rhs);
    if left.is_empty() || right.is_empty() {
        return Err(format!("«{norm}»: пустая сторона уравнения"));
    }
    let n_left = left.len();
    let mut species = Vec::with_capacity(left.len() + right.len());
    for t in left.into_iter().chain(right) {
        // отбрасываем ведущие цифры-коэффициенты
        let bare = strip_leading_digits(t);
        let f = parse_formula(bare)
            .map_err(|e| format!("«{norm}»: {e}"))?;
        if species.iter().any(|prev: &Formula| prev.text == f.text) {
            return Err(format!("«{norm}»: вещество {bare} повторяется"));
        }
        species.push(f);
    }
    let coefs = balance(&species, n_left)
        .map_err(|e| format!("«{norm}»: {e}"))?;
    Ok(Equation { species, n_left, coefs })
}

fn split_arrow(s: &str) -> Option<(&str, &str)> {
    for sep in ["->", "=>", " → ", "→", " = ", "="] {
        if let Some(p) = s.find(sep) {
            return Some((&s[..p], &s[p + sep.len()..]));
        }
    }
    None
}

fn split_terms(side: &str) -> Vec<&str> {
    side.split('+').map(str::trim).filter(|t| !t.is_empty()).collect()
}

fn strip_leading_digits(t: &str) -> &str {
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    &t[i..]
}

/// Балансировка: минимальные целые положительные коэффициенты.
/// Матрица «элементы × вещества» (реагенты +, продукты −), нуль-
/// пространство — рациональный Гаусс, свободная переменная — последнее
/// вещество; результат масштабируется в наименьшие целые.
/// Алгоритм зеркален Python-генератору (curriculum_gen.py).
pub fn balance(species: &[Formula], n_left: usize) -> Result<Vec<i64>, String> {
    let n = species.len();
    if n < 2 || n_left == 0 || n_left >= n {
        return Err("уравнение: нужны реагенты и продукты".into());
    }
    // множество элементов (по Z, сортировка — как в Python по символу)
    let mut el_syms: Vec<&'static str> = Vec::new();
    for f in species {
        for &(z, _) in &f.atoms {
            let sym = super::periodic::by_z(z).unwrap().symbol;
            if !el_syms.contains(&sym) {
                el_syms.push(sym);
            }
        }
    }
    // Python сортирует по строке символа — повторим для бит-в-бит
    el_syms.sort_unstable();
    let rows = el_syms.len();

    // A[элемент][вещество]: +count для реагентов, −count для продуктов
    let mut a: Vec<Vec<Rat>> = el_syms
        .iter()
        .map(|sym| {
            let z = super::periodic::by_symbol(sym).unwrap().z;
            species
                .iter()
                .enumerate()
                .map(|(j, f)| {
                    let c = f.count_of(z) as i64;
                    let signed = if j < n_left { c } else { -c };
                    Rat::from_i(signed)
                })
                .collect()
        })
        .collect();

    // прямой ход Гаусса по столбцам [0, n-1)
    let mut piv: Vec<usize> = Vec::new();
    let mut r = 0usize;
    for col in 0..n - 1 {
        let lead = (r..rows).find(|&rr| !a[rr][col].is_zero());
        let Some(lead) = lead else { continue };
        a.swap(r, lead);
        let pv = a[r][col];
        for j in 0..n {
            a[r][j] = a[r][j].div(pv);
        }
        for rr in 0..rows {
            if rr != r && !a[rr][col].is_zero() {
                let f = a[rr][col];
                for j in 0..n {
                    a[rr][j] = a[rr][j].sub(f.mul(a[r][j]));
                }
            }
        }
        piv.push(col);
        r += 1;
        if r == rows {
            break;
        }
    }

    // свободная переменная x[n-1] = 1
    let mut x = vec![Rat::zero(); n];
    x[n - 1] = Rat::from_i(1);
    for (i, &col) in piv.iter().enumerate() {
        x[col] = a[i][n - 1].neg();
    }

    // проверка полного баланса (включая строки вне базиса)
    for row in 0..rows {
        let acc: Rat = (0..n).fold(Rat::zero(), |acc, j| acc.add(a[row][j].mul(x[j])));
        if !acc.is_zero() {
            return Err("уравнение не балансируется (нет решения)".into());
        }
    }
    if x.iter().any(|v| v.num <= 0) {
        return Err("отрицательный или нулевой коэффициент".into());
    }

    // НОК знаменателей → целые, затем НОД чисел → минимальные
    let mut lcm: i64 = 1;
    for v in &x {
        let d = v.den;
        lcm = lcm / gcd(lcm.unsigned_abs() as u64, d as u64) as i64 * d;
    }
    let ints: Vec<i64> = x.iter().map(|v| v.num * (lcm / v.den)).collect();
    let mut g: i64 = ints[0].abs();
    for v in &ints[1..] {
        g = gcd(g.unsigned_abs() as u64, v.unsigned_abs() as u64) as i64;
    }
    if g == 0 {
        return Err("нулевые коэффициенты".into());
    }
    Ok(ints.into_iter().map(|v| v / g).collect())
}

// ─── Базовые пересчёты (моль-центричная система) ──────────────────────────

/// n = m / M.
pub fn moles_from_mass(mass_g: f64, molar: f64) -> f64 {
    mass_g / molar
}

/// m = n · M.
pub fn mass_from_moles(moles: f64, molar: f64) -> f64 {
    moles * molar
}

/// V = n · V_m (газ, н. у.).
pub fn volume_from_moles(moles: f64) -> f64 {
    moles * VM
}

/// n = V / V_m (газ, н. у.).
pub fn moles_from_volume(volume_l: f64) -> f64 {
    volume_l / VM
}

/// N = n · N_A.
pub fn particles_from_moles(moles: f64) -> f64 {
    moles * NA
}

/// n = N / N_A.
pub fn moles_from_particles(particles: f64) -> f64 {
    particles / NA
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eq(s: &str) -> Equation {
        parse_equation(s).unwrap()
    }

    #[test]
    fn rat_arith() {
        let a = Rat::new(1, 3);
        let b = Rat::new(1, 6);
        assert_eq!(a.add(b), Rat::new(1, 2));
        assert_eq!(a.mul(b), Rat::new(1, 18));
        assert_eq!(a.div(b), Rat::new(2, 1));
        assert_eq!(Rat::new(-4, -8), Rat::new(1, 2));
        assert_eq!(Rat::new(4, -8), Rat::new(-1, 2));
    }

    #[test]
    fn balance_classics() {
        let cases = [
            ("Al + HCl -> AlCl3 + H2", vec![2, 6, 2, 3]),
            ("Al + O2 -> Al2O3", vec![4, 3, 2]),
            ("Mg + O2 -> MgO", vec![2, 1, 2]),
            ("N2 + H2 -> NH3", vec![1, 3, 2]),
            ("KMnO4 -> K2MnO4 + MnO2 + O2", vec![2, 1, 1, 1]),
            ("Fe + H2O -> Fe3O4 + H2", vec![3, 4, 1, 4]),
            ("Fe2O3 + H2 -> Fe + H2O", vec![1, 3, 2, 3]),
            ("WO3 + H2 -> W + H2O", vec![1, 3, 1, 3]),
            ("Fe2O3 + CO -> Fe + CO2", vec![1, 3, 2, 3]),
            ("Zn + HCl -> ZnCl2 + H2", vec![1, 2, 1, 1]),
            ("C3H8 + O2 -> CO2 + H2O", vec![1, 5, 3, 4]),
            ("C2H6 + O2 -> CO2 + H2O", vec![2, 7, 4, 6]),
            ("C2H5OH + O2 -> CO2 + H2O", vec![1, 3, 2, 3]),
            ("C6H12O6 + O2 -> CO2 + H2O", vec![1, 6, 6, 6]),
            ("2H2 + O2 -> 2H2O", vec![2, 1, 2]), // коэффициенты на входе отброшены
        ];
        for (s, expect) in cases {
            let e = eq(s);
            assert_eq!(e.coefs, expect, "«{s}»");
            assert!(e.mass_balanced(), "«{s}»: масса не сохраняется");
        }
    }

    #[test]
    fn balance_rejects() {
        // переформулировка с веществом только справа не балансируется
        // положительно: H2 -> H2O (кислород берётся из ниоткуда)
        assert!(parse_equation("H2 -> H2O").is_err());
        assert!(parse_equation("H2 O2").is_err(), "нет стрелки");
        assert!(parse_equation("-> H2O").is_err());
    }

    #[test]
    fn equation_convert() {
        // Fe2O3 + 3CO -> 2Fe + 3CO2: 160 г Fe2O3 → ~112 г Fe
        let e = eq("Fe2O3 + CO -> Fe + CO2");
        let i_fe2o3 = e.index_of("Fe2O3").unwrap();
        let i_fe = e.index_of("Fe").unwrap();
        let n_fe = e.convert(i_fe2o3, 160.0, true, i_fe);
        let m_fe = mass_from_moles(n_fe, e.molar_mass(i_fe));
        assert!((m_fe - 112.0).abs() < 1.0, "m(Fe) = {m_fe}");
        assert!(e.mass_balanced());
        assert_eq!(e.coefs_str(), "1 3 2 3");
        assert_eq!(e.to_string_pretty(), "Fe2O3 + 3CO -> 2Fe + 3CO2");
    }

    #[test]
    fn mole_conversions() {
        assert!((moles_from_mass(18.0, 18.015) - 0.99917).abs() < 1e-3);
        assert!((volume_from_moles(2.0) - 44.8).abs() < 1e-9);
        assert!((moles_from_volume(5.6) - 0.25).abs() < 1e-12);
        assert!((particles_from_moles(2.0) - 1.204e24).abs() < 1e20);
        assert!((moles_from_particles(6.02e23) - 1.0).abs() < 1e-14);
    }
}
