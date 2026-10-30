//! # T2: Процедурные текстуры — спектральный синтез (цикл T, v0.55.0)
//!
//! Текстура как **функция**, а не битмап: `TextureSpec::sample(u, v)`
//! аналитически вычисляется в любой точке — бесконечный зум без
//! пикселизации, детерминизм бит-в-бит (одно целочисленное зерно,
//! никакого системного времени и никаких hash-map с рандомным порядком).
//!
//! Математика:
//! - **value noise** на целочисленной решётке (splitmix64-перемешивание
//!   координат + билинейная интерполяция с quintic-сглаживанием);
//! - **fBm** — сумма октав (частота ×2, амплитуда ×gain) — фрактальная
//!   размерность управляется числом октав;
//! - **domain warp** — искривление входных координат шумом: мрамор
//!   и дерево получаются из одного и того же шума разными проекциями;
//! - **SVD rank-k кодек** — тайл 16×16 раскладывается односторонним
//!   вращением Якоби (Hestenes): A = U·Σ·Vᵀ, держим топ-k сингулярных
//!   троек. Кривая качество/ранг — метрика PSNR.
//!
//! Тот же принцип, что у акустического кристалла T1: сигнал — это
//! низкоранговый спектральный объект, а не поток байт.

use std::path::Path;

// ---------------------------------------------------------------------------
// Value noise + fBm
// ---------------------------------------------------------------------------

/// Целочисленный хеш решётки → [0, 1). Чистая арифметика u64:
/// бит-в-бит одинакова на любой платформе (не зависит от порядка
/// обхода, от аллокатора, от чего бы то ни было).
pub fn hash_lattice(ix: i64, iy: i64, seed: u64) -> f64 {
    let mut h = (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (iy as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ seed.wrapping_mul(0x1656_67B1_9E37_79F9);
    h = h.wrapping_add(0x9E37_79B9_7F4A_7C15);
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// Quintic-сглаживание (Perlin): C²-непрерывность на стыках ячеек.
fn smooth(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Value noise в точке (x, y): билинейная интерполяция четырёх
/// углов решётки со сглаживанием.
pub fn value_noise(x: f64, y: f64, seed: u64) -> f64 {
    let ix = x.floor();
    let iy = y.floor();
    let fx = smooth(x - ix);
    let fy = smooth(y - iy);
    let c00 = hash_lattice(ix as i64, iy as i64, seed);
    let c10 = hash_lattice(ix as i64 + 1, iy as i64, seed);
    let c01 = hash_lattice(ix as i64, iy as i64 + 1, seed);
    let c11 = hash_lattice(ix as i64 + 1, iy as i64 + 1, seed);
    let a = c00 + (c10 - c00) * fx;
    let b = c01 + (c11 - c01) * fx;
    a + (b - a) * fy
}

/// Фрактальный шум (fBm): `octaves` октав, лакунарность 2, persistence
/// `gain`. Результат нормирован в [0, 1].
pub fn fbm(x: f64, y: f64, seed: u64, octaves: u32, gain: f64) -> f64 {
    let octaves = octaves.clamp(1, 12);
    let g = gain.clamp(0.05, 0.95);
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut norm = 0.0;
    let (mut fx, mut fy) = (x, y);
    for _ in 0..octaves {
        sum += amp * value_noise(fx, fy, seed);
        norm += amp;
        amp *= g;
        fx *= 2.0;
        fy *= 2.0;
    }
    (sum / norm.max(1e-12)).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Текстура как функция: стили
// ---------------------------------------------------------------------------

/// Стиль материала: один и тот же fBm, разные проекции.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TexStyle {
    /// Чистый фрактал.
    Noise,
    /// Мрамор: синус, искривлённый шумом (домен-варп).
    Marble,
    /// Дерево: кольца с турбулентностью.
    Wood,
}

impl TexStyle {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "noise" => Some(TexStyle::Noise),
            "marble" => Some(TexStyle::Marble),
            "wood" => Some(TexStyle::Wood),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TexStyle::Noise => "noise",
            TexStyle::Marble => "marble",
            TexStyle::Wood => "wood",
        }
    }
}

/// Палитра для RGB-рендера (3 опорные точки).
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub lo: [u8; 3],
    pub mid: [u8; 3],
    pub hi: [u8; 3],
}

impl Palette {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "gray" => Palette {
                lo: [12, 12, 16],
                mid: [128, 128, 132],
                hi: [244, 244, 246],
            },
            "copper" => Palette {
                lo: [24, 12, 8],
                mid: [176, 92, 44],
                hi: [255, 208, 160],
            },
            "ice" => Palette {
                lo: [6, 14, 30],
                mid: [56, 130, 190],
                hi: [214, 240, 255],
            },
            "jade" => Palette {
                lo: [8, 24, 16],
                mid: [40, 150, 96],
                hi: [190, 240, 200],
            },
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        "gray|copper|ice|jade"
    }

    /// Отображение t ∈ [0,1] в RGB (два линейных участка).
    pub fn map(&self, t: f64) -> [u8; 3] {
        let t = t.clamp(0.0, 1.0);
        let (a, b, k) = if t < 0.5 { (self.lo, self.mid, t * 2.0) } else { (self.mid, self.hi, t * 2.0 - 1.0) };
        [
            (a[0] as f64 + (b[0] as f64 - a[0] as f64) * k).round() as u8,
            (a[1] as f64 + (b[1] as f64 - a[1] as f64) * k).round() as u8,
            (a[2] as f64 + (b[2] as f64 - a[2] as f64) * k).round() as u8,
        ]
    }
}

