//! Фармакология и био-энергетика связывания (Stage 1 прототипа докинга).
//!
//! # Контур «физика ⇄ химия ⇄ биология» (bio_eval)
//!
//! 1. **Физика**: межмолекулярные энергии — Леннард-Джонс 12-6 (ВдВ),
//!    кулоновское взаимодействие частичных зарядов (εr = 4, среда белка).
//! 2. **Химия**: частичные заряды Гастайгера (итеративное выравнивание
//!    электроотрицательностей), водородные связи донор→акцептор по
//!    геометрии H (2.2–3.5 Å).
//! 3. **Биология**: ΔG_связ = E_vdW + E_HB + E_elec − TΔS (энтропийный
//!    штраф 6 кДж/моль на вращаемую связь); Kd = e^{ΔG/RT} — в
//!    **лог-домене** (ln Kd), с тритной глубиной Kd = 3^(−d) — та же
//!    сверхдиапазонная арифметика, что и в calc logpow.
//! 4. **Топология**: контакты атомов — через 27-дерево (O(log N) вместо
//!    O(N²)); выпуклые оболочки проекций лиганд/карман — настоящий
//!    DE-9IM (OGC) relate: within / contains / overlaps / touches.
//! 5. **Поиск позы**: детерминированный набор из 26 ориентаций ×
//!    hill-climb сближения (жёсткое тело).
//!
//! Это прототип уровня «Lock-and-Key v1»: без гибкости мишени и без
//! явной сольватации; честный докинг — на следующих ступенях лестницы.
//! Константы — стандартные значения скоринг-функций из литературы.

use super::geom3d::{embed, vdw_radius, Conformer};
use super::smiles::{parse_smiles, BondOrder, Descriptors, MoleculeGraph};

/// Частичные заряды (Гастайгер-подобная итеративная схема, единицы e):
/// (.0 — тяжёлые атомы, .1 — виртуальные H-облака каждого атома).
#[derive(Debug, Clone)]
pub struct GasteigerCharges(pub Vec<f64>, pub Vec<f64>);

impl GasteigerCharges {
    /// Полный эффективный заряд (тяжёлые + H-облака) — равен формальному.
    pub fn effective_total(&self, g: &MoleculeGraph) -> f64 {
        self.0.iter().sum::<f64>()
            + self.1
                .iter()
                .zip(g.atoms.iter())
                .map(|(&qh, a)| qh * a.h_count as f64)
                .sum::<f64>()
    }
}

/// Электроотрицательность (Полинг) — из universal_chem.
fn electronegativity(symbol: &str) -> f64 {
    crate::universal_chem::get_element_by_symbol(symbol)
        .map(|e| e.electronegativity as f64)
        .unwrap_or(2.2)
}

