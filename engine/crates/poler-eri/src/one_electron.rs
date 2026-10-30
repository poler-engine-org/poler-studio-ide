//! Exact Cartesian one-electron integrals (S, T, V) for arbitrary angular momentum.
//!
//! # AUDIT (v3.2.0-d)
//!
//! The original `IntegralEngine` filled ALL Cartesian components of a shell
//! pair with the s-type primitive value — a placeholder that is only correct
//! for l = 0 (H2 worked; LiH produced S(p,p) = 0.63 and identical p-rows).
//! This module computes the real per-component integrals:
//!
//! * **Overlap** — binomial expansion of (x−A)^a (x−B)^b around the Gaussian
//!   product center P, with exact 1D moments
//!   `M(k) = (k−1)!! √(π/p) / (2p)^{k/2}` (odd k → 0).
//! * **Kinetic** — the exact operator identity
//!   `∇²φ_b = [4β²|r−B|² − (6+4Σb_d)β] φ_b + Σ_d b_d(b_d−1) φ_{b−2e_d}`,
//!   which reduces T entirely to overlap integrals.
//!
//! # AUDIT (v3.3.0 — «медицинская находка» v0.74.0)
//!
//! The v3.2.0-d nuclear attraction used a **naive raw-power Hermite trick**:
//! `(x−P) e^{−p(r−P)²} = (1/(2p)) ∂_P e^{…}` raised to power k for the moment
//! `(x−P)^k`. That identity is only valid to FIRST order: differentiating
//! twice brings a product-rule term (`∂²_P e = 4p²(x−P)² e − 2p e`), so the
//! pure `(1/(2p))^k ∂^k_P` dictionary silently drops the `−2p·e` part.
//! Symptom: V(p,p) came out **positive** (+4.43 Ha for O 2p in H2O where the
//! truth is −10.14 Ha) — every molecule with occupied p orbitals was
//! catastrophically wrong (H2O: −44.09 vs −74.96 Ha; HF: −26.7 vs −98.6 Ha),
//! while pure-s systems (H2, He) matched PySCF to 12 digits. The LiH test of
//! v0.73.0 had locked in the bug (−7.810054 instead of −7.862246).
//!
//! **Fix**: proper McMurchie–Davidson expansion in Hermite Gaussians
//! `D_t = ∂^t_u [e^{−p u²}]`, whose multiplication rule
//! `u·D_t = −D_{t+1}/(2p) − t·D_{t−1}`
//! generates the E-coefficient recursion
//! `E'[s] = X·E[s] − E[s−1]/(2p) − (s+1)·E[s+1]`, and the Coulomb auxiliary
//! integrals `R_{tuv} = (−1)^{t+u+v} ∂^t_x ∂^u_y ∂^v_z [F₀(p|P−C|²)]`.
//! Analytic proof case ⟨px|−Z/|r−C||px⟩ at one center: naive → +Zπ/(3p²),
//! MD → −2πZ/(3p²) = exact spherical result.
//!
//! All three reproduce the proven s-type seeds (`overlap_ss`, `kinetic_ss`,
//! `nuclear_ss`) exactly when a = b = (0,0,0).

use crate::boys::boys_f;
use crate::types::Point;
use std::f64::consts::PI;

/// Double factorial (k−1)!! as f64 (k odd or even alike; caller passes k).
fn dfact(k: usize) -> f64 {
    if k == 0 {
        return 1.0;
    }
    let mut r = 1.0f64;
    let mut m = k;
    while m > 0 {
        r *= m as f64;
        if m <= 2 {
            break;
        }
        m -= 2;
    }
    r
}

/// 1D moment of the centered Gaussian: M(k) = ∫ (x−P)^k e^{−p(x−P)²} dx.
fn moment(k: usize, p: f64) -> f64 {
    if k % 2 == 1 {
        return 0.0;
    }
    dfact(k.saturating_sub(1)) * libm::sqrt(PI / p) / libm::pow(2.0 * p, (k / 2) as f64)
}

/// Binomial coefficient C(n, k).
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

/// Integer power of f64 (exact for the small exponents used here).
fn ipow(x: f64, n: usize) -> f64 {
    let mut r = 1.0;
    for _ in 0..n {
        r *= x;
    }
    r
}

/// Gaussian product center P = (αA + βB)/p.
fn product_center(ra: &Point, rb: &Point, alpha: f64, beta: f64) -> [f64; 3] {
    let p = alpha + beta;
    [
        (alpha * ra.0[0] + beta * rb.0[0]) / p,
        (alpha * ra.0[1] + beta * rb.0[1]) / p,
        (alpha * ra.0[2] + beta * rb.0[2]) / p,
    ]
}

