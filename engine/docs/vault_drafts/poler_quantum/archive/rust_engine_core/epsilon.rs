//! Локальная информационная плотность ε(W_t) и семантические бонусы.
//!
//! Формула (спецификация §3.А):
//!
//! ```text
//! ε(W_t) = κ · (1 + ln(1 + count(kw))) ·
//!          Σ_{w ∈ Unique(W_t) \ {kw}} (ln(N_total) − ln(freq(w)))² +
//!          Σ_{w ∈ W_t} Bonus_semantic(w)
//! ```
//!
//! * `N_total` — объём токенов корпуса (глобальный либо локальный);
//! * `freq(w)` — глобальная частота токена;
//! * `Bonus_semantic` — весовые коэффициенты критических маркеров
//!   (отрицания, обязательность, критичность, угроза, код-маркеры),
//!   которые наивный BM25/TF-IDF игнорирует, а векторный RAG усредняет;
//! * `κ` — масштабный калибровочный коэффициент.
//!
//! Семантические маркеры ищутся в окне автоматом Ахо-Корасик
//! (LeftmostLongest) — классическая задача мультитокен-поиска по
//! предвычисленному словарю фраз.

use aho_corasick::{AhoCorasick, MatchKind};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use crate::tokenizer::InvertedIndex;

/// Таблица маркеров: (фраза, вес). Отрицания — максимальный вес:
/// именно на них ломается косинусное сходство эмбеддингов.
const MARKER_TABLE: &[(&str, f64)] = &[
    // --- Отрицания (Negation Blindness fix) ---
    ("не должна", 2.0),
    ("не должен", 2.0),
    ("не должно", 2.0),
    ("не должны", 2.0),
    ("не может", 2.0),
    ("не может быть", 2.0),
    ("не допускается", 2.0),
    ("не следует", 1.8),
    ("запрещено", 2.0),
    ("запрещается", 2.0),
    ("никогда", 1.8),
    ("ни при каких", 1.8),
    ("never", 2.0),
    ("must not", 2.0),
    ("mustn't", 2.0),
    ("cannot", 2.0),
    ("can't", 1.8),
    ("do not", 1.6),
    ("don't", 1.6),
    // --- Обязанность / модальность ---
    ("обязана", 1.5),
    ("обязан", 1.5),
    ("обязательство", 1.5),
    ("обязательно", 1.4),
    ("должна", 1.3),
    ("должен", 1.3),
    ("должны", 1.3),
    ("требуется", 1.2),
    ("следует", 1.0),
    ("must", 1.5),
    ("shall", 1.5),
    ("required", 1.4),
    ("mandatory", 1.4),
    // --- Критичность ---
    ("критично", 1.5),
    ("критический", 1.4),
    ("критическая", 1.4),
    ("важно", 1.2),
    ("важный", 1.1),
    ("важная", 1.1),
    ("срочно", 1.3),
    ("critical", 1.5),
    ("urgent", 1.3),
    ("important", 1.2),
    // --- Угроза / риск / кризис ---
    ("угроза", 1.2),
    ("кризис", 1.2),
    ("риск", 1.1),
    ("опасность", 1.2),
    ("авария", 1.2),
    ("threat", 1.2),
    ("crisis", 1.2),
    ("risk", 1.1),
    ("danger", 1.2),
    ("failure", 1.1),
    // --- Код-маркеры ---
    ("unsafe", 1.0),
    ("todo", 1.0),
    ("fixme", 1.2),
    ("hack", 1.0),
    ("deprecated", 1.3),
    ("устарел", 1.2),
    ("устаревший", 1.2),
    ("panic!", 1.2),
    ("unwrap()", 0.8),
    // --- Метрики / время ---
    ("метрика", 0.8),
    ("metric", 0.8),
    ("дедлайн", 1.0),
    ("deadline", 1.0),
];

static MARKER_AC: LazyLock<(AhoCorasick, Vec<f64>)> = LazyLock::new(|| {
    let pats: Vec<&str> = MARKER_TABLE.iter().map(|(p, _)| *p).collect();
    let ac = AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostLongest)
        .build(pats)
        .expect("построение автомата маркеров не может провалиться");
    let weights = MARKER_TABLE.iter().map(|(_, w)| *w).collect();
    (ac, weights)
});

/// Считает Σ Bonus_semantic по окну текста (нижний регистр + Ахо-Корасик).
pub fn semantic_bonus(window_text: &str) -> f64 {
    if window_text.is_empty() {
        return 0.0;
    }
    let hay = window_text.to_lowercase();
    let (ac, weights) = &*MARKER_AC;
    let mut total = 0.0;
    for m in ac.find_iter(&hay) {
        total += weights[m.pattern().as_usize()];
    }
    total
}

