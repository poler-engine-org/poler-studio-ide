//! Каскадный ультра-скрининг библиотек лигандов v0.77.0 (контур B).
//!
//! Прогон сотен молекул SDF/SMILES-библиотеки в активный карман за
//! секунды, с ранжированием по вычисленному скорингу. Источник скорости:
//! [`PocketField`] строится ОДИН раз ([`DockContext`] в `Arc`) и
//! переиспользуется всеми лигандами; скоринг быстрой стадии — через
//! JIT-ядро A3 (первое боевое применение внутреннего компилятора).
//!
//! Каскад:
//! 1. **Пре-фильтры без докинга** (микросекунды): Липински, стехиометрия,
//!    заряд, размер.
//! 2. **Быстрый прогон**: `dock_prepared` с runs=2, steps=600 (JIT).
//!    Топ-N (по умолчанию 10%) проходит дальше.
//! 3. **Полный докинг** с параметрами по умолчанию (runs=12, steps=2200).
//!
//! Детерминизм: сид лиганда = FNV(seed_key|карман) — результат не зависит
//! от порядка/числа воркеров (тест: 1 воркер ≡ 8 воркеров, бит-в-бит).

use super::dock::{
    dock_prepared, prepare_dock_context, prepare_ligand, prepare_ligand_conf, DockContext,
    DockParams, LigandPrep, PocketSpec,
};
use super::pdb::MacroMol;
use super::pharma::molar_mass_of;
use super::sdf::{LibraryEntry, SdfMol};
use super::smiles::{descriptors, logp_estimate, MoleculeGraph};
use rayon::prelude::*;
use std::sync::Arc;

// ─── Настройки ──────────────────────────────────────────────────────────

/// Настройки каскада скрининга.
#[derive(Debug, Clone)]
pub struct ScreenOptions {
    /// Число воркеров (0 — глобальный пул rayon).
    pub workers: usize,
    /// Число лигандов в полный докинг (стадия 3); 0 → 10% библиотеки,
    /// минимум 1.
    pub top: usize,
    /// Прогонов MC быстрой стадии.
    pub fast_runs: usize,
    /// Шагов MC быстрой стадии.
    pub fast_steps: usize,
    /// JIT-ядро скоринга быстрой стадии (контур A3).
    pub jit: bool,
    /// Прогонов полного докинга (стадия 3).
    pub full_runs: usize,
    /// Шагов полного докинга (стадия 3).
    pub full_steps: usize,
    /// Верхние границы пре-фильтров (Липински).
    pub max_mw: f64,
    pub max_logp: f64,
    pub max_hbd: usize,
    pub max_hba: usize,
    /// Максимум тяжёлых атомов (грубая стена скоринга).
    pub max_heavy: usize,
    /// Предел модуля формального заряда.
    pub max_charge: i32,
}

impl Default for ScreenOptions {
    fn default() -> Self {
        ScreenOptions {
            workers: 0,
            top: 0,
            fast_runs: 2,
            fast_steps: 600,
            jit: true,
            full_runs: 12,
            full_steps: 2200,
            max_mw: 500.0,
            max_logp: 5.0,
            max_hbd: 5,
            max_hba: 10,
            max_heavy: 60,
            max_charge: 3,
        }
    }
}

// ─── Строка результата ──────────────────────────────────────────────────

/// Одна строка таблицы скрининга.
#[derive(Debug, Clone)]
pub struct ScreenRow {
    /// Имя записи (титул SDF / имя .smi).
    pub name: String,
    /// SMILES (для SDF-записей — формула Хилла как идентификатор).
    pub smiles: String,
    /// Формула Хилла.
    pub formula: String,
    /// Стадия, на которой завершён лиганд: 1 — отсев пре-фильтром,
    /// 2 — быстрый прогон, 3 — полный докинг.
    pub stage: u8,
    /// Прошел ли каскад до конца (стадия 2+3 завершены докингом).
    pub docked: bool,
    /// Причина отсева на стадии 1 (пусто для прошедших).
    pub reject: Option<&'static str>,
    /// ΔG лучшей позы, кДж/моль (меньше — лучше; 0 для отсева).
    pub score: f64,
    /// ΔG ансамбля (лог-домен), кДж/моль.
    pub dg_ensemble: f64,
    /// lg Kd.
    pub log10_kd: f64,
    /// H-связей лучшей позы.
    pub hbonds: usize,
    /// Число кластеров поз.
    pub clusters: usize,
    /// Кластер-RMSD: отклонение второго кластера от лучшего, Å.
    pub cluster_rmsd: Option<f64>,
    /// Липински проходит.
    pub lipinski: bool,
    /// Молярная масса, г/моль.
    pub mw: f64,
    /// Время на лиганд (мс, стадии 2+3).
    pub ms: f64,
}

