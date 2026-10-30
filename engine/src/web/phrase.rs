//! Фразовый поиск: позиционный инвертированный индекс (Lucene .prx-подход).
//!
//! Проблема v0.10.0: веб-индекс — «мешок слов». Запрос «"Rust async
//! runtime"» находил страницу, где "Rust" в первом абзаце, а "runtime" —
//! в футере через 5000 слов: postings хранят только tf, порядок токенов
//! потерян.
//!
//! Решение (как в ядре poler-engine и в Google):
//! * в postings рядом с (term, page_id) хранятся ПОЗИЦИИ токенов —
//!   `positions BLOB` с delta-varint-кодированием (компактно: ~1–2 байта
//!   на вхождение вместо u32);
//! * фразовый запрос «"word1 word2"» проверяет смежность:
//!   `position(word2) == position(word1) + 1` (и далее по цепочке);
//! * фразовые термы стеммингуются тем же путём, что и индекс —
//!   «владение памятью» находит «владения памятью» (падежный шум
//!   не должен ломать цитаты).
//!
//! Кавычки-разделители фраз: ASCII `"..."` и типографские `«...»`
//! (укр/рос клавиатуры).

use super::stem::tokenize_stem;

/// Разобранный запрос: фразы (в кавычках) + свободные термы.
#[derive(Debug, Default, Clone)]
pub struct QueryParts {
    /// Фразы по 2+ токенов — требуют смежности позиций.
    pub phrases: Vec<Vec<String>>,
    /// Свободные термы (включая однотокенные «фразы» из кавычек).
    pub terms: Vec<String>,
}

impl QueryParts {
    /// Все термы запроса (фразовые + свободные), без повторов, порядок
    /// сохранён — для сниппетов и title-буста.
    pub fn all_terms(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let push = |t: &str, out: &mut Vec<String>| {
            if !out.iter().any(|x| x == t) {
                out.push(t.to_string());
            }
        };
        for t in &self.terms {
            push(t, &mut out);
        }
        for ph in &self.phrases {
            for t in ph {
                push(t, &mut out);
            }
        }
        out
    }
}

/// Разбор запроса: сегменты в кавычках → фразы, остальное → термы.
///
/// Однотокенная «фраза» (`"gzip"`) деградирует до свободного терма —
/// смежность одного слова не определена. Незакрытая кавычка трактуется
/// щедро: хвост до конца строки считается фразой.
pub fn parse_query(q: &str) -> QueryParts {
    let mut phrases: Vec<Vec<String>> = Vec::new();
    let mut free = String::new();
    let mut buf = String::new();
    // 0 — вне кавычек, 1 — внутри "...", 2 — внутри «...»
    let mut mode = 0u8;
    let flush_phrase = |buf: &mut String, phrases: &mut Vec<Vec<String>>, free: &mut String| {
        let toks = tokenize_stem(buf);
        if toks.len() >= 2 {
            phrases.push(toks);
        } else if toks.len() == 1 {
            free.push(' ');
            free.push_str(buf.trim());
        }
        buf.clear();
    };
    for ch in q.chars() {
        match mode {
            0 => match ch {
                '"' => {
                    mode = 1;
                    free.push(' ');
                }
                '«' => {
                    mode = 2;
                    free.push(' ');
                }
                _ => free.push(ch),
            },
            1 => match ch {
                '"' => {
                    mode = 0;
                    flush_phrase(&mut buf, &mut phrases, &mut free);
                }
                _ => buf.push(ch),
            },
            _ => match ch {
                '»' => {
                    mode = 0;
                    flush_phrase(&mut buf, &mut phrases, &mut free);
                }
                _ => buf.push(ch),
            },
        }
    }
    // незакрытая кавычка — щедрая трактовка: это тоже фраза
    if mode != 0 && !buf.trim().is_empty() {
        flush_phrase(&mut buf, &mut phrases, &mut free);
    }
    QueryParts {
        phrases,
        terms: tokenize_stem(&free),
    }
}

// ----------------------------------------------------------------------
// Позиционный кодек: delta + LEB128 varint (Lucene .prx в миниатюре)
// ----------------------------------------------------------------------

/// Кодирование отсортированных позиций: дельты + varint.
/// Пустой список → пустой blob. Защита от неостортированного входа:
/// копия сортируется и дедуплицируется.
pub fn encode_positions(positions: &[u32]) -> Vec<u8> {
    let mut sorted: Vec<u32> = positions.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let mut out = Vec::with_capacity(sorted.len() * 2);
    let mut prev: u32 = 0;
    for &p in &sorted {
        // дельта всегда ≥ 0 (позиции возрастают), первая = сама позиция
        let mut v = p.wrapping_sub(prev);
        prev = p;
        loop {
            let mut b = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                break;
            }
        }
    }
    out
}

/// Декодирование blob обратно в отсортированные позиции.
/// Битый blob (переполнение varint) обрезается до последней целой позиции.
pub fn decode_positions(blob: &[u8]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::with_capacity(blob.len());
    let mut acc: u32 = 0;
    let mut shift: u32 = 0;
    let mut prev: u32 = 0;
    for &b in blob {
        acc |= ((b & 0x7f) as u32) << shift;
        if b & 0x80 == 0 {
            prev = prev.wrapping_add(acc);
            out.push(prev);
            acc = 0;
            shift = 0;
        } else {
            shift += 7;
            if shift > 28 {
                break; // защита от битого blob
            }
        }
    }
    out
}

