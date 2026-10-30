//! 27-дерево — тритное пространственное индексирование XYZ (Сессия-14).
//!
//! Порт якоря `rstar.poler` (STR bulk-load, сплит-эвристики Greene/Quadtree/
//! R-star) на тритное ядро — вместо бинарного октодерева тройное деление
//! по каждой оси:
//!
//! ```text
//!     октодерево: 2×2×2 = 8 детей    (биты:  dx, dy, dz)
//!     27-дерево:  3×3×3 = 27 детей   (триты: dx, dy, dz ∈ {−1, 0, +1})
//! ```
//!
//! Путь к ячейке — ЧИСТАЯ ТРИТНАЯ ЗАПИСЬ: уровень = 3 трита (по одному
//! на ось), глубина d уровней = 3d тритов. Это квантованные трит-
//! координаты объекта на последовательных масштабах 3⁻¹ — пространственный
//! аналог тритной глубины вероятностей (logprob: P = 3^(−d)).
//!
//! Инвариант корректности: объект хранится в узле, чья ячейка его
//! содержит; объект, пересекающий трети, остаётся в узле (как в R-дереве).
//! Запрос режет ветви, чья ячейка не пересекает окно запроса.
//! Вставка/запрос — O(log₂₇ N) в среднем, память — O(N).

use crate::calc::trits::Trit;

/// Вместимость узла до сплита.
const CAP: usize = 8;

/// 3D-конверт в координатах тритной решётки (целые, один масштаб k).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BBox3 {
    pub minx: i64,
    pub miny: i64,
    pub minz: i64,
    pub maxx: i64,
    pub maxy: i64,
    pub maxz: i64,
}

impl BBox3 {
    pub fn new(minx: i64, miny: i64, minz: i64, maxx: i64, maxy: i64, maxz: i64) -> Self {
        BBox3 {
            minx: minx.min(maxx),
            miny: miny.min(maxy),
            minz: minz.min(maxz),
            maxx: maxx.max(minx),
            maxy: maxy.max(miny),
            maxz: maxz.max(minz),
        }
    }

    /// Непустое пересечение (касание считается пересечением).
    pub fn intersects(&self, o: &BBox3) -> bool {
        self.minx <= o.maxx
            && self.maxx >= o.minx
            && self.miny <= o.maxy
            && self.maxy >= o.miny
            && self.minz <= o.maxz
            && self.maxz >= o.minz
    }

    /// Полное вложение self ⊆ o.
    fn inside(&self, o: &BBox3) -> bool {
        self.minx >= o.minx
            && self.maxx <= o.maxx
            && self.miny >= o.miny
            && self.maxy <= o.maxy
            && self.minz >= o.minz
            && self.maxz <= o.maxz
    }

    /// Центры по осям (полусумма, без переполнения: координаты ≤ ~1.7e9).
    fn cx(&self) -> i64 {
        self.minx + (self.maxx - self.minx) / 2
    }
    fn cy(&self) -> i64 {
        self.miny + (self.maxy - self.miny) / 2
    }
    fn cz(&self) -> i64 {
        self.minz + (self.maxz - self.minz) / 2
    }
}

/// Узел 27-дерева.
#[derive(Clone, Debug)]
struct Node {
    /// Ячейка узла (включая границы).
    cell: BBox3,
    /// Объекты, не влезшие ни в одного ребёнка (пересекают трети) —
    /// лежат гарантированно внутри ячейки узла.
    items: Vec<(BBox3, usize)>,
    /// 27 детей (3×3×3): индекс = (dx+1) + 3·(dy+1) + 9·(dz+1).
    children: [Option<Box<Node>>; 27],
}

impl Node {
    fn new(cell: BBox3) -> Self {
        Node {
            cell,
            items: Vec::new(),
            children: Default::default(),
        }
    }

    /// Трети ячейки по оси: (lo, mid, hi) диапазоны.
    /// Ось делима, если в ней ≥ 3 узлов решётки.
    fn thirds(v0: i64, v1: i64) -> [(i64, i64); 3] {
        let span = v1 - v0;
        if span < 2 {
            // неделимая ось: единственная «средняя» треть, боковые пусты
            return [(v0, v1), (v1 + 1, v1), (v1 + 1, v1)];
        }
        let m1 = v0 + span / 3;
        let m2 = v0 + 2 * span / 3;
        [(v0, m1), (m1 + 1, m2), (m2 + 1, v1)]
    }

    /// Трит-цифра по позиции центра объекта относительно трети оси.
    fn digit(center: i64, thirds: [(i64, i64); 3]) -> Trit {
        let (lo, mid, hi) = (thirds[0], thirds[1], thirds[2]);
        if center <= lo.1 && lo.0 <= lo.1 {
            -1
        } else if center >= hi.0 && hi.0 <= hi.1 {
            1
        } else {
            0
        }
    }

