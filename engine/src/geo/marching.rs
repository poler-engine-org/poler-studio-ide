//! Marching cubes в тритной философии poler-engine (Сессия-12).
//!
//! Изоповерхность скалярного поля f: ℝ³ → ℝ на регулярной воксельной
//! решётке. Вместо классических 256-кейсных таблиц Лоренсена–Клина
//! (4 КиБ «магических чисел», которые невозможно проверить) используется
//! **marching tetrahedra** с выводом таблицы из первых принципов:
//!
//! * каждый куб делится на 6 тетраэдров веером вокруг главной диагонали;
//! * диагональ выбирается по чётности куба (x+y+z) — соседние кубы
//!   согласованы, поверхность замкнута (watertight);
//! * у тетраэдра 4 вершины → 16 случаев: 0/4 внутри — пусто,
//!   1/3 внутри — один треугольник, 2/2 — четырёхугольник (2 треугольника).
//!   Связность выводится тривиально из множества «рассечённых» рёбер;
//! * ориентация нормалей не зашита в таблицу, а **исправляется на лету**
//!   градиентным тестом: нормаль смотрит ОТ области «внутри» (f > level).
//!   Самокорректирующаяся таблица — ошибки winding исключены по построению.
//!
//! Точная математика: внутренность = строго `f > level`, поэтому на
//! рассечённом ребре f_in ≠ f_out и интерполяция
//! `p_in + (p_out − p_in)·t`, `t = (level − f_in)/(f_out − f_in)` безопасна.
//!
//! Мост к viz-ядру (Сессии-8/9/10): `viz_iso3` рисует треугольники
//! ортоскопической проекцией с painter's-алгоритмом — та же сцена,
//! что и `viz_surf`, но поверхность замкнутая (объём, а не график).
//!
//! Зачем движку изоповерхности:
//! * рельеф Этерии (TIN Spade) → объёмный рендер слоёв;
//! * фредеритовые жилы — 3D-поля потоков, изолируем уровень насыщения;
//! * квантовые поля |ψ(x,y,z)|² — «облака вероятности».

use std::fmt;

/// Точка в пространстве узлов решётки (целочисленные индексы как f64 —
/// точны в double, интерполяция детерминирована).
#[derive(Clone, Copy, PartialEq)]
pub struct Vec3(pub f64, pub f64, pub f64);

impl fmt::Debug for Vec3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:.3},{:.3},{:.3})", self.0, self.1, self.2)
    }
}

impl Vec3 {
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3(self.0 - o.0, self.1 - o.1, self.2 - o.2)
    }
    fn cross(a: Vec3, b: Vec3) -> Vec3 {
        Vec3(
            a.1 * b.2 - a.2 * b.1,
            a.2 * b.0 - a.0 * b.2,
            a.0 * b.1 - a.1 * b.0,
        )
    }
    fn dot(a: Vec3, b: Vec3) -> f64 {
        a.0 * b.0 + a.1 * b.1 + a.2 * b.2
    }
}

/// Треугольник изоповерхности.
pub type Tri = [Vec3; 3];

/// Воксельная решётка `nz × ny × nx` (срезы по z). `data[iz·ny·nx + iy·nx + ix]`.
pub struct VoxelGrid {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub data: Vec<f64>,
}

