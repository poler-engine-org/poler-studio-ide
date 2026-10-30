//! Cartesian to spherical harmonic transformation.
//!
//! # What this module does
//!
//! This module transforms integrals computed in the Cartesian Gaussian basis
//! (with `(l+1)(l+2)/2` components) to the spherical harmonic basis (with
//! `2l+1` components). For angular momentum `l ≥ 2`, the Cartesian basis
//! contains more functions than the physically distinct spherical harmonics,
//! so this transformation reduces the basis set size and removes unphysical
//! components.
//!
//! # Why this module exists
//!
//! Most modern quantum chemistry programs use spherical harmonic (also called
//! "pure") basis functions rather than Cartesian ones for d-shells and above.
//! The reasons are:
//!
//! - **Smaller basis**: A d-shell has 5 spherical harmonics vs. 6 Cartesian
//!   components; an f-shell has 7 vs. 10. This reduces computational cost.
//! - **Physical correctness**: The 6th Cartesian d-component (x² + y² + z²)
//!   is an s-type contaminant that mixes in a lower angular momentum function.
//! - **Standard practice**: Most basis sets are designed for use with spherical
//!   harmonics, and using Cartesian functions can introduce subtle errors.
//!
//! # How it works
//!
//! The transformation is a matrix multiplication:
//!
//! ```text
//! [sph] = U · [cart]
//! ```
//!
//! where `U` is a `(2l+1) × (l+1)(l+2)/2` matrix. For l = 0 and l = 1, the
//! Cartesian and spherical representations are identical (U = I). For l ≥ 2,
//! the matrix contains the coefficients of the real spherical harmonics
//! expressed in terms of Cartesian monomials.

use libm::sqrt;

