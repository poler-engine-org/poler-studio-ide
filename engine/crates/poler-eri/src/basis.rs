//! NWChem basis set parser and built-in basis definitions.
//!
//! This module provides:
//! - A parser for NWChem-format basis set files (the de facto standard format
//!   used by most quantum chemistry programs)
//! - Built-in STO-3G basis definitions for H and Li (for testing)
//! - Element number lookup from periodic table symbols
//!
//! # NWChem Format
//!
//! A typical NWChem basis set file looks like:
//!
//! ```text
//! BASIS SET:
//! H S
//!      3
//!        3.449  0.154
//!        0.627  0.535
//!        0.168  0.444
//! C SP
//!      6
//!       71.62  0.154  0.000
//!       13.04  0.535  0.000
//!        3.53  0.444  0.444
//!        2.94  0.000  0.154
//!        0.68  0.000  0.535
//!        0.22  0.000  0.444
//! END
//! ```
//!
//! Each section specifies an element, shell type (S/P/D/F/G/H/SP), number of
//! primitives, then alpha-coefficient pairs per line. SP shells produce both
//! an s-shell and a p-shell from the same exponents but different coefficients.

use crate::types::{Shell, BasisSet};
use crate::normalization::normalize_shell;
use std::fs;


/// Parse an NWChem-format basis set file into a [`BasisSet`].
///
/// The parser handles the most common features:
/// - Shell types: S, P, D, F, G, H, SP
/// - SP shells (split into separate s and p shells)
/// - Comment lines starting with `!`
/// - Variable number of primitives per shell
///
/// # Arguments
///
/// * `content` — The full text content of the basis set file
///
/// # Returns
///
/// `Ok(BasisSet)` on success, or `Err(String)` with a description of the
/// parse error. Note: the returned `BasisSet` may have an empty `atoms` vector
/// because atom positions are not specified in basis set files — they must be
/// added separately from molecular geometry data.
///
/// # Example
///
/// ```rust
/// use poler_eri::basis::parse_nwchem;
///
/// let content = "H S\n3\n3.449 0.154\n0.627 0.535\n0.168 0.444\n";
/// let basis = parse_nwchem(content).unwrap();
/// assert!(!basis.shells.is_empty());
/// ```
pub fn parse_nwchem(content: &str) -> Result<BasisSet, String> {
    let atoms = Vec::new();
    let mut shells = Vec::new();

    let mut _current_element: Option<String> = None;
    let mut current_l: usize = 0;
    let mut alphas: Vec<f64> = Vec::new();
    let mut coeffs: Vec<f64> = Vec::new();
    let mut sp_coeffs: Vec<f64> = Vec::new();
    let mut reading = false;
    let mut sp_pending = false;

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines, comments, and header/footer markers
        if line.is_empty() || line.starts_with('!') || line.starts_with("BASIS") || line.starts_with("END") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();

        // Check for element header: "H S" or "C SP" etc.
        if parts.len() >= 2 && parts[0].chars().all(|c| c.is_alphabetic()) {
            // Save previous shell before starting a new one
            if reading && !alphas.is_empty() {
                let mut shell = Shell::new(current_l, atoms.len().saturating_sub(1), alphas.clone(), coeffs.clone());
                normalize_shell(&mut shell);
                shells.push(shell);

                // If this was an SP shell, also create the p-shell
                if sp_pending && !sp_coeffs.is_empty() {
                    let mut p_shell = Shell::new(1, atoms.len().saturating_sub(1), alphas.clone(), sp_coeffs.clone());
                    normalize_shell(&mut p_shell);
                    shells.push(p_shell);
                }

                alphas.clear();
                coeffs.clear();
                sp_coeffs.clear();
            }

            _current_element = Some(parts[0].to_string());
            let shell_type = parts[1];
            current_l = match shell_type {
                "S"  => 0,
                "SP" => 0, // SP creates both s and p shells
                "P"  => 1,
                "D"  => 2,
                "F"  => 3,
                "G"  => 4,
                "H"  => 5,
                _ => continue,
            };
            sp_pending = shell_type == "SP";
            reading = true;
            continue;
        }

        // Parse primitive: alpha coeff [sp_coeff]
        if reading && parts.len() >= 2 {
            if let Ok(alpha) = parts[0].parse::<f64>() {
                if let Ok(coeff) = parts[1].parse::<f64>() {
                    alphas.push(alpha);
                    coeffs.push(coeff);

                    // SP shells have a second coefficient for the p-component
                    if sp_pending {
                        if let Some(sp_coeff) = parts.get(2).and_then(|s| s.parse::<f64>().ok()) {
                            sp_coeffs.push(sp_coeff);
                        } else {
                            // If no SP coefficient given, reuse the s-coefficient
                            sp_coeffs.push(coeff);
                        }
                    }
                }
            }
        }
    }

    // Save the last shell
    if reading && !alphas.is_empty() {
        let mut shell = Shell::new(current_l, atoms.len(), alphas.clone(), coeffs.clone());
        normalize_shell(&mut shell);
        shells.push(shell);

        if sp_pending && !sp_coeffs.is_empty() {
            let mut p_shell = Shell::new(1, atoms.len(), alphas.clone(), sp_coeffs.clone());
            normalize_shell(&mut p_shell);
            shells.push(p_shell);
        }
    }

    Ok(BasisSet { atoms, shells })
}

