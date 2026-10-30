//! # Шумовые модели физических кубитов: «идеал vs железо»
//!
//! Идеальный субстрат (`qpc`) даёт точные Борн-вероятности — ноль ошибок
//! гейтов, зчитывания и декогеренции. Физические машины платят на каждом
//! шаге. Этот модуль — калиброванный по железу контраст: те же схемы
//! прогоняются через стохастические траектории (Monte Carlo wavefunction)
//! с каналами:
//!
//! * **Деполяризация** после гейтов: E(ρ) = (1−p)ρ + p/3·(XρX + YρY + ZρZ)
//!   — траектория сэмплирует Паули (I, X, Y, Z) с (1−p, p/3, p/3, p/3).
//! * **Амплитудное затухание** (T1): точный MCWF — прыжок K₁ с вероятностью
//!   γ·⟨1_q⟩, иначе K₀-эволюция с ренормировкой; γ = 1 − e^(−t/T1).
//! * **Фазовое затухание** (Tφ): Z-ошибка с p_φ = (1 − e^(−t/Tφ))/2,
//!   1/Tφ = 1/T2 − 1/(2T1).
//! * **Чтение**: симметричный бит-флип с p_read.
//!
//! Оракулы-привилегии (flipphase/flipindex/flipzero/prep) шумом не
//! поражаются — это синтез владельца вектора состояния, не физический
//! гейт (документированная граница модели).
//!
//! Пресеты железа (публичные листы ошибок, порядок величин):
//! * `ibm-heron`: 1Q ~2.5e-4, 2Q ~8e-3, readout ~1.5e-2, T1/T2 ~200/100 мкс
//! * `google-willow`: 1Q ~1.5e-3, 2Q ~4e-3, readout ~1e-2
//! * `noisy-90s`: демо-масштаб ошибок 1990-х (1e-2 / 6e-2 / 5e-2)

use crate::error::Result;
use crate::gates::Gate;
use crate::qpc::{Circuit, Op};
use crate::rng::Rng;
use crate::statevector::Statevector;

/// Калибровка шума.
#[derive(Clone, Debug)]
pub struct NoiseModel {
    /// Деполяризация после 1-кубитного гейта.
    pub p1q: f64,
    /// Деполяризация после 2-кубитного гейта.
    pub p2q: f64,
    /// Симметричная ошибка чтения.
    pub p_read: f64,
    /// Время релаксации T1, мкс (амплитудное затухание).
    pub t1_us: f64,
    /// Время декогеренции T2, мкс (фазовое затухание).
    pub t2_us: f64,
    /// Длительность 1Q-гейта, нс.
    pub gate1_ns: f64,
    /// Длительность 2Q-гейта, нс.
    pub gate2_ns: f64,
}

impl Default for NoiseModel {
    fn default() -> Self {
        NoiseModel::ideal()
    }
}

impl NoiseModel {
    /// Идеальный субстрат: ноль шума (контраст «идеал vs железо»).
    pub fn ideal() -> NoiseModel {
        NoiseModel {
            p1q: 0.0,
            p2q: 0.0,
            p_read: 0.0,
            t1_us: f64::INFINITY,
            t2_us: f64::INFINITY,
            gate1_ns: 0.0,
            gate2_ns: 0.0,
        }
    }

    /// Пресеты физического железа (порядки величин из публичных листов).
    pub fn preset(name: &str) -> Option<NoiseModel> {
        Some(match name {
            "ideal" => NoiseModel::ideal(),
            "ibm-heron" => NoiseModel {
                p1q: 2.5e-4,
                p2q: 8.0e-3,
                p_read: 1.5e-2,
                t1_us: 200.0,
                t2_us: 100.0,
                gate1_ns: 50.0,
                gate2_ns: 300.0,
            },
            "google-willow" => NoiseModel {
                p1q: 1.5e-3,
                p2q: 4.0e-3,
                p_read: 1.0e-2,
                t1_us: 100.0,
                t2_us: 80.0,
                gate1_ns: 25.0,
                gate2_ns: 40.0,
            },
            "noisy-90s" => NoiseModel {
                p1q: 1.0e-2,
                p2q: 6.0e-2,
                p_read: 5.0e-2,
                t1_us: 20.0,
                t2_us: 10.0,
                gate1_ns: 100.0,
                gate2_ns: 1000.0,
            },
            _ => return None,
        })
    }

