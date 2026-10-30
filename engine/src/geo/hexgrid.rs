//! Гекс-сетка на трит-спирали — мини-порт H3 (Сессия-14).
//!
//! Якорь порта: `h3.poler` (uber/h3): битовая упаковка индекса ячейки,
//! кольца/диски (hexRing, kRing). Тритная замена битов:
//!
//! * **осевые координаты** (q, r) — гекс-решётка без тригонометрии:
//!   соседей 6, дистанция (|Δq| + |Δr| + |Δq+Δr|)/2 — целочисленно;
//! * **трит-спираль** — обход диска по кольцам: центр s=0, кольцо k
//!   занимает s ∈ [3k²−3k+1, 3k²+3k]; ОДИН 6-тритный блок держит
//!   729 состояний (смещённый код s−364) → диски до радиуса 13
//!   (547 ячеек) упакованы в 6 тритов — против байта+резерва в H3;
//! * **икосаэдр** — 20 граней из 12 вершин золотого сечения (как в H3):
//!   индекс грани — 3 трита (смещённый код f−13, 27 состояний),
//!   адрес ячейки мира = 3 трита (грань) + 6 тритов (спираль) = 9 тритов —
//!   ровно один DE-9IM-блок: топология и география живут в одном формате.
//!
//! Детерминизм: обход колец фиксирован (SW-старт, направления E→NE→NW→W→SW→SE),
//! биты адреса воспроизводимы от запуска к запуску.

use crate::calc::trits::Trits;

/// Направления соседей (осевые): E, NE, NW, W, SW, SE.
pub const AXIAL_DIRS: [(i64, i64); 6] = [
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, 0),
    (-1, 1),
    (0, 1),
];

/// Максимальный радиус диска в 6-тритном блоке: s_max = 3·13·14 = 546 < 729.
pub const MAX_DISK_RADIUS: i64 = 13;

/// Смещение кода спирали: 6 тритов (±364) + 365 = диапазон s ∈ [0, 728].
const SPIRAL_OFFSET: i64 = 364;
/// Смещение кода грани: 3 трита (±13) + 13 = диапазон f ∈ [0, 26].
const FACE_OFFSET: i64 = 13;

/// Гекс-дистанция между осевыми координатами (целочисленно).
pub fn hex_distance(a: (i64, i64), b: (i64, i64)) -> i64 {
    let (dq, dr) = (a.0 - b.0, a.1 - b.1);
    (dq.abs() + dr.abs() + (dq + dr).abs()) / 2
}

/// Соседи ячейки (6 штук).
pub fn hex_neighbors(c: (i64, i64)) -> [(i64, i64); 6] {
    AXIAL_DIRS.map(|(dq, dr)| (c.0 + dq, c.1 + dr))
}

/// Кольцо радиуса k (6k ячеек, k ≥ 1): старт SW, обход по направлениям.
pub fn hex_ring(k: i64) -> Vec<(i64, i64)> {
    if k <= 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity((6 * k) as usize);
    let mut hex = (-k, k); // центр + k·SW
    for &d in AXIAL_DIRS.iter() {
        for _ in 0..k {
            out.push(hex);
            hex = (hex.0 + d.0, hex.1 + d.1);
        }
    }
    out
}

/// Диск радиуса r (1 + 3r(r+1) ячеек): центр + кольца 1..=r.
pub fn hex_disk(r: i64) -> Vec<(i64, i64)> {
    if r < 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity((1 + 3 * r * (r + 1)) as usize);
    out.push((0, 0));
    for k in 1..=r {
        out.extend(hex_ring(k));
    }
    out
}

/// Осевые координаты → индекс трит-спирали s (позиция в обходе диска).
pub fn axial_to_spiral(q: i64, r: i64) -> Result<i64, String> {
    let k = hex_distance((0, 0), (q, r));
    if k == 0 {
        return Ok(0);
    }
    if k > MAX_DISK_RADIUS {
        return Err(format!(
            "hex: дистанция {k} > {MAX_DISK_RADIUS} — вне 6-тритного блока спирали"
        ));
    }
    let base = 3 * k * (k - 1) + 1; // первый s кольца k
    let want = (q, r);
    let pos = hex_ring(k)
        .iter()
        .position(|&c| c == want)
        .ok_or_else(|| "hex: кольцо не содержит ячейку (внутренняя ошибка)".to_string())?;
    Ok(base + pos as i64)
}