/// Primitive Cartesian overlap (unnormalized):
/// `∫ (x−A)^{a0}(y−A)^{a1}(z−A)^{a2} · (x−B)^{b0}… e^{−α r_A² − β r_B²} d³r`.
pub fn overlap_cartesian(
    ra: &Point,
    rb: &Point,
    alpha: f64,
    beta: f64,
    a: [usize; 3],
    b: [usize; 3],
) -> f64 {
    let p = alpha + beta;
    let rab2 = ra.dist2(rb);
    let kab = libm::exp(-alpha * beta * rab2 / p);
    let pc = product_center(ra, rb, alpha, beta);

    let mut total = kab;
    for dim in 0..3 {
        let pa_d = pc[dim] - ra.0[dim];
        let pb_d = pc[dim] - rb.0[dim];
        let mut s = 0.0;
        for i in 0..=a[dim] {
            for j in 0..=b[dim] {
                s += binom(a[dim], i)
                    * binom(b[dim], j)
                    * ipow(pa_d, a[dim] - i)
                    * ipow(pb_d, b[dim] - j)
                    * moment(i + j, p);
            }
        }
        total *= s;
    }
    total
}

/// Primitive Cartesian kinetic energy:
/// `−½ ⟨a| ∇² |b⟩` via the exact operator identity (see module docs).
pub fn kinetic_cartesian(
    ra: &Point,
    rb: &Point,
    alpha: f64,
    beta: f64,
    a: [usize; 3],
    b: [usize; 3],
) -> f64 {
    let mut sum_plus = 0.0;
    for d in 0..3 {
        let mut b2 = b;
        b2[d] += 2;
        sum_plus += overlap_cartesian(ra, rb, alpha, beta, a, b2);
    }
    let s = overlap_cartesian(ra, rb, alpha, beta, a, b);
    let mut sum_minus = 0.0;
    for d in 0..3 {
        if b[d] >= 2 {
            let mut b2 = b;
            b2[d] -= 2;
            sum_minus += b[d] as f64 * (b[d] - 1) as f64
                * overlap_cartesian(ra, rb, alpha, beta, a, b2);
        }
    }
    let lsum = (b[0] + b[1] + b[2]) as f64;
    -0.5 * (4.0 * beta * beta * sum_plus - (6.0 + 4.0 * lsum) * beta * s + sum_minus)
}

/// One term of the symbolic derivative state: c · x^mx y^my z^mz F_j(T).
#[derive(Clone, Debug)]
struct BoysTerm {
    mx: usize,
    my: usize,
    mz: usize,
    j: usize,
    c: f64,
}

/// Evaluate ∂_x^{kx} ∂_y^{ky} ∂_z^{kz} [F₀(T)] at T = p|P−C|²,
/// with x ≡ Pₓ−Cₓ etc., via the exact structural rule
/// ∂_x [x^m F_j] = m·x^{m−1} F_j − 2p·x^{m+1} F_{j+1}.
fn boys_derivative(kx: usize, ky: usize, kz: usize, p: f64) -> Vec<BoysTerm> {
    let mut terms = vec![BoysTerm { mx: 0, my: 0, mz: 0, j: 0, c: 1.0 }];
    // apply ∂_x kx times
    for _ in 0..kx {
        let mut next = Vec::with_capacity(terms.len() * 2);
        for t in &terms {
            if t.mx > 0 {
                next.push(BoysTerm { mx: t.mx - 1, my: t.my, mz: t.mz, j: t.j, c: t.c * t.mx as f64 });
            }
            next.push(BoysTerm { mx: t.mx + 1, my: t.my, mz: t.mz, j: t.j + 1, c: t.c * (-2.0 * p) });
        }
        terms = next;
    }
    for _ in 0..ky {
        let mut next = Vec::with_capacity(terms.len() * 2);
        for t in &terms {
            if t.my > 0 {
                next.push(BoysTerm { mx: t.mx, my: t.my - 1, mz: t.mz, j: t.j, c: t.c * t.my as f64 });
            }
            next.push(BoysTerm { mx: t.mx, my: t.my + 1, mz: t.mz, j: t.j + 1, c: t.c * (-2.0 * p) });
        }
        terms = next;
    }
    for _ in 0..kz {
        let mut next = Vec::with_capacity(terms.len() * 2);
        for t in &terms {
            if t.mz > 0 {
                next.push(BoysTerm { mx: t.mx, my: t.my, mz: t.mz - 1, j: t.j, c: t.c * t.mz as f64 });
            }
            next.push(BoysTerm { mx: t.mx, my: t.my, mz: t.mz + 1, j: t.j + 1, c: t.c * (-2.0 * p) });
        }
        terms = next;
    }
    terms
}

