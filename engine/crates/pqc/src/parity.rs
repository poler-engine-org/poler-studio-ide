//! RQ4: анзац паритета с Qiskit — точное зеркало
//! `poler_quantum.quantum.ansatz.PolerAnsatz` на Rust.
//!
//! Слои схемы (порядок в точности как в Python):
//!
//! 1. **Восприятие** — `R_y(θ_i)` на кубите `i`, `θ_i = arccos(clip(p_i))`;
//! 2. **Резонанс** — запутыватель соседей `(i → i+1)`: CX (по умолчанию)
//!    или CZ;
//! 3. **Стабилизация** — `R_z` на каждом кубите: mode B — константный
//!    `gamma`, mode C — адаптивная фаза `gamma·exp(−|sin(kappa·π)|)`.
//!
//! ## Конвенция битов
//!
//! Обе стороны используют little-endian индексацию qiskit: бит `i`
//! амплитуды `amps[k]` — это кубит `i`, `k = Σ b_i·2^i`. Big-endian
//! swap ловится зондами с асимметричными `p` (см. кросс-тест).
//!
//! ## Контракт фаз SplitMix64
//!
//! Векторы `p` для кросс-теста порождаются языко-агностичным потоком
//! SplitMix64: `x_k = splitmix64^k(seed)`, `p_k = (x_k >> 11)/2^52 − 1
//! ∈ [−1, 1)` — бит-в-бит одинаково в Rust и Python (старшие 53 бита
//! u64 точно конвертируются в f64, деление на степень двойки точно).

use crate::born::BornSampler;
use crate::entangle::Entangler;
use crate::error::{PqcError, Result};
use crate::rng::splitmix64;
use crate::rng::Rng;
use crate::statevector::{phase_to_theta, Statevector};

/// Режимы стабилизации (совпадают с Python `MODES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParityMode {
    /// Восприятие + резонанс (CX/CZ цепь), без R_z.
    A,
    /// Mode A + `R_z(gamma)` на каждом кубите.
    B,
    /// Mode A + `R_z(gamma·exp(−|sin(kappa·π)|))` — адаптивная фаза.
    C,
}

impl ParityMode {
    /// Имя режима в JSON-протоколе.
    pub fn name(self) -> &'static str {
        match self {
            ParityMode::A => "A",
            ParityMode::B => "B",
            ParityMode::C => "C",
        }
    }

    /// Разбор режима из имени протокола.
    pub fn from_name(s: &str) -> Option<ParityMode> {
        match s {
            "A" => Some(ParityMode::A),
            "B" => Some(ParityMode::B),
            "C" => Some(ParityMode::C),
            _ => None,
        }
    }
}

/// Анзац паритета: Rust-зеркало Qiskit-анзаца POLER.
#[derive(Clone, Copy, Debug)]
pub struct ParityAnsatz {
    n: usize,
    mode: ParityMode,
    ent: Entangler,
}

impl ParityAnsatz {
    /// Новый анзац: `n ≥ 1` кубитов, режим A/B/C, запутыватель CX/CZ.
    pub fn new(n: usize, mode: ParityMode, ent: Entangler) -> Result<ParityAnsatz> {
        if n == 0 {
            return Err(PqcError::EmptyState);
        }
        if n > crate::statevector::MAX_QUBITS {
            return Err(PqcError::TooManyQubits {
                requested: n,
                max: crate::statevector::MAX_QUBITS,
            });
        }
        Ok(ParityAnsatz { n, mode, ent })
    }

    /// Число кубитов.
    pub fn n_qubits(&self) -> usize {
        self.n
    }

    /// Режим стабилизации.
    pub fn mode(&self) -> ParityMode {
        self.mode
    }

    /// Запутыватель резонансного слоя.
    pub fn entangler(&self) -> Entangler {
        self.ent
    }

    /// Адаптивная фаза стабилизации (mode C):
    /// `phi = gamma · exp(−|sin(kappa·π)|)` — как в Python.
    pub fn stabilisation_phase(gamma: f64, kappa: f64) -> f64 {
        gamma * (-(kappa * core::f64::consts::PI).sin().abs()).exp()
    }

    /// Полная схема анзаца для фаз `p` (длина ровно `n`).
    ///
    /// Возвращает точный statevector: `amps[k]`, `k = Σ b_i·2^i`.
    pub fn circuit(&self, p: &[f64], gamma: f64, kappa: f64) -> Result<Statevector> {
        if p.len() != self.n {
            return Err(PqcError::LengthMismatch {
                expected: self.n,
                actual: p.len(),
            });
        }
        for &x in p {
            if !x.is_finite() || x < -1.0 || x > 1.0 {
                return Err(PqcError::BadPhase(x));
            }
        }

        // 1. Восприятие: R_y(arccos p).
        let mut sv = Statevector::new(self.n)?;
        for (i, &x) in p.iter().enumerate() {
            sv.apply_ry(i, phase_to_theta(x))?;
        }

        // 2. Резонанс: запутыватель соседей.
        for i in 0..self.n - 1 {
            match self.ent {
                Entangler::Cx => sv.apply_cnot(i, i + 1)?,
                Entangler::Cz => sv.apply_cz(i, i + 1)?,
            }
        }

        // 3. Стабилизация: R_z.
        match self.mode {
            ParityMode::A => {}
            ParityMode::B => {
                for i in 0..self.n {
                    sv.apply_rz(i, gamma)?;
                }
            }
            ParityMode::C => {
                let phi = Self::stabilisation_phase(gamma, kappa);
                for i in 0..self.n {
                    sv.apply_rz(i, phi)?;
                }
            }
        }
        Ok(sv)
    }

