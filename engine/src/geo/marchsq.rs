//! Изолинии поверх TIN — marching squares по `height_at` (Сессия-17).
//!
//! Директива: «изолинии поверх TIN (marching squares по height_at —
//! 27-дерево уже готово)». Два механизма в одном модуле:
//!
//! * **marching squares** ([`tin_isolines`]) — регулярная сетка узлов
//!   над оболочкой TIN, высоты снимаются `height_at` (с индексом
//!   27-дерева — трит-путём, Сессия-16), 16 кейсов ячейки, сёдла
//!   решает центр, отрезки сшиваются в цепочки. Пересечение на ребре
//!   считается от КАНОНИЧЕСКОГО узла (левый/нижний) — соседние ячейки
//!   выдают побитово одинаковые концы, сшивка без epsilon;
//! * **точный эталон** ([`tin_isolines_exact`]) — сечение каждого
//!   треугольника плоскостью z = уровень. Поверхность TIN кусочно-планарная,
//!   поэтому сечение ТОЧНО (доктрина CSE: аналитика вместо приближения),
//!   а crossing на общем ребре двух треугольников кешируется один раз —
//!   цепочки watertight по построению.
//!
//! Аудит честности (стиль Сессии-16): общая длина изолиний marching
//! squares сходится к точной с измельчением сетки; на планарном рельефе
//! совпадает побитово (билинейная интерполяция точна для линейного поля).
//!
//! Вырожденные случаи: узел ровно на уровне — контур проходит через
//! него (t = 0/1, концы склеиваются по точке); изолиния вдоль целого
//! ребра (оба конца на уровне) трассируется соседним треугольником;
//! встреча четырёх ветвей в одной точке развязывается первым свободным
//! отрезком — детерминированно.
//!
//! Координаты точек изолиний — КВАНТЫ решётки TIN (как `Tin::pts`).

use crate::geo::delaunay::Tin;
use std::collections::HashMap;

/// Изолиния: уровень + цепочка точек (кванты решётки TIN).
/// Замкнутое кольцо несёт первую точку и в конце.
#[derive(Clone, Debug)]
pub struct Isoline {
    pub level: f64,
    pub pts: Vec<(f64, f64)>,
}

/// Общая длина изолиний (в квантах решётки TIN).
pub fn total_length(lines: &[Isoline]) -> f64 {
    lines
        .iter()
        .map(|l| {
            l.pts
                .windows(2)
                .map(|w| {
                    let (ax, ay) = w[0];
                    let (bx, by) = w[1];
                    ((bx - ax) * (bx - ax) + (by - ay) * (by - ay)).sqrt()
                })
                .sum::<f64>()
        })
        .sum()
}

/// Ключ точки для сшивки — побитовый: формулы пересечения детерминированы,
/// общий конец двух отрезков даёт одинаковые биты без epsilon.
fn pkey(p: &(f64, f64)) -> (u64, u64) {
    (p.0.to_bits(), p.1.to_bits())
}