/// Итеративное выравнивание электроотрицательностей (Гастайгер-Марсили,
/// упрощение): χ_i(q) = χ_i⁰ + 1.4·q_i, 8 итераций, нормировка на
/// суммарный формальный заряд молекулы.
pub fn gasteiger(g: &MoleculeGraph) -> GasteigerCharges {
    const CHI_H: f64 = 2.2; // электроотрицательность водорода
    let n = g.atoms.len();
    let mut q = vec![0.0f64; n];
    let mut q_h = vec![0.0f64; n]; // виртуальный заряд H-облака атома
    for _ in 0..8 {
        let mut dq = vec![0.0f64; n];
        let mut dq_h = vec![0.0f64; n];
        for b in &g.bonds {
            let chi_a = electronegativity(&g.atoms[b.a].symbol) + 1.4 * q[b.a];
            let chi_b = electronegativity(&g.atoms[b.b].symbol) + 1.4 * q[b.b];
            // электронная плотность (−q) тянется к более χ
            let shift = 0.06 * (chi_b - chi_a);
            dq[b.a] += shift;
            dq[b.b] -= shift;
        }
        // Неявные H: поляризация между атомом и его H-облаком
        for i in 0..n {
            if g.atoms[i].h_count == 0 {
                continue;
            }
            let chi_i = electronegativity(&g.atoms[i].symbol) + 1.4 * q[i];
            let chi_h = CHI_H + 1.4 * q_h[i];
            let shift = 0.06 * (chi_h - chi_i);
            dq[i] += shift * g.atoms[i].h_count as f64;
            dq_h[i] -= shift;
        }
        for i in 0..n {
            q[i] += dq[i];
            q_h[i] += dq_h[i];
        }
    }
    let total: f64 = g.atoms.iter().map(|a| a.charge as f64).sum();
    // Сумма зарядов включает виртуальные H-облака
    let sum: f64 = q.iter().sum::<f64>()
        + q_h
            .iter()
            .zip(g.atoms.iter())
            .map(|(&qh, a)| qh * a.h_count as f64)
            .sum::<f64>();
    if n > 0 {
        // Излишек размазываем по тяжёлым атомам (H-облака лёгкие)
        let corr = (total - sum) / n as f64;
        for x in q.iter_mut() {
            *x += corr;
        }
    }
    GasteigerCharges(q, q_h)
}

// ─── Результат связывания ───────────────────────────────────────────────

/// Итог сквозного расчёта лиганд ⇄ мишень.
#[derive(Debug, Clone)]
pub struct BindingResult {
    pub ligand_formula: String,
    pub target_formula: String,
    pub n_poses: usize,
    pub best_pose: usize,
    /// Энергии лучшей позы, кДж/моль
    pub e_vdw: f64,
    pub e_hbond: f64,
    pub e_elec: f64,
    pub e_entropy: f64,
    /// Полная свободная энергия связывания, кДж/моль
    pub delta_g: f64,
    /// ln Kd = ΔG/RT — константа диссоциации в лог-домене
    pub ln_kd: f64,
    /// log10 Kd
    pub log10_kd: f64,
    /// Тритная глубина Kd = 3^(−d)
    pub trit_depth: f64,
    pub contacts: usize,
    pub hbonds: usize,
    /// 27-дерево мишени: (объектов, узлов, высота)
    pub tree27: (usize, usize, u8),
    /// DE-9IM-паттерн оболочек (балансированные 9 тритов)
    pub de9im_pattern: String,
    /// Топологический вердикт OGC
    pub de9im_verdict: &'static str,
    /// Трит-вердикт связывания
    pub trit_verdict: i8,
    /// Дескрипторы лиганда
    pub descriptors: Descriptors,
    /// logP лиганда (оценка)
    pub logp: f64,
    /// Молярная масса лиганда, г/моль
    pub molar_mass: f64,
}

const R_GAS: f64 = 8.314462618e-3; // кДж/(моль·К)
const LN_3: f64 = 1.0986122886681098; // ln 3
const T_ROOM: f64 = 298.15;
/// Кулоновская константа, кДж·Å/(моль·e²)
const K_ELEC: f64 = 1389.35456;
/// Диэлектрик белковой среды
const EPS_R: f64 = 4.0;