    /// Born-счётчики исходов `(k, count)` по схеме — наблюдения χ².
    pub fn sample_counts(
        &self,
        p: &[f64],
        gamma: f64,
        kappa: f64,
        rng: &mut Rng,
        shots: u64,
    ) -> Result<Vec<(u64, u64)>> {
        let sv = self.circuit(p, gamma, kappa)?;
        let sampler = BornSampler::new(&sv)?;
        Ok(sampler.sample_counts(rng, shots))
    }
}

/// Языко-агностичный вектор фаз из сида: `p_k = (x_k >> 11)/2^52 − 1`.
///
/// Старшие 53 бита SplitMix64-потока дают бит-точную конверсию в f64
/// в обоих языках; диапазон `[−1, 1)`.
pub fn splitmix_phases(seed: u64, n: usize) -> Vec<f64> {
    let mut state = seed;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let x = splitmix64(&mut state);
        out.push((x >> 11) as f64 / (1u64 << 52) as f64 - 1.0);
    }
    out
}

/// Детерминированный сид ГПСЧ выстрелов для конфигурации кросс-теста:
/// FNV-1a64 по канонической строке `<seed>-<n>-<mode>-<ent>`.
///
/// Режимы A/B/C дают одно и то же распределение Борна (R_z диагонален),
/// поэтому общий сид делал бы их χ²-статистики **полностью
/// коррелированными** — одна неудачная выборка роняла все три режима.
/// Независимый поток на конфигурацию делает статистики независимыми
/// и остаётся полностью детерминированным (воспроизводимость на
/// фиксированных сидах не страдает).
pub fn config_rng_seed(seed: u64, n: usize, mode: ParityMode, ent: Entangler) -> u64 {
    let id = format!("{seed}-{n}-{}-{}", mode.name(), ent.name());
    pqw::checksum::fnv1a64(id.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_names_roundtrip() {
        for m in [ParityMode::A, ParityMode::B, ParityMode::C] {
            assert_eq!(ParityMode::from_name(m.name()), Some(m));
        }
        assert_eq!(ParityMode::from_name("Z"), None);
    }

    #[test]
    fn rejects_zero_qubits() {
        assert!(ParityAnsatz::new(0, ParityMode::A, Entangler::Cx).is_err());
    }

    #[test]
    fn rejects_wrong_p_length() {
        let ans = ParityAnsatz::new(3, ParityMode::A, Entangler::Cx).unwrap();
        assert!(ans.circuit(&[0.1, 0.2], 0.5, 0.8).is_err());
        assert!(ans.circuit(&[0.1, 0.2, 1.5], 0.5, 0.8).is_err());
        assert!(ans.circuit(&[0.1, 0.2, 0.3], 0.5, 0.8).is_ok());
    }

    #[test]
    fn stabilisation_phase_formula() {
        let (gamma, kappa) = (0.7, 0.3);
        let expect = gamma * (-(kappa * core::f64::consts::PI).sin().abs()).exp();
        assert!((ParityAnsatz::stabilisation_phase(gamma, kappa) - expect).abs() < 1e-15);
        // Инвариант Python-теста: фаза меняется с kappa.
        assert!(
            ParityAnsatz::stabilisation_phase(0.5, 0.8)
                != ParityAnsatz::stabilisation_phase(0.5, 0.3)
        );
    }

    /// Зонд little-endian: p = [1, −1] (кубит 0 в |0⟩, кубит 1 в |1⟩),
    /// CX(0→1) не стреляет → амплитуда сосредоточена в индексе 2 = 0b10.
    /// При big-endian swap амплитуда ушла бы в индекс 1.
    #[test]
    fn le_probe_two_qubits() {
        let ans = ParityAnsatz::new(2, ParityMode::A, Entangler::Cx).unwrap();
        let sv = ans.circuit(&[1.0, -1.0], 0.5, 0.8).unwrap();
        let amps = sv.amplitudes();
        assert!((amps[2].re - 1.0).abs() < 1e-12 && amps[2].im.abs() < 1e-12);
        assert!(amps[1].norm_sq() < 1e-24);
        assert!(amps[0].norm_sq() + amps[3].norm_sq() < 1e-24);
    }

    /// Зонд little-endian на CZ: p = [−1, 1, 1] → |ψ⟩ = |1⟩₀, CZ не
    /// трогает populations → амплитуда в индексе 1 (big-endian дал бы 4).
    #[test]
    fn le_probe_three_qubits_cz() {
        let ans = ParityAnsatz::new(3, ParityMode::A, Entangler::Cz).unwrap();
        let sv = ans.circuit(&[-1.0, 1.0, 1.0], 0.5, 0.8).unwrap();
        let amps = sv.amplitudes();
        assert!((amps[1].re - 1.0).abs() < 1e-12 && amps[1].im.abs() < 1e-12);
        assert!(amps[4].norm_sq() < 1e-24);
    }

    /// R_z-слои (mode B/C) диагональны: вероятности Борна не отличаются
    /// от mode A при том же запутывателе, амплитуды отличаются фазами.
    #[test]
    fn rz_layers_preserve_born_probabilities() {
        let p = splitmix_phases(42, 4);
        let a = ParityAnsatz::new(4, ParityMode::A, Entangler::Cx)
            .unwrap()
            .circuit(&p, 0.5, 0.8)
            .unwrap();
        let b = ParityAnsatz::new(4, ParityMode::B, Entangler::Cx)
            .unwrap()
            .circuit(&p, 0.9, 0.8)
            .unwrap();
        let c = ParityAnsatz::new(4, ParityMode::C, Entangler::Cx)
            .unwrap()
            .circuit(&p, 0.9, 0.3)
            .unwrap();
        let (pa, pb, pc) = (a.probabilities(), b.probabilities(), c.probabilities());
        for k in 0..pa.len() {
            assert!((pa[k] - pb[k]).abs() < 1e-12, "B: k={k}");
            assert!((pa[k] - pc[k]).abs() < 1e-12, "C: k={k}");
        }
        // Фазы стабилизации реально меняют амплитуды.
        assert!(a.amplitudes() != b.amplitudes());
    }

    /// Хрестоматийный случай: p = [0, 1] — кубит 0 в суперпозиции,
    /// кубит 1 в |0⟩; CX(0→1) → состояние Белла (|00⟩ + |11⟩)/√2.
    #[test]
    fn bell_state_direction() {
        let ans = ParityAnsatz::new(2, ParityMode::A, Entangler::Cx).unwrap();
        let sv = ans.circuit(&[0.0, 1.0], 0.5, 0.8).unwrap();
        let amps = sv.amplitudes();
        let inv = std::f64::consts::FRAC_1_SQRT_2;
        assert!((amps[0].re - inv).abs() < 1e-12);
        assert!((amps[3].re - inv).abs() < 1e-12);
        assert!(amps[1].norm_sq() < 1e-24 && amps[2].norm_sq() < 1e-24);
    }

    #[test]
    fn splitmix_phases_deterministic_and_bounded() {
        let a = splitmix_phases(1337, 8);
        let b = splitmix_phases(1337, 8);
        assert_eq!(a, b);
        assert_eq!(a.len(), 8);
        assert!(a.iter().all(|&x| (-1.0..1.0).contains(&x)));
        // Разные сиды — разные потоки.
        assert_ne!(a, splitmix_phases(2026, 8));
        // Снимок первых значений — бит-точность контракта с Python
        // (эталон: splitmix64-поток, p = (x >> 11)/2^52 − 1).
        let s = splitmix_phases(42, 2);
        assert_eq!(s[0].to_bits(), 0x3FDEEB99_1317F5B4);
        assert_eq!(s[1].to_bits(), 0xBFE5C407_33136644);
    }

    /// Сиды выстрелов: детерминизм + независимость режимов A/B/C.
    #[test]
    fn config_rng_seed_independent_across_modes() {
        let a = config_rng_seed(1337, 4, ParityMode::A, Entangler::Cz);
        let b = config_rng_seed(1337, 4, ParityMode::B, Entangler::Cz);
        let c = config_rng_seed(1337, 4, ParityMode::C, Entangler::Cz);
        assert_eq!(a, config_rng_seed(1337, 4, ParityMode::A, Entangler::Cz));
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
        assert_ne!(a, config_rng_seed(1337, 4, ParityMode::A, Entangler::Cx));
    }

    /// Born-счётчики: сумма равна числу выстрелов, исходы допустимы.
    #[test]
    fn sample_counts_sum_to_shots() {
        let p = splitmix_phases(7, 3);
        let ans = ParityAnsatz::new(3, ParityMode::B, Entangler::Cz).unwrap();
        let mut rng = Rng::seed_from_u64(11);
        let counts = ans.sample_counts(&p, 0.5, 0.8, &mut rng, 10_000).unwrap();
        let total: u64 = counts.iter().map(|(_, c)| c).sum();
        assert_eq!(total, 10_000);
        assert!(counts.iter().all(|(k, _)| (*k as usize) < 8));
    }
}