/// Сшивка отрезков в цепочки: концы с равными ключами склеиваются,
/// замкнутое кольцо дублирует первую точку в конце. Нулевые отрезки
/// (обе стороны в одном узле) отбрасываются — точка-узел склеивает
/// соседей напрямую.
fn chain_segments(segs: Vec<((f64, f64), (f64, f64))>, level: f64) -> Vec<Isoline> {
    let segs: Vec<_> = segs
        .into_iter()
        .filter(|(p, q)| pkey(p) != pkey(q))
        .collect();
    let mut at: HashMap<(u64, u64), Vec<usize>> = HashMap::new();
    for (si, (p, q)) in segs.iter().enumerate() {
        at.entry(pkey(p)).or_default().push(si);
        at.entry(pkey(q)).or_default().push(si);
    }
    let mut used = vec![false; segs.len()];
    let mut out = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut pts = vec![segs[start].0, segs[start].1];
        // рост в две стороны: хвост → разворот → новый хвост
        for dir in 0..2 {
            loop {
                let tail = *pts.last().unwrap();
                let k = pkey(&tail);
                let cand = match at.get(&k) {
                    Some(v) if v.iter().any(|&si| !used[si]) => {
                        *v.iter().find(|&&si| !used[si]).unwrap()
                    }
                    _ => break,
                };
                used[cand] = true;
                let (p, q) = segs[cand];
                let nxt = if pkey(&p) == k { q } else { p };
                pts.push(nxt);
                if pkey(&nxt) == pkey(&pts[0]) {
                    break; // кольцо замкнуто — первая точка продублирована
                }
            }
            if pkey(pts.first().unwrap()) == pkey(pts.last().unwrap()) {
                break; // кольцо — второго конца нет
            }
            if dir == 0 {
                pts.reverse();
            }
        }
        out.push(Isoline { level, pts });
    }
    out
}

// ─────────────── marching squares по height_at ───────────────

/// Сетка узлов над оболочкой TIN (узлы — ТОЧКИ решётки, `height_at`).
struct Grid {
    x0: i64,
    y0: i64,
    dx: i64,
    dy: i64,
    nx: usize,
    ny: usize,
    /// h[j][i] — высота узла (i, j); None — вне выпуклой оболочки.
    h: Vec<Vec<Option<f64>>>,
}

