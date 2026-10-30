//! Overlap integral evaluation for Gaussian basis functions.
//!
//! # What this module does
//!
//! This module computes overlap integrals between Gaussian-type orbitals (GTOs).
//! Currently, it provides the base case for two s-type (l=0) primitive Gaussians,
//! which is the fundamental building block from which all higher-angular-momentum
//! overlap integrals can be derived via recurrence relations.
//!
//! # Why this module exists
//!
//! The overlap integral S_μν = ⟨χ_μ|χ_ν⟩ is one of the most fundamental quantities
//! in quantum chemistry. It appears in:
//!
//! - **The Roothaan-Hall equations**: F C = S C ε, where S is the overlap matrix.
//! - **Orthonormalization**: The canonical molecular orbitals satisfy C^T S C = I.
//! - **Population analysis**: Mulliken and Löwdin charges require the overlap matrix.
//! - **Integral screening**: The Schwarz inequality |(μν|λσ)| ≤ √(μμ|νν)·√(λλ|σσ)
//!   uses diagonal overlap-liked quantities.
//!
//! The ss overlap is also the base case for ALL other one-electron integrals
//! (kinetic, nuclear attraction, dipole) and two-electron integrals (ERI), since
//! higher angular momentum integrals are built from s-type integrals through
//! recurrence relations (Obara-Saika, McMurchie-Davidson).
//!
//! # How it works
//!
//! The overlap between two s-type primitive Gaussians centered at A and B with
//! exponents α and β separated by squared distance R_AB² is:
//!
//! ```text
//! S_ss = (π / (α + β))^(3/2) · exp(−α·β·R_AB² / (α + β))
//! ```
//!
//! This formula is derived by:
//! 1. Forming the Gaussian product: two Gaussians multiplied together yield a
//!    new Gaussian centered at the product center P = (α·A + β·B)/(α+β).
//! 2. The prefactor (π/(α+β))^(3/2) comes from integrating exp(−p·r²) over all
//!    space, where p = α + β is the combined exponent.
//! 3. The exponential factor accounts for the displacement between the two
//!    centers — as the centers move apart, the overlap decreases.