/// McMurchie–Davidson E-coefficients for one dimension.
///
/// Expands `(u+X_PA)^i (u+X_PB)^j e^{−p u²}` in the Hermite-Gaussian basis
/// `D_t = ∂^t_u [e^{−p u²}]` via the multiplication rule
/// `u·D_t = −D_{t+1}/(2p) − t·D_{t−1}`, which yields the shift recursion.
/// Returns E[0..=i+j] (impossible t are exactly 0).
fn md_e_coeffs(i: usize, j: usize, p: f64, x_pa: f64, x_pb: f64) -> Vec<f64> {
    let mut e: Vec<f64> = vec![1.0]; // (i=0, j=0): Λ = e^{−pu²} = D_0
    for _ in 0..i {
        e = md_shift(e, x_pa, p);
    }
    for _ in 0..j {
        e = md_shift(e, x_pb, p);
    }
    e
}

/// One MD shift: multiply Λ = Σ E[t]·D_t by (u + X).
///
/// From `(u+X)·D_s = X·D_s − D_{s+1}/(2p) − s·D_{s−1}`:
/// the contribution of E[s] lands on out[s] (X·E[s]), out[s+1] (−E[s]/(2p))
/// and out[s−1] (−s·E[s]).
fn md_shift(e: Vec<f64>, x: f64, p: f64) -> Vec<f64> {
    let n = e.len();
    let mut out = vec![0.0f64; n + 1];
    for (s, val) in e.iter().enumerate() {
        if *val == 0.0 {
            continue;
        }
        out[s] += x * val;
        out[s + 1] -= val / (2.0 * p);
        if s > 0 {
            out[s - 1] -= s as f64 * val;
        }
    }
    out
}

