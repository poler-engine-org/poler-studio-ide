//! # Формальный верификатор квантовых схем (цикл P)
//!
//! Трёхслойная процедура в духе SMT-решателей — препроцессинг,
//! фальсификация, доказательство:
//!
//! 1. **Структурный линт** (прекондиции): индексы кубитов в регистре,
//!    коллизии контролей, мёртвые кубиты.
//! 2. **Быстрая фальсификация** на случайных состояниях (аналог bounded
//!    model checking): контрпример к унитарности/эквивалентности ищется
//!    за микросекунды, до тяжёлой алгебры.
//! 3. **Полное доказательство**: матрица 2ⁿ×2ⁿ оператора над
//!    ℤ[1/√2, i] — точным кольцом Клиффорда+T (невязка — структурный
//!    нуль, `ProvedExact`) — либо над f64 с явным допуском
//!    (`VerifiedNumeric`).
//!
//! Схемы вне размера (`--n` велика) получают честный вердикт
//! `VerifiedSampling` — вероятностное подтверждение на случайных
//! состояниях без построения полной матрицы.
//!
//! Слои соответствуют архитектуре tools/verifiers (Z3-арбитры циклов
//! G/H), но живут в самом крейте: вердикт доступен CLI `pqc verify`,
//! shell-команде `quantum verify` и MCP-инструменту `poler_quantum`.

use crate::error::{PqcError, Result};
use crate::exact::{ExactCx, ExactStatevector};
use crate::gates::Gate;
use crate::qpc::{Circuit, Op};
use crate::statevector::Statevector;

/// Предел точного пути: 2⁸×2⁸ матриц над ℤ[1/√2, i]
/// (256² элементов кольца — доли секунды).
pub const MAX_EXACT_QUBITS: usize = 8;

/// Предел численного пути: 2¹⁰×2¹⁰ комплексных f64 (~17 МБ).
pub const MAX_NUMERIC_QUBITS: usize = 10;

/// Вердикт верификатора.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    /// Доказано точно в кольце ℤ[1/√2, i]: невязка — структурный нуль.
    ProvedExact,
    /// Доказано численно: полная матрица над f64, допуск `tol`.
    VerifiedNumeric(f64),
    /// Подтверждено вероятностно: случайные состояния, полная матрица
    /// за пределами размера (честная градация уверенности).
    VerifiedSampling(f64),
    /// Опровергнуто: предъявлен контрпример, `deviation` — норма нарушения.
    Refuted(f64),
}