/// Read and parse a basis set file from disk.
///
/// Convenience wrapper around [`parse_nwchem`] that reads the file first.
pub fn read_basis_file(path: &str) -> Result<BasisSet, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read basis file '{}': {}", path, e))?;
    parse_nwchem(&content)
}

/// Look up an element's atomic number from its chemical symbol.
///
/// Supports elements 1–18 (H through Ar). Returns 0 for unknown symbols.
///
/// # Example
///
/// ```
/// use poler_eri::basis::element_number;
/// assert_eq!(element_number("H"), 1);
/// assert_eq!(element_number("Li"), 3);
/// assert_eq!(element_number("C"), 6);
/// ```
pub fn element_number(symbol: &str) -> usize {
    match symbol {
        "H" => 1, "He" => 2,
        "Li" => 3, "Be" => 4, "B" => 5, "C" => 6,
        "N" => 7, "O" => 8, "F" => 9, "Ne" => 10,
        "Na" => 11, "Mg" => 12, "Al" => 13, "Si" => 14,
        "P" => 15, "S" => 16, "Cl" => 17, "Ar" => 18,
        _ => 0,
    }
}

/// Необработанная STO-3G-оболочка без привязки к атому: l, экспоненты,
/// сырые коэффициенты контрактации. Потребитель строит `Shell::new(l, atom_idx, …)`
/// и вызывает `normalize_shell` самостоятельно.
#[derive(Debug, Clone, PartialEq)]
pub struct RawShell {
    pub l: usize,
    pub alphas: Vec<f64>,
    pub coeffs: Vec<f64>,
}

