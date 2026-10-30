//! Мост «Живая муха» → фазовое пространство нарратива.
//!
//! Каста нейронов (BFS от семян по потоку сигнала, детерминированный
//! жадный отбор по синаптической массе) извлекает из коннектома
//! FLYCSR1 три вещи:
//!
//! 1. **Ротор J_sub = A_sub − A_subᵀ** — однонаправленные доминирования
//!    внутри касты: кто кого подавляет. Проекция в архетипное поле
//!    (якорь нейрона = FNV-хэш root_id mod 12) даёт циркуляцию смыслов
//!    γJ·p канонического уравнения: живой мозг закручивает нарратив.
//! 2. **Метрику Ляпунова D = L·Lᵀ** — грамиана роторных масс касты:
//!    G = I + β·(|J_sub| + |J_sub|ᵀ)/2, разложение Холецкого L, и
//!    диссипативный член D·p гасит семантические пертурбации —
//!    устойчивость по Ляпунову калибруется графом мухи.
//! 3. **Драматургические пары** — топ циркуляций J_lit, отображённые
//!    обратно в пары нейронов: конфликт нарратива = роторная пара
//!    реального мозга (кто доминирует над кем).
//!
//! Вся конструкция детерминирована: порядок обхода и отбора фиксирован,
//! повторный запуск на том же артефакте даёт побитово ту же касту.

use std::collections::HashMap;

use crate::graph::connectome::{Connectome, ConnectomeNodes};

use super::linalg::{cholesky_factor, Mat};
use super::qualia::{archetype_name, fnv1a64};

/// Максимум нейронов в касте (кубическая стоимость induced-подграфа
/// и его проекции — 64³ ≈ 262 тыс. операций, микросекунды).
pub const MAX_CAST: usize = 64;

/// Каста нейронов мухи + её фазовые проекции.
pub struct FlyCast {
    /// Индексы CSR выбранных нейронов (отсортированы по возрастанию).
    pub members: Vec<usize>,
    /// Ротор касты J_sub = A_sub − A_subᵀ (m×m, антисимметричная).
    pub j_sub: Mat,
    /// Холецкий-фактор L метрики Ляпунова (нижний треугольник, m×m).
    pub l_chol: Mat,
    /// Метрика Ляпунова D = L·Lᵀ (m×m, SPD).
    pub d_lyap: Mat,
    /// Статистика построения.
    pub stats: CastStats,
    /// Якорь каждого члена: индекс архетипа (0..12).
    pub anchors: Vec<usize>,
}

/// Статистика касты.
#[derive(Clone, Debug, Default)]
pub struct CastStats {
    /// Сколько нейронов достигнуто BFS до усечения.
    pub reached: usize,
    /// Число рёбер внутри касты (обе вершины — члены).
    pub inner_edges: usize,
    /// Суммарная |знаковая| масса внутренних рёбер.
    pub inner_abs_mass: f64,
    /// Время построения, мс.
    pub build_ms: u128,
    /// Максимальная циркуляция внутри касты.
    pub top_rotor: Option<(usize, usize, i32)>,
}

