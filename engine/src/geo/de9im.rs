//! DE-9IM на тритах — топологическая матрица пересечений (Сессия-12).
//!
//! Порт якоря `geo.poler::geo-main/geo/src/algorithm/relate/mod.rs`
//! (georust/geo, `IntersectionMatrix`) на тритное ядро. Матрица OGC 9-IM:
//! строки — Interior / Boundary / Exterior геометрии A, столбцы — B.
//!
//! ## Тритная упаковка (теория Сессии-11)
//!
//! Классическая ячейка хранит РАЗМЕРНОСТЬ пересечения dim ∈ {∅, 0, 1, 2} —
//! четыре состояния, байт в JTS. Тритное ядро сжимает её до трёх:
//!
//! | трит | смысл                          | dim-эквивалент |
//! |------|--------------------------------|----------------|
//! |  −1  | пересечение пусто (Exterior)   | ∅              |
//! |   0  | касание в точке (Boundary)     | dim 0          |
//! |  +1  | внутреннее пересечение (Int.)  | dim ≥ 1        |
//!
//! 9 ячеек × 1 трит = **одно сбалансированно-троичное 9-тритное число**
//! (3⁹ = 19 683 состояния, укладывается в i16). «6-тритный блок + резерв
//! 3 трита»: полный 12-тритный трайт-блок несёт матрицу (9) + флаги (3).
//! Трит = кутрит: S_z|m⟩ = mℏ|m⟩ — ячейка матрицы буквально хранит
//! проекцию спина-1 топологии.
//!
//! ## Точность
//!
//! Все предикаты считаются на тритной решётке (`trit_coord`): знаки
//! ориентации, принадлежность ребру, пересечения рёбер — ЦЕЛОЧИСЛЕННО,
//! без epsilon. Касания «вершина-в-вершину» и коллинеарные перекрытия
//! различаются честно (0 против +1), что f64-геометрии даётся только
//! адаптивной арифметикой Шевчука.

use crate::calc::trits::{Trit, Trits};
use crate::geo::trit_coord::{
    cross_sign, on_segment, point_in_ring, seg_classify, SegX, TritPoint,
};

/// Матрица DE-9IM: 9 тритов.
/// Индексация `cells[r*3 + c]`, r ∈ {0: I(A), 1: B(A), 2: E(A)},
/// c ∈ {0: I(B), 1: B(B), 2: E(B)}.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct De9im {
    pub cells: [Trit; 9],
}

macro_rules! cell {
    ($s:expr, $r:expr, $c:expr) => {
        $s.cells[$r * 3 + $c]
    };
}

impl De9im {
    pub const EMPTY: De9im = De9im {
        cells: [-1; 9],
    };

    #[inline]
    pub fn ii(&self) -> Trit {
        cell!(self, 0, 0)
    } // I(A) ∩ I(B)
    #[inline]
    pub fn ib(&self) -> Trit {
        cell!(self, 0, 1)
    } // I(A) ∩ B(B)
    #[inline]
    pub fn ie(&self) -> Trit {
        cell!(self, 0, 2)
    } // I(A) ∩ E(B)
    #[inline]
    pub fn bi(&self) -> Trit {
        cell!(self, 1, 0)
    } // B(A) ∩ I(B)
    #[inline]
    pub fn bb(&self) -> Trit {
        cell!(self, 1, 1)
    } // B(A) ∩ B(B)
    #[inline]
    pub fn be(&self) -> Trit {
        cell!(self, 1, 2)
    } // B(A) ∩ E(B)
    #[inline]
    pub fn ei(&self) -> Trit {
        cell!(self, 2, 0)
    } // E(A) ∩ I(B)
    #[inline]
    pub fn eb(&self) -> Trit {
        cell!(self, 2, 1)
    } // E(A) ∩ B(B)
    #[inline]
    pub fn ee(&self) -> Trit {
        cell!(self, 2, 2)
    } // E(A) ∩ E(B)

    /// Сбалансированно-троичный 9-тритный код матрицы
    /// (старшая цифра — II). ±(3⁹−1)/2 = ±9841.
    pub fn trit_code(&self) -> i64 {
        let mut v: i64 = 0;
        for &c in &self.cells {
            v = v * 3 + c as i64;
        }
        v
    }

    /// Из тритного кода (обратное к `trit_code`).
    pub fn from_trit_code(code: i64) -> Result<De9im, String> {
        if code.abs() > 9841 {
            return Err(format!("de9im: код {code} вне диапазона 9 тритов (±9841)"));
        }
        let mut cells = [0i8; 9];
        let mut n = code;
        for i in (0..9).rev() {
            let r = n.rem_euclid(3);
            cells[i] = match r {
                0 => 0,
                1 => 1,
                _ => -1,
            };
            n = (n - cells[i] as i64) / 3;
        }
        Ok(De9im { cells })
    }

    /// Человекочитаемая 9-символьная строка «−0+…» (строки I,B,E × столбцы).
    pub fn bal_string(&self) -> String {
        self.cells
            .iter()
            .map(|&c| match c {
                -1 => '−',
                0 => '0',
                _ => '+',
            })
            .collect()
    }

    /// OGC-подобная строка шаблона (F/T/*) — как в JTS relate().
    pub fn pattern_string(&self) -> String {
        self.cells
            .iter()
            .map(|&c| if c == -1 { 'F' } else { 'T' })
            .collect()
    }

    // ── предикаты OGC (3-значные ячейки: −1 = ∅, 0/+1 = непусто) ──

    /// Пересекаются ли (любое непустое пересечение кроме E∩E).
    pub fn intersects(&self) -> bool {
        !(self.disjoint())
    }

    /// Не имеют ни одной общей точки (кроме внешности).
    pub fn disjoint(&self) -> bool {
        self.ii() == -1 && self.ib() == -1 && self.bi() == -1 && self.bb() == -1
    }

    /// Совпадают: внутренности пересекаются, ни одна внутренность
    /// не выходит во внешность другой.
    pub fn equals(&self) -> bool {
        self.ii() != -1 && self.ie() == -1 && self.ei() == -1
    }

    /// A содержит B: внутренности пересекаются, B не выходит за A
    /// (ни внутренностью, ни границей).
    pub fn contains(&self) -> bool {
        self.ii() != -1 && self.ei() == -1 && self.eb() == -1
    }

    /// A внутри B (транспонированный contains).
    pub fn within(&self) -> bool {
        self.ii() != -1 && self.ie() == -1 && self.be() == -1
    }

    /// A покрывает B: B целиком в замыкании A (граница может касаться).
    pub fn covers(&self) -> bool {
        self.ei() == -1
            && self.eb() == -1
            && (self.ii() != -1 || self.bi() != -1 || self.bb() != -1)
    }

    /// A покрыт B (транспонированный covers).
    pub fn covered_by(&self) -> bool {
        self.ie() == -1
            && self.be() == -1
            && (self.ii() != -1 || self.ib() != -1 || self.bb() != -1)
    }

    /// Касаются: внутренности не пересекаются, но границы/граница-внутренность
    /// соприкасаются.
    pub fn touches(&self) -> bool {
        self.ii() == -1 && (self.bb() != -1 || self.ib() == 0 || self.bi() == 0)
    }

    /// Перекрываются (равная размерность): внутренности пересекаются
    /// «площадно», обе выглядывают наружу друг друга.
    pub fn overlaps(&self) -> bool {
        self.ii() == 1 && self.ie() == 1 && self.ei() == 1
    }

    /// Пересекаются накрест (для смешанных размерностей — линия сквозь
    /// полигон): внутренностное пересечение точечное, обе выглядывают.
    pub fn crosses(&self) -> bool {
        self.ii() == 0 && self.ie() != -1 && self.ei() != -1
    }

    /// Имена всех сработавших предикатов (для отчёта).
    pub fn predicate_names(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.intersects() {
            v.push("intersects");
        }
        if self.disjoint() {
            v.push("disjoint");
        }
        if self.equals() {
            v.push("equals");
        }
        if self.contains() {
            v.push("contains");
        }
        if self.within() {
            v.push("within");
        }
        if self.covers() {
            v.push("covers");
        }
        if self.covered_by() {
            v.push("covered_by");
        }
        if self.touches() {
            v.push("touches");
        }
        if self.overlaps() {
            v.push("overlaps");
        }
        if self.crosses() {
            v.push("crosses");
        }
        v
    }
}

// ────────────── вычисление relate на тритной решётке ──────────────

/// Точка↔точка.
pub fn relate_point_point(a: &TritPoint, b: &TritPoint) -> Result<De9im, String> {
    let mut m = De9im::EMPTY;
    if a == b {
        m.cells[0] = 0; // II: точка (dim 0)
    } else {
        m.cells[2] = 0; // IE: {a} во внешности B
        m.cells[6] = 0; // EI
    }
    m.cells[8] = 1; // EE: внешности площадно пересекаются
    Ok(m)
}

/// Точка↔кольцо (полигон без дыр).
pub fn relate_point_ring(p: &TritPoint, ring: &[TritPoint]) -> Result<De9im, String> {
    if ring.len() < 3 {
        return Err("de9im: кольцо < 3 вершин".into());
    }
    let pos = point_in_ring(p, ring);
    let mut m = De9im::EMPTY;
    match pos {
        1 => m.cells[0] = 0,   // II: точка внутри (dim 0)
        0 => m.cells[1] = 0,   // IB: точка на границе
        _ => m.cells[2] = 0,   // IE: точка снаружи
    }
    // E(A) — всё кроме точки; внутренность/граница кольца заведомо
    // площадно/линейно выходят за пределы одной точки.
    m.cells[6] = 1; // EI: внутренность кольца минус точка — площадь
    m.cells[7] = 1; // EB: граница кольца минус возможная точка — линия
    m.cells[8] = 1; // EE
    Ok(m)
}

