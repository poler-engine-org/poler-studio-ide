#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Статистическая корректность Born-сэмплирования: фиксированные семена
//! делают все допуски детерминированными.

use pqc::{BornSampler, PhaseAnsatz, Rng, Statevector};

/// Порог в сигмах для детерминированных статистических проверок.
const SIG: f64 = 5.0;

#[test]
fn same_seed_same_shots() {
    let sv = Statevector::from_phases(&[0.3, -0.4, 0.9]).unwrap();
    let s = BornSampler::new(&sv).unwrap();
    let mut a = Rng::seed_from_u64(42);
    let mut b = Rng::seed_from_u64(42);
    assert_eq!(s.sample_n(&mut a, 1000), s.sample_n(&mut b, 1000));
}

#[test]
fn different_seeds_diverge() {
    let sv = Statevector::from_phases(&[0.0, 0.0]).unwrap();
    let s = BornSampler::new(&sv).unwrap();
    let mut a = Rng::seed_from_u64(1);
    let mut b = Rng::seed_from_u64(2);
    let diff = (0..1000)
        .filter(|_| s.sample(&mut a) != s.sample(&mut b))
        .count();
    assert!(diff > 600, "overlap {diff}/1000");
}

#[test]
fn poles_give_deterministic_outcome() {
    // p = [+1, −1]: единственный исход 0b10.
    let sv = Statevector::from_phases(&[1.0, -1.0]).unwrap();
    let s = BornSampler::new(&sv).unwrap();
    let mut rng = Rng::seed_from_u64(5);
    for _ in 0..1000 {
        assert_eq!(s.sample(&mut rng), 0b10);
    }
}

#[test]
fn zero_probability_states_never_sampled() {
    // Чередование [1, −1, 1, −1] → единственный исход 0b1010.
    let sv = Statevector::from_phases(&[1.0, -1.0, 1.0, -1.0]).unwrap();
    let s = BornSampler::new(&sv).unwrap();
    let mut rng = Rng::seed_from_u64(9);
    for _ in 0..2000 {
        assert_eq!(s.sample(&mut rng), 0b1010);
    }
}

#[test]
fn uniform_frequencies_match_theory() {
    // p = 0 всюду → равномерное распределение на 16 исходах.
    let sv = Statevector::from_phases(&[0.0; 4]).unwrap();
    let s = BornSampler::new(&sv).unwrap();
    let shots = 40_000u64;
    let mut rng = Rng::seed_from_u64(42);
    let counts = s.sample_counts(&mut rng, shots);

    assert_eq!(counts.iter().map(|(_, c)| c).sum::<u64>(), shots);
    assert_eq!(counts.len(), 16);
    let e = shots as f64 / 16.0;
    for (outcome, c) in &counts {
        let dev = (*c as f64 - e).abs();
        assert!(
            dev < SIG * e.sqrt() + 2.0,
            "outcome {outcome:#x}: count {c}, expected {e:.1}"
        );
    }
}

#[test]
fn empirical_frequencies_match_analytic_product() {
    let ps = [0.3, -0.7, 0.55, -0.15, 0.85, -0.95];
    let sv = Statevector::from_phases(&ps).unwrap();
    let s = BornSampler::new(&sv).unwrap();
    let shots = 30_000u64;
    let mut rng = Rng::seed_from_u64(7);
    let counts = s.sample_counts(&mut rng, shots);

    let mut obs = vec![0u64; 64];
    for (o, c) in &counts {
        obs[*o as usize] += c;
    }
    for x in 0..64usize {
        let theory = s.outcome_probability(x as u64);
        let e = theory * shots as f64;
        if e >= 25.0 {
            let dev = (obs[x] as f64 - e).abs();
            assert!(
                dev < SIG * e.sqrt() + 2.0,
                "x = {x:#x}: obs {}, expected {e:.1} (theory {theory:.5})",
                obs[x]
            );
        } else {
            // Редкие исходы: суммарная хвостовая вероятность мала.
            assert!(obs[x] as f64 <= e + SIG * e.sqrt() + 5.0);
        }
    }
}

#[test]
fn counts_sorted_and_complete() {
    let sv = Statevector::from_phases(&[0.2, 0.4]).unwrap();
    let s = BornSampler::new(&sv).unwrap();
    let mut rng = Rng::seed_from_u64(3);
    let counts = s.sample_counts(&mut rng, 5_000);
    assert_eq!(counts.iter().map(|(_, c)| c).sum::<u64>(), 5_000);
    for w in counts.windows(2) {
        assert!(w[0].1 >= w[1].1, "not sorted: {w:?}");
    }
}

#[test]
fn top_k_is_prefix_of_counts() {
    let sv = Statevector::from_phases(&[0.1, -0.9, 0.6]).unwrap();
    let s = BornSampler::new(&sv).unwrap();
    // Два одинаково засеянных ГПСЧ: последовательности выстрелов идентичны.
    let (mut rng1, mut rng2) = (Rng::seed_from_u64(11), Rng::seed_from_u64(11));
    let counts = s.sample_counts(&mut rng1, 4_000);
    let top = s.top_k(&mut rng2, 4_000, 3);
    assert_eq!(top.len(), 3);
    assert_eq!(top[0], counts[0]);
    assert_eq!(top[1], counts[1]);
    assert_eq!(top[2], counts[2]);
}

