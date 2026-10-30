//! Интерактивный 3D/4D-визор молекул: `poler-engine --view-mol "CC(=O)Oc1ccccc1C(=O)O"`.
//!
//! SVG — мёртвый снимок; здесь — живая орбитальная камера прямо в терминале:
//!
//! * **3D**: орбитальное вращение мышью/стрелками (yaw/pitch), зум;
//!   программный растеризатор с z-буфером, полублоки ▀ (двойное вертикальное
//!   разрешение), CPK-раскраска, ламбертово затенение.
//! * **4D**: кручение вращаемых σ-связей во времени (`t`) — молекула
//!   «дышит» конформациями; ползунок скорости, пауза.
//! * Режимы: шаростержневой (ball-and-stick), Ван-дер-Ваальс
//!   (space-filling, реальные габариты), каркас (wireframe).
//! * Не-TTY: одиночный кадр ANSI/ASCII (для пайпов и CI).
//!
//! Рендер — чистый Rust без зависимостей; TUI-цикл — crossterm (уже в
//! дереве движка). Для будущего GPU-окна P³-Engine рендер-ядро
//! (камера/сцена/кадр) отделено от вывода.

use super::geom3d::{covalent_radius, vdw_radius, Conformer};
use super::smiles::{BondOrder, MoleculeGraph};

/// Режим отображения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Шаростержневой: атомы — сферы ковалентных радиусов, связи — стержни.
    BallStick,
    /// Space-filling: Ван-дер-Ваальсовы сферы (реальный объём молекулы).
    Vdw,
    /// Каркас: только связи.
    Wire,
}

impl ViewMode {
    pub fn next(self) -> ViewMode {
        match self {
            ViewMode::BallStick => ViewMode::Vdw,
            ViewMode::Vdw => ViewMode::Wire,
            ViewMode::Wire => ViewMode::BallStick,
        }
    }

    pub fn name_ru(self) -> &'static str {
        match self {
            ViewMode::BallStick => "шаростержневой",
            ViewMode::Vdw => "ван-дер-ваальс",
            ViewMode::Wire => "каркас",
        }
    }
}

/// Орбитальная камера.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub yaw: f64,
    pub pitch: f64,
    /// Зум (множитель масштаба).
    pub zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            yaw: 0.7,
            pitch: 0.42,
            zoom: 1.0,
        }
    }
}

/// CPK-цвет атома (r, g, b 0–255).
pub fn cpk_color(symbol: &str) -> (u8, u8, u8) {
    match symbol {
        "H" => (240, 240, 240),
        "C" => (120, 120, 120),
        "N" => (48, 80, 248),
        "O" => (248, 48, 48),
        "S" => (248, 200, 48),
        "P" => (248, 140, 0),
        "F" | "Cl" => (48, 230, 48),
        "Br" => (166, 64, 32),
        "I" => (148, 80, 210),
        "Fe" => (216, 96, 96),
        "Zn" => (160, 128, 176),
        "Mg" => (96, 200, 96),
        "Ca" => (64, 192, 192),
        "Na" => (160, 192, 255),
        "K" => (192, 128, 255),
        "Li" => (204, 102, 255),
        _ => (255, 128, 192),
    }
}

/// Экранный радиус сферы атома (пиксели) по режиму.
fn atom_radius_px(symbol: &str, mode: ViewMode, scale: f64) -> f64 {
    let angstrom = match mode {
        ViewMode::BallStick => {
            if symbol == "H" {
                0.24
            } else {
                (covalent_radius(symbol) * 0.55).max(0.3)
            }
        }
        ViewMode::Vdw => vdw_radius(symbol) * 0.97,
        ViewMode::Wire => 0.07,
    };
    (angstrom * scale).max(1.2)
}

/// Кадр рендера: пиксельный буфер W×H (H = 2×строк полублоков).
struct Canvas {
    w: usize,
    h: usize,
    /// Глубина: больше = ближе к камере.
    z: Vec<f64>,
    /// Цвет RGB.
    c: Vec<(u8, u8, u8)>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Canvas {
            w,
            h,
            z: vec![f64::NEG_INFINITY; w * h],
            c: vec![(0, 0, 0); w * h],
        }
    }

    /// Пиксель с z-тестом (большая глубина ближе).
    fn set(&mut self, x: i64, y: i64, z: f64, color: (u8, u8, u8)) {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            return;
        }
        let idx = y as usize * self.w + x as usize;
        if z > self.z[idx] {
            self.z[idx] = z;
            self.c[idx] = color;
        }
    }
}

