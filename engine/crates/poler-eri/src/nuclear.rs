//! Nuclear attraction integral evaluation for Gaussian basis functions.
//!
//! # What this module does
//!
//! This module computes nuclear attraction integrals between Gaussian-type
//! orbitals (GTOs) and a point nucleus. Currently, it provides the base case
//! for two s-type (l=0) primitive Gaussians interacting with a nuclear charge,
//! which is the fundamental building block for all nuclear attraction integrals
//! via recurrence relations.
//!
//! # Why this module exists
//!
//! The nuclear attraction integral V_μν = ⟨χ_μ| −Z_C/|r−C| |χ_ν⟩ is a core
//! component of the one-electron Hamiltonian Ĥ = T̂ + V̂_nuc. It appears in:
//!
//! - **Core Hamiltonian**: The nuclear attraction matrix is summed over all
//!   nuclei to form V_nuc, which is added to the kinetic energy matrix to
//!   give H_core = T + V_nuc.
//! - **Energy evaluation**: The total electronic energy E = tr(P·(H_core + F))/2
//!   requires nuclear attraction integrals for every basis function pair and
//!   every nucleus.
//! - **Geometry optimization**: Nuclear attraction integrals change with
//!   molecular geometry, contributing to energy gradients.
//!
//! The ss nuclear attraction integral is also the base case for building higher
//! angular momentum nuclear attraction integrals through recurrence relations
//! (McMurchie-Davidson, Obara-Saika).
//!
//! # How it works
//!
//! The nuclear attraction integral between two s-type primitive Gaussians
//! centered at A and B with exponents α and β, and a point nucleus at C
//! with charge Z_C, is:
//!
//! ```text
//! V_ss = −Z_C · 2π / (α + β) · F_0((α+β)·R_PC²) · exp(−α·β·R_AB² / (α+β))
//! ```
//!
//! where:
//! - P is the Gaussian product center: P = (α·A + β·B) / (α + β)
//! - R_PC² = |P − C|² is the squared distance from the product center to the nucleus
//! - F_0(T) is the Boys function of order 0
//!
//! The Boys function F_0(T) = ∫₀¹ exp(−T·u²) du = √(π/T)·erf(√T)/2
//! smoothly interpolates between F_0(0) = 1 (nucleus at the product center)
//! and F_0(∞) = 0 (nucleus far from the product center).

use crate::boys::boys_f;

