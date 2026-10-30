//! # Точный режим: идеальные кубиты в кольце ℤ[1/√2, i]
//!
//! Амплитуды схем из {H, X, Y, Z, S, T, S†, T†, CX, CZ, SWAP, CCX,
//! flipphase, flipzero} живут точно в кольце
//!
//! ```text
//! z = (a + b·√2 + i·(c + d·√2)) / 2^k,   a,b,c,d ∈ ℤ, k ∈ ℕ
//! ```
//!
//! — это кольцо целостности (подкольцо алгебраических чисел ℝ[i, √2]).
//! Все операции — целочисленные с проверкой переполнения: **ни одного
//! округления за всю схему**. Физическая машина теряет ~1e-3 на гейт;
//! f64-симуляция теряет 2.2e-16 на операцию; здесь погрешность равна
//! нулю в принципе — вероятности Борна читаются как точные элементы
//! ℤ[√2] с диадическим знаменателем.
//!
//! ## Честные границы
//!
//! * Коллапс (mid-circuit measure) требует 1/√p — квадратичного
//!   расширения кольца; точный режим измеряет только терминально
//!   (семпл Берна по f64-конверсии точного распределения).
//! * Вращения Ry/Rx/Rz/CP вне {π/2, π} не лежат в кольце — отклоняются.
//! * Глубина ограничена i128 (сотни гейтов; переполнение детектируется).

use crate::error::{PqcError, Result};
use crate::gates::Gate;
use crate::qpc::{Circuit, Op};

/// √2 в f64 (только для конверсии отображения, не для арифметики).
const SQRT2: f64 = core::f64::consts::SQRT_2;

/// Распаковка Option<i128> с ошибкой переполнения кольца.
#[inline]
fn ck(v: Option<i128>) -> Result<i128> {
    v.ok_or(PqcError::ExactOverflow)
}

/// Точное число кольца ℤ[√2] с диадическим знаменателем: (a + b√2)/2^k.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactDyadic {
    /// Рациональная часть числителя.
    pub a: i128,
    /// Коэффициент при √2.
    pub b: i128,
    /// Диадический знаменатель 2^k.
    pub k: u32,
}

impl ExactDyadic {
    /// Нуль.
    pub const ZERO: ExactDyadic = ExactDyadic { a: 0, b: 0, k: 0 };

    /// Числитель-нормализация: сброс общего множителя 2.
    fn reduce(a: i128, b: i128, k: u32) -> ExactDyadic {
        let (mut a, mut b, mut k) = (a, b, k);
        while k > 0 && (a & 1 == 0) && (b & 1 == 0) {
            a >>= 1;
            b >>= 1;
            k -= 1;
        }
        ExactDyadic { a, b, k }
    }

    /// f64-приближение (для отображения и сэмплирования).
    pub fn to_f64(self) -> f64 {
        (self.a as f64 + self.b as f64 * SQRT2) / 2.0f64.powi(self.k.min(1000) as i32)
    }
}

impl core::fmt::Display for ExactDyadic {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.k == 0 {
            return match (self.a, self.b) {
                (0, 0) => write!(f, "0"),
                (a, 0) => write!(f, "{a}"),
                (0, b) => write!(f, "{b}·√2"),
                (a, b) => write!(f, "{a} {b:+}·√2"),
            };
        }
        match (self.a, self.b) {
            (0, 0) => write!(f, "0"),
            (a, 0) => write!(f, "{a}/2^{}", self.k),
            (0, b) => write!(f, "{b}·√2/2^{}", self.k),
            (a, b) => write!(f, "({a} {b:+}·√2)/2^{}", self.k),
        }
    }
}

/// Точная комплексная амплитуда: (a + b√2 + i·(c + d√2)) / 2^k.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactCx {
    /// Рациональная часть Re.
    pub a: i128,
    /// √2-часть Re.
    pub b: i128,
    /// Рациональная часть Im.
    pub c: i128,
    /// √2-часть Im.
    pub d: i128,
    /// Диадический знаменатель 2^k.
    pub k: u32,
}

