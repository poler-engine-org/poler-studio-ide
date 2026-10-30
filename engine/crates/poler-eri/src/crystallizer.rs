//! # VrrCrystallizer — Circuit-to-Code Emitter
//!
//! ## Overview
//!
//! The Crystallizer is the second stage of the POLER-ERI meta-compiler pipeline.
//! It takes a [`Circuit`](crate::circuit::Circuit) (produced by the
//! [`CircuitBuilder`](crate::circuit::CircuitBuilder)) and emits **flat Rust
//! source code** — a function that evaluates the circuit with no loops, no
//! branches, no allocations, and no indirection.
//!
//! ## What is "Crystallization"?
//!
//! The term "crystallization" is an analogy from materials science: just as a
//! liquid crystallizes into a rigid, ordered solid with a definite structure,
//! the circuit's abstract gate sequence crystallizes into a rigid, ordered
//! sequence of Rust statements with a definite memory layout.
//!
//! More concretely, crystallization means:
//!
//! 1. **No loops** — Each gate becomes exactly one Rust statement.
//! 2. **No branches** — No `if`, `match`, or conditional expressions.
//! 3. **No allocations** — All intermediate results are in the `operands` slice.
//! 4. **No indirection** — All array indices are literal constants.
//!
//! ## v3.2.0 Changes
//!
//! - Generated functions now carry `#[inline(always)]` for aggressive inlining.
//! - Generated functions include `debug_assert!` bounds checks on `operands`
//!   and `prefactors` slices at entry, using emitted `N_OPERANDS` and
//!   `N_PREFACTORS` constants.
//! - The doc comment on each generated function now clearly documents the
//!   result operand: `/// Result: operands[RESULT_IDX] = I(la,lb|lc,ld)`.
//! - HRR intermediate index maps are emitted as `pub const HRR_BRA_MAP` and
//!   `pub const HRR_KET_MAP`, enabling downstream code to locate any
//!   intermediate `I(na,nb|nc,nd)` without re-running the circuit.
//! - Version strings updated from v3.1.0 to v3.2.0.
//!
//! ## v3.1.0 Changes
//!
//! - Added support for `GateCoeff::ScaledPrefactor(scalar, idx)` which emits
//!   `scalar * prefactors[idx]` in the generated code.
//! - Updated stage names to 5 stages (VRR-Bra, VRR-Ket, VRR-Full, HRR-Bra, HRR-Ket).
//! - Updated doc comments to document the fixed 7-prefactor layout.
//!
//! ## Generated Code Format
//!
//! For a non-trivial quartet, the crystallizer produces code like:
//!
//! ```rust,ignore
//! const N_OPERANDS: usize = 22;
//! const N_PREFACTORS: usize = 7;
//!
//! /// HRR-Bra index map: [m][entry] -> (na, nb, operand_idx)
//! pub const HRR_BRA_MAP: &[[[usize; 3]]] = &[/* ... */];
//!
//! /// HRR-Ket index map: [(nc, nd, operand_idx), ...]
//! pub const HRR_KET_MAP: &[[usize; 3]] = &[/* ... */];
//!
//! #[inline(always)]
//! pub fn eri_pppp(operands: &mut [f64], prefactors: &[f64]) {
//!     debug_assert!(operands.len() >= N_OPERANDS, ...);
//!     debug_assert!(prefactors.len() >= N_PREFACTORS, ...);
//!     // Gate 0: VRR-Bra [VRR-Bra: g[1][0] = c00*g[0][0]]
//!     operands[1] = prefactors[0] * operands[0];
//!     // ...
//! }
//! ```
//!
//! ## Prefactor Layout (v3.2.0)
//!
//! The generated function expects exactly 7 prefactors:
//!
//! | Index | Name | Description                         |
//! |-------|------|-------------------------------------|
//! | 0     | c00  | PA_i (root-dependent for Rys)        |
//! | 1     | c0p  | QC_i (root-dependent for Rys)        |
//! | 2     | b10  | 1/(2p) (root-independent)           |
//! | 3     | b01  | 1/(2q) (root-independent)           |
//! | 4     | b00  | t/(2(p+q)) (root-dependent)         |
//! | 5     | -AB  | -(A_i - B_i) (root-independent)     |
//! | 6     | -CD  | -(C_i - D_i) (root-independent)     |

use crate::circuit::{Circuit, GateCoeff};