/// Спецификация текстуры — чистая функция от (u, v) ∈ [0,1]².
#[derive(Debug, Clone)]
pub struct TextureSpec {
    pub style: TexStyle,
    pub seed: u64,
    /// Базовая частота (число ячеек шума на тайл).
    pub freq: f64,
    /// Октавы fBm.
    pub octaves: u32,
    /// Persistence.
    pub gain: f64,
    /// Сила искривления домена.
    pub warp: f64,
    /// Число колец для marble/wood.
    pub rings: f64,
    /// Контраст вокруг середины (1.0 = нет; >1 — резче переходы).
    pub contrast: f64,
}

impl Default for TextureSpec {
    fn default() -> Self {
        Self {
            style: TexStyle::Marble,
            seed: 7,
            freq: 4.0,
            octaves: 5,
            gain: 0.5,
            warp: 1.2,
            rings: 6.0,
            contrast: 1.0,
        }
    }
}

impl TextureSpec {
    /// Аналитический сэмпл в точке (u, v) ∈ [0,1]² → [0,1].
    /// Зум учитывается вызывающим: sample(u/zoom, v/zoom).
    pub fn sample(&self, u: f64, v: f64) -> f64 {
        let x = u * self.freq;
        let y = v * self.freq;
        match self.style {
            TexStyle::Noise => fbm(x, y, self.seed, self.octaves, self.gain),
            TexStyle::Marble => {
                // Домен-варп: координата дышит шумом
                let wx = fbm(x + 5.2, y + 1.3, self.seed ^ 0xA5A5, self.octaves, self.gain) - 0.5;
                let wy = fbm(x + 1.7, y + 9.2, self.seed ^ 0x5A5A, self.octaves, self.gain) - 0.5;
                let phase = (x + self.warp * wx * 3.0) / self.freq.max(1e-9);
                let s = (phase * self.rings * std::f64::consts::TAU).sin();
                let veins = 0.5 + 0.5 * s;
                // Модуляция плотности шумом
                let d = fbm(x + self.warp * wy * 2.0, y, self.seed, self.octaves, self.gain);
                (veins * 0.72 + d * 0.28).clamp(0.0, 1.0)
            }
            TexStyle::Wood => {
                let turb = fbm(x, y, self.seed ^ 0x1234, self.octaves, self.gain) - 0.5;
                let r = ((u - 0.5) * (u - 0.5) + (v - 0.5) * (v - 0.5)).sqrt();
                let rings = (r + turb * self.warp * 0.22) * self.rings;
                let grain = rings - rings.floor();
                // Волокна: выраженная модуляция вдоль одной оси
                let fiber = fbm(x * 0.5, y * 3.0, self.seed ^ 0xBEEF, 3, 0.6);
                (grain * 0.7 + fiber * 0.3).clamp(0.0, 1.0)
            }
        }
    }

