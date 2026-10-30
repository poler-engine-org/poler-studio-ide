#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Формат v3 (`POLER_Q3`): Packed4-фазы + гироскопная топология
//! `J = A − Aᵀ` в топологической секции (RQ10).

use pqw::gyro::{GyroData, GyroSection};
use pqw::{
    Flags, Header, HyperParams, PqwReader, PqwWriter, FORMAT_VERSION_V3, HEADER_SIZE,
    MAGIC_V3, TritEncoding,
};

fn v3_writer() -> PqwWriter {
    let mut w = PqwWriter::new(64).unwrap();
    w = w.hyperparams(0.25, 0.5, 1.0, 0.05);
    w.add_phase(1, 0.9).unwrap();
    w.add_phase(4, -0.8).unwrap();
    w.add_phase(7, -0.6).unwrap();
    w.add_phase(12, 0.7).unwrap();
    w
}

fn sample_gyro() -> GyroData {
    GyroData::new(
        256, // окно
        77_777,
        vec![(1, 4, 3.5), (4, 12, -1.0), (7, 12, 2.0), (2, 9, 0.5)],
        64,
    )
    .unwrap()
}

#[test]
fn v3_full_roundtrip() {
    let bytes = v3_writer().to_bytes_v3(&sample_gyro()).unwrap();
    assert_eq!(&bytes[..8], &MAGIC_V3);

    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert!(r.header().is_gyro());
    assert_eq!(r.header().format_version, FORMAT_VERSION_V3);
    assert_eq!(r.encoding(), TritEncoding::Packed4);
    assert_eq!(r.d_pol(), 64);
    assert_eq!(r.nnz(), 4);

    // Фазы читаются как в v2: канонический sparse-вид.
    let arcs: Vec<(u32, f64)> = r.decoded().collect();
    assert_eq!(arcs, vec![(1, 1.0), (4, -1.0), (7, -1.0), (12, 1.0)]);
    assert_eq!(r.iter_packed_trits().unwrap().count(), 64);

    // Гироскоп: топологическая секция больше не пуста.
    let g = r.gyro().expect("v3 must expose the gyro section");
    assert_eq!(g.window(), 256);
    assert_eq!(g.ticks(), 77_777);
    assert_eq!(g.pairs().len(), 4);
    // Верхний треугольник, отсортирован по (i,j), знаки сохранены.
    assert_eq!(g.pairs()[0].i, 1);
    assert_eq!(g.pairs()[0].j, 4);
    assert!(g.pairs()[0].weight > 0.0);
    assert!(g.pairs()[2].weight < 0.0); // (4, 12, −1.0)

    // Digest покрывает фазы + гироскоп.
    r.verify_payload().unwrap();
}

#[test]
fn v3_layout_math() {
    // d=64: фазы 16 B; гироскоп — кодек по измерению (RQ23): для этого
    // набора пар gap-RLE (слоты 7, 38, 70, 73 → Δ 7/31/32/3, записи
    // 2 Б) даёт 32 + 8 = 40 B против 52 B разреженного — выбран v2.
    let bytes = v3_writer().to_bytes_v3(&sample_gyro()).unwrap();
    assert_eq!(bytes.len(), HEADER_SIZE + 16 + 32 + 4 * 2);
    let h = Header::from_bytes(&bytes).unwrap();
    assert_eq!(h.phase_offset, HEADER_SIZE as u64);
    assert_eq!(h.phase_len, 16);
    assert_eq!(h.topology_offset, (HEADER_SIZE + 16) as u64);
    assert_eq!(h.topology_len, 40);
    assert_eq!(h.flags, Flags::v3(true));
    assert!(h.flags.gyro());
    // Выбранный кодек — gap-RLE, пары читаются как раньше.
    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert_eq!(r.gyro().unwrap().codec(), pqw::GYRO_SECTION_VERSION_RLE);
    assert_eq!(r.gyro().unwrap().pairs().len(), 4);
}

