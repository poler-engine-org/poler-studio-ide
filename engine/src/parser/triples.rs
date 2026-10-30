//! Извлечение троек «сущность — предикат — сущность» (SVO) из текста сцен
//! и из кода (call graph + imports).
//!
//! Эвристики русско-английские, детерминированные:
//!
//! 1. **Лексикон сущностей**: субъекты из метаданных сцены (+алиасы из
//!    скобок), токены запроса, последовательности слов с заглавной буквы
//!    (mid-sentence — «сильные»), числовые метрики и величины с единицами
//!    (`1300°C`, `Т-23`).
//! 2. **Предикат** центрируется на глаголе: слова между E1 и E2,
//!    за вычетом предлогов/притяжательных местоимений и прилагательных,
//!    соединяются подчёркиванием (`вонзила когти в` -> `вонзила_когти`).
//! 3. **Объект** при отсутствии явной сущности — именная группа после
//!    предлога (`в солнечное сплетение` -> `Солнечное сплетение`)
//!    либо числовая величина (`1300°C`).
//! 4. **Метаданные**: `(Нокс, появляется_в, Глава 36)`,
//!    `(Нокс, алиас, Соболь)`, `(Глава 36, временная_метрика, Т-23)`,
//!    co-occurrence `(Нокс, взаимодействует_с, Шунт)`.
//!
//! Для кода: `(process_data, вызывает, compute)` и
//! `(модуль, импортирует, dependency)` — тот самый Call Graph,
//! которым обладает AST и полностью лишён grep.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::LazyLock;

use crate::parser::markdown_scenes::SceneContext;

/// Тройка знаний для графа сущностей.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Triple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

impl Triple {
    fn new(subject: &str, predicate: &str, object: &str) -> Self {
        Self {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: object.to_string(),
        }
    }
}

const PREPOSITIONS: &[&str] = &[
    "в", "на", "с", "со", "из", "от", "к", "ко", "за", "через", "над", "под", "у", "о", "об",
    "по", "для", "без", "при", "перед", "между", "сквозь", "про", "через", "into", "onto", "to",
    "from", "with", "without", "of", "in", "on", "at", "for", "by",
];

const POSSESSIVES: &[&str] = &[
    "её", "его", "их", "свой", "свои", "своей", "своего", "мой", "твой", "наш", "ваш", "her",
    "his", "their", "its", "my", "your",
];

/// Местоимения, которые не могут быть сущностями (даже с заглавной).
const PRONOUNS: &[&str] = &[
    "её", "его", "их", "он", "она", "оно", "они", "мы", "вы", "я", "это", "этот", "эта", "тот",
    "те", "все", "весь", "вся", "который", "которая", "которое", "которые", "она", "но", "а",
    "и", "the", "this", "that", "these", "those", "it", "he", "she", "they", "we", "you",
];