/// Построение касты: BFS от семян (khop шагов, все знаки связей),
/// затем детерминированный жадный отбор: семена всегда входят; прочие
/// кандидаты — по (хоп ↑, макс. вес ребра к уже выбранным ↓, индекс ↑).
pub fn build_cast(
    con: &Connectome,
    seeds: &[usize],
    khop: usize,
    max_m: usize,
) -> Result<FlyCast, String> {
    if seeds.is_empty() {
        return Err("каста пуста: укажите хотя бы один нейрон-семя".into());
    }
    if seeds.len() > 16 {
        return Err(format!("{} семян — максимум 16", seeds.len()));
    }
    for &s in seeds {
        if s >= con.n_nodes() {
            return Err(format!("нейрон {s} вне диапазона (узлов: {})", con.n_nodes()));
        }
    }
    let khop = khop.clamp(1, 4);
    let max_m = max_m.clamp(seeds.len().max(2), MAX_CAST);
    let t0 = std::time::Instant::now();

    // ── BFS: (узел → хоп), очередь по слоям ─────────────────────────
    let mut hop_of: HashMap<usize, usize> = HashMap::new();
    let mut frontier: Vec<usize> = seeds.to_vec();
    for &s in &frontier {
        hop_of.insert(s, 0);
    }
    let mut next: Vec<usize> = Vec::new();
    for step in 1..=khop {
        next.clear();
        let mut fresh: Vec<usize> = Vec::new();
        for &u in &frontier {
            if let Some(edges) = con.out_edges(u) {
                for e in edges {
                    let v = e.target as usize;
                    if let std::collections::hash_map::Entry::Vacant(e) = hop_of.entry(v) {
                        e.insert(step);
                        fresh.push(v);
                    }
                }
            }
        }
        fresh.sort_unstable();
        fresh.dedup();
        next.extend_from_slice(&fresh);
        frontier = next.clone();
        if frontier.is_empty() {
            break;
        }
    }
    let reached = hop_of.len();

    // ── Жадкий детерминированный отбор ──────────────────────────────
    let mut selected: Vec<usize> = seeds.to_vec();
    selected.sort_unstable();
    selected.dedup();
    let mut selected_set: std::collections::HashSet<usize> =
        selected.iter().copied().collect();
    // Кандидаты по слоям хопа; внутри слоя — по убыванию максимального
    // веса ребра К УЖЕ ВЫБРАННЫМ (связь с ядром касты важнее индекса).
    for step in 1..=khop {
        if selected.len() >= max_m {
            break;
        }
        let layer: Vec<usize> = hop_of
            .iter()
            .filter(|(&v, &h)| h == step && !selected_set.contains(&v))
            .map(|(&v, _)| v)
            .collect();
        if layer.is_empty() {
            continue;
        }
        // Сила связи с уже выбранными (исходящие рёбра кандидата к ядру
        // касты; двусторонность покрывается BFS-слоями — на следующий
        // хоп проходят и обратные связи).
        let mut scored: Vec<(usize, u32)> = Vec::with_capacity(layer.len());
        for v in &layer {
            let mut best = 0u32;
            for e in con.out_edges(*v).into_iter().flatten() {
                if selected_set.contains(&(e.target as usize)) {
                    best = best.max(e.weight as u32);
                }
            }
            scored.push((*v, best));
        }
        scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        for (v, _) in scored {
            if selected.len() >= max_m {
                break;
            }
            selected_set.insert(v);
            selected.push(v);
        }
    }
    selected.sort_unstable();
    let m = selected.len();
    // Индекс члена в касте.
    let pos: HashMap<usize, usize> =
        selected.iter().enumerate().map(|(i, &v)| (v, i)).collect();

    // ── Induced-подграф: рёбра между членами ────────────────────────
    let mut a_sub = Mat::zeros(m, m);
    let mut inner_edges = 0usize;
    let mut inner_abs_mass = 0.0f64;
    for (i, &u) in selected.iter().enumerate() {
        for e in con.out_edges(u).into_iter().flatten() {
            if let Some(&j) = pos.get(&(e.target as usize)) {
                let sw = e.signed_weight();
                a_sub.set(i, j, sw as f32);
                inner_edges += 1;
                inner_abs_mass += sw.abs() as f64;
            }
        }
    }
    // Ротор: J = A − Aᵀ (антисимметричен по построению).
    let j_sub = {
        let mut j = Mat::zeros(m, m);
        for i in 0..m {
            for c in 0..m {
                j.set(i, c, a_sub.at(i, c) - a_sub.at(c, i));
            }
        }
        j
    };

    // ── Метрика Ляпунова: G = I + β·sym(|J|), D = L·Lᵀ = G ──────────
    // sym(|J|) — неотрицательная симметричная матрица циркуляционных
    // масс. β подбирается адаптивно: β·max_rowsum(sym) ≤ 1/2 — строгое
    // диагональное доминирование (Гершгорин) гарантирует SPD на любом
    // коннектоме, включая хабы ядра v783 с роторными массами в тысячи.
    let sym_mass = |i: usize, c: usize| (j_sub.at(i, c).abs() + j_sub.at(c, i).abs()) / 2.0;
    let max_row: f32 = (0..m)
        .map(|i| (0..m).filter(|&c| c != i).map(|c| sym_mass(i, c)).sum::<f32>())
        .fold(0.0f32, f32::max);
    let beta = if max_row > 1e-9 {
        (0.5 / max_row).min(0.05)
    } else {
        0.05
    };
    let mut g = Mat::identity(m);
    for i in 0..m {
        for c in 0..m {
            if i != c {
                g.set(i, c, g.at(i, c) + beta * sym_mass(i, c));
            }
        }
    }
    let l_chol = cholesky_factor(&g)
        .map_err(|e| format!("муха: метрика Ляпунова не SPD ({e})"))?;
    let d_lyap = l_chol.matmul(&l_chol.transpose());

    // ── Топ циркуляции внутри касты ────────────────────────────────
    let mut top_rotor: Option<(usize, usize, i32)> = None;
    for i in 0..m {
        for c in (i + 1)..m {
            let jj = j_sub.at(i, c) as i32;
            if top_rotor.map(|(_, _, b)| jj.abs() > b).unwrap_or(jj != 0) {
                top_rotor = Some((selected[i], selected[c], jj));
            }
        }
    }

    // ── Якоря архетипов (нужна таблица root_id либо индекс) ─────────
    let anchors = selected
        .iter()
        .map(|&v| anchor_of_index(v))
        .collect();

    let stats = CastStats {
        reached,
        inner_edges,
        inner_abs_mass,
        build_ms: t0.elapsed().as_millis(),
        top_rotor,
    };
    Ok(FlyCast {
        members: selected,
        j_sub,
        l_chol,
        d_lyap,
        stats,
        anchors,
    })
}

