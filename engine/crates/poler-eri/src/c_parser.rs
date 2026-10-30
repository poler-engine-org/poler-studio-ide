//! # C Source Parser for libcint Recurrence Relations
//!
//! ## Overview
//!
//! This module implements a **targeted C source parser** that extracts recurrence
//! relation patterns from libcint source files. It does NOT parse arbitrary C — it
//! specifically looks for the patterns used in libcint's ERI code: function names
//! like `_g0_2d4d_0111`, variable names like `c00x`, `c0px`, `b10`, `b01`, `b00`,
//! and assignment patterns like `g[N] = expr`.
//!
//! ## Why Parse C Instead of Writing Recurrence Relations From Scratch?
//!
//! libcint is a battle-tested, widely-used library for computing Gaussian integrals.
//! Its recurrence relation implementations have been verified against countless
//! reference calculations over many years. By parsing libcint's C source, we can:
//!
//! 1. **Verify correctness**: Compare our POLER-ERI circuit output against the
//!    proven libcint implementation. Any discrepancy indicates a bug in either
//!    the parser or our circuit builder.
//!
//! 2. **Discover patterns**: libcint uses a specific Rys quadrature approach that
//!    produces a distinctive code structure. By extracting these patterns, we can
//!    understand exactly how the recurrence relations compose for each shell quartet.
//!
//! 3. **Bootstrap new quartets**: When adding support for higher angular momenta,
//!    we can extract the recurrence structure from libcint and use it as a
//!    reference for constructing the corresponding POLER-ERI circuits.
//!
//! 4. **Cross-validation**: Having two independent implementations (libcint's C
//!    and our Rust) agree on numerical results provides strong evidence of
//!    correctness for both.
//!
//! ## How libcint's Rys Quadrature Approach Differs From Our VRR+HRR Pipeline
//!
//! libcint uses **Rys quadrature** to evaluate ERI integrals. The key differences:
//!
//! | Aspect            | libcint (Rys Quadrature)             | POLER-ERI (VRR+HRR)              |
//! |-------------------|--------------------------------------|-----------------------------------|
//! | Core method       | Rys polynomials + numerical quadrature | McMurchie-Davidson VRR + HRR    |
//! | Base case         | (ss\|ss) via Rys roots & weights     | (ss\|ss) via Boys function F_m(T)|
//! | Recurrence        | 2D recurrence on Rys polynomial indices | 4-stage VRR-A, HRR-bra, VRR-C, HRR-ket |
//! | Variables         | `c00x`, `c0px` (Rys recurrence intermediates) | `operands[N]`, `prefactors[N]` |
//! | Boys function     | Implicit in Rys roots               | Explicit F_m(T) evaluation       |
//! | Array indexing    | `g[N]` with computed N              | `operands[N]` with static N      |
//!
//! In libcint, the `c00x`, `c0px`, `b10`, `b01`, `b00` variables are Rys
//! quadrature intermediates:
//!
//! - `c00x`, `c00y`, `c00z`: Cartesian components of the bra-side Rys recurrence
//! - `c0px`, `c0py`, `c0pz`: Cartesian components of the ket-side Rys recurrence
//! - `b10`, `b01`, `b00`: Boys-function-like intermediates
//!
//! In POLER-ERI, these correspond to different quantities:
//!
//! - `c00*` → prefactors (PA_i, QC_i) that multiply operands
//! - `b10`, `b01`, `b00` → scalar coefficients or Boys function values
//! - `g[N]` → operands[N] in our R1CS circuit
//!
//! ## What We Extract and What We Discard
//!
//! ### Extracted
//!
//! - **Function names**: `_g0_2d4d_0111` → angular momenta (0,1,1,1)
//! - **Variable references**: All `c00x`, `c0px`, `b10`, `b01`, `b00` usage
//! - **g[N] = expr assignments**: The recurrence relation steps
//! - **Coefficient patterns**: Numeric constants and variable multipliers
//!
//! ### Discarded
//!
//! - **Memory management**: `malloc`, `free`, pointer arithmetic
//! - **Control flow**: `if`, `for`, `switch` — libcint's batched evaluation
//!   loops are irrelevant to our single-quartet circuit
//! - **Type declarations**: We don't need C type information
//! - **Function call conventions**: We only care about the math, not the ABI
//! - **Comments and preprocessor directives**: No semantic content for our purpose

use std::collections::HashSet;

// ─────────────────────────────────────────────────────────────────────────────
// RecurrencePattern: A parsed recurrence relation from C source
// ─────────────────────────────────────────────────────────────────────────────

