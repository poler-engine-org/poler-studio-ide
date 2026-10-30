#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Формат v0.2 (Packed4): плотные упакованные триты, 4 дуги на байт,
//! magic `POLER_Q2`. Проверяются: битовая раскладка, round-trip,
//! ровно 4-кратное сжатие фазовых блоков, валидация повреждений,
//! каноническая sparse-семантика и паритет квантового поведения с v1.

mod common;

use common::Rng;
use pqw::phase::{nearest_trit, pack_quad, unpack_quad};
use pqw::{
    Flags, Header, HyperParams, PqwError, PqwReader, PqwWriter, Trit, TritEncoding,
    FORMAT_VERSION_V2, HEADER_SIZE, MAGIC_V2,
};

fn re_checksum(b: &mut [u8]) {
    let c = pqw::checksum::fnv1a64(&b[..0x78]);
    b[0x78..0x80].copy_from_slice(&c.to_le_bytes());
}

/// Тритовое состояние с фоном и спайками: дуги (0, +1), (2, −1), (5, +1),
/// слабые сигналы (7, +0.4 → Zero) и чистый фон на остальных позициях.
fn trit_writer(d_pol: u32) -> PqwWriter {
    let mut w = PqwWriter::new(d_pol)
        .unwrap()
        .hyperparams(0.25, 0.5, 0.99, 0.05);
    w.add_phase(0, 1.0).unwrap();
    w.add_phase(2, -1.0).unwrap();
    w.add_phase(5, 1.0).unwrap();
    w.add_phase(7, 0.4).unwrap(); // |p| < 0.5 → трит Zero
    w.add_phase(9, -0.9).unwrap(); // → трит Neg
    w
}

#[test]
fn magic_and_header_fields() {
    let bytes = trit_writer(16).to_bytes_packed().unwrap();
    assert_eq!(&bytes[..8], &MAGIC_V2);
    assert_eq!(&bytes[..8], b"POLER_Q2");

    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert!(r.header().is_packed());
    assert_eq!(r.header().format_version, FORMAT_VERSION_V2);
    assert_eq!(r.encoding(), TritEncoding::Packed4);
    assert_eq!(r.encoding().name(), "packed4");
    assert_eq!(r.d_pol(), 16);
    // Ненулевые триты: (0, +1), (2, −1), (5, +1), (9, −1) — четыре штуки.
    assert_eq!(r.nnz(), 4);
    // Плотные триты: ceil(16/4) = 4 байта, топологии нет.
    assert_eq!(r.header().phase_len, 4);
    assert_eq!(r.header().topology_len, 0);
    assert_eq!(r.header().topology_offset, HEADER_SIZE as u64);
    assert_eq!(r.header().phase_offset, HEADER_SIZE as u64);
    assert_eq!(r.header().flags, Flags::none());
    // Чистые триты лежат точно на многообразии P² = P.
    assert_eq!(r.mcweeny_residual(), 0.0);
    assert_eq!(bytes.len(), HEADER_SIZE + 4);
    r.verify_payload().unwrap();
}

#[test]
fn bit_layout_of_packed_phases() {
    // Дуги 0 (+1), 2 (−1), 5 (+1), 9 (−1); остальные Zero.
    let bytes = trit_writer(16).to_bytes_packed().unwrap();
    let packed = &bytes[HEADER_SIZE..];

    // Байт 0: пары [Pos, Zero, Neg, Zero] = 0b10_00_00_01 = 0x21.
    assert_eq!(packed[0], 0x21);
    // Байт 1: пары [Zero, Pos, Zero, Zero] = 0b00_00_01_00 = 0x04.
    assert_eq!(packed[1], 0x04);
    // Байт 2: дуга 9 (−1) в паре 1 = 0b00_00_10_00 = 0x08.
    assert_eq!(packed[2], 0x08);
    // Байт 3: чистый фон — ноль.
    assert_eq!(packed[3], 0x00);
}

#[test]
fn iter_packed_trits_dense_view() {
    let bytes = trit_writer(16).to_bytes_packed().unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    let trits: Vec<(u32, Trit)> = r.iter_packed_trits().unwrap().collect();
    assert_eq!(trits.len(), 16);
    assert_eq!(trits[0], (0, Trit::Pos));
    assert_eq!(trits[1], (1, Trit::Zero));
    assert_eq!(trits[2], (2, Trit::Neg));
    assert_eq!(trits[5], (5, Trit::Pos));
    assert_eq!(trits[7], (7, Trit::Zero)); // 0.4 → фон
    assert_eq!(trits[9], (9, Trit::Neg));
    assert_eq!(trits[15], (15, Trit::Zero));
}

