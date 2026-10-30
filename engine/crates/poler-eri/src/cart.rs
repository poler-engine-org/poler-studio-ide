//! Cartesian component enumeration and shell naming for Gaussian basis functions.
//!
//! # What this module does
//!
//! This module provides utilities for working with Cartesian Gaussian basis functions.
//! It maps angular momentum quantum numbers to shell names (s, p, d, f, g, h),
//! enumerates all Cartesian components (nx, ny, nz) for a given angular momentum,
//! and computes the number of Cartesian components.
//!
//! # Why this module exists
//!
//! In quantum chemistry, each shell with angular momentum `l` has multiple Cartesian
//! components. For example, a d-shell (l=2) has 6 components: xx, xy, xz, yy, yz, zz.
//! These components are needed when:
//!
//! - Building integral batches (one integral per Cartesian component pair)
//! - Transforming from Cartesian to spherical harmonic representations
//! - Indexing into matrices (Fock, density, overlap)
//!
//! The reverse lexicographic ordering convention (x > y > z) is chosen to match
//! the ordering used in most quantum chemistry codes (Libint, libcint, PySCF),
//! which ensures compatibility with standard basis set libraries and integral
//! screening algorithms.
//!
//! # How it works
//!
//! - [`shell_name`] returns the spectroscopic notation for a given angular momentum.
//! - [`cart_comps`] generates all (nx, ny, nz) triples summing to `l`, ordered
//!   in reverse lexicographic order (x-heavy first).
//! - [`ncart`] returns the closed-form count `(l+1)(l+2)/2`.

use crate::types::CartComp;

/// Returns the spectroscopic shell name for a given angular momentum quantum number.
///
/// # What it does
///
/// Maps the integer angular momentum `l` to its standard spectroscopic notation:
///
/// | l | Name |
/// |---|------|
/// | 0 | s    |
/// | 1 | p    |
/// | 2 | d    |
/// | 3 | f    |
/// | 4 | g    |
/// | 5 | h    |
///
/// For `l >= 6`, this function panics since shells beyond h are not commonly
/// used in practical calculations and would require special handling.
///
/// # Why it exists
///
/// Shell names are used for human-readable output, basis set parsing, and
/// debugging. When printing a basis set or an integral batch, you want to see
/// "(dp|ff)" rather than "(2,3|3,3)".
///
/// # Examples
///
/// ```
/// use poler_eri::cart::shell_name;
///
/// assert_eq!(shell_name(0), "s");
/// assert_eq!(shell_name(1), "p");
/// assert_eq!(shell_name(2), "d");
/// assert_eq!(shell_name(3), "f");
/// ```
pub fn shell_name(l: usize) -> &'static str {
    match l {
        0 => "s",
        1 => "p",
        2 => "d",
        3 => "f",
        4 => "g",
        5 => "h",
        _ => panic!("shell_name: angular momentum l={} is not supported (max l=5)", l),
    }
}

/// Generates all Cartesian components (nx, ny, nz) for angular momentum `l`.
///
/// # What it does
///
/// Returns a `Vec<CartComp>` containing every triple `(nx, ny, nz)` such that
/// `nx + ny + nz = l`, ordered in **reverse lexicographic** order (also called
/// "canonical" order in many quantum chemistry codes).
///
/// Reverse lexicographic order means: we iterate `nx` from `l` down to `0`,
/// then for each `nx`, iterate `ny` from `l - nx` down to `0`, and set
/// `nz = l - nx - ny`. This places x-heavy components first.
///
/// # Why this ordering
///
/// This ordering matches the convention used in Libint, libcint, and PySCF.
/// It ensures that:
///
/// - Components with higher x-angular momentum appear first
/// - The ordering is deterministic and reproducible
/// - Cartesian-to-spherical transformations produce the standard ordering
///
/// # How it works
///
/// For each `nx` from `l` down to `0`:
///   For each `ny` from `l - nx` down to `0`:
///     `nz = l - nx - ny`
///     Append `(nx, ny, nz)`
///
/// # Examples
///
/// ```
/// use poler_eri::cart::cart_comps;
///
/// // s-shell: only one component
/// let s = cart_comps(0);
/// assert_eq!(s, vec![(0, 0, 0)]);
///
/// // p-shell: three components
/// let p = cart_comps(1);
/// assert_eq!(p, vec![(1, 0, 0), (0, 1, 0), (0, 0, 1)]);
///
/// // d-shell: six components
/// let d = cart_comps(2);
/// assert_eq!(d, vec![
///     (2, 0, 0), (1, 1, 0), (1, 0, 1),
///     (0, 2, 0), (0, 1, 1), (0, 0, 2)
/// ]);
/// ```
pub fn cart_comps(l: usize) -> Vec<CartComp> {
    let n = ncart(l);
    let mut comps = Vec::with_capacity(n);

    // Reverse lexicographic: nx from l..0, ny from (l-nx)..0, nz = l - nx - ny
    for nx in (0..=l).rev() {
        let l_rem = l - nx;
        for ny in (0..=l_rem).rev() {
            let nz = l_rem - ny;
            comps.push((nx, ny, nz));
        }
    }

    comps
}

