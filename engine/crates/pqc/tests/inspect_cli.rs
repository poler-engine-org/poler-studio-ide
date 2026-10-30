#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Интеграционные тесты CLI `pqc inspect`: детект форматов (v1/v2/raw/opaque),
//! крипто-разведка (structured/encrypted-like), JSON-отчёт собственным парсером,
//! DOT-вывод и коды ошибок.

use std::process::Command;

use pqc::json::Json;
use pqw::PqwWriter;

fn pqc_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pqc"))
}

fn tmp(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("pqc-inspect-{name}-{}.tmp", std::process::id()));
    p
}

fn v1_bytes() -> Vec<u8> {
    PqwWriter::new(8)
        .unwrap()
        .add_phase(3, -1.0)
        .unwrap()
        .add_phase(6, 0.5)
        .unwrap()
        .to_bytes()
        .unwrap()
}

fn v2_bytes() -> Vec<u8> {
    PqwWriter::new(9)
        .unwrap()
        .add_phase(2, 1.0)
        .unwrap()
        .add_phase(7, -1.0)
        .unwrap()
        .to_bytes_packed()
        .unwrap()
}

fn raw_bytes() -> Vec<u8> {
    // Дуга −1 по индексу 0, +1 по индексу 4: кандидат raw Packed4.
    vec![0b0000_0010, 0b0000_0001]
}

#[test]
fn inspect_v1_container_full_report() {
    let path = tmp("v1");
    std::fs::write(&path, v1_bytes()).unwrap();
    let out = pqc_bin()
        .arg("inspect")
        .arg(&path)
        .arg("--no-hex")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("pqw-v1 (POLER_QW, curved)"));
    assert!(text.contains("BYTE MAP"));
    assert!(text.contains("header_checksum"));
    assert!(text.contains("INTEGRITY"));
    assert!(text.contains("payload digest  : OK"));
    // Две дуги: одна с кривизной (p̂ = −1.0), одна σ = 32 (p̂ = 0.5).
    assert!(text.contains("nnz        : 2"));
    std::fs::remove_file(&path).ok();
}

#[test]
fn inspect_v2_container_and_corruption() {
    let path = tmp("v2");
    std::fs::write(&path, v2_bytes()).unwrap();
    let out = pqc_bin()
        .arg("inspect")
        .arg(&path)
        .arg("--no-hex")
        .arg("--decode")
        .arg("all")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("pqw-v2 (POLER_Q2, Packed4)"));
    assert!(text.contains("nnz        : 2"));
    assert!(text.contains("INTEGRITY"));
    assert!(text.contains("payload digest  : OK"));

    // Порча payload с сохранением nnz (трит +1 → −1): ридер разбирается
    // успешно, но SHA-256 digest бьётся.
    let mut corrupt = v2_bytes();
    corrupt[0x80] ^= 0b0011_0000; // трит 2: 0b01 (+1) → 0b10 (−1)
    std::fs::write(&path, corrupt).unwrap();
    let out = pqc_bin()
        .arg("inspect")
        .arg(&path)
        .arg("--no-hex")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("payload digest  : FAIL"));
    std::fs::remove_file(&path).ok();
}

#[test]
fn inspect_raw_packed4_candidate() {
    let path = tmp("raw");
    std::fs::write(&path, raw_bytes()).unwrap();
    let out = pqc_bin()
        .arg("inspect")
        .arg(&path)
        .arg("--no-hex")
        .arg("--graph")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("raw-packed4 (candidate)"));
    assert!(text.contains("d_pol      : 8"));
    assert!(text.contains("nnz        : 2 (1 × −1, 1 × +1)"));
    assert!(text.contains("GRAPH"));
    assert!(text.contains("nodes 2, edges 1"));
    std::fs::remove_file(&path).ok();
}

#[test]
fn inspect_json_parses_with_zero_dep_parser() {
    let path = tmp("json");
    std::fs::write(&path, raw_bytes()).unwrap();
    let out = pqc_bin()
        .arg("inspect")
        .arg(&path)
        .arg("--json")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let json = Json::parse(&text).expect("JSON отчёт должен парситься");
    assert_eq!(
        json.get("kind").and_then(|k| k.as_str()),
        Some("raw-packed4 (candidate)")
    );
    let d_pol = json.get("d_pol").and_then(|d| d.as_f64()).expect("d_pol");
    assert!((d_pol - 8.0).abs() < 1e-9);
    let arcs = json.get("arcs").and_then(|a| a.as_arr()).expect("arcs");
    assert_eq!(arcs.len(), 2);
    let crypto = json.get("crypto").expect("crypto");
    assert_eq!(
        crypto.get("verdict").and_then(|v| v.as_str()),
        Some("structured")
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn inspect_text_is_structured_not_packed4() {
    let path = tmp("text");
    std::fs::write(&path, b"hello world, plain text!").unwrap();
    let out = pqc_bin()
        .arg("inspect")
        .arg(&path)
        .arg("--no-hex")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    // Печатаемый ASCII не детектируется как Packed4-кандидат.
    assert!(text.contains("kind       : opaque"));
    assert!(text.contains("structured"));
    std::fs::remove_file(&path).ok();
}

#[test]
fn inspect_encrypted_like_verdict() {
    // Детерминированный xorshift-поток — статистически шифроподобный.
    let mut x: u64 = 0x9E3779B97F4A7C15;
    let mut data = vec![0u8; 4096];
    for b in data.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = (x >> 33) as u8;
    }
    let path = tmp("enc");
    std::fs::write(&path, &data).unwrap();
    let out = pqc_bin()
        .arg("inspect")
        .arg(&path)
        .arg("--no-hex")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("encrypted-like"));
    std::fs::remove_file(&path).ok();
}

