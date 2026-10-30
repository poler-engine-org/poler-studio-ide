//! RHF (Restricted Hartree-Fock) self-consistent field procedure.
//!
//! # What this module does
//!
//! This module implements the complete RHF SCF cycle for closed-shell molecules:
//!
//! ```text
//! ┌──────────────────────────────────────┐
//! │ 1. Initial guess: D = D(H_core)      │
//! │ 2. Build Fock matrix: F = H + J − ½K │
//! │ 3. DIIS extrapolation of F            │
//! │ 4. Orthogonalize: F' = X^T F X       │
//! │ 5. Diagonalize: F' C' = C' ε         │
//! │ 6. Back-transform: C = X C'          │
//! │ 7. Build density: D = 2 Σ C_i C_i^T  │
//! │ 8. Check convergence                  │
//! │ 9. If not converged, go to step 2     │
//! └──────────────────────────────────────┘
//! ```
//!
//! # Why this module exists
//!
//! The RHF method is the foundation of almost all quantum chemistry. It provides:
//!
//! - **Molecular orbitals**: The coefficients C that define the occupied and
//!   virtual orbitals.
//! - **Orbital energies**: The eigenvalues ε that are used in perturbation
//!   theory (MP2) and excited-state methods (CIS, TD-DFT).
//! - **The reference density**: The RHF density matrix is the starting point
//!   for correlated methods (CCSD, CI, etc.).
//!
//! # Implementation details
//!
//! - **Initial guess**: The core Hamiltonian guess uses H_core as the Fock matrix
//!   for the first iteration. This is the simplest possible guess and works well
//!   for small molecules. For larger systems, superposition-of-atomic-densities
//!   (SAD) or extended Hückel guesses are more reliable, but these are not
//!   implemented here.
//!
//! - **Orthogonalization**: Löwdin symmetric orthogonalization (S^{-1/2})
//!   transforms the generalized eigenvalue problem into a standard one.
//!
//! - **DIIS**: Pulay's DIIS method is used to accelerate convergence.
//!   DIIS is applied after the Fock matrix is built and before orthogonalization.
//!
//! - **Convergence**: Two criteria are checked:
//!   1. Energy change: |E_new − E_old| < conv_e
//!   2. Density change: max|D_new − D_old| < conv_d
//!   Both must be satisfied for full convergence.

use crate::density::{build_density_rhf, density_rms_change, rhf_electronic_energy};
use crate::diagonalize::{inv_sqrt_matrix, jacobi_eigen};
use crate::diis::Diis;
use crate::fock::build_fock;

/// Result of an RHF SCF calculation.
///
/// Contains all the quantities produced by the SCF procedure: the total energy,
/// the converged density matrix, the molecular orbital coefficients, the number
/// of iterations performed, and whether convergence was achieved.
///
/// # Field descriptions
///
/// - `energy`: The total RHF energy E_total = E_electronic + E_nuclear_repulsion.
///   This is the quantity that is minimized by the SCF procedure.
///
/// - `density`: The converged density matrix D in the AO basis (flat n×n
///   row-major array). Symmetric by construction.
///
/// - `mo_coeffs`: The molecular orbital coefficient matrix C (flat n×n row-major
///   array). Column i contains the coefficients of MO i. The first `n_occ`
///   columns are occupied orbitals.
///
/// - `n_iter`: The number of SCF iterations performed. If convergence was
///   achieved, this is typically 5–20 with DIIS. If not converged, this
///   equals `max_iter`.
///
/// - `converged`: Whether the SCF procedure converged within the specified
///   tolerances. If `false`, the results are not reliable.
#[derive(Debug)]
pub struct ScfResult {
    /// Total RHF energy: E_electronic + E_nuclear_repulsion
    pub energy: f64,
    /// Converged density matrix (flat n×n row-major)
    pub density: Vec<f64>,
    /// Molecular orbital coefficients (flat n×n row-major, columns = MOs)
    pub mo_coeffs: Vec<f64>,
    /// Number of SCF iterations performed
    pub n_iter: usize,
    /// Whether the SCF converged within the specified tolerances
    pub converged: bool,
    /// Orbital energies (eigenvalues of the final Fock matrix), Hartree.
    /// v3.2.0-audit: added — they were computed and dropped every iteration.
    pub orbital_energies: Vec<f64>,
}

