//! # BatchCompiler — Auto-Generation of ERI Code for All Shell Quartets
//!
//! ## Overview
//!
//! The `BatchCompiler` is the third and final stage of the POLER-ERI v3.2.0
//! meta-compiler pipeline. It orchestrates the full code generation process by:
//!
//! 1. Iterating over all unique shell quartets `(la, lb | lc, ld)` up to a
//!    maximum angular momentum `MAX_AM`
//! 2. Building an R1CS [`Circuit`] for each quartet using
//!    [`CircuitBuilder`](crate::circuit::CircuitBuilder)
//! 3. Crystallizing each circuit into flat Rust source code using
//!    [`VrrCrystallizer`](crate::crystallizer::VrrCrystallizer)
//! 4. Writing the generated code to individual `.rs` files and producing
//!    a `mod.rs` that ties them all together
//!
//! ## v3.2.0 Changes
//!
//! - Updated version strings to v3.2.0
//! - Circuit now has `g_idx`, `hrr_bra_map`, `hrr_ket_map`, `rys_order` fields
//! - Crystallizer emits `#[inline(always)]`, `debug_assert!`, HRR index maps
//! - Shell driver module added for full Cartesian integral computation
//!
//! ## Shell Quartet Enumeration
//!
//! For a maximum angular momentum `MAX_AM`, the number of unique shell quartets
//! under permutational symmetry (la ≥ lb, lc ≥ ld) is:
//!
//! | MAX_AM | Quartets | Includes          |
//! |--------|----------|-------------------|
//! | 0      | 1        | (ss\|ss)          |
//! | 1      | 9        | s,s + p,p         |
//! | 2      | 36       | s,p,d             |
//! | 3      | 100      | s,p,d,f           |
//! | 4      | 225      | s,p,d,f,g         |

use crate::cart::shell_name;
use crate::circuit::CircuitBuilder;
use crate::crystallizer::VrrCrystallizer;

use std::fs;
use std::io;

// ─────────────────────────────────────────────────────────────────────────────
// GeneratedModule: A single crystallized ERI function
// ─────────────────────────────────────────────────────────────────────────────

/// A generated Rust source module for one shell quartet.
///
/// Contains the module name (used for the file name and function name),
/// the shell quartet specification, the generated Rust source code, and
/// the number of gates in the circuit (for summary reporting).
#[derive(Debug, Clone)]
pub struct GeneratedModule {
    /// Module/function name (e.g., "eri_pppp").
    pub name: String,
    /// Shell quartet (la, lb, lc, ld).
    pub quartet: (usize, usize, usize, usize),
    /// Generated Rust source code.
    pub code: String,
    /// Number of R1CS gates in the circuit.
    pub n_gates: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// BatchCompiler: Orchestrates batch code generation
// ─────────────────────────────────────────────────────────────────────────────

/// Batch compiler that generates ERI code for all shell quartets up to MAX_AM.
///
/// # Usage
///
/// ```rust
/// use poler_eri::BatchCompiler;
///
/// let batch = BatchCompiler::new(2);
/// let modules = batch.compile_all();
/// BatchCompiler::print_summary(&modules);
/// BatchCompiler::write_to_dir(&modules, "./generated").unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct BatchCompiler {
    /// Maximum angular momentum for shell quartet generation.
    pub max_am: usize,
}

impl BatchCompiler {
    /// Create a new batch compiler for shell quartets up to `max_am`.
    pub fn new(max_am: usize) -> Self {
        BatchCompiler { max_am }
    }

    /// Compile all shell quartets up to `max_am`.
    pub fn compile_all(&self) -> Vec<GeneratedModule> {
        let mut modules = Vec::new();

        for la in 0..=self.max_am {
            for lb in 0..=la {
                for lc in 0..=self.max_am {
                    for ld in 0..=lc {
                        modules.push(Self::compile_one(la, lb, lc, ld));
                    }
                }
            }
        }

        modules
    }

    /// Compile a single shell quartet.
    pub fn compile_one(la: usize, lb: usize, lc: usize, ld: usize) -> GeneratedModule {
        let name = format!(
            "eri_{}{}{}{}",
            shell_name(la),
            shell_name(lb),
            shell_name(lc),
            shell_name(ld)
        );

        let circuit = CircuitBuilder::new(la, lb, lc, ld).build_circuit();
        let n_gates = circuit.gates.len();
        let code = VrrCrystallizer::crystallize(&circuit, &name);

        GeneratedModule {
            name,
            quartet: (la, lb, lc, ld),
            code,
            n_gates,
        }
    }

