#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Verify (pp|ss) and (pp|pp) quartets at the generic test geometry.
use poler_eri::shell_driver::eri_shell_full;
use poler_eri::types::{Point, QuartetData};

fn main() {
    let a = Point([0.0, 0.0, 0.0]);
    let b = Point([0.5, 0.0, 0.0]);
    let c = Point([3.0, 0.7, 0.0]);
    let d = Point([3.0, -0.5, 0.3]);
    let (al, be, ga, de) = (1.5, 1.3, 1.7, 1.1);
    let qd = QuartetData::new(a, b, c, d, al, be, ga, de);

    // (pp|ss): 9 values — layout (i, j) with p comps (x,y,z)
    let vals = eri_shell_full(&qd, 1, 1, 0, 0);
    let labs = ["x", "y", "z"];
    let mut k = 0;
    for i in 0..3 {
        for j in 0..3 {
            println!("(p{} p{} | s s) = {:.12}", labs[i], labs[j], vals[k]);
            k += 1;
        }
    }
    println!();
    // (pp|pp): 81 values — print the 4 corner/interesting components
    let vals = eri_shell_full(&qd, 1, 1, 1, 1);
    // layout: ((i*3+j)*3+k)*3+l
    let idx = |i: usize, j: usize, k: usize, l: usize| ((i * 3 + j) * 3 + k) * 3 + l;
    for (i, j, k, l) in [(0, 0, 0, 0), (0, 1, 0, 1), (0, 0, 1, 2), (1, 2, 0, 1), (2, 2, 2, 2)] {
        println!("(p{} p{} | p{} p{}) = {:.12}", labs[i], labs[j], labs[k], labs[l], vals[idx(i, j, k, l)]);
    }
}
