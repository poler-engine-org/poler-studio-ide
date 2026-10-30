//! Контекст-рефлекс `W` — автобиографическая память диалога (RQ23).
//!
//! ## Физика
//!
//! Русла `J` — структурная память: что с чем связано. Контекст-рефлекс —
//! **динамическая** память: что только что происходило. Кольцо гироскопа
//! (`VecDeque<(координата, полярность)>`) — окно направленного контекста,
//! в котором события потока порождают пары русел. При рестарте процесса
//! кольцо пусто: мозг знает структуру, но первый вопрос новой сессии
//! спаривается только с самим собой — нить разговора рвётся.
//!
//! Контекст-рефлекс хранит хвост сенсорного потока диалога (последние
//! события `REFLEX_TRAIL_CAP`), имя собеседника и счётчик реплик. При
//! resume хвост восстанавливает кольцо гироскопа ([`crate::gyro_lattice`]
//! `restore_ring`): первый вопрос новой сессии спаривается с последними
//! словами предыдущей — русла растут **сквозь** границу сессий, а волна
//! речи (`L5Generator`) получает затравку кольца из следа: ответ
//! начинается там, где разговор остановился.
//!
//! Важно: `W` — наблюдатель, а не участник. Обновление следа не трогает
//! ни фазы, ни русла (это делает `ingest`); след просто запоминает
//! порядок событий потока, чтобы гироскоп мог его пережить.

use pqw::checksum::fnv1a64;
use pqw::reflex::{ReflexData, ReflexSection};
use pqw::stream::tokenize;

/// Потолок следа: последние 256 событий потока диалога (≈ последняя
/// страница разговора). Рабочая память, не корпус.
pub const REFLEX_TRAIL_CAP: usize = 256;
/// Потолок имени собеседника (байт UTF-8).
pub const REFLEX_NAME_CAP: usize = 128;

/// Динамический след диалога: хвост сенсорного потока + собеседник.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContextReflex {
    /// Имя собеседника (пустая строка — аноним).
    interlocutor: String,
    /// Число реплик «вопрос → ответ» за всю историю диалога.
    turns: u64,
    /// События потока в хронологическом порядке (старые → новые).
    /// Vec с потолком [`REFLEX_TRAIL_CAP`]: выталкивание старейшего —
    /// редкая O(cap)-операция, зато честные срезы без склейки.
    trail: Vec<(u32, i8)>,
}

impl ContextReflex {
    /// Пустой рефлекс: нет ни следа, ни собеседника.
    pub fn new() -> ContextReflex {
        ContextReflex::default()
    }

    /// Имя собеседника (пустая строка — аноним).
    pub fn interlocutor(&self) -> &str {
        &self.interlocutor
    }

    /// Число реплик диалога.
    pub fn turns(&self) -> u64 {
        self.turns
    }

    /// Хвост сенсорного потока в хронологическом порядке.
    pub fn trail(&self) -> &[(u32, i8)] {
        &self.trail
    }

    /// Рефлекс пуст (нет ни следа, ни имени)? Пустой рефлекс не
    /// записывается — контейнер остаётся v4.
    pub fn is_empty(&self) -> bool {
        self.trail.is_empty() && self.interlocutor.is_empty()
    }