// ─────────────────────────────────────────────────────────────────────────────
// VrrCrystallizer: Unit struct with associated methods
// ─────────────────────────────────────────────────────────────────────────────

/// The VRR circuit crystallizer — converts a [`Circuit`] into flat Rust source code.
///
/// `VrrCrystallizer` is a unit struct (zero-sized type). All functionality is
/// provided through associated methods, not through `&self` methods.
///
/// # Usage
///
/// ```rust
/// use poler_eri::{CircuitBuilder, VrrCrystallizer};
///
/// let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
/// let code = VrrCrystallizer::crystallize(&circuit, "eri_pppp");
/// println!("{}", code);
/// ```
pub struct VrrCrystallizer;

impl VrrCrystallizer {
    /// Crystallize a circuit into a Rust function with associated constants.
    ///
    /// Converts the circuit's gate sequence into a Rust function body with one
    /// statement per gate. The generated output includes:
    ///
    /// - `const N_OPERANDS: usize` — number of operand slots required
    /// - `const N_PREFACTORS: usize` — number of prefactor slots required
    /// - `pub const HRR_BRA_MAP: &[[[usize; 3]]]` — HRR-Bra intermediate index map
    /// - `pub const HRR_KET_MAP: &[[usize; 3]]` — HRR-Ket intermediate index map
    /// - The function itself, with `#[inline(always)]` and `debug_assert!` guards
    ///
    /// The generated function signature is:
    ///
    /// ```rust,ignore
    /// #[inline(always)]
    /// pub fn {name}(operands: &mut [f64], prefactors: &[f64])
    /// ```
    ///
    /// # Arguments
    ///
    /// - `circuit` — The R1CS circuit to crystallize
    /// - `name` — The name of the generated function (e.g., `"eri_pppp"`)
    ///
    /// # Returns
    ///
    /// A `String` containing the complete Rust source code (constants + function).
    pub fn crystallize(circuit: &Circuit, name: &str) -> String {
        let mut out = String::new();

        // ── Constants: N_OPERANDS and N_PREFACTORS ──
        out.push_str(&format!(
            "/// Number of operand slots required by the crystallized function.\n"
        ));
        out.push_str(&format!(
            "pub const N_OPERANDS: usize = {};\n",
            circuit.n_operands
        ));
        out.push_str(&format!(
            "/// Number of prefactor slots required by the crystallized function.\n"
        ));
        out.push_str(&format!(
            "pub const N_PREFACTORS: usize = {};\n",
            circuit.n_prefactors
        ));
        out.push_str("\n");

        // ── HRR-Bra index map ──
        Self::emit_hrr_bra_map(circuit, &mut out);
        out.push_str("\n");

        // ── HRR-Ket index map ──
        Self::emit_hrr_ket_map(circuit, &mut out);
        out.push_str("\n");

        // ── Function header with documentation ──
        out.push_str(&format!(
            "/// Crystallized ERI function for shell quartet {}.\n",
            circuit.quartet_label()
        ));
        out.push_str(&format!(
            "/// Generated by POLER-ERI v3.2.0 VrrCrystallizer.\n"
        ));
        out.push_str(&format!(
            "/// Gates: {}, Operands: {}, Prefactors: {}\n",
            circuit.gates.len(),
            circuit.n_operands,
            circuit.n_prefactors
        ));
        out.push_str(&format!(
            "/// nmax={}, mmax={}, rys_order={}, result_operand={}\n",
            circuit.nmax, circuit.mmax, circuit.rys_order, circuit.result_operand
        ));
        out.push_str(&format!(
            "/// Result: operands[{}] = I({},{},{},{})\n",
            circuit.result_operand, circuit.la, circuit.lb, circuit.lc, circuit.ld
        ));
        out.push_str("///\n");
        out.push_str("/// Prefactor layout:\n");
        out.push_str("///   [0] c00  = PA_i (root-dependent: PA_i + t*PQ_i for Rys)\n");
        out.push_str("///   [1] c0p  = QC_i (root-dependent: QC_i + t*PQ_i for Rys)\n");
        out.push_str("///   [2] b10  = 1/(2p) (root-independent)\n");
        out.push_str("///   [3] b01  = 1/(2q) (root-independent)\n");
        out.push_str("///   [4] b00  = t/(2(p+q)) (root-dependent)\n");
        out.push_str("///   [5] -AB  = -(A_i - B_i) (root-independent)\n");
        out.push_str("///   [6] -CD  = -(C_i - D_i) (root-independent)\n");
        out.push_str("///\n");
        out.push_str("/// Caller: operands[0] = 1.0 (base case). Evaluate for each\n");
        out.push_str("/// Cartesian direction (x,y,z) and each Rys root, then compute\n");
        out.push_str("/// gx * gy * gz * weight and sum over roots.\n");

        // ── #[inline(always)] attribute ──
        out.push_str("#[inline(always)]\n");

        out.push_str(&format!(
            "pub fn {}(operands: &mut [f64], prefactors: &[f64]) {{\n",
            name
        ));

        // ── debug_assert! bounds checks ──
        out.push_str(&format!(
            "    debug_assert!(operands.len() >= N_OPERANDS, \"operands slice too short: got {{}}, need {{}}\", operands.len(), N_OPERANDS);\n"
        ));
        out.push_str(&format!(
            "    debug_assert!(prefactors.len() >= N_PREFACTORS, \"prefactors slice too short: got {{}}, need {{}}\", prefactors.len(), N_PREFACTORS);\n"
        ));

        if circuit.is_base_case() {
            // Base case: no recurrence steps
            out.push_str(
                "    // Base case: operands[0] = 1.0 (Rys polynomial at order 0) — no recurrence steps needed.\n",
            );
        } else {
            // Emit one statement per gate
            for (i, gate) in circuit.gates.iter().enumerate() {
                let stage_name = stage_name(gate.stage);

                // Comment line
                out.push_str(&format!(
                    "    // Gate {}: {} [{}]\n",
                    i, stage_name, gate.label
                ));

                // Code line
                let cl_str = Self::format_coeff(&gate.coeff_left);
                let cr_str = Self::format_coeff(&gate.coeff_right);

                let stmt = match (&gate.coeff_left, &gate.coeff_right) {
                    // Both coefficients are zero: result = 0.0
                    (GateCoeff::Zero, GateCoeff::Zero) => {
                        format!("    operands[{}] = 0.0;\n", gate.result)
                    }
                    // Right is zero: result = cl * left
                    (GateCoeff::Zero, _) => {
                        format!(
                            "    operands[{}] = {} * operands[{}];\n",
                            gate.result, cr_str, gate.right
                        )
                    }
                    // Left is zero: result = cr * right
                    (_, GateCoeff::Zero) => {
                        format!(
                            "    operands[{}] = {} * operands[{}];\n",
                            gate.result, cl_str, gate.left
                        )
                    }
                    // Both non-zero: result = cl * left + cr * right
                    _ => {
                        format!(
                            "    operands[{}] = {} * operands[{}] + {} * operands[{}];\n",
                            gate.result, cl_str, gate.left, cr_str, gate.right
                        )
                    }
                };

                out.push_str(&stmt);
            }
        }

        out.push_str("}\n");
        out
    }

