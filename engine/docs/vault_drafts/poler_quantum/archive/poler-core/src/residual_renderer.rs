//! residual_renderer.rs — Траекторный декодер стационарного вектора p* в текст
//! Восстанавливает токены и семантические символы из фазового латентного состояния.

pub struct ResidualRenderer {
    pub vocab: Vec<String>,
    pub embeddings: Vec<Vec<f64>>,
    pub dim: usize,
}

impl ResidualRenderer {
    pub fn new(vocab: Vec<String>, embeddings: Vec<Vec<f64>>, dim: usize) -> Self {
        Self { vocab, embeddings, dim }
    }

    /// Декодирование вектора p в наиболее вероятный строковый токен через косинусное сходство
    pub fn decode_token(&self, p: &[f64]) -> Option<String> {
        if self.vocab.is_empty() || self.embeddings.is_empty() {
            return None;
        }

        let p_norm = self.vector_norm(p);
        if p_norm < 1e-9 {
            return None;
        }

        let mut best_sim = -1.0;
        let mut best_idx = 0;

        for (idx, emb) in self.embeddings.iter().enumerate() {
            let emb_norm = self.vector_norm(emb);
            if emb_norm < 1e-9 {
                continue;
            }
            let dot: f64 = p.iter().zip(emb.iter()).map(|(a, b)| a * b).sum();
            let sim = dot / (p_norm * emb_norm);
            if sim > best_sim {
                best_sim = sim;
                best_idx = idx;
            }
        }

        self.vocab.get(best_idx).cloned()
    }

    fn vector_norm(&self, v: &[f64]) -> f64 {
        v.iter().map(|x| x * x).sum::<f64>().sqrt()
    }
}
