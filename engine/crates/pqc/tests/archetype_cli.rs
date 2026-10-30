#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! RQ18: нелинейная архетипическая алгебра `⊗_ε` — интеграционные
//! тесты CLI.
//!
//! Конвейер: `pqc train --quantized --gyro` (мозг v4) →
//! `pqc archetype` (мозг ⊗ промпт, мозг ⊗ мозг — структурный
//! изоморфизм знаний) → `pqc generate/ask` с мостом ⊗_ε в лотерее
//! речи (`--no-bridge` / `--bridge-eps`).

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
        "rq18_{}_{}_{}",
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

/// Квантовый кластер (словарь A).
const QM: &str = "квант фаза решётка квант фаза решётка трит born момент \
                  импульс кристалл память русло";

/// Термодинамический кластер (словарь B, строго непересекающийся с A).
const THD: &str = "энтропия мера хаос порядок система термодинамика \
                   тепло энергия пар газ энтропия мера";

fn write_corpus(dir: &std::path::Path, name: &str, text: &str) -> PathBuf {
    let corpus_dir = dir.join(name);
    fs::create_dir_all(&corpus_dir).unwrap();
    let mut docs = Vec::new();
    for i in 0..8 {
        docs.push(text);
        let _ = i;
    }
    fs::write(corpus_dir.join("docs.txt"), docs.join("\n\n")).unwrap();
    corpus_dir
}