    /// Растер в WxH. `zoom` > 1 — входим глубже в функцию (тайл
    /// бесконечен), пикселизации не будет — просто честный ресэмпл.
    pub fn render(&self, w: u32, h: u32, zoom: f64, pal: &Palette) -> Texture {
        let zoom = zoom.max(1e-6);
        let contrast = self.contrast.clamp(0.25, 4.0);
        let mut rgb = Vec::with_capacity((w as usize) * (h as usize) * 3);
        for py in 0..h {
            // Пиксель-центры, а не углы — корректный ресэмпл
            let v = (py as f64 + 0.5) / h as f64 / zoom;
            for px in 0..w {
                let u = (px as f64 + 0.5) / w as f64 / zoom;
                let t = self.sample(u, v);
                // Контраст вокруг середины — «криспер» без потери
                // детерминизма (чистая арифметика).
                let tc = ((t - 0.5) * contrast + 0.5).clamp(0.0, 1.0);
                let c = pal.map(tc);
                rgb.extend_from_slice(&c);
            }
        }
        Texture { w, h, rgb }
    }
}

// ---------------------------------------------------------------------------
// Растер: PNG + хеш детерминизма
// ---------------------------------------------------------------------------

/// Готовая текстура (RGB, 8 бит/канал).
#[derive(Debug, Clone)]
pub struct Texture {
    pub w: u32,
    pub h: u32,
    pub rgb: Vec<u8>,
}

impl Texture {
    /// PNG через суверенный энкодер P³ (тот же, что у рендера мира).
    pub fn write_png(&self, path: &Path) -> std::io::Result<()> {
        crate::p3::png::encode_rgb(path, self.w, self.h, &self.rgb)
    }

    /// Яркостной канал (Rec. 601) — вход SVD-кодека.
    pub fn gray(&self) -> Vec<u8> {
        self.rgb
            .chunks_exact(3)
            .map(|c| {
                let y = 0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64;
                y.round().clamp(0.0, 255.0) as u8
            })
            .collect()
    }

    /// Хеш детерминизма: FNV-1a по текселам.
    pub fn texture_hash(&self) -> u64 {
        let mut hsh: u64 = 0xcbf2_9ce4_8422_2325;
        for b in &self.rgb {
            hsh ^= *b as u64;
            hsh = hsh.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hsh
    }
}

// ---------------------------------------------------------------------------
// SVD rank-k кодек (одностороннее вращение Якоби–Хестенеса)
// ---------------------------------------------------------------------------

/// Односторонний SVD матрицы `a` (m×n, row-major): A = U·Σ·Vᵀ.
/// Возвращает (U m×n, σ n, V n×n). Столбцы попарно ортогонализуются
/// ротациями Гивенса до сходимости; нормы столбцов = сингулярные числа.
pub fn jacobi_svd(a: &[f64], m: usize, n: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    debug_assert_eq!(a.len(), m * n);
    let mut b = a.to_vec(); // рабочая копия: столбцы постепенно становятся U·Σ
    let mut v = vec![0.0f64; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }
    let eps = 1e-13;
    for _sweep in 0..16 {
        let mut off = 0.0f64;
        for p in 0..n.saturating_sub(1) {
            for q in (p + 1)..n {
                let mut ap = 0.0;
                let mut aq = 0.0;
                let mut gpq = 0.0;
                for i in 0..m {
                    let x = b[i * n + p];
                    let y = b[i * n + q];
                    ap += x * x;
                    aq += y * y;
                    gpq += x * y;
                }
                if ap < eps || aq < eps {
                    continue;
                }
                if gpq.abs() <= eps * (ap * aq).sqrt() {
                    continue;
                }
                off += gpq * gpq;
                // Угол ротации столбцов p,q
                let zeta = (aq - ap) / (2.0 * gpq);
                let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                let c = (1.0 + t * t).sqrt().recip();
                let s = c * t;
                for i in 0..m {
                    let x = b[i * n + p];
                    let y = b[i * n + q];
                    b[i * n + p] = c * x - s * y;
                    b[i * n + q] = s * x + c * y;
                }
                for i in 0..n {
                    let x = v[i * n + p];
                    let y = v[i * n + q];
                    v[i * n + p] = c * x - s * y;
                    v[i * n + q] = s * x + c * y;
                }
            }
        }
        if off < 1e-20 {
            break;
        }
    }
    // Сингулярные числа = нормы столбцов b; U = нормированные столбцы.
    let mut sigma = vec![0.0f64; n];
    let mut u = vec![0.0f64; m * n];
    for j in 0..n {
        let mut norm = 0.0;
        for i in 0..m {
            norm += b[i * n + j] * b[i * n + j];
        }
        norm = norm.sqrt();
        sigma[j] = norm;
        if norm > 1e-15 {
            for i in 0..m {
                u[i * n + j] = b[i * n + j] / norm;
            }
        }
    }
    // Контракт SVD: σ по убыванию, колонки U и V переставлены вслед.
    let order = sigma_order(&sigma);
    let mut su = vec![0.0f64; m * n];
    let mut sv = vec![0.0f64; n * n];
    let mut ssigma = vec![0.0f64; n];
    for (nj, &j) in order.iter().enumerate() {
        ssigma[nj] = sigma[j];
        for i in 0..m {
            su[i * n + nj] = u[i * n + j];
        }
        for i in 0..n {
            sv[i * n + nj] = v[i * n + j];
        }
    }
    (su, ssigma, sv)
}

/// Сортировка троек по убыванию σ (возвращает перестановки).
fn sigma_order(sigma: &[f64]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..sigma.len()).collect();
    idx.sort_by(|&a, &b| sigma[b].partial_cmp(&sigma[a]).unwrap_or(std::cmp::Ordering::Equal));
    idx
}

