//! Детект намерения запроса: код ↔ проза (v0.82.0).
//!
//! Проблема, вскрытая стресс-тестом на реальных репо (gin-gonic, express):
//! запрос `func Default` топ-10 забивает `docs/doc.md` — слово «default»
//! встречается в документации сотни раз, IIR-резонанс накапливается до
//! R≈55000 против R≈4240 у настоящей точки определения в `gin.go`.
//! Разработчик/агент вынужден руками дописывать `--extensions go`.
//!
//! Симметричное решение (санкция владельца):
//! * запрос — синтаксис кода (`func`, `def`, `fn`, `class`, `pub`,
//!   прикреплённые скобки `Default(`) → приоритет исходникам, документация
//!   сдвигается вниз;
//! * запрос — естественный язык / тема / нарратив → приоритет документации
//!   и прозе, код сдвигается вниз (слова могут промелькнуть в комментариях).
//!
//! ## Почему тиры, а не штраф по энергии
//!
//! Мультипликативный штраф (R × 0.2) НЕ решаетreported-кейс:
//! 55000 × 0.2 = 11000 > 4240 — документация всё ещё вытесняет код.
//! IIR-резонанс накапливается неограниченно с числом хитов, перекрёстная
//! сопоставимость R между файлами разного размера нарушена — ANY константа
//! штрафа хрупка. Вместо фальсификации метрики ранжирование становится
//! ДВУХУРОВНЕВЫМ: ключ сортировки = (tir, −R). ε и R остаются честными
//! (канон MVR: метрики не искажаются политикой), гарантируется сдвиг
//! НЕЗАВИСИМО от магнитуд резонанса.
//!
//! Тиры:
//! * 0 — класс файла соответствует намерению (или намерение нейтрально);
//! * 1 — класс файла конфликтует с намерением;
//! * 2 — активен точный сигнатурный запрос, но хит НЕ совпал с паттерном
//!   (запросил `func Default(` — получил `form:",default=1"`).
//!
//! ## Сигнатурный слой
//!
//! Токенизация запроса рвёт скобки: `func Default()` → [func, default] —
//! точность выражения теряется, proximity-AND подтягивает случайные
//! `default` из struct-тегов. Если в запросе есть ПРИКРЕПЛЁННАЯ скобка
//! (word-символ, за которым сразу `(` или `[`), включается литеральный
//! сигнатурный паттерн: слова-префиксы (case-insensitive, разделители —
//! любые не-alnum-символы: `::`, `->`, `.`) + якорь (case-SENSITIVE,
//! идентификаторы чувствительны к регистру) + скобка. Хиты, где паттерн
//! совпал байт-в-байт, поднимаются в тир 0; остальные — в тир 2.

use std::path::Path;

/// Кириллица (блоки Unicode: основной + дополнительный): надёжный маркер
/// прозаического запроса — идентификаторы кода кириллицей — экзотика.
fn is_cyrillic(c: char) -> bool {
    matches!(c, '\u{0400}'..='\u{04FF}' | '\u{0500}'..='\u{052F}')
}

/// Намерение запроса: код, проза или нейтрально.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryIntent {
    /// Запрос содержит синтаксис кода — приоритет исходникам.
    Code,
    /// Запрос — естественный язык — приоритет документации/прозе.
    Prose,
    /// Маркеров нет — ранжирование без intent-тиров.
    Neutral,
}

/// Класс файла по расширению.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileClass {
    /// Документация и проза: md, txt, rst…
    Doc,
    /// Исходный код: rs, go, py, c, js…
    Code,
    /// Данные/конфиги: json, toml, yaml…
    Data,
}

/// Режим intent-детекта (CLI `--intent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IntentMode {
    /// Автодетект по маркерам запроса (по умолчанию).
    #[default]
    Auto,
    /// Принудительно: приоритет коду.
    Code,
    /// Принудительно: приоритет документации.
    Prose,
    /// Выключить intent-ранжирование.
    Off,
}

impl IntentMode {
    /// Эффективное намерение для данного запроса.
    pub fn resolve(self, query: &str) -> QueryIntent {
        match self {
            Self::Auto => detect_query_intent(query),
            Self::Code => QueryIntent::Code,
            Self::Prose => QueryIntent::Prose,
            Self::Off => QueryIntent::Neutral,
        }
    }
}