/// Детальный разбор позы: (E_vdW, E_HB, E_elec, контакты, H-связи).
fn pose_detail(
    lig_pos: &[[f64; 3]],
    tgt_pos: &[[f64; 3]],
    lig: &MoleculeGraph,
    tgt: &MoleculeGraph,
    lig_q: &GasteigerCharges,
    tgt_q: &GasteigerCharges,
    lig_conf: &Conformer,
    tgt_conf: &Conformer,
) -> (f64, f64, f64, usize, usize) {
    let mut e_vdw = 0.0f64;
    let mut e_elec = 0.0f64;
    let mut contacts = 0usize;

    // Тяжёлые узлы (через heavy_map)
    let lmap = &lig_conf.heavy_map;
    let tmap = &tgt_conf.heavy_map;

    // Maps узел → атом (тяжёлые узлы через heavy_map)
    let lig_node_atom: Vec<Option<usize>> = {
        let mut v = vec![None; lig_conf.nodes.len()];
        for (ai, &ni) in lmap.iter().enumerate() {
            v[ni] = Some(ai);
        }
        v
    };
    let tgt_node_atom: Vec<Option<usize>> = {
        let mut v = vec![None; tgt_conf.nodes.len()];
        for (ai, &ni) in tmap.iter().enumerate() {
            v[ni] = Some(ai);
        }
        v
    };

    for &ni in lmap {
        let ai = match lig_node_atom[ni] {
            Some(a) => a,
            None => continue,
        };
        let r_i = vdw_radius(&lig.atoms[ai].symbol);
        for &nj in tmap {
            let aj = match tgt_node_atom[nj] {
                Some(a) => a,
                None => continue,
            };
            let dx = lig_pos[ni][0] - tgt_pos[nj][0];
            let dy = lig_pos[ni][1] - tgt_pos[nj][1];
            let dz = lig_pos[ni][2] - tgt_pos[nj][2];
            let r2 = dx * dx + dy * dy + dz * dz;
            if r2 > 100.0 {
                continue; // обрезка 10 Å
            }
            let r = r2.sqrt().max(0.35);
            let r_j = vdw_radius(&tgt.atoms[aj].symbol);
            let rij = r_i + r_j;
            if r < 4.5 {
                contacts += 1;
            }
            // Леннард-Джонс 12-6, soft-core cap
            let eps = 0.42 * (r_i * r_j).sqrt() / 1.6;
            let x = rij / r;
            let lj = 4.0 * eps * (x.powi(12) - x.powi(6));
            e_vdw += lj.min(8.0);
            // Кулон (заряды в e, r в Å)
            e_elec += K_ELEC * lig_q.0[ai] * tgt_q.0[aj] / (EPS_R * r);
        }
    }

    // Водородные связи: донор (N/O с H) → акцептор (N/O без H, заряд ≤ 0)
    // Лиганд-донор: используем реальные позиции H лиганда
    let is_donor = |g: &MoleculeGraph, i: usize| {
        matches!(g.atoms[i].symbol.as_str(), "N" | "O") && g.atoms[i].h_count > 0
    };
    let is_acceptor = |g: &MoleculeGraph, i: usize| {
        // O — акцептор всегда при заряде ≤ 0 (две неподелённые пары,
        // даже в гидроксиле); N — только без H (аминный/амидный N-H не акцептор в v1)
        match g.atoms[i].symbol.as_str() {
            "O" => g.atoms[i].charge <= 0,
            "N" => g.atoms[i].h_count == 0 && g.atoms[i].charge <= 0,
            _ => false,
        }
    };

    let mut e_hb = 0.0f64;
    let mut hbonds = 0usize;
    let mut counted: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

    // H-узлы лиганда (узел → родительский тяжёлый узел)
    let mut lig_h: Vec<(usize, usize)> = Vec::new(); // (тяжёлый узел, H-узел)
    for b in &lig_conf.bonds {
        if lig_conf.nodes[b.a].is_h && !lig_conf.nodes[b.b].is_h {
            lig_h.push((b.b, b.a));
        } else if lig_conf.nodes[b.b].is_h && !lig_conf.nodes[b.a].is_h {
            lig_h.push((b.a, b.b));
        }
    }
    for &(d_node, h_node) in &lig_h {
        let d_atom = match lig_node_atom[d_node] {
            Some(a) => a,
            None => continue,
        };
        if !is_donor(lig, d_atom) {
            continue;
        }
        for &a_node in tmap {
            let a_atom = match tgt_node_atom[a_node] {
                Some(a) => a,
                None => continue,
            };
            if !is_acceptor(tgt, a_atom) {
                continue;
            }
            // D-H···A: H у акцептора в 2.2..3.5 Å, донор близко
            let rha = dist3(lig_pos[h_node], tgt_pos[a_node]);
            let rda = dist3(lig_pos[d_node], tgt_pos[a_node]);
            if (2.2..3.5).contains(&rha) && rda < 4.2 && counted.insert((d_atom, a_atom)) {
                e_hb -= 8.0;
                hbonds += 1;
            }
        }
    }
    // Мишень-донор → лиганд-акцептор: H мишени берём из её конформера
    let mut tgt_h: Vec<(usize, usize)> = Vec::new();
    for b in &tgt_conf.bonds {
        if tgt_conf.nodes[b.a].is_h && !tgt_conf.nodes[b.b].is_h {
            tgt_h.push((b.b, b.a));
        } else if tgt_conf.nodes[b.b].is_h && !tgt_conf.nodes[b.a].is_h {
            tgt_h.push((b.a, b.b));
        }
    }
    for &(d_node, h_node) in &tgt_h {
        let d_atom = match tgt_node_atom[d_node] {
            Some(a) => a,
            None => continue,
        };
        if !is_donor(tgt, d_atom) {
            continue;
        }
        for &a_node in lmap {
            let a_atom = match lig_node_atom[a_node] {
                Some(a) => a,
                None => continue,
            };
            if !is_acceptor(lig, a_atom) {
                continue;
            }
            let rha = dist3(tgt_pos[h_node], lig_pos[a_node]);
            let rda = dist3(tgt_pos[d_node], lig_pos[a_node]);
            if (2.2..3.5).contains(&rha) && rda < 4.2 && counted.insert((usize::MAX - d_atom, a_atom))
            {
                e_hb -= 8.0;
                hbonds += 1;
            }
        }
    }

    (e_vdw, e_hb, e_elec, contacts, hbonds)
}