/// Батч-вычисление ε для готового окна токенов (режим `hits`).
///
/// * `query_tokens` — токены запроса (kw); их вхождения дают множитель
///   `1 + ln(1 + count)`, сами они исключаются из unique-суммы;
/// * `bonus` — заранее посчитанный [`semantic_bonus`] по тексту окна.
pub fn calculate_epsilon(
    window_tokens: &[&str],
    query_tokens: &[String],
    global_counts: &HashMap<String, usize>,
    total_tokens: usize,
    kappa: f64,
    bonus: f64,
) -> f64 {
    if window_tokens.is_empty() || query_tokens.is_empty() {
        return 0.0;
    }
    let log_n = (total_tokens.max(1) as f64).ln();
    let query_set: HashSet<&str> = query_tokens.iter().map(|s| s.as_str()).collect();

    let mut kw_count = 0usize;
    let mut unique: HashSet<&str> = HashSet::new();
    for t in window_tokens {
        if query_set.contains(*t) {
            kw_count += 1;
        } else {
            unique.insert(*t);
        }
    }

    let mut sum = 0.0;
    for t in unique {
        let freq = *global_counts.get(t).unwrap_or(&1) as f64;
        let rarity = (log_n - freq.ln()).max(0.0);
        sum += rarity * rarity;
    }

    let intensity = 1.0 + (kw_count as f64 + 1.0).ln();
    kappa * intensity * sum + bonus
}

/// Инкрементальный калькулятор ε со скользящим окном (режим `field`).
///
/// Поддерживает окно вокруг позиции `center` радиусом `radius` и
/// амортизированно O(1) пересчитывает ε при сдвиге окна на один токен:
/// для каждого токена хранится счётчик вхождений, а сумма rarity²
/// по уникальным токенам обновляется при переходах 0→1 и 1→0.
/// Полный проход по документу — строго O(N).
///
/// Семантический бонус в field-режиме не добавляется: он определён
/// на уровне совпадений (hits), а не на каждой позиции (см. README).
pub struct SlidingEpsilon {
    radius: usize,
    kappa: f64,
    log_n: f64,
    rarity2: HashMap<String, f64>,
    query_set: HashSet<String>,
    window_counts: HashMap<String, usize>,
    unique_rarity_sum: f64,
    kw_count: usize,
    head: usize,
    tail: usize,
}

impl SlidingEpsilon {
    /// Готовит калькулятор: предвычисляет rarity² для словаря документа.
    pub fn new(
        index: &InvertedIndex,
        query_tokens: &[String],
        global_counts: &HashMap<String, usize>,
        total_tokens: usize,
        kappa: f64,
        radius: usize,
    ) -> Self {
        let log_n = (total_tokens.max(1) as f64).ln();
        let mut rarity2 = HashMap::with_capacity(index.token_counts.len());
        for tok in index.token_counts.keys() {
            let freq = *global_counts.get(tok).unwrap_or(&1) as f64;
            let r = (log_n - freq.ln()).max(0.0);
            rarity2.insert(tok.clone(), r * r);
        }
        Self {
            radius,
            kappa,
            log_n,
            rarity2,
            query_set: query_tokens.iter().cloned().collect(),
            window_counts: HashMap::new(),
            unique_rarity_sum: 0.0,
            kw_count: 0,
            head: 0,
            tail: 0,
        }
    }

    fn rarity2_of(&self, tok: &str) -> f64 {
        match self.rarity2.get(tok) {
            Some(v) => *v,
            None => {
                // токен вне словаря документа -> freq = 1 -> rarity = ln(N)
                let r = self.log_n;
                r * r
            }
        }
    }

    fn push(&mut self, tok: &str) {
        if self.query_set.contains(tok) {
            self.kw_count += 1;
            return;
        }
        let was_zero = self.window_counts.get(tok).copied().unwrap_or(0) == 0;
        if was_zero {
            let r2 = self.rarity2_of(tok);
            self.unique_rarity_sum += r2;
        }
        *self.window_counts.entry(tok.to_string()).or_insert(0) += 1;
    }

    fn pop(&mut self, tok: &str) {
        if self.query_set.contains(tok) {
            self.kw_count = self.kw_count.saturating_sub(1);
            return;
        }
        if let Some(e) = self.window_counts.get_mut(tok) {
            *e -= 1;
            if *e == 0 {
                self.window_counts.remove(tok);
                self.unique_rarity_sum -= self.rarity2_of(tok);
            }
        }
    }

    /// Сдвигает окно к новому центру и возвращает ε текущего окна.
    pub fn advance_to(&mut self, center: usize, index: &InvertedIndex) -> f64 {
        let total = index.total_tokens;
        let start = center.saturating_sub(self.radius);
        let end = (center + self.radius + 1).min(total);
        while self.head < end {
            let tok = index.tokens[self.head].clone();
            self.push(&tok);
            self.head += 1;
        }
        while self.tail < start {
            let tok = index.tokens[self.tail].clone();
            self.pop(&tok);
            self.tail += 1;
        }
        self.current()
    }

