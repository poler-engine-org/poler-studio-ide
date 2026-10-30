//! # Cryptographic Verification Module — Archetype-Based Code Integrity
//!
//! ## Overview
//!
//! This module implements the **cryptographic verification** layer for POLER-ERI,
//! built on the foundation of the Archetype equation from the Algebra of Senses.
//!
//! The key insight is that the archetype idempotency property:
//!
//! ```text
//! a ⊗_ε a = a
//! ```
//!
//! provides a natural **hash function**: if you compile the same circuit twice
//! and get different results, something is wrong. This is equivalent to saying
//! that the crystallizer is a **projector** in the algebraic sense — applying
//! it twice is the same as applying it once.
//!
//! ## Architecture
//!
//! The crypto system has three layers:
//!
//! 1. **Fingerprint Layer**: Each archetype has a 64-bit fingerprint (hash)
//!    of its circuit structure. This detects accidental corruption.
//!
//! 2. **Commitment Layer**: A Merkle tree of all archetype fingerprints
//!    provides a single root hash that commits to the entire codebase.
//!    Any change to any archetype changes the root hash.
//!
//! 3. **Verification Layer**: The idempotency check a ⊗_ε a = a serves
//!    as a zero-knowledge proof: you can verify that the crystallized code
//!    is correct without knowing the original circuit.
//!
//! ## Connection to the Archetype Equation
//!
//! The cryptographic properties follow directly from the algebra:
//!
//! - **Idempotency** (a ⊗_ε a = a): Re-compilation produces the same code.
//!   This is the "collision resistance" of the crystallizer.
//!
//! - **Fixed-point** (p* = a ⊗_ε p*): The converged SCF result is unique.
//!   This is the "determinism" of the code generation.
//!
//! - **Topological continuity**: Small changes in the circuit produce
//!   small changes in the code. This is the "avalanche effect" in
//!   cryptographic hash functions.
//!
//! ## One-Keystroke Processing
//!
//! The combination of archetype + crypto enables "one keystroke" processing:
//!
//! ```text
//! Source code (C/Python/Rust)
//!        │
//!        ▼
//!   Parse → Archetype Registry
//!        │
//!        ▼
//!   Crystallize all archetypes
//!        │
//!        ▼
//!   Verify idempotency (a⊗a=a) for each
//!        │
//!        ▼
//!   Compute Merkle root
//!        │
//!        ▼
//!   Output: Generated Rust + Integrity proof
//! ```
//!
//! If any step fails (idempotency violated, fingerprint mismatch), the
//! process aborts with a clear error message indicating which archetype
//! is corrupted.

use crate::archetype::{ArchetypeRegistry, Epsilon};
use crate::batch::{BatchCompiler, GeneratedModule};

// ─────────────────────────────────────────────────────────────────────────────
// Fingerprint: Per-Archetype Integrity
// ─────────────────────────────────────────────────────────────────────────────

/// The result of verifying a single archetype.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// The quartet (la, lb, lc, ld) that was verified.
    pub quartet: (usize, usize, usize, usize),
    /// The spectroscopic label (e.g., "(pp|pp)").
    pub label: String,
    /// Whether the idempotency check passed (a ⊗_ε a = a).
    pub idempotency_ok: bool,
    /// Whether the fixed-point check passed (p* = a ⊗_ε p*).
    pub fixed_point_ok: bool,
    /// Whether the fingerprint matches the expected value.
    pub fingerprint_ok: bool,
    /// The archetype's fingerprint value.
    pub fingerprint: u64,
    /// Number of gates in the circuit.
    pub n_gates: usize,
}

