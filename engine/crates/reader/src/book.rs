//! Книга: txt / markdown / fb2 → структурированные фразы.
//!
//! fb2 парсится лёгким сканером тегов (без XML-крейта): текст живёт в
//! <p>…</p> внутри <body>; заголовки <title> — маркеры параграфа.

use crate::phonemes::Tune;
use crate::ReaderError;

/// Одна фраза книги (единица интонации).
#[derive(Debug, Clone, PartialEq)]
pub struct Phrase {
    /// Слова фразы (уже очищенные от пунктуации).
    pub words: Vec<String>,
    /// Интонация по конечной пунктуации.
    pub tune: Tune,
    /// Пауза после фразы (с).
    pub pause_s: f64,
}

/// Распознанный формат книги.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Plain,
    Markdown,
    Fb2,
    PolerBook,
}

/// Определить формат по расширению.
pub fn detect_format(path: &std::path::Path) -> Format {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "md" | "markdown" => Format::Markdown,
        "fb2" => Format::Fb2,
        "poler-book" | "polerbook" => Format::PolerBook,
        _ => Format::Plain,
    }
}

/// Интонация по конечному знаку.
fn tune_of(sent: &str) -> (Tune, f64) {
    let trimmed = sent.trim_end();
    if trimmed.ends_with("...") || trimmed.ends_with('…') {
        (Tune::Musing, 0.55)
    } else if trimmed.ends_with('!') {
        (Tune::Exclam, 0.42)
    } else if trimmed.ends_with('?') {
        (Tune::Question, 0.45)
    } else if trimmed.ends_with('.') {
        (Tune::Statement, 0.42)
    } else {
        (Tune::Statement, 0.35)
    }
}

/// Разбить абзац на фразы по границам предложений.
fn split_phrases(par: &str) -> Vec<Phrase> {
    let mut out = Vec::new();
    // грубое разбиение по . ! ? … (с учётом многоточия)
    let mut rest = par.trim();
    while !rest.is_empty() {
        // найти ближайший терминатор
        let mut cut: Option<(usize, usize)> = None; // (индекс, длина знака)
        let bytes: Vec<(usize, char)> = rest.char_indices().collect();
        for (i, ch) in &bytes {
            if matches!(ch, '.' | '!' | '?' | '…') {
                let mut len = ch.len_utf8();
                // многоточие: поглотить последовательность одинаковых
                if *ch == '.' {
                    while i + len < rest.len() && rest.as_bytes()[i + len] == b'.' {
                        len += 1;
                    }
                }
                cut = Some((*i, len));
                break;
            }
        }
        let (sent, tail) = match cut {
            Some((i, len)) => (&rest[..i + len], &rest[i + len..]),
            None => (rest, ""),
        };
        let words: Vec<String> = sent
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| !w.is_empty())
            .collect();
        if !words.is_empty() {
            let (tune, pause) = tune_of(sent);
            out.push(Phrase {
                words,
                tune,
                pause_s: pause,
            });
        }
        rest = tail.trim_start();
    }
    out
}

/// Разобрать сырой текст (уже без разметки) в фразы.
pub fn parse_text(raw: &str) -> Vec<Phrase> {
    let mut out = Vec::new();
    for par in raw.split("\n\n") {
        let par = par.trim();
        if par.is_empty() {
            continue;
        }
        let mut ph = split_phrases(par);
        if let Some(last) = ph.last_mut() {
            // параграфная пауза поверх фразовой
            last.pause_s += 0.30;
        }
        out.extend(ph);
    }
    out
}

/// Вытащить текст из fb2 (теги <p>; заголовки <title> как параграфы).
pub fn parse_fb2(raw: &str) -> Vec<Phrase> {
    let mut text = String::with_capacity(raw.len() / 2);
    let mut depth = 0usize;
    let mut in_p = false;
    let mut iter = raw.char_indices().peekable();
    let bytes = raw.as_bytes();
    while let Some((i, ch)) = iter.next() {
        if ch == '<' {
            // прочитать имя тега
            let rest = &raw[i + 1..];
            let close = rest.find('>').map(|p| p).unwrap_or(rest.len());
            let tag_raw = &rest[..close.min(rest.len())];
            let tag = tag_raw.trim().trim_start_matches('/');
            let is_close = tag_raw.starts_with('/');
            let is_empty = tag_raw.ends_with('/');
            let lower = tag.to_ascii_lowercase();
            if lower == "p" || lower == "title" || lower == "v" {
                if is_empty {
                    text.push_str("\n\n");
                } else if is_close {
                    in_p = false;
                    depth = depth.saturating_sub(1);
                    text.push('\n');
                } else {
                    in_p = true;
                    depth += 1;
                    // открытие абзаца — граница параграфа (\n\n для parse_text)
                    text.push_str("\n\n");
                }
            }
            // пропустить содержимое тега ВМЕСТЕ с '>' (close+1 символов)
            for _ in 0..=close.min(rest.len().saturating_sub(1)) {
                if iter.next().is_none() {
                    break;
                }
            }
            continue;
        }
        if in_p {
            text.push(ch);
            let _ = depth;
            let _ = bytes;
        }
    }
    // переводы строк: серия ≥2 — граница параграфа (\n\n), одиночный — пробел
    // (внутритекстовые переносы внутри <p> склеиваются словом)
    let mut normalized = String::with_capacity(text.len());
    let mut nl_run = 0usize;
    for ch in text.chars() {
        if ch == '\n' {
            nl_run += 1;
            continue;
        }
        if nl_run > 0 {
            normalized.push_str(if nl_run >= 2 { "\n\n" } else { " " });
            nl_run = 0;
        }
        normalized.push(ch);
    }
    if nl_run > 0 {
        normalized.push_str(if nl_run >= 2 { "\n\n" } else { " " });
    }
    parse_text(&normalized)
}