/// Индекс спирали s → осевые координаты.
pub fn spiral_to_axial(s: i64) -> Result<(i64, i64), String> {
    if s < 0 || s > 3 * MAX_DISK_RADIUS * (MAX_DISK_RADIUS + 1) {
        return Err(format!(
            "hex: s = {s} вне диапазона 6-тритного блока [0, {}]",
            3 * MAX_DISK_RADIUS * (MAX_DISK_RADIUS + 1)
        ));
    }
    if s == 0 {
        return Ok((0, 0));
    }
    // кольцо k занимает s ∈ [3k²−3k+1, 3k²+3k] → k = floor((3+√(9+12(s−1)))/6)
    let kf = ((9.0 + 12.0 * (s - 1) as f64).sqrt() + 3.0) / 6.0;
    let mut k = kf.floor() as i64;
    // корректировка границ (f64-квадратный корень у края диапазона)
    while k > 1 && s < 3 * k * (k - 1) + 1 {
        k -= 1;
    }
    while 3 * k * (k + 1) < s {
        k += 1;
    }
    let base = 3 * k * (k - 1) + 1;
    let ring = hex_ring(k);
    let idx = (s - base) as usize;
    if idx >= ring.len() {
        return Err(format!("hex: s = {s} не ложится в кольцо {k} (внутренняя ошибка)"));
    }
    Ok(ring[idx])
}

/// Трит-адрес ячейки диска: 6-тритный блок, смещённый код s−364.
pub fn hex_trit_id(s: i64) -> Result<Trits, String> {
    let code = s - SPIRAL_OFFSET;
    if code < -364 || code > 364 {
        return Err(format!(
            "hex: s = {s} вне 6-тритного блока (729 состояний, диски ≤ r {MAX_DISK_RADIUS})"
        ));
    }
    let t = Trits::from_i64(code)?;
    // выравнивание до 6 разрядов (старшие нули значимы — это блок)
    let mut digits = t.digits.clone();
    while digits.len() < 6 {
        digits.insert(0, 0);
    }
    Ok(Trits { digits })
}

/// Обратное к `hex_trit_id`: 6-тритный блок → s.
pub fn trit_id_to_hex(t: &Trits) -> Result<i64, String> {
    if t.len() > 6 {
        return Err(format!("hex: адрес {:?} длиннее 6 тритов", t.digits));
    }
    Ok(t.to_i64() + SPIRAL_OFFSET)
}

/// Адрес ячейки мира: грань икосаэдра (3 трита) + спираль (6 тритов) = 9.
pub fn world_trit_id(face: usize, s: i64) -> Result<Trits, String> {
    if face >= 20 {
        return Err(format!("hex: грань {face} вне икосаэдра (0..19)"));
    }
    let fcode = face as i64 - FACE_OFFSET;
    let fc = Trits::from_i64(fcode)?;
    let mut fd = fc.digits.clone();
    while fd.len() < 3 {
        fd.insert(0, 0);
    }
    let sc = hex_trit_id(s)?;
    let mut digits = fd;
    digits.extend(sc.digits.iter().cloned());
    Ok(Trits { digits })
}

/// Разбор мирового адреса: (грань, s).
pub fn world_trit_parse(t: &Trits) -> Result<(usize, i64), String> {
    let mut digits = t.digits.clone();
    while digits.len() < 9 {
        digits.insert(0, 0);
    }
    if digits.len() > 9 {
        return Err(format!("hex: мировой адрес {:?} длиннее 9 тритов", t.digits));
    }
    // грань — ПЕРВЫЕ 3 трита, спираль — ПОСЛЕДНИЕ 6
    let sd = digits.split_off(3);
    let fd = digits;
    let face = (Trits { digits: fd }.to_i64() + FACE_OFFSET) as usize;
    let s = trit_id_to_hex(&Trits { digits: sd })?;
    if face >= 20 {
        return Err(format!("hex: грань {face} вне икосаэдра"));
    }
    Ok((face, s))
}

// ─────────────── res>0: дробление апертурой 9 (Сессия-15) ───────────────
//
// Доктрина CSE (universal_letters): аналитика вместо LUT. Настоящий H3
// использует апертуру 7 — масштабирование на √7 иррационально, посему
// мегабайты таблиц поворотов и пентагон-исключений. Тритная апертура 9
// = 3² — ТОЧНОЕ решёточное масштабирование осевых координат:
//
//   потомок(q, r) = (3q + i, 3r + j),  i, j ∈ {−1, 0, +1}
//
// Девять потомков, ребро /3, адрес = +2 трита уточнения на уровень
// (сбалансированная пара (i, j)). Родитель — div_euclid(3). Ноль таблиц,
// чистая целочисленная арифметика, воспроизводимость побитово.

