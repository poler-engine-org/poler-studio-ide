//! Выделение семантически целостных сцен из markdown/текста.
//!
//! Решение проблемы Chunk Fragmentation: границы enclosing scope
//! привязаны к структуре документа (заголовки сцен/глав), а не к
//! нарезке по N токенов. Метаданные сцены (временная метрика,
//! локация, субъекты) парсятся из строк-карточек над совпадением
//! и попадают в ContextAnchor без потери квантификаторов.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::parser::ast_code::CodeScope;

/// Контекст сцены, в которой найдено совпадение.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneContext {
    /// Глава / раздел (из `#`-заголовка либо имени файла).
    pub chapter: String,
    /// Временная метрика в формате контракта: `Метрика: Т-23`.
    pub temporal_metric: Option<String>,
    /// Локация в формате контракта: `Локация: Разлом Каньона`.
    pub location: Option<String>,
    /// Субъекты сцены (метки контракта).
    pub subjects: Vec<String>,
    /// Полный текст законченной сцены (enclosing scope).
    pub enclosing_scope: String,
    /// Распознанный тег метрики (`Т-23`) — для temporal-фильтрации. В JSON не попадает.
    #[serde(skip)]
    pub metric_tag: Option<String>,
    /// Распарсенные имена субъектов и их алиасы — для графа сущностей.
    #[serde(skip)]
    pub subject_names: Vec<String>,
    /// Пары (алиас, базовое имя) из одной скобочной группы.
    #[serde(skip)]
    pub subject_pairs: Vec<(String, String)>,
}

/// Обрезает строку до `max_bytes` без разрыва UTF-8 символа.
pub fn truncate_char_safe(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = s[..end].to_string();
    out.push('…');
    out
}

