//! Лексикон кристалла — секция `LEXI` контейнера v4 (`POLER_Q4`).
//!
//! ## Физика секции
//!
//! Сенсорный кодировщик (RQ6+) односторонен: токен → `fnv1a64` →
//! координата `h mod d_pol`. Генерация речи (RQ17) требует обратной
//! карты — **какое слово какой координатой кристаллизовано**. Лексикон
//! накапливается во время обучения (доминантный токен на координату —
//! «главное слово» русла) и переживает рестарт бит-в-бит, в отличие от
//! TF-IDF статистики: словарь — часть мнения, а не инерции.
//!
//! Коллизии хеша честны: если два токена делят координату, координата
//! говорит словом с наибольшей частотой свидетельства — как полюс
//! МакВини выбирает выровненную фазу из суперпозиции вкладов.
//!
//! ## Раскладка секции (little-endian)
//!
//! ```text
//! 0x00  4   magic "LEXI"
//! 0x04  4   версия секции (u32, = 1)
//! 0x08  4   d_pol — перекрёстная проверка с заголовком контейнера
//! 0x0C  4   число записей N (u32, ≥ 1)
//! 0x10  ·   N записей: [u32 coord LE][u16 len LE][len байт UTF-8]
//! ```
//!
//! Координаты строго возрастают — детерминизм сериализации. Секция
//! стоит в контейнере v4 сразу за гироскопной (`topology_offset +
//! topology_len`) и заканчивается ровно в EOF: её длина выводима из
//! заголовка секции, отдельного поля смещения в заголовке контейнера
//! не требуется.

use crate::error::{PqwError, Result};

/// Магические байты секции лексикона.
pub const LEXI_MAGIC: [u8; 4] = *b"LEXI";
/// Версия раскладки секции, поддерживаемая этой сборкой.
pub const LEXI_SECTION_VERSION: u32 = 1;
/// Размер служебного заголовка секции.
pub const LEXI_HEADER_SIZE: usize = 0x10;
/// Потолок длины токена (u16-поле длины + здравый смысл: токены
/// кодировщика — прогоны буквенно-цифровых символов).
pub const LEXI_MAX_TOKEN: usize = 255;

/// Обратная карта кодировщика: координата → доминантный токен.
///
/// Записи отсортированы по координате, координаты уникальны. Пустой
/// лексикон не существует: нет слов — нет секции (контейнер остаётся v3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lexicon {
    entries: Vec<(u32, String)>,
}

impl Lexicon {
    /// Конструкция из сырых пар: сортировка по координате, валидация
    /// диапазона (`coord < d_pol`), непустых токенов разумной длины и
    /// уникальности координат.
    pub fn new(mut entries: Vec<(u32, String)>, d_pol: u32) -> Result<Lexicon> {
        entries.retain(|(_, t)| !t.is_empty());
        if entries.is_empty() {
            return Err(PqwError::Layout(
                "lexicon requires at least one entry (use a v3 container without LEXI)",
            ));
        }
        entries.sort_unstable_by_key(|&(c, _)| c);
        for w in entries.windows(2) {
            if w[0].0 == w[1].0 {
                return Err(PqwError::DuplicateIndex(w[1].0));
            }
        }
        for &(c, ref t) in &entries {
            if c >= d_pol {
                return Err(PqwError::BadIndex { index: c, d_pol });
            }
            if t.len() > LEXI_MAX_TOKEN {
                return Err(PqwError::Layout(
                    "lexicon token exceeds the u16 length field (255 bytes)",
                ));
            }
        }
        Ok(Lexicon { entries })
    }

    /// Число записей.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Лексикон непуст по контракту (метод для clippy).
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Записи `(координата, токен)` в порядке возрастания координат.
    pub fn entries(&self) -> &[(u32, String)] {
        &self.entries
    }

    /// Доминантный токен координаты — бинарный поиск по плотному
    /// отсортированному массиву, ноль аллокаций.
    pub fn token_of(&self, coord: u32) -> Option<&str> {
        self.entries
            .binary_search_by_key(&coord, |&(c, _)| c)
            .ok()
            .map(|i| self.entries[i].1.as_str())
    }

