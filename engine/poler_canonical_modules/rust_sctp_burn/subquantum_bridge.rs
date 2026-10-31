//! SUBQUANTUM BRIDGE: Ядро генерації сенсів та ліквідація токенізації
//! Реалізує 8-шаговий цикл ℘–O–L–ε–R[n]–Ψ та дешифрування траєкторії.

use serde::{Serialize, Deserialize};
use std::collections::VecDeque;

/// Одиниця субквантового мосту - Квант мови
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubquantumQubit {
    pub phase: f64,        // Фаза Ψ
    pub amplitude: f64,    // Маса m(t)
    pub significance: f64, // Енергія ε
}

/// Атом сенсу: Архетип + Семантичний вектор
#[derive(Debug, Clone)]
pub struct SubquantumWord {
    pub archetype: String,      // Назва архетипу (Root, Flow, Prism...)
    pub embedding: Vec<f64>,    // Точка у латентному просторі p
    pub signature: f64,         // Resonance Signature R[n]
}

/// Результат досягнення стаціонарності (Wheeler-DeWitt Limit)
#[derive(Debug, Serialize, Deserialize)]
pub struct StationarityResult {
    pub h_psi: f64,         // Значення H^Ψ (ціль: 0)
    pub conductivity: f64,  // Провідність сенсу
    pub stabilized: bool,   // Режим когнітивного спокою
}

/// Резонансний словник (SubquantumDictionary)
pub struct SubquantumDictionary {
    pub anchors: Vec<SubquantumWord>,
    pub dim: usize,
    pub conductivity_threshold: f64,
}

impl SubquantumDictionary {
    pub fn new(dim: usize) -> Self {
        Self {
            anchors: Vec::new(),
            dim,
            conductivity_threshold: 0.95,
        }
    }

    /// Резонансне комбінування (⊗ε)
    pub fn combine(&self, a: &SubquantumWord, b: &SubquantumWord, epsilon: f64) -> SubquantumWord {
        let mut combined_emb = Vec::with_capacity(a.embedding.len());
        for (va, vb) in a.embedding.iter().zip(b.embedding.iter()) {
            combined_emb.push((va + vb) * epsilon);
        }
        SubquantumWord {
            archetype: format!("{}⊗{}", a.archetype, b.archetype),
            embedding: combined_emb,
            signature: (a.signature + b.signature) / 2.0,
        }
    }
}

/// Три Судді (Math, Logic, Ethics)
pub struct ThreeJudges {
    pub math_norm_limit: f64,
    pub free_energy_limit: f64,
    pub ethics_filter: f64,
}

impl Default for ThreeJudges {
    fn default() -> Self {
        Self {
            math_norm_limit: 1.05,
            free_energy_limit: 1e-7,
            ethics_filter: 0.0,
        }
    }
}

impl ThreeJudges {
    pub fn judge_trajectory(&self, p_t: &[f64], f_energy: f64) -> bool {
        let norm: f64 = p_t.iter().map(|x| x * x).sum::<f64>().sqrt();
        let math_pass = norm <= self.math_norm_limit;
        let logic_pass = f_energy < self.free_energy_limit;
        math_pass && logic_pass
    }
}