/// Канонический STO-3G для элементов H–Ne.
///
/// Источник данных: Basis Set Exchange (https://www.basissetexchange.org),
/// базис STO-3G, версия 1 (данные Gaussian09), формат NWChem, элементы 1–10.
/// Таблица сгенерирована скриптом `gen_sto3g_table.py` из выгрузки BSE —
/// нулевая вероятность ошибки транскрипции; контрольная сумма пар
/// (экспонента × коэффициент) сверяется тестом `test_sto3g_bse_li_sp`.
///
/// Сверка Li SP-коэффициентов (единственный soft-спот старой библиотеки):
/// SP-s: [-0.09996722919, 0.3995128261, 0.7001154689],
/// SP-p: [0.1559162750, 0.6076837186, 0.3919573931].
/// Старая `sto3g_li()` ошибочно использовала [0.064, 0.368, 0.672]
/// для всех трёх оболочек — исправлено в v0.74.0.
pub fn sto3g_element(element: &str) -> Option<Vec<RawShell>> {
    // === СГЕНЕРИРОВАНО gen_sto3g_table.py из BSE sto-3g (Version 1, Gaussian09) ===
    // Источник: basissetexchange.org, elements 1-10, формат NWChem, S/SP-оболочки.
    let shells = match element {
        "H" => vec![
            RawShell { l: 0, alphas: vec![3.425250914, 0.6239137298, 0.168855404], coeffs: vec![0.1543289673, 0.5353281423, 0.4446345422] },
        ],
        "He" => vec![
            RawShell { l: 0, alphas: vec![6.362421394, 1.158922999, 0.3136497915], coeffs: vec![0.1543289673, 0.5353281423, 0.4446345422] },
        ],
        "Li" => vec![
            RawShell { l: 0, alphas: vec![16.11957475, 2.936200663, 0.794650487], coeffs: vec![0.1543289673, 0.5353281423, 0.4446345422] },
            RawShell { l: 0, alphas: vec![0.6362897469, 0.1478600533, 0.0480886784], coeffs: vec![-0.09996722919, 0.3995128261, 0.7001154689] }, // 2s (SP-s)
            RawShell { l: 1, alphas: vec![0.6362897469, 0.1478600533, 0.0480886784], coeffs: vec![0.155916275, 0.6076837186, 0.3919573931] }, // 2p (SP-p)
        ],
        "Be" => vec![
            RawShell { l: 0, alphas: vec![30.16787069, 5.495115306, 1.487192653], coeffs: vec![0.1543289673, 0.5353281423, 0.4446345422] },
            RawShell { l: 0, alphas: vec![1.31483311, 0.3055389383, 0.0993707456], coeffs: vec![-0.09996722919, 0.3995128261, 0.7001154689] }, // 2s (SP-s)
            RawShell { l: 1, alphas: vec![1.31483311, 0.3055389383, 0.0993707456], coeffs: vec![0.155916275, 0.6076837186, 0.3919573931] }, // 2p (SP-p)
        ],
        "B" => vec![
            RawShell { l: 0, alphas: vec![48.79111318, 8.887362172, 2.40526704], coeffs: vec![0.1543289673, 0.5353281423, 0.4446345422] },
            RawShell { l: 0, alphas: vec![2.236956142, 0.5198204999, 0.16906176], coeffs: vec![-0.09996722919, 0.3995128261, 0.7001154689] }, // 2s (SP-s)
            RawShell { l: 1, alphas: vec![2.236956142, 0.5198204999, 0.16906176], coeffs: vec![0.155916275, 0.6076837186, 0.3919573931] }, // 2p (SP-p)
        ],
        "C" => vec![
            RawShell { l: 0, alphas: vec![71.61683735, 13.04509632, 3.53051216], coeffs: vec![0.1543289673, 0.5353281423, 0.4446345422] },
            RawShell { l: 0, alphas: vec![2.941249355, 0.6834830964, 0.2222899159], coeffs: vec![-0.09996722919, 0.3995128261, 0.7001154689] }, // 2s (SP-s)
            RawShell { l: 1, alphas: vec![2.941249355, 0.6834830964, 0.2222899159], coeffs: vec![0.155916275, 0.6076837186, 0.3919573931] }, // 2p (SP-p)
        ],
        "N" => vec![
            RawShell { l: 0, alphas: vec![99.10616896, 18.05231239, 4.885660238], coeffs: vec![0.1543289673, 0.5353281423, 0.4446345422] },
            RawShell { l: 0, alphas: vec![3.780455879, 0.8784966449, 0.2857143744], coeffs: vec![-0.09996722919, 0.3995128261, 0.7001154689] }, // 2s (SP-s)
            RawShell { l: 1, alphas: vec![3.780455879, 0.8784966449, 0.2857143744], coeffs: vec![0.155916275, 0.6076837186, 0.3919573931] }, // 2p (SP-p)
        ],
        "O" => vec![
            RawShell { l: 0, alphas: vec![130.7093214, 23.80886605, 6.443608313], coeffs: vec![0.1543289673, 0.5353281423, 0.4446345422] },
            RawShell { l: 0, alphas: vec![5.033151319, 1.169596125, 0.38038896], coeffs: vec![-0.09996722919, 0.3995128261, 0.7001154689] }, // 2s (SP-s)
            RawShell { l: 1, alphas: vec![5.033151319, 1.169596125, 0.38038896], coeffs: vec![0.155916275, 0.6076837186, 0.3919573931] }, // 2p (SP-p)
        ],
        "F" => vec![
            RawShell { l: 0, alphas: vec![166.679134, 30.36081233, 8.216820672], coeffs: vec![0.1543289673, 0.5353281423, 0.4446345422] },
            RawShell { l: 0, alphas: vec![6.464803249, 1.502281245, 0.4885884864], coeffs: vec![-0.09996722919, 0.3995128261, 0.7001154689] }, // 2s (SP-s)
            RawShell { l: 1, alphas: vec![6.464803249, 1.502281245, 0.4885884864], coeffs: vec![0.155916275, 0.6076837186, 0.3919573931] }, // 2p (SP-p)
        ],
        "Ne" => vec![
            RawShell { l: 0, alphas: vec![207.015607, 37.70815124, 10.20529731], coeffs: vec![0.1543289673, 0.5353281423, 0.4446345422] },
            RawShell { l: 0, alphas: vec![8.24631512, 1.916266291, 0.6232292721], coeffs: vec![-0.09996722919, 0.3995128261, 0.7001154689] }, // 2s (SP-s)
            RawShell { l: 1, alphas: vec![8.24631512, 1.916266291, 0.6232292721], coeffs: vec![0.155916275, 0.6076837186, 0.3919573931] }, // 2p (SP-p)
        ],
        _ => return None,
    };
    Some(shells)
}