impl ExactCx {
    /// Нуль.
    pub const ZERO: ExactCx = ExactCx { a: 0, b: 0, c: 0, d: 0, k: 0 };
    /// Единица.
    pub const ONE: ExactCx = ExactCx { a: 1, b: 0, c: 0, d: 0, k: 0 };

    fn reduce(a: i128, b: i128, c: i128, d: i128, k: u32) -> ExactCx {
        let (mut a, mut b, mut c, mut d, mut k) = (a, b, c, d, k);
        while k > 0 && (a & 1 == 0) && (b & 1 == 0) && (c & 1 == 0) && (d & 1 == 0) {
            a >>= 1;
            b >>= 1;
            c >>= 1;
            d >>= 1;
            k -= 1;
        }
        ExactCx { a, b, c, d, k }
    }

    /// Сложение (выравнивание знаменателей сдвигом).
    pub fn add(self, rhs: ExactCx) -> Result<ExactCx> {
        let k = self.k.max(rhs.k);
        let sh = |v: i128, from: u32| -> Result<i128> {
            let s = k - from;
            if s >= 127 {
                return Err(PqcError::ExactOverflow);
            }
            let shifted = ck(v.checked_shl(s))?;
            // Контроль потери битов при сдвиге влево.
            if (shifted >> s) != v {
                return Err(PqcError::ExactOverflow);
            }
            Ok(shifted)
        };
        let a = ck(sh(self.a, self.k)?.checked_add(sh(rhs.a, rhs.k)?))?;
        let b = ck(sh(self.b, self.k)?.checked_add(sh(rhs.b, rhs.k)?))?;
        let c = ck(sh(self.c, self.k)?.checked_add(sh(rhs.c, rhs.k)?))?;
        let d = ck(sh(self.d, self.k)?.checked_add(sh(rhs.d, rhs.k)?))?;
        Ok(ExactCx::reduce(a, b, c, d, k))
    }

    /// Умножение (полное раскрытие произведения билинейных форм).
    pub fn mul(self, rhs: ExactCx) -> Result<ExactCx> {
        let m = |x: i128, y: i128| -> Result<i128> { ck(x.checked_mul(y)) };
        let a = |x: i128, y: i128| -> Result<i128> { ck(x.checked_add(y)) };
        let s = |x: i128, y: i128| -> Result<i128> { ck(x.checked_sub(y)) };

        // re = a₁a₂ + 2b₁b₂ − c₁c₂ − 2d₁d₂
        let b1b2 = m(self.b, rhs.b)?;
        let d1d2 = m(self.d, rhs.d)?;
        let re_a = s(
            a(m(self.a, rhs.a)?, m(b1b2, 2)?)?,
            a(m(self.c, rhs.c)?, m(d1d2, 2)?)?,
        )?;
        // re-коэффициент при √2: a₁b₂ + b₁a₂ − c₁d₂ − d₁c₂
        let re_b = s(
            a(m(self.a, rhs.b)?, m(self.b, rhs.a)?)?,
            a(m(self.c, rhs.d)?, m(self.d, rhs.c)?)?,
        )?;
        // im = a₁c₂ + 2b₁d₂ + c₁a₂ + 2d₁b₂
        let b1d2 = m(self.b, rhs.d)?;
        let d1b2 = m(self.d, rhs.b)?;
        let im_c = a(
            a(a(m(self.a, rhs.c)?, m(b1d2, 2)?)?, m(self.c, rhs.a)?)?,
            m(d1b2, 2)?,
        )?;
        // im-коэффициент при √2: a₁d₂ + b₁c₂ + c₁b₂ + d₁a₂
        let im_d = a(
            a(a(m(self.a, rhs.d)?, m(self.b, rhs.c)?)?, m(self.c, rhs.b)?)?,
            m(self.d, rhs.a)?,
        )?;
        Ok(ExactCx::reduce(re_a, re_b, im_c, im_d, self.k + rhs.k))
    }

