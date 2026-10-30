//! `pqc` — суверенное ML-ядро poler-engine (PLAN_POLER_V2, Part E/F).
//!
//! Один статический бинарь, ноль внешних ML-библиотек: ни ONNX Runtime,
//! ни fastembed, ни candle, ни Python. Вся математика прямого прохода —
//! здесь, на чистом Rust c SIMD/AVX2 (runtime-детект, как у Teddy).
//!
//! ```text
//! poler-engine
//!   └── pqc
//!       ├── pqw      — контейнер весов .pqw v2 (mmap zero-copy, Sha256,
//!       │              выравнивание секций на страницу 4096)
//!       ├── tensor   — кернелы: dot fp32/int8/int4, LayerNorm/RMSNorm,
//!       │              GELU/SiLU, softmax, RoPE-таблицы, квантование
//!       ├── encoder  — BERT/XLM-R-энкодер (спина BGE-M3)
//!       ├── deberta  — DeBERTa-v2/v3-энкодер (спина GLiNER: disentangled
//!       │              attention, log-бакеты относительных позиций)
//!       ├── sha256   — собственный FIPS 180-4 хэш
//!       └── selftest — полный автономный цикл инференса (CLI --pqw-selftest)
//!
//! Потребители: vectors::pqw_bridge (dense-эмбеддинги),
//! ner::native_gliner (span-NER), llm::glm_engine (GLM-декодер).
//! ```
//!
//! ## Статус интеграции с POLER-Quantum-RS
//!
//! Вычислительное ядро разрабатывается в дереве poler-engine (вендор-модель
//! по канону FSST-кирпича: приватный репозиторий POLER-Quantum-RS нельзя
//! ставить git-зависимостью в публичный poler-engine). Формат `.pqw` v2
//! согласован с семейством контейнеров pqw-крейта (магия PQW2NN vs
//! POLER_QW v1 фазовых состояний); вынос тензора/энкодера в
//! crates/pqc-inference — отдельный кирпич после открытия репозитория.

pub mod encoder;
pub mod deberta;
pub mod nfc_tables;
pub mod pqw;
pub mod selftest;
pub mod sha256;
pub mod tensor;
pub mod tokenizer;

pub use encoder::{EncoderModel, EncoderOut};
pub use pqw::{ModelType, PqwBuilder, Quant, QuantizedWeightsView, TensorView};
pub use tokenizer::UnigramTokenizer;

/// FNV-1a — быстрый детерминированный хэш байтов.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Плейсхолдер-токенизатор до кирпича «реальные веса»: UAX#29-слова →
/// детерминированные ID через FNV-1a по модулю словаря.
///
/// **НЕ языковая токенизация** — конвейерный тест-двойник (как
/// HashEmbedder в векторном слое): одинаковые слова дают одинаковые ID,
/// порядок сохраняется. Настоящий XLM-R-sentencepiece/BPE-токенизатор
/// приезжает вместе с конвертером реальных весов (следующий кирпич).
pub fn hash_token_ids(text: &str, vocab: u32) -> Vec<u32> {
    use unicode_segmentation::UnicodeSegmentation;
    let v = vocab.max(1) as u64;
    text.unicode_words()
        .map(|w| (fnv1a(w.as_bytes()) % v) as u32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_token_ids_deterministic_and_bounded() {
        let a = hash_token_ids("проклятые княжества нокс", 64);
        let b = hash_token_ids("проклятые княжества нокс", 64);
        assert_eq!(a, b);
        assert_eq!(a.len(), 3);
        assert!(a.iter().all(|&t| t < 64));
        // Разные слова → (почти наверняка) разные ID; равные слова → равные ID.
        let c = hash_token_ids("нокс", 64);
        assert_eq!(a[2], c[0], "одно и то же слово обязано давать один ID");
    }

    #[test]
    fn hash_token_ids_empty_text() {
        assert!(hash_token_ids("", 64).is_empty());
        assert!(hash_token_ids("… … …", 64).is_empty());
    }
}
