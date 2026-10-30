#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Сквозной мост pqw → pqc: контейнер → анзац → Born-сэмплирование,
//! включая mmap, LENS-семантику фона, McWeeny-заострение и детерминизм.

use pqc::{Ansatz, Engine, LoadOptions, PhaseAnsatz, Rng};
use pqw::mcweeny::purify_p;
use pqw::{PqwReader, PqwWriter};

const SIG: f64 = 5.0;

#[test]
fn small_container_uses_statevector() {
    let bytes = PqwWriter::new(4)
        .unwrap()
        .add_phase(0, 0.6)
        .unwrap()
        .add_phase(2, -1.0)
        .unwrap()
        .to_bytes()
        .unwrap();
    let reader = PqwReader::from_bytes(&bytes).unwrap();
    let ansatz = Ansatz::from_reader(&reader, &LoadOptions::default()).unwrap();
    let Ansatz::Statevector(sv) = &ansatz else {
        panic!("d_pol = 4 must use the statevector engine");
    };
    // Маргиналы считаются из деквантованных p̂, а не из исходных p.
    let m = sv.marginals();
    let p0 = reader.arcs().next().unwrap().p();
    let p2 = reader.arcs().nth(1).unwrap().p();
    assert!((m[0] - (1.0 - p0) / 2.0).abs() < 1e-12);
    assert!((m[2] - (1.0 - p2) / 2.0).abs() < 1e-12);
    assert!((m[1] - 0.5).abs() < 1e-12); // LENS-пропуск = фон.
}

#[test]
fn large_container_uses_product() {
    let mut w = PqwWriter::new(100).unwrap();
    w.add_phase(1, 0.5).unwrap();
    w.add_phase(99, -0.5).unwrap();
    let bytes = w.to_bytes().unwrap();
    let reader = PqwReader::from_bytes(&bytes).unwrap();
    let ansatz = Ansatz::from_reader(&reader, &LoadOptions::default()).unwrap();
    let Ansatz::Product(pa) = &ansatz else {
        panic!("d_pol = 100 must use the product engine");
    };
    assert_eq!(pa.d_pol(), 100);
    assert_eq!(pa.nnz(), 2);
}

#[test]
fn lens_missing_arcs_are_fair_coins() {
    // ε = 0.5: остаются только |p| ≥ 0.5 — дуги 0, 2, 5, 7.
    let dense = [0.9, 0.1, -0.8, 0.0, 0.3, -0.95, 0.05, 0.7];
    let mut w = PqwWriter::new(8).unwrap();
    w = w.hyperparams(0.01, 0.1, 0.99, 0.5);
    w.add_state(&dense).unwrap();
    let bytes = w.to_bytes().unwrap();
    let reader = PqwReader::from_bytes(&bytes).unwrap();
    assert_eq!(reader.nnz(), 4);

    let mut opts = LoadOptions::default();
    opts.max_sv_qubits = 0; // принудительно product
    let ansatz = Ansatz::from_reader(&reader, &opts).unwrap();
    let Ansatz::Product(pa) = &ansatz else {
        panic!("forced product engine");
    };

    // Средний вес: фон (4 монеты) + спайки с p̂.
    let expected = pa.expected_weight();
    let mut manual = 4.0 * 0.5; // LENS-пропуски: дуги 1, 3, 4, 6.
    for arc in reader.arcs() {
        manual += (1.0 - arc.p()) / 2.0;
    }
    assert!((expected - manual).abs() < 1e-12);
}

#[test]
fn purify_sharpens_distributions() {
    let bytes = PqwWriter::new(4)
        .unwrap()
        .add_phase(0, 0.6)
        .unwrap()
        .to_bytes()
        .unwrap();
    let reader = PqwReader::from_bytes(&bytes).unwrap();

    let mut opts = LoadOptions::default();
    opts.max_sv_qubits = 0;
    let plain = Ansatz::from_reader(&reader, &opts).unwrap();
    opts.purify_steps = 2;
    let sharp = Ansatz::from_reader(&reader, &opts).unwrap();
    let (Ansatz::Product(a), Ansatz::Product(b)) = (&plain, &sharp) else {
        panic!()
    };

    // Ожидание считаем от деквантованной p̂ из файла, а не от исходного p.
    let p_hat = reader.arcs().next().unwrap().p();
    let m0 = a.marginal_theory()[0].1;
    let m2 = b.marginal_theory()[0].1;
    // p̂ > 0 → McWeeny гонит p̂ к +1 → P(b = 1) = (1−p)/2 убывает.
    assert!(m2 < m0, "purify must sharpen: {m2} !< {m0}");
    // И ровно к значению двух шагов purify_p от p̂.
    let p2 = purify_p(purify_p(p_hat));
    assert!((m2 - (1.0 - p2) / 2.0).abs() < 1e-12);
}