    /// Умножение на i: z → iz (гейт S на |1⟩).
    pub fn mul_i(self) -> ExactCx {
        // iz = −y + ix: re' = −im, im' = re.
        ExactCx::reduce(-self.c, -self.d, self.a, self.b, self.k)
    }

    /// Умножение на (1+i) (полушаг T).
    pub fn mul_1_plus_i(self) -> Result<ExactCx> {
        // (x + iy)(1+i) = (x − y) + i(x + y)
        let a = ck(self.a.checked_sub(self.c))?;
        let b = ck(self.b.checked_sub(self.d))?;
        let c = ck(self.a.checked_add(self.c))?;
        let d = ck(self.b.checked_add(self.d))?;
        Ok(ExactCx::reduce(a, b, c, d, self.k))
    }

    /// T = e^{iπ/4} = (1+i)/√2.
    pub fn mul_t(self) -> Result<ExactCx> {
        self.mul_1_plus_i()?.mul_omega_checked()
    }

    /// T† = e^{−iπ/4} = (1−i)/√2.
    pub fn mul_tdg(self) -> Result<ExactCx> {
        // (x + iy)(1−i) = (x + y) + i(y − x)
        let a = ck(self.a.checked_add(self.c))?;
        let b = ck(self.b.checked_add(self.d))?;
        let c = ck(self.c.checked_sub(self.a))?;
        let d = ck(self.d.checked_sub(self.b))?;
        ExactCx::reduce(a, b, c, d, self.k).mul_omega_checked()
    }

    /// ω-масштаб с контролем переполнения (b·2 может переполниться).
    fn mul_omega_checked(self) -> Result<ExactCx> {
        let b2 = self.b.checked_mul(2).ok_or(PqcError::ExactOverflow)?;
        let d2 = self.d.checked_mul(2).ok_or(PqcError::ExactOverflow)?;
        Ok(ExactCx::reduce(b2, self.a, d2, self.c, self.k + 1))
    }

    /// Сопряжение.
    pub fn conj(self) -> ExactCx {
        ExactCx::reduce(self.a, self.b, -self.c, -self.d, self.k)
    }

    /// |z|² точно: (a²+2b²+c²+2d² + 2(ab+cd)√2)/2^{2k}.
    pub fn norm_sq(self) -> Result<ExactDyadic> {
        let m = |x: i128, y: i128| -> Result<i128> { ck(x.checked_mul(y)) };
        let a = |x: i128, y: i128| -> Result<i128> { ck(x.checked_add(y)) };

        let aa = m(self.a, self.a)?;
        let bb = m(self.b, self.b)?;
        let cc = m(self.c, self.c)?;
        let dd = m(self.d, self.d)?;
        let num_a = a(a(a(aa, m(bb, 2)?)?, cc)?, m(dd, 2)?)?;
        let ab = m(self.a, self.b)?;
        let cd = m(self.c, self.d)?;
        let num_b = m(a(ab, cd)?, 2)?;
        Ok(ExactDyadic::reduce(num_a, num_b, self.k * 2))
    }

    /// f64-приближение (для кросс-чека и сэмплирования).
    pub fn to_f64(self) -> (f64, f64) {
        let den = 2.0f64.powi(self.k as i32);
        (
            (self.a as f64 + self.b as f64 * SQRT2) / den,
            (self.c as f64 + self.d as f64 * SQRT2) / den,
        )
    }

    /// Отображение в канонической форме.
    pub fn display(self) -> String {
        let part = |r: i128, s: i128| -> String {
            match (r, s) {
                (0, 0) => "0".into(),
                (0, s) => format!("{s}·√2"),
                (r, 0) => format!("{r}"),
                (r, s) => format!("({r} {s:+}·√2)"),
            }
        };
        if self.k == 0 {
            format!("{} + i·{}", part(self.a, self.b), part(self.c, self.d))
        } else {
            format!(
                "[{} + i·{}]/2^{}",
                part(self.a, self.b),
                part(self.c, self.d),
                self.k
            )
        }
    }
}

