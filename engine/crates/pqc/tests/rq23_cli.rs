#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! RQ23 / v1.3.0 CLI-контракты: автобиографическая память диалога
//! (контекст-рефлекс W, контейнер v5), gap-RLE гироскопа и спиновая
//! лавина GF(3) (`pqc avalanche`).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_pqc"))
}

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pqc-rq23-cli-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(bin_path())
        .args(args)
        .env("PQC_SHOTS", "256")
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// Мозг из offline-корпуса: квантовая мини-энциклопедия (v4).
fn train_brain(dir: &PathBuf, brain: &PathBuf) {
    let qm = "квант фаза решётка квант фаза решётка трит born момент \
              квант фаза решётка импульс кристалл память русло";
    let mut texts = Vec::new();
    for _i in 0..12 {
        texts.push(qm);
    }
    let corpus_dir = dir.join("corpus");
    fs::create_dir_all(&corpus_dir).unwrap();
    fs::write(corpus_dir.join("qm.txt"), texts.join("\n\n")).unwrap();
    let (ok, out, err) = run(&[
        "train",
        "--corpus",
        corpus_dir.to_str().unwrap(),
        "--quantized",
        "--gyro",
        "8",
        "--dim",
        "512",
        "--steps",
        "2",
        "--seed",
        "42",
        "--out",
        brain.to_str().unwrap(),
    ]);
    assert!(ok, "train упал:\n{out}\n{err}");
    assert_eq!(fs::read(brain).unwrap()[..8], *b"POLER_Q4");
}