/// Primitive Cartesian nuclear attraction (McMurchie–Davidson, v3.3.0):
/// `−Z_C ∫ (poly) e^{−α r_A² − β r_B²} / |r − C| d³r`.
///
/// V = −Z·K_AB·(2π/p)·Σ_{tuv} E^t_{i_x j_x} E^u E^v · R_{tuv},
/// R_{tuv} = (−1)^{t+u+v} ∂^t_x ∂^u_y ∂^v_z [F₀(p|P−C|²)].
///
/// v3.3.0 FIX: прежняя версия раскладывала полином в сырые степени (x−P)^k
/// и применяла (1/(2p))^k·∂^k — тождество (x−P)e = (1/(2p))∂e верно только
/// в первом порядке; со второго порядка product rule даёт член −2p·e, который
/// наивная формула теряет. Симптом: V(p,p) выходил ПОЛОЖИТЕЛЬНЫМ (+4.43 Ha
/// для O 2p в H2O, истина −10.14 Ha), все молекулы с занятыми p-оболочками
/// были катастрофически неверны (H2O: −44.09 против −74.96 Ha), а LiH-тест
/// v0.73.0 зафиксировал баг (−7.810054 вместо −7.862246). Проверено против
/// PySCF 2.14: H2O/NH3/CH4/HF/CO сходятся до ~1e-9 Ha.
pub fn nuclear_cartesian(
    ra: &Point,
    rb: &Point,
    rc: &Point,
    alpha: f64,
    beta: f64,
    zc: f64,
    a: [usize; 3],
    b: [usize; 3],
) -> f64 {
    let p = alpha + beta;
    let rab2 = ra.dist2(&rb);
    let kab = libm::exp(-alpha * beta * rab2 / p);
    let pc = product_center(ra, rb, alpha, beta);
    let x = pc[0] - rc.0[0];
    let y = pc[1] - rc.0[1];
    let z = pc[2] - rc.0[2];
    let t_arg = p * (x * x + y * y + z * z);

    // E-коэффициенты по каждой оси (Hermite-разложение полинома пары).
    let ex = md_e_coeffs(a[0], b[0], p, pc[0] - ra.0[0], pc[0] - rb.0[0]);
    let ey = md_e_coeffs(a[1], b[1], p, pc[1] - ra.0[1], pc[1] - rb.0[1]);
    let ez = md_e_coeffs(a[2], b[2], p, pc[2] - ra.0[2], pc[2] - rb.0[2]);

    // Σ_{tuv} E^t E^u E^v · (−1)^{t+u+v} · ∂^{t+u+v}[F₀]
    let mut total = 0.0f64;
    for (t, e_t) in ex.iter().enumerate() {
        if *e_t == 0.0 {
            continue;
        }
        for (u, e_u) in ey.iter().enumerate() {
            if *e_u == 0.0 {
                continue;
            }
            for (v, e_v) in ez.iter().enumerate() {
                if *e_v == 0.0 {
                    continue;
                }
                let k_all = t + u + v;
                let sign = if k_all % 2 == 0 { 1.0 } else { -1.0 };
                let mut val = 0.0f64;
                for term in &boys_derivative(t, u, v, p) {
                    val += term.c
                        * ipow(x, term.mx)
                        * ipow(y, term.my)
                        * ipow(z, term.mz)
                        * boys_f(term.j, t_arg);
                }
                total += e_t * e_u * e_v * sign * val;
            }
        }
    }

    -zc * kab * (2.0 * PI / p) * total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s_seed_consistency() {
        let ra = Point([0.0, 0.0, 0.0]);
        let rb = Point([0.5, 0.0, 0.0]);
        let alpha = 1.3;
        let beta = 0.9;
        let expected = crate::overlap::overlap_ss(alpha, beta, ra.dist2(&rb));
        let got = overlap_cartesian(&ra, &rb, alpha, beta, [0, 0, 0], [0, 0, 0]);
        assert!((got - expected).abs() < 1e-14, "s-seed overlap: {got} vs {expected}");
    }

    #[test]
    fn test_t_seed_consistency() {
        let ra = Point([0.0, 0.0, 0.0]);
        let rb = Point([0.5, 0.1, 0.0]);
        let alpha = 1.3;
        let beta = 0.9;
        let expected = crate::kinetic::kinetic_ss(alpha, beta, ra.dist2(&rb));
        let got = kinetic_cartesian(&ra, &rb, alpha, beta, [0, 0, 0], [0, 0, 0]);
        assert!((got - expected).abs() < 1e-13, "s-seed kinetic: {got} vs {expected}");
    }

    #[test]
    fn test_v_seed_consistency() {
        let ra = Point([0.0, 0.0, 0.0]);
        let rb = Point([0.5, 0.1, 0.2]);
        let rc = Point([1.0, 0.3, -0.4]);
        let alpha = 1.3;
        let beta = 0.9;
        let p = alpha + beta;
        let pc = product_center(&ra, &rb, alpha, beta);
        let dx = pc[0] - rc.0[0];
        let dy = pc[1] - rc.0[1];
        let dz = pc[2] - rc.0[2];
        let expected = crate::nuclear::nuclear_ss(
            alpha, beta, ra.dist2(&rb), dx * dx + dy * dy + dz * dz, 2.0,
        );
        let got = nuclear_cartesian(&ra, &rb, &rc, alpha, beta, 2.0, [0, 0, 0], [0, 0, 0]);
        assert!((got - expected).abs() < 1e-14, "s-seed nuclear: {got} vs {expected}");
    }

    #[test]
    fn test_p_self_overlap_symmetry() {
        // Normalized p-shell primitives on the same center:
        // <px|px> = 1-ish scale, <px|py> = 0 exactly.
        let r = Point([0.0, 0.0, 0.0]);
        let s_px_px = overlap_cartesian(&r, &r, 1.0, 1.0, [1, 0, 0], [1, 0, 0]);
        let s_px_py = overlap_cartesian(&r, &r, 1.0, 1.0, [1, 0, 0], [0, 1, 0]);
        // <px|px> = M(2)M(0)M(0) = sqrt(pi)/2 (for p=2)
        assert!(s_px_px > 0.0);
        assert!(s_px_py.abs() < 1e-15, "px/py must be orthogonal: {s_px_py}");
    }

    #[test]
    fn test_nuclear_p_perpendicular_vanishes() {
        // p_y on A, s on B along x-axis, nucleus C on the x-axis:
        // odd in y -> integral = 0.
        let ra = Point([0.0, 0.0, 0.0]);
        let rb = Point([2.0, 0.0, 0.0]);
        let rc = Point([1.0, 0.0, 0.0]);
        let v = nuclear_cartesian(&ra, &rb, &rc, 1.0, 1.0, 1.0, [0, 1, 0], [0, 0, 0]);
        assert!(v.abs() < 1e-14, "perpendicular p must vanish: {v}");
    }
}