/// Сцена: молекула + камера → пиксельный холст.
fn draw_scene(
    conf: &Conformer,
    positions: &[[f64; 3]],
    cam: &Camera,
    mode: ViewMode,
    w: usize,
    h: usize,
) -> Canvas {
    let mut canvas = Canvas::new(w, h);
    let n = positions.len();
    if n == 0 {
        return canvas;
    }

    // Масштаб: вписать молекулу (радиус от центроида)
    let r_max = positions
        .iter()
        .map(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt())
        .fold(0.0f64, f64::max)
        .max(1.5);
    let scale = (w.min(h) as f64) / (2.4 * r_max) * cam.zoom;
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;

    let (sy, cyw) = (cam.yaw.sin(), cam.yaw.cos());
    let (sp, cp) = (cam.pitch.sin(), cam.pitch.cos());

    // Проецируем все узлы
    let mut proj: Vec<(f64, f64, f64)> = Vec::with_capacity(n); // (sx, sy, depth)
    for p in positions {
        let xr = p[0] * cyw + p[2] * sy;
        let zr = -p[0] * sy + p[2] * cyw;
        let yr = p[1] * cp - zr * sp;
        let zr2 = p[1] * sp + zr * cp;
        proj.push((cx + xr * scale, cy - yr * scale, zr2));
    }

    // 1. Связи (Bresenham с интерполяцией глубины)
    if mode != ViewMode::Vdw {
        for b in &conf.bonds {
            let (i, j) = (b.a, b.b);
            if i >= proj.len() || j >= proj.len() {
                continue;
            }
            let (x1, y1, z1) = proj[i];
            let (x2, y2, z2) = proj[j];
            // Цвет связи: тёмное среднее CPK-цветов концов
            let (ca, cb) = (cpk_color(&conf.nodes[i].symbol), cpk_color(&conf.nodes[j].symbol));
            let dim = |c: (u8, u8, u8)| -> (u8, u8, u8) {
                ((c.0 as u32 * 55 / 100) as u8, (c.1 as u32 * 55 / 100) as u8, (c.2 as u32 * 55 / 100) as u8)
            };
            let mix = dim((
                ((ca.0 as u32 + cb.0 as u32) / 2) as u8,
                ((ca.1 as u32 + cb.1 as u32) / 2) as u8,
                ((ca.2 as u32 + cb.2 as u32) / 2) as u8,
            ));
            let lines = match b.order {
                BondOrder::Double => 2,
                BondOrder::Triple => 3,
                _ => 1,
            };
            // Перпендикуляр смещения параллельных линий
            let dx = x2 - x1;
            let dy = y2 - y1;
            let len = (dx * dx + dy * dy).sqrt().max(1e-9);
            let px = -dy / len;
            let py = dx / len;
            for k in 0..lines {
                let off = (k as f64 - (lines - 1) as f64 / 2.0) * 1.6;
                let (ax, ay) = (x1 + px * off, y1 + py * off);
                let (bx, by) = (x2 + px * off, y2 + py * off);
                draw_line(&mut canvas, ax, ay, z1, bx, by, z2, mix);
            }
        }
    }

    // 2. Атомы (сферы)
    // Порядок не важен: z-буфер обрабатывает перекрытия
    for (ni, node) in conf.nodes.iter().enumerate() {
        if ni >= proj.len() {
            continue;
        }
        let (sx, sy, zc) = proj[ni];
        let r = atom_radius_px(&node.symbol, mode, scale);
        if mode == ViewMode::Wire && node.is_h {
            continue; // H в каркасе не рисуем
        }
        let base = cpk_color(&node.symbol);
        let ir = r.ceil() as i64;
        let x0 = (sx as i64) - ir;
        let y0 = (sy as i64) - ir;
        // Сфера: фронт-полусфера + ламбертово затенение
        for dy in -ir..=ir {
            for dxx in -ir..=ir {
                let d2 = (dxx * dxx + dy * dy) as f64;
                if d2 > r * r {
                    continue;
                }
                let zf = zc + (r * r - d2).sqrt();
                let (nx, ny) = (dxx as f64 / r, dy as f64 / r);
                let nz = (1.0 - (nx * nx + ny * ny).min(1.0)).sqrt();
                let l_dot = (-0.4 * nx - 0.6 * ny + 0.7 * nz).max(0.0);
                let shade = 0.45 + 0.55 * l_dot;
                let color = (
                    (base.0 as f64 * shade).min(255.0) as u8,
                    (base.1 as f64 * shade).min(255.0) as u8,
                    (base.2 as f64 * shade).min(255.0) as u8,
                );
                canvas.set((sx as i64) + dxx, (sy as i64) + dy, zf, color);
            }
        }
    }

    canvas
}

/// Линия Брезенхэма с интерполяцией глубины.
fn draw_line(
    canvas: &mut Canvas,
    x1: f64,
    y1: f64,
    z1: f64,
    x2: f64,
    y2: f64,
    z2: f64,
    color: (u8, u8, u8),
) {
    let steps = (((x2 - x1).abs().max((y2 - y1).abs()) * 1.4).ceil() as usize).max(1);
    for s in 0..=steps {
        let t = s as f64 / steps as f64;
        let x = x1 + (x2 - x1) * t;
        let y = y1 + (y2 - y1) * t;
        let z = z1 + (z2 - z1) * t;
        canvas.set(x.round() as i64, y.round() as i64, z, color);
    }
}

/// ANSI-кадр из холста: полублоки ▀ (верх/низ в одном символе).
pub fn canvas_to_ansi(canvas: &Canvas, color: bool, luminance_chars: bool) -> String {
    let rows = canvas.h / 2;
    let mut out = String::with_capacity(rows * canvas.w * 20);
    for r in 0..rows {
        for col in 0..canvas.w {
            let top = r * 2 * canvas.w + col;
            let bot = (r * 2 + 1) * canvas.w + col;
            let top_c = canvas.c[top];
            let bot_c = canvas.c[bot];
            let top_set = canvas.z[top] > f64::NEG_INFINITY;
            let bot_set = canvas.z[bot] > f64::NEG_INFINITY;
            if !top_set && !bot_set {
                out.push(' ');
                continue;
            }
            if luminance_chars {
                // ASCII-градации яркости (пайп/CI)
                let lum = |c: (u8, u8, u8)| -> f64 {
                    0.299 * c.0 as f64 + 0.587 * c.1 as f64 + 0.114 * c.2 as f64
                };
                let l = if top_set { lum(top_c) } else { lum(bot_c) };
                let ramp = " .:-=+*#%@";
                let idx = ((l / 255.0) * (ramp.len() - 1) as f64).round() as usize;
                out.push(ramp.chars().nth(idx.min(ramp.len() - 1)).unwrap_or(' '));
                continue;
            }
            let fg = if top_set { top_c } else { (0, 0, 0) };
            let bg = if bot_set { bot_c } else { (0, 0, 0) };
            if color {
                out.push_str(&format!(
                    "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
                    fg.0, fg.1, fg.2, bg.0, bg.1, bg.2
                ));
            } else {
                // Без цвета: символ по яркости
                let lum =
                    0.299 * fg.0 as f64 + 0.587 * fg.1 as f64 + 0.114 * fg.2 as f64;
                let ramp = " .:-=+*#%@";
                let idx = ((lum / 255.0) * (ramp.len() - 1) as f64).round() as usize;
                out.push(ramp.chars().nth(idx.min(ramp.len() - 1)).unwrap_or(' '));
            }
        }
        out.push_str("\x1b[0m\n");
    }
    out
}

