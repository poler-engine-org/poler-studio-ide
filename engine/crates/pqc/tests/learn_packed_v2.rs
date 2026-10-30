#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Интеграция RQ5 × v0.2: петля Born-обучения сходится к тритовому
//! состоянию и сериализуется в упакованный контейнер `POLER_Q2`,
//! который читатель возвращает в квантовый контур без потерь.

use pqc::{quadratic_target, Ansatz, BornOptimizer, LoadOptions};
use pqw::{PqwReader, PqwWriter, Trit};

/// Замкнутый цикл: фазовый вектор → обучение → триты → v2-контейнер → анзац.
#[test]
fn learn_converge_serialize_reload() {
    // 1. Стартовое состояние: слабые сигналы на дугах, фон честные монеты.
    let d = 256u32;
    let arcs: Vec<(u32, f32)> = (0..d)
        .step_by(4)
        .map(|i| (i, if (i / 4) % 2 == 0 { 0.2 } else { -0.2 }))
        .collect();
    let mut w = PqwWriter::new(d).unwrap();
    for &(i, p) in &arcs {
        w.add_phase(i, p).unwrap();
    }
    let bytes = w.to_bytes().unwrap();

    // 2. Загрузка в product-движок (d = 256 > порога 20).
    let reader = PqwReader::from_bytes(&bytes).unwrap();
    let mut ansatz = Ansatz::from_reader(&reader, &LoadOptions::default()).unwrap();
    assert_eq!(ansatz.engine().name(), "product");

    // 3. Петля обучения к чистым тритам: цель ±1, пурификация каждые 2 шага.
    let target: Vec<f64> = (0..d)
        .map(|i| {
            if i % 8 == 0 {
                1.0
            } else if i % 8 == 4 {
                -1.0
            } else {
                0.0
            }
        })
        .collect();
    let mut opt = BornOptimizer::new(0.2, 0.9, 4096)
        .with_seed(77)
        .with_purify_every(2);
    let reports = opt.run(&mut ansatz, quadratic_target(&target), 8).unwrap();
    assert_eq!(reports.len(), 8);
    assert!(reports.last().unwrap().purified);

    // 4. Сходимость к тритам: дуги ±0.2 → ±1 точно.
    let p = BornOptimizer::parameters(&ansatz).unwrap();
    for (i, &pv) in p.iter().enumerate() {
        if i % 8 == 0 {
            assert!(pv > 0.999, "дуга {i}: p = {pv}");
        } else if i % 8 == 4 {
            assert!(pv < -0.999, "дуга {i}: p = {pv}");
        }
    }

    // 5. Сериализация обученного состояния в v2 (упакованные триты).
    let mut w2 = PqwWriter::new(d).unwrap();
    for (i, &pv) in p.iter().enumerate() {
        if pv.abs() >= 0.05 {
            w2.add_phase(i as u32, pv as f32).unwrap();
        }
    }
    let packed = w2.to_bytes_packed().unwrap();
    assert_eq!(&packed[..8], b"POLER_Q2");
    assert_eq!(packed.len(), 128 + (d as usize) / 4);

    // 6. Перезагрузка: v2-контейнер даёт то же тритовое состояние.
    let r2 = PqwReader::from_bytes(&packed).unwrap();
    assert_eq!(r2.nnz(), 64);
    let trits: Vec<(u32, Trit)> = r2.iter_packed_trits().unwrap().collect();
    assert_eq!(trits.len(), d as usize);
    assert_eq!(trits[0].1, Trit::Pos);
    assert_eq!(trits[4].1, Trit::Neg);
    assert_eq!(trits[1].1, Trit::Zero);
    r2.verify_payload().unwrap();

    // 7. Квантовая семантика: анзац из v2 сэмплирует детерминированные
    // биты на спайках (P(b=1) = 0 или 1) и честные монеты на фоне.
    let ansatz2 = Ansatz::from_reader(&r2, &LoadOptions::default()).unwrap();
    let mut rng = pqc::Rng::seed_from_u64(5);
    let rep = ansatz2.sample(&mut rng, 2000, 4).unwrap();
    for &(q, theory, obs) in &rep.marginals {
        let expected = if q % 8 == 0 {
            0.0 // p = +1 → |0⟩
        } else {
            1.0 // p = −1 → |1⟩
        };
        assert!((theory - expected).abs() < 1e-12);
        assert!(
            (obs - expected).abs() < 1e-12,
            "дуга {q}: наблюдение {obs} против теории {expected}"
        );
    }
}

/// Оба движка читают v2: statevector для малых d, product для больших.
#[test]
fn v2_container_feeds_both_engines() {
    let mut w = PqwWriter::new(6).unwrap();
    w.add_phase(0, 1.0).unwrap();
    w.add_phase(2, -1.0).unwrap();
    w.add_phase(4, 1.0).unwrap();
    let packed = w.to_bytes_packed().unwrap();
    let r = PqwReader::from_bytes(&packed).unwrap();

    // d = 6 ≤ 20 → statevector.
    let ansatz = Ansatz::from_reader(&r, &LoadOptions::default()).unwrap();
    assert_eq!(ansatz.engine().name(), "statevector");
    let mut rng = pqc::Rng::seed_from_u64(1);
    let rep = ansatz.sample(&mut rng, 500, 4).unwrap();
    // p = +1 → q0, q4 всегда 0; p = −1 → q2 всегда 1; фон q1,3,5 — монеты.
    for &(q, theory, _obs) in &rep.marginals {
        match q {
            0 | 4 => assert!(theory < 1e-12),
            2 => assert!((theory - 1.0).abs() < 1e-12),
            _ => assert!((theory - 0.5).abs() < 1e-12),
        }
    }

    // d = 6 с принудительным product-движком — те же маргиналы дуг.
    let opts = LoadOptions {
        max_sv_qubits: 0,
        ..LoadOptions::default()
    };
    let ansatz_p = Ansatz::from_reader(&r, &opts).unwrap();
    assert_eq!(ansatz_p.engine().name(), "product");
    let rep_p = ansatz_p.sample(&mut rng, 500, 4).unwrap();
    // Product отдаёт маргиналы только хранимых дуг — сверяем по индексу.
    assert_eq!(rep_p.marginals.len(), 3);
    for (q, t1, _) in &rep.marginals {
        let mp = rep_p.marginals.iter().find(|m| m.0 == *q);
        match mp {
            Some((_, _, t2)) => assert!((t1 - t2).abs() < 1e-15, "дуга {q}: {t1} vs {t2}"),
            None => assert!((t1 - 0.5).abs() < 1e-12, "фон {q} должен быть монетой"),
        }
    }
}
