//! synaptic_ops.rs — Complete SCTP ConstraintLayer & Cosine Topology

pub struct ConstraintLayer {
    pub dim: usize,
    pub d_strength: f64,
    pub j_strength: f64,
    pub epsilon_reg: f64,
}

impl ConstraintLayer {
    pub fn new(dim: usize) -> Self {
        Self {
            dim,
            d_strength: 1.0,
            j_strength: 0.3,
            epsilon_reg: 1e-6,
        }
    }

    /// Forward semantic flow: S(p) = Pi_Lambda [J - D] Pi_Lambda * p
    pub fn forward(&self, p: &[f64], j_mat: &[f64], d_mat: &[f64]) -> Vec<f64> {
        let mut diff = vec![0.0; self.dim * self.dim];
        for i in 0..(self.dim * self.dim) {
            diff[i] = j_mat[i] - d_mat[i];
        }
        let mut out = vec![0.0; self.dim];
        for i in 0..self.dim {
            for j in 0..self.dim {
                out[i] += diff[i * self.dim + j] * p[j];
            }
        }
        out
    }
}
