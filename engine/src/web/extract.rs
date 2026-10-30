//! Извлечение и чистка контента — Readability-lite (Mozilla, Apache-2.0).
//!
//! Полный Readability строит DOM-дерево и считает плотность текста по
//! блокам; у нас DOM уже отрендерен Chromium (innerText), поэтому
//! остаётся: чистка шаблонного мусора, язык, сниппет с окном вокруг
//! совпадения и ОДИН токенизатор для документов и запросов
//! (урок бага «слайд-шоу»: разные токенизаторы → ноль хитов).

/// Чистка рендер-текста: пустышки, хвостовые пробелы, 3+ переводов строки.
pub fn clean_text(raw: &str, max_bytes: usize) -> String {
    let mut out = String::with_capacity(raw.len().min(max_bytes));
    let mut blank = 0;
    'lines: for line in raw.lines() {
        let l = line.trim_end();
        if l.trim().is_empty() {
            blank += 1;
            if blank <= 1 {
                out.push('\n');
            }
        } else {
            blank = 0;
            if out.len() + l.len() >= max_bytes {
                // обрезаем строку по границе символа и останавливаемся
                let room = max_bytes.saturating_sub(out.len());
                let room = floor_char_boundary(l, room);
                out.push_str(&l[..room]);
                break 'lines;
            }
            out.push_str(l);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

/// Максимальный индекс ≤ `idx`, попадающий на границу UTF-8 символа.
fn floor_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = idx.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Определение языка страницы: whatlang (байт-триграммы, 75 языков)
/// с порогом уверенности, fallback — алфавитная эвристика кириллица/латиница.
///
/// v2.0 Foundation (2.2): было «доля кириллицы > 20% → ru, иначе en» —
/// украинские страницы терялись в «ru», многоязычные размазывались.
/// whatlang отличает ukr/rus/bel и ещё ~70 языков; уверенность ниже
/// порога или короткий текст — старая эвристика. Поле `lang` — только
/// метаданные для выдачи (принцип «инструмент не понимает контент»).
pub fn detect_lang(text: &str) -> &'static str {
    let sample: String = text.chars().take(4000).collect();
    // whatlang: определение по всему тексту; короткий/цифровой — None
    if sample.chars().count() >= 16 {
        if let Some(info) = whatlang::detect(&sample) {
            // уверенность whatlang [0..1]: ниже порога не доверяем
            if info.confidence() > 0.6 {
                return lang_code(info.lang());
            }
        }
    }
    alphabet_fallback(&sample)
}

/// Двухбуквенный код для выдачи (совместимость со старым «ru»/«en»).
fn lang_code(lang: whatlang::Lang) -> &'static str {
    match lang {
        whatlang::Lang::Ukr => "uk",
        whatlang::Lang::Rus => "ru",
        whatlang::Lang::Eng => "en",
        whatlang::Lang::Deu => "de",
        whatlang::Lang::Fra => "fr",
        whatlang::Lang::Spa => "es",
        whatlang::Lang::Ita => "it",
        whatlang::Lang::Pol => "pl",
        whatlang::Lang::Bel => "be",
        whatlang::Lang::Cmn => "zh",
        whatlang::Lang::Jpn => "ja",
        whatlang::Lang::Kor => "ko",
        whatlang::Lang::Tur => "tr",
        whatlang::Lang::Por => "pt",
        whatlang::Lang::Nld => "nl",
        whatlang::Lang::Swe => "sv",
        whatlang::Lang::Uzb => "uz",
        _ => "xx",
    }
}

/// Старая алфавитная эвристика (fallback при низкой уверенности whatlang).
fn alphabet_fallback(sample: &str) -> &'static str {
    let mut cyr = 0usize;
    let mut lat = 0usize;
    for c in sample.chars() {
        if ('\u{0400}'..='\u{04FF}').contains(&c) {
            cyr += 1;
        } else if c.is_ascii_alphabetic() {
            lat += 1;
        }
    }
    if cyr + lat == 0 {
        return "";
    }
    if cyr as f64 / (cyr + lat) as f64 > 0.2 {
        "ru"
    } else {
        "en"
    }
}

/// ОДИН токенизатор для индексации документов И запросов:
/// lowercase, ё→е (нормализация, найденный ранее баг), юникодные слова.
pub fn web_tokenize(s: &str) -> Vec<String> {
    let norm: String = s
        .chars()
        .map(|c| match c {
            'ё' | 'Ё' => 'е',
            other => other.to_lowercase().next().unwrap_or(other),
        })
        .collect::<String>()
        .to_lowercase();
    // v2.0 Foundation (2.4): UAX#29 word boundaries вместо ручного
    // накопления alphanumeric-символов. Для кириллицы/латиницы поведение
    // идентично; выигрывают апострофные («don't» — одно слово), CJK
    // (идеограммы сегментируются по словарю), деванагари и пр.
    use unicode_segmentation::UnicodeSegmentation;
    let mut out = Vec::new();
    for word in norm.unicode_words() {
        push_token(&mut out, word);
    }
    out
}

