//! fep_loss.rs — Расчет функционала свободной энергии F и градиента nabla F
//! Реализует принцип активного вывода Карла Фристона (Free Energy Principle).
//! F = ||g(p) - Omega(o)||_G^2 + lambda * R_L(p)

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FEPLossConfig {
    pub lambda_reg: f64,
    pub curvature_weight: f64,
}

impl Default for FEPLossConfig {
    fn default() -> Self {
        Self {
            lambda_reg: 1e-4,
            curvature_weight: 0.01,
        }
    }
}

pub struct FEPLoss {
    pub config: FEPLossConfig,
    pub dim: usize,
}

impl FEPLoss {
    pub fn new(dim: usize, config: FEPLossConfig) -> Self {
        Self { config, dim }
    }

    /// Вычисление свободной энергии F и её градиента 
abla F
    /// F(p, o) = \sum_i (p_i - \Omega(o_i))^2 + \lambda ||p||^2
    pub fn compute_loss_and_grad(&self, p: &[f64], obs_tanh: &[f64]) -> (f64, Vec<f64>) {
        let mut loss = 0.0;
        let mut grad = vec![0.0; self.dim];

        for i in 0..self.dim {
            let pred_err = p[i] - obs_tanh[i];
            let reg_term = self.config.lambda_reg * p[i];
            
            loss += pred_err * pred_err + 0.5 * self.config.lambda_reg * p[i] * p[i];
            grad[i] = 2.0 * pred_err + reg_term;
        }

        (loss, grad)
    }
}
