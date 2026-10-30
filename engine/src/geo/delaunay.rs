//! TIN Делоне на тритной решётке — Bowyer-Watson + Муха (Сессия-15/16).
//!
//! Переплетение модулей (доктрина Сессии-14 продолжается):
//!
//! * **точность** — координаты узлов целые (решётка 3⁻ᵏ), in-circle тест
//!   считается **i128-детерминантом** — ни одного округления, как
//!   `cross_sign`, только степенью выше. Строгий знак `> 0` = «точка
//!   внутри описанной окружности»: сокруглённость (детерминант 0) —
//!   НЕ плохой треугольник, оба варианта триангуляции легальны;
//! * **Муха** (из `universal_letters`/`flypulse`) — порядок вставки точек
//!   задаёт **золотая фаза** φ(i) = i·Φ mod 2π (Gold Phase Lock):
//!   детерминированная перестановка без состояния и RNG. Ротор Мухи в
//!   `flypulse` не даёт динамике застыть в циклах — здесь та же работа:
//!   вырожденные развилки (сокруглённые четвёрки) муха развязывает
//!   порядком вставки, а отсортированный вход (худший случай O(n²)
//!   Бойера—Ватсона) расщёлкивается как орешек — декорреляция фазой;
//! * **переплетение** — `eteria::tin_boltz` сеет вершины золотой
//!   спиралью (та же Φ!), строит TIN, считает больцмановские энергии
//!   через LogProb за пределами f64;
//! * **27-дерево** (Сессия-16) — `Tin::build_spatial27` индексирует
//!   конверты треугольников тритным деревом 3×3×3: `height_at` идёт по
//!   трит-пути вместо лобового перебора.
//!
//! ## Ускорение Сессии-16: O(n²) → O(n log n) в среднем
//!
//! Лобовой Бойер—Ватсон на каждой вставке сканирует ВСЕ треугольники
//! (и вдобавок пересобирает вектор — O(T) на шаг). Триада ускорения:
//!
//! 1. **History-DAG** (Гибас—Кемани—Сугихара): мёртвые треугольники НЕ
//!    удаляются, а хранят `kids` — потомков, покрывших их территорию.
//!    Локализация точки — спуск от супер-треугольника по детям: точка
//!    всегда в одном из них (веер полости покрывает её). Глубина —
//!    O(log n) в среднем при псевдослучайном (мушином!) порядке вставки;
//!    никаких переворотов структуры — только append.
//! 2. **Смежность + BFS-полость**: сосед через ребро (a→b) живёт в
//!    `adj[(b→a)]` (направленные CCW-рёбра, обновление O(полость)).
//!    Полость расширяется BFS от СОДЕРЖАЩЕГО треугольника — он всегда
//!    плохой: точка внутри треугольника лежит и внутри его диска
//!    (диск выпуклый и содержит треугольник; точка не вершина — дедуп).
//!    Полость звёздна относительно p ⇒ связна (Бойер/Ватсон; так же
//!    ищет конфликтную зону CGAL) — BFS находит её целиком.
//! 3. **Хирургия за O(полость)**: смерть — флаг `alive = false` +
//!    kids по bbox-фильтру (надмножество геометрического покрытия —
//!    локализация ничего не теряет), рёбра adj удаляются/вставляются
//!    только у участников. Вектор НЕ пересобирается.
//!
//! `delaunay_bruteforce` (лобовой) сохранён как эталон: тесты сверяют
//! обе машины побитово-канонизированно на сетках (сады сокруглённостей),
//! случайных облаках и adversarial-порядках.
//!
//! Терминология Bowyer-Watson: «плохой» треугольник — вставляемая точка
//! строго внутри его описанной окружности. Полость = связная компонента
//! плохих треугольников; граница полости сшивается в циклы (направленные
//! рёбра плохих CCW-треугольников, внутренние рёбра гасятся по смежности),
//! точка веером триангулирует каждый цикл. Алгоритм **не имеет
//! переворотов и потому не может циклиться в принципе** — муха нужна
//! против вырожденных развилок и adversarial-порядков, не против зацикливания.
//!
//! Границы точности (грабля №30, поймана до реализации): координаты
//! решётки k ≤ 14 → |x| ≤ (3¹⁴−1)/2 ≈ 2.39·10⁶; супер-треугольник до
//! ~7.2·10⁶; разности ≤ 1.5·10⁷; члены детерминанта ≤ ~10³⁰ ≪ 1.7·10³⁸
//! (i128) — запас восемь порядков. k > 18 переполнил бы i128 — поэтому
//! [`MAX_LATTICE_K`] = 14.

use std::collections::{HashMap, HashSet, VecDeque};
use std::f64::consts::TAU;

/// Золотое сечение — импорт из первоисточника (universal_letters).
/// Муха летает по той же константе, что и буквы.
use crate::universal_letters::PHI;

use crate::geo::tree27::{BBox3, Tree27};

/// Максимальный k решётки для TIN (анализ переполнения i128 — модульные доки).
pub const MAX_LATTICE_K: u8 = 14;

/// TIN: вершины (целые координаты решётки 3⁻ᵏ) + треугольники CCW.
/// Опционально несёт 27-дерево конвертов треугольников (Сессия-16):
/// строится `build_spatial27`, ускоряет `height_at`.
#[derive(Clone, Debug)]
pub struct Tin {
    pub pts: Vec<(i64, i64)>,
    pub tris: Vec<[usize; 3]>,
    index: Option<Tree27>,
}

