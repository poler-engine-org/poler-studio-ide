//! Реестр функций калькулятора (цикл M, v0.48.0).
//!
//! РЕГРЕССИИ прошлой сессии, закрытые здесь:
//! - erf: Хорнер с правильными знаками И внешним множителем t
//!   (формула A&S 7.1.26, точность 1.5e-7);
//! - zeta: Эйлер–Маклорен вместо расходящегося ряда (ζ(2) = π²/6 и
//!   ζ(−1) = −1/12 сходятся);
//! - гамма: Ланцрош g=7, n=9.

use super::astro;
use super::geodesy;
use super::logprob;
use super::matrix::Matrix;
use super::numbers;
use super::solve::Complex;
use super::trits::Trits;
use super::units;
use super::units::Unit;
use super::viz;
use super::Value;

// ---------------------------------------------------------------------
// Скалярные помощники
// ---------------------------------------------------------------------

/// Скалярный аргумент: числа и безразмерные величины.
fn scalar_arg(args: &[Value], i: usize) -> Result<f64, String> {
    match args.get(i) {
        Some(Value::Scalar(v)) => Ok(*v),
        Some(Value::Quantity(q, u)) if u.is_dimensionless() => Ok(*q),
        Some(other) => Err(format!("аргумент {} должен быть числом, получено {other}", i + 1)),
        None => Err(format!("не хватает аргументов (нужен аргумент {})", i + 1)),
    }
}

/// Угловой аргумент: число = радианы; величина угловой размерности — конверсия.
fn angle_arg(args: &[Value], i: usize) -> Result<f64, String> {
    match args.get(i) {
        Some(Value::Scalar(v)) => Ok(*v),
        Some(Value::Quantity(q, u)) if u.dim[units::DIM_ANGLE] == 1
            && u.dim.iter().enumerate().all(|(k, &d)| k == units::DIM_ANGLE || d == 0) =>
        {
            let rad = units::by_name("rad").unwrap();
            Ok(units::convert(*q, u, &rad)?)
        }
        Some(other) => Err(format!(
            "аргумент {} должен быть углом (радианы или deg/rad/turn): {other}",
            i + 1
        )),
        None => Err(format!("не хватает аргументов (нужен аргумент {})", i + 1)),
    }
}

/// Числовой аргумент с СИ-конверсией: Scalar либо Quantity в базовых
/// единицах СИ (`3600 s`, `20 W`, `310 K` — работают). Сессия-13.
fn num_arg_si(args: &[Value], i: usize, name: &str, what: &str) -> Result<f64, String> {
    match args.get(i) {
        Some(Value::Scalar(v)) => Ok(*v),
        Some(q @ Value::Quantity(..)) => q
            .as_f64()
            .ok_or_else(|| format!("{name}: {what} — величина без числового значения")),
        Some(other) => Err(format!("{name}: {what} — число или величина, получено {other}")),
        None => Err(format!("{name}: не хватает аргументов ({what})")),
    }
}

fn need(args: &[Value], n: usize, name: &str) -> Result<(), String> {
    if args.len() != n {
        Err(format!(
            "{name}: ожидается {} аргумент(ов), получено {}",
            n,
            args.len()
        ))
    } else {
        Ok(())
    }
}

/// Seed мира (сессия-14): целое ≥ 0 (или умолчание). Мир детерминирован:
/// один seed — одна Этерия.
fn seed_arg(args: &[Value], i: usize, default: u64) -> Result<u64, String> {
    match args.get(i) {
        None => Ok(default),
        Some(Value::Scalar(v)) if *v >= 0.0 && *v <= 9.0e15 && v.fract() == 0.0 => Ok(*v as u64),
        Some(Value::Quantity(..)) => args[i]
            .as_f64()
            .filter(|v| *v >= 0.0 && v.fract() == 0.0)
            .map(|v| v as u64)
            .ok_or_else(|| "seed — целое ≥ 0".to_string()),
        Some(other) => Err(format!("seed — целое ≥ 0, получено {other}")),
    }
}

fn one_arg(args: &[Value], name: &str) -> Result<f64, String> {
    need(args, 1, name)?;
    scalar_arg(args, 0)
}

fn matrix_arg(args: &[Value], i: usize) -> Result<Matrix, String> {
    match args.get(i) {
        Some(Value::Matrix(m)) => Ok(m.clone()),
        Some(other) => Err(format!("аргумент {} должен быть матрицей, получено {other}", i + 1)),
        None => Err("не хватает аргументов".into()),
    }
}

/// Кольцо/точка: матрица N×2 (N=1 — точка, N=2 — отрезок, ≥3 — кольцо,
/// замыкается автоматически). Возвращает f64-координаты.
fn ring_arg(args: &[Value], i: usize, name: &str) -> Result<Vec<(f64, f64)>, String> {
    let m = match args.get(i) {
        Some(Value::Matrix(m)) => m.clone(),
        Some(other) => {
            return Err(format!(
                "{name}: аргумент {} — матрица Nx2 (кольцо/точка), получено {other}",
                i + 1
            ))
        }
        None => return Err(format!("{name}: не хватает аргументов")),
    };
    if m.cols != 2 || m.rows < 1 {
        return Err(format!(
            "{name}: матрица {}×{} — нужно Nx2 (x, y в столбцах)",
            m.rows, m.cols
        ));
    }
    let mut pts = Vec::with_capacity(m.rows);
    for r in 0..m.rows {
        pts.push((m.get(r, 0).re, m.get(r, 1).re));
    }
    Ok(pts)
}

/// f64-кольцо → тритные точки решётки 3⁻ᵏ.
fn quantize_ring(ring: &[(f64, f64)], k: u8) -> Result<Vec<crate::geo::trit_coord::TritPoint>, String> {
    ring.iter()
        .map(|&(x, y)| {
            crate::geo::trit_coord::TritPoint::quantize(x, y, k)
                .map_err(|e| format!("квантование ({x}, {y}): {e}"))
        })
        .collect()
}

/// Полигон с дырками (сессия-16): матрица Nx2 (кольцо без дырок) либо
/// список [внешнее, дырка1, …] из `poly(…)` — в тритных точках решётки.
fn polygon_arg(
    args: &[Value],
    i: usize,
    name: &str,
    k: u8,
) -> Result<Vec<Vec<crate::geo::trit_coord::TritPoint>>, String> {
    let rings_f: Vec<Vec<(f64, f64)>> = match args.get(i) {
        Some(Value::Matrix(m)) => {
            if m.cols != 2 || m.rows < 1 {
                return Err(format!(
                    "{name}: полигон — матрица Nx2 либо poly(кольца), получено {}×{}",
                    m.rows, m.cols
                ));
            }
            vec![(0..m.rows).map(|r| (m.get(r, 0).re, m.get(r, 1).re)).collect()]
        }
        Some(Value::List(items)) if !items.is_empty() => {
            let mut out = Vec::with_capacity(items.len());
            for (j, it) in items.iter().enumerate() {
                match it {
                    Value::Matrix(m) if m.cols == 2 && m.rows >= 1 => {
                        out.push(
                            (0..m.rows)
                                .map(|r| (m.get(r, 0).re, m.get(r, 1).re))
                                .collect(),
                        );
                    }
                    other => {
                        return Err(format!(
                            "{name}: кольцо {} полигона — матрица Nx2, получено {other}",
                            j + 1
                        ))
                    }
                }
            }
            out
        }
        Some(other) => {
            return Err(format!(
                "{name}: полигон — кольцо Nx2 или poly(кольца), получено {other}"
            ))
        }
        None => return Err(format!("{name}: не хватает аргументов")),
    };
    rings_f
        .iter()
        .map(|r| quantize_ring(r, k).map_err(|e| format!("{name}: {e}")))
        .collect()
}

/// Геометрия смешанной размерности (сессия-17): разбор значения в
/// [`crate::geo::de9im::Geometry`] на решётке k.
///
/// Распознавание: матрица 1×2 — точка; 2×2 — отрезок-линия; ≥3×2 —
/// кольцо-полигон (конвенция `de9im`); `line(…)` — открытая ломаная;
/// `poly(…)`/список матриц — полигон с дырками; `gc(…)` — коллекция
/// (вложенность допустима — разбирается рекурсивно).
fn parse_geometry_value(
    v: &Value,
    k: u8,
    name: &str,
) -> Result<crate::geo::de9im::Geometry, String> {
    use crate::geo::de9im::Geometry;
    match v {
        Value::Matrix(m) => {
            if m.cols != 2 || m.rows < 1 {
                return Err(format!(
                    "{name}: геометрия — матрица Nx2, получено {}×{}",
                    m.rows, m.cols
                ));
            }
            let mut pts = Vec::with_capacity(m.rows);
            for r in 0..m.rows {
                pts.push((m.get(r, 0).re, m.get(r, 1).re));
            }
            let pts = quantize_ring(&pts, k).map_err(|e| format!("{name}: {e}"))?;
            match m.rows {
                1 => Ok(Geometry::Point(pts[0].clone())),
                2 => Ok(Geometry::Line(pts)),
                _ => Ok(Geometry::Polygon(vec![pts])),
            }
        }
        Value::List(items) if !items.is_empty() => match items.first() {
            Some(Value::Str(tag)) if tag == "line" => {
                if items.len() != 2 {
                    return Err(format!(
                        "{name}: line(…) — помеченный список [\"line\", матрица]"
                    ));
                }
                match &items[1] {
                    Value::Matrix(m) if m.cols == 2 && m.rows >= 2 => {
                        let mut pts = Vec::with_capacity(m.rows);
                        for r in 0..m.rows {
                            pts.push((m.get(r, 0).re, m.get(r, 1).re));
                        }
                        Ok(Geometry::Line(
                            quantize_ring(&pts, k).map_err(|e| format!("{name}: {e}"))?,
                        ))
                    }
                    other => Err(format!(
                        "{name}: line(…) — матрица Nx2 (≥ 2 вершин), получено {other}"
                    )),
                }
            }
            Some(Value::Str(tag)) if tag == "gc" => {
                let mut comps = Vec::with_capacity(items.len() - 1);
                for it in &items[1..] {
                    comps.push(parse_geometry_value(it, k, name)?);
                }
                if comps.is_empty() {
                    return Err(format!("{name}: пустая коллекция"));
                }
                Ok(Geometry::Collection(comps))
            }
            Some(Value::Matrix(_)) => {
                // poly(…): список колец (первое — внешнее)
                let mut rings = Vec::with_capacity(items.len());
                for (j, it) in items.iter().enumerate() {
                    match it {
                        Value::Matrix(m) if m.cols == 2 && m.rows >= 3 => {
                            let mut pts = Vec::with_capacity(m.rows);
                            for r in 0..m.rows {
                                pts.push((m.get(r, 0).re, m.get(r, 1).re));
                            }
                            rings.push(
                                quantize_ring(&pts, k)
                                    .map_err(|e| format!("{name}: кольцо {}: {e}", j + 1))?,
                            );
                        }
                        other => {
                            return Err(format!(
                                "{name}: кольцо {} полигона — матрица Nx2 (≥ 3), получено {other}",
                                j + 1
                            ))
                        }
                    }
                }
                Ok(Geometry::Polygon(rings))
            }
            _ => Err(format!(
                "{name}: геометрия — матрица Nx2, line(…), poly(…) или gc(…)"
            )),
        },
        other => Err(format!(
            "{name}: геометрия — матрица Nx2, line(…), poly(…) или gc(…), получено {other}"
        )),
    }
}

/// Аргумент-геометрия (сессия-17) для de9im_gc.
fn geometry_arg(
    args: &[Value],
    i: usize,
    name: &str,
    k: u8,
) -> Result<crate::geo::de9im::Geometry, String> {
    match args.get(i) {
        Some(v) => parse_geometry_value(v, k, name),
        None => Err(format!("{name}: не хватает аргументов")),
    }
}

/// Структурная валидация без квантования (для конструктора gc).
fn validate_geometry_shape(v: &Value, name: &str) -> Result<(), String> {
    match v {
        Value::Matrix(m) => {
            if m.cols != 2 || m.rows < 1 {
                return Err(format!(
                    "матрица Nx2 (точка/линия/кольцо), получено {}×{}",
                    m.rows, m.cols
                ));
            }
            Ok(())
        }
        Value::List(items) if !items.is_empty() => match items.first() {
            Some(Value::Str(tag)) if tag == "line" => {
                if items.len() == 2 {
                    match &items[1] {
                        Value::Matrix(m) if m.cols == 2 && m.rows >= 2 => Ok(()),
                        other => Err(format!("line(…) — Nx2 ≥ 2 вершин, получено {other}")),
                    }
                } else {
                    Err("line(…) — помеченный список [\"line\", матрица]".into())
                }
            }
            Some(Value::Str(tag)) if tag == "gc" => {
                if items.len() < 2 {
                    return Err("пустая коллекция".into());
                }
                for it in &items[1..] {
                    validate_geometry_shape(it, name)?;
                }
                Ok(())
            }
            Some(Value::Matrix(_)) => {
                for (j, it) in items.iter().enumerate() {
                    match it {
                        Value::Matrix(m) if m.cols == 2 && m.rows >= 3 => {}
                        other => {
                            return Err(format!(
                                "кольцо {} — матрица Nx2 (≥ 3), получено {other}",
                                j + 1
                            ))
                        }
                    }
                }
                Ok(())
            }
            _ => Err("матрица Nx2, line(…), poly(…) или gc(…)".into()),
        },
        other => Err(format!(
            "матрица Nx2, line(…), poly(…) или gc(…), получено {other}"
        )),
    }
}

/// Описание геометрии для отчёта de9im_gc.
fn describe_geometry(g: &crate::geo::de9im::Geometry) -> String {
    use crate::geo::de9im::Geometry;
    match g {
        Geometry::Point(_) => "точка".into(),
        Geometry::Line(l) => format!("линия ({} вершин)", l.len()),
        Geometry::Polygon(r) => {
            format!("полигон ({} колец, дырок {})", r.len(), r.len() - 1)
        }
        Geometry::Collection(cs) => {
            let (mut pts, mut lns, mut polys, mut colls) = (0usize, 0, 0, 0);
            for c in cs {
                match c {
                    Geometry::Point(_) => pts += 1,
                    Geometry::Line(_) => lns += 1,
                    Geometry::Polygon(_) => polys += 1,
                    Geometry::Collection(_) => colls += 1,
                }
            }
            format!("коллекция [точек {pts} · линий {lns} · полигонов {polys} · коллекций {colls}]")
        }
    }
}

/// Complex → Value: вещественный результат остаётся числом (нулевая
/// регрессия отображения), комплексный — Complex (цикл O).
fn complex_to_value(c: Complex) -> Value {
    if c.im == 0.0 {
        Value::Scalar(c.re)
    } else {
        Value::Complex(c)
    }
}

/// Гибкое число: Scalar | Quantity (в базовых СИ) | Complex (вещественная
/// часть с диагностикуой) — для t и ħ в schrodinger (цикл O).
fn flex_num(args: &[Value], i: usize, name: &str) -> Result<f64, String> {
    match args.get(i) {
        Some(Value::Scalar(v)) => Ok(*v),
        Some(Value::Quantity(q, _)) => Ok(*q), // значение в базовых СИ (hbar → 1.05e-34)
        Some(other) => Err(format!(
            "{name}: аргумент {} должен быть числом, получено {other}",
            i + 1
        )),
        None => Err(format!("{name}: не хватает аргументов")),
    }
}

/// Строковый аргумент (имя планеты, тритная запись).
fn str_arg(args: &[Value], i: usize) -> Result<String, String> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s.clone()),
        Some(other) => Err(format!("аргумент {} должен быть строкой \"…\": {other}", i + 1)),
        None => Err("не хватает аргументов".into()),
    }
}

/// Список чисел целиком (viz_prob/viz_bars сессии-8):
/// скаляры и безразмерные величины; вектор-матрица [a, b, c]
/// (грабля 16: литерал в скобках — матрица-строка).
fn float_list_arg(args: &[Value], i: usize, name: &str) -> Result<Vec<f64>, String> {
    match args.get(i) {
        Some(Value::List(items)) => items
            .iter()
            .map(|x| match x {
                Value::Scalar(v) => Ok(*v),
                Value::Quantity(q, u) if u.is_dimensionless() => Ok(*q),
                other => Err(format!(
                    "{name}: элементы списка должны быть числами, получено {other}"
                )),
            })
            .collect(),
        Some(Value::Matrix(m)) => viz::matrix_as_vector(m).ok_or_else(|| {
            format!(
                "{name}: матрица {}×{} — не вектор; подайте строку [a, b, c] или столбец",
                m.rows, m.cols
            )
        }),
        Some(other) => Err(format!(
            "{name}: аргумент {} должен быть списком [ … ], получено {other}",
            i + 1
        )),
        None => Err("не хватает аргументов".into()),
    }
}

/// Дата (y, m, d[, час UTC]) из аргументов, начиная с индекса i.
fn date_args(args: &[Value], i: usize) -> Result<(i32, u32, u32, f64), String> {
    let y = scalar_arg(args, i)?;
    let m = scalar_arg(args, i + 1)?;
    let d = scalar_arg(args, i + 2)?;
    let h = if args.len() > i + 3 { scalar_arg(args, i + 3)? } else { 12.0 };
    let (yi, mi, di) = (y as i32, m as u32, d as u32);
    if !(1..=12).contains(&mi) || !(1..=31).contains(&di) {
        return Err(format!("некорректная дата {yi}-{mi}-{di}"));
    }
    Ok((yi, mi, di, h))
}

// ---------------------------------------------------------------------
// Специальные функции (тяжёлая математика)
// ---------------------------------------------------------------------

/// Гамма-функция, аппроксимация Ланцроша (g=7, n=9).
/// Точность ~1e-13 в комплексной плоскости |z|<последнего полюса.
pub fn gamma_impl(x: f64) -> Result<f64, String> {
    if x.is_nan() {
        return Err("NaN".into());
    }
    if x == 0.0 || (x < 0.0 && x.fract() == 0.0) {
        // цикл N: полюс — расходимость на расширенной прямой.
        // Вычет в x = −n равен (−1)ⁿ/n! (для анализа — solve вокруг полюса)
        return Ok(f64::INFINITY);
    }
    const G: f64 = 7.0;
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.5203681218851,
        -1259.1392167224028,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507343278686905,
        -0.13857109526572012,
        9.984_369_578_019_572e-6,
        1.5056327351493116e-7,
    ];
    if x < 0.5 {
        // отражение: Γ(z)Γ(1−z) = π/sin(πz)
        Ok(std::f64::consts::PI / ((std::f64::consts::PI * x).sin() * gamma_impl(1.0 - x)?))
    } else {
        let z = x - 1.0;
        let mut acc = C[0];
        for (i, &c) in C.iter().enumerate().skip(1) {
            acc += c / (z + i as f64);
        }
        let t = z + G + 0.5;
        Ok((2.0 * std::f64::consts::PI).sqrt() * t.powf(z + 0.5) * (-t).exp() * acc)
    }
}

/// ln|Γ(x)| — для больших x без переполнения.
pub fn lgamma_impl(x: f64) -> Result<f64, String> {
    if x <= 0.0 && x.fract() == 0.0 {
        return Err("Γ имеет полюса в неположительных целых".into());
    }
    Ok(gamma_impl(x)?.abs().ln())
}

/// Функция ошибок, A&S 7.1.26 (|ε| ≤ 1.5e-7).
/// РЕГРЕССИЯ: Хорнер с правильными знаками и внешним множителем t:
///   erf(x) = 1 − t·(a1 + a2·t + a3·t² + a4·t³ + a5·t⁴)·e^{−x²},
///   t = 1/(1+px).
pub fn erf_impl(x: f64) -> f64 {
    if x == 0.0 {
        return 0.0; // нечётная функция — точно (полином A&S даёт ~1e-9 в нуле)
    }
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let p = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let t = 1.0 / (1.0 + p * x);
    // Хорнер: a1 + t·(a2 + t·(a3 + t·(a4 + t·a5))), затем × t
    let poly = a1 + t * (a2 + t * (a3 + t * (a4 + t * a5)));
    let y = 1.0 - poly * t * (-x * x).exp();
    sign * y
}

/// Дзета-функция Римана: Эйлер–Маклорен для s>1, функциональное
/// уравнение для s<1, тривиальные нули/ζ(0) — точно.
pub fn zeta_impl(s: f64) -> Result<f64, String> {
    if !s.is_finite() {
        return Err("ζ: аргумент должен быть конечен".into());
    }
    if s == 1.0 {
        // гармонический ряд расходится к +∞ — расширенная прямая,
        // в одном стиле с полюсами Γ (gamma(-1) → ∞)
        return Ok(f64::INFINITY);
    }
    // тривиальные нули: s = −2, −4, −6, …
    if s < 0.0 && s.fract() == 0.0 && (s / 2.0).fract() == 0.0 {
        return Ok(0.0);
    }
    if s == 0.0 {
        return Ok(-0.5);
    }
    if s > 1.0 {
        // Эйлер–Маклорен: Σ_{k=1}^{N−1} k^{-s} + N^{1−s}/(s−1) + N^{-s}/2
        //   + s·N^{-s-1}/12 − s(s+1)(s+2)·N^{-s-3}/720
        let nf: f64 = 30.0;
        let mut sum = 0.0;
        for k in 1..30u64 {
            sum += (k as f64).powf(-s);
        }
        sum += nf.powf(1.0 - s) / (s - 1.0);
        sum += 0.5 * nf.powf(-s);
        sum += s * nf.powf(-s - 1.0) / 12.0;
        sum -= s * (s + 1.0) * (s + 2.0) * nf.powf(-s - 3.0) / 720.0;
        Ok(sum)
    } else {
        // s < 1 (кроме 0 и отрицательных чётных): функциональное уравнение
        // ζ(s) = 2^s π^{s−1} sin(πs/2) Γ(1−s) ζ(1−s)
        let g = gamma_impl(1.0 - s)?;
        let z2 = zeta_impl(1.0 - s)?;
        Ok(2.0_f64.powf(s) * std::f64::consts::PI.powf(s - 1.0)
            * (std::f64::consts::PI * s / 2.0).sin()
            * g
            * z2)
    }
}

// ---------------------------------------------------------------------
// Реестр
// ---------------------------------------------------------------------

/// Является ли имя известной функцией (для парсера: вызов vs умножение).
pub fn is_function(name: &str) -> bool {
    // v0.93.0: законы физики — вызываемые функции: pendulum(0.5),
    // rc_tau(1e4, 1e-4), doppler(440, 343, 30). ТОЧНЫЙ id — частичные
    // совпадения запрещены, иначе `kepler(1+2)` станет вызовом.
    if crate::calc::scriptgen::find_law_exact(name).is_some() {
        return true;
    }
    matches!(
        name,
        // математика
        "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2"
        | "sinh" | "cosh" | "tanh" | "asinh" | "acosh" | "atanh"
        | "sind" | "cosd" | "tand" | "atan2d"
        | "exp" | "ln" | "log" | "log2" | "log10" | "sqrt" | "cbrt" | "hypot" | "pow"
        | "abs" | "floor" | "ceil" | "round" | "sign" | "fract"
        | "deg2rad" | "rad2deg" | "min" | "max" | "mean" | "median" | "std" | "clamp" | "linspace"
        // специальная
        | "gamma" | "lgamma" | "erf" | "erfc" | "zeta" | "beta"
        // теория чисел
        | "gcd" | "lcm" | "mod" | "is_prime" | "next_prime" | "prev_prime"
        | "factorize" | "divisors" | "fib" | "binomial" | "catalan"
        | "factorial" | "fact"
        // физика (цикл N)
        | "lorentz"
        // матрицы/квант
        | "transpose" | "det" | "inv" | "pinv" | "trace" | "expm" | "charpoly" | "eigen"
        | "eigen_sturm"
        | "identity" | "rot2" | "rotx" | "roty" | "rotz" | "so_gen"
        // квант цикла O: Шрёдингер
        | "schrodinger" | "dagger" | "kron" | "tridiag" | "eye"
        | "pauli_x" | "pauli_y" | "pauli_z" | "hadamard"
        // триты
        | "trits" | "trit_val" | "trit_and" | "trit_or" | "trit_not"
        // визуализация (цикл U, сессия-8)
        | "viz" | "viz_bell" | "viz_scale" | "viz_prob" | "viz_bars" | "viz_matrix"
        // суверенный рендер (сессия-9)
        | "viz_graph" | "viz_field" | "viz_surf"
        // GIS-ядро на тритах (сессия-12)
        | "viz_iso3" | "de9im" | "geo_pred"
        // сверхдиапазонные вероятности (сессия-13/14)
        | "logpow" | "regress" | "logp" | "expneg" | "lpsum"
        // переплетение модулей (сессия-14): 27-дерево, гексы, Этерия
        | "spatial27" | "hexring" | "hexdisk" | "hexdist" | "hexid"
        | "de9im_line" | "eteria_field" | "eteria_hex" | "eteria_boltz"
        | "viz_hex"
        // муха над картой (сессия-15): TIN, res>0, Line×Line
        | "tin" | "eteria_tin" | "h3children" | "h3parent" | "de9im_ll"
        // кристалл рельефа (сессия-16): ускорение BW, viz_tin, полигоны с дырками
        | "eteria_tin_pts" | "viz_tin" | "poly" | "de9im_poly"
        // три горизонта (сессия-17): изолинии TIN, H3 res→2, GeometryCollection
        | "tin_isolines" | "viz_isolines" | "h3addr" | "h3cell"
        | "line" | "gc" | "de9im_gc"
        // единицы/температура
        | "degC" | "degF"
        // астрономия
        | "sun_lon" | "sun_ra" | "sun_dec"
        | "moon_lon" | "moon_lat" | "moon_dist" | "moon_phase" | "moon_illum"
        | "moon_age" | "moon_ra" | "moon_dec"
        | "planet_lon" | "planet_dist" | "sunrise" | "sunset"
        // квантовая химия (118 элементов, архетипы, тритные связи, формулы)
        | "chem_elem" | "chem_bond" | "chem_mass" | "chem_formula" | "chem_archetype"
        // голос учёного (сессия-18): роторы + триединое ядро
        | "speak"
        // аб-иницио квантовая химия (v0.73.0): RHF/STO-3G из POLER-ERI
        | "hf"
        // лестница молекул (v0.74.0): SMILES → 3D → био-контур
        | "chem_smiles" | "chem_3d" | "bio_eval"
        // белки и докинг (v0.75.0): PDB → карман → гибкая поза
        | "pdb_info" | "dock"
        // log-домен в матрицах (v0.74.0): logdet/lsolve за пределами f64
        | "logdet" | "lsolve"
        // геодезия/навигация
        | "dist" | "bearing" | "midpoint" | "dest" | "earth_radius"
    )
}



// ─── log-домен в матрицах (v0.74.0) ─────────────────────────────────────

const LN_3_F64: f64 = 1.0986122886681098;

/// Отчёт logdet: знак, ln|det|, log10|det|, тритная глубина, обусловленность.
fn logdet_report(m: &Matrix, sign: f64, ln_abs: f64) -> String {
    let n = m.rows;
    let log10 = ln_abs / std::f64::consts::LN_10;
    // «Честный» f64-детерминант для сравнения
    let f64_det = m.det().ok().and_then(|c| {
        let v = c.re;
        if v.is_finite() {
            Some(v)
        } else {
            None
        }
    });
    let mut s = String::new();
    s.push_str(&format!("══ч log-домен определителя ({n}×{n}) ══\n"));
    s.push_str(&format!(
        "[1] sign = {sign:+.0}, ln|det| = {ln_abs:.4}, log10|det| = {log10:.4} — значение удерживается аналитически, за пределами f64 (~10^±308).\n"
    ));
    match f64_det {
        Some(v) if v != 0.0 => {
            s.push_str(&format!(
                "[2] f64-детерминант: {v:.3e} — влезает в диапазон, лог-домен подтверждает: log10 = {log10:.3}.\n"
            ));
        }
        Some(_) => {
            let d = -ln_abs / LN_3_F64;
            s.push_str(&format!(
                "[2] f64-детерминант: 0.0 (UNDERFLOW!) — обычный det потерял значение; лог-домен держит |det| = 10^({log10:.1}), тритная глубина d = {d:.2} (|det| = 3^(-d)).\n"
            ));
        }
        None => s.push_str("[2] f64-детерминант: переполнение/недоступен — лог-домен единственный путь.\n"),
    }
    if 0.0 < ln_abs && ln_abs.is_finite() {
        s.push_str(&format!(
            "[3] |det| = 3^(-{:.4}) — тритная глубина, та же сверхдиапазонная форма, что и logpow.\n",
            -ln_abs / LN_3_F64
        ));
    }
    match m.cond_inf() {
        Ok(cond) => {
            let verdict = if cond < 1e3 {
                ("+1", "обусловлена отлично — решать можно смело")
            } else if cond < 1e8 {
                ("0", "умеренная обусловленность — следите за невязкой")
            } else {
                ("-1", "близко к сингулярной — решение ненадёжно")
            };
            s.push_str(&format!(
                "[4] Обусловленность cond_∞ = {cond:.3e} → трит-вердикт: {} — {}.\n",
                verdict.0, verdict.1
            ));
        }
        Err(e) => s.push_str(&format!("[4] Обусловленность: LU не сошёлся ({e}).\n")),
    }
    s.push_str("Смежное: lsolve(A, b) — LU-решение с эквилибровкой строк (матрицы с разбросом 1e-12…1e12).");
    s
}

