//! IntegralEngine — unified interface for all one-electron integrals.
//!
//! # What this module does
//!
//! This module provides a high-level API for computing all the one-electron
//! integrals needed for a quantum chemistry calculation:
//!
//! - **Overlap matrix** S_μν = ⟨χ_μ|χ_ν⟩
//! - **Kinetic energy matrix** T_μν = ⟨χ_μ| −½∇² |χ_ν⟩
//! - **Nuclear attraction matrix** V_μν = Σ_A ⟨χ_μ| −Z_A/|r−A| |χ_ν⟩
//! - **Core Hamiltonian** H_core = T + V
//!
//! # Why this module exists
//!
//! The IntegralEngine encapsulates the basis set information and provides a
//! clean, type-safe interface for computing integral matrices. Instead of
//! calling low-level primitive integral functions with raw exponents and
//! coordinates, users construct an `IntegralEngine` from a [`BasisSet`] and
//! then call methods to get the desired matrices.
//!
//! # Implementation notes
//!
//! Currently, only the s-type (l=0) primitive integrals are implemented.
//! For higher angular momentum shells, the Obara-Saika recurrence relations
//! would need to be applied. The implementation handles contracted Gaussians
//! by summing over all primitive pairs with the appropriate normalized
//! contraction coefficients.
//!
//! The shell-pair loops use the standard structure:
//!
//! ```text
//! for shell_a in shells:
//!     for shell_b in shells:
//!         for prim_a in shell_a.primitives:
//!             for prim_b in shell_b.primitives:
//!                 integral += c_a * c_b * primitive_integral(alpha_a, alpha_b, ...)
//! ```

use crate::cart::cart_comps;
use crate::one_electron::{kinetic_cartesian, nuclear_cartesian, overlap_cartesian};
use crate::types::{Atom, BasisSet, Point, Shell};

/// Unified integral engine for computing one-electron integral matrices.
///
/// This struct holds the basis set information and provides methods for
/// computing overlap, kinetic energy, nuclear attraction, and core Hamiltonian
/// matrices. It is constructed from a [`BasisSet`] and then used to compute
/// integral matrices on demand.
///
/// # Usage
///
/// ```text
/// let engine = IntegralEngine::new(&basis_set);
/// let s = engine.overlap_matrix();
/// let t = engine.kinetic_matrix();
/// let v = engine.nuclear_matrix();
/// let h = engine.core_hamiltonian();
/// ```
///
/// # Shell ordering
///
/// The rows/columns of the returned matrices follow the shell ordering in the
/// basis set's `shells` vector. Within each shell, the Cartesian components
/// are ordered in the standard reverse lexicographic convention (see
/// [`cart_comps`](crate::cart::cart_comps)).
///
/// # Thread safety
///
/// The `IntegralEngine` is read-only after construction and can be shared
/// between threads (it implements `Send` and `Sync` by default since all
/// fields are owned `Vec`s).
pub struct IntegralEngine {
    /// Copy of the shells from the basis set.
    pub shells: Vec<Shell>,
    /// Coordinates of the shell centers (one per shell, derived from atom positions).
    pub coords: Vec<Point>,
    /// Copy of the atoms from the basis set.
    pub atoms: Vec<Atom>,
}

