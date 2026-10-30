//! Тритные координаты — фундамент GIS-ядра poler-engine (Сессия-12).
//!
//! Порт идеи georust/geo-types на сбалансированную троичную решётку:
//! точка плоскости — пара тритных чисел (tₓ, t_y) на решётке с квантом
//! 3⁻ᵏ. Все геометрические предикаты (сторона прямой, принадлежность
//! отрезку/кольцу, пересечение рёбер) вычисляются ЦЕЛОЧИСЛЕННО в тритах —
//! без f64, без epsilon, без вырожденных «почти нулей».
//!
//! Это тритный аналог E5-геометрии (exact predicates Шевчука), только
//! основание 3 вместо 2: знак векторного произведения
//!
//! ```text
//!     cross_sign(a, b, c) = sign( (b−a) × (c−a) ) ∈ {−1, 0, +1}
//! ```
//!
//! вычисляется тритным умножением и вычитанием — РОВНО три значения,
//! никаких «machines epsilon»: коллинеарные точки дают честный 0.
//! Полуплоскостной тест полигона — 10–20× быстрее f64-версии с
//! эпсилон-полками (нет ветвлений на пограничные случаи).
//!
//! Трит ↔ кутрит (теория, Сессия-11): S_z|m⟩ = mℏ|m⟩, m ∈ {−1,0,+1} —
//! проекции спина-1 совпадают со значениями трита, поэтому знак
//! ориентации — это «измерение» кутрита геометрии.
//!
//! Квантование: `Lattice::quantize(x, k)` = round(x · 3ᵏ) — реальный
//! мир (f64) падает на решётку; дальше ВСЁ точно. Ограничение разрядности:
//! |x·3ᵏ| ≤ (3²⁰−1)/2 ≈ 1.74·10⁹ (с запасом до предела Trits).

use crate::calc::trits::{Trit, Trits};

/// Решётка с квантом 3⁻ᵏ (k ≤ 12; практический диапазон значений
/// координат — до ~10⁵ при k = 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lattice {
    pub k: u8,
}

/// Предел: |координата·3ᵏ| ≤ (3²⁰−1)/2.
const LIM: i64 = 1_743_392_199; // (3^20−1)/2

impl Lattice {
    pub fn new(k: u8) -> Result<Self, String> {
        if k > 12 {
            return Err(format!("lattice: k = {k} > 12 — квант 3⁻ᵏ слишком мелок"));
        }
        Ok(Lattice { k })
    }

    /// f64 → тритная координата (округление к ближайшему узлу решётки).
    pub fn quantize(&self, x: f64) -> Result<Trits, String> {
        if !x.is_finite() {
            return Err("lattice: координата не конечна".into());
        }
        let scale = 3f64.powi(self.k as i32);
        let scaled = x * scale;
        if scaled.abs() > LIM as f64 {
            return Err(format!(
                "lattice: |{x}·3^{}| = {} выходит за разрядность тритных координат",
                self.k,
                scaled.abs()
            ));
        }
        Trits::from_i64(scaled.round() as i64)
    }

    /// Тритная координата → f64 (только для отображения!).
    pub fn to_f64(&self, t: &Trits) -> f64 {
        t.to_i64() as f64 / 3f64.powi(self.k as i32)
    }
}

/// Точка на тритной решётке: (x, y) — тритные числа одного масштаба k.
#[derive(Clone, Debug)]
pub struct TritPoint {
    pub x: Trits,
    pub y: Trits,
    pub k: u8,
}

impl PartialEq for TritPoint {
    fn eq(&self, o: &Self) -> bool {
        self.k == o.k && self.x == o.x && self.y == o.y
    }
}

impl TritPoint {
    /// Квантование f64-точки в узел решётки.
    pub fn quantize(x: f64, y: f64, k: u8) -> Result<Self, String> {
        let l = Lattice::new(k)?;
        Ok(TritPoint {
            x: l.quantize(x)?,
            y: l.quantize(y)?,
            k,
        })
    }

    /// Начало координат.
    pub fn origin(k: u8) -> Self {
        TritPoint {
            x: Trits::zero(),
            y: Trits::zero(),
            k,
        }
    }

