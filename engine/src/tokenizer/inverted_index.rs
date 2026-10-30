//! Инвертированный индекс в оперативной памяти + Unicode-токенизатор.
//!
//! В отличие от классических BM25-пайплайнов, индексируются **все** токены
//! длиной >= 1, включая стоп-слова и отрицания: запросы вида «не должна»
//! обязаны находиться (устранение Negation Blindness у векторного RAG).
//! Фильтрация стоп-слов происходит только на этапе оценки ε, где высокая
//! глобальная частота естественным образом обнуляет их rarity-вклад.

use std::collections::HashMap;

/// Инвертированный индекс одного документа.
#[derive(Debug, Clone, Default)]
pub struct InvertedIndex {
    /// Токены в порядке следования (lowercase).
    pub tokens: Vec<String>,
    /// Байтовое смещение начала каждого токена в исходном тексте.
    pub positions: Vec<usize>,
    /// Токен -> список индексов в `tokens` (posting list).
    pub index_map: HashMap<String, Vec<usize>>,
    /// Глобальные (в пределах документа) частоты токенов.
    pub token_counts: HashMap<String, usize>,
    /// Число токенов.
    pub total_tokens: usize,
}

impl InvertedIndex {
    /// Строит индекс за один линейный проход O(N) по тексту.
    ///
    /// Токеном считается максимальная последовательность букв/цифр/`_`
    /// (Unicode `is_alphanumeric`, кириллица включена), приведённая к нижнему
    /// регистру. Пунктуация и пробелы — разделители.
    pub fn build(text: &str) -> Self {
        let mut idx = Self {
            tokens: Vec::with_capacity(text.len() / 6 + 8),
            positions: Vec::with_capacity(text.len() / 6 + 8),
            index_map: HashMap::new(),
            token_counts: HashMap::new(),
            total_tokens: 0,
        };

        let mut current = String::with_capacity(16);
        let mut start = 0usize;
        let mut in_word = false;

        for (byte_idx, ch) in text.char_indices() {
            if ch.is_alphanumeric() || ch == '_' {
                if !in_word {
                    in_word = true;
                    start = byte_idx;
                    current.clear();
                }
                for lc in ch.to_lowercase() {
                    current.push(lc);
                }
            } else if in_word {
                in_word = false;
                idx.push_token(&current, start);
            }
        }
        if in_word {
            idx.push_token(&current, start);
        }

        idx.total_tokens = idx.tokens.len();
        idx
    }

    fn push_token(&mut self, tok: &str, byte_pos: usize) {
        let ti = self.tokens.len();
        *self.token_counts.entry(tok.to_string()).or_insert(0) += 1;
        self.index_map.entry(tok.to_string()).or_default().push(ti);
        self.tokens.push(tok.to_string());
        self.positions.push(byte_pos);
    }

    /// Находит вхождения фразы (последовательности токенов) и возвращает
    /// индексы токена, с которого начинается фраза.
    ///
    /// Однотокенный запрос — O(1) lookup по `index_map`.
    /// Многотокенный — проверка posting list первого токена.
    pub fn find_phrase(&self, phrase: &[String]) -> Vec<usize> {
        if phrase.is_empty() {
            return Vec::new();
        }
        if phrase.len() == 1 {
            return self.index_map.get(&phrase[0]).cloned().unwrap_or_default();
        }
        let mut out = Vec::new();
        if let Some(starts) = self.index_map.get(&phrase[0]) {
            'outer: for &s in starts {
                if s + phrase.len() > self.tokens.len() {
                    continue;
                }
                for (off, pt) in phrase.iter().enumerate().skip(1) {
                    if &self.tokens[s + off] != pt {
                        continue 'outer;
                    }
                }
                out.push(s);
            }
        }
        out
    }

    /// Байтовый диапазон текста, покрывающий окно токенов `[start, end)`.
    pub fn window_byte_range(&self, start: usize, end: usize) -> (usize, usize) {
        if start >= end || end == 0 || start >= self.tokens.len() {
            return (0, 0);
        }
        let end = end.min(self.tokens.len());
        let begin = self.positions[start];
        let last_tok = &self.tokens[end - 1];
        let finish = self.positions[end - 1] + last_tok.len();
        (begin, finish)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn basic_tokenize_and_positions() {
        let idx = InvertedIndex::build("Нокс вонзила Когти в сплетение");
        assert_eq!(idx.tokens, vec!["нокс", "вонзила", "когти", "в", "сплетение"]);
        assert_eq!(idx.total_tokens, 5);
        // позиции указывают на начало слов в байтах (кириллица = 2 байта/символ)
        assert_eq!(&idx.tokens[0], "нокс");
        assert_eq!(idx.positions[0], 0);
        assert!(idx.positions[3] > 0);
    }

    #[test]
    fn single_token_lookup() {
        let idx = InvertedIndex::build("нокс и нокс и снова нокс");
        let hits = idx.find_phrase(&q(&["нокс"]));
        assert_eq!(hits, vec![0, 2, 5]);
    }

    #[test]
    fn negation_phrase_is_findable() {
        // Ключевой тест устранения Negation Blindness
        let idx = InvertedIndex::build("Система не должна отключаться. Система обязана работать.");
        let hits = idx.find_phrase(&q(&["не", "должна"]));
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn multiword_phrase_order_matters() {
        let idx = InvertedIndex::build("когти вонзила и вонзила когти");
        let hits = idx.find_phrase(&q(&["вонзила", "когти"]));
        assert_eq!(hits, vec![3]);
    }

    #[test]
    fn short_tokens_are_indexed() {
        let idx = InvertedIndex::build("а б в_г деф");
        // "в_г" — один токен (внутреннее подчёркивание), "а","б" — токены длины 1
        assert_eq!(idx.tokens, vec!["а", "б", "в_г", "деф"]);
        assert!(!idx.find_phrase(&q(&["а"])).is_empty());
    }

    #[test]
    fn empty_text() {
        let idx = InvertedIndex::build("");
        assert_eq!(idx.total_tokens, 0);
        assert!(idx.find_phrase(&q(&["нокс"])).is_empty());
    }

    #[test]
    fn window_byte_range_is_correct() {
        let idx = InvertedIndex::build("alpha beta gamma delta");
        let (b, e) = idx.window_byte_range(1, 3); // beta gamma
        assert_eq!(&"alpha beta gamma delta"[b..e], "beta gamma");
    }

    #[test]
    fn token_counts() {
        let idx = InvertedIndex::build("нокс нокс шунт");
        assert_eq!(idx.token_counts.get("нокс"), Some(&2));
        assert_eq!(idx.token_counts.get("шунт"), Some(&1));
    }
}