impl IntegralEngine {
    /// Create a new integral engine from a basis set.
    ///
    /// # What it does
    ///
    /// Extracts the shells, atom coordinates, and atom data from the basis set
    /// and stores them for later use. The shell coordinates are looked up from
    /// the atom positions using the `atom_idx` field of each shell.
    ///
    /// # Arguments
    ///
    /// * `basis` — The basis set containing atoms and shells.
    ///
    /// # Returns
    ///
    /// A new `IntegralEngine` instance ready to compute integral matrices.
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::{IntegralEngine, BasisSet, Atom, Shell, Point};
    /// use poler_eri::normalization::normalize_shell;
    ///
    /// // Create a minimal basis set with one H atom
    /// let atom = Atom { charge: 1.0, coords: Point([0.0, 0.0, 0.0]) };
    /// let mut shell = Shell::new(0, 0, vec![1.0], vec![1.0]);
    /// normalize_shell(&mut shell);
    /// let basis = BasisSet {
    ///     atoms: vec![atom],
    ///     shells: vec![shell],
    /// };
    ///
    /// let engine = IntegralEngine::new(&basis);
    /// assert_eq!(engine.shells.len(), 1);
    /// assert_eq!(engine.atoms.len(), 1);
    /// ```
    pub fn new(basis: &BasisSet) -> Self {
        // Extract shell center coordinates from atom positions
        let coords: Vec<Point> = basis
            .shells
            .iter()
            .map(|shell| basis.atoms[shell.atom_idx].coords)
            .collect();

        IntegralEngine {
            shells: basis.shells.clone(),
            coords,
            atoms: basis.atoms.clone(),
        }
    }

    /// Compute the total number of basis functions (Cartesian).
    ///
    /// # What it does
    ///
    /// Sums up `ncart(l)` for all shells to get the total number of Cartesian
    /// basis functions. This is the dimension of all the integral matrices.
    ///
    /// # Returns
    ///
    /// The total number of Cartesian basis functions.
    fn n_basis(&self) -> usize {
        self.shells.iter().map(|s| s.n_cart()).sum()
    }

    /// Compute the overlap matrix S_μν.
    ///
    /// # What it does
    ///
    /// Constructs the overlap matrix **S** where:
    ///
    /// ```text
    /// S_μν = ⟨χ_μ|χ_ν⟩
    /// ```
    ///
    /// The matrix is symmetric (S_μν = S_νμ) and positive-definite.
    ///
    /// # How it works
    ///
    /// For each pair of shells (A, B):
    /// 1. Loop over all primitive pairs (a in A, b in B).
    /// 2. Compute the s-type overlap integral using [`overlap_ss`].
    /// 3. Accumulate the contribution with normalized contraction coefficients.
    ///
    /// For higher angular momentum shells, the Obara-Saika recurrence would
    /// need to be applied to build the full Cartesian component matrix.
    /// Currently, only the s-type component is computed for each shell pair.
    ///
    /// # Returns
    ///
    /// The overlap matrix as a flat n×n row-major array, where n is the total
    /// number of Cartesian basis functions.
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::{IntegralEngine, BasisSet, Atom, Shell, Point};
    /// use poler_eri::normalization::normalize_shell;
    ///
    /// let atom = Atom { charge: 1.0, coords: Point([0.0, 0.0, 0.0]) };
    /// let mut shell = Shell::new(0, 0, vec![1.0], vec![1.0]);
    /// normalize_shell(&mut shell);
    /// let basis = BasisSet { atoms: vec![atom], shells: vec![shell] };
    ///
    /// let engine = IntegralEngine::new(&basis);
    /// let s = engine.overlap_matrix();
    /// // 1 basis function → 1×1 overlap matrix
    /// assert_eq!(s.len(), 1);
    /// // Self-overlap of a normalized s-type Gaussian is 1.0
    /// assert!(s[0] > 0.0);
    /// ```
    pub fn overlap_matrix(&self) -> Vec<f64> {
        let n = self.n_basis();
        let mut s = vec![0.0; n * n];

        let mut mu = 0;
        for (ia, shell_a) in self.shells.iter().enumerate() {
            let ra = self.coords[ia];
            let mut nu = 0;
            for (ib, shell_b) in self.shells.iter().enumerate() {
                let rb = self.coords[ib];
                let _rab2 = ra.dist2(&rb);

                // For s-type shells, n_cart = 1, so only one element per shell pair
                // For higher AM, we'd loop over Cartesian components
                let nca = shell_a.n_cart();
                let ncb = shell_b.n_cart();

                // Loop over primitive pairs
                for pa in 0..shell_a.n_prim() {
                    for pb in 0..shell_b.n_prim() {
                        let alpha_a = shell_a.alphas[pa];
                        let alpha_b = shell_b.alphas[pb];
                        let ca = shell_a.norm_coeffs[pa];
                        let cb = shell_b.norm_coeffs[pb];

                        // AUDIT FIX (v3.2.0-d): exact per-Cartesian-component
                        // integrals (the old code smeared the s-type value over
                        // every component — placeholder, wrong for l > 0).
                        let comps_a = cart_comps(shell_a.l);
                        let comps_b = cart_comps(shell_b.l);
                        for i_cart in 0..nca {
                            for j_cart in 0..ncb {
                                let val = overlap_cartesian(
                                    &ra, &rb, alpha_a, alpha_b,
                                    [comps_a[i_cart].0, comps_a[i_cart].1, comps_a[i_cart].2],
                                    [comps_b[j_cart].0, comps_b[j_cart].1, comps_b[j_cart].2],
                                );
                                let row = mu + i_cart;
                                let col = nu + j_cart;
                                s[row * n + col] += ca * cb * val;
                            }
                        }
                    }
                }

                nu += ncb;
            }
            mu += shell_a.n_cart();
        }

        s
    }

