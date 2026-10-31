#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Pure Continuous Dynamics Ablation Test for POLER v0.3.5
//!
//! ABLATION RULES:
//! 1. hierarchical_bb is DISABLED (returns None)
//! 2. brute_binary_search is DISABLED (returns None)
//! 3. extract_verified is STRICT THRESHOLD ONLY (p_i = [x_i > 0.5]) — zero combo search, zero delta radius.
//! 4. Test both WITH ROTOR (gamma > 0) and WITHOUT ROTOR (gamma = 0).

use nalgebra::DMatrix;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct PolynomialConstraint {
    pub var_indices: Vec<usize>,
    pub eval: fn(&[f64]) -> f64,
    pub jacobian: fn(&[f64], &mut [f64]),
}

pub struct FactorizationTopology {
    pub n_bits_p: usize,
    pub n_bits_q: usize,
    pub n_bits_n: usize,
    pub total_vars: usize,
    pub constraints: Vec<PolynomialConstraint>,
    pub fixed_var_indices: Vec<usize>,
    pub skew_j: DMatrix<f64>,
}

fn and_eval(v: &[f64]) -> f64 { v[0] * v[1] - v[2] }
fn and_jac(v: &[f64], g: &mut [f64]) { g[0] = v[1]; g[1] = v[0]; g[2] = -1.0; }

fn xor_eval(v: &[f64]) -> f64 { v[0] + v[1] - 2.0 * v[0] * v[1] - v[2] }
fn xor_jac(v: &[f64], g: &mut [f64]) { g[0] = 1.0 - 2.0 * v[1]; g[1] = 1.0 - 2.0 * v[0]; g[2] = -1.0; }

fn fa_sum_eval(v: &[f64]) -> f64 {
    let (a, b, cin, s) = (v[0], v[1], v[2], v[3]);
    a + b + cin - 2.0*a*b - 2.0*a*cin - 2.0*b*cin + 4.0*a*b*cin - s
}
fn fa_sum_jac(v: &[f64], g: &mut [f64]) {
    let (a, b, cin) = (v[0], v[1], v[2]);
    g[0] = 1.0 - 2.0*b - 2.0*cin + 4.0*b*cin;
    g[1] = 1.0 - 2.0*a - 2.0*cin + 4.0*a*cin;
    g[2] = 1.0 - 2.0*a - 2.0*b + 4.0*a*b;
    g[3] = -1.0;
}

fn fa_carry_eval(v: &[f64]) -> f64 {
    let (a, b, cin, cout) = (v[0], v[1], v[2], v[3]);
    a*b + a*cin + b*cin - 2.0*a*b*cin - cout
}
fn fa_carry_jac(v: &[f64], g: &mut [f64]) {
    let (a, b, cin) = (v[0], v[1], v[2]);
    g[0] = b + cin - 2.0*b*cin;
    g[1] = a + cin - 2.0*a*cin;
    g[2] = a + b - 2.0*a*b;
    g[3] = -1.0;
}

impl FactorizationTopology {
    pub fn build(nbp: usize, nbq: usize) -> Self {
        let nbn = nbp + nbq;
        let npq = nbp + nbq;
        let mut total = npq;
        let mut cons = Vec::new();
        let mut fixed = Vec::new();

        let mut pp = vec![vec![0usize; nbq]; nbp];
        for i in 0..nbp {
            for j in 0..nbq {
                let ppi = total;
                total += 1;
                pp[i][j] = ppi;
                cons.push(PolynomialConstraint {
                    var_indices: vec![i, nbp + j, ppi],
                    eval: and_eval,
                    jacobian: and_jac,
                });
            }
        }

        let mut carries: Vec<Vec<usize>> = Vec::new();
        for col in 0..nbn {
            let mut col_vars: Vec<usize> = Vec::new();
            for i in 0..nbp {
                if col >= i && (col - i) < nbq {
                    col_vars.push(pp[i][col - i]);
                }
            }
            let prev_c = if col > 0 && col - 1 < carries.len() {
                carries[col - 1].clone()
            } else {
                Vec::new()
            };
            let mut all_in = col_vars;
            all_in.extend(prev_c);

            let mut next_c = Vec::new();
            let mut cur_in = all_in;
            while cur_in.len() > 1 {
                if cur_in.len() == 2 {
                    let s_var = total; total += 1;
                    let c_var = total; total += 1;
                    cons.push(PolynomialConstraint {
                        var_indices: vec![cur_in[0], cur_in[1], s_var],
                        eval: xor_eval, jacobian: xor_jac,
                    });
                    cons.push(PolynomialConstraint {
                        var_indices: vec![cur_in[0], cur_in[1], c_var],
                        eval: and_eval, jacobian: and_jac,
                    });
                    next_c.push(c_var);
                    cur_in = vec![s_var];
                } else {
                    let a = cur_in.remove(0);
                    let b = cur_in.remove(0);
                    let cin = cur_in.remove(0);
                    let s_var = total; total += 1;
                    let c_var = total; total += 1;
                    cons.push(PolynomialConstraint {
                        var_indices: vec![a, b, cin, s_var],
                        eval: fa_sum_eval, jacobian: fa_sum_jac,
                    });
                    cons.push(PolynomialConstraint {
                        var_indices: vec![a, b, cin, c_var],
                        eval: fa_carry_eval, jacobian: fa_carry_jac,
                    });
                    next_c.push(c_var);
                    cur_in.insert(0, s_var);
                }
            }
            if !cur_in.is_empty() {
                fixed.push(cur_in[0]);
            }
            carries.push(next_c);
        }

        let mut adj = DMatrix::zeros(total, total);
        for c in &cons {
            let n_vars = c.var_indices.len();
            if n_vars < 2 { continue; }
            let out_idx = c.var_indices[n_vars - 1];
            for &in_idx in &c.var_indices[..n_vars-1] {
                adj[(in_idx, out_idx)] += 1.0;
            }
        }
        let skew_j = &adj - &adj.transpose();

        FactorizationTopology {
            n_bits_p: nbp, n_bits_q: nbq, n_bits_n: nbn,
            total_vars: total, constraints: cons,
            fixed_var_indices: fixed, skew_j,
        }
    }