    /// Кодирование секции: 16 B служебных + N × (6 + len) байт.
    ///
    /// `d_pol` контейнера дублируется в заголовок секции — перекрёстная
    /// проверка при чтении (лексикон несовместимой размерности — отказ).
    pub(crate) fn encode(&self, d_pol: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(LEXI_HEADER_SIZE + self.entries.len() * 10);
        out.extend_from_slice(&LEXI_MAGIC);
        out.extend_from_slice(&LEXI_SECTION_VERSION.to_le_bytes());
        out.extend_from_slice(&d_pol.to_le_bytes());
        out.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        for &(coord, ref token) in &self.entries {
            out.extend_from_slice(&coord.to_le_bytes());
            out.extend_from_slice(&(token.len() as u16).to_le_bytes());
            out.extend_from_slice(token.as_bytes());
        }
        out
    }

    /// Полный разбор и валидация секции поверх среза: magic, версия,
    /// точная длина, диапазоны координат, строгий порядок, UTF-8.
    ///
    /// Срез обязан исчерпываться записями ровно до конца (никаких
    /// хвостовых байтов) — ридер подаёт хвост контейнера целиком.
    pub fn decode(bytes: &[u8], d_pol: u32) -> Result<Lexicon> {
        let (lex, used) = Self::decode_prefix(bytes, d_pol)?;
        if used != bytes.len() {
            return Err(PqwError::Layout(
                "lexicon section: trailing bytes after the last entry",
            ));
        }
        Ok(lex)
    }