fn strip_inline_md(s: &str) -> String {
    s.trim()
        .trim_matches(|c: char| c == '*' || c == '`' || c == '#' || c == '-' || c == '>')
        .trim()
        .to_string()
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Потолки защиты от патологических «карточек» (bug: строка-мегабайт со
/// «Субъекты:» внутри раздувала метаданные до гигабайт через каскад
/// subject_names → тройки → co-occurrence).
const META_VALUE_MAX: usize = 256;
const META_LINE_MAX: usize = 512;
const META_SCAN_MAX: usize = 8 * 1024;
const SUBJECTS_MAX: usize = 16;
const SUBJECT_NAME_MAX: usize = 80;
/// Жёсткий потолок enclosing_scope при построении сцены (pass 3).
const SCOPE_HARD_MAX: usize = 1024 * 1024;

/// Обрезает строку до max байт по границе символа.
fn clip(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Границы сцены (лёгкая структура — вычисляется на каждое совпадение).
#[derive(Debug, Clone)]
pub struct SceneBounds {
    /// Глава / раздел.
    pub chapter: String,
    /// Байтовое начало сцены.
    pub start: usize,
    /// Байтовый конец сцены (не включительно).
    pub end: usize,
    /// true — сцена ограничена заголовками markdown; false — txt-абзац.
    pub structured: bool,
}

/// Заголовок markdown: уровень (#..######) и чистый текст.
fn parse_heading(line: &str) -> Option<(usize, String)> {
    let t = line.trim_start();
    if !t.starts_with('#') {
        return None;
    }
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if hashes > 6 {
        return None;
    }
    let rest = t[hashes..].trim();
    if rest.is_empty() {
        return None;
    }
    Some((hashes, strip_inline_md(rest)))
}

/// Лёгкие метаданные сцены: всё, кроме текста сцены (никаких клонов
/// enclosing_scope) — используется в проходе 2 потокового конвейера.
#[derive(Debug, Clone, Default)]
pub struct LightSceneMeta {
    pub chapter: String,
    pub temporal_metric: Option<String>,
    pub metric_tag: Option<String>,
    pub location: Option<String>,
    pub subjects: Vec<String>,
    pub subject_names: Vec<String>,
    pub subject_pairs: Vec<(String, String)>,
}

/// Есть ли в тексте markdown-заголовки (для reconstructed bounds).
pub fn has_headings(text: &str) -> bool {
    text.lines().any(|l| parse_heading(l).is_some())
}

/// Парсит карточку метаданных сцены (структурированной или txt-фолбэк).
pub fn light_meta(text: &str, bounds: &SceneBounds) -> LightSceneMeta {
    let start = bounds.start.min(text.len());
    let end = bounds.end.min(text.len()).max(start);
    // Карточка метаданных живёт в начале сцены: сканируем только
    // META_SCAN_MAX байт, каждая строка обрезается до META_LINE_MAX —
    // гигантские строки-простыни не участвуют в разборе.
    let scan_zone = clip(&text[start..end], META_SCAN_MAX);

    let mut m = LightSceneMeta {
        chapter: bounds.chapter.clone(),
        ..LightSceneMeta::default()
    };

    if bounds.structured {
        // метаданные — карточка сцены: первые 60 строк от начала сцены
        for raw_line in scan_zone.lines().take(60) {
            let line = clip(raw_line, META_LINE_MAX);
            let lower = line.to_lowercase();
            let cleaned = strip_inline_md(line);
            if m.temporal_metric.is_none() && lower.contains("метрика:") {
                let value = after_colon(&cleaned);
                if !value.is_empty() {
                    m.metric_tag = extract_metric_tag(&value);
                    m.temporal_metric = Some(format!("Метрика: {value}"));
                }
            }
            if m.location.is_none()
                && (lower.contains("локация:") || lower.contains("место:"))
            {
                let value = after_colon(&cleaned);
                if !value.is_empty() {
                    m.location = Some(format!("Локация: {value}"));
                }
            }
            if m.subjects.is_empty()
                && (lower.contains("субъекты:")
                    || lower.contains("персонажи:")
                    || lower.contains("герои:")
                    || lower.contains("действующие лица:"))
            {
                let value = after_colon(&cleaned);
                if !value.is_empty() {
                    m.subjects.push(format!("Субъекты: {value}"));
                    let (names, pairs) = parse_subjects(&value);
                    m.subject_names = names;
                    m.subject_pairs = pairs;
                }
            }
            if m.temporal_metric.is_some() && m.location.is_some() && !m.subjects.is_empty() {
                break; // карточка прочитана целиком
            }
        }
    } else {
        // txt-фолбэк: метаданные ищутся строками выше абзаца (макс. 50)
        for raw_line in text[..start].lines().rev().take(50) {
            let line = clip(raw_line, META_LINE_MAX);
            let lower = line.to_lowercase();
            let cleaned = strip_inline_md(line);
            if m.temporal_metric.is_none() && lower.contains("метрика:") {
                let value = after_colon(&cleaned);
                if !value.is_empty() {
                    m.metric_tag = extract_metric_tag(&value);
                    m.temporal_metric = Some(format!("Метрика: {value}"));
                }
            }
            if m.location.is_none()
                && (lower.contains("локация:") || lower.contains("место:"))
            {
                let value = after_colon(&cleaned);
                if !value.is_empty() {
                    m.location = Some(format!("Локация: {value}"));
                }
            }
            if m.subjects.is_empty()
                && (lower.contains("субъекты:")
                    || lower.contains("персонажи:")
                    || lower.contains("герои:"))
            {
                let value = after_colon(&cleaned);
                if !value.is_empty() {
                    m.subjects.push(format!("Субъекты: {value}"));
                    let (names, pairs) = parse_subjects(&value);
                    m.subject_names = names;
                    m.subject_pairs = pairs;
                }
            }
        }
    }
    m
}

/// Кэшированный локатор сцен: заголовки markdown сканируются ОДИН раз
/// на файл/чанк, затем `locate(off)` выполняется за O(log H + окно).
/// Прежний `SceneContext::locate` пересканировал весь текст на каждом
/// хите — на суперчастотных запросах это давало терабайты сканирования.
pub struct SceneLocator {
    headings: Vec<(usize, usize, usize, String)>,
    stem: String,
}

impl SceneLocator {
    /// Предвычисляет заголовки (один проход по тексту).
    pub fn new(text: &str, file_path: &Path) -> Self {
        let mut headings = Vec::new();
        let mut pos = 0usize;
        for line in text.lines() {
            if let Some((lvl, h)) = parse_heading(line) {
                headings.push((pos, pos + line.len(), lvl, h));
            }
            pos += line.len() + 1;
        }
        Self {
            headings,
            stem: file_stem(file_path),
        }
    }

    /// Локализация сцены для байтового смещения (дёшево).
    pub fn locate(&self, text: &str, off: usize) -> SceneBounds {
        let off = off.min(text.len());

        if self.headings.is_empty() {
            // txt: ограниченный поиск абзаца (64 КБ окно)
            const PARAGRAPH_WINDOW: usize = 64 * 1024;
            let mut back_from = off.saturating_sub(PARAGRAPH_WINDOW);
            while back_from < off && !text.is_char_boundary(back_from) {
                back_from += 1;
            }
            let start = text[back_from..off]
                .rfind("\n\n")
                .map(|i| back_from + i + 2)
                .unwrap_or(back_from);
            let mut fwd_to = (off + PARAGRAPH_WINDOW).min(text.len());
            while fwd_to > off && !text.is_char_boundary(fwd_to) {
                fwd_to += 1;
            }
            let fwd_to = fwd_to.min(text.len());
            let end = text[off..fwd_to]
                .find("\n\n")
                .map(|i| off + i)
                .unwrap_or(fwd_to);
            return SceneBounds {
                chapter: self.stem.clone(),
                start,
                end,
                structured: false,
            };
        }

        // markdown: ближайший заголовок сверху (бинарный поиск)
        let idx = self
            .headings
            .binary_search_by(|h| h.0.cmp(&off))
            .map(|i| i + 1)
            .unwrap_or_else(|i| i);
        let above = &self.headings[..idx.min(self.headings.len())];
        let (scene_start, scene_level) = match above.last() {
            Some(h) => (h.0, h.2),
            None => (0, 7),
        };
        let scene_end = self
            .headings
            .iter()
            .filter(|h| h.0 > scene_start && h.2 <= scene_level)
            .map(|h| h.0)
            .min()
            .unwrap_or(text.len());

        let chapter = self
            .headings
            .iter()
            .rev()
            .find(|h| h.0 <= off && h.2 == 1)
            .or_else(|| self.headings.first())
            .map(|h| h.3.clone())
            .unwrap_or_else(|| self.stem.clone());

        SceneBounds {
            chapter,
            start: scene_start,
            end: scene_end,
            structured: true,
        }
    }
}

impl SceneContext {
    /// Лёгкая локализация сцены для байтового смещения (без клонирования
    /// текста) — вызывается на каждое совпадение.
    pub fn locate(text: &str, byte_offset: usize, file_path: &Path) -> SceneBounds {
        let off = byte_offset.min(text.len());

        let mut headings: Vec<(usize, usize, usize, String)> = Vec::new();
        let mut pos = 0usize;
        for line in text.lines() {
            if let Some((lvl, h)) = parse_heading(line) {
                headings.push((pos, pos + line.len(), lvl, h));
            }
            pos += line.len() + 1;
        }

        // txt без заголовков: абзац как enclosing scope.
        // Поиск границ абзаца ограничен окном PARAGRAPH_WINDOW байт:
        // в корпусах без пустых строк (LM1B: предложение = строка)
        // неограниченный поиск перевода-абзаца сканировал весь файл
        // НА КАЖДОМ хите — терабайты на суперчастотных запросах.
        if headings.is_empty() {
            const PARAGRAPH_WINDOW: usize = 64 * 1024;
            // выравнивание границ окна по границам символов UTF-8
            let mut back_from = off.saturating_sub(PARAGRAPH_WINDOW);
            while back_from < off && !text.is_char_boundary(back_from) {
                back_from += 1;
            }
            let start = text[back_from..off]
                .rfind("\n\n")
                .map(|i| back_from + i + 2)
                .unwrap_or(back_from);
            let mut fwd_to = (off + PARAGRAPH_WINDOW).min(text.len());
            while fwd_to > off && !text.is_char_boundary(fwd_to) {
                fwd_to += 1;
            }
            let fwd_to = fwd_to.min(text.len());
            let end = text[off..fwd_to]
                .find("\n\n")
                .map(|i| off + i)
                .unwrap_or(fwd_to);
            return SceneBounds {
                chapter: file_stem(file_path),
                start,
                end,
                structured: false,
            };
        }

        let above: Vec<&(usize, usize, usize, String)> =
            headings.iter().filter(|h| h.0 <= off).collect();
        let (scene_start, scene_level) = match above.last() {
            Some(h) => (h.0, h.2),
            None => (0, 7), // выше любого уровня
        };
        // Сцена — семантически целостный блок: заканчивается на первом
        // заголовке ПОСЛЕ совпадения (любого уровня); если такового нет —
        // на границе родительского раздела (уровень <= scene_level) или EOF.
        let scene_end = headings
            .iter()
            .filter(|h| h.0 > off)
            .map(|h| h.0)
            .min()
            .or_else(|| {
                headings
                    .iter()
                    .filter(|h| h.0 > scene_start && h.2 <= scene_level)
                    .map(|h| h.0)
                    .min()
            })
            .unwrap_or(text.len());

        let chapter = headings
            .iter()
            .rfind(|h| h.0 <= off && h.2 == 1)
            .or_else(|| headings.first())
            .map(|h| h.3.clone())
            .unwrap_or_else(|| file_stem(file_path));

        SceneBounds {
            chapter,
            start: scene_start,
            end: scene_end,
            structured: true,
        }
    }

    /// Полное построение SceneContext по границам — один раз на уникальную
    /// сцену (кэшируется вызывающим кодом). Делегирует разбор карточки
    /// метаданных [`light_meta`].
    pub fn build(text: &str, bounds: &SceneBounds, _file_path: &Path) -> SceneContext {
        let start = bounds.start.min(text.len());
        let end = bounds.end.min(text.len()).max(start);
        let scope_raw = clip(&text[start..end], SCOPE_HARD_MAX);
        let meta = light_meta(text, bounds);

        SceneContext {
            chapter: meta.chapter,
            temporal_metric: meta.temporal_metric,
            location: meta.location,
            subjects: meta.subjects,
            enclosing_scope: scope_raw.trim().to_string(),
            metric_tag: meta.metric_tag,
            subject_names: meta.subject_names,
            subject_pairs: meta.subject_pairs,
        }
    }

    /// SceneContext из одних метаданных (enclosing_scope пуст) — для
    /// извлечения троек в проходе 2 без клонирования текста сцены.
    pub fn from_meta(meta: &LightSceneMeta) -> Self {
        Self {
            chapter: meta.chapter.clone(),
            temporal_metric: meta.temporal_metric.clone(),
            location: meta.location.clone(),
            subjects: meta.subjects.clone(),
            enclosing_scope: String::new(),
            metric_tag: meta.metric_tag.clone(),
            subject_names: meta.subject_names.clone(),
            subject_pairs: meta.subject_pairs.clone(),
        }
    }

    /// Совместимый API: локализация + построение за один вызов.
    pub fn extract_from_text(text: &str, byte_offset: usize, file_path: &Path) -> Self {
        let bounds = Self::locate(text, byte_offset, file_path);
        Self::build(text, &bounds, file_path)
    }

    /// Строит сцену из кодового скоупа (файлы исходников).
    pub fn from_code_scope(scope: &CodeScope, file_path: &Path, max_scope_bytes: usize) -> Self {
        let sig = match (&scope.name, scope.scope_type.as_str()) {
            (Some(n), ty) => format!("{} :: {} (L{}–L{})", file_stem(file_path), n, scope.start_line, scope.end_line).replace(" :: ", &format!(" [{ty}] :: ")),
            (None, ty) => format!("{} [{}] (L{}–L{})", file_stem(file_path), ty, scope.start_line, scope.end_line),
        };
        SceneContext {
            chapter: sig,
            temporal_metric: None,
            location: None,
            subjects: Vec::new(),
            enclosing_scope: truncate_char_safe(&scope.text, max_scope_bytes),
            metric_tag: None,
            subject_names: Vec::new(),
            subject_pairs: Vec::new(),
        }
    }
}

fn after_colon(s: &str) -> String {
    match s.find(':') {
        Some(i) => strip_inline_md(clip(&s[i + 1..], META_VALUE_MAX)),
        None => String::new(),
    }
}

/// `Т-23` / `T-23` -> нормализованный `Т-23`.
fn extract_metric_tag(value: &str) -> Option<String> {
    let mut i = 0usize;
    while i < value.len() {
        let c = value[i..].chars().next()?;
        let clen = c.len_utf8();
        if matches!(c, 'Т' | 'т' | 'T' | 't') && value[i + clen..].starts_with('-') {
            let digits: String = value[i + clen + 1..]
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect();
            if !digits.is_empty() {
                return Some(format!("Т-{digits}"));
            }
        }
        i += clen;
    }
    None
}

/// `Мальчик (гибрид), Соболь (Нокс)` -> (плоский список имён,
/// пары алиас -> базовое имя из той же части).
fn parse_subjects(value: &str) -> (Vec<String>, Vec<(String, String)>) {
    let mut names = Vec::new();
    let mut pairs = Vec::new();
    for part in value.split(',') {
        if names.len() >= SUBJECTS_MAX {
            break; // карточка не может перечислять тысячи субъектов
        }
        let p = clip(part.trim(), SUBJECT_NAME_MAX);
        if p.is_empty() {
            continue;
        }
        if p.ends_with(')') {
            if let Some(open) = p.find('(') {
                let base = p[..open].trim();
                let alias = p[open + 1..p.len() - 1].trim();
                if !base.is_empty() {
                    names.push(base.to_string());
                }
                if !alias.is_empty() {
                    names.push(alias.to_string());
                    if !base.is_empty() {
                        pairs.push((alias.to_string(), base.to_string()));
                    }
                }
                continue;
            }
        }
        names.push(p.to_string());
    }
    (names, pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CH: &str = "# Глава 36. Инертный\n\n**Метрика: Т-23**\n**Локация: Разлом Каньона**\n**Субъекты: Мальчик (гибрид), Соболь (Нокс)**\n\nНокс вонзила когти в солнечное сплетение противника. Шунт сбрасывает тепло.\n";

    #[test]
    fn chapter_and_metadata_extracted() {
        let off = CH.find("вонзила").unwrap();
        let sc = SceneContext::extract_from_text(CH, off, Path::new("/book/chapter_36.md"));
        assert_eq!(sc.chapter, "Глава 36. Инертный");
        assert_eq!(sc.temporal_metric.as_deref(), Some("Метрика: Т-23"));
        assert_eq!(sc.location.as_deref(), Some("Локация: Разлом Каньона"));
        assert_eq!(sc.subjects.len(), 1);
        assert!(sc.subjects[0].contains("Соболь (Нокс)"));
        assert_eq!(sc.metric_tag.as_deref(), Some("Т-23"));
        assert!(sc.enclosing_scope.contains("Шунт"));
    }

    #[test]
    fn subject_names_parsed_with_aliases() {
        let off = CH.find("вонзила").unwrap();
        let sc = SceneContext::extract_from_text(CH, off, Path::new("/book/chapter_36.md"));
        assert!(sc.subject_names.contains(&"Мальчик".to_string()));
        assert!(sc.subject_names.contains(&"Нокс".to_string()));
        assert!(sc.subject_names.contains(&"Соболь".to_string()));
        assert!(sc.subject_names.contains(&"гибрид".to_string()));
    }

    #[test]
    fn scene_stops_at_next_heading() {
        let text = "# Глава 1\n\nТекст сцены один с keyword.\n\n## Подсцена\n\nДругой текст.\n\n# Глава 2\n\nЕщё текст.\n";
        let off = text.find("keyword").unwrap();
        let sc = SceneContext::extract_from_text(text, off, Path::new("/b/a.md"));
        assert!(sc.enclosing_scope.contains("сцены один"));
        assert!(!sc.enclosing_scope.contains("Другой текст"));
    }

    #[test]
    fn subsection_hit_uses_subsection_scope_but_chapter_heading() {
        let text = "# Глава 1\n\nВступление.\n\n## Подсцена\n\nНокс действует.\n";
        let off = text.find("Нокс").unwrap();
        let sc = SceneContext::extract_from_text(text, off, Path::new("/b/a.md"));
        assert_eq!(sc.chapter, "Глава 1");
        assert!(sc.enclosing_scope.contains("Подсцена"));
        assert!(!sc.enclosing_scope.contains("Вступление"));
    }

    #[test]
    fn txt_falls_back_to_paragraph() {
        let text = "Первый абзац.\n\nВторой абзац с Нокс внутри.\n\nТретий абзац.\n";
        let off = text.find("Нокс").unwrap();
        let sc = SceneContext::extract_from_text(text, off, Path::new("/b/plain.txt"));
        assert_eq!(sc.enclosing_scope, "Второй абзац с Нокс внутри.");
        assert_eq!(sc.chapter, "plain");
    }

    #[test]
    fn markdown_bold_is_stripped() {
        let text = "# Глава\n\n**Метрика: Т-7**\n\nКлючевое слово здесь.\n";
        let off = text.find("Ключевое").unwrap();
        let sc = SceneContext::extract_from_text(text, off, Path::new("/b/x.md"));
        assert_eq!(sc.temporal_metric.as_deref(), Some("Метрика: Т-7"));
        assert_eq!(sc.metric_tag.as_deref(), Some("Т-7"));
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        let s = "кириллица".repeat(100);
        let t = truncate_char_safe(&s, 25); // 12.5 символов кириллицы
        assert!(t.len() <= 25 + 3);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn metric_tag_variants() {
        assert_eq!(extract_metric_tag("Т-23"), Some("Т-23".to_string()));
        assert_eq!(extract_metric_tag("t-5 хвост"), Some("Т-5".to_string()));
        assert_eq!(extract_metric_tag("нет метрики"), None);
    }
}