/// Compute the overlap integral between two s-type primitive Gaussians.
///
/// # What it does
///
/// Evaluates ⟨s_A|s_B⟩ where both s-type functions are unnormalized primitive
/// Gaussians (just exp(−α|r−A|²) and exp(−β|r−B|²)). The normalization is
/// assumed to be handled separately by the caller (via
/// [`normalize_shell`](crate::normalization::normalize_shell)).
///
/// # Why this is the base case
///
/// All overlap integrals for higher angular momenta can be computed from this
/// result using the Obara-Saika recurrence:
///
/// ```text
/// S_{i+1,j} = (P_i − A_i) · S_{i,j} + (i/(2p)) · S_{i−1,j} + (j/(2p)) · S_{i,j−1}
/// ```
///
/// where i, j are angular momentum indices, P is the product center, and
/// p = α + β. Starting from S_{0,0} = S_ss, one can build any (l_a, l_b) pair.
///
/// # How it works
///
/// The closed-form formula is:
///
/// ```text
/// S_ss(α, β, R_AB²) = (π / (α + β))^(3/2) · exp(−α·β·R_AB² / (α + β))
/// ```
///
/// - `p = α + β` is the combined exponent (Gaussian product theorem).
/// - `(π/p)^(3/2)` is the integral of the product Gaussian over all space.
/// - `exp(−α·β·R_AB² / p)` is the displacement factor that reduces overlap
///   when centers are separated.
///
/// # Arguments
///
/// * `alpha_a` — Gaussian exponent on center A (must be positive)
/// * `alpha_b` — Gaussian exponent on center B (must be positive)
/// * `rab2` — Squared distance |A − B|² between the two centers (must be non-negative)
///
/// # Returns
///
/// The overlap integral value. Always positive for valid inputs.
///
/// # Examples
///
/// ```
/// use poler_eri::overlap::overlap_ss;
///
/// // Two s-type Gaussians on the same center (R=0): maximum overlap
/// let s_same = overlap_ss(1.0, 1.0, 0.0);
/// // S_ss = (π/2)^(3/2) ≈ 3.9269
/// assert!(s_same > 0.0);
///
/// // As centers move apart, overlap decreases
/// let s_far = overlap_ss(1.0, 1.0, 10.0);
/// assert!(s_far < s_same);
/// ```
pub fn overlap_ss(alpha_a: f64, alpha_b: f64, rab2: f64) -> f64 {
    use std::f64::consts::PI;

    let p = alpha_a + alpha_b;
    let prefactor = libm::pow(PI / p, 1.5);        // (π / p)^(3/2)
    let exponent = -alpha_a * alpha_b * rab2 / p;   // −α·β·R² / p
    prefactor * libm::exp(exponent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_overlap_ss_same_center() {
        // Two s-Gaussians on the same center (rab2 = 0):
        // S_ss = (π / (α + β))^(3/2)
        let val = overlap_ss(1.0, 1.0, 0.0);
        let expected = libm::pow(PI / 2.0, 1.5);
        assert!((val - expected).abs() < 1e-12,
            "Same-center overlap: expected {}, got {}", expected, val);
    }

    #[test]
    fn test_overlap_ss_different_exponents() {
        // Different exponents on the same center
        let val = overlap_ss(2.0, 3.0, 0.0);
        let expected = libm::pow(PI / 5.0, 1.5);
        assert!((val - expected).abs() < 1e-12,
            "Different exponents: expected {}, got {}", expected, val);
    }

    #[test]
    fn test_overlap_ss_separated_centers() {
        // Separated centers: S_ss = (π/p)^(3/2) * exp(-α·β·R²/p)
        let alpha_a = 1.0;
        let alpha_b = 1.0;
        let rab2 = 1.0;
        let p = alpha_a + alpha_b;
        let expected = libm::pow(PI / p, 1.5) * libm::exp(-alpha_a * alpha_b * rab2 / p);
        let val = overlap_ss(alpha_a, alpha_b, rab2);
        assert!((val - expected).abs() < 1e-12);
    }

    #[test]
    fn test_overlap_ss_decreases_with_distance() {
        let s0 = overlap_ss(1.0, 1.0, 0.0);
        let s1 = overlap_ss(1.0, 1.0, 1.0);
        let s5 = overlap_ss(1.0, 1.0, 5.0);
        let s10 = overlap_ss(1.0, 1.0, 10.0);
        assert!(s0 > s1, "Overlap should decrease with distance");
        assert!(s1 > s5, "Overlap should decrease with distance");
        assert!(s5 > s10, "Overlap should decrease with distance");
    }

    #[test]
    fn test_overlap_ss_non_negative() {
        // Overlap should always be non-negative. For very large exponents
        // and distances, the value can underflow to zero, which is acceptable.
        for &alpha_a in &[0.5, 1.0, 5.0, 100.0] {
            for &alpha_b in &[0.5, 1.0, 5.0, 100.0] {
                for &rab2 in &[0.0, 1.0, 10.0, 100.0] {
                    let val = overlap_ss(alpha_a, alpha_b, rab2);
                    assert!(val >= 0.0,
                        "Overlap should be non-negative for α_a={}, α_b={}, R²={}", alpha_a, alpha_b, rab2);
                }
            }
        }
    }

    #[test]
    fn test_overlap_ss_positive_for_moderate_params() {
        // For moderate exponents and distances, overlap should be strictly positive
        for &alpha_a in &[0.5, 1.0, 5.0] {
            for &alpha_b in &[0.5, 1.0, 5.0] {
                for &rab2 in &[0.0, 1.0, 5.0] {
                    let val = overlap_ss(alpha_a, alpha_b, rab2);
                    assert!(val > 0.0,
                        "Overlap should be positive for α_a={}, α_b={}, R²={}", alpha_a, alpha_b, rab2);
                }
            }
        }
    }

    #[test]
    fn test_overlap_ss_symmetry() {
        // S_ss(α, β, R²) = S_ss(β, α, R²) by symmetry
        let val_ab = overlap_ss(2.0, 3.0, 1.5);
        let val_ba = overlap_ss(3.0, 2.0, 1.5);
        assert!((val_ab - val_ba).abs() < 1e-14,
            "Overlap should be symmetric: S(ab)={}, S(ba)={}", val_ab, val_ba);
    }

    #[test]
    fn test_overlap_ss_unit_exponents_known_value() {
        // α = β = 1, R² = 0: S = (π/2)^(3/2)
        let val = overlap_ss(1.0, 1.0, 0.0);
        // (π/2)^1.5 ≈ 1.9687
        let expected = libm::pow(PI / 2.0, 1.5);
        assert!((val - expected).abs() < 1e-10,
            "Known value check: expected {}, got {}", expected, val);
    }
}
