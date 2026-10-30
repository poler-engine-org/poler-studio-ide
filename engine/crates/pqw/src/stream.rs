//! Zero-storage стриминг: текстовый чанк → фазовое состояние → контейнер
//! `.pqw` **целиком в RAM**, без единого обращения к диску.
//!
//! Назначение — интеграция с `poler-engine`: графовый поиск порождает
//! текстовые чанки, каждый чанк немедленно превращается в квантовое
//! состояние (ε-плотность → LENS-фильтр → тритование) и уходит в
//! вычислительное ядро `pqc`. Промежуточные файлы не пишутся вовсе.
//!
//! ## Схема кодирования (детерминизм без зависимостей)
//!
//! 1. **Токенизация** — максимальные прогоны буквенно-цифровых символов
//!    (`char::is_alphanumeric`, Unicode: латиница, кириллица, цифры…).
//!    Пунктуация и пробелы — разделители.
//! 2. **Хеш токена** — FNV-1a 64-bit по UTF-8 байтам. Младшие биты дают
//!    координату `h mod d_pol`, старший бит — полярность вклада `±1`.
//! 3. **Накопление** — `acc[i] ±= 1` на токен; частые токены дают плотные
//!    координаты, редкие — почти нулевые.
//! 4. **L∞-нормализация** — `p_i = acc_i / max|acc| ∈ [−1, 1]`.
//! 5. **ε-плотность (LENS)** — координаты с `|p_i| < ε` отбрасываются
//!    [`PqwWriter::add_state`]: остаются только плотные (частотные)
//!    координаты внимания.
//!
//! ```
//! use pqw::stream::TextPhaseEncoder;
//! use pqw::PqwReader;
//!
//! let enc = TextPhaseEncoder::new(64, 0.2)?;
//! let bytes = enc.to_container("квантовое ядро квантовое ядро poler")?;
//! let reader = PqwReader::from_bytes(&bytes)?;
//! assert!(reader.nnz() > 0); // плотные координаты выжили после LENS
//! # Ok::<(), pqw::PqwError>(())
//! ```

use crate::error::{PqwError, Result};
use crate::header::HyperParams;
use crate::writer::PqwWriter;

/// Эталонный кодировщик текстовых чанков в фазовые состояния.
#[derive(Clone, Debug)]
pub struct TextPhaseEncoder {
    d_pol: u32,
    hyper: HyperParams,
}

impl TextPhaseEncoder {
    /// Новый кодировщик: размерность `d_pol ≥ 1` и порог ε-плотности
    /// LENS `epsilon ∈ [0, 1]`.
    pub fn new(d_pol: u32, epsilon_threshold: f32) -> Result<TextPhaseEncoder> {
        if d_pol == 0 {
            return Err(PqwError::BadDimension(d_pol));
        }
        if !epsilon_threshold.is_finite() || epsilon_threshold < 0.0 || epsilon_threshold > 1.0 {
            return Err(PqwError::BadValue(epsilon_threshold));
        }
        Ok(TextPhaseEncoder {
            d_pol,
            hyper: HyperParams {
                epsilon_threshold,
                ..HyperParams::default()
            },
        })
    }

    /// Переопределить гиперпараметры потока (ε сохраняется из `new`).
    pub fn with_hyperparams(mut self, eta: f32, gamma: f32, rho: f32) -> TextPhaseEncoder {
        self.hyper.eta = eta;
        self.hyper.gamma = gamma;
        self.hyper.rho = rho;
        self
    }

    /// Размерность состояния.
    pub fn d_pol(&self) -> u32 {
        self.d_pol
    }

    /// Порог ε-плотности LENS.
    pub fn epsilon(&self) -> f32 {
        self.hyper.epsilon_threshold
    }

    /// Гиперпараметры, попадающие в заголовок контейнера.
    pub fn hyperparams(&self) -> HyperParams {
        self.hyper
    }