    fn gamma_damp(&self, two_qubit: bool) -> f64 {
        let t_ns = if two_qubit { self.gate2_ns } else { self.gate1_ns };
        if !self.t1_us.is_finite() || self.t1_us <= 0.0 || t_ns <= 0.0 {
            return 0.0;
        }
        1.0 - (-t_ns / (self.t1_us * 1e3)).exp()
    }

    fn p_phi(&self, two_qubit: bool) -> f64 {
        let t_ns = if two_qubit { self.gate2_ns } else { self.gate1_ns };
        if !self.t2_us.is_finite() || self.t2_us <= 0.0 || t_ns <= 0.0 {
            return 0.0;
        }
        // 1/Tφ = 1/T2 − 1/(2T1); Tφ в мкс
        let tphi_inv_us = 1.0 / self.t2_us
            - if self.t1_us.is_finite() && self.t1_us > 0.0 {
                1.0 / (2.0 * self.t1_us)
            } else {
                0.0
            };
        if tphi_inv_us <= 0.0 {
            return 0.0;
        }
        let tphi_ns = 1e3 / tphi_inv_us;
        0.5 * (1.0 - (-t_ns / tphi_ns).exp())
    }
}

/// Отчёт «идеал vs железо».
#[derive(Clone, Debug)]
pub struct NoisyReport {
    /// Идеальные вероятности (эталонный прогон без шума).
    pub ideal_probs: Vec<f64>,
    /// Зашумлённые вероятности (ансамбль траекторий).
    pub noisy_probs: Vec<f64>,
    /// Гистограмма зашумлённых исходов.
    pub counts: Vec<(u64, u64)>,
    /// Число выстрелов.
    pub shots: u64,
    /// Total variation distance ½·Σ|p_ideal − p_noisy|.
    pub tvd: f64,
    /// Классическая точность воспроизведения: (Σ√(p·q))².
    pub classical_fidelity: f64,
    /// Пиковая вероятность идеала (метрика успеха).
    pub ideal_peak: f64,
    /// Пиковая вероятность железа.
    pub noisy_peak: f64,
    /// Модель шума.
    pub model: NoiseModel,
    /// Число кубитов.
    pub n_qubits: usize,
    /// χ² Пирсона между идеалом и наблюдённым железом
    /// (бункы с ожиданием ≥ 4; цикл Q).
    pub chi2: f64,
    /// Степени свободы χ² (учтённые бунки − 1).
    pub chi2_dof: usize,
}

fn apply_pauli(sv: &mut Statevector, q: usize, which: u8) -> Result<()> {
    let g = match which {
        1 => Gate::X { q },
        2 => Gate::Y { q },
        3 => Gate::Z { q },
        _ => return Ok(()), // I
    };
    sv.apply(g)
}

/// Деполяризация на кубите q: сэмпл Паули.
fn depolarize(sv: &mut Statevector, q: usize, p: f64, rng: &mut Rng) -> Result<()> {
    if p <= 0.0 {
        return Ok(());
    }
    let u = rng.next_f64();
    if u < p {
        // равномерно X/Y/Z
        let w = rng.next_u64() % 3;
        apply_pauli(sv, q, w as u8 + 1)
    } else {
        Ok(())
    }
}

