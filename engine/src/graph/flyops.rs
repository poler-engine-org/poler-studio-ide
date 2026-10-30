//! C2/v0.33.0 «Живая муха»: глубокое взаимодействие с коннектомом FLYCSR1.
//!
//! Надстройка над [`Connectome`] (C1/v0.30.0): восемь операций, которые
//! превращают статичную матрицу A в живой объект допроса для ИИ-агента:
//!
//! - [`Connectome::neighbors`] — полный список партнёров нейрона
//!   (направление/знак/лимит), сортировка по синаптической массе;
//! - [`Connectome::shortest_path`] — кратчайший путь сигнала u→v (BFS
//!   с восстановлением цепочки: каждый прыжок с весом и медиатором);
//! - [`Connectome::common_partners`] — пересечение окрестностей набора
//!   нейронов: общие мишени (divergence) и общие источники (convergence);
//! - [`Connectome::degree_ranking`] — топ нейронов по исходящей/входящей
//!   степени (хабы);
//! - [`Connectome::pagerank`] — взвешенный PageRank по |A| с фильтром
//!   знака (модуляторы не проводят сигнал и в ранг не входят);
//! - [`Connectome::rotor_top`] — глобальный топ пар по циркуляции
//!   J = A − Aᵀ: самые однонаправленные влияния мозга;
//! - [`Connectome::motif_census`] — мотивы вокруг нейрона: реципрокные
//!   пары, feedforward-треугольники (u→v→w + u→w) и feedback-циклы
//!   (u→v→w→u);
//! - [`Connectome::propagate`] — симуляция распространения сигнала:
//!   x(t+1) = leak·x(t) + γ·A·x(t) на знаковых весах (торможение гасит,
//!   возбуждение разгоняет) — муха «думает» в RAM.
//!
//! Все операции работают через публичный API CSR/CSC (срезы строк —
//! сплошная память), без копий графа. Тяжёлые проходы (rotor_top,
//! pagerank, propagate) — O(m) на итерацию с ранним отсечением.

use std::collections::{BinaryHeap, VecDeque};

use super::connectome::{Connectome, Edge, InEdges, SignFilter};

/// Направление обзора: исходящие (мишени) или входящие (источники).
/// Для `common_partners` семантика: Out = общие мишени (вниз по потоку),
/// In = общие источники (вверх по потоку).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Исходящие рёбра / общие мишени.
    Out,
    /// Входящие рёбра / общие источники.
    In,
}

impl Direction {
    /// Парсинг CLI/MCP-значения (принимает синонимы вверх/вниз потока).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "out" | "исх" | "down" | "вниз" => Ok(Self::Out),
            "in" | "вх" | "up" | "вверх" => Ok(Self::In),
            _ => Err(format!(
                "неизвестное направление «{s}» (ожидалось out | in; для common: down | up)"
            )),
        }
    }
}

/// Знак, пропускаемый фильтром (None — все связи).
fn wanted_sign(filter: SignFilter) -> Option<i32> {
    match filter {
        SignFilter::All => None,
        SignFilter::Excitatory => Some(1),
        SignFilter::Inhibitory => Some(-1),
    }
}

/// Пропускает ли ребро фильтр знака.
#[inline]
fn passes(e: &Edge, want: Option<i32>) -> bool {
    match want {
        None => true,
        Some(s) => e.sign() == s,
    }
}

// ══════════════════════════════════════════════════════════════════
// Соседи
// ══════════════════════════════════════════════════════════════════

impl Connectome {
    /// Партнёры нейрона u по направлению: мишени (Out) или источники (In).
    /// Сортировка по весу (синаптической массе) по убыванию, затем по
    /// индексу партнёра. `limit` усекает список (0 — без усечения).
    /// None — нейрон вне диапазона.
    pub fn neighbors(
        &self,
        u: usize,
        dir: Direction,
        filter: SignFilter,
        limit: usize,
        csc: &InEdges,
    ) -> Option<Vec<Edge>> {
        let want = wanted_sign(filter);
        let mut list: Vec<Edge> = match dir {
            Direction::Out => self.out_edges(u)?.filter(|e| passes(e, want)).collect(),
            Direction::In => csc.in_edges(self, u)?.filter(|e| passes(e, want)).collect(),
        };
        list.sort_by(|a, b| b.weight.cmp(&a.weight).then(a.target.cmp(&b.target)));
        if limit > 0 {
            list.truncate(limit);
        }
        Some(list)
    }

    // ══════════════════════════════════════════════════════════════
    // Кратчайший путь сигнала
    // ══════════════════════════════════════════════════════════════