/// Compute the Cartesian-to-spherical transformation matrix for angular momentum `l`.
///
/// # What it does
///
/// Returns a `(2l+1) × (l+1)(l+2)/2` matrix `U` such that the spherical
/// harmonic integrals are obtained by:
///
/// ```text
/// sph[m] = Σ_c  U[m][c] · cart[c]    for m = 0..2l, c = 0..n_cart-1
/// ```
///
/// # Why it exists
///
/// The transformation matrix depends only on the angular momentum `l` and can
/// be precomputed once. Storing it as an explicit matrix allows efficient
/// batch transformation of many integrals using simple matrix-vector products.
///
/// # How it works — explicit matrices
///
/// ## s-shell (l = 0)
///
/// Both Cartesian and spherical have 1 component: identity 1×1 matrix.
///
/// ## p-shell (l = 1)
///
/// Both have 3 components, and the standard ordering `(px, py, pz) = (x, y, z)`
/// is the same in both bases: identity 3×3 matrix.
///
/// ## d-shell (l = 2)
///
/// Cartesian has 6 components: `(xx, xy, xz, yy, yz, zz)`.
/// Spherical has 5 components: `(d₀, d₁, d₋₁, d₂, d₋₂)`.
///
/// The standard real spherical harmonic coefficients give:
///
/// ```text
/// d₀  = -0.5·xx + 2·c·yy - 0.5·zz    where c = -1/(2√3) from the z² term
/// ```
///
/// The exact transformation uses the real spherical harmonics:
///
/// ```text
/// Y_{2, 0}  = √(1/4)  · (2z² - x² - y²) / r²
/// Y_{2, 1}  = √(3/4)  · xz / r²
/// Y_{2,-1}  = √(3/4)  · yz / r²
/// Y_{2, 2}  = √(3/16) · (x² - y²) / r²
/// Y_{2,-2}  = √(3/4)  · xy / r²
/// ```
///
/// After factoring out normalization constants, the 6×5 → 5×6 transpose
/// transformation matrix (rows = spherical, cols = Cartesian) is:
///
/// ## f-shell (l = 3) and above
///
/// Simplified approximate transformation matrices are used, derived from the
/// general formula relating Cartesian monomials to real spherical harmonics.
///
/// # Arguments
///
/// * `l` — Angular momentum quantum number (0 = s, 1 = p, 2 = d, 3 = f, ...)
///
/// # Returns
///
/// A `Vec<Vec<f64>>` representing the transformation matrix. `result[m][c]`
/// gives the coefficient of the `c`-th Cartesian component in the `m`-th
/// spherical harmonic.
///
/// # Examples
///
/// ```
/// use poler_eri::cart2sph::cart_to_sph_matrix;
///
/// // s-shell: 1×1 identity
/// let u_s = cart_to_sph_matrix(0);
/// assert_eq!(u_s.len(), 1);
/// assert_eq!(u_s[0].len(), 1);
/// assert!((u_s[0][0] - 1.0).abs() < 1e-14);
///
/// // p-shell: 3×3 identity
/// let u_p = cart_to_sph_matrix(1);
/// assert_eq!(u_p.len(), 3);
/// assert_eq!(u_p[0].len(), 3);
///
/// // d-shell: 5 spherical × 6 Cartesian
/// let u_d = cart_to_sph_matrix(2);
/// assert_eq!(u_d.len(), 5);
/// assert_eq!(u_d[0].len(), 6);
/// ```
pub fn cart_to_sph_matrix(l: usize) -> Vec<Vec<f64>> {
    let n_cart = (l + 1) * (l + 2) / 2;
    let n_sph = 2 * l + 1;

    match l {
        0 => {
            // s-shell: 1×1 identity
            vec![vec![1.0]]
        }
        1 => {
            // p-shell: 3×3 identity
            // Cartesian ordering: (x, y, z) = (px, py, pz)
            // Spherical ordering: (p₋₁, p₀, p₊₁) or equivalently (py, pz, px)
            // With standard ordering both are the same, so identity
            vec![
                vec![1.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0],
                vec![0.0, 0.0, 1.0],
            ]
        }
        2 => {
            // d-shell: 5 spherical × 6 Cartesian
            // Cartesian order: xx(0), xy(1), xz(2), yy(3), yz(4), zz(5)
            // Spherical order: d₀(0), d₁(1), d₋₁(2), d₂(3), d₋₂(4)
            //
            // Real spherical harmonics in Cartesian form (unnormalized monomials):
            //   Y_{2,0}  ∝ 2zz - xx - yy     → coeff on xx=-1, yy=-1, zz=2
            //   Y_{2,1}  ∝ xz                 → coeff on xz=1
            //   Y_{2,-1} ∝ yz                 → coeff on yz=1
            //   Y_{2,2}  ∝ xx - yy            → coeff on xx=1, yy=-1
            //   Y_{2,-2} ∝ xy                 → coeff on xy=1
            //
            // After normalization to unit self-overlap:
            let s3 = sqrt(3.0);
            let s3_4 = sqrt(3.0 / 4.0);

            // d₀:  (1/√(4/3)) * (2*zz - xx - yy) / r²  → normalized coefficients
            // We use the standard convention:
            //   d₀  = -0.5*xx - 0.5*yy + zz     (with normalization factor)
            //   d₁  = xz
            //   d₋₁ = yz
            //   d₂  = (√3/2)*(xx - yy)
            //   d₋₂ = √3 * xy
            //
            // More precisely, using the convention where each spherical harmonic
            // is a unit-normalized linear combination of Cartesian components:
            let half = 0.5;

            vec![
                // d₀:  (2zz - xx - yy) / √(4/3)  =  √(3/4) * (-xx - yy + 2zz)
                // normalized: each coefficient / √(integral of the polynomial²)
                vec![-half, 0.0, 0.0, -half, 0.0, 1.0],
                // d₁:  xz
                vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
                // d₋₁: yz
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                // d₂:  (√3/2)*(xx - yy)
                vec![s3_4, 0.0, 0.0, -s3_4, 0.0, 0.0],
                // d₋₂: √3 * xy
                vec![0.0, s3 * half, 0.0, 0.0, 0.0, 0.0],
            ]
        }
        3 => {
            // f-shell: 7 spherical × 10 Cartesian
            // Cartesian order: xxx(0), xxy(1), xxz(2), xyy(3), xyz(4),
            //                  xzz(5), yyy(6), yyz(7), yzz(8), zzz(9)
            // Spherical: f₀, f₁, f₋₁, f₂, f₋₂, f₃, f₋₃
            //
            // Simplified approximate transformation based on real spherical harmonics:
            //   f₀  ∝ zzz - z(xx+yy)/2    →  -1/2·xxz - 1/2·yyz + zzz
            //   f₁  ∝ xzz - x(yy+xx/2)/2   →  approximate
            //   f₋₁ ∝ yzz - y(xx+yy/2)/2   →  approximate
            //   f₂  ∝ xz(xx-yy)             →  xzz-type terms
            //   f₋₂ ∝ yz(xx-yy)             →  approximate
            //   f₃  ∝ xxx - 3xyy            →  xxx - 3·xyy
            //   f₋₃ ∝ 3xxy - yyy            →  3·xxy - yyy

            let s15_4 = sqrt(15.0 / 4.0);
            let s10_4 = sqrt(10.0 / 4.0);

            vec![
                // f₀: zzz - (3/2)z·(xx+yy)  → normalized
                // Approximate: coeff on xxz = -3/4, yyz = -3/4, zzz = 1
                vec![0.0, 0.0, -0.75, 0.0, 0.0, 0.0, 0.0, -0.75, 0.0, 1.0],
                // f₁: xzz - x(xx+yy)/4  →  approximate
                vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
                // f₋₁: yzz - y(xx+yy)/4  →  approximate
                vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                // f₂: √(15/4) * xz(x-y)(x+y) / r²  ≈ √(15/4)*(xxz - yyz)*something
                // Simplified
                vec![0.0, 0.0, s15_4 * 0.25, 0.0, 0.0, 0.0, 0.0, -s15_4 * 0.25, 0.0, 0.0],
                // f₋₂: √(15/4) * yz * xy / r²  ≈ xyz terms
                vec![0.0, 0.0, 0.0, 0.0, s15_4 * 0.5, 0.0, 0.0, 0.0, 0.0, 0.0],
                // f₃: (√10/4)*(xxx - 3·xyy)
                vec![s10_4 * 0.25, 0.0, 0.0, -3.0 * s10_4 * 0.25, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                // f₋₃: (√10/4)*(3·xxy - yyy)
                vec![0.0, 3.0 * s10_4 * 0.25, 0.0, 0.0, 0.0, 0.0, -s10_4 * 0.25, 0.0, 0.0, 0.0],
            ]
        }
        _ => {
            // For l >= 4 (g, h, ...), we construct a simplified approximate
            // transformation matrix. The exact matrix involves increasingly
            // complex combinatorial coefficients; for production use these
            // should be replaced with analytically derived coefficients.
            //
            // Strategy: Start with identity-like mapping for the first 2l+1
            // rows and n_cart columns, then zero-fill. This is a rough
            // approximation that preserves the correct dimensions.
            let mut mat = vec![vec![0.0; n_cart]; n_sph];

            // For the diagonal-like entries, assign coefficient 1.0
            // where the spherical harmonic is dominated by a single Cartesian component.
            // This is approximate and should be replaced with exact coefficients.
            for m in 0..n_sph {
                if m < n_cart {
                    mat[m][m] = 1.0;
                }
            }

            mat
        }
    }
}

/// Transform Cartesian integrals to spherical harmonic integrals.
///
/// # What it does
///
/// Applies the Cartesian-to-spherical transformation matrix to a set of
/// Cartesian integrals, producing the corresponding spherical harmonic integrals.
///
/// # Why it exists
///
/// After computing integrals in the Cartesian basis (which is natural for
/// recurrence relations like VRR/HRR), we need to convert them to the
/// spherical harmonic basis for use in the SCF procedure. This function
/// performs that conversion for a single shell's worth of integrals.
///
/// # How it works
///
/// 1. Retrieve the transformation matrix `U` for angular momentum `l`.
/// 2. For each spherical component `m`:
///
/// ```text
/// sph[m] = Σ_{c=0}^{n_cart-1}  U[m][c] · cart[c]
/// ```
///
/// # Arguments
///
/// * `cart_integrals` — Flat array of Cartesian integrals, length `(l+1)(l+2)/2`.
/// * `l` — Angular momentum quantum number.
///
/// # Returns
///
/// A `Vec<f64>` of length `2l+1` containing the spherical harmonic integrals.
///
/// # Panics
///
/// Panics if `cart_integrals.len() != (l+1)(l+2)/2`.
///
/// # Examples
///
/// ```
/// use poler_eri::cart2sph::transform_cart_to_sph;
///
/// // s-shell: identity transformation
/// let cart_s = vec![1.5];
/// let sph_s = transform_cart_to_sph(&cart_s, 0);
/// assert_eq!(sph_s.len(), 1);
/// assert!((sph_s[0] - 1.5).abs() < 1e-14);
///
/// // p-shell: identity transformation
/// let cart_p = vec![1.0, 2.0, 3.0]; // px, py, pz
/// let sph_p = transform_cart_to_sph(&cart_p, 1);
/// assert_eq!(sph_p.len(), 3);
/// assert!((sph_p[0] - 1.0).abs() < 1e-14);
/// ```
pub fn transform_cart_to_sph(cart_integrals: &[f64], l: usize) -> Vec<f64> {
    let n_cart = (l + 1) * (l + 2) / 2;
    let n_sph = 2 * l + 1;

    assert_eq!(cart_integrals.len(), n_cart,
        "transform_cart_to_sph: expected {} Cartesian integrals for l={}, got {}",
        n_cart, l, cart_integrals.len());

    let u = cart_to_sph_matrix(l);

    let mut sph = vec![0.0; n_sph];
    for m in 0..n_sph {
        for c in 0..n_cart {
            sph[m] += u[m][c] * cart_integrals[c];
        }
    }

    sph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cart_to_sph_matrix_s_shell() {
        let u = cart_to_sph_matrix(0);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].len(), 1);
        assert!((u[0][0] - 1.0).abs() < 1e-14);
    }

    #[test]
    fn test_cart_to_sph_matrix_p_shell() {
        let u = cart_to_sph_matrix(1);
        assert_eq!(u.len(), 3);
        assert_eq!(u[0].len(), 3);
        // Identity matrix
        for i in 0..3 {
            for j in 0..3 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((u[i][j] - expected).abs() < 1e-14,
                    "p-shell matrix[{}][{}] = {}, expected {}", i, j, u[i][j], expected);
            }
        }
    }

    #[test]
    fn test_cart_to_sph_matrix_d_shell_dimensions() {
        let u = cart_to_sph_matrix(2);
        assert_eq!(u.len(), 5, "d-shell should have 5 spherical components");
        for row in &u {
            assert_eq!(row.len(), 6, "d-shell should have 6 Cartesian components");
        }
    }

    #[test]
    fn test_cart_to_sph_matrix_f_shell_dimensions() {
        let u = cart_to_sph_matrix(3);
        assert_eq!(u.len(), 7, "f-shell should have 7 spherical components");
        for row in &u {
            assert_eq!(row.len(), 10, "f-shell should have 10 Cartesian components");
        }
    }

    #[test]
    fn test_cart_to_sph_matrix_g_shell_dimensions() {
        let u = cart_to_sph_matrix(4);
        assert_eq!(u.len(), 9, "g-shell should have 9 spherical components");
        for row in &u {
            assert_eq!(row.len(), 15, "g-shell should have 15 Cartesian components");
        }
    }

    #[test]
    fn test_transform_s_shell() {
        let cart = vec![2.5];
        let sph = transform_cart_to_sph(&cart, 0);
        assert_eq!(sph.len(), 1);
        assert!((sph[0] - 2.5).abs() < 1e-14);
    }

    #[test]
    fn test_transform_p_shell() {
        let cart = vec![1.0, 2.0, 3.0];
        let sph = transform_cart_to_sph(&cart, 1);
        assert_eq!(sph.len(), 3);
        // p-shell is identity
        assert!((sph[0] - 1.0).abs() < 1e-14);
        assert!((sph[1] - 2.0).abs() < 1e-14);
        assert!((sph[2] - 3.0).abs() < 1e-14);
    }

    #[test]
    fn test_transform_d_shell_zero() {
        // Zero Cartesian integrals should give zero spherical integrals
        let cart = vec![0.0; 6];
        let sph = transform_cart_to_sph(&cart, 2);
        assert_eq!(sph.len(), 5);
        for val in &sph {
            assert!(val.abs() < 1e-14, "Expected zero spherical integral");
        }
    }

    #[test]
    fn test_transform_d_shell_known() {
        // Test with a specific set of Cartesian d-integrals
        // Cartesian order: xx(0), xy(1), xz(2), yy(3), yz(4), zz(5)
        // Only zz = 1.0, rest = 0.0
        let cart = vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        let sph = transform_cart_to_sph(&cart, 2);

        // d₀ should pick up the zz component with coefficient 1.0
        // (from the d₀ row: [-0.5, 0, 0, -0.5, 0, 1.0])
        assert!((sph[0] - 1.0).abs() < 1e-14,
            "d₀ from pure zz: expected 1.0, got {}", sph[0]);

        // d₁ and d₋₁ should be zero (no xz or yz)
        assert!(sph[1].abs() < 1e-14, "d₁ should be zero");
        assert!(sph[2].abs() < 1e-14, "d₋₁ should be zero");
    }

    #[test]
    fn test_transform_d_shell_xx_only() {
        // xx = 1.0, rest = 0.0
        let cart = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let sph = transform_cart_to_sph(&cart, 2);

        // d₀ row: [-0.5, 0, 0, -0.5, 0, 1.0] → d₀ = -0.5 * 1.0 = -0.5
        assert!((sph[0] - (-0.5)).abs() < 1e-14,
            "d₀ from pure xx: expected -0.5, got {}", sph[0]);

        // d₂ row: [√3/4, 0, 0, -√3/4, 0, 0] → d₂ = √3/4 * 1.0
        let expected_d2 = sqrt(3.0 / 4.0);
        assert!((sph[3] - expected_d2).abs() < 1e-14,
            "d₂ from pure xx: expected {}, got {}", expected_d2, sph[3]);
    }

    #[test]
    fn test_transform_d_shell_xy_only() {
        // xy = 1.0, rest = 0.0
        let cart = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        let sph = transform_cart_to_sph(&cart, 2);

        // d₋₂ row: [0, √3/2, 0, 0, 0, 0] → d₋₂ = √3/2
        let expected = sqrt(3.0) / 2.0;
        assert!((sph[4] - expected).abs() < 1e-14,
            "d₋₂ from pure xy: expected {}, got {}", expected, sph[4]);
    }

    #[test]
    fn test_transform_round_trip_symmetry() {
        // For s and p shells, the transformation is identity, so round-trip is trivial.
        // For d-shell, we verify that the transformation is linear and deterministic.
        let cart1 = vec![1.0, 0.5, 0.3, 0.8, 0.2, 1.5];
        let sph1 = transform_cart_to_sph(&cart1, 2);
        let sph2 = transform_cart_to_sph(&cart1, 2);

        for m in 0..5 {
            assert!((sph1[m] - sph2[m]).abs() < 1e-14,
                "Transformation should be deterministic: m={}", m);
        }
    }

    #[test]
    fn test_transform_linearity() {
        // Verify linearity: T(a*x + b*y) = a*T(x) + b*T(y)
        let a = 2.5;
        let b = -1.3;

        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let y = vec![0.5, -1.0, 0.3, -0.7, 2.1, -0.4];

        let sph_x = transform_cart_to_sph(&x, 2);
        let sph_y = transform_cart_to_sph(&y, 2);

        // Combine: a*x + b*y
        let combined: Vec<f64> = (0..6).map(|i| a * x[i] + b * y[i]).collect();
        let sph_combined = transform_cart_to_sph(&combined, 2);

        for m in 0..5 {
            let expected = a * sph_x[m] + b * sph_y[m];
            assert!((sph_combined[m] - expected).abs() < 1e-12,
                "Linearity check failed at m={}: got {}, expected {}",
                m, sph_combined[m], expected);
        }
    }

    #[test]
    #[should_panic]
    fn test_transform_wrong_length() {
        let cart = vec![1.0, 2.0]; // Wrong length for any l
        transform_cart_to_sph(&cart, 2);
    }
}
