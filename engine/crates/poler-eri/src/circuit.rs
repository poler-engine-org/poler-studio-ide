//! # R1CS Circuit Builder — The Heart of the POLER-ERI Meta-Compiler
//!
//! ## Overview
//!
//! This module implements the **Rank-1 Constraint System (R1CS) circuit builder**
//! for electron repulsion integrals (ERIs). It is the central component of the
//! POLER-ERI v3.2.0 meta-compiler: it takes a shell quartet specification
//! `(la, lb, lc, ld)` — the angular momenta of the four Gaussian centers — and
//! produces a directed acyclic graph (DAG) of R1CS gates that represents the
//! complete 2D VRR table + HRR expansion pipeline, matching libcint's algorithm.
//!
//! ## v3.2.0: Full HRR Intermediate Recording
//!
//! The key change in v3.2.0 is that the [`Circuit`] struct now records **ALL**
//! HRR intermediate results — not just the final target — enabling the shell
//! driver to compute ALL Cartesian component integrals via Rys factorization.
//!
//! Previously (v3.1.0), the circuit only saved `I(la, lb | m, 0)` for each `m`
//! (the HRR-Bra target) and `I(la, lb | lc, ld)` (the HRR-Ket target). In
//! v3.2.0, the circuit records:
//!
//! - **`g_idx[n][m]`**: the full 2D VRR table operand index map (was a local
//!   variable in `build_circuit`, now a field of `Circuit`)
//! - **`hrr_bra_map[m]`**: ALL `I(na, nb | m, 0)` intermediates for each `m`,
//!   covering every `(na, nb)` with `na + nb ≤ nmax` and `nb ≤ lb`
//! - **`hrr_ket_map`**: ALL `I(la, lb | nc, nd)` intermediates, covering every
//!   `(nc, nd)` with `nc + nd ≤ mmax` and `nd ≤ ld`
//!
//! The new [`Circuit::find_operand`] method allows the shell driver to look up
//! the operand index for any `I(na, nb | nc, nd)` that the circuit computes.
//!
//! ## v3.1.0 Overhaul: Full libcint-Compatible Recurrence
//!
//! The v3.0.x CircuitBuilder used a SIMPLIFIED 4-stage pipeline that omitted
//! critical recurrence terms. Specifically:
//!
//! - VRR-A only built to `la` (not `la+lb`), missing the `n/(2p)` decrement
//! - VRR-C used a single cross-term instead of the proper `b00` coupling
//! - No 2D VRR table — just a linear chain
//! - No Rys quadrature — used Boys function (incompatible with proper cross-terms)
//!
//! The v3.1.0+ CircuitBuilder implements the FULL recurrence structure:
//!
//! 1. **2D VRR table**: `g[n][m]` for `n = 0..nmax`, `m = 0..mmax`
//! 2. **Rys quadrature**: `operands[0] = 1.0`, cross-terms via `b00 = t/(2(p+q))`
//! 3. **Proper HRR**: triangular expansion from `(nmax,0|mmax,0)` to `(la,lb|lc,ld)`
//!
//! ## The 5-Stage Pipeline
//!
//! | Stage | Name      | Description                                    |
//! |-------|-----------|------------------------------------------------|
//! | 0     | VRR-Bra   | Build `g[n][0]` for `n = 0..nmax`             |
//! | 1     | VRR-Ket   | Build `g[0][m]` for `m = 0..mmax`             |
//! | 2     | VRR-Full  | Build `g[n][m]` for `n≥1, m≥1` with b00 cross |
//! | 3     | HRR-Bra   | Expand from `(nmax,0|m,0)` to `(la,lb|m,0)`   |
//! | 4     | HRR-Ket   | Expand from `(la,lb|mmax,0)` to `(la,lb|lc,ld)` |
//!
//! ## Quantum Chemistry Background
//!
//! In the Rys quadrature approach (matching libcint), the ERI is computed by:
//!
//! 1. Computing Rys roots `t_k` and weights `w_k`
//! 2. For each root, evaluating a 2D polynomial `g[n][m](t)` via VRR
//! 3. Multiplying `gx * gy * gz * w_k` and summing over roots
//!
//! The VRR recurrence in the Rys form:
//!
//! ```text
//! g[n+1][0] = c00 * g[n][0] + n * b10 * g[n-1][0]           (bra VRR, m=0)
//! g[0][m+1] = c0p * g[0][m] + m * b01 * g[0][m-1]           (ket VRR, n=0)
//! g[n+1][m] = c00 * g[n][m] + n * b10 * g[n-1][m] + m * b00 * g[n][m-1]  (full VRR)
//! ```
//!
//! Where:
//! - `c00 = PA_i` (root-dependent for Rys: `PA_i + t * PQ_i`)
//! - `c0p = QC_i` (root-dependent for Rys: `QC_i + t * PQ_i`)
//! - `b10 = 1/(2p)` (root-independent)
//! - `b01 = 1/(2q)` (root-independent)
//! - `b00 = t/(2(p+q))` (root-dependent)
//!
//! The HRR recurrence:
//!
//! ```text
//! I(a, b+1_i | cd) = I(a+1_i, b | cd) - AB_i * I(a, b | cd)
//! I(ab | c, d+1_i) = I(ab | c+1_i, d) - CD_i * I(ab | cd)
//! ```
//!
//! ## Prefactor Layout
//!
//! The prefactors array has exactly 7 entries:
//!
//! | Index | Name | Description                         | Root-dependent |
//! |-------|------|-------------------------------------|----------------|
//! | 0     | c00  | `PA_i` for current direction        | Yes            |
//! | 1     | c0p  | `QC_i` for current direction        | Yes            |
//! | 2     | b10  | `1/(2p)`                            | No             |
//! | 3     | b01  | `1/(2q)`                            | No             |
//! | 4     | b00  | `t/(2(p+q))`                        | Yes            |
//! | 5     | AB   | `A_i - B_i` (negated for HRR)       | No             |
//! | 6     | CD   | `C_i - D_i` (negated for HRR)       | No             |
//!
//! The caller should put `-AB_i` in `prefactors[5]` and `-CD_i` in `prefactors[6]`
//! so that the HRR gate computes `I(a+1,b) + (-AB) * I(a,b) = I(a+1,b) - AB*I(a,b)`.

use crate::boys::boys_f;
use crate::cart::shell_name;

// ─────────────────────────────────────────────────────────────────────────────
// GateCoeff: Coefficient types for R1CS gates
// ─────────────────────────────────────────────────────────────────────────────

/// Coefficient type for an R1CS gate's left or right term.
///
/// Each gate computes: `result = coeff_left × operand[left] + coeff_right × operand[right]`
///
/// The coefficient can take several forms, reflecting the different mathematical
/// quantities that appear in the VRR/HRR recurrence relations:
///
/// | Variant               | Meaning                              | Example in VRR              |
/// |-----------------------|--------------------------------------|-----------------------------|
/// | `Zero`                | Coefficient is exactly 0             | Unused term                 |
/// | `One`                 | Coefficient is exactly 1             | HRR main term               |
/// | `Scalar(f64)`         | A compile-time constant              | Numerical prefactors        |
/// | `BoyF(usize)`         | Boys function F_m(T) at runtime     | Legacy, not used in v3.2.0  |
/// | `Prefactor(usize)`    | Runtime prefactor from table         | c00, c0p, AB, CD, etc.     |
/// | `Operand(usize)`      | Value of another operand             | Cross-term references       |
/// | `ScaledPrefactor(f64, usize)` | `scalar * prefactors[index]` | `n * b10`, `m * b00`, etc. |
///
/// # The ScaledPrefactor variant
///
/// This variant is essential for the 2D VRR table recurrence. Terms like
/// `n * b10 * g[n-1][m]` have `n` as a compile-time constant (the angular
/// momentum index) and `b10 = 1/(2p)` as a runtime prefactor. Rather than
/// pre-computing `n/(2p)` as a separate prefactor (which would require one
/// per VRR step), `ScaledPrefactor(n as f64, b10_idx)` represents this
/// compactly and lets the crystallizer emit `2.0 * prefactors[2] * operands[0]`.
#[derive(Debug, Clone, PartialEq)]
pub enum GateCoeff {
    /// Coefficient is exactly zero — the term is absent.
    Zero,
    /// Coefficient is exactly one — the term passes through unchanged.
    One,
    /// Coefficient is a compile-time scalar constant.
    Scalar(f64),
    /// Coefficient is the Boys function F_m(T), evaluated at runtime.
    /// Legacy variant from v3.0.x; not emitted by the v3.2.0 CircuitBuilder.
    BoyF(usize),
    /// Coefficient is a runtime prefactor, indexed into the prefactor table.
    Prefactor(usize),
    /// Coefficient is the value of another operand, enabling cross-references.
    Operand(usize),
    /// Coefficient is `scalar * prefactors[index]`.
    ///
    /// This represents terms like `n * b10` or `m * b00` where `n` or `m` is
    /// a compile-time constant (angular momentum index) and the prefactor is
    /// a runtime value. The crystallizer emits `scalar * prefactors[index]`.
    ScaledPrefactor(f64, usize),
}