    /// ε текущего окна без сдвига.
    pub fn current(&self) -> f64 {
        let intensity = 1.0 + (self.kw_count as f64 + 1.0).ln();
        self.kappa * intensity * self.unique_rarity_sum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn counts(pairs: &[(&str, usize)]) -> HashMap<String, usize> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn empty_window_is_zero() {
        assert_eq!(calculate_epsilon(&[], &q(&["нокс"]), &HashMap::new(), 100, 1.0, 0.0), 0.0);
    }

    #[test]
    fn rare_words_raise_epsilon() {
        let idx_common = InvertedIndex::build("нокс и в и в и в и в и в");
        let idx_rare = InvertedIndex::build("нокс кварц базальт гранит слюда");
        let mut g1 = idx_common.token_counts.clone();
        g1.insert("нокс".into(), 1);
        let mut g2 = idx_rare.token_counts.clone();
        g2.insert("нокс".into(), 1);
        let e_common = calculate_epsilon(&idx_common.tokens.iter().map(|s| s.as_str()).collect::<Vec<_>>(), &q(&["нокс"]), &g1, 1000, 1.0, 0.0);
        let e_rare = calculate_epsilon(&idx_rare.tokens.iter().map(|s| s.as_str()).collect::<Vec<_>>(), &q(&["нокс"]), &g2, 1000, 1.0, 0.0);
        assert!(e_rare > e_common, "rare={e_rare} common={e_common}");
    }

    #[test]
    fn keyword_repetition_raises_intensity() {
        let g = counts(&[("нокс", 5), ("шунт", 5), ("когти", 5)]);
        let w1: Vec<&str> = vec!["нокс", "шунт", "когти"];
        let w2: Vec<&str> = vec!["нокс", "нокс", "нокс", "шунт", "когти"];
        let e1 = calculate_epsilon(&w1, &q(&["нокс"]), &g, 1000, 1.0, 0.0);
        let e2 = calculate_epsilon(&w2, &q(&["нокс"]), &g, 1000, 1.0, 0.0);
        assert!(e2 > e1);
    }

    #[test]
    fn kappa_scales_linearly() {
        let w: Vec<&str> = vec!["нокс", "шунт"];
        let g = counts(&[("нокс", 3), ("шунт", 7)]);
        let e1 = calculate_epsilon(&w, &q(&["нокс"]), &g, 1000, 1.0, 0.0);
        let e2 = calculate_epsilon(&w, &q(&["нокс"]), &g, 1000, 2.0, 0.0);
        assert!((e2 - 2.0 * e1).abs() < 1e-9);
    }

    #[test]
    fn bonus_is_added() {
        let w: Vec<&str> = vec!["нокс", "шунт"];
        let g = counts(&[("нокс", 3), ("шунт", 7)]);
        let base = calculate_epsilon(&w, &q(&["нокс"]), &g, 1000, 1.0, 0.0);
        let with = calculate_epsilon(&w, &q(&["нокс"]), &g, 1000, 1.0, 5.5);
        assert!((with - base - 5.5).abs() < 1e-9);
    }

    #[test]
    fn semantic_bonus_negation_outweighs_plain() {
        let neg = semantic_bonus("система не должна отключаться");
        let plain = semantic_bonus("система может отключаться");
        assert!(neg > plain, "neg={neg} plain={plain}");
        assert!(neg >= 2.0);
    }

    #[test]
    fn semantic_bonus_leftmost_longest() {
        // "must not" (2.0) должен побить "must" (1.5)
        let b = semantic_bonus("this must not happen");
        assert!((b - 2.0).abs() < 1e-9, "b={b}");
    }

    #[test]
    fn semantic_bonus_empty() {
        assert_eq!(semantic_bonus(""), 0.0);
        assert_eq!(semantic_bonus("обычные слова без маркеров"), 0.0);
    }

    #[test]
    fn sliding_matches_batch_exactly() {
        let text = "alpha beta gamma delta alpha epsilon zeta alpha beta theta iota \
                    kappa lambda alpha mu nu xi omicron pi";
        let index = InvertedIndex::build(text);
        let g = index.token_counts.clone();
        let total = index.total_tokens;
        let query = q(&["alpha"]);
        let radius = 3;

        let mut slider = SlidingEpsilon::new(&index, &query, &g, total, 1.0, radius);
        for center in 0..index.total_tokens {
            let sliding = slider.advance_to(center, &index);
            let start = center.saturating_sub(radius);
            let end = (center + radius + 1).min(total);
            let batch = calculate_epsilon(&index.tokens[start..end].iter().map(|s| s.as_str()).collect::<Vec<_>>(), &query, &g, total, 1.0, 0.0);
            assert!(
                (sliding - batch).abs() < 1e-6,
                "center={center}: sliding={sliding} batch={batch}"
            );
        }
    }

    #[test]
    fn sliding_o_n_does_not_degrade() {
        // инвариант: после прохода по всему документу head == total
        let text = "раз два три четыре пять раз два три нокс восемь девять нокс";
        let index = InvertedIndex::build(text);
        let g = index.token_counts.clone();
        let mut slider = SlidingEpsilon::new(&index, &q(&["нокс"]), &g, index.total_tokens, 1.0, 2);
        for c in 0..index.total_tokens {
            let _ = slider.advance_to(c, &index);
        }
        assert_eq!(slider.head, index.total_tokens);
    }
}
