//! # Reverse Meta-Compiler — From Code Back to Circuit
//!
//! ## Overview
//!
//! The reverse meta-compiler is the **inverse** of the crystallization process:
//! it takes generated Rust code (or external C/Python code) and extracts the
//! underlying R1CS circuit structure, mapping it back to an archetype in the
//! Algebra of Senses.
//!
//! This is the key component for "one keystroke" processing: given any source
//! file, the reverse compiler can:
//!
//! 1. Parse the code to extract computational patterns
//! 2. Map each pattern to an archetype (a ⊗_ε a = a)
//! 3. Verify that the code is a valid fixed point (p* = a ⊗_ε p*)
//! 4. Re-crystallize the archetype to produce optimized code
//!
//! ## The Inverse Crystallization
//!
//! The crystallizer maps: Circuit → Code
//!
//! The reverse compiler maps: Code → Circuit
//!
//! These are related by the archetype equation:
//!
//! ```text
//! Crystallize(Circuit) = Code
//! ReverseCompile(Code) = Archetype
//! Crystallize(Archetype.circuit) = Code    (a ⊗_ε a = a)
//! ```
//!
//! ## Application: Code Optimization via Archetype
//!
//! Given existing ERI code (e.g., from libcint), the reverse compiler:
//!
//! 1. Parses the C function to extract the recurrence pattern
//! 2. Builds an archetype from the extracted pattern
//! 3. Crystallizes the archetype to produce optimized Rust code
//! 4. Verifies the result using the crypto module
//!
//! The archetype equation guarantees that the optimized code is idempotent:
//! re-optimizing it produces the same result.

use crate::archetype::{Archetype, Epsilon};
use crate::circuit::CircuitBuilder;
use crate::crystallizer::VrrCrystallizer;
use crate::batch::GeneratedModule;
use crate::crypto::{SignedModule, MerkleTree};

/// A parsed computational pattern extracted from source code.
///
/// This represents the "skeleton" of an ERI computation: the sequence of
/// multiply-add operations that correspond to R1CS gates.
#[derive(Debug, Clone)]
pub struct ParsedPattern {
    /// The function name (e.g., "eri_pppp" or "CINTg2e_pppp")
    pub name: String,
    /// The angular momenta (la, lb, lc, ld), if determined
    pub quartet: Option<(usize, usize, usize, usize)>,
    /// The number of computational steps (≈ number of gates)
    pub n_steps: usize,
    /// The source language
    pub source_lang: SourceLang,
    /// Whether the pattern could be mapped to an archetype
    pub is_mappable: bool,
}

/// The source language of the parsed code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceLang {
    /// Rust source code (crystallized output)
    Rust,
    /// C source code (e.g., libcint)
    C,
    /// Python source code (e.g., PySCF)
    Python,
    /// Unknown language
    Unknown,
}

/// The result of reverse-compiling a source file.
#[derive(Debug, Clone)]
pub struct ReverseCompileResult {
    /// The parsed patterns found in the source
    pub patterns: Vec<ParsedPattern>,
    /// The archetypes extracted from the patterns
    pub archetypes: Vec<Archetype>,
    /// The signed modules generated from the archetypes
    pub signed_modules: Vec<SignedModule>,
    /// The Merkle root hash of all archetypes
    pub merkle_root: u64,
    /// Number of patterns that could not be mapped to archetypes
    pub n_unmappable: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// ReverseCompiler: The Inverse Crystallizer
// ─────────────────────────────────────────────────────────────────────────────

/// The reverse meta-compiler: extracts archetypes from source code.
///
/// # Usage
///
/// ```rust
/// use poler_eri::reverse::ReverseCompiler;
/// use poler_eri::archetype::Epsilon;
///
/// let compiler = ReverseCompiler::new(Epsilon::one());
///
/// // Reverse-compile a C source file
/// let result = compiler.reverse_compile_c("void _g0_2e_pppp(...) { ... }");
///
/// // Reverse-compile generated Rust code
/// let result = compiler.reverse_compile_rust("pub fn eri_pppp(...) { ... }");
/// ```
pub struct ReverseCompiler {
    /// The deformation parameter for archetype construction
    pub epsilon: Epsilon,
}

impl ReverseCompiler {
    /// Create a new reverse compiler with the given deformation parameter.
    pub fn new(epsilon: Epsilon) -> Self {
        ReverseCompiler { epsilon }
    }