/// Ориентация тройки (i64): > 0 — CCW, < 0 — CW, 0 — коллинеарны.
#[inline]
fn orient2(ax: i64, ay: i64, bx: i64, by: i64, cx: i64, cy: i64) -> i64 {
    (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
}

/// Точный in-circle: знак детерминанта 3×3 (i128, ни одного округления).
/// `> 0` ⇔ p строго внутри описанной окружности (a, b, c CCW);
/// `= 0` ⇔ сокруглённость (муха развяжет порядком вставки);
/// `< 0` ⇔ снаружи.
#[inline]
fn incircle_det(
    ax: i64, ay: i64, bx: i64, by: i64, cx: i64, cy: i64, px: i64, py: i64,
) -> i128 {
    let adx = (ax - px) as i128;
    let ady = (ay - py) as i128;
    let bdx = (bx - px) as i128;
    let bdy = (by - py) as i128;
    let cdx = (cx - px) as i128;
    let cdy = (cy - py) as i128;
    let alift = adx * adx + ady * ady;
    let blift = bdx * bdx + bdy * bdy;
    let clift = cdx * cdx + cdy * cdy;
    // форма Шевчука (Lecture Notes on Geometric Robustness):
    // adx·(bdy·clift − cdy·blift) − ady·(bdx·clift − cdx·blift)
    // + alift·(bdx·cdy − cdx·bdy)  [> 0 ⇔ внутри, при (a,b,c) CCW]
    adx * (bdy * clift - cdy * blift)
        - ady * (bdx * clift - cdx * blift)
        + alift * (bdx * cdy - cdx * bdy)
}

/// Муха: фаза индекса точки — Gold Phase Lock из universal_letters.
#[inline]
pub fn fly_phase(i: usize) -> f64 {
    ((i as f64) * PHI) % TAU
}

/// Мушиный порядок вставки: индексы по возрастанию sin(φ(i)).
/// Детерминизм без состояния: перестановка — чистая функция от n.
pub fn fly_order(n: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| {
        fly_phase(a)
            .sin()
            .total_cmp(&fly_phase(b).sin())
            .then(a.cmp(&b))
    });
    idx
}

/// Узел истории триангуляции (DAG Гибаса—Кемани—Сугихары): мёртвый узел
/// хранит детей — треугольники, покрывшие его территорию. Локализация —
/// спуск от корня; никаких удалений из вектора — только append и флаги.
#[derive(Clone, Debug)]
struct TriNode {
    /// Вершины CCW (0..2 — супер-треугольник, 3.. — точки входа).
    v: [usize; 3],
    alive: bool,
    /// Потомки мёртвого узла: fan-треугольники, чьи КОНВЕРТЫ пересекают
    /// его конверт (bbox-фильтр — надмножество геометрического покрытия,
    /// спуск локализации ничего не теряет).
    kids: Vec<usize>,
}

/// Точка в замкнутом треугольнике (CCW, ориентации ≥ 0; ребро/вершина
/// считаются внутри — детям DAG разрешено накрывать точку краем).
#[inline]
fn tri_contains(pts: &[(i64, i64)], v: &[usize; 3], px: i64, py: i64) -> bool {
    let (ax, ay) = pts[v[0]];
    let (bx, by) = pts[v[1]];
    let (cx, cy) = pts[v[2]];
    orient2(ax, ay, bx, by, px, py) >= 0
        && orient2(bx, by, cx, cy, px, py) >= 0
        && orient2(cx, cy, ax, ay, px, py) >= 0
}

/// Конверт треугольника (minx, maxx, miny, maxy).
#[inline]
fn tri_bbox(pts: &[(i64, i64)], v: &[usize; 3]) -> (i64, i64, i64, i64) {
    let (ax, ay) = pts[v[0]];
    let (bx, by) = pts[v[1]];
    let (cx, cy) = pts[v[2]];
    (
        ax.min(bx).min(cx),
        ax.max(bx).max(cx),
        ay.min(by).min(cy),
        ay.max(by).max(cy),
    )
}

impl Tin {
    /// Пустой TIN (для тестов viz-слоя и валидации ошибок).
    pub fn empty() -> Tin {
        Tin {
            pts: Vec::new(),
            tris: Vec::new(),
            index: None,
        }
    }

    /// Число рёбер триангуляции (для формулы Эйлера: T = 2n − 2 − h).
    pub fn edge_count(&self) -> usize {
        let mut und: HashMap<(usize, usize), u32> = HashMap::new();
        for &[a, b, c] in &self.tris {
            for &(u, v) in &[(a, b), (b, c), (c, a)] {
                let key = if u < v { (u, v) } else { (v, u) };
                *und.entry(key).or_insert(0) += 1;
            }
        }
        und.len()
    }

    /// Рёбра выпуклой оболочки (принадлежат ровно одному треугольнику).
    pub fn hull(&self) -> Vec<[usize; 2]> {
        let mut cnt: HashMap<(usize, usize), u32> = HashMap::new();
        for &[a, b, c] in &self.tris {
            for &(u, v) in &[(a, b), (b, c), (c, a)] {
                let key = if u < v { (u, v) } else { (v, u) };
                *cnt.entry(key).or_insert(0) += 1;
            }
        }
        let mut hull: Vec<[usize; 2]> = cnt
            .into_iter()
            .filter(|(_, c)| *c == 1)
            .map(|((u, v), _)| [u, v])
            .collect();
        hull.sort();
        hull
    }

