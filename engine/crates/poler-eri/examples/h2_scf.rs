#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Decisive physics test: H2/STO-3G RHF energy at R = 1.4 bohr.
//!
//! Textbook reference: E_RHF(H2/STO-3G, R=1.4) = -1.1167 Ha
//! (Szasz "Elementary Methods", Liberles, standard course notes).
//!
//! Pipeline: STO-3G basis -> IntegralEngine (S, T, V) -> contracted ERI
//! tensor via shell_driver::eri_shell_full -> rhf_scf (DIIS).

use poler_eri::basis::sto3g_h;
use poler_eri::cart::ncart;
use poler_eri::engine::IntegralEngine;
use poler_eri::molecule::Molecule;
use poler_eri::scf::rhf_scf;
use poler_eri::shell_driver::eri_shell_full;
use poler_eri::types::{Atom, BasisSet, Point, QuartetData};

fn main() {
    // ── Sanity: (ss|ss) at origin with all exponents 1 -> pi^2.5/4 = 4.373355
    let o = Point([0.0, 0.0, 0.0]);
    let qd = QuartetData::new(o, o, o, o, 1.0, 1.0, 1.0, 1.0);
    let v = eri_shell_full(&qd, 0, 0, 0, 0);
    println!("sanity (ss|ss) = {:.6}  (expected 4.373355)", v[0]);
    assert!((v[0] - 4.373355).abs() < 1e-5, "ssss base value wrong");

    // ── H2 at R = 1.4 bohr
    let r = 1.4;
    let mol = Molecule::h2(r);
    let e_nuc = mol.nuclear_repulsion();
    println!("E_nuc = {:.6} Ha (expected {:.6})", e_nuc, 1.0 / r);

    // ── Basis: two normalized STO-3G s shells
    let mut s0 = sto3g_h();
    let mut s1 = sto3g_h();
    s0.atom_idx = 0;
    s1.atom_idx = 1;
    let basis = BasisSet {
        atoms: vec![
            Atom { charge: 1.0, coords: mol.atoms[0].coords },
            Atom { charge: 1.0, coords: mol.atoms[1].coords },
        ],
        shells: vec![s0, s1],
    };

    let engine = IntegralEngine::new(&basis);

    // Total number of Cartesian basis functions
    let n: usize = engine.shells.iter().map(|s| ncart(s.l)).sum();
    println!("n_basis = {}", n);

    // Shell offsets in the global basis index space
    let mut offs = Vec::with_capacity(engine.shells.len());
    let mut acc = 0usize;
    for s in &engine.shells {
        offs.push(acc);
        acc += ncart(s.l);
    }

    let s_mat = engine.overlap_matrix();
    let h_core = engine.core_hamiltonian();
    println!("S(0,0) = {:.6}  (must be 1.0)", s_mat[0]);
    println!("S(0,1) = {:.6}  (STO-3G ~0.6593)", s_mat[1]);

    // ── Full contracted ERI tensor, chemist ordering (mu nu | lam sig)
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

    // Reference values for H2/STO-3G (Szabo-Ostlund worked example, R=1.4):
    // (11|11) = 0.7746, (11|22) = 0.5697  [1-based labels -> 0-based idx 3]
    println!("(11|11) = {:.6}  (textbook 0.7746)", eri[0]);
    println!("(11|22) = {:.6}  (textbook 0.5697)", eri[3]);
    // Full tensor dump + 8-fold symmetry check
    let labels = ["1111", "1112", "1121", "1122", "1211", "1212", "1221", "1222",
                  "2111", "2112", "2121", "2122", "2211", "2212", "2221", "2222"];
    let mut max_asym = 0.0f64;
    for (i, lab) in labels.iter().enumerate() {
        let mu = i / 8; let nu = (i / 4) % 2; let lam = (i / 2) % 2; let sig = i % 2;
        print!("({})={:8.5}  ", lab, eri[i]);
        if i % 4 == 3 { println!(); }
        // permuted index (nu mu lam sig) and (lam sig mu nu) must match
        let p1 = nu * 8 + mu * 4 + lam * 2 + sig;
        let p2 = lam * 8 + sig * 4 + mu * 2 + nu;
        max_asym = max_asym.max((eri[i] - eri[p1]).abs()).max((eri[i] - eri[p2]).abs());
    }
    println!("max |8-fold symmetry violation| = {:.2e}", max_asym);
    assert!(max_asym < 1e-12, "ERI tensor not symmetric");

    run_scf(&s_mat, &h_core, &eri, n, e_nuc, "built-in rounded basis");

    // ── Exact standard STO-3G parameters (Hehre/Stewart/Pople 1969) ──────────
    let exact_alphas = vec![3.449250, 0.6239137, 0.1688554];
    let exact_coeffs = vec![0.15432897, 0.53532814, 0.44463454];
    let mut e0 = poler_eri::types::Shell::new(0, 0, exact_alphas.clone(), exact_coeffs.clone());
    let mut e1 = poler_eri::types::Shell::new(0, 1, exact_alphas, exact_coeffs);
    poler_eri::normalization::normalize_shell(&mut e0);
    poler_eri::normalization::normalize_shell(&mut e1);
    let basis_ex = BasisSet {
        atoms: vec![
            Atom { charge: 1.0, coords: mol.atoms[0].coords },
            Atom { charge: 1.0, coords: mol.atoms[1].coords },
        ],
        shells: vec![e0, e1],
    };
    let engine_ex = IntegralEngine::new(&basis_ex);
    let s_ex = engine_ex.overlap_matrix();
    let h_ex = engine_ex.core_hamiltonian();
    let eri_ex = contracted_eri(&engine_ex, n);
    println!();
    println!("exact-basis S(0,0) = {:.6}  (must be 1.0)", s_ex[0]);
    println!("exact-basis S(0,1) = {:.6}  (textbook 0.6593)", s_ex[1]);
    println!("exact-basis (11|11) = {:.6}  (textbook 0.7746)", eri_ex[0]);
    println!("exact-basis (11|22) = {:.6}  (textbook 0.5697)", eri_ex[n * n * n + n * n + n + 1]);

    run_scf(&s_ex, &h_ex, &eri_ex, n, e_nuc, "exact standard basis");
}

fn contracted_eri(engine: &IntegralEngine, n: usize) -> Vec<f64> {
    let mut offs = Vec::with_capacity(engine.shells.len());
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

fn run_scf(s_mat: &[f64], h_core: &[f64], eri: &[f64], n: usize, e_nuc: f64, label: &str) {
    let n_occ = 1usize; // 2 electrons, closed shell
    let res = rhf_scf(h_core, s_mat, eri, n, n_occ, e_nuc, 100, 1e-10, 1e-8);
    println!();
    println!("E_RHF(H2/STO-3G, R=1.4) [{}] = {:.6} Ha", label, res.energy);
    println!("converged = {}, iterations = {}", res.converged, res.n_iter);

    let reference = -1.1167;
    let diff = (res.energy - reference).abs();
    println!("reference = {} Ha, |diff| = {:.2e}", reference, diff);
    if diff < 5e-4 {
        println!("VERDICT: REAL QUANTUM CHEMISTRY — matches textbook");
    } else if diff < 5e-3 {
        println!("VERDICT: CLOSE — basis/contraction slight deviation");
    } else {
        println!("VERDICT: WRONG MATH — needs audit");
    }
}

#[allow(dead_code)]
fn dump_tensor() {}
