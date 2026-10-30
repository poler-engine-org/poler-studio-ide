//! R3: «Кадр из гамильтониана» — первый публичный артефакт
//! «изображение, просчитанное математически».
//!
//! Пайплайн (каждый шаг — честная математика, никакого ассет-пайплайна):
//!
//! ```text
//!   Гамильтониан Х  =  −J Σ ZᵢZᵢ₊₁  −  h Σ Xᵢ          (POLER calc, Rust)
//!          │
//!   эволюция |Ψ(t)⟩ = exp(−iHt)|Ψ₀⟩  (expm, шаг dt)     (POLER calc, Rust)
//!          │
//!   геометрия: worldlines ⟨Zᵢ⟩(t) + облако |⟨b|Ψ⟩|²     (R³-бокс → P³)
//!          │
//!   рендер: P³ → тройной буфер RGB+depth+seg            (P³ Engine, Zig)
//!          │
//!   артефакт: 3 PNG (rgb / depth / seg)                (POLER png, Rust)
//! ```
//!
//! Честность артефакта:
//! - энергия сохраняется до машинного ε (унитарная эволюция) — метрика
//!   `energy_drift` печатается рядом с кадром;
//! - глубина пикселя — метрика Фубини–Штуди d_FS(точка, глаз) ∈ [0, π/2];
//! - сегментация — идентификаторы кубитов/облака: по seg-буферу видно,
//!   какой объект записал пиксель;
//! - детерминированность: те же параметры → байт-в-байт те же PNG.

use std::path::PathBuf;
use std::time::Instant;

use super::ffi::Camera;
use crate::calc::matrix::Matrix;
use crate::calc::solve::Complex;

/// |c|² без лишнего корня (гипотенуза не нужна).
#[inline]
fn c_norm_sqr(c: Complex) -> f64 {
    c.re * c.re + c.im * c.im
}

/// Параметры кадра.
#[derive(Clone, Debug)]
pub struct FrameConfig {
    /// Число кубитов цепочки Изинга (2..=8; 2ⁿ состояний).
    pub n_qubits: usize,
    /// Число шагов эволюции (t_k = k·dt, dt = 0.08).
    pub steps: usize,
    /// Разрешение кадра.
    pub width: u32,
    pub height: u32,
    /// Связь J (ферромагнитная: −J·ZᵢZᵢ₊₁).
    pub jz: f64,
    /// Поперечное поле h.
    pub hx: f64,
    /// Топ-M базисных состояний облака амплитуд на шаг.
    pub cloud_top: usize,
    /// Камера P³.
    pub camera: Camera,
    /// Каталог для PNG.
    pub out_dir: PathBuf,
}

impl Default for FrameConfig {
    fn default() -> Self {
        FrameConfig {
            n_qubits: 6,
            steps: 64,
            width: 960,
            height: 540,
            jz: 1.0,
            hx: 0.9,
            cloud_top: 7,
            camera: Camera::default(),
            out_dir: PathBuf::from("."),
        }
    }
}

/// Итог рендера кадра.
#[derive(Clone, Debug)]
pub struct FrameOutput {
    pub rgb_png: PathBuf,
    pub depth_png: PathBuf,
    pub seg_png: PathBuf,
    pub n_points: usize,
    pub n_edges: usize,
    pub painted_px: usize,
    pub max_fs_depth: f64,
    /// Дрейф энергии ⟨Ψ|H|Ψ⟩ за всю эволюцию (унитарность).
    pub energy_drift: f64,
    /// Время всего пайплайна.
    pub elapsed_ms: f64,
}

// -----------------------------------------------------------------------------
// 1. Гамильтониан и эволюция (POLER calc)
// -----------------------------------------------------------------------------

/// Сигнатура спина: z_i(s) = +1, если бит i нулевой; −1 иначе.
#[inline]
fn zspin(state: usize, qubit: usize) -> f64 {
    if state & (1 << qubit) == 0 {
        1.0
    } else {
        -1.0
    }
}

