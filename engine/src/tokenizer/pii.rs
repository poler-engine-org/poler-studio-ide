//! Zero-copy очистка PII-паттернов (email / телефон / IP / секреты / карты).
//!
//! Ключевое свойство: [`PiiCleaner::clean`] возвращает `Cow::Borrowed`
//! (ноль аллокаций), если в тексте нет ни одного совпадения — типичный
//! случай для исходников и художественного текста. При наличии PII текст
//! собирается один раз с заменой всех spans на маркеры `[EMAIL]`, `[PHONE]`,
//! `[IP]`, `[SECRET]`, `[CARD]`.
//!
//! Телефонный эвристический шаблон требует пост-валидацию (>= 9 цифр),
//! чтобы не съедать числа вида `1300`, `Т-23` или версии библиотек.

use regex::Regex;
use std::borrow::Cow;
use std::sync::LazyLock;

/// Режим очистки PII в пайплайне движка.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiiMode {
    /// Очистка выключена: текст индексируется как есть.
    Off,
    /// Совпадения маскируются маркерами `[EMAIL]` / `[PHONE]` / …
    Mask,
}

/// Класс литерального якоря паттерна (техника ripgrep literal prefilter:
/// regex не запускается, если якорного байта в тексте нет вовсе).
enum Anchor {
    /// Требуется байт `@`.
    At,
    /// Требуется хотя бы одна ASCII-цифра.
    Digit,
    /// Якоря нет — проверяется всегда.
    None,
}

type Pattern = (Regex, &'static str, Anchor);

static PATTERNS: LazyLock<Vec<Pattern>> = LazyLock::new(|| {
    vec![
        // Секреты и токены доступа — проверяются первыми (самые специфичные).
        (
            Regex::new(
                r"\b(?:sk-[A-Za-z0-9_-]{12,}|ghp_[A-Za-z0-9]{20,}|gho_[A-Za-z0-9]{20,}\
                 |AKIA[0-9A-Z]{12,}|xox[baprs]-[A-Za-z0-9-]{10,})\b",
            )
            .unwrap(),
            "[SECRET]",
            Anchor::None,
        ),
        (
            Regex::new(r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}").unwrap(),
            "[EMAIL]",
            Anchor::At,
        ),
        (
            Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").unwrap(),
            "[IP]",
            Anchor::Digit,
        ),
        (
            Regex::new(r"\+?\d[\d\s().\u{2010}-]{7,16}\d").unwrap(),
            "[PHONE]",
            Anchor::Digit,
        ),
        (
            Regex::new(r"\b\d{13,19}\b").unwrap(),
            "[CARD]",
            Anchor::Digit,
        ),
    ]
});

fn digit_count(s: &str) -> usize {
    s.chars().filter(|c| c.is_ascii_digit()).count()
}

/// Санитайзер PII. Строится дёшево; сами регулярные выражения компилируются
/// один раз на процесс через `LazyLock`.
pub struct PiiCleaner {
    _priv: (),
}

impl PiiCleaner {
    /// Создаёт санитайзер (паттерны — глобальные, лениво инициализируемые).
    pub fn new() -> Self {
        Self { _priv: () }
    }

    /// Возвращает очищенную копию текста либо исходный срез без аллокаций.
    ///
    /// Совпадения разных классов, пересекающиеся между собой, разрешаются
    /// в пользу начавшегося раньше (и более длинного) — сортировка spans
    /// по `(start, Reverse(end))`.
    pub fn clean<'a>(&self, text: &'a str) -> Cow<'a, str> {
        // literal prefilter (ripgrep/kwset-техника): наличие якорных байтов
        // проверяется SIMD-поиском memchr до запуска любого regex
        let bytes = text.as_bytes();
        let has_at = memchr::memchr(b'@', bytes).is_some();
        let has_digit = bytes.iter().any(|b| b.is_ascii_digit());

        let mut spans: Vec<(usize, usize, &'static str)> = Vec::new();
        for (re, label, anchor) in PATTERNS.iter() {
            match anchor {
                Anchor::At if !has_at => continue,
                Anchor::Digit if !has_digit => continue,
                _ => {}
            }
            for m in re.find_iter(text) {
                if *label == "[PHONE]" && digit_count(m.as_str()) < 9 {
                    continue; // защита от ложных срабатываний на числах/версиях
                }
                spans.push((m.start(), m.end(), *label));
            }
        }
        if spans.is_empty() {
            return Cow::Borrowed(text);
        }

        spans.sort_by_key(|s| (s.0, std::cmp::Reverse(s.1)));

        let mut out = String::with_capacity(text.len());
        let mut last = 0usize;
        for (s, e, label) in spans {
            if s < last {
                continue; // перекрытие с уже замаскированным span
            }
            out.push_str(&text[last..s]);
            out.push_str(label);
            last = e;
        }
        out.push_str(&text[last..]);
        Cow::Owned(out)
    }
}

impl Default for PiiCleaner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_is_masked() {
        let c = PiiCleaner::new();
        let out = c.clean("Контакт: user@example.com отправил отчёт");
        assert!(out.contains("[EMAIL]"));
        assert!(!out.contains("user@example.com"));
    }

    #[test]
    fn phone_with_enough_digits_is_masked() {
        let c = PiiCleaner::new();
        let out = c.clean("Звоните +7 (495) 123-45-67 сегодня");
        assert!(out.contains("[PHONE]"), "got: {out}");
    }

    #[test]
    fn short_numbers_are_not_phones() {
        let c = PiiCleaner::new();
        let out = c.clean("Температура 1300 и метрика Т-23, версия 1.2.3");
        assert_eq!(out, "Температура 1300 и метрика Т-23, версия 1.2.3");
    }

    #[test]
    fn ip_is_masked() {
        let c = PiiCleaner::new();
        let out = c.clean("Сервер 192.168.10.42 недоступен");
        assert!(out.contains("[IP]"));
        assert!(!out.contains("192.168"));
    }

    #[test]
    fn secret_token_is_masked() {
        let c = PiiCleaner::new();
        let out = c.clean("sk-abcdef1234567890XYZ утёк в логи");
        assert!(out.contains("[SECRET]"), "got: {out}");
    }

    #[test]
    fn clean_text_is_zero_copy() {
        let c = PiiCleaner::new();
        let src = "Обычный текст без персональных данных. Нокс и шунт.";
        let cow = c.clean(src);
        assert!(matches!(cow, Cow::Borrowed(_)));
    }

    #[test]
    fn multiple_pii_in_one_line() {
        let c = PiiCleaner::new();
        let out = c.clean("a@b.io и 10.0.0.1 рядом");
        assert!(out.contains("[EMAIL]"));
        assert!(out.contains("[IP]"));
    }
}
