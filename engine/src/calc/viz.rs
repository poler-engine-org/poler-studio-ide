//! Семантический визуализатор (цикл U, Сессия-8): мост «машина → человек».
//!
//! Идея из полевого диалога (Текстовий файл (2).txt): Калькулятор выдаёт
//! точные значения — 1.616255e-35, I₃ = 2.914854 — но для человека это
//! «ни о чём». Нужен автоматический слой: тип значения → правило → сцена
//! с человеческими якорями. Здесь этот слой реализован как семейство
//! viz-функций, возвращающих компактный SVG 1.1 (UTF-8, monospace,
//! ноль внешних зависимостей — чистые строки).
//!
//! Правила визуализации:
//! - число в [2, 3.2]   → радар нарушения Белла (CGLMP);
//! - положительное число → лог-шкала Вселенной с якорями;
//! - список вероятностей → столбцы Born-распределения;
//! - прочий список       → спектр (бары);
//! - матрица             → теплокарта (знак → цвет, |v| → интенсивность);
//! - комплексное         → бары Re/Im.
//!
//! Грабля 16: литерал `[a, b, c]` в языке Калькулятора — это матрица-строка
//! (не Value::List): viz-слой принимает 1×N/N×1 как список чисел, так что
//! `viz([0.8, 0.2])` даёт Born-бары, а `viz([[1,2],[3,4]])` — теплокарту.
//!
//! Физические якоря замкнуты на результаты Сессий 5–7:
//! классика 2.0 · Цирельсон √8 ≈ 2.828427 · Ацин d=3 ≈ 2.914854 ·
//! квкварт d=4 ≈ 2.972698.

use std::collections::HashMap;

use super::matrix::Matrix;
use super::Value;

// ---------------------------------------------------------------------
// Якоря физики (Сессии 5–7) и шкалы Вселенной
// ---------------------------------------------------------------------

/// Классический локальный предел CGLMP (любое d).
const BELL_CLASSICAL: f64 = 2.0;
/// Цирельсон для кубитов: √8.
const BELL_TSIRELSON: f64 = 2.8284271247461903;
/// Оптимум Ацина d=3 (Сессия 6): 1 + √(11/3).
const BELL_ACIN: f64 = 2.9148542161522637;
/// Некомпактный оптимум d=4 (Сессия 7).
const BELL_QUQUART: f64 = 2.972698267102243;

/// Человеческие якоря лог-шкалы (метры): от Планковской длины
/// до наблюдаемой Вселенной.
const SCALE_ANCHORS: [(&str, f64); 14] = [
    ("Планковская длина", 1.616255e-35),
    ("протон", 1.68e-15),
    ("атом H", 1.06e-10),
    ("ДНК (ширина)", 2.5e-9),
    ("вирус", 1.0e-7),
    ("эритроцит", 8.0e-6),
    ("человек", 1.7),
    ("Эверест", 8.849e3),
    ("Земля", 1.2742e7),
    ("Солнце", 1.3927e9),
    ("орбита Земли", 2.99e11),
    ("парсек", 3.0857e16),
    ("Млечный Путь", 9.5e20),
    ("Вселенная", 8.8e26),
];

/// Человеческий эталон шкалы: рост в метрах.
const HUMAN_M: f64 = 1.7;

// Палитра (единая для всех сцен).
const C_GREEN: &str = "#16a34a";
const C_GREEN_BG: &str = "#dcfce7";
const C_PURPLE: &str = "#7c3aed";
const C_ORANGE: &str = "#ea580c";
const C_RED: &str = "#dc2626";
const C_BLUE: &str = "#2563eb";
const C_BLUE_BG: &str = "#93c5fd";
const C_TEXT: &str = "#111827";
const C_MUTED: &str = "#6b7280";
const C_GRID: &str = "#d1d5db";

// ---------------------------------------------------------------------
// Помощники форматирования и SVG-гигиены
// ---------------------------------------------------------------------

/// XML-экранирование текстовых узлов и атрибутов.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Человекочитаемое число: обычное — без хвостовых нулей,
/// крайние порядки — инженерная запись (1.616e-35).
fn num(x: f64) -> String {
    if !x.is_finite() {
        return "NaN".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    let a = x.abs();
    if (1e-4..1e6).contains(&a) {
        let s = format!("{:.6}", x);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    } else {
        format!("{:.3e}", x)
    }
}

/// Отношение к человеку: «в 3.5 раза», «в 1.052e35 раз».
fn human_ratio(x: f64) -> String {
    if x < 1e4 {
        format!("{:.2}", x)
    } else {
        format!("{:.3e}", x)
    }
}

fn svg_open(w: u32, h: u32) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" \
         font-family=\"monospace\">\n<rect width=\"{w}\" height=\"{h}\" fill=\"#ffffff\"/>\n"
    )
}

fn title(s: &str, w: u32) -> String {
    // Грабля 23: заголовок ужимается кеглем, пока не влезет в холст.
    let avail = w as f64 - 16.0;
    let mut size = 14.0;
    while size > 9.0 && text_w(s, size) > avail {
        size -= 0.5;
    }
    format!(
        "<text x=\"{}\" y=\"22\" font-size=\"{size:.1}\" font-weight=\"bold\" fill=\"{}\" \
         text-anchor=\"middle\">{}</text>\n",
        w / 2,
        C_TEXT,
        esc(s)
    )
}

// ---------------------------------------------------------------------
// Сессия-10: ТИПОГРАФИКА — ни одна надпись не имеет права обрезаться
// ---------------------------------------------------------------------
// Полевой отчёт (slit.png, scale.svg): подписи выходили за холст и
// налетали друг на друга. Грабля 23: SVG-текст не переносится и не
// сжимается сам — холст, перенос и укладку подписей считает ДВИЖОК.
//
// Метрика: моноширинное семейство — продвижение глифа 0.60–0.6023 em
// (DejaVu Sans Mono, Courier New; Consolas 0.55). Оценка 0.62 em —
// верхняя граница: влезшее по оценке влезет и в реальном рендере.

/// Консервативное продвижение глифа моноширинного шрифта (em).
const MONO_EM: f64 = 0.62;

/// Оценка ширины строки при кегле `size` (px).
fn text_w(s: &str, size: f64) -> f64 {
    s.chars().count() as f64 * MONO_EM * size
}

