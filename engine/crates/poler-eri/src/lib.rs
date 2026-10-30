#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! # POLER-ERI v3.2.0 — Quantum-Inspired ERI Meta-Compiler
//!
//! This crate implements a meta-compiler that generates optimized Rust code for
//! electron repulsion integrals (ERIs). It does NOT compute integrals directly —
//! instead, it **compiles the mathematical recipe** (recurrence relations) into flat,
//! zero-alloc, SIMD-ready Rust source code.
//!
//! ## Architecture
//!
//! The meta-compiler pipeline has 3 stages (plus archetype/crypto layers):
//!
//! 1. **CircuitBuilder** — builds an R1CS circuit from VRR+HRR recurrence relations
//! 2. **VrrCrystallizer** — flattens the circuit into linear Rust code
//! 3. **BatchCompiler** — auto-iterates all shell quartets up to MAX_AM
//!
//! Additionally in v3.2.0:
//!
//! 4. **Archetype** — the Algebra of Senses: a ⊗_ε a = a
//! 5. **Crypto** — Merkle tree + fingerprint verification of generated code
//! 6. **Reverse** — inverse crystallization: code → archetype (reverse meta-compilation)
//! 7. **ShellDriver** — full shell-level ERI computation via Rys factorization
//!
//! ## The Archetype Equation
//!
//! An archetype is a non-trivial idempotent element a ∈ O such that:
//!
//! ```text
//! a ⊗_ε a = a    (Idempotency)
//! p* = a ⊗_ε p*  (Fixed-point equation)
//! ```
//!
//! This guarantees that re-compilation produces identical code and that
//! the SCF iteration converges to a fixed point.
//!
//! ## Quick Example
//!
//! ```rust
//! use poler_eri::{CircuitBuilder, VrrCrystallizer, BatchCompiler};
//!
//! // Build circuit for (pp|pp) quartet
//! let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
//! println!("(pp|pp): {} gates, {} operands", circuit.gates.len(), circuit.n_operands);
//!
//! // Crystallize to Rust code
//! let code = VrrCrystallizer::crystallize(&circuit, "eri_pppp");
//! println!("{}", code);
//!
//! // Batch-compile all quartets up to d-shell
//! let batch = BatchCompiler::new(2);
//! let modules = batch.compile_all();
//! println!("Generated {} modules", modules.len());
//! ```

#![allow(unused)]

pub mod types;
pub mod boys;
pub mod cart;
pub mod normalization;
pub mod overlap;
pub mod kinetic;
pub mod nuclear;
pub mod dipole;
pub mod vrr;
pub mod hrr;
pub mod eri;
pub mod circuit;
pub mod crystallizer;
pub mod batch;
pub mod c_parser;
pub mod transpiler;
pub mod contraction;
pub mod cart2sph;
pub mod screening;
pub mod basis;
pub mod molecule;
pub mod engine;
pub mod diagonalize;
pub mod density;
pub mod diis;
pub mod fock;
pub mod scf;
pub mod eri_rys;
pub mod shell_driver;
pub mod one_electron;
pub mod eri_direct;

// ── v3.2.0: Archetype, Crypto, Reverse Meta-Compiler ──────────────────────

pub mod archetype;
pub mod crypto;
pub mod reverse;

// ── Public API re-exports ──────────────────────────────────────────────────

pub use types::{Shell, Point, QuartetData, Atom, BasisSet, CartComp};
pub use boys::boys_f;
pub use cart::shell_name;
pub use circuit::{CircuitBuilder, Circuit, VrrGate, GateCoeff, HrrIndex, evaluate_circuit};
pub use crystallizer::VrrCrystallizer;
pub use batch::{BatchCompiler, GeneratedModule};
pub use c_parser::CParser;
pub use transpiler::Transpiler;
pub use molecule::Molecule;
pub use engine::IntegralEngine;

// ── v3.1.0 Public API ─────────────────────────────────────────────────────

pub use archetype::{Archetype, ArchetypeRegistry, Epsilon, SenseElement, direct_sum, tensor_product};
pub use crypto::{CryptoVerifier, MerkleTree, SignedModule, VerificationResult};
pub use reverse::{ReverseCompiler, ReverseCompileResult, ParsedPattern, SourceLang};

// ── v3.2.0: Shell Driver ────────────────────────────────────────────────────

pub use shell_driver::{ShellQuartetDriver, ExtractionPlan, eri_shell_full, compute_prefactors, compute_rys_roots};