    /// Плотный фазовый вектор чанка (до LENS-фильтра): накопленные
    /// знаки токенов, L∞-нормализованные в `[−1, 1]`.
    ///
    /// Пустой текст (или текст без токенов) даёт нулевой вектор.
    pub fn encode(&self, text: &str) -> Vec<f32> {
        let d = self.d_pol as usize;
        let mut acc = vec![0i32; d];
        for token in tokenize(text) {
            let h = crate::checksum::fnv1a64(token.as_bytes());
            let idx = (h % self.d_pol as u64) as usize;
            // Старший бит хеша отделён от младших (индекс) — полярность
            // вклада не коррелирует с координатой при d_pol | 2^k.
            let sign: i32 = if (h >> 63) & 1 == 1 { 1 } else { -1 };
            acc[idx] += sign;
        }
        let max = acc.iter().map(|a| a.unsigned_abs()).max().unwrap_or(0) as f32;
        if max == 0.0 {
            return vec![0.0; d];
        }
        acc.iter().map(|&a| a as f32 / max).collect()
    }

    /// Числo токенов в чанке (диагностика ε-плотности).
    pub fn token_count(&self, text: &str) -> usize {
        tokenize(text).count()
    }

    /// Ин-мемори контейнер `.pqw` чанка: ε-плотность → LENS → триты.
    ///
    /// Zero-storage: байты собираются в RAM, диск не касается.
    pub fn to_container(&self, text: &str) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        self.write_container(text, &mut out)?;
        Ok(out)
    }

    /// Контейнер чанка, дописанный в чужой буфер: переиспользование
    /// аллокаций между чанками потоковой обработки.
    pub fn write_container(&self, text: &str, out: &mut Vec<u8>) -> Result<()> {
        let state = self.encode(text);
        let writer = PqwWriter::new(self.d_pol)?.hyperparams(
            self.hyper.eta,
            self.hyper.gamma,
            self.hyper.rho,
            self.hyper.epsilon_threshold,
        );
        let mut writer = writer;
        writer.add_state(&state)?;
        writer.write_to_vec(out)
    }
}