#[test]
fn outcome_probability_matches_analytic() {
    let ps = [0.62, -0.31, 0.05];
    let sv = Statevector::from_phases(&ps).unwrap();
    let s = BornSampler::new(&sv).unwrap();
    for x in 0..8u64 {
        let mut theory = 1.0;
        for (q, &p) in ps.iter().enumerate() {
            let theta = p.acos();
            theory *= if x & (1 << q) == 0 {
                (theta / 2.0).cos()
            } else {
                (theta / 2.0).sin()
            };
        }
        assert!((s.outcome_probability(x) - theory * theory).abs() < 1e-12);
    }
}

// --- Продуктовый движок ---

#[test]
fn product_marginals_match_theory() {
    let pa = PhaseAnsatz::new(64, vec![(1, 0.6), (5, -0.8), (30, 0.0), (63, -0.2)]).unwrap();
    let shots = 40_000u64;
    let mut rng = Rng::seed_from_u64(42);
    let st = pa.sample(&mut rng, shots);
    assert_eq!(st.ones.len(), 4);
    let theory = [0.2, 0.9, 0.5, 0.6];
    for (k, &t) in theory.iter().enumerate() {
        let obs = st.ones[k] as f64 / shots as f64;
        let sigma = (t * (1.0 - t) / shots as f64).sqrt();
        assert!(
            (obs - t).abs() < SIG * sigma + 1e-12,
            "arc {k}: obs {obs}, theory {t}"
        );
    }
}

#[test]
fn product_weight_matches_statevector_moments() {
    // Один и тот же вектор фаз: statevector и product дают одинаковые моменты веса.
    let ps: Vec<f64> = [0.4, -0.7, 0.0, 0.9, -0.3, 0.55, -0.15, 0.8].to_vec();
    let sv = Statevector::from_phases(&ps).unwrap();
    let marginals = sv.marginals();
    let theory_mean: f64 = marginals.iter().sum();
    let theory_var: f64 = marginals.iter().map(|&m| m * (1.0 - m)).sum();

    let pa = PhaseAnsatz::new(
        8,
        ps.iter().enumerate().map(|(i, &p)| (i as u32, p)).collect(),
    )
    .unwrap();
    let shots = 50_000u64;
    let mut rng = Rng::seed_from_u64(17);
    let st = pa.sample(&mut rng, shots);
    let sigma_mean = (theory_var / shots as f64).sqrt();
    assert!(
        (st.weight_mean - theory_mean).abs() < SIG * sigma_mean,
        "mean {} vs {}",
        st.weight_mean,
        theory_mean
    );
    assert!(
        (st.weight_var - theory_var).abs() / theory_var < 0.05,
        "var {} vs {}",
        st.weight_var,
        theory_var
    );
    assert!((pa.expected_weight() - theory_mean).abs() < 1e-12);
}

#[test]
fn background_is_fair_binomial() {
    // Только фон: d = 200 честных монет без дуг.
    let pa = PhaseAnsatz::new(200, vec![]).unwrap();
    let shots = 30_000u64;
    let mut rng = Rng::seed_from_u64(21);
    let st = pa.sample(&mut rng, shots);
    let mean_t = 100.0;
    let var_t = 50.0;
    let sigma = (var_t / shots as f64).sqrt();
    assert!((st.weight_mean - mean_t).abs() < SIG * sigma);
    assert!((st.weight_var - var_t).abs() / var_t < 0.05);
    assert_eq!(
        st.weight_hist.iter().sum::<u64>(),
        shots,
        "гистограмма теряет выстрелы"
    );
}

#[test]
fn popcount_trick_exact_on_word_boundaries() {
    // d = 129 = 2 слова + 1 хвост; дуга p = −1 всегда даёт единицу.
    let pa = PhaseAnsatz::new(129, vec![(128, -1.0)]).unwrap();
    let shots = 20_000u64;
    let mut rng = Rng::seed_from_u64(33);
    let st = pa.sample(&mut rng, shots);
    // Вес = Binomial(128, ½) + 1 ∈ [1, 129]; нулевой бин пуст.
    assert_eq!(st.weight_hist[0], 0);
    let mean_t = 65.0; // фон 128 монет (среднее 64) + дуга p = −1 всегда 1.
    let var_t = 32.0;
    let sigma = (var_t / shots as f64).sqrt();
    assert!((st.weight_mean - mean_t).abs() < SIG * sigma);
    assert!((st.weight_var - var_t).abs() / var_t < 0.05);
}

#[test]
fn product_patterns_sum_to_shots() {
    let pa = PhaseAnsatz::new(100, vec![(3, 0.5), (10, -0.5), (42, 0.9), (99, -1.0)]).unwrap();
    let shots = 10_000u64;
    let mut rng = Rng::seed_from_u64(5);
    let st = pa.sample(&mut rng, shots);
    let pats = st.patterns.expect("nnz <= 64");
    assert_eq!(pats.iter().map(|(_, c)| c).sum::<u64>(), shots);
    // Дуга p = −1 всегда единица → бит 3 паттерна всегда выставлен.
    assert!(pats.iter().all(|(p, _)| p & (1 << 3) != 0));
    // Дуга p = 0.9 → P(1) = 0.05: суммарная частота паттернов с битом 2 мала.
    let ones42: u64 = pats
        .iter()
        .filter(|(p, _)| p & (1 << 2) != 0)
        .map(|(_, c)| c)
        .sum();
    assert!(ones42 < shots / 10, "ones42 = {ones42} of {shots}");
}