/// Реконструкция ранга k: A ≈ Σ_{j≤k} σ_j·u_j·v_jᵀ.
pub fn svd_rank_reconstruct(
    u: &[f64],
    sigma: &[f64],
    v: &[f64],
    m: usize,
    n: usize,
    k: usize,
) -> Vec<f64> {
    let order = sigma_order(sigma);
    let k = k.min(n);
    let mut out = vec![0.0f64; m * n];
    for &j in order.iter().take(k) {
        let s = sigma[j];
        if s <= 1e-15 {
            continue;
        }
        for i in 0..m {
            let ui = u[i * n + j];
            if ui == 0.0 {
                continue;
            }
            for c in 0..n {
                out[i * n + c] += s * ui * v[c * n + j];
            }
        }
    }
    out
}

/// Статистика кодека одного тайла/текстуры.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct CodecStats {
    pub psnr_db: f64,
    pub rmse: f64,
    pub rank: usize,
    pub full_rank: usize,
    /// Доля сохранённых компонент (k / full).
    pub kept_ratio: f64,
}

/// PSNR между оригиналом и реконструкцией (0..255).
pub fn psnr(orig: &[u8], recon: &[f64]) -> f64 {
    let n = orig.len().min(recon.len());
    let mut mse = 0.0f64;
    for i in 0..n {
        let d = orig[i] as f64 - recon[i].clamp(0.0, 255.0);
        mse += d * d;
    }
    mse /= n.max(1) as f64;
    if mse <= 1e-12 {
        120.0
    } else {
        10.0 * (255.0 * 255.0 / mse).log10()
    }
}

/// Тайловый SVD rank-k кодек: текстура (серый) разбивается на тайлы
/// `tile`×`tile`, каждый сжимается до ранга k. Возвращает реконструкцию
/// и сводную статистику. Это текстурный аналог акустического кристалла:
/// «сколько ранга нужно, чтобы материал остался собой».
pub fn svd_encode_gray(
    gray: &[u8],
    w: usize,
    h: usize,
    tile: usize,
    rank: usize,
) -> (Vec<u8>, CodecStats) {
    assert_eq!(gray.len(), w * h);
    let tile = tile.max(4);
    let mut out = vec![0u8; w * h];
    let mut psnr_sum = 0.0;
    let mut rmse_sum = 0.0;
    let mut tiles = 0usize;
    for ty in (0..h).step_by(tile) {
        for tx in (0..w).step_by(tile) {
            let th = (h - ty).min(tile);
            let tw = (w - tx).min(tile);
            // Тайл → матрица th×tw (row-major, f64)
            let mut a = vec![0.0f64; th * tw];
            for dy in 0..th {
                for dx in 0..tw {
                    a[dy * tw + dx] = gray[(ty + dy) * w + (tx + dx)] as f64;
                }
            }
            let (u, sigma, v) = jacobi_svd(&a, th, tw);
            let recon = svd_rank_reconstruct(&u, &sigma, &v, th, tw, rank);
            let orig_tile: Vec<u8> = (0..th * tw).map(|i| a[i] as u8).collect();
            psnr_sum += psnr(&orig_tile, &recon);
            let mut mse = 0.0;
            for i in 0..th * tw {
                let d = a[i] - recon[i].clamp(0.0, 255.0);
                mse += d * d;
            }
            rmse_sum += (mse / (th * tw).max(1) as f64).sqrt();
            for dy in 0..th {
                for dx in 0..tw {
                    out[(ty + dy) * w + (tx + dx)] = recon[dy * tw + dx].clamp(0.0, 255.0).round() as u8;
                }
            }
            tiles += 1;
        }
    }
    let t = tiles.max(1) as f64;
    let full = tile.min(w.max(h));
    (
        out,
        CodecStats {
            psnr_db: psnr_sum / t,
            rmse: rmse_sum / t,
            rank: rank.min(full),
            full_rank: full,
            kept_ratio: rank.min(full) as f64 / full as f64,
        },
    )
}