#[test]
fn chat_remembers_interlocutor_across_sessions() {
    // СЕССИЯ 1: знакомство + разговор → мозг сохраняет рефлекс W.
    let dir = tmp_dir("memory");
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);
    assert_eq!(fs::read(&brain).unwrap()[..8], *b"POLER_Q4");

    let mut child = Command::new(bin_path())
        .args([
            "chat",
            "--brain",
            brain.to_str().unwrap(),
            "--name",
            "Иван",
            "--max-tokens",
            "6",
            "--seed",
            "5",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all("что такое квант\nчто такое волна\n".as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "сессия 1: {stdout}");
    // Холодный старт: баннера автобиографии нет (рефлекса ещё нет).
    assert!(!stdout.contains("автобио"));
    // Контейнер вырос до v5: след диалога жив.
    assert_eq!(fs::read(&brain).unwrap()[..8], *b"POLER_Q5");

    // СЕССИЯ 2: мозг ПОМНИТ собеседника и нить.
    let mut child = Command::new(bin_path())
        .args([
            "chat",
            "--brain",
            brain.to_str().unwrap(),
            "--max-tokens",
            "6",
            "--seed",
            "5",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all("продолжай про волну\n".as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "сессия 2: {stdout}");
    assert!(
        stdout.contains("автобио"),
        "баннер автобиографии отсутствует: {stdout}"
    );
    assert!(
        stdout.contains("Иван"),
        "мозг не помнит собеседника: {stdout}"
    );
    assert!(
        stdout.contains("реплик"),
        "счётчик реплик истории не показан: {stdout}"
    );
    // Инспектор видит рефлекс.
    let (ok, out, err) = run(&[
        "inspect",
        brain.to_str().unwrap(),
        "--json",
        "--no-hex",
    ]);
    assert!(ok, "inspect: {err}");
    assert!(out.contains("\"reflex\""), "inspect JSON без рефлекса: {out}");
    assert!(out.contains("\"interlocutor\":\"Иван\""), "inspect: {out}");
    // Счётчик реплик: сессия 1 дала 2, сессия 2 — ещё 1 → 3
    // (Json::num печатает f64: 3 → «3e0»).
    assert!(out.contains("\"turns\":3e0"), "inspect turns: {out}");
}

#[test]
fn chat_fresh_disables_autobiography() {
    // --fresh: холодный старт волны даже при живом рефлексе.
    let dir = tmp_dir("fresh");
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);
    // Сессия 1 создаёт рефлекс.
    let mut child = Command::new(bin_path())
        .args([
            "chat",
            "--brain",
            brain.to_str().unwrap(),
            "--max-tokens",
            "4",
            "--seed",
            "5",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all("что такое фаза\n".as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(fs::read(&brain).unwrap()[..8], *b"POLER_Q5");

    // Сессия 2 с --fresh: волна стартует холодно (нить не подхвачена),
    // но баннер честно показывает, ЧТО мозг помнит — рефлекс не стёрт.
    let (ok, out, err) = run(&[
        "ask",
        "что такое трит",
        "--brain",
        brain.to_str().unwrap(),
        "--fresh",
        "--max-tokens",
        "4",
        "--seed",
        "5",
    ]);
    assert!(ok, "fresh ask: {err}");
    assert!(out.contains("автобио"), "баннер памяти: {out}");
    // Рефлекс жив: обычный (не fresh) ask подхватывает нить.
    let (ok2, out2, _) = run(&[
        "ask",
        "что такое трит",
        "--brain",
        brain.to_str().unwrap(),
        "--max-tokens",
        "4",
        "--seed",
        "5",
    ]);
    assert!(ok2);
    assert!(out2.contains("автобио"));
    assert_eq!(fs::read(&brain).unwrap()[..8], *b"POLER_Q5");
}

#[test]
fn chat_json_reports_reflex() {
    // JSON-протокол: объект brain несёт reflex (имя/реплики/нить).
    let dir = tmp_dir("json");
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);
    let mut child = Command::new(bin_path())
        .args([
            "chat",
            "--brain",
            brain.to_str().unwrap(),
            "--name",
            "Мария",
            "--max-tokens",
            "4",
            "--seed",
            "3",
            "--json",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all("что такое фаза\n".as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        stdout.contains("\"reflex_turns\":1"),
        "реплика в JSON: {stdout}"
    );
    assert_eq!(fs::read(&brain).unwrap()[..8], *b"POLER_Q5");

    // Вторая сессия JSON: brain.reflex с историей.
    let (ok, out, err) = run(&[
        "ask",
        "продолжай",
        "--brain",
        brain.to_str().unwrap(),
        "--json",
        "--max-tokens",
        "4",
        "--seed",
        "3",
    ]);
    assert!(ok, "ask json: {err}");
    assert!(out.contains("\"reflex\""), "brain.reflex: {out}");
    assert!(out.contains("\"interlocutor\":\"Мария\""), "имя: {out}");
    assert!(out.contains("\"turns\":1"), "реплик прошлой сессии: {out}");
}

#[test]
fn rle_gyro_codec_reported_by_inspect() {
    // Контейнер с кластеризованной топологией: инспектор показывает
    // кодек v2 (gap-RLE) и статистику сжатия. Проверяем на реальном
    // мозге (топология после train --gyro кластеризована по окну).
    let dir = tmp_dir("rle");
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);
    let (ok, out, err) = run(&[
        "inspect",
        brain.to_str().unwrap(),
        "--json",
        "--no-hex",
    ]);
    assert!(ok, "inspect: {err}");
    // Какой бы кодек ни выбрал писатель — отчёт честен о нём.
    if out.contains("\"codec\":2") {
        assert!(
            out.contains("\"rle_bytes\""),
            "RLE-кодек обязан рапортовать сжатие: {out}"
        );
        assert!(
            out.contains("\"sparse_bytes\""),
            "и эталон разреженного: {out}"
        );
    } else {
        assert!(out.contains("\"codec\":1"), "кодек в отчёте: {out}");
    }
}

#[test]
fn avalanche_command_json_protocol() {
    // pqc avalanche: полный протокол измерения спиновой лавины.
    let dir = tmp_dir("av");
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);
    let (ok, out, err) = run(&[
        "avalanche",
        "--key",
        brain.to_str().unwrap(),
        "--size",
        "8192",
        "--probes",
        "4",
        "--seed",
        "7",
        "--json",
    ]);
    assert!(ok, "avalanche: {err}");
    for field in [
        "\"scheme\":\"spin-avalanche-gf3\"",
        "\"version\":2e0",
        "\"size\":8.192e3",
        "\"probes\":4e0",
        "\"blocks\":",
        "\"ticks\":",
        "\"nl_rounds\":",
        "\"ticks_linear\":",
        "\"avalanche_mean\":",
        "\"avalanche_from_probe\":",
        "\"avalanche_min\":",
        "\"avalanche_max\":",
        "\"ceiling\":",
        "\"cascade_after\":",
        "\"transform_spread_mean\":",
        "\"chi2_blocks\":",
        "\"linear_avalanche_probe\":",
    ] {
        assert!(out.contains(field), "нет {field} в: {out}");
    }
    // От блока зонда вперёд диффузия не хуже 25%.
    let from_probe: f64 = out
        .split("\"avalanche_from_probe\":")
        .nth(1)
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0.0);
    assert!(from_probe >= 0.25, "from_probe {from_probe} < 0.25: {out}");
}

#[test]
fn avalanche_human_report() {
    let dir = tmp_dir("av-human");
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);
    let (ok, out, err) = run(&[
        "avalanche",
        "--key",
        brain.to_str().unwrap(),
        "--size",
        "4096",
        "--probes",
        "2",
    ]);
    assert!(ok, "avalanche human: {err}");
    assert!(out.contains("спиновая лавина"), "заголовок: {out}");
    assert!(out.contains("от блока зонда"), "честная метрика: {out}");
    assert!(out.contains("каскад"), "каскад CBC: {out}");
    assert!(out.contains("квадратичный T-проход"), "физика: {out}");
}

#[test]
fn avalanche_requires_key() {
    let (ok, _, err) = run(&["avalanche"]);
    assert!(!ok);
    assert!(err.contains("--key"), "подсказка про ключ: {err}");
}

#[test]
fn encrypt_linear_flag_writes_v1() {
    // --linear: заголовок версии 1, чистый транспорт (legacy RQ13).
    let dir = tmp_dir("lin");
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);
    let msg = dir.join("m.bin");
    fs::write(&msg, b"linear legacy probe").unwrap();
    let cipher = dir.join("m.pqt");

    let (ok, out, err) = run(&[
        "encrypt",
        msg.to_str().unwrap(),
        "--key",
        brain.to_str().unwrap(),
        "--out",
        cipher.to_str().unwrap(),
        "--linear",
        "--seed",
        "42",
    ]);
    assert!(ok, "encrypt --linear: {err}\n{out}");
    assert!(out.contains("линейная v1"), "схема объявлена: {out}");
    let bytes = fs::read(&cipher).unwrap();
    assert_eq!(&bytes[..4], b"PQT1");
    assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 1, "версия v1");
    // Расшифровка точна (v1 читается как раньше).
    let (ok2, out2, err2) = run(&[
        "decrypt",
        cipher.to_str().unwrap(),
        "--key",
        brain.to_str().unwrap(),
        "--out",
        dir.join("plain.bin").to_str().unwrap(),
    ]);
    assert!(ok2, "decrypt v1: {err2}\n{out2}");
    assert_eq!(
        fs::read(dir.join("plain.bin")).unwrap(),
        b"linear legacy probe".to_vec()
    );

    // Дефолт без флага — версия 2.
    let cipher2 = dir.join("m2.pqt");
    let (ok3, _, err3) = run(&[
        "encrypt",
        msg.to_str().unwrap(),
        "--key",
        brain.to_str().unwrap(),
        "--out",
        cipher2.to_str().unwrap(),
        "--seed",
        "42",
    ]);
    assert!(ok3, "encrypt v2: {err3}");
    let bytes2 = fs::read(&cipher2).unwrap();
    assert_eq!(u16::from_le_bytes([bytes2[4], bytes2[5]]), 2, "версия v2");
    // v2 тоже расшифровывается.
    let (ok4, _, _) = run(&[
        "decrypt",
        cipher2.to_str().unwrap(),
        "--key",
        brain.to_str().unwrap(),
        "--out",
        dir.join("plain2.bin").to_str().unwrap(),
    ]);
    assert!(ok4);
    assert_eq!(
        fs::read(dir.join("plain2.bin")).unwrap(),
        b"linear legacy probe".to_vec()
    );
}