    /// Разбор с возвратом числа потреблённых байтов — контейнер v5
    /// хранит за лексиконом секцию контекст-рефлекса `REFL` (RQ23):
    /// длина LEXI выводима из её собственного заголовка, EOF больше
    /// не равен концу лексикона.
    ///
    /// Возвращает лексикон и длину секции в байтах.
    pub fn decode_prefix(bytes: &[u8], d_pol: u32) -> Result<(Lexicon, usize)> {
        if bytes.len() < LEXI_HEADER_SIZE {
            return Err(PqwError::Truncated {
                need: LEXI_HEADER_SIZE,
                have: bytes.len(),
            });
        }
        if bytes[..4] != LEXI_MAGIC {
            return Err(PqwError::Layout("lexicon section: bad magic"));
        }
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != LEXI_SECTION_VERSION {
            return Err(PqwError::UnsupportedVersion(version));
        }
        let d_section = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if d_section != d_pol {
            return Err(PqwError::InconsistentTopology {
                field: "lexicon d_pol",
                expected: d_pol as u64,
                actual: d_section as u64,
            });
        }
        let n = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        if n == 0 {
            return Err(PqwError::Layout(
                "lexicon section requires at least one entry",
            ));
        }
        // Точная длина: ни байтом меньше (усечение), ни байтом больше
        // (хвост после секции запрещён).
        let mut body = &bytes[LEXI_HEADER_SIZE..];
        let mut entries: Vec<(u32, String)> = Vec::with_capacity(n.min(1 << 16));
        for k in 0..n {
            if body.len() < 6 {
                return Err(PqwError::Truncated {
                    need: LEXI_HEADER_SIZE + 6 * (k + 1),
                    have: bytes.len(),
                });
            }
            let coord = u32::from_le_bytes(body[0..4].try_into().unwrap());
            let len = u16::from_le_bytes([body[4], body[5]]) as usize;
            if len == 0 {
                return Err(PqwError::Layout("lexicon section: empty token"));
            }
            if body.len() < 6 + len {
                return Err(PqwError::Truncated {
                    need: LEXI_HEADER_SIZE + 6 * (k + 1) + len,
                    have: bytes.len(),
                });
            }
            let token = std::str::from_utf8(&body[6..6 + len])
                .map_err(|_| PqwError::Layout("lexicon section: token is not UTF-8"))?;
            if coord >= d_pol {
                return Err(PqwError::BadIndex { index: coord, d_pol });
            }
            if let Some(prev) = entries.last() {
                if prev.0 >= coord {
                    return Err(PqwError::UnsortedTopology);
                }
            }
            entries.push((coord, token.to_string()));
            body = &body[6 + len..];
        }
        let used = bytes.len() - body.len();
        Ok((Lexicon { entries }, used))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Lexicon {
        Lexicon::new(
            vec![
                (7, "решётка".to_string()),
                (3, "фаза".to_string()),
                (11, "трит".to_string()),
            ],
            16,
        )
        .unwrap()
    }

    #[test]
    fn roundtrip_bit_exact() {
        let lex = sample();
        let bytes = lex.encode(16);
        let back = Lexicon::decode(&bytes, 16).unwrap();
        assert_eq!(lex, back);
        // Плотный порядок: сортировка по координате на входе.
        assert_eq!(back.entries()[0], (3, "фаза".to_string()));
        assert_eq!(back.entries()[1], (7, "решётка".to_string()));
        assert_eq!(back.entries()[2], (11, "трит".to_string()));
    }

    #[test]
    fn token_of_binary_search() {
        let lex = sample();
        assert_eq!(lex.token_of(3), Some("фаза"));
        assert_eq!(lex.token_of(11), Some("трит"));
        assert_eq!(lex.token_of(5), None);
        assert_eq!(lex.token_of(16), None);
        assert_eq!(lex.len(), 3);
        assert!(!lex.is_empty());
    }

    #[test]
    fn new_rejects_empty_and_duplicates() {
        assert!(Lexicon::new(vec![], 8).is_err());
        assert!(Lexicon::new(vec![(0, String::new())], 8).is_err());
        // Дубликат координаты — конфликт доминантности, отказ.
        assert!(Lexicon::new(
            vec![(2, "a".into()), (2, "b".into())],
            8
        )
        .is_err());
        // Координата вне решётки.
        assert!(Lexicon::new(vec![(8, "a".into())], 8).is_err());
        // Токен длиннее u16-поля.
        let long = "x".repeat(256);
        assert!(Lexicon::new(vec![(1, long)], 8).is_err());
    }

    #[test]
    fn decode_rejects_corruption() {
        let lex = sample();
        let bytes = lex.encode(16);
        // Плохой magic.
        let mut bad = bytes.clone();
        bad[0] = b'X';
        assert!(Lexicon::decode(&bad, 16).is_err());
        // Хвостовые байты.
        let mut tail = bytes.clone();
        tail.push(0);
        assert!(Lexicon::decode(&tail, 16).is_err());
        // Усечение записи.
        assert!(Lexicon::decode(&bytes[..bytes.len() - 1], 16).is_err());
        // Несогласованный d_pol.
        assert!(Lexicon::decode(&bytes, 32).is_err());
        // Пустая секция (N = 0).
        let mut empty = Vec::new();
        empty.extend_from_slice(&LEXI_MAGIC);
        empty.extend_from_slice(&LEXI_SECTION_VERSION.to_le_bytes());
        empty.extend_from_slice(&16u32.to_le_bytes());
        empty.extend_from_slice(&0u32.to_le_bytes());
        assert!(Lexicon::decode(&empty, 16).is_err());
        // Слишком короткий заголовок.
        assert!(Lexicon::decode(&bytes[..8], 16).is_err());
    }

    #[test]
    fn collisions_dominant_token_wins() {
        // Две пары не могут делить координату в одном лексиконе —
        // доминантность разрешается на этапе строительства (builder).
        let lex = Lexicon::new(vec![(4, "энтропия".into())], 8).unwrap();
        assert_eq!(lex.token_of(4), Some("энтропия"));
    }

    #[test]
    fn decode_prefix_reports_consumed_bytes() {
        // v5-контракт: за лексиконом могут идти данные рефлекса —
        // decode_prefix возвращает точную длину секции.
        let lex = sample();
        let mut bytes = lex.encode(16);
        let consumed = bytes.len();
        bytes.extend_from_slice(b"REFL-GARBAGE-FOLLOW");
        let (back, used) = Lexicon::decode_prefix(&bytes, 16).unwrap();
        assert_eq!(used, consumed);
        assert_eq!(back.len(), 3);
        // Полный decode с теми же хвостовыми байтами — отказ (v4-контракт).
        assert!(Lexicon::decode(&bytes, 16).is_err());
        // Без хвоста decode сходится бит-в-бит.
        assert_eq!(Lexicon::decode(&bytes[..consumed], 16).unwrap(), back);
    }
}
