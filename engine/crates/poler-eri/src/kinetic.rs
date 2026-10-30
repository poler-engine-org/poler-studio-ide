//! Kinetic energy integral evaluation for Gaussian basis functions.
//!
//! # What this module does
//!
//! This module computes kinetic energy integrals between Gaussian-type orbitals
//! (GTOs). Currently, it provides the base case for two s-type (l=0) primitive
//! Gaussians, which is the fundamental building block for all kinetic energy
//! integrals via recurrence relations.
//!
//! # Why this module exists
//!
//! The kinetic energy integral T_μν = ⟨χ_μ| −½∇² |χ_ν⟩ is a core component of
//! the one-electron Hamiltonian Ĥ = T̂ + V̂_nuc. It appears in:
//!
//! - **Fock matrix construction**: The Fock matrix F = H_core + G, where H_core
//!   includes the kinetic energy contribution.
//! - **Energy evaluation**: The total electronic energy E = tr(P·(H_core + F))/2
//!   requires kinetic energy integrals.
//! - **Molecular orbital analysis**: Kinetic energy contributions to orbital
//!   energies reveal bonding character.
//!
//! The ss kinetic integral is also the base case for building higher angular
//! momentum kinetic integrals through recurrence relations (Obara-Saika), just
//! as the ss overlap is the base case for overlap integrals.
//!
//! # How it works
//!
//! The kinetic energy integral between two s-type primitive Gaussians is derived
//! from the overlap integral using the identity:
//!
//! ```text
//! T_ss = α_a · α_b / (α_a + α_b) · (3 − 2·α_a·α_b·R_AB² / (α_a + α_b)) · S_ss
//! ```
//!
//! This formula comes from applying the Laplacian operator to the Gaussian
//! product. For an s-type Gaussian φ = exp(−α|r−A|²), the Laplacian gives:
//!
//! ```text
//! ∇² exp(−α r²) = (4α²r² − 6α) exp(−α r²)
//! ```
//!
//! After forming the Gaussian product and integrating, the result factorizes
//! into the overlap S_ss multiplied by a polynomial in the combined exponent
//! and inter-center distance.

use crate::overlap::overlap_ss;