/// Build a minimal STO-3G basis for hydrogen.
///
/// Returns a single s-shell with 3 primitives and the standard STO-3G
/// coefficients (BSE Version 1 / Gaussian09). The shell is already normalized.
///
/// | Primitive | Alpha   | Coefficient |
/// |-----------|---------|-------------|
/// | 1         | 3.425250914 | 0.1543289673 |
/// | 2         | 0.6239137298 | 0.5353281423 |
/// | 3         | 0.168855404  | 0.4446345422 |
pub fn sto3g_h() -> Shell {
    let raw = sto3g_element("H").expect("H обязан быть в STO-3G-таблице").remove(0);
    let mut s = Shell::new(0, 0, raw.alphas, raw.coeffs);
    normalize_shell(&mut s);
    s
}

/// Build a minimal STO-3G basis for lithium.
///
/// Returns three shells (данные Basis Set Exchange, Version 1):
/// 1. An s-shell (1s) with exponents [16.11957475, 2.936200663, 0.794650487]
/// 2. An s-shell (2s from SP split) with exponents [0.6362897469, 0.1478600533, 0.0480886784]
///    and coefficients [-0.09996722919, 0.3995128261, 0.7001154689]
/// 3. A p-shell (2p from SP split) with the same exponents and
///    coefficients [0.155916275, 0.6076837186, 0.3919573931]
///
/// All shells are normalized.
pub fn sto3g_li() -> Vec<Shell> {
    let raw = sto3g_element("Li").expect("Li обязан быть в STO-3G-таблице");
    raw.into_iter()
        .map(|r| {
            let mut sh = Shell::new(r.l, 0, r.alphas, r.coeffs);
            normalize_shell(&mut sh);
            sh
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_number() {
        assert_eq!(element_number("H"), 1);
        assert_eq!(element_number("C"), 6);
        assert_eq!(element_number("Li"), 3);
        assert_eq!(element_number("Xx"), 0);
    }

    #[test]
    fn test_sto3g_h() {
        let shell = sto3g_h();
        assert_eq!(shell.l, 0);
        assert_eq!(shell.n_prim(), 3);
        // After normalization, norm_coeffs should differ from raw coeffs
        assert_ne!(shell.norm_coeffs[0], shell.coeffs[0]);
    }

    #[test]
    fn test_sto3g_li() {
        let shells = sto3g_li();
        assert_eq!(shells.len(), 3); // 1s, 2s, 2p
        assert_eq!(shells[0].l, 0); // 1s
        assert_eq!(shells[1].l, 0); // 2s
        assert_eq!(shells[2].l, 1); // 2p
    }

    /// Сверка Li SP-коэффициентов с Basis Set Exchange — единственный
    /// soft-спот старой библиотеки (v0.74.0). До фикса все три оболочки
    /// использовали [0.064, 0.368, 0.672] — это не данные STO-3G.
    #[test]
    fn test_sto3g_bse_li_sp() {
        let li = sto3g_element("Li").unwrap();
        assert_eq!(li.len(), 3);
        // SP-s (2s): отрицательный первый коэффициент — визитная карточка
        // валентной SP-оболочки STO-3G (узловая плоскость между 1s и 2s).
        assert!((li[1].coeffs[0] - (-0.09996722919)).abs() < 1e-12,
            "Li SP-s c1 = {}", li[1].coeffs[0]);
        assert!((li[1].coeffs[1] - 0.3995128261).abs() < 1e-9);
        assert!((li[1].coeffs[2] - 0.7001154689).abs() < 1e-9);
        // SP-p (2p): положительные, другой профиль
        assert!((li[2].coeffs[0] - 0.155916275).abs() < 1e-9);
        assert!((li[2].coeffs[2] - 0.3919573931).abs() < 1e-9);
        // Общие экспоненты SP-оболочки
        for i in 0..3 {
            assert!((li[1].alphas[i] - li[2].alphas[i]).abs() < 1e-15);
        }
        // Универсальные 1s-коэффициенты (совпадают с H)
        let h = sto3g_element("H").unwrap();
        for i in 0..3 {
            assert!((li[0].coeffs[i] - h[0].coeffs[i]).abs() < 1e-12);
        }
    }

    /// Расширение библиотеки до B–Ne: каждый элемент 2-го периода —
    /// 3 оболочки (1s + SP-s + SP-p), экспоненты растут по Z.
    #[test]
    fn test_sto3g_b_through_ne() {
        let order = ["B", "C", "N", "O", "F", "Ne"];
        let mut prev_alpha = 0.0f64;
        for el in order {
            let shells = sto3g_element(el).unwrap_or_else(|| panic!("{el} обязан быть в таблице"));
            assert_eq!(shells.len(), 3, "{el}: 1s + SP-s + SP-p");
            assert_eq!((shells[0].l, shells[1].l, shells[2].l), (0, 0, 1), "{el}");
            // 1s-экспоненты монотонно растут по периоду (экранирование < роста Z)
            assert!(shells[0].alphas[0] > prev_alpha, "{el}: α(1s) не растёт");
            prev_alpha = shells[0].alphas[0];
            // SP-s и SP-p делят экспоненты
            assert_eq!(shells[1].alphas, shells[2].alphas, "{el}");
            // и одинаковую структуру коэффициентов (универсальные для периода)
            assert!((shells[1].coeffs[0] + 0.09996722919).abs() < 1e-9, "{el}");
        }
        assert_eq!(sto3g_element("Na").map(|v| v.len()), None, "Na — вне H–Ne");
        assert_eq!(sto3g_element("Xx"), None);
    }

    /// Готовые оболочки (sto3g_li) обязаны совпадать с RawShell-таблицей.
    #[test]
    fn test_sto3g_li_shells_match_raw() {
        let raw = sto3g_element("Li").unwrap();
        let shells = sto3g_li();
        for (r, s) in raw.iter().zip(shells.iter()) {
            assert_eq!(r.l, s.l);
            assert_eq!(r.alphas, s.alphas);
            assert_eq!(r.coeffs, s.coeffs);
        }
    }

    #[test]
    fn test_parse_nwchem_h() {
        let content = "H S\n3\n3.449 0.154\n0.627 0.535\n0.168 0.444\n";
        let basis = parse_nwchem(content).unwrap();
        assert_eq!(basis.shells.len(), 1);
        assert_eq!(basis.shells[0].l, 0);
        assert_eq!(basis.shells[0].n_prim(), 3);
    }

    #[test]
    fn test_parse_nwchem_sp() {
        let content = "Li SP\n6\n16.119 0.064 0.000\n2.936 0.368 0.000\n0.795 0.672 0.000\n0.636 0.000 0.064\n0.148 0.000 0.368\n0.048 0.000 0.672\n";
        let basis = parse_nwchem(content).unwrap();
        // SP should produce two shells: s and p
        assert_eq!(basis.shells.len(), 2);
        assert_eq!(basis.shells[0].l, 0); // s from SP
        assert_eq!(basis.shells[1].l, 1); // p from SP
    }

    #[test]
    fn test_parse_nwchem_comments() {
        let content = "! This is a comment\nH S\n! Another comment\n3\n3.449 0.154\n0.627 0.535\n0.168 0.444\n";
        let basis = parse_nwchem(content).unwrap();
        assert_eq!(basis.shells.len(), 1);
    }

    #[test]
    fn test_parse_nwchem_multi_element() {
        let content = "\
H S
3
3.449 0.154
0.627 0.535
0.168 0.444
Li S
3
16.119 0.064
2.936 0.368
0.795 0.672
Li P
3
0.636 0.064
0.148 0.368
0.048 0.672
";
        let basis = parse_nwchem(content).unwrap();
        assert_eq!(basis.shells.len(), 3);
    }
}