/// H = −J Σ_{i} ZᵢZᵢ₊₁ − h Σ_i Xᵢ (цепочка, открытые границы).
fn ising_hamiltonian(n: usize, jz: f64, hx: f64) -> Matrix {
    let dim = 1usize << n;
    let mut h = Matrix::zeros(dim, dim);
    // Диагональ: −J·z_i·z_{i+1}
    for s in 0..dim {
        let mut e = 0.0;
        for i in 0..n.saturating_sub(1) {
            e -= jz * zspin(s, i) * zspin(s, i + 1);
        }
        h.set(s, s, Complex::new(e, 0.0));
    }
    // Внедиагональ: −h·Xᵢ (флип бита i)
    for s in 0..dim {
        for i in 0..n {
            let t = s ^ (1 << i);
            if t > s {
                h.set(t, s, Complex::new(-hx, 0.0));
                h.set(s, t, Complex::new(-hx, 0.0));
            }
        }
    }
    h
}

/// ⟨Ψ|A|Ψ⟩ для вещественно-симметричного A и комплексного Ψ.
fn expectation(a: &Matrix, psi: &Matrix) -> f64 {
    // A·Ψ → вектор; ⟨Ψ|·(AΨ) → скаляр
    let apsi = a.mul(psi).expect("размеры согласованы");
    let mut acc = Complex::new(0.0, 0.0);
    for k in 0..psi.rows {
        // сопряжение Ψ
        let c = psi.get(k, 0).conj();
        acc = acc + c * apsi.get(k, 0);
    }
    acc.re
}

/// ⟨Z_i⟩(Ψ).
fn z_expectation(psi: &Matrix, qubit: usize) -> f64 {
    let mut acc = 0.0;
    for s in 0..psi.rows {
        let p = c_norm_sqr(psi.get(s, 0));
        acc += zspin(s, qubit) * p;
    }
    acc
}

// -----------------------------------------------------------------------------
// 2. Геометрия: эволюция → точки P³
// -----------------------------------------------------------------------------

/// Сцена кадра: однородные точки + сегменты + рёбра полилиний.
struct Scene {
    pts: Vec<f64>,
    segs: Vec<u8>,
    pairs: Vec<u32>,
}