    /// Emit the `HRR_BRA_MAP` const array for the circuit.
    ///
    /// The map is structured as `&[[[usize; 3]]]`, indexed by `m` (ket level),
    /// where each entry is `[na, nb, operand_idx]` representing `I(na, nb | m, 0)`.
    fn emit_hrr_bra_map(circuit: &Circuit, out: &mut String) {
        out.push_str("/// HRR-Bra index map: [m][entry] -> (na, nb, operand_idx)\n");
        out.push_str("/// For each m, lists all I(na, nb | m, 0) results\n");
        out.push_str("pub const HRR_BRA_MAP: &[[[usize; 3]]] = &[\n");

        for (m, entries) in circuit.hrr_bra_map.iter().enumerate() {
            out.push_str(&format!("    // m={}: [(na, nb, operand_idx), ...]\n", m));
            out.push_str("    &[\n");
            for entry in entries {
                out.push_str(&format!(
                    "        [{}, {}, {}],  // I({}, {} | {}, 0)\n",
                    entry.i, entry.j, entry.operand, entry.i, entry.j, m
                ));
            }
            out.push_str("    ],\n");
        }

        out.push_str("];\n");
    }

    /// Emit the `HRR_KET_MAP` const array for the circuit.
    ///
    /// The map is structured as `&[[usize; 3]]`, where each entry is
    /// `[nc, nd, operand_idx]` representing `I(la, lb | nc, nd)`.
    fn emit_hrr_ket_map(circuit: &Circuit, out: &mut String) {
        out.push_str("/// HRR-Ket index map: [(nc, nd, operand_idx), ...]\n");
        out.push_str(&format!(
            "/// Each entry is I({}, {} | nc, nd)\n",
            circuit.la, circuit.lb
        ));
        out.push_str("pub const HRR_KET_MAP: &[[usize; 3]] = &[\n");

        for entry in &circuit.hrr_ket_map {
            out.push_str(&format!(
                "    [{}, {}, {}],  // I({}, {} | {}, {})\n",
                entry.i, entry.j, entry.operand,
                circuit.la, circuit.lb, entry.i, entry.j
            ));
        }

        out.push_str("];\n");
    }