#[test]
fn sparse_view_matches_nonzero_trits() {
    let bytes = trit_writer(16).to_bytes_packed().unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();

    // Канонический sparse-вид: только ненулевые триты.
    let arcs: Vec<(u32, f64)> = r.decoded().collect();
    assert_eq!(arcs, vec![(0, 1.0), (2, -1.0), (5, 1.0), (9, -1.0)]);
    assert_eq!(r.indices().to_vec(), vec![0, 2, 5, 9]);

    // p̂ = ±1 точно, фазовые углы — 0 и π.
    for &(i, p) in &arcs {
        let arc = r.arcs().find(|a| a.index == i).unwrap();
        assert!((arc.theta() - p.acos()).abs() < 1e-15);
    }
}

#[test]
fn exactly_fourfold_phase_compression() {
    // Плотное тритовое состояние: все d дуг значимы.
    let d = 4096u32;
    let mut w = PqwWriter::new(d).unwrap();
    let mut state = vec![0.0f32; d as usize];
    for (i, v) in state.iter_mut().enumerate() {
        *v = if i % 2 == 0 { 1.0 } else { -1.0 };
    }
    w.add_state(&state).unwrap();

    let v1 = w.to_bytes().unwrap();
    let v2 = w.to_bytes_packed().unwrap();
    let r1 = PqwReader::from_bytes(&v1).unwrap();
    let r2 = PqwReader::from_bytes(&v2).unwrap();

    // Фазовые блоки: d байтов кривизны → d/4 упакованных байтов — ровно 4x.
    assert_eq!(r1.header().phase_len, u64::from(d));
    assert_eq!(r2.header().phase_len, u64::from(d) / 4);
    assert_eq!(r1.header().phase_len, 4 * r2.header().phase_len);

    // Файл целиком: топология исчезла, суммарное сжатие ≥ 4x.
    assert!(
        v2.len() * 4 <= v1.len(),
        "v2 = {} байтов, v1 = {} байтов — нет 4x по файлу",
        v2.len(),
        v1.len()
    );
    assert_eq!(v2.len(), HEADER_SIZE + (d as usize) / 4);
    // Оценка писателя точна.
    assert_eq!(w.estimated_size_packed(), v2.len());
}

#[test]
fn d65536_fits_in_16kib() {
    // Экономия RAM из спецификации v0.2: вектор на 65536 дуг — 16 КиБ.
    let d = 65536u32;
    let mut w = PqwWriter::new(d).unwrap();
    let mut state = vec![0.0f32; d as usize];
    for (i, v) in state.iter_mut().enumerate() {
        *v = if i % 3 == 0 {
            1.0
        } else if i % 3 == 1 {
            -1.0
        } else {
            0.0
        };
    }
    w.add_state(&state).unwrap();
    let bytes = w.to_bytes_packed().unwrap();
    assert_eq!(bytes.len(), HEADER_SIZE + 65536 / 4);
    assert_eq!(bytes.len(), HEADER_SIZE + 16 * 1024);
    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert_eq!(r.nnz(), 65536 - 65536 / 3);
}

#[test]
fn v1_and_v2_same_trit_state_same_semantics() {
    // Одно тритовое состояние: v1 (разрежённо, кривизна σ = 63) и
    // v2 (упакованно) дают идентичные дуги и маргиналы.
    let mut w = PqwWriter::new(12).unwrap();
    for &(i, p) in &[(0u32, 1.0f32), (3, -1.0), (7, 1.0), (11, -1.0)] {
        w.add_phase(i, p).unwrap();
    }
    let v1 = w.to_bytes().unwrap();
    let v2 = w.to_bytes_packed().unwrap();

    let r1 = PqwReader::from_bytes(&v1).unwrap();
    let r2 = PqwReader::from_bytes(&v2).unwrap();
    let d1: Vec<(u32, f64)> = r1.decoded().collect();
    let d2: Vec<(u32, f64)> = r2.decoded().collect();
    assert_eq!(d1, d2);
    // v1 тоже ложится на триты точно: σ = 63 → p̂ = ±1.
    assert_eq!(r1.mcweeny_residual(), 0.0);
}

#[test]
fn bit_reproducible_serialization() {
    // Побитовая воспроизводимость: одинаковые дуги → идентичные байты.
    let a = trit_writer(64).to_bytes_packed().unwrap();
    let b = trit_writer(64).to_bytes_packed().unwrap();
    assert_eq!(a, b);
    // write_packed_trits дописывает в существующий буфер без затирания.
    let mut buf = vec![0xFFu8; 3];
    trit_writer(64).write_packed_trits(&mut buf).unwrap();
    assert_eq!(&buf[..3], &[0xFF, 0xFF, 0xFF]);
    assert_eq!(&buf[3..], &a[..]);
}

