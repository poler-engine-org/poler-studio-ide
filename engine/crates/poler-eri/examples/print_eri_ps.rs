#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Print (ps|ss) values for the same test geometry (bra-side p only).
use poler_eri::shell_driver::eri_shell_full;
use poler_eri::types::{Point, QuartetData};

fn main() {
    let a = Point([0.0, 0.0, 0.0]);
    let b = Point([0.5, 0.0, 0.0]);
    let c = Point([3.0, 0.7, 0.0]);
    let d = Point([3.0, -0.5, 0.3]);
    let (al, be, ga, de) = (1.5, 1.3, 1.7, 1.1);
    let qd = QuartetData::new(a, b, c, d, al, be, ga, de);
    let vals = eri_shell_full(&qd, 1, 0, 0, 0); // (p s | s s) -> 3
    println!("(p(1,0,0) s | s s) = {:.12}", vals[0]);
    println!("(p(0,1,0) s | s s) = {:.12}", vals[1]);
    println!("(p(0,0,1) s | s s) = {:.12}", vals[2]);
}