    /// Format a [`GateCoeff`] as a Rust expression string.
    ///
    /// The mapping is:
    ///
    /// | GateCoeff                | Rust Expression                  |
    /// |--------------------------|----------------------------------|
    /// | `Zero`                   | `"0.0"`                          |
    /// | `One`                    | `"1.0"`                          |
    /// | `Scalar(s)`              | `"{s:.15e}"` (full precision)    |
    /// | `BoyF(m)`                | `"boys_f({m})"`                  |
    /// | `Prefactor(i)`           | `"prefactors[{i}]"`              |
    /// | `Operand(i)`             | `"operands[{i}]"`                |
    /// | `ScaledPrefactor(s, i)`  | `"{s:.15e} * prefactors[{i}]"`   |
    fn format_coeff(coeff: &GateCoeff) -> String {
        match coeff {
            GateCoeff::Zero => "0.0".to_string(),
            GateCoeff::One => "1.0".to_string(),
            GateCoeff::Scalar(s) => format!("{:.15e}", s),
            GateCoeff::BoyF(m) => format!("boys_f({})", m),
            GateCoeff::Prefactor(i) => format!("prefactors[{}]", i),
            GateCoeff::Operand(i) => format!("operands[{}]", i),
            GateCoeff::ScaledPrefactor(scalar, i) => {
                // If scalar is a whole number, format without decimal point noise
                if *scalar == scalar.floor() && *scalar >= 0.0 && *scalar < 100.0 {
                    let n = *scalar as i64;
                    format!("{} * prefactors[{}]", n, i)
                } else {
                    format!("{:.15e} * prefactors[{}]", scalar, i)
                }
            }
        }
    }
}

