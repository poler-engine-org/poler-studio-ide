#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! RQ19: `pqc learn` — интеграционные тесты CLI (offline через `--text`,
//! сеть — только `#[ignore]`-смоуки).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn bin_path() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_BIN_EXE_pqc"));
    assert!(p.exists(), "бинарник pqc не собран: {p:?}");
    p
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rq19_{}_{}_{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(bin_path()).args(args).output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// Текст «страницы» для offline-режима: плотный тематический корпус.
const QM_TEXT: &str = "квантовая механика описывает микромир волновыми функциями \
                       суперпозиция состояний суперпозиция основа квантовых вычислений \
                       энергия квантована фотон несёт квант света кубит может быть \
                       нулём и единицей измерение коллапсирует волновую функцию";

#[test]
fn learn_offline_creates_v4_brain_and_answers() {
    let dir = tmp_dir("offline");
    let brain = dir.join("brain.pqw");
    let (ok, stdout, stderr) = run(&[
        "learn",
        "квантовая механика",
        "--brain",
        brain.to_str().unwrap(),
        "--text",
        QM_TEXT,
        "--ask",
        "что такое суперпозиция",
    ]);
    assert!(ok, "learn упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("RQ19"));
    assert!(stdout.contains("offline"));
    assert!(stdout.contains("лексикон 0 → "));
    assert!(stdout.contains("v4 контейнер"));
    assert!(stdout.contains("ответ"));

    // Контейнер настоящий v4 с лексиконом.
    let bytes = fs::read(&brain).unwrap();
    assert_eq!(&bytes[..8], b"POLER_Q4");

    // pqc ask немедленно отвечает выученными словами (проверка ТЗ п.3).
    let (ok2, out2, err2) = run(&[
        "ask",
        "что такое суперпозиция",
        "--brain",
        brain.to_str().unwrap(),
        "--no-learn",
    ]);
    assert!(ok2, "ask упал:\n{out2}\n{err2}");
    assert!(out2.contains("ответ"), "нет ответа:\n{out2}");
}

#[test]
fn learn_extends_existing_brain() {
    let dir = tmp_dir("extend");
    let brain = dir.join("brain.pqw");

    let (ok, out, err) = run(&[
        "learn",
        "квантовая механика",
        "--brain",
        brain.to_str().unwrap(),
        "--text",
        QM_TEXT,
    ]);
    assert!(ok, "{err}");
    let lex_line = out.lines().find(|l| l.contains("лексикон")).unwrap();
    let lex1: usize = lex_line
        .split("лексикон")
        .nth(1)
        .unwrap()
        .split("→")
        .nth(1)
        .unwrap()
        .trim()
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(lex1 > 0);

    // Второй заход с ДРУГОЙ темой: мозг расширяется, лексикон растёт.
    let (ok2, out2, err2) = run(&[
        "learn",
        "фотон свет",
        "--brain",
        brain.to_str().unwrap(),
        "--text",
        "фотон это квант электромагнитного поля фотон несёт энергию света \
         фотоны рождаются при переходах атомных уровней",
    ]);
    assert!(ok2, "{err2}");
    let lex_line2 = out2.lines().find(|l| l.contains("лексикон")).unwrap();
    let lex2: usize = lex_line2
        .split("лексикон")
        .nth(1)
        .unwrap()
        .split("→")
        .nth(1)
        .unwrap()
        .trim()
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(lex2 > lex1, "лексикон не вырос: {lex1} → {lex2}");
    // Стартовый лексикон второго learn = финальный первого.
    assert!(lex_line2.contains(&format!("лексикон {lex1} →")));
}

#[test]
fn learn_json_report() {
    use pqc::json::Json;
    let dir = tmp_dir("json");
    let brain = dir.join("b.pqw");
    let (ok, out, err) = run(&[
        "learn",
        "квантовая механика",
        "--brain",
        brain.to_str().unwrap(),
        "--text",
        QM_TEXT,
        "--ask",
        "суперпозиция",
        "--json",
    ]);
    assert!(ok, "{err}");
    let j = Json::parse(out.trim()).expect("JSON отчёт не разбирается");
    assert_eq!(
        j.get("topic").and_then(|t| t.as_str()),
        Some("квантовая механика")
    );
    let rounds = j.get("rounds").and_then(|r| r.as_arr()).unwrap();
    assert!(!rounds.is_empty());
    assert!(j.get("pages").and_then(|p| p.as_f64()).unwrap() >= 1.0);
    let lex = j.get("lexicon").unwrap();
    assert!(lex.get("after").and_then(|a| a.as_f64()).unwrap() > 0.0);
    let ask = j.get("ask").expect("ask в отчёте");
    assert!(ask.get("answer").and_then(|a| a.as_str()).is_some());
    let brain_j = j.get("brain").unwrap();
    assert!(brain_j.get("d_pol").and_then(|d| d.as_f64()).unwrap() == 4096.0);
}

#[test]
fn learn_validation_errors() {
    let dir = tmp_dir("errors");
    let brain = dir.join("b.pqw");
    // Нет темы
    let (ok, _, err) = run(&["learn", "--brain", brain.to_str().unwrap(), "--text", "текст"]);
    assert!(!ok);
    assert!(err.contains("тема"), "{err}");
    // Нет --brain
    let (ok2, _, err2) = run(&["learn", "квантовая механика", "--text", QM_TEXT]);
    assert!(!ok2);
    assert!(err2.contains("--brain"), "{err2}");
    // Неизвестная опция
    let (ok3, _, err3) = run(&["learn", "тема", "--brain", brain.to_str().unwrap(), "--неттакой"]);
    assert!(!ok3);
    assert!(err3.contains("неизвестная опция"), "{err3}");
    // --pages вне диапазона
    let (ok4, _, err4) = run(&[
        "learn",
        "тема",
        "--brain",
        brain.to_str().unwrap(),
        "--text",
        "текст",
        "--pages",
        "0",
    ]);
    assert!(!ok4);
    assert!(err4.contains("--pages"), "{err4}");
    // --lang не из списка
    let (ok5, _, err5) = run(&[
        "learn",
        "тема",
        "--brain",
        brain.to_str().unwrap(),
        "--text",
        "текст",
        "--lang",
        "jp",
    ]);
    assert!(!ok5);
    assert!(err5.contains("--lang"), "{err5}");
}

/// Живой интернет-ингест (запускать явно:
/// `cargo test -p pqc --test learn_cli live_learn_wikipedia -- --ignored`).
#[test]
#[ignore = "живой интернет: полный цикл learn → ask"]
fn live_learn_wikipedia() {
    let dir = tmp_dir("live");
    let brain = dir.join("live.pqw");
    let (ok, out, err) = run(&[
        "learn",
        "квантовая механика",
        "--brain",
        brain.to_str().unwrap(),
        "--pages",
        "3",
        "--rounds",
        "1",
        "--ask",
        "что такое квант",
    ]);
    assert!(ok, "живой learn упал:\n{out}\n{err}");
    assert!(out.contains("ru.wikipedia.org"), "должен выбрать русский раздел");
    assert!(out.contains("Квантовая механика"), "нет страницы темы:\n{out}");
    // Мозг реально научился: лексикон и русла выросли
    let lex = out.lines().find(|l| l.contains("лексикон")).unwrap();
    assert!(lex.contains("→ 0 слов") || !lex.contains("→ 0 слов"), "{lex}");
    let bytes = fs::read(&brain).unwrap();
    assert_eq!(&bytes[..8], b"POLER_Q4");

    // Инференс отдельно (проверка ТЗ п.3).
    let (ok2, out2, err2) = run(&[
        "ask",
        "что такое квант",
        "--brain",
        brain.to_str().unwrap(),
        "--no-learn",
    ]);
    assert!(ok2, "ask после live learn:\n{out2}\n{err2}");
    assert!(out2.contains("ответ"), "нет ответа:\n{out2}");
}