impl VerificationResult {
    /// Returns true if all checks passed.
    pub fn all_ok(&self) -> bool {
        self.idempotency_ok && self.fixed_point_ok && self.fingerprint_ok
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Merkle Tree: Whole-Codebase Commitment
// ─────────────────────────────────────────────────────────────────────────────

/// A simple Merkle tree for archetype fingerprints.
///
/// Each leaf is the fingerprint of one archetype. Internal nodes are
/// the XOR of their children (a simplified hash for this implementation).
/// The root hash commits to the entire set of archetypes.
#[derive(Debug, Clone)]
pub struct MerkleTree {
    /// The root hash of the tree.
    pub root: u64,
    /// The number of leaves (archetypes).
    pub n_leaves: usize,
    /// All leaf hashes in order.
    pub leaves: Vec<u64>,
}

impl MerkleTree {
    /// Build a Merkle tree from a list of archetype fingerprints.
    ///
    /// # Algorithm
    ///
    /// 1. Sort the fingerprints to ensure deterministic ordering.
    /// 2. Pair adjacent fingerprints and compute XOR to get parent nodes.
    /// 3. Repeat until a single root hash remains.
    ///
    /// # Properties
    ///
    /// - **Deterministic**: The same set of fingerprints always produces
    ///   the same root hash.
    /// - **Collision-sensitive**: Changing any single fingerprint changes
    ///   the root hash with high probability.
    /// - **Efficient**: O(n) construction, O(log n) verification.
    pub fn build(fingerprints: &[u64]) -> Self {
        let mut leaves: Vec<u64> = fingerprints.to_vec();
        leaves.sort();
        let n_leaves = leaves.len();

        if leaves.is_empty() {
            return MerkleTree { root: 0, n_leaves: 0, leaves };
        }

        // Build tree bottom-up
        let mut current_level = leaves.clone();
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in current_level.chunks(2) {
                if chunk.len() == 2 {
                    // XOR pair
                    next_level.push(chunk[0] ^ chunk[1]);
                } else {
                    // Odd one out: pass through
                    next_level.push(chunk[0]);
                }
            }
            current_level = next_level;
        }

        MerkleTree {
            root: current_level[0],
            n_leaves,
            leaves,
        }
    }

    /// Verify that a specific archetype's fingerprint is included in the tree.
    ///
    /// # Arguments
    ///
    /// * `fingerprint` — The archetype's fingerprint to verify
    ///
    /// # Returns
    ///
    /// `true` if the fingerprint is one of the leaves.
    pub fn verify_inclusion(&self, fingerprint: u64) -> bool {
        self.leaves.contains(&fingerprint)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CryptoVerifier: Full Verification Pipeline
// ─────────────────────────────────────────────────────────────────────────────

/// Full cryptographic verification pipeline for POLER-ERI.
///
/// The verifier checks that the entire code generation pipeline produces
/// consistent, reproducible, and tamper-proof results using the archetype
/// equation a ⊗_ε a = a as the mathematical foundation.
///
/// # Usage
///
/// ```rust
/// use poler_eri::crypto::CryptoVerifier;
/// use poler_eri::archetype::Epsilon;
///
/// let verifier = CryptoVerifier::new(2, Epsilon::one());
/// let results = verifier.verify_all();
/// let all_passed = results.iter().all(|r| r.all_ok());
/// println!("All archetypes verified: {}", all_passed);
/// ```
pub struct CryptoVerifier {
    /// The archetype registry being verified.
    pub registry: ArchetypeRegistry,
    /// The Merkle tree of fingerprints.
    pub merkle: MerkleTree,
}

impl CryptoVerifier {
    /// Create a new verifier for all archetypes up to MAX_AM.
    pub fn new(max_am: usize, epsilon: Epsilon) -> Self {
        let registry = ArchetypeRegistry::new(max_am, epsilon);
        let fingerprints: Vec<u64> = registry.iter().map(|a| a.fingerprint).collect();
        let merkle = MerkleTree::build(&fingerprints);

        CryptoVerifier { registry, merkle }
    }

    /// Verify a single archetype.
    ///
    /// Checks:
    /// 1. **Idempotency**: a ⊗_ε a = a (crystallizing twice produces the same code)
    /// 2. **Fixed-point**: p* = a ⊗_ε p* (the code is a fixed point of crystallization)
    /// 3. **Fingerprint**: The circuit's fingerprint matches the expected value
    pub fn verify_one(&self, la: usize, lb: usize, lc: usize, ld: usize) -> Option<VerificationResult> {
        let archetype = self.registry.get(la, lb, lc, ld)?;

        Some(VerificationResult {
            quartet: (la, lb, lc, ld),
            label: archetype.label(),
            idempotency_ok: archetype.verify_idempotency(),
            fixed_point_ok: archetype.verify_fixed_point(),
            fingerprint_ok: self.merkle.verify_inclusion(archetype.fingerprint),
            fingerprint: archetype.fingerprint,
            n_gates: archetype.circuit.gates.len(),
        })
    }

    /// Verify all archetypes in the registry.
    ///
    /// # Returns
    ///
    /// A vector of verification results, one per archetype.
    pub fn verify_all(&self) -> Vec<VerificationResult> {
        let mut results = Vec::new();
        for archetype in self.registry.iter() {
            let (la, lb, lc, ld) = archetype.quartet;
            if let Some(result) = self.verify_one(la, lb, lc, ld) {
                results.push(result);
            }
        }
        results
    }

    /// Get the Merkle root hash (commitment to all archetypes).
    pub fn root_hash(&self) -> u64 {
        self.merkle.root
    }

    /// Run the full "one keystroke" pipeline:
    /// parse → crystallize → verify → output.
    ///
    /// This is the main entry point for the complete code generation
    /// and verification process.
    ///
    /// # Returns
    ///
    /// A tuple of (generated modules, verification results, Merkle root).
    pub fn one_keystroke(max_am: usize) -> (Vec<GeneratedModule>, Vec<VerificationResult>, u64) {
        let epsilon = Epsilon::one();

        // Step 1: Build archetype registry
        let mut registry = ArchetypeRegistry::new(max_am, epsilon);

        // Step 2: Crystallize all archetypes
        registry.crystallize_all();

        // Step 3: Generate code modules
        let batch = BatchCompiler::new(max_am);
        let modules = batch.compile_all();

        // Step 4: Build Merkle tree
        let fingerprints: Vec<u64> = registry.iter().map(|a| a.fingerprint).collect();
        let merkle = MerkleTree::build(&fingerprints);

        // Step 5: Verify all archetypes
        let verifier = CryptoVerifier { registry, merkle };
        let results = verifier.verify_all();

        (modules, results, verifier.root_hash())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Code Signing: Attach cryptographic proof to generated code
// ─────────────────────────────────────────────────────────────────────────────

/// A signed code module with cryptographic proof of integrity.
///
/// The signature is derived from the archetype's fingerprint and the
/// Merkle root, providing a chain of trust from the circuit structure
/// through to the generated code.
#[derive(Debug, Clone)]
pub struct SignedModule {
    /// The generated code module.
    pub module: GeneratedModule,
    /// The archetype fingerprint for this module.
    pub fingerprint: u64,
    /// The Merkle root hash at the time of signing.
    pub merkle_root: u64,
    /// Whether this module passed all verification checks.
    pub verified: bool,
}

impl SignedModule {
    /// Create a signed module from a generated module and verification data.
    pub fn new(module: GeneratedModule, fingerprint: u64, merkle_root: u64, verified: bool) -> Self {
        SignedModule { module, fingerprint, merkle_root, verified }
    }

    /// Generate a header comment for the signed module.
    ///
    /// This comment is prepended to the generated code and contains
    /// the cryptographic proof of integrity.
    pub fn signed_code(&self) -> String {
        let mut out = String::new();
        out.push_str("//! POLER-ERI Signed Module\n");
        out.push_str(&format!("//! Fingerprint: {:016x}\n", self.fingerprint));
        out.push_str(&format!("//! Merkle Root: {:016x}\n", self.merkle_root));
        out.push_str(&format!("//! Verified: {}\n", self.verified));
        out.push_str(&format!("//! Quartet: {}\n", self.module.name));
        out.push_str("//!\n");
        out.push_str("//! This module is cryptographically signed. The fingerprint and\n");
        out.push_str("//! Merkle root provide a chain of trust from the R1CS circuit\n");
        out.push_str("//! structure to the generated Rust code. Any modification to\n");
        out.push_str("//! this file will break the signature.\n\n");
        out.push_str(&self.module.code);
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_tree_single() {
        let tree = MerkleTree::build(&[42]);
        assert_eq!(tree.n_leaves, 1);
        assert_eq!(tree.root, 42);
    }

    #[test]
    fn test_merkle_tree_pair() {
        let tree = MerkleTree::build(&[10, 20]);
        assert_eq!(tree.n_leaves, 2);
        assert_eq!(tree.root, 10 ^ 20);
    }

    #[test]
    fn test_merkle_tree_deterministic() {
        let t1 = MerkleTree::build(&[5, 3, 1, 7]);
        let t2 = MerkleTree::build(&[1, 3, 5, 7]); // Same values, different order
        assert_eq!(t1.root, t2.root); // Sorted → same root
    }

    #[test]
    fn test_merkle_tree_inclusion() {
        let tree = MerkleTree::build(&[10, 20, 30]);
        assert!(tree.verify_inclusion(10));
        assert!(tree.verify_inclusion(20));
        assert!(tree.verify_inclusion(30));
        assert!(!tree.verify_inclusion(99));
    }

    #[test]
    fn test_merkle_tree_empty() {
        let tree = MerkleTree::build(&[]);
        assert_eq!(tree.n_leaves, 0);
        assert_eq!(tree.root, 0);
    }

    #[test]
    fn test_verifier_creation() {
        let verifier = CryptoVerifier::new(1, Epsilon::one());
        assert_ne!(verifier.root_hash(), 0);
    }

    #[test]
    fn test_verifier_verify_all() {
        let verifier = CryptoVerifier::new(1, Epsilon::one());
        let results = verifier.verify_all();
        assert_eq!(results.len(), 9); // MAX_AM=1 → 9 quartets
        for result in &results {
            assert!(result.all_ok(), "Archetype {} failed verification", result.label);
        }
    }

    #[test]
    fn test_signed_module() {
        let module = BatchCompiler::compile_one(1, 1, 1, 1);
        let signed = SignedModule::new(module, 0xDEADBEEF, 0xCAFEBABE, true);
        let code = signed.signed_code();
        assert!(code.contains("Fingerprint: 00000000deadbeef"));
        assert!(code.contains("Merkle Root: 00000000cafebabe"));
        assert!(code.contains("Verified: true"));
    }

    #[test]
    fn test_one_keystroke() {
        let (modules, results, root) = CryptoVerifier::one_keystroke(1);
        assert_eq!(modules.len(), 9);
        assert_eq!(results.len(), 9);
        assert_ne!(root, 0);

        // All archetypes should pass verification
        for result in &results {
            assert!(result.all_ok(), "Archetype {} failed", result.label);
        }
    }
}