/// Согласовать диапазон [min, max] в [-0.85, 0.85].
fn normalize_axis(values: &mut [f64]) {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &v in values.iter() {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    let span = (hi - lo).max(1e-12);
    for v in values.iter_mut() {
        *v = -0.85 + 1.7 * (*v - lo) / span;
    }
}

fn build_scene(n: usize, cloud_top: usize, states: &[Matrix]) -> Scene {
    // --- Worldlines кубитов: (i, t_k, ⟨Z_i⟩) ---
    let t_total = states.len();
    let mut pts = Vec::with_capacity(t_total * (n + cloud_top + 3) * 4);
    let mut segs = Vec::new();
    let mut pairs = Vec::new();

    // Вертикальная ось — время; нормализуем t в [-0.85, 0.85]
    let mut times: Vec<f64> = (0..t_total).map(|k| k as f64).collect();
    normalize_axis(&mut times);

    // ⟨Z_i⟩ по всем шагам — для нормализации z-оси наблюдаемых
    let mut zvals: Vec<f64> = Vec::with_capacity(n * t_total);
    for psi in states {
        for i in 0..n {
            zvals.push(z_expectation(psi, i));
        }
    }
    normalize_axis(&mut zvals);

    let x_spacing = 1.7 / (n as f64).max(1.0);
    let x0 = -0.85 + x_spacing / 2.0;

    // Полилинии кубитов (сегменты 1..=n)
    let mut base_index = 0u32;
    for i in 0..n {
        let start = base_index;
        for k in 0..t_total {
            let z = zvals[k * n + i];
            pts.extend_from_slice(&[x0 + i as f64 * x_spacing, times[k], z, 1.0]);
            segs.push((i + 1) as u8);
            if k > 0 {
                pairs.push(base_index - 1);
                pairs.push(base_index);
            }
            base_index += 1;
        }
        debug_assert_eq!(base_index - start, t_total as u32);
    }

    // --- Облако амплитуд + занавес: топ-M базисных состояний на шаг ---
    // Точка: (позиция базиса b, t_k, p_b·масштаб). Соседние по b — ребро
    // (мгновенное распределение), та же b на соседнем t — ребро (поток
    // вероятности во времени) → сетка-«занавес» |⟨b|Ψ(t)⟩|².
    let dim = states[0].rows;
    let mut pmax = 0.0f64;
    for psi in states {
        for b in 0..dim {
            pmax = pmax.max(c_norm_sqr(psi.get(b, 0)));
        }
    }
    let pmax = pmax.max(1e-12);

    // Индекс точки облака по (шаг, базис) — для вертикальных нитей занавеса
    let mut cloud_index: Vec<std::collections::HashMap<usize, u32>> =
        Vec::with_capacity(states.len());
    for (k, psi) in states.iter().enumerate() {
        // Топ-M по вероятности (детерминированно: сортировка по убыванию,
        // при равенстве — по индексу)
        let mut idx: Vec<usize> = (0..dim).collect();
        idx.sort_by(|&a, &b| {
            c_norm_sqr(psi.get(b, 0))
                .partial_cmp(&c_norm_sqr(psi.get(a, 0)))
                .unwrap()
                .then(a.cmp(&b))
        });
        let selected: Vec<usize> = idx.iter().take(cloud_top).copied().collect();
        let mut row_map: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
        let row_start = base_index;
        for &b in &selected {
            let p = c_norm_sqr(psi.get(b, 0));
            // x — позиция базисного состояния (центрированная), z — p·масштаб
            let bx = -0.85 + 1.7 * (b as f64 + 0.5) / dim as f64;
            let z = -0.85 + 1.7 * (p / pmax).max(0.0);
            pts.extend_from_slice(&[bx, times[k], z, 1.0]);
            segs.push(8);
            row_map.insert(b, base_index);
            base_index += 1;
        }
        // Горизонтальные рёбра: мгновенная форма распределения — только
        // между СОСЕДНИМИ базисными состояниями (b, b+1): никаких
        // длинных «плавающих» хорды между далёкими компонентами
        let sorted_by_b = {
            let mut s = selected.clone();
            s.sort_unstable();
            s
        };
        for w in sorted_by_b.windows(2) {
            if w[1] - w[0] == 1 {
                if let (Some(&i0), Some(&i1)) = (row_map.get(&w[0]), row_map.get(&w[1])) {
                    pairs.push(i0);
                    pairs.push(i1);
                }
            }
        }
        let _ = row_start;
        // Вертикальные нити: та же b на предыдущем шаге — поток вероятности
        if k > 0 {
            if let Some(prev) = cloud_index.last() {
                for (&b, &cur) in row_map.iter() {
                    if let Some(&before) = prev.get(&b) {
                        pairs.push(before);
                        pairs.push(cur);
                    }
                }
            }
        }
        cloud_index.push(row_map);
    }

    // --- Осевой трипод + бокс данных (сегмент 9, сталь) ---
    // Трипод задаёт центр/ориентацию, бокс [-0.92, 0.92]³ — масштаб:
    // пространственная отсылка для чтения глубины кадра.
    let origin = base_index;
    for (x, y, z) in [
        (0.0f64, 0.0f64, 0.0f64),
        (0.92, 0.0, 0.0),
        (0.0, 0.92, 0.0),
        (0.0, 0.0, 0.92),
    ] {
        pts.extend_from_slice(&[x, y, z, 1.0]);
        segs.push(9);
        base_index += 1;
    }
    debug_assert_eq!(base_index as usize, pts.len() / 4);
    for axis_end in [origin + 1, origin + 2, origin + 3] {
        pairs.push(origin);
        pairs.push(axis_end);
    }

    // Бокс данных: 8 вершин ±0.92 + 12 рёбер (все с w = 1 < |coord| не
    // превышает единицу — афинная карта UW не переключается)
    let c = 0.92f64;
    let box_start = base_index;
    for &sx in &[-c, c] {
        for &sy in &[-c, c] {
            for &sz in &[-c, c] {
                pts.extend_from_slice(&[sx, sy, sz, 1.0]);
                segs.push(9);
            }
        }
    }
    // Индексы вершин: бит0 = x, бит1 = y, бит2 = z (порядок вложенных циклов)
    #[rustfmt::skip]
    let box_edges: [(u32, u32); 12] = [
        (0, 1), (2, 3), (4, 5), (6, 7), // вдоль x
        (0, 2), (1, 3), (4, 6), (5, 7), // вдоль y
        (0, 4), (1, 5), (2, 6), (3, 7), // вдоль z
    ];
    for (a, b) in box_edges {
        pairs.push(box_start + a);
        pairs.push(box_start + b);
    }

    Scene {
        pts,
        segs,
        pairs,
    }
}

// -----------------------------------------------------------------------------
// 3. Пайплайн кадра
// -----------------------------------------------------------------------------

/// Рассчитать и отрендерить кадр из гамильтониана. Возвращает метрики
/// честности (дрейф энергии) и пути к PNG.
pub fn render_hamiltonian_frame(cfg: &FrameConfig) -> Result<FrameOutput, String> {
    let t0 = Instant::now();
    if !(2..=8).contains(&cfg.n_qubits) {
        return Err(format!("n_qubits: 2..=8, получено {}", cfg.n_qubits));
    }
    if cfg.steps < 4 || cfg.steps > 512 {
        return Err(format!("steps: 4..=512, получено {}", cfg.steps));
    }
    if cfg.width < 64 || cfg.height < 64 || cfg.width > 4096 || cfg.height > 4096 {
        return Err("размер кадра: 64..=4096 по каждой стороне".into());
    }
    if cfg.cloud_top == 0 || cfg.cloud_top > 64 {
        return Err("cloud_top: 1..=64".into());
    }
    std::fs::create_dir_all(&cfg.out_dir)
        .map_err(|e| format!("out_dir {}: {e}", cfg.out_dir.display()))?;

    // --- 1. Гамильтониан и начальное состояние |0…0⟩ ---
    let h = ising_hamiltonian(cfg.n_qubits, cfg.jz, cfg.hx);
    let dim = 1usize << cfg.n_qubits;
    let mut psi = Matrix::zeros(dim, 1);
    psi.set(0, 0, Complex::ONE);

    let e0 = expectation(&h, &psi);

    // --- 2. Эволюция: U_dt = expm(−i·H·dt), ψ ← U·ψ ---
    let dt = 0.08f64;
    let u_dt = h
        .scale_c(Complex::new(0.0, -dt))
        .expm()
        .map_err(|e| format!("expm: {e}"))?;

    let mut states: Vec<Matrix> = Vec::with_capacity(cfg.steps + 1);
    states.push(psi.clone());
    for _ in 0..cfg.steps {
        psi = u_dt.mul(&psi).map_err(|e| format!("evolve: {e}"))?;
        // Ренормализация не нужна: expm унитарен с точностью ε; но следим
        // за нормой как метрикой честности (без модификации вектора).
        states.push(psi.clone());
    }
    let e_final = expectation(&h, states.last().unwrap());
    let norm_final = states
        .last()
        .unwrap()
        .data
        .iter()
        .map(|c| c_norm_sqr(*c))
        .sum::<f64>();
    let energy_drift = (e_final - e0).abs().max((norm_final - 1.0).abs());

    // --- 3. Сцена P³ ---
    let scene = build_scene(cfg.n_qubits, cfg.cloud_top, &states);
    let n_points = scene.segs.len();

    // --- 4. Рендер: FFI-ядро Zig → (фолбэк) Rust-близнец ---
    let npix = cfg.width as usize * cfg.height as usize;
    let mut rgb = vec![0u8; npix * 3];
    let mut depth = vec![0f32; npix];
    let mut seg = vec![0u8; npix];
    crate::p3::ffi::render_frame_auto(
        &scene.pts,
        &scene.segs,
        &scene.pairs,
        cfg.width,
        cfg.height,
        cfg.camera,
        2,
        &mut rgb,
        &mut depth,
        &mut seg,
    )?;

    let painted_px = seg.iter().filter(|&&s| s != 0).count();
    let max_fs_depth = depth
        .iter()
        .filter(|&&d| d < 1e29)
        .cloned()
        .fold(0.0f32, f32::max) as f64;
    if painted_px < 32 {
        return Err(format!(
            "рендер пуст: {painted_px} пикселей — проверьте камеру/сцену"
        ));
    }

    // --- 4.5 Аннотация RGB-кадра: заголовок + легенда (только RGB;
    // depth/seg остаются чистыми научными буферами) ---
    annotate_rgb(
        &mut rgb,
        cfg.width,
        cfg.height,
        cfg.n_qubits,
        cfg.jz,
        cfg.hx,
        energy_drift,
        max_fs_depth,
    );

    // --- 5. Артефакты: тройной буфер → PNG ---
    let stem = format!(
        "p3_frame_n{}_t{}_j{}_h{}",
        cfg.n_qubits, cfg.steps, cfg.jz, cfg.hx
    );
    let rgb_png = cfg.out_dir.join(format!("{stem}_rgb.png"));
    let depth_png = cfg.out_dir.join(format!("{stem}_depth.png"));
    let seg_png = cfg.out_dir.join(format!("{stem}_seg.png"));

    // depth: d_FS ∈ [0, π/2] → grayscale (близко = светло)
    let mut gray = vec![0u8; npix];
    let scale = 255.0 / (std::f64::consts::FRAC_PI_2);
    for (g, &d) in gray.iter_mut().zip(depth.iter()) {
        *g = if d >= 1e29 {
            0 // фон
        } else {
            let v = 255.0 - (d as f64) * scale * 0.9;
            v.clamp(6.0, 255.0) as u8
        };
    }
    // seg: идентификатор → палитра (зеркально PALETTE Zig-рендера)
    let seg_palette: [[u8; 3]; 10] = [
        [8, 8, 14],
        [96, 202, 252],
        [252, 128, 144],
        [148, 252, 128],
        [252, 214, 96],
        [186, 148, 252],
        [128, 252, 214],
        [252, 168, 96],
        [172, 176, 190],
        [140, 180, 255],
    ];
    let mut seg_rgb = vec![0u8; npix * 3];
    for (dst, &s) in seg_rgb.chunks_exact_mut(3).zip(seg.iter()) {
        let c = seg_palette[(s as usize) % seg_palette.len()];
        dst.copy_from_slice(&c);
    }

    super::png::encode_rgb(&rgb_png, cfg.width, cfg.height, &rgb)
        .map_err(|e| format!("PNG rgb: {e}"))?;
    super::png::encode_gray(&depth_png, cfg.width, cfg.height, &gray)
        .map_err(|e| format!("PNG depth: {e}"))?;
    super::png::encode_rgb(&seg_png, cfg.width, cfg.height, &seg_rgb)
        .map_err(|e| format!("PNG seg: {e}"))?;

    Ok(FrameOutput {
        rgb_png,
        depth_png,
        seg_png,
        n_points,
        n_edges: scene.pairs.len() / 2,
        painted_px,
        max_fs_depth,
        energy_drift,
        elapsed_ms: t0.elapsed().as_secs_f64() * 1000.0,
    })
}

// -----------------------------------------------------------------------------
// 4. Аннотация RGB-кадра: растровый шрифт 5×7 + легенда
// -----------------------------------------------------------------------------

/// Классический 5×7 растровый шрифт (7 строк по 5 бит, MSB слева).
/// Только то, что нужно для заголовка/легенды.
pub(crate) fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x04, 0x02, 0x08, 0x10, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '=' => [0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x0C, 0x04, 0x08],
        '|' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        '[' => [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E],
        ']' => [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        '^' => [0x04, 0x0A, 0x11, 0x00, 0x00, 0x00, 0x00],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '<' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        '>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        _ => [0x00; 7], // пробел и неизвестное
    }
}

