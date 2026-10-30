//! 3D-укладчик конформеров из молекулярного графа (Stage 1).
//!
//! # Алгоритм
//!
//! 1. **Разворот неявных H** в явные узлы (для шаростержневого рендера).
//! 2. **Кольца**: каждое SSSR-кольцо — правильный многоугольник (радиус из
//!    средней длины связи) в плоскости z=0. Сплавленные кольца пристраиваются
//!    дугой к уже размещённым атомам (нафталин, индол, пурин).
//! 3. **Цепи**: BFS от размещённых атомов; направление нового атома — из
//!    идеальных направлений гибридизации родителя (sp³ 109.47°, sp² 120°,
//!    sp 180°), свободное от занятых связей, максимально удалённое от
//!    уже размещённых атомов.
//! 4. **Релаксация** (детерминированная, без случайностей): пружины связей,
//!    мягкие 1-3 ограничения, отталкивание Ван-дер-Ваальса, лёгкое
//!    уплощение ароматических колец.
//!
//! Это эскизный конформер для визуализации и докинга-прототипа,
//! НЕ минимизация энергии: честная геометрия будет через RHF-градиенты
//! (POLER-ERI) на следующих стадиях лестницы.

use super::smiles::{BondOrder, MoleculeGraph};

/// Гибридизация атома.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hybrid {
    Sp,
    Sp2,
    Sp3,
}

impl Hybrid {
    pub fn ideal_angle(self) -> f64 {
        match self {
            Hybrid::Sp => std::f64::consts::PI,
            Hybrid::Sp2 => std::f64::consts::PI * 2.0 / 3.0,
            Hybrid::Sp3 => 1.9106332362490184, // 109.47°
        }
    }

    pub fn name_ru(self) -> &'static str {
        match self {
            Hybrid::Sp => "sp",
            Hybrid::Sp2 => "sp²",
            Hybrid::Sp3 => "sp³",
        }
    }
}

/// Ковалентные радиусы (Pyykkö 2009, одинарная связь, Å).
pub fn covalent_radius(symbol: &str) -> f64 {
    match symbol {
        "H" => 0.31,
        "B" => 0.84,
        "C" => 0.76,
        "N" => 0.71,
        "O" => 0.66,
        "F" => 0.57,
        "Si" => 1.11,
        "P" => 1.07,
        "S" => 1.05,
        "Cl" => 1.02,
        "Se" => 1.20,
        "Br" => 1.20,
        "I" => 1.39,
        "Na" => 1.66,
        "K" => 2.03,
        "Li" => 1.28,
        "Mg" => 1.41,
        "Ca" => 1.76,
        "Fe" => 1.32,
        "Zn" => 1.22,
        "Cu" => 1.32,
        "Mn" => 1.39,
        "Co" => 1.26,
        "Ni" => 1.24,
        "Cr" => 1.39,
        "Ti" => 1.60,
        "V" => 1.53,
        "Ag" => 1.45,
        "Au" => 1.36,
        _ => 1.50, // консервативный дефолт
    }
}

/// Ван-дер-Ваальсовы радиусы (Bondi 1964, Å).
pub fn vdw_radius(symbol: &str) -> f64 {
    match symbol {
        "H" => 1.20,
        "C" => 1.70,
        "N" => 1.55,
        "O" => 1.52,
        "F" => 1.47,
        "Si" => 2.10,
        "P" => 1.80,
        "S" => 1.80,
        "Cl" => 1.75,
        "Se" => 1.90,
        "Br" => 1.85,
        "I" => 1.98,
        "Li" => 1.82,
        "Na" => 2.27,
        "K" => 2.75,
        "Mg" => 1.73,
        "Ca" => 2.31,
        "Fe" => 2.0,
        "Zn" => 1.39,
        "Cu" => 1.4,
        _ => 1.8,
    }
}

/// Множитель длины связи по порядку.
fn bond_factor(order: BondOrder) -> f64 {
    match order {
        BondOrder::Single => 1.0,
        BondOrder::Double => 0.87,
        BondOrder::Triple => 0.78,
        BondOrder::Quadruple => 0.72,
        BondOrder::Aromatic => 0.91,
    }
}

/// Целевая длина связи (Å).
fn bond_length(sym_a: &str, sym_b: &str, order: BondOrder) -> f64 {
    (covalent_radius(sym_a) + covalent_radius(sym_b)) * bond_factor(order)
}

/// Гибридизация атома по окружению.
pub fn hybridization(g: &MoleculeGraph, idx: usize) -> Hybrid {
    let nbrs = g.neighbors(idx);
    let has_multiple = nbrs.iter().any(|(_, o)| {
        matches!(o, BondOrder::Double | BondOrder::Triple | BondOrder::Aromatic)
    });
    let n_multiple: usize = nbrs
        .iter()
        .filter(|(_, o)| matches!(o, BondOrder::Double | BondOrder::Triple))
        .count();
    let degree = nbrs.len();
    // sp: тройная связь, две двойных (аллен), или степень 2 при π-окружении
    if n_multiple >= 2 || nbrs.iter().any(|(_, o)| *o == BondOrder::Triple) {
        return Hybrid::Sp;
    }
    if has_multiple && degree <= 3 {
        return Hybrid::Sp2;
    }
    if g.atoms[idx].aromatic && degree <= 3 {
        return Hybrid::Sp2;
    }
    Hybrid::Sp3
}