/// Перенос по словам под ширину `max_w`; слово длиннее строки режется
/// по символам. Результат всегда непуст (пустая строка → одна пустая).
fn wrap_text(s: &str, size: f64, max_w: f64) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split(' ').filter(|w| !w.is_empty()) {
        if text_w(word, size) > max_w {
            // Слово-гигант: закрываем текущую строку и режем по глифам.
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            let mut piece = String::new();
            for ch in word.chars() {
                if !piece.is_empty() && text_w(&format!("{piece}{ch}"), size) > max_w {
                    lines.push(std::mem::take(&mut piece));
                }
                piece.push(ch);
            }
            cur = piece;
            continue;
        }
        let cand = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if text_w(&cand, size) <= max_w {
            cur = cand;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Примечание подвала сцены.
struct Note {
    text: String,
    size: f64,
    color: &'static str,
}

impl Note {
    fn new(text: impl Into<String>, size: f64, color: &'static str) -> Self {
        Note {
            text: text.into(),
            size,
            color,
        }
    }
}

/// Кегль примечания: сначала уменьшение (маркеры остаются одной
/// строкой), перенос — только когда и 8 pt не влезает.
fn note_layout(text: &str, base: f64, max_w: f64) -> (f64, Vec<String>) {
    let mut size = base;
    while size > 8.0 && text_w(text, size) > max_w {
        size -= 1.0;
    }
    (size, wrap_text(text, size, max_w))
}

/// Высота блока примечаний — холст обязан знать её ДО отрисовки.
fn notes_height(notes: &[Note], x: f64, w: f64) -> f64 {
    let max_w = (w - x - 8.0).max(40.0);
    let mut h = 0.0;
    for n in notes {
        let (size, lines) = note_layout(&n.text, n.size, max_w);
        h += lines.len() as f64 * (size + 3.5) + 3.0;
    }
    h
}

/// SVG-блок примечаний от базовой линии `y0`; возвращает (svg, высота).
fn notes_svg(notes: &[Note], x: f64, w: f64, y0: f64) -> (String, f64) {
    let max_w = (w - x - 8.0).max(40.0);
    let mut out = String::new();
    let mut y = y0;
    for n in notes {
        let (size, lines) = note_layout(&n.text, n.size, max_w);
        for line in &lines {
            out.push_str(&format!(
                "<text x=\"{x:.0}\" y=\"{y:.1}\" font-size=\"{size:.0}\" fill=\"{}\">{}</text>\n",
                n.color,
                esc(line)
            ));
            y += size + 3.5;
        }
        y += 3.0;
    }
    (out, y - y0)
}

/// Текст с белым ореолом: читается поверх рёбер, сетки и квандов.
fn halo_text(x: f64, y: f64, size: f64, anchor: &str, fill: &str, content: &str) -> String {
    let common =
        format!("x=\"{x:.1}\" y=\"{y:.1}\" font-size=\"{size:.0}\" text-anchor=\"{anchor}\"");
    format!(
        "<text {common} stroke=\"#ffffff\" stroke-width=\"2.4\" \
         stroke-linejoin=\"round\" fill=\"#ffffff\">{}</text>\n\
         <text {common} fill=\"{fill}\">{}</text>\n",
        esc(content),
        esc(content)
    )
}

/// Компактная подпись клетки теплокарты: полный num() → короче →
/// мельче; крайний случай — маркер «·» (знак живёт в цвете клетки).
fn cell_label(v: f64, cell_w: f64) -> (String, f64) {
    let max_w = cell_w - 5.0;
    let candidates = [
        num(v),
        format!("{:.4}", v),
        format!("{:.2}", v),
        format!("{:.1}", v),
        format!("{:.0}", v),
        format!("{:.1e}", v),
        format!("{:.0e}", v),
    ];
    for size in [10.0, 9.0, 8.0] {
        for c in &candidates {
            if text_w(c, size) <= max_w {
                return (c.clone(), size);
            }
        }
    }
    ("·".into(), 10.0)
}

/// Подпись значения столбца в слот: (текст, кегль), если влезает
/// горизонтально; None — тесно, нужна вертикальная надпись.
fn bar_value_label(text: &str, slot_w: f64) -> Option<(String, f64)> {
    for size in [10.0, 9.0, 8.0, 7.0] {
        if text_w(text, size) <= slot_w - 2.0 {
            return Some((text.to_string(), size));
        }
    }
    None
}

/// Прямоугольник для проверки наложений (экранные координаты).
#[derive(Clone, Copy, Debug)]
struct LBox {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

/// Пересечение прямоугольников с зазором `gap`.
fn boxes_hit(a: &LBox, b: &LBox, gap: f64) -> bool {
    a.x0 < b.x1 + gap && b.x0 < a.x1 + gap && a.y0 < b.y1 + gap && b.y0 < a.y1 + gap
}

/// Размещённая подпись оси: бокс [x, x+w] в ряду `row`, выноска от
/// засечки `tick` рисуется сценой при |центр − засечка| > 5 px.
#[derive(Clone, Debug)]
struct Placed {
    x: f64,
    y: f64,
    w: f64,
    tick: f64,
    row: i32,
    text: String,
}

/// Левый край бокса подписи шириной `wpx` у засечки `tick` — с укором
/// в холст [pad, w−pad] и защитой от паники при ширине больше холста.
fn label_x0(tick: f64, wpx: f64, w: f64, pad: f64) -> f64 {
    if wpx > w - 2.0 * pad {
        return pad;
    }
    let xc = tick.clamp(pad + wpx / 2.0, w - pad - wpx / 2.0);
    xc - wpx / 2.0
}

/// Укладка подписей одномерной оси (грабля 23: чередование «верх/низ»
/// не спасало — соседние по лог-шкале подписи налетали друг на друга).
/// Четыре ряда: ±1 (ближние), ±2 (дальние); row 3 — маркер значения
/// (самый важный, размещается первым). Зазор в ряду — 7 px, всё — в
/// границах холста, детерминированно (порядок слева направо).
fn place_axis_labels(
    items: &[(f64, String)],
    value: Option<(f64, String)>,
    w: f64,
    pad: f64,
    ys: [f64; 5],
    font: f64,
) -> Vec<Placed> {
    let mut placed: Vec<Placed> = Vec::new();
    if let Some((tick, text)) = value {
        let wpx = text_w(&text, font + 3.0);
        placed.push(Placed {
            x: label_x0(tick, wpx, w, pad),
            y: ys[4],
            w: wpx,
            tick,
            row: 3,
            text,
        });
    }
    for (k, (tick, text)) in items.iter().enumerate() {
        let wpx = text_w(text, font);
        let rows: [i32; 4] = if k % 2 == 0 {
            [1, 2, -1, -2]
        } else {
            [-1, -2, 1, 2]
        };
        let mut done = false;
        for &row in &rows {
            let y = match row {
                1 => ys[1],
                2 => ys[0],
                -1 => ys[2],
                _ => ys[3],
            };
            let x0 = label_x0(*tick, wpx, w, pad);
            let clash = placed
                .iter()
                .any(|q| q.row == row && x0 < q.x + q.w + 7.0 && q.x < x0 + wpx + 7.0);
            if !clash {
                placed.push(Placed {
                    x: x0,
                    y,
                    w: wpx,
                    tick: *tick,
                    row,
                    text: text.clone(),
                });
                done = true;
                break;
            }
        }
        if !done {
            // Все четыре ряда конфликтуют: первый свободный горизонтальный
            // зазор дальнего нижнего ряда (с выноской от засечки).
            let mut boxes: Vec<(f64, f64)> = placed
                .iter()
                .filter(|q| q.row == -2)
                .map(|q| (q.x, q.x + q.w))
                .collect();
            boxes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            let mut x = pad;
            for (bx0, bx1) in boxes {
                if x + wpx + 7.0 <= bx0 {
                    break;
                }
                if x < bx1 + 7.0 {
                    x = bx1 + 7.0;
                }
            }
            x = x.clamp(pad, (w - pad - wpx).max(pad));
            placed.push(Placed {
                x,
                y: ys[3],
                w: wpx,
                tick: *tick,
                row: -2,
                text: text.clone(),
            });
        }
    }
    placed
}

/// Разбивка имени узла графа на строки под ширину: 10 pt / 150 px,
/// при необходимости 9 pt / 8 pt. Многострочность ≤2 сохраняет
/// привязку к узлу без наездов на соседей.
fn node_label_lines(name: &str) -> (f64, Vec<String>) {
    for (size, max_w) in [(10.0, 150.0), (9.0, 140.0), (8.0, 130.0)] {
        let lines = wrap_text(name, size, max_w);
        if lines.len() <= 2 {
            return (size, lines);
        }
    }
    (8.0, wrap_text(name, 8.0, 130.0))
}

// ------- Аудит собственного SVG (регрессионная сеть грабли 23) -------

/// Аудит сцены: каждая <text>-надпись обязана лежать в границах холста
/// (viewBox), с учётом text-anchor и поворота. Пустой результат — чисто.
#[allow(dead_code)]
pub fn audit_text_bounds(svg: &str) -> Vec<String> {
    let mut bad = Vec::new();
    let (w, h) = match parse_view_box(svg) {
        Some(v) => v,
        None => {
            bad.push("нет viewBox".into());
            return bad;
        }
    };
    let tol = 1.0;
    for (attrs, content) in text_elements(svg) {
        let content = unesc(content.trim());
        let x = attr_f(&attrs, "x").unwrap_or(0.0);
        let y = attr_f(&attrs, "y").unwrap_or(0.0);
        let size = attr_f(&attrs, "font-size").unwrap_or(12.0);
        let anchor = attr_s(&attrs, "text-anchor").unwrap_or_else(|| "start".into());
        let wpx = text_w(&content, size);
        let (mut x0, mut x1) = match anchor.as_str() {
            "middle" => (x - wpx / 2.0, x + wpx / 2.0),
            "end" => (x - wpx, x),
            _ => (x, x + wpx),
        };
        let mut y0 = y - 0.85 * size;
        let mut y1 = y + 0.30 * size;
        if let Some((a, cx, cy)) = parse_rotate(&attrs) {
            let t = a.to_radians();
            let (ca, sa) = (t.cos(), t.sin());
            let rot = |(px, py): (f64, f64)| -> (f64, f64) {
                let (dx, dy) = (px - cx, py - cy);
                (cx + dx * ca - dy * sa, cy + dx * sa + dy * ca)
            };
            let (ax, ay) = rot((x, y));
            let (bx, by) = rot((x + wpx, y));
            x0 = ax.min(bx) - 0.3 * size;
            x1 = ax.max(bx) + 0.3 * size;
            y0 = ay.min(by) - 0.85 * size;
            y1 = ay.max(by) + 0.30 * size;
        }
        if x0 < -tol || x1 > w + tol || y0 < -tol || y1 > h + tol {
            let head: String = content.chars().take(18).collect();
            bad.push(format!(
                "«{head}…» за холстом {w}×{h}: x[{x0:.0}…{x1:.0}] y[{y0:.0}…{y1:.0}]"
            ));
        }
    }
    bad
}

#[allow(dead_code)]
fn parse_view_box(svg: &str) -> Option<(f64, f64)> {
    let i = svg.find("viewBox=\"")? + 9;
    let rest = &svg[i..];
    let end = rest.find('"')?;
    let mut it = rest[..end].split_whitespace();
    it.next()?;
    it.next()?;
    let w: f64 = it.next()?.parse().ok()?;
    let h: f64 = it.next()?.parse().ok()?;
    Some((w, h))
}

#[allow(dead_code)]
fn text_elements(svg: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = svg;
    while let Some(i) = rest.find("<text") {
        rest = &rest[i + 5..];
        let close = match rest.find('>') {
            Some(c) => c,
            None => break,
        };
        let attrs = rest[..close].to_string();
        let after = &rest[close + 1..];
        let end = match after.find("</text>") {
            Some(e) => e,
            None => break,
        };
        out.push((attrs, after[..end].to_string()));
        rest = &after[end + 7..];
    }
    out
}

#[allow(dead_code)]
fn attr_s(attrs: &str, key: &str) -> Option<String> {
    let pat = format!("{key}=\"");
    let i = attrs.find(&pat)? + pat.len();
    let rest = &attrs[i..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[allow(dead_code)]
fn attr_f(attrs: &str, key: &str) -> Option<f64> {
    attr_s(attrs, key).and_then(|v| v.parse().ok())
}

#[allow(dead_code)]
fn parse_rotate(attrs: &str) -> Option<(f64, f64, f64)> {
    let s = attr_s(attrs, "transform")?;
    let i = s.find("rotate(")? + 7;
    let rest = &s[i..];
    let end = rest.find(')')?;
    let nums: Vec<f64> = rest[..end]
        .split([',', ' '])
        .filter_map(|t| t.trim().parse().ok())
        .collect();
    match nums.len() {
        1 => Some((nums[0], 0.0, 0.0)),
        3 => Some((nums[0], nums[1], nums[2])),
        _ => None,
    }
}

#[allow(dead_code)]
fn unesc(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
}

// ---------------------------------------------------------------------
// viz_bell: радар нарушения неравенства Белла
// ---------------------------------------------------------------------

/// Радар CGLMP: ось I ∈ [1.5, 3.2], зелёная зона классики,
/// вертикали Цирельсона/Ацина/квкварта, маркер значения, вердикт.
pub fn bell_svg(i: f64) -> String {
    const X0: f64 = 60.0;
    const X1: f64 = 610.0;
    const V0: f64 = 1.5;
    const V1: f64 = 3.2;
    let xf = |v: f64| X0 + (v - V0) * (X1 - X0) / (V1 - V0);
    let clamp = |v: f64| v.clamp(V0, V1);

    // Вердикт — до холста: высота примечаний известна заранее (грабля 23:
    // раньше длинные вердикты упирались в правый край).
    let (line1, line2) = bell_verdict(i);
    let notes = vec![
        Note::new(line1, 12.0, C_TEXT),
        Note::new(line2, 11.0, C_MUTED),
    ];
    let h = (204.0 + notes_height(&notes, 60.0, 640.0) + 10.0).ceil() as u32;

    let mut s = svg_open(640, h);
    s.push_str(&title("CGLMP: нарушение неравенства Белла", 640));

    // Зона классики: заливка от левого края до 2.0.
    s.push_str(&format!(
        "<rect x=\"{:.1}\" y=\"48\" width=\"{:.1}\" height=\"120\" fill=\"{}\"/>\n",
        X0,
        xf(BELL_CLASSICAL) - X0,
        C_GREEN_BG
    ));
    s.push_str(&format!(
        "<text x=\"{:.1}\" y=\"44\" font-size=\"11\" fill=\"{}\">классика ≤ 2.0</text>\n",
        X0 + 6.0,
        C_GREEN
    ));

    // Оси и сетка.
    s.push_str(&format!(
        "<line x1=\"{X0}\" y1=\"168\" x2=\"{X1}\" y2=\"168\" stroke=\"{}\"/>\n",
        C_GRID
    ));
    for v in [1.5f64, 2.0, 2.5, 3.0] {
        s.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"168\" x2=\"{:.1}\" y2=\"173\" stroke=\"{}\"/>\n\
             <text x=\"{:.1}\" y=\"186\" font-size=\"10\" fill=\"{}\" text-anchor=\"middle\">{}</text>\n",
            xf(v),
            xf(v),
            C_MUTED,
            xf(v),
            C_MUTED,
            num(v)
        ));
    }

    // Квантовые вертикали: Цирельсон, Ацин d=3, квкварт d=4.
    let markers: [(f64, &str, &str); 3] = [
        (BELL_TSIRELSON, "Цирельсон √8 (кубиты)", C_PURPLE),
        (BELL_ACIN, "Ацин d=3", C_ORANGE),
        (BELL_QUQUART, "квкварт d=4", C_RED),
    ];
    for (v, label, color) in markers {
        let x = xf(v);
        s.push_str(&format!(
            "<line x1=\"{x:.1}\" y1=\"52\" x2=\"{x:.1}\" y2=\"168\" stroke=\"{color}\" \
             stroke-dasharray=\"5 3\" stroke-width=\"1.6\"/>\n\
             <text x=\"{x:.1}\" y=\"{}\" font-size=\"10\" fill=\"{color}\" \
             transform=\"rotate(-38 {x:.1} {})\" font-weight=\"bold\">{}</text>\n",
            128.0,
            128.0,
            esc(label)
        ));
    }

    // Маркер значения.
    let xv = xf(clamp(i));
    s.push_str(&format!(
        "<line x1=\"{xv:.1}\" y1=\"48\" x2=\"{xv:.1}\" y2=\"168\" stroke=\"{}\" stroke-width=\"2.6\"/>\n\
         <circle cx=\"{xv:.1}\" cy=\"108\" r=\"6\" fill=\"#ffffff\" stroke=\"{}\" stroke-width=\"2.6\"/>\n",
        C_RED, C_RED
    ));
    // Грабля 23: у крайних значений метка «I = …» вылезала за холст —
    // укор центра по ширине текста.
    let it = format!("I = {}", num(i));
    let half = text_w(&it, 12.0) / 2.0;
    let xc = xv.clamp(8.0 + half, 632.0 - half);
    s.push_str(&format!(
        "<text x=\"{xc:.1}\" y=\"98\" font-size=\"12\" font-weight=\"bold\" fill=\"{}\" \
         text-anchor=\"middle\">{}</text>\n",
        C_RED,
        esc(&it)
    ));

    // Вердикт: человеческая расшифровка (с уменьшением кегля/переносом).
    let (block, _) = notes_svg(&notes, 60.0, 640.0, 204.0);
    s.push_str(&block);
    s.push_str("</svg>");
    s
}

/// Человекочитаемый вердикт радара: превышение над классикой и
/// Цирельсоном, ранг размерности, порог шума η* = 2/I.
fn bell_verdict(i: f64) -> (String, String) {
    if !i.is_finite() {
        return ("I не конечен — нечего визуализировать".into(), String::new());
    }
    if i <= BELL_CLASSICAL {
        return (
            format!("I = {} ≤ 2.0 — внутри классического предела", num(i)),
            "локальные стратегии справляются: телепатии нет".into(),
        );
    }
    let over_classical = (i / BELL_CLASSICAL - 1.0) * 100.0;
    let mut line1 = format!(
        "I = {} · +{:.2}% над классикой",
        num(i),
        over_classical
    );
    let mut line2 = format!("η* = 2/I = {:.4} (порог шума)", 2.0 / i);
    if i > BELL_TSIRELSON {
        line1.push_str(&format!(
            " · +{:.2}% над Цирельсоном",
            (i / BELL_TSIRELSON - 1.0) * 100.0
        ));
        let rank = [
            (BELL_TSIRELSON, "кубиты (Цирельсон)"),
            (BELL_ACIN, "кутриты (Ацин d=3)"),
            (BELL_QUQUART, "квкварты (d=4)"),
        ]
        .iter()
        .min_by(|a, b| {
            (a.0 - i).abs().partial_cmp(&(b.0 - i).abs()).unwrap()
        })
        .map(|(_, r)| *r)
        .unwrap_or("?");
        line2.push_str(&format!(" · ближайший ранг: {rank}"));
    } else {
        line2.push_str(" — ниже кубитного предела √8");
    }
    (line1, line2)
}

// ---------------------------------------------------------------------
// viz_scale: лог-шкала Вселенной с человеческими якорями
// ---------------------------------------------------------------------

/// Лог-шкала Вселенной (метры): 1e-35 → 1e+27, якоря от Планковской
/// длины до наблюдаемой Вселенной, позиция значения и отношение
/// к человеческому росту.
pub fn scale_svg(x_m: f64) -> Result<String, String> {
    if !x_m.is_finite() || x_m <= 0.0 {
        return Err(format!(
            "viz_scale: нужно x > 0 (логарифмическая шкала), получено {}",
            num(x_m)
        ));
    }
    const X0: f64 = 70.0;
    const X1: f64 = 620.0;
    const L0: f64 = -35.0;
    const L1: f64 = 27.0;
    let xf = |lg: f64| X0 + (lg - L0) * (X1 - X0) / (L1 - L0);

    // Примечания — до отрисовки: высота холста известна заранее
    // (грабля 23: раньше третья строка обрезалась на правом краю).
    let (line1, line2) = scale_verdict(x_m);
    let line3 = "якоря: Планк → протон → атом → ДНК → вирус → эритроцит → человек → \
                 Эверест → Земля → Солнце → орбита → парсек → Галактика → Вселенная"
        .to_string();
    let notes = vec![
        Note::new(line1, 12.0, C_TEXT),
        Note::new(line2, 11.0, C_TEXT),
        Note::new(line3, 10.0, C_MUTED),
    ];
    const W: f64 = 640.0;
    const NX: f64 = 70.0;
    const NOTES_Y: f64 = 246.0;
    let h = (NOTES_Y + notes_height(&notes, NX, W) + 10.0).ceil() as u32;

    // Укладка подписей якорей (грабля 23: чередование «верх/низ» не
    // спасало — «Земля» налетала на «орбиту Земли», «ДНК» на «вирус»).
    // Ряды: [дальний верх, ближний верх, ближний низ, дальний низ,
    // маркер значения]; маркер размещается первым и в своём ряду.
    const BAND_TOP: f64 = 132.0;
    const BAND_BOT: f64 = 172.0;
    let ys = [96.0, 114.0, 192.0, 210.0, 66.0];
    let items: Vec<(f64, String)> = SCALE_ANCHORS
        .iter()
        .map(|(name, m)| (xf(m.log10()), (*name).to_string()))
        .collect();
    let lg = x_m.log10();
    let xv = xf(lg.clamp(L0, L1));
    let placed = place_axis_labels(
        &items,
        Some((xv, format!("{} м", num(x_m)))),
        W,
        8.0,
        ys,
        9.0,
    );

    let mut s = svg_open(W as u32, h);
    s.push_str(&title("Шкала Вселенной (логарифм, метры)", W as u32));

    // Полоса шкалы и декадные засечки.
    s.push_str(&format!(
        "<rect x=\"{X0}\" y=\"{BAND_TOP}\" width=\"{:.1}\" height=\"40\" fill=\"#f3f4f6\" \
         stroke=\"{}\"/>\n",
        X1 - X0,
        C_GRID
    ));
    for l in [-30i32, -20, -10, 0, 10, 20] {
        let x = xf(l as f64);
        s.push_str(&format!(
            "<line x1=\"{x:.1}\" y1=\"{BAND_TOP}\" x2=\"{x:.1}\" y2=\"{BAND_BOT}\" stroke=\"{}\"/>\n\
             <text x=\"{x:.1}\" y=\"226\" font-size=\"9\" fill=\"{}\" \
             text-anchor=\"middle\">1e{l}</text>\n",
            C_GRID, C_MUTED
        ));
    }

    // Якоря: засечка + подпись (с выноской, если подпись ушла от засечки).
    for p in &placed {
        let is_value = p.row == 3;
        if !is_value {
            s.push_str(&format!(
                "<line x1=\"{:.1}\" y1=\"{BAND_TOP}\" x2=\"{:.1}\" y2=\"{BAND_BOT}\" \
                 stroke=\"{}\"/>\n",
                p.tick, p.tick, C_MUTED
            ));
        }
        let center = p.x + p.w / 2.0;
        if (center - p.tick).abs() > 5.0 {
            // Выноска от засечки к ближнему краю подписи.
            let edge = if center > p.tick { p.x } else { p.x + p.w };
            let (y_from, y_to) = if p.y < BAND_TOP {
                (BAND_TOP, p.y + 3.0)
            } else {
                (BAND_BOT, p.y - 5.0)
            };
            s.push_str(&format!(
                "<line x1=\"{:.1}\" y1=\"{y_from:.1}\" x2=\"{edge:.1}\" y2=\"{y_to:.1}\" \
                 stroke=\"#9ca3af\" stroke-width=\"0.6\"/>\n",
                p.tick
            ));
        }
        let (fill, weight, size) = if is_value {
            (C_RED, " font-weight=\"bold\"", 12.0)
        } else {
            (C_MUTED, "", 9.0)
        };
        s.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"{size:.0}\"{weight} \
             fill=\"{fill}\">{}</text>\n",
            p.x,
            p.y,
            esc(&p.text)
        ));
    }
    // Линия маркера значения — через полосу (не задевает ряды подписей).
    s.push_str(&format!(
        "<line x1=\"{xv:.1}\" y1=\"128\" x2=\"{xv:.1}\" y2=\"176\" stroke=\"{}\" \
         stroke-width=\"2.6\"/>\n",
        C_RED
    ));

    let (block, _) = notes_svg(&notes, NX, W, NOTES_Y);
    s.push_str(&block);
    s.push_str("</svg>");
    Ok(s)
}

/// Якорный вердикт: соседние якоря и отношение к росту человека.
fn scale_verdict(x_m: f64) -> (String, String) {
    // Соседние якоря: последний ≤ x и первый > x.
    let mut below: Option<(usize)> = None;
    let mut above: Option<usize> = None;
    for (k, (_, m)) in SCALE_ANCHORS.iter().enumerate() {
        if *m <= x_m {
            below = Some(k);
        } else if above.is_none() {
            above = Some(k);
        }
    }
    let mut line1 = String::new();
    match (below, above) {
        (Some(b), Some(a)) => line1.push_str(&format!(
            "между «{}» и «{}»",
            SCALE_ANCHORS[b].0, SCALE_ANCHORS[a].0
        )),
        (Some(b), None) => line1.push_str(&format!(
            "за пределом «{}» — крупнее наблюдаемой Вселенной",
            SCALE_ANCHORS[b].0
        )),
        (None, Some(a)) => line1.push_str(&format!(
            "мельче «{}» — глубже Планковской длины",
            SCALE_ANCHORS[a].0
        )),
        (None, None) => line1.push_str("вне шкалы"),
    }

    // Отношение к человеку.
    let ratio = x_m / HUMAN_M;
    let line2 = if ratio.log10().abs() < 0.01 {
        format!("это и есть человеческий масштаб ({HUMAN_M} м)")
    } else if ratio > 1.0 {
        format!(
            "в {} раз больше человека ({HUMAN_M} м)",
            human_ratio(ratio)
        )
    } else {
        format!(
            "в {} раз меньше человека ({HUMAN_M} м)",
            human_ratio(1.0 / ratio)
        )
    };
    (line1, line2)
}

// ---------------------------------------------------------------------
// viz_prob: Born-вероятности столбцами
// ---------------------------------------------------------------------

/// Столбчатая диаграмма вероятностей: нормировка по максимуму,
/// проценты от суммы, подсветка argmax, контроль Σ.
pub fn prob_svg(ps: &[f64]) -> Result<String, String> {
    if ps.is_empty() {
        return Err("viz_prob: пустой список вероятностей".into());
    }
    if ps.iter().any(|p| !p.is_finite() || *p < 0.0) {
        return Err("viz_prob: вероятности должны быть ≥ 0".into());
    }
    if ps.len() > 40 {
        return Err(format!(
            "viz_prob: слишком много исходов ({}, до 40)",
            ps.len()
        ));
    }
    let n = ps.len() as f64;
    let bw = (540.0 / n - 8.0).min(78.0);
    let x0 = 60.0 + (540.0 - (bw + 8.0) * n) / 2.0;
    let sum: f64 = ps.iter().sum();
    let pmax = ps.iter().cloned().fold(0.0, f64::max).max(1e-12);
    let amax = ps
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(k, _)| k)
        .unwrap_or(0);

    // Подписи исходов (грабля 23): при тесных слотах — вертикально,
    // с укором вниз, чтобы не наехать на заголовок.
    let labels: Vec<String> = ps
        .iter()
        .map(|p| format!("{:.2}%", if sum > 0.0 { p / sum * 100.0 } else { 0.0 }))
        .collect();
    let slot = bw + 8.0;
    let crowded = labels.iter().any(|t| bar_value_label(t, slot).is_none());
    let idx_w = text_w(&format!("p{}", ps.len() - 1), 10.0);
    let idx_step = ((idx_w + 2.0) / slot).ceil().max(1.0) as usize;
    let idx_size = if idx_w <= slot - 2.0 { 10.0 } else { 8.0 };

    let notes = vec![Note::new(
        format!(
            "Σ = {} · максимум p{amax} = {} · Born-коллапс: один исход станет реальностью",
            num(sum),
            num(ps[amax])
        ),
        12.0,
        C_TEXT,
    )];
    const W: f64 = 640.0;
    const NOTES_Y: f64 = 222.0;
    let h = (NOTES_Y + notes_height(&notes, 60.0, W) + 8.0).ceil() as u32;

    let mut s = svg_open(W as u32, h);
    s.push_str(&title("Born: вероятности исходов", W as u32));
    s.push_str(&format!(
        "<line x1=\"60\" y1=\"180\" x2=\"600\" y2=\"180\" stroke=\"{}\"/>\n",
        C_GRID
    ));
    for (k, p) in ps.iter().enumerate() {
        let x = x0 + (bw + 8.0) * k as f64;
        let hgt = (p / pmax * 118.0).max(1.5);
        let fill = if k == amax { C_BLUE } else { C_BLUE_BG };
        let xc = x + bw / 2.0;
        s.push_str(&format!(
            "<rect x=\"{x:.1}\" y=\"{:.1}\" width=\"{bw:.1}\" height=\"{hgt:.1}\" \
             fill=\"{fill}\"/>\n",
            180.0 - hgt
        ));
        let color = if k == amax { C_BLUE } else { C_MUTED };
        let bold = if k == amax { " font-weight=\"bold\"" } else { "" };
        if crowded {
            // Вертикальная надпись над столбцом.
            let yv = (172.0 - hgt - 4.0).max(30.0 + text_w(&labels[k], 8.0));
            s.push_str(&format!(
                "<text x=\"{xc:.1}\" y=\"{yv:.1}\" font-size=\"8\"{bold} fill=\"{color}\" \
                 transform=\"rotate(-90 {xc:.1} {yv:.1})\" \
                 text-anchor=\"start\">{}</text>\n",
                esc(&labels[k])
            ));
        } else if let Some((t, size)) = bar_value_label(&labels[k], slot) {
            s.push_str(&format!(
                "<text x=\"{xc:.1}\" y=\"{:.1}\" font-size=\"{size:.0}\"{bold} fill=\"{color}\" \
                 text-anchor=\"middle\">{}</text>\n",
                172.0 - hgt,
                esc(&t)
            ));
        }
        if k % idx_step == 0 {
            s.push_str(&format!(
                "<text x=\"{xc:.1}\" y=\"196\" font-size=\"{idx_size:.0}\" fill=\"{}\" \
                 text-anchor=\"middle\">p{k}</text>\n",
                C_MUTED
            ));
        }
    }
    let (block, _) = notes_svg(&notes, 60.0, W, NOTES_Y);
    s.push_str(&block);
    s.push_str("</svg>");
    Ok(s)
}