#[test]
fn quantization_rule_half_threshold() {
    let mut w = PqwWriter::new(8).unwrap();
    for (i, p) in [(0u32, 0.5f32), (1, 0.49), (2, -0.5), (3, -0.49), (4, 0.99)] {
        w.add_phase(i, p).unwrap();
    }
    let bytes = w.to_bytes_packed().unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    let trits: Vec<(u32, Trit)> = r.iter_packed_trits().unwrap().collect();
    assert_eq!(trits[0].1, Trit::Pos); // 0.5 → +1
    assert_eq!(trits[1].1, Trit::Zero); // 0.49 → 0
    assert_eq!(trits[2].1, Trit::Neg); // −0.5 → −1
    assert_eq!(trits[3].1, Trit::Zero); // −0.49 → 0
    assert_eq!(trits[4].1, Trit::Pos);
    // nearest_trit — то же правило напрямую.
    assert_eq!(nearest_trit(0.5), Trit::Pos);
    assert_eq!(nearest_trit(-0.49), Trit::Zero);
}

#[test]
fn corrupt_packed_trit_rejected() {
    let bytes = trit_writer(16).to_bytes_packed().unwrap();
    // Пара 0b11 в любом байте — ReservedTrit (даже в паддинге).
    let mut bad = bytes.clone();
    bad[HEADER_SIZE] |= 0b11;
    assert!(matches!(
        PqwReader::from_bytes(&bad),
        Err(PqwError::ReservedTrit(_))
    ));
}

#[test]
fn nonzero_padding_beyond_d_rejected() {
    // d = 6: 2 байта упакованных, второй байт несёт дуги 4, 5 и ДВЕ пары
    // паддинга. Ненулевой паддинг — Layout-ошибка.
    let mut w = PqwWriter::new(6).unwrap();
    w.add_phase(0, 1.0).unwrap();
    w.add_phase(5, -1.0).unwrap();
    let mut bytes = w.to_bytes_packed().unwrap();
    assert_eq!(bytes.len(), HEADER_SIZE + 2);
    // Дуга 6 (паддинг-пара 2 второго байта) → Pos: за пределами d_pol.
    bytes[HEADER_SIZE + 1] |= 0b01 << 4;
    // digest в заголовке теперь не сходится, но структурная проверка
    // паддинга должна сработать раньше ленивого digest: пересоберём digest.
    let digest = pqw::sha256::sha256_trunc24(&bytes[HEADER_SIZE..]);
    bytes[0x28..0x40].copy_from_slice(&digest);
    re_checksum(&mut bytes);
    assert!(matches!(
        PqwReader::from_bytes(&bytes),
        Err(PqwError::Layout(_))
    ));
}

#[test]
fn nnz_mismatch_rejected() {
    let mut bytes = trit_writer(16).to_bytes_packed().unwrap();
    // nnz → 5 при фактических 4 ненулевых тритах.
    bytes[0x60..0x68].copy_from_slice(&5u64.to_le_bytes());
    re_checksum(&mut bytes);
    assert!(matches!(
        PqwReader::from_bytes(&bytes),
        Err(PqwError::InconsistentTopology {
            field: "nnz",
            expected: 4,
            actual: 5
        })
    ));
}

#[test]
fn topology_in_v2_rejected() {
    let mut bytes = trit_writer(16).to_bytes_packed().unwrap();
    // Заявлена непустая топология — противоречие плотному формату.
    bytes[0x48..0x50].copy_from_slice(&8u64.to_le_bytes());
    re_checksum(&mut bytes);
    assert!(matches!(
        PqwReader::from_bytes(&bytes),
        Err(PqwError::Layout(_))
    ));
}

#[test]
fn trailing_bytes_rejected() {
    let mut bytes = trit_writer(16).to_bytes_packed().unwrap();
    bytes.push(0);
    assert!(matches!(
        PqwReader::from_bytes(&bytes),
        Err(PqwError::Layout(_))
    ));
}

#[test]
fn phase_len_mismatch_rejected() {
    let mut bytes = trit_writer(16).to_bytes_packed().unwrap();
    // phase_len → 3 при ceil(16/4) = 4.
    bytes[0x58..0x60].copy_from_slice(&3u64.to_le_bytes());
    re_checksum(&mut bytes);
    assert!(matches!(
        PqwReader::from_bytes(&bytes),
        Err(PqwError::InconsistentTopology {
            field: "phase_len",
            expected: 4,
            actual: 3
        })
    ));
}