    /// Ячейка ребёнка по трит-тройке (dx, dy, dz).
    fn child_cell(&self, dx: Trit, dy: Trit, dz: Trit) -> BBox3 {
        let tx = Node::thirds(self.cell.minx, self.cell.maxx);
        let ty = Node::thirds(self.cell.miny, self.cell.maxy);
        let tz = Node::thirds(self.cell.minz, self.cell.maxz);
        let pick = |t: Trit, th: [(i64, i64); 3]| match t {
            -1 => th[0],
            1 => th[2],
            _ => th[1],
        };
        let (x0, x1) = pick(dx, tx);
        let (y0, y1) = pick(dy, ty);
        let (z0, z1) = pick(dz, tz);
        BBox3::new(x0, y0, z0, x1, y1, z1)
    }

    /// Сплит: 27 детей; объекты уходят вниз, если целиком в ребёнке.
    fn split(&mut self, depth: u8, max_depth: u8, nodes: &mut usize) {
        if depth >= max_depth || !self.splittable() {
            return; // тупиковый узел: объекты копятся (корректность не страдает)
        }
        let items = std::mem::take(&mut self.items);
        for (bbox, id) in items {
            let dx = Node::digit(bbox.cx(), Node::thirds(self.cell.minx, self.cell.maxx));
            let dy = Node::digit(bbox.cy(), Node::thirds(self.cell.miny, self.cell.maxy));
            let dz = Node::digit(bbox.cz(), Node::thirds(self.cell.minz, self.cell.maxz));
            let child_cell = self.child_cell(dx, dy, dz);
            if bbox.inside(&child_cell) {
                let idx = child_index(dx, dy, dz);
                if self.children[idx].is_none() {
                    self.children[idx] = Some(Box::new(Node::new(child_cell)));
                    *nodes += 1;
                }
                let child = self.children[idx].as_mut().unwrap();
                child.insert(bbox, id, depth + 1, max_depth, nodes);
            } else {
                self.items.push((bbox, id)); // пересекает трети — остаётся
            }
        }
    }

    /// Есть ли хотя бы одна делимая ось (≥ 3 узлов по оси).
    fn splittable(&self) -> bool {
        self.cell.maxx - self.cell.minx >= 2
            || self.cell.maxy - self.cell.miny >= 2
            || self.cell.maxz - self.cell.minz >= 2
    }

    /// Вставка с рекурсивным сплитом. Возвращает трит-путь (3 трита/уровень).
    fn insert(&mut self, bbox: BBox3, id: usize, depth: u8, max_depth: u8, nodes: &mut usize) -> Vec<Trit> {
        if !bbox.inside(&self.cell) {
            return vec![]; // вне ячейки — не сюда (вызывает insert дерева)
        }
        // предел глубины: объект остаётся здесь (трит-путь ≤ 3·max_depth)
        if depth >= max_depth {
            self.items.push((bbox, id));
            return vec![];
        }
        let has_children = self.children.iter().any(|c| c.is_some());
        if !has_children && self.items.len() < CAP {
            self.items.push((bbox, id));
            return vec![];
        }
        // узел переполнен или уже разделён — вниз (если влезаем в ребёнка)
        let dx = Node::digit(bbox.cx(), Node::thirds(self.cell.minx, self.cell.maxx));
        let dy = Node::digit(bbox.cy(), Node::thirds(self.cell.miny, self.cell.maxy));
        let dz = Node::digit(bbox.cz(), Node::thirds(self.cell.minz, self.cell.maxz));
        let child_cell = self.child_cell(dx, dy, dz);
        let idx = child_index(dx, dy, dz);
        let fits = bbox.inside(&child_cell) && (has_children || self.items.len() >= CAP);
        if !fits {
            self.items.push((bbox, id)); // пересекает трети — остаётся в узле
            return vec![];
        }
        // первый перелив: сплит раздаёт старые объекты по детям
        if !has_children {
            self.split(depth, max_depth, nodes);
        }
        if self.children[idx].is_none() {
            self.children[idx] = Some(Box::new(Node::new(child_cell)));
            *nodes += 1;
        }
        let mut path = vec![dx, dy, dz];
        let child = self.children[idx].as_mut().unwrap();
        path.extend(child.insert(bbox, id, depth + 1, max_depth, nodes));
        path
    }

    /// Запрос окна: все id, чьи конверты пересекают q (с отсечением ветвей).
    fn query(&self, q: &BBox3, out: &mut Vec<usize>) {
        if !self.cell.intersects(q) {
            return;
        }
        for (bbox, id) in &self.items {
            if bbox.intersects(q) {
                out.push(*id);
            }
        }
        for child in self.children.iter().flatten() {
            child.query(q, out);
        }
    }