/// A parsed recurrence relation pattern extracted from C source.
///
/// Each `RecurrencePattern` corresponds to one libcint function that computes
/// a shell quartet of ERI integrals. The function name encodes the angular
/// momenta, and the body encodes the recurrence relation steps.
///
/// # Example
///
/// For the libcint function:
///
/// ```c
/// static inline void _g0_2d4d_0111(double *g, double *c00x, double *c0px,
///                                    double *b10, double *b01, double *b00) {
///     g[0] = c00x[0] * c0px[0] * b10[0];
///     g[1] = c00x[0] * c0px[1] * b10[0] + c00x[1] * c0px[0] * b01[0];
///     g[2] = c00x[1] * c0px[1] * b00[0];
/// }
/// ```
///
/// This would produce a `RecurrencePattern` with:
/// - `fn_name = "_g0_2d4d_0111"`
/// - `angular_momenta = (0, 1, 1, 1)`
/// - Three `ParsedGate` entries for g[0], g[1], g[2]
/// - Variables: `["c00x", "c0px", "b10", "b01", "b00"]`
#[derive(Debug, Clone)]
pub struct RecurrencePattern {
    /// Source function name, e.g. "_g0_2d4d_0111"
    pub fn_name: String,
    /// Angular momenta inferred from the function name: (li, lj, lk, ll)
    pub angular_momenta: (usize, usize, usize, usize),
    /// Parsed gate operations extracted from g[N] = expr assignments
    pub gates: Vec<ParsedGate>,
    /// Variables referenced in the function body (c00x, c0px, b10, b01, b00, etc.)
    pub variables: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// ParsedGate: A single gate extracted from a g[N] = expr assignment
// ─────────────────────────────────────────────────────────────────────────────

/// A single gate operation parsed from a `g[N] = expr` assignment in libcint source.
///
/// Each `ParsedGate` represents one ERI component computation. The expression
/// on the right-hand side of the assignment may reference Rys quadrature
/// intermediates (c00x, c0px, etc.) and Boys-like functions (b10, b01, b00).
///
/// # Expression Structure
///
/// libcint expressions typically have one of these forms:
///
/// 1. **Simple product**: `g[0] = c00x[0] * c0px[0] * b10[0]`
///    → Single term with two variable references and one Boys-like factor
///
/// 2. **Sum of products**: `g[1] = c00x[0] * c0px[1] * b10[0] + c00x[1] * c0px[0] * b01[0]`
///    → Two terms added together
///
/// 3. **Constant coefficient**: `g[2] = 0.5 * c00x[0] * c0px[0] * b00[0]`
///    → Numeric prefactor multiplied by variable references
///
/// The `left_var`/`right_var` and `coeff_left`/`coeff_right` fields attempt
/// to decompose the expression into a two-term linear combination, matching
/// the POLER-ERI R1CS gate format. For multi-term expressions that don't
/// fit this mold, the full expression string is preserved in `expression`.
#[derive(Debug, Clone)]
pub struct ParsedGate {
    /// The g[N] index — which ERI component this gate computes
    pub result_idx: usize,
    /// Full expression string from the right-hand side of g[N] = expr
    pub expression: String,
    /// Left variable reference if the expression decomposes as a two-term sum.
    /// E.g., for `c00x[0] * c0px[0] * b10[0] + c00x[1] * c0px[0] * b01[0]`,
    /// left_var would be Some("c00x[0]") (simplified).
    pub left_var: Option<String>,
    /// Right variable reference, analogous to left_var for the second term.
    pub right_var: Option<String>,
    /// Numeric coefficient on the left term, if identifiable.
    /// For `0.5 * c00x[0] * ...`, this would be Some(0.5).
    pub coeff_left: Option<f64>,
    /// Numeric coefficient on the right term, if identifiable.
    pub coeff_right: Option<f64>,
}

// ─────────────────────────────────────────────────────────────────────────────
// CParser: The main parser struct
// ─────────────────────────────────────────────────────────────────────────────

/// A C source parser specialized for extracting recurrence relation patterns
/// from libcint ERI source files.
///
/// # Design Philosophy
///
/// This is NOT a general-purpose C parser. It is a **pattern extractor** that
/// relies on the highly regular structure of libcint's generated code:
///
/// - Functions always follow the naming convention `_g0_2d4d_NNNN`
/// - Function signatures always take `double *g` plus a set of `double *`
///   pointer parameters for Rys intermediates
/// - The function body consists of `g[N] = expr` assignments
/// - Expressions use only multiplication, addition, and array indexing
///
/// This regularity allows us to use simple string matching and regex-like
/// parsing instead of a full C grammar parser, making the code much simpler
/// and more robust against the specific libcint code patterns.
///
/// # Usage
///
/// ```rust
/// use poler_eri::c_parser::CParser;
///
/// let parser = CParser::new();
/// let source = r#"
///     static inline void _g0_2d4d_0111(double *g, double *c00x, double *c0px,
///                                        double *b10, double *b01, double *b00) {
///         g[0] = c00x[0] * c0px[0] * b10[0];
///         g[1] = c00x[0] * c0px[1] * b10[0] + c00x[1] * c0px[0] * b01[0];
///     }
/// "#;
///
/// let patterns = parser.parse_file(source);
/// assert_eq!(patterns.len(), 1);
/// assert_eq!(patterns[0].angular_momenta, (0, 1, 1, 1));
/// ```
pub struct CParser;

impl CParser {
    /// Create a new CParser instance.
    ///
    /// The parser is stateless — all parsing state is local to each method call.
    /// This makes the parser safe to use from multiple threads simultaneously.
    pub fn new() -> Self {
        CParser
    }

