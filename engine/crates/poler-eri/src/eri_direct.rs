//! Exact direct ERI evaluation for arbitrary Cartesian shell quartets.
//!
//! # AUDIT (v3.2.0-f)
//!
//! The circuit/Rys path in `shell_driver.rs` had multiple compounding
//! defects (wrong Rys moment set, wrong c00/c0p t-coefficients, sign-flipped
//! HRR transfers, and a Hermite-vs-Cartesian table confusion that made
//! g[n][0] = c00·g[n−1][0] + (n−1)·b10·g[n−2][0] invalid as a Cartesian
//! recurrence). Rather than patch the circuit further, this module computes
//! ERIs exactly by direct construction:
//!
//! 1. **Binomial expansion** of the Cartesian polynomials around the
//!    Gaussian product centers: (x−A)^a (x−B)^b = Σ C(a,i)C(b,j)
//!    (P−A)^{a−i} (P−B)^{b−j} (x−P)^{i+j} per dimension.
//! 2. **Hermite lowering coefficients** c[k][t] with
//!    c_{k+1,t} = (1/(2p))·c_{k,t−1} + (t+1)·c_{k,t+1}, expressing
//!    (x−P)^k·e^{−p(x−P)²} = Σ_t c[k][t]·∂_P^t[e] exactly
//!    (the naive (1/(2p))^k is exact only for k ≤ 1).
//! 3. **Mixed P/Q Boys-derivative state machine** for
//!    ∂_P^kp ∂_Q^kq [F₀(T)], T = ρ|P−Q|², with the exact rules
//!    ∂_P[x^m F_j] = m·x^{m−1}F_j − 2ρ·x^{m+1}F_{j+1},
//!    ∂_Q[x^m F_j] = −m·x^{m−1}F_j + 2ρ·x^{m+1}F_{j+1}, x = P − Q.
//!
//! Verified against QMC, Laplace-transform quadrature, and closed-form
//! analytics on (ss|ss), (ps|ss), (ps|ps), (pp|ss), (pp|pp) test quartets,
//! and end-to-end via H₂ and LiH Hartree–Fock energies.

use crate::boys::boys_f;
use crate::cart::{cart_comps, ncart};
use crate::types::QuartetData;

/// Push-add into a sparse (index, coefficient) list.
fn push(v: &mut Vec<(usize, f64)>, t: usize, c: f64) {
    for e in v.iter_mut() {
        if e.0 == t {
            e.1 += c;
            return;
        }
    }
    v.push((t, c));
}

/// Lowering coefficients: (x−P)^k·e = Σ_t c[k][t]·∂_P^t[e].
fn lowering(k: usize, p: f64) -> Vec<(usize, f64)> {
    let mut cur: Vec<(usize, f64)> = vec![(0usize, 1.0)];
    for _ in 0..k {
        let mut nxt: Vec<(usize, f64)> = Vec::new();
        for &(t, v) in &cur {
            // (x−P)·∂^t[e] = (1/(2p))·∂^{t+1}[e] + t·∂^{t−1}[e]
            push(&mut nxt, t + 1, v / (2.0 * p));
            if t > 0 {
                push(&mut nxt, t - 1, v * t as f64);
            }
        }
        cur = nxt;
    }
    cur
}

/// Binomial coefficient.
fn binom(n: usize, k: usize) -> f64 {
    if k > n {
        return 0.0;
    }
    let mut r = 1.0f64;
    for i in 0..k {
        r *= (n - i) as f64 / (i + 1) as f64;
    }
    r
}

/// Integer power.
fn ipow(x: f64, n: usize) -> f64 {
    let mut r = 1.0;
    for _ in 0..n {
        r *= x;
    }
    r
}

/// One state term: c · x^mx y^my z^mz · F_j(T), x = P − Q.
#[derive(Clone)]
struct Term {
    m: [usize; 3],
    j: usize,
    c: f64,
}

