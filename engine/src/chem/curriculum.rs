//! Контур D «Curriculum Learning» (v0.79.0): задачник 5–9 класс как
//! строгий бенчмарк математического ядра.
//!
//! Методика — Benchmark-Driven Development: каждая задача с известным
//! ответом превращается в юнит-тест; корпус из 187 задач покрывает
//! лестницу «смеси (5–6) → моль и формулы (7) → уравнения, избыток,
//! выход, растворы (8) → концентрации, скорость, горение (9)».
//! Ответы корпуса посчитаны независимой Python-реализацией
//! (scripts/curriculum_gen.py, свой парсер формул и свой
//! балансировщик на Fraction) — принцип двойной записи MVR.
//!
//! Структура: [`tasks`] — встроенный корпус (`include_str!`),
//! [`solve`] — решатель по типу задачи, [`run_grade`] — прогон
//! класса с отчётом, текстовый разбор формулировок — в
//! [`super::curriculum_nl`].

use serde::Deserialize;
use std::sync::OnceLock;

use super::formula::parse_formula;
use super::periodic;
use super::stoich::{self, parse_equation};

// ─── Модель задачи ────────────────────────────────────────────────────────

/// Задача задачника (строка JSON корпуса).
#[derive(Deserialize, Debug, Clone)]
pub struct Task {
    pub id: String,
    pub grade: u8,
    pub topic: String,
    pub kind: String,
    #[serde(default)]
    pub text: String,
    pub input: serde_json::Value,
    #[serde(default)]
    pub answer: Option<f64>,
    #[serde(default)]
    pub answer_str: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub tol: Option<f64>,
}

impl Task {
    /// Допуск сравнения числового ответа.
    pub fn tolerance(&self) -> f64 {
        self.tol
            .unwrap_or_else(|| self.answer.map(|a| (a.abs() * 0.008).max(0.02)).unwrap_or(0.02))
    }
}

/// Корпус задач (187 задач, 5–9 класс), встроен в бинарник.
pub fn tasks() -> &'static [Task] {
    static TASKS: OnceLock<Vec<Task>> = OnceLock::new();
    TASKS.get_or_init(|| {
        let raw: &str = include_str!("curriculum_tasks.json");
        #[derive(Deserialize)]
        struct Doc {
            tasks: Vec<Task>,
        }
        let doc: Doc = serde_json::from_str(raw).expect("curriculum_tasks.json: битый JSON");
        doc.tasks
    })
}

/// Ответ задачи: число (с допуском) или строка (точное совпадение).
#[derive(Debug, Clone, PartialEq)]
pub enum TaskAnswer {
    Num(f64),
    Str(String),
}

impl TaskAnswer {
    /// Проверка против эталона задачи.
    pub fn matches(&self, task: &Task) -> Result<(), String> {
        match (self, task.answer, &task.answer_str) {
            (TaskAnswer::Num(got), Some(exp), _) => {
                let tol = task.tolerance();
                if (got - exp).abs() <= tol {
                    Ok(())
                } else {
                    Err(format!(
                        "{} [{}/{}]: получено {got:.6}, эталон {exp:.6}, |Δ|={:.6} > допуск {tol:.4}",
                        task.id, task.grade, task.topic,
                        (got - exp).abs()
                    ))
                }
            }
            (TaskAnswer::Str(got), _, Some(exp)) => {
                if got == exp {
                    Ok(())
                } else {
                    Err(format!(
                        "{} [{}/{}]: получено «{got}», эталон «{exp}»",
                        task.id, task.grade, task.topic
                    ))
                }
            }
            _ => Err(format!("{}: несоответствие типа ответа", task.id)),
        }
    }
}

// ─── Извлечение полей ввода ───────────────────────────────────────────────

fn f_field(v: &serde_json::Value, key: &str) -> Result<f64, String> {
    v.get(key)
        .and_then(|x| x.as_f64())
        .ok_or_else(|| format!("поле «{key}» отсутствует или не число: {v}"))
}

fn s_field(v: &serde_json::Value, key: &str) -> Result<String, String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("поле «{key}» отсутствует или не строка: {v}"))
}

/// Z элемента: по символу («Na») или русскому имени («натрия»).
fn z_field(v: &serde_json::Value, key: &str) -> Result<u8, String> {
    let s = s_field(v, key)?;
    periodic::by_symbol(&s)
        .or_else(|| periodic::by_name_ru(&s))
        .map(|e| e.z)
        .ok_or_else(|| format!("«{s}» — не элемент"))
}