// ─── Развёрнутый граф (явные водороды) ──────────────────────────────────

/// Узел конформера: тяжёлый атом или явно развёрнутый водород.
#[derive(Debug, Clone)]
pub struct ConfNode {
    pub symbol: String,
    pub z: u8,
    pub is_h: bool,
}

/// Связь конформера.
#[derive(Debug, Clone)]
pub struct ConfBond {
    pub a: usize,
    pub b: usize,
    pub order: BondOrder,
    /// Целевая длина (Å).
    pub target: f64,
}

/// 3D-конформер: узлы, связи, координаты (Å).
#[derive(Debug, Clone)]
pub struct Conformer {
    pub nodes: Vec<ConfNode>,
    pub bonds: Vec<ConfBond>,
    pub positions: Vec<[f64; 3]>,
    /// Число тяжёлых атомов.
    pub heavy: usize,
    /// Атом графа → узел конформера (H-узлы идут после своего родителя).
    pub heavy_map: Vec<usize>,
    /// Сработала ли релаксация.
    pub relaxed: bool,
}

impl Conformer {
    /// Расстояние между атомами.
    pub fn dist(&self, i: usize, j: usize) -> f64 {
        let d = sub(self.positions[i], self.positions[j]);
        norm(d)
    }

    /// Габаритный бокс (min, max).
    pub fn bbox(&self) -> ([f64; 3], [f64; 3]) {
        let mut mn = [f64::MAX; 3];
        let mut mx = [f64::MIN; 3];
        for p in &self.positions {
            for k in 0..3 {
                mn[k] = mn[k].min(p[k]);
                mx[k] = mx[k].max(p[k]);
            }
        }
        (mn, mx)
    }

    /// RMS отклонения ароматических колец от плоскости (для контроля).
    pub fn aromatic_planarity(&self, g: &MoleculeGraph) -> f64 {
        let mut total = 0.0f64;
        let mut count = 0usize;
        for ring in &g.rings {
            if ring.len() > 6 || !ring.iter().all(|&i| g.atoms[i].aromatic) {
                continue;
            }
            // Плоскость по SVD-lite: нормаль из первых трёх атомов
            let (i, j, k) = (ring[0], ring[1], ring[2 % ring.len()]);
            let p0 = self.positions[i];
            let n = normalize(cross(
                sub(self.positions[j], p0),
                sub(self.positions[k], p0),
            ));
            for &a in ring {
                let d = dot(sub(self.positions[a], p0), n);
                total += d * d;
                count += 1;
            }
        }
        if count == 0 {
            0.0
        } else {
            (total / count as f64).sqrt()
        }
    }
}

// ─── Векторная мелочь (без внешних крейтов) ─────────────────────────────

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
fn normalize(a: [f64; 3]) -> [f64; 3] {
    let n = norm(a);
    if n < 1e-12 {
        [1.0, 0.0, 0.0]
    } else {
        scale(a, 1.0 / n)
    }
}

// ─── Укладка ────────────────────────────────────────────────────────────

