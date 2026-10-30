//! Мир Этерии — рельеф, биомы и гекс-карта (Сессия-14).
//!
//! План PORTING_NOTES.md, шаг 4–5: «высотная карта Этерии = TIN →
//! изолинии через viz_field» и «H3-гексагоны → карта биомов Этерии».
//! Сессия-14 сплетает три слоя движка в один конвейер:
//!
//! ```text
//!   diamond-square (детерминированный, seed)  ──▶ карта высот
//!        │                                          │
//!   hexgrid: гекс-диск на грани икосаэдра      биомы (6 типов,
//!        │                                          фредерит — лор)
//!        ▼                                          ▼
//!   calc::logprob: P_i = e^(−E_i), E_i = (1−h_i)·scale
//!        │  каждый член ~ e^(−1e5) — ВНЕ f64; ряд Z = Σ P_i
//!        │  суммируется только лог-доменной арифметикой (lpsum)
//!        ▼
//!   viz_hex: SVG-карта · eteria_field → viz_field: изолинии
//! ```
//!
//! Детерминизм: value-noise на splitmix64 без состояния RNG —
//! один seed даёт один и тот же мир от запуска к запуску.

use crate::calc::logprob::{lpsum_lns, LogProb};
use crate::geo::hexgrid::{
    axial_to_spiral, face_of_direction, hex_disk, world_address, world_trit_id,
};

/// Биомы Этерии: (имя, цвет SVG). Фредерит — лор проекта
/// (фредеритовые поля из geo/mod.rs, Сессия-12).
pub const BIOMES: [(&str, &str); 6] = [
    ("океан", "#1d4ed8"),
    ("побережье", "#7dd3fc"),
    ("степь", "#d9a44a"),
    ("лес", "#15803d"),
    ("горы", "#78716c"),
    ("фредерит", "#a21caf"),
];

/// Биом по высоте [0,1]: пороги подобраны под островное затухание.
pub fn biome_index(h: f64) -> usize {
    if h < 0.30 {
        0
    } else if h < 0.38 {
        1
    } else if h < 0.55 {
        2
    } else if h < 0.72 {
        3
    } else if h < 0.88 {
        4
    } else {
        5
    }
}

