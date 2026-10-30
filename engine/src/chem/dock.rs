//! Гибкий молекулярный докинг: SMILES-лиганд в карман белка из PDB.
//!
//! # Ступень 5 лестницы «атом → молекула → белок → клетка»
//!
//! **Протокол** (Flexible Ligand Docking, стиль AutoDock/Vina на CPU):
//!
//! 1. **Лиганд**: SMILES → 3D-конформер (geom3d) → заряды Гастайгера
//!    (эффективные: тяжёлый атом + его H-облако) → торсионное дерево
//!    (вращаемые σ-связи, кроме амидных).
//! 2. **Рецептор**: PDB → атомы + united-заряды + реконструированные
//!    H-доноры; 27-дерево (tree27) собирает «поле кармана» — атомы
//!    в сфере кармана+10 Å — один раз, O(log N).
//! 3. **Скоринг** (кДж/моль, константы как в pharma.rs):
//!    LJ 12-6 (soft-core cap), кулон (εr=4), направленные водородные
//!    связи (D–H···A геометрия с угловым фактором), липофильная рампа
//!    (гидрофобные контакты 3.2–4.5 Å), десольватационный штраф за
//!    похороненную ненасыщенную полярность.
//! 4. **Поиск**: Монте-Карло с имитацией отжига. Кручения квантованы
//!    тритами: сектор 2π/27 (13⅓°) — шаг {−1,0,+1}×N секторов;
//!    жёсткое тело — трансляции ≤0.8 Å и повороты ≤0.3 рад.
//!    Метрополис при kT от 12 → 0.4 кДж/моль. Финальная координатная
//!    шлифовка (27-секторные сканы кручений + локальные сдвиги).
//! 5. **Термодинамика**: ΔG лучшей позы + ансамбль топ-поз через
//!    logsumexp в лог-домене (та же сверхдиапазонная арифметика, что
//!    calc logpow); Kd = e^(ΔG/RT), тритная глубина Kd = 3^(−d).
//! 6. **Валидация redocking**: RMSD тяжёлых атомов против кристалла
//!    (жадное спаривание по элементам), успех ≤ 2 Å — академический
//!    стандарт.
//!
//! Всё детерминировано: RNG xorshift64* с сидом от FNV(SMILES+файл).

use super::geom3d::{vdw_radius, Conformer};
use super::pdb::{
    receptor_charges, reconstruct_donors, MacroMol, Pocket, ProteinIndex, ResKind,
};
use super::pharma::{gasteiger, GasteigerCharges};
use super::smiles::{parse_smiles, BondOrder, MoleculeGraph};

// ─── Константы (кДж/моль; Å) ────────────────────────────────────────────

const R_GAS: f64 = 8.314462618e-3; // кДж/(моль·К)
const T_ROOM: f64 = 298.15;
const LN_3: f64 = 1.0986122886681098;
const LN_10: f64 = std::f64::consts::LN_10;
/// Кулоновская константа, кДж·Å/(моль·e²) — как в pharma.rs.
const K_ELEC: f64 = 1389.35456;
/// Диэлектрик белковой среды: ε(r) = EPS_R·r (дистанционно-зависимый,
/// как в докинг-полях AutoDock — экранировка растёт с расстоянием).
const EPS_R: f64 = 4.0;
/// Верхний обрез пар, Å.
const CUTOFF: f64 = 10.0;
/// Cap отталкивания LJ, кДж/моль.
const LJ_CAP: f64 = 8.0;
/// Максимум H-связи, кДж/моль.
const HB_ENERGY: f64 = 8.0;
/// Липофильная пара (3.2–4.5 Å, рампа), кДж/моль.
const LIPO_PAIR: f64 = 0.35;
/// Десольватационный штраф за похороненную полярность, кДж/моль.
const DESOLV_POLAR: f64 = 3.0;
/// Энтропийный штраф на вращаемую связь (как в pharma.rs), кДж/моль.
const TORSION_ENTROPY: f64 = 6.0;
/// Комнатная RT, кДж/моль.
const RT: f64 = R_GAS * T_ROOM;
/// Тритный сектор кручения: 360°/27 = 13⅓°.
const TORS_SECTOR: f64 = std::f64::consts::TAU / 27.0;

// ─── Подготовка лиганда ─────────────────────────────────────────────────

/// Вращаемая связь (торсионная степень свободы).
#[derive(Debug, Clone)]
pub struct Torsion {
    /// Узел конформера A (ближний к корню).
    pub a: usize,
    /// Узел конформера B (дальняя сторона).
    pub b: usize,
    /// Поддерево со стороны B (вращается).
    pub moving: Vec<usize>,
}

/// Подготовленный лиганд.
pub struct LigandPrep {
    pub graph: MoleculeGraph,
    pub conf: Conformer,
    /// Базовые координаты с центроидом в нуле (все узлы).
    pub base: Vec<[f64; 3]>,
    /// Эффективные заряды тяжёлых атомов (тяжёлый + H-облака), e.
    pub q_eff: Vec<f64>,
    /// ВдВ радиусы тяжёлых атомов, Å.
    pub vdw: Vec<f64>,
    /// Узел конформера → атом графа (None для H-узлов).
    pub node_atom: Vec<Option<usize>>,
    /// Вращаемые связи (BFS-порядок от корня).
    pub torsions: Vec<Torsion>,
    /// Доноры: (узел донора, H-узлы).
    pub donors: Vec<(usize, Vec<usize>)>,
    /// Акцепторные узлы (тяжёлые).
    pub acceptors: Vec<usize>,
    /// Число вращаемых связей (для энтропии).
    pub n_rot: usize,
    /// Формула Хилла.
    pub formula: String,
}

/// Амидная связь C(=O)–N? (кручение запрещено — барьер ~80 кДж/моль).
fn is_amide_bond(g: &MoleculeGraph, bi: usize) -> bool {
    let b = &g.bonds[bi];
    let pairs = [(b.a, b.b), (b.b, b.a)];
    for &(x, y) in &pairs {
        if g.atoms[x].symbol != "N" {
            continue;
        }
        for (nb, ord) in g.neighbors(y) {
            if nb != x
                && g.atoms[nb].symbol == "O"
                && matches!(ord, BondOrder::Double | BondOrder::Aromatic)
            {
                return true;
            }
        }
    }
    false
}

/// Подготовить лиганд из SMILES.
pub fn prepare_ligand(smiles: &str) -> Result<LigandPrep, String> {
    let g = super::resolve_input(smiles)?;
    let conf = super::geom3d::embed(&g)?;
    prepare_ligand_conf(g, conf)
}