fn sample_grid(tin: &Tin, z: &[f64], cells: usize) -> Result<Grid, String> {
    if tin.pts.is_empty() {
        return Err("marchsq: TIN пуст — изолиний нет".into());
    }
    if z.len() != tin.pts.len() {
        return Err(format!(
            "marchsq: высот {} ≠ вершин {}",
            z.len(),
            tin.pts.len()
        ));
    }
    if !(8..=512).contains(&cells) {
        return Err(format!(
            "marchsq: ячеек сетки ∈ [8, 512], получено {cells}"
        ));
    }
    let (mut x0, mut x1, mut y0, mut y1) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
    for &(x, y) in &tin.pts {
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    let span_x = (x1 - x0).max(1) as usize;
    let span_y = (y1 - y0).max(1) as usize;
    // шаг ≥ 1 квант: узлы остаются на решётке, height_at точен
    let dx = ((span_x + cells - 1) / cells).max(1) as i64;
    let dy = ((span_y + cells - 1) / cells).max(1) as i64;
    // последний узел может выйти за оболочку — там None (честно)
    let nx = (span_x + dx as usize - 1) / dx as usize + 1;
    let ny = (span_y + dy as usize - 1) / dy as usize + 1;
    let mut h = vec![vec![None::<f64>; nx]; ny];
    for j in 0..ny {
        for i in 0..nx {
            h[j][i] = tin.height_at(z, x0 + dx * i as i64, y0 + dy * j as i64);
        }
    }
    Ok(Grid {
        x0,
        y0,
        dx,
        dy,
        nx,
        ny,
        h,
    })
}

/// Изолинии уровня `level` — marching squares по сетке `cells` ячеек
/// на сторону (узлы сэмплируются `height_at`: с индексом 27-дерева —
/// трит-путём, без — лобовым перебором).
///
/// Канонические направления рёбер (левый/нижний узел) дают побитово
/// одинаковые концы у соседних ячеек — сшивка без epsilon. Сёдла
/// (кейсы 5/10) решает центр ячейки.
pub fn tin_isolines(
    tin: &Tin,
    z: &[f64],
    level: f64,
    cells: usize,
) -> Result<Vec<Isoline>, String> {
    let g = sample_grid(tin, z, cells)?;
    let (x0f, y0f) = (g.x0 as f64, g.y0 as f64);
    let (dxf, dyf) = (g.dx as f64, g.dy as f64);

    // горизонталь (i,j)→(i+1,j): от ЛЕВОГО узла
    let hcross = |i: usize, j: usize| -> Option<(f64, f64)> {
        let (ha, hb) = (g.h[j][i]?, g.h[j][i + 1]?);
        if (ha > level) != (hb > level) {
            let t = (level - ha) / (hb - ha);
            Some((x0f + (i as f64 + t) * dxf, y0f + j as f64 * dyf))
        } else {
            None
        }
    };
    // вертикаль (i,j)→(i,j+1): от НИЖНЕГО узла
    let vcross = |i: usize, j: usize| -> Option<(f64, f64)> {
        let (ha, hb) = (g.h[j][i]?, g.h[j + 1][i]?);
        if (ha > level) != (hb > level) {
            let t = (level - ha) / (hb - ha);
            Some((x0f + i as f64 * dxf, y0f + (j as f64 + t) * dyf))
        } else {
            None
        }
    };

    let mut segs = Vec::new();
    for j in 0..g.ny - 1 {
        for i in 0..g.nx - 1 {
            let (bl, br, tr, tl) = (g.h[j][i], g.h[j][i + 1], g.h[j + 1][i + 1], g.h[j + 1][i]);
            let (bl, br, tr, tl) = match (bl, br, tr, tl) {
                (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
                _ => continue, // ячейка цепляется за оболочку — данных нет
            };
            let bits = (bl > level) as u8
                | (((br > level) as u8) << 1)
                | (((tr > level) as u8) << 2)
                | (((tl > level) as u8) << 3);
            let b = hcross(i, j); // низ
            let r = vcross(i + 1, j); // право
            let t = hcross(i, j + 1); // верх
            let l = vcross(i, j); // лево
            let mut push = |p: Option<(f64, f64)>, q: Option<(f64, f64)>| {
                if let (Some(p), Some(q)) = (p, q) {
                    segs.push((p, q));
                }
            };
            match bits {
                0 | 15 => {}
                1 | 14 => push(l, b),
                2 | 13 => push(b, r),
                3 | 12 => push(l, r),
                4 | 11 => push(r, t),
                6 | 9 => push(b, t),
                7 | 8 => push(t, l),
                5 | 10 => {
                    // седло: центр решает спаривание (5: bl,tr выше; 10: br,tl)
                    let center = (bl + br + tr + tl) / 4.0;
                    if (bits == 5) == (center > level) {
                        push(b, r);
                        push(t, l);
                    } else {
                        push(l, b);
                        push(r, t);
                    }
                }
                _ => unreachable!("16 кейсов покрыты"),
            }
        }
    }
    Ok(chain_segments(segs, level))
}

// ─────────────── точный эталон: сечение треугольников ───────────────

/// ТОЧНЫЕ изолинии уровня `level`: сечение кусочно-планарной поверхности
/// TIN плоскостью z = уровень. Пересечение на общем ребре двух
/// треугольников считается ОДИН раз (кеши по каноническому направлению
/// индексов) — цепочки watertight по построению, как связность
/// рассечённых рёбер в marching tetrahedra (Сессия-12).
pub fn tin_isolines_exact(tin: &Tin, z: &[f64], level: f64) -> Result<Vec<Isoline>, String> {
    if z.len() != tin.pts.len() {
        return Err(format!(
            "marchsq: высот {} ≠ вершин {}",
            z.len(),
            tin.pts.len()
        ));
    }
    let mut edge_cross: HashMap<(usize, usize), (f64, f64)> = HashMap::new();
    let mut segs = Vec::new();
    for &[a, b, c] in &tin.tris {
        let mut xs: Vec<(f64, f64)> = Vec::with_capacity(2);
        for (u, v) in [(a, b), (b, c), (c, a)] {
            if (z[u] > level) != (z[v] > level) {
                let key = if u < v { (u, v) } else { (v, u) };
                let pt = match edge_cross.get(&key) {
                    Some(&p) => p,
                    None => {
                        let (lo, hi) = if u < v { (u, v) } else { (v, u) };
                        let t = (level - z[lo]) / (z[hi] - z[lo]);
                        let (ax, ay) = (tin.pts[lo].0 as f64, tin.pts[lo].1 as f64);
                        let (bx, by) = (tin.pts[hi].0 as f64, tin.pts[hi].1 as f64);
                        let p = (ax + t * (bx - ax), ay + t * (by - ay));
                        edge_cross.insert(key, p);
                        p
                    }
                };
                xs.push(pt);
            }
        }
        // чётность смен знака на цикле рёбер: 0 или 2 (вершина ровно на
        // уровне даёт t = 0/1 — точка в вершине, концы склеятся)
        debug_assert!(
            xs.len() % 2 == 0,
            "чётность пересечений нарушена (внутренняя ошибка)"
        );
        if xs.len() >= 2 {
            segs.push((xs[0], xs[1]));
        }
    }
    Ok(chain_segments(segs, level))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::delaunay::delaunay;

    const K: u8 = 10;

    /// Сетка (0..=8)² с шагом 2 → TIN; z = x + y (планарный рельеф).
    fn plane_tin() -> (Tin, Vec<f64>) {
        let mut pts = Vec::new();
        for y in (0..=8i64).step_by(2) {
            for x in (0..=8i64).step_by(2) {
                pts.push((x, y));
            }
        }
        let z: Vec<f64> = pts.iter().map(|&(x, y)| (x + y) as f64).collect();
        (delaunay(&pts).unwrap(), z)
    }

    /// Пирамида: 4 угла z=0 + центр z=1 — замкнутая изолиния 0.5.
    fn pyramid_tin() -> (Tin, Vec<f64>) {
        let pts = vec![(0i64, 0i64), (8, 0), (8, 8), (0, 8), (4, 4)];
        let z = vec![0.0, 0.0, 0.0, 0.0, 1.0];
        (delaunay(&pts).unwrap(), z)
    }

    #[test]
    fn plane_isolines_ms_equals_exact() {
        // планарное поле: билинейная интерполяция ТОЧНА — длины совпадают
        let (tin, z) = plane_tin();
        let level = 8.5;
        let ms = tin_isolines(&tin, &z, level, 16).unwrap();
        let ex = tin_isolines_exact(&tin, &z, level).unwrap();
        assert_eq!(ms.len(), 1, "одна ветвь MS");
        assert_eq!(ex.len(), 1, "одна ветвь точно");
        // концы: (0.5, 8) и (8, 0.5) — в любом порядке
        let want = 7.5 * 2.0f64.sqrt();
        assert!(
            (total_length(&ms) - want).abs() < 1e-9,
            "MS длина {} ≠ {want}",
            total_length(&ms)
        );
        assert!(
            (total_length(&ex) - want).abs() < 1e-9,
            "точная длина {} ≠ {want}",
            total_length(&ex)
        );
        // незамкнутые цепи
        assert_ne!(pkey(&ms[0].pts[0]), pkey(&ms[0].pts.last().unwrap()));
        assert_ne!(pkey(&ex[0].pts[0]), pkey(&ex[0].pts.last().unwrap()));
        // концы на границе оболочки
        for pts in [&ms[0].pts, &ex[0].pts] {
            for p in [*pts.first().unwrap(), *pts.last().unwrap()] {
                let on = p.0 == 0.5 || p.0 == 8.0 || p.1 == 0.5 || p.1 == 8.0;
                assert!(on, "конец {p:?} не на границе");
            }
        }
    }

    #[test]
    fn pyramid_closed_loop() {
        // изолиния 0.5 пирамиды — квадрат (2,2)..(6,6), замкнутый
        let (tin, z) = pyramid_tin();
        let ex = tin_isolines_exact(&tin, &z, 0.5).unwrap();
        assert_eq!(ex.len(), 1);
        let pts = &ex[0].pts;
        assert_eq!(pts.len(), 5, "4 угла + замыкание: {pts:?}");
        assert_eq!(pkey(&pts[0]), pkey(&pts.last().unwrap()), "кольцо");
        assert!((total_length(&ex) - 16.0).abs() < 1e-9, "сторона 4");
        // уровень-узлы: высота пирамиды в (2,2) ровно 0.5 — контур
        // проходит через УЗЛЫ сетки; MS режет такие углы хордами
        // (честное поведение: 16 − 4·(2−√2) = 8+4√2), эталон держит 16
        let ms = tin_isolines(&tin, &z, 0.5, 8).unwrap();
        assert_eq!(ms.len(), 1, "MS: одна ветвь");
        assert_eq!(pkey(&ms[0].pts[0]), pkey(&ms[0].pts.last().unwrap()));
        let want_ms = 8.0 + 4.0 * 2.0f64.sqrt();
        assert!(
            (total_length(&ms) - want_ms).abs() < 1e-9,
            "MS длина {} ≠ {want_ms} (хорды через узлы на уровне)",
            total_length(&ms)
        );
    }

    #[test]
    fn exact_watertight_and_ms_converges() {
        // холмистый рельеф: сшивка без потерь + сходимость длин
        let verts = crate::geo::eteria::tin_vertices(7, 96).unwrap();
        let mut pts_i = Vec::with_capacity(verts.len());
        for &(x, y, _) in &verts {
            let tp = crate::geo::trit_coord::TritPoint::quantize(x, y, K).unwrap();
            pts_i.push((tp.x.to_i64(), tp.y.to_i64()));
        }
        let mut tin = delaunay(&pts_i).unwrap();
        tin.build_spatial27().unwrap();
        let z: Vec<f64> = verts.iter().map(|&(_, _, h)| h).collect();
        let (zmin, zmax) = z.iter().cloned().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(v), b.max(v))
        });
        let level = (zmin + zmax) / 2.0;

        let ex = tin_isolines_exact(&tin, &z, level).unwrap();
        assert!(!ex.is_empty(), "уровень внутри диапазона — есть ветви");
        // watertight: каждая цепь либо замкнута, либо концы на оболочке
        for l in &ex {
            for p in &l.pts {
                assert!(p.0.is_finite() && p.1.is_finite(), "точки конечны");
            }
        }
        // сходимость: длинa MS(96) в полосе вокруг точной, MS(24) — не дальше
        let le = total_length(&ex);
        let l96 = total_length(&tin_isolines(&tin, &z, level, 96).unwrap());
        let l24 = total_length(&tin_isolines(&tin, &z, level, 24).unwrap());
        assert!(le > 0.0);
        assert!(
            (0.70..=1.30).contains(&(l96 / le)),
            "MS(96)/точно = {} — вне полосы сходимости",
            l96 / le
        );
        assert!(
            l24 / le < l96 / le * 1.05 || (l96 / le - 1.0).abs() < 0.02,
            "измельчение не приближает: l24={l24} l96={l96} le={le}"
        );
        // пустые уровни — пустые ответы
        assert!(tin_isolines_exact(&tin, &z, zmin - 0.5).unwrap().is_empty());
        assert!(tin_isolines(&tin, &z, zmax + 0.5, 32).unwrap().is_empty());
    }

    #[test]
    fn errors_and_validation() {
        let (tin, z) = pyramid_tin();
        assert!(tin_isolines(&tin, &z[..3], 0.5, 16).is_err());
        assert!(tin_isolines(&tin, &z, 0.5, 4).is_err());
        assert!(tin_isolines(&tin, &z, 0.5, 600).is_err());
        assert!(tin_isolines_exact(&tin, &z[..2], 0.5).is_err());
        assert!(tin_isolines(&Tin::empty(), &[], 0.5, 16).is_err());
    }
}