/// splitmix64 — детерминированный миксер (без состояния).
fn splitmix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Value-noise [0,1) на целочисленной решётке: хеш координат + seed.
fn noise01(seed: u64, x: i64, y: i64) -> f64 {
    let h = splitmix(
        seed
            .wrapping_mul(0x100000001B3)
            .wrapping_add((x as u64).wrapping_mul(31))
            .wrapping_add((y as u64).wrapping_mul(0x9E3779B1)),
    );
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// Гладкий шум: билинейная интерполяция value-noise по узлам.
fn smooth_noise(seed: u64, x: f64, y: f64) -> f64 {
    let (x0, y0) = (x.floor() as i64, y.floor() as i64);
    let (fx, fy) = (x - x0 as f64, y - y0 as f64);
    // сглаживание (smoothstep) убывает от сеточной структуры
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let n00 = noise01(seed, x0, y0);
    let n10 = noise01(seed, x0 + 1, y0);
    let n01 = noise01(seed, x0, y0 + 1);
    let n11 = noise01(seed, x0 + 1, y0 + 1);
    let top = n00 + (n10 - n00) * sx;
    let bot = n01 + (n11 - n01) * sx;
    top + (bot - top) * sy
}

/// Карта высот Этерии: diamond-square + островное затухание.
/// `n = 2^m + 1` ∈ {9, 17, 33, 65}; результат ∈ [0,1].
///
/// Фрактал: углы → ромбы → квадраты, амплитуда × 0.55 на уровень
/// (характеристика ~0.78 — «говорящие» хребты, не белый шум).
/// Затухание к краям превращает карту в архипелаг: океан снаружи.
pub fn heightmap(seed: u64, n: usize) -> Result<Vec<Vec<f64>>, String> {
    let m = (n as f64 - 1.0).log2().round() as i32;
    if n < 9 || n > 65 || (1 << m) + 1 != n as i32 {
        return Err(format!(
            "eteria: размер сетки n ∈ {{9, 17, 33, 65}} (2^m+1), получено {n}"
        ));
    }
    let s = seed;
    let mut z = vec![vec![0.0f64; n]; n];
    // углы
    z[0][0] = noise01(s, 1, 1);
    z[0][n - 1] = noise01(s, 2, 1);
    z[n - 1][0] = noise01(s, 1, 2);
    z[n - 1][n - 1] = noise01(s, 2, 2);
    let mut step = (n - 1) as usize; // текущий шаг сетки
    let mut amp = 1.0f64; // амплитуда уровня
    while step > 1 {
        let half = step / 2;
        // ромб: центры квадратов
        for y in (half..n).step_by(step) {
            for x in (half..n).step_by(step) {
                let avg = (z[y - half][x - half]
                    + z[y - half][x + half]
                    + z[y + half][x - half]
                    + z[y + half][x + half])
                    / 4.0;
                z[y][x] = avg + (noise01(s, x as i64, y as i64) - 0.5) * amp;
            }
        }
        // квадрат: середины рёбер
        for y in 0..n {
            let xs: Vec<usize> = if y % step == half {
                (0..n).step_by(step).collect() // на строках ромбов — узлы
            } else {
                (half..n).step_by(step).collect() // иначе — середины
            };
            for x in xs {
                if z[y][x] != 0.0 || (x == 0 && y == 0) {
                    continue; // уже выставлено (углы/ромбы)
                }
                let mut acc = 0.0;
                let mut cnt = 0;
                if y >= half {
                    acc += z[y - half][x];
                    cnt += 1;
                }
                if y + half < n {
                    acc += z[y + half][x];
                    cnt += 1;
                }
                if x >= half {
                    acc += z[y][x - half];
                    cnt += 1;
                }
                if x + half < n {
                    acc += z[y][x + half];
                    cnt += 1;
                }
                if cnt > 0 {
                    z[y][x] = acc / cnt as f64 + (noise01(s, x as i64 + 7, y as i64 + 13) - 0.5) * amp;
                }
            }
        }
        step = half;
        amp *= 0.55;
    }
    // островное затухание: h *= max(0, 1 − d²), d — нормированный радиус;
    // затем клип снизу (глубокий шум = океан 0) и деление на максимум
    // (нормализация по min ПЫДНИМАЛА бы дно и топила бы берега)
    let c = (n - 1) as f64 / 2.0;
    let mut zmax = f64::NEG_INFINITY;
    for y in 0..n {
        for x in 0..n {
            let dx = (x as f64 - c) / c;
            let dy = (y as f64 - c) / c;
            let d2 = (dx * dx + dy * dy) / 1.15; // континент до ~76% радиуса
            z[y][x] = (z[y][x] * (1.0 - d2).max(0.0)).max(0.0);
            zmax = zmax.max(z[y][x]);
        }
    }
    for row in z.iter_mut() {
        for v in row.iter_mut() {
            *v /= zmax.max(1e-12);
        }
    }
    Ok(z)
}

/// Билинейная выборка карты (координаты в [0, n−1], края клампятся).
fn sample(z: &[Vec<f64>], x: f64, y: f64) -> f64 {
    let n = z.len();
    let clamp = |v: f64| v.clamp(0.0, (n - 1) as f64);
    let (x, y) = (clamp(x), clamp(y));
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(n - 1), (y0 + 1).min(n - 1));
    let (fx, fy) = (x - x0 as f64, y - y0 as f64);
    let top = z[y0][x0] + (z[y0][x1] - z[y0][x0]) * fx;
    let bot = z[y1][x0] + (z[y1][x1] - z[y1][x0]) * fx;
    top + (bot - top) * fy
}

/// Ячейка гекс-карты Этерии.
#[derive(Clone, Copy, Debug)]
pub struct HexCell {
    pub q: i64,
    pub r: i64,
    /// высота [0,1]
    pub h: f64,
    /// биом (индекс в BIOMES)
    pub biome: usize,
}