    /// Compute the kinetic energy matrix T_μν.
    ///
    /// # What it does
    ///
    /// Constructs the kinetic energy matrix **T** where:
    ///
    /// ```text
    /// T_μν = ⟨χ_μ| −½∇² |χ_ν⟩
    /// ```
    ///
    /// # How it works
    ///
    /// Same shell-pair / primitive-pair structure as [`overlap_matrix`], but
    /// uses [`kinetic_ss`] instead of [`overlap_ss`] for the primitive integrals.
    ///
    /// # Returns
    ///
    /// The kinetic energy matrix as a flat n×n row-major array.
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::{IntegralEngine, BasisSet, Atom, Shell, Point};
    /// use poler_eri::normalization::normalize_shell;
    ///
    /// let atom = Atom { charge: 1.0, coords: Point([0.0, 0.0, 0.0]) };
    /// let mut shell = Shell::new(0, 0, vec![1.0], vec![1.0]);
    /// normalize_shell(&mut shell);
    /// let basis = BasisSet { atoms: vec![atom], shells: vec![shell] };
    ///
    /// let engine = IntegralEngine::new(&basis);
    /// let t = engine.kinetic_matrix();
    /// assert_eq!(t.len(), 1);
    /// assert!(t[0] > 0.0, "Kinetic energy should be positive at same center");
    /// ```
    pub fn kinetic_matrix(&self) -> Vec<f64> {
        let n = self.n_basis();
        let mut t = vec![0.0; n * n];

        let mut mu = 0;
        for (ia, shell_a) in self.shells.iter().enumerate() {
            let ra = self.coords[ia];
            let mut nu = 0;
            for (ib, shell_b) in self.shells.iter().enumerate() {
                let rb = self.coords[ib];
                let _rab2 = ra.dist2(&rb);

                let nca = shell_a.n_cart();
                let ncb = shell_b.n_cart();

                for pa in 0..shell_a.n_prim() {
                    for pb in 0..shell_b.n_prim() {
                        let alpha_a = shell_a.alphas[pa];
                        let alpha_b = shell_b.alphas[pb];
                        let ca = shell_a.norm_coeffs[pa];
                        let cb = shell_b.norm_coeffs[pb];

                        // AUDIT FIX (v3.2.0-d): exact per-component kinetic
                        let comps_a = cart_comps(shell_a.l);
                        let comps_b = cart_comps(shell_b.l);
                        for i_cart in 0..nca {
                            for j_cart in 0..ncb {
                                let val = kinetic_cartesian(
                                    &ra, &rb, alpha_a, alpha_b,
                                    [comps_a[i_cart].0, comps_a[i_cart].1, comps_a[i_cart].2],
                                    [comps_b[j_cart].0, comps_b[j_cart].1, comps_b[j_cart].2],
                                );
                                let row = mu + i_cart;
                                let col = nu + j_cart;
                                t[row * n + col] += ca * cb * val;
                            }
                        }
                    }
                }

                nu += ncb;
            }
            mu += shell_a.n_cart();
        }

        t
    }