/// Нарисовать текст в RGB-буфер (масштаб 1 = 5×7 + межбуквенный пиксель).
pub(crate) fn draw_text(
    rgb: &mut [u8],
    width: u32,
    x0: usize,
    y0: usize,
    text: &str,
    color: [u8; 3],
    scale: usize,
) {
    let mut cx = x0;
    for ch in text.chars() {
        let g = glyph(ch);
        for (gy, row) in g.iter().enumerate() {
            for gx in 0..5usize {
                if row & (1 << (4 - gx)) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = cx + gx * scale + sx;
                            let py = y0 + gy * scale + sy;
                            if px < width as usize {
                                let idx = py * width as usize * 3 + px * 3;
                                if idx + 2 < rgb.len() {
                                    rgb[idx] = color[0];
                                    rgb[idx + 1] = color[1];
                                    rgb[idx + 2] = color[2];
                                }
                            }
                        }
                    }
                }
            }
        }
        cx += 6 * scale;
        if cx > width as usize {
            break;
        }
    }
}

/// Свотч легенды: цветной квадратик.
pub(crate) fn draw_swatch(rgb: &mut [u8], width: u32, x0: usize, y0: usize, color: [u8; 3], size: usize) {
    for dy in 0..size {
        for dx in 0..size {
            let px = x0 + dx;
            let py = y0 + dy;
            let idx = py * width as usize * 3 + px * 3;
            if idx + 2 < rgb.len() {
                rgb[idx] = color[0];
                rgb[idx + 1] = color[1];
                rgb[idx + 2] = color[2];
            }
        }
    }
}

