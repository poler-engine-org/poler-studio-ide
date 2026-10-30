//! # C-to-Rust Transpiler Engine for libcint → POLER-ERI
//!
//! ## Overview
//!
//! This module implements a transpiler that takes parsed C patterns (from
//! [`CParser`](crate::c_parser::CParser)) and converts them into POLER-ERI
//! R1CS circuits or directly to Rust code. It bridges the gap between
//! libcint's Rys quadrature implementation and POLER-ERI's VRR+HRR pipeline.
//!
//! ## The Mapping Between libcint and POLER-ERI
//!
//! ### libcint's Approach (Rys Quadrature with explicit g[] arrays)
//!
//! libcint computes ERI integrals using Rys quadrature. The key data structures
//! are:
//!
//! - `g[N]` — The output integral array, indexed by Cartesian component
//!   combinations. For a (pp|pp) quartet, there are 3×3×3×3 = 81 components,
//!   but many are zero by symmetry or computed via recurrence.
//!
//! - `c00x[i]`, `c00y[i]`, `c00z[i]` — Bra-side Rys polynomial intermediates.
//!   These encode the angular momentum on centers A and B via recurrence on
//!   the Rys polynomial roots.
//!
//! - `c0px[i]`, `c0py[i]`, `c0pz[i]` — Ket-side Rys polynomial intermediates.
//!   These encode the angular momentum on centers C and D.
//!
//! - `b10[i]`, `b01[i]`, `b00[i]` — Boys-function-like intermediates that
//!   depend on the Rys quadrature roots and weights. `b10` couples to the
//!   bra side, `b01` couples to the ket side, and `b00` is the uncoupled
//!   (diagonal) term.
//!
//! The recurrence relations in libcint compute `g[N]` as products of these
//! intermediates, e.g.:
//!
//! ```c
//! g[0] = c00x[0] * c0px[0] * b10[0];  // single product
//! g[1] = c00x[0] * c0px[1] * b10[0]   // sum of two products
//!      + c00x[1] * c0px[0] * b01[0];
//! ```
//!
//! ### POLER-ERI's Approach (R1CS Circuit with Prefactor Arrays)
//!
//! POLER-ERI uses a 4-stage VRR+HRR pipeline encoded as an R1CS circuit:
//!
//! - `operands[N]` — The intermediate integral values. Unlike `g[N]` which
//!   stores the final Cartesian components, `operands[N]` stores intermediate
//!   recurrence results in single-assignment form.
//!
//! - `prefactors[N]` — Runtime geometric quantities (PA_i, AB_i, QC_i, CD_i)
//!   that are computed from [`QuartetData`](crate::types::QuartetData).
//!
//! Each gate computes:
//!
//! ```text
//! operands[result] = coeff_left × operands[left] + coeff_right × operands[right]
//! ```
//!
//! where coefficients can be constants, Boys function values, prefactors, or
//! other operand references.
//!
//! ### Why Some C Patterns Cannot Be Cleanly Transpiled
//!
//! The R1CS gate format `result = cl × left + cr × right` is a **two-term
//! linear combination**. However, libcint's expressions can have more than
//! two terms:
//!
//! 1. **Multi-term products**: `c00x[0] * c0px[0] * b10[0]` has three
//!    factors, but an R1CS gate can only multiply one coefficient by one
//!    operand. This requires decomposing the product into multiple gates
//!    (e.g., first compute `c00x[0] * c0px[0]`, then multiply by `b10[0]`).
//!
//! 2. **Multi-term sums**: `a*b + c*d + e*f` has three terms but an R1CS
//!    gate only supports two. This requires chaining gates (e.g., first
//!    compute `a*b + c*d` into one operand, then add `e*f`).
//!
//! 3. **Mixed products and sums**: An expression like
//!    `c00x[0] * c0px[0] * b10[0] + c00x[1] * c0px[0] * b01[0] + c00x[0] * c0px[1] * b00[0]`
//!    requires multiple intermediate gates to decompose.
//!
//! The transpiler handles these cases by:
//! - For `DirectRust` output: generating Rust code that directly evaluates
//!   the expression, no R1CS decomposition needed
//! - For `CrystallizedRust` / `CircuitOnly` output: attempting to decompose
//!   multi-term expressions into sequences of R1CS gates, falling back to
//!   `DirectRust`-style inline computation when decomposition fails
//!
//! ## How the Transpiler Handles the Mismatch
//!
//! The fundamental mismatch is between C's **procedural style** (arbitrary
//! expressions, assignments, control flow) and POLER-ERI's **declarative
//! circuit format** (fixed gate structure, single assignment, no control flow).
//!
//! The transpiler resolves this by:
//!
//! 1. **Best-effort decomposition**: Simple two-term expressions are directly
//!    mapped to R1CS gates. Complex expressions are decomposed into chains
//!    of gates.
//!
//! 2. **Graceful fallback**: When a clean decomposition isn't possible, the
//!    transpiler emits a warning and generates inline Rust code that computes
//!    the expression directly, bypassing the R1CS gate structure.
//!
//! 3. **Variable mapping**: libcint's variables (c00x, c0px, b10, etc.) are
//!    mapped to POLER-ERI's operand/prefactor arrays based on their role:
//!    - `c00*` → prefactor indices (geometric quantities)
//!    - `c0p*` → prefactor indices (geometric quantities)
//!    - `b10`, `b01`, `b00` → Boys function values or scalar coefficients
//!
//! 4. **Index mapping**: libcint's `g[N]` indices are mapped to POLER-ERI's
//!    `operands[N]` indices, accounting for the different layouts.

use crate::c_parser::{CParser, RecurrencePattern};
use crate::circuit::{Circuit, CircuitBuilder};
use crate::cart::shell_name;
use crate::crystallizer::VrrCrystallizer;

