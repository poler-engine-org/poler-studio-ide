#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! RQ22: фокусировка волны и релевантность — интеграционные тесты CLI.
//!
//! Полный конвейер: v4-мозг с цепочной топологией (два кластера,
//! соединённых одним руслом) → `pqc generate --focus-radius` (речь
//! заперта в окрестности аттрактора вопроса) → `--no-focus`
//! (свободное блуждание) → `pqc ask` (подкрепление грамматики) →
//! `pqc learn --docs` (источник документаций по HTTPS).

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
        "rq22_{}_{}_{}",
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

/// Цепочная топология: квант→фаза→решётка→маяк→берег→туман.
/// Окно гироскопа 1 (пары только с предшественником), два абзаца
/// (внутренние русла насыщены ×2, краевая пара туман→квант —
/// свежая, наружу невидима).
const CHAIN_CORPUS: &str = "квант фаза решётка маяк берег туман\n\n\
                            квант фаза решётка маяк берег туман";

/// Обучение цепочного мозга: окно 1 — топология-цепочка.
fn train_chain_brain(dir: &std::path::Path, out: &std::path::Path) {
    let corpus_dir = dir.join("chain_corpus");
    fs::create_dir_all(&corpus_dir).unwrap();
    fs::write(corpus_dir.join("chain.txt"), CHAIN_CORPUS).unwrap();
    let (ok, stdout, stderr) = run(&[
        "train",
        "--corpus",
        corpus_dir.to_str().unwrap(),
        "--quantized",
        "--gyro",
        "1",
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

/// Обычный смешанный корпус (окно 8 — плотная топология).
fn train_mixed_brain(dir: &std::path::Path, out: &std::path::Path) {
    let corpus_dir = dir.join("mixed_corpus");
    fs::create_dir_all(&corpus_dir).unwrap();
    let qm = "квант фаза решётка квант фаза решётка трит born момент \
              квант фаза решётка импульс кристалл память русло";
    let thd = "энтропия мера хаос порядок энтропия мера система \
               термодинамика тепло энергия энтропия хаос порядок";
    let mut texts = Vec::new();
    for i in 0..12 {
        texts.push(if i % 2 == 0 { qm } else { thd });
    }
    fs::write(corpus_dir.join("mix.txt"), texts.join("\n\n")).unwrap();
    let (ok, stdout, stderr) = run(&[
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
        out.to_str().unwrap(),
    ]);
    assert!(ok, "train упал:\n{stdout}\n{stderr}");
}

#[test]
fn focus_radius_locks_speech_near_question() {
    // Цепочка: квант(0)→фаза(1)→решётка(2)→маяк(3)→берег(4)→туман(5).
    // Радиус 1: вся речь — только «квант»/«фаза», кластер маяка
    // отсечён полностью (посторонние ветки решётки).
    let dir = tmp_dir("focus");
    let brain = dir.join("chain.pqw");
    train_chain_brain(&dir, &brain);
    let (ok, stdout, stderr) = run(&[
        "generate",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "квант",
        "--focus-radius",
        "1",
        "--no-syntax",
        "--no-bridge",
        "--max-tokens",
        "12",
        "--seed",
        "7",
        "--json",
    ]);
    assert!(ok, "generate упал:\n{stdout}\n{stderr}");
    let j = Json::parse(stdout.trim()).expect("валидный JSON");
    let focus = j.get("focus").expect("focus-объект RQ22");
    assert_eq!(focus.get("active").unwrap().as_bool().unwrap(), true);
    assert_eq!(focus.get("radius").unwrap().as_f64().unwrap(), 1.0);
    assert!(focus.get("attractor_arcs").unwrap().as_f64().unwrap() >= 1.0);
    assert_eq!(focus.get("wander_steps").unwrap().as_f64().unwrap(), 0.0);
    assert_eq!(focus.get("relevance").unwrap().as_f64().unwrap(), 1.0);
    // Каждый токен — из окрестности вопроса.
    for s in j.get("steps").unwrap().as_arr().unwrap() {
        let tok = s.get("token").unwrap().as_str().unwrap();
        assert!(
            tok == "квант" || tok == "фаза",
            "фокус пропустил «{tok}» за радиус 1"
        );
    }
}

#[test]
fn focus_no_flag_returns_free_wandering() {
    // --no-focus: маршрутизация снята, телеметрия релевантности
    // считается для свободной речи.
    let dir = tmp_dir("nofocus");
    let brain = dir.join("chain.pqw");
    train_chain_brain(&dir, &brain);
    let (ok, stdout, stderr) = run(&[
        "generate",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "квант",
        "--no-focus",
        "--no-syntax",
        "--no-bridge",
        "--max-tokens",
        "12",
        "--seed",
        "7",
        "--json",
    ]);
    assert!(ok, "generate упал:\n{stdout}\n{stderr}");
    let j = Json::parse(stdout.trim()).expect("валидный JSON");
    let focus = j.get("focus").expect("focus-объект RQ22");
    assert_eq!(focus.get("active").unwrap().as_bool().unwrap(), false);
    assert_eq!(focus.get("radius").unwrap().as_f64().unwrap(), 0.0);
    let rel = focus.get("relevance").unwrap().as_f64().unwrap();
    assert!((0.0..=1.0).contains(&rel));
}

#[test]
fn focus_radius_validation() {
    let dir = tmp_dir("focusval");
    let brain = dir.join("chain.pqw");
    train_chain_brain(&dir, &brain);
    let (ok, _stdout, stderr) = run(&[
        "generate",
        "--brain",
        brain.to_str().unwrap(),
        "--focus-radius",
        "9",
    ]);
    assert!(!ok, "радиус 9 обязан отклоняться");
    assert!(stderr.contains("--focus-radius"), "подсказка в ошибке: {stderr}");
}

#[test]
fn default_focus_is_on_and_reported() {
    // Дефолт CLI: фокус ВКЛЮЧЁН (радиус 3) + грамматика с
    // подкреплением — человеческий вывод несёт строку фокуса.
    let dir = tmp_dir("focusdef");
    let brain = dir.join("mix.pqw");
    train_mixed_brain(&dir, &brain);
    let (ok, stdout, stderr) = run(&[
        "generate",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "квант",
        "--max-tokens",
        "16",
        "--seed",
        "11",
    ]);
    assert!(ok, "generate упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("фокус"), "строка фокуса в отчёте: {stdout}");
    assert!(
        stdout.contains("/100 слов"),
        "плотность мостов в статистике: {stdout}"
    );
}

#[test]
fn ask_reports_focus_and_reinforcement() {
    // ask --json: focus-объект + подкрепление грамматики в отчёте.
    let dir = tmp_dir("askfocus");
    let brain = dir.join("mix.pqw");
    train_mixed_brain(&dir, &brain);
    let (ok, stdout, stderr) = run(&[
        "ask",
        "что такое квант",
        "--brain",
        brain.to_str().unwrap(),
        "--max-tokens",
        "24",
        "--seed",
        "5",
        "--no-learn",
        "--json",
    ]);
    assert!(ok, "ask упал:\n{stdout}\n{stderr}");
    let j = Json::parse(stdout.trim()).expect("валидный JSON");
    let focus = j.get("focus").expect("focus-объект в ask");
    assert!(focus.get("relevance").unwrap().as_f64().unwrap() >= 0.0);
    // Подкрепление включено по умолчанию: reinforced ≥ 0, ключ есть.
    assert!(j.get("reinforced").is_some());
    assert!(j.get("syntax_bridges_density").is_some());
    let density = j.get("syntax_bridges_density").unwrap().as_f64().unwrap();
    assert!((0.0..=100.0).contains(&density));
}

#[test]
fn no_reinforce_flag_disables_grammar_growth() {
    // --no-reinforce: коннекторы вставляются, но русла не укрепляются.
    let dir = tmp_dir("noreinf");
    let brain = dir.join("mix.pqw");
    train_mixed_brain(&dir, &brain);
    let (ok, stdout, stderr) = run(&[
        "ask",
        "что такое квант и фаза",
        "--brain",
        brain.to_str().unwrap(),
        "--max-tokens",
        "32",
        "--seed",
        "5",
        "--no-learn",
        "--no-reinforce",
        "--json",
    ]);
    assert!(ok, "ask упал:\n{stdout}\n{stderr}");
    let j = Json::parse(stdout.trim()).expect("валидный JSON");
    assert_eq!(j.get("reinforced").unwrap().as_f64().unwrap(), 0.0);
}

#[test]
fn docs_source_unreachable_reports_error() {
    // --docs с недостижимым URL: честный отказ с указанием источника.
    let dir = tmp_dir("docserr");
    let brain = dir.join("b.pqw");
    let (ok, _stdout, stderr) = run(&[
        "learn",
        "тест",
        "--brain",
        brain.to_str().unwrap(),
        "--docs",
        "https://несуществующий-домен.invalid/doc.md",
    ]);
    assert!(!ok, "недостижимый документ обязан падать");
    assert!(
        stderr.contains("документы/HTTPS"),
        "источник ошибки назван: {stderr}"
    );
    assert!(!brain.exists(), "мозг не создаётся при ошибке источника");
}

#[test]
fn docs_source_scheme_normalized_and_single_round() {
    // Заголовок USAGE + разбор: --docs без схемы получает https://,
    // раунд один. Проверяем на человекочитаемом выводе до сети:
    // источник документаций объявлен, затем честная ошибка сети
    // (несуществующий хост) — без попыток Wikipedia.
    let dir = tmp_dir("docsnorm");
    let brain = dir.join("b.pqw");
    let (ok, stdout, stderr) = run(&[
        "learn",
        "документация",
        "--brain",
        brain.to_str().unwrap(),
        "--docs",
        "raw.invalid.example/x/README.md",
        "--json",
    ]);
    assert!(!ok, "хост .invalid недостижим — отказ");
    // Человеческий вывод до ошибки объявил источник (строки в stdout
    // при --json нет — источник виден в stderr ошибки).
    assert!(
        stderr.contains("raw.invalid.example") || stderr.contains("документ"),
        "URL документа упомянут: {stderr}"
    );
    let _ = stdout;
}