/// Построить эскизный 3D-конформер молекулярного графа.
pub fn embed(g: &MoleculeGraph) -> Result<Conformer, String> {
    if g.atoms.is_empty() {
        return Err("пустой молекулярный граф".into());
    }

    // 1. Разворот неявных H в явные узлы (тяжёлые — первыми, затем H-веер
    // каждого атома; нумерация детерминирована порядком графа)
    let mut nodes: Vec<ConfNode> = Vec::with_capacity(g.atoms.len() * 2);
    let mut heavy_map: Vec<usize> = Vec::with_capacity(g.atoms.len());
    let mut extra_bonds: Vec<(usize, usize, f64)> = Vec::new();
    for (i, a) in g.atoms.iter().enumerate() {
        heavy_map.push(nodes.len());
        nodes.push(ConfNode {
            symbol: a.symbol.clone(),
            z: a.z,
            is_h: a.symbol == "H",
        });
        for _ in 0..a.h_count {
            let h = nodes.len();
            nodes.push(ConfNode {
                symbol: "H".into(),
                z: 1,
                is_h: true,
            });
            extra_bonds.push((
                heavy_map[i],
                h,
                bond_length(&g.atoms[i].symbol, "H", BondOrder::Single),
            ));
        }
    }
    let mut bonds: Vec<ConfBond> = Vec::with_capacity(g.bonds.len() + extra_bonds.len());
    for b in &g.bonds {
        bonds.push(ConfBond {
            a: heavy_map[b.a],
            b: heavy_map[b.b],
            order: b.order,
            target: bond_length(&g.atoms[b.a].symbol, &g.atoms[b.b].symbol, b.order),
        });
    }
    for (a, h, t) in extra_bonds {
        bonds.push(ConfBond {
            a,
            b: h,
            order: BondOrder::Single,
            target: t,
        });
    }

    let n = nodes.len();
    let mut positions: Vec<[f64; 3]> = vec![[f64::NAN; 3]; n];

    // Список смежности по узлам
    let mut adjacency: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    for b in &bonds {
        adjacency[b.a].push((b.b, b.target));
        adjacency[b.b].push((b.a, b.target));
    }

    // 2. Кольца: правильные многоугольники + пристройка сплавленных
    for ring in &g.rings {
        let placed: Vec<usize> = ring
            .iter()
            .copied()
            .filter(|&a| positions[heavy_map[a]][0].is_finite())
            .map(|a| heavy_map[a])
            .collect();
        if placed.is_empty() {
            place_polygon(ring, g, &heavy_map, &mut positions);
        } else {
            attach_ring(ring, g, &heavy_map, &mut positions, &placed);
        }
    }

    // 3. Цепи: BFS
    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    let mut visited = vec![false; n];
    for a in 0..g.atoms.len() {
        let node = heavy_map[a];
        if positions[node][0].is_finite() {
            queue.push_back(node);
            visited[node] = true;
        }
    }
    // Изолированные фрагменты без колец: старт с первого атома
    if queue.is_empty() {
        positions[heavy_map[0]] = [0.0, 0.0, 0.0];
        queue.push_back(heavy_map[0]);
        visited[heavy_map[0]] = true;
    }
    let mut fallback_origin = [0.0, 0.0, 0.0];
    while let Some(u) = queue.pop_front() {
        // Соседи-узлы u (в терминах узлов) — размещаем неразмещённых
        let nbrs_u: Vec<(usize, f64)> = adjacency[u].clone();
        let placed_dirs: Vec<[f64; 3]> = nbrs_u
            .iter()
            .filter(|(w, _)| positions[*w][0].is_finite())
            .map(|(w, _)| normalize(sub(positions[*w], positions[u])))
            .collect();
        let mut new_dirs_stack: Vec<[f64; 3]> = Vec::new();
        for (w, target) in &nbrs_u {
            if positions[*w][0].is_finite() || visited[*w] {
                continue;
            }
            // Гибридизация u
            let u_atom = node_atom_idx(&heavy_map, u);
            let hyb = if let Some(ai) = u_atom {
                hybridization(g, ai)
            } else {
                Hybrid::Sp3 // H не бывает родителем в SMILES-графе
            };
            let mut dirs = placed_dirs.clone();
            dirs.extend(new_dirs_stack.iter().copied());
            let d = new_direction(hyb, &dirs, &positions, u);
            positions[*w] = add(positions[u], scale(d, *target));
            new_dirs_stack.push(d);
            visited[*w] = true;
            queue.push_back(*w);
        }
        fallback_origin = positions[u];
    }
    // Хвосты: если что-то осталось неразмещённым (изолированные компоненты)
    for i in 0..n {
        if !positions[i][0].is_finite() {
            positions[i] = add(fallback_origin, [5.0, 0.0, 0.0]);
            fallback_origin = positions[i];
        }
    }

    // Центрирование
    let mut centroid = [0.0f64; 3];
    for p in &positions {
        centroid = add(centroid, *p);
    }
    centroid = scale(centroid, 1.0 / n as f64);
    for p in positions.iter_mut() {
        *p = sub(*p, centroid);
    }

    // 4. Релаксация
    let relaxed = relax(&nodes, &bonds, &mut positions, g, &heavy_map, &adjacency);

    Ok(Conformer {
        nodes,
        bonds,
        positions,
        heavy: g.atoms.len(),
        heavy_map,
        relaxed,
    })
}

// ─── Конформер из внешних координат (SDF, контур B) ─────────────────────

