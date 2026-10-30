//! Core types for the POLER-ERI meta-compiler and runtime.
//!
//! This module defines the fundamental data structures used throughout the
//! project: points in 3D space, Gaussian shells, atomic data, basis sets,
//! and the all-important [`QuartetData`] which holds precomputed geometric
//! quantities for a shell quartet.

/// A Gaussian primitive shell on a specific atom.
///
/// A shell is defined by its angular momentum `l`, the atom it belongs to,
/// and sets of exponents (alphas) and contraction coefficients. After
/// normalization, `norm_coeffs` contains the properly normalized coefficients
/// that account for the GTO self-overlap.
///
/// # Cartesian vs Spherical
///
/// - Cartesian components: `(l+1)(l+2)/2` (e.g., l=2 → 6: xx, xy, xz, yy, yz, zz)
/// - Spherical harmonics: `2l+1` (e.g., l=2 → 5: d₀, d₊₁, d₋₁, d₊₂, d₋₂)
#[derive(Debug, Clone)]
pub struct Shell {
    /// Angular momentum quantum number (0=s, 1=p, 2=d, 3=f, ...)
    pub l: usize,
    /// Index of the atom this shell belongs to in the molecule's atom list
    pub atom_idx: usize,
    /// Gaussian exponents (alpha values) for each primitive
    pub alphas: Vec<f64>,
    /// Raw contraction coefficients from the basis set file
    pub coeffs: Vec<f64>,
    /// Normalized contraction coefficients (coeffs * N_alpha / sqrt(S_self))
    pub norm_coeffs: Vec<f64>,
}

impl Shell {
    /// Create a new shell with unnormalized coefficients.
    ///
    /// Call [`normalize_shell`](crate::normalization::normalize_shell) after
    /// creation to populate `norm_coeffs`.
    pub fn new(l: usize, atom_idx: usize, alphas: Vec<f64>, coeffs: Vec<f64>) -> Self {
        let n = alphas.len();
        Shell {
            l,
            atom_idx,
            alphas: alphas.clone(),
            coeffs: coeffs.clone(),
            norm_coeffs: vec![0.0; n],
        }
    }

    /// Number of primitive Gaussians in this shell.
    pub fn n_prim(&self) -> usize { self.alphas.len() }

    /// Number of Cartesian components: `(l+1)(l+2)/2`.
    ///
    /// | l | n_cart |
    /// |---|--------|
    /// | 0 | 1      |
    /// | 1 | 3      |
    /// | 2 | 6      |
    /// | 3 | 10     |
    pub fn n_cart(&self) -> usize { (self.l + 1) * (self.l + 2) / 2 }

    /// Number of spherical harmonic components: `2l+1`.
    pub fn n_sph(&self) -> usize { 2 * self.l + 1 }
}

/// A point in 3D Euclidean space.
///
/// Used to represent atomic coordinates and product centers (P, Q) in the
/// McMurchie-Davidson / Obara-Saika scheme.
#[derive(Debug, Clone, Copy)]
pub struct Point(pub [f64; 3]);

impl Point {
    /// Origin point (0, 0, 0).
    pub fn zero() -> Self { Point([0.0, 0.0, 0.0]) }

    /// Squared distance between two points: |a - b|².
    pub fn dist2(&self, other: &Point) -> f64 {
        let dx = self.0[0] - other.0[0];
        let dy = self.0[1] - other.0[1];
        let dz = self.0[2] - other.0[2];
        dx*dx + dy*dy + dz*dz
    }
}

impl std::ops::Sub for Point {
    type Output = Point;
    fn sub(self, rhs: Point) -> Point {
        Point([self.0[0]-rhs.0[0], self.0[1]-rhs.0[1], self.0[2]-rhs.0[2]])
    }
}

/// Cartesian component specification: (nx, ny, nz) where nx+ny+nz = l.
///
/// For example, a d-shell (l=2) has components:
/// (2,0,0), (1,1,0), (1,0,1), (0,2,0), (0,1,1), (0,0,2)
pub type CartComp = (usize, usize, usize);

