//! Schwarz integral screening for two-electron integrals.
//!
//! # What this module does
//!
//! This module implements the Schwarz inequality for screening two-electron
//! repulsion integrals (ERIs). The Schwarz inequality provides an upper bound
//! on the magnitude of any ERI:
//!
//! ```text
//! |(μν|λσ)| ≤ √(μμ|νν) · √(λλ|σσ)
//! ```
//!
//! By precomputing the diagonal shell-pair estimates `(μμ|μμ)` and `(νν|νν)`,
//! we can skip integral quartets that are guaranteed to be below a given
//! threshold.
//!
//! # Why this module exists
//!
//! In a typical molecular calculation with N basis functions, there are O(N⁴)
//! possible ERI quartets. However, most of these are negligibly small because
//! the basis functions have little spatial overlap. The Schwarz inequality lets
//! us identify and skip these negligible integrals without computing them,
//! reducing the effective scaling from O(N⁴) to near O(N²) for large molecules.
//!
//! # How it works
//!
//! 1. **Precompute diagonal estimates**: For each shell pair (μ, ν), compute
//!    an estimate of the maximum ERI magnitude: `Q_μν = √(μν|μν)`.
//!    In practice, we approximate `(μν|μν) ≈ S_μμ · S_νν` where S is the
//!    overlap-like diagonal quantity.
//!
//! 2. **Screen quartets**: For a quartet (μ,ν|λ,σ), compute
//!    `√(Q_μν · Q_λσ)` and compare against the threshold. If below the
//!    threshold, the integral is skipped.

use libm::sqrt;

/// A Schwarz screening estimate for a shell pair.
///
/// # What it represents
///
/// Each `SchwarzEstimate` stores the upper bound on the maximum ERI magnitude
/// for a given shell pair (μ, ν), along with the indices identifying the pair.
/// The estimate is derived from the diagonal ERI `(μν|μν)` via:
///
/// ```text
/// Q_μν = √(μν|μν) ≈ √(S_μμ · S_νν)
/// ```
///
/// # How it is used
///
/// Two `SchwarzEstimate` values are combined in [`should_compute`] to decide
/// whether a particular integral quartet should be evaluated or skipped.
#[derive(Debug, Clone)]
pub struct SchwarzEstimate {
    /// Upper bound on the maximum ERI for this shell pair: √((ab|ab))
    pub max_integral: f64,
    /// Indices of the two shells in the pair (i, j)
    pub shell_pair_idx: (usize, usize),
}

/// Compute Schwarz screening estimates for all shell pairs.
///
/// # What it does
///
/// Given a vector of diagonal ERI values (or overlap-like estimates) for each
/// shell, computes the pairwise Schwarz estimate for every unique shell pair.
/// The estimate for pair (i, j) is:
///
/// ```text
/// Q_ij = √(eri_diag[i] · eri_diag[j])
/// ```
///
/// where `eri_diag[i]` approximates `(ii|ii)` — the diagonal ERI of shell i
/// with itself.
///
/// # Why it exists
///
/// Computing all pairwise estimates upfront allows the screening check to be
/// performed with a single multiplication and comparison, rather than computing
/// square roots on the fly during the integral loop. This is critical for
/// performance: the screening check is executed O(N²) times in the outer loop
/// and must be as cheap as possible.
///
/// # How it works
///
/// For each pair (i, j) with `i ≤ j`:
///
/// ```text
/// max_integral = √(eri_diag[i] · eri_diag[j])
/// ```
///
/// The number of estimates returned is `n * (n + 1) / 2` (all unique pairs
/// including diagonal).
///
/// # Arguments
///
/// * `eri_diag` — Diagonal ERI estimates for each shell, length `n`.
///   `eri_diag[i]` should approximate `(ii|ii)` for shell i.
/// * `n` — Number of shells (must equal `eri_diag.len()`).
///
/// # Returns
///
/// A `Vec<SchwarzEstimate>` containing the screening estimate for each unique
/// shell pair, ordered as (0,0), (0,1), ..., (0,n-1), (1,1), (1,2), ..., (n-1,n-1).
///
/// # Panics
///
/// Panics if `eri_diag.len() != n`.
///
/// # Examples
///
/// ```
/// use poler_eri::screening::{schwarz_estimate, SchwarzEstimate};
///
/// let eri_diag = vec![1.0, 0.5, 0.01];
/// let estimates = schwarz_estimate(&eri_diag, 3);
///
/// // 3 shells → 3*4/2 = 6 unique pairs
/// assert_eq!(estimates.len(), 6);
///
/// // Pair (0,0): √(1.0 * 1.0) = 1.0
/// assert!((estimates[0].max_integral - 1.0).abs() < 1e-14);
///
/// // Pair (0,1): √(1.0 * 0.5) ≈ 0.707
/// let expected_01 = libm::sqrt(1.0 * 0.5);
/// assert!((estimates[1].max_integral - expected_01).abs() < 1e-14);
/// ```
pub fn schwarz_estimate(eri_diag: &[f64], n: usize) -> Vec<SchwarzEstimate> {
    assert_eq!(eri_diag.len(), n,
        "schwarz_estimate: eri_diag length {} != n={}",
        eri_diag.len(), n);

    let n_pairs = n * (n + 1) / 2;
    let mut estimates = Vec::with_capacity(n_pairs);

    for i in 0..n {
        for j in i..n {
            let max_integral = sqrt(eri_diag[i] * eri_diag[j]);
            estimates.push(SchwarzEstimate {
                max_integral,
                shell_pair_idx: (i, j),
            });
        }
    }

    estimates
}