/// Якорь нейрона по root_id (если таблица есть) либо по CSR-индексу.
pub fn anchor_of(_con: &Connectome, nodes: Option<&ConnectomeNodes>, idx: usize) -> usize {
    let key = nodes
        .and_then(|n| n.root_id(idx))
        .unwrap_or(idx as u64);
    (fnv1a64(&key.to_le_bytes()) % super::qualia::ARCHETYPE_COUNT as u64) as usize
}

/// Якорь по индексу (когда таблица root_id не загружена).
fn anchor_of_index(idx: usize) -> usize {
    (fnv1a64(&(idx as u64).to_le_bytes()) % super::qualia::ARCHETYPE_COUNT as u64) as usize
}

/// Фазовые проекции касты в архетипное пространство dims×dims.
pub struct FieldProjection {
    /// Циркуляция смыслов J_lit = B·J_sub·Bᵀ (dims×dims, антисимметричная).
    pub j_lit: Mat,
    /// Диссипативная метрика D_lit = B·D_sub·Bᵀ + I (dims×dims, SPD).
    pub d_lit: Mat,
    /// Сила связи архетипа с кастой: сколько нейронов якорится в него.
    pub archetype_load: [usize; super::qualia::ARCHETYPE_COUNT],
}

impl FlyCast {
    /// Проецировать касту в фазовое пространство нарратива (dims осей).
    ///
    /// Матрица связи B (dims×m): ось a получает ненулевую связь с
    /// нейроном j, если якорь члена j приходится на ось хэша терма
    /// архетипа a... — нет: якорь архетипа a — это ПРОИЗВОЛЬНАЯ ось из
    /// лексики; здесь связка проще и честнее: якорь нейрона = архетип,
    /// архетип распределяет свою массу по «своим» осям лексики через
    /// нормированный якорный вектор. B = A_arch (dims×12) · M (12×m),
    /// где M[a][j] = 1/√n_a при якоре члена j в архетипе a.
    pub fn project_to_field(&self, dims: usize) -> FieldProjection {
        let dims = dims.clamp(16, 256);
        let m = self.members.len();
        // Якорные векторы архетипов в фазовом пространстве (dims).
        let anchors_field: Vec<Vec<f32>> = (0..super::qualia::ARCHETYPE_COUNT)
            .map(|a| {
                let mut v = vec![0.0f32; dims];
                for kw in super::qualia::ARCHETYPES[a].keywords {
                    v[super::qualia::axis_of(kw, dims)] += 1.0;
                }
                super::linalg::normalize(&v)
            })
            .collect();
        // Нагрузка архетипов членами касты.
        let mut load = [0usize; super::qualia::ARCHETYPE_COUNT];
        for &a in &self.anchors {
            load[a] += 1;
        }
        // B (dims×m): столбец j = якорный вектор архетипа члена j,
        // нормированный на √n_a (масса архетипа делится между его нейронами).
        let mut b = Mat::zeros(dims, m);
        for (j, &a) in self.anchors.iter().enumerate() {
            let n_a = load[a].max(1) as f32;
            let k = 1.0 / n_a.sqrt();
            for (r, &w) in anchors_field[a].iter().enumerate() {
                b.set(r, j, w * k);
            }
        }
        let bt = b.transpose();
        // J_lit = B·J_sub·Bᵀ — антисимметрична (J_sub антисимметрична).
        let j_raw = b.matmul(&self.j_sub).matmul(&bt);
        // Нормировка циркуляции: max|J_lit| = 1. Сырые роторные массы
        // мухи достигают тысяч (ядро v783: топ-ротор +2395) — Эйлерова
        // дискретизация p ← p − ηγJ·p устойчива лишь при ηγ‖J‖ < 2,
        // поэтому ФОРМА циркуляции нормируется к единице, сила —
        // управляется γ. Драматургические пары сообщают сырые J.
        let jmax = j_raw.abs_max().max(1e-9);
        let j_lit = j_raw.scaled(1.0 / jmax);
        // D_lit = I + B·D_sub·Bᵀ (нормированная диссипация) — SPD.
        let d_raw = b.matmul(&self.d_lyap).matmul(&bt);
        let dmax = d_raw.abs_max().max(1e-9);
        let d_scaled = d_raw.scaled(1.0 / dmax);
        let mut d_lit = Mat::identity(dims);
        for r in 0..dims {
            for c in 0..dims {
                d_lit.set(r, c, d_lit.at(r, c) + d_scaled.at(r, c));
            }
        }
        FieldProjection {
            j_lit,
            d_lit,
            archetype_load: load,
        }
    }