/// Гекс-карта Этерии: диск радиуса `radius` (≤ 10) на решётке высот.
/// Осевые координаты маппятся в квадрат [−1,1]² сетки и сэмплируются
/// билинейно — гекс-диск вырезает из карты шестиугольный сектор.
pub fn hex_cells(seed: u64, radius: i64) -> Result<Vec<HexCell>, String> {
    if radius < 1 || radius > 10 {
        return Err(format!(
            "eteria: радиус гекс-диска ∈ [1, 10] (6-тритная спираль), получено {radius}"
        ));
    }
    let n = 65usize;
    let z = heightmap(seed, n)?;
    let mut out = Vec::with_capacity((1 + 3 * radius * (radius + 1)) as usize);
    for (q, r) in hex_disk(radius) {
        let u = q as f64 / radius as f64;
        let v = r as f64 / radius as f64;
        let gx = (u * 0.5 + 0.5) * (n - 1) as f64;
        let gy = (v * 0.5 + 0.5) * (n - 1) as f64;
        let h = sample(&z, gx, gy);
        out.push(HexCell { q, r, h, biome: biome_index(h) });
    }
    Ok(out)
}

/// Грань икосаэдра сектора: детерминированное направление из seed.
pub fn sector_face(seed: u64) -> Result<usize, String> {
    let a = noise01(seed, 101, 7) * 2.0 - 1.0;
    let b = noise01(seed, 11, 113) * 2.0 - 1.0;
    let c = noise01(seed, 31, 37) * 2.0 - 1.0;
    face_of_direction((a, b, c))
}

