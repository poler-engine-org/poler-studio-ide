//! Matrix diagonalization and linear algebra utilities for the SCF procedure.
//!
//! # What this module does
//!
//! This module provides core linear algebra routines needed by the self-consistent
//! field (SCF) iteration:
//!
//! - **Symmetric matrix diagonalization** via the Jacobi eigenvalue algorithm
//! - **Matrix inverse square root** (S^{-1/2}) for basis set orthogonalization
//! - **Matrix-vector multiplication** used in DIIS extrapolation
//!
//! # Why this module exists
//!
//! The Roothaan-Hall equation **F C = S C ε** requires:
//!
//! 1. Orthogonalization of the overlap matrix **S** to form **S^{-1/2}**, so that
//!    the generalized eigenvalue problem can be transformed into a standard one:
//!    **F' C' = C' ε** where **F' = S^{-1/2T} F S^{-1/2}** and **C = S^{-1/2} C'**.
//! 2. Diagonalization of the transformed Fock matrix to obtain molecular orbital
//!    coefficients **C** and orbital energies **ε**.
//! 3. Matrix-vector products for DIIS error vector computation and extrapolation.
//!
//! # Implementation notes
//!
//! The Jacobi algorithm is chosen for its simplicity and guaranteed convergence
//! for symmetric matrices. It works by iteratively applying plane rotations to
//! zero out off-diagonal elements. While not the fastest method (O(n³) per sweep,
//! typically 5-10 sweeps), it is:
//!
//! - Simple to implement correctly
//! - Guaranteed to converge for any symmetric matrix
//! - Produces orthogonal eigenvectors by construction
//! - Sufficient for the moderate basis set sizes (< 1000 basis functions)
//!   used with this code

use libm::sqrt;

/// Multiply a symmetric matrix (stored row-major) by a vector.
///
/// # What it does
///
/// Computes **y = A · x** where A is an n×n matrix stored as a flat array
/// in row-major order: `A[i][j] = a[i * n + j]`.
///
/// # Why it exists
///
/// This operation is used by the DIIS module to compute error vectors
/// (F·D·S − S·D·F) and by the SCF module for Fock matrix transformations.
/// A dedicated function avoids repeated boilerplate and ensures consistent
/// indexing.
///
/// # Arguments
///
/// * `a` — Flat array representing the n×n matrix A in row-major order.
///   Must have length exactly `n * n`.
/// * `x` — Input vector of length `n`.
/// * `n` — Dimension of the matrix and vectors.
///
/// # Returns
///
/// The product vector **y = A · x** of length `n`.
///
/// # Panics
///
/// Panics if `a.len() != n * n` or `x.len() != n`.
///
/// # Examples
///
/// ```
/// use poler_eri::diagonalize::mat_vec_mul;
///
/// // Identity matrix times vector = same vector
/// let eye = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
/// let v = vec![1.0, 2.0, 3.0];
/// let y = mat_vec_mul(&eye, &v, 3);
/// assert!((y[0] - 1.0).abs() < 1e-12);
/// assert!((y[1] - 2.0).abs() < 1e-12);
/// assert!((y[2] - 3.0).abs() < 1e-12);
/// ```
pub fn mat_vec_mul(a: &[f64], x: &[f64], n: usize) -> Vec<f64> {
    assert_eq!(a.len(), n * n, "Matrix must have n*n elements");
    assert_eq!(x.len(), n, "Vector must have n elements");
    let mut y = vec![0.0; n];
    for i in 0..n {
        let mut sum = 0.0;
        for j in 0..n {
            sum += a[i * n + j] * x[j];
        }
        y[i] = sum;
    }
    y
}