/// Кольцо↔кольцо — центральный кейс. Точные свидетели:
/// * вершины, середины рёбер и СМЕЩЁННЫЕ ВНУТРЬ узлы (offset-точки на
///   один квант от середины ребра к внутренности кольца) — строго
///   внутри/снаружи другого кольца (offset-свидетель закрывает случай
///   совпадающих/вложенных без «нырков» — идентичные квадраты); 
/// * классифицированные пересечения рёбер (Proper/Touch/Collinear).
pub fn relate_ring_ring(a: &[TritPoint], b: &[TritPoint]) -> Result<De9im, String> {
    if a.len() < 3 || b.len() < 3 {
        return Err("de9im: кольцо < 3 вершин".into());
    }
    if a[0].k != b[0].k {
        return Err("de9im: кольца на решётках разного масштаба".into());
    }

    // ── свидетели ──
    let mut a_vert_in = false; // вершина A строго внутри B
    let mut a_vert_out = false; // вершина A строго снаружи B
    let mut a_mid_in = false; // середина ребра A строго внутри B
    let mut a_mid_out = false;
    let mut a_off_in = false; // offset-узел A (квант внутрь) строго внутри B
    let mut a_off_out = false;
    let mut b_vert_in = false;
    let mut b_vert_out = false;
    let mut b_mid_in = false;
    let mut b_mid_out = false;
    let mut b_off_in = false;
    let mut b_off_out = false;
    let mut proper = false; // собственное пересечение рёбер
    let mut touch = false; // касание границ в точке
    let mut collinear = false; // коллинеарное перекрытие границ

    for p in a {
        match point_in_ring(p, b) {
            1 => a_vert_in = true,
            -1 => a_vert_out = true,
            _ => {}
        }
    }
    for p in b {
        match point_in_ring(p, a) {
            1 => b_vert_in = true,
            -1 => b_vert_out = true,
            _ => {}
        }
    }
    // Середины рёбер И offset-узлы: середина = (p+q)/2 (обе пробы
    // floor/ceil); offset = середина + один квант решётки к ВНУТРЕННОСТИ
    // кольца (по знаку обхода) — прямой свидетель I(кольца) у ребра.
    let orient_a = ring_orientation(a)?;
    let orient_b = ring_orientation(b)?;
    for ring_is_a in [true, false] {
        let (r, other) = if ring_is_a { (a, b) } else { (b, a) };
        let orient = if ring_is_a { orient_a } else { orient_b };
        let n = r.len();
        for i in 0..n {
            let p = &r[i];
            let q = &r[(i + 1) % n];
            let mx = (p.x.to_i64() + q.x.to_i64());
            let my = (p.y.to_i64() + q.y.to_i64());
            for (dx, dy) in [(0i64, 0i64), (1, 0), (0, 1), (1, 1)] {
                let cx = (mx + dx) / 2;
                let cy = (my + dy) / 2;
                let mid = TritPoint {
                    x: Trits::from_i64(cx)?,
                    y: Trits::from_i64(cy)?,
                    k: p.k,
                };
                match point_in_ring(&mid, other) {
                    1 => {
                        if ring_is_a {
                            a_mid_in = true
                        } else {
                            b_mid_in = true
                        }
                    }
                    -1 => {
                        if ring_is_a {
                            a_mid_out = true
                        } else {
                            b_mid_out = true
                        }
                    }
                    _ => {}
                }
                // offset-проба: середина + квант к внутренности (только
                // для точной середины — dx=dy даёт ceil-вариант, пропускаем).
                if dx == 0 && dy == 0 {
                    let ddx = q.x.to_i64() - p.x.to_i64();
                    let ddy = q.y.to_i64() - p.y.to_i64();
                    if (ddx, ddy) != (0, 0) {
                        // левая нормаль к ребру; при CW-обходе внутренность
                        // справа — меняем знак.
                        let (mut nx, mut ny) = (-ddy, ddx);
                        if orient == -1 {
                            (nx, ny) = (ddy, -ddx);
                        }
                        let sx = nx.signum();
                        let sy = ny.signum();
                        if (sx, sy) != (0, 0) {
                            let off = TritPoint {
                                x: Trits::from_i64(cx + sx)?,
                                y: Trits::from_i64(cy + sy)?,
                                k: p.k,
                            };
                            match point_in_ring(&off, other) {
                                1 => {
                                    if ring_is_a {
                                        a_off_in = true
                                    } else {
                                        b_off_in = true
                                    }
                                }
                                -1 => {
                                    if ring_is_a {
                                        a_off_out = true
                                    } else {
                                        b_off_out = true
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
    // Пересечения рёбер.
    let (na, nb) = (a.len(), b.len());
    for i in 0..na {
        for j in 0..nb {
            let class = seg_classify(
                &a[i],
                &a[(i + 1) % na],
                &b[j],
                &b[(j + 1) % nb],
            );
            match class {
                SegX::Proper => proper = true,
                SegX::Touch => touch = true,
                SegX::Collinear => collinear = true,
                SegX::None => {}
            }
        }
    }

    // ── сборка матрицы ──
    let mut m = De9im::EMPTY;

    // II: внутренности площадно пересекаются ⟺ «нырок», накрытие или
    // совпадение (offset-свидетель).
    if a_vert_in || b_vert_in || proper || a_mid_in || b_mid_in || a_off_in || b_off_in {
        m.cells[0] = 1;
    }

    // IB: граница B проходит по внутренности A (линейно).
    if b_vert_in || b_mid_in || proper {
        m.cells[1] = 1;
    }
    // BI: симметрично.
    if a_vert_in || a_mid_in || proper {
        m.cells[3] = 1;
    }

    // BB: коллинеарное перекрытие (+1), точечное касание ИЛИ собственное
    // пересечение рёбер (0 — точка на обеих границах).
    if collinear {
        m.cells[4] = 1;
    } else if touch || proper {
        m.cells[4] = 0;
    }

    // IE: часть внутренности A снаружи B (площадно).
    if a_vert_out || a_mid_out || a_off_out || proper || b_vert_in || b_mid_in {
        m.cells[2] = 1;
    }
    // EI: симметрично.
    if b_vert_out || b_mid_out || b_off_out || proper || a_vert_in || a_mid_in {
        m.cells[6] = 1;
    }

    // BE: часть границы A снаружи B (линейно).
    if a_vert_out || a_mid_out || proper {
        m.cells[5] = 1;
    }
    // EB: симметрично.
    if b_vert_out || b_mid_out || proper {
        m.cells[7] = 1;
    }

    // EE: внешности двух ограниченных колец всегда пересекаются площадно.
    m.cells[8] = 1;

    Ok(m)
}

/// Универсальный вход: relate двух геометрий (точка = 1 вершина,
/// отрезок = 2, кольцо ≥ 3; кольцо замыкаем автоматически).
pub fn relate(a: &[TritPoint], b: &[TritPoint]) -> Result<De9im, String> {
    let a = close_ring(a);
    let b = close_ring(b);
    match (a.len(), b.len()) {
        (1, 1) => relate_point_point(&a[0], &b[0]),
        (1, _) => relate_point_ring(&a[0], &b),
        (_, 1) => {
            let m = relate_point_ring(&b[0], &a)?;
            Ok(transpose(&m))
        }
        _ => relate_ring_ring(&a, &b),
    }
}

/// Транспонирование матрицы (A↔B).
pub fn transpose(m: &De9im) -> De9im {
    let mut t = De9im::EMPTY;
    for r in 0..3 {
        for c in 0..3 {
            t.cells[c * 3 + r] = m.cells[r * 3 + c];
        }
    }
    t
}

/// Знак обхода кольца: +1 — CCW (положительная площадь), −1 — CW,
/// 0 — вырожденное. Точно: удвоенная площадь в i64 с проверкой переполнения.
fn ring_orientation(ring: &[TritPoint]) -> Result<Trit, String> {
    let n = ring.len();
    let mut area2: i64 = 0;
    for i in 0..n {
        let p = &ring[i];
        let q = &ring[(i + 1) % n];
        let t = p
            .x
            .to_i64()
            .checked_mul(q.y.to_i64())
            .and_then(|v| {
                q.x.to_i64()
                    .checked_mul(p.y.to_i64())
                    .and_then(|w| v.checked_sub(w))
            })
            .ok_or_else(|| "de9im: переполнение площади кольца".to_string())?;
        area2 = area2
            .checked_add(t)
            .ok_or_else(|| "de9im: переполнение площади кольца".to_string())?;
    }
    Ok(match area2 {
        x if x > 0 => 1,
        x if x < 0 => -1,
        _ => 0,
    })
}

/// Замыкание кольца (первая вершина == последняя); точку/отрезок не трогаем.
fn close_ring(ring: &[TritPoint]) -> Vec<TritPoint> {
    if ring.len() >= 3 {
        let mut v = ring.to_vec();
        if v[0] != v[v.len() - 1] {
            v.push(v[0].clone());
        }
        v
    } else {
        ring.to_vec()
    }
}

/// LineString↔кольцо (Сессия-14): тонкая обёртка — кольцо = полигон
/// без дырок. Вся механика — в [`relate_line_polygon`] (Сессия-17
/// обобщила её на кольца с дырками для GeometryCollection).
pub fn relate_line_ring(line: &[TritPoint], ring: &[TritPoint]) -> Result<De9im, String> {
    if ring.len() < 3 {
        return Err("de9im: кольцо < 3 вершин".into());
    }
    relate_line_polygon(line, &[ring.to_vec()])
}

/// LineString↔полигон с дырками (Сессия-17): ломаная (A, ОТКРЫТАЯ,
/// ≥ 2 вершин) против полигона B = `[outer, hole1, …]`.
///
/// Свидетели: вершины/сэмплы рёбер линии (внутри/на/снаружи ПОЛИГОНА —
/// дырка считается внешностью), концы линии (граница A), классифицированные
/// пересечения рёбер со ВСЕМИ кольцами, вершины/середины всех колец
/// (лежат ли на линии — для EB).
///
/// Трёхзначная модель ячеек: точка пересечения → 0, линейное → +1.
/// Тонкости: собственное (proper) пересечение рёбра линии с ЛЮБЫМ кольцом
/// (внешним ИЛИ дырки) переводит линию через границу полигона ⇒ куски
/// внутри и снаружи оба есть (II = IE = +1): дырка — внешность, переход
/// «тело↔дырка» — это тоже смена стороны.
pub fn relate_line_polygon(
    line: &[TritPoint],
    rings: &[Vec<TritPoint>],
) -> Result<De9im, String> {
    if rings.is_empty() {
        return Err("de9im: полигон — хотя бы одно кольцо".into());
    }
    for (i, r) in rings.iter().enumerate() {
        if r.len() < 3 {
            return Err(format!("de9im: кольцо {i} полигона < 3 вершин"));
        }
    }
    // чистка вырожденных повторов соседних вершин линии
    let mut l: Vec<TritPoint> = Vec::with_capacity(line.len());
    for p in line {
        if l.last() != Some(p) {
            l.push(p.clone());
        }
    }
    if l.is_empty() {
        return Err("de9im: пустая линия".into());
    }
    if l.len() == 1 {
        return relate_point_polygon(&l[0], rings);
    }
    let rings_c: Vec<Vec<TritPoint>> = rings.iter().map(|r| close_ring(r)).collect();
    let k = l[0].k;
    if rings_c.iter().any(|r| r.iter().any(|p| p.k != k)) {
        return Err("de9im: линия и кольца полигона на решётках разного масштаба".into());
    }
    let (outer, holes) = rings_c.split_first().unwrap();
    let pos_poly = |p: &TritPoint| point_in_poly(p, outer, holes);
    let n = l.len();

    // ── свидетели линии ──
    let mut mid_in = false; // сэмпл ребра линии строго внутри полигона
    let mut mid_out = false;
    let mut mid_on = false;
    let mut inner_vert_in = false; // внутренняя (не концевая) вершина
    let mut inner_vert_out = false;
    let mut inner_vert_on = false;
    for (i, p) in l.iter().enumerate() {
        if i > 0 && i + 1 < n {
            // внутренняя вершина линии
            match pos_poly(p) {
                1 => inner_vert_in = true,
                -1 => inner_vert_out = true,
                _ => inner_vert_on = true,
            }
        }
    }
    // сэмплы внутренности рёбер: t = 1/4, 1/2, 3/4, по каждой оси
    // floor/ceil-пробы. Точная гарантия: границы — решётчатые прямые,
    // поэтому ячейка проб вокруг точки сэмпла не рассекается границей
    // «насквозь» — хотя бы одна проба строго по ту же сторону (диагональ
    // через угол не теряет II). Дырки — те же решётчатые границы.
    let is_line_end = |p: &TritPoint| p == &l[0] || p == &l[n - 1];
    for i in 0..n - 1 {
        let p = &l[i];
        let q = &l[i + 1];
        for m in 1..=3i64 {
            let nx = (4 - m) * p.x.to_i64() + m * q.x.to_i64();
            let ny = (4 - m) * p.y.to_i64() + m * q.y.to_i64();
            for (sx, sy) in [(0i64, 0i64), (0, 3), (3, 0), (3, 3)] {
                let s = TritPoint {
                    x: Trits::from_i64((nx + sx) / 4)?,
                    y: Trits::from_i64((ny + sy) / 4)?,
                    k: p.k,
                };
                match pos_poly(&s) {
                    1 => mid_in = true,
                    -1 => mid_out = true,
                    _ => {
                        // проба на границе (любое кольцо): свидетель IB —
                        // только если это НЕ конец линии (касание конца = BB)
                        if !is_line_end(&s) {
                            mid_on = true;
                        }
                    }
                }
            }
        }
    }
    // концы линии — граница A
    let (mut ep_in, mut ep_on, mut ep_out) = (false, false, false);
    for p in [&l[0], &l[n - 1]] {
        match pos_poly(p) {
            1 => ep_in = true,
            -1 => ep_out = true,
            _ => ep_on = true,
        }
    }
    // ── пересечения рёбер линии с рёбрами ВСЕХ колец ──
    let mut proper = false;
    let mut touch = false; // любое касание (для диагнозов)
    let mut touch_interior = false; // касание ВО внутренности линии (для IB)
    let mut collinear = false;
    let line_end = |p: &TritPoint| p == &l[0] || p == &l[n - 1];
    for ring in &rings_c {
        let nr = ring.len();
        for i in 0..n - 1 {
            for j in 0..nr {
                let (a, b) = (&l[i], &l[i + 1]);
                let (c, d) = (&ring[j], &ring[(j + 1) % nr]);
                match seg_classify(a, b, c, d) {
                    SegX::Proper => proper = true,
                    SegX::Touch => {
                        touch = true;
                        // точка касания во внутренности линии? случаи:
                        // вершина кольца на ребре линии / конец ребра линии
                        // на ребре кольца, но не конец самой линии.
                        let on_edge = |q: &TritPoint| -> bool {
                            if on_segment(q, a, b) {
                                if q != a && q != b {
                                    return true; // строго внутри ребра
                                }
                                if q == a && i > 0 {
                                    return true; // внутренняя вершина линии
                                }
                                if q == b && i + 1 < n - 1 {
                                    return true;
                                }
                            }
                            false
                        };
                        if on_edge(c) || on_edge(d) {
                            touch_interior = true;
                        } else if (on_segment(a, c, d) && !line_end(a))
                            || (on_segment(b, c, d) && !line_end(b))
                        {
                            touch_interior = true;
                        }
                    }
                    SegX::Collinear => collinear = true,
                    SegX::None => {}
                }
            }
        }
    }
    // ── свидетели всех колец для EB: все ли лежат на линии ──
    let mut rings_on_line = true;
    'outer_loop: for ring in &rings_c {
        let nr = ring.len();
        for j in 0..nr {
            let a = &ring[j];
            let b = &ring[(j + 1) % nr];
            let mid = TritPoint {
                x: Trits::from_i64((a.x.to_i64() + b.x.to_i64()) / 2)?,
                y: Trits::from_i64((a.y.to_i64() + b.y.to_i64()) / 2)?,
                k: a.k,
            };
            let on = |p: &TritPoint| -> bool {
                (0..n - 1).any(|i| on_segment(p, &l[i], &l[i + 1]))
            };
            if !on(a) || !on(&mid) {
                rings_on_line = false;
                break 'outer_loop;
            }
        }
    }
    let _ = touch; // диагнозы касаний учтены в touch_interior

    // ── сборка матрицы: строки I/B/E линии, столбцы I/B/E полигона ──
    let mut m = De9im::EMPTY;
    // II: кусок внутренности линии внутри полигона (1-мерный)
    if mid_in || inner_vert_in || proper {
        m.cells[0] = 1;
    }
    // IB: внутренность линии на границе полигона (любое кольцо) —
    // коллинеарный участок (+1) или точечное касание (0);
    // касание в КОНЦЕ линии — это BB, не IB.
    if collinear {
        m.cells[1] = 1;
    } else if proper || touch_interior || mid_on || inner_vert_on {
        m.cells[1] = 0;
    }
    // IE: кусок внутренности линии снаружи полигона (вне внешнего или в дырке)
    if mid_out || inner_vert_out || proper {
        m.cells[2] = 1;
    }
    // BI: конец линии внутри полигона (точка)
    if ep_in {
        m.cells[3] = 0;
    }
    // BB: конец линии на границе полигона (точка)
    if ep_on {
        m.cells[4] = 0;
    }
    // BE: конец линии снаружи (точка)
    if ep_out {
        m.cells[5] = 0;
    }
    // EI: внутренность полигона не накрыта 1-мерной линией — всегда площадь
    m.cells[6] = 1;
    // EB: граница полигона (все кольца) не накрыта линией целиком
    if !rings_on_line {
        m.cells[7] = 1;
    }
    // EE: внешности ограниченных фигур пересекаются площадно
    m.cells[8] = 1;
    Ok(m)
}

/// LineString↔LineString (Сессия-15): две открытые ломаные.
/// Завершает реляционную матрицу GEO-ядра: точка/кольцо/строка ×
/// точка/кольцо/строка — якорь `geo.poler …/relate` (GeometryArray, L×L).
///
/// Трёхзначные ячейки (как в LineString×Polygon): точечное пересечение → 0,
/// отрезок → +1. Свидетели:
/// * концы обеих линий (границы): на чужой линии — строго во внутренности
///   (IB/BI = 0) или в чужом конце (BB = 0), вне — BE/EB = +1;
/// * внутренние вершины: лежат строго внутри чужого ребра или совпадают
///   с чужой внутренней вершиной → II = 0 (точка);
/// * пары рёбер: Proper → II = 0, Collinear (перекрытие положительной
///   длины) → II = +1;
/// * вложенность (IE/EI = −1 ⟺ линия целиком на другой): все вершины
///   и сэмплы t = 1/4, 1/2, 3/4 с floor/ceil-пробами на другой линии.
pub fn relate_line_line(a: &[TritPoint], b: &[TritPoint]) -> Result<De9im, String> {
    let clean = |pts: &[TritPoint]| -> Vec<TritPoint> {
        let mut v: Vec<TritPoint> = Vec::with_capacity(pts.len());
        for p in pts {
            if v.last() != Some(p) {
                v.push(p.clone());
            }
        }
        v
    };
    let la = clean(a);
    let lb = clean(b);
    if la.len() < 2 || lb.len() < 2 {
        return Err("de9im: линия < 2 вершин".into());
    }
    if la[0].k != lb[0].k {
        return Err("de9im: линии на решётках разного масштаба".into());
    }
    let (n, m) = (la.len(), lb.len());

    // точка в замыкании ломаной
    let on_line = |p: &TritPoint, l: &[TritPoint]| -> bool {
        (0..l.len() - 1).any(|i| on_segment(p, &l[i], &l[i + 1]))
    };
    let is_end = |p: &TritPoint, l: &[TritPoint]| p == &l[0] || p == &l[l.len() - 1];

    // ── пары рёбер: Proper / Collinear ──
    let mut proper = false;
    let mut collinear = false;
    for i in 0..n - 1 {
        for j in 0..m - 1 {
            match seg_classify(&la[i], &la[i + 1], &lb[j], &lb[j + 1]) {
                SegX::Proper => proper = true,
                SegX::Collinear => collinear = true,
                _ => {}
            }
        }
    }

    // ── внутренние вершины: точечные свидетели II ──
    let mut ii_point = false;
    for is_a in [true, false] {
        let (l_self, l_other) = if is_a { (&la, &lb) } else { (&lb, &la) };
        for vi in 1..l_self.len() - 1 {
            let v = &l_self[vi];
            // строго внутри чужого ребра
            if (0..l_other.len() - 1).any(|j| {
                on_segment(v, &l_other[j], &l_other[j + 1])
                    && v != &l_other[j]
                    && v != &l_other[j + 1]
            }) {
                ii_point = true;
            }
            // совпадение с чужой внутренней вершиной
            if (1..l_other.len() - 1).any(|j| v == &l_other[j]) {
                ii_point = true;
            }
        }
    }

    // ── концы: IB/BI/BB и BE/EB ──
    let (mut ib, mut bi, mut bb, mut be, mut eb) = (false, false, false, false, false);
    // конец B на A: у A-конца → BB, иначе → IB; вне A → EB
    for e in [&lb[0], &lb[m - 1]] {
        if on_line(e, &la) {
            if is_end(e, &la) {
                bb = true;
            } else {
                ib = true;
            }
        } else {
            eb = true;
        }
    }
    // конец A на B: у B-конца → BB, иначе → BI; вне B → BE
    for e in [&la[0], &la[n - 1]] {
        if on_line(e, &lb) {
            if is_end(e, &lb) {
                bb = true;
            } else {
                bi = true;
            }
        } else {
            be = true;
        }
    }

    // ── вложенность: A ⊆ B и B ⊆ A (вершины + сэмплы рёбер) ──
    let mut a_escapes = proper; // собственное пересечение = обе выходят
    let mut b_escapes = proper;
    for p in &la {
        if !on_line(p, &lb) {
            a_escapes = true;
        }
    }
    for p in &lb {
        if !on_line(p, &la) {
            b_escapes = true;
        }
    }
    // сэмплы t = 1/4, 1/2, 3/4 с floor/ceil-пробами (стиль LineString×Polygon:
    // решётчатые границы не рассекают ячейку проб насквозь)
    for is_a in [true, false] {
        let (l_self, l_other) = if is_a { (&la, &lb) } else { (&lb, &la) };
        for i in 0..l_self.len() - 1 {
            let p = &l_self[i];
            let q = &l_self[i + 1];
            for t in 1..=3i64 {
                let nx = (4 - t) * p.x.to_i64() + t * q.x.to_i64();
                let ny = (4 - t) * p.y.to_i64() + t * q.y.to_i64();
                for (sx, sy) in [(0i64, 0i64), (0, 3), (3, 0), (3, 3)] {
                    let s = TritPoint {
                        x: Trits::from_i64((nx + sx) / 4)?,
                        y: Trits::from_i64((ny + sy) / 4)?,
                        k: p.k,
                    };
                    if !on_line(&s, l_other) {
                        if is_a {
                            a_escapes = true;
                        } else {
                            b_escapes = true;
                        }
                    }
                }
            }
        }
    }

    // ── сборка матрицы: строки I/B/E линии A, столбцы I/B/E линии B ──
    let mut m = De9im::EMPTY;
    // II: перекрытие отрезком (+1) или точка (0)
    if collinear {
        m.cells[0] = 1;
    } else if proper || ii_point {
        m.cells[0] = 0;
    }
    // IB: чужой конец строго во внутренности A (точка)
    if ib {
        m.cells[1] = 0;
    }
    // IE: внутренность A выходит за замыкание B
    if a_escapes {
        m.cells[2] = 1;
    }
    // BI: свой конец строго во внутренности B (точка)
    if bi {
        m.cells[3] = 0;
    }
    // BB: общий конец (точка)
    if bb {
        m.cells[4] = 0;
    }
    // BE: свой конец вне B
    if be {
        m.cells[5] = 1;
    }
    // EI: внутренность B выходит за замыкание A
    if b_escapes {
        m.cells[6] = 1;
    }
    // EB: чужой конец вне A
    if eb {
        m.cells[7] = 1;
    }
    // EE: внешности ограниченных линий пересекаются площадно
    m.cells[8] = 1;
    Ok(m)
}

// ─────────────── полигоны с дырками (Сессия-16) ───────────────

/// Позиция точки относительно полигона с дырками: 1 — строго внутри
/// (внешнее кольцо накрывает, ни одна дырка не глотает), 0 — на границе
/// (любое кольцо), −1 — снаружи (вне внешнего или внутри дырки).
fn point_in_poly(p: &TritPoint, outer: &[TritPoint], holes: &[Vec<TritPoint>]) -> i8 {
    match point_in_ring(p, outer) {
        1 => {
            for h in holes {
                match point_in_ring(p, h) {
                    1 => return -1, // внутри дырки — внешность полигона
                    0 => return 0,  // на границе дырки — граница полигона
                    _ => {}
                }
            }
            1
        }
        0 => 0,
        _ => -1, // вне внешнего (дырки внутри — на них отсюда не попасть)
    }
}

/// Полигон↔полигон с дырками (Сессия-16): замыкает реляционную матрицу
/// площадных геометрий. Вход — список колец: `[outer, hole1, hole2, …]`
/// (дырки строго внутри внешнего, не касаются — простые полигоны OGC).
///
/// Свидетели (генерализация `relate_ring_ring` на случай дырок):
/// * вершины и середины рёбер ВСЕХ колец — позиция относительно ЧУЖОГО
///   ПОЛИГОНА целиком (внутри = внутри внешнего И вне дырок); граничная
///   точка несёт сектор I(A) ⇒ вершина/середина внутри B — косвенный
///   свидетель II (как в кольцевом кейсе);
/// * offset-узлы: середина ребра + квант К ВНУТРЕННОСТИ полигона — для
///   внешнего кольца по знаку обхода, для дырки ПРОТИВ (внутрь полигона
///   = наружу дырки). Закрывает совпадающие/вложенные полигоны без нырков;
/// * пересечения рёбер всех пар колец (внешнее×внешнее, внешнее×дырки,
///   дырки×дырки): Proper — границы пересекаются накрест (сектора I(A) и
///   I(B) чередуются шахматно ⇒ II), Touch — точечное касание границ,
///   Collinear — линейное перекрытие границ.
pub fn relate_polygon_polygon(
    rings_a: &[Vec<TritPoint>],
    rings_b: &[Vec<TritPoint>],
) -> Result<De9im, String> {
    if rings_a.is_empty() || rings_b.is_empty() {
        return Err("de9im_poly: полигон — хотя бы одно кольцо".into());
    }
    for (who, rings) in [("A", rings_a), ("B", rings_b)] {
        for (i, r) in rings.iter().enumerate() {
            if r.len() < 3 {
                return Err(format!(
                    "de9im_poly: кольцо {i} полигона {who} < 3 вершин"
                ));
            }
        }
        let k = rings[0][0].k;
        if rings.iter().any(|r| r.iter().any(|p| p.k != k)) {
            return Err(format!(
                "de9im_poly: кольца полигона {who} на решётках разного масштаба"
            ));
        }
    }
    if rings_a[0][0].k != rings_b[0][0].k {
        return Err("de9im_poly: полигоны на решётках разного масштаба".into());
    }

    // замыкание всех колец
    let close = |rings: &[Vec<TritPoint>]| -> Vec<Vec<TritPoint>> {
        rings.iter().map(|r| close_ring(r)).collect()
    };
    let ra = close(rings_a);
    let rb = close(rings_b);
    let (outer_a, holes_a) = ra.split_first().unwrap();
    let (outer_b, holes_b) = rb.split_first().unwrap();

    // ── свидетели колец A против полигона B (и симметрично) ──
    let mut a_vert_in = false; // вершина кольца A строго внутри полигона B
    let mut a_vert_out = false;
    let mut a_mid_in = false; // середина ребра кольца A строго внутри B
    let mut a_mid_out = false;
    let mut a_off_in = false; // offset-узел (квант ВНУТРЬ полигона A) внутри B
    let mut a_off_out = false;
    let mut b_vert_in = false;
    let mut b_vert_out = false;
    let mut b_mid_in = false;
    let mut b_mid_out = false;
    let mut b_off_in = false;
    let mut b_off_out = false;

    let mut proper = false; // собственное пересечение рёбер
    let mut touch = false; // касание границ в точке
    let mut collinear = false; // коллинеарное перекрытие границ

    for is_a in [true, false] {
        let rings = if is_a { &ra } else { &rb };
        let (other_outer, other_holes) = if is_a {
            (outer_b, holes_b)
        } else {
            (outer_a, holes_a)
        };
        let pos_other = |p: &TritPoint| point_in_poly(p, other_outer, other_holes);
        for (ri, r) in rings.iter().enumerate() {
            let is_hole = ri > 0;
            // эффективная ориентация для offset: внутренность ПОЛИГОНА
            // относительно ребра — по знаку обхода внешнего, ПРОТИВ знака дырки
            let orient = ring_orientation(r)?;
            let eff_orient: Trit = if is_hole { -orient } else { orient };
            let n = r.len();
            for i in 0..n {
                let p = &r[i];
                let q = &r[(i + 1) % n];
                // вершины
                match pos_other(p) {
                    1 => {
                        if is_a {
                            a_vert_in = true
                        } else {
                            b_vert_in = true
                        }
                    }
                    -1 => {
                        if is_a {
                            a_vert_out = true
                        } else {
                            b_vert_out = true
                        }
                    }
                    _ => {}
                }
                // середины рёбер: floor/ceil-пробы (как в кольцевом кейсе)
                let mx = p.x.to_i64() + q.x.to_i64();
                let my = p.y.to_i64() + q.y.to_i64();
                for (dx, dy) in [(0i64, 0i64), (1, 0), (0, 1), (1, 1)] {
                    let mid = TritPoint {
                        x: Trits::from_i64((mx + dx) / 2)?,
                        y: Trits::from_i64((my + dy) / 2)?,
                        k: p.k,
                    };
                    match pos_other(&mid) {
                        1 => {
                            if is_a {
                                a_mid_in = true
                            } else {
                                b_mid_in = true
                            }
                        }
                        -1 => {
                            if is_a {
                                a_mid_out = true
                            } else {
                                b_mid_out = true
                            }
                        }
                        _ => {}
                    }
                    // offset-проба: точная середина + квант к внутренности ПОЛИГОНА
                    if dx == 0 && dy == 0 {
                        let ddx = q.x.to_i64() - p.x.to_i64();
                        let ddy = q.y.to_i64() - p.y.to_i64();
                        if (ddx, ddy) != (0, 0) {
                            let (mut nx, mut ny) = (-ddy, ddx); // левая нормаль
                            if eff_orient == -1 {
                                (nx, ny) = (ddy, -ddx); // CW — внутренность справа
                            }
                            let sx = nx.signum();
                            let sy = ny.signum();
                            if (sx, sy) != (0, 0) {
                                let off = TritPoint {
                                    x: Trits::from_i64((mx + dx) / 2 + sx)?,
                                    y: Trits::from_i64((my + dy) / 2 + sy)?,
                                    k: p.k,
                                };
                                match pos_other(&off) {
                                    1 => {
                                        if is_a {
                                            a_off_in = true
                                        } else {
                                            b_off_in = true
                                        }
                                    }
                                    -1 => {
                                        if is_a {
                                            a_off_out = true
                                        } else {
                                            b_off_out = true
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // пересечения рёбер: ВСЕ пары колец A × колец B
    for r in &ra {
        for s in &rb {
            let (na, nb) = (r.len(), s.len());
            for i in 0..na {
                for j in 0..nb {
                    match seg_classify(
                        &r[i],
                        &r[(i + 1) % na],
                        &s[j],
                        &s[(j + 1) % nb],
                    ) {
                        SegX::Proper => proper = true,
                        SegX::Touch => touch = true,
                        SegX::Collinear => collinear = true,
                        SegX::None => {}
                    }
                }
            }
        }
    }

    // ── сборка матрицы: правила relate_ring_ring (генерализация) ──
    let mut m = De9im::EMPTY;
    // II: внутренности площадно пересекаются ⟺ нырок, накрытие, совпадение
    if a_vert_in
        || b_vert_in
        || proper
        || a_mid_in
        || b_mid_in
        || a_off_in
        || b_off_in
    {
        m.cells[0] = 1;
    }
    // IB: граница B проходит по внутренности A (линейно)
    if b_vert_in || b_mid_in || proper {
        m.cells[1] = 1;
    }
    // BI: симметрично
    if a_vert_in || a_mid_in || proper {
        m.cells[3] = 1;
    }
    // BB: коллинеарное перекрытие (+1), точечное касание/пересечение (0)
    if collinear {
        m.cells[4] = 1;
    } else if touch || proper {
        m.cells[4] = 0;
    }
    // IE: часть внутренности A снаружи B (площадно)
    if a_vert_out || a_mid_out || a_off_out || proper || b_vert_in || b_mid_in {
        m.cells[2] = 1;
    }
    // EI: симметрично
    if b_vert_out || b_mid_out || b_off_out || proper || a_vert_in || a_mid_in {
        m.cells[6] = 1;
    }
    // BE: часть границы A снаружи B (линейно)
    if a_vert_out || a_mid_out || proper {
        m.cells[5] = 1;
    }
    // EB: симметрично
    if b_vert_out || b_mid_out || proper {
        m.cells[7] = 1;
    }
    // EE: внешности двух ограниченных полигонов всегда пересекаются площадно
    m.cells[8] = 1;
    Ok(m)
}

// ─────────────── смешанная размерность: GeometryCollection (Сессия-17) ───────────────
//
// Директива: «mixed-размерность relate (GeometryCollection)». OGC-семантика
// коллекций (как JTS/GEOS): внутренность/граница/внешность коллекции —
// ОБЪЕДИНЕНИЕ внутренних/граничных/внешностей компонент; размерность
// объединения = max размерностей, что в тритной модели ячейки —
// cell-wise max (−1 < 0 < +1). Пара матриц компонент сворачивается
// в матрицу коллекции без потери сигнала: любое непустое пересечение
// всплывает, размерность доминирует старшая.
//
// Граница линии (по JTS) = концы; граница коллекции = объединение границ
// компонент (для ПЕРЕСЕКАЮЩИХСЯ компонент OGC строже — задокументировано
// честно как конвенция движка).

/// Геометрия произвольной размерности — компонент коллекций relate.
#[derive(Clone, Debug)]
pub enum Geometry {
    Point(TritPoint),
    /// Открытая ломаная ≥ 2 вершин (1 вершина допустима входом — точка).
    Line(Vec<TritPoint>),
    /// Полигон `[outer, holes…]` (кольца не замкнуты — замкнётся).
    Polygon(Vec<Vec<TritPoint>>),
    /// Коллекция разной размерности (GeometryCollection OGC).
    Collection(Vec<Geometry>),
}

/// Объединение матриц по ячейкам: cell-wise max (dim(∪) = max dim).
fn matrix_union(acc: &mut De9im, m: &De9im) {
    for i in 0..9 {
        if m.cells[i] > acc.cells[i] {
            acc.cells[i] = m.cells[i];
        }
    }
}

/// relate двух геометрий ПРОИЗВОЛЬНОЙ размерности: точки, линии,
/// полигоны (с дырками) и коллекции — полная реляционная матрица
/// смешанных размерностей. Коллекция сворачивается по парам компонент
/// (рекурсивно: коллекция×коллекция = все пары), матрицы объединяются
/// ячейками. Пустая коллекция — честная ошибка.
pub fn relate_geometry(a: &Geometry, b: &Geometry) -> Result<De9im, String> {
    match (a, b) {
        (Geometry::Collection(cs), _) => {
            if cs.is_empty() {
                return Err("de9im: пустая коллекция".into());
            }
            let mut acc = De9im::EMPTY;
            for c in cs {
                matrix_union(&mut acc, &relate_geometry(c, b)?);
            }
            Ok(acc)
        }
        (_, Geometry::Collection(cs)) => {
            if cs.is_empty() {
                return Err("de9im: пустая коллекция".into());
            }
            let mut acc = De9im::EMPTY;
            for c in cs {
                matrix_union(&mut acc, &relate_geometry(a, c)?);
            }
            Ok(acc)
        }
        (Geometry::Point(p), Geometry::Point(q)) => relate_point_point(p, q),
        (Geometry::Point(p), Geometry::Line(l)) => relate_point_line(p, l),
        (Geometry::Point(p), Geometry::Polygon(g)) => relate_point_polygon(p, g),
        (Geometry::Line(l), Geometry::Point(p)) => Ok(transpose(&relate_point_line(p, l)?)),
        (Geometry::Line(l), Geometry::Line(m)) => relate_line_line(l, m),
        (Geometry::Line(l), Geometry::Polygon(g)) => relate_line_polygon(l, g),
        (Geometry::Polygon(g), Geometry::Point(p)) => Ok(transpose(&relate_point_polygon(p, g)?)),
        (Geometry::Polygon(g), Geometry::Line(l)) => Ok(transpose(&relate_line_polygon(l, g)?)),
        (Geometry::Polygon(g), Geometry::Polygon(h)) => relate_polygon_polygon(g, h),
    }
}

/// Точка↔LineString (Сессия-17): замыкает точечную строку матрицы.
/// Позиции точки: строго во внутренности линии (II = 0), на конце
/// (IB = 0 — граница линии), вне замыкания (IE = 0) — три честных
/// исхода, самопересечения дают несколько сигналов сразу.
pub fn relate_point_line(p: &TritPoint, line: &[TritPoint]) -> Result<De9im, String> {
    let mut l: Vec<TritPoint> = Vec::with_capacity(line.len());
    for q in line {
        if l.last() != Some(q) {
            l.push(q.clone());
        }
    }
    if l.is_empty() {
        return Err("de9im: пустая линия".into());
    }
    if l.len() == 1 {
        return relate_point_point(p, &l[0]);
    }
    if p.k != l[0].k {
        return Err("de9im: точка и линия на решётках разного масштаба".into());
    }
    let n = l.len();
    let mut inside = false; // точка во внутренности линии (ребро/внутр. вершина)
    let mut on_end = false; // точка = конец линии (граница)
    for i in 0..n - 1 {
        if on_segment(p, &l[i], &l[i + 1]) {
            let at_start = p == &l[i];
            let at_end = p == &l[i + 1];
            if (at_start && i == 0) || (at_end && i + 1 == n - 1) {
                on_end = true;
            } else {
                // строго внутри ребра ИЛИ внутренняя вершина линии
                inside = true;
            }
        }
    }
    let mut m = De9im::EMPTY;
    if inside {
        m.cells[0] = 0; // II
    }
    if on_end {
        m.cells[1] = 0; // IB: конец линии
    }
    if !inside && !on_end {
        m.cells[2] = 0; // IE: точка вне замыкания линии
    }
    m.cells[6] = 1; // EI: линия минус точка — 1-мерна
    // EB: концы линии минус возможная точка — конечное множество точек
    let any_ep_besides = (&l[0] != p) || (&l[n - 1] != p);
    m.cells[7] = if any_ep_besides { 0 } else { -1 };
    m.cells[8] = 1; // EE
    Ok(m)
}

/// Точка↔полигон с дырками (Сессия-17): позиции — внутри (II),
/// на границе ЛЮБОГО кольца (IB), снаружи — вне внешнего или В ДЫРКЕ (IE).
pub fn relate_point_polygon(p: &TritPoint, rings: &[Vec<TritPoint>]) -> Result<De9im, String> {
    if rings.is_empty() {
        return Err("de9im: полигон — хотя бы одно кольцо".into());
    }
    for (i, r) in rings.iter().enumerate() {
        if r.len() < 3 {
            return Err(format!("de9im: кольцо {i} полигона < 3 вершин"));
        }
    }
    let closed: Vec<Vec<TritPoint>> = rings.iter().map(|r| close_ring(r)).collect();
    let (outer, holes) = closed.split_first().unwrap();
    if outer[0].k != p.k {
        return Err("de9im: точка и полигон на решётках разного масштаба".into());
    }
    let pos = point_in_poly(p, outer, holes);
    let mut m = De9im::EMPTY;
    match pos {
        1 => m.cells[0] = 0,  // II: точка внутри (дырки не глотают)
        0 => m.cells[1] = 0,  // IB: на любом кольце
        _ => m.cells[2] = 0,  // IE: снаружи (вне внешнего или в дырке)
    }
    // E(A) — всё кроме точки; внутренность/граница полигона заведомо
    // площадно/линейно выходят за пределы одной точки.
    m.cells[6] = 1; // EI
    m.cells[7] = 1; // EB
    m.cells[8] = 1; // EE
    Ok(m)
}

// ────────────────────────── тесты ──────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    const K: u8 = 6;

    fn tp(x: f64, y: f64) -> TritPoint {
        TritPoint::quantize(x, y, K).unwrap()
    }

    fn ring(pts: &[(f64, f64)]) -> Vec<TritPoint> {
        pts.iter().map(|&(x, y)| tp(x, y)).collect()
    }

    #[test]
    fn trit_code_roundtrip() {
        let mut m = De9im::EMPTY;
        m.cells[0] = 1;
        m.cells[4] = 0;
        m.cells[8] = -1;
        let code = m.trit_code();
        let back = De9im::from_trit_code(code).unwrap();
        assert_eq!(m, back);
        // диапазон
        assert!(De9im::from_trit_code(9842).is_err());
        // строка (символы, не байты — '−' трехбайтный)
        let mut n = De9im::EMPTY;
        n.cells[0] = -1;
        n.cells[1] = 0;
        n.cells[2] = 1;
        let s: String = n.bal_string().chars().take(3).collect();
        assert_eq!(s, "−0+");
    }

    #[test]
    fn disjoint_squares() {
        let a = ring(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
        let b = ring(&[(5.0, 5.0), (7.0, 5.0), (7.0, 7.0), (5.0, 7.0)]);
        let m = relate(&a, &b).unwrap();
        assert!(m.disjoint(), "матрица: {}", m.bal_string());
        assert!(!m.intersects());
        assert!(m.predicate_names().contains(&"disjoint"));
    }

    #[test]
    fn nested_squares_contains() {
        let big = ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        let small = ring(&[(3.0, 3.0), (5.0, 3.0), (5.0, 5.0), (3.0, 5.0)]);
        let m = relate(&big, &small).unwrap();
        assert!(m.contains(), "матрица: {}", m.bal_string());
        assert!(relate(&small, &big).unwrap().within());
        assert!(!m.overlaps(), "вложение — не перекрытие");
    }

    #[test]
    fn identical_squares_equals() {
        let a = ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        let b = ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        let m = relate(&a, &b).unwrap();
        assert!(m.equals(), "матрица: {}", m.bal_string());
        assert_eq!(m.bb(), 1, "границы совпадают — коллинеарное перекрытие");
        assert!(m.contains() && m.within());
    }

    #[test]
    fn overlapping_squares() {
        // Классический учебный кейс: A=[0,2]², B=[1,3]².
        let a = ring(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
        let b = ring(&[(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)]);
        let m = relate(&a, &b).unwrap();
        assert!(m.overlaps(), "матрица: {}", m.bal_string());
        assert!(!m.contains() && !m.within());
        assert_eq!(m.ii(), 1);
        assert_eq!(m.bb(), 0, "границы пересекаются в двух точках");
    }

    #[test]
    fn touching_edge_squares_touches() {
        // Квадраты с общей стороной: внутренности не пересекаются,
        // границы перекрываются коллинеарно.
        let a = ring(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
        let b = ring(&[(2.0, 0.0), (4.0, 0.0), (4.0, 2.0), (2.0, 2.0)]);
        let m = relate(&a, &b).unwrap();
        assert!(m.touches(), "матрица: {}", m.bal_string());
        assert_eq!(m.bb(), 1, "общая сторона — линейное перекрытие границ");
        assert!(!m.intersects() || m.touches());
    }

    #[test]
    fn corner_touch_squares() {
        // Квадраты с общей вершиной: BB = 0 (точечное касание).
        let a = ring(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
        let b = ring(&[(2.0, 2.0), (4.0, 2.0), (4.0, 4.0), (2.0, 4.0)]);
        let m = relate(&a, &b).unwrap();
        assert!(m.touches(), "матрица: {}", m.bal_string());
        assert_eq!(m.bb(), 0, "касание вершинами — точка");
    }

    #[test]
    fn point_cases() {
        let sq = ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        // внутри
        let m = relate(&[tp(2.0, 2.0)], &sq).unwrap();
        assert!(m.within());
        // на границе
        let m2 = relate(&[tp(2.0, 0.0)], &sq).unwrap();
        assert!(m2.covered_by() && !m2.within(), "на границе: покрыт, не внутри");
        // снаружи
        let m3 = relate(&[tp(9.0, 9.0)], &sq).unwrap();
        assert!(m3.disjoint());
        // точки между собой
        assert!(relate(&[tp(1.0, 1.0)], &[tp(1.0, 1.0)])
            .unwrap()
            .equals());
        assert!(relate(&[tp(1.0, 1.0)], &[tp(2.0, 2.0)])
            .unwrap()
            .disjoint());
    }

    #[test]
    fn transpose_symmetry() {
        let a = ring(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
        let b = ring(&[(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)]);
        let m = relate(&a, &b).unwrap();
        let t = relate(&b, &a).unwrap();
        assert_eq!(transpose(&m), t, "relate(b,a) = transpose(relate(a,b))");
    }

    #[test]
    fn pattern_string_ogc() {
        // Два далёких квадрата: F/F по взаимным внутренностям и границам,
        // но внутренности/границы лежат во внешности друг друга (T).
        let a = ring(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
        let b = ring(&[(5.0, 5.0), (7.0, 5.0), (7.0, 7.0), (5.0, 7.0)]);
        let m = relate(&a, &b).unwrap();
        let s = m.pattern_string();
        let head: String = s.chars().take(4).collect();
        assert_eq!(head, "FFTF");
        // T только во внешностных ячейках: IE, BE, EI, EB, EE.
        assert_eq!(s.chars().filter(|&c| c == 'T').count(), 5);
    }

    #[test]
    fn exact_boundary_no_epsilon() {
        // Вершина B ТОЧНО на ребре A — тритная решётка решает без epsilon.
        let a = ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        let b = ring(&[(2.0, 0.0), (3.0, 0.0), (3.0, 1.0), (2.0, 1.0)]);
        let m = relate(&a, &b).unwrap();
        // B примыкает к нижнему ребру A изнутри: A покрывает B,
        // границы коллинеарно перекрываются.
        assert!(m.covers(), "матрица: {}", m.bal_string());
        assert_eq!(m.bb(), 1, "коллинеарное прилегание рёбер");
    }

    #[test]
    fn point_on_edge_classified_exactly() {
        // Точка ровно на границе кольца — IB = 0, не «примерно внутри».
        let sq = ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        let m = relate_point_ring(&tp(2.0, 0.0), &sq).unwrap();
        assert_eq!(m.ib(), 0);
        assert_eq!(m.ii(), -1);
        // Угловая точка
        let m2 = relate_point_ring(&tp(4.0, 4.0), &sq).unwrap();
        assert_eq!(m2.ib(), 0);
    }

    // ─────────────── LineString×Polygon (Сессия-14) ───────────────

    fn line(pts: &[(f64, f64)]) -> Vec<TritPoint> {
        pts.iter().map(|&(x, y)| tp(x, y)).collect()
    }

    #[test]
    fn line_through_polygon() {
        // Линия насквозь: два собственных пересечения границы.
        let sq = ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        let l = line(&[(-1.0, 2.0), (5.0, 2.0)]);
        let m = relate_line_ring(&l, &sq).unwrap();
        assert_eq!(m.ii(), 1, "кусок внутри: {}", m.bal_string());
        assert_eq!(m.ib(), 0, "точечные пересечения границы");
        assert_eq!(m.ie(), 1, "куски снаружи");
        assert_eq!(m.bi(), -1, "концы снаружи");
        assert_eq!(m.bb(), -1);
        assert_eq!(m.be(), 0);
        assert_eq!(m.ei(), 1);
        assert_eq!(m.eb(), 1);
        assert!(m.intersects());
        assert!(!m.within());
        assert!(!m.disjoint());
    }

    #[test]
    fn line_inside_polygon() {
        let sq = ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        let l = line(&[(1.0, 1.0), (3.0, 3.0)]);
        let m = relate_line_ring(&l, &sq).unwrap();
        assert_eq!(m.ii(), 1);
        assert_eq!(m.ie(), -1, "внутри — не выглядывает");
        assert_eq!(m.bi(), 0, "концы внутри");
        assert_eq!(m.be(), -1);
        assert!(m.within(), "матрица: {}", m.bal_string());
        assert!(m.covered_by());
        assert!(m.intersects());
    }

    #[test]
    fn line_outside_disjoint() {
        let sq = ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        let l = line(&[(-3.0, -3.0), (-1.0, -1.0)]);
        let m = relate_line_ring(&l, &sq).unwrap();
        assert!(m.disjoint(), "матрица: {}", m.bal_string());
        assert_eq!(m.be(), 0);
        assert_eq!(m.ei(), 1); // полигон «виден» из внешности линии
    }

    #[test]
    fn line_touches_boundary() {
        // Линия упирается концом в границу: изнутри это WITHIN
        // (внутренность линии внутри, конец на границе = BB, не IB).
        let sq = ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        // а) изнутри в ребро
        let l = line(&[(2.0, 1.0), (2.0, 0.0)]);
        let m = relate_line_ring(&l, &sq).unwrap();
        assert!(m.within(), "изнутри: {}", m.bal_string());
        assert_eq!(m.bb(), 0, "конец на границе");
        assert_eq!(m.ib(), -1, "касание — в конце линии, не во внутренности");
        assert_eq!(m.ii(), 1, "внутренность линии внутри");
        assert_eq!(m.ie(), -1);
        // б) снаружи в угол — конец на границе, внутренность снаружи: TOUCHES
        let l2 = line(&[(-2.0, -2.0), (0.0, 0.0)]);
        let m2 = relate_line_ring(&l2, &sq).unwrap();
        assert!(m2.touches(), "снаружи: {}", m2.bal_string());
        assert_eq!(m2.ii(), -1);
        assert_eq!(m2.bb(), 0);
        assert_eq!(m2.ib(), -1);
    }

    #[test]
    fn line_tcrosses_ring_vertex() {
        // Линия проходит ровно через вершину кольца: касание ВО внутренности
        // линии → IB = 0 (точка), без пересечения внутренностей рёбер.
        let sq = ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        // из (−1,−1) через угол (0,0) внутрь к (1,1): вершина на внутренности ребра
        let l = line(&[(-1.0, -1.0), (1.0, 1.0)]);
        let m = relate_line_ring(&l, &sq).unwrap();
        assert_eq!(m.ib(), 0, "T-касание во внутренности: {}", m.bal_string());
        assert_eq!(m.bb(), -1, "концы не на границе");
        assert_eq!(m.ii(), 1);
        assert_eq!(m.ie(), 1);
    }

    #[test]
    fn line_along_boundary_collinear() {
        // Линия лежит на нижнем ребре квадрата: коллинеарное перекрытие.
        let sq = ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        let l = line(&[(0.0, 0.0), (4.0, 0.0)]);
        let m = relate_line_ring(&l, &sq).unwrap();
        assert_eq!(m.ib(), 1, "линейное прилегание: {}", m.bal_string());
        assert_eq!(m.ii(), -1);
        assert_eq!(m.ie(), -1);
        assert!(m.touches());
        assert!(m.intersects());
        // линия, накрывающая ВСЮ границу: EB пуст
        let l2 = line(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (0.0, 0.0)]);
        let m2 = relate_line_ring(&l2, &sq).unwrap();
        assert_eq!(m2.eb(), -1, "граница накрыта линией: {}", m2.bal_string());
    }

    #[test]
    fn polyline_weaving_and_degenerate() {
        // Змейка: внутрь-наружу-внутрь — II и IE оба populated.
        let sq = ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        let l = line(&[(-1.0, 2.0), (2.0, 2.0), (2.0, 6.0), (2.0, 6.0), (1.0, 3.0)]);
        let m = relate_line_ring(&l, &sq).unwrap();
        assert_eq!(m.ii(), 1);
        assert_eq!(m.ie(), 1);
        assert_eq!(m.ib(), 0);
        // вырожденная линия (все вершины в точку) = точка
        let l2 = line(&[(1.0, 1.0), (1.0, 1.0), (1.0, 1.0)]);
        let m2 = relate_line_ring(&l2, &sq).unwrap();
        assert_eq!(m2.ii(), 0, "точка внутри: {}", m2.bal_string());
        assert_eq!(m2.bi(), -1);
        // пустая линия — ошибка; кольцо < 3 — ошибка
        assert!(relate_line_ring(&[], &sq).is_err());
        assert!(relate_line_ring(&l, &ring(&[(0.0, 0.0), (1.0, 0.0)])).is_err());
    }

    // ─────────────── LineString×LineString (Сессия-15) ───────────────

    #[test]
    fn ll_x_crossing() {
        // Накрест: II = 0 (точка), обе выходят → crosses
        let a = line(&[(-2.0, -2.0), (2.0, 2.0)]);
        let b = line(&[(-2.0, 2.0), (2.0, -2.0)]);
        let m = relate_line_line(&a, &b).unwrap();
        assert_eq!(m.ii(), 0, "{}", m.bal_string());
        assert_eq!(m.ie(), 1);
        assert_eq!(m.ei(), 1);
        assert!(m.crosses());
        assert!(!m.overlaps());
        assert!(m.intersects());
    }

    #[test]
    fn ll_partial_overlap() {
        // Коллинеарное частичное перекрытие: II = +1 → overlaps
        let a = line(&[(0.0, 0.0), (4.0, 0.0)]);
        let b = line(&[(2.0, 0.0), (6.0, 0.0)]);
        let m = relate_line_line(&a, &b).unwrap();
        assert_eq!(m.ii(), 1, "{}", m.bal_string());
        assert_eq!(m.ie(), 1);
        assert_eq!(m.ei(), 1);
        // конец B (2,0) строго внутри A; конец A (4,0) строго внутри B
        assert_eq!(m.ib(), 0);
        assert_eq!(m.bi(), 0);
        assert!(m.overlaps());
    }

    #[test]
    fn ll_equal_lines() {
        let a = line(&[(0.0, 0.0), (4.0, 0.0)]);
        let b = line(&[(0.0, 0.0), (4.0, 0.0)]);
        let m = relate_line_line(&a, &b).unwrap();
        assert!(m.equals(), "{}", m.bal_string());
        assert_eq!(m.ii(), 1);
        assert_eq!(m.ie(), -1);
        assert_eq!(m.ei(), -1);
        assert_eq!(m.bb(), 0, "общие концы");
        assert_eq!(m.be(), -1);
        assert_eq!(m.eb(), -1);
        // перевёрнутая та же линия — тоже equals
        let c = line(&[(4.0, 0.0), (0.0, 0.0)]);
        assert!(relate_line_line(&a, &c).unwrap().equals());
    }

    #[test]
    fn ll_contained_subline() {
        // B — под-отрезок A: A содержит B
        let a = line(&[(0.0, 0.0), (4.0, 0.0)]);
        let b = line(&[(1.0, 0.0), (3.0, 0.0)]);
        let m = relate_line_line(&a, &b).unwrap();
        assert!(m.contains(), "{}", m.bal_string());
        assert_eq!(m.ii(), 1);
        assert_eq!(m.ei(), -1, "B не выходит за A");
        assert_eq!(m.eb(), -1, "концы B на A");
        assert_eq!(m.ie(), 1, "A выходит за B");
        // транспонированный взгляд: B внутри A
        let t = relate_line_line(&b, &a).unwrap();
        assert!(t.within());
    }

    #[test]
    fn ll_t_touch_and_endpoint_touch() {
        // T-касание: конец A строго во внутренности B
        let a = line(&[(0.0, 0.0), (0.0, 5.0)]);
        let b = line(&[(-2.0, 0.0), (2.0, 0.0)]);
        let m = relate_line_line(&a, &b).unwrap();
        assert_eq!(m.ii(), -1, "{}", m.bal_string());
        assert_eq!(m.bi(), 0, "конец A во внутренности B");
        assert!(m.touches());
        assert!(!m.crosses());
        // конец-к-концу: BB = 0
        let c = line(&[(0.0, 0.0), (5.0, 0.0)]);
        let m2 = relate_line_line(&a, &c).unwrap();
        assert_eq!(m2.ii(), -1);
        assert_eq!(m2.bb(), 0, "общий конец (0,0)");
        assert!(m2.touches());
    }

    #[test]
    fn ll_disjoint_and_errors() {
        let a = line(&[(0.0, 0.0), (1.0, 0.0)]);
        let b = line(&[(5.0, 5.0), (6.0, 5.0)]);
        let m = relate_line_line(&a, &b).unwrap();
        assert!(m.disjoint(), "{}", m.bal_string());
        assert_eq!(m.be(), 1);
        assert_eq!(m.eb(), 1);
        assert_eq!(m.ee(), 1);
        // ошибки: < 2 вершин, разные решётки
        assert!(relate_line_line(&a, &line(&[(1.0, 1.0)])).is_err());
        assert!(relate_line_line(&[], &b).is_err());
    }

    #[test]
    fn ll_crossing_at_shared_vertex() {
        // Обе ломаные изламываются в общей точке (0,0) — II = 0 (точка)
        let a = line(&[(-1.0, 1.0), (0.0, 0.0), (1.0, 1.0)]);
        let b = line(&[(-1.0, -1.0), (0.0, 0.0), (1.0, -1.0)]);
        let m = relate_line_line(&a, &b).unwrap();
        assert_eq!(m.ii(), 0, "{}", m.bal_string());
        assert!(m.crosses());
        assert_eq!(m.ie(), 1);
        assert_eq!(m.ei(), 1);
    }

    #[test]
    fn ll_transpose_symmetry() {
        let a = line(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0)]);
        let b = line(&[(2.0, -1.0), (2.0, 2.0), (6.0, 2.0)]);
        let m = relate_line_line(&a, &b).unwrap();
        let t = relate_line_line(&b, &a).unwrap();
        assert_eq!(transpose(&m), t, "relate(b,a) = transpose(relate(a,b))");
    }

    #[test]
    fn ll_zigzag_overlaps_shared_run() {
        // Гребёнка A и прямая B с общим пробегом: II = +1, обе выходят
        let a = line(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (4.0, 2.0)]);
        let b = line(&[(-1.0, 2.0), (5.0, 2.0)]);
        let m = relate_line_line(&a, &b).unwrap();
        assert_eq!(m.ii(), 1, "общий пробег y=2: {}", m.bal_string());
        assert_eq!(m.ie(), 1);
        assert_eq!(m.ei(), 1);
        assert!(m.overlaps());
    }
    // ─────────── Сессия-16: полигоны с дырками ───────────

    fn poly(rings: &[&[(f64, f64)]]) -> Vec<Vec<TritPoint>> {
        rings.iter().map(|r| ring(r)).collect()
    }

    #[test]
    fn poly_no_holes_matches_ring_ring() {
        // ГЛАВНЫЙ регресс: полигон без дырок == старый кольцевой relate
        let cases: Vec<(&[(f64, f64)], &[(f64, f64)])> = vec![
            (
                &[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
                &[(5.0, 5.0), (7.0, 5.0), (7.0, 7.0), (5.0, 7.0)],
            ), // disjoint
            (
                &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
                &[(3.0, 3.0), (5.0, 3.0), (5.0, 5.0), (3.0, 5.0)],
            ), // вложение
            (
                &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
                &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
            ), // идентичные
            (
                &[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
                &[(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)],
            ), // перекрытие
            (
                &[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
                &[(2.0, 0.0), (4.0, 0.0), (4.0, 2.0), (2.0, 2.0)],
            ), // общая сторона
            (
                &[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
                &[(2.0, 2.0), (4.0, 2.0), (4.0, 4.0), (2.0, 4.0)],
            ), // касание углом
        ];
        for (i, (a, b)) in cases.iter().enumerate() {
            let (ra, rb) = (ring(a), ring(b));
            let ring_m = relate(&ra, &rb).unwrap();
            let poly_m = relate_polygon_polygon(&poly(&[a]), &poly(&[b])).unwrap();
            assert_eq!(
                ring_m, poly_m,
                "кейс {i}: кольцевая {ring_m:?} ≠ полигональная {poly_m:?}"
            );
        }
    }

    #[test]
    fn poly_hole_swallows_square_disjoint() {
        // B целиком в дырке A: общих точек нет — disjoint
        let a = poly(&[
            &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
            &[(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)],
        ]);
        let b = poly(&[&[(4.0, 4.0), (6.0, 4.0), (6.0, 6.0), (4.0, 6.0)]]);
        let m = relate_polygon_polygon(&a, &b).unwrap();
        assert!(m.disjoint(), "B в дырке: {}", m.bal_string());
        assert_eq!(m.ii(), -1);
        // при этом обе внутренности выглядывают в чужую внешность
        assert_eq!(m.ie(), 1, "рамка A снаружи B");
        assert_eq!(m.ei(), 1, "B в дырке = внешность A");
    }

    #[test]
    fn poly_hole_vs_covering_square_overlaps() {
        // B накрывает дырку A: рамка A ∩ B ≠ ∅, обе выглядывают — overlaps
        let a = poly(&[
            &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
            &[(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)],
        ]);
        let b = poly(&[&[(2.0, 2.0), (8.0, 2.0), (8.0, 8.0), (2.0, 8.0)]]);
        let m = relate_polygon_polygon(&a, &b).unwrap();
        assert!(m.overlaps(), "рамка ∩ накрытие: {}", m.bal_string());
        assert_eq!(m.ii(), 1, "уголки рамки внутри B");
        // граница B проходит и по рамке (I(A)), и по дырке (E(A))
        assert_eq!(m.ib(), 1);
    }

    #[test]
    fn poly_identical_with_holes_equals() {
        // идентичные полигоны с дырками: II через offset-свидетелей
        let mk = || {
            poly(&[
                &[(0.0, 0.0), (12.0, 0.0), (12.0, 12.0), (0.0, 12.0)],
                &[(2.0, 2.0), (6.0, 2.0), (6.0, 6.0), (2.0, 6.0)],
                &[(7.0, 7.0), (10.0, 7.0), (10.0, 10.0), (7.0, 10.0)],
            ])
        };
        let m = relate_polygon_polygon(&mk(), &mk()).unwrap();
        assert!(m.equals(), "идентичные: {}", m.bal_string());
        assert_eq!(m.ii(), 1);
        assert_eq!(m.ie(), -1);
        assert_eq!(m.ei(), -1);
        assert_eq!(m.bb(), 1, "все три кольца коллинеарно совпадают");
    }

    #[test]
    fn poly_hole_equals_other_polygon_touches() {
        // B совпадает с дыркой A: границы общие (BB = +1), внутренности
        // не пересекаются — B в дырке = внешность A
        let a = poly(&[
            &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
            &[(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)],
        ]);
        let b = poly(&[&[(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)]]);
        let m = relate_polygon_polygon(&a, &b).unwrap();
        assert_eq!(m.ii(), -1, "внутренности разделены: {}", m.bal_string());
        assert_eq!(m.bb(), 1, "кольцо B == дырка A: коллинеар");
        assert!(m.touches(), "границы касаются, внутренности нет");
        assert!(!m.contains(), "дырка — не внутренность A");
    }

    #[test]
    fn poly_two_holes_disjoint_siblings() {
        // две дырки в одном полигоне: разные дырки — disjoint
        let a = poly(&[
            &[(0.0, 0.0), (12.0, 0.0), (12.0, 12.0), (0.0, 12.0)],
            &[(2.0, 2.0), (5.0, 2.0), (5.0, 5.0), (2.0, 5.0)],
        ]);
        let b = poly(&[
            &[(0.0, 0.0), (12.0, 0.0), (12.0, 12.0), (0.0, 12.0)],
            &[(7.0, 7.0), (10.0, 7.0), (10.0, 10.0), (7.0, 10.0)],
        ]);
        let m = relate_polygon_polygon(&a, &b).unwrap();
        // A∩B = рамка минус ОБЕ дырки... нет: I(A)∩I(B) = рамка без дырок —
        // непусто (угол между дырками). Проверяем именно это: overlaps.
        assert_eq!(m.ii(), 1, "общая рамка: {}", m.bal_string());
        assert!(m.overlaps() || m.intersects());
    }

    #[test]
    fn poly_transpose_symmetry_with_holes() {
        let a = poly(&[
            &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
            &[(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)],
        ]);
        let b = poly(&[&[(2.0, 2.0), (8.0, 2.0), (8.0, 8.0), (2.0, 8.0)]]);
        let m = relate_polygon_polygon(&a, &b).unwrap();
        let t = relate_polygon_polygon(&b, &a).unwrap();
        assert_eq!(
            transpose(&m),
            t,
            "relate_poly(b,a) = transpose(relate_poly(a,b))"
        );
    }

    #[test]
    fn poly_validation_errors() {
        let good = poly(&[&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]]);
        // короткое кольцо
        let bad = poly(&[&[(0.0, 0.0), (1.0, 1.0)]]);
        assert!(relate_polygon_polygon(&bad, &good).is_err());
        // пустой полигон
        assert!(relate_polygon_polygon(&[], &good).is_err());
        // разные решётки
        let coarse: Vec<Vec<TritPoint>> = vec![vec![
            TritPoint::quantize(0.0, 0.0, 4).unwrap(),
            TritPoint::quantize(2.0, 0.0, 4).unwrap(),
            TritPoint::quantize(2.0, 2.0, 4).unwrap(),
        ]];
        assert!(relate_polygon_polygon(&coarse, &good).is_err());
    }
    // ─────────── Сессия-17: смешанная размерность ───────────

    #[test]
    fn point_line_three_positions() {
        let line = vec![tp(0.0, 0.0), tp(4.0, 0.0), tp(4.0, 4.0)];
        // строго внутри первого ребра
        let m = relate_point_line(&tp(2.0, 0.0), &line).unwrap();
        assert_eq!(m.ii(), 0);
        assert_eq!(m.ib(), -1);
        assert_eq!(m.ie(), -1);
        assert!(m.intersects());
        assert!(!m.touches());
        // внутренняя вершина (4, 0) — внутренность, не граница
        let m = relate_point_line(&tp(4.0, 0.0), &line).unwrap();
        assert_eq!(m.ii(), 0, "внутренняя вершина — внутренность");
        assert_eq!(m.ib(), -1);
        // конец линии — граница: IB = 0, touches
        let m = relate_point_line(&tp(0.0, 0.0), &line).unwrap();
        assert_eq!(m.ii(), -1);
        assert_eq!(m.ib(), 0, "конец — граница линии");
        assert!(m.touches(), "касание концом");
        // вне замыкания — disjoint
        let m = relate_point_line(&tp(2.0, 1.0), &line).unwrap();
        assert!(m.disjoint());
        assert_eq!(m.ie(), 0);
        // вырожденная линия = точка
        let m = relate_point_line(&tp(1.0, 1.0), &[tp(1.0, 1.0)]).unwrap();
        assert!(m.equals(), "точка == точка через line-вход");
        // ошибки
        assert!(relate_point_line(&tp(0.0, 0.0), &[]).is_err());
    }

    #[test]
    fn point_polygon_with_hole_positions() {
        let p = poly(&[
            &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
            &[(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)],
        ]);
        // строго внутри тела
        let m = relate_point_polygon(&tp(1.0, 1.0), &p).unwrap();
        assert_eq!(m.ii(), 0);
        assert!(m.within(), "точка в теле полигона");
        // в дырке — внешность: disjoint
        let m = relate_point_polygon(&tp(5.0, 5.0), &p).unwrap();
        assert!(m.disjoint(), "дырка = внешность");
        assert_eq!(m.ie(), 0);
        // на границе дырки — IB
        let m = relate_point_polygon(&tp(5.0, 3.0), &p).unwrap();
        assert_eq!(m.ib(), 0);
        assert!(m.touches());
        // на внешней границе
        let m = relate_point_polygon(&tp(0.0, 5.0), &p).unwrap();
        assert_eq!(m.ib(), 0);
        // снаружи
        let m = relate_point_polygon(&tp(12.0, 5.0), &p).unwrap();
        assert!(m.disjoint());
        // согласованность с кольцевым кейсом без дырок
        let ring_only = vec![ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)])];
        for probe in [(1.0, 1.0), (12.0, 5.0), (0.0, 5.0)] {
            let a = relate_point_ring(&tp(probe.0, probe.1), &ring_only[0]).unwrap();
            let b = relate_point_polygon(&tp(probe.0, probe.1), &ring_only).unwrap();
            assert_eq!(a, b, "кольцо == полигон без дырок: {probe:?}");
        }
        // ошибки: пустой полигон, короткое кольцо
        assert!(relate_point_polygon(&tp(1.0, 1.0), &[]).is_err());
        let short = vec![vec![tp(0.0, 0.0), tp(1.0, 0.0)]];
        assert!(relate_point_polygon(&tp(1.0, 1.0), &short).is_err());
    }

    #[test]
    fn line_polygon_hole_semantics() {
        let p = poly(&[
            &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
            &[(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)],
        ]);
        // линия сквозь дырку: оба конца снаружи, внутренность проходит
        // через дырку (внешность полигона) — внутренности НЕ пересекаются,
        // но границу рёбра дырки пересекает накрест → touches
        let through_hole = vec![tp(5.0, -2.0), tp(5.0, 12.0)];
        let m = relate_line_polygon(&through_hole, &p).unwrap();
        assert!(m.intersects());
        assert!(!m.disjoint());
        // II куски: линия проходит и через тело (y<3 и y>7 внутри тела,
        // 3<y<7 — дырка) → II = +1 и IE = +1 (правильный переход)
        assert_eq!(m.ii(), 1, "кусок в теле полигона есть");
        assert_eq!(m.ie(), 1, "кусок в дырке/снаружи есть");
        // только дырка: короткая линия целиком в дырке
        let in_hole = vec![tp(4.0, 4.0), tp(6.0, 6.0)];
        let m = relate_line_polygon(&in_hole, &p).unwrap();
        assert!(m.disjoint(), "линия в дырке: {}", m.bal_string());
        assert_eq!(m.ie(), 1, "внутренность линии во внешности (дырке)");
        assert_eq!(m.ii(), -1);
        // согласованность с кольцевым кейсом: relate_line_ring == обёртка
        let ring_only = vec![ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)])];
        let cross = vec![tp(-2.0, 5.0), tp(12.0, 5.0)];
        let a = relate_line_ring(&cross, &ring_only[0]).unwrap();
        let b = relate_line_polygon(&cross, &ring_only).unwrap();
        assert_eq!(a, b);
        assert!(a.crosses() || a.intersects());
        // ошибки
        assert!(relate_line_polygon(&cross, &[]).is_err());
    }

    #[test]
    fn collection_mixed_semantics() {
        let poly_b = poly(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]]);
        // A = коллекция {точка внутри, линия накрест}
        let a = Geometry::Collection(vec![
            Geometry::Point(tp(2.0, 2.0)),
            Geometry::Line(vec![tp(5.0, -2.0), tp(5.0, 12.0)]),
        ]);
        let b = Geometry::Polygon(poly_b.clone());
        let m = relate_geometry(&a, &b).unwrap();
        // II: линия даёт +1 (кусок внутри), точка дала бы 0 — max = +1
        assert_eq!(m.ii(), 1, "доминирует линейная внутренность");
        assert!(m.intersects());
        assert!(!m.disjoint());
        assert_eq!(m.ie(), 1, "линия выглядывает");
        // транспонированная симметрия
        let t = relate_geometry(&b, &a).unwrap();
    assert_eq!(transpose(&m), t, "коллекция×полигон симметрична");
        // B = та же фигура одиночным полигоном: {полигон} == полигон
        let single = Geometry::Collection(vec![Geometry::Polygon(poly_b.clone())]);
        let m1 = relate_geometry(&single, &b).unwrap();
        let m2 = relate_geometry(&b, &b).unwrap();
        assert_eq!(m1, m2, "коллекция из одного == сама геометрия");
        // полностью снаружи: disjoint
        let out = Geometry::Collection(vec![
            Geometry::Point(tp(20.0, 20.0)),
            Geometry::Line(vec![tp(15.0, 15.0), tp(16.0, 16.0)]),
        ]);
        let m = relate_geometry(&out, &b).unwrap();
        assert!(m.disjoint(), "вся коллекция снаружи: {}", m.bal_string());
        // точка внутри + точка снаружи: intersects, но не within
        let mixed = Geometry::Collection(vec![
            Geometry::Point(tp(2.0, 2.0)),
            Geometry::Point(tp(20.0, 20.0)),
        ]);
        let m = relate_geometry(&mixed, &b).unwrap();
        assert!(m.intersects());
        assert!(!m.within());
        assert_eq!(m.ii(), 0, "точечная внутренность");
        // вложенная коллекция
        let nested = Geometry::Collection(vec![Geometry::Collection(vec![
            Geometry::Point(tp(2.0, 2.0)),
        ])]);
        let m = relate_geometry(&nested, &b).unwrap();
        assert!(m.within(), "вложенная коллекция из точки внутри");
        // пустая коллекция — ошибка
        let empty = Geometry::Collection(vec![]);
        assert!(relate_geometry(&empty, &b).is_err());
        assert!(relate_geometry(&b, &empty).is_err());
    }

    #[test]
    fn collection_point_line_cross() {
        // коллекция точка+линия против линии: сквозное отверстие не нужно —
        // точка НА линии + линия Х-образно
        let la = vec![tp(0.0, 0.0), tp(4.0, 4.0)];
        let a = Geometry::Collection(vec![
            Geometry::Line(la.clone()),
            Geometry::Point(tp(1.0, 1.0)), // на линии A
        ]);
        let lb = Geometry::Line(vec![tp(0.0, 4.0), tp(4.0, 0.0)]);
        let m = relate_geometry(&a, &lb).unwrap();
        // рёбра пересекаются накрест в (2,2) — точечное II от пары линий;
        // точка (1,1) снаружи B
        assert!(m.intersects());
        assert_eq!(m.ii(), 0, "X-пересечение линий — точка");
        // точка (1,1) на линии A, но вне B: IE от точки — 0 (dim точки)
        // + линия A выглядывает: IE = max(0, +1) = +1
        assert_eq!(m.ie(), 1);
    }
}