/// Подготовить лиганд из готового графа и конформера
/// (SDF-путь: конформер файла приоритетнее укладчика, канон ступени B1).
pub fn prepare_ligand_conf(g: MoleculeGraph, conf: Conformer) -> Result<LigandPrep, String> {
    let n_nodes = conf.nodes.len();

    // Центроид в ноль
    let mut cx = [0.0f64; 3];
    for p in &conf.positions {
        cx[0] += p[0];
        cx[1] += p[1];
        cx[2] += p[2];
    }
    for k in 0..3 {
        cx[k] /= n_nodes.max(1) as f64;
    }
    let base: Vec<[f64; 3]> = conf
        .positions
        .iter()
        .map(|p| [p[0] - cx[0], p[1] - cx[1], p[2] - cx[2]])
        .collect();

    // Заряды: тяжёлый атом + H-облака (эффективный перенос на центр масс)
    let gc: GasteigerCharges = gasteiger(&g);
    let mut q_eff = vec![0.0f64; g.atoms.len()];
    for (ai, a) in g.atoms.iter().enumerate() {
        q_eff[ai] = gc.0[ai] + gc.1[ai] * a.h_count as f64;
    }

    // Узел → атом
    let mut node_atom = vec![None; n_nodes];
    for (ai, &ni) in conf.heavy_map.iter().enumerate() {
        node_atom[ni] = Some(ai);
    }

    // Смежность узлов
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n_nodes];
    for b in &conf.bonds {
        adj[b.a].push(b.b);
        adj[b.b].push(b.a);
    }

    // Вращаемые связи: из графа, минус амиды; BFS-порядок от узла атома 0
    let rotatable: Vec<usize> = g
        .rotatable_bonds()
        .into_iter()
        .filter(|&bi| !is_amide_bond(&g, bi))
        .collect();
    let root = conf.heavy_map[0];
    let mut order: Vec<usize> = vec![root];
    let mut seen = vec![false; n_nodes];
    seen[root] = true;
    let mut qi = 0;
    while qi < order.len() {
        let u = order[qi];
        qi += 1;
        for &w in &adj[u] {
            if !seen[w] {
                seen[w] = true;
                order.push(w);
            }
        }
    }
    let depth: Vec<usize> = {
        let mut d = vec![0usize; n_nodes];
        for &u in &order {
            for &w in &adj[u] {
                if d[w] == 0 && w != root {
                    d[w] = d[u] + 1;
                }
            }
        }
        d
    };

    let mut torsions = Vec::new();
    for &bi in &rotatable {
        let gb = &g.bonds[bi];
        let na = conf.heavy_map[gb.a];
        let nb = conf.heavy_map[gb.b];
        // поддерево со стороны более глубокого узла
        let (anchor, far) = if depth[na] <= depth[nb] {
            (na, nb)
        } else {
            (nb, na)
        };
        // BFS от far, не пересекая связь anchor-far
        let mut moving = vec![far];
        let mut vis = vec![false; n_nodes];
        vis[far] = true;
        vis[anchor] = true;
        let mut k = 0;
        while k < moving.len() {
            let u = moving[k];
            k += 1;
            for &w in &adj[u] {
                if !vis[w] {
                    vis[w] = true;
                    moving.push(w);
                }
            }
        }
        torsions.push(Torsion {
            a: anchor,
            b: far,
            moving,
        });
    }

    // Доноры/акцепторы лиганда (конвенция pharma.rs)
    let is_donor_atom = |g: &MoleculeGraph, i: usize| {
        matches!(g.atoms[i].symbol.as_str(), "N" | "O") && g.atoms[i].h_count > 0
    };
    let is_acceptor_atom = |g: &MoleculeGraph, i: usize| match g.atoms[i].symbol.as_str() {
        "O" => g.atoms[i].charge <= 0,
        "N" => g.atoms[i].h_count == 0 && g.atoms[i].charge <= 0,
        _ => false,
    };
    let mut donors = Vec::new();
    let mut acceptors = Vec::new();
    for (ai, &ni) in conf.heavy_map.iter().enumerate() {
        if is_donor_atom(&g, ai) {
            let hs: Vec<usize> = adj[ni]
                .iter()
                .copied()
                .filter(|&w| conf.nodes[w].is_h)
                .collect();
            donors.push((ni, hs));
        }
        if is_acceptor_atom(&g, ai) {
            acceptors.push(ni);
        }
    }

    let n_rot = torsions.len();
    let vdw: Vec<f64> = g.atoms.iter().map(|a| vdw_radius(&a.symbol)).collect();
    let formula = g.hill_formula();
    Ok(LigandPrep {
        formula,
        graph: g,
        conf,
        base,
        q_eff,
        vdw,
        node_atom,
        torsions,
        donors,
        acceptors,
        n_rot,
    })
}

// ─── Поле кармана (предвычисление) ──────────────────────────────────────

/// Плоское SoA-поле атомов рецептора вокруг кармана.
pub struct PocketField {
    /// Позиции, Å.
    pub pos: Vec<[f64; 3]>,
    /// Заряды, e.
    pub q: Vec<f64>,
    /// ВдВ радиусы, Å.
    pub vdw: Vec<f64>,
    /// Акцепторы H-связей.
    pub acceptor: Vec<bool>,
    /// Неполярные атомы (C/S/галогены с |q|<0.3) — для липофильной рампы.
    pub nonpolar: Vec<bool>,
    /// Донорные H: (индекс донора в поле, позиция H).
    pub donor_h: Vec<(usize, [f64; 3])>,
    /// Индексы атомов-источников в mm (для обновления поля при induced fit,
    // контур C); параллелен pos.
    pub src: Vec<usize>,
    /// Статистика 27-дерева (объектов, узлов, высота).
    pub tree27: (usize, usize, u8),
    /// Число атомов поля.
    pub len: usize,
}

/// Акцептор лиганд⇄белок по (остаток, атом).
fn is_receptor_acceptor(res_name: &str, atom_name: &str) -> bool {
    match atom_name {
        "O" | "OXT" => true, // карбонил хребта
        "OD1" | "OD2" => matches!(res_name, "ASP" | "ASN"),
        "OE1" | "OE2" => matches!(res_name, "GLU" | "GLN"),
        "OG" => res_name == "SER",
        "OG1" => res_name == "THR",
        "OH" => res_name == "TYR",
        "NE2" => res_name == "HIS", // HID-таутомер: ND1 донор, NE2 акцептор
        _ => false,
    }
}

/// Собрать поле кармана: атомы рецептора в сфере (центр, radius+10),
/// донорные H внутри, флаги акцепторов.
pub fn build_field(
    mm: &MacroMol,
    index: &ProteinIndex,
    charges: &[f64],
    donors: &[super::pdb::DonorH],
    center: [f64; 3],
    radius: f64,
) -> PocketField {
    let atoms = index.within(mm, center, radius + 10.0);
    // позиция → локальный индекс
    let mut local: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut pos = Vec::with_capacity(atoms.len());
    let mut q = Vec::with_capacity(atoms.len());
    let mut vdw = Vec::with_capacity(atoms.len());
    let mut acceptor = Vec::with_capacity(atoms.len());
    let mut nonpolar = Vec::with_capacity(atoms.len());
    for (k, &ai) in atoms.iter().enumerate() {
        local.insert(ai, k);
        pos.push(mm.atoms[ai].pos);
        q.push(charges[ai]);
        vdw.push(vdw_radius(mm.atoms[ai].elem_str()));
        acceptor.push(false);
        let el = mm.atoms[ai].elem_str();
        let np = matches!(el, "C" | "S" | "Se" | "F" | "Cl" | "Br" | "I") && charges[ai].abs() < 0.3;
        nonpolar.push(np);
    }
    // флаги акцепторов по имени атома/остатка
    for (k, &ai) in atoms.iter().enumerate() {
        let a = &mm.atoms[ai];
        let res = a.res_str();
        let nm = a.name_str();
        if is_receptor_acceptor(res, nm) && !a.het {
            acceptor[k] = true;
        }
        if a.het {
            // HETATM-ионы: SO4/PO4 кислороды — акцепторы
            if matches!(res, "SO4" | "PO4") && a.elem_str() == "O" {
                acceptor[k] = true;
            }
        }
    }
    // донорные H внутри поля
    let mut donor_h = Vec::new();
    for d in donors {
        if let Some(&k) = local.get(&d.donor) {
            donor_h.push((k, d.h));
        }
    }
    PocketField {
        len: atoms.len(),
        pos,
        q,
        vdw,
        acceptor,
        nonpolar,
        donor_h,
        src: atoms,
        tree27: index.stats,
    }
}

// ─── Скоринг ────────────────────────────────────────────────────────────

/// Слагаемые энергии позы, кДж/моль.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScoreTerms {
    pub e_vdw: f64,
    pub e_hb: f64,
    pub e_elec: f64,
    pub e_lipo: f64,
    pub e_desolv: f64,
    /// Энтропийный штраф кручений (константа позы).
    pub e_tors: f64,
    pub contacts: usize,
    pub hbonds: usize,
    pub clashes: usize,
    /// Полная ΔG.
    pub delta_g: f64,
}

impl ScoreTerms {
    /// Цель поиска (без константной энтропии).
    fn objective(&self) -> f64 {
        self.e_vdw + self.e_hb + self.e_elec + self.e_lipo + self.e_desolv
    }
}

fn dist2(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    dist2(a, b).sqrt()
}