/// Апертура дробления: 9 = 3² (решёточный масштаб).
pub const APERTURE: i64 = 3;

/// Девять потомков ячейки (aperture 9): (3q+i, 3r+j), порядок детерминирован.
pub fn h3_children(q: i64, r: i64) -> [(i64, i64); 9] {
    let mut out = [(0i64, 0i64); 9];
    let mut n = 0usize;
    for &i in &[-1i64, 0, 1] {
        for &j in &[-1i64, 0, 1] {
            out[n] = (APERTURE * q + i, APERTURE * r + j);
            n += 1;
        }
    }
    out
}

/// Родитель ячейки + уточняющие триты: ((q', r'), (i, j)).
/// Сбалансированное деление: остаток 2 ≡ трит −1 С ЗАЁМОМ из частного
/// (−1 = 3·0 − 1, а не 3·(−1) + 2 — грабля №31, поймана тестом).
pub fn h3_parent(q: i64, r: i64) -> ((i64, i64), (i64, i64)) {
    let bal_div = |v: i64| -> (i64, i64) {
        let mut qu = v.div_euclid(APERTURE);
        let mut re = v.rem_euclid(APERTURE); // ∈ {0, 1, 2}
        if re == 2 {
            re = -1;
            qu += 1;
        }
        (qu, re)
    };
    let (pq, i) = bal_div(q);
    let (pr, j) = bal_div(r);
    ((pq, pr), (i, j))
}

/// Уточняющая трит-пара (i, j) → 2 трита (старший i, младший j).
pub fn refine_trits(i: i64, j: i64) -> Result<Trits, String> {
    if !(-1..=1).contains(&i) || !(-1..=1).contains(&j) {
        return Err(format!("hex: уточнение ({i},{j}) — триты, только −1/0/+1"));
    }
    let ti = Trits::from_i64(i)?;
    let tj = Trits::from_i64(j)?;
    Ok(Trits {
        digits: [ti.digits.clone(), tj.digits.clone()].concat(),
    })
}

/// Разбор уточняющей пары: 2 трита → (i, j).
pub fn parse_refine(t: &Trits) -> Result<(i64, i64), String> {
    if t.len() != 2 {
        return Err(format!("hex: уточнение {:?} — ровно 2 трита", t.digits));
    }
    let i = Trits { digits: vec![t.digits[0]] }.to_i64();
    let j = Trits { digits: vec![t.digits[1]] }.to_i64();
    Ok((i, j))
}

/// Дочерний мировой адрес: родитель (9+2k тритов) + уточнение (i, j).
pub fn child_world_address(parent: &Trits, i: i64, j: i64) -> Result<Trits, String> {
    if parent.len() < 9 || parent.len() % 2 == 0 {
        return Err(format!(
            "hex: родительский адрес {:?} — нечётная длина ≥ 9 (res-0 = 9)",
            parent.digits
        ));
    }
    let mut digits = parent.digits.clone();
    digits.extend(refine_trits(i, j)?.digits.iter().cloned());
    Ok(Trits { digits })
}

/// Родительский мировой адрес + уточнение: (адрес, i, j).
pub fn parent_world_address(child: &Trits) -> Result<(Trits, i64, i64), String> {
    if child.len() < 11 || child.len() % 2 == 0 {
        return Err(format!(
            "hex: адрес {:?} — дроблёный (нечётная длина ≥ 11)",
            child.digits
        ));
    }
    let (i, j) = parse_refine(&Trits {
        digits: child.digits[child.digits.len() - 2..].to_vec(),
    })?;
    let parent = Trits {
        digits: child.digits[..child.digits.len() - 2].to_vec(),
    };
    Ok((parent, i, j))
}

/// Разрешение адреса: res = (длина − 9) / 2 (уровней дробления).
pub fn world_address_res(t: &Trits) -> Result<usize, String> {
    let n = t.len();
    if n < 9 || n % 2 == 0 {
        return Err(format!(
            "hex: адрес {:?} — длина ≥ 9 и нечётная (9 + 2·res)",
            t.digits
        ));
    }
    Ok((n - 9) / 2)
}