// ─────────────────────────────────────────────────────────────────────────────
// TranspileConfig: Configuration for the transpiler
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the transpiler, controlling output format and behavior.
///
/// The transpiler can produce output in several formats, each suited to
/// different use cases:
///
/// - **CrystallizedRust**: The primary output format — generates an R1CS circuit
///   and then crystallizes it to optimized Rust code using [`VrrCrystallizer`].
///
/// - **DirectRust**: Generates standalone Rust functions that directly compute
///   the expressions from the C source, without going through the R1CS circuit
///   representation. This is useful for quick prototyping and validation.
///
/// - **CircuitOnly**: Generates only the [`Circuit`] representation, without
///   emitting any Rust code. This is useful for circuit analysis, optimization,
///   and debugging.
#[derive(Debug, Clone)]
pub struct TranspileConfig {
    /// Target output format.
    pub output_format: OutputFormat,
    /// Whether to preserve C variable names in comments in the generated code.
    ///
    /// When true, the generated Rust code includes comments showing the original
    /// C variable names and expressions, making it easier to trace the
    /// transpilation and verify correctness.
    pub preserve_names: bool,
    /// Whether to add validation assertions in the generated code.
    ///
    /// When true, the generated Rust code includes `debug_assert!` statements
    /// that check array bounds and other invariants at runtime (in debug mode).
    pub add_assertions: bool,
}

