//! # Алгоритмы для квантового ПК POLER
//!
//! Эталоны, на которых физические машины теряют точность, а идеальный
//! субстрат — нет: QFT (фазы π/2^k), Grover (оракул применён к вектору
//! состояния точно, без декомпозиции в T-гейты), Bernstein–Vazirani,
//! Deutsch–Jozsa, GHZ. Все схемы — [`Circuit`] с QCASM-семантикой.

use crate::error::{PqcError, Result};
use crate::gates::Gate;
use crate::qpc::{Circuit, Op};

/// Пара Белла (|00⟩ + |11⟩)/√2.
pub fn bell() -> Result<Circuit> {
    let mut c = Circuit::new(2)?;
    c.gate(Gate::H { q: 0 })
        .gate(Gate::Cx {
            control: 0,
            target: 1,
        })
        .push(Op::MeasureAll);
    Ok(c)
}

/// GHZ-состояние (|0…0⟩ + |1…1⟩)/√2 на n кубитах.
pub fn ghz(n: usize) -> Result<Circuit> {
    if n == 0 {
        return Err(PqcError::EmptyState);
    }
    let mut c = Circuit::new(n)?;
    c.gate(Gate::H { q: 0 });
    for q in 1..n {
        c.gate(Gate::Cx {
            control: q - 1,
            target: q,
        });
    }
    c.push(Op::MeasureAll);
    Ok(c)
}

/// Квантовое преобразование Фурье на n кубитах.
///
/// Прямая схема (Nielsen & Chuang / qiskit): для j от старшего к младшему —
/// H(j), затем CP(k, j, π/2^{j−k}) для k < j по убыванию. Финальные
/// SWAPs бит-реверса опущены (регистр живёт в переставленной конвенции —
/// стандартная практика; паритет с qiskit верифицируется на той же
/// раскладке гейтов). Обратное QFT — точное дзеркало: обратный порядок
/// операций с инвертированными фазами (H и перестановки самодвойственны).
pub fn qft(n: usize, inverse: bool) -> Result<Circuit> {
    if n == 0 {
        return Err(PqcError::EmptyState);
    }
    let mut ops: Vec<Op> = Vec::new();
    for j in (0..n).rev() {
        ops.push(Op::Gate(Gate::H { q: j }));
        for k in (0..j).rev() {
            let angle = core::f64::consts::PI / (1u64 << (j - k)) as f64;
            ops.push(Op::Gate(Gate::Cp {
                control: k,
                target: j,
                theta: angle,
            }));
        }
    }
    if inverse {
        ops = ops
            .into_iter()
            .rev()
            .map(|op| match op {
                Op::Gate(Gate::Cp {
                    control,
                    target,
                    theta,
                }) => Op::Gate(Gate::Cp {
                    control,
                    target,
                    theta: -theta,
                }),
                other => other,
            })
            .collect();
    }
    let mut c = Circuit::new(n)?;
    for op in ops {
        c.push(op);
    }
    Ok(c)
}

/// Гровер: поиск помеченных состояний. Оракул — точная инверсия фазы
/// (`flipphase` по маске каждого помеченного состояния): идеальная машина
/// не платит за синтез оракула из T-гейтов — привилегия владельца
/// вектора состояния. Диффузор — H^n · (I − 2|0…0⟩⟨0…0|) · H^n
/// (глобальный знак на итерацию отброшен как ненаблюдаемый).
///
/// Возвращает схему и число итераций R = ⌊π/4·√(2^n/M)⌋.
pub fn grover(n: usize, marks: &[usize]) -> Result<(Circuit, usize)> {
    if n == 0 || marks.is_empty() {
        return Err(PqcError::BadArgument {
            what: "grover needs n >= 1 and at least one marked state".into(),
        });
    }
    let dim = 1usize << n;
    for &m in marks {
        if m >= dim {
            return Err(PqcError::BadArgument {
                what: format!("marked state {m} exceeds {n} qubits"),
            });
        }
    }
    let m = marks.len() as f64;
    let r = ((core::f64::consts::PI / 4.0) * (dim as f64 / m).sqrt()).floor() as usize;
    let r = r.max(1);

    let mut c = Circuit::new(n)?;
    for q in 0..n {
        c.gate(Gate::H { q });
    }
    for _ in 0..r {
        // Оракул: точная инверсия фазы помеченных состояний.
        for &mark in marks {
            c.push(Op::FlipIndex { idx: mark });
        }
        // Диффузор: H^n (I − 2|0⟩⟨0|) H^n.
        for q in 0..n {
            c.gate(Gate::H { q });
        }
        c.push(Op::FlipZero);
        for q in 0..n {
            c.gate(Gate::H { q });
        }
    }
    c.push(Op::MeasureAll);
    Ok((c, r))
}