    /// Построить 27-дерево конвертов треугольников (Сессия-16):
    /// каждый треугольник — объект с конвертом (z = 0), id = номер в
    /// `tris`. Возвращает (объектов, узлов дерева). Повторный вызов
    /// перестраивает индекс заново.
    pub fn build_spatial27(&mut self) -> Result<(usize, usize), String> {
        if self.tris.is_empty() {
            return Err("tin: нечего индексировать — триангуляция пуста".into());
        }
        let (mut x0, mut x1, mut y0, mut y1) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
        for &(x, y) in &self.pts {
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        // невырожденность: коллинеарный вход отброшен delaunay(); clами
        // защищаем микро-TIN от вырожденного корня
        let bounds = BBox3::new(
            x0.min(-1),
            y0.min(-1),
            0,
            x1.max(1),
            y1.max(1),
            0,
        );
        let mut tree = Tree27::new(bounds, 8);
        for (ti, t) in self.tris.iter().enumerate() {
            let (bx0, bx1, by0, by1) = tri_bbox(&self.pts, t);
            tree.insert(BBox3::new(bx0, by0, 0, bx1, by1, 0), ti)
                .map_err(|e| format!("tin: индекс 27-дерева: {e}"))?;
        }
        let (count, nodes, _) = tree.stats();
        self.index = Some(tree);
        Ok((count, nodes))
    }

    /// Высота TIN в узле решётки (барицентрическая интерполяция, f64 по
    /// вершинам z). None — вне выпуклой оболочки. Точка на ребре/вершине
    /// принадлежит первому найденному треугольнику (граница включена).
    ///
    /// С индексом 27-дерева ([`build_spatial27`]) кандидаты отбираются
    /// трит-путём — порядок перебора (по номеру треугольника) и результат
    /// идентичны лобовому пути: содержащий треугольник всегда среди
    /// конвертов, накрывающих точку.
    pub fn height_at(&self, z: &[f64], qx: i64, qy: i64) -> Option<f64> {
        let candidates: Vec<usize> = match &self.index {
            Some(tree) => tree.query_point(qx, qy, 0),
            None => (0..self.tris.len()).collect(),
        };
        for ti in candidates {
            let &[a, b, c] = &self.tris[ti];
            let (ax, ay) = self.pts[a];
            let (bx, by) = self.pts[b];
            let (cx, cy) = self.pts[c];
            let o1 = orient2(ax, ay, bx, by, qx, qy);
            let o2 = orient2(bx, by, cx, cy, qx, qy);
            let o3 = orient2(cx, cy, ax, ay, qx, qy);
            if o1 >= 0 && o2 >= 0 && o3 >= 0 {
                // барицентрические веса (площади подтреугольников)
                let w_a = o2 as f64;
                let w_b = o3 as f64;
                let w_c = o1 as f64;
                let total = w_a + w_b + w_c;
                if total == 0.0 {
                    continue; // вырожденный треугольник — следующий кандидат
                }
                return Some((w_a * z[a] + w_b * z[b] + w_c * z[c]) / total);
            }
        }
        None
    }
}

/// Делоне-триангуляция Бойера—Ватсона, УСКОРЕННАЯ (Сессия-16):
/// history-DAG локализации + BFS-полость по смежности + хирургия за
/// O(полость). Порядок вставки водит Муха (sin(i·Φ)).
///
/// Вход: целые координаты решётки (из [`crate::geo::trit_coord::TritPoint`]
/// через `to_i64`, k ≤ [`MAX_LATTICE_K`]). Выход: TIN с CCW-треугольниками.
///
/// Ошибки (честные): < 3 точек; все точки коллинеарны; зажим полости
/// (pinch — две границы в одной вершине, на решётчатых данных не встречается).
pub fn delaunay(points: &[(i64, i64)]) -> Result<Tin, String> {
    // 0. дедуп — муха не садится дважды на одну точку
    let mut seen = std::collections::HashSet::new();
    let mut pts: Vec<(i64, i64)> = Vec::with_capacity(points.len());
    for &p in points {
        if seen.insert(p) {
            pts.push(p);
        }
    }
    if pts.len() < 3 {
        return Err(format!(
            "tin: нужно ≥ 3 различных точек, получено {}",
            pts.len()
        ));
    }

    // 1. супер-треугольник: строго содержит bbox [−M, M]² (доказано
    //    ориентационными проверками всех четырёх углов)
    let m = pts.iter().map(|p| p.0.abs().max(p.1.abs())).max().unwrap().max(1);
    let super_v: [(i64, i64); 3] = [
        (-3 * m - 2, -2 * m - 1),
        (3 * m + 2, -2 * m - 1),
        (0, 3 * m + 2),
    ];
    // CCW-проверка супер-треугольника (инвариант всех треугольников)
    debug_assert!(
        orient2(
            super_v[0].0, super_v[0].1, super_v[1].0, super_v[1].1,
            super_v[2].0, super_v[2].1
        ) > 0
    );

    // внутренняя сетка: [суперA, суперB, суперC, ...точки]
    let mut all: Vec<(i64, i64)> = super_v.to_vec();
    all.extend(pts.iter().cloned());

    // история: узел 0 — супер-треугольник (корень DAG, всегда содержит
    // все точки). Живые и мёртвые узлы живут в одном векторе — только append.
    let mut tris: Vec<TriNode> = Vec::with_capacity(2 * pts.len() + 8);
    tris.push(TriNode {
        v: [0, 1, 2],
        alive: true,
        kids: Vec::new(),
    });
    // смежность живых: направленное ребро (a→b CCW) → треугольник;
    // сосед через ребро (a→b) = adj[(b→a)]. Хул-рёбер нет, пока жив супер.
    let mut adj: HashMap<(usize, usize), usize> = HashMap::new();
    for &(a, b) in &[(0usize, 1usize), (1, 2), (2, 0)] {
        adj.insert((a, b), 0);
    }

    // 2. вставка в мушином порядке (золотая фаза индекса ВО ВХОДЕ)
    let order = fly_order(pts.len());
    for &pi in &order {
        let p_idx = pi + 3; // смещение супер-треугольника
        let (px, py) = pts[pi];

        // (а) ЛОКАЛИЗАЦИЯ: спуск по истории от супер-треугольника.
        // Дети покрывают родителя (bbox-надмножество) — точка не теряется.
        let mut cur = 0usize;
        while !tris[cur].alive {
            let mut next: Option<usize> = None;
            for &kid in &tris[cur].kids {
                if tri_contains(&all, &tris[kid].v, px, py) {
                    next = Some(kid);
                    break;
                }
            }
            match next {
                Some(k) => cur = k,
                None => {
                    return Err(
                        "tin: локализация потеряла точку (внутренняя ошибка history-DAG)".into(),
                    )
                }
            }
        }
        // Содержащий треугольник ВСЕГДА плохой: точка внутри треугольника
        // лежит и внутри его описанного диска (диск выпуклый, треугольник —
        // выпуклая оболочка трёх точек окружности), строго — точка не
        // вершина (дедуп). Полость непуста, BFS есть с чего стартовать.

        // (б) ПОЛОСТЬ: BFS по смежности от содержащего, строгий in-circle
        // (i128). Полость звёздна относительно p ⇒ связна — BFS целиком.
        let mut cavity: Vec<usize> = Vec::new();
        let mut in_cavity: HashSet<usize> = HashSet::new();
        let mut queue: VecDeque<usize> = VecDeque::new();
        in_cavity.insert(cur);
        cavity.push(cur);
        queue.push_back(cur);
        while let Some(ti) = queue.pop_front() {
            let [a, b, c] = tris[ti].v;
            for &(u, w) in &[(a, b), (b, c), (c, a)] {
                if let Some(&nb) = adj.get(&(w, u)) {
                    if !in_cavity.contains(&nb) {
                        let [x, y, z] = tris[nb].v;
                        let (ax, ay) = all[x];
                        let (bx, by) = all[y];
                        let (cx, cy) = all[z];
                        if incircle_det(ax, ay, bx, by, cx, cy, px, py) > 0 {
                            in_cavity.insert(nb);
                            cavity.push(nb);
                            queue.push_back(nb);
                        }
                    }
                }
            }
        }

        // (в) граница полости: направленные CCW-рёбра плохих, чей сосед
        // (обратное ребро) не плох. СОРТИРОВКА — доктрина детерминизма
        // (порядок рёбер полости воспроизводим побитово).
        let mut boundary: Vec<(usize, usize)> = Vec::with_capacity(cavity.len() * 3);
        for &ti in &cavity {
            let [a, b, c] = tris[ti].v;
            for &(u, w) in &[(a, b), (b, c), (c, a)] {
                match adj.get(&(w, u)) {
                    Some(&nb) if in_cavity.contains(&nb) => {} // внутреннее — гасится
                    _ => boundary.push((u, w)),
                }
            }
        }
        boundary.sort_unstable();

        // сшивка границы в циклы: у каждой вершины ровно одно исходящее
        let mut next: HashMap<usize, usize> = HashMap::with_capacity(boundary.len());
        for &(a, b) in &boundary {
            if next.insert(a, b).is_some() {
                return Err(format!(
                    "tin: зажим полости в вершине {a} — муха отказывается сшивать"
                ));
            }
        }

        // (д) веер точки по каждому циклу
        let fan_base = tris.len();
        let mut fans: Vec<[usize; 3]> = Vec::with_capacity(boundary.len());
        let mut used: HashMap<usize, usize> = next.clone(); // потребляем рёбра
        for &(start, _) in &boundary {
            if used.get(&start).is_none() {
                continue; // цикл уже закрыт с другой вершины
            }
            let mut c = start;
            loop {
                let nxt = match used.remove(&c) {
                    Some(n) => n,
                    None => break, // цикл замкнулся
                };
                // CCW-инвариант: граница полости направлена так, что
                // полость слева ⇒ (грань, след, точка) — CCW
                fans.push([c, nxt, p_idx]);
                c = nxt;
            }
        }

        // (е) хирургия за O(полость): смерть — флаг + kids (bbox-фильтр),
        // рёбра adj удаляются только у умирающих, fan вставляется append'ом.
        // Fan-ключи свободны: (грань→след) — ребро умершего, (след→p) и
        // (p→грань) — p свежая вершина, рёбер с ней ещё нет.
        for &ti in &cavity {
            let [a, b, c] = tris[ti].v;
            for &(u, w) in &[(a, b), (b, c), (c, a)] {
                adj.remove(&(u, w));
            }
            tris[ti].alive = false;
        }
        for &ti in &cavity {
            let (x0, x1, y0, y1) = tri_bbox(&all, &tris[ti].v);
            tris[ti].kids = (fan_base..fan_base + fans.len())
                .filter(|&f| {
                    let (fx0, fx1, fy0, fy1) = tri_bbox(&all, &fans[f - fan_base]);
                    fx0 <= x1 && fx1 >= x0 && fy0 <= y1 && fy1 >= y0
                })
                .collect();
        }
        for (i, fan) in fans.into_iter().enumerate() {
            let id = fan_base + i;
            tris.push(TriNode {
                v: fan,
                alive: true,
                kids: Vec::new(),
            });
            for &(u, w) in &[(fan[0], fan[1]), (fan[1], fan[2]), (fan[2], fan[0])] {
                adj.insert((u, w), id);
            }
        }
    }

    // 3. живые без супер-вершин, в порядке создания (детерминизм),
    //    перенумерация: −3
    let mut out: Vec<[usize; 3]> = Vec::new();
    for t in &tris {
        if t.alive && t.v[0] > 2 && t.v[1] > 2 && t.v[2] > 2 {
            out.push([t.v[0] - 3, t.v[1] - 3, t.v[2] - 3]);
        }
    }
    if out.is_empty() {
        return Err("tin: все точки коллинеарны — TIN вырожден".into());
    }
    Ok(Tin {
        pts,
        tris: out,
        index: None,
    })
}

/// Лобовой Бойер—Ватсон (эталон Сессии-15): на каждой вставке — полный
/// скан всех живых треугольников и пересборка вектора, O(n²). Оставлен
/// для аудита: тесты сверяют его с ускоренной [`delaunay`] канонизированно
/// (множество треугольников обязано совпадать побитово).
pub fn delaunay_bruteforce(points: &[(i64, i64)]) -> Result<Tin, String> {
    // 0. дедуп — муха не садится дважды на одну точку
    let mut seen = std::collections::HashSet::new();
    let mut pts: Vec<(i64, i64)> = Vec::with_capacity(points.len());
    for &p in points {
        if seen.insert(p) {
            pts.push(p);
        }
    }
    if pts.len() < 3 {
        return Err(format!(
            "tin: нужно ≥ 3 различных точек, получено {}",
            pts.len()
        ));
    }

    // 1. супер-треугольник: строго содержит bbox [−M, M]²
    let m = pts.iter().map(|p| p.0.abs().max(p.1.abs())).max().unwrap().max(1);
    let super_v: [(i64, i64); 3] = [
        (-3 * m - 2, -2 * m - 1),
        (3 * m + 2, -2 * m - 1),
        (0, 3 * m + 2),
    ];
    debug_assert!(
        orient2(
            super_v[0].0, super_v[0].1, super_v[1].0, super_v[1].1,
            super_v[2].0, super_v[2].1
        ) > 0
    );

    let mut all: Vec<(i64, i64)> = super_v.to_vec();
    all.extend(pts.iter().cloned());
    let mut tris: Vec<[usize; 3]> = vec![[0, 1, 2]];

    // 2. вставка в мушином порядке
    let order = fly_order(pts.len());
    for &pi in &order {
        let p_idx = pi + 3;
        let (px, py) = pts[pi];

        // плохие треугольники: точка СТРОГО внутри описанной окружности
        let mut bad: Vec<usize> = Vec::new();
        for (ti, t) in tris.iter().enumerate() {
            let (ax, ay) = all[t[0]];
            let (bx, by) = all[t[1]];
            let (cx, cy) = all[t[2]];
            if incircle_det(ax, ay, bx, by, cx, cy, px, py) > 0 {
                bad.push(ti);
            }
        }
        if bad.is_empty() {
            return Err("tin: точка вне триангуляции (внутренняя ошибка супер-треугольника)".into());
        }

        // направленные рёбра плохих CCW-треугольников; внутренние гасятся
        let mut dir_cnt: HashMap<(usize, usize), u32> = HashMap::new();
        for &ti in &bad {
            let [a, b, c] = tris[ti];
            for &e in &[(a, b), (b, c), (c, a)] {
                *dir_cnt.entry(e).or_insert(0) += 1;
            }
        }
        // граница: ребро (a→b), чей обратный (b→a) не встречается.
        // СОРТИРОВКА: доктрина детерминизма — порядок рёбер полости
        // воспроизводим побитово (HashMap-итерация рандомизирована).
        let mut boundary: Vec<(usize, usize)> = dir_cnt
            .iter()
            .filter(|(&(a, b), _)| !dir_cnt.contains_key(&(b, a)))
            .map(|(&(a, b), _)| (a, b))
            .collect();
        boundary.sort_unstable();

        // сшивка границы в циклы
        let mut next: HashMap<usize, usize> = HashMap::with_capacity(boundary.len());
        for &(a, b) in &boundary {
            if next.insert(a, b).is_some() {
                return Err(format!(
                    "tin: зажим полости в вершине {a} — муха отказывается сшивать"
                ));
            }
        }

        // веер точки по каждому циклу
        let mut fans: Vec<[usize; 3]> = Vec::with_capacity(boundary.len());
        let mut used: HashMap<usize, usize> = next.clone();
        for &(start, _) in &boundary {
            if used.get(&start).is_none() {
                continue;
            }
            let mut cur = start;
            loop {
                let nxt = match used.remove(&cur) {
                    Some(n) => n,
                    None => break,
                };
                fans.push([cur, nxt, p_idx]);
                cur = nxt;
            }
        }

        // хирургия: выкинуть плохие, вставить веер
        let mut keep: Vec<[usize; 3]> = Vec::with_capacity(tris.len() - bad.len() + fans.len());
        let bad_set: std::collections::HashSet<usize> = bad.into_iter().collect();
        for (ti, t) in tris.into_iter().enumerate() {
            if !bad_set.contains(&ti) {
                keep.push(t);
            }
        }
        keep.extend(fans);
        tris = keep;
    }

    // 3. выкинуть супер-треугольники (вершины 0..2)
    tris.retain(|t| t[0] > 2 && t[1] > 2 && t[2] > 2);
    for t in tris.iter_mut() {
        for v in t.iter_mut() {
            *v -= 3;
        }
    }
    if tris.is_empty() {
        return Err("tin: все точки коллинеарны — TIN вырожден".into());
    }
    Ok(Tin {
        pts,
        tris,
        index: None,
    })
}

/// Полная верификация Делоне (для тестов и аудита): ни одна вершина
/// не лежит СТРОГО внутри описанной окружности чужого треугольника
/// (сокруглённость допускается — оба варианта легальны). Точно, i128.
pub fn verify_delaunay(tin: &Tin) -> Result<(), String> {
    for &[a, b, c] in &tin.tris {
        let (ax, ay) = tin.pts[a];
        let (bx, by) = tin.pts[b];
        let (cx, cy) = tin.pts[c];
        if orient2(ax, ay, bx, by, cx, cy) <= 0 {
            return Err(format!("tin: треугольник {a},{b},{c} не CCW"));
        }
        for (vi, &(px, py)) in tin.pts.iter().enumerate() {
            if vi == a || vi == b || vi == c {
                continue;
            }
            if incircle_det(ax, ay, bx, by, cx, cy, px, py) > 0 {
                return Err(format!(
                    "tin: вершина {vi} строго внутри окружности {a},{b},{c} — не Делоне"
                ));
            }
        }
    }
    Ok(())
}

/// Детерминированный ГПСЧ для тестов (splitmix64 — как в Этерии).
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Формула Эйлера: T = 2n − 2 − h (h — вершины оболочки).
    fn euler_check(tin: &Tin) {
        let n = tin.pts.len();
        let h = {
            let mut v: Vec<usize> = tin.hull().iter().flat_map(|e| e.iter().cloned()).collect();
            v.sort();
            v.dedup();
            v.len()
        };
        assert_eq!(
            tin.tris.len(),
            2 * n - 2 - h,
            "Эйлер: n={n}, h={h}, T={}",
            tin.tris.len()
        );
    }