/// Полный кадр: сцена + ANSI.
pub fn render_frame(
    conf: &Conformer,
    positions: &[[f64; 3]],
    cam: &Camera,
    mode: ViewMode,
    width: usize,
    rows: usize,
    color: bool,
) -> String {
    let canvas = draw_scene(conf, positions, cam, mode, width, rows * 2);
    canvas_to_ansi(&canvas, color, false)
}

/// Позиции с 4D-кручением вращаемых σ-связей в момент t.
///
/// Каждая вращаемая связь получает фазу по золотому углу (детерминизм),
/// амплитуду 35° и период ~7 единиц времени: молекула «дышит»
/// конформациями без случайностей.
pub fn torsion_positions(
    conf: &Conformer,
    g: &MoleculeGraph,
    t: f64,
) -> Vec<[f64; 3]> {
    let mut pos = conf.positions.clone();
    if t == 0.0 {
        return pos;
    }
    let rotatable = g.rotatable_bonds();
    if rotatable.is_empty() {
        return pos;
    }
    // Смежность узлов
    let n = conf.nodes.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for b in &conf.bonds {
        adj[b.a].push(b.b);
        adj[b.b].push(b.a);
    }
    const GOLDEN: f64 = 2.399963229728653;
    for (k, &bi) in rotatable.iter().enumerate() {
        let gb = &g.bonds[bi];
        let na = conf.heavy_map[gb.a];
        let nb = conf.heavy_map[gb.b];
        let phase = (k as f64) * GOLDEN;
        let theta = 35.0f64.to_radians() * (0.9 * t + phase).sin();
        if theta.abs() < 1e-6 {
            continue;
        }
        // Поддерево со стороны nb (не пересекая связь na-nb)
        let mut subtree = vec![nb];
        let mut seen = vec![false; n];
        seen[nb] = true;
        seen[na] = true; // блокируем переход через связь
        let mut qi = 0;
        while qi < subtree.len() {
            let u = subtree[qi];
            qi += 1;
            for &w in &adj[u] {
                if !seen[w] {
                    seen[w] = true;
                    subtree.push(w);
                }
            }
        }
        // Ось вращения: na → nb
        let pa = pos[na];
        let pb = pos[nb];
        let axis = normalize_axis([pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]]);
        rotate_around(&mut pos, &subtree, pa, axis, theta);
    }
    pos
}

fn normalize_axis(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n < 1e-12 {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / n, v[1] / n, v[2] / n]
    }
}

/// Вращение Родрига поддерева вокруг оси (точка p, направление u) на угол θ.
fn rotate_around(
    pos: &mut [[f64; 3]],
    indices: &[usize],
    p: [f64; 3],
    u: [f64; 3],
    theta: f64,
) {
    let (s, c) = (theta.sin(), theta.cos());
    let dot_v = |v: [f64; 3]| v[0] * u[0] + v[1] * u[1] + v[2] * u[2];
    let cross_u_v = |v: [f64; 3]| {
        [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ]
    };
    for &i in indices {
        let v = [pos[i][0] - p[0], pos[i][1] - p[1], pos[i][2] - p[2]];
        let d = dot_v(v);
        let cr = cross_u_v(v);
        pos[i] = [
            p[0] + v[0] * c + cr[0] * s + u[0] * d * (1.0 - c),
            p[1] + v[1] * c + cr[1] * s + u[1] * d * (1.0 - c),
            p[2] + v[2] * c + cr[2] * s + u[2] * d * (1.0 - c),
        ];
    }
}

/// Один кадр (не-TTY путь): без raw-mode, с рамкой и подписью.
pub fn render_single(
    conf: &Conformer,
    g: &MoleculeGraph,
    formula: &str,
    mode: ViewMode,
    color: bool,
) -> String {
    let cam = Camera::default();
    let positions = torsion_positions(conf, g, 0.0);
    let width = 64usize;
    let rows = 22usize;
    let frame = render_frame(conf, &positions, &cam, mode, width, rows, color);
    let mut out = String::new();
    out.push_str(&format!(
        "── 3D {} · {} · {} атомов · режим: {} ──\n",
        formula,
        if conf.relaxed { "конформер" } else { "эскиз" },
        conf.nodes.len(),
        mode.name_ru()
    ));
    out.push_str(&frame);
    out.push_str("Интерактивно: poler-engine --view-mol <SMILES> в терминале (вращение стрелками/hjkl, зум +/-, режим m, 4D-кручение t)\n");
    out
}