/// Построить конформер из ЗАДАННЫХ тяжёлых координат (SDF-путь).
///
/// * `heavy_pos` — координаты тяжёлых атомов графа (порядок `g.atoms`),
///   принимаются как есть (SDF-конформер приоритетнее укладчика, без
///   релаксации — доверяем файлу);
/// * `explicit_h[i]` — координаты явных H атома i из SDF (используются
///   первыми); недостающие до `h_count` H строятся конусом по
///   гибридизации (sp³: 109.5°, sp²: 120°, sp: линейно) — детерминированно.
pub fn conformer_from_positions(
    g: &MoleculeGraph,
    heavy_pos: &[[f64; 3]],
    explicit_h: &[Vec<[f64; 3]>],
) -> Result<Conformer, String> {
    if g.atoms.is_empty() {
        return Err("пустой молекулярный граф".into());
    }
    if heavy_pos.len() != g.atoms.len() {
        return Err(format!(
            "координат {} ≠ атомов {} (SDF-путь)",
            heavy_pos.len(),
            g.atoms.len()
        ));
    }
    for p in heavy_pos {
        if !p.iter().all(|v| v.is_finite()) {
            return Err("нечисловые координаты (NaN/inf) в SDF".into());
        }
    }

    // Узлы: тяжёлые + H-веер каждого атома (нумерация как в embed)
    let mut nodes: Vec<ConfNode> = Vec::with_capacity(g.atoms.len() * 2);
    let mut heavy_map: Vec<usize> = Vec::with_capacity(g.atoms.len());
    // H-позиции атома i: явные из файла, затем сгенерированные
    let mut h_pos: Vec<Vec<[f64; 3]>> = Vec::with_capacity(g.atoms.len());
    for (i, a) in g.atoms.iter().enumerate() {
        heavy_map.push(nodes.len());
        nodes.push(ConfNode {
            symbol: a.symbol.clone(),
            z: a.z,
            is_h: a.symbol == "H",
        });
        let mut hp: Vec<[f64; 3]> = Vec::with_capacity(a.h_count as usize);
        if i < explicit_h.len() {
            for p in explicit_h[i].iter().take(a.h_count as usize) {
                if p.iter().all(|v| v.is_finite()) {
                    hp.push(*p);
                }
            }
        }
        // недостающие H — конус по гибридизации
        let missing = (a.h_count as usize).saturating_sub(hp.len());
        if missing > 0 {
            let cone = h_cone_dirs(g, i, heavy_pos, missing);
            let bl = bond_length(&a.symbol, "H", BondOrder::Single);
            for d in cone {
                let p = [
                    heavy_pos[i][0] + d[0] * bl,
                    heavy_pos[i][1] + d[1] * bl,
                    heavy_pos[i][2] + d[2] * bl,
                ];
                hp.push(p);
            }
        }
        for _ in 0..(a.h_count as usize) {
            nodes.push(ConfNode {
                symbol: "H".into(),
                z: 1,
                is_h: true,
            });
        }
        h_pos.push(hp);
    }

    // Связи: тяжёлые⇄тяжёлые (из графа) + тяжёлый⇄H
    let mut bonds: Vec<ConfBond> = Vec::with_capacity(g.bonds.len() + g.atoms.len() * 2);
    for b in &g.bonds {
        bonds.push(ConfBond {
            a: heavy_map[b.a],
            b: heavy_map[b.b],
            order: b.order,
            target: bond_length(&g.atoms[b.a].symbol, &g.atoms[b.b].symbol, b.order),
        });
    }
    let mut node = 0usize;
    for (i, a) in g.atoms.iter().enumerate() {
        node += 1; // тяжёлый узел
        for _ in 0..(a.h_count as usize) {
            let bl = bond_length(&a.symbol, "H", BondOrder::Single);
            bonds.push(ConfBond {
                a: heavy_map[i],
                b: node,
                order: BondOrder::Single,
                target: bl,
            });
            node += 1;
        }
    }

    // Позиции: тяжёлые + H (явные/конус)
    let mut positions: Vec<[f64; 3]> = Vec::with_capacity(nodes.len());
    for i in 0..g.atoms.len() {
        positions.push(heavy_pos[i]);
        for p in &h_pos[i] {
            positions.push(*p);
        }
    }

    Ok(Conformer {
        nodes,
        bonds,
        positions,
        heavy: g.atoms.len(),
        heavy_map,
        relaxed: false,
    })
}

/// Направления `m` недостающих H атома i: конус вокруг биссектрисы
/// тяжёлых соседей. Полуугол — из VSEPR-равномерности: сумма ВСЕХ
/// направлений связей идеальной геометрии равна нулю → cos α = |Σn|/m.
/// Проверки: CH₂ (2 соседа) → α=54.74° (H–H 109.47°); CH₃ (1 сосед) →
/// α=70.53° (H–H 120°); ароматический CH → α=0 (биссектриса в плоскости
/// кольца); =CH₂ (1 сосед, sp²) → α=60°. Гибридизация следует из геометрии.
fn h_cone_dirs(g: &MoleculeGraph, i: usize, heavy_pos: &[[f64; 3]], m: usize) -> Vec<[f64; 3]> {
    // направления к тяжёлым соседям
    let mut nbr_dirs: Vec<[f64; 3]> = Vec::new();
    for (w, _) in g.neighbors(i) {
        let d = sub(heavy_pos[w], heavy_pos[i]);
        let n = norm(d);
        if n > 1e-9 {
            nbr_dirs.push(scale(d, 1.0 / n));
        }
    }
    // ось конуса: против суммы направлений к соседям (биссектриса H)
    let mut sum = [0.0f64; 3];
    for d in &nbr_dirs {
        sum = add(sum, *d);
    }
    let sn = norm(sum);
    let c = if sn > 1e-6 {
        scale(sum, -1.0 / sn)
    } else {
        // изолированный атом (k=0) — детерминированная ось +X
        [1.0, 0.0, 0.0]
    };
    // VSEPR: cos α = |Σn| / m (зажать в [0,1] от неидеальных структур)
    let cos_a = (sn / m as f64).clamp(0.0, 1.0);
    let alpha = cos_a.acos();
    // ортонормальный базис ⊥ c
    let ref_dir = nbr_dirs
        .first()
        .copied()
        .unwrap_or([0.0, 1.0, 0.0]);
    let mut e1 = sub(ref_dir, scale(c, dot(ref_dir, c)));
    if norm(e1) < 1e-6 {
        e1 = sub([0.0, 0.0, 1.0], scale(c, c[2]));
        if norm(e1) < 1e-6 {
            e1 = sub([1.0, 0.0, 0.0], scale(c, c[0]));
        }
    }
    e1 = scale(e1, 1.0 / norm(e1));
    let e2 = cross(c, e1);
    // m направлений по конусу (β равномерно; при m=1 → α=0, dir = c)
    (0..m)
        .map(|j| {
            let beta = std::f64::consts::TAU * j as f64 / m as f64;
            let radial = add(scale(e1, beta.cos()), scale(e2, beta.sin()));
            add(scale(c, alpha.cos()), scale(radial, alpha.sin()))
        })
        .collect()
}

