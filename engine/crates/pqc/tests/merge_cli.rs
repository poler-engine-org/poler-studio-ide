#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! RQ21: merge --settle + грамматические мосты — интеграционные
//! тесты CLI.
//!
//! Конвейер: `pqc train --quantized --gyro` (два v4-мозга) →
//! `pqc merge --settle [N]` (слияние ⊗_ε + консолидация волной
//! Π_Λ(e^{Δt·J} p) + стационар ĤΨ = 0) → `pqc ask` (мульти-доменная
//! речь с грамматическими мостами). Отдельно: `--no-syntax`
//! возвращает чистую ассоциативную топологию RQ17.

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
        "rq21_{}_{}_{}",
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

/// Корпус одного домена: плотная цепочка знаменательных слов.
fn write_corpus(dir: &std::path::Path, name: &str, text: &str) -> PathBuf {
    let corpus_dir = dir.join(name);
    fs::create_dir_all(&corpus_dir).unwrap();
    fs::write(corpus_dir.join("mix.txt"), text.repeat(6)).unwrap();
    corpus_dir
}

const PHYSICS: &str = "квант фаза решётка момент импульс кристалл память \
                       русло трит шаг волна полюс заряд спин энергия фотон \
                       поле гармоника кубит запутанность суперпозиция ";
const PYTHON: &str = "генератор итератор замыкание поток декоратор функция \
                      список контекст менеджер корутина asyncio поток ленивые \
                      вычисления компилятор байткод стек кадр куча объект ";