    /// Кратчайший путь сигнала from→to: BFS по исходящим рёбрам с
    /// фильтром знака. Возвращает цепочку прыжков (каждый — с весом и
    /// медиатором) либо `found: false`. from == to — пустой путь.
    /// Сложность O(n + m); 138 639 узлов обходятся за миллисекунды.
    pub fn shortest_path(&self, from: usize, to: usize, filter: SignFilter) -> PathReport {
        if from >= self.n_nodes() || to >= self.n_nodes() {
            return PathReport { found: false, hops: Vec::new() };
        }
        if from == to {
            return PathReport { found: true, hops: Vec::new() };
        }
        let want = wanted_sign(filter);
        let none = u32::MAX;
        // (родитель, позиция ребра в CSR-массивах)
        let mut parent: Vec<(u32, u32)> = vec![(none, none); self.n_nodes()];
        let mut visited = vec![false; self.n_nodes()];
        visited[from] = true;
        let mut queue: VecDeque<u32> = VecDeque::new();
        queue.push_back(from as u32);
        let mut reached = false;
        'bfs: while let Some(u) = queue.pop_front() {
            let Some(edges) = self.out_edges(u as usize) else { break };
            for e in edges {
                if !passes(&e, want) || visited[e.target as usize] {
                    continue;
                }
                let v = e.target as usize;
                visited[v] = true;
                parent[v] = (u, e.idx as u32);
                if v == to {
                    reached = true;
                    break 'bfs;
                }
                queue.push_back(e.target);
            }
        }
        if !reached {
            return PathReport { found: false, hops: Vec::new() };
        }
        // Восстановление цепочки от to к from.
        let mut hops_rev: Vec<PathHop> = Vec::new();
        let mut cur = to;
        while cur != from {
            let (pu, ei) = parent[cur];
            debug_assert!(pu != none, "посещённый без родителя");
            let (pu, ei) = (pu as usize, ei as usize);
            let e = Edge {
                idx: ei,
                target: cur as u32,
                weight: self_edge_weight(self, ei),
                nt: self_edge_nt(self, ei),
            };
            hops_rev.push(PathHop { from: pu, to: cur, edge: e });
            cur = pu;
        }
        hops_rev.reverse();
        PathReport { found: true, hops: hops_rev }
    }

    // ══════════════════════════════════════════════════════════════
    // Общие партнёры набора
    // ══════════════════════════════════════════════════════════════

    /// Общие партнёры набора нейронов: пересечение окрестностей.
    /// Out — общие мишени (нейроны, на которые влияет весь набор),
    /// In — общие источники (нейроны, влияющие на весь набор).
    /// Результат сортирован по суммарной массе связей ко всем узлам
    /// набора. Пустой набор/вне диапазона — ошибка (агенту нужен явный
    /// отказ, а не пустышка).
    pub fn common_partners(
        &self,
        nodes: &[usize],
        dir: Direction,
        filter: SignFilter,
        csc: &InEdges,
    ) -> Result<Vec<CommonPartner>, String> {
        if nodes.is_empty() {
            return Err("набор нейронов пуст".to_string());
        }
        if nodes.len() > 32 {
            return Err(format!("набор из {} нейронов слишком велик (максимум 32)", nodes.len()));
        }
        let want = wanted_sign(filter);
        for &u in nodes {
            if u >= self.n_nodes() {
                return Err(format!("нейрон {u} вне диапазона (узлов: {})", self.n_nodes()));
            }
        }
        let mut nodes = nodes.to_vec();
        nodes.sort_unstable();
        nodes.dedup();

        // Список (партнёр, рёбра от каждого узла набора), отсортирован
        // по партнёру — пересечение идёт как слияние двух сортированных.
        let partners_of = |u: usize| -> Vec<(u32, Edge)> {
            let base: Vec<Edge> = match dir {
                Direction::Out => self.out_edges(u).map(|it| it.collect()).unwrap_or_default(),
                Direction::In => csc.in_edges(self, u).map(|it| it.collect()).unwrap_or_default(),
            };
            let mut out: Vec<(u32, Edge)> =
                base.into_iter().filter(|e| passes(e, want)).map(|e| (e.target, e)).collect();
            out.sort_by_key(|(p, _)| *p);
            out
        };

        let mut acc: Vec<(u32, Vec<(usize, Edge)>)> = partners_of(nodes[0])
            .into_iter()
            .map(|(p, e)| (p, vec![(nodes[0], e)]))
            .collect();
        for &u in &nodes[1..] {
            let cur = partners_of(u);
            let mut next: Vec<(u32, Vec<(usize, Edge)>)> = Vec::with_capacity(acc.len().min(cur.len()));
            let (mut i, mut j) = (0usize, 0usize);
            while i < acc.len() && j < cur.len() {
                match acc[i].0.cmp(&cur[j].0) {
                    std::cmp::Ordering::Less => i += 1,
                    std::cmp::Ordering::Greater => j += 1,
                    std::cmp::Ordering::Equal => {
                        let mut members = std::mem::take(&mut acc[i].1);
                        members.push((u, cur[j].1));
                        next.push((acc[i].0, members));
                        i += 1;
                        j += 1;
                    }
                }
            }
            acc = next;
        }
        let mut out: Vec<CommonPartner> = acc
            .into_iter()
            .map(|(p, members)| CommonPartner {
                node: p as usize,
                total_weight: members.iter().map(|(_, e)| e.weight as u64).sum(),
                members,
            })
            .collect();
        out.sort_by(|a, b| {
            b.total_weight.cmp(&a.total_weight).then(a.node.cmp(&b.node))
        });
        Ok(out)
    }

    // ══════════════════════════════════════════════════════════════
    // Хабы: топ степеней
    // ══════════════════════════════════════════════════════════════

    /// Топ нейронов по степени: Out — исходящей (куда раздают сигнал),
    /// In — входящей (кого бомбардируют). Связки по степени разрывает
    /// меньший индекс — результат детерминирован.
    pub fn degree_ranking(
        &self,
        dir: Direction,
        top: usize,
        csc: &InEdges,
    ) -> Vec<(usize, u32)> {
        let mut ranking: Vec<(usize, u32)> = (0..self.n_nodes())
            .filter_map(|v| match dir {
                Direction::Out => self.out_degree(v).map(|d| (v, d)),
                Direction::In => csc.in_degree(v).map(|d| (v, d)),
            })
            .collect();
        ranking.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        ranking.truncate(top);
        ranking
    }

    // ══════════════════════════════════════════════════════════════
    // PageRank
    // ══════════════════════════════════════════════════════════════

    /// Взвешенный PageRank по |signed_weight| (модуляторы — вес 0 —
    /// не проводят сигнал и в ранг не входят; фильтр знака сужает граф
    /// до возбуждающих/тормозных путей). Классическая итерация степени
    /// с равномерной раздачей массы висячих узлов; критерий остановки —
    /// L1-норма изменения < tol или max_iters. Σ rank = 1.
    pub fn pagerank(
        &self,
        filter: SignFilter,
        damping: f64,
        max_iters: usize,
        tol: f64,
        top: usize,
    ) -> PageRankReport {
        let n = self.n_nodes();
        if n == 0 {
            return PageRankReport { top: Vec::new(), iterations: 0, converged: true };
        }
        let want = wanted_sign(filter);
        // Исходящая сигнальная масса каждого узла (после фильтра).
        let mut out_sum: Vec<f64> = vec![0.0; n];
        let mut any_edge = false;
        for u in 0..n {
            if let Some(edges) = self.out_edges(u) {
                for e in edges {
                    if !passes(&e, want) {
                        continue;
                    }
                    let w = e.signed_weight().unsigned_abs() as f64;
                    if w > 0.0 {
                        out_sum[u] += w;
                        any_edge = true;
                    }
                }
            }
        }
        if !any_edge {
            return PageRankReport { top: Vec::new(), iterations: 0, converged: true };
        }
        let damping = damping.clamp(0.05, 0.99);
        let mut rank: Vec<f64> = vec![1.0 / n as f64; n];
        let mut next: Vec<f64> = vec![0.0; n];
        let mut iterations = 0usize;
        let mut converged = false;
        for it in 0..max_iters.max(1) {
            iterations = it + 1;
            next.iter_mut().for_each(|v| *v = 0.0);
            let mut dangling = 0.0f64;
            for u in 0..n {
                let ru = rank[u];
                if ru == 0.0 {
                    continue;
                }
                if out_sum[u] == 0.0 {
                    dangling += ru;
                    continue;
                }
                if let Some(edges) = self.out_edges(u) {
                    for e in edges {
                        if !passes(&e, want) {
                            continue;
                        }
                        let w = e.signed_weight().unsigned_abs() as f64;
                        if w > 0.0 {
                            next[e.target as usize] += ru * w / out_sum[u];
                        }
                    }
                }
            }
            let base = (1.0 - damping) / n as f64;
            let even = damping * dangling / n as f64;
            let mut l1 = 0.0f64;
            for v in 0..n {
                let nv = base + damping * next[v] + even;
                l1 += (nv - rank[v]).abs();
                rank[v] = nv;
            }
            if l1 < tol {
                converged = true;
                break;
            }
        }
        let mut idx: Vec<usize> = (0..n).filter(|&v| rank[v] > 0.0).collect();
        idx.sort_by(|&a, &b| rank[b].total_cmp(&rank[a]).then(a.cmp(&b)));
        idx.truncate(top);
        PageRankReport {
            top: idx.into_iter().map(|v| (v, rank[v])).collect(),
            iterations,
            converged,
        }
    }

    // ══════════════════════════════════════════════════════════════
    // Ротор: глобальный топ циркуляции
    // ══════════════════════════════════════════════════════════════

    /// Топ-K пар по циркуляции J = A − Aᵀ (только J > 0 — антисимметрия
    /// даёт вторую половину бесплатно). Это самые однонаправленные
    /// влияния мозга: чистые потоки без встречной компенсации.
    /// `min_abs` отсекает слабые пары (по умолчанию 1). Проход по всем
    /// рёбрам с бинарным поиском обратного; дедупликация — ориентация
    /// всегда «положительный J первым».
    pub fn rotor_top(&self, k: usize, min_abs: i32) -> Vec<RotorPair> {
        if k == 0 || min_abs < 1 {
            return Vec::new();
        }
        let min_abs = min_abs.max(1);
        // Мин-куча размера k: (j, u, v) — выталкивает наименьший j.
        let mut heap: BinaryHeap<std::cmp::Reverse<(i32, u32, u32)>> = BinaryHeap::new();
        for u in 0..self.n_nodes() {
            let Some(edges) = self.out_edges(u) else { continue };
            for e in edges {
                if e.sign() == 0 {
                    continue; // модуляторы не входят в J
                }
                let fwd = e.signed_weight();
                let bwd = self.signed_weight(e.target as usize, u);
                let j = fwd - bwd;
                if j.abs() < min_abs {
                    continue;
                }
                // Дедупликация пары: при двунаправленной связи (bwd ≠ 0)
                // пара сканируется с обеих сторон — эмитим только сторону
                // с J > 0 (обратная даст J < 0 и молчит). Односторонняя
                // связь (bwd == 0) сканируется один раз — нормализуем
                // ориентацию «положительный J первым».
                let (pu, pv, pj) = if j > 0 {
                    (u, e.target as usize, j)
                } else if bwd == 0 {
                    (e.target as usize, u, -j)
                } else {
                    continue;
                };
                let entry = std::cmp::Reverse((pj, pu as u32, pv as u32));
                if heap.len() < k {
                    heap.push(entry);
                } else if entry.0 .0 > heap.peek().map(|r| r.0 .0).unwrap_or(i32::MIN) {
                    heap.pop();
                    heap.push(entry);
                }
            }
        }
        let mut out: Vec<RotorPair> = heap
            .into_sorted_vec()
            .into_iter()
            .map(|std::cmp::Reverse((j, u, v))| RotorPair {
                u: u as usize,
                v: v as usize,
                j,
                forward: nonzero(self.signed_weight(u as usize, v as usize)),
                backward: nonzero(self.signed_weight(v as usize, u as usize)),
            })
            .collect();
        out.sort_by(|a, b| b.j.cmp(&a.j).then(a.u.cmp(&b.u)).then(a.v.cmp(&b.v)));
        out
    }

    // ══════════════════════════════════════════════════════════════
    // Мотивы вокруг нейрона
    // ══════════════════════════════════════════════════════════════

    /// Перепись мотивов вокруг нейрона u:
    /// - реципрокные пары: u⇄v (оба ребра с весами);
    /// - feedforward-треугольники: u→v→w плюс прямое u→w (последователь
    ///   с обходным путём — базовая схема усиления/модуляции);
    /// - feedback-циклы длины 3: u→v→w→u (кольцо обратной связи).
    /// `examples` ограничивает список примеров (сами счётчики полные).
    pub fn motif_census(&self, u: usize, csc: &InEdges, examples: usize) -> Option<MotifReport> {
        if u >= self.n_nodes() {
            return None;
        }
        let outs: Vec<Edge> = self.out_edges(u).map(|it| it.collect()).unwrap_or_default();
        let ins: Vec<Edge> = csc.in_edges(self, u).map(|it| it.collect()).unwrap_or_default();

        // Членство в окрестностях — O(1) проверка.
        let mut member_out = vec![false; self.n_nodes()];
        for e in &outs {
            member_out[e.target as usize] = true;
        }
        let mut member_in = vec![false; self.n_nodes()];
        for e in &ins {
            member_in[e.target as usize] = true;
        }

        // Реципрокные пары.
        let mut reciprocal: Vec<(Edge, Edge)> = Vec::new();
        for e in &outs {
            if e.target as usize == u {
                continue; // аутапс не пара
            }
            if let Some(back) = self.edge(e.target as usize, u) {
                reciprocal.push((*e, back));
            }
        }

        // Треугольники: проход по строкам исходящих соседей.
        let mut feedforward = 0usize;
        let mut feedback3 = 0usize;
        let mut ff_examples: Vec<(usize, usize)> = Vec::new();
        for e in &outs {
            let v = e.target as usize;
            if v == u {
                continue;
            }
            if let Some(vrow) = self.out_edges(v) {
                for w in vrow {
                    let w = w.target as usize;
                    if w == u || w == v {
                        continue;
                    }
                    if member_out[w] {
                        feedforward += 1;
                        if ff_examples.len() < examples {
                            ff_examples.push((v, w));
                        }
                    }
                    if member_in[w] {
                        feedback3 += 1;
                    }
                }
            }
        }

        Some(MotifReport {
            node: u,
            out_degree: outs.len(),
            in_degree: ins.len(),
            reciprocal,
            feedforward,
            feedback3,
            ff_examples,
        })
    }

    // ══════════════════════════════════════════════════════════════
    // Симуляция распространения сигнала
    // ══════════════════════════════════════════════════════════════

    /// Живая динамика: x(t+1) = leak·x(t) + γ·A·x(t) на знаковых весах
    /// (gaba с минусом гасит цель, ach/glut с плюсом разгоняют,
    /// модуляторы — вес 0 — не проводят). Семена стартуют с x = +1.
    /// Отчёт: количество активных (|x| > theta) и баланс масс на каждом
    /// шаге + топ возбуждённых нейронов в конце. Значения мягко
    /// ограничены ±1e15 (защита от расходящегося кольца).
    pub fn propagate(
        &self,
        seeds: &[usize],
        steps: usize,
        gamma: f64,
        leak: f64,
        filter: SignFilter,
        top: usize,
        theta: f64,
    ) -> Result<PropagateReport, String> {
        if seeds.is_empty() {
            return Err("семена пусты: укажите хотя бы один нейрон".to_string());
        }
        if seeds.len() > 64 {
            return Err(format!("{} семян — максимум 64", seeds.len()));
        }
        for &s in seeds {
            if s >= self.n_nodes() {
                return Err(format!("нейрон {s} вне диапазона (узлов: {})", self.n_nodes()));
            }
        }
        let steps = steps.clamp(1, 64);
        let gamma = gamma.clamp(0.0, 10.0);
        let leak = leak.clamp(0.0, 1.0);
        let theta = if theta > 0.0 { theta } else { 0.01 };
        let want = wanted_sign(filter);

        let n = self.n_nodes();
        let mut x: Vec<f64> = vec![0.0; n];
        let mut dedup_seeds: Vec<usize> = seeds.to_vec();
        dedup_seeds.sort_unstable();
        dedup_seeds.dedup();
        for &s in &dedup_seeds {
            x[s] = 1.0;
        }
        let mut acc: Vec<f64> = vec![0.0; n];
        let mut timeline: Vec<PropagateStep> = Vec::with_capacity(steps);
        for step in 1..=steps {
            acc.iter_mut().for_each(|v| *v = 0.0);
            for u in 0..n {
                if x[u].abs() < 1e-12 {
                    continue;
                }
                if let Some(edges) = self.out_edges(u) {
                    for e in edges {
                        if !passes(&e, want) {
                            continue;
                        }
                        let sw = e.signed_weight();
                        if sw != 0 {
                            acc[e.target as usize] += x[u] * sw as f64;
                        }
                    }
                }
            }
            let mut active = 0usize;
            let mut positive_mass = 0.0f64;
            let mut negative_mass = 0.0f64;
            for v in 0..n {
                let nv = (leak * x[v] + gamma * acc[v]).clamp(-1e15, 1e15);
                x[v] = nv;
                if nv.abs() > theta {
                    active += 1;
                    if nv > 0.0 {
                        positive_mass += nv;
                    } else {
                        negative_mass += nv;
                    }
                }
            }
            timeline.push(PropagateStep { step, active, positive_mass, negative_mass });
        }
        let mut idx: Vec<usize> = (0..n).filter(|&v| x[v].abs() > theta).collect();
        idx.sort_by(|&a, &b| x[b].abs().total_cmp(&x[a].abs()).then(a.cmp(&b)));
        idx.truncate(top);
        Ok(PropagateReport {
            seeds: dedup_seeds,
            steps: timeline,
            top: idx.into_iter().map(|v| (v, x[v])).collect(),
        })
    }
}

