//! zstd-словарь для doc store: сжатие текстов страниц веб-индекса.
//!
//! ## Задача (v2.0, Приоритет 3.2 — PLAN_POLER_V2)
//!
//! SQLite-таблица `pages` хранит полный текст каждой страницы
//! (`pages.text`) — на краулах это доминирующая часть объёма БД.
//! zstd со обученным словарём (обучение на сэмпле текстов самого
//! корпуса) сжимает HTML-очищенный текст в 3–5×: словарь даёт общий
//! контекст коротким страницам, где «холодный» zstd почти не выигрывает.
//!
//! ## Схема хранения (обратная совместимость)
//!
//! * новая колонка `pages.text_c BLOB`; сжатые строки пишутся в неё,
//!   `pages.text` получает `''`;
//! * строки старых версий (`text_c IS NULL`) читаются как раньше —
//!   ленивая миграция без ALTER-переписывания таблицы;
//! * каждый blob самодокументирован заголовком `[b'P', b'Z', mode]`:
//!   `mode 0` — zstd без словаря (первые страницы, словарь ещё не обучен),
//!   `mode 1` — zstd со словарём (после обучения; словарь иммутабелен,
//!   хранится в `meta['zstd_dict']`, генераций не заводим — суверенная
//!   простота, документы текстовых краулов стабильны);
//! * декомпрессия — только при материализации top-N результатов поиска
//!   и при переиндексации термов старых строк.
//!
//! ## Принцип «инструмент, не ИИ»
//!
//! Слой байтовый и семантически слеп: что дали — то сжали, что запросили —
//! то разжали. Никаких решений о контенте.

use std::io;

/// Магия blob'а doc store.
const BLOB_MAGIC: [u8; 2] = *b"PZ";
/// Режим: zstd без словаря.
const MODE_PLAIN: u8 = 0;
/// Режим: zstd со словарём.
const MODE_DICT: u8 = 1;
/// Уровень сжатия (значение по умолчанию zstd: баланс скорость/плотность).
const LEVEL: i32 = 3;
/// Словарь обучается, когда накоплено столько страниц.
pub const DICT_TRAIN_MIN_PAGES: usize = 16;
/// Сколько страниц берётся в обучение (свежие, до 8 КБ каждая).
const DICT_TRAIN_SAMPLE_PAGES: usize = 256;
const DICT_TRAIN_SAMPLE_BYTES: usize = 8 * 1024;
/// Верхняя граница размера словаря.
const DICT_MAX_SIZE: usize = 32 * 1024;

/// Кодек doc store: сжатие/разжатие текстов с опциональным словарём.
///
/// Подготовленные словари (`EncoderDictionary::copy`) владеют собственной
/// копией байтов (`'static`) — кодекс свободен от самозаимствований;
/// компрессоры собираются по требованию (attach готового CDict — микросекунды,
/// апсерты страниц и материализация top-N несопоставимо дороже).
pub struct DocStoreCodec {
    /// None — plain-режим (словарь ещё не обучен).
    enc: Option<zstd::dict::EncoderDictionary<'static>>,
    dec: Option<zstd::dict::DecoderDictionary<'static>>,
    /// Байты словаря для персистентности в meta-таблице.
    dict_bytes: Option<Vec<u8>>,
}

impl DocStoreCodec {
    /// Кодек без словаря (до обучения).
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            enc: None,
            dec: None,
            dict_bytes: None,
        })
    }

    /// Кодек со словарём (после загрузки/обучения).
    pub fn with_dict(dict: Vec<u8>) -> io::Result<Self> {
        let mut c = Self::new()?;
        c.install_dict(dict)?;
        Ok(c)
    }

    /// Установить словарь (единожды: при открытии БД со словарём или
    /// после первого обучения; словарь иммутабелен — см. доку модуля).
    pub fn install_dict(&mut self, dict: Vec<u8>) -> io::Result<()> {
        if dict.is_empty() {
            return Ok(());
        }
        self.enc = Some(zstd::dict::EncoderDictionary::copy(&dict, LEVEL));
        self.dec = Some(zstd::dict::DecoderDictionary::copy(&dict));
        self.dict_bytes = Some(dict);
        Ok(())
    }

    pub fn has_dict(&self) -> bool {
        self.enc.is_some()
    }

    /// Сжать текст страницы: `[P][Z][mode] + zstd-фрейм`.
    pub fn compress(&self, text: &str) -> io::Result<Vec<u8>> {
        let raw = text.as_bytes();
        if raw.is_empty() {
            return Ok(Vec::new()); // пустой текст не кодируем вовсе
        }
        let mut out = Vec::with_capacity(raw.len() / 3 + 16);
        out.extend_from_slice(&BLOB_MAGIC);
        if let Some(d) = &self.enc {
            out.push(MODE_DICT);
            let mut c = zstd::bulk::Compressor::with_prepared_dictionary(d)?;
            out.extend_from_slice(&c.compress(raw)?);
        } else {
            out.push(MODE_PLAIN);
            let mut c = zstd::bulk::Compressor::new(LEVEL)?;
            out.extend_from_slice(&c.compress(raw)?);
        }
        Ok(out)
    }

    /// Разжать blob (или вернуть исходные байты, если это не наш формат —
    /// защита от повреждений: текст восстанавливается «как есть»).
    pub fn decompress(&self, blob: &[u8]) -> io::Result<String> {
        if blob.len() < 3 || blob[..2] != BLOB_MAGIC {
            // не наш формат — считаем сырым текстом (legacy/повреждение)
            return Ok(String::from_utf8_lossy(blob).into_owned());
        }
        let mode = blob[2];
        let payload = &blob[3..];
        let raw = match mode {
            MODE_DICT => {
                let d = self.dec.as_ref().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "blob требует словарь, которого нет")
                })?;
                let mut dec = zstd::bulk::Decompressor::with_prepared_dictionary(d)?;
                dec.decompress(payload, 512 * 1024)?
            }
            MODE_PLAIN => {
                let mut dec = zstd::bulk::Decompressor::new()?;
                dec.decompress(payload, 512 * 1024)?
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("неизвестный режим doc-blob: {mode}"),
                ))
            }
        };
        Ok(String::from_utf8_lossy(&raw).into_owned())
    }

    /// Обучить словарь на сэмпле текстов страниц.
    ///
    /// Возвращает `None`, если zstd-тренер отказался работать
    /// (слишком мало/однообразно данных) — кодек продолжит в plain-режиме.
    pub fn train_dict_from_texts(texts: &[&str]) -> Option<Vec<u8>> {
        if texts.len() < DICT_TRAIN_MIN_PAGES {
            return None;
        }
        let samples: Vec<Vec<u8>> = texts
            .iter()
            .take(DICT_TRAIN_SAMPLE_PAGES)
            .map(|t| t.as_bytes().iter().take(DICT_TRAIN_SAMPLE_BYTES).cloned().collect())
            .collect();
        let refs: Vec<&[u8]> = samples.iter().map(|v| v.as_slice()).collect();
        zstd::dict::from_samples(&refs, DICT_MAX_SIZE).ok()
    }

    /// Словарь для персистентности (meta-таблица).
    pub fn dict_bytes(&self) -> Option<&[u8]> {
        self.dict_bytes.as_deref()
    }
}