/// Bernstein–Vazirani: восстановление секретного вектора s через
/// f(x) = s·x mod 2. n кубитов данных + 1 анцилла (последний кубит).
pub fn bernstein_vazirani(n: usize, secret: u64) -> Result<Circuit> {
    if n == 0 || n >= 64 {
        return Err(PqcError::BadArgument {
            what: "bernstein-vazirani needs 1..=63 data qubits".into(),
        });
    }
    if secret >> n != 0 {
        return Err(PqcError::BadArgument {
            what: format!("secret {secret} exceeds {n} bits"),
        });
    }
    let total = n + 1;
    let anc = n; // последний кубит
    let mut c = Circuit::new(total)?;
    c.gate(Gate::X { q: anc });
    for q in 0..total {
        c.gate(Gate::H { q });
    }
    // Оракул U_f: |x⟩|y⟩ → |x⟩|y ⊕ s·x⟩ — CNOT от каждого бита секрета.
    for k in 0..n {
        if secret >> k & 1 == 1 {
            c.gate(Gate::Cx {
                control: k,
                target: anc,
            });
        }
    }
    for q in 0..n {
        c.gate(Gate::H { q });
    }
    // Измеряем только биты данных.
    for q in 0..n {
        c.push(Op::Measure { q });
    }
    Ok(c)
}

/// Deutsch–Jozsa: константность/сбалансированность f: {0,1}^n → {0,1}
/// за один вызов оракула. Оракул задаётся масками помеченных состояний
/// (f(x) = 1 ⟺ x ∈ marks): фазовая инверсия — точно.
pub fn deutsch_jozsa(n: usize, marks: &[usize]) -> Result<Circuit> {
    if n == 0 {
        return Err(PqcError::EmptyState);
    }
    let dim = 1usize << n;
    for &m in marks {
        if m >= dim {
            return Err(PqcError::BadArgument {
                what: format!("marked state {m} exceeds {n} qubits"),
            });
        }
    }
    let mut c = Circuit::new(n)?;
    for q in 0..n {
        c.gate(Gate::H { q });
    }
    // Оракул: фазовая инверсия помеченных состояний (f = индикатор marks).
    for &m in marks {
        c.push(Op::FlipIndex { idx: m });
    }
    for q in 0..n {
        c.gate(Gate::H { q });
    }
    c.push(Op::MeasureAll);
    Ok(c)
}

/// Число зубьев гребёнки x ≡ offset (mod period) на регистре из N = 2^n
/// состояний.
pub fn comb_teeth(n: usize, period: usize, offset: usize) -> usize {
    let dim = 1usize << n;
    // количество x ∈ [0, dim): x ≡ offset (mod period)
    if offset >= dim {
        0
    } else {
        (dim - 1 - offset) / period + 1
    }
}

/// Аналитика QFT-гребёнки (ядро поиска периода, Том VII §2.3):
///
/// |QFT|ψ⟩|²(k) = |Σ_{j=0}^{m−1} e^{2πi·k·r·j/N}|² / (m·N)
///             = sin²(π·k·r·m/N) / (m·N·sin²(π·k·r/N)),
///
/// где m — число зубьев (сдвиг offset даёт только фазовый множитель
/// e^{2πik·offset/N} и в вероятности не виден). Пики: k ≈ кратные N/r.
pub fn comb_qft_prob(n: usize, period: usize, offset: usize, k: usize) -> f64 {
    let dim = 1usize << n;
    let m = comb_teeth(n, period, offset);
    if m == 0 || k >= dim {
        return 0.0;
    }
    let theta = core::f64::consts::PI * 2.0 * (k * period % dim) as f64 / dim as f64;
    let num = (m as f64 * theta / 2.0).sin();
    let den = (theta / 2.0).sin();
    if den.abs() < 1e-15 {
        // θ → 0 (mod π): все зубья в фазе, |Σ|² = m².
        m as f64 / dim as f64
    } else {
        (num * num) / (m as f64 * dim as f64 * den * den)
    }
}

/// Подходящие дроби (конвергенты) k/N в порядке роста знаменателя.
///
/// Теорема (Шор): если |k/N − l/r| < 1/(2r²), то l/r — конвергент k/N.
pub fn convergents(mut num: usize, mut den: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let (mut p_prev, mut q_prev) = (0usize, 1usize); // «−1-й» конвергент 0/1
    let (mut p_curr, mut q_curr) = (1usize, 0usize); // «0-й» конвергент 1/0
    while den > 0 {
        let a = num / den;
        let (p_next, q_next) = (a * p_curr + p_prev, a * q_curr + q_prev);
        p_prev = p_curr;
        q_prev = q_curr;
        p_curr = p_next;
        q_curr = q_next;
        let rem = num % den;
        num = den;
        den = rem;
        if q_curr > 0 {
            out.push((p_curr, q_curr));
        }
        if q_curr > (1usize << 30) {
            break; // защита от разрастания
        }
    }
    out.retain(|(_, q)| *q > 0);
    out
}

/// Кандидаты на период из измеренного пика k ≠ 0: знаменатели конвергентов
/// k/N (кроме тривиального 1), ограниченные сверху размером регистра.
/// ⚠ Среди них бывают ложные (5/29 для 11/64 — лучшее диофантово
/// приближение, но не период): окончательный выбор — [`recover_period`].
pub fn period_candidates(k: usize, n: usize) -> Vec<usize> {
    let dim = 1usize << n;
    if k == 0 || k >= dim {
        return Vec::new();
    }
    convergents(k, dim)
        .into_iter()
        .map(|(_, q)| q)
        .filter(|&q| q > 1 && q < dim)
        .collect()
}