/// Интерактивный TUI-цикл (crossterm). Возвращает при выходе (q/Esc).
pub fn run_interactive(
    conf: &Conformer,
    g: &MoleculeGraph,
    formula: &str,
    initial_mode: ViewMode,
) -> Result<(), String> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use crossterm::{execute, terminal};

    if !atty_check() {
        // Не-терминал: одиночный кадр
        let frame = render_single(conf, g, formula, initial_mode, false);
        print!("{frame}");
        return Ok(());
    }

    terminal::enable_raw_mode().map_err(|e| format!("raw mode: {e}"))?;
    let mut cleanup = || -> std::io::Result<()> {
        execute!(
            std::io::stdout(),
            terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        )?;
        terminal::disable_raw_mode()
    };
    if execute!(std::io::stdout(), terminal::EnterAlternateScreen, crossterm::cursor::Hide).is_err() {
        let _ = cleanup();
        return Err("не удалось войти в alternate screen".into());
    }

    let mut cam = Camera::default();
    let mut mode = initial_mode;
    let mut animate = true;
    let mut paused = false;
    let mut speed = 1.0f64;
    let mut t = 0.0f64;
    let (mut w, mut h) = terminal::size().map_err(|e| format!("size: {e}"))?;
    let result = (|| -> Result<(), String> {
        loop {
            // Кадр
            let positions = torsion_positions(conf, g, t);
            let rows = h.saturating_sub(4).max(8) as usize;
            let cols = w.saturating_sub(2).max(20) as usize;
            let frame = render_frame(conf, &positions, &cam, mode, cols, rows, true);
            let status = format!(
                " {} · {} · {} атомов · {} · yaw {:+.1}° pitch {:+.1}° zoom {:.2}{} · t={:.1} speed {:.1} ",
                formula,
                mode.name_ru(),
                conf.nodes.len(),
                if conf.relaxed { "конформер" } else { "эскиз" },
                cam.yaw.to_degrees(),
                cam.pitch.to_degrees(),
                cam.zoom,
                if animate {
                    if paused { " · ⏸" } else { " · ▶4D" }
                } else {
                    ""
                },
                t,
                speed
            );
            let help = " ←→↑↓/hjkl: вращение · +/-: зум · m: режим · t: 4D вкл/выкл · space: пауза · [ ]: скорость · r: сброс · q: выход ";
            let mut out = String::new();
            out.push_str("\x1b[H\x1b[2J");
            out.push_str(&format!("\x1b[1m\x1b[38;2;180;220;255m{status}\x1b[0m\n"));
            out.push_str(&frame);
            out.push_str(&format!("\x1b[38;2;140;140;160m{help}\x1b[0m\r"));
            print!("{out}");
            use std::io::Write;
            let _ = std::io::stdout().flush();

            // События с таймаутом (частота кадров ~33 мс)
            let timeout = std::time::Duration::from_millis(33);
            match crossterm::event::poll(timeout) {
                Ok(true) => loop {
                    match crossterm::event::read() {
                        Ok(Event::Key(KeyEvent { code, modifiers, .. })) => {
                            match code {
                                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                                    return Ok(());
                                }
                                KeyCode::Left | KeyCode::Char('h') => cam.yaw -= 0.18,
                                KeyCode::Right | KeyCode::Char('l') => cam.yaw += 0.18,
                                KeyCode::Up | KeyCode::Char('k') => cam.pitch += 0.14,
                                KeyCode::Down | KeyCode::Char('j') => cam.pitch -= 0.14,
                                KeyCode::Char('+') | KeyCode::Char('=') => cam.zoom *= 1.15,
                                KeyCode::Char('-') | KeyCode::Char('_') => cam.zoom /= 1.15,
                                KeyCode::Char('m') => mode = mode.next(),
                                KeyCode::Char('t') => animate = !animate,
                                KeyCode::Char(' ') => paused = !paused,
                                KeyCode::Char('[') => speed = (speed / 1.4).max(0.1),
                                KeyCode::Char(']') => speed = (speed * 1.4).min(10.0),
                                KeyCode::Char('r') => {
                                    cam = Camera::default();
                                    t = 0.0;
                                }
                                _ => {}
                            }
                            if let Ok(true) = crossterm::event::poll(std::time::Duration::from_millis(0))
                            {
                                continue; // сливаем очередь событий
                            }
                            break;
                        }
                        Ok(Event::Resize(nw, nh)) => {
                            w = nw;
                            h = nh;
                            break;
                        }
                        Ok(_) => break,
                        Err(e) => return Err(format!("событие: {e}")),
                    }
                },
                Ok(false) => {}
                Err(e) => return Err(format!("poll: {e}")),
            }

            // Время 4D
            if animate && !paused {
                t += 0.05 * speed;
            }
        }
    })();

    let _ = cleanup();
    result
}

