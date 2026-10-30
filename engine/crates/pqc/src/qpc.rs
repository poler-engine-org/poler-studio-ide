//! # qpc — POLER Quantum PC: идеальный кубитный субстрат
//!
//! Инструмент «квантовый компьютер, который работает точнее физических
//! кубитов»: statevector-машина с **нулевой** ошибкой гейтов, нулевой
//! декогеренцией и нулевой ошибкой считывания. Физическая машина платит
//! за каждый гейт (fidelity ~1e-3), за каждое считывание (~1e-2) и за
//! время когерентности (мкс); здесь единственный источник приближения —
//! округление f64 (2.2e-16), а режим [`crate::exact`] убирает и его:
//! амплитуды Clifford+T-схем живут в точном кольце ℤ[1/√2, i].
//!
//! ## QCASM-lite
//!
//! ```text
//! qubits 3
//! h 0
//! cx 0 1
//! t 2
//! ry 1 0.7853981633974483
//! measure all
//! ```
//!
//! Инструкции: `qubits N` (обязательна, первой), `h|x|y|z|s|sdg|t|tdg q`,
//! `cx|cz|swap a b`, `ccx c1 c2 t`, `cp c t θ`, `ry|rx|rz q θ`,
//! `flipphase MASK` (фаза −1 на состояниях, где все биты MASK равны 1),
//! `flipzero` (фаза −1 на всех состояниях, кроме |0…0⟩ — диффузор Гровера
//! без декомпозиции: идеальная машина применяет оракул прямо к вектору
//! состояния), `measure q`, `measure all`, `barrier`, комментарии `#`.
//!
//! Битовая конвенция — как в qiskit: кубит 0 — младший бит индекса.

use crate::born::BornSampler;
use crate::complex::Cx;
use crate::error::{PqcError, Result};
use crate::rng::Rng;
use crate::statevector::Statevector;

/// Одна инструкция схемы.
#[derive(Clone, Debug)]
pub enum Op {
    /// Унитарный гейт.
    Gate(crate::gates::Gate),
    /// Измерение кубита `q` (коллапс в per-shot режиме).
    Measure { q: usize },
    /// Измерение всех кубитов.
    MeasureAll,
    /// Фаза −1 на базисных состояниях, где все биты маски равны 1
    /// (точный классический оракул — привилегия владельца вектора состояния).
    FlipPhase { mask: usize },
    /// Фаза −1 ровно на одном базисном состоянии `idx`
    /// (точечный оракул Гровера/Дойча–Йожи).
    FlipIndex { idx: usize },
    /// Фаза −1 на всех состояниях, кроме |0…0⟩.
    FlipZero,
    /// Подготовка гребёнки: равная суперпозиция базисных состояний
    /// x ≡ offset (mod period). Привилегия владельца вектора состояния
    /// (как оракул Гровера): идеальная машина не платит за синтез
    /// модульной экспоненты — теоретико-числовой субстрат УДЕ (Том VII §2.3:
    /// «фазовые вращения a^x», поиск периода через автокорреляционный пик).
    PrepComb { period: usize, offset: usize },
    /// Барьер (семантический маркер, на динамику не влияет).
    Barrier,
}

/// Схема для квантового ПК.
#[derive(Clone, Debug, Default)]
pub struct Circuit {
    n_qubits: usize,
    ops: Vec<Op>,
}

impl Circuit {
    /// Пустая схема на `n` кубитов.
    pub fn new(n_qubits: usize) -> Result<Circuit> {
        if n_qubits == 0 {
            return Err(PqcError::EmptyState);
        }
        if n_qubits > crate::statevector::MAX_QUBITS {
            return Err(PqcError::TooManyQubits {
                requested: n_qubits,
                max: crate::statevector::MAX_QUBITS,
            });
        }
        Ok(Circuit {
            n_qubits,
            ops: Vec::new(),
        })
    }

    /// Число кубитов.
    pub fn n_qubits(&self) -> usize {
        self.n_qubits
    }

    /// Операции схемы.
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    /// Добавить операцию (владелец отвечает за диапазон кубитов).
    pub fn push(&mut self, op: Op) -> &mut Self {
        self.ops.push(op);
        self
    }

    /// Добавить гейт.
    pub fn gate(&mut self, g: crate::gates::Gate) -> &mut Self {
        self.ops.push(Op::Gate(g));
        self
    }