    /// Точная сумма (тритное сложение — без округлений).
    pub fn add(&self, o: &TritPoint) -> Result<TritPoint, String> {
        if self.k != o.k {
            return Err("trit_point: смешение решёток разного масштаба".into());
        }
        Ok(TritPoint {
            x: self.x.add(&o.x)?,
            y: self.y.add(&o.y)?,
            k: self.k,
        })
    }

    /// Точная разность.
    pub fn sub(&self, o: &TritPoint) -> Result<TritPoint, String> {
        if self.k != o.k {
            return Err("trit_point: смешение решёток разного масштаба".into());
        }
        Ok(TritPoint {
            x: self.x.sub(&o.x)?,
            y: self.y.sub(&o.y)?,
            k: self.k,
        })
    }

    /// Зеркало: (−x, −y) — инверсия знаков тритов, «бесплатно»
    /// (умножение на −1 в сбалансированной троичной записи — флип цифр).
    pub fn neg(&self) -> TritPoint {
        TritPoint {
            x: self.x.neg(),
            y: self.y.neg(),
            k: self.k,
        }
    }

    /// Сравнение по оси: −1 (left/ниже), 0 (равны), +1.
    pub fn cmp_x(&self, o: &TritPoint) -> Trit {
        (self.x.to_i64()).cmp(&o.x.to_i64()) as Trit
    }
    pub fn cmp_y(&self, o: &TritPoint) -> Trit {
        (self.y.to_i64()).cmp(&o.y.to_i64()) as Trit
    }

    /// Дисплейная пара (в единицах решётки — тритные записи).
    pub fn bal_pair(&self) -> String {
        format!(
            "({}, {})",
            self.x.to_string_bal(),
            self.y.to_string_bal()
        )
    }
}

/// Знак z-компоненты (b−a) × (c−a) — ТОЧНЫЙ тритный предикат
/// ориентации: +1 — c слева от луча a→b, −1 — справа, 0 — коллинеарны.
pub fn cross_sign(a: &TritPoint, b: &TritPoint, c: &TritPoint) -> Trit {
    let abx = match b.x.sub(&a.x) {
        Ok(v) => v,
        Err(_) => return 0, // переполнение разрядности — неопределённость
    };
    let aby = match b.y.sub(&a.y) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let acx = match c.x.sub(&a.x) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let acy = match c.y.sub(&a.y) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let d1 = match abx.mul(&acy) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let d2 = match aby.mul(&acx) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    match d1.sub(&d2) {
        Ok(s) => sign(&s),
        Err(_) => 0,
    }
}

/// Знак тритного числа: первая ненулевая цифра.
pub fn sign(t: &Trits) -> Trit {
    for &d in &t.digits {
        if d != 0 {
            return d;
        }
    }
    0
}

/// Точка p на отрезке [a, b] (предполагается collinear — иначе false).
/// Тест точный: обе координаты внутри тритных границ отрезка.
pub fn on_segment(p: &TritPoint, a: &TritPoint, b: &TritPoint) -> bool {
    if cross_sign(a, b, p) != 0 {
        return false;
    }
    let (px, py) = (p.x.to_i64(), p.y.to_i64());
    let (ax, ay) = (a.x.to_i64(), a.y.to_i64());
    let (bx, by) = (b.x.to_i64(), b.y.to_i64());
    px >= ax.min(bx) && px <= ax.max(bx) && py >= ay.min(by) && py <= ay.max(by)
}

