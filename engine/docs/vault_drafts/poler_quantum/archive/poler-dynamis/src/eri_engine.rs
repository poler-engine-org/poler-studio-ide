//! eri_engine.rs — Двухэлектронные интегралы отталкивания ERI и сборка Fock матрицы
//! Реализует N^4 цикл и полный scatter_fock для кулоновского (J) и обменного (K) взаимодействия.

pub struct EriEngine {
    pub nao: usize,
}

impl EriEngine {
    pub fn new(nao: usize) -> Self {
        Self { nao }
    }

    /// Сборка матрицы Фока: F = H_core + J - 0.5 * K
    pub fn compute_fock(&self, h_core: &[f64], dm: &[f64], eri_tensor: &[f64]) -> Vec<f64> {
        let n = self.nao;
        let mut fock = h_core.to_vec();
        let mut j_mat = vec![0.0; n * n];
        let mut k_mat = vec![0.0; n * n];

        for mu in 0..n {
            for nu in 0..n {
                let idx_munu = mu * n + nu;
                for lambda in 0..n {
                    for sigma in 0..n {
                        let idx_ls = lambda * n + sigma;
                        let eri_val = eri_tensor[mu * n * n * n + nu * n * n + lambda * n + sigma];
                        
                        // Coulomb contribution (J)
                        j_mat[idx_munu] += dm[idx_ls] * eri_val;
                        
                        // Exchange contribution (K)
                        let idx_musigma = mu * n + sigma;
                        let idx_lnu = lambda * n + nu;
                        k_mat[idx_musigma] += dm[idx_lnu] * eri_val;
                    }
                }
            }
        }

        for i in 0..(n * n) {
            fock[i] += j_mat[i] - 0.5 * k_mat[i];
        }

        fock
    }
}