/// Кривая качество/ранг: PSNR для списка рангов.
pub fn rank_curve(gray: &[u8], w: usize, h: usize, tile: usize, ranks: &[usize]) -> Vec<CodecStats> {
    ranks.iter().map(|&k| svd_encode_gray(gray, w, h, tile, k).1).collect()
}

// ---------------------------------------------------------------------------
// U0: Normal maps — рельеф из той же аналитической функции (цикл U)
// ---------------------------------------------------------------------------

/// Нормаль касательного пространства из height-функции в точке (u, v).
///
/// Честная производная центральными разностями: dh/du ≈ (h⁺−h⁻)/(2·du)
/// — оценка **не зависит от разрешения** (du→0 сходится к h′(u)).
/// Рельеф — физическая высота: амплитуда `amplitude` — доля от размера
/// тайла (0.08 = «8% глубины»), поэтому зум и разрешение меняют картинку
/// только за счёт новых деталей функции, а не за счёт пересчёта наклона.
///
/// Плоская высота даёт чистый Z (128, 128, 255 в 8-битном коде).
pub fn height_normal(
    f: impl Fn(f64, f64) -> f64,
    u: f64,
    v: f64,
    du: f64,
    dv: f64,
    amplitude: f64,
) -> [f64; 3] {
    let dhx = (f(u + du, v) - f(u - du, v)) / (2.0 * du.max(1e-12));
    let dhy = (f(u, v + dv) - f(u, v - dv)) / (2.0 * dv.max(1e-12));
    let mut n = [-dhx * amplitude, -dhy * amplitude, 1.0];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len > 1e-15 {
        for x in &mut n {
            *x /= len;
        }
    }
    n
}

impl TextureSpec {
    /// Нормаль из **собственной** height-функции (тот же `sample`,
    /// что красит цвет) в точке (u, v) при шаге численной производной
    /// (du, dv).
    pub fn normal_at(&self, u: f64, v: f64, du: f64, dv: f64, amplitude: f64) -> [f64; 3] {
        height_normal(|x, y| self.sample(x, y), u, v, du, dv, amplitude)
    }