// ─── Лестница молекул (v0.74.0): отчёты chem_smiles / chem_3d ───────────

fn expect_str(v: &Value, name: &str, what: &str) -> Result<String, String> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        other => Err(format!("{name}: ожидалась строка ({what}), получено {other}")),
    }
}

/// Паспорт молекулы из SMILES: стехиометрия, связи, кольца, дескрипторы.
pub fn smiles_passport(g: &crate::chem::MoleculeGraph, input: &str) -> String {
    use crate::chem::{descriptors, logp_estimate};
    let d = descriptors(g);
    let q = crate::chem::gasteiger(g);
    let mut s = String::new();
    s.push_str(&format!("══ч {} ══\n", input.trim()));
    s.push_str(&format!(
        "[1] Стехиометрия: {} · {} тяжёлых атомов, {} всего с H · суммарный заряд {:+} · M = {:.2} г/моль.\n",
        g.hill_formula(),
        g.atoms.len(),
        d.total_atoms,
        g.total_charge(),
        g.atoms.iter().map(|a| match crate::universal_chem::get_element_by_symbol(&a.symbol) {
            Some(e) => e.atomic_mass,
            None => 12.0,
        }).sum::<f64>() + g.atoms.iter().map(|a| a.h_count as f64 * 1.008).sum::<f64>()
    ));
    // Типы связей
    let mut n_single = 0;
    let mut n_double = 0;
    let mut n_triple = 0;
    let mut n_arom = 0;
    for b in &g.bonds {
        match b.order {
            crate::chem::BondOrder::Single => n_single += 1,
            crate::chem::BondOrder::Double => n_double += 1,
            crate::chem::BondOrder::Triple => n_triple += 1,
            crate::chem::BondOrder::Aromatic => n_arom += 1,
            crate::chem::BondOrder::Quadruple => n_single += 1,
        }
    }
    s.push_str(&format!(
        "[2] Топология: {} связей (одинарных {}, двойных {}, тройных {}, ароматических {}), колец {}, из них ароматических {}, вращаемых σ-связей {}.\n",
        g.bonds.len(), n_single, n_double, n_triple, n_arom, d.rings, d.aromatic_rings, d.rotatable
    ));
    if !g.rings.is_empty() {
        let ring_sizes: Vec<String> = g.rings.iter().map(|r| format!("{}", r.len())).collect();
        s.push_str(&format!("    Кольца (SSSR): [{}] члены.\n", ring_sizes.join(", ")));
    }
    // Тритные связи по парам элементов (контур universal_chem)
    let mut bond_types: Vec<(String, i8, f64)> = Vec::new();
        for b in &g.bonds {
            let ea = crate::universal_chem::get_element_by_symbol(&g.atoms[b.a].symbol);
            let eb = crate::universal_chem::get_element_by_symbol(&g.atoms[b.b].symbol);
            if let (Some(ea), Some(eb)) = (ea, eb) {
                let cb = crate::universal_chem::calculate_bond(ea, eb);
                let key = format!("{}-{}", ea.symbol, eb.symbol);
                if !bond_types.iter().any(|(k, _, _)| *k == key) {
                    bond_types.push((key, cb.trit, cb.delta_chi));
                }
            }
        }
    if !bond_types.is_empty() {
        let parts: Vec<String> = bond_types
            .iter()
            .map(|(k, t, dc)| format!("{k} {}", if *t == 1 { "полярная(+1)" } else if *t == -1 { "ионная(-1)" } else { "неполярная(0)" }))
            .collect();
        s.push_str(&format!("[3] Химия связей: {}.\n", parts.join("; ")));
    }
    s.push_str(&format!(
        "[4] Фармакофор: HBD {}, HBA {}, Lipinski-совместимость: {}; logP ≈ {:+.2}.\n",
        d.h_bond_donors,
        d.h_bond_acceptors,
        if d.h_bond_donors <= 5 && d.h_bond_acceptors <= 10 { "проходит" } else { "нарушение" },
        logp_estimate(g)
    ));
    // Заряды Гастайгера: топ-полярные атомы
    let mut polar: Vec<(usize, f64)> = g.atoms.iter().enumerate().map(|(i, _)| (i, q.0[i])).collect();
    polar.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    if polar.len() >= 2 {
        let (imin, qmin) = polar[0];
        let (imax, qmax) = polar[polar.len() - 1];
        s.push_str(&format!(
            "[5] Заряды Гастайгера: δ− на {} ({:+.3}e), δ+ на {} ({:+.3}e) — нуклеофильный и электрофильный центры.\n",
            g.atoms[imin].symbol, qmin, g.atoms[imax].symbol, qmax
        ));
    }
    // Архетипы
    let mut arch_counts = std::collections::BTreeMap::new();
    for a in &g.atoms {
        if let Some(e) = crate::universal_chem::get_element_by_symbol(&a.symbol) {
            *arch_counts.entry(e.archetype.name()).or_insert(0usize) += 1;
        }
    }
    let dominant = arch_counts.iter().max_by_key(|(_, &v)| v).map(|(k, _)| *k).unwrap_or("Mixed");
    s.push_str(&format!(
        "[6] Квантовый архетип: {} (атомов по классам: {}).\n",
        dominant,
        arch_counts.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(", ")
    ));
    s.push_str("Подробнее: chem_3d(\"…\") — 3D-конформер; bio_eval(лиганд, мишень) — био-контур связывания; pdb_info(файл.pdb) — паспорт белка; dock(лиганд, файл.pdb) — гибкий докинг; --view-mol / --view-complex — интерактивные визоры.");
    s
}

