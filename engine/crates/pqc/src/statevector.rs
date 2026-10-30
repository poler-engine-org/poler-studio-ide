//! Statevector-движок: 2^n комплексных амплитуд + авто-векторизуемые ядра
//! применения гейтов + параллельный fork-join на `std::thread`.
//!
//! ## Ядра SIMD
//!
//! Однокубитный гейт на кубите `q` действует на пары амплитуд
//! `(i, i + 2^q)`. Обход организован блоками длины `2^(q+1)`:
//! внутри блока младшая и старшая половины — два непрерывных потока
//! памяти, `zip` двух `&mut [Cx]` авто-векторизуется в SSE/AVX-регистры
//! без единой внешней зависимости. Именно поэтому faer не нужен:
//! statevector-симуляция — это стридовые парные обновления, а не GEMM.
//!
//! ## Конвенция битов
//!
//! Индекс амплитуды `i` несёт бит `b_q` кубита `q` в разряде `q`
//! (младший бит — кубит 0) — так же, как в qiskit; это закладывает
//! паритет с qiskit в RQ4.

use crate::complex::Cx;
use crate::error::{PqcError, Result};
use crate::gates::Gate;
use crate::parallel::{par_blocks_mut, par_zip_map};

/// Жёсткий потолок движка: 2²⁶ амплитуд × 16 байт = 1 GiB.
pub const MAX_QUBITS: usize = 26;

/// θ = arccos(p) — угол анзаца R_y для трека `p ∈ [−1, 1]`.
///
/// Значения на границе (|p| = 1 чуть за счёт округления) зажимаются:
/// пользовательский вход не должен ломать `acos`.
#[inline]
pub fn phase_to_theta(p: f64) -> f64 {
    p.clamp(-1.0, 1.0).acos()
}

/// Чистое состояние в вычислительном базисе: `amps[i]` — амплитуда
/// базисного состояния с битовой строкой `i`.
#[derive(Clone, Debug)]
pub struct Statevector {
    n_qubits: usize,
    amps: Vec<Cx>,
}

impl Statevector {
    /// |0…0⟩ размерности 2^n.
    pub fn new(n_qubits: usize) -> Result<Statevector> {
        if n_qubits == 0 {
            return Err(PqcError::EmptyState);
        }
        if n_qubits > MAX_QUBITS {
            return Err(PqcError::TooManyQubits {
                requested: n_qubits,
                max: MAX_QUBITS,
            });
        }
        let mut amps = vec![Cx::ZERO; 1usize << n_qubits];
        amps[0] = Cx::ONE;
        Ok(Statevector { n_qubits, amps })
    }

    /// Число кубитов.
    pub fn n_qubits(&self) -> usize {
        self.n_qubits
    }

    /// Размерность гильбертова пространства 2^n.
    pub fn dim(&self) -> usize {
        self.amps.len()
    }

    /// Амплитуды (только чтение).
    pub fn amplitudes(&self) -> &[Cx] {
        &self.amps
    }

    /// Амплитуды (мутация — на совести вызывающего: неунитарное
    /// преобразование разрушает норму и Born-правило).
    pub fn amplitudes_mut(&mut self) -> &mut [Cx] {
        &mut self.amps
    }

    fn check_qubit(&self, q: usize) -> Result<()> {
        if q >= self.n_qubits {
            Err(PqcError::BadQubit {
                q,
                n_qubits: self.n_qubits,
            })
        } else {
            Ok(())
        }
    }

    // ------------------------------------------------------------------
    // Анзац POLER: R_y(arccos p)
    // ------------------------------------------------------------------

    /// Анзац POLER: `|ψ⟩ = ⊗_q R_y(θ_q)|0⟩`, `θ_q = arccos(p_q)`.
    ///
    /// Born-семантика трека: `P(b_q = 1) = sin²(θ_q/2) = (1 − p_q)/2`,
    /// то есть собственное значение McWeeny `λ_q = (1 + p_q)/2` — это
    /// в точности `P(b_q = 0)`. Чистые триты: `p = +1` → |0⟩,
    /// `p = −1` → |1⟩, `p = 0` → честная монета.
    pub fn from_phases(ps: &[f64]) -> Result<Statevector> {
        let mut sv = Statevector::new(ps.len())?;
        for (q, &p) in ps.iter().enumerate() {
            if !p.is_finite() || p < -1.0 || p > 1.0 {
                return Err(PqcError::BadPhase(p));
            }
            sv.apply_ry(q, phase_to_theta(p))?;
        }
        Ok(sv)
    }

