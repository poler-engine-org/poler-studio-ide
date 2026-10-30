#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! McWeeny-инвариант формата: подпись в заголовке, очистка на лету.

mod common;

use common::Rng;
use pqw::{PqwReader, PqwWriter};

/// Сильные треки + явный нуль: 0.9, −0.9, 0.5, −0.5, 0.0, 1.0.
fn strong_fixture() -> Vec<u8> {
    let mut w = PqwWriter::new(6)
        .unwrap()
        .hyperparams(0.01, 0.1, 0.99, 0.05);
    w.add_phase(0, 0.9).unwrap();
    w.add_phase(1, -0.9).unwrap();
    w.add_phase(2, 0.5).unwrap();
    w.add_phase(3, -0.5).unwrap();
    w.add_phase(4, 0.0).unwrap(); // явный нуль
    w.add_phase(5, 1.0).unwrap();
    w.to_bytes().unwrap()
}

#[test]
fn stored_residual_matches_recomputation() {
    let bytes = strong_fixture();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    let recomputed = pqw::mcweeny::idempotency_residual(r.decoded().map(|(_, p)| p));
    assert!(
        (r.mcweeny_residual() - recomputed).abs() < 1e-15,
        "stored = {}, recomputed = {}",
        r.mcweeny_residual(),
        recomputed
    );
    // Максимум |λ²−λ| даёт явный нуль (λ = 1/2 → 1/4).
    assert!((r.mcweeny_residual() - 0.25).abs() < 1e-15);
}

#[test]
fn purification_two_steps_fixes_strong_arcs() {
    let bytes = strong_fixture();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    let purified = r.purified(); // 2 шага McWeeny
    let by_index: std::collections::HashMap<u32, f64> = purified.into_iter().collect();

    // Сильные треки уходят к чистым фазам за 1–2 такта.
    assert!(by_index[&0] > 0.999);
    assert!(by_index[&1] < -0.999);
    assert_eq!(by_index[&5], 1.0);

    // Триты (знаки) сохраняются.
    for (i, sign) in [(0u32, 1.0), (1, -1.0), (2, 1.0), (3, -1.0), (5, 1.0)] {
        assert_eq!(by_index[&i].signum(), sign, "arc {i}");
    }

    // Явный нуль — неподвижная точка потока.
    assert_eq!(by_index[&4], 0.0);
}

#[test]
fn residual_collapses_after_purification() {
    let bytes = strong_fixture();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    // Без явного нуля (он даёт константный вклад 0.25 как неподвижная точка).
    let mut strong: Vec<f64> = r
        .decoded()
        .filter(|(i, _)| *i != 4)
        .map(|(_, p)| p)
        .collect();

    let before = pqw::mcweeny::idempotency_residual(strong.iter().copied());
    let mut s1 = strong.clone();
    let after2 = pqw::mcweeny::purify_state(&mut s1, 2);
    let after5 = pqw::mcweeny::purify_state(&mut strong, 5);

    assert!(after2 < before);
    assert!(after5 < 1e-5, "after5 = {after5}");
}

#[test]
fn purify_steps_converge_monotonically() {
    let mut rng = Rng::new(31337);
    let d = 64;
    let mut w = PqwWriter::new(d).unwrap();
    for i in 0..d {
        let p = (rng.unit() * 2.0 - 1.0) as f32;
        if p.abs() >= 0.3 {
            w.add_phase(i, p).unwrap();
        }
    }
    let out = w.to_bytes().unwrap();
    let r = PqwReader::from_bytes(&out).unwrap();
    assert!(r.nnz() > 0);

    let mut ps: Vec<f64> = r.decoded().map(|(_, p)| p).collect();
    let mut prev = pqw::mcweeny::idempotency_residual(ps.iter().copied());
    for _ in 0..4 {
        let now = pqw::mcweeny::purify_state(&mut ps, 1);
        assert!(now <= prev, "residual must be non-increasing");
        prev = now;
    }
}