fn train_brain(_dir: &std::path::Path, corpus: &std::path::Path, out: &std::path::Path) {
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

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(bin_path()).args(args).output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn merge_settle_reports_and_writes_v4() {
    let dir = tmp_dir("mrgsettle");
    let corpus_a = write_corpus(&dir, "physics", PHYSICS);
    let corpus_b = write_corpus(&dir, "python", PYTHON);
    let a = dir.join("a.pqw");
    let b = dir.join("b.pqw");
    let m = dir.join("m.pqw");
    train_brain(&dir, &corpus_a, &a);
    train_brain(&dir, &corpus_b, &b);

    // Флаг с явным числом тактов + живой вопрос на стыке доменов.
    let (ok, stdout, stderr) = run(&[
        "merge",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--out",
        m.to_str().unwrap(),
        "--settle",
        "3",
        "--ask",
        "как моделировать запутанность генератором",
    ]);
    assert!(ok, "merge --settle упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("Merge (RQ20)"), "заголовок: {stdout}");
    assert!(stdout.contains("сеттлинг"), "отчёт сеттлинга: {stdout}");
    assert!(stdout.contains("стационар"), "стационар ĤΨ = 0: {stdout}");
    assert!(stdout.contains("ответ"), "ответ на стыковом вопросе: {stdout}");

    // Слитый и осевший мозг — валидный v4.
    let bytes = fs::read(&m).unwrap();
    assert_eq!(&bytes[..8], b"POLER_Q4");

    // Сеттлинг детерминирован: повтор слияния+сеттлинга бит-в-бит.
    let m2 = dir.join("m2.pqw");
    let (ok2, _, stderr2) = run(&[
        "merge",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--out",
        m2.to_str().unwrap(),
        "--settle",
        "3",
    ]);
    assert!(ok2, "повторный merge --settle:\n{stderr2}");
    assert_eq!(
        fs::read(&m).unwrap(),
        fs::read(&m2).unwrap(),
        "сеттлинг обязан быть детерминированным бит-в-бит"
    );
}

#[test]
fn merge_settle_json_protocol() {
    let dir = tmp_dir("mrgjson");
    let corpus_a = write_corpus(&dir, "physics", PHYSICS);
    let corpus_b = write_corpus(&dir, "python", PYTHON);
    let a = dir.join("a.pqw");
    let b = dir.join("b.pqw");
    let m = dir.join("m.pqw");
    train_brain(&dir, &corpus_a, &a);
    train_brain(&dir, &corpus_b, &b);

    let (ok, stdout, stderr) = run(&[
        "merge",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--out",
        m.to_str().unwrap(),
        "--settle",
        "--json",
    ]);
    assert!(ok, "merge --settle --json упал:\n{stdout}\n{stderr}");
    let j = Json::parse(stdout.trim()).expect("валидный JSON");
    let settle = j.get("settle").expect("секция settle");
    let ticks = settle.get("ticks_run").unwrap().as_f64().unwrap();
    assert!(ticks >= 1.0 && ticks <= 3.0, "тактов 1..=3: {ticks}");
    let moved = settle.get("moved_per_tick").unwrap().as_arr().unwrap();
    assert_eq!(moved.len() as f64, ticks, "moved_per_tick согласован");
    // Ранний выход только на стационаре: не осел — отработал все такты.
    let stationary = settle.get("stationary").unwrap().as_bool().unwrap();
    if !stationary {
        assert_eq!(
            ticks,
            settle.get("ticks_requested").unwrap().as_f64().unwrap(),
            "без стационара — отработаны все запрошенные такты"
        );
    }
    // Закон сохранения русел.
    assert_eq!(
        settle.get("channels_after").unwrap().as_f64().unwrap(),
        settle.get("channels_before").unwrap().as_f64().unwrap()
            + settle.get("channels_grown").unwrap().as_f64().unwrap(),
        "каналы сходятся в арифметику"
    );
}

#[test]
fn merge_settle_flag_variants() {
    let dir = tmp_dir("mrgflags");
    let corpus_a = write_corpus(&dir, "physics", PHYSICS);
    let corpus_b = write_corpus(&dir, "python", PYTHON);
    let a = dir.join("a.pqw");
    let b = dir.join("b.pqw");
    train_brain(&dir, &corpus_a, &a);
    train_brain(&dir, &corpus_b, &b);

    // --settle без значения → 3 такта (ТЗ 2–4).
    let m_default = dir.join("m_default.pqw");
    let (ok, stdout, stderr) = run(&[
        "merge",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--out",
        m_default.to_str().unwrap(),
        "--settle",
        "--json",
    ]);
    assert!(ok, "--settle без значения:\n{stdout}\n{stderr}");
    let j = Json::parse(stdout.trim()).unwrap();
    assert_eq!(
        j.get("settle").unwrap().get("ticks_requested").unwrap().as_f64().unwrap(),
        3.0,
        "дефолт сеттлинга — 3 такта"
    );

    // --settle рядом с другими флагами: значение не съедает флаг.
    let m_flag = dir.join("m_flag.pqw");
    let (ok, stdout, stderr) = run(&[
        "merge",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--out",
        m_flag.to_str().unwrap(),
        "--settle",
        "--ask",
        "что такое квант",
    ]);
    assert!(ok, "--settle --ask:\n{stdout}\n{stderr}");
    assert!(stdout.contains("сеттлинг"), "сеттлинг выполнен: {stdout}");
    assert!(stdout.contains("вопрос"), "ask не съеден: {stdout}");

    // Границы валидации.
    for bad in ["0", "17", "abc"] {
        let m_bad = dir.join("m_bad.pqw");
        let (ok, _, stderr) = run(&[
            "merge",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
            "--out",
            m_bad.to_str().unwrap(),
            "--settle",
            bad,
        ]);
        assert!(!ok, "--settle {bad} принят");
        assert!(stderr.contains("--settle"), "понятный отказ: {stderr}");
    }
}

#[test]
fn merge_without_settle_keeps_r20_behavior() {
    // Без --settle слияние честно RQ20: строки сеттлинга нет.
    let dir = tmp_dir("mrgplain");
    let corpus_a = write_corpus(&dir, "physics", PHYSICS);
    let corpus_b = write_corpus(&dir, "python", PYTHON);
    let a = dir.join("a.pqw");
    let b = dir.join("b.pqw");
    let m = dir.join("m.pqw");
    train_brain(&dir, &corpus_a, &a);
    train_brain(&dir, &corpus_b, &b);

    let (ok, stdout, stderr) = run(&[
        "merge",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--out",
        m.to_str().unwrap(),
    ]);
    assert!(ok, "merge без settle:\n{stdout}\n{stderr}");
    assert!(!stdout.contains("сеттлинг"), "сеттлинга нет без флага");
    assert_eq!(fs::read(&m).unwrap()[..8], *b"POLER_Q4");
}

#[test]
fn generate_syntax_bridges_on_by_default() {
    // CLI-дефолт RQ21: грамматические мосты включены — длинная речь
    // содержит коннекторы, облака разорваны.
    let dir = tmp_dir("syntax");
    let corpus = write_corpus(&dir, "chain", PHYSICS);
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &corpus, &brain);

    let (ok, stdout, stderr) = run(&[
        "generate",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "квант",
        "--max-tokens",
        "30",
        "--seed",
        "7",
    ]);
    assert!(ok, "generate упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("мостов"), "статистика мостов: {stdout}");
    // Речь длинная — мосты обязаны появиться (облака ≤ BRIDGE_RUN).
    let stats = stdout
        .lines()
        .find(|l| l.contains("статистика"))
        .expect("строка статистики");
    assert!(
        stats.contains("мостов 1") || stats.contains("мостов 2")
            || stats.contains("мостов 3") || stats.contains("мостов 4")
            || stats.contains("мостов 5") || stats.contains("мостов 6")
            || stats.contains("мостов 7") || stats.contains("мостов 8")
            || stats.contains("мостов 9"),
        "мостов ≥ 1 в длинной речи: {stats}"
    );
}

#[test]
fn generate_no_syntax_returns_pure_r17() {
    // --no-syntax: чистая топология RQ17 — ни одного моста.
    let dir = tmp_dir("nosyntax");
    let corpus = write_corpus(&dir, "chain", PHYSICS);
    let brain = dir.join("brain.pqw");
    train_brain(&dir, &corpus, &brain);

    let (ok, stdout, stderr) = run(&[
        "generate",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "квант",
        "--max-tokens",
        "30",
        "--seed",
        "7",
        "--no-syntax",
        "--json",
    ]);
    assert!(ok, "generate --no-syntax упал:\n{stdout}\n{stderr}");
    let j = Json::parse(stdout.trim()).expect("JSON");
    assert_eq!(
        j.get("bridge_tokens").unwrap().as_f64().unwrap(),
        0.0,
        "мостов нет без синтаксиса"
    );
    for s in j.get("steps").unwrap().as_arr().unwrap() {
        assert_ne!(
            s.get("source").unwrap().as_str().unwrap(),
            "bridge",
            "шагов Bridge нет: {:?}",
            s.get("token")
        );
    }
}
