//! SimHash: 64-битный отпечаток документа для поиска near-дубликатов.
//!
//! Украдено у Google: Manku, Jain, Das Sarma —
//! «Detecting Near-Duplicates for Web Crawling» (WWW 2007).
//! Именно так Googlebot отбрасывал зеркала и дубли выдачи.
//!
//! Идея: каждая фича (шингл из 4 слов) хешируется в 64 бита; биты
//! голосуют по принципу большинства. У похожих документов
//! распределения шинглов похожи → расстояние Хэмминга мало.
//! Порог Google: ≤ 3 бита из 64.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Размер шингла (слова в окне).
const SHINGLE: usize = 4;

/// 64-бит SimHash по токенам документа.
pub fn simhash(tokens: &[String]) -> u64 {
    if tokens.is_empty() {
        return 0;
    }
    let mut bits = [0i64; 64];
    if tokens.len() < SHINGLE {
        add_shingle(&tokens.join(" "), &mut bits);
    } else {
        for win in tokens.windows(SHINGLE) {
            add_shingle(&win.join(" "), &mut bits);
        }
    }
    let mut fp: u64 = 0;
    for (i, v) in bits.iter().enumerate() {
        if *v > 0 {
            fp |= 1u64 << i;
        }
    }
    fp
}

fn add_shingle(shingle: &str, bits: &mut [i64; 64]) {
    let mut h = DefaultHasher::new();
    shingle.hash(&mut h);
    let v = h.finish();
    for (i, b) in bits.iter_mut().enumerate() {
        if v & (1u64 << i) != 0 {
            *b += 1;
        } else {
            *b -= 1;
        }
    }
}

/// Расстояние Хэмминга между двумя отпечатками.
pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Near-дубликат: адаптивный порог Хэмминга.
///
/// Google использует жёсткое ≤ 3 бита из 64 — но на миллиардах ДОЛГИХ
/// документов (тысячи шинглов, голосование стабильно). На коротких
/// текстах шум голосования выше, поэтому порог масштабируется от длины:
/// 3 при ≥ 2000 токенов (режим Google), до 10 на коротких.
pub fn near_duplicate(a: u64, b: u64, n_tokens: usize) -> bool {
    hamming(a, b) <= hamming_threshold(n_tokens)
}

/// Адаптивный порог: 3 (длинные документы) … 10 (короткие).
pub fn hamming_threshold(n_tokens: usize) -> u32 {
    if n_tokens >= 2000 {
        return 3;
    }
    3 + (((2000 - n_tokens) * 7) / 2000) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn identical_documents() {
        let t = toks("the quick brown fox jumps over the lazy dog again and again");
        assert_eq!(hamming(simhash(&t), simhash(&t)), 0);
    }

    #[test]
    fn near_duplicates_detected() {
        // SimHash — вероятностная метрика: работает от десятков шинглов,
        // как в реальном применении Google (страницы, не фразы)
        let base = "rust programming language memory safety without garbage \
                    collector zero cost abstractions ownership and borrowing \
                    model lifetimes traits concurrency fearless systems \
                    embedded web assembly performance reliability tooling";
        let a = toks(base);
        let mut b = toks(base);
        b[8] = "collectors".to_string(); // одно слово из ~30
        assert!(near_duplicate(simhash(&a), simhash(&b), a.len()));
    }

    #[test]
    fn different_documents_far_apart() {
        let a = toks("rust memory safety ownership borrowing lifetimes compiler");
        let b = toks("cooking recipe pasta tomato sauce basil cheese oven minutes");
        assert!(!near_duplicate(simhash(&a), simhash(&b), a.len().max(b.len())));
    }

    #[test]
    fn short_input_ok() {
        let t = toks("hello world");
        assert_ne!(simhash(&t), 0);
    }

    #[test]
    fn empty_is_zero() {
        assert_eq!(simhash(&[]), 0);
    }

    #[test]
    fn mirror_pages() {
        // типичный дубль: тот же контент + короткая шаблонная шапка/подвал
        let content = "main article content of the page with many unique words \
                       about search engines indexing ranking crawling strategies \
                       and distributed systems architectures used by modern web \
                       crawlers to discover and store documents at scale";
        let a = toks(content);
        let b = toks(&format!("home menu {content} footer contact"));
        assert!(near_duplicate(simhash(&a), simhash(&b), a.len()));
    }

    #[test]
    fn adaptive_threshold() {
        assert_eq!(hamming_threshold(5000), 3);
        assert_eq!(hamming_threshold(2000), 3);
        assert!(hamming_threshold(30) > 5); // короткие — мягче
        assert!(hamming_threshold(30) <= 10); // но не безумно
    }
}