/// Map a stage index to its human-readable name.
///
/// # Panics
///
/// Panics if `stage > 4`, which should never happen for a well-formed circuit.
fn stage_name(stage: usize) -> &'static str {
    match stage {
        0 => "VRR-Bra",
        1 => "VRR-Ket",
        2 => "VRR-Full",
        3 => "HRR-Bra",
        4 => "HRR-Ket",
        _ => panic!(
            "Invalid stage index {}: must be 0-4 (VRR-Bra, VRR-Ket, VRR-Full, HRR-Bra, HRR-Ket)",
            stage
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::CircuitBuilder;

    /// Test that (ss|ss) crystallizes to a function with debug_assert and base case.
    #[test]
    fn test_crystallize_ssss() {
        let circuit = CircuitBuilder::new(0, 0, 0, 0).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_ssss");

        assert!(
            code.contains("pub fn eri_ssss(operands: &mut [f64], prefactors: &[f64])"),
            "Should contain function signature"
        );
        assert!(
            code.contains("Base case"),
            "(ss|ss) should mention base case"
        );
    }

    /// Test that (ps|ss) crystallizes to a function with VRR-Bra gates.
    #[test]
    fn test_crystallize_psss() {
        let circuit = CircuitBuilder::new(1, 0, 0, 0).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_psss");

        assert!(
            code.contains("pub fn eri_psss"),
            "Should contain function signature"
        );
        assert!(
            code.contains("VRR-Bra"),
            "(ps|ss) should contain VRR-Bra stage"
        );
        assert!(
            code.contains("prefactors[0] * operands[0]"),
            "(ps|ss) should multiply c00 by base operand"
        );
    }

    /// Test that (pp|pp) crystallizes with all five stage names present.
    #[test]
    fn test_crystallize_pppp_all_stages() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_pppp");

        assert!(code.contains("VRR-Bra"), "(pp|pp) should contain VRR-Bra");
        assert!(code.contains("VRR-Ket"), "(pp|pp) should contain VRR-Ket");
        assert!(code.contains("VRR-Full"), "(pp|pp) should contain VRR-Full");
        assert!(code.contains("HRR-Bra"), "(pp|pp) should contain HRR-Bra");
        assert!(code.contains("HRR-Ket"), "(pp|pp) should contain HRR-Ket");
    }

    /// Test that Zero-coefficient gates produce simplified code (no addition).
    #[test]
    fn test_zero_coeff_simplification() {
        let circuit = CircuitBuilder::new(1, 0, 0, 0).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_psss");

        assert!(
            !code.contains("+ 0.0 * operands["),
            "Zero coefficient should be simplified away"
        );
    }

    /// Test that ScaledPrefactor formats correctly.
    #[test]
    fn test_scaled_prefactor_format() {
        use crate::circuit::GateCoeff;
        // Integer-valued scalar
        assert_eq!(
            VrrCrystallizer::format_coeff(&GateCoeff::ScaledPrefactor(2.0, 3)),
            "2 * prefactors[3]"
        );
        assert_eq!(
            VrrCrystallizer::format_coeff(&GateCoeff::ScaledPrefactor(1.0, 4)),
            "1 * prefactors[4]"
        );
        // Non-integer scalar
        let result = VrrCrystallizer::format_coeff(&GateCoeff::ScaledPrefactor(1.5, 2));
        assert!(result.contains("1.5") && result.contains("prefactors[2]"));
    }

    /// Test format_coeff for all variants.
    #[test]
    fn test_format_coeff_all_variants() {
        use crate::circuit::GateCoeff;

        assert_eq!(VrrCrystallizer::format_coeff(&GateCoeff::Zero), "0.0");
        assert_eq!(VrrCrystallizer::format_coeff(&GateCoeff::One), "1.0");
        assert_eq!(
            VrrCrystallizer::format_coeff(&GateCoeff::Scalar(1.5)),
            format!("{:.15e}", 1.5f64)
        );
        assert_eq!(
            VrrCrystallizer::format_coeff(&GateCoeff::BoyF(0)),
            "boys_f(0)"
        );
        assert_eq!(
            VrrCrystallizer::format_coeff(&GateCoeff::Prefactor(0)),
            "prefactors[0]"
        );
        assert_eq!(
            VrrCrystallizer::format_coeff(&GateCoeff::Operand(2)),
            "operands[2]"
        );
    }

    /// Test that the generated code contains gate number comments.
    #[test]
    fn test_gate_number_comments() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_pppp");

        // Should have "Gate 0:", "Gate 1:", etc.
        assert!(code.contains("Gate 0:"), "Should contain 'Gate 0:' comment");
        assert!(code.contains("Gate 1:"), "Should contain 'Gate 1:' comment");
    }

    /// Test that the generated code includes the quartet label in the doc comment.
    #[test]
    fn test_quartet_label_in_doc() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_pppp");

        assert!(
            code.contains("(pp|pp)"),
            "Doc comment should mention the quartet label"
        );
    }

    /// Test that the generated code includes prefactor layout documentation.
    #[test]
    fn test_prefactor_layout_in_doc() {
        let circuit = CircuitBuilder::new(1, 0, 0, 0).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_psss");

        assert!(code.contains("Prefactor layout"), "Should document prefactor layout");
        assert!(code.contains("[0] c00"), "Should document c00");
        assert!(code.contains("[4] b00"), "Should document b00");
    }

    /// Test stage_name function for all valid stages.
    #[test]
    fn test_stage_name() {
        assert_eq!(stage_name(0), "VRR-Bra");
        assert_eq!(stage_name(1), "VRR-Ket");
        assert_eq!(stage_name(2), "VRR-Full");
        assert_eq!(stage_name(3), "HRR-Bra");
        assert_eq!(stage_name(4), "HRR-Ket");
    }

    /// Test that stage_name panics for invalid stage.
    #[test]
    #[should_panic(expected = "Invalid stage index")]
    fn test_stage_name_invalid() {
        stage_name(5);
    }

    /// Test that the function name is correctly used.
    #[test]
    fn test_custom_function_name() {
        let circuit = CircuitBuilder::new(0, 0, 0, 0).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "my_custom_eri");

        assert!(
            code.contains("pub fn my_custom_eri"),
            "Should use the provided function name"
        );
    }

    /// Test that (dd|dd) crystallizes successfully with many gates.
    #[test]
    fn test_crystallize_dddd() {
        let circuit = CircuitBuilder::new(2, 2, 2, 2).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_dddd");

        // Should have significantly more gates than v3.0.x (which had 8)
        let gate_count = code.matches("Gate ").count();
        assert!(
            gate_count > 8,
            "(dd|dd) should have more than 8 gates in v3.2.0, found {}",
            gate_count
        );
    }

    /// Test that the version string is v3.2.0.
    #[test]
    fn test_version_string() {
        let circuit = CircuitBuilder::new(0, 0, 0, 0).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_ssss");
        assert!(
            code.contains("v3.2.0"),
            "Generated code should reference v3.2.0"
        );
    }

    /// Test that the generated code includes `#[inline(always)]`.
    #[test]
    fn test_inline_always_attribute() {
        let circuit = CircuitBuilder::new(0, 0, 0, 0).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_ssss");
        assert!(
            code.contains("#[inline(always)]"),
            "Generated function should have #[inline(always)]"
        );
    }

    /// Test that the generated code includes debug_assert! bounds checks.
    #[test]
    fn test_debug_assert_bounds() {
        let circuit = CircuitBuilder::new(1, 0, 0, 0).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_psss");

        assert!(
            code.contains("debug_assert!(operands.len() >= N_OPERANDS"),
            "Should contain operands debug_assert"
        );
        assert!(
            code.contains("debug_assert!(prefactors.len() >= N_PREFACTORS"),
            "Should contain prefactors debug_assert"
        );
        assert!(
            code.contains("operands slice too short"),
            "Should contain operands error message"
        );
        assert!(
            code.contains("prefactors slice too short"),
            "Should contain prefactors error message"
        );
    }

    /// Test that N_OPERANDS and N_PREFACTORS constants are emitted.
    #[test]
    fn test_constants_emitted() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_pppp");

        assert!(
            code.contains("pub const N_OPERANDS: usize"),
            "Should emit N_OPERANDS constant"
        );
        assert!(
            code.contains("pub const N_PREFACTORS: usize"),
            "Should emit N_PREFACTORS constant"
        );
        assert!(
            code.contains(&format!("pub const N_PREFACTORS: usize = {};", circuit.n_prefactors)),
            "N_PREFACTORS should match circuit"
        );
    }

    /// Test that result_operand is documented as I(la,lb|lc,ld).
    #[test]
    fn test_result_operand_documented() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_pppp");

        assert!(
            code.contains(&format!(
                "Result: operands[{}] = I(1,1,1,1)",
                circuit.result_operand
            )),
            "Should document result_operand as I(la,lb,lc,ld)"
        );
    }

    /// Test that HRR_BRA_MAP is emitted.
    #[test]
    fn test_hrr_bra_map_emitted() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_pppp");

        assert!(
            code.contains("pub const HRR_BRA_MAP: &[[[usize; 3]]]"),
            "Should emit HRR_BRA_MAP constant"
        );
        assert!(
            code.contains("HRR-Bra index map"),
            "Should document HRR_BRA_MAP"
        );
    }

    /// Test that HRR_KET_MAP is emitted.
    #[test]
    fn test_hrr_ket_map_emitted() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_pppp");

        assert!(
            code.contains("pub const HRR_KET_MAP: &[[usize; 3]]"),
            "Should emit HRR_KET_MAP constant"
        );
        assert!(
            code.contains("HRR-Ket index map"),
            "Should document HRR_KET_MAP"
        );
    }

    /// Test that rys_order is documented in the function doc comment.
    #[test]
    fn test_rys_order_in_doc() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_pppp");

        assert!(
            code.contains("rys_order="),
            "Should document rys_order in function doc"
        );
        assert!(
            code.contains(&format!("rys_order={}", circuit.rys_order)),
            "rys_order value should match circuit"
        );
    }

    /// Test that the base case (ss|ss) still has inline and debug_assert.
    #[test]
    fn test_base_case_has_inline_and_assert() {
        let circuit = CircuitBuilder::new(0, 0, 0, 0).build_circuit();
        let code = VrrCrystallizer::crystallize(&circuit, "eri_ssss");

        assert!(
            code.contains("#[inline(always)]"),
            "Base case should still have #[inline(always)]"
        );
        assert!(
            code.contains("debug_assert!(operands.len() >= N_OPERANDS"),
            "Base case should still have operands debug_assert"
        );
        assert!(
            code.contains("debug_assert!(prefactors.len() >= N_PREFACTORS"),
            "Base case should still have prefactors debug_assert"
        );
    }
}
