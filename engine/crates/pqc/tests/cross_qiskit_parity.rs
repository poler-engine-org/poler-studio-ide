#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Кросс-тест RQ4: χ²-паритет Rust (`pqc`) ↔ Python/Qiskit
//! (`poler_quantum.quantum.ansatz.PolerAnsatz`).
//!
//! Матрица: сиды {42, 1337, 2026} × n ∈ {2, 4, 8} × режимы {A, B, C} ×
//! запутыватели {cx, cz} = 54 конфигурации по 100 000 Born-выстрелов,
//! плюс два зонда конвенции битов (асимметричные p). Rust экспортирует
//! амплитуды и счётчики в JSON (`pqc-parity/1`), Python-раннер
//! (`tests/qiskit_parity_runner.py`) строит те же схемы в Qiskit и
//! проверяет:
//!
//! * совпадение statevector-амплитуд до ε < 1e−12;
//! * совпадение argmax |амплитуд| (явная проверка little-endian);
//! * воспроизводимость контракта фаз SplitMix64;
//! * χ²-критерий Пирсона: агрегат каждого режима (Σχ² ~ χ²(Σdf),
//!   конфигурации независимы благодаря `config_rng_seed`) с поправкой
//!   Бонферрони на 3 режима — p > 0.05/3, семейный уровень значимости
//!   5% (статистически корректная форма требования «паритет на всех
//!   базовых модах с p > 0.05»; контроль numpy.multinomial: идеальный
//!   сэмплер даёт ~5% конфигураций ниже 0.05). Отдельные конфигурации
//!   проходят жёсткий пол p > 1e-4.
//!
//! Требует `python3` с `qiskit`, `scipy` и `poler_quantum`; при их
//! отсутствии тест пропускается с предупреждением (строгий режим —
//! переменная окружения `PQC_PARITY_STRICT=1`).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use pqc::json::Json;
use pqc::parity::{config_rng_seed, splitmix_phases, ParityAnsatz, ParityMode};
use pqc::{Entangler, Rng};

const SEEDS: [u64; 3] = [42, 1337, 2026];
const NS: [usize; 3] = [2, 4, 8];
const SHOTS: u64 = 100_000;
const GAMMA: f64 = 0.5;
const KAPPA: f64 = 0.8;
const AMP_EPS: f64 = 1e-12;
/// Семейный уровень 5% с поправкой Бонферрони на 3 режима A/B/C.
const MODE_ALPHA: f64 = 0.05 / 3.0;
/// Пол отдельной конфигурации (сэмплировщик корректен ⇔ p-value
/// равномерны — отдельные значения ниже 0.05 закономерны; контроль
/// numpy.multinomial даёт те же доли).
const CONFIG_FLOOR: f64 = 1e-4;

fn runner_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/qiskit_parity_runner.py")
}

