#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! E2E-тесты Zero-Disk Streaming Ingestion Pipeline (v0.39.0,
//! docs/DIRECTIVE_STREAMING_INGESTION_PIPELINE.md, §5).
//!
//! Критерии приёмки:
//! 1. **Zero Raw Disk Footprint** — сырые байты НИКОГДА не касаются ФС;
//!    проверяется: после записи существуют только `.poler` (+ временные
//!    `.part`-файлы удалены), размер архива < сырого потока.
//! 2. **Bounded RAM ≤ 48 МиБ** — пик RSS сообщается `StreamWriteStats`
//!    (VmHWM); жёсткий ассерт — в release (debug-раннер тестов сам
//!    по себе прожорлив), в debug — мягкий отчёт.
//! 3. **Lossless Recovery** — SHA256 восстановленного потока == исходному.
//! 4. **Crystal Ingestion** — .poler → .t5c без распаковки.
//!
//! Гигантские сценарии (2 GiB / 10 GiB) помечены #[ignore]:
//!     cargo test --release --test stream_ingest_e2e -- --ignored
//! Обычный прогон (`cargo test`) использует компактные размеры.

use poler_engine::archive::reader::PolerReader;
use poler_engine::archive::stream_writer::{
    write_stream, StreamWriter, SyntheticStream, StreamWriteConfig,
};
use poler_engine::pqc::sha256::{hex, Sha256};
use poler_engine::triune::{IngestConfig, StreamCrystalBuilder};

use std::io::Read;

fn cfg_quiet() -> StreamWriteConfig {
    StreamWriteConfig { progress_bytes: 0, ..Default::default() }
}

/// Полный SHA256 синтетического потока (независимый эталон).
fn synthetic_sha256(total: u64, seed: u64) -> [u8; 32] {
    let mut s = SyntheticStream::new(total, seed);
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = s.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    h.finalize()
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("poler-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ─────────────────── 1-3: компактный сквозной прогон ───────────────────

/// Синтетика 64 МиБ → .poler → verify (SHA256 потока) → чтение
/// случайных диапазонов → распаковка → сравнение sha256 записей.
#[test]
fn lossless_roundtrip_synthetic_64mib() {
    let total: u64 = 64 * 1024 * 1024;
    let dir = temp_dir("rt64");
    let poler = dir.join("synth64.poler");

    let stats = write_stream(
        SyntheticStream::new(total, 0xE2E5),
        &poler,
        cfg_quiet(),
        "synth.bin",
    )
    .unwrap();
    assert_eq!(stats.total_raw, total);
    assert!(stats.total_stored < total, "архив обязан быть меньше сырья");
    // zero-disk: временные файлы убраны
    assert!(!poler.with_extension("poler.part").exists());
    assert!(!dir.join("synth64.poler.part.files").exists());

    let reader = PolerReader::open(&poler).unwrap();
    // 3. Lossless: полный поток == эталон
    let mut h = Sha256::new();
    let n = reader
        .stream_out(&mut std::io::BufWriter::new(
            std::fs::File::create(dir.join("copy.bin")).unwrap(),
        ))
        .unwrap();
    assert_eq!(n, total);
    // копия на диск — артефакт ТЕСТА (сравнение), не пайплайна
    let mut f = std::fs::File::open(dir.join("copy.bin")).unwrap();
    let mut buf = vec![0u8; 512 * 1024];
    loop {
        let r = f.read(&mut buf).unwrap();
        if r == 0 {
            break;
        }
        h.update(&buf[..r]);
    }
    let got = hex(&h.finalize());
    let want = hex(&synthetic_sha256(total, 0xE2E5));
    assert_eq!(got, want, "SHA256 потока совпал бит-в-бит");

    // случайный доступ на границах чанков
    let mut out = Vec::new();
    reader.read_range(0, 17, &mut out).unwrap();
    assert_eq!(out.len(), 17);
    reader.read_range(total - 9, 9, &mut out).unwrap();
    assert_eq!(out.len(), 9);

    // verify-отчёт
    let rep = reader.verify().unwrap();
    assert!(rep.all_ok, "verify: {:?}", rep.files_bad);

    let _ = std::fs::remove_dir_all(&dir);
}

/// tar с гетерогенными файлами → .poler → таблица файлов → распаковка →
/// sha256 каждого файла == эталону (критерий 3 для записей).
#[test]
fn lossless_tar_entries() {
    let dir = temp_dir("tar");
    // файлы: текст (сжимается), случайный (STORE), пустой, кириллица
    let mut rnd = Vec::with_capacity(300 * 1024);
    let mut st = 42u64;
    while rnd.len() < 300 * 1024 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1);
        rnd.extend_from_slice(&st.to_le_bytes());
    }
    let text = "квантовая решётка тритов держит синтаксис живой речи\n".repeat(4096);

    let mut tar_bytes = Vec::new();
    {
        let mut b = tar::Builder::new(&mut tar_bytes);
        let mut put = |name: &str, data: &[u8]| {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            b.append_data(&mut h, name, data).unwrap();
        };
        put("docs/readme.txt", text.as_bytes());
        put("data/random.bin", &rnd);
        put("empty.txt", b"");
        put("docs/кіриллица.md", "кристал знань памятає слова світу\n".as_bytes());
        b.finish().unwrap();
    }
    let poler = dir.join("docs.poler");
    let stats = write_stream(&tar_bytes[..], &poler, cfg_quiet(), "docs.tar").unwrap();
    assert!(stats.tar_mode, "tar детектирован");
    assert_eq!(stats.files, 4, "четыре записи в таблице");

    let reader = PolerReader::open(&poler).unwrap();
    let names: Vec<&str> = reader.files().iter().map(|f| f.name.as_str()).collect();
    for expect in ["docs/readme.txt", "data/random.bin", "empty.txt", "docs/кіриллица.md"] {
        assert!(names.contains(&expect), "записи {expect} нет в {names:?}");
    }
    // zip-slip защита не нужна тут, но распаковка обязана пройти sha256
    let out_dir = dir.join("unpacked");
    let rep = reader.extract_all(&out_dir).unwrap();
    assert_eq!(rep.files_written, 4);
    assert_eq!(rep.files_ok, 4, "все sha256 сошлись: {:?}", rep.files_bad);
    // содержимое текстовой записи
    let got = std::fs::read(out_dir.join("docs/readme.txt")).unwrap();
    assert_eq!(got, text.as_bytes());

    let _ = std::fs::remove_dir_all(&dir);
}