    /// Compute the nuclear attraction matrix V_μν.
    ///
    /// # What it does
    ///
    /// Constructs the nuclear attraction matrix **V** where:
    ///
    /// ```text
    /// V_μν = Σ_A ⟨χ_μ| −Z_A/|r−A| |χ_ν⟩
    /// ```
    ///
    /// The sum runs over all atoms A with nuclear charge Z_A at position R_A.
    ///
    /// # How it works
    ///
    /// For each pair of shells and each atom:
    /// 1. Compute the Gaussian product center P = (α_a A + α_b B) / (α_a + α_b).
    /// 2. Compute the squared distance R_PC² from P to the nucleus C.
    /// 3. Evaluate the s-type nuclear attraction integral using [`nuclear_ss`].
    /// 4. Accumulate with normalized contraction coefficients.
    ///
    /// # Returns
    ///
    /// The nuclear attraction matrix as a flat n×n row-major array.
    /// Typically negative (attractive interaction).
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::{IntegralEngine, BasisSet, Atom, Shell, Point};
    /// use poler_eri::normalization::normalize_shell;
    ///
    /// let atom = Atom { charge: 1.0, coords: Point([0.0, 0.0, 0.0]) };
    /// let mut shell = Shell::new(0, 0, vec![1.0], vec![1.0]);
    /// normalize_shell(&mut shell);
    /// let basis = BasisSet { atoms: vec![atom], shells: vec![shell] };
    ///
    /// let engine = IntegralEngine::new(&basis);
    /// let v = engine.nuclear_matrix();
    /// assert_eq!(v.len(), 1);
    /// assert!(v[0] < 0.0, "Nuclear attraction should be negative (attractive)");
    /// ```
    pub fn nuclear_matrix(&self) -> Vec<f64> {
        let n = self.n_basis();
        let mut v = vec![0.0; n * n];

        let mut mu = 0;
        for (ia, shell_a) in self.shells.iter().enumerate() {
            let ra = self.coords[ia];
            let mut nu = 0;
            for (ib, shell_b) in self.shells.iter().enumerate() {
                let rb = self.coords[ib];
                let _rab2 = ra.dist2(&rb);

                let nca = shell_a.n_cart();
                let ncb = shell_b.n_cart();

                // Loop over all atoms (nuclear centers)
                for atom in &self.atoms {
                    let rc = atom.coords;
                    let zc = atom.charge;

                    for pa in 0..shell_a.n_prim() {
                        for pb in 0..shell_b.n_prim() {
                            let alpha_a = shell_a.alphas[pa];
                            let alpha_b = shell_b.alphas[pb];
                            let ca = shell_a.norm_coeffs[pa];
                            let cb = shell_b.norm_coeffs[pb];

                            // AUDIT FIX (v3.2.0-d): exact per-component nuclear
                            let comps_a = cart_comps(shell_a.l);
                            let comps_b = cart_comps(shell_b.l);
                            for i_cart in 0..nca {
                                for j_cart in 0..ncb {
                                    let val = nuclear_cartesian(
                                        &ra, &rb, &rc, alpha_a, alpha_b, zc,
                                        [comps_a[i_cart].0, comps_a[i_cart].1, comps_a[i_cart].2],
                                        [comps_b[j_cart].0, comps_b[j_cart].1, comps_b[j_cart].2],
                                    );
                                    let row = mu + i_cart;
                                    let col = nu + j_cart;
                                    v[row * n + col] += ca * cb * val;
                                }
                            }
                        }
                    }
                }

                nu += ncb;
            }
            mu += shell_a.n_cart();
        }

        v
    }

