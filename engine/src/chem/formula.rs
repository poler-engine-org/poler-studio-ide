//! Парсер химических формул школьного вида: `H2SO4`, `Ca(OH)2`,
//! `Al2(SO4)3`, кристаллогидраты `CuSO4·5H2O`.
//!
//! Контур D (v0.79.0): молярная масса, массовые доли элементов —
//! первые настоящие «юнит-тесты учебника» лестницы Curriculum Learning.
//! Парсер — рекурсивный спуск по байтам, без аллокаций на горячем пути
//! (вектор состава — единственная аллокация).

use super::periodic;

/// Разобранная формула: агрегированный состав + исходный текст.
#[derive(Debug, Clone, PartialEq)]
pub struct Formula {
    /// Исходный текст формулы («CuSO4·5H2O»).
    pub text: String,
    /// Состав: (Z, число атомов) — гидратная часть просуммирована.
    pub atoms: Vec<(u8, u64)>,
    /// Гидратная часть: (коэффициент, текст формулы воды) — для
    /// массовой доли воды в кристаллогидрате.
    pub hydrate: Option<(u64, String)>,
}

impl Formula {
    /// Молярная масса, г/моль.
    pub fn molar_mass(&self) -> f64 {
        self.atoms
            .iter()
            .map(|&(z, n)| periodic::by_z(z).unwrap().mass * n as f64)
            .sum()
    }

    /// Число атомов элемента (по Z) в формульной единице.
    pub fn count_of(&self, z: u8) -> u64 {
        self.atoms.iter().find(|&&(zz, _)| zz == z).map(|&(_, n)| n).unwrap_or(0)
    }

    /// Массовая доля элемента (0..1), None если элемента нет.
    pub fn mass_fraction(&self, z: u8) -> Option<f64> {
        if self.count_of(z) == 0 {
            return None;
        }
        Some(self.count_of(z) as f64 * periodic::by_z(z).unwrap().mass / self.molar_mass())
    }

    /// Суммарное число атомов (для проверок).
    pub fn total_atoms(&self) -> u64 {
        self.atoms.iter().map(|&(_, n)| n).sum()
    }

    /// Формула Хилла («C6H12O6») — канонический вывод состава.
    pub fn hill_formula(&self) -> String {
        let mut rest: Vec<(u8, u64)> = self.atoms.clone();
        rest.sort_by_key(|&(z, _)| z);
        // Хилл: сначала C, потом H, остальные по алфавиту символов
        let mut order: Vec<(u8, u64)> = Vec::with_capacity(rest.len());
        if let Some(p) = rest.iter().position(|&(z, _)| z == 6) {
            order.push(rest.remove(p));
        }
        if let Some(p) = rest.iter().position(|&(z, _)| z == 1) {
            order.push(rest.remove(p));
        }
        // остальное — по символу лексикографически
        rest.sort_by_key(|&(z, _)| periodic::by_z(z).unwrap().symbol.to_string());
        order.extend(rest);
        let mut out = String::new();
        for (z, n) in order {
            out.push_str(periodic::by_z(z).unwrap().symbol);
            if n > 1 {
                out.push_str(&n.to_string());
            }
        }
        out
    }
}

/// Разбор формулы. Допустимы: элементы (строгий регистр), индексы,
/// круглые скобки (одна вложенность — школьный канон), «·nH2O».
pub fn parse_formula(s: &str) -> Result<Formula, String> {
    let text = s.trim().to_string();
    if text.is_empty() {
        return Err("пустая формула".into());
    }
    let mut atoms: Vec<(u8, u64)> = Vec::with_capacity(8);
    let mut hydrate = None;
    let bytes = text.as_bytes();

    // гидрат: «·5H2O» — точка-разделитель U+00B7 (2 байта) или '*'
    let hydrate_split = find_hydrate_split(&text);

    let main_end = hydrate_split.map(|(p, _)| p).unwrap_or(bytes.len());
    parse_group(&text, 0, main_end, &mut atoms)?;

    if let Some((pos, n)) = hydrate_split {
        // pos указывает на «·» (2 байта); пропускаем «·» и коэффициент
        let mut j = pos + 2;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        let mut h_atoms: Vec<(u8, u64)> = Vec::with_capacity(4);
        parse_group(&text, j, text.len(), &mut h_atoms)?;
        for (z, cnt) in h_atoms {
            add_atom(&mut atoms, z, cnt * n);
        }
        hydrate = Some((n, text[j..].to_string()));
    }

    if atoms.is_empty() {
        return Err(format!("«{text}»: пустой состав"));
    }
    atoms.sort_by_key(|&(z, _)| z);
    Ok(Formula { text, atoms, hydrate })
}

/// Найти разделитель гидрата «·». Возвращает (байтовая позиция начала
/// «·nH2O», коэффициент n).
fn find_hydrate_split(s: &str) -> Option<(usize, u64)> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == 0xC2 && i + 1 < b.len() && b[i + 1] == 0xB7 {
            // «·» U+00B7
            let mut j = i + 2;
            let start = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > start {
                let n: u64 = s[start..j].parse().unwrap_or(0);
                if n > 0 && j < b.len() && b[j].is_ascii_uppercase() {
                    return Some((i, n));
                }
            }
        }
        i += 1;
    }
    None
}