/// Отчёт 3D-укладчика.
fn conformer_report(g: &crate::chem::MoleculeGraph, conf: &crate::chem::Conformer, input: &str) -> String {
    let (mn, mx) = conf.bbox();
    let size = [mx[0] - mn[0], mx[1] - mn[1], mx[2] - mn[2]];
    let mut lens: Vec<f64> = conf.bonds.iter().map(|b| {
        let d = [
            conf.positions[b.a][0] - conf.positions[b.b][0],
            conf.positions[b.a][1] - conf.positions[b.b][1],
            conf.positions[b.a][2] - conf.positions[b.b][2],
        ];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    }).collect();
    lens.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let (lmin, lmax) = (lens[0], lens[lens.len() - 1]);
    let lmean = lens.iter().sum::<f64>() / lens.len() as f64;
    let planarity = conf.aromatic_planarity(g);
    let mut s = String::new();
    s.push_str(&format!("══ч 3D-конформер {} ══\n", input.trim()));
    s.push_str(&format!(
        "[1] {} узлов ({} тяжёлых + {} H), {} связей; укладка: кольца-полигоны + BFS-цепи + {} итераций релаксации.\n",
        conf.nodes.len(),
        conf.heavy,
        conf.nodes.len() - conf.heavy,
        conf.bonds.len(),
        if conf.relaxed { "160" } else { "0" }
    ));
    s.push_str(&format!(
        "[2] Длины связей: min {lmin:.3} Å, среднее {lmean:.3} Å, max {lmax:.3} Å (химический коридор 0.9–1.6 для одинарных).\n"
    ));
    s.push_str(&format!(
        "[3] Габариты: {:.2} × {:.2} × {:.2} Å (ВдВ-габарит см. --view-mol режим m).\n",
        size[0], size[1], size[2]
    ));
    if g.rings.iter().any(|r| r.len() <= 6 && r.iter().all(|&i| g.atoms[i].aromatic)) {
        s.push_str(&format!("[4] Планарность ароматических колец: RMS {planarity:.4} Å (бензольные кольца — плоские).\n"));
    }
    // Гибридизации
    let mut hyb_counts = std::collections::BTreeMap::new();
    for i in 0..g.atoms.len() {
        let h = crate::chem::hybridization(g, i);
        *hyb_counts.entry(h.name_ru()).or_insert(0usize) += 1;
    }
    s.push_str(&format!(
        "[5] Гибридизации: {} (углы 109.47°/120°/180° соблюдены релаксацией).\n",
        hyb_counts.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(", ")
    ));
    s.push_str("Первые координаты (Å): ");
    for (i, p) in conf.positions.iter().take(4).enumerate() {
        s.push_str(&format!("{}{}({:+.2},{:+.2},{:+.2})", if i == 0 { "" } else { " " }, conf.nodes[i].symbol, p[0], p[1], p[2]));
    }
    s.push_str("…\n");
    s.push_str("Живая 3D/4D-сцена: poler-engine --view-mol — вращение, зум, ВдВ-оболочки, кручение связей.");
    s
}
/// Вызвать функцию по имени.
pub fn call_function(name: &str, args: &[Value]) -> Result<Value, String> {
    // Списки: скалярные функции применяются поэлементно
    if let Some(Value::List(_)) = args.iter().find(|a| matches!(a, Value::List(_))) {
        if !matches!(
            name,
            "min" | "max" | "mean" | "median" | "std" | "gcd" | "lcm" | "atan2" | "pow"
                | "hypot" | "clamp" | "mod" | "binomial" | "trit_and" | "trit_or"
                | "midpoint" | "dist" | "bearing" | "dest" | "sunrise" | "sunset"
                | "planet_lon" | "planet_dist"
                // viz-функции принимают список целиком (сессия-8)
                | "viz" | "viz_prob" | "viz_bars"
                // стек срезов — целиком (сессия-12)
                | "viz_iso3"
                // сумма ряда — список ЦЕЛИКОМ, а не поэлементно (сессия-14)
                | "lpsum"
                // вектор правой части — список целиком (v0.74.0)
                | "lsolve"
                // окно запроса — список из 6 чисел (сессия-14)
                | "spatial27"
                // полигон с дырками — список колец целиком (сессия-16)
                | "poly" | "de9im_poly"
                // коллекции и уровни-списки — целиком (сессия-17)
                | "gc" | "de9im_gc" | "tin_isolines" | "viz_isolines"
        ) {
            return map_over_lists(name, args);
        }
    }

    // v0.93.0: законы физики — прямые вызовы из выражений (Фаза 3
    // протокола POLER): calc pendulum(0.5) · calc rc_tau(1e4, 1e-4) ·
    // calc doppler(440, 343, 30). Ветка — ПОСЛЕ list-маппинга:
    // pendulum([0.5, 1, 2]) свипает параметр поэлементно.
    if let Some(law) = crate::calc::scriptgen::find_law_exact(name) {
        return law_eval(law, args);
    }

    match name {
        // ---------- тригонометрия ----------
        "sin" => {
            // цикл O: sin(x+iy) = sin x·cosh y + i·cos x·sinh y
            match args {
                [Value::Complex(c)] => Ok(Value::Complex(Complex::new(
                    c.re.sin() * c.im.cosh(),
                    c.re.cos() * c.im.sinh(),
                ))),
                _ => Ok(Value::Scalar(angle_arg(args, 0)?.sin())),
            }
        }
        "cos" => {
            // цикл O: cos(x+iy) = cos x·cosh y − i·sin x·sinh y
            match args {
                [Value::Complex(c)] => Ok(Value::Complex(Complex::new(
                    c.re.cos() * c.im.cosh(),
                    -(c.re.sin() * c.im.sinh()),
                ))),
                _ => Ok(Value::Scalar(angle_arg(args, 0)?.cos())),
            }
        }
        "tan" => Ok(Value::Scalar(angle_arg(args, 0)?.tan())),
        "asin" => {
            // цикл N: |x| > 1 — комплексная ветвь (DLMF 4.23):
            // asin(x) = π/2 − i·acosh(x) при x > 1; нечётность при x < −1
            let x = one_arg(args, name)?;
            if x > 1.0 {
                Ok(Value::Complex(Complex::new(
                    std::f64::consts::FRAC_PI_2,
                    -(x + (x * x - 1.0).sqrt()).ln(),
                )))
            } else if x < -1.0 {
                Ok(Value::Complex(Complex::new(
                    -std::f64::consts::FRAC_PI_2,
                    (-x + (x * x - 1.0).sqrt()).ln(),
                )))
            } else {
                Ok(Value::Scalar(x.asin()))
            }
        }
        "acos" => {
            // цикл N: acos(x) = i·acosh(x) при x > 1; π − i·acosh(−x) при x < −1
            let x = one_arg(args, name)?;
            if x > 1.0 {
                Ok(Value::Complex(Complex::new(
                    0.0,
                    (x + (x * x - 1.0).sqrt()).ln(),
                )))
            } else if x < -1.0 {
                Ok(Value::Complex(Complex::new(
                    std::f64::consts::PI,
                    -(-x + (x * x - 1.0).sqrt()).ln(),
                )))
            } else {
                Ok(Value::Scalar(x.acos()))
            }
        }
        "atan" => Ok(Value::Scalar(one_arg(args, name)?.atan())),
        "atan2" => {
            need(args, 2, name)?;
            Ok(Value::Scalar(scalar_arg(args, 0)?.atan2(scalar_arg(args, 1)?)))
        }
        "sinh" => Ok(Value::Scalar(one_arg(args, name)?.sinh())),
        "cosh" => Ok(Value::Scalar(one_arg(args, name)?.cosh())),
        "tanh" => Ok(Value::Scalar(one_arg(args, name)?.tanh())),
        "asinh" => Ok(Value::Scalar(one_arg(args, name)?.asinh())),
        "acosh" => {
            // цикл N: x < 1 — комплексная ветвь acosh(x) = i·acos(x)
            let x = one_arg(args, name)?;
            if x < 1.0 {
                // acos(x) для x<−1 сам комплексный: acosh = i·acos(x)
                if x < -1.0 {
                    let a = (-x + (x * x - 1.0).sqrt()).ln();
                    // acos(x) = π − i·a → i·acos = i·π + a
                    return Ok(Value::Complex(Complex::new(a, std::f64::consts::PI)));
                }
                return Ok(Value::Complex(Complex::new(0.0, x.acos())));
            }
            Ok(Value::Scalar(x.acosh()))
        }
        "atanh" => {
            // цикл N: |x| ≥ 1 — расширенный результат (DLMF 4.37):
            // x=±1 → ±∞; x>1 → ½ln((x+1)/(x−1)) − i·π/2; x<−1 — нечётно
            let x = one_arg(args, name)?;
            if x == 1.0 {
                return Ok(Value::Scalar(f64::INFINITY));
            }
            if x == -1.0 {
                return Ok(Value::Scalar(f64::NEG_INFINITY));
            }
            if x > 1.0 {
                Ok(Value::Complex(Complex::new(
                    0.5 * ((x + 1.0) / (x - 1.0)).ln(),
                    -std::f64::consts::FRAC_PI_2,
                )))
            } else if x < -1.0 {
                Ok(Value::Complex(Complex::new(
                    0.5 * ((-x - 1.0) / (1.0 - x)).ln(),
                    std::f64::consts::FRAC_PI_2,
                )))
            } else {
                Ok(Value::Scalar(x.atanh()))
            }
        }
        "sind" => Ok(Value::Scalar(one_arg(args, name)?.to_radians().sin())),
        "cosd" => Ok(Value::Scalar(one_arg(args, name)?.to_radians().cos())),
        "tand" => Ok(Value::Scalar(one_arg(args, name)?.to_radians().tan())),
        "atan2d" => {
            need(args, 2, name)?;
            Ok(Value::Scalar(
                scalar_arg(args, 0)?.atan2(scalar_arg(args, 1)?).to_degrees(),
            ))
        }

        // ---------- экспоненты/логарифмы ----------
        "exp" => {
            // цикл O: экспонента комплексного — тождество Эйлера
            // exp(iπ) = −1; e^{x+iy} = e^x·(cos y + i·sin y)
            match args {
                [Value::Complex(c)] => {
                    Ok(Value::Complex(Complex::from_polar(c.re.exp(), c.im)))
                }
                _ => Ok(Value::Scalar(one_arg(args, name)?.exp())),
            }
        }
        "ln" => {
            // цикл N: расширенный логарифм — ln(0) = −∞ (предел),
            // ln(x<0) = ln|x| + iπ (главная ветвь)
            // сессия-14: ln(лог-вероятности) — ln P как обычное число
            if let Some(Value::LogProb(p)) = args.first() {
                return Ok(Value::Scalar(p.ln()));
            }
            let x = one_arg(args, name)?;
            if x == 0.0 {
                return Ok(Value::Scalar(f64::NEG_INFINITY));
            }
            if x < 0.0 {
                return Ok(Value::Complex(Complex::new(
                    (-x).ln(),
                    std::f64::consts::PI,
                )));
            }
            Ok(Value::Scalar(x.ln()))
        }
        "log" => {
            // log(x) = log10 (калькуляторная конвенция); log(x, b) — по основанию b
            // цикл N: x ≤ 0 — расширенно (0 → −∞; отрицательное → комплексно)
            match args.len() {
                1 => {
                    let x = scalar_arg(args, 0)?;
                    if x == 0.0 {
                        return Ok(Value::Scalar(f64::NEG_INFINITY));
                    }
                    if x < 0.0 {
                        let l = Complex::new((-x).ln(), std::f64::consts::PI);
                        return Ok(Value::Complex(Complex::new(
                            l.re / std::f64::consts::LN_10,
                            l.im / std::f64::consts::LN_10,
                        )));
                    }
                    Ok(Value::Scalar(x.log10()))
                }
                2 => {
                    let (x, b) = (scalar_arg(args, 0)?, scalar_arg(args, 1)?);
                    if x <= 0.0 || b <= 0.0 || b == 1.0 {
                        return Err("log(x, b): x > 0, b > 0, b ≠ 1".into());
                    }
                    Ok(Value::Scalar(x.log(b)))
                }
                n => Err(format!("log: 1 или 2 аргумента, получено {n}")),
            }
        }
        "log2" => {
            if let Some(Value::LogProb(p)) = args.first() {
                return Ok(Value::Scalar(p.ln() / std::f64::consts::LN_2));
            }
            let x = one_arg(args, name)?;
            if x == 0.0 {
                return Ok(Value::Scalar(f64::NEG_INFINITY));
            }
            if x < 0.0 {
                let l = Complex::new((-x).ln(), std::f64::consts::PI);
                return Ok(Value::Complex(Complex::new(
                    l.re / std::f64::consts::LN_2,
                    l.im / std::f64::consts::LN_2,
                )));
            }
            Ok(Value::Scalar(x.log2()))
        }
        "log10" => {
            if let Some(Value::LogProb(p)) = args.first() {
                return Ok(Value::Scalar(p.log10()));
            }
            let x = one_arg(args, name)?;
            if x == 0.0 {
                return Ok(Value::Scalar(f64::NEG_INFINITY));
            }
            if x < 0.0 {
                return Ok(Value::Complex(Complex::new(
                    (-x).log10(),
                    std::f64::consts::PI / std::f64::consts::LN_10,
                )));
            }
            Ok(Value::Scalar(x.log10()))
        }
        "sqrt" => {
            // √ размерной величины: размерности обязаны быть чётными
            // (√(s²) = s, √(m²/s²) = m/s) — нужно законам Кеплера и др.
            if let Some(Value::Quantity(q, u)) = args.first() {
                if u.dim.iter().all(|&dd| (dd as i32) % 2 == 0) {
                    let mut dim = [0i8; units::NDIM];
                    for (i, &dd) in u.dim.iter().enumerate() {
                        dim[i] = dd / 2;
                    }
                    let unit = Unit { dim, factor: u.factor.sqrt(), name: "", offset: 0.0 };
                    return Ok(Value::Quantity(q.sqrt(), unit));
                }
                return Err("sqrt: размерности величины должны быть чётными (√(s²)=s ок, √(m) — нет)".into());
            }
            // цикл N: sqrt(x<0) = i·√|x| — тахионный множитель Лоренца
            // и любая мнимая ось без обходного пути через solve
            let x = one_arg(args, name)?;
            if x < 0.0 {
                return Ok(Value::Complex(Complex::new(0.0, (-x).sqrt())));
            }
            Ok(Value::Scalar(x.sqrt()))
        }
        "cbrt" => Ok(Value::Scalar(one_arg(args, name)?.cbrt())),
        "hypot" => {
            need(args, 2, name)?;
            Ok(Value::Scalar(scalar_arg(args, 0)?.hypot(scalar_arg(args, 1)?)))
        }
        "pow" => {
            need(args, 2, name)?;
            Ok(Value::Scalar(scalar_arg(args, 0)?.powf(scalar_arg(args, 1)?)))
        }

        // ---------- округление/знак ----------
        "abs" => {
            // цикл O: модуль комплексного |3+4i| = 5 и амплитуды 1×1
            // (квантовая вероятность: abs(⟨1|ψ⟩)^2)
            // ГРАБЛИ сессии-4 (исправлены): abs был только скалярным — теперь
            // вектор (1×n / n×1) даёт евклидову норму ‖v‖₂, общая матрица —
            // норму Фробениуса ‖A‖_F = sqrt(Σ|a_ij|²): одна формула на оба случая
            match args {
                [Value::Complex(c)] => Ok(Value::Scalar(c.abs())),
                [Value::Matrix(m)] => {
                    if m.rows == 1 && m.cols == 1 {
                        Ok(Value::Scalar(m.get(0, 0).abs()))
                    } else {
                        let s = m.data.iter().map(|c| c.re * c.re + c.im * c.im).sum::<f64>();
                        Ok(Value::Scalar(s.sqrt()))
                    }
                }
                _ => Ok(Value::Scalar(one_arg(args, name)?.abs())),
            }
        }
        "floor" => Ok(Value::Scalar(one_arg(args, name)?.floor())),
        "ceil" => Ok(Value::Scalar(one_arg(args, name)?.ceil())),
        "round" => Ok(Value::Scalar(one_arg(args, name)?.round())),
        // ГРАБЛИ сессии-4 (исправлены): sign(0) → 0 (мат-конвенция знака);
        // f64::signum отдаёт 1.0 для +0.0 (и −1.0 для −0.0) — на нулевых
        // траекториях CGLMP-сканов это ломало симметрию разностей
        "sign" => {
            let x = one_arg(args, name)?;
            Ok(Value::Scalar(if x == 0.0 { 0.0 } else { x.signum() }))
        }
        "fract" => Ok(Value::Scalar(one_arg(args, name)?.fract())),
        "deg2rad" => Ok(Value::Scalar(one_arg(args, name)?.to_radians())),
        "rad2deg" => Ok(Value::Scalar(one_arg(args, name)?.to_degrees())),

        // ---------- агрегаты (вариативные) ----------
        "min" | "max" => {
            if args.is_empty() {
                return Err(format!("{name}: нужен хотя бы один аргумент"));
            }
            let mut best = scalar_arg(args, 0)?;
            for i in 1..args.len() {
                let v = scalar_arg(args, i)?;
                best = if name == "min" { best.min(v) } else { best.max(v) };
            }
            Ok(Value::Scalar(best))
        }
        "mean" | "median" | "std" => {
            if args.is_empty() {
                return Err(format!("{name}: нужен хотя бы один аргумент"));
            }
            let mut xs: Vec<f64> = Vec::with_capacity(args.len());
            for i in 0..args.len() {
                xs.push(scalar_arg(args, i)?);
            }
            match name {
                "mean" => Ok(Value::Scalar(xs.iter().sum::<f64>() / xs.len() as f64)),
                "median" => {
                    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    let n = xs.len();
                    Ok(Value::Scalar(if n % 2 == 1 {
                        xs[n / 2]
                    } else {
                        (xs[n / 2 - 1] + xs[n / 2]) / 2.0
                    }))
                }
                _ => {
                    let m = xs.iter().sum::<f64>() / xs.len() as f64;
                    let var = xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / xs.len() as f64;
                    Ok(Value::Scalar(var.sqrt()))
                }
            }
        }
        "clamp" => {
            need(args, 3, name)?;
            let (x, lo, hi) = (scalar_arg(args, 0)?, scalar_arg(args, 1)?, scalar_arg(args, 2)?);
            Ok(Value::Scalar(x.clamp(lo, hi)))
        }

        // ---------- траектории (v0.93.0, Фаза 3 протокола POLER) ----------
        "linspace" => {
            need(args, 3, name)?;
            let n = scalar_arg(args, 2)?;
            if !(n >= 1.0 && n.fract() == 0.0 && n <= 65_536.0) {
                return Err(format!(
                    "linspace(a, b, n): n — целое 1..=65536 (точек траектории), получено {n}"
                ));
            }
            let n = n as usize;
            let a = args[0].clone();
            let b = args[1].clone();
            // только числа и величины — остальное внятная ошибка
            match (&a, &b) {
                (Value::Scalar(_), Value::Scalar(_))
                | (Value::Scalar(_), Value::Quantity(..))
                | (Value::Quantity(..), Value::Scalar(_))
                | (Value::Quantity(..), Value::Quantity(..)) => {}
                _ => {
                    return Err(
                        "linspace(a, b, n): a и b — числа или величины (список — outside)".into()
                    )
                }
            }
            if n == 1 {
                return Ok(Value::List(vec![a]));
            }
            // (b − a)/(n − 1) — один раз; единицы сохраняются арифметикой
            // значений (linspace(0 m, 1 m, 5) → величины в метрах)
            let span = crate::calc::parser::binary_op(crate::calc::parser::BinOp::Sub, &b, &a)?;
            let step = crate::calc::parser::binary_op(
                crate::calc::parser::BinOp::Div,
                &span,
                &Value::Scalar((n - 1) as f64),
            )?;
            let mut out = Vec::with_capacity(n);
            out.push(a.clone());
            for i in 1..n - 1 {
                let fr = crate::calc::parser::binary_op(
                    crate::calc::parser::BinOp::Mul,
                    &step,
                    &Value::Scalar(i as f64),
                )?;
                out.push(crate::calc::parser::binary_op(
                    crate::calc::parser::BinOp::Add,
                    &a,
                    &fr,
                )?);
            }
            // концы — точно a и b: никакого дрейфа накопления шага
            out.push(b.clone());
            Ok(Value::List(out))
        }

        // ---------- специальная математика ----------
        "gamma" => Ok(Value::Scalar(gamma_impl(one_arg(args, name)?)?)),
        "lgamma" => Ok(Value::Scalar(lgamma_impl(one_arg(args, name)?)?)),
        "erf" => Ok(Value::Scalar(erf_impl(one_arg(args, name)?))),
        "erfc" => Ok(Value::Scalar(1.0 - erf_impl(one_arg(args, name)?))),
        "zeta" => Ok(Value::Scalar(zeta_impl(one_arg(args, name)?)?)),
        "beta" => {
            need(args, 2, name)?;
            let (a, b) = (scalar_arg(args, 0)?, scalar_arg(args, 1)?);
            Ok(Value::Scalar(
                gamma_impl(a)? * gamma_impl(b)? / gamma_impl(a + b)?,
            ))
        }

        // ---------- теория чисел ----------
        "gcd" => {
            need(args, 2, name)?;
            Ok(Value::Scalar(numbers::gcd(scalar_arg(args, 0)?, scalar_arg(args, 1)?)?))
        }
        "lcm" => {
            need(args, 2, name)?;
            Ok(Value::Scalar(numbers::lcm(scalar_arg(args, 0)?, scalar_arg(args, 1)?)?))
        }
        "mod" => {
            need(args, 2, name)?;
            Ok(Value::Scalar(
                scalar_arg(args, 0)?.rem_euclid(scalar_arg(args, 1)?),
            ))
        }
        "is_prime" => Ok(Value::Scalar(
            if numbers::is_prime(one_arg(args, name)?)? { 1.0 } else { 0.0 },
        )),
        "next_prime" => Ok(Value::Scalar(numbers::next_prime(one_arg(args, name)?)?)),
        "prev_prime" => Ok(Value::Scalar(numbers::prev_prime(one_arg(args, name)?)?)),
        "factorize" => {
            let fs = numbers::factorize(one_arg(args, name)?)?;
            let mut out: Vec<Value> = Vec::new();
            for (p, k) in fs {
                out.push(Value::Scalar(p));
                out.push(Value::Scalar(k));
            }
            Ok(Value::List(out))
        }
        "divisors" => {
            let ds = numbers::divisors(one_arg(args, name)?)?;
            Ok(Value::List(ds.into_iter().map(Value::Scalar).collect()))
        }
        "fib" => {
            // цикл N: точный big-путь за f64-пределом (F(78))
            let n = one_arg(args, name)?;
            if !n.is_finite() || n.fract() != 0.0 {
                return Err("fib: ожидается целое".into());
            }
            if n < 0.0 {
                return Err("fib: ожидается неотрицательное (реализация без негафибоначчи)".into());
            }
            if n <= 78.0 {
                Ok(Value::Scalar(numbers::fib(n)?))
            } else {
                Ok(Value::BigInt(numbers::fib_big(n as u64)?))
            }
        }
        "binomial" => {
            // цикл N: точный big-путь (C(100,50) уже неточен в f64)
            need(args, 2, name)?;
            let (n, k) = (scalar_arg(args, 0)?, scalar_arg(args, 1)?);
            if n.fract() != 0.0 || k.fract() != 0.0 {
                return Err("binomial: ожидается целые".into());
            }
            if n < 0.0 || k < 0.0 {
                return Err("binomial: n, k ≥ 0".into());
            }
            if k > n {
                return Ok(Value::Scalar(0.0));
            }
            if n > 10_000.0 {
                return Err("binomial: n ≤ 10 000 (точный big-путь)".into());
            }
            let s = numbers::binomial_big(n as u64, k as u64)?;
            // малые результаты — Scalar (совместимость), гиганты — точное целое
            if let Ok(iv) = s.parse::<i64>() {
                if iv.abs() <= 9_007_199_254_740_992 {
                    return Ok(Value::Scalar(iv as f64));
                }
            }
            Ok(Value::BigInt(s))
        }
        "catalan" => {
            // цикл N: точный big-путь (C_n = C(2n,n)/(n+1), деление нацело)
            let n = one_arg(args, name)?;
            if n.fract() != 0.0 || n < 0.0 {
                return Err("catalan: ожидается целое n ≥ 0".into());
            }
            if n > 5_000.0 {
                return Err("catalan: n ≤ 5 000 (точный big-путь)".into());
            }
            let b = numbers::binomial_big(2 * n as u64, n as u64)?;
            let s = numbers::big_div_small(&b, n as u64 + 1);
            if let Ok(iv) = s.parse::<i64>() {
                if iv.abs() <= 9_007_199_254_740_992 {
                    return Ok(Value::Scalar(iv as f64));
                }
            }
            Ok(Value::BigInt(s))
        }
        "factorial" | "fact" => {
            // цикл N: точный факториал (18! — потолок f64-точности)
            let n = one_arg(args, name)?;
            if !n.is_finite() || n.fract() != 0.0 || n < 0.0 {
                return Err("factorial: ожидается целое n ≥ 0".into());
            }
            if n <= 18.0 {
                let r: u64 = (2..=n as u64).product();
                return Ok(Value::Scalar(r as f64));
            }
            Ok(Value::BigInt(numbers::factorial_big(n as u64)?))
        }
        "lorentz" => {
            // цикл N: γ(v) = 1/√(1−v²), v в долях c.
            // v < c → вещественная γ; v = c → ∞ (расходимость);
            // v > c → комплексная γ = −i/√(v²−1) — тахионная ветвь
            // (мнимое собственное время).
            let v = one_arg(args, name)?;
            let s = 1.0 - v * v;
            if s > 0.0 {
                Ok(Value::Scalar(1.0 / s.sqrt()))
            } else if s == 0.0 {
                Ok(Value::Scalar(f64::INFINITY))
            } else {
                Ok(Value::Complex(Complex::new(0.0, -1.0 / (-s).sqrt())))
            }
        }

        // ---------- матрицы/квант ----------
        "transpose" => Ok(Value::Matrix(matrix_arg(args, 0)?.transpose())),
        "det" => Ok(complex_to_value(matrix_arg(args, 0)?.det()?)),
        "inv" => Ok(Value::Matrix(matrix_arg(args, 0)?.inv()?)),
        "pinv" => Ok(Value::Matrix(matrix_arg(args, 0)?.pinv()?)),
        "trace" => Ok(complex_to_value(matrix_arg(args, 0)?.trace()?)),
        "expm" => Ok(Value::Matrix(matrix_arg(args, 0)?.expm()?)),
        "charpoly" => {
            let c = matrix_arg(args, 0)?.charpoly()?;
            Ok(Value::List(c.into_iter().map(complex_to_value).collect()))
        }
        "eigen" => {
            let ev = matrix_arg(args, 0)?.eigenvalues()?;
            Ok(Value::List(ev.into_iter().map(complex_to_value).collect()))
        }
        "eigen_sturm" => {
            // Штурм-бисекция: без charpoly — иммунен к взрыву Уилкинсона (n>=28);
            // симметричные вещественные, машинная точность при n=256+
            let ev = matrix_arg(args, 0)?.eigenvalues_sturm()?;
            Ok(Value::List(ev.into_iter().map(complex_to_value).collect()))
        }
        "identity" => {
            let n = one_arg(args, name)?;
            if n.fract() != 0.0 || !(1.0..=12.0).contains(&n) {
                return Err("identity(n): n — целое 1..=12".into());
            }
            Ok(Value::Matrix(Matrix::identity(n as usize)))
        }
        // eye — алиас identity; ГРАБЛИ сессии-4 (исправлены): искусственный
        // лимит 64 душил кутрит-конвейеры (eye(81), eye(729) для 3⁶ состояний,
        // eye(2187) для 3⁷). Потолок 4096: dense n² × 16 байт, eye(4096) ≈ 268 МБ;
        // крупнее — собирайте kron'ом, память всё равно упрётся раньше
        "eye" => {
            let n = one_arg(args, name)?;
            if n.fract() != 0.0 || !(1.0..=4096.0).contains(&n) {
                return Err("eye(n): n — целое 1..=4096 (4096²·16 байт ≈ 268 МБ dense)".into());
            }
            Ok(Value::Matrix(Matrix::identity(n as usize)))
        }
        "rot2" => Ok(Value::Matrix(Matrix::rot2(angle_arg(args, 0)?))),
        "rotx" => Ok(Value::Matrix(Matrix::rot_x(angle_arg(args, 0)?))),
        "roty" => Ok(Value::Matrix(Matrix::rot_y(angle_arg(args, 0)?))),
        "rotz" => Ok(Value::Matrix(Matrix::rot_z(angle_arg(args, 0)?))),
        "so_gen" => {
            need(args, 3, name)?;
            let (n, i, j) = (scalar_arg(args, 0)?, scalar_arg(args, 1)?, scalar_arg(args, 2)?);
            if n.fract() != 0.0 || i.fract() != 0.0 || j.fract() != 0.0 {
                return Err("so_gen(n, i, j): целые аргументы".into());
            }
            Ok(Value::Matrix(Matrix::so_generator(n as usize, i as usize, j as usize)?))
        }

        // ---------- квант цикла O: Шрёдингер ----------
        "schrodinger" => {
            // |Ψ(t)⟩ = expm(−i·H·t/ħ)·|Ψ₀⟩ — решатель уравнения Шрёдингера.
            // ħ = 1 по умолчанию (натуральные единицы); для СИ передайте
            // четвёртым аргументом hbar (или t уже в t/ħ).
            if args.len() != 3 && args.len() != 4 {
                return Err(
                    "schrodinger(H, psi0, t[, hbar]): 3 или 4 аргумента (по умолчанию ħ = 1)"
                        .into(),
                );
            }
            let h = matrix_arg(args, 0)?;
            let psi0 = matrix_arg(args, 1)?;
            let t = flex_num(args, 2, name)?;
            let hbar = if args.len() == 4 { flex_num(args, 3, name)? } else { 1.0 };
            if !hbar.is_finite() || hbar == 0.0 {
                return Err("schrodinger: ħ должен быть конечным и ненулевым".into());
            }
            if !h.is_square() {
                return Err("schrodinger: гамильтониан H — квадратная матрица".into());
            }
            if psi0.cols != 1 || psi0.rows != h.rows {
                return Err(format!(
                    "schrodinger: psi0 — вектор-столбец {}×1, получено {}×{}",
                    h.rows, psi0.rows, psi0.cols
                ));
            }
            let u = h.scale_c(Complex::new(0.0, -t / hbar)).expm()?;
            Ok(Value::Matrix(u.mul(&psi0)?))
        }
        "dagger" => Ok(Value::Matrix(matrix_arg(args, 0)?.dagger())),
        "kron" => {
            need(args, 2, name)?;
            Ok(Value::Matrix(matrix_arg(args, 0)?.kron(&matrix_arg(args, 1)?)))
        }
        "pauli_x" => {
            need(args, 0, name)?;
            Ok(Value::Matrix(Matrix::from_rows(&[vec![0.0, 1.0], vec![1.0, 0.0]])?))
        }
        "pauli_y" => {
            need(args, 0, name)?;
            Ok(Value::Matrix(Matrix::from_complex_rows(&[
                vec![Complex::ZERO, Complex::new(0.0, -1.0)],
                vec![Complex::new(0.0, 1.0), Complex::ZERO],
            ])?))
        }
        "pauli_z" => {
            need(args, 0, name)?;
            Ok(Value::Matrix(Matrix::from_rows(&[vec![1.0, 0.0], vec![0.0, -1.0]])?))
        }
        "hadamard" => {
            need(args, 0, name)?;
            let k = std::f64::consts::FRAC_1_SQRT_2;
            Ok(Value::Matrix(Matrix::from_rows(&[vec![k, k], vec![k, -k]])?))
        }
        "tridiag" => {
            // трёхдиагональная матрица: d на диагонали, off на соседях —
            // дискретизация лапласиана (яма, осциллятор на сетке).
            // v0.62 (AI-API-реформа): лимит поднят 64 -> 512 — безопасно для
            // det (LU, O(n^3)) и eigen_sturm (Штурм, машинная точность при
            // n=256+); eigen/charpoly ломаются на n>=28 ВСЕГДА (Уилкинсон),
            // независимо от лимита билдера.
            need(args, 3, name)?;
            let (d, off, n) = (
                scalar_arg(args, 0)?,
                scalar_arg(args, 1)?,
                scalar_arg(args, 2)?,
            );
            if n.fract() != 0.0 || !(2.0..=512.0).contains(&n) {
                return Err("tridiag(d, off, n): n — целое 2..=512".into());
            }
            let n = n as usize;
            let mut m = Matrix::zeros(n, n);
            for i in 0..n {
                m.set(i, i, Complex::new(d, 0.0));
            }
            for i in 0..n - 1 {
                m.set(i, i + 1, Complex::new(off, 0.0));
                m.set(i + 1, i, Complex::new(off, 0.0));
            }
            Ok(Value::Matrix(m))
        }

        // ---------- визуализация (цикл U, сессия-8) ----------
        "viz" => {
            need(args, 1, name)?;
            Ok(Value::Str(viz::auto_svg(&args[0])?))
        }
        "viz_bell" => {
            let i = one_arg(args, name)?;
            Ok(Value::Str(viz::bell_svg(i)))
        }
        "viz_scale" => {
            // Метры: скаляр по конвенции либо величина длиновой
            // размерности (конверсия через factor к базовой).
            need(args, 1, name)?;
            let x_m = match args.get(0) {
                Some(Value::Scalar(x)) => *x,
                Some(Value::Quantity(q, u))
                    if u.dim[0] == 1 && u.dim[1..].iter().all(|d| *d == 0) =>
                {
                    q * u.factor
                }
                Some(other) => {
                    return Err(format!(
                        "viz_scale: аргумент 1 должен быть числом (метры) или длиной, \
                         получено {other}"
                    ))
                }
                None => return Err("viz_scale: не хватает аргументов".into()),
            };
            Ok(Value::Str(viz::scale_svg(x_m)?))
        }
        "viz_prob" => {
            let ps = float_list_arg(args, 0, name)?;
            Ok(Value::Str(viz::prob_svg(&ps)?))
        }
        "viz_bars" => {
            let vs = float_list_arg(args, 0, name)?;
            Ok(Value::Str(viz::bars_svg(&vs)?))
        }
        "viz_matrix" => {
            let m = matrix_arg(args, 0)?;
            Ok(Value::Str(viz::matrix_svg(&m)?))
        }
        // ---------- суверенный рендер (сессия-9) ----------
        "viz_graph" => {
            // Смежность + опциональные метки ОДНОЙ строкой через запятую
            // (грабля 16: литерала списка строк в языке нет).
            if args.is_empty() || args.len() > 2 {
                return Err(format!(
                    "viz_graph: ожидается 1–2 аргумента, получено {}",
                    args.len()
                ));
            }
            let m = matrix_arg(args, 0)?;
            let labels = match args.get(1) {
                None => None,
                Some(Value::Str(s)) => Some(
                    s.split(',')
                        .map(|t| t.trim().to_string())
                        .collect::<Vec<_>>(),
                ),
                Some(other) => {
                    return Err(format!(
                        "viz_graph: метки — строка через запятую \
                         (\"имя1, имя2, …\"), получено {other}"
                    ))
                }
            };
            Ok(Value::Str(viz::graph_svg(&m, labels.as_deref())?))
        }
        "viz_field" => {
            let m = matrix_arg(args, 0)?;
            Ok(Value::Str(viz::field_svg(&m)?))
        }
        "viz_surf" => {
            let m = matrix_arg(args, 0)?;
            Ok(Value::Str(viz::surf_svg(&m)?))
        }
        // ---------- GIS-ядро на тритах (сессия-12) ----------
        "viz_iso3" => {
            // Стек z-срезов: список матриц (список строится функциями —
            // литерала списка матриц в языке нет; либо одна матрица
            // (rows = nz·ny, блоки строк — срезы), либо список).
            let level = match args.get(1) {
                None => None,
                Some(Value::Scalar(v)) => Some(*v),
                Some(Value::Quantity(q, u)) if u.is_dimensionless() => Some(*q),
                Some(other) => {
                    return Err(format!(
                        "viz_iso3: уровень — число, получено {other}"
                    ))
                }
            };
            let slices: Vec<Matrix> = match args.get(0) {
                Some(Value::List(items)) => {
                    let mut ms = Vec::with_capacity(items.len());
                    for (i, it) in items.iter().enumerate() {
                        match it {
                            Value::Matrix(m) => ms.push(m.clone()),
                            other => {
                                return Err(format!(
                                    "viz_iso3: срез {i} должен быть матрицей, \
                                     получено {other}"
                                ))
                            }
                        }
                    }
                    ms
                }
                Some(Value::Matrix(m)) => {
                    // Одна матрица (nz·ny)×nx: nz блоков по ny строк.
                    if m.rows % m.cols != 0 || m.rows / m.cols < 2 {
                        return Err(format!(
                            "viz_iso3: матрица {}×{} не разбивается на куб \
                             (rows должны быть nz·cols)",
                            m.rows, m.cols
                        ));
                    }
                    let ny = m.cols;
                    let nz = m.rows / ny;
                    let mut ms = Vec::with_capacity(nz);
                    for k in 0..nz {
                        let mut plane = Matrix::zeros(ny, m.cols);
                        for i in 0..ny {
                            for j in 0..m.cols {
                                plane.set(i, j, m.get(k * ny + i, j));
                            }
                        }
                        ms.push(plane);
                    }
                    ms
                }
                Some(other) => {
                    return Err(format!(
                        "viz_iso3: аргумент 1 — список матриц или блочная \
                         матрица, получено {other}"
                    ))
                }
                None => return Err("viz_iso3: не хватает аргументов".into()),
            };
            Ok(Value::Str(viz::iso3_svg(&slices, level)?))
        }
        "de9im" => {
            // relate(a, b [, k]): кольца — матрицы Nx2; точка — 1x2.
            let a = ring_arg(args, 0, "de9im")?;
            let b = ring_arg(args, 1, "de9im")?;
            let k = match args.get(2) {
                None => 6u8,
                Some(Value::Scalar(v)) if *v >= 0.0 && *v <= 12.0 => *v as u8,
                Some(other) => {
                    return Err(format!(
                        "de9im: масштаб k ∈ [0, 12] — число, получено {other}"
                    ))
                }
            };
            let qa = quantize_ring(&a, k)?;
            let qb = quantize_ring(&b, k)?;
            let m = crate::geo::de9im::relate(&qa, &qb)
                .map_err(|e| format!("de9im: {e}"))?;
            let preds = m.predicate_names().join(", ");
            Ok(Value::Str(format!(
                "{} · код {} · предикаты: {}",
                m.bal_string(),
                m.trit_code(),
                if preds.is_empty() { "—" } else { &preds }
            )))
        }
        "geo_pred" => {
            // Список сработавших предикатов relate(a, b [, k]).
            let a = ring_arg(args, 0, "geo_pred")?;
            let b = ring_arg(args, 1, "geo_pred")?;
            let k = match args.get(2) {
                None => 6u8,
                Some(Value::Scalar(v)) if *v >= 0.0 && *v <= 12.0 => *v as u8,
                Some(other) => {
                    return Err(format!(
                        "geo_pred: масштаб k ∈ [0, 12] — число, получено {other}"
                    ))
                }
            };
            let qa = quantize_ring(&a, k)?;
            let qb = quantize_ring(&b, k)?;
            let m = crate::geo::de9im::relate(&qa, &qb)
                .map_err(|e| format!("geo_pred: {e}"))?;
            Ok(Value::List(
                m.predicate_names()
                    .into_iter()
                    .map(|p| Value::Str(p.to_string()))
                    .collect(),
            ))
        }
        // ---------- сверхдиапазонные вероятности (сессия-13/14) ----------
        "logpow" => {
            // logpow(b, x): b^x, когда результат за пределами f64 —
            // сессия-14: полноценное значение-лог-вероятность: участвует
            // в +−·/^ и рядах lpsum; Display показывает 10^(…) и 3^(…).
            need(args, 2, name)?;
            let b = scalar_arg(args, 0)?;
            let x = scalar_arg(args, 1)?;
            Ok(Value::LogProb(logprob::LogProb::powf(b, x)?))
        }
        "logp" => {
            // logp(p): поднять представимое число в лог-домен — вход в
            // сверхмалый мир (logp(0.5) + logpow(2, -1e15) работает).
            let x = num_arg_si(args, 0, name, "вероятность p > 0")?;
            Ok(Value::LogProb(logprob::LogProb::from_f64(x)?))
        }
        "expneg" => {
            // expneg(x) = e^(−x), x ≥ 0 — больцмановский/круксовский
            // штраф как лог-вероятность (ΔS/k_B ~ 1e30 — за пределами f64).
            let x = num_arg_si(args, 0, name, "показатель x ≥ 0")?;
            Ok(Value::LogProb(logprob::LogProb::exp_neg(x)?))
        }
        "lpsum" => {
            // lpsum(p1, p2, …) / lpsum([p1, …]) — сумма ряда вероятностей
            // в лог-домене (Z = Σ P_i): члены ~ 10^(−10^5) не выпадают в 0.
            let items: Vec<Value> = match args.first() {
                Some(Value::List(items)) if args.len() == 1 => items.clone(),
                _ => args.to_vec(),
            };
            if items.is_empty() {
                return Err("lpsum: пустой ряд — нужны члены (> 0)".into());
            }
            let mut lns: Vec<f64> = Vec::with_capacity(items.len());
            for (i, it) in items.iter().enumerate() {
                match it {
                    Value::LogProb(p) => lns.push(p.ln()),
                    Value::Scalar(v) => {
                        lns.push(logprob::LogProb::from_f64(*v).map_err(|e| {
                            format!("lpsum: член {} — {e}", i + 1)
                        })?.ln())
                    }
                    other => {
                        return Err(format!(
                            "lpsum: член {} — лог-вероятность или число > 0, получено {}",
                            i + 1,
                            other.type_name()
                        ))
                    }
                }
            }
            Ok(Value::LogProb(logprob::lpsum_lns(&lns)?))
        }
        "regress" => {
            // regress(N_бит, Δt_с[, Вт, К]): P(опыт из будущего | регрессия
            // в прошлое) = 2^(-N) и P(регрессия) = e^(-ΔS/k_B) — полный
            // отчёт с тритными глубинами и бюджетом Вселенной.
            if args.len() < 2 || args.len() > 4 {
                return Err("regress: 2–4 аргумента: (N_бит, Δt_с[, Вт, К])".into());
            }
            let n = num_arg_si(args, 0, name, "число бит опыта N")?;
            let dt = num_arg_si(args, 1, name, "глубина регрессии Δt, с")?;
            let watts = match args.get(2) {
                None => 20.0,
                Some(_) => num_arg_si(args, 2, name, "мощность мозга, Вт")?,
            };
            let kelvin = match args.get(3) {
                None => 310.15,
                Some(_) => num_arg_si(args, 3, name, "температура мозга, К")?,
            };
            Ok(Value::Str(logprob::regress_report(n, dt, watts, kelvin)?))
        }

        // ---------- переплетение модулей (сессия-14) ----------
        "spatial27" => {
            // spatial27(конверты Nx6, окно[, k]): 27-дерево (тритное 3×3×3)
            // против октодерева — пространственный индекс XYZ на тритах.
            // Ответ — список номеров строк (1-based), чьи конверты
            // пересекают окно. Конверт: [minx,miny,minz,maxx,maxy,maxz].
            let boxes_m = matrix_arg(args, 0)?;
            if boxes_m.cols != 6 || boxes_m.rows < 1 {
                return Err(format!(
                    "spatial27: конверты — матрица Nx6 [minx,miny,minz,maxx,maxy,maxz], \
                     получено {}×{}",
                    boxes_m.rows, boxes_m.cols
                ));
            }
            // окно: список из 6 чисел ИЛИ матрица 1×6
            let qnums: Vec<f64> = match args.get(1) {
                Some(Value::List(items)) if items.len() == 6 => items
                    .iter()
                    .map(|v| v.as_f64().ok_or_else(|| "spatial27: окно — 6 чисел".to_string()))
                    .collect::<Result<_, _>>()?,
                Some(Value::Matrix(m)) if m.cols == 6 && m.rows == 1 => {
                    (0..6).map(|j| m.get(0, j).re).collect()
                }
                other => {
                    return Err(format!(
                        "spatial27: окно — список из 6 чисел [minx,miny,minz,maxx,maxy,maxz], \
                         получено {:?}",
                        other.map(|v| v.type_name())
                    ))
                }
            };
            let k = match args.get(2) {
                None => 6u8,
                Some(Value::Scalar(v)) if *v >= 0.0 && *v <= 12.0 => *v as u8,
                Some(other) => {
                    return Err(format!("spatial27: k ∈ [0, 12], получено {other}"))
                }
            };
            let lat = crate::geo::trit_coord::Lattice::new(k)?;
            let qi = |v: f64| -> Result<i64, String> { Ok(lat.quantize(v)?.to_i64()) };
            let mut bboxes = Vec::with_capacity(boxes_m.rows);
            let mut bounds_min = [i64::MAX; 3];
            let mut bounds_max = [i64::MIN; 3];
            for r in 0..boxes_m.rows {
                let c: Vec<i64> = (0..6)
                    .map(|j| qi(boxes_m.get(r, j).re))
                    .collect::<Result<_, _>>()?;
                for a in 0..3 {
                    bounds_min[a] = bounds_min[a].min(c[a]);
                    bounds_max[a] = bounds_max[a].max(c[a + 3]);
                }
                bboxes.push(crate::geo::tree27::BBox3::new(
                    c[0], c[1], c[2], c[3], c[4], c[5],
                ));
            }
            let q = crate::geo::tree27::BBox3::new(
                qi(qnums[0])?,
                qi(qnums[1])?,
                qi(qnums[2])?,
                qi(qnums[3])?,
                qi(qnums[4])?,
                qi(qnums[5])?,
            );
            let bounds = crate::geo::tree27::BBox3::new(
                bounds_min[0], bounds_min[1], bounds_min[2],
                bounds_max[0], bounds_max[1], bounds_max[2],
            );
            let mut tree = crate::geo::tree27::Tree27::new(bounds, 8);
            for (i, b) in bboxes.iter().enumerate() {
                tree.insert(*b, i).map_err(|e| format!("spatial27: {e}"))?;
            }
            Ok(Value::List(
                tree.query(&q)
                    .into_iter()
                    .map(|i| Value::Scalar(i as f64 + 1.0)) // 1-based
                    .collect(),
            ))
        }
        "hexring" | "hexdisk" => {
            // hexring(k) / hexdisk(r): гексы трит-спирали (мини-H3).
            let r = num_arg_si(args, 0, name, "радиус кольца/диска")?;
            if !(0.0..=13.0).contains(&r) || r.fract() != 0.0 {
                return Err(format!(
                    "hexring/hexdisk: радиус — целое ∈ [0, 13] (6-тритная спираль), получено {r}"
                ));
            }
            let cells = if name == "hexring" {
                crate::geo::hexgrid::hex_ring(r as i64)
            } else {
                crate::geo::hexgrid::hex_disk(r as i64)
            };
            if cells.is_empty() {
                return Err("hexring: кольцо 0 пусто — радиус ≥ 1 (диск 0 = центр)".into());
            }
            let rows: Vec<Vec<f64>> = cells.iter().map(|&(q, r)| vec![q as f64, r as f64]).collect();
            Ok(Value::Matrix(Matrix::from_rows(&rows)?))
        }
        "hexdist" => {
            // hexdist(q1, r1, q2, r2): гекс-дистанция (|Δq|+|Δr|+|Δq+Δr|)/2.
            need(args, 4, name)?;
            let a = (num_arg_si(args, 0, name, "q1")? as i64, num_arg_si(args, 1, name, "r1")? as i64);
            let b = (num_arg_si(args, 2, name, "q2")? as i64, num_arg_si(args, 3, name, "r2")? as i64);
            Ok(Value::Scalar(crate::geo::hexgrid::hex_distance(a, b) as f64))
        }
        "hexid" => {
            // hexid(q, r): тритный адрес ячейки — s спирали + 6-тритный блок.
            need(args, 2, name)?;
            let q = num_arg_si(args, 0, name, "q")? as i64;
            let r = num_arg_si(args, 1, name, "r")? as i64;
            let s = crate::geo::hexgrid::axial_to_spiral(q, r)
                .map_err(|e| format!("hexid: {e}"))?;
            let t = crate::geo::hexgrid::hex_trit_id(s).map_err(|e| format!("hexid: {e}"))?;
            let ring = crate::geo::hexgrid::hex_distance((0, 0), (q, r));
            Ok(Value::Str(format!(
                "s = {s} · блок 6 тритов: {} · кольцо {ring}",
                t.to_string_bal()
            )))
        }
        "de9im_line" => {
            // relate LineString×Polygon: линия — ОТКРЫТАЯ ломаная Nx2,
            // кольцо — ≥ 3 вершин (замыкается автоматически).
            let a = ring_arg(args, 0, "de9im_line")?;
            let b = ring_arg(args, 1, "de9im_line")?;
            let k = match args.get(2) {
                None => 6u8,
                Some(Value::Scalar(v)) if *v >= 0.0 && *v <= 12.0 => *v as u8,
                Some(other) => {
                    return Err(format!(
                        "de9im_line: масштаб k ∈ [0, 12] — число, получено {other}"
                    ))
                }
            };
            let qa = quantize_ring(&a, k)?;
            let qb = quantize_ring(&b, k)?;
            let m = crate::geo::de9im::relate_line_ring(&qa, &qb)
                .map_err(|e| format!("de9im_line: {e}"))?;
            let preds = m.predicate_names().join(", ");
            Ok(Value::Str(format!(
                "{} · код {} · предикаты: {}",
                m.bal_string(),
                m.trit_code(),
                if preds.is_empty() { "—" } else { &preds }
            )))
        }
        "eteria_field" => {
            // eteria_field([seed[, n]]): карта высот Этерии → viz_field.
            let seed = seed_arg(args, 0, 7)?;
            let n = match args.get(1) {
                None => 33usize,
                Some(Value::Scalar(v)) if *v == 9.0 || *v == 17.0 || *v == 33.0 || *v == 65.0 => {
                    *v as usize
                }
                Some(other) => {
                    return Err(format!(
                        "eteria_field: n ∈ {{9, 17, 33, 65}} (2^m+1), получено {other}"
                    ))
                }
            };
            let z = crate::geo::eteria::heightmap(seed, n)?;
            Ok(Value::Matrix(Matrix::from_rows(&z)?))
        }
        "eteria_hex" => {
            // eteria_hex([seed[, radius]]): [q, r, высота, биом] гекс-карты.
            let seed = seed_arg(args, 0, 7)?;
            let radius = match args.get(1) {
                None => 8.0,
                Some(_) => num_arg_si(args, 1, name, "радиус гекс-диска")?,
            };
            if !(1.0..=10.0).contains(&radius) || radius.fract() != 0.0 {
                return Err(format!(
                    "eteria_hex: радиус — целое ∈ [1, 10], получено {radius}"
                ));
            }
            let cells = crate::geo::eteria::hex_cells(seed, radius as i64)?;
            let rows: Vec<Vec<f64>> = cells
                .iter()
                .map(|c| vec![c.q as f64, c.r as f64, c.h, c.biome as f64])
                .collect();
            Ok(Value::Matrix(Matrix::from_rows(&rows)?))
        }
        "eteria_boltz" => {
            // eteria_boltz([seed[, radius[, scale]]]): больцмановское
            // переплетение — Z = Σ e^(−E_i) в лог-домене за f64.
            let seed = seed_arg(args, 0, 7)?;
            let radius = match args.get(1) {
                None => 8.0,
                Some(_) => num_arg_si(args, 1, name, "радиус гекс-диска")?,
            };
            let scale = match args.get(2) {
                None => 1e5,
                Some(_) => num_arg_si(args, 2, name, "масштаб энергий")?,
            };
            Ok(Value::Str(
                crate::geo::eteria::boltzmann_report(seed, radius as i64, scale)?,
            ))
        }
        "tin" => {
            // tin(точки Nx2[, k]): TIN Делоне — Bowyer-Watson на тритной
            // решётке, порядок вставки водит Муха (sin(i·Φ)).
            let pts = ring_arg(args, 0, "tin")?;
            if pts.len() < 3 {
                return Err(format!(
                    "tin: нужно ≥ 3 точек (Nx2), получено {}",
                    pts.len()
                ));
            }
            let k = match args.get(1) {
                None => 10u8,
                Some(Value::Scalar(v)) if *v >= 1.0 && *v <= 14.0 => *v as u8,
                Some(other) => {
                    return Err(format!(
                        "tin: решётка k ∈ [1, 14] (i128-запас детерминанта), получено {other}"
                    ))
                }
            };
            let mut pts_i: Vec<(i64, i64)> = Vec::with_capacity(pts.len());
            for &(x, y) in &pts {
                let tp = crate::geo::trit_coord::TritPoint::quantize(x, y, k)
                    .map_err(|e| format!("tin: квантование ({x}, {y}): {e}"))?;
                pts_i.push((tp.x.to_i64(), tp.y.to_i64()));
            }
            let tin = crate::geo::delaunay::delaunay(&pts_i)
                .map_err(|e| format!("tin: {e}"))?;
            crate::geo::delaunay::verify_delaunay(&tin)
                .map_err(|e| format!("tin: {e}"))?;
            let hull = tin.hull().len();
            let n = tin.pts.len();
            Ok(Value::Str(format!(
                "TIN Делоне · {n} точек · {} треугольников · оболочка {hull} рёбер\n\
                 муха: порядок вставки sin(i·Φ mod 2π) — Gold Phase Lock из universal_letters\n\
                 сессия-16: history-DAG локализация + BFS-полость — O(n log n) в среднем\n\
                 Эйлер: T = 2n−2−h = {} · детерминант i128, ноль округлений · Делоне верифицирован",
                tin.tris.len(),
                2 * n as i64 - 2 - hull as i64
            )))
        }
        "eteria_tin" => {
            // eteria_tin([seed[, n[, scale]]]): мир Этерии как TIN —
            // золотой посев + Муха + LogProb за пределами f64.
            let seed = seed_arg(args, 0, 7)?;
            let n = match args.get(1) {
                None => 256.0,
                Some(_) => num_arg_si(args, 1, name, "вершин TIN")?,
            };
            let scale = match args.get(2) {
                None => 1e5,
                Some(_) => num_arg_si(args, 2, name, "масштаб энергий")?,
            };
            if !(3.0..=5000.0).contains(&n) {
                return Err(format!("eteria_tin: вершин n ∈ [3, 5000], получено {n}"));
            }
            Ok(Value::Str(crate::geo::eteria::tin_boltz_report(
                seed,
                n as usize,
                scale,
            )?))
        }
        "h3children" => {
            // h3children(q, r): 9 потомков апертурой 9 = 3² (res+1).
            need(args, 2, name)?;
            let q = num_arg_si(args, 0, name, "q")? as i64;
            let r = num_arg_si(args, 1, name, "r")? as i64;
            let kids = crate::geo::hexgrid::h3_children(q, r);
            let list: Vec<String> = kids
                .iter()
                .enumerate()
                .map(|(n, &(cq, cr))| {
                    let di = (n as i64 / 3) - 1;
                    let dj = (n as i64 % 3) - 1;
                    format!("({cq:+}, {cr:+})←({di:+},{dj:+})")
                })
                .collect();
            Ok(Value::Str(format!(
                "дробление апертурой 9 = 3² (решётка масштабируется ТОЧНО, \
                 ноль таблиц — против √7-таблиц H3)\n\
                 потомки ({q}, {r}): {} · адрес +2 трита на уровень",
                list.join(" ")
            )))
        }
        "h3parent" => {
            // h3parent(q, r): родитель + уточняющие триты.
            need(args, 2, name)?;
            let q = num_arg_si(args, 0, name, "q")? as i64;
            let r = num_arg_si(args, 1, name, "r")? as i64;
            let ((pq, pr), (i, j)) = crate::geo::hexgrid::h3_parent(q, r);
            let t = crate::geo::hexgrid::refine_trits(i, j)
                .map_err(|e| format!("h3parent: {e}"))?;
            Ok(Value::Str(format!(
                "родитель ({q}, {r}) → ({pq}, {pr}) · уточнение ({i:+}, {j:+}) \
                 → 2 трита: {} · адрес −2 трита",
                t.to_string_bal()
            )))
        }
        "de9im_ll" => {
            // relate LineString×LineString: обе — ОТКРЫТЫЕ ломаные Nx2.
            let a = ring_arg(args, 0, "de9im_ll")?;
            let b = ring_arg(args, 1, "de9im_ll")?;
            let k = match args.get(2) {
                None => 6u8,
                Some(Value::Scalar(v)) if *v >= 0.0 && *v <= 12.0 => *v as u8,
                Some(other) => {
                    return Err(format!(
                        "de9im_ll: масштаб k ∈ [0, 12] — число, получено {other}"
                    ))
                }
            };
            let qa = quantize_ring(&a, k)?;
            let qb = quantize_ring(&b, k)?;
            let m = crate::geo::de9im::relate_line_line(&qa, &qb)
                .map_err(|e| format!("de9im_ll: {e}"))?;
            let preds = m.predicate_names().join(", ");
            Ok(Value::Str(format!(
                "{} · код {} · предикаты: {}",
                m.bal_string(),
                m.trit_code(),
                if preds.is_empty() { "—" } else { &preds }
            )))
        }
        "eteria_tin_pts" => {
            // eteria_tin_pts([seed[, n]]): вершины золотого посева Nx3
            // [x, y, высота] — готовый вход viz_tin (сессия-16).
            let seed = seed_arg(args, 0, 7)?;
            let n = match args.get(1) {
                None => 256.0,
                Some(_) => num_arg_si(args, 1, name, "вершин TIN")?,
            };
            if !(3.0..=5000.0).contains(&n) {
                return Err(format!(
                    "eteria_tin_pts: вершин n ∈ [3, 5000], получено {n}"
                ));
            }
            let verts = crate::geo::eteria::tin_vertices(seed, n as usize)?;
            let rows: Vec<Vec<f64>> = verts
                .iter()
                .map(|&(x, y, h)| vec![x, y, h])
                .collect();
            Ok(Value::Matrix(Matrix::from_rows(&rows)?))
        }
        "viz_tin" => {
            // viz_tin(точки Nx2|Nx3[, k][, "заголовок"]): SVG-рельеф
            // триангуляции (сессия-16). Nx3 — [x, y, высота] → биомы;
            // Nx2 — структура сети без окраса.
            let m = matrix_arg(args, 0)?;
            if (m.cols != 2 && m.cols != 3) || m.rows < 1 {
                return Err(format!(
                    "viz_tin: точки — матрица Nx2 (x, y) либо Nx3 (x, y, высота), \
                     получено {}×{}",
                    m.rows, m.cols
                ));
            }
            let k = match args.get(1) {
                None => 10u8,
                Some(Value::Scalar(v)) if *v >= 1.0 && *v <= 14.0 => *v as u8,
                Some(other) => {
                    return Err(format!(
                        "viz_tin: решётка k ∈ [1, 14] (i128-запас детерминанта), \
                         получено {other}"
                    ))
                }
            };
            let title_text = match args.get(2) {
                Some(Value::Str(s)) => s.clone(),
                None => "TIN Делоне · карта триангуляции".to_string(),
                Some(other) => {
                    return Err(format!(
                        "viz_tin: заголовок — строка, получено {other}"
                    ))
                }
            };
            let has_z = m.cols == 3;
            let mut pts_i: Vec<(i64, i64)> = Vec::with_capacity(m.rows);
            let mut z: Vec<f64> = Vec::with_capacity(m.rows);
            for r in 0..m.rows {
                let (x, y) = (m.get(r, 0).re, m.get(r, 1).re);
                let tp = crate::geo::trit_coord::TritPoint::quantize(x, y, k)
                    .map_err(|e| format!("viz_tin: квантование ({x}, {y}): {e}"))?;
                pts_i.push((tp.x.to_i64(), tp.y.to_i64()));
                // Nx2 — без высот (колонки 2 нет): 0.0, окрас отключён has_z
                z.push(if has_z { m.get(r, 2).re } else { 0.0 });
            }
            let mut tin = crate::geo::delaunay::delaunay(&pts_i)
                .map_err(|e| format!("viz_tin: {e}"))?;
            crate::geo::delaunay::verify_delaunay(&tin)
                .map_err(|e| format!("viz_tin: {e}"))?;
            let (_, idx_nodes) = tin
                .build_spatial27()
                .map_err(|e| format!("viz_tin: {e}"))?;
            let hull = tin.hull().len();
            let note = format!(
                "{} вершин · {} треугольников · оболочка {hull} рёбер · \
                 27-дерево: {idx_nodes} узлов · муха sin(i·Φ)",
                tin.pts.len(),
                tin.tris.len()
            );
            Ok(Value::Str(viz::tin_svg(
                &tin,
                if has_z { Some(&z) } else { None },
                &title_text,
                &note,
            )?))
        }
        "poly" => {
            // poly(кольцо, дырка1, …): полигон с дырками — список колец
            // (первое — внешнее) для de9im_poly (сессия-16).
            if args.is_empty() {
                return Err("poly: хотя бы одно кольцо — внешнее".into());
            }
            let mut rings = Vec::with_capacity(args.len());
            for (i, a) in args.iter().enumerate() {
                match a {
                    Value::Matrix(m) if m.cols == 2 && m.rows >= 3 => {
                        rings.push(Value::Matrix(m.clone()));
                    }
                    other => {
                        return Err(format!(
                            "poly: кольцо {} — матрица Nx2 (≥ 3 вершин), получено {other}",
                            i + 1
                        ))
                    }
                }
            }
            Ok(Value::List(rings))
        }
        "de9im_poly" => {
            // de9im_poly(A, B[, k]): relate полигон×полигон с дырками.
            // Полигон — кольцо Nx2 (без дырок) либо poly(кольца…).
            let k = match args.get(2) {
                None => 6u8,
                Some(Value::Scalar(v)) if *v >= 0.0 && *v <= 12.0 => *v as u8,
                Some(other) => {
                    return Err(format!(
                        "de9im_poly: масштаб k ∈ [0, 12] — число, получено {other}"
                    ))
                }
            };
            let pa = polygon_arg(args, 0, "de9im_poly", k)?;
            let pb = polygon_arg(args, 1, "de9im_poly", k)?;
            let m = crate::geo::de9im::relate_polygon_polygon(&pa, &pb)
                .map_err(|e| format!("de9im_poly: {e}"))?;
            let preds = m.predicate_names().join(", ");
            let holes_a = pa.len() - 1;
            let holes_b = pb.len() - 1;
            Ok(Value::Str(format!(
                "{} · код {} · предикаты: {}\n\
                 A: {} колец (дырок {holes_a}) · B: {} колец (дырок {holes_b})",
                m.bal_string(),
                m.trit_code(),
                if preds.is_empty() { "—" } else { &preds },
                pa.len(),
                pb.len()
            )))
        }
        "viz_hex" => {
            // viz_hex(гекс-карта[, заголовок]): SVG-карта биомов.
            // карта — матрица Nx4 [q, r, высота, биом] (eteria_hex) или Nx3.
            let m = matrix_arg(args, 0)?;
            if (m.cols != 3 && m.cols != 4) || m.rows < 1 {
                return Err(format!(
                    "viz_hex: карта — Nx4 [q, r, высота, биом] либо Nx3 [q, r, биом], \
                     получено {}×{}",
                    m.rows, m.cols
                ));
            }
            let title_text = match args.get(1) {
                Some(Value::Str(s)) => s.clone(),
                None => "Этерия · гекс-карта биомов".to_string(),
                Some(other) => {
                    return Err(format!("viz_hex: заголовок — строка, получено {other}"))
                }
            };
            let biome_col = m.cols - 1;
            let mut cells = Vec::with_capacity(m.rows);
            for r in 0..m.rows {
                let q = m.get(r, 0).re;
                let rr = m.get(r, 1).re;
                let b = m.get(r, biome_col).re;
                if q.fract() != 0.0 || rr.fract() != 0.0 || b.fract() != 0.0 || b < 0.0 || b > 5.0 {
                    return Err(format!(
                        "viz_hex: строка {}: [q, r, биом] — целые, биом ∈ [0, 5]",
                        r + 1
                    ));
                }
                cells.push((q as i64, rr as i64, b as usize));
            }
            let note = format!("{} ячеек · трит-спираль H3 · сессия-14", m.rows);
            Ok(Value::Str(viz::hex_svg(&cells, &title_text, &note)?))
        }
        // ---------- три горизонта (сессия-17) ----------
        "tin_isolines" => {
            // tin_isolines(точки Nx3[, уровни][, сетка][, k]): изолинии
            // TIN — marching squares по height_at + ТОЧНЫЙ эталон
            // (сечение треугольников) с аудитом длин.
            let m = matrix_arg(args, 0)?;
            if m.cols != 3 || m.rows < 3 {
                return Err(format!(
                    "tin_isolines: точки — матрица Nx3 [x, y, высота] (≥ 3), \
                     получено {}×{} — изолинии требуют высот",
                    m.rows, m.cols
                ));
            }
            // уровни: умолчание — 5 равномерных (1/6..5/6 диапазона);
            // число — столько равномерных; список — явные уровни
            let levels_arg: Option<Vec<f64>> = match args.get(1) {
                None => None,
                Some(Value::Scalar(v)) => {
                    if !(*v >= 1.0 && *v <= 20.0 && v.fract() == 0.0) {
                        return Err(format!(
                            "tin_isolines: уровней ∈ [1, 20] (целое) либо список, получено {v}"
                        ));
                    }
                    Some(Vec::new()) // маркер «равномерные» — заполняется после zmin/zmax
                }
                Some(Value::List(items)) => {
                    let mut out = Vec::with_capacity(items.len());
                    for it in items {
                        match it {
                            Value::Scalar(v) => out.push(*v),
                            other => {
                                return Err(format!(
                                    "tin_isolines: уровень — число, получено {other}"
                                ))
                            }
                        }
                    }
                    if out.is_empty() {
                        return Err("tin_isolines: список уровней пуст".into());
                    }
                    Some(out)
                }
                Some(other) => {
                    return Err(format!(
                        "tin_isolines: уровни — число либо список чисел, получено {other}"
                    ))
                }
            };
            let n_levels = match &levels_arg {
                Some(v) if v.is_empty() => 5usize,
                Some(v) => v.len(),
                None => 5usize,
            };
            let grid = match args.get(2) {
                None => 96.0,
                Some(Value::Scalar(v)) if *v >= 16.0 && *v <= 384.0 && v.fract() == 0.0 => *v,
                Some(other) => {
                    return Err(format!(
                        "tin_isolines: сетка ∈ [16, 384] ячеек на сторону, получено {other}"
                    ))
                }
            };
            let k = match args.get(3) {
                None => 10u8,
                Some(Value::Scalar(v)) if *v >= 1.0 && *v <= 14.0 => *v as u8,
                Some(other) => {
                    return Err(format!(
                        "tin_isolines: решётка k ∈ [1, 14], получено {other}"
                    ))
                }
            };
            let mut pts_i: Vec<(i64, i64)> = Vec::with_capacity(m.rows);
            let mut z: Vec<f64> = Vec::with_capacity(m.rows);
            for r in 0..m.rows {
                let (x, y) = (m.get(r, 0).re, m.get(r, 1).re);
                let tp = crate::geo::trit_coord::TritPoint::quantize(x, y, k)
                    .map_err(|e| format!("tin_isolines: квантование ({x}, {y}): {e}"))?;
                pts_i.push((tp.x.to_i64(), tp.y.to_i64()));
                z.push(m.get(r, 2).re);
            }
            let mut tin = crate::geo::delaunay::delaunay(&pts_i)
                .map_err(|e| format!("tin_isolines: {e}"))?;
            crate::geo::delaunay::verify_delaunay(&tin)
                .map_err(|e| format!("tin_isolines: {e}"))?;
            let (idx_objs, idx_nodes) = tin
                .build_spatial27()
                .map_err(|e| format!("tin_isolines: {e}"))?;
            let (zmin, zmax) =
                z.iter()
                    .cloned()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                        (a.min(v), b.max(v))
                    });
            let levels: Vec<f64> = match levels_arg {
                Some(explicit) if !explicit.is_empty() => explicit,
                _ => (1..=n_levels)
                    .map(|i| zmin + (zmax - zmin) * i as f64 / (n_levels as f64 + 1.0))
                    .collect(),
            };
            let mut rows = Vec::with_capacity(levels.len());
            for lev in &levels {
                let ms = crate::geo::marchsq::tin_isolines(&tin, &z, *lev, grid as usize)
                    .map_err(|e| format!("tin_isolines: {e}"))?;
                let ex = crate::geo::marchsq::tin_isolines_exact(&tin, &z, *lev)
                    .map_err(|e| format!("tin_isolines: {e}"))?;
                let lm = crate::geo::marchsq::total_length(&ms);
                let le = crate::geo::marchsq::total_length(&ex);
                let dev = if le > 1e-12 {
                    (lm - le).abs() / le * 100.0
                } else if lm > 1e-12 {
                    f64::INFINITY
                } else {
                    0.0
                };
                rows.push(format!(
                    "  уровень {:.3}: MS {} ветвей · {:.1} квантов · точно {} · {:.1} · Δ {}%",
                    lev,
                    ms.len(),
                    lm,
                    ex.len(),
                    le,
                    if dev.is_finite() { format!("{dev:.1}") } else { "∞".into() }
                ));
            }
            Ok(Value::Str(format!(
                "изолинии TIN · marching squares по height_at + точное сечение · {} вершин · {} треугольников · сетка {}×{} · 27-дерево: \
                 {idx_objs} конвертов · {idx_nodes} узлов\n{}\nэталон: сечение треугольников \
                 плоскостью z = уровень — цепочки watertight, длины сходятся к MS",
                tin.pts.len(),
                tin.tris.len(),
                grid,
                grid,
                rows.join("\n")
            )))
        }
        "viz_isolines" => {
            // viz_isolines(точки Nx3[, уровни][, k][, "заголовок"]):
            // SVG-карта — биомы TIN + изолинии marching squares.
            let m = matrix_arg(args, 0)?;
            if m.cols != 3 || m.rows < 3 {
                return Err(format!(
                    "viz_isolines: точки — матрица Nx3 [x, y, высота] (≥ 3), получено {}×{}",
                    m.rows, m.cols
                ));
            }
            let levels_arg: Option<Vec<f64>> = match args.get(1) {
                None => None,
                Some(Value::Scalar(v)) => {
                    if !(*v >= 1.0 && *v <= 12.0 && v.fract() == 0.0) {
                        return Err(format!(
                            "viz_isolines: уровней ∈ [1, 12] (целое) либо список, получено {v}"
                        ));
                    }
                    Some(Vec::new())
                }
                Some(Value::List(items)) => {
                    let mut out = Vec::with_capacity(items.len());
                    for it in items {
                        match it {
                            Value::Scalar(v) => out.push(*v),
                            other => {
                                return Err(format!(
                                    "viz_isolines: уровень — число, получено {other}"
                                ))
                            }
                        }
                    }
                    if out.is_empty() || out.len() > 8 {
                        return Err("viz_isolines: уровней — от 1 до 8".into());
                    }
                    Some(out)
                }
                Some(other) => {
                    return Err(format!(
                        "viz_isolines: уровни — число либо список, получено {other}"
                    ))
                }
            };
            let n_levels = match &levels_arg {
                Some(v) if v.is_empty() => 5usize,
                Some(v) => v.len(),
                None => 5usize,
            };
            let k = match args.get(2) {
                None => 10u8,
                Some(Value::Scalar(v)) if *v >= 1.0 && *v <= 14.0 => *v as u8,
                Some(other) => {
                    return Err(format!(
                        "viz_isolines: решётка k ∈ [1, 14], получено {other}"
                    ))
                }
            };
            let title_text = match args.get(3) {
                Some(Value::Str(s)) => s.clone(),
                None => "TIN · изолинии рельефа".to_string(),
                Some(other) => {
                    return Err(format!(
                        "viz_isolines: заголовок — строка, получено {other}"
                    ))
                }
            };
            let mut pts_i: Vec<(i64, i64)> = Vec::with_capacity(m.rows);
            let mut z: Vec<f64> = Vec::with_capacity(m.rows);
            for r in 0..m.rows {
                let (x, y) = (m.get(r, 0).re, m.get(r, 1).re);
                let tp = crate::geo::trit_coord::TritPoint::quantize(x, y, k)
                    .map_err(|e| format!("viz_isolines: квантование ({x}, {y}): {e}"))?;
                pts_i.push((tp.x.to_i64(), tp.y.to_i64()));
                z.push(m.get(r, 2).re);
            }
            let mut tin = crate::geo::delaunay::delaunay(&pts_i)
                .map_err(|e| format!("viz_isolines: {e}"))?;
            crate::geo::delaunay::verify_delaunay(&tin)
                .map_err(|e| format!("viz_isolines: {e}"))?;
            let (idx_objs, idx_nodes) = tin
                .build_spatial27()
                .map_err(|e| format!("viz_isolines: {e}"))?;
            let (zmin, zmax) =
                z.iter()
                    .cloned()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                        (a.min(v), b.max(v))
                    });
            let levels: Vec<f64> = match levels_arg {
                Some(explicit) if !explicit.is_empty() => explicit,
                _ => (1..=n_levels)
                    .map(|i| zmin + (zmax - zmin) * i as f64 / (n_levels as f64 + 1.0))
                    .collect(),
            };
            // изолинии: marching squares по height_at (27-дерево), сетка 96
            let mut level_lines = Vec::with_capacity(levels.len());
            let mut branches = 0usize;
            for lev in &levels {
                let chains = crate::geo::marchsq::tin_isolines(&tin, &z, *lev, 96)
                    .map_err(|e| format!("viz_isolines: {e}"))?;
                branches += chains.len();
                level_lines.push((*lev, chains));
            }
            let note = format!(
                "{} вершин · {} треугольников · изолинии: {} уровней · {} ветвей · \
                 marching squares по height_at · 27-дерево: {} узлов",
                tin.pts.len(),
                tin.tris.len(),
                levels.len(),
                branches,
                idx_nodes
            );
            Ok(Value::Str(viz::isolines_svg(
                &tin, &z, &level_lines, &title_text, &note,
            )?))
        }
        "h3addr" => {
            // h3addr(face, q, r[, res]): мировой адрес ячейки НА уровне res
            // (умолчание 2 — канон Сессии-17, 13 тритов).
            if args.len() < 3 || args.len() > 4 {
                return Err(format!(
                    "h3addr: h3addr(face, q, r[, res]) — 3 или 4 аргумента, получено {}",
                    args.len()
                ));
            }
            let face = num_arg_si(args, 0, name, "грань")?;
            if !(0.0..=19.0).contains(&face) || face.fract() != 0.0 {
                return Err(format!("h3addr: грань ∈ [0, 19], получено {face}"));
            }
            let q = num_arg_si(args, 1, name, "q")?;
            let r = num_arg_si(args, 2, name, "r")?;
            if q.fract() != 0.0 || r.fract() != 0.0 {
                return Err("h3addr: q, r — целые осевые координаты".into());
            }
            let res = match args.get(3) {
                None => crate::geo::hexgrid::CANON_RES as f64,
                Some(_) => num_arg_si(args, 3, name, "res")?,
            };
            if !(0.0..=6.0).contains(&res) || res.fract() != 0.0 {
                return Err(format!(
                    "h3addr: res ∈ [0, 6] (канон Сессии-17 — 2, адрес 13 тритов), получено {res}"
                ));
            }
            let res = res as usize;
            let (fq, qr, rr) = (face as usize, q as i64, r as i64);
            let t = crate::geo::hexgrid::world_address(fq, qr, rr, res)
                .map_err(|e| format!("h3addr: {e}"))?;
            // путь родителей для отчёта: res-0 координаты + пары уточнений
            let (mut pq, mut pr) = (qr, rr);
            let mut path = Vec::with_capacity(res);
            for _ in 0..res {
                let ((a, b), ij) = crate::geo::hexgrid::h3_parent(pq, pr);
                path.push(ij);
                (pq, pr) = (a, b);
            }
            let s = crate::geo::hexgrid::axial_to_spiral(pq, pr)
                .map_err(|e| format!("h3addr: {e}"))?;
            // roundtrip — доказательство в самом отчёте
            let (f2, q2, r2) = crate::geo::hexgrid::world_address_parse(&t, res)
                .map_err(|e| format!("h3addr: {e}"))?;
            let same = (f2, q2, r2) == (fq, qr, rr);
            // адрес с разделителями: грань · спираль · уточнения
            let bal = t.to_string_bal();
            let seg = format!(
                "{} · {} · {}",
                &bal[..3],
                &bal[3..9],
                (0..res)
                    .map(|i| &bal[9 + 2 * i..9 + 2 * i + 2])
                    .collect::<Vec<_>>()
                    .join(" · ")
            );
            let path_str = path
                .iter()
                .rev()
                .map(|&(i, j)| format!("({i:+}, {j:+})"))
                .collect::<Vec<_>>()
                .join(" ");
            Ok(Value::Str(format!(
                "грань {}/20 · res {} · адрес {} тритов: {}\nструктура: {seg}\n                 спираль res-0: s = {s} → ({pq:+}, {pr:+}) · уточнения (снизу вверх): {path_str}\n                 roundtrip: ({qr:+}, {rr:+}) → адрес → ({q2:+}, {r2:+}) — {}",
                fq + 1,
                res,
                t.len(),
                bal,
                if same { "сходится" } else { "РАСХОДИТСЯ" },
            )))
        }
        "h3cell" => {
            // h3cell("адрес"): разбор мирового адреса — грань, res,
            // спираль, путь уточнений, координаты ячейки на её уровне.
            let addr = str_arg(args, 0)?;
            // длина строки-оригинала — единственный источник res:
            // Trits::parse обрезает ведущие нули (грани 4..19)
            let n_chars = addr.chars().count();
            if n_chars < 9 || n_chars % 2 == 0 {
                return Err(format!(
                    "h3cell: адрес — нечётное число тритов ≥ 9 (9 + 2·res), \
                     получено {n_chars} символов"
                ));
            }
            let res = (n_chars - 9) / 2;
            if res > crate::geo::hexgrid::MAX_RES {
                return Err(format!(
                    "h3cell: res = {res} > {} (механика Сессии-17)",
                    crate::geo::hexgrid::MAX_RES
                ));
            }
            let t = Trits::parse(&addr).map_err(|e| format!("h3cell: {e}"))?;
            let (face, q, r) = crate::geo::hexgrid::world_address_parse(&t, res)
                .map_err(|e| format!("h3cell: {e}"))?;
            // путь родителей: res-0 + пары уточнений
            let (mut pq, mut pr) = (q, r);
            let mut path = Vec::with_capacity(res);
            for _ in 0..res {
                let ((a, b), ij) = crate::geo::hexgrid::h3_parent(pq, pr);
                path.push(ij);
                (pq, pr) = (a, b);
            }
            let s = crate::geo::hexgrid::axial_to_spiral(pq, pr)
                .map_err(|e| format!("h3cell: {e}"))?;
            let path_str = path
                .iter()
                .rev()
                .map(|&(i, j)| format!("({i:+}, {j:+})"))
                .collect::<Vec<_>>()
                .join(" ");
            Ok(Value::Str(format!(
                "адрес {} тритов · res {} · грань {}/20\nспираль res-0: s = {s} → ({pq:+}, {pr:+})\n                 уточнения (снизу вверх): {}\nячейка res-{res}: ({q:+}, {r:+})",
                9 + 2 * res,
                res,
                face + 1,
                if path_str.is_empty() { "— (res 0)" } else { &path_str },
            )))
        }
        "line" => {
            // line(точки Nx2): открытая ломаная для gc/de9im_gc.
            let m = matrix_arg(args, 0)?;
            if m.cols != 2 || m.rows < 2 {
                return Err(format!(
                    "line: ломаная — матрица Nx2 (≥ 2 вершин), получено {}×{}",
                    m.rows, m.cols
                ));
            }
            Ok(Value::List(vec![Value::Str("line".into()), Value::Matrix(m)]))
        }
        "gc" => {
            // gc(геометрия, …): GeometryCollection — коллекция разной
            // размерности (точки/линии/полигоны) для de9im_gc.
            if args.is_empty() {
                return Err("gc: коллекция — хотя бы одна геометрия".into());
            }
            for (i, a) in args.iter().enumerate() {
                validate_geometry_shape(a, "gc")
                    .map_err(|e| format!("gc: элемент {}: {e}", i + 1))?;
            }
            let mut items = Vec::with_capacity(args.len() + 1);
            items.push(Value::Str("gc".into()));
            items.extend(args.iter().cloned());
            Ok(Value::List(items))
        }
        "de9im_gc" => {
            // de9im_gc(A, B[, k]): relate геометрий ПРОИЗВОЛЬНОЙ
            // размерности — точки/линии/полигоны/коллекции (union OGC).
            let k = match args.get(2) {
                None => 6u8,
                Some(Value::Scalar(v)) if *v >= 0.0 && *v <= 12.0 => *v as u8,
                Some(other) => {
                    return Err(format!(
                        "de9im_gc: масштаб k ∈ [0, 12] — число, получено {other}"
                    ))
                }
            };
            let a = geometry_arg(args, 0, "de9im_gc", k)?;
            let b = geometry_arg(args, 1, "de9im_gc", k)?;
            let m = crate::geo::de9im::relate_geometry(&a, &b)
                .map_err(|e| format!("de9im_gc: {e}"))?;
            let preds = m.predicate_names().join(", ");
            Ok(Value::Str(format!(
                "{} · код {} · предикаты: {}\nA: {} · B: {}",
                m.bal_string(),
                m.trit_code(),
                if preds.is_empty() { "—" } else { &preds },
                describe_geometry(&a),
                describe_geometry(&b),
            )))
        }
        // ---------- триты ----------
        "trits" => {
            let v = one_arg(args, name)?;
            let t = Trits::from_i64(v as i64)
                .map_err(|e| format!("trits({v}): {e}"))?;
            Ok(Value::Str(t.to_string_bal()))
        }
        "trit_val" => {
            let s = str_arg(args, 0)?;
            Ok(Value::Scalar(Trits::parse(&s)?.to_i64() as f64))
        }
        "trit_and" | "trit_or" => {
            need(args, 2, name)?;
            let a = Trits::parse(&str_arg(args, 0)?)?;
            let b = Trits::parse(&str_arg(args, 1)?)?;
            let r = if name == "trit_and" { Trits::t_and(&a, &b) } else { Trits::t_or(&a, &b) };
            Ok(Value::Str(r.to_string_bal()))
        }
        "trit_not" => {
            let a = Trits::parse(&str_arg(args, 0)?)?;
            Ok(Value::Str(Trits::t_not(&a).to_string_bal()))
        }

        // ---------- температура ----------
        "degC" => {
            let c = one_arg(args, name)?;
            let u = units::by_name("degC").unwrap();
            Ok(Value::Quantity(c, u))
        }
        "degF" => {
            let f = one_arg(args, name)?;
            let u = units::by_name("degF").unwrap();
            Ok(Value::Quantity(f, u))
        }

        // ---------- астрономия ----------
        "sun_lon" => {
            let (y, m, d, h) = date_args(args, 0)?;
            Ok(Value::Scalar(astro::sun_ecliptic_lon(y, m, d, h)))
        }
        "sun_ra" => {
            let (y, m, d, h) = date_args(args, 0)?;
            let (ra, _) = astro::sun_ra_dec(y, m, d, h);
            Ok(Value::Scalar(ra))
        }
        "sun_dec" => {
            let (y, m, d, h) = date_args(args, 0)?;
            let (_, dec) = astro::sun_ra_dec(y, m, d, h);
            Ok(Value::Scalar(dec))
        }
        "moon_lon" => {
            let (y, m, d, h) = date_args(args, 0)?;
            Ok(Value::Scalar(astro::moon_position(y, m, d, h).0))
        }
        "moon_lat" => {
            let (y, m, d, h) = date_args(args, 0)?;
            Ok(Value::Scalar(astro::moon_position(y, m, d, h).1))
        }
        "moon_dist" => {
            let (y, m, d, h) = date_args(args, 0)?;
            Ok(Value::Scalar(astro::moon_position(y, m, d, h).2))
        }
        "moon_phase" => {
            let (y, m, d, h) = date_args(args, 0)?;
            Ok(Value::Scalar(astro::moon_elongation(y, m, d, h)))
        }
        "moon_illum" => {
            let (y, m, d, h) = date_args(args, 0)?;
            Ok(Value::Scalar(astro::moon_illumination(y, m, d, h)))
        }
        "moon_age" => {
            let (y, m, d, h) = date_args(args, 0)?;
            Ok(Value::Scalar(astro::moon_age_days(y, m, d, h)))
        }
        "moon_ra" => {
            let (y, m, d, h) = date_args(args, 0)?;
            let (ra, _) = astro::moon_ra_dec(y, m, d, h);
            Ok(Value::Scalar(ra))
        }
        "moon_dec" => {
            let (y, m, d, h) = date_args(args, 0)?;
            let (_, dec) = astro::moon_ra_dec(y, m, d, h);
            Ok(Value::Scalar(dec))
        }
        "planet_lon" => {
            let pname = str_arg(args, 0)?;
            let (y, m, d, h) = date_args(args, 1)?;
            let (lon, _, _) = astro::planet_geocentric(&pname, y, m, d, h)?;
            Ok(Value::Scalar(lon))
        }
        "planet_dist" => {
            let pname = str_arg(args, 0)?;
            let (y, m, d, h) = date_args(args, 1)?;
            let (_, _, r) = astro::planet_geocentric(&pname, y, m, d, h)?;
            Ok(Value::Scalar(r))
        }
        "sunrise" | "sunset" => {
            // sunrise(lat, lon, y, m, d[, hour]) → UTC часы
            let (lat, lon) = (scalar_arg(args, 0)?, scalar_arg(args, 1)?);
            let (y, m, d, _) = date_args(args, 2)?;
            let ss = astro::sunrise_sunset_utc(y, m, d, lat, lon)?;
            match ss {
                None => Ok(Value::Str(
                    if name == "sunrise" { "полярная ночь" } else { "полярный день" }.into(),
                )),
                Some((rise, set)) => Ok(Value::Scalar(if name == "sunrise" { rise } else { set })),
            }
        }

        // ---------- геодезия/навигация ----------
        "dist" => {
            need(args, 4, name)?;
            Ok(Value::Scalar(geodesy::great_circle_km(
                scalar_arg(args, 0)?,
                scalar_arg(args, 1)?,
                scalar_arg(args, 2)?,
                scalar_arg(args, 3)?,
            )))
        }
        "bearing" => {
            need(args, 4, name)?;
            Ok(Value::Scalar(geodesy::bearing_deg(
                scalar_arg(args, 0)?,
                scalar_arg(args, 1)?,
                scalar_arg(args, 2)?,
                scalar_arg(args, 3)?,
            )))
        }
        "midpoint" => {
            need(args, 4, name)?;
            let (lat, lon) = geodesy::midpoint(
                scalar_arg(args, 0)?,
                scalar_arg(args, 1)?,
                scalar_arg(args, 2)?,
                scalar_arg(args, 3)?,
            );
            Ok(Value::List(vec![Value::Scalar(lat), Value::Scalar(lon)]))
        }
        "dest" => {
            need(args, 4, name)?;
            let (lat, lon) = geodesy::destination(
                scalar_arg(args, 0)?,
                scalar_arg(args, 1)?,
                scalar_arg(args, 2)?,
                scalar_arg(args, 3)?,
            );
            Ok(Value::List(vec![Value::Scalar(lat), Value::Scalar(lon)]))
        }
        "earth_radius" => Ok(Value::Scalar(geodesy::earth_radius_km(one_arg(args, name)?))),

        // ---------- квантовая химия и 118 элементов ----------
        "chem_elem" => {
            need(args, 1, name)?;
            let elem = resolve_chem_element(&args[0], name)?;
            Ok(Value::Str(format!(
                "[{}] {} ({}) · Z={} · A={:.4} г/моль · χ={:.2} · Блок {:?} · Гр.{} Пер.{} · {:?}",
                elem.symbol, elem.name_ru, elem.name, elem.z, elem.atomic_mass, elem.electronegativity,
                elem.block, elem.group, elem.period, elem.archetype
            )))
        }
        "chem_bond" => {
            need(args, 2, name)?;
            let elem_a = resolve_chem_element(&args[0], name)?;
            let elem_b = resolve_chem_element(&args[1], name)?;
            let bond = crate::universal_chem::calculate_bond(elem_a, elem_b);
            let type_str = match bond.bond_type {
                crate::universal_chem::BondType::NonPolarCovalent => "Неполярный ковалентный",
                crate::universal_chem::BondType::PolarCovalent => "Полярный ковалентный",
                crate::universal_chem::BondType::Ionic => "Ионный",
                crate::universal_chem::BondType::Metallic => "Металлический",
                crate::universal_chem::BondType::Inert => "Инертный/благородный газ",
            };
            let trit_char = match bond.trit {
                -1 => "− (акцептор/ион)",
                0 => "0 (симметричный)",
                1 => "+ (донор/полярный)",
                _ => "?",
            };
            Ok(Value::Str(format!(
                "{}-{} · Тип: {} · Трит: {} · Δχ = {:.2} · Длина ≈ {:.1} пм · Фазовый резонанс cos(Δφ) = {:.4}",
                elem_a.symbol, elem_b.symbol, type_str, trit_char, bond.delta_chi, bond.estimated_length_pm, bond.phase_resonance
            )))
        }
        "chem_mass" => {
            need(args, 1, name)?;
            let formula = match &args[0] {
                Value::Str(s) => s.as_str(),
                other => return Err(format!("{name}: ожидалась формула (строка), получено {other}")),
            };
            let mol = crate::universal_chem::parse_chemical_formula(formula)
                .map_err(|e| format!("{name}: {e}"))?;
            Ok(Value::Scalar(mol.molar_mass))
        }
        "chem_formula" => {
            need(args, 1, name)?;
            let formula = match &args[0] {
                Value::Str(s) => s.as_str(),
                other => return Err(format!("{name}: ожидалась формула (строка), получено {other}")),
            };
            let mol = crate::universal_chem::parse_chemical_formula(formula)
                .map_err(|e| format!("{name}: {e}"))?;
            let counts_str: Vec<String> = mol.counts.iter().map(|(k, v)| format!("{k}:{v}")).collect();
            let fractions_str: Vec<String> = mol.mass_fractions.iter().map(|(k, v)| format!("{k}: {:.2}%", v * 100.0)).collect();
            Ok(Value::Str(format!(
                "Формула {} · M = {:.4} г/моль · e⁻ = {} · Состав: [{}] · Массовые доли: [{}]",
                formula, mol.molar_mass, mol.total_electrons, counts_str.join(", "), fractions_str.join(", ")
            )))
        }
        "chem_archetype" => {
            need(args, 1, name)?;
            let elem = resolve_chem_element(&args[0], name)?;
            Ok(Value::Str(format!(
                "[{}] {} · Архетип: {} · Описание: {} · Золотая фаза φ(Z) = {:.4} рад",
                elem.symbol, elem.name, elem.archetype.name(), elem.archetype.description(), elem.phase_phi
            )))
        }
        "speak" => {
            need(args, 1, name)?;
            let text = match &args[0] {
                Value::Str(s) => s.clone(),
                other => return Err(format!(
                    "{name}: ожидался текст или формула (строка), получено {other}"
                )),
            };
            Ok(Value::Str(crate::scivoice::scientist_read(&text)))
        }
        "hf" => {
            if args.is_empty() || args.len() > 2 {
                return Err(format!(
                    "{name}: ожидается 1-2 аргумента: hf(\"H2\") или hf(\"H2\", 1.4), получено {}",
                    args.len()
                ));
            }
            let spec = match &args[0] {
                Value::Str(s) => s.clone(),
                other => return Err(format!(
                    "{name}: ожидалась молекула (строка вида \"H2\" или \"LiH@3.0\"), получено {other}"
                )),
            };
            // опциональная длина связи вторым аргументом (Бора)
            let spec = if args.len() >= 2 {
                let r = match &args[1] {
                    Value::Scalar(r) => *r,
                    other => return Err(format!(
                        "{name}: длина связи должна быть числом (Бора), получено {other}"
                    )),
                };
                format!("{spec}@{r}")
            } else {
                spec
            };
            let res = crate::quantum::eri::hartree_fock(&spec)?;
            Ok(Value::Str(crate::quantum::eri::hf_report(&res)))
        }
        "chem_smiles" => {
            need(args, 1, name)?;
            let input = expect_str(&args[0], name, "SMILES или имя молекулы")?;
            let g = crate::chem::resolve_input(&input)?;
            Ok(Value::Str(smiles_passport(&g, &input)))
        }
        "chem_3d" => {
            need(args, 1, name)?;
            let input = expect_str(&args[0], name, "SMILES или имя молекулы")?;
            let g = crate::chem::resolve_input(&input)?;
            let conf = crate::chem::embed(&g)?;
            Ok(Value::Str(conformer_report(&g, &conf, &input)))
        }
        "bio_eval" => {
            need(args, 2, name)?;
            let ligand = expect_str(&args[0], name, "лиганд (SMILES или имя)")?;
            let target = expect_str(&args[1], name, "мишень (SMILES или имя)")?;
            let res = crate::chem::binding_report(&ligand, &target)?;
            Ok(Value::Str(crate::chem::binding_text(&res)))
        }
        "pdb_info" => {
            need(args, 1, name)?;
            let path = expect_str(&args[0], name, "путь к PDB/mmCIF-файлу")?;
            let mm = crate::chem::pdb::MacroMol::from_file(&path)?;
            Ok(Value::Str(crate::chem::pdb::macro_passport(&mm, &path)))
        }
        "dock" => {
            need(args, 2, name)?;
            let smiles = expect_str(&args[0], name, "лиганд (SMILES или имя)")?;
            let path = expect_str(&args[1], name, "путь к PDB-белку")?;
            let mm = crate::chem::pdb::MacroMol::from_file(&path)?;
            let params = crate::chem::dock::DockParams::default();
            let res = crate::chem::dock::dock(&smiles, &mm, &params, &crate::chem::dock::PocketSpec::Auto)?;
            Ok(Value::Str(crate::chem::dock::dock_text(&res, &smiles)))
        }
        "logdet" => {
            need(args, 1, name)?;
            let m = matrix_arg(args, 0)?;
            let (sign, ln_abs) = m.logdet()?;
            Ok(Value::Str(logdet_report(&m, sign, ln_abs)))
        }
        "lsolve" => {
            need(args, 2, name)?;
            let m = matrix_arg(args, 0)?;
            let b: Vec<Complex> = match args.get(1) {
                Some(Value::List(items)) => {
                    let mut v = Vec::with_capacity(items.len());
                    for it in items {
                        match it.as_f64() {
                            Some(x) => v.push(Complex::new(x, 0.0)),
                            None => {
                                return Err(format!(
                                    "{name}: b должен быть списком чисел, получено {it}"
                                ))
                            }
                        }
                    }
                    v
                }
                Some(Value::Matrix(bm)) => {
                    // Матрица-столбец N×1
                    if bm.cols != 1 {
                        return Err(format!(
                            "{name}: b — список или матрица-столбец N×1, получено {}×{}",
                            bm.rows, bm.cols
                        ));
                    }
                    (0..bm.rows)
                        .map(|r| bm.get(r, 0))
                        .collect()
                }
                Some(other) => {
                    return Err(format!(
                        "{name}: b — список чисел или матрица-столбец, получено {other}"
                    ))
                }
                None => return Err("не хватает аргументов".into()),
            };
            let x = m.solve_lu(&b)?;
            // Невязка ||Ax − b||∞ / ||b||∞
            let n = m.rows;
            let mut resid = 0.0f64;
            let mut bnorm = 0.0f64;
            for r in 0..n {
                let mut ax = Complex::ZERO;
                for c in 0..n {
                    ax = ax.add(m.get(r, c).mul(x[c]));
                }
                let d = ax.sub(b[r]).abs();
                resid = resid.max(d);
                bnorm = bnorm.max(b[r].abs());
            }
            let rel = if bnorm > 0.0 { resid / bnorm } else { resid };
            let mut items: Vec<Value> = Vec::with_capacity(x.len());
            for xi in &x {
                items.push(complex_to_value(*xi));
            }
            items.push(Value::Str(format!(
                " LU+эквилибровка: невязка {rel:.2e}"
            )));
            Ok(Value::List(items))
        }

        other => Err(format!("неизвестная функция «{other}» (каталог: calc funcs)")),
    }
}

