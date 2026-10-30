//! Химический модуль v0.75.0: лестница «атом → молекула → белок → (клетка)».
//!
//! * [`smiles`] — OpenSMILES-парсер: молекулярный граф, SSSR, дескрипторы,
//!   оценка logP.
//! * [`geom3d`] — 3D-укладчик конформеров: гибридизация sp/sp²/sp³,
//!   кольцевые шаблоны, детерминированная релаксация.
//! * [`pharma`] — фармакология: дескрипторы, заряды Гастайгера,
//!   энергия связывания лиганд⇄мишень в LogProb-домене.
//! * [`pdb`] — PDB/mmCIF-макромолекулы: парсер, остатки, вторичная
//!   структура, united-заряды, реконструкция H-доноров, карманы
//!   (лигандный + слепой grid-burial), 27-дерево рецептора.
//! * [`dock`] — гибкий молекулярный докинг: торсионное дерево,
//!   скоринг (LJ + кулон + направленные H-связи + липофильная рампа +
//!   десольватация), MC/SA с трит-квантованными кручениями, ансамбль
//!   в лог-домене, redocking-RMSD.
//! * [`sdf`] — SDF/MOL V2000-библиотеки лигандов: парсер с поглощением
//!   H (координаты сохраняются), писатель, `.smi` (контур B).
//! * [`screen`] — каскадный ультра-скрининг библиотек: пре-фильтры →
//!   быстрый MC (JIT-ядро) → полный докинг; rayon + детерминизм (B).
//! * [`periodic`] + [`formula`] + [`stoich`] — фундамент точной химии:
//!   таблица Менделеева (IUPAC 2021), парсер формул (гидраты),
//!   балансировщик уравнений (рациональный Гаусс) (контур D).
//! * [`curriculum`] + [`curriculum_nl`] — задачник 5–9 класса как
//!   бенчмарк Curriculum Learning: 187 задач с ответами независимой
//!   Python-реализации + разбор русских формулировок (SciVoice) (D).
//! * [`view`] — интерактивные 3D/4D-визоры (орбитальная камера, CPK,
//!   шаростержневой / Ван-дер-Ваальс / каркас, кручение связей;
//!   комплексы белок-лиганд: CA-трейс / карман / ВдВ-поверхность).
//!
//! Двигатель универсален: здесь — настоящая химия Земли; Eteria — лишь
//! один из выводимых миров.

pub mod geom3d;
pub mod pharma;
pub mod pdb;
pub mod dock;
pub mod sdf;
pub mod rotamer;
pub mod induced_fit;
pub mod screen;
pub mod smiles;
pub mod view;
pub mod periodic;
pub mod formula;
pub mod stoich;
pub mod curriculum;
pub mod curriculum_nl;

pub use formula::parse_formula;
pub use stoich::{parse_equation, Equation};
pub use curriculum::{report_text, run_grade, solve, tasks};

pub use geom3d::{conformer_from_positions, embed, hybridization, Conformer, Hybrid};
pub use pharma::{binding_report, binding_text, gasteiger, BindingResult, GasteigerCharges};
pub use smiles::{
    descriptors, logp_estimate, parse_smiles, BondOrder, Descriptors, MoleculeGraph,
};

// ─── Словарь известных молекул (имя → SMILES) ───────────────────────────

/// Известные молекулы: лекарства, нейромедиаторы, растворители.
/// SMILES — канонические записи PubChem.
pub fn known_molecule_smiles(name: &str) -> Option<&'static str> {
    let key = name.trim().to_lowercase();
    Some(match key.as_str() {
        "вода" | "water" => "O",
        "метан" | "methane" => "C",
        "этанол" | "ethanol" | "спирт" => "CCO",
        "метанол" | "methanol" => "CO",
        "ацетон" | "acetone" => "CC(=O)C",
        "бензол" | "benzene" => "c1ccccc1",
        "толуол" | "toluene" => "Cc1ccccc1",
        "фенол" | "phenol" => "Oc1ccccc1",
        "пиридин" | "pyridine" => "c1ccncc1",
        "пиррол" | "pyrrole" => "c1cc[nH]c1",
        "кофеин" | "caffeine" => "CN1C=NC2=C1C(=O)N(C)C(=O)N2C",
        "аспирин" | "aspirin" | "ацетилсалициловая кислота" => "CC(=O)Oc1ccccc1C(=O)O",
        "парацетамол" | "paracetamol" | "acetaminophen" => "CC(=O)Nc1ccc(O)cc1",
        "дофамин" | "dopamine" => "NCCc1ccc(O)c(O)c1",
        "серотонин" | "serotonin" => "NCCc1c[nH]c2ccc(O)cc12",
        "никотин" | "nicotine" => "CN1CCCC1c2cccnc2",
        "ибупрофен" | "ibuprofen" => "CC(C)Cc1ccc(cc1)C(C)C(=O)O",
        "глицин" | "glycine" => "NCC(=O)O",
        "аланин" | "alanine" => "C[C@@H](N)C(=O)O",
        "аденин" | "adenine" => "NC1=NC=NC2=C1N=CN2",
        "глюкоза" | "glucose" => "OC1COC(CO)C(O)C1O",
        _ => return None,
    })
}

/// Разрешить вход calc/CLI: имя молекулы или SMILES.
pub fn resolve_input(input: &str) -> Result<MoleculeGraph, String> {
    let t = input.trim();
    if t.is_empty() {
        return Err("пустой ввод: ожидается SMILES или имя молекулы".into());
    }
    if let Some(smi) = known_molecule_smiles(t) {
        return parse_smiles(smi);
    }
    match parse_smiles(t) {
        Ok(g) => Ok(g),
        Err(e) => Err(format!(
            "«{t}» не распознан ни как имя ({}) ни как SMILES: {e}",
            "вода, кофеин, аспирин, дофамин…"
        )),
    }
}