impl Default for DocStoreCodec {
    fn default() -> Self {
        Self::new().expect("zstd-кодек без словаря не может провалиться")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(n: usize, topic: &str) -> String {
        format!(
            "Страница {n} о {topic}. Текст страницы содержит слова {topic} \
             и общие служебные обороты: содержание, навигация, обратная связь, \
             поиск по сайту, редактировать, версия для печати. "
        ).repeat(3 + n % 5)
    }

    #[test]
    fn roundtrip_plain_mode() {
        let c = DocStoreCodec::new().unwrap();
        assert!(!c.has_dict());
        for text in ["короткий", &page(1, "нокс"), &page(2, "резонанс")] {
            let blob = c.compress(text).unwrap();
            assert_eq!(blob[..2], BLOB_MAGIC);
            assert_eq!(blob[2], MODE_PLAIN);
            assert_eq!(c.decompress(&blob).unwrap(), text);
        }
    }

    #[test]
    fn roundtrip_dict_mode() {
        let corpus: Vec<String> = (0..64).map(|i| page(i, "протокол")).collect();
        let refs: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let dict = DocStoreCodec::train_dict_from_texts(&refs).expect("словарь обязан обучиться");
        let c = DocStoreCodec::with_dict(dict).unwrap();
        assert!(c.has_dict());
        for text in &corpus {
            let blob = c.compress(text).unwrap();
            assert_eq!(blob[2], MODE_DICT);
            assert_eq!(c.decompress(&blob).unwrap(), *text);
        }
    }

    #[test]
    fn dict_beats_plain_on_short_pages() {
        // Короткие страницы — случай, где словарь даёт фору.
        let corpus: Vec<String> = (0..64).map(|i| page(i % 8, "вектор")).collect();
        let refs: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let dict = DocStoreCodec::train_dict_from_texts(&refs).expect("словарь");
        let plain = DocStoreCodec::new().unwrap();
        let dictc = DocStoreCodec::with_dict(dict).unwrap();
        let short: Vec<&str> = corpus.iter().map(|s| s.as_str()).take(8).collect();
        let p: usize = short.iter().map(|t| plain.compress(t).unwrap().len()).sum();
        let d: usize = short.iter().map(|t| dictc.compress(t).unwrap().len()).sum();
        assert!(d < p, "словарь должен сжимать короткие страницы лучше: dict={d} plain={p}");
    }

    #[test]
    fn empty_text_is_empty_blob() {
        let c = DocStoreCodec::new().unwrap();
        assert!(c.compress("").unwrap().is_empty());
        assert_eq!(c.decompress(&[]).unwrap(), "");
    }

    #[test]
    fn legacy_bytes_pass_through() {
        // Не-blob байты (старые строки/повреждение) — как сырой текст.
        let c = DocStoreCodec::new().unwrap();
        assert_eq!(c.decompress("просто текст".as_bytes()).unwrap(), "просто текст");
        assert_eq!(c.decompress(b"P").unwrap(), "P");
        assert_eq!(c.decompress(b"PZ").unwrap(), "PZ");
    }

    #[test]
    fn dict_blob_requires_dict() {
        let corpus: Vec<String> = (0..32).map(|i| page(i, "система")).collect();
        let refs: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let dict = DocStoreCodec::train_dict_from_texts(&refs).expect("словарь");
        let c = DocStoreCodec::with_dict(dict).unwrap();
        let blob = c.compress(&corpus[0]).unwrap();
        // Читает кодек БЕЗ словаря — обязан отказаться, а не портить данные.
        let bare = DocStoreCodec::new().unwrap();
        assert!(bare.decompress(&blob).is_err());
    }

    #[test]
    fn train_needs_min_pages() {
        assert!(DocStoreCodec::train_dict_from_texts(&["одна страница"]).is_none());
    }
}
