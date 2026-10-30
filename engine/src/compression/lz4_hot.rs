//! lz4_flex на горячем пути: парковка постингов watcher-состояния.
//!
//! ## Задача (v2.0, Приоритет 3.3 — PLAN_POLER_V2)
//!
//! Watcher-режим удерживает между ресканами для КАЖДОГО файла:
//! * `hits` — индексы токенов совпадений (`Vec<u32>`);
//! * `hit_keys` — байтовые позиции совпадений (`Vec<usize>` = 8 байт на хит).
//!
//! На частотных запросах это десятки мегабайт сырых массивов. lz4_flex
//! (чистый Rust, декомпрессия ~ГБ/с) сжимает LE-байты массивов при
//! парковке и разжимает по требованию: «горячие» постинги остаются
//! горячими — разжатие дешевле повторного скана файла.
//!
//! ## Кодирование
//!
//! Значения сериализуются как little-endian байты и сжимаются
//! `lz4_flex::compress_prepend_size` (размер в заголовке — self-describing
//! blob). Возрастающие последовательности (позиции хитов) имеют
//! старшие байты, стремящиеся к повторам — LZ4-матчер собирает их в
//! словарные ссылки; на типовых постингах выходит 2–5× сжатия.
//!
//! Дельта-кодирование НЕ применяется сознательно: varint-поток
//! разрушает байтовую выровненность, на которой живёт LZ4-матчер,
//! и замедляет декомпрессию; выгода по размеру на коротких постингах
//! не окупает усложнение.

/// Сжатый lz4-массив целых: самодостаточный blob.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PostingsStore {
    data: Vec<u8>,
    len: u32,
}

impl PostingsStore {
    /// Упаковать массив `u32` (LE-байты → lz4).
    pub fn from_u32(vals: &[u32]) -> Self {
        let mut raw = Vec::with_capacity(vals.len() * 4);
        for v in vals {
            raw.extend_from_slice(&v.to_le_bytes());
        }
        Self::pack(raw, vals.len() as u32)
    }

    /// Упаковать массив `usize` (LE-байты → lz4).
    pub fn from_usize(vals: &[usize]) -> Self {
        let mut raw = Vec::with_capacity(std::mem::size_of_val(vals));
        for v in vals {
            raw.extend_from_slice(&v.to_le_bytes());
        }
        Self::pack(raw, vals.len() as u32)
    }

    fn pack(raw: Vec<u8>, len: u32) -> Self {
        if raw.is_empty() {
            return Self { data: Vec::new(), len: 0 };
        }
        let data = lz4_flex::compress_prepend_size(&raw);
        Self { data, len }
    }

    /// Число упакованных значений.
    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Сжатые байты на диске/в памяти.
    pub fn compressed_bytes(&self) -> usize {
        self.data.len()
    }

    /// Сырые байты до сжатия (бенчмарк плотности).
    pub fn raw_bytes(&self) -> usize {
        self.len as usize * 4.max(std::mem::size_of::<usize>())
    }

    fn unpack(&self, width: usize) -> Option<Vec<u8>> {
        if self.data.is_empty() {
            return Some(Vec::new());
        }
        lz4_flex::decompress_size_prepended(&self.data).ok().map(|mut raw| {
            // обрезаем возможный хвост от выравнивания (незначим)
            raw.truncate(self.len as usize * width);
            raw
        })
    }

    /// Распаковать массив `u32`.
    pub fn to_u32(&self) -> Vec<u32> {
        let Some(raw) = self.unpack(4) else {
            return Vec::new();
        };
        raw.chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    /// Распаковать массив `usize`.
    pub fn to_usize(&self) -> Vec<usize> {
        let w = std::mem::size_of::<usize>();
        let Some(raw) = self.unpack(w) else {
            return Vec::new();
        };
        raw.chunks_exact(w)
            .map(|c| {
                let mut b = [0u8; 8];
                b[..w].copy_from_slice(c);
                usize::from_le_bytes(b)
            })
            .collect()
    }

    /// Есть ли значение в массиве (разжатие + линейный/бинарный поиск).
    /// Позиции хитов упорядочены по возрастанию — бинарный поиск.
    pub fn contains(&self, needle: usize) -> bool {
        self.to_usize().binary_search(&needle).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_u32() {
        let vals: Vec<u32> = (0..1000u32).map(|i| i * 3 + 7).collect();
        let s = PostingsStore::from_u32(&vals);
        assert_eq!(s.to_u32(), vals);
        assert_eq!(s.len(), 1000);
    }

    #[test]
    fn roundtrip_usize_ascending() {
        let mut v = 0usize;
        let vals: Vec<usize> = (0..5000)
            .map(|i| {
                v += 13 + (i % 40);
                v
            })
            .collect();
        let s = PostingsStore::from_usize(&vals);
        assert_eq!(s.to_usize(), vals);
        assert!(s.contains(*vals.last().unwrap()));
        assert!(!s.contains(1));
    }

    #[test]
    fn empty_and_single() {
        let e = PostingsStore::from_u32(&[]);
        assert!(e.is_empty());
        assert_eq!(e.to_u32(), Vec::<u32>::new());
        let s = PostingsStore::from_u32(&[42]);
        assert_eq!(s.to_u32(), vec![42]);
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn compresses_ascending_positions() {
        // Реалистичные байтовые позиции хитов (растут с шагом ~сотни).
        let mut pos = 0usize;
        let vals: Vec<usize> = (0..10_000)
            .map(|i| {
                pos += 120 + (i % 97);
                pos
            })
            .collect();
        let s = PostingsStore::from_usize(&vals);
        let raw = vals.len() * std::mem::size_of::<usize>();
        assert!(
            s.compressed_bytes() * 4 < raw * 3,
            "lz4 обязан заметно сжимать возрастающие позиции: {} из {}",
            s.compressed_bytes(),
            raw
        );
    }

    #[test]
    fn roundtrip_random_u32() {
        // Неупорядоченные значения — корректность важнее ratios.
        let mut x = 0x1234_5678_9ABC_DEF0u64;
        let vals: Vec<u32> = (0..777)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x >> 32) as u32
            })
            .collect();
        let s = PostingsStore::from_u32(&vals);
        assert_eq!(s.to_u32(), vals);
    }

    #[test]
    fn clone_is_independent() {
        let a = PostingsStore::from_u32(&[1, 2, 3]);
        let b = a.clone();
        assert_eq!(a.to_u32(), b.to_u32());
        assert_eq!(a, b);
    }
}
