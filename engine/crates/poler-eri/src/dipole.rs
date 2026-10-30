//! Dipole moment integrals for Gaussian basis functions.
//!
//! # What this module does
//!
//! This module computes electric dipole moment integrals between Gaussian-type
//! orbitals (GTOs). The dipole integral measures the contribution of a pair of
//! basis functions to the molecular electric dipole moment.
//!
//! # Why this module exists
//!
//! The electric dipole moment is one of the most important molecular properties:
//!
//! - **IR intensities**: Dipole moment derivatives determine infrared absorption strengths.
//! - **Molecular polarity**: The permanent dipole moment indicates charge separation.
//! - **Response properties**: Dipole polarizabilities require dipole integrals as input.
//! - **Transition moments**: Oscillator strengths for electronic excitations use dipole integrals.
//!
//! The dipole integral for a basis function pair (μ, ν) in direction i is:
//!
//! ```text
//! μ_i = ⟨χ_μ | r_i | χ_ν⟩
//! ```
//!
//! where `r_i` is the i-th Cartesian coordinate (x, y, or z).
//!
//! # How it works
//!
//! For s-type primitive Gaussians, the dipole integral has a simple closed form.
//! Using the Gaussian product theorem, two s-type Gaussians centered at A and B
//! with exponents α and β produce a product Gaussian centered at the product
//! center P = (α·A + β·B) / (α + β). The dipole integral is then:
//!
//! ```text
//! ⟨s_A | r_i | s_B⟩ = P_i · S_ss(α, β, R_AB²)
//! ```
//!
//! where `P_i` is the i-th coordinate of the product center and `S_ss` is the
//! overlap integral. This formula reflects the fact that the charge centroid
//! of the product Gaussian lies at P.

use crate::types::Point;
use crate::overlap::overlap_ss;

