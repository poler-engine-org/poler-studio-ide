#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Формат v4 (`POLER_Q4`): v3 (Packed4-фазы + гироскоп) + секция
//! лексикона `LEXI` — обратная карта кодировщика (RQ17, L5-генерация).

use pqw::gyro::GyroData;
use pqw::lexicon::Lexicon;
use pqw::{
    PqwReader, PqwWriter, FORMAT_VERSION_V4, HEADER_SIZE, MAGIC_V3, MAGIC_V4,
    TritEncoding,
};

fn v4_writer() -> PqwWriter {
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
        8, // окно
        1234,
        vec![(1, 4, 3.5), (4, 12, -1.0), (7, 12, 2.0)],
        64,
    )
    .unwrap()
}

fn sample_lexicon() -> Lexicon {
    Lexicon::new(
        vec![
            (1, "фаза".to_string()),
            (4, "решётка".to_string()),
            (7, "трит".to_string()),
            (12, "момент".to_string()),
        ],
        64,
    )
    .unwrap()
}

#[test]
fn v4_full_roundtrip() {
    let bytes = v4_writer()
        .to_bytes_v4(&sample_gyro(), &sample_lexicon())
        .unwrap();
    assert_eq!(&bytes[..8], &MAGIC_V4);

    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert!(r.header().is_lexicon());
    assert_eq!(r.header().format_version, FORMAT_VERSION_V4);
    assert_eq!(r.encoding(), TritEncoding::Packed4);
    assert_eq!(r.d_pol(), 64);
    assert_eq!(r.nnz(), 4);

    // Фазы — по правилам v2/v3.
    let arcs: Vec<(u32, f64)> = r.decoded().collect();
    assert_eq!(arcs, vec![(1, 1.0), (4, -1.0), (7, -1.0), (12, 1.0)]);

    // Гироскоп доступен как в v3.
    let g = r.gyro().expect("v4 must expose the gyro section");
    assert_eq!(g.window(), 8);
    assert_eq!(g.ticks(), 1234);
    assert_eq!(g.pairs().len(), 3);

    // Лексикон: обратная карта читается бит-в-бит.
    let lex = r.lexicon().expect("v4 must expose the lexicon section");
    assert_eq!(lex.len(), 4);
    assert_eq!(lex.token_of(1), Some("фаза"));
    assert_eq!(lex.token_of(4), Some("решётка"));
    assert_eq!(lex.token_of(7), Some("трит"));
    assert_eq!(lex.token_of(12), Some("момент"));
    assert_eq!(lex.token_of(5), None);

    // Digest покрывает фазы + гироскоп + лексикон.
    assert!(r.verify_payload().is_ok());
}

#[test]
fn v4_layout_lexicon_follows_gyro() {
    let bytes = v4_writer()
        .to_bytes_v4(&sample_gyro(), &sample_lexicon())
        .unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    let h = r.header();
    let packed_len = 64usize.div_ceil(4);
    assert_eq!(h.phase_offset, HEADER_SIZE as u64);
    assert_eq!(h.phase_len, packed_len as u64);
    // Гироскоп сразу за фазами; лексикон сразу за гироскопом — до EOF.
    assert_eq!(h.topology_offset, (HEADER_SIZE + packed_len) as u64);
    let gyro_end = h.topology_offset + h.topology_len;
    assert_eq!(&bytes[gyro_end as usize..gyro_end as usize + 4], b"LEXI");
    // Точный конец файла: заголовок секции + записи, без хвостов.
    let lex = r.lexicon().unwrap();
    let lexi_len =
        16 + lex.entries().iter().map(|(_, t)| 6 + t.len()).sum::<usize>();
    assert_eq!(bytes.len(), gyro_end as usize + lexi_len);
}

#[test]
fn v4_digest_detects_corruption() {
    let bytes = v4_writer()
        .to_bytes_v4(&sample_gyro(), &sample_lexicon())
        .unwrap();
    // Порча последнего байта токена кириллицы XOR-ом по младшему биту:
    // UTF-8 остаётся валидным (D1 82 «т» → D1 83 «у»), структурные
    // проверки проходят — ловит только digest payload.
    let mut bad = bytes.clone();
    let last = bad.len() - 1;
    bad[last] ^= 0x01;
    let r = PqwReader::from_bytes(&bad).unwrap();
    assert!(r.verify_payload().is_err());
    // Против неискажённого файла digest сходится.
    let clean = PqwReader::from_bytes(&bytes).unwrap();
    assert!(clean.verify_payload().is_ok());
}

#[test]
fn v4_rejects_trailing_bytes() {
    let bytes = v4_writer()
        .to_bytes_v4(&sample_gyro(), &sample_lexicon())
        .unwrap();
    let mut bad = bytes.clone();
    bad.push(0x00);
    assert!(PqwReader::from_bytes(&bad).is_err());
}

#[test]
fn v4_rejects_truncation() {
    let bytes = v4_writer()
        .to_bytes_v4(&sample_gyro(), &sample_lexicon())
        .unwrap();
    assert!(PqwReader::from_bytes(&bytes[..bytes.len() - 1]).is_err());
}

#[test]
fn v3_still_readable_and_v4_magic_distinct() {
    // Регресс: v3-контейнер без лексикона обязан читаться как раньше.
    let bytes = v4_writer().to_bytes_v3(&sample_gyro()).unwrap();
    assert_eq!(&bytes[..8], &MAGIC_V3);
    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert!(r.gyro().is_some());
    assert!(r.lexicon().is_none(), "v3 has no lexicon section");
}

#[test]
fn v4_rejects_lexicon_with_bad_d_pol() {
    // Секция лексикона с чужой размерностью не может пройти валидацию
    // ридера (d_pol сверяется на уровне секции).
    let lex = Lexicon::new(vec![(1, "a".to_string())], 64).unwrap();
    let bytes = v4_writer()
        .to_bytes_v4(&sample_gyro(), &lex)
        .unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert_eq!(r.d_pol(), 64);
    // Перепишем заголовок лексикона: d_pol = 128 при контейнере 64.
    let mut bad = bytes.clone();
    let h = r.header();
    let lexi_start = (h.topology_offset + h.topology_len) as usize;
    bad[lexi_start + 8..lexi_start + 12].copy_from_slice(&128u32.to_le_bytes());
    // checksum заголовка не тронут — портится payload, структурная
    // проверка секции обязана отвергнуть чужую размерность.
    assert!(PqwReader::from_bytes(&bad).is_err());
}