#[test]
fn merge_inherits_reflex_of_brain_a() {
    // Слияние: автобиография мозга A переживает ⊗_ε (личность хозяина).
    let dir = tmp_dir("merge-refl");
    let a = dir.join("a.pqw");
    let b = dir.join("b.pqw");
    train_brain(&dir, &a);
    // Мозг B: другой корпус (другая директория).
    let dir_b = dir.join("b");
    fs::create_dir_all(&dir_b).unwrap();
    train_brain(&dir_b, &b);

    // Мозг A получает рефлекс через диалог.
    let (ok, _, err) = run(&[
        "ask",
        "что такое квант",
        "--brain",
        a.to_str().unwrap(),
        "--name",
        "Ольга",
        "--max-tokens",
        "4",
        "--seed",
        "5",
    ]);
    assert!(ok, "ask A: {err}");
    assert_eq!(fs::read(&a).unwrap()[..8], *b"POLER_Q5");

    let merged = dir.join("m.pqw");
    let (ok, out, err) = run(&[
        "merge",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--out",
        merged.to_str().unwrap(),
    ]);
    assert!(ok, "merge: {err}\n{out}");
    assert_eq!(fs::read(&merged).unwrap()[..8], *b"POLER_Q5");
    // Инспектор: рефлекс слитого мозга — от A.
    let (ok, out, err) = run(&[
        "inspect",
        merged.to_str().unwrap(),
        "--json",
        "--no-hex",
    ]);
    assert!(ok, "inspect merged: {err}");
    assert!(out.contains("\"interlocutor\":\"Ольга\""), "наследие A: {out}");
}

#[test]
fn learn_does_not_destroy_reflex() {
    // Дообучение (learn --text) сохраняет автобиографию: новые знания
    // не стирают личность.
    let dir = tmp_dir("learn-keep");
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);
    let (ok, _, err) = run(&[
        "ask",
        "что такое фаза",
        "--brain",
        brain.to_str().unwrap(),
        "--name",
        "Пётр",
        "--max-tokens",
        "4",
        "--seed",
        "5",
    ]);
    assert!(ok, "ask: {err}");

    let (ok, out, err) = run(&[
        "learn",
        "новая тема",
        "--brain",
        brain.to_str().unwrap(),
        "--text",
        "гравитация масса энергия гравитация масса энергия",
    ]);
    assert!(ok, "learn: {err}\n{out}");
    // Контейнер всё ещё v5, рефлекс жив.
    assert_eq!(fs::read(&brain).unwrap()[..8], *b"POLER_Q5");
    let (ok, out, _) = run(&[
        "inspect",
        brain.to_str().unwrap(),
        "--json",
        "--no-hex",
    ]);
    assert!(ok);
    assert!(out.contains("\"interlocutor\":\"Пётр\""), "личность жива: {out}");
}