fn resolve_chem_element(v: &Value, name: &str) -> Result<&'static crate::universal_chem::Element, String> {
    match v {
        Value::Scalar(z) => {
            if *z >= 1.0 && *z <= 118.0 && z.fract() == 0.0 {
                crate::universal_chem::get_element_by_z(*z as u8)
                    .ok_or_else(|| format!("{name}: элемент с Z={z} не найден"))
            } else {
                Err(format!("{name}: Z — целое число от 1 до 118, получено {z}"))
            }
        }
        Value::Str(s) => {
            crate::universal_chem::get_element_by_symbol(s)
                .ok_or_else(|| format!("{name}: неизвестный химический символ «{s}»"))
        }
        other => Err(format!("{name}: ожидался символ элемента (строка) или атомный номер Z (число), получено {other}")),
    }
}

/// Поэлементное применение функции к первому списку в аргументх.
fn map_over_lists(name: &str, args: &[Value]) -> Result<Value, String> {
    for (i, a) in args.iter().enumerate() {
        if let Value::List(items) = a {
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                let mut call_args = args.to_vec();
                call_args[i] = it.clone();
                out.push(call_function(name, &call_args)?);
            }
            return Ok(Value::List(out));
        }
    }
    unreachable!("map_over_lists вызывается только при наличии списка")
}

