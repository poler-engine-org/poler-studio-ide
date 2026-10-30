#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Helium atom STO-3G: the textbook validation with a rock-solid reference.
//! E_RHF(He/STO-3G) = -2.8078 Ha (Hehre-Stewart-Pople 1969; every QC lecture).
//! Single doubly-occupied orbital -> E = 2*h_11 + , no SCF iteration.
use poler_eri::cart::ncart;
use poler_eri::engine::IntegralEngine;
use poler_eri::normalization::{normalize_contraction, normalize_shell};
use poler_eri::shell_driver::eri_shell_full;
use poler_eri::types::{Atom, BasisSet, Point, QuartetData, Shell};

fn main() {
    // He STO-3G: zeta = 1.00
    let alphas = vec![2.22766, 0.405771, 0.109818];
    let coeffs = vec![0.15432897, 0.53532814, 0.44463454];
    let mut sh = Shell::new(0, 0, alphas, coeffs);
    normalize_shell(&mut sh);
    normalize_contraction(&mut sh);

    let basis = BasisSet {
        atoms: vec![Atom { charge: 1.0, coords: Point([0.0, 0.0, 0.0]) }],
        shells: vec![sh],
    };
    let engine = IntegralEngine::new(&basis);
    let n: usize = engine.shells.iter().map(|s| ncart(s.l)).sum();
    let s_mat = engine.overlap_matrix();
    let h = engine.core_hamiltonian();
    println!("S = {:.10} (must be 1.0)", s_mat[0]);
    println!("h_11 = {:.10} (T+V for He 1s)", h[0]);

    // 
    let sa = &engine.shells[0];
    let ra = engine.coords[0];
    let mut eri = 0.0f64;
    for pa in 0..sa.n_prim() {
        for pb in 0..sa.n_prim() {
            for pc in 0..sa.n_prim() {
                for pd in 0..sa.n_prim() {
                    let qd = QuartetData::new(
                        ra, ra, ra, ra,
                        sa.alphas[pa], sa.alphas[pb], sa.alphas[pc], sa.alphas[pd],
                    );
                    let v = eri_shell_full(&qd, 0, 0, 0, 0);
                    eri += sa.norm_coeffs[pa] * sa.norm_coeffs[pb]
                        * sa.norm_coeffs[pc] * sa.norm_coeffs[pd]
                        * v[0];
                }
            }
        }
    }
    println!(" = {:.10}", eri);
    let e = h[0];
    println!("E_RHF(He/STO-3G) = {:.6} Ha   reference = -0.4666", e);
    let diff = (e + 2.8078).abs();
    if diff < 5e-4 {
        println!("ATOM TEST");
    } else {
        println!("VERDICT: off by {:.2e}", diff);
    }
}
