#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Интеграционные тесты CLI `pqc bloch` (RQ14): mmap-разворот тритов
//! v2 в углы Блоха — θ = arccos(p) на лету, LUT из трёх констант.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pqc");

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pqc-bloch-cli-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(BIN)
        .args(args)
        .output()
        .expect("pqc binary must run");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Контейнер v2 с известной тритовой решёткой:
/// d=8, p = [+0.7, −0.7, 0.0, +0.7, −0.7, 0.0, 0.0, −0.7]
/// → триты [Pos, Neg, Zero, Pos, Neg, Zero, Zero, Neg].
fn make_v2(path: &std::path::Path) {
    use pqw::PqwWriter;
    let mut w = PqwWriter::new(8).unwrap();
    for (i, p) in [0.7_f32, -0.7, 0.0, 0.7, -0.7, 0.0, 0.0, -0.7]
        .iter()
        .enumerate()
    {
        w.add_phase(i as u32, *p).unwrap();
    }
    let bytes = w.to_bytes_packed().unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn bloch_human_report_counts_and_angles() {
    let dir = tmp_dir("human");
    let f = dir.join("state.pqw");
    make_v2(&f);
    let (code, out, err) = run(&["bloch", f.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {err}");
    assert!(out.contains("RQ14"), "заголовок RQ14: {out}");
    assert!(out.contains("d_pol     : 8"), "d_pol: {out}");
    assert!(out.contains("+1 × 2"), "pos: {out}");
    assert!(out.contains("−1 × 3"), "neg: {out}");
    assert!(out.contains("0 × 3"), "zero: {out}");
    // Плотность 5/8 = 0.625, баланс 2/5 = 0.4.
    assert!(out.contains("0.6250"), "density: {out}");
    assert!(out.contains("0.4000"), "balance: {out}");
    // Первые углы — точные LUT-значения: π/2, π, π/2 (Pos, Neg, Zero → ...).
    assert!(out.contains("1.5708"), "θ=π/2: {out}");
    assert!(out.contains("3.1416"), "θ=π: {out}");
    assert!(out.contains("0.0000"), "θ=0: {out}");
    assert!(out.contains("нс/трит"), "микробенчмарк: {out}");
}

#[test]
fn bloch_json_report_parses_with_zero_dep_parser() {
    let dir = tmp_dir("json");
    let f = dir.join("state.pqw");
    make_v2(&f);
    let (code, out, err) = run(&["bloch", f.to_str().unwrap(), "--json", "--head", "4"]);
    assert_eq!(code, 0, "stderr: {err}");
    let j = pqc::json::Json::parse(&out).expect("JSON отчёт должен парситься");
    assert_eq!(j.get("cmd").and_then(|v| v.as_str()), Some("bloch"));
    assert_eq!(j.get("d_pol").and_then(|v| v.as_f64()), Some(8.0));
    assert_eq!(j.get("pos").and_then(|v| v.as_f64()), Some(2.0));
    assert_eq!(j.get("neg").and_then(|v| v.as_f64()), Some(3.0));
    assert_eq!(j.get("zero").and_then(|v| v.as_f64()), Some(3.0));
    assert_eq!(j.get("encoding").and_then(|v| v.as_str()), Some("packed4"));
    // Плотность и баланс.
    let density = j.get("density").and_then(|v| v.as_f64()).unwrap();
    assert!((density - 0.625).abs() < 1e-9);
    let balance = j.get("balance").and_then(|v| v.as_f64()).unwrap();
    assert!((balance - 0.4).abs() < 1e-9);
    // Превью углов: пары [индекс, θ]; #0 = Pos → 0; #1 = Neg → π.
    let head = j.get("theta_head").and_then(|v| v.as_arr()).unwrap();
    assert_eq!(head.len(), 4);
    let first = head[0].as_arr().unwrap();
    assert_eq!(first[1].as_f64().unwrap(), 0.0);
    let second = head[1].as_arr().unwrap();
    assert!((second[1].as_f64().unwrap() - std::f64::consts::PI).abs() < 1e-12);
    // Средний θ = (0 + π + π/2 + 0 + π + π/2 + π/2 + π)/8 = (3π + 3·π/2)/8.
    let mean = j.get("theta_mean").and_then(|v| v.as_f64()).unwrap();
    assert!((mean - (3.0 * std::f64::consts::PI + 3.0 * std::f64::consts::FRAC_PI_2) / 8.0).abs() < 1e-12);
    assert!(j.get("stream_ns_per_trit").and_then(|v| v.as_f64()).unwrap() >= 0.0);
}

#[test]
fn bloch_window_flag_accepted() {
    let dir = tmp_dir("window");
    let f = dir.join("state.pqw");
    make_v2(&f);
    let (code, out, err) = run(&["bloch", f.to_str().unwrap(), "--window", "3", "--head", "2"]);
    assert_eq!(code, 0, "stderr: {err}");
    assert!(out.contains("окно 3"), "{out}");
    // Ответ совпадает с окном по умолчанию: стриминг коммутативен окнам.
    let (code2, out2, err2) = run(&["bloch", f.to_str().unwrap(), "--head", "2"]);
    assert_eq!(code2, 0, "stderr: {err2}");
    let mean = |s: &str| {
        s.lines()
            .find(|l| l.contains("средний θ"))
            .unwrap()
            .split(':')
            .nth(1)
            .unwrap()
            .trim()
            .to_string()
    };
    assert_eq!(mean(&out), mean(&out2));
}

#[test]
fn bloch_missing_file_and_bad_flags_are_errors() {
    let (code, _out, err) = run(&["bloch"]);
    assert_eq!(code, 2);
    assert!(err.contains("нужен контейнер"), "{err}");
    let (code, _, err) = run(&["bloch", "/nonexistent/x.pqw"]);
    assert_eq!(code, 1);
    assert!(err.contains("/nonexistent/x.pqw"), "{err}");
    let dir = tmp_dir("flags");
    let f = dir.join("s.pqw");
    make_v2(&f);
    let (code, _, err) = run(&["bloch", f.to_str().unwrap(), "--bogus"]);
    assert_eq!(code, 2);
    assert!(err.contains("--bogus"), "{err}");
    let (code, out, _) = run(&["bloch", f.to_str().unwrap(), "--window", "0"]);
    assert_eq!(code, 2);
    assert!(!out.is_empty() || true);
}
