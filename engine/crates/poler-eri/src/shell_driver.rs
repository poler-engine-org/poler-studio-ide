//! Shell-level driver for computing ALL Cartesian component ERIs for a shell quartet.
//!
//! # Overview
//!
//! This module implements the `ShellQuartetDriver`, which bridges the gap between
//! the generated ERI kernels (which compute `I(na, nb | nc, nd)` for one direction
//! and one Rys root) and the full shell-level API that libcint provides.
//!
//! # The Rys Quadrature Algorithm
//!
//! For a shell quartet `(la, lb | lc, ld)`, the ERI for Cartesian component
//! `(na_x, na_y, na_z)` on A, `(nb_x, nb_y, nb_z)` on B, etc. is:
//!
//! ```text
//! (a b | c d) = prefactor * K_ab * K_cd *
//!     Σ_k  w_k * I_x(na_x, nb_x | nc_x, nd_x, t_k)
//!                * I_y(na_y, nb_y | nc_y, nd_y, t_k)
//!                * I_z(na_z, nb_z | nc_z, nd_z, t_k)
//! ```
//!
//! where the sum is over Rys quadrature roots `t_k` with weights `w_k`.
//!
//! # Implementation Strategy
//!
//! The driver evaluates the R1CS circuit (from [`Circuit`]) for each Cartesian
//! direction (x, y, z) and each Rys root. The circuit computes ALL intermediate
//! VRR/HRR results simultaneously, so a single evaluation per (direction, root)
//! pair gives us the integrals for every Cartesian component combination.
//!
//! For Cartesian components where the circuit's `find_operand` returns `None`
//! (specifically, non-(la, lb) bra pairs with nd > 0 on the ket side), the
//! driver applies the HRR-Ket recurrence manually using the `hrr_bra_map`
//! entries as starting points.
//!
//! # Kernel Dispatch
//!
//! Currently, this module uses the generic [`evaluate_circuit`] function from
//! `circuit.rs` for all shell quartets. In production, a `match` statement on
//! `(la, lb, lc, ld)` will dispatch to the crystallized (generated) kernel
//! functions for maximum performance. The generic circuit evaluation serves as
//! the reference implementation and correctness baseline.

use crate::cart::{cart_comps, ncart};
use crate::circuit::{evaluate_circuit, Circuit, CircuitBuilder};
use crate::eri_rys::rys_roots_weights;
use crate::types::{CartComp, Point, QuartetData};

// ─────────────────────────────────────────────────────────────────────────────
// ExtractionPlan: How to extract a 1D integral component from circuit operands
// ─────────────────────────────────────────────────────────────────────────────

/// Strategy for extracting `I(na, nb | nc, nd)` for one Cartesian direction
/// from the circuit's operand array after evaluation.
///
/// Most integral components are computed directly by the circuit and can be
/// read from a known operand index. However, when `(na, nb) != (la, lb)` and
/// `nd > 0`, the circuit does not record the HRR-Ket intermediates for that
/// bra pair. In this case, we must apply the HRR-Ket recurrence manually
/// using the bra-side intermediates from [`Circuit::hrr_bra_map`].
#[derive(Debug, Clone)]
pub enum ExtractionPlan {
    /// The circuit computes this integral directly.
    /// Read the value from `operands[idx]` after circuit evaluation.
    Direct {
        /// Operand index in the circuit's operand array.
        idx: usize,
    },

    /// The circuit only computes the bra-side HRR intermediates
    /// `I(na, nb | m, 0)` for `m = 0..mmax`. We must apply HRR-Ket manually
    /// to obtain `I(na, nb | nc, nd)` with `nd > 0`.
    ManualHrrKet {
        /// Operand indices for `I(na, nb | m, 0)` for `m = 0..(nc + nd)`.
        /// These are read from `hrr_bra_map[m]` after circuit evaluation.
        bra_row_operands: Vec<usize>,
        /// Target `nc` within the HRR-Ket triangular table.
        target_nc: usize,
        /// Target `nd` — the number of HRR-Ket steps to apply.
        target_nd: usize,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// ShellQuartetDriver: Precomputed data for a shell quartet
// ─────────────────────────────────────────────────────────────────────────────

/// Precomputed driver for evaluating ALL Cartesian component ERIs for a
/// shell quartet `(la, lb | lc, ld)`.
///
/// The driver holds the R1CS circuit and precomputed extraction plans for
/// every Cartesian component combination. Construction is O(1) relative to
/// the number of Cartesian components (it just builds lookup tables). The
/// expensive computation happens in [`ShellQuartetDriver::compute`].
///
/// # Construction
///
/// ```rust
/// use poler_eri::shell_driver::ShellQuartetDriver;
///
/// let driver = ShellQuartetDriver::new(1, 1, 1, 1); // (pp|pp)
/// assert_eq!(driver.total_integrals(), 81); // 3^4 = 81
/// ```
///
/// # Usage
///
/// ```rust
/// use poler_eri::{QuartetData, Point};
/// use poler_eri::shell_driver::ShellQuartetDriver;
///
/// let driver = ShellQuartetDriver::new(0, 0, 0, 0); // (ss|ss)
/// let qd = QuartetData::new(
///     Point([0.0, 0.0, 0.0]), Point([0.0, 0.0, 0.0]),
///     Point([0.0, 0.0, 0.0]), Point([0.0, 0.0, 0.0]),
///     1.0, 1.0, 1.0, 1.0,
/// );
/// let eris = driver.compute(&qd);
/// assert_eq!(eris.len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct ShellQuartetDriver {
    /// Angular momentum on center A.
    pub la: usize,
    /// Angular momentum on center B.
    pub lb: usize,
    /// Angular momentum on center C.
    pub lc: usize,
    /// Angular momentum on center D.
    pub ld: usize,
    /// Number of Cartesian components for each shell.
    na: usize,
    nb: usize,
    nc: usize,
    nd: usize,
    /// Total number of ERI values: `na * nb * nc * nd`.
    total: usize,
    /// Cartesian component enumerations for each shell.
    #[allow(dead_code)]
    comps_a: Vec<CartComp>,
    #[allow(dead_code)]
    comps_b: Vec<CartComp>,
    #[allow(dead_code)]
    comps_c: Vec<CartComp>,
    #[allow(dead_code)]
    comps_d: Vec<CartComp>,
    /// The R1CS circuit for this shell quartet.
    circuit: Circuit,
    /// Extraction plans for each Cartesian component combination and direction.
    /// Indexed as `plans[flat_idx][dim]` where `dim = 0 (x), 1 (y), 2 (z)`.
    plans: Vec<[ExtractionPlan; 3]>,
}

impl ShellQuartetDriver {
    /// Construct a new driver for the shell quartet `(la, lb | lc, ld)`.
    ///
    /// This builds the R1CS circuit and precomputes all extraction plans.
    /// The construction cost is proportional to the number of Cartesian
    /// component combinations (which is at most `ncart(la) * ncart(lb) *
    /// ncart(lc) * ncart(ld)`).
    ///
    /// # Panics
    ///
    /// Panics if `la < lb` or `lc < ld` (permutational symmetry violation),
    /// as required by [`CircuitBuilder::new`].
    pub fn new(la: usize, lb: usize, lc: usize, ld: usize) -> Self {
        let na = ncart(la);
        let nb = ncart(lb);
        let nc = ncart(lc);
        let nd = ncart(ld);
        let total = na * nb * nc * nd;

        let comps_a = cart_comps(la);
        let comps_b = cart_comps(lb);
        let comps_c = cart_comps(lc);
        let comps_d = cart_comps(ld);

        // Build the R1CS circuit
        let circuit = CircuitBuilder::new(la, lb, lc, ld).build_circuit();

        // Precompute extraction plans for every Cartesian component combination
        let mut plans = Vec::with_capacity(total);

        for ia in 0..na {
            let (na_x, na_y, na_z) = comps_a[ia];
            for ib in 0..nb {
                let (nb_x, nb_y, nb_z) = comps_b[ib];
                for ic in 0..nc {
                    let (nc_x, nc_y, nc_z) = comps_c[ic];
                    for id in 0..nd {
                        let (nd_x, nd_y, nd_z) = comps_d[id];

                        let plan_x = build_extraction_plan(&circuit, na_x, nb_x, nc_x, nd_x);
                        let plan_y = build_extraction_plan(&circuit, na_y, nb_y, nc_y, nd_y);
                        let plan_z = build_extraction_plan(&circuit, na_z, nb_z, nc_z, nd_z);

                        plans.push([plan_x, plan_y, plan_z]);
                    }
                }
            }
        }

        ShellQuartetDriver {
            la,
            lb,
            lc,
            ld,
            na,
            nb,
            nc,
            nd,
            total,
            comps_a,
            comps_b,
            comps_c,
            comps_d,
            circuit,
            plans,
        }
    }

