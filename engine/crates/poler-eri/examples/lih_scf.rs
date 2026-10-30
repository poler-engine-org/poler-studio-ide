#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! End-to-end p-shell test: LiH/STO-3G RHF at R = 3.0 bohr.
//!
//! Literature region: E_RHF(LiH/STO-3G, R~3.0) ≈ -7.86 Ha.
//! Exercises (pp|pp), (pp|ss), (ps|ps), (ss|pp) and HRR paths.

use poler_eri::cart::ncart;
use poler_eri::engine::IntegralEngine;
use poler_eri::molecule::Molecule;
use poler_eri::normalization::{normalize_contraction, normalize_shell};
use poler_eri::scf::rhf_scf;
use poler_eri::shell_driver::eri_shell_full;
use poler_eri::types::{Atom, BasisSet, Point, QuartetData, Shell};

fn contracted_eri(engine: &IntegralEngine, n: usize) -> Vec<f64> {
    let mut offs = Vec::new();
    let mut acc = 0usize;
    for s in &engine.shells {
        offs.push(acc);
        acc += ncart(s.l);
    }
    let mut eri = vec![0.0f64; n * n * n * n];
    for (ia, sa) in engine.shells.iter().enumerate() {
        for (ib, sb) in engine.shells.iter().enumerate() {
            for (ic, sc) in engine.shells.iter().enumerate() {
                for (id, sd) in engine.shells.iter().enumerate() {
                    let ra = engine.coords[ia];
                    let rb = engine.coords[ib];
                    let rc = engine.coords[ic];
                    let rd = engine.coords[id];
                    for pa in 0..sa.n_prim() {
                        for pb in 0..sb.n_prim() {
                            for pc in 0..sc.n_prim() {
                                for pd in 0..sd.n_prim() {
                                    let qd = QuartetData::new(
                                        ra, rb, rc, rd,
                                        sa.alphas[pa], sb.alphas[pb],
                                        sc.alphas[pc], sd.alphas[pd],
                                    );
                                    let vals = eri_shell_full(&qd, sa.l, sb.l, sc.l, sd.l);
                                    let coef = sa.norm_coeffs[pa]
                                        * sb.norm_coeffs[pb]
                                        * sc.norm_coeffs[pc]
                                        * sd.norm_coeffs[pd];
                                    let nca = ncart(sa.l);
                                    let ncb = ncart(sb.l);
                                    let ncc = ncart(sc.l);
                                    let ncd = ncart(sd.l);
                                    let mut idx = 0usize;
                                    for i in 0..nca {
                                        for j in 0..ncb {
                                            for k in 0..ncc {
                                                for l in 0..ncd {
                                                    let mu = offs[ia] + i;
                                                    let nu = offs[ib] + j;
                                                    let lam = offs[ic] + k;
                                                    let sig = offs[id] + l;
                                                    eri[mu * n * n * n + nu * n * n + lam * n + sig] +=
                                                        coef * vals[idx];
                                                    idx += 1;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    eri
}

fn main() {
    // ── Standard STO-3G (Hehre-Stewart-Pople 1969; Li zeta scaling) ─────────
    let h_alphas = vec![3.449250, 0.6239137, 0.1688554];
    let h_coeffs = vec![0.15432897, 0.53532814, 0.44463454];
    let li_1s_alphas = vec![16.1195950, 2.9362007, 0.7949132];
    let li_1s_coeffs = vec![0.15432897, 0.53532814, 0.44463454];
    let li_sp_alphas = vec![0.6366994, 0.1479427, 0.0482660];
    let li_2s_coeffs = vec![-0.09996723, 0.39951283, 0.70011547];
    let li_2p_coeffs = vec![0.15591627, 0.60768372, 0.39195757];

    let r = 3.0; // Li-H distance in bohr
    let mol = Molecule::new(
        vec![
            Atom { charge: 3.0, coords: Point([0.0, 0.0, 0.0]) },
            Atom { charge: 1.0, coords: Point([r, 0.0, 0.0]) },
        ],
        0.0,
        1,
    );
    let e_nuc = mol.nuclear_repulsion();
    println!("LiH  E_nuc = {:.6} Ha (3/3.0 = 1.0)", e_nuc);

    let mut li1 = Shell::new(0, 0, li_1s_alphas, li_1s_coeffs);
    normalize_shell(&mut li1);
    normalize_contraction(&mut li1);
    let mut li2s = Shell::new(0, 0, li_sp_alphas.clone(), li_2s_coeffs);
    normalize_shell(&mut li2s);
    normalize_contraction(&mut li2s);
    let mut li2p = Shell::new(1, 0, li_sp_alphas, li_2p_coeffs);
    normalize_shell(&mut li2p);
    normalize_contraction(&mut li2p);
    let mut hs = Shell::new(0, 1, h_alphas, h_coeffs);
    normalize_shell(&mut hs);
    normalize_contraction(&mut hs);

    let basis = BasisSet {
        atoms: vec![
            Atom { charge: 3.0, coords: Point([0.0, 0.0, 0.0]) },
            Atom { charge: 1.0, coords: Point([r, 0.0, 0.0]) },
        ],
        shells: vec![li1, li2s, li2p, hs],
    };
    let engine = IntegralEngine::new(&basis);
    let n: usize = engine.shells.iter().map(|s| ncart(s.l)).sum();
    println!("n_basis = {} (Li:1s,2s,2px,2py,2pz + H:1s)", n);

    let s_mat = engine.overlap_matrix();
    println!("S(0,0) = {:.6} (must be 1.0 after contraction norm)", s_mat[0]);
    println!("S(1,1) = {:.6}", s_mat[n + 1]);
    println!("S(0,3) = Li1s-H1s = {:.6}", s_mat[3]);

    println!("S matrix:");
    for i in 0..n {
        let mut row = String::new();
        for j in 0..n {
            row.push_str(&format!("{:9.5} ", s_mat[i * n + j]));
        }
        println!("  {}", row);
    }

    let h_core = engine.core_hamiltonian();
    let eri = contracted_eri(&engine, n);


    // dump for independent Python SCF
    if std::env::var("DUMP_MATS").is_ok() {
        println!("MAT_S");
        for i in 0..n { for j in 0..n { print!("{:.16e} ", s_mat[i*n+j]); } println!(); }
        println!("MAT_H");
        for i in 0..n { for j in 0..n { print!("{:.16e} ", h_core[i*n+j]); } println!(); }
        println!("MAT_ERI");
        for i in 0..n*n*n*n { print!("{:.16e} ", eri[i]); } println!();
        println!("E_NUC {:.16e}", e_nuc);
    }
    let n_occ = 2usize; // 4 electrons
    let res = rhf_scf(&h_core, &s_mat, &eri, n, n_occ, e_nuc, 200, 1e-10, 1e-8);
    println!();
    println!("E_RHF(LiH/STO-3G, R=3.0) = {:.6} Ha", res.energy);
    println!("converged = {}, iterations = {}", res.converged, res.n_iter);
    println!("literature region: ~ -7.86 Ha");
}