impl VoxelGrid {
    /// Из срезов: `slices[iz][iy][ix]`. Все срезы одинаковой формы,
    /// ≥ 2×2×2 узлов, значения конечны.
    pub fn from_slices(slices: &[Vec<Vec<f64>>]) -> Result<Self, String> {
        if slices.len() < 2 {
            return Err(format!(
                "marching: нужно ≥ 2 z-срезов, получено {}",
                slices.len()
            ));
        }
        let nz = slices.len();
        let ny = slices[0].len();
        if ny < 2 {
            return Err(format!("marching: срез 0 имеет {} строк (нужно ≥ 2)", ny));
        }
        let nx = slices[0][0].len();
        if nx < 2 {
            return Err(format!(
                "marching: срез 0 имеет {} столбцов (нужно ≥ 2)",
                nx
            ));
        }
        let mut data = Vec::with_capacity(nx * ny * nz);
        for (iz, s) in slices.iter().enumerate() {
            if s.len() != ny {
                return Err(format!(
                    "marching: срез {iz} имеет {} строк, ожидается {ny}",
                    s.len()
                ));
            }
            for (iy, row) in s.iter().enumerate() {
                if row.len() != nx {
                    return Err(format!(
                        "marching: срез {iz} строка {iy}: {} столбцов, ожидается {nx}",
                        row.len()
                    ));
                }
                for (ix, &v) in row.iter().enumerate() {
                    if !v.is_finite() {
                        return Err(format!(
                            "marching: значение ({iz},{iy},{ix}) не конечно"
                        ));
                    }
                    data.push(v);
                }
            }
        }
        Ok(VoxelGrid { nx, ny, nz, data })
    }

    #[inline]
    pub fn get(&self, ix: usize, iy: usize, iz: usize) -> f64 {
        self.data[iz * self.ny * self.nx + iy * self.nx + ix]
    }

