//! Induced Fit / Soft Pocket v0.78.0 (контур C).
//!
//! Гибкость боковых цепей кармана при связывании — прямое расширение
//! MC/SA-машины докинга: кручения лиганда квантованы тритами (2π/27),
//! тем же механизмом квантуются χ-углы боковых цепей ([`super::rotamer`]).
//!
//! Цикл (C2):
//! 1. **Раунд 1** — докинг в жёсткий карман (обычный `dock_prepared`).
//! 2. **Раунд 2** — заморозить позу лиганда → MC-релаксация χ боковых
//!    цепей кармана в поле лиганда (Метрополис + kT-отжиг, предложения
//!    ±1..3 сектора, софт-кор C3: стена LJ cap 8 — столкновения
//!    боковая⇄лиганд штрафуются мягко, не отсечением).
//! 3. **Раунд 3** — редокинг лиганда в обновлённый `PocketField`
//!    (пересборка из смещённых координат).
//! 4. Сходимость: ≤3 раунда или |ΔScore| < 1 кДж.
//!
//! Скоринг релаксации — сфокусированный (честно): пары
//! мутант×лиганд (LJ soft + кулон с контактным полом B5) +
//! мутант×статическое поле (LJ soft — боковая цепь не складывается в
//! белок) + награда за H-связи акцепторы-мутанта×доноры-лиганда.
//! Полный скоринг — на редокинге (раунд 3).
//!
//! Детерминизм: сид χ-MC = FNV(seed_key|χ|раунд); повтор ≡ первому.

use super::dock::{
    dock_prepared, prepare_dock_context, prepare_ligand, fnv1a, DockContext, DockParams,
    DockResult, LigandPrep, PocketSpec, Rng,
};
use super::geom3d::vdw_radius;
use super::pdb::{receptor_charges, MacroMol};
use super::rotamer::{
    chi_atoms, is_flexible, observed_chi, rotate_about, side_depth, CHI_SECTOR_DEG,
};

// Константы — синхронно с dock.rs (дублирование как в chem_kernel.rs:
// приватность констант канона скоринга).
const K_ELEC: f64 = 1389.35456;
const EPS_R: f64 = 4.0;
const LJ_CAP: f64 = 8.0;
const HB_ENERGY: f64 = 8.0;
/// Квадрат контактного пола кулона (B5): r_eff ≥ 0.75·rij.
const FLOOR2: f64 = 0.75 * 0.75;
/// Дальняя отсечка пар релаксации, Å.
const PAIR_CUTOFF: f64 = 10.0;
/// Рестрейнт возврата к кристаллическому χ: кДж/моль на сектор².
/// Физический смысл: внутренний торсионный потенциал боковой цепи
/// (который сфокусированный скор не считает) держит конформацию
/// близко к ротамеру; без него LJ-притяжение «плавит» цепи на лиганд
/// (поймано C4: −11 секторов, RMSD 0.43 → 3.48 Å).
const K_CHI_RESTRAINT: f64 = 0.6;

// ─── Параметры и результат ──────────────────────────────────────────────

/// Параметры induced fit.
#[derive(Debug, Clone)]
pub struct InducedFitParams {
    /// Шагов MC χ-релаксации на раунд.
    pub chi_steps: usize,
    /// Начальная kT χ-MC, кДж/моль.
    pub t0: f64,
    /// Конечная kT.
    pub t1: f64,
    /// Максимум раундов цикла (канон: ≤3).
    pub max_rounds: usize,
    /// Порог сходимости |ΔScore|, кДж/моль.
    pub dg_tol: f64,
    /// Параметры докинга раундов 1/3.
    pub dock: DockParams,
}

impl Default for InducedFitParams {
    fn default() -> Self {
        InducedFitParams {
            chi_steps: 320,
            t0: 6.0,
            t1: 0.3,
            max_rounds: 3,
            dg_tol: 1.0,
            dock: DockParams::default(),
        }
    }
}

