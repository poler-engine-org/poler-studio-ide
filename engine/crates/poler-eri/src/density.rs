//! Density matrix construction for restricted Hartree-Fock (RHF).
//!
//! # What this module does
//!
//! This module constructs the density matrix **D** from molecular orbital
//! coefficients **C** for the restricted (closed-shell) Hartree-Fock method.
//!
//! # Why this module exists
//!
//! The density matrix is the central quantity in the SCF iteration. After
//! diagonalizing the Fock matrix to obtain MO coefficients, the density
//! matrix must be rebuilt before the next Fock matrix can be constructed:
//!
//! ```text
//! SCF cycle:  C → D → F → C → D → ... (until convergence)
//! ```
//!
//! The RHF density matrix for a closed-shell system with `n_occ` doubly
//! occupied orbitals is:
//!
//! ```text
//! D_μν = 2 · Σ_{i=1}^{n_occ} C_μi · C_νi
//! ```
//!
//! The factor of 2 accounts for spin degeneracy (each spatial orbital holds
//! 2 electrons: one α and one β).

/// Build the RHF density matrix from molecular orbital coefficients.
///
/// # What it does
///
/// Constructs the closed-shell density matrix **D** by summing over the
/// occupied molecular orbitals:
///
/// ```text
/// D_μν = 2 · Σ_{i=0}^{n_occ-1} C_μi · C_νi
/// ```
///
/// where `C_μi` is the coefficient of basis function μ in molecular orbital i,
/// and the factor of 2 accounts for the two electrons (α and β) in each
/// occupied spatial orbital.
///
/// # Why this is important
///
/// The density matrix is the quantity that changes between SCF iterations.
/// Convergence is reached when the density matrix stops changing (within a
/// tolerance), not when the energy stops changing. The density matrix is also
/// used to compute the DIIS error vector: **e = FDS − SDF**.
///
/// # How it works
///
/// The MO coefficient matrix **C** has dimensions `n × n` (n basis functions ×
/// n molecular orbitals), stored in row-major order. The first `n_occ` columns
/// correspond to the occupied orbitals. The density matrix is computed by:
///
/// 1. Zero-initialize the n×n density matrix.
/// 2. For each occupied orbital `i` (0 ≤ i < n_occ):
///    - For each pair (μ, ν): D_μν += 2 * C_μi * C_νi
///
/// The result is symmetric by construction (D_μν = D_νμ).
///
/// # Arguments
///
/// * `mo_coeffs` — Flat array of MO coefficients in row-major order.
///   `mo_coeffs[mu * n + i]` is the coefficient of basis function `mu`
///   in molecular orbital `i`. Must have length `n * n`.
/// * `n` — Number of basis functions (dimension of the square matrices).
/// * `n_occ` — Number of occupied molecular orbitals (doubly occupied
///   spatial orbitals). For water with STO-3G: n_occ = 5.
///
/// # Returns
///
/// The density matrix **D** as a flat array of length `n * n` in row-major order.
/// Guaranteed to be symmetric.
///
/// # Panics
///
/// Panics if `mo_coeffs.len() != n * n` or `n_occ > n`.
///
/// # Examples
///
/// ```
/// use poler_eri::density::build_density_rhf;
///
/// // Two basis functions, one occupied orbital
/// // MO coefficients: orbital 0 = [1.0, 0.0], orbital 1 = [0.0, 1.0]
/// let c = vec![1.0, 0.0, 0.0, 1.0]; // 2x2 identity
/// let d = build_density_rhf(&c, 2, 1);
/// // D = 2 * C_0 * C_0^T = 2 * [1,0]^T * [1,0] = [[2,0],[0,0]]
/// assert!((d[0] - 2.0).abs() < 1e-12);
/// assert!(d[1].abs() < 1e-12);
/// assert!(d[2].abs() < 1e-12);
/// assert!(d[3].abs() < 1e-12);
/// ```
pub fn build_density_rhf(mo_coeffs: &[f64], n: usize, n_occ: usize) -> Vec<f64> {
    assert_eq!(mo_coeffs.len(), n * n, "MO coefficient matrix must have n*n elements");
    assert!(n_occ <= n, "Number of occupied orbitals cannot exceed total orbitals");

    let mut density = vec![0.0; n * n];

    for i in 0..n_occ {
        for mu in 0..n {
            let c_mu_i = mo_coeffs[mu * n + i];
            for nu in 0..n {
                let c_nu_i = mo_coeffs[nu * n + i];
                density[mu * n + nu] += 2.0 * c_mu_i * c_nu_i;
            }
        }
    }

    density
}

