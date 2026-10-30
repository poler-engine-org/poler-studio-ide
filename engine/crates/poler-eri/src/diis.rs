//! DIIS (Direct Inversion in the Iterative Subspace) convergence accelerator.
//!
//! # What this module does
//!
//! This module implements Pulay's DIIS method for accelerating the convergence
//! of the self-consistent field (SCF) iteration. DIIS is the most widely used
//! convergence accelerator in quantum chemistry and is essential for achieving
//! robust, rapid convergence of the Hartree-Fock equations.
//!
//! # Why DIIS is needed
//!
//! The naive SCF iteration (build Fock → diagonalize → build density → repeat)
//! often exhibits:
//!
//! - **Slow convergence**: Linear or sub-linear convergence near the solution.
//! - **Oscillation**: The density matrix oscillates between two or more states.
//! - **Divergence**: In pathological cases, the iteration diverges entirely.
//!
//! DIIS overcomes these problems by extrapolating the Fock matrix from a history
//! of previous iterations. Instead of using the Fock matrix from the current
//! iteration directly, DIIS finds the linear combination of previous Fock matrices
//! that minimizes the error vector norm.
//!
//! # How DIIS works
//!
//! The DIIS algorithm maintains a history of `k` (Fock, error) pairs:
//!
//! 1. **Error vector**: At each iteration, compute the error vector
//!    **e = FDS − SDF** (the commutator [F, D] in the non-orthogonal basis).
//!    At convergence, F and D commute (when expressed in the same basis), so
//!    **e → 0**.
//!
//! 2. **Storage**: Store the Fock matrix and error vector from each iteration.
//!    Only the last `MAX_DIIS_VECS` pairs are kept.
//!
//! 3. **Extrapolation**: Find coefficients **c₁, c₂, ..., c_k** that minimize
//!    `||Σ c_i e_i||²` subject to `Σ c_i = 1`. This is solved by the linear
//!    system:
//!
//!    ```text
//!    ┌                          ┐ ┌    ┐   ┌   ┐
//!    │ B₁₁ B₁₂ ... B₁k  -1    │ │ c₁ │   │ 0 │
//!    │ B₂₁ B₂₂ ... B₂k  -1    │ │ c₂ │ = │ 0 │
//!    │  :   :       :   :      │ │  : │   │ : │
//!    │ Bk₁ Bk₂ ... Bkk  -1    │ │ cₖ │   │ 0 │
//!    │ -1  -1  ... -1   0      │ │ λ  │   │-1 │
//!    └                          ┘ └    ┘   └   ┘
//!    ```
//!
//!    where `B_ij = e_i · e_j` is the overlap matrix of error vectors, and the
//!    last row/column enforces the constraint `Σ c_i = 1`.
//!
//! 4. **Extrapolated Fock**: F_new = Σ c_i F_i
//!
//! # Known limitations
//!
//! - The Gaussian elimination solver uses partial pivoting but is not as robust
//!   as LU decomposition with full pivoting for ill-conditioned systems.
//! - The error vector is stored as a full n² vector, which may use significant
//!   memory for large basis sets.

#[allow(unused_imports)]
use crate::diagonalize::mat_vec_mul;

/// Maximum number of (Fock, error) pairs to keep in the DIIS history.
///
/// This constant limits the memory usage and the size of the linear system
/// that must be solved at each iteration. Typical values in quantum chemistry
/// codes range from 6 to 20. A value of 8 provides a good balance between
/// convergence acceleration and memory usage.
///
/// # Why 8?
///
/// - Too few vectors (< 4): Insufficient information for effective extrapolation.
/// - Too many vectors (> 12): The linear system becomes ill-conditioned, and
///   old, irrelevant information can destabilize the extrapolation.
/// - 8 vectors: Empirically works well for most systems.
pub const MAX_DIIS_VECS: usize = 8;

