//! GTO (Gaussian Type Orbital) normalization for contracted basis shells.
//!
//! # What this module does
//!
//! This module computes the normalized contraction coefficients for Gaussian
//! basis function shells. Raw basis set files provide unnormalized exponents
//! (`alphas`) and coefficients (`coeffs`), but integral evaluation requires
//! properly normalized coefficients (`norm_coeffs`) that account for the
//! self-overlap of each primitive Gaussian.
//!
//! # Why this module exists
//!
//! Gaussian integrals (overlap, kinetic, nuclear attraction, ERI) are derived
//! assuming normalized basis functions. If we skip normalization, the integrals
//! would be off by factors that depend on the exponents and angular momentum,
//! making results meaningless.
//!
//! The normalization procedure transforms raw coefficients into coefficients
//! that, when combined with the primitive Gaussian, produce functions with
//! unit self-overlap. This is done per-primitive at the shell level; per-component
//! Cartesian normalization (for nx, ny, nz individually) is handled separately
//! in the [`cart2sph`](crate::cart2sph) module.
//!
//! # How it works
//!
//! The normalization constant for a primitive Gaussian with exponent α and
//! angular momentum l is:
//!
//! ```text
//! N(α, l) = (2α/π)^(3/4) · (4α)^(l/2) / √((2l-1)!!)
//! ```
//!
//! For the s-shell (l=0), this simplifies to the well-known result:
//!
//! ```text
//! N(α, 0) = (2α/π)^(3/4)
//! ```
//!
//! For each primitive i in a shell, the normalized coefficient is:
//!
//! ```text
//! norm_coeffs[i] = coeffs[i] · N(α_i, l)
//! ```
//!
//! The double factorial `(2l-1)!! = (2l-1)·(2l-3)·...·3·1` is computed by
//! the [`fact2`] helper function.

use crate::types::Shell;

/// Compute the double factorial n!! = n · (n-2) · (n-4) · ... · 2 or 1.
///
/// # What it does
///
/// Returns the double factorial of `n`. For even `n`, the product terminates at 2;
/// for odd `n`, it terminates at 1. By convention, `0!! = 1`.
///
/// # Why it exists
///
/// Double factorials appear in the GTO normalization constant and in the
/// analytical expressions for overlap integrals of higher-angular-momentum
/// shells. They are a building block for the combinatorial factors that arise
/// when integrating powers of coordinates against Gaussian functions.
///
/// # How it works
///
/// Simple iterative product: start at `n` and decrement by 2 until reaching
/// 0 or 1. Returns 1 for the base case `n = 0`.
///
/// # Examples
///
/// ```
/// use poler_eri::normalization::fact2;
///
/// assert_eq!(fact2(0), 1);   // 0!! = 1 (by convention)
/// assert_eq!(fact2(1), 1);   // 1!! = 1
/// assert_eq!(fact2(2), 2);   // 2!! = 2
/// assert_eq!(fact2(3), 3);   // 3!! = 3·1 = 3
/// assert_eq!(fact2(4), 8);   // 4!! = 4·2 = 8
/// assert_eq!(fact2(5), 15);  // 5!! = 5·3·1 = 15
/// assert_eq!(fact2(6), 48);  // 6!! = 6·4·2 = 48
/// assert_eq!(fact2(7), 105); // 7!! = 7·5·3·1 = 105
/// ```
pub fn fact2(n: usize) -> usize {
    if n < 2 {
        return 1;
    }
    let mut result = 1usize;
    let mut k = n;
    while k > 0 {
        result *= k;
        if k <= 2 {
            break;
        }
        k -= 2;
    }
    result
}

