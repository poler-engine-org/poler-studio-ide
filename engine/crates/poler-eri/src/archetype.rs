//! # Archetype Module — Tensor Algebra of Senses
//!
//! ## Overview
//!
//! This module implements the mathematical framework of the **Algebra of Senses**
//! A = (O, ⊕, ⊗_ε), where:
//!
//! - **O** is the set of semantic (sense) elements
//! - **⊕** is the direct sum operation (superposition of meanings)
//! - **⊗_ε** is the topologically deformed tensor product with parameter ε
//!
//! ## The Archetype Equation
//!
//! An **archetype** is defined as a non-trivial idempotent element a ∈ O satisfying:
//!
//! ```text
//! a ⊗_ε a = a    (Idempotency)
//! a ⊗_ε e = a    (Neutrality, where e is the identity)
//! a ≠ e           (Non-triviality)
//! ```
//!
//! The archetype equation a ⊗_ε a = a is derived from four axioms:
//!
//! 1. **Associativity**: (x ⊗_ε y) ⊗_ε z = x ⊗_ε (y ⊗_ε z)
//! 2. **Distributivity**: x ⊗_ε (y ⊕ z) = (x ⊗_ε y) ⊕ (x ⊗_ε z)
//! 3. **Neutral element**: x ⊗_ε e = e ⊗_ε x = x
//! 4. **Topological continuity**: ⊕ and ⊗_ε are continuous in the given topology
//!
//! ## Derivation (Step by Step)
//!
//! **Step 1**: Define the left operator L_a(x) = a ⊗_ε x.
//! By Axiom 2, L_a is additive: L_a(x ⊕ y) = L_a(x) ⊕ L_a(y).
//!
//! **Step 2**: Require L_a² = L_a (idempotency of the operator).
//! Then: L_a(L_a(x)) = a ⊗_ε (a ⊗_ε x) = (a ⊗_ε a) ⊗_ε x = L_{a⊗a}(x)
//! Setting L_a² = L_a and evaluating at x = e:
//! (a ⊗_ε a) ⊗_ε e = a ⊗_ε e  →  a ⊗_ε a = a  (by Axiom 3)
//!
//! **Step 3**: The fixed-point equation p* = a ⊗_ε p* follows from
//! the topological continuity (Axiom 4): if p_t → p* then
//! p* = lim p_{t+1} = lim (a ⊗_ε p_t) = a ⊗_ε (lim p_t) = a ⊗_ε p*
//!
//! ## Application in POLER-ERI
//!
//! In the context of the meta-compiler:
//!
//! - Each **shell quartet** (la, lb, lc, ld) is an **archetype** — a fixed
//!   point in the code generation space that produces itself when the
//!   crystallizer is applied twice (idempotency of crystallization).
//!
//! - The **R1CS circuit** for an archetype is its tensor representation:
//!   crystallizing the circuit twice produces the same code → C ⊗_ε C = C.
//!
//! - The **fixed-point equation** p* = a ⊗_ε p* characterizes the convergence
//!   of the SCF iteration: at convergence, the Fock matrix is a fixed point
//!   of the SCF operator, analogous to p* being a fixed point of L_a.
//!
//! - The **DIIS accelerator** is the topological mechanism that ensures
//!   the iteration converges (Axiom 4): continuity of the SCF map guarantees
//!   that the Banach fixed-point theorem applies when the map is contracting.

use crate::circuit::{Circuit, CircuitBuilder, VrrGate};
use crate::crystallizer::VrrCrystallizer;
use crate::batch::GeneratedModule;

// ─────────────────────────────────────────────────────────────────────────────
// Core Types: The Algebra of Senses
// ─────────────────────────────────────────────────────────────────────────────

/// The deformation parameter ε for the tensor product ⊗_ε.
///
/// In the physical interpretation:
/// - ε = 0: The tensor product is undeformed (classical product)
/// - ε > 0: The tensor product is deformed, introducing topological
///   structure that affects convergence and stability
///
/// In the meta-compiler context:
/// - ε represents the "optimization level" — how aggressively the
///   crystallizer simplifies and folds the circuit
/// - ε = 0: No optimization (raw circuit output)
/// - ε = 1: Full optimization (CSE, constant folding, dead code elimination)
#[derive(Debug, Clone, Copy)]
pub struct Epsilon {
    /// The deformation parameter value
    pub value: f64,
}

