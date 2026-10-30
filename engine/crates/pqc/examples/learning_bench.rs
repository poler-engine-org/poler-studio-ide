#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Бенчмарк петли фазового обучения Born (RQ5): стоимость одного шага
//! градиентного спуска «измерение → проектор → фазовый шаг → пурификация»
//! при d = 512 и d = 4096 (product-движок).
//!
//! ```text
//! cargo run --release -p pqc --example learning_bench
//! ```

use std::time::Duration;

use pqc::{quadratic_target, Ansatz, BornOptimizer, LoadOptions};

/// Число выстрелов на шаг измерения.
const SHOTS: u64 = 1024;
/// Замеряемых шагов (усреднение).
const STEPS: usize = 200;

fn bench(d: usize, purify_every: usize, gamma: f64, label: &str) {
    // Спайки на каждой четвёртой координате: старт ±0.3.
    let start: Vec<f64> = (0..d)
        .map(|i| match i % 4 {
            0 => 0.3,
            2 => -0.3,
            _ => 0.0,
        })
        .collect();
    let target: Vec<f64> = (0..d)
        .map(|i| match i % 4 {
            0 => 1.0,
            2 => -1.0,
            _ => 0.0,
        })
        .collect();

    let mut ansatz = Ansatz::from_phases(&start, &LoadOptions::default()).unwrap();
    debug_assert_eq!(ansatz.engine().name(), "product");

    let mut opt = BornOptimizer::new(0.2, gamma, SHOTS)
        .with_seed(2026)
        .with_purify_every(purify_every);

    // Прогрев: JIT-кэшей нет, но прогрев сглаживает аллокатор.
    for _ in 0..20 {
        opt.step(&mut ansatz, quadratic_target(&target)).unwrap();
    }

    // Замер: лучший (минимальный) шаг из STEPS последовательных.
    let mut best = Duration::MAX;
    let mut total = Duration::ZERO;
    for _ in 0..STEPS {
        let t0 = std::time::Instant::now();
        let rep = opt.step(&mut ansatz, quadratic_target(&target)).unwrap();
        let dt = t0.elapsed();
        best = best.min(dt);
        total += dt;
        assert!(rep.step > 0);
    }

    let mean = total / STEPS as u32;
    let loss = {
        let p = BornOptimizer::parameters(&ansatz).unwrap();
        0.5 * p
            .iter()
            .zip(&target)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
    };
    println!(
        "{label:<34} d={d:<5} shots={SHOTS:<5} best={best:?} mean={mean:?} финальная потеря={loss:.6}"
    );
}

fn main() {
    println!("Петля фазового обучения Born (RQ5): один градиентный шаг\n");
    println!("(γ = 0.9 без пурификации; γ = 0.5 с пурификацией — у полюсов");
    println!(" сильный момент залипает, см. документацию pqc::learn)\n");
    bench(512, 0, 0.9, "product, без пурификации");
    bench(512, 2, 0.5, "product, McWeeny каждые 2 шага");
    bench(4096, 0, 0.9, "product, без пурификации");
    bench(4096, 2, 0.5, "product, McWeeny каждые 2 шага");
}