    /// Compile a custom list of shell quartets.
    pub fn compile_quartets(quartets: &[(usize, usize, usize, usize)]) -> Vec<GeneratedModule> {
        quartets
            .iter()
            .map(|&(la, lb, lc, ld)| Self::compile_one(la, lb, lc, ld))
            .collect()
    }

    /// Write all generated modules to a directory.
    pub fn write_to_dir(modules: &[GeneratedModule], dir: &str) -> io::Result<()> {
        fs::create_dir_all(dir)?;

        for module in modules {
            let path = format!("{}/{}.rs", dir, module.name);
            fs::write(&path, &module.code)?;
        }

        let mod_rs = Self::generate_mod_rs(modules);
        let mod_path = format!("{}/mod.rs", dir);
        fs::write(&mod_path, mod_rs)?;

        Ok(())
    }

    /// Generate the contents of `mod.rs` that declares all generated modules.
    pub fn generate_mod_rs(modules: &[GeneratedModule]) -> String {
        let mut out = String::new();

        out.push_str("//! Auto-generated ERI module declarations.\n");
        out.push_str("//! Generated by POLER-ERI v3.2.0 BatchCompiler.\n");
        out.push_str(&format!(
            "//! Total modules: {}\n\n",
            modules.len()
        ));

        for module in modules {
            let (la, lb, lc, ld) = module.quartet;
            out.push_str(&format!(
                "pub mod {}; // ({},{},{},{}) — {} gates\n",
                module.name, la, lb, lc, ld, module.n_gates
            ));
        }

        out
    }