// ─────────────── res→2: полный адрес с любого уровня (Сессия-17) ───────────────
//
// Сессия-15 построила механику дробления на ОДИН уровень (children/parent,
// +2 трита). Сессия-17 замыкает цикл координаты ⇄ адрес на ЛЮБОМ уровне:
//
//   ячейка res-2 (q, r) ──два родителя вверх──▶ (q₀, r₀) + уточнения
//   (i₁,j₁), (i₂,j₂) ──спираль res-0──▶ s ──грань──▶ 9 тритов
//   ──+refine(i₁,j₁)+refine(i₂,j₂)──▶ 13 тритов
//
// Каноническая глубина — res 2 (адрес 13 тритов); механика допускает
// до MAX_RES = 6 (17 тритов, масштаб решётки 3⁶ = 729 — с запасом i64).
// Ограничение одно честное: res-0 предок должен лежать в диске R ≤ 13
// (6-тритная спираль).

/// Каноническая глубина Сессии-17: res 2 = адрес 13 тритов.
pub const CANON_RES: usize = 2;
/// Предел механики адресов: 9 + 2·6 = 17 тритов.
pub const MAX_RES: usize = 6;

/// Все потомки ячейки на уровне `res` (9^res ячеек, res ≤ MAX_RES).
/// res 0 → сама ячейка; res 1 → 9 детей; res 2 → 81 внук.
pub fn h3_descendants(q: i64, r: i64, res: usize) -> Result<Vec<(i64, i64)>, String> {
    if res > MAX_RES {
        return Err(format!(
            "hex: res = {res} > {MAX_RES} (адрес ≤ 9 + 2·{MAX_RES} = {} тритов)",
            9 + 2 * MAX_RES
        ));
    }
    let mut cells = vec![(q, r)];
    for _ in 0..res {
        let mut next = Vec::with_capacity(cells.len() * 9);
        for &(cq, cr) in &cells {
            next.extend(h3_children(cq, cr));
        }
        cells = next;
    }
    Ok(cells)
}

/// Мировой адрес ячейки (q, r) НА УРОВНЕ res: грань (3 трита) +
/// спираль res-0 родителя (6 тритов) + res уточняющих пар (2 трита
/// каждая). res 2 → 13 тритов. Путь родителей наверх — чистая
/// сбалансированная тройка, как у `h3_parent`.
pub fn world_address(face: usize, q: i64, r: i64, res: usize) -> Result<Trits, String> {
    if face >= 20 {
        return Err(format!("hex: грань {face} вне икосаэдра (0..19)"));
    }
    if res > MAX_RES {
        return Err(format!(
            "hex: res = {res} > {MAX_RES} — адрес ≤ {} тритов",
            9 + 2 * MAX_RES
        ));
    }
    // поднимаемся res уровней: уточнения собираются СВЕРХУ ВНИЗ,
    // в адрес кладутся СНИЗУ ВВЕРХ (первая пара — первый уровень)
    let mut refinements: Vec<(i64, i64)> = Vec::with_capacity(res);
    let (mut cq, mut cr) = (q, r);
    for _ in 0..res {
        let ((pq, pr), ij) = h3_parent(cq, cr);
        refinements.push(ij);
        (cq, cr) = (pq, pr);
    }
    // res-0 родитель должен попасть в 6-тритную спираль
    let s = axial_to_spiral(cq, cr)?;
    let mut t = world_trit_id(face, s)?;
    for &(i, j) in refinements.iter().rev() {
        t = child_world_address(&t, i, j)?;
    }
    debug_assert_eq!(t.len(), 9 + 2 * res);
    Ok(t)
}

/// Обратный к [`world_address`]: адрес + ожидаемое разрешение →
/// (грань, q, r на уровне res). Пары уточнения применяются слева
/// направо: q ← 3q + i.
///
/// res задаётся ЯВНО: `Trits::parse` обрезает ведущие нули (грани 4..19
/// кодируются с нулевой старшей тройкой), длина разобранного адреса
/// ненадёжна — источник длины только строка-оригинал.
pub fn world_address_parse(t: &Trits, res: usize) -> Result<(usize, i64, i64), String> {
    if res > MAX_RES {
        return Err(format!(
            "hex: res = {res} > {MAX_RES} (механика Сессии-17)"
        ));
    }
    let want_len = 9 + 2 * res;
    let mut digits = t.digits.clone();
    if digits.len() > want_len {
        return Err(format!(
            "hex: адрес {:?} длиннее {want_len} тритов для res {res}",
            t.digits
        ));
    }
    while digits.len() < want_len {
        digits.insert(0, 0);
    }
    // база 9 тритов + res пар уточнения
    let base: Vec<i8> = digits[..9].to_vec();
    let (face, s) = world_trit_parse(&Trits { digits: base })?;
    let (mut q, mut r) = spiral_to_axial(s)?;
    for lvl in 0..res {
        let pair = &digits[9 + 2 * lvl..9 + 2 * lvl + 2];
        let (i, j) = parse_refine(&Trits {
            digits: pair.to_vec(),
        })?;
        (q, r) = (3 * q + i, 3 * r + j);
    }
    Ok((face, q, r))
}