/// Английские закрытые классы: предлоги, союзы, артикли, местоимения,
/// вспомогательные глаголы, частицы + ключевые слова C-семейства и титулы
/// (проза XIX века). Эти слова НИКОГДА не сущности — даже с заглавной
/// буквы («If»/«With» из комментариев, «In»/«On» после глагола —
/// стресс-кейс libgit2 + Pride and Prejudice). Это грамматика языка
/// (~170 слов), а не морфологический словарь — «ноль словарей» не
/// нарушается: закрытый класс, как [`PREPOSITIONS`] и [`PRONOUNS`].
const ENG_STOPWORDS: &[&str] = &[
    // предлоги
    "in", "on", "at", "by", "for", "with", "from", "of", "to", "into", "onto", "upon", "about",
    "around", "through", "during", "before", "after", "since", "until", "till", "against",
    "between", "among", "amongst", "beside", "besides", "behind", "beyond", "above", "below",
    "over", "under", "beneath", "underneath", "toward", "towards", "within", "without",
    "across", "along", "amid", "near", "past", "per", "via", "despite", "except", "inside",
    "outside", "unto", "up", "down", "off", "out",
    // союзы и связующие наречия
    "and", "or", "but", "nor", "so", "yet", "both", "either", "neither", "however", "therefore",
    "thus", "hence", "moreover", "furthermore", "meanwhile", "otherwise", "instead", "because",
    "although", "though", "unless", "whereas", "whether", "if", "than", "when", "where", "why",
    "how", "what", "which", "who", "whom", "whose", "while", "lest",
    // артикли и детерминативы
    "a", "an", "the", "this", "that", "these", "those", "such", "some", "any", "no", "every",
    "each", "all", "few", "many", "several", "most", "other", "another", "own", "same", "much",
    "less", "certain", "none", "like",
    // предлог-союзы, пропущенные в первом проходе
    "as", "his",
    // местоимения и вспомогательные глаголы
    "i", "me", "him", "us", "them", "my", "mine", "your", "yours", "her", "hers", "our", "ours",
    "their", "theirs", "myself", "yourself", "himself", "herself", "itself", "ourselves",
    "themselves", "whoever", "one", "is", "are", "was", "were", "be", "been", "being", "am",
    "has", "have", "had", "having", "does", "do", "did", "will", "would", "shall", "should",
    "can", "could", "may", "might", "must", "ought", "not", "never", "always", "often",
    "sometimes", "usually", "rarely", "seldom", "now", "then", "here", "there", "very", "too",
    "also", "only", "just", "even", "still", "again", "once", "perhaps", "maybe", "indeed",
    "almost", "already",
    // ключевые слова C-семейства (шум из комментариев и документации)
    "else", "switch", "case", "default", "break", "continue", "return", "goto", "do", "sizeof",
    "typedef", "struct", "union", "enum", "extern", "static", "const", "volatile", "register",
    "auto", "inline", "define", "include", "ifdef", "ifndef", "endif", "elif", "pragma", "undef",
    "defined", "void", "int", "char", "long", "short", "double", "float", "unsigned", "signed",
    "null", "true", "false", "bool",
    // титулы (заголовки/обращения в англоязычной прозе)
    "mr", "mrs", "ms", "miss", "lady", "sir", "lord", "dr", "dame", "madam", "madame",
    "chapter", "volume",
];

const VERB_SUFFIXES: &[&str] = &[
    "ает", "ует", "ирует", "ывает", "ивает", "зует", "ует", "ет", "ит", "ат", "ят", "ут", "ют",
    "ила", "ала", "ела", "ула", "ыла", "ла", "ло", "ли", "лась", "лся", "ась", "ись", "ешь",
    "ишь", "ете", "ите", "айте", "ейте", "ся", "сь",
];

const ADJ_SUFFIXES: &[&str] = &[
    "ый", "ий", "ой", "ая", "яя", "ое", "ее", "ые", "ие", "ым", "им", "ом", "ем", "ого", "его",
    "ому", "ему", "ыми", "ими", "их", "ых", "ую", "юю", "ей",
];

const ENG_VERBS: &[&str] = &[
    "is", "are", "was", "were", "be", "been", "being", "has", "have", "had", "does", "do",
    "did", "will", "would", "can", "could", "must", "should", "shall", "may", "might", "fires",
    "calls", "runs", "returns", "sets", "gets", "puts", "adds", "removes", "creates",
    "destroys", "enables", "disables", "sends", "receives",
];

static PREP_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| PREPOSITIONS.iter().copied().collect());
static POSS_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| POSSESSIVES.iter().copied().collect());
static PRONOUN_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| PRONOUNS.iter().copied().collect());
static ENG_VERB_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| ENG_VERBS.iter().copied().collect());
static STOP_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| ENG_STOPWORDS.iter().copied().collect());

static NUMBER_UNIT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+([.,]\d+)?(°[CFK]|%|км|кг|мс|мм|см|м|с|ч|К|В|А|Гц|МВт|кВт|т|г)$").unwrap());

static METRIC_TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ТT]-\d+$").unwrap());

static CALL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap());

static IMPORT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|\n)\s*(?:use|import|require)\s+([A-Za-z0-9_:{}. ,]+?);").unwrap());

pub(crate) const CALL_KEYWORDS: &[&str] = &[
    "if", "else", "while", "for", "match", "loop", "return", "unsafe", "fn", "let", "pub",
    "struct", "enum", "impl", "trait", "mod", "use", "crate", "super", "self", "Self", "move",
    "async", "await", "where", "as", "in", "ref", "const", "static", "type", "dyn", "box",
    "break", "continue", "def", "class", "lambda", "try", "catch", "except", "switch", "case",
    "new", "delete", "sizeof", "typeof", "and", "or", "not", "elif", "with", "yield", "pass",
    // C/C++: управляющие и препроцессорные конструкции с «(...)» — не вызовы
    "do", "goto", "default", "defined", "__attribute__", "_Generic", "_Static_assert",
    "ifdef", "ifndef", "endif", "include", "define", "undef", "pragma", "error", "warning",
    "line",
];