fn node_atom_idx(heavy_map: &[usize], node: usize) -> Option<usize> {
    heavy_map.iter().position(|&h| h == node)
}

/// Правильный многоугольник для кольца в плоскости z=0.
fn place_polygon(
    ring: &[usize],
    g: &MoleculeGraph,
    heavy_map: &[usize],
    positions: &mut Vec<[f64; 3]>,
) {
    let k = ring.len();
    if k < 3 {
        return;
    }
    // Средняя длина связи кольца
    let mut l_sum = 0.0f64;
    for i in 0..k {
        let a = ring[i];
        let b = ring[(i + 1) % k];
        let order = g
            .bonds
            .iter()
            .find(|bb| {
                (bb.a == a && bb.b == b) || (bb.a == b && bb.b == a)
            })
            .map(|bb| bb.order)
            .unwrap_or(BondOrder::Single);
        l_sum += bond_length(&g.atoms[a].symbol, &g.atoms[b].symbol, order);
    }
    let side = l_sum / k as f64;
    let r = side / (2.0 * (std::f64::consts::PI / k as f64).sin());
    for (i, &a) in ring.iter().enumerate() {
        let ang = 2.0 * std::f64::consts::PI * i as f64 / k as f64;
        positions[heavy_map[a]] = [r * ang.cos(), r * ang.sin(), 0.0];
    }
}

/// Пристройка кольца к уже размещённым атомам (сплавленные системы).
fn attach_ring(
    ring: &[usize],
    g: &MoleculeGraph,
    heavy_map: &[usize],
    positions: &mut Vec<[f64; 3]>,
    placed_nodes: &[usize],
) {
    let k = ring.len();
    if k < 3 || placed_nodes.is_empty() {
        return;
    }
    let placed_set: std::collections::HashSet<usize> = placed_nodes.iter().copied().collect();
    // Найдём самый длинный циклический подряд идущий размещённый блок
    let mut best_start = 0usize;
    let mut best_len = 0usize;
    for s in 0..k {
        let mut l = 0usize;
        while l < k && placed_set.contains(&heavy_map[ring[(s + l) % k]]) {
            l += 1;
        }
        if l > best_len {
            best_len = l;
            best_start = s;
        }
    }
    if best_len == k {
        return; // всё размещено
    }
    if best_len == 0 {
        place_polygon(ring, g, heavy_map, positions);
        return;
    }
    // Концы блока
    let e1 = heavy_map[ring[(best_start + best_len - 1) % k]];
    let e2 = heavy_map[ring[best_start % k]];
    let p1 = positions[e1];
    let p2 = positions[e2];
    let chord = sub(p2, p1);
    let chord_len = norm(chord);
    if chord_len < 1e-6 {
        place_polygon(ring, g, heavy_map, positions);
        return;
    }
    // Средняя длина связи кольца
    let mut l_sum = 0.0f64;
    for i in 0..k {
        let a = ring[i];
        let b = ring[(i + 1) % k];
        let order = g
            .bonds
            .iter()
            .find(|bb| (bb.a == a && bb.b == b) || (bb.a == b && bb.b == a))
            .map(|bb| bb.order)
            .unwrap_or(BondOrder::Single);
        l_sum += bond_length(&g.atoms[a].symbol, &g.atoms[b].symbol, order);
    }
    let side = l_sum / k as f64;
    let r = side / (2.0 * (std::f64::consts::PI / k as f64).sin());
    // Центр дуги: на перпендикулярной биссектрисе, сторона — прочь от
    // центроида размещённых атомов блока
    let mut run_centroid = [0.0f64; 3];
    for l in 0..best_len {
        run_centroid = add(run_centroid, positions[heavy_map[ring[(best_start + l) % k]]]);
    }
    run_centroid = scale(run_centroid, 1.0 / best_len as f64);
    let mid = scale(add(p1, p2), 0.5);
    let perp = normalize(cross(chord, [0.0, 0.0, 1.0]));
    let mut away = sub(mid, run_centroid);
    if norm(away) < 1e-6 {
        away = perp;
    }
    away = normalize(away);
    // Высота центра от хорды: h² = r² − (c/2)² (минимум 0.2r)
    let half_c = (chord_len / 2.0).min(r * 0.98);
    let h = (r * r - half_c * half_c).max((0.2 * r) * (0.2 * r)).sqrt();
    let center = add(mid, scale(away, h));
    // Углы e1 и e2 относительно центра
    let v1 = sub(p1, center);
    let v2 = sub(p2, center);
    let mut a1 = v1[1].atan2(v1[0]);
    let mut a2 = v2[1].atan2(v2[0]);
    // Идём от a1 к a2 в сторону «прочь» (через сторону away): направление —
    // то, которое НЕ проходит через run_centroid
    let mut delta = a2 - a1;
    while delta > std::f64::consts::PI {
        delta -= std::f64::consts::TAU;
    }
    while delta < -std::f64::consts::PI {
        delta += std::f64::consts::TAU;
    }
    // Если размещённый блок лежит на стороне away — разворачиваем дугу
    let probe = add(
        center,
        [
            r * (a1 + delta * 0.5).cos(),
            r * (a1 + delta * 0.5).sin(),
            0.0,
        ],
    );
    if dot(sub(probe, center), away) > 0.0 && dot(sub(run_centroid, center), away) > 0.0 {
        delta -= std::f64::consts::TAU.copysign(delta);
        if delta.abs() < 1e-9 {
            delta = std::f64::consts::TAU / k as f64;
        }
    }
    let _ = &mut a1;
    let _ = &mut a2;
    // Неразмещённые атомы кольца — по дуге между e1 и e2
    let m = k - best_len;
    let radius = norm(v1).max(0.7 * r);
    for step in 1..=m {
        let ang = a1 + delta * step as f64 / (m + 1) as f64;
        // позиция в плоскости xy (z=0), затем сдвиг к плоскости размещённых
        let pos = [
            center[0] + radius * ang.cos(),
            center[1] + radius * ang.sin(),
            mid[2],
        ];
        // ищем следующий неразмещённый атом за e1 по порядку кольца
        let idx = ring[(best_start + best_len - 1 + step) % k];
        let node = heavy_map[idx];
        if !positions[node][0].is_finite() {
            positions[node] = pos;
        }
    }
}

