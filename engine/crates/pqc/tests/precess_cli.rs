#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Интеграционные тесты CLI `pqc precess` (RQ11: уравнение архетипа).
//!
//! Петля прецессии поверх чекпоинта v3: чистая прецессия (орбита),
//! оседание с McWeeny-проекцией (контейнер-архетип), JSON-отчёт,
//! валидность записанного контейнера и отказ на v2 без гироскопа.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn bin_path() -> PathBuf {
    env!("CARGO_BIN_EXE_pqc").into()
}

fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pqc_precess_test_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// Направленный корпус: повторяющиеся переходы «токен A раньше токена B»
/// дают выживающую циркуляцию J в ε-воротах.
fn write_corpus(dir: &std::path::Path) {
    let chain = "phase memory born lens entanglement arcs topology epsilon ";
    let sep = "--- separator line with distinct markers xqzw --- ";
    fs::write(
        dir.join("a.rs"),
        format!("{}\n{}", chain.repeat(60), sep.repeat(30)),
    )
    .unwrap();
    fs::write(
        dir.join("b.md"),
        format!("{}\n{}", chain.repeat(60), sep.repeat(30)),
    )
    .unwrap();
}

/// Обучить маленький v3-контейнер с гироскопом; возвращает путь.
fn train_v3(dir: &std::path::Path) -> PathBuf {
    let out = dir.join("state.pqw");
    let st = Command::new(bin_path())
        .args([
            "train",
            "--corpus",
            dir.to_str().unwrap(),
            "--dim",
            "512",
            "--epsilon",
            "0.05",
            "--block",
            "512",
            "--steps",
            "1",
            "--shots",
            "300",
            "--out",
            out.to_str().unwrap(),
            "--gyro",
        ])
        .output()
        .unwrap();
    assert!(
        st.status.success(),
        "train stdout: {}",
        String::from_utf8_lossy(&st.stdout)
    );
    let raw = fs::read(&out).unwrap();
    assert_eq!(&raw[..8], b"POLER_Q3", "обучение обязано дать v3");
    out
}

/// Обучить v2-контейнер без гироскопа.
fn train_v2(dir: &std::path::Path) -> PathBuf {
    let out = dir.join("state_v2.pqw");
    let st = Command::new(bin_path())
        .args([
            "train",
            "--corpus",
            dir.to_str().unwrap(),
            "--dim",
            "512",
            "--epsilon",
            "0.05",
            "--block",
            "512",
            "--steps",
            "1",
            "--shots",
            "300",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(st.status.success());
    out
}

#[test]
fn precess_rejects_v2_container() {
    // Нет топологической секции — нет уравнения фиксации: честный отказ.
    let dir = tmp_dir("v2_reject");
    write_corpus(&dir);
    let v2 = train_v2(&dir);
    let st = Command::new(bin_path())
        .args(["precess", v2.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!st.status.success(), "v2 без гироскопа обязан отказать");
    let err = String::from_utf8_lossy(&st.stderr).to_string();
    assert!(err.contains("--gyro"), "{err}");
}

#[test]
fn precess_pure_orbit_report() {
    // Чистая прецессия: отчёт с траекторией и невязкой p* = a ⊗_ε p*.
    let dir = tmp_dir("pure");
    write_corpus(&dir);
    let v3 = train_v3(&dir);
    let st = Command::new(bin_path())
        .args([
            "precess",
            v3.to_str().unwrap(),
            "--eta",
            "0.05",
            "--ticks",
            "2000",
        ])
        .output()
        .unwrap();
    assert!(
        st.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&st.stdout),
        String::from_utf8_lossy(&st.stderr)
    );
    let text = String::from_utf8_lossy(&st.stdout).to_string();
    assert!(text.contains("петля архетипа"), "{text}");
    assert!(text.contains("русла J"), "{text}");
    assert!(text.contains("РЕЗУЛЬТАТ"), "{text}");
    assert!(text.contains("p* = a ⊗_ε p*"), "{text}");
    assert!(text.contains("r (Курамото)"), "{text}");
    // Режим по умолчанию — чистая прецессия, подсказка об оседании.
    assert!(text.contains("--purify-every"), "{text}");
}

#[test]
fn precess_settles_and_writes_archetype() {
    // Оседание: прецессия + McWeeny каждые 4 тика. Итоговое состояние
    // пишется валидным v3-контейнером с сохранёнными руслами J.
    let dir = tmp_dir("settle");
    write_corpus(&dir);
    let v3 = train_v3(&dir);
    let arch = dir.join("archetype.pqw");
    let st = Command::new(bin_path())
        .args([
            "precess",
            v3.to_str().unwrap(),
            "--eta",
            "0.05",
            "--ticks",
            "200000",
            "--delta",
            "1e-12",
            "--purify-every",
            "4",
            "--out",
            arch.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        st.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&st.stdout)
    );
    let text = String::from_utf8_lossy(&st.stdout).to_string();
    assert!(text.contains("записано"), "{text}");

    // Контейнер-архетип: валидный v3, гироскоп перенесён.
    let raw = fs::read(&arch).unwrap();
    assert_eq!(&raw[..8], b"POLER_Q3");
    let reader = pqw::PqwReader::from_bytes(&raw).unwrap();
    assert!(reader.header().is_gyro());
    let src = fs::read(&v3).unwrap();
    let src_reader = pqw::PqwReader::from_bytes(&src).unwrap();
    assert_eq!(reader.d_pol(), src_reader.d_pol());
    let g = reader.gyro().expect("русла J обязаны сохраниться");
    let g0 = src_reader.gyro().expect("исходник v3");
    assert_eq!(g.pairs().len(), g0.pairs().len());
    assert_eq!(g.ticks(), g0.ticks());
    assert!(g.pairs().len() > 0);
    // Если фиксация достигнута — моменты погашены и фазы на тритах.
    if text.contains("ДОСТИГНУТ") {
        assert!(text.contains("точные"), "{text}");
    }
}

#[test]
fn precess_json_report_parseable() {
    // JSON: все ключи отчёта присутствуют.
    let dir = tmp_dir("json");
    write_corpus(&dir);
    let v3 = train_v3(&dir);
    let st = Command::new(bin_path())
        .args([
            "precess",
            v3.to_str().unwrap(),
            "--eta",
            "0.05",
            "--ticks",
            "500",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(st.status.success());
    let text = String::from_utf8_lossy(&st.stdout).to_string();
    for key in [
        "\"fixated\"",
        "\"ticks\"",
        "\"torque_initial\"",
        "\"torque_final\"",
        "\"r_initial\"",
        "\"r_final\"",
        "\"contraction\"",
        "\"omega\"",
        "\"trits_exact\"",
        "\"trace\"",
        "\"purify_every\"",
    ] {
        assert!(text.contains(key), "нет ключа {key}: {text}");
    }
}

#[test]
fn inspect_modes_show_archetype_residuals() {
    // inspect --modes: невязки Ritz / ортонормальности (идемпотентность
    // A² = A) и захват компонент у каждой моды.
    let dir = tmp_dir("modes");
    write_corpus(&dir);
    let v3 = train_v3(&dir);
    let st = Command::new(bin_path())
        .args(["inspect", v3.to_str().unwrap(), "--modes", "4", "--no-hex"])
        .output()
        .unwrap();
    assert!(st.status.success());
    let text = String::from_utf8_lossy(&st.stdout).to_string();
    assert!(text.contains("Ritz"), "{text}");
    assert!(text.contains("захват"), "{text}");
    assert!(text.contains("A² = A"), "{text}");
}
