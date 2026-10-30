#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Отладка A3: поиск первого расхождения JIT vs REF на случайном поле.

use poler_engine::chem::dock::{prepare_ligand, PocketField};
use poler_engine::graph::chem_kernel::{compile_scoring_kernel, jit_scoring_reference};

fn synthetic_field(n: usize, seed: u64) -> PocketField {
    let mut s = seed;
    let lcg = |s: &mut u64| {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*s >> 11) as f64 / (1u64 << 53) as f64)
    };
    let mut pos = Vec::with_capacity(n);
    let mut q = Vec::with_capacity(n);
    let mut vdw = Vec::with_capacity(n);
    let mut acceptor = Vec::with_capacity(n);
    let mut nonpolar = Vec::with_capacity(n);
    for _ in 0..n {
        pos.push([lcg(&mut s) * 14.0 - 7.0, lcg(&mut s) * 14.0 - 7.0, lcg(&mut s) * 14.0 - 7.0]);
        let r = lcg(&mut s);
        q.push(if r < 0.3 { -0.5 } else if r < 0.6 { 0.4 } else { 0.0 });
        vdw.push(1.7 + lcg(&mut s));
        acceptor.push(r < 0.25);
        nonpolar.push(r >= 0.5);
    }
    let mut donor_h = Vec::new();
    for k in 0..n.min(30) {
        if acceptor[k] {
            donor_h.push((k, [pos[k][0] + 1.0, pos[k][1] + 0.2, pos[k][2] - 0.3]));
        }
    }
    PocketField { len: n, pos, q, vdw, acceptor, nonpolar, donor_h, src: (0..n).collect(), tree27: Default::default() }
}

fn main() {
    let lig = prepare_ligand("NC(=N)c1ccccc1").expect("лиганд");
    let field = synthetic_field(97, 11);
    let kernel = compile_scoring_kernel(&lig, &field).expect("компиляция");
    let mut s = 11u64 ^ 0xABCD;
    let lcg = |s: &mut u64| {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*s >> 11) as f64 / (1u64 << 53) as f64)
    };
    for pose_i in 0..25 {
        let c = [lcg(&mut s) * 4.0 - 2.0, lcg(&mut s) * 4.0 - 2.0, lcg(&mut s) * 4.0 - 2.0];
        let pos: Vec<[f64; 3]> = lig.base.iter().map(|b| [b[0] + c[0], b[1] + c[1], b[2] + c[2]]).collect();
        let j = kernel.score(&lig, &pos);
        let r = jit_scoring_reference(&lig, &field, &pos);
        if j.e_hb.to_bits() != r.e_hb.to_bits() || j.hbonds != r.hbonds {
            println!("поза #{pose_i}: c={:?}", c);
            println!("  JIT: e_hb={} hbonds={}", j.e_hb, j.hbonds);
            println!("  REF: e_hb={} hbonds={}", r.e_hb, r.hbonds);
            // детально: доноры лиганда
            for (di, &(dn, ref hs)) in lig.donors.iter().enumerate() {
                println!("  донор {di}: node {dn}, H nodes {:?}", hs);
            }
            // все акцепторы в 4.2 Å от доноров и их rha
            for (di, &(dn, ref hs)) in lig.donors.iter().enumerate() {
                let dp = pos[dn];
                for k in 0..field.len {
                    if !field.acceptor[k] { continue; }
                    let ap = field.pos[k];
                    let dx = ap[0]-dp[0]; let dy = ap[1]-dp[1]; let dz = ap[2]-dp[2];
                    let rda2 = dx*dx+dy*dy+dz*dz;
                    if rda2 > 17.64 { continue; }
                    let mut best = 0.0f64;
                    let mut best_h: Option<usize> = None;
                    for (hi, &h) in hs.iter().enumerate() {
                        let hp = pos[h];
                        let rha2 = (ap[0]-hp[0]).powi(2)+(ap[1]-hp[1]).powi(2)+(ap[2]-hp[2]).powi(2);
                        if rha2 > 12.25 || rha2 < 4.84 { continue; }
                        let v1 = [hp[0]-dp[0], hp[1]-dp[1], hp[2]-dp[2]];
                        let v2 = [ap[0]-hp[0], ap[1]-hp[1], ap[2]-hp[2]];
                        let n1 = (v1[0]*v1[0]+v1[1]*v1[1]+v1[2]*v1[2]).sqrt();
                        let n2 = (v2[0]*v2[0]+v2[1]*v2[1]+v2[2]*v2[2]).sqrt();
                        if n1 <= 1e-9 || n2 <= 1e-9 { continue; }
                        let dot = v1[0]*v2[0]+v1[1]*v2[1]+v1[2]*v2[2];
                        let cosphi = dot/(n1*n2);
                        let f = ((1.0-cosphi)*0.5).powi(2);
                        let e = -8.0*f;
                        if e < best { best = e; best_h = Some(hi); }
                    }
                    println!("    донор{di}→акц k={k}: rda2={:.3} best={:.4} H={:?}", rda2, best, best_h);
                }
            }
            break;
        }
    }
    println!("готово");
}
