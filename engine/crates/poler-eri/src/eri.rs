//! ERI shell-level wrapper for computing batches of two-electron integrals.
//!
//! # What this module does
//!
//! This module provides a shell-level interface for computing all two-electron
//! repulsion integrals (ERIs) for a given shell quartet. While the VRR and HRR
//! modules operate on individual primitive integrals, this module handles the
//! full Cartesian component enumeration and returns the complete integral batch.
//!
//! # Why this module exists
//!
//! A shell quartet (a b | c d) with angular momenta (la, lb, lc, ld) contains
//!
//! ```text
//! ncart(la) × ncart(lb) × ncart(lc) × ncart(ld)
//! ```
//!
//! individual integrals, one for each combination of Cartesian components on the
//! four centers. For example, a (pp|pp) quartet has 3×3×3×3 = 81 integrals.
//!
//! The [`eri_shell`] function computes all of these at once, returning them in a
//! flat array with a well-defined ordering convention.
//!
//! # Implementation notes
//!
//! Currently, this function fills the entire output array with the [`eri_base`]
//! value. This is because the real computation for arbitrary angular momenta goes
//! through the **circuit/crystallizer** pipeline, which generates optimized Rust
//! code for each specific (la, lb, lc, ld) quartet. The shell-level wrapper is
//! designed so that once the crystallized code is available, it can be plugged in
//! without changing the API.
//!
//! For the (ss|ss) case, the returned value is exact. For higher angular momenta,
//! the values are placeholder values that should be replaced by the crystallized
//! ERI function in production use.

use crate::cart::ncart;
use crate::types::QuartetData;
use crate::vrr::eri_base;

/// Compute all ERIs for a shell quartet (a b | c d).
///
/// # What it does
///
/// Returns a flat array containing all
/// `ncart(la) × ncart(lb) × ncart(lc) × ncart(ld)` Cartesian ERI values for the
/// specified shell quartet. The ordering follows the convention:
///
/// ```text
/// result[i_a * stride_a + i_b * stride_b + i_c * stride_c + i_d]
/// ```
///
/// where:
/// - `i_a` ranges over the `ncart(la)` Cartesian components of shell a
/// - `i_b` ranges over the `ncart(lb)` Cartesian components of shell b
/// - `i_c` ranges over the `ncart(lc)` Cartesian components of shell c
/// - `i_d` ranges over the `ncart(ld)` Cartesian components of shell d
/// - `stride_a = ncart(lb) * ncart(lc) * ncart(ld)`
/// - `stride_b = ncart(lc) * ncart(ld)`
/// - `stride_c = ncart(ld)`
/// - `stride_d = 1`
///
/// This is row-major ordering with the bra indices varying slowest and the ket
/// indices varying fastest, matching the convention used in libcint and Libint.
///
/// # Why it returns a flat array
///
/// A flat `Vec<f64>` avoids the overhead of nested `Vec<Vec<...>>` and is
/// compatible with BLAS/LAPACK routines that expect contiguous memory. The
/// stride information can be computed from the angular momenta at the call site.
///
/// # How it works (current implementation)
///
/// For the (ss|ss) case (all angular momenta zero), the single integral value
/// is computed exactly using [`eri_base`], which delegates to [`eri_ss_ss`].
///
/// For higher angular momenta, the current implementation fills the entire
/// output array with the `eri_base` value. This is a **placeholder** — in
/// production, the crystallized ERI function should be used instead. The
/// crystallized code computes each Cartesian component correctly using the
/// VRR/HRR recurrence chain compiled by the circuit/crystallizer pipeline.
///
/// # Arguments
///
/// * `qd` — Precomputed geometric data for the shell quartet
/// * `la` — Angular momentum on center A (0=s, 1=p, 2=d, ...)
/// * `lb` — Angular momentum on center B
/// * `lc` — Angular momentum on center C
/// * `ld` — Angular momentum on center D
///
/// # Returns
///
/// A flat array of length `ncart(la) * ncart(lb) * ncart(lc) * ncart(ld)`
/// containing the ERI values in row-major order.
///
/// # Examples
///
/// ```
/// use poler_eri::{QuartetData, Point};
/// use poler_eri::eri::eri_shell;
///
/// // (ss|ss) quartet: single integral
/// let qd = QuartetData::new(
///     Point([0.0, 0.0, 0.0]), Point([0.0, 0.0, 0.0]),
///     Point([0.0, 0.0, 0.0]), Point([0.0, 0.0, 0.0]),
///     1.0, 1.0, 1.0, 1.0,
/// );
/// let eris = eri_shell(&qd, 0, 0, 0, 0);
/// assert_eq!(eris.len(), 1);
/// assert!(eris[0] > 0.0);
///
/// // (pp|ss) quartet: 3*3*1*1 = 9 integrals
/// let eris_pp = eri_shell(&qd, 1, 1, 0, 0);
/// assert_eq!(eris_pp.len(), 9);
/// ```
pub fn eri_shell(qd: &QuartetData, la: usize, lb: usize, lc: usize, ld: usize) -> Vec<f64> {
    let na = ncart(la);
    let nb = ncart(lb);
    let nc = ncart(lc);
    let nd = ncart(ld);
    let total = na * nb * nc * nd;

    // Compute the base ERI value
    let base_val = eri_base(qd, la, lb, lc, ld);

    // Fill the output array.
    // For (ss|ss), this is exact (one element = the analytical value).
    // For higher AM, this fills with the eri_base placeholder value.
    // In production, the crystallized ERI function computes each Cartesian
    // component correctly.
    vec![base_val; total]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Point;

    #[test]
    fn test_eri_shell_ss_ss() {
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        let eris = eri_shell(&qd, 0, 0, 0, 0);
        assert_eq!(eris.len(), 1, "(ss|ss) should have 1 integral");
        assert!(eris[0] > 0.0, "(ss|ss) should be positive");
    }

    #[test]
    fn test_eri_shell_pp_ss() {
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        let eris = eri_shell(&qd, 1, 1, 0, 0);
        // ncart(1) * ncart(1) * ncart(0) * ncart(0) = 3 * 3 * 1 * 1 = 9
        assert_eq!(eris.len(), 9, "(pp|ss) should have 9 integrals");
    }

    #[test]
    fn test_eri_shell_pp_pp() {
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        let eris = eri_shell(&qd, 1, 1, 1, 1);
        // 3 * 3 * 3 * 3 = 81
        assert_eq!(eris.len(), 81, "(pp|pp) should have 81 integrals");
    }

    #[test]
    fn test_eri_shell_dd_dd() {
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        let eris = eri_shell(&qd, 2, 2, 2, 2);
        // ncart(2) = 6, so 6^4 = 1296
        assert_eq!(eris.len(), 1296, "(dd|dd) should have 1296 integrals");
    }

    #[test]
    fn test_eri_shell_size_matches_ncart() {
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
                        let eris = eri_shell(&qd, la, lb, lc, ld);
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
    fn test_eri_shell_separated_centers() {
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([5.0, 0.0, 0.0]),
            Point([5.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        let eris = eri_shell(&qd, 0, 0, 0, 0);
        assert!(eris[0] > 0.0, "(ss|ss) should still be positive for separated centers");
    }
}