/// Оценить позу лиганда в поле кармана.
pub fn score_pose(lig: &LigandPrep, field: &PocketField, pos: &[[f64; 3]]) -> ScoreTerms {
    let mut t = ScoreTerms::default();
    // Пары тяжёлый-лиганд × поле
    let mut hb_partner: Vec<bool> = vec![false; lig.conf.heavy_map.len()]; // по атомам графа
    let mut buried: Vec<usize> = vec![0; lig.conf.heavy_map.len()];
    for (ai, &ni) in lig.conf.heavy_map.iter().enumerate() {
        let p = pos[ni];
        let ri = lig.vdw[ai];
        let qi = lig.q_eff[ai];
        let sym_i = lig.graph.atoms[ai].symbol.as_str();
        let nonpolar_i = matches!(sym_i, "C" | "S" | "F" | "Cl" | "Br" | "I" | "Se");
        for k in 0..field.len {
            let r2 = dist2(p, field.pos[k]);
            if r2 > CUTOFF * CUTOFF {
                continue;
            }
            let r = r2.sqrt().max(0.35);
            let rj = field.vdw[k];
            let rij = ri + rj;
            if r < 4.5 {
                t.contacts += 1;
                buried[ai] += 1;
                if r < 0.72 * rij {
                    t.clashes += 1;
                }
            }
            // КОНТАКТНЫЙ ПОЛ: r_eff = max(r, 0.75·rij). Ниже 75% суммы
            // ВдВ-радиусов кулон сатурируется (LJ там и так capped 8) —
            // без пола кулон ВОЗНАГРАЖДАЛ столкновения (до −45 кДж/пара
            // на под-контактных дистанциях, MC выбирал мусорные позы с
            // 19 клатшами; поймано enrichment-критерием контура B5,
            // закрыто тестом clash_coulomb_saturation).
            let r_eff = if r < 0.75 * rij { 0.75 * rij } else { r };
            // LJ 12-6 soft-core (как pharma.rs)
            let eps = 0.42 * (ri * rj).sqrt() / 1.6;
            let x = rij / r_eff;
            let lj = 4.0 * eps * (x.powi(12) - x.powi(6));
            t.e_vdw += lj.min(LJ_CAP);
            // Кулон: дистанционно-зависимый диэлектрик ε(r) = 4r
            // (стандарт докинг-полей: экранировка растёт с расстоянием),
            // контактный пол — см. выше
            t.e_elec += K_ELEC * qi * field.q[k] / (EPS_R * r_eff * r_eff);
            // Липофильная рампа: неполярный × неполярный
            if nonpolar_i && field.nonpolar[k] && (3.2..=4.5).contains(&r) {
                let f = ((4.5 - r) / 1.3).clamp(0.0, 1.0);
                t.e_lipo -= LIPO_PAIR * f;
            }
        }
    }
    // H-связи: лиганд-донор → поле-акцептор (реальные H лиганда)
    for &(d_node, ref hs) in &lig.donors {
        let dp = pos[d_node];
        for k in 0..field.len {
            if !field.acceptor[k] {
                continue;
            }
            let ap = field.pos[k];
            let rda = dist(dp, ap);
            if rda > 4.2 {
                continue;
            }
            // ищем лучший H
            let mut best_e = 0.0f64;
            for &h_node in hs {
                let hp = pos[h_node];
                let rha = dist(hp, ap);
                if !(2.2..=3.5).contains(&rha) {
                    continue;
                }
                // угловой фактор: cos φ между D→H и H→A; φ=180° идеал
                let v1 = [hp[0] - dp[0], hp[1] - dp[1], hp[2] - dp[2]];
                let v2 = [ap[0] - hp[0], ap[1] - hp[1], ap[2] - hp[2]];
                let n1 = v1.iter().map(|x| x * x).sum::<f64>().sqrt();
                let n2 = v2.iter().map(|x| x * x).sum::<f64>().sqrt();
                if n1 < 1e-9 || n2 < 1e-9 {
                    continue;
                }
                let cosphi = (v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2]) / (n1 * n2);
                let f = ((1.0 - cosphi) / 2.0).powi(2);
                let e = -HB_ENERGY * f;
                if e < best_e {
                    best_e = e;
                }
            }
            if best_e < 0.0 {
                t.e_hb += best_e;
                t.hbonds += 1;
                if let Some(ai) = lig.node_atom[d_node] {
                    hb_partner[ai] = true;
                }
            }
        }
    }
    // H-связи: поле-донор (реконструированные H) → лиганд-акцептор
    for &(dk, hpos) in &field.donor_h {
        let dp = field.pos[dk];
        for &a_node in &lig.acceptors {
            let ap = pos[a_node];
            let rda = dist(dp, ap);
            if rda > 4.2 {
                continue;
            }
            let rha = dist(hpos, ap);
            if !(2.2..=3.5).contains(&rha) {
                continue;
            }
            let v1 = [hpos[0] - dp[0], hpos[1] - dp[1], hpos[2] - dp[2]];
            let v2 = [ap[0] - hpos[0], ap[1] - hpos[1], ap[2] - hpos[2]];
            let n1 = v1.iter().map(|x| x * x).sum::<f64>().sqrt();
            let n2 = v2.iter().map(|x| x * x).sum::<f64>().sqrt();
            if n1 < 1e-9 || n2 < 1e-9 {
                continue;
            }
            let cosphi = (v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2]) / (n1 * n2);
            let f = ((1.0 - cosphi) / 2.0).powi(2);
            t.e_hb += -HB_ENERGY * f;
            t.hbonds += 1;
            if let Some(ai) = lig.node_atom[a_node] {
                hb_partner[ai] = true;
            }
        }
    }
    // Десольватация: похороненная полярность без H-связи
    for (ai, &ni) in lig.conf.heavy_map.iter().enumerate() {
        let sym_i = lig.graph.atoms[ai].symbol.as_str();
        let polar = matches!(sym_i, "N" | "O") || lig.q_eff[ai].abs() >= 0.25;
        if polar && !hb_partner[ai] && buried[ai] >= 10 {
            let f = (buried[ai] as f64 / 24.0).min(1.0);
            t.e_desolv += DESOLV_POLAR * f;
        }
    }
    t.e_tors = TORSION_ENTROPY * lig.n_rot.min(12) as f64;
    t.delta_g = t.objective() + t.e_tors;
    t
}

// ─── Геометрия позы ─────────────────────────────────────────────────────

/// Состояние позы: жёсткое тело + кручения.
#[derive(Debug, Clone)]
pub struct PoseState {
    pub center: [f64; 3],
    pub rot: [[f64; 3]; 3],
    /// Кручения, радианы.
    pub torsions: Vec<f64>,
}

fn identity3() -> [[f64; 3]; 3] {
    [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ]
}

fn mat_mul(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut r = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    r
}

fn mat_vec(m: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

/// Матрица поворота Родригеса (ось + угол).
fn rot_matrix(axis: [f64; 3], angle: f64) -> [[f64; 3]; 3] {
    let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let u = if n < 1e-12 {
        [0.0, 0.0, 1.0]
    } else {
        [axis[0] / n, axis[1] / n, axis[2] / n]
    };
    let (s, c) = (angle.sin(), angle.cos());
    let d = 1.0 - c;
    [
        [
            c + u[0] * u[0] * d,
            u[0] * u[1] * d - u[2] * s,
            u[0] * u[2] * d + u[1] * s,
        ],
        [
            u[1] * u[0] * d + u[2] * s,
            c + u[1] * u[1] * d,
            u[1] * u[2] * d - u[0] * s,
        ],
        [
            u[2] * u[0] * d - u[1] * s,
            u[2] * u[1] * d + u[0] * s,
            c + u[2] * u[2] * d,
        ],
    ]
}

/// Вращение точек вокруг оси (p, u) на угол θ (Родригес).
fn rotate_points(pos: &mut [[f64; 3]], idx: &[usize], p: [f64; 3], u: [f64; 3], theta: f64) {
    let (s, c) = (theta.sin(), theta.cos());
    for &i in idx {
        let v = [pos[i][0] - p[0], pos[i][1] - p[1], pos[i][2] - p[2]];
        let d = v[0] * u[0] + v[1] * u[1] + v[2] * u[2];
        let cr = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        pos[i] = [
            p[0] + v[0] * c + cr[0] * s + u[0] * d * (1.0 - c),
            p[1] + v[1] * c + cr[1] * s + u[1] * d * (1.0 - c),
            p[2] + v[2] * c + cr[2] * s + u[2] * d * (1.0 - c),
        ];
    }
}

/// Применить состояние к базовым координатам → позиции узлов.
pub fn apply_state(lig: &LigandPrep, st: &PoseState) -> Vec<[f64; 3]> {
    let mut pos = lig.base.clone();
    // кручения по порядку (вложения корректны: оси — текущие координаты)
    for (ti, t) in lig.torsions.iter().enumerate() {
        let theta = st.torsions[ti];
        if theta.abs() < 1e-12 {
            continue;
        }
        let pa = pos[t.a];
        let pb = pos[t.b];
        let axis = [
            pb[0] - pa[0],
            pb[1] - pa[1],
            pb[2] - pa[2],
        ];
        let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        if n < 1e-9 {
            continue;
        }
        let u = [axis[0] / n, axis[1] / n, axis[2] / n];
        rotate_points(&mut pos, &t.moving, pa, u, theta);
    }
    // жёсткое тело
    for p in pos.iter_mut() {
        *p = mat_vec(&st.rot, *p);
        p[0] += st.center[0];
        p[1] += st.center[1];
        p[2] += st.center[2];
    }
    pos
}

// ─── RNG (детерминированный) ────────────────────────────────────────────

pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Rng(if seed == 0 { 0x9E3779B97F4A7C15 } else { seed })
    }
    pub(crate) fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    pub(crate) fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub(crate) fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f64()
    }
    /// Случайная точка в сфере радиуса r.
    fn in_sphere(&mut self, r: f64) -> [f64; 3] {
        loop {
            let p = [self.range(-r, r), self.range(-r, r), self.range(-r, r)];
            if p[0] * p[0] + p[1] * p[1] + p[2] * p[2] <= r * r {
                return p;
            }
        }
    }
    /// Случайная ось на сфере.
    fn axis(&mut self) -> [f64; 3] {
        let z = self.range(-1.0, 1.0);
        let phi = self.range(0.0, std::f64::consts::TAU);
        let r = (1.0 - z * z).sqrt();
        [r * phi.cos(), r * phi.sin(), z]
    }
}