// ─── Законы физики как функции (v0.93.0) ───────────────────────────────────

/// Вычислить закон с позиционными аргументами (порядок реестра).
/// Правила:
/// • голое число наследует единицу переменной реестра: pendulum(0.5) ≡ L = 0.5 м;
/// • величина идёт как есть (единицы в Display): rc_tau(10e3 ohm, 100e-6 F);
/// • недостающие хвостовые аргументы — умолчания реестра: pendulum() ≡ pendulum(1 м);
/// • строка — готовое значение: pendulum("0.5 m");
/// • вектор [x1, x2, …] (матрица-строка/столбец) — СВИП параметра:
///   pendulum([0.5, 1, 2]) → [T(0.5), T(1), T(2)].
/// Возвращается ЗНАЧЕНИЕ (не скрипт): мост реестр → арифметическое ядро.
fn law_eval(law: &crate::calc::scriptgen::LawSpec, args: &[Value]) -> Result<Value, String> {
    // векторный аргумент → свип параметра (только один вектор за раз)
    let is_vec = |a: &Value| {
        matches!(a, Value::Matrix(m) if m.rows == 1 || m.cols == 1)
    };
    if let Some(vi) = args.iter().position(is_vec) {
        if args.iter().skip(vi + 1).any(is_vec) {
            return Err(format!(
                "закон «{}»: свип — по одному векторному аргументу за раз (получено несколько)",
                law.id
            ));
        }
        let m = match &args[vi] {
            Value::Matrix(m) => m,
            _ => unreachable!(),
        };
        let n = m.rows.max(m.cols);
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            // строка: (0, i); столбец: (i, 0)
            let (r, c) = if m.rows == 1 { (0, i) } else { (i, 0) };
            let x = m.get(r, c).re;
            let mut sub = args.to_vec();
            sub[vi] = Value::Scalar(x);
            out.push(law_eval(law, &sub)?);
        }
        return Ok(Value::List(out));
    }
    if args.len() > law.vars.len() {
        return Err(format!(
            "закон «{}» принимает не больше {} аргумент(ов): {} — по порядку реестра",
            law.id,
            law.vars.len(),
            law.vars.iter().map(|v| v.name).collect::<Vec<_>>().join(", ")
        ));
    }
    let mut overrides: Vec<(String, String)> = Vec::with_capacity(args.len());
    for (i, arg) in args.iter().enumerate() {
        let var = &law.vars[i];
        let raw = match arg {
            Value::Scalar(x) => {
                if var.unit.is_empty() {
                    format!("{x}")
                } else {
                    format!("{x} {}", var.unit)
                }
            }
            // величина/точное целое: Display несёт единицы и круглится парсером
            Value::Quantity(..) | Value::BigInt(_) => format!("{arg}"),
            // строка — готовое значение с единицей
            Value::Str(st) => st.clone(),
            other => {
                return Err(format!(
                    "закон «{}»: аргумент «{}» — число (получено {})",
                    law.id,
                    var.name,
                    other.type_name()
                ))
            }
        };
        overrides.push((var.name.to_string(), raw));
    }
    let expr = crate::calc::scriptgen::law_expression(law, &overrides)?;
    // выражение закрыто (числа + константы CODATA) — переменные сессии не нужны
    let empty = std::collections::HashMap::new();
    crate::calc::parser::parse_and_eval(&expr, &empty)
        .map_err(|e| format!("закон «{}»: {e}", law.id))
}

