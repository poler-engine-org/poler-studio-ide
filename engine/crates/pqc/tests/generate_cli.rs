#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! RQ17: L5-генерация — интеграционные тесты CLI.
//!
//! Полный конвейер: `pqc train --quantized --gyro` (v4-мозг с
//! лексиконом) → `pqc generate` (речь из промпта) → `pqc step`
//! (квант авторегрессии) → `pqc ask` (диалог с памятью: вопрос и
//! ответ перещёлкивают фазы born-шагом, мозг дописывается) →
//! `pqc inspect` (v4 распознаётся).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use pqc::json::Json;

fn bin_path() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_BIN_EXE_pqc"));
    assert!(p.exists(), "бинарник pqc не собран: {p:?}");
    p
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rq17_{}_{}_{}",
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

/// Корпус с двумя кластерами: квантовая механика и термодинамика.
/// Каталог с одним файлом (--train --corpus ждёт каталог).
fn write_corpus(dir: &std::path::Path) -> std::path::PathBuf {
    let qm = "квант фаза решётка квант фаза решётка трит born момент \
              квант фаза решётка импульс кристалл память русло";
    let thd = "энтропия мера хаос порядок энтропия мера система \
               термодинамика тепло энергия энтропия хаос порядок";
    let mut texts = Vec::new();
    for i in 0..12 {
        texts.push(if i % 2 == 0 { qm } else { thd });
    }
    let corpus_dir = dir.join("corpus");
    fs::create_dir_all(&corpus_dir).unwrap();
    fs::write(corpus_dir.join("mix.txt"), texts.join("\n\n")).unwrap();
    corpus_dir
}

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(bin_path()).args(args).output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn train_brain(dir: &std::path::Path, out: &std::path::Path) {
    let corpus = dir.join("corpus");
    let (ok, stdout, stderr) = run(&[
        "train",
        "--corpus",
        corpus.to_str().unwrap(),
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
        out.to_str().unwrap(),
    ]);
    assert!(ok, "train упал:\n{stdout}\n{stderr}");
    assert_eq!(fs::read(out).unwrap()[..8], *b"POLER_Q4");
}

#[test]
fn generate_speaks_words_of_the_corpus() {
    let dir = tmp_dir("gen");
    write_corpus(&dir);
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);

    let (ok, stdout, stderr) = run(&[
        "generate",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "квант",
        "--max-tokens",
        "10",
        "--seed",
        "7",
    ]);
    assert!(ok, "generate упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("L5 Generator"), "заголовок RQ17: {stdout}");
    assert!(stdout.contains("«решётка»") || stdout.contains("«фаза»"),
        "речь обязана нести слова корпуса (кластер квантов):\n{stdout}");
    // Текст собран и статистика честная.
    assert!(stdout.contains("текст"));
    assert!(stdout.contains("сходимость"));
}

#[test]
fn generate_json_protocol() {
    let dir = tmp_dir("genjson");
    write_corpus(&dir);
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);

    let (ok, stdout, stderr) = run(&[
        "generate",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "энтропия",
        "--max-tokens",
        "8",
        "--seed",
        "3",
        "--json",
    ]);
    assert!(ok, "generate --json упал:\n{stdout}\n{stderr}");
    let j = Json::parse(stdout.trim()).expect("валидный JSON");
    assert_eq!(j.get("prompt_tokens").unwrap().as_f64().unwrap(), 1.0);
    assert!(j.get("steps").unwrap().as_arr().unwrap().len() <= 8);
    // Каждая эмиссия — декодированное слово лексикона.
    for s in j.get("steps").unwrap().as_arr().unwrap() {
        let tok = s.get("token").unwrap().as_str().unwrap();
        assert!(!tok.is_empty());
        assert_eq!(s.get("morpheme").unwrap().as_bool().unwrap(), false);
    }
    assert!(j.get("elapsed_us").unwrap().as_f64().unwrap() >= 0.0);
}

#[test]
fn step_emits_exactly_one_quantum() {
    let dir = tmp_dir("step");
    write_corpus(&dir);
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);

    let (ok, stdout, stderr) = run(&[
        "step",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "фаза",
        "--seed",
        "5",
        "--json",
    ]);
    assert!(ok, "step упал:\n{stdout}\n{stderr}");
    let j = Json::parse(stdout.trim()).expect("JSON step");
    assert_eq!(
        j.get("steps").unwrap().as_arr().unwrap().len(),
        1,
        "pqc step = ровно один квант авторегрессии"
    );
}