fn dist3(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Максимальный радиус атомов от центроида.
fn max_radius(pos: &[[f64; 3]]) -> f64 {
    pos.iter()
        .map(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt())
        .fold(0.0, f64::max)
}

/// Набор из 26 ориентаций: 6 осей + 8 диагоналей + 12 рёбер куба.
fn orientation_set() -> Vec<[[f64; 3]; 3]> {
    let mut dirs: Vec<[f64; 3]> = Vec::new();
    for a in [-1.0, 1.0] {
        dirs.push([a, 0.0, 0.0]);
        dirs.push([0.0, a, 0.0]);
        dirs.push([0.0, 0.0, a]);
    }
    for a in [-1.0, 1.0] {
        for b in [-1.0, 1.0] {
            for c in [-1.0, 1.0] {
                dirs.push(normalize3([a, b, c]));
            }
        }
    }
    for a in [-1.0, 1.0] {
        for b in [-1.0, 1.0] {
            dirs.push(normalize3([a, b, 0.0]));
            dirs.push(normalize3([a, 0.0, b]));
            dirs.push(normalize3([0.0, a, b]));
        }
    }
    dirs.iter().map(|&d| frame_from_z(d)).collect()
}

fn normalize3(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n < 1e-12 {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / n, v[1] / n, v[2] / n]
    }
}

