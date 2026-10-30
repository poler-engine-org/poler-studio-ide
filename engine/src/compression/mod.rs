//! Плотность памяти индекса (v2.0, Приоритет 3 — PLAN_POLER_V2).
//!
//! Три кирпича:
//!
//! * [`fsst`] — словарь корпуса: FSST-сжатые термы + interning
//!   ([`fsst::VocabArena`]); пер-файловые словари watcher-состояния
//!   ссылаются на ID термов (8 байт на запись);
//! * [`lz4_hot`] — парковка постингов ([`lz4_hot::PostingsStore`]):
//!   сжатие при укладке в долгоживущее состояние, разжатие на горячем
//!   пути (lz4_flex, ~ГБ/с);
//! * [`zstd_dict`] — doc store веб-краулера ([`zstd_dict::DocStoreCodec`]):
//!   `pages.text` → zstd BLOB со обученным словарём.
//!
//! ## Цель (критерий успеха приоритета 3)
//!
//! Инвертированный индекс в RAM — в 5–10× меньше. Архитектурно:
//!
//! ```text
//! было:  global  HashMap<String, usize>   ~64–96 Б/терм
//!        файл    HashMap<String, usize>   ~64–96 Б/(терм,файл)
//! стало: global  VocabArena               ~14 Б/терм (FSST blob+индекс)
//!        файл    Vec<(term_id, count)>      8 Б/(терм,файл)
//!        постинги PostingsStore (lz4)      ~2–5× меньше сырья
//! ```
//!
//! ## Унифицированный доступ к частотам
//!
//! Резонансные формулы (ε, IIR, POLER[Ψ]) исторически принимали
//! `&HashMap<String, usize>`. Трейт [`TermFreqs`] делает источник частот
//! взаимозаменяемым: глобальная статистика теперь живёт в сжатой
//! [`GlobalStats`], локальная (по одному файлу) — как раньше в HashMap.
//! Переключение — enum [`StatsRef`] без dyn-диспетчеризации на горячем
//! пути (мономорфизация).

pub mod fsst;
pub mod lz4_hot;
pub mod zstd_dict;

pub use fsst::{FsstTable, VocabArena};
pub use lz4_hot::PostingsStore;
pub use zstd_dict::DocStoreCodec;

use std::collections::HashMap;

/// Источник глобальных частот термов для резонансных формул.
pub trait TermFreqs {
    /// Частота терма в корпусе (None — терм вне словаря).
    fn freq(&self, term: &str) -> Option<u32>;
}

impl TermFreqs for HashMap<String, usize> {
    fn freq(&self, term: &str) -> Option<u32> {
        self.get(term).map(|&v| v as u32)
    }
}

impl TermFreqs for HashMap<Box<str>, u32> {
    fn freq(&self, term: &str) -> Option<u32> {
        self.get(term).copied()
    }
}

impl<S: TermFreqs> TermFreqs for &S {
    fn freq(&self, term: &str) -> Option<u32> {
        (*self).freq(term)
    }
}

/// Глобальная статистика корпуса: сжатый словарь + счётчики по ID.
///
/// `counts` индексируется ID из [`VocabArena`]; частоты — `u32`
/// (корпус > 4 млрд токенов не поддерживается сознательно: цель
/// poler — рабочая станция, а не дата-центр).
pub struct GlobalStats<'a> {
    pub vocab: &'a VocabArena,
    pub counts: &'a [u32],
}

impl TermFreqs for GlobalStats<'_> {
    fn freq(&self, term: &str) -> Option<u32> {
        self.vocab.id_of(term).map(|id| {
            self.counts.get(id as usize).copied().unwrap_or(0)
        })
    }
}

/// Выбор источника статистики в проходе 2: глобальная (корпус) или
/// локальная (файл; режим `--local-stats`).
pub enum StatsRef<'a> {
    Global(GlobalStats<'a>),
    Local(&'a HashMap<Box<str>, u32>),
}

impl TermFreqs for StatsRef<'_> {
    fn freq(&self, term: &str) -> Option<u32> {
        match self {
            StatsRef::Global(g) => g.freq(term),
            StatsRef::Local(m) => m.freq(term),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn termfreqs_hashmap_variants() {
        let mut m1: HashMap<String, usize> = HashMap::new();
        m1.insert("нокс".into(), 3);
        assert_eq!(m1.freq("нокс"), Some(3));
        assert_eq!(m1.freq("нет"), None);

        let mut m2: HashMap<Box<str>, u32> = HashMap::new();
        m2.insert("нокс".into(), 5);
        assert_eq!(m2.freq("нокс"), Some(5));
        assert_eq!(m2.freq("нет"), None);
    }

    #[test]
    fn global_stats_reads_arena() {
        let mut arena = VocabArena::new();
        let a = arena.intern("нокс");
        let b = arena.intern("когти");
        arena.ensure_compact();
        let counts = vec![0u32; arena.len()];
        let mut counts = counts;
        counts[a as usize] = 7;
        counts[b as usize] = 2;
        let g = GlobalStats {
            vocab: &arena,
            counts: &counts,
        };
        assert_eq!(g.freq("нокс"), Some(7));
        assert_eq!(g.freq("когти"), Some(2));
        assert_eq!(g.freq("прочее"), None);
    }
}