/// Compute the dipole moment integral for two s-type primitive Gaussians.
///
/// # What it does
///
/// Evaluates `⟨s_A | r_dir | s_B⟩` — the dipole integral in the specified
/// Cartesian direction between two unnormalized s-type primitive Gaussians.
///
/// # Why it exists
///
/// The ss dipole integral is the base case for all higher-angular-momentum
/// dipole integrals. Just as `overlap_ss` is the seed for the overlap recurrence,
/// `dipole_ss` is the seed for the dipole recurrence relations:
///
/// ```text
/// ⟨a+1_i | r_j | b⟩ = P_j · ⟨a+1_i | b⟩ + ...  (recurrence terms)
/// ```
///
/// # How it works
///
/// The product Gaussian from two s-type primitives centered at A and B has its
/// center at:
///
/// ```text
/// P = (α·A + β·B) / (α + β)
/// ```
///
/// The dipole integral equals the product center coordinate times the overlap:
///
/// ```text
/// ⟨s_A | r_dir | s_B⟩ = P_dir · S_ss(α, β, R_AB²)
/// ```
///
/// where:
/// - `P_dir` is the `direction`-th coordinate of the product center P
/// - `S_ss` is the s-s overlap integral
/// - `direction` selects x (0), y (1), or z (2)
///
/// # Arguments
///
/// * `alpha_a` — Gaussian exponent on center A (must be positive)
/// * `alpha_b` — Gaussian exponent on center B (must be positive)
/// * `rab2` — Squared distance |A − B|² (must be non-negative)
/// * `ra` — Coordinates of center A
/// * `rb` — Coordinates of center B
/// * `direction` — Cartesian direction index: 0 = x, 1 = y, 2 = z
///
/// # Returns
///
/// The dipole integral value `⟨s_A | r_dir | s_B⟩`.
///
/// # Panics
///
/// Panics if `direction > 2`.
///
/// # Examples
///
/// ```
/// use poler_eri::dipole::dipole_ss;
/// use poler_eri::types::Point;
///
/// // Same center, α=β=1: P = A = B, so dipole = A_dir * S_ss
/// let ra = Point([1.0, 0.0, 0.0]);
/// let rb = Point([1.0, 0.0, 0.0]);
/// let val = dipole_ss(1.0, 1.0, 0.0, &ra, &rb, 0);
/// // S_ss(1,1,0) = (π/2)^(3/2), P_x = 1.0
/// assert!(val > 0.0);
///
/// // Origin: both centers at origin → dipole should be zero
/// let origin = Point([0.0, 0.0, 0.0]);
/// let val_zero = dipole_ss(1.0, 1.0, 0.0, &origin, &origin, 0);
/// assert!(val_zero.abs() < 1e-14);
/// ```
pub fn dipole_ss(
    alpha_a: f64,
    alpha_b: f64,
    rab2: f64,
    ra: &Point,
    rb: &Point,
    direction: usize,
) -> f64 {
    assert!(direction <= 2, "dipole_ss: direction must be 0, 1, or 2, got {}", direction);

    let p = alpha_a + alpha_b;

    // Product center P = (α·A + β·B) / (α + β)
    let p_coord = (alpha_a * ra.0[direction] + alpha_b * rb.0[direction]) / p;

    // Dipole = P_dir * S_ss
    let overlap = overlap_ss(alpha_a, alpha_b, rab2);
    p_coord * overlap
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_dipole_ss_same_center_x() {
        // Two s-Gaussians on the same center at (2, 0, 0), direction = x
        let ra = Point([2.0, 0.0, 0.0]);
        let rb = Point([2.0, 0.0, 0.0]);
        let val = dipole_ss(1.0, 1.0, 0.0, &ra, &rb, 0);

        // P_x = (1*2 + 1*2) / 2 = 2.0
        // S_ss = (π/2)^(3/2)
        let s_ss = libm::pow(PI / 2.0, 1.5);
        let expected = 2.0 * s_ss;
        assert!((val - expected).abs() < 1e-12,
            "Same-center x-dipole: expected {}, got {}", expected, val);
    }

    #[test]
    fn test_dipole_ss_origin() {
        // Both centers at origin: P = origin, so dipole = 0
        let origin = Point([0.0, 0.0, 0.0]);
        for dir in 0..3 {
            let val = dipole_ss(1.0, 1.0, 0.0, &origin, &origin, dir);
            assert!(val.abs() < 1e-14,
                "Dipole at origin in direction {} should be zero, got {}", dir, val);
        }
    }

    #[test]
    fn test_dipole_ss_separated_centers() {
        // Center A at (0, 0, 0), center B at (1, 0, 0), α_a = α_b = 1
        let ra = Point([0.0, 0.0, 0.0]);
        let rb = Point([1.0, 0.0, 0.0]);
        let rab2 = ra.dist2(&rb); // = 1.0

        let val_x = dipole_ss(1.0, 1.0, rab2, &ra, &rb, 0);

        // P_x = (1*0 + 1*1) / 2 = 0.5
        // S_ss = (π/2)^(3/2) * exp(-1*1*1/2) = (π/2)^(3/2) * exp(-0.5)
        let s_ss = libm::pow(PI / 2.0, 1.5) * libm::exp(-0.5);
        let expected = 0.5 * s_ss;
        assert!((val_x - expected).abs() < 1e-12,
            "Separated x-dipole: expected {}, got {}", expected, val_x);

        // y and z should give zero (P_y = P_z = 0)
        let val_y = dipole_ss(1.0, 1.0, rab2, &ra, &rb, 1);
        let val_z = dipole_ss(1.0, 1.0, rab2, &ra, &rb, 2);
        assert!(val_y.abs() < 1e-14, "y-dipole should be zero");
        assert!(val_z.abs() < 1e-14, "z-dipole should be zero");
    }

    #[test]
    fn test_dipole_ss_different_exponents() {
        // Asymmetric exponents shift the product center toward the tighter Gaussian
        let ra = Point([0.0, 0.0, 0.0]);
        let rb = Point([2.0, 0.0, 0.0]);

        // Large alpha_a → P closer to A
        let val_tight_a = dipole_ss(10.0, 1.0, 4.0, &ra, &rb, 0);
        // P_x = (10*0 + 1*2) / 11 = 2/11 ≈ 0.182

        // Large alpha_b → P closer to B
        let val_tight_b = dipole_ss(1.0, 10.0, 4.0, &ra, &rb, 0);
        // P_x = (1*0 + 10*2) / 11 = 20/11 ≈ 1.818

        // The tight_b case should have a larger dipole (P is further from origin)
        assert!(val_tight_b.abs() > val_tight_a.abs(),
            "Product center should shift toward tighter Gaussian");
    }

    #[test]
    fn test_dipole_ss_symmetry() {
        // dipole_ss should NOT be symmetric in general (the product center depends
        // on both exponents), but for equal exponents and symmetric positions it is.
        let ra = Point([0.0, 0.0, 0.0]);
        let rb = Point([1.0, 0.0, 0.0]);
        let rab2 = 1.0;

        // Same exponents: ⟨s_A | x | s_B⟩ should equal ⟨s_B | x | s_A⟩
        // because the product center is symmetric
        let val_ab = dipole_ss(2.0, 2.0, rab2, &ra, &rb, 0);
        let val_ba = dipole_ss(2.0, 2.0, rab2, &rb, &ra, 0);
        assert!((val_ab - val_ba).abs() < 1e-14,
            "Dipole should be symmetric for equal exponents: {} vs {}", val_ab, val_ba);
    }

    #[test]
    fn test_dipole_ss_decreases_at_large_distance() {
        // For fixed exponents, the dipole integral eventually decreases as
        // centers separate far enough (the overlap factor decays faster than
        // the product center shifts). Starting from moderate distance.
        let ra = Point([0.0, 0.0, 0.0]);

        let mut prev = f64::INFINITY;
        for dist in [2.0, 3.0, 5.0, 10.0, 20.0].iter() {
            let rb = Point([*dist, 0.0, 0.0]);
            let rab2 = dist * dist;
            let val = dipole_ss(1.0, 1.0, rab2, &ra, &rb, 0);
            assert!(val.abs() < prev,
                "Dipole magnitude should decrease: dist={}, val={}, prev={}",
                dist, val, prev);
            prev = val.abs();
        }
    }

    #[test]
    fn test_dipole_ss_positive_near_center() {
        // When the product center is in the positive direction, dipole should be positive
        let ra = Point([0.0, 0.0, 0.0]);
        let rb = Point([1.0, 0.0, 0.0]);
        let val = dipole_ss(1.0, 1.0, 1.0, &ra, &rb, 0);
        assert!(val > 0.0, "Dipole should be positive when product center is positive");
    }

    #[test]
    fn test_dipole_ss_negative_direction() {
        // Product center in negative direction should give negative dipole
        let ra = Point([-1.0, 0.0, 0.0]);
        let rb = Point([-1.0, 0.0, 0.0]);
        let val = dipole_ss(1.0, 1.0, 0.0, &ra, &rb, 0);
        assert!(val < 0.0, "Dipole should be negative for negative product center");
    }

    #[test]
    fn test_dipole_ss_y_and_z_directions() {
        let ra = Point([0.0, 3.0, 0.0]);
        let rb = Point([0.0, 3.0, 0.0]);

        let val_x = dipole_ss(1.0, 1.0, 0.0, &ra, &rb, 0);
        let val_y = dipole_ss(1.0, 1.0, 0.0, &ra, &rb, 1);
        let val_z = dipole_ss(1.0, 1.0, 0.0, &ra, &rb, 2);

        let s_ss = libm::pow(PI / 2.0, 1.5);
        assert!(val_x.abs() < 1e-14, "x-dipole should be zero");
        assert!((val_y - 3.0 * s_ss).abs() < 1e-12, "y-dipole should be 3*S_ss");
        assert!(val_z.abs() < 1e-14, "z-dipole should be zero");
    }

    #[test]
    #[should_panic]
    fn test_dipole_ss_invalid_direction() {
        let ra = Point([0.0, 0.0, 0.0]);
        let rb = Point([0.0, 0.0, 0.0]);
        dipole_ss(1.0, 1.0, 0.0, &ra, &rb, 3);
    }
}