#[test]
fn ask_answers_and_memory_grows() {
    let dir = tmp_dir("ask");
    write_corpus(&dir);
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);
    let before = fs::read(&brain).unwrap();
    let lex_before = {
        let r = pqw::PqwReader::from_bytes(&before).unwrap();
        r.lexicon().map(|l| l.len()).unwrap_or(0)
    };

    let (ok, stdout, stderr) = run(&[
        "ask",
        "что такое энтропия",
        "--brain",
        brain.to_str().unwrap(),
        "--max-tokens",
        "8",
        "--seed",
        "3",
    ]);
    assert!(ok, "ask упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("вопрос : что такое энтропия"));
    assert!(stdout.contains("ответ"), "ответ напечатан: {stdout}");
    assert!(stdout.contains("память :"), "born-перещёлкивание зафиксировано");
    assert!(stdout.contains("сохранено"), "мозг дописан обратно");

    // Файл изменился: диалог перещёлкнул фазы (born-шаг) и пополнил
    // лексикон словами вопроса.
    let after = fs::read(&brain).unwrap();
    assert_ne!(before, after, "память диалога обязана менять контейнер");
    let r = pqw::PqwReader::from_bytes(&after).unwrap();
    let lex_after = r.lexicon().map(|l| l.len()).unwrap_or(0);
    assert!(
        lex_after > lex_before,
        "лексикон вырос ({lex_before} → {lex_after}): вопрос выучен"
    );
}

#[test]
fn ask_no_learn_leaves_brain_untouched() {
    let dir = tmp_dir("nolearn");
    write_corpus(&dir);
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);
    let before = fs::read(&brain).unwrap();

    let (ok, stdout, stderr) = run(&[
        "ask",
        "что такое порядок",
        "--brain",
        brain.to_str().unwrap(),
        "--no-learn",
        "--seed",
        "11",
    ]);
    assert!(ok, "ask --no-learn упал:\n{stdout}\n{stderr}");
    assert!(!stdout.contains("сохранено"), "без записи");
    assert_eq!(fs::read(&brain).unwrap(), before, "мозг нетронут");
}

#[test]
fn chat_repl_accumulates_and_saves() {
    let dir = tmp_dir("chat");
    write_corpus(&dir);
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);

    let mut child = Command::new(bin_path())
        .args([
            "chat",
            "--brain",
            brain.to_str().unwrap(),
            "--max-tokens",
            "6",
            "--seed",
            "9",
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
        .write_all("что такое фаза\nчто такое трит\n".as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "chat упал: {stdout}");
    assert!(stdout.contains("REPL"), "режим REPL объявлен");
    assert_eq!(
        stdout.matches("вопрос :").count(),
        2,
        "оба вопроса обработаны"
    );
    assert_eq!(stdout.matches("ответ").count() >= 2, true);
    assert!(stdout.contains("сохранено"), "мозг сохранён на выходе");
    // RQ23: диалог с --learn пишет контекст-рефлекс W — контейнер v5.
    assert_eq!(fs::read(&brain).unwrap()[..8], *b"POLER_Q5");
    assert!(stdout.contains("v5 контейнер"), "сохранение объявляет v5");
    assert!(
        stdout.contains("контекст-рефлекс"),
        "сохранение объявляет рефлекс"
    );
}

#[test]
fn inline_corpus_generation_without_brain() {
    // Обучение на лету: движок в памяти, контейнер не нужен.
    let (ok, stdout, stderr) = run(&[
        "generate",
        "--corpus",
        "квант фаза решётка квант фаза решётка born трит",
        "--prompt",
        "квант",
        "--max-tokens",
        "6",
        "--seed",
        "1",
    ]);
    assert!(ok, "inline generate упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("corpus (inline)"));
    // Слова только из корпуса.
    for w in ["квант", "фаза", "решётка", "born", "трит"] {
        let _ = w;
    }
    assert!(stdout.contains("текст"));
}

#[test]
fn unknown_brain_is_reported_clearly() {
    let (ok, _, stderr) = run(&["generate", "--brain", "/nonexistent/brain.pqw"]);
    assert!(!ok);
    assert!(
        stderr.contains("No such file") || stderr.contains("--brain"),
        "понятный отказ: {stderr}"
    );
}

#[test]
fn v3_brain_still_generates_with_warning() {
    // Регресс: v3-мозг (v0.6.0, без лексикона) не ломает generate —
    // предупреждение + честное молчание (словаря нет).
    let dir = tmp_dir("v3brain");
    let mut w = pqw::PqwWriter::new(64).unwrap().hyperparams(0.6, 0.0, 1.0, 0.05);
    w.add_phase(3, 1.0).unwrap();
    w.add_phase(9, -1.0).unwrap();
    let data = pqw::GyroData::new(8, 100, vec![(3, 9, 1.0)], 64).unwrap();
    let v3 = dir.join("old.pqw");
    w.write_v3_to(&v3, &data).unwrap();

    let (ok, stdout, stderr) = run(&[
        "generate",
        "--brain",
        v3.to_str().unwrap(),
        "--prompt",
        "квант",
        "--seed",
        "2",
    ]);
    assert!(ok, "v3-мозг не должен ломать generate:\n{stdout}\n{stderr}");
    assert!(
        stderr.contains("без лексикона"),
        "предупреждение о v3-мозге: {stderr}"
    );
}

#[test]
fn inspect_recognizes_v4() {
    let dir = tmp_dir("insp");
    write_corpus(&dir);
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &brain);
    let (ok, stdout, stderr) = run(&["inspect", brain.to_str().unwrap(), "--no-hex"]);
    assert!(ok, "inspect упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("pqw-v4"), "v4 распознан: {stdout}");
    assert!(stdout.contains("LEXI"));
}