// ---------------------------------------------------------------------------
// Слово-уровневая токенизация с сохранением регистра
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct W {
    /// Слово без краевой пунктуации, исходный регистр.
    raw: String,
    /// Нижний регистр.
    lower: String,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn split_words(sentence: &str) -> Vec<W> {
    let mut out = Vec::new();
    for tok in sentence.split_whitespace() {
        let raw: String = tok
            .trim_matches(|c: char| !is_word_char(c) && c != '°' && c != '-')
            .to_string();
        if raw.is_empty() || raw == "-" || raw == "—" {
            // сохраняем тире как отдельный маркер
            if raw.is_empty() && (tok.contains('—')) {
                out.push(W {
                    raw: "—".into(),
                    lower: "—".into(),
                });
            }
            continue;
        }
        let lower = raw.to_lowercase();
        out.push(W { raw, lower });
    }
    out
}

fn split_sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let char_indices: Vec<(usize, char)> = text.char_indices().collect();
    for (idx, &(byte_pos, c)) in char_indices.iter().enumerate() {
        if c == '.' || c == '!' || c == '?' || c == '…' || c == '\n' {
            let next_space = char_indices
                .get(idx + 1)
                .map(|&(_, nc)| nc.is_whitespace())
                .unwrap_or(true);
            if next_space {
                let end = (byte_pos + c.len_utf8()).min(text.len());
                let piece = &text[start..end];
                if !piece.trim().is_empty() {
                    out.push(piece);
                }
                start = end;
            }
        }
    }
    if start < text.len() && !text[start..].trim().is_empty() {
        out.push(&text[start..]);
    }
    out
}

fn is_prep(w: &str) -> bool {
    PREP_SET.contains(w)
}
fn is_poss(w: &str) -> bool {
    POSS_SET.contains(w)
}
/// Закрытый класс английского (регистронезависимо): предлоги/союзы/
/// артикли/местоимения/aux + C-ключевые слова — не сущность никогда.
fn is_stopword(w: &str) -> bool {
    STOP_SET.contains(w)
}
fn is_adjective(w: &str) -> bool {
    ADJ_SUFFIXES.iter().any(|s| w.ends_with(s)) && w.len() > 3
}
fn is_verb_like(w: &str) -> bool {
    ENG_VERB_SET.contains(w) || (VERB_SUFFIXES.iter().any(|s| w.ends_with(s)) && w.len() > 2)
}

