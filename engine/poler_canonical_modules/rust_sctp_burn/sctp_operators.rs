//! SCTP OPERATORS: D (Dissipation), J (Resonance), Pi_Lambda (Projector)

#[derive(Debug, Clone)]
pub struct SctpProcessor {
    pub dim: usize,
    pub eta: f64,
    pub gamma: f64,
    pub epsilon_reg: f64,
}

impl SctpProcessor {
    pub fn new(dim: usize) -> Self {
        Self {
            dim,
            eta: 0.05,
            gamma: 0.5,
            epsilon_reg: 1e-6,
        }
    }

    /// McWeeny Density Matrix Purification: P_new = 3P^2 - 2P^3
    pub fn mcweeny_purify(p: &[f64], n: usize) -> Vec<f64> {
        // Multiplies n x n matrices: 3*P^2 - 2*P^3
        let p2 = Self::mat_mul(p, p, n);
        let p3 = Self::mat_mul(&p2, p, n);
        let mut out = vec![0.0; n * n];
        for i in 0..(n * n) {
            out[i] = 3.0 * p2[i] - 2.0 * p3[i];
        }
        out
    }

    /// Semantic Flow Operator: S(p) = Pi_Lambda [J - D] Pi_Lambda * p
    pub fn semantic_flow(&self, p: &[f64], j_mat: &[f64], d_mat: &[f64]) -> Vec<f64> {
        let mut diff = vec![0.0; self.dim * self.dim];
        for i in 0..(self.dim * self.dim) {
            diff[i] = j_mat[i] - d_mat[i];
        }
        Self::mat_vec_mul(&diff, p, self.dim)
    }

    fn mat_mul(a: &[f64], b: &[f64], n: usize) -> Vec<f64> {
        let mut out = vec![0.0; n * n];
        for i in 0..n {
            for k in 0..n {
                let aik = a[i * n + k];
                for j in 0..n {
                    out[i * n + j] += aik * b[k * n + j];
                }
            }
        }
        out
    }

    fn mat_vec_mul(mat: &[f64], vec: &[f64], n: usize) -> Vec<f64> {
        let mut out = vec![0.0; n];
        for i in 0..n {
            for j in 0..n {
                out[i] += mat[i * n + j] * vec[j];
            }
        }
        out
    }
}
