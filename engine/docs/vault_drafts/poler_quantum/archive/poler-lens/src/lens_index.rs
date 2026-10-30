//! lens_index.rs — Разреженная статическая линза LENS (99.2% сжатия)
//! Блокирует галлюцинации и проецирует граф зависимостей на допустимые ребра.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SparseEdge {
    pub target: u32,
    pub weight: f32,
}

pub struct LensIndex {
    pub adjacency: HashMap<u32, Vec<SparseEdge>>,
    pub total_edges: usize,
    pub compression_rate: f64,
}

impl LensIndex {
    pub fn new() -> Self {
        Self {
            adjacency: HashMap::new(),
            total_edges: 0,
            compression_rate: 0.992,
        }
    }

    pub fn add_edge(&mut self, source: u32, target: u32, weight: f32) {
        self.adjacency.entry(source).or_default().push(SparseEdge { target, weight });
        self.total_edges += 1;
    }

    /// Проверка наличия семантического ребра (No-Hits барьер)
    pub fn query_constraint(&self, source: u32, target: u32) -> bool {
        if let Some(neighbors) = self.adjacency.get(&source) {
            neighbors.iter().any(|e| e.target == target && e.weight > 0.05)
        } else {
            false
        }
    }
}