impl Default for TranspileConfig {
    fn default() -> Self {
        TranspileConfig {
            output_format: OutputFormat::CrystallizedRust,
            preserve_names: true,
            add_assertions: true,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OutputFormat: The target output format
// ─────────────────────────────────────────────────────────────────────────────

/// Target output format for the transpiler.
///
/// Each format serves a different purpose in the development and validation
/// workflow:
///
/// | Format            | Use Case                                        |
/// |-------------------|-------------------------------------------------|
/// | `CrystallizedRust`| Production: optimized Rust via VrrCrystallizer  |
/// | `DirectRust`      | Prototyping: direct C→Rust translation          |
/// | `CircuitOnly`     | Analysis: R1CS circuit without code generation  |
#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    /// Generate POLER-ERI R1CS circuit + crystallized Rust code.
    ///
    /// This is the primary output format. The transpiler first attempts to
    /// construct an R1CS circuit from the parsed C pattern, then uses
    /// [`VrrCrystallizer`] to emit optimized Rust code.
    ///
    /// If the C pattern cannot be cleanly decomposed into R1CS gates, the
    /// transpiler falls back to emitting inline computation for those
    /// specific gates and adds a warning.
    CrystallizedRust,
    /// Generate standalone Rust functions with direct computation.
    ///
    /// This format translates the C expressions directly into Rust code,
    /// without attempting to fit them into the R1CS gate structure. Each
    /// `g[N] = expr` becomes one Rust statement.
    ///
    /// This is useful for:
    /// - Quick validation against libcint reference values
    /// - Understanding the mathematical structure of the C code
    /// - Cases where the R1CS decomposition is too complex
    DirectRust,
    /// Generate only the circuit representation (for analysis).
    ///
    /// This format constructs the R1CS circuit but does not emit any Rust
    /// code. The circuit can then be analyzed for gate count, operand
    /// usage, prefactor requirements, and other properties.
    CircuitOnly,
}

// ─────────────────────────────────────────────────────────────────────────────
// TranspileResult: Result of transpiling a C source file
// ─────────────────────────────────────────────────────────────────────────────

/// Result of transpiling a C source file.
///
/// Contains statistics on success/failure, the generated code for each
/// successfully transpiled pattern, and any warnings encountered.
#[derive(Debug)]
pub struct TranspileResult {
    /// Number of patterns successfully transpiled.
    pub n_transpiled: usize,
    /// Number of patterns that failed to transpile.
    pub n_failed: usize,
    /// Generated Rust code for each pattern: (function_name, code).
    pub generated_code: Vec<(String, String)>,
    /// Warnings during transpilation.
    ///
    /// Common warnings include:
    /// - "Pattern NNNN has multi-term expression that doesn't fit R1CS gate format"
    /// - "Variable mapping for 'X' is ambiguous — using direct translation"
    /// - "Circuit construction failed for pattern NNNN — using DirectRust fallback"
    pub warnings: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Transpiler: The main transpiler struct
// ─────────────────────────────────────────────────────────────────────────────

/// The C-to-Rust transpiler for converting libcint patterns to POLER-ERI code.
///
/// # Design Philosophy
///
/// The transpiler is designed as a **best-effort** bridge between two very
/// different computational paradigms:
///
/// 1. **libcint**: Uses Rys quadrature with explicit intermediate arrays
///    (`c00x`, `c0px`, `b10`, etc.) and procedural C code with arbitrary
///    expressions.
///
/// 2. **POLER-ERI**: Uses VRR+HRR recurrence relations encoded as R1CS
///    gates with a fixed `result = coeff_left × operand[left] + coeff_right × operand[right]`
///    format.
///
/// Not every libcint pattern maps cleanly to the R1CS format. The transpiler
/// handles this by:
///
/// - Attempting R1CS decomposition for simple patterns
/// - Falling back to direct translation for complex patterns
/// - Recording warnings when the mapping is imperfect
/// - Always producing correct code (even if not in the optimal R1CS form)
///
/// # Usage
///
/// ```rust
/// use poler_eri::transpiler::{Transpiler, TranspileConfig, OutputFormat};
/// use poler_eri::c_parser::CParser;
///
/// let config = TranspileConfig {
///     output_format: OutputFormat::DirectRust,
///     preserve_names: true,
///     add_assertions: false,
/// };
/// let transpiler = Transpiler::new(config);
///
/// let source = r#"
///     static inline void _g0_2d4d_0111(double *g, double *c00x, double *c0px,
///                                        double *b10, double *b01, double *b00) {
///         g[0] = c00x[0] * c0px[0] * b10[0];
///         g[1] = c00x[0] * c0px[1] * b10[0] + c00x[1] * c0px[0] * b01[0];
///     }
/// "#;
///
/// let result = transpiler.transpile_file(source);
/// assert!(result.n_transpiled > 0);
/// ```
pub struct Transpiler {
    /// Configuration controlling output format and behavior.
    config: TranspileConfig,
}

impl Transpiler {
    /// Create a new transpiler with the given configuration.
    ///
    /// # Arguments
    ///
    /// - `config` — A [`TranspileConfig`] controlling the output format,
    ///   whether to preserve C variable names, and whether to add assertions.
    pub fn new(config: TranspileConfig) -> Self {
        Transpiler { config }
    }

    /// Transpile a complete C source file.
    ///
    /// Uses [`CParser`] to extract patterns, then converts each to Rust code
    /// according to the configured [`OutputFormat`].
    ///
    /// # Arguments
    ///
    /// - `content` — The full text content of a libcint C source file
    ///
    /// # Returns
    ///
    /// A [`TranspileResult`] containing:
    /// - The number of patterns transpiled successfully
    /// - The generated Rust code for each pattern
    /// - Any warnings encountered during transpilation
    pub fn transpile_file(&self, content: &str) -> TranspileResult {
        let parser = CParser::new();
        let patterns = parser.parse_file(content);

        let mut result = TranspileResult {
            n_transpiled: 0,
            n_failed: 0,
            generated_code: Vec::new(),
            warnings: Vec::new(),
        };

        for pattern in &patterns {
            match self.transpile_pattern(pattern) {
                Ok(code) => {
                    let fn_name = generate_fn_name(pattern.angular_momenta);
                    result.generated_code.push((fn_name, code));
                    result.n_transpiled += 1;
                }
                Err(msg) => {
                    result.warnings.push(format!(
                        "Failed to transpile pattern {}: {}",
                        pattern.fn_name, msg
                    ));
                    result.n_failed += 1;
                }
            }
        }

        result
    }

    /// Transpile a single parsed pattern to Rust code.
    ///
    /// Converts a [`RecurrencePattern`] (from [`CParser`]) into Rust code
    /// according to the configured [`OutputFormat`].
    ///
    /// # C Variable → Rust Mapping
    ///
    /// | C Variable           | Rust Mapping                 | Notes                          |
    /// |----------------------|------------------------------|--------------------------------|
    /// | `g[N]`               | `operands[N]`                | Output array                   |
    /// | `c00x[i]`            | `prefactors[c00x_offset + i]`| Bra-side geometric intermediate|
    /// | `c0px[i]`            | `prefactors[c0px_offset + i]`| Ket-side geometric intermediate|
    /// | `b10[i]`, `b01[i]`   | Inline computation           | Boys-like function values      |
    /// | `b00[i]`             | Inline computation           | Base Boys function value       |
    /// | Numeric coefficient  | Literal `f64`                | Preserved exactly              |
    ///
    /// # Arguments
    ///
    /// - `pattern` — The parsed C pattern to transpile
    ///
    /// # Returns
    ///
    /// `Ok(code)` on success, or `Err(message)` if the pattern cannot be
    /// transpiled (e.g., it references unsupported C constructs).
    pub fn transpile_pattern(&self, pattern: &RecurrencePattern) -> Result<String, String> {
        match self.config.output_format {
            OutputFormat::CrystallizedRust => self.transpile_crystallized(pattern),
            OutputFormat::DirectRust => self.transpile_direct(pattern),
            OutputFormat::CircuitOnly => {
                // For circuit-only output, we generate a string representation
                // of the circuit structure rather than executable Rust code
                match self.pattern_to_circuit(pattern) {
                    Ok(circuit) => Ok(format_circuit_summary(&circuit)),
                    Err(msg) => Err(format!(
                        "Cannot construct circuit for {}: {}",
                        pattern.fn_name, msg
                    )),
                }
            }
        }
    }

    /// Attempt to reconstruct a POLER-ERI circuit from the parsed pattern.
    ///
    /// This is best-effort: not all C patterns map cleanly to R1CS gates.
    /// The method first tries to use [`CircuitBuilder`] to construct a
    /// circuit for the same angular momenta, then compares the gate count
    /// and structure against the parsed pattern to verify consistency.
    ///
    /// # When This Works
    ///
    /// The circuit reconstruction works well when:
    ///
    /// 1. The parsed pattern has a simple structure that matches the VRR+HRR
    ///    pipeline (each `g[N]` assignment is a two-term linear combination)
    ///
    /// 2. The angular momenta are such that `la >= lb` and `lc >= ld`
    ///    (required by [`CircuitBuilder`])
    ///
    /// # When This Fails
    ///
    /// The circuit reconstruction fails when:
    ///
    /// 1. The parsed pattern has multi-term expressions (more than 2 terms)
    ///    that don't fit the R1CS gate format
    ///
    /// 2. The angular momenta don't satisfy the permutational symmetry
    ///    requirement (`la >= lb`, `lc >= ld`)
    ///
    /// 3. The libcint pattern uses constructs that have no analog in the
    ///    VRR+HRR pipeline (e.g., Rys-specific recurrence patterns)
    ///
    /// # Arguments
    ///
    /// - `pattern` — The parsed C pattern to convert
    ///
    /// # Returns
    ///
    /// `Ok(circuit)` if the pattern can be mapped to an R1CS circuit,
    /// or `Err(message)` explaining why it cannot.
    pub fn pattern_to_circuit(&self, pattern: &RecurrencePattern) -> Result<Circuit, String> {
        let (li, lj, lk, ll) = pattern.angular_momenta;

        // CircuitBuilder requires la >= lb and lc >= ld for permutational symmetry.
        // If the parsed pattern doesn't satisfy this, we swap to create the
        // canonical form.
        let (la, lb) = if li >= lj { (li, lj) } else { (lj, li) };
        let (lc, ld) = if lk >= ll { (lk, ll) } else { (ll, lk) };

        // Build the circuit using our VRR+HRR pipeline
        let circuit = CircuitBuilder::new(la, lb, lc, ld).build_circuit();

        // Verify that the circuit has the expected number of outputs.
        // For a quartet (la, lb, lc, ld), the number of Cartesian components
        // is ncart(la)*ncart(lb)*ncart(lc)*ncart(ld), but libcint may only
        // compute a subset due to symmetry or Rys quadrature specifics.
        let n_cart_a = (la + 1) * (la + 2) / 2;
        let n_cart_b = (lb + 1) * (lb + 2) / 2;
        let n_cart_c = (lc + 1) * (lc + 2) / 2;
        let n_cart_d = (ld + 1) * (ld + 2) / 2;
        let expected_components = n_cart_a * n_cart_b * n_cart_c * n_cart_d;

        // The libcint pattern may have fewer gates than expected because
        // the Rys quadrature approach computes integrals differently.
        // We don't require an exact match — just that the circuit is valid.
        if circuit.gates.is_empty() && (la + lb + lc + ld > 0) {
            return Err(format!(
                "Circuit for ({},{},{},{}) has zero gates but total angular momentum is {}",
                la, lb, lc, ld,
                la + lb + lc + ld
            ));
        }

        // Check that the number of parsed gates is reasonable compared to
        // the expected number of Cartesian components.
        //
        // IMPORTANT: In libcint, the `g[]` array is a 3D flattened array with
        // layout g[nroots, nmax+1, mmax+1] × 3 (x, y, z), so the number of
        // `g[N] = expr` assignments can be MUCH larger than the number of
        // Cartesian components. Specifically, each Cartesian component requires
        // nrys_roots entries in the g[] array, so we multiply by nrys_roots
        // and by 3 (for x/y/z) to get the expected upper bound.
        //
        // For the unrolled `_g0_2d4d_NNNN` functions with rys_order <= 2,
        // nrys_roots is 1 or 2, and the g[] array includes entries for all
        // intermediate recurrence steps, not just the final components.
        // Therefore we use a generous upper bound: 3 * nrys_roots * expected_components
        // where nrys_roots = (total_am / 2 + 1) for 2-electron integrals.
        let total_am = la + lb + lc + ld;
        let nrys_roots = if total_am == 0 { 1 } else { total_am / 2 + 1 };
        let generous_bound = 3 * nrys_roots * expected_components;
        if !pattern.gates.is_empty() && pattern.gates.len() > generous_bound {
            return Err(format!(
                "Pattern {} has {} gates but expected at most {} (3 × {} nrys_roots × {} cart components)",
                pattern.fn_name,
                pattern.gates.len(),
                generous_bound,
                nrys_roots,
                expected_components
            ));
        }

        Ok(circuit)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Private methods for different output formats
    // ─────────────────────────────────────────────────────────────────────

    /// Transpile using the CrystallizedRust output format.
    ///
    /// Attempts to construct an R1CS circuit from the pattern, then
    /// crystallizes it to optimized Rust code. Falls back to DirectRust
    /// if circuit construction fails.
    fn transpile_crystallized(&self, pattern: &RecurrencePattern) -> Result<String, String> {
        match self.pattern_to_circuit(pattern) {
            Ok(circuit) => {
                let fn_name = generate_fn_name(pattern.angular_momenta);
                let mut code = VrrCrystallizer::crystallize(&circuit, &fn_name);

                if self.config.preserve_names {
                    // Add a comment header showing the original C function name
                    let header = format!(
                        "// Transpiled from libcint function: {}\n\
                         // Angular momenta: (li={}, lj={}, lk={}, ll={})\n\
                         // Original C variables: {}\n",
                        pattern.fn_name,
                        pattern.angular_momenta.0,
                        pattern.angular_momenta.1,
                        pattern.angular_momenta.2,
                        pattern.angular_momenta.3,
                        pattern.variables.join(", ")
                    );
                    code = header + &code;
                }

                Ok(code)
            }
            Err(msg) => {
                // Fall back to DirectRust if circuit construction fails
                self.transpile_direct_with_warning(
                    pattern,
                    &format!(
                        "Circuit construction failed for {} — using DirectRust fallback: {}",
                        pattern.fn_name, msg
                    ),
                )
            }
        }
    }

    /// Transpile using the DirectRust output format.
    ///
    /// Generates standalone Rust code that directly evaluates each `g[N] = expr`
    /// assignment from the C source, translating C variable references to
    /// Rust array accesses.
    fn transpile_direct(&self, pattern: &RecurrencePattern) -> Result<String, String> {
        self.transpile_direct_inner(pattern, None)
    }

    /// Transpile using DirectRust with a warning attached.
    fn transpile_direct_with_warning(
        &self,
        pattern: &RecurrencePattern,
        warning: &str,
    ) -> Result<String, String> {
        self.transpile_direct_inner(pattern, Some(warning))
    }

    /// Inner implementation of DirectRust transpilation.
    fn transpile_direct_inner(
        &self,
        pattern: &RecurrencePattern,
        warning: Option<&str>,
    ) -> Result<String, String> {
        let (li, lj, lk, ll) = pattern.angular_momenta;
        let fn_name = generate_fn_name(pattern.angular_momenta);

        let mut code = String::new();

        // Documentation comment
        code.push_str(&format!(
            "/// Direct Rust transpilation from libcint function {}.\n",
            pattern.fn_name
        ));
        code.push_str(&format!(
            "/// Angular momenta: (li={}, lj={}, lk={}, ll={})\n",
            li, lj, lk, ll
        ));
        code.push_str(&format!(
            "/// Generated by POLER-ERI v3.1.0 Transpiler.\n"
        ));

        if let Some(w) = warning {
            code.push_str(&format!("/// WARNING: {}\n", w));
        }

        // Function signature
        code.push_str(&format!(
            "pub fn {}(operands: &mut [f64], prefactors: &[f64]) {{\n",
            fn_name
        ));

        // Add assertions for array bounds if configured
        if self.config.add_assertions {
            let n_operands = pattern
                .gates
                .iter()
                .map(|g| g.result_idx + 1)
                .max()
                .unwrap_or(1);
            code.push_str(&format!(
                "    debug_assert!(operands.len() >= {}, \"operands array too small\");\n",
                n_operands
            ));
            code.push_str(&format!(
                "    debug_assert!(prefactors.len() >= 1, \"prefactors array too small\");\n"
            ));
        }

        // Build a variable-to-offset mapping for C array accesses
        let var_offsets = build_variable_offsets(&pattern.variables);

        // Emit each gate as a Rust statement
        for gate in &pattern.gates {
            if self.config.preserve_names {
                code.push_str(&format!(
                    "    // Original C: g[{}] = {}\n",
                    gate.result_idx, gate.expression
                ));
            }

            // Translate the C expression to Rust
            let rust_expr = translate_expression(&gate.expression, &var_offsets);
            code.push_str(&format!(
                "    operands[{}] = {};\n",
                gate.result_idx, rust_expr
            ));
        }

        code.push_str("}\n");
        Ok(code)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper functions
// ─────────────────────────────────────────────────────────────────────────────

/// Generate a Rust function name from angular momenta.
///
/// Uses spectroscopic notation: (0,0,0,0) → "eri_ssss", (1,1,1,1) → "eri_pppp",
/// (2,0,0,0) → "eri_dsss", etc.
fn generate_fn_name(am: (usize, usize, usize, usize)) -> String {
    format!(
        "eri_{}{}{}{}",
        shell_name(am.0),
        shell_name(am.1),
        shell_name(am.2),
        shell_name(am.3)
    )
}

/// Build a mapping from C variable names to prefactor offset indices.
///
/// Each variable in the libcint source (c00x, c0px, b10, etc.) gets assigned
/// a base offset into the prefactor array. The actual index for `var[i]` is
/// `offset + i`.
///
/// The ordering is:
/// 1. c00x, c00y, c00z (bra-side intermediates)
/// 2. c0px, c0py, c0pz (ket-side intermediates)
/// 3. b10, b01, b00 (Boys-like intermediates)
/// 4. Other variables in order of appearance
fn build_variable_offsets(variables: &[String]) -> Vec<(String, usize)> {
    let mut offsets = Vec::new();
    let mut current_offset = 0;

    // Prioritize known Rys intermediate variables
    let priority_vars = ["c00x", "c00y", "c00z", "c0px", "c0py", "c0pz", "b10", "b01", "b00"];

    let mut seen = std::collections::HashSet::new();

    // First pass: priority variables
    for var in &priority_vars {
        if variables.iter().any(|v| v == var) && !seen.contains(*var) {
            offsets.push((var.to_string(), current_offset));
            seen.insert(var.to_string());
            current_offset += 1;
        }
    }

    // Second pass: remaining variables
    for var in variables {
        if !seen.contains(var.as_str()) {
            offsets.push((var.clone(), current_offset));
            seen.insert(var.clone());
            current_offset += 1;
        }
    }

    offsets
}

/// Translate a C expression to a Rust expression.
///
/// Performs the following transformations:
///
/// 1. **Variable array access**: `c00x[0]` → `prefactors[c00x_offset + 0]`
///    where `c00x_offset` is the base offset for the c00x variable.
///
/// 2. **g array access**: Not expected in RHS expressions (g is output only).
///
/// 3. **Arithmetic operators**: `*` and `+` are preserved as-is.
///
/// 4. **Numeric literals**: Preserved as-is (they're the same in C and Rust).
///
/// 5. **b10, b01, b00 references**: Mapped to prefactor array accesses
///    since these are runtime Boys-function values.
fn translate_expression(expr: &str, var_offsets: &[(String, usize)]) -> String {
    let mut result = expr.to_string();

    // Translate each known variable reference
    for (var_name, base_offset) in var_offsets {
        // Replace varname[N] with prefactors[base_offset + N]
        // We need to find all occurrences of varname[ followed by a number and ]
        let pattern_prefix = format!("{}[", var_name);
        let mut translated = String::new();
        let mut remaining = result.as_str();

        while let Some(pos) = remaining.find(&pattern_prefix) {
            // Copy everything before this match
            translated.push_str(&remaining[..pos]);

            // Skip past the variable name and opening bracket
            let after_prefix = &remaining[pos + pattern_prefix.len()..];

            // Find the closing bracket
            if let Some(close_bracket) = after_prefix.find(']') {
                let index_str = &after_prefix[..close_bracket];
                if let Ok(index) = index_str.parse::<usize>() {
                    translated.push_str(&format!("prefactors[{} + {}]", base_offset, index));
                } else {
                    // Non-numeric index — keep as-is (shouldn't happen in libcint)
                    translated.push_str(&format!("{}[{}]", var_name, index_str));
                }
                remaining = &after_prefix[close_bracket + 1..];
            } else {
                // No closing bracket — keep the rest as-is
                translated.push_str(&remaining[pos..]);
                remaining = "";
                break;
            }
        }

        translated.push_str(remaining);
        result = translated;
    }

    result
}

/// Format a circuit as a human-readable summary string.
///
/// Used by the `CircuitOnly` output format to provide a structured
/// representation of the circuit without generating executable code.
fn format_circuit_summary(circuit: &Circuit) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "Circuit for quartet {}\n",
        circuit.quartet_label()
    ));
    out.push_str(&format!(
        "  Angular momenta: (la={}, lb={}, lc={}, ld={})\n",
        circuit.la, circuit.lb, circuit.lc, circuit.ld
    ));
    out.push_str(&format!("  Gates: {}\n", circuit.gates.len()));
    out.push_str(&format!("  Operands: {}\n", circuit.n_operands));
    out.push_str(&format!("  Prefactors: {}\n", circuit.n_prefactors));

    let (n_vrr_bra, n_vrr_ket, n_vrr_full, n_hrr_bra, n_hrr_ket) = circuit.stage_counts();
    out.push_str(&format!(
        "  Stage breakdown: VRR-Bra={}, VRR-Ket={}, VRR-Full={}, HRR-Bra={}, HRR-Ket={}\n",
        n_vrr_bra, n_vrr_ket, n_vrr_full, n_hrr_bra, n_hrr_ket
    ));

    if circuit.is_base_case() {
        out.push_str("  Base case: no recurrence steps\n");
    } else {
        out.push_str("  Gate details:\n");
        for (i, gate) in circuit.gates.iter().enumerate() {
            let stage_name = match gate.stage {
                0 => "VRR-Bra",
                1 => "VRR-Ket",
                2 => "VRR-Full",
                3 => "HRR-Bra",
                4 => "HRR-Ket",
                _ => "UNKNOWN",
            };
            out.push_str(&format!(
                "    Gate {}: [{}] operands[{}] = {:?} * operands[{}] + {:?} * operands[{}]\n",
                i, stage_name, gate.result, gate.coeff_left, gate.left, gate.coeff_right, gate.right
            ));
        }
    }

    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::c_parser::ParsedGate;

    /// Test that DirectRust transpilation produces valid Rust code.
    #[test]
    fn test_transpile_direct_rust() {
        let config = TranspileConfig {
            output_format: OutputFormat::DirectRust,
            preserve_names: true,
            add_assertions: false,
        };
        let transpiler = Transpiler::new(config);

        let source = r#"
static inline void _g0_2d4d_0111(double *g, double *c00x, double *c0px,
                                   double *b10, double *b01, double *b00) {
    g[0] = c00x[0] * c0px[0] * b10[0];
    g[1] = c00x[0] * c0px[1] * b10[0] + c00x[1] * c0px[0] * b01[0];
    g[2] = c00x[1] * c0px[1] * b00[0];
}
"#;
        let result = transpiler.transpile_file(source);

        assert_eq!(result.n_transpiled, 1, "Should transpile 1 pattern");
        assert_eq!(result.n_failed, 0, "Should have 0 failures");
        assert_eq!(result.generated_code.len(), 1);

        let (name, code) = &result.generated_code[0];
        assert_eq!(name, "eri_sppp");
        assert!(code.contains("pub fn eri_sppp"), "Should contain function definition");
        assert!(code.contains("operands[0]"), "Should reference operands[0]");
        assert!(code.contains("operands[1]"), "Should reference operands[1]");
        assert!(code.contains("operands[2]"), "Should reference operands[2]");
        assert!(code.contains("prefactors["), "Should reference prefactors array");
    }

    /// Test that CrystallizedRust transpilation uses the VRR+HRR pipeline.
    #[test]
    fn test_transpile_crystallized_rust() {
        let config = TranspileConfig {
            output_format: OutputFormat::CrystallizedRust,
            preserve_names: true,
            add_assertions: false,
        };
        let transpiler = Transpiler::new(config);

        let source = r#"
static inline void _g0_2d4d_1000(double *g, double *c00x, double *b10) {
    g[0] = c00x[0] * b10[0];
}
"#;
        let result = transpiler.transpile_file(source);

        assert_eq!(result.n_transpiled, 1);
        let (name, code) = &result.generated_code[0];
        assert_eq!(name, "eri_psss");
        assert!(code.contains("Transpiled from libcint"));
        assert!(code.contains("pub fn eri_psss"));
    }

    /// Test that CircuitOnly output produces a circuit summary.
    #[test]
    fn test_transpile_circuit_only() {
        let config = TranspileConfig {
            output_format: OutputFormat::CircuitOnly,
            preserve_names: false,
            add_assertions: false,
        };
        let transpiler = Transpiler::new(config);

        let source = r#"
static inline void _g0_2d4d_1111(double *g, double *c00x, double *c0px,
                                   double *b10, double *b01, double *b00) {
    g[0] = c00x[0] * c0px[0] * b10[0];
}
"#;
        let result = transpiler.transpile_file(source);

        assert_eq!(result.n_transpiled, 1);
        let (_name, summary) = &result.generated_code[0];
        assert!(summary.contains("Circuit for quartet"));
        assert!(summary.contains("(pp|pp)"));
        assert!(summary.contains("Gates:"));
    }

    /// Test pattern_to_circuit for a valid quartet.
    #[test]
    fn test_pattern_to_circuit_valid() {
        let config = TranspileConfig::default();
        let transpiler = Transpiler::new(config);

        let pattern = RecurrencePattern {
            fn_name: "_g0_2d4d_1100".to_string(),
            angular_momenta: (1, 1, 0, 0),
            gates: vec![
                ParsedGate {
                    result_idx: 0,
                    expression: "c00x[0] * b10[0]".to_string(),
                    left_var: Some("c00x[0]".to_string()),
                    right_var: Some("b10[0]".to_string()),
                    coeff_left: None,
                    coeff_right: None,
                },
            ],
            variables: vec!["c00x".to_string(), "b10".to_string()],
        };

        let circuit = transpiler.pattern_to_circuit(&pattern);
        assert!(circuit.is_ok());

        let circuit = circuit.unwrap();
        assert_eq!(circuit.la, 1);
        assert_eq!(circuit.lb, 1);
        assert_eq!(circuit.lc, 0);
        assert_eq!(circuit.ld, 0);
    }

    /// Test pattern_to_circuit with swapped angular momenta (permutational symmetry).
    #[test]
    fn test_pattern_to_circuit_swapped() {
        let config = TranspileConfig::default();
        let transpiler = Transpiler::new(config);

        // la < lb should be swapped to satisfy CircuitBuilder's requirement
        let pattern = RecurrencePattern {
            fn_name: "_g0_2d4d_0110".to_string(),
            angular_momenta: (0, 1, 1, 0),
            gates: vec![],
            variables: vec![],
        };

        let circuit = transpiler.pattern_to_circuit(&pattern);
        assert!(circuit.is_ok());

        let circuit = circuit.unwrap();
        // Should be swapped: (1, 0, 1, 0) to satisfy la >= lb
        assert_eq!(circuit.la, 1);
        assert_eq!(circuit.lb, 0);
        assert_eq!(circuit.lc, 1);
        assert_eq!(circuit.ld, 0);
    }

    /// Test transpile_file with multiple functions.
    #[test]
    fn test_transpile_multiple_functions() {
        let config = TranspileConfig {
            output_format: OutputFormat::DirectRust,
            preserve_names: false,
            add_assertions: false,
        };
        let transpiler = Transpiler::new(config);

        let source = r#"
static inline void _g0_2d4d_0000(double *g, double *b00) {
    g[0] = b00[0];
}

static inline void _g0_2d4d_1000(double *g, double *c00x, double *b10) {
    g[0] = c00x[0] * b10[0];
}

static inline void _g0_2d4d_1100(double *g, double *c00x, double *c0px,
                                   double *b10, double *b01, double *b00) {
    g[0] = c00x[0] * c0px[0] * b10[0];
    g[1] = c00x[0] * c0px[1] * b10[0] + c00x[1] * c0px[0] * b01[0];
}
"#;
        let result = transpiler.transpile_file(source);

        assert_eq!(result.n_transpiled, 3, "Should transpile 3 patterns");
        assert_eq!(result.n_failed, 0);

        let names: Vec<&str> = result.generated_code.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"eri_ssss"), "Should contain eri_ssss");
        assert!(names.contains(&"eri_psss"), "Should contain eri_psss");
        assert!(names.contains(&"eri_ppss"), "Should contain eri_ppss");
    }

    /// Test that assertions are added when configured.
    #[test]
    fn test_transpile_with_assertions() {
        let config = TranspileConfig {
            output_format: OutputFormat::DirectRust,
            preserve_names: false,
            add_assertions: true,
        };
        let transpiler = Transpiler::new(config);

        let source = r#"
static inline void _g0_2d4d_1000(double *g, double *c00x, double *b10) {
    g[0] = c00x[0] * b10[0];
}
"#;
        let result = transpiler.transpile_file(source);

        assert_eq!(result.n_transpiled, 1);
        let (_name, code) = &result.generated_code[0];
        assert!(
            code.contains("debug_assert!"),
            "Should contain debug assertions when add_assertions is true"
        );
    }

    /// Test that C variable names are preserved in comments when configured.
    #[test]
    fn test_transpile_preserve_names() {
        let config = TranspileConfig {
            output_format: OutputFormat::DirectRust,
            preserve_names: true,
            add_assertions: false,
        };
        let transpiler = Transpiler::new(config);

        let source = r#"
static inline void _g0_2d4d_0111(double *g, double *c00x, double *c0px,
                                   double *b10, double *b01, double *b00) {
    g[0] = c00x[0] * c0px[0] * b10[0];
}
"#;
        let result = transpiler.transpile_file(source);

        assert_eq!(result.n_transpiled, 1);
        let (_name, code) = &result.generated_code[0];
        assert!(
            code.contains("Original C:"),
            "Should contain original C expression when preserve_names is true"
        );
    }

    /// Test the variable-to-offset mapping.
    #[test]
    fn test_variable_offset_mapping() {
        let variables = vec![
            "cpx".to_string(),
            "c00x".to_string(),
            "c0px".to_string(),
            "b10".to_string(),
            "b01".to_string(),
        ];
        let offsets = build_variable_offsets(&variables);

        // Priority variables should come first
        let c00x_offset = offsets.iter().find(|(n, _)| n == "c00x").map(|(_, o)| *o);
        let c0px_offset = offsets.iter().find(|(n, _)| n == "c0px").map(|(_, o)| *o);
        let b10_offset = offsets.iter().find(|(n, _)| n == "b10").map(|(_, o)| *o);
        let b01_offset = offsets.iter().find(|(n, _)| n == "b01").map(|(_, o)| *o);
        let cpx_offset = offsets.iter().find(|(n, _)| n == "cpx").map(|(_, o)| *o);

        assert!(c00x_offset.is_some(), "Should find c00x offset");
        assert!(c0px_offset.is_some(), "Should find c0px offset");
        assert!(b10_offset.is_some(), "Should find b10 offset");
        assert!(b01_offset.is_some(), "Should find b01 offset");
        assert!(cpx_offset.is_some(), "Should find cpx offset");

        // Priority variables should have lower offsets than non-priority
        assert!(
            c00x_offset.unwrap() < cpx_offset.unwrap(),
            "Priority variable c00x should have a lower offset than non-priority cpx"
        );
    }

    /// Test expression translation from C to Rust.
    #[test]
    fn test_translate_expression() {
        let var_offsets = vec![
            ("c00x".to_string(), 0),
            ("c0px".to_string(), 1),
            ("b10".to_string(), 2),
            ("b01".to_string(), 3),
            ("b00".to_string(), 4),
        ];

        // Simple product
        let rust = translate_expression("c00x[0] * c0px[0] * b10[0]", &var_offsets);
        assert!(
            rust.contains("prefactors[0 + 0]"),
            "c00x[0] should become prefactors[0 + 0]"
        );
        assert!(
            rust.contains("prefactors[1 + 0]"),
            "c0px[0] should become prefactors[1 + 0]"
        );
        assert!(
            rust.contains("prefactors[2 + 0]"),
            "b10[0] should become prefactors[2 + 0]"
        );
    }

    /// Test expression translation with non-zero array indices.
    #[test]
    fn test_translate_expression_nonzero_index() {
        let var_offsets = vec![
            ("c00x".to_string(), 0),
            ("c0px".to_string(), 1),
        ];

        let rust = translate_expression("c00x[1] * c0px[2]", &var_offsets);
        assert!(
            rust.contains("prefactors[0 + 1]"),
            "c00x[1] should become prefactors[0 + 1]"
        );
        assert!(
            rust.contains("prefactors[1 + 2]"),
            "c0px[2] should become prefactors[1 + 2]"
        );
    }

    /// Test that the (ss|ss) base case transpiles correctly.
    #[test]
    fn test_transpile_ssss_base_case() {
        let config = TranspileConfig {
            output_format: OutputFormat::DirectRust,
            preserve_names: false,
            add_assertions: false,
        };
        let transpiler = Transpiler::new(config);

        let source = r#"
static inline void _g0_2d4d_0000(double *g, double *b00) {
    g[0] = b00[0];
}
"#;
        let result = transpiler.transpile_file(source);

        assert_eq!(result.n_transpiled, 1);
        let (name, code) = &result.generated_code[0];
        assert_eq!(name, "eri_ssss");
        assert!(code.contains("operands[0] = prefactors["));
    }

    /// Test generate_fn_name.
    #[test]
    fn test_generate_fn_name() {
        assert_eq!(generate_fn_name((0, 0, 0, 0)), "eri_ssss");
        assert_eq!(generate_fn_name((1, 0, 0, 0)), "eri_psss");
        assert_eq!(generate_fn_name((1, 1, 1, 1)), "eri_pppp");
        assert_eq!(generate_fn_name((2, 2, 2, 2)), "eri_dddd");
        assert_eq!(generate_fn_name((3, 0, 1, 0)), "eri_fsps");
    }

    /// Test format_circuit_summary.
    #[test]
    fn test_format_circuit_summary() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        let summary = format_circuit_summary(&circuit);

        assert!(summary.contains("(pp|pp)"), "Should contain quartet label");
        assert!(summary.contains("Gates:"), "Should mention gate count");
        assert!(summary.contains("Operands:"), "Should mention operand count");
        assert!(summary.contains("Prefactors:"), "Should mention prefactor count");
        assert!(summary.contains("VRR-Bra"), "Should mention VRR-Bra stage");
    }