/// Состояние идеальных кубитов: 2^n точных амплитуд.
#[derive(Clone, Debug)]
pub struct ExactStatevector {
    n_qubits: usize,
    amps: Vec<ExactCx>,
}

/// Результат точного прогона.
#[derive(Clone, Debug)]
pub struct ExactReport {
    /// Число кубитов.
    pub n_qubits: usize,
    /// Точные вероятности Борна |⟨x|ψ⟩|² (элементы ℤ[√2], диадические).
    pub exact_probs: Vec<ExactDyadic>,
    /// f64-приближения вероятностей.
    pub probs_f64: Vec<f64>,
    /// Точные амплитуды.
    pub amplitudes: Vec<ExactCx>,
    /// Норма-невязка: |Σ|⟨x|ψ⟩|² − 1| в f64 (контроль целостности кольца).
    pub norm_residual: f64,
}

impl ExactStatevector {
    /// |0…0⟩.
    pub fn new(n_qubits: usize) -> Result<ExactStatevector> {
        if n_qubits == 0 {
            return Err(PqcError::EmptyState);
        }
        if n_qubits > crate::statevector::MAX_QUBITS {
            return Err(PqcError::TooManyQubits {
                requested: n_qubits,
                max: crate::statevector::MAX_QUBITS,
            });
        }
        let mut amps = vec![ExactCx::ZERO; 1usize << n_qubits];
        amps[0] = ExactCx::ONE;
        Ok(ExactStatevector { n_qubits, amps })
    }

    /// Амплитуды.
    pub fn amplitudes(&self) -> &[ExactCx] {
        &self.amps
    }

    fn check_q(&self, q: usize) -> Result<()> {
        if q >= self.n_qubits {
            Err(PqcError::BadQubit {
                q,
                n_qubits: self.n_qubits,
            })
        } else {
            Ok(())
        }
    }

