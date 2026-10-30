//! Molecule definitions and utilities.
//!
//! # What this module does
//!
//! This module defines the [`Molecule`] struct, which represents a molecular
//! system as a collection of atoms with nuclear charges and 3D coordinates.
//! It provides methods for computing molecular properties (nuclear repulsion
//! energy, electron count) and factory methods for common test molecules.
//!
//! # Why this module exists
//!
//! The `Molecule` struct is the top-level data structure that anchors all
//! quantum chemistry calculations. Before computing any integrals or running
//! SCF, you need a molecule. This module provides:
//!
//! - **Nuclear repulsion energy**: The classical Coulomb repulsion between
//!   all pairs of nuclei, which is a constant added to the electronic energy.
//! - **Electron count**: Needed to determine occupation numbers in SCF.
//! - **XYZ parser**: Standard molecular geometry format support.
//! - **Test molecules**: Pre-built H₂, LiH, and Li₄₀ for regression testing.
//!
//! # How it works
//!
//! The nuclear repulsion energy is:
//!
//! ```text
//! E_nuc = Σ_{i<j}  Z_i · Z_j / R_ij
//! ```
//!
//! where `Z_i` is the nuclear charge and `R_ij` is the inter-nuclear distance
//! in Bohr. The 1/R Coulomb potential uses atomic units (Hartree, Bohr).

use crate::types::{Atom, Point};
use libm::sqrt;

/// A molecular system defined by its atoms, charge, and spin multiplicity.
///
/// # What it represents
///
/// A `Molecule` is the complete specification of the nuclear framework for a
/// quantum chemistry calculation. It contains:
///
/// - The list of atoms (nuclear charge + 3D position)
/// - The total molecular charge (0 for neutral, +1 for cation, etc.)
/// - The spin multiplicity (2S+1, where S is the total spin quantum number)
///
/// # Construction
///
/// Use the [`Molecule::new`] constructor, the factory methods
/// ([`Molecule::h2`], [`Molecule::lih`], [`Molecule::li40`]), or parse from
/// an XYZ string with [`Molecule::from_xyz`].
///
/// # Examples
///
/// ```
/// use poler_eri::molecule::Molecule;
/// use poler_eri::{Atom, Point};
///
/// // Build a water molecule manually
/// let water = Molecule::new(
///     vec![
///         Atom { charge: 8.0,  coords: Point([0.0, 0.0, 0.0]) },
///         Atom { charge: 1.0,  coords: Point([0.0, 1.43, 0.95]) },
///         Atom { charge: 1.0,  coords: Point([0.0, -1.43, 0.95]) },
///     ],
///     0.0,
///     1,
/// );
/// assert_eq!(water.n_electrons(), 10); // 8 + 1 + 1 = 10
/// assert!(water.nuclear_repulsion() > 0.0);
/// ```
#[derive(Debug, Clone)]
pub struct Molecule {
    /// List of atoms in the molecule, each with nuclear charge and 3D position.
    pub atoms: Vec<Atom>,
    /// Total molecular charge (0 = neutral, positive = cation, negative = anion).
    pub charge: f64,
    /// Spin multiplicity (2S+1). 1 = singlet, 2 = doublet, 3 = triplet, etc.
    pub multiplicity: usize,
}

impl Molecule {
    /// Create a new molecule from its constituent data.
    ///
    /// # What it does
    ///
    /// Constructs a `Molecule` from a list of atoms, total charge, and spin
    /// multiplicity. No validation is performed beyond basic sanity checks.
    ///
    /// # Why it exists
    ///
    /// This is the primary constructor for building molecules programmatically.
    /// For standard test molecules, prefer the factory methods ([`h2`](Molecule::h2),
    /// [`lih`](Molecule::lih), [`li40`](Molecule::li40)) which use well-known
    /// geometries.
    ///
    /// # Arguments
    ///
    /// * `atoms` — Vector of [`Atom`] structs, each with charge and coordinates.
    /// * `charge` — Total molecular charge (e.g., 0.0 for neutral).
    /// * `multiplicity` — Spin multiplicity (2S+1). Must be ≥ 1.
    ///
    /// # Returns
    ///
    /// A new `Molecule` instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::molecule::Molecule;
    /// use poler_eri::types::{Atom, Point};
    ///
    /// let h2 = Molecule::new(
    ///     vec![
    ///         Atom { charge: 1.0, coords: Point([0.0, 0.0, 0.0]) },
    ///         Atom { charge: 1.0, coords: Point([1.4, 0.0, 0.0]) },
    ///     ],
    ///     0.0,
    ///     1,
    /// );
    /// assert_eq!(h2.atoms.len(), 2);
    /// ```
    pub fn new(atoms: Vec<Atom>, charge: f64, multiplicity: usize) -> Self {
        assert!(multiplicity >= 1, "Multiplicity must be >= 1");
        Molecule { atoms, charge, multiplicity }
    }