/// Вес ребра по позиции в CSR-массивах (восстановление пути).
#[inline]
fn self_edge_weight(con: &Connectome, idx: usize) -> u16 {
    con.row_edge_weight(idx)
}

/// Медиатор ребра по позиции в CSR-массивах.
#[inline]
fn self_edge_nt(con: &Connectome, idx: usize) -> u8 {
    con.row_edge_nt(idx)
}

/// Some(x), если x ≠ 0 (для ротора: отсутствие связи = 0 = Null).
#[inline]
fn nonzero(x: i32) -> Option<i32> {
    if x == 0 { None } else { Some(x) }
}

/// Прыжок пути: откуда, куда и каким ребром (вес/медиатор/знак).
#[derive(Clone, Debug)]
pub struct PathHop {
    /// Источник прыжка.
    pub from: usize,
    /// Мишень прыжка.
    pub to: usize,
    /// Ребро CSR (target == to).
    pub edge: Edge,
}

/// Результат поиска кратчайшего пути.
#[derive(Clone, Debug)]
pub struct PathReport {
    /// Найден ли путь.
    pub found: bool,
    /// Цепочка прыжков (пустая при found && from == to).
    pub hops: Vec<PathHop>,
}

impl PathReport {
    /// Длина пути в прыжках.
    pub fn length(&self) -> usize {
        self.hops.len()
    }