    /// Применить гейт точно. Вне кольца — ошибка [`PqcError::NotExactGate`].
    pub fn apply(&mut self, gate: Gate) -> Result<()> {
        match gate {
            Gate::H { q } => {
                self.check_q(q)?;
                let half = 1usize << q;
                for blk_start in (0..self.amps.len()).step_by(half << 1) {
                    for i in 0..half {
                        let (a, b) = (
                            self.amps[blk_start + i],
                            self.amps[blk_start + half + i],
                        );
                        // a' = (a+b)/√2, b' = (a−b)/√2
                        let sum = a.add(b)?;
                        let diff = a.add(ExactCx {
                            a: -b.a,
                            b: -b.b,
                            c: -b.c,
                            d: -b.d,
                            k: b.k,
                        })?;
                        self.amps[blk_start + i] = sum.mul_omega_checked()?;
                        self.amps[blk_start + half + i] = diff.mul_omega_checked()?;
                    }
                }
                Ok(())
            }
            Gate::X { q } => {
                self.check_q(q)?;
                let half = 1usize << q;
                for blk_start in (0..self.amps.len()).step_by(half << 1) {
                    for i in 0..half {
                        self.amps.swap(blk_start + i, blk_start + half + i);
                    }
                }
                Ok(())
            }
            Gate::Y { q } => {
                self.check_q(q)?;
                let half = 1usize << q;
                for blk_start in (0..self.amps.len()).step_by(half << 1) {
                    for i in 0..half {
                        let (a, b) = (
                            self.amps[blk_start + i],
                            self.amps[blk_start + half + i],
                        );
                        // a' = −i·b, b' = i·a
                        self.amps[blk_start + i] = neg_i(b);
                        self.amps[blk_start + half + i] = a.mul_i();
                    }
                }
                Ok(())
            }
            Gate::Z { q } => {
                self.check_q(q)?;
                let half = 1usize << q;
                for blk_start in (0..self.amps.len()).step_by(half << 1) {
                    for i in half..half << 1 {
                        let v = self.amps[blk_start + i];
                        self.amps[blk_start + i] = ExactCx::reduce(
                            -v.a, -v.b, -v.c, -v.d, v.k,
                        );
                    }
                }
                Ok(())
            }
            Gate::S { q } => {
                self.check_q(q)?;
                let half = 1usize << q;
                for blk_start in (0..self.amps.len()).step_by(half << 1) {
                    for i in half..half << 1 {
                        self.amps[blk_start + i] = self.amps[blk_start + i].mul_i();
                    }
                }
                Ok(())
            }
            Gate::Sdg { q } => {
                self.check_q(q)?;
                let half = 1usize << q;
                for blk_start in (0..self.amps.len()).step_by(half << 1) {
                    for i in half..half << 1 {
                        self.amps[blk_start + i] = neg_i(self.amps[blk_start + i]);
                    }
                }
                Ok(())
            }
            Gate::T { q } => {
                self.check_q(q)?;
                let half = 1usize << q;
                for blk_start in (0..self.amps.len()).step_by(half << 1) {
                    for i in half..half << 1 {
                        self.amps[blk_start + i] = self.amps[blk_start + i].mul_t()?;
                    }
                }
                Ok(())
            }
            Gate::Tdg { q } => {
                self.check_q(q)?;
                let half = 1usize << q;
                for blk_start in (0..self.amps.len()).step_by(half << 1) {
                    for i in half..half << 1 {
                        self.amps[blk_start + i] = self.amps[blk_start + i].mul_tdg()?;
                    }
                }
                Ok(())
            }
            Gate::Cx { control, target } => {
                self.check_q(control)?;
                self.check_q(target)?;
                if control == target {
                    return Err(PqcError::SameQubit { control, target });
                }
                let (bc, bt) = (1usize << control, 1usize << target);
                for i in 0..self.amps.len() {
                    if i & bc != 0 && i & bt == 0 {
                        self.amps.swap(i, i | bt);
                    }
                }
                Ok(())
            }
            Gate::Cz { control, target } => {
                self.check_q(control)?;
                self.check_q(target)?;
                if control == target {
                    return Err(PqcError::SameQubit { control, target });
                }
                let (bc, bt) = (1usize << control, 1usize << target);
                for (i, v) in self.amps.iter_mut().enumerate() {
                    if i & bc != 0 && i & bt != 0 {
                        *v = ExactCx::reduce(-v.a, -v.b, -v.c, -v.d, v.k);
                    }
                }
                Ok(())
            }
            Gate::Swap { a, b } => {
                self.check_q(a)?;
                self.check_q(b)?;
                if a == b {
                    return Err(PqcError::SameQubit { control: a, target: b });
                }
                let (ba, bb) = (1usize << a, 1usize << b);
                for i in 0..self.amps.len() {
                    if i & ba != 0 && i & bb == 0 {
                        self.amps.swap(i, i ^ ba ^ bb);
                    }
                }
                Ok(())
            }
            Gate::Ccx { c1, c2, target } => {
                self.check_q(c1)?;
                self.check_q(c2)?;
                self.check_q(target)?;
                if c1 == c2 || c1 == target || c2 == target {
                    return Err(PqcError::BadCcxCollision);
                }
                let (b1, b2, bt) = (1usize << c1, 1usize << c2, 1usize << target);
                for i in 0..self.amps.len() {
                    if i & b1 != 0 && i & b2 != 0 && i & bt == 0 {
                        self.amps.swap(i, i | bt);
                    }
                }
                Ok(())
            }
            // Цикл P: CP(θ) точен в кольце ⟺ e^{iθ} ∈ ℤ[1/√2, i],
            // т.е. θ = k·π/4 (mod 2π). Множитель — степень t = e^{iπ/4}
            // (mul_t/mul_tdg уже живут в кольце). QFT-3 (CP(π/2), CP(π/4))
            // становится доказуемо точным.
            Gate::Cp {
                control,
                target,
                theta,
            } => {
                self.check_q(control)?;
                self.check_q(target)?;
                if control == target {
                    return Err(PqcError::SameQubit { control, target });
                }
                let quarter = core::f64::consts::FRAC_PI_4;
                let tau = core::f64::consts::TAU;
                // Кратчайшая дуга: k ∈ (−4…4], остаток сравнивается по mod 2π.
                let mut k = (theta / quarter).round() as i64;
                k = k.rem_euclid(8);
                if k > 4 {
                    k -= 8;
                }
                let residual = (theta - k as f64 * quarter).rem_euclid(tau);
                let residual = residual.min(tau - residual);
                if residual > 1e-9 {
                    return Err(PqcError::NotExactGate {
                        gate: format!("Cp({theta:.6})"),
                    });
                }
                let steps = k.unsigned_abs() as u32;
                let positive = k > 0;
                let (bc, bt) = (1usize << control, 1usize << target);
                for (i, v) in self.amps.iter_mut().enumerate() {
                    if i & bc != 0 && i & bt != 0 && steps > 0 {
                        let mut w = *v;
                        for _ in 0..steps {
                            w = if positive { w.mul_t()? } else { w.mul_tdg()? };
                        }
                        *v = w;
                    }
                }
                Ok(())
            }
            other => Err(PqcError::NotExactGate {
                gate: format!("{other}"),
            }),
        }
    }