#[test]
fn verify_payload_detects_corruption() {
    let mut bytes = PqwWriter::new(8)
        .unwrap()
        .add_phase(3, 0.42)
        .unwrap()
        .to_bytes()
        .unwrap();
    // Портим один фазовый байт (последний байт файла).
    let last = bytes.len() - 1;
    bytes[last] ^= 0x10;

    let reader = PqwReader::from_bytes(&bytes).unwrap();
    let mut opts = LoadOptions::default();
    opts.verify_payload = true;
    assert!(Ansatz::from_reader(&reader, &opts).is_err());

    // Без --verify загрузка проходит (ленивая целостность).
    opts.verify_payload = false;
    assert!(Ansatz::from_reader(&reader, &opts).is_ok());
}

#[test]
fn sampling_is_reproducible() {
    let mut w = PqwWriter::new(10).unwrap();
    w = w.hyperparams(0.01, 0.1, 0.99, 0.05);
    for (i, p) in [0.5, -0.3, 0.8, -0.95].iter().enumerate() {
        w.add_phase(i as u32 * 2, *p).unwrap();
    }
    let bytes = w.to_bytes().unwrap();
    let reader = PqwReader::from_bytes(&bytes).unwrap();

    let a = Ansatz::from_reader(&reader, &LoadOptions::default()).unwrap();
    let b = Ansatz::from_reader(&reader, &LoadOptions::default()).unwrap();
    let (mut r1, mut r2) = (Rng::seed_from_u64(42), Rng::seed_from_u64(42));
    let ra = a.sample(&mut r1, 2000, 5).unwrap();
    let rb = b.sample(&mut r2, 2000, 5).unwrap();
    assert_eq!(ra.shots, rb.shots);
    assert_eq!(ra.top, rb.top);
    assert_eq!(ra.weight_mean, rb.weight_mean);
    assert_eq!(ra.marginals, rb.marginals);
}

#[test]
fn report_structure_is_consistent() {
    let bytes = PqwWriter::new(6)
        .unwrap()
        .add_phase(1, -0.7)
        .unwrap()
        .add_phase(4, 0.3)
        .unwrap()
        .to_bytes()
        .unwrap();
    let reader = PqwReader::from_bytes(&bytes).unwrap();
    let ansatz = Ansatz::from_reader(&reader, &LoadOptions::default()).unwrap();
    let mut rng = Rng::seed_from_u64(3);
    let report = ansatz.sample(&mut rng, 5000, 3).unwrap();

    assert_eq!(report.engine, Engine::Statevector);
    assert_eq!(report.shots, 5000);
    assert_eq!(report.marginals.len(), 6);
    assert!(report.top.len() <= 3);
    assert_eq!(report.top.len(), report.top_probs.len());
    // Счётчики топа не превосходят число выстрелов, частоты суммируемо ≤ 1.
    assert!(report.top.iter().all(|(_, c)| *c <= 5000));
    // Дуга p = −0.7 → P(b=1) ≈ 0.85.
    let q1 = report.marginals.iter().find(|m| m.0 == 1).unwrap();
    assert!((q1.1 - 0.85).abs() < 0.02);
}

#[test]
fn dense_phase_vector_large_goes_product() {
    let ps: Vec<f64> = (0..30).map(|i| ((i % 7) as f64 - 3.0) / 3.0).collect();
    let ansatz = Ansatz::from_phases(&ps, &LoadOptions::default()).unwrap();
    let Ansatz::Product(pa) = &ansatz else {
        panic!("30 > 20 must use product");
    };
    assert_eq!(pa.nnz(), 30);
    let m = pa.marginal_theory();
    assert!((m[0].1 - (1.0 - ps[0]) / 2.0).abs() < 1e-12);
}