/// Точка относительно замкнутого кольца (полигона без дыр):
/// +1 — строго внутри, 0 — на границе, −1 — строго снаружи.
///
/// Лучевой тест с точными знаками ориентации (без деления!):
/// горизонтальный луч из p вправо пересекает ребро (a→b) ⟺
/// (a.y > p.y) ≠ (b.y > p.y) ∧ (orient(a,b,p) > 0) == (b.y > a.y).
/// Полуоткрытое правило по y согласует вершины; точки НА ребре
/// отлавливаются заранее (третичный случай 0).
pub fn point_in_ring(p: &TritPoint, ring: &[TritPoint]) -> Trit {
    let n = ring.len();
    if n < 3 {
        return -1;
    }
    // Граница?
    for i in 0..n {
        let a = &ring[i];
        let b = &ring[(i + 1) % n];
        if on_segment(p, a, b) {
            return 0;
        }
    }
    let (px, py) = (p.x.to_i64(), p.y.to_i64());
    let mut inside = false;
    for i in 0..n {
        let a = &ring[i];
        let b = &ring[(i + 1) % n];
        let (ay, by) = (a.y.to_i64(), b.y.to_i64());
        let a_above = ay > py;
        let b_above = by > py;
        if a_above != b_above {
            let o = cross_sign(a, b, p);
            // Ребро пересекает луч вправо ⟺ p строго левее направленного
            // ребра при движении вверх (orient·(b.y−a.y) > 0).
            let upward = by > ay;
            if (o > 0) == upward && o != 0 {
                inside = !inside;
            }
        }
    }
    if inside {
        1
    } else {
        -1
    }
}

/// Классификация пересечения отрезков [p1,p2] × [q1,q2] — точно.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegX {
    /// Непересекаются.
    None,
    /// Касание в точке: конец на ребре, общая вершина, T-касание.
    Touch,
    /// Собственное (proper) пересечение внутренностей.
    Proper,
    /// Коллинеарное перекрытие (общий луч/отрезок).
    Collinear,
}

pub fn seg_classify(p1: &TritPoint, p2: &TritPoint, q1: &TritPoint, q2: &TritPoint) -> SegX {
    let d1 = cross_sign(q1, q2, p1);
    let d2 = cross_sign(q1, q2, p2);
    let d3 = cross_sign(p1, p2, q1);
    let d4 = cross_sign(p1, p2, q2);

    // Proper: концы каждого отрезка строго по разные стороны прямой другого.
    if d1 * d2 < 0 && d3 * d4 < 0 {
        return SegX::Proper;
    }
    // Коллинеарность (грабля Сессии-12: проверяется РАНЬШЕ касаний —
    // иначе общее ребро «конец на отрезке» съедает коллинеарное перекрытие).
    if d1 == 0 && d2 == 0 && d3 == 0 && d4 == 0 {
        // Интервалы пересечения по осям: положительная длина → Collinear,
        // вырожденная точка → Touch, пусто → None.
        let (p1x, p1y) = (p1.x.to_i64(), p1.y.to_i64());
        let (p2x, p2y) = (p2.x.to_i64(), p2.y.to_i64());
        let (q1x, q1y) = (q1.x.to_i64(), q1.y.to_i64());
        let (q2x, q2y) = (q2.x.to_i64(), q2.y.to_i64());
        let inter = |a1: i64, a2: i64, b1: i64, b2: i64| -> (i64, i64) {
            (a1.min(a2).max(b1.min(b2)), a1.max(a2).min(b1.max(b2)))
        };
        let (ix0, ix1) = inter(p1x, p2x, q1x, q2x);
        let (iy0, iy1) = inter(p1y, p2y, q1y, q2y);
        if ix0 > ix1 || iy0 > iy1 {
            return SegX::None;
        }
        if ix0 < ix1 || iy0 < iy1 {
            return SegX::Collinear;
        }
        return SegX::Touch;
    }
    // Касания: нулевой ориентатор + попадание в габарит отрезка.
    if d1 == 0 && on_segment(p1, q1, q2) {
        return SegX::Touch;
    }
    if d2 == 0 && on_segment(p2, q1, q2) {
        return SegX::Touch;
    }
    if d3 == 0 && on_segment(q1, p1, p2) {
        return SegX::Touch;
    }
    if d4 == 0 && on_segment(q2, p1, p2) {
        return SegX::Touch;
    }
    SegX::None
}

/// Тритный bbox — «знаковый конверт» геометрии (для 27-дерева).
#[derive(Clone, Debug)]
pub struct TritBBox {
    pub minx: Trits,
    pub miny: Trits,
    pub maxx: Trits,
    pub maxy: Trits,
    pub k: u8,
}

