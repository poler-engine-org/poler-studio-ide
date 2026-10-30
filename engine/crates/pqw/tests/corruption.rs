#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Матрица повреждений: каждая порча байтов должна ловиться ровно тем
//! механизмом целостности, который за неё отвечает.

use pqw::{PqwError, PqwReader};

fn fixture() -> Vec<u8> {
    // d = 8, дуги (1, 0.5), (3, −1.0), (5, 0.0) — как в golden-тесте:
    // топология [1, 3, 5] u16 на 0x80..0x86, фазы [130, 252, 1] на 0x86..0x89.
    let mut w = pqw::PqwWriter::new(8)
        .unwrap()
        .hyperparams(0.5, 0.25, 0.75, 0.125);
    w.add_phase(1, 0.5).unwrap();
    w.add_phase(3, -1.0).unwrap();
    w.add_phase(5, 0.0).unwrap();
    w.to_bytes().unwrap()
}

fn re_checksum(b: &mut [u8]) {
    let c = pqw::checksum::fnv1a64(&b[..0x78]);
    b[0x78..0x80].copy_from_slice(&c.to_le_bytes());
}

fn put_u64(b: &mut [u8], off: usize, v: u64) {
    b[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

#[test]
fn bad_magic() {
    let mut b = fixture();
    b[0] = b'X';
    assert!(matches!(
        PqwReader::from_bytes(&b),
        Err(PqwError::BadMagic(_))
    ));
}

#[test]
fn future_version_rejected() {
    let mut b = fixture();
    b[8..12].copy_from_slice(&2u32.to_le_bytes());
    re_checksum(&mut b);
    assert!(matches!(
        PqwReader::from_bytes(&b),
        Err(PqwError::UnsupportedVersion(2))
    ));
}

#[test]
fn header_bitflip_detected() {
    let mut b = fixture();
    b[0x0C] ^= 0x01; // d_pol без пересчёта checksum
    assert!(matches!(
        PqwReader::from_bytes(&b),
        Err(PqwError::CorruptHeader { .. })
    ));
}

#[test]
fn payload_bitflip_detected_by_verify() {
    let mut b = fixture();
    b[0x86] |= 0b0100_0000; // бит кривизны: структура остаётся валидной
    assert!(PqwReader::from_bytes(&b).is_ok()); // digest проверяется лениво...
    let r = PqwReader::from_bytes(&b).unwrap();
    assert!(matches!(r.verify_payload(), Err(PqwError::CorruptPayload))); // ...но ловит подмену
}

#[test]
fn reserved_trit_detected() {
    let mut b = fixture();
    b[0x86] |= 0b0000_0011; // трит 0b11 зарезервирован
    assert!(matches!(
        PqwReader::from_bytes(&b),
        Err(PqwError::ReservedTrit(_))
    ));
}

#[test]
fn corrupted_topology_index_detected() {
    let mut b = fixture();
    b[0x80] = 0xFE; // индекс 1 → 254 (>= d_pol)
    re_checksum(&mut b);
    assert!(matches!(
        PqwReader::from_bytes(&b),
        Err(PqwError::BadIndex { .. })
    ));
}

#[test]
fn unsorted_topology_detected() {
    let mut b = fixture();
    b[0x82] = 7; // индекс 3 → 7: топология [1, 7, 5] (в границах, но не отсортирована)
    re_checksum(&mut b);
    assert!(matches!(
        PqwReader::from_bytes(&b),
        Err(PqwError::UnsortedTopology)
    ));
}

#[test]
fn truncated_file() {
    let b = fixture();
    assert!(matches!(
        PqwReader::from_bytes(&b[..130]),
        Err(PqwError::Truncated { .. })
    ));
}

#[test]
fn empty_buffer() {
    assert!(matches!(
        PqwReader::from_bytes(&[]),
        Err(PqwError::Truncated { .. })
    ));
}

#[test]
fn trailing_bytes_rejected() {
    let mut b = fixture();
    b.push(0xAA);
    assert!(matches!(
        PqwReader::from_bytes(&b),
        Err(PqwError::Layout(_))
    ));
}

#[test]
fn nonzero_reserved_field_rejected() {
    let mut b = fixture();
    put_u64(&mut b, 0x70, 1);
    re_checksum(&mut b);
    assert!(matches!(
        PqwReader::from_bytes(&b),
        Err(PqwError::ReservedBits { .. })
    ));
}

#[test]
fn reserved_flag_bits_rejected() {
    let mut b = fixture();
    put_u64(&mut b, 0x68, 0b111); // бит 2 и выше зарезервированы
    re_checksum(&mut b);
    assert!(matches!(
        PqwReader::from_bytes(&b),
        Err(PqwError::ReservedBits { .. })
    ));
}

#[test]
fn curvature_flag_required() {
    let mut b = fixture();
    put_u64(&mut b, 0x68, 0b01); // только INDEX16, без CURVATURE
    re_checksum(&mut b);
    assert!(matches!(
        PqwReader::from_bytes(&b),
        Err(PqwError::UnsupportedFlags(_))
    ));
}

#[test]
fn inconsistent_topology_len_detected() {
    let mut b = fixture();
    put_u64(&mut b, 0x48, 8); // должно быть 6 (3 × u16)
    re_checksum(&mut b);
    assert!(matches!(
        PqwReader::from_bytes(&b),
        Err(PqwError::InconsistentTopology { .. })
    ));
}

#[test]
fn digest_mismatch_detected() {
    let mut b = fixture();
    for byte in &mut b[0x28..0x40] {
        *byte = 0xFF;
    }
    re_checksum(&mut b);
    // Структура цела — разбор проходит...
    let r = PqwReader::from_bytes(&b).unwrap();
    // ...но digest не совпадает.
    assert!(matches!(r.verify_payload(), Err(PqwError::CorruptPayload)));
}
