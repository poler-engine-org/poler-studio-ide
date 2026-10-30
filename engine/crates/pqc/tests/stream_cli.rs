#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Интеграционные тесты CLI `pqc stream` (RQ6): сквозной прогон бинарника
//! на тексте/файле, JSON-отчёт, барьер NO_HITS и zero-dep HTTP-регламент
//! (https сознательно отвергается с внятным сообщением).

use std::process::{Command, Stdio};

fn pqc_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pqc"))
}

const TEXT: &str = "фазовый континуум фазовый триты борн анзац линза решётка";

#[test]
fn stream_text_reports_learning() {
    let out = pqc_bin()
        .args([
            "stream", "--text", TEXT, "--dim", "512", "--steps", "8", "--shots", "1024", "--seed",
            "42",
        ])
        .output()
        .expect("запуск pqc stream");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Zero-Storage Streaming Engine"));
    assert!(text.contains("Packed4"));
    assert!(text.contains("Active Inference"));
    assert!(text.contains("qcm"));
    // Побитовая воспроизводимость CLI при фиксированном сиде.
    let out2 = pqc_bin()
        .args([
            "stream", "--text", TEXT, "--dim", "512", "--steps", "8", "--shots", "1024", "--seed",
            "42",
        ])
        .output()
        .unwrap();
    // Отчёты совпадают построчно, кроме строки elapsed.
    let text2 = String::from_utf8_lossy(&out2.stdout).into_owned();
    let l1: Vec<&str> = text.lines().filter(|l| !l.starts_with("elapsed")).collect();
    let l2: Vec<&str> = text2
        .lines()
        .filter(|l| !l.starts_with("elapsed"))
        .collect();
    assert_eq!(l1, l2);
}

#[test]
fn stream_json_parses_with_zero_dep_parser() {
    let out = pqc_bin()
        .args([
            "stream", "--text", TEXT, "--dim", "256", "--steps", "3", "--shots", "512", "--json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json_text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    // Парсинг собственным zero-dep JSON-парсером крейта.
    let parsed = pqc::json::Json::parse(&json_text).expect("JSON отчёт не парсится");
    assert_eq!(
        parsed.get("source_kind").and_then(|v| v.as_str()),
        Some("text")
    );
    assert_eq!(parsed.get("d_pol").and_then(|v| v.as_f64()), Some(256.0));
    assert_eq!(parsed.get("no_hits").and_then(|v| v.as_bool()), Some(false));
    assert!(
        parsed
            .get("container_bytes")
            .and_then(|v| v.as_f64())
            .unwrap()
            > 0.0
    );
    assert!(parsed.get("fock").is_some());
    assert!(parsed.get("step").is_some());
    assert!(parsed
        .get("elapsed_ms")
        .and_then(|v| v.as_f64())
        .unwrap()
        .is_finite());
}

#[test]
fn stream_no_hits_barrier() {
    let out = pqc_bin()
        .args(["stream", "--text", "… — !!!", "--dim", "128", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let parsed = pqc::json::Json::parse(String::from_utf8_lossy(&out.stdout).trim()).unwrap();
    assert_eq!(parsed.get("no_hits").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(parsed.get("nnz").and_then(|v| v.as_f64()), Some(0.0));
    match parsed.get("step") {
        None => {}
        Some(pqc::json::Json::Null) => {}
        _ => panic!("шаг при NO_HITS не выполняется"),
    }
}

#[test]
fn stream_https_is_rejected_with_hint() {
    let out = pqc_bin()
        .args(["stream", "--url", "https://example.com/page"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("https") && err.contains("--file"),
        "сообщение должно подсказывать --file: {err}"
    );
}

#[test]
fn stream_requires_exactly_one_source() {
    let out = pqc_bin()
        .args(["stream", "--text", "a", "--dim", "64"])
        .output()
        .unwrap();
    // Один источник — ок.
    assert!(out.status.success());

    let out = pqc_bin()
        .args(["stream", "--text", "a", "--file", "/etc/hostname"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn stream_file_source_and_container_dump() {
    let dir = std::env::temp_dir().join(format!("pqc-stream-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let html = dir.join("page.html");
    let container = dir.join("chunk.pqw");
    std::fs::write(&html, format!("<html><body><p>{TEXT}</p></body></html>")).unwrap();

    let out = pqc_bin()
        .args([
            "stream",
            "--file",
            html.to_str().unwrap(),
            "--dim",
            "512",
            "--steps",
            "4",
            "--shots",
            "512",
            "--out",
            container.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text_ok(&out.stdout));

    // Дамп контейнера — валидный Packed4 v2: читается zero-copy читателем.
    let bytes = std::fs::read(&container).unwrap();
    let reader = pqw::PqwReader::from_bytes(&bytes).expect("дамп не читается");
    assert_eq!(reader.d_pol(), 512);
    assert!(reader.nnz() > 0);
    assert!(reader.header().is_packed());

    let _ = std::fs::remove_dir_all(&dir);
}

fn text_ok(stdout: &[u8]) -> bool {
    let s = String::from_utf8_lossy(stdout);
    s.contains("container") && s.contains("fock")
}

/// Стандартный ввод: echo-текст через pipe.
#[test]
fn stream_stdin_source() {
    use std::io::Write;
    let mut child = pqc_bin()
        .args(["stream", "--stdin", "--dim", "256", "--shots", "256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(TEXT.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text_ok(&out.stdout));
}

/// URL на замкнутый порт отклоняется без паники и без зависания.
#[test]
fn stream_http_connect_failure_is_graceful() {
    // Порт 1 на localhost: соединение отвергается ОС мгновенно.
    let out = pqc_bin()
        .args(["stream", "--url", "http://127.0.0.1:1/"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("connect") || err.contains("refused"),
        "err: {err}"
    );
}