/// Каталог функций для `calc funcs [фильтр]`.
pub fn catalog(filter: &str) -> String {
    let groups: &[(&str, &[&str])] = &[
        ("математика", &[
            "sin cos tan asin acos atan atan2 sinh cosh tanh asinh acosh atanh",
            "sind cosd tand atan2d (градусы)",
            "exp ln log log2 log10 sqrt cbrt hypot pow abs floor ceil round sign fract",
            "deg2rad rad2deg min max mean median std clamp",
            "linspace(a, b, n) — сетка траектории: n точек от a до b включительно (n ≤ 65536)",
        ]),
        ("специальная", &["gamma lgamma erf erfc zeta beta"]),
        ("теория чисел", &[
            "gcd lcm mod is_prime next_prime prev_prime factorize divisors fib binomial catalan",
        ]),
        ("матрицы/квант", &[
            "transpose det inv pinv trace expm charpoly eigen identity eye",
            "rot2 rotx roty rotz so_gen kron dagger tridiag",
            "квант O: schrodinger(H, psi0, t[, hbar]) — уравнение Шрёдингера",
            "pauli_x pauli_y pauli_z hadamard — пресеты кубитных вентилей",
            "A^(-1) — обратная; expm(J·θ) — вращение Ли; kron — тензорное произведение",
        ]),
        ("триты", &["trits trit_val trit_and trit_or trit_not"]),
        ("температура", &["degC(x) degF(x) — конструкторы: degC(25) to degF"]),
        ("астрономия", &[
            "sun_lon sun_ra sun_dec (y, m, d[, час UTC])",
            "moon_lon moon_lat moon_dist moon_phase moon_illum moon_age moon_ra moon_dec",
            "planet_lon planet_dist (\"mars\", y, m, d[, час])",
            "sunrise sunset (lat, lon, y, m, d) → часы UTC",
        ]),
        ("геодезия", &[
            "dist bearing (lat1, lon1, lat2, lon2) — км / град",
            "midpoint dest earth_radius",
        ]),
        ("визуализация (цикл U, сессия-8)", &[
            "viz(x) — авто: число → Белл/шкала, список → вероятности/спектр, матрица → теплокарта",
            "viz_bell(I) — радар CGLMP: классика 2.0 · Цирельсон 2.828 · Ацин 2.915 · квкварт 2.973",
            "viz_scale(x) — лог-шкала Вселенной в метрах: Планковская длина → наблюдаемая Вселенная",
            "viz_prob([p…]) — Born-вероятности столбцами · viz_bars([v…]) — спектр значений",
            "viz_matrix(M) — теплокарта матрицы до 64×64: красный ≥ 0, синий < 0",
            "вывод — SVG 1.1 строкой: сохраняйте в файл и открывайте глазами",
        ]),
        ("суверенный рендер (сессия-9)", &[
            "viz_graph(A, \"м1, м2, …\") — силовая укладка Fruchterman–Reingold: смежность → схема",
            "узлы: радиус/цвет по степени · рёбра: толщина по весу · компоненты union-find",
            "viz_field(M) — изолинии marching squares (7 уровней) поверх дивергентной карты",
            "viz_surf(M) — 3D-поверхность: поворот, ортоскопия, painter's-алгоритм (до 48×48)",
            "математика вырезана из Graphviz/NetworkX/Matplotlib — ноль зависимостей, чистый Rust",
        ]),
        ("GIS-ядро на тритах (сессия-12)", &[
            "de9im(кольцоA, кольцоB[, k]) — топология OGC 9-IM: 9 тритов → сбалансированный код",
            "матрица «−0+…» · предикаты: intersects/contains/within/touches/overlaps/equals…",
            "geo_pred(A, B[, k]) — список сработавших предикатов",
            "точность без epsilon: координаты квантуются в тритную решётку 3⁻ᵏ (k ≤ 12)",
            "viz_iso3(стек срезов, уровень) — изоповерхность: marching tetrahedra, watertight",
            "кольцо — матрица Nx2 [x,y; x,y; …] · стек — список матриц либо блочная (nz·ny)×nx",
        ]),
        ("сверхдиапазонные вероятности (сессия-13)", &[
            "logpow(b, x) — b^x за пределами f64: значение-лог-вероятность (10^… = 3^…)",
            "logpow(2, -1e15) → 10^(-3.010e+14) — то, что раньше выпадало в 0.0",
            "logp(p) — поднять число > 0 в лог-домен · expneg(x) = e^(-x) — больцмановский штраф",
            "lpsum(p1, p2, … | [ряд]) — сумма ряда сверхмалых вероятностей (Z = Σ P_i)",
            "арифметика: P+Q (logaddexp) · P−Q · P·Q · P/Q · P^k · 1−P — всё без потерь порядков",
            "regress(N_бит, Δt_с[, Вт, К]) — регрессия в прошлое с сохранением опыта",
            "P(опыт|регрессия) = 2^(-N) · P(регрессия) = e^(-ΔS/k_B), ΔS = σ·Δt, σ = P/T",
            "умолчания: мозг 20 Вт / 310.15 К · бюджет Вселенной 10^(140.91) испытаний",
            "тритная глубина d: P = 3^(-d) — сколько тритов сбалансированной тройки «против»",
        ]),
        ("переплетение модулей (сессия-14)", &[
            "spatial27(конверты Nx6, окно[, k]) — 27-дерево: тритное деление 3×3×3 XYZ",
            "окно = [minx,miny,minz,maxx,maxy,maxz] → номера строк, пересекающих окно",
            "hexring(k) / hexdisk(r) — гексы трит-спирали · hexdist(q1,r1,q2,r2) — дистанция",
            "hexid(q, r) — тритный адрес: s спирали + 6-тритный блок (729 состояний)",
            "de9im_line(линия Nx2, кольцо[, k]) — relate LineString×Polygon",
            "eteria_field([seed[, n]]) — карта высот Этерии → viz_field(eteria_field())",
            "eteria_hex([seed[, R]]) — [q, r, высота, биом] · viz_hex(eteria_hex()) — SVG-карта",
            "eteria_boltz([seed[, R[, scale]]]) — Z = Σ e^(-E_i) в лог-домене за f64",
            "конвейер: diamond-square → гексы H3 → биомы → LogProb → lpsum → viz_hex",
        ]),
        ("муха над картой (сессия-15)", &[
            "tin(точки Nx2[, k]) — TIN Делоне: Bowyer-Watson, детерминант i128, ноль округлений",
            "муха водит порядок вставки: sin(i·Φ mod 2π) — Gold Phase Lock из universal_letters",
            "сокруглённости и adversarial-порядки развязаны золотой фазой; BW не циклится в принципе",
            "eteria_tin([seed[, n[, scale]]]) — мир Этерии как TIN: золотой посев + Муха + LogProb",
            "конвейер: Φ-спираль посева → delaunay → высоты terrain_detail → Z = Σ e^(−E_i) за f64",
            "h3children(q, r) / h3parent(q, r) — дробление апертурой 9 = 3²: точное решёточное",
            "масштабирование (3q+i, 3r+j), адрес +2 трита на уровень — ноль таблиц против √7 у H3",
            "de9im_ll(линияA Nx2, линияB Nx2[, k]) — relate LineString×LineString: реляционная",
            "матрица GEO-ядра замкнута: точка/кольцо/строка × точка/кольцо/строка",
        ]),
        ("кристалл рельефа (сессия-16)", &[
            "сессия-16 ускоряет Муху: history-DAG локализация + BFS-полость по смежности",
            "tin() теперь O(n log n) в среднем: спуск по истории мёртвых треугольников (Гибас—Кемани—Сугихара)",
            "кросс-аудит: delaunay_bruteforce — эталон, тесты сверяют обе машины побитово",
            "27-дерево в TIN: конверты треугольников → height_at трит-путём (spatial27 внутри)",
            "eteria_tin_pts([seed[, n]]) — вершины золотого посева Nx3 [x, y, высота]",
            "viz_tin(точки Nx2|Nx3[, k][, \"заголовок\"]) — SVG-рельеф: биомы по средней высоте треугольников",
            "viz_tin(eteria_tin_pts(7, 256)) — карта Этерии триангуляцией Мухи (конвейер одной строкой)",
            "poly(кольцо, дырка1, …) — полигон с дырками: первое кольцо внешнее, остальные — дырки",
            "de9im_poly(A, B[, k]) — relate полигон×полигон: A и B — кольцо Nx2 или poly(…)",
            "дырка = внешность: B в дырке A → disjoint · B = дырка A → touches · идентичные → equals",
        ]),
        ("три горизонта (сессия-17)", &[
            "tin_isolines(точки Nx3[, уровни][, сетка][, k]) — изолинии TIN: marching squares по height_at",
            "уровни — число равномерных (умолчание 5) или список; сетка ∈ [16, 384] ячеек на сторону",
            "эталон в отчёте: ТОЧНОЕ сечение треугольников плоскостью z = уровень — цепочки watertight",
            "аудит честности: длина MS сходится к точной (Δ%); планарный рельеф — совпадение побитово",
            "viz_isolines(точки Nx3[, уровни][, k][, \"заголовок\"]) — SVG: биомы TIN + изолинии с подписями",
            "viz_isolines(eteria_tin_pts(7, 256)) — карта изолиний Этерии конвейером одной строкой",
            "h3addr(face, q, r[, res]) — мировой адрес ячейки НА уровне res (умолчание 2 = 13 тритов)",
            "h3addr строит адрес снизу вверх: родители → спираль res-0 → +2 трита на уровень · roundtrip в отчёте",
            "h3cell(\"адрес\") — разбор: грань, res, спираль, путь уточнений, координаты уровня",
            "res→2: 81 внук на ячейку, центр-потомок (9q, 9r); координаты ⇄ адрес на любом уровне ≤ 6",
            "line(точки Nx2) — открытая ломаная · poly(кольца…) — полигон · gc(…) — GeometryCollection",
            "de9im_gc(A, B[, k]) — relate СМЕШАННЫХ размерностей: union-семантика OGC, dim(∪) = max dim",
            "матрица: точка → 0, линия/площадь → +1; дырки = внешность; коллекция из одного == сама геометрия",
        ]),
        ("квантовая химия (118 элементов)", &[
            "chem_elem(Z_или_символ) — паспорт элемента: Z, молярная масса, электроотрицательность, блок, архетип",
            "chem_bond(elem1, elem2) — тип связи (ковалентный/ионный/металлический), трит {-1,0,+1}, фазовый резонанс",
            "chem_mass(\"формула\") — стехиометрическая молярная масса (H2O, H2SO4, Fe2(SO4)3, Ca(OH)2, CH3COOH)",
            "chem_formula(\"формула\") — полный анализ: молярная масса, электроны, число атомов и массовые доли %",
            "chem_archetype(elem) — квантово-химический архетип (12 классов), золотая фаза φ(Z) = (Z·Φ) mod 2π",
            "chem_smiles(\"CC(=O)Oc1ccccc1C(=O)O\") — паспорт молекулы из SMILES: топология, кольца, дескрипторы Липински, заряды Гастайгера",
            "chem_3d(\"кофеин\") — 3D-конформер: длины связей, габариты, гибридизации sp/sp²/sp³, планарность колец",
            "bio_eval(\"дофамин\", \"серотонин\") — био-контур: позы, E_vdW/E_HB/E_elec, ΔG и Kd в лог-домене, 27-дерево, DE-9IM",
            "pdb_info(\"3ptb.pdb\") — паспорт белка: цепочки, состав, вторичная структура, NET-заряд",
            "dock(\"NC(=N)c1ccccc1\", \"3ptb.pdb\") — гибкий докинг в PDB-карман: MC/SA, кручения трит-секторами, Kd, redocking-RMSD",
            "logdet(A) — определитель в лог-домене: sign, ln|det|, тритная глубина, cond_∞ — за пределами f64",
            "lsolve(A, b) — LU-решение с эквилибровкой строк: матрицы с разбросом 1e-12…1e12",
        ]),
        ("голос учёного (роторы + триединное ядро)", &[
            "speak(\"текст_или_формула\") — научное чтение результата: токенизатор → языковой ротор букв мира",
            "(104k строк No-Mul ASM) → химротор 118 элементов → триединое ядро; трит-вердикт {-1,0,+1}",
            "примеры: speak(\"C6H12O6\") · speak(\"H2O\") · speak(\"энергия резонанса фазы\")",
        ]),
        ("аб-иницио квантовая химия (RHF/STO-3G)", &[
            "hf(\"молекула\"[, R_Бор]) — полный Хартри–Фок из POLER-ERI: E_total, ε_MO, SCF-итерации",
            "молекулы: H2 (R=1.4), He, LiH (R=3.0), HeH+ (R=1.4632); длина связи в Бора",
            "эталоны: E(H2)=−1.116664 Ha (учебник −1.1167) · E(He)=−2.807844 Ha (HSP-1969 −2.8078)",
            "примеры: hf(\"H2\") · hf(\"LiH\", 3.0) · hf(\"HeH+\")",
        ]),
    ];
    let mut out = String::new();
    for (g, lines) in groups {
        if !filter.is_empty()
            && !g.contains(&filter.to_lowercase())
            && !lines.join(" ").contains(&filter.to_lowercase())
        {
            continue;
        }
        out.push_str(&format!("── {g}\n"));
        for l in *lines {
            out.push_str(&format!("  {l}\n"));
        }
    }
    // v0.93.0: законы физики — вызываемые функции (сигнатуры из реестра)
    {
        let g = "законы физики (вызываемые)";
        let lines: Vec<String> = crate::calc::scriptgen::LAWS
            .iter()
            .map(|l| {
                if l.vars.is_empty() {
                    format!("{}() — {}", l.id, l.title)
                } else {
                    format!(
                        "{}({}) — {}",
                        l.id,
                        l.vars.iter().map(|v| v.name).collect::<Vec<_>>().join(", "),
                        l.title
                    )
                }
            })
            .collect();
        let joined = lines.join(" ");
        if filter.is_empty()
            || g.contains(&filter.to_lowercase())
            || joined.to_lowercase().contains(&filter.to_lowercase())
        {
            out.push_str(&format!("── {g}\n"));
            for l in &lines {
                out.push_str(&format!("  {l}\n"));
            }
            out.push_str("  аргументы — по порядку реестра; голое число наследует единицу переменной;\n");
            out.push_str("  пропущенные хвостовые — умолчания реестра: pendulum() ≡ pendulum(1 м)\n");
            out.push_str("  свип параметра — списком: pendulum([0.5, 1, 2]) → [T(0.5), T(1), T(2)]\n");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: &[Value]) -> Result<Value, String> {
        call_function(name, args)
    }
    fn s(v: f64) -> Value {
        Value::Scalar(v)
    }
    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn is_function_registry() {
        assert!(is_function("sin"));
        assert!(is_function("expm"));
        assert!(is_function("moon_illum"));
        assert!(!is_function("sinx"));
        assert!(!is_function("x"));
        assert!(!is_function("pi")); // константа, не функция
        // v0.93.0: законы — функции (точный id), частичное — нет
        assert!(is_function("pendulum"));
        assert!(is_function("rc_tau"));
        assert!(is_function("doppler"));
        assert!(is_function("signal_wavelength"));
        assert!(!is_function("kepler")); // частичное совпадение НЕ функция
        assert!(is_function("linspace"));
    }

    // ─── v0.93.0: законы физики как функции (Фаза 3 протокола POLER) ───
    #[test]
    fn laws_as_functions_core_battery() {
        // pendulum() — умолчание реестра L = 1 м: T = 2π√(1/g0) ≈ 2.00641 с
        let t = call("pendulum", &[]).unwrap().as_f64().unwrap();
        assert!((t - 2.00641).abs() < 1e-4, "T = {t}");
        // pendulum(0.5) — голое число наследует метры: T = 2π√(0.5/g0) ≈ 1.41875 с
        let t = call("pendulum", &[s(0.5)]).unwrap().as_f64().unwrap();
        assert!((t - 1.41875).abs() < 1e-4, "T = {t}");
        // rc_tau(10e3, 100e-6) = 1 с (ом·фарад = секунда)
        let tau = call("rc_tau", &[s(10e3), s(100e-6)]).unwrap().as_f64().unwrap();
        assert!((tau - 1.0).abs() < 1e-12, "τ = {tau}");
        // doppler(440, 343, 30) = 440·343/(343−30) ≈ 482.62 Гц
        let f = call("doppler", &[s(440.0), s(343.0), s(30.0)])
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((f - 440.0 * 343.0 / 313.0).abs() < 1e-9, "f = {f}");
        // signal_wavelength(2.4e9) = c/f ≈ 0.124913 м
        let lam = call("signal_wavelength", &[s(2.4e9)]).unwrap().as_f64().unwrap();
        assert!((lam - 0.124913).abs() < 1e-5, "λ = {lam}");
        // слишком много аргументов — внятная ошибка со списком переменных
        let err = call("pendulum", &[s(1.0), s(2.0)]).unwrap_err();
        assert!(err.contains("pendulum") && err.contains("L"), "err = {err}");
        // матрица — не аргумент закона
        let err = call("pendulum", &[Value::Matrix(Matrix::identity(2))]).unwrap_err();
        assert!(err.contains("число"), "err = {err}");
        // несуществующий закон — обычная ошибка парсера (не паника)
        assert!(call("no_such_law", &[]).is_err());
    }

    #[test]
    fn law_parameter_sweep_over_list() {
        // свип параметра: pendulum([0.5, 1, 2]) → [T(0.5), T(1), T(2)]
        let sweep = call(
            "pendulum",
            &[Value::List(vec![s(0.5), s(1.0), s(2.0)])],
        )
        .unwrap();
        match sweep {
            Value::List(items) => {
                assert_eq!(items.len(), 3);
                let t05 = items[0].as_f64().unwrap();
                let t1 = items[1].as_f64().unwrap();
                let t2 = items[2].as_f64().unwrap();
                assert!((t05 - 1.41875).abs() < 1e-4);
                // T ∝ √L: T(2)/T(0.5) = 2 точно
                assert!((t2 / t05 - 2.0).abs() < 1e-12, "T(2)/T(0.5) = {}", t2 / t05);
                assert!(t1 > t05 && t2 > t1);
            }
            other => panic!("ожидался список, получено {other:?}"),
        }
    }

    #[test]
    fn law_parameter_sweep_over_matrix_vector() {
        // CLI-синтаксис свипа: [0.5, 1, 2] — матрица-строка (литерал в
        // выражении парсится как матрица, не как список)
        let v = Matrix::from_rows(&[vec![0.5, 1.0, 2.0]]).unwrap();
        let sweep = call("pendulum", &[Value::Matrix(v)]).unwrap();
        match sweep {
            Value::List(items) => {
                assert_eq!(items.len(), 3);
                let t05 = items[0].as_f64().unwrap();
                let t2 = items[2].as_f64().unwrap();
                assert!((t05 - 1.41875).abs() < 1e-4);
                // T ∝ √L: T(2)/T(0.5) = 2 точно
                assert!((t2 / t05 - 2.0).abs() < 1e-12);
            }
            other => panic!("ожидался список, получено {other:?}"),
        }
        // столбец — тоже вектор
        let v = Matrix::from_rows(&[vec![0.5], vec![1.0]]).unwrap();
        let sweep = call("pendulum", &[Value::Matrix(v)]).unwrap();
        assert!(matches!(sweep, Value::List(ref items) if items.len() == 2));
        // два вектора сразу — внятная ошибка
        let a = Matrix::from_rows(&[vec![440.0, 500.0]]).unwrap();
        let b = Matrix::from_rows(&[vec![30.0, 40.0]]).unwrap();
        let err = call("doppler", &[Value::Matrix(a), Value::Matrix(b)]).unwrap_err();
        assert!(err.contains("одному векторному"), "err = {err}");
        // настоящая матрица 2×2 — не аргумент закона (не вектор)
        let m = Matrix::from_rows(&[vec![1.0, 2.0], vec![3.0, 4.0]]).unwrap();
        let err = call("pendulum", &[Value::Matrix(m)]).unwrap_err();
        assert!(err.contains("число"), "err = {err}");
    }

    #[test]
    fn linspace_core() {
        // целые точки включительно, концы точны
        let v = call("linspace", &[s(0.0), s(1.0), s(5.0)]).unwrap();
        match v {
            Value::List(items) => {
                let xs: Vec<f64> = items.iter().map(|i| i.as_f64().unwrap()).collect();
                assert_eq!(xs, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
            }
            other => panic!("ожидался список, получено {other:?}"),
        }
        // n = 1 → [a]; симметрия; обратный порядок
        let one = call("linspace", &[s(2.0), s(9.0), s(1.0)]).unwrap();
        assert!(matches!(one, Value::List(ref items) if items.len() == 1));
        let v = call("linspace", &[s(-1.0), s(1.0), s(3.0)]).unwrap();
        match v {
            Value::List(items) => {
                let xs: Vec<f64> = items.iter().map(|i| i.as_f64().unwrap()).collect();
                assert_eq!(xs, vec![-1.0, 0.0, 1.0]);
            }
            other => panic!("ожидался список, получено {other:?}"),
        }
        let v = call("linspace", &[s(1.0), s(-1.0), s(5.0)]).unwrap();
        match v {
            Value::List(items) => {
                let xs: Vec<f64> = items.iter().map(|i| i.as_f64().unwrap()).collect();
                assert_eq!(xs, vec![1.0, 0.5, 0.0, -0.5, -1.0]);
            }
            other => panic!("ожидался список, получено {other:?}"),
        }
        // мусорное n — внятная ошибка
        assert!(call("linspace", &[s(0.0), s(1.0), s(0.0)]).is_err());
        assert!(call("linspace", &[s(0.0), s(1.0), s(2.5)]).is_err());
        assert!(call("linspace", &[s(0.0), s(1.0), s(1e7)]).is_err());
    }

    #[test]
    fn trajectory_composition_end_to_end() {
        // Фаза 3 протокола POLER: ось времени → поэлементная тригонометрия.
        // cos([0, π/2, π, 3π/2, 2π]) = [1, 0, −1, 0, 1] — маятниковая
        // траектория x(t) = cos(2πt/T) на сетке linspace(0, T, 5).
        let pi = std::f64::consts::PI;
        let t = Value::List(vec![s(0.0), s(pi / 2.0), s(pi), s(3.0 * pi / 2.0), s(2.0 * pi)]);
        let x = call("cos", &[t]).unwrap();
        match x {
            Value::List(items) => {
                let xs: Vec<f64> = items.iter().map(|i| i.as_f64().unwrap()).collect();
                assert!((xs[0] - 1.0).abs() < 1e-12);
                assert!(xs[1].abs() < 1e-12);
                assert!((xs[2] + 1.0).abs() < 1e-12);
                assert!(xs[3].abs() < 1e-12);
                assert!((xs[4] - 1.0).abs() < 1e-12);
            }
            other => panic!("ожидался список, получено {other:?}"),
        }
        // полная композиция через парсер: сетка + масштаб + косинус
        use std::collections::HashMap;
        let mut vars = HashMap::new();
        vars.insert("t".into(), call("linspace", &[s(0.0), s(1.0), s(5.0)]).unwrap());
        let expr = crate::calc::parser::parse("cos(2*pi*t)", false).unwrap();
        let val = crate::calc::parser::eval(&expr, &mut vars).unwrap();
        match val {
            Value::List(items) => {
                assert_eq!(items.len(), 5);
                let xs: Vec<f64> = items.iter().map(|i| i.as_f64().unwrap()).collect();
                assert!((xs[0] - 1.0).abs() < 1e-12);
                assert!(xs[1].abs() < 1e-12);
                assert!((xs[2] + 1.0).abs() < 1e-12);
            }
            other => panic!("ожидался список, получено {other:?}"),
        }
    }

    #[test]
    fn special_functions_anchors() {
        // Γ(0.5) = √π
        assert!(close(gamma_impl(0.5).unwrap(), std::f64::consts::PI.sqrt(), 1e-13));
        // Γ(6) = 120
        assert!(close(gamma_impl(6.0).unwrap(), 120.0, 1e-12));
        // Γ(−0.5) = −2√π (отражение)
        assert!(close(gamma_impl(-0.5).unwrap(), -2.0 * std::f64::consts::PI.sqrt(), 1e-12));
        // цикл N: полюса — расходимость на расширенной прямой (не ошибка)
        assert!(gamma_impl(-1.0).unwrap().is_infinite());
        assert!(gamma_impl(-2.0).unwrap().is_infinite());
        assert!(gamma_impl(0.0).unwrap().is_infinite());

        // erf: документированная точность A&S — 1.5e-7
        assert!(close(erf_impl(1.0), 0.8427007929497149, 1.5e-7));
        assert!(close(erf_impl(0.5), 0.5204998778130465, 1.5e-7));
        assert!(close(erf_impl(2.0), 0.9953222650189527, 1.5e-7));
        assert!(close(erf_impl(-1.0), -0.8427007929497149, 1.5e-7));
        assert!(close(erf_impl(0.0), 0.0, 1e-15));
        // erfc(1) — в зоне уверенности A&S (при x=3 катастрофично сокращение)
        assert!(close(1.0 - erf_impl(1.0), 0.15729920705028513, 1e-6));

        // ζ (Эйлер–Маклорен):
        assert!(close(zeta_impl(2.0).unwrap(), std::f64::consts::PI.powi(2) / 6.0, 1e-12));
        assert!(close(zeta_impl(4.0).unwrap(), std::f64::consts::PI.powi(4) / 90.0, 1e-12));
        assert!(close(zeta_impl(0.0).unwrap(), -0.5, 1e-15));
        assert!(close(zeta_impl(-1.0).unwrap(), -1.0 / 12.0, 1e-10));
        assert!(close(zeta_impl(-2.0).unwrap(), 0.0, 1e-15)); // тривиальный нуль
        assert!(close(zeta_impl(3.0).unwrap(), 1.2020569031595942, 1e-10));
        assert!(zeta_impl(1.0).unwrap().is_infinite()); // полюс: ζ(1) = +∞

        // beta(2,3) = 1/12
        let b = call("beta", &[s(2.0), s(3.0)]).unwrap();
        assert!(close(b.as_f64().unwrap(), 1.0 / 12.0, 1e-12));
    }

    // РЕГРЕССИЯ: потерянный внешний множитель t в Хорнере erf давал
    // erf(1) ≈ 0.665 (правильно 0.8427).
    #[test]
    fn erf_outer_t_factor_regression() {
        let v = erf_impl(1.0);
        assert!(v > 0.84 && v < 0.85, "erf(1) = {v}");
        let v = call("erf", &[s(1.0)]).unwrap();
        assert!(close(v.as_f64().unwrap(), 0.84270079, 1.5e-7));
    }

    #[test]
    fn trig_domain_checks() {
        // цикл N: домены расширены — «невозможное → возможное».
        // Вне старой области определения функции возвращают КОМПЛЕКСНЫЙ
        // результат или ∞, а не ошибку.
        // asin(2) = π/2 − i·acosh(2)
        match call("asin", &[s(2.0)]).unwrap() {
            Value::Complex(c) => {
                assert!(close(c.re, std::f64::consts::FRAC_PI_2, 1e-12));
                assert!(c.im < 0.0 && close(c.im, -(2.0f64 + 3.0f64.sqrt()).ln(), 1e-12));
            }
            _ => panic!("asin(2) должен быть комплексным"),
        }
        // acos(−1.5) = π − i·acosh(1.5)
        match call("acos", &[s(-1.5)]).unwrap() {
            Value::Complex(c) => {
                assert!(close(c.re, std::f64::consts::PI, 1e-12));
                assert!(c.im < 0.0);
            }
            _ => panic!("acos(−1.5) должен быть комплексным"),
        }
        // ln(−1) = iπ (главная ветвь)
        match call("ln", &[s(-1.0)]).unwrap() {
            Value::Complex(c) => {
                assert!(c.re.abs() < 1e-15);
                assert!(close(c.im, std::f64::consts::PI, 1e-12));
            }
            _ => panic!("ln(−1) должен быть iπ"),
        }
        // sqrt(−4) = 2i
        match call("sqrt", &[s(-4.0)]).unwrap() {
            Value::Complex(c) => {
                assert!(c.re.abs() < 1e-15);
                assert!(close(c.im, 2.0, 1e-12));
            }
            _ => panic!("sqrt(−4) должен быть 2i"),
        }
        // log(0) = −∞ (предел)
        assert_eq!(call("log", &[s(0.0)]).unwrap().as_f64(), Some(f64::NEG_INFINITY));
        // acosh(0.5) = i·acos(0.5) = i·π/3
        match call("acosh", &[s(0.5)]).unwrap() {
            Value::Complex(c) => {
                assert!(c.re.abs() < 1e-15);
                assert!(close(c.im, std::f64::consts::PI / 3.0, 1e-12));
            }
            _ => panic!("acosh(0.5) должен быть i·π/3"),
        }
        // atanh(1) = ∞ (расходимость на границе)
        assert_eq!(call("atanh", &[s(1.0)]).unwrap().as_f64(), Some(f64::INFINITY));
        // угловые величины
        let v = call("sin", &[Value::Quantity(180.0, units::by_name("deg").unwrap())]).unwrap();
        assert!(close(v.as_f64().unwrap(), 0.0, 1e-14));
    }

    #[test]
    fn aggregates() {
        assert!(close(
            call("mean", &[s(1.0), s(2.0), s(3.0), s(4.0)]).unwrap().as_f64().unwrap(),
            2.5,
            1e-15
        ));
        assert!(close(
            call("median", &[s(1.0), s(9.0), s(5.0)]).unwrap().as_f64().unwrap(),
            5.0,
            1e-15
        ));
        assert!(close(
            call("median", &[s(1.0), s(2.0), s(8.0), s(9.0)]).unwrap().as_f64().unwrap(),
            5.0,
            1e-15
        ));
        assert!(close(
            call("std", &[s(2.0), s(4.0), s(4.0), s(4.0), s(5.0), s(5.0), s(7.0), s(9.0)])
                .unwrap()
                .as_f64().unwrap(),
            2.0,
            1e-12
        ));
        assert!(close(
            call("max", &[s(3.0), s(7.0), s(2.0)]).unwrap().as_f64().unwrap(),
            7.0,
            1e-15
        ));
    }

    #[test]
    fn number_theory_calls() {
        assert_eq!(call("is_prime", &[s(97.0)]).unwrap().as_f64().unwrap(), 1.0);
        assert_eq!(call("is_prime", &[s(561.0)]).unwrap().as_f64().unwrap(), 0.0);
        assert_eq!(call("next_prime", &[s(13.0)]).unwrap().as_f64().unwrap(), 17.0);
        match call("factorize", &[s(360.0)]).unwrap() {
            Value::List(items) => {
                assert_eq!(items.len(), 6); // 2,3,3,5,1,1
                assert_eq!(items[0].as_f64().unwrap(), 2.0);
                assert_eq!(items[1].as_f64().unwrap(), 3.0);
            }
            other => panic!("{other:?}"),
        }
        match call("divisors", &[s(12.0)]).unwrap() {
            Value::List(items) => assert_eq!(items.len(), 6),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn list_mapping() {
        match call("sin", &[Value::List(vec![s(0.0), s(1.0)])]).unwrap() {
            Value::List(items) => {
                assert!(close(items[0].as_f64().unwrap(), 0.0, 1e-15));
                assert!(close(items[1].as_f64().unwrap(), 0.8414709848078965, 1e-15));
            }
            other => panic!("{other:?}"),
        }
        // вектор + скаляр — бродкаст
        match call_function("+", &[Value::List(vec![s(1.0), s(2.0)]), s(10.0)]) {
            Err(_) => {} // + — оператор, не функция; проверим через binary_op в parser-тестах
            Ok(v) => panic!("+ не функция: {v:?}"),
        }
    }

    #[test]
    fn temperature_constructors() {
        let c = call("degC", &[s(100.0)]).unwrap();
        let f = call("degF", &[Value::Scalar(0.0)]).unwrap();
        let _ = (c, f);
        // конверсия через parser: degC(100) to degF = 212
        let mut vars = std::collections::HashMap::new();
        let v = super::super::parser::parse_and_eval("degC(100) to degF", &vars).unwrap();
        assert!(close(v.as_f64().unwrap(), 212.0, 1e-9));
        vars.clear();
        let v = super::super::parser::parse_and_eval("degF(32) to degC", &vars).unwrap();
        assert!(close(v.as_f64().unwrap(), 0.0, 1e-9));
    }

    #[test]
    fn astro_calls() {
        // фаза Луны на солнечном затмении 2024-04-08 ~ 0
        let v = call("moon_illum", &[s(2024.0), s(4.0), s(8.0), s(18.35)]).unwrap();
        assert!(v.as_f64().unwrap() < 0.01);
        // солнце в равноденствие
        let v = call("sun_lon", &[s(2024.0), s(3.0), s(20.0), s(3.1)]).unwrap();
        let lon = v.as_f64().unwrap();
        assert!(lon < 1.5 || lon > 358.5, "sun_lon = {lon}");
        // планета
        let v = call("planet_lon", &[Value::Str("mars".into()), s(2024.0), s(6.0), s(1.0)])
            .unwrap();
        assert!((0.0..360.0).contains(&v.as_f64().unwrap()));
        // восход на экваторе в равноденствие
        let v = call("sunrise", &[s(0.0), s(0.0), s(2024.0), s(3.0), s(20.0)]).unwrap();
        assert!(close(v.as_f64().unwrap(), 6.0, 0.25));
    }

    #[test]
    fn geodesy_calls() {
        let d = call("dist", &[s(50.45), s(30.52), s(49.84), s(24.03)]).unwrap();
        let d = d.as_f64().unwrap();
        assert!(d > 450.0 && d < 475.0, "{d}");
        let b = call("bearing", &[s(0.0), s(0.0), s(0.0), s(10.0)]).unwrap();
        assert!(close(b.as_f64().unwrap(), 90.0, 1e-6));
        match call("midpoint", &[s(0.0), s(10.0), s(0.0), s(20.0)]).unwrap() {
            Value::List(items) => {
                assert!(close(items[1].as_f64().unwrap(), 15.0, 1e-9));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn arity_errors() {
        assert!(call("sin", &[]).is_err());
        assert!(call("atan2", &[s(1.0)]).is_err());
        assert!(call("gcd", &[s(1.0)]).is_err());
        assert!(call("nothing_here", &[]).is_err());
        assert!(call_function("expm", &[s(1.0)]).is_err()); // не матрица
    }

    #[test]
    fn catalog_lists() {
        assert!(catalog("").contains("gamma"));
        assert!(catalog("").contains("moon_illum"));
        assert!(!catalog("zeta").is_empty());
    }

    // ===== ГРАБЛИ сессии-4 → исправления сессии-5 =====

    #[test]
    fn sign_zero_returns_zero() {
        // было: sign(0) = 1.0 (f64::signum отдаёт 1.0 для +0.0)
        // — ломало симметрию разностей CGLMP-сканов
        assert_eq!(call("sign", &[s(0.0)]).unwrap(), s(0.0));
        assert_eq!(call("sign", &[s(-0.0)]).unwrap(), s(0.0));
        assert_eq!(call("sign", &[s(5.0)]).unwrap(), s(1.0));
        assert_eq!(call("sign", &[s(-5.0)]).unwrap(), s(-1.0));
    }

    #[test]
    fn abs_vector_and_frobenius_norm() {
        // вектор-строка [3, 4] → ‖v‖₂ = 5 (было: ошибка «не скаляр»)
        let m = Matrix::from_rows(&[vec![3.0, 4.0]]).unwrap();
        assert_eq!(call("abs", &[Value::Matrix(m)]).unwrap(), s(5.0));
        // вектор-столбец [3; 4]
        let m = Matrix::from_rows(&[vec![3.0], vec![4.0]]).unwrap();
        assert_eq!(call("abs", &[Value::Matrix(m)]).unwrap(), s(5.0));
        // матрица [1, 2; 3, 4] → ‖A‖_F = sqrt(1+4+9+16) = sqrt(30)
        let m = Matrix::from_rows(&[vec![1.0, 2.0], vec![3.0, 4.0]]).unwrap();
        let v = match call("abs", &[Value::Matrix(m)]).unwrap() {
            Value::Scalar(x) => x,
            other => panic!("не скаляр: {other:?}"),
        };
        assert!(close(v, 30.0f64.sqrt(), 1e-12), "{v}");
        // 1×1 по-прежнему модуль элемента
        let m = Matrix::from_rows(&[vec![-7.0]]).unwrap();
        assert_eq!(call("abs", &[Value::Matrix(m)]).unwrap(), s(7.0));
    }

    #[test]
    fn eye_beyond_64_qutrit_pipelines() {
        // было: eye(n) ≤ 64 — кутритные конвейеры 3^4=81, 3^6=729 не собирались
        let v = call("eye", &[s(81.0)]).unwrap();
        match v {
            Value::Matrix(m) => {
                assert_eq!((m.rows, m.cols), (81, 81));
                // единичная: след = n
                let tr: f64 = (0..81).map(|i| m.get(i, i).re).sum();
                assert!(close(tr, 81.0, 1e-12));
            }
            other => panic!("не матрица: {other:?}"),
        }
        let v = call("eye", &[s(729.0)]).unwrap();
        match v {
            Value::Matrix(m) => assert_eq!((m.rows, m.cols), (729, 729)),
            other => panic!("не матрица: {other:?}"),
        }
        // потолок 4096 остаётся защитой dense-памяти (268 МБ)
        assert!(call("eye", &[s(4097.0)]).is_err());
        assert!(call("eye", &[s(0.0)]).is_err());
        assert!(call("eye", &[s(2.5)]).is_err());
    }

    #[test]
    fn viz_session9_sovereign_render_calls() {
        // Сессия-9: реестр + диспетчер для графа/поля/поверхности.
        assert!(is_function("viz_graph"));
        assert!(is_function("viz_field"));
        assert!(is_function("viz_surf"));
        assert!(!is_function("viz_graf"));

        // K4 с метками одной строкой через запятую (грабля 16).
        let k4 = Value::Matrix(
            Matrix::from_rows(&[
                vec![0.0, 1.0, 1.0, 1.0],
                vec![1.0, 0.0, 1.0, 1.0],
                vec![1.0, 1.0, 0.0, 1.0],
                vec![1.0, 1.0, 1.0, 0.0],
            ])
            .unwrap(),
        );
        let v = call(
            "viz_graph",
            &[k4.clone(), Value::Str("a, b, c, d".into())],
        )
        .unwrap();
        match v {
            Value::Str(svg) => {
                assert!(svg.starts_with("<svg"));
                assert!(svg.ends_with("</svg>"));
                assert!(svg.contains("рёбер = 6"));
                assert!(svg.contains("компонент = 1"));
                assert!(svg.contains(">a<") && svg.contains(">d<"));
            }
            other => panic!("viz_graph: не строка {other:?}"),
        }
        // Без меток — авто-имена v0…v3.
        match call("viz_graph", &[k4]).unwrap() {
            Value::Str(svg) => assert!(svg.contains(">v0<")),
            other => panic!("viz_graph: не строка {other:?}"),
        }
        // Метки не строкой и неверное число меток — ошибки.
        let two = Value::Matrix(Matrix::from_rows(&[vec![0.0, 1.0], vec![1.0, 0.0]]).unwrap());
        assert!(call("viz_graph", &[two.clone(), Value::Scalar(1.0)]).is_err());
        assert!(call("viz_graph", &[two, Value::Str("одна".into())]).is_err());

        // Поле и поверхность через тот же реестр.
        let f = Matrix::from_rows(&[vec![0.0, 1.0, 0.0], vec![1.0, 2.0, 1.0]]).unwrap();
        match call("viz_field", &[Value::Matrix(f.clone())]).unwrap() {
            Value::Str(svg) => assert!(svg.contains("изолинии: 7 уровней")),
            other => panic!("viz_field: не строка {other:?}"),
        }
        match call("viz_surf", &[Value::Matrix(f)]).unwrap() {
            Value::Str(svg) => assert!(svg.contains("квадов 2")),
            other => panic!("viz_surf: не строка {other:?}"),
        }
        // Каталог знает новую группу.
        assert!(catalog("").contains("суверенный рендер"));
    }

    // ─────────── GIS-ядро на тритах (сессия-12) ───────────

    fn ring_of(pts: &[(f64, f64)]) -> Value {
        Value::Matrix(
            Matrix::from_rows(
                &pts.iter().map(|&(x, y)| vec![x, y]).collect::<Vec<_>>(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn de9im_overlapping_and_nested() {
        let a = ring_of(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
        let b = ring_of(&[(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)]);
        match call("de9im", &[a.clone(), b]).unwrap() {
            Value::Str(rep) => {
                assert!(rep.contains("overlaps"), "отчёт: {rep}");
                assert!(rep.contains("код"));
            }
            other => panic!("de9im: не строка {other:?}"),
        }
        let big = ring_of(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        let small = ring_of(&[(3.0, 3.0), (5.0, 3.0), (5.0, 5.0), (3.0, 5.0)]);
        match call("geo_pred", &[big, small]).unwrap() {
            Value::List(items) => {
                let names: Vec<String> = items
                    .iter()
                    .map(|v| match v {
                        Value::Str(s) => s.clone(),
                        o => format!("{o}"),
                    })
                    .collect();
                assert!(names.contains(&"contains".to_string()), "{names:?}");
                assert!(!names.contains(&"overlaps".to_string()));
            }
            other => panic!("geo_pred: не список {other:?}"),
        }
    }

    #[test]
    fn de9im_point_and_disjoint() {
        let sq = ring_of(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        let inside = ring_of(&[(2.0, 2.0)]);
        match call("geo_pred", &[inside, sq]).unwrap() {
            Value::List(items) => {
                let names: Vec<String> = items
                    .iter()
                    .map(|v| match v {
                        Value::Str(s) => s.clone(),
                        o => format!("{o}"),
                    })
                    .collect();
                assert!(names.contains(&"within".to_string()), "{names:?}");
            }
            other => panic!("geo_pred: не список {other:?}"),
        }
        let far = ring_of(&[(9.0, 9.0)]);
        match call("de9im", &[far, ring_of(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)])]).unwrap() {
            Value::Str(rep) => assert!(rep.contains("disjoint")),
            other => panic!("de9im: не строка {other:?}"),
        }
        // Неверный масштаб — ошибка.
        assert!(call("de9im", &[ring_of(&[(0.0, 0.0)]), ring_of(&[(1.0, 1.0)]), s(13.0)]).is_err());
    }

    #[test]
    fn viz_iso3_sphere_scene() {
        // Сфера как стек z-срезов (3 среза 8×8): f = r − d.
        let n = 8usize;
        let c = (n - 1) as f64 / 2.0;
        let r = n as f64 * 0.4;
        let slices: Vec<Value> = (0..3)
            .map(|iz| {
                let z = iz as f64;
                let mut rows: Vec<Vec<f64>> = Vec::new();
                for iy in 0..n {
                    let mut row: Vec<f64> = Vec::new();
                    for ix in 0..n {
                        let d = (
                            (ix as f64 - c).powi(2)
                                + (iy as f64 - c).powi(2)
                                + (z - c).powi(2)
                        )
                        .sqrt();
                        row.push(r - d);
                    }
                    rows.push(row);
                }
                Value::Matrix(Matrix::from_rows(&rows).unwrap())
            })
            .collect();
        match call("viz_iso3", &[Value::List(slices)]).unwrap() {
            Value::Str(svg) => {
                assert!(svg.starts_with("<svg"));
                assert!(svg.contains("marching tetrahedra"));
                assert!(svg.contains("треугольников"));
            }
            other => panic!("viz_iso3: не строка {other:?}"),
        }
        // Уровень вне поля — ошибка.
        let flat = Value::Matrix(
            Matrix::from_rows(&[vec![1.0, 1.0], vec![1.0, 1.0]]).unwrap(),
        );
        assert!(call("viz_iso3", &[flat]).is_err());
    }

    // ─────────── сверхдиапазонные вероятности (сессия-13) ───────────

    #[test]
    fn logpow_within_and_beyond_f64() {
        // 10^10 представимо: значение + тритная форма рядом.
        match call("logpow", &[s(10.0), s(10.0)]).unwrap() {
            Value::LogProb(p) => {
                assert!((p.log10() - 10.0).abs() < 1e-9);
                assert!((p.log3() - 20.959).abs() < 1e-2);
                assert!((p.to_f64().unwrap() - 1e10).abs() < 1e-3);
            }
            other => panic!("logpow: не лог-вероятность {other:?}"),
        }
        // 2^(-10^15) — вне f64, лог-домен отвечает точно.
        match call("logpow", &[s(2.0), s(-1e15)]).unwrap() {
            Value::LogProb(p) => {
                assert!((p.log10() + 3.0103e14).abs() < 1e10);
                assert!((p.log3() + 6.3093e14).abs() < 1e10);
                assert!(p.to_f64().is_none());
                let shown = format!("{}", Value::LogProb(p));
                assert!(shown.contains("10^(-3.010e+14)"), "вывод: {shown}");
                assert!(shown.contains("3^(-6.309e+14)"), "вывод: {shown}");
            }
            other => panic!("logpow: не лог-вероятность {other:?}"),
        }
        // Основание ≤ 0 и не-числа — ошибки.
        assert!(call("logpow", &[s(-2.0), s(3.0)]).is_err());
        assert!(call("logpow", &[s(2.0)]).is_err());
        assert!(call("logpow", &[Value::Str("x".into()), s(3.0)]).is_err());
    }

    #[test]
    fn logprob_value_arithmetic_end_to_end() {
        // Сложение вероятностей — logaddexp: 0.5 + 0.25 = 0.75
        // (операторы гоняем через binary_op — единая точка диспетчеризации).
        let a = Value::LogProb(crate::calc::logprob::LogProb::from_f64(0.5).unwrap());
        let b = Value::LogProb(crate::calc::logprob::LogProb::from_f64(0.25).unwrap());
        let r = crate::calc::parser::binary_op(crate::calc::parser::BinOp::Add, &a, &b).unwrap();
        match r {
            Value::LogProb(p) => assert!((p.to_f64().unwrap() - 0.75).abs() < 1e-15),
            other => panic!("0.5+0.25: {other:?}"),
        }
        // Скаляр поднимается: logp(0.5) + 0.25 = 0.75
        let c = Value::Scalar(0.25);
        let r2 = crate::calc::parser::binary_op(crate::calc::parser::BinOp::Add, &a, &c).unwrap();
        match r2 {
            Value::LogProb(p) => assert!((p.to_f64().unwrap() - 0.75).abs() < 1e-15),
            other => panic!("logp+scalar: {other:?}"),
        }
        // Дополнение сверхмалой: 1 − logpow(2, -1e5) ≈ 1.
        let lp = call("logpow", &[s(2.0), s(-1e5)]).unwrap();
        let one = Value::Scalar(1.0);
        let r3 = crate::calc::parser::binary_op(crate::calc::parser::BinOp::Sub, &one, &lp).unwrap();
        match r3 {
            Value::LogProb(p) => {
                assert!(p.to_f64().is_some());
                assert!((p.to_f64().unwrap() - 1.0).abs() < 1e-5);
            }
            other => panic!("1−P: {other:?}"),
        }
        // Произведение сверхмалых — сумма логарифмов: 2^(-1e5)·2^(-1e5) = 2^(-2e5).
        let r4 = crate::calc::parser::binary_op(crate::calc::parser::BinOp::Mul, &lp, &lp).unwrap();
        match r4 {
            Value::LogProb(p) => assert!((p.log10() + 6.0206e4).abs() < 1e-1),
            other => panic!("P·P: {other:?}"),
        }
        // Деление — разность логов: 2^(-1e5) / 2^(-1e5) = 1.
        let r5 = crate::calc::parser::binary_op(crate::calc::parser::BinOp::Div, &lp, &lp).unwrap();
        match r5 {
            Value::LogProb(p) => assert!(p.ln().abs() < 1e-9),
            other => panic!("P/P: {other:?}"),
        }
        // Отрицание и P·(−1) — запрещены.
        assert!(crate::calc::parser::binary_op(crate::calc::parser::BinOp::Mul, &lp, &Value::Scalar(-1.0)).is_err());
        // P^2 — степень.
        let r6 = crate::calc::parser::binary_op(crate::calc::parser::BinOp::Pow, &lp, &Value::Scalar(2.0)).unwrap();
        match r6 {
            Value::LogProb(p) => assert!((p.log10() + 6.0206e4).abs() < 1e-1),
            other => panic!("P^2: {other:?}"),
        }
    }

    #[test]
    fn lpsum_series_and_logp_expneg() {
        // lpsum(0.5, 0.25, 0.125) = 0.875
        match call("lpsum", &[s(0.5), s(0.25), s(0.125)]).unwrap() {
            Value::LogProb(p) => assert!((p.to_f64().unwrap() - 0.875).abs() < 1e-14),
            other => panic!("lpsum: {other:?}"),
        }
        // ряд сверхмалых: Σ 3^(-i), i=1..40 — геометрическая, сумма = 1/2
        let items: Vec<Value> = (1..=40i64)
            .map(|i| call("logpow", &[s(3.0), s(-(i as f64))]).unwrap())
            .collect();
        match call("lpsum", &[Value::List(items)]).unwrap() {
            Value::LogProb(p) => {
                // 1/2 − 3^(−40) ≈ 0.5
                assert!((p.to_f64().unwrap() - 0.5).abs() < 1e-10);
            }
            other => panic!("lpsum ряд: {other:?}"),
        }
        // все члены вне f64: Σ 2^(−(1e6+i·1e3)) ≈ 2^(−1e6)
        let tiny: Vec<Value> = (0..10i64)
            .map(|i| call("logpow", &[s(2.0), s(-(1e6 + i as f64 * 1e3))]).unwrap())
            .collect();
        match call("lpsum", &[Value::List(tiny)]).unwrap() {
            Value::LogProb(p) => {
                assert!((p.log10() + 3.0103e5).abs() < 1e2, "Z: {}", p.log10());
                assert!(p.to_f64().is_none()); // а в f64 сумма «получилась» бы 0.0
            }
            other => panic!("lpsum сверхмалы: {other:?}"),
        }
        // logp — вход в лог-домен, отрицательные запрещены
        match call("logp", &[s(0.5)]).unwrap() {
            Value::LogProb(p) => assert!((p.to_f64().unwrap() - 0.5).abs() < 1e-15),
            other => panic!("logp: {other:?}"),
        }
        assert!(call("logp", &[s(0.0)]).is_err());
        assert!(call("logp", &[s(-0.5)]).is_err());
        // expneg — больцмановский штраф
        match call("expneg", &[s(1e30)]).unwrap() {
            Value::LogProb(p) => {
                assert!((p.log10() + 4.3429e29).abs() < 1e26);
                assert!(p.to_f64().is_none());
            }
            other => panic!("expneg: {other:?}"),
        }
        assert!(call("expneg", &[s(-1.0)]).is_err());
        // пустой ряд — ошибка; смесь с строкой — ошибка
        assert!(call("lpsum", &[]).is_err());
        assert!(call("lpsum", &[Value::Str("x".into())]).is_err());
        // ln/log10/log2 от лог-вероятности — числа
        let lp = call("logpow", &[s(2.0), s(-1e5)]).unwrap();
        match call("log10", &[lp.clone()]).unwrap() {
            Value::Scalar(v) => assert!((v + 3.0103e4).abs() < 1e-2),
            other => panic!("log10(LogProb): {other:?}"),
        }
        match call("ln", &[lp]).unwrap() {
            Value::Scalar(v) => assert!((v + 6.93147e4).abs() < 1e-1),
            other => panic!("ln(LogProb): {other:?}"),
        }
    }

    #[test]
    fn regress_full_report_matches_reference() {
        // Эталон — Python-калькуляция Сессии-13:
        // N = 1e5 бит, Δt = 3600 с, 20 Вт, 310.15 К.
        match call("regress", &[s(1e5), s(3600.0)]).unwrap() {
            Value::Str(rep) => {
                assert!(rep.contains("ΔS/k_B = 1.681e+25"), "ΔS: {rep}");
                assert!(rep.contains("4.671e+21"), "σ: {rep}");
                assert!(rep.contains("10^(-3.010e+04)"), "память: {rep}");
                assert!(rep.contains("3^(-6.309e+04)"), "триты памяти: {rep}");
                assert!(rep.contains("10^(-7.302e+24)"), "регрессия: {rep}");
                assert!(rep.contains("3^(-1.530e+25)"), "триты регрессии: {rep}");
                assert!(rep.contains("10^(140.91)"), "бюджет: {rep}");
                assert!(rep.contains("0 реализаций"), "вердикт: {rep}");
                assert!(rep.contains("20 порядков"), "дороже: {rep}");
                assert!(rep.contains("Θ-канал"), "каналы: {rep}");
            }
            other => panic!("regress: не строка {other:?}"),
        }
    }

    #[test]
    fn regress_defaults_overrides_and_errors() {
        // Умолчания совпадают с явными 20 Вт / 310.15 К.
        let a = match call("regress", &[s(1.0), s(60.0)]).unwrap() {
            Value::Str(a) => a,
            other => panic!("regress: не строка {other:?}"),
        };
        let b = match call("regress", &[s(1.0), s(60.0), s(20.0), s(310.15)]).unwrap() {
            Value::Str(b) => b,
            other => panic!("regress: не строка {other:?}"),
        };
        assert_eq!(a, b);
        assert!(a.contains("= 0.5 = 3^(-6.309e-01)"), "один бит: {a}");
        assert!(a.contains("ΔS/k_B = 2.802e+23"), "минута: {a}");

        // 30 лет ≈ 9.467e8 с: ΔS/k_B = 4.422e30.
        match call("regress", &[s(1e13), s(9.467e8)]).unwrap() {
            Value::Str(rep) => assert!(rep.contains("4.422e+30"), "30 лет: {rep}"),
            other => panic!("regress: не строка {other:?}"),
        }

        // Величины в СИ проходят: 3600 s — те же секунды.
        let q = match call(
            "regress",
            &[s(1.0), Value::Quantity(3600.0, units::by_name("s").unwrap().clone())],
        ) {
            Ok(Value::Str(q)) => q,
            other => panic!("regress с величиной: {other:?}"),
        };
        assert!(q.contains("Δt = 3600 с"), "СИ-величина: {q}");

        // Ошибки: неверная арность и физически бессмысленные входы.
        assert!(call("regress", &[s(1.0)]).is_err());
        assert!(call("regress", &[]).is_err());
        assert!(call("regress", &[s(1.0), s(2.0), s(3.0), s(4.0), s(5.0)]).is_err());
        assert!(call("regress", &[s(-1.0), s(60.0)]).is_err());
        assert!(call("regress", &[s(1.0), s(0.0)]).is_err());
        assert!(call("regress", &[s(1.0), s(60.0), s(0.0)]).is_err());
        assert!(call("regress", &[s(1.0), s(60.0), s(20.0), s(-5.0)]).is_err());
        assert!(call("regress", &[Value::Str("много".into()), s(60.0)]).is_err());

        // Каталог знает группу.
        assert!(catalog("").contains("сверхдиапазонные вероятности"));
        assert!(catalog("регресс").contains("regress"));
    }

    // ─────────── переплетение модулей (сессия-14) ───────────

    fn mat(rows: &[Vec<f64>]) -> Value {
        Value::Matrix(Matrix::from_rows(rows).unwrap())
    }

    #[test]
    fn spatial27_matches_bruteforce() {
        // 30 конвертов в объёме [-5..5]³, окно [0..2]³ — против перебора
        let mut rows = Vec::new();
        let mut boxes: Vec<[f64; 6]> = (0..30)
            .map(|i| {
                let x = ((i * 37) % 11) as f64 - 5.0;
                let y = ((i * 53) % 11) as f64 - 5.0;
                let z = ((i * 71) % 11) as f64 - 5.0;
                let w = ((i * 29) % 3) as f64;
                [x, y, z, x + w, y + w, z + w]
            })
            .collect();
        // гарантированные пересечения окна
        boxes.push([0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        boxes.push([1.0, 1.0, 1.0, 2.0, 2.0, 2.0]);
        boxes.push([-2.0, -2.0, -2.0, 0.5, 0.5, 0.5]);
        for b in &boxes {
            rows.push(b.to_vec());
        }
        let window = Value::List(
            [0.0, 0.0, 0.0, 2.0, 2.0, 2.0]
                .iter()
                .map(|&v| Value::Scalar(v))
                .collect(),
        );
        let hits = match call("spatial27", &[mat(&rows), window.clone()]).unwrap() {
            Value::List(items) => items,
            other => panic!("spatial27: {other:?}"),
        };
        // лобовой перебор в квантах k=6 (умолчание): касание считается
        let expected: Vec<usize> = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                let q = |v: f64| (v * 729.0).round() as i64;
                (q(b[0]) <= q(2.0) && q(b[3]) >= q(0.0))
                    && (q(b[1]) <= q(2.0) && q(b[4]) >= q(0.0))
                    && (q(b[2]) <= q(2.0) && q(b[5]) >= q(0.0))
            })
            .map(|(i, _)| i + 1)
            .collect();
        let got: Vec<f64> = hits
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        assert_eq!(
            got,
            expected.iter().map(|&i| i as f64).collect::<Vec<_>>(),
            "27-дерево ≠ перебор"
        );
        assert!(!got.is_empty());
        // плохие аргументы
        assert!(call("spatial27", &[mat(&[vec![1.0, 2.0]]), window.clone()]).is_err());
        assert!(call(
            "spatial27",
            &[mat(&rows), Value::List(vec![Value::Scalar(1.0)])]
        )
        .is_err());
    }

    #[test]
    fn hex_functions_roundtrip() {
        // кольцо 1 = 6 соседей центра
        match call("hexring", &[s(1.0)]).unwrap() {
            Value::Matrix(m) => {
                assert_eq!(m.rows, 6);
                assert_eq!(m.cols, 2);
            }
            other => panic!("hexring: {other:?}"),
        }
        // диск 4 = 61 ячейка
        match call("hexdisk", &[s(4.0)]).unwrap() {
            Value::Matrix(m) => assert_eq!(m.rows, 61),
            other => panic!("hexdisk: {other:?}"),
        }
        // дистанция (0,0)→(3,-3) = 3
        match call("hexdist", &[s(0.0), s(0.0), s(3.0), s(-3.0)]).unwrap() {
            Value::Scalar(v) => assert_eq!(v, 3.0),
            other => panic!("hexdist: {other:?}"),
        }
        // тритный адрес: центр s=0; кольцо 1 стартует с (−1,1) s=1
        match call("hexid", &[s(0.0), s(0.0)]).unwrap() {
            Value::Str(t) => assert!(t.contains("s = 0"), "{t}"),
            other => panic!("hexid: {other:?}"),
        }
        match call("hexid", &[s(-1.0), s(1.0)]).unwrap() {
            Value::Str(t) => assert!(t.contains("s = 1"), "{t}"),
            other => panic!("hexid: {other:?}"),
        }
        // ошибки
        assert!(call("hexring", &[s(14.0)]).is_err());
        assert!(call("hexring", &[s(1.5)]).is_err());
        assert!(call("hexid", &[s(14.0), s(0.0)]).is_err()); // дистанция 14 > 13
    }

    #[test]
    fn de9im_line_cases() {
        let sq = mat(&[
            vec![0.0, 0.0],
            vec![4.0, 0.0],
            vec![4.0, 4.0],
            vec![0.0, 4.0],
        ]);
        // насквозь: II=+1, IB=0, IE=+1
        let through = mat(&[vec![-1.0, 2.0], vec![5.0, 2.0]]);
        match call("de9im_line", &[through, sq.clone()]).unwrap() {
            Value::Str(t) => {
                assert!(t.starts_with("+0+"), "матрица: {t}");
                assert!(t.contains("код"), "{t}");
            }
            other => panic!("de9im_line: {other:?}"),
        }
        // внутри: within
        let inside = mat(&[vec![1.0, 1.0], vec![3.0, 3.0]]);
        match call("de9im_line", &[inside, sq.clone()]).unwrap() {
            Value::Str(t) => assert!(t.contains("within"), "{t}"),
            other => panic!("de9im_line: {other:?}"),
        }
        // снаружи: disjoint
        let outside = mat(&[vec![-3.0, -3.0], vec![-1.0, -1.0]]);
        match call("de9im_line", &[outside, sq]).unwrap() {
            Value::Str(t) => assert!(t.contains("disjoint"), "{t}"),
            other => panic!("de9im_line: {other:?}"),
        }
    }

    #[test]
    fn eteria_pipeline_end_to_end() {
        // поле высот: 33×33, значения в [0,1]
        match call("eteria_field", &[]).unwrap() {
            Value::Matrix(m) => {
                assert_eq!((m.rows, m.cols), (33, 33));
                for r in 0..m.rows {
                    for c in 0..m.cols {
                        let v = m.get(r, c).re;
                        assert!((0.0..=1.0).contains(&v), "h = {v}");
                    }
                }
            }
            other => panic!("eteria_field: {other:?}"),
        }
        assert!(call("eteria_field", &[s(7.0), s(10.0)]).is_err());
        // гекс-карта R8: 217×4
        let cells = match call("eteria_hex", &[]).unwrap() {
            Value::Matrix(m) => {
                assert_eq!((m.rows, m.cols), (217, 4));
                m
            }
            other => panic!("eteria_hex: {other:?}"),
        };
        // биомы целые 0..5
        for r in 0..cells.rows {
            let b = cells.get(r, 3).re;
            assert!(b.fract() == 0.0 && (0.0..=5.0).contains(&b));
        }
        // viz_hex по карте Этерии: 217 полигонов
        match call("viz_hex", &[Value::Matrix(cells.clone())]).unwrap() {
            Value::Str(svg) => {
                assert!(svg.starts_with("<svg"));
                assert_eq!(svg.matches("<polygon").count(), 217);
                assert!(svg.contains("океан ×"));
            }
            other => panic!("viz_hex: {other:?}"),
        }
        // больцмановский отчёт: весь ряд за f64
        match call("eteria_boltz", &[]).unwrap() {
            Value::Str(rep) => {
                assert!(rep.contains("217 ячеек"), "{rep}");
                assert!(rep.contains("весь ряд вне f64"), "{rep}");
                assert!(rep.contains("Z = Σ P_i"), "{rep}");
            }
            other => panic!("eteria_boltz: {other:?}"),
        }
        // собственный заголовок и ошибки viz_hex
        assert!(call(
            "viz_hex",
            &[Value::Matrix(cells.clone()), Value::Str("Мой мир".into())]
        )
        .is_ok());
        assert!(call("viz_hex", &[mat(&[vec![1.0, 2.0]])]).is_err());
        // seed меняет мир
        let a = call("eteria_field", &[s(7.0)]).unwrap();
        let b = call("eteria_field", &[s(42.0)]).unwrap();
        assert_ne!(format!("{a}"), format!("{b}"));
        // а один seed — детерминирован
        let a2 = call("eteria_field", &[s(7.0)]).unwrap();
        assert_eq!(format!("{a}"), format!("{a2}"));
    }

    // ===== Сессия-16: кристалл рельефа =====

    #[test]
    fn tin_report_mentions_acceleration() {
        let pts = mat(&[
            vec![0.0, 0.0],
            vec![4.0, 0.0],
            vec![4.0, 4.0],
            vec![0.0, 4.0],
            vec![2.0, 2.0],
        ]);
        match call("tin", &[pts]).unwrap() {
            Value::Str(rep) => {
                assert!(rep.contains("5 точек"), "{rep}");
                assert!(rep.contains("4 треугольников"), "{rep}");
                assert!(rep.contains("history-DAG"), "сессия-16 в отчёте: {rep}");
                assert!(rep.contains("Gold Phase Lock"), "{rep}");
                assert!(rep.contains("верифицирован"), "{rep}");
            }
            other => panic!("tin: {other:?}"),
        }
        // ошибки
        assert!(call("tin", &[mat(&[vec![0.0, 0.0]])]).is_err());
    }

    #[test]
    fn eteria_tin_pts_feeds_viz_tin() {
        // конвейер одной строкой: посев → Муха → TIN → биомы → SVG
        let pts = match call("eteria_tin_pts", &[s(7.0), s(128.0)]).unwrap() {
            Value::Matrix(m) => {
                assert_eq!((m.rows, m.cols), (128, 3));
                for r in 0..m.rows {
                    let (x, y, h) = (
                        m.get(r, 0).re,
                        m.get(r, 1).re,
                        m.get(r, 2).re,
                    );
                    assert!((0.0..=1.0).contains(&x), "x = {x}");
                    assert!((0.0..=1.0).contains(&y), "y = {y}");
                    assert!((0.0..=1.0).contains(&h), "h = {h}");
                }
                m
            }
            other => panic!("eteria_tin_pts: {other:?}"),
        };
        match call("viz_tin", &[Value::Matrix(pts.clone())]).unwrap() {
            Value::Str(svg) => {
                assert!(svg.starts_with("<svg"));
                assert!(svg.contains("128 вершин"), "дедуп не нужен: см. вывод");
                assert!(svg.contains("27-дерево"), "{svg}");
                assert!(svg.contains("муха"), "{svg}");
                let polys = svg.matches("<polygon").count();
                assert!(polys >= 200, "мозаика из {polys} треугольников");
            }
            other => panic!("viz_tin: {other:?}"),
        }
        // структура без высот: Nx2 из тех же точек
        let pts2 = match call("eteria_tin_pts", &[s(7.0), s(32.0)]).unwrap() {
            Value::Matrix(m) => m,
            other => panic!("eteria_tin_pts: {other:?}"),
        };
        let rows: Vec<Vec<f64>> = (0..pts2.rows)
            .map(|r| vec![pts2.get(r, 0).re, pts2.get(r, 1).re])
            .collect();
        let flat = mat(&rows);
        match call("viz_tin", &[flat]).unwrap() {
            Value::Str(svg) => assert!(svg.contains("#94a3b8"), "графит без рельефа"),
            other => panic!("viz_tin Nx2: {other:?}"),
        }
        // ошибки
        assert!(call("eteria_tin_pts", &[s(7.0), s(2.0)]).is_err());
        assert!(call("viz_tin", &[mat(&[vec![1.0, 2.0]])]).is_err());
        assert!(call("viz_tin", &[mat(&[vec![1.0, 2.0, 3.0]])]).is_err());
    }

    #[test]
    fn de9im_poly_hole_semantics() {
        let outer_a = ring_of(&[
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
        ]);
        let hole_a = ring_of(&[(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)]);
        let poly_a = match call("poly", &[outer_a.clone(), hole_a.clone()]).unwrap() {
            v @ Value::List(_) => v,
            other => panic!("poly: {other:?}"),
        };
        // B в дырке A — disjoint
        let in_hole = ring_of(&[(4.0, 4.0), (6.0, 4.0), (6.0, 6.0), (4.0, 6.0)]);
        match call("de9im_poly", &[poly_a.clone(), in_hole]).unwrap() {
            Value::Str(t) => {
                assert!(t.contains("disjoint"), "B в дырке: {t}");
                assert!(t.contains("дырок 1"), "{t}");
            }
            other => panic!("de9im_poly: {other:?}"),
        }
        // B = дырка A — touches
        match call("de9im_poly", &[poly_a.clone(), hole_a]).unwrap() {
            Value::Str(t) => assert!(t.contains("touches"), "B = дырка: {t}"),
            other => panic!("de9im_poly: {other:?}"),
        }
        // идентичные с дырками — equals
        let outer_a2 = ring_of(&[
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
        ]);
        let hole_a2 = ring_of(&[(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)]);
        let poly_a2 = call("poly", &[outer_a2, hole_a2]).unwrap();
        match call("de9im_poly", &[poly_a.clone(), poly_a2]).unwrap() {
            Value::Str(t) => assert!(t.contains("equals"), "идентичные: {t}"),
            other => panic!("de9im_poly: {other:?}"),
        }
        // кольца без poly (обратная совместимость): вложенные квадраты
        let big = ring_of(&[(0.0, 0.0), (8.0, 0.0), (8.0, 8.0), (0.0, 8.0)]);
        let small = ring_of(&[(2.0, 2.0), (4.0, 2.0), (4.0, 4.0), (2.0, 4.0)]);
        match call("de9im_poly", &[big, small]).unwrap() {
            Value::Str(t) => assert!(t.contains("contains"), "кольца как полигоны: {t}"),
            other => panic!("de9im_poly: {other:?}"),
        }
        // ошибки: короткое кольцо в poly, не-кольцо, разные k
        let short = ring_of(&[(0.0, 0.0), (1.0, 1.0)]);
        assert!(call("poly", &[short]).is_err());
        assert!(call("de9im_poly", &[s(1.0), s(2.0)]).is_err());
        assert!(call("poly", &[]).is_err());
    }
    // ===== Сессия-17: три горизонта =====

    #[test]
    fn tin_isolines_report_and_errors() {
        // пирамида: замкнутая изолиния 0.5
        let pts = mat(&[
            vec![0.0, 0.0, 0.0],
            vec![8.0, 0.0, 0.0],
            vec![8.0, 8.0, 0.0],
            vec![0.0, 8.0, 0.0],
            vec![4.0, 4.0, 1.0],
        ]);
        match call(
            "tin_isolines",
            &[pts.clone(), Value::List(vec![Value::Scalar(0.5)])],
        )
        .unwrap()
        {
            Value::Str(rep) => {
                assert!(rep.contains("уровень 0.500"), "{rep}");
                assert!(rep.contains("marching squares"), "{rep}");
                assert!(rep.contains("точно"), "{rep}");
                assert!(rep.contains("Δ"), "{rep}");
                assert!(rep.contains("watertight"), "{rep}");
            }
            other => panic!("tin_isolines: {other:?}"),
        }
        // авто-уровни (число) + сетка
        match call("tin_isolines", &[pts.clone(), s(3.0), s(16.0)]).unwrap() {
            Value::Str(rep) => {
                assert!(rep.contains("уровень"), "{rep}");
                assert!(rep.contains("сетка 16×16"), "{rep}");
            }
            other => panic!("tin_isolines авто: {other:?}"),
        }
        // ошибки: Nx2 (нет высот), уровней 0, сетка мимо, уровень не число
        let pts2 = mat(&[vec![0.0, 0.0], vec![1.0, 1.0], vec![2.0, 0.0]]);
        assert!(call("tin_isolines", &[pts2]).is_err());
        assert!(call("tin_isolines", &[pts.clone(), s(0.0)]).is_err());
        assert!(call("tin_isolines", &[pts.clone(), s(5.0), s(8.0)]).is_err());
        assert!(call(
            "tin_isolines",
            &[pts.clone(), Value::List(vec![Value::Str("x".into())])]
        )
        .is_err());
    }

    #[test]
    fn viz_isolines_svg_pipeline() {
        // конвейер одной строкой: посев → Муха → TIN → изолинии → SVG
        let pts = match call("eteria_tin_pts", &[s(7.0), s(64.0)]).unwrap() {
            Value::Matrix(m) => m,
            other => panic!("eteria_tin_pts: {other:?}"),
        };
        match call("viz_isolines", &[Value::Matrix(pts.clone())]).unwrap() {
            Value::Str(svg) => {
                assert!(svg.starts_with("<svg"));
                assert!(svg.contains("изолинии"), "{svg}");
                assert!(
                    svg.contains("<polyline") || svg.contains("<polygon"),
                    "изолинии нарисованы"
                );
                assert!(svg.contains("marching squares"), "{svg}");
                let n_iso = svg.matches("<polyline").count();
                assert!(n_iso > 0, "полилиний: {n_iso}");
            }
            other => panic!("viz_isolines: {other:?}"),
        }
        // число уровней + решётка + заголовок
        match call(
            "viz_isolines",
            &[
                Value::Matrix(pts.clone()),
                s(3.0),
                s(10.0),
                Value::Str("Мой мир".into()),
            ],
        )
        .unwrap()
        {
            Value::Str(svg) => assert!(svg.contains("Мой мир")),
            other => panic!("viz_isolines кастом: {other:?}"),
        }
        // явные уровни списком
        let lvls = Value::List(vec![Value::Scalar(0.4), Value::Scalar(0.6)]);
        assert!(call("viz_isolines", &[Value::Matrix(pts.clone()), lvls]).is_ok());
        // ошибки: уровней 0, > 12, Nx2
        assert!(call("viz_isolines", &[Value::Matrix(pts.clone()), s(0.0)]).is_err());
        assert!(call("viz_isolines", &[Value::Matrix(pts.clone()), s(13.0)]).is_err());
        let flat = mat(&[vec![0.0, 0.0], vec![1.0, 1.0], vec![2.0, 0.0]]);
        assert!(call("viz_isolines", &[flat]).is_err());
    }

    #[test]
    fn h3addr_h3cell_roundtrip() {
        // h3addr: умолчание res 2 → 13 тритов
        match call("h3addr", &[s(7.0), s(2.0), s(-3.0)]).unwrap() {
            Value::Str(t) => {
                assert!(t.contains("13 тритов"), "{t}");
                assert!(t.contains("res 2"), "{t}");
                assert!(t.contains("сходится"), "{t}");
                assert!(t.contains("грань 8/20"), "{t}");
                assert!(t.contains("уточнения"), "{t}");
            }
            other => panic!("h3addr: {other:?}"),
        }
        // res 0 → 9 тритов; res 1 → 11; res 3 → 15 (механика дальше канона)
        for (res, want) in [(0.0, "9 тритов"), (1.0, "11 тритов"), (3.0, "15 тритов")] {
            match call("h3addr", &[s(7.0), s(2.0), s(-3.0), s(res)]).unwrap() {
                Value::Str(t) => assert!(t.contains(want), "res {res}: {t}"),
                other => panic!("h3addr res {res}: {other:?}"),
            }
        }
        // h3cell: адрес из hexgrid напрямую (грань 4 — нулевая старшая тройка)
        let t = crate::geo::hexgrid::world_address(4, 9, 9, 2).unwrap();
        let addr = t.to_string_bal();
        assert_eq!(addr.chars().count(), 13);
        match call("h3cell", &[Value::Str(addr.clone())]).unwrap() {
            Value::Str(rep) => {
                assert!(rep.contains("res 2"), "{rep}");
                assert!(rep.contains("грань 5/20"), "{rep}");
                assert!(rep.contains("ячейка res-2: (+9, +9)"), "{rep}");
                assert!(rep.contains("уточнения"), "{rep}");
            }
            other => panic!("h3cell: {other:?}"),
        }
        // короткий адрес res 0 (клетка в пределах спирали R13)
        let t0 = crate::geo::hexgrid::world_address(4, 2, -3, 0).unwrap();
        match call("h3cell", &[Value::Str(t0.to_string_bal())]).unwrap() {
            Value::Str(rep) => assert!(rep.contains("res 0"), "{rep}"),
            other => panic!("h3cell res0: {other:?}"),
        }
        // ошибки: грань 20, res 9, кривая строка, чётная длина
        assert!(call("h3addr", &[s(20.0), s(0.0), s(0.0)]).is_err());
        assert!(call("h3addr", &[s(7.0), s(0.0), s(0.0), s(9.0)]).is_err());
        assert!(call("h3cell", &[Value::Str("abc".into())]).is_err());
        assert!(call("h3cell", &[Value::Str("101".into())]).is_err());
        assert!(call("h3cell", &[Value::Str("1010101010".into())]).is_err());
    }

    #[test]
    fn de9im_gc_mixed_collections() {
        let poly_b = ring_of(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        let point_in = ring_of(&[(2.0, 2.0)]);
        let line_cross = mat(&[vec![5.0, -2.0], vec![5.0, 12.0]]);
        // line() конструктор + gc() коллекция {точка, линия} vs полигон
        let line_val = call("line", &[line_cross]).unwrap();
        let gc_a = call("gc", &[point_in.clone(), line_val]).unwrap();
        match call("de9im_gc", &[gc_a.clone(), poly_b.clone()]).unwrap() {
            Value::Str(t) => {
                assert!(t.contains("intersects"), "{t}");
                assert!(t.contains("коллекция [точек 1 · линий 1"), "{t}");
                assert!(t.contains("полигон (1 колец"), "{t}");
            }
            other => panic!("de9im_gc: {other:?}"),
        }
        // транспонированная симметрия на уровне отчёта: код совпадает
        // (матрица A×B и B×A взаимно транспонированы — код может
        // отличаться, проверяем предикаты обоих направлений)
        match call("de9im_gc", &[poly_b.clone(), gc_a.clone()]).unwrap() {
            Value::Str(t) => assert!(t.contains("intersects"), "обратное: {t}"),
            other => panic!("de9im_gc reverse: {other:?}"),
        }
        // одиночная коллекция == сама геометрия
        let gc_ring = call("gc", &[poly_b.clone()]).unwrap();
        let s1 = match call("de9im_gc", &[gc_ring, poly_b.clone()]).unwrap() {
            Value::Str(t) => t.lines().next().unwrap().to_string(),
            other => panic!("{other:?}"),
        };
        let s2 = match call("de9im_poly", &[poly_b.clone(), poly_b.clone()]).unwrap() {
            Value::Str(t) => t.lines().next().unwrap().to_string(),
            other => panic!("{other:?}"),
        };
        assert_eq!(
            s1.split(" · предикаты").next(),
            s2.split(" · предикаты").next(),
            "gc(один полигон) == сам полигон"
        );
        // вся коллекция снаружи — disjoint
        let far_pt = ring_of(&[(20.0, 20.0)]);
        let far_line = mat(&[vec![15.0, 15.0], vec![16.0, 16.0]]);
        // gc(полигон с дыркой + линия в дырке) vs B-в-дырке — disjoint
        let outer = ring_of(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        let hole = ring_of(&[(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)]);
        let poly_hole = call("poly", &[outer, hole]).unwrap();
        let in_hole_line = mat(&[vec![3.4, 3.2], vec![3.4, 6.8]]);
        let in_hole_line = call("line", &[in_hole_line]).unwrap();
        let b_in_hole = ring_of(&[(4.0, 4.0), (6.0, 4.0), (6.0, 6.0), (4.0, 6.0)]);
        let gc_hole = call("gc", &[poly_hole, in_hole_line]).unwrap();
        match call("de9im_gc", &[gc_hole, b_in_hole]).unwrap() {
            Value::Str(t) => assert!(t.contains("disjoint"), "B в дырке: {t}"),
            other => panic!("{other:?}"),
        }
        // точка снаружи + линия снаружи
        let far_line2 = call("line", &[far_line]).unwrap();
        let gc_far = call("gc", &[far_pt, far_line2]).unwrap();
        match call("de9im_gc", &[gc_far, poly_b.clone()]).unwrap() {
            Value::Str(t) => assert!(t.contains("disjoint"), "всё снаружи: {t}"),
            other => panic!("{other:?}"),
        }
        // ошибки: пустая gc, line из 1 точки, скаляр вместо геометрии
        assert!(call("gc", &[]).is_err());
        assert!(call("line", &[ring_of(&[(1.0, 1.0)])]).is_err());
        assert!(call("de9im_gc", &[s(1.0), poly_b.clone()]).is_err());
        // gc из скаляра — ошибка валидации формы
        assert!(call("gc", &[s(1.0)]).is_err());
    }

    #[test]
    fn chem_functions_calls() {
        // chem_elem по символу и Z
        let v1 = call("chem_elem", &[Value::Str("Fe".into())]).unwrap();
        assert!(v1.to_string().contains("Железо") && v1.to_string().contains("Z=26"));
        let v2 = call("chem_elem", &[s(1.0)]).unwrap();
        assert!(v2.to_string().contains("Водород") && v2.to_string().contains("Z=1"));

        // chem_bond
        let vb = call("chem_bond", &[Value::Str("Na".into()), Value::Str("Cl".into())]).unwrap();
        assert!(vb.to_string().contains("Ионный") && vb.to_string().contains("−"));

        // chem_mass
        let vm = call("chem_mass", &[Value::Str("H2SO4".into())]).unwrap();
        assert!(close(vm.as_f64().unwrap(), 98.078, 0.05));

        // chem_formula
        let vf = call("chem_formula", &[Value::Str("Fe2(SO4)3".into())]).unwrap();
        assert!(vf.to_string().contains("M = 399.8") && vf.to_string().contains("Fe:2"));

        // chem_archetype
        let va = call("chem_archetype", &[Value::Str("C".into())]).unwrap();
        assert!(va.to_string().contains("OrganicLifeNonmetal"));
    }

    // ─── Лестница молекул v0.74.0 ─────────────────────────────────────
    #[test]
    fn chem_smiles_aspirin() {
        let v = call("chem_smiles", &[Value::Str("аспирин".into())]).unwrap();
        match v {
            Value::Str(rep) => {
                assert!(rep.contains("C9H8O4"), "формула: {rep}");
                assert!(rep.contains("колец"), "кольца: {rep}");
                assert!(rep.contains("Гастайгера"), "заряды: {rep}");
                assert!(rep.contains("Lipinski"), "дескрипторы: {rep}");
            }
            other => panic!("chem_smiles: {other:?}"),
        }
        // И прямой SMILES
        let v = call("chem_smiles", &[Value::Str("CC(=O)Oc1ccccc1C(=O)O".into())]).unwrap();
        match v {
            Value::Str(rep) => assert!(rep.contains("C9H8O4")),
            other => panic!("chem_smiles SMILES: {other:?}"),
        }
    }

    #[test]
    fn chem_smiles_errors() {
        assert!(call("chem_smiles", &[Value::Str("xyzzy_qq".into())]).is_err());
        assert!(call("chem_smiles", &[s(1.0)]).is_err());
        assert!(call("chem_smiles", &[]).is_err());
    }

    #[test]
    fn chem_3d_caffeine() {
        let v = call("chem_3d", &[Value::Str("кофеин".into())]).unwrap();
        match v {
            Value::Str(rep) => {
                assert!(rep.contains("конформер"), "{rep}");
                assert!(rep.contains("Å"), "{rep}");
                assert!(rep.contains("ибридизац"), "{rep}");
                assert!(rep.contains("sp"), "{rep}");
            }
            other => panic!("chem_3d: {other:?}"),
        }
    }

    #[test]
    fn bio_eval_water_dimer() {
        let v = call(
            "bio_eval",
            &[Value::Str("вода".into()), Value::Str("вода".into())],
        )
        .unwrap();
        match v {
            Value::Str(rep) => {
                assert!(rep.contains("ΔG"), "{rep}");
                assert!(rep.contains("Kd = e^"), "лог-домен: {rep}");
                assert!(rep.contains("тритная глубина"), "{rep}");
                assert!(rep.contains("27-дерево"), "{rep}");
                assert!(rep.contains("DE-9IM"), "{rep}");
                assert!(rep.contains("Трит-вердикт"), "{rep}");
            }
            other => panic!("bio_eval: {other:?}"),
        }
    }

    #[test]
    fn bio_eval_needs_two_args() {
        assert!(call("bio_eval", &[Value::Str("вода".into())]).is_err());
    }

    // ─── log-домен в матрицах v0.74.0 ─────────────────────────────────
    #[test]
    fn logdet_underflow_beyond_f64() {
        // diag(0.1; 400): det = 1e-400 — за пределами f64 (underflow → 0.0)
        let n = 400;
        let rows: Vec<Vec<f64>> = (0..n)
            .map(|i| (0..n).map(|j| if i == j { 0.1 } else { 0.0 }).collect())
            .collect();
        let m = Matrix::from_rows(&rows).unwrap();
        // Обычный det теряет значение
        let d_f64 = m.det().unwrap().re;
        assert_eq!(d_f64, 0.0, "f64-детерминант обязан упасть в underflow");
        // Лог-домен держит
        let (sign, ln_abs) = m.logdet().unwrap();
        assert_eq!(sign, 1.0);
        let expected = 400.0f64 * 0.1f64.ln();
        assert!((ln_abs - expected).abs() < 1e-9, "ln|det| = {ln_abs}");
        assert!((ln_abs / std::f64::consts::LN_10 + 400.0).abs() < 1e-9, "log10 = −400");
        // Отчёт функции
        match call("logdet", &[Value::Matrix(m)]).unwrap() {
            Value::Str(rep) => {
                assert!(rep.contains("UNDERFLOW"), "{rep}");
                assert!(rep.contains("3^"), "тритная глубина: {rep}");
                assert!(rep.contains("cond"), "{rep}");
            }
            other => panic!("logdet: {other:?}"),
        }
    }

    #[test]
    fn logdet_well_conditioned() {
        let m = Matrix::from_rows(&[vec![4.0, 1.0], vec![1.0, 3.0]]).unwrap();
        let (sign, ln_abs) = m.logdet().unwrap();
        // det = 4·3 − 1·1 = 11
        assert_eq!(sign, 1.0);
        assert!((ln_abs - 11.0f64.ln()).abs() < 1e-12);
        let cond = m.cond_inf().unwrap();
        // ||A||∞ = 5; A⁻¹ = [3,−1;−1,4]/11 → ||A⁻¹||∞ = 4/11·... max(4/11? (3+1)/11=0.36, (1+4)/11=0.45) → cond = 5·0.4545 = 2.27
        assert!(cond < 3.0 && cond > 1.0, "cond = {cond}");
        match call("logdet", &[Value::Matrix(m)]).unwrap() {
            Value::Str(rep) => {
                assert!(rep.contains("влезает в диапазон"), "{rep}");
                assert!(rep.contains("+1"), "вердикт отличной обусловленности: {rep}");
            }
            other => panic!("logdet: {other:?}"),
        }
    }

    #[test]
    fn lsolve_small_system() {
        // 2x + y = 5; x + 3y = 10 → x = 1, y = 3
        let a = Matrix::from_rows(&[vec![2.0, 1.0], vec![1.0, 3.0]]).unwrap();
        let b = Value::List(vec![s(5.0), s(10.0)]);
        match call("lsolve", &[Value::Matrix(a), b]).unwrap() {
            Value::List(items) => {
                assert_eq!(items.len(), 3, "2 решения + строка невязки");
                assert!((items[0].as_f64().unwrap() - 1.0).abs() < 1e-9);
                assert!((items[1].as_f64().unwrap() - 3.0).abs() < 1e-9);
                match &items[2] {
                    Value::Str(t) => assert!(t.contains("невязка")),
                    other => panic!("третий элемент: {other:?}"),
                }
            }
            other => panic!("lsolve: {other:?}"),
        }
    }

    #[test]
    fn lsolve_equilibrated_wide_range() {
        // Разброс порядков 1e-6 … 1e6 — эквилибровка обязана спасти точность
        let a = Matrix::from_rows(&[
            vec![1e-6, 2e-6],
            vec![1e6, 1e6],
        ])
        .unwrap();
        // x = 3, y = -1: b1 = 3e-6 − 2e-6 = 1e-6; b2 = 3e6 − 1e6 = 2e6
        let b = Value::List(vec![s(1e-6), s(2e6)]);
        match call("lsolve", &[Value::Matrix(a), b]).unwrap() {
            Value::List(items) => {
                assert!((items[0].as_f64().unwrap() - 3.0).abs() < 1e-6, "x = {:?}", items[0].as_f64());
                assert!((items[1].as_f64().unwrap() + 1.0).abs() < 1e-6, "y = {:?}", items[1].as_f64());
            }
            other => panic!("lsolve: {other:?}"),
        }
    }

    #[test]
    fn logdet_errors() {
        let rect = Matrix::from_rows(&[vec![1.0, 2.0]]).unwrap();
        assert!(call("logdet", &[Value::Matrix(rect)]).is_err());
        assert!(call("logdet", &[s(1.0)]).is_err());
        assert!(call("lsolve", &[s(1.0)]).is_err());
    }

    #[test]
    fn logdet_registered() {
        assert!(is_function("logdet"));
        assert!(is_function("lsolve"));
        assert!(catalog("").contains("logdet"));
    }

    #[test]
    fn chem_functions_registered() {
        assert!(is_function("chem_smiles"));
        assert!(is_function("chem_3d"));
        assert!(is_function("bio_eval"));
        assert!(catalog("").contains("chem_smiles"));
        assert!(catalog("").contains("bio_eval"));
    }

    #[test]
    fn speak_scientist_voice() {
        // Глюкоза: полный контур — стехиометрия + связи + роторы + ядро
        let v = call("speak", &[Value::Str("C6H12O6".into())]).unwrap();
        let text = v.to_string();
        assert!(text.contains("180"), "молярная масса обязана быть в фразе: {text}");
        assert!(text.contains("Трит-вердикт"));
        assert!(text.contains("ротор"), "контур роторов обязателен: {text}");

        // Проза без формулы — языковой контур без химии
        let v2 = call("speak", &[Value::Str("резонанс фазы".into())]).unwrap();
        assert!(v2.to_string().contains("Языковой контур"));

        // Не-строка — внятная ошибка
        assert!(call("speak", &[s(1.0)]).is_err());
    }
}

