#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Бенчмарк Active Inference (RQ6): сквозной zero-storage цикл
//! «HTML/текст → TF-IDF ε-плотность → LENS-CSR → Packed4 → Born-фазовый шаг →
//! QCM + коммутатор Фокиана» — throughput потока и FLOPs-соотношение против
//! плотного self-attention.
//!
//! ```text
//! cargo run --release -p pqc --example active_inference_bench
//! ```
//!
//! FLOPs-модель чанка (честный подсчёт операций нашего пути):
//!
//! * кодирование: `~6·tokens` (FNV-хеш + накопление + idf);
//! * контейнер Packed4: `d/4` записи байтов;
//! * измерение Борна: `shots·nnz` испытаний Бернулли;
//! * градиент + момент + шаг: `~10·nnz`;
//! * QCM + коммутатор: `shots_qcm·nnz + 5·d`.
//!
//! Против плотного self-attention того же чанка: `2·tokens²·d` (QKᵀ/√d и
//! взвешивание V). Справочные числа моделей — из архитектурного манифеста
//! POLER (Qwen2.5-1.5B: 792.7M dense против 36.7M POLER — 21.6×).

use std::time::{Duration, Instant};

use pqc::stream_engine::{fock_residual, StreamEngine};
use pqc::Rng;

/// Выстрелов на измерение в петле.
const SHOTS: u64 = 1024;
/// Выстрелов QCM-оценки.
const QCM_SHOTS: u64 = 256;
/// Повторений замера (best-of).
const REPEATS: usize = 64;

/// Реалистичная страница: HTML-обёртка + ~500 слов_PAYLOAD.
fn page(seed: u64) -> String {
    let words = [
        "фазовый",
        "континуум",
        "триты",
        "борн",
        "анзац",
        "линза",
        "решётка",
        "разреженный",
        "граф",
        "индекс",
        "поток",
        "обучение",
        "онлайн",
        "фазы",
        "квантование",
        "многообразие",
        "проектор",
        "когерентность",
        "резонанс",
        "энтропия",
        "свидетельство",
        "коммутатор",
        "стационарность",
        "память",
    ];
    let mut body = String::new();
    let mut x = seed | 1;
    for _ in 0..500 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        body.push_str(words[(x as usize) % words.len()]);
        body.push(' ');
    }
    format!(
        "<html><head><title>POLER streaming bench</title>\
         <style>body {{ font: serif }}</style></head>\
         <body><h1>Zero-Storage Streaming Engine</h1><p>{body}</p>\
         <script>var ignore = 1;</script></body></html>"
    )
}

/// Прогон одного движка; возвращает (лучший ingest, суммарный, отчёты).
fn bench(d: u32, epsilon: f32, steps: usize, label: &str) -> (Duration, Duration, usize, u64) {
    let mut engine = StreamEngine::new(d, epsilon, 42)
        .unwrap()
        .with_shots(SHOTS)
        .with_qcm_shots(QCM_SHOTS);

    // Прогрев: аллокации буфера, таблицы Юникода, кэш аллокатора.
    let warm = page(1);
    engine.ingest_html(warm.as_bytes(), steps).unwrap();

    let mut best = Duration::MAX;
    let mut total = Duration::ZERO;
    let mut tokens = 0usize;
    let mut nnz = 0u64;
    for k in 0..REPEATS {
        let html = page(1000 + k as u64);
        let t0 = Instant::now();
        let rep = engine.ingest_html(html.as_bytes(), steps).unwrap();
        let dt = t0.elapsed();
        best = best.min(dt);
        total += dt;
        tokens = rep.tokens;
        nnz = rep.nnz;
        assert!(!rep.no_hits);
    }
    let mean = total / REPEATS as u32;
    let throughput = tokens as f64 / mean.as_secs_f64();
    println!(
        "{label:<28} d={d:<6} nnz={nnz:<4} шагов={steps:<3} \
         best={best:>10.3?} mean={mean:>10.3?} \
         throughput={throughput:>9.0} ток/с"
    );
    (best, total, tokens, nnz)
}