// ─────────────────────────────────────────────────────────────────────────────
// VrrGate: A single R1CS gate in the VRR/HRR circuit
// ─────────────────────────────────────────────────────────────────────────────

/// A single gate in the R1CS circuit representing one step of the VRR/HRR pipeline.
///
/// Each gate computes one linear combination:
///
/// ```text
/// operands[result] = coeff_left × operands[left] + coeff_right × operands[right]
/// ```
///
/// This is the fundamental unit of computation in the meta-compiler. The entire
/// ERI evaluation for a shell quartet is expressed as a sequence of these gates,
/// evaluated in order. No loops, no branches, no allocations — just a straight-line
/// sequence of multiply-add operations.
#[derive(Debug, Clone, PartialEq)]
pub struct VrrGate {
    /// Index into operands[] where the result is stored (single-assignment).
    pub result: usize,
    /// Index into operands[] for the left input.
    pub left: usize,
    /// Index into operands[] for the right input.
    pub right: usize,
    /// Coefficient for the left operand.
    pub coeff_left: GateCoeff,
    /// Coefficient for the right operand.
    pub coeff_right: GateCoeff,
    /// Pipeline stage: 0=VRR-Bra, 1=VRR-Ket, 2=VRR-Full, 3=HRR-Bra, 4=HRR-Ket.
    pub stage: usize,
    /// Human-readable label for debugging and code generation.
    pub label: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// HrrIndex: Index entry for an HRR intermediate result
// ─────────────────────────────────────────────────────────────────────────────

/// An index entry recording one HRR intermediate result.
///
/// In the HRR expansion, `I(i, j | ...)` represents a partial integral where
/// `i` is the angular momentum on the first center of the pair (A for bra, C
/// for ket) and `j` is the angular momentum on the second center (B for bra, D
/// for ket).
///
/// For bra-side entries: `HrrIndex { j: nb, i: na, operand }` records
/// `I(na, nb | m, 0)` for a specific `m` (determined by which
/// `hrr_bra_map[m]` list this entry belongs to).
///
/// For ket-side entries: `HrrIndex { j: nd, i: nc, operand }` records
/// `I(la, lb | nc, nd)` where `(la, lb)` is the circuit's fixed bra pair.
#[derive(Debug, Clone, PartialEq)]
pub struct HrrIndex {
    /// Angular momentum on the second center of the pair (nb for bra, nd for ket).
    pub j: usize,
    /// Angular momentum on the first center of the pair (na for bra, nc for ket).
    pub i: usize,
    /// Operand index in the circuit's operand array.
    pub operand: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Circuit: The complete R1CS circuit for a shell quartet
// ─────────────────────────────────────────────────────────────────────────────

/// A complete R1CS circuit for evaluating ERI recurrence relations for one
/// shell quartet `(la, lb | lc, ld)` using the 2D VRR table + HRR expansion.
///
/// The circuit is a flat list of [`VrrGate`]s that, when evaluated in sequence,
/// transforms the base case `g[0][0] = 1.0` into the target integral component
/// `gx(la, lb | lc, ld)` for one Cartesian direction and one Rys root.
///
/// # Operand Model
///
/// - `operands[0]` = `1.0` (base case, the Rys polynomial at order 0)
/// - Subsequent operands hold VRR table entries and HRR intermediates
/// - The final result is the last operand written by the last gate
///
/// # Prefactor Model
///
/// Exactly 7 prefactors are needed per evaluation (one Cartesian direction,
/// one Rys root):
///
/// ```text
/// prefactors[0] = c00   (PA_i for current direction, root-dependent)
/// prefactors[1] = c0p   (QC_i for current direction, root-dependent)
/// prefactors[2] = b10   (1/(2p), root-independent)
/// prefactors[3] = b01   (1/(2q), root-independent)
/// prefactors[4] = b00   (t/(2(p+q)), root-dependent)
/// prefactors[5] = -AB_i (negated A-B component, root-independent)
/// prefactors[6] = -CD_i (negated C-D component, root-independent)
/// ```
///
/// # v3.2.0 HRR Intermediate Maps
///
/// The circuit records ALL HRR intermediate results, not just the final target.
/// This enables the shell driver to compute ALL Cartesian component integrals
/// via Rys factorization without re-running the circuit.
///
/// - **`g_idx[n][m]`**: operand index for the VRR table entry `g[n][m]`
/// - **`hrr_bra_map[m]`**: list of [`HrrIndex`] entries for `I(na, nb | m, 0)`,
///   covering ALL `na + nb ≤ nmax` with `nb ≤ lb`
/// - **`hrr_ket_map`**: list of [`HrrIndex`] entries for `I(la, lb | nc, nd)`,
///   covering ALL `nc + nd ≤ mmax` with `nd ≤ ld`
#[derive(Debug, Clone, PartialEq)]
pub struct Circuit {
    /// Angular momentum on center A (first bra center).
    pub la: usize,
    /// Angular momentum on center B (second bra center).
    pub lb: usize,
    /// Angular momentum on center C (first ket center).
    pub lc: usize,
    /// Angular momentum on center D (second ket center).
    pub ld: usize,
    /// Maximum bra angular momentum index: nmax = la + lb.
    pub nmax: usize,
    /// Maximum ket angular momentum index: mmax = lc + ld.
    pub mmax: usize,
    /// The ordered list of R1CS gates comprising the circuit.
    pub gates: Vec<VrrGate>,
    /// Total number of operand slots allocated (including the base case).
    pub n_operands: usize,
    /// Total number of prefactor slots allocated (always 7 for v3.2.0).
    pub n_prefactors: usize,
    /// Index of the operand holding the final result.
    pub result_operand: usize,
    /// Number of Rys roots = (nmax + mmax) / 2 + 1.
    ///
    /// This is the order of Rys quadrature needed for this shell quartet.
    /// For (ss|ss) this is 1, for (pp|pp) this is 3, etc.
    pub rys_order: usize,
    /// VRR table index: `g_idx[n][m]` = operand index for `g[n][m]`.
    ///
    /// Dimensions: `(nmax + 1) × (mmax + 1)`.
    /// Row `n` contains operand indices for `g[n][0], g[n][1], ..., g[n][mmax]`.
    pub g_idx: Vec<Vec<usize>>,
    /// HRR-Bra intermediate map: `hrr_bra_map[m]` = list of [`HrrIndex`] entries
    /// for `I(na, nb | m, 0)`, covering ALL `na + nb ≤ nmax` with `nb ≤ lb`.
    ///
    /// For `m = 0..mmax`, each entry list contains ALL HRR-Bra intermediates
    /// for that ket level, including the `nb = 0` row (which corresponds to
    /// the VRR table entries `g[na][m]`).
    ///
    /// The shell driver can use these to look up `I(na, nb | m, 0)` for any
    /// valid `(na, nb)` combination, enabling computation of Cartesian components
    /// that don't use the full `(la, lb)` bra pair.
    pub hrr_bra_map: Vec<Vec<HrrIndex>>,
    /// HRR-Ket intermediate map: list of [`HrrIndex`] entries for
    /// `I(la, lb | nc, nd)`, covering ALL `nc + nd ≤ mmax` with `nd ≤ ld`.
    ///
    /// This includes the `nd = 0` row (which corresponds to the HRR-Bra results
    /// `I(la, lb | nc, 0)`) and ALL intermediate rows up to `nd = ld`.
    ///
    /// Note: ket-side intermediates are only recorded for the fixed bra pair
    /// `(la, lb)`. For other bra pairs `(na, nb) ≠ (la, lb)` with `nd > 0`,
    /// the shell driver must apply HRR-Ket manually using the `hrr_bra_map`
    /// entries as starting points.
    pub hrr_ket_map: Vec<HrrIndex>,
}

impl Circuit {
    /// Returns the shell quartet label in spectroscopic notation.
    pub fn quartet_label(&self) -> String {
        format!(
            "({}{}|{}{})",
            shell_name(self.la),
            shell_name(self.lb),
            shell_name(self.lc),
            shell_name(self.ld)
        )
    }

    /// Returns the number of gates in each pipeline stage.
    ///
    /// Returns a 5-tuple `(n_vrr_bra, n_vrr_ket, n_vrr_full, n_hrr_bra, n_hrr_ket)`.
    pub fn stage_counts(&self) -> (usize, usize, usize, usize, usize) {
        let mut counts = [0usize; 5];
        for gate in &self.gates {
            if gate.stage < 5 {
                counts[gate.stage] += 1;
            }
        }
        (counts[0], counts[1], counts[2], counts[3], counts[4])
    }

    /// Returns true if this is the trivial (ss|ss) base case with zero gates.
    pub fn is_base_case(&self) -> bool {
        self.gates.is_empty()
    }