/// Ключевые слова синтаксиса кода (целые whitespace-токены запроса).
const CODE_KEYWORDS: &[&str] = &[
    "func", "fn", "def", "class", "struct", "enum", "impl", "trait", "interface", "type",
    "pub", "priv", "private", "protected", "static", "const", "let", "var", "return",
    "async", "await", "package", "import", "module", "void", "typedef", "inline",
    "extern", "override", "virtual", "template", "namespace", "using", "new", "delete",
    "self", "this", "super", "constructor", "destructor",
];

/// Вопросительные слова — верный признак прозы.
const QUESTION_WORDS: &[&str] = &[
    "how", "what", "why", "where", "who", "when", "which",
    "как", "что", "где", "почему", "кто", "когда", "сколько", "зачем", "чем", "чему",
];

/// Хинты документации: такие слова в запросе → ищут описание, не код.
const DOC_HINT_WORDS: &[&str] = &[
    "readme", "changelog", "install", "uninstall", "guide", "tutorial", "manual",
    "documentation", "license", "architecture", "introduction", "overview", "example",
    "описание", "установка", "инструкция", "документация", "справка", "руководство",
];

/// Расширения документации/прозы.
const DOC_EXTS: &[&str] = &["md", "markdown", "txt", "rst", "adoc", "org"];

/// Расширения исходного кода.
///
/// v0.83.0: +`s`,`asm` (`.S` приводится к нижнему регистру здесь же).
/// v0.84.0: +`hh`,`hxx`,`inc`,`inl` — современный C++ (Blender/LLVM):
/// заголовки `.hh`/`.hxx` и inline-фрагменты `.inc`/`.inl` классифицируются
/// как код (тир проза-интента честно понижается).
const CODE_EXTS: &[&str] = &[
    "rs", "py", "c", "h", "cpp", "hpp", "cc", "hh", "hxx", "cxx", "inc", "inl", "js",
    "jsx", "ts", "tsx", "java", "go", "kt", "kts", "swift", "cs", "scala", "dart",
    "rb", "php", "pl", "lua", "sh", "bash", "zsh", "fish", "m", "mm", "zig", "v",
    "sv", "vh", "sql", "r", "jl", "s", "asm",
];

/// Определяет намерение запроса по структуре (детерминированно, без ML).
pub fn detect_query_intent(query: &str) -> QueryIntent {
    // Кириллица — верный признак прозаического/нарративного запроса
    // (идентификаторы кода кириллицей — экзотика; документация — норма).
    if query.chars().any(is_cyrillic) {
        return QueryIntent::Prose;
    }
    // Прикреплённая скобка — синтаксис сигнатуры.
    if SignatureQuery::parse(query).is_some() {
        return QueryIntent::Code;
    }
    let words: Vec<String> = query
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return QueryIntent::Neutral;
    }
    if words.iter().any(|w| CODE_KEYWORDS.contains(&w.as_str())) {
        return QueryIntent::Code;
    }
    if words
        .iter()
        .any(|w| QUESTION_WORDS.contains(&w.as_str()) || DOC_HINT_WORDS.contains(&w.as_str()))
    {
        return QueryIntent::Prose;
    }
    // 3+ «простых» слова без подчёркиваний и CamelCase — проза.
    let plain = words.len() >= 3
        && words.iter().all(|w| {
            !w.contains('_')
                && w.chars()
                    .zip(w.chars().skip(1))
                    .all(|(a, b)| !(a.is_lowercase() && b.is_uppercase()))
        });
    if plain {
        return QueryIntent::Prose;
    }
    QueryIntent::Neutral
}

/// Классифицирует файл по расширению.
pub fn classify_path(path: &Path) -> FileClass {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    if DOC_EXTS.contains(&ext.as_str()) {
        FileClass::Doc
    } else if CODE_EXTS.contains(&ext.as_str()) {
        FileClass::Code
    } else {
        FileClass::Data
    }
}

/// Тир ранжирования хита (меньше = выше).
///
/// * 0 — класс соответствует намерению (или намерение нейтрально);
/// * 1 — класс конфликтует с намерением;
/// * 2 — сигнатурный запрос активен, но хит не совпал с паттерном.
///
/// Сигнатура доминирует: точное совпадение паттерна — тир 0 (даже в
/// документации: литеральное упоминание сигнатуры — легитимный ответ),
/// несовпадение — тир 2 независимо от класса.
pub fn hit_tier(intent: QueryIntent, signature_active: bool, signature_matched: bool, path: &Path) -> u8 {
    if signature_active {
        return if signature_matched { 0 } else { 2 };
    }
    match intent {
        QueryIntent::Neutral => 0,
        QueryIntent::Code => match classify_path(path) {
            FileClass::Doc => 1,
            _ => 0,
        },
        QueryIntent::Prose => match classify_path(path) {
            FileClass::Code => 1,
            _ => 0,
        },
    }
}