/// Токены чанка: прогоны буквенно-цифровых символов (Unicode).
/// Аллокации нет — итератор по срезам исходной строки.
pub fn tokenize(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        let start = rest.find(|c: char| c.is_alphanumeric())?;
        let tail = &rest[start..];
        let len = tail
            .find(|c: char| !c.is_alphanumeric())
            .unwrap_or(tail.len());
        let token = &tail[..len];
        rest = &tail[len..];
        Some(token)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksum::fnv1a64;
    use crate::PqwReader;

    fn idx_of(token: &str, d: u32) -> usize {
        (fnv1a64(token.as_bytes()) % d as u64) as usize
    }

    fn sign_of(token: &str) -> f32 {
        if (fnv1a64(token.as_bytes()) >> 63) & 1 == 1 {
            1.0
        } else {
            -1.0
        }
    }

    #[test]
    fn rejects_bad_dimension_and_epsilon() {
        assert!(TextPhaseEncoder::new(0, 0.1).is_err());
        assert!(TextPhaseEncoder::new(8, -0.1).is_err());
        assert!(TextPhaseEncoder::new(8, 1.5).is_err());
        assert!(TextPhaseEncoder::new(8, f32::NAN).is_err());
        assert!(TextPhaseEncoder::new(8, 0.0).is_ok());
        assert!(TextPhaseEncoder::new(8, 1.0).is_ok());
    }

    #[test]
    fn tokenize_unicode_words() {
        let toks: Vec<&str> = tokenize("Квантовое ядро poler-quantum v0.1! 42").collect();
        assert_eq!(
            toks,
            vec!["Квантовое", "ядро", "poler", "quantum", "v0", "1", "42"]
        );
        assert_eq!(tokenize("… , — ;").count(), 0);
        assert_eq!(tokenize("").count(), 0);
    }

    #[test]
    fn encode_linf_normalized_and_deterministic() {
        let enc = TextPhaseEncoder::new(16, 0.05).unwrap();
        let text = "alpha alpha alpha beta";
        let a = enc.encode(text);
        let b = enc.encode(text);
        assert_eq!(a, b);
        // alpha ×3 — вся плотность; beta ×1 → |p| = 1/3.
        let ia = idx_of("alpha", 16);
        let ib = idx_of("beta", 16);
        assert!((a[ia].abs() - 1.0).abs() < 1e-6);
        assert!((a[ib].abs() - 1.0 / 3.0).abs() < 1e-6);
        assert!(a.iter().all(|p| p.abs() <= 1.0));
        // Полярность следует старшему биту хеша.
        assert!((a[ia] - 3.0 * sign_of("alpha") / 3.0).abs() < 1e-6);
    }

    #[test]
    fn encode_empty_text_is_zero_state() {
        let enc = TextPhaseEncoder::new(8, 0.1).unwrap();
        assert!(enc.encode(" … !!! ").iter().all(|&p| p == 0.0));
    }

    #[test]
    fn lens_filters_epsilon_density() {
        // d=8, ε=0.4: выживает только alpha (|p|=1), beta (1/3) отбрасывается.
        let enc = TextPhaseEncoder::new(8, 0.4).unwrap();
        let bytes = enc.to_container("alpha alpha alpha beta").unwrap();
        let reader = PqwReader::from_bytes(&bytes).unwrap();
        assert_eq!(reader.nnz(), 1);
        assert_eq!(reader.indices().to_vec(), vec![idx_of("alpha", 8) as u32]);
        // Деквантование трита: p̂ = sign(±1)·(σ/63) — |p̂| ≥ 1 − 1/126.
        let (_, p) = reader.decoded().next().unwrap();
        assert!(p.abs() > 1.0 - 1.0 / 126.0);
        assert!(p.signum() == sign_of("alpha").signum() as f64);
    }

    #[test]
    fn container_roundtrip_zero_storage() {
        let enc = TextPhaseEncoder::new(128, 0.15).unwrap();
        let text = "теория фазового континуума теория тритов теория";
        let bytes = enc.to_container(text).unwrap();
        let reader = PqwReader::from_bytes(&bytes).unwrap();
        assert_eq!(reader.d_pol(), 128);
        assert!(reader.nnz() >= 2);
        assert!(reader.verify_payload().is_ok());
        assert!((reader.hyperparams().epsilon_threshold - 0.15).abs() < 1e-6);
    }

    #[test]
    fn write_container_appends_to_buffer() {
        let enc = TextPhaseEncoder::new(32, 0.1).unwrap();
        let mut buf = b"PREFIX".to_vec();
        enc.write_container("alpha alpha alpha", &mut buf).unwrap();
        let n = buf.len();
        enc.write_container("alpha alpha alpha", &mut buf).unwrap();
        // Два контейнера дописаны в хвост, префикс цел.
        assert_eq!(&buf[..6], b"PREFIX");
        let one = &buf[6..n];
        let two = &buf[n..];
        assert_eq!(one, two); // детерминизм сериализации
        assert!(PqwReader::from_bytes(one).is_ok());
        assert!(PqwReader::from_bytes(two).is_ok());
    }

    #[test]
    fn hyperparams_land_in_header() {
        let enc = TextPhaseEncoder::new(16, 0.1)
            .unwrap()
            .with_hyperparams(0.7, 0.3, 0.9);
        let bytes = enc.to_container("x y z").unwrap();
        let reader = PqwReader::from_bytes(&bytes).unwrap();
        let h = reader.hyperparams();
        assert!((h.eta - 0.7).abs() < 1e-6);
        assert!((h.gamma - 0.3).abs() < 1e-6);
        assert!((h.rho - 0.9).abs() < 1e-6);
        assert!((h.epsilon_threshold - 0.1).abs() < 1e-6);
    }

    #[test]
    fn token_count_matches_tokenize() {
        let enc = TextPhaseEncoder::new(8, 0.1).unwrap();
        assert_eq!(enc.token_count("a b c d"), 4);
        assert_eq!(enc.token_count(""), 0);
    }
}