    /// Print a summary table of all generated modules.
    pub fn print_summary(modules: &[GeneratedModule]) {
        println!(
            "{:<16} {:<12} {:<8} {:<8} {:<12}",
            "Module", "Quartet", "Gates", "nmax", "mmax"
        );
        println!("{}", "-".repeat(60));

        let mut total_gates = 0usize;

        for module in modules {
            let (la, lb, lc, ld) = module.quartet;
            let quartet_str = format!(
                "({}{}|{}{})",
                shell_name(la),
                shell_name(lb),
                shell_name(lc),
                shell_name(ld)
            );

            let nmax = la + lb;
            let mmax = lc + ld;

            println!(
                "{:<16} {:<12} {:<8} {:<8} {:<12}",
                module.name, quartet_str, module.n_gates, nmax, mmax
            );

            total_gates += module.n_gates;
        }

        println!("{}", "-".repeat(60));
        println!(
            "Total: {} modules, {} gates",
            modules.len(),
            total_gates
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let compiler = BatchCompiler::new(3);
        assert_eq!(compiler.max_am, 3);
    }

    #[test]
    fn test_compile_all_s_only() {
        let compiler = BatchCompiler::new(0);
        let modules = compiler.compile_all();

        assert_eq!(modules.len(), 1, "max_am=0 should produce 1 module");
        assert_eq!(modules[0].quartet, (0, 0, 0, 0));
        assert_eq!(modules[0].name, "eri_ssss");
        assert_eq!(modules[0].n_gates, 0, "(ss|ss) should have 0 gates");
    }

    #[test]
    fn test_compile_all_sp() {
        let compiler = BatchCompiler::new(1);
        let modules = compiler.compile_all();

        assert_eq!(
            modules.len(),
            9,
            "max_am=1 should produce 9 modules, got {}",
            modules.len()
        );
    }

    #[test]
    fn test_compile_all_spd() {
        let compiler = BatchCompiler::new(2);
        let modules = compiler.compile_all();

        assert_eq!(
            modules.len(),
            36,
            "max_am=2 should produce 36 modules, got {}",
            modules.len()
        );
    }

    #[test]
    fn test_compile_one_pppp() {
        let module = BatchCompiler::compile_one(1, 1, 1, 1);

        assert_eq!(module.name, "eri_pppp");
        assert_eq!(module.quartet, (1, 1, 1, 1));
        assert!(module.n_gates > 0, "(pp|pp) should have gates");
        assert!(
            module.code.contains("pub fn eri_pppp"),
            "Code should contain the function definition"
        );
    }

    #[test]
    fn test_compile_one_ssss() {
        let module = BatchCompiler::compile_one(0, 0, 0, 0);

        assert_eq!(module.name, "eri_ssss");
        assert_eq!(module.n_gates, 0);
        assert!(module.code.contains("Base case"));
    }

    #[test]
    fn test_compile_quartets_custom() {
        let modules = BatchCompiler::compile_quartets(&[
            (0, 0, 0, 0),
            (1, 0, 0, 0),
            (1, 1, 1, 1),
        ]);

        assert_eq!(modules.len(), 3);
        assert_eq!(modules[0].name, "eri_ssss");
        assert_eq!(modules[1].name, "eri_psss");
        assert_eq!(modules[2].name, "eri_pppp");
    }

    #[test]
    fn test_module_names() {
        let compiler = BatchCompiler::new(2);
        let modules = compiler.compile_all();

        for module in &modules {
            let (la, lb, lc, ld) = module.quartet;
            let expected = format!(
                "eri_{}{}{}{}",
                shell_name(la),
                shell_name(lb),
                shell_name(lc),
                shell_name(ld)
            );
            assert_eq!(
                module.name, expected,
                "Module name mismatch for quartet ({},{},{},{})",
                la, lb, lc, ld
            );
        }
    }

    #[test]
    fn test_generated_code_structure() {
        let compiler = BatchCompiler::new(1);
        let modules = compiler.compile_all();

        for module in &modules {
            assert!(
                module.code.contains("pub fn"),
                "Module {} should contain a function definition",
                module.name
            );

            assert!(
                module.code.contains("operands: &mut [f64]")
                    && module.code.contains("prefactors: &[f64]"),
                "Module {} should have the correct function signature",
                module.name
            );

            assert!(
                module.code.trim_end().ends_with('}'),
                "Module {} should end with '}}'",
                module.name
            );
        }
    }

    #[test]
    fn test_generate_mod_rs() {
        let modules = BatchCompiler::compile_quartets(&[
            (0, 0, 0, 0),
            (1, 1, 1, 1),
        ]);

        let mod_rs = BatchCompiler::generate_mod_rs(&modules);

        assert!(mod_rs.contains("pub mod eri_ssss"));
        assert!(mod_rs.contains("pub mod eri_pppp"));
        assert!(mod_rs.contains("Auto-generated"));
        assert!(mod_rs.contains("Total modules: 2"));
    }

    #[test]
    fn test_write_to_dir() {
        let dir = format!(
            "/tmp/poler_eri_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let modules = BatchCompiler::compile_quartets(&[
            (0, 0, 0, 0),
            (1, 1, 1, 1),
        ]);

        let result = BatchCompiler::write_to_dir(&modules, &dir);
        assert!(result.is_ok(), "write_to_dir should succeed: {:?}", result);

        assert!(
            std::path::Path::new(&format!("{}/eri_ssss.rs", dir)).exists(),
            "eri_ssss.rs should exist"
        );
        assert!(
            std::path::Path::new(&format!("{}/eri_pppp.rs", dir)).exists(),
            "eri_pppp.rs should exist"
        );
        assert!(
            std::path::Path::new(&format!("{}/mod.rs", dir)).exists(),
            "mod.rs should exist"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_gate_count_monotonicity() {
        let psss = BatchCompiler::compile_one(1, 0, 0, 0);
        let pppp = BatchCompiler::compile_one(1, 1, 1, 1);

        assert!(
            pppp.n_gates > psss.n_gates,
            "(pp|pp) should have more gates ({}) than (ps|ss) ({})",
            pppp.n_gates,
            psss.n_gates
        );
    }

    #[test]
    fn test_print_summary_no_panic() {
        let compiler = BatchCompiler::new(1);
        let modules = compiler.compile_all();
        BatchCompiler::print_summary(&modules);
    }

    #[test]
    fn test_compile_all_ordering() {
        let compiler = BatchCompiler::new(2);
        let modules = compiler.compile_all();

        for window in modules.windows(2) {
            let (la1, lb1, lc1, ld1) = window[0].quartet;
            let (la2, lb2, lc2, ld2) = window[1].quartet;

            assert!(
                (la1, lb1, lc1, ld1) <= (la2, lb2, lc2, ld2),
                "Quartets should be in lexicographic order"
            );
        }
    }

    #[test]
    fn test_total_gate_count_sp() {
        let compiler = BatchCompiler::new(1);
        let modules = compiler.compile_all();

        let total_gates: usize = modules.iter().map(|m| m.n_gates).sum();
        assert!(
            total_gates > 0,
            "Total gate count should be positive, got {}",
            total_gates
        );
    }
}
