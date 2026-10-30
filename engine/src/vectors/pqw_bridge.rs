//! Мост `.pqw` → векторный слой (Part E, задачи 4.5.1 + 3.1).
//!
//! `PqwEmbedder` открывает нативный энкодер из контейнера `.pqw`
//! (mmap zero-copy + Sha256) и реализует трейт [`Embedder`] — точку
//! подключения, подготовленную ещё в кирпиче 1 (там был HashEmbedder,
//! здесь — настоящий нейронный forward на кернелах pqc). Дальше векторы
//! уходят в RaBitQ-субстрат/HNSW как у любого другого эмбеддера:
//! модель сменная, субстрат масштаба — неизменен.
//!
//! Токенизация: если в `.pqw` есть секция `__tokenizer__` (конвертер
//! реальных весов BGE-M3) — настоящий Unigram/Metaspace-конвейер
//! (`pqc::tokenizer`); для синтетических демо-моделей без секции —
//! детерминированный хэш-двойник `hash_token_ids`.
//!
//! CLI: `poler-engine --semantic dense --model bge-m3.pqw -q "…"`.

use std::path::Path;

use super::Embedder;
use crate::pqc::encoder::EncoderModel;
use crate::pqc::tokenizer::UnigramTokenizer;
use crate::pqc::hash_token_ids;

/// Эмбеддер поверх `.pqw`-энкодера (BGE-M3-класс).
pub struct PqwEmbedder {
    model: EncoderModel,
    vocab: u32,
    tokenizer: Option<UnigramTokenizer>,
}

impl PqwEmbedder {
    /// Открывает модель (с Sha256-верификацией весов).
    pub fn open(path: &Path) -> Result<Self, String> {
        let model = EncoderModel::open(path)?;
        let vocab = model.view().header().vocab as u32;
        let tokenizer = match model.view().tensor("__tokenizer__") {
            Some(t) => Some(UnigramTokenizer::parse(t.raw_bytes()?)?),
            None => None,
        };
        Ok(Self {
            model,
            vocab,
            tokenizer,
        })
    }

    /// Токенизация текста: настоящий Unigram или хэш-фолбэк.
    pub fn token_ids(&self, text: &str) -> Vec<u32> {
        match &self.tokenizer {
            Some(tk) => tk.encode(text),
            None => hash_token_ids(text, self.vocab),
        }
    }

    /// Токенизатор, если модель несёт секцию `__tokenizer__`.
    pub fn tokenizer(&self) -> Option<&UnigramTokenizer> {
        self.tokenizer.as_ref()
    }

    /// Доступ к энкодеру (параллельное эмбеддингирование корпуса).
    pub fn model(&self) -> &EncoderModel {
        &self.model
    }
}

impl Embedder for PqwEmbedder {
    fn name(&self) -> &str {
        "pqw-native-encoder (BGE-M3-класс, int8/int4 через pqc)"
    }

    fn dim(&self) -> usize {
        self.model.hidden()
    }

    fn normalized(&self) -> bool {
        // CLS-пулинг с L2-нормализацией — косинус == IP.
        true
    }

    fn embed_batch(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        // Энкодер ограничен max_pos (XLM-R: −2 на паддинг/сдвиг позиций).
        let cap = self.model.view().header().max_pos;
        let cap = cap.saturating_sub(if self.model.view().header().xlmr_positions() {
            2
        } else {
            0
        });
        let mut out = Vec::with_capacity(texts.len());
        for t in texts {
            let ids = self.token_ids(t);
            if ids.is_empty() {
                return Err(format!("текст без токенов: {t:?}"));
            }
            let ids = if ids.len() > cap { &ids[..cap] } else { &ids[..] };
            out.push(self.model.embed(ids)?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pqc::pqw::Quant;

    fn tmp_model() -> std::path::PathBuf {
        // Небольшая синтетическая модель для контрактов моста.
        let path = std::env::temp_dir().join(format!(
            "poler-bridge-{}.pqw",
            std::process::id()
        ));
        let (builder, _) =
            crate::pqc::encoder::synth_encoder(33, 1, 32, 4, 64, 64, 48, Quant::Int8);
        builder.write_to(&path).unwrap();
        path
    }

    #[test]
    fn pqw_embedder_satisfies_contract() {
        fn dim_of(e: &mut dyn Embedder) -> usize {
            e.dim()
        }
        let path = tmp_model();
        let mut e = PqwEmbedder::open(&path).unwrap();
        assert_eq!(dim_of(&mut e), 32);
        assert!(e.normalized());
        assert!(e.name().contains("pqw"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn pqw_embedder_batch_matches_direct_encoder() {
        let path = tmp_model();
        let mut e = PqwEmbedder::open(&path).unwrap();
        let texts = ["нокс вонзил когти", "the vector layer"];
        let vs = e.embed_batch(&texts).unwrap();
        assert_eq!(vs.len(), 2);
        for v in &vs {
            let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((n - 1.0).abs() < 1e-5, "норма {n}");
        }
        // Прямой вызов энкодера теми же токенами — тот же вектор.
        let ids = hash_token_ids(texts[0], 64);
        let direct = EncoderModel::open(&path).unwrap().embed(&ids).unwrap();
        assert_eq!(vs[0], direct);
        // Пустой текст — честная ошибка, не мусорный вектор.
        assert!(e.embed_batch(&["   "]).is_err());
        let _ = std::fs::remove_file(&path);
    }
}