    /// Суммарная синаптическая масса пути.
    pub fn total_weight(&self) -> u64 {
        self.hops.iter().map(|h| h.edge.weight as u64).sum()
    }
}

/// Общий партнёр набора нейронов.
#[derive(Clone, Debug)]
pub struct CommonPartner {
    /// Индекс общего партнёра.
    pub node: usize,
    /// Суммарная масса его связей со всеми узлами набора.
    pub total_weight: u64,
    /// (узел набора, ребро) для каждого участника.
    pub members: Vec<(usize, Edge)>,
}

/// Результат PageRank.
#[derive(Clone, Debug)]
pub struct PageRankReport {
    /// Топ узлов: (индекс, ранг). Σ всех рангов = 1.
    pub top: Vec<(usize, f64)>,
    /// Выполнено итераций.
    pub iterations: usize,
    /// Сошёлся ли по L1 < tol.
    pub converged: bool,
}

/// Пара с максимальной циркуляцией J = A − Aᵀ (J > 0).
#[derive(Clone, Debug)]
pub struct RotorPair {
    /// Источник чистого потока (J[u][v] > 0).
    pub u: usize,
    /// Мишень чистого потока.
    pub v: usize,
    /// J[u][v] = A(u,v) − A(v,u) > 0.
    pub j: i32,
    /// Знаковый вес прямого направления (None — связи нет).
    pub forward: Option<i32>,
    /// Знаковый вес обратного направления (None — связи нет).
    pub backward: Option<i32>,
}

