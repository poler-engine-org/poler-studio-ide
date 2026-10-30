//! Vector Layer — плотная векторная полоса поиска (v2.0, Приоритет 4).
//!
//! Кирпич 1 из 3 — **субстрат масштаба**: то, что определяет «1 ГБ
//! данных с задержкой микросекунды», а не сменную модель.
//!
//! ```text
//! Embedder (кирпич 2: нативный .pqw-энкодер через pqc — Part E)
//!     │  Vec<f32> (d)
//!     ▼
//! rabitq::Encoder ── вращение Адамара + 1-битные коды
//!     │  mu/delta/gamma (12 Б) + d_pad/8 Б кода
//!     ▼
//! store::QuantizedStore ── append-only, mmap zero-copy
//!     │  слоты → DocId
//!     ▼
//! hnsw::HnswIndex ── popcount-обход (sym) → ADC-переранжирование
//!     │
//!     ▼
//! top-k (косинус-оценка, DocId) — инструмент отдал, ИИ понимает
//! ```
//!
//! Бюджет памяти на 1B векторов (research §5.6): коды 64-d — 8 ГБ +
//! верхний граф ~5 ГБ + LRU-кэш переранжирования ~0.3 ГБ ≈ 13 ГБ.
//! Оригиналы fp32 в индексе не живут вообще.
//!
//! Принцип «инструмент, не ИИ»: слой индексирует числовую близость и
//! отдаёт кандидатов. Он не выбирает «правильный» результат, не
//! генерирует текст и не интерпретирует смысл векторов — это дело
//! эмбеддера (инференс готовой модели) и ИИ-потребителя.
//!
//! Кирпич 2 из 3 — нативный инференс (Part E): PqwEmbedder открывает
//! BGE-M3-класс из контейнера .pqw (mmap + Sha256, weight-only
//! int8/int4, SIMD-кернелы pqc::tensor). fastembed/ort ОТМЕНЕНЫ —
//! суверенный стек без ONNX Runtime и C++ FFI.

pub mod hnsw;
pub mod pqw_bridge;
pub mod rabitq;
pub mod store;

pub use hnsw::{HnswConfig, HnswIndex};
pub use pqw_bridge::PqwEmbedder;
pub use rabitq::{adc_cos, adc_ip, sym_ip, Encoder, QueryPrep, VecScalars};
pub use store::{CodeSource, QuantizedStore, QuantizedStoreView};

// ---------------------------------------------------------------------------
// Embedder — точка подключения нейронной модели (кирпич 2)
// ---------------------------------------------------------------------------

/// Источник эмбеддингов: инференс ГОТОВОЙ модели (никакого обучения).
///
/// Реализации: `HashEmbedder` (детерминированная проекция — конвейерные
/// тесты без модели); кирпич 2 добавит `FastEmbedder` (BGE-M3/nomic
/// через fastembed-rs, CPU ONNX). Трейт сознательно минимален: имя,
/// размерность, флаг нормализации и пакетное встраивание.
pub trait Embedder {
    /// Человекочитаемое имя модели (для логов и метаданных).
    fn name(&self) -> &str;
    /// Размерность выходных векторов.
    fn dim(&self) -> usize;
    /// L2-нормализованы ли выходы (косинус == IP).
    fn normalized(&self) -> bool;
    /// Встраивает пакет текстов; порядок сохраняется.
    fn embed_batch(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String>;
}

// ---------------------------------------------------------------------------
// HashEmbedder — детерминированная проекция (НЕ семантическая)
// ---------------------------------------------------------------------------

/// Детерминированная feature-hashing проекция текста в вектор.
///
/// **НЕ понимает язык**: токены UAX#29 хэшируются в фиксированные
/// позиции со знаками (3 позиции на токен), суммируются, L2-норм.
/// Назначение — тесты конвейера и MCP-потребители без модели:
/// одинаковые слова дают близкие векторы, разных — почти ортогональные.
/// Это инструментальная проекция, а не «понимание»: семантическую
/// близость даст только нейронный эмбеддер из кирпича 2.
#[derive(Debug, Clone)]
pub struct HashEmbedder {
    dim: usize,
    seed: u64,
}

impl HashEmbedder {
    /// Новая проекция фиксированной размерности.
    pub fn new(dim: usize, seed: u64) -> Self {
        assert!(dim >= 8, "HashEmbedder: размерность {dim} слишком мала");
        Self { dim, seed }
    }
}

impl Embedder for HashEmbedder {
    fn name(&self) -> &str {
        "hash-projection (не семантический)"
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn normalized(&self) -> bool {
        true
    }

    fn embed_batch(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        Ok(texts.iter().map(|t| self.embed_one(t)).collect())
    }
}

impl HashEmbedder {
    fn embed_one(&self, text: &str) -> Vec<f32> {
        use unicode_segmentation::UnicodeSegmentation;
        let mut v = vec![0.0f32; self.dim];
        for word in text.unicode_words() {
            let base = fnv1a(word.as_bytes()) ^ self.seed;
            for k in 0..3u64 {
                let mut z = base ^ k.wrapping_mul(0x9E37_79B9_7F4A_7C15);
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                let pos = (z >> 33) as usize % self.dim;
                let sign = if z & 1 == 0 { 1.0f32 } else { -1.0f32 };
                v[pos] += sign;
            }
        }
        let norm = (v.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>()).sqrt();
        if norm > 0.0 {
            for x in v.iter_mut() {
                *x /= norm as f32;
            }
        }
        v
    }
}

/// FNV-1a — быстрый детерминированный хэш байтов.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_embedder_is_deterministic() {
        let mut a = HashEmbedder::new(64, 1);
        let mut b = HashEmbedder::new(64, 1);
        let x = a.embed_batch(&["проклятые княжества", "the vector layer"]).unwrap();
        let y = b.embed_batch(&["проклятые княжества", "the vector layer"]).unwrap();
        assert_eq!(x, y);
        assert_eq!(x.len(), 2);
        assert_eq!(x[0].len(), 64);
    }

    #[test]
    fn hash_embedder_is_normalized() {
        let mut e = HashEmbedder::new(128, 5);
        let vs = e.embed_batch(&["нокс когти резонанс", "system vector index"]).unwrap();
        for v in &vs {
            let n = (v.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>()).sqrt();
            assert!((n - 1.0).abs() < 1e-5, "норма {n} != 1");
        }
    }

    #[test]
    fn hash_embedder_similarity_follows_word_overlap() {
        // Пересечение словаря → выше косинус; принцип инструментальной
        // проекции (не семантики!): только общие слова сближают.
        let mut e = HashEmbedder::new(256, 42);
        let vs = e
            .embed_batch(&[
                "нокс когти вонзил",
                "нокс когти вонзил снова",
                "протокол мьютекса",
            ])
            .unwrap();
        let ip = |a: &[f32], b: &[f32]| {
            a.iter().zip(b).map(|(x, y)| (*x as f64) * (*y as f64)).sum::<f64>()
        };
        let overlap = ip(&vs[0], &vs[1]);
        let disjoint = ip(&vs[0], &vs[2]);
        assert!(
            overlap > disjoint + 0.1,
            "пересечение словаря не сближает: {overlap} vs {disjoint}"
        );
    }

    #[test]
    fn embedder_contract_via_generic() {
        // Трейт работает полиморфно — кирпич 2 подключит BGE-M3 сюда же.
        fn dim_of(e: &mut dyn Embedder) -> usize {
            e.dim()
        }
        let mut e = HashEmbedder::new(64, 0);
        assert_eq!(dim_of(&mut e), 64);
        assert!(e.normalized());
        assert!(e.name().contains("hash"));
    }
}
