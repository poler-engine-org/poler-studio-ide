//! Разбор текстовых формулировок школьных задач по химии (SciVoice,
//! контур D v0.79.0): «Вычислите объём (н. у.) газа CO2, полученного
//! из 250 г CaCO3. Уравнение реакции: CaCO3 -> CaO + CO2.» →
//! структурированный ввод решателя.
//!
//! Метод — каскад правил по ключевым словам + позиционные извлекатели:
//! формулы-токены (свалidируются парсером формул), числа с единицами
//! («58,5 г», «0,2 моль/л», «1,204·10^24», «24%», «30 °C»), римские
//! валентности, сегмент уравнения. Позиции байтов совпадают в
//! оригинале и в lowercase-копии (кириллица А-Я → а-я сохраняет
//! длину), поэтому поиск ключевых слов идёт по копии, а извлечение
//! регистрозависимых токенов — по оригиналу.

use serde_json::{json, Value};

// ─── Числовые токены ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
struct NumTok {
    val: f64,
    start: usize,
    end: usize,
}

/// Все числа текста: «24», «6,4» (запятая — школьный стиль),
/// «1,204·10^24» (сливается в одно), «1.5» (точка тоже принимается).
fn numbers(text: &str) -> Vec<NumTok> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        // индекс формулы (O2, H2SO4, C6H12O6): цифра вплотную к латинской
        // букве — часть формулы, а не число задачи
        if i > 0 && b[i - 1].is_ascii_alphanumeric() {
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            continue;
        }
        let start = i;
        let mut j = i;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        let mut buf = String::from(&text[i..j]);
        // десятичный разделитель: запятая или точка + цифра
        if j + 1 < b.len()
            && (b[j] == b',' || b[j] == b'.')
            && b[j + 1].is_ascii_digit()
        {
            buf.push('.');
            j += 1;
            let ds = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            buf.push_str(&text[ds..j]);
        }
        let mut val: f64 = buf.parse().unwrap_or(0.0);
        // научная запись «·10^24»
        if j + 4 < b.len() && b[j] == 0xC2 && b[j + 1] == 0xB7 && b[j + 2] == b'1' && b[j + 3] == b'0' && b[j + 4] == b'^'
        {
            let mut k = j + 5;
            let neg = k < b.len() && b[k] == b'-';
            if neg {
                k += 1;
            }
            let es = k;
            while k < b.len() && b[k].is_ascii_digit() {
                k += 1;
            }
            if k > es {
                let e: i32 = text[es..k].parse().unwrap_or(0);
                val *= 10f64.powi(if neg { -e } else { e });
                j = k;
            }
        }
        out.push(NumTok { val, start, end: j });
        i = j;
    }
    out
}

/// Числа с указанной единицей: за числом (пробелов сколько угодно)
/// следует единица, за единицей — не буква и не '/'.
fn unit_nums(nums: &[NumTok], text: &str, unit: &str) -> Vec<NumTok> {
    nums.iter()
        .filter(|n| {
            let b = text.as_bytes();
            let mut p = n.end;
            // пробелы и NBSP (UTF-8 C2 A0)
            while p < b.len()
                && (b[p] == b' ' || (b[p] == 0xC2 && p + 1 < b.len() && b[p + 1] == 0xA0))
            {
                p += if b[p] == b' ' { 1 } else { 2 };
            }
            if !text[p..].starts_with(unit) {
                return false;
            }
            let q = p + unit.len();
            if q >= b.len() {
                return true;
            }
            let rest = &text[q..];
            let next = rest.chars().next().unwrap();
            !(next.is_alphanumeric() || next == '/')
        })
        .copied()
        .collect()
}

/// Числа-проценты: «24%», «12%.»
fn percents(nums: &[NumTok], text: &str) -> Vec<NumTok> {
    unit_nums(nums, text, "%")
}

// ─── Формулы-токены ────────────────────────────────────────────────────────