    /// Есть ли в схеме измерение до последнего гейта (per-shot коллапс).
    pub fn has_mid_circuit_measure(&self) -> bool {
        let last_gate = self.ops.iter().rposition(|op| {
            matches!(
                op,
                Op::Gate(_)
                    | Op::FlipPhase { .. }
                    | Op::FlipIndex { .. }
                    | Op::FlipZero
                    | Op::PrepComb { .. }
            )
        });
        let first_measure = self
            .ops
            .iter()
            .position(|op| matches!(op, Op::Measure { .. } | Op::MeasureAll));
        match (last_gate, first_measure) {
            (Some(lg), Some(fm)) => fm < lg,
            _ => false,
        }
    }

    /// QCASM-lite парсер. Ошибки — с номером строки.
    pub fn parse(text: &str) -> Result<Circuit> {
        let mut n_qubits: Option<usize> = None;
        let mut ops: Vec<Op> = Vec::new();
        for (idx, raw) in text.lines().enumerate() {
            let line_no = idx + 1;
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let toks: Vec<&str> = line.split_whitespace().collect();
            let bad = |what: String| PqcError::QcParse {
                line: line_no,
                what,
            };
            let usize_arg = |spec: usize, min: usize| -> Result<usize> {
                toks.get(spec)
                    .and_then(|t| t.parse::<usize>().ok())
                    .filter(|v| *v >= min)
                    .ok_or_else(|| bad(format!("expected {spec} index, got `{}`", line)))
            };
            let f64_arg = |spec: usize| -> Result<f64> {
                toks.get(spec)
                    .and_then(|t| t.parse::<f64>().ok())
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| bad(format!("expected finite angle, got `{}`", line)))
            };
            let check_q = |q: usize| -> Result<()> {
                let n = n_qubits.ok_or_else(|| bad("`qubits N` must come first".into()))?;
                if q >= n {
                    return Err(bad(format!("qubit {q} out of range [0, {n})")));
                }
                Ok(())
            };
            match toks[0] {
                "qubits" | "qubit" => {
                    if n_qubits.is_some() {
                        return Err(bad("duplicate `qubits` directive".into()));
                    }
                    let n = usize_arg(1, 1).map_err(|_| bad(format!("expected `qubits N`, got `{}`", line)))?;
                    if n > crate::statevector::MAX_QUBITS {
                        return Err(bad(format!(
                            "{n} qubits exceed engine max {}",
                            crate::statevector::MAX_QUBITS
                        )));
                    }
                    n_qubits = Some(n);
                }
                "h" | "x" | "y" | "z" | "s" | "sdg" | "t" | "tdg" => {
                    let q = usize_arg(1, 0)?;
                    check_q(q)?;
                    let g = match toks[0] {
                        "h" => crate::gates::Gate::H { q },
                        "x" => crate::gates::Gate::X { q },
                        "y" => crate::gates::Gate::Y { q },
                        "z" => crate::gates::Gate::Z { q },
                        "s" => crate::gates::Gate::S { q },
                        "sdg" => crate::gates::Gate::Sdg { q },
                        "t" => crate::gates::Gate::T { q },
                        _ => crate::gates::Gate::Tdg { q },
                    };
                    ops.push(Op::Gate(g));
                }
                "ry" | "rx" | "rz" => {
                    let q = usize_arg(1, 0)?;
                    check_q(q)?;
                    let theta = f64_arg(2)?;
                    let g = match toks[0] {
                        "ry" => crate::gates::Gate::Ry { q, theta },
                        "rx" => crate::gates::Gate::Rx { q, theta },
                        _ => crate::gates::Gate::Rz { q, theta },
                    };
                    ops.push(Op::Gate(g));
                }
                "cx" | "cz" | "swap" => {
                    let a = usize_arg(1, 0)?;
                    let b = usize_arg(2, 0)?;
                    check_q(a)?;
                    check_q(b)?;
                    if a == b {
                        return Err(bad(format!("{} needs two distinct qubits", toks[0])));
                    }
                    let g = match toks[0] {
                        "cx" => crate::gates::Gate::Cx {
                            control: a,
                            target: b,
                        },
                        "cz" => crate::gates::Gate::Cz {
                            control: a,
                            target: b,
                        },
                        _ => crate::gates::Gate::Swap { a, b },
                    };
                    ops.push(Op::Gate(g));
                }
                "ccx" => {
                    let c1 = usize_arg(1, 0)?;
                    let c2 = usize_arg(2, 0)?;
                    let t = usize_arg(3, 0)?;
                    check_q(c1)?;
                    check_q(c2)?;
                    check_q(t)?;
                    if c1 == c2 || c1 == t || c2 == t {
                        return Err(bad("ccx needs three pairwise distinct qubits".into()));
                    }
                    ops.push(Op::Gate(crate::gates::Gate::Ccx {
                        c1,
                        c2,
                        target: t,
                    }));
                }
                "cp" => {
                    let c = usize_arg(1, 0)?;
                    let t = usize_arg(2, 0)?;
                    check_q(c)?;
                    check_q(t)?;
                    if c == t {
                        return Err(bad("cp needs two distinct qubits".into()));
                    }
                    let theta = f64_arg(3)?;
                    ops.push(Op::Gate(crate::gates::Gate::Cp {
                        control: c,
                        target: t,
                        theta,
                    }));
                }
                "flipphase" => {
                    if n_qubits.is_none() {
                        return Err(bad("`qubits N` must come first".into()));
                    }
                    let mask = usize_arg(1, 0)?;
                    if mask >= 1usize << n_qubits.unwrap() {
                        return Err(bad(format!("mask {mask} exceeds {n_qubits:?} qubits")));
                    }
                    ops.push(Op::FlipPhase { mask });
                }
                "flipstate" => {
                    if n_qubits.is_none() {
                        return Err(bad("`qubits N` must come first".into()));
                    }
                    let idx = usize_arg(1, 0)?;
                    if idx >= 1usize << n_qubits.unwrap() {
                        return Err(bad(format!("state {idx} exceeds {n_qubits:?} qubits")));
                    }
                    ops.push(Op::FlipIndex { idx });
                }
                "flipzero" => {
                    if n_qubits.is_none() {
                        return Err(bad("`qubits N` must come first".into()));
                    }
                    ops.push(Op::FlipZero);
                }
                "prep" => {
                    let n = n_qubits
                        .ok_or_else(|| bad("`qubits N` must come first".into()))?;
                    let period = usize_arg(1, 1)?;
                    let offset = usize_arg(2, 0)?;
                    if offset >= period {
                        return Err(bad(format!(
                            "prep: offset {offset} must be < period {period}"
                        )));
                    }
                    if period >= 1usize << n {
                        return Err(bad(format!(
                            "prep: period {period} must be < 2^{n} (dim)"
                        )));
                    }
                    ops.push(Op::PrepComb { period, offset });
                }
                "measure" => {
                    let which = *toks
                        .get(1)
                        .ok_or_else(|| bad("measure needs `q` or `all`".into()))?;
                    if which == "all" {
                        ops.push(Op::MeasureAll);
                    } else {
                        let q = which
                            .parse::<usize>()
                            .map_err(|_| bad(format!("bad measure target `{which}`")))?;
                        check_q(q)?;
                        ops.push(Op::Measure { q });
                    }
                }
                "barrier" => ops.push(Op::Barrier),
                other => {
                    return Err(bad(format!("unknown instruction `{other}`")));
                }
            }
        }
        let n = n_qubits.ok_or_else(|| PqcError::QcParse {
            line: 0,
            what: "missing `qubits N` directive".into(),
        })?;
        Ok(Circuit { n_qubits: n, ops })
    }

    /// Применить неразрушающие операции к состоянию (гейты и фазовые
    /// оракулы). `Measure` игнорируются — измерениями управляет раннер.
    pub(crate) fn apply_unitary(sv: &mut Statevector, ops: &[Op]) -> Result<()> {
        for op in ops {
            match op {
                Op::Gate(g) => sv.apply(*g)?,
                Op::FlipPhase { mask } => {
                    for (i, a) in sv.amplitudes_mut().iter_mut().enumerate() {
                        if *mask != 0 && i & *mask == *mask {
                            *a = -*a;
                        }
                    }
                }
                Op::FlipIndex { idx } => {
                    if *idx < sv.amplitudes().len() {
                        sv.amplitudes_mut()[*idx] = -sv.amplitudes()[*idx];
                    }
                }
                Op::FlipZero => {
                    for (i, a) in sv.amplitudes_mut().iter_mut().enumerate() {
                        if i != 0 {
                            *a = -*a;
                        }
                    }
                }
                Op::PrepComb { period: period_, offset: offset_ } => {
                    let (period, offset) = (*period_, *offset_);
                    // Гребёнка: x ≡ offset (mod period), амплитуда 1/√m.
                    // Заменяет текущее состояние (привилегия state-owner).
                    let mut m = 0usize;
                    for x in 0..sv.amplitudes().len() {
                        if x % period == offset {
                            m += 1;
                        }
                    }
                    if m == 0 {
                        return Err(PqcError::BadArgument {
                            what: format!("prep: no indices ≡ {offset} (mod {period})"),
                        });
                    }
                    let a = Cx {
                        re: 1.0 / (m as f64).sqrt(),
                        im: 0.0,
                    };
                    for (x, amp) in sv.amplitudes_mut().iter_mut().enumerate() {
                        *amp = if x % period == offset { a } else { Cx::ZERO };
                    }
                }
                Op::Measure { .. } | Op::MeasureAll | Op::Barrier => {}
            }
        }
        Ok(())
    }

    /// Коллапс кубита `q` по исходу `bit` (перенормировка включена).
    pub(crate) fn collapse(sv: &mut Statevector, q: usize, bit: usize) -> Result<()> {
        let mask = 1usize << q;
        let mut acc = 0.0f64;
        for (i, a) in sv.amplitudes_mut().iter_mut().enumerate() {
            if (i & mask != 0) != (bit != 0) {
                *a = Cx::ZERO;
            } else {
                acc += a.norm_sq();
            }
        }
        if acc <= 0.0 || !acc.is_finite() {
            return Err(PqcError::NotNormalized { norm: 0.0 });
        }
        let k = 1.0 / acc.sqrt();
        for a in sv.amplitudes_mut() {
            *a = a.scale(k);
        }
        Ok(())
    }
}