/// TTY-проверка без libc-зависимостей: crossterm не даёт прямого API,
/// используем stdin через_atty хак std (is_terminal стабилен с 1.70).
fn atty_check() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::super::geom3d::embed;
    use super::super::smiles::parse_smiles;
    use super::*;

    fn conf_of(smi: &str) -> (MoleculeGraph, Conformer) {
        let g = parse_smiles(smi).unwrap();
        let c = embed(&g).unwrap();
        (g, c)
    }

    #[test]
    fn render_water_frame() {
        let (g, c) = conf_of("O");
        let frame = render_frame(&c, &c.positions, &Camera::default(), ViewMode::BallStick, 40, 12, false);
        assert!(!frame.is_empty());
        assert!(frame.contains('\n'));
        // ASCII-градации что-то рисуют (не пробелы только)
        let drawn: usize = frame.chars().filter(|ch| ":-=+*#%@".contains(*ch)).count();
        assert!(drawn >= 3, "кадр воды пуст: {frame:?}");
    }

    #[test]
    fn render_benzene_color() {
        let (g, c) = conf_of("c1ccccc1");
        let frame = render_frame(&c, &c.positions, &Camera::default(), ViewMode::BallStick, 50, 16, true);
        assert!(frame.contains("▀"), "полублоки обязаны присутствовать");
        assert!(frame.contains("\x1b[38;2;"), "ANSI-цвет");
    }

    #[test]
    fn render_modes_differ() {
        let (g, c) = conf_of("CC(=O)Oc1ccccc1C(=O)O");
        let a = render_frame(&c, &c.positions, &Camera::default(), ViewMode::BallStick, 60, 20, false);
        let b = render_frame(&c, &c.positions, &Camera::default(), ViewMode::Vdw, 60, 20, false);
        let d = render_frame(&c, &c.positions, &Camera::default(), ViewMode::Wire, 60, 20, false);
        assert_ne!(a, b, "шаростержневой ≠ ВдВ");
        assert_ne!(b, d, "ВдВ ≠ каркас");
    }

    #[test]
    fn render_deterministic() {
        let (g, c) = conf_of("CCO");
        let cam = Camera { yaw: 1.1, pitch: -0.3, zoom: 1.4 };
        let a = render_frame(&c, &c.positions, &cam, ViewMode::BallStick, 44, 14, false);
        let b = render_frame(&c, &c.positions, &cam, ViewMode::BallStick, 44, 14, false);
        assert_eq!(a, b);
    }

    #[test]
    fn camera_changes_view() {
        let (g, c) = conf_of("CCO");
        let a = render_frame(&c, &c.positions, &Camera::default(), ViewMode::BallStick, 44, 14, false);
        let rotated = Camera { yaw: 2.5, pitch: 1.2, zoom: 1.0 };
        let b = render_frame(&c, &c.positions, &rotated, ViewMode::BallStick, 44, 14, false);
        assert_ne!(a, b, "поворот камеры меняет кадр");
    }

    #[test]
    fn torsion_animates_butane() {
        let (g, c) = conf_of("CCCC");
        assert!(g.rotatable_bonds().len() >= 1, "у бутана есть вращаемая связь");
        let p0 = torsion_positions(&c, &g, 0.0);
        let p1 = torsion_positions(&c, &g, 1.7);
        // Молекула изменилась
        let mut changed = false;
        for i in 0..p0.len() {
            if (p0[i][0] - p1[i][0]).abs() > 0.05 || (p0[i][1] - p1[i][1]).abs() > 0.05 {
                changed = true;
                break;
            }
        }
        assert!(changed, "кручение 4D обязано менять геометрию");
        // Связи сохраняются: длина C-C в p1 остаётся ~1.52
        for b in &c.bonds {
            let d0 = {
                let dx = p0[b.a][0] - p0[b.b][0];
                let dy = p0[b.a][1] - p0[b.b][1];
                let dz = p0[b.a][2] - p0[b.b][2];
                (dx * dx + dy * dy + dz * dz).sqrt()
            };
            let d1 = {
                let dx = p1[b.a][0] - p1[b.b][0];
                let dy = p1[b.a][1] - p1[b.b][1];
                let dz = p1[b.a][2] - p1[b.b][2];
                (dx * dx + dy * dy + dz * dz).sqrt()
            };
            assert!((d0 - d1).abs() < 1e-6, "кручение рвёт связь: {d0} → {d1}");
        }
    }

    #[test]
    fn torsion_deterministic() {
        let (g, c) = conf_of("CCCC");
        let a = torsion_positions(&c, &g, 2.3);
        let b = torsion_positions(&c, &g, 2.3);
        assert_eq!(a, b);
    }

    #[test]
    fn render_single_has_caption() {
        let (g, c) = conf_of("O");
        let text = render_single(&c, &g, "H2O", ViewMode::BallStick, false);
        assert!(text.contains("H2O"));
        assert!(text.contains("--view-mol"));
    }
}

// ═══ v0.75.0: визор белок-лигандного комплекса ═════════════════════════

/// Сфера сцены (общий рендер для комплексов).
pub struct SceneSphere {
    pub pos: [f64; 3],
    /// Радиус в Å.
    pub radius: f64,
    pub color: (u8, u8, u8),
}

/// Линия сцены (трейс/связи/дисульфиды).
pub struct SceneLine {
    pub a: [f64; 3],
    pub b: [f64; 3],
    pub color: (u8, u8, u8),
}

/// Режимы визора комплекса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexMode {
    /// CA-трейс белка + лиганд CPK.
    Trace,
    /// Трейс + боковые цепи кармана + лиганд CPK.
    Pocket,
    /// Space-filling ВдВ-поверхность белка + лиганд CPK.
    Vdw,
}

impl ComplexMode {
    pub fn next(self) -> ComplexMode {
        match self {
            ComplexMode::Trace => ComplexMode::Pocket,
            ComplexMode::Pocket => ComplexMode::Vdw,
            ComplexMode::Vdw => ComplexMode::Trace,
        }
    }

    pub fn name_ru(self) -> &'static str {
        match self {
            ComplexMode::Trace => "CA-трейс",
            ComplexMode::Pocket => "карман",
            ComplexMode::Vdw => "ВдВ-поверхность",
        }
    }
}

/// Собранная сцена комплекса: трейс/карман/поверхность/лиганды.
pub struct ComplexScene {
    pub trace_lines: Vec<SceneLine>,
    pub ca_spheres: Vec<SceneSphere>,
    pub pocket_lines: Vec<SceneLine>,
    pub pocket_spheres: Vec<SceneSphere>,
    pub vdw_spheres: Vec<SceneSphere>,
    pub ligand_lines: Vec<SceneLine>,
    pub ligand_spheres: Vec<SceneSphere>,
    pub n_residues: usize,
    pub n_ligand_atoms: usize,
    pub pocket_names: Vec<String>,
    pub title: String,
}

/// Связь по расстоянию (внутри остатка/лиганда): d ≤ r_i + r_j + 0.45 Å.
fn infer_bonds(atoms: &[(usize, [f64; 3])], radii: &dyn Fn(usize) -> f64) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for i in 0..atoms.len() {
        for j in (i + 1)..atoms.len() {
            let d = {
                let dx = atoms[i].1[0] - atoms[j].1[0];
                let dy = atoms[i].1[1] - atoms[j].1[1];
                let dz = atoms[i].1[2] - atoms[j].1[2];
                (dx * dx + dy * dy + dz * dz).sqrt()
            };
            if d <= radii(i) + radii(j) + 0.45 {
                out.push((i, j));
            }
        }
    }
    out
}