    /// Каноническая форма для сравнения машин: вершины треугольника
    /// сортируются, набор — сортируется (порядок в векторе не важен).
    fn canonical(tin: &Tin) -> Vec<[usize; 3]> {
        let mut v: Vec<[usize; 3]> = tin
            .tris
            .iter()
            .map(|t| {
                let mut s = *t;
                s.sort_unstable();
                s
            })
            .collect();
        v.sort_unstable();
        v
    }

    /// Кросс-аудит ускорения: BFS-полость обязана находить то же
    /// множество плохих, что и лобовой скан (иначе полость была бы
    /// несвязной — тест ловит и это).
    fn cross_check(points: &[(i64, i64)]) {
        let fast = delaunay(points).unwrap();
        let brute = delaunay_bruteforce(points).unwrap();
        verify_delaunay(&fast).unwrap();
        verify_delaunay(&brute).unwrap();
        assert_eq!(
            canonical(&fast),
            canonical(&brute),
            "ускоренная машина разошлась с эталоном на {} точках",
            points.len()
        );
        euler_check(&fast);
        euler_check(&brute);
    }

    #[test]
    fn square_with_center() {
        // квадрат + центр: 4 треугольника, все содержат центр
        let pts = vec![(0, 0), (4, 0), (4, 4), (0, 4), (2, 2)];
        let tin = delaunay(&pts).unwrap();
        assert_eq!(tin.tris.len(), 4);
        verify_delaunay(&tin).unwrap();
        euler_check(&tin);
        // центр в каждом треугольнике
        for t in &tin.tris {
            assert!(t.contains(&4), "треугольник {t:?} без центра");
        }
    }