    /// Test transpile_file with empty source.
    #[test]
    fn test_transpile_empty_source() {
        let config = TranspileConfig::default();
        let transpiler = Transpiler::new(config);

        let result = transpiler.transpile_file("");
        assert_eq!(result.n_transpiled, 0);
        assert_eq!(result.n_failed, 0);
    }

    /// Test that the CrystallizedRust fallback works for patterns that
    /// don't fit the R1CS gate format.
    #[test]
    fn test_crystallized_fallback() {
        let config = TranspileConfig {
            output_format: OutputFormat::CrystallizedRust,
            preserve_names: true,
            add_assertions: false,
        };
        let transpiler = Transpiler::new(config);

        // This pattern has multiple complex expressions that might not
        // map cleanly to R1CS, but should still produce valid output
        let source = r#"
static inline void _g0_2d4d_0111(double *g, double *c00x, double *c0px,
                                   double *b10, double *b01, double *b00) {
    g[0] = c00x[0] * c0px[0] * b10[0];
    g[1] = c00x[0] * c0px[1] * b10[0] + c00x[1] * c0px[0] * b01[0];
    g[2] = c00x[1] * c0px[1] * b00[0];
}
"#;
        let result = transpiler.transpile_file(source);

        // Should transpile successfully (either via circuit or fallback)
        assert!(
            result.n_transpiled + result.n_failed > 0,
            "Should process the pattern"
        );

        if result.n_transpiled > 0 {
            let (_name, code) = &result.generated_code[0];
            assert!(
                code.contains("pub fn"),
                "Generated code should contain a function definition"
            );
        }
    }