/// Compute the kinetic energy integral between two s-type primitive Gaussians.
///
/// # What it does
///
/// Evaluates ⟨s_A| −½∇² |s_B⟩ where both s-type functions are unnormalized
/// primitive Gaussians. Note: this function returns the full kinetic integral
/// value including the −½ factor from the kinetic energy operator.
///
/// # Why this is the base case
///
/// All kinetic energy integrals for higher angular momenta can be computed from
/// the s-type overlap integral and this ss kinetic result using the Obara-Saika
/// recurrence:
///
/// ```text
/// T_{i+1,j} = (P_i − A_i) · T_{i,j} + (i/(2p)) · T_{i−1,j} + (j/(2p)) · T_{i,j−1}
///             + (1/(2p)) · (i · S_{i−1,j} + j · S_{i,j−1})
/// ```
///
/// where S denotes overlap integrals and T denotes kinetic integrals.
///
/// # How it works
///
/// The closed-form formula is:
///
/// ```text
/// T_ss = α_a · α_b / (α_a + α_b) · (3 − 2·α_a·α_b·R_AB² / (α_a + α_b)) · S_ss
/// ```
///
/// where S_ss is the overlap integral computed by [`overlap_ss`].
///
/// Derivation sketch:
/// 1. Apply −½∇² to the Gaussian on center B.
/// 2. The Laplacian of exp(−α_b|r−B|²) yields (4α_b²|r−B|² − 6α_b)·exp(−α_b|r−B|²).
/// 3. Multiply by the Gaussian on center A and integrate.
/// 4. Use the Gaussian product theorem to shift coordinates to the product center P.
/// 5. The radial integral and displacement factors combine to give the formula above.
///
/// # Arguments
///
/// * `alpha_a` — Gaussian exponent on center A (must be positive)
/// * `alpha_b` — Gaussian exponent on center B (must be positive)
/// * `rab2` — Squared distance |A − B|² between the two centers (must be non-negative)
///
/// # Returns
///
/// The kinetic energy integral ⟨s_A| −½∇² |s_B⟩. Can be negative when the overlap
/// of kinetic contributions is destructive (e.g., at large separations).
///
/// # Examples
///
/// ```
/// use poler_eri::kinetic::kinetic_ss;
///
/// // Same-center kinetic integral
/// let t_same = kinetic_ss(1.0, 1.0, 0.0);
/// // Should be positive (constructive overlap of kinetic energy)
/// assert!(t_same > 0.0);
///
/// // Kinetic integral decreases as centers separate
/// let t_far = kinetic_ss(1.0, 1.0, 10.0);
/// assert!(t_far.abs() < t_same.abs());
/// ```
pub fn kinetic_ss(alpha_a: f64, alpha_b: f64, rab2: f64) -> f64 {
    let p = alpha_a + alpha_b;
    let mu = alpha_a * alpha_b / p;  // Reduced exponent

    // Compute the overlap integral S_ss first
    let s_ss = overlap_ss(alpha_a, alpha_b, rab2);

    // Kinetic energy factor: μ · (3 − 2·μ·R²)
    // where μ = α_a·α_b / (α_a + α_b) is the reduced exponent
    let factor = mu * (3.0 - 2.0 * mu * rab2);

    factor * s_ss
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kinetic_ss_same_center() {
        // When rab2 = 0: T_ss = α_a·α_b/(α_a+α_b) · 3 · S_ss
        // For α_a = α_b = 1: T_ss = 0.5 · 3 · (π/2)^(3/2) = 1.5 · (π/2)^(3/2)
        let val = kinetic_ss(1.0, 1.0, 0.0);
        let s_ss = overlap_ss(1.0, 1.0, 0.0);
        let expected = 1.0 * 1.0 / 2.0 * 3.0 * s_ss;
        assert!((val - expected).abs() < 1e-12,
            "Same-center kinetic: expected {}, got {}", expected, val);
    }

    #[test]
    fn test_kinetic_ss_positive_at_origin() {
        // At zero separation, kinetic energy should be positive
        let val = kinetic_ss(1.0, 1.0, 0.0);
        assert!(val > 0.0, "Kinetic energy should be positive at zero separation");
    }

    #[test]
    fn test_kinetic_ss_symmetry() {
        // T_ss(α, β, R²) = T_ss(β, α, R²) by symmetry
        let val_ab = kinetic_ss(2.0, 3.0, 1.5);
        let val_ba = kinetic_ss(3.0, 2.0, 1.5);
        assert!((val_ab - val_ba).abs() < 1e-14,
            "Kinetic integral should be symmetric: T(ab)={}, T(ba)={}", val_ab, val_ba);
    }

    #[test]
    fn test_kinetic_ss_decreases_with_distance() {
        // Kinetic energy magnitude generally decreases with distance
        let t0 = kinetic_ss(1.0, 1.0, 0.0);
        let t1 = kinetic_ss(1.0, 1.0, 1.0);
        let t5 = kinetic_ss(1.0, 1.0, 5.0);
        let t10 = kinetic_ss(1.0, 1.0, 10.0);
        assert!(t0 > t1, "Kinetic should decrease with distance");
        assert!(t1.abs() > t5.abs(), "Kinetic magnitude should decrease with distance");
        assert!(t5.abs() > t10.abs(), "Kinetic magnitude should decrease with distance");
    }

    #[test]
    fn test_kinetic_ss_can_be_negative() {
        // At large separation, the (3 - 2*mu*rab2) factor can become negative
        // For α_a = α_b = 0.5, p = 1, mu = 0.25
        // 3 - 2*0.25*R² < 0 when R² > 6
        let val = kinetic_ss(0.5, 0.5, 10.0);
        assert!(val < 0.0, "Kinetic can be negative at large separation");
    }

    #[test]
    fn test_kinetic_ss_relation_to_overlap() {
        // At the same center, T_ss = (3/2) * (α_a*α_b/p) * S_ss ... actually
        // T_ss = mu * 3 * S_ss at rab2=0
        // And mu = alpha_a * alpha_b / p
        let alpha_a = 2.0;
        let alpha_b = 3.0;
        let p = alpha_a + alpha_b;
        let mu = alpha_a * alpha_b / p;

        let t_val = kinetic_ss(alpha_a, alpha_b, 0.0);
        let s_val = overlap_ss(alpha_a, alpha_b, 0.0);

        let expected = mu * 3.0 * s_val;
        assert!((t_val - expected).abs() < 1e-12,
            "T_ss = 3*μ*S_ss at origin: expected {}, got {}", expected, t_val);
    }

    #[test]
    fn test_kinetic_ss_h_atom_like() {
        // For a hydrogen-like 1s orbital: T = α_a·α_b/(α_a+α_b) * 3 * S_ss
        // With STO-3G first primitive for H: α ≈ 3.425
        let alpha = 3.4252509100;
        let t = kinetic_ss(alpha, alpha, 0.0);
        let s = overlap_ss(alpha, alpha, 0.0);
        // mu = α²/(2α) = α/2 = 1.7126
        // T = 1.7126 * 3 * S
        let mu = alpha / 2.0;
        let expected = mu * 3.0 * s;
        assert!((t - expected).abs() < 1e-10);
    }
}