/// Амплитудное затухание на кубите q (точный MCWF-шаг).
fn amplitude_damp(sv: &mut Statevector, q: usize, gamma: f64, rng: &mut Rng) -> Result<()> {
    if gamma <= 0.0 {
        return Ok(());
    }
    let mask = 1usize << q;
    let p1: f64 = sv
        .amplitudes()
        .iter()
        .enumerate()
        .map(|(i, a)| if i & mask != 0 { a.norm_sq() } else { 0.0 })
        .sum();
    if p1 <= 0.0 {
        return Ok(()); // нечего релаксировать
    }
    let jump_prob = gamma * p1;
    let u = rng.next_f64();
    if u < jump_prob {
        // прыжок K₁: |1⟩→|0⟩ на кубите q, ренормировка
        let dim = sv.amplitudes().len();
        let mut moved = vec![crate::complex::Cx::ZERO; dim];
        let amps = sv.amplitudes_mut();
        for i in 0..dim {
            if i & mask != 0 {
                moved[i ^ mask] = amps[i];
                amps[i] = crate::complex::Cx::ZERO;
            }
        }
        for i in 0..dim {
            amps[i] = moved[i];
        }
        sv.normalize()?;
    } else {
        // K₀-эволюция: |1⟩-компоненты × √(1−γ) — ГЛОБАЛЬНАЯ нормировка
        // выживания √(1−γ·p₁) выполняется normalize() автоматически.
        // ⚠ Встраивать 1/√(1−γp₁) только в |1⟩-строки НЕЛЬЗЯ — это
        // искажает отношение амплитуд (баг найден сверкой с qiskit-Kraus).
        let scale = (1.0 - gamma).sqrt();
        let dim = sv.amplitudes().len();
        let amps = sv.amplitudes_mut();
        for i in 0..dim {
            if i & mask != 0 {
                amps[i] = amps[i].scale(scale);
            }
        }
        sv.normalize()?;
    }
    Ok(())
}

/// Z-ошибка фазового затухания.
fn phase_damp(sv: &mut Statevector, q: usize, p_phi: f64, rng: &mut Rng) -> Result<()> {
    if p_phi <= 0.0 {
        return Ok(());
    }
    if rng.next_f64() < p_phi {
        sv.apply(Gate::Z { q })?;
    }
    Ok(())
}

/// χ²-статистика Пирсона между идеальным распределением и наблюдёнными
/// счётчиками: Σ_x (o_x − N·p_x)² / (N·p_x) по бункам с ожиданием
/// N·p_x ≥ 4 (классическое правило достаточного ожидания).
/// Возвращает (χ², dof). Цикл Q: шум поверх верификатора.
pub fn chi2_stat(ideal_probs: &[f64], counts: &[(u64, u64)], shots: u64) -> (f64, usize) {
    if shots == 0 {
        return (0.0, 0);
    }
    let mut chi2 = 0.0f64;
    let mut bins = 0usize;
    // ожидание по идеалу для каждого наблюдённого исхода
    let observed = |x: u64| -> f64 {
        counts
            .iter()
            .find(|&&(o, _)| o == x)
            .map(|&(_, c)| c as f64)
            .unwrap_or(0.0)
    };
    let dim = ideal_probs.len();
    for x in 0..dim {
        let e = ideal_probs[x] * shots as f64;
        if e >= 4.0 {
            let o = observed(x as u64);
            chi2 += (o - e) * (o - e) / e;
            bins += 1;
        }
    }
    (chi2, bins.saturating_sub(1))
}