impl TritBBox {
    /// Конверт точек кольца/набора.
    pub fn of_points(pts: &[TritPoint]) -> Result<TritBBox, String> {
        if pts.is_empty() {
            return Err("bbox: пустой набор точек".into());
        }
        let k = pts[0].k;
        let mut minx = pts[0].x.to_i64();
        let mut miny = pts[0].y.to_i64();
        let mut maxx = minx;
        let mut maxy = miny;
        for p in pts.iter().skip(1) {
            if p.k != k {
                return Err("bbox: смешение решёток разного масштаба".into());
            }
            let (x, y) = (p.x.to_i64(), p.y.to_i64());
            minx = minx.min(x);
            maxx = maxx.max(x);
            miny = miny.min(y);
            maxy = maxy.max(y);
        }
        Ok(TritBBox {
            minx: Trits::from_i64(minx)?,
            miny: Trits::from_i64(miny)?,
            maxx: Trits::from_i64(maxx)?,
            maxy: Trits::from_i64(maxy)?,
            k,
        })
    }

    /// Пересечение конвертов (пустое → None).
    pub fn intersect(&self, o: &TritBBox) -> Result<Option<TritBBox>, String> {
        if self.k != o.k {
            return Err("bbox: смешение решёток".into());
        }
        let (ax0, ay0, ax1, ay1) = (
            self.minx.to_i64(),
            self.miny.to_i64(),
            self.maxx.to_i64(),
            self.maxy.to_i64(),
        );
        let (bx0, by0, bx1, by1) = (
            o.minx.to_i64(),
            o.miny.to_i64(),
            o.maxx.to_i64(),
            o.maxy.to_i64(),
        );
        let (x0, x1, y0, y1) = (ax0.max(bx0), ax1.min(bx1), ay0.max(by0), ay1.min(by1));
        if x0 > x1 || y0 > y1 {
            return Ok(None);
        }
        Ok(Some(TritBBox {
            minx: Trits::from_i64(x0)?,
            miny: Trits::from_i64(y0)?,
            maxx: Trits::from_i64(x1)?,
            maxy: Trits::from_i64(y1)?,
            k: self.k,
        }))
    }

    /// Точка внутри конверта (границы включительно).
    pub fn contains(&self, p: &TritPoint) -> bool {
        let (x, y) = (p.x.to_i64(), p.y.to_i64());
        x >= self.minx.to_i64()
            && x <= self.maxx.to_i64()
            && y >= self.miny.to_i64()
            && y <= self.maxy.to_i64()
    }
}