/// FNV-1a хэш (сид).
pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ─── Результат ──────────────────────────────────────────────────────────

/// Кластер поз (сводка).
#[derive(Debug, Clone)]
pub struct PoseSummary {
    pub energy: f64,
    pub size: usize,
    pub torsions: Vec<f64>,
    /// RMSD представителя кластера к лучшей позе, Å (None у лучшего) —
    /// мера разнообразия поз (кластер-RMSD скрининга, контур B).
    pub diversity: Option<f64>,
}

/// Итог докинга.
pub struct DockResult {
    pub ligand_formula: String,
    pub pocket_center: [f64; 3],
    pub pocket_radius: f64,
    pub pocket_method: &'static str,
    /// Остатки кармана (имя+цепь+номер), ближайшие к лучшей позе.
    pub pocket_residues: Vec<String>,
    pub best: ScoreTerms,
    /// Лучшие позиции узлов лиганда.
    pub best_positions: Vec<[f64; 3]>,
    pub best_torsions: Vec<f64>,
    /// Кластеры (энергия, размер).
    pub clusters: Vec<PoseSummary>,
    /// RMSD к кристаллу, Å (redocking).
    pub rmsd: Option<f64>,
    /// Ансамбль: ln Z = logsumexp(−E_i/RT).
    pub ln_z: f64,
    /// ΔG ансамбля = −RT(ln Z − ln N).
    pub dg_ensemble: f64,
    pub ln_kd: f64,
    pub log10_kd: f64,
    pub trit_depth: f64,
    /// Число вычислений скоринга.
    pub n_evals: usize,
    pub elapsed_ms: f64,
    pub tree27: (usize, usize, u8),
    pub n_rot: usize,
    pub runs: usize,
    pub steps: usize,
    pub trit_verdict: i8,
}

// ─── Поиск: MC/SA + шлифовка ────────────────────────────────────────────

/// Параметры докинга.
#[derive(Debug, Clone)]
pub struct DockParams {
    /// Число независимых прогонов.
    pub runs: usize,
    /// Шагов Метрополиса на прогон.
    pub steps: usize,
    /// Сид (0 → от FNV(SMILES)).
    pub seed: u64,
    /// Начальная «температура» kT, кДж/моль.
    pub t0: f64,
    /// Конечная kT.
    pub t1: f64,
    /// JIT-ядро скоринга (контур A3): AVX2-машинный код, таблицы кармана
    /// вшиты в исполняемую страницу. Без AVX2/малого поля — прозрачный откат.
    pub jit: bool,
}

impl Default for DockParams {
    fn default() -> Self {
        DockParams {
            runs: 12,
            steps: 2200,
            seed: 0,
            t0: 12.0,
            t1: 0.4,
            jit: true,
        }
    }
}

/// Спецификация кармана.
pub enum PocketSpec {
    /// По кристаллическому лиганду (redocking-протокол).
    Auto,
    /// Слепой геометрический поиск.
    Blind,
    /// Явный центр (x,y,z Å).
    Point([f64; 3], f64),
}

impl DockResult {
    /// logsumexp — лог-домен ансамбля.
    fn logsumexp(vals: &[f64]) -> f64 {
        let m = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if !m.is_finite() {
            return m;
        }
        m + vals.iter().map(|v| (v - m).exp()).sum::<f64>().ln()
    }
}

/// Кластеризация поз: представители с RMSD ≥ 1.0 Å — разные кластеры.
fn cluster_poses(
    lig: &LigandPrep,
    poses: &[(f64, Vec<[f64; 3]>, Vec<f64>)],
) -> Vec<PoseSummary> {
    let heavy: Vec<usize> = lig.conf.heavy_map.clone();
    let mut sorted: Vec<&(f64, Vec<[f64; 3]>, Vec<f64>)> = poses.iter().collect();
    sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut reps: Vec<(f64, Vec<f64>, usize)> = Vec::new(); // (энергия, кручения, размер)
    let mut rep_pos: Vec<Vec<[f64; 3]>> = Vec::new();
    for p in sorted {
        let mut merged = false;
        for (ri, rp) in rep_pos.iter().enumerate() {
            let mut s = 0.0f64;
            for &ni in &heavy {
                let dx = p.1[ni][0] - rp[ni][0];
                let dy = p.1[ni][1] - rp[ni][1];
                let dz = p.1[ni][2] - rp[ni][2];
                s += dx * dx + dy * dy + dz * dz;
            }
            let rmsd = (s / heavy.len().max(1) as f64).sqrt();
            if rmsd < 1.0 {
                reps[ri].2 += 1;
                merged = true;
                break;
            }
        }
        if !merged {
            reps.push((p.0, p.2.clone(), 1));
            rep_pos.push(p.1.clone());
        }
        if reps.len() >= 16 {
            break;
        }
    }
    reps.into_iter()
        .zip(rep_pos.iter())
        .enumerate()
        .map(|(i, ((e, t, size), rp))| PoseSummary {
            energy: e,
            torsions: t,
            size,
            diversity: if i == 0 {
                None
            } else {
                // RMSD к представителю лучшего кластера (тяжёлые атомы)
                let mut s = 0.0f64;
                for &ni in &heavy {
                    let dx = rp[ni][0] - rep_pos[0][ni][0];
                    let dy = rp[ni][1] - rep_pos[0][ni][1];
                    let dz = rp[ni][2] - rep_pos[0][ni][2];
                    s += dx * dx + dy * dy + dz * dz;
                }
                Some((s / heavy.len().max(1) as f64).sqrt())
            },
        })
        .collect()
}

/// RMSD к кристаллическому лиганду: жадное спаривание по элементам.
pub fn rmsd_to_crystal(
    lig: &LigandPrep,
    pos: &[[f64; 3]],
    crystal_atoms: &[super::pdb::PdbAtom],
) -> Option<f64> {
    if crystal_atoms.is_empty() {
        return None;
    }
    // пары (расстояние, i, j) по элементам
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for (ai, &ni) in lig.conf.heavy_map.iter().enumerate() {
        let el_i = lig.graph.atoms[ai].symbol.as_str();
        for (j, ca) in crystal_atoms.iter().enumerate() {
            if ca.elem_str() != el_i {
                continue;
            }
            let d = dist(pos[ni], ca.pos);
            pairs.push((d, ai, j));
        }
    }
    pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut used_i = vec![false; lig.conf.heavy_map.len()];
    let mut used_j = vec![false; crystal_atoms.len()];
    let mut sum = 0.0f64;
    let mut n = 0usize;
    for &(d, i, j) in &pairs {
        if used_i[i] || used_j[j] {
            continue;
        }
        used_i[i] = true;
        used_j[j] = true;
        sum += d * d;
        n += 1;
    }
    if n == 0 {
        return None;
    }
    Some((sum / n as f64).sqrt())
}