    /// Compute the nuclear repulsion energy of the molecule.
    ///
    /// # What it does
    ///
    /// Calculates the classical Coulomb repulsion energy between all pairs of
    /// nuclei:
    ///
    /// ```text
    /// E_nuc = Σ_{i<j}  Z_i · Z_j / R_ij
    /// ```
    ///
    /// where `Z_i` is the nuclear charge of atom `i`, and `R_ij` is the
    /// Euclidean distance between atoms `i` and `j`.
    ///
    /// # Why it exists
    ///
    /// The nuclear repulsion energy is a constant contribution to the total
    /// energy that must be added to the electronic energy from the SCF
    /// calculation. It is independent of the basis set and depends only on
    /// the nuclear geometry.
    ///
    /// # How it works
    ///
    /// A double loop over all unique atom pairs (i < j) computes the Coulomb
    /// potential `Z_i · Z_j / R_ij` and accumulates the total. Distances are
    /// computed using [`Point::dist2`] and taking the square root.
    ///
    /// # Returns
    ///
    /// The nuclear repulsion energy in Hartree (atomic units).
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::molecule::Molecule;
    /// use poler_eri::types::{Atom, Point};
    ///
    /// // H₂ at R = 1.4 Bohr: E_nuc = 1*1/1.4 ≈ 0.714
    /// let h2 = Molecule::new(
    ///     vec![
    ///         Atom { charge: 1.0, coords: Point([0.0, 0.0, 0.0]) },
    ///         Atom { charge: 1.0, coords: Point([1.4, 0.0, 0.0]) },
    ///     ],
    ///     0.0,
    ///     1,
    /// );
    /// let e_nuc = h2.nuclear_repulsion();
    /// assert!((e_nuc - 1.0/1.4).abs() < 1e-10);
    /// ```
    pub fn nuclear_repulsion(&self) -> f64 {
        let mut energy = 0.0;
        let n = self.atoms.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let r2 = self.atoms[i].coords.dist2(&self.atoms[j].coords);
                if r2 > 0.0 {
                    let r = sqrt(r2);
                    energy += self.atoms[i].charge * self.atoms[j].charge / r;
                }
            }
        }
        energy
    }

    /// Compute the total number of electrons in the molecule.
    ///
    /// # What it does
    ///
    /// Returns the total electron count: the sum of all nuclear charges minus
    /// the total molecular charge.
    ///
    /// ```text
    /// N_e = Σ_i Z_i - Q
    /// ```
    ///
    /// # Why it exists
    ///
    /// The electron count determines the occupation of molecular orbitals in
    /// the SCF procedure. For a closed-shell molecule with `N_e` electrons,
    /// there are `N_e / 2` occupied orbitals.
    ///
    /// # Returns
    ///
    /// The total number of electrons as a `usize`. Rounded from the floating-point
    /// computation (should be integer for physically meaningful molecules).
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::molecule::Molecule;
    ///
    /// // Neutral H₂: 1 + 1 = 2 electrons
    /// let h2 = Molecule::h2(1.4);
    /// assert_eq!(h2.n_electrons(), 2);
    ///
    /// // Neutral LiH: 3 + 1 = 4 electrons
    /// let lih = Molecule::lih();
    /// assert_eq!(lih.n_electrons(), 4);
    /// ```
    pub fn n_electrons(&self) -> usize {
        let total_z: f64 = self.atoms.iter().map(|a| a.charge).sum();
        let ne = total_z - self.charge;
        assert!(ne >= 0.0, "Negative electron count: total_Z={}, charge={}", total_z, self.charge);
        ne.round() as usize
    }

    /// Create an H₂ molecule along the x-axis.
    ///
    /// # What it does
    ///
    /// Constructs a hydrogen molecule with the two H atoms placed symmetrically
    /// about the origin along the x-axis, separated by the given bond length.
    ///
    /// ```text
    /// H at (-R/2, 0, 0) —— H at (+R/2, 0, 0)
    /// ```
    ///
    /// # Why it exists
    ///
    /// H₂ is the simplest molecule and the standard test case for quantum
    /// chemistry methods. The equilibrium bond length is approximately 1.4 Bohr
    /// (0.74 Å). It is used throughout the POLER-ERI test suite for verifying
    /// integral accuracy and SCF convergence.
    ///
    /// # Arguments
    ///
    /// * `bond_length` — H-H distance in Bohr. The standard equilibrium value
    ///   is ~1.4 Bohr.
    ///
    /// # Returns
    ///
    /// A neutral singlet H₂ molecule.
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::molecule::Molecule;
    ///
    /// let h2 = Molecule::h2(1.4);
    /// assert_eq!(h2.atoms.len(), 2);
    /// assert_eq!(h2.n_electrons(), 2);
    /// assert!(h2.nuclear_repulsion() > 0.0);
    /// ```
    pub fn h2(bond_length: f64) -> Self {
        let half = bond_length / 2.0;
        Molecule {
            atoms: vec![
                Atom { charge: 1.0, coords: Point([-half, 0.0, 0.0]) },
                Atom { charge: 1.0, coords: Point([half, 0.0, 0.0]) },
            ],
            charge: 0.0,
            multiplicity: 1,
        }
    }

    /// Create a LiH molecule.
    ///
    /// # What it does
    ///
    /// Constructs a lithium hydride molecule along the x-axis with the
    /// experimental equilibrium bond length of 3.015 Bohr (1.596 Å).
    /// Li is placed at the origin and H at +3.015 Bohr along x.
    ///
    /// ```text
    /// Li at (0, 0, 0) —————— H at (3.015, 0, 0)
    /// ```
    ///
    /// # Why it exists
    ///
    /// LiH is a standard test molecule for quantum chemistry methods because:
    ///
    /// - It has 4 electrons (manageable for debugging).
    /// - It has both core (Li 1s) and valence (Li 2s, H 1s) electrons.
    /// - It is polar, testing charge transfer and dipole moments.
    /// - It appears in many benchmark datasets (e.g., the NIST CCCBDB).
    ///
    /// # Returns
    ///
    /// A neutral singlet LiH molecule.
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::molecule::Molecule;
    ///
    /// let lih = Molecule::lih();
    /// assert_eq!(lih.atoms.len(), 2);
    /// assert_eq!(lih.n_electrons(), 4);
    /// ```
    pub fn lih() -> Self {
        let bond_length = 3.015; // Bohr
        Molecule {
            atoms: vec![
                Atom { charge: 3.0, coords: Point([0.0, 0.0, 0.0]) },
                Atom { charge: 1.0, coords: Point([bond_length, 0.0, 0.0]) },
            ],
            charge: 0.0,
            multiplicity: 1,
        }
    }

    /// Create a Li₄₀ cluster in a simple cubic lattice.
    ///
    /// # What it does
    ///
    /// Constructs a Li₄₀ cluster by placing lithium atoms on a 3D simple cubic
    /// lattice with lattice constant a = 3.51 Bohr. The cluster is built by
    /// filling layers of the lattice until 40 atoms are placed:
    ///
    /// - Layer 1 (z=0): 4×4 = 16 atoms
    /// - Layer 2 (z=a): 4×4 = 16 atoms
    /// - Layer 3 (z=2a): 4×2 = 8 atoms
    /// - Total: 16 + 16 + 8 = 40 atoms
    ///
    /// # Why it exists
    ///
    /// Li₄₀ is a challenging test case for:
    ///
    /// - **Performance benchmarking**: 40 atoms × 120 electrons is large enough
    ///   to stress-test integral screening and SCF convergence.
    /// - **Periodic boundary conditions**: The cubic lattice approximates the
    ///   bcc structure of bulk lithium.
    /// - **Scalability testing**: The O(N²) and O(N⁴) scaling of different
    ///   algorithms becomes apparent at this system size.
    ///
    /// The lattice constant 3.51 Bohr corresponds to the experimental lattice
    /// constant of lithium metal (3.51 Bohr ≈ 1.86 Å for the conventional
    /// bcc cell, scaled for the simple cubic approximation).
    ///
    /// # Returns
    ///
    /// A neutral singlet Li₄₀ cluster.
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::molecule::Molecule;
    ///
    /// let li40 = Molecule::li40();
    /// assert_eq!(li40.atoms.len(), 40);
    /// assert_eq!(li40.n_electrons(), 120);
    /// ```
    pub fn li40() -> Self {
        let a = 3.51; // Bohr, lattice constant
        let mut atoms = Vec::with_capacity(40);

        // Layer 1: z = 0, 4×4 grid (16 atoms)
        for ix in 0..4 {
            for iy in 0..4 {
                atoms.push(Atom {
                    charge: 3.0,
                    coords: Point([ix as f64 * a, iy as f64 * a, 0.0]),
                });
            }
        }

        // Layer 2: z = a, 4×4 grid (16 atoms)
        for ix in 0..4 {
            for iy in 0..4 {
                atoms.push(Atom {
                    charge: 3.0,
                    coords: Point([ix as f64 * a, iy as f64 * a, a]),
                });
            }
        }

        // Layer 3: z = 2a, 4×2 grid (8 atoms)
        for ix in 0..4 {
            for iy in 0..2 {
                atoms.push(Atom {
                    charge: 3.0,
                    coords: Point([ix as f64 * a, iy as f64 * a, 2.0 * a]),
                });
            }
        }

        assert_eq!(atoms.len(), 40, "Li40 should have exactly 40 atoms");

        Molecule {
            atoms,
            charge: 0.0,
            multiplicity: 1,
        }
    }

    /// Parse a molecule from an XYZ format string.
    ///
    /// # What it does
    ///
    /// Parses the standard XYZ molecular geometry format:
    ///
    /// ```text
    /// N                          ← number of atoms
    /// comment line               ← ignored
    /// Z1  x1  y1  z1            ← atom 1 (Z = atomic number, coordinates in Å)
    /// Z2  x2  y2  z2            ← atom 2
    /// ...
    /// ```
    ///
    /// The coordinates in XYZ files are in Ångströms and are converted to Bohr
    /// (1 Å = 1.8897259886 Bohr).
    ///
    /// # Why it exists
    ///
    /// The XYZ format is the most widely used molecular geometry format in
    /// computational chemistry. Almost every quantum chemistry program can
    /// read and write XYZ files. Supporting this format allows POLER-ERI to
    /// import geometries from databases, visualization programs, and other
    /// chemistry software.
    ///
    /// # How it works
    ///
    /// 1. Read the first line as the atom count `N`.
    /// 2. Skip the comment line.
    /// 3. For each of the next `N` lines, parse the atomic symbol/number and
    ///    three coordinates.
    /// 4. Convert coordinates from Ångström to Bohr.
    /// 5. Construct a neutral singlet molecule.
    ///
    /// # Arguments
    ///
    /// * `content` — A string containing the XYZ-format geometry.
    ///
    /// # Returns
    ///
    /// `Ok(Molecule)` on success, or `Err(String)` with a descriptive error
    /// message if parsing fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use poler_eri::molecule::Molecule;
    ///
    /// let xyz = "2\nH2 molecule\n1  0.0  0.0  0.0\n1  0.0  0.0  0.74\n";
    /// let mol = Molecule::from_xyz(xyz).unwrap();
    /// assert_eq!(mol.atoms.len(), 2);
    /// assert_eq!(mol.n_electrons(), 2);
    /// ```
    pub fn from_xyz(content: &str) -> Result<Self, String> {
        let angstrom_to_bohr = 1.8897259886;

        let lines: Vec<&str> = content.lines().collect();
        if lines.len() < 3 {
            return Err("XYZ file must have at least 3 lines (count, comment, atoms)".to_string());
        }

        // Parse atom count
        let n_atoms: usize = lines[0].trim().parse()
            .map_err(|e| format!("Failed to parse atom count from '{}': {}", lines[0], e))?;

        if lines.len() < 2 + n_atoms {
            return Err(format!(
                "Expected {} atom lines, but file has only {} lines",
                n_atoms, lines.len() - 2
            ));
        }

        // Skip comment line (lines[1])

        let mut atoms = Vec::with_capacity(n_atoms);

        for i in 0..n_atoms {
            let line = lines[2 + i].trim();
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                return Err(format!(
                    "Line {} has insufficient fields: expected at least 4 (Z x y z), got {}",
                    i + 3, parts.len()
                ));
            }

            // Parse atomic number (support both number and symbol)
            let z: f64 = parse_atomic_number(parts[0])
                .map_err(|e| format!("Line {}: {}", i + 3, e))?;

            let x: f64 = parts[1].parse()
                .map_err(|e| format!("Line {}: failed to parse x coordinate: {}", i + 3, e))?;
            let y: f64 = parts[2].parse()
                .map_err(|e| format!("Line {}: failed to parse y coordinate: {}", i + 3, e))?;
            let z_coord: f64 = parts[3].parse()
                .map_err(|e| format!("Line {}: failed to parse z coordinate: {}", i + 3, e))?;

            atoms.push(Atom {
                charge: z,
                coords: Point([
                    x * angstrom_to_bohr,
                    y * angstrom_to_bohr,
                    z_coord * angstrom_to_bohr,
                ]),
            });
        }

        Ok(Molecule {
            atoms,
            charge: 0.0,
            multiplicity: 1,
        })
    }
}