/// Перепись мотивов вокруг нейрона.
#[derive(Clone, Debug)]
pub struct MotifReport {
    /// Центральный нейрон.
    pub node: usize,
    /// Исходящих рёбер.
    pub out_degree: usize,
    /// Входящих рёбер.
    pub in_degree: usize,
    /// Реципрокные пары u⇄v: (прямое, обратное ребро).
    pub reciprocal: Vec<(Edge, Edge)>,
    /// Feedforward-треугольники u→v→w + u→w.
    pub feedforward: usize,
    /// Feedback-циклы u→v→w→u.
    pub feedback3: usize,
    /// Примеры (v, w) feedforward-треугольников.
    pub ff_examples: Vec<(usize, usize)>,
}

/// Один шаг симуляции распространения.
#[derive(Clone, Copy, Debug)]
pub struct PropagateStep {
    /// Номер шага (1..=steps).
    pub step: usize,
    /// Активных нейронов (|x| > theta).
    pub active: usize,
    /// Суммарная положительная масса (возбуждение).
    pub positive_mass: f64,
    /// Суммарная отрицательная масса (торможение).
    pub negative_mass: f64,
}

/// Результат симуляции распространения сигнала.
#[derive(Clone, Debug)]
pub struct PropagateReport {
    /// Семена (дедуплицированы).
    pub seeds: Vec<usize>,
    /// Хронология шагов.
    pub steps: Vec<PropagateStep>,
    /// Топ нейронов по |x| в конце: (индекс, потенциал).
    pub top: Vec<(usize, f64)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::connectome::test_support::synth;

    // Граф: 0→1 (ach w10), 0→2 (gaba w5), 1→3 (glut w3),
    //       2→3 (ach w7), 3→1 (gaba w9), 1→2 (da w4 — модулятор).
    fn con() -> Connectome {
        Connectome::from_raw(&synth(
            false,
            &[(0, 1, 10, 1), (0, 2, 5, 0), (1, 3, 3, 2), (2, 3, 7, 1), (3, 1, 9, 0), (1, 2, 4, 5)],
        ))
        .unwrap()
    }

    fn csc(c: &Connectome) -> InEdges {
        c.build_in_edges()
    }