#[test]
fn statevector_cutoff_is_configurable() {
    let bytes = PqwWriter::new(16)
        .unwrap()
        .add_phase(0, 0.5)
        .unwrap()
        .to_bytes()
        .unwrap();
    let reader = PqwReader::from_bytes(&bytes).unwrap();

    // 16 ≤ 20 → statevector.
    let a = Ansatz::from_reader(&reader, &LoadOptions::default()).unwrap();
    assert!(matches!(a, Ansatz::Statevector(_)));

    // Порог 10 → product.
    let mut opts = LoadOptions::default();
    opts.max_sv_qubits = 10;
    let b = Ansatz::from_reader(&reader, &opts).unwrap();
    assert!(matches!(b, Ansatz::Product(_)));
}

#[test]
fn ansatz_from_phases_matches_reader_path() {
    // Плотный вектор в памяти против тех же фаз через контейнер.
    let ps = [0.3_f32, -0.6, 0.9, 0.0, -1.0, 0.45];
    let mut w = PqwWriter::new(6).unwrap();
    w = w.hyperparams(0.01, 0.1, 0.99, 0.0); // ε = 0: LENS хранит всё.
    w.add_state(&ps).unwrap();
    let bytes = w.to_bytes().unwrap();
    let reader = PqwReader::from_bytes(&bytes).unwrap();

    let via_file = Ansatz::from_reader(&reader, &LoadOptions::default()).unwrap();
    let direct = Ansatz::from_phases(
        &ps.iter().map(|&p| p as f64).collect::<Vec<_>>(),
        &LoadOptions::default(),
    )
    .unwrap();
    let (Ansatz::Statevector(a), Ansatz::Statevector(b)) = (&via_file, &direct) else {
        panic!()
    };
    for (x, y) in a.amplitudes().iter().zip(b.amplitudes()) {
        // Квантование p̂ в файле вносит ≤ 1/126 по p — амплитуды близки,
        // но не обязаны совпадать бит-в-бит.
        assert!((*x - *y).norm() < 1e-2);
    }
}

#[cfg(unix)]
#[test]
fn mmap_pipeline_end_to_end() {
    use pqw::Mmap;

    let mut path = std::env::temp_dir();
    path.push(format!("pqc-mmap-{}.poler", std::process::id()));
    let mut w = PqwWriter::new(12).unwrap();
    w = w.hyperparams(0.01, 0.1, 0.99, 0.08);
    let dense: Vec<f32> = (0..12)
        .map(|i| if i % 3 == 0 { 0.9 } else { 0.01 })
        .collect();
    w.add_state(&dense).unwrap();
    w.write_to(&path).unwrap();

    let map = Mmap::open(&path).unwrap();
    let reader = PqwReader::from_bytes(map.as_slice()).unwrap();
    let ansatz = Ansatz::from_reader(&reader, &LoadOptions::default()).unwrap();
    let mut rng = Rng::seed_from_u64(1);
    let report = ansatz.sample(&mut rng, 1000, 4).unwrap();
    assert_eq!(report.shots, 1000);
    // Спайки p = 0.9 на кубитах 0, 3, 6, 9: теория — от деквантованной p̂.
    let mut p_hat = std::collections::HashMap::new();
    for arc in reader.arcs() {
        p_hat.insert(arc.index, arc.p());
    }
    for q in [0u32, 3, 6, 9] {
        let m = report.marginals.iter().find(|m| m.0 == q).unwrap();
        let t = (1.0 - p_hat[&q]) / 2.0;
        assert!(
            (m.1 - t).abs() < 1e-12,
            "theory q{q} = {} (ожидается {t})",
            m.1
        );
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn phase_ansatz_direct_construction() {
    let pa = PhaseAnsatz::new(16, vec![(2, 0.5), (7, -0.5)]).unwrap();
    let mut rng = Rng::seed_from_u64(77);
    let st = pa.sample(&mut rng, 5000);
    assert_eq!(st.ones.len(), 2);
    // Оба пути отсчёта согласованы с теорией в пределах 5σ.
    for (k, &t) in [0.25_f64, 0.75].iter().enumerate() {
        let obs = st.ones[k] as f64 / 5000.0;
        let sigma = (t * (1.0 - t) / 5000.0).sqrt();
        assert!((obs - t).abs() < SIG * sigma);
    }
}