/// Итог скрининга.
#[derive(Debug, Clone)]
pub struct ScreenResult {
    /// Ранжированные строки: сначала докинг-завершённые по score
    /// (возрастание), затем отсев стадии 1 (в порядке библиотеки).
    pub rows: Vec<ScreenRow>,
    /// Записей на входе.
    pub n_input: usize,
    /// Прошло пре-фильтры.
    pub n_prefiltered: usize,
    /// Быстрый прогон (стадия 2).
    pub n_fast: usize,
    /// Полный докинг (стадия 3).
    pub n_full: usize,
    /// Полное время, мс.
    pub elapsed_ms: f64,
    /// Пропускная способность стадий 1+2, лигандов/с.
    pub lig_per_sec: f64,
    /// Воркеров реально.
    pub workers: usize,
    /// Описание кармана.
    pub pocket_desc: String,
    /// Сид-заметка о детерминизме.
    pub seed_note: &'static str,
    /// Ошибки разбора/докинга (честно, без проглатывания).
    pub errors: Vec<String>,
}

// ─── Внутреннее представление ───────────────────────────────────────────

/// Лиганд, разобранный для каскада (граф есть, 3D — на стадии 2).
struct ScreenLigand {
    name: String,
    /// SMILES (если известен — сид-ключ) или пусто.
    smiles: String,
    /// Сид-ключ: SMILES либо «имя|формула» (детерминизм независимо).
    seed_key: String,
    graph: MoleculeGraph,
    /// Тяжёлые координаты SDF (пусто для .smi — уложит embed).
    heavy_pos: Option<Vec<[f64; 3]>>,
    explicit_h: Vec<Vec<[f64; 3]>>,
    /// Данные пре-фильтров (стадия 1).
    mw: f64,
    logp: f64,
    hbd: usize,
    hba: usize,
    heavy: usize,
    charge: i32,
    lipinski: bool,
}

impl ScreenLigand {
    /// Собрать из записи библиотеки (граф + дескрипторы, без 3D).
    fn from_entry(e: &LibraryEntry) -> Result<Self, String> {
        match e {
            LibraryEntry::Sdf(m) => Self::from_sdf(m),
            LibraryEntry::Smiles { name, smiles } => {
                let g = super::smiles::parse_smiles(smiles)
                    .map_err(|e| format!("«{name}»: {e}"))?;
                Self::from_graph(name.clone(), smiles.clone(), g, None, Vec::new())
            }
        }
    }

    fn from_sdf(m: &SdfMol) -> Result<Self, String> {
        let name = if m.name.trim().is_empty() {
            m.graph.hill_formula()
        } else {
            m.name.clone()
        };
        Self::from_graph(
            name,
            String::new(),
            m.graph.clone(),
            Some(m.heavy_pos.clone()),
            m.explicit_h.clone(),
        )
    }

    fn from_graph(
        name: String,
        smiles: String,
        g: MoleculeGraph,
        heavy_pos: Option<Vec<[f64; 3]>>,
        explicit_h: Vec<Vec<[f64; 3]>>,
    ) -> Result<Self, String> {
        let d = descriptors(&g);
        let mw = molar_mass_of(&g);
        let logp = logp_estimate(&g);
        let lipinski =
            d.h_bond_donors <= 5 && d.h_bond_acceptors <= 10 && mw <= 500.0 && logp <= 5.0;
        let seed_key = if smiles.is_empty() {
            format!("sdf:{}|{}", name, g.hill_formula())
        } else {
            smiles.clone()
        };
        Ok(ScreenLigand {
            name,
            smiles,
            seed_key,
            graph: g,
            heavy_pos,
            explicit_h,
            mw,
            logp,
            hbd: d.h_bond_donors,
            hba: d.h_bond_acceptors,
            heavy: d.heavy_atoms,
            charge: 0, // заполним после (total_charge)
            lipinski,
        })
    }