fn yields_list(v: &serde_json::Value) -> Result<Vec<f64>, String> {
    v.get("yields")
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
        .filter(|a: &Vec<f64>| !a.is_empty())
        .ok_or_else(|| format!("поле «yields» отсутствует: {v}"))
}

/// Масса воды в кристаллогидрате: n·M(H2O)/M(гидрат)·100%.
fn hydrate_water_pct(formula: &str) -> Result<f64, String> {
    let f = parse_formula(formula)?;
    let (n, _) = f
        .hydrate
        .as_ref()
        .ok_or_else(|| format!("«{formula}» — не кристаллогидрат (нет ·nH2O)"))?;
    let m_h2o = parse_formula("H2O")?.molar_mass();
    Ok(*n as f64 * m_h2o / f.molar_mass() * 100.0)
}

// ─── Решатель ─────────────────────────────────────────────────────────────

/// Решить задачу по структурированному вводу.
pub fn solve(kind: &str, input: &serde_json::Value) -> Result<TaskAnswer, String> {
    use TaskAnswer::{Num, Str};
    let ans = match kind {
        // ── 5–6 класс: смеси и доли ──
        "mixture_fraction" => {
            Num(f_field(input, "component_g")? / f_field(input, "total_g")? * 100.0)
        }
        "mixture_component_from_fraction" => {
            Num(f_field(input, "total_g")? * f_field(input, "w_pct")? / 100.0)
        }
        "mixture_mass_from_fraction" => {
            Num(f_field(input, "component_g")? / f_field(input, "w_pct")? * 100.0)
        }
        "mixture_two" => {
            let (m1, w1) = (f_field(input, "m1_g")?, f_field(input, "w1_pct")?);
            let (m2, w2) = (f_field(input, "m2_g")?, f_field(input, "w2_pct")?);
            Num((m1 * w1 + m2 * w2) / (m1 + m2))
        }
        "solution_evaporate" => {
            let (m, w, ev) = (
                f_field(input, "m_g")?,
                f_field(input, "w_pct")?,
                f_field(input, "evaporated_g")?,
            );
            Num(m * w / 100.0 / (m - ev) * 100.0)
        }
        "alloy_rest" => {
            Num(f_field(input, "total_g")? * (100.0 - f_field(input, "w_main_pct")?) / 100.0)
        }
        "dry_mass" => {
            Num(f_field(input, "m_g")? * (100.0 - f_field(input, "water_pct")?) / 100.0)
        }

        // ── 7 класс: моль, формулы ──
        "molar_mass" => Num(parse_formula(&s_field(input, "formula")?)?.molar_mass()),
        "mass_fraction_element" => {
            let f = parse_formula(&s_field(input, "formula")?)?;
            let z = z_field(input, "element")?;
            Num(f.mass_fraction(z).ok_or("элемента нет в формуле")? * 100.0)
        }
        "moles_from_mass" => {
            let f = parse_formula(&s_field(input, "formula")?)?;
            Num(stoich::moles_from_mass(f_field(input, "mass_g")?, f.molar_mass()))
        }
        "mass_from_moles" => {
            let f = parse_formula(&s_field(input, "formula")?)?;
            Num(stoich::mass_from_moles(f_field(input, "moles")?, f.molar_mass()))
        }
        "particles_from_moles" => {
            Num(stoich::particles_from_moles(f_field(input, "moles")?))
        }
        "moles_from_particles" => {
            Num(stoich::moles_from_particles(f_field(input, "particles")?))
        }
        "volume_from_moles" => Num(stoich::volume_from_moles(f_field(input, "moles")?)),
        "volume_from_mass" => {
            let f = parse_formula(&s_field(input, "formula")?)?;
            Num(stoich::moles_from_mass(f_field(input, "mass_g")?, f.molar_mass()) * stoich::VM)
        }
        "mass_from_volume" => {
            let f = parse_formula(&s_field(input, "formula")?)?;
            Num(stoich::moles_from_volume(f_field(input, "volume_l")?) * f.molar_mass())
        }
        "formula_from_valence" => {
            let a = s_field(input, "a")?;
            let b = s_field(input, "b")?;
            let (va, vb) = (f_field(input, "a_valence")?, f_field(input, "b_valence")?);
            let va = va as i64;
            let vb = vb as i64;
            if va <= 0 || vb <= 0 {
                return Err("валентность должна быть положительной".into());
            }
            let g = gcd64(va, vb);
            let na = vb / g;
            let nb = va / g;
            let fa = format!("{a}{}", if na > 1 { na.to_string() } else { String::new() });
            let fb = format!("{b}{}", if nb > 1 { nb.to_string() } else { String::new() });
            Str(fa + &fb)
        }

        // ── 8 класс: уравнения ──
        "balance" => Str(parse_equation(&s_field(input, "equation")?)?.coefs_str()),
        "mass_from_mass_eq" => {
            let eq = parse_equation(&s_field(input, "equation")?)?;
            let (i_from, i_to) = eq_indices(&eq, &s_field(input, "from")?, &s_field(input, "to")?)?;
            Num(eq.convert(i_from, f_field(input, "mass_g")?, true, i_to) * eq.molar_mass(i_to))
        }
        "volume_from_mass_eq" => {
            let eq = parse_equation(&s_field(input, "equation")?)?;
            let (i_from, i_to) = eq_indices(&eq, &s_field(input, "from")?, &s_field(input, "to")?)?;
            Num(eq.convert(i_from, f_field(input, "mass_g")?, true, i_to) * stoich::VM)
        }
        "mass_from_volume_eq" => {
            let eq = parse_equation(&s_field(input, "equation")?)?;
            let (i_from, i_to) = eq_indices(&eq, &s_field(input, "from")?, &s_field(input, "to")?)?;
            // объём (н. у.) → моли: V / Vm, затем мольное соотношение и масса
            Num(
                eq.convert(i_from, f_field(input, "volume_l")? / stoich::VM, false, i_to)
                    * eq.molar_mass(i_to),
            )
        }
        "volume_from_volume_eq" => {
            let eq = parse_equation(&s_field(input, "equation")?)?;
            let (i_from, i_to) = eq_indices(&eq, &s_field(input, "from")?, &s_field(input, "to")?)?;
            // литры → моли → мольное соотношение → литры
            Num(
                eq.convert(i_from, f_field(input, "volume_l")? / stoich::VM, false, i_to)
                    * stoich::VM,
            )
        }
        "limiting_reagent" => {
            let eq = parse_equation(&s_field(input, "equation")?)?;
            let i1 = eq.index_of(&s_field(input, "r1")?).ok_or("r1 нет в уравнении")?;
            let i2 = eq.index_of(&s_field(input, "r2")?).ok_or("r2 нет в уравнении")?;
            let ip = eq.index_of(&s_field(input, "product")?).ok_or("product нет в уравнении")?;
            let (m1, m2) = (f_field(input, "m1_g")?, f_field(input, "m2_g")?);
            let e1 = m1 / eq.molar_mass(i1) / eq.coefs[i1] as f64;
            let e2 = m2 / eq.molar_mass(i2) / eq.coefs[i2] as f64;
            Num(e1.min(e2) * eq.coefs[ip] as f64 * eq.molar_mass(ip))
        }
        "limiting_reagent_volume" => {
            let eq = parse_equation(&s_field(input, "equation")?)?;
            let i1 = eq.index_of(&s_field(input, "r1")?).ok_or("r1 нет в уравнении")?;
            let i2 = eq.index_of(&s_field(input, "r2")?).ok_or("r2 нет в уравнении")?;
            let ip = eq.index_of(&s_field(input, "product")?).ok_or("product нет в уравнении")?;
            let n1 = qty_to_moles(&eq, i1, f_field(input, "q1")?, input.get("q1_is_volume").and_then(|v| v.as_bool()).unwrap_or(false))?;
            let n2 = qty_to_moles(&eq, i2, f_field(input, "q2")?, input.get("q2_is_volume").and_then(|v| v.as_bool()).unwrap_or(false))?;
            let e1 = n1 / eq.coefs[i1] as f64;
            let e2 = n2 / eq.coefs[i2] as f64;
            Num(e1.min(e2) * eq.coefs[ip] as f64 * eq.molar_mass(ip))
        }
        "yield" => {
            let eq = parse_equation(&s_field(input, "equation")?)?;
            let (i_from, i_to) = eq_indices(&eq, &s_field(input, "from")?, &s_field(input, "to")?)?;
            let theo = eq.convert(i_from, f_field(input, "mass_g")?, true, i_to) * eq.molar_mass(i_to);
            Num(f_field(input, "actual_g")? / theo * 100.0)
        }
        "yield_inverse" => {
            let eq = parse_equation(&s_field(input, "equation")?)?;
            let (i_from, i_to) = eq_indices(&eq, &s_field(input, "from")?, &s_field(input, "to")?)?;
            let eta = f_field(input, "yield_pct")?;
            if eta <= 0.0 || eta > 100.0 {
                return Err(format!("выход {eta}% вне (0; 100]"));
            }
            let n_to = f_field(input, "mass_g")? / eq.molar_mass(i_to);
            let n_from = n_to * eq.coefs[i_from] as f64 / eq.coefs[i_to] as f64 / (eta / 100.0);
            Num(n_from * eq.molar_mass(i_from))
        }
        "impurity" => {
            let eq = parse_equation(&s_field(input, "equation")?)?;
            let i_raw = eq.index_of(&s_field(input, "raw")?).ok_or("raw нет в уравнении")?;
            let i_to = eq.index_of(&s_field(input, "to")?).ok_or("to нет в уравнении")?;
            let m_pure = f_field(input, "mass_g")? * f_field(input, "purity_pct")? / 100.0;
            Num(eq.convert(i_raw, m_pure, true, i_to) * eq.molar_mass(i_to))
        }
        "impurity_yield" => {
            let eq = parse_equation(&s_field(input, "equation")?)?;
            let i_raw = eq.index_of(&s_field(input, "raw")?).ok_or("raw нет в уравнении")?;
            let i_to = eq.index_of(&s_field(input, "to")?).ok_or("to нет в уравнении")?;
            let m_pure = f_field(input, "mass_g")? * f_field(input, "purity_pct")? / 100.0;
            let eta = f_field(input, "yield_pct")?;
            Num(eq.convert(i_raw, m_pure, true, i_to) * eq.molar_mass(i_to) * eta / 100.0)
        }
        "formula_from_ratio" => {
            let els = input
                .get("elements")
                .and_then(|x| x.as_array())
                .ok_or("поле «elements» отсутствует")?;
            let mut zs: Vec<u8> = Vec::with_capacity(els.len());
            let mut ws: Vec<f64> = Vec::with_capacity(els.len());
            for e in els {
                zs.push(z_field(e, "element")?);
                ws.push(f_field(e, "w_pct")?);
            }
            let ns: Vec<f64> = ws
                .iter()
                .zip(&zs)
                .map(|(&w, &z)| w / 100.0 / periodic::by_z(z).unwrap().mass)
                .collect();
            let mn = ns.iter().cloned().fold(f64::INFINITY, f64::min);
            let ratio: Vec<f64> = ns.iter().map(|&n| n / mn).collect();
            let mut k: u32 = 0;
            for kk in 1..=12u32 {
                if ratio
                    .iter()
                    .all(|&r| (r * kk as f64 - (r * kk as f64).round()).abs() < 0.1)
                {
                    k = kk;
                    break;
                }
            }
            if k == 0 {
                return Err("не удаётся рационализировать отношения".into());
            }
            let mut out = String::new();
            for (&z, &r) in zs.iter().zip(&ratio) {
                let c = (r * k as f64).round() as u64;
                out.push_str(periodic::by_z(z).unwrap().symbol);
                if c > 1 {
                    out.push_str(&c.to_string());
                }
            }
            Str(out)
        }

        // ── 8 класс: растворы ──
        "solution_w" => {
            let (s, w) = (f_field(input, "solute_g")?, f_field(input, "water_g")?);
            Num(s / (s + w) * 100.0)
        }
        "solution_add_water" => {
            let (m, w, add) = (
                f_field(input, "m_g")?,
                f_field(input, "w_pct")?,
                f_field(input, "add_water_g")?,
            );
            Num(m * w / 100.0 / (m + add) * 100.0)
        }
        "solution_add_solute" => {
            let (m, w, add) = (
                f_field(input, "m_g")?,
                f_field(input, "w_pct")?,
                f_field(input, "add_solute_g")?,
            );
            Num((m * w / 100.0 + add) / (m + add) * 100.0)
        }
        "hydrate_water" => Num(hydrate_water_pct(&s_field(input, "formula")?)?),

        // ── 9 класс: концентрации, скорость ──
        "molarity" => {
            let f = parse_formula(&s_field(input, "formula")?)?;
            Num(f_field(input, "mass_g")? / f.molar_mass() / (f_field(input, "volume_ml")? / 1000.0))
        }
        "molarity_dilution" => {
            Num(f_field(input, "c1")? * f_field(input, "v1_ml")? / f_field(input, "v2_ml")?)
        }
        "molarity_mass" => {
            let f = parse_formula(&s_field(input, "formula")?)?;
            Num(f_field(input, "c")? * f_field(input, "volume_ml")? / 1000.0 * f.molar_mass())
        }
        "van_hoff" => {
            let (g, dt) = (f_field(input, "gamma")?, f_field(input, "dt_c")?);
            if g <= 0.0 {
                return Err("температурный коэффициент должен быть > 0".into());
            }
            Num(g.powf(dt / 10.0))
        }
        "staged_yield" => {
            let ys = yields_list(input)?;
            let mut total = 1.0;
            for y in ys {
                if !(0.0..=100.0).contains(&y) {
                    return Err(format!("выход стадии {y}% вне [0; 100]"));
                }
                total *= y / 100.0;
            }
            Num(total * 100.0)
        }
        other => return Err(format!("неизвестный тип задачи «{other}»")),
    };
    Ok(ans)
}