// ---------------------------------------------------------------------
// viz_matrix: теплокарта матрицы
// ---------------------------------------------------------------------

/// Теплокарта: v ≥ 0 → красная интенсивность, v < 0 → синяя,
/// |v|/max нормирует яркость; подписи клеток при n ≤ 8.
/// Ограничение 64×64 (4096 клеток) — гигиена размера сцены.
pub fn matrix_svg(m: &Matrix) -> Result<String, String> {
    if m.rows == 0 || m.cols == 0 {
        return Err("viz_matrix: пустая матрица".into());
    }
    if m.rows > 64 || m.cols > 64 {
        return Err(format!(
            "viz_matrix: {}×{} — слишком крупно для сцены (до 64×64)",
            m.rows, m.cols
        ));
    }
    const CELL: f64 = 46.0;
    let (ox, oy) = (56.0, 34.0);
    let label_cells = m.rows <= 8 && m.cols <= 8;

    let vmax = (0..m.rows)
        .flat_map(|i| (0..m.cols).map(move |j| m.get(i, j)))
        .map(|c| c.re.hypot(c.im))
        .fold(0.0f64, f64::max)
        .max(1e-12);

    // След и примечание — до холста (грабля 23: у широких матриц
    // подпись клеток вылезала за клетку, футер — за холст).
    let tr: f64 = (0..m.rows.min(m.cols)).map(|k| m.get(k, k).re).sum();
    let notes = vec![Note::new(
        format!(
            "{}×{} · |max| = {} · Re(след) = {} · красный ≥ 0, синий < 0",
            m.rows,
            m.cols,
            num(vmax),
            num(tr)
        ),
        11.0,
        C_TEXT,
    )];
    let w = (ox + m.cols as f64 * CELL + 8.0).max(420.0);
    let notes_y = oy + m.rows as f64 * CELL + 30.0;
    let h = (notes_y + notes_height(&notes, ox, w) + 8.0).ceil() as u32;

    let mut s = svg_open(w as u32, h);
    s.push_str(&title(&format!("Матрица {}×{}", m.rows, m.cols), w as u32));
    for i in 0..m.rows {
        for j in 0..m.cols {
            let c = m.get(i, j);
            let mag = c.re.hypot(c.im) / vmax;
            let base = if c.re >= 0.0 { "#ef4444" } else { "#3b82f6" };
            let x = ox + j as f64 * CELL;
            let y = oy + i as f64 * CELL;
            // Полупрозрачная интенсивность поверх белого.
            s.push_str(&format!(
                "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{}\" height=\"{}\" \
                 fill=\"#ffffff\" stroke=\"{}\"/>\n\
                 <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{}\" height=\"{}\" \
                 fill=\"{base}\" fill-opacity=\"{:.3}\" stroke=\"{}\"/>\n",
                CELL - 1.0,
                CELL - 1.0,
                C_GRID,
                CELL - 1.0,
                CELL - 1.0,
                (0.12 + 0.88 * mag).min(1.0),
                C_GRID
            ));
            if label_cells {
                // Подпись клетки вписывается в клетку: num() → короче → мельче.
                let dark = mag > 0.55;
                let (txt, size) = cell_label(c.re, CELL);
                s.push_str(&format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"{size:.0}\" fill=\"{}\" \
                     text-anchor=\"middle\">{}</text>\n",
                    x + CELL / 2.0,
                    y + CELL / 2.0 + size * 0.35,
                    if dark { "#ffffff" } else { C_TEXT },
                    esc(&txt)
                ));
            }
        }
    }
    let (block, _) = notes_svg(&notes, ox, w, notes_y);
    s.push_str(&block);
    s.push_str("</svg>");
    Ok(s)
}

// ---------------------------------------------------------------------
// viz_bars: спектр значений
// ---------------------------------------------------------------------

/// Столбцы спектра: положительные — вверх (синие), отрицательные —
/// вниз (оранжевые); подписи значений и сводка min/max/среднее.
pub fn bars_svg(vs: &[f64]) -> Result<String, String> {
    bars_svg_titled(vs, "Спектр значений")
}

/// То же с произвольным заголовком (для авто-маршрута Re/Im).
pub fn bars_svg_titled(vs: &[f64], head: &str) -> Result<String, String> {
    if vs.is_empty() {
        return Err("viz_bars: пустой список".into());
    }
    if vs.iter().any(|v| !v.is_finite()) {
        return Err("viz_bars: не конечные значения".into());
    }
    if vs.len() > 40 {
        return Err(format!(
            "viz_bars: слишком много значений ({}, до 40)",
            vs.len()
        ));
    }
    let n = vs.len() as f64;
    let bw = (540.0 / n - 8.0).min(78.0);
    let x0 = 60.0 + (540.0 - (bw + 8.0) * n) / 2.0;
    let vmax = vs.iter().cloned().fold(0.0f64, f64::max)
        .max(vs.iter().cloned().fold(0.0f64, f64::min).abs())
        .max(1e-12);
    let base_y = 130.0; // ноль спектра

    // Подписи значений (грабля 23): тесные слоты → вертикально
    // (положительные — вверх от столбца, отрицательные — вниз от
    // нижнего конца; холст растёт под них заранее).
    let labels: Vec<String> = vs.iter().map(|v| num(*v)).collect();
    let slot = bw + 8.0;
    let crowded = labels.iter().any(|t| bar_value_label(t, slot).is_none());
    let vert_w = labels.iter().map(|t| text_w(t, 8.0)).fold(0.0, f64::max);
    let has_neg_vert = crowded && vs.iter().any(|v| *v < 0.0);
    let idx_w = text_w(&format!("[{}]", vs.len() - 1), 10.0);
    let idx_step = ((idx_w + 2.0) / slot).ceil().max(1.0) as usize;
    let idx_size = if idx_w <= slot - 2.0 { 10.0 } else { 8.0 };

    let notes = vec![Note::new(
        format!(
            "n = {} · min = {} · max = {} · среднее = {}",
            vs.len(),
            num(vs.iter().cloned().fold(f64::INFINITY, f64::min)),
            num(vs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),
            num(vs.iter().sum::<f64>() / n)
        ),
        12.0,
        C_TEXT,
    )];
    const W: f64 = 640.0;
    let neg_bottom = if has_neg_vert { 196.0 + vert_w } else { 206.0 };
    let idx_y = neg_bottom + 12.0;
    let notes_y = idx_y + 18.0;
    let h = (notes_y + notes_height(&notes, 60.0, W) + 8.0).ceil() as u32;

    let mut s = svg_open(W as u32, h);
    s.push_str(&title(head, W as u32));
    s.push_str(&format!(
        "<line x1=\"60\" y1=\"{base_y}\" x2=\"600\" y2=\"{base_y}\" stroke=\"{}\"/>\n",
        C_GRID
    ));
    for (k, v) in vs.iter().enumerate() {
        let x = x0 + (bw + 8.0) * k as f64;
        let frac = v / vmax; // ∈ [-1, 1]
        let (y, hh) = if frac >= 0.0 {
            (base_y - frac * 62.0, frac * 62.0)
        } else {
            (base_y, -frac * 62.0)
        };
        let fill = if frac >= 0.0 { C_BLUE } else { C_ORANGE };
        let xc = x + bw / 2.0;
        s.push_str(&format!(
            "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{bw:.1}\" height=\"{:.1}\" \
             fill=\"{fill}\" fill-opacity=\"0.85\"/>\n",
            hh.max(1.5)
        ));
        if crowded {
            let wpx = text_w(&labels[k], 8.0);
            if frac >= 0.0 {
                let yv = (y - 4.0).max(30.0 + wpx);
                s.push_str(&format!(
                    "<text x=\"{xc:.1}\" y=\"{yv:.1}\" font-size=\"8\" fill=\"{}\" \
                     transform=\"rotate(-90 {xc:.1} {yv:.1})\" \
                     text-anchor=\"start\">{}</text>\n",
                    C_MUTED,
                    esc(&labels[k])
                ));
            } else {
                let yv = y + hh + 4.0;
                s.push_str(&format!(
                    "<text x=\"{xc:.1}\" y=\"{yv:.1}\" font-size=\"8\" fill=\"{}\" \
                     transform=\"rotate(90 {xc:.1} {yv:.1})\" \
                     text-anchor=\"start\">{}</text>\n",
                    C_MUTED,
                    esc(&labels[k])
                ));
            }
        } else if let Some((t, size)) = bar_value_label(&labels[k], slot) {
            let ly = if frac >= 0.0 { y - 6.0 } else { y + hh + 14.0 };
            s.push_str(&format!(
                "<text x=\"{xc:.1}\" y=\"{ly:.1}\" font-size=\"{size:.0}\" fill=\"{}\" \
                 text-anchor=\"middle\">{}</text>\n",
                C_MUTED,
                esc(&t)
            ));
        }
        if k % idx_step == 0 {
            s.push_str(&format!(
                "<text x=\"{xc:.1}\" y=\"{idx_y:.1}\" font-size=\"{idx_size:.0}\" fill=\"{}\" \
                 text-anchor=\"middle\">[{k}]</text>\n",
                C_MUTED
            ));
        }
    }
    let (block, _) = notes_svg(&notes, 60.0, W, notes_y);
    s.push_str(&block);
    s.push_str("</svg>");
    Ok(s)
}

// ---------------------------------------------------------------------
// viz(x): автоматический маршрут «тип значения → правило → сцена»
// ---------------------------------------------------------------------

/// Автодиспетчер — тот самый «автоматический слой» из идеи:
/// Калькулятор посчитал, слой сам выбирает наглядную сцену
/// по типу значения.
pub fn auto_svg(v: &Value) -> Result<String, String> {
    match v {
        Value::Matrix(m) => {
            // Вектор-матрица (1×N или N×1) — это список чисел
            // в синтаксисе языка: [0.8, 0.2] → Born-бары.
            if let Some(ns) = matrix_as_vector(m) {
                route_numbers(&ns)
            } else {
                matrix_svg(m)
            }
        }
        Value::List(items) => {
            let nums: Option<Vec<f64>> = items
                .iter()
                .map(|x| match x {
                    Value::Scalar(f) => Some(*f),
                    Value::Quantity(q, u) if u.is_dimensionless() => Some(*q),
                    _ => None,
                })
                .collect();
            match nums {
                Some(ns) => route_numbers(&ns),
                None => Err(
                    "viz(список): элементы должны быть числами — например viz_bars([1, -2, 3])"
                        .into(),
                ),
            }
        }
        Value::Quantity(q, u) => {
            let pure_length = u.dim[0] == 1 && u.dim[1..].iter().all(|d| *d == 0);
            if pure_length {
                scale_svg(q * u.factor)
            } else if u.is_dimensionless() {
                auto_scalar(*q)
            } else {
                Err(format!(
                    "viz: величина в «{}» — шкала Вселенной в метрах; \
                     переведите в длину или подайте число",
                    u.display()
                ))
            }
        }
        Value::Scalar(x) => auto_scalar(*x),
        Value::Complex(c) => {
            bars_svg_titled(&[c.re, c.im], "Комплексное число: Re и Im")
        }
        Value::LogProb(p) => {
            // сессия-14: представимая вероятность идёт обычным маршрутом
            // (0.5 → шкала), сверхмалая — честная диагностика глубины
            // (как BigInt: точная величина вне лог-шкалы сцены 10^±27).
            match p.to_f64() {
                Some(x) if x > 0.0 => auto_scalar(x),
                _ => Err(format!(
                    "viz(лог-вероятность {} = {}): величина за пределами \
                     лог-шкалы сцены (10^±27) — глубина {} тритов «против»; \
                     подайте log10(P) числом: viz(log10(logpow(2, -1e15)))",
                    p.fmt10(),
                    p.fmt3(),
                    crate::calc::logprob::LogProb::trit_depth(p)
                )),
            }
        }
        Value::BigInt(n) => Err(format!(
            "viz(BigInt с {N} цифрами): точная большая арифметика — вне \
             лог-шкалы; подайте log10 вручную",
            N = n.trim_start_matches('-').len()
        )),
        Value::Str(s) => Err(format!("viz(строка «{s}»): нечего визуализировать")),
    }
}

/// Маршрут числового набора: вероятности (Σ ≈ 1, все ≥ 0) →
/// Born-бары, иначе спектр.
fn route_numbers(ns: &[f64]) -> Result<String, String> {
    let sum: f64 = ns.iter().sum();
    if !ns.is_empty() && ns.iter().all(|p| *p >= 0.0) && (sum - 1.0).abs() < 0.02 {
        prob_svg(ns)
    } else {
        bars_svg(ns)
    }
}

/// Матрица-вектор (1×N либо N×1) → список чисел (Re-часть).
/// Синтаксис языка: [0.8, 0.2] — это строка-матрица.
pub fn matrix_as_vector(m: &Matrix) -> Option<Vec<f64>> {
    if m.rows == 1 {
        Some((0..m.cols).map(|j| m.get(0, j).re).collect())
    } else if m.cols == 1 {
        Some((0..m.rows).map(|i| m.get(i, 0).re).collect())
    } else {
        None
    }
}

/// Скаляр: окно CGLMP [2, 3.2] → радар Белла; положительное →
/// лог-шкала; иначе — одиночный бар.
fn auto_scalar(x: f64) -> Result<String, String> {
    if (2.0..3.2).contains(&x) {
        Ok(bell_svg(x))
    } else if x > 0.0 {
        scale_svg(x)
    } else {
        bars_svg(&[x])
    }
}

// =====================================================================
// Сессия-9: СУВЕРЕННЫЙ РЕНДЕР — графы и поля нативным ядром
// =====================================================================
// «От перестановки слагаемых сумма не меняется»: чистая математика
// рендеринга, вырезанная из стандартных библиотек, перенесена в
// монолитный движок без единой зависимости:
//   • Fruchterman–Reingold (Graphviz/NetworkX) → viz_graph;
//   • marching squares (Matplotlib contour)    → viz_field;
//   • поворот + ортоскопия + painter's-алгоритм
//     (Matplotlib plot_surface)                → viz_surf.
// Детерминизм: старт узлов — золотая спираль, итерации фиксированы,
// RNG нет вовсе — биты укладки воспроизводимы от запуска к запуску.

/// Точка плоскости (сеточные либо экранные координаты).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pt {
    x: f64,
    y: f64,
}

/// Золотой угол (радианы): шаг спирали Фибоначчи (Vogel).
const GOLDEN_ANGLE: f64 = 2.399963229728653;

/// Итераций силовой укладки (фиксировано — детерминизм).
const FR_ITERS: usize = 300;

/// Линейная интерполяция hex-цвета: #rrggbb → #rrggbb.
fn lerp_hex(c0: &str, c1: &str, t: f64) -> String {
    let parse = |c: &str| -> [u8; 3] {
        let v = u32::from_str_radix(&c[1..], 16).unwrap_or(0);
        [(v >> 16) as u8, (v >> 8) as u8, v as u8]
    };
    let [r0, g0, b0] = parse(c0);
    let [r1, g1, b1] = parse(c1);
    let t = t.clamp(0.0, 1.0);
    format!(
        "#{:02x}{:02x}{:02x}",
        (r0 as f64 + (r1 as f64 - r0 as f64) * t).round() as u8,
        (g0 as f64 + (g1 as f64 - g0 as f64) * t).round() as u8,
        (b0 as f64 + (b1 as f64 - b0 as f64) * t).round() as u8
    )
}

// ---------------------------------------------------------------------
// viz_graph: силовая укладка Fruchterman–Reingold
// ---------------------------------------------------------------------

/// Укладка Fruchterman–Reingold: отталкивание всех пар k²/d,
/// притяжение рёбер d²/k (вес нормирован на максимум), смещение
/// ограничено температурой, остывающей линейно; рамка жёсткая.
/// Старт — золотая спираль: без RNG, бит-в-бит стабильно.
fn fruchterman_reingold(
    n: usize,
    edges: &[(usize, usize, f64)],
    x0: f64,
    y0: f64,
    w: f64,
    h: f64,
) -> Vec<Pt> {
    let k = (w * h / n.max(1) as f64).sqrt();
    let (cx, cy) = (x0 + w / 2.0, y0 + h / 2.0);
    let rmax = w.min(h) * 0.45;
    let mut pos: Vec<Pt> = (0..n)
        .map(|i| {
            let r = rmax * ((i as f64 + 0.5) / n as f64).sqrt();
            let a = i as f64 * GOLDEN_ANGLE;
            Pt {
                x: cx + r * a.cos(),
                y: cy + r * a.sin(),
            }
        })
        .collect();
    let wmax = edges
        .iter()
        .map(|e| e.2)
        .fold(0.0f64, f64::max)
        .max(1e-12);
    let t0 = w / 10.0;
    for it in 0..FR_ITERS {
        let temp = (t0 * (1.0 - it as f64 / FR_ITERS as f64)).max(0.01);
        let mut disp = vec![(0.0f64, 0.0f64); n];
        // Отталкивание всех пар.
        for i in 0..n {
            for j in (i + 1)..n {
                let (dx, dy) = (pos[i].x - pos[j].x, pos[i].y - pos[j].y);
                let d = (dx * dx + dy * dy).sqrt().max(0.01);
                let f = k * k / d;
                let (ux, uy) = (dx / d, dy / d);
                disp[i].0 += ux * f;
                disp[i].1 += uy * f;
                disp[j].0 -= ux * f;
                disp[j].1 -= uy * f;
            }
        }
        // Притяжение рёбер (вес нормирован).
        for &(a, b, wgt) in edges {
            let (dx, dy) = (pos[b].x - pos[a].x, pos[b].y - pos[a].y);
            let d = (dx * dx + dy * dy).sqrt().max(0.01);
            let f = d * d / k * (wgt / wmax);
            let (ux, uy) = (dx / d, dy / d);
            disp[a].0 -= ux * f;
            disp[a].1 -= uy * f;
            disp[b].0 += ux * f;
            disp[b].1 += uy * f;
        }
        // Шаг с температурным потолком + жёсткая рамка.
        for i in 0..n {
            let (dx, dy) = disp[i];
            let d = (dx * dx + dy * dy).sqrt();
            if d > 1e-12 {
                let step = d.min(temp);
                pos[i].x += dx / d * step;
                pos[i].y += dy / d * step;
            }
            pos[i].x = pos[i].x.clamp(x0, x0 + w);
            pos[i].y = pos[i].y.clamp(y0, y0 + h);
        }
    }
    pos
}