/// Diagonalize a real symmetric matrix using the Jacobi eigenvalue algorithm.
///
/// # What it does
///
/// Given a real symmetric matrix **A**, finds all eigenvalues and eigenvectors
/// such that **A · V = V · Λ**, where Λ is diagonal (eigenvalues) and V is
/// orthogonal (eigenvectors as columns).
///
/// # Why Jacobi
///
/// The Jacobi method is simple, robust, and guaranteed to converge for any
/// real symmetric matrix. It works by iteratively applying Givens rotations
/// to zero out the largest off-diagonal element until the matrix is diagonal.
///
/// For typical quantum chemistry problems (< 1000 basis functions), convergence
/// is reached in 5-10 sweeps.
///
/// # How it works
///
/// 1. Initialize eigenvectors to the identity matrix.
/// 2. Find the off-diagonal element with the largest absolute value.
/// 3. Compute a Givens rotation angle θ that zeros this element:
///    - τ = (a_jj − a_ii) / (2 · a_ij)
///    - t = sign(τ) / (|τ| + √(1 + τ²))
///    - c = 1/√(1 + t²), s = t · c
/// 4. Apply the rotation to the matrix and accumulate in the eigenvector matrix.
/// 5. Repeat until the off-diagonal norm is below tolerance.
///
/// # Arguments
///
/// * `mat` — Flat array representing the n×n symmetric matrix in row-major order.
///   The matrix is **not** modified (a copy is used internally).
/// * `n` — Dimension of the matrix.
///
/// # Returns
///
/// A tuple `(eigenvalues, eigenvectors)` where:
/// - `eigenvalues` is a `Vec<f64>` of length `n` (sorted in ascending order)
/// - `eigenvectors` is a `Vec<f64>` of length `n * n` in row-major order,
///   where column j is the eigenvector corresponding to eigenvalue j
///
/// # Panics
///
/// Panics if `mat.len() != n * n`.
///
/// # Examples
///
/// ```
/// use poler_eri::diagonalize::jacobi_eigen;
///
/// // Diagonal matrix: eigenvalues are already on the diagonal
/// let mat = vec![2.0, 0.0, 0.0, 5.0]; // 2x2 diagonal
/// let (vals, vecs) = jacobi_eigen(&mat, 2);
/// assert!((vals[0] - 2.0).abs() < 1e-8);
/// assert!((vals[1] - 5.0).abs() < 1e-8);
/// ```
pub fn jacobi_eigen(mat: &[f64], n: usize) -> (Vec<f64>, Vec<f64>) {
    assert_eq!(mat.len(), n * n, "Matrix must have n*n elements");

    // Work on a copy of the matrix
    let mut a = mat.to_vec();

    // Eigenvector matrix, initialized to identity
    let mut v = vec![0.0; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }

    let max_sweeps = 100;
    let tol = 1e-12;

    for _sweep in 0..max_sweeps {
        // Find the off-diagonal element with the largest absolute value
        let mut max_val: f64 = 0.0;
        let mut p = 0usize;
        let mut q = 1usize;
        for i in 0..n {
            for j in (i + 1)..n {
                let val = libm::fabs(a[i * n + j]);
                if val > max_val {
                    max_val = val;
                    p = i;
                    q = j;
                }
            }
        }

        // Check convergence
        if max_val < tol {
            break;
        }

        // Compute the Jacobi rotation to zero out a[p][q]
        let app = a[p * n + p];
        let aqq = a[q * n + q];
        let apq = a[p * n + q];

        let tau: f64;
        if libm::fabs(apq) < 1e-30 {
            continue;
        }

        let diff = aqq - app;
        if libm::fabs(diff) < 1e-30 {
            tau = 0.0; // a_pp ≈ a_qq, rotation by π/4
        } else {
            tau = diff / (2.0 * apq);
        }

        // t = sign(τ) / (|τ| + sqrt(1 + τ²))
        let t = if tau >= 0.0 {
            1.0 / (tau + sqrt(1.0 + tau * tau))
        } else {
            -1.0 / (-tau + sqrt(1.0 + tau * tau))
        };

        let c = 1.0 / sqrt(1.0 + t * t);
        let s = t * c;

        // Update the matrix: apply the rotation to rows/columns p and q
        // First, update the diagonal elements
        a[p * n + p] = app - t * apq;
        a[q * n + q] = aqq + t * apq;
        a[p * n + q] = 0.0;
        a[q * n + p] = 0.0;

        // Update off-diagonal elements in rows/columns p and q
        for r in 0..n {
            if r == p || r == q {
                continue;
            }
            let arp = a[r * n + p];
            let arq = a[r * n + q];
            a[r * n + p] = c * arp - s * arq;
            a[p * n + r] = a[r * n + p];
            a[r * n + q] = s * arp + c * arq;
            a[q * n + r] = a[r * n + q];
        }

        // Update the eigenvector matrix
        for r in 0..n {
            let vrp = v[r * n + p];
            let vrq = v[r * n + q];
            v[r * n + p] = c * vrp - s * vrq;
            v[r * n + q] = s * vrp + c * vrq;
        }
    }

    // Extract eigenvalues from the diagonal
    let mut eigenvalues = Vec::with_capacity(n);
    for i in 0..n {
        eigenvalues.push(a[i * n + i]);
    }

    // Sort eigenvalues (and corresponding eigenvectors) in ascending order
    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_by(|&i, &j| {
        eigenvalues[i]
            .partial_cmp(&eigenvalues[j])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut sorted_vals = vec![0.0; n];
    let mut sorted_vecs = vec![0.0; n * n];
    for (new_idx, &old_idx) in indices.iter().enumerate() {
        sorted_vals[new_idx] = eigenvalues[old_idx];
        // Copy column old_idx to column new_idx
        for r in 0..n {
            sorted_vecs[r * n + new_idx] = v[r * n + old_idx];
        }
    }

    (sorted_vals, sorted_vecs)
}

/// Compute the inverse square root of a symmetric positive-definite matrix.
///
/// # What it does
///
/// Given a symmetric positive-definite matrix **S** (e.g., the overlap matrix),
/// computes **S^{-1/2}** such that **S^{-1/2} · S · S^{-1/2} = I**.
///
/// # Why it exists
///
/// The Löwdin symmetric orthogonalization transforms the generalized eigenvalue
/// problem **F C = S C ε** into a standard one by:
///
/// 1. Computing **S^{-1/2}** from the eigendecomposition of **S**.
/// 2. Transforming: **F' = S^{-1/2T} · F · S^{-1/2}** (symmetric because S^{-1/2}
///    is symmetric for symmetric S).
/// 3. Solving the standard eigenvalue problem **F' C' = C' ε**.
/// 4. Back-transforming: **C = S^{-1/2} · C'**.
///
/// # How it works
///
/// Given the eigendecomposition **S = U · Λ · U^T**:
///
/// ```text
/// S^{-1/2} = U · Λ^{-1/2} · U^T
/// ```
///
/// where Λ^{-1/2} means taking the reciprocal square root of each eigenvalue.
/// This is valid because S is symmetric positive-definite, so all eigenvalues
/// are positive.
///
/// # Arguments
///
/// * `mat` — Flat array representing the n×n symmetric positive-definite matrix
///   in row-major order.
/// * `n` — Dimension of the matrix.
///
/// # Returns
///
/// The matrix **S^{-1/2}** as a flat array of length `n * n` in row-major order.
///
/// # Panics
///
/// Panics if any eigenvalue is non-positive (indicating the matrix is not
/// positive-definite).
///
/// # Examples
///
/// ```
/// use poler_eri::diagonalize::inv_sqrt_matrix;
///
/// // Identity matrix: S^{-1/2} = I
/// let eye = vec![1.0, 0.0, 0.0, 1.0];
/// let result = inv_sqrt_matrix(&eye, 2);
/// assert!((result[0] - 1.0).abs() < 1e-10);
/// assert!((result[3] - 1.0).abs() < 1e-10);
/// ```
pub fn inv_sqrt_matrix(mat: &[f64], n: usize) -> Vec<f64> {
    let (eigenvalues, eigenvectors) = jacobi_eigen(mat, n);

    // Compute Λ^{-1/2} = 1/sqrt(λ_i) for each eigenvalue
    let mut lambda_inv_sqrt = vec![0.0; n];
    for i in 0..n {
        if eigenvalues[i] <= 0.0 {
            panic!(
                "inv_sqrt_matrix: eigenvalue {} is non-positive ({}). \
                 The matrix is not positive-definite.",
                i, eigenvalues[i]
            );
        }
        lambda_inv_sqrt[i] = 1.0 / sqrt(eigenvalues[i]);
    }

    // Compute S^{-1/2} = U · Λ^{-1/2} · U^T
    // Step 1: T = U · Λ^{-1/2}  (scale each column of U by corresponding λ^{-1/2})
    let mut t = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            t[i * n + j] = eigenvectors[i * n + j] * lambda_inv_sqrt[j];
        }
    }

    // Step 2: result = T · U^T
    let mut result = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut sum = 0.0;
            for k in 0..n {
                sum += t[i * n + k] * eigenvectors[j * n + k]; // U^T[k][j] = U[j][k]
            }
            result[i * n + j] = sum;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mat_vec_mul_identity() {
        let eye = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let v = vec![1.0, 2.0, 3.0];
        let y = mat_vec_mul(&eye, &v, 3);
        for i in 0..3 {
            assert!((y[i] - v[i]).abs() < 1e-12, "Identity * v should equal v");
        }
    }

    #[test]
    fn test_mat_vec_mul_diagonal() {
        let d = vec![2.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 5.0];
        let v = vec![1.0, 1.0, 1.0];
        let y = mat_vec_mul(&d, &v, 3);
        assert!((y[0] - 2.0).abs() < 1e-12);
        assert!((y[1] - 3.0).abs() < 1e-12);
        assert!((y[2] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn test_jacobi_diagonal() {
        let mat = vec![2.0, 0.0, 0.0, 5.0];
        let (vals, _) = jacobi_eigen(&mat, 2);
        assert!((vals[0] - 2.0).abs() < 1e-8);
        assert!((vals[1] - 5.0).abs() < 1e-8);
    }

    #[test]
    fn test_jacobi_symmetric() {
        // [1 2; 2 4] has eigenvalues 0 and 5
        let mat = vec![1.0, 2.0, 2.0, 4.0];
        let (vals, vecs) = jacobi_eigen(&mat, 2);
        assert!((vals[0] - 0.0).abs() < 1e-8, "eigenvalue 0 = {}", vals[0]);
        assert!((vals[1] - 5.0).abs() < 1e-8, "eigenvalue 1 = {}", vals[1]);

        // Verify eigenvectors are orthogonal
        let dot = vecs[0] * vecs[2] + vecs[1] * vecs[3];
        assert!(dot.abs() < 1e-8, "Eigenvectors should be orthogonal, dot = {}", dot);
    }

    #[test]
    fn test_jacobi_3x3() {
        // Symmetric 3x3 matrix
        let mat = vec![
            2.0, -1.0, 0.0,
            -1.0, 2.0, -1.0,
            0.0, -1.0, 2.0,
        ];
        let (vals, vecs) = jacobi_eigen(&mat, 3);

        // Known eigenvalues: 2-sqrt(2), 2, 2+sqrt(2)
        let e1 = 2.0 - sqrt(2.0);
        let e2 = 2.0;
        let e3 = 2.0 + sqrt(2.0);
        assert!((vals[0] - e1).abs() < 1e-8, "eigenvalue 0: got {} expected {}", vals[0], e1);
        assert!((vals[1] - e2).abs() < 1e-8, "eigenvalue 1: got {} expected {}", vals[1], e2);
        assert!((vals[2] - e3).abs() < 1e-8, "eigenvalue 2: got {} expected {}", vals[2], e3);

        // Verify A * v = λ * v for each eigenvector
        for k in 0..3 {
            for i in 0..3 {
                let mut av = 0.0;
                for j in 0..3 {
                    av += mat[i * 3 + j] * vecs[j * 3 + k];
                }
                let lv = vals[k] * vecs[i * 3 + k];
                assert!((av - lv).abs() < 1e-6,
                    "A*v != λ*v at i={}, k={}: {} vs {}", i, k, av, lv);
            }
        }
    }

    #[test]
    fn test_inv_sqrt_identity() {
        let eye = vec![1.0, 0.0, 0.0, 1.0];
        let result = inv_sqrt_matrix(&eye, 2);
        assert!((result[0] - 1.0).abs() < 1e-10);
        assert!((result[1] - 0.0).abs() < 1e-10);
        assert!((result[2] - 0.0).abs() < 1e-10);
        assert!((result[3] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_inv_sqrt_verification() {
        // S^{-1/2} * S * S^{-1/2} should equal I
        let s = vec![
            1.0, 0.5, 0.0,
            0.5, 1.0, 0.5,
            0.0, 0.5, 1.0,
        ];
        let sinv_half = inv_sqrt_matrix(&s, 3);

        // Compute sinv_half * S * sinv_half
        let n = 3;
        // temp = sinv_half * S
        let mut temp = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                let mut sum = 0.0;
                for k in 0..n {
                    sum += sinv_half[i * n + k] * s[k * n + j];
                }
                temp[i * n + j] = sum;
            }
        }
        // result = temp * sinv_half
        let mut result = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                let mut sum = 0.0;
                for k in 0..n {
                    sum += temp[i * n + k] * sinv_half[j * n + k];
                }
                result[i * n + j] = sum;
            }
        }

        // Should be identity
        for i in 0..n {
            for j in 0..n {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((result[i * n + j] - expected).abs() < 1e-8,
                    "S^(-1/2)*S*S^(-1/2)[{}][{}] = {}, expected {}", i, j, result[i * n + j], expected);
            }
        }
    }
}