    /// Точные вероятности Борна.
    pub fn exact_probabilities(&self) -> Result<Vec<ExactDyadic>> {
        self.amps.iter().map(|a| a.norm_sq()).collect()
    }

    /// Прогнать схему точно (измерения — только терминальные, игнорируются
    /// как в быстром пути `qpc::run`; фазовые оракулы поддержаны).
    pub fn run_circuit(&mut self, circuit: &Circuit) -> Result<()> {
        for op in circuit.ops() {
            match op {
                Op::Gate(g) => self.apply(*g)?,
                Op::FlipPhase { mask } => {
                    for (i, v) in self.amps.iter_mut().enumerate() {
                        if *mask != 0 && i & mask == *mask {
                            *v = ExactCx::reduce(-v.a, -v.b, -v.c, -v.d, v.k);
                        }
                    }
                }
                Op::FlipIndex { idx } => {
                    if *idx < self.amps.len() {
                        let v = self.amps[*idx];
                        self.amps[*idx] = ExactCx::reduce(-v.a, -v.b, -v.c, -v.d, v.k);
                    }
                }
                Op::FlipZero => {
                    for (i, v) in self.amps.iter_mut().enumerate() {
                        if i != 0 {
                            *v = ExactCx::reduce(-v.a, -v.b, -v.c, -v.d, v.k);
                        }
                    }
                }
                Op::PrepComb { period: period_, offset: offset_ } => {
                    let (period, offset) = (*period_, *offset_);
                    // Зубья 1/√m точны в кольце ⟺ m = 2^s:
                    // 2^{-s/2} = 1/2^{s/2} (чётное s) или √2/2^{(s+1)/2}
                    // (нечётное s). Иначе — честный отказ (как Ry вне {0,π/2,π}).
                    let dim = self.amps.len();
                    let mut m = 0usize;
                    for x in 0..dim {
                        if x % period == offset {
                            m += 1;
                        }
                    }
                    let s = m.trailing_zeros();
                    if m == 0 || (1usize << s) != m {
                        return Err(PqcError::NotExactGate {
                            gate: format!(
                                "prepcomb: 1/sqrt(m={m}) outside Z[1/sqrt(2), i] \
                                 (m = N/r must be a power of two)"
                            ),
                        });
                    }
                    let tooth = if s.is_multiple_of(2) {
                        // 1/2^{s/2}
                        ExactCx {
                            a: 1,
                            b: 0,
                            c: 0,
                            d: 0,
                            k: s / 2,
                        }
                    } else {
                        // √2/2^{(s+1)/2}
                        ExactCx {
                            a: 0,
                            b: 1,
                            c: 0,
                            d: 0,
                            k: s.div_ceil(2),
                        }
                    };
                    for (x, v) in self.amps.iter_mut().enumerate() {
                        *v = if x % period == offset { tooth } else { ExactCx::ZERO };
                    }
                }
                Op::Measure { .. } | Op::MeasureAll | Op::Barrier => {}
            }
        }
        Ok(())
    }
}