/// FLOPs-сравнение пути POLER против плотного attention на том же чанке.
fn flops_report(d: u32, tokens: usize, nnz: u64, steps: usize, mean: Duration) {
    // Наш путь (верхняя оценка — с запасом в сторону POLER).
    let encode = 6.0 * tokens as f64;
    let container = d as f64 / 4.0;
    let measure = SHOTS as f64 * nnz as f64 * (steps + 1) as f64;
    let grad = 10.0 * nnz as f64 * (steps + 1) as f64;
    let eval = QCM_SHOTS as f64 * nnz as f64 + 5.0 * d as f64;
    let poler = encode + container + measure + grad + eval;

    // Плотный self-attention на том же чанке: 2·n²·d (QKᵀ + V-взвешивание).
    let n = tokens as f64;
    let dense = 2.0 * n * n * d as f64;

    println!();
    println!("  FLOPs чанка (n = {tokens} токенов, d = {d}):");
    println!("    POLER zero-storage путь : {:>12.0}", poler);
    println!("    плотный self-attention  : {:>12.0}", dense);
    println!(
        "    соотношение             : {:>12.0}x дешевле",
        dense / poler
    );
    println!("    (манифест: Qwen2.5-1.5B dense 792.7M vs POLER 36.7M = 21.6x)");
    let _ = mean;
}

fn main() {
    println!("Active Inference (RQ6): сквозной zero-storage цикл");
    println!("(HTML → TF-IDF ε-плотность → LENS-CSR → Packed4 → Born-шаг → QCM)\n");

    // Замеры для d = 512 и d = 4096.
    let (best512, _, tokens512, nnz512) = bench(512, 0.2, 1, "сквозной ingest, 1 шаг");
    let (best_steps, total_steps, tokens_s, nnz_s) = bench(512, 0.2, 8, "сквозной ingest, 8 шагов");
    let (best4k, _, tokens4k, nnz4k) = bench(4096, 0.15, 1, "сквозной ingest, 1 шаг");

    // QCM + коммутатор изолированно (бюджет RQ6: < 150 мкс).
    let mut engine = StreamEngine::new(512, 0.2, 7)
        .unwrap()
        .with_qcm_shots(QCM_SHOTS);
    engine.ingest_html(page(3).as_bytes(), 2).unwrap();
    let state: Vec<f64> = engine.model().to_vec();
    let arcs = engine.model_arcs().to_vec();
    let ansatz = pqc::Ansatz::Product(pqc::PhaseAnsatz::new(512, arcs).unwrap());
    let mut rng = Rng::seed_from_u64(7);
    let mut best_eval = Duration::MAX;
    for _ in 0..256 {
        let t0 = Instant::now();
        let f = fock_residual(&state, engine.model());
        let sample = ansatz.sample(&mut rng, QCM_SHOTS, 0).unwrap();
        let qcm = pqc::coherence::coherence(&sample);
        let dt = t0.elapsed();
        assert!(f.normalized >= 0.0 && qcm.qcm_theory.is_finite());
        best_eval = best_eval.min(dt);
    }
    println!(
        "\n  QCM + коммутатор (d = 512, изолированно): best = {best_eval:.3?} \
         (бюджет RQ6: 150 мкс)"
    );

    // FLOPs-отчёт по основной конфигурации.
    let mean_steps = total_steps / REPEATS as u32;
    flops_report(512, tokens_s, nnz_s, 8, mean_steps);

    println!("\n  Сводка:");
    println!("    d = 512,  1 шаг  : best {best512:.3?}");
    println!("    d = 512,  8 шагов: best {best_steps:.3?}");
    println!("    d = 4096, 1 шаг  : best {best4k:.3?}");
    let _ = (tokens512, nnz512, tokens4k, nnz4k);
}