    #[test]
    fn grid_euler_and_delaunay() {
        // сетка 4×4: n=16, на границе триангуляции ВСЕ 12 внешних точек
        // (коллинеарные на рёбрах оболочки включительно) ⇒ T = 2·16−2−12 = 18
        let mut pts = Vec::new();
        for y in 0..4 {
            for x in 0..4 {
                pts.push((x * 3, y * 3));
            }
        }
        let tin = delaunay(&pts).unwrap();
        assert_eq!(tin.tris.len(), 18);
        verify_delaunay(&tin).unwrap();
        euler_check(&tin);
        // каждое внутреннее ребро ровно у двух треугольников
        let mut cnt: HashMap<(usize, usize), u32> = HashMap::new();
        for &[a, b, c] in &tin.tris {
            for &(u, v) in &[(a, b), (b, c), (c, a)] {
                let key = if u < v { (u, v) } else { (v, u) };
                *cnt.entry(key).or_insert(0) += 1;
            }
        }
        assert!(cnt.values().all(|&c| c <= 2), "ребро с тремя треугольниками");
    }

    #[test]
    fn cocircular_quartet_fly_breaks_the_tie() {
        // ТОЧНО сокруглённые: (±1,0),(0,±1) на единичной окружности.
        // Детерминант = 0 — оба варианта легальны, муха выбирает порядок.
        let pts = vec![(1, 0), (0, 1), (-1, 0), (0, -1)];
        let tin = delaunay(&pts).unwrap();
        assert_eq!(tin.tris.len(), 2, "диагональ делит четырёхугольник");
        // строгий Делоне не нарушен (сокруглённость — не нарушение)
        verify_delaunay(&tin).unwrap();
        // детерминизм: вторая сборка — побитово та же
        let tin2 = delaunay(&pts).unwrap();
        assert_eq!(tin.tris, tin2.tris);
    }

