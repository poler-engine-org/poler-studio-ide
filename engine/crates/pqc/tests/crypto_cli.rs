#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Интеграционные тесты CLI `pqc encrypt` / `pqc decrypt` (RQ12–RQ13).
//!
//! По умолчанию — трит-схема GF(3) (RQ13): `p* = a ⊗_ε p* ⊕ m` в
//! Packed4-тритах, прецессия по руслам J + решётка LENS, лавина ~2/3.
//! `--f32` — исследовательская схема RQ12 на фазах f32. Ключ —
//! контейнер v3 с гироскопом J; расшифровка автодетектит магию PQT1/PQC1.

use std::io::Write;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_pqc");

/// Делает ключ .pqw v3: d=256, путь + гребёнка (делокализованные моды).
fn make_key(path: &std::path::Path) {
    // Генерируем ключ через уже проверенный модульный API: пишем
    // контейнер v3 с гироскопом из python-free Rust-теста.
    // Здесь проще всего — зашить готовый контейнер, созданный в тесте.
    use pqw::{GyroData, PqwWriter};
    let mut w = PqwWriter::new(256).unwrap().hyperparams(0.05, 0.5, 0.75, 0.05);
    for i in 0u32..256 {
        let p = if i % 3 == 0 { -0.7 } else { 0.6 };
        w.add_phase(i, p).unwrap();
    }
    let mut pairs: Vec<(u32, u32, f64)> =
        (0u32..255).map(|i| (i, i + 1, 1.0)).collect();
    pairs.extend((0u32..252).map(|i| (i, i + 4, 0.6)));
    let gyro = GyroData::new(256, 5000, pairs, 256).unwrap();
    let bytes = w.to_bytes_v3(&gyro).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pqc-crypto-cli-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(BIN)
        .args(args)
        .env_remove("RUST_BACKTRACE")
        .output()
        .expect("pqc binary");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn cli_encrypt_decrypt_roundtrip() {
    let dir = tmp_dir("roundtrip");
    let key = dir.join("key.pqw");
    make_key(&key);
    let msg = dir.join("msg.bin");
    let cipher = dir.join("msg.pqc");
    let plain = dir.join("plain.bin");
    let payload: Vec<u8> = (0..777u32).map(|i| (i * 37 + 11) as u8).collect();
    std::fs::write(&msg, &payload).unwrap();

    let (code, out, err) = run(&[
        "encrypt",
        msg.to_str().unwrap(),
        "--key",
        key.to_str().unwrap(),
        "--out",
        cipher.to_str().unwrap(),
        "--seed",
        "42",
    ]);
    assert_eq!(code, 0, "encrypt failed: {err}\n{out}");
    // RQ23: дефолт — нелинейная схема v2 (транспорт + спин-раунд GF(3)).
    assert!(out.contains("GF(3)"), "out: {out}");
    assert!(out.contains("спин-раунд"), "out: {out}");
    assert!(out.contains("лавина"), "out: {out}");
    assert!(cipher.exists());

    let (code, out, err) = run(&[
        "decrypt",
        cipher.to_str().unwrap(),
        "--key",
        key.to_str().unwrap(),
        "--out",
        plain.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "decrypt failed: {err}\n{out}");
    assert!(out.contains("RQ13"));
    assert!(out.contains("GF(3)"));
    assert_eq!(std::fs::read(&plain).unwrap(), payload);
}

#[test]
fn cli_encrypt_text_stdin_json() {
    let dir = tmp_dir("text");
    let key = dir.join("key.pqw");
    make_key(&key);
    let cipher = dir.join("t.pqc");

    // --text
    let (code, _out, err) = run(&[
        "encrypt",
        "--text",
        "POLER archetype cipher",
        "--key",
        key.to_str().unwrap(),
        "--out",
        cipher.to_str().unwrap(),
        "--seed",
        "7",
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(cipher.exists());

    // stdout без --out: текст возвращается в терминал
    let (code, out, err) = run(&[
        "decrypt",
        cipher.to_str().unwrap(),
        "--key",
        key.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("POLER archetype cipher"), "out: {out}");

    // --stdin + --json
    let mut child = Command::new(BIN)
        .args([
            "encrypt",
            "--stdin",
            "--key",
            key.to_str().unwrap(),
            "--out",
            cipher.to_str().unwrap(),
            "--seed",
            "9",
            "--json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"stdin message 123")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let json = String::from_utf8_lossy(&out.stdout);
    // Числа в JSON — научная запись (1.7e1); проверяем ключи.
    assert!(json.contains("\"msg_len\":"));
    assert!(json.contains("\"k_modes\":"));
    assert!(json.contains("\"digest\":\""));
}

#[test]
fn cli_wrong_key_rejected_or_garbage() {
    let dir = tmp_dir("wrongkey");
    let key_a = dir.join("a.pqw");
    let key_b = dir.join("b.pqw");
    make_key(&key_a);
    // Другой ключ: другая структура русел (сдвиги {2, 7} вместо {1, 4})
    // — другие моды и другое подпространство. (Инверсия весов НЕ годится:
    // J → −J лишь меняет местами (u, v) плоскостей — проекторы те же.)
    use pqw::{GyroData, PqwWriter};
    let mut w = PqwWriter::new(256).unwrap().hyperparams(0.05, 0.5, 0.75, 0.05);
    for i in 0u32..256 {
        let p = if i % 3 == 0 { 0.7 } else { -0.6 };
        w.add_phase(i, p).unwrap();
    }
    let mut pairs: Vec<(u32, u32, f64)> =
        (0u32..254).map(|i| (i, i + 2, 1.0)).collect();
    pairs.extend((0u32..249).map(|i| (i, i + 7, 0.8)));
    let gyro = GyroData::new(256, 5000, pairs, 256).unwrap();
    std::fs::write(&key_b, w.to_bytes_v3(&gyro).unwrap()).unwrap();

    let msg = dir.join("m.bin");
    let cipher = dir.join("m.pqc");
    std::fs::write(&msg, b"secret message for the archetype").unwrap();
    let (code, _, err) = run(&[
        "encrypt",
        msg.to_str().unwrap(),
        "--key",
        key_a.to_str().unwrap(),
        "--out",
        cipher.to_str().unwrap(),
        "--seed",
        "1",
    ]);
    assert_eq!(code, 0, "{err}");

    let plain = dir.join("p.bin");
    let (code, out, err) = run(&[
        "decrypt",
        cipher.to_str().unwrap(),
        "--key",
        key_b.to_str().unwrap(),
        "--out",
        plain.to_str().unwrap(),
    ]);
    // Чужой ключ: либо отказ (структура/ёмкость/моды), либо мусор.
    if code == 0 {
        let got = std::fs::read(&plain).unwrap();
        assert_ne!(got, b"secret message for the archetype");
        assert!(out.contains("RQ13"));
    } else {
        assert!(!err.is_empty());
    }
}

#[test]
fn cli_corruption_and_missing_args() {
    let dir = tmp_dir("errors");
    let key = dir.join("key.pqw");
    make_key(&key);
    let cipher = dir.join("c.pqc");
    let (code, _, _) = run(&[
        "encrypt",
        "--text",
        "hello",
        "--key",
        key.to_str().unwrap(),
        "--out",
        cipher.to_str().unwrap(),
        "--seed",
        "3",
    ]);
    assert_eq!(code, 0);

    // Порча: digest ловит.
    let mut bytes = std::fs::read(&cipher).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    std::fs::write(&cipher, bytes).unwrap();
    let (code, _, err) = run(&[
        "decrypt",
        cipher.to_str().unwrap(),
        "--key",
        key.to_str().unwrap(),
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("digest"));

    // Без ключа — подсказка.
    let (code, _, err) = run(&["encrypt", "--text", "x", "--out", "/tmp/x.pqc"]);
    assert_eq!(code, 2);
    assert!(err.contains("--key"));

    // Без входа — подсказка.
    let (code, _, err) = run(&[
        "encrypt",
        "--key",
        key.to_str().unwrap(),
        "--out",
        "/tmp/x.pqc",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("--text") || err.contains("stdin"));
}

#[test]
fn cli_f32_legacy_roundtrip() {
    // --f32: исследовательская схема RQ12 (фазы f32, магия PQC1).
    let dir = tmp_dir("f32-legacy");
    let key = dir.join("key.pqw");
    make_key(&key);
    let msg = dir.join("msg.bin");
    let cipher = dir.join("msg.pqc");
    let plain = dir.join("plain.bin");
    let payload: Vec<u8> = (0..512u32).map(|i| (i * 29 + 3) as u8).collect();
    std::fs::write(&msg, &payload).unwrap();

    let (code, out, err) = run(&[
        "encrypt",
        msg.to_str().unwrap(),
        "--key",
        key.to_str().unwrap(),
        "--out",
        cipher.to_str().unwrap(),
        "--seed",
        "11",
        "--f32",
    ]);
    assert_eq!(code, 0, "{err}\n{out}");
    assert!(out.contains("RQ12"), "ожидалась legacy-схема RQ12: {out}");
    let bytes = std::fs::read(&cipher).unwrap();
    assert_eq!(&bytes[..4], b"PQC1", "магия PQC1");

    let (code, out, err) = run(&[
        "decrypt",
        cipher.to_str().unwrap(),
        "--key",
        key.to_str().unwrap(),
        "--out",
        plain.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{err}\n{out}");
    assert!(out.contains("RQ12"));
    assert_eq!(std::fs::read(&plain).unwrap(), payload);
}

#[test]
fn cli_autodetect_both_schemes() {
    // Расшифровка автодетектит схему по магии PQT1/PQC1 без флагов.
    let dir = tmp_dir("autodetect");
    let key = dir.join("key.pqw");
    make_key(&key);
    let payload = b"same payload both schemes".to_vec();

    let trite_cipher = dir.join("t.pqt");
    let f32_cipher = dir.join("t.pqc");
    let trite_args: Vec<&str> = vec!["--seed", "5"];
    let f32_args: Vec<&str> = vec!["--seed", "5", "--f32"];
    for (out_path, extra) in [(&trite_cipher, &trite_args), (&f32_cipher, &f32_args)] {
        let mut args: Vec<&str> = vec![
            "encrypt",
            "--text",
            "same payload both schemes",
            "--key",
            key.to_str().unwrap(),
            "--out",
            out_path.to_str().unwrap(),
        ];
        args.extend_from_slice(extra);
        let (code, _, err) = run(&args);
        assert_eq!(code, 0, "{err}");
    }
    assert_eq!(&std::fs::read(&trite_cipher).unwrap()[..4], b"PQT1");
    assert_eq!(&std::fs::read(&f32_cipher).unwrap()[..4], b"PQC1");
    // Трит-контейнер в ~10 раз меньше f32 на том же сообщении.
    assert!(
        std::fs::read(&trite_cipher).unwrap().len() * 4
            < std::fs::read(&f32_cipher).unwrap().len()
    );

    for cipher in [&trite_cipher, &f32_cipher] {
        let (code, out, err) = run(&[
            "decrypt",
            cipher.to_str().unwrap(),
            "--key",
            key.to_str().unwrap(),
        ]);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("same payload both schemes"), "out: {out}");
    }
    let _ = payload;
}

#[test]
fn cli_trite_json_telemetry() {
    // JSON трит-схемы: схема, измеренная лавина и расширение.
    let dir = tmp_dir("trite-json");
    let key = dir.join("key.pqw");
    make_key(&key);
    let msg = dir.join("msg.bin");
    let cipher = dir.join("m.pqt");
    let payload: Vec<u8> = (0..2048u32).map(|i| (i * 31 + 7) as u8).collect();
    std::fs::write(&msg, &payload).unwrap();

    let (code, out, err) = run(&[
        "encrypt",
        msg.to_str().unwrap(),
        "--key",
        key.to_str().unwrap(),
        "--out",
        cipher.to_str().unwrap(),
        "--seed",
        "3",
        "--json",
    ]);
    assert_eq!(code, 0, "{err}\n{out}");
    assert!(out.contains("\"scheme\":\"trite-gf3\""), "out: {out}");
    assert!(out.contains("\"avalanche\":"), "out: {out}");
    assert!(out.contains("\"expansion\":"), "out: {out}");
    assert!(out.contains("\"ticks\":"), "out: {out}");
    assert!(out.contains("\"digest\":\""), "out: {out}");
    // Расширение ≤ ×2 (заголовок+IV амортизируются на 2 КиБ).
    let exp: f64 = out
        .split("\"expansion\":")
        .nth(1)
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(99.0);
    assert!(exp <= 2.0, "расширение ×{exp} — триты не экономят");
    // Лавина с ТЕМ ЖЕ IV (RQ23): честная диффузия без шума цепочки.
    // Флип в середине 17-блочного текста: CBC-каскад идёт только вперёд,
    // префиксные блоки не тронуты — разведённая лавина ≥ 0.1 при
    // поблочной ~2/3 (см. pqc avalanche — метрика от блока зонда).
    let av: f64 = out
        .split("\"avalanche\":")
        .nth(1)
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0.0);
    assert!(av >= 0.1, "лавина {av} — диффузии нет");
    assert!(av <= 0.72, "лавина {av} выше потолка GF(3)");
    // Схема v2 по умолчанию: спин-раунды активны, версия в отчёте.
    assert!(out.contains("\"version\":2"), "out: {out}");
    assert!(out.contains("\"nl_rounds\":"), "out: {out}");
}