/// Determine whether an integral quartet should be computed based on Schwarz screening.
///
/// # What it does
///
/// Given the Schwarz estimates for two shell pairs (the bra pair ij and the
/// ket pair kl), determines whether the integral (ij|kl) is likely above the
/// screening threshold. The decision is based on the Schwarz inequality:
///
/// ```text
/// |(ij|kl)| ≤ √(Q_ij · Q_kl)
/// ```
///
/// If `√(Q_ij · Q_kl) ≤ threshold`, the integral is guaranteed to be below
/// the threshold and can be safely skipped.
///
/// # Why it exists
///
/// This is the inner-loop screening decision that is called for every potential
/// integral quartet. It must be extremely fast — just one multiplication, one
/// square root, and one comparison. The precomputed estimates from
/// [`schwarz_estimate`] make this possible.
///
/// # How it works
///
/// ```text
/// should_compute = √(estimate_ij · estimate_kl) > threshold
/// ```
///
/// Equivalently, to avoid the square root:
///
/// ```text
/// should_compute = estimate_ij · estimate_kl > threshold²
/// ```
///
/// However, we use the explicit square root form for clarity and because
/// `estimate_ij` and `estimate_kl` are already square roots of the diagonal
/// ERIs.
///
/// # Arguments
///
/// * `estimate_ij` — Schwarz estimate for the bra shell pair: `√((ij|ij))`
/// * `estimate_kl` — Schwarz estimate for the ket shell pair: `√((kl|kl))`
/// * `threshold` — Screening threshold (typically 1e-10 to 1e-14). Integrals
///   whose upper bound is below this threshold will be skipped.
///
/// # Returns
///
/// `true` if the integral should be computed (likely above threshold),
/// `false` if it should be skipped (guaranteed below threshold).
///
/// # Examples
///
/// ```
/// use poler_eri::screening::should_compute;
///
/// // Large estimates: should compute
/// assert!(should_compute(1.0, 1.0, 1e-10));
///
/// // Zero estimates: should skip
/// assert!(!should_compute(0.0, 1.0, 1e-10));
///
/// // Marginal case: √(1e-20 * 1e-20) = 1e-20 < 1e-10
/// assert!(!should_compute(1e-20, 1e-20, 1e-10));
///
/// // Just above threshold: √(1e-5 * 1e-5) = 1e-5 > 1e-10
/// assert!(should_compute(1e-5, 1e-5, 1e-10));
/// ```
pub fn should_compute(estimate_ij: f64, estimate_kl: f64, threshold: f64) -> bool {
    sqrt(estimate_ij * estimate_kl) > threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schwarz_estimate_single_shell() {
        let eri_diag = vec![4.0];
        let estimates = schwarz_estimate(&eri_diag, 1);
        assert_eq!(estimates.len(), 1);
        assert_eq!(estimates[0].shell_pair_idx, (0, 0));
        // sqrt(4.0 * 4.0) = 4.0
        assert!((estimates[0].max_integral - 4.0).abs() < 1e-14);
    }

    #[test]
    fn test_schwarz_estimate_two_shells() {
        let eri_diag = vec![4.0, 9.0];
        let estimates = schwarz_estimate(&eri_diag, 2);
        // 2*3/2 = 3 pairs: (0,0), (0,1), (1,1)
        assert_eq!(estimates.len(), 3);

        // (0,0): √(4*4) = 4... wait, √(eri_diag[0] * eri_diag[0]) = √(16) = 4
        // No, it's √(4.0 * 4.0) = 4.0... hmm wait
        // Actually: sqrt(eri_diag[i] * eri_diag[j]) where eri_diag are the diagonal ERIs
        // sqrt(4.0 * 4.0) = 4.0
        assert!((estimates[0].max_integral - 4.0).abs() < 1e-14);

        // (0,1): √(4.0 * 9.0) = 6.0
        assert!((estimates[1].max_integral - 6.0).abs() < 1e-14);

        // (1,1): √(9.0 * 9.0) = 9.0
        assert!((estimates[2].max_integral - 9.0).abs() < 1e-14);
    }

    #[test]
    fn test_schwarz_estimate_three_shells() {
        let eri_diag = vec![1.0, 0.5, 0.01];
        let estimates = schwarz_estimate(&eri_diag, 3);
        // 3*4/2 = 6 pairs
        assert_eq!(estimates.len(), 6);

        // (0,0): √(1.0 * 1.0) = 1.0
        assert!((estimates[0].max_integral - 1.0).abs() < 1e-14);
        assert_eq!(estimates[0].shell_pair_idx, (0, 0));

        // (0,1): √(1.0 * 0.5)
        assert!((estimates[1].max_integral - sqrt(0.5)).abs() < 1e-14);
        assert_eq!(estimates[1].shell_pair_idx, (0, 1));

        // (0,2): √(1.0 * 0.01) = 0.1
        assert!((estimates[2].max_integral - 0.1).abs() < 1e-14);
        assert_eq!(estimates[2].shell_pair_idx, (0, 2));

        // (1,1): √(0.5 * 0.5) = 0.5
        assert!((estimates[3].max_integral - 0.5).abs() < 1e-14);
        assert_eq!(estimates[3].shell_pair_idx, (1, 1));

        // (1,2): √(0.5 * 0.01)
        assert!((estimates[4].max_integral - sqrt(0.005)).abs() < 1e-14);
        assert_eq!(estimates[4].shell_pair_idx, (1, 2));

        // (2,2): √(0.01 * 0.01) = 0.01
        assert!((estimates[5].max_integral - 0.01).abs() < 1e-14);
        assert_eq!(estimates[5].shell_pair_idx, (2, 2));
    }

    #[test]
    fn test_schwarz_estimate_symmetry() {
        // The estimate for (i,j) equals the estimate for (j,i)
        let eri_diag = vec![3.0, 7.0];
        let estimates = schwarz_estimate(&eri_diag, 2);

        // Only (0,1) is stored (not (1,0)), but the value should be symmetric
        let val_01 = estimates[1].max_integral;
        let expected = sqrt(3.0 * 7.0);
        assert!((val_01 - expected).abs() < 1e-14);
    }

    #[test]
    fn test_schwarz_estimate_zero_diagonal() {
        let eri_diag = vec![1.0, 0.0, 2.0];
        let estimates = schwarz_estimate(&eri_diag, 3);

        // Pairs involving shell 1 should have zero estimate
        // (0,1): √(1.0 * 0.0) = 0.0
        assert!(estimates[1].max_integral.abs() < 1e-14);
        // (1,1): √(0.0 * 0.0) = 0.0
        assert!(estimates[3].max_integral.abs() < 1e-14);
        // (1,2): √(0.0 * 2.0) = 0.0
        assert!(estimates[4].max_integral.abs() < 1e-14);
    }

    #[test]
    fn test_should_compute_large_estimates() {
        assert!(should_compute(1.0, 1.0, 1e-10));
        assert!(should_compute(0.1, 0.1, 1e-10));
    }

    #[test]
    fn test_should_compute_zero_estimates() {
        assert!(!should_compute(0.0, 1.0, 1e-10));
        assert!(!should_compute(1.0, 0.0, 1e-10));
        assert!(!should_compute(0.0, 0.0, 1e-10));
    }

    #[test]
    fn test_should_compute_marginal() {
        // √(1e-20) = 1e-10, which is NOT > 1e-10 (strictly greater)
        assert!(!should_compute(1e-10, 1e-10, 1e-10));

        // √(1e-18) = 1e-9 > 1e-10
        assert!(should_compute(1e-9, 1e-9, 1e-10));
    }

    #[test]
    fn test_should_compute_decreasing_thresholds() {
        // With fixed estimates, lowering the threshold should eventually include the integral
        let eij = 1e-8;
        let ekl = 1e-8;

        assert!(!should_compute(eij, ekl, 1e-6)); // √(1e-16) = 1e-8 < 1e-6? No, 1e-8 < 1e-6, so skip
        assert!(should_compute(eij, ekl, 1e-10));  // 1e-8 > 1e-10, so compute
    }

    #[test]
    fn test_should_compute_symmetry() {
        // should_compute(a, b, t) == should_compute(b, a, t)
        assert_eq!(
            should_compute(0.5, 0.3, 1e-10),
            should_compute(0.3, 0.5, 1e-10)
        );
    }

    #[test]
    fn test_schwarz_screening_consistency() {
        // Verify that should_compute is consistent with the estimates from schwarz_estimate
        let eri_diag = vec![1.0, 0.5, 0.01];
        let estimates = schwarz_estimate(&eri_diag, 3);
        let threshold = 1e-5;

        // Check that every pair of estimates is consistently screened
        for (i, est_ij) in estimates.iter().enumerate() {
            for (j, est_kl) in estimates.iter().enumerate() {
                let product = est_ij.max_integral * est_kl.max_integral;
                let upper_bound = sqrt(product);
                let decision = should_compute(est_ij.max_integral, est_kl.max_integral, threshold);
                assert_eq!(decision, upper_bound > threshold,
                    "Inconsistent screening for pairs ({}, {}): upper_bound={}, threshold={}",
                    i, j, upper_bound, threshold);
            }
        }
    }
}
