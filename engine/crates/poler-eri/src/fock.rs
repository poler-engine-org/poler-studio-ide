//! Fock matrix builder: F = H + J − ½ K.
//!
//! # What this module does
//!
//! This module constructs the Fock matrix **F** from the one-electron core
//! Hamiltonian **H_core** and the two-electron Coulomb (**J**) and exchange
//! (**K**) matrices:
//!
//! ```text
//! F_μν = H_core_μν + J_μν − ½ K_μν
//! ```
//!
//! where:
//!
//! - **J_μν** = Σ_{λσ} D_{λσ} (μν | λσ)  — the Coulomb (direct) matrix
//! - **K_μν** = Σ_{λσ} D_{λσ} (μλ | νσ)  — the exchange matrix
//! - **D_{λσ}** is the density matrix from the current SCF iteration
//! - **(μν | λσ)** are the two-electron repulsion integrals
//!
//! # Why this module exists
//!
//! The Fock matrix is the central quantity in the SCF iteration. It represents
//! the effective one-electron Hamiltonian that each electron experiences due to
//! the average field of all other electrons. The Roothaan-Hall equation
//!
//! ```text
//! F C = S C ε
//! ```
//!
//! is solved iteratively: each iteration builds a new Fock matrix from the
//! current density, diagonalizes it to obtain new MO coefficients, and builds
//! a new density from the occupied orbitals.
//!
//! # Computational cost
//!
//! The J and K matrix construction is the dominant cost in the SCF procedure.
//! With a naive O(N⁴) implementation (no screening), the cost scales as:
//!
//! - **J matrix**: Σ_{μνλσ} D_{λσ} (μν|λσ) — O(N⁴) multiplications and additions
//! - **K matrix**: Σ_{μνλσ} D_{λσ} (μλ|νσ) — O(N⁴) multiplications and additions
//!
//! where N is the number of basis functions. For a typical molecule with
//! N = 100 basis functions, this means ~10⁸ operations per SCF iteration.
//!
//! # Known limitations
//!
//! - **No screening**: This implementation does not use Schwarz screening or
//!   density-based screening to skip negligible ERI contributions. All N⁴
//!   integrals are processed regardless of their magnitude. For large basis
//!   sets, this can be 10-100× slower than a screened implementation.
//!
//! - **No density fitting**: The O(N⁴) scaling can be reduced to O(N³) using
//!   density fitting (resolution-of-identity) approximations, but this is
//!   not implemented here.
//!
//! - **No parallelism**: The loops are single-threaded. Production codes use
//!   OpenMP or MPI to parallelize the J/K construction.
//!
//! These limitations are intentional to keep the implementation simple and
//! correct. The POLER-ERI meta-compiler focuses on the integral computation
//! itself; the Fock construction is a straightforward (if expensive) application
//! of the computed integrals.

/// Compute the Coulomb (J) and exchange (K) matrices from the density matrix and ERI buffer.
///
/// # What it does
///
/// Constructs the J and K matrices using the standard O(N⁴) algorithm:
///
/// ```text
/// J_μν = Σ_{λ=0}^{n-1} Σ_{σ=0}^{n-1} D_{λσ} · (μν | λσ)
/// K_μν = Σ_{λ=0}^{n-1} Σ_{σ=0}^{n-1} D_{λσ} · (μλ | νσ)
/// ```
///
/// # How it works
///
/// The ERI buffer is stored as a flat array with **8-fold symmetry**:
///
/// ```text
/// (μν|λσ) = (νμ|λσ) = (μν|σλ) = (νμ|σλ)   [bra/ket permutational symmetry]
///          = (λσ|μν) = (σλ|μν) = (λσ|νμ) = (σλ|νμ)   [bra-ket symmetry]
/// ```
///
/// The indexing convention is:
///
/// ```text
/// eri_buf[μ * n³ + ν * n² + λ * n + σ] = (μν | λσ)
/// ```
///
/// This is the "canonical" or "physicist's" ordering, where the bra indices
/// (μ, ν) vary slowest and the ket indices (λ, σ) vary fastest.
///
/// # Arguments
///
/// * `density` — Density matrix D, flat n×n row-major array
/// * `eri_buf` — ERI buffer, flat n⁴ array with canonical ordering.
///   `eri_buf[μ * n³ + ν * n² + λ * n + σ]` = (μν|λσ).
/// * `n` — Number of basis functions
///
/// # Returns
///
/// A tuple `(J, K)` where both are flat n×n row-major arrays.
///
/// # Performance
///
/// This is an O(N⁴) algorithm with no screening. For N = 100, expect ~10⁸
/// operations. This is the dominant cost in each SCF iteration.
///
/// # Examples
///
/// ```
/// use poler_eri::fock::compute_jk;
///
/// let n = 2;
/// // Simple ERI buffer: all integrals = 0.5
/// let eri_buf = vec![0.5; n * n * n * n];
/// let density = vec![1.0, 0.0, 0.0, 1.0];
///
/// let (j, k) = compute_jk(&density, &eri_buf, n);
/// assert_eq!(j.len(), n * n);
/// assert_eq!(k.len(), n * n);
/// ```
pub fn compute_jk(density: &[f64], eri_buf: &[f64], n: usize) -> (Vec<f64>, Vec<f64>) {
    let nn = n * n;
    assert_eq!(density.len(), nn, "Density matrix must have n² elements");
    assert_eq!(eri_buf.len(), nn * nn, "ERI buffer must have n⁴ elements");

    let mut j = vec![0.0; nn];
    let mut k = vec![0.0; nn];

    // J_μν = Σ_{λσ} D_{λσ} (μν|λσ)
    for mu in 0..n {
        for nu in 0..n {
            let mut j_val = 0.0;
            let mut k_val = 0.0;
            for lam in 0..n {
                for sig in 0..n {
                    let d_ls = density[lam * n + sig];
                    // (μν|λσ)
                    let eri_mnls = eri_buf[mu * n * n * n + nu * n * n + lam * n + sig];
                    // (μλ|νσ)
                    let eri_mlns = eri_buf[mu * n * n * n + lam * n * n + nu * n + sig];

                    j_val += d_ls * eri_mnls;
                    k_val += d_ls * eri_mlns;
                }
            }
            j[mu * n + nu] = j_val;
            k[mu * n + nu] = k_val;
        }
    }

    (j, k)
}