    // 1. Соседи: направление, сортировка по весу, лимит, фильтр знака.
    #[test]
    fn neighbors_direction_sort_limit() {
        let c = con();
        let cs = csc(&c);
        // исходящие 0: мишени 1 (w10) и 2 (w5) — по убыванию веса
        let out = c.neighbors(0, Direction::Out, SignFilter::All, 0, &cs).unwrap();
        assert_eq!(out.iter().map(|e| e.target).collect::<Vec<_>>(), vec![1, 2]);
        // входящие 3: источники 1 (w3), 2 (w7) — по убыванию веса
        let inc = c.neighbors(3, Direction::In, SignFilter::All, 0, &cs).unwrap();
        assert_eq!(inc.iter().map(|e| e.target).collect::<Vec<_>>(), vec![2, 1]);
        // лимит 1 — только сильнейший
        let lim = c.neighbors(3, Direction::In, SignFilter::All, 1, &cs).unwrap();
        assert_eq!(lim.len(), 1);
        assert_eq!(lim[0].target, 2);
        // фильтр: только тормозные исходящие 0 → одно gaba-ребро в 2
        let inh = c.neighbors(0, Direction::Out, SignFilter::Inhibitory, 0, &cs).unwrap();
        assert_eq!(inh.len(), 1);
        assert_eq!((inh[0].target, inh[0].weight), (2, 5));
        // вне диапазона — None
        assert!(c.neighbors(99, Direction::Out, SignFilter::All, 0, &cs).is_none());
    }

    // 2. Кратчайший путь: цепочка, массы, фильтр, недостижимость.
    #[test]
    fn shortest_path_chain_and_filters() {
        let c = con();
        // 0→3: два пути длины 2 (через 1 и через 2), BFS — через 1 (первый)
        let r = c.shortest_path(0, 3, SignFilter::All);
        assert!(r.found);
        assert_eq!(r.length(), 2);
        assert_eq!(r.hops[0].to, 1);
        assert_eq!(r.hops[1].from, 1);
        assert_eq!(r.hops[1].to, 3);
        assert_eq!(r.total_weight(), 10 + 3);
        // прямой прыжок
        let r2 = c.shortest_path(0, 1, SignFilter::All);
        assert_eq!(r2.length(), 1);
        assert_eq!(r2.hops[0].edge.weight, 10);
        // from == to — пустой путь
        let r3 = c.shortest_path(2, 2, SignFilter::All);
        assert!(r3.found);
        assert_eq!(r3.length(), 0);
        // фильтр возбуждающих: 0→{1,2}; 1→3 glut; 2→3 ach — оба хопа валидны
        let r4 = c.shortest_path(0, 3, SignFilter::Excitatory);
        assert!(r4.found);
        // фильтр тормозных: из 0 только gaba в 2, из 2 нет gaba-исходящих
        let r5 = c.shortest_path(0, 3, SignFilter::Inhibitory);
        assert!(!r5.found);
        // недостижимо: в 0 никто не входит
        assert!(!c.shortest_path(3, 0, SignFilter::All).found);
        // вне диапазона
        assert!(!c.shortest_path(0, 99, SignFilter::All).found);
        assert!(!c.shortest_path(99, 0, SignFilter::All).found);
    }

    // 3. Общие партнёры: мишени и источники, пустое пересечение, ошибки.
    #[test]
    fn common_partners_both_directions() {
        let c = con();
        let cs = csc(&c);
        // общие мишени {0, 2}: 0→{1,2}, 2→{3} → пусто
        let down = c.common_partners(&[0, 2], Direction::Out, SignFilter::All, &cs).unwrap();
        assert!(down.is_empty());
        // общие мишени {0, 1}: 0→{1,2}, 1→{2,3} → {2}
        let down2 = c.common_partners(&[0, 1], Direction::Out, SignFilter::All, &cs).unwrap();
        assert_eq!(down2.len(), 1);
        assert_eq!(down2[0].node, 2);
        // масса: 0→2 gaba w5 + 1→2 da w4 (модулятор, но фильтр All — рёбра есть)
        assert_eq!(down2[0].total_weight, 9);
        assert_eq!(down2[0].members.len(), 2);
        // общие источники {1, 3}: в 1 входят {0, 3}, в 3 входят {1, 2} → пусто
        let up = c.common_partners(&[1, 3], Direction::In, SignFilter::All, &cs).unwrap();
        assert!(up.is_empty());
        // общие источники {2, 3}: в 2 входят {0, 1}, в 3 входят {1, 2} → {1}
        let up2 = c.common_partners(&[2, 3], Direction::In, SignFilter::All, &cs).unwrap();
        assert_eq!(up2.len(), 1);
        assert_eq!(up2[0].node, 1);
        // один нейрон — все его партнёры тривиально «общие»
        let single = c.common_partners(&[0], Direction::Out, SignFilter::All, &cs).unwrap();
        assert_eq!(single.len(), 2);
        // ошибки: пусто, вне диапазона, слишком много
        assert!(c.common_partners(&[], Direction::Out, SignFilter::All, &cs).is_err());
        assert!(c.common_partners(&[0, 99], Direction::Out, SignFilter::All, &cs).is_err());
        let many: Vec<usize> = (0..33).collect();
        assert!(c.common_partners(&many, Direction::Out, SignFilter::All, &cs).is_err());
    }