/// Сдвиг одного χ остатка (дифф-отчёт C2).
#[derive(Debug, Clone)]
pub struct ChiShift {
    /// Остаток (имя+цепь+номер).
    pub residue: String,
    /// χ1 или χ2.
    pub chi: u8,
    /// Сдвиг в трит-секторах (±).
    pub delta_sectors: i32,
    /// χ до, градусы.
    pub before_deg: f64,
    /// χ после.
    pub after_deg: f64,
    /// Вклад остатка в скор релаксации (после − до), кДж.
    pub energy_gain: f64,
}

/// Итог induced fit.
pub struct InducedFitResult {
    /// Раундов выполнено.
    pub rounds: usize,
    /// Сошёлся ли по |ΔScore| < порога (или χ перестали двигаться).
    pub converged: bool,
    /// Раунд 1 (жёсткий карман).
    pub base: DockResult,
    /// Финал (после последнего редокинга).
    pub final_res: DockResult,
    /// Дифф по остаткам (все раунды суммарно).
    pub shifts: Vec<ChiShift>,
    /// Скор χ-релаксации последнего раунда (после − до, кДж).
    pub chi_relax_gain: f64,
    /// Время, мс.
    pub elapsed_ms: f64,
}

// ─── Скоринг релаксации (сфокусированный) ───────────────────────────────

/// Атом контекста релаксации: (mm-индекс, позиция, vdw, заряд,
/// акцептор?, остаток).
type RAtom = (usize, [f64; 3], f64, f64, bool, usize);

/// LJ 12-6 с cap (софт-кор C3: столкновение штрафуется мягко, не отсечением).
fn lj_soft(p: [f64; 3], ri: f64, q: [f64; 3], rj: f64) -> f64 {
    let d = dist(p, q);
    if d > PAIR_CUTOFF {
        return 0.0;
    }
    let rij = ri + rj;
    let d = d.max(0.35);
    let eps = 0.42 * (ri * rj).sqrt() / 1.6;
    let x = rij / d;
    let lj = 4.0 * eps * (x.powi(12) - x.powi(6));
    lj.min(LJ_CAP)
}