/// Returns the number of Cartesian components for angular momentum `l`.
///
/// # What it does
///
/// Computes the closed-form expression `(l+1)(l+2)/2`, which counts the number
/// of ways to partition `l` into three non-negative integers (nx, ny, nz).
///
/// # Why it exists
///
/// This is used everywhere — to allocate integral arrays, to determine batch
/// sizes, and to validate that loop bounds are correct. Having a standalone
/// function avoids repeating the formula and potential off-by-one errors.
///
/// # Examples
///
/// ```
/// use poler_eri::cart::ncart;
///
/// assert_eq!(ncart(0), 1);  // s: 1
/// assert_eq!(ncart(1), 3);  // p: 3
/// assert_eq!(ncart(2), 6);  // d: 6
/// assert_eq!(ncart(3), 10); // f: 10
/// assert_eq!(ncart(4), 15); // g: 15
/// ```
pub fn ncart(l: usize) -> usize {
    (l + 1) * (l + 2) / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_name_basic() {
        assert_eq!(shell_name(0), "s");
        assert_eq!(shell_name(1), "p");
        assert_eq!(shell_name(2), "d");
        assert_eq!(shell_name(3), "f");
        assert_eq!(shell_name(4), "g");
        assert_eq!(shell_name(5), "h");
    }

    #[test]
    #[should_panic]
    fn test_shell_name_too_high() {
        shell_name(6);
    }

    #[test]
    fn test_ncart_values() {
        assert_eq!(ncart(0), 1);
        assert_eq!(ncart(1), 3);
        assert_eq!(ncart(2), 6);
        assert_eq!(ncart(3), 10);
        assert_eq!(ncart(4), 15);
        assert_eq!(ncart(5), 21);
    }

    #[test]
    fn test_cart_comps_count_matches_ncart() {
        for l in 0..=5 {
            let comps = cart_comps(l);
            assert_eq!(comps.len(), ncart(l),
                "cart_comps({}) returned {} components, expected {}", l, comps.len(), ncart(l));
        }
    }

    #[test]
    fn test_cart_comps_angular_momentum_sum() {
        for l in 0..=5 {
            for (nx, ny, nz) in cart_comps(l) {
                assert_eq!(nx + ny + nz, l,
                    "Component ({}, {}, {}) does not sum to l={}", nx, ny, nz, l);
            }
        }
    }

    #[test]
    fn test_cart_comps_s_shell() {
        let comps = cart_comps(0);
        assert_eq!(comps, vec![(0, 0, 0)]);
    }

    #[test]
    fn test_cart_comps_p_shell() {
        let comps = cart_comps(1);
        assert_eq!(comps, vec![(1, 0, 0), (0, 1, 0), (0, 0, 1)]);
    }

    #[test]
    fn test_cart_comps_d_shell() {
        let comps = cart_comps(2);
        assert_eq!(comps, vec![
            (2, 0, 0), (1, 1, 0), (1, 0, 1),
            (0, 2, 0), (0, 1, 1), (0, 0, 2),
        ]);
    }

    #[test]
    fn test_cart_comps_deterministic() {
        // Calling cart_comps twice should produce identical results
        for l in 0..=5 {
            assert_eq!(cart_comps(l), cart_comps(l));
        }
    }

    #[test]
    fn test_cart_comps_no_duplicates() {
        for l in 0..=5 {
            let comps = cart_comps(l);
            let mut seen = std::collections::HashSet::new();
            for &c in &comps {
                assert!(seen.insert(c),
                    "Duplicate component {:?} found for l={}", c, l);
            }
        }
    }
}