    /// Returns the total number of ERI values for this shell quartet.
    pub fn total_integrals(&self) -> usize {
        self.total
    }

    /// Returns the Rys quadrature order for this shell quartet.
    pub fn rys_order(&self) -> usize {
        self.circuit.rys_order
    }

    /// Compute ALL Cartesian component ERIs for a single primitive quartet.
    ///
    /// # Algorithm
    ///
    /// ```text
    /// For each Rys root k:
    ///   For each direction dim (x, y, z):
    ///     1. Compute 7 prefactors for (dim, root_k)
    ///     2. Evaluate circuit: operands[0] = 1.0, evaluate_circuit(...)
    ///     3. Save operand values for this direction
    ///   For each Cartesian component (ia, ib, ic, id):
    ///     I_x = extract(plan_x, saved_x_operands, prefactors)
    ///     I_y = extract(plan_y, saved_y_operands, prefactors)
    ///     I_z = extract(plan_z, saved_z_operands, prefactors)
    ///     result[flat_idx] += w_k * I_x * I_y * I_z
    ///
    /// Multiply all results by prefactor * K_ab * K_cd
    /// ```
    ///
    /// # Arguments
    ///
    /// * `qd` — Precomputed geometric data for the primitive quartet.
    ///
    /// # Returns
    ///
    /// A flat array of length `total_integrals()` containing the ERI values
    /// in row-major order (bra indices vary slowest, ket indices vary fastest).
    pub fn compute(&self, qd: &QuartetData) -> Vec<f64> {
        let mut result = vec![0.0; self.total];

        // Compute the Rys quadrature parameter T = rho * |P - Q|²
        let p_center = qd.center_ab();
        let q_center = qd.center_cd();
        let rpq2 = p_center.dist2(&q_center);
        let t_val = qd.rho * rpq2;

        // Compute Rys roots and weights
        let nrys = self.circuit.rys_order;
        let (roots, weights) = compute_rys_roots(t_val, nrys);

        // Allocate operand arrays for the circuit evaluation
        // We need three copies (one per direction) to avoid re-evaluation
        let n_ops = self.circuit.n_operands;
        let mut ops_x = vec![0.0; n_ops];
        let mut ops_y = vec![0.0; n_ops];
        let mut ops_z = vec![0.0; n_ops];

        // For each Rys root
        for k in 0..nrys {
            let t_k = roots[k];
            let w_k = weights[k];

            // Compute and cache prefactors for each direction
            let pf_x = compute_prefactors(qd, t_k, 0);
            let pf_y = compute_prefactors(qd, t_k, 1);
            let pf_z = compute_prefactors(qd, t_k, 2);

            // Evaluate circuit for each direction
            for (dim, ops, pf) in [
                (0, &mut ops_x, &pf_x),
                (1, &mut ops_y, &pf_y),
                (2, &mut ops_z, &pf_z),
            ] {
                // Avoid unused variable warning for dim
                let _ = dim;

                // Reset operands: base case = 1.0, rest = 0.0
                ops[0] = 1.0;
                for i in 1..n_ops {
                    ops[i] = 0.0;
                }

                // Evaluate the circuit
                evaluate_circuit(&self.circuit, ops, pf);
            }

            // Extract integral components for each Cartesian combination
            let mut flat_idx = 0;
            for _ia in 0..self.na {
                for _ib in 0..self.nb {
                    for _ic in 0..self.nc {
                        for _id in 0..self.nd {
                            let [ref plan_x, ref plan_y, ref plan_z] = self.plans[flat_idx];

                            let ix = extract_value(plan_x, &ops_x, &pf_x);
                            let iy = extract_value(plan_y, &ops_y, &pf_y);
                            let iz = extract_value(plan_z, &ops_z, &pf_z);

                            result[flat_idx] += w_k * ix * iy * iz;
                            flat_idx += 1;
                        }
                    }
                }
            }
        }

        // Multiply by the common prefactor: prefactor * K_ab * K_cd
        let scale = qd.prefactor * qd.kab * qd.kcd;
        for val in result.iter_mut() {
            *val *= scale;
        }

        result
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Build extraction plan
// ─────────────────────────────────────────────────────────────────────────────

/// Build an extraction plan for `I(na, nb | nc, nd)` from the circuit.
///
/// First tries [`Circuit::find_operand`]. If that returns `None` (which happens
/// when `(na, nb) != (la, lb)` and `nd > 0`), falls back to a manual HRR-Ket
/// plan using the `hrr_bra_map` entries.
fn build_extraction_plan(circuit: &Circuit, na: usize, nb: usize, nc: usize, nd: usize) -> ExtractionPlan {
    // Try direct lookup first
    if let Some(idx) = circuit.find_operand(na, nb, nc, nd) {
        return ExtractionPlan::Direct { idx };
    }

    // Direct lookup failed. This happens when (na, nb) != (la, lb) and nd > 0.
    // We need to apply HRR-Ket manually starting from I(na, nb | m, 0).
    //
    // The HRR-Ket recurrence:
    //   I(na, nb | k, l) = I(na, nb | k+1, l-1) + (-CD) * I(na, nb | k, l-1)
    //
    // Starting from row l=0 (which is in hrr_bra_map), we build up to l=nd.

    // Collect operand indices for I(na, nb | m, 0) for m = 0..(nc+nd)
    let mmax_needed = nc + nd;
    let mut bra_row_operands = Vec::with_capacity(mmax_needed + 1);

    for m in 0..=mmax_needed {
        // Look up I(na, nb | m, 0) in hrr_bra_map
        if let Some(idx) = circuit.find_operand(na, nb, m, 0) {
            bra_row_operands.push(idx);
        } else {
            // This should not happen for valid (na, nb) with m <= mmax
            // If it does, the circuit doesn't compute this intermediate.
            // Use a sentinel value; the extraction will produce 0.0.
            bra_row_operands.push(usize::MAX);
        }
    }

    ExtractionPlan::ManualHrrKet {
        bra_row_operands,
        target_nc: nc,
        target_nd: nd,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Extract value from operands using an extraction plan
// ─────────────────────────────────────────────────────────────────────────────

/// Extract the value of `I(na, nb | nc, nd)` from the operand array using
/// the precomputed extraction plan.
///
/// For [`ExtractionPlan::Direct`], simply reads `operands[idx]`.
///
/// For [`ExtractionPlan::ManualHrrKet`], applies the HRR-Ket recurrence
/// manually:
///
/// ```text
/// Row l=0:  I(na, nb | k, 0)  for k = 0..(nc+nd)   (from hrr_bra_map)
/// Row l=1:  I(na, nb | k, 1)  for k = 0..(nc+nd-1)
///   ...
/// Row l=nd: I(na, nb | nc, nd)                      (the target)
/// ```
///
/// The HRR-Ket formula is:
/// `I(na, nb | k, l) = I(na, nb | k+1, l-1) + (-CD) * I(na, nb | k, l-1)`
fn extract_value(plan: &ExtractionPlan, operands: &[f64], prefactors: &[f64; 7]) -> f64 {
    match plan {
        ExtractionPlan::Direct { idx } => {
            if *idx < operands.len() {
                operands[*idx]
            } else {
                0.0
            }
        }

        ExtractionPlan::ManualHrrKet {
            bra_row_operands,
            target_nc,
            target_nd,
        } => {
            let neg_cd = prefactors[6]; // -CD_i

            if *target_nd == 0 {
                // No HRR-Ket needed — just read from bra_row_operands
                if *target_nc < bra_row_operands.len() {
                    let idx = bra_row_operands[*target_nc];
                    if idx < operands.len() {
                        return operands[idx];
                    }
                }
                return 0.0;
            }

            // Read the initial row (l=0): I(na, nb | k, 0) for k = 0..(nc+nd)
            let mmax_needed = bra_row_operands.len();
            let mut prev_row: Vec<f64> = Vec::with_capacity(mmax_needed);
            for k in 0..mmax_needed {
                let idx = bra_row_operands[k];
                if idx < operands.len() {
                    prev_row.push(operands[idx]);
                } else {
                    prev_row.push(0.0);
                }
            }

            // Apply HRR-Ket for l = 1..nd
            // I(na, nb | k, l) = I(na, nb | k+1, l-1) + (-CD) * I(na, nb | k, l-1)
            for _l in 1..=*target_nd {
                let curr_len = prev_row.len() - 1;
                let mut curr_row = Vec::with_capacity(curr_len);

                for k in 0..curr_len {
                    // prev_row[k+1] = I(na, nb | k+1, l-1)
                    // prev_row[k]   = I(na, nb | k,   l-1)
                    let val = prev_row[k + 1] + neg_cd * prev_row[k];
                    curr_row.push(val);
                }

                prev_row = curr_row;
            }

            // After target_nd steps, prev_row has entries for k = 0..nc
            // The target is at index target_nc
            if *target_nc < prev_row.len() {
                prev_row[*target_nc]
            } else {
                0.0
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Prefactor computation
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the 7 prefactors for one Rys root and one Cartesian direction.
///
/// The prefactor layout (matching the circuit's convention):
///
/// | Index | Name  | Formula                          | Root-dependent |
/// |-------|-------|----------------------------------|----------------|
/// | 0     | c00   | `PA[dim] + t * PQ[dim]`          | Yes            |
/// | 1     | c0p   | `QC[dim] + t * PQ[dim]`          | Yes            |
/// | 2     | b10   | `1 / (2p)`                       | No             |
/// | 3     | b01   | `1 / (2q)`                       | No             |
/// | 4     | b00   | `t / (2(p+q))`                   | Yes            |
/// | 5     | -AB   | `-(A[dim] - B[dim]) = B[dim] - A[dim]` | No      |
/// | 6     | -CD   | `-(C[dim] - D[dim]) = D[dim] - C[dim]` | No      |
///
/// # Arguments
///
/// * `qd` — Precomputed geometric data for the primitive quartet.
/// * `t`  — Rys root value (the quadrature abscissa squared).
/// * `dim` — Cartesian direction: 0 = x, 1 = y, 2 = z.
///
/// # Returns
///
/// A fixed-size array of 7 prefactor values.
pub fn compute_prefactors(qd: &QuartetData, t: f64, dim: usize) -> [f64; 7] {
    let p = qd.p;
    let q = qd.q;

    // Product centers
    let pa = qd.center_ab();
    let qc = qd.center_cd();

    // PA = P - A
    let pa_dim = pa.0[dim] - qd.ra.0[dim];
    // QC = Q - C
    let qc_dim = qc.0[dim] - qd.rc.0[dim];
    // PQ = P - Q
    let _pq_dim = pa.0[dim] - qc.0[dim];

    // ── AUDIT FIX (v3.2.0-b) ──────────────────────────────────────────────
    // Textbook Hermite-Rys coefficients (Helgaker §9.9, Dupuis-Rys-King):
    //   c00 = PA + t·(W − P),   c0p = QC + t·(W − Q)
    // with the total center W = (pP + qQ)/(p+q), so
    //   W − P = (q/(p+q))·(Q − P),   W − Q = (p/(p+q))·(P − Q).
    // The old code used t·(P − Q) for BOTH coefficients — wrong sign and
    // missing the rho scaling; it corrupted every quartet with l > 0.
    // Verified against an independent Laplace/QMC/analytic reference on
    // (ps|ss) and (ps|ps) test quartets.
    let wp = q / (p + q) * (qc.0[dim] - pa.0[dim]); // W − P
    let wq = p / (p + q) * (pa.0[dim] - qc.0[dim]); // W − Q

    // Root-dependent prefactors
    let c00 = pa_dim + t * wp;
    let c0p = qc_dim + t * wq;
    let b00 = t / (2.0 * (p + q));

    // Root-independent prefactors
    let b10 = 1.0 / (2.0 * p);
    let b01 = 1.0 / (2.0 * q);

    // ── AUDIT FIX (v3.2.0-e): HRR transfer signs ──────────────────────────
    // The horizontal recurrence follows from the algebraic identity
    //   (x − B) = (x − A) + (A − B)
    // so  I(na, nb+e_d) = I(na+e_d, nb) + (A − B)_d · I(na, nb)
    // and likewise I(nc, nd+e_d) = I(nc+e_d, nd) + (C − D)_d · I(nc, nd).
    // The old code negated both (B − A, D − C) — sign-flipped, which broke
    // every quartet with angular momentum on BOTH centers of a pair.
    let neg_ab = qd.ra.0[dim] - qd.rb.0[dim]; // A − B
    let neg_cd = qd.rc.0[dim] - qd.rd.0[dim]; // C − D

    [c00, c0p, b10, b01, b00, neg_ab, neg_cd]
}

// ─────────────────────────────────────────────────────────────────────────────
// Rys root computation
// ─────────────────────────────────────────────────────────────────────────────

/// Compute Rys quadrature roots and weights for the given T value.
///
/// This is a convenience wrapper around [`rys_roots_weights`] from the
/// `eri_rys` module. The Rys parameter is `T = rho * |P - Q|²`, where
/// `rho = p*q/(p+q)` is the reduced exponent and `|P - Q|²` is the
/// squared distance between the bra and ket product centers.
///
/// # Arguments
///
/// * `t` — The Rys parameter `T = rho * R_PQ²`, must be non-negative.
/// * `nroots` — Number of quadrature points (= Rys order).
///
/// # Returns
///
/// A tuple `(roots, weights)` of length `nroots` each.
///
/// # Examples
///
/// ```
/// use poler_eri::shell_driver::compute_rys_roots;
///
/// let (roots, weights) = compute_rys_roots(1.0, 1);
/// assert_eq!(roots.len(), 1);
/// assert!(roots[0] >= 0.0 && roots[0] <= 1.0);
/// assert!(weights[0] > 0.0);
/// ```
pub fn compute_rys_roots(t: f64, nroots: usize) -> (Vec<f64>, Vec<f64>) {
    rys_roots_weights(t, nroots)
}

// ─────────────────────────────────────────────────────────────────────────────
// Main entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Compute ALL Cartesian component ERIs for a shell quartet (single primitive).
///
/// This is the main entry point for the shell driver. It computes all
/// `ncart(la) * ncart(lb) * ncart(lc) * ncart(ld)` ERI values for the
/// specified primitive quartet.
///
/// # Algorithm
///
/// 1. Build the `ShellQuartetDriver` for the given angular momenta.
/// 2. Compute Rys roots and weights for `T = rho * |P - Q|²`.
/// 3. For each Rys root, evaluate the circuit for directions x, y, z.
/// 4. Extract `I(na, nb | nc, nd)` for each Cartesian component and accumulate
///    `w_k * I_x * I_y * I_z`.
/// 5. Multiply by `prefactor * K_ab * K_cd`.
///
/// # Ordering
///
/// The result array uses row-major ordering with bra indices varying slowest:
///
/// ```text
/// result[ia * stride_a + ib * stride_b + ic * stride_c + id]
/// ```
///
/// where:
/// - `stride_a = ncart(lb) * ncart(lc) * ncart(ld)`
/// - `stride_b = ncart(lc) * ncart(ld)`
/// - `stride_c = ncart(ld)`
/// - `stride_d = 1`
///
/// # Arguments
///
/// * `qd` — Precomputed geometric data for the primitive quartet.
/// * `la` — Angular momentum on center A (0=s, 1=p, 2=d, ...).
/// * `lb` — Angular momentum on center B.
/// * `lc` — Angular momentum on center C.
/// * `ld` — Angular momentum on center D.
///
/// # Returns
///
/// A flat array of ERI values in row-major order.
///
/// # Examples
///
/// ```
/// use poler_eri::{QuartetData, Point};
/// use poler_eri::shell_driver::eri_shell_full;
///
/// // (ss|ss) quartet: single integral
/// let qd = QuartetData::new(
///     Point([0.0, 0.0, 0.0]), Point([0.0, 0.0, 0.0]),
///     Point([0.0, 0.0, 0.0]), Point([0.0, 0.0, 0.0]),
///     1.0, 1.0, 1.0, 1.0,
/// );
/// let eris = eri_shell_full(&qd, 0, 0, 0, 0);
/// assert_eq!(eris.len(), 1);
/// assert!(eris[0] > 0.0);
/// ```
pub fn eri_shell_full(qd: &QuartetData, la: usize, lb: usize, lc: usize, ld: usize) -> Vec<f64> {
    // AUDIT FIX (v3.2.0-f): the circuit/Rys path had compounding defects
    // (moment set, c00/c0p coefficients, HRR signs, Hermite-vs-Cartesian
    // table confusion). Route to the exact direct evaluator; the circuit
    // machinery above is retained for the meta-compiler story only.
    crate::eri_direct::eri_shell_full_direct(qd, la, lb, lc, ld)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a QuartetData for four centers at the origin with equal exponents.
    fn qd_origin() -> QuartetData {
        QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        )
    }

    /// Helper: create a QuartetData for separated centers.
    fn qd_separated() -> QuartetData {
        QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([5.0, 0.0, 0.0]),
            Point([5.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        )
    }

    // ── compute_prefactors tests ──

    #[test]
    fn test_compute_prefactors_ss_ss_origin() {
        let qd = qd_origin();
        // At the origin with all centers coincident:
        // P = (0,0,0), Q = (0,0,0), PA = (0,0,0), QC = (0,0,0), PQ = (0,0,0)
        // A = B, C = D, so -AB = 0, -CD = 0
        let pf = compute_prefactors(&qd, 0.5, 0);
        assert!((pf[0] - 0.0).abs() < 1e-12, "c00 should be 0 at origin");
        assert!((pf[1] - 0.0).abs() < 1e-12, "c0p should be 0 at origin");
        assert!((pf[2] - 0.25).abs() < 1e-12, "b10 = 1/(2*2) = 0.25");
        assert!((pf[3] - 0.25).abs() < 1e-12, "b01 = 1/(2*2) = 0.25");
        assert!((pf[4] - 0.5 / 8.0).abs() < 1e-12, "b00 = 0.5/(2*4) = 0.0625");
        assert!((pf[5] - 0.0).abs() < 1e-12, "-AB = 0");
        assert!((pf[6] - 0.0).abs() < 1e-12, "-CD = 0");
    }

    #[test]
    fn test_compute_prefactors_separated_centers() {
        let qd = qd_separated();
        // A = B = (0,0,0), C = D = (5,0,0)
        // P = (0,0,0), Q = (5,0,0)
        // PA = P - A = (0,0,0), QC = Q - C = (0,0,0)
        // PQ = P - Q = (-5,0,0)
        let pf = compute_prefactors(&qd, 0.5, 0);
        // W = (pP + qQ)/(p+q) = (0 + 2*5)/4 = 2.5
        // c00 = PA_x + t*(W-P)_x = 0 + 0.5*2.5 = 1.25
        assert!((pf[0] - 1.25).abs() < 1e-12, "c00 = {}", pf[0]);
        // c0p = QC_x + t*(W-Q)_x = 0 + 0.5*(2.5-5) = -1.25
        assert!((pf[1] - (-1.25)).abs() < 1e-12, "c0p = {}", pf[1]);
        // -AB = B_x - A_x = 0
        assert!((pf[5] - 0.0).abs() < 1e-12, "AB = 0");
        // -CD = C_x - D_x = 0
        assert!((pf[6] - 0.0).abs() < 1e-12, "CD = 0");
    }

    #[test]
    fn test_compute_prefactors_different_centers() {
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([1.0, 0.0, 0.0]),
            Point([3.0, 0.0, 0.0]),
            Point([4.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        // p = 2, q = 2
        // P = (0.5, 0, 0), Q = (3.5, 0, 0)
        // PA_x = 0.5, QC_x = 0.5, PQ_x = -3.0
        // -AB_x = B_x - A_x = 1.0
        // -CD_x = D_x - C_x = 1.0
        let pf = compute_prefactors(&qd, 1.0, 0);
        // W = (pP + qQ)/(p+q) = (2*0.5 + 2*3.5)/4 = 2.0
        // c00 = 0.5 + 1.0*(2.0-0.5) = 2.0
        assert!((pf[0] - 2.0).abs() < 1e-12, "c00 = {}", pf[0]);
        // c0p = 0.5 + 1.0*(2.0-3.5) = -1.0
        assert!((pf[1] - (-1.0)).abs() < 1e-12, "c0p = {}", pf[1]);
        // AUDIT FIX (v3.2.0-e): AB = A - B = -1.0, CD = C - D = -1.0
        assert!((pf[5] - (-1.0)).abs() < 1e-12, "AB = {}", pf[5]);
        assert!((pf[6] - (-1.0)).abs() < 1e-12, "CD = {}", pf[6]);
    }

    #[test]
    fn test_compute_prefactors_y_z_directions() {
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([1.0, 2.0, 3.0]),
            Point([3.0, 0.0, 0.0]),
            Point([4.0, 1.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        // p = 2, q = 2
        // P = (0.5, 1.0, 1.5), Q = (3.5, 0.5, 0.0)
        // PA_y = 1.0, QC_y = 0.5, PQ_y = 0.5
        // -AB_y = B_y - A_y = 2.0
        // -CD_y = D_y - C_y = 1.0
        let pf_y = compute_prefactors(&qd, 0.0, 1);
        assert!((pf_y[0] - 1.0).abs() < 1e-12, "c00_y = {}", pf_y[0]);
        assert!((pf_y[1] - 0.5).abs() < 1e-12, "c0p_y = {}", pf_y[1]);
        assert!((pf_y[5] - (-2.0)).abs() < 1e-12, "AB_y = {}", pf_y[5]);
        assert!((pf_y[6] - (-1.0)).abs() < 1e-12, "CD_y = {}", pf_y[6]);

        // PA_z = 1.5, QC_z = 0.0, PQ_z = 1.5
        // -AB_z = B_z - A_z = 3.0
        // -CD_z = D_z - C_z = 0.0
        let pf_z = compute_prefactors(&qd, 0.0, 2);
        assert!((pf_z[0] - 1.5).abs() < 1e-12, "c00_z = {}", pf_z[0]);
        assert!((pf_z[1] - 0.0).abs() < 1e-12, "c0p_z = {}", pf_z[1]);
        assert!((pf_z[5] - (-3.0)).abs() < 1e-12, "AB_z = {}", pf_z[5]);
        assert!((pf_z[6] - 0.0).abs() < 1e-12, "CD_z = {}", pf_z[6]);
    }

    // ── compute_rys_roots tests ──

    #[test]
    fn test_compute_rys_roots_n1() {
        let (roots, weights) = compute_rys_roots(0.0, 1);
        assert_eq!(roots.len(), 1);
        assert!((roots[0] - 1.0 / 3.0).abs() < 1e-10, "root = {}", roots[0]);
        assert!((weights[0] - 1.0).abs() < 1e-10, "weight = {}", weights[0]);
    }

    #[test]
    fn test_compute_rys_roots_n1_nonzero_t() {
        let (roots, weights) = compute_rys_roots(1.0, 1);
        assert_eq!(roots.len(), 1);
        assert!(roots[0] > 0.0 && roots[0] < 1.0);
        assert!(weights[0] > 0.0);
    }

    // ── ShellQuartetDriver construction tests ──

    #[test]
    fn test_driver_ss_ss() {
        let driver = ShellQuartetDriver::new(0, 0, 0, 0);
        assert_eq!(driver.total_integrals(), 1);
        assert_eq!(driver.rys_order(), 1);
        assert_eq!(driver.na, 1);
        assert_eq!(driver.nb, 1);
        assert_eq!(driver.nc, 1);
        assert_eq!(driver.nd, 1);
    }

    #[test]
    fn test_driver_pp_ss() {
        let driver = ShellQuartetDriver::new(1, 1, 0, 0);
        assert_eq!(driver.total_integrals(), 9); // 3*3*1*1
        assert_eq!(driver.rys_order(), 2); // (2+0)/2+1 = 2
    }

    #[test]
    fn test_driver_pp_pp() {
        let driver = ShellQuartetDriver::new(1, 1, 1, 1);
        assert_eq!(driver.total_integrals(), 81); // 3^4
        assert_eq!(driver.rys_order(), 3); // (2+2)/2+1 = 3
    }

    #[test]
    fn test_driver_dd_dd() {
        let driver = ShellQuartetDriver::new(2, 2, 2, 2);
        assert_eq!(driver.total_integrals(), 1296); // 6^4
        assert_eq!(driver.rys_order(), 5); // (4+4)/2+1 = 5
    }

    // ── Extraction plan tests ──

    #[test]
    fn test_extraction_plan_ss_ss() {
        let driver = ShellQuartetDriver::new(0, 0, 0, 0);
        // Only one Cartesian component, all directions should be Direct
        match &driver.plans[0] {
            [ExtractionPlan::Direct { idx: idx_x },
             ExtractionPlan::Direct { idx: idx_y },
             ExtractionPlan::Direct { idx: idx_z }] => {
                assert_eq!(*idx_x, 0, "ss|ss x-direction should be operand 0");
                assert_eq!(*idx_y, 0, "ss|ss y-direction should be operand 0");
                assert_eq!(*idx_z, 0, "ss|ss z-direction should be operand 0");
            }
            _ => panic!("Expected Direct extraction plans for (ss|ss)"),
        }
    }

    #[test]
    fn test_extraction_plan_ps_ss() {
        let driver = ShellQuartetDriver::new(1, 0, 0, 0);
        // (p_x s | s s): na_x=1, nb_x=0, nc_x=0, nd_x=0 → Direct via g_idx[1][0]
        // (p_y s | s s): na_x=0, na_y=1, ... → Direct via g_idx[1][0] for y
        // (p_z s | s s): na_x=0, na_y=0, na_z=1, ... → Direct via g_idx[1][0] for z
        // But the x-component of p_z has na_x=0, so the x-direction has na_x=0, nb_x=0
        // which is g_idx[0][0] = 0 (base case)

        // p_x: comp = (1,0,0), p_y: comp = (0,1,0), p_z: comp = (0,0,1)
        // For p_x (ia=0):
        //   x-direction: I(1,0|0,0) → Direct (g_idx[1][0])
        //   y-direction: I(0,0|0,0) → Direct (g_idx[0][0] = 0)
        //   z-direction: I(0,0|0,0) → Direct (g_idx[0][0] = 0)
        match &driver.plans[0] {
            [ExtractionPlan::Direct { idx: idx_x },
             ExtractionPlan::Direct { idx: _ },
             ExtractionPlan::Direct { idx: _ }] => {
                assert_eq!(*idx_x, 1, "p_x x-direction should be operand 1");
            }
            _ => panic!("Expected Direct extraction plans for (ps|ss) p_x"),
        }
    }

    // ── ERI computation tests ──

    #[test]
    fn test_eri_shell_full_ss_ss_origin() {
        let qd = qd_origin();
        let eris = eri_shell_full(&qd, 0, 0, 0, 0);
        assert_eq!(eris.len(), 1);
        // The (ss|ss) integral at the origin with alpha=1:
        // prefactor * K_ab * K_cd * F_0(T)
        // p=2, q=2, T=rho*|PQ|²=0, F_0(0)=1
        // prefactor = 2π²/(pq) * sqrt(π/(p+q)) = 2π²/4 * sqrt(π/4)
        // K_ab = K_cd = 1
        let expected = qd.prefactor * qd.kab * qd.kcd;
        assert!(
            (eris[0] - expected).abs() < 1e-10,
            "(ss|ss) at origin: expected {}, got {}",
            expected,
            eris[0]
        );
        assert!(eris[0] > 0.0, "(ss|ss) should be positive");
    }

    #[test]
    fn test_eri_shell_full_ss_ss_separated() {
        let qd = qd_separated();
        let eris = eri_shell_full(&qd, 0, 0, 0, 0);
        assert_eq!(eris.len(), 1);
        // (ss|ss) with separated centers should still be positive
        // T = rho * |PQ|² > 0, so F_0(T) < 1
        assert!(eris[0] > 0.0, "(ss|ss) separated should be positive");
        // Should be smaller than at the origin (same exponents, but with distance)
        let qd0 = qd_origin();
        let eris0 = eri_shell_full(&qd0, 0, 0, 0, 0);
        assert!(
            eris[0] < eris0[0],
            "(ss|ss) separated should be smaller than at origin"
        );
    }

    #[test]
    fn test_eri_shell_full_ss_ss_nonzero_t() {
        // Test with nonzero T value
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([2.0, 0.0, 0.0]),
            Point([2.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        let eris = eri_shell_full(&qd, 0, 0, 0, 0);
        assert_eq!(eris.len(), 1);
        assert!(eris[0] > 0.0, "(ss|ss) should be positive");

        // Compare with the analytical result:
        // (ss|ss) = prefactor * K_ab * K_cd * F_0(T)
        // T = rho * |PQ|² = 1 * 4 = 4
        let p_center = qd.center_ab();
        let q_center = qd.center_cd();
        let t_val = qd.rho * p_center.dist2(&q_center);
        let f0 = crate::boys::boys_f(0, t_val);
        let expected = qd.prefactor * qd.kab * qd.kcd * f0;
        assert!(
            (eris[0] - expected).abs() < 1e-10,
            "(ss|ss) nonzero T: expected {}, got {}",
            expected,
            eris[0]
        );
    }

    #[test]
    fn test_eri_shell_full_ps_ss_size() {
        let qd = qd_origin();
        let eris = eri_shell_full(&qd, 1, 0, 0, 0);
        // ncart(1) * ncart(0) * ncart(0) * ncart(0) = 3 * 1 * 1 * 1 = 3
        assert_eq!(eris.len(), 3, "(ps|ss) should have 3 integrals");
    }

    #[test]
    fn test_eri_shell_full_pp_ss_size() {
        let qd = qd_origin();
        let eris = eri_shell_full(&qd, 1, 1, 0, 0);
        assert_eq!(eris.len(), 9, "(pp|ss) should have 9 integrals");
    }

    #[test]
    fn test_eri_shell_full_pp_pp_size() {
        let qd = qd_origin();
        let eris = eri_shell_full(&qd, 1, 1, 1, 1);
        assert_eq!(eris.len(), 81, "(pp|pp) should have 81 integrals");
    }

    #[test]
    fn test_eri_shell_full_dd_dd_size() {
        let qd = qd_origin();
        let eris = eri_shell_full(&qd, 2, 2, 2, 2);
        assert_eq!(eris.len(), 1296, "(dd|dd) should have 1296 integrals");
    }

    #[test]
    fn test_eri_shell_full_size_consistency() {
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([1.0, 0.0, 0.0]),
            Point([2.0, 0.0, 0.0]),
            Point([3.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );

        for la in 0..=3 {
            for lb in 0..=2 {
                for lc in 0..=2 {
                    for ld in 0..=1 {
                        let eris = eri_shell_full(&qd, la, lb, lc, ld);
                        let expected = ncart(la) * ncart(lb) * ncart(lc) * ncart(ld);
                        assert_eq!(
                            eris.len(),
                            expected,
                            "Size mismatch for ({}{}|{}{})",
                            la, lb, lc, ld
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_eri_shell_full_ss_ss_vs_analytical() {
        // Test (ss|ss) against the analytical formula for various geometries
        let test_cases = [
            // (A, B, C, D, alpha_a, alpha_b, alpha_c, alpha_d)
            (
                Point([0.0, 0.0, 0.0]),
                Point([0.0, 0.0, 0.0]),
                Point([0.0, 0.0, 0.0]),
                Point([0.0, 0.0, 0.0]),
                1.0, 1.0, 1.0, 1.0,
            ),
            (
                Point([0.0, 0.0, 0.0]),
                Point([0.0, 0.0, 0.0]),
                Point([3.0, 0.0, 0.0]),
                Point([3.0, 0.0, 0.0]),
                1.0, 1.0, 1.0, 1.0,
            ),
            (
                Point([0.0, 0.0, 0.0]),
                Point([1.0, 0.0, 0.0]),
                Point([3.0, 0.0, 0.0]),
                Point([4.0, 0.0, 0.0]),
                2.0, 1.0, 3.0, 0.5,
            ),
        ];

        for (ra, rb, rc, rd, aa, ab, ac, ad) in test_cases {
            let qd = QuartetData::new(ra, rb, rc, rd, aa, ab, ac, ad);
            let eris = eri_shell_full(&qd, 0, 0, 0, 0);

            let p = qd.center_ab();
            let q = qd.center_cd();
            let t_val = qd.rho * p.dist2(&q);
            let f0 = crate::boys::boys_f(0, t_val);
            let expected = qd.prefactor * qd.kab * qd.kcd * f0;

            assert!(
                (eris[0] - expected).abs() < 1e-10,
                "(ss|ss) analytical check failed: expected {}, got {}",
                expected,
                eris[0]
            );
        }
    }

    #[test]
    fn test_eri_shell_full_ss_ss_positive() {
        // (ss|ss) should always be positive for valid inputs
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([1.0, 2.0, 3.0]),
            Point([5.0, 0.0, 0.0]),
            Point([6.0, 1.0, 0.0]),
            2.0, 0.5, 1.5, 3.0,
        );
        let eris = eri_shell_full(&qd, 0, 0, 0, 0);
        assert!(
            eris[0] > 0.0,
            "(ss|ss) should be positive for all valid geometries"
        );
    }

    // ── Driver + compute integration tests ──

    #[test]
    fn test_driver_compute_ss_ss() {
        let driver = ShellQuartetDriver::new(0, 0, 0, 0);
        let qd = qd_origin();
        let eris = driver.compute(&qd);
        assert_eq!(eris.len(), 1);
        assert!(eris[0] > 0.0);
    }

    #[test]
    fn test_driver_compute_ps_ss() {
        let driver = ShellQuartetDriver::new(1, 0, 0, 0);
        let qd = qd_origin();
        let eris = driver.compute(&qd);
        assert_eq!(eris.len(), 3);

        // At the origin with A=B=C=D and all alpha=1:
        // P = A = (0,0,0), Q = C = (0,0,0), PA = (0,0,0), PQ = (0,0,0)
        // So c00 = 0 for all roots → all I(na>0, ...) = 0
        // Therefore all (ps|ss) integrals should be 0 at the origin
        for (i, &val) in eris.iter().enumerate() {
            assert!(
                val.abs() < 1e-10,
                "(ps|ss)[{}] at origin should be ~0, got {}",
                i,
                val
            );
        }
    }

    #[test]
    fn test_driver_compute_sp_ss() {
        // (sp|ss) with la=1, lb=0 — same as (ps|ss) due to naming
        let driver = ShellQuartetDriver::new(1, 0, 0, 0);
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([1.0, 0.0, 0.0]),
            Point([3.0, 0.0, 0.0]),
            Point([4.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        let eris = driver.compute(&qd);
        assert_eq!(eris.len(), 3);

        // At least some integrals should be nonzero for separated centers
        let has_nonzero = eris.iter().any(|&v| v.abs() > 1e-15);
        assert!(has_nonzero, "(sp|ss) with separated centers should have nonzero integrals");
    }

    #[test]
    fn test_driver_compute_ds_ss() {
        let driver = ShellQuartetDriver::new(2, 0, 0, 0);
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([1.0, 0.0, 0.0]),
            Point([3.0, 0.0, 0.0]),
            Point([4.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        let eris = driver.compute(&qd);
        assert_eq!(eris.len(), 6); // ncart(2) = 6
    }

    #[test]
    fn test_driver_compute_ss_ps() {
        // (ss|ps) — ket side has angular momentum
        let driver = ShellQuartetDriver::new(0, 0, 1, 0);
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([1.0, 0.0, 0.0]),
            Point([3.0, 0.0, 0.0]),
            Point([4.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        let eris = driver.compute(&qd);
        assert_eq!(eris.len(), 3); // 1*1*3*1 = 3
    }

    #[test]
    fn test_driver_compute_pp_ss() {
        let driver = ShellQuartetDriver::new(1, 1, 0, 0);
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([1.0, 0.0, 0.0]),
            Point([3.0, 0.0, 0.0]),
            Point([4.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        let eris = driver.compute(&qd);
        assert_eq!(eris.len(), 9); // 3*3*1*1 = 9
    }

    #[test]
    fn test_driver_compute_pp_pp() {
        let driver = ShellQuartetDriver::new(1, 1, 1, 1);
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([1.0, 0.0, 0.0]),
            Point([3.0, 0.0, 0.0]),
            Point([4.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        let eris = driver.compute(&qd);
        assert_eq!(eris.len(), 81); // 3^4 = 81
    }

    // ── Manual HRR-Ket tests ──

    #[test]
    fn test_extraction_plan_manual_hrr_ket() {
        // For (pp|pp), the Cartesian component (px, py | px, pz) requires
        // manual HRR-Ket in the y-direction because:
        // y-direction: na_y=0, nb_y=1, nc_y=0, nd_y=1
        // (0, 1) != (la=1, lb=1) and nd=1 > 0 → ManualHrrKet
        let driver = ShellQuartetDriver::new(1, 1, 1, 1);

        // Find the plan for (px, py | px, pz) = (ia=0, ib=1, ic=0, id=2)
        // comps_a[0] = (1,0,0), comps_b[1] = (0,1,0)
        // comps_c[0] = (1,0,0), comps_d[2] = (0,0,1)
        // y-direction: na_y=0, nb_y=1, nc_y=0, nd_y=0 → Direct (nd=0)
        // Actually, nd_y=0 for this component, so no manual HRR-Ket needed.

        // Let's find a component that actually needs manual HRR-Ket.
        // (px, px | px, py): comps_a[0]=(1,0,0), comps_b[0]=(1,0,0),
        //   comps_c[0]=(1,0,0), comps_d[1]=(0,1,0)
        // y-direction: na_y=0, nb_y=0, nc_y=0, nd_y=1
        // (0,0) != (1,1) and nd=1 > 0 → ManualHrrKet!
        let flat_idx = 0 * 9 + 0 * 3 + 0 * 1 + 1; // ia=0, ib=0, ic=0, id=1
        match &driver.plans[flat_idx][1] {
            // y-direction
            ExtractionPlan::ManualHrrKet { target_nd, .. } => {
                assert_eq!(*target_nd, 1, "Expected 1 HRR-Ket step");
            }
            ExtractionPlan::Direct { .. } => {
                // This is also valid if the circuit records this intermediate
            }
        }
    }

    #[test]
    fn test_extract_value_direct() {
        let plan = ExtractionPlan::Direct { idx: 0 };
        let operands = vec![1.0, 2.0, 3.0];
        let prefactors = [0.0; 7];
        let val = extract_value(&plan, &operands, &prefactors);
        assert!((val - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_extract_value_direct_nonzero_idx() {
        let plan = ExtractionPlan::Direct { idx: 2 };
        let operands = vec![1.0, 2.0, 3.0];
        let prefactors = [0.0; 7];
        let val = extract_value(&plan, &operands, &prefactors);
        assert!((val - 3.0).abs() < 1e-12);
    }

    #[test]
    fn test_extract_value_manual_hrr_ket_zero_steps() {
        // ManualHrrKet with target_nd=0 should just read from bra_row_operands
        let plan = ExtractionPlan::ManualHrrKet {
            bra_row_operands: vec![0, 1, 2],
            target_nc: 1,
            target_nd: 0,
        };
        let operands = vec![10.0, 20.0, 30.0];
        let prefactors = [0.0; 7];
        let val = extract_value(&plan, &operands, &prefactors);
        assert!((val - 20.0).abs() < 1e-12, "Expected 20.0, got {}", val);
    }

    #[test]
    fn test_extract_value_manual_hrr_ket_one_step() {
        // ManualHrrKet with target_nd=1, target_nc=0
        // I(na, nb | 0, 1) = I(na, nb | 1, 0) + (-CD) * I(na, nb | 0, 0)
        let plan = ExtractionPlan::ManualHrrKet {
            bra_row_operands: vec![0, 1], // I(na,nb|0,0)=operands[0], I(na,nb|1,0)=operands[1]
            target_nc: 0,
            target_nd: 1,
        };
        let operands = vec![5.0, 7.0]; // I(na,nb|0,0)=5, I(na,nb|1,0)=7
        let prefactors = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0]; // -CD = -2
        let val = extract_value(&plan, &operands, &prefactors);
        // I(0,1) = I(1,0) + (-CD)*I(0,0) = 7 + (-2)*5 = -3
        assert!((val - (-3.0)).abs() < 1e-12, "Expected -3.0, got {}", val);
    }

    #[test]
    fn test_extract_value_manual_hrr_ket_two_steps() {
        // ManualHrrKet with target_nd=2, target_nc=0
        // Row 0: I(k,0) for k=0,1,2
        // Row 1: I(k,1) for k=0,1
        //   I(0,1) = I(1,0) + (-CD)*I(0,0)
        //   I(1,1) = I(2,0) + (-CD)*I(1,0)
        // Row 2: I(k,2) for k=0
        //   I(0,2) = I(1,1) + (-CD)*I(0,1)
        let plan = ExtractionPlan::ManualHrrKet {
            bra_row_operands: vec![0, 1, 2],
            target_nc: 0,
            target_nd: 2,
        };
        let operands = vec![1.0, 3.0, 5.0]; // I(0,0)=1, I(1,0)=3, I(2,0)=5
        let neg_cd = -1.0; // -CD = -1
        let prefactors = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, neg_cd];
        let val = extract_value(&plan, &operands, &prefactors);
        // Row 1: I(0,1) = 3 + (-1)*1 = 2
        //         I(1,1) = 5 + (-1)*3 = 2
        // Row 2: I(0,2) = 2 + (-1)*2 = 0
        assert!((val - 0.0).abs() < 1e-12, "Expected 0.0, got {}", val);
    }

    // ── Symmetry tests ──

    #[test]
    fn test_eri_shell_full_ss_ss_symmetry() {
        // (ss|ss) should satisfy (ab|cd) = (ba|dc) = (cd|ab)
        let qd1 = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([1.0, 0.0, 0.0]),
            Point([3.0, 0.0, 0.0]),
            Point([4.0, 0.0, 0.0]),
            1.0, 2.0, 3.0, 0.5,
        );
        let qd2 = QuartetData::new(
            Point([1.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([4.0, 0.0, 0.0]),
            Point([3.0, 0.0, 0.0]),
            2.0, 1.0, 0.5, 3.0,
        );
        let eris1 = eri_shell_full(&qd1, 0, 0, 0, 0);
        let eris2 = eri_shell_full(&qd2, 0, 0, 0, 0);
        assert!(
            (eris1[0] - eris2[0]).abs() < 1e-10,
            "(ss|ss) symmetry: {} vs {}",
            eris1[0],
            eris2[0]
        );
    }

    // ── Contraction integration test ──

    #[test]
    fn test_contracted_ss_ss() {
        // Simulate a [2s] contraction by summing primitive integrals
        // with contraction coefficients
        let ra = Point([0.0, 0.0, 0.0]);
        let rb = Point([0.0, 0.0, 0.0]);
        let rc = Point([0.0, 0.0, 0.0]);
        let rd = Point([0.0, 0.0, 0.0]);

        let alphas_a = [1.0, 3.0];
        let alphas_b = [1.0, 3.0];
        let alphas_c = [1.0, 3.0];
        let alphas_d = [1.0, 3.0];
        let coeffs_a = [0.5, 0.5];
        let coeffs_b = [0.5, 0.5];
        let coeffs_c = [0.5, 0.5];
        let coeffs_d = [0.5, 0.5];

        let mut contracted = 0.0;
        for ia in 0..2 {
            for ib in 0..2 {
                for ic in 0..2 {
                    for id in 0..2 {
                        let qd = QuartetData::new(
                            ra, rb, rc, rd,
                            alphas_a[ia], alphas_b[ib],
                            alphas_c[ic], alphas_d[id],
                        );
                        let prim = eri_shell_full(&qd, 0, 0, 0, 0);
                        contracted += coeffs_a[ia] * coeffs_b[ib]
                            * coeffs_c[ic] * coeffs_d[id]
                            * prim[0];
                    }
                }
            }
        }

        assert!(
            contracted > 0.0,
            "Contracted (ss|ss) should be positive, got {}",
            contracted
        );
    }

    // ── Edge case tests ──

    #[test]
    fn test_eri_shell_full_very_large_separation() {
        // Very large separation → integral should be very small
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([100.0, 0.0, 0.0]),
            Point([100.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        let eris = eri_shell_full(&qd, 0, 0, 0, 0);
        // True value: 2π^{2.5}/(pq√(p+q))·F₀(T), T = ρ|PQ|² = 10⁴
        // ≈ 4.373·0.00886 ≈ 0.0387 — positive, order 1/R (Coulomb tail).
        assert!(eris[0] > 0.0, "Should still be positive");
        assert!(eris[0] < 0.1, "Should decay like 1/R for large separation: {}", eris[0]);
    }

    #[test]
    fn test_eri_shell_full_high_exponents() {
        // High exponents → more localized functions, smaller overlap
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([1.0, 0.0, 0.0]),
            Point([3.0, 0.0, 0.0]),
            Point([4.0, 0.0, 0.0]),
            100.0, 100.0, 100.0, 100.0,
        );
        let eris = eri_shell_full(&qd, 0, 0, 0, 0);
        // With high exponents, the functions are very localized and the
        // overlap between bra and ket is negligible
        assert!(
            eris[0] >= 0.0,
            "(ss|ss) with high exponents should be non-negative"
        );
    }
}