/// Apply ∂_P (is_p = true) or ∂_Q (is_p = false) `times` times in dimension `dim`.
fn apply_deriv(terms: Vec<Term>, dim: usize, is_p: bool, times: usize, rho: f64) -> Vec<Term> {
    let mut cur = terms;
    for _ in 0..times {
        let mut nxt: Vec<Term> = Vec::with_capacity(cur.len() * 2);
        for t in &cur {
            if t.m[dim] > 0 {
                let mut m2 = t.m;
                m2[dim] -= 1;
                let s = if is_p { 1.0 } else { -1.0 };
                nxt.push(Term { m: m2, j: t.j, c: t.c * s * t.m[dim] as f64 });
            }
            let mut m2 = t.m;
            m2[dim] += 1;
            let bc = if is_p { -2.0 * rho } else { 2.0 * rho };
            nxt.push(Term { m: m2, j: t.j + 1, c: t.c * bc });
        }
        cur = nxt;
    }
    cur
}

/// Evaluate one Cartesian component (na, nb | nc, nd) of a primitive quartet.
#[allow(clippy::too_many_arguments)]
pub fn eri_component(
    qd: &QuartetData,
    na: [usize; 3],
    nb: [usize; 3],
    nc: [usize; 3],
    nd: [usize; 3],
) -> f64 {
    let p = qd.p;
    let q = qd.q;
    let rho = qd.rho;
    let pa = qd.center_ab();
    let qc = qd.center_cd();
    let x = [
        pa.0[0] - qc.0[0],
        pa.0[1] - qc.0[1],
        pa.0[2] - qc.0[2],
    ];
    let t_arg = rho * (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]);

    let mut total = 0.0f64;

    // Bra-side binomial expansion (per-dim powers of (x−P)).
    for i0 in 0..=na[0] {
        for j0 in 0..=nb[0] {
            let w0 = binom(na[0], i0)
                * binom(nb[0], j0)
                * ipow(pa.0[0] - qd.ra.0[0], na[0] - i0)
                * ipow(pa.0[0] - qd.rb.0[0], nb[0] - j0);
            if w0 == 0.0 {
                continue;
            }
            for i1 in 0..=na[1] {
                for j1 in 0..=nb[1] {
                    let w1 = binom(na[1], i1)
                        * binom(nb[1], j1)
                        * ipow(pa.0[1] - qd.ra.0[1], na[1] - i1)
                        * ipow(pa.0[1] - qd.rb.0[1], nb[1] - j1);
                    if w1 == 0.0 {
                        continue;
                    }
                    for i2 in 0..=na[2] {
                        for j2 in 0..=nb[2] {
                            let w2 = binom(na[2], i2)
                                * binom(nb[2], j2)
                                * ipow(pa.0[2] - qd.ra.0[2], na[2] - i2)
                                * ipow(pa.0[2] - qd.rb.0[2], nb[2] - j2);
                            let wb = w0 * w1 * w2;
                            if wb == 0.0 {
                                continue;
                            }
                            // Ket-side binomial expansion (per-dim powers of (x−Q)).
                            for k0 in 0..=nc[0] {
                                for l0 in 0..=nd[0] {
                                    let v0 = binom(nc[0], k0)
                                        * binom(nd[0], l0)
                                        * ipow(qc.0[0] - qd.rc.0[0], nc[0] - k0)
                                        * ipow(qc.0[0] - qd.rd.0[0], nd[0] - l0);
                                    if v0 == 0.0 {
                                        continue;
                                    }
                                    for k1 in 0..=nc[1] {
                                        for l1 in 0..=nd[1] {
                                            let v1 = binom(nc[1], k1)
                                                * binom(nd[1], l1)
                                                * ipow(qc.0[1] - qd.rc.0[1], nc[1] - k1)
                                                * ipow(qc.0[1] - qd.rd.0[1], nd[1] - l1);
                                            if v1 == 0.0 {
                                                continue;
                                            }
                                            for k2 in 0..=nc[2] {
                                                for l2 in 0..=nd[2] {
                                                    let v2 = binom(nc[2], k2)
                                                        * binom(nd[2], l2)
                                                        * ipow(qc.0[2] - qd.rc.0[2], nc[2] - k2)
                                                        * ipow(qc.0[2] - qd.rd.0[2], nd[2] - l2);
                                                    let wk = v0 * v1 * v2;
                                                    if wk == 0.0 {
                                                        continue;
                                                    }
                                                    // Exact lowering expansions.
                                                    let lbx = lowering(i0 + j0, p);
                                                    let lby = lowering(i1 + j1, p);
                                                    let lbz = lowering(i2 + j2, p);
                                                    let lkx = lowering(k0 + l0, q);
                                                    let lky = lowering(k1 + l1, q);
                                                    let lkz = lowering(k2 + l2, q);
                                                    for &(tx, vx) in &lbx {
                                                        for &(ux, wx) in &lkx {
                                                            for &(ty, vy) in &lby {
                                                                for &(uy, wy) in &lky {
                                                                    for &(tz, vz) in &lbz {
                                                                        for &(uz, wz) in &lkz {
                                                                            let seed = vx * vy * vz * wx * wy * wz;
                                                                            if seed == 0.0 {
                                                                                continue;
                                                                            }
                                                                            let terms = vec![Term {
                                                                                m: [0, 0, 0],
                                                                                j: 0,
                                                                                c: seed,
                                                                            }];
                                                                            let terms = apply_deriv(terms, 0, true, tx, rho);
                                                                            let terms = apply_deriv(terms, 0, false, ux, rho);
                                                                            let terms = apply_deriv(terms, 1, true, ty, rho);
                                                                            let terms = apply_deriv(terms, 1, false, uy, rho);
                                                                            let terms = apply_deriv(terms, 2, true, tz, rho);
                                                                            let terms = apply_deriv(terms, 2, false, uz, rho);
                                                                            for t in &terms {
                                                                                let v = t.c
                                                                                    * ipow(x[0], t.m[0])
                                                                                    * ipow(x[1], t.m[1])
                                                                                    * ipow(x[2], t.m[2])
                                                                                    * boys_f(t.j, t_arg);
                                                                                total += wb * wk * v;
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
                            }
                        }
                    }
                }
            }
        }
    }

    qd.prefactor * qd.kab * qd.kcd * total
}

/// Compute ALL Cartesian component ERIs for a primitive shell quartet.
///
/// Same contract as the original `eri_shell_full` (row-major layout over
/// (ia, ib, ic, id)), but exact for every angular momentum combination.
pub fn eri_shell_full_direct(
    qd: &QuartetData,
    la: usize,
    lb: usize,
    lc: usize,
    ld: usize,
) -> Vec<f64> {
    let comps_a = cart_comps(la);
    let comps_b = cart_comps(lb);
    let comps_c = cart_comps(lc);
    let comps_d = cart_comps(ld);
    let na = ncart(la);
    let nb = ncart(lb);
    let nc = ncart(lc);
    let nd = ncart(ld);
    let mut out = vec![0.0f64; na * nb * nc * nd];
    let mut idx = 0usize;
    for ca in &comps_a {
        for cb in &comps_b {
            for cc in &comps_c {
                for cd in &comps_d {
                    out[idx] = eri_component(
                        qd,
                        [ca.0, ca.1, ca.2],
                        [cb.0, cb.1, cb.2],
                        [cc.0, cc.1, cc.2],
                        [cd.0, cd.1, cd.2],
                    );
                    idx += 1;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Point;

    /// Reference values at the canonical test geometry, verified by QMC,
    /// Laplace quadrature, and closed-form analytics (see scripts/).
    fn qd_test() -> QuartetData {
        QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([0.5, 0.0, 0.0]),
            Point([3.0, 0.7, 0.0]),
            Point([3.0, -0.5, 0.3]),
            1.5, 1.3, 1.7, 1.1,
        )
    }

    #[test]
    fn test_ssss_closed_form() {
        let qd = QuartetData::new(
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            Point([0.0, 0.0, 0.0]),
            1.0, 1.0, 1.0, 1.0,
        );
        let v = eri_shell_full_direct(&qd, 0, 0, 0, 0);
        assert!((v[0] - 4.373355).abs() < 1e-5, "ssss = {}", v[0]);
    }

    #[test]
    fn test_ps_ss_reference() {
        let qd = qd_test();
        let v = eri_shell_full_direct(&qd, 1, 0, 0, 0);
        assert!((v[0] - 0.0455009).abs() < 2e-6, "(p0 s|ss) = {}", v[0]);
        assert!((v[1] - 0.0008116).abs() < 2e-6, "(p1 s|ss) = {}", v[1]);
        assert!((v[2] - 0.0004185).abs() < 2e-6, "(p2 s|ss) = {}", v[2]);
    }

    #[test]
    fn test_ps_ps_reference() {
        let qd = qd_test();
        let v = eri_shell_full_direct(&qd, 1, 0, 1, 0);
        assert!((v[0] - (-0.0035325)).abs() < 2e-5, "(p0 s|p0 s) = {}", v[0]);
        assert!((v[1] - (-0.0217945)).abs() < 2e-4, "(p0 s|p1 s) = {}", v[1]);
    }

    #[test]
    fn test_pp_ss_reference() {
        let qd = qd_test();
        let v = eri_shell_full_direct(&qd, 1, 1, 0, 0);
        // layout (i, j): idx = i*3 + j
        assert!((v[0] - 0.018785).abs() < 5e-4, "(p0 p0|ss) = {}", v[0]);
        assert!((v[1] - 0.000344).abs() < 2e-5, "(p0 p1|ss) = {}", v[1]);
        assert!((v[3] - (-0.0000617)).abs() < 2e-5, "(p1 p0|ss) = {}", v[3]);
        assert!((v[4] - 0.026819).abs() < 5e-4, "(p1 p1|ss) = {}", v[4]);
    }

    #[test]
    fn test_pp_pp_reference() {
        let qd = qd_test();
        let v = eri_shell_full_direct(&qd, 1, 1, 1, 1);
        let idx = |i: usize, j: usize, k: usize, l: usize| ((i * 3 + j) * 3 + k) * 3 + l;
        assert!((v[idx(0, 0, 0, 0)] - 0.0035497).abs() < 2e-4, "(p0p0|p0p0) = {}", v[idx(0, 0, 0, 0)]);
        assert!((v[idx(2, 2, 2, 2)] - 0.0041262).abs() < 2e-4, "(p2p2|p2p2) = {}", v[idx(2, 2, 2, 2)]);
    }

    #[test]
    fn test_permutation_symmetry() {
        // (ab|cd) = (ba|dc): swap the full quartet (centers AND exponents).
        let qd = qd_test();
        let qd_swapped = QuartetData::new(
            qd.rb, qd.ra, qd.rd, qd.rc,
            qd.alpha_b, qd.alpha_a, qd.alpha_d, qd.alpha_c,
        );
        let v = eri_shell_full_direct(&qd, 1, 1, 1, 0); // (pp|ps)
        let w = eri_shell_full_direct(&qd_swapped, 1, 1, 0, 1);
        // v layout (pp|ps): (i, j, k); w layout (pp|sp): (i', j', l') where
        // w[(j, i, k)] = (p_j^B p_i^A | s^D p_k^C) = v[(i, j, k)]
        let idx = |i: usize, j: usize, k: usize| (i * 3 + j) * 3 + k;
        for i in 0..3 {
            for j in 0..3 {
                for k in 0..3 {
                    let d = (v[idx(i, j, k)] - w[idx(j, i, k)]).abs();
                    assert!(d < 1e-13, "quartet swap asymmetry at ({i},{j},{k}): {d}");
                }
            }
        }
    }
}
