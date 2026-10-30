//! Контекст-рефлекс `W` — секция `REFL` контейнера v5 (`POLER_Q5`).
//!
//! ## Физика секции
//!
//! Русла `J` — структурная долговременная память: каналы смысла,
//! накопленные гистерезисом двух свидетелей. Но волна мысли живёт ещё
//! и **контекстом**: кольцом последних событий сенсорного потока.
//! При рестарте кольцо обнулялось — мозг узнавал свою структуру, но не
//! помнил, о чём только что говорил. Контекст-рефлекс `W` — динамический
//! след диалога: последние события `(координата, полярность)` в порядке
//! появления, имя собеседника и счётчик реплик. Новая сессия поднимает
//! кольцо гироскопа из хвоста следа — нить разговора не рвётся на
//! границе сессий, русла продолжают расти сквозь рестарт.
//!
//! ## Раскладка секции (little-endian)
//!
//! ```text
//! 0x00  4   magic "REFL"
//! 0x04  4   версия секции (u32, = 1)
//! 0x08  4   d_pol — перекрёстная проверка с заголовком контейнера
//! 0x0C  4   число событий следа N (u32, ≥ 1)
//! 0x10  8   реплик диалога turns (u64)
//! 0x18  4   длина имени собеседника L (u32; 0 = аноним)
//! 0x1C  4   reserved (u32, = 0)
//! 0x20  ·   L байт UTF-8 имени собеседника
//!       ·   N записей: [coord u16|u32 LE][sign i8]
//! ```
//!
//! События хранятся в хронологическом порядке (старые → новые):
//! восстановление кольца берёт хвост. Полярность `sign ∈ {−1, 0, +1}` —
//! та же конвенция, что у сенсорного потока гироскопа. Секция стоит в
//! контейнере v5 сразу за лексиконом (`LEXI`) и заканчивается ровно
//! в EOF: смещение выводимо разбором секции лексикона.

use crate::error::{PqwError, Result};

/// Магические байты секции контекст-рефлекса.
pub const REFL_MAGIC: [u8; 4] = *b"REFL";
/// Версия раскладки секции, поддерживаемая этой сборкой.
pub const REFL_SECTION_VERSION: u32 = 1;
/// Размер служебного заголовка секции.
pub const REFL_HEADER_SIZE: usize = 0x20;
/// Потолок длины имени собеседника (UTF-8 байт): имя — маркер личности,
/// не биография.
pub const REFL_MAX_NAME: usize = 128;
/// Потолок числа событий следа в секции: след — рабочая память,
/// не корпус (256 событий ≈ последняя страница диалога).
pub const REFL_MAX_EVENTS: usize = 4096;

/// Полные данные контекст-рефлекса для записи в контейнер v5.
#[derive(Clone, Debug, PartialEq)]
pub struct ReflexData {
    /// Имя собеседника (пустая строка — аноним).
    interlocutor: String,
    /// Число реплик диалога «вопрос → ответ» (u64: диалог долгий).
    turns: u64,
    /// События следа `(координата, полярность)` в хронологическом порядке.
    events: Vec<(u32, i8)>,
}

impl ReflexData {
    /// Конструкция с валидацией: события в диапазоне `coord < d_pol`,
    /// полярности `{−1, 0, +1}`, след непуст (пустой рефлекс — это v4),
    /// имя разумной длины.
    pub fn new(
        interlocutor: impl Into<String>,
        turns: u64,
        events: Vec<(u32, i8)>,
        d_pol: u32,
    ) -> Result<ReflexData> {
        let interlocutor = interlocutor.into();
        if interlocutor.len() > REFL_MAX_NAME {
            return Err(PqwError::Layout(
                "reflex section: interlocutor name exceeds 128 bytes",
            ));
        }
        if events.is_empty() {
            return Err(PqwError::Layout(
                "reflex section requires at least one event (use a v4 container without REFL)",
            ));
        }
        if events.len() > REFL_MAX_EVENTS {
            return Err(PqwError::Layout(
                "reflex section: trail exceeds 4096 events",
            ));
        }
        for &(c, s) in &events {
            if c >= d_pol {
                return Err(PqwError::BadIndex { index: c, d_pol });
            }
            if !(-1..=1).contains(&s) {
                return Err(PqwError::Layout(
                    "reflex section: event polarity must be -1, 0 or +1",
                ));
            }
        }
        Ok(ReflexData {
            interlocutor,
            turns,
            events,
        })
    }

    /// Имя собеседника.
    pub fn interlocutor(&self) -> &str {
        &self.interlocutor
    }

    /// Число реплик диалога.
    pub fn turns(&self) -> u64 {
        self.turns
    }

    /// События следа в хронологическом порядке.
    pub fn events(&self) -> &[(u32, i8)] {
        &self.events
    }