// ─────────────── икосаэдр H3: 20 граней золотого сечения ───────────────

/// 12 вершин икосаэдра (0, ±1, ±φ) на описанной сфере R = √(1+φ²) ≈ 1.902
/// (рёбра длины 2 — точные пороги граней без округлений).
pub fn icosa_vertices() -> [(f64, f64, f64); 12] {
    let p = (1.0 + 5.0f64.sqrt()) / 2.0;
    let mut v = Vec::with_capacity(12);
    for &s1 in [1.0, -1.0].iter() {
        for &s2 in [p, -p].iter() {
            v.push((0.0, s1, s2));
            v.push((s1, s2, 0.0));
            v.push((s2, 0.0, s1));
        }
    }
    // v имеет дубликаты циклических перестановок — собираем уникальные
    let mut uniq: Vec<(f64, f64, f64)> = Vec::new();
    for x in v {
        if !uniq.iter().any(|&u| {
            (u.0 - x.0).abs() < 1e-9 && (u.1 - x.1).abs() < 1e-9 && (u.2 - x.2).abs() < 1e-9
        }) {
            uniq.push(x);
        }
    }
    let mut arr = [(0.0, 0.0, 0.0); 12];
    for (i, &u) in uniq.iter().take(12).enumerate() {
        arr[i] = u;
    }
    arr
}

/// 20 граней икосаэдра: тройки вершин с ребрами длины 2 (сфера R=1).
pub fn icosa_faces() -> Vec<[usize; 3]> {
    let v = icosa_vertices();
    let d2 = |a: (f64, f64, f64), b: (f64, f64, f64)| {
        (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)
    };
    let mut faces = Vec::new();
    for i in 0..12 {
        for j in (i + 1)..12 {
            if (d2(v[i], v[j]) - 4.0).abs() > 1e-9 {
                continue; // не ребро
            }
            for k in (j + 1)..12 {
                if (d2(v[i], v[k]) - 4.0).abs() < 1e-9
                    && (d2(v[j], v[k]) - 4.0).abs() < 1e-9
                {
                    faces.push([i, j, k]);
                }
            }
        }
    }
    faces
}

/// Центры граней (нормированные) — для выбора грани направления.
pub fn icosa_face_centers() -> Vec<(f64, f64, f64)> {
    let v = icosa_vertices();
    icosa_faces()
        .iter()
        .map(|&[a, b, c]| {
            let mut x = (v[a].0 + v[b].0 + v[c].0) / 3.0;
            let mut y = (v[a].1 + v[b].1 + v[c].1) / 3.0;
            let mut z = (v[a].2 + v[b].2 + v[c].2) / 3.0;
            let n = (x * x + y * y + z * z).sqrt();
            x /= n;
            y /= n;
            z /= n;
            (x, y, z)
        })
        .collect()
}

