#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Roundtrip-тесты: точная байтовая раскладка, восстановление значений,
//! детерминизм, LENS-фильтрация, zero-copy.

mod common;

use common::Rng;
use pqw::{PqwReader, PqwWriter, Trit};

/// d = 8, дуги (1, 0.5), (3, −1.0), (5, 0.0), гиперпараметры 0.5/0.25/0.75/0.125.
/// Ровно этот файл продиктован вручную в golden-тесте ниже.
fn small_fixture() -> Vec<u8> {
    let mut w = PqwWriter::new(8)
        .unwrap()
        .hyperparams(0.5, 0.25, 0.75, 0.125);
    w.add_phase(1, 0.5).unwrap();
    w.add_phase(3, -1.0).unwrap();
    w.add_phase(5, 0.0).unwrap();
    w.to_bytes().unwrap()
}

#[test]
fn golden_layout_exact_bytes() {
    let b = small_fixture();
    assert_eq!(b.len(), 137); // 0x80 + 6 топология + 3 фазы

    // 0x00..0x10: magic + version + d_pol
    assert_eq!(&b[0..8], b"POLER_QW");
    assert_eq!(u32::from_le_bytes(b[8..12].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(b[12..16].try_into().unwrap()), 8);

    // 0x10..0x20: гиперпараметры (f32 LE)
    assert_eq!(f32::from_le_bytes(b[0x10..0x14].try_into().unwrap()), 0.5);
    assert_eq!(f32::from_le_bytes(b[0x14..0x18].try_into().unwrap()), 0.25);
    assert_eq!(f32::from_le_bytes(b[0x18..0x1C].try_into().unwrap()), 0.75);
    assert_eq!(f32::from_le_bytes(b[0x1C..0x20].try_into().unwrap()), 0.125);

    // 0x20..0x40: McWeeny-подпись (residual + digest)
    // Максимум |λ²−λ| даёт явный нуль (λ = 1/2 → 0.25).
    assert_eq!(f64::from_le_bytes(b[0x20..0x28].try_into().unwrap()), 0.25);
    let digest = pqw::sha256::sha256_trunc24(&b[0x80..]);
    assert_eq!(&b[0x28..0x40], &digest[..]);

    // 0x40..0x80: таблица смещений
    assert_eq!(u64::from_le_bytes(b[0x40..0x48].try_into().unwrap()), 0x80);
    assert_eq!(u64::from_le_bytes(b[0x48..0x50].try_into().unwrap()), 6);
    assert_eq!(u64::from_le_bytes(b[0x50..0x58].try_into().unwrap()), 0x86);
    assert_eq!(u64::from_le_bytes(b[0x58..0x60].try_into().unwrap()), 3);
    assert_eq!(u64::from_le_bytes(b[0x60..0x68].try_into().unwrap()), 3);
    assert_eq!(u64::from_le_bytes(b[0x68..0x70].try_into().unwrap()), 3); // INDEX16 | CURVATURE
    assert_eq!(u64::from_le_bytes(b[0x70..0x78].try_into().unwrap()), 0); // reserved
    assert_eq!(
        u64::from_le_bytes(b[0x78..0x80].try_into().unwrap()),
        pqw::checksum::fnv1a64(&b[..0x78])
    );

    // 0x80..: топология u16 LE [1, 3, 5] + фазовые блоки
    assert_eq!(&b[0x80..0x86], &[1, 0, 3, 0, 5, 0]);
    // (Pos, σ=32) | (Neg, σ=63) | (Zero, σ=0)
    assert_eq!(&b[0x86..0x89], &[130, 252, 1]);
}

#[test]
fn random_roundtrip_within_quant_bound() {
    let mut rng = Rng::new(12345);
    let d = 512;
    let mut w = PqwWriter::new(d).unwrap();
    let mut orig: Vec<(u32, f32)> = Vec::new();
    let mut used = std::collections::HashSet::new();
    while orig.len() < 100 {
        let i = (rng.next_u64() % u64::from(d)) as u32;
        if !used.insert(i) {
            continue;
        }
        let p = (rng.unit() * 2.0 - 1.0) as f32;
        w.add_phase(i, p).unwrap();
        orig.push((i, p));
    }
    let bytes = w.to_bytes().unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();

    let dec: Vec<(u32, f64)> = r.decoded().collect();
    assert_eq!(dec.len(), 100);
    assert!(dec.windows(2).all(|w| w[0].0 < w[1].0)); // индексы возрастают

    for (i, p) in &orig {
        let found = dec.iter().find(|(idx, _)| idx == i).unwrap();
        assert!(
            (found.1 - f64::from(*p)).abs() <= pqw::QUANT_EPS + 1e-9,
            "arc {i}: p_hat = {}, p = {p}",
            found.1
        );
    }
    assert!(r.verify_payload().is_ok());
}

#[test]
fn deterministic_writes() {
    assert_eq!(small_fixture(), small_fixture());
}

#[test]
fn indices_sorted_after_shuffled_inserts() {
    let mut rng = Rng::new(99);
    let d = 256;
    let mut idxs: Vec<u32> = (0..40)
        .map(|_| (rng.next_u64() % u64::from(d)) as u32)
        .collect();
    idxs.sort();
    idxs.dedup();
    let expected = idxs.clone();

    let mut w = PqwWriter::new(d).unwrap();
    let mut pool = idxs;
    let mut rng2 = Rng::new(7);
    while !pool.is_empty() {
        let k = (rng2.next_u64() % pool.len() as u64) as usize;
        let i = pool.remove(k);
        w.add_phase(i, 0.5).unwrap();
    }

    let out = w.to_bytes().unwrap();
    let r = PqwReader::from_bytes(&out).unwrap();
    assert_eq!(r.indices().to_vec(), expected);
}

#[test]
fn lens_pruning_by_epsilon() {
    let mut w = PqwWriter::new(4).unwrap().hyperparams(0.01, 0.1, 0.99, 0.1);
    w.add_state(&[0.05, -0.2, 0.0, 0.5]).unwrap();
    let bytes = w.to_bytes().unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();

    // 0.05 и 0.0 отсечены LENS (|p| < eps = 0.1)
    assert_eq!(r.nnz(), 2);
    let dec: Vec<(u32, f64)> = r.decoded().collect();
    assert_eq!(dec[0].0, 1);
    assert!((dec[0].1 - (-13.0 / 63.0)).abs() < 1e-12); // −0.2 → σ = 13
    assert_eq!(dec[1].0, 3);
    assert!((dec[1].1 - 32.0 / 63.0).abs() < 1e-12); // 0.5 → σ = 32
    assert_eq!(r.hyperparams().epsilon_threshold, 0.1);
}

#[test]
fn explicit_zero_arc_is_stored_but_dense_zeros_are_pruned() {
    // Явная дуга с p = 0 хранится (трит Zero):
    let mut w = PqwWriter::new(4)
        .unwrap()
        .hyperparams(0.01, 0.1, 0.99, 0.05);
    w.add_phase(2, 0.0).unwrap();
    w.add_phase(0, 0.3).unwrap();
    let bytes = w.to_bytes().unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert_eq!(r.nnz(), 2);
    let zero_arc = r.arcs().find(|a| a.index == 2).unwrap();
    assert_eq!(zero_arc.phase.trit(), Trit::Zero);
    assert_eq!(zero_arc.phase.p(), 0.0);

    // Плотное состояние: нули отсекаются LENS:
    let mut w2 = PqwWriter::new(4)
        .unwrap()
        .hyperparams(0.01, 0.1, 0.99, 0.05);
    w2.add_state(&[0.0; 4]).unwrap();
    let bytes2 = w2.to_bytes().unwrap();
    let r2 = PqwReader::from_bytes(&bytes2).unwrap();
    assert_eq!(r2.nnz(), 0);
}

#[test]
fn dense_state_d4096_index16() {
    let mut rng = Rng::new(2024);
    let d = 4096usize;
    let mut state = vec![0.0_f32; d];
    for v in state.iter_mut() {
        let mut p = (rng.unit() * 2.0 - 1.0) as f32;
        if p.abs() < 0.05 {
            p = 0.5; // все дуги значимы — проверяем плотный случай
        }
        *v = p;
    }
    let mut w = PqwWriter::new(d as u32).unwrap();
    w.add_state(&state).unwrap();
    let bytes = w.to_bytes().unwrap();

    assert_eq!(bytes.len(), 0x80 + 2 * d + d); // 12416
    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert_eq!(r.nnz(), d as u64);
    assert_eq!(r.header().topology_len, (2 * d) as u64);

    let dec: Vec<(u32, f64)> = r.decoded().collect();
    for k in [0usize, 1, 100, 2047, 4095] {
        let (i, p_hat) = dec[k];
        assert_eq!(i as usize, k);
        assert!((p_hat - f64::from(state[k])).abs() <= pqw::QUANT_EPS + 1e-9);
    }
}

#[test]
fn forced_index32_mode() {
    let mut w = PqwWriter::new(8).unwrap().force_index32();
    w.add_phase(1, 0.5).unwrap();
    w.add_phase(3, -0.5).unwrap();
    let bytes = w.to_bytes().unwrap();
    assert_eq!(u64::from_le_bytes(bytes[0x48..0x50].try_into().unwrap()), 8); // 2 × u32
    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert!(!r.header().flags.index16());
    assert_eq!(r.indices().to_vec(), vec![1, 3]);
}

#[test]
fn large_dimension_forces_u32() {
    let d = 65537; // > 65536 → u16 невозможен
    let mut w = PqwWriter::new(d).unwrap();
    w.add_phase(65536, 0.9).unwrap();
    w.add_phase(0, -0.9).unwrap();
    let bytes = w.to_bytes().unwrap();
    assert_eq!(u64::from_le_bytes(bytes[0x48..0x50].try_into().unwrap()), 8);
    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert_eq!(r.indices().to_vec(), vec![0, 65536]);
}

#[test]
fn writer_rejects_bad_input() {
    let mut w = PqwWriter::new(8).unwrap();
    assert!(matches!(
        w.add_phase(8, 0.5),
        Err(pqw::PqwError::BadIndex { .. })
    ));
    assert!(matches!(
        w.add_phase(0, 1.5),
        Err(pqw::PqwError::BadValue(_))
    ));
    assert!(matches!(
        w.add_phase(0, -1.2),
        Err(pqw::PqwError::BadValue(_))
    ));
    assert!(matches!(
        w.add_phase(0, f32::NAN),
        Err(pqw::PqwError::BadValue(_))
    ));
    w.add_phase(1, 0.5).unwrap();
    assert!(matches!(
        w.add_phase(1, 0.5),
        Err(pqw::PqwError::DuplicateIndex(_))
    ));

    assert!(matches!(
        PqwWriter::new(0),
        Err(pqw::PqwError::BadDimension(_))
    ));

    let mut w2 = PqwWriter::new(4).unwrap();
    assert!(matches!(
        w2.add_state(&[0.0; 5]),
        Err(pqw::PqwError::StateLen { .. })
    ));

    // NaN в гиперпараметрах ловится при сборке
    let mut w3 = PqwWriter::new(4)
        .unwrap()
        .hyperparams(f32::NAN, 0.1, 0.99, 0.05);
    w3.add_phase(0, 0.5).unwrap();
    assert!(matches!(w3.to_bytes(), Err(pqw::PqwError::BadValue(_))));
}

#[test]
fn empty_state_file_is_valid() {
    let mut w = PqwWriter::new(4)
        .unwrap()
        .hyperparams(0.01, 0.1, 0.99, 0.05);
    w.add_state(&[0.0; 4]).unwrap();
    let bytes = w.to_bytes().unwrap();
    assert_eq!(bytes.len(), 0x80);

    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert_eq!(r.nnz(), 0);
    assert_eq!(r.arcs().count(), 0);
    assert_eq!(r.mcweeny_residual(), 0.0);
    assert!(r.verify_payload().is_ok()); // digest пустого payload
}

#[test]
fn file_size_formula() {
    let mut rng = Rng::new(555);
    let d = 1000;
    let mut w = PqwWriter::new(d).unwrap();
    let mut used = std::collections::HashSet::new();
    while used.len() < 300 {
        used.insert((rng.next_u64() % u64::from(d)) as u32);
    }
    for &i in &used {
        w.add_phase(i, 0.7).unwrap();
    }
    let bytes = w.to_bytes().unwrap();
    // INDEX16: 128 + 2·nnz (топология) + 1·nnz (фазы)
    assert_eq!(bytes.len(), 0x80 + 2 * 300 + 300);
}

#[test]
fn zero_copy_phase_slice() {
    let bytes = small_fixture();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    let pb = r.phase_bytes();
    assert_eq!(pb, &[130, 252, 1]);
    // Срез заимствуется из исходного буфера без копирования.
    assert_eq!(pb.as_ptr() as usize, bytes.as_ptr() as usize + 0x86);
}

#[test]
fn indices_borrowed_in_u32_mode() {
    use std::borrow::Cow;

    let mut w = PqwWriter::new(65537).unwrap();
    w.add_phase(65536, 0.5).unwrap();
    w.add_phase(0, 0.5).unwrap();
    let bytes = w.to_bytes().unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();

    let aligned = (bytes.as_ptr() as usize + 0x80) % std::mem::align_of::<u32>() == 0;
    match r.indices() {
        Cow::Borrowed(v) => {
            assert!(aligned, "borrowed requires alignment");
            assert_eq!(v, &[0u32, 65536]);
        }
        Cow::Owned(v) => {
            assert!(!aligned, "owned fallback only for unaligned buffers");
            assert_eq!(v, &[0u32, 65536]);
        }
    }
}

#[cfg(unix)]
#[test]
fn mmap_zero_copy_roundtrip() {
    use pqw::Mmap;
    use std::path::Path;

    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
    let path = dir.join("pqw_mmap_roundtrip.poler");
    let bytes = small_fixture();
    std::fs::write(&path, &bytes).unwrap();

    let map = Mmap::open(Path::new(&path)).unwrap();
    let r = PqwReader::from_bytes(map.as_slice()).unwrap();
    assert_eq!(r.nnz(), 3);
    assert_eq!(r.indices().to_vec(), vec![1, 3, 5]);

    let dec: Vec<(u32, f64)> = r.decoded().collect();
    assert!((dec[0].1 - 32.0 / 63.0).abs() < 1e-12);
    assert_eq!(dec[1].1, -1.0);
    assert_eq!(dec[2].1, 0.0);
    assert!(r.verify_payload().is_ok());

    drop(r);
    drop(map);
    std::fs::remove_file(&path).unwrap();
}