    /// Normal map WxH: RGB = n·0.5+0.5. Бесконечный зум — тот же
    /// принцип, что у [`TextureSpec::render`]: честный ресэмпл функции.
    /// `amplitude` — глубина рельефа в долях тайла (0.05..0.3 — рабочие
    /// значения для marble/wood/noise).
    pub fn render_normal(&self, w: u32, h: u32, zoom: f64, amplitude: f64) -> Texture {
        let zoom = zoom.max(1e-6);
        let amplitude = amplitude.clamp(0.0, 1.5);
        let mut rgb = Vec::with_capacity((w as usize) * (h as usize) * 3);
        for py in 0..h {
            let v = (py as f64 + 0.5) / h as f64 / zoom;
            let dv = 1.0 / h as f64 / zoom;
            for px in 0..w {
                let u = (px as f64 + 0.5) / w as f64 / zoom;
                let du = 1.0 / w as f64 / zoom;
                let n = self.normal_at(u, v, du, dv, amplitude);
                rgb.push(((n[0] * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8);
                rgb.push(((n[1] * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8);
                rgb.push(((n[2] * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
        Texture { w, h, rgb }
    }
}

// ============================================================================
// ТЕСТЫ
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_lattice_range_and_determinism() {
        for ix in -3i64..3 {
            for iy in -3i64..3 {
                let v = hash_lattice(ix, iy, 42);
                assert!((0.0..1.0).contains(&v), " вне [0,1): {v}");
                assert_eq!(v, hash_lattice(ix, iy, 42), "зерно фиксирует хеш");
            }
        }
        // Разные зерна — разные значения (не обязаны всегда, но на этих — да)
        let a = hash_lattice(7, 9, 1);
        let b = hash_lattice(7, 9, 2);
        assert_ne!(a, b);
        // Разброс статистически разумный
        let n = 1000usize;
        let sum: f64 = (0..n).map(|i| hash_lattice(i as i64, (i * 3) as i64, 777)).sum();
        assert!((sum / n as f64 - 0.5).abs() < 0.08, "среднее ~ 0.5, got {}", sum / n as f64);
    }

    #[test]
    fn value_noise_continuous_and_bounded() {
        let s = 99u64;
        for &(x, y) in &[(0.0, 0.0), (0.5, 0.5), (1.0, 1.0), (-2.7, 3.1), (10.0, -0.5)] {
            let v = value_noise(x, y, s);
            assert!((0.0..=1.0).contains(&v), "{v}");
        }
        // C¹-гладкость: малый шаг меняет значение мало (quintic)
        let (x, y) = (3.21, 4.56);
        let v0 = value_noise(x, y, s);
        let v1 = value_noise(x + 1e-3, y, s);
        assert!((v0 - v1).abs() < 5e-3, "разрыв: {v0} vs {v1}");
    }

    #[test]
    fn fbm_bounded_deterministic() {
        let a = fbm(1.1, 2.2, 7, 5, 0.5);
        let b = fbm(1.1, 2.2, 7, 5, 0.5);
        assert_eq!(a.to_bits(), b.to_bits(), "бит-в-бит");
        assert!((0.0..=1.0).contains(&a));
        // Больше октав — больше деталей (дисперсия растёт к краям)
        let c = fbm(1.1, 2.2, 7, 1, 0.5);
        assert!((a - c).abs() > 1e-6);
    }

    #[test]
    fn texture_determinism_and_seed_sensitivity() {
        let pal = Palette::parse("copper").unwrap();
        let spec = TextureSpec { seed: 7, ..Default::default() };
        let t1 = spec.render(64, 64, 1.0, &pal);
        let t2 = spec.render(64, 64, 1.0, &pal);
        assert_eq!(t1.texture_hash(), t2.texture_hash(), "бит-в-бит");
        let spec2 = TextureSpec { seed: 8, ..Default::default() };
        let t3 = spec2.render(64, 64, 1.0, &pal);
        assert_ne!(t1.texture_hash(), t3.texture_hash(), "другое зерно — другая текстура");
        assert_eq!(t1.rgb.len(), 64 * 64 * 3);
        // Зум: другая глубина, но валидный растр
        let t4 = spec.render(64, 64, 4.0, &pal);
        assert_ne!(t1.texture_hash(), t4.texture_hash());
    }

    #[test]
    fn styles_all_render() {
        let pal = Palette::parse("ice").unwrap();
        for style in [TexStyle::Noise, TexStyle::Marble, TexStyle::Wood] {
            let spec = TextureSpec { style, ..Default::default() };
            let t = spec.render(48, 48, 1.0, &pal);
            assert_eq!(t.rgb.len(), 48 * 48 * 3);
            // Не плоская текстура
            let (mut lo, mut hi) = (255u8, 0u8);
            for c in t.rgb.chunks_exact(3) {
                lo = lo.min(c[0]);
                hi = hi.max(c[0]);
            }
            assert!(hi > lo + 16, "{style:?} почти плоская: {lo}..{hi}");
        }
    }

    #[test]
    fn png_roundtrip_via_p3_decoder() {
        let pal = Palette::parse("jade").unwrap();
        let spec = TextureSpec::default();
        let t = spec.render(40, 30, 1.0, &pal);
        let dir = std::env::temp_dir().join("poler_tex_test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("tex.png");
        t.write_png(&p).unwrap();
        let raw = std::fs::read(&p).unwrap();
        // Суверенный декодер P³ проверяет структуру (ct = 2 → truecolor RGB)
        let (w, h, ct, _raw) = crate::p3::png::decode_own(&raw).expect("PNG валиден");
        assert_eq!((w, h), (40, 30));
        assert_eq!(ct, 2, "color type 2 = RGB");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn svd_rank_one_matrix_recovered() {
        // A = u·vᵀ (ранг 1): реконструкция ранга 1 должна быть точной
        let (m, n) = (6usize, 5usize);
        let u0 = [1.0, 2.0, 3.0, -1.0, 0.5, 2.5];
        let v0 = [0.5, -1.0, 2.0, 1.0, 0.25];
        let mut a = vec![0.0; m * n];
        for i in 0..m {
            for j in 0..n {
                a[i * n + j] = u0[i] * v0[j];
            }
        }
        let (u, sigma, v) = jacobi_svd(&a, m, n);
        assert!((sigma[0] - (u0.iter().map(|x| x * x).sum::<f64>().sqrt()
            * v0.iter().map(|x| x * x).sum::<f64>().sqrt())).abs() < 1e-9,
            "σ₁ = |u||v|");
        // Хвост почти нулевой
        assert!(sigma.iter().skip(1).all(|s| *s < 1e-9), "ранг 1: хвост нулевой: {:?}", sigma);
        let recon = svd_rank_reconstruct(&u, &sigma, &v, m, n, 1);
        for i in 0..m * n {
            assert!((a[i] - recon[i]).abs() < 1e-9, "ранг-1 реконструкция точна");
        }
    }

    #[test]
    fn svd_full_rank_lossless_and_curve_monotone() {
        // Полный ранг → почти без потерь
        let spec = TextureSpec { style: TexStyle::Noise, seed: 3, freq: 3.0, ..Default::default() };
        let pal = Palette::parse("gray").unwrap();
        let t = spec.render(64, 64, 1.0, &pal);
        let gray = t.gray();
        let (full, stats_full) = svd_encode_gray(&gray, 64, 64, 16, 16);
        assert!(stats_full.psnr_db > 50.0, "полный ранг почти лосслесс: {}", stats_full.psnr_db);
        let diff_max = gray.iter().zip(&full).map(|(a, b)| (*a as i32 - *b as i32).abs()).max().unwrap();
        assert!(diff_max <= 1, "квантование ±1, got {diff_max}");
        // Кривая монотонно растёт
        let curve = rank_curve(&gray, 64, 64, 16, &[1, 2, 4, 8, 16]);
        for w in curve.windows(2) {
            assert!(
                w[1].psnr_db >= w[0].psnr_db - 0.5,
                "PSNR растёт с рангом: {} → {}",
                w[0].psnr_db,
                w[1].psnr_db
            );
        }
        assert!(curve.last().unwrap().kept_ratio <= 1.0);
        // Низкий ранг реально сжимает: ранг 1 сильно хуже полного
        assert!(curve[0].psnr_db < curve.last().unwrap().psnr_db - 6.0);
    }

    #[test]
    fn palette_map_extremes() {
        let pal = Palette::parse("gray").unwrap();
        assert_eq!(pal.map(0.0), pal.lo);
        assert_eq!(pal.map(1.0), pal.hi);
        let mid = pal.map(0.5);
        assert_eq!(mid, pal.mid);
        // Клампы
        assert_eq!(pal.map(-1.0), pal.lo);
        assert_eq!(pal.map(2.0), pal.hi);
        assert!(Palette::parse("no-such").is_none());
        assert!(TexStyle::parse("marble").is_some());
        assert!(TexStyle::parse("bogus").is_none());
    }

    // ── U0: normal maps ────────────────────────────────────────────────────

    #[test]
    fn normal_flat_height_is_pure_z() {
        // Плоская высота → нормаль строго (0,0,1)
        let n = height_normal(|_, _| 0.5, 0.3, 0.7, 1e-3, 1e-3, 3.0);
        assert!((n[0].abs()) < 1e-12 && (n[1].abs()) < 1e-12, "{n:?}");
        assert!((n[2] - 1.0).abs() < 1e-12);
        // Амплитуда 0 — тоже плоскость при любом рельефе
        let n0 = height_normal(|x, y| x * y * 9.0, 0.4, 0.4, 1e-3, 1e-3, 0.0);
        assert!((n0[2] - 1.0).abs() < 1e-12, "amplitude=0 → плоскость");
    }

    #[test]
    fn normal_plane_has_constant_tilt_and_unit_length() {
        // h = 2u + 4v: наклон постоянен → одна и та же нормаль везде
        let d = 1e-3;
        let a = height_normal(|x, y| 2.0 * x + 4.0 * y, 0.1, 0.2, d, d, 1.0);
        let b = height_normal(|x, y| 2.0 * x + 4.0 * y, 0.8, 0.6, d, d, 1.0);
        for i in 0..3 {
            assert!((a[i] - b[i]).abs() < 1e-9, "наклон постоянен: {a:?} vs {b:?}");
        }
        // Единичная длина и корректный знак: h растёт по u → нормаль смотрит в −u
        let len = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-12);
        assert!(a[0] < 0.0, "градиент +u → нормаль −u");
        assert!(a[1] < 0.0, "градиент +v → нормаль −v");
        assert!(a[2] > 0.0);
        // Точная пропорция: n ∝ (−2A, −4A, 1)
        let expected = [-2.0f64, -4.0, 1.0];
        let elen = (4.0 + 16.0 + 1.0f64).sqrt();
        for i in 0..3 {
            assert!((a[i] - expected[i] / elen).abs() < 1e-9, "пропорция плоскости: {a:?}");
        }
    }

    #[test]
    fn normal_map_units_and_blue_dominance() {
        let spec = TextureSpec { style: TexStyle::Marble, seed: 11, ..Default::default() };
        let t = spec.render_normal(64, 64, 1.0, 0.08);
        assert_eq!(t.rgb.len(), 64 * 64 * 3);
        let mut z_min = 255u8;
        for c in t.rgb.chunks_exact(3) {
            // Декодируем нормаль и проверяем |n| ≈ 1
            let nx = c[0] as f64 / 127.5 - 1.0;
            let ny = c[1] as f64 / 127.5 - 1.0;
            let nz = c[2] as f64 / 127.5 - 1.0;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            assert!((len - 1.0).abs() < 0.03, "не единичная: {len}");
            z_min = z_min.min(c[2]);
        }
        assert!(z_min > 100, "z-компонента доминирует: {z_min}");
        // Синусоидальные жилы гарантируют наклоны в обе стороны:
        // x-канал обязан иметь тексели и меньше, и больше нейтрали 128
        let (mut x_lo, mut x_hi) = (0usize, 0usize);
        for c in t.rgb.chunks_exact(3) {
            if c[0] < 128 {
                x_lo += 1;
            }
            if c[0] > 128 {
                x_hi += 1;
            }
        }
        assert!(x_lo > 16 && x_hi > 16, "наклоны в обе стороны: lo={x_lo} hi={x_hi}");
        // Детерминизм и чувствительность к амплитуде
        let t2 = spec.render_normal(64, 64, 1.0, 0.08);
        assert_eq!(t.texture_hash(), t2.texture_hash(), "бит-в-бит");
        let t3 = spec.render_normal(64, 64, 1.0, 0.16);
        assert_ne!(t.texture_hash(), t3.texture_hash(), "амплитуда меняет рельеф");
        // Разрешение-инвариантность: численная производная сходится к
        // аналитической при измельчении шага. Спек с 2 октавами: самый
        // тонкий признак ~1/16 тайла — шаг 1/64 уже заведомо ниже
        // Найквиста, обе оценки в региме сходимости.
        let smooth_spec =
            TextureSpec { style: TexStyle::Marble, seed: 11, octaves: 2, ..Default::default() };
        let lo = smooth_spec.normal_at(0.5, 0.5, 1.0 / 64.0, 1.0 / 64.0, 0.08);
        let hi = smooth_spec.normal_at(0.5, 0.5, 1.0 / 256.0, 1.0 / 256.0, 0.08);
        for i in 0..3 {
            assert!(
                (lo[i] - hi[i]).abs() < 0.02,
                "производная не сходится: {lo:?} vs {hi:?}"
            );
        }
    }
}