/// Compute the RHF electronic energy from the density, core Hamiltonian, and Fock matrices.
///
/// # What it does
///
/// Computes the electronic energy using the formula:
///
/// ```text
/// E_elec = 0.5 * tr(D · (H_core + F))
/// ```
///
/// where tr denotes the matrix trace (sum of diagonal elements). This is equivalent
/// to:
///
/// ```text
/// E_elec = 0.5 * Σ_μ Σ_ν D_νμ · (H_core_μν + F_μν)
/// ```
///
/// # Why it exists
///
/// The RHF energy is not simply `tr(D · F)` — that would double-count the
/// electron-electron repulsion. The correct formula uses the average of the
/// core Hamiltonian and Fock contributions to avoid this double counting.
/// This is a well-known result from the variational principle applied to the
/// Hartree-Fock energy functional.
///
/// # Arguments
///
/// * `density` — Density matrix D, flat n×n row-major array
/// * `h_core` — Core Hamiltonian matrix H, flat n×n row-major array
/// * `fock` — Fock matrix F, flat n×n row-major array
/// * `n` — Dimension of the matrices
///
/// # Returns
///
/// The electronic energy E_elec.
///
/// # Examples
///
/// ```
/// use poler_eri::density::rhf_electronic_energy;
///
/// let d = vec![2.0, 0.0, 0.0, 0.0]; // 2x2 density
/// let h = vec![1.0, 0.0, 0.0, 1.0]; // 2x2 core Hamiltonian
/// let f = vec![2.0, 0.0, 0.0, 2.0]; // 2x2 Fock
/// let e = rhf_electronic_energy(&d, &h, &f, 2);
/// // E = 0.5 * (2*3 + 0) = 3.0
/// assert!((e - 3.0).abs() < 1e-12);
/// ```
pub fn rhf_electronic_energy(density: &[f64], h_core: &[f64], fock: &[f64], n: usize) -> f64 {
    let mut energy = 0.0;
    for mu in 0..n {
        for nu in 0..n {
            energy += density[nu * n + mu] * (h_core[mu * n + nu] + fock[mu * n + nu]);
        }
    }
    0.5 * energy
}

/// Compute the maximum absolute difference between two density matrices.
///
/// # What it does
///
/// Computes `max_μν |D_old_μν − D_new_μν|`, which is the convergence metric
/// for the density matrix in the SCF procedure.
///
/// # Why it exists
///
/// SCF convergence is typically assessed by two criteria:
/// 1. Energy change: |E_new − E_old| < conv_e
/// 2. Density change: max|D_new − D_old| < conv_d
///
/// Both must be satisfied for full convergence. The density criterion is
/// often the stricter of the two and catches oscillatory behavior that
/// the energy criterion alone might miss.
///
/// # Arguments
///
/// * `d_old` — Previous density matrix, flat n×n row-major array
/// * `d_new` — Current density matrix, flat n×n row-major array
/// * `n` — Dimension of the matrices
///
/// # Returns
///
/// The maximum absolute element-wise difference between the two density matrices.
pub fn density_rms_change(d_old: &[f64], d_new: &[f64], n: usize) -> f64 {
    let mut max_diff = 0.0;
    for i in 0..(n * n) {
        let diff = libm::fabs(d_new[i] - d_old[i]);
        if diff > max_diff {
            max_diff = diff;
        }
    }
    max_diff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_density_rhf_identity() {
        // MO coefficients = identity, 1 occupied orbital
        let c = vec![1.0, 0.0, 0.0, 1.0];
        let d = build_density_rhf(&c, 2, 1);
        // D_00 = 2 * C_00 * C_00 = 2, D_01 = 2 * C_00 * C_10 = 0
        // D_10 = 2 * C_10 * C_00 = 0, D_11 = 2 * C_10 * C_10 = 0
        assert!((d[0] - 2.0).abs() < 1e-12);
        assert!(d[1].abs() < 1e-12);
        assert!(d[2].abs() < 1e-12);
        assert!(d[3].abs() < 1e-12);
    }

    #[test]
    fn test_build_density_rhf_two_occupied() {
        // MO coefficients = identity, 2 occupied orbitals
        let c = vec![1.0, 0.0, 0.0, 1.0];
        let d = build_density_rhf(&c, 2, 2);
        // D_00 = 2*1*1 + 2*0*0 = 2, D_11 = 2*0*0 + 2*1*1 = 2
        assert!((d[0] - 2.0).abs() < 1e-12);
        assert!((d[3] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn test_build_density_rhf_symmetry() {
        let c = vec![0.6, -0.8, 0.8, 0.6]; // rotation matrix
        let d = build_density_rhf(&c, 2, 1);
        assert!((d[0 * 2 + 1] - d[1 * 2 + 0]).abs() < 1e-12,
            "Density matrix should be symmetric");
    }

    #[test]
    fn test_rhf_electronic_energy_simple() {
        let d = vec![2.0, 0.0, 0.0, 0.0];
        let h = vec![1.0, 0.0, 0.0, 1.0];
        let f = vec![2.0, 0.0, 0.0, 2.0];
        let e = rhf_electronic_energy(&d, &h, &f, 2);
        // sum: D_00*(H_00+F_00) + D_10*(H_01+F_01) + D_01*(H_10+F_10) + D_11*(H_11+F_11)
        //    = 2*3 + 0 + 0 + 0 = 6
        // E = 0.5 * 6 = 3.0
        assert!((e - 3.0).abs() < 1e-12);
    }

    #[test]
    fn test_density_rms_change_zero() {
        let d = vec![1.0, 2.0, 2.0, 5.0];
        let diff = density_rms_change(&d, &d, 2);
        assert!(diff.abs() < 1e-12, "Same matrix should have zero change");
    }

    #[test]
    fn test_density_rms_change_nonzero() {
        let d1 = vec![1.0, 0.0, 0.0, 1.0];
        let d2 = vec![1.1, 0.0, 0.0, 0.9];
        let diff = density_rms_change(&d1, &d2, 2);
        let expected = 0.1; // max(|0.1|, |−0.1|) = 0.1
        assert!((diff - expected).abs() < 1e-12);
    }
}