/// Пара (мутант × лиганд): LJ soft + кулон с контактным полом B5.
fn pair_energy(p: [f64; 3], ri: f64, qi: f64, q: [f64; 3], rj: f64, qj: f64) -> f64 {
    let d = dist(p, q);
    if d > PAIR_CUTOFF {
        return 0.0;
    }
    let rij = ri + rj;
    let d = d.max(0.35);
    let eps = 0.42 * (ri * rj).sqrt() / 1.6;
    let x = rij / d;
    let lj = 4.0 * eps * (x.powi(12) - x.powi(6));
    let mut e = lj.min(LJ_CAP);
    // кулон: ε(r)=4r, контактный пол 0.75·rij (синхронно с B5)
    let r2 = d * d;
    let fl = FLOOR2 * rij * rij;
    let r_eff2 = if r2 > fl { r2 } else { fl };
    e += K_ELEC * qi * qj / (EPS_R * r_eff2);
    e
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Энергия остатка (все его атомы из оверлея) против замороженного
/// лиганда + статического поля.
fn residue_energy(
    atoms: &[RAtom],
    lig_heavy: &[([f64; 3], f64, f64)],
    lig_donor_h: &[[f64; 3]],
    static_field: &[RAtom],
) -> f64 {
    let mut e = 0.0f64;
    for &(_, p, ri, qi, acc, ri_res) in atoms.iter() {
        // пары с замороженным лигандом (тяжёлые узлы)
        for &(lp, rl, ql) in lig_heavy.iter() {
            e += pair_energy(p, ri, qi, lp, rl, ql);
        }
        // H-связи: акцептор остатка × донорные H лиганда
        // (дистанционный фактор 0.5·HB — угловая проверка на редокинге)
        if acc {
            for &hp in lig_donor_h.iter() {
                let d = dist(p, hp);
                if d < 3.5 {
                    e -= HB_ENERGY * ((3.5 - d) / 1.0).clamp(0.0, 1.0) * 0.5;
                }
            }
        }
        // статическое поле (свой остаток исключён — внутренняя геометрия
        // боковой цепи сохраняется связями, не считается)
        for &(_, fp, rf, _qf, _a, fres) in static_field.iter() {
            if fres == ri_res {
                continue;
            }
            e += lj_soft(p, ri, fp, rf);
        }
    }
    e
}

// ─── χ-задачи ───────────────────────────────────────────────────────────

/// Одна χ-степень свободы остатка кармана.
struct ChiJob {
    ri: usize,
    chi: u8,
    /// Индексы атомов оси (N, CA, CB, X1) / (CA, CB, X1, X2).
    axis: [usize; 4],
    /// Индексы движущихся атомов (глубина ≥ chi+1).
    moving: Vec<usize>,
    /// Текущий χ, градусы (на старте = наблюдаемый).
    cur_deg: f64,
}

/// Собрать χ-задачи для гибких остатков кармана в 6 Å от позы лиганда.
fn chi_jobs(mm: &MacroMol, ctx: &DockContext, lig_pos: &[[f64; 3]]) -> Vec<ChiJob> {
    let mut jobs = Vec::new();
    for &ri in ctx.pocket.residues.iter() {
        let res_name = mm.residues[ri].name_str();
        if !is_flexible(res_name) {
            continue;
        }
        // остаток близко к лиганду?
        let res_atoms = mm.residue_atom_ids(ri);
        let mut near = false;
        'outer: for &ai in res_atoms.iter() {
            let p = mm.atoms[ai].pos;
            for &lp in lig_pos.iter() {
                if dist(p, lp) < 6.0 {
                    near = true;
                    break 'outer;
                }
            }
        }
        if !near {
            continue;
        }
        for chi in [1u8, 2u8] {
            let Some(names) = chi_atoms(res_name, chi) else {
                continue;
            };
            let mut axis = [0usize; 4];
            let mut ok = true;
            for (k, nm) in names.iter().enumerate() {
                match mm.residue_atom(ri, nm) {
                    Some(ai) => axis[k] = ai,
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
            // движущиеся атомы: глубина ≥ chi+1 (хромофоры боковой цепи)
            let moving: Vec<usize> = res_atoms
                .iter()
                .copied()
                .filter(|&ai| side_depth(mm.atoms[ai].name_str()) >= chi + 1)
                .collect();
            if moving.is_empty() {
                continue;
            }
            let cur_deg = observed_chi(mm, ri, chi).unwrap_or(0.0);
            jobs.push(ChiJob {
                ri,
                chi,
                axis,
                moving,
                cur_deg,
            });
        }
    }
    jobs
}

/// Акцептор по (остаток, атом) — как dock.rs `is_receptor_acceptor`.
fn residue_acceptor(res_name: &str, atom_name: &str) -> bool {
    match atom_name {
        "O" | "OXT" => true,
        "OD1" | "OD2" => matches!(res_name, "ASP" | "ASN"),
        "OE1" | "OE2" => matches!(res_name, "GLU" | "GLN"),
        "OG" => res_name == "SER",
        "OG1" => res_name == "THR",
        "OH" => res_name == "TYR",
        "NE2" => res_name == "HIS",
        _ => false,
    }
}

// ─── Раунд 2: MC-релаксация ─────────────────────────────────────────────

/// MC-релаксация χ боковых цепей в поле замороженного лиганда.
/// Возвращает (мутант-mm, сдвиги, Δскора релаксации).
fn relax_side_chains(
    mm: &MacroMol,
    ctx: &DockContext,
    lig: &LigandPrep,
    lig_pos: &[[f64; 3]],
    params: &InducedFitParams,
    seed: u64,
) -> (MacroMol, Vec<ChiShift>, f64) {
    let charges = receptor_charges(mm);
    let jobs = chi_jobs(mm, ctx, lig_pos);
    if jobs.is_empty() {
        return (mm.clone(), Vec::new(), 0.0);
    }

    // Лиганд: тяжёлые узлы + донорные H (заморожены)
    let lig_heavy: Vec<([f64; 3], f64, f64)> = lig
        .conf
        .heavy_map
        .iter()
        .enumerate()
        .map(|(ai, &ni)| (lig_pos[ni], lig.vdw[ai], lig.q_eff[ai]))
        .collect();
    let lig_donor_h: Vec<[f64; 3]> = lig
        .donors
        .iter()
        .flat_map(|&(_, ref hs)| hs.iter().map(|&h| lig_pos[h]))
        .collect();

    // Остатки, которые мутируем
    let job_res: Vec<usize> = {
        let mut v: Vec<usize> = jobs.iter().map(|j| j.ri).collect();
        v.sort_unstable();
        v.dedup();
        v
    };

    // Статическое поле: атомы поля БЕЗ мутируемых остатков
    let static_field: Vec<RAtom> = (0..ctx.field.len)
        .filter(|&k| {
            let ri = mm
                .residue_of_atom(ctx.field.src[k])
                .unwrap_or(usize::MAX);
            !job_res.contains(&ri)
        })
        .map(|k| {
            let ri = mm
                .residue_of_atom(ctx.field.src[k])
                .unwrap_or(usize::MAX);
            (
                ctx.field.src[k],
                ctx.field.pos[k],
                ctx.field.vdw[k],
                ctx.field.q[k],
                ctx.field.acceptor[k],
                ri,
            )
        })
        .collect();

    // Атомы мутируемых остатков: из поля + из mm (за сферой поля —
    // стерика) — позиции из ТЕКУЩЕГО mm (раунды 2+ уже с мутациями)
    let mut mut_atoms: Vec<RAtom> = Vec::new();
    for &ri in job_res.iter() {
        for ai in mm.residue_atom_ids(ri) {
            let a = &mm.atoms[ai];
            if a.elem_str() == "H" {
                continue;
            }
            // в поле — заряды/флаги из поля (united), вне — из charges
            let in_field = ctx.field.src.iter().position(|&s| s == ai);
            let (p, vdw, q, acc) = match in_field {
                Some(k) => (
                    ctx.field.pos[k], // раунд 1 = mm-позиция; раунды 2+ — мм уже мутирован, поле стейл → берём mm!
                    ctx.field.vdw[k],
                    ctx.field.q[k],
                    ctx.field.acceptor[k],
                ),
                None => (
                    a.pos,
                    vdw_radius(a.elem_str()),
                    charges[ai],
                    residue_acceptor(a.res_str(), a.name_str()) && !a.het,
                ),
            };
            // позиции всегда из mm (мутант текущего раунда)
            let p = a.pos;
            let _ = p;
            mut_atoms.push((ai, a.pos, vdw, q, acc, ri));
        }
    }

    // Скор остатков ДО
    let res_energy_of = |atoms: &Vec<RAtom>, ri: usize| -> f64 {
        let subset: Vec<RAtom> = atoms.iter().filter(|t| t.5 == ri).cloned().collect();
        residue_energy(&subset, &lig_heavy, &lig_donor_h, &static_field)
    };
    let e_before: f64 = job_res
        .iter()
        .map(|&ri| res_energy_of(&mut_atoms, ri))
        .sum();

    // Рабочий оверлей (позиции мутируемых атомов)
    let mut work: Vec<RAtom> = mut_atoms.clone();
    let find_work = |ai: usize, w: &Vec<RAtom>| -> usize {
        w.iter().position(|t| t.0 == ai).expect("атом в work")
    };

    // Метрополис по χ-задачам (с рестрейнтом к кристаллическому χ —
    // внутренний торсионный потенциал в сфокусированном скоре)
    let mut rng = Rng::new(seed);
    let mut before_deg: Vec<(usize, u8, f64)> = jobs
        .iter()
        .map(|j| (j.ri, j.chi, j.cur_deg))
        .collect();
    let mut cur_deg: Vec<f64> = jobs.iter().map(|j| j.cur_deg).collect();
    let n_jobs = jobs.len();
    for step in 0..params.chi_steps {
        let frac = step as f64 / params.chi_steps.max(1) as f64;
        let kt = params.t0 * (params.t1 / params.t0).powf(frac);
        let ji = (rng.next_u64() % n_jobs as u64) as usize;
        let mag = 1.0 + (rng.next_u64() % 3) as f64;
        let sgn = if rng.f64() < 0.5 { -1.0 } else { 1.0 };
        let d_deg = sgn * mag * CHI_SECTOR_DEG;
        let job = &jobs[ji];
        // ось вращения: (axis[1], axis[2]) — CA-CB (χ1) / CB-X1 (χ2);
        // позиции из оверлея (X1 мог двинуться χ1-шагами)
        let ax_b = work[find_work(job.axis[1], &work)].1;
        let ax_c = work[find_work(job.axis[2], &work)].1;
        // кандидаты
        let mut cand = work.clone();
        for &ai in job.moving.iter() {
            let wi = find_work(ai, &work);
            cand[wi].1 = rotate_about(work[wi].1, ax_b, ax_c, d_deg.to_radians());
        }
        // ΔE остатка + рестрейнт χ² к кристаллу
        let e_old = res_energy_of(&work, job.ri);
        let e_new = res_energy_of(&cand, job.ri);
        let d_sect_old = (cur_deg[ji] - before_deg[ji].2) / CHI_SECTOR_DEG;
        let d_sect_new = d_sect_old + d_deg / CHI_SECTOR_DEG;
        let restraint =
            K_CHI_RESTRAINT * (d_sect_new * d_sect_new - d_sect_old * d_sect_old);
        let de = e_new - e_old + restraint;
        if de <= 0.0 || rng.f64() < (-de / kt).exp() {
            work = cand;
            cur_deg[ji] += d_deg;
        }
    }

    let e_after: f64 = job_res
        .iter()
        .map(|&ri| res_energy_of(&work, ri))
        .sum();

    // Применить финальные позиции к мутанту
    let mut mutant = mm.clone();
    for &(ai, p, ..) in work.iter() {
        mutant.atoms[ai].pos = p;
    }

    // Дифф-отчёт
    let mut shifts = Vec::new();
    for (ji, j) in jobs.iter().enumerate() {
        let delta = ((cur_deg[ji] - before_deg[ji].2) / CHI_SECTOR_DEG).round() as i32;
        if delta == 0 {
            continue;
        }
        let e_old = res_energy_of(&mut_atoms, j.ri);
        let e_new = res_energy_of(&work, j.ri);
        let res = &mm.residues[j.ri];
        shifts.push(ChiShift {
            residue: format!(
                "{}{}{}",
                res.name_str(),
                if res.chain == 0 { '_' } else { res.chain as char },
                res.seq
            ),
            chi: j.chi,
            delta_sectors: delta,
            before_deg: before_deg[ji].2,
            after_deg: cur_deg[ji],
            energy_gain: e_new - e_old,
        });
    }

    (mutant, shifts, e_after - e_before)
}

// ─── Главный цикл ───────────────────────────────────────────────────────

/// Induced fit докинг: раунд 1 (жёсткий) → χ-релаксация → редокинг,
/// сходимость ≤ max_rounds или |ΔScore| < dg_tol.
pub fn induced_fit(
    smiles: &str,
    mm: &MacroMol,
    params: &InducedFitParams,
    spec: &PocketSpec,
) -> Result<InducedFitResult, String> {
    let t_all = std::time::Instant::now();
    let lig = prepare_ligand(smiles)?;
    if lig.conf.heavy_map.is_empty() {
        return Err("лиганд без тяжёлых атомов".into());
    }

    // Раунд 1: жёсткий карман
    let ctx1 = prepare_dock_context(mm, spec)?;
    let base = dock_prepared(mm, &ctx1, &lig, smiles, &params.dock)?;

    let mut mutant = mm.clone();
    let mut all_shifts: Vec<ChiShift> = Vec::new();
    let mut last_gain = 0.0f64;
    let mut cur_pos = base.best_positions.clone();
    let mut cur_dg = base.best.delta_g;
    let mut final_res: Option<DockResult> = None;
    let mut converged = false;
    let mut rounds = 0usize;

    for round in 0..params.max_rounds.max(1) {
        rounds = round + 1;
        // сид раунда: FNV(seed_key|χ|round) — детерминизм
        let seed = fnv1a(format!("{}|chi|{}", smiles, round).as_bytes());
        // Раунд 2: χ-релаксация в поле текущей позы (контекст текущего
        // мутанта — чтобы поле и остатки были согласованы)
        let ctx_now = prepare_dock_context(&mutant, spec)?;
        let (mm2, shifts, gain) =
            relax_side_chains(&mutant, &ctx_now, &lig, &cur_pos, params, seed);
        mutant = mm2;
        all_shifts.extend(shifts);
        last_gain = gain;
        // Раунд 3: редокинг в обновлённый карман
        let ctx_after = prepare_dock_context(&mutant, spec)?;
        let r3 = dock_prepared(&mutant, &ctx_after, &lig, smiles, &params.dock)?;
        let dg = (r3.best.delta_g - cur_dg).abs();
        cur_pos = r3.best_positions.clone();
        cur_dg = r3.best.delta_g;
        let moved = all_shifts
            .iter()
            .rev()
            .take(8)
            .filter(|s| s.delta_sectors != 0)
            .count();
        final_res = Some(r3);
        if dg < params.dg_tol || moved == 0 {
            converged = true;
            break;
        }
    }

    let final_res = final_res.expect("цикл обязан сделать ≥1 раунд");

    Ok(InducedFitResult {
        rounds,
        converged,
        base,
        final_res,
        shifts: all_shifts,
        chi_relax_gain: last_gain,
        elapsed_ms: t_all.elapsed().as_secs_f64() * 1000.0,
    })
}

/// Текстовый отчёт induced fit (моноширинный, как dock_text).
pub fn induced_fit_text(r: &InducedFitResult, ligand_input: &str) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(2048);
    let _ = writeln!(s, "═══ Induced Fit: {ligand_input} ⇄ гибкий карман ═══");
    let _ = writeln!(
        s,
        "[1] Раундов: {} (сходимость: {}), χ-релаксация: {:+.1} кДж/моль, время {:.0} мс",
        r.rounds,
        if r.converged { "да" } else { "нет" },
        r.chi_relax_gain,
        r.elapsed_ms
    );
    let _ = writeln!(
        s,
        "[2] Жёсткий карман: ΔG = {:.1} кДж/моль, H-связей {}, RMSD {}",
        r.base.best.delta_g,
        r.base.best.hbonds,
        match r.base.rmsd {
            Some(v) => format!("{v:.2} Å"),
            None => "—".into(),
        }
    );
    let _ = writeln!(
        s,
        "[3] После подгонки: ΔG = {:.1} кДж/моль, H-связей {}, RMSD {} (ΔG {:+.1})",
        r.final_res.best.delta_g,
        r.final_res.best.hbonds,
        match r.final_res.rmsd {
            Some(v) => format!("{v:.2} Å"),
            None => "—".into(),
        },
        r.final_res.best.delta_g - r.base.best.delta_g
    );
    if r.shifts.is_empty() {
        let _ = writeln!(
            s,
            "[4] Дифф по остаткам: подгонки не потребовалось (карман уже подогнан)"
        );
    } else {
        let _ = writeln!(s, "[4] Дифф по остаткам (трит-секторы 13⅓°):");
        for sh in r.shifts.iter() {
            let _ = writeln!(
                s,
                "    {:<8} χ{}: {:+} сект. ({:+.0}° → {:+.0}°), скор {:+.1} кДж",
                sh.residue,
                sh.chi,
                sh.delta_sectors,
                sh.before_deg,
                sh.after_deg,
                sh.energy_gain
            );
        }
    }
    let _ = writeln!(
        s,
        "Примечание: χ-релаксация — сфокусированный скор (пары мутант×лиганд + мутант×поле, софт-кор LJ cap 8); полный скоринг — на редокинге."
    );
    s
}

// ─── Тесты ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn load_3ptb() -> MacroMol {
        MacroMol::from_file("tests/fixtures/3ptb.pdb").expect("3ptb.pdb")
    }

    /// C4: 3PTB НЕ регрессирует после включения гибкости (порог ≤ 2.0 Å;
    /// физиологичная катионная форма — канон контура B).
    #[test]
    fn induced_fit_3ptb_no_regression() {
        let mm = load_3ptb();
        let params = InducedFitParams {
            chi_steps: 120, // ускоренный тест
            dock: DockParams {
                runs: 4,
                steps: 800,
                ..Default::default()
            },
            ..Default::default()
        };
        let r = induced_fit("NC(=[NH2+])c1ccccc1", &mm, &params, &PocketSpec::Auto).unwrap();
        let rmsd = r.final_res.rmsd.unwrap_or(f64::INFINITY);
        eprintln!(
            "induced fit 3PTB: раундов {}, RMSD жёсткий {:.2} → гибкий {:.2} Å, ΔG {:.1} → {:.1}, сдвигов {}",
            r.rounds,
            r.base.rmsd.unwrap_or(0.0),
            rmsd,
            r.base.best.delta_g,
            r.final_res.best.delta_g,
            r.shifts.len()
        );
        assert!(rmsd <= 2.0, "RMSD после гибкости {rmsd} > 2.0 Å");
        assert!(r.rounds <= 3, "раундов больше канонических трёх");
    }

    /// Детерминизм: повторный прогон ≡ первому.
    #[test]
    fn induced_fit_determinism() {
        let mm = load_3ptb();
        let params = InducedFitParams {
            chi_steps: 80,
            dock: DockParams {
                runs: 2,
                steps: 400,
                ..Default::default()
            },
            ..Default::default()
        };
        let ra = induced_fit("NC(=[NH2+])c1ccccc1", &mm, &params, &PocketSpec::Auto).unwrap();
        let rb = induced_fit("NC(=[NH2+])c1ccccc1", &mm, &params, &PocketSpec::Auto).unwrap();
        assert_eq!(ra.rounds, rb.rounds);
        assert_eq!(ra.shifts.len(), rb.shifts.len());
        for (a, b) in ra.shifts.iter().zip(rb.shifts.iter()) {
            assert_eq!(a.residue, b.residue);
            assert_eq!(a.delta_sectors, b.delta_sectors);
        }
        assert!(
            (ra.final_res.best.delta_g - rb.final_res.best.delta_g).abs() < 1e-9,
            "ΔG повтор ≠ первому: {} vs {}",
            ra.final_res.best.delta_g,
            rb.final_res.best.delta_g
        );
    }

    /// C3: софт-кор — столкновение штрафуется мягко (cap), не отсечением.
    #[test]
    fn soft_core_clash_bounded() {
        let e = lj_soft([0.0; 3], 1.8, [0.0; 3], 1.8);
        assert!((e - LJ_CAP).abs() < 1e-9, "LJ на r→0 = {e}, а не cap {LJ_CAP}");
        // кулоновская пара тоже сатурируется полом B5
        let e2 = pair_energy([0.0; 3], 1.8, -0.8, [0.0; 3], 1.5, 0.4);
        assert!(e2 > -200.0 && e2 < 0.0, "пара на r→0: {e2}");
    }

    /// Дифф-отчёт: числа на месте, формат читается.
    #[test]
    fn induced_fit_report_shape() {
        let mm = load_3ptb();
        let params = InducedFitParams {
            chi_steps: 60,
            dock: DockParams {
                runs: 2,
                steps: 300,
                ..Default::default()
            },
            ..Default::default()
        };
        let r = induced_fit("NC(=[NH2+])c1ccccc1", &mm, &params, &PocketSpec::Auto).unwrap();
        let t = induced_fit_text(&r, "benzamidinium");
        assert!(t.contains("Induced Fit"));
        assert!(t.contains("Раундов"));
        assert!(t.contains("Дифф по остаткам"));
    }

    /// χ-задачи строятся только для гибких остатков у лиганда.
    #[test]
    fn chi_jobs_selection() {
        let mm = load_3ptb();
        let ctx = prepare_dock_context(&mm, &PocketSpec::Auto).unwrap();
        let lig = prepare_ligand("NC(=[NH2+])c1ccccc1").unwrap();
        let mut pos = lig.base.clone();
        for p in pos.iter_mut() {
            p[0] += ctx.pocket.center[0];
            p[1] += ctx.pocket.center[1];
            p[2] += ctx.pocket.center[2];
        }
        let jobs = chi_jobs(&mm, &ctx, &pos);
        assert!(!jobs.is_empty(), "в кармане 3PTB обязаны быть гибкие остатки");
        for j in jobs.iter() {
            let name = mm.residues[j.ri].name_str();
            assert!(is_flexible(name), "{name} не гибкий, но попал в задачи");
            assert!(!j.moving.is_empty());
        }
        // Asp189 — ключевой остаток S1 — обязан быть среди задач
        let has_asp = jobs.iter().any(|j| mm.residues[j.ri].name_str() == "ASP");
        assert!(has_asp, "Asp189 (S1) обязан релаксироваться");
    }
}