/// Compute the GTO normalization constant N(α, l) for a primitive Gaussian.
///
/// # What it does
///
/// Returns the normalization constant that, when multiplied by a raw
/// contraction coefficient, produces a properly normalized primitive
/// Gaussian function with unit self-overlap.
///
/// # Why it exists
///
/// The raw coefficients in basis set files (e.g., STO-3G, cc-pVDZ) are not
/// normalized. They must be multiplied by the normalization constant before
/// use in integral evaluation. This function isolates the math so that
/// [`normalize_shell`] can simply loop over primitives.
///
/// # How it works
///
/// The formula is:
///
/// ```text
/// N(α, l) = (2α/π)^(3/4) · (4α)^(l/2) / √((2l-1)!!)
/// ```
///
/// - The first factor `(2α/π)^(3/4)` is the s-shell normalization, ensuring
///   that the Gaussian envelope has unit self-overlap.
/// - The second factor `(4α)^(l/2)` accounts for the increased overlap from
///   the polynomial prefactor x^l.
/// - The third factor `1/√((2l-1)!!)` compensates for the combinatorial
///   growth of the overlap integral with angular momentum.
///
/// For l=0, the angular factors collapse to 1, giving:
///
/// ```text
/// N(α, 0) = (2α/π)^(3/4)
/// ```
///
/// # Arguments
///
/// * `alpha` — Gaussian exponent (must be positive)
/// * `l` — Angular momentum quantum number (0 for s, 1 for p, etc.)
///
/// # Examples
///
/// ```ignore
/// use poler_eri::normalization::gto_norm;
///
/// // s-shell normalization
/// let alpha = 1.0;
/// let n_s = gto_norm(alpha, 0);
/// // N(1.0, 0) = (2/π)^(3/4) ≈ 1.1284
/// assert!((n_s - 1.1284).abs() < 0.001);
/// ```
fn gto_norm(alpha: f64, l: usize) -> f64 {
    use std::f64::consts::PI;

    // s-shell part: (2α/π)^(3/4)
    let s_part = libm::pow(2.0 * alpha / PI, 0.75);

    if l == 0 {
        return s_part;
    }

    // Angular part: (4α)^(l/2) / √((2l-1)!!)
    // We compute this as a product of l factors: ∏_{i=1}^{l} √(4α / (2i-1))
    // This avoids overflow in fact2 for large l and is numerically stable.
    let mut result = s_part;
    for i in 1..=l {
        result *= libm::sqrt(4.0 * alpha / (2 * i - 1) as f64);
    }
    result
}

/// Normalize a shell's contraction coefficients in-place.
///
/// # What it does
///
/// Computes `norm_coeffs[i] = coeffs[i] * N(α_i, l)` for each primitive `i`
/// in the shell, where `N(α, l)` is the GTO normalization constant computed
/// by [`gto_norm`]. The result is stored in `shell.norm_coeffs`.
///
/// # Why it exists
///
/// Integral evaluation routines (overlap, kinetic, nuclear, ERI) require
/// normalized coefficients. This function must be called once after creating
/// a [`Shell`] from raw basis set data, before any integrals are computed.
/// The `Shell::new` constructor initializes `norm_coeffs` to zeros; this
/// function fills in the correct values.
///
/// # How it works
///
/// 1. For each primitive `i` in the shell:
///    a. Retrieve the exponent `alpha = alphas[i]` and raw coefficient `c = coeffs[i]`.
///    b. Compute the normalization constant `N = gto_norm(alpha, l)`.
///    c. Set `norm_coeffs[i] = c * N`.
///
/// The angular momentum `l` is shared across all primitives in the shell,
/// but each primitive has its own exponent, so the normalization constant
/// differs per primitive.
///
/// # Arguments
///
/// * `shell` — A mutable reference to a [`Shell`]. After this call,
///   `shell.norm_coeffs` will contain the normalized coefficients.
///
/// # Example
///
/// ```
/// use poler_eri::types::Shell;
/// use poler_eri::normalization::normalize_shell;
///
/// // Create an unnormalized s-shell with one primitive
/// let mut shell = Shell::new(0, 0, vec![3.4252509100], vec![0.1543289673]);
/// normalize_shell(&mut shell);
///
/// // norm_coeffs[0] should now be coeffs[0] * N(3.425, 0)
/// assert!(shell.norm_coeffs[0] != 0.0);
/// assert!(shell.norm_coeffs[0] > 0.0);
/// ```
pub fn normalize_shell(shell: &mut Shell) {
    let l = shell.l;
    for i in 0..shell.n_prim() {
        let alpha = shell.alphas[i];
        let coeff = shell.coeffs[i];
        shell.norm_coeffs[i] = coeff * gto_norm(alpha, l);
    }
}