/// Грань направления: максимум скалярного произведения с центрами.
pub fn face_of_direction(dir: (f64, f64, f64)) -> Result<usize, String> {
    let (x, y, z) = dir;
    let n = (x * x + y * y + z * z).sqrt();
    if !n.is_finite() || n < 1e-12 {
        return Err("hex: нулевое направление не задаёт грань".into());
    }
    let (x, y, z) = (x / n, y / n, z / n);
    let centers = icosa_face_centers();
    let mut best = 0usize;
    let mut best_dot = f64::NEG_INFINITY;
    for (i, &c) in centers.iter().enumerate() {
        let dot = c.0 * x + c.1 * y + c.2 * z;
        if dot > best_dot {
            best_dot = dot;
            best = i;
        }
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aperture9_children_and_parents() {
        // 9 потомков, все различны, roundtrip родитель↔потомок
        for &(q, r) in &[(0i64, 0i64), (2, -3), (-5, 7), (13, 13)] {
            let kids = h3_children(q, r);
            let mut sorted: Vec<(i64, i64)> = kids.to_vec();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), 9, "потомки ({q},{r}) не уникальны");
            for &(cq, cr) in &kids {
                let ((pq, pr), (i, j)) = h3_parent(cq, cr);
                assert_eq!((pq, pr), (q, r), "родитель {cq},{cr} ≠ {q},{r}");
                assert_eq!((3 * pq + i, 3 * pr + j), (cq, cr));
            }
        }
        // потомки соседних родителей соседствуют на res-1 (ребро /3)
        let a = h3_children(0, 0);
        let b = h3_children(1, 0);
        // потомок (1,0) родителя (0,0) [i=+1,j=0] и (2,0) родителя (1,0) [i=−1]
        assert!(a.contains(&(1, 0)));
        assert!(b.contains(&(2, 0)));
        assert_eq!(hex_distance((1, 0), (2, 0)), 1, "граница res-1");
        // дистанции внутри семейства: центр-потомок (0,0) к (1,0) = 1
        assert_eq!(hex_distance((0, 0), (1, 0)), 1);
    }

    #[test]
    fn refinement_trit_roundtrip() {
        for &i in &[-1i64, 0, 1] {
            for &j in &[-1i64, 0, 1] {
                let t = refine_trits(i, j).unwrap();
                assert_eq!(t.len(), 2, "({i},{j}) — 2 трита");
                let (i2, j2) = parse_refine(&t).unwrap();
                assert_eq!((i2, j2), (i, j));
            }
        }
        assert!(refine_trits(2, 0).is_err());
        assert!(parse_refine(&Trits { digits: vec![1] }).is_err());
        // 9 состояний 2 тритов — весь диапазон уточнения
        let mut codes = std::collections::HashSet::new();
        for &i in &[-1i64, 0, 1] {
            for &j in &[-1i64, 0, 1] {
                codes.insert(refine_trits(i, j).unwrap().to_i64());
            }
        }
        assert_eq!(codes.len(), 9);
    }

    #[test]
    fn world_address_refinement_res() {
        // res-0 адрес → res-1 → res-2, обратно, разрешение считается
        let base = world_trit_id(7, 42).unwrap();
        assert_eq!(world_address_res(&base).unwrap(), 0);
        let c1 = child_world_address(&base, -1, 1).unwrap();
        assert_eq!(c1.len(), 11);
        assert_eq!(world_address_res(&c1).unwrap(), 1);
        let c2 = child_world_address(&c1, 0, -1).unwrap();
        assert_eq!(c2.len(), 13);
        assert_eq!(world_address_res(&c2).unwrap(), 2);
        // peel-back: два уровня вверх
        let (p1, i1, j1) = parent_world_address(&c2).unwrap();
        assert_eq!((i1, j1), (0, -1));
        assert_eq!(p1.digits, c1.digits);
        let (p0, i0, j0) = parent_world_address(&p1).unwrap();
        assert_eq!((i0, j0), (-1, 1));
        assert_eq!(p0.digits, base.digits);
        // ошибки формы
        assert!(child_world_address(&Trits { digits: vec![0; 8] }, 0, 0).is_err());
        assert!(parent_world_address(&base).is_err());
        assert!(world_address_res(&Trits { digits: vec![0; 10] }).is_err());
    }

    #[test]
    fn children_tile_parent_neighborhood() {
        // слияние семей: 4 соседних res-0 родителя дают связный res-1 блок,
        // все 36 ячеек различны, дистанции согласованы с решёткой 3×
        let parents = [(0i64, 0i64), (1, 0), (0, 1), (1, 1)];
        let mut cells = Vec::new();
        for &(q, r) in &parents {
            cells.extend(h3_children(q, r));
        }
        let mut sorted = cells.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 36, "4 семьи × 9 = 36 уникальных res-1 ячеек");
        // res-1 блок [−1,3]² содержит свои углы
        assert!(cells.contains(&(-1, -1)));
        assert!(cells.contains(&(3, 3)));
    }

    #[test]
    fn ring_sizes_and_distances() {
        assert_eq!(hex_ring(1).len(), 6);
        assert_eq!(hex_ring(5).len(), 30);
        assert_eq!(hex_disk(0).len(), 1);
        assert_eq!(hex_disk(1).len(), 7);
        assert_eq!(hex_disk(10).len(), 331);
        assert_eq!(hex_disk(13).len(), 547);
        // все ячейки кольца k на дистанции k, все уникальны
        for k in [1i64, 4, 9] {
            let ring = hex_ring(k);
            for &c in &ring {
                assert_eq!(hex_distance((0, 0), c), k, "кольцо {k}: {c:?}");
            }
            let mut sorted = ring.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), ring.len(), "кольцо {k}: дубликаты");
        }
    }

    #[test]
    fn spiral_roundtrip_full_disk() {
        // полный диск радиуса 10 (331 ячейка) — туда и обратно
        for &c in hex_disk(10).iter() {
            let s = axial_to_spiral(c.0, c.1).unwrap();
            let back = spiral_to_axial(s).unwrap();
            assert_eq!(back, c, "s={s}");
        }
        // граничные значения
        assert_eq!(axial_to_spiral(0, 0).unwrap(), 0);
        assert_eq!(spiral_to_axial(0).unwrap(), (0, 0));
        // первый шаг кольца 1 = старт SW = (−1, 1)
        assert_eq!(spiral_to_axial(1).unwrap(), (-1, 1));
        assert_eq!(axial_to_spiral(-1, 1).unwrap(), 1);
        // последнее s кольца k: 3k²+3k
        for k in [1i64, 5, 10, 13] {
            let s_max = 3 * k * (k + 1);
            let c = spiral_to_axial(s_max).unwrap();
            assert_eq!(hex_distance((0, 0), c), k, "s_max кольца {k}");
        }
    }

    #[test]
    fn six_trit_block_packing() {
        // s=0 → код −364 → 6 тритов
        let t0 = hex_trit_id(0).unwrap();
        assert_eq!(t0.len(), 6);
        assert_eq!(trit_id_to_hex(&t0).unwrap(), 0);
        // весь диапазон блока: s ∈ [0, 728]
        for s in [0i64, 1, 364, 365, 546, 728] {
            let t = hex_trit_id(s).unwrap();
            assert_eq!(t.len(), 6, "s={s}");
            assert_eq!(trit_id_to_hex(&t).unwrap(), s, "s={s}");
        }
        // вне блока — ошибка
        assert!(hex_trit_id(729).is_err());
        assert!(hex_trit_id(-1).is_err());
        // полный диск радиуса 13 пакуется
        for &c in hex_disk(13).iter() {
            let s = axial_to_spiral(c.0, c.1).unwrap();
            let t = hex_trit_id(s).unwrap();
            assert_eq!(trit_id_to_hex(&t).unwrap(), s);
        }
    }

    #[test]
    fn world_address_nine_trits() {
        // (грань, s) → 9 тритов → обратно
        for (face, s) in [(0usize, 0i64), (7, 42), (19, 546)] {
            let t = world_trit_id(face, s).unwrap();
            assert_eq!(t.len(), 9, "грань {face}, s {s}");
            let (f2, s2) = world_trit_parse(&t).unwrap();
            assert_eq!((f2, s2), (face, s));
        }
        assert!(world_trit_id(20, 0).is_err());
        assert!(world_trit_id(0, 729).is_err());
    }

    #[test]
    fn neighbors_are_at_unit_distance() {
        for &n in hex_neighbors((3, -2)).iter() {
            assert_eq!(hex_distance((3, -2), n), 1);
        }
        // сосед центра (0,1) — классическая гекс-координата
        assert!(hex_neighbors((0, 0)).contains(&(0, 1)));
        assert!(hex_neighbors((0, 0)).contains(&(1, 0)));
        assert!(hex_neighbors((0, 0)).contains(&(1, -1)));
    }

    #[test]
    fn world_address_res2_roundtrip() {
        // координаты ⇄ адрес на res 0/1/2/3 — полный цикл
        for (face, q, r) in [
            (7usize, 2i64, -3i64),
            (0, 0, 0),
            (19, -5, 7),
            (4, 9, 9), // грань с нулевой старшей тройкой
            (13, 3, 1),
        ] {
            for res in [0usize, 1, 2, 3] {
                // поднимаем res уровней — res-0 предок должен попасть в R13
                let (mut pq, mut pr) = (q, r);
                for _ in 0..res {
                    let ((a, b), _) = h3_parent(pq, pr);
                    (pq, pr) = (a, b);
                }
                if hex_distance((0, 0), (pq, pr)) > MAX_DISK_RADIUS {
                    continue; // вне спирали — ошибка отдельно проверена
                }
                let t = world_address(face, q, r, res).unwrap();
                assert_eq!(t.len(), 9 + 2 * res, "res {res}: {t:?}");
                let (f2, q2, r2) = world_address_parse(&t, res).unwrap();
                assert_eq!((f2, q2, r2), (face, q, r), "res {res} roundtrip");
            }
        }
        // res 2 → ровно 13 тритов, res-0 → 9
        assert_eq!(
            world_address(7, 2, -3, CANON_RES).unwrap().len(),
            13,
            "канон Сессии-17: 13 тритов"
        );
        assert_eq!(world_address(7, 2, -3, 0).unwrap().len(), 9);
        // координаты res-2 (9q+i)·3+j адресуются честно
        let q2 = 3 * 3 * 2 + 1; // i₁=+1, i₂=+... строим снизу
        let r2 = 3 * 3 * -3 - 1;
        let t = world_address(7, q2, r2, 2).unwrap();
        let (_, qq, rr) = world_address_parse(&t, 2).unwrap();
        assert_eq!((qq, rr), (q2, r2), "res-2 координаты roundtrip");
        // ошибки: res за пределом, грань, далёкая res-0 клетка
        assert!(world_address(7, 0, 0, MAX_RES + 1).is_err());
        assert!(world_address(20, 0, 0, 2).is_err());
        assert!(world_address(7, 400, 400, 2).is_err());
        // разбор: адрес длиннее заявленного res / res за пределом
        let t13 = world_address(7, 2, -3, 2).unwrap();
        assert!(world_address_parse(&t13, 1).is_err());
        assert!(world_address_parse(&t13, MAX_RES + 1).is_err());
    }

    #[test]
    fn descendants_tile_and_center_path() {
        // 81 внук res-2, все уникальны, соседи не пересекаются
        let d00 = h3_descendants(0, 0, 2).unwrap();
        assert_eq!(d00.len(), 81, "9² = 81");
        let mut sorted = d00.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 81, "уникальные");
        let d10 = h3_descendants(1, 0, 2).unwrap();
        let mut all = d00.clone();
        all.extend(d10.iter().cloned());
        let mut s2 = all.clone();
        s2.sort();
        s2.dedup();
        assert_eq!(s2.len(), 162, "семьи соседей не пересекаются");
        // res 1 == h3_children
        let d1 = h3_descendants(2, -3, 1).unwrap();
        assert_eq!(d1, h3_children(2, -3).to_vec());
        // res 0 — сама ячейка
        assert_eq!(h3_descendants(5, 5, 0).unwrap(), vec![(5, 5)]);
        // центр-потомок res-2 остаётся в (9q, 9r)
        assert!(d00.contains(&(0, 0)));
        let d22 = h3_descendants(2, 2, 2).unwrap();
        assert!(d22.contains(&(18, 18)), "центр 9·(2,2)");
        // границы: res-2 потомки (0,0) занимают [−4, +4], потомки (1,0) — [5, 13]
        assert!(d00.contains(&(-4, -4)) && d00.contains(&(4, 4)));
        assert!(d10.contains(&(5, -4)) && d10.contains(&(13, 4)));
        // ошибка res > MAX_RES
        assert!(h3_descendants(0, 0, MAX_RES + 1).is_err());
    }

    #[test]
    fn icosahedron_structure() {
        let v = icosa_vertices();
        // 12 вершин на описанной сфере R = √(1+φ²) ≈ 1.902
        let p = (1.0 + 5.0f64.sqrt()) / 2.0;
        let r_expected = (1.0 + p * p).sqrt();
        for &x in v.iter() {
            let r = (x.0 * x.0 + x.1 * x.1 + x.2 * x.2).sqrt();
            assert!((r - r_expected).abs() < 1e-9, "вершина не на сфере: {x:?}");
        }
        let faces = icosa_faces();
        assert_eq!(faces.len(), 20, "у икосаэдра 20 граней");
        let centers = icosa_face_centers();
        assert_eq!(centers.len(), 20);
        // центры граней — на ЕДИНИЧНОЙ сфере (для face_of_direction)
        for &c in centers.iter() {
            let r = (c.0 * c.0 + c.1 * c.1 + c.2 * c.2).sqrt();
            assert!((r - 1.0).abs() < 1e-9);
        }
        // центр грани ближе к своей грани, чем к чужой: face_of(центр) = сама
        for (i, &c) in centers.iter().enumerate() {
            assert_eq!(face_of_direction(c).unwrap(), i, "грань {i}");
        }
        // 20 направлений дают 20 разных граней (полное покрытие сферы)
        let mut seen = std::collections::HashSet::new();
        for &c in centers.iter() {
            seen.insert(face_of_direction(c).unwrap());
        }
        assert_eq!(seen.len(), 20);
    }
}