fn gcd64(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a.abs()
}

fn eq_indices(
    eq: &super::stoich::Equation,
    from: &str,
    to: &str,
) -> Result<(usize, usize), String> {
    let i = eq.index_of(from).ok_or_else(|| format!("«{from}» нет в уравнении"))?;
    let j = eq.index_of(to).ok_or_else(|| format!("«{to}» нет в уравнении"))?;
    Ok((i, j))
}

fn qty_to_moles(
    eq: &super::stoich::Equation,
    idx: usize,
    qty: f64,
    is_volume: bool,
) -> Result<f64, String> {
    if is_volume {
        Ok(stoich::moles_from_volume(qty))
    } else {
        Ok(qty / eq.molar_mass(idx))
    }
}

/// Решить задачу целиком.
pub fn solve_task(t: &Task) -> Result<TaskAnswer, String> {
    solve(&t.kind, &t.input)
}

// ─── Прогон класса / отчёт ────────────────────────────────────────────────

/// Провал одной задачи.
#[derive(Debug, Clone)]
pub struct Failure {
    pub id: String,
    pub topic: String,
    pub reason: String,
}

/// Отчёт по классу (или по всему корпусу, grade = 0).
#[derive(Debug, Clone)]
pub struct GradeReport {
    pub grade: u8,
    pub total: usize,
    pub passed: usize,
    pub failures: Vec<Failure>,
}

