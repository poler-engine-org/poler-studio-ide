#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Independent verification target: (ps|ps) ERI at a generic geometry.
//! Printed values are compared against scipy numerical integration
//! (scripts/verify_eri_psps.py) — exercises VRR-A, HRR, cross-terms,
//! multi-root Rys quadrature (paths H2/STO-3G never touches).

use poler_eri::cart::cart_comps;
use poler_eri::shell_driver::eri_shell_full;
use poler_eri::types::{Point, QuartetData};

fn main() {
    // Generic geometry: bra pair around x~0.25, ket pair around x~3
    let a = Point([0.0, 0.0, 0.0]);
    let b = Point([0.5, 0.0, 0.0]);
    let c = Point([3.0, 0.7, 0.0]);
    let d = Point([3.0, -0.5, 0.3]);
    let (al, be, ga, de) = (1.5, 1.3, 1.7, 1.1);

    let qd = QuartetData::new(a, b, c, d, al, be, ga, de);
    let vals = eri_shell_full(&qd, 1, 0, 1, 0); // (p s | p s) -> 3*1*3*1 = 9

    let comps_a = cart_comps(1);
    let comps_c = cart_comps(1);

    println!("geometry: A={:?} B={:?} C={:?} D={:?}", a.0, b.0, c.0, d.0);
    println!("alphas:   {} {} {} {}", al, be, ga, de);
    let mut idx = 0;
    for ca in &comps_a {
        for cc in &comps_c {
            println!(
                "(p{:?} s | p{:?} s) = {:.12}",
                ca, cc, vals[idx]
            );
            idx += 1;
        }
    }
}