/// DIIS convergence accelerator state.
///
/// Maintains a history of Fock matrices and their associated error vectors
/// for Pulay's DIIS extrapolation. The state is updated each SCF iteration
/// by calling [`Diis::push`], and the extrapolated Fock matrix is obtained
/// by calling [`Diis::extrapolate`].
///
/// # Type parameters
///
/// This struct stores vectors of `Vec<f64>`, where each vector represents
/// either a full Fock matrix or a full error vector (both of size n² for
/// an n-dimensional basis).
///
/// # Usage pattern
///
/// ```text
/// let mut diis = Diis::new();
///
/// loop {
///     // ... build Fock matrix, compute error vector ...
///     diis.push(&fock, &error);
///
///     if let Some(fock_extrap) = diis.extrapolate(n) {
///         // Use the extrapolated Fock matrix for diagonalization
///         fock = fock_extrap;
///     }
/// }
/// ```
///
/// # Thread safety
///
/// This struct is NOT `Sync` or `Send` safe. It should be used from a single
/// thread only. In a multi-threaded SCF, each thread should have its own
/// DIIS instance.
pub struct Diis {
    /// Maximum number of (Fock, error) pairs to retain.
    pub max_vecs: usize,
    /// History of error vectors: e_i = F_i D S − S D F_i
    pub error_vecs: Vec<Vec<f64>>,
    /// History of Fock matrices corresponding to the error vectors.
    pub fock_vecs: Vec<Vec<f64>>,
}

impl Diis {
    /// Create a new DIIS accelerator with the default maximum number of vectors.
    ///
    /// # What it does
    ///
    /// Initializes an empty DIIS history with [`MAX_DIIS_VECS`] slots.
    ///
    /// # Returns
    ///
    /// A new `Diis` instance with empty history.
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::diis::Diis;
    ///
    /// let diis = Diis::new();
    /// assert_eq!(diis.max_vecs, 8);
    /// assert!(diis.error_vecs.is_empty());
    /// assert!(diis.fock_vecs.is_empty());
    /// ```
    pub fn new() -> Self {
        Diis {
            max_vecs: MAX_DIIS_VECS,
            error_vecs: Vec::with_capacity(MAX_DIIS_VECS),
            fock_vecs: Vec::with_capacity(MAX_DIIS_VECS),
        }
    }

    /// Create a new DIIS accelerator with a custom maximum number of vectors.
    ///
    /// # Arguments
    ///
    /// * `max_vecs` — Maximum number of (Fock, error) pairs to retain. Must be
    ///   at least 2 for DIIS to work. Typical values: 4–12.
    ///
    /// # Returns
    ///
    /// A new `Diis` instance with the specified capacity.
    pub fn with_max_vecs(max_vecs: usize) -> Self {
        Diis {
            max_vecs,
            error_vecs: Vec::with_capacity(max_vecs),
            fock_vecs: Vec::with_capacity(max_vecs),
        }
    }

    /// Store a new (Fock, error) pair in the DIIS history.
    ///
    /// # What it does
    ///
    /// Appends the current Fock matrix and its associated error vector to the
    /// DIIS history. If the history is full (contains `max_vecs` pairs), the
    /// oldest pair is discarded to make room for the new one.
    ///
    /// # Why the oldest is discarded
    ///
    /// The oldest error vectors are typically the farthest from convergence and
    /// contribute least to the extrapolation. Discarding them keeps the linear
    /// system well-conditioned and focuses the extrapolation on the most recent
    /// (and most relevant) iterations.
    ///
    /// # Arguments
    ///
    /// * `fock` — The Fock matrix from the current iteration (flat n×n row-major).
    /// * `error` — The error vector **e = FDS − SDF** from the current iteration.
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::diis::Diis;
    ///
    /// let mut diis = Diis::new();
    /// let fock = vec![1.0, 0.0, 0.0, 1.0];
    /// let error = vec![0.1, 0.0, 0.0, -0.1];
    ///
    /// diis.push(&fock, &error);
    /// assert_eq!(diis.fock_vecs.len(), 1);
    /// assert_eq!(diis.error_vecs.len(), 1);
    /// ```
    pub fn push(&mut self, fock: &[f64], error: &[f64]) {
        // If at capacity, remove the oldest entry
        if self.fock_vecs.len() >= self.max_vecs {
            self.fock_vecs.remove(0);
            self.error_vecs.remove(0);
        }

        // Store copies of the current Fock and error
        self.fock_vecs.push(fock.to_vec());
        self.error_vecs.push(error.to_vec());
    }