/// Компоненты связности: union-find со сжатием пути.
fn components(n: usize, edges: &[(usize, usize, f64)]) -> usize {
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut Vec<usize>, i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        let mut c = i;
        while p[c] != r {
            let nx = p[c];
            p[c] = r;
            c = nx;
        }
        r
    }
    for &(a, b, _) in edges {
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb {
            parent[ra] = rb;
        }
    }
    (0..n).filter(|&i| find(&mut parent, i) == i).count()
}

/// Силовая схема графа: квадратная матрица смежности (Re-часть,
/// |w| — вес ребра, неориентированная симметризация max(|a_ij|, |a_ji|),
/// петли не рисуются) → SVG. Рёбра — толщиной по весу, узлы —
/// радиусом и цветом по степени, вердикт — со статистикой графа.
pub fn graph_svg(m: &Matrix, labels: Option<&[String]>) -> Result<String, String> {
    if m.rows != m.cols {
        return Err(format!(
            "viz_graph: матрица смежности должна быть квадратной, получено {}×{}",
            m.rows, m.cols
        ));
    }
    if m.rows == 0 {
        return Err("viz_graph: пустая матрица смежности".into());
    }
    if m.rows > 64 {
        return Err(format!(
            "viz_graph: {} узлов — слишком крупно для сцены (до 64)",
            m.rows
        ));
    }
    let n = m.rows;
    for i in 0..n {
        for j in 0..n {
            if !m.get(i, j).re.is_finite() {
                return Err(format!("viz_graph: вес ({i},{j}) не конечен"));
            }
        }
    }
    let names: Vec<String> = match labels {
        Some(ls) => {
            if ls.len() != n {
                return Err(format!(
                    "viz_graph: {} меток для {} узлов — должно совпадать",
                    ls.len(),
                    n
                ));
            }
            ls.to_vec()
        }
        None => (0..n).map(|k| format!("v{k}")).collect(),
    };

    // Рёбра: симметризация, петли считаются, но не рисуются.
    let mut edges: Vec<(usize, usize, f64)> = Vec::new();
    let mut loops = 0usize;
    for i in 0..n {
        if m.get(i, i).re.abs() > 1e-9 {
            loops += 1;
        }
    }
    for i in 0..n {
        for j in (i + 1)..n {
            let w = m.get(i, j).re.abs().max(m.get(j, i).re.abs());
            if w > 1e-9 {
                edges.push((i, j, w));
            }
        }
    }

    let (fx, fy, fw, fh) = (76.0, 56.0, 488.0, 296.0);
    let mut pos = fruchterman_reingold(n, &edges, fx, fy, fw, fh);

    let mut deg = vec![0usize; n];
    for &(a, b, _) in &edges {
        deg[a] += 1;
        deg[b] += 1;
    }
    let dmax = deg.iter().cloned().max().unwrap_or(1).max(1);
    // Радиусы нужны до релаксации: узлы — твёрдые диски.
    let radii: Vec<f64> = (0..n)
        .map(|i| 7.0 + 6.0 * (deg[i] as f64 / dmax as f64).sqrt())
        .collect();
    // Релаксация перекрытий (грабля 20: в плотных графах FR-равновесие
    // допускает слипание узлов): разводим пары вдоль линии центров,
    // детерминированно, в рамке.
    for _ in 0..80 {
        let mut moved = false;
        for i in 0..n {
            for j in (i + 1)..n {
                let (dx, dy) = (pos[j].x - pos[i].x, pos[j].y - pos[i].y);
                let d = (dx * dx + dy * dy).sqrt();
                let min_d = radii[i] + radii[j] + 6.0;
                if d < min_d {
                    let (ux, uy) = if d > 1e-9 {
                        (dx / d, dy / d)
                    } else {
                        // Точное совпадение: разводим по золотому углу пары.
                        (GOLDEN_ANGLE.cos(), GOLDEN_ANGLE.sin())
                    };
                    let push = (min_d - d) / 2.0;
                    pos[i].x -= ux * push;
                    pos[i].y -= uy * push;
                    pos[j].x += ux * push;
                    pos[j].y += uy * push;
                    moved = true;
                }
            }
        }
        for p in pos.iter_mut() {
            p.x = p.x.clamp(fx, fx + fw);
            p.y = p.y.clamp(fy, fy + fh);
        }
        if !moved {
            break;
        }
    }
    let wmax = edges
        .iter()
        .map(|e| e.2)
        .fold(0.0f64, f64::max)
        .max(1e-12);

    // Вердикт и холст — до отрисовки (грабля 23: строки статистики
    // упирались в правый край).
    let comps = components(n, &edges);
    let mean_deg = 2.0 * edges.len() as f64 / n as f64;
    let density = if n > 1 {
        edges.len() as f64 * 2.0 / (n * (n - 1)) as f64 * 100.0
    } else {
        0.0
    };
    let mut line2 = format!(
        "укладка: {FR_ITERS} итераций FR · k = sqrt(area/N) · старт — золотая \
         спираль (детерминизм, без RNG)"
    );
    if loops > 0 {
        line2.push_str(&format!(" · петель {loops} (не рисуются)"));
    }
    let notes = vec![
        Note::new(
            format!(
                "N = {} · рёбер = {} · средняя степень = {:.2} · плотность = {:.1}% · \
                 компонент = {}",
                n, edges.len(), mean_deg, density, comps
            ),
            12.0,
            C_TEXT,
        ),
        Note::new(line2, 11.0, C_MUTED),
        Note::new(
            format!(
                "узлы: радиус и цвет — по степени ({} → {}); рёбра: толщина — по весу",
                C_BLUE_BG, C_RED
            ),
            10.0,
            C_MUTED,
        ),
    ];
    const W: u32 = 640;
    const NOTES_Y: f64 = 396.0;
    let h = (NOTES_Y + notes_height(&notes, 60.0, W as f64) + 14.0).ceil() as u32;

    let mut s = svg_open(W, h);
    s.push_str(&title("Граф: силовая укладка Fruchterman–Reingold", W));
    // Рёбра.
    for &(a, b, w) in &edges {
        let sw = 1.0 + 2.5 * (w / wmax);
        s.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
             stroke=\"#64748b\" stroke-opacity=\"0.7\" stroke-width=\"{sw:.2}\"/>\n",
            pos[a].x, pos[a].y, pos[b].x, pos[b].y
        ));
    }
    // Узлы: радиус и цвет растут со степенью (метки — после, поверх).
    for i in 0..n {
        let r = radii[i];
        let fill = lerp_hex(C_BLUE_BG, C_RED, deg[i] as f64 / dmax as f64);
        s.push_str(&format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{r:.1}\" fill=\"{fill}\" \
             stroke=\"#1f2937\" stroke-width=\"1\"/>\n",
            pos[i].x, pos[i].y
        ));
    }
    // Метки узлов (грабля 23: длинные имена краевых узлов обрезались
    // и налетали на соседей). Кандидаты: ниже → выше → справа → слева;
    // каждый бокс проверяется против узлов и уже размещённых меток,
    // с укором в холст; текст — с белым ореолом поверх рёбер.
    let circle_boxes: Vec<LBox> = (0..n)
        .map(|i| LBox {
            x0: pos[i].x - radii[i] - 3.0,
            y0: pos[i].y - radii[i] - 3.0,
            x1: pos[i].x + radii[i] + 3.0,
            y1: pos[i].y + radii[i] + 3.0,
        })
        .collect();
    let mut label_boxes: Vec<LBox> = Vec::new();
    for i in 0..n {
        let (size, lines) = node_label_lines(&names[i]);
        let lw = lines.iter().map(|l| text_w(l, size)).fold(0.0, f64::max);
        let nlines = lines.len();
        let (cx, cy, r) = (pos[i].x, pos[i].y, radii[i]);
        // (первая базовая линия, anchor, x текста, бокс) — детерминированный порядок.
        let mut cands: Vec<(f64, &'static str, f64, LBox)> = Vec::new();
        {
            let fy = cy + r + 13.0;
            let tx = cx.clamp(10.0 + lw / 2.0, 630.0 - lw / 2.0);
            cands.push((
                fy,
                "middle",
                tx,
                LBox {
                    x0: tx - lw / 2.0,
                    y0: fy - 8.0,
                    x1: tx + lw / 2.0,
                    y1: fy + nlines as f64 * 11.0 - 8.0,
                },
            ));
        }
        {
            let fy = cy - r - 10.0 - (nlines - 1) as f64 * 11.0;
            let tx = cx.clamp(10.0 + lw / 2.0, 630.0 - lw / 2.0);
            cands.push((
                fy,
                "middle",
                tx,
                LBox {
                    x0: tx - lw / 2.0,
                    y0: fy - 8.0,
                    x1: tx + lw / 2.0,
                    y1: fy + nlines as f64 * 11.0 - 8.0,
                },
            ));
        }
        if nlines == 1 {
            let tx = (cx + r + 6.0).min(630.0 - lw);
            cands.push((
                cy + 3.5,
                "start",
                tx,
                LBox {
                    x0: tx,
                    y0: cy - 5.0,
                    x1: tx + lw,
                    y1: cy + 7.0,
                },
            ));
            let tx = (cx - r - 6.0).max(10.0);
            cands.push((
                cy + 3.5,
                "end",
                tx,
                LBox {
                    x0: tx - lw,
                    y0: cy - 5.0,
                    x1: tx,
                    y1: cy + 7.0,
                },
            ));
        }
        let mut picked: Option<(f64, &'static str, f64)> = None;
        for (fy, anchor, tx, b) in cands {
            if b.y0 < 30.0 || b.y1 > 386.0 {
                continue;
            }
            let free = label_boxes.iter().all(|q| !boxes_hit(&b, q, 2.0))
                && circle_boxes.iter().all(|q| !boxes_hit(&b, q, 0.5));
            if free {
                label_boxes.push(b);
                picked = Some((fy, anchor, tx));
                break;
            }
        }
        // Фолбэк: ниже узла, с ореолом (практически недостижимо: 4 кандидата).
        let (fy, anchor, tx) = picked.unwrap_or_else(|| {
            (
                cy + r + 13.0,
                "middle",
                cx.clamp(10.0 + lw / 2.0, 630.0 - lw / 2.0),
            )
        });
        for (li, line) in lines.iter().enumerate() {
            let y = fy + li as f64 * 11.0;
            s.push_str(&halo_text(tx, y, size, anchor, C_TEXT, line));
        }
    }
    let (block, _) = notes_svg(&notes, 60.0, W as f64, NOTES_Y);
    s.push_str(&block);
    s.push_str("</svg>");
    Ok(s)
}

// ---------------------------------------------------------------------
// viz_field: изолинии marching squares
// ---------------------------------------------------------------------

/// Сегмент изолинии — два пересечения на рёбрах клетки.
type Seg = [Pt; 2];

/// Marching squares: изолиния уровня `level` по сетке узлов z[i][j]
/// (узел (i, j) лежит в точке (x = j, y = i)). Пересечение на ребре
/// строгое (знаки «выше уровня» должны РАЗЛИЧАТЬСЯ), поэтому узел
/// ровно на уровне не даёт двойных точек; двусмысленные диагонали
/// (углы «выше» через один) разрешаются средним по клетке.
fn marching_squares(z: &[Vec<f64>], level: f64) -> Vec<Seg> {
    let (rows, cols) = (z.len(), z[0].len());
    let cross = |p: Pt, vp: f64, q: Pt, vq: f64| -> Option<Pt> {
        if (vp > level) == (vq > level) {
            return None;
        }
        let t = (level - vp) / (vq - vp);
        Some(Pt {
            x: p.x + (q.x - p.x) * t,
            y: p.y + (q.y - p.y) * t,
        })
    };
    let mut out = Vec::new();
    for i in 0..rows - 1 {
        for j in 0..cols - 1 {
            // Углы клетки: a — верх-лево, b — верх-право,
            // c — низ-право, d — низ-лево.
            let pa = Pt {
                x: j as f64,
                y: i as f64,
            };
            let pb = Pt {
                x: j as f64 + 1.0,
                y: i as f64,
            };
            let pc = Pt {
                x: j as f64 + 1.0,
                y: i as f64 + 1.0,
            };
            let pd = Pt {
                x: j as f64,
                y: i as f64 + 1.0,
            };
            let (a, b, c, d) = (z[i][j], z[i][j + 1], z[i + 1][j + 1], z[i + 1][j]);
            let top = cross(pa, a, pb, b);
            let right = cross(pb, b, pc, c);
            let bottom = cross(pd, d, pc, c);
            let left = cross(pa, a, pd, d);
            let pts: Vec<Pt> = [top, right, bottom, left].into_iter().flatten().collect();
            match pts.len() {
                2 => out.push([pts[0], pts[1]]),
                4 => {
                    // Диагональная двусмысленность: среднее решает.
                    // diag_ac = «выше» углы a и c (иначе b и d);
                    // лента через центр → топ-право + низ-лево,
                    // островки → топ-лево + право-низ.
                    let center_above = (a + b + c + d) / 4.0 > level;
                    let diag_ac = a > level && c > level;
                    let (t_, r_, b_, l_) = (
                        top.unwrap(),
                        right.unwrap(),
                        bottom.unwrap(),
                        left.unwrap(),
                    );
                    if diag_ac == center_above {
                        out.push([t_, r_]);
                        out.push([b_, l_]);
                    } else {
                        out.push([t_, l_]);
                        out.push([r_, b_]);
                    }
                }
                _ => {}
            }
        }
    }
    // Вырожденные сегменты нулевой длины (узел ровно на уровне).
    out.retain(|s| (s[0].x - s[1].x).abs() > 1e-9 || (s[0].y - s[1].y).abs() > 1e-9);
    out
}

/// Сшивка сегментов в полилинии: концы квантуются в ключ 1/1024 узла
/// сетки; замкнутые кольца заканчиваются в стартовой точке.
fn chain_segments(segs: &[Seg]) -> Vec<Vec<Pt>> {
    let key = |p: &Pt| ((p.x * 1024.0).round() as i64, (p.y * 1024.0).round() as i64);
    let mut ends: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (k, seg) in segs.iter().enumerate() {
        ends.entry(key(&seg[0])).or_default().push(k);
        ends.entry(key(&seg[1])).or_default().push(k);
    }
    let mut used = vec![false; segs.len()];
    let mut lines: Vec<Vec<Pt>> = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut line = vec![segs[start][0], segs[start][1]];
        // Рост вперёд от хвоста.
        loop {
            let tip = *line.last().unwrap();
            let next = ends
                .get(&key(&tip))
                .and_then(|c| c.iter().cloned().find(|&k| !used[k]));
            match next {
                Some(k) => {
                    used[k] = true;
                    let other = if key(&segs[k][0]) == key(&tip) {
                        segs[k][1]
                    } else {
                        segs[k][0]
                    };
                    line.push(other);
                }
                None => break,
            }
        }
        // Рост назад от головы.
        loop {
            let head = line[0];
            let next = ends
                .get(&key(&head))
                .and_then(|c| c.iter().cloned().find(|&k| !used[k]));
            match next {
                Some(k) => {
                    used[k] = true;
                    let other = if key(&segs[k][0]) == key(&head) {
                        segs[k][1]
                    } else {
                        segs[k][0]
                    };
                    line.insert(0, other);
                }
                None => break,
            }
        }
        lines.push(line);
    }
    lines
}