    #[test]
    fn collinear_plus_one() {
        // гребёнка на прямой + одна точка вне: веер из одной вершины
        let mut pts: Vec<(i64, i64)> = (0..8).map(|i| (i * 5, 0)).collect();
        pts.push((20, 7));
        let tin = delaunay(&pts).unwrap();
        verify_delaunay(&tin).unwrap();
        euler_check(&tin);
        // каждая прямолинейная точка участвует в треугольниках
        for vi in 0..pts.len() {
            assert!(
                tin.tris.iter().any(|t| t.contains(&vi)),
                "вершина {vi} потеряна"
            );
        }
    }

    #[test]
    fn all_collinear_is_honest_error() {
        assert!(delaunay(&[(0, 0), (5, 0), (9, 0)]).is_err());
        assert!(delaunay(&[(2, 2), (2, 2), (2, 2)]).is_err());
        assert!(delaunay(&[(1, 1), (1, 1)]).is_err());
    }

    #[test]
    fn random_cloud_full_verification() {
        // 200 точек splitmix64 (детерминизм): полная проверка Делоне
        let mut state = 0xDEADBEEFu64;
        let pts: Vec<(i64, i64)> = (0..200)
            .map(|_| {
                let x = (splitmix64(&mut state) % 20_000) as i64 - 10_000;
                let y = (splitmix64(&mut state) % 20_000) as i64 - 10_000;
                (x, y)
            })
            .collect();
        let tin = delaunay(&pts).unwrap();
        verify_delaunay(&tin).unwrap();
        euler_check(&tin);
        // побитовый детерминизм второго прогона
        let tin2 = delaunay(&pts).unwrap();
        assert_eq!(tin.tris, tin2.tris);
    }