/// Шлифовка: координатный спуск по кручениям (27 секторов) и жёсткому телу.
fn polish(
    lig: &LigandPrep,
    field: &PocketField,
    kernel: Option<&crate::graph::chem_kernel::JitScoringKernel>,
    y_buf: &mut [f64],
    st: &mut PoseState,
    pos: &mut Vec<[f64; 3]>,
    cur_e: &mut f64,
    evals: &mut usize,
) {
    for _sweep in 0..4 {
        let mut improved = false;
        // кручения: ±1..3 трит-сектора
        for ti in 0..st.torsions.len() {
            let base = st.torsions[ti];
            let mut best = (*cur_e, 0.0f64);
            for k in 1..=3i32 {
                for sgn in [-1.0f64, 1.0] {
                    let delta = sgn * k as f64 * TORS_SECTOR;
                    st.torsions[ti] = base + delta;
                    let cand = apply_state(lig, st);
                    let t = match kernel {
                    Some(k) => {
                        for v in y_buf.iter_mut() {
                            *v = 0.0;
                        }
                        k.exec_into(&cand, y_buf);
                        k.terms_from_y(lig, y_buf)
                    }
                    None => score_pose(lig, field, &cand),
                };
                    *evals += 1;
                    if t.objective() < best.0 {
                        best = (t.objective(), delta);
                    }
                }
            }
            st.torsions[ti] = base + best.1;
            if best.1 != 0.0 {
                improved = true;
                *cur_e = best.0;
                *pos = apply_state(lig, st);
            } else {
                st.torsions[ti] = base;
            }
        }
        // жёсткое тело: 6 трансляций + 3 поворота
        let axes: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        for ax in axes {
            for sgn in [-1.0f64, 1.0] {
                let save = st.center;
                st.center = [
                    st.center[0] + sgn * ax[0] * 0.25,
                    st.center[1] + sgn * ax[1] * 0.25,
                    st.center[2] + sgn * ax[2] * 0.25,
                ];
                let cand = apply_state(lig, st);
                let t = match kernel {
                    Some(k) => {
                        for v in y_buf.iter_mut() {
                            *v = 0.0;
                        }
                        k.exec_into(&cand, y_buf);
                        k.terms_from_y(lig, y_buf)
                    }
                    None => score_pose(lig, field, &cand),
                };
                *evals += 1;
                if t.objective() < *cur_e {
                    *cur_e = t.objective();
                    *pos = cand;
                    improved = true;
                } else {
                    st.center = save;
                }
            }
        }
        for (ai, &axis) in axes.iter().enumerate() {
            let save = st.rot;
            let m = rot_matrix(axis, 4.0f64.to_radians());
            st.rot = mat_mul(&m, &st.rot);
            let cand = apply_state(lig, st);
            let t = match kernel {
                    Some(k) => {
                        for v in y_buf.iter_mut() {
                            *v = 0.0;
                        }
                        k.exec_into(&cand, y_buf);
                        k.terms_from_y(lig, y_buf)
                    }
                    None => score_pose(lig, field, &cand),
                };
            *evals += 1;
            if t.objective() < *cur_e {
                *cur_e = t.objective();
                *pos = cand;
                improved = true;
            } else {
                st.rot = save;
            }
            let _ = ai;
        }
        if !improved {
            break;
        }
    }
}

/// Главный вход: докинг SMILES-лиганда в белок из PDB.
pub fn dock(smiles: &str, mm: &MacroMol, params: &DockParams, spec: &PocketSpec) -> Result<DockResult, String> {
    let lig = prepare_ligand(smiles)?;
    if lig.conf.heavy_map.is_empty() {
        return Err("лиганд без тяжёлых атомов".into());
    }
    let ctx = prepare_dock_context(mm, spec)?;
    dock_prepared(mm, &ctx, &lig, smiles, params)
}

/// Контекст докинга: всё, что зависит только от белка и кармана.
/// Строится ОДИН раз и переиспользуется всеми лигандами скрининга
/// (контур B — главный источник скорости: PocketField в Arc, только чтение).
pub struct DockContext {
    /// United-заряды рецептора (по всем атомам mm).
    pub charges: Vec<f64>,
    /// Реконструированные донорные H.
    pub donors: Vec<super::pdb::DonorH>,
    /// 27-дерево рецептора.
    pub index: ProteinIndex,
    /// Карман связывания.
    pub pocket: Pocket,
    /// Плоское SoA-поле кармана (общее для всех лигандов).
    pub field: PocketField,
    /// Кристаллический лиганд (эталон RMSD, redocking) — пуст вне Auto.
    pub crystal_atoms: Vec<super::pdb::PdbAtom>,
}

/// Собрать контекст докинга: рецептор, заряды, доноры, 27-дерево, карман,
/// поле. Лиганд-независимая часть `dock()`.
pub fn prepare_dock_context(mm: &MacroMol, spec: &PocketSpec) -> Result<DockContext, String> {
    let receptor = mm.receptor_atoms();
    if receptor.is_empty() {
        return Err("в PDB нет атомов рецептора (белок/ионы)".into());
    }
    let charges = receptor_charges(mm);
    let donors = reconstruct_donors(mm);
    let index = ProteinIndex::build(mm, &receptor);

    // Карман
    let crystal_lig: Option<usize> = match spec {
        PocketSpec::Auto => {
            // первый лиганд (не вода/ион) — источник кармана и эталон RMSD
            mm.ligand_residues().first().copied()
        }
        _ => None,
    };
    let pocket: Pocket = match spec {
        PocketSpec::Auto => {
            let l = crystal_lig.ok_or("PocketSpec::Auto, но лиганда в PDB нет — используйте blind или точку")?;
            super::pdb::pocket_from_ligand(mm, l)
        }
        PocketSpec::Blind => super::pdb::pocket_blind(mm)?,
        PocketSpec::Point(c, r) => super::pdb::pocket_at_point(mm, *c, *r),
    };

    // Поле кармана (tree27 → плоские массивы)
    let field = build_field(mm, &index, &charges, &donors, pocket.center, pocket.radius + 2.0);

    // Эталон RMSD
    let crystal_atoms: Vec<super::pdb::PdbAtom> = crystal_lig
        .map(|l| {
            mm.residue_atom_ids(l)
                .into_iter()
                .filter(|&i| mm.atoms[i].elem_str() != "H")
                .map(|i| mm.atoms[i])
                .collect()
        })
        .unwrap_or_default();

    Ok(DockContext {
        charges,
        donors,
        index,
        pocket,
        field,
        crystal_atoms,
    })
}