/// Больцмановское переплетение (демо «прорыва», Сессия-14):
/// аномальные вероятности по ячейкам Этерии.
///
/// E_i = (1.05 − h_i)·`scale` (горы — дёшево, океан — дорог; даже пик
/// несёт штраф 0.05·scale), P_i = e^(−E_i).
/// При scale ~ 1e5 КАЖДЫЙ член ~ e^(−5·10³) и меньше — вне f64: прямое
/// суммирование даёт Σ = 0.0 с полной потерей информации; лог-домен
/// выдаёт Z, нормированные доли и тритные глубины — без потери порядка.
pub fn boltzmann_report(seed: u64, radius: i64, scale: f64) -> Result<String, String> {
    if !(1.0..1e12).contains(&scale) {
        return Err(format!(
            "eteria: масштаб энергий scale ∈ [1, 1e12] (доля k_B·T в знаменателе), получено {scale}"
        ));
    }
    let cells = hex_cells(seed, radius)?;
    let face = sector_face(seed)?;
    // маржа 0.05: даже пик (h=1) платит 0.05·scale — весь ряд за f64
    let lns: Vec<f64> = cells.iter().map(|c| -(1.05 - c.h) * scale).collect();
    let z = lpsum_lns(&lns)?;
    // топ-3 по вероятности (минимум энергии = максимум высоты)
    let mut idx: Vec<usize> = (0..cells.len()).collect();
    idx.sort_by(|&a, &b| cells[b].h.partial_cmp(&cells[a].h).unwrap());
    let top: Vec<String> = idx
        .iter()
        .take(3)
        .map(|&i| {
            let c = cells[i];
            let p = LogProb::from_ln(lns[i])?;
            let share = LogProb::from_ln(lns[i] - z.ln())?;
            let s = axial_to_spiral(c.q, c.r)?;
            let wid = world_trit_id(face, s)?;
            // Сессия-17: центр-потомок res-2 (9q, 9r) — адрес 13 тритов:
            // та же ячейка, но на канонической глубине иерархии H3
            let wid13 = world_address(face, 9 * c.q, 9 * c.r, 2)
                .map(|t| t.to_string_bal())
                .unwrap_or_else(|_| "— вне спирали".into());
            Ok(format!(
                "  ({:+}, {:+}) {} · h = {:.3} · P = {} · доля = {} · s = {} · мир 9 тритов: {} · res-2 (13 тритов): {}",
                c.q,
                c.r,
                BIOMES[c.biome].0,
                c.h,
                p.fmt10(),
                share.fmt10(),
                s,
                wid.to_string_bal(),
                wid13
            ))
        })
        .collect::<Result<Vec<String>, String>>()?;
    // сравнение с f64: прямая сумма ряда в f64
    let f64_sum: f64 = lns.iter().map(|&x| x.exp()).sum();
    let z_in_f64 = if f64_sum == 0.0 { "0.0 (ряд целиком в underflow)" } else { "частично" };
    Ok([
        format!(
            "Этерия · гекс-диск R{radius} · {} ячеек · грань икосаэдра {}/20 · seed {}",
            cells.len(),
            face + 1,
            seed
        ),
        format!(
            "E_i = (1.05−h_i)·{} → ln P_i ∈ [{}, {}] — весь ряд вне f64",
            crate::calc::logprob::fmt_num(scale),
            crate::calc::logprob::exp_str(lns.iter().cloned().fold(f64::INFINITY, f64::min)),
            crate::calc::logprob::exp_str(lns.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
        ),
        format!(
            "Z = Σ P_i = {} = {} · тритная глубина {} против",
            z.fmt10(),
            z.fmt3(),
            crate::calc::logprob::exp_str(z.trit_depth())
        ),
        format!("прямая сумма в f64: {z_in_f64} · лог-домен: потеряно 0 порядков"),
        "топ-ячейки:".to_string(),
    ]
    .into_iter()
    .chain(top)
    .collect::<Vec<_>>()
    .join("\n"))
}

// ─────────────── TIN Делоне + Муха (Сессия-15) ───────────────
//
// Переплетение пяти модулей одним конвейером:
//   universal_letters (Φ — золотое сечение) → сеем вершины спиралью
//   Фибоначчи (та же константа, что у букв и у Мухи);
//   delaunay (Bowyer-Watson, муха водит порядок вставки) → TIN;
//   terrain_detail (diamond-square) → высоты вершин;
//   logprob: E_i = (1.05−h_i)·scale → Z = Σ e^(−E_i) за пределами f64;
//   барицентрическая интерполяция TIN против прямой terrain_detail —
//   честная метрика точности рельефа на решётке.

/// Вершины TIN Этерии: n точек, посеянных золотой спиралью
/// (x_i = frac(i·φ), y_i = frac(i·φ²) + splitmix-джиттер от seed),
/// высоты — `terrain_detail` (тот же мир, тот же seed).
pub fn tin_vertices(seed: u64, n: usize) -> Result<Vec<(f64, f64, f64)>, String> {
    if !(3..=5000).contains(&n) {
        return Err(format!("eteria: вершин TIN n ∈ [3, 5000], получено {n}"));
    }
    let phi = (1.0 + 5.0f64.sqrt()) / 2.0;
    let mut state = seed | 1;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let fi = i as f64 + 0.5;
        let jx = splitmix_jitter(&mut state) * 0.4;
        let jy = splitmix_jitter(&mut state) * 0.4;
        let x = (fi * phi + jx).fract();
        let y = (fi * phi * phi + jy).fract();
        let h = terrain_detail(seed, x, y);
        out.push((x, y, h));
    }
    Ok(out)
}

/// Детерминированный джиттер ∈ (−0.5, 0.5) без состояния RNG
/// (splitmix64-шаг, как в value-noise Этерии).
fn splitmix_jitter(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64 - 0.5
}