    // ------------------------------------------------------------------
    // Гейты
    // ------------------------------------------------------------------

    /// R_y(θ) = [[cos(θ/2), −sin(θ/2)], [sin(θ/2), cos(θ/2)]].
    pub fn apply_ry(&mut self, q: usize, theta: f64) -> Result<()> {
        let h = 0.5 * theta;
        let (s, c) = (h.sin(), h.cos());
        self.apply_gate2_real(q, [c, -s, s, c])
    }

    /// R_x(θ) = [[cos(θ/2), −i·sin(θ/2)], [−i·sin(θ/2), cos(θ/2)]].
    pub fn apply_rx(&mut self, q: usize, theta: f64) -> Result<()> {
        let h = 0.5 * theta;
        let (s, c) = (h.sin(), h.cos());
        let ms = Cx::new(0.0, -s);
        self.apply_gate2(q, [[Cx::new(c, 0.0), ms], [ms, Cx::new(c, 0.0)]])
    }

    /// R_z(θ) = diag(e^{−iθ/2}, e^{+iθ/2}).
    pub fn apply_rz(&mut self, q: usize, theta: f64) -> Result<()> {
        let h = 0.5 * theta;
        let (s, c) = (h.sin(), h.cos());
        self.apply_diag(q, Cx::new(c, -s), Cx::new(c, s))
    }

    /// Адамар.
    pub fn apply_h(&mut self, q: usize) -> Result<()> {
        let k = std::f64::consts::FRAC_1_SQRT_2;
        self.apply_gate2_real(q, [k, k, k, -k])
    }

    /// Паули X.
    pub fn apply_x(&mut self, q: usize) -> Result<()> {
        self.check_qubit(q)?;
        let half = 1usize << q;
        let block = half << 1;
        par_blocks_mut(&mut self.amps, block, move |slice| {
            for blk in slice.chunks_mut(block) {
                let (lo, hi) = blk.split_at_mut(half);
                for (a, b) in lo.iter_mut().zip(hi.iter_mut()) {
                    core::mem::swap(a, b);
                }
            }
        });
        Ok(())
    }

    /// Паули Y = [[0, −i], [i, 0]].
    pub fn apply_y(&mut self, q: usize) -> Result<()> {
        self.check_qubit(q)?;
        let half = 1usize << q;
        let block = half << 1;
        par_blocks_mut(&mut self.amps, block, move |slice| {
            for blk in slice.chunks_mut(block) {
                let (lo, hi) = blk.split_at_mut(half);
                for (a, b) in lo.iter_mut().zip(hi.iter_mut()) {
                    let (a0, b0) = (*a, *b);
                    // a' = −i·b ; b' = i·a
                    *a = Cx::new(b0.im, -b0.re);
                    *b = Cx::new(-a0.im, a0.re);
                }
            }
        });
        Ok(())
    }

    /// Паули Z.
    pub fn apply_z(&mut self, q: usize) -> Result<()> {
        self.apply_diag(q, Cx::ONE, -Cx::ONE)
    }

    /// CNOT: control → target.
    pub fn apply_cnot(&mut self, control: usize, target: usize) -> Result<()> {
        self.check_qubit(control)?;
        self.check_qubit(target)?;
        if control == target {
            return Err(PqcError::SameQubit { control, target });
        }
        let (bc, bt) = (1usize << control, 1usize << target);
        let block = 1usize << (control.max(target) + 1);
        par_blocks_mut(&mut self.amps, block, move |slice| {
            // Локальные индексы внутри блока несут те же биты control/target:
            // пары (i, i | bt) не покидают блок. CNOT стреляет при control = 1.
            for i in 0..slice.len() {
                if i & bc != 0 && i & bt == 0 {
                    slice.swap(i, i | bt);
                }
            }
        });
        Ok(())
    }