/// Собрать сцену из макромолекулы: белок + лиганды (HETATM не-воды).
pub fn build_complex_scene(mm: &crate::chem::pdb::MacroMol) -> ComplexScene {
    use crate::chem::pdb::{ResClass, ResKind};

    let mut sc = ComplexScene {
        trace_lines: Vec::new(),
        ca_spheres: Vec::new(),
        pocket_lines: Vec::new(),
        pocket_spheres: Vec::new(),
        vdw_spheres: Vec::new(),
        ligand_lines: Vec::new(),
        ligand_spheres: Vec::new(),
        n_residues: 0,
        n_ligand_atoms: 0,
        pocket_names: Vec::new(),
        title: mm.title.clone(),
    };

    // Центрирование: сдвиг всех координат к центроиду CA (камера в нуле)
    let mut centroid = [0.0f64; 3];
    let mut n_ca = 0usize;
    for (ri, r) in mm.residues.iter().enumerate() {
        if r.kind != ResKind::Protein {
            continue;
        }
        if let Some(ca) = mm.residue_atom(ri, "CA") {
            centroid[0] += mm.atoms[ca].pos[0];
            centroid[1] += mm.atoms[ca].pos[1];
            centroid[2] += mm.atoms[ca].pos[2];
            n_ca += 1;
        }
    }
    if n_ca > 0 {
        for k in 0..3 {
            centroid[k] /= n_ca as f64;
        }
    }
    let shift = |p: [f64; 3]| -> [f64; 3] {
        [p[0] - centroid[0], p[1] - centroid[1], p[2] - centroid[2]]
    };

    // ВдВ-поверхность: все тяжёлые атомы белка
    for a in &mm.atoms {
        if a.het || a.elem_str() == "H" {
            continue;
        }
        sc.vdw_spheres.push(SceneSphere {
            pos: shift(a.pos),
            radius: vdw_radius(a.elem_str()),
            color: cpk_color(a.elem_str()),
        });
    }

    // CA-трейс по цепочкам
    let mut prev: Option<(u8, [f64; 3], ResClass)> = None;
    for (ri, r) in mm.residues.iter().enumerate() {
        if r.kind != ResKind::Protein {
            continue;
        }
        sc.n_residues += 1;
        let cls = mm.res_class(ri);
        if let Some(ca) = mm.residue_atom(ri, "CA") {
            let p = shift(mm.atoms[ca].pos);
            sc.ca_spheres.push(SceneSphere {
                pos: p,
                radius: 0.62,
                color: cls.color(),
            });
            if let Some((pc, pp, _pcls)) = prev {
                if pc == r.chain {
                    sc.trace_lines.push(SceneLine {
                        a: pp,
                        b: p,
                        color: dim(cls.color()),
                    });
                }
            }
            prev = Some((r.chain, p, cls));
        } else {
            prev = None;
        }
    }

    // Лиганды: атомы + связи по расстоянию
    let lig_res: Vec<usize> = mm.ligand_residues();
    let mut lig_atom_idx: Vec<usize> = Vec::new();
    for &lri in &lig_res {
        for ai in mm.residue_atom_ids(lri) {
            lig_atom_idx.push(ai);
        }
    }
    sc.n_ligand_atoms = lig_atom_idx.len();
    let lig_pos: Vec<(usize, [f64; 3])> = lig_atom_idx
        .iter()
        .map(|&ai| (ai, shift(mm.atoms[ai].pos)))
        .collect();
    let cov_of = |ai: usize| covalent_radius(mm.atoms[ai].elem_str());
    let bonds = infer_bonds(&lig_pos, &|k| cov_of(lig_pos[k].0));
    for (k, &(ai, p)) in lig_pos.iter().enumerate() {
        sc.ligand_spheres.push(SceneSphere {
            pos: p,
            radius: if mm.atoms[ai].elem_str() == "H" { 0.24 } else { 0.85 },
            color: cpk_color(mm.atoms[ai].elem_str()),
        });
    }
    for &(i, j) in &bonds {
        sc.ligand_lines.push(SceneLine {
            a: lig_pos[i].1,
            b: lig_pos[j].1,
            color: dim(cpk_color(mm.atoms[lig_pos[i].0].elem_str())),
        });
    }

    // Карман: остатки в 8 Å от лигандов; боковые цепи — каркасом
    if !lig_atom_idx.is_empty() {
        let mut pocket_res: Vec<usize> = Vec::new();
        for r in mm.residues.iter().enumerate() {
            let (ri, rr) = r;
            if rr.kind != ResKind::Protein {
                continue;
            }
            'atoms: for ai in mm.residue_atom_ids(ri) {
                for &lj in &lig_atom_idx {
                    let d = {
                        let dx = mm.atoms[ai].pos[0] - mm.atoms[lj].pos[0];
                        let dy = mm.atoms[ai].pos[1] - mm.atoms[lj].pos[1];
                        let dz = mm.atoms[ai].pos[2] - mm.atoms[lj].pos[2];
                        (dx * dx + dy * dy + dz * dz).sqrt()
                    };
                    if d <= 8.0 {
                        pocket_res.push(ri);
                        break 'atoms;
                    }
                }
            }
        }
        let mut named: Vec<String> = Vec::new();
        for &ri in &pocket_res {
            let rr = &mm.residues[ri];
            named.push(format!(
                "{}{}{}",
                rr.name_str(),
                if rr.chain == 0 { '_' } else { rr.chain as char },
                rr.seq
            ));
            // боковая цепь: всё кроме N/CA/C/O/OXT
            let atoms: Vec<(usize, [f64; 3])> = mm
                .residue_atom_ids(ri)
                .into_iter()
                .filter(|&ai| {
                    let nm = mm.atoms[ai].name_str();
                    !matches!(nm, "N" | "CA" | "C" | "O" | "OXT") && mm.atoms[ai].elem_str() != "H"
                })
                .map(|ai| (ai, shift(mm.atoms[ai].pos)))
                .collect();
            let cls_color = mm.res_class(ri).color();
            for &(ai, p) in &atoms {
                sc.pocket_spheres.push(SceneSphere {
                    pos: p,
                    radius: 0.45,
                    color: cls_color,
                });
            }
            // связи внутри боковой цепи
            let pbonds = infer_bonds(&atoms, &|k| cov_of(atoms[k].0));
            for &(i, j) in &pbonds {
                sc.pocket_lines.push(SceneLine {
                    a: atoms[i].1,
                    b: atoms[j].1,
                    color: dim(cls_color),
                });
            }
            // связь CB→CA (визуальная непрерывность)
            if let (Some(ca), Some(cb)) = (mm.residue_atom(ri, "CA"), mm.residue_atom(ri, "CB")) {
                sc.pocket_lines.push(SceneLine {
                    a: shift(mm.atoms[ca].pos),
                    b: shift(mm.atoms[cb].pos),
                    color: dim(cls_color),
                });
            }
        }
        // дедуп имён (остаток мог добавиться один раз — уже уникально)
        sc.pocket_names = named;
    }

    // Дисульфиды — золотые линии
    for &(a, b) in &mm.ssbonds {
        sc.trace_lines.push(SceneLine {
            a: shift(mm.atoms[a].pos),
            b: shift(mm.atoms[b].pos),
            color: (250, 190, 60),
        });
    }

    sc
}