    /// LigandPrep: SDF-конформер приоритетнее укладчика.
    fn prepare(&self) -> Result<LigandPrep, String> {
        match &self.heavy_pos {
            Some(pos) => {
                let conf = super::geom3d::conformer_from_positions(
                    &self.graph,
                    pos,
                    &self.explicit_h,
                )?;
                prepare_ligand_conf(self.graph.clone(), conf)
            }
            None => prepare_ligand(&self.seed_key),
        }
    }
}

// ─── Каскад ─────────────────────────────────────────────────────────────

/// Прогнать библиотеку через каскад скрининга.
///
/// `entries` — разобранные записи (SDF/.smi); белок `mm` + `spec` задают
/// карман; контекст (поле, 27-дерево, заряды) строится ОДИН раз.
pub fn screen_library(
    entries: &[LibraryEntry],
    mm: &MacroMol,
    spec: &PocketSpec,
    opts: &ScreenOptions,
) -> Result<ScreenResult, String> {
    let t_all = std::time::Instant::now();

    // ── Контекст: один раз на белок+карман ──
    let ctx: Arc<DockContext> = Arc::new(prepare_dock_context(mm, spec)?);
    let pocket_desc = format!(
        "{} r={:.1} Å @ ({:.1},{:.1},{:.1})",
        ctx.pocket.method,
        ctx.pocket.radius,
        ctx.pocket.center[0],
        ctx.pocket.center[1],
        ctx.pocket.center[2]
    );

    // ── Разбор + стадия 1: пре-фильтры (последовательно, микросекунды) ──
    let mut ligands: Vec<ScreenLigand> = Vec::with_capacity(entries.len());
    let mut rejects: Vec<ScreenRow> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for e in entries {
        match ScreenLigand::from_entry(e) {
            Ok(mut l) => {
                l.charge = l.graph.total_charge();
                let reject = prefilter_reject(&l, opts);
                match reject {
                    Some(reason) => {
                        rejects.push(reject_row(&l, reason));
                    }
                    None => ligands.push(l),
                }
            }
            Err(e) => errors.push(e),
        }
    }
    let n_input = entries.len();
    let n_prefiltered = ligands.len();
    if ligands.is_empty() {
        return Ok(ScreenResult {
            rows: rejects,
            n_input,
            n_prefiltered: 0,
            n_fast: 0,
            n_full: 0,
            elapsed_ms: t_all.elapsed().as_secs_f64() * 1000.0,
            lig_per_sec: 0.0,
            workers: actual_workers(opts.workers),
            pocket_desc,
            seed_note: SEED_NOTE,
            errors,
        });
    }

    // ── Стадия 2: быстрый прогон (rayon, JIT-ядро) ──
    let fast_params = DockParams {
        runs: opts.fast_runs.max(1),
        steps: opts.fast_steps.max(50),
        seed: 0,
        t0: 12.0,
        t1: 0.4,
        jit: opts.jit,
    };
    let t_fast = std::time::Instant::now();
    let fast_ligs: Vec<&ScreenLigand> = ligands.iter().collect();
    let fast_results: Vec<(usize, Result<super::dock::DockResult, String>)> = run_parallel(
        opts.workers,
        &fast_ligs,
        |l| l.prepare(),
        |l, lig| {
            dock_prepared(mm, &ctx, lig, &l.seed_key, &fast_params)
                .map_err(|e| format!("«{}»: {e}", l.name))
        },
    );

    // Результаты стадии 2 в порядке библиотеки
    let mut fast_outs: Vec<Option<super::dock::DockResult>> =
        (0..ligands.len()).map(|_| None).collect();
    for (i, r) in fast_results {
        match r {
            Ok(res) => fast_outs[i] = Some(res),
            Err(e) => errors.push(e),
        }
    }
    let fast_ms_total = t_fast.elapsed().as_secs_f64() * 1000.0;

    // Ранжирование стадии 2 по ΔG лучшей позы (неудачи — в конец)
    let mut ranked: Vec<usize> = (0..ligands.len()).collect();
    ranked.sort_by(|&a, &b| {
        let sa = fast_outs[a].as_ref().map(|r| r.best.delta_g);
        let sb = fast_outs[b].as_ref().map(|r| r.best.delta_g);
        match (sa, sb) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    // ── Стадия 3: полный докинг топ-N ──
    let n_full_target = if opts.top > 0 {
        opts.top.min(ligands.len())
    } else {
        ((ligands.len() as f64 * 0.1).ceil() as usize).clamp(1, ligands.len())
    };
    let full_params = DockParams {
        runs: opts.full_runs.max(1),
        steps: opts.full_steps.max(100),
        seed: 0,
        t0: 12.0,
        t1: 0.4,
        jit: opts.jit,
    };
    let full_idx: Vec<usize> = ranked.iter().copied().take(n_full_target).collect();
    let full_ligs: Vec<&ScreenLigand> = full_idx.iter().map(|&i| &ligands[i]).collect();
    let full_results: Vec<(usize, Result<super::dock::DockResult, String>)> = if full_ligs.is_empty()
    {
        Vec::new()
    } else {
        run_parallel(opts.workers, &full_ligs, |l| l.prepare(), |l, lig| {
            dock_prepared(mm, &ctx, lig, &l.seed_key, &full_params)
                .map_err(|e| format!("«{}»: {e}", l.name))
        })
    };

    // Полные результаты по индексу лиганда
    let mut full_of: std::collections::BTreeMap<usize, super::dock::DockResult> =
        std::collections::BTreeMap::new();
    for (i, r) in full_results {
        match r {
            Ok(res) => {
                full_of.insert(full_idx[i], res);
            }
            Err(e) => errors.push(e),
        }
    }

    // ── Таблица ──
    let mut docked_rows: Vec<ScreenRow> = Vec::new();
    let mut failed_rows: Vec<ScreenRow> = Vec::new();
    for i in 0..ligands.len() {
        let l = &ligands[i];
        match (&fast_outs[i], full_of.get(&i)) {
            (Some(fast), Some(full)) => {
                docked_rows.push(row_of(l, full, 3, fast.elapsed_ms + full.elapsed_ms));
            }
            (Some(fast), None) => {
                // не попал в топ-N (или полный докинг упал — ошибка в errors)
                docked_rows.push(row_of(l, fast, 2, fast.elapsed_ms));
            }
            (None, _) => {
                // быстрый прогон упал — честная строка без score
                failed_rows.push(reject_row(l, "ошибка быстрого прогона"));
            }
        }
    }

    // Ранжирование: score по возрастанию (ΔG — чем меньше, тем сильнее),
    // затем имя (детерминизм при равенстве)
    docked_rows.sort_by(|a, b| {
        a.score
            .partial_cmp(&b.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    let n_full = full_of.len();
    let n_fast = docked_rows.len();

    let mut all_rows = docked_rows;
    all_rows.extend(failed_rows);
    all_rows.extend(rejects);

    let elapsed_ms = t_all.elapsed().as_secs_f64() * 1000.0;
    // пропускная способность: стадии 1+2 на всю библиотеку
    let lig_per_sec = if fast_ms_total > 0.0 {
        n_input as f64 / (fast_ms_total / 1000.0)
    } else {
        0.0
    };

    Ok(ScreenResult {
        rows: all_rows,
        n_input,
        n_prefiltered,
        n_fast,
        n_full,
        elapsed_ms,
        lig_per_sec,
        workers: actual_workers(opts.workers),
        pocket_desc,
        seed_note: SEED_NOTE,
        errors,
    })
}

/// Заметка о детерминизме (единая для отчётов).
const SEED_NOTE: &str = "сид = FNV(SMILES|карман); 1 воркер ≡ N воркеров (тест)";

/// Строка отсева/ошибки без докинга.
fn reject_row(l: &ScreenLigand, reason: &'static str) -> ScreenRow {
    ScreenRow {
        name: l.name.clone(),
        smiles: if l.smiles.is_empty() {
            l.graph.hill_formula()
        } else {
            l.smiles.clone()
        },
        formula: l.graph.hill_formula(),
        stage: 1,
        docked: false,
        reject: Some(reason),
        score: 0.0,
        dg_ensemble: 0.0,
        log10_kd: 0.0,
        hbonds: 0,
        clusters: 0,
        cluster_rmsd: None,
        lipinski: l.lipinski,
        mw: l.mw,
        ms: 0.0,
    }
}

/// Причина отсева пре-фильтрами (None = проходит).
fn prefilter_reject(l: &ScreenLigand, opts: &ScreenOptions) -> Option<&'static str> {
    if l.heavy == 0 {
        return Some("нет тяжёлых атомов");
    }
    if l.heavy > opts.max_heavy {
        return Some("слишком большой (>60 тяжёлых атомов)");
    }
    if l.mw > opts.max_mw {
        return Some("масса > 500 (Липински)");
    }
    if l.logp > opts.max_logp {
        return Some("logP > 5 (Липински)");
    }
    if l.hbd > opts.max_hbd {
        return Some("доноров > 5 (Липински)");
    }
    if l.hba > opts.max_hba {
        return Some("акцепторов > 10 (Липински)");
    }
    if l.charge.abs() > opts.max_charge {
        return Some("заряд |q| > 3");
    }
    if l.graph.components > 1 {
        return Some("не одна молекула (фрагменты)");
    }
    None
}

/// Строка из результата докинга.
fn row_of(l: &ScreenLigand, r: &super::dock::DockResult, stage: u8, ms: f64) -> ScreenRow {
    ScreenRow {
        name: l.name.clone(),
        smiles: if l.smiles.is_empty() {
            l.graph.hill_formula()
        } else {
            l.smiles.clone()
        },
        formula: l.graph.hill_formula(),
        stage,
        docked: true,
        reject: None,
        score: r.best.delta_g,
        dg_ensemble: r.dg_ensemble,
        log10_kd: r.log10_kd,
        hbonds: r.best.hbonds,
        clusters: r.clusters.len(),
        cluster_rmsd: r.clusters.get(1).and_then(|c| c.diversity),
        lipinski: l.lipinski,
        mw: l.mw,
        ms,
    }
}

/// Число воркеров, которое реально будет использовано.
fn actual_workers(requested: usize) -> usize {
    if requested > 0 {
        requested
    } else {
        rayon::current_num_threads()
    }
}

/// Параллельный прогон с фиксированным пулом (если workers > 0).
/// ВОЗВРАТ в порядке входа (детерминизм), сиды — per-лиганд.
/// Ошибки prepare/докинга возвращаются на месте (без паники).
fn run_parallel<P, S, R>(
    workers: usize,
    ligands: &[&ScreenLigand],
    prepare: P,
    score: S,
) -> Vec<(usize, Result<R, String>)>
where
    P: Fn(&ScreenLigand) -> Result<LigandPrep, String> + Sync,
    S: Fn(&ScreenLigand, &LigandPrep) -> Result<R, String> + Sync,
    R: Send,
{
    let one = |(i, l): (usize, & &ScreenLigand)| {
        (
            i,
            prepare(l).and_then(|lig| {
                if lig.conf.heavy_map.is_empty() {
                    Err(format!("«{}»: лиганд без тяжёлых атомов", l.name))
                } else {
                    score(l, &lig)
                }
            }),
        )
    };
    if workers > 0 {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap_or_else(|_| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(1)
                    .build()
                    .unwrap()
            });
        pool.install(|| ligands.par_iter().enumerate().map(one).collect())
    } else {
        ligands.par_iter().enumerate().map(one).collect()
    }
}

// ─── Отчёт: моноширинная таблица + JSON ────────────────────────────────

/// Таблица скрининга в терминал (моноширинная, как dock_text).
pub fn screen_text(r: &ScreenResult, top: usize) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(2048);
    let _ = writeln!(
        s,
        "УЛЬТРА-СКРИНИНГ · карман: {} · воркеров: {}",
        r.pocket_desc, r.workers
    );
    let _ = writeln!(
        s,
        "библиотека: {} записей → пре-фильтры {} → быстрый MC {} → полный докинг {}",
        r.n_input, r.n_prefiltered, r.n_fast, r.n_full
    );
    let _ = writeln!(
        s,
        "пропускная способность (стадии 1+2): {:.1} лиг/с · время {:.0} мс",
        r.lig_per_sec, r.elapsed_ms
    );
    let _ = writeln!(
        s,
        "детерминизм: {}; повторный прогон ≡ первому",
        r.seed_note
    );
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        " {:>3}  {:<18} {:<12} {:>9} {:>9} {:>7} {:>6} {:>5}",
        "#", "имя", "формула", "ΔG", "ΔG_анс", "lgKd", "H-св", "клст"
    );
    let _ = writeln!(s, "{}", "-".repeat(84));
    let shown = if top > 0 { top.min(r.rows.len()) } else { r.rows.len() };
    for (i, row) in r.rows.iter().take(shown).enumerate() {
        if row.docked {
            let cl_rmsd = match row.cluster_rmsd {
                Some(v) => format!("{v:.1}"),
                None => "—".into(),
            };
            let _ = writeln!(
                s,
                " {:>3}  {:<18} {:<12} {:>9.1} {:>9.1} {:>7.1} {:>6} {:>5}  стадия {} · {:.0} мс{}",
                i + 1,
                truncate(&row.name, 18),
                truncate(&row.formula, 12),
                row.score,
                row.dg_ensemble,
                row.log10_kd,
                row.hbonds,
                cl_rmsd,
                row.stage,
                row.ms,
                if row.stage == 3 { "" } else { " (топ не дошёл)" }
            );
        } else if let Some(reason) = row.reject {
            let _ = writeln!(
                s,
                " {:>3}  {:<18} {:<12} {:>9} {:>9} {:>7} {:>6} {:>5}  отсев: {}",
                i + 1,
                truncate(&row.name, 18),
                truncate(&row.formula, 12),
                "—",
                "—",
                "—",
                "—",
                "—",
                reason
            );
        } else {
            let _ = writeln!(
                s,
                " {:>3}  {:<18} {:<12} {:>9} {:>9} {:>7} {:>6} {:>5}  ошибка докинга",
                i + 1,
                truncate(&row.name, 18),
                truncate(&row.formula, 12),
                "—",
                "—",
                "—",
                "—",
                "—"
            );
        }
    }
    s
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        format!("{:<width$}", s, width = n)
    } else {
        let t: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

/// JSON-выгрузка результата (honest: ручная сериализация без serde в chem).
pub fn screen_json(r: &ScreenResult) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(4096);
    let _ = writeln!(s, "{{");
    let _ = writeln!(
        s,
        "  \"n_input\": {}, \"n_prefiltered\": {}, \"n_fast\": {}, \"n_full\": {},",
        r.n_input, r.n_prefiltered, r.n_fast, r.n_full
    );
    let _ = writeln!(
        s,
        "  \"lig_per_sec\": {:.3}, \"elapsed_ms\": {:.1}, \"workers\": {},",
        r.lig_per_sec, r.elapsed_ms, r.workers
    );
    let _ = writeln!(s, "  \"pocket\": \"{}\",", escape_json(&r.pocket_desc));
    let _ = writeln!(s, "  \"rows\": [");
    for (i, row) in r.rows.iter().enumerate() {
        let _ = write!(
            s,
            "    {{\"name\": \"{}\", \"smiles\": \"{}\", \"formula\": \"{}\", \"stage\": {}, \"docked\": {}, \"reject\": {}, \"score\": {:.4}, \"dg_ensemble\": {:.4}, \"log10_kd\": {:.4}, \"hbonds\": {}, \"clusters\": {}, \"cluster_rmsd\": {}, \"lipinski\": {}, \"mw\": {:.2}, \"ms\": {:.1}}}",
            escape_json(&row.name),
            escape_json(&row.smiles),
            escape_json(&row.formula),
            row.stage,
            row.docked,
            match row.reject {
                Some(rj) => format!("\"{}\"", escape_json(rj)),
                None => "null".into(),
            },
            row.score,
            row.dg_ensemble,
            row.log10_kd,
            row.hbonds,
            row.clusters,
            match row.cluster_rmsd {
                Some(v) => format!("{v:.3}"),
                None => "null".into(),
            },
            row.lipinski,
            row.mw,
            row.ms
        );
        if i + 1 < r.rows.len() {
            s.push(',');
        }
        s.push('\n');
    }
    let _ = writeln!(s, "  ]");
    let _ = writeln!(s, "}}");
    s
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

// ─── Тесты ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Мини-библиотека из SMILES (4 лиганда: бензамидин + 3 декоя).
    fn mini_entries() -> Vec<LibraryEntry> {
        [
            ("NC(=N)c1ccccc1", "benzamidine"),
            ("Cc1ccccc1", "toluene"),
            ("c1ccccc1", "benzene"),
            ("CCO", "ethanol"),
        ]
        .iter()
        .map(|(smi, name)| LibraryEntry::Smiles {
            name: name.to_string(),
            smiles: smi.to_string(),
        })
        .collect()
    }

    fn load_3ptb() -> MacroMol {
        MacroMol::from_file("tests/fixtures/3ptb.pdb").expect("3ptb.pdb в fixtures")
    }

    #[test]
    fn prefilter_lipinski_rejects_big() {
        // декан: logP ~ 5 — зависит от оценки; полная проверка на массу:
        let opts = ScreenOptions::default();
        let entries = vec![LibraryEntry::Smiles {
            name: "big".into(),
            smiles: "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC".into(),
        }];
        let l = ScreenLigand::from_entry(&entries[0]).unwrap();
        let reject = prefilter_reject(&l, &opts);
        assert!(reject.is_some(), "полимер должен отсеяться: {reject:?}");
        assert!(l.mw > 500.0);
    }

    #[test]
    fn prefilter_passes_druglike() {
        let opts = ScreenOptions::default();
        let entries = mini_entries();
        for e in &entries {
            let l = ScreenLigand::from_entry(e).unwrap();
            assert!(
                prefilter_reject(&l, &opts).is_none(),
                "«{}» не должен отсеиваться",
                l.name
            );
        }
    }

    #[test]
    fn screen_cascade_determinism_one_vs_many_workers() {
        let mm = load_3ptb();
        let entries = mini_entries();
        let spec = PocketSpec::Blind;
        // 1 воркер
        let opts1 = ScreenOptions {
            workers: 1,
            top: 2,
            fast_steps: 120, // ускоренный тест
            full_steps: 240,
            ..Default::default()
        };
        // 4 воркера
        let opts4 = ScreenOptions {
            workers: 4,
            ..opts1.clone()
        };
        let r1 = screen_library(&entries, &mm, &spec, &opts1).unwrap();
        let r4 = screen_library(&entries, &mm, &spec, &opts4).unwrap();
        assert_eq!(r1.rows.len(), r4.rows.len());
        for (a, b) in r1.rows.iter().zip(r4.rows.iter()) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.stage, b.stage);
            assert_eq!(a.docked, b.docked);
            assert_eq!(a.hbonds, b.hbonds, "{}: H-связи разошлись", a.name);
            assert!(
                (a.score - b.score).abs() < 1e-9,
                "{}: score {} vs {}",
                a.name,
                a.score,
                b.score
            );
            assert!((a.dg_ensemble - b.dg_ensemble).abs() < 1e-9);
        }
    }

    #[test]
    fn screen_repeat_run_identical() {
        let mm = load_3ptb();
        let entries = mini_entries();
        let opts = ScreenOptions {
            workers: 2,
            top: 2,
            fast_steps: 120,
            full_steps: 240,
            ..Default::default()
        };
        let ra = screen_library(&entries, &mm, &PocketSpec::Blind, &opts).unwrap();
        let rb = screen_library(&entries, &mm, &PocketSpec::Blind, &opts).unwrap();
        for (a, b) in ra.rows.iter().zip(rb.rows.iter()) {
            assert_eq!(a.name, b.name);
            assert!((a.score - b.score).abs() < 1e-12, "повтор ≠ первый");
        }
    }

    #[test]
    fn screen_json_shape() {
        let mm = load_3ptb();
        let entries = mini_entries();
        let opts = ScreenOptions {
            workers: 1,
            top: 2,
            fast_steps: 100,
            full_steps: 100,
            ..Default::default()
        };
        let r = screen_library(&entries, &mm, &PocketSpec::Blind, &opts).unwrap();
        let j = screen_json(&r);
        assert!(j.starts_with('{'));
        assert!(j.contains("\"n_input\": 4"));
        assert!(j.contains("\"rows\": ["));
        assert!(j.contains("\"name\": \"benzamidine\""));
        assert!(j.contains("\"score\":"));
        // валидность кавычек/запятых: без незакрытых строк
        assert!(j.trim_end().ends_with('}'));
    }

    #[test]
    fn screen_text_table_has_header_and_rows() {
        let mm = load_3ptb();
        let entries = mini_entries();
        let opts = ScreenOptions {
            workers: 1,
            top: 2,
            fast_steps: 100,
            full_steps: 100,
            ..Default::default()
        };
        let r = screen_library(&entries, &mm, &PocketSpec::Blind, &opts).unwrap();
        let t = screen_text(&r, 10);
        assert!(t.contains("УЛЬТРА-СКРИНИНГ"));
        assert!(t.contains("лиг/с"));
        assert!(t.contains("benzamidine"));
    }

    /// B5: валидация обогащения на мини-библиотеке 3PTB (честная).
    ///
    /// 30 молекул: бензамидиний-КАТИОН (физиологическая форма активного
    /// при pH 7: pKa ≈ 11.5; кристаллический лиганд 3PTB, остаток BAM)
    /// + 29 нейтральных декоев близкой массы без основных групп (S1
    /// трипсина предпочитает катионы — декои их не имеют). Протокол —
    /// redocking (карман auto по BAM).
    ///
    /// Что вскрыла валидация (и что починено в этой же ступени):
    /// 1. кулон без контактного пола вознаграждал столкновения —
    ///    вератрол набирал E_elec = −808 при 19 клатшах; пол
    ///    r_eff = max(r, 0.8·rij) сатурирует (фикс + тест
    ///    clash_coulomb_saturation, JIT синхронно);
    /// 2. нейтральный бензамидин (канон PubChem) зарядами Гастайгера
    ///    ОТТАЛКИВАЕТСЯ от Asp189 — актив обязан протонироваться, как
    ///    в реальных скрининг-библиотеках;
    /// 3. писатель M CHG терял заряды в round-trip (тест round_trip_charged).
    ///
    /// Пределы метода (честно): одна мишень, 1 актив, декои подобраны
    /// «по смыслу», а не DUD-E-протоколом; бензофенон (#1) честно
    /// выигрывает по липофильности в этой скоринг-функции — актив в
    /// топ-3 подтверждает РАНЖИРОВАНИЕ каскада, не качество скоринга.
    #[test]
    fn enrichment_benzamidine_top3() {
        let (mols, skipped) =
            super::super::sdf::read_sdf("tests/fixtures/mini_lib.sdf").expect("mini_lib.sdf");
        assert_eq!(mols.len(), 30, "30 записей в мини-библиотеке");
        assert!(skipped.is_empty(), "пропусков быть не должно: {skipped:?}");
        let entries: Vec<LibraryEntry> = mols.into_iter().map(LibraryEntry::Sdf).collect();
        let mm = load_3ptb();
        // полноценный каскад: быстрая стадия по умолчанию, топ-3 в полный
        let opts = ScreenOptions {
            workers: 2,
            top: 3,
            ..Default::default()
        };
        let r = screen_library(&entries, &mm, &PocketSpec::Auto, &opts).unwrap();
        assert_eq!(r.n_input, 30);
        // декои проходят пре-фильтры (все лекарепригодны) — каскад честный
        assert_eq!(r.n_prefiltered, 30, "все 30 проходят пре-фильтры");
        let top3: Vec<&str> = r.rows.iter().take(3).map(|x| x.name.as_str()).collect();
        eprintln!("топ-3: {top3:?}");
        assert!(
            top3.contains(&"benzamidine"),
            "актив не в топ-3: {top3:?} — enrichment@10% провален"
        );
        // скоринг актива — не хуже типичных декоев (ΔG ниже = сильнее)
        let bam = r.rows.iter().find(|x| x.name == "benzamidine").unwrap();
        let med = r.rows[r.n_fast / 2].score;
        assert!(
            bam.score <= med,
            "ΔG актива {bam:.1} хуже медианы {med:.1}",
            bam = bam.score
        );
    }
}