/// Скалярное поле: матрица узлов (Re) → изолинии marching squares
/// поверх дивергентной карты (красный ≥ центра, синий <).
/// 7 уровней строго внутри (zmin, zmax); плоское поле рисует фон без
/// изолиний. Гигиена сцены: сетка 2×2…64×64.
pub fn field_svg(m: &Matrix) -> Result<String, String> {
    if m.rows < 2 || m.cols < 2 {
        return Err(format!(
            "viz_field: нужно ≥ 2×2 узлов сетки, получено {}×{}",
            m.rows, m.cols
        ));
    }
    if m.rows > 64 || m.cols > 64 {
        return Err(format!(
            "viz_field: {}×{} — слишком крупно для сцены (до 64×64)",
            m.rows, m.cols
        ));
    }
    let z: Vec<Vec<f64>> = (0..m.rows)
        .map(|i| (0..m.cols).map(|j| m.get(i, j).re).collect())
        .collect();
    if z.iter().any(|r| r.iter().any(|v| !v.is_finite())) {
        return Err("viz_field: значения поля должны быть конечными".into());
    }
    let flat: Vec<f64> = z.iter().flat_map(|r| r.iter().cloned()).collect();
    let zmin = flat.iter().cloned().fold(f64::INFINITY, f64::min);
    let zmax = flat.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let cell = (540.0 / (m.rows - 1).max(m.cols - 1) as f64).clamp(7.0, 56.0);
    let (ox, oy) = (56.0, 40.0);
    let plot_w = (m.cols as f64 - 1.0) * cell;
    let plot_h = (m.rows as f64 - 1.0) * cell;
    let mid = (zmin + zmax) / 2.0;
    let half = ((zmax - zmin) / 2.0).max(1e-12);

    // Изолинии — до холста (грабля 23: статистика изолиний входит в
    // примечания, а примечания определяют высоту холста).
    let mut iso = String::new();
    let mut total_len = 0.0;
    let mut n_lines = 0usize;
    if zmax - zmin > 1e-12 {
        for (k, lev) in (1..=7)
            .map(|q| zmin + (zmax - zmin) * q as f64 / 8.0)
            .enumerate()
        {
            let segs = marching_squares(&z, lev);
            let lines = chain_segments(&segs);
            let color = lerp_hex("#2563eb", "#dc2626", k as f64 / 6.0);
            for line in &lines {
                let pts: Vec<String> = line
                    .iter()
                    .map(|p| format!("{:.1},{:.1}", ox + p.x * cell, oy + p.y * cell))
                    .collect();
                iso.push_str(&format!(
                    "<polyline points=\"{}\" fill=\"none\" stroke=\"{color}\" \
                     stroke-width=\"1.6\" stroke-linejoin=\"round\"/>\n",
                    pts.join(" ")
                ));
                n_lines += 1;
            }
            for seg in &segs {
                total_len +=
                    ((seg[0].x - seg[1].x).powi(2) + (seg[0].y - seg[1].y).powi(2)).sqrt();
            }
        }
    }
    let notes = vec![
        Note::new(
            format!("сетка {}×{} · z ∈ [{}, {}]", m.rows, m.cols, num(zmin), num(zmax)),
            12.0,
            C_TEXT,
        ),
        Note::new(
            if zmax - zmin > 1e-12 {
                format!(
                    "изолинии: 7 уровней · полилиний {} · суммарная длина ≈ {:.1} ед. сетки \
                     · marching squares (двусмысленные клетки — средним)",
                    n_lines, total_len
                )
            } else {
                "плоское поле: zmin = zmax — изолиний нет".to_string()
            },
            11.0,
            C_MUTED,
        ),
    ];
    let w = (ox * 2.0 + plot_w).max(460.0);
    let notes_y = oy + plot_h + 28.0;
    let h = (notes_y + notes_height(&notes, ox, w) + 8.0).ceil() as u32;

    let mut s = svg_open(w as u32, h);
    s.push_str(&title("Поле: изолинии marching squares", w as u32));
    // Фон: дивергентная карта по среднему четырёх узлов клетки.
    for i in 0..m.rows - 1 {
        for j in 0..m.cols - 1 {
            let v = (z[i][j] + z[i][j + 1] + z[i + 1][j + 1] + z[i + 1][j]) / 4.0;
            let t = ((v - mid) / half).clamp(-1.0, 1.0);
            let base = if t >= 0.0 { "#ef4444" } else { "#3b82f6" };
            s.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.2}\" height=\"{:.2}\" \
                 fill=\"{base}\" fill-opacity=\"{:.3}\" stroke=\"{}\" \
                 stroke-width=\"0.3\"/>\n",
                ox + j as f64 * cell,
                oy + i as f64 * cell,
                cell,
                cell,
                0.08 + 0.5 * t.abs(),
                C_GRID
            ));
        }
    }
    s.push_str(&iso);
    let (block, _) = notes_svg(&notes, ox, w, notes_y);
    s.push_str(&block);
    s.push_str("</svg>");
    Ok(s)
}

// ---------------------------------------------------------------------
// viz_surf: 3D-поверхность — поворот, ортоскопия, painter's-алгоритм
// ---------------------------------------------------------------------

/// Ортоскопическая проекция точки (x, y, z): поворот на азимут φ
/// вокруг вертикали, наклон камеры на высоту θ. Возвращает
/// (экран_x, экран_y, глубина) — математика plot_surface.
fn project3(
    x: f64,
    y: f64,
    z: f64,
    cf: f64,
    sf: f64,
    ct: f64,
    st: f64,
) -> (f64, f64, f64) {
    let x1 = x * cf - y * sf;
    let y1 = x * sf + y * cf;
    let sx = x1;
    let sy = z * ct - y1 * st;
    let depth = y1 * ct + z * st;
    (sx, sy, depth)
}

/// 3D-поверхность: матрица узлов — высоты z = f(x, y). Сетка
/// нормируется в куб [−1, 1]³, поворачивается на азимут −60° и
/// наклоняется на 28°, квады сортируются по глубине (painter's:
/// дальние рисуются раньше), цвет — по высоте (синий → красный).
/// Штатив осей — в ближне-нижнем углу. Гигиена: 2×2…48×48.
pub fn surf_svg(m: &Matrix) -> Result<String, String> {
    if m.rows < 2 || m.cols < 2 {
        return Err(format!(
            "viz_surf: нужно ≥ 2×2 узлов высот, получено {}×{}",
            m.rows, m.cols
        ));
    }
    if m.rows > 48 || m.cols > 48 {
        return Err(format!(
            "viz_surf: {}×{} — слишком крупно для сцены (до 48×48)",
            m.rows, m.cols
        ));
    }
    let mut zmin = f64::INFINITY;
    let mut zmax = f64::NEG_INFINITY;
    for i in 0..m.rows {
        for j in 0..m.cols {
            let v = m.get(i, j).re;
            if !v.is_finite() {
                return Err(format!("viz_surf: высота ({i},{j}) не конечна"));
            }
            zmin = zmin.min(v);
            zmax = zmax.max(v);
        }
    }
    let mid = (zmin + zmax) / 2.0;
    let half = ((zmax - zmin) / 2.0).max(1e-12);

    let nx = |j: usize| 2.0 * j as f64 / (m.cols - 1) as f64 - 1.0;
    let ny = |i: usize| 1.0 - 2.0 * i as f64 / (m.rows - 1) as f64; // строка 0 — «север»
    let nz = |v: f64| (v - mid) / half;

    const AZ: f64 = -60.0; // азимут камеры, градусы
    const EL: f64 = 28.0; // высота камеры, градусы
    let (phi, theta) = (AZ.to_radians(), EL.to_radians());
    let (cf, sf) = (phi.cos(), phi.sin());
    let (ct, st) = (theta.cos(), theta.sin());

    // Проекции узлов + штатив осей (входит в общую рамку).
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(m.rows * m.cols + 4);
    for i in 0..m.rows {
        for j in 0..m.cols {
            let (sx, sy, _) = project3(nx(j), ny(i), nz(m.get(i, j).re), cf, sf, ct, st);
            pts.push((sx, sy));
        }
    }
    // Штатив из углового базиса (как у Matplotlib: угол сетки).
    let (bx, by, bz) = (-1.05, -1.05, -1.05);
    let tripod = [
        (bx + 0.55, by, bz, "x"),
        (bx, by + 0.55, bz, "y"),
        (bx, by, bz + 0.55, "z"),
    ];
    let origin = project3(bx, by, bz, cf, sf, ct, st);
    for (tx, ty, tz, _) in tripod {
        let (sx, sy, _) = project3(tx, ty, tz, cf, sf, ct, st);
        pts.push((sx, sy));
    }

    // Рамка: вписать в канву с сохранением пропорций.
    let (mut x0, mut y0, mut x1, mut y1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for (sx, sy) in &pts {
        x0 = x0.min(*sx);
        x1 = x1.max(*sx);
        y0 = y0.min(*sy);
        y1 = y1.max(*sy);
    }
    let (cw, ch) = (552.0, 316.0);
    let scale = (cw / (x1 - x0).max(1e-9)).min(ch / (y1 - y0).max(1e-9));
    let (cxm, cym) = (320.0, 212.0);
    let to_screen = |sx: f64, sy: f64| -> (f64, f64) {
        (
            cxm + (sx - (x0 + x1) / 2.0) * scale,
            cym - (sy - (y0 + y1) / 2.0) * scale,
        )
    };

    // Квады по глубине (painter's: дальние раньше) — до холста:
    // их число входит в примечания, примечания — в высоту холста.
    let mut quads: Vec<(f64, [(f64, f64); 4], f64)> = Vec::new();
    for i in 0..m.rows - 1 {
        for j in 0..m.cols - 1 {
            let corners = [(i, j), (i, j + 1), (i + 1, j + 1), (i + 1, j)];
            let mut screen = [(0.0f64, 0.0f64); 4];
            let mut depth = 0.0;
            let mut zavg = 0.0;
            for (k, &(ci, cj)) in corners.iter().enumerate() {
                let v = m.get(ci, cj).re;
                let (sx, sy, d) = project3(nx(cj), ny(ci), nz(v), cf, sf, ct, st);
                screen[k] = to_screen(sx, sy);
                depth += d;
                zavg += nz(v);
            }
            quads.push((depth / 4.0, screen, zavg / 4.0));
        }
    }
    quads.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // Примечания — до холста (грабля 23: третья строка футера
    // «…без зависимостей» обрезалась на правом краю — полевой отчёт).
    let notes = vec![
        Note::new(
            format!(
                "сетка {}×{} · квадов {} · z ∈ [{}, {}]",
                m.rows,
                m.cols,
                quads.len(),
                num(zmin),
                num(zmax)
            ),
            12.0,
            C_TEXT,
        ),
        Note::new(
            format!(
                "камера: азимут {AZ}°, высота {EL}° (ортоскопия) · painter's-алгоритм: \
                 дальние квады раньше",
            ),
            11.0,
            C_MUTED,
        ),
        Note::new(
            "цвет — высота: синий (мин) → красный (макс) · поворот/проекция — как \
             plot_surface, без зависимостей",
            10.0,
            C_MUTED,
        ),
    ];
    const W: u32 = 640;
    const NOTES_Y: f64 = 404.0;
    let h = (NOTES_Y + notes_height(&notes, 60.0, W as f64) + 12.0).ceil() as u32;

    let mut s = svg_open(W, h);
    s.push_str(&title("Поверхность z = f(x, y): 3D-проекция", W));

    for (_, screen, zt) in &quads {
        let fill = lerp_hex("#3b82f6", "#ef4444", (zt + 1.0) / 2.0);
        let pts_str: Vec<String> = screen
            .iter()
            .map(|(x, y)| format!("{x:.1},{y:.1}"))
            .collect();
        s.push_str(&format!(
            "<polygon points=\"{}\" fill=\"{fill}\" fill-opacity=\"1\" \
             stroke=\"#334155\" stroke-opacity=\"0.35\" stroke-width=\"0.4\"/>\n",
            pts_str.join(" ")
        ));
    }

    // Штатив осей поверх сцены (грабля 23: метки «x»/«y» тонули в
    // сетке): линия + подпись за концом оси, с белым ореолом.
    let (osx, osy) = to_screen(origin.0, origin.1);
    for (tx, ty, tz, label) in tripod {
        let (sx, sy, _) = project3(tx, ty, tz, cf, sf, ct, st);
        let (px, py) = to_screen(sx, sy);
        s.push_str(&format!(
            "<line x1=\"{osx:.1}\" y1=\"{osy:.1}\" x2=\"{px:.1}\" y2=\"{py:.1}\" \
             stroke=\"{}\" stroke-width=\"1.2\"/>\n",
            C_MUTED
        ));
        let (dx, dy) = (px - osx, py - osy);
        let d = (dx * dx + dy * dy).sqrt().max(1e-9);
        let (lx, ly) = (px + dx / d * 14.0, py + dy / d * 14.0);
        s.push_str(&halo_text(lx, ly + 3.5, 11.0, "middle", C_MUTED, label));
    }

    let (block, _) = notes_svg(&notes, 60.0, W as f64, NOTES_Y);
    s.push_str(&block);
    s.push_str("</svg>");
    Ok(s)
}

// ---------------------------------------------------------------------
// viz_iso3 (Сессия-12): изоповерхность 3D-поля — marching tetrahedra
// ---------------------------------------------------------------------

/// Изоповерхность воксельного поля: стек срезов `slices[iz][iy][ix]`
/// (матрица = z-сечение), уровень `level` (по умолчанию — середина
/// диапазона поля). Треугольники извлекает `geo::marching`
/// (самокорректирующиеся таблицы, watertight), сцена — как у viz_surf:
/// нормировка в куб [−1,1]³, поворот −60°/28°, painter's-алгоритм.
/// Цвет — высота z: синий → красный.
pub fn iso3_svg(slices: &[Matrix], level: Option<f64>) -> Result<String, String> {
    use crate::geo::marching::{marching_tetrahedra, VoxelGrid};

    if slices.is_empty() {
        return Err("viz_iso3: пустой стек срезов".into());
    }
    if slices.len() > 40 || slices[0].rows > 40 || slices[0].cols > 40 {
        return Err(format!(
            "viz_iso3: {}×{}×{} — слишком крупно для сцены (до 40 по каждой оси)",
            slices[0].cols,
            slices[0].rows,
            slices.len()
        ));
    }
    let mut zslices: Vec<Vec<Vec<f64>>> = Vec::with_capacity(slices.len());
    for (k, m) in slices.iter().enumerate() {
        let mut plane: Vec<Vec<f64>> = Vec::with_capacity(m.rows);
        for i in 0..m.rows {
            let mut row = Vec::with_capacity(m.cols);
            for j in 0..m.cols {
                let v = m.get(i, j).re;
                if !v.is_finite() {
                    return Err(format!("viz_iso3: срез {k}, узел ({i},{j}) не конечен"));
                }
                row.push(v);
            }
            plane.push(row);
        }
        zslices.push(plane);
    }
    let grid = VoxelGrid::from_slices(&zslices)?;
    let (lo, hi) = grid.minmax();
    if !(lo < hi) {
        return Err("viz_iso3: поле постоянно — изоповерхность не определена".into());
    }
    let level = level.unwrap_or((lo + hi) / 2.0);

    let tris = marching_tetrahedra(&grid, level);
    if tris.is_empty() {
        return Err(format!(
            "viz_iso3: уровень {level} вне поля [{lo}, {hi}] — пусто"
        ));
    }

    // Нормировка координат в [−1, 1]³ (индексы → куб).
    let (nx, ny, nz) = (grid.nx as f64, grid.ny as f64, grid.nz as f64);
    let nrm = |v: &crate::geo::marching::Vec3| -> (f64, f64, f64) {
        (
            2.0 * v.0 / (nx - 1.0) - 1.0,
            2.0 * v.1 / (ny - 1.0) - 1.0,
            2.0 * v.2 / (nz - 1.0) - 1.0,
        )
    };

    const AZ: f64 = -60.0;
    const EL: f64 = 28.0;
    let (phi, theta) = (AZ.to_radians(), EL.to_radians());
    let (cf, sf) = (phi.cos(), phi.sin());
    let (ct, st) = (theta.cos(), theta.sin());

    // Проекции + глубина: painter's — дальние раньше.
    let mut faces: Vec<(f64, [(f64, f64); 3], f64)> = Vec::with_capacity(tris.len());
    let mut bbox = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for t in &tris {
        let mut screen = [(0.0f64, 0.0f64); 3];
        let mut depth = 0.0;
        let mut zavg = 0.0;
        for (k, v) in t.iter().enumerate() {
            let (x, y, z) = nrm(v);
            let (sx, sy, d) = project3(x, y, z, cf, sf, ct, st);
            screen[k] = (sx, sy);
            depth += d;
            zavg += z;
        }
        bbox[0] = bbox[0].min(screen[0].0).min(screen[1].0).min(screen[2].0);
        bbox[1] = bbox[1].min(screen[0].1).min(screen[1].1).min(screen[2].1);
        bbox[2] = bbox[2].max(screen[0].0).max(screen[1].0).max(screen[2].0);
        bbox[3] = bbox[3].max(screen[0].1).max(screen[1].1).max(screen[2].1);
        faces.push((depth / 3.0, screen, zavg / 3.0));
    }
    faces.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let (cw, ch) = (552.0, 316.0);
    let scale = (cw / (bbox[2] - bbox[0]).max(1e-9)).min(ch / (bbox[3] - bbox[1]).max(1e-9));
    let (cxm, cym) = (320.0, 206.0);
    let to_screen = |sx: f64, sy: f64| -> (f64, f64) {
        (
            cxm + (sx - (bbox[0] + bbox[2]) / 2.0) * scale,
            cym - (sy - (bbox[1] + bbox[3]) / 2.0) * scale,
        )
    };

    // Штатив осей в ближне-нижнем углу.
    let (bx, by, bz) = (-1.05, -1.05, -1.05);
    let tripod = [
        (bx + 0.55, by, bz, "x"),
        (bx, by + 0.55, bz, "y"),
        (bx, by, bz + 0.55, "z"),
    ];
    let origin = project3(bx, by, bz, cf, sf, ct, st);

    let notes = vec![
        Note::new(
            format!(
                "изоповерхность f = {level} · сетка {}×{}×{} · треугольников {}",
                grid.nx,
                grid.ny,
                grid.nz,
                tris.len()
            ),
            12.0,
            C_TEXT,
        ),
        Note::new(
            format!(
                "поле ∈ [{}, {}] · marching tetrahedra: 6 тетраэдров/куб, \
                 самокоррекция нормалей · watertight",
                num(lo),
                num(hi)
            ),
            10.0,
            C_MUTED,
        ),
        Note::new(
            "цвет — высота z: синий (мин) → красный (макс) · камера −60°/28°, \
             painter's-алгоритм",
            10.0,
            C_MUTED,
        ),
    ];
    const W: u32 = 640;
    const NOTES_Y: f64 = 380.0;
    let h = (NOTES_Y + notes_height(&notes, 60.0, W as f64) + 12.0).ceil() as u32;

    let mut s = svg_open(W, h);
    s.push_str(&title("Изоповерхность: marching tetrahedra", W));

    for (_, screen, zt) in &faces {
        let fill = lerp_hex("#3b82f6", "#ef4444", (zt + 1.0) / 2.0);
        let pts_str: Vec<String> = screen
            .iter()
            .map(|(x, y)| {
                let (px, py) = to_screen(*x, *y);
                format!("{px:.1},{py:.1}")
            })
            .collect();
        s.push_str(&format!(
            "<polygon points=\"{}\" fill=\"{fill}\" fill-opacity=\"0.96\" \
             stroke=\"#334155\" stroke-opacity=\"0.3\" stroke-width=\"0.3\"/>\n",
            pts_str.join(" ")
        ));
    }

    let (osx, osy) = to_screen(origin.0, origin.1);
    for (tx, ty, tz, label) in tripod {
        let (sx, sy, _) = project3(tx, ty, tz, cf, sf, ct, st);
        let (px, py) = to_screen(sx, sy);
        s.push_str(&format!(
            "<line x1=\"{osx:.1}\" y1=\"{osy:.1}\" x2=\"{px:.1}\" y2=\"{py:.1}\" \
             stroke=\"{}\" stroke-width=\"1.2\"/>\n",
            C_MUTED
        ));
        let (dx, dy) = (px - osx, py - osy);
        let d = (dx * dx + dy * dy).sqrt().max(1e-9);
        let (lx, ly) = (px + dx / d * 14.0, py + dy / d * 14.0);
        s.push_str(&halo_text(lx, ly + 3.5, 11.0, "middle", C_MUTED, label));
    }

    let (block, _) = notes_svg(&notes, 60.0, W as f64, NOTES_Y);
    s.push_str(&block);
    s.push_str("</svg>");
    Ok(s)
}

// =====================================================================
// Сессия-14: ГЕКС-КАРТЫ — тритная спираль H3 на холсте движка
// =====================================================================
// Пайплайн PORTING_NOTES: гекс-сетка (мини-H3) → карта биомов Этерии.
// Осевые координаты → pointy-top гексы; цвет — биом; легенда —
// «■ имя ×N»; примечание — параметры мира. Детерминизм полный.

/// Гекс-карта из ячеек (q, r, биом): SVG-сцена с легендой.
/// `note` — строка-подпись (seed, радиус, число ячеек).
pub fn hex_svg(
    cells: &[(i64, i64, usize)],
    title_text: &str,
    note: &str,
) -> Result<String, String> {
    use crate::geo::eteria::BIOMES;
    if cells.is_empty() {
        return Err("viz_hex: пустой набор ячеек".into());
    }
    if cells.iter().any(|&(_, _, b)| b >= BIOMES.len()) {
        return Err("viz_hex: биом ∈ [0, 5] (океан…фредерит)".into());
    }
    const W: f64 = 640.0;
    // геометрия pointy-top: центр ячейки (q,r) → (u, v) в единицах размера
    let uv = |q: i64, r: i64| -> (f64, f64) {
        (3f64.sqrt() * (q as f64 + r as f64 / 2.0), 1.5 * r as f64)
    };
    let (mut umin, mut umax, mut vmin, mut vmax) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(q, r, _) in cells {
        let (u, v) = uv(q, r);
        umin = umin.min(u);
        umax = umax.max(u);
        vmin = vmin.min(v);
        vmax = vmax.max(v);
    }
    // размер гекса: вписываем диск + запас на сам гекс (радиус S)
    let avail_w = W - 64.0;
    let avail_h = 430.0;
    let su = avail_w / (umax - umin + 3f64.sqrt());
    let sv = avail_h / (vmax - vmin + 2.0);
    let s = su.min(sv).clamp(6.0, 30.0);
    let ox = 32.0 - umin * s + 3f64.sqrt() * s / 2.0; // центр ячейки → холст
    let oy = 34.0 - vmin * s + s;
    let px = |q: i64, r: i64| -> (f64, f64) {
        let (u, v) = uv(q, r);
        (u * s + ox, v * s + oy)
    };

    // легенда: «■ имя ×N» с переносом строк
    let counts = {
        let mut c = [0usize; 6];
        for &(_, _, b) in cells {
            c[b] += 1;
        }
        c
    };
    let items: Vec<(usize, String)> = (0..BIOMES.len())
        .filter(|&b| counts[b] > 0)
        .map(|b| (b, format!("{} ×{}", BIOMES[b].0, counts[b])))
        .collect();
    let item_w = |t: &str| text_w(t, 11.0) + 26.0;
    let mut legend_rows: Vec<Vec<(usize, String, f64)>> = vec![Vec::new()];
    let mut row_w = 30.0;
    for (b, t) in items {
        let w = item_w(&t);
        if row_w + w > W - 20.0 && !legend_rows.last().unwrap().is_empty() {
            legend_rows.push(Vec::new());
            row_w = 30.0;
        }
        legend_rows.last_mut().unwrap().push((b, t, row_w));
        row_w += w;
    }
    let legend_h = legend_rows.len() as f64 * 17.0 + 6.0;

    // высота холста: титул + карта + легенда + примечание
    let plot_h = (vmax - vmin) * s + 2.0 * s;
    let notes = vec![Note::new(note, 10.0, C_MUTED)];
    let notes_y = 34.0 + plot_h + legend_h + 18.0;
    let h = (notes_y + notes_height(&notes, 30.0, W) + 8.0).ceil() as u32;

    let mut out = svg_open(W as u32, h);
    out.push_str(&title(title_text, W as u32));
    // фон-океан: весь плот под гексами — мягкий синий
    out.push_str(&format!(
        "<rect x=\"16\" y=\"30\" width=\"{:.0}\" height=\"{:.0}\" fill=\"#dbeafe\" \
         stroke=\"{}\" stroke-width=\"0.6\"/>\n",
        W - 32.0,
        plot_h + 8.0,
        C_GRID
    ));
    // гексы
    let mut polygons = 0usize;
    for &(q, r, b) in cells {
        let (cx, cy) = px(q, r);
        let mut pts = String::new();
        for k in 0..6 {
            let a = std::f64::consts::FRAC_PI_6 + k as f64 * std::f64::consts::FRAC_PI_3;
            let (hx, hy) = (cx + s * a.cos(), cy + s * a.sin());
            pts.push_str(&format!("{:.1},{:.1} ", hx, hy));
        }
        out.push_str(&format!(
            "<polygon points=\"{}\" fill=\"{}\" stroke=\"#ffffff\" stroke-width=\"0.7\"/>\n",
            pts.trim_end(),
            BIOMES[b].1
        ));
        polygons += 1;
        // подпись координат — только на крупных гексах (R ≤ 4)
        if s >= 22.0 {
            out.push_str(&halo_text(
                cx,
                cy + 3.5,
                8.5,
                "middle",
                "#ffffff",
                &format!("{q},{r}"),
            ));
        }
    }
    // легенда
    for (ri, row) in legend_rows.iter().enumerate() {
        let y = 34.0 + plot_h + 14.0 + ri as f64 * 17.0;
        for &(b, ref t, x) in row {
            out.push_str(&format!(
                "<rect x=\"{x:.0}\" y=\"{y:.0}\" width=\"11\" height=\"11\" fill=\"{}\" \
                 stroke=\"#ffffff\"/>\n",
                BIOMES[b].1
            ));
            out.push_str(&format!(
                "<text x=\"{:.0}\" y=\"{:.1}\" font-size=\"11\" fill=\"{}\">{}</text>\n",
                x + 15.0,
                y + 9.5,
                C_TEXT,
                esc(t)
            ));
        }
    }
    let (block, _) = notes_svg(&notes, 30.0, W, notes_y);
    out.push_str(&block);
    out.push_str("</svg>");
    debug_assert!(polygons == cells.len());
    Ok(out)
}

/// TIN-рельеф (Сессия-16): SVG-карта триангуляции Делоне.
///
/// Треугольники — полигоны с цветом биома по СРЕДНЕЙ высоте (та же
/// палитра, что у гекс-карты Этерии: океан → фредерит), рёбра — светлый
/// штрих, конверты пропорций сохранены. Без высот — единый графитовый
/// тон: чистая структура сети (муха над картой без окраса).
///
/// Переплетение: цвет идёт из `eteria::BIOMES`, геометрия — из
/// `geo::delaunay::Tin` (Муха водила порядок вставки), статистика
/// Эйлера — в примечании вызывающего.
pub fn tin_svg(
    tin: &crate::geo::delaunay::Tin,
    z: Option<&[f64]>,
    title_text: &str,
    note: &str,
) -> Result<String, String> {
    use crate::geo::eteria::{biome_index, BIOMES};
    if tin.tris.is_empty() {
        return Err("viz_tin: триангуляция пуста — ≥ 3 неколлинеарных точек".into());
    }
    if let Some(zs) = z {
        if zs.len() != tin.pts.len() {
            return Err(format!(
                "viz_tin: высот {} ≠ вершин {}",
                zs.len(),
                tin.pts.len()
            ));
        }
    }
    const W: f64 = 640.0;
    // bbox → плот с сохранением пропорций, центрирование
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(x, y) in &tin.pts {
        x0 = x0.min(x as f64);
        x1 = x1.max(x as f64);
        y0 = y0.min(y as f64);
        y1 = y1.max(y as f64);
    }
    let avail_w = W - 64.0;
    let avail_h = 430.0;
    let sx = avail_w / (x1 - x0 + 1.0).max(1.0);
    let sy = avail_h / (y1 - y0 + 1.0).max(1.0);
    let s = sx.min(sy);
    let px = |x: i64| (x as f64 - x0) * s + (W - (x1 - x0) * s) / 2.0;
    let py = |y: i64| (y as f64 - y0) * s + (430.0 - (y1 - y0) * s) / 2.0 + 34.0;

    // нормализация высот → биом по средней высоте треугольника
    let (zmin, zmax) = match z {
        Some(zs) => zs.iter().cloned().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(v), b.max(v))
        }),
        None => (0.0, 0.0),
    };
    let flat = zmax <= zmin;
    let tri_t = |t: &[usize; 3]| -> f64 {
        match z {
            Some(zs) => {
                if flat {
                    0.5
                } else {
                    (t.iter().map(|&v| zs[v]).sum::<f64>() / 3.0 - zmin) / (zmax - zmin)
                }
            }
            None => 0.0,
        }
    };

    // подсчёт биомов треугольников (для легенды)
    let mut counts = [0usize; 6];
    let mut biome_of: Vec<usize> = Vec::with_capacity(tin.tris.len());
    for t in &tin.tris {
        let b = if z.is_some() {
            let b = biome_index(tri_t(t).clamp(0.0, 1.0));
            counts[b] += 1;
            b
        } else {
            usize::MAX
        };
        biome_of.push(b);
    }

    // легенда «■ имя ×N» (только с рельефом)
    let items: Vec<(usize, String)> = (0..BIOMES.len())
        .filter(|&b| counts[b] > 0)
        .map(|b| (b, format!("{} ×{}", BIOMES[b].0, counts[b])))
        .collect();
    let item_w = |t: &str| text_w(t, 11.0) + 26.0;
    let mut legend_rows: Vec<Vec<(usize, String, f64)>> = vec![Vec::new()];
    let mut row_w = 30.0;
    for (b, t) in items {
        let w = item_w(&t);
        if row_w + w > W - 20.0 && !legend_rows.last().unwrap().is_empty() {
            legend_rows.push(Vec::new());
            row_w = 30.0;
        }
        legend_rows.last_mut().unwrap().push((b, t, row_w));
        row_w += w;
    }
    let legend_h = legend_rows.len() as f64 * 17.0 + 6.0;

    let plot_h = (y1 - y0) * s + 8.0;
    let notes = vec![Note::new(note, 10.0, C_MUTED)];
    let notes_y = 34.0 + plot_h + legend_h + 18.0;
    let h = (notes_y + notes_height(&notes, 30.0, W) + 8.0).ceil() as u32;

    let mut out = svg_open(W as u32, h);
    out.push_str(&title(title_text, W as u32));
    // подложка карты — светлый плот с рамкой (не океан: это карта сети)
    out.push_str(&format!(
        "<rect x=\"16\" y=\"30\" width=\"{:.0}\" height=\"{:.0}\" fill=\"#f8fafc\" \
         stroke=\"{}\" stroke-width=\"0.6\"/>\n",
        W - 32.0,
        plot_h + 8.0,
        C_GRID
    ));
    // треугольники: цвет биома по средней высоте (или графит без высот)
    let mut polygons = 0usize;
    for (ti, t) in tin.tris.iter().enumerate() {
        let mut pts = String::new();
        for &v in t {
            let (x, y) = tin.pts[v];
            pts.push_str(&format!("{:.1},{:.1} ", px(x), py(y)));
        }
        let fill = if z.is_some() {
            BIOMES[biome_of[ti]].1
        } else {
            "#94a3b8"
        };
        out.push_str(&format!(
            "<polygon points=\"{}\" fill=\"{}\" stroke=\"#ffffff\" stroke-width=\"0.7\"/>\n",
            pts.trim_end(),
            fill
        ));
        polygons += 1;
    }
    // вершины: точки сети на читаемом масштабе (не замусоривать мелкие карты)
    if tin.pts.len() <= 400 && s > 6.0 {
        for &(x, y) in &tin.pts {
            out.push_str(&format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"1.6\" fill=\"#1e293b\"/>\n",
                px(x),
                py(y)
            ));
        }
    }
    // легенда биомов
    for (ri, row) in legend_rows.iter().enumerate() {
        let y = 34.0 + plot_h + 14.0 + ri as f64 * 17.0;
        for &(b, ref t, x) in row {
            out.push_str(&format!(
                "<rect x=\"{x:.0}\" y=\"{y:.0}\" width=\"11\" height=\"11\" fill=\"{}\" \
                 stroke=\"#ffffff\"/>\n",
                BIOMES[b].1
            ));
            out.push_str(&format!(
                "<text x=\"{:.0}\" y=\"{:.1}\" font-size=\"11\" fill=\"{}\">{}</text>\n",
                x + 15.0,
                y + 9.5,
                C_TEXT,
                esc(t)
            ));
        }
    }
    let (block, _) = notes_svg(&notes, 30.0, W, notes_y);
    out.push_str(&block);
    out.push_str("</svg>");
    debug_assert!(polygons == tin.tris.len());
    Ok(out)
}


