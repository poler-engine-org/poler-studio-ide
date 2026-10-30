#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Проба: точная копия обёртки src/quantum/eri.rs::hartree_fock внутри крейта —
//! для сверки с независимым эталоном PySCF (scripts/rhf_reference_pyscf.py)
//! без сборки всего движка.
use poler_eri::basis::sto3g_element;
use poler_eri::cart::ncart;
use poler_eri::engine::IntegralEngine;
use poler_eri::molecule::Molecule;
use poler_eri::normalization::{normalize_contraction, normalize_shell};
use poler_eri::scf::rhf_scf;
use poler_eri::shell_driver::eri_shell_full;
use poler_eri::types::{Atom, BasisSet, Point, QuartetData, Shell};

const BOHR_PER_ANG: f64 = 1.8897259886;

fn hf_for(atoms: &[(&str, [f64; 3])], charge: i32) -> Result<(f64, bool, usize, usize), String> {
    let mol = Molecule::new(
        atoms
            .iter()
            .map(|(el, xyz)| Atom {
                charge: poler_eri::basis::element_number(el) as f64,
                coords: Point(*xyz),
            })
            .collect(),
        charge as f64,
        1,
    );
    let n_electrons = mol.n_electrons();
    let n_occ = n_electrons / 2;
    let e_nuc = mol.nuclear_repulsion();

    let mut shells = Vec::new();
    let mut atom_index = 0usize;
    for (el, _) in atoms {
        for raw in sto3g_element(el).ok_or_else(|| format!("нет базиса для {el}"))? {
            let mut shell = Shell::new(raw.l, atom_index, raw.alphas, raw.coeffs);
            normalize_shell(&mut shell);
            normalize_contraction(&mut shell);
            shells.push(shell);
        }
        atom_index += 1;
    }
    let basis = BasisSet {
        atoms: atoms
            .iter()
            .map(|(el, xyz)| Atom {
                charge: poler_eri::basis::element_number(el) as f64,
                coords: Point(*xyz),
            })
            .collect(),
        shells,
    };
    let engine = IntegralEngine::new(&basis);
    let n: usize = engine.shells.iter().map(|s| ncart(s.l)).sum();

    let mut offs = Vec::with_capacity(engine.shells.len());
    let mut acc = 0usize;
    for s in &engine.shells {
        offs.push(acc);
        acc += ncart(s.l);
    }

    let s_mat = engine.overlap_matrix();
    let h_core = engine.core_hamiltonian();

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

    let res = rhf_scf(&h_core, &s_mat, &eri, n, n_occ, e_nuc, 200, 1e-10, 1e-8);
    Ok((res.energy, res.converged, res.n_iter, n))
}

fn geom(name: &str) -> Vec<(&'static str, [f64; 3])> {
    match name {
        "H2" => vec![("H", [-0.7, 0.0, 0.0]), ("H", [0.7, 0.0, 0.0])],
        "He" => vec![("He", [0.0, 0.0, 0.0])],
        "LiH" => vec![("Li", [0.0, 0.0, 0.0]), ("H", [3.0, 0.0, 0.0])],
        "H2O" => {
            let r_oh = 0.9572 * BOHR_PER_ANG;
            let half = 0.5 * 104.52_f64.to_radians();
            let (s, c) = (half.sin(), half.cos());
            vec![
                ("O", [0.0, 0.0, 0.0]),
                ("H", [r_oh * s, r_oh * c, 0.0]),
                ("H", [-r_oh * s, r_oh * c, 0.0]),
            ]
        }
        "NH3" => {
            let r_nh = 1.0116 * BOHR_PER_ANG;
            let half = 0.5 * 106.7_f64.to_radians();
            let (s, c) = (half.sin(), half.cos());
            let mut g = vec![("N", [0.0, 0.0, 0.0])];
            for k in 0..3 {
                let phi = (k as f64) * std::f64::consts::TAU / 3.0;
                g.push(("H", [r_nh * s * phi.cos(), r_nh * s * phi.sin(), r_nh * c]));
            }
            g
        }
        "CH4" => {
            let r_ch = 1.0870 * BOHR_PER_ANG;
            let dirs: [[f64; 3]; 4] = [
                [1.0, 1.0, 1.0],
                [1.0, -1.0, -1.0],
                [-1.0, 1.0, -1.0],
                [-1.0, -1.0, 1.0],
            ];
            let mut g = vec![("C", [0.0, 0.0, 0.0])];
            for d in dirs {
                let nrm = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                g.push(("H", [r_ch * d[0] / nrm, r_ch * d[1] / nrm, r_ch * d[2] / nrm]));
            }
            g
        }
        "HF" => {
            let r = 0.9168 * BOHR_PER_ANG;
            vec![("F", [0.0, 0.0, 0.0]), ("H", [r, 0.0, 0.0])]
        }
        "CO" => {
            let r = 1.128 * BOHR_PER_ANG;
            vec![("C", [0.0, 0.0, 0.0]), ("O", [r, 0.0, 0.0])]
        }
        _ => panic!("нет геометрии {name}"),
    }
}

#[test]
fn probe_vs_pyscf() {
    // Эталон PySCF 2.14 (scripts/rhf_reference_pyscf.py, BSE STO-3G v1)
    let reference = [
        ("H2", -1.116714325),
        ("He", -2.807783957),
        ("LiH", -7.862246324),
        ("H2O", -74.962928261),
        ("NH3", -55.426271142),
        ("CH4", -39.726810114),
        ("HF", -98.570757656),
        ("CO", -111.224558690),
    ];
    for (name, ref_e) in reference {
        let (e, conv, iters, n) = hf_for(&geom(name), 0).unwrap();
        println!(
            "{name:4} engine={e:>15.9} pyscf={ref_e:>15.9} d={:>12.3e} conv={} it={} n={}",
            e - ref_e, conv, iters, n
        );
        assert!(conv, "{name}: SCF не сошёлся");
        assert!(
            (e - ref_e).abs() < 1e-6,
            "{name}: E = {e} против PySCF {ref_e} (Δ = {})",
            e - ref_e
        );
        assert!(iters <= 200);
        assert!(n > 0);
    }
}
