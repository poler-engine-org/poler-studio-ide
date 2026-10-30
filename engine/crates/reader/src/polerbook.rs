//! .poler-book — формат «книга в кармане»: текст + паспорт интонаций.
//!
//! Одна книга = один файл: сырой текст НЕ хранится как PCM (это и есть
//! обход Шеннона для аудио): хранятся только
//!   текст (UTF-8) + фразовые сиды (u64) + архетипы + паузы.
//! Аудиокнига на 10 часов звучания весит меньше мегабайта, потому что
//! волна — чистая функция (текст, семя, личность).
//!
//! Разметка (little-endian):
//!   magic   : 11 байт "POLERBOOK1"
//!   version : u32 = 1
//!   n_text  : u32 — байт текста
//!   text    : n_text байт UTF-8 (сырые абзацы)
//!   n_sect  : u32 — количество записей паспорта
//!   записи  : { seed: u64, arch: u8, pause_ms: u16 } × n_sect

use crate::voice::Archetype;
use crate::{ReaderError, Result};
use std::path::Path;

pub const MAGIC: &[u8; 10] = b"POLERBOOK1";
pub const VERSION: u32 = 1;

/// Паспортная запись фразы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhrasePassport {
    /// 64-битное семя фразы (личность диктора + фразовая вариативность).
    pub seed: u64,
    /// Опорный архетип фразы.
    pub arch: Archetype,
    /// Пауза после фразы (мс).
    pub pause_ms: u16,
}

/// Книга в формате .poler-book.
#[derive(Debug, Clone, PartialEq)]
pub struct PolerBook {
    /// Текст (абзацы через \n\n).
    pub text: String,
    /// Паспорт: по записи на фразу.
    pub passports: Vec<PhrasePassport>,
}

fn arch_to_u8(a: Archetype) -> u8 {
    match a {
        Archetype::ACalm => 0,
        Archetype::ABright => 1,
        Archetype::IDark => 2,
        Archetype::UCalm => 3,
    }
}

fn arch_from_u8(v: u8) -> Option<Archetype> {
    match v {
        0 => Some(Archetype::ACalm),
        1 => Some(Archetype::ABright),
        2 => Some(Archetype::IDark),
        3 => Some(Archetype::UCalm),
        _ => None,
    }
}

impl PolerBook {
    /// Собрать книгу из текста и списка паспортов.
    pub fn new(text: String, passports: Vec<PhrasePassport>) -> Self {
        PolerBook { text, passports }
    }

    /// Сериализовать в байты.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(10 + 4 + 4 + self.text.len() + 4 + self.passports.len() * 13);
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&VERSION.to_le_bytes());
        b.extend_from_slice(&(self.text.len() as u32).to_le_bytes());
        b.extend_from_slice(self.text.as_bytes());
        b.extend_from_slice(&(self.passports.len() as u32).to_le_bytes());
        for p in &self.passports {
            b.extend_from_slice(&p.seed.to_le_bytes());
            b.push(arch_to_u8(p.arch));
            b.extend_from_slice(&p.pause_ms.to_le_bytes());
        }
        b
    }

    /// Десериализовать из байтов.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 10 + 4 + 4 || &data[0..10] != MAGIC {
            return Err(ReaderError::BadInput("не .poler-book (нет MAGIC)".into()));
        }
        let mut pos = 10;
        let version = u32::from_le_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
        ]);
        if version != VERSION {
            return Err(ReaderError::BadInput(format!(
                "версия {version} не поддерживается (ожидается {VERSION})"
            )));
        }
        pos += 4;
        let n_text = u32::from_le_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
        ]) as usize;
        pos += 4;
        if pos + n_text > data.len() {
            return Err(ReaderError::BadInput("обрезан текст книги".into()));
        }
        let text = String::from_utf8(data[pos..pos + n_text].to_vec())
            .map_err(|_| ReaderError::BadInput("текст не UTF-8".into()))?;
        pos += n_text;
        if pos + 4 > data.len() {
            return Err(ReaderError::BadInput("обрезан заголовок паспорта".into()));
        }
        let n = u32::from_le_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
        ]) as usize;
        pos += 4;
        let need = 8 + 1 + 2;
        if pos + n * need > data.len() {
            return Err(ReaderError::BadInput("обрезаны записи паспорта".into()));
        }
        let mut passports = Vec::with_capacity(n);
        for i in 0..n {
            let p = pos + i * need;
            let seed = u64::from_le_bytes([
                data[p],
                data[p + 1],
                data[p + 2],
                data[p + 3],
                data[p + 4],
                data[p + 5],
                data[p + 6],
                data[p + 7],
            ]);
            let arch = arch_from_u8(data[p + 8])
                .ok_or_else(|| ReaderError::BadInput(format!("код архетипа {}", data[p + 8])))?;
            let pause_ms = u16::from_le_bytes([data[p + 9], data[p + 10]]);
            passports.push(PhrasePassport { seed, arch, pause_ms });
        }
        Ok(PolerBook { text, passports })
    }

    /// Записать в файл.
    pub fn save(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.to_bytes()).map_err(ReaderError::Io)
    }

    /// Прочитать из файла.
    pub fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read(path).map_err(ReaderError::Io)?;
        Self::from_bytes(&data)
    }

    /// Упакованный размер (байт).
    pub fn size_bytes(&self) -> usize {
        10 + 4 + 4 + self.text.len() + 4 + self.passports.len() * 13
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PolerBook {
        PolerBook::new(
            "Привет мир.\n\nВторой абзац!".into(),
            vec![
                PhrasePassport { seed: 1, arch: Archetype::ACalm, pause_ms: 720 },
                PhrasePassport { seed: 2, arch: Archetype::IDark, pause_ms: 420 },
            ],
        )
    }

    #[test]
    fn roundtrip_bytes() {
        let b = sample();
        let bytes = b.to_bytes();
        let back = PolerBook::from_bytes(&bytes).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn roundtrip_file() {
        let dir = std::env::temp_dir().join("poler_reader_book");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("t.poler-book");
        let b = sample();
        b.save(&p).unwrap();
        let back = PolerBook::load(&p).unwrap();
        assert_eq!(back, b);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn bad_magic_rejected() {
        assert!(PolerBook::from_bytes(b"NOTAPOLERBOOK").is_err());
    }

    #[test]
    fn truncated_rejected() {
        let bytes = sample().to_bytes();
        for cut in [5usize, 15, 20] {
            if cut < bytes.len() {
                assert!(PolerBook::from_bytes(&bytes[..cut]).is_err());
            }
        }
    }

    #[test]
    fn size_is_compact() {
        // 31 символ текста + 2 записи ≈ 31 + 26 + 23 = ~80 байт
        let b = sample();
        assert!(b.size_bytes() < 120);
        assert!(b.size_bytes() >= b.text.len());
    }

    #[test]
    fn cyrillic_text_survives() {
        let b = PolerBook::new("Кириллиця ± ёщё раз…".into(), vec![]);
        let back = PolerBook::from_bytes(&b.to_bytes()).unwrap();
        assert_eq!(back.text, "Кириллиця ± ёщё раз…");
    }
}