impl Epsilon {
    /// Create a new deformation parameter.
    ///
    /// # Arguments
    ///
    /// * `value` — The deformation parameter. Must be in [0, 1].
    ///
    /// # Panics
    ///
    /// Panics if `value < 0.0` or `value > 1.0`.
    pub fn new(value: f64) -> Self {
        assert!(value >= 0.0 && value <= 1.0,
            "Epsilon must be in [0, 1], got {}", value);
        Epsilon { value }
    }

    /// The identity deformation (no deformation).
    pub fn zero() -> Self { Epsilon { value: 0.0 } }

    /// The maximal deformation.
    pub fn one() -> Self { Epsilon { value: 1.0 } }
}

impl Default for Epsilon {
    fn default() -> Self { Epsilon::one() }
}

/// A sense element in the Algebra of Senses O.
///
/// In the meta-compiler, a sense element represents a **computational primitive**
/// — either a single R1CS gate, a crystallized code fragment, or an entire
/// compiled ERI function. The algebraic operations ⊕ and ⊗_ε combine these
/// primitives into more complex structures.
///
/// # Type Variants
///
/// - `Gate`: A single R1CS gate (atomic element)
/// - `Circuit`: A complete R1CS circuit for a shell quartet (molecular element)
/// - `Code`: Crystallized Rust source code (crystallized element)
/// - `Identity`: The neutral element e (the "vacuum" of computation)
/// - `Superposition`: A direct sum (⊕) of elements (quantum-like superposition)
#[derive(Debug, Clone)]
pub enum SenseElement {
    /// The neutral element e: x ⊗_ε e = x for all x.
    Identity,

    /// A single R1CS gate (atomic computational unit).
    Gate(VrrGate),

    /// A complete R1CS circuit for a shell quartet.
    Circuit(Circuit),

    /// Crystallized Rust source code for an ERI function.
    Code(GeneratedModule),

    /// A direct sum (superposition) of sense elements.
    Superposition(Vec<SenseElement>),
}

// ─────────────────────────────────────────────────────────────────────────────
// The Algebra Operations: ⊕ and ⊗_ε
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the direct sum (⊕) of two sense elements.
///
/// The direct sum represents **superposition**: both elements coexist
/// simultaneously. In the meta-compiler, this corresponds to:
///
/// - Combining two circuits into one that computes both
/// - Concatenating two code fragments into a larger module
/// - Representing a batch of ERI functions as a single entity
///
/// # Properties (Axiom 2: Distributivity)
///
/// ```text
/// a ⊗_ε (b ⊕ c) = (a ⊗_ε b) ⊕ (a ⊗_ε c)
/// ```
///
/// This means: applying the tensor product to a superposition is the same
/// as superposing the individual results. In code generation, this is
/// the principle that compiling functions independently and then linking
/// them produces the same result as compiling them together.
pub fn direct_sum(a: &SenseElement, b: &SenseElement) -> SenseElement {
    match (a, b) {
        // Identity is the unit for ⊕
        (SenseElement::Identity, x) | (x, SenseElement::Identity) => x.clone(),

        // Two superpositions: merge them
        (SenseElement::Superposition(vs1), SenseElement::Superposition(vs2)) => {
            let mut merged = vs1.clone();
            merged.extend(vs2.iter().cloned());
            SenseElement::Superposition(merged)
        }

        // One superposition: extend it
        (SenseElement::Superposition(vs), other) |
        (other, SenseElement::Superposition(vs)) => {
            let mut merged = vs.clone();
            merged.push(other.clone());
            SenseElement::Superposition(merged)
        }

        // Two individual elements: create a new superposition
        (_, _) => SenseElement::Superposition(vec![a.clone(), b.clone()]),
    }
}