    /// Высота поддерева (узлы-листья = 1).
    fn height(&self) -> u8 {
        1 + self
            .children
            .iter()
            .flatten()
            .map(|c| c.height())
            .max()
            .unwrap_or(0)
    }
}

/// Индекс ребёнка из трит-тройки: (dx+1) + 3·(dy+1) + 9·(dz+1) ∈ [0, 27).
fn child_index(dx: Trit, dy: Trit, dz: Trit) -> usize {
    ((dx + 1) as usize) + 3 * ((dy + 1) as usize) + 9 * ((dz + 1) as usize)
}

/// 27-дерево над тритной решёткой: тритное деление 3×3×3 вместо октодерева.
/// `Clone`/`Debug` — TIN хранит дерево как пространственный индекс (Сессия-16).
#[derive(Clone, Debug)]
pub struct Tree27 {
    root: Node,
    /// Число объектов.
    count: usize,
    /// Число узлов (включая корень).
    nodes: usize,
    max_depth: u8,
}

impl Tree27 {
    /// Дерево с корневой ячейкой `bounds` (в координатах решётки).
    /// `max_depth` ≤ 8 → трит-путь ≤ 24 тритов.
    pub fn new(bounds: BBox3, max_depth: u8) -> Self {
        Tree27 {
            root: Node::new(bounds),
            count: 0,
            nodes: 1,
            max_depth,
        }
    }

    /// Вставить объект (конверт + id). Возвращает трит-путь спуска
    /// (3 трита на уровень: dx, dy, dz) — пространственный трит-адрес.
    pub fn insert(&mut self, bbox: BBox3, id: usize) -> Result<Vec<Trit>, String> {
        if !bbox.inside(&self.root.cell) {
            return Err(format!(
                "tree27: объект {:?} вне корневой ячейки {:?} — расширьте bounds",
                (bbox.minx, bbox.miny, bbox.minz, bbox.maxx, bbox.maxy, bbox.maxz),
                (self.root.cell.minx, self.root.cell.miny, self.root.cell.minz,
                 self.root.cell.maxx, self.root.cell.maxy, self.root.cell.maxz)
            ));
        }
        let path = self.root.insert(bbox, id, 0, self.max_depth, &mut self.nodes);
        self.count += 1;
        Ok(path)
    }