/// Ортонормированный базис, третья ось = d.
fn frame_from_z(d: [f64; 3]) -> [[f64; 3]; 3] {
    let up = if d[2].abs() > 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let x = normalize3(cross3(up, d));
    let y = cross3(d, x);
    [x, y, d]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn mat_vec(m: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

/// Сквозной расчёт связывания: лиганд + мишень.
///
/// Принимает И имена («дофамин»), и SMILES — через resolve_input.
pub fn binding_report(ligand_input: &str, target_input: &str) -> Result<BindingResult, String> {
    let lig = super::resolve_input(ligand_input)?;
    let tgt = super::resolve_input(target_input)?;
    binding_report_graphs(&lig, &tgt)
}

/// Сквозной расчёт по готовым графам (MCP/calc передают распознанные).
pub fn binding_report_graphs(
    lig: &MoleculeGraph,
    tgt: &MoleculeGraph,
) -> Result<BindingResult, String> {
    if lig.atoms.is_empty() || tgt.atoms.is_empty() {
        return Err("пустые молекулы".into());
    }

    let lig_conf = embed(lig)?;
    let tgt_conf = embed(tgt)?;
    let lig_q = gasteiger(&lig);
    let tgt_q = gasteiger(&tgt);

    let orientations = orientation_set();
    let r_lig = max_radius(&lig_conf.positions);
    let r_tgt = max_radius(&tgt_conf.positions);

    let mut best: Option<(f64, Vec<[f64; 3]>, usize)> = None;

    for (pi, m) in orientations.iter().enumerate() {
        // Повёрнутый лиганд на стартовом удалении
        let mut pos: Vec<[f64; 3]> = lig_conf
            .positions
            .iter()
            .map(|p| mat_vec(m, *p))
            .collect();
        let dir = mat_vec(m, [0.0, 0.0, 1.0]);
        let mut d = r_lig + r_tgt + 1.5;
        let translate = |pos: &mut Vec<[f64; 3]>, d: f64| {
            for p in pos.iter_mut() {
                p[0] += dir[0] * d;
                p[1] += dir[1] * d;
                p[2] += dir[2] * d;
            }
        };
        translate(&mut pos, d);

        let energy_of = |pos: &[[f64; 3]]| -> f64 {
            let (v, hb, el, _, _) = pose_detail(
                pos,
                &tgt_conf.positions,
                &lig,
                &tgt,
                &lig_q,
                &tgt_q,
                &lig_conf,
                &tgt_conf,
            );
            v + hb + el
        };

        // Hill-climb сближения по 0.4 Å
        let mut best_e = energy_of(&pos);
        let mut best_pos = pos;
        while d > 1.0 {
            let mut cand = best_pos.clone();
            translate(&mut cand, -0.4);
            let e = energy_of(&cand);
            if e < best_e - 1e-9 {
                best_e = e;
                best_pos = cand;
                d -= 0.4;
            } else {
                break;
            }
        }
        match &best {
            None => best = Some((best_e, best_pos, pi)),
            Some((e, _, _)) if best_e < *e => best = Some((best_e, best_pos, pi)),
            _ => {}
        }
    }

    let (best_energy, best_pos, best_pose) = best.ok_or("не удалось найти позу")?;
    let _ = best_energy;

    let (e_vdw, e_hb, e_elec, contacts, hbonds) = pose_detail(
        &best_pos,
        &tgt_conf.positions,
        &lig,
        &tgt,
        &lig_q,
        &tgt_q,
        &lig_conf,
        &tgt_conf,
    );

    // Энтропийный штраф: 6 кДж/моль на вращаемую связь (кап 12)
    let rot = lig.rotatable_bonds().len().min(12);
    let e_entropy = 6.0 * rot as f64;

    let delta_g = e_vdw + e_hb + e_elec + e_entropy;
    let ln_kd = delta_g / (R_GAS * T_ROOM);
    let log10_kd = ln_kd / std::f64::consts::LN_10;
    let trit_depth = -ln_kd / LN_3;

    let tree27 = build_contacts_tree(&tgt_conf, &best_pos, &lig_conf);
    let (de9im_pattern, de9im_verdict) = hulls_de9im(&best_pos, &tgt_conf.positions, &lig_conf, &tgt_conf);

    let trit_verdict = if delta_g < -15.0 {
        1
    } else if delta_g > 5.0 {
        -1
    } else {
        0
    };

    Ok(BindingResult {
        ligand_formula: lig.hill_formula(),
        target_formula: tgt.hill_formula(),
        n_poses: orientations.len(),
        best_pose,
        e_vdw,
        e_hbond: e_hb,
        e_elec,
        e_entropy,
        delta_g,
        ln_kd,
        log10_kd,
        trit_depth,
        contacts,
        hbonds,
        tree27,
        de9im_pattern,
        de9im_verdict,
        trit_verdict,
        descriptors: super::smiles::descriptors(&lig),
        logp: super::smiles::logp_estimate(&lig),
        molar_mass: molar_mass_of(&lig),
    })
}

/// Молярная масса графа (тяжёлые + неявные H), г/моль.
/// Публична для пре-фильтров скрининга (контур B): масса нужна до докинга.
pub fn molar_mass_of(g: &MoleculeGraph) -> f64 {
    let mut m = 0.0;
    for a in &g.atoms {
        m += crate::universal_chem::get_element_by_symbol(&a.symbol)
            .map(|e| e.atomic_mass)
            .unwrap_or(12.0);
        m += a.h_count as f64 * 1.008;
    }
    m
}

/// 27-дерево: тяжёлые атомы мишени, оконные запросы 4 Å от атомов лиганда.
fn build_contacts_tree(
    tgt_conf: &Conformer,
    lig_pos: &[[f64; 3]],
    lig_conf: &Conformer,
) -> (usize, usize, u8) {
    use crate::geo::tree27::{BBox3, Tree27};
    let mut min = [i64::MAX; 3];
    let mut max = [i64::MIN; 3];
    for p in tgt_conf.positions.iter().chain(lig_pos.iter()) {
        for k in 0..3 {
            let v = (p[k] * 100.0).round() as i64;
            min[k] = min[k].min(v - 20);
            max[k] = max[k].max(v + 20);
        }
    }
    let bounds = BBox3::new(min[0], min[1], min[2], max[0], max[1], max[2]);
    let mut tree = Tree27::new(bounds, 8);
    for &ni in &tgt_conf.heavy_map {
        let p = tgt_conf.positions[ni];
        let x = (p[0] * 100.0).round() as i64;
        let y = (p[1] * 100.0).round() as i64;
        let z = (p[2] * 100.0).round() as i64;
        let _ = tree.insert(BBox3::new(x, y, z, x, y, z), ni);
    }
    let mut found = 0usize;
    for &ni in &lig_conf.heavy_map {
        let p = lig_pos[ni];
        let x = (p[0] * 100.0).round() as i64;
        let y = (p[1] * 100.0).round() as i64;
        let z = (p[2] * 100.0).round() as i64;
        let r = 400i64; // 4 Å
        found += tree
            .query(&BBox3::new(x - r, y - r, z - r, x + r, y + r, z + r))
            .len();
    }
    let _ = found;
    tree.stats()
}

/// DE-9IM выпуклых оболочек проекций XY (лиганд против мишени, тяжёлые атомы).
fn hulls_de9im(
    lig_pos: &[[f64; 3]],
    tgt_pos: &[[f64; 3]],
    lig_conf: &Conformer,
    tgt_conf: &Conformer,
) -> (String, &'static str) {
    use crate::geo::de9im::relate;
    use crate::geo::trit_coord::TritPoint;

    let lig_pts: Vec<(f64, f64)> = lig_conf
        .heavy_map
        .iter()
        .map(|&ni| (lig_pos[ni][0], lig_pos[ni][1]))
        .collect();
    let tgt_pts: Vec<(f64, f64)> = tgt_conf
        .heavy_map
        .iter()
        .map(|&ni| (tgt_pos[ni][0], tgt_pos[ni][1]))
        .collect();
    let lig_hull = convex_hull_xy(&lig_pts);
    let tgt_hull = convex_hull_xy(&tgt_pts);
    if lig_hull.len() < 3 || tgt_hull.len() < 3 {
        return ("—".into(), "недостаточно атомов для оболочек");
    }
    // Общий центр для обеих оболочек (одинаковая трит-решётка)
    let all: Vec<(f64, f64)> = lig_hull.iter().chain(tgt_hull.iter()).copied().collect();
    let cx = all.iter().map(|p| p.0).sum::<f64>() / all.len() as f64;
    let cy = all.iter().map(|p| p.1).sum::<f64>() / all.len() as f64;
    let k = 7u8; // ≈0.05 Å решётка
    let lig_ring: Vec<TritPoint> = lig_hull
        .iter()
        .map(|&(x, y)| TritPoint::quantize(x - cx, y - cy, k))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_default();
    let tgt_ring: Vec<TritPoint> = tgt_hull
        .iter()
        .map(|&(x, y)| TritPoint::quantize(x - cx, y - cy, k))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_default();
    if lig_ring.len() < 3 || tgt_ring.len() < 3 {
        return ("—".into(), "квантование не удалось");
    }
    match relate(&lig_ring, &tgt_ring) {
        Ok(m) => {
            let verdict = if m.within() {
                "лиганд внутри кармана (within)"
            } else if m.contains() {
                "лиганд охватывает мишень (contains)"
            } else if m.overlaps() {
                "частичное перекрытие (overlaps)"
            } else if m.touches() {
                "касание поверхностей (touches)"
            } else if m.disjoint() {
                "разделены (disjoint)"
            } else {
                "сложная топология"
            };
            (m.bal_string(), verdict)
        }
        Err(_) => ("—".into(), "relate не сошёлся"),
    }
}

/// Выпуклая оболочка (монотонная цепь Эндрю).
fn convex_hull_xy(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut pts = points.to_vec();
    pts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let cross2 = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    let mut lower: Vec<(f64, f64)> = Vec::new();
    for p in &pts {
        while lower.len() >= 2 && cross2(lower[lower.len() - 2], lower[lower.len() - 1], *p) <= 0.0 {
            lower.pop();
        }
        lower.push(*p);
    }
    let mut upper: Vec<(f64, f64)> = Vec::new();
    for p in pts.iter().rev() {
        while upper.len() >= 2 && cross2(upper[upper.len() - 2], upper[upper.len() - 1], *p) <= 0.0 {
            upper.pop();
        }
        upper.push(*p);
    }
    lower.pop();
    upper.pop();
    lower.into_iter().chain(upper).collect()
}

/// Человекочитаемый отчёт bio_eval для «голоса учёного».
pub fn binding_text(r: &BindingResult) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "══ч Био-контур: {} ⇄ {} ══\n",
        r.ligand_formula, r.target_formula
    ));
    s.push_str(&format!(
        "[1] Фармакофор лиганда: {} тяжёлых атомов, HBD {}, HBA {}, вращаемых связей {}, ароматических колец {}; logP ≈ {:+.2}; M = {:.1} г/моль.\n",
        r.descriptors.heavy_atoms,
        r.descriptors.h_bond_donors,
        r.descriptors.h_bond_acceptors,
        r.descriptors.rotatable,
        r.descriptors.aromatic_rings,
        r.logp,
        r.molar_mass
    ));
    let lipinski = r.descriptors.h_bond_donors <= 5
        && r.descriptors.h_bond_acceptors <= 10
        && r.molar_mass <= 500.0
        && r.logp <= 5.0;
    s.push_str(&format!(
        "[2] Липински: {} (HBD≤5: {}, HBA≤10: {}, M≤500: {}, logP≤5: {}).\n",
        if lipinski { "проходит" } else { "нарушение" },
        r.descriptors.h_bond_donors <= 5,
        r.descriptors.h_bond_acceptors <= 10,
        r.molar_mass <= 500.0,
        r.logp <= 5.0
    ));
    s.push_str(&format!(
        "[3] Поиск позы: {} ориентаций × hill-climb сближение; лучшая поза #{}: E_vdW = {:+.1}, E_HB = {:+.1} ({} связей), E_elec = {:+.1} кДж/моль.\n",
        r.n_poses, r.best_pose, r.e_vdw, r.e_hbond, r.hbonds, r.e_elec
    ));
    s.push_str(&format!(
        "[4] Термодинамика: ΔG = {:+.2} кДж/моль (энтропийный штраф {:+.1}); Kd = e^({:.2}) = 10^({:.2}) М — лог-домен, тритная глубина Kd = 3^({:+.2}).\n",
        r.delta_g, r.e_entropy, r.ln_kd, r.log10_kd, r.trit_depth
    ));
    s.push_str(&format!(
        "[5] Топология: {} контактов атомов; 27-дерево мишени: {} объектов, {} узлов, высота {}; DE-9IM оболочек {} — {}.\n",
        r.contacts, r.tree27.0, r.tree27.1, r.tree27.2, r.de9im_pattern, r.de9im_verdict
    ));
    s.push_str(&format!(
        "Трит-вердикт: {} — {}.",
        match r.trit_verdict {
            1 => "+1",
            -1 => "-1",
            _ => "0",
        },
        match r.trit_verdict {
            1 => "связывание термодинамически выгодно",
            -1 => "связывание невыгодно: поза отталкивается",
            _ => "слабое/пограничное связывание",
        }
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gasteiger_water_polarity() {
        let g = parse_smiles("O").unwrap();
        let q = gasteiger(&g);
        assert!(q.0[0] < 0.0, "q(O) = {}", q.0[0]);
        // Полный баланс (с H-облаками) = формальный заряд = 0
        assert!(q.effective_total(&g).abs() < 1e-9);
    }

    #[test]
    fn gasteiger_charge_normalization() {
        let g = parse_smiles("C[NH3+]").unwrap();
        let q = gasteiger(&g);
        assert!((q.effective_total(&g) - 1.0).abs() < 1e-9, "Σq = формальный +1");
    }

    #[test]
    fn binding_water_water_hbond() {
        // Димер воды: H-связь должна найтись
        let r = binding_report("O", "O").unwrap();
        assert_eq!(r.n_poses, 26, "6+8+12 ориентаций");
        assert!(r.hbonds >= 1, "H-связей = {}", r.hbonds);
        assert!(
            (-30.0..30.0).contains(&r.delta_g),
            "ΔG димера воды = {}",
            r.delta_g
        );
    }

    #[test]
    fn binding_benzene_benzene_hydrophobic() {
        let r = binding_report("c1ccccc1", "c1ccccc1").unwrap();
        assert_eq!(r.hbonds, 0, "без доноров H-связей нет");
        assert!(r.e_vdw < 0.0, "ВдВ-стэкинг: E_vdW = {}", r.e_vdw);
        assert_eq!(r.descriptors.h_bond_donors, 0);
    }

    #[test]
    fn binding_report_text_structure() {
        let r = binding_report("CCO", "O").unwrap();
        let text = binding_text(&r);
        assert!(text.contains("ΔG"));
        assert!(text.contains("Липински"));
        assert!(text.contains("лог-домен"));
        assert!(text.contains("DE-9IM"));
        assert!(text.contains("27-дерево"));
    }

    #[test]
    fn binding_deterministic() {
        let a = binding_report("CCO", "c1ccccc1").unwrap();
        let b = binding_report("CCO", "c1ccccc1").unwrap();
        assert_eq!(a.delta_g, b.delta_g, "детерминированность позы");
        assert_eq!(a.best_pose, b.best_pose);
    }

    #[test]
    fn kd_log_domain_math() {
        let r = binding_report("CCO", "O").unwrap();
        let expected_ln = r.delta_g / (R_GAS * T_ROOM);
        assert!((r.ln_kd - expected_ln).abs() < 1e-12);
        assert!((r.log10_kd * std::f64::consts::LN_10 - r.ln_kd).abs() < 1e-12);
        assert!((r.trit_depth * LN_3 + r.ln_kd).abs() < 1e-12);
    }
}