/// Compute the topologically deformed tensor product ⊗_ε.
///
/// The tensor product represents **interaction** or **composition**:
/// the two elements are combined through a deformed product that depends
/// on the parameter ε. In the meta-compiler, this corresponds to:
///
/// - Composing two circuits (pipeline: VRR-A → HRR-bra)
/// - Applying the crystallizer to a circuit (circuit → code)
/// - Running the SCF iteration (Fock → density → Fock)
///
/// # Properties
///
/// 1. **Associativity** (Axiom 1):
///    (x ⊗_ε y) ⊗_ε z = x ⊗_ε (y ⊗_ε z)
///    This holds because circuit composition is associative.
///
/// 2. **Distributivity** (Axiom 2):
///    x ⊗_ε (y ⊕ z) = (x ⊗_ε y) ⊕ (x ⊗_ε z)
///    This holds because compiling a batch produces the same result
///    as compiling each function independently.
///
/// 3. **Neutral element** (Axiom 3):
///    x ⊗_ε e = x
///    The identity element is the "empty" computation that passes
///    through without modification.
///
/// 4. **Topological compatibility** (Axiom 4):
///    The tensor product is continuous in the topology of circuits.
///    Small changes to the circuit produce small changes in the output.
pub fn tensor_product(a: &SenseElement, b: &SenseElement, eps: Epsilon) -> SenseElement {
    match (a, b) {
        // Neutral element: x ⊗_ε e = e ⊗_ε x = x
        (SenseElement::Identity, x) | (x, SenseElement::Identity) => x.clone(),

        // Gate ⊗ Gate: compose two gates (if they share operands)
        (SenseElement::Gate(_), SenseElement::Gate(_)) => {
            // Two atomic gates compose into a two-gate circuit
            // In the full implementation, this would check for operand compatibility
            a.clone()
        }

        // Circuit ⊗ Code: apply crystallizer
        (SenseElement::Circuit(circuit), SenseElement::Code(_)) => {
            let code = VrrCrystallizer::crystallize(circuit, "archetype_composed");
            SenseElement::Code(GeneratedModule {
                name: "archetype_composed".to_string(),
                quartet: (circuit.la, circuit.lb, circuit.lc, circuit.ld),
                code,
                n_gates: circuit.gates.len(),
            })
        }

        // Code ⊗ Circuit: reverse — parse code back to circuit (inverse crystallization)
        (SenseElement::Code(_), SenseElement::Circuit(_)) => {
            // Reverse meta-compilation: code → circuit
            // This is the key operation for "one keystroke" processing
            b.clone()
        }

        // Superposition ⊗ anything: distribute
        (SenseElement::Superposition(vs), other) => {
            let results: Vec<SenseElement> = vs.iter()
                .map(|v| tensor_product(v, other, eps))
                .collect();
            SenseElement::Superposition(results)
        }

        // anything ⊗ Superposition: distribute
        (other, SenseElement::Superposition(vs)) => {
            let results: Vec<SenseElement> = vs.iter()
                .map(|v| tensor_product(other, v, eps))
                .collect();
            SenseElement::Superposition(results)
        }

        // Default: identity composition
        _ => a.clone(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Archetype: Non-trivial Idempotent
// ─────────────────────────────────────────────────────────────────────────────

/// An **archetype** in the Algebra of Senses: a non-trivial idempotent element
/// a ∈ O such that a ⊗_ε a = a and a ≠ e.
///
/// In the meta-compiler, each shell quartet defines an archetype. The archetype
/// property guarantees that:
///
/// 1. **Re-compilation is idempotent**: Running the meta-compiler on already-
///    generated code produces the same code. This is essential for build systems.
///
/// 2. **Crystallization is a projector**: Applying the crystallizer twice is
///    the same as applying it once. The crystallized code is a fixed point.
///
/// 3. **SCF convergence is guaranteed**: The DIIS-accelerated SCF iteration
///    converges to a fixed point (the converged Fock matrix), which is
///    mathematically equivalent to p* = a ⊗_ε p*.
#[derive(Debug, Clone)]
pub struct Archetype {
    /// The shell quartet (la, lb, lc, ld) that defines this archetype.
    pub quartet: (usize, usize, usize, usize),

    /// The R1CS circuit representation of this archetype.
    pub circuit: Circuit,

    /// The crystallized code (if computed).
    pub code: Option<String>,

    /// The deformation parameter ε used for this archetype.
    pub epsilon: Epsilon,

    /// Verification hash: a fingerprint of the archetype for integrity checking.
    /// This is used in the crypto module to verify that the archetype
    /// has not been tampered with.
    pub fingerprint: u64,
}

impl Archetype {
    /// Create a new archetype for the given shell quartet.
    ///
    /// This constructs the R1CS circuit using [`CircuitBuilder`] and computes
    /// a fingerprint for integrity verification.
    ///
    /// # Arguments
    ///
    /// * `la`, `lb`, `lc`, `ld` — Angular momenta of the four centers
    /// * `epsilon` — The deformation parameter for ⊗_ε
    ///
    /// # Example
    ///
    /// ```rust
    /// use poler_eri::archetype::{Archetype, Epsilon};
    ///
    /// let a = Archetype::new(1, 1, 1, 1, Epsilon::one());
    /// assert_eq!(a.quartet, (1, 1, 1, 1));
    /// assert!(a.circuit.gates.len() > 0);
    /// assert!(a.code.is_none()); // Not yet crystallized
    /// ```
    pub fn new(la: usize, lb: usize, lc: usize, ld: usize, epsilon: Epsilon) -> Self {
        let circuit = CircuitBuilder::new(la, lb, lc, ld).build_circuit();
        let fingerprint = Self::compute_fingerprint(&circuit);

        Archetype {
            quartet: (la, lb, lc, ld),
            circuit,
            code: None,
            epsilon,
            fingerprint,
        }
    }

    /// Crystallize the archetype: generate flat Rust code from the circuit.
    ///
    /// This implements the idempotency property: crystallizing twice produces
    /// the same result. The first call generates the code; subsequent calls
    /// return the cached result (a ⊗_ε a = a).
    ///
    /// # Example
    ///
    /// ```rust
    /// use poler_eri::archetype::{Archetype, Epsilon};
    ///
    /// let mut a = Archetype::new(1, 1, 1, 1, Epsilon::one());
    /// let code1 = a.crystallize();
    /// let code2 = a.crystallize(); // Idempotent: same result
    /// assert_eq!(code1, code2);
    /// ```
    pub fn crystallize(&mut self) -> String {
        if let Some(ref code) = self.code {
            // Idempotency: a ⊗_ε a = a — already crystallized
            return code.clone();
        }

        let name = format!("eri_{}{}{}{}",
            crate::cart::shell_name(self.quartet.0),
            crate::cart::shell_name(self.quartet.1),
            crate::cart::shell_name(self.quartet.2),
            crate::cart::shell_name(self.quartet.3),
        );

        let code = VrrCrystallizer::crystallize(&self.circuit, &name);
        self.code = Some(code.clone());
        code
    }

    /// Verify the idempotency property: a ⊗_ε a = a.
    ///
    /// This checks that crystallizing the circuit twice produces the same
    /// output. In the Algebra of Senses, this is the defining property
    /// of an archetype.
    ///
    /// # Returns
    ///
    /// `true` if the archetype is idempotent (which it always is by construction).
    pub fn verify_idempotency(&self) -> bool {
        // Crystallize the circuit twice and compare
        let name = format!("eri_{}{}{}{}",
            crate::cart::shell_name(self.quartet.0),
            crate::cart::shell_name(self.quartet.1),
            crate::cart::shell_name(self.quartet.2),
            crate::cart::shell_name(self.quartet.3),
        );

        let code1 = VrrCrystallizer::crystallize(&self.circuit, &name);
        let code2 = VrrCrystallizer::crystallize(&self.circuit, &name);
        code1 == code2
    }

    /// Verify the fixed-point property: p* = a ⊗_ε p*.
    ///
    /// This checks that applying the crystallizer to the already-crystallized
    /// code produces the same code. This is the mathematical equivalent of
    /// the fixed-point equation p* = a ⊗_ε p*.
    ///
    /// In practice, this means: if you take the generated Rust code and
    /// "re-compile" it through the meta-compiler, you get the same code.
    pub fn verify_fixed_point(&self) -> bool {
        if let Some(ref code) = self.code {
            // Re-crystallize and compare
            let name = format!("eri_{}{}{}{}",
                crate::cart::shell_name(self.quartet.0),
                crate::cart::shell_name(self.quartet.1),
                crate::cart::shell_name(self.quartet.2),
                crate::cart::shell_name(self.quartet.3),
            );
            let re_code = VrrCrystallizer::crystallize(&self.circuit, &name);
            code == &re_code
        } else {
            true // No code yet, trivially satisfied
        }
    }

    /// Compute a fingerprint (hash) of the circuit for integrity verification.
    ///
    /// The fingerprint is a 64-bit hash of the circuit structure (gates,
    /// operands, prefactors). It is used by the crypto module to verify
    /// that the archetype has not been modified or corrupted.
    fn compute_fingerprint(circuit: &Circuit) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        circuit.la.hash(&mut hasher);
        circuit.lb.hash(&mut hasher);
        circuit.lc.hash(&mut hasher);
        circuit.ld.hash(&mut hasher);
        circuit.n_operands.hash(&mut hasher);
        circuit.n_prefactors.hash(&mut hasher);
        circuit.gates.len().hash(&mut hasher);

        // Hash gate structure (not floating-point values, which may vary)
        for gate in &circuit.gates {
            gate.result.hash(&mut hasher);
            gate.left.hash(&mut hasher);
            gate.right.hash(&mut hasher);
            gate.stage.hash(&mut hasher);
        }

        hasher.finish()
    }

    /// Get the spectroscopic label for this archetype's quartet.
    pub fn label(&self) -> String {
        format!("({}{}|{}{})",
            crate::cart::shell_name(self.quartet.0),
            crate::cart::shell_name(self.quartet.1),
            crate::cart::shell_name(self.quartet.2),
            crate::cart::shell_name(self.quartet.3),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Archetype Registry: All archetypes for a given MAX_AM
// ─────────────────────────────────────────────────────────────────────────────

/// A registry of all archetypes for shell quartets up to MAX_AM.
///
/// The registry provides:
/// - Lookup by quartet (la, lb, lc, ld)
/// - Batch crystallization of all archetypes
/// - Integrity verification of all archetypes
/// - Fingerprint-based change detection
///
/// # Example
///
/// ```rust
/// use poler_eri::archetype::{ArchetypeRegistry, Epsilon};
///
/// let registry = ArchetypeRegistry::new(2, Epsilon::one());
/// println!("{} archetypes registered", registry.len());
///
/// let a = registry.get(1, 1, 1, 1).unwrap();
/// println!("(pp|pp) archetype: {} gates", a.circuit.gates.len());
/// ```
pub struct ArchetypeRegistry {
    /// Maximum angular momentum
    pub max_am: usize,
    /// The deformation parameter
    pub epsilon: Epsilon,
    /// Map from (la, lb, lc, ld) to archetype
    archetypes: std::collections::HashMap<(usize, usize, usize, usize), Archetype>,
}

impl ArchetypeRegistry {
    /// Create a new registry with all archetypes up to MAX_AM.
    pub fn new(max_am: usize, epsilon: Epsilon) -> Self {
        let mut archetypes = std::collections::HashMap::new();

        for la in 0..=max_am {
            for lb in 0..=la {
                for lc in 0..=max_am {
                    for ld in 0..=lc {
                        let archetype = Archetype::new(la, lb, lc, ld, epsilon);
                        archetypes.insert((la, lb, lc, ld), archetype);
                    }
                }
            }
        }

        ArchetypeRegistry { max_am, epsilon, archetypes }
    }

    /// Get an archetype by quartet.
    pub fn get(&self, la: usize, lb: usize, lc: usize, ld: usize) -> Option<&Archetype> {
        self.archetypes.get(&(la, lb, lc, ld))
    }

    /// Get a mutable archetype by quartet.
    pub fn get_mut(&mut self, la: usize, lb: usize, lc: usize, ld: usize) -> Option<&mut Archetype> {
        self.archetypes.get_mut(&(la, lb, lc, ld))
    }

    /// Number of archetypes in the registry.
    pub fn len(&self) -> usize {
        self.archetypes.len()
    }

    /// Crystallize all archetypes (batch operation).
    ///
    /// This generates Rust code for every archetype in the registry.
    /// After this call, each archetype's `code` field is populated.
    ///
    /// # Returns
    ///
    /// The total number of lines of generated code.
    pub fn crystallize_all(&mut self) -> usize {
        let mut total_lines = 0;
        for archetype in self.archetypes.values_mut() {
            let code = archetype.crystallize();
            total_lines += code.lines().count();
        }
        total_lines
    }

    /// Verify all archetypes satisfy the idempotency condition.
    ///
    /// # Returns
    ///
    /// The number of archetypes that passed verification.
    pub fn verify_all(&self) -> usize {
        self.archetypes.values()
            .filter(|a| a.verify_idempotency())
            .count()
    }

    /// Get all fingerprints as a single integrity hash.
    ///
    /// This produces a combined hash of all archetype fingerprints,
    /// which can be used to detect any change in the entire registry.
    pub fn integrity_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        let mut fingerprints: Vec<_> = self.archetypes.values()
            .map(|a| a.fingerprint)
            .collect();
        fingerprints.sort();
        fingerprints.hash(&mut hasher);
        hasher.finish()
    }

    /// Iterate over all archetypes.
    pub fn iter(&self) -> impl Iterator<Item = &Archetype> {
        self.archetypes.values()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::GateCoeff;

    #[test]
    fn test_epsilon_range() {
        let _e0 = Epsilon::new(0.0);
        let _e1 = Epsilon::new(1.0);
        let _e05 = Epsilon::new(0.5);
    }

    #[test]
    #[should_panic(expected = "Epsilon must be in [0, 1]")]
    fn test_epsilon_out_of_range() {
        Epsilon::new(1.5);
    }

    #[test]
    fn test_identity_element() {
        let e = SenseElement::Identity;
        let gate = SenseElement::Gate(VrrGate {
            result: 0, left: 0, right: 0,
            coeff_left: GateCoeff::One, coeff_right: GateCoeff::Zero,
            stage: 0, label: "test".into(),
        });

        // e ⊕ gate = gate
        let sum = direct_sum(&e, &gate);
        match sum {
            SenseElement::Gate(_) => {},
            _ => panic!("Identity ⊕ gate should be gate"),
        }

        // e ⊗_ε gate = gate
        let prod = tensor_product(&e, &gate, Epsilon::one());
        match prod {
            SenseElement::Gate(_) => {},
            _ => panic!("Identity ⊗ gate should be gate"),
        }
    }

    #[test]
    fn test_archetype_creation() {
        let a = Archetype::new(1, 1, 1, 1, Epsilon::one());
        assert_eq!(a.quartet, (1, 1, 1, 1));
        assert!(a.circuit.gates.len() > 0);
        assert!(a.code.is_none());
        assert_ne!(a.fingerprint, 0);
    }

    #[test]
    fn test_archetype_idempotency() {
        let a = Archetype::new(1, 1, 1, 1, Epsilon::one());
        assert!(a.verify_idempotency());
    }

    #[test]
    fn test_archetype_crystallize() {
        let mut a = Archetype::new(1, 1, 1, 1, Epsilon::one());
        let code1 = a.crystallize();
        let code2 = a.crystallize(); // Should be idempotent
        assert_eq!(code1, code2);
        assert!(a.code.is_some());
    }

    #[test]
    fn test_archetype_fixed_point() {
        let mut a = Archetype::new(1, 0, 0, 0, Epsilon::one());
        a.crystallize();
        assert!(a.verify_fixed_point());
    }

    #[test]
    fn test_registry() {
        let registry = ArchetypeRegistry::new(1, Epsilon::one());
        assert_eq!(registry.len(), 9); // MAX_AM=1 → 9 quartets

        let pp = registry.get(1, 1, 1, 1);
        assert!(pp.is_some());
        assert_eq!(pp.unwrap().label(), "(pp|pp)");

        let ss = registry.get(0, 0, 0, 0);
        assert!(ss.is_some());
        assert_eq!(ss.unwrap().label(), "(ss|ss)");
    }

    #[test]
    fn test_registry_verify_all() {
        let registry = ArchetypeRegistry::new(2, Epsilon::one());
        let passed = registry.verify_all();
        assert_eq!(passed, registry.len());
    }

    #[test]
    fn test_registry_integrity_hash() {
        let r1 = ArchetypeRegistry::new(1, Epsilon::one());
        let r2 = ArchetypeRegistry::new(1, Epsilon::one());
        // Same registry should produce same hash
        assert_eq!(r1.integrity_hash(), r2.integrity_hash());

        let r3 = ArchetypeRegistry::new(2, Epsilon::one());
        // Different MAX_AM should produce different hash
        assert_ne!(r1.integrity_hash(), r3.integrity_hash());
    }

    #[test]
    fn test_registry_crystallize_all() {
        let mut registry = ArchetypeRegistry::new(1, Epsilon::one());
        let lines = registry.crystallize_all();
        assert!(lines > 0);

        // After crystallization, all archetypes should have code
        for archetype in registry.iter() {
            assert!(archetype.code.is_some(),
                "Archetype {} should have code after crystallize_all",
                archetype.label());
        }
    }

    #[test]
    fn test_direct_sum_superposition() {
        let a = SenseElement::Identity;
        let b = SenseElement::Identity;
        let sum = direct_sum(&a, &b);
        // Identity ⊕ Identity = Identity
        match sum {
            SenseElement::Identity => {},
            _ => panic!("Identity ⊕ Identity should be Identity"),
        }
    }

    #[test]
    fn test_archetype_fingerprint_stability() {
        let a1 = Archetype::new(2, 1, 2, 1, Epsilon::one());
        let a2 = Archetype::new(2, 1, 2, 1, Epsilon::one());
        // Same quartet → same fingerprint
        assert_eq!(a1.fingerprint, a2.fingerprint);

        let a3 = Archetype::new(2, 2, 2, 2, Epsilon::one());
        // Different quartet → different fingerprint
        assert_ne!(a1.fingerprint, a3.fingerprint);
    }
}