// ─────────────────── 4: кристалл из .poler ───────────────────

#[test]
fn crystal_ingest_from_poler_archive() {
    let dir = temp_dir("crystal");
    // текстовый tar (мелкие файлы — нарезка CDC останется одной записью,
    // поэтому один большой текстовый файл)
    let corpus = "мозг мухи держит ритм мысли \
        вихрь крутит смысл по кругу жизни \
        кристалл хранит слова мира в тритах \
        ротор разводит лексику по архетипам \
        синусоида внимания дышит гомеостазом \
        квантовая решётка живёт без умножения"
        .repeat(64);
    let mut tar_bytes = Vec::new();
    {
        let mut b = tar::Builder::new(&mut tar_bytes);
        let mut h = tar::Header::new_gnu();
        h.set_size(corpus.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, "corpus.txt", corpus.as_bytes()).unwrap();
        b.finish().unwrap();
    }
    let poler = dir.join("corpus.poler");
    write_stream(&tar_bytes[..], &poler, cfg_quiet(), "corpus.tar").unwrap();

    // .poler → кристалл без распаковки
    let reader = PolerReader::open(&poler).unwrap();
    let mut builder = StreamCrystalBuilder::new(IngestConfig {
        vocab: 4096,
        dims: 64,
        theta_hi: 1.7,
        theta_lo: 0.5,
        chunk_bytes: 64 * 1024,
        word_cap: 1 << 20,
        bigram_cap: 1 << 21,
    });
    let fed = builder.feed_poler(&reader).unwrap();
    assert_eq!(fed, 1, "одна текстовая запись скормлена");
    let (crystal, stats) = builder.finalize().unwrap();
    assert!(stats.total_words > 300, "слова посчитаны: {}", stats.total_words);
    assert!(crystal.id_of("кристалл").is_some());
    assert!(crystal.id_of("вихрь").is_some());
    let t5c = dir.join("memory.t5c");
    crystal.save(&t5c).unwrap();
    assert!(t5c.metadata().unwrap().len() > 0);

    // кристалл перечитывается и отвечает
    let bytes = std::fs::read(&t5c).unwrap();
    let re = poler_engine::triune::Crystal::load(&bytes, 64).unwrap();
    assert!(re.id_of("мозг").is_some());

    let _ = std::fs::remove_dir_all(&dir);
}

// ─────────────────── приёмочные гиганты ───────────────────