/// Compute the nuclear attraction integral between two s-type primitive Gaussians
/// and a point nucleus.
///
/// # What it does
///
/// Evaluates ⟨s_A| −Z_C/|r−C| |s_B⟩ where both s-type functions are unnormalized
/// primitive Gaussians centered at A and B, and the nucleus with charge Z_C is
/// located at point C. The function returns the full integral value including the
/// −Z_C factor.
///
/// # Why this is the base case
///
/// All nuclear attraction integrals for higher angular momenta can be computed
/// from this result using recurrence relations. In the McMurchie-Davidson scheme,
/// the ss integral with the Boys function serves as the seed for building the
/// Hermite expansion coefficients. In the Obara-Saika scheme, the recurrence is:
///
/// ```text
/// V_{i+1,j} = (P_i − A_i) · V_{i,j} + (i/(2p)) · V_{i−1,j} + (j/(2p)) · V_{i,j−1}
///             − (1/(2p)) · R_PC_i · V_aux_{i,j}
/// ```
///
/// Starting from V_{0,0} = V_ss, one can build any (l_a, l_b) pair.
///
/// # How it works
///
/// The closed-form formula is:
///
/// ```text
/// V_ss = −Z_C · 2π / p · F_0(p · R_PC²) · exp(−α_a · α_b · R_AB² / p)
/// ```
///
/// where p = α_a + α_b is the combined exponent.
///
/// Derivation sketch:
/// 1. Form the Gaussian product on center P (Gaussian product theorem).
/// 2. The Coulomb potential 1/|r−C| is expanded using the Laplace transform
///    of the Coulomb operator, which introduces the Boys function.
/// 3. For s-type Gaussians, only F_0 is needed (no angular momentum on the
///    Coulomb expansion).
/// 4. The exponential factor accounts for the displacement between centers A and B.
///
/// # Arguments
///
/// * `alpha_a` — Gaussian exponent on center A (must be positive)
/// * `alpha_b` — Gaussian exponent on center B (must be positive)
/// * `rab2` — Squared distance |A − B|² between the two Gaussian centers
/// * `rpc2` — Squared distance |P − C|² from the product center P to the nucleus C
/// * `zc` — Nuclear charge of the attracting nucleus (e.g., 1.0 for H, 6.0 for C)
///
/// # Returns
///
/// The nuclear attraction integral ⟨s_A| −Z_C/|r−C| |s_B⟩.
/// Negative for attractive (normal) interactions, positive for Z_C < 0 (unphysical).
///
/// # Examples
///
/// ```
/// use poler_eri::nuclear::nuclear_ss;
///
/// // Nuclear attraction when product center coincides with nucleus (rpc2 = 0)
/// let v_at = nuclear_ss(1.0, 1.0, 0.0, 0.0, 1.0);
/// // Should be negative (attractive)
/// assert!(v_at < 0.0);
///
/// // As nucleus moves away, |V| decreases
/// let v_far = nuclear_ss(1.0, 1.0, 0.0, 10.0, 1.0);
/// assert!(v_far.abs() < v_at.abs());
/// ```
pub fn nuclear_ss(alpha_a: f64, alpha_b: f64, rab2: f64, rpc2: f64, zc: f64) -> f64 {
    use std::f64::consts::PI;

    let p = alpha_a + alpha_b;

    // Prefactor: -Z_C * 2π / p
    let prefactor = -zc * 2.0 * PI / p;

    // Boys function F_0(p * R_PC²)
    let boys_arg = p * rpc2;
    let f0 = boys_f(0, boys_arg);

    // Displacement exponential: exp(-α_a * α_b * R_AB² / p)
    let exp_factor = libm::exp(-alpha_a * alpha_b * rab2 / p);

    prefactor * f0 * exp_factor
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_nuclear_ss_attractive() {
        // Nuclear attraction should be negative (attractive) for positive Z_C
        let val = nuclear_ss(1.0, 1.0, 0.0, 0.0, 1.0);
        assert!(val < 0.0, "Nuclear attraction should be negative, got {}", val);
    }

    #[test]
    fn test_nuclear_ss_at_nucleus() {
        // When rpc2 = 0 (product center at nucleus): F_0(0) = 1
        // V_ss = -Z_C * 2π/p * 1 * exp(0) = -Z_C * 2π/p
        let alpha_a = 1.0;
        let alpha_b = 1.0;
        let p = alpha_a + alpha_b;
        let expected = -1.0 * 2.0 * PI / p;
        let val = nuclear_ss(alpha_a, alpha_b, 0.0, 0.0, 1.0);
        assert!((val - expected).abs() < 1e-12,
            "Nuclear at nucleus: expected {}, got {}", expected, val);
    }

    #[test]
    fn test_nuclear_ss_decreases_with_distance() {
        // As the nucleus moves away from the product center, |V| decreases
        let v0 = nuclear_ss(1.0, 1.0, 0.0, 0.0, 1.0);
        let v1 = nuclear_ss(1.0, 1.0, 0.0, 1.0, 1.0);
        let v5 = nuclear_ss(1.0, 1.0, 0.0, 5.0, 1.0);
        let v10 = nuclear_ss(1.0, 1.0, 0.0, 10.0, 1.0);
        assert!(v0.abs() > v1.abs(), "|V| should decrease with distance");
        assert!(v1.abs() > v5.abs(), "|V| should decrease with distance");
        assert!(v5.abs() > v10.abs(), "|V| should decrease with distance");
    }

    #[test]
    fn test_nuclear_ss_scales_with_charge() {
        // V_ss should scale linearly with Z_C
        let v1 = nuclear_ss(1.0, 1.0, 0.0, 1.0, 1.0);
        let v6 = nuclear_ss(1.0, 1.0, 0.0, 1.0, 6.0);
        assert!((v6 - 6.0 * v1).abs() < 1e-12,
            "V should scale linearly with Z_C: v6={}, 6*v1={}", v6, 6.0 * v1);
    }

    #[test]
    fn test_nuclear_ss_symmetry() {
        // V_ss(α, β, R_AB², R_PC², Z) = V_ss(β, α, R_AB², R_PC², Z)
        // Note: R_PC² depends on P = (α·A + β·B)/p, so symmetry holds when
        // rpc2 is provided as a parameter that already accounts for P.
        let val_ab = nuclear_ss(2.0, 3.0, 1.0, 0.5, 1.0);
        let val_ba = nuclear_ss(3.0, 2.0, 1.0, 0.5, 1.0);
        assert!((val_ab - val_ba).abs() < 1e-14,
            "Nuclear integral should be symmetric in exponents (given same rpc2)");
    }

    #[test]
    fn test_nuclear_ss_boys_function_connection() {
        // Verify the formula: V_ss = -Z_C * 2π/p * F_0(p*rpc2) * exp(-α_a*α_b*rab2/p)
        let alpha_a = 2.0;
        let alpha_b = 3.0;
        let rab2 = 1.5;
        let rpc2 = 0.8;
        let zc = 5.0;
        let p = alpha_a + alpha_b;

        let expected = -zc * 2.0 * PI / p
            * boys_f(0, p * rpc2)
            * libm::exp(-alpha_a * alpha_b * rab2 / p);
        let val = nuclear_ss(alpha_a, alpha_b, rab2, rpc2, zc);
        assert!((val - expected).abs() < 1e-12,
            "Formula verification: expected {}, got {}", expected, val);
    }

    #[test]
    fn test_nuclear_ss_displacement_reduces_attraction() {
        // Separating the Gaussian centers (increasing rab2) reduces |V|
        // because exp(-α_a*α_b*rab2/p) decreases
        let v_same = nuclear_ss(1.0, 1.0, 0.0, 1.0, 1.0);
        let v_sep = nuclear_ss(1.0, 1.0, 5.0, 1.0, 1.0);
        assert!(v_same.abs() > v_sep.abs(),
            "Increasing rab2 should decrease |V|: same={}, sep={}", v_same, v_sep);
    }

    #[test]
    fn test_nuclear_ss_h_atom_like() {
        // For a hydrogen atom (Z=1) with a 1s Gaussian:
        // α_a = α_b = α, rab2 = 0, nucleus at center → rpc2 = 0
        // V_ss = -1 * 2π/(2α) * F_0(0) * 1 = -π/α
        let alpha = 1.0;
        let val = nuclear_ss(alpha, alpha, 0.0, 0.0, 1.0);
        let expected = -PI / alpha;
        assert!((val - expected).abs() < 1e-12,
            "H-atom like: expected {}, got {}", expected, val);
    }
}