fn dim(c: (u8, u8, u8)) -> (u8, u8, u8) {
    (
        (c.0 as u32 * 70 / 100) as u8,
        (c.1 as u32 * 70 / 100) as u8,
        (c.2 as u32 * 70 / 100) as u8,
    )
}

/// Общий рендер сцены из сфер и линий (z-буфер, ламберт, полублоки).
pub fn render_complex_frame(
    sc: &ComplexScene,
    mode: ComplexMode,
    cam: &Camera,
    width: usize,
    rows: usize,
    color: bool,
) -> String {
    let h = rows * 2;
    let mut canvas = Canvas::new(width, h);

    // границы сцены
    let mut r_max = 1.5f64;
    let mut extend = |p: [f64; 3]| {
        let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        if r > r_max {
            r_max = r;
        }
    };
    for s in &sc.ca_spheres {
        extend(s.pos);
    }
    for s in &sc.ligand_spheres {
        extend(s.pos);
    }
    if mode == ComplexMode::Vdw {
        for s in &sc.vdw_spheres {
            extend(s.pos);
        }
    }
    let scale = (width.min(h) as f64) / (2.4 * r_max) * cam.zoom;
    let cx = width as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let (sy, cyw) = (cam.yaw.sin(), cam.yaw.cos());
    let (sp, cp) = (cam.pitch.sin(), cam.pitch.cos());
    let project = |p: [f64; 3]| -> (f64, f64, f64) {
        let xr = p[0] * cyw + p[2] * sy;
        let zr = -p[0] * sy + p[2] * cyw;
        let yr = p[1] * cp - zr * sp;
        let zr2 = p[1] * sp + zr * cp;
        (cx + xr * scale, cy - yr * scale, zr2)
    };

    let draw_sphere = |canvas: &mut Canvas, pos: [f64; 3], radius: f64, base: (u8, u8, u8)| {
        let (sx, syc, zc) = project(pos);
        let r = (radius * scale).max(1.2);
        let ir = r.ceil() as i64;
        for dy in -ir..=ir {
            for dxx in -ir..=ir {
                let d2 = (dxx * dxx + dy * dy) as f64;
                if d2 > r * r {
                    continue;
                }
                let zf = zc + (r * r - d2).sqrt();
                let (nx, ny) = (dxx as f64 / r, dy as f64 / r);
                let nz = (1.0 - (nx * nx + ny * ny).min(1.0)).sqrt();
                let l_dot = (-0.4 * nx - 0.6 * ny + 0.7 * nz).max(0.0);
                let shade = 0.45 + 0.55 * l_dot;
                let c = (
                    (base.0 as f64 * shade).min(255.0) as u8,
                    (base.1 as f64 * shade).min(255.0) as u8,
                    (base.2 as f64 * shade).min(255.0) as u8,
                );
                canvas.set((sx as i64) + dxx, (syc as i64) + dy, zf, c);
            }
        }
    };

    // 1. линии
    let lines: &[SceneLine] = match mode {
        ComplexMode::Trace => &sc.trace_lines,
        ComplexMode::Pocket => &sc.trace_lines,
        ComplexMode::Vdw => &[],
    };
    for l in lines {
        let (x1, y1, z1) = project(l.a);
        let (x2, y2, z2) = project(l.b);
        draw_line(&mut canvas, x1, y1, z1, x2, y2, z2, l.color);
    }
    if mode == ComplexMode::Pocket {
        for l in &sc.pocket_lines {
            let (x1, y1, z1) = project(l.a);
            let (x2, y2, z2) = project(l.b);
            draw_line(&mut canvas, x1, y1, z1, x2, y2, z2, l.color);
        }
        for s in &sc.pocket_spheres {
            draw_sphere(&mut canvas, s.pos, s.radius, s.color);
        }
    }

    // 2. сферы
    let spheres: &[SceneSphere] = match mode {
        ComplexMode::Trace | ComplexMode::Pocket => &sc.ca_spheres,
        ComplexMode::Vdw => &sc.vdw_spheres,
    };
    for s in spheres {
        draw_sphere(&mut canvas, s.pos, s.radius, s.color);
    }
    // лиганд поверх
    for l in &sc.ligand_lines {
        let (x1, y1, z1) = project(l.a);
        let (x2, y2, z2) = project(l.b);
        draw_line(&mut canvas, x1, y1, z1, x2, y2, z2, l.color);
    }
    for s in &sc.ligand_spheres {
        draw_sphere(&mut canvas, s.pos, s.radius, s.color);
    }

    canvas_to_ansi(&canvas, color, false)
}

