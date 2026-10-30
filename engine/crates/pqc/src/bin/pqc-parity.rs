#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! CLI `pqc-parity`: экспорт конфигураций RQ4 для кросс-теста с Qiskit.
//!
//! Собирает анзац паритета (зеркало `poler_quantum.quantum.ansatz`),
//! вычисляет точный statevector, выполняет Born-выстрелы и сериализует
//! всё в JSON — протокол `pqc-parity/1` для Python-раннера.
//!
//! ```text
//! pqc-parity --seed 42 --n 4 --mode A --ent cx --shots 100000
//!            [--gamma 0.5] [--kappa 0.8] [--p-csv "0.1,-0.7"]
//!            [--out file.json | --stdout]
//! ```
//!
//! Фазы по умолчанию — языко-агностичный поток SplitMix64
//! (`p_k = (x_k >> 11)/2^52 − 1`), воспроизводимый в Python бит-в-бит;
//! `--p-csv` задаёт явный вектор (зонды конвенции битов).

use std::process::exit;

use pqc::json::Json;
use pqc::parity::{config_rng_seed, splitmix_phases, ParityAnsatz, ParityMode};
use pqc::{Entangler, Rng};

const USAGE: &str = "\
pqc-parity — экспорт конфигураций кросс-теста паритета с Qiskit

USAGE:
    pqc-parity --seed <S> --n <Q> --mode <A|B|C> --ent <cx|cz> --shots <N>
               [--gamma <F>] [--kappa <F>] [--p-csv <csv>]
               [--out <file.json> | --stdout]

OPTIONS:
    --seed <S>     сид SplitMix64-потока фаз (и ГПСЧ выстрелов)
    --n <Q>        число кубитов (1..=26)
    --mode <M>     режим стабилизации: A | B | C
    --ent <E>      запутыватель резонансного слоя: cx | cz
    --shots <N>    Born-выстрелов
    --gamma <F>    фаза стабилизации mode B/C (default 0.5)
    --kappa <F>    адаптивность mode C (default 0.8)
    --p-csv <csv>  явные фазы через запятую (зонды BE/LE)
    --out <file>   записать JSON в файл
    --stdout       вывести JSON в stdout";

struct Args {
    seed: u64,
    n: usize,
    mode: ParityMode,
    ent: Entangler,
    shots: u64,
    gamma: f64,
    kappa: f64,
    p_csv: Option<Vec<f64>>,
    out: Option<String>,
    stdout: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut seed: Option<u64> = None;
    let mut n: Option<usize> = None;
    let mut mode: Option<ParityMode> = None;
    let mut ent: Option<Entangler> = None;
    let mut shots: Option<u64> = None;
    let mut gamma = 0.5_f64;
    let mut kappa = 0.8_f64;
    let mut p_csv: Option<Vec<f64>> = None;
    let mut out: Option<String> = None;
    let mut stdout = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let mut val = || {
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("аргумент {a} требует значение"))
        };
        match a {
            "--seed" => seed = Some(val()?.parse().map_err(|e| format!("--seed: {e}"))?),
            "--n" => n = Some(val()?.parse().map_err(|e| format!("--n: {e}"))?),
            "--mode" => {
                mode = Some(
                    ParityMode::from_name(&val()?)
                        .ok_or_else(|| "mode must be A | B | C".to_string())?,
                )
            }
            "--ent" => {
                ent = Some(
                    Entangler::from_name(&val()?)
                        .ok_or_else(|| "ent must be cx | cz".to_string())?,
                )
            }
            "--shots" => shots = Some(val()?.parse().map_err(|e| format!("--shots: {e}"))?),
            "--gamma" => gamma = val()?.parse().map_err(|e| format!("--gamma: {e}"))?,
            "--kappa" => kappa = val()?.parse().map_err(|e| format!("--kappa: {e}"))?,
            "--p-csv" => {
                let csv = val()?;
                p_csv = Some(
                    csv.split(',')
                        .map(|s| s.trim().parse::<f64>())
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| format!("--p-csv: {e}"))?,
                );
            }
            "--out" => out = Some(val()?),
            "--stdout" => stdout = true,
            _ => return Err(format!("неизвестный аргумент: {a}")),
        }
        i += 1;
    }

    Ok(Args {
        seed: seed.ok_or("--seed обязателен")?,
        n: n.ok_or("--n обязателен")?,
        mode: mode.ok_or("--mode обязателен")?,
        ent: ent.ok_or("--ent обязателен")?,
        shots: shots.ok_or("--shots обязателен")?,
        gamma,
        kappa,
        p_csv,
        out,
        stdout,
    })
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{USAGE}\n\nerror: {e}");
            exit(2);
        }
    };

    let p: Vec<f64> = match &args.p_csv {
        Some(explicit) => {
            if explicit.len() != args.n {
                eprintln!(
                    "error: --p-csv содержит {} фаз, ожидается {}",
                    explicit.len(),
                    args.n
                );
                exit(2);
            }
            explicit.clone()
        }
        None => splitmix_phases(args.seed, args.n),
    };

    let ansatz = match ParityAnsatz::new(args.n, args.mode, args.ent) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            exit(2);
        }
    };

    let sv = match ansatz.circuit(&p, args.gamma, args.kappa) {
        Ok(sv) => sv,
        Err(e) => {
            eprintln!("error: {e}");
            exit(2);
        }
    };

    // Born-выстрелы: независимый поток на конфигурацию (как в кросс-тесте).
    let mut rng = Rng::seed_from_u64(config_rng_seed(args.seed, args.n, args.mode, args.ent));
    let counts = match ansatz.sample_counts(&p, args.gamma, args.kappa, &mut rng, args.shots) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            exit(2);
        }
    };

    // Счётчики по возрастанию исхода — детерминизм документа.
    let mut counts = counts;
    counts.sort_by_key(|(k, _)| *k);

    let doc = Json::Obj(vec![
        ("schema".into(), Json::str("pqc-parity/1")),
        ("seed".into(), Json::num(args.seed as f64)),
        ("n".into(), Json::num(args.n as f64)),
        ("mode".into(), Json::str(args.mode.name())),
        ("ent".into(), Json::str(args.ent.name())),
        ("gamma".into(), Json::num(args.gamma)),
        ("kappa".into(), Json::num(args.kappa)),
        ("shots".into(), Json::num(args.shots as f64)),
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
    ]);

    let text = doc.to_string();
    match (&args.out, args.stdout) {
        (Some(path), _) => {
            if let Err(e) = std::fs::write(path, &text) {
                eprintln!("error: не удалось записать {path}: {e}");
                exit(1);
            }
            println!("written: {path} ({} байт)", text.len());
        }
        (None, true) => println!("{text}"),
        (None, false) => {
            eprintln!("{USAGE}\n\nerror: укажите --out <file> или --stdout");
            exit(2);
        }
    }
}