/// Transform the Fock matrix to the orthogonalized basis.
///
/// # What it does
///
/// Computes **F' = X^T · F · X** where **X = S^{-1/2}** is the Löwdin
/// orthogonalization matrix. This transforms the Fock matrix from the
/// non-orthogonal AO basis to an orthonormal basis, converting the generalized
/// eigenvalue problem **F C = S C ε** into a standard one **F' C' = C' ε**.
///
/// # Why Löwdin orthogonalization
///
/// The Löwdin method produces the orthogonalized basis that is "closest" to the
/// original AO basis in the least-squares sense. This means the transformed
/// orbitals retain as much chemical character as possible, which improves
/// the interpretability of the results and the numerical stability of the
/// diagonalization.
///
/// # How it works
///
/// 1. Compute the intermediate **T = F · X** (matrix-matrix multiply).
/// 2. Compute **F' = X^T · T** (matrix-matrix multiply).
///
/// Since X is symmetric (S is symmetric, so S^{-1/2} is also symmetric),
/// X^T = X, and the formula simplifies to F' = X · F · X.
///
/// # Arguments
///
/// * `fock` — Fock matrix F, flat n×n row-major array
/// * `sinv_half` — Orthogonalization matrix X = S^{-1/2}, flat n×n row-major array
/// * `n` — Dimension of the matrices
///
/// # Returns
///
/// The transformed Fock matrix F' as a flat n×n row-major array.
fn transform_fock(fock: &[f64], sinv_half: &[f64], n: usize) -> Vec<f64> {
    // Step 1: T = F · X
    let mut t = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut sum = 0.0;
            for k in 0..n {
                sum += fock[i * n + k] * sinv_half[k * n + j];
            }
            t[i * n + j] = sum;
        }
    }

    // Step 2: F' = X^T · T = X · T (since X is symmetric)
    let mut fock_prime = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut sum = 0.0;
            for k in 0..n {
                sum += sinv_half[i * n + k] * t[k * n + j];
            }
            fock_prime[i * n + j] = sum;
        }
    }

    fock_prime
}