// ---------------------------------------------------------------------------
// Сигнатурный слой
// ---------------------------------------------------------------------------

/// Литеральный сигнатурный паттерн из запроса.
///
/// `func Default()` → префиксы [func], якорь `Default` (case-sensitive),
/// скобка `(`. Матчится байт-уровнево: слова через любые не-alnum
/// разделители (`::`, `->`, `.`, пробелы), якорь — точно по регистру,
/// скобка — сразу за якорем (после опционального whitespace).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureQuery {
    /// Слова до якоря (lowercase, case-insensitive матч).
    pub prefix_words: Vec<String>,
    /// Идентификатор перед скобкой — КАК НАПИСАН в запросе (case-sensitive).
    pub anchor_word: String,
    /// Открывающая скобка: `(` или `[`.
    pub bracket: u8,
}

impl SignatureQuery {
    /// Разбирает запрос, если тот содержит прикреплённую скобку:
    /// word-символ, за которым НЕПОСРЕДСТВЕННО следует `(` или `[`.
    ///
    /// `func Default()` → Some; `mass (in grams)` → None (скобка через
    /// пробел — прозовая приписка, не сигнатура); кириллица → None.
    pub fn parse(query: &str) -> Option<Self> {
        if query.chars().any(is_cyrillic) {
            return None;
        }
        let bytes = query.as_bytes();
        // первая прикреплённая скобка
        let mut attach: Option<usize> = None;
        for i in 1..bytes.len() {
            let prev = bytes[i - 1];
            let cur = bytes[i];
            if (cur == b'(' || cur == b'[')
                && (prev.is_ascii_alphanumeric() || prev == b'_')
            {
                attach = Some(i);
                break;
            }
        }
        let bracket_pos = attach?;
        // якорь — назад до начала слова
        let mut s = bracket_pos;
        while s > 0 && (bytes[s - 1].is_ascii_alphanumeric() || bytes[s - 1] == b'_') {
            s -= 1;
        }
        if s == bracket_pos || bracket_pos - s < 2 {
            return None; // пустой/односимвольный якорь — шум
        }
        let anchor_word = query[s..bracket_pos].to_string();
        // префиксные слова — из головы запроса
        let mut prefix_words = Vec::new();
        for tok in query[..s].split_whitespace() {
            let core: String = tok
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if core.is_empty() {
                return None; // мусорный токен — не сигнатура
            }
            prefix_words.push(core.to_lowercase());
        }
        if prefix_words.len() > 5 {
            return None; // сигнатуры короткие
        }
        Some(Self {
            prefix_words,
            anchor_word,
            bracket: bytes[bracket_pos],
        })
    }

    /// Проверяет хит на байтовой позиции файла.
    ///
    /// Хит всегда стоит на токене запроса (префиксном слове или якоре) —
    /// выравнивание паттерна начинается с этого слова:
    /// * якорь (case-sensitive) + скобка сразу после + префиксные слова
    ///   ЗА якорем (обратный ход; для `func Default(` call-site
    ///   `Default(http.MethodPost)` — без `func` перед ним — НЕ матч);
    /// * префиксное слово + остаток паттерна вперёд (слова через любые
    ///   не-alnum разделители, якорь по регистру, скобка в конце).
    pub fn matches_at(&self, text: &str, byte_pos: usize) -> bool {
        let bytes = text.as_bytes();
        if byte_pos >= bytes.len() {
            return false;
        }
        let word_end = byte_pos + word_len_at(bytes, byte_pos);
        if word_end == byte_pos {
            return false;
        }
        let word = &text[byte_pos..word_end];
        // Вариант A: токен хита — сам якорь (call-site / определение).
        if word == self.anchor_word {
            return bracket_after(bytes, word_end, self.bracket)
                && self.prefix_before(text, byte_pos);
        }
        // Вариант B: токен хита — одно из префиксных слов (ci).
        let lower = word.to_lowercase();
        let Some(k) = self.prefix_words.iter().position(|w| *w == lower) else {
            return false;
        };
        let mut cursor = word_end;
        for p in &self.prefix_words[k + 1..] {
            cursor = skip_non_word(bytes, cursor);
            let wlen = word_len_at(bytes, cursor);
            if wlen == 0 || text[cursor..cursor + wlen].to_lowercase() != *p {
                return false;
            }
            cursor += wlen;
        }
        cursor = skip_non_word(bytes, cursor);
        let wlen = word_len_at(bytes, cursor);
        if wlen == 0 || &text[cursor..cursor + wlen] != self.anchor_word {
            return false;
        }
        cursor += wlen;
        bracket_after(bytes, cursor, self.bracket)
    }