    /// Топ драматургических пар: сильнейшие циркуляции J_lit,
    /// отображённые обратно в пары нейронов касты.
    ///
    /// Для пары осей (r, c) с J_lit[r][c] ≠ 0 ищется пара членов
    /// (u, v), дающая максимум вклада в эту циркуляцию через якоря.
    /// Возвращает (нейрон-u, нейрон-v, J, архетип-u, архетип-v).
    pub fn dramatic_pairs(&self, proj: &FieldProjection, top: usize) -> Vec<DramaticPair> {
        let dims = proj.j_lit.rows;
        // Вклад члена j в ось r: B[r][j] (dims×m).
        let mut pairs: Vec<DramaticPair> = Vec::new();
        // Соберём все пары осей с ненулевой циркуляцией.
        let mut axes: Vec<(usize, usize, f32)> = Vec::new();
        for r in 0..dims {
            for c in 0..dims {
                let v = proj.j_lit.at(r, c);
                if v.abs() > 1e-6 {
                    axes.push((r, c, v));
                }
            }
        }
        axes.sort_by(|a, b| b.2.abs().partial_cmp(&a.2.abs()).unwrap().then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1)));
        axes.truncate(top.max(1));
        let mut seen: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for &(r, c, jval) in &axes {
            // Лучшие члены для осей r и c: максимум B (масса связи).
            let mut best_r: Option<(usize, f32)> = None;
            let mut best_c: Option<(usize, f32)> = None;
            // B не сохранена — пересчитаем локально через якоря.
            for (j, &a) in self.anchors.iter().enumerate() {
                let load = proj.archetype_load[a].max(1) as f32;
                let k = 1.0 / load.sqrt();
                let w_r = anchor_axis_weight(a, r, dims) * k;
                let w_c = anchor_axis_weight(a, c, dims) * k;
                if w_r > 1e-9 && best_r.map(|(_, b)| w_r > b).unwrap_or(true) {
                    best_r = Some((j, w_r));
                }
                if w_c > 1e-9 && best_c.map(|(_, b)| w_c > b).unwrap_or(true) {
                    best_c = Some((j, w_c));
                }
            }
            if let (Some((iu, _)), Some((iv, _))) = (best_r, best_c) {
                if iu != iv {
                    // Дедуп двунаправленной пары (конвенция rotor_top C2):
                    // эмитит сторона с J > 0 («кто доминирует»), зеркало
                    // с J < 0 молчит.
                    let key = (iu.min(iv), iu.max(iv));
                    if jval < 0.0 || !seen.insert(key) {
                        continue;
                    }
                    pairs.push(DramaticPair {
                        u: self.members[iu],
                        v: self.members[iv],
                        j: jval,
                        u_archetype: archetype_name(self.anchors[iu]),
                        v_archetype: archetype_name(self.anchors[iv]),
                    });
                }
            }
        }
        pairs
    }
}

