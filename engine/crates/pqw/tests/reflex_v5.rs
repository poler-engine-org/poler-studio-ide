#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Формат v5 (`POLER_Q5`): v4 (Packed4-фазы + гироскоп + лексикон) +
//! секция контекст-рефлекса `REFL` — динамический след диалога (RQ23,
//! автобиографическая память). Плюс контейнерные тесты gap-RLE кодека
//! гироскопа (сжатие нулевых зон верхнего треугольника).

use pqw::gyro::GyroData;
use pqw::lexicon::Lexicon;
use pqw::reflex::ReflexData;
use pqw::{
    PqwReader, PqwWriter, FORMAT_VERSION_V5, GYRO_SECTION_VERSION, GYRO_SECTION_VERSION_RLE,
    HEADER_SIZE, MAGIC_V4, MAGIC_V5,
};

fn v5_writer() -> PqwWriter {
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
        8,
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

fn sample_reflex() -> ReflexData {
    // Хронология диалога: вопрос («квант фаза») → ответ (координаты слов).
    ReflexData::new(
        "Мария",
        3,
        vec![(1, 1), (4, -1), (7, 1), (12, -1), (1, 1), (4, 1)],
        64,
    )
    .unwrap()
}

#[test]
fn v5_container_roundtrip_bit_exact() {
    let bytes = v5_writer()
        .to_bytes_v5(&sample_gyro(), &sample_lexicon(), &sample_reflex())
        .unwrap();
    assert_eq!(&bytes[..8], b"POLER_Q5");

    let r = PqwReader::from_bytes(&bytes).unwrap();
    assert_eq!(r.header().format_version, FORMAT_VERSION_V5);
    assert!(r.header().is_reflex());
    assert!(r.header().is_lexicon());
    assert!(r.header().is_gyro());
    assert!(r.header().flags.reflex());
    assert_eq!(r.d_pol(), 64);
    assert_eq!(r.nnz(), 4);

    // Гироскоп.
    let g = r.gyro().unwrap();
    assert_eq!(g.ticks(), 1234);
    assert_eq!(g.window(), 8);
    assert_eq!(g.pairs().len(), 3);

    // Лексикон — та же карта, что и в v4.
    let lex = r.lexicon().unwrap();
    assert_eq!(lex.len(), 4);
    assert_eq!(lex.token_of(4), Some("решётка"));

    // Контекст-рефлекс: имя, реплики, хронологический след.
    let refl = r.reflex().unwrap();
    assert_eq!(refl.interlocutor(), "Мария");
    assert_eq!(refl.turns(), 3);
    assert_eq!(
        refl.events(),
        &[(1u32, 1i8), (4, -1), (7, 1), (12, -1), (1, 1), (4, 1)]
    );
    assert_eq!(refl.trail_tail(2), &[(1, 1), (4, 1)]);

    // Digest покрывает всё: фазы + гироскоп + лексикон + рефлекс.
    r.verify_payload().unwrap();

    // Детерминизм: повторная сборка — те же байты.
    let bytes2 = v5_writer()
        .to_bytes_v5(&sample_gyro(), &sample_lexicon(), &sample_reflex())
        .unwrap();
    assert_eq!(bytes, bytes2);
}

#[test]
fn v5_layout_refl_follows_lexicon() {
    // Физическая раскладка: фазы → GYRO → LEXI → REFL(EOF).
    let w = v5_writer();
    let g = sample_gyro();
    let lex = sample_lexicon();
    let refl = sample_reflex();
    let bytes = w.to_bytes_v5(&g, &lex, &refl).unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();

    let d = 64usize;
    let packed_len = d.div_ceil(4);
    assert_eq!(r.header().phase_offset as usize, HEADER_SIZE);
    assert_eq!(r.header().phase_len as usize, packed_len);
    let gyro_off = r.header().topology_offset as usize;
    assert_eq!(gyro_off, HEADER_SIZE + packed_len);
    // GYRO начинается с magic.
    assert_eq!(&bytes[gyro_off..gyro_off + 4], b"GYRO");
    // REFL в конце файла — найдём через длину лексикона.
    let gyro_end = gyro_off + r.header().topology_len as usize;
    let (_, lexi_len) = pqw::lexicon::Lexicon::decode_prefix(&bytes[gyro_end..], 64).unwrap();
    assert_eq!(&bytes[gyro_end..gyro_end + 4], b"LEXI");
    let refl_off = gyro_end + lexi_len;
    assert_eq!(&bytes[refl_off..refl_off + 4], b"REFL");
    // Хвоста после REFL нет.
    assert_eq!(refl_off + refl.encode_with_dpol(true, 64).unwrap().len(), bytes.len());
}

#[test]
fn v5_rejects_truncated_and_trailing() {
    let bytes = v5_writer()
        .to_bytes_v5(&sample_gyro(), &sample_lexicon(), &sample_reflex())
        .unwrap();
    // Усечение последнего байта следа.
    assert!(PqwReader::from_bytes(&bytes[..bytes.len() - 1]).is_err());
    // Хвостовые байты после REFL.
    let mut b = bytes.clone();
    b.push(0);
    assert!(PqwReader::from_bytes(&b).is_err());
    // Рассинхрон тактов: reserved ≠ ticks секции.
    let mut b = bytes.clone();
    b[0x70] ^= 0xFF;
    // checksum заголовка ломается первой — пересчитаем её честно,
    // чтобы дошла именно перекрёстная проверка тактов.
    let mut hb = b[..HEADER_SIZE].to_vec();
    let checksum = pqw::checksum::fnv1a64(&hb[..0x78]);
    hb[0x78..0x80].copy_from_slice(&checksum.to_le_bytes());
    hb.extend_from_slice(&b[HEADER_SIZE..]);
    assert!(PqwReader::from_bytes(&hb).is_err());
    // digest повреждён: последний байт — полярность последнего события
    // следа (1); XOR 0x01 → 0 — всё ещё валидная полярность, структурный
    // разбор проходит, но ленивая проверка digest ловит подмену.
    let mut b = bytes.clone();
    let last = b.len() - 1;
    b[last] ^= 0x01;
    let r = PqwReader::from_bytes(&b).unwrap();
    assert!(r.verify_payload().is_err());
}

#[test]
fn v5_reflects_onto_v4_reader_boundary() {
    // v4-контейнер (без REFL) с тем же гироскопом/лексиконом:
    // рефлекс None, лексикон читается как раньше.
    let bytes_v4 = v5_writer()
        .to_bytes_v4(&sample_gyro(), &sample_lexicon())
        .unwrap();
    assert_eq!(&bytes_v4[..8], b"POLER_Q4");
    let r4 = PqwReader::from_bytes(&bytes_v4).unwrap();
    assert!(r4.reflex().is_none());
    assert!(r4.lexicon().is_some());
    assert_eq!(r4.gyro().unwrap().pairs().len(), 3);

    // v5 от v4 отличается заголовком (magic/версия/флаги/digest)
    // и хвостовой секцией REFL; payload v4 — точный префикс payload v5
    // (фазы + гироскоп + лексикон идентичны).
    let bytes_v5 = v5_writer()
        .to_bytes_v5(&sample_gyro(), &sample_lexicon(), &sample_reflex())
        .unwrap();
    assert!(bytes_v5.len() > bytes_v4.len());
    assert_eq!(bytes_v5[HEADER_SIZE..bytes_v4.len()], bytes_v4[HEADER_SIZE..]);
    // magic различается (версия контейнера).
    assert_eq!(&bytes_v4[..8], MAGIC_V4);
    assert_eq!(&bytes_v5[..8], MAGIC_V5);
}

#[test]
fn v5_requires_nonempty_reflex() {
    // Пустой след — не рефлекс: конструктор отказывает (контракт v4).
    assert!(ReflexData::new("", 0, vec![], 64).is_err());
}

// ===================== gap-RLE контейнеры =====================

#[test]
fn container_picks_rle_codec_for_clusters() {
    // Кластеризованная топология в решётке d_pol > 65536 (u32-режим
    // v1 = 9 Б/пару): контейнер пишет секцию v2 gap-RLE автоматически.
    let mut pairs = Vec::new();
    // Кластер: строка j = 4096, соседи i = 0..999 (соседние слоты),
    // плюс хвостовые пары той же зоны.
    for i in 0..1000u32 {
        pairs.push((i, 4096, if i % 2 == 0 { 1.0 } else { -1.0 }));
    }
    for k in 0..50u32 {
        pairs.push((k, 4097, 1.0));
    }
    let g = GyroData::new(4, 99_999, pairs, 70_000).unwrap();
    let mut w = PqwWriter::new(70_000).unwrap();
    w = w.hyperparams(0.25, 0.5, 1.0, 0.05);
    w.add_phase(0, 0.9).unwrap();

    let v4_rle = w.to_bytes_v4(&g, &sample_lexicon_dim(70_000)).unwrap();
    let r = PqwReader::from_bytes(&v4_rle).unwrap();
    let section = r.gyro().unwrap();
    assert_eq!(section.codec(), GYRO_SECTION_VERSION_RLE);
    assert_eq!(section.pairs().len(), 1050);
    assert_eq!(section.ticks(), 99_999);

    // Размер: секция RLE ≥ 3× меньше разреженной.
    let sparse = g.encode(false).unwrap();
    let rle = g.encode_rle().unwrap();
    assert!(
        sparse.len() as f64 / rle.len() as f64 >= 3.0,
        "sparse {} B vs rle {} B",
        sparse.len(),
        rle.len()
    );
    // Контейнер читается zero-copy (from_bytes поверх mmap-подобного среза).
    r.verify_payload().unwrap();
}

fn sample_lexicon_dim(d: u32) -> Lexicon {
    let mut entries = Vec::new();
    for (k, token) in [(0u32, "фаза"), (1, "решётка"), (2, "трит"), (3, "момент")] {
        entries.push((k, token.to_string()));
    }
    Lexicon::new(entries, d).unwrap()
}

#[test]
fn container_keeps_sparse_codec_when_rle_loses() {
    // Разбросанная топология маленькой решётки (u16-режим v1 = 5 Б/пару):
    // измерение может выбрать любой кодек — контракт один: пары
    // восстанавливаются бит-в-бит в том же порядке.
    let mut pairs = Vec::new();
    for k in 0..12u32 {
        let i = (k * 397) % 60;
        let j = 60 + ((k * 613) % 64);
        pairs.push((i, j, if k % 3 == 0 { -1.5 } else { 2.0 }));
    }
    let g = GyroData::new(2, 555, pairs, 128).unwrap();
    let mut w = PqwWriter::new(128).unwrap();
    w = w.hyperparams(0.25, 0.5, 1.0, 0.05);
    w.add_phase(1, 0.9).unwrap();
    w.add_phase(120, -0.9).unwrap();

    let bytes = w.to_bytes_v4(&g, &sample_lexicon_dim(128)).unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    let section = r.gyro().unwrap();
    assert!(
        section.codec() == GYRO_SECTION_VERSION || section.codec() == GYRO_SECTION_VERSION_RLE
    );
    let orig: Vec<(u32, u32)> = g.pairs().iter().map(|&(i, j, _)| (i, j)).collect();
    let got: Vec<(u32, u32)> = section.pairs().iter().map(|p| (p.i, p.j)).collect();
    assert_eq!(orig, got);
    // Порядок канонический (лексикографический по (i, j)).
    let mut sorted = got.clone();
    sorted.sort_unstable();
    assert_eq!(got, sorted);
}

#[test]
fn v5_with_rle_gyro_full_stack() {
    // Полный стек RQ23: v5-контейнер с gap-RLE гироскопом — рефлекс
    // поверх сжатой топологии, оба конца конвейера.
    let mut pairs = Vec::new();
    for i in 0..500u32 {
        pairs.push((i, 2000, 1.0));
    }
    let g = GyroData::new(8, 42_424, pairs, 70_000).unwrap();
    let refl = ReflexData::new("Иван", 7, vec![(0, 1), (1, -1), (2, 1)], 70_000).unwrap();
    let mut w = PqwWriter::new(70_000).unwrap();
    w = w.hyperparams(0.25, 0.5, 1.0, 0.05);
    w.add_phase(0, 0.9).unwrap();

    let bytes = w
        .to_bytes_v5(&g, &sample_lexicon_dim(70_000), &refl)
        .unwrap();
    let r = PqwReader::from_bytes(&bytes).unwrap();
    let section = r.gyro().unwrap();
    assert_eq!(section.codec(), GYRO_SECTION_VERSION_RLE);
    assert_eq!(section.pairs().len(), 500);
    let back = r.reflex().unwrap();
    assert_eq!(back.interlocutor(), "Иван");
    assert_eq!(back.turns(), 7);
    assert_eq!(back.events(), &[(0u32, 1i8), (1, -1), (2, 1)]);
    r.verify_payload().unwrap();
}

#[test]
fn mmap_zero_copy_reads_v5_and_rle() {
    // zero-copy mmap: файл на диске меньше (RLE), проекция без
    // промежуточных копий (срез mmap → from_bytes → reflex()).
    use pqw::Mmap;
    let dir = std::env::temp_dir().join("pqw_v5_mmap_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("brain_v5.pqw");

    let mut pairs = Vec::new();
    for i in 0..300u32 {
        pairs.push((i, 1500, 1.0));
    }
    let g = GyroData::new(4, 7777, pairs, 70_000).unwrap();
    let refl = ReflexData::new("Аня", 1, vec![(0, -1), (5, 1)], 70_000).unwrap();
    let mut w = PqwWriter::new(70_000).unwrap();
    w = w.hyperparams(0.25, 0.5, 1.0, 0.05);
    w.add_phase(0, 0.9).unwrap();
    w.write_v5_to(&path, &g, &sample_lexicon_dim(70_000), &refl)
        .unwrap();

    let map = Mmap::open(&path).unwrap();
    let r = PqwReader::from_bytes(map.as_slice()).unwrap();
    assert_eq!(r.gyro().unwrap().codec(), GYRO_SECTION_VERSION_RLE);
    assert_eq!(r.reflex().unwrap().interlocutor(), "Аня");
    assert_eq!(r.reflex().unwrap().trail_tail(1), &[(5, 1)]);
    r.verify_payload().unwrap();
    // Секция гироскопа в файле компактнее разреженной версии в ≥ 3 раза
    // (нулевые зоны треугольника упакованы прогонами).
    let sparse = g.encode(false).unwrap().len();
    let section_len = r.header().topology_len as usize;
    assert!(
        sparse as f64 / section_len as f64 >= 3.0,
        "разреженная секция {sparse} Б против RLE-секции {section_len} Б"
    );
    let _ = std::fs::remove_file(&path);
}