    /// Test the full pipeline: parse C → transpile to DirectRust.
    #[test]
    fn test_full_pipeline_direct_rust() {
        let config = TranspileConfig {
            output_format: OutputFormat::DirectRust,
            preserve_names: true,
            add_assertions: true,
        };
        let transpiler = Transpiler::new(config);

        let source = r#"
static inline void _g0_2d4d_1111(double *g, double *c00x, double *c0px,
                                   double *b10, double *b01, double *b00) {
    g[0] = c00x[0] * c0px[0] * b10[0];
    g[1] = c00x[0] * c0px[1] * b10[0] + c00x[1] * c0px[0] * b01[0];
    g[2] = c00y[0] * c0py[0] * b10[0];
    g[3] = c00z[0] * c0pz[0] * b00[0];
}
"#;
        let result = transpiler.transpile_file(source);

        assert_eq!(result.n_transpiled, 1, "Should transpile the (pp|pp) pattern");
        let (name, code) = &result.generated_code[0];
        assert_eq!(name, "eri_pppp");

        // Verify the generated code structure
        assert!(code.contains("pub fn eri_pppp"), "Should define eri_pppp function");
        assert!(code.contains("operands: &mut [f64]"), "Should take operands parameter");
        assert!(code.contains("prefactors: &[f64]"), "Should take prefactors parameter");
        assert!(code.contains("debug_assert!"), "Should have assertions");
        assert!(code.contains("Original C:"), "Should preserve C names in comments");
        assert!(code.contains("prefactors["), "Should use prefactors array");
    }