/// Вес связи архетипа a с осью axis (пересчёт якоря на лету).
fn anchor_axis_weight(a: usize, axis: usize, dims: usize) -> f32 {
    let mut v = vec![0.0f32; dims];
    for kw in super::qualia::ARCHETYPES[a].keywords {
        v[super::qualia::axis_of(kw, dims)] += 1.0;
    }
    let n = super::linalg::norm2(&v);
    if n < 1e-12 {
        return 0.0;
    }
    v[axis] / n
}

/// Драматургическая пара: конфликт нарратива = роторная пара мухи.
#[derive(Clone, Debug)]
pub struct DramaticPair {
    /// Доминирующий нейрон (u: J(u,v) > 0).
    pub u: usize,
    /// Подавляемый нейрон.
    pub v: usize,
    /// Циркуляция J_lit осей пары.
    pub j: f32,
    /// Архетип доминанты.
    pub u_archetype: &'static str,
    /// Архетип подавляемого.
    pub v_archetype: &'static str,
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

    #[test]
    fn cast_deterministic_and_complete() {
        let c = con();
        let a = build_cast(&c, &[0], 2, 16).unwrap();
        let b = build_cast(&c, &[0], 2, 16).unwrap();
        assert_eq!(a.members, b.members, "каста детерминирована");
        // khop=2 от 0 достигает все 4 узла
        assert_eq!(a.stats.reached, 4);
        assert_eq!(a.members, vec![0, 1, 2, 3]);
        assert_eq!(a.j_sub.rows, 4);
        // Ротор: J[0][1] = A(0,1) − A(1,0) = 10 − 0 = +10 (ach)
        assert_eq!(a.j_sub.at(0, 1), 10.0);
        assert_eq!(a.j_sub.at(1, 0), -10.0, "антисимметрия");
        // Модулятор da (вес 4) не входит в J (signed_weight = 0),
        // обратного ребра 2→1 нет → J(1,2) = 0
        assert_eq!(a.j_sub.at(1, 2), 0.0, "da-модулятор невидим для J");
        // J(2,3) = A(2,3) − A(3,2) = 7 − 0 = +7 (ach)
        assert_eq!(a.j_sub.at(2, 3), 7.0);
        // J(1,3) = A(1,3) − A(3,1) = 3 − (−9) = +12 (glut против gaba)
        assert_eq!(a.j_sub.at(1, 3), 12.0);
        assert_eq!(a.j_sub.at(3, 1), -12.0);
        // Внутренние рёбра — все 6 (мод-ребро тоже в A_sub с весом 0)
        assert_eq!(a.stats.inner_edges, 6);
    }

    #[test]
    fn cast_truncates_by_mass() {
        let c = con();
        // max_m = 2: входят семя 0 и сильнейший сосед (1: w10 ach)
        let s = build_cast(&c, &[0], 2, 2).unwrap();
        assert_eq!(s.members, vec![0, 1]);
        // Метрика Ляпунова SPD по построению: Холецкий уже прошёл;
        // D = L·Lᵀ восстанавливает G
        let g = s.l_chol.matmul(&s.l_chol.transpose());
        for r in 0..2 {
            assert!((g.at(r, r) - s.d_lyap.at(r, r)).abs() < 1e-5);
        }
    }

    #[test]
    fn cast_guards() {
        let c = con();
        assert!(build_cast(&c, &[], 2, 8).is_err());
        assert!(build_cast(&c, &[99], 2, 8).is_err());
        let many: Vec<usize> = (0..17).collect();
        assert!(build_cast(&c, &many, 2, 8).is_err());
    }

    #[test]
    fn field_projection_mathematics() {
        let c = con();
        let cast = build_cast(&c, &[0], 2, 16).unwrap();
        let proj = cast.project_to_field(64);
        // J_lit антисимметрична
        let jt = proj.j_lit.transpose();
        for r in 0..64 {
            for cc in 0..64 {
                assert!(
                    (proj.j_lit.at(r, cc) + jt.at(r, cc)).abs() < 1e-4,
                    "J_lit[{r}][{cc}] не антисимметрична"
                );
            }
        }
        // D_lit строго диагонально доминирует (SPD): diag ≥ 1
        for r in 0..64 {
            assert!(proj.d_lit.at(r, r) >= 1.0 - 1e-5);
        }
        // Нагрузка архетипов: 4 нейрона распределены
        assert_eq!(proj.archetype_load.iter().sum::<usize>(), 4);
        // Драматургические пары не пусты и валидны
        let pairs = cast.dramatic_pairs(&proj, 3);
        assert!(!pairs.is_empty(), "циркуляция есть — пары обязаны быть");
        for p in &pairs {
            assert!(cast.members.contains(&p.u));
            assert!(cast.members.contains(&p.v));
            assert!(p.u_archetype.len() > 1 && p.v_archetype.len() > 1);
        }
    }
}