#[test]
fn v3_deterministic_bytes() {
    let a = v3_writer().to_bytes_v3(&sample_gyro()).unwrap();
    let b = v3_writer().to_bytes_v3(&sample_gyro()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn v3_corruption_detected() {
    let bytes = v3_writer().to_bytes_v3(&sample_gyro()).unwrap();

    // Ломаем вес пары: digest обязан заметить.
    let mut bad = bytes.clone();
    let topo = Header::from_bytes(&bytes).unwrap().topology_offset as usize;
    bad[topo + 32 + 4] ^= 0x40; // квант веса первой пары
    assert!(PqwReader::from_bytes(&bad).is_ok()); // структура цела…
    assert!(PqwReader::from_bytes(&bad).unwrap().verify_payload().is_err());

    // Ломаем счётчик тактов в reserved: перекрёстная проверка обязана сработать
    // (checksum пересчитываем, чтобы дошла именно пара reserved ↔ секция).
    let mut bad = bytes.clone();
    bad[0x70] ^= 0x01;
    let checksum = pqw::checksum::fnv1a64(&bad[..0x78]);
    bad[0x78..0x80].copy_from_slice(&checksum.to_le_bytes());
    assert!(PqwReader::from_bytes(&bad).is_err());

    // Ломаем magic гироскопной секции.
    let mut bad = bytes.clone();
    let topo = Header::from_bytes(&bytes).unwrap().topology_offset as usize;
    bad[topo] = b'X';
    assert!(PqwReader::from_bytes(&bad).is_err());

    // Отрезаем хвост с гироскопом.
    let bad = bytes[..bytes.len() - 5].to_vec();
    assert!(PqwReader::from_bytes(&bad).is_err());

    // Приклеиваем мусорный хвост.
    let mut bad = bytes.clone();
    bad.push(0xAA);
    assert!(PqwReader::from_bytes(&bad).is_err());
}

#[test]
fn v3_empty_gyro_rejected() {
    // Пустой гироскоп — это v2, не v3.
    let err = GyroData::new(256, 1, vec![(1, 2, 0.0)], 64);
    assert!(err.is_err());
}

#[test]
fn v2_reader_still_blind_to_gyro() {
    // v2-контейнер: gyro() → None, топология обязана быть пустой.
    let bytes = v3_writer().to_bytes_packed().unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert!(r.gyro().is_none());
    assert_eq!(r.header().format_version, 2);
}

#[test]
fn v3_u32_indices_when_d_pol_large() {
    // d_pol > 65536 → u32-режим разреженного кодека = 9 Б/пару;
    // gap-RLE пишет слот (≈2.1e9 → varint 5 Б) + квант = 6 Б —
    // измерение выбирает RLE и здесь (RQ23).
    let mut w = PqwWriter::new(70_000).unwrap();
    w.add_phase(65_537, 0.9).unwrap();
    let g = GyroData::new(64, 5, vec![(65_536, 65_537, 1.5)], 70_000).unwrap();
    let bytes = w.to_bytes_v3(&g).unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    let sec = r.gyro().unwrap();
    assert!(!sec.index16());
    assert_eq!(sec.codec(), pqw::GYRO_SECTION_VERSION_RLE);
    assert_eq!(sec.pairs().len(), 1);
    assert_eq!(sec.pairs()[0].i, 65_536);
    assert!((sec.pairs()[0].weight - 1.5).abs() <= 1.5 / 127.0 / 2.0 + 1e-12);
    // Размер: 32 + varint(слот) 5 Б + квант 1 Б = 38.
    assert_eq!(sec_bytes(&bytes), 32 + 6);
}

fn sec_bytes(bytes: &[u8]) -> usize {
    let h = Header::from_bytes(bytes).unwrap();
    h.topology_len as usize
}

#[test]
fn gyro_section_decode_via_public_api() {
    let bytes = v3_writer().to_bytes_v3(&sample_gyro()).unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    let g = r.gyro().unwrap();
    // Деквантованные веса в пределах кванта решётки.
    let orig = sample_gyro();
    let scale = orig.scale() as f64;
    let tol = scale / 127.0 / 2.0 + 1e-12;
    for (p, &(i, j, w)) in g.pairs().iter().zip(orig.pairs()) {
        assert_eq!((p.i, p.j), (i, j));
        assert!(
            (p.weight - w).abs() <= tol,
            "pair ({i},{j}): {} vs {w}",
            p.weight
        );
    }
    // Сечение читается и без читателя: сырые байты.
    let h = r.header();
    let raw = &bytes[h.topology_offset as usize..h.topology_offset as usize + h.topology_len as usize];
    let s2 = GyroSection::decode(raw, h.d_pol, h.flags.index16()).unwrap();
    assert_eq!(s2.pairs().len(), g.pairs().len());
    assert_eq!(s2.ticks(), g.ticks());
    // Гиперпараметры пробрасываются в v3 как в v2.
    assert_eq!(
        r.hyperparams(),
        HyperParams {
            eta: 0.25,
            gamma: 0.5,
            rho: 1.0,
            epsilon_threshold: 0.05,
        }
    );
}