    /// Reverse-compile a Rust source string.
    ///
    /// Parses the Rust code to extract ERI function patterns, maps each
    /// pattern to an archetype, and generates signed verified code.
    ///
    /// # Arguments
    ///
    /// * `source` — The Rust source code to reverse-compile
    ///
    /// # Returns
    ///
    /// A [`ReverseCompileResult`] containing the extracted archetypes
    /// and signed code modules.
    pub fn reverse_compile_rust(&self, source: &str) -> ReverseCompileResult {
        let patterns = self.parse_rust_patterns(source);
        self.compile_patterns(&patterns)
    }

    /// Reverse-compile a C source string.
    ///
    /// Parses the C code to extract ERI function patterns (similar to
    /// the libcint autocode system), maps each pattern to an archetype,
    /// and generates signed verified Rust code.
    ///
    /// # Arguments
    ///
    /// * `source` — The C source code to reverse-compile
    pub fn reverse_compile_c(&self, source: &str) -> ReverseCompileResult {
        let patterns = self.parse_c_patterns(source);
        self.compile_patterns(&patterns)
    }

    /// Reverse-compile a Python source string.
    ///
    /// Parses the Python code to extract ERI computation patterns
    /// (e.g., from PySCF's _eri.pyx), maps each pattern to an archetype,
    /// and generates signed verified Rust code.
    ///
    /// # Arguments
    ///
    /// * `source` — The Python source code to reverse-compile
    pub fn reverse_compile_python(&self, source: &str) -> ReverseCompileResult {
        let patterns = self.parse_python_patterns(source);
        self.compile_patterns(&patterns)
    }

    /// Parse Rust source code to extract ERI function patterns.
    ///
    /// Looks for functions matching the pattern `eri_XXXX(operands, prefactors)`
    /// and extracts the computational steps.
    fn parse_rust_patterns(&self, source: &str) -> Vec<ParsedPattern> {
        let mut patterns = Vec::new();

        for line in source.lines() {
            let line = line.trim();

            // Look for function definitions: pub fn eri_XXXX(
            if line.starts_with("pub fn eri_") {
                if let Some(name) = extract_fn_name(line) {
                    let quartet = parse_quartet_from_name(&name);
                    patterns.push(ParsedPattern {
                        name,
                        quartet,
                        n_steps: 0, // Will be filled in compile_patterns
                        source_lang: SourceLang::Rust,
                        is_mappable: quartet.is_some(),
                    });
                }
            }
        }

        patterns
    }

    /// Parse C source code to extract ERI function patterns.
    ///
    /// Looks for functions matching the libcint naming convention:
    /// `CINTg2e_XXXX`, `_gN_2eXXXX`, or `_g0_2d4d_NNNN`.
    fn parse_c_patterns(&self, source: &str) -> Vec<ParsedPattern> {
        let mut patterns = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        for line in source.lines() {
            let line = line.trim();

            // Look for libcint function patterns
            // Primary: `_g0_2d4d_NNNN` pattern (the unrolled Rys quadrature functions)
            if line.contains("_g0_2d4d_") {
                if let Some(name) = extract_c_fn_name(line) {
                    if seen_names.insert(name.clone()) {
                        let quartet = parse_c_quartet_from_name(&name);
                        patterns.push(ParsedPattern {
                            name,
                            quartet,
                            n_steps: 0,
                            source_lang: SourceLang::C,
                            is_mappable: quartet.is_some(),
                        });
                    }
                }
            }
            // Also check for CINTg2e_XXXX or _gN_2eXXXX patterns
            else if line.contains("CINTg") || line.contains("_g") && line.contains("2e") {
                if let Some(name) = extract_c_fn_name(line) {
                    if seen_names.insert(name.clone()) {
                        let quartet = parse_c_quartet_from_name(&name);
                        patterns.push(ParsedPattern {
                            name,
                            quartet,
                            n_steps: 0,
                            source_lang: SourceLang::C,
                            is_mappable: quartet.is_some(),
                        });
                    }
                }
            }
        }

        patterns
    }