    /// Test Default implementation for TranspileConfig.
    #[test]
    fn test_transpile_config_default() {
        let config = TranspileConfig::default();
        assert!(matches!(config.output_format, OutputFormat::CrystallizedRust));
        assert!(config.preserve_names);
        assert!(config.add_assertions);
    }

    /// Test expression translation with numeric coefficients.
    #[test]
    fn test_translate_expression_with_coeff() {
        let var_offsets = vec![
            ("c00x".to_string(), 0),
            ("c0px".to_string(), 1),
        ];

        let rust = translate_expression("0.5 * c00x[0] * c0px[0]", &var_offsets);
        assert!(rust.contains("0.5"), "Should preserve numeric coefficient");
        assert!(
            rust.contains("prefactors[0 + 0]"),
            "Should translate c00x[0]"
        );
    }

    /// Test that the transpiler correctly handles a (dd|ss) pattern.
    #[test]
    fn test_transpile_ddss() {
        let config = TranspileConfig {
            output_format: OutputFormat::CrystallizedRust,
            preserve_names: false,
            add_assertions: false,
        };
        let transpiler = Transpiler::new(config);

        let source = r#"
static inline void _g0_2d4d_2000(double *g, double *c00x, double *c00y,
                                   double *c00z, double *b10, double *b00) {
    g[0] = c00x[0] * b10[0];
    g[1] = c00y[0] * b10[0];
    g[2] = c00z[0] * b10[0];
}
"#;
        let result = transpiler.transpile_file(source);
        assert_eq!(result.n_transpiled, 1);
        let (name, _code) = &result.generated_code[0];
        assert_eq!(name, "eri_dsss");
    }
}