    // 4. Топ степеней: сортировка, детерминизм, направления.
    #[test]
    fn degree_ranking_sorted() {
        let c = con();
        let cs = csc(&c);
        // out: 0→2, 1→2, 2→1, 3→1 (связки по меньшему индексу)
        let out = c.degree_ranking(Direction::Out, 4, &cs);
        assert_eq!(out[0], (0, 2));
        assert_eq!(out[1], (1, 2));
        // in: 1←{0,3}, 2←{0,1}, 3←{1,2}
        let inc = c.degree_ranking(Direction::In, 3, &cs);
        assert_eq!(inc[0], (1, 2));
        assert_eq!(inc[1], (2, 2));
        assert_eq!(inc[2], (3, 2));
        // усечение
        assert_eq!(c.degree_ranking(Direction::Out, 1, &cs).len(), 1);
    }

    // 5. PageRank: звезда с обратной связью, фильтры, сходимость.
    #[test]
    fn pagerank_star_exact() {
        // 0 → {1,2,3} равными ach-весами w5 + endorsement 1→0:
        // центр получает ПОЛНУЮ массу нейрона 1 и обязан ранжироваться
        // выше безответных листьев 2, 3.
        let c = Connectome::from_raw(&synth(
            false,
            &[(0, 1, 5, 1), (0, 2, 5, 1), (0, 3, 5, 1), (1, 0, 5, 1)],
        ))
        .unwrap();
        let r = c.pagerank(SignFilter::All, 0.85, 50, 1e-12, 10);
        assert!(r.converged);
        assert_eq!(r.top.len(), 4);
        let rank_of = |v: usize| r.top.iter().find(|&&(n, _)| n == v).unwrap().1;
        let center = rank_of(0);
        let leaf = rank_of(2);
        assert!(center > leaf, "центр {center} должен ранжироваться выше листа {leaf}");
        // аналитика фиксированной точки: r0 ≈ 0.325, r_leaf ≈ 0.225
        assert!((center - 0.325).abs() < 0.005, "r0 = {center}");
        assert!((leaf - 0.225).abs() < 0.005, "r_leaf = {leaf}");
        // Σ = 1
        let sum: f64 = r.top.iter().map(|(_, p)| p).sum();
        assert!((sum - 1.0).abs() < 1e-9, "Σ рангов = {sum}");
        // фильтр тормозных на графе без gaba — пустой ранг
        let r2 = c.pagerank(SignFilter::Inhibitory, 0.85, 10, 1e-9, 10);
        assert!(r2.top.is_empty());
        assert!(r2.converged);
        // модуляторные рёбра не проводят ранг
        let c2 = Connectome::from_raw(&synth(false, &[(0, 1, 100, 5)])).unwrap();
        assert!(c2.pagerank(SignFilter::All, 0.85, 10, 1e-9, 10).top.is_empty());
    }

    // 6. Ротор-топ: ориентация, дедуп, минимальный порог.
    #[test]
    fn rotor_top_orientation_and_dedup() {
        let c = con();
        // сильнейшая циркуляция — пара 1⇄3: J(1,3) = A(1,3) − A(3,1) = 3 − (−9) = 12
        let top = c.rotor_top(10, 1);
        assert!(!top.is_empty());
        assert_eq!(top[0].u, 1);
        assert_eq!(top[0].v, 3);
        assert_eq!(top[0].j, 12);
        assert_eq!(top[0].forward, Some(3));
        assert_eq!(top[0].backward, Some(-9));
        // каждая пара ровно один раз, все j > 0, убывание
        for w in top.windows(2) {
            assert!(w[0].j >= w[1].j);
        }
        assert!(top.iter().all(|p| p.j > 0));
        let mut pairs: Vec<(usize, usize)> = top.iter().map(|p| (p.u, p.v)).collect();
        pairs.sort_unstable();
        pairs.dedup();
        assert_eq!(pairs.len(), top.len(), "дубликаты пар в rotor_top");
        // порог отсекает слабые
        let strong = c.rotor_top(10, 6);
        assert!(strong.iter().all(|p| p.j >= 6));
        // k = 0 — пусто
        assert!(c.rotor_top(0, 1).is_empty());
    }

    // 7. Мотивы: реципрокная пара, feedforward, feedback.
    #[test]
    fn motif_census_reciprocal_ff_fb() {
        // строим: 0⇄1 (реципрокная), 0→2, 1→2 (feedforward 0→1→2 + 0→2),
        // 2→0 (feedback 0→1→2→0), 3 — изолирован от 0
        let c = Connectome::from_raw(&synth(
            false,
            &[(0, 1, 6, 1), (1, 0, 4, 0), (0, 2, 8, 1), (1, 2, 5, 2), (2, 0, 7, 1), (2, 3, 3, 1)],
        ))
        .unwrap();
        let cs = csc(&c);
        let m = c.motif_census(0, &cs, 10).unwrap();
        assert_eq!(m.node, 0);
        assert_eq!(m.out_degree, 2);
        assert_eq!(m.in_degree, 2);
        // реципрокные пары 0⇄1 и 0⇄2 (в порядке исходящих)
        assert_eq!(m.reciprocal.len(), 2);
        assert_eq!((m.reciprocal[0].0.weight, m.reciprocal[0].1.weight), (6, 4));
        assert_eq!((m.reciprocal[1].0.weight, m.reciprocal[1].1.weight), (8, 7));
        // feedforward: 0→1→2 + 0→2 = 1 треугольник
        assert_eq!(m.feedforward, 1);
        assert_eq!(m.ff_examples, vec![(1, 2)]);
        // feedback: 0→1→2→0 = 1 цикл
        assert_eq!(m.feedback3, 1);
        // вне диапазона
        assert!(c.motif_census(99, &cs, 10).is_none());
    }