/// Результат прогона схемы на квантовом ПК.
#[derive(Clone, Debug)]
pub struct QpcReport {
    /// Число кубитов.
    pub n_qubits: usize,
    /// Число гейтов (без measure/barrier).
    pub gate_count: usize,
    /// Финальное состояние (unitarная часть; в per-shot режиме —
    /// состояние последнего выстрела после коллапсов).
    pub final_state: Statevector,
    /// Распределение Борна |⟨x|ψ⟩|² финального состояния.
    pub probabilities: Vec<f64>,
    /// Гистограмма выстрелов (outcome, count), по убыванию частоты.
    pub counts: Vec<(u64, u64)>,
    /// Число выстрелов.
    pub shots: u64,
    /// Режим исполнения: true = per-shot с коллапсами.
    pub per_shot: bool,
    /// Маргиналы P(b_q = 1) финального состояния.
    pub marginals: Vec<f64>,
    /// Энтропия Борна распределения исходов, биты.
    pub entropy_bits: f64,
    /// Ландауэров пол: k_B·T·ln2 · H бит при T = 300 K, джоули.
    pub landauer_j: f64,
    /// ‖ψ‖ финального состояния (контроль унитарности).
    pub norm: f64,
}

/// Постоянная Больцмана, Дж/К (CODATA 2018).
pub const K_B: f64 = 1.380649e-23;
/// Эталонная температура ландауэровского пола, К.
pub const LANDAUER_T: f64 = 300.0;