#[cfg(test)]
mod golden_tests {
    use super::*;
    use crate::graph::connectome::Connectome;

    fn core() -> Connectome {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/flywire-connectome/flywire_v783_core.csr.zst");
        assert!(p.exists(), "артефакт {} не найден", p.display());
        Connectome::load(&p).unwrap()
    }

    // Золотые числа L1 на ядре v783: каста от нейрона 0 (тот же старт,
    // что в золоте C2 — фронты [13, 443] → 457 достигнуто).
    #[test]
    fn golden_l1_cast_from_neuron_zero() {
        let con = core();
        let cast = build_cast(&con, &[0], 2, 48).unwrap();
        // BFS достигает 457 узлов (13 + 443 + стартовый — золото C2),
        // жадный отбор усекает до 48 членов.
        assert_eq!(cast.stats.reached, 457);
        assert_eq!(cast.members.len(), 48);
        assert_eq!(cast.members[0], 0, "семя всегда первый");
        // Детерминизм отбора: закреплённые первые/последние члены.
        assert_eq!(&cast.members[..8], &[0, 3474, 6135, 6277, 10798, 13609, 16197, 19680]);
        assert_eq!(cast.members[47], 136_978);
        // Induced-подграф касты.
        assert_eq!(cast.stats.inner_edges, 252);
        assert_eq!(cast.stats.inner_abs_mass as i64, 4345);
        // Топ-ротор внутри касты.
        assert_eq!(cast.stats.top_rotor, Some((31_552, 80_928, 197)));
        // Повторное построение — побитово та же каста.
        let cast2 = build_cast(&con, &[0], 2, 48).unwrap();
        assert_eq!(cast.members, cast2.members);
        assert_eq!(cast.j_sub.data, cast2.j_sub.data);
    }

    #[test]
    fn golden_l1_field_projection_and_pairs() {
        let con = core();
        let cast = build_cast(&con, &[0], 2, 48).unwrap();
        let proj = cast.project_to_field(64);
        // Нормировка циркуляции: max|J_lit| = 1.
        assert!((proj.j_lit.abs_max() - 1.0).abs() < 1e-5, "max|J| = {}", proj.j_lit.abs_max());
        // Антисимметрия J_lit.
        let jt = proj.j_lit.transpose();
        for r in 0..64 {
            for c in 0..64 {
                assert!((proj.j_lit.at(r, c) + jt.at(r, c)).abs() < 1e-4);
            }
        }
        // Диссипация SPD: диагональ ≥ 1.
        for r in 0..64 {
            assert!(proj.d_lit.at(r, r) >= 1.0 - 1e-5);
        }
        // Нагрузка архетипов: 48 нейронов по 12 якорям.
        assert_eq!(proj.archetype_load.iter().sum::<usize>(), 48);
        assert_eq!(
            proj.archetype_load,
            [6, 4, 5, 2, 6, 4, 2, 2, 2, 5, 6, 4]
        );
        // Драматургические пары: доминирование Тишины над
        // Сверхпроводимостью (нейроны 13609 → 41414), J = +1.
        let pairs = cast.dramatic_pairs(&proj, 5);
        assert!(!pairs.is_empty());
        assert!(
            pairs.iter().all(|p| p.j > 0.0 || pairs.len() == 1),
            "дедуп зеркал: все пары J > 0"
        );
        let top = &pairs[0];
        assert_eq!((top.u, top.v), (13_609, 41_414), "топ-пара: {top:?}");
        assert!((top.j - 1.0).abs() < 1e-4, "J = {}", top.j);
        // Каждая пара — уникальная (нет зеркал).
        let mut keys: Vec<(usize, usize)> =
            pairs.iter().map(|p| (p.u.min(p.v), p.u.max(p.v))).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), pairs.len(), "зеркальные дубликаты запрещены");
    }
}