    pub fn forward_pass(&self, pq: &[f64], nbits: &[f64]) -> (Vec<f64>, f64) {
        let npq = self.n_bits_p + self.n_bits_q;
        let mut full = vec![0.0f64; self.total_vars];
        full[..npq].copy_from_slice(pq);

        let mut energy = 0.0;
        for c in &self.constraints {
            let n_vars = c.var_indices.len();
            let mut v = Vec::with_capacity(n_vars);
            for &idx in &c.var_indices {
                v.push(full[idx]);
            }
            let r = (c.eval)(&v);
            let out_idx = c.var_indices[n_vars - 1];
            full[out_idx] = full[out_idx] - r;
            energy += r * r;
        }

        for (col, &fixed_idx) in self.fixed_var_indices.iter().enumerate() {
            if col < nbits.len() {
                let diff = full[fixed_idx] - nbits[col];
                energy += 10.0 * diff * diff;
            }
        }

        (full, energy)
    }

    pub fn compute_grad(&self, pq: &[f64], nbits: &[f64]) -> (Vec<f64>, f64) {
        let npq = self.n_bits_p + self.n_bits_q;
        let (_full, base_e) = self.forward_pass(pq, nbits);
        let mut g = vec![0.0f64; npq];
        let eps = 1e-6;
        let inv_eps = 1.0 / eps;

        let mut pq_pert = pq.to_vec();
        for i in 0..npq {
            pq_pert[i] += eps;
            let (_, e_plus) = self.forward_pass(&pq_pert, nbits);
            g[i] = (e_plus - base_e) * inv_eps;
            pq_pert[i] = pq[i];
        }

        (g, base_e)
    }
}

pub fn uint_to_bits(n: u64, n_bits: usize) -> Vec<f64> {
    let mut bits = Vec::with_capacity(n_bits);
    for b in 0..n_bits {
        bits.push(if (n >> b) & 1 == 1 { 1.0 } else { 0.0 });
    }
    bits
}