    #[test]
    fn sorted_input_is_decorrelated_by_fly() {
        // adversarial-порядок: точки по возрастанию x — муха перемешивает
        let pts: Vec<(i64, i64)> = (0..100)
            .map(|i| (i * 7, (i * i * 13 % 101) * 5))
            .collect();
        let tin = delaunay(&pts).unwrap();
        verify_delaunay(&tin).unwrap();
        euler_check(&tin);
    }

    #[test]
    fn fly_order_is_deterministic_permutation() {
        for n in [0usize, 1, 2, 7, 50, 333] {
            let o1 = fly_order(n);
            let o2 = fly_order(n);
            assert_eq!(o1, o2, "детерминизм n={n}");
            let mut sorted = o1.clone();
            sorted.sort();
            assert_eq!(sorted, (0..n).collect::<Vec<_>>(), "перестановка n={n}");
        }
        // золотая фаза — и та же константа, что в universal_letters
        assert_eq!(fly_phase(1), PHI % TAU);
        assert_eq!(fly_phase(0), 0.0);
    }

    #[test]
    fn height_interpolation() {
        // пирамида: центр выше углов — высота в центре = z центра
        let pts = vec![(0, 0), (8, 0), (8, 8), (0, 8), (4, 4)];
        let z = vec![0.0, 0.0, 0.0, 0.0, 10.0];
        let tin = delaunay(&pts).unwrap();
        assert_eq!(tin.height_at(&z, 4, 4), Some(10.0));
        // вершина оболочки
        assert_eq!(tin.height_at(&z, 0, 0), Some(0.0));
        // середина ребра угол→центр: линейная интерполяция 0→10
        let h = tin.height_at(&z, 2, 2).unwrap();
        assert!((h - 5.0).abs() < 1e-12, "середина ребра (0,0)-(4,4): {h}");
        // середина ребра основания (оболочка): z обоих концов 0
        assert_eq!(tin.height_at(&z, 4, 0), Some(0.0));
        // вне оболочки
        assert_eq!(tin.height_at(&z, 20, 20), None);
    }

    #[test]
    fn super_triangle_covers_bbox() {
        // супер-треугольник строго содержит bbox при любом M (ориентации > 0)
        for &m in &[1i64, 10, 1000, 2_391_484] {
            let (a, b, c) = (
                (-3 * m - 2, -2 * m - 1),
                (3 * m + 2, -2 * m - 1),
                (0, 3 * m + 2),
            );
            for &(px, py) in &[(-m, m), (m, m), (-m, -m), (m, -m)] {
                let o1 = orient2(a.0, a.1, b.0, b.1, px, py);
                let o2 = orient2(b.0, b.1, c.0, c.1, px, py);
                let o3 = orient2(c.0, c.1, a.0, a.1, px, py);
                assert!(o1 > 0 && o2 > 0 && o3 > 0, "угол ({px},{py}) вне супер M={m}");
            }
        }
    }

    // ─────────── Сессия-16: ускорение против эталона ───────────

    #[test]
    fn accelerated_matches_bruteforce_core() {
        cross_check(&[(0, 0), (4, 0), (4, 4), (0, 4), (2, 2)]);
        // сетка 4×4 — сад сокруглённостей (каждый прямоугольник решётки)
        let mut pts = Vec::new();
        for y in 0..4 {
            for x in 0..4 {
                pts.push((x * 3, y * 3));
            }
        }
        cross_check(&pts);
    }

    #[test]
    fn accelerated_matches_bruteforce_grid12() {
        // сетка 12×12 = 144 точки: сотни сокруглённых четвёрок —
        // стресс-тест связности BFS-полости
        let mut pts = Vec::new();
        for y in 0..12 {
            for x in 0..12 {
                pts.push((x * 5, y * 5));
            }
        }
        cross_check(&pts);
    }

    #[test]
    fn accelerated_matches_bruteforce_random() {
        // три облака разных семян
        for seed in [0xDEADBEEFu64, 0xC0FFEE, 2026] {
            let mut st = seed;
            let pts: Vec<(i64, i64)> = (0..300)
                .map(|_| {
                    let x = (splitmix64(&mut st) % 30_000) as i64 - 15_000;
                    let y = (splitmix64(&mut st) % 30_000) as i64 - 15_000;
                    (x, y)
                })
                .collect();
            cross_check(&pts);
        }
    }