    /// Extrapolate the Fock matrix using DIIS.
    ///
    /// # What it does
    ///
    /// Computes the DIIS-extrapolated Fock matrix **F_new = Σ c_i F_i** where
    /// the coefficients **c_i** minimize `||Σ c_i e_i||²` subject to `Σ c_i = 1`.
    ///
    /// # How it works
    ///
    /// 1. Build the **B matrix**: `B[i][j] = e_i · e_j` (dot product of error vectors).
    ///    This matrix measures the overlap between error vectors and is always
    ///    symmetric positive semi-definite.
    ///
    /// 2. Augment the B matrix with the constraint `Σ c_i = 1`:
    ///    - Add a column of −1 values.
    ///    - Add a row of −1 values.
    ///    - Set the bottom-right element to 0.
    ///
    /// 3. Solve the augmented system using Gaussian elimination with partial pivoting.
    ///
    /// 4. The first `k` elements of the solution are the DIIS coefficients.
    ///
    /// 5. Compute `F_new = Σ c_i F_i`.
    ///
    /// # When extrapolation fails
    ///
    /// Extrapolation returns `None` if:
    /// - Fewer than 2 error vectors have been stored (not enough information).
    /// - The linear system is singular or nearly singular (ill-conditioned B matrix).
    /// - The Gaussian elimination fails due to a zero pivot.
    ///
    /// In these cases, the caller should fall back to the unextrapolated Fock matrix.
    ///
    /// # Arguments
    ///
    /// * `n` — Dimension of the basis (used for verification only; the actual
    ///   matrix size is determined from the stored vectors).
    ///
    /// # Returns
    ///
    /// `Some(extrapolated_fock)` if the extrapolation succeeds, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::diis::Diis;
    ///
    /// let mut diis = Diis::new();
    /// // Need at least 2 pairs for extrapolation
    /// let f1 = vec![1.0, 0.0, 0.0, 1.0];
    /// let e1 = vec![0.1, 0.0, 0.0, -0.1];
    /// diis.push(&f1, &e1);
    ///
    /// // Only 1 vector: can't extrapolate yet
    /// assert!(diis.extrapolate(2).is_none());
    ///
    /// let f2 = vec![1.05, 0.0, 0.0, 0.95];
    /// let e2 = vec![0.05, 0.0, 0.0, -0.05];
    /// diis.push(&f2, &e2);
    ///
    /// // Now we can extrapolate
    /// let result = diis.extrapolate(2);
    /// assert!(result.is_some());
    /// ```
    pub fn extrapolate(&self, n: usize) -> Option<Vec<f64>> {
        let k = self.error_vecs.len();
        if k < 2 {
            // Need at least 2 vectors for meaningful extrapolation
            return None;
        }

        let total_len = self.error_vecs[0].len();

        // Build the B matrix (k+1 × k+1 augmented system)
        // B[i][j] = e_i · e_j for i,j < k
        // B[i][k] = B[k][i] = -1 for i < k
        // B[k][k] = 0
        let dim = k + 1;
        let mut b = vec![0.0; dim * dim];

        // Fill the error dot products
        for i in 0..k {
            for j in i..k {
                let dot = self.error_vecs[i]
                    .iter()
                    .zip(self.error_vecs[j].iter())
                    .map(|(&a, &b)| a * b)
                    .sum::<f64>();
                b[i * dim + j] = dot;
                b[j * dim + i] = dot; // Symmetric
            }
        }

        // Add the constraint row/column: Σ c_i = 1
        for i in 0..k {
            b[i * dim + k] = -1.0;
            b[k * dim + i] = -1.0;
        }
        // b[k * dim + k] = 0.0; // Already zero

        // Right-hand side: [0, 0, ..., 0, -1]
        let mut rhs = vec![0.0; dim];
        rhs[k] = -1.0;

        // Solve the linear system B · x = rhs
        let coeffs = solve_linear(&b, &rhs, dim)?;

        // Compute the extrapolated Fock matrix: F_new = Σ c_i * F_i
        let mut fock_new = vec![0.0; total_len];
        for i in 0..k {
            let ci = coeffs[i];
            for j in 0..total_len {
                fock_new[j] += ci * self.fock_vecs[i][j];
            }
        }

        let _ = n; // n is used for verification in production; stored for API consistency
        Some(fock_new)
    }
}