/// Прогон: solve по структурированному вводу (эталонный режим) либо
/// через разбор текста (режим SciVoice — проверка понимания
/// формулировок).
pub fn run_grade(grade: Option<u8>, from_text: bool) -> Vec<GradeReport> {
    let mut grades: Vec<u8> = Vec::new();
    for t in tasks() {
        if let Some(g) = grade {
            if t.grade != g {
                continue;
            }
        }
        if !grades.contains(&t.grade) {
            grades.push(t.grade);
        }
    }
    grades.sort_unstable();
    grades
        .into_iter()
        .map(|g| run_one(g, from_text))
        .collect()
}

fn run_one(grade: u8, from_text: bool) -> GradeReport {
    let mut rep = GradeReport { grade, total: 0, passed: 0, failures: Vec::new() };
    for t in tasks() {
        if t.grade != grade {
            continue;
        }
        rep.total += 1;
        let result = if from_text {
            super::curriculum_nl::parse_task_text(&t.text)
                .map_err(|e| format!("текст не разобран: {e}"))
                .and_then(|(kind, input)| solve(&kind, &input))
        } else {
            solve_task(t)
        };
        let result = result_task(t, result);
        match result {
            Ok(()) => rep.passed += 1,
            Err(reason) => rep.failures.push(Failure {
                id: t.id.clone(),
                topic: t.topic.clone(),
                reason,
            }),
        }
    }
    rep
}