/// Конец максимального валидного префикса-формулы, начинающегося в
/// `start` (элементы со строгим регистром, индексы, одна скобочная
/// группа, гидрат «·nH2O»). None — валидной формулы нет.
fn try_formula_end(text: &str, start: usize) -> Option<usize> {
    let b = text.as_bytes();
    let mut i = start;
    let mut any = false;
    loop {
        if i >= b.len() || !b[i].is_ascii_uppercase() {
            break;
        }
        let mut j = i + 1;
        if j < b.len() && b[j].is_ascii_lowercase() {
            j += 1;
        }
        let sym = &text[i..j];
        if super::periodic::by_symbol(sym).is_none() {
            break;
        }
        let mut k = j;
        while k < b.len() && b[k].is_ascii_digit() {
            k += 1;
        }
        any = true;
        i = k;
        // скобочная группа (только если внутри — валидная формула)
        if i < b.len() && b[i] == b'(' {
            let mut depth = 1usize;
            let mut m = i + 1;
            while m < b.len() && depth > 0 {
                if b[m] == b'(' {
                    depth += 1;
                } else if b[m] == b')' {
                    depth -= 1;
                }
                m += 1;
            }
            if depth == 0 {
                let inner_ok = try_formula_end(text, i + 1).map(|e| e == m - 1).unwrap_or(false);
                if inner_ok {
                    let mut q = m;
                    while q < b.len() && b[q].is_ascii_digit() {
                        q += 1;
                    }
                    i = q;
                }
            }
        }
    }
    // гидрат «·5H2O»
    if i + 2 < b.len() + 1 && i + 1 < b.len() && b[i] == 0xC2 && b[i + 1] == 0xB7 {
        let mut j = i + 2;
        let ds = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > ds && j < b.len() && b[j].is_ascii_uppercase() {
            if let Some(e) = try_formula_end(text, j) {
                i = e;
            }
        }
    }
    if any {
        Some(i)
    } else {
        None
    }
}