/// Направление для нового соседа атома u по гибридизации и занятым направлениям.
fn new_direction(
    hyb: Hybrid,
    dirs: &[[f64; 3]],
    positions: &[[f64; 3]],
    u: usize,
) -> [f64; 3] {
    match dirs.len() {
        0 => [1.0, 0.0, 0.0],
        1 => {
            let v = dirs[0];
            match hyb {
                Hybrid::Sp => scale(v, -1.0),
                Hybrid::Sp2 => rotate_in_plane(v, std::f64::consts::PI * 2.0 / 3.0),
                Hybrid::Sp3 => {
                    // конус 109.47° вокруг −v, детерминированный азимут
                    let perp = canonical_perp(v);
                    let perp2 = normalize(cross(v, perp));
                    let phi: f64 = 0.0;
                    let cos_t = -1.0 / 3.0;
                    let sin_t = (8.0f64 / 9.0).sqrt();
                    let d = add(
                        scale(v, cos_t),
                        add(scale(perp, sin_t * phi.cos()), scale(perp2, sin_t * phi.sin())),
                    );
                    normalize(d)
                }
            }
        }
        2 => {
            let (v1, v2) = (dirs[0], dirs[1]);
            match hyb {
                Hybrid::Sp => scale(v1, -1.0),
                Hybrid::Sp2 => normalize(scale(add(v1, v2), -1.0)),
                Hybrid::Sp3 => {
                    // Два решения: d0 ± n√(1−|d0|²), d0 в span{v1,v2}
                    let n = normalize(cross(v1, v2));
                    // d0: наименьшие квадраты для d·v1 = d·v2 = −1/3
                    let b = add(scale(v1, -1.0 / 3.0), scale(v2, -1.0 / 3.0));
                    // проекция b на span{v1,v2} через двойное Грама-Шмидта
                    let u1 = v1;
                    let u2 = normalize(sub(v2, scale(u1, dot(v2, u1))));
                    let d0 = add(
                        scale(u1, dot(b, u1)),
                        scale(u2, dot(b, u2)),
                    );
                    let d0 = add(
                        d0,
                        scale(
                            normalize(add(u1, u2)),
                            0.0,
                        ),
                    );
                    let residual = (1.0 - dot(d0, d0)).max(0.0).sqrt();
                    let cand_a = normalize(add(d0, scale(n, residual)));
                    let cand_b = normalize(sub(d0, scale(n, residual)));
                    // Выбираем дальнюю от размещённых атомов
                    let score = |d: [f64; 3]| -> f64 {
                        let p = add(positions[u], d);
                        let mut mn = f64::MAX;
                        for q in positions {
                            if q[0].is_finite() {
                                mn = mn.min(norm(sub(p, *q)));
                            }
                        }
                        mn
                    };
                    if score(cand_a) >= score(cand_b) {
                        cand_a
                    } else {
                        cand_b
                    }
                }
            }
        }
        _ => {
            let mut s = [0.0f64; 3];
            for v in dirs {
                s = add(s, *v);
            }
            if norm(s) < 1e-6 {
                [0.0, 0.0, 1.0]
            } else {
                normalize(scale(s, -1.0))
            }
        }
    }
}

/// Поворот вектора на угол вокруг Z с проецированием (для sp²-вееров).
fn rotate_in_plane(v: [f64; 3], ang: f64) -> [f64; 3] {
    let (s, c) = (ang.sin(), ang.cos());
    normalize([v[0] * c - v[1] * s, v[0] * s + v[1] * c, v[2] * 0.1])
}

/// Детерминированный перпендикуляр.
fn canonical_perp(v: [f64; 3]) -> [f64; 3] {
    let try1 = cross(v, [0.0, 0.0, 1.0]);
    if norm(try1) > 1e-6 {
        normalize(try1)
    } else {
        normalize(cross(v, [1.0, 0.0, 0.0]))
    }
}

// ─── Релаксация ─────────────────────────────────────────────────────────

/// Достигается ли j из i за ≤ max_hops связей (BFS с отсечкой).
fn close_in_graph(i: usize, j: usize, max_hops: usize, adjacency: &[Vec<(usize, f64)>]) -> bool {
    let mut visited = vec![false; adjacency.len()];
    let mut frontier = vec![i];
    visited[i] = true;
    for _ in 0..max_hops {
        let mut next = Vec::new();
        for &u in &frontier {
            for (w, _) in &adjacency[u] {
                if *w == j {
                    return true;
                }
                if !visited[*w] {
                    visited[*w] = true;
                    next.push(*w);
                }
            }
        }
        if next.is_empty() {
            return false;
        }
        frontier = next;
    }
    false
}