#[test]
fn inspect_dot_written_and_matrix_small() {
    let path = tmp("dot-src");
    std::fs::write(&path, raw_bytes()).unwrap();
    let dot_path = tmp("lens");
    let out = pqc_bin()
        .arg("inspect")
        .arg(&path)
        .arg("--no-hex")
        .arg("--decode")
        .arg("all")
        .arg("--dot")
        .arg(&dot_path)
        .arg("--matrix")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(&format!("DOT       : written to {}", dot_path.display())));
    let dot = std::fs::read_to_string(&dot_path).unwrap();
    assert!(dot.contains("digraph LENS"));
    assert!(dot.contains("N0 -> N4"));
    // d_pol = 8 ≤ 64 — матрица печатается.
    assert!(text.contains("MATRIX"));
    assert!(text.contains('X'));
    std::fs::remove_file(&path).ok();
    std::fs::remove_file(&dot_path).ok();
}

#[test]
fn inspect_missing_file_fails_gracefully() {
    let out = pqc_bin()
        .arg("inspect")
        .arg("/nonexistent/poler-state.pqw")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("pqc inspect"));
}

#[test]
fn inspect_usage_error_without_file() {
    let out = pqc_bin().arg("inspect").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

// ── RQ10: контейнер v3 (POLER_Q3) — гироскоп J = A − Aᵀ и моды Im(P) ──

fn v3_bytes() -> Vec<u8> {
    let mut w = PqwWriter::new(64).unwrap();
    w = w.hyperparams(0.25, 0.5, 1.0, 0.05);
    w.add_phase(1, 0.9).unwrap();
    w.add_phase(4, -0.8).unwrap();
    w.add_phase(9, -0.7).unwrap();
    let gyro = pqw::gyro::GyroData::new(
        128,
        5_000,
        vec![(1, 4, 3.5), (4, 9, -2.0), (9, 12, 1.0)],
        64,
    )
    .unwrap();
    w.to_bytes_v3(&gyro).unwrap()
}

#[test]
fn inspect_detects_v3_and_prints_gyro() {
    let path = tmp("v3");
    std::fs::write(&path, v3_bytes()).unwrap();
    let out = pqc_bin().arg("inspect").arg(&path).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(text.contains("pqw-v3"), "{text}");
    assert!(text.contains("POLER_Q3"), "{text}");
    assert!(text.contains("GYRO"), "{text}");
    assert!(text.contains("J = A − Aᵀ"), "{text}");
    // Метрики секции.
    assert!(text.contains("window"), "{text}");
    assert!(text.contains("ticks"), "{text}");
    assert!(text.contains("5000"), "{text}");
    // Моды Im(P) с фазовыми углами.
    assert!(text.contains("моды Im(P)"), "{text}");
    assert!(text.contains("λ1"), "{text}");
    std::fs::remove_file(&path).ok();
}

#[test]
fn inspect_v3_json_gyro_modes() {
    let path = tmp("v3_json");
    std::fs::write(&path, v3_bytes()).unwrap();
    let out = pqc_bin()
        .arg("inspect")
        .arg(&path)
        .arg("--json")
        .arg("--modes")
        .arg("2")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let doc = Json::parse(&text).unwrap();
    assert!(doc.get("gyro").is_some());
    let g = doc.get("gyro").unwrap();
    assert_eq!(g.get("window").and_then(Json::as_f64), Some(128.0));
    assert_eq!(g.get("ticks").and_then(Json::as_f64), Some(5000.0));
    assert_eq!(g.get("pairs").and_then(Json::as_f64), Some(3.0));
    let modes = g.get("modes").unwrap().as_arr().unwrap();
    assert!(!modes.is_empty());
    let m1 = modes[0].get("lambda").and_then(Json::as_f64).unwrap();
    assert!(m1 > 0.0);
    // λ_max ≥ max|J_ij| = 3.5 (кососимметричная J нормальна: сингулярные
    // числа = |собственные значения|; тест-вектор (e_i+e_j)/√2 даёт ‖Jx‖=|w|).
    assert!(m1 >= 3.5, "λ1 = {m1} < max веса пары 3.5");
    // Фазовый портрет Im(P): у дуги 1 есть p̂ и θ.
    let u = modes[0].get("u").unwrap().as_arr().unwrap();
    let first = &u[0];
    assert!(first.get("p").is_some(), "фазовый портрет Im(P) обязателен");
    assert!(first.get("theta").is_some());
    std::fs::remove_file(&path).ok();
}

#[test]
fn inspect_v3_modes_option_validation() {
    let path = tmp("v3_bad_modes");
    std::fs::write(&path, v3_bytes()).unwrap();
    for bad in ["0", "abc", "65"] {
        let out = pqc_bin()
            .arg("inspect")
            .arg(&path)
            .arg("--modes")
            .arg(bad)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "--modes {bad} должно отклоняться");
    }
    std::fs::remove_file(&path).ok();
}