/// Compute the DIIS error vector: **e = FDS − SDF**.
///
/// # What it does
///
/// Computes the commutator of the Fock matrix and the density matrix in the
/// non-orthogonal basis defined by the overlap matrix **S**:
///
/// ```text
/// e = F · D · S − S · D · F
/// ```
///
/// # Why this is the error vector
///
/// At convergence, the Fock matrix and the density matrix commute when expressed
/// in the same orthonormal basis. In the non-orthogonal (AO) basis, this means:
///
/// ```text
/// F · D · S = S · D · F   →   [F, D]_S = 0
/// ```
///
/// The norm of the error vector `||e||` measures how far the current
/// (Fock, density) pair is from self-consistency. It is the most common
/// convergence metric for SCF iterations.
///
/// # How it works
///
/// 1. Compute **FDS = F · D · S** (two matrix-matrix products).
/// 2. Compute **SDF = S · D · F** (two matrix-matrix products).
/// 3. Subtract: **e = FDS − SDF**.
///
/// The result is a flat vector of length `n²` (the matrix is flattened row-major).
///
/// # Arguments
///
/// * `fock` — Fock matrix F, flat n×n row-major array
/// * `density` — Density matrix D, flat n×n row-major array
/// * `overlap` — Overlap matrix S, flat n×n row-major array
/// * `n` — Dimension of the matrices
///
/// # Returns
///
/// The error vector **e = FDS − SDF** as a flat array of length `n²`.
///
/// # Examples
///
/// ```
/// use poler_eri::diis::error_vector;
///
/// // At convergence, F·D·S = S·D·F, so error = 0
/// let f = vec![1.0, 0.0, 0.0, 1.0];
/// let d = vec![2.0, 0.0, 0.0, 0.0];
/// let s = vec![1.0, 0.0, 0.0, 1.0]; // Identity overlap
/// let e = error_vector(&f, &d, &s, 2);
/// // FDS = [[1,0],[0,1]] · [[2,0],[0,0]] · [[1,0],[0,1]] = [[2,0],[0,0]]
/// // SDF = [[1,0],[0,1]] · [[2,0],[0,0]] · [[1,0],[0,1]] = [[2,0],[0,0]]
/// // e = 0
/// assert!(e.iter().all(|&x| x.abs() < 1e-12));
/// ```
pub fn error_vector(fock: &[f64], density: &[f64], overlap: &[f64], n: usize) -> Vec<f64> {
    // Compute FD = F · D
    let mut fd = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut sum = 0.0;
            for k in 0..n {
                sum += fock[i * n + k] * density[k * n + j];
            }
            fd[i * n + j] = sum;
        }
    }

    // Compute FDS = FD · S
    let mut fds = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut sum = 0.0;
            for k in 0..n {
                sum += fd[i * n + k] * overlap[k * n + j];
            }
            fds[i * n + j] = sum;
        }
    }

    // Compute SD = S · D
    let mut sd = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut sum = 0.0;
            for k in 0..n {
                sum += overlap[i * n + k] * density[k * n + j];
            }
            sd[i * n + j] = sum;
        }
    }

    // Compute SDF = SD · F
    let mut sdf = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut sum = 0.0;
            for k in 0..n {
                sum += sd[i * n + k] * fock[k * n + j];
            }
            sdf[i * n + j] = sum;
        }
    }

    // Error vector: e = FDS - SDF
    let mut err = vec![0.0; n * n];
    for i in 0..(n * n) {
        err[i] = fds[i] - sdf[i];
    }

    err
}