/// Отчёт «Этерия · TIN + Муха»: триангуляция Делоне на золотом посеве,
/// больцмановская статистика высот за пределами f64 и честная метрика
/// TIN-интерполяции против diamond-square.
pub fn tin_boltz_report(seed: u64, n: usize, scale: f64) -> Result<String, String> {
    if !(3..=5000).contains(&n) {
        return Err(format!("eteria: вершин TIN n ∈ [3, 5000], получено {n}"));
    }
    if !(1.0..1e12).contains(&scale) {
        return Err(format!(
            "eteria: масштаб энергий scale ∈ [1, 1e12], получено {scale}"
        ));
    }
    let verts = tin_vertices(seed, n)?;
    // решётка k = 10: квант 3⁻¹⁰, координаты ≤ 3¹⁰ — запас i128 (k ≤ 14)
    let k = 10u8;
    let mut pts_i: Vec<(i64, i64)> = Vec::with_capacity(n);
    for &(x, y, _) in &verts {
        let tp = crate::geo::trit_coord::TritPoint::quantize(x, y, k)
            .map_err(|e| format!("eteria: квантование ({x}, {y}): {e}"))?;
        pts_i.push((tp.x.to_i64(), tp.y.to_i64()));
    }
    let mut tin = crate::geo::delaunay::delaunay(&pts_i)?;
    crate::geo::delaunay::verify_delaunay(&tin)?;
    // 27-дерево конвертов треугольников (Сессия-16): height_at идёт
    // трит-путём — переплетение TIN ↔ пространственное индексирование
    let (idx_objs, idx_nodes) = tin
        .build_spatial27()
        .map_err(|e| format!("eteria: {e}"))?;
    let z: Vec<f64> = verts.iter().map(|&(_, _, h)| h).collect();

    // больцмановский ряд: E_i = (1.05 − h_i)·scale — маржа 0.05 как в
    // hex-версии: даже пик платит, ряд не вырождается в одну единицу
    let lns: Vec<f64> = verts.iter().map(|&(_, _, h)| -(1.05 - h) * scale).collect();
    let zpart = lpsum_lns(&lns)?;
    let f64_sum: f64 = lns.iter().map(|&x| x.exp()).sum();
    let z_in_f64 = if f64_sum == 0.0 {
        "0.0 (ряд целиком в underflow)"
    } else {
        "частично"
    };

    // топ-3 вершины по высоте (минимум энергии)
    let mut idx: Vec<usize> = (0..verts.len()).collect();
    idx.sort_by(|&a, &b| verts[b].2.partial_cmp(&verts[a].2).unwrap());
    let top: Vec<String> = idx
        .iter()
        .take(3)
        .map(|&i| {
            let (x, y, h) = verts[i];
            let p = LogProb::from_ln(lns[i])?;
            let share = LogProb::from_ln(lns[i] - zpart.ln())?;
            Ok(format!(
                "  ({:.3}, {:.3}) {} · h = {:.3} · P = {} · доля = {}",
                x,
                y,
                BIOMES[biome_index(h)].0,
                h,
                p.fmt10(),
                share.fmt10()
            ))
        })
        .collect::<Result<Vec<String>, String>>()?;

    // метрика точности: TIN-интерполяция против terrain_detail в 33
    // пробах золотой спирали ЗА пределами посева (i = n..n+32)
    let mut max_dev = 0.0f64;
    let mut probes = 0usize;
    for i in n..n + 33 {
        let fi = i as f64 + 0.5;
        let phi = (1.0 + 5.0f64.sqrt()) / 2.0;
        let x = (fi * phi).fract();
        let y = (fi * phi * phi).fract();
        let tp = crate::geo::trit_coord::TritPoint::quantize(x, y, k)
            .map_err(|e| format!("eteria: проба ({x}, {y}): {e}"))?;
        if let Some(h_tin) = tin.height_at(&z, tp.x.to_i64(), tp.y.to_i64()) {
            let h_direct = terrain_detail(seed, x, y);
            max_dev = max_dev.max((h_tin - h_direct).abs());
            probes += 1;
        }
    }

    // изолинии (Сессия-17): медианный уровень — marching squares по
    // height_at (тот же 27-дерево) против ТОЧНОГО сечения треугольников
    let (mut zlo, mut zhi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &h in &z {
        zlo = zlo.min(h);
        zhi = zhi.max(h);
    }
    let level = (zlo + zhi) / 2.0;
    let ms = crate::geo::marchsq::tin_isolines(&tin, &z, level, 64)?;
    let ex = crate::geo::marchsq::tin_isolines_exact(&tin, &z, level)?;
    let iso_line = format!(
        "изолинии (сессия-17): уровень {:.3} — marching squares {} ветвей · длина {:.0} \
         против точно {} · {:.0} квантов (27-дерево: height_at трит-путём)",
        level,
        ms.len(),
        crate::geo::marchsq::total_length(&ms),
        ex.len(),
        crate::geo::marchsq::total_length(&ex)
    );

    Ok([
        format!(
            "Этерия · TIN Делоне · {} вершин золотого посева · {} треугольников · \
             оболочка {} рёбер · seed {}",
            verts.len(),
            tin.tris.len(),
            tin.hull().len(),
            seed
        ),
        format!(
            "муха: порядок вставки sin(i·Φ mod 2π) · решётка k = {k} · \
             Делоне верифицирован (i128, ноль округлений)"
        ),
        format!(
            "ускорение сессии-16: history-DAG + BFS-полость · 27-дерево: \
             {idx_objs} конвертов · {idx_nodes} узлов — height_at трит-путём"
        ),
        format!(
            "E_i = (1.05−h_i)·{} → Z = Σ e^(−E_i) = {} = {}",
            crate::calc::logprob::fmt_num(scale),
            zpart.fmt10(),
            zpart.fmt3()
        ),
        format!("прямая сумма в f64: {z_in_f64} · лог-домен: потеряно 0 порядков"),
        format!(
            "точность рельефа: TIN против diamond-square в {probes} пробах · \
             max |Δh| = {:.4}",
            max_dev
        ),
        iso_line,
        "топ-вершины (минимум энергии):".to_string(),
    ]
    .into_iter()
    .chain(top)
    .collect::<Vec<_>>()
    .join("\n"))
}

/// Число биомов в наборе ячеек (для легенды карты).
pub fn biome_counts(cells: &[HexCell]) -> [usize; 6] {
    let mut c = [0usize; 6];
    for cell in cells {
        c[cell.biome] += 1;
    }
    c
}

/// Гладкий шум (для внешних слоёв, если понадобится мелкая детализация).
pub fn terrain_detail(seed: u64, x: f64, y: f64) -> f64 {
    smooth_noise(seed ^ 0xDE7A, x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heightmap_deterministic_and_bounded() {
        for seed in [7u64, 42, 2025] {
            let a = heightmap(seed, 33).unwrap();
            let b = heightmap(seed, 33).unwrap();
            assert_eq!(a, b, "детерминизм seed={seed}");
            assert_eq!(a.len(), 33);
            for row in &a {
                assert_eq!(row.len(), 33);
                for &v in row {
                    assert!((0.0..=1.0).contains(&v), "h={v} вне [0,1]");
                }
            }
            // остров: пик — в средней зоне, не у края
            let mut maxv = 0.0f64;
            let mut maxpos = (0usize, 0usize);
            for (y, row) in a.iter().enumerate() {
                for (x, &v) in row.iter().enumerate() {
                    if v > maxv {
                        maxv = v;
                        maxpos = (x, y);
                    }
                }
            }
            let (cx, cy) = (maxpos.0 as f64, maxpos.1 as f64);
            let d = ((cx - 16.0).powi(2) + (cy - 16.0).powi(2)).sqrt();
            assert!(d < 16.0 * 0.8, "пик {maxv} в {maxpos:?} — не у края");
            // углы заметно ниже пика (островное затухание; после
            // нормализации угол не обязан быть ровно 0)
            for &(x, y) in &[(0usize, 0usize), (32, 0), (0, 32), (32, 32), (16, 0), (0, 16)] {
                assert!(a[y][x] < 0.5 * maxv, "край ({x},{y}) = {} ≥ полпика", a[y][x]);
            }
        }
    }

    #[test]
    fn heightmap_rejects_bad_sizes() {
        assert!(heightmap(7, 8).is_err());
        assert!(heightmap(7, 10).is_err());
        assert!(heightmap(7, 66).is_err());
        assert!(heightmap(7, 9).is_ok());
        assert!(heightmap(7, 65).is_ok());
    }

    #[test]
    fn hex_cells_cover_disk_with_biomes() {
        for radius in [3i64, 8, 10] {
            let cells = hex_cells(42, radius).unwrap();
            assert_eq!(cells.len(), (1 + 3 * radius * (radius + 1)) as usize);
            // центр — всегда в наборе
            assert!(cells.iter().any(|c| c.q == 0 && c.r == 0));
            // биомы в диапазоне, высоты в [0,1]
            for c in &cells {
                assert!(c.biome < 6);
                assert!((0.0..=1.0).contains(&c.h));
            }
            // разнообразие: на R8 хотя бы 3 биома
            let counts = biome_counts(&hex_cells(42, 8).unwrap());
            assert!(counts.iter().filter(|&&n| n > 0).count() >= 3);
        }
        assert!(hex_cells(42, 0).is_err());
        assert!(hex_cells(42, 11).is_err());
    }

    #[test]
    fn boltzmann_report_beyond_f64() {
        let rep = boltzmann_report(7, 8, 1e5).unwrap();
        assert!(rep.contains("217 ячеек"), "R8 = 217: {rep}");
        assert!(rep.contains("грань икосаэдра"), "{rep}");
        assert!(rep.contains("Z = Σ P_i"), "{rep}");
        assert!(rep.contains("0.0 (ряд целиком в underflow)"), "{rep}");
        assert!(rep.contains("мир 9 тритов"), "{rep}");
        // весь ряд за f64: ln P_i ≤ −0.05·1e5 = −5000 ≪ −708
        assert!(rep.contains("-5.000e+03"), "маржа пика: {rep}");
        // Z тоже за f64 — форма 10^(…)
        assert!(rep.contains("10^("), "Z в форме 10^(…): {rep}");
        // ошибки валидации
        assert!(boltzmann_report(7, 8, 0.0).is_err());
        assert!(boltzmann_report(7, 12, 1e5).is_err());
    }

    #[test]
    fn boltzmann_reports_session17() {
        // res-2 адрес в гекс-отчёте: 13 тритов центр-потомка топ-ячейки
        let rep = boltzmann_report(7, 8, 1e5).unwrap();
        assert!(rep.contains("res-2 (13 тритов)"), "адрес res-2: {rep}");
        // длина адреса в самом деле 13 тритов
        let cells = hex_cells(7, 8).unwrap();
        let face = sector_face(7).unwrap();
        let top = cells
            .iter()
            .max_by(|a, b| a.h.partial_cmp(&b.h).unwrap())
            .unwrap();
        let wid13 = world_address(face, 9 * top.q, 9 * top.r, 2).unwrap();
        assert_eq!(wid13.len(), 13);
        assert!(rep.contains(&wid13.to_string_bal()), "адрес в отчёте: {rep}");
        // изолинии в TIN-отчёте
        let rep = tin_boltz_report(7, 64, 1e5).unwrap();
        assert!(rep.contains("изолинии (сессия-17)"), "строка изолиний: {rep}");
        assert!(rep.contains("marching squares"), "{rep}");
        assert!(rep.contains("против точно"), "{rep}");
    }

    #[test]
    fn boltzmann_matches_manual_lpsum() {
        // Z из отчёта = Z из прямого пересчёта lpsum по тем же ячейкам
        let cells = hex_cells(7, 6).unwrap();
        let lns: Vec<f64> = cells.iter().map(|c| -(1.05 - c.h) * 1e5).collect();
        let z = lpsum_lns(&lns).unwrap();
        let rep = boltzmann_report(7, 6, 1e5).unwrap();
        // доля топ-ячейки: e^{ln P_0 − ln Z} — проверяем топ-строку
        let mut hmax = f64::NEG_INFINITY;
        let mut imax = 0usize;
        for (i, c) in cells.iter().enumerate() {
            if c.h > hmax {
                hmax = c.h;
                imax = i;
            }
        }
        let share = LogProb::from_ln(lns[imax] - z.ln()).unwrap();
        assert!(
            rep.contains(&share.fmt10()),
            "доля {share:?} в отчёте: {rep}"
        );
    }
}