/// Энтропия Шеннона распределения в битах.
pub fn shannon_bits(probs: &[f64]) -> f64 {
    let mut h = 0.0;
    for &p in probs {
        if p > 0.0 {
            h -= p * p.log2();
        }
    }
    h
}

/// Прогнать схему: один унитарный проход + N выстрелов Борна,
/// либо per-shot с коллапсами, если измерения стоят до гейтов.
pub fn run(circuit: &Circuit, shots: u64, seed: u64) -> Result<QpcReport> {
    let n = circuit.n_qubits();
    let mut sv = Statevector::new(n)?;
    let gate_count = circuit
        .ops
        .iter()
        .filter(|op| {
            matches!(
                op,
                Op::Gate(_)
                    | Op::FlipPhase { .. }
                    | Op::FlipIndex { .. }
                    | Op::FlipZero
                    | Op::PrepComb { .. }
            )
        })
        .count();

    let per_shot = circuit.has_mid_circuit_measure();
    let mut rng = Rng::seed_from_u64(seed);
    let counts;

    if !per_shot {
        // Быстрый путь: вся схема унитарна, измерения — в конце.
        Circuit::apply_unitary(&mut sv, circuit.ops())?;
        let sampler = BornSampler::new(&sv)?;
        counts = if shots > 0 {
            sampler.sample_counts(&mut rng, shots)
        } else {
            Vec::new()
        };
    } else {
        // Per-shot: полный прогон на каждый выстрел, коллапсы честные.
        if shots == 0 {
            return Err(PqcError::BadArgument {
                what: "mid-circuit measurement requires --shots >= 1".into(),
            });
        }
        let mut histogram = std::collections::BTreeMap::new();
        for _ in 0..shots {
            let mut trial = Statevector::new(n)?;
            for op in circuit.ops() {
                match op {
                    Op::Gate(_)
                    | Op::FlipPhase { .. }
                    | Op::FlipIndex { .. }
                    | Op::FlipZero
                    | Op::PrepComb { .. } => {
                        Circuit::apply_unitary(&mut trial, std::slice::from_ref(op))?;
                    }
                    Op::Measure { q } => {
                        let p1: f64 = trial
                            .amplitudes()
                            .iter()
                            .enumerate()
                            .map(|(i, a)| if i & (1usize << q) != 0 { a.norm_sq() } else { 0.0 })
                            .sum();
                        let bit = if rng.next_f64() < p1 { 1usize } else { 0 };
                        Circuit::collapse(&mut trial, *q, bit)?;
                    }
                    Op::MeasureAll => {
                        for q in 0..n {
                            let p1: f64 = trial
                                .amplitudes()
                                .iter()
                                .enumerate()
                                .map(|(i, a)| {
                                    if i & (1usize << q) != 0 {
                                        a.norm_sq()
                                    } else {
                                        0.0
                                    }
                                })
                                .sum();
                            let bit = if rng.next_f64() < p1 { 1usize } else { 0 };
                            Circuit::collapse(&mut trial, q, bit)?;
                        }
                    }
                    Op::Barrier => {}
                }
            }
            // Исход выстрела: честный Борн-семпл остаточного состояния
            // (после коллапсов распределение сконцентрировано на одном исходе).
            let sampler = BornSampler::new(&trial)?;
            let outcome = sampler.sample(&mut rng);
            *histogram.entry(outcome).or_insert(0u64) += 1;
            sv = trial;
        }
        counts = histogram.into_iter().collect();
        sv.normalize()?;
    }

    let probabilities = sv.probabilities();
    let marginals = sv.marginals();
    let entropy_bits = shannon_bits(&probabilities);
    let norm = sv.norm();
    Ok(QpcReport {
        n_qubits: n,
        gate_count,
        final_state: sv,
        probabilities,
        counts,
        shots,
        per_shot,
        marginals,
        entropy_bits,
        landauer_j: entropy_bits * K_B * LANDAUER_T * core::f64::consts::LN_2,
        norm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bell_and_run() {
        let c = Circuit::parse("qubits 2\nh 0\ncx 0 1\nmeasure all\n").unwrap();
        assert_eq!(c.n_qubits(), 2);
        let rep = run(&c, 1000, 42).unwrap();
        // Белл: только 00 и 11 с равными вероятностями.
        let p00 = rep.probabilities[0];
        let p11 = rep.probabilities[3];
        assert!((p00 - 0.5).abs() < 1e-12 && (p11 - 0.5).abs() < 1e-12);
        assert!(rep.probabilities[1] < 1e-12 && rep.probabilities[2] < 1e-12);
        let c00 = rep.counts.iter().find(|(o, _)| *o == 0).map(|(_, c)| *c).unwrap_or(0);
        let c11 = rep.counts.iter().find(|(o, _)| *o == 3).map(|(_, c)| *c).unwrap_or(0);
        assert_eq!(c00 + c11, 1000);
        assert!(c00 > 400 && c11 > 400, "fair coin: {c00}/{c11}");
    }

    #[test]
    fn parse_errors_carry_lines() {
        assert!(matches!(
            Circuit::parse("qubits 2\nh 5\n"),
            Err(PqcError::QcParse { line: 2, .. })
        ));
        // Нет директивы qubits вовсе — line 0 (пост-сканум).
        assert!(matches!(
            Circuit::parse("# только комментарий\n\n"),
            Err(PqcError::QcParse { line: 0, .. })
        ));
        // Гейт до qubits — тоже QcParse, с номером строки гейта.
        assert!(matches!(
            Circuit::parse("h 0\n"),
            Err(PqcError::QcParse { line: 1, .. })
        ));
        assert!(Circuit::parse("qubits 1\nfoo 0\n").is_err());
    }

    #[test]
    fn mid_circuit_measure_collapses() {
        // Измерили q0 в |+⟩, затем гейт после измерения: per-shot путь.
        let c = Circuit::parse("qubits 2\nh 0\nmeasure 0\ncx 0 1\nmeasure all\n").unwrap();
        let rep = run(&c, 200, 7).unwrap();
        assert!(rep.per_shot);
        // После коллапса q0 и CX: исходы 00/11 детерминированы в каждом выстреле.
        for (o, cnt) in &rep.counts {
            assert!(*o == 0 || *o == 3, "outcome {o} impossible");
            assert!(*cnt > 0);
        }
    }

    #[test]
    fn flip_phase_oracle() {
        // Суперпозиция → flipphase 1 (бит 0) → амплитуды с битом 0 инвертируются,
        // вероятности не меняются: фазовый оракул невидим в |⟨x|ψ⟩|².
        let c = Circuit::parse("qubits 2\nh 0\nh 1\nflipphase 1\n").unwrap();
        let rep = run(&c, 0, 1).unwrap();
        for p in &rep.probabilities {
            assert!((p - 0.25).abs() < 1e-12);
        }
        let a = rep.final_state.amplitudes();
        assert!((a[0].re - 0.5).abs() < 1e-12, "|00> untouched");
        assert!((a[1].re + 0.5).abs() < 1e-12, "|01> flipped");
        assert!((a[2].re - 0.5).abs() < 1e-12, "|10> untouched");
        assert!((a[3].re + 0.5).abs() < 1e-12, "|11> flipped");
    }

    #[test]
    fn landauer_floor_is_h_kb_t_ln2() {
        let h = shannon_bits(&[0.5, 0.5]);
        assert!((h - 1.0).abs() < 1e-12);
        let one_bit = K_B * LANDAUER_T * core::f64::consts::LN_2;
        assert!((one_bit - 2.87e-21).abs() < 0.02e-21);
    }

    #[test]
    fn prep_comb_builds_equal_superposition() {
        // Гребёнка с периодом 3, сдвиг 1, 8 состояний: зубья {1,4,7}.
        let c = Circuit::parse("qubits 3\nprep 3 1\nmeasure all\n").unwrap();
        let rep = run(&c, 0, 1).unwrap();
        for (x, p) in rep.probabilities.iter().enumerate() {
            let want = if x % 3 == 1 { 1.0 / 3.0 } else { 0.0 };
            assert!((p - want).abs() < 1e-12, "x={x}: {p} vs {want}");
        }
        assert!((rep.norm - 1.0).abs() < 1e-12);
    }

    #[test]
    fn prep_comb_parse_rejects_bad_offsets() {
        assert!(Circuit::parse("qubits 3\nprep 3 3\n").is_err(), "offset ≥ period");
        assert!(Circuit::parse("qubits 3\nprep 0 0\n").is_err(), "period 0");
        assert!(Circuit::parse("qubits 3\nprep 9 1\n").is_err(), "period ≥ dim");
        assert!(Circuit::parse("prep 3 1\n").is_err(), "no qubits directive");
        // period=1 — легальная равномерная суперпозиция (аналог H^n).
        let c = Circuit::parse("qubits 2\nprep 1 0\n").unwrap();
        let rep = run(&c, 0, 1).unwrap();
        for p in &rep.probabilities {
            assert!((p - 0.25).abs() < 1e-12);
        }
    }

    #[test]
    fn prep_comb_is_periodic_under_shift() {
        // Автокорреляционный критерий (Thm F.9/Q_Λ) для КОНЕЧНОЙ гребёнки:
        // лаг = периоду даёт строгий максимум автокорреляции (зубья на краю
        // заворачиваются, поэтому corr(r) < Σp², но пик — единственный лидер;
        // точное равенство corr = Σp² — свойство бесконечной гребёнки).
        let c = Circuit::parse("qubits 4\nprep 5 2\n").unwrap();
        let rep = run(&c, 0, 1).unwrap();
        let n = 16usize;
        let corr = |lag: usize| -> f64 {
            (0..n)
                .map(|x| rep.probabilities[x] * rep.probabilities[(x + lag) % n])
                .sum()
        };
        let peak = corr(5);
        assert!(peak > 0.0);
        // Круговая автокорреляция симметрична: corr(d) = corr(N−d),
        // поэтому пик — на лагах ±5 (mod 16): и 5, и 11.
        assert!((corr(11) - peak).abs() < 1e-14, "mirror peak at N−r");
        for lag in 1..n {
            if lag == 5 || lag == 11 {
                continue;
            }
            // Кратные периода дают убывающие субпики (2/9, 1/9), чужие — 0.
            assert!(
                corr(lag) < peak - 1e-12,
                "lag {lag}: {} must be below peak {peak}",
                corr(lag)
            );
        }
    }
}