/// Умножение на −i.
fn neg_i(z: ExactCx) -> ExactCx {
    // −iz = y − ix: re' = im, im' = −re.
    ExactCx::reduce(z.c, z.d, -z.a, -z.b, z.k)
}

/// Полный точный прогон схемы с отчётом.
pub fn run_exact(circuit: &Circuit) -> Result<ExactReport> {
    if circuit.has_mid_circuit_measure() {
        return Err(PqcError::BadArgument {
            what: "exact mode: mid-circuit measurement needs 1/sqrt(p) outside \
                   the ring Z[1/sqrt(2), i]; terminal measurements only"
                .into(),
        });
    }
    let mut sv = ExactStatevector::new(circuit.n_qubits())?;
    sv.run_circuit(circuit)?;
    let exact_probs = sv.exact_probabilities()?;
    let probs_f64: Vec<f64> = exact_probs.iter().map(|p| p.to_f64()).collect();
    let norm_residual = (probs_f64.iter().sum::<f64>() - 1.0).abs();
    Ok(ExactReport {
        n_qubits: circuit.n_qubits(),
        exact_probs,
        probs_f64,
        amplitudes: sv.amps.clone(),
        norm_residual,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bell_state_amplitudes_are_exact() {
        let c = Circuit::parse("qubits 2\nh 0\ncx 0 1\n").unwrap();
        let rep = run_exact(&c).unwrap();
        // |ψ⟩ = (|00⟩ + |11⟩)/√2: точные [1/√2, 0, 0, 1/√2].
        assert_eq!(rep.amplitudes[1], ExactCx::ZERO);
        assert_eq!(rep.amplitudes[2], ExactCx::ZERO);
        // 1/√2 = √2/2: a=0, b=1, k=1.
        assert_eq!(
            rep.amplitudes[0],
            ExactCx { a: 0, b: 1, c: 0, d: 0, k: 1 }
        );
        assert_eq!(
            rep.amplitudes[3],
            ExactCx { a: 0, b: 1, c: 0, d: 0, k: 1 }
        );
        // P = 1/2 точно.
        assert_eq!(rep.exact_probs[0], ExactDyadic { a: 1, b: 0, k: 1 });
        assert!((rep.probs_f64[0] - 0.5).abs() < 1e-15);
        assert!(rep.norm_residual < 1e-15);
    }

    #[test]
    fn t_gate_phase_is_exact_quarter_root() {
        let c = Circuit::parse("qubits 1\nh 0\nt 0\n").unwrap();
        let rep = run_exact(&c).unwrap();
        // |ψ⟩ = (|0⟩ + e^{iπ/4}|1⟩)/√2; e^{iπ/4} = (1+i)/√2.
        // Амплитуда |1⟩ = (1+i)/2: a=1, c=1, k=1.
        assert_eq!(
            rep.amplitudes[1],
            ExactCx { a: 1, b: 0, c: 1, d: 0, k: 1 }
        );
        assert!(rep.norm_residual < 1e-15);
    }

    #[test]
    fn t_squared_is_s() {
        // T·T = S: амплитуда |1⟩ после h, t, t равна i/√2.
        let c = Circuit::parse("qubits 1\nh 0\nt 0\nt 0\n").unwrap();
        let rep = run_exact(&c).unwrap();
        let s = Circuit::parse("qubits 1\nh 0\ns 0\n").unwrap();
        let rep_s = run_exact(&s).unwrap();
        assert_eq!(rep.amplitudes[1], rep_s.amplitudes[1]);
        // i/√2: c=1(мнимая рациональная), b√2-часть мнимой = 1 → c=0, d=1, k=1.
        assert_eq!(
            rep.amplitudes[1],
            ExactCx { a: 0, b: 0, c: 0, d: 1, k: 1 }
        );
    }

    #[test]
    fn grover_two_qubit_exact() {
        // Гровер на 2 кубитах, помечено |11⟩: R = 1 итерация, P = 1 точно.
        let (c, r) = crate::algorithms::grover(2, &[3]).unwrap();
        assert_eq!(r, 1);
        let rep = run_exact(&c).unwrap();
        assert_eq!(rep.exact_probs[3], ExactDyadic { a: 1, b: 0, k: 0 });
        assert_eq!(rep.exact_probs[0], ExactDyadic::ZERO);
        assert!(rep.norm_residual < 1e-15);
    }

    #[test]
    fn deep_clifford_circuit_bit_exact_deterministic() {
        // Одна и та же схема дважды — біт-в-біт идентичные амплитуды
        // (тривиально для детерминированного кода, но фиксирует контракт).
        let text = "qubits 3\nh 0\nh 1\nt 2\ncx 0 1\nt 1\ncx 1 2\nswap 0 2\nccx 0 1 2\ntdg 0\ns 1\ncz 0 2\n";
        let r1 = run_exact(&Circuit::parse(text).unwrap()).unwrap();
        let r2 = run_exact(&Circuit::parse(text).unwrap()).unwrap();
        assert_eq!(r1.amplitudes, r2.amplitudes);
        assert!(r1.norm_residual < 1e-14);
        // Сверка с f64-движком qpc: расхождение только от округлений f64.
        let f64_rep = crate::qpc::run(&Circuit::parse(text).unwrap(), 0, 1).unwrap();
        for (i, ((re, im), a)) in r1
            .amplitudes
            .iter()
            .map(|z| z.to_f64())
            .zip(f64_rep.final_state.amplitudes())
            .enumerate()
        {
            assert!(
                (re - a.re).abs() < 1e-12 && (im - a.im).abs() < 1e-12,
                "amp[{i}]: exact {re}+{im}i vs f64 {}+{}i",
                a.re,
                a.im
            );
        }
    }

    #[test]
    fn rotations_rejected_outside_ring() {
        let c = Circuit::parse("qubits 1\nry 0 0.5\n").unwrap();
        assert!(matches!(
            run_exact(&c),
            Err(PqcError::NotExactGate { .. })
        ));
        let c = Circuit::parse("qubits 2\ncp 0 1 0.3\n").unwrap();
        assert!(matches!(
            run_exact(&c),
            Err(PqcError::NotExactGate { .. })
        ));
    }

    #[test]
    fn mid_circuit_measure_rejected() {
        let c = Circuit::parse("qubits 2\nh 0\nmeasure 0\nh 1\n").unwrap();
        assert!(matches!(
            run_exact(&c),
            Err(PqcError::BadArgument { .. })
        ));
    }

    #[test]
    fn ring_arithmetic_ring_closure() {
        // e^{iπ/4} = (1+i)/√2 = (√2 + i√2)/2 в кольце: a=0, b=1, c=0, d=1, k=1.
        // Квадрат: e^{iπ/2} = i — проверка структуры кольца.
        let z = ExactCx { a: 0, b: 1, c: 0, d: 1, k: 1 };
        let z2 = z.mul(z).unwrap();
        assert_eq!(z2, ExactCx { a: 0, b: 0, c: 1, d: 0, k: 0 });
        // ω = 1/√2: ω² = 1/2 (редукция числителя сдвигает k).
        let omega = ExactCx { a: 0, b: 1, c: 0, d: 0, k: 1 };
        assert_eq!(
            omega.mul(omega).unwrap(),
            ExactCx { a: 1, b: 0, c: 0, d: 0, k: 1 }
        );
        // √2 · √2 = 2.
        let r2 = ExactCx { a: 0, b: 1, c: 0, d: 0, k: 0 };
        assert_eq!(
            r2.mul(r2).unwrap(),
            ExactCx { a: 2, b: 0, c: 0, d: 0, k: 0 }
        );
        // (1+i)/2 · (1−i)/2 = |(1+i)/2|² = 1/2 — сопряжение.
        let w = ExactCx { a: 1, b: 0, c: 1, d: 0, k: 1 };
        assert_eq!(
            w.mul(w.conj()).unwrap(),
            ExactCx { a: 1, b: 0, c: 0, d: 0, k: 1 }
        );
    }
}