/// Восстановление периода по наблюдённым пикам (не включайте k = 0).
/// Правило: r = min{ q ≥ 2 : каждый пик k даёт k·q/N близко к целому }.
/// Пик k ≈ l·N/r измеряется с квантованием ±1/2 и шириной главного
/// лепестка 1/(2m), m ≈ N/q ⇒ допуск |frac(k·q/N)| ≤ q/N + ε.
/// Множители r проходят всегда (период любой кратности согласован) —
/// поэтому минимум; делители/чужие q отбрасываются.
/// Честная граница: правило доказуемо точно при 1.5·r² < N (квантование
/// не может притянуть чужой q к целому); за пределами — эвристика Шора
/// (стандартный совет n ≥ 2·log₂ r² остаётся в силе).
pub fn recover_period(peaks: &[usize], n: usize) -> Option<usize> {
    let dim = 1usize << n;
    if peaks.is_empty() {
        return None;
    }
    for q in 2..dim {
        let tol = q as f64 / dim as f64 + 1e-12;
        let ok = peaks.iter().all(|&k| {
            let d = (k * q) % dim;
            let dist = d.min(dim - d) as f64 / dim as f64;
            dist <= tol
        });
        if ok {
            return Some(q);
        }
    }
    None
}

/// НОД (Евклид).
pub fn gcd(a: usize, b: usize) -> usize {
    let (mut a, mut b) = (a, b);
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// НОК (наименьшее общее кратное).
pub fn lcm(a: usize, b: usize) -> usize {
    if a == 0 || b == 0 {
        return 0;
    }
    a / gcd(a, b) * b
}

/// Поиск периода (ядро Шора, теоретико-числовой субстрат УДЕ):
///
/// 1. Гребёнка |ψ⟩ = (1/√m) Σ_j |offset + j·r⟩ — состояние после оракула
///    модульного возведения в степень (оракул — привилегия владельца
///    вектора состояния, как у Гровера).
/// 2. QFT с явным бит-реверсом (истинная конвенция |x⟩ → Σ ω^{xk}|k⟩/√N).
/// 3. MeasureAll: пики на k ≈ l·N/r; период восстанавливается цепной
///    дробью k/N (кандидаты из [`period_candidates`], затем lcm по
///    нескольким выстрелам; сертификация a^r ≡ 1 (mod N) — вне машины,
///    инструментами SMT, см. tools/verifiers/verify_number_theory_smt.py).
pub fn period_finding(n: usize, period: usize, offset: usize) -> Result<Circuit> {
    if n == 0 {
        return Err(PqcError::EmptyState);
    }
    let dim = 1usize << n;
    if period == 0 || period >= dim {
        return Err(PqcError::BadArgument {
            what: format!("period must be in [1, {dim})"),
        });
    }
    if offset >= period {
        return Err(PqcError::BadArgument {
            what: format!("offset {offset} must be < period {period}"),
        });
    }
    let mut c = Circuit::new(n)?;
    c.push(Op::PrepComb { period, offset });
    // QFT без бит-реверса…
    let q = qft(n, false)?;
    for op in q.ops() {
        c.push(op.clone());
    }
    // …плюс явный бит-реверс: SWAP(i, n−1−i).
    for i in 0..n / 2 {
        c.gate(Gate::Swap { a: i, b: n - 1 - i });
    }
    c.push(Op::MeasureAll);
    Ok(c)
}

// ---------------------------------------------------------------------------
// Цикл P: квантовая телепортация и реестр алгоритмов (CLI / shell / MCP)
// ---------------------------------------------------------------------------

/// Когерентная квантовая телепортация (Нильсен–Чанг §1.3.7 + принцип
/// отложенного измерения §4.4).
///
/// Три кубита: q0 — состояние |ψ⟩ = α|0⟩ + β|1⟩, пара Белла q1–q2.
/// Классический канал и коррекции по битам измерения заменены когерентными
/// CNOT/CZ (измерение коммутирует с операциями на чужих кубитах), поэтому
/// телепортация детерминирована: финальное состояние — |+⟩₀⊗|+⟩₁⊗|ψ⟩₂,
/// кубит q2 несёт |ψ⟩ точно.
///
/// Все вентили — Клиффорд (H, CNOT, CZ): канал допускает машинное
/// доказательство в кольце ℤ[1/√2, i] — см. [`crate::verify`].
pub fn teleport() -> Result<Circuit> {
    let mut c = Circuit::new(3)?;
    // Пара Белла на (q1, q2).
    c.gate(Gate::H { q: 1 })
        .gate(Gate::Cx {
            control: 1,
            target: 2,
        })
        // Разрушающее взаимодействие: q0 ⊗ q1 → базис Белла.
        .gate(Gate::Cx {
            control: 0,
            target: 1,
        })
        .gate(Gate::H { q: 0 })
        // Когерентные коррекции (отложенное измерение):
        // X^{m1} — CNOT(q1→q2), Z^{m0} — CZ(q0→q2).
        .gate(Gate::Cx {
            control: 1,
            target: 2,
        })
        .gate(Gate::Cz {
            control: 0,
            target: 2,
        });
    Ok(c)
}

/// Телепортация с препаратом Ry(θ)|0⟩ = cos(θ/2)|0⟩ + sin(θ/2)|1⟩ на q0
/// и терминальным измерением (для Born-статистики).
pub fn teleport_prepared(theta: f64) -> Result<Circuit> {
    let mut c = Circuit::new(3)?;
    c.gate(Gate::Ry { q: 0, theta });
    for op in teleport()?.ops() {
        c.push(op.clone());
    }
    c.push(Op::MeasureAll);
    Ok(c)
}

/// Отчёт о телепортации: канал q0 → q2 под контролем фиделити.
#[derive(Clone, Debug)]
pub struct TeleportReport {
    /// Угол препарата Ry(θ)|0⟩ (для точного прогона — |+⟩, θ условно π/2).
    pub theta: f64,
    /// Входное состояние q0: амплитуды (re, im).
    pub psi_in: [(f64, f64); 2],
    /// Восстановленное состояние q2 (ветка q0=q1=0; все ветки совпадают).
    pub psi_out: [(f64, f64); 2],
    /// Фиделити |⟨ψ_in|ψ_out⟩|².
    pub fidelity: f64,
    /// Максимальное расхождение амплитуд по всем четырём веткам (q0, q1).
    pub branch_deviation: f64,
    /// Точный прогон в кольце ℤ[1/√2, i] (препарат — Клиффорд |+⟩).
    pub exact: bool,
    /// Вектор Блоха входа (x, y, z).
    pub bloch_in: [f64; 3],
    /// Вектор Блоха выхода (q2).
    pub bloch_out: [f64; 3],
    /// Число вентилей канала.
    pub gate_count: usize,
}

/// Вектор Блоха состояния α|0⟩ + β|1⟩:
/// x = 2Re(α*β), y = 2Im(α*β)... строго: x = 2Re(ᾱβ), y = 2Im(ᾱβ), z = |α|²−|β|².
pub fn bloch_of(alpha: (f64, f64), beta: (f64, f64)) -> [f64; 3] {
    let (ar, ai) = alpha;
    let (br, bi) = beta;
    // ᾱβ = (ar − i·ai)(br + i·bi)
    let re = ar * br + ai * bi;
    let im = ar * bi - ai * br;
    let na = ar * ar + ai * ai;
    let nb = br * br + bi * bi;
    [2.0 * re, 2.0 * im, na - nb]
}

/// Фиделити двух чистых состояний |⟨a|b⟩|².
fn fidelity_of(a: [(f64, f64); 2], b: [(f64, f64); 2]) -> f64 {
    let dot = (a[0].0 * b[0].0 + a[0].1 * b[0].1)
        + (a[1].0 * b[1].0 + a[1].1 * b[1].1);
    dot * dot
}

/// Прогон телепортации с препаратом Ry(θ)|0⟩ (f64-симуляция).
///
/// Фиделити канала обязана быть 1 с точностью до округлений f64 —
/// это и есть контроль отсутствия шума в кремнии.
pub fn run_teleport(theta: f64) -> Result<TeleportReport> {
    use crate::statevector::Statevector;
    let psi_in = [(cos_half(theta), 0.0), (sin_half(theta), 0.0)];
    let mut sv = Statevector::new(3)?;
    sv.apply(Gate::Ry { q: 0, theta })?;
    let core = teleport()?;
    let gate_count = core.ops().len();
    for op in core.ops() {
        if let Op::Gate(g) = op {
            sv.apply(*g)?;
        }
    }
    Ok(teleport_report_from_raw(
        theta,
        psi_in,
        sv.amplitudes(),
        false,
        gate_count,
    ))
}

/// Точный прогон телепортации: препарат |+⟩ = H|0⟩, кольцо ℤ[1/√2, i].
///
/// Финальные амплитуды обязаны быть РОВНО √2/4 (структурное равенство
/// элементов кольца, не f64-сравнение) — машинное доказательство канала.
pub fn run_teleport_exact() -> Result<TeleportReport> {
    let mut sv = crate::exact::ExactStatevector::new(3)?;
    sv.apply(Gate::H { q: 0 })?;
    let core = teleport()?;
    let gate_count = core.ops().len();
    for op in core.ops() {
        if let Op::Gate(g) = op {
            sv.apply(*g)?;
        }
    }
    // Ожидание: |+⟩|+⟩|+⟩ — все восемь амплитуд √2/4 (вещественная):
    // Re = √2/4 = b/2^k при b=1, k=2; Im = 0.
    let expected = crate::exact::ExactCx {
        a: 0,
        b: 1,
        c: 0,
        d: 0,
        k: 2,
    };
    let amps = sv.amplitudes();
    for (i, a) in amps.iter().enumerate() {
        if *a != expected {
            return Err(PqcError::BadArgument {
                what: format!(
                    "точная телепортация нарушена: amps[{i}] = {a:?}, ожидалось √2/4"
                ),
            });
        }
    }
    let psi_in = [(std::f64::consts::FRAC_1_SQRT_2, 0.0); 2];
    let raw: Vec<crate::complex::Cx> = amps
        .iter()
        .map(|a| {
            let (re, im) = a.to_f64();
            crate::complex::Cx { re, im }
        })
        .collect();
    let mut rep = teleport_report_from_raw(
        std::f64::consts::FRAC_PI_2,
        psi_in,
        &raw,
        true,
        gate_count,
    );
    // Точный путь: равенство амплитуд уже доказано структурно —
    // фиделити 1 и нулевая невязка по определению, не по f64.
    rep.branch_deviation = 0.0;
    rep.fidelity = 1.0;
    Ok(rep)
}

fn cos_half(theta: f64) -> f64 {
    (theta * 0.5).cos()
}
fn sin_half(theta: f64) -> f64 {
    (theta * 0.5).sin()
}

/// Сборка отчёта из сырых амплитуд (f64-путь).
fn teleport_report_from_raw(
    theta: f64,
    psi_in: [(f64, f64); 2],
    raw: &[crate::complex::Cx],
    exact: bool,
    gate_count: usize,
) -> TeleportReport {
    let mut worst = 0.0f64;
    let mut psi_out = [(0.0, 0.0); 2];
    let mut first = true;
    for m1 in 0..2usize {
        for m0 in 0..2usize {
            let x = m0 + 2 * m1;
            let a0 = (raw[x].re, raw[x].im);
            let a1 = (raw[x + 4].re, raw[x + 4].im);
            let e0 = (psi_in[0].0 * 0.5, psi_in[0].1 * 0.5);
            let e1 = (psi_in[1].0 * 0.5, psi_in[1].1 * 0.5);
            let d0 = ((a0.0 - e0.0).powi(2) + (a0.1 - e0.1).powi(2)).sqrt();
            let d1 = ((a1.0 - e1.0).powi(2) + (a1.1 - e1.1).powi(2)).sqrt();
            worst = worst.max(d0).max(d1);
            if first {
                psi_out = [(a0.0 * 2.0, a0.1 * 2.0), (a1.0 * 2.0, a1.1 * 2.0)];
                first = false;
            }
        }
    }
    TeleportReport {
        theta,
        psi_in,
        psi_out,
        fidelity: fidelity_of(psi_in, psi_out),
        branch_deviation: worst,
        exact,
        bloch_in: bloch_of(psi_in[0], psi_in[1]),
        bloch_out: bloch_of(psi_out[0], psi_out[1]),
        gate_count,
    }
}

/// Параметры реестра эталонных алгоритмов (цикл P: одна точка правды
/// для CLI-бинаря `pqc`, shell-команды `quantum` и MCP `poler_quantum`).
#[derive(Clone, Debug, Default)]
pub struct AlgoParams {
    /// Число кубитов (--n).
    pub n: usize,
    /// Пометки Гровера/Дойча–Йожи (--marks, запятые).
    pub marks: Vec<usize>,
    /// Секрет Бернштейна–Вазирани (--secret).
    pub secret: u64,
    /// Период гребёнки (--period).
    pub period: usize,
    /// Смещение гребёнки (--offset).
    pub offset: usize,
    /// Угол препарата телепортации (--theta).
    pub theta: f64,
}

impl AlgoParams {
    /// Разумные значения по умолчанию для демо-прогонов.
    pub fn demo(n: usize) -> Self {
        AlgoParams {
            n,
            marks: vec![22 % (1usize << n.max(1))],
            secret: 0b1011,
            period: 0,
            offset: 0,
            theta: 0.7,
        }
    }
}

/// Каталог эталонных алгоритмов: (имя, описание).
pub fn catalog() -> &'static [(&'static str, &'static str)] {
    &[
        ("bell", "пара Белла (|00⟩+|11⟩)/√2 — 2 кубита"),
        ("ghz", "GHZ (|0…0⟩+|1…1⟩)/√2 — --n кубитов"),
        ("qft", "квантовое преобразование Фурье — --n"),
        ("iqft", "обратное QFT — --n"),
        ("grover", "поиск Гровера — --n --marks 22"),
        ("bv", "Бернштейн–Вазирани — --n --secret 11"),
        ("dj", "Дойч–Йожа — --n --marks"),
        ("period", "поиск периода (ядро Шора) — --n --period R"),
        ("teleport", "квантовая телепортация — --theta rad"),
    ]
}