/// Одиночный кадр комплекса (не-TTY).
pub fn render_complex_single(sc: &ComplexScene, mode: ComplexMode, color: bool) -> String {
    let frame = render_complex_frame(sc, mode, &Camera::default(), 68, 24, color);
    let mut out = String::new();
    let title_short: String = sc.title.chars().take(48).collect();
    out.push_str(&format!(
        "── Комплекс: {} · {} остатков · {} атомов лиганда · режим: {} ──\n",
        if title_short.is_empty() { "белок" } else { &title_short },
        sc.n_residues,
        sc.n_ligand_atoms,
        mode.name_ru()
    ));
    if !sc.pocket_names.is_empty() {
        out.push_str(&format!(
            "Карман ({} остатков): {}\n",
            sc.pocket_names.len(),
            sc.pocket_names
                .iter()
                .take(16)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    out.push_str(&frame);
    out.push_str(
        "Интерактивно: poler-engine --view-complex <file.pdb> (вращение стрелками/hjkl, зум +/-, m режим: трейс/карман/ВдВ)\n",
    );
    out
}

/// Интерактивный TUI-цикл визора комплекса.
pub fn run_complex_interactive(sc: &ComplexScene) -> Result<(), String> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use crossterm::{execute, terminal};

    if !atty_check() {
        print!("{}", render_complex_single(sc, ComplexMode::Pocket, false));
        return Ok(());
    }

    terminal::enable_raw_mode().map_err(|e| format!("raw mode: {e}"))?;
    let mut cleanup = || -> std::io::Result<()> {
        execute!(
            std::io::stdout(),
            terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        )?;
        terminal::disable_raw_mode()
    };
    if execute!(std::io::stdout(), terminal::EnterAlternateScreen, crossterm::cursor::Hide).is_err()
    {
        let _ = cleanup();
        return Err("не удалось войти в alternate screen".into());
    }

    let mut cam = Camera {
        yaw: 0.7,
        pitch: 0.42,
        zoom: 1.0,
    };
    let mut mode = ComplexMode::Pocket;
    let (mut w, mut h) = terminal::size().map_err(|e| format!("size: {e}"))?;
    let result = (|| -> Result<(), String> {
        loop {
            let rows = h.saturating_sub(4).max(8) as usize;
            let cols = w.saturating_sub(2).max(20) as usize;
            let frame = render_complex_frame(sc, mode, &cam, cols, rows, true);
            let status = format!(
                " {} · {} · {} остатков · лиганд {} атомов · карман {} · yaw {:+.0}° pitch {:+.0}° zoom {:.2} ",
                if sc.title.is_empty() { "комплекс" } else { sc.title.as_str() },
                mode.name_ru(),
                sc.n_residues,
                sc.n_ligand_atoms,
                sc.pocket_names.len(),
                cam.yaw.to_degrees(),
                cam.pitch.to_degrees(),
                cam.zoom
            );
            let help =
                " ←→↑↓/hjkl: вращение · +/-: зум · m: режим (трейс/карман/ВдВ) · r: сброс · q: выход ";
            let mut out = String::new();
            out.push_str("\x1b[H\x1b[2J");
            out.push_str(&format!("\x1b[1m\x1b[38;2;180;220;255m{status}\x1b[0m\n"));
            out.push_str(&frame);
            out.push_str(&format!("\x1b[38;2;140;140;160m{help}\x1b[0m\r"));
            print!("{out}");
            use std::io::Write;
            let _ = std::io::stdout().flush();

            let timeout = std::time::Duration::from_millis(33);
            match crossterm::event::poll(timeout) {
                Ok(true) => loop {
                    match crossterm::event::read() {
                        Ok(Event::Key(KeyEvent { code, modifiers, .. })) => {
                            match code {
                                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                                KeyCode::Char('c')
                                    if modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    return Ok(())
                                }
                                KeyCode::Left | KeyCode::Char('h') => cam.yaw -= 0.18,
                                KeyCode::Right | KeyCode::Char('l') => cam.yaw += 0.18,
                                KeyCode::Up | KeyCode::Char('k') => cam.pitch += 0.14,
                                KeyCode::Down | KeyCode::Char('j') => cam.pitch -= 0.14,
                                KeyCode::Char('+') | KeyCode::Char('=') => cam.zoom *= 1.15,
                                KeyCode::Char('-') | KeyCode::Char('_') => cam.zoom /= 1.15,
                                KeyCode::Char('m') => mode = mode.next(),
                                KeyCode::Char('r') => {
                                    cam = Camera {
                                        yaw: 0.7,
                                        pitch: 0.42,
                                        zoom: 1.0,
                                    };
                                }
                                _ => {}
                            }
                            if let Ok(true) =
                                crossterm::event::poll(std::time::Duration::from_millis(0))
                            {
                                continue;
                            }
                            break;
                        }
                        Ok(Event::Resize(nw, nh)) => {
                            w = nw;
                            h = nh;
                            break;
                        }
                        Ok(_) => break,
                        Err(e) => return Err(format!("событие: {e}")),
                    }
                },
                Ok(false) => {}
                Err(e) => return Err(format!("poll: {e}")),
            }
        }
    })();

    let _ = cleanup();
    result
}