    /// Обратная проверка префиксных слов перед якорем (в порядке
    /// «последнее-перед-якорем … первое»), слова через любые не-alnum
    /// разделители — зеркально прямому ходу.
    fn prefix_before(&self, text: &str, anchor_start: usize) -> bool {
        if self.prefix_words.is_empty() {
            return true;
        }
        let bytes = text.as_bytes();
        let mut cursor = anchor_start;
        for p in self.prefix_words.iter().rev() {
            cursor = skip_non_word_back(bytes, cursor);
            if cursor == 0 {
                return false; // слово не уместилось
            }
            let wstart = word_start_back(bytes, cursor);
            if text[wstart..cursor].to_lowercase() != *p {
                return false;
            }
            cursor = wstart;
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Запрос-объявление типа (v0.85.0, ГРАБЛЯ 43)
// ---------------------------------------------------------------------------

/// Запрос-объявление типа: `class Name` / `struct Name` (+ хвост `:` `{` `;`).
///
/// ГРАБЛЯ 43 (LLVM, стресс-тест владельца): в определениях мировых C++
/// проектов между ключевым словом и именем типа стоит макрос экспорта
/// видимости — `class LLVM_ABI Function : public GlobalObject`,
/// `class Q_CORE_EXPORT QObject`, `struct PLATFORM_EXPORT BMVert`.
/// Токенайзер не рвёт `LLVM_ABI` (подчёркивание — словесный символ),
/// строгая фраза (код-интент) требует соседства токенов — определение
/// невидимо, выдачу забивают forward-декларации `class Function;`.
/// Запрос формы «класс/структура + имя» включает мягкий fallback:
/// между keyword и именем допускается гэп из макро-атрибутов
/// (валидация — в [`crate::streaming::FileTokens::find_type_decl_gaps`]).
///
/// В легальном C++ между `class`/`struct` и именем типа не может стоять
/// ничего, кроме атрибутов и макросов (грамматика: class-key
/// attribute-specifier-seq[opt] class-head-name) — поэтому гэп-токен
/// из заглавных/цифр/подчёркиваний почти наверняка макрос, ложных
/// срабатываний на переменные/elaborated-описатели нет (гэп там пуст).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDeclQuery {
    /// Ключевое слово (lowercase): `class` | `struct`.
    pub keyword: String,
    /// Имя типа КАК НАПИСАНО в запросе (case-sensitive — как якорь
    /// [`SignatureQuery`]: идентификаторы C++ чувствительны к регистру).
    pub name_word: String,
    /// v0.86.0 (ГРАБЛЯ 45, Boost.Spirit): запрос с template-префиксом —
    /// `template struct phrase_parse` / `template class Parser`.
    /// В реальном C++ между `template` и class-key всегда стоит
    /// параметрический блок `<typename Iterator, …>` — строгая фраза
    /// слепа к нему (между токенами ~60 байт параметров).
    pub template: bool,
}

impl TypeDeclQuery {
    /// Разбирает запрос формы `class Name` / `struct Name` с необязательным
    /// пунктуационным хвостом из `:` `{` `;` (базовый класс / тело /
    /// декларация) и — v0.86.0 — трёхсловную форму
    /// `template struct Name` / `template class Name` (ГРАБЛЯ 45).
    ///
    /// Отказ (None):
    /// * прикреплённая скобка — это сигнатурный слой ([`SignatureQuery`]);
    /// * кириллица;
    /// * слов ≠ 2 (и ≠ 3 для template-формы): `enum class Color`,
    ///   `class LLVM_ABI Function` (юзер сам вписал макрос — точный путь),
    ///   `class Foo usage`;
    /// * первое слово не `class`/`struct` (для template-формы — второе);
    /// * имя короче 2 символов или начинается с цифры (шум).
    pub fn parse(query: &str) -> Option<Self> {
        if query.chars().any(is_cyrillic) {
            return None;
        }
        if SignatureQuery::parse(query).is_some() {
            return None; // прикреплённая скобка — сигнатурный слой
        }
        let mut words: Vec<String> = Vec::new();
        for tok in query.split_whitespace() {
            let core: String = tok
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if core.is_empty() {
                // пунктуационный хвост: допустимы только : { ;
                if !tok.chars().all(|c| matches!(c, ':' | '{' | ';')) {
                    return None;
                }
                continue;
            }
            if core.len() > 64 {
                return None; // мусорный «токен» — не объявление типа
            }
            words.push(core);
        }
        // v0.86.0: template-форма — 3 слова, class-key на второй позиции.
        // Параметрический блок `<…>` юзер НЕ вписывает (токенизация запроса
        // его бы разорвала) — движок восстанавливает его в файле сам.
        if words.len() == 3 && words[0].eq_ignore_ascii_case("template") {
            let keyword = words[1].to_lowercase();
            if keyword != "class" && keyword != "struct" {
                return None;
            }
            let name_word = words[2].clone();
            if name_word.len() < 2
                || name_word
                    .chars()
                    .next()
                    .map_or(true, |c| c.is_ascii_digit())
            {
                return None; // односимвольное/цифровое имя — шум
            }
            return Some(Self { keyword, name_word, template: true });
        }
        if words.len() != 2 {
            return None;
        }
        let keyword = words[0].to_lowercase();
        if keyword != "class" && keyword != "struct" {
            return None;
        }
        let name_word = words[1].clone();
        if name_word.len() < 2
            || name_word
                .chars()
                .next()
                .map_or(true, |c| c.is_ascii_digit())
        {
            return None; // односимвольное/цифровое имя — шум
        }
        Some(Self { keyword, name_word, template: false })
    }
}

#[inline]
fn is_word_b(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Длина [A-Za-z0-9_]-слова, начинающегося в `i`.
fn word_len_at(bytes: &[u8], i: usize) -> usize {
    let mut l = 0usize;
    while i + l < bytes.len() && is_word_b(bytes[i + l]) {
        l += 1;
    }
    l
}

/// Пропускает любые не-словесные байты (пробелы, `:`, `.`, `-`, `>`…).
fn skip_non_word(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && !is_word_b(bytes[i]) {
        i += 1;
    }
    i
}

/// Обратный пропуск не-словесных байтов от позиции (эксклюзивно влево).
fn skip_non_word_back(bytes: &[u8], mut i: usize) -> usize {
    while i > 0 && !is_word_b(bytes[i - 1]) {
        i -= 1;
    }
    i
}

/// Начало [A-Za-z0-9_]-слова, заканчивающегося в `i` (эксклюзивно).
fn word_start_back(bytes: &[u8], mut i: usize) -> usize {
    while i > 0 && is_word_b(bytes[i - 1]) {
        i -= 1;
    }
    i
}

/// Скобка сразу после якоря (между ними — только whitespace).
fn bracket_after(bytes: &[u8], mut i: usize, bracket: u8) -> bool {
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') {
        i += 1;
    }
    i < bytes.len() && bytes[i] == bracket
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- detect_query_intent ----------

    #[test]
    fn code_keyword_is_code_intent() {
        assert_eq!(detect_query_intent("func Default"), QueryIntent::Code);
        assert_eq!(detect_query_intent("impl Engine for"), QueryIntent::Code);
        assert_eq!(detect_query_intent("def main"), QueryIntent::Code);
        assert_eq!(detect_query_intent("pub fn new"), QueryIntent::Code);
    }

    #[test]
    fn attached_bracket_is_code_intent() {
        assert_eq!(detect_query_intent("express()"), QueryIntent::Code);
        assert_eq!(detect_query_intent("func Default()"), QueryIntent::Code);
        assert_eq!(detect_query_intent("main["), QueryIntent::Code);
    }

    #[test]
    fn cyrillic_is_prose_intent() {
        assert_eq!(detect_query_intent("как работает резонанс"), QueryIntent::Prose);
        assert_eq!(detect_query_intent("Нокс"), QueryIntent::Prose);
    }

    #[test]
    fn question_and_doc_hints_are_prose() {
        assert_eq!(detect_query_intent("how to install"), QueryIntent::Prose);
        assert_eq!(detect_query_intent("readme"), QueryIntent::Prose);
        assert_eq!(detect_query_intent("architecture"), QueryIntent::Prose);
    }

    #[test]
    fn plain_three_words_are_prose() {
        assert_eq!(detect_query_intent("gin web framework"), QueryIntent::Prose);
    }

    #[test]
    fn identifiers_are_neutral() {
        // одиночный идентификатор — ни код, ни проза: без тиров
        assert_eq!(detect_query_intent("compress_scope"), QueryIntent::Neutral);
        assert_eq!(detect_query_intent("Default"), QueryIntent::Neutral);
        // camelCase-пара — идентификаторный поиск
        assert_eq!(detect_query_intent("compressScope"), QueryIntent::Neutral);
    }

    #[test]
    fn space_paren_is_not_code_signature() {
        // скобка через пробел — прозаическая приписка, НЕ сигнатура:
        // три простых слова → прозовый интент, но сигнатурный слой не активен
        assert_eq!(detect_query_intent("mass (in grams)"), QueryIntent::Prose);
        assert!(SignatureQuery::parse("mass (in grams)").is_none());
    }

    // ---------- classify_path ----------

    #[test]
    fn classify_extensions() {
        assert_eq!(classify_path(Path::new("/a/b/doc.md")), FileClass::Doc);
        assert_eq!(classify_path(Path::new("/a/b/README.MD")), FileClass::Doc);
        assert_eq!(classify_path(Path::new("/a/b/notes.txt")), FileClass::Doc);
        assert_eq!(classify_path(Path::new("/a/b/gin.go")), FileClass::Code);
        assert_eq!(classify_path(Path::new("/a/b/lib.rs")), FileClass::Code);
        assert_eq!(classify_path(Path::new("/a/b/cfg.json")), FileClass::Data);
        assert_eq!(classify_path(Path::new("/a/b/Makefile")), FileClass::Data);
        // v0.84.0: современный C++ (Blender/LLVM) — .hh/.hxx/.cxx/.inc/.inl это код
        assert_eq!(classify_path(Path::new("/a/b/bmesh_class.hh")), FileClass::Code);
        assert_eq!(classify_path(Path::new("/a/b/dna_gen.hxx")), FileClass::Code);
        assert_eq!(classify_path(Path::new("/a/b/bmesh_ops.cxx")), FileClass::Code);
        assert_eq!(classify_path(Path::new("/a/b/inline_ops.inc")), FileClass::Code);
        assert_eq!(classify_path(Path::new("/a/b/vec_inl.inl")), FileClass::Code);
    }

    // ---------- hit_tier ----------

    #[test]
    fn tiers_intent_gated() {
        let go = Path::new("/repo/gin.go");
        let md = Path::new("/repo/doc.md");
        // код-запрос: доки → тир 1, код → 0
        assert_eq!(hit_tier(QueryIntent::Code, false, false, go), 0);
        assert_eq!(hit_tier(QueryIntent::Code, false, false, md), 1);
        // проза-запрос: код → тир 1, доки → 0
        assert_eq!(hit_tier(QueryIntent::Prose, false, false, go), 1);
        assert_eq!(hit_tier(QueryIntent::Prose, false, false, md), 0);
        // нейтрально — без тиров
        assert_eq!(hit_tier(QueryIntent::Neutral, false, false, md), 0);
        // сигнатура доминирует над классом
        assert_eq!(hit_tier(QueryIntent::Code, true, true, md), 0);
        assert_eq!(hit_tier(QueryIntent::Code, true, false, go), 2);
    }

    // ---------- SignatureQuery::parse ----------

    #[test]
    fn signature_parse_variants() {
        let s = SignatureQuery::parse("func Default()").unwrap();
        assert_eq!(s.prefix_words, vec!["func"]);
        assert_eq!(s.anchor_word, "Default");
        assert_eq!(s.bracket, b'(');

        let s = SignatureQuery::parse("express()").unwrap();
        assert!(s.prefix_words.is_empty());
        assert_eq!(s.anchor_word, "express");

        // разделители в префиксе: двоеточия/звёздочки срезаются в ядра
        let s = SignatureQuery::parse("func (e *Engine) Default(").unwrap();
        assert_eq!(s.prefix_words, vec!["func", "e", "engine"]);
        assert_eq!(s.anchor_word, "Default");

        // мусорный токен → не сигнатура
        assert!(SignatureQuery::parse("?? Default(").is_none());
        // однобуквенный якорь → шум
        assert!(SignatureQuery::parse("func f(").is_none());
        // кириллица → не сигнатура
        assert!(SignatureQuery::parse("функция Default(").is_none());
        // слишком длинный префикс
        assert!(SignatureQuery::parse("a b c d e f g(").is_none());
    }

    // ---------- SignatureQuery::matches_at ----------

    #[test]
    fn signature_matches_real_definition() {
        let s = SignatureQuery::parse("func Default()").unwrap();
        let text = "// Default returns an Engine\nfunc Default() *Engine {";
        let pos = text.find("func Default").unwrap();
        assert!(s.matches_at(text, pos));
    }

    #[test]
    fn signature_rejects_struct_tag_noise() {
        // gin-кейс: form:",default=1" — токен default БЕЗ func перед ним
        let s = SignatureQuery::parse("func Default()").unwrap();
        let text = "Name string `form:\"name\" binding:\"required\"`";
        let pos = text.find("form").unwrap();
        assert!(!s.matches_at(text, pos));
        // и якорь без скобки не матчится
        let text2 = "func Default usage example";
        let pos2 = text2.find("func").unwrap();
        assert!(!s.matches_at(text2, pos2));
    }

    #[test]
    fn signature_go_receiver_method() {
        // приёмник Go-метода между func и именем — не-alnum разделители
        let s = SignatureQuery::parse("func (e *Engine) Default(").unwrap();
        let text = "func (e *Engine) Default() *Engine {";
        let pos = text.find("func").unwrap();
        assert!(s.matches_at(text, pos));
    }

    #[test]
    fn signature_path_separators() {
        // fs::path( — префикс через ::
        let s = SignatureQuery::parse("fs::path(").unwrap();
        assert_eq!(s.prefix_words, vec!["fs"]);
        assert_eq!(s.anchor_word, "path");
        let text = "auto p = fs::path(\"/tmp\");";
        let pos = text.find("fs").unwrap();
        assert!(s.matches_at(text, pos));
        // ptr->foo( — префикс через ->
        let s2 = SignatureQuery::parse("ptr->foo(").unwrap();
        assert_eq!(s2.prefix_words, vec!["ptr"]);
        assert_eq!(s2.anchor_word, "foo");
        let text2 = "x = ptr->foo(1);";
        let pos2 = text2.find("ptr").unwrap();
        assert!(s2.matches_at(text2, pos2));
        // ЧЕСТНОЕ ОГРАНИЧЕНИЕ (документировано): C++-шаблоны — якорь и
        // скобка обязаны стоять рядом (после ws); `vector(` НЕ матчится
        // на `vector<int>(` — литеральная семантика без угадывания
        let s3 = SignatureQuery::parse("std::vector(").unwrap();
        let text3 = "auto v = std::vector<int>();";
        let pos3 = text3.find("std").unwrap();
        assert!(!s3.matches_at(text3, pos3));
    }

    #[test]
    fn signature_case_sensitivity_on_anchor() {
        // якорь чувствителен к регистру: Default ≠ default
        let s = SignatureQuery::parse("func Default()").unwrap();
        let text = "func default() int {";
        let pos = text.find("func").unwrap();
        assert!(!s.matches_at(text, pos));
        // а префиксные слова — нет
        let s2 = SignatureQuery::parse("FUNC Default()").unwrap();
        assert!(s2.matches_at("func Default() *Engine {", 0));
    }

    #[test]
    fn signature_call_site_without_prefix() {
        let s = SignatureQuery::parse("express()").unwrap();
        let text = "const app = express();";
        let pos = text.find("express").unwrap();
        assert!(s.matches_at(text, pos));
        // упоминание без вызова — не матч
        let text2 = "express is a framework";
        let pos2 = text2.find("express").unwrap();
        assert!(!s.matches_at(text2, pos2));
    }

    #[test]
    fn signature_call_site_requires_prefix_backwards() {
        // gin-кейс: `Default(http.MethodPost)` — вызов БЕЗ `func` перед
        // ним не матчится на запрос `func Default(` (требуется определение)
        let s = SignatureQuery::parse("func Default()").unwrap();
        let call = "assert.Equal(t, YAML, Default(http.MethodPost, MIMEYAML))";
        let pos = call.find("Default").unwrap();
        assert!(!s.matches_at(call, pos), "call-site без func не должен матчиться");
        // определение — матчится
        let def = "func Default(opts ...OptionFunc) *Engine {";
        let dpos = def.find("Default").unwrap();
        assert!(s.matches_at(def, dpos));
        // обратный ход через приёмник Go-метода: (e *Engine) Default(
        let s2 = SignatureQuery::parse("func (e *Engine) Default(").unwrap();
        let m = "func (e *Engine) Default() *Engine {";
        let mpos = m.find("Default").unwrap();
        assert!(s2.matches_at(m, mpos));
    }

    #[test]
    fn signature_bracket_index() {
        let s = SignatureQuery::parse("vec[").unwrap();
        assert_eq!(s.bracket, b'[');
        let text = "let x = vec[0];";
        let pos = text.find("vec").unwrap();
        assert!(s.matches_at(text, pos));
    }

    // ---------- TypeDeclQuery::parse (v0.85.0, ГРАБЛЯ 43) ----------

    #[test]
    fn type_decl_parse_variants() {
        // канонические формы запроса-объявления (+ хвост : { ;)
        let td = TypeDeclQuery::parse("class Function :").unwrap();
        assert_eq!(td.keyword, "class");
        assert_eq!(td.name_word, "Function");
        assert!(TypeDeclQuery::parse("struct BMVert {").is_some());
        assert!(TypeDeclQuery::parse("class Function").is_some());
        assert!(TypeDeclQuery::parse("class Function;").is_some());
        assert!(TypeDeclQuery::parse("struct Function{").is_some());
        // РЕГРЕССИЯ Blender v0.84.0: определение без макроса обязано
        // идти точным путём — а значит, парс запроса должен работать
    }

    #[test]
    fn type_decl_parse_rejects() {
        // юзер сам вписал макрос — 3 слова, точный путь (LLVM-кейс
        // владельца: `class LLVM_ABI Function` уже нашёл [1/1])
        assert!(TypeDeclQuery::parse("class LLVM_ABI Function").is_none());
        // enum class — 3 слова, не наш домен
        assert!(TypeDeclQuery::parse("enum class Color :").is_none());
        // прикреплённая скобка — сигнатурный слой
        assert!(TypeDeclQuery::parse("class Function(").is_none());
        assert!(TypeDeclQuery::parse("struct Function[").is_none());
        // кириллица
        assert!(TypeDeclQuery::parse("class Функція :").is_none());
        // не class/struct
        assert!(TypeDeclQuery::parse("impl Engine for").is_none());
        assert!(TypeDeclQuery::parse("fn parse").is_none());
        // 3+ слова / прозаический хвост
        assert!(TypeDeclQuery::parse("class Function usage").is_none());
        assert!(TypeDeclQuery::parse("class Function (deprecated)").is_none());
        // мусорный хвост
        assert!(TypeDeclQuery::parse("class Function ???").is_none());
        // однобуквенное имя — шум
        assert!(TypeDeclQuery::parse("class F").is_none());
        // пусто
        assert!(TypeDeclQuery::parse("").is_none());
    }

    // ---------- v0.86.0: template-форма TypeDeclQuery (ГРАБЛЯ 45) ----------

    #[test]
    fn type_decl_parse_template_forms() {
        // Boost.Spirit-кейс: `template struct phrase_parse`
        let td = TypeDeclQuery::parse("template struct phrase_parse").unwrap();
        assert_eq!(td.keyword, "struct");
        assert_eq!(td.name_word, "phrase_parse");
        assert!(td.template);
        let td = TypeDeclQuery::parse("template class Parser").unwrap();
        assert_eq!(td.keyword, "class");
        assert_eq!(td.name_word, "Parser");
        assert!(td.template);
        // хвостовая пунктуация допустима и в template-форме
        let td = TypeDeclQuery::parse("template struct phrase_parse {").unwrap();
        assert!(td.template);
        // регистр template-слова не важен (class-key строго lowercase-сравнивается)
        assert!(TypeDeclQuery::parse("TEMPLATE struct Foo").is_some());
        // отказы: 2 слова (template сам — не class-key), typename/fn на
        // второй позиции, цифровое имя
        assert!(TypeDeclQuery::parse("template struct").is_none());
        assert!(TypeDeclQuery::parse("template typename phrase_parse").is_none());
        assert!(TypeDeclQuery::parse("template fn parse").is_none());
        assert!(TypeDeclQuery::parse("template struct 9parse").is_none());
        assert!(TypeDeclQuery::parse("template struct p").is_none());
        // обычная форма — template=false (обратная совместимость)
        let td = TypeDeclQuery::parse("class Function :").unwrap();
        assert!(!td.template);
    }
}