/// Сколько раз фраза встречается в документе ПО СМЕЖНОСТИ.
///
/// `pos_lists[i]` — отсортированные позиции i-го терма фразы; вхождение
/// в позиции p считается, если для каждого i список содержит `p + i`.
/// Классический алгоритм позиционной пересечки (Lucene PhraseScorer).
pub fn phrase_occurrences(pos_lists: &[&[u32]]) -> usize {
    if pos_lists.is_empty() || pos_lists.iter().any(|l| l.is_empty()) {
        return 0;
    }
    let mut count = 0usize;
    for &p in pos_lists[0] {
        let mut ok = true;
        for (off, list) in pos_lists.iter().enumerate().skip(1) {
            let want = p + off as u32;
            if list.binary_search(&want).is_err() {
                ok = false;
                break;
            }
        }
        if ok {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrip() {
        let cases: Vec<Vec<u32>> = vec![
            vec![],
            vec![0],
            vec![1, 2, 3],
            vec![5, 6, 7, 100, 101, 1000],
            vec![0, 127, 128, 16383, 16384, u32::MAX - 1],
            vec![42; 50], // повторы (после dedup — одна)
        ];
        for c in cases {
            let enc = encode_positions(&c);
            let dec = decode_positions(&enc);
            let mut expect = c.clone();
            expect.sort_unstable();
            expect.dedup();
            assert_eq!(dec, expect, "case {c:?}");
        }
    }

    #[test]
    fn varint_compact() {
        // плотные позиции ~1 байт на вхождение
        let dense: Vec<u32> = (0..1000).map(|i| i * 2).collect();
        let enc = encode_positions(&dense);
        assert!(enc.len() < 1100, "len={}", enc.len());
        assert_eq!(decode_positions(&enc), dense);
    }

    #[test]
    fn parse_basic_quotes() {
        let qp = parse_query("\"rust async\" runtime");
        assert_eq!(qp.phrases, vec![vec!["rust".to_string(), "async".to_string()]]);
        // v2.0: латиница стеммингуется Snowball («runtime» → «runtim»)
        assert_eq!(qp.terms, vec!["runtim".to_string()]);
    }

    #[test]
    fn parse_guillemets() {
        let qp = parse_query("«владение памятью» безопасность");
        assert_eq!(qp.phrases.len(), 1);
        assert_eq!(qp.phrases[0].len(), 2);
        assert_eq!(qp.terms, vec!["безопасност".to_string()]);
    }

    #[test]
    fn parse_single_word_phrase_degrades_to_term() {
        let qp = parse_query("\"gzip\" module");
        assert!(qp.phrases.is_empty());
        assert!(qp.terms.contains(&"gzip".to_string()));
        // v2.0: Snowball («module» → «modul»)
        assert!(qp.terms.contains(&"modul".to_string()));
    }

    #[test]
    fn parse_unterminated_quote_is_phrase() {
        let qp = parse_query("\"rust async");
        assert_eq!(qp.phrases, vec![vec!["rust".to_string(), "async".to_string()]]);
    }

    #[test]
    fn parse_phrase_terms_are_stemmed() {
        // «ініціація відбулася» — стемминг обязан примениться и к фразе
        let qp = parse_query("«ініціація відбулася»");
        assert_eq!(qp.phrases[0][0], "ініціац");
    }

    #[test]
    fn parse_no_phrases_plain_query() {
        let qp = parse_query("gzip static module");
        assert!(qp.phrases.is_empty());
        assert_eq!(qp.terms.len(), 3);
    }

    #[test]
    fn parse_multiple_phrases() {
        let qp = parse_query("\"memory safety\" и «сборщик мусора»");
        assert_eq!(qp.phrases.len(), 2);
    }

    #[test]
    fn occurrences_adjacent() {
        let a: Vec<u32> = vec![3, 10];
        let b: Vec<u32> = vec![4, 11, 30];
        assert_eq!(phrase_occurrences(&[&a, &b]), 2);
    }

    #[test]
    fn occurrences_scattered_zero() {
        let a: Vec<u32> = vec![3];
        let b: Vec<u32> = vec![50];
        assert_eq!(phrase_occurrences(&[&a, &b]), 0);
    }

    #[test]
    fn occurrences_three_terms_order_matters() {
        let x: Vec<u32> = vec![0, 5];
        let y: Vec<u32> = vec![1, 7];
        let z: Vec<u32> = vec![2];
        // x@5,y@7,z@2: цепочка 0,1,2 есть; 5,6,7 — нет
        assert_eq!(phrase_occurrences(&[&x, &y, &z]), 1);
    }

    #[test]
    fn occurrences_repeated_term() {
        // «buffalo buffalo» в «buffalo buffalo buffalo» → 2 вхождения
        let p: Vec<u32> = vec![0, 1, 2];
        assert_eq!(phrase_occurrences(&[&p, &p]), 2);
    }

    #[test]
    fn all_terms_dedup() {
        let qp = parse_query("\"rust async\" rust");
        let all = qp.all_terms();
        assert_eq!(all, vec!["rust".to_string(), "async".to_string()]);
    }
}