/// Прогнать схему на зашумлённом железе. Оракулы-привилегии
/// (flip*/prep) применяются точно; физические гейты — с каналами.
pub fn run_noisy(
    circuit: &Circuit,
    model: &NoiseModel,
    shots: u64,
    seed: u64,
) -> Result<NoisyReport> {
    // Эталон: идеальный прогон (быстрый путь).
    let ideal = crate::qpc::run(circuit, 0, seed)?;
    let ideal_probs = ideal.probabilities.clone();

    let n = circuit.n_qubits();
    let mut rng = Rng::seed_from_u64(seed ^ 0x5EED_0000_0000_0001);
    let mut counts: std::collections::BTreeMap<u64, u64> = std::collections::BTreeMap::new();
    let per_shot = circuit.has_mid_circuit_measure();

    for _ in 0..shots {
        let mut sv = Statevector::new(n)?;
        let mut terminal_sampled = false;
        for op in circuit.ops() {
            match op {
                Op::Gate(g) => {
                    sv.apply(*g)?;
                    let (two_q, qubits): (bool, Vec<usize>) = match g {
                        Gate::Cx { control, target } => (true, vec![*control, *target]),
                        Gate::Cz { control, target } => (true, vec![*control, *target]),
                        Gate::Swap { a, b } => (true, vec![*a, *b]),
                        Gate::Cp { control, target, .. } => (true, vec![*control, *target]),
                        Gate::Ccx { c1, c2, target } => (true, vec![*c1, *c2, *target]),
                        Gate::H { q }
                        | Gate::X { q }
                        | Gate::Y { q }
                        | Gate::Z { q }
                        | Gate::S { q }
                        | Gate::Sdg { q }
                        | Gate::T { q }
                        | Gate::Tdg { q }
                        | Gate::Ry { q, .. }
                        | Gate::Rx { q, .. }
                        | Gate::Rz { q, .. }
                        | Gate::U2 { q, .. } => (false, vec![*q]),
                    };
                    let p_dep = if two_q { model.p2q } else { model.p1q };
                    let gamma = model.gamma_damp(two_q);
                    let pphi = model.p_phi(two_q);
                    for &q in &qubits {
                        depolarize(&mut sv, q, p_dep, &mut rng)?;
                        amplitude_damp(&mut sv, q, gamma, &mut rng)?;
                        phase_damp(&mut sv, q, pphi, &mut rng)?;
                    }
                }
                Op::FlipPhase { .. } | Op::FlipIndex { .. } | Op::FlipZero | Op::PrepComb { .. } => {
                    // привилегия владельца — без шума
                    let slice = [op.clone()];
                    Circuit::apply_unitary(&mut sv, &slice)?;
                }
                Op::Barrier => {}
                Op::Measure { .. } | Op::MeasureAll => {
                    if per_shot {
                        // честный коллапс + чтение
                        let qs: Vec<usize> = match op {
                            Op::Measure { q } => vec![*q],
                            _ => (0..n).collect(),
                        };
                        for q in qs {
                            let p1: f64 = sv
                                .amplitudes()
                                .iter()
                                .enumerate()
                                .map(|(i, a)| if i >> q & 1 == 1 { a.norm_sq() } else { 0.0 })
                                .sum();
                            let bit = if rng.next_f64() < p1 { 1usize } else { 0 };
                            Circuit::collapse(&mut sv, q, bit)?;
                        }
                    }
                    terminal_sampled = true;
                }
            }
        }
        // терминальный Борн-семпл (+ чтение с флипом)
        let outcome = if terminal_sampled || per_shot {
            let sampler = crate::born::BornSampler::new(&sv)?;
            
            sampler.sample(&mut rng)
        } else {
            let sampler = crate::born::BornSampler::new(&sv)?;
            sampler.sample(&mut rng)
        };
        let mut noisy_bits = outcome;
        if model.p_read > 0.0 {
            for q in 0..n {
                if rng.next_f64() < model.p_read {
                    noisy_bits ^= 1u64 << q;
                }
            }
        }
        *counts.entry(noisy_bits).or_insert(0) += 1;
        let _ = terminal_sampled;
    }

    let dim = ideal_probs.len();
    let mut noisy_probs = vec![0.0f64; dim];
    for (o, c) in &counts {
        noisy_probs[*o as usize] = *c as f64 / shots as f64;
    }
    let tvd = 0.5
        * ideal_probs
            .iter()
            .zip(&noisy_probs)
            .map(|(a, b)| (a - b).abs())
            .sum::<f64>();
    let bc: f64 = ideal_probs
        .iter()
        .zip(&noisy_probs)
        .map(|(a, b)| (a * b).sqrt())
        .sum();
    let counts: Vec<(u64, u64)> = counts.into_iter().collect();
    let (chi2, chi2_dof) = chi2_stat(&ideal_probs, &counts, shots);
    Ok(NoisyReport {
        ideal_peak: ideal_probs.iter().cloned().fold(0.0, f64::max),
        noisy_peak: noisy_probs.iter().cloned().fold(0.0, f64::max),
        classical_fidelity: bc * bc,
        counts,
        ideal_probs,
        noisy_probs,
        shots,
        tvd,
        model: model.clone(),
        n_qubits: n,
        chi2,
        chi2_dof,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithms as alg;

    #[test]
    fn zero_noise_bitwise_ideal() {
        // p=0: зашумлённый прогон (а) детерминирован и (б) совпадает
        // с идеальным в пределах биномиального шума сэмплинга.
        let (c, _) = alg::grover(6, &[22]).unwrap();
        let model = NoiseModel::ideal();
        let rep1 = run_noisy(&c, &model, 2000, 42).unwrap();
        let rep2 = run_noisy(&c, &model, 2000, 42).unwrap();
        assert_eq!(rep1.counts, rep2.counts, "детерминизм: сид → гистограмма");
        // TVD против ТОЧНОГО идеала ограничен биномиальным шумом:
        // E[TVD] ≲ √(K/(2·shots)) для K ненулевых исходов.
        let k = rep1.ideal_probs.iter().filter(|p| **p > 1e-9).count();
        let bound = 8.0 * (k as f64 / (2.0 * 2000.0)).sqrt();
        assert!(rep1.tvd < bound, "TVD = {} > биномиальный предел {}", rep1.tvd, bound);
        assert!(rep1.ideal_peak > 0.99);
    }

    #[test]
    fn amplitude_damping_decays_excitation() {
        // |1⟩ с затуханием γ: P(1) = 1−γ (одна траектория детерминирована
        // в среднем: прыжок с вероятностью γ).
        let gamma = 0.3;
        let model = NoiseModel {
            p1q: 0.0,
            p2q: 0.0,
            p_read: 0.0,
            t1_us: 1.0,
            t2_us: f64::INFINITY,
            gate1_ns: 356.7, // γ = 1 − e^{−0.3567/1·1000·1e-3}… подберём ниже
            gate2_ns: 0.0,
        };
        // вычислим фактическую γ модели
        let gamma_actual = model.gamma_damp(false);
        let mut c = Circuit::new(1).unwrap();
        c.gate(Gate::X { q: 0 });
        c.push(Op::MeasureAll);
        let shots = 20_000u64;
        let rep = run_noisy(&c, &model, shots, 7).unwrap();
        let p1_noisy: f64 = rep
            .noisy_probs
            .iter()
            .enumerate()
            .map(|(i, p)| if i & 1 == 1 { *p } else { 0.0 })
            .sum();
        let want = 1.0 - gamma_actual;
        let sigma = (want * (1.0 - want) / shots as f64).sqrt();
        assert!(
            (p1_noisy - want).abs() < 5.0 * sigma,
            "P(1) = {p1_noisy}, хочу {want} (γ={gamma_actual}, модель γ={gamma})"
        );
        // Идеал: P(1) = 1 точно.
        let p1_ideal: f64 = rep
            .ideal_probs
            .iter()
            .enumerate()
            .map(|(i, p)| if i & 1 == 1 { *p } else { 0.0 })
            .sum();
        assert!((p1_ideal - 1.0).abs() < 1e-12, "идеал не портится");
    }

    #[test]
    fn depolarizing_ensemble_matches_exact_channel() {
        // Bell: H → CX → depol(p) на ОБОИХ кубитах (шум после 2Q-гейта).
        // Пары {XX,YY,ZZ} сохраняют носитель, {Z} одиночные тоже,
        // {X,Y} и смешанные пары уводят из 00:
        // P(00) = ½(1−p)² + p(1−p)/3 + p²/6 — сверяется с ансамблем.
        let p = 0.15f64;
        let model = NoiseModel {
            p1q: 0.0,
            p2q: p,
            p_read: 0.0,
            t1_us: f64::INFINITY,
            t2_us: f64::INFINITY,
            gate1_ns: 0.0,
            gate2_ns: 0.0,
        };
        let mut c = Circuit::new(2).unwrap();
        c.gate(Gate::H { q: 0 });
        c.gate(Gate::Cx {
            control: 0,
            target: 1,
        });
        c.push(Op::MeasureAll);
        let shots = 60_000u64;
        let rep = run_noisy(&c, &model, shots, 123).unwrap();
        let want = 0.5 * (1.0 - p).powi(2) + p * (1.0 - p) / 3.0 + p * p / 6.0;
        let sigma = (want * (1.0 - want) / shots as f64).sqrt();
        assert!(
            (rep.noisy_probs[0] - want).abs() < 6.0 * sigma,
            "P(00) = {}, точный канал {}",
            rep.noisy_probs[0],
            want
        );
        assert!((rep.ideal_probs[0] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn readout_flip_symmetric() {
        // P(0→1) = P(1→0) = p_read: измеряем |0…0⟩.
        let model = NoiseModel {
            p_read: 0.1,
            ..NoiseModel::ideal()
        };
        let c = alg::bell().unwrap();
        let shots = 20_000u64;
        let rep = run_noisy(&c, &model, shots, 5).unwrap();
        // Bell: идеал 00/11 по ½; с флипом: 01/10 появляются с весом
        // 2·p(1−p)·½ каждый…
        let p01 = rep.noisy_probs[1];
        let p10 = rep.noisy_probs[2];
        let want_single = 0.5 * 2.0 * 0.1 * 0.9;
        let sigma = (want_single * (1.0 - want_single) / shots as f64).sqrt();
        assert!((p01 - want_single).abs() < 6.0 * sigma, "p01 = {p01}");
        assert!((p10 - want_single).abs() < 6.0 * sigma, "p10 = {p10}");
    }

    #[test]
    fn grover_degrades_monotonically_with_noise() {
        // Монотонность по ОДНОЙ ручке (p2q) — физический закон;
        // разные пресеты железа НЕ обязаны упорядочиваться.
        let (c, _) = alg::grover(6, &[22]).unwrap();
        let mut prev_peak = 1.0;
        for p2q in [0.0, 0.004, 0.02, 0.1] {
            let model = NoiseModel {
                p2q,
                ..NoiseModel::ideal()
            };
            let rep = run_noisy(&c, &model, 8000, 42).unwrap();
            assert!(
                rep.noisy_peak <= prev_peak + 0.01,
                "p2q={p2q}: пик {} вырос после {}",
                rep.noisy_peak,
                prev_peak
            );
            prev_peak = rep.noisy_peak;
        }
        // Все пресеты железа деградируют относительно идеала.
        let ideal = run_noisy(&c, &NoiseModel::ideal(), 100, 42).unwrap();
        assert!(ideal.ideal_peak > 0.99);
        for preset in ["google-willow", "ibm-heron", "noisy-90s"] {
            let model = NoiseModel::preset(preset).unwrap();
            let rep = run_noisy(&c, &model, 8000, 42).unwrap();
            assert!(
                rep.noisy_peak < ideal.ideal_peak,
                "{preset}: пик {} не ниже идеала",
                rep.noisy_peak
            );
            assert!(rep.tvd > 0.01, "{preset}: TVD = {} — шума не видно", rep.tvd);
        }
    }
}