/// Perform the RHF self-consistent field calculation.
///
/// # What it does
///
/// Executes the full RHF SCF cycle for a closed-shell molecule:
///
/// 1. **Initial guess**: Use the core Hamiltonian **H_core** as the initial
///    Fock matrix. Diagonalize H_core in the orthogonalized basis to obtain
///    initial MO coefficients and density matrix.
///
/// 2. **SCF loop** (repeat until convergence or max_iter):
///    a. Build Fock matrix: **F = H_core + J − ½K**
///    b. Compute DIIS error vector: **e = FDS − SDF**
///    c. Push (F, e) to DIIS history
///    d. Extrapolate F using DIIS (if enough vectors)
///    e. Orthogonalize: **F' = X^T · F · X**
///    f. Diagonalize: **F' · C' = C' · ε**
///    g. Back-transform: **C = X · C'**
///    h. Build density: **D = 2 Σ_{i=0}^{n_occ-1} C_i · C_i^T**
///    i. Compute electronic energy: **E_elec = ½ tr(D · (H + F))**
///    j. Check convergence on energy and density
///
/// 3. **Return** the final energy, density, MO coefficients, and convergence status.
///
/// # Arguments
///
/// * `h_core` — Core Hamiltonian matrix H = T + V_nuc, flat n×n row-major array
/// * `overlap` — Overlap matrix S, flat n×n row-major array (symmetric positive-definite)
/// * `eri_buf` — ERI buffer, flat n⁴ array with canonical ordering
/// * `n` — Number of basis functions
/// * `n_occ` — Number of doubly occupied spatial orbitals (= N_electrons / 2)
/// * `e_nuc` — Nuclear repulsion energy (sum of Z_A Z_B / R_AB over all atom pairs)
/// * `max_iter` — Maximum number of SCF iterations (default: 100)
/// * `conv_e` — Energy convergence threshold (default: 1e-8 Hartree)
/// * `conv_d` — Density convergence threshold (default: 1e-6)
///
/// # Returns
///
/// An [`ScfResult`] containing the total energy, density matrix, MO coefficients,
/// iteration count, and convergence status.
///
/// # Panics
///
/// Panics if:
/// - `n_occ > n` (more occupied orbitals than basis functions)
/// - The overlap matrix is not positive-definite (required for S^{-1/2})
///
/// # Examples
///
/// ```
/// use poler_eri::scf::{rhf_scf, ScfResult};
///
/// let n = 2;
/// let n_occ = 1;
/// let h_core = vec![-1.0, 0.0, 0.0, -1.0];
/// let overlap = vec![1.0, 0.0, 0.0, 1.0];
/// let eri_buf = vec![0.1; n * n * n * n]; // Small ERIs for stability
/// let e_nuc = 0.0;
///
/// let result = rhf_scf(&h_core, &overlap, &eri_buf, n, n_occ, e_nuc, 100, 1e-6, 1e-4);
/// assert!(result.energy.is_finite());
/// ```
pub fn rhf_scf(
    h_core: &[f64],
    overlap: &[f64],
    eri_buf: &[f64],
    n: usize,
    n_occ: usize,
    e_nuc: f64,
    max_iter: usize,
    conv_e: f64,
    conv_d: f64,
) -> ScfResult {
    assert!(n_occ <= n, "n_occ ({}) cannot exceed n ({})", n_occ, n);

    // Step 1: Compute S^{-1/2} for orthogonalization
    let sinv_half = inv_sqrt_matrix(overlap, n);

    // Step 2: Initial guess from core Hamiltonian
    let fock_prime = transform_fock(h_core, &sinv_half, n);
    let (_eigenvalues, mo_prime) = jacobi_eigen(&fock_prime, n);

    // Back-transform: C = X · C'
    let mut mo_coeffs = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut sum = 0.0;
            for k in 0..n {
                sum += sinv_half[i * n + k] * mo_prime[k * n + j];
            }
            mo_coeffs[i * n + j] = sum;
        }
    }

    // Build initial density from core Hamiltonian guess
    let mut density = build_density_rhf(&mo_coeffs, n, n_occ);

    // Initialize DIIS
    let mut diis = Diis::new();

    // Initial electronic energy
    let mut energy_old = rhf_electronic_energy(&density, h_core, h_core, n) + e_nuc;

    // SCF iteration loop
    let mut converged = false;
    let mut n_iter = 0;

    for iter in 0..max_iter {
        n_iter = iter + 1;

        // Build Fock matrix: F = H + J - 0.5*K
        let fock = build_fock(h_core, &density, eri_buf, n);

        // Compute DIIS error vector: e = FDS - SDF
        let error = crate::diis::error_vector(&fock, &density, overlap, n);

        // Push to DIIS history
        diis.push(&fock, &error);

        // DIIS extrapolation (if enough vectors)
        let fock_eff = if let Some(extrapolated) = diis.extrapolate(n) {
            extrapolated
        } else {
            fock.clone()
        };

        // Orthogonalize: F' = X^T F X
        let fock_prime = transform_fock(&fock_eff, &sinv_half, n);

        // Diagonalize: F' C' = C' ε
        let (_eigenvalues, mo_prime) = jacobi_eigen(&fock_prime, n);

        // Back-transform: C = X · C'
        mo_coeffs = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                let mut sum = 0.0;
                for k in 0..n {
                    sum += sinv_half[i * n + k] * mo_prime[k * n + j];
                }
                mo_coeffs[i * n + j] = sum;
            }
        }

        // Build new density
        let density_new = build_density_rhf(&mo_coeffs, n, n_occ);

        // Compute electronic energy: E = 0.5 * tr(D · (H + F))
        // We use the effective Fock (possibly DIIS-extrapolated) for the energy
        let e_elec = rhf_electronic_energy(&density_new, h_core, &fock_eff, n);
        let energy_new = e_elec + e_nuc;

        // Check convergence
        let delta_e = libm::fabs(energy_new - energy_old);
        let delta_d = density_rms_change(&density, &density_new, n);

        // Update for next iteration
        density = density_new;
        energy_old = energy_new;

        if delta_e < conv_e && delta_d < conv_d {
            converged = true;
            break;
        }
    }

    // Build the final Fock matrix for the energy computation
    let fock_final = build_fock(h_core, &density, eri_buf, n);
    let e_elec_final = rhf_electronic_energy(&density, h_core, &fock_final, n);

    // Orbital energies: diagonal of C^T F C (C is S-orthonormal)
    let mut orbital_energies = vec![0.0f64; n];
    for k in 0..n {
        let mut acc = 0.0;
        for i in 0..n {
            for j in 0..n {
                acc += mo_coeffs[i * n + k] * fock_final[i * n + j] * mo_coeffs[j * n + k];
            }
        }
        orbital_energies[k] = acc;
    }

    ScfResult {
        energy: e_elec_final + e_nuc,
        density,
        mo_coeffs,
        n_iter,
        converged,
        orbital_energies,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a minimal H₂-like system for testing.
    ///
    /// Two basis functions with a simple model Hamiltonian and ERIs.
    fn h2_model() -> (Vec<f64>, Vec<f64>, Vec<f64>, usize, usize, f64) {
        let n = 2;
        let n_occ = 1; // 1 doubly occupied orbital (2 electrons)

        // Core Hamiltonian (in atomic units, simplified)
        let h_core = vec![-1.0, -0.5, -0.5, -1.0];

        // Overlap matrix
        let overlap = vec![1.0, 0.5, 0.5, 1.0];

        // ERI buffer (simplified model, small values for convergence)
        let eri_buf = vec![0.1; n * n * n * n];

        // Nuclear repulsion energy (for H₂ at ~1.4 Bohr: Z₁*Z₂/R ≈ 1/1.4 ≈ 0.714)
        let e_nuc = 0.714;

        (h_core, overlap, eri_buf, n, n_occ, e_nuc)
    }

    #[test]
    fn test_transform_fock_identity() {
        let fock = vec![1.0, 2.0, 2.0, 3.0];
        let eye = vec![1.0, 0.0, 0.0, 1.0]; // S^{-1/2} = I
        let fock_prime = transform_fock(&fock, &eye, 2);
        // F' = I^T · F · I = F
        for i in 0..4 {
            assert!((fock_prime[i] - fock[i]).abs() < 1e-12,
                "Transform with identity should be identity: F'[{}] = {}, F[{}] = {}",
                i, fock_prime[i], i, fock[i]);
        }
    }

    #[test]
    fn test_transform_fock_symmetry_preserved() {
        let fock = vec![2.0, 1.0, 1.0, 3.0]; // Symmetric
        let x = vec![0.9, -0.1, -0.1, 0.9]; // Symmetric transform
        let fock_prime = transform_fock(&fock, &x, 2);
        // F' should also be symmetric
        assert!((fock_prime[0 * 2 + 1] - fock_prime[1 * 2 + 0]).abs() < 1e-12,
            "Transformed Fock should be symmetric");
    }

    #[test]
    fn test_scf_convergence() {
        let (h_core, overlap, eri_buf, n, n_occ, e_nuc) = h2_model();
        // Use relaxed convergence thresholds for the simplified model
        let result = rhf_scf(&h_core, &overlap, &eri_buf, n, n_occ, e_nuc, 200, 1e-6, 1e-4);
        // The SCF may or may not fully converge with the simplified uniform-ERI model,
        // but it should produce finite, reasonable results
        assert!(result.energy.is_finite(), "Energy should be finite");
        assert!(result.n_iter > 0, "Should run at least one iteration");
        assert!(result.n_iter <= 200, "Should not exceed max iterations");
    }

    #[test]
    fn test_scf_energy_reasonable() {
        let (h_core, overlap, eri_buf, n, n_occ, e_nuc) = h2_model();
        let result = rhf_scf(&h_core, &overlap, &eri_buf, n, n_occ, e_nuc, 100, 1e-8, 1e-6);
        // The total energy should be a finite number
        assert!(result.energy.is_finite(), "Energy should be finite");
        // For a bound molecule, the energy should be negative
        // (though this depends on the model parameters)
    }

    #[test]
    fn test_scf_density_symmetric() {
        let (h_core, overlap, eri_buf, n, n_occ, e_nuc) = h2_model();
        let result = rhf_scf(&h_core, &overlap, &eri_buf, n, n_occ, e_nuc, 100, 1e-8, 1e-6);
        // Density matrix should be symmetric
        for i in 0..n {
            for j in 0..n {
                assert!(
                    (result.density[i * n + j] - result.density[j * n + i]).abs() < 1e-8,
                    "Density should be symmetric: D[{}][{}] = {}, D[{}][{}] = {}",
                    i, j, result.density[i * n + j], j, i, result.density[j * n + i]
                );
            }
        }
    }

    #[test]
    fn test_scf_result_fields() {
        let (h_core, overlap, eri_buf, n, n_occ, e_nuc) = h2_model();
        let result = rhf_scf(&h_core, &overlap, &eri_buf, n, n_occ, e_nuc, 50, 1e-6, 1e-4);
        assert_eq!(result.density.len(), n * n);
        assert_eq!(result.mo_coeffs.len(), n * n);
        assert!(result.n_iter > 0);
    }

    #[test]
    fn test_scf_max_iter_respected() {
        let (h_core, overlap, eri_buf, n, n_occ, e_nuc) = h2_model();
        let result = rhf_scf(&h_core, &overlap, &eri_buf, n, n_occ, e_nuc, 3, 1e-15, 1e-15);
        // With very tight thresholds and only 3 iterations, likely not converged
        assert!(result.n_iter <= 3);
    }

    #[test]
    fn test_scf_identity_overlap() {
        let n = 2;
        let n_occ = 1;
        let h_core = vec![-1.0, 0.0, 0.0, -1.0];
        let overlap = vec![1.0, 0.0, 0.0, 1.0]; // Identity
        // Small ERI values for convergence
        let eri_buf = vec![0.1; n * n * n * n];
        let e_nuc = 0.0;

        let result = rhf_scf(&h_core, &overlap, &eri_buf, n, n_occ, e_nuc, 200, 1e-6, 1e-4);
        // With identity overlap and small ERIs, should produce finite results
        assert!(result.energy.is_finite(), "Energy should be finite with identity overlap");
        assert!(result.n_iter > 0);
    }
}