    #[test]
    fn accelerated_matches_bruteforce_cocircular_ring() {
        // 40 точек, квантованных на одну окружность — сплошные det = 0
        let mut pts = Vec::new();
        for i in 0..40usize {
            let a = i as f64 * std::f64::consts::TAU / 40.0;
            pts.push(((1000.0 * a.cos()).round() as i64, (1000.0 * a.sin()).round() as i64));
        }
        cross_check(&pts);
    }

    #[test]
    fn accelerated_large_cloud_verified() {
        // 1500 точек: ускоренная машина + полная верификация Делоне
        // (лобая на таком объёме уже секунды — сравнивать не будем)
        let mut st = 0xBEEFu64;
        let pts: Vec<(i64, i64)> = (0..1500)
            .map(|_| {
                let x = (splitmix64(&mut st) % 60_000) as i64 - 30_000;
                let y = (splitmix64(&mut st) % 60_000) as i64 - 30_000;
                (x, y)
            })
            .collect();
        let tin = delaunay(&pts).unwrap();
        verify_delaunay(&tin).unwrap();
        euler_check(&tin);
        assert_eq!(tin.tris.len() % 2, 0, "T чётно при отсутствии коллинеарностей");
    }

    #[test]
    fn dag_survives_super_triangle_death() {
        // после смерти супер-треугольника корень DAG мёртв — спуск обязан
        // работать через его детей (проверяем поздними вставками)
        let pts = vec![(0, 0), (100, 0), (100, 100), (0, 100), (50, 50), (25, 75)];
        let tin = delaunay(&pts).unwrap();
        verify_delaunay(&tin).unwrap();
        euler_check(&tin);
        // все 6 точек живы в триангуляции
        for vi in 0..6 {
            assert!(tin.tris.iter().any(|t| t.contains(&vi)), "вершина {vi} потеряна");
        }
    }

    #[test]
    fn spatial27_index_height_at_equivalence() {
        // индекс 27-дерева НЕ меняет ни один ответ height_at
        let mut st = 0xABBAu64;
        let pts: Vec<(i64, i64)> = (0..400)
            .map(|_| {
                let x = (splitmix64(&mut st) % 9_000) as i64 - 4_500;
                let y = (splitmix64(&mut st) % 9_000) as i64 - 4_500;
                (x, y)
            })
            .collect();
        let z: Vec<f64> = (0..pts.len()).map(|i| (i as f64 * 0.37).fract()).collect();
        let mut tin = delaunay(&pts).unwrap();
        let brute = tin.clone(); // копия БЕЗ индекса
        let (objs, nodes) = tin.build_spatial27().unwrap();
        assert_eq!(objs, tin.tris.len());
        assert!(nodes >= 1, "дерево обязано существовать");
        // пробы: узлы решётки по bbox, на рёбрах и за оболочкой
        let mut st2 = 0x1234u64;
        for _ in 0..500 {
            let qx = (splitmix64(&mut st2) % 12_000) as i64 - 6_000;
            let qy = (splitmix64(&mut st2) % 12_000) as i64 - 6_000;
            assert_eq!(
                tin.height_at(&z, qx, qy),
                brute.height_at(&z, qx, qy),
                "расхождение индекса в ({qx},{qy})"
            );
        }
        // за пределами: None и там и там
        assert_eq!(tin.height_at(&z, 1_000_000, 1_000_000), None);
    }

    #[test]
    fn spatial27_index_rebuild_and_small_tin() {
        // малый TIN: 3 точки — 1 треугольник, индекс строится и работает
        let mut tin = delaunay(&[(0, 0), (8, 0), (4, 6)]).unwrap();
        let z = vec![1.0, 2.0, 3.0];
        let (objs, _) = tin.build_spatial27().unwrap();
        assert_eq!(objs, 1);
        // центроид: среднее высот
        let h = tin.height_at(&z, 4, 2).unwrap();
        assert!((h - 2.0).abs() < 1e-12, "центроид: {h}");
        // перестройка — ок
        assert!(tin.build_spatial27().is_ok());
        // пустой TIN честно отказывает
        let mut empty = Tin::empty();
        assert!(empty.build_spatial27().is_err());
    }

    /// Бенчмарк-аудит ускорения Сессии-16:
    /// `cargo test --lib delaunay -- --ignored --nocapture`
    #[test]
    #[ignore = "бенчмарк: python-скрипт сравнения времени fast против bruteforce"]
    fn bench_delaunay_vs_bruteforce() {
        let mut st = 0xB16B00B5u64;
        let pts: Vec<(i64, i64)> = (0..2000)
            .map(|_| {
                let x = (splitmix64(&mut st) % 80_000) as i64 - 40_000;
                let y = (splitmix64(&mut st) % 80_000) as i64 - 40_000;
                (x, y)
            })
            .collect();
        let t0 = std::time::Instant::now();
        let fast = delaunay(&pts).unwrap();
        let t_fast = t0.elapsed();
        let t1 = std::time::Instant::now();
        let brute = delaunay_bruteforce(&pts).unwrap();
        let t_brute = t1.elapsed();
        let speedup = t_brute.as_secs_f64() / t_fast.as_secs_f64().max(1e-9);
        println!(
            "n = {} · fast {:?} · bruteforce {:?} · ускорение ×{:.1}",
            pts.len(),
            t_fast,
            t_brute,
            speedup
        );
        assert_eq!(canonical(&fast), canonical(&brute), "результаты обязаны совпасть");
    }
}