    // 8. Симуляция: затухание без циклов, торможение гасит, ошибки.
    #[test]
    fn propagate_dynamics_and_guards() {
        let c = con();
        // из 0 сигнал уходит в 1 и 2; возбуждение 1 (+10γ), торможение 2 (−5γ)
        let r = c
            .propagate(&[0], 3, 0.1, 0.5, SignFilter::All, 10, 0.01)
            .unwrap();
        assert_eq!(r.seeds, vec![0]);
        assert_eq!(r.steps.len(), 3);
        assert_eq!(r.steps[0].step, 1);
        // шаг 1: x1 = 0.1*10 = 1.0 > 0; x2 = 0.1*(−5) = −0.5 < 0
        assert!(r.steps[0].positive_mass > 0.0);
        assert!(r.steps[0].negative_mass < 0.0);
        assert_eq!(r.steps[0].active, 3); // 0 (leak 0.5) + 1 + 2
        // топ отсортирован по |x|
        let mut prev = f64::INFINITY;
        for &(v, x) in &r.top {
            assert!(x.abs() <= prev + 1e-12);
            prev = x.abs();
            let _ = v;
        }
        // только тормозные пути: из 0 — только gaba в 2; нейрон 1 недостижим
        let r2 = c
            .propagate(&[0], 2, 0.1, 0.5, SignFilter::Inhibitory, 10, 0.01)
            .unwrap();
        assert!(r2.steps[0].negative_mass < 0.0);
        assert!(!r2.top.iter().any(|&(v, _)| v == 1), "возбуждающий путь при фильтре inh невозможен");
        // гварды: пустые семена, вне диапазона, слишком много
        assert!(c.propagate(&[], 3, 0.1, 0.5, SignFilter::All, 10, 0.01).is_err());
        assert!(c.propagate(&[99], 3, 0.1, 0.5, SignFilter::All, 10, 0.01).is_err());
        let many: Vec<usize> = (0..65).collect();
        assert!(c.propagate(&many, 3, 0.1, 0.5, SignFilter::All, 10, 0.01).is_err());
        // clamp параметров не паникует
        assert!(c.propagate(&[0], 1000, 100.0, 5.0, SignFilter::All, 5, -1.0).is_ok());
    }

    // ---------- Золотые числа C2 на ядре v783 (артефакты из git) ----------

    fn artifact(name: &str) -> std::path::PathBuf {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/flywire-connectome")
            .join(name);
        assert!(p.exists(), "артефакт {} не найден", p.display());
        p
    }

    // 9. Ядро v783: путь, ротор-топ, PageRank, мотивы, симуляция.
    #[test]
    fn golden_c2_core_analytics() {
        let con = Connectome::load(&artifact("flywire_v783_core.csr.zst")).unwrap();
        let csc = con.build_in_edges();

        // Кратчайший путь 0→116214: прямое ребро w17 ach (J-поток отчёта E).
        let p = con.shortest_path(0, 116_214, SignFilter::All);
        assert!(p.found);
        assert_eq!(p.length(), 1);
        assert_eq!(p.hops[0].edge.weight, 17);
        assert_eq!(p.hops[0].edge.nt_name(), "ach");
        assert_eq!(p.total_weight(), 17);

        // Глобальный топ циркуляции (порог 50): золотые пары ядра.
        let rot = con.rotor_top(5, 50);
        assert_eq!(rot.len(), 5);
        let expected = [
            (74_067usize, 133_436usize, 2395i32),
            (112_021, 86_059, 2343),
            (129_880, 136_978, 2330),
            (82_815, 60_985, 2307),
            (53_290, 90_883, 1886),
        ];
        for (i, &(u, v, j)) in expected.iter().enumerate() {
            assert_eq!((rot[i].u, rot[i].v, rot[i].j), (u, v, j), "пара #{i}");
        }
        // антисимметрия компонент: J = fwd − bwd
        for p in &rot {
            let f = p.forward.unwrap_or(0);
            let b = p.backward.unwrap_or(0);
            assert_eq!(p.j, f - b);
        }

        // Хабы ядра: 79529 — гигант (6399 исходящих / 5080 входящих).
        let top_out = con.degree_ranking(Direction::Out, 1, &csc);
        let top_in = con.degree_ranking(Direction::In, 1, &csc);
        assert_eq!(top_out[0], (79_529, 6399));
        assert_eq!(top_in[0], (79_529, 5080));

        // PageRank ядра: топ-1 — нейрон 66912.
        let pr = con.pagerank(SignFilter::All, 0.85, 30, 1e-9, 1);
        assert_eq!(pr.top.len(), 1);
        assert_eq!(pr.top[0].0, 66_912);
        assert!((pr.top[0].1 - 0.001836547).abs() < 1e-7, "rank = {}", pr.top[0].1);

        // Мотивы вокруг 79529: 3684 реципрокных, 11999 ff, 8082 fb.
        let m = con.motif_census(79_529, &csc, 0).unwrap();
        assert_eq!(m.reciprocal.len(), 3684);
        assert_eq!(m.feedforward, 11_999);
        assert_eq!(m.feedback3, 8082);

        // Симуляция от 0: 14 → 455 → 15457 активных; торможение берёт верх.
        let sim = con
            .propagate(&[0], 3, 0.05, 0.8, SignFilter::All, 5, 0.01)
            .unwrap();
        let act: Vec<usize> = sim.steps.iter().map(|s| s.active).collect();
        assert_eq!(act, vec![14, 455, 15_457]);
        assert!(sim.steps[2].negative_mass.abs() > sim.steps[2].positive_mass,
            "шаг 3: торможение обязано доминировать: {sim:?}");
        // топ-1 по |x| — сильно торможенный 129880 (совпадает с ротор-топом)
        assert_eq!(sim.top[0].0, 129_880);
        assert!(sim.top[0].1 < 0.0, "потенциал 129880 отрицательный");
    }
}