fn result_task(t: &Task, res: Result<TaskAnswer, String>) -> Result<(), String> {
    let ans = res?;
    ans.matches(t)
}

/// Текстовый отчёт для CLI.
pub fn report_text(reps: &[GradeReport], from_text: bool) -> String {
    let mode = if from_text { "SciVoice: текст → решение" } else { "структурированный ввод" };
    let mut out = String::new();
    out.push_str(&format!(
        "═══ Задачник «Curriculum Learning» 5–9 класс — режим: {mode} ═══\n"
    ));
    let (mut total, mut passed) = (0usize, 0usize);
    for r in reps {
        let mark = if r.failures.is_empty() { "✓" } else { "✗" };
        out.push_str(&format!(
            "{mark} Класс {}: {}/{} ({:.1}%)\n",
            r.grade,
            r.passed,
            r.total,
            if r.total > 0 { r.passed as f64 * 100.0 / r.total as f64 } else { 0.0 }
        ));
        total += r.total;
        passed += r.passed;
        for f in &r.failures {
            out.push_str(&format!("    · {} [{}]: {}\n", f.id, f.topic, f.reason));
        }
    }
    out.push_str(&format!(
        "═══ Итого: {}/{} задач ({:.1}%) ═══\n",
        passed,
        total,
        if total > 0 { passed as f64 * 100.0 / total as f64 } else { 0.0 }
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_loads() {
        let ts = tasks();
        assert_eq!(ts.len(), 187, "размер корпуса");
        assert!(ts.iter().all(|t| !t.text.is_empty()));
        assert!(ts.iter().all(|t| t.answer.is_some() || t.answer_str.is_some()));
        let grades: Vec<u8> = {
            let mut g: Vec<u8> = ts.iter().map(|t| t.grade).collect();
            g.sort_unstable();
            g.dedup();
            g
        };
        assert_eq!(grades, vec![5, 6, 7, 8, 9]);
    }

    #[test]
    fn corpus_full_solve() {
        // Каждая задача корпуса = строгий юнит-тест (структурированный ввод)
        let mut fails: Vec<String> = Vec::new();
        for t in tasks() {
            if let Err(e) = solve_task(t).and_then(|a| a.matches(t).map(|_| a)) {
                fails.push(e);
            }
        }
        assert!(
            fails.is_empty(),
            "провалено {} задач:\n{}",
            fails.len(),
            fails.join("\n")
        );
    }

    #[test]
    fn corpus_tight_crosscheck() {
        // Двойная запись: Rust против Python-эталонов — числовое ядро
        // совпадает сильно точнее школьного допуска (fp-порядок операций
        // даёт ~1e-9, а не 0.8%)
        let mut worst = 0.0f64;
        for t in tasks() {
            if let (Some(exp), Ok(TaskAnswer::Num(got))) = (t.answer, solve_task(t)) {
                let rel = if exp != 0.0 { (got - exp).abs() / exp.abs() } else { got.abs() };
                worst = worst.max(rel);
                assert!(
                    rel < 1e-9,
                    "{}: Rust {got:.12} vs Python {exp:.12} (rel {rel:.3e})",
                    t.id
                );
            }
        }
        assert!(worst < 1e-9);
    }

    #[test]
    fn van_hoff_and_stages() {
        let inp = |json: &str| serde_json::from_str::<serde_json::Value>(json).unwrap();
        let a = solve("van_hoff", &inp(r#"{"gamma": 2, "dt_c": 30}"#)).unwrap();
        assert_eq!(a, TaskAnswer::Num(8.0));
        // 0.8×0.9×100 в fp = 72.000…01 — сравнение с допуском, не бит-в-бит
        let b = solve("staged_yield", &inp(r#"{"yields": [80, 90]}"#)).unwrap();
        match b {
            TaskAnswer::Num(v) => assert!((v - 72.0).abs() < 1e-9, "staged = {v}"),
            _ => panic!("ожидалось число"),
        }
    }

    #[test]
    fn valence_formula() {
        let inp = |json: &str| serde_json::from_str::<serde_json::Value>(json).unwrap();
        let a = solve(
            "formula_from_valence",
            &inp(r#"{"a": "Al", "a_valence": 3, "b": "O", "b_valence": 2}"#),
        )
        .unwrap();
        assert_eq!(a, TaskAnswer::Str("Al2O3".into()));
        let b = solve(
            "formula_from_valence",
            &inp(r#"{"a": "Mg", "a_valence": 2, "b": "N", "b_valence": 3}"#),
        )
        .unwrap();
        assert_eq!(b, TaskAnswer::Str("Mg3N2".into()));
    }

    #[test]
    fn hydrate_water_share() {
        let w = hydrate_water_pct("CuSO4·5H2O").unwrap();
        assert!((w - 36.0).abs() < 0.3, "w(H2O) = {w}");
        assert!(hydrate_water_pct("CuSO4").is_err());
    }
}