#[test]
fn iter_packed_trits_requires_v2() {
    // v1-контейнер: плотного массива нет — NotPacked.
    let mut w = PqwWriter::new(8).unwrap();
    w.add_phase(1, 0.5).unwrap();
    let v1 = w.to_bytes().unwrap();
    let r = PqwReader::from_bytes(&v1).unwrap();
    assert!(matches!(r.iter_packed_trits(), Err(PqwError::NotPacked)));
    // v1 по-прежнему читается как раньше.
    assert_eq!(r.encoding(), TritEncoding::Curved);
    assert_eq!(r.nnz(), 1);
}

#[test]
fn backward_compat_v1_unchanged() {
    // Золотой образец v1 из corruption-тестов: топология + байты кривизны.
    let mut w = PqwWriter::new(8)
        .unwrap()
        .hyperparams(0.5, 0.25, 0.75, 0.125);
    w.add_phase(1, 0.5).unwrap();
    w.add_phase(3, -1.0).unwrap();
    w.add_phase(5, 0.0).unwrap();
    let bytes = w.to_bytes().unwrap();
    assert_eq!(&bytes[..8], b"POLER_QW");
    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert!(!r.header().is_packed());
    assert_eq!(r.nnz(), 3);
    assert_eq!(r.indices().to_vec(), vec![1, 3, 5]);
    assert_eq!(r.phase_bytes().len(), 3);
}

#[test]
fn fuzz_roundtrip_random_states() {
    // Случайные состояния: round-trip через nearest_trit стабилен,
    // magic/checksum/digest согласованы, nnz пересчитан верно.
    let mut rng = Rng::new(20260828);
    for case in 0..64 {
        let d = 1 + (rng.next_u64() % 300) as u32;
        let mut w = PqwWriter::new(d).unwrap();
        let mut expected: Vec<(u32, Trit)> = Vec::new();
        for i in 0..d {
            let p = (rng.unit() * 2.0 - 1.0) as f32;
            if p.abs() >= 0.05 {
                w.add_phase(i, p).unwrap();
            }
            let t = nearest_trit(p);
            if t != Trit::Zero {
                expected.push((i, t));
            }
        }
        let bytes = w.to_bytes_packed().unwrap();
        let r = PqwReader::from_bytes(&bytes).unwrap();
        assert_eq!(r.d_pol(), d, "case {case}");
        assert_eq!(r.nnz(), expected.len() as u64, "case {case}");
        r.verify_payload().unwrap();
        let got: Vec<(u32, Trit)> = r
            .iter_packed_trits()
            .unwrap()
            .filter(|&(_, t)| t != Trit::Zero)
            .collect();
        assert_eq!(got, expected, "case {case}");
    }
}

#[test]
fn hyperparams_and_purify_survive_packing() {
    let bytes = trit_writer(16).to_bytes_packed().unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert_eq!(
        r.hyperparams(),
        HyperParams {
            eta: 0.25,
            gamma: 0.5,
            rho: 0.99,
            epsilon_threshold: 0.05
        }
    );
    // Пурификация тритов — тождественная: ±1 и фон — неподвижные точки.
    let purified = r.purified();
    assert_eq!(purified, vec![(0, 1.0), (2, -1.0), (5, 1.0), (9, -1.0)]);
}

#[test]
fn header_roundtrip_through_writer() {
    // Заголовок, собранный писателем, разбирается напрямую.
    let bytes = trit_writer(12).to_bytes_packed().unwrap();
    let h = Header::from_bytes(&bytes).unwrap();
    assert_eq!(h.encoding(), TritEncoding::Packed4);
    assert_eq!(h.phase_len, 3); // ceil(12/4)
    let rebuilt = h.to_bytes();
    assert_eq!(&rebuilt[..HEADER_SIZE], &bytes[..HEADER_SIZE]);
}

#[test]
fn quad_helpers_roundtrip_all_patterns() {
    // Все 81 комбинация тритов упаковывается и распаковывается.
    for a in 0..3 {
        for b in 0..3 {
            for c in 0..3 {
                for e in 0..3 {
                    let q = [trit_of(a), trit_of(b), trit_of(c), trit_of(e)];
                    let byte = pack_quad(q);
                    assert_eq!(unpack_quad(byte).unwrap(), q);
                }
            }
        }
    }
}

fn trit_of(v: u8) -> Trit {
    match v {
        0 => Trit::Neg,
        1 => Trit::Zero,
        _ => Trit::Pos,
    }
}