    /// Кодирование секции: 32 B служебных + имя + N × (2w+1) B записей,
    /// где w = 2 при `index16` (u16-координаты), иначе 4.
    pub fn encode(&self, index16: bool) -> Result<Vec<u8>> {
        let record = if index16 { 3usize } else { 5 };
        let mut out =
            Vec::with_capacity(REFL_HEADER_SIZE + self.interlocutor.len() + self.events.len() * record);
        out.extend_from_slice(&REFL_MAGIC);
        out.extend_from_slice(&REFL_SECTION_VERSION.to_le_bytes());
        // d_pol подставляет писатель контейнера (перекрёстная проверка).
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(self.events.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.turns.to_le_bytes());
        out.extend_from_slice(&(self.interlocutor.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(self.interlocutor.as_bytes());
        for &(c, s) in &self.events {
            if index16 {
                debug_assert!(c <= u16::MAX as u32);
                out.extend_from_slice(&(c as u16).to_le_bytes());
            } else {
                out.extend_from_slice(&c.to_le_bytes());
            }
            out.push(s as u8);
        }
        Ok(out)
    }

    /// Кодирование с явным `d_pol` для перекрёстной проверки при чтении.
    pub fn encode_with_dpol(&self, index16: bool, d_pol: u32) -> Result<Vec<u8>> {
        let mut out = self.encode(index16)?;
        out[8..12].copy_from_slice(&d_pol.to_le_bytes());
        Ok(out)
    }
}

/// Разобранная секция контекст-рефлекса (zero-copy не критична: след —
/// сотни событий, не миллионы).
#[derive(Clone, Debug, PartialEq)]
pub struct ReflexSection {
    interlocutor: String,
    turns: u64,
    events: Vec<(u32, i8)>,
    index16: bool,
}

impl ReflexSection {
    /// Полный разбор и валидация секции поверх среза.
    ///
    /// Проверяются: magic, версия, совпадение `d_pol`, точная длина,
    /// диапазон координат, полярности `{−1, 0, +1}`, UTF-8 имени.
    /// Возвращает также число потреблённых байтов — за секцией в
    /// контейнере v5 данных нет (EOF), но интроспекция любит точность.
    pub fn decode(bytes: &[u8], d_pol: u32, index16: bool) -> Result<(ReflexSection, usize)> {
        if bytes.len() < REFL_HEADER_SIZE {
            return Err(PqwError::Truncated {
                need: REFL_HEADER_SIZE,
                have: bytes.len(),
            });
        }
        if bytes[..4] != REFL_MAGIC {
            return Err(PqwError::Layout("reflex section: bad magic"));
        }
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != REFL_SECTION_VERSION {
            return Err(PqwError::UnsupportedVersion(version));
        }
        let d_section = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if d_section != d_pol {
            return Err(PqwError::InconsistentTopology {
                field: "reflex d_pol",
                expected: d_pol as u64,
                actual: d_section as u64,
            });
        }
        let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        if count == 0 {
            return Err(PqwError::Layout(
                "reflex section requires at least one event",
            ));
        }
        if count > REFL_MAX_EVENTS {
            return Err(PqwError::Layout("reflex section: trail exceeds 4096 events"));
        }
        let turns = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        let name_len = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as usize;
        if name_len > REFL_MAX_NAME {
            return Err(PqwError::Layout(
                "reflex section: interlocutor name exceeds 128 bytes",
            ));
        }
        let reserved = u32::from_le_bytes(bytes[28..32].try_into().unwrap());
        if reserved != 0 {
            return Err(PqwError::ReservedBits {
                value: reserved as u64,
            });
        }
        let record = if index16 { 3usize } else { 5 };
        let expected = REFL_HEADER_SIZE + name_len + count * record;
        if bytes.len() != expected {
            return Err(PqwError::Layout(
                "reflex section: length does not match name + event count",
            ));
        }

        let name = std::str::from_utf8(&bytes[REFL_HEADER_SIZE..REFL_HEADER_SIZE + name_len])
            .map_err(|_| PqwError::Layout("reflex section: interlocutor name is not UTF-8"))?;
        let mut events = Vec::with_capacity(count);
        let mut body = &bytes[REFL_HEADER_SIZE + name_len..];
        for _ in 0..count {
            let (c, rest) = if index16 {
                let c = u16::from_le_bytes([body[0], body[1]]) as u32;
                (c, &body[3..])
            } else {
                let c = u32::from_le_bytes(body[0..4].try_into().unwrap());
                (c, &body[5..])
            };
            let s = body[if index16 { 2 } else { 4 }] as i8;
            body = rest;
            if c >= d_pol {
                return Err(PqwError::BadIndex { index: c, d_pol });
            }
            if !(-1..=1).contains(&s) {
                return Err(PqwError::Layout(
                    "reflex section: event polarity must be -1, 0 or +1",
                ));
            }
            events.push((c, s));
        }
        Ok((
            ReflexSection {
                interlocutor: name.to_string(),
                turns,
                events,
                index16,
            },
            expected,
        ))
    }

    /// Имя собеседника (пустая строка — аноним).
    pub fn interlocutor(&self) -> &str {
        &self.interlocutor
    }

    /// Число реплик диалога.
    pub fn turns(&self) -> u64 {
        self.turns
    }

    /// События следа в хронологическом порядке.
    pub fn events(&self) -> &[(u32, i8)] {
        &self.events
    }

    /// Хвост следа — последние `n` событий (восстановление кольца
    /// гироскопа при resume).
    pub fn trail_tail(&self, n: usize) -> &[(u32, i8)] {
        let take = n.min(self.events.len());
        &self.events[self.events.len() - take..]
    }

    /// Ширина координат событий (true → u16).
    pub fn index16(&self) -> bool {
        self.index16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ReflexData {
        ReflexData::new(
            "Иван",
            12,
            vec![(3, 1), (7, -1), (9, 1), (1, 0)],
            16,
        )
        .unwrap()
    }

    #[test]
    fn data_rejects_bad_input() {
        // Пустой след — это не рефлекс (контейнер остаётся v4).
        assert!(ReflexData::new("", 0, vec![], 16).is_err());
        // Координата вне решётки.
        assert!(ReflexData::new("", 1, vec![(16, 1)], 16).is_err());
        // Полярность вне {−1, 0, +1}.
        assert!(ReflexData::new("", 1, vec![(3, 2)], 16).is_err());
        // Имя длиннее потолка.
        let long = "x".repeat(129);
        assert!(ReflexData::new(long, 1, vec![(1, 1)], 16).is_err());
        // След длиннее потолка.
        let many: Vec<(u32, i8)> = (0..4097).map(|i| (i % 16, 1)).collect();
        assert!(ReflexData::new("", 1, many, 16).is_err());
    }

    #[test]
    fn section_roundtrip_u16() {
        let d = sample();
        let bytes = d.encode_with_dpol(true, 16).unwrap();
        // Имя «Иван» — 8 байт UTF-8; записи 4 × 3 Б (u16 + i8).
        assert_eq!(bytes.len(), REFL_HEADER_SIZE + 8 + 4 * 3);
        let (s, used) = ReflexSection::decode(&bytes, 16, true).unwrap();
        assert_eq!(used, bytes.len());
        assert_eq!(s.interlocutor(), "Иван");
        assert_eq!(s.turns(), 12);
        assert_eq!(s.events(), &[(3u32, 1i8), (7, -1), (9, 1), (1, 0)]);
        assert_eq!(s.trail_tail(2), &[(9, 1), (1, 0)]);
        assert_eq!(s.trail_tail(10), s.events());
        assert!(s.index16());
    }

    #[test]
    fn section_roundtrip_u32_anonymous() {
        // d_pol > 65536 — координаты u32; имя пустое (аноним).
        let d = ReflexData::new("", 1_000_000, vec![(70_000, -1), (65_537, 1)], 1_000_000).unwrap();
        let bytes = d.encode_with_dpol(false, 1_000_000).unwrap();
        let (s, used) = ReflexSection::decode(&bytes, 1_000_000, false).unwrap();
        assert_eq!(used, bytes.len());
        assert_eq!(s.interlocutor(), "");
        assert_eq!(s.turns(), 1_000_000);
        assert_eq!(s.events(), &[(70_000u32, -1i8), (65_537, 1)]);
        assert!(!s.index16());
    }

    #[test]
    fn decode_rejects_corruption() {
        let bytes = sample().encode_with_dpol(true, 16).unwrap();
        // Короткий буфер.
        assert!(ReflexSection::decode(&bytes[..31], 16, true).is_err());
        // Плохой magic.
        let mut b = bytes.clone();
        b[0] = b'X';
        assert!(ReflexSection::decode(&b, 16, true).is_err());
        // Плохая версия.
        let mut b = bytes.clone();
        b[4] = 2;
        assert!(ReflexSection::decode(&b, 16, true).is_err());
        // Несовпадение d_pol.
        assert!(ReflexSection::decode(&bytes, 32, true).is_err());
        // Ломаем счётчик событий.
        let mut b = bytes.clone();
        b[12] = 99;
        assert!(ReflexSection::decode(&b, 16, true).is_err());
        // Ненулевое reserved.
        let mut b = bytes.clone();
        b[28] = 1;
        assert!(ReflexSection::decode(&b, 16, true).is_err());
        // Нулевой счётчик событий.
        let mut b = bytes.clone();
        b[12..16].copy_from_slice(&0u32.to_le_bytes());
        assert!(ReflexSection::decode(&b, 16, true).is_err());
        // Координата вне решётки (первая запись — за именем «Иван» 8 Б).
        let mut b = bytes.clone();
        b[REFL_HEADER_SIZE + 8..REFL_HEADER_SIZE + 10]
            .copy_from_slice(&9999u16.to_le_bytes());
        assert!(ReflexSection::decode(&b, 16, true).is_err());
        // Полярность 2.
        let mut b = bytes.clone();
        b[REFL_HEADER_SIZE + 8 + 2] = 2;
        assert!(ReflexSection::decode(&b, 16, true).is_err());
        // Битое UTF-8 имя.
        let d = ReflexData::new("ab", 1, vec![(1, 1)], 16).unwrap();
        let mut b = d.encode_with_dpol(true, 16).unwrap();
        b[REFL_HEADER_SIZE] = 0xFF;
        assert!(ReflexSection::decode(&b, 16, true).is_err());
    }
}