    /// Запрос окна: id всех объектов, чьи конверты пересекают q.
    pub fn query(&self, q: &BBox3) -> Vec<usize> {
        let mut out = Vec::new();
        self.root.query(q, &mut out);
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Точечный запрос (вырожденное окно).
    pub fn query_point(&self, x: i64, y: i64, z: i64) -> Vec<usize> {
        self.query(&BBox3::new(x, y, z, x, y, z))
    }

    /// Статистика: (объектов, узлов, высота).
    pub fn stats(&self) -> (usize, usize, u8) {
        (self.count, self.nodes, self.root.height())
    }

    /// Трит-путь как сбалансированно-троичная строка «+0−·…»
    /// (уровень = 3 символа: dx dy dz).
    pub fn path_string(path: &[Trit]) -> String {
        let mut s = String::new();
        for (i, &t) in path.iter().enumerate() {
            if i > 0 && i % 3 == 0 {
                s.push('·');
            }
            s.push(match t {
                -1 => '−',
                0 => '0',
                _ => '+',
            });
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Детерминированный LCG (движок без RNG — биты воспроизводимы).
    fn lcg(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *state >> 33
    }

    fn boxes(n: usize, seed: u64, span: i64) -> Vec<BBox3> {
        let mut st = seed;
        (0..n)
            .map(|_| {
                let x = (lcg(&mut st) as i64 % (2 * span)) - span;
                let y = (lcg(&mut st) as i64 % (2 * span)) - span;
                let z = (lcg(&mut st) as i64 % (2 * span)) - span;
                let w = lcg(&mut st) as i64 % 8;
                BBox3::new(x, y, z, x + w, y + w, z + w)
            })
            .collect()
    }

    #[test]
    fn matches_brute_force_on_random_windows() {
        for seed in [7u64, 42, 2025] {
            let items = boxes(400, seed, 500);
            let bounds = BBox3::new(-520, -520, -520, 540, 540, 540);
            let mut tree = Tree27::new(bounds, 8);
            for (i, b) in items.iter().enumerate() {
                tree.insert(*b, i).unwrap();
            }
            // случайные окна запросов — против лобового перебора
            let mut st = seed.wrapping_mul(3);
            for _ in 0..60 {
                let x0 = (lcg(&mut st) as i64 % 1000) - 500;
                let y0 = (lcg(&mut st) as i64 % 1000) - 500;
                let z0 = (lcg(&mut st) as i64 % 1000) - 500;
                let w = 40 + lcg(&mut st) as i64 % 200;
                let q = BBox3::new(x0, y0, z0, x0 + w, y0 + w, z0 + w);
                let expected: Vec<usize> = items
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| b.intersects(&q))
                    .map(|(i, _)| i)
                    .collect();
                assert_eq!(tree.query(&q), expected, "seed={seed}, q={q:?}");
            }
        }
    }

    #[test]
    fn point_queries_and_stats() {
        let items = boxes(300, 99, 200);
        let mut tree = Tree27::new(BBox3::new(-210, -210, -210, 215, 215, 215), 6);
        for (i, b) in items.iter().enumerate() {
            tree.insert(*b, i).unwrap();
        }
        // точечный запрос = вырожденное окно
        for b in items.iter().take(40) {
            let p = (b.minx, b.miny, b.minz);
            let direct = tree.query(&BBox3::new(p.0, p.1, p.2, p.0, p.1, p.2));
            assert_eq!(tree.query_point(p.0, p.1, p.2), direct);
            assert!(direct.contains(&items.iter().position(|o| o == b).unwrap()));
        }
        let (count, nodes, height) = tree.stats();
        assert_eq!(count, 300);
        assert!(nodes > 1, "дерево должно ветвиться: {nodes} узлов");
        assert!(height <= 7, "высота {height} > max_depth+1");
    }

    #[test]
    fn trit_path_is_balanced_ternary() {
        let mut tree = Tree27::new(BBox3::new(-1000, -1000, -1000, 1000, 1000, 1000), 8);
        // наполняем до перелива (CAP=8) — корень сплитится на 27
        for i in 0..9 {
            let c = (i as i64 - 4) * 10;
            tree.insert(BBox3::new(c, c, c, c + 1, c + 1, c + 1), i as usize)
                .unwrap();
        }
        // объект строго в ++- секторе: центр (900, 900, −900) → dx=+, dy=+, dz=−
        let path = tree
            .insert(BBox3::new(900, 900, -905, 905, 905, -900), 100)
            .unwrap();
        assert!(!path.is_empty(), "после сплита объект уходит вниз");
        assert!(path.len() % 3 == 0, "путь кратен 3 тритам: {path:?}");
        assert!(path.len() <= 24, "путь ≤ 3·max_depth: {path:?}");
        assert_eq!(&path[..3], &[1, 1, -1], "первый уровень: ++−");
        assert!(path.iter().all(|&t| t.abs() <= 1));
        let s = Tree27::path_string(&path);
        assert!(s.starts_with("++−"), "строка пути: {s}");
        if path.len() > 3 {
            assert!(s.contains('·'), "разделитель уровней: {s}");
        }
        // кластер в одном секторе гонит путь глубже: 9 объектов в ++-
        let mut deep = Tree27::new(BBox3::new(-1000, -1000, -1000, 1000, 1000, 1000), 8);
        for i in 0..9usize {
            let c = 900 + (i as i64 % 3) * 3;
            let z = -900 - (i as i64 / 3) * 3;
            deep.insert(BBox3::new(c, c, z, c + 1, c + 1, z + 1), i).unwrap();
        }
        let last = deep.insert(BBox3::new(901, 901, -901, 902, 902, -900), 100).unwrap();
        assert!(last.len() >= 6, "кластер углубляет путь: {:?}", last);
        assert!(Tree27::path_string(&last).contains('·'));
    }

    #[test]
    fn out_of_bounds_rejected_and_straddlers_kept() {
        let mut tree = Tree27::new(BBox3::new(-10, -10, -10, 10, 10, 10), 4);
        // объект вне корня — ошибка
        assert!(tree.insert(BBox3::new(20, 0, 0, 30, 1, 1), 0).is_err());
        // объект во всю корневую ячейку — пересекает все трети, живёт в корне
        let path = tree.insert(BBox3::new(-10, -10, -10, 10, 10, 10), 1).unwrap();
        assert!(path.is_empty(), "страйдлер: путь пуст — объект в корне");
        // и находится любым запросом внутрь корня
        assert_eq!(tree.query_point(0, 0, 0), vec![1]);
    }

    #[test]
    fn degenerate_points_only() {
        // облако точек (нулевой объём) — классический кейс индекса
        let mut tree = Tree27::new(BBox3::new(0, 0, 0, 2187, 2187, 2187), 8); // 3^7
        for i in 0..200 {
            let (x, y, z) = ((i * 37) % 2187, (i * 91) % 2187, (i * 53) % 2187);
            tree.insert(BBox3::new(x, y, z, x, y, z), i as usize).unwrap();
        }
        let hit = tree.query(&BBox3::new(0, 0, 0, 100, 100, 100));
        let expected = (0..200)
            .filter(|&i| (i * 37) % 2187 <= 100 && (i * 91) % 2187 <= 100 && (i * 53) % 2187 <= 100)
            .collect::<Vec<_>>();
        assert_eq!(hit, expected);
    }
}
