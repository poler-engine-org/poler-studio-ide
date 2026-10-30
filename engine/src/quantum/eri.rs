//! # Ab initio мост POLER-Engine ↔ POLER-ERI (RHF/STO-3G)
//!
//! «Вот это пригодится для химии» — Drive-архив «POLER_Quantum_Chemistry_
//! MetaCompiler» содержал полноценное квантово-химическое ядро (RHF, ERI,
//! DIIS, Jacobi). Аудит v0.73.0 нашёл в нём шесть реальных математических
//! дефектов (моменты Райса, индекс β в Чебышёве, коэффициенты c00/c0p,
//! знаки HRR, заполнители-плейсхолдеры в одноэлектронных интегралах p-оболочек,
//! паника канонизации квартетов) — все устранены и проверены:
//!
//! * E(H₂/STO-3G, R=1.4) = −1.116714 Ha (PySCF: −1.116714)
//! * E(He/STO-3G)        = −2.807784 Ha (PySCF: −2.807784)
//! * E(H/STO-3G)         = −0.494907 Ha
//!
//! ## v0.74.0 — «медицинская находка» + второй период B–Ne
//!
//! Сверка с независимым PySCF 2.14 вскрыла СЕДЬМОЙ дефект, который аудит
//! v0.73.0 пропустил и залочил тестом: `nuclear_cartesian` использовал
//! наивное тождество `(x−P)e = (1/(2p))∂e` в k-й степени, теряя product-rule
//! члены со второго порядка. V(p,p) выходил ПОЛОЖИТЕЛЬНЫМ: H2O = −44.09 Ha
//! (истина −74.96), LiH = −7.810054 (истина −7.862246). Фикс — корректное
//! McMurchie–Davidson разложение (E-коэффициенты, см. one_electron.rs).
//! После фикса базис расширен до B–Ne (Basis Set Exchange, версия 1):
//!
//! * E(H₂O) = −74.962928 · E(NH₃) = −55.426271 · E(CH₄) = −39.726810
//! * E(HF)  = −98.570758 · E(CO)  = −111.224559 — все до 1e-6 Ha против PySCF
//!
//! Философия прежняя: движок универсален. Eteria — выводимый клиент;
//! здесь — настоящая химия Земли: Хартри–Фок на контрактных гауссовых
//! базисах, «голос учёного» говорит аб-иницио числами.
//!
//! Пример: `calc hf("H2", 1.4)` → полный RHF-отчёт в Хартри.

use poler_eri::cart::ncart;
use poler_eri::engine::IntegralEngine;
use poler_eri::molecule::Molecule;
use poler_eri::normalization::{normalize_contraction, normalize_shell};
use poler_eri::scf::rhf_scf;
use poler_eri::shell_driver::eri_shell_full;
use poler_eri::types::{Atom, BasisSet, Point, QuartetData, Shell};

/// Результат расчёта Хартри–Фока.
#[derive(Debug, Clone)]
pub struct HfResult {
    /// Название/формула молекулы.
    pub molecule: String,
    /// Геометрия (в Бора) — по-атомно.
    pub geometry: Vec<(String, [f64; 3])>,
    /// Число базисных функций (картезовых).
    pub n_basis: usize,
    /// Число электронов.
    pub n_electrons: usize,
    /// Электронная энергия + межъядерное отталкивание, Хартри.
    pub energy_hartree: f64,
    /// Энергия межъядерного отталкивания, Хартри.
    pub e_nuc: f64,
    /// Сошёлся ли SCF.
    pub converged: bool,
    /// Число SCF-итераций.
    pub iterations: usize,
    /// Энергии занятых орбиталей, Хартри.
    pub occupied_orbital_energies: Vec<f64>,
    /// Метод (для отчёта).
    pub method: &'static str,
}

/// STO-3G-оболочка: экспоненты и коэффициенты контрактации.
struct Sto3gShell {
    l: usize,
    alphas: Vec<f64>,
    coeffs: Vec<f64>,
}

