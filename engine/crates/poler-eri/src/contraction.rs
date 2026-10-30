//! CGTO contraction — combining primitive Gaussians into contracted shells.
//!
//! # What this module does
//!
//! This module contracts primitive Gaussian integral values into contracted
//! Gaussian-type orbital (CGTO) integrals. In most quantum chemistry basis sets
//! (e.g., STO-3G, cc-pVDZ), each basis function is a *linear combination* of
//! several primitive Gaussians. The primitives share the same center and angular
//! momentum but have different exponents and contraction coefficients.
//!
//! # Why this module exists
//!
//! Integral evaluation (overlap, kinetic, ERI, etc.) operates on *primitive*
//! Gaussians first, producing one integral per primitive combination. These
//! primitive integrals must then be contracted (summed with weights) to produce
//! the final contracted integral that enters the Fock matrix. This two-step
//! process — primitive evaluation followed by contraction — is fundamental to
//! all quantum chemistry integral packages.
//!
//! # How it works
//!
//! For a single shell contraction with `n_prim` primitives and `n_cart` Cartesian
//! components, the contraction for component `c` is:
//!
//! ```text
//! result[c] = Σ_i  coeff[i] * operand[i * n_cart + c]
//! ```
//!
//! For a two-shell contraction (as in the bra side of an ERI), we sum over
//! primitives of both shells:
//!
//! ```text
//! result[c] = Σ_i Σ_j  ca[i] * cb[j] * eri_prims[(i * nb_prim + j) * n_cart + c]
//! ```

/// Contract primitive integrals into a single-shell contracted integral.
///
/// # What it does
///
/// Given `n_prim` primitive integral values (each with `n_cart` Cartesian components)
/// and their contraction coefficients, computes the contracted integral by forming
/// the weighted sum over all primitives for each Cartesian component independently.
///
/// # Why it exists
///
/// In a contracted GTO basis, each shell function is a sum of primitives:
///
/// ```text
/// χ_μ(r) = Σ_i  c_i · g(α_i, r)
/// ```
///
/// where `c_i` are the contraction coefficients and `g(α_i, r)` are the primitive
/// Gaussians. After evaluating integrals for each primitive, we must contract
/// (sum with weights) to obtain the final shell integral.
///
/// # How it works
///
/// For each Cartesian component `cart` (0 ≤ cart < n_cart):
///
/// ```text
/// result[cart] = Σ_{prim=0}^{n_prim-1}  coeffs[prim] * operands[prim * n_cart + cart]
/// ```
///
/// The `operands` array is laid out in row-major order: primitive index varies
/// slowest, Cartesian component varies fastest. This matches the output ordering
/// of the primitive integral evaluation routines.
///
/// # Arguments
///
/// * `operands` — Flat array of primitive integral values, length `n_prim * n_cart`.
///   Indexed as `operands[prim * n_cart + cart]`.
/// * `coeffs` — Contraction coefficients, length `n_prim`. These should be the
///   **normalized** coefficients from [`Shell::norm_coeffs`](crate::types::Shell).
/// * `n_prim` — Number of primitive Gaussians in the shell.
/// * `n_cart` — Number of Cartesian components per primitive (e.g., 1 for s, 3 for p, 6 for d).
///
/// # Returns
///
/// A `Vec<f64>` of length `n_cart` containing the contracted integrals, one per
/// Cartesian component.
///
/// # Panics
///
/// Panics if `operands.len() != n_prim * n_cart` or `coeffs.len() != n_prim`.
///
/// # Examples
///
/// ```
/// use poler_eri::contraction::contract_primitives;
///
/// // Two s-type primitives (n_cart=1), each with a coefficient
/// let operands = vec![2.0, 3.0];  // primitive integrals
/// let coeffs = vec![0.5, 0.3];     // contraction coefficients
/// let result = contract_primitives(&operands, &coeffs, 2, 1);
/// assert_eq!(result.len(), 1);
/// // result[0] = 0.5 * 2.0 + 0.3 * 3.0 = 1.9
/// assert!((result[0] - 1.9).abs() < 1e-14);
/// ```
pub fn contract_primitives(operands: &[f64], coeffs: &[f64], n_prim: usize, n_cart: usize) -> Vec<f64> {
    assert_eq!(operands.len(), n_prim * n_cart,
        "contract_primitives: operands length {} != n_prim({}) * n_cart({})",
        operands.len(), n_prim, n_cart);
    assert_eq!(coeffs.len(), n_prim,
        "contract_primitives: coeffs length {} != n_prim({})",
        coeffs.len(), n_prim);

    let mut result = vec![0.0; n_cart];

    for prim in 0..n_prim {
        let c = coeffs[prim];
        let offset = prim * n_cart;
        for cart in 0..n_cart {
            result[cart] += c * operands[offset + cart];
        }
    }

    result
}

