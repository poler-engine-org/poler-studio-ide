#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Генератор мини-библиотеки скрининга (контур B5, v0.77.0).
//!
//! Состав: бензамидин — известный актив трипсина (кристаллический лиганд
//! 3PTB, остаток BAM) + 29 нейтральных декоев близкой массы/формулы без
//! основных групп (амидины/гуанидины/первичные амины отсутствуют —
//! S1-карман трипсина предпочитает катионные группы, декои их не имеют).
//!
//! Конформеры строит сам движок (`geom3d::embed`) и пишет SDF V2000
//! собственным писателем (`sdf::mol_block`) — фикстура самодостаточна,
//! воспроизводима и заодно тестирует round-trip писателя на 30 молекулах.
//!
//! Запуск: `cargo run --release --example sdf_minilib`
//! Выход: `tests/fixtures/mini_lib.sdf` (30 записей, `$$$$`-разделители).

use poler_engine::chem::geom3d::embed;
use poler_engine::chem::sdf::sdf_text;
use poler_engine::chem::smiles::parse_smiles;

/// Библиотека: (имя, SMILES). Первый — актив, остальные — декои.
///
/// Актив — ПРОТОНИРОВАННЫЙ бензамидиний (физиологическая форма при pH 7:
/// pKa амидина ≈ 11.5, катион 100%; именно солевой мосток N+···O−
/// Asp189 доминирует в экспериментальном связывании). Нейтральная
/// форма — артефакт канонических SMILES PubChem, её заряды Гастайгера
/// (N−) ДАЖЕ отталкиваются от Asp189 — честно задокументировано.
const LIB: &[(&str, &str)] = &[
    // ── актив ──
    ("benzamidine", "NC(=[NH2+])c1ccccc1"), // катион — лиганд 3PTB (BAM), ингибитор трипсина
    // ── декои: нейтральные, близкая масса, без основных групп ──
    ("toluene", "Cc1ccccc1"),
    ("phenol", "Oc1ccccc1"),
    ("aniline", "Nc1ccccc1"),
    ("anisole", "COc1ccccc1"),
    ("benzaldehyde", "O=Cc1ccccc1"),
    ("acetophenone", "CC(=O)c1ccccc1"),
    ("benzonitrile", "N#Cc1ccccc1"),
    ("nitrobenzene", "[O-][N+](=O)c1ccccc1"),
    ("benzyl-alcohol", "OCc1ccccc1"),
    ("phenethyl-alcohol", "OCCc1ccccc1"),
    ("benzamide", "NC(=O)c1ccccc1"),
    ("methyl-benzoate", "COC(=O)c1ccccc1"),
    ("chlorobenzene", "Clc1ccccc1"),
    ("fluorobenzene", "Fc1ccccc1"),
    ("bromobenzene", "Brc1ccccc1"),
    ("iodobenzene", "Ic1ccccc1"),
    ("o-xylene", "Cc1ccccc1C"),
    ("mesitylene", "Cc1c(C)ccc(C)c1C"),
    ("ethylbenzene", "CCc1ccccc1"),
    ("styrene", "C=Cc1ccccc1"),
    ("allylbenzene", "C=CCc1ccccc1"),
    ("naphthalene", "c1ccc2ccccc2c1"),
    ("veratrole", "COc1ccccc1OC"),
    ("resorcinol", "Oc1cccc(O)c1"),
    ("hydroquinone", "Oc1ccc(O)cc1"),
    ("diphenyl-ether", "O(c1ccccc1)c1ccccc1"),
    ("benzophenone", "O=C(c1ccccc1)c1ccccc1"),
    ("diphenylmethane", "C(c1ccccc1)c1ccccc1"),
    ("biphenyl", "c1ccc(-c2ccccc2)cc1"),
];

fn main() -> std::process::ExitCode {
    let mut records = Vec::with_capacity(LIB.len());
    for (name, smi) in LIB {
        match parse_smiles(smi).and_then(|g| embed(&g).map(|conf| (g, conf))) {
            Ok((g, conf)) => {
                eprintln!("ok: {name} — {}", g.hill_formula());
                records.push(((*name).to_string(), g, conf));
            }
            Err(e) => {
                eprintln!("«{name}» ({smi}): {e}");
                return std::process::ExitCode::FAILURE;
            }
        }
    }
    let text = sdf_text(&records);
    let out = "tests/fixtures/mini_lib.sdf";
    if let Err(e) = std::fs::write(out, &text) {
        eprintln!("не записать {out}: {e}");
        return std::process::ExitCode::FAILURE;
    }
    eprintln!(
        "записано: {out} ({} записей, {} байт)",
        records.len(),
        text.len()
    );
    std::process::ExitCode::SUCCESS
}