fn relax(
    nodes: &[ConfNode],
    bonds: &[ConfBond],
    positions: &mut Vec<[f64; 3]>,
    g: &MoleculeGraph,
    heavy_map: &[usize],
    adjacency: &[Vec<(usize, f64)>],
) -> bool {
    let n = nodes.len();
    if n == 0 {
        return false;
    }
    // Пары 1-3 (для мягких угловых ограничений)
    let mut one_three: Vec<(usize, usize, f64)> = Vec::new();
    for i in 0..n {
        let nbrs: Vec<usize> = bonds
            .iter()
            .filter(|b| b.a == i || b.b == i)
            .map(|b| if b.a == i { b.b } else { b.a })
            .collect();
        for x in 0..nbrs.len() {
            for y in (x + 1)..nbrs.len() {
                let (p, q) = (nbrs[x], nbrs[y]);
                let l1 = bonds
                    .iter()
                    .find(|b| (b.a == i && b.b == p) || (b.b == i && b.a == p))
                    .map(|b| b.target)
                    .unwrap_or(1.5);
                let l2 = bonds
                    .iter()
                    .find(|b| (b.a == i && b.b == q) || (b.b == i && b.a == q))
                    .map(|b| b.target)
                    .unwrap_or(1.5);
                // угол при i: идеал по гибридизации узла i
                let ai = node_atom_idx(heavy_map, i);
                let angle = ai
                    .map(|a| hybridization(g, a).ideal_angle())
                    .unwrap_or(1.9106332362490184);
                let target13 = (l1 * l1 + l2 * l2 - 2.0 * l1 * l2 * angle.cos()).sqrt();
                one_three.push((p, q, target13));
            }
        }
    }

    // Ароматические кольца для уплощения
    let mut arom_rings: Vec<Vec<usize>> = Vec::new();
    for ring in &g.rings {
        if ring.len() <= 6 && ring.iter().all(|&i| g.atoms[i].aromatic) {
            arom_rings.push(ring.iter().map(|&a| heavy_map[a]).collect());
        }
    }

    let iterations = 160;
    for it in 0..iterations {
        let mut delta: Vec<[f64; 3]> = vec![[0.0; 3]; n];

        // Пружины связей
        for b in bonds {
            let d = sub(positions[b.b], positions[b.a]);
            let l = norm(d).max(1e-9);
            let diff = (l - b.target) / l;
            let f = scale(d, 0.25 * diff);
            delta[b.a] = add(delta[b.a], f);
            delta[b.b] = sub(delta[b.b], f);
        }

        // 1-3 мягкие пружины (только если ближе цели — сжатие угла)
        for (p, q, t) in &one_three {
            if *p >= n || *q >= n {
                continue;
            }
            let d = sub(positions[*q], positions[*p]);
            let l = norm(d).max(1e-9);
            if l < *t {
                let diff = (l - *t) / l;
                let f = scale(d, 0.06 * diff);
                delta[*p] = add(delta[*p], f);
                delta[*q] = sub(delta[*q], f);
            }
        }

        // Отталкивание ВдВ (не связанные, не 1-3 и не 1-4):
        // в ароматических кольцах мета-пары C·C сидят на 2.4 Å — это
        // 1-4-связанные пары, которые силовые поля исключают из
        // несвязанного отталкивания (иначе бензол раздувается)
        for i in 0..n {
            for j in (i + 1)..n {
                let bonded = bonds
                    .iter()
                    .any(|b| (b.a == i && b.b == j) || (b.a == j && b.b == i));
                if bonded {
                    continue;
                }
                let is13 = one_three
                    .iter()
                    .any(|(p, q, _)| (*p == i && *q == j) || (*p == j && *q == i));
                if is13 {
                    continue;
                }
                if close_in_graph(i, j, 3, &adjacency) {
                    continue; // 1-4 и ближе
                }
                let d = sub(positions[j], positions[i]);
                let l = norm(d);
                let rmin = 0.72 * (vdw_radius(&nodes[i].symbol) + vdw_radius(&nodes[j].symbol));
                if l < rmin && l > 1e-9 {
                    let push = (rmin - l) / rmin;
                    let f = scale(normalize(d), 0.12 * push);
                    delta[i] = sub(delta[i], f);
                    delta[j] = add(delta[j], f);
                }
            }
        }

        // Уплощение ароматических колец
        if it % 2 == 0 {
            for ring in &arom_rings {
                if ring.len() < 3 {
                    continue;
                }
                let p0 = positions[ring[0]];
                let nrm = normalize(cross(
                    sub(positions[ring[1]], p0),
                    sub(positions[ring[2 % ring.len()]], p0),
                ));
                for &a in ring {
                    let dist = dot(sub(positions[a], p0), nrm);
                    delta[a] = add(delta[a], scale(nrm, -0.1 * dist));
                }
            }
        }

        // Применяем
        let mut max_move = 0.0f64;
        for i in 0..n {
            positions[i] = add(positions[i], delta[i]);
            max_move = max_move.max(norm(delta[i]));
        }
        if it > 20 && max_move < 1e-4 {
            break;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::super::smiles::parse_smiles;
    use super::*;

    #[test]
    fn embed_water_angle() {
        let g = parse_smiles("O").unwrap();
        let c = embed(&g).unwrap();
        assert_eq!(c.nodes.len(), 3);
        let (p1, p2) = (c.positions[1], c.positions[2]);
        let v1 = sub(p1, c.positions[0]);
        let v2 = sub(p2, c.positions[0]);
        let cosang = dot(v1, v2) / (norm(v1) * norm(v2));
        let ang = cosang.acos().to_degrees();
        assert!(ang > 95.0 && ang < 120.0, "угол воды = {ang:.1}°");
        // длины O-H ≈ 0.97 Å
        for i in 1..3 {
            let l = c.dist(0, i);
            assert!((l - 0.97).abs() < 0.15, "O-H = {l:.3}");
        }
    }

    #[test]
    fn embed_methane_tetrahedral() {
        let g = parse_smiles("C").unwrap();
        let c = embed(&g).unwrap();
        assert_eq!(c.nodes.len(), 5);
        let mut angles = Vec::new();
        for i in 1..5 {
            for j in (i + 1)..5 {
                let v1 = sub(c.positions[i], c.positions[0]);
                let v2 = sub(c.positions[j], c.positions[0]);
                let cosang = dot(v1, v2) / (norm(v1) * norm(v2));
                angles.push(cosang.acos().to_degrees());
            }
        }
        let mn = angles.iter().cloned().fold(f64::MAX, f64::min);
        let mx = angles.iter().cloned().fold(f64::MIN, f64::max);
        assert!(mn > 100.0 && mx < 118.0, "H-C-H углы: {mn:.1}..{mx:.1}°");
    }

    #[test]
    fn embed_benzene_planar_ring() {
        let g = parse_smiles("c1ccccc1").unwrap();
        let c = embed(&g).unwrap();
        assert_eq!(c.nodes.len(), 12); // 6 C + 6 H
        // Кольцо планарно
        let rms = c.aromatic_planarity(&g);
        assert!(rms < 0.05, "RMS планарности бензола = {rms:.4}");
        // Длины C-C в кольце: 1.31..1.45
        for b in &c.bonds {
            if b.order == BondOrder::Aromatic {
                let l = c.dist(b.a, b.b);
                assert!(
                    (1.30..1.46).contains(&l),
                    "ароматическая C-C = {l:.3}"
                );
            }
        }
        // Радиус кольца ~1.39 (сторона правильного 6-угольника) — по ТЯЖЁЛЫМ узлам
        let mut r_mean = 0.0;
        for &ni in &c.heavy_map {
            r_mean += norm(c.positions[ni]);
        }
        r_mean /= c.heavy_map.len() as f64;
        assert!((r_mean - 1.39).abs() < 0.15, "радиус кольца = {r_mean:.3}");
    }

    #[test]
    fn embed_aspirin_sane_geometry() {
        let g = parse_smiles("CC(=O)Oc1ccccc1C(=O)O").unwrap();
        let c = embed(&g).unwrap();
        assert_eq!(c.nodes.len(), 21, "9 тяжёлых + 8 H + 4 O-H... = 9+8+... wait");
        // Никаких NaN и разумные габариты
        for p in &c.positions {
            assert!(p[0].is_finite() && p[1].is_finite() && p[2].is_finite());
        }
        let (mn, mx) = c.bbox();
        let size = sub(mx, mn);
        assert!(size[0] < 12.0 && size[1] < 12.0 && size[2] < 8.0, "bbox = {size:?}");
        // Все длины связей в химическом диапазоне
        for b in &c.bonds {
            let l = c.dist(b.a, b.b);
            assert!(l > 0.6 && l < 2.4, "связь = {l:.3}");
        }
        assert!(c.relaxed);
    }

    #[test]
    fn embed_caffeine_fused_rings() {
        let g = parse_smiles("CN1C=NC2=C1C(=O)N(C)C(=O)N2C").unwrap();
        let c = embed(&g).unwrap();
        // 14 тяжёлых + 10 H
        assert_eq!(c.nodes.len(), 24);
        for p in &c.positions {
            assert!(p.iter().all(|x| x.is_finite()));
        }
        for b in &c.bonds {
            let l = c.dist(b.a, b.b);
            assert!(l > 0.7 && l < 2.4, "связь = {l:.3}");
        }
    }

    #[test]
    fn embed_ethanol_rotatable() {
        let g = parse_smiles("CCO").unwrap();
        let c = embed(&g).unwrap();
        assert_eq!(c.nodes.len(), 9);
        // C-C ~1.52, C-O ~1.42 (узлы interleaved: тяжёлые через heavy_map)
        let (n_c0, n_c1, n_o) = (c.heavy_map[0], c.heavy_map[1], c.heavy_map[2]);
        let l_cc = c.dist(n_c0, n_c1);
        let l_co = c.dist(n_c1, n_o);
        assert!((l_cc - 1.52).abs() < 0.2, "C-C = {l_cc:.3}");
        assert!((l_co - 1.42).abs() < 0.2, "C-O = {l_co:.3}");
    }
}
