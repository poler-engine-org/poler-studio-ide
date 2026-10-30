//! poler_core.rs — Главный оркестратор когнитивного цикла ℘–O–L–ε–R[n]–Ψ
//! Интегрирует SCTP ConstraintLayer, EnergyEngine и логический проектор Pi_Lambda.

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolerCoreConfig {
    pub dim: usize,
    pub eta0: f64,
    pub gamma: f64,
    pub rho: f64,
    pub stationarity_threshold: f64,
}

impl Default for PolerCoreConfig {
    fn default() -> Self {
        Self {
            dim: 128,
            eta0: 0.05,
            gamma: 0.5,
            rho: 0.9,
            stationarity_threshold: 1e-7,
        }
    }
}

pub struct PolerCore {
    pub config: PolerCoreConfig,
    pub state_p: Vec<f64>,
    pub memory_history: Vec<Vec<f64>>,
    pub step: usize,
}

impl PolerCore {
    pub fn new(config: PolerCoreConfig) -> Self {
        let dim = config.dim;
        Self {
            config,
            state_p: vec![0.0; dim],
            memory_history: Vec::new(),
            step: 0,
        }
    }

    /// Единый шаг эволюции: ℘ -> O -> L -> ε -> R[n] -> Ψ
    pub fn evolve_step(&mut self, observation: &[f64], forbidden: bool) -> (Vec<f64>, f64, bool) {
        if forbidden {
            return (self.state_p.clone(), 0.0, false);
        }

        let dim = self.config.dim;
        // 1. ℘ (Perception): tanh(o_t)
        let mut obs_tanh = vec![0.0; dim];
        for i in 0..dim {
            obs_tanh[i] = observation.get(i).copied().unwrap_or(0.0).tanh();
        }

        // 2. F (Free Energy gradient) & ε (Significance)
        let mut grad_f = vec![0.0; dim];
        let mut f_energy = 0.0;
        for i in 0..dim {
            let diff = self.state_p[i] - obs_tanh[i];
            grad_f[i] = 2.0 * diff;
            f_energy += diff * diff;
        }

        // 3. R[n] (IIR Memory Resonance Echo)
        let mut grad_eps = vec![0.0; dim];
        let history_len = self.memory_history.len();
        for (k, past) in self.memory_history.iter().rev().take(8).enumerate() {
            let weight = self.config.rho.powi((k + 1) as i32);
            for i in 0..dim {
                grad_eps[i] += weight * (self.state_p[i] - past[i]);
            }
        }

        // 4. Ψ-Flow Update: p_{t+1} = p_t + eta * (-grad_F + gamma * grad_eps)
        let mut p_next = vec![0.0; dim];
        for i in 0..dim {
            let dp = -grad_f[i] + self.config.gamma * grad_eps[i];
            p_next[i] = (self.state_p[i] + self.config.eta0 * dp).clamp(-1.0, 1.0);
        }

        self.state_p = p_next.clone();
        self.memory_history.push(obs_tanh);
        self.step += 1;

        let stabilized = f_energy < self.config.stationarity_threshold;
        (p_next, f_energy, stabilized)
    }
}