    /// CZ: фаза −1 на |11⟩ по паре (control, target).
    pub fn apply_cz(&mut self, control: usize, target: usize) -> Result<()> {
        self.check_qubit(control)?;
        self.check_qubit(target)?;
        if control == target {
            return Err(PqcError::SameQubit { control, target });
        }
        let (bc, bt) = (1usize << control, 1usize << target);
        let block = 1usize << (control.max(target) + 1);
        par_blocks_mut(&mut self.amps, block, move |slice| {
            for i in 0..slice.len() {
                if i & bc != 0 && i & bt != 0 {
                    slice[i] = -slice[i];
                }
            }
        });
        Ok(())
    }

    /// S = diag(1, i).
    pub fn apply_s(&mut self, q: usize) -> Result<()> {
        self.apply_diag(q, Cx::ONE, Cx::I)
    }

    /// S† = diag(1, −i).
    pub fn apply_sdg(&mut self, q: usize) -> Result<()> {
        self.apply_diag(q, Cx::ONE, -Cx::I)
    }

    /// T = diag(1, e^{iπ/4}).
    pub fn apply_t(&mut self, q: usize) -> Result<()> {
        self.apply_diag(q, Cx::ONE, Cx::cis(core::f64::consts::FRAC_PI_4))
    }

    /// T† = diag(1, e^{−iπ/4}).
    pub fn apply_tdg(&mut self, q: usize) -> Result<()> {
        self.apply_diag(q, Cx::ONE, Cx::cis(-core::f64::consts::FRAC_PI_4))
    }

    /// SWAP(a, b): перестановка амплитуд с разными битами a/b.
    pub fn apply_swap(&mut self, a: usize, b: usize) -> Result<()> {
        self.check_qubit(a)?;
        self.check_qubit(b)?;
        if a == b {
            return Err(PqcError::SameQubit { control: a, target: b });
        }
        let (ba, bb) = (1usize << a, 1usize << b);
        let block = 1usize << (a.max(b) + 1);
        par_blocks_mut(&mut self.amps, block, move |slice| {
            for i in 0..slice.len() {
                // Стреляем с бита a = 1, b = 0: пара (i, i^ba^bb) встречается один раз.
                if i & ba != 0 && i & bb == 0 {
                    slice.swap(i, i ^ ba ^ bb);
                }
            }
        });
        Ok(())
    }

    /// CCX (Тоффоли): X на target при обоих контрольных битах 1.
    pub fn apply_ccx(&mut self, c1: usize, c2: usize, target: usize) -> Result<()> {
        self.check_qubit(c1)?;
        self.check_qubit(c2)?;
        self.check_qubit(target)?;
        if c1 == c2 || c1 == target || c2 == target {
            return Err(PqcError::BadCcxCollision);
        }
        let (b1, b2, bt) = (1usize << c1, 1usize << c2, 1usize << target);
        let block = 1usize << (c1.max(c2).max(target) + 1);
        par_blocks_mut(&mut self.amps, block, move |slice| {
            for i in 0..slice.len() {
                if i & b1 != 0 && i & b2 != 0 && i & bt == 0 {
                    slice.swap(i, i | bt);
                }
            }
        });
        Ok(())
    }

    /// CP(θ) = diag(1, 1, 1, e^{iθ}) — контролируемая фаза.
    pub fn apply_cp(&mut self, control: usize, target: usize, theta: f64) -> Result<()> {
        self.check_qubit(control)?;
        self.check_qubit(target)?;
        if control == target {
            return Err(PqcError::SameQubit { control, target });
        }
        let phase = Cx::cis(theta);
        let (bc, bt) = (1usize << control, 1usize << target);
        let block = 1usize << (control.max(target) + 1);
        par_blocks_mut(&mut self.amps, block, move |slice| {
            for i in 0..slice.len() {
                if i & bc != 0 && i & bt != 0 {
                    slice[i] *= phase;
                }
            }
        });
        Ok(())
    }