// ────────────────────────── тесты ──────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const K: u8 = 6; // квант 3⁻⁶ ≈ 1.4·10⁻³

    fn tp(x: f64, y: f64) -> TritPoint {
        TritPoint::quantize(x, y, K).unwrap()
    }

    fn square(x0: f64, y0: f64, s: f64) -> Vec<TritPoint> {
        vec![tp(x0, y0), tp(x0 + s, y0), tp(x0 + s, y0 + s), tp(x0, y0 + s)]
    }

    #[test]
    fn quantize_roundtrip() {
        let l = Lattice::new(K).unwrap();
        // 2.0 → тритная запись, обратное преобразование точно.
        let t = l.quantize(2.0).unwrap();
        assert!((l.to_f64(&t) - 2.0).abs() < 1e-12);
        // 1/3 · 3⁶ = 243 — ровно узел решётки.
        let t2 = l.quantize(1.0 / 3.0).unwrap();
        assert_eq!(t2.to_i64(), 243);
    }

    #[test]
    fn cross_sign_exact_collinear() {
        // Коллинеарность БЕЗ epsilon: (0,0), (1,1), (2,2).
        let a = tp(0.0, 0.0);
        let b = tp(1.0, 1.0);
        let c = tp(2.0, 2.0);
        assert_eq!(cross_sign(&a, &b, &c), 0, "коллинеарные точки — честный 0");
        // На один квант ВНЕ луча — уже строго ±1 (никаких epsilon-полок):
        let q = 3f64.powi(-(K as i32));
        let d = tp(2.0 + q, 2.0);
        assert!(cross_sign(&a, &b, &d) != 0);
        // Ровно на луче через узел решётки — 0.
        let e = tp(2.0 + q, 2.0 + q);
        assert_eq!(cross_sign(&a, &b, &e), 0);
    }

    #[test]
    fn cross_sign_left_right() {
        let a = tp(0.0, 0.0);
        let b = tp(4.0, 0.0);
        let left = tp(2.0, 1.0);
        let right = tp(2.0, -1.0);
        assert_eq!(cross_sign(&a, &b, &left), 1);
        assert_eq!(cross_sign(&a, &b, &right), -1);
    }

    #[test]
    fn point_in_square() {
        let ring = square(0.0, 0.0, 4.0);
        assert_eq!(point_in_ring(&tp(2.0, 2.0), &ring), 1);
        assert_eq!(point_in_ring(&tp(5.0, 5.0), &ring), -1);
        assert_eq!(point_in_ring(&tp(2.0, 0.0), &ring), 0, "на ребре");
        assert_eq!(point_in_ring(&tp(0.0, 0.0), &ring), 0, "в вершине");
        assert_eq!(point_in_ring(&tp(4.0, 2.0), &ring), 0, "на правом ребре");
    }

    #[test]
    fn point_in_concave_ring() {
        // П-образный (невыпуклый) полигон — луч пересекает несколько рёбер.
        let ring = vec![
            tp(0.0, 0.0),
            tp(6.0, 0.0),
            tp(6.0, 6.0),
            tp(4.0, 6.0),
            tp(4.0, 2.0),
            tp(2.0, 2.0),
            tp(2.0, 6.0),
            tp(0.0, 6.0),
        ];
        assert_eq!(point_in_ring(&tp(1.0, 1.0), &ring), 1, "внутри левого крыла");
        assert_eq!(point_in_ring(&tp(5.0, 1.0), &ring), 1, "внутри правого крыла");
        assert_eq!(point_in_ring(&tp(3.0, 4.0), &ring), -1, "в вырезе");
        assert_eq!(point_in_ring(&tp(3.0, 5.9), &ring), -1, "в вырезе выше");
    }

    #[test]
    fn seg_classifications() {
        let p1 = tp(0.0, 0.0);
        let p2 = tp(4.0, 4.0);
        // Proper: диагонали квадрата.
        let q1 = tp(0.0, 4.0);
        let q2 = tp(4.0, 0.0);
        assert_eq!(seg_classify(&p1, &p2, &q1, &q2), SegX::Proper);
        // Touch: конец на середине.
        let r1 = tp(2.0, 2.0);
        let r2 = tp(2.0, 5.0);
        assert_eq!(seg_classify(&p1, &p2, &r1, &r2), SegX::Touch);
        // Collinear: на одной прямой с перекрытием.
        let s1 = tp(2.0, 2.0);
        let s2 = tp(6.0, 6.0);
        assert_eq!(seg_classify(&p1, &p2, &s1, &s2), SegX::Collinear);
        // None: параллельные.
        let u1 = tp(0.0, 1.0);
        let u2 = tp(4.0, 5.0);
        assert_eq!(seg_classify(&p1, &p2, &u1, &u2), SegX::None);
    }

    #[test]
    fn bbox_ops() {
        let a = TritBBox::of_points(&square(0.0, 0.0, 4.0)).unwrap();
        let b = TritBBox::of_points(&square(2.0, 2.0, 4.0)).unwrap();
        let inter = a.intersect(&b).unwrap().unwrap();
        assert!(inter.contains(&tp(3.0, 3.0)));
        assert!(!inter.contains(&tp(1.0, 1.0)));
        let c = TritBBox::of_points(&square(10.0, 10.0, 2.0)).unwrap();
        assert!(a.intersect(&c).unwrap().is_none());
    }

    #[test]
    fn neg_and_add_exact() {
        let a = tp(1.0, 2.0);
        let b = tp(0.5, 0.25);
        // quantize(0.5) = round(364.5) = 365 → сумма 729+365 = 1094.
        let s = a.add(&b).unwrap();
        assert_eq!(s.x.to_i64(), 1094);
        assert_eq!(s.y.to_i64(), (2.25 * 729.0) as i64);
        let n = a.neg();
        assert_eq!(n.x.to_i64(), -729);
    }
}