/// Докинг готового лиганда в готовый контекст (ядро `dock()`, контур B).
/// `seed_key` — ключ детерминизма сида: SMILES или имя SDF-записи;
/// результат НЕ зависит от порядка/числа вызовов (сид = FNV(seed_key|карман)).
pub fn dock_prepared(
    mm: &MacroMol,
    ctx: &DockContext,
    lig: &LigandPrep,
    seed_key: &str,
    params: &DockParams,
) -> Result<DockResult, String> {
    let t_start = std::time::Instant::now();
    if lig.conf.heavy_map.is_empty() {
        return Err("лиганд без тяжёлых атомов".into());
    }
    let field = &ctx.field;
    let pocket = &ctx.pocket;
    let crystal_atoms = &ctx.crystal_atoms;
    let index = &ctx.index;

    // JIT-ядро скоринга (контур A3): таблицы кармана → машинный код.
    // Прозрачный откат: без AVX2/малого поля остаётся score_pose.
    let jit_kernel: Option<crate::graph::chem_kernel::JitScoringKernel> = if params.jit {
        crate::graph::chem_kernel::compile_scoring_kernel(lig, field).ok()
    } else {
        None
    };
    // единая точка скоринга: JIT или интерпретатор; y-буфер ПЕРЕИСПОЛЬЗУЕТСЯ
    // (zero-alloc в MC-цикле; kernel пишет поверх, слоты 3/4 и hb-флаги
    // аккумулируются от нуля — перезануляем перед вызовом)
    let mut y_buf: Vec<f64> = jit_kernel
        .as_ref()
        .map(|k| vec![0.0f64; k.n_slots()])
        .unwrap_or_default();
    let score_of = |pos: &Vec<[f64; 3]>, y: &mut Vec<f64>| -> ScoreTerms {
        match &jit_kernel {
            Some(k) => {
                for v in y.iter_mut() {
                    *v = 0.0;
                }
                k.exec_into(pos, y);
                k.terms_from_y(lig, y)
            }
            None => score_pose(lig, field, pos),
        }
    };

    // Сид
    let seed = if params.seed != 0 {
        params.seed
    } else {
        fnv1a(format!("{}|{}|{}", seed_key, pocket.method, pocket.radius).as_bytes())
    };

    let mut n_evals = 0usize;
    // Ансамбль: (objective, positions, torsions)
    let mut ensemble: Vec<(f64, Vec<[f64; 3]>, Vec<f64>)> = Vec::new();

    let mut best_state: Option<PoseState> = None;
    let mut best_obj = f64::INFINITY;
    let mut best_positions: Vec<[f64; 3]> = Vec::new();
    let mut best_terms = ScoreTerms::default();

    for run in 0..params.runs {
        let mut rng = Rng::new(seed ^ (fnv1a(&run.to_le_bytes()) << 1));
        // начальная поза: центр кармана + джиттер, случайная ориентация
        let jitter = rng.in_sphere(1.2);
        let axis = rng.axis();
        let ang = rng.range(0.0, std::f64::consts::TAU);
        let mut st = PoseState {
            center: [
                pocket.center[0] + jitter[0],
                pocket.center[1] + jitter[1],
                pocket.center[2] + jitter[2],
            ],
            rot: rot_matrix(axis, ang),
            torsions: (0..lig.torsions.len())
                .map(|_| rng.range(-std::f64::consts::PI, std::f64::consts::PI))
                .collect(),
        };
        let mut pos = apply_state(lig, &st);
        let mut terms = score_of(&pos, &mut y_buf);
        n_evals += 1;
        let mut cur_obj = terms.objective();
        ensemble.push((cur_obj, pos.clone(), st.torsions.clone()));
        if cur_obj < best_obj {
            best_obj = cur_obj;
            best_state = Some(st.clone());
            best_positions = pos.clone();
            best_terms = terms;
        }

        for step in 0..params.steps {
            let frac = step as f64 / params.steps.max(1) as f64;
            let kt = params.t0 * (params.t1 / params.t0).powf(frac);
            // предложение
            let u = rng.f64();
            let mut cand = st.clone();
            if u < 0.55 && !st.torsions.is_empty() {
                // трит-кручение: ±1..3 сектора по 13⅓°
                let ti = (rng.next_u64() % cand.torsions.len() as u64) as usize;
                let mag = 1.0 + (rng.next_u64() % 3) as f64;
                let sgn = if rng.f64() < 0.5 { -1.0 } else { 1.0 };
                cand.torsions[ti] += sgn * mag * TORS_SECTOR;
            } else if u < 0.85 {
                // жёсткое тело
                let v = rng.in_sphere(0.8);
                cand.center = [
                    cand.center[0] + v[0],
                    cand.center[1] + v[1],
                    cand.center[2] + v[2],
                ];
                let axis = rng.axis();
                let ang = rng.range(-0.3, 0.3);
                let m = rot_matrix(axis, ang);
                cand.rot = mat_mul(&m, &cand.rot);
            } else if !st.torsions.is_empty() {
                // два кручения сразу
                let t1 = (rng.next_u64() % cand.torsions.len() as u64) as usize;
                let t2 = (rng.next_u64() % cand.torsions.len() as u64) as usize;
                for &ti in &[t1, t2] {
                    let mag = 1.0 + (rng.next_u64() % 2) as f64;
                    let sgn = if rng.f64() < 0.5 { -1.0 } else { 1.0 };
                    cand.torsions[ti] += sgn * mag * TORS_SECTOR;
                }
            }
            // мягкая стена кармана
            if dist(cand.center, pocket.center) > pocket.radius + 1.5 {
                continue;
            }
            let cand_pos = apply_state(lig, &cand);
            let cand_terms = score_of(&cand_pos, &mut y_buf);
            n_evals += 1;
            let new_obj = cand_terms.objective();
            let de = new_obj - cur_obj;
            if de <= 0.0 || rng.f64() < (-de / kt).exp() {
                st = cand;
                pos = cand_pos;
                cur_obj = new_obj;
                terms = cand_terms;
                if new_obj < best_obj {
                    best_obj = new_obj;
                    best_state = Some(st.clone());
                    best_positions = pos.clone();
                    best_terms = terms;
                }
                // в ансамбль — улучшения
                if ensemble.len() < 96 || new_obj < ensemble.last().map(|e| e.0).unwrap_or(f64::INFINITY) {
                    ensemble.push((new_obj, pos.clone(), st.torsions.clone()));
                }
            }
        }
        let _ = &mut terms;
    }

    // Шлифовка лучшей позы
    if let Some(mut st) = best_state.take() {
        let mut pos = best_positions.clone();
        let mut e = best_obj;
        polish(lig, field, jit_kernel.as_ref(), &mut y_buf, &mut st, &mut pos, &mut e, &mut n_evals);
        best_state = Some(st);
        best_positions = pos;
        best_obj = e;
        let terms = score_of(&best_positions, &mut y_buf);
        n_evals += 1;
        best_terms = terms;
        ensemble.push((best_obj, best_positions.clone(), best_state.as_ref().unwrap().torsions.clone()));
    }

    // Финальный скоринг лучшей позы (JIT-ядро, если доступно)
    let best = score_of(&best_positions, &mut y_buf);
    n_evals += 1;

    // Кластеры
    ensemble.push((best.objective(), best_positions.clone(), best_state.as_ref().map(|s| s.torsions.clone()).unwrap_or_default()));
    ensemble.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    ensemble.truncate(64);
    let clusters = cluster_poses(lig, &ensemble);

    // Лог-домен ансамбля
    let ln_z = {
        let vals: Vec<f64> = ensemble.iter().map(|(e, _, _)| -e / RT).collect();
        DockResult::logsumexp(&vals)
    };
    let n_poses = ensemble.len().max(1);
    let dg_ensemble = -RT * (ln_z - (n_poses as f64).ln());

    // RMSD
    let rmsd = rmsd_to_crystal(lig, &best_positions, crystal_atoms);

    // Kd
    let delta_g = best.delta_g;
    let ln_kd = delta_g / RT;
    let log10_kd = ln_kd / LN_10;
    let trit_depth = -ln_kd / LN_3;

    // Остатки кармана возле лучшей позы
    let near_atoms = index.within(mm, pocket.center, pocket.radius);
    let mut pocket_residues: Vec<String> = Vec::new();
    {
        // контактные остатки: атомы в 4.5 Å от лиганда
        let mut res_set: Vec<usize> = Vec::new();
        for &ni in &lig.conf.heavy_map {
            let hits = index.within(mm, best_positions[ni], 4.5);
            for h in hits {
                // найти остаток атома h
                if let Some(ri) = mm.residue_of_atom(h) {
                    if !res_set.contains(&ri) {
                        res_set.push(ri);
                    }
                }
            }
        }
        for &ri in &res_set {
            let r = &mm.residues[ri];
            pocket_residues.push(format!(
                "{}{}{}",
                r.name_str(),
                if r.chain == 0 { '_' } else { r.chain as char },
                r.seq
            ));
        }
        let _ = near_atoms;
    }

    let trit_verdict = if delta_g < -25.0 {
        1
    } else if delta_g > 5.0 {
        -1
    } else {
        0
    };

    let elapsed_ms = t_start.elapsed().as_secs_f64() * 1000.0;

    Ok(DockResult {
        ligand_formula: lig.formula.clone(),
        pocket_center: pocket.center,
        pocket_radius: pocket.radius,
        pocket_method: pocket.method,
        pocket_residues,
        best,
        best_positions,
        best_torsions: best_state.as_ref().map(|s| s.torsions.clone()).unwrap_or_default(),
        clusters,
        rmsd,
        ln_z,
        dg_ensemble,
        ln_kd,
        log10_kd,
        trit_depth,
        n_evals,
        elapsed_ms,
        tree27: field.tree27,
        n_rot: lig.n_rot,
        runs: params.runs,
        steps: params.steps,
        trit_verdict,
    })
}

// ─── Запись позы в PDB ──────────────────────────────────────────────────

/// Записать лучшую позу как PDB-файл (HETATM LIG) — читается обратно
/// парсером движка для --view-complex.
pub fn write_dock_pdb(
    path: &str,
    lig: &LigandPrep,
    pos: &[[f64; 3]],
    res: &DockResult,
) -> Result<(), String> {
    use std::io::Write;
    let mut out = String::new();
    out.push_str("REMARK   1 POLER-ENGINE DOCKED POSE (v0.75.0)\n");
    out.push_str(&format!(
        "REMARK   2 LIGAND {} ROTATABLE {} POCKET {} R={:.2}\n",
        res.ligand_formula, res.n_rot, res.pocket_method, res.pocket_radius
    ));
    out.push_str(&format!(
        "REMARK   3 dG={:.2} kJ/mol EvdW={:.2} EHB={:.2} Eelec={:.2} Elipo={:.2} Edesolv={:.2} Etors={:.2}\n",
        res.best.delta_g, res.best.e_vdw, res.best.e_hb, res.best.e_elec, res.best.e_lipo,
        res.best.e_desolv, res.best.e_tors
    ));
    out.push_str(&format!(
        "REMARK   4 Kd=e^({:.2}) M  HBONDS={} CONTACTS={} CLASHES={}\n",
        res.ln_kd, res.best.hbonds, res.best.contacts, res.best.clashes
    ));
    let mut serial = 1u32;
    let mut elem_counter: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    // тяжёлые атомы + H; имена ≤3 символов (колонки 13–16)
    for (ni, node) in lig.conf.nodes.iter().enumerate() {
        let counter = elem_counter.entry(node.symbol.clone()).or_insert(0);
        *counter += 1;
        let name = if node.is_h {
            format!("H{}", (*counter).min(99))
        } else {
            format!("{}{}", node.symbol, (*counter).min(99))
        };
        // 4-символьное поле имени: 1-буквенный элемент → с 14-й колонки
        let nm = if node.symbol.len() >= 2 && !node.is_h {
            format!("{:>4}", name)
        } else {
            format!(" {:<3}", name)
        };
        // Байтовая раскладка PDB: HETATM(0-5) serial(6-10) ' '(11) name(12-15)
        // ' '(16 altLoc) res(17-19) ' '(20) chain(21) seq(22-25) ' '(26)
        // '   '(27-29) X(30-37) Y(38-45) Z(46-54) occ(54-59) B(60-65)
        // 10×' '(66-75) elem(76-77)
        out.push_str(&format!(
            "HETATM{:>5} {} {:>3} X{:>4}    {:8.3}{:8.3}{:8.3}{:6.2}{:6.2}          {:>2}\n",
            serial,
            nm,
            "LIG",
            1,
            pos[ni][0],
            pos[ni][1],
            pos[ni][2],
            1.0,
            0.0,
            node.symbol
        ));
        serial += 1;
    }
    out.push_str("END\n");
    let mut f = std::fs::File::create(path).map_err(|e| format!("запись {path}: {e}"))?;
    f.write_all(out.as_bytes()).map_err(|e| format!("запись: {e}"))?;
    Ok(())
}