/// Precomputed data for a shell quartet (a|b|c|d).
///
/// This struct holds ALL geometric quantities needed to evaluate ERI recurrence
/// relations for a given set of four centers with Gaussian exponents. The
/// constructor computes everything up front so the VRR/HRR pipeline can run
/// without any further geometric calculations.
///
/// # Key Quantities
///
/// - `p = alpha_a + alpha_b` — combined exponent for bra pair
/// - `q = alpha_c + alpha_d` — combined exponent for ket pair
/// - `rho = p*q/(p+q)` — reduced exponent for Boys function argument
/// - `kab = exp(-alpha_a * alpha_b * R_AB² / p)` — bra overlap factor
/// - `kcd = exp(-alpha_c * alpha_d * R_CD² / q)` — ket overlap factor
#[derive(Debug, Clone)]
pub struct QuartetData {
    /// Center A coordinates
    pub ra: Point,
    /// Center B coordinates
    pub rb: Point,
    /// Center C coordinates
    pub rc: Point,
    /// Center D coordinates
    pub rd: Point,
    /// Gaussian exponent on center A
    pub alpha_a: f64,
    /// Gaussian exponent on center B
    pub alpha_b: f64,
    /// Gaussian exponent on center C
    pub alpha_c: f64,
    /// Gaussian exponent on center D
    pub alpha_d: f64,
    /// Squared distance |A - B|²
    pub rab2: f64,
    /// Squared distance |C - D|²
    pub rcd2: f64,
    /// Combined bra exponent: `alpha_a + alpha_b`
    pub p: f64,
    /// Combined ket exponent: `alpha_c + alpha_d`
    pub q: f64,
    /// Reduced exponent: `p * q / (p + q)` — appears in Boys function argument
    pub rho: f64,
    /// Bra overlap factor: `exp(-alpha_a * alpha_b * R_AB² / p)`
    pub kab: f64,
    /// Ket overlap factor: `exp(-alpha_c * alpha_d * R_CD² / q)`
    pub kcd: f64,
    /// Overall prefactor: `2π²/(p*q) * sqrt(π/(p+q))`
    pub prefactor: f64,
}

impl QuartetData {
    /// Compute all geometric quantities for a shell quartet.
    ///
    /// This is the constructor you call before running the VRR/HRR pipeline.
    /// All the heavy math (exponentials, square roots) happens here so the
    /// recurrence relation evaluation is just additions and multiplications.
    pub fn new(ra: Point, rb: Point, rc: Point, rd: Point,
               alpha_a: f64, alpha_b: f64, alpha_c: f64, alpha_d: f64) -> Self {
        use libm::exp;
        let p = alpha_a + alpha_b;
        let q = alpha_c + alpha_d;
        let rab2 = ra.dist2(&rb);
        let rcd2 = rc.dist2(&rd);
        let rho = p * q / (p + q);
        let kab = exp(-alpha_a * alpha_b * rab2 / p);
        let kcd = exp(-alpha_c * alpha_d * rcd2 / q);
        let prefactor = 2.0 * std::f64::consts::PI.powi(2) / (p * q)
            * libm::sqrt(std::f64::consts::PI / (p + q));
        QuartetData { ra, rb, rc, rd, alpha_a, alpha_b, alpha_c, alpha_d,
                      rab2, rcd2, p, q, rho, kab, kcd, prefactor }
    }

    /// Product center P = (alpha_a * A + alpha_b * B) / p.
    ///
    /// This is the center of the Gaussian product on the bra side.
    /// Appears in the VRR recurrence as `PA_i = P_i - A_i`.
    pub fn center_ab(&self) -> Point {
        Point([
            (self.alpha_a * self.ra.0[0] + self.alpha_b * self.rb.0[0]) / self.p,
            (self.alpha_a * self.ra.0[1] + self.alpha_b * self.rb.0[1]) / self.p,
            (self.alpha_a * self.ra.0[2] + self.alpha_b * self.rb.0[2]) / self.p,
        ])
    }

    /// Product center Q = (alpha_c * C + alpha_d * D) / q.
    ///
    /// This is the center of the Gaussian product on the ket side.
    /// Appears in the VRR recurrence as `QC_i = Q_i - C_i`.
    pub fn center_cd(&self) -> Point {
        Point([
            (self.alpha_c * self.rc.0[0] + self.alpha_d * self.rd.0[0]) / self.q,
            (self.alpha_c * self.rc.0[1] + self.alpha_d * self.rd.0[1]) / self.q,
            (self.alpha_c * self.rc.0[2] + self.alpha_d * self.rd.0[2]) / self.q,
        ])
    }
}

/// A single atom with nuclear charge and 3D coordinates.
#[derive(Debug, Clone)]
pub struct Atom {
    /// Nuclear charge (1=H, 3=Li, 6=C, 8=O, etc.)
    pub charge: f64,
    /// Position in 3D space (in Bohr)
    pub coords: Point,
}

/// A complete basis set: atoms + contracted Gaussian shells.
#[derive(Debug, Clone)]
pub struct BasisSet {
    /// List of atoms in the molecule
    pub atoms: Vec<Atom>,
    /// List of contracted Gaussian shells (one per shell in the basis set)
    pub shells: Vec<Shell>,
}