/// Разбор сегмента [start, end) в агрегирующий вектор.
fn parse_group(text: &str, start: usize, end: usize, out: &mut Vec<(u8, u64)>) -> Result<(), String> {
    let b = text.as_bytes();
    let mut i = start;
    while i < end {
        let c = b[i];
        if c == b' ' || c == b'\t' {
            i += 1;
            continue;
        }
        if !c.is_ascii_uppercase() {
            return Err(format!(
                "«{text}»: на позиции {i} ожидался элемент с заглавной буквы"
            ));
        }
        // символ: [A-Z][a-z]?
        let mut j = i + 1;
        if j < end && b[j].is_ascii_lowercase() {
            j += 1;
        }
        let sym = &text[i..j];
        let el = periodic::by_symbol(sym)
            .ok_or_else(|| format!("«{text}»: неизвестный элемент «{sym}»"))?;
        // индекс
        let mut k = j;
        while k < end && b[k].is_ascii_digit() {
            k += 1;
        }
        let n: u64 = if k > j {
            text[j..k].parse().map_err(|_| format!("«{text}»: индекс слишком велик"))?
        } else {
            1
        };
        add_atom(out, el.z, n);
        i = k;
        // скобочная группа
        if i < end && b[i] == b'(' {
            let mut depth = 1usize;
            let mut m = i + 1;
            while m < end && depth > 0 {
                if b[m] == b'(' {
                    depth += 1;
                } else if b[m] == b')' {
                    depth -= 1;
                }
                m += 1;
            }
            if depth != 0 {
                return Err(format!("«{text}»: незакрытая скобка"));
            }
            let inner_end = m - 1; // позиция ')'
            let mut inner: Vec<(u8, u64)> = Vec::with_capacity(4);
            parse_group(text, i + 1, inner_end, &mut inner)?;
            let mut q = m;
            while q < end && b[q].is_ascii_digit() {
                q += 1;
            }
            let mult: u64 = if q > m {
                text[m..q].parse().map_err(|_| format!("«{text}»: множитель скобки слишком велик"))?
            } else {
                1
            };
            for (z, cnt) in inner {
                add_atom(out, z, cnt * mult);
            }
            i = q;
        }
    }
    Ok(())
}

fn add_atom(out: &mut Vec<(u8, u64)>, z: u8, n: u64) {
    if let Some(slot) = out.iter_mut().find(|(zz, _)| *zz == z) {
        slot.1 += n;
    } else {
        out.push((z, n));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mm(s: &str) -> f64 {
        parse_formula(s).unwrap().molar_mass()
    }

    #[test]
    fn molar_masses_school() {
        // школьные значения в допуске 1%
        let checks = [
            ("H2O", 18.0), ("CO2", 44.0), ("H2SO4", 98.0), ("CaCO3", 100.0),
            ("NaCl", 58.5), ("NaOH", 40.0), ("HNO3", 63.0), ("Ca(OH)2", 74.0),
            ("Fe2O3", 160.0), ("Na2CO3", 106.0), ("Al2(SO4)3", 342.0),
            ("KOH", 56.0), ("BaCl2", 208.0), ("CuSO4", 160.0),
            ("CuSO4·5H2O", 250.0), ("Na2CO3·10H2O", 286.0),
        ];
        for (f, school) in checks {
            let got = mm(f);
            assert!(
                (got - school).abs() / school < 0.01,
                "{f}: {got:.3} vs школьное {school}"
            );
        }
    }

    #[test]
    fn brackets_and_hydrates() {
        let f = parse_formula("Al2(SO4)3").unwrap();
        assert_eq!(f.count_of(13), 2); // Al
        assert_eq!(f.count_of(8), 12); // O: 3×4
        assert_eq!(f.count_of(16), 3); // S
        let h = parse_formula("CuSO4·5H2O").unwrap();
        assert_eq!(h.count_of(29), 1); // Cu
        assert_eq!(h.count_of(8), 9); // O: 4 + 5
        assert_eq!(h.count_of(1), 10); // H: 5×2
        assert_eq!(h.hydrate.as_ref().unwrap().0, 5);
        let n = parse_formula("Na2CO3·10H2O").unwrap();
        assert_eq!(n.hydrate.as_ref().unwrap().0, 10);
        assert_eq!(n.count_of(1), 20);
        assert_eq!(n.count_of(11), 2);
    }

    #[test]
    fn mass_fractions() {
        let f = parse_formula("CaCO3").unwrap();
        let w_ca = f.mass_fraction(20).unwrap() * 100.0; // Ca
        assert!((w_ca - 40.0).abs() < 0.5, "w(Ca)={w_ca}");
        let w_o = f.mass_fraction(8).unwrap() * 100.0;
        assert!((w_o - 48.0).abs() < 0.5, "w(O)={w_o}");
        assert!(f.mass_fraction(26).is_none(), "Fe в CaCO3 нет");
        // CuSO4·5H2O: w(Cu) ≈ 25.5% (школьное 25.6%)
        let h = parse_formula("CuSO4·5H2O").unwrap();
        let w_cu = h.mass_fraction(29).unwrap() * 100.0;
        assert!((w_cu - 25.5).abs() < 0.3, "w(Cu)={w_cu}");
    }

    #[test]
    fn reject_garbage() {
        assert!(parse_formula("").is_err());
        assert!(parse_formula("h2o").is_err(), "регистр строгий");
        assert!(parse_formula("H2So4").is_err());
        assert!(parse_formula("Xx4").is_err(), "неизвестный элемент");
        assert!(parse_formula("Ca(OH").is_err(), "незакрытая скобка");
        assert!(parse_formula("Zz").is_err());
    }

    #[test]
    fn hill() {
        assert_eq!(parse_formula("H2O").unwrap().hill_formula(), "H2O");
        assert_eq!(parse_formula("C2H6O").unwrap().hill_formula(), "C2H6O");
        let gl = parse_formula("C6H12O6").unwrap();
        assert_eq!(gl.hill_formula(), "C6H12O6");
    }
}