/// Заголовок + легенда поверх RGB-кадра (пост-процесс, depth/seg чисты).
#[allow(clippy::too_many_arguments)]
fn annotate_rgb(
    rgb: &mut [u8],
    width: u32,
    height: u32,
    n: usize,
    jz: f64,
    hx: f64,
    energy_drift: f64,
    max_fs_depth: f64,
) {
    let white = [235u8, 240, 248];
    let dim = [150u8, 158, 172];
    let (m, ts) = (16usize, 2usize); // отступ и масштаб заголовка

    // Полупрозрачная плашка под текст (тёмная, чтобы текст читался поверх)
    let panel_h = m + 7 * ts + 10 + 7 + 14;
    for y in 0..panel_h.min(height as usize) {
        for x in 0..(width as usize).min(900) {
            let idx = y * width as usize * 3 + x * 3;
            if idx + 2 < rgb.len() {
                rgb[idx] = (rgb[idx] / 4).max(4);
                rgb[idx + 1] = (rgb[idx + 1] / 4).max(5);
                rgb[idx + 2] = (rgb[idx + 2] / 4).max(8);
            }
        }
    }

    // Заголовок
    draw_text(
        rgb,
        width,
        m,
        m,
        &format!("POLER ENGINE X P3 | ISING N={n} J={jz:.1} H={hx:.1} | HAMILTONIAN FRAME"),
        white,
        ts,
    );

    // Легенда: worldlines кубитов + облако + бокс
    let worldline_colors: [[u8; 3]; 7] = [
        [96, 202, 252],
        [252, 128, 144],
        [148, 252, 128],
        [252, 214, 96],
        [186, 148, 252],
        [128, 252, 214],
        [172, 176, 190],
    ];
    let ly = m + 7 * ts + 10;
    let mut lx = m;
    for (i, col) in worldline_colors.iter().enumerate() {
        draw_swatch(rgb, width, lx, ly + 1, *col, 7);
        let label = if i < 6 { format!("Q{i} ") } else { "PSI^2 ".into() };
        draw_text(rgb, width, lx + 11, ly, &label, dim, 1);
        lx += 11 + 6 * label.chars().count() + 8;
    }
    draw_swatch(rgb, width, lx, ly + 1, [140, 180, 255], 7);
    draw_text(rgb, width, lx + 11, ly, "DATA BOX", dim, 1);

    // Строка честности
    let hy = ly + 12;
    draw_text(
        rgb,
        width,
        m,
        hy,
        &format!(
            "DEPTH=D_FS IN [0,PI/2] MAX={max_fs_depth:.3} | E-DRIFT={energy_drift:.1E} | DETERMINISTIC"
        ),
        dim,
        1,
    );
}