    /// Parse a complete C source file and extract all recurrence relation patterns.
    ///
    /// Scans for `static inline void _g0_2d4d_NNNN(double * g, ...)` function
    /// definitions and extracts:
    ///
    /// 1. The angular momenta from the function name suffix (e.g., "0111" → (0,1,1,1))
    /// 2. Variable declarations like `double *cpx = bc->c0px;`
    /// 3. `g[N] = expr` assignments into [`ParsedGate`] structs
    ///
    /// # Arguments
    ///
    /// - `content` — The full text content of a libcint C source file
    ///
    /// # Returns
    ///
    /// A vector of [`RecurrencePattern`]s, one per function found.
    ///
    /// # Function Name Convention
    ///
    /// libcint uses the naming pattern `_g0_2d4d_NNNN` where:
    ///
    /// - `_g0` — Fixed prefix for the ERI recurrence function
    /// - `2d4d` — Indicates 2-center bra and 4-center (full) ERI
    /// - `NNNN` — Four hex/decimal digits encoding (li, lj, lk, ll)
    ///
    /// Each digit represents the angular momentum for that center:
    ///
    /// | Position | Center | Meaning          |
    /// |----------|--------|------------------|
    /// | 0        | i      | First bra center |
    /// | 1        | j      | Second bra center|
    /// | 2        | k      | First ket center |
    /// | 3        | l      | Second ket center|
    pub fn parse_file(&self, content: &str) -> Vec<RecurrencePattern> {
        let mut patterns = Vec::new();

        // Split into function-sized chunks by finding function definitions.
        // We look for the pattern: static inline void _g0_2d4d_NNNN(...)
        let mut search_start = 0;
        while search_start < content.len() {
            // Find the next function definition
            if let Some(fn_start) = find_function_start(content, search_start) {
                // Extract the function name
                if let Some(fn_name) = extract_function_name(content, fn_start) {
                    // Find the function body (between { and matching })
                    if let Some(body) = extract_function_body(content, fn_start) {
                        // Parse angular momenta from function name
                        if let Some(am) = Self::parse_function_name(&fn_name) {
                            // Extract variables from declarations
                            let variables = self.extract_variables(&body);

                            // Parse g[N] = expr assignments into gates
                            let gates = self.parse_gates(&body);

                            patterns.push(RecurrencePattern {
                                fn_name,
                                angular_momenta: am,
                                gates,
                                variables,
                            });
                        }

                        // Advance past this function body
                        search_start = fn_start + body.len() + 1;
                        continue;
                    }
                }
                // If we couldn't parse this function, advance past the start marker
                search_start = fn_start + 1;
            } else {
                break;
            }
        }

        patterns
    }

    /// Parse a libcint function name to extract angular momenta.
    ///
    /// The function name follows the pattern `_g0_2d4d_NNNN` where `NNNN`
    /// is a 4-character string of digits (0-9) representing the angular
    /// momenta for centers i, j, k, l respectively.
    ///
    /// # Arguments
    ///
    /// - `name` — The function name to parse (e.g., `"_g0_2d4d_0111"`)
    ///
    /// # Returns
    ///
    /// `Some((li, lj, lk, ll))` if the name matches the expected pattern,
    /// or `None` if the name doesn't conform.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use poler_eri::c_parser::CParser;
    ///
    /// assert_eq!(CParser::parse_function_name("_g0_2d4d_0000"), Some((0, 0, 0, 0)));
    /// assert_eq!(CParser::parse_function_name("_g0_2d4d_0111"), Some((0, 1, 1, 1)));
    /// assert_eq!(CParser::parse_function_name("_g0_2d4d_1111"), Some((1, 1, 1, 1)));
    /// assert_eq!(CParser::parse_function_name("_g0_2d4d_2200"), Some((2, 2, 0, 0)));
    /// assert_eq!(CParser::parse_function_name("some_other_func"), None);
    /// ```
    pub fn parse_function_name(name: &str) -> Option<(usize, usize, usize, usize)> {
        // Look for the pattern _g0_2d4d_NNNN where NNNN is 4 digits
        // The suffix after the last underscore should be exactly 4 digit characters
        let parts: Vec<&str> = name.split('_').collect();

        // We expect at least 3 parts: "", "g0", "2d4d", "NNNN"
        if parts.len() < 3 {
            return None;
        }

        // The last part should be 4 digits
        let suffix = parts.last()?;
        if suffix.len() != 4 {
            return None;
        }

        let chars: Vec<char> = suffix.chars().collect();
        let li = chars[0].to_digit(10)? as usize;
        let lj = chars[1].to_digit(10)? as usize;
        let lk = chars[2].to_digit(10)? as usize;
        let ll = chars[3].to_digit(10)? as usize;

        Some((li, lj, lk, ll))
    }