/// Отчёт верификации.
#[derive(Clone, Debug)]
pub struct VerificationReport {
    /// Свойство: "unitarity" | "equivalence" | "teleport_channel".
    pub property: &'static str,
    /// Человекочитаемый субъект: "qft(n=3)".
    pub subject: String,
    /// Кубитов в схеме.
    pub n_qubits: usize,
    /// Размерность гильбертова пространства 2ⁿ.
    pub dim: usize,
    /// Число унитарных операций.
    pub gate_count: usize,
    /// Метод: "exact:Z[1/√2,i]" | "numeric:f64" | "hybrid" | "sampling:f64".
    pub method: &'static str,
    /// Вердикт.
    pub verdict: Verdict,
    /// Максимальная невязка (0 — структурный нуль точного пути).
    pub max_deviation: f64,
    /// Промежуточные проверки: (имя, ok, невязка).
    pub checks: Vec<(&'static str, bool, f64)>,
    /// Заметки (линт, допущения).
    pub notes: Vec<String>,
}

impl VerificationReport {
    /// Однострочный вердикт для CLI/логов.
    pub fn verdict_line(&self) -> String {
        match self.verdict {
            Verdict::ProvedExact => "ДОКАЗАНО ТОЧНО (кольцо ℤ[1/√2, i], невязка — структурный нуль)".to_string(),
            Verdict::VerifiedNumeric(tol) => format!(
                "ПОДТВЕРЖДЕНО ЧИСЛЕННО (допуск {tol:.1e}, невязка {:.3e})",
                self.max_deviation
            ),
            Verdict::VerifiedSampling(tol) => format!(
                "ПОДТВЕРЖДЕНО ВЕРОЯТНОСТНО (выборка состояний, допуск {tol:.1e}, \
                 невязка {:.3e}; полная матрица вне предела n={})",
                self.max_deviation, MAX_NUMERIC_QUBITS
            ),
            Verdict::Refuted(d) => format!(
                "ОПРОВЕРГНУТО (контрпример найден, невязка {d:.3e})"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Структурный линт
// ---------------------------------------------------------------------------

/// Мягкие предупреждения линта (жёсткие ошибки — `Err` из `verify_*`).
pub fn lint(c: &Circuit) -> Vec<String> {
    let n = c.n_qubits();
    let mut warns = Vec::new();
    let mut used = vec![false; n];
    let mark = |q: usize, used: &mut Vec<bool>| {
        if q < n {
            used[q] = true;
        }
    };
    for (i, op) in c.ops().iter().enumerate() {
        match op {
            Op::Gate(g) => match *g {
                Gate::Ry { q, .. }
                | Gate::Rx { q, .. }
                | Gate::Rz { q, .. }
                | Gate::H { q }
                | Gate::X { q }
                | Gate::Y { q }
                | Gate::Z { q }
                | Gate::S { q }
                | Gate::T { q }
                | Gate::Sdg { q }
                | Gate::Tdg { q }
                | Gate::U2 { q, .. } => mark(q, &mut used),
                Gate::Cx { control, target }
                | Gate::Cz { control, target }
                | Gate::Cp { control, target, .. } => {
                    mark(control, &mut used);
                    mark(target, &mut used);
                    if control == target {
                        warns.push(format!("op[{i}]: контроль = цель = {control}"));
                    }
                }
                Gate::Swap { a, b } => {
                    mark(a, &mut used);
                    mark(b, &mut used);
                    if a == b {
                        warns.push(format!("op[{i}]: SWAP кубита {a} на себя"));
                    }
                }
                Gate::Ccx { c1, c2, target } => {
                    mark(c1, &mut used);
                    mark(c2, &mut used);
                    mark(target, &mut used);
                    if c1 == c2 || c1 == target || c2 == target {
                        warns.push(format!("op[{i}]: коллизия Ccx({c1},{c2},{target})"));
                    }
                }
            },
            Op::Measure { q } => mark(*q, &mut used),
            _ => {}
        }
    }
    for (q, u) in used.iter().enumerate() {
        if !u {
            warns.push(format!("кубит {q} не задействован ни одной операцией"));
        }
    }
    warns
}

/// Жёсткий линт: индексы в регистре. Возвращает Err при выходе за пределы.
fn lint_hard(c: &Circuit) -> Result<Vec<String>> {
    let n = c.n_qubits();
    let mut errs = Vec::new();
    for (i, op) in c.ops().iter().enumerate() {
        let mut check = |q: usize, what: &str| {
            if q >= n {
                errs.push(format!("op[{i}] {what}: кубит {q} вне регистра (n={n})"));
            }
        };
        match op {
            Op::Gate(g) => match *g {
                Gate::Ry { q, .. }
                | Gate::Rx { q, .. }
                | Gate::Rz { q, .. }
                | Gate::H { q }
                | Gate::X { q }
                | Gate::Y { q }
                | Gate::Z { q }
                | Gate::S { q }
                | Gate::T { q }
                | Gate::Sdg { q }
                | Gate::Tdg { q }
                | Gate::U2 { q, .. } => check(q, "однокубитный"),
                Gate::Cx { control, target }
                | Gate::Cz { control, target }
                | Gate::Cp { control, target, .. } => {
                    check(control, "контроль");
                    check(target, "цель");
                }
                Gate::Swap { a, b } => {
                    check(a, "swap-a");
                    check(b, "swap-b");
                }
                Gate::Ccx { c1, c2, target } => {
                    check(c1, "ccx-c1");
                    check(c2, "ccx-c2");
                    check(target, "ccx-target");
                }
            },
            Op::Measure { q } => check(*q, "measure"),
            _ => {}
        }
    }
    if errs.is_empty() {
        Ok(lint(c))
    } else {
        Err(PqcError::BadArgument {
            what: errs.join("; "),
        })
    }
}

// ---------------------------------------------------------------------------
// Случайные состояния (splitmix64 — детерминированный, без внешних ящиков)
// ---------------------------------------------------------------------------

struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Случайная фаза [0, 2π).
    fn angle(&mut self) -> f64 {
        self.next_u64() as f64 / u64::MAX as f64 * core::f64::consts::TAU
    }
    /// Равномерно-фазовое состояние единичной нормы: amp_i = e^{iφ_i}/√d.
    fn state(&mut self, dim: usize) -> Vec<(f64, f64)> {
        let norm = 1.0 / (dim as f64).sqrt();
        (0..dim)
            .map(|_| {
                let phi = self.angle();
                (phi.cos() * norm, phi.sin() * norm)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Численное применение операций (гейты + фазовые оракулы)
// ---------------------------------------------------------------------------

fn apply_ops_f64(sv: &mut Statevector, ops: &[Op]) -> Result<()> {
    for op in ops {
        match *op {
            Op::Gate(g) => sv.apply(g)?,
            Op::FlipPhase { mask } => {
                let m = mask;
                for (i, a) in sv.amplitudes_mut().iter_mut().enumerate() {
                    if m != 0 && i & m == m {
                        a.re = -a.re;
                        a.im = -a.im;
                    }
                }
            }
            Op::FlipIndex { idx } => {
                let amps = sv.amplitudes_mut();
                if idx < amps.len() {
                    amps[idx].re = -amps[idx].re;
                    amps[idx].im = -amps[idx].im;
                }
            }
            Op::FlipZero => {
                for (i, a) in sv.amplitudes_mut().iter_mut().enumerate() {
                    if i != 0 {
                        a.re = -a.re;
                        a.im = -a.im;
                    }
                }
            }
            Op::Barrier | Op::Measure { .. } | Op::MeasureAll => {}
            Op::PrepComb { .. } => {
                return Err(PqcError::BadArgument {
                    what: "verify: PrepComb — неунитарная подготовка состояния".into(),
                })
            }
        }
    }
    Ok(())
}

/// Фильтр унитарной части схемы (без Measure/Barrier).
fn unitary_part(c: &Circuit) -> Result<Circuit> {
    let mut f = Circuit::new(c.n_qubits())?;
    for op in c.ops() {
        match op {
            Op::PrepComb { .. } => {
                return Err(PqcError::BadArgument {
                    what: "verify: схема содержит неунитарную подготовку (PrepComb)".into(),
                })
            }
            Op::Barrier | Op::Measure { .. } | Op::MeasureAll => {}
            other => {
                f.push(other.clone());
            }
        }
    }
    Ok(f)
}

// ---------------------------------------------------------------------------
// Матрица оператора
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum UniMatrix {
    Exact(Vec<Vec<ExactCx>>),
    F64(Vec<Vec<(f64, f64)>>),
}

impl UniMatrix {
    fn method(&self) -> &'static str {
        match self {
            UniMatrix::Exact(_) => "exact:Z[1/√2,i]",
            UniMatrix::F64(_) => "numeric:f64",
        }
    }
    fn to_f64(&self) -> Vec<Vec<(f64, f64)>> {
        match self {
            UniMatrix::F64(rows) => rows.clone(),
            UniMatrix::Exact(rows) => rows
                .iter()
                .map(|r| r.iter().map(|c| c.to_f64()).collect())
                .collect(),
        }
    }
}

/// Построить матрицу 2ⁿ×2ⁿ оператора схемы: столбец j = U|j⟩.
///
/// Точный путь пробуется первым; при выходе за кольцо (Ry вне сеток π/2,
/// CP(π/8)…) — честный откат на f64.
fn build_matrix(c: &Circuit) -> Result<(UniMatrix, usize)> {
    let n = c.n_qubits();
    let dim = 1usize << n;
    let filtered = unitary_part(c)?;
    let gate_count = filtered.ops().len();

    // 1) Точный путь: ℤ[1/√2, i].
    if n <= MAX_EXACT_QUBITS {
        let mut cols: Vec<Vec<ExactCx>> = Vec::with_capacity(dim);
        let mut exact_ok = true;
        for j in 0..dim {
            let mut sv = ExactStatevector::new(n)?;
            for b in 0..n {
                if j & (1usize << b) != 0 {
                    sv.apply(Gate::X { q: b })?;
                }
            }
            match sv.run_circuit(&filtered) {
                Ok(()) => cols.push(sv.amplitudes().to_vec()),
                Err(PqcError::NotExactGate { .. }) => {
                    exact_ok = false;
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        if exact_ok {
            let mut rows = vec![vec![ExactCx::ZERO; dim]; dim];
            for (j, col) in cols.iter().enumerate() {
                for i in 0..dim {
                    rows[i][j] = col[i];
                }
            }
            return Ok((UniMatrix::Exact(rows), gate_count));
        }
    }

    // 2) Численный путь.
    let mut rows = vec![vec![(0.0f64, 0.0f64); dim]; dim];
    for j in 0..dim {
        let mut sv = Statevector::new(n)?;
        for b in 0..n {
            if j & (1usize << b) != 0 {
                sv.apply(Gate::X { q: b })?;
            }
        }
        apply_ops_f64(&mut sv, filtered.ops())?;
        for (i, a) in sv.amplitudes().iter().enumerate() {
            rows[i][j] = (a.re, a.im);
        }
    }
    Ok((UniMatrix::F64(rows), gate_count))
}

// ---------------------------------------------------------------------------
// Комплексная арифметика f64 (маленькие помощники)
// ---------------------------------------------------------------------------

fn cadd(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 + b.0, a.1 + b.1)
}
fn cmul(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}
fn cconj(a: (f64, f64)) -> (f64, f64) {
    (a.0, -a.1)
}
fn cabs(a: (f64, f64)) -> f64 {
    a.0.hypot(a.1)
}
fn cdot_conj(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    cmul(cconj(a), b)
}

/// U†U над f64; возвращает максимум внедиагонали и |диагональ − 1|.
fn unitarity_deviation_f64(u: &[Vec<(f64, f64)>]) -> (f64, f64) {
    let dim = u.len();
    let mut off = 0.0f64;
    let mut dia = 0.0f64;
    for i in 0..dim {
        for j in 0..dim {
            // (U†U)[i][j] = Σ_k conj(U[k][i])·U[k][j]
            let mut acc = (0.0, 0.0);
            for k in 0..dim {
                acc = cadd(acc, cdot_conj(u[k][i], u[k][j]));
            }
            if i == j {
                dia = dia.max((cabs(acc) - 1.0).abs());
            } else {
                off = off.max(cabs(acc));
            }
        }
    }
    (off, dia)
}

// ---------------------------------------------------------------------------
// Публичные проверки
// ---------------------------------------------------------------------------

/// Верифицировать унитарность оператора схемы: U†U = I.
pub fn verify_unitary(c: &Circuit) -> Result<VerificationReport> {
    let started = std::time::Instant::now();
    let soft = lint_hard(c)?;
    let n = c.n_qubits();
    let dim = 1usize << n;
    let subject = format!("unitarity(n={n})");
    let mut notes = soft.clone();
    let mut checks: Vec<(&'static str, bool, f64)> = Vec::new();

    // Слой 2: фальсификация на случайных состояниях.
    if n <= crate::statevector::MAX_QUBITS {
        let filtered = unitary_part(c)?;
        let mut rng = SplitMix64(0xC0FFEE);
        let mut worst = 0.0f64;
        for _ in 0..3 {
            let psi = rng.state(dim);
            let mut sv = Statevector::new(n)?;
            for (i, a) in psi.iter().enumerate() {
                sv.amplitudes_mut()[i] = crate::complex::Cx {
                    re: a.0,
                    im: a.1,
                };
            }
            apply_ops_f64(&mut sv, filtered.ops())?;
            let after: f64 = sv
                .amplitudes()
                .iter()
                .map(|a| a.re * a.re + a.im * a.im)
                .sum();
            worst = worst.max((after - 1.0).abs());
        }
        checks.push(("норма случайных состояний", worst <= 1e-9, worst));
        if worst > 1e-6 {
            return Ok(finish(
                "unitarity",
                &subject,
                n,
                dim,
                c.ops().len(),
                "sampling:f64",
                Verdict::Refuted(worst),
                worst,
                checks,
                notes,
                started,
            ));
        }
    }

    // Слой 3: полная матрица.
    if n <= MAX_NUMERIC_QUBITS {
        let (m, gate_count) = build_matrix(c)?;
        let (verdict, dev) = match &m {
            UniMatrix::Exact(rows) => {
                // U†U над кольцом: структурная проверка.
                // Любое несовпадение элементов — опровержение (арифметика
                // кольца точна, «почти ноль» невозможен).
                let one = ExactCx::ONE;
                let zero = ExactCx::ZERO;
                let mut mismatch: Option<(usize, usize, ExactCx)> = None;
                'outer: for i in 0..dim {
                    for j in 0..dim {
                        let mut acc = ExactCx::ZERO;
                        for k in 0..dim {
                            acc = acc.add(rows[k][i].conj().mul(rows[k][j])?)?;
                        }
                        let expect = if i == j { one } else { zero };
                        if acc != expect {
                            mismatch = Some((i, j, acc));
                            break 'outer;
                        }
                    }
                }
                match mismatch {
                    None => (Verdict::ProvedExact, 0.0),
                    Some((i, j, acc)) => {
                        // f64-невязка информативна (место нарушения).
                        let (er, ei) = acc.to_f64();
                        let target = if i == j { 1.0 } else { 0.0 };
                        let dev = (er - target).hypot(ei);
                        (Verdict::Refuted(dev), dev)
                    }
                }
            }
            UniMatrix::F64(rows) => {
                let (off, dia) = unitarity_deviation_f64(rows);
                let dev = off.max(dia);
                let tol = 1e-9 * (1.0 + gate_count as f64 / 100.0);
                if dev <= tol {
                    (Verdict::VerifiedNumeric(tol), dev)
                } else {
                    (Verdict::Refuted(dev), dev)
                }
            }
        };
        checks.push((
            "U†U = I",
            !matches!(verdict, Verdict::Refuted(_)),
            dev,
        ));
        let method = m.method();
        if method.starts_with("exact") {
            notes.push("доказательство в кольце ℤ[1/√2, i]: невязка — структурный нуль".into());
        }
        return Ok(finish(
            "unitarity",
            &subject,
            n,
            dim,
            gate_count,
            method,
            verdict,
            dev,
            checks,
            notes,
            started,
        ));
    }

    // За пределом матриц — вероятностный вердикт.
    notes.push(format!(
        "полная матрица 2^{n}×2^{n} за пределом точного пути (n ≤ {MAX_EXACT_QUBITS}) \
         и численного (n ≤ {MAX_NUMERIC_QUBITS})"
    ));
    let mut worst = 0.0f64;
    let filtered = unitary_part(c)?;
    let mut rng = SplitMix64(0xBEEF);
    for _ in 0..8 {
        let psi = rng.state(dim.min(1usize << 16));
        let mut sv = Statevector::new(n)?;
        let take = psi.len().min(sv.amplitudes().len());
        for i in 0..take {
            sv.amplitudes_mut()[i] = crate::complex::Cx {
                re: psi[i].0,
                im: psi[i].1,
            };
        }
        apply_ops_f64(&mut sv, filtered.ops())?;
        let after: f64 = sv
            .amplitudes()
            .iter()
            .map(|a| a.re * a.re + a.im * a.im)
            .sum();
        worst = worst.max((after - 1.0).abs());
    }
    let verdict = if worst <= 1e-9 {
        Verdict::VerifiedSampling(1e-9)
    } else {
        Verdict::Refuted(worst)
    };
    Ok(finish(
        "unitarity",
        &subject,
        n,
        dim,
        c.ops().len(),
        "sampling:f64",
        verdict,
        worst,
        checks,
        notes,
        started,
    ))
}

/// Верифицировать эквивалентность двух схем: U_A = U_B
/// (с точностью до глобальной фазы при `up_to_global_phase`).
pub fn verify_equivalence(
    a: &Circuit,
    b: &Circuit,
    up_to_global_phase: bool,
) -> Result<VerificationReport> {
    let started = std::time::Instant::now();
    if a.n_qubits() != b.n_qubits() {
        return Err(PqcError::BadArgument {
            what: format!(
                "эквивалентность: разные регистры ({} и {} кубитов)",
                a.n_qubits(),
                b.n_qubits()
            ),
        });
    }
    let n = a.n_qubits();
    let dim = 1usize << n;
    let subject = format!("equivalence(n={n}, phase={up_to_global_phase})");
    let mut notes = Vec::new();
    notes.extend(lint_hard(a)?);
    notes.extend(lint_hard(b)?);
    let mut checks: Vec<(&'static str, bool, f64)> = Vec::new();

    // Слой 2: фальсификация.
    let fa = unitary_part(a)?;
    let fb = unitary_part(b)?;
    if n <= crate::statevector::MAX_QUBITS {
        let mut rng = SplitMix64(0x5EED);
        let mut worst = 0.0f64;
        for _ in 0..3 {
            let psi = rng.state(dim);
            let apply = |f: &Circuit| -> Result<Vec<(f64, f64)>> {
                let mut sv = Statevector::new(n)?;
                for (i, p) in psi.iter().enumerate() {
                    sv.amplitudes_mut()[i] = crate::complex::Cx { re: p.0, im: p.1 };
                }
                apply_ops_f64(&mut sv, f.ops())?;
                Ok(sv
                    .amplitudes()
                    .iter()
                    .map(|x| (x.re, x.im))
                    .collect())
            };
            let va = apply(&fa)?;
            let vb = apply(&fb)?;
            let dev = if up_to_global_phase {
                // фазовая нормализация по первому значимому входу
                let (ia, ib) = (va[0], vb[0]);
                let lam = if cabs(ib) < 1e-12 {
                    (1.0, 0.0)
                } else {
                    cmul(ia, cconj(ib))
                };
                let norm = cabs(lam);
                let lam = if norm < 1e-12 {
                    (1.0, 0.0)
                } else {
                    (lam.0 / norm, lam.1 / norm)
                };
                va.iter()
                    .zip(vb.iter())
                    .map(|(x, y)| cabs(cadd(*x, cmul((-lam.0, -lam.1), *y))))
                    .fold(0.0, f64::max)
            } else {
                va.iter()
                    .zip(vb.iter())
                    .map(|(x, y)| cabs(cadd(*x, (-y.0, -y.1))))
                    .fold(0.0, f64::max)
            };
            worst = worst.max(dev);
        }
        checks.push(("случайные состояния", worst <= 1e-9, worst));
        if worst > 1e-6 {
            return Ok(finish(
                "equivalence",
                &subject,
                n,
                dim,
                a.ops().len() + b.ops().len(),
                "sampling:f64",
                Verdict::Refuted(worst),
                worst,
                checks,
                notes,
                started,
            ));
        }
    }

    // Слой 3: полные матрицы.
    if n > MAX_NUMERIC_QUBITS {
        return Ok(finish(
            "equivalence",
            &subject,
            n,
            dim,
            a.ops().len() + b.ops().len(),
            "sampling:f64",
            Verdict::VerifiedSampling(1e-9),
            0.0,
            checks,
            notes,
            started,
        ));
    }
    let (ma, ga) = build_matrix(a)?;
    let (mb, gb) = build_matrix(b)?;
    let method = match (ma.method(), mb.method()) {
        (x, y) if x == y => x,
        _ => "hybrid:exact×f64",
    };
    let (verdict, dev) = match (&ma, &mb) {
        (UniMatrix::Exact(ra), UniMatrix::Exact(rb)) => {
            if !up_to_global_phase {
                let mut equal = true;
                let mut dev = 0.0f64;
                'outer: for i in 0..dim {
                    for j in 0..dim {
                        if ra[i][j] != rb[i][j] {
                            equal = false;
                            let (xr, xi) = ra[i][j].to_f64();
                            let (yr, yi) = rb[i][j].to_f64();
                            dev = dev.max((xr - yr).hypot(xi - yi));
                            if dev > 1e-6 {
                                break 'outer;
                            }
                        }
                    }
                }
                if equal {
                    (Verdict::ProvedExact, 0.0)
                } else {
                    (Verdict::Refuted(dev), dev)
                }
            } else {
                // D = U_A·U_B†; эквивалентность с фазой ⟺ D = λ·I, |λ| = 1.
                // λ — первый ненулевой диагональный элемент (для унитарных
                // U_A, U_B он обязан существовать).
                let mut d = vec![vec![ExactCx::ZERO; dim]; dim];
                for i in 0..dim {
                    for j in 0..dim {
                        let mut acc = ExactCx::ZERO;
                        for k in 0..dim {
                            acc = acc.add(cmul_exact(ra[i][k], rb[j][k].conj())?)?;
                        }
                        d[i][j] = acc;
                    }
                }
                let lambda = (0..dim).find(|&i| d[i][i] != ExactCx::ZERO).map(|i| d[i][i]);
                match lambda {
                    None => {
                        // нулевая диагональ — не λ·I ни при каком λ ≠ 0
                        (Verdict::Refuted(1.0), 1.0)
                    }
                    Some(l) => {
                        let mut equal = true;
                        let mut dev = 0.0f64;
                        for i in 0..dim {
                            for j in 0..dim {
                                let expect = if i == j { l } else { ExactCx::ZERO };
                                if d[i][j] != expect {
                                    equal = false;
                                    let (xr, xi) = d[i][j].to_f64();
                                    let (yr, yi) = expect.to_f64();
                                    dev = dev.max((xr - yr).hypot(xi - yi));
                                }
                            }
                        }
                        if !equal {
                            (Verdict::Refuted(dev), dev)
                        } else {
                            // |λ| = 1 — иначе это масштаб, а не фаза.
                            let n2 = l.norm_sq()?.to_f64();
                            if (n2 - 1.0).abs() <= 1e-12 {
                                (Verdict::ProvedExact, 0.0)
                            } else {
                                let dd = (n2 - 1.0).abs();
                                (Verdict::Refuted(dd), dd)
                            }
                        }
                    }
                }
            }
        }
        _ => {
            let ua = ma.to_f64();
            let ub = mb.to_f64();
            let tol = 1e-9 * (1.0 + (ga + gb) as f64 / 100.0);
            let (verdict, dev) = if !up_to_global_phase {
                let mut dev = 0.0f64;
                for i in 0..dim {
                    for j in 0..dim {
                        dev = dev
                            .max((ua[i][j].0 - ub[i][j].0).hypot(ua[i][j].1 - ub[i][j].1));
                    }
                }
                if dev <= tol {
                    (Verdict::VerifiedNumeric(tol), dev)
                } else {
                    (Verdict::Refuted(dev), dev)
                }
            } else {
                // λ = A[i*][j*] / B[i*][j*] по максимуму |B|.
                let (mut bi, mut bj) = (0usize, 0usize);
                let mut best = 0.0;
                for i in 0..dim {
                    for j in 0..dim {
                        let m = cabs(ub[i][j]);
                        if m > best {
                            best = m;
                            (bi, bj) = (i, j);
                        }
                    }
                }
                let lam = cmul(ua[bi][bj], cconj(ub[bi][bj]));
                let norm = cabs(lam);
                if (norm - 1.0).abs() > 1e-9 {
                    let d = (norm - 1.0).abs();
                    (Verdict::Refuted(d), d)
                } else {
                    let lam = (lam.0 / norm, lam.1 / norm);
                    let mut dev = 0.0f64;
                    for i in 0..dim {
                        for j in 0..dim {
                            let scaled = cmul(lam, ub[i][j]);
                            dev = dev
                                .max((ua[i][j].0 - scaled.0).hypot(ua[i][j].1 - scaled.1));
                        }
                    }
                    if dev <= tol {
                        (Verdict::VerifiedNumeric(tol), dev)
                    } else {
                        (Verdict::Refuted(dev), dev)
                    }
                }
            };
            (verdict, dev)
        }
    };
    checks.push((
        "U_A = U_B",
        !matches!(verdict, Verdict::Refuted(_)),
        dev,
    ));
    if method.starts_with("exact") {
        notes.push("доказательство в кольце ℤ[1/√2, i]".into());
    }
    Ok(finish(
        "equivalence",
        &subject,
        n,
        dim,
        ga + gb,
        method,
        verdict,
        dev,
        checks,
        notes,
        started,
    ))
}

fn cmul_exact(a: ExactCx, b: ExactCx) -> Result<ExactCx> {
    a.mul(b)
}

/// Формальная верификация канала телепортации (цикл P).
///
/// Свойство: U_канала · (|ψ⟩⊗|00⟩ с парой Белла) = |+⟩|+⟩|ψ⟩ для всех |ψ⟩.
/// Проверяется ТОЧНО на базисе |0⟩, |1⟩ входа — по линейности это
/// покрывает произвольные α, β. Все амплитуды — элементы ℤ[1/√2, i],
/// сравнение структурное (равенство элементов кольца).
pub fn verify_teleport_channel() -> Result<VerificationReport> {
    let started = std::time::Instant::now();
    let expected = ExactCx {
        a: 1,
        b: 0,
        c: 0,
        d: 0,
        k: 1,
    }; // 1/2: |+⟩|+⟩|b⟩ — по ½ на каждый из 4 базисов с bit2 = b
    let chan = crate::algorithms::teleport()?;
    let mut checks = Vec::new();
    for b in 0..2usize {
        // Канал сам готовит пару Белла (H(1), CNOT(1→2) — первые гейты):
        // на вход подаётся только |b⟩₀ ⊗ |00⟩₁₂.
        let mut sv = ExactStatevector::new(3)?;
        if b == 1 {
            sv.apply(Gate::X { q: 0 })?;
        }
        for op in chan.ops() {
            if let Op::Gate(g) = op {
                sv.apply(*g)?;
            }
        }
        let mut ok = true;
        for (i, a) in sv.amplitudes().iter().enumerate() {
            let target = if (i >> 2) & 1 == b {
                expected
            } else {
                ExactCx::ZERO
            };
            if *a != target {
                ok = false;
                break;
            }
        }
        checks.push((if b == 0 { "базис |0⟩" } else { "базис |1⟩" }, ok, 0.0));
        if !ok {
            return Ok(finish(
                "teleport_channel",
                "teleport(3 кубита)",
                3,
                8,
                chan.ops().len(),
                "exact:Z[1/√2,i]",
                Verdict::Refuted(1.0),
                1.0,
                checks,
                vec![],
                started,
            ));
        }
    }
    let mut notes = vec![
        "свойство: канал переносит q0 → q2 с сохранением |ψ⟩".to_string(),
        "доказано на базисе {|0⟩, |1⟩} — по линейности ∀α,β".to_string(),
        "когерентные коррекции (отложенное измерение) — классический канал не нужен".to_string(),
    ];
    notes.push(format!("гейтов в канале: {}", chan.ops().len()));
    Ok(finish(
        "teleport_channel",
        "teleport(3 кубита)",
        3,
        8,
        chan.ops().len(),
        "exact:Z[1/√2,i]",
        Verdict::ProvedExact,
        0.0,
        checks,
        notes,
        started,
    ))
}

#[allow(clippy::too_many_arguments)]
fn finish(
    property: &'static str,
    subject: &str,
    n_qubits: usize,
    dim: usize,
    gate_count: usize,
    method: &'static str,
    verdict: Verdict,
    max_deviation: f64,
    checks: Vec<(&'static str, bool, f64)>,
    notes: Vec<String>,
    started: std::time::Instant,
) -> VerificationReport {
    VerificationReport {
        property,
        subject: subject.to_string(),
        n_qubits,
        dim,
        gate_count,
        method,
        verdict,
        max_deviation,
        checks,
        notes: {
            let mut n = notes;
            n.push(format!("время верификации: {} мкс", started.elapsed().as_micros()));
            n
        },
    }
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithms;

    const TOL: f64 = 1e-10;

    #[test]
    fn unitary_bell_proved_exact() {
        let rep = verify_unitary(&algorithms::bell().unwrap()).unwrap();
        assert_eq!(rep.verdict, Verdict::ProvedExact, "{:?}", rep.notes);
        assert_eq!(rep.method, "exact:Z[1/√2,i]");
    }

    #[test]
    fn unitary_ghz_proved_exact() {
        let rep = verify_unitary(&algorithms::ghz(5).unwrap()).unwrap();
        assert_eq!(rep.verdict, Verdict::ProvedExact);
    }

    #[test]
    fn unitary_qft3_proved_exact_via_ring_cp() {
        // QFT-3: CP(π/2) и CP(π/4) — в кольце (цикл P: точный Cp).
        let rep = verify_unitary(&algorithms::qft(3, false).unwrap()).unwrap();
        assert_eq!(rep.verdict, Verdict::ProvedExact, "метод: {}", rep.method);
    }

    #[test]
    fn unitary_qft4_numeric_fallback() {
        // QFT-4 содержит CP(π/8) — вне ℤ[1/√2, i], честный f64.
        let rep = verify_unitary(&algorithms::qft(4, false).unwrap()).unwrap();
        assert_eq!(rep.method, "numeric:f64");
        match rep.verdict {
            Verdict::VerifiedNumeric(tol) => assert!(tol < 1e-8),
            other => panic!("ожидался VerifiedNumeric, получен {other:?}"),
        }
    }

    #[test]
    fn unitary_grover_proved_exact() {
        // Оракул и диффузор — фазовые перевороты: точно в кольце.
        let rep = verify_unitary(&algorithms::grover(4, &[3]).unwrap().0).unwrap();
        assert_eq!(rep.verdict, Verdict::ProvedExact);
    }

    #[test]
    fn equivalence_qft_iqft_identity_exact() {
        let mut c = algorithms::qft(3, false).unwrap();
        for op in algorithms::qft(3, true).unwrap().ops() {
            c.push(op.clone());
        }
        let rep = verify_equivalence(&c, &Circuit::new(3).unwrap(), false).unwrap();
        assert_eq!(rep.verdict, Verdict::ProvedExact, "{:?}", rep.checks);
    }

    #[test]
    fn equivalence_qft_iqft_numeric() {
        let mut c = algorithms::qft(5, false).unwrap();
        for op in algorithms::qft(5, true).unwrap().ops() {
            c.push(op.clone());
        }
        let rep = verify_equivalence(&c, &Circuit::new(5).unwrap(), false).unwrap();
        match rep.verdict {
            Verdict::VerifiedNumeric(tol) => {
                assert!(tol < 1e-8, "tol {tol}");
                assert!(rep.max_deviation < TOL, "dev {}", rep.max_deviation);
            }
            other => panic!("ожидался VerifiedNumeric, получен {other:?}"),
        }
    }

    #[test]
    fn equivalence_bell_manual_exact() {
        let mut manual = Circuit::new(2).unwrap();
        manual
            .gate(Gate::H { q: 0 })
            .gate(Gate::Cx {
                control: 0,
                target: 1,
            });
        let mut bell = algorithms::bell().unwrap();
        // bell() оканчивается MeasureAll — верификатор его отбросит.
        let rep = verify_equivalence(&bell, &manual, false).unwrap();
        assert_eq!(rep.verdict, Verdict::ProvedExact);
        bell.push(Op::Barrier);
        let _ = bell;
    }

    #[test]
    fn equivalence_t_fourth_power_is_z_exact() {
        // T·T·T·T = Z — глубокая проверка кольца (степени e^{iπ/4}).
        let mut tt = Circuit::new(1).unwrap();
        for _ in 0..4 {
            tt.gate(Gate::T { q: 0 });
        }
        let mut z = Circuit::new(1).unwrap();
        z.gate(Gate::Z { q: 0 });
        let rep = verify_equivalence(&tt, &z, false).unwrap();
        assert_eq!(rep.verdict, Verdict::ProvedExact);
    }

    #[test]
    fn equivalence_global_phase_minus_one() {
        // Z·X = −X·Z: эквивалентны с точностью до фазы λ = −1,
        // НЕ эквивалентны без неё.
        let mut a = Circuit::new(1).unwrap();
        a.gate(Gate::Z { q: 0 }).gate(Gate::X { q: 0 });
        let mut b = Circuit::new(1).unwrap();
        b.gate(Gate::X { q: 0 }).gate(Gate::Z { q: 0 });
        let with_phase = verify_equivalence(&a, &b, true).unwrap();
        assert_eq!(with_phase.verdict, Verdict::ProvedExact);
        let strict = verify_equivalence(&a, &b, false).unwrap();
        assert!(matches!(strict.verdict, Verdict::Refuted(_)));
    }

    #[test]
    fn equivalence_refuted_x_vs_z() {
        let mut x = Circuit::new(1).unwrap();
        x.gate(Gate::X { q: 0 });
        let mut z = Circuit::new(1).unwrap();
        z.gate(Gate::Z { q: 0 });
        let rep = verify_equivalence(&x, &z, false).unwrap();
        assert!(matches!(rep.verdict, Verdict::Refuted(d) if d > 0.5));
    }

    #[test]
    fn teleport_channel_proved_exact() {
        let rep = verify_teleport_channel().unwrap();
        assert_eq!(rep.verdict, Verdict::ProvedExact);
        assert_eq!(rep.gate_count, 6);
        assert_eq!(rep.checks.len(), 2);
        assert!(rep.checks.iter().all(|c| c.1));
    }

    #[test]
    fn lint_finds_unused_qubit() {
        let mut c = Circuit::new(3).unwrap();
        c.gate(Gate::H { q: 0 });
        let warns = lint(&c);
        assert!(warns.iter().any(|w| w.contains("кубит 1 не задействован")));
        assert!(warns.iter().any(|w| w.contains("кубит 2 не задействован")));
    }

    #[test]
    fn hard_lint_rejects_out_of_range() {
        let mut c = Circuit::new(2).unwrap();
        c.gate(Gate::H { q: 5 });
        assert!(verify_unitary(&c).is_err());
    }

    #[test]
    fn sampling_verdict_for_large_registers() {
        // n = 12 > MAX_NUMERIC_QUBITS: вероятностный вердикт, не крах.
        let rep = verify_unitary(&algorithms::ghz(12).unwrap()).unwrap();
        assert_eq!(rep.method, "sampling:f64");
        assert!(matches!(rep.verdict, Verdict::VerifiedSampling(_)));
    }

    #[test]
    fn prepcomb_is_rejected_as_non_unitary() {
        let rep = verify_unitary(&algorithms::period_finding(4, 3, 0).unwrap());
        assert!(rep.is_err());
    }

    #[test]
    fn report_verdict_line_is_human_readable() {
        let rep = verify_unitary(&algorithms::bell().unwrap()).unwrap();
        let line = rep.verdict_line();
        assert!(line.contains("ДОКАЗАНО ТОЧНО"), "{line}");
    }
}