/// Построить схему алгоритма по имени (общая точка входа CLI/shell/MCP).
///
/// Возвращает схему и число итераций (Гровер).
pub fn build(name: &str, p: &AlgoParams) -> Result<(Circuit, usize)> {
    let n = if p.n == 0 { 4 } else { p.n };
    match name {
        "bell" => bell().map(|c| (c, 0)),
        "ghz" => ghz(n).map(|c| (c, 0)),
        "qft" => qft(n, false).map(|c| (c, 0)),
        "iqft" => qft(n, true).map(|c| (c, 0)),
        "grover" => {
            let marks = if p.marks.is_empty() {
                vec![22 % (1usize << n)]
            } else {
                p.marks.clone()
            };
            grover(n, &marks)
        }
        "bv" => bernstein_vazirani(n, p.secret).map(|c| (c, 0)),
        "dj" => {
            let marks = if p.marks.is_empty() {
                vec![22 % (1usize << n)]
            } else {
                p.marks.clone()
            };
            deutsch_jozsa(n, &marks).map(|c| (c, 0))
        }
        "period" => {
            if p.period == 0 {
                Err(PqcError::BadArgument {
                    what: "period: задайте --period R (1 ≤ R < 2^n)".into(),
                })
            } else {
                period_finding(n, p.period, p.offset).map(|c| (c, 0))
            }
        }
        "teleport" => teleport_prepared(p.theta).map(|c| (c, 0)),
        other => Err(PqcError::BadArgument {
            what: format!(
                "неизвестный алгоритм `{other}` (доступно: bell ghz qft iqft grover bv dj period teleport)"
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qpc::run;

    const TOL: f64 = 1e-10;

    #[test]
    fn qft_of_zero_is_uniform() {
        let c = qft(4, false).unwrap();
        // Без измерений — смотрим вероятности через run(shots=0).
        let rep = run(&c, 0, 1).unwrap();
        for (i, &p) in rep.probabilities.iter().enumerate() {
            assert!(
                (p - 1.0 / 16.0).abs() < TOL,
                "P[{i}] = {p}, expected 1/16"
            );
        }
    }

    #[test]
    fn qft_then_iqft_is_identity() {
        let mut c = qft(5, false).unwrap();
        let inv = qft(5, true).unwrap();
        for op in inv.ops() {
            c.push(op.clone());
        }
        let rep = run(&c, 0, 1).unwrap();
        assert!((rep.probabilities[0] - 1.0).abs() < 1e-9, "back to |0>");
        for (i, &p) in rep.probabilities.iter().enumerate().skip(1) {
            assert!(p < 1e-9, "P[{i}] = {p} must vanish");
        }
        assert!((rep.norm - 1.0).abs() < 1e-12);
    }

    #[test]
    fn qft_phase_structure_on_single_bit() {
        // |1⟩ (x=1) через QFT(2): амплитуды — четвертные корни из −1 / 2,
        // фазы {0, π/2, π, −π/2} в переставленной (бит-реверсной) конвенции.
        let mut c = Circuit::new(2).unwrap();
        c.gate(Gate::X { q: 0 });
        let f = qft(2, false).unwrap();
        for op in f.ops() {
            c.push(op.clone());
        }
        let rep = run(&c, 0, 1).unwrap();
        let a = rep.final_state.amplitudes();
        let mut phases: Vec<f64> = Vec::new();
        for amp in a {
            assert!((amp.norm() - 0.5).abs() < TOL, "uniform magnitude");
            phases.push(amp.im.atan2(amp.re));
        }
        phases.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let expected = [
            -core::f64::consts::FRAC_PI_2,
            0.0,
            core::f64::consts::FRAC_PI_2,
            core::f64::consts::PI,
        ];
        for (got, &want) in phases.iter().zip(expected.iter()) {
            assert!((got - want).abs() < 1e-9, "phases {phases:?}");
        }
    }

    #[test]
    fn grover_finds_marked_state() {
        // 5 кубитов, одно помеченное состояние 0b10110 = 22.
        let (c, r) = grover(5, &[22]).unwrap();
        let rep = run(&c, 0, 1).unwrap();
        let p = rep.probabilities[22];
        assert!(p > 0.95, "P(marked) = {p} after {r} iterations");
    }

    #[test]
    fn grover_two_marks() {
        let (c, _) = grover(6, &[9, 40]).unwrap();
        let rep = run(&c, 0, 1).unwrap();
        let p = rep.probabilities[9] + rep.probabilities[40];
        assert!(p > 0.9, "P(marks) = {p}");
    }

    #[test]
    fn bv_recovers_secret() {
        let secret = 0b1011u64;
        let c = bernstein_vazirani(4, secret).unwrap();
        let rep = run(&c, 0, 1).unwrap();
        // Исход: биты данных несут секрет (анцилла не измерялась, но после
        // H на данных исход детерминирован). Крайний случай: исход с младшими
        // 4 битами = secret и любым значением анциллы.
        let mut best = 0.0f64;
        for (i, &p) in rep.probabilities.iter().enumerate() {
            if i & 0xF == secret as usize {
                best += p;
            }
        }
        assert!(best > 0.999, "P(secret) = {best}");
    }

    #[test]
    fn dj_constant_and_balanced() {
        // Константная f = 0 (нет меток): исход |0…0⟩ с вероятностью 1.
        let c0 = deutsch_jozsa(4, &[]).unwrap();
        let r0 = run(&c0, 0, 1).unwrap();
        assert!((r0.probabilities[0] - 1.0).abs() < TOL);

        // Сбалансированная f = младший бит: 8 нулей из 16.
        let marks: Vec<usize> = (0..16usize).filter(|x| x & 1 == 1).collect();
        let c1 = deutsch_jozsa(4, &marks).unwrap();
        let r1 = run(&c1, 0, 1).unwrap();
        assert!(r1.probabilities[0] < 1e-10, "balanced never gives |0>");
    }

    #[test]
    fn period_finding_exact_divisor() {
        // r = 4 делит N = 64: пики ровно на k ∈ {0, 16, 32, 48}, каждый 1/4.
        let c = period_finding(6, 4, 0).unwrap();
        let rep = run(&c, 0, 1).unwrap();
        for (k, &p) in rep.probabilities.iter().enumerate() {
            let want = if k % 16 == 0 { 0.25 } else { 0.0 };
            assert!((p - want).abs() < TOL, "k={k}: {p} vs {want}");
        }
        // Сдвиг не влияет на |QFT|ψ⟩|² — только фазовый множитель.
        let c2 = period_finding(6, 4, 3).unwrap();
        let rep2 = run(&c2, 0, 1).unwrap();
        for (a, b) in rep.probabilities.iter().zip(&rep2.probabilities) {
            assert!((a - b).abs() < TOL);
        }
    }

    #[test]
    fn period_finding_dirichlet_parity() {
        // r = 6 не делит N = 64: вся кривая |QFT|ψ⟩|² совпадает с
        // аналитикой Дирихле comb_qft_prob по всем 64 точкам.
        for &(n, r, o) in &[(6usize, 6usize, 0usize), (6, 6, 5), (5, 7, 2), (4, 3, 1)] {
            let c = period_finding(n, r, o).unwrap();
            let rep = run(&c, 0, 1).unwrap();
            let mut max_dev = 0.0f64;
            for (k, &p) in rep.probabilities.iter().enumerate() {
                let want = comb_qft_prob(n, r, o, k);
                max_dev = max_dev.max((p - want).abs());
            }
            assert!(max_dev < TOL, "n={n} r={r} o={o}: max dev {max_dev}");
            // Сумма вероятностей аналитики = 1 (Parseval).
            let s: f64 = (0..1usize << n).map(|k| comb_qft_prob(n, r, o, k)).sum();
            assert!((s - 1.0).abs() < 1e-9, "n={n} r={r}: sum {s}");
        }
    }

    #[test]
    fn period_finding_recovers_period_by_lcm() {
        // Выстрелы → главные лепестки (p̂ ≥ 0.3·max) → recover_period = r.
        // r = 6: доказуемая зона (1.5·36 = 54 < 64); r = 12: за границей
        // (1.5·144 = 216 > 64) — эвристика, восстановление эмпирически
        // подтверждено (все 12 лепестков наблюдаются и согласованы).
        for &(n, r) in &[(6usize, 6usize), (6, 4), (6, 12), (5, 5), (7, 7)] {
            let c = period_finding(n, r, 0).unwrap();
            let rep = run(&c, 1024, 42 + r as u64).unwrap();
            let max_cnt = rep.counts.iter().map(|(_, c)| *c).max().unwrap_or(0);
            let thr = ((max_cnt as f64) * 0.3) as u64;
            let peaks: Vec<usize> = rep
                .counts
                .iter()
                .filter(|&&(o, cnt)| o != 0 && cnt >= thr.max(6))
                .map(|&(o, _)| o as usize)
                .collect();
            assert!(peaks.len() >= 2, "n={n} r={r}: peaks {peaks:?}");
            let rec = recover_period(&peaks, n).expect("recovery");
            assert_eq!(rec, r, "n={n} r={r}: recovered {rec}, peaks {peaks:?}");
        }
    }

    #[test]
    fn recover_period_rejects_divisors_and_strangers() {
        // Пики r=6 (N=64): 5 не кратно 6 — отвергается; 3 — делитель: отвергается;
        // 12 — кратное: проходит, но минимум — 6.
        let peaks = [11usize, 21, 32, 43, 53];
        assert_eq!(recover_period(&peaks, 6), Some(6));
        // Один пик 11 (l=1, gcd=1): конвергент 1/6… но и 1/5, 5/29 рядом —
        // правило целостности выбирает 6 (5: 55/64 = 0.859 — далеко от целого).
        assert_eq!(recover_period(&[11], 6), Some(6));
        // Пики r=4 (N=64): q=2 отвергается (16·2/64 = 0.5 — полупуть).
        assert_eq!(recover_period(&[16, 32, 48], 6), Some(4));
        // Пустой вход — None.
        assert_eq!(recover_period(&[], 6), None);
    }

    #[test]
    fn convergents_reference_values() {
        // 11/64 → [0;5,1,4,2] → конвергенты 0/1, 1/5, 1/6, 5/29, 11/64.
        let cs = convergents(11, 64);
        assert_eq!(
            cs,
            vec![(0usize, 1usize), (1, 5), (1, 6), (5, 29), (11, 64)]
        );
        // 21/64 → [0;3,21] → 0/1, 1/3, 21/64.
        assert_eq!(convergents(21, 64), vec![(0usize, 1usize), (1, 3), (21, 64)]);
        // 53/64 → [0;1,4,1,4,2] → 0/1, 1/1, 4/5, 5/6, 24/29, 53/64.
        assert_eq!(
            convergents(53, 64),
            vec![(0usize, 1usize), (1, 1), (4, 5), (5, 6), (24, 29), (53, 64)]
        );
        // Кандидаты периода из k=11, n=6: {5, 6, 29, 64} → фильтр → 5,6,29.
        assert_eq!(period_candidates(11, 6), vec![5usize, 6, 29]);
    }

    #[test]
    fn period_finding_bad_args() {
        assert!(period_finding(0, 4, 0).is_err());
        assert!(period_finding(6, 0, 0).is_err());
        assert!(period_finding(6, 64, 0).is_err());
        assert!(period_finding(6, 4, 4).is_err());
        assert!(period_finding(6, 4, 9).is_err());
    }

    // -----------------------------------------------------------------
    // Цикл P: телепортация
    // -----------------------------------------------------------------

    #[test]
    fn teleport_is_six_clifford_gates() {
        let c = teleport().unwrap();
        assert_eq!(c.n_qubits(), 3);
        assert_eq!(c.ops().len(), 6);
        // Все вентили — Клиффорд: точная верификация в кольце доступна.
        for op in c.ops() {
            match op {
                Op::Gate(Gate::H { .. })
                | Op::Gate(Gate::Cx { .. })
                | Op::Gate(Gate::Cz { .. }) => {}
                other => panic!("не-Клиффорд вентиль в канале: {other:?}"),
            }
        }
    }

    #[test]
    fn teleport_preserves_state_f64() {
        // |0⟩, |1⟩, |+⟩, |−⟩ и общий Ry(0.7): фиделити обязана быть 1.
        for theta in [0.0, core::f64::consts::PI, std::f64::consts::FRAC_PI_2, -std::f64::consts::FRAC_PI_2, 0.7] {
            let rep = run_teleport(theta).unwrap();
            assert!(
                (rep.fidelity - 1.0).abs() < 1e-12,
                "theta={theta}: фиделити {} < 1",
                rep.fidelity
            );
            assert!(
                rep.branch_deviation < 1e-12,
                "theta={theta}: расхождение веток {}",
                rep.branch_deviation
            );
            // Вектор Блоха сохраняется каналом точно.
            for i in 0..3 {
                assert!(
                    (rep.bloch_in[i] - rep.bloch_out[i]).abs() < 1e-12,
                    "theta={theta}: Блох[{i}] in={} out={}",
                    rep.bloch_in[i],
                    rep.bloch_out[i]
                );
            }
        }
    }

    #[test]
    fn teleport_exact_in_ring() {
        // Препарат |+⟩: все амплитуды — РОВНО √2/4 в ℤ[1/√2, i].
        let rep = run_teleport_exact().unwrap();
        assert!(rep.exact);
        assert_eq!(rep.fidelity, 1.0);
        assert_eq!(rep.branch_deviation, 0.0);
        assert_eq!(rep.gate_count, 6);
        // |+⟩: x = +1 на экваторе.
        assert!((rep.bloch_out[0] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn teleport_prepared_runs_through_qpc() {
        // Схема с MeasureAll совместима с общим раннером.
        // Финал: |+⟩|+⟩|ψ⟩, ψ = Ry(0.7)|0⟩ ⟹ P = ¼·cos²(θ/2) или ¼·sin²(θ/2).
        let theta = 0.7f64;
        let c = teleport_prepared(theta).unwrap();
        let rep = run(&c, 4096, 42).unwrap();
        assert_eq!(rep.n_qubits, 3);
        let p_lo = 0.25 * (theta * 0.5).cos().powi(2);
        let p_hi = 0.25 * (theta * 0.5).sin().powi(2);
        for (i, &p) in rep.probabilities.iter().enumerate() {
            let expect = if (i >> 2) & 1 == 0 { p_lo } else { p_hi };
            assert!(
                (p - expect).abs() < 1e-9,
                "P[{i}] = {p}, ожидалось {expect}"
            );
        }
        // Маргинала q2: P(1) = sin²(θ/2) — состояние доставлено.
        assert!((rep.marginals[2] - (theta * 0.5).sin().powi(2)).abs() < 1e-9);
    }

    #[test]
    fn bloch_of_pure_states() {
        // |0⟩: северный полюс.
        assert_eq!(bloch_of((1.0, 0.0), (0.0, 0.0)), [0.0, 0.0, 1.0]);
        // |1⟩: южный полюс.
        assert_eq!(bloch_of((0.0, 0.0), (1.0, 0.0)), [0.0, 0.0, -1.0]);
        // |+⟩: x = +1.
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let b = bloch_of((s, 0.0), (s, 0.0));
        assert!((b[0] - 1.0).abs() < 1e-15 && b[2].abs() < 1e-15);
        // |i+⟩ = (|0⟩ + i|1⟩)/√2: y = +1.
        let b = bloch_of((s, 0.0), (0.0, s));
        assert!((b[1] - 1.0).abs() < 1e-15);
    }

    #[test]
    fn registry_builds_all_algorithms() {
        for (name, _) in catalog() {
            let p = AlgoParams {
                n: 4,
                marks: vec![3],
                secret: 11,
                period: if *name == "period" { 5 } else { 0 },
                offset: 0,
                theta: 0.7,
            };
            let (c, _) = build(name, &p).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(c.n_qubits() >= 2, "{name}: пустая схема");
        }
        // Неизвестное имя — честный отказ.
        assert!(build("no-such-algo", &AlgoParams::demo(4)).is_err());
    }
}