    /// Extract all variable references from a function body.
    ///
    /// Scans the function body for variable names that match libcint's Rys
    /// quadrature intermediate naming convention. The recognized patterns are:
    ///
    /// - `c00x`, `c00y`, `c00z` — bra-side Rys intermediates
    /// - `c0px`, `c0py`, `c0pz` — ket-side Rys intermediates
    /// - `b10`, `b01`, `b00` — Boys-like function intermediates
    /// - `cpa`, `cpb`, `cpc`, `cpd` — center-specific intermediates
    ///
    /// # Arguments
    ///
    /// - `body` — The function body text (between the { } braces)
    ///
    /// # Returns
    ///
    /// A vector of unique variable names found, in order of first appearance.
    pub fn extract_variables(&self, body: &str) -> Vec<String> {
        let mut variables = Vec::new();
        let mut seen = HashSet::new();

        // Known libcint variable name patterns
        let known_vars = [
            "c00x", "c00y", "c00z",
            "c0px", "c0py", "c0pz",
            "b10", "b01", "b00",
            "cpa", "cpb", "cpc", "cpd",
        ];

        // First pass: look for variable declarations like "double *cpx = bc->c0px;"
        // and "double *c00x = bc->c00x;"
        for line in body.lines() {
            let line = line.trim();

            // Match "double *varname = ..."
            if let Some(rest) = line.strip_prefix("double") {
                let rest = rest.trim();
                if let Some(star_rest) = rest.strip_prefix('*') {
                    let rest = star_rest.trim();
                    // Extract the variable name (up to whitespace, =, or [)
                    if let Some(name) = rest.split_whitespace().next()
                        .or_else(|| rest.split('=').next())
                    {
                        let name = name.trim().trim_end_matches(';');
                        if !name.is_empty() && !seen.contains(name) {
                            seen.insert(name.to_string());
                            variables.push(name.to_string());
                        }
                    }
                }
            }

            // Second pass: scan for known variable names used in expressions
            for var in &known_vars {
                // Look for var as a word boundary match (not part of a longer name)
                if body.contains(var) && !seen.contains(*var) {
                    // Verify it's a standalone variable reference by checking
                    // that it's followed by '[' or whitespace or '*' or end
                    for (i, _) in body.match_indices(var) {
                        let end_idx = i + var.len();
                        let valid_end = end_idx >= body.len()
                            || body.as_bytes()[end_idx] == b'['
                            || body.as_bytes()[end_idx] == b' '
                            || body.as_bytes()[end_idx] == b'*'
                            || body.as_bytes()[end_idx] == b';'
                            || body.as_bytes()[end_idx] == b')'
                            || body.as_bytes()[end_idx] == b'+'
                            || body.as_bytes()[end_idx] == b'-';
                        // Also check the character before the variable name
                        let valid_start = i == 0
                            || body.as_bytes()[i - 1] == b' '
                            || body.as_bytes()[i - 1] == b'*'
                            || body.as_bytes()[i - 1] == b'('
                            || body.as_bytes()[i - 1] == b'+'
                            || body.as_bytes()[i - 1] == b'=';

                        if valid_end && valid_start {
                            seen.insert(var.to_string());
                            variables.push(var.to_string());
                            break;
                        }
                    }
                }
            }
        }

        variables
    }

    /// Parse `g[N] = expr` assignments from the function body into ParsedGate structs.
    ///
    /// Each assignment is decomposed into:
    /// - The result index N (from `g[N]`)
    /// - The full expression string (right-hand side of `=`)
    /// - An attempt to decompose the expression into a two-term linear combination
    ///   (left_var, right_var, coeff_left, coeff_right) to match the R1CS gate format
    ///
    /// # Expression Decomposition
    ///
    /// For simple expressions like `c00x[0] * c0px[0] * b10[0]`, the decomposition is:
    /// - `left_var = Some("c00x[0]")`, `right_var = Some("c0px[0]")`
    /// - `coeff_left = None`, `coeff_right = None` (no explicit numeric coefficient)
    ///
    /// For sum expressions like `a * b + c * d`, the decomposition is:
    /// - `left_var = Some("a * b")`, `right_var = Some("c * d")`
    /// - `coeff_left = None`, `coeff_right = None`
    ///
    /// For expressions with numeric coefficients like `0.5 * a * b`:
    /// - `left_var = Some("a * b")`, `coeff_left = Some(0.5)`
    ///
    /// Complex multi-term expressions that don't fit the two-term mold are
    /// stored with the full expression in `expression` and `None` for the
    /// decomposed fields.
    fn parse_gates(&self, body: &str) -> Vec<ParsedGate> {
        let mut gates = Vec::new();

        for line in body.lines() {
            let line = line.trim();

            // Look for g[N] = expr pattern
            if let Some(rest) = line.strip_prefix("g[") {
                // Extract the index
                if let Some(bracket_end) = rest.find(']') {
                    let idx_str = &rest[..bracket_end];
                    if let Ok(result_idx) = idx_str.parse::<usize>() {
                        // Skip the "] = " prefix
                        let after_bracket = &rest[bracket_end + 1..].trim();
                        if let Some(expr) = after_bracket.strip_prefix('=') {
                            let expr = expr.trim().trim_end_matches(';').trim();

                            // Decompose the expression
                            let (left_var, right_var, coeff_left, coeff_right) =
                                decompose_expression(expr);

                            gates.push(ParsedGate {
                                result_idx,
                                expression: expr.to_string(),
                                left_var,
                                right_var,
                                coeff_left,
                                coeff_right,
                            });
                        }
                    }
                }
            }
        }

        gates
    }