    /// Диспетчер именованных гейтов.
    pub fn apply(&mut self, gate: Gate) -> Result<()> {
        match gate {
            Gate::Ry { q, theta } => self.apply_ry(q, theta),
            Gate::Rx { q, theta } => self.apply_rx(q, theta),
            Gate::Rz { q, theta } => self.apply_rz(q, theta),
            Gate::H { q } => self.apply_h(q),
            Gate::X { q } => self.apply_x(q),
            Gate::Y { q } => self.apply_y(q),
            Gate::Z { q } => self.apply_z(q),
            Gate::Cx { control, target } => self.apply_cnot(control, target),
            Gate::Cz { control, target } => self.apply_cz(control, target),
            Gate::S { q } => self.apply_s(q),
            Gate::T { q } => self.apply_t(q),
            Gate::Sdg { q } => self.apply_sdg(q),
            Gate::Tdg { q } => self.apply_tdg(q),
            Gate::Swap { a, b } => self.apply_swap(a, b),
            Gate::Ccx { c1, c2, target } => self.apply_ccx(c1, c2, target),
            Gate::Cp {
                control,
                target,
                theta,
            } => self.apply_cp(control, target, theta),
            Gate::U2 { q, ref m } => self.apply_gate2(q, *m),
        }
    }

    /// Последовательность гейтов.
    pub fn apply_seq(&mut self, gates: impl IntoIterator<Item = Gate>) -> Result<()> {
        for g in gates {
            self.apply(g)?;
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Низкоуровневые ядра
    // ------------------------------------------------------------------

    /// Вещественный однокубитный гейт `m = [m00, m01, m10, m11]`.
    ///
    /// Ядро: два непрерывных потока (младшая/старшая половина блока 2^(q+1)),
    /// четыре вещественных умножения-сложения на пару — авто-векторизация.
    pub fn apply_gate2_real(&mut self, q: usize, m: [f64; 4]) -> Result<()> {
        self.check_qubit(q)?;
        let half = 1usize << q;
        let block = half << 1;
        par_blocks_mut(&mut self.amps, block, move |slice| {
            for blk in slice.chunks_mut(block) {
                let (lo, hi) = blk.split_at_mut(half);
                for (a, b) in lo.iter_mut().zip(hi.iter_mut()) {
                    let (a0, b0) = (*a, *b);
                    a.re = m[0] * a0.re + m[1] * b0.re;
                    a.im = m[0] * a0.im + m[1] * b0.im;
                    b.re = m[2] * a0.re + m[3] * b0.re;
                    b.im = m[2] * a0.im + m[3] * b0.im;
                }
            }
        });
        Ok(())
    }

    /// Комплексный однокубитный гейт 2×2.
    pub fn apply_gate2(&mut self, q: usize, m: [[Cx; 2]; 2]) -> Result<()> {
        self.check_qubit(q)?;
        let half = 1usize << q;
        let block = half << 1;
        par_blocks_mut(&mut self.amps, block, move |slice| {
            for blk in slice.chunks_mut(block) {
                let (lo, hi) = blk.split_at_mut(half);
                for (a, b) in lo.iter_mut().zip(hi.iter_mut()) {
                    let (a0, b0) = (*a, *b);
                    *a = m[0][0] * a0 + m[0][1] * b0;
                    *b = m[1][0] * a0 + m[1][1] * b0;
                }
            }
        });
        Ok(())
    }

    /// Диагональный однокубитный гейт: `d0` на бите 0, `d1` на бите 1.
    pub fn apply_diag(&mut self, q: usize, d0: Cx, d1: Cx) -> Result<()> {
        self.check_qubit(q)?;
        let half = 1usize << q;
        let block = half << 1;
        par_blocks_mut(&mut self.amps, block, move |slice| {
            for blk in slice.chunks_mut(block) {
                let (lo, hi) = blk.split_at_mut(half);
                for (a, b) in lo.iter_mut().zip(hi.iter_mut()) {
                    *a *= d0;
                    *b *= d1;
                }
            }
        });
        Ok(())
    }

    // ------------------------------------------------------------------
    // Измерения
    // ------------------------------------------------------------------

    /// |⟨outcome|ψ⟩|² для одного базисного состояния.
    pub fn probability(&self, outcome: usize) -> f64 {
        self.amps[outcome].norm_sq()
    }

    /// Полное распределение Борна |⟨x|ψ⟩|² (параллельно на больших размерах).
    pub fn probabilities(&self) -> Vec<f64> {
        let mut probs = vec![0.0_f64; self.amps.len()];
        let amps = &self.amps;
        par_zip_map(amps, &mut probs, |inp, out| {
            for (p, a) in out.iter_mut().zip(inp.iter()) {
                *p = a.norm_sq();
            }
        });
        probs
    }

    /// ‖ψ‖.
    pub fn norm(&self) -> f64 {
        self.amps.iter().map(|a| a.norm_sq()).sum::<f64>().sqrt()
    }

    /// Нормировка in-place; возвращает прежнюю норму.
    pub fn normalize(&mut self) -> Result<f64> {
        let n = self.norm();
        if n == 0.0 || !n.is_finite() {
            return Err(PqcError::NotNormalized { norm: n });
        }
        let k = 1.0 / n;
        for a in &mut self.amps {
            *a = a.scale(k);
        }
        Ok(n)
    }

    /// Маргиналы P(b_q = 1) по всем кубитам.
    pub fn marginals(&self) -> Vec<f64> {
        let n = self.n_qubits;
        let mut m = vec![0.0_f64; n];
        for (i, a) in self.amps.iter().enumerate() {
            let w = a.norm_sq();
            if w == 0.0 {
                continue;
            }
            for (q, mq) in m.iter_mut().enumerate() {
                if i & (1usize << q) != 0 {
                    *mq += w;
                }
            }
        }
        m
    }

    /// ⟨Z_q⟩ = P(b_q = 0) − P(b_q = 1). Для анзаца из фаз — сам трек:
    /// `from_phases(p).expect_z(q) == p_q` (обратная проверка кодирования).
    pub fn expect_z(&self, q: usize) -> Result<f64> {
        self.check_qubit(q)?;
        let bit = 1usize << q;
        let mut e = 0.0;
        for (i, a) in self.amps.iter().enumerate() {
            let w = a.norm_sq();
            if w == 0.0 {
                continue;
            }
            e += if i & bit == 0 { w } else { -w };
        }
        Ok(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-12;

    #[test]
    fn ground_state() {
        let sv = Statevector::new(3).unwrap();
        assert_eq!(sv.dim(), 8);
        assert_eq!(sv.amplitudes()[0], Cx::ONE);
        assert!(sv.amplitudes()[1..].iter().all(|a| *a == Cx::ZERO));
    }

    #[test]
    fn rejects_empty_and_huge() {
        assert!(matches!(Statevector::new(0), Err(PqcError::EmptyState)));
        assert!(matches!(
            Statevector::new(27),
            Err(PqcError::TooManyQubits { requested: 27, .. })
        ));
    }

    #[test]
    fn probabilities_sum_to_one() {
        let sv = Statevector::from_phases(&[0.3, -0.7, 0.5]).unwrap();
        let p = sv.probabilities();
        assert_eq!(p.len(), 8);
        assert!((p.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn expect_z_roundtrips_phase() {
        let ps = [0.25, -0.6, 0.9, -1.0];
        let sv = Statevector::from_phases(&ps).unwrap();
        for (q, &p) in ps.iter().enumerate() {
            assert!((sv.expect_z(q).unwrap() - p).abs() < TOL);
        }
    }

    #[test]
    fn normalize_rescales() {
        let mut sv = Statevector::from_phases(&[0.0, 0.0]).unwrap();
        for a in sv.amplitudes_mut() {
            *a = a.scale(2.0);
        }
        let old = sv.normalize().unwrap();
        assert!((old - 2.0).abs() < 1e-12);
        assert!((sv.norm() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn bad_qubit_rejected() {
        let mut sv = Statevector::new(2).unwrap();
        assert!(matches!(
            sv.apply_ry(2, 1.0),
            Err(PqcError::BadQubit { q: 2, n_qubits: 2 })
        ));
        assert!(sv.apply_cnot(0, 0).is_err());
    }
}