/// Renormalize the CONTRACTION to exact unit self-overlap (in-place).
///
/// STO-3G-style published coefficients are least-squares fits to STOs and
/// leave the contraction slightly unnormalized (e.g. H/STO-3G self-overlap
/// is 0.999454). Production quantum-chemistry codes renormalize at load
/// time:  d_i -> d_i / sqrt(<phi|phi>). This function does exactly that,
/// on top of the per-primitive normalization done by [`normalize_shell`].
///
/// The self-overlap of the contracted shell for angular momentum l is
/// `<phi|phi> = sum_ij d_i d_j * S_ij` with the primitive overlap
/// `S_ij = (pi/(a_i+a_j))^(3/2) * fact2(2l-1)!! / (2(a_i+a_j))^l`
/// (for one Cartesian component; all components are equivalent).
///
/// AUDIT (v3.2.0-a): this step was missing entirely, which left every
/// STO-3G contraction off by up to ~7e-4 in norm.
pub fn normalize_contraction(shell: &mut Shell) {
    use std::f64::consts::PI;
    let l = shell.l;
    let n = shell.n_prim();
    let odd2 = fact2(2 * l + 1) as f64; // (2l+1)!! = (2l-1)!! * (2l+1)... careful
    // For the l-th Cartesian component: <x^l g_i | x^l g_j> =
    //   (pi/(a_i+a_j))^(3/2) * (2l-1)!! / (2(a_i+a_j))^l
    let df = fact2(2 * l.saturating_sub(1)) as f64; // (2l-1)!!, with (-1)!! = 1
    let _ = odd2;
    let mut self_overlap = 0.0f64;
    for i in 0..n {
        for j in 0..n {
            let b = shell.alphas[i] + shell.alphas[j];
            let s_ij = libm::pow(PI / b, 1.5) * df / libm::pow(2.0 * b, l as f64);
            self_overlap += shell.norm_coeffs[i] * shell.norm_coeffs[j] * s_ij;
        }
    }
    if self_overlap > 1e-12 {
        let inv = 1.0 / libm::sqrt(self_overlap);
        for c in shell.norm_coeffs.iter_mut() {
            *c *= inv;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Shell;

    #[test]
    fn test_fact2_base_cases() {
        assert_eq!(fact2(0), 1);
        assert_eq!(fact2(1), 1);
        assert_eq!(fact2(2), 2);
    }

    #[test]
    fn test_fact2_odd() {
        assert_eq!(fact2(3), 3);
        assert_eq!(fact2(5), 15);
        assert_eq!(fact2(7), 105);
        assert_eq!(fact2(9), 945);
    }

    #[test]
    fn test_fact2_even() {
        assert_eq!(fact2(4), 8);
        assert_eq!(fact2(6), 48);
        assert_eq!(fact2(8), 384);
        assert_eq!(fact2(10), 3840);
    }

    #[test]
    fn test_gto_norm_s_shell() {
        // For s-shell: N(α, 0) = (2α/π)^(3/4)
        use std::f64::consts::PI;
        let alpha = 1.0;
        let expected = libm::pow(2.0 * alpha / PI, 0.75);
        let computed = gto_norm(alpha, 0);
        assert!((computed - expected).abs() < 1e-12,
            "s-shell norm: expected {}, got {}", expected, computed);
    }

    #[test]
    fn test_gto_norm_s_shell_alpha_variations() {
        // N(α, 0) should increase with α (tighter Gaussian → larger normalization)
        let n1 = gto_norm(1.0, 0);
        let n2 = gto_norm(10.0, 0);
        let n3 = gto_norm(100.0, 0);
        assert!(n2 > n1, "N(10) > N(1) for s-shell");
        assert!(n3 > n2, "N(100) > N(10) for s-shell");
    }

    #[test]
    fn test_gto_norm_p_shell() {
        // For p-shell: N(α, 1) = (2α/π)^(3/4) * √(4α)
        use std::f64::consts::PI;
        let alpha = 2.0;
        let s_part = libm::pow(2.0 * alpha / PI, 0.75);
        let expected = s_part * libm::sqrt(4.0 * alpha);
        let computed = gto_norm(alpha, 1);
        assert!((computed - expected).abs() < 1e-12,
            "p-shell norm: expected {}, got {}", expected, computed);
    }

    #[test]
    fn test_gto_norm_d_shell() {
        // For d-shell: N(α, 2) = (2α/π)^(3/4) * 4α / √3
        use std::f64::consts::PI;
        let alpha = 3.0;
        let s_part = libm::pow(2.0 * alpha / PI, 0.75);
        let expected = s_part * 4.0 * alpha / libm::sqrt(3.0);
        let computed = gto_norm(alpha, 2);
        assert!((computed - expected).abs() < 1e-12,
            "d-shell norm: expected {}, got {}", expected, computed);
    }

    #[test]
    fn test_gto_norm_higher_l_increases_norm() {
        // For sufficiently large α, the (4α)^(l/2) factor dominates the
        // double factorial in the denominator, so normalization increases with l.
        // For α = 1.0, the double factorial catches up at l=3, so use larger α.
        let alpha = 5.0;
        let n0 = gto_norm(alpha, 0);
        let n1 = gto_norm(alpha, 1);
        let n2 = gto_norm(alpha, 2);
        let n3 = gto_norm(alpha, 3);
        assert!(n1 > n0, "p norm > s norm for α=5");
        assert!(n2 > n1, "d norm > p norm for α=5");
        assert!(n3 > n2, "f norm > d norm for α=5");
    }

    #[test]
    fn test_gto_norm_increases_monotonically_for_large_alpha() {
        // For very large α, the angular part grows as (4α)^(l/2) which
        // always dominates the double factorial
        let alpha = 100.0;
        let mut prev = gto_norm(alpha, 0);
        for l in 1..=5 {
            let curr = gto_norm(alpha, l);
            assert!(curr > prev, "N(α={}, l={}) > N(α={}, l={})", alpha, l, alpha, l-1);
            prev = curr;
        }
    }

    #[test]
    fn test_normalize_shell_s_type() {
        let mut shell = Shell::new(0, 0, vec![3.4252509100], vec![0.1543289673]);
        normalize_shell(&mut shell);

        // norm_coeffs should be positive and non-zero
        assert!(shell.norm_coeffs[0] > 0.0);

        // It should equal coeff * N(alpha, 0)
        let expected = 0.1543289673 * gto_norm(3.4252509100, 0);
        assert!((shell.norm_coeffs[0] - expected).abs() < 1e-12);
    }

    #[test]
    fn test_normalize_shell_multiple_primitives() {
        // STO-3G-like s-shell with 3 primitives
        let alphas = vec![3.4252509100, 0.6239137298, 0.1688554040];
        let coeffs = vec![0.1543289673, 0.5353281423, 0.4446345422];
        let mut shell = Shell::new(0, 0, alphas.clone(), coeffs.clone());
        normalize_shell(&mut shell);

        for i in 0..3 {
            let expected = coeffs[i] * gto_norm(alphas[i], 0);
            assert!((shell.norm_coeffs[i] - expected).abs() < 1e-12,
                "Primitive {}: expected {}, got {}", i, expected, shell.norm_coeffs[i]);
        }
    }

    #[test]
    fn test_normalize_shell_p_type() {
        let mut shell = Shell::new(1, 0, vec![1.0], vec![1.0]);
        normalize_shell(&mut shell);

        let expected = gto_norm(1.0, 1);
        assert!((shell.norm_coeffs[0] - expected).abs() < 1e-12);
    }
}
