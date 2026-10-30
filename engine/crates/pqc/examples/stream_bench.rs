#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Бенчмарк zero-storage стриминга (RQ3): латентность сквозного
//! конвейера «текст → ε-плотность → LENS → .pqw (RAM) → Born → QCM».
//!
//! ```text
//! cargo run --release -p pqc --example stream_bench
//! ```

use std::time::Duration;

use pqc::entangle::{Entanglement, Entangler};
use pqc::stream::ZeroStoragePipeline;

fn chunk(kb: usize) -> String {
    let words = [
        "фазовый",
        "континуум",
        "трит",
        "кривизна",
        "запутанность",
        "когерентность",
        "борн",
        "ансамбль",
        "интерференция",
        "резонанс",
        "спайк",
        "линза",
        "поток",
        "квантование",
        "многообразие",
        "проектор",
        "ядро",
        "решётка",
        "фон",
        "порог",
        "плотность",
        "энтропия",
        "выстрел",
        "сэмплирование",
    ];
    let mut text = String::with_capacity(kb * 1024);
    let mut i = 0usize;
    while text.len() < kb * 1024 {
        text.push_str(words[i % words.len()]);
        text.push(' ');
        i += 1;
    }
    text
}

fn bench(d_pol: u32, epsilon: f32, shots: u64, text: &str, ent: Entanglement, label: &str) {
    let pipe = ZeroStoragePipeline::new(d_pol, epsilon)
        .unwrap()
        .with_shots(shots)
        .with_entanglement(ent);
    let (rep, best) = pipe.run_timed(text, 99, 21).unwrap();
    println!(
        "{label:<38} d={d_pol:<5} nnz={:<4} bytes={:<6} shots={shots:<6} best={best:?} QCM={:.4}",
        rep.nnz, rep.container_bytes, rep.coherence.qcm_theory
    );
    // Требование RQ3: сквозной прогон d=512 строго быстрее 2 мс.
    if d_pol == 512 {
        assert!(
            best < Duration::from_millis(2),
            "бюджет 2 мс превышен: {best:?}"
        );
    }
}

fn main() {
    println!("zero-storage streaming bench (best of 21, release)");
    let text = chunk(2);
    bench(
        512,
        0.2,
        256,
        &text,
        Entanglement::None,
        "product d=512, без энтанглмента",
    );
    bench(
        512,
        0.2,
        256,
        &text,
        Entanglement::from_topology(Entangler::Cx),
        "product d=512, topology:cx (LENS)",
    );
    bench(
        512,
        0.2,
        256,
        &text,
        Entanglement::from_topology(Entangler::Cz),
        "product d=512, topology:cz (LENS)",
    );
    bench(
        4096,
        0.15,
        256,
        &chunk(8),
        Entanglement::None,
        "product d=4096",
    );
    bench(
        65536,
        0.1,
        128,
        &chunk(32),
        Entanglement::None,
        "product d=65536",
    );
    bench(16, 0.15, 256, &text, Entanglement::None, "statevector d=16");
    println!("все прогоны в бюджете 2 мс");
}