/// Build the Fock matrix: F = H_core + J − ½ K.
///
/// # What it does
///
/// Constructs the Fock matrix from the core Hamiltonian and the Coulomb and
/// exchange matrices:
///
/// ```text
/// F_μν = H_core_μν + J_μν − ½ K_μν
/// ```
///
/// The factor of ½ in front of K accounts for the self-interaction correction
/// in Hartree-Fock theory. Each electron interacts with the total electron
/// density through J, but the exchange term K corrects for the fact that
/// electrons of the same spin avoid each other (Fermi correlation), which
/// halves the exchange contribution.
///
/// # Why the ½ factor
///
/// The Hartree-Fock energy is:
///
/// ```text
/// E = Σ_{μν} D_{μν} H_{μν} + ½ Σ_{μνλσ} D_{μν} D_{λσ} [(μν|λσ) − ½(μλ|νσ)]
/// ```
///
/// The Fock matrix is the variational derivative of this energy with respect
/// to the density matrix:
///
/// ```text
/// ∂E/∂D_{μν} = H_{μν} + Σ_{λσ} D_{λσ} [(μν|λσ) − ½(μλ|νσ)]
///            = H_{μν} + J_{μν} − ½ K_{μν}
/// ```
///
/// The ½ factor in the energy becomes 1 for J and ½ for K in the Fock matrix.
///
/// # Arguments
///
/// * `h_core` — Core Hamiltonian matrix H_core = T + V_nuc, flat n×n row-major array
/// * `density` — Density matrix D, flat n×n row-major array
/// * `eri_buf` — ERI buffer, flat n⁴ array with canonical ordering
/// * `n` — Number of basis functions
///
/// # Returns
///
/// The Fock matrix F as a flat n×n row-major array.
///
/// # Examples
///
/// ```
/// use poler_eri::fock::build_fock;
///
/// let n = 2;
/// let h_core = vec![1.0, 0.0, 0.0, 1.0];
/// let density = vec![1.0, 0.0, 0.0, 1.0];
/// let eri_buf = vec![0.5; n * n * n * n];
///
/// let fock = build_fock(&h_core, &density, &eri_buf, n);
/// assert_eq!(fock.len(), n * n);
/// // F = H + J - 0.5*K
/// // With all ERIs = 0.5 and D = I:
/// // J_μν = Σ D_λσ * 0.5 = n * 0.5 = 1.0 (for each μν)
/// // K_μν = Σ D_λσ * 0.5 = n * 0.5 = 1.0 (for each μν)
/// // F = H + 1.0 - 0.5 = H + 0.5
/// assert!((fock[0] - 1.5).abs() < 1e-10);
/// assert!((fock[3] - 1.5).abs() < 1e-10);
/// ```
pub fn build_fock(h_core: &[f64], density: &[f64], eri_buf: &[f64], n: usize) -> Vec<f64> {
    let (j, k) = compute_jk(density, eri_buf, n);

    let mut fock = vec![0.0; n * n];
    for i in 0..(n * n) {
        fock[i] = h_core[i] + j[i] - 0.5 * k[i];
    }

    fock
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_jk_zero_eri() {
        let n = 2;
        let eri_buf = vec![0.0; n * n * n * n];
        let density = vec![1.0, 0.0, 0.0, 1.0];
        let (j, k) = compute_jk(&density, &eri_buf, n);
        // All ERIs are zero, so J and K should be zero
        for &val in j.iter() {
            assert!(val.abs() < 1e-12, "J should be zero with zero ERIs");
        }
        for &val in k.iter() {
            assert!(val.abs() < 1e-12, "K should be zero with zero ERIs");
        }
    }

    #[test]
    fn test_compute_jk_uniform_eri() {
        let n = 2;
        let eri_val = 0.5;
        let eri_buf = vec![eri_val; n * n * n * n];
        let density = vec![1.0, 0.0, 0.0, 1.0];

        let (j, k) = compute_jk(&density, &eri_buf, n);

        // J_μν = Σ_{λσ} D_{λσ} * 0.5
        // D is diagonal with D_00 = D_11 = 1
        // J_00 = 0.5 * (1 + 1) = 1.0 (sum over λ=0,1; σ=0,1 of D_λσ * 0.5)
        // Actually: J_00 = D_00*0.5 + D_11*0.5 = 0.5 + 0.5 = 1.0
        assert!((j[0] - 1.0).abs() < 1e-10, "J_00 = {}", j[0]);
        assert!((j[3] - 1.0).abs() < 1e-10, "J_11 = {}", j[3]);
    }

    #[test]
    fn test_build_fock_zero_eri() {
        let n = 2;
        let h_core = vec![1.0, 0.5, 0.5, 2.0];
        let density = vec![1.0, 0.0, 0.0, 1.0];
        let eri_buf = vec![0.0; n * n * n * n];

        let fock = build_fock(&h_core, &density, &eri_buf, n);

        // With zero ERIs, F = H_core
        assert!((fock[0] - 1.0).abs() < 1e-12);
        assert!((fock[1] - 0.5).abs() < 1e-12);
        assert!((fock[2] - 0.5).abs() < 1e-12);
        assert!((fock[3] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn test_build_fock_formula() {
        let n = 2;
        let h_core = vec![1.0, 0.0, 0.0, 1.0];
        let density = vec![1.0, 0.0, 0.0, 1.0];
        let eri_buf = vec![0.5; n * n * n * n];

        let fock = build_fock(&h_core, &density, &eri_buf, n);

        // J and K both have all elements = 1.0 for this case
        // F = H + J - 0.5*K = H + 0.5
        assert!((fock[0] - 1.5).abs() < 1e-10, "F_00 = {}", fock[0]);
        assert!((fock[3] - 1.5).abs() < 1e-10, "F_11 = {}", fock[3]);
    }

    #[test]
    fn test_build_fock_symmetry() {
        let n = 3;
        let h_core = vec![
            1.0, 0.5, 0.2,
            0.5, 2.0, 0.3,
            0.2, 0.3, 1.5,
        ];
        let density = vec![
            1.0, 0.1, 0.0,
            0.1, 2.0, 0.2,
            0.0, 0.2, 0.5,
        ];
        let eri_buf = vec![0.1; n * n * n * n];

        let fock = build_fock(&h_core, &density, &eri_buf, n);

        // Fock matrix should be symmetric (H is symmetric, J and K are symmetric
        // for symmetric D and symmetric ERIs)
        for i in 0..n {
            for j in 0..n {
                assert!(
                    (fock[i * n + j] - fock[j * n + i]).abs() < 1e-10,
                    "Fock matrix should be symmetric: F[{}][{}] = {}, F[{}][{}] = {}",
                    i, j, fock[i * n + j], j, i, fock[j * n + i]
                );
            }
        }
    }

    #[test]
    fn test_jk_single_eri() {
        let n = 2;
        let mut eri_buf = vec![0.0; n * n * n * n];
        // Set only (00|00) = 1.0
        eri_buf[0] = 1.0;
        let density = vec![2.0, 0.0, 0.0, 0.0]; // D_00 = 2

        let (j, k) = compute_jk(&density, &eri_buf, n);

        // J_00 = Σ D_λσ (00|λσ) = D_00 * (00|00) = 2 * 1 = 2
        assert!((j[0] - 2.0).abs() < 1e-12, "J_00 = {}", j[0]);
        // K_00 = Σ D_λσ (0λ|0σ) = D_00 * (00|00) = 2 * 1 = 2
        assert!((k[0] - 2.0).abs() < 1e-12, "K_00 = {}", k[0]);
    }
}
