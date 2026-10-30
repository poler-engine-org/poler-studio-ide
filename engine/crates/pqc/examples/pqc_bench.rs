#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Микро-бенчмарк ядер pqc: анзац, вероятности, CDF, сэмплирование,
//! продуктовый движок. Запуск: `cargo run --release -p pqc --example pqc_bench`
//! (переименован из `bench` при всасывании в workspace poler-engine —
//! коллизия имён example-таргетов с движком, cargo#6313).
//!
//! Совет: `RUSTFLAGS="-C target-cpu=native" cargo run --release ...`
//! включает AVX2/FMA-генерацию ядер на машине исполнения.

use std::time::Instant;

use pqc::{BornSampler, PhaseAnsatz, Rng, Statevector};

fn main() {
    println!("pqc bench ({} потоков)", pqc::parallel::worker_count());

    // --- 1. Statevector: анзац R_y(arccos p) на n = 20 (2^20 амплитуд) ---
    let n = 20usize;
    let ps: Vec<f64> = (0..n)
        .map(|i| (((i * 37 + 11) % 199) as f64 - 99.0) / 99.0)
        .collect();
    let t = Instant::now();
    let sv = Statevector::from_phases(&ps).expect("from_phases");
    println!(
        "from_phases    n=20 (1 048 576 амплитуд, 20 Ry): {:>10.2?}",
        t.elapsed()
    );

    let t = Instant::now();
    let probs = sv.probabilities();
    let sum: f64 = probs.iter().sum();
    println!(
        "probabilities  2^20 (параллельно): {:>10.2?}  (сумма = {sum:.12})",
        t.elapsed()
    );

    let t = Instant::now();
    let sampler = BornSampler::new(&sv).expect("normalized");
    println!("cdf build      2^20: {:>10.2?}", t.elapsed());

    let mut rng = Rng::seed_from_u64(42);
    let shots = 100_000u64;
    let t = Instant::now();
    let out = sampler.sample_n(&mut rng, shots);
    println!("sampling       {shots} выстрелов: {:>10.2?}", t.elapsed());

    // --- 2. Гейты: последовательность на n = 22 (2^22 = 4M амплитуд) ---
    let n = 22usize;
    let mut sv = Statevector::new(n).expect("new");
    let t = Instant::now();
    for q in 0..n {
        sv.apply_ry(q, 0.5 + q as f64 * 0.01).expect("ry");
    }
    sv.apply_cnot(0, n - 1).expect("cnot");
    sv.apply_h(1).expect("h");
    println!(
        "gate sequence  n=22 (22 Ry + CNOT + H, 4 194 304 амплитуд): {:>10.2?}",
        t.elapsed()
    );
    println!(
        "               норма после последовательности: {:.12}",
        sv.norm()
    );

    // --- 3. Продуктовый движок: d_pol = 65 536, nnz = 512 ---
    let d = 65_536u32;
    let arcs: Vec<(u32, f64)> = (0..512)
        .map(|k| {
            let i = (k as u64 * 131 + 7) % u64::from(d);
            let p = (((k * 7919) % 199) as f64 - 99.0) / 99.0;
            (i as u32, p)
        })
        .collect();
    // Сортировка по индексу (конструктор требует строгую возрастаемость).
    let mut arcs = arcs;
    arcs.sort_unstable_by_key(|a| a.0);
    arcs.dedup_by_key(|a| a.0);
    let pa = PhaseAnsatz::new(d, arcs).expect("ansatz");
    let mut rng = Rng::seed_from_u64(42);
    let shots = 10_000u64;
    let t = Instant::now();
    let st = pa.sample(&mut rng, shots);
    println!(
        "product sample d=65536 nnz={} {shots} выстрелов: {:>10.2?}",
        pa.nnz(),
        t.elapsed()
    );
    println!(
        "               вес: среднее {:.1} (теория {:.1}), дисперсия {:.1}",
        st.weight_mean,
        pa.expected_weight(),
        st.weight_var
    );
    let _ = out;
}