pub fn run_pure_solver(
    n: u64,
    nbp: usize,
    nbq: usize,
    use_rotor: bool,
    max_iter: usize,
    restarts: usize,
) -> (bool, u64, u64, usize, f64, u128) {
    let topo = FactorizationTopology::build(nbp, nbq);
    let nbits = uint_to_bits(n, nbp + nbq);
    let npq = nbp + nbq;

    let t0 = Instant::now();
    let mut best_e_overall = f64::INFINITY;
    let mut total_iters = 0;

    for _r in 0..restarts {
        // Initial point: middle [0.5, ...] with slight random offset
        let mut pq: Vec<f64> = (0..npq).map(|i| {
            if i == 0 || i == nbp { 1.0 } else { 0.5 + 0.1 * ((i as f64 * 13.0).sin()) }
        }).collect();

        let mut m = vec![0.0f64; npq];
        let mut vs = vec![0.0f64; npq];
        let mut pq_history: Vec<Vec<f64>> = Vec::new();

        let lr = 0.03;
        let gamma_rotor = if use_rotor { 0.05 } else { 0.0 };

        for it in 0..max_iter {
            total_iters += 1;
            let (g, e) = topo.compute_grad(&pq, &nbits);
            if e < best_e_overall { best_e_overall = e; }

            // PURE STRICT EXTRACTION: No trial division, no ±3, no combination search
            let mut p_test = 0u64;
            for b in 0..nbp { if pq[b] > 0.5 { p_test |= 1 << b; } }
            let mut q_test = 0u64;
            for b in 0..nbq { if pq[nbp + b] > 0.5 { q_test |= 1 << b; } }

            if p_test > 1 && q_test > 1 && p_test * q_test == n {
                return (true, p_test, q_test, total_iters, e, t0.elapsed().as_millis());
            }

            // Rotor force: gamma * J * p
            let mut resonance_force = vec![0.0f64; npq];
            if use_rotor && pq_history.len() >= 2 {
                let start = if pq_history.len() > 10 { pq_history.len() - 10 } else { 0 };
                let mut a = vec![vec![0.0f64; npq]; npq];
                let mut count = 0.0f64;
                for k in start..pq_history.len() - 1 {
                    let pk = &pq_history[k];
                    let pk1 = &pq_history[k + 1];
                    for i in 0..npq {
                        for j in 0..npq {
                            a[i][j] += pk[i] * (pk1[j] - pk[j]);
                        }
                    }
                    count += 1.0;
                }
                if count > 0.0 {
                    let inv = 1.0 / count;
                    for i in 0..npq {
                        let mut f = 0.0;
                        for j in 0..npq {
                            let ji = gamma_rotor * (a[i][j] - a[j][i]) * inv;
                            f += ji * pq[j];
                        }
                        resonance_force[i] = f;
                    }
                }
            }

            // Update Adam
            let t = (it + 1) as f64;
            for i in 0..npq {
                if i == 0 || i == nbp { continue; } // odd anchor
                let gi = g[i] + resonance_force[i];
                m[i] = 0.9 * m[i] + 0.1 * gi;
                vs[i] = 0.999 * vs[i] + 0.001 * gi * gi;
                let mh = m[i] / (1.0 - 0.9_f64.powf(t));
                let vh = vs[i] / (1.0 - 0.999_f64.powf(t));
                pq[i] -= lr * mh / (vh.sqrt() + 1e-8);
                pq[i] = pq[i].clamp(0.0, 1.0);
            }

            pq_history.push(pq.clone());
            if pq_history.len() > 12 { pq_history.remove(0); }
        }
    }

    (false, 0, 0, total_iters, best_e_overall, t0.elapsed().as_millis())
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════════════╗");
    println!("║       POLER PURE CONTINUOUS DYNAMICS ABLATION TEST (ZERO FALLBACKS)      ║");
    println!("║  [x] brute_binary_search: DISABLED (None)                                ║");
    println!("║  [x] hierarchical_bb:     DISABLED (None)                                ║");
    println!("║  [x] radius +/- 3 search: DISABLED (Strict threshold [x > 0.5] only)     ║");
    println!("╚══════════════════════════════════════════════════════════════════════════╝\n");

    let targets = vec![
        (35u64, 3, 3, "6-bit: 35 = 5 * 7"),
        (143u64, 4, 4, "8-bit: 143 = 11 * 13"),
        (3233u64, 6, 6, "12-bit: 3233 = 53 * 61"),
        (6557u64, 7, 7, "13-bit: 6557 = 79 * 83"),
    ];

    for (n, nbp, nbq, desc) in targets {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🎯 Target: {} ({})", n, desc);

        // 1. WITHOUT ROTOR (gamma = 0)
        let (found_no, p_no, q_no, it_no, e_no, ms_no) = run_pure_solver(n, nbp, nbq, false, 8000, 5);
        if found_no {
            println!("  [Mode: NO Rotor  (γ=0)] ✅ Solved: {} = {} × {} ({} iters, E={:.4}, {}ms)", n, p_no, q_no, it_no, e_no, ms_no);
        } else {
            println!("  [Mode: NO Rotor  (γ=0)] ❌ STUCK: E={:.4} ({} iters, {}ms)", e_no, it_no, ms_no);
        }

        // 2. WITH ROTOR (gamma > 0)
        let (found_r, p_r, q_r, it_r, e_r, ms_r) = run_pure_solver(n, nbp, nbq, true, 8000, 5);
        if found_r {
            println!("  [Mode: WITH Rotor(γ>0)] 🚀 SOLVED: {} = {} × {} ({} iters, E={:.4}, {}ms)", n, p_r, q_r, it_r, e_r, ms_r);
        } else {
            println!("  [Mode: WITH Rotor(γ>0)] ❌ STUCK: E={:.4} ({} iters, {}ms)", e_r, it_r, ms_r);
        }
    }
}