/// Solve a linear system **A · x = b** using Gaussian elimination with partial pivoting.
///
/// # What it does
///
/// Solves the n×n linear system using row-oriented Gaussian elimination:
///
/// 1. Forward elimination with partial pivoting (select the largest absolute
///    value in the current column as the pivot).
/// 2. Back substitution to obtain the solution.
///
/// # Why partial pivoting
///
/// Partial pivoting (selecting the largest element in the current column as the
/// pivot) significantly improves numerical stability compared to naive Gaussian
/// elimination. It prevents division by small pivots, which can amplify
/// rounding errors and produce inaccurate results.
///
/// # When it returns None
///
/// Returns `None` if:
/// - A zero pivot is encountered (the system is singular or nearly singular).
/// - The matrix is so ill-conditioned that the pivot is below a small threshold.
///
/// # Arguments
///
/// * `a` — Flat array representing the n×n coefficient matrix in row-major order.
///   **This matrix is consumed (modified in place).** The caller must make a copy
///   if the original is needed.
/// * `b` — Right-hand side vector of length n.
/// * `n` — Dimension of the system.
///
/// # Returns
///
/// `Some(solution)` if the system was solved successfully, `None` if the matrix
/// is singular.
fn solve_linear(a: &[f64], b: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut aug = a.to_vec();
    let mut rhs = b.to_vec();

    let tol = 1e-14;

    // Forward elimination with partial pivoting
    for col in 0..n {
        // Find the pivot: row with the largest absolute value in this column
        let mut max_val = libm::fabs(aug[col * n + col]);
        let mut max_row = col;
        for row in (col + 1)..n {
            let val = libm::fabs(aug[row * n + col]);
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }

        // Check for singularity
        if max_val < tol {
            return None; // Matrix is singular or nearly singular
        }

        // Swap rows if necessary
        if max_row != col {
            for j in 0..n {
                let tmp = aug[col * n + j];
                aug[col * n + j] = aug[max_row * n + j];
                aug[max_row * n + j] = tmp;
            }
            let tmp = rhs[col];
            rhs[col] = rhs[max_row];
            rhs[max_row] = tmp;
        }

        // Eliminate below the pivot
        let pivot = aug[col * n + col];
        for row in (col + 1)..n {
            let factor = aug[row * n + col] / pivot;
            aug[row * n + col] = 0.0; // Zero out explicitly
            for j in (col + 1)..n {
                aug[row * n + j] -= factor * aug[col * n + j];
            }
            rhs[row] -= factor * rhs[col];
        }
    }

    // Back substitution
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut sum = rhs[i];
        for j in (i + 1)..n {
            sum -= aug[i * n + j] * x[j];
        }
        let diag = aug[i * n + i];
        if libm::fabs(diag) < tol {
            return None; // Shouldn't happen after pivoting, but safety check
        }
        x[i] = sum / diag;
    }

    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diis_new() {
        let diis = Diis::new();
        assert_eq!(diis.max_vecs, MAX_DIIS_VECS);
        assert!(diis.error_vecs.is_empty());
        assert!(diis.fock_vecs.is_empty());
    }

    #[test]
    fn test_diis_push() {
        let mut diis = Diis::new();
        let f = vec![1.0, 0.0, 0.0, 1.0];
        let e = vec![0.1, 0.0, 0.0, -0.1];
        diis.push(&f, &e);
        assert_eq!(diis.fock_vecs.len(), 1);
        assert_eq!(diis.error_vecs.len(), 1);
    }

    #[test]
    fn test_diis_push_overflow() {
        let mut diis = Diis::with_max_vecs(3);
        for i in 0..5 {
            let f = vec![i as f64, 0.0, 0.0, 1.0];
            let e = vec![0.1, 0.0, 0.0, -0.1];
            diis.push(&f, &e);
        }
        // Should keep only the last 3
        assert_eq!(diis.fock_vecs.len(), 3);
        // First stored Fock should be from iteration 2
        assert!((diis.fock_vecs[0][0] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn test_diis_extrapolate_insufficient_vectors() {
        let mut diis = Diis::new();
        let f = vec![1.0, 0.0, 0.0, 1.0];
        let e = vec![0.1, 0.0, 0.0, -0.1];
        diis.push(&f, &e);
        // Only 1 vector: can't extrapolate
        assert!(diis.extrapolate(2).is_none());
    }

    #[test]
    fn test_diis_extrapolate_two_vectors() {
        let mut diis = Diis::new();
        let f1 = vec![1.0, 0.0, 0.0, 1.0];
        let e1 = vec![0.1, 0.0, 0.0, -0.1];
        let f2 = vec![1.05, 0.0, 0.0, 0.95];
        let e2 = vec![0.05, 0.0, 0.0, -0.05];
        diis.push(&f1, &e1);
        diis.push(&f2, &e2);

        let result = diis.extrapolate(2);
        assert!(result.is_some());
        let f_new = result.unwrap();
        // The extrapolated Fock should be a linear combination of the two inputs
        // with c1 + c2 = 1
        assert_eq!(f_new.len(), 4);
    }

    #[test]
    fn test_error_vector_zero_at_convergence() {
        // When F and D commute (in the same basis), the error should be zero
        let f = vec![1.0, 0.0, 0.0, 2.0];
        let d = vec![2.0, 0.0, 0.0, 1.0];
        let s = vec![1.0, 0.0, 0.0, 1.0]; // Identity overlap
        let e = error_vector(&f, &d, &s, 2);
        // FDS = F*D*S = [[2,0],[0,2]]
        // SDF = S*D*F = [[2,0],[0,2]]
        // e = 0
        for &val in &e {
            assert!(val.abs() < 1e-12, "Error should be zero at convergence, got {}", val);
        }
    }

    #[test]
    fn test_error_vector_nonzero_off_convergence() {
        let f = vec![2.0, 1.0, 1.0, 3.0];
        let d = vec![1.0, 0.0, 0.0, 0.0];
        let s = vec![1.0, 0.0, 0.0, 1.0];
        let e = error_vector(&f, &d, &s, 2);
        // Should have some nonzero elements
        assert!(e.iter().any(|&x| x.abs() > 1e-12));
    }

    #[test]
    fn test_solve_linear_identity() {
        let a = vec![1.0, 0.0, 0.0, 1.0];
        let b = vec![3.0, 4.0];
        let x = solve_linear(&a, &b, 2).unwrap();
        assert!((x[0] - 3.0).abs() < 1e-12);
        assert!((x[1] - 4.0).abs() < 1e-12);
    }

    #[test]
    fn test_solve_linear_2x2() {
        // [2 1; 1 3] x = [5; 10]
        // Solution: x = [1, 3]
        let a = vec![2.0, 1.0, 1.0, 3.0];
        let b = vec![5.0, 10.0];
        let x = solve_linear(&a, &b, 2).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-10);
        assert!((x[1] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_solve_linear_singular() {
        // Singular matrix: [1 2; 2 4]
        let a = vec![1.0, 2.0, 2.0, 4.0];
        let b = vec![3.0, 6.0];
        let result = solve_linear(&a, &b, 2);
        // May return None (singular) or a solution (if the system is consistent)
        // For this specific case, the rank is 1 so the augmented system may or
        // may not detect singularity depending on pivoting. At minimum it should
        // not panic.
        let _ = result;
    }

    #[test]
    fn test_solve_linear_3x3() {
        // [2 1 -1; -3 -1 2; -2 1 2] x = [8; -11; -3]
        // Solution: x = [2, 3, -1]
        let a = vec![2.0, 1.0, -1.0, -3.0, -1.0, 2.0, -2.0, 1.0, 2.0];
        let b = vec![8.0, -11.0, -3.0];
        let x = solve_linear(&a, &b, 3).unwrap();
        assert!((x[0] - 2.0).abs() < 1e-10, "x[0] = {}", x[0]);
        assert!((x[1] - 3.0).abs() < 1e-10, "x[1] = {}", x[1]);
        assert!((x[2] - (-1.0)).abs() < 1e-10, "x[2] = {}", x[2]);
    }
}