fn python_available() -> bool {
    Command::new("python3")
        .args(["-c", "import qiskit, scipy, poler_quantum"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// JSON-документ протокола pqc-parity/1 для одной конфигурации.
fn export_document(
    seed: u64,
    n: usize,
    mode: ParityMode,
    ent: Entangler,
    shots: u64,
    p: &[f64],
    p_source: &str,
) -> Json {
    let ansatz = ParityAnsatz::new(n, mode, ent).unwrap();
    let sv = ansatz.circuit(p, GAMMA, KAPPA).unwrap();
    // Независимый поток на конфигурацию: режимы A/B/C не делят выборку.
    let mut rng = Rng::seed_from_u64(config_rng_seed(seed, n, mode, ent));
    let mut counts = ansatz
        .sample_counts(p, GAMMA, KAPPA, &mut rng, shots)
        .unwrap();
    counts.sort_by_key(|(k, _)| *k);

    Json::Obj(vec![
        ("schema".into(), Json::str("pqc-parity/1")),
        ("p_source".into(), Json::str(p_source)),
        ("seed".into(), Json::num(seed as f64)),
        ("n".into(), Json::num(n as f64)),
        ("mode".into(), Json::str(mode.name())),
        ("ent".into(), Json::str(ent.name())),
        ("gamma".into(), Json::num(GAMMA)),
        ("kappa".into(), Json::num(KAPPA)),
        ("shots".into(), Json::num(shots as f64)),
        ("p".into(), Json::num_arr(p.iter().copied())),
        (
            "amplitudes_re".into(),
            Json::num_arr(sv.amplitudes().iter().map(|a| a.re)),
        ),
        (
            "amplitudes_im".into(),
            Json::num_arr(sv.amplitudes().iter().map(|a| a.im)),
        ),
        (
            "counts".into(),
            Json::Obj(
                counts
                    .iter()
                    .map(|(k, c)| (k.to_string(), Json::num(*c as f64)))
                    .collect(),
            ),
        ),
    ])
}

#[test]
fn cross_qiskit_parity_chi2_and_amplitudes() {
    let have_python = python_available();
    if !have_python {
        if std::env::var("PQC_PARITY_STRICT").is_ok() {
            panic!("PQC_PARITY_STRICT=1, но python3/qiskit/scipy/poler_quantum недоступны");
        }
        eprintln!(
            "SKIP: python3 с qiskit/scipy/poler_quantum недоступен — \
             кросс-тест паритета не запущен (PQC_PARITY_STRICT=1 форсирует ошибку)"
        );
        return;
    }

    let dir = std::env::temp_dir().join(format!("pqc-parity-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("создать временный каталог");

    // 54 конфигурации: сид × n × режим × запутыватель.
    let mut files = Vec::new();
    for &seed in &SEEDS {
        for &n in &NS {
            for mode in [ParityMode::A, ParityMode::B, ParityMode::C] {
                for ent in [Entangler::Cx, Entangler::Cz] {
                    let p = splitmix_phases(seed, n);
                    let doc = export_document(seed, n, mode, ent, SHOTS, &p, "splitmix");
                    let name = format!("{seed}-{n}-{}-{}.json", mode.name(), ent.name());
                    let path = dir.join(&name);
                    fs::write(&path, doc.to_string()).expect("записать конфигурацию");
                    files.push(name);
                }
            }
        }
    }

    // Зонды конвенции битов: асимметричные p дают argmax, чувствительный
    // к BE/LE swap (2 и 3 кубита, разные запутыватели).
    let probes: [(u64, usize, ParityMode, Entangler, &[f64], &str); 2] = [
        (
            0u64,
            2,
            ParityMode::A,
            Entangler::Cx,
            &[1.0, -1.0],
            "probe-le-2-cx.json",
        ),
        (
            0u64,
            3,
            ParityMode::A,
            Entangler::Cz,
            &[-1.0, 1.0, 1.0],
            "probe-le-3-cz.json",
        ),
    ];
    for &(seed, n, mode, ent, p, name) in &probes {
        let doc = export_document(seed, n, mode, ent, 1000, p, "explicit");
        fs::write(dir.join(name), doc.to_string()).expect("записать зонд");
        files.push(name.to_string());
    }
    assert_eq!(files.len(), 56);

    // Python-раннер: Qiskit-референс + χ² + вердикт.
    let verdict_path = dir.join("verdict.json");
    let out = Command::new("python3")
        .arg(runner_path())
        .arg("--indir")
        .arg(&dir)
        .arg("--out")
        .arg(&verdict_path)
        .output()
        .expect("запустить Python-раннер");

    if !out.status.success() {
        panic!(
            "Python-раннер упал ({}):\n--- stdout ---\n{}\n--- stderr ---\n{}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
    println!("{}", String::from_utf8_lossy(&out.stdout));

    let verdict_text = fs::read_to_string(&verdict_path).expect("прочитать вердикт");
    let verdict = Json::parse(&verdict_text).expect("разобрать вердикт JSON");

    assert_eq!(
        verdict.get("pass").and_then(Json::as_bool),
        Some(true),
        "вердикт не прошёл:\n{verdict_text}"
    );
    assert_eq!(verdict.get("n_configs").and_then(Json::as_f64), Some(56.0));
    let max_amp = verdict
        .get("max_amp_diff")
        .and_then(Json::as_f64)
        .expect("max_amp_diff");
    assert!(
        max_amp < AMP_EPS,
        "максимальное расхождение амплитуд {max_amp:e} >= {AMP_EPS:e}"
    );
    let min_p = verdict
        .get("min_p_value")
        .and_then(Json::as_f64)
        .expect("min_p_value");
    assert!(
        min_p > CONFIG_FLOOR,
        "минимальный p-value {min_p} <= пол {CONFIG_FLOOR}: грубая ошибка сэмплирования"
    );

    // Агрегат по режимам: Σχ² ~ χ²(Σdf), p > 0.05 — критерий DoD.
    let modes = verdict.get("modes").and_then(Json::as_arr).expect("modes");
    assert_eq!(modes.len(), 3, "режимы A/B/C");
    for m in modes {
        let name = m.get("mode").and_then(Json::as_str).unwrap_or("?");
        let p = m.get("p_value").and_then(Json::as_f64).unwrap_or(0.0);
        assert!(
            p > MODE_ALPHA,
            "агрегированный p-value режима {name} = {p} <= alpha {MODE_ALPHA} (Бонферрони)"
        );
        assert_eq!(m.get("pass").and_then(Json::as_bool), Some(true));
    }

    // Каждая конфигурация: амплитуды, конвенция битов, пол χ².
    let configs = verdict
        .get("configs")
        .and_then(Json::as_arr)
        .expect("configs");
    assert_eq!(configs.len(), 56);
    for cfg in configs {
        let name = cfg.get("file").and_then(Json::as_str).unwrap_or("?");
        assert_eq!(
            cfg.get("pass").and_then(Json::as_bool),
            Some(true),
            "конфигурация {name} не прошла"
        );
        assert_eq!(
            cfg.get("bit_order_ok").and_then(Json::as_bool),
            Some(true),
            "конвенция битов нарушена в {name}"
        );
        assert_eq!(
            cfg.get("contract_ok").and_then(Json::as_bool),
            Some(true),
            "контракт SplitMix64 нарушен в {name}"
        );
        assert_eq!(
            cfg.get("amp_ok").and_then(Json::as_bool),
            Some(true),
            "амплитуды разошлись в {name}"
        );
        assert!(
            cfg.get("p_value").and_then(Json::as_f64).unwrap_or(0.0) > CONFIG_FLOOR,
            "p-value ниже пола в {name}"
        );
    }

    // Санити референса: сам Qiskit little-endian.
    let st = verdict.get("qiskit_self_test").expect("self-test");
    assert_eq!(st.get("ok").and_then(Json::as_bool), Some(true));

    // Уборка временного каталога.
    let _ = fs::remove_dir_all(&dir);
}

/// Смоук бинарника pqc-parity: валидный JSON-документ на stdout.
#[test]
fn pqc_parity_binary_smoke() {
    let exe = env!("CARGO_BIN_EXE_pqc-parity");
    let out = Command::new(exe)
        .args([
            "--seed", "42", "--n", "4", "--mode", "B", "--ent", "cz", "--shots", "1000", "--stdout",
        ])
        .output()
        .expect("запустить pqc-parity");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let doc = Json::parse(&text).expect("бинарник вывел невалидный JSON");
    assert_eq!(
        doc.get("schema").and_then(Json::as_str),
        Some("pqc-parity/1")
    );
    assert_eq!(doc.get("n").and_then(Json::as_f64), Some(4.0));
    assert_eq!(doc.get("shots").and_then(Json::as_f64), Some(1000.0));
    assert_eq!(
        doc.get("p").and_then(Json::as_arr).map(<[Json]>::len),
        Some(4)
    );
    assert_eq!(
        doc.get("amplitudes_re")
            .and_then(Json::as_arr)
            .map(<[Json]>::len),
        Some(16)
    );
    // Сумма счётчиков = выстрелы (counts — JSON-объект).
    match doc.get("counts") {
        Some(Json::Obj(pairs)) => {
            let sum: f64 = pairs.iter().filter_map(|(_, v)| v.as_f64()).sum();
            assert_eq!(sum, 1000.0);
        }
        _ => panic!("counts должен быть JSON-объектом"),
    }
}