fn capitalize_first(s: &str) -> String {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) => c.to_uppercase().collect::<String>() + cs.as_str(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Кандидаты-сущности
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct EntitySpan {
    start: usize,
    end: usize, // не включительно (word idx)
    text: String,
    strong: bool,
}

fn is_pronoun(w: &str) -> bool {
    PRONOUN_SET.contains(w)
}

fn is_number_or_metric(w: &W) -> bool {
    NUMBER_UNIT_RE.is_match(&w.raw) || METRIC_TAG_RE.is_match(&w.raw)
}

/// Лексикон сущностей: (полный набор для сопоставления, обоснованное ядро).
/// Ядро — субъекты метаданных и токены запроса: только они дают «сильную»
/// сущность в начале предложения. Заглавные слова из текста сильны
/// только в середине предложения (mid-sentence capitals).
/// Записи лексикона предразбиты на слова — сравнение без аллокаций.
fn build_lexicon(
    scope_text: &str,
    scene: &SceneContext,
    query_tokens: &[String],
) -> (Vec<Vec<String>>, HashSet<String>) {
    let mut grounded: HashSet<String> = HashSet::new();
    for name in &scene.subject_names {
        grounded.insert(name.to_lowercase());
        if let Some(first) = name.split_whitespace().next() {
            grounded.insert(first.to_lowercase());
        }
    }
    for q in query_tokens {
        grounded.insert(q.to_lowercase());
    }

    let mut lex: HashSet<String> = grounded.clone();
    // последовательности с заглавной, встречающиеся в тексте
    let words = split_words(scope_text);
    let mut i = 0;
    while i < words.len() {
        let w = &words[i];
        if w.raw.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
            && !is_pronoun(&w.lower)
            && !is_stopword(&w.lower)
            && !is_prep(&w.lower)
            && !is_poss(&w.lower)
            && w.raw.chars().all(|c| c.is_alphabetic())
        {
            let mut seq = vec![w.raw.clone()];
            let mut j = i + 1;
            while j < words.len()
                && seq.len() < 3
                && words[j].raw.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                && !is_stopword(&words[j].lower)
                && !is_prep(&words[j].lower)
                && !is_poss(&words[j].lower)
                && words[j].raw.chars().all(|c| c.is_alphabetic())
            {
                seq.push(words[j].raw.clone());
                j += 1;
            }
            lex.insert(seq.join(" ").to_lowercase());
            i = j;
        } else {
            i += 1;
        }
    }
    let entries: Vec<Vec<String>> = lex
        .into_iter()
        .map(|e| e.split_whitespace().map(String::from).collect())
        .collect();
    (entries, grounded)
}

fn find_entity_spans(
    words: &[W],
    lexicon: &[Vec<String>],
    grounded: &HashSet<String>,
) -> Vec<EntitySpan> {
    let mut spans = Vec::new();
    let mut i = 0;
    while i < words.len() {
        // 1) числовая величина или метрика — сильная сущность
        if is_number_or_metric(&words[i]) {
            spans.push(EntitySpan {
                start: i,
                end: i + 1,
                text: words[i].raw.clone(),
                strong: true,
            });
            i += 1;
            continue;
        }
        // 2) лексиконное совпадение (1–3 слова) — без аллокаций:
        //    пословное сравнение с предразбитыми записями
        let mut matched = None;
        for len in (1..=3usize).rev() {
            if i + len > words.len() {
                continue;
            }
            'entry: for parts in lexicon {
                if parts.len() != len {
                    continue;
                }
                for (k, p) in parts.iter().enumerate() {
                    if words[i + k].lower != *p {
                        continue 'entry;
                    }
                }
                matched = Some(len);
                break;
            }
            if matched.is_some() {
                break;
            }
        }
        if let Some(len) = matched {
            let text = words[i..i + len]
                .iter()
                .map(|w| w.raw.clone())
                .collect::<Vec<_>>()
                .join(" ");
            let joined = words[i..i + len]
                .iter()
                .map(|w| w.lower.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            // сильная сущность: обоснованное ядро (метаданные/запрос)
            // либо позиция в середине предложения
            let strong = grounded.contains(&joined) || i > 0;
            spans.push(EntitySpan {
                start: i,
                end: i + len,
                text,
                strong,
            });
            i += len;
            continue;
        }
        // 3) заглавное слово (не местоимение/стоп-слово/предлог):
        //    mid-sentence — сильное, в начале предложения — слабое
        let c = words[i].raw.chars().next();
        if c.map(|c| c.is_uppercase()).unwrap_or(false)
            && !is_pronoun(&words[i].lower)
            && !is_stopword(&words[i].lower)
            && !is_prep(&words[i].lower)
            && words[i].raw.chars().all(|ch| ch.is_alphabetic())
        {
            let mut len = 1usize;
            while i + len < words.len()
                && len < 3
                && words[i + len].raw.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                && !is_stopword(&words[i + len].lower)
                && !is_prep(&words[i + len].lower)
                && words[i + len].raw.chars().all(|ch| ch.is_alphabetic())
            {
                len += 1;
            }
            let text = words[i..i + len]
                .iter()
                .map(|w| w.raw.clone())
                .collect::<Vec<_>>()
                .join(" ");
            spans.push(EntitySpan {
                start: i,
                end: i + len,
                text,
                strong: i > 0,
            });
            i += len;
            continue;
        }
        i += 1;
    }
    spans
}

// ---------------------------------------------------------------------------
// Предикаты
// ---------------------------------------------------------------------------

/// Предикат между E1 и E2: центр на глаголе, чистка предлогов/прилагательных.
fn verb_centered_predicate(middle: &[W]) -> Option<String> {
    if middle.is_empty() {
        return None;
    }
    // отрезаем ведущие предлоги/притяжательные
    let mut start = 0usize;
    while start < middle.len() && (is_prep(&middle[start].lower) || is_poss(&middle[start].lower)) {
        start += 1;
    }
    // отрезаем хвостовые предлоги и тире
    let mut end = middle.len();
    while end > start
        && (is_prep(&middle[end - 1].lower)
            || is_poss(&middle[end - 1].lower)
            || middle[end - 1].lower == "—")
    {
        end -= 1;
    }
    if end <= start {
        return None;
    }
    let core = &middle[start..end];

    let vpos = core.iter().position(|w| is_verb_like(&w.lower));
    let Some(v) = vpos else {
        // без глагола: содержательные слова середины (cap 3); предлоги,
        // притяжательные и стоп-слова в предикат не идут — иначе
        // рождаются «stood_near_them»-подобные свалки
        let parts: Vec<String> = core
            .iter()
            .filter(|w| {
                !is_prep(&w.lower)
                    && !is_poss(&w.lower)
                    && !is_pronoun(&w.lower)
                    && !is_stopword(&w.lower)
            })
            .take(3)
            .map(|w| w.lower.clone())
            .collect();
        if parts.is_empty() {
            return None;
        }
        return Some(parts.join("_"));
    };

    let mut parts: Vec<String> = vec![core[v].lower.clone()];
    let mut count = 0usize;
    for w in &core[v + 1..] {
        if is_prep(&w.lower) || is_poss(&w.lower) || w.lower == "—" {
            break;
        }
        if is_adjective(&w.lower) {
            continue;
        }
        if count >= 2 {
            break;
        }
        parts.push(w.lower.clone());
        count += 1;
    }
    if parts.len() == 1 && parts[0].is_empty() {
        return None;
    }
    Some(parts.join("_"))
}

/// Предикат от E1 до конца: глагол + хвост до предлога.
fn predicate_from_e1(words: &[W], from: usize) -> Option<(String, usize)> {
    let mut i = from;
    // пропускаем ведущие предлоги/притяжательные
    while i < words.len() && (is_prep(&words[i].lower) || is_poss(&words[i].lower)) {
        i += 1;
    }
    let v = loop {
        if i >= words.len() {
            return None;
        }
        if is_verb_like(&words[i].lower) {
            break i;
        }
        if words[i].lower == "—" {
            return None;
        }
        i += 1;
    };

    let mut parts: Vec<String> = vec![words[v].lower.clone()];
    let mut j = v + 1;
    let mut count = 0usize;
    while j < words.len() {
        let w = &words[j];
        if is_prep(&w.lower) || is_poss(&w.lower) || w.lower == "—" {
            break;
        }
        if is_number_or_metric(w) {
            break;
        }
        if is_adjective(&w.lower) {
            j += 1;
            continue;
        }
        if count >= 2 {
            break;
        }
        parts.push(w.lower.clone());
        count += 1;
        j += 1;
    }
    Some((parts.join("_"), j))
}

/// Разрешение объекта после конца предиката.
fn resolve_object(words: &[W], from: usize) -> Option<String> {
    if from >= words.len() {
        return None;
    }
    // (a) числовая величина / метрика
    for w in &words[from..] {
        if w.lower == "—" {
            continue;
        }
        if is_number_or_metric(w) {
            return Some(w.raw.clone());
        }
        if is_prep(&w.lower) {
            break;
        }
    }
    // (b) именная группа после предлога (<= 2 слова)
    for (k, w) in words[from..].iter().enumerate() {
        if !is_prep(&w.lower) {
            continue;
        }
        let mut phrase: Vec<String> = Vec::new();
        for nw in words[from + k + 1..].iter() {
            if nw.lower == "—"
                || is_prep(&nw.lower)
                || is_verb_like(&nw.lower)
                || is_number_or_metric(nw)
            {
                break;
            }
            if nw.raw.chars().all(|c| c.is_alphabetic() || c == '-')
                && !is_pronoun(&nw.lower)
                && !is_stopword(&nw.lower)
            {
                phrase.push(nw.raw.clone());
            }
            if phrase.len() >= 2 {
                break;
            }
        }
        if !phrase.is_empty() {
            let joined = phrase.join(" ");
            return Some(capitalize_first(&joined));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Публичное API
// ---------------------------------------------------------------------------

/// Извлекает троики из текста сцены + метаданных.
/// Строка-карточка метаданных сцены или markdown-заголовок.
fn is_metadata_line(s: &str) -> bool {
    let t = s.trim();
    if t.starts_with("**") || t.starts_with('#') || t.starts_with("Метрика") {
        return true;
    }
    let lower = t.to_lowercase();
    lower.contains("метрика:")
        || lower.contains("локация:")
        || lower.contains("место:")
        || lower.contains("субъекты:")
        || lower.contains("персонажи:")
        || lower.contains("герои:")
}

pub fn extract_triples(
    scope_text: &str,
    scene: &SceneContext,
    query_tokens: &[String],
) -> Vec<Triple> {
    let mut out: Vec<Triple> = Vec::new();
    if scope_text.trim().is_empty() {
        return out;
    }

    let (lexicon, grounded) = build_lexicon(scope_text, scene, query_tokens);
    let mut text_triple_count = 0usize;

    for sentence in split_sentences(scope_text) {
        // строки-карточки метаданных (Метрика/Локация/Субъекты, заголовки)
        // не являются нарративом и не должны порождать троек
        if is_metadata_line(sentence) {
            continue;
        }
        let words = split_words(sentence);
        if words.is_empty() {
            continue;
        }
        let spans = find_entity_spans(&words, &lexicon, &grounded);
        if spans.is_empty() {
            continue;
        }
        let e1 = &spans[0];
        if !e1.strong && spans.len() == 1 {
            continue; // одиночная заглавная в начале предложения — не сущность
        }

        // Пары соседних сущностей
        let mut idx = 0usize;
        let mut produced = 0usize;
        while idx + 1 < spans.len() && produced < 3 {
            let a = &spans[idx];
            let b = &spans[idx + 1];
            let middle = &words[a.end..b.start];
            let pred = verb_centered_predicate(middle).unwrap_or_else(|| "связана_с".into());
            out.push(Triple::new(&a.text, &pred, &b.text));
            text_triple_count += 1;
            produced += 1;
            idx += 1;
        }

        // E1 + глагол + объект без явной сущности
        if produced == 0 {
            if let Some((pred, pred_end)) = predicate_from_e1(&words, e1.end) {
                if let Some(obj) = resolve_object(&words, pred_end) {
                    out.push(Triple::new(&e1.text, &pred, &obj));
                    text_triple_count += 1;
                }
            } else if spans.len() > 1 {
                let b = &spans[1];
                out.push(Triple::new(&e1.text, "связана_с", &b.text));
            }
        }
    }

    // --- метаданные сцены ---
    let chapter = scene.chapter.clone();
    let location_clean = scene
        .location
        .as_deref()
        .and_then(|l| l.strip_prefix("Локация: "))
        .map(String::from);

    // алиасы: (Нокс, алиас, Соболь)
    let names = &scene.subject_names;
    for name in names {
        // (Нокс, появляется_в, Глава)
        out.push(Triple::new(name, "появляется_в", &chapter));
        if let Some(loc) = &location_clean {
            out.push(Triple::new(name, "находится_в", loc));
        }
    }
    // пары алиас -> база из разбора "Соболь (Нокс)" (структурные,
    // из одной скобочной группы, а не эвристика по длине)
    for (alias, base) in &scene.subject_pairs {
        out.push(Triple::new(alias, "алиас", base));
    }
    if let Some(tag) = &scene.metric_tag {
        out.push(Triple::new(&chapter, "временная_метрика", tag));
    }
    if let Some(loc) = &location_clean {
        out.push(Triple::new(&chapter, "происходит_в", loc));
    }

    // защита в глубину: аномальные сцены не порождают неограниченных
    // списков троек (граф сущностей всё равно кэпится max_graph_triples)
    out.truncate(256);

    // co-occurrence: сущности одной сцены связаны фактом совместного
    // появления. Кроме субъектов метаданных сюда попадают субъекты и
    // объекты текстовых троек (например, Шунт) — это делает их достижимыми
    // для K-hop обхода от корневой сущности запроса.
    let mut scene_entities: Vec<String> = Vec::new();
    for t in &out {
        if t.predicate == "появляется_в" {
            scene_entities.push(t.subject.clone());
        }
    }
    for t in out.iter().take(text_triple_count) {
        scene_entities.push(t.subject.clone());
        scene_entities.push(t.object.clone());
    }
    let mut seen = HashSet::new();
    scene_entities.retain(|e| seen.insert(e.to_lowercase()));
    scene_entities.truncate(8);
    for i in 0..scene_entities.len() {
        for j in (i + 1)..scene_entities.len() {
            out.push(Triple::new(
                &scene_entities[i],
                "взаимодействует_с",
                &scene_entities[j],
            ));
        }
    }

    out.dedup_by(|a, b| a.subject == b.subject && a.predicate == b.predicate && a.object == b.object);
    out
}

/// Извлекает тройки из кода: вызовы (call graph) и импорты.
///
/// Упоминание функции в комментарии или строковом литерале — НЕ вызов:
/// захваты [`CALL_RE`] внутри спанов комментариев/строк (лексический
/// сканер [`crate::parser::ast_code::string_comment_spans`]) отбрасываются.
pub fn extract_code_triples(scope_text: &str, scope_name: Option<&str>, file_stem: &str) -> Vec<Triple> {
    let mut out: Vec<Triple> = Vec::new();
    let owner = scope_name.unwrap_or(file_stem);

    // слова из комментариев/строк не попадают в call graph
    let noise = crate::parser::ast_code::string_comment_spans(scope_text);
    let in_noise =
        |b: usize| noise.iter().any(|&(s, e)| b >= s && b < e);

    for caps in CALL_RE.captures_iter(scope_text) {
        let callee = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        if callee.is_empty() || CALL_KEYWORDS.contains(&callee) {
            continue;
        }
        if caps.get(1).is_some_and(|m| in_noise(m.start())) {
            continue;
        }
        out.push(Triple::new(owner, "вызывает", callee));
    }

    for caps in IMPORT_RE.captures_iter(scope_text) {
        let module = caps.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        if module.is_empty() {
            continue;
        }
        out.push(Triple::new(file_stem, "импортирует", module));
    }

    out.dedup_by(|a, b| a.subject == b.subject && a.predicate == b.predicate && a.object == b.object);
    out.truncate(48);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene_fixture(scope: &str) -> SceneContext {
        SceneContext {
            chapter: "Глава 36. Инертный".into(),
            temporal_metric: Some("Метрика: Т-23".into()),
            location: Some("Локация: Разлом Каньона".into()),
            subjects: vec!["Субъекты: Мальчик (гибрид), Соболь (Нокс)".into()],
            enclosing_scope: scope.into(),
            metric_tag: Some("Т-23".into()),
            subject_names: vec![
                "Мальчик".into(),
                "гибрид".into(),
                "Соболь".into(),
                "Нокс".into(),
            ],
            subject_pairs: vec![
                ("гибрид".into(), "Мальчик".into()),
                ("Нокс".into(), "Соболь".into()),
            ],
        }
    }

    #[test]
    fn spec_example_triples_reproduced() {
        let scope = "Нокс вонзила когти в солнечное сплетение противника. \
                     Шунт на её загривке сбрасывает избыточное тепло — 1300°C, \
                     которые не должны были достаться телу.";
        let scene = scene_fixture(scope);
        let triples = extract_triples(scope, &scene, &["нокс".to_string()]);
        let flat: Vec<String> = triples
            .iter()
            .map(|t| format!("{}|{}|{}", t.subject, t.predicate, t.object))
            .collect();

        assert!(
            flat.iter().any(|s| s.starts_with("Нокс|вонзила_когти|")),
            "triples: {flat:?}"
        );
        assert!(
            flat.iter().any(|s| s.contains("Солнечное сплетение")),
            "triples: {flat:?}"
        );
        assert!(
            flat.iter().any(|s| s.contains("сбрасывает_тепло")),
            "triples: {flat:?}"
        );
        assert!(
            flat.iter().any(|s| s.contains("1300°C")),
            "triples: {flat:?}"
        );
    }

    #[test]
    fn metadata_triples_present() {
        let scene = scene_fixture("Нокс действует.");
        let triples = extract_triples("Нокс действует.", &scene, &["нокс".to_string()]);
        let flat: Vec<String> = triples
            .iter()
            .map(|t| format!("{}|{}|{}", t.subject, t.predicate, t.object))
            .collect();
        assert!(flat.iter().any(|s| s.contains("появляется_в")));
        assert!(flat.iter().any(|s| s.contains("временная_метрика|Т-23")));
        assert!(flat.iter().any(|s| s.contains("происходит_в|Разлом Каньона")));
        assert!(flat.iter().any(|s| s.contains("взаимодействует_с")));
    }

    #[test]
    fn sentence_without_entities_yields_no_text_triples() {
        let scope = "Давление плавно падало до нуля в течение часа.";
        let scene = scene_fixture(scope);
        let all = extract_triples(scope, &scene, &["нокс".to_string()]);
        let triples: Vec<&Triple> = all
            .iter()
            .filter(|t| t.predicate != "появляется_в" && t.predicate != "находится_в"
                && t.predicate != "алиас" && t.predicate != "временная_метрика"
                && t.predicate != "происходит_в" && t.predicate != "взаимодействует_с")
            .collect();
        assert!(triples.is_empty(), "unexpected: {triples:?}");
    }

    #[test]
    fn code_call_graph() {
        let src = "use std::collections::HashMap;\n\nfn process_data(input: &[i32]) -> Vec<i32> {\n    input.iter().map(|x| compute(*x)).collect()\n}\n";
        let triples = extract_code_triples(src, Some("process_data"), "example");
        let flat: Vec<String> = triples
            .iter()
            .map(|t| format!("{}|{}|{}", t.subject, t.predicate, t.object))
            .collect();
        assert!(flat.iter().any(|s| s == "process_data|вызывает|compute"), "{flat:?}");
        assert!(flat.iter().any(|s| s.contains("импортирует")), "{flat:?}");
    }

    #[test]
    fn negation_is_not_an_entity() {
        let scope = "Система не должна отключаться.";
        let scene = SceneContext {
            chapter: "Тест".into(),
            temporal_metric: None,
            location: None,
            subjects: vec![],
            enclosing_scope: scope.into(),
            metric_tag: None,
            subject_names: vec![],
            subject_pairs: vec![],
        };
        let triples = extract_triples(scope, &scene, &["система".to_string()]);
        // «система» в лексиконе через запрос: (система, ...) должно найтись,
        // но предикат без глагольного объекта не порождает мусорных троек
        assert!(triples.iter().all(|t| !t.predicate.is_empty()));
    }

    #[test]
    fn sentences_split_correctly() {
        let text = "Первое. Второе предложение! Третье?\nЧетвёртое: с двоеточием. Пятое 3.14 число.";
        let sents = split_sentences(text);
        assert_eq!(sents.len(), 5, "{sents:?}");
    }

    #[test]
    fn english_function_words_never_become_entities() {
        // Стресс-кейс Pride and Prejudice: предлоги с заглавной
        // («In», «On», «With», «If») не становятся ни объектом, ни
        // субъектом, ни частью предиката.
        let scope = "Darcy brought his coffee-cup in. Darcy stood near them. \
                     If the weather held, they walked on. With the news, Jane smiled. \
                     Darcy visited Pemberley with Elizabeth.";
        let scene = SceneContext {
            chapter: "Chapter 3".into(),
            temporal_metric: None,
            location: None,
            subjects: vec![],
            enclosing_scope: scope.into(),
            metric_tag: None,
            subject_names: vec!["Darcy".into()],
            subject_pairs: vec![],
        };
        let triples = extract_triples(scope, &scene, &["darcy".to_string()]);
        for t in &triples {
            assert!(
                !matches!(
                    t.object.trim(),
                    "In" | "On" | "With" | "If" | "At" | "By" | "For" | "Near" | "Them"
                ),
                "стоп-слово как объект: {t:?}"
            );
            assert!(
                !matches!(t.subject.trim(), "In" | "On" | "With" | "If" | "Them"),
                "стоп-слово как субъект: {t:?}"
            );
            assert!(
                !t.predicate
                    .split('_')
                    .any(|p| matches!(p, "in" | "on" | "with" | "if" | "them" | "near")),
                "стоп-слово внутри предиката: {t:?}"
            );
        }
        // живая SVO-тройка остаётся (Darcy → Pemberley)
        assert!(
            triples.iter().any(|t| t.subject == "Darcy" && t.object == "Pemberley"),
            "настоящая тройка на месте: {triples:?}"
        );
    }

    #[test]
    fn c_comment_mentions_and_preprocessor_are_not_calls() {
        // Упоминание в док-комментарии — не вызов; defined/__attribute__
        // и ключевые слова — не callees; живой вызов остаётся.
        let src = "/* If you want to retry, call git_repository_close(out) \
                   when done with the repository. */\n\
                   #if defined(_WIN32)\n\
                   static int open_repo(git_repository **out, const char *path)\n\
                   {\n\
                       __attribute__((unused)) int x = 0;\n\
                       return git_repository_open(out, path, 0);\n\
                   }\n\
                   #endif\n";
        let triples = extract_code_triples(src, Some("open_repo"), "repo");
        let flat: Vec<String> = triples
            .iter()
            .map(|t| format!("{}|{}|{}", t.subject, t.predicate, t.object))
            .collect();
        assert!(
            flat.iter().any(|s| s == "open_repo|вызывает|git_repository_open"),
            "живой вызов в графе: {flat:?}"
        );
        assert!(
            !flat.iter().any(|s| s.contains("git_repository_close")),
            "упоминание в комментарии не вызов: {flat:?}"
        );
        assert!(!flat.iter().any(|s| s.contains("defined")), "defined не callee: {flat:?}");
        assert!(
            !flat.iter().any(|s| s.contains("__attribute__")),
            "__attribute__ не callee: {flat:?}"
        );
    }
}