// ─────────────── изолинии поверх TIN (Сессия-17) ───────────────

/// SVG-карта изолиний поверх рельефа TIN: биомные треугольники (как
/// `tin_svg`) + изолинии marching squares тёмными полилиниями + подписи
/// уровней с белым ореолом (грабля-23-аудит: кламп в канву, анти-
/// перекрытие ≥ 34 px между подписями, максимум 8 уровней).
///
/// `levels` — пары (уровень, цепочки изолиний в квантах решётки TIN).
pub fn isolines_svg(
    tin: &crate::geo::delaunay::Tin,
    z: &[f64],
    levels: &[(f64, Vec<crate::geo::marchsq::Isoline>)],
    title_text: &str,
    note: &str,
) -> Result<String, String> {
    use crate::geo::eteria::{biome_index, BIOMES};
    use crate::geo::marchsq::Isoline;
    if tin.tris.is_empty() {
        return Err("viz_isolines: триангуляция пуста — ≥ 3 неколлинеарных точек".into());
    }
    if z.len() != tin.pts.len() {
        return Err(format!(
            "viz_isolines: высот {} ≠ вершин {}",
            z.len(),
            tin.pts.len()
        ));
    }
    if levels.is_empty() {
        return Err("viz_isolines: уровни пусты — изолиний не будет".into());
    }
    if levels.len() > 8 {
        return Err(format!(
            "viz_isolines: уровней {} > 8 (читаемость карты)",
            levels.len()
        ));
    }
    const W: f64 = 640.0;
    // bbox → плот с сохранением пропорций (как tin_svg)
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(x, y) in &tin.pts {
        x0 = x0.min(x as f64);
        x1 = x1.max(x as f64);
        y0 = y0.min(y as f64);
        y1 = y1.max(y as f64);
    }
    let avail_w = W - 64.0;
    let avail_h = 430.0;
    let sx = avail_w / (x1 - x0 + 1.0).max(1.0);
    let sy = avail_h / (y1 - y0 + 1.0).max(1.0);
    let s = sx.min(sy);
    let px = |x: f64| (x - x0) * s + (W - (x1 - x0) * s) / 2.0;
    let py = |y: f64| (y - y0) * s + (430.0 - (y1 - y0) * s) / 2.0 + 34.0;

    let (zmin, zmax) = z.iter().cloned().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
        (a.min(v), b.max(v))
    });
    let flat = zmax <= zmin;
    let tri_t = |t: &[usize; 3]| -> f64 {
        if flat {
            0.5
        } else {
            (t.iter().map(|&v| z[v]).sum::<f64>() / 3.0 - zmin) / (zmax - zmin)
        }
    };

    // биомы треугольников (для окраса и легенды)
    let mut counts = [0usize; 6];
    let mut biome_of: Vec<usize> = Vec::with_capacity(tin.tris.len());
    for t in &tin.tris {
        let b = biome_index(tri_t(t).clamp(0.0, 1.0));
        counts[b] += 1;
        biome_of.push(b);
    }

    // легенда биомов «■ имя ×N»
    let items: Vec<(usize, String)> = (0..BIOMES.len())
        .filter(|&b| counts[b] > 0)
        .map(|b| (b, format!("{} ×{}", BIOMES[b].0, counts[b])))
        .collect();
    let item_w = |t: &str| text_w(t, 11.0) + 26.0;
    let mut legend_rows: Vec<Vec<(usize, String, f64)>> = vec![Vec::new()];
    let mut row_w = 30.0;
    for (b, t) in items {
        let w = item_w(&t);
        if row_w + w > W - 20.0 && !legend_rows.last().unwrap().is_empty() {
            legend_rows.push(Vec::new());
            row_w = 30.0;
        }
        legend_rows.last_mut().unwrap().push((b, t, row_w));
        row_w += w;
    }
    let legend_h = legend_rows.len() as f64 * 17.0 + 6.0;

    let plot_h = (y1 - y0) * s + 8.0;
    let notes = vec![Note::new(note, 10.0, C_MUTED)];
    let notes_y = 34.0 + plot_h + legend_h + 18.0;
    let h = (notes_y + notes_height(&notes, 30.0, W) + 8.0).ceil() as u32;

    let mut out = svg_open(W as u32, h);
    out.push_str(&title(title_text, W as u32));
    out.push_str(&format!(
        "<rect x=\"16\" y=\"30\" width=\"{:.0}\" height=\"{:.0}\" fill=\"#f8fafc\" \
         stroke=\"{}\" stroke-width=\"0.6\"/>\n",
        W - 32.0,
        plot_h + 8.0,
        C_GRID
    ));
    // рельеф: биомные треугольники
    let mut polygons = 0usize;
    for (ti, t) in tin.tris.iter().enumerate() {
        let mut pts = String::new();
        for &v in t {
            let (x, y) = tin.pts[v];
            pts.push_str(&format!("{:.1},{:.1} ", px(x as f64), py(y as f64)));
        }
        out.push_str(&format!(
            "<polygon points=\"{}\" fill=\"{}\" stroke=\"#ffffff\" stroke-width=\"0.7\"/>\n",
            pts.trim_end(),
            BIOMES[biome_of[ti]].1
        ));
        polygons += 1;
    }
    // изолинии: тёмные полилинии поверх рельефа
    let mut lines_drawn = 0usize;
    let mut pts_drawn = 0usize;
    for (_, chains) in levels {
        for l in chains {
            if l.pts.len() < 2 {
                continue;
            }
            let mut pts = String::new();
            for &(x, y) in &l.pts {
                pts.push_str(&format!("{:.1},{:.1} ", px(x), py(y)));
            }
            let closed = l.pts.first() == l.pts.last();
            let tag = if closed { "polygon" } else { "polyline" };
            // замкнутые — polygon (без дубля последней точки)
            let pts = if closed {
                l.pts[..l.pts.len() - 1]
                    .iter()
                    .map(|&(x, y)| format!("{:.1},{:.1} ", px(x), py(y)))
                    .collect::<String>()
            } else {
                pts
            };
            out.push_str(&format!(
                "<{tag} points=\"{}\" fill=\"none\" stroke=\"#0f172a\" \
                 stroke-width=\"1.3\" opacity=\"0.92\" stroke-linejoin=\"round\" stroke-linecap=\"round\"/>\n",
                pts.trim_end()
            ));
            lines_drawn += 1;
            pts_drawn += l.pts.len();
        }
    }
    debug_assert!(lines_drawn > 0 || polygons > 0);
    // подписи уровней: самая длинная цепь каждого уровня, середина,
    // ореол, кламп в канву, анти-перекрытие (грабля-23)
    let mut placed: Vec<(f64, f64)> = Vec::new();
    let plot_bottom = 34.0 + plot_h;
    let mut labels = 0usize;
    for (level, chains) in levels {
        if chains.is_empty() {
            continue;
        }
        let longest: &Isoline = chains
            .iter()
            .max_by_key(|l| l.pts.len())
            .unwrap();
        let mid = longest.pts[longest.pts.len() / 2];
        let (mut lx, mut ly) = (px(mid.0), py(mid.1) - 4.0);
        lx = lx.clamp(40.0, W - 40.0);
        ly = ly.clamp(44.0, plot_bottom - 6.0);
        if placed.iter().any(|&(ox, oy)| {
            ((ox - lx) * (ox - lx) + (oy - ly) * (oy - ly)).sqrt() < 34.0
        }) {
            continue; // перекрытие — уровень останется без подписи (честно)
        }
        placed.push((lx, ly));
        out.push_str(&halo_text(lx, ly, 10.0, "middle", "#0f172a", &num(*level)));
        labels += 1;
    }
    // легенда биомов
    for (ri, row) in legend_rows.iter().enumerate() {
        let y = 34.0 + plot_h + 14.0 + ri as f64 * 17.0;
        for &(b, ref t, x) in row {
            out.push_str(&format!(
                "<rect x=\"{x:.0}\" y=\"{y:.0}\" width=\"11\" height=\"11\" fill=\"{}\" \
                 stroke=\"#ffffff\"/>\n",
                BIOMES[b].1
            ));
            out.push_str(&format!(
                "<text x=\"{:.0}\" y=\"{:.1}\" font-size=\"11\" fill=\"{}\">{}</text>\n",
                x + 15.0,
                y + 9.5,
                C_TEXT,
                esc(t)
            ));
        }
    }
    let (block, _) = notes_svg(&notes, 30.0, W, notes_y);
    out.push_str(&block);
    out.push_str("</svg>");
    debug_assert!(polygons == tin.tris.len());
    debug_assert!(labels <= levels.len());
    let _ = pts_drawn;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::solve::Complex;

    fn starts_and_ends(s: &str) -> bool {
        s.starts_with("<svg") && s.ends_with("</svg>")
    }

    #[test]
    fn xml_escape_basics() {
        assert_eq!(esc("&<>\""), "&amp;&lt;&gt;&quot;");
        assert_eq!(esc("Ацин d=3"), "Ацин d=3");
    }

    #[test]
    fn num_formats_human_digits() {
        assert_eq!(num(0.0), "0");
        assert_eq!(num(2.0), "2");
        assert_eq!(num(1.7), "1.7");
        assert_eq!(num(1.616255e-35), "1.616e-35");
        assert_eq!(num(2.9148542161522637), "2.914854");
    }

    #[test]
    fn bell_svg_wellformed_and_anchored() {
        let s = bell_svg(BELL_ACIN);
        assert!(starts_and_ends(&s));
        assert!(s.contains("xmlns"));
        // Позиции вертикалей: x = 60 + (v-1.5)·550/1.7.
        let x_ts = 60.0 + (BELL_TSIRELSON - 1.5) * 550.0 / 1.7;
        let x_ac = 60.0 + (BELL_ACIN - 1.5) * 550.0 / 1.7;
        assert!(s.contains(&format!("{x_ts:.1}")));
        assert!(s.contains(&format!("{x_ac:.1}")));
        assert!(s.contains("Цирельсон"));
        assert!(s.contains("Ацин"));
    }

    #[test]
    fn bell_verdict_ranking_and_noise() {
        let (l1, l2) = bell_verdict(BELL_ACIN);
        assert!(l1.contains("над классикой"));
        assert!(l1.contains("над Цирельсоном"));
        assert!(l2.contains("0.6861")); // η* = 2/2.914854
        assert!(l2.contains("кутриты"));
        let (c1, _) = bell_verdict(1.87);
        assert!(c1.contains("внутри классического"));
    }

    #[test]
    fn bell_classical_zone() {
        let s = bell_svg(1.87);
        assert!(s.contains("классика ≤ 2.0"));
        assert!(s.contains("I = 1.87"));
    }

    #[test]
    fn scale_planck_anchor() {
        let s = scale_svg(1.616255e-35).unwrap();
        assert!(starts_and_ends(&s));
        assert!(s.contains("Планков"));
        assert!(s.contains("1.616e-35"));
        let (_, l2) = scale_verdict(1.616255e-35);
        assert!(l2.contains("меньше человека"));
        assert!(l2.contains("1.052e35")); // 1.7/1.616e-35
    }

    #[test]
    fn scale_human_is_reference() {
        let (_, l2) = scale_verdict(1.7);
        assert!(l2.contains("человеческий масштаб"));
        let s = scale_svg(1.7).unwrap();
        assert!(s.contains("1.7 м"));
    }

    #[test]
    fn scale_earth_between_anchors() {
        let (l1, l2) = scale_verdict(6.371e6);
        assert!(l1.contains("Эверест") && l1.contains("Земля"));
        assert!(l2.contains("больше человека"));
        // Отрицательные и нулевые значения отклоняются.
        assert!(scale_svg(-1.0).is_err());
        assert!(scale_svg(0.0).is_err());
    }

    #[test]
    fn prob_born_bars() {
        let s = prob_svg(&[0.8083, 0.0796, 0.1121]).unwrap();
        assert!(starts_and_ends(&s));
        assert!(s.contains("80.83%"));
        assert!(s.contains("Σ = 1"));
        assert!(s.contains("p0"));
        assert!(prob_svg(&[-0.5, 1.5]).is_err());
        assert!(prob_svg(&[]).is_err());
    }

    #[test]
    fn matrix_heatmap_identity() {
        let m = Matrix::identity(3);
        let s = matrix_svg(&m).unwrap();
        assert!(starts_and_ends(&s));
        assert!(s.contains("Матрица 3×3"));
        assert!(s.contains("Re(след) = 3"));
        assert!(s.contains("|max| = 1"));
        // Слишком крупная сцена отклоняется (гигиена размера).
        let big = Matrix::zeros(100, 100);
        assert!(matrix_svg(&big).is_err());
    }

    #[test]
    fn bars_spectrum() {
        let s = bars_svg(&[1.0, -2.0, 3.0]).unwrap();
        assert!(starts_and_ends(&s));
        assert!(s.contains("n = 3"));
        assert!(s.contains("min = -2"));
        assert!(s.contains("max = 3"));
        assert!(bars_svg(&[]).is_err());
    }

    #[test]
    fn auto_routes_by_type() {
        // Матрица → теплокарта (идентичность содержимого).
        let m = Matrix::identity(3);
        assert_eq!(auto_svg(&Value::Matrix(m.clone())).unwrap(), matrix_svg(&m).unwrap());
        // Список вероятностей → Born-бары.
        let probs = Value::List(vec![Value::Scalar(0.5), Value::Scalar(0.5)]);
        assert!(auto_svg(&probs).unwrap().contains("Σ = 1"));
        // Прочий список → спектр.
        let spec = Value::List(vec![Value::Scalar(1.0), Value::Scalar(-2.0)]);
        assert!(auto_svg(&spec).unwrap().contains("Спектр"));
        // Скаляр в окне CGLMP → радар Белла.
        assert!(auto_svg(&Value::Scalar(2.872934)).unwrap().contains("Цирельсон"));
        // Крошечный скаляр → шкала Вселенной.
        assert!(auto_svg(&Value::Scalar(1.616255e-35)).unwrap().contains("Планков"));
        // Комплексное → бары Re/Im.
        assert!(auto_svg(&Value::Complex(Complex::new(3.0, -4.0)))
            .unwrap()
            .contains("Re и Im"));
        // Строка отклоняется.
        assert!(auto_svg(&Value::Str("x".into())).is_err());
    }

    #[test]
    fn quantity_length_converted_to_meters() {
        use super::super::units;
        // 1 км = 1000 м → между «Эверест» и «Земля»? Нет: log10(1000)=3
        // — между «человек» и «Эверест».
        let km = units::by_name("km").expect("единица km есть");
        let s = auto_svg(&Value::Quantity(1.0, km)).unwrap();
        assert!(s.contains("1000 м"));
        let (l1, _) = scale_verdict(1000.0);
        assert!(l1.contains("человек") && l1.contains("Эверест"));
    }

    #[test]
    fn matrix_vector_routes_to_prob_or_bars() {
        // Грабля 16: [a, b, c] в языке — матрица-строка, не List.
        let probs = Matrix::from_rows(&[vec![0.8083, 0.0796, 0.1121]]).unwrap();
        let s = auto_svg(&Value::Matrix(probs)).unwrap();
        assert!(s.contains("Σ = 1"));
        let spec = Matrix::from_rows(&[vec![1.0, -2.0, 3.0]]).unwrap();
        assert!(auto_svg(&Value::Matrix(spec)).unwrap().contains("Спектр"));
        // Истинная 2×2-матрица остаётся теплокартой.
        let m22 = Matrix::from_rows(&[vec![1.0, 0.0], vec![0.0, -1.0]]).unwrap();
        assert!(auto_svg(&Value::Matrix(m22)).unwrap().contains("Матрица 2×2"));
    }

    // ===== Сессия-9: суверенный рендер (граф / поле / поверхность) =====

    #[test]
    fn graph_k4_deterministic_and_stats() {
        let m = Matrix::from_rows(&[
            vec![0.0, 1.0, 1.0, 1.0],
            vec![1.0, 0.0, 1.0, 1.0],
            vec![1.0, 1.0, 0.0, 1.0],
            vec![1.0, 1.0, 1.0, 0.0],
        ])
        .unwrap();
        let s1 = graph_svg(&m, None).unwrap();
        let s2 = graph_svg(&m, None).unwrap();
        assert_eq!(s1, s2, "укладка обязана быть бит-в-бит детерминированной");
        assert!(starts_and_ends(&s1));
        assert!(s1.contains("N = 4"));
        assert!(s1.contains("рёбер = 6"));
        assert!(s1.contains("компонент = 1"));
        assert!(s1.contains("плотность = 100"));
        assert!(s1.contains("Fruchterman"));
        assert!(s1.contains("v0") && s1.contains("v3"));
        // Все узлы внутри рамки укладки.
        assert!(s1.contains("золотая спираль"));
    }

    #[test]
    fn graph_two_islands_and_repulsion() {
        let m = Matrix::from_rows(&[
            vec![0.0, 1.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 1.0],
            vec![0.0, 0.0, 1.0, 0.0],
        ])
        .unwrap();
        let s = graph_svg(&m, None).unwrap();
        assert!(s.contains("компонент = 2"));
        assert!(s.contains("рёбер = 2"));
        // Изолированная пара: чистое отталкивание разводит узлы по рамке.
        let pos = fruchterman_reingold(2, &[], 76.0, 56.0, 488.0, 296.0);
        let d = ((pos[0].x - pos[1].x).powi(2) + (pos[0].y - pos[1].y).powi(2)).sqrt();
        assert!(d > 250.0, "изолированные узлы должны разойтись, d = {d}");
    }

    #[test]
    fn graph_labels_and_errors() {
        let m = Matrix::from_rows(&[vec![0.0, 2.0], vec![2.0, 0.0]]).unwrap();
        let s = graph_svg(&m, Some(&["альфа".to_string(), "бета".to_string()])).unwrap();
        assert!(s.contains("альфа") && s.contains("бета"));
        assert!(s.contains("рёбер = 1"));
        // Не квадрат, не та длина меток, слишком крупно, NaN — ошибки.
        let rect = Matrix::from_rows(&[vec![0.0, 1.0]]).unwrap();
        assert!(graph_svg(&rect, None).is_err());
        assert!(graph_svg(&m, Some(&["одна".to_string()])).is_err());
        assert!(graph_svg(&Matrix::zeros(65, 65), None).is_err());
        let nan_m = Matrix::from_rows(&[vec![f64::NAN, 0.0], vec![0.0, 0.0]]).unwrap();
        assert!(graph_svg(&nan_m, None).is_err());
        // Асимметричная смежность симметризуется: одно ребро, без ошибки.
        let dir = Matrix::from_rows(&[vec![0.0, 1.0], vec![0.0, 0.0]]).unwrap();
        assert!(graph_svg(&dir, None).unwrap().contains("рёбер = 1"));
        // Петли считаются, но не рисуются.
        let looped = Matrix::from_rows(&[vec![5.0, 1.0], vec![1.0, 0.0]]).unwrap();
        assert!(graph_svg(&looped, None).unwrap().contains("петель 1"));
    }

    #[test]
    fn marching_squares_plane_exact() {
        // z = x: узел (i, j) → z = j. Сетка 3×4, уровень 1.5:
        // ровно 2 вертикальных сегмента на x = 1.5.
        let z: Vec<Vec<f64>> = (0..3)
            .map(|_| (0..4).map(|j| j as f64).collect())
            .collect();
        let segs = marching_squares(&z, 1.5);
        assert_eq!(segs.len(), 2, "по сегменту на строку клеток");
        for seg in &segs {
            assert!((seg[0].x - 1.5).abs() < 1e-9);
            assert!((seg[1].x - 1.5).abs() < 1e-9);
            assert!(seg[0].y.fract().abs() < 1e-9 && seg[1].y.fract().abs() < 1e-9);
        }
        // Сшивка: одна полилиния x = 1.5, y = 0 → 2.
        let lines = chain_segments(&segs);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].len(), 3);
        let ys: Vec<f64> = lines[0].iter().map(|p| p.y).collect();
        assert_eq!(ys, vec![0.0, 1.0, 2.0]);
    }

    #[test]
    fn marching_squares_saddle_ambiguity() {
        // Диагонально двусмысленное поле: центр каждой клетки выше
        // уровня 0.5 → «лента» соединяет диагональ, контуры —
        // 4 ОТКРЫТЫЕ дуги вокруг граничных «нулевых» узлов.
        let z = vec![
            vec![1.0, 0.0, 1.0],
            vec![0.0, 2.0, 0.0],
            vec![1.0, 0.0, 1.0],
        ];
        let segs = marching_squares(&z, 0.5);
        assert_eq!(segs.len(), 8, "4 клетки × 2 сегмента (двусмысленность)");
        let lines = chain_segments(&segs);
        assert_eq!(lines.len(), 4, "по дуге на каждый «нулевой» узел");
        for line in &lines {
            assert_eq!(line.len(), 3);
            assert_ne!(line[0], *line.last().unwrap(), "дуги открытые");
        }
    }

    #[test]
    fn marching_squares_closed_ring_radial() {
        // Радиальное поле: уровень 1.2 вырезает ОДНО замкнутое кольцо
        // вокруг блока 2×2 внутренних узлов (0.707 < 1.2 < 1.581).
        let z: Vec<Vec<f64>> = (0..4)
            .map(|i| {
                (0..4)
                    .map(|j| {
                        ((i as f64 - 1.5).powi(2) + (j as f64 - 1.5).powi(2)).sqrt()
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let segs = marching_squares(&z, 1.2);
        assert_eq!(segs.len(), 8);
        let lines = chain_segments(&segs);
        assert_eq!(lines.len(), 1, "одно замкнутое кольцо");
        assert_eq!(lines[0].len(), 9); // 8 точек + замыкание
        assert_eq!(lines[0][0], *lines[0].last().unwrap());
    }

    #[test]
    fn field_svg_interference_scene() {
        // Двущелевая картина Born: cos²(πx/3)·exp(−y²/2), сетка 13×17.
        let mut rows = Vec::new();
        for i in 0..13 {
            let y = i as f64 / 12.0 * 2.0 - 1.0;
            rows.push(
                (0..17)
                    .map(|j| {
                        let x = j as f64 / 16.0 * 4.0 - 2.0;
                        (std::f64::consts::PI * x / 3.0).cos().powi(2)
                            * (-y * y / 2.0).exp()
                    })
                    .collect::<Vec<_>>(),
            );
        }
        let m = Matrix::from_rows(&rows).unwrap();
        let s = field_svg(&m).unwrap();
        assert!(starts_and_ends(&s));
        assert!(s.contains("изолинии: 7 уровней"));
        assert!(s.contains("сетка 13×17"));
        assert!(s.matches("<polyline").count() > 0);
        // Ошибки гигиены: мелко, крупно, NaN.
        assert!(field_svg(&Matrix::from_rows(&[vec![1.0, 2.0]]).unwrap()).is_err());
        assert!(field_svg(&Matrix::zeros(65, 65)).is_err());
        let nan = Matrix::from_rows(&[vec![0.0, f64::NAN], vec![1.0, 2.0]]).unwrap();
        assert!(field_svg(&nan).is_err());
    }

    #[test]
    fn field_flat_degenerate() {
        let m = Matrix::from_rows(&[vec![1.0, 1.0], vec![1.0, 1.0]]).unwrap();
        let s = field_svg(&m).unwrap();
        assert!(starts_and_ends(&s));
        assert!(s.contains("плоское поле"));
        assert_eq!(s.matches("<polyline").count(), 0);
    }

    #[test]
    fn graph_dense_no_node_overlap() {
        // Грабля 20: почти полный граф (14 рёбер из 15) — FR-равновесие
        // без релаксации слипает узлы; после неё центры дальше
        // суммы радиусов.
        fn circles_of(s: &str) -> Vec<(f64, f64, f64)> {
            s.split("<circle")
                .skip(1)
                .map(|frag| {
                    let get = |key: &str| -> f64 {
                        let k = frag.find(key).unwrap() + key.len();
                        let rest = &frag[k..];
                        let end = rest.find('"').unwrap();
                        rest[..end].parse().unwrap()
                    };
                    (get("cx=\""), get("cy=\""), get("r=\""))
                })
                .collect()
        }
        let mut a = vec![vec![0.0; 6]; 6];
        for i in 0..6 {
            for j in 0..6 {
                // 14 рёбер из 15: нет связи 0↔5 в ОБЕ стороны
                // (симметризация иначе её восстановит).
                if i != j && !(i == 0 && j == 5) && !(i == 5 && j == 0) {
                    a[i][j] = ((i + 2) * (j + 3)) as f64;
                }
            }
        }
        let m = Matrix::from_rows(&a).unwrap();
        let s = graph_svg(&m, None).unwrap();
        assert!(s.contains("рёбер = 14"));
        let circles = circles_of(&s);
        assert_eq!(circles.len(), 6);
        for i in 0..circles.len() {
            for j in (i + 1)..circles.len() {
                let (dx, dy) = (circles[i].0 - circles[j].0, circles[i].1 - circles[j].1);
                let d = (dx * dx + dy * dy).sqrt();
                assert!(
                    d + 0.2 >= circles[i].2 + circles[j].2,
                    "узлы {i} и {j} слиплись: d = {d:.1} < {}",
                    circles[i].2 + circles[j].2
                );
            }
        }
    }

    #[test]
    fn surf_svg_gaussian_deterministic() {
        // Гауссов бугор exp(−3r²), сетка 11×13 → 120 квадов.
        let mut rows = Vec::new();
        for i in 0..11 {
            let y = i as f64 / 10.0 * 2.0 - 1.0;
            rows.push(
                (0..13)
                    .map(|j| {
                        let x = j as f64 / 12.0 * 2.0 - 1.0;
                        (-3.0 * (x * x + y * y)).exp()
                    })
                    .collect::<Vec<_>>(),
            );
        }
        let m = Matrix::from_rows(&rows).unwrap();
        let s1 = surf_svg(&m).unwrap();
        let s2 = surf_svg(&m).unwrap();
        assert_eq!(s1, s2, "проекция детерминирована");
        assert!(starts_and_ends(&s1));
        assert_eq!(s1.matches("<polygon").count(), 10 * 12); // (R−1)·(C−1)
        assert!(s1.contains("квадов 120"));
        assert!(s1.contains("азимут -60"));
        assert!(s1.contains("painter"));
        assert!(s1.contains(">x<") && s1.contains(">y<") && s1.contains(">z<"));
        // Гигиена сцены.
        assert!(surf_svg(&Matrix::from_rows(&[vec![1.0, 2.0]]).unwrap()).is_err());
        assert!(surf_svg(&Matrix::zeros(49, 49)).is_err());
        let nan = Matrix::from_rows(&[vec![0.0, f64::NAN], vec![1.0, 2.0]]).unwrap();
        assert!(surf_svg(&nan).is_err());
    }

    // ===== Сессия-10: типографика (грабля 23) =====

    #[test]
    fn typography_wrap_basics() {
        let lines = wrap_text("якоря: Планк → протон → атом", 9.0, 60.0);
        assert!(lines.len() >= 2);
        for l in &lines {
            assert!(text_w(l, 9.0) <= 60.0, "строка шире лимита: «{l}»");
        }
        // Слово-гигант режется по символам.
        let hard = wrap_text("оченьдлинноеслово", 9.0, 40.0);
        assert!(hard.len() >= 3);
        for l in &hard {
            assert!(text_w(l, 9.0) <= 40.0);
        }
        // Пустая строка — одна пустая строка, не ноль.
        assert_eq!(wrap_text("", 9.0, 60.0).len(), 1);
    }

    #[test]
    fn typography_note_shrink_before_wrap() {
        // 79 символов при 12 pt — 588 px: в 572 влезает только мельче,
        // и маркеры остаются одной строкой.
        let (size, lines) = note_layout(
            "N = 6 · рёбер = 11 · средняя степень = 3.67 · плотность = 73.3% · компонент = 1",
            12.0,
            572.0,
        );
        assert_eq!(lines.len(), 1, "маркеры обязаны оставаться одной строкой");
        assert!(size < 12.0 && size >= 8.0);
        // Совсем длинная строка — перенос при минимальном кегле.
        let (_, lines2) = note_layout(&"слово ".repeat(40), 12.0, 200.0);
        assert!(lines2.len() > 1);
    }

    #[test]
    fn audit_bounds_parser_basics() {
        // Известное переполнение ловится.
        let bad = "<svg xmlns=\"x\" viewBox=\"0 0 100 50\">\n<text x=\"90\" y=\"30\" \
                   font-size=\"10\">длинный текст</text>\n</svg>";
        assert_eq!(audit_text_bounds(bad).len(), 1);
        // Якорь «middle» учитывается.
        let ok = "<svg viewBox=\"0 0 300 50\">\n<text x=\"150\" y=\"30\" font-size=\"10\" \
                  text-anchor=\"middle\">ок</text>\n</svg>";
        assert!(audit_text_bounds(ok).is_empty());
        // Повёрнутый текст — по повёрнутому охвату.
        let rot = "<svg viewBox=\"0 0 100 200\">\n<text x=\"50\" y=\"150\" font-size=\"10\" \
                   transform=\"rotate(-90 50 150)\">вертикально</text>\n</svg>";
        assert!(audit_text_bounds(rot).is_empty());
        // Повёрнутый за холст — ловится.
        let rot_bad = "<svg viewBox=\"0 0 100 100\">\n<text x=\"50\" y=\"90\" font-size=\"10\" \
                       transform=\"rotate(-90 50 90)\">вертикально длинно</text>\n</svg>";
        assert_eq!(audit_text_bounds(rot_bad).len(), 1);
    }

    #[test]
    fn scale_anchor_placer_no_collisions() {
        let items: Vec<(f64, String)> = SCALE_ANCHORS
            .iter()
            .map(|(n, m)| (70.0 + (m.log10() + 35.0) * 550.0 / 62.0, (*n).to_string()))
            .collect();
        let placed = place_axis_labels(
            &items,
            Some((80.0, "1.616e-35 м".into())),
            640.0,
            8.0,
            [96.0, 114.0, 192.0, 210.0, 66.0],
            9.0,
        );
        assert_eq!(placed.len(), items.len() + 1, "все якоря + маркер значения");
        for p in &placed {
            assert!(
                p.x >= 7.5 && p.x + p.w <= 632.5,
                "«{}» выходит за холст: [{:.1}…{:.1}]",
                p.text,
                p.x,
                p.x + p.w
            );
        }
        for i in 0..placed.len() {
            for j in (i + 1)..placed.len() {
                let (a, b) = (&placed[i], &placed[j]);
                if a.row == b.row {
                    assert!(
                        a.x + a.w + 7.0 <= b.x || b.x + b.w + 7.0 <= a.x,
                        "наложение в ряду {}: «{}» и «{}»",
                        a.row,
                        a.text,
                        b.text
                    );
                }
            }
        }
    }

    #[test]
    fn axis_placer_extreme_crowding() {
        // 20 подписей с шагом 12 px: все размещаются, в холсте, без наложений.
        let items: Vec<(f64, String)> = (0..20)
            .map(|k| (100.0 + k as f64 * 12.0, format!("якорь-{k:02}")))
            .collect();
        let placed = place_axis_labels(
            &items,
            None,
            640.0,
            8.0,
            [96.0, 114.0, 192.0, 210.0, 66.0],
            9.0,
        );
        assert_eq!(placed.len(), 20);
        for p in &placed {
            assert!(p.x >= 7.5 && p.x + p.w <= 632.5, "«{}» за холстом", p.text);
        }
        for i in 0..placed.len() {
            for j in (i + 1)..placed.len() {
                let (a, b) = (&placed[i], &placed[j]);
                if a.row == b.row {
                    assert!(
                        a.x + a.w + 7.0 <= b.x || b.x + b.w + 7.0 <= a.x,
                        "наложение в ряду {}: «{}» и «{}»",
                        a.row,
                        a.text,
                        b.text
                    );
                }
            }
        }
    }

    #[test]
    fn matrix_cell_labels_fit_cell() {
        // Подписи клеток с длинными числами вписаны в клетку.
        let dense = Matrix::from_rows(
            &(0..8)
                .map(|i| {
                    (0..8)
                        .map(|j| (i as f64 * 8.0 + j as f64) * 0.12345 - 4.0)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let s = matrix_svg(&dense).unwrap();
        for i in 0..8 {
            for j in 0..8 {
                let (txt, size) = cell_label(dense.get(i, j).re, 46.0);
                assert!(text_w(&txt, size) <= 41.0, "«{txt}» шире клетки");
            }
        }
        assert!(s.contains("Матрица 8×8"));
    }

    #[test]
    fn all_scenes_labels_fit_canvas() {
        // Регрессионная сеть грабли 23: на ЛЮБЫХ расчётах ни одна
        // надпись не выходит за холст (включая повёрнутые и вертикальные).
        let dense8 = Matrix::from_rows(
            &(0..8)
                .map(|i| {
                    (0..8)
                        .map(|j| (i as f64 * 8.0 + j as f64) * 0.12345 - 4.0)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let k4 = Matrix::from_rows(&[
            vec![0.0, 1.0, 1.0, 1.0],
            vec![1.0, 0.0, 1.0, 1.0],
            vec![1.0, 1.0, 0.0, 1.0],
            vec![1.0, 1.0, 1.0, 0.0],
        ])
        .unwrap();
        let mut slit = Vec::new();
        for i in 0..13 {
            let y = i as f64 / 12.0 * 2.0 - 1.0;
            slit.push(
                (0..17)
                    .map(|j| {
                        let x = j as f64 / 16.0 * 4.0 - 2.0;
                        (std::f64::consts::PI * x / 3.0).cos().powi(2) * (-y * y / 2.0).exp()
                    })
                    .collect::<Vec<_>>(),
            );
        }
        let slit_m = Matrix::from_rows(&slit).unwrap();
        let mut sombrero = Vec::new();
        for i in 0..11 {
            let y = i as f64 / 10.0 * 2.0 - 1.0;
            sombrero.push(
                (0..13)
                    .map(|j| {
                        let x = j as f64 / 12.0 * 2.0 - 1.0;
                        (-3.0 * (x * x + y * y)).exp()
                    })
                    .collect::<Vec<_>>(),
            );
        }
        let sombrero_m = Matrix::from_rows(&sombrero).unwrap();
        let long_labels: Vec<String> = vec![
            "экстремально длинное имя узла графа".into(),
            "б".into(),
            "среднее имя".into(),
            "д".into(),
        ];
        let scenes: Vec<String> = vec![
            bell_svg(BELL_ACIN),
            bell_svg(1.87),
            bell_svg(3.2),
            bell_svg(2.0),
            scale_svg(1.616255e-35).unwrap(),
            scale_svg(8.8e26).unwrap(),
            scale_svg(1.7).unwrap(),
            scale_svg(1e-40).unwrap(), // мельче Планка — укор маркера значения
            prob_svg(&[0.8083, 0.0796, 0.1121]).unwrap(),
            prob_svg(&(0..40).map(|_| 1.0 / 40.0).collect::<Vec<_>>()).unwrap(),
            prob_svg(&[1.0]).unwrap(),
            bars_svg(&[1.0, -2.0, 3.0]).unwrap(),
            bars_svg(&(0..40).map(|k| (k as f64 - 19.0) * 1234.5).collect::<Vec<_>>()).unwrap(),
            bars_svg(&[-5000.0, 0.25, 123456.0]).unwrap(),
            bars_svg_titled(&[3.0, -4.0], "Комплексное число: Re и Im").unwrap(),
            matrix_svg(&Matrix::identity(3)).unwrap(),
            matrix_svg(&dense8).unwrap(),
            graph_svg(&k4, None).unwrap(),
            graph_svg(&k4, Some(&long_labels)).unwrap(),
            field_svg(&slit_m).unwrap(),
            field_svg(&Matrix::from_rows(&[vec![1.0, 1.0], vec![1.0, 1.0]]).unwrap()).unwrap(),
            surf_svg(&sombrero_m).unwrap(),
        ];
        for (k, svg) in scenes.iter().enumerate() {
            let bad = audit_text_bounds(svg);
            assert!(bad.is_empty(), "сцена {k}: надписи за холстом (грабля 23): {bad:?}");
        }
    }

    #[test]
    fn hex_svg_eteria_map() {
        // карта Этерии R8: 217 гексов, все полигоны, легенда, подписи в холсте
        let cells = crate::geo::eteria::hex_cells(7, 8).unwrap();
        let triples: Vec<(i64, i64, usize)> =
            cells.iter().map(|c| (c.q, c.r, c.biome)).collect();
        let svg = hex_svg(
            &triples,
            "Этерия · гекс-карта биомов",
            "seed 7 · R8 · 217 ячеек · трит-спираль 6 тритов",
        )
        .unwrap();
        assert!(starts_and_ends(&svg));
        assert_eq!(svg.matches("<polygon").count(), 217);
        assert!(svg.contains("океан ×"), "легенда: {svg}");
        assert!(svg.contains("фредерит") || svg.contains("горы ×") || svg.contains("лес ×"));
        // грабля 23: ни одна надпись не вылезает за холст
        let bad = audit_text_bounds(&svg);
        assert!(bad.is_empty(), "надписи за холстом: {bad:?}");
        // маленький диск — крупный гекс, подписи координат включаются
        let small: Vec<(i64, i64, usize)> =
            crate::geo::hexgrid::hex_disk(2).iter().map(|&(q, r)| (q, r, 3)).collect();
        let svg2 = hex_svg(&small, "Малый диск", "R2").unwrap();
        assert_eq!(svg2.matches("<polygon").count(), 19);
        assert!(svg2.contains("1,0"), "подписи координат: {}", &svg2[..svg2.len().min(400)]);
        // ошибки
        assert!(hex_svg(&[], "пусто", "-").is_err());
        assert!(hex_svg(&[(0, 0, 9)], "биом", "-").is_err());
    }

    // ===== Сессия-16: TIN-рельеф =====

    #[test]
    fn tin_svg_relief_and_structure() {
        use crate::geo::delaunay::delaunay;
        let pts = vec![(0, 0), (8, 0), (8, 8), (0, 8), (4, 4)];
        let tin = delaunay(&pts).unwrap();
        // рельеф: пирамида — 4 треугольника с разными биомами
        let z = vec![0.0, 0.1, 0.9, 0.2, 0.7];
        let svg = tin_svg(&tin, Some(&z), "TIN · пирамида", "5 вершин · 4 треугольника").unwrap();
        assert!(starts_and_ends(&svg));
        assert_eq!(svg.matches("<polygon").count(), 4, "все треугольники нарисованы");
        assert!(svg.contains("лес ×2"), "высокий центр — лес: см. вывод");
        assert!(svg.matches("<circle").count() == 5, "вершины сети");
        let bad = audit_text_bounds(&svg);
        assert!(bad.is_empty(), "надписи за холстом: {bad:?}");
        // структура без высот: графитовый тон, без легенды биомов
        let svg2 = tin_svg(&tin, None, "TIN · сеть", "структура").unwrap();
        assert!(starts_and_ends(&svg2));
        assert!(svg2.contains("#94a3b8"), "нейтральный тон без рельефа");
        assert!(!svg2.contains("океан ×"), "без легенды биомов");
        // детерминизм: побитово
        let svg3 = tin_svg(&tin, Some(&z), "TIN · пирамида", "5 вершин · 4 треугольника").unwrap();
        assert_eq!(svg, svg3);
    }

    #[test]
    fn tin_svg_eteria_relief() {
        // конвейер Сессии-16: золотой посев → Муха → треугольники → биомы
        let verts = crate::geo::eteria::tin_vertices(7, 128).unwrap();
        let k = 10u8;
        let pts_i: Vec<(i64, i64)> = verts
            .iter()
            .map(|&(x, y, _)| {
                let tp = crate::geo::trit_coord::TritPoint::quantize(x, y, k).unwrap();
                (tp.x.to_i64(), tp.y.to_i64())
            })
            .collect();
        let tin = crate::geo::delaunay::delaunay(&pts_i).unwrap();
        let z: Vec<f64> = verts.iter().map(|&(_, _, h)| h).collect();
        let svg = tin_svg(
            &tin,
            Some(&z),
            "Этерия · TIN-рельеф",
            "128 вершин золотого посева · Муха · сессия-16",
        )
        .unwrap();
        assert!(starts_and_ends(&svg));
        assert!(svg.matches("<polygon").count() == tin.tris.len());
        // разнообразие биомов: не один цвет на всю карту
        let colors: std::collections::HashSet<&str> = svg
            .lines()
            .filter(|l| l.contains("<polygon"))
            .filter_map(|l| l.split("fill=\"").nth(1))
            .map(|rest| rest.split('"').next().unwrap())
            .collect();
        assert!(colors.len() >= 3, "мозаика биомов: {:?}", colors);
        let bad = audit_text_bounds(&svg);
        assert!(bad.is_empty(), "надписи за холстом: {bad:?}");
    }

    #[test]
    fn tin_svg_errors() {
        use crate::geo::delaunay::delaunay;
        let tin = delaunay(&[(0, 0), (8, 0), (4, 6)]).unwrap();
        // несогласованные высоты
        assert!(tin_svg(&tin, Some(&[1.0, 2.0]), "-", "-").is_err());
        // пустая триангуляция
        let empty = crate::geo::delaunay::Tin::empty();
        assert!(tin_svg(&empty, None, "-", "-").is_err());
    }
}