    /// (min, max) поля — для уровня по умолчанию (середина).
    pub fn minmax(&self) -> (f64, f64) {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for &v in &self.data {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        (lo, hi)
    }
}

/// Рассечённое ребро тетраэдра: угол куба a (внутри) и b (снаружи).
#[derive(Clone, Copy)]
struct CutEdge {
    a: u8, // угол куба, ВНУТРИ (f > level)
    b: u8, // угол куба, снаружи
}

/// Смещения углов куба: угол c → (c&1, c>>1&1, c>>2&1) по (x, y, z).
#[inline]
fn corner_off(c: u8) -> (usize, usize, usize) {
    ((c & 1) as usize, ((c >> 1) & 1) as usize, ((c >> 2) & 1) as usize)
}

/// Тетраэдры веера вокруг главной диагонали 0–7 куба.
/// ВАЖНО (грабля Сессии-12): веер ОДИН для всех кубов — паритетное
/// чередование диагоналей (0–7 / 1–6) рассекает общие грани соседей
/// ПО-РАЗНОМУ и рвёт поверхность (588 дырок на сфере 14³). Однородный
/// веер задаёт на каждой грани «ко-диагональ» (min → противополжный угол
/// грани) — она зависит только от ГРАНИ, оба соседа согласованы,
/// поверхность замкнута (тест sphere_watertight_euler).
#[inline]
fn tetra_fan() -> [[u8; 4]; 6] {
    [
        [0, 1, 3, 7],
        [0, 3, 2, 7],
        [0, 2, 6, 7],
        [0, 6, 4, 7],
        [0, 4, 5, 7],
        [0, 5, 1, 7],
    ]
}

/// Изоповерхность поля на уровне `level`: треугольники в координатах
/// узлов решётки. Нормали смотрят ОТ области f > level (наружу внутренности).
pub fn marching_tetrahedra(grid: &VoxelGrid, level: f64) -> Vec<Tri> {
    let mut tris: Vec<Tri> = Vec::new();
    if grid.nx < 2 || grid.ny < 2 || grid.nz < 2 {
        return tris;
    }
    for iz in 0..grid.nz - 1 {
        for iy in 0..grid.ny - 1 {
            for ix in 0..grid.nx - 1 {
                let mut f = [0.0f64; 8];
                let mut p = [[0.0f64; 3]; 8];
                for c in 0u8..8 {
                    let (dx, dy, dz) = corner_off(c);
                    let (x, y, z) = (ix + dx, iy + dy, iz + dz);
                    f[c as usize] = grid.get(x, y, z);
                    p[c as usize] = [x as f64, y as f64, z as f64];
                }
                let fan = tetra_fan();
                for tet in fan {
                    let mut mask = 0u8;
                    for (i, &c) in tet.iter().enumerate() {
                        if f[c as usize] > level {
                            mask |= 1 << i;
                        }
                    }
                    if mask == 0 || mask == 0b1111 {
                        continue;
                    }
                    // Рассечённые рёбра: пары (внутри, снаружи).
                    let mut cuts: Vec<CutEdge> = Vec::with_capacity(4);
                    for i in 0..4usize {
                        for j in i + 1..4usize {
                            let (mi, mj) = (mask >> i & 1, mask >> j & 1);
                            if mi != mj {
                                let (a, b) = if mi == 1 { (tet[i], tet[j]) } else { (tet[j], tet[i]) };
                                cuts.push(CutEdge { a, b });
                            }
                        }
                    }
                    // Вершины на рассечённых рёбрах (интерполяция).
                    let verts: Vec<Vec3> = cuts
                        .iter()
                        .map(|ce| {
                            let (a, b) = (ce.a as usize, ce.b as usize);
                            let (fa, fb) = (f[a], f[b]);
                            let t = if fa == fb {
                                0.5
                            } else {
                                (level - fa) / (fb - fa)
                            };
                            Vec3(
                                p[a][0] + (p[b][0] - p[a][0]) * t,
                                p[a][1] + (p[b][1] - p[a][1]) * t,
                                p[a][2] + (p[b][2] - p[a][2]) * t,
                            )
                        })
                        .collect();
                    // Направление «внутрь»: центроид внутри − центроид снаружи.
                    let mut cin = [0.0f64; 3];
                    let mut cout = [0.0f64; 3];
                    let (mut nin, mut nout) = (0.0f64, 0.0f64);
                    for (i, &c) in tet.iter().enumerate() {
                        let cc = c as usize;
                        if mask >> i & 1 == 1 {
                            for k in 0..3 {
                                cin[k] += p[cc][k];
                            }
                            nin += 1.0;
                        } else {
                            for k in 0..3 {
                                cout[k] += p[cc][k];
                            }
                            nout += 1.0;
                        }
                    }
                    let inward = Vec3(
                        cin[0] / nin - cout[0] / nout,
                        cin[1] / nin - cout[1] / nout,
                        cin[2] / nin - cout[2] / nout,
                    );
                    // Нормаль ДОЛЖНА смотреть от внутренности: n·inward < 0.
                    let mut fix = |v0: Vec3, v1: Vec3, v2: Vec3| -> Tri {
                        let n = Vec3::cross(v1.sub(v0), v2.sub(v0));
                        if Vec3::dot(n, inward) > 0.0 {
                            [v0, v2, v1]
                        } else {
                            [v0, v1, v2]
                        }
                    };
                    match cuts.len() {
                        3 => tris.push(fix(verts[0], verts[1], verts[2])),
                        4 => {
                            let quad = quad_cycle(&cuts);
                            let q: Vec<Vec3> = quad.iter().map(|&ci| verts[ci]).collect();
                            tris.push(fix(q[0], q[1], q[2]));
                            tris.push(fix(q[0], q[2], q[3]));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    tris
}

/// Цикл из 4 рассечённых рёбер (кейс 2+2). Рёбра cuts — пары углов куба;
/// смежные рёбра цикла имеют общий угол. Возврат: индексы в cuts по циклу.
fn quad_cycle(cuts: &[CutEdge]) -> Vec<usize> {
    let key = |c: &CutEdge| (c.a.min(c.b), c.a.max(c.b));
    let mut path: Vec<usize> = vec![0];
    let mut used = [false; 4];
    used[0] = true;
    let k0 = key(&cuts[0]);
    let (mut cur_end, cur_start) = (k0.1, k0.0);
    while path.len() < 4 {
        let mut advanced = false;
        for i in 0..4usize {
            if used[i] {
                continue;
            }
            let k = key(&cuts[i]);
            if k.0 == cur_end || k.1 == cur_end {
                cur_end = if k.0 == cur_end { k.1 } else { k.0 };
                used[i] = true;
                path.push(i);
                advanced = true;
                break;
            }
        }
        if !advanced {
            let mut rest: Vec<usize> = (0..4).filter(|i| !used[*i]).collect();
            path.append(&mut rest);
            return path;
        }
    }
    if cur_end != cur_start {
        path.reverse();
    }
    path
}

/// Нормаль треугольника (ненормированная).
pub fn tri_normal(t: &Tri) -> Vec3 {
    Vec3::cross(t[1].sub(t[0]), t[2].sub(t[0]))
}

/// Площадь треугольника.
pub fn tri_area(t: &Tri) -> f64 {
    let n = tri_normal(t);
    Vec3::dot(n, n).sqrt() / 2.0
}

// ────────────────────────── тесты ──────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    fn sphere_grid(n: usize, r: f64) -> VoxelGrid {
        let c = (n - 1) as f64 / 2.0;
        let mut slices = vec![vec![vec![0.0f64; n]; n]; n];
        for iz in 0..n {
            for iy in 0..n {
                for ix in 0..n {
                    let d = (
                        (ix as f64 - c).powi(2)
                            + (iy as f64 - c).powi(2)
                            + (iz as f64 - c).powi(2)
                    )
                    .sqrt();
                    slices[iz][iy][ix] = r - d; // внутри: f > 0
                }
            }
        }
        VoxelGrid::from_slices(&slices).unwrap()
    }

    fn topology(tris: &[Tri]) -> (i64, i64, i64, i64) {
        let key = |v: &Vec3| {
            format!(
                "{:.8},{:.8},{:.8}",
                (v.0 * 1e8).round() / 1e8,
                (v.1 * 1e8).round() / 1e8,
                (v.2 * 1e8).round() / 1e8
            )
        };
        let mut edges: HashMap<(String, String), u32> = HashMap::new();
        let mut verts: HashSet<String> = HashSet::new();
        for t in tris {
            for k in 0..3 {
                let a = key(&t[k]);
                let b = key(&t[(k + 1) % 3]);
                verts.insert(a.clone());
                verts.insert(b.clone());
                let e = if a < b { (a, b) } else { (b, a) };
                *edges.entry(e).or_insert(0) += 1;
            }
        }
        let bad = edges.values().filter(|&&c| c != 2).count() as i64;
        let chi = verts.len() as i64 - edges.len() as i64 + tris.len() as i64;
        (verts.len() as i64, edges.len() as i64, tris.len() as i64, bad)
    }

    #[test]
    fn sphere_watertight_euler() {
        // Замкнутость: каждое ребро — ровно в двух треугольниках;
        // Эйлер: V − E + F = 2 (одна сфера).
        let n = 14usize;
        let g = sphere_grid(n, (n - 1) as f64 * 0.42);
        let tris = marching_tetrahedra(&g, 0.0);
        assert!(!tris.is_empty(), "сфера должна дать треугольники");
        let (v, e, f, bad) = topology(&tris);
        assert_eq!(bad, 0, "рёбра с кратностью ≠ 2 — разрывы поверхности");
        assert_eq!(v - e + f, 2, "χ = V−E+F должен быть 2 (сфера), получен {}", v - e + f);
    }

    #[test]
    fn sphere_normals_outward() {
        let n = 12usize;
        let g = sphere_grid(n, (n - 1) as f64 * 0.4);
        let tris = marching_tetrahedra(&g, 0.0);
        let c = Vec3(
            (n - 1) as f64 / 2.0,
            (n - 1) as f64 / 2.0,
            (n - 1) as f64 / 2.0,
        );
        for t in &tris {
            let centroid = Vec3(
                (t[0].0 + t[1].0 + t[2].0) / 3.0,
                (t[0].1 + t[1].1 + t[2].1) / 3.0,
                (t[0].2 + t[1].2 + t[2].2) / 3.0,
            );
            let nrm = tri_normal(t);
            let out = Vec3(centroid.0 - c.0, centroid.1 - c.1, centroid.2 - c.2);
            assert!(
                Vec3::dot(nrm, out) > 0.0,
                "нормаль внутрь: n={:?} out={:?}",
                nrm,
                out
            );
        }
    }

    #[test]
    fn no_degenerate_triangles() {
        let g = sphere_grid(12, 11.0 * 0.4);
        let tris = marching_tetrahedra(&g, 0.0);
        assert!(!tris.is_empty());
        for t in &tris {
            assert!(tri_area(t) > 1e-12, "вырожденный треугольник {:?}", t);
        }
    }

    #[test]
    fn empty_and_full() {
        let slices = vec![
            vec![vec![1.0, 1.0], vec![1.0, 1.0]],
            vec![vec![1.0, 1.0], vec![1.0, 1.0]],
        ];
        let g = VoxelGrid::from_slices(&slices).unwrap();
        assert!(marching_tetrahedra(&g, 0.0).is_empty(), "всё внутри — пусто");
        assert!(marching_tetrahedra(&g, 2.0).is_empty(), "всё снаружи — пусто");
    }

    #[test]
    fn two_blobs_two_components() {
        // Два далёких шара → χ = 4 (две замкнутые компоненты).
        let n = 16usize;
        let c = (n - 1) as f64 / 2.0;
        let mut slices = vec![vec![vec![0.0f64; n]; n]; n];
        let centers = [(c * 0.4, c, c), (c * 1.6, c, c)];
        for iz in 0..n {
            for iy in 0..n {
                for ix in 0..n {
                    // max ЗНАЧЕНИЙ БЛОБОВ (не с нулём! грабля Сессии-12:
                    // инициализация 0.0 + max() превращала всю внешнюю
                    // область в f=0=level → вырожденные треугольники).
                    let mut v = f64::NEG_INFINITY;
                    for &(cx, cy, cz) in &centers {
                        let d = (
                            (ix as f64 - cx).powi(2)
                                + (iy as f64 - cy).powi(2)
                                + (iz as f64 - cz).powi(2)
                        )
                        .sqrt();
                        v = v.max(n as f64 * 0.14 - d);
                    }
                    slices[iz][iy][ix] = v;
                }
            }
        }
        let g = VoxelGrid::from_slices(&slices).unwrap();
        let tris = marching_tetrahedra(&g, 0.0);
        let (v, e, f, bad) = topology(&tris);
        assert_eq!(bad, 0, "watertight нарушен для двух шаров");
        assert_eq!(v - e + f, 4, "две сферы: χ = 4, получен {}", v - e + f);
    }

    #[test]
    fn plane_slab_between_levels() {
        // Поле, переходящее уровень на одной плоскости z: f = z.
        let slices = vec![
            vec![vec![-1.0, -1.0, -1.0], vec![-1.0, -1.0, -1.0]],
            vec![vec![0.5, 0.5, 0.5], vec![0.5, 0.5, 0.5]],
            vec![vec![2.0, 2.0, 2.0], vec![2.0, 2.0, 2.0]],
        ];
        let g = VoxelGrid::from_slices(&slices).unwrap();
        let tris = marching_tetrahedra(&g, 0.0);
        assert!(!tris.is_empty());
        // Интерполяция: между слоями f=−1 (z=0) и f=0.5 (z=1) уровень 0
        // даёт z = t·1, t = (0−(−1))/(0.5−(−1)) = 2/3.
        for t in &tris {
            for v in t {
                assert!(
                    (v.2 - 2.0 / 3.0).abs() < 1e-9,
                    "вершина вне уровня: {:?}",
                    v
                );
            }
        }
    }
}