// ─── Отчёт ──────────────────────────────────────────────────────────────

/// Человекочитаемый отчёт докинга («голос учёного»).
pub fn dock_text(r: &DockResult, ligand_input: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "═══ Докинг: {ligand_input} ({}) ⇄ карман {} ═══\n",
        r.ligand_formula, r.pocket_method
    ));
    s.push_str(&format!(
        "[1] Карман: центр ({:.2}, {:.2}, {:.2}) Å, радиус {:.1} Å; остатки контакта ({}): {}.\n",
        r.pocket_center[0],
        r.pocket_center[1],
        r.pocket_center[2],
        r.pocket_radius,
        r.pocket_residues.len(),
        if r.pocket_residues.is_empty() {
            "—".into()
        } else {
            r.pocket_residues
                .iter()
                .take(14)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
                + if r.pocket_residues.len() > 14 { "…" } else { "" }
        }
    ));
    s.push_str(&format!(
        "[2] Лиганд: {} вращаемых связей; поиск: {} прогонов × {} шагов MC/SA (кручения трит-секторами 13⅓°, kT 12→0.4 кДж/моль); {} вычислений скоринга за {:.0} мс.\n",
        r.n_rot, r.runs, r.steps, r.n_evals, r.elapsed_ms
    ));
    s.push_str(&format!(
        "[3] Лучшая поза: E_vdW = {:+.1}, E_HB = {:+.1} ({} связей), E_elec = {:+.1}, E_lipo = {:+.1}, E_desolv = {:+.1}, E_tors = {:+.1} кДж/моль; контактов {}, стерических конфликтов {}.\n",
        r.best.e_vdw,
        r.best.e_hb,
        r.best.hbonds,
        r.best.e_elec,
        r.best.e_lipo,
        r.best.e_desolv,
        r.best.e_tors,
        r.best.contacts,
        r.best.clashes
    ));
    s.push_str(&format!(
        "[4] Термодинамика: ΔG = {:+.2} кДж/моль; Kd = e^({:.2}) = 10^({:.2}) М — лог-домен; тритная глубина Kd = 3^({:+.2}); ансамбль {} поз: ln Z = {:.2}, ΔG_анс = {:+.2} кДж/моль (logsumexp).\n",
        r.best.delta_g, r.ln_kd, r.log10_kd, r.trit_depth, r.clusters.iter().map(|c| c.size).sum::<usize>(), r.ln_z, r.dg_ensemble
    ));
    if !r.clusters.is_empty() {
        let c0 = &r.clusters[0];
        s.push_str(&format!(
            "[5] Кластеры поз: {} (лучший: {:.1} кДж/моль, {} поз; RMSD-порог 1.0 Å){}.\n",
            r.clusters.len(),
            c0.energy,
            c0.size,
            if r.clusters.len() > 1 {
                format!(
                    "; следующий: {:.1} кДж/моль × {}",
                    r.clusters[1].energy, r.clusters[1].size
                )
            } else {
                String::new()
            }
        ));
    }
    s.push_str("Примечание: ΔG — энергия лучшей позы скоринг-функции (LJ/кулоновский/H-связи/липофильность, без полной десольватационной энтропии) — ранжирует позы, но абсолютное значение систематически глубже экспериментального; Kd из неё — сравнительная оценка.\n");
    if let Some(rmsd) = r.rmsd {
        let verdict = if rmsd <= 2.0 {
            "УСПЕХ (академический стандарт redocking ≤ 2 Å)"
        } else if rmsd <= 3.0 {
            "приемлемо (пограничная поза)"
        } else {
            "мимо кристалла (ошибка позы)"
        };
        s.push_str(&format!(
            "[6] Redocking-валидация: RMSD к кристаллу = {:.2} Å — {verdict}.\n",
            rmsd
        ));
    }
    s.push_str(&format!(
        "[7] Топология: 27-дерево рецептора — {} объектов, {} узлов, высота {} (O(log₂₇ N)).\n",
        r.tree27.0, r.tree27.1, r.tree27.2
    ));
    s.push_str(&format!(
        "Трит-вердикт: {} — {}.",
        match r.trit_verdict {
            1 => "+1",
            -1 => "-1",
            _ => "0",
        },
        match r.trit_verdict {
            1 => "связывание сильное: поза энергетически выгодна",
            -1 => "связывание невыгодно",
            _ => "умеренное связывание",
        }
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Мини-«рецептор»: 4 ALA у кармана (упрощённая сетка координат).
    fn mini_pdb() -> &'static str {
        "\
ATOM      1  N   ALA A   1      -4.000   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1      -2.500   0.000   0.000  1.00 10.00           C
ATOM      3  C   ALA A   1      -1.000   0.500   0.000  1.00 10.00           C
ATOM      4  O   ALA A   1      -1.000  -0.700   0.000  1.00 10.00           O
ATOM      5  N   ALA A   2       4.000   0.000   0.000  1.00 10.00           N
ATOM      6  CA  ALA A   2       2.500   0.000   0.000  1.00 10.00           C
ATOM      7  C   ALA A   2       1.000   0.500   0.000  1.00 10.00           C
ATOM      8  O   ALA A   2       1.000  -0.700   0.000  1.00 10.00           O
ATOM      9  N   ALA A   3       0.000   4.000   0.000  1.00 10.00           N
ATOM     10  CA  ALA A   3       0.000   2.500   0.000  1.00 10.00           C
ATOM     11  C   ALA A   3       0.000   1.000   0.500  1.00 10.00           C
ATOM     12  O   ALA A   3       0.000   1.000  -0.700  1.00 10.00           O
ATOM     13  N   ALA A   4       0.000  -4.000   0.000  1.00 10.00           N
ATOM     14  CA  ALA A   4       0.000  -2.500   0.000  1.00 10.00           C
ATOM     15  C   ALA A   4       0.000  -1.000   0.500  1.00 10.00           C
ATOM     16  O   ALA A   4       0.000  -1.000  -0.700  1.00 10.00           O
HETATM   17  C1  BEN A  10       0.000   0.000   3.500  1.00 12.00           C
HETATM   18  C2  BEN A  10       1.400   0.000   3.500  1.00 12.00           C
HETATM   19  C3  BEN A  10       2.100   1.212   3.500  1.00 12.00           C
HETATM   20  C4  BEN A  10       1.400   2.425   3.500  1.00 12.00           C
HETATM   21  C5  BEN A  10       0.000   2.425   3.500  1.00 12.00           C
HETATM   22  C6  BEN A  10      -0.700   1.212   3.500  1.00 12.00           C
HETATM   23  N1  BEN A  10       0.000   0.000   6.000  1.00 12.00           N
HETATM   24  N2  BEN A  10       0.000   0.000   7.500  1.00 12.00           N
"
    }

    #[test]
    fn ligand_prep_torsions_benzamidine() {
        let lig = prepare_ligand("NC(=N)c1ccccc1").unwrap();
        assert_eq!(lig.formula, "C7H8N2");
        // кольцо–амидин: 1 вращаемая связь; кольцо внутренне жёсткое
        assert_eq!(lig.torsions.len(), 1, "вращаемых связей: {}", lig.torsions.len());
        assert_eq!(lig.n_rot, 1);
        // доноры: оба N (NH и NH2); акцепторов нет (N с H)
        assert_eq!(lig.donors.len(), 2);
        assert!(lig.acceptors.is_empty(), "N с H не акцепторы");
        // эффективный заряд суммы = 0 (нейтральная форма)
        let q: f64 = lig.q_eff.iter().sum();
        assert!(q.abs() < 1e-9, "Σq_eff = {q}");
    }

    #[test]
    fn ligand_prep_amide_excluded() {
        // ацетамид: CH3-C(=O)-NH2 — связь C-N амидная, НЕ вращаемая
        let lig = prepare_ligand("CC(=O)N").unwrap();
        assert_eq!(lig.torsions.len(), 0, "амидная связь исключена");
        // пропан: вращения метилов вырождены — 0; бутан — 1
        let lig2 = prepare_ligand("CCC").unwrap();
        assert_eq!(lig2.torsions.len(), 0, "пропан: метилы терминальные");
        let lig3 = prepare_ligand("CCCC").unwrap();
        assert_eq!(lig3.torsions.len(), 1, "бутан: C2-C3 вращаемая");
    }

    #[test]
    fn charged_ligand_q_eff() {
        let lig = prepare_ligand("C[NH3+]").unwrap();
        let q: f64 = lig.q_eff.iter().sum();
        assert!((q - 1.0).abs() < 1e-9, "метиламмоний +1, Σq_eff = {q}");
    }

    #[test]
    fn score_water_near_carbonyl_hbond() {
        // вода-донор к карбонильному O мини-рецептора
        let mm = super::super::pdb::parse_pdb(mini_pdb()).unwrap();
        let lig = prepare_ligand("O").unwrap();
        let receptor = mm.receptor_atoms();
        let charges = receptor_charges(&mm);
        let donors = reconstruct_donors(&mm);
        let index = ProteinIndex::build(&mm, &receptor);
        let field = build_field(&mm, &index, &charges, &donors, [0.0, 0.0, 0.0], 12.0);
        // позиция: H воды на O карбонила ALA3 (0, 1, -0.7)
        let mut pos = lig.base.clone();
        // ставим O воды в 2.8 Å от карбонильного O
        for p in pos.iter_mut() {
            p[0] += 0.0;
            p[1] += -0.7 + 2.8;
            p[2] += 0.0;
        }
        let t = score_pose(&lig, &field, &pos);
        assert!(t.e_hb < -1.0, "H-связь вода→карбонил: E_HB = {}", t.e_hb);
    }

    #[test]
    fn score_clash_penalized() {
        let mm = super::super::pdb::parse_pdb(mini_pdb()).unwrap();
        let lig = prepare_ligand("CCC").unwrap();
        let receptor = mm.receptor_atoms();
        let charges = receptor_charges(&mm);
        let donors = reconstruct_donors(&mm);
        let index = ProteinIndex::build(&mm, &receptor);
        let field = build_field(&mm, &index, &charges, &donors, [0.0, 0.0, 0.0], 12.0);
        // вплотную к CA ALA2 (2.5, 0, 0) — стерический конфликт
        let mut pos = lig.base.clone();
        for p in pos.iter_mut() {
            p[0] += 2.5;
        }
        let t_clash = score_pose(&lig, &field, &pos);
        // далеко — конфликтов нет
        let mut pos2 = lig.base.clone();
        for p in pos2.iter_mut() {
            p[0] += 30.0;
        }
        let t_far = score_pose(&lig, &field, &pos2);
        assert!(t_clash.e_vdw > t_far.e_vdw, "клэш должен отталкивать");
        assert!(t_clash.clashes >= 1);
        assert_eq!(t_far.clashes, 0);
    }

    /// Регрессия контура B5 (2026-10-06): кулон без контактного пола
    /// ВОЗНАГРАЖДАЛ столкновения — O⁻-лиганд, вжатый в положительные
    /// united-заряды, набирал до −45 кДж/пара на под-контактных
    /// дистанциях, MC выбирал позы с 19 клатшами (вератрол E_elec=−808
    /// при 19 конфликтах). Пол r_eff = max(r, 0.75·rij) сатурирует кулон,
    /// LJ и так capped 8 → клэш перестаёт окупаться.
    #[test]
    fn clash_coulomb_saturation() {
        let mm = super::super::pdb::parse_pdb(mini_pdb()).unwrap();
        // фенол: O несёт Gasteiger ≈ −0.55 — электростатически жадный
        let lig = prepare_ligand("Oc1ccccc1").unwrap();
        let receptor = mm.receptor_atoms();
        let charges = receptor_charges(&mm);
        let donors = reconstruct_donors(&mm);
        let index = ProteinIndex::build(&mm, &receptor);
        let field = build_field(&mm, &index, &charges, &donors, [0.0, 0.0, 0.0], 12.0);
        // атом поля с максимальным положительным зарядом — приманка
        let (kmax, qmax) = field
            .q
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();
        assert!(*qmax > 0.0, "заряд-приманка обязан быть положительным");
        // O — точно на приманке (r = 0!), остальной лиганд — далеко
        let mut pos = lig.base.clone();
        for (ni, p) in pos.iter_mut().enumerate() {
            if ni == lig.conf.heavy_map[0] {
                *p = field.pos[kmax];
            } else {
                p[0] += 40.0;
            }
        }
        let t = score_pose(&lig, &field, &pos);
        // БЕЗ пола единственная пара O(r=0)·(+q) давала
        // K·(−0.55·q)/(4·r²) → −∞; с полом вклад пары ограничен
        // величиной на контакте 0.75·rij ≈ −9 кДж. Порог −60 отделяет
        // сатурированный кулон (≈ −30..−50 с окружением) от взрыва.
        assert!(
            t.e_elec > -60.0,
            "кулон на клэш-дистанциях обязан сатурироваться: e_elec = {}",
            t.e_elec
        );
        assert!(t.clashes >= 1, "поза вжата — клэши обязаны считаться");
    }

    #[test]
    fn rmsd_greedy_matching_symmetric() {
        // бензол 6-кратно симметричен: поворот кристалла на 60° → RMSD ≈ 0
        // (жадное спаривание по элементам обязано найти перестановку)
        let lig = prepare_ligand("c1ccccc1").unwrap();
        let (s, c) = ((std::f64::consts::TAU / 6.0).sin(), (std::f64::consts::TAU / 6.0).cos());
        let mut crystal = Vec::new();
        for (ai, &ni) in lig.conf.heavy_map.iter().enumerate() {
            let p = lig.base[ni];
            crystal.push(super::super::pdb::PdbAtom {
                serial: ai as u32 + 1,
                name: *b"C1  ",
                res_name: *b"BEN",
                chain: b'A',
                res_seq: 10,
                icode: 0,
                pos: [p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]],
                b_factor: 12.0,
                element: *b"C ",
                formal_charge: 0,
                het: true,
            });
        }
        let r = rmsd_to_crystal(&lig, &lig.base, &crystal).unwrap();
        assert!(r < 0.15, "60°-симметрия бензола: RMSD = {r}");
    }

    #[test]
    fn dock_mini_redocking_finds_pocket() {
        // redocking BEN в мини-карман: лиганд должен вернуться к кристаллу
        let mm = super::super::pdb::parse_pdb(mini_pdb()).unwrap();
        let params = DockParams {
            runs: 4,
            steps: 300,
            seed: 42,
            t0: 8.0,
            t1: 0.3,
            jit: false, // интерпретатор: тест проверяет канон v0.75
        };
        let res = dock("NC(=O)c1ccccc1", &mm, &params, &PocketSpec::Auto).unwrap();
        assert_eq!(res.pocket_method, "лиганд");
        assert!(res.n_evals > 100, "вычислений скоринга: {}", res.n_evals);
        assert!(res.best.contacts > 0, "контакты есть");
        // детерминизм
        let res2 = dock("NC(=O)c1ccccc1", &mm, &params, &PocketSpec::Auto).unwrap();
        assert!(
            (res.best.delta_g - res2.best.delta_g).abs() < 1e-9,
            "детерминированность: {} vs {}",
            res.best.delta_g,
            res2.best.delta_g
        );
    }

    #[test]
    fn logsumexp_ensemble() {
        let vals = vec![-10.0f64, -12.0, -15.0];
        let lse = DockResult::logsumexp(&vals);
        // max = -10: lse = -10 + ln(1 + e^-2 + e^-5)
        let expect = -10.0 + (1.0 + (-2.0f64).exp() + (-5.0f64).exp()).ln();
        assert!((lse - expect).abs() < 1e-12);
        // вырожденный случай
        assert_eq!(DockResult::logsumexp(&[]), f64::NEG_INFINITY);
    }

    #[test]
    fn apply_state_preserves_bond_lengths() {
        // жёсткое тело + кручения НЕ меняют длин связей (инвариант Родригеса)
        let lig = prepare_ligand("CCCCO").unwrap();
        let before: Vec<f64> = lig
            .conf
            .bonds
            .iter()
            .map(|b| dist(lig.base[b.a], lig.base[b.b]))
            .collect();
        let st = PoseState {
            center: [3.0, 4.0, 5.0],
            rot: rot_matrix([1.0, 1.0, 0.5], 0.7),
            torsions: vec![1.1, -0.8, 0.4],
        };
        let pos = apply_state(&lig, &st);
        for (bi, b) in lig.conf.bonds.iter().enumerate() {
            let d = dist(pos[b.a], pos[b.b]);
            assert!(
                (d - before[bi]).abs() < 1e-9,
                "связь {bi}: {d} vs {}",
                before[bi]
            );
        }
    }

    #[test]
    fn torsion_sector_is_27th_of_turn() {
        assert!((TORS_SECTOR * 27.0 - std::f64::consts::TAU).abs() < 1e-12);
    }
}