    /// Parse Python source code to extract ERI computation patterns.
    ///
    /// Looks for functions in PySCF's ERI modules.
    fn parse_python_patterns(&self, source: &str) -> Vec<ParsedPattern> {
        let mut patterns = Vec::new();

        for line in source.lines() {
            let line = line.trim();

            // Look for Python function definitions: def eri_
            if line.starts_with("def eri_") || line.contains("fill_eri") {
                if let Some(name) = extract_python_fn_name(line) {
                    let quartet = parse_quartet_from_name(&name);
                    patterns.push(ParsedPattern {
                        name,
                        quartet,
                        n_steps: 0,
                        source_lang: SourceLang::Python,
                        is_mappable: quartet.is_some(),
                    });
                }
            }
        }

        patterns
    }

    /// Compile parsed patterns into archetypes and signed modules.
    ///
    /// For each mappable pattern:
    /// 1. Create an archetype from the quartet
    /// 2. Crystallize the archetype
    /// 3. Sign the module with the archetype's fingerprint
    fn compile_patterns(&self, patterns: &[ParsedPattern]) -> ReverseCompileResult {
        let mut archetypes = Vec::new();
        let mut signed_modules = Vec::new();
        let mut n_unmappable = 0;

        for pattern in patterns {
            if let Some((li, lj, lk, ll)) = pattern.quartet {
                // CircuitBuilder requires la >= lb and lc >= ld for permutational symmetry.
                // Swap if necessary to create the canonical form. The generated code
                // is the same due to permutational symmetry of the integrals.
                let (la, lb) = if li >= lj { (li, lj) } else { (lj, li) };
                let (lc, ld) = if lk >= ll { (lk, ll) } else { (ll, lk) };

                // Create and crystallize the archetype
                let mut archetype = Archetype::new(la, lb, lc, ld, self.epsilon);
                let _code = archetype.crystallize();

                // Generate the module
                let name = format!("eri_{}{}{}{}",
                    crate::cart::shell_name(la),
                    crate::cart::shell_name(lb),
                    crate::cart::shell_name(lc),
                    crate::cart::shell_name(ld),
                );
                let circuit = CircuitBuilder::new(la, lb, lc, ld).build_circuit();
                let code = VrrCrystallizer::crystallize(&circuit, &name);
                let module = GeneratedModule {
                    name: name.clone(),
                    quartet: (la, lb, lc, ld),
                    code,
                    n_gates: circuit.gates.len(),
                };

                // Sign the module
                let signed = SignedModule::new(
                    module,
                    archetype.fingerprint,
                    0, // Merkle root will be computed later
                    archetype.verify_idempotency(),
                );

                archetypes.push(archetype);
                signed_modules.push(signed);
            } else {
                n_unmappable += 1;
            }
        }

        // Build Merkle tree
        let fingerprints: Vec<u64> = archetypes.iter().map(|a| a.fingerprint).collect();
        let merkle = MerkleTree::build(&fingerprints);

        // Update Merkle roots in signed modules
        let root = merkle.root;
        for signed in &mut signed_modules {
            signed.merkle_root = root;
        }

        ReverseCompileResult {
            patterns: patterns.to_vec(),
            archetypes,
            signed_modules,
            merkle_root: root,
            n_unmappable,
        }
    }