    /// Запомнить имя собеседника (обрезка до [`REFLEX_NAME_CAP`] байт
    /// по границе символов UTF-8; пустое имя = аноним).
    pub fn set_interlocutor(&mut self, name: &str) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }
        let mut cut = trimmed.len().min(REFLEX_NAME_CAP);
        while cut > 0 && !trimmed.is_char_boundary(cut) {
            cut -= 1;
        }
        self.interlocutor = trimmed[..cut].to_string();
    }

    /// Событие потока в след: `(координата, полярность)` уже вычислены.
    pub fn observe_event(&mut self, coord: u32, sign: i8) {
        self.trail.push((coord, sign.signum()));
        if self.trail.len() > REFLEX_TRAIL_CAP {
            // Выталкиваем ровно старейшее событие (потолок + 1).
            self.trail.drain(..self.trail.len() - REFLEX_TRAIL_CAP);
        }
    }

    /// Токен в след — та же конвенция сенсорного кодировщика, что у
    /// [`pqw::stream`] и гироскопа: координата `fnv1a64 mod d_pol`,
    /// полярность — старший бит хеша.
    pub fn observe_token(&mut self, token: &str, d_pol: u32) {
        if d_pol == 0 {
            return;
        }
        let h = fnv1a64(token.as_bytes());
        let coord = (h % d_pol as u64) as u32;
        let sign: i8 = if (h >> 63) & 1 == 1 { 1 } else { -1 };
        self.observe_event(coord, sign);
    }

    /// Текст в след (токены в порядке появления).
    pub fn observe_text(&mut self, text: &str, d_pol: u32) {
        for token in tokenize(text) {
            self.observe_token(token, d_pol);
        }
    }

    /// Одна реплика диалога: вопрос собеседника + ответ мозга
    /// (координаты эмиссий с Born-полярностями). Счётчик реплик растёт.
    pub fn observe_turn(&mut self, question: &str, answer: &[(u32, i8)], d_pol: u32) {
        self.observe_text(question, d_pol);
        for &(coord, sign) in answer {
            self.observe_event(coord, sign);
        }
        self.turns += 1;
    }

    /// Последние `n` событий следа (затравка кольца гироскопа/речи).
    pub fn trail_tail(&self, n: usize) -> &[(u32, i8)] {
        let take = n.min(self.trail.len());
        &self.trail[self.trail.len() - take..]
    }

    /// Последние слова нити через лексикон (для баннера автобиографии):
    /// хвост следа → токены, координаты без слова пропускаются.
    pub fn thread_words<'a>(
        &self,
        n: usize,
        token_of: impl Fn(u32) -> Option<&'a str>,
    ) -> Vec<&'a str> {
        self.trail_tail(n)
            .iter()
            .filter_map(|&(c, _)| token_of(c))
            .collect()
    }

    /// Данные для записи в контейнер v5: `None`, если след пуст
    /// (имя без следа не переживает рестарт — нечего восстанавливать).
    pub fn to_data(&self, d_pol: u32) -> Option<ReflexData> {
        if self.trail.is_empty() {
            return None;
        }
        ReflexData::new(
            self.interlocutor.clone(),
            self.turns,
            self.trail.to_vec(),
            d_pol,
        )
        .ok()
    }

    /// Восстановление из секции контейнера v5: след, имя, реплики.
    pub fn absorb(&mut self, section: &ReflexSection) {
        self.interlocutor = section.interlocutor().to_string();
        self.turns = section.turns();
        self.trail.clear();
        self.trail.extend(section.events().iter().copied());
        // Потолок следа — часть контракта рабочей памяти.
        if self.trail.len() > REFLEX_TRAIL_CAP {
            let cut = self.trail.len() - REFLEX_TRAIL_CAP;
            self.trail.drain(..cut);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_reflex_is_not_serializable() {
        let r = ContextReflex::new();
        assert!(r.is_empty());
        assert_eq!(r.to_data(64), None);
        // Имя без следа: личность есть, но восстанавливать нечего —
        // сериализация невозможна (след пуст).
        let mut r = ContextReflex::new();
        r.set_interlocutor("Иван");
        assert!(!r.is_empty());
        assert_eq!(r.to_data(64), None);
    }

    #[test]
    fn turn_accumulates_trail_and_counter() {
        let mut r = ContextReflex::new();
        r.set_interlocutor("Мария");
        r.observe_turn("квант фаза", &[(7, 1), (9, -1)], 128);
        assert_eq!(r.turns(), 1);
        // Вопрос дал 2 события, ответ — 2: хвост в хронологическом порядке.
        assert_eq!(r.trail().len(), 4);
        assert_eq!(&r.trail()[2..], &[(7, 1), (9, -1)]);
        r.observe_turn("решётка", &[(11, 1)], 128);
        assert_eq!(r.turns(), 2);
        assert_eq!(r.trail().len(), 6);
        assert_eq!(r.interlocutor(), "Мария");
        assert!(!r.is_empty());
    }

    #[test]
    fn trail_is_capped_at_256_events() {
        let mut r = ContextReflex::new();
        for k in 0..1000u32 {
            r.observe_event(k % 128, 1);
        }
        assert_eq!(r.trail().len(), REFLEX_TRAIL_CAP);
        // Хвост — последние события.
        assert_eq!(r.trail().last(), Some(&(999 % 128, 1)));
        assert_eq!(r.trail_tail(3).len(), 3);
    }

    #[test]
    fn token_convention_matches_gyro_encoder() {
        // Та же координата и полярность, что у TritGyro::observe_text.
        let mut r = ContextReflex::new();
        r.observe_token("квант", 64);
        let h = fnv1a64("квант".as_bytes());
        let coord = (h % 64) as u32;
        let sign: i8 = if (h >> 63) & 1 == 1 { 1 } else { -1 };
        assert_eq!(r.trail(), &[(coord, sign)]);
    }

    #[test]
    fn roundtrip_through_container_section() {
        let mut r = ContextReflex::new();
        r.set_interlocutor("Иван");
        r.observe_turn("квант фаза решётка", &[(3, 1), (5, -1)], 128);
        let data = r.to_data(128).unwrap();
        let bytes = data.encode_with_dpol(true, 128).unwrap();
        let (section, _) = ReflexSection::decode(&bytes, 128, true).unwrap();
        let mut back = ContextReflex::new();
        back.absorb(&section);
        assert_eq!(back.interlocutor(), "Иван");
        assert_eq!(back.turns(), 1);
        assert_eq!(back.trail(), r.trail());
        // Повторная сериализация — те же данные.
        assert_eq!(back.to_data(128).unwrap(), data);
    }

    #[test]
    fn thread_words_decode_through_lexicon() {
        let mut r = ContextReflex::new();
        r.observe_event(10, 1); // «фаза»
        r.observe_event(999, 1); // без слова
        r.observe_event(20, -1); // «трит»
        let words = r.thread_words(3, |c| match c {
            10 => Some("фаза"),
            20 => Some("трит"),
            _ => None,
        });
        assert_eq!(words, vec!["фаза", "трит"]);
    }

    #[test]
    fn name_trimming_respects_utf8_boundary() {
        let mut r = ContextReflex::new();
        r.set_interlocutor(&"И".repeat(200)); // 400 байт UTF-8
        assert!(r.interlocutor().len() <= REFLEX_NAME_CAP);
        assert!(r.interlocutor().chars().all(|c| c == 'И'));
        // Пустой/пробельный ввод не стирает известного собеседника.
        let saved = r.interlocutor().to_string();
        r.set_interlocutor("   ");
        assert_eq!(r.interlocutor(), saved);
    }
}