/// 2 GiB: пик RSS процесса при потоковой записи ≤ 48 МиБ.
#[test]
#[ignore = "гигантский сценарий: cargo test --release -- --ignored"]
fn rss_budget_2gib() {
    let total: u64 = 2 * 1024 * 1024 * 1024;
    let dir = temp_dir("rss2g");
    let poler = dir.join("giant.poler");
    let stats = write_stream(
        SyntheticStream::new(total, 0x5555),
        &poler,
        cfg_quiet(),
        "giant.bin",
    )
    .unwrap();
    assert_eq!(stats.total_raw, total);
    eprintln!(
        "rss-budget: пик RSS {} КиБ (бюджет {} КиБ), ratio {:.3}",
        stats.peak_rss_kb,
        48 * 1024,
        stats.ratio
    );
    if !cfg!(debug_assertions) {
        assert!(
            stats.peak_rss_kb <= 48 * 1024,
            "пик RSS {} КиБ превысил бюджет 48 МиБ",
            stats.peak_rss_kb
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Полный приёмочный сценарий директивы: 10 GiB синтетики на стеснённом
/// диске, zero-disk запись, lossless SHA256, бюджет RAM, скорость пайплайна.
#[test]
#[ignore = "гигантский сценарий: cargo test --release -- --ignored"]
fn acceptance_10gib_zero_disk() {
    let total: u64 = 10 * 1024 * 1024 * 1024;
    let dir = temp_dir("acc10g");

    // свободное место: архив ~×0.15-0.25 от сырья, сырьё НЕ пишем вообще;
    // порог 2 GiB = архив 10 GiB (~1.5 GiB) + запас на verify/метаданные
    let statvfs = tempdir_free_bytes();
    if statvfs < 2 * 1024 * 1024 * 1024 {
        eprintln!("acceptance: свободно {statvfs} байт < 2 GiB — пропуск (гигант требует ~1.5-2.5 GiB под архив)");
        return;
    }

    // 1-2. запись: сырьё генерируется на лету, на диск не попадает
    let poler = dir.join("huge.poler");
    let stats = write_stream(
        SyntheticStream::new(total, 0xACCE),
        &poler,
        cfg_quiet(),
        "huge.bin",
    )
    .unwrap();
    assert_eq!(stats.total_raw, total);
    assert!(
        stats.total_stored < total / 2,
        "10 GiB синтетики обязаны ужаться минимум вдвое (получено {})",
        stats.total_stored
    );
    if !cfg!(debug_assertions) {
        assert!(
            stats.peak_rss_kb <= 48 * 1024,
            "бюджет RAM 48 МиБ пробит: {} КиБ",
            stats.peak_rss_kb
        );
    }
    eprintln!(
        "acceptance: 10 GiB → {} ({:.1}% от сырья), RSS {} КиБ, {} МБ/с",
        poler_engine::archive::fmt_bytes(stats.total_stored),
        stats.ratio * 100.0,
        stats.peak_rss_kb,
        stats.throughput_mbs
    );

    // 3. lossless: verify() делает полный SHA256 потока и сверяет с
    // трейлером (трейлер посчитан независимым прогоном при записи)
    let reader = PolerReader::open(&poler).unwrap();
    let rep = reader.verify().unwrap();
    assert!(rep.all_ok, "10 GiB lossless: {:?}", rep.files_bad);

    let _ = std::fs::remove_dir_all(&dir);
}

/// Свободное место в tempdir (байты), оценка через df(1);
/// при недоступности df — не блокируем сценарий.
fn tempdir_free_bytes() -> u64 {
    let out = std::process::Command::new("df")
        .arg("-B1")
        .arg(std::env::temp_dir())
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout);
            // последняя колонка Available второй строки
            s.lines()
                .nth(1)
                .and_then(|l| l.split_whitespace().nth(3).and_then(|v| v.parse().ok()))
                .unwrap_or(u64::MAX / 4)
        }
        _ => u64::MAX / 4,
    }
}

/// Детерминизм пайплайна: один поток дважды → побитово одинаковые .poler.
#[test]
fn pipeline_is_deterministic() {
    let dir = temp_dir("det");
    let a = dir.join("a.poler");
    let b = dir.join("b.poler");
    let cfg = cfg_quiet();
    write_stream(SyntheticStream::new(32 * 1024 * 1024, 7), &a, cfg.clone(), "s.bin").unwrap();
    write_stream(SyntheticStream::new(32 * 1024 * 1024, 7), &b, cfg, "s.bin").unwrap();
    let ba = std::fs::read(&a).unwrap();
    let bb = std::fs::read(&b).unwrap();
    assert_eq!(ba, bb, "одинаковые потоки дают побитово одинаковые контейнеры");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Incremental push_bytes порциями нечётного размера == цельному потоку.
#[test]
fn incremental_push_matches_bulk() {
    let dir = temp_dir("inc");
    let a = dir.join("a.poler");
    let b = dir.join("b.poler");
    let cfg = cfg_quiet();
    write_stream(SyntheticStream::new(24 * 1024 * 1024, 3), &a, cfg.clone(), "s.bin").unwrap();
    let mut w = StreamWriter::open(&b, cfg).unwrap();
    let mut src = SyntheticStream::new(24 * 1024 * 1024, 3);
    let mut buf = vec![0u8; 133 * 1024 + 7];
    loop {
        let n = src.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        w.push_bytes(&buf[..n]).unwrap();
    }
    w.finish("s.bin").unwrap();
    assert_eq!(
        std::fs::read(&a).unwrap(),
        std::fs::read(&b).unwrap(),
        "порционная подача не меняет контейнер"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