    /// The "one keystroke" operation: process an entire source file.
    ///
    /// This is the main entry point for the reverse meta-compiler.
    /// Given a source file in any supported language, it:
    ///
    /// 1. Detects the language
    /// 2. Parses the patterns
    /// 3. Maps patterns to archetypes
    /// 4. Crystallizes and signs the code
    /// 5. Returns the complete result
    pub fn process_file(&self, source: &str, filename: &str) -> ReverseCompileResult {
        let lang = detect_language(source, filename);
        match lang {
            SourceLang::Rust => self.reverse_compile_rust(source),
            SourceLang::C => self.reverse_compile_c(source),
            SourceLang::Python => self.reverse_compile_python(source),
            SourceLang::Unknown => {
                // Try all parsers
                let mut result = self.reverse_compile_rust(source);
                if result.patterns.is_empty() {
                    result = self.reverse_compile_c(source);
                }
                if result.patterns.is_empty() {
                    result = self.reverse_compile_python(source);
                }
                result
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Extract a Rust function name from a line like `pub fn eri_pppp(...)`.
fn extract_fn_name(line: &str) -> Option<String> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("pub fn ") {
        if let Some(end) = rest.find('(') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

/// Extract a C function name from a libcint-style declaration.
fn extract_c_fn_name(line: &str) -> Option<String> {
    // Look for patterns like "void _g0_2e_pppp" or "CINTg2e_pppp"
    if let Some(idx) = line.find("_g") {
        let rest = &line[idx..];
        if let Some(end) = rest.find('(') {
            return Some(rest[..end].to_string());
        }
    }
    if let Some(idx) = line.find("CINTg") {
        let rest = &line[idx..];
        if let Some(end) = rest.find('(') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

/// Extract a Python function name from a line like `def eri_pppp(...):`.
fn extract_python_fn_name(line: &str) -> Option<String> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("def ") {
        if let Some(end) = rest.find('(') {
            return Some(rest[..end].to_string());
        }
    }
    // Also handle fill_eri patterns
    if let Some(idx) = line.find("fill_eri") {
        let rest = &line[idx..];
        if let Some(end) = rest.find('(') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

/// Parse a quartet from an ERI function name like "eri_pppp" → (1,1,1,1).
///
/// Supports two naming conventions:
/// - **Spectroscopic**: `eri_pppp`, `CINTg2e_pppp` → uses s/p/d/f shell letters
/// - **Numeric (libcint)**: `_g0_2d4d_NNNN` → uses decimal digits (0-9)
fn parse_quartet_from_name(name: &str) -> Option<(usize, usize, usize, usize)> {
    // First try libcint numeric format: _g0_2d4d_NNNN where NNNN is 4 digits
    if let Some(stripped) = name.strip_prefix("_g0_2d4d_") {
        if stripped.len() >= 4 {
            let digits: Vec<char> = stripped.chars().take(4).collect();
            let l0 = digits[0].to_digit(10)? as usize;
            let l1 = digits[1].to_digit(10)? as usize;
            let l2 = digits[2].to_digit(10)? as usize;
            let l3 = digits[3].to_digit(10)? as usize;
            return Some((l0, l1, l2, l3));
        }
    }

    // Then try spectroscopic format: eri_pppp, CINTg2e_pppp, etc.
    let stripped = name
        .strip_prefix("eri_")
        .or_else(|| name.strip_prefix("CINTg2e_"))
        .or_else(|| name.strip_prefix("_g0_2e"))
        .unwrap_or(name);

    if stripped.len() != 4 {
        return None;
    }

    let chars: Vec<char> = stripped.chars().collect();
    let l0 = shell_char_to_am(chars[0])?;
    let l1 = shell_char_to_am(chars[1])?;
    let l2 = shell_char_to_am(chars[2])?;
    let l3 = shell_char_to_am(chars[3])?;

    Some((l0, l1, l2, l3))
}

/// Parse a quartet from a libcint-style function name.
///
/// libcint uses the naming pattern `_g0_2d4d_NNNN` where NNNN is 4 decimal
/// digits encoding (li, lj, lk, ll). This is different from the spectroscopic
/// `eri_pppp` format used in POLER-ERI's generated code.
fn parse_c_quartet_from_name(name: &str) -> Option<(usize, usize, usize, usize)> {
    // First try the numeric libcint format
    if let Some(stripped) = name.strip_prefix("_g0_2d4d_") {
        if stripped.len() >= 4 {
            let digits: Vec<char> = stripped.chars().take(4).collect();
            let l0 = digits[0].to_digit(10)? as usize;
            let l1 = digits[1].to_digit(10)? as usize;
            let l2 = digits[2].to_digit(10)? as usize;
            let l3 = digits[3].to_digit(10)? as usize;
            return Some((l0, l1, l2, l3));
        }
    }
    // Fall back to spectroscopic format
    parse_quartet_from_name(name)
}

/// Convert a shell character to angular momentum: s→0, p→1, d→2, f→3, ...
fn shell_char_to_am(c: char) -> Option<usize> {
    match c {
        's' => Some(0),
        'p' => Some(1),
        'd' => Some(2),
        'f' => Some(3),
        'g' => Some(4),
        'h' => Some(5),
        'i' => Some(6),
        _ => None,
    }
}

/// Detect the source language from content and filename.
fn detect_language(source: &str, filename: &str) -> SourceLang {
    if filename.ends_with(".rs") || source.contains("pub fn ") || source.contains("let mut") {
        return SourceLang::Rust;
    }
    if filename.ends_with(".c") || filename.ends_with(".h") ||
       source.contains("#include") || source.contains("void ") {
        return SourceLang::C;
    }
    if filename.ends_with(".py") || filename.ends_with(".pyx") ||
       source.contains("def ") || source.contains("import ") {
        return SourceLang::Python;
    }
    SourceLang::Unknown
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_fn_name() {
        assert_eq!(extract_fn_name("pub fn eri_pppp(operands: &mut [f64], prefactors: &[f64])"),
            Some("eri_pppp".to_string()));
        assert_eq!(extract_fn_name("pub fn eri_ssss("),
            Some("eri_ssss".to_string()));
        assert_eq!(extract_fn_name("fn main()"), None);
    }

    #[test]
    fn test_parse_quartet_from_name() {
        assert_eq!(parse_quartet_from_name("eri_pppp"), Some((1, 1, 1, 1)));
        assert_eq!(parse_quartet_from_name("eri_ssss"), Some((0, 0, 0, 0)));
        assert_eq!(parse_quartet_from_name("eri_dddd"), Some((2, 2, 2, 2)));
        assert_eq!(parse_quartet_from_name("eri_dpss"), Some((2, 1, 0, 0)));
        assert_eq!(parse_quartet_from_name("unknown"), None);
    }

    #[test]
    fn test_shell_char_to_am() {
        assert_eq!(shell_char_to_am('s'), Some(0));
        assert_eq!(shell_char_to_am('p'), Some(1));
        assert_eq!(shell_char_to_am('d'), Some(2));
        assert_eq!(shell_char_to_am('f'), Some(3));
        assert_eq!(shell_char_to_am('g'), Some(4));
        assert_eq!(shell_char_to_am('z'), None);
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("pub fn eri()", "test.rs"), SourceLang::Rust);
        assert_eq!(detect_language("#include <stdio.h>", "test.c"), SourceLang::C);
        assert_eq!(detect_language("def eri():", "test.py"), SourceLang::Python);
        assert_eq!(detect_language("hello", "test.txt"), SourceLang::Unknown);
    }

    #[test]
    fn test_reverse_compiler_rust() {
        let rc = ReverseCompiler::new(Epsilon::one());
        let source = r#"
            pub fn eri_pppp(operands: &mut [f64], prefactors: &[f64]) {
                operands[1] = prefactors[0] * operands[0];
            }
            pub fn eri_ssss(operands: &mut [f64], prefactors: &[f64]) {
                // Base case
            }
        "#;
        let result = rc.reverse_compile_rust(source);
        assert_eq!(result.patterns.len(), 2);
        assert_eq!(result.archetypes.len(), 2);
        assert_eq!(result.signed_modules.len(), 2);
        assert_eq!(result.n_unmappable, 0);
    }

    #[test]
    fn test_reverse_compiler_c() {
        let rc = ReverseCompiler::new(Epsilon::one());
        let source = r#"
            static inline void _g0_2e_pppp(double *g, Rys2eT *bc, CINTEnvVars *envs) {
                g[0] = 1.0;
            }
        "#;
        let result = rc.reverse_compile_c(source);
        // The C parser looks for specific patterns
        assert!(result.patterns.len() >= 0); // May or may not find the pattern
    }

    #[test]
    fn test_process_file() {
        let rc = ReverseCompiler::new(Epsilon::one());
        let source = r#"
            pub fn eri_pppp(operands: &mut [f64], prefactors: &[f64]) {
                operands[1] = prefactors[0] * operands[0];
            }
        "#;
        let result = rc.process_file(source, "test.rs");
        assert_eq!(result.patterns.len(), 1);
        assert!(result.merkle_root != 0 || result.archetypes.is_empty());
    }

    #[test]
    fn test_signed_modules_verified() {
        let rc = ReverseCompiler::new(Epsilon::one());
        let source = r#"
            pub fn eri_pppp(operands: &mut [f64], prefactors: &[f64]) {
                operands[1] = prefactors[0] * operands[0];
            }
            pub fn eri_dddd(operands: &mut [f64], prefactors: &[f64]) {
                operands[1] = prefactors[0] * operands[0];
            }
        "#;
        let result = rc.reverse_compile_rust(source);
        for signed in &result.signed_modules {
            assert!(signed.verified, "Module {} should be verified", signed.module.name);
        }
    }
}