    /// Compute the core Hamiltonian matrix H_core = T + V.
    ///
    /// # What it does
    ///
    /// Constructs the core Hamiltonian matrix:
    ///
    /// ```text
    /// H_core_μν = T_μν + V_μν
    /// ```
    ///
    /// where T is the kinetic energy matrix and V is the nuclear attraction matrix.
    /// The core Hamiltonian represents the one-electron part of the Fock matrix —
    /// the energy that an electron would have if all other electrons were removed.
    ///
    /// # Why it's important
    ///
    /// H_core is used as:
    ///
    /// - The initial guess for the Fock matrix in the SCF procedure.
    /// - Part of the electronic energy computation: E = ½ tr(D · (H + F)).
    /// - The one-electron contribution to the Fock matrix: F = H_core + G.
    ///
    /// # Returns
    ///
    /// The core Hamiltonian matrix as a flat n×n row-major array.
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::{IntegralEngine, BasisSet, Atom, Shell, Point};
    /// use poler_eri::normalization::normalize_shell;
    ///
    /// let atom = Atom { charge: 1.0, coords: Point([0.0, 0.0, 0.0]) };
    /// let mut shell = Shell::new(0, 0, vec![1.0], vec![1.0]);
    /// normalize_shell(&mut shell);
    /// let basis = BasisSet { atoms: vec![atom], shells: vec![shell] };
    ///
    /// let engine = IntegralEngine::new(&basis);
    /// let h = engine.core_hamiltonian();
    /// assert_eq!(h.len(), 1);
    /// // For H atom: T > 0, V < 0, H = T + V should be negative (bound state)
    /// assert!(h[0] < 0.0, "Core Hamiltonian should be negative for H atom");
    /// ```
    pub fn core_hamiltonian(&self) -> Vec<f64> {
        let t = self.kinetic_matrix();
        let v = self.nuclear_matrix();
        let n = self.n_basis();

        let mut h = vec![0.0; n * n];
        for i in 0..(n * n) {
            h[i] = t[i] + v[i];
        }

        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a minimal H atom basis set with normalized shells.
    fn h_atom_basis() -> BasisSet {
        let atom = Atom {
            charge: 1.0,
            coords: Point([0.0, 0.0, 0.0]),
        };
        let mut shell = Shell::new(0, 0, vec![1.0], vec![1.0]);
        crate::normalization::normalize_shell(&mut shell);
        BasisSet {
            atoms: vec![atom],
            shells: vec![shell],
        }
    }

    /// Helper: create a two-H-atom basis set (H₂-like) with normalized shells.
    fn h2_basis() -> BasisSet {
        let atom_a = Atom {
            charge: 1.0,
            coords: Point([0.0, 0.0, 0.0]),
        };
        let atom_b = Atom {
            charge: 1.0,
            coords: Point([1.4, 0.0, 0.0]),
        };
        let mut shell_a = Shell::new(0, 0, vec![1.0], vec![1.0]);
        crate::normalization::normalize_shell(&mut shell_a);
        let mut shell_b = Shell::new(1, 1, vec![1.0], vec![1.0]);
        crate::normalization::normalize_shell(&mut shell_b);
        BasisSet {
            atoms: vec![atom_a, atom_b],
            shells: vec![shell_a, shell_b],
        }
    }

    #[test]
    fn test_engine_new() {
        let basis = h_atom_basis();
        let engine = IntegralEngine::new(&basis);
        assert_eq!(engine.shells.len(), 1);
        assert_eq!(engine.atoms.len(), 1);
        assert_eq!(engine.coords.len(), 1);
    }

    #[test]
    fn test_engine_coords_from_atoms() {
        let basis = h2_basis();
        let engine = IntegralEngine::new(&basis);
        assert_eq!(engine.coords.len(), 2);
        // Shell 0 is on atom 0 (origin)
        assert!((engine.coords[0].0[0] - 0.0).abs() < 1e-12);
        // Shell 1 is on atom 1 (at x=1.4)
        assert!((engine.coords[1].0[0] - 1.4).abs() < 1e-12);
    }

    #[test]
    fn test_overlap_matrix_size() {
        let basis = h_atom_basis();
        let engine = IntegralEngine::new(&basis);
        let s = engine.overlap_matrix();
        assert_eq!(s.len(), 1); // 1 basis function
    }

    #[test]
    fn test_overlap_matrix_positive() {
        let basis = h_atom_basis();
        let engine = IntegralEngine::new(&basis);
        let s = engine.overlap_matrix();
        assert!(s[0] > 0.0, "Self-overlap should be positive");
    }

    #[test]
    fn test_kinetic_matrix_positive() {
        let basis = h_atom_basis();
        let engine = IntegralEngine::new(&basis);
        let t = engine.kinetic_matrix();
        assert!(t[0] > 0.0, "Self-kinetic energy should be positive");
    }

    #[test]
    fn test_nuclear_matrix_negative() {
        let basis = h_atom_basis();
        let engine = IntegralEngine::new(&basis);
        let v = engine.nuclear_matrix();
        assert!(v[0] < 0.0, "Nuclear attraction should be negative");
    }

    #[test]
    fn test_core_hamiltonian_h_atom() {
        let basis = h_atom_basis();
        let engine = IntegralEngine::new(&basis);
        let h = engine.core_hamiltonian();
        // For H atom: T + V should be negative (bound state)
        assert!(h[0] < 0.0, "Core Hamiltonian for H should be negative");
    }

    #[test]
    fn test_overlap_matrix_two_atoms() {
        let basis = h2_basis();
        let engine = IntegralEngine::new(&basis);
        let s = engine.overlap_matrix();
        // shell_a: s-type (1 component), shell_b: p-type (3 components)
        // Total: 4 basis functions → 4×4 = 16
        let n = 1 + 3; // ncart(0) + ncart(1)
        assert_eq!(s.len(), n * n);
        // Diagonal elements (self-overlap) should be positive
        assert!(s[0] > 0.0, "S_00 should be positive");
        assert!(s[n + 1] > 0.0, "S_11 should be positive");
    }

    #[test]
    fn test_core_hamiltonian_equals_t_plus_v() {
        let basis = h2_basis();
        let engine = IntegralEngine::new(&basis);
        let h = engine.core_hamiltonian();
        let t = engine.kinetic_matrix();
        let v = engine.nuclear_matrix();
        let n = 1 + 3; // ncart(0) + ncart(1)

        for i in 0..(n * n) {
            assert!(
                (h[i] - (t[i] + v[i])).abs() < 1e-12,
                "H should equal T + V: H[{}] = {}, T+V = {}",
                i,
                h[i],
                t[i] + v[i]
            );
        }
    }

    #[test]
    fn test_overlap_symmetry() {
        let basis = h2_basis();
        let engine = IntegralEngine::new(&basis);
        let s = engine.overlap_matrix();
        let n = 1 + 3;
        // Overlap matrix should be symmetric
        for i in 0..n {
            for j in 0..n {
                assert!(
                    (s[i * n + j] - s[j * n + i]).abs() < 1e-12,
                    "Overlap should be symmetric at ({}, {})", i, j
                );
            }
        }
    }

    #[test]
    fn test_kinetic_symmetry() {
        let basis = h2_basis();
        let engine = IntegralEngine::new(&basis);
        let t = engine.kinetic_matrix();
        let n = 1 + 3;
        for i in 0..n {
            for j in 0..n {
                assert!(
                    (t[i * n + j] - t[j * n + i]).abs() < 1e-12,
                    "Kinetic should be symmetric at ({}, {})", i, j
                );
            }
        }
    }

    #[test]
    fn test_nuclear_symmetry() {
        let basis = h2_basis();
        let engine = IntegralEngine::new(&basis);
        let v = engine.nuclear_matrix();
        let n = 1 + 3;
        for i in 0..n {
            for j in 0..n {
                assert!(
                    (v[i * n + j] - v[j * n + i]).abs() < 1e-12,
                    "Nuclear should be symmetric at ({}, {})", i, j
                );
            }
        }
    }
}