// =============================================================================
// ТЕСТЫ
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p3::ffi::P3Lib;

    #[test]
    fn ising_ground_state_energy_two_qubits() {
        // n=2, J=1, h=0: H = −Z₀Z₁ → основное состояние |00⟩, E₀ = −1
        let h = ising_hamiltonian(2, 1.0, 0.0);
        let mut psi = Matrix::zeros(4, 1);
        psi.set(0, 0, Complex::ONE);
        let e = expectation(&h, &psi);
        assert!((e - (-1.0)).abs() < 1e-12, "E = {e}");
    }

    #[test]
    fn evolution_is_unitary() {
        // ⟨Ψ|H|Ψ⟩ сохраняется с точностью expm
        let h = ising_hamiltonian(4, 1.0, 0.7);
        let mut psi = Matrix::zeros(16, 1);
        psi.set(0, 0, Complex::ONE);
        let e0 = expectation(&h, &psi);
        let u = h.scale_c(Complex::new(0.0, -0.08)).expm().unwrap();
        for _ in 0..32 {
            psi = u.mul(&psi).unwrap();
        }
        let e1 = expectation(&h, &psi);
        assert!((e1 - e0).abs() < 1e-12, "drift = {}", (e1 - e0).abs());
    }

    #[test]
    fn frame_pipeline_small() {
        // Полный пайплайн на маленьком кадре → temp-каталог
        let dir = std::env::temp_dir().join("poler_p3_frame_test");
        let cfg = FrameConfig {
            n_qubits: 4,
            steps: 8,
            width: 160,
            height: 120,
            cloud_top: 5,
            out_dir: dir,
            ..Default::default()
        };
        let out = render_hamiltonian_frame(&cfg).expect("пайплайн кадра");
        assert!(out.painted_px > 32);
        assert!(out.max_fs_depth <= std::f64::consts::FRAC_PI_2 + 1e-4);
        assert!(out.energy_drift < 1e-10, "drift = {}", out.energy_drift);
        assert!(out.rgb_png.exists() && out.depth_png.exists() && out.seg_png.exists());
        // Все три PNG валидны (round-trip собственного парсера)
        for p in [&out.rgb_png, &out.depth_png, &out.seg_png] {
            let bytes = std::fs::read(p).unwrap();
            let (w, h, _ct, px) = super::super::png::decode_own(&bytes).unwrap();
            assert_eq!((w, h), (160, 120));
            let channels = if p.to_string_lossy().contains("_depth") { 1 } else { 3 };
            assert_eq!(px.len(), (w * h) as usize * channels);
        }
    }

    #[test]
    fn scene_segments_covered_by_render() {
        // Та же сцена, отрендеренная напрямую: в seg-буфере обязаны быть
        // и worldlines кубитов (1..=n), и облако (8), и трипод (9).
        let n = 4usize;
        let dim = 1 << n;
        let h = ising_hamiltonian(n, 1.0, 0.7);
        let mut psi = Matrix::zeros(dim, 1);
        psi.set(0, 0, Complex::ONE);
        let u = h.scale_c(Complex::new(0.0, -0.08)).expm().unwrap();
        let mut states = vec![psi.clone()];
        for _ in 0..8 {
            psi = u.mul(&psi).unwrap();
            states.push(psi.clone());
        }
        let scene = build_scene(n, 5, &states);
        assert_eq!(scene.pts.len(), scene.segs.len() * 4);
        assert!(scene.pairs.len() >= 8, "рёбра worldlines+трипод");

        let lib = P3Lib::open().expect("libp3ffi");
        let (w, hh) = (200u32, 150u32);
        let npix = (w * hh) as usize;
        let mut rgb = vec![0u8; npix * 3];
        let mut depth = vec![0f32; npix];
        let mut seg = vec![0u8; npix];
        lib.render_frame(&scene.pts, &scene.segs, &scene.pairs, w, hh, Default::default(), 2, &mut rgb, &mut depth, &mut seg)
            .unwrap();
        let kinds: std::collections::HashSet<u8> =
            seg.iter().copied().filter(|&s| s != 0).collect();
        for expected in 1..=(n as u8) {
            assert!(kinds.contains(&expected), "нет worldline {expected}");
        }
        assert!(kinds.contains(&8), "нет облака амплитуд");
        assert!(kinds.contains(&9), "нет трипода");
        // Глубина — FS-метрика в пределах [0, π/2]
        for &d in depth.iter() {
            if d < 1e29 {
                assert!((0.0..=std::f64::consts::FRAC_PI_2 as f32 + 1e-4).contains(&d));
            }
        }
    }

    #[test]
    fn frame_is_deterministic() {
        // Тот же конфиг → байт-в-байт те же PNG
        let dir = std::env::temp_dir().join("poler_p3_frame_det");
        let cfg = FrameConfig {
            n_qubits: 3,
            steps: 6,
            width: 96,
            height: 72,
            cloud_top: 4,
            out_dir: dir.clone(),
            ..Default::default()
        };
        let a = render_hamiltonian_frame(&cfg).unwrap();
        let b = render_hamiltonian_frame(&cfg).unwrap();
        let fa = std::fs::read(&a.rgb_png).unwrap();
        let fb = std::fs::read(&b.rgb_png).unwrap();
        assert_eq!(fa, fb, "рендер обязан быть детерминированным");
    }
}