/// Формулы-токены текста с позициями. Фильтр достоверности: токен
/// содержит цифру, строчную вторую букву («Fe», «O2», «NaCl») ИЛИ не
/// менее двух заглавных символов элементов («CO», «KOH» — вещества без
/// индексов и без строчных); одиночные «I», «V» (римские цифры —
/// максимальный префикс «I») отбрасываются.
fn formulas_in(text: &str) -> Vec<(String, usize, usize)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if b[i].is_ascii_uppercase() {
            let prev_ok = i == 0
                || !(b[i - 1].is_ascii_alphanumeric());
            if prev_ok {
                if let Some(end) = try_formula_end(text, i) {
                    let tok = &text[i..end];
                    let has_digit = tok.bytes().any(|c| c.is_ascii_digit());
                    let has_lower = tok.bytes().any(|c| c.is_ascii_lowercase());
                    let n_upper = tok.bytes().filter(|c| c.is_ascii_uppercase()).count();
                    // «углерода(IV)»: токен из одних римских букв, верно
                    // читающийся как число ≤ 30, — это римская цифра
                    // (степень окисления в номенклатуре), а не формула
                    let roman_like = tok.bytes().all(|c| matches!(c, b'I' | b'V' | b'X' | b'L' | b'C'))
                        && roman_parse(tok).is_some();
                    if (has_digit || has_lower || n_upper >= 2) && !roman_like {
                        out.push((tok.to_string(), i, end));
                    }
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

// ─── Римские цифры ─────────────────────────────────────────────────────────

fn roman_values(text: &str) -> Vec<(u32, usize, usize)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if matches!(b[i], b'I' | b'V' | b'X' | b'L' | b'C') {
            let prev_ok = i == 0 || !(b[i - 1].is_ascii_alphanumeric());
            if prev_ok {
                let start = i;
                let mut j = i;
                while j < b.len() && matches!(b[j], b'I' | b'V' | b'X' | b'L' | b'C') {
                    j += 1;
                }
                let next_ok = j >= b.len() || !(b[j].is_ascii_alphanumeric());
                if next_ok && j - start >= 1 {
                    if let Some(v) = roman_parse(&text[start..j]) {
                        out.push((v, start, j));
                    }
                    i = j;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

fn roman_parse(s: &str) -> Option<u32> {
    let vals: Vec<(u8, i64)> = s
        .bytes()
        .map(|c| {
            (
                c,
                match c {
                    b'I' => 1,
                    b'V' => 5,
                    b'X' => 10,
                    b'L' => 50,
                    b'C' => 100,
                    _ => 0,
                },
            )
        })
        .collect();
    if vals.iter().any(|&(_, v)| v == 0) {
        return None;
    }
    // знаковый аккумулятор: вычитание младшей ПЕРЕД старшей (IV, IX)
    // на нуле u32 давало underflow → «IV» не распознавалась вовсе
    let mut total = 0i64;
    for k in 0..vals.len() {
        let v = vals[k].1;
        let next = vals.get(k + 1).map(|&(_, nv)| nv).unwrap_or(0);
        if v < next {
            total -= v;
        } else {
            total += v;
        }
    }
    if total > 0 && total <= 30 {
        Some(total as u32)
    } else {
        None
    }
}

// ─── Уравнение реакции ─────────────────────────────────────────────────────

/// Сегмент уравнения в тексте: расширяется от «->» до границ
/// предложения/двоеточия. Возвращает (сегмент, позиция начала).
fn find_equation(text: &str) -> Option<(String, usize)> {
    let arrow = text.find("->")?;
    let b = text.as_bytes();
    // левая граница: до ':' '.' '?' '!' или начала
    let mut s = arrow;
    while s > 0 && !matches!(b[s - 1], b':' | b'.' | b'?' | b'!' | b'\n') {
        s -= 1;
    }
    // правая граница: до '.' '?' '!' или конца
    let mut e = arrow + 2;
    while e < b.len() && !matches!(b[e], b'.' | b'?' | b'!' | b'\n') {
        e += 1;
    }
    let seg = text[s..e].trim();
    if seg.contains("->") && seg.contains('+') {
        Some((seg.to_string(), s))
    } else {
        None
    }
}

// ─── Вспомогательные ───────────────────────────────────────────────────────

/// Байт «букво-цифровой» для границ слова: ASCII-буквы/цифры и ВСЕ
/// байты ≥ 0x80 (кириллица UTF-8 — всегда часть слова).
fn byte_wordish(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b >= 0x80
}

/// Конец первого вхождения ключевого слова в lowercase-копии
/// (границы слова соблюдаются). Позиции байтов идентичны оригиналу.
fn kw_end(low: &str, kw: &str) -> Option<usize> {
    let bl = low.as_bytes();
    let bk = kw.as_bytes();
    let mut from = 0usize;
    while let Some(p) = low[from..].find(kw) {
        let s = from + p;
        let e = s + bk.len();
        let left_ok = s == 0 || !byte_wordish(bl[s - 1]);
        let right_ok = e >= bl.len() || !byte_wordish(bl[e]);
        if left_ok && right_ok {
            return Some(e);
        }
        from = s + 1;
    }
    None
}

fn next_formula_after(forms: &[(String, usize, usize)], pos: usize) -> Option<String> {
    forms.iter().find(|(_, s, _)| *s >= pos).map(|(t, _, _)| t.clone())
}

fn num_after(nums: &[NumTok], pos: usize) -> Option<NumTok> {
    nums.iter().find(|n| n.start >= pos).copied()
}

/// Символ элемента в скобках после позиции: «…натрия (Na)…» → Na.
fn symbol_in_parens(text: &str, from: usize) -> Option<String> {
    let b = text.as_bytes();
    let mut i = from;
    while i < b.len() {
        if b[i] == b'(' {
            let mut j = i + 1;
            if j < b.len() && b[j].is_ascii_uppercase() {
                j += 1;
                if j < b.len() && b[j].is_ascii_lowercase() {
                    j += 1;
                }
                if j < b.len() && b[j] == b')' {
                    return Some(text[i + 1..j].to_string());
                }
            }
            // неэлементная скобка — выходим
            return None;
        }
        i += 1;
    }
    None
}

// ─── Каскад разбора ────────────────────────────────────────────────────────

/// Разобрать формулировку задачи → (тип, ввод для [`super::curriculum::solve`]).
pub fn parse_task_text(text: &str) -> Result<(String, Value), String> {
    let low = text.to_lowercase();
    let nums = numbers(text);
    let masses_raw = [
        unit_nums(&nums, text, "г"),
        unit_nums(&nums, text, "кг"),
    ];
    let mut masses: Vec<NumTok> = masses_raw.concat();
    masses.sort_by_key(|n| n.start);
    let liters = unit_nums(&nums, text, "л");
    let mls = unit_nums(&nums, text, "мл");
    let mols = unit_nums(&nums, text, "моль");
    let molls = unit_nums(&nums, text, "моль/л");
    let pcts = percents(&nums, text);
    let forms = formulas_in(text);
    let eq = find_equation(text);
    let eq_start = eq.as_ref().map(|(_, s)| *s).unwrap_or(usize::MAX);
    let celsius = unit_nums(&nums, text, "°C");

    // 1. «Расставьте коэффициенты…»
    if kw_end(&low, "коэффициенты").is_some() && low.contains("расставьте") {
        let (segment, _) = eq.ok_or("нет уравнения в тексте")?;
        return Ok(("balance".into(), json!({ "equation": segment })));
    }

    // 2. концентрации
    if low.contains("концентрац") && (!molls.is_empty() || !mls.is_empty()) {
        if low.contains("до объёма") {
            let (c1, v1, v2) = (
                molls[0].val,
                mls[0].val,
                mls.get(1).ok_or("нет второго объёма")?.val,
            );
            return Ok(("molarity_dilution".into(), json!({ "c1": c1, "v1_ml": v1, "v2_ml": v2 })));
        }
        if low.contains("какая масса") {
            let f = next_formula_after(&forms, kw_end(&low, "масса").unwrap_or(0))
                .ok_or("нет формулы вещества")?;
            return Ok((
                "molarity_mass".into(),
                json!({ "formula": f, "c": molls[0].val, "volume_ml": mls[0].val }),
            ));
        }
        let m = masses[0];
        let f = next_formula_after(&forms, m.end).ok_or("нет формулы после массы")?;
        return Ok((
            "molarity".into(),
            json!({ "formula": f, "mass_g": m.val, "volume_ml": mls[0].val }),
        ));
    }

    // 3. правило Вант-Гоффа
    if low.contains("во сколько раз") && low.contains("скорость") {
        let dt = celsius[0].val;
        let g = num_after(&nums, kw_end(&low, "равен").ok_or("нет «равен»")?)
            .ok_or("нет температурного коэффициента")?
            .val;
        return Ok(("van_hoff".into(), json!({ "gamma": g, "dt_c": dt })));
    }

    // 4. многостадийный выход
    if low.contains("две стадии") || low.contains("общий выход") {
        let ys: Vec<f64> = pcts.iter().map(|n| n.val).collect();
        return Ok(("staged_yield".into(), json!({ "yields": ys })));
    }

    // 5. выход продукта
    if low.contains("практически получили") {
        let m = masses[0];
        let a = masses[1];
        let from = next_formula_after(&forms, m.end).ok_or("нет вещества-сырья")?;
        let to = next_formula_after(&forms, a.end).ok_or("нет продукта")?;
        return Ok((
            "yield".into(),
            json!({ "equation": eq.as_ref().map(|(s, _)| s.clone()).unwrap_or_default(),
                    "from": from, "mass_g": m.val, "to": to, "actual_g": a.val }),
        ));
    }

    // 6. обратный расчёт по выходу
    if low.contains("нужно взять") && low.contains("при выходе") {
        let from = next_formula_after(&forms, kw_end(&low, "массу").ok_or("нет «массу»")?)
            .ok_or("нет сырья")?;
        let m = masses[0];
        let to = next_formula_after(&forms, m.end).ok_or("нет продукта")?;
        return Ok((
            "yield_inverse".into(),
            json!({ "equation": eq.as_ref().map(|(s, _)| s.clone()).unwrap_or_default(),
                    "from": from, "to": to, "mass_g": m.val, "yield_pct": pcts[0].val }),
        ));
    }

    // 7. примеси + выход (комбинированная)
    if low.contains("руды, содержащей") || (low.contains("с выходом") && low.contains("содержащ")) {
        let m = masses[0];
        let purity = pcts[0].val;
        let eta = pcts.get(1).ok_or("нет процента выхода")?.val;
        let raw = next_formula_after(&forms, pcts[0].end).ok_or("нет сырья")?;
        let to = next_formula_after(&forms, kw_end(&low, "полученного").unwrap_or(kw_end(&low, "массу").unwrap_or(0)))
            .ok_or("нет продукта")?;
        return Ok((
            "impurity_yield".into(),
            json!({ "equation": eq.as_ref().map(|(s, _)| s.clone()).unwrap_or_default(),
                    "raw": raw, "mass_g": m.val, "purity_pct": purity,
                    "yield_pct": eta, "to": to }),
        ));
    }

    // 8. примеси
    if low.contains("сырья, содержащего") {
        let m = masses[0];
        let to = next_formula_after(&forms, kw_end(&low, "массу").ok_or("нет «массу»")?)
            .ok_or("нет продукта")?;
        let raw = next_formula_after(&forms, pcts[0].end).ok_or("нет сырья")?;
        return Ok((
            "impurity".into(),
            json!({ "equation": eq.as_ref().map(|(s, _)| s.clone()).unwrap_or_default(),
                    "raw": raw, "mass_g": m.val, "purity_pct": pcts[0].val, "to": to }),
        ));
    }

    // 9. избыток/недостаток (с уравнением)
    if eq.is_some() && low.contains("взаимодействии") {
        let (segment, _) = eq.unwrap();
        // величины до уравнения: г/л с флагом объёма, по позициям
        let mut qs: Vec<(f64, bool, usize)> = masses
            .iter()
            .filter(|n| n.start < eq_start)
            .map(|n| (n.val, false, n.start))
            .collect();
        qs.extend(liters.iter().filter(|n| n.start < eq_start).map(|n| (n.val, true, n.start)));
        qs.sort_by_key(|q| q.2);
        if qs.len() < 2 {
            return Err("нужно два реагента".into());
        }
        let product = next_formula_after(&forms, kw_end(&low, "масса").ok_or("нет «масса»")?)
            .ok_or("нет продукта")?;
        let (q1, v1, _) = qs[0];
        let (q2, v2, _) = qs[1];
        let r1 = next_formula_after(&forms, qs[0].2).ok_or("нет первого реагента")?;
        let r2 = next_formula_after(&forms, qs[1].2).ok_or("нет второго реагента")?;
        if v1 || v2 {
            return Ok((
                "limiting_reagent_volume".into(),
                json!({ "equation": segment, "r1": r1, "q1": q1, "q1_is_volume": v1,
                        "r2": r2, "q2": q2, "q2_is_volume": v2, "product": product }),
            ));
        }
        return Ok((
            "limiting_reagent".into(),
            json!({ "equation": segment, "r1": r1, "m1_g": q1,
                    "r2": r2, "m2_g": q2, "product": product }),
        ));
    }

    // 10. выпаривание
    if low.contains("выпарили") {
        let (m, ev) = (masses[0], masses[1]);
        return Ok((
            "solution_evaporate".into(),
            json!({ "m_g": m.val, "w_pct": pcts[0].val, "evaporated_g": ev.val }),
        ));
    }

    // 11. смешение двух смесей/растворов (2 массы + 2 доли)
    if masses.len() == 2 && pcts.len() == 2 {
        return Ok((
            "mixture_two".into(),
            json!({ "m1_g": masses[0].val, "w1_pct": pcts[0].val,
                    "m2_g": masses[1].val, "w2_pct": pcts[1].val }),
        ));
    }

    // 12. добавление воды/соли
    if low.contains("добавили") && masses.len() == 2 {
        let tail: String = text[masses[1].end..].chars().take(24).collect();
        let tail_low = tail.to_lowercase();
        if tail_low.contains("вод") {
            return Ok((
                "solution_add_water".into(),
                json!({ "m_g": masses[0].val, "w_pct": pcts[0].val,
                        "add_water_g": masses[1].val }),
            ));
        }
        if tail_low.contains("сол") {
            return Ok((
                "solution_add_solute".into(),
                json!({ "m_g": masses[0].val, "w_pct": pcts[0].val,
                        "add_solute_g": masses[1].val }),
            ));
        }
    }

    // 13. кристаллогидраты
    if low.contains("кристаллогидрате") {
        let f = next_formula_after(&forms, kw_end(&low, "кристаллогидрате").unwrap_or(0))
            .ok_or("нет формулы кристаллогидрата")?;
        return Ok(("hydrate_water".into(), json!({ "formula": f })));
    }

    // 14. вывод простейшей формулы
    if low.contains("простейшую формулу") {
        let mut els = Vec::new();
        for p in &pcts {
            let sym = symbol_in_parens(text, p.end).ok_or("нет символа в скобках")?;
            els.push(json!({ "element": sym, "w_pct": p.val }));
        }
        if els.is_empty() {
            return Err("нет массовых долей".into());
        }
        return Ok(("formula_from_ratio".into(), json!({ "elements": els })));
    }

    // 15. формула по валентности
    if low.contains("составьте формулу соединения") {
        let mut from = 0usize;
        let mut pairs: Vec<(String, u32)> = Vec::new();
        while let Some(e) = kw_end(&low[from..], "валентность").map(|p| from + p) {
            let mut i = e;
            let b = text.as_bytes();
            while i < b.len() && b[i] == b' ' {
                i += 1;
            }
            // символ [A-Z][a-z]?
            if i < b.len() && b[i].is_ascii_uppercase() {
                let mut j = i + 1;
                if j < b.len() && b[j].is_ascii_lowercase() {
                    j += 1;
                }
                let sym = &text[i..j];
                if super::periodic::by_symbol(sym).is_some() {
                    // римская цифра после «равна» / «—»
                    let rom = roman_values(text)
                        .into_iter()
                        .find(|(_, s, _)| *s > j);
                    if let Some((v, _, _)) = rom {
                        pairs.push((sym.to_string(), v));
                    }
                }
            }
            from = e;
        }
        if pairs.len() < 2 {
            return Err("не найдены две валентности".into());
        }
        return Ok((
            "formula_from_valence".into(),
            json!({ "a": pairs[0].0, "a_valence": pairs[0].1,
                    "b": pairs[1].0, "b_valence": pairs[1].1 }),
        ));
    }

    // 16. молярная масса
    if low.contains("молярную массу") {
        let f = next_formula_after(&forms, kw_end(&low, "массу").ok_or("нет «массу»")?)
            .ok_or("нет формулы")?;
        return Ok(("molar_mass".into(), json!({ "formula": f })));
    }

    // 17. массовая доля элемента
    if low.contains("массовую долю элемента") {
        let el_pos = kw_end(&low, "элемента").ok_or("нет «элемента»")?;
        let el = symbol_in_parens(text, el_pos)
            .ok_or("нет символа элемента в скобках")?;
        // падежи: «в веществе CaCO3» / «в вещества…» / «веществом…»
        let subst_pos = kw_end(&low, "веществе")
            .or_else(|| kw_end(&low, "вещества"))
            .or_else(|| kw_end(&low, "веществом"))
            .ok_or("нет «вещества»")?;
        let f = next_formula_after(&forms, subst_pos)
            .ok_or("нет формулы вещества")?;
        return Ok(("mass_fraction_element".into(), json!({ "formula": f, "element": el })));
    }

    // 18. растворение
    if low.contains("растворением") {
        return Ok((
            "solution_w".into(),
            json!({ "solute_g": masses[0].val, "water_g": masses[1].val }),
        ));
    }

    // 19. сухое вещество
    if low.contains("сухого вещества") {
        return Ok((
            "dry_mass".into(),
            json!({ "m_g": masses[0].val, "water_pct": pcts[0].val }),
        ));
    }

    // 20. остальные компоненты сплава
    if low.contains("остальных компонентов") {
        return Ok((
            "alloy_rest".into(),
            json!({ "total_g": masses[0].val, "w_main_pct": pcts[0].val }),
        ));
    }

    // 21. масса всей смеси по компоненту
    if low.contains("масса всей смеси") || low.contains("массу всей смеси") {
        return Ok((
            "mixture_mass_from_fraction".into(),
            json!({ "component_g": masses[0].val, "w_pct": pcts[0].val }),
        ));
    }

    // 22а. примеси: «…массой M г примеси составляют K г…» → доля чистого
    if low.contains("примеси составляют") && masses.len() == 2 {
        return Ok((
            "mixture_fraction".into(),
            json!({ "total_g": masses[0].val,
                    "component_g": masses[0].val - masses[1].val }),
        ));
    }

    // 22. доля компонента в смеси
    if low.contains("смеси содержится") || low.contains("приходится") {
        return Ok((
            "mixture_fraction".into(),
            json!({ "total_g": masses[0].val, "component_g": masses[1].val }),
        ));
    }

    // 23. масса компонента по доле
    if low.contains("такой смеси") || low.contains("сколько граммов") {
        return Ok((
            "mixture_component_from_fraction".into(),
            json!({ "total_g": masses[0].val, "w_pct": pcts[0].val }),
        ));
    }

    // 24. семейство «по уравнению»
    if let Some((segment, _)) = eq {
        let asks_volume = low.contains("объём");
        let mass_given = masses.iter().any(|n| n.start < eq_start);
        let given_mass = masses.iter().find(|n| n.start < eq_start).copied();
        let given_vol = liters.iter().find(|n| n.start < eq_start).copied();
        let target_kw = if asks_volume {
            kw_end(&low, "объём")
        } else {
            kw_end(&low, "массу").or_else(|| kw_end(&low, "масса"))
        };
        let to = next_formula_after(&forms, target_kw.unwrap_or(0))
            .ok_or("нет целевого вещества")?;
        if asks_volume && mass_given {
            let m = given_mass.unwrap();
            let from = next_formula_after(&forms, m.end).ok_or("нет исходного вещества")?;
            return Ok((
                "volume_from_mass_eq".into(),
                json!({ "equation": segment, "from": from, "mass_g": m.val, "to": to }),
            ));
        }
        if asks_volume && !mass_given {
            let v = given_vol.ok_or("нет исходного объёма")?;
            let from = next_formula_after(&forms, v.end).ok_or("нет исходного вещества")?;
            return Ok((
                "volume_from_volume_eq".into(),
                json!({ "equation": segment, "from": from, "volume_l": v.val, "to": to }),
            ));
        }
        if !asks_volume && !mass_given {
            let v = given_vol.ok_or("нет исходного объёма")?;
            let from = next_formula_after(&forms, v.end).ok_or("нет исходного вещества")?;
            return Ok((
                "mass_from_volume_eq".into(),
                json!({ "equation": segment, "from": from, "volume_l": v.val, "to": to }),
            ));
        }
        let m = given_mass.unwrap();
        let from = next_formula_after(&forms, m.end).ok_or("нет исходного вещества")?;
        return Ok((
            "mass_from_mass_eq".into(),
            json!({ "equation": segment, "from": from, "mass_g": m.val, "to": to }),
        ));
    }

    // 25. количество вещества по числу молекул
    if low.contains("количеству вещества соответствует") && low.contains("молекул") {
        let particles = nums[0].val;
        let f = next_formula_after(&forms, kw_end(&low, "молекул").ok_or("нет «молекул»")?)
            .ok_or("нет формулы")?;
        return Ok(("moles_from_particles".into(), json!({ "formula": f, "particles": particles })));
    }

    // 26. число молекул
    if low.contains("молекул") {
        let n = mols[0];
        let f = next_formula_after(&forms, n.end)
            .or_else(|| next_formula_after(&forms, kw_end(&low, "вещества").unwrap_or(0)))
            .ok_or("нет формулы")?;
        return Ok(("particles_from_moles".into(), json!({ "formula": f, "moles": n.val })));
    }

    // 27/28. объём газа
    if low.contains("объём") && !mols.is_empty() {
        let n = mols[0];
        let f = next_formula_after(&forms, n.end).ok_or("нет формулы газа")?;
        return Ok(("volume_from_moles".into(), json!({ "formula": f, "moles": n.val })));
    }
    if low.contains("объём") && !masses.is_empty() {
        let m = masses[0];
        let f = next_formula_after(&forms, m.end).ok_or("нет формулы газа")?;
        return Ok(("volume_from_mass".into(), json!({ "formula": f, "mass_g": m.val })));
    }

    // 29. масса газа по объёму
    if low.contains("массу имеют") && !liters.is_empty() {
        let v = liters[0];
        let f = next_formula_after(&forms, v.end).ok_or("нет формулы газа")?;
        return Ok(("mass_from_volume".into(), json!({ "formula": f, "volume_l": v.val })));
    }

    // 30. масса по количеству вещества
    if low.contains("массу имеет") && !mols.is_empty() {
        let n = mols[0];
        let f = next_formula_after(&forms, kw_end(&low, "вещества").unwrap_or(n.end))
            .ok_or("нет формулы")?;
        return Ok(("mass_from_moles".into(), json!({ "formula": f, "moles": n.val })));
    }

    // 31. количество вещества по массе
    if low.contains("количество вещества") && !masses.is_empty() {
        let m = masses[0];
        let f = next_formula_after(&forms, m.end).ok_or("нет формулы")?;
        return Ok(("moles_from_mass".into(), json!({ "formula": f, "mass_g": m.val })));
    }

    Err(format!("формулировка не распознана: «{text}»"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_units() {
        let t = "Вычислите объём (н. у.), который занимают 64 г газа O2 и 5,6 л (н. у.) газа CO.";
        let ns = numbers(t);
        let gs = unit_nums(&ns, t, "г");
        let ls = unit_nums(&ns, t, "л");
        assert_eq!(gs.len(), 1);
        assert_eq!(gs[0].val, 64.0);
        assert_eq!(ls.len(), 1);
        assert_eq!(ls[0].val, 5.6);
        // научная запись
        let s = "соответствует 1,204·10^24 молекул O2";
        let ns2 = numbers(s);
        assert_eq!(ns2.len(), 1, "только слитое число: {:?}", ns2);
        assert!((ns2[0].val - 1.204e24).abs() < 1e18);
        // проценты
        let p = "Массовая доля крахмала в смеси равна 12%.";
        let ns3 = numbers(p);
        let ps = percents(&ns3, p);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].val, 12.0);
    }

    #[test]
    fn formulas_tokens() {
        let t = "массу Fe, которая получается из 160 г Fe2O3 и CuSO4·5H2O (IV)";
        let fs = formulas_in(t);
        let toks: Vec<&str> = fs.iter().map(|(s, _, _)| s.as_str()).collect();
        assert_eq!(toks, vec!["Fe", "Fe2O3", "CuSO4·5H2O"]);
        // грабля D: формулы без индексов и без строчных (CO, KOH) — тоже
        // вещества; римские IV/VI при этом остаются цифрами, не формулами
        let t2 = "массу CO из 44,8 л (н. у.) газа CO; молярную массу KOH; оксид углерода(IV)";
        let fs2 = formulas_in(t2);
        let toks2: Vec<&str> = fs2.iter().map(|(s, _, _)| s.as_str()).collect();
        assert_eq!(toks2, vec!["CO", "CO", "KOH"]);
    }

    #[test]
    fn equation_segment() {
        let t = "Вычислите массу Fe. Уравнение реакции: Fe2O3 + CO -> Fe + CO2.";
        let (seg, _) = find_equation(t).unwrap();
        assert_eq!(seg, "Fe2O3 + CO -> Fe + CO2");
    }

    #[test]
    fn parse_samples() {
        let (kind, inp) = parse_task_text(
            "Вычислите массу O2, которая получается из 12 г H2. \
             Уравнение реакции: H2 + O2 -> H2O.",
        )
        .unwrap();
        assert_eq!(kind, "mass_from_mass_eq");
        assert_eq!(inp["from"], "H2");
        assert_eq!(inp["to"], "O2");
        assert_eq!(inp["mass_g"], 12.0);

        let (kind, _) = parse_task_text(
            "Расставьте коэффициенты в уравнении реакции: Al + HCl -> AlCl3 + H2.",
        )
        .unwrap();
        assert_eq!(kind, "balance");

        let (kind, inp) = parse_task_text(
            "Составьте формулу соединения, в которой валентность Al равна III, \
             а валентность O — II.",
        )
        .unwrap();
        assert_eq!(kind, "formula_from_valence");
        assert_eq!(inp["a"], "Al");
        assert_eq!(inp["a_valence"], 3);
        assert_eq!(inp["b"], "O");
        assert_eq!(inp["b_valence"], 2);

        // грабля D: вычитающая римская запись (IV = 4) на u32-аккумуляторе
        // давала underflow → «IV» читалась как следующая цифра (II = 2)
        let (kind, inp) = parse_task_text(
            "Составьте формулу соединения, в которой валентность C равна IV, а валентность O — II.",
        )
        .unwrap();
        assert_eq!(kind, "formula_from_valence");
        assert_eq!(inp["a"], "C");
        assert_eq!(inp["a_valence"], 4);
        assert_eq!(inp["b"], "O");
        assert_eq!(inp["b_valence"], 2);
    }

    #[test]
    fn corpus_text_roundtrip() {
        // Каждая формулировка корпуса → структурированный ввод → решение,
        // совпадающее с эталоном. Это тест понимания естественного языка.
        use super::super::curriculum::{solve, tasks};
        use super::super::curriculum::TaskAnswer;
        let mut fails: Vec<String> = Vec::new();
        for t in tasks() {
            let res = parse_task_text(&t.text)
                .map_err(|e| format!("разбор: {e}"))
                .and_then(|(kind, input)| solve(&kind, &input))
                .and_then(|a| a.matches(t).map(|_| a).map_err(|e| e));
            if let Err(e) = res {
                fails.push(format!("{} «{}»: {}", t.id, &t.text[..t.text.len().min(60)], e));
            }
        }
        assert!(
            fails.is_empty(),
            "не разобрано/не решено {} формулировок:\n{}",
            fails.len(),
            fails.join("\n")
        );
    }
}