fn train_brain(corpus: &std::path::Path, out: &std::path::Path) {
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
fn archetype_brain_vs_prompt_resonates() {
    // Знакомый вопрос изоморфен памяти: гейт открыт, пересечение
    // непусто, продукт — новый устойчивый смысл.
    let dir = tmp_dir("arch_prompt");
    let corpus = write_corpus(&dir, "corpus", QM);
    let brain = dir.join("brain.pqw");
    train_brain(&corpus, &brain);

    let (ok, stdout, stderr) = run(&[
        "archetype",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "квант фаза решётка",
        "--eps",
        "0.25",
    ]);
    assert!(ok, "archetype упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("Archetype Algebra (RQ18)"), "{stdout}");
    assert!(stdout.contains("РЕЗОНАНС"), "знакомый вопрос обязан резонировать:\n{stdout}");
    assert!(stdout.contains("согласие"));
    // Идемпотентность заявлена в отчёте (ТЗ п.3).
    assert!(stdout.contains("c ⊗_ε c = c"));
}

#[test]
fn archetype_orthogonal_prompt_gives_zero() {
    // Чужой вопрос (термодинамика в квантовом мозге): энергия мала,
    // гейт заперт — произведение Zero, ложных ассоциаций нет.
    let dir = tmp_dir("arch_ortho");
    let corpus = write_corpus(&dir, "corpus", QM);
    let brain = dir.join("brain.pqw");
    train_brain(&corpus, &brain);

    let (ok, stdout, stderr) = run(&[
        "archetype",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "энтропия термодинамика хаос пар газ",
    ]);
    assert!(ok, "archetype упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("ЗАПЕРТ"), "ортogonalьные архетипы:\n{stdout}");
    assert!(stdout.contains("Zero"), "{stdout}");
}

#[test]
fn archetype_two_brains_isomorphism() {
    // Мозг ⊗ мозг: два мозга на ОДНОМ корпусе — идентичные решётки,
    // E = 1, полный резонанс. Два мозга на НЕПЕРЕСЕКАЮЩИХСЯ словарях —
    // гейт заперт (структурного изоморфизма нет).
    let dir = tmp_dir("arch_two");
    let corpus_a = write_corpus(&dir, "corpus_a", QM);
    let corpus_b = write_corpus(&dir, "corpus_b", THD);
    let brain_a = dir.join("a.pqw");
    let brain_a2 = dir.join("a2.pqw");
    let brain_b = dir.join("b.pqw");
    train_brain(&corpus_a, &brain_a);
    train_brain(&corpus_a, &brain_a2);
    train_brain(&corpus_b, &brain_b);

    // Один и тот же корпус → один и тот же архетип: E = 1.
    let (ok, stdout, stderr) = run(&[
        "archetype",
        "--brain",
        brain_a.to_str().unwrap(),
        "--with",
        brain_a2.to_str().unwrap(),
    ]);
    assert!(ok, "archetype --with упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("РЕЗОНАНС"), "одинаковые мозги обязаны резонировать:\n{stdout}");
    assert!(stdout.contains("E = co/min(nnz) = 1.000"), "{stdout}");

    // Непересекающиеся словари: co ≈ 0 (хеш-коллизии ≤ единиц),
    // E ≈ 0 < 0.5 — заперто.
    let (ok, stdout, stderr) = run(&[
        "archetype",
        "--brain",
        brain_a.to_str().unwrap(),
        "--with",
        brain_b.to_str().unwrap(),
    ]);
    assert!(ok, "archetype --with упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("ЗАПЕРТ"), "разные домены не изоморфны:\n{stdout}");
}

#[test]
fn archetype_json_report() {
    let dir = tmp_dir("arch_json");
    let corpus = write_corpus(&dir, "corpus", QM);
    let brain = dir.join("brain.pqw");
    train_brain(&corpus, &brain);

    let (ok, stdout, stderr) = run(&[
        "archetype",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "квант",
        "--json",
    ]);
    assert!(ok, "archetype --json упал:\n{stdout}\n{stderr}");
    let v = Json::parse(stdout.trim()).expect("валидный JSON");
    // Структура: brain / operand / product / eps.
    assert!(v.get("brain").is_some(), "нет brain: {stdout}");
    assert!(v.get("operand").is_some(), "нет operand: {stdout}");
    assert!(v.get("eps").is_some(), "нет eps: {stdout}");
    let product = v.get("product").expect("нет product");
    for key in ["nnz", "co_support", "resonance", "conflict", "energy", "resonant"] {
        assert!(product.get(key).is_some(), "product.{key}: {stdout}");
    }
    // Знакомое слово резонирует.
    let resonant = product
        .get("resonant")
        .and_then(Json::as_bool)
        .expect("resonant — bool");
    assert!(resonant, "«квант» изоморфен квантовому мозгу: {stdout}");
    // Энергия в [0, 1].
    let energy = product
        .get("energy")
        .and_then(Json::as_f64)
        .expect("energy — число");
    assert!((0.0..=1.0).contains(&energy));
    assert!(energy > 0.0, "знакомое слово даёт пересечение: {energy}");
}

#[test]
fn archetype_contract_errors() {
    let dir = tmp_dir("arch_err");
    let corpus = write_corpus(&dir, "corpus", QM);
    let brain = dir.join("brain.pqw");
    train_brain(&corpus, &brain);

    // Без --brain.
    let (ok, _, stderr) = run(&["archetype", "--prompt", "квант"]);
    assert!(!ok);
    assert!(stderr.contains("нужен --brain"));

    // --with и --prompt вместе.
    let (ok, _, stderr) = run(&[
        "archetype",
        "--brain",
        brain.to_str().unwrap(),
        "--with",
        brain.to_str().unwrap(),
        "--prompt",
        "квант",
    ]);
    assert!(!ok);
    assert!(stderr.contains("взаимно исключают"));

    // ε вне (0, 1].
    for bad in ["0", "1.5", "-0.5", "abc"] {
        let (ok, _, stderr) = run(&[
            "archetype",
            "--brain",
            brain.to_str().unwrap(),
            "--eps",
            bad,
        ]);
        assert!(!ok, "ε={bad} принят");
        assert!(stderr.contains("--eps"));
    }

    // Размерности решёток различаются.
    let small = dir.join("small.pqw");
    let small_corpus = write_corpus(&dir, "small", QM);
    let (ok, stdout, stderr) = run(&[
        "train",
        "--corpus",
        small_corpus.to_str().unwrap(),
        "--quantized",
        "--gyro",
        "8",
        "--dim",
        "256",
        "--steps",
        "2",
        "--seed",
        "42",
        "--out",
        small.to_str().unwrap(),
    ]);
    assert!(ok, "train упал:\n{stdout}\n{stderr}");
    let (ok, _, stderr) = run(&[
        "archetype",
        "--brain",
        brain.to_str().unwrap(),
        "--with",
        small.to_str().unwrap(),
    ]);
    assert!(!ok);
    assert!(stderr.contains("размерности решёток различаются"));
}

#[test]
fn generate_bridge_line_and_no_bridge() {
    // Мост по умолчанию включён: generate печатает строку моста;
    // --no-bridge её убирает, речь работает как в RQ17.
    let dir = tmp_dir("gen_bridge");
    let corpus = write_corpus(&dir, "corpus", QM);
    let brain = dir.join("brain.pqw");
    train_brain(&corpus, &brain);

    let (ok, stdout, stderr) = run(&[
        "generate",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "квант фаза",
        "--max-tokens",
        "8",
        "--seed",
        "7",
    ]);
    assert!(ok, "generate упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("мост ⊗_ε"), "строка моста:\n{stdout}");

    let (ok, stdout, stderr) = run(&[
        "generate",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "квант фаза",
        "--max-tokens",
        "8",
        "--seed",
        "7",
        "--no-bridge",
    ]);
    assert!(ok, "generate --no-bridge упал:\n{stdout}\n{stderr}");
    assert!(!stdout.contains("мост ⊗_ε"), "мост выключен:\n{stdout}");
    assert!(stdout.contains("L5 Generator"), "{stdout}");
}

#[test]
fn generate_bridge_json_and_eps() {
    // JSON-отчёт несёт объект bridge (энергия/пересечение/резонанс);
    // --bridge-eps 0.99 запирает гейт на слабом пересечении.
    let dir = tmp_dir("gen_json");
    let corpus = write_corpus(&dir, "corpus", QM);
    let brain = dir.join("brain.pqw");
    train_brain(&corpus, &brain);

    let (ok, stdout, stderr) = run(&[
        "generate",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "квант фаза решётка",
        "--max-tokens",
        "6",
        "--seed",
        "7",
        "--json",
    ]);
    assert!(ok, "generate --json упал:\n{stdout}\n{stderr}");
    let v = Json::parse(stdout.trim()).expect("валидный JSON");
    let bridge = v.get("bridge").expect("bridge — объект");
    for key in ["energy", "co_support", "resonance", "conflict", "resonant"] {
        assert!(bridge.get(key).is_some(), "bridge.{key}");
    }
    // Источник archetype легитимен в steps (RQ21 добавил bridge —
    // грамматические мосты включены в CLI по умолчанию).
    if let Some(steps) = v.get("steps").and_then(Json::as_arr) {
        for s in steps {
            let src = s
                .get("source")
                .and_then(Json::as_str)
                .expect("source — строка");
            assert!(
                ["flow", "backtrack", "archetype", "kinetic", "bridge"].contains(&src),
                "источник {src}"
            );
        }
    }

    // Жёсткий порог: три токена, из них один знаком (co = 1 из 3) —
    // E = 1/3 < 0.99, гейт заперт (ложных ассоциаций нет).
    let (ok, stdout, _) = run(&[
        "generate",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "зукбар шлёп квант",
        "--max-tokens",
        "4",
        "--seed",
        "7",
        "--bridge-eps",
        "0.99",
        "--json",
    ]);
    assert!(ok, "generate --bridge-eps упал: {stdout}");
    let v = Json::parse(stdout.trim()).expect("валидный JSON");
    let bridge = v.get("bridge").expect("bridge");
    let energy = bridge
        .get("energy")
        .and_then(Json::as_f64)
        .expect("energy — число");
    let resonant = bridge
        .get("resonant")
        .and_then(Json::as_bool)
        .expect("bridge.resonant");
    assert!((energy - 1.0 / 3.0).abs() < 1e-9, "E = 1/3: {energy}");
    assert!(!resonant, "E = 1/3 < 0.99 — гейт заперт: {stdout}");

    // Порог за границей (0, 1] отвергается.
    let (ok, _, stderr) = run(&[
        "generate",
        "--brain",
        brain.to_str().unwrap(),
        "--prompt",
        "квант",
        "--bridge-eps",
        "1.5",
    ]);
    assert!(!ok);
    assert!(stderr.contains("--bridge-eps"));
}

#[test]
fn ask_bridge_present() {
    // ask: мост участвует в диалоге (строка статистики не обязательна —
    // команда обязана отработать и с мостом, и без него).
    let dir = tmp_dir("ask_bridge");
    let corpus = write_corpus(&dir, "corpus", QM);
    let brain = dir.join("brain.pqw");
    train_brain(&corpus, &brain);

    let (ok, stdout, stderr) = run(&[
        "ask",
        "что такое квант?",
        "--brain",
        brain.to_str().unwrap(),
        "--max-tokens",
        "8",
        "--seed",
        "7",
        "--no-learn",
    ]);
    assert!(ok, "ask упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("L5 Dialog"), "{stdout}");

    let (ok, stdout, stderr) = run(&[
        "ask",
        "что такое квант?",
        "--brain",
        brain.to_str().unwrap(),
        "--max-tokens",
        "8",
        "--seed",
        "7",
        "--no-learn",
        "--no-bridge",
    ]);
    assert!(ok, "ask --no-bridge упал:\n{stdout}\n{stderr}");
    assert!(stdout.contains("L5 Dialog"), "{stdout}");
}