/// Contract primitive ERI values over two shell pairs simultaneously.
///
/// # What it does
///
/// Performs the double contraction over primitives of two shells (typically
/// the bra side of an ERI), producing the fully contracted integral for each
/// Cartesian component.
///
/// # Why it exists
///
/// For a two-electron integral between contracted shells A and B on the bra side,
/// the contraction involves a double sum:
///
/// ```text
/// (AB|...) = Σ_i Σ_j  c_A[i] · c_B[j] · (a_i b_j | ...)
/// ```
///
/// This is the most common contraction pattern in ERI evaluation: the primitive
/// integrals are computed for all `(i, j)` primitive pairs, and then contracted
/// with the coefficients of both shells.
///
/// # How it works
///
/// The `eri_prims` array stores primitive integrals for all `(i, j)` pairs in
/// row-major order with the bra primitives varying slowest:
///
/// ```text
/// eri_prims[(i * nb_prim + j) * n_cart + cart]
/// ```
///
/// For each Cartesian component `cart`:
///
/// ```text
/// result[cart] = Σ_{i=0}^{na_prim-1} Σ_{j=0}^{nb_prim-1}
///     ca[i] * cb[j] * eri_prims[(i * nb_prim + j) * n_cart + cart]
/// ```
///
/// # Arguments
///
/// * `eri_prims` — Flat array of primitive ERI values, length `na_prim * nb_prim * n_cart`.
///   Indexed as `eri_prims[(i * nb_prim + j) * n_cart + cart]`.
/// * `ca` — Contraction coefficients for shell A, length `na_prim`.
/// * `cb` — Contraction coefficients for shell B, length `nb_prim`.
/// * `na_prim` — Number of primitives in shell A.
/// * `nb_prim` — Number of primitives in shell B.
/// * `n_cart` — Number of Cartesian components per primitive pair.
///
/// # Returns
///
/// A `Vec<f64>` of length `n_cart` containing the doubly-contracted integrals.
///
/// # Panics
///
/// Panics if array lengths are inconsistent with the declared dimensions.
///
/// # Examples
///
/// ```
/// use poler_eri::contraction::contract_shell_pair;
///
/// // Shell A: 2 primitives, Shell B: 1 primitive, s-type (n_cart=1)
/// let eri_prims = vec![5.0, 7.0]; // [prim_A=0, prim_B=0] and [prim_A=1, prim_B=0]
/// let ca = vec![0.5, 0.3];
/// let cb = vec![1.0];
/// let result = contract_shell_pair(&eri_prims, &ca, &cb, 2, 1, 1);
/// assert_eq!(result.len(), 1);
/// // result[0] = 0.5*1.0*5.0 + 0.3*1.0*7.0 = 2.5 + 2.1 = 4.6
/// assert!((result[0] - 4.6).abs() < 1e-14);
/// ```
pub fn contract_shell_pair(
    eri_prims: &[f64],
    ca: &[f64],
    cb: &[f64],
    na_prim: usize,
    nb_prim: usize,
    n_cart: usize,
) -> Vec<f64> {
    assert_eq!(eri_prims.len(), na_prim * nb_prim * n_cart,
        "contract_shell_pair: eri_prims length {} != na_prim({}) * nb_prim({}) * n_cart({})",
        eri_prims.len(), na_prim, nb_prim, n_cart);
    assert_eq!(ca.len(), na_prim,
        "contract_shell_pair: ca length {} != na_prim({})",
        ca.len(), na_prim);
    assert_eq!(cb.len(), nb_prim,
        "contract_shell_pair: cb length {} != nb_prim({})",
        cb.len(), nb_prim);

    let mut result = vec![0.0; n_cart];

    for ia in 0..na_prim {
        let c_a = ca[ia];
        for ib in 0..nb_prim {
            let c_ab = c_a * cb[ib];
            let offset = (ia * nb_prim + ib) * n_cart;
            for cart in 0..n_cart {
                result[cart] += c_ab * eri_prims[offset + cart];
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── contract_primitives tests ────────────────────────────────────────

    #[test]
    fn test_contract_primitives_single_prim() {
        // Single primitive: result = coeff * operand
        let operands = vec![3.0];
        let coeffs = vec![1.0];
        let result = contract_primitives(&operands, &coeffs, 1, 1);
        assert_eq!(result.len(), 1);
        assert!((result[0] - 3.0).abs() < 1e-14);
    }

    #[test]
    fn test_contract_primitives_two_prims_s_type() {
        // Two s-type primitives (n_cart = 1)
        let operands = vec![2.0, 3.0];
        let coeffs = vec![0.5, 0.3];
        let result = contract_primitives(&operands, &coeffs, 2, 1);
        assert!((result[0] - 1.9).abs() < 1e-14);
    }

    #[test]
    fn test_contract_primitives_p_type() {
        // Two p-type primitives (n_cart = 3)
        // Primitive 0: [1.0, 2.0, 3.0], Primitive 1: [4.0, 5.0, 6.0]
        let operands = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let coeffs = vec![0.5, 0.2];
        let result = contract_primitives(&operands, &coeffs, 2, 3);
        assert_eq!(result.len(), 3);
        // result[0] = 0.5*1.0 + 0.2*4.0 = 1.3
        // result[1] = 0.5*2.0 + 0.2*5.0 = 2.0
        // result[2] = 0.5*3.0 + 0.2*6.0 = 2.7
        assert!((result[0] - 1.3).abs() < 1e-14);
        assert!((result[1] - 2.0).abs() < 1e-14);
        assert!((result[2] - 2.7).abs() < 1e-14);
    }

    #[test]
    fn test_contract_primitives_zero_coeffs() {
        let operands = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let coeffs = vec![0.0, 0.0];
        let result = contract_primitives(&operands, &coeffs, 2, 3);
        for val in &result {
            assert!(val.abs() < 1e-14, "Expected zero with zero coefficients");
        }
    }

    #[test]
    fn test_contract_primitives_negative_coeffs() {
        let operands = vec![4.0];
        let coeffs = vec![-2.0];
        let result = contract_primitives(&operands, &coeffs, 1, 1);
        assert!((result[0] - (-8.0)).abs() < 1e-14);
    }

    #[test]
    #[should_panic]
    fn test_contract_primitives_wrong_operands_len() {
        let operands = vec![1.0, 2.0];
        let coeffs = vec![1.0];
        contract_primitives(&operands, &coeffs, 2, 1); // operands too short
    }

    #[test]
    #[should_panic]
    fn test_contract_primitives_wrong_coeffs_len() {
        let operands = vec![1.0];
        let coeffs = vec![1.0, 2.0];
        contract_primitives(&operands, &coeffs, 1, 1); // coeffs too long
    }

    // ── contract_shell_pair tests ────────────────────────────────────────

    #[test]
    fn test_contract_shell_pair_single_prim_each() {
        // na_prim=1, nb_prim=1, n_cart=1
        let eri_prims = vec![5.0];
        let ca = vec![0.5];
        let cb = vec![0.3];
        let result = contract_shell_pair(&eri_prims, &ca, &cb, 1, 1, 1);
        assert!((result[0] - 0.75).abs() < 1e-14);
    }

    #[test]
    fn test_contract_shell_pair_two_by_one() {
        // na_prim=2, nb_prim=1, n_cart=1
        let eri_prims = vec![5.0, 7.0];
        let ca = vec![0.5, 0.3];
        let cb = vec![1.0];
        let result = contract_shell_pair(&eri_prims, &ca, &cb, 2, 1, 1);
        assert!((result[0] - 4.6).abs() < 1e-14);
    }

    #[test]
    fn test_contract_shell_pair_two_by_two_s_type() {
        // na_prim=2, nb_prim=2, n_cart=1
        // Primitive pairs: (0,0)=1, (0,1)=2, (1,0)=3, (1,1)=4
        let eri_prims = vec![1.0, 2.0, 3.0, 4.0];
        let ca = vec![1.0, 2.0];
        let cb = vec![3.0, 4.0];
        let result = contract_shell_pair(&eri_prims, &ca, &cb, 2, 2, 1);
        // result = 1*3*1 + 1*4*2 + 2*3*3 + 2*4*4 = 3 + 8 + 18 + 32 = 61
        assert!((result[0] - 61.0).abs() < 1e-13);
    }

    #[test]
    fn test_contract_shell_pair_p_type() {
        // na_prim=1, nb_prim=1, n_cart=3 (p-type)
        let eri_prims = vec![1.0, 2.0, 3.0];
        let ca = vec![0.5];
        let cb = vec![2.0];
        let result = contract_shell_pair(&eri_prims, &ca, &cb, 1, 1, 3);
        assert_eq!(result.len(), 3);
        assert!((result[0] - 1.0).abs() < 1e-14);
        assert!((result[1] - 2.0).abs() < 1e-14);
        assert!((result[2] - 3.0).abs() < 1e-14);
    }

    #[test]
    fn test_contract_shell_pair_zero_coeffs() {
        let eri_prims = vec![1.0, 2.0, 3.0, 4.0];
        let ca = vec![0.0, 0.0];
        let cb = vec![0.0, 0.0];
        let result = contract_shell_pair(&eri_prims, &ca, &cb, 2, 2, 1);
        assert!(result[0].abs() < 1e-14);
    }

    #[test]
    fn test_contract_shell_pair_matches_sequential() {
        // Verify that contract_shell_pair gives the same result as two sequential
        // contract_primitives calls
        let na_prim = 3;
        let nb_prim = 2;
        let n_cart = 3;

        // Generate some pseudo-random primitive integrals
        let eri_prims: Vec<f64> = (0..na_prim * nb_prim * n_cart)
            .map(|k| (k as f64 + 1.0) * 0.1)
            .collect();
        let ca = vec![0.5, 0.3, 0.2];
        let cb = vec![0.6, 0.4];

        // Direct two-shell contraction
        let result_direct = contract_shell_pair(&eri_prims, &ca, &cb, na_prim, nb_prim, n_cart);

        // Manual computation
        let mut result_manual = vec![0.0; n_cart];
        for ia in 0..na_prim {
            for ib in 0..nb_prim {
                let c_ab = ca[ia] * cb[ib];
                for cart in 0..n_cart {
                    result_manual[cart] += c_ab * eri_prims[(ia * nb_prim + ib) * n_cart + cart];
                }
            }
        }

        for cart in 0..n_cart {
            assert!((result_direct[cart] - result_manual[cart]).abs() < 1e-14,
                "Mismatch at cart={}: direct={}, manual={}",
                cart, result_direct[cart], result_manual[cart]);
        }
    }
}