/// Прочитать книгу из файла (формат по расширению).
pub fn load_book(path: &std::path::Path) -> Result<Vec<Phrase>, ReaderError> {
    let raw = std::fs::read_to_string(path).map_err(ReaderError::Io)?;
    let phrases = match detect_format(path) {
        Format::Fb2 => parse_fb2(&raw),
        Format::Markdown => parse_text(&strip_markdown(&raw)),
        _ => parse_text(&raw),
    };
    if phrases.is_empty() {
        return Err(ReaderError::BadInput(format!(
            "в {} не найдено текста для чтения",
            path.display()
        )));
    }
    Ok(phrases)
}

/// Убрать markdown-разметку (заголовки/выделение/ссылки) — v1.
fn strip_markdown(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    for line in md.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            continue; // блоки кода пропускаем
        }
        let stripped = t
            .trim_start_matches('#')
            .trim()
            .replace("**", "")
            .replace('*', "")
            .replace("__", "")
            .replace('`', "");
        // ссылки [текст](url) → текст
        let mut clean = String::with_capacity(stripped.len());
        let mut chars = stripped.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '[' {
                for c2 in chars.by_ref() {
                    if c2 == ']' {
                        break;
                    }
                    clean.push(c2);
                }
                // пропустить (url)
                if chars.peek() == Some(&'(') {
                    for c2 in chars.by_ref() {
                        if c2 == ')' {
                            break;
                        }
                    }
                }
            } else {
                clean.push(c);
            }
        }
        if !clean.trim().is_empty() {
            out.push_str(&clean);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_text() {
        let ph = parse_text("Привет мир. Как дела?\n\nНовый абзац!");
        assert_eq!(ph.len(), 3);
        assert_eq!(ph[0].tune, Tune::Statement);
        assert_eq!(ph[1].tune, Tune::Question);
        assert_eq!(ph[2].tune, Tune::Exclam);
        // параграфная пауза добавлена к ПОСЛЕДНЕЙ фразе абзаца
        assert!((ph[1].pause_s - (0.45 + 0.30)).abs() < 1e-9);
        assert!(ph[1].pause_s > ph[0].pause_s);
        // последняя фраза книги тоже получает абзацную паузу (хвост тишины)
        assert!(ph[2].pause_s >= 0.70);
    }

    #[test]
    fn ellipsis_is_musing() {
        let ph = parse_text("Может быть потом…");
        assert_eq!(ph[0].tune, Tune::Musing);
    }

    #[test]
    fn words_cleaned_from_punctuation() {
        let ph = parse_text("Слово, другое; третье.");
        assert_eq!(ph[0].words, vec!["Слово", "другое", "третье"]);
    }

    #[test]
    fn fb2_paragraphs() {
        let fb2 = r#"<?xml version="1.0"?>
<FictionBook><body><section><title>Глава</title>
<p>Первое предложение.</p>
<p>Второе предложение?</p>
</section></body></FictionBook>"#;
        let ph = parse_fb2(fb2);
        assert_eq!(ph.len(), 3);
        assert_eq!(ph[1].words, vec!["Первое", "предложение"]);
        assert_eq!(ph[2].tune, Tune::Question);
    }

    #[test]
    fn markdown_stripped() {
        let md = "# Заголовок\n\nТекст с **выделением** и [ссылкой](http://x).\n";
        let t = strip_markdown(md);
        assert!(!t.contains('#'));
        assert!(!t.contains("**"));
        assert!(!t.contains("http"));
        assert!(t.contains("Заголовок"));
        assert!(t.contains("выделением"));
        assert!(t.contains("ссылкой"));
    }

    #[test]
    fn format_detection() {
        assert_eq!(detect_format(std::path::Path::new("a.fb2")), Format::Fb2);
        assert_eq!(detect_format(std::path::Path::new("a.md")), Format::Markdown);
        assert_eq!(
            detect_format(std::path::Path::new("a.poler-book")),
            Format::PolerBook
        );
        assert_eq!(detect_format(std::path::Path::new("a.txt")), Format::Plain);
    }

    #[test]
    fn empty_input_no_phrases() {
        assert!(parse_text("   \n\n  ").is_empty());
    }
}