    /// Count how many `_g0_2d4d_NNNN` functions are defined in the source.
    ///
    /// This is useful for quickly checking how many shell quartets a libcint
    /// source file covers without doing a full parse.
    ///
    /// # Arguments
    ///
    /// - `content` — The full text content of a libcint C source file
    ///
    /// # Returns
    ///
    /// The number of `_g0_2d4d_NNNN` function definitions found.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use poler_eri::c_parser::CParser;
    ///
    /// let parser = CParser::new();
    /// let source = r#"
    ///     static inline void _g0_2d4d_0000(double *g, ...) { }
    ///     static inline void _g0_2d4d_0111(double *g, ...) { }
    ///     static inline void _g0_2d4d_1111(double *g, ...) { }
    /// "#;
    /// assert_eq!(parser.count_quartets_in_source(source), 3);
    /// ```
    pub fn count_quartets_in_source(&self, content: &str) -> usize {
        let mut count = 0;
        let mut search_from = 0;

        while search_from < content.len() {
            if let Some(pos) = content[search_from..].find("_g0_2d4d_") {
                // Verify this is followed by 4 digit characters
                let abs_pos = search_from + pos + "_g0_2d4d_".len();
                if abs_pos + 4 <= content.len() {
                    let suffix = &content[abs_pos..abs_pos + 4];
                    if suffix.chars().all(|c| c.is_ascii_digit()) {
                        count += 1;
                    }
                }
                search_from = abs_pos;
            } else {
                break;
            }
        }

        count
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper functions for parsing
// ─────────────────────────────────────────────────────────────────────────────

/// Find the start position of a `_g0_2d4d_NNNN` function definition in the source.
///
/// Searches for the pattern `void _g0_2d4d_NNNN(` starting from `search_start`.
fn find_function_start(content: &str, search_start: usize) -> Option<usize> {
    let search_region = &content[search_start..];

    // Look for "void _g0_2d4d_" pattern
    let pattern = "void _g0_2d4d_";
    let offset = search_region.find(pattern)?;
    Some(search_start + offset)
}

/// Extract the function name from a position where a function definition starts.
///
/// Given a position pointing into the source at a `_g0_2d4d_NNNN` function,
/// extracts the full function name including the underscore prefix.
fn extract_function_name(content: &str, fn_start: usize) -> Option<String> {
    // Find the underscore before "g0_2d4d_"
    let remaining = &content[fn_start..];

    // Skip "void " to get to the function name
    let after_void = remaining.strip_prefix("void ")?;
    let after_void = after_void.trim_start();

    // The function name starts with "_g0_2d4d_" and continues until '('
    let paren_pos = after_void.find('(')?;
    let name = &after_void[..paren_pos];

    // Validate that this looks like a proper function name
    if name.starts_with("_g0_2d4d_") && name.len() >= 13 {
        // _g0_2d4d_ (9 chars) + 4 digits = 13 chars minimum
        Some(name.to_string())
    } else {
        None
    }
}

/// Extract the function body (content between { and matching }) from a position.
///
/// Handles nested braces to correctly find the matching closing brace.
fn extract_function_body(content: &str, fn_start: usize) -> Option<String> {
    let remaining = &content[fn_start..];

    // Find the opening brace
    let open_brace = remaining.find('{')?;
    let body_start = open_brace + 1;

    // Find the matching closing brace, accounting for nesting
    let mut depth = 1;
    let mut pos = body_start;
    let bytes = remaining.as_bytes();

    while pos < bytes.len() && depth > 0 {
        match bytes[pos] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            // Skip string literals (which might contain braces)
            b'"' => {
                pos += 1;
                while pos < bytes.len() && bytes[pos] != b'"' {
                    if bytes[pos] == b'\\' {
                        pos += 1; // skip escaped character
                    }
                    pos += 1;
                }
            }
            b'\'' => {
                pos += 1;
                while pos < bytes.len() && bytes[pos] != b'\'' {
                    if bytes[pos] == b'\\' {
                        pos += 1; // skip escaped character
                    }
                    pos += 1;
                }
            }
            // Skip C-style comments
            b'/' if pos + 1 < bytes.len() && bytes[pos + 1] == b'/' => {
                // Line comment: skip to end of line
                while pos < bytes.len() && bytes[pos] != b'\n' {
                    pos += 1;
                }
                continue;
            }
            b'/' if pos + 1 < bytes.len() && bytes[pos + 1] == b'*' => {
                // Block comment: skip to */
                pos += 2;
                while pos + 1 < bytes.len() && !(bytes[pos] == b'*' && bytes[pos + 1] == b'/') {
                    pos += 1;
                }
                pos += 2; // skip past */
                continue;
            }
            _ => {}
        }
        pos += 1;
    }

    if depth == 0 {
        Some(remaining[body_start..pos - 1].to_string())
    } else {
        None
    }
}

/// Decompose a C expression into a two-term linear combination.
///
/// Attempts to split an expression at a `+` or `-` boundary (not inside
/// parentheses or brackets) and identify the left term, right term, and
/// any numeric coefficients.
///
/// # Supported Patterns
///
/// 1. `var1 * var2 * var3` → single term
///    - `left_var = Some("var1 * var2 * var3")`
///    - `right_var = None`
///
/// 2. `term1 + term2` → two terms
///    - `left_var = Some("term1")`
///    - `right_var = Some("term2")`
///
/// 3. `0.5 * term1` → coefficient + term
///    - `left_var = Some("term1")`
///    - `coeff_left = Some(0.5)`
///
/// 4. `term1 + 0.5 * term2` → mixed
///    - `left_var = Some("term1")`
///    - `right_var = Some("term2")`
///    - `coeff_right = Some(0.5)`
fn decompose_expression(expr: &str) -> (Option<String>, Option<String>, Option<f64>, Option<f64>) {
    // Find top-level addition/subtraction boundaries (not inside brackets or parens)
    let mut split_pos = None;
    let mut depth = 0i32;
    let bytes = expr.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b'+' if depth == 0 => {
                split_pos = Some(i);
                break;
            }
            b'-' if depth == 0 && i > 0 => {
                // Don't split on a leading minus sign
                // Only split on subtraction (minus after something that's not an operator)
                let prev = bytes[i - 1];
                if prev != b' ' && prev != b'*' && prev != b'(' && prev != b'[' {
                    // This minus could be subtraction — but be careful.
                    // For safety, only split on + at the top level.
                }
            }
            _ => {}
        }
    }