/// Parse an atomic number from either a numeric string or an element symbol.
///
/// Supports both formats:
/// - Numeric: "1", "6", "8"
/// - Symbol: "H", "C", "O", "Li"
///
/// Returns the nuclear charge as `f64`.
fn parse_atomic_number(s: &str) -> Result<f64, String> {
    // Try parsing as a number first
    if let Ok(n) = s.parse::<f64>() {
        if n > 0.0 && n <= 118.0 {
            return Ok(n);
        }
    }

    // Try as element symbol
    let z = match s {
        "H"  => 1.0,
        "He" => 2.0,
        "Li" => 3.0,
        "Be" => 4.0,
        "B"  => 5.0,
        "C"  => 6.0,
        "N"  => 7.0,
        "O"  => 8.0,
        "F"  => 9.0,
        "Ne" => 10.0,
        "Na" => 11.0,
        "Mg" => 12.0,
        "Al" => 13.0,
        "Si" => 14.0,
        "P"  => 15.0,
        "S"  => 16.0,
        "Cl" => 17.0,
        "Ar" => 18.0,
        "K"  => 19.0,
        "Ca" => 20.0,
        _ => return Err(format!("Unknown element symbol: '{}'", s)),
    };

    Ok(z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_molecule_new() {
        let mol = Molecule::new(
            vec![
                Atom { charge: 1.0, coords: Point([0.0, 0.0, 0.0]) },
                Atom { charge: 1.0, coords: Point([1.0, 0.0, 0.0]) },
            ],
            0.0,
            1,
        );
        assert_eq!(mol.atoms.len(), 2);
        assert!((mol.charge - 0.0).abs() < 1e-14);
        assert_eq!(mol.multiplicity, 1);
    }

    #[test]
    fn test_nuclear_repulsion_h2() {
        let h2 = Molecule::h2(1.4);
        let e_nuc = h2.nuclear_repulsion();
        let expected = 1.0 * 1.0 / 1.4;
        assert!((e_nuc - expected).abs() < 1e-10,
            "H2 nuclear repulsion: expected {}, got {}", expected, e_nuc);
    }

    #[test]
    fn test_nuclear_repulsion_single_atom() {
        let h = Molecule::new(
            vec![Atom { charge: 1.0, coords: Point([0.0, 0.0, 0.0]) }],
            0.0,
            2,
        );
        assert!((h.nuclear_repulsion() - 0.0).abs() < 1e-14,
            "Single atom should have zero nuclear repulsion");
    }

    #[test]
    fn test_nuclear_repulsion_three_atoms() {
        // He₃ equilateral triangle with side length 1.0 Bohr
        let mol = Molecule::new(
            vec![
                Atom { charge: 2.0, coords: Point([0.0, 0.0, 0.0]) },
                Atom { charge: 2.0, coords: Point([1.0, 0.0, 0.0]) },
                Atom { charge: 2.0, coords: Point([0.5, sqrt(0.75), 0.0]) },
            ],
            0.0,
            1,
        );
        let e_nuc = mol.nuclear_repulsion();
        // 3 pairs, each: 2*2/1.0 = 4.0
        let expected = 3.0 * 4.0;
        assert!((e_nuc - expected).abs() < 1e-10,
            "He3 nuclear repulsion: expected {}, got {}", expected, e_nuc);
    }

    #[test]
    fn test_n_electrons_h2() {
        let h2 = Molecule::h2(1.4);
        assert_eq!(h2.n_electrons(), 2);
    }

    #[test]
    fn test_n_electrons_lih() {
        let lih = Molecule::lih();
        assert_eq!(lih.n_electrons(), 4);
    }

    #[test]
    fn test_n_electrons_li40() {
        let li40 = Molecule::li40();
        assert_eq!(li40.n_electrons(), 120);
    }

    #[test]
    fn test_n_electrons_cation() {
        let h2_plus = Molecule::new(
            vec![
                Atom { charge: 1.0, coords: Point([0.0, 0.0, 0.0]) },
                Atom { charge: 1.0, coords: Point([1.4, 0.0, 0.0]) },
            ],
            1.0,  // +1 charge
            2,    // doublet
        );
        assert_eq!(h2_plus.n_electrons(), 1);
    }

    #[test]
    fn test_h2_geometry() {
        let h2 = Molecule::h2(1.4);
        assert_eq!(h2.atoms.len(), 2);
        assert!((h2.atoms[0].charge - 1.0).abs() < 1e-14);
        assert!((h2.atoms[1].charge - 1.0).abs() < 1e-14);

        // H atoms at ±0.7 along x
        assert!((h2.atoms[0].coords.0[0] - (-0.7)).abs() < 1e-14);
        assert!((h2.atoms[1].coords.0[0] - 0.7).abs() < 1e-14);

        // Verify distance
        let r2 = h2.atoms[0].coords.dist2(&h2.atoms[1].coords);
        assert!((r2 - 1.96).abs() < 1e-14);
    }

    #[test]
    fn test_lih_geometry() {
        let lih = Molecule::lih();
        assert_eq!(lih.atoms.len(), 2);
        assert!((lih.atoms[0].charge - 3.0).abs() < 1e-14); // Li
        assert!((lih.atoms[1].charge - 1.0).abs() < 1e-14); // H

        // Li at origin, H at 3.015 along x
        assert!((lih.atoms[0].coords.0[0] - 0.0).abs() < 1e-14);
        assert!((lih.atoms[1].coords.0[0] - 3.015).abs() < 1e-14);
    }

    #[test]
    fn test_li40_atom_count() {
        let li40 = Molecule::li40();
        assert_eq!(li40.atoms.len(), 40);

        // All atoms should be lithium (Z=3)
        for atom in &li40.atoms {
            assert!((atom.charge - 3.0).abs() < 1e-14,
                "All atoms should be Li, found Z={}", atom.charge);
        }
    }

    #[test]
    fn test_li40_no_coincident_atoms() {
        let li40 = Molecule::li40();
        for i in 0..40 {
            for j in (i + 1)..40 {
                let r2 = li40.atoms[i].coords.dist2(&li40.atoms[j].coords);
                assert!(r2 > 0.0, "Atoms {} and {} are coincident", i, j);
            }
        }
    }

    #[test]
    fn test_from_xyz_numeric() {
        let xyz = "2\nH2 test\n1  0.0  0.0  0.0\n1  0.0  0.0  0.74\n";
        let mol = Molecule::from_xyz(xyz).unwrap();
        assert_eq!(mol.atoms.len(), 2);
        assert_eq!(mol.n_electrons(), 2);
    }

    #[test]
    fn test_from_xyz_element_symbols() {
        let xyz = "2\nLiH test\nLi  0.0  0.0  0.0\nH  0.0  0.0  1.596\n";
        let mol = Molecule::from_xyz(xyz).unwrap();
        assert_eq!(mol.atoms.len(), 2);
        assert!((mol.atoms[0].charge - 3.0).abs() < 1e-14); // Li
        assert!((mol.atoms[1].charge - 1.0).abs() < 1e-14); // H
    }

    #[test]
    fn test_from_xyz_water() {
        let xyz = "3\nWater molecule\nO  0.0  0.0  0.0\nH  0.0  0.757  0.587\nH  0.0  -0.757  0.587\n";
        let mol = Molecule::from_xyz(xyz).unwrap();
        assert_eq!(mol.atoms.len(), 3);
        assert_eq!(mol.n_electrons(), 10); // 8 + 1 + 1
    }

    #[test]
    fn test_from_xyz_coordinates_in_bohr() {
        let xyz = "2\nH2 test\n1  0.0  0.0  0.0\n1  1.0  0.0  0.0\n";
        let mol = Molecule::from_xyz(xyz).unwrap();
        // Coordinates should be converted from Å to Bohr
        let angstrom_to_bohr = 1.8897259886;
        let expected_x = 1.0 * angstrom_to_bohr;
        assert!((mol.atoms[1].coords.0[0] - expected_x).abs() < 1e-10,
            "Coordinate conversion: expected {}, got {}", expected_x, mol.atoms[1].coords.0[0]);
    }

    #[test]
    fn test_from_xyz_too_few_lines() {
        let xyz = "2\nonly comment line\n";
        let result = Molecule::from_xyz(xyz);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_xyz_invalid_atom_count() {
        let xyz = "abc\ncomment\n1 0 0 0\n";
        let result = Molecule::from_xyz(xyz);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_xyz_unknown_element() {
        let xyz = "1\nbad element\nXx  0.0  0.0  0.0\n";
        let result = Molecule::from_xyz(xyz);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_atomic_number_numeric() {
        assert_eq!(parse_atomic_number("1").unwrap(), 1.0);
        assert_eq!(parse_atomic_number("6").unwrap(), 6.0);
        assert_eq!(parse_atomic_number("8").unwrap(), 8.0);
    }

    #[test]
    fn test_parse_atomic_number_symbol() {
        assert_eq!(parse_atomic_number("H").unwrap(), 1.0);
        assert_eq!(parse_atomic_number("C").unwrap(), 6.0);
        assert_eq!(parse_atomic_number("O").unwrap(), 8.0);
        assert_eq!(parse_atomic_number("Li").unwrap(), 3.0);
        assert_eq!(parse_atomic_number("Cl").unwrap(), 17.0);
    }

    #[test]
    fn test_parse_atomic_number_unknown() {
        assert!(parse_atomic_number("Xx").is_err());
        assert!(parse_atomic_number("0").is_err());
        assert!(parse_atomic_number("200").is_err());
    }

    #[test]
    fn test_nuclear_repulsion_lih() {
        let lih = Molecule::lih();
        let e_nuc = lih.nuclear_repulsion();
        // E_nuc = 3 * 1 / 3.015 = 0.995...
        let expected = 3.0 / 3.015;
        assert!((e_nuc - expected).abs() < 1e-8,
            "LiH nuclear repulsion: expected {}, got {}", expected, e_nuc);
    }

    #[test]
    fn test_nuclear_repulsion_positive() {
        let h2 = Molecule::h2(1.4);
        assert!(h2.nuclear_repulsion() > 0.0);

        let lih = Molecule::lih();
        assert!(lih.nuclear_repulsion() > 0.0);

        let li40 = Molecule::li40();
        assert!(li40.nuclear_repulsion() > 0.0);
    }
}