/// Стандартный STO-3G (Hehre–Stewart–Pople 1969; данные Basis Set Exchange,
/// Version 1 / Gaussian09 — сверено с BSE в v0.74.0, включая Li SP).
/// Проверено: H → −0.494907, He → −2.807844, LiH → −7.810054.
fn sto3g(element: &str) -> Vec<Sto3gShell> {
    match poler_eri::basis::sto3g_element(element) {
        Some(raw) => raw
            .into_iter()
            .map(|r| Sto3gShell { l: r.l, alphas: r.alphas, coeffs: r.coeffs })
            .collect(),
        None => Vec::new(),
    }
}

fn charge_of(element: &str) -> f64 {
    poler_eri::basis::element_number(element) as f64
}

/// Спецификация молекулы: (элемент, координаты Бора).
type AtomSpec = (&'static str, [f64; 3]);

/// Разбор формулы HF-калькулятора: "H2", "H2@1.4", "He", "LiH", "LiH@3.0",
/// "HeH+", "HeH+@1.5", "H2O", "NH3", "CH4", "HF", "CO".
/// Длина связи — в Бора (только для двухатомных).
pub fn parse_hf_spec(spec: &str) -> Result<(String, f64), String> {
    let s = spec.trim();
    let (name, r) = if let Some(pos) = s.find('@') {
        let r: f64 = s[pos + 1..]
            .trim()
            .parse()
            .map_err(|e| format!("не читается длина связи «{}»: {e}", &s[pos + 1..]))?;
        if !(0.2..=20.0).contains(&r) {
            return Err(format!("длина связи {r} Бора вне разумного диапазона 0.2..20"));
        }
        (&s[..pos], r)
    } else {
        (s, f64::NAN)
    };
    let known = ["H2", "He", "LiH", "HeH+", "H2O", "NH3", "CH4", "HF", "CO"];
    let name = name.replace([' ', '-'], "");
    if known.contains(&name.as_str()) {
        Ok((name, r))
    } else {
        Err(format!(
            "молекула «{name}» не в STO-3G-библиотеке аб-иницио (доступны: H2, He, LiH, HeH+, H2O, NH3, CH4, HF, CO — v0.74.0: второй период B–Ne)"
        ))
    }
}

fn default_bond(name: &str) -> f64 {
    match name {
        "H2" => 1.4,      // учебник Сабо-Остлунда
        "LiH" => 3.0,     // классическая тестовая геометрия
        "HeH+" => 1.4632, // Сабо-Остлунд
        _ => 0.0,
    }
}

/// Бора → для геометрий: 1 Å = 1.8897259886 Бора.
const BOHR_PER_ANG: f64 = 1.8897259886;

/// Экспериментальные равновесные геометрии многотомных молекул (Å).
/// r и углы — справочные (NIST CCCBDB experimental).
fn build_atoms(name: &str, r: f64) -> Result<(Vec<AtomSpec>, i32, usize), String> {
    // (атомы, заряд, мультиплетность)
    match name {
        "H2" => {
            let r = if r.is_nan() { default_bond("H2") } else { r };
            Ok((
                vec![("H", [-r / 2.0, 0.0, 0.0]), ("H", [r / 2.0, 0.0, 0.0])],
                0,
                1,
            ))
        }
        "He" => Ok((vec![("He", [0.0, 0.0, 0.0])], 0, 1)),
        "LiH" => {
            let r = if r.is_nan() { default_bond("LiH") } else { r };
            Ok((vec![("Li", [0.0, 0.0, 0.0]), ("H", [r, 0.0, 0.0])], 0, 1))
        }
        "HeH+" => {
            let r = if r.is_nan() { default_bond("HeH+") } else { r };
            Ok((
                vec![("He", [0.0, 0.0, 0.0]), ("H", [r, 0.0, 0.0])],
                1,
                1,
            ))
        }
        // Вода: C2v, r(OH) = 0.9572 Å, θ(HOH) = 104.52° (эксперимент).
        // Биссектриса вдоль +y, плоскость xy.
        "H2O" => {
            let r_oh = 0.9572 * BOHR_PER_ANG;
            let half = 0.5 * 104.52_f64.to_radians();
            let (s, c) = (half.sin(), half.cos());
            Ok((
                vec![
                    ("O", [0.0, 0.0, 0.0]),
                    ("H", [r_oh * s, r_oh * c, 0.0]),
                    ("H", [-r_oh * s, r_oh * c, 0.0]),
                ],
                0,
                1,
            ))
        }
        // Аммиак: C3v, r(NH) = 1.0116 Å, θ(HNH) = 106.7°.
        // Ось C3 вдоль +z; H на конусе с полярным углом θ/2 от оси.
        "NH3" => {
            let r_nh = 1.0116 * BOHR_PER_ANG;
            let half = 0.5 * 106.7_f64.to_radians();
            let (s, c) = (half.sin(), half.cos());
            let mut atoms = vec![("N", [0.0, 0.0, 0.0])];
            for k in 0..3 {
                let phi = (k as f64) * std::f64::consts::TAU / 3.0;
                atoms.push(("H", [r_nh * s * phi.cos(), r_nh * s * phi.sin(), r_nh * c]));
            }
            Ok((atoms, 0, 1))
        }
        // Метан: Td, r(CH) = 1.0870 Å (эксперимент), тетраэдрические направления.
        "CH4" => {
            let r_ch = 1.0870 * BOHR_PER_ANG;
            let dirs: [[f64; 3]; 4] = [
                [1.0, 1.0, 1.0],
                [1.0, -1.0, -1.0],
                [-1.0, 1.0, -1.0],
                [-1.0, -1.0, 1.0],
            ];
            let mut atoms = vec![("C", [0.0, 0.0, 0.0])];
            for d in dirs {
                let norm = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                atoms.push((
                    "H",
                    [r_ch * d[0] / norm, r_ch * d[1] / norm, r_ch * d[2] / norm],
                ));
            }
            Ok((atoms, 0, 1))
        }
        // Фтороводород: r(HF) = 0.9168 Å (эксперимент), ось x.
        "HF" => {
            let r_hf = if r.is_nan() { 0.9168 * BOHR_PER_ANG } else { r };
            Ok((vec![("F", [0.0, 0.0, 0.0]), ("H", [r_hf, 0.0, 0.0])], 0, 1))
        }
        // Монооксид углерода: r(CO) = 1.128 Å (эксперимент), ось x.
        "CO" => {
            let r_co = if r.is_nan() { 1.128 * BOHR_PER_ANG } else { r };
            Ok((vec![("C", [0.0, 0.0, 0.0]), ("O", [r_co, 0.0, 0.0])], 0, 1))
        }
        other => Err(format!("неизвестная молекула {other}")),
    }
}

/// Полный RHF/STO-3G расчёт по спецификации ("H2", "LiH@3.0", ...).
pub fn hartree_fock(spec: &str) -> Result<HfResult, String> {
    let (name, r) = parse_hf_spec(spec)?;
    let (atom_specs, charge, multiplicity) = build_atoms(&name, r)?;

    // Молекула (для E_nuc и числа электронов)
    let mol = Molecule::new(
        atom_specs
            .iter()
            .map(|(el, xyz)| Atom { charge: charge_of(el), coords: Point(*xyz) })
            .collect(),
        charge as f64,
        multiplicity,
    );
    let n_electrons = mol.n_electrons();
    if n_electrons % 2 != 0 || multiplicity != 1 {
        return Err(format!(
            "RHF требует закрытую оболочку: {name} имеет {n_electrons} e⁻"
        ));
    }
    let n_occ = n_electrons / 2;
    let e_nuc = mol.nuclear_repulsion();

    // Базис: оболочки по атомам
    let mut shells = Vec::new();
    let mut atom_index = 0usize;
    for (el, _) in &atom_specs {
        for sh in sto3g(el) {
            let mut shell = Shell::new(sh.l, atom_index, sh.alphas.clone(), sh.coeffs.clone());
            normalize_shell(&mut shell);
            normalize_contraction(&mut shell);
            shells.push(shell);
        }
        atom_index += 1;
    }
    let basis = BasisSet {
        atoms: atom_specs
            .iter()
            .map(|(el, xyz)| Atom { charge: charge_of(el), coords: Point(*xyz) })
            .collect(),
        shells,
    };
    let engine = IntegralEngine::new(&basis);
    let n: usize = engine.shells.iter().map(|s| ncart(s.l)).sum();

    // Смещения оболочек в глобальной нумерации
    let mut offs = Vec::with_capacity(engine.shells.len());
    let mut acc = 0usize;
    for s in &engine.shells {
        offs.push(acc);
        acc += ncart(s.l);
    }

    let s_mat = engine.overlap_matrix();
    let h_core = engine.core_hamiltonian();

    // Полный контрактный тензор ERI (хим. нотация (μν|λσ), row-major)
    let mut eri = vec![0.0f64; n * n * n * n];
    for (ia, sa) in engine.shells.iter().enumerate() {
        for (ib, sb) in engine.shells.iter().enumerate() {
            for (ic, sc) in engine.shells.iter().enumerate() {
                for (id, sd) in engine.shells.iter().enumerate() {
                    let ra = engine.coords[ia];
                    let rb = engine.coords[ib];
                    let rc = engine.coords[ic];
                    let rd = engine.coords[id];
                    for pa in 0..sa.n_prim() {
                        for pb in 0..sb.n_prim() {
                            for pc in 0..sc.n_prim() {
                                for pd in 0..sd.n_prim() {
                                    let qd = QuartetData::new(
                                        ra, rb, rc, rd,
                                        sa.alphas[pa], sb.alphas[pb],
                                        sc.alphas[pc], sd.alphas[pd],
                                    );
                                    let vals =
                                        eri_shell_full(&qd, sa.l, sb.l, sc.l, sd.l);
                                    let coef = sa.norm_coeffs[pa]
                                        * sb.norm_coeffs[pb]
                                        * sc.norm_coeffs[pc]
                                        * sd.norm_coeffs[pd];
                                    let nca = ncart(sa.l);
                                    let ncb = ncart(sb.l);
                                    let ncc = ncart(sc.l);
                                    let ncd = ncart(sd.l);
                                    let mut idx = 0usize;
                                    for i in 0..nca {
                                        for j in 0..ncb {
                                            for k in 0..ncc {
                                                for l in 0..ncd {
                                                    let mu = offs[ia] + i;
                                                    let nu = offs[ib] + j;
                                                    let lam = offs[ic] + k;
                                                    let sig = offs[id] + l;
                                                    eri[mu * n * n * n
                                                        + nu * n * n
                                                        + lam * n
                                                        + sig] += coef * vals[idx];
                                                    idx += 1;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let res = rhf_scf(&h_core, &s_mat, &eri, n, n_occ, e_nuc, 200, 1e-10, 1e-8);

    // Энергии занятых орбиталей (res хранит полный вектор собственных значений)
    let occ = res
        .orbital_energies
        .iter()
        .take(n_occ)
        .cloned()
        .collect();

    Ok(HfResult {
        molecule: name,
        geometry: atom_specs.iter().map(|(el, xyz)| (el.to_string(), *xyz)).collect(),
        n_basis: n,
        n_electrons,
        energy_hartree: res.energy,
        e_nuc,
        converged: res.converged,
        iterations: res.n_iter,
        occupied_orbital_energies: occ,
        method: "RHF/STO-3G",
    })
}

/// Человекочитаемый отчёт для `calc hf` и «голоса учёного».
pub fn hf_report(res: &HfResult) -> String {
    let ev = 27.211386245988; // Хартри → эВ
    let mut s = String::new();
    s.push_str(&format!(
        "═ч {} · {} ══\n",
        res.method, res.molecule
    ));
    let mut geo = String::new();
    for (i, (el, xyz)) in res.geometry.iter().enumerate() {
        geo.push_str(&format!(
            "{}{el}({:+.4},{:+.4},{:+.4})",
            if i == 0 { "" } else { " " },
            xyz[0],
            xyz[1],
            xyz[2]
        ));
    }
    s.push_str(&format!("геометрия (Бор): {geo}\n"));
    s.push_str(&format!(
        "базис: {} функций · {} электронов · {} занятых МО\n",
        res.n_basis,
        res.n_electrons,
        res.occupied_orbital_energies.len()
    ));
    s.push_str(&format!(
        "E_nuc = {:.6} Ha\n",
        res.e_nuc
    ));
    for (i, eps) in res.occupied_orbital_energies.iter().enumerate() {
        s.push_str(&format!(
            "ε_{} = {:+.6} Ha ({:+.4} эВ)\n",
            i + 1,
            eps,
            eps * ev
        ));
    }
    s.push_str(&format!(
        "E_total = {:.6} Ha = {:+.4} эВ\n",
        res.energy_hartree,
        res.energy_hartree * ev
    ));
    s.push_str(&format!(
        "SCF: {} итераций, {}",
        res.iterations,
        if res.converged { "сошлось" } else { "НЕ сошлось" }
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_h2_textbook() {
        let r = hartree_fock("H2").unwrap();
        assert!((r.energy_hartree - (-1.116714325)).abs() < 1e-6,
            "E(H2) = {}", r.energy_hartree);
        assert!(r.converged);
        assert_eq!(r.n_basis, 2);
    }

    #[test]
    fn test_he_textbook() {
        let r = hartree_fock("He").unwrap();
        assert!((r.energy_hartree - (-2.807783957)).abs() < 1e-6,
            "E(He) = {}", r.energy_hartree);
    }

    /// v0.74.0: LiH сверен с независимым PySCF 2.14 (BSE STO-3G v1, R=3.0 Бора).
    /// До фикса nuclear_cartesian (McMurchie–Davidson) движок давал −7.810054 —
    /// тест v0.73.0 зафиксировал баг: наивная (1/(2p))^k·∂^k-формула теряет
    /// product-rule члены со второго порядка, V(p,p) выходил положительным.
    #[test]
    fn test_lih_runs_and_converges() {
        let r = hartree_fock("LiH").unwrap();
        assert!(r.converged, "LiH SCF must converge");
        assert_eq!(r.n_basis, 6); // 1s,2s,2px,2py,2pz + H1s
        assert_eq!(r.n_electrons, 4);
        assert!((r.energy_hartree - (-7.862246324)).abs() < 1e-6,
            "E(LiH) = {}", r.energy_hartree);
    }

    /// Второй период B–Ne (v0.74.0): геометрии экспериментальные (NIST CCCBDB),
    /// эталоны — независимый RHF/STO-3G PySCF 2.14 на том же BSE-базисе
    /// (scripts/rhf_reference_pyscf.py). Совпадение до 1e-6 Ha.
    #[test]
    fn test_second_period_molecules_vs_pyscf() {
        let reference = [
            ("H2O", -74.962928261),
            ("NH3", -55.426271142),
            ("CH4", -39.726810114),
            ("HF", -98.570757656),
            ("CO", -111.224558690),
        ];
        for (name, ref_e) in reference {
            let r = hartree_fock(name).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(r.converged, "{name}: SCF не сошёлся");
            assert!(
                (r.energy_hartree - ref_e).abs() < 1e-6,
                "E({name}) = {} против PySCF {ref_e}",
                r.energy_hartree
            );
            assert!(r.n_basis >= 6, "{name}: n_basis = {}", r.n_basis);
        }
    }

    #[test]
    fn test_spec_parsing() {
        assert!(parse_hf_spec("H2@1.4").is_ok());
        assert!(parse_hf_spec("Uuo").is_err());
        assert!(parse_hf_spec("H2@100").is_err());
    }

    #[test]
    fn test_hf_report_nonempty() {
        let r = hartree_fock("H2@1.4").unwrap();
        let rep = hf_report(&r);
        assert!(rep.contains("RHF/STO-3G"));
        assert!(rep.contains("Ha"));
    }
}
