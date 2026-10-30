#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Verify Rys roots/weights: defining property sum_k w_k x_k^(2m) = F_m(T).
use poler_eri::eri_rys::rys_roots_weights;

fn main() {
    for t in [0.5, 2.5, 9.375] {
        for n in 1..=4usize {
            let (roots, weights) = rys_roots_weights(t, n);
            println!("T={} n={}", t, n);
            for i in 0..n {
                println!("  root[{}] = {:.12}  w[{}] = {:.12}", i, roots[i], i, weights[i]);
            }
        }
    }
}