fn push_token(out: &mut Vec<String>, t: &str) {
    // длины 2..=32: одиночные буквы/мусор не индексируем
    if t.chars().count() >= 2 && t.chars().count() <= 32 {
        out.push(t.to_string());
    }
}

/// Сниппет: окно до `max_chars` вокруг первого вхождения терма запроса.
pub fn snippet_for(text: &str, query_terms: &[String], max_chars: usize) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        return String::new();
    }
    let lower = flat.to_lowercase().replace('ё', "е");
    // позиция первого совпадения любого терма
    let mut best: Option<usize> = None;
    for t in query_terms {
        if let Some(i) = lower.find(t.as_str()) {
            best = Some(best.map_or(i, |b: usize| b.min(i)));
        }
    }
    let pos = best.unwrap_or(0);
    // границы слов + жёсткий cap по СИМВОЛАМ
    let chars: Vec<char> = flat.chars().collect();
    let pos_c = flat[..pos.min(flat.len())].chars().count();
    let half = max_chars / 2;
    let start = pos_c.saturating_sub(half);
    let start = to_word_start(&chars, start);
    let end = (pos_c + half).min(chars.len());
    let end = to_word_end(&chars, end);
    let end = end.min(start + max_chars); // cap
    let mut snip: String = chars[start..end].iter().collect();
    if start > 0 {
        snip.insert(0, '…');
    }
    if end < chars.len() {
        snip.push('…');
    }
    snip.replace('\n', " ")
}

fn to_word_start(chars: &[char], mut i: usize) -> usize {
    while i > 0 && chars[i - 1] != ' ' {
        i -= 1;
    }
    i
}

fn to_word_end(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && chars[i] != ' ' {
        i += 1;
    }
    i
}

/// Заголовок-фолбэк: первая содержательная строка текста.
pub fn title_from_text(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| l.len() >= 3)
        .unwrap_or("")
        .chars()
        .take(120)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_consistency() {
        // запрос и документ обязаны давать одинаковые термы
        assert_eq!(
            web_tokenize("Ёлка-палка"),
            web_tokenize("ёлка палка")
        );
        assert_eq!(
            web_tokenize("Слайд-шоу"),
            vec!["слайд".to_string(), "шоу".to_string()]
        );
    }

    #[test]
    fn tokenize_unicode() {
        let t = web_tokenize("Привет, мир! Hello world 42");
        assert_eq!(
            t,
            vec![
                "привет".to_string(),
                "мир".to_string(),
                "hello".to_string(),
                "world".to_string(),
                "42".to_string()
            ]
        );
    }

    #[test]
    fn short_tokens_dropped() {
        assert!(web_tokenize("a b c").is_empty());
        assert_eq!(
            web_tokenize("ab cd"),
            vec!["ab".to_string(), "cd".to_string()]
        );
    }

    #[test]
    fn cleaning_collapses_blanks() {
        let raw = "line one\n\n\n\n   \nline two\n   \n\n\nline three";
        let c = clean_text(raw, 1024);
        assert!(!c.contains("\n\n\n"));
        assert!(c.contains("line one"));
        assert!(c.contains("line three"));
    }

    #[test]
    fn cleaning_respects_cap() {
        let c = clean_text(&"word ".repeat(1000), 100);
        assert!(c.len() < 200);
    }

    #[test]
    fn lang_detection() {
        assert_eq!(detect_lang("привет мир как дела"), "ru");
        assert_eq!(detect_lang("hello world how are you"), "en");
        assert_eq!(detect_lang("123 456"), "");
        // v2.0 Foundation (2.2): whatlang отличает украинский от русского
        let uk = detect_lang("українська мова це最美 скринька зі значеннями что це");
        assert_eq!(uk, "uk", "украинский текст: {uk}");
    }

    #[test]
    fn snippet_finds_term() {
        let text = "abc ".repeat(50) + "rust ownership model explained here " + &"xyz ".repeat(50);
        let snip = snippet_for(&text, &["ownership".to_string()], 80);
        assert!(snip.contains("ownership"));
        assert!(snip.chars().count() <= 82);
        assert!(snip.starts_with('…'));
    }

    #[test]
    fn snippet_no_term_from_start() {
        let snip = snippet_for("короткий текст", &["нетого".to_string()], 40);
        assert!(!snip.is_empty());
    }

    #[test]
    fn title_fallback() {
        assert_eq!(title_from_text("\n\n  \nЗаголовок страницы\nтекст"), "Заголовок страницы");
    }
}