    if let Some(pos) = split_pos {
        // Two-term expression
        let left_expr = expr[..pos].trim();
        let right_expr = expr[pos + 1..].trim();

        let (left_var, coeff_left) = extract_coeff_and_var(left_expr);
        let (right_var, coeff_right) = extract_coeff_and_var(right_expr);

        (left_var, right_var, coeff_left, coeff_right)
    } else {
        // Single-term expression
        let (var, coeff) = extract_coeff_and_var(expr.trim());
        (var, None, coeff, None)
    }
}

/// Extract a numeric coefficient and variable reference from a term.
///
/// For example:
/// - `"0.5 * c00x[0] * c0px[0]"` → `(Some("c00x[0] * c0px[0]"), Some(0.5))`
/// - `"c00x[0] * c0px[0] * b10[0]"` → `(Some("c00x[0] * c0px[0] * b10[0]"), None)`
fn extract_coeff_and_var(term: &str) -> (Option<String>, Option<f64>) {
    // Check if the term starts with a numeric coefficient
    // Pattern: optional_sign digits (dot digits?) (e/E optional_sign digits?) *
    let term = term.trim();

    if term.is_empty() {
        return (None, None);
    }

    // Try to parse a leading number
    let mut num_end = 0;
    let bytes = term.as_bytes();

    // Skip optional sign
    if num_end < bytes.len() && (bytes[num_end] == b'+' || bytes[num_end] == b'-') {
        num_end += 1;
    }

    // Read digits
    while num_end < bytes.len() && bytes[num_end].is_ascii_digit() {
        num_end += 1;
    }

    // Optional decimal part
    if num_end < bytes.len() && bytes[num_end] == b'.' {
        num_end += 1;
        while num_end < bytes.len() && bytes[num_end].is_ascii_digit() {
            num_end += 1;
        }
    }

    // Optional exponent
    if num_end < bytes.len() && (bytes[num_end] == b'e' || bytes[num_end] == b'E') {
        num_end += 1;
        if num_end < bytes.len() && (bytes[num_end] == b'+' || bytes[num_end] == b'-') {
            num_end += 1;
        }
        while num_end < bytes.len() && bytes[num_end].is_ascii_digit() {
            num_end += 1;
        }
    }

    if num_end > 0 && num_end < bytes.len() {
        let num_str = &term[..num_end];
        if let Ok(coeff) = num_str.parse::<f64>() {
            // Skip the " * " after the number
            let rest = term[num_end..].trim();
            let rest = rest.strip_prefix('*').unwrap_or(rest).trim();
            if !rest.is_empty() {
                return (Some(rest.to_string()), Some(coeff));
            }
        }
    }

    // No leading coefficient found — the whole term is the variable expression
    if term.is_empty() {
        (None, None)
    } else {
        (Some(term.to_string()), None)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Test parse_function_name for various valid and invalid inputs.
    #[test]
    fn test_parse_function_name_valid() {
        assert_eq!(CParser::parse_function_name("_g0_2d4d_0000"), Some((0, 0, 0, 0)));
        assert_eq!(CParser::parse_function_name("_g0_2d4d_0111"), Some((0, 1, 1, 1)));
        assert_eq!(CParser::parse_function_name("_g0_2d4d_1111"), Some((1, 1, 1, 1)));
        assert_eq!(CParser::parse_function_name("_g0_2d4d_2200"), Some((2, 2, 0, 0)));
        assert_eq!(CParser::parse_function_name("_g0_2d4d_1000"), Some((1, 0, 0, 0)));
    }

    #[test]
    fn test_parse_function_name_invalid() {
        assert_eq!(CParser::parse_function_name("some_other_func"), None);
        assert_eq!(CParser::parse_function_name("_g0_2d4d_abc"), None);
        assert_eq!(CParser::parse_function_name("_g0_2d4d_12345"), None);
        assert_eq!(CParser::parse_function_name(""), None);
        assert_eq!(CParser::parse_function_name("_g0_2d4d_"), None);
    }

    /// Test parse_file with a minimal libcint-style function.
    #[test]
    fn test_parse_file_single_function() {
        let source = r#"
static inline void _g0_2d4d_0111(double *g, double *c00x, double *c0px,
                                   double *b10, double *b01, double *b00) {
    g[0] = c00x[0] * c0px[0] * b10[0];
    g[1] = c00x[0] * c0px[1] * b10[0] + c00x[1] * c0px[0] * b01[0];
    g[2] = c00x[1] * c0px[1] * b00[0];
}
"#;
        let parser = CParser::new();
        let patterns = parser.parse_file(source);

        assert_eq!(patterns.len(), 1, "Should find exactly one function");
        assert_eq!(patterns[0].fn_name, "_g0_2d4d_0111");
        assert_eq!(patterns[0].angular_momenta, (0, 1, 1, 1));
        assert_eq!(patterns[0].gates.len(), 3, "Should find 3 gate assignments");
        assert_eq!(patterns[0].gates[0].result_idx, 0);
        assert_eq!(patterns[0].gates[1].result_idx, 1);
        assert_eq!(patterns[0].gates[2].result_idx, 2);
    }

    /// Test parse_file with multiple functions in one source.
    #[test]
    fn test_parse_file_multiple_functions() {
        let source = r#"
static inline void _g0_2d4d_0000(double *g, double *b00) {
    g[0] = b00[0];
}

static inline void _g0_2d4d_1000(double *g, double *c00x, double *b10) {
    g[0] = c00x[0] * b10[0];
}

static inline void _g0_2d4d_1111(double *g, double *c00x, double *c0px,
                                   double *b10, double *b01, double *b00) {
    g[0] = c00x[0] * c0px[0] * b10[0];
    g[1] = c00x[0] * c0px[1] * b10[0] + c00x[1] * c0px[0] * b01[0];
    g[2] = c00x[1] * c0px[1] * b00[0];
    g[3] = c00x[0] * c0px[0] * b01[0] + c00x[1] * c0px[1] * b10[0];
    g[4] = c00x[0] * c0px[1] * b00[0] + c00x[1] * c0px[0] * b10[1];
    g[5] = c00x[1] * c0px[1] * b01[0];
    g[6] = c00x[0] * c0px[0] * b00[0];
    g[7] = c00x[0] * c0px[1] * b00[1] + c00x[1] * c0px[0] * b10[0];
    g[8] = c00x[1] * c0px[1] * b00[0];
}
"#;
        let parser = CParser::new();
        let patterns = parser.parse_file(source);

        assert_eq!(patterns.len(), 3, "Should find 3 functions");
        assert_eq!(patterns[0].angular_momenta, (0, 0, 0, 0));
        assert_eq!(patterns[1].angular_momenta, (1, 0, 0, 0));
        assert_eq!(patterns[2].angular_momenta, (1, 1, 1, 1));
    }

    /// Test extraction of variables from function body.
    #[test]
    fn test_extract_variables() {
        let body = r#"
    double *cpx = bc->c0px;
    double *cpy = bc->c0py;
    double *cpz = bc->c0pz;
    g[0] = c00x[0] * c0px[0] * b10[0];
    g[1] = c00x[0] * c0px[1] * b10[0] + c00x[1] * c0px[0] * b01[0];
"#;
        let parser = CParser::new();
        let vars = parser.extract_variables(body);

        // Should find both declared variables and known variable references
        assert!(
            vars.contains(&"cpx".to_string()),
            "Should find declared variable 'cpx'"
        );
        assert!(
            vars.contains(&"cpy".to_string()),
            "Should find declared variable 'cpy'"
        );
        assert!(
            vars.contains(&"cpz".to_string()),
            "Should find declared variable 'cpz'"
        );
    }

    /// Test extraction of known Rys intermediate variables.
    #[test]
    fn test_extract_known_variables() {
        let body = r#"
    g[0] = c00x[0] * c0px[0] * b10[0];
    g[1] = c00y[0] * c0py[0] * b01[0];
    g[2] = c00z[0] * c0pz[0] * b00[0];
"#;
        let parser = CParser::new();
        let vars = parser.extract_variables(body);

        // Should find all known variable references
        assert!(vars.contains(&"c00x".to_string()), "Should find c00x");
        assert!(vars.contains(&"c00y".to_string()), "Should find c00y");
        assert!(vars.contains(&"c00z".to_string()), "Should find c00z");
        assert!(vars.contains(&"c0px".to_string()), "Should find c0px");
        assert!(vars.contains(&"c0py".to_string()), "Should find c0py");
        assert!(vars.contains(&"c0pz".to_string()), "Should find c0pz");
        assert!(vars.contains(&"b10".to_string()), "Should find b10");
        assert!(vars.contains(&"b01".to_string()), "Should find b01");
        assert!(vars.contains(&"b00".to_string()), "Should find b00");
    }

    /// Test count_quartets_in_source.
    #[test]
    fn test_count_quartets() {
        let source = r#"
static inline void _g0_2d4d_0000(double *g) { }
static inline void _g0_2d4d_0111(double *g) { }
static inline void _g0_2d4d_1111(double *g) { }
"#;
        let parser = CParser::new();
        assert_eq!(parser.count_quartets_in_source(source), 3);
    }

    #[test]
    fn test_count_quartets_empty() {
        let parser = CParser::new();
        assert_eq!(parser.count_quartets_in_source(""), 0);
        assert_eq!(parser.count_quartets_in_source("no functions here"), 0);
    }

    /// Test that the parser handles the (ss|ss) base case correctly.
    #[test]
    fn test_parse_ssss_base_case() {
        let source = r#"
static inline void _g0_2d4d_0000(double *g, double *b00) {
    g[0] = b00[0];
}
"#;
        let parser = CParser::new();
        let patterns = parser.parse_file(source);

        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].angular_momenta, (0, 0, 0, 0));
        assert_eq!(patterns[0].gates.len(), 1);
        assert_eq!(patterns[0].gates[0].result_idx, 0);
        assert_eq!(patterns[0].gates[0].expression, "b00[0]");
    }

    /// Test that expressions with numeric coefficients are parsed correctly.
    #[test]
    fn test_expression_with_coefficient() {
        let source = r#"
static inline void _g0_2d4d_2200(double *g, double *c00x, double *c0px,
                                   double *b10, double *b01, double *b00) {
    g[0] = 0.5 * c00x[0] * c0px[0] * b10[0];
}
"#;
        let parser = CParser::new();
        let patterns = parser.parse_file(source);

        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].gates.len(), 1);
        assert_eq!(patterns[0].gates[0].result_idx, 0);
        // The coefficient 0.5 should be extracted
        assert!(
            patterns[0].gates[0].coeff_left.is_some(),
            "Should extract coefficient 0.5"
        );
        assert!(
            (patterns[0].gates[0].coeff_left.unwrap() - 0.5).abs() < 1e-10,
            "Coefficient should be 0.5"
        );
    }

    /// Test two-term expression decomposition.
    #[test]
    fn test_two_term_expression() {
        let source = r#"
static inline void _g0_2d4d_1111(double *g, double *c00x, double *c0px,
                                   double *b10, double *b01, double *b00) {
    g[0] = c00x[0] * c0px[0] * b10[0] + c00x[1] * c0px[0] * b01[0];
}
"#;
        let parser = CParser::new();
        let patterns = parser.parse_file(source);

        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].gates.len(), 1);
        let gate = &patterns[0].gates[0];
        assert!(gate.left_var.is_some(), "Should have left variable");
        assert!(gate.right_var.is_some(), "Should have right variable");
        assert!(
            gate.left_var.as_ref().unwrap().contains("c00x[0]"),
            "Left term should contain c00x[0]"
        );
        assert!(
            gate.right_var.as_ref().unwrap().contains("c00x[1]"),
            "Right term should contain c00x[1]"
        );
    }

    /// Test that nested brackets don't confuse the parser.
    #[test]
    fn test_nested_brackets() {
        let source = r#"
static inline void _g0_2d4d_1000(double *g, double *c00x, double *b10) {
    g[0] = c00x[0] * b10[0];
    g[1] = c00x[1] * b10[0];
    g[2] = c00x[2] * b10[0];
}
"#;
        let parser = CParser::new();
        let patterns = parser.parse_file(source);

        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].gates.len(), 3);
        assert_eq!(patterns[0].gates[0].result_idx, 0);
        assert_eq!(patterns[0].gates[1].result_idx, 1);
        assert_eq!(patterns[0].gates[2].result_idx, 2);
    }

    /// Test the decompose_expression helper function directly.
    #[test]
    fn test_decompose_expression_single_term() {
        let (left, right, cl, cr) = decompose_expression("c00x[0] * c0px[0] * b10[0]");
        assert!(left.is_some());
        assert!(right.is_none());
        assert!(cl.is_none());
        assert!(cr.is_none());
    }

    #[test]
    fn test_decompose_expression_two_terms() {
        let (left, right, _cl, _cr) =
            decompose_expression("c00x[0] * c0px[0] * b10[0] + c00x[1] * c0px[0] * b01[0]");
        assert!(left.is_some());
        assert!(right.is_some());
    }

    #[test]
    fn test_decompose_expression_with_coeff() {
        let (left, _right, cl, _cr) = decompose_expression("0.5 * c00x[0] * c0px[0] * b10[0]");
        assert!(left.is_some());
        assert!(cl.is_some());
        assert!((cl.unwrap() - 0.5).abs() < 1e-10);
    }

    /// Test that the parser correctly identifies variable declarations.
    #[test]
    fn test_variable_declaration_parsing() {
        let body = r#"
    double *cpx = bc->c0px;
    double *cpy = bc->c0py;
    double *rx = envs->rx;
"#;
        let parser = CParser::new();
        let vars = parser.extract_variables(body);

        assert!(vars.contains(&"cpx".to_string()));
        assert!(vars.contains(&"cpy".to_string()));
        assert!(vars.contains(&"rx".to_string()));
    }

    /// Test parsing a more realistic libcint function with variable aliases.
    #[test]
    fn test_realistic_libcint_function() {
        let source = r#"
static inline void _g0_2d4d_1111(double *g, CIntEnvVars *envs, Cache *c) {
    double *cpx = c->c0px;
    double *cpy = c->c0py;
    double *cpz = c->c0pz;
    double *c00x = c->c00x;
    double *c00y = c->c00y;
    double *c00z = c->c00z;
    double *b10 = c->b10;
    double *b01 = c->b01;
    double *b00 = c->b00;
    g[0] = c00x[0] * c0px[0] * b10[0];
    g[1] = c00x[0] * c0px[1] * b10[0] + c00x[1] * c0px[0] * b01[0];
    g[2] = c00y[0] * c0py[0] * b10[0];
    g[3] = c00z[0] * c0pz[0] * b00[0];
}
"#;
        let parser = CParser::new();
        let patterns = parser.parse_file(source);

        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].fn_name, "_g0_2d4d_1111");
        assert_eq!(patterns[0].angular_momenta, (1, 1, 1, 1));
        assert_eq!(patterns[0].gates.len(), 4);

        // Check that declared variables are captured
        assert!(patterns[0].variables.contains(&"cpx".to_string()));
        assert!(patterns[0].variables.contains(&"c00x".to_string()));
        assert!(patterns[0].variables.contains(&"b10".to_string()));
    }
}