    /// Look up the operand index for `I(na, nb | nc, nd)`.
    ///
    /// Returns `Some(operand_index)` if the circuit computes this integral
    /// component, or `None` if it is not available.
    ///
    /// # Lookup Strategy
    ///
    /// 1. **`nb == 0 && nd == 0`**: Direct VRR table lookup via `g_idx[na][nc]`
    /// 2. **`nd == 0 && nb > 0`**: HRR-Bra intermediate lookup in
    ///    `hrr_bra_map[nc]` for the entry with `(i=na, j=nb)`
    /// 3. **`nd > 0 && na == la && nb == lb`**: HRR-Ket intermediate lookup
    ///    in `hrr_ket_map` for the entry with `(i=nc, j=nd)`
    /// 4. **Otherwise**: Returns `None` — the shell driver must apply HRR-Ket
    ///    manually using `hrr_bra_map` entries as starting points
    ///
    /// # Panics
    ///
    /// Does not panic; returns `None` for out-of-bounds or unavailable entries.
    pub fn find_operand(&self, na: usize, nb: usize, nc: usize, nd: usize) -> Option<usize> {
        // Validate angular momentum bounds
        if na + nb > self.nmax || nb > self.lb {
            return None;
        }
        if nc + nd > self.mmax || nd > self.ld {
            return None;
        }

        // Case 1: No HRR needed on either side — direct VRR table lookup
        if nb == 0 && nd == 0 {
            if na < self.g_idx.len() && nc < self.g_idx[na].len() {
                return Some(self.g_idx[na][nc]);
            }
            return None;
        }

        // Case 2: HRR-Bra only (nd == 0) — look in hrr_bra_map
        if nd == 0 {
            if nc < self.hrr_bra_map.len() {
                for entry in &self.hrr_bra_map[nc] {
                    if entry.i == na && entry.j == nb {
                        return Some(entry.operand);
                    }
                }
            }
            return None;
        }

        // Case 3: HRR-Ket needed (nd > 0) — only available for (na, nb) == (la, lb)
        if na == self.la && nb == self.lb {
            for entry in &self.hrr_ket_map {
                if entry.i == nc && entry.j == nd {
                    return Some(entry.operand);
                }
            }
        }

        None
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CircuitBuilder: Constructs the R1CS circuit for a shell quartet
// ─────────────────────────────────────────────────────────────────────────────

/// The prefactor indices for the v3.2.0 circuit.
///
/// These are fixed indices into the 7-element prefactor array.
pub mod prefactor_indices {
    /// c00 = PA_i (root-dependent: PA_i + t * PQ_i for Rys)
    pub const C00: usize = 0;
    /// c0p = QC_i (root-dependent: QC_i + t * PQ_i for Rys)
    pub const C0P: usize = 1;
    /// b10 = 1/(2p) (root-independent)
    pub const B10: usize = 2;
    /// b01 = 1/(2q) (root-independent)
    pub const B01: usize = 3;
    /// b00 = t/(2(p+q)) (root-dependent)
    pub const B00: usize = 4;
    /// AB = -(A_i - B_i) (negated for HRR formula; root-independent)
    pub const AB: usize = 5;
    /// CD = -(C_i - D_i) (negated for HRR formula; root-independent)
    pub const CD: usize = 6;
    /// Total number of prefactors
    pub const TOTAL: usize = 7;
}

/// Builder for constructing an R1CS circuit from a shell quartet specification.
///
/// The builder implements the 5-stage VRR+HRR pipeline with the 2D VRR table
/// approach, allocating operand slots as needed and emitting [`VrrGate`]s that
/// encode the recurrence relations.
///
/// # Usage
///
/// ```rust
/// use poler_eri::CircuitBuilder;
///
/// let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
/// assert_eq!(circuit.la, 1);
/// assert!(!circuit.gates.is_empty());
/// assert_eq!(circuit.n_prefactors, 7); // Fixed prefactor count in v3.2.0
/// ```
///
/// # Operand Allocation
///
/// Operand slot 0 is always reserved for the base case `g[0][0] = 1.0`.
/// The 2D VRR table is allocated first, then HRR intermediates:
///
/// ```text
/// Slot 0:           g[0][0] = 1.0     — base case, pre-populated
/// Slot 1..nmax:     g[1..=nmax][0]    — VRR-Bra results
/// Slot nmax+1..:    g[0][1..=mmax]    — VRR-Ket results
/// Slot ...:         g[n][m] n>0,m>0   — VRR-Full results
/// Slot ...:         HRR-Bra intermediates
/// Slot ...:         HRR-Ket intermediates
/// ```
///
/// # Prefactor Allocation
///
/// Exactly 7 prefactor slots (fixed layout):
///
/// ```text
/// [0] c00, [1] c0p, [2] b10, [3] b01, [4] b00, [5] -AB, [6] -CD
/// ```
#[derive(Debug)]
pub struct CircuitBuilder {
    /// Angular momentum on center A.
    la: usize,
    /// Angular momentum on center B.
    lb: usize,
    /// Angular momentum on center C.
    lc: usize,
    /// Angular momentum on center D.
    ld: usize,
    /// Next operand slot to allocate.
    next_operand: usize,
}

impl CircuitBuilder {
    /// Create a new circuit builder for the shell quartet `(la, lb | lc, ld)`.
    ///
    /// # Panics
    ///
    /// Panics if `la < lb` or `lc < ld` (permutational symmetry violation).
    pub fn new(la: usize, lb: usize, lc: usize, ld: usize) -> Self {
        assert!(
            la >= lb,
            "Permutational symmetry requires la ({}) >= lb ({})",
            la, lb
        );
        assert!(
            lc >= ld,
            "Permutational symmetry requires lc ({}) >= ld ({})",
            lc, ld
        );
        CircuitBuilder {
            la,
            lb,
            lc,
            ld,
            next_operand: 0,
        }
    }

    /// Allocate the next operand slot and return its index.
    fn alloc_operand(&mut self) -> usize {
        let idx = self.next_operand;
        self.next_operand += 1;
        idx
    }

    /// Build the complete R1CS circuit for this shell quartet.
    ///
    /// This method implements the 5-stage 2D VRR + HRR pipeline:
    ///
    /// ## Stage 0: VRR-Bra
    ///
    /// Build `g[n][0]` for `n = 0..nmax` using the bra VRR recurrence:
    ///
    /// ```text
    /// g[0][0] = 1.0                    (base case)
    /// g[1][0] = c00 * g[0][0]          (first step, no decrement)
    /// g[n][0] = c00 * g[n-1][0] + (n-1) * b10 * g[n-2][0]   (n >= 2)
    /// ```
    ///
    /// ## Stage 1: VRR-Ket
    ///
    /// Build `g[0][m]` for `m = 0..mmax` using the ket VRR recurrence:
    ///
    /// ```text
    /// g[0][1] = c0p * g[0][0]          (first step, no decrement)
    /// g[0][m] = c0p * g[0][m-1] + (m-1) * b01 * g[0][m-2]   (m >= 2)
    /// ```
    ///
    /// ## Stage 2: VRR-Full
    ///
    /// Build `g[n][m]` for `n >= 1, m >= 1` using the full VRR with cross-term:
    ///
    /// ```text
    /// g[1][m] = c00 * g[0][m] + m * b00 * g[0][m-1]          (n=1, no bra decrement)
    /// g[n][m] = c00 * g[n-1][m] + (n-1) * b10 * g[n-2][m] + m * b00 * g[n-1][m-1]  (n >= 2)
    /// ```
    ///
    /// For the 3-term case (n >= 2), two gates are needed:
    /// ```text
    /// temp = c00 * g[n-1][m] + (n-1) * b10 * g[n-2][m]
    /// g[n][m] = 1.0 * temp + m * b00 * g[n-1][m-1]
    /// ```
    ///
    /// ## Stage 3: HRR-Bra
    ///
    /// Expand from `(nmax, 0 | m, 0)` to `(la, lb | m, 0)` for each `m = 0..mmax`.
    ///
    /// The HRR formula: `I(i, j) = I(i+1, j-1) - AB * I(i, j-1)`
    ///
    /// In gate form with negated AB:
    /// `I(i, j) = One * I(i+1, j-1) + Prefactor(AB_idx) * I(i, j-1)`
    ///
    /// ## Stage 4: HRR-Ket
    ///
    /// Expand from `(la, lb | mmax, 0)` to `(la, lb | lc, ld)`.
    ///
    /// The HRR formula: `I(k, l) = I(k+1, l-1) - CD * I(k, l-1)`
    ///
    /// In gate form with negated CD:
    /// `I(k, l) = One * I(k+1, l-1) + Prefactor(CD_idx) * I(k, l-1)`
    #[allow(clippy::needless_range_loop)]
    pub fn build_circuit(mut self) -> Circuit {
        let la = self.la;
        let lb = self.lb;
        let lc = self.lc;
        let ld = self.ld;
        let nmax = la + lb;
        let mmax = lc + ld;

        let mut gates = Vec::new();

        // ── Allocate the base case operand: operands[0] = g[0][0] = 1.0 ──
        let base = self.alloc_operand();
        debug_assert_eq!(base, 0, "Base operand must be slot 0");

        // ── 2D VRR table: allocate operand slots for g[n][m] ──
        //
        // Layout:
        //   g[0][0] = operand 0 (base case)
        //   g[1][0] .. g[nmax][0] = operands 1..nmax
        //   g[0][1] .. g[0][mmax] = operands nmax+1..nmax+mmax
        //   g[n][m] for n>0, m>0  = operands allocated sequentially

        let mut g_idx = vec![vec![0usize; mmax + 1]; nmax + 1];
        g_idx[0][0] = base;

        // Allocate VRR-Bra column: g[n][0] for n = 1..nmax
        for n in 1..=nmax {
            g_idx[n][0] = self.alloc_operand();
        }

        // Allocate VRR-Ket row: g[0][m] for m = 1..mmax
        for m in 1..=mmax {
            g_idx[0][m] = self.alloc_operand();
        }

        // Allocate VRR-Full interior: g[n][m] for n >= 1, m >= 1
        // We iterate in column-major order (m first, then n) so that
        // g[n][m-1] is always available when we compute g[n][m]
        for m in 1..=mmax {
            for n in 1..=nmax {
                g_idx[n][m] = self.alloc_operand();
            }
        }

        // Prefactor indices (fixed layout)
        let pf_c00 = prefactor_indices::C00;
        let pf_c0p = prefactor_indices::C0P;
        let pf_b10 = prefactor_indices::B10;
        let pf_b01 = prefactor_indices::B01;
        let pf_b00 = prefactor_indices::B00;
        let pf_ab = prefactor_indices::AB;
        let pf_cd = prefactor_indices::CD;

        // ─────────────────────────────────────────────────────────────────
        // Stage 0: VRR-Bra — Build g[n][0] for n = 0..nmax
        // ─────────────────────────────────────────────────────────────────
        //
        // g[0][0] = 1.0 (base case, already in operands[0])
        // g[1][0] = c00 * g[0][0]
        // g[n][0] = c00 * g[n-1][0] + (n-1) * b10 * g[n-2][0]  for n >= 2

        if nmax >= 1 {
            // g[1][0] = c00 * g[0][0]
            gates.push(VrrGate {
                result: g_idx[1][0],
                left: g_idx[0][0],
                right: g_idx[0][0], // unused (coeff_right = Zero)
                coeff_left: GateCoeff::Prefactor(pf_c00),
                coeff_right: GateCoeff::Zero,
                stage: 0,
                label: "VRR-Bra: g[1][0] = c00 * g[0][0]".to_string(),
            });
        }

        for n in 2..=nmax {
            // g[n][0] = c00 * g[n-1][0] + (n-1) * b10 * g[n-2][0]
            gates.push(VrrGate {
                result: g_idx[n][0],
                left: g_idx[n - 1][0],
                right: g_idx[n - 2][0],
                coeff_left: GateCoeff::Prefactor(pf_c00),
                coeff_right: GateCoeff::ScaledPrefactor((n - 1) as f64, pf_b10),
                stage: 0,
                label: format!("VRR-Bra: g[{}][0] = c00*g[{}][0] + {}*b10*g[{}][0]", n, n - 1, n - 1, n - 2),
            });
        }

        // ─────────────────────────────────────────────────────────────────
        // Stage 1: VRR-Ket — Build g[0][m] for m = 0..mmax
        // ─────────────────────────────────────────────────────────────────
        //
        // g[0][0] = 1.0 (already in operands[0])
        // g[0][1] = c0p * g[0][0]
        // g[0][m] = c0p * g[0][m-1] + (m-1) * b01 * g[0][m-2]  for m >= 2

        if mmax >= 1 {
            // g[0][1] = c0p * g[0][0]
            gates.push(VrrGate {
                result: g_idx[0][1],
                left: g_idx[0][0],
                right: g_idx[0][0], // unused (coeff_right = Zero)
                coeff_left: GateCoeff::Prefactor(pf_c0p),
                coeff_right: GateCoeff::Zero,
                stage: 1,
                label: "VRR-Ket: g[0][1] = c0p * g[0][0]".to_string(),
            });
        }

        for m in 2..=mmax {
            // g[0][m] = c0p * g[0][m-1] + (m-1) * b01 * g[0][m-2]
            gates.push(VrrGate {
                result: g_idx[0][m],
                left: g_idx[0][m - 1],
                right: g_idx[0][m - 2],
                coeff_left: GateCoeff::Prefactor(pf_c0p),
                coeff_right: GateCoeff::ScaledPrefactor((m - 1) as f64, pf_b01),
                stage: 1,
                label: format!("VRR-Ket: g[0][{}] = c0p*g[0][{}] + {}*b01*g[0][{}]", m, m - 1, m - 1, m - 2),
            });
        }

        // ─────────────────────────────────────────────────────────────────
        // Stage 2: VRR-Full — Build g[n][m] for n >= 1, m >= 1
        // ─────────────────────────────────────────────────────────────────
        //
        // g[1][m] = c00 * g[0][m] + m * b00 * g[0][m-1]          (n=1, no bra decrement)
        // g[n][m] = c00 * g[n-1][m] + (n-1) * b10 * g[n-2][m] + m * b00 * g[n-1][m-1]  (n >= 2)
        //
        // For n >= 2, we need 2 gates:
        //   temp = c00 * g[n-1][m] + (n-1) * b10 * g[n-2][m]
        //   g[n][m] = One * temp + m * b00 * g[n-1][m-1]

        for m in 1..=mmax {
            for n in 1..=nmax {
                if n == 1 {
                    // g[1][m] = c00 * g[0][m] + m * b00 * g[0][m-1]
                    gates.push(VrrGate {
                        result: g_idx[1][m],
                        left: g_idx[0][m],
                        right: g_idx[0][m - 1],
                        coeff_left: GateCoeff::Prefactor(pf_c00),
                        coeff_right: GateCoeff::ScaledPrefactor(m as f64, pf_b00),
                        stage: 2,
                        label: format!(
                            "VRR-Full: g[1][{}] = c00*g[0][{}] + {}*b00*g[0][{}]",
                            m, m, m, m - 1
                        ),
                    });
                } else {
                    // n >= 2: 3-term sum, needs 2 gates
                    // Gate 1: temp = c00 * g[n-1][m] + (n-1) * b10 * g[n-2][m]
                    let temp = self.alloc_operand();
                    gates.push(VrrGate {
                        result: temp,
                        left: g_idx[n - 1][m],
                        right: g_idx[n - 2][m],
                        coeff_left: GateCoeff::Prefactor(pf_c00),
                        coeff_right: GateCoeff::ScaledPrefactor((n - 1) as f64, pf_b10),
                        stage: 2,
                        label: format!(
                            "VRR-Full(p1): temp = c00*g[{}][{}] + {}*b10*g[{}][{}]",
                            n - 1, m, n - 1, n - 2, m
                        ),
                    });

                    // Gate 2: g[n][m] = 1.0 * temp + m * b00 * g[n-1][m-1]
                    gates.push(VrrGate {
                        result: g_idx[n][m],
                        left: temp,
                        right: g_idx[n - 1][m - 1],
                        coeff_left: GateCoeff::One,
                        coeff_right: GateCoeff::ScaledPrefactor(m as f64, pf_b00),
                        stage: 2,
                        label: format!(
                            "VRR-Full(p2): g[{}][{}] = temp + {}*b00*g[{}][{}]",
                            n, m, m, n - 1, m - 1
                        ),
                    });
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────
        // Stage 3: HRR-Bra — Expand from (nmax,0|m,0) to (la,lb|m,0)
        // ─────────────────────────────────────────────────────────────────
        //
        // For each ket level m = 0..mmax, apply the HRR to transfer
        // lb units of angular momentum from center A to center B.
        //
        // The HRR builds a triangular table for each m:
        //   Row j=0: I(i, 0 | m, 0) = g[i][m] for i = 0..nmax  (VRR results)
        //   Row j=1: I(i, 1 | m, 0) for i = 0..(nmax-1)
        //   Row j=2: I(i, 2 | m, 0) for i = 0..(nmax-2)
        //   ...
        //   Row j=lb: I(la, lb | m, 0) (the target for this m)
        //
        // Formula: I(i, j) = I(i+1, j-1) + (-AB) * I(i, j-1)
        //   = One * I(i+1, j-1) + Prefactor(AB_idx) * I(i, j-1)
        //
        // v3.2.0: We record ALL intermediates in hrr_bra_map[m], not just
        // the final I(la, lb | m, 0).

        // Initialize hrr_bra_map: one entry list per ket level m
        let mut hrr_bra_map: Vec<Vec<HrrIndex>> = vec![Vec::new(); mmax + 1];

        // Store the final HRR-bra result for each m: I(la, lb | m, 0)
        let mut hrr_bra_result = vec![0usize; mmax + 1];

        for m in 0..=mmax {
            // Row j=0 (nb=0): I(i, 0 | m, 0) = g[i][m] for i = 0..nmax
            // Record ALL j=0 entries in hrr_bra_map[m]
            for i in 0..=nmax {
                hrr_bra_map[m].push(HrrIndex {
                    j: 0,
                    i,
                    operand: g_idx[i][m],
                });
            }

            if lb > 0 {
                // Build rows j = 1, 2, ..., lb
                // We'll track the current row as a vector of operand indices
                let mut prev_row: Vec<usize> = Vec::with_capacity(nmax + 1);
                for i in 0..=nmax {
                    prev_row.push(g_idx[i][m]);
                }

                for j in 1..=lb {
                    let mut curr_row: Vec<usize> = Vec::with_capacity(nmax);

                    // I(i, j) = I(i+1, j-1) + (-AB) * I(i, j-1)
                    // for i = 0..(nmax - j)
                    for i in 0..=(nmax - j) {
                        let result = self.alloc_operand();

                        // I(i+1, j-1) is prev_row[i+1]
                        // I(i, j-1) is prev_row[i]
                        gates.push(VrrGate {
                            result,
                            left: prev_row[i + 1],
                            right: prev_row[i],
                            coeff_left: GateCoeff::One,
                            coeff_right: GateCoeff::Prefactor(pf_ab),
                            stage: 3,
                            label: format!(
                                "HRR-Bra: I({},{})|{},0 from I({},{}|{},0) + AB*I({},{}|{},0) [m={}]",
                                i, j, m, i + 1, j - 1, m, i, j - 1, m, m
                            ),
                        });

                        curr_row.push(result);

                        // Record ALL entries in hrr_bra_map[m]
                        hrr_bra_map[m].push(HrrIndex {
                            j,
                            i,
                            operand: result,
                        });
                    }

                    prev_row = curr_row;
                }

                // After lb HRR steps, prev_row has la+1 entries (i=0..la).
                // prev_row[la] = I(la, lb | m, 0)
                debug_assert_eq!(prev_row.len(), la + 1,
                    "After {} HRR-bra steps, should have {} entries, got {}",
                    lb, la + 1, prev_row.len());
                hrr_bra_result[m] = prev_row[la];
            } else {
                // lb = 0: No HRR-bra needed. I(la, 0 | m, 0) = g[la][m]
                hrr_bra_result[m] = g_idx[la][m];
            }
        }

        // ─────────────────────────────────────────────────────────────────
        // Stage 4: HRR-Ket — Expand from (la,lb|mmax,0) to (la,lb|lc,ld)
        // ─────────────────────────────────────────────────────────────────
        //
        // We need I(la, lb | k, l) for k+l = mmax, l = ld.
        // The HRR transfers ld units from center C to center D.
        //
        // v3.2.0: We record ALL intermediates in hrr_ket_map, not just
        // the final I(la, lb | lc, ld).

        let mut hrr_ket_map: Vec<HrrIndex> = Vec::new();

        // Record the nd=0 (l=0) row: I(la, lb | k, 0) for k = 0..mmax
        for k in 0..=mmax {
            hrr_ket_map.push(HrrIndex {
                j: 0,
                i: k,
                operand: hrr_bra_result[k],
            });
        }

        let mut result_operand = if ld > 0 {
            // Start with HRR-bra results as row l=0
            let mut prev_row: Vec<usize> = Vec::with_capacity(mmax + 1);
            for m in 0..=mmax {
                prev_row.push(hrr_bra_result[m]);
            }

            for l in 1..=ld {
                let mut curr_row: Vec<usize> = Vec::with_capacity(mmax);

                // I(la, lb | k, l) = I(la, lb | k+1, l-1) + (-CD) * I(la, lb | k, l-1)
                for k in 0..=(mmax - l) {
                    let result = self.alloc_operand();

                    gates.push(VrrGate {
                        result,
                        left: prev_row[k + 1],
                        right: prev_row[k],
                        coeff_left: GateCoeff::One,
                        coeff_right: GateCoeff::Prefactor(pf_cd),
                        stage: 4,
                        label: format!(
                            "HRR-Ket: I({},{})|{},{}) from I({},{})|{},{}) + CD*I({},{})|{},{})",
                            la, lb, k, l,
                            la, lb, k + 1, l - 1,
                            la, lb, k, l - 1
                        ),
                    });

                    curr_row.push(result);

                    // Record ALL entries in hrr_ket_map
                    hrr_ket_map.push(HrrIndex {
                        j: l,
                        i: k,
                        operand: result,
                    });
                }

                prev_row = curr_row;
            }

            // After ld HRR-ket steps, prev_row has lc+1 entries.
            // prev_row[lc] = I(la, lb | lc, ld)
            debug_assert_eq!(prev_row.len(), lc + 1,
                "After {} HRR-ket steps, should have {} entries, got {}",
                ld, lc + 1, prev_row.len());
            prev_row[lc]
        } else {
            // ld = 0: No HRR-ket needed. Result = I(la, lb | mmax, 0) = hrr_bra_result[mmax]
            hrr_bra_result[mmax]
        };

        // For the (ss|ss) case with no gates, the result is just the base operand
        if gates.is_empty() {
            result_operand = base;
        }

        // Compute Rys order: number of Rys roots needed
        let rys_order = (nmax + mmax) / 2 + 1;

        Circuit {
            la,
            lb,
            lc,
            ld,
            nmax,
            mmax,
            gates,
            n_operands: self.next_operand,
            n_prefactors: prefactor_indices::TOTAL,
            result_operand,
            rys_order,
            g_idx,
            hrr_bra_map,
            hrr_ket_map,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Circuit Evaluation
// ─────────────────────────────────────────────────────────────────────────────

/// Evaluate all gates in a circuit, writing results into the operands array.
///
/// This function executes the circuit in gate order, computing each gate's
/// result as:
///
/// ```text
/// operands[result] = resolve(coeff_left) × operands[left]
///                  + resolve(coeff_right) × operands[right]
/// ```
///
/// # Arguments
///
/// - `circuit` — The circuit to evaluate
/// - `operands` — Mutable operand array. Must be pre-populated with the base
///   case in `operands[0]` (= 1.0) and have sufficient capacity.
/// - `prefactors` — Prefactor array with 7 runtime values. Must have at least
///   `circuit.n_prefactors` elements.
///
/// # Panics
///
/// Panics if `prefactors` is too short or if any operand index is out of bounds.
pub fn evaluate_circuit(circuit: &Circuit, operands: &mut Vec<f64>, prefactors: &[f64]) {
    // Ensure the operand array has enough slots for all results
    if operands.len() < circuit.n_operands {
        operands.resize(circuit.n_operands, 0.0);
    }

    for gate in &circuit.gates {
        let cl = resolve_coeff(&gate.coeff_left, operands, prefactors);
        let cr = resolve_coeff(&gate.coeff_right, operands, prefactors);

        let left_val = operands[gate.left];
        let right_val = operands[gate.right];
        operands[gate.result] = cl * left_val + cr * right_val;
    }
}

/// Resolve a [`GateCoeff`] to its numerical value given current operands and prefactors.
fn resolve_coeff(coeff: &GateCoeff, operands: &[f64], prefactors: &[f64]) -> f64 {
    match coeff {
        GateCoeff::Zero => 0.0,
        GateCoeff::One => 1.0,
        GateCoeff::Scalar(s) => *s,
        GateCoeff::BoyF(m) => {
            // Legacy: evaluate F_m(0) = 1/(2m+1) as placeholder
            boys_f(*m, 0.0)
        }
        GateCoeff::Prefactor(i) => {
            assert!(
                *i < prefactors.len(),
                "Prefactor index {} out of bounds (prefactors has {} elements)",
                i,
                prefactors.len()
            );
            prefactors[*i]
        }
        GateCoeff::Operand(i) => {
            assert!(
                *i < operands.len(),
                "Operand index {} out of bounds (operands has {} elements)",
                i,
                operands.len()
            );
            operands[*i]
        }
        GateCoeff::ScaledPrefactor(scalar, i) => {
            assert!(
                *i < prefactors.len(),
                "Prefactor index {} out of bounds (prefactors has {} elements)",
                i,
                prefactors.len()
            );
            *scalar * prefactors[*i]
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── GateCoeff tests ──

    #[test]
    fn test_gate_coeff_equality() {
        assert_eq!(GateCoeff::Zero, GateCoeff::Zero);
        assert_eq!(GateCoeff::One, GateCoeff::One);
        assert_eq!(GateCoeff::Scalar(1.5), GateCoeff::Scalar(1.5));
        assert_eq!(GateCoeff::BoyF(3), GateCoeff::BoyF(3));
        assert_eq!(GateCoeff::Prefactor(2), GateCoeff::Prefactor(2));
        assert_eq!(GateCoeff::Operand(5), GateCoeff::Operand(5));
        assert_eq!(GateCoeff::ScaledPrefactor(2.0, 3), GateCoeff::ScaledPrefactor(2.0, 3));
    }

    // ── HrrIndex tests ──

    #[test]
    fn test_hrr_index_equality() {
        let a = HrrIndex { j: 1, i: 2, operand: 5 };
        let b = HrrIndex { j: 1, i: 2, operand: 5 };
        let c = HrrIndex { j: 1, i: 2, operand: 6 };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // ── CircuitBuilder tests ──

    #[test]
    fn test_circuit_ssss() {
        let circuit = CircuitBuilder::new(0, 0, 0, 0).build_circuit();
        assert_eq!(circuit.la, 0);
        assert_eq!(circuit.lb, 0);
        assert_eq!(circuit.lc, 0);
        assert_eq!(circuit.ld, 0);
        assert_eq!(circuit.nmax, 0);
        assert_eq!(circuit.mmax, 0);
        assert!(circuit.is_base_case());
        assert_eq!(circuit.gates.len(), 0);
        assert_eq!(circuit.n_operands, 1);
        assert_eq!(circuit.n_prefactors, 7);
        assert_eq!(circuit.rys_order, 1); // (0+0)/2 + 1 = 1
    }

    #[test]
    fn test_circuit_psss() {
        let circuit = CircuitBuilder::new(1, 0, 0, 0).build_circuit();
        assert_eq!(circuit.la, 1);
        assert_eq!(circuit.nmax, 1);
        assert_eq!(circuit.mmax, 0);
        assert!(!circuit.is_base_case());
        // Stage 0 (VRR-Bra): 1 gate (g[1][0] = c00 * g[0][0])
        // Stages 1,2: 0 gates (mmax=0, no ket VRR)
        // Stage 3 (HRR-Bra): 0 gates (lb=0)
        // Stage 4 (HRR-Ket): 0 gates (ld=0)
        let (s0, s1, s2, s3, s4) = circuit.stage_counts();
        assert_eq!(s0, 1); // VRR-Bra
        assert_eq!(s1, 0); // VRR-Ket (mmax=0)
        assert_eq!(s2, 0); // VRR-Full (no interior)
        assert_eq!(s3, 0); // HRR-Bra (lb=0)
        assert_eq!(s4, 0); // HRR-Ket (ld=0)
        assert_eq!(circuit.rys_order, 1); // (1+0)/2 + 1 = 1
    }

    #[test]
    fn test_circuit_ppss() {
        let circuit = CircuitBuilder::new(1, 1, 0, 0).build_circuit();
        assert_eq!(circuit.nmax, 2);
        assert_eq!(circuit.mmax, 0);
        let (s0, s1, s2, s3, s4) = circuit.stage_counts();
        assert_eq!(s0, 2); // VRR-Bra: g[1][0], g[2][0]
        assert_eq!(s1, 0); // VRR-Ket (mmax=0)
        assert_eq!(s2, 0); // VRR-Full
        // HRR-Bra: 1 step (transfer 1 unit from A to B)
        // For m=0 only: I(0,1|0,0) and I(1,1|0,0) = 2 gates
        assert!(s3 >= 1, "HRR-Bra should have at least 1 gate for (pp|ss)");
        assert_eq!(s4, 0); // HRR-Ket (ld=0)
        assert_eq!(circuit.rys_order, 2); // (2+0)/2 + 1 = 2
    }

    #[test]
    fn test_circuit_pppp() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        assert_eq!(circuit.nmax, 2);
        assert_eq!(circuit.mmax, 2);
        assert!(!circuit.is_base_case());

        let (s0, s1, s2, s3, s4) = circuit.stage_counts();

        // VRR-Bra: g[1][0], g[2][0] = 2 gates
        assert_eq!(s0, 2);

        // VRR-Ket: g[0][1], g[0][2] = 2 gates
        assert_eq!(s1, 2);

        // VRR-Full: g[1][1], g[2][1], g[1][2], g[2][2]
        // g[1][1]: 1 gate (2-term)
        // g[2][1]: 2 gates (3-term)
        // g[1][2]: 1 gate (2-term)
        // g[2][2]: 2 gates (3-term)
        // Total: 6 gates
        assert_eq!(s2, 6);

        // HRR-Bra: for each m=0,1,2, compute I(1,1|m,0)
        // For each m, 1 HRR step with nmax=2:
        //   I(0,1|m,0) and I(1,1|m,0) = 2 gates per m
        // Total: 3 * 2 = 6 gates
        assert_eq!(s3, 6);

        // HRR-Ket: ld=1, 1 step with mmax=2
        // I(1,1|0,1), I(1,1|1,1) = 2 gates
        assert_eq!(s4, 2);

        // Total gates
        assert_eq!(circuit.gates.len(), 2 + 2 + 6 + 6 + 2);

        assert_eq!(circuit.rys_order, 3); // (2+2)/2 + 1 = 3
    }

    #[test]
    fn test_circuit_dddd() {
        let circuit = CircuitBuilder::new(2, 2, 2, 2).build_circuit();
        assert_eq!(circuit.nmax, 4);
        assert_eq!(circuit.mmax, 4);

        let (s0, s1, s2, s3, s4) = circuit.stage_counts();

        // VRR-Bra: nmax=4, gates for g[1][0]..g[4][0]
        // g[1][0]: 1 gate, g[2][0]: 1 gate, g[3][0]: 1 gate, g[4][0]: 1 gate
        assert_eq!(s0, 4);

        // VRR-Ket: mmax=4
        assert_eq!(s1, 4);

        // VRR-Full: should be substantial
        assert!(s2 > 0);

        // HRR-Bra and HRR-Ket should have gates
        assert!(s3 > 0);
        assert!(s4 > 0);

        // Total should be significantly more than v3.0.x (which had 8 gates)
        assert!(circuit.gates.len() > 8, "(dd|dd) should have many more gates than v3.0.x");

        assert_eq!(circuit.rys_order, 5); // (4+4)/2 + 1 = 5
    }

    #[test]
    fn test_prefactor_count_is_7() {
        // All circuits should have exactly 7 prefactors
        for la in 0..=2 {
            for lb in 0..=la {
                for lc in 0..=2 {
                    for ld in 0..=lc {
                        let circuit = CircuitBuilder::new(la, lb, lc, ld).build_circuit();
                        assert_eq!(circuit.n_prefactors, 7,
                            "({}{}|{}{}) should have 7 prefactors, got {}",
                            shell_name(la), shell_name(lb), shell_name(lc), shell_name(ld),
                            circuit.n_prefactors);
                    }
                }
            }
        }
    }

    #[test]
    fn test_stage_counts_ssss() {
        let circuit = CircuitBuilder::new(0, 0, 0, 0).build_circuit();
        let (s0, s1, s2, s3, s4) = circuit.stage_counts();
        assert_eq!((s0, s1, s2, s3, s4), (0, 0, 0, 0, 0));
    }

    #[test]
    fn test_quartet_label() {
        let c = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        assert_eq!(c.quartet_label(), "(pp|pp)");

        let c2 = CircuitBuilder::new(2, 1, 0, 0).build_circuit();
        assert_eq!(c2.quartet_label(), "(dp|ss)");
    }

    // ── evaluate_circuit tests ──

    #[test]
    fn test_evaluate_ssss() {
        let circuit = CircuitBuilder::new(0, 0, 0, 0).build_circuit();
        let mut operands = vec![1.0];
        let prefactors = [0.0; 7];
        evaluate_circuit(&circuit, &mut operands, &prefactors);
        assert_eq!(operands[0], 1.0);
    }

    #[test]
    fn test_evaluate_psss() {
        let circuit = CircuitBuilder::new(1, 0, 0, 0).build_circuit();
        let mut operands = vec![1.0]; // g[0][0] = 1.0
        let prefactors = [0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // c00 = 0.5
        evaluate_circuit(&circuit, &mut operands, &prefactors);
        // g[1][0] = c00 * g[0][0] = 0.5 * 1.0 = 0.5
        assert!((operands[circuit.result_operand] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_evaluate_ppss() {
        let circuit = CircuitBuilder::new(1, 1, 0, 0).build_circuit();
        let mut operands = vec![1.0]; // g[0][0] = 1.0
        // c00=0.5, b10=0.25, AB=-0.3 (negated for HRR)
        let prefactors = [0.5, 0.0, 0.25, 0.0, 0.0, -0.3, 0.0];
        evaluate_circuit(&circuit, &mut operands, &prefactors);
        // g[1][0] = 0.5 * 1.0 = 0.5
        // g[2][0] = 0.5 * 0.5 + 1.0 * 0.25 * 1.0 = 0.25 + 0.25 = 0.5
        // HRR: I(1,1|0,0) = I(2,0|0,0) + (-0.3) * I(1,0|0,0) = 0.5 + (-0.3) * 0.5 = 0.35
        // But wait, we also compute I(0,1|0,0) = I(1,0|0,0) + (-0.3) * I(0,0|0,0) = 0.5 + (-0.3) * 1.0 = 0.2
        // And I(1,1|0,0) = I(2,0|0,0) + (-0.3) * I(1,0|0,0) = 0.5 + (-0.3) * 0.5 = 0.35
        assert!((operands[circuit.result_operand] - 0.35).abs() < 1e-10);
    }

    #[test]
    fn test_evaluate_scaled_prefactor() {
        // Test that ScaledPrefactor resolves correctly
        let circuit = CircuitBuilder::new(2, 0, 0, 0).build_circuit();
        let mut operands = vec![1.0]; // g[0][0] = 1.0
        // c00=0.5, b10=0.1
        let prefactors = [0.5, 0.0, 0.1, 0.0, 0.0, 0.0, 0.0];
        evaluate_circuit(&circuit, &mut operands, &prefactors);
        // g[1][0] = 0.5 * 1.0 = 0.5
        // g[2][0] = 0.5 * g[1][0] + 1*b10 * g[0][0] = 0.5*0.5 + 1*0.1*1.0 = 0.25 + 0.1 = 0.35
        assert!((operands[circuit.result_operand] - 0.35).abs() < 1e-10);
    }

    #[test]
    fn test_permutation_symmetry_assertion() {
        // la < lb should panic
        let result = std::panic::catch_unwind(|| {
            CircuitBuilder::new(0, 1, 0, 0)
        });
        assert!(result.is_err());

        // lc < ld should panic
        let result = std::panic::catch_unwind(|| {
            CircuitBuilder::new(1, 0, 0, 1)
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_vrr_full_cross_term() {
        // Test (ps|ps) = (1,0|1,0): nmax=1, mmax=1
        let circuit = CircuitBuilder::new(1, 0, 1, 0).build_circuit();
        let mut operands = vec![1.0];
        // c00=0.3, c0p=0.4, b00=0.2
        let prefactors = [0.3, 0.4, 0.0, 0.0, 0.2, 0.0, 0.0];
        evaluate_circuit(&circuit, &mut operands, &prefactors);
        // VRR-Bra: g[1][0] = c00 * g[0][0] = 0.3
        // VRR-Ket: g[0][1] = c0p * g[0][0] = 0.4
        // VRR-Full: g[1][1] = c00 * g[0][1] + 1*b00 * g[0][0] = 0.3*0.4 + 1*0.2*1.0 = 0.12 + 0.2 = 0.32
        assert!((operands[circuit.result_operand] - 0.32).abs() < 1e-10,
            "Expected 0.32, got {}", operands[circuit.result_operand]);
    }

    #[test]
    fn test_result_operand_valid() {
        // For all quartets up to d-shell, verify result_operand < n_operands
        for la in 0..=2 {
            for lb in 0..=la {
                for lc in 0..=2 {
                    for ld in 0..=lc {
                        let circuit = CircuitBuilder::new(la, lb, lc, ld).build_circuit();
                        assert!(circuit.result_operand < circuit.n_operands,
                            "({}{}|{}{}): result_operand {} >= n_operands {}",
                            shell_name(la), shell_name(lb), shell_name(lc), shell_name(ld),
                            circuit.result_operand, circuit.n_operands);
                    }
                }
            }
        }
    }

    #[test]
    fn test_no_duplicate_result_operands() {
        // Each gate should write to a unique operand
        let circuit = CircuitBuilder::new(2, 2, 2, 2).build_circuit();
        let mut results: Vec<usize> = circuit.gates.iter().map(|g| g.result).collect();
        results.sort();
        results.dedup();
        assert_eq!(results.len(), circuit.gates.len(),
            "Some gates write to the same operand");
    }

    // ── v3.2.0: g_idx, hrr_bra_map, hrr_ket_map, find_operand tests ──

    #[test]
    fn test_g_idx_dimensions() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        // nmax=2, mmax=2 → g_idx should be 3×3
        assert_eq!(circuit.g_idx.len(), 3);
        for row in &circuit.g_idx {
            assert_eq!(row.len(), 3);
        }
    }

    #[test]
    fn test_g_idx_base_case() {
        let circuit = CircuitBuilder::new(0, 0, 0, 0).build_circuit();
        assert_eq!(circuit.g_idx.len(), 1);
        assert_eq!(circuit.g_idx[0].len(), 1);
        assert_eq!(circuit.g_idx[0][0], 0); // base operand
    }

    #[test]
    fn test_g_idx_no_duplicates() {
        let circuit = CircuitBuilder::new(2, 1, 1, 0).build_circuit();
        // Collect all g_idx entries and check they are unique
        let mut all_g: Vec<usize> = Vec::new();
        for row in &circuit.g_idx {
            for &op in row {
                all_g.push(op);
            }
        }
        let mut sorted = all_g.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), all_g.len(), "g_idx has duplicate operand indices");
    }

    #[test]
    fn test_hrr_bra_map_dimensions() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        // mmax=2 → hrr_bra_map should have 3 entries (m=0,1,2)
        assert_eq!(circuit.hrr_bra_map.len(), 3);

        // For each m, we should have entries for ALL (na, nb) with na+nb ≤ 2, nb ≤ 1:
        // nb=0: na=0,1,2  → 3 entries
        // nb=1: na=0,1    → 2 entries
        // Total: 5 entries per m
        for m in 0..=2 {
            assert_eq!(circuit.hrr_bra_map[m].len(), 5,
                "hrr_bra_map[{}] should have 5 entries, got {}", m, circuit.hrr_bra_map[m].len());
        }
    }

    #[test]
    fn test_hrr_bra_map_covers_all_pairs() {
        let circuit = CircuitBuilder::new(2, 2, 0, 0).build_circuit();
        // nmax=4, lb=2, mmax=0
        // For m=0, we should have ALL (na, nb) with na+nb ≤ 4, nb ≤ 2:
        // nb=0: na=0,1,2,3,4  → 5 entries
        // nb=1: na=0,1,2,3    → 4 entries
        // nb=2: na=0,1,2      → 3 entries
        // Total: 12 entries
        assert_eq!(circuit.hrr_bra_map[0].len(), 12);

        // Verify specific entries exist
        let map0 = &circuit.hrr_bra_map[0];
        assert!(map0.iter().any(|e| e.i == 0 && e.j == 0), "Missing I(0,0|0,0)");
        assert!(map0.iter().any(|e| e.i == 2 && e.j == 0), "Missing I(2,0|0,0)");
        assert!(map0.iter().any(|e| e.i == 2 && e.j == 2), "Missing I(2,2|0,0)");
        assert!(map0.iter().any(|e| e.i == 0 && e.j == 2), "Missing I(0,2|0,0)");
    }

    #[test]
    fn test_hrr_bra_map_lb_zero() {
        let circuit = CircuitBuilder::new(2, 0, 1, 0).build_circuit();
        // lb=0: no HRR-Bra gates, but hrr_bra_map should still have nb=0 entries
        // nmax=2, mmax=1
        for m in 0..=1 {
            // nb=0: na=0,1,2 → 3 entries
            assert_eq!(circuit.hrr_bra_map[m].len(), 3);
            for entry in &circuit.hrr_bra_map[m] {
                assert_eq!(entry.j, 0, "All entries should have nb=0 when lb=0");
            }
        }
    }

    #[test]
    fn test_hrr_ket_map_dimensions() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        // la=1, lb=1, lc=1, ld=1, mmax=2
        // nd=0: nc=0,1,2  → 3 entries
        // nd=1: nc=0,1    → 2 entries
        // Total: 5 entries
        assert_eq!(circuit.hrr_ket_map.len(), 5);
    }

    #[test]
    fn test_hrr_ket_map_covers_all_pairs() {
        let circuit = CircuitBuilder::new(1, 0, 2, 2).build_circuit();
        // la=1, lb=0, lc=2, ld=2, mmax=4
        // nd=0: nc=0,1,2,3,4 → 5 entries
        // nd=1: nc=0,1,2,3   → 4 entries
        // nd=2: nc=0,1,2     → 3 entries
        // Total: 12 entries
        assert_eq!(circuit.hrr_ket_map.len(), 12);

        // Verify specific entries exist
        assert!(circuit.hrr_ket_map.iter().any(|e| e.i == 0 && e.j == 0), "Missing I(1,0|0,0)");
        assert!(circuit.hrr_ket_map.iter().any(|e| e.i == 4 && e.j == 0), "Missing I(1,0|4,0)");
        assert!(circuit.hrr_ket_map.iter().any(|e| e.i == 2 && e.j == 2), "Missing I(1,0|2,2)");
    }

    #[test]
    fn test_hrr_ket_map_ld_zero() {
        let circuit = CircuitBuilder::new(1, 1, 2, 0).build_circuit();
        // ld=0: no HRR-Ket gates, but hrr_ket_map should still have nd=0 entries
        // mmax=2
        // nd=0: nc=0,1,2 → 3 entries
        assert_eq!(circuit.hrr_ket_map.len(), 3);
        for entry in &circuit.hrr_ket_map {
            assert_eq!(entry.j, 0, "All entries should have nd=0 when ld=0");
        }
    }

    #[test]
    fn test_find_operand_vrr_direct() {
        let circuit = CircuitBuilder::new(1, 0, 1, 0).build_circuit();
        // g_idx[0][0] = 0 (base), g_idx[1][0] and g_idx[0][1] and g_idx[1][1]
        assert_eq!(circuit.find_operand(0, 0, 0, 0), Some(circuit.g_idx[0][0]));
        assert_eq!(circuit.find_operand(1, 0, 0, 0), Some(circuit.g_idx[1][0]));
        assert_eq!(circuit.find_operand(0, 0, 1, 0), Some(circuit.g_idx[0][1]));
        assert_eq!(circuit.find_operand(1, 0, 1, 0), Some(circuit.g_idx[1][1]));
    }

    #[test]
    fn test_find_operand_hrr_bra() {
        let circuit = CircuitBuilder::new(1, 1, 0, 0).build_circuit();
        // lb=1, so hrr_bra_map[0] should have I(0,0|0,0), I(1,0|0,0), I(2,0|0,0), I(0,1|0,0), I(1,1|0,0)
        let op_0_0 = circuit.find_operand(0, 0, 0, 0);
        let op_1_0 = circuit.find_operand(1, 0, 0, 0);
        let op_2_0 = circuit.find_operand(2, 0, 0, 0);
        let op_0_1 = circuit.find_operand(0, 1, 0, 0);
        let op_1_1 = circuit.find_operand(1, 1, 0, 0);

        assert!(op_0_0.is_some(), "I(0,0|0,0) should be found");
        assert!(op_1_0.is_some(), "I(1,0|0,0) should be found");
        assert!(op_2_0.is_some(), "I(2,0|0,0) should be found");
        assert!(op_0_1.is_some(), "I(0,1|0,0) should be found");
        assert!(op_1_1.is_some(), "I(1,1|0,0) should be found");

        // The nb=0 entries should match g_idx
        assert_eq!(op_0_0, Some(circuit.g_idx[0][0]));
        assert_eq!(op_1_0, Some(circuit.g_idx[1][0]));
        assert_eq!(op_2_0, Some(circuit.g_idx[2][0]));

        // The final result should be I(1,1|0,0)
        assert_eq!(op_1_1, Some(circuit.result_operand));
    }

    #[test]
    fn test_find_operand_hrr_ket() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        // Should be able to find I(1,1|0,0), I(1,1|1,0), I(1,1|2,0), I(1,1|0,1), I(1,1|1,1)
        let op_1_1_0_0 = circuit.find_operand(1, 1, 0, 0);
        let op_1_1_1_0 = circuit.find_operand(1, 1, 1, 0);
        let op_1_1_2_0 = circuit.find_operand(1, 1, 2, 0);
        let op_1_1_0_1 = circuit.find_operand(1, 1, 0, 1);
        let op_1_1_1_1 = circuit.find_operand(1, 1, 1, 1);

        assert!(op_1_1_0_0.is_some(), "I(1,1|0,0) should be found");
        assert!(op_1_1_1_0.is_some(), "I(1,1|1,0) should be found");
        assert!(op_1_1_2_0.is_some(), "I(1,1|2,0) should be found");
        assert!(op_1_1_0_1.is_some(), "I(1,1|0,1) should be found");
        assert!(op_1_1_1_1.is_some(), "I(1,1|1,1) should be found");

        // The final result should be I(1,1|1,1)
        assert_eq!(op_1_1_1_1, Some(circuit.result_operand));
    }

    #[test]
    fn test_find_operand_out_of_bounds() {
        let circuit = CircuitBuilder::new(1, 0, 1, 0).build_circuit();
        // na + nb > nmax → None
        assert_eq!(circuit.find_operand(3, 0, 0, 0), None);
        // nb > lb → None
        assert_eq!(circuit.find_operand(0, 1, 0, 0), None);
        // nc + nd > mmax → None
        assert_eq!(circuit.find_operand(0, 0, 3, 0), None);
        // nd > ld → None
        assert_eq!(circuit.find_operand(0, 0, 0, 1), None);
    }

    #[test]
    fn test_find_operand_non_la_lb_ket() {
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        // I(0,1|0,1) is NOT in hrr_ket_map (because na=0 ≠ la=1 or nb=1 = lb=1 but na≠la)
        // Actually, na=0 ≠ la=1, so this returns None
        assert_eq!(circuit.find_operand(0, 1, 0, 1), None);

        // But I(0,1|0,0) IS available via hrr_bra_map (nd=0)
        assert!(circuit.find_operand(0, 1, 0, 0).is_some(), "I(0,1|0,0) should be found via hrr_bra_map");
    }

    #[test]
    fn test_rys_order() {
        assert_eq!(CircuitBuilder::new(0, 0, 0, 0).build_circuit().rys_order, 1); // (0+0)/2+1
        assert_eq!(CircuitBuilder::new(1, 0, 0, 0).build_circuit().rys_order, 1); // (1+0)/2+1
        assert_eq!(CircuitBuilder::new(1, 1, 0, 0).build_circuit().rys_order, 2); // (2+0)/2+1
        assert_eq!(CircuitBuilder::new(1, 1, 1, 1).build_circuit().rys_order, 3); // (2+2)/2+1
        assert_eq!(CircuitBuilder::new(2, 2, 2, 2).build_circuit().rys_order, 5); // (4+4)/2+1
        assert_eq!(CircuitBuilder::new(3, 1, 2, 0).build_circuit().rys_order, 4); // (4+2)/2+1
    }

    #[test]
    fn test_find_operand_consistent_with_evaluation() {
        // Evaluate (pp|ss) and verify find_operand gives correct operand indices
        let circuit = CircuitBuilder::new(1, 1, 0, 0).build_circuit();
        let mut operands = vec![1.0];
        let prefactors = [0.5, 0.0, 0.25, 0.0, 0.0, -0.3, 0.0];
        evaluate_circuit(&circuit, &mut operands, &prefactors);

        // Check I(0,1|0,0) = I(1,0|0,0) + (-AB) * I(0,0|0,0)
        // = 0.5 + (-0.3) * 1.0 = 0.2
        let op_0_1 = circuit.find_operand(0, 1, 0, 0).unwrap();
        assert!((operands[op_0_1] - 0.2).abs() < 1e-10,
            "I(0,1|0,0) should be 0.2, got {}", operands[op_0_1]);

        // Check I(1,1|0,0) = I(2,0|0,0) + (-AB) * I(1,0|0,0)
        // = 0.5 + (-0.3) * 0.5 = 0.35
        let op_1_1 = circuit.find_operand(1, 1, 0, 0).unwrap();
        assert!((operands[op_1_1] - 0.35).abs() < 1e-10,
            "I(1,1|0,0) should be 0.35, got {}", operands[op_1_1]);
    }

    #[test]
    fn test_find_operand_pppp_consistent() {
        // Evaluate (pp|pp) and verify find_operand gives correct results
        let circuit = CircuitBuilder::new(1, 1, 1, 1).build_circuit();
        let mut operands = vec![1.0];
        // c00=0.3, c0p=0.4, b10=0.1, b01=0.15, b00=0.2, AB=-0.5, CD=-0.3
        let prefactors = [0.3, 0.4, 0.1, 0.15, 0.2, -0.5, -0.3];
        evaluate_circuit(&circuit, &mut operands, &prefactors);

        // g[0][0] = 1.0
        assert!((operands[circuit.find_operand(0, 0, 0, 0).unwrap()] - 1.0).abs() < 1e-10);
        // g[1][0] = c00 * g[0][0] = 0.3
        assert!((operands[circuit.find_operand(1, 0, 0, 0).unwrap()] - 0.3).abs() < 1e-10);
        // g[0][1] = c0p * g[0][0] = 0.4
        assert!((operands[circuit.find_operand(0, 0, 1, 0).unwrap()] - 0.4).abs() < 1e-10);
        // g[1][1] = c00*g[0][1] + 1*b00*g[0][0] = 0.3*0.4 + 0.2*1.0 = 0.32
        assert!((operands[circuit.find_operand(1, 0, 1, 0).unwrap()] - 0.32).abs() < 1e-10);

        // I(1,1|1,1) should match result_operand
        let op_final = circuit.find_operand(1, 1, 1, 1).unwrap();
        assert_eq!(op_final, circuit.result_operand);
    }
}
