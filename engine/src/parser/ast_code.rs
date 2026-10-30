//! AST-lite извлечение enclosing scope для кода.
//!
//! Полноценный парсинг каждого языка неприемлем по скорости, а `grep`
//! вообще не понимает структуры. Компромисс — лексический сканер
//! с полноценной машинерией строк/комментарий:
//!
//! * **Brace-языки** (Rust/C/C++/JS/TS/Java/Go/…): прямой проход
//!   с подсчётом глубины `{}` валиден, пока сканер понимает
//!   строковые литералы, char-литералы (`'{'`!), raw-строки Rust
//!   (`r#"…"#`), line/block-комментарии и template-литералы JS
//!   c `${…}`-интерполяцией. Внутренняя незакрытая `{` перед
//!   совпадением даёт искомый скоуп, парный `}` закрывает его.
//! * **Python**: отступная модель — заголовок блока (`def`/`class`,
//!   строка с меньшим отступом, оканчивающаяся на `:`) и тело
//!   до строки с отступом <= заголовка.
//!
//! Результат — полный логический блок (функция/класс целиком),
//! а не изолированная строка, как у grep.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::LazyLock;

/// Языковая модель файла.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeLang {
    /// Фигурные скобки: Rust, C, C++, JS, TS, Java, Go, …
    Brace,
    /// Значимые отступы: Python.
    Python,
    /// Не код — использовать текстовые парсеры.
    Plain,
}

/// Определение языка по расширению файла.
pub fn detect_lang(path: &Path) -> CodeLang {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "py" | "pyw" | "pyi" => CodeLang::Python,
        // v0.84.0: +`hxx`,`cxx`,`inc`,`inl` — современный C++ (Blender/LLVM);
        // `.hh` был и раньше, остальные inline-морфологии добираем
        "rs" | "c" | "h" | "cpp" | "hpp" | "cc" | "hh" | "hxx" | "cxx" | "inc" | "inl"
        | "js" | "jsx" | "ts" | "tsx"
        | "java" | "go" | "kt" | "kts" | "swift" | "cs" | "scala" | "dart" | "m" | "mm"
        | "json" | "css" | "scss" | "groovy" => CodeLang::Brace,
        _ => CodeLang::Plain,
    }
}

/// Выделенный логический блок кода.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeScope {
    /// "function" | "class" | "block" | "file"
    pub scope_type: String,
    /// Имя функции/класса/модуля, если распознано.
    pub name: Option<String>,
    /// Номер начальной строки (1-based).
    pub start_line: usize,
    /// Номер конечной строки (1-based, включительно).
    pub end_line: usize,
    /// Полный текст скоупа.
    pub text: String,
}

static SIGNATURE_RE: LazyLock<Regex> = LazyLock::new(|| {
    // v0.86.0 (ГРАБЛЯ 45, Boost.Spirit): необязательный template-префикс
    // `template <…>` перед class-key. Параметрический блок — всё до `>`
    // в пределах СТРОКИ без `;`/`{`/`}` (незакрытый блок = префикс не
    // матчится, а `struct Foo` на следующей строке ловится прежней
    // продукцией — многострочные объявления не теряются).
    // Прежде без префикса `template <typename T> struct Foo {` был
    // НЕВИДИМ символьному слою: ключевое слово struct не в начале строки.
    Regex::new(
        r#"^\s*(?:template\s*<[^;{}\n]*>\s*)?(?:pub(?:\([^)]*\))?\s+)?(?:pub\(crate\)\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+\"[^\"]*\"\s+)?(?:const\s+)?(?:static\s+)?(fn|def|function|func|class|struct|impl|trait|enum|interface|mod|module|type)\s+([A-Za-z_][A-Za-z0-9_]*)"#,
    )
    .unwrap()
});

/// Байтовые спаны строковых литералов и комментариев (для фильтрации
/// ложных вызовов в AIDDE): `[(start, end)]` в порядке следования.
pub fn string_comment_spans(source: &str) -> Vec<(usize, usize)> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mode {
        Normal,
        LineComment,
        BlockComment,
        Str(u8),
        RawStr(u8),
        Template,
    }
    let b = source.as_bytes();
    let mut spans = Vec::new();
    let mut mode = Mode::Normal;
    let mut span_start = 0usize;
    let mut i = 0usize;

    while i < b.len() {
        match mode {
            Mode::Normal => match b[i] {
                b'/' if i + 1 < b.len() && b[i + 1] == b'/' => {
                    span_start = i;
                    mode = Mode::LineComment;
                    i += 2;
                }
                b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                    span_start = i;
                    mode = Mode::BlockComment;
                    i += 2;
                }
                b'"' => {
                    let (is_raw, hashes) = raw_string_prefix(b, i);
                    span_start = i;
                    mode = if is_raw {
                        Mode::RawStr(hashes)
                    } else {
                        Mode::Str(b'"')
                    };
                    i += 1;
                }
                b'\'' => {
                    // char-литерал или lifetime: пропускаем, lifetime не строка
                    i = skip_char_or_lifetime(b, i);
                }
                b'`' => {
                    span_start = i;
                    mode = Mode::Template;
                    i += 1;
                }
                _ => i += 1,
            },
            Mode::LineComment => {
                if b[i] == b'\n' {
                    spans.push((span_start, i));
                    mode = Mode::Normal;
                }
                i += 1;
            }
            Mode::BlockComment => {
                if b[i] == b'*' && i + 1 < b.len() && b[i + 1] == b'/' {
                    spans.push((span_start, i + 2));
                    mode = Mode::Normal;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            Mode::Str(q) => match b[i] {
                b'\\' => i += 2,
                c if c == q => {
                    spans.push((span_start, i + 1));
                    mode = Mode::Normal;
                    i += 1;
                }
                _ => i += 1,
            },
            Mode::RawStr(hashes) => {
                if b[i] == b'"' {
                    let mut h = hashes;
                    let mut j = i + 1;
                    while h > 0 && j < b.len() && b[j] == b'#' {
                        h -= 1;
                        j += 1;
                    }
                    if h == 0 {
                        spans.push((span_start, j));
                        mode = Mode::Normal;
                        i = j;
                        continue;
                    }
                }
                i += 1;
            }
            Mode::Template => match b[i] {
                b'\\' => i += 2,
                b'`' => {
                    spans.push((span_start, i + 1));
                    mode = Mode::Normal;
                    i += 1;
                }
                _ => i += 1,
            },
        }
    }
    match mode {
        Mode::Normal => {}
        _ => spans.push((span_start, b.len())),
    }
    spans
}

static C_SIGNATURE_RE: LazyLock<Regex> = LazyLock::new(|| {
    // C-определение: [модификаторы] тип имя(аргументы) { — без ключевого
    // слова (в отличие от fn/def/class). Управляющие конструкции и
    // прототипы (оканчивающиеся на ';') отфильтрованы.
    Regex::new(
        r"^[\s]*((?:static|inline|extern|asmlinkage|__visible|__init|__exit|__always_inline|noinline|__weak|__noreturn|const|volatile|unsigned|signed|struct|enum|union|typedef)\s+)*[A-Za-z_][A-Za-z0-9_\s\*]*?[\s\*]([A-Za-z_][A-Za-z0-9_]*)\s*\([^;{}]*\)\s*\{?\s*$",
    )
    .unwrap()
});

const C_CONTROL_KEYWORDS: &[&str] = &[
    "if", "for", "while", "switch", "return", "sizeof", "else", "do", "case",
];

/// Открывашка C-сигнатуры: `тип имя(` — строка НАЧИНАЕТ многстрочную
/// ANSI-сигнатуру (параметры на той же строке, хвост — ниже). Без
/// требования закрытия `)` и `{`: носитель имени может кончаться запятой
/// (`int git_foo(git_repository **out, const char *path,`).
static C_SIG_OPEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^[\s]*((?:static|inline|extern|asmlinkage|__visible|__init|__exit|__always_inline|noinline|__weak|__noreturn|const|volatile|unsigned|signed|struct|enum|union|typedef)\s+)*[A-Za-z_][A-Za-z0-9_\s\*]*?[\s\*]([A-Za-z_][A-Za-z0-9_]*)\s*\(",
    )
    .unwrap()
});

/// Строка-носитель имени многострочной C-сигнатуры.
fn is_c_signature_opener(line: &str) -> bool {
    if let Some(name) = C_SIG_OPEN_RE
        .captures(line)
        .and_then(|c| c.get(c.len() - 1))
        .map(|m| m.as_str())
    {
        return !name.is_empty() && !C_CONTROL_KEYWORDS.contains(&name);
    }
    false
}

/// Сигнатура C-функции одной строки: имя. Возвращает None для прототипов,
/// вызовов (с ';' на конце) и управляющих конструкций.
pub(crate) fn c_signature_name(line: &str) -> Option<String> {
    let t = line.trim_end();
    if t.ends_with(';') || t.ends_with(',') || t.is_empty() {
        return None;
    }
    let caps = C_SIGNATURE_RE.captures(line)?;
    let name = caps
        .get(caps.len() - 1)
        .map(|m| m.as_str())?
        .to_string();
    if name.is_empty() || C_CONTROL_KEYWORDS.contains(&name.as_str()) {
        return None;
    }
    Some(name)
}

/// Лёгкий lookup имени сигнатуры скоупа, начинающегося в `byte_begin`.
pub fn light_signature(source: &str, byte_begin: usize) -> Option<String> {
    find_signature(source, byte_begin).1
}

/// Сигнатура одной строки: (вид, имя) — для таблицы символов AIDDE.
/// Явные ключевo-языки (fn/def/class/…) — через SIGNATURE_RE;
/// C-стиль (тип имя(args)) — через C_SIGNATURE_RE.
pub(crate) fn signature_of(line: &str) -> Option<(String, String)> {
    if let Some(caps) = SIGNATURE_RE.captures(line) {
        return Some((
            caps.get(1)?.as_str().to_string(),
            caps.get(2)?.as_str().to_string(),
        ));
    }
    c_signature_name(line).map(|n| ("fn".to_string(), n))
}

/// Точка входа: возвращает enclosing scope для байтового смещения.
pub fn extract_enclosing_scope(source: &str, byte_offset: usize, lang: CodeLang) -> CodeScope {
    let off = byte_offset.min(source.len());
    match locate_scope(source, off, lang) {
        Some((begin, end)) => materialize_scope(source, begin, end),
        None => file_scope(source),
    }
}

/// Диапазон сигнатуры над открывающей скобкой (C-стиль): (старт байт,
/// конец байт — до строки со скобкой). Сигнатура — часть «функции
/// целиком»: без неё скоуп — тело без контракта. Поднимается от строки
/// со скобкой вверх ≤10 строк, пока строки «похожи на сигнатуру»; носитель
/// имени — самая верхняя строка, распознанная как полная сигнатура либо
/// открывашка многострочной (кончается на `(`). Покрывает оба стиля
/// расстановки скобки: K&R (`{` на собственной строке) и ANSI
/// (`const char *path) {` — скобка в конце последней строки параметров);
/// однострочная сигнатура не поднимается — она уже на строке со скобкой.
/// Управляющие конструкции (if/while/switch…) отфильтрованы контрольными
/// словами в `c_signature_name`.
fn signature_span(source: &str, open_line_start: usize) -> Option<(usize, usize)> {
    // Оба стиля поднимают многострочную сигнатуру: K&R (`{` на собственной
    // строке) и ANSI (`const char *path) {`). Однострочная сигнатура
    // (`int f(void) {`, `fn foo() {`) не поднимается: строка над ней — не
    // часть сигнатуры, parts остаются пустыми и скоуп начинается прямо
    // со строки со скобкой, где сигнатура уже есть.
    let mut parts: Vec<usize> = Vec::new(); // старты строк-частей сигнатуры
    let mut pos = open_line_start;
    for _ in 0..10 {
        if pos == 0 {
            break;
        }
        let prev_end = pos - 1; // позиция '\n'
        let prev_start = line_start(source, prev_end);
        let line = source[prev_start..].lines().next().unwrap_or("");
        if !is_signature_part(line) {
            break;
        }
        parts.push(prev_start);
        pos = prev_start;
    }
    // верхняя строка-носитель имени: полная сигнатура, `(`-открывашка
    // или ANSI-открывашка (параметры на строке имени, хвост ниже)
    for &start in parts.iter().rev() {
        let line = source[start..].lines().next().unwrap_or("");
        if c_signature_name(line).is_some()
            || line.trim_end().ends_with('(')
            || is_c_signature_opener(line)
        {
            return Some((start, open_line_start));
        }
    }
    None
}

/// Похоже ли строка на часть сигнатуры над `{`: аргумент/продолжение
/// (кончается на `(`, `,`, `)`) либо распознанная однострочная сигнатура.
fn is_signature_part(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty()
        || t.starts_with('}')
        || t.starts_with('#')
        || t.starts_with("//")
        || t.starts_with("/*")
        || t.starts_with('*')
        || t.ends_with(';')
        || t.ends_with('{')
    {
        return false;
    }
    t.ends_with('(') || t.ends_with(',') || t.ends_with(')') || c_signature_name(line).is_some()
}

/// Лёгкая локализация скоупа (без клонирования текста): байтовый диапазон
/// либо `None` для файлового уровня. Вызывается на каждое совпадение.
pub fn locate_scope(source: &str, byte_offset: usize, lang: CodeLang) -> Option<(usize, usize)> {
    let off = byte_offset.min(source.len());
    match lang {
        CodeLang::Python => python_bounds(source, off),
        CodeLang::Brace => brace_bounds(source, off),
        CodeLang::Plain => None,
    }
}

/// Материализация скоупа по границам (текст + сигнатура): один раз на
/// уникальный скоуп. Начало поднимается к строкам сигнатуры над `{`
/// (C-стиль) — «функция целиком» включает её контракт.
pub fn materialize_scope(source: &str, begin: usize, end: usize) -> CodeScope {
    let end = end.min(source.len()).max(begin);
    let brace_line_start = line_start(source, begin);
    let scope_begin = signature_span(source, brace_line_start)
        .map(|(s, _)| s)
        .unwrap_or(brace_line_start);
    let scope_text = source[scope_begin..end].trim().to_string();
    let (scope_type, name) = find_signature(source, brace_line_start);
    CodeScope {
        scope_type: scope_type.to_string(),
        name,
        start_line: line_of(source, scope_begin),
        end_line: line_of(source, end.saturating_sub(1).max(scope_begin)),
        text: scope_text,
    }
}

fn file_scope(source: &str) -> CodeScope {
    let lines = source.lines().count().max(1);
    CodeScope {
        scope_type: "file".to_string(),
        name: None,
        start_line: 1,
        end_line: lines,
        text: source.to_string(),
    }
}

fn line_of(source: &str, byte_pos: usize) -> usize {
    source[..byte_pos.min(source.len())].matches('\n').count() + 1
}

fn line_start(source: &str, byte_pos: usize) -> usize {
    source[..byte_pos.min(source.len())]
        .rfind('\n')
        .map(|i| i + 1)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Лексический сканер для brace-языков
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum LexMode {
    Normal,
    LineComment,
    BlockComment,
    /// Обычная строка с экранированием; u8 — кавычка.
    Str(u8),
    /// Raw-строка Rust: u8 — количество решёток.
    RawStr(u8),
    /// Template-литерал JS (обратная кавычка).
    Template,
}

/// Проверяет, является ли кавычка на `quote_idx` началом Rust raw-строки.
/// Возвращает (является, количество решёток).
fn raw_string_prefix(b: &[u8], quote_idx: usize) -> (bool, u8) {
    let mut j = quote_idx;
    let mut hashes = 0u8;
    while j > 0 && b[j - 1] == b'#' {
        hashes += 1;
        j -= 1;
    }
    if j > 0 && b[j - 1] == b'r' {
        // граница идентификатора: перед 'r' не должно быть буквы (кроме 'b' от br"")
        let boundary_ok = j < 2 || !(b[j - 2].is_ascii_alphanumeric() && b[j - 2] != b'b');
        if boundary_ok {
            return (true, hashes);
        }
    }
    (false, 0)
}

/// Обрабатывает `'` в Rust: char-литерал (`'{'`, `'\n'`, `'ф'`)
/// или lifetime (`'a`, `'static`); возвращает новую позицию.
fn skip_char_or_lifetime(b: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    if j >= b.len() {
        return j;
    }
    // экранированный char-литерал
    if b[j] == b'\\' {
        j += 2;
        while j < b.len() && b[j] != b'\'' && b[j] != b'\n' {
            j += 1;
        }
        return if j < b.len() && b[j] == b'\'' { j + 1 } else { j };
    }
    // lifetime: идентификатор после ' без закрывающей кавычки рядом
    if b[j].is_ascii_alphabetic() || b[j] == b'_' {
        let mut k = j;
        while k < b.len() && (b[k].is_ascii_alphanumeric() || b[k] == b'_') {
            k += 1;
        }
        // `'a` — одиночный char-литерал (закрывающая кавычка сразу)
        if k < b.len() && b[k] == b'\'' && k == j + 1 {
            return k + 1;
        }
        return k; // lifetime — просто пропускаем
    }
    // char-литерал из небуквенного символа (возможно multibyte)
    let mut k = j;
    let mut steps = 0;
    while k < b.len() && steps < 5 && b[k] != b'\'' && b[k] != b'\n' {
        k += 1;
        steps += 1;
    }
    if k < b.len() && b[k] == b'\'' {
        k + 1
    } else {
        k
    }
}

/// Универсальный прямой сканер.
///
/// * `stop_at` — остановиться на байтовой позиции (фаза 1: собрать стек
///   открытых `{` до смещения совпадения);
/// * `stop_on_close` — остановиться, когда закрывающая `}` возвращает
///   глубину в минус (фаза 2: найти парную скобку; стек должен содержать
///   ровно открывающую позицию).
///
/// Возвращает позицию остановки.
fn lex_scan(
    source: &str,
    start: usize,
    stop_at: Option<usize>,
    stack: &mut Vec<usize>,
    stop_on_close: bool,
) -> usize {
    lex_scan_state(source, start, LexMode::Normal, stop_at, stack, stop_on_close).0
}

/// v0.86.0 (ГРАБЛЯ 44): stateful-вариант [`lex_scan`] — возвращает режим
/// лексера на позиции остановки (для инкрементального [`ScopeLocator`]).
fn lex_scan_state(
    source: &str,
    start: usize,
    mut mode: LexMode,
    stop_at: Option<usize>,
    stack: &mut Vec<usize>,
    stop_on_close: bool,
) -> (usize, LexMode) {
    let b = source.as_bytes();
    let mut i = start;

    while i < b.len() {
        if let Some(sa) = stop_at {
            if i >= sa {
                return (i, mode);
            }
        }
        match mode {
            LexMode::Normal => match b[i] {
                b'/' if i + 1 < b.len() && b[i + 1] == b'/' => {
                    mode = LexMode::LineComment;
                    i += 2;
                }
                b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                    mode = LexMode::BlockComment;
                    i += 2;
                }
                b'"' => {
                    let (is_raw, hashes) = raw_string_prefix(b, i);
                    if is_raw {
                        mode = LexMode::RawStr(hashes);
                        i += 1;
                    } else {
                        mode = LexMode::Str(b'"');
                        i += 1;
                    }
                }
                b'\'' => {
                    i = skip_char_or_lifetime(b, i);
                }
                b'`' => {
                    mode = LexMode::Template;
                    i += 1;
                }
                b'{' => {
                    stack.push(i);
                    i += 1;
                }
                b'}' => {
                    stack.pop();
                    if stop_on_close && stack.is_empty() {
                        return (i + 1, mode);
                    }
                    i += 1;
                }
                _ => i += 1,
            },
            LexMode::LineComment => {
                if b[i] == b'\n' {
                    mode = LexMode::Normal;
                }
                i += 1;
            }
            LexMode::BlockComment => {
                if b[i] == b'*' && i + 1 < b.len() && b[i + 1] == b'/' {
                    mode = LexMode::Normal;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            LexMode::Str(q) => match b[i] {
                b'\\' => i += 2,
                c if c == q => {
                    mode = LexMode::Normal;
                    i += 1;
                }
                b'\n' => {
                    mode = LexMode::Normal; // незакрытая строка — восстановление
                    i += 1;
                }
                _ => i += 1,
            },
            LexMode::RawStr(hashes) => {
                if b[i] == b'"' {
                    let mut h = hashes;
                    let mut j = i + 1;
                    while h > 0 && j < b.len() && b[j] == b'#' {
                        h -= 1;
                        j += 1;
                    }
                    if h == 0 {
                        mode = LexMode::Normal;
                        i = j;
                        continue;
                    }
                }
                i += 1;
            }
            LexMode::Template => match b[i] {
                b'\\' => i += 2,
                b'`' => {
                    mode = LexMode::Normal;
                    i += 1;
                }
                b'$' if i + 1 < b.len() && b[i + 1] == b'{' => {
                    stack.push(i); // ${ — открывает блок интерполяции
                    mode = LexMode::Normal;
                    i += 2;
                }
                _ => i += 1,
            },
        }
    }
    (b.len(), mode)
}

/// v0.86.0 (ГРАБЛЯ 44, продолжение — вторая квадратичная точка):
/// инкрементальный локатор скоупов для brace-языков.
///
/// `brace_bounds` сканирует файл от байта 0 на КАЖДЫЙ запрос (фаза 1
/// собирает стек `{` до смещения) — на монолите 250k строк с ~100k
/// вызовов это Σ(off) ≈ 10¹⁰ байт-шагов лексера: O(N²) по CPU.
/// Локатор сохраняет состояние лексера (позиция + режим + стек `{`)
/// между запросами: запросы по возрастанию смещений сканируют файл
/// РОВНО ОДИН РАЗ — суммарно O(N). Обратный запрос (смещение меньше
/// отсканированного) честно ресканирует с нуля — семантика идентична
/// [`brace_bounds`], порядок вызовов из `captures_iter` всегда прямой.
///
/// Фаза 2 (поиск парной `}` от открывающей скобки) локальна — одна
/// функция, не кэшируется.
pub struct ScopeLocator<'a> {
    source: &'a str,
    /// Позиция, до которой лексер дошёл; `(scanned, mode, stack)` —
    /// согласованное состояние на ЭТОЙ позиции.
    scanned: usize,
    mode: LexMode,
    stack: Vec<usize>,
}

impl<'a> ScopeLocator<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            scanned: 0,
            mode: LexMode::Normal,
            stack: Vec::new(),
        }
    }

    /// Локализация enclosing-скоупа (семантика [`brace_bounds`]).
    pub fn locate(&mut self, off: usize) -> Option<(usize, usize)> {
        let source = self.source;
        let off = off.min(source.len());
        // фаза 1: досканировать до off (прямой порядок — инкремент)
        if off < self.scanned {
            let mut stack = Vec::new();
            let (p, m) =
                lex_scan_state(source, 0, LexMode::Normal, Some(off), &mut stack, false);
            self.scanned = p;
            self.mode = m;
            self.stack = stack;
        } else {
            let (p, m) = lex_scan_state(
                source,
                self.scanned,
                self.mode,
                Some(off),
                &mut self.stack,
                false,
            );
            self.scanned = p;
            self.mode = m;
        }

        // совпадение в строке сигнатуры (до '{'): дословно brace_bounds
        let open_pos = match self.stack.last() {
            Some(&p) => p,
            None => {
                let line_end = source[off..]
                    .find('\n')
                    .map(|i| off + i)
                    .unwrap_or(source.len());
                let mut probe: Vec<usize> = Vec::new();
                lex_scan(source, off, Some(line_end), &mut probe, false);
                *probe.last()?
            }
        };

        // фаза 2: парная закрывающая скобка от open_pos (локально)
        let mut close_stack: Vec<usize> = Vec::new();
        let close_end = lex_scan(source, open_pos, None, &mut close_stack, true);

        Some((line_start(source, open_pos), close_end))
    }
}

fn brace_bounds(source: &str, off: usize) -> Option<(usize, usize)> {
    // Фаза 1: стек открытых '{' на момент смещения.
    let mut stack: Vec<usize> = Vec::new();
    lex_scan(source, 0, Some(off), &mut stack, false);

    // Совпадение в строке сигнатуры (до '{'): если открывающая скобка
    // блока находится на той же строке — совпадение принадлежит этому блоку.
    let open_pos = match stack.last() {
        Some(&p) => p,
        None => {
            let line_end = source[off..]
                .find('\n')
                .map(|i| off + i)
                .unwrap_or(source.len());
            let mut probe: Vec<usize> = Vec::new();
            lex_scan(source, off, Some(line_end), &mut probe, false);
            *probe.last()?
        }
    };

    // Фаза 2: парная закрывающая скобка от open_pos.
    let mut close_stack: Vec<usize> = Vec::new();
    let close_end = lex_scan(source, open_pos, None, &mut close_stack, true);

    Some((line_start(source, open_pos), close_end))
}

/// Ищет сигнатуру (fn/class/…) в строке с открывающей скобкой и до 10 строк выше.
fn find_signature(source: &str, open_pos: usize) -> (&'static str, Option<String>) {
    let open_line_start = line_start(source, open_pos);
    let mut candidates: Vec<&str> = Vec::new();

    // строка, содержащая открывающую скобку (однострочная сигнатура)
    if let Some(l) = source[open_line_start..].lines().next() {
        candidates.push(l);
    }

    // строки выше (многострочные сигнатуры/модификаторы)
    let mut pos = open_line_start;
    for _ in 0..10 {
        if pos == 0 {
            break;
        }
        let prev_end = pos - 1; // позиция '\n'
        let prev_start = line_start(source, prev_end);
        if let Some(l) = source[prev_start..].lines().next() {
            let t = l.trim();
            if t.is_empty() || t.starts_with('}') {
                break; // пустая строка / конец предыдущего блока
            }
            candidates.push(l);
        }
        pos = prev_start;
    }

    for line in candidates {
        if let Some(caps) = SIGNATURE_RE.captures(line) {
            let kind = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let name = caps.get(2).map(|m| m.as_str().to_string());
            let ty = match kind {
                "fn" | "def" | "function" | "func" => "function",
                "class" | "struct" | "impl" | "trait" | "enum" | "interface" | "mod"
                | "module" | "type" => "class",
                _ => "block",
            };
            return (ty, name);
        }
    }
    // C-стиль: тип имя(аргументы) без ключевого слова fn/def — строки
    // сигнатуры над `{` склеиваются (многострочные сигнатуры). В клей
    // входит и сама строка со скобкой: ANSI-хвост (`*opts) {`) замыкает
    // список параметров — без него клей кончается запятой.
    if let Some((sig_start, _)) = signature_span(source, open_line_start) {
        let open_line_end = source[open_line_start..]
            .lines()
            .next()
            .map(|l| open_line_start + l.len())
            .unwrap_or(open_line_start);
        let joined: String = source[sig_start..open_line_end]
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" ");
        if let Some(name) = c_signature_name(&joined) {
            return ("function", Some(name));
        }
    }
    // Однострочная C-сигнатура на самой строке со скобкой
    // (`int git_open(void) {`): раньше молча давала block/None.
    if let Some(l) = source[open_line_start..].lines().next() {
        if let Some(name) = c_signature_name(l) {
            return ("function", Some(name));
        }
    }
    ("block", None)
}

// ---------------------------------------------------------------------------
// Python: отступная модель
// ---------------------------------------------------------------------------

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn python_bounds(source: &str, off: usize) -> Option<(usize, usize)> {
    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let mut starts = Vec::with_capacity(lines.len() + 1);
    starts.push(0usize);
    for l in &lines {
        let last = *starts.last().unwrap();
        starts.push(last + l.len() + 1);
    }

    let mut li = starts.partition_point(|&s| s <= off).saturating_sub(1);
    li = li.min(lines.len() - 1);

    let cur_indent = indent_of(lines[li]);
    let cur_trim = lines[li].trim();

    // Если совпадение на строке заголовка def/class — это и есть скоуп.
    let header_idx = if cur_trim.starts_with("def ")
        || cur_trim.starts_with("class ")
        || cur_trim.starts_with("async def ")
    {
        li
    } else {
        let mut found = None;
        let mut j = li as isize - 1;
        while j >= 0 {
            let l = lines[j as usize];
            let t = l.trim_end();
            let ind = indent_of(l);
            if !t.is_empty() && ind < cur_indent {
                if t.ends_with(':') {
                    found = Some(j as usize);
                }
                break; // первый же меньший отступ определяет блок
            }
            j -= 1;
        }
        found?
    };

    let header = lines[header_idx];
    let header_indent = indent_of(header);

    // Конец блока: первая непустая строка с отступом <= заголовка.
    let mut end_idx = lines.len() - 1;
    for (k, l) in lines.iter().enumerate().skip(header_idx + 1) {
        if l.trim().is_empty() {
            continue;
        }
        if indent_of(l) <= header_indent {
            end_idx = k.saturating_sub(1).max(header_idx);
            break;
        }
        end_idx = k;
    }
    while end_idx > header_idx && lines[end_idx].trim().is_empty() {
        end_idx -= 1;
    }

    // границы в байтах (последняя строка без завершающего \n включается)
    let end_byte = (starts[end_idx + 1]).min(source.len());
    Some((starts[header_idx], end_byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope_at(src: &str, needle: &str) -> CodeScope {
        let off = src.find(needle).expect("needle not found");
        extract_enclosing_scope(src, off, CodeLang::Brace)
    }

    #[test]
    fn rust_nested_functions() {
        let src = "fn outer() {\n    let x = 1;\n    fn inner() {\n        let y = 2;\n    }\n}\n";
        let sc = scope_at(src, "y =");
        assert_eq!(sc.name.as_deref(), Some("inner"));
        assert_eq!(sc.scope_type, "function");
        assert!(sc.text.contains("fn inner"));
        assert!(sc.text.contains("y = 2"));
        assert!(!sc.text.contains("x = 1"));
    }

    #[test]
    fn braces_inside_strings_are_ignored() {
        let src = "fn a() {\n    let s = \"{ not a brace\";\n    let x = 1;\n}\n";
        let sc = scope_at(src, "x =");
        assert_eq!(sc.name.as_deref(), Some("a"));
        assert!(sc.text.contains("let x"));
    }

    #[test]
    fn braces_inside_char_literals() {
        let src = "fn b() {\n    let c = '{';\n    let x = 2;\n}\n";
        let sc = scope_at(src, "x =");
        assert_eq!(sc.name.as_deref(), Some("b"));
    }

    #[test]
    fn braces_inside_comments() {
        let src = "fn c() {\n    // } закрывающая в комментарии\n    let x = 3;\n}\n";
        let sc = scope_at(src, "x =");
        assert_eq!(sc.name.as_deref(), Some("c"));
    }

    #[test]
    fn rust_raw_strings() {
        let src = "fn d() {\n    let r = r#\"{ { }\"#;\n    let x = 4;\n}\n";
        let sc = scope_at(src, "x =");
        assert_eq!(sc.name.as_deref(), Some("d"));
        assert!(sc.text.contains("x = 4"));
    }

    #[test]
    fn class_scope_struct() {
        let src = "struct Machine {\n    field: i32,\n}\n\nimpl Machine {\n    fn go(&self) {\n        let v = self.field;\n    }\n}\n";
        let sc = scope_at(src, "v =");
        assert_eq!(sc.name.as_deref(), Some("go"));
        // совпадение на поле структуры даёт class-скоуп
        let sc2 = scope_at(src, "field: i32");
        assert_eq!(sc2.name.as_deref(), Some("Machine"));
        assert_eq!(sc2.scope_type, "class");
    }

    #[test]
    fn outside_any_brace_is_file() {
        let src = "const A: i32 = 5;\nfn f() { let q = 1; }\n";
        let sc = scope_at(src, "A: i32");
        assert_eq!(sc.scope_type, "file");
    }

    #[test]
    fn js_template_literal() {
        let src = "function greet(name) {\n    const msg = `Hello ${name} { brace`;\n    return msg;\n}\n";
        let sc = scope_at(src, "return");
        assert_eq!(sc.name.as_deref(), Some("greet"));
    }

    #[test]
    fn python_inner_def_scope() {
        let src = "class A:\n    def outer(self):\n        x = 1\n        def inner():\n            y = 2\n        return inner\n";
        let off = src.find("y =").unwrap();
        let sc = extract_enclosing_scope(src, off, CodeLang::Python);
        assert_eq!(sc.name.as_deref(), Some("inner"));
        assert_eq!(sc.scope_type, "function");
        assert!(sc.text.contains("y = 2"));
        assert!(!sc.text.contains("x = 1"));
    }

    #[test]
    fn python_class_scope() {
        let src = "class Worker:\n    name = \"w\"\n\n    def run(self):\n        return 1\n";
        let off = src.find("name =").unwrap();
        let sc = extract_enclosing_scope(src, off, CodeLang::Python);
        assert_eq!(sc.name.as_deref(), Some("Worker"));
        assert_eq!(sc.scope_type, "class");
    }

    #[test]
    fn python_method_scope() {
        let src = "class Worker:\n    def run(self):\n        data = self.load()\n        return data\n";
        let off = src.find("data =").unwrap();
        let sc = extract_enclosing_scope(src, off, CodeLang::Python);
        assert_eq!(sc.name.as_deref(), Some("run"));
        assert!(sc.text.contains("def run"));
    }

    #[test]
    fn detect_lang_by_extension() {
        assert_eq!(detect_lang(Path::new("/a/b/main.rs")), CodeLang::Brace);
        assert_eq!(detect_lang(Path::new("/a/b/app.py")), CodeLang::Python);
        assert_eq!(detect_lang(Path::new("/a/b/chapter.md")), CodeLang::Plain);
        // v0.84.0: современный C++ (Blender/LLVM) — inline-морфологии
        // фигурно-скобочные, enclosing_scope работает как для .cpp
        assert_eq!(detect_lang(Path::new("/a/b/bmesh_class.hh")), CodeLang::Brace);
        assert_eq!(detect_lang(Path::new("/a/b/dna_gen.hxx")), CodeLang::Brace);
        assert_eq!(detect_lang(Path::new("/a/b/bmesh_ops.cxx")), CodeLang::Brace);
        assert_eq!(detect_lang(Path::new("/a/b/inline_ops.inc")), CodeLang::Brace);
        assert_eq!(detect_lang(Path::new("/a/b/vec_inl.inl")), CodeLang::Brace);
    }

    #[test]
    fn c_style_signature_is_part_of_scope() {
        // C-стиль: `{` на собственной строке — сигнатура (контракт функции)
        // входит в «функцию целиком», иначе скоуп — тело без контракта
        // (стресс-кейс libgit2: движок обязан подавать сигнатуру + фокус).
        let src = "/* doc comment */\nint git_repository_open(git_repository **out, const char *path, int flags)\n{\n    return open_it(out, path, flags);\n}\n";
        let sc = scope_at(src, "open_it");
        assert_eq!(sc.name.as_deref(), Some("git_repository_open"));
        assert!(
            sc.text.starts_with("int git_repository_open"),
            "сигнатура открывает скоуп: {}",
            sc.text
        );
        assert!(sc.text.contains("{"));
        assert!(sc.text.contains("open_it"));
        assert_eq!(sc.start_line, 2, "start_line на сигнатуре: {}", sc.start_line);
        // док-коммент НЕ втягивается (только строки сигнатуры)
        assert!(!sc.text.contains("doc comment"));
    }

    #[test]
    fn c_multiline_signature_fully_in_scope() {
        let src = "static int call_foo(\n    int a,\n    const char *b)\n{\n    return do_call(a, b);\n}\n";
        let sc = scope_at(src, "do_call");
        assert!(
            sc.text.starts_with("static int call_foo("),
            "многострочная сигнатура целиком: {}",
            sc.text
        );
        assert!(sc.text.contains("const char *b)"));
    }

    #[test]
    fn rust_same_line_signature_unchanged() {
        // Rust-стиль: сигнатура на строке со скобкой — поведение прежнее,
        // дублирования строк сигнатуры нет.
        let src = "fn alpha() {\n    let z = 9;\n}\n";
        let sc = scope_at(src, "z =");
        assert_eq!(sc.text.matches("fn alpha").count(), 1);
    }

    #[test]
    fn ansi_multiline_signature_lifted() {
        // ANSI-стиль: скобка в конце последней строки параметров —
        // многострочная сигнатура тоже поднимается в «функцию целиком"
        // (провал смоук-фикстуры v0.81.0: скоуп начинался со строки
        // параметров, имя функции оставалось вне скоупа).
        let src = "/* doc */\nint git_repository_init_ext(git_repository **repo_out, const char *path,\n        unsigned flags, const git_repository_init_options *opts) {\n    return init_it(out, path);\n}\n";
        let sc = scope_at(src, "init_it");
        assert_eq!(sc.name.as_deref(), Some("git_repository_init_ext"));
        assert_eq!(sc.scope_type, "function");
        assert!(
            sc.text.starts_with("int git_repository_init_ext("),
            "ANSI-сигнатура открывает скоуп: {}",
            sc.text
        );
        assert!(sc.text.contains("*opts) {"));
        assert_eq!(sc.start_line, 2, "start_line на носителе имени: {}", sc.start_line);
        // док-коммент не втягивается
        assert!(!sc.text.contains("doc"));
    }

    #[test]
    fn ansi_single_line_signature_unchanged() {
        // Однострочная ANSI-сигнатура (`int f(void) {`) не поднимается:
        // она уже на строке со скобкой; строка выше (конец прошлого тела)
        // не притягивается.
        let src = "int prev(void) { return 0; }\nint git_open(void) {\n    return do_open();\n}\n";
        let sc = scope_at(src, "do_open");
        assert_eq!(sc.name.as_deref(), Some("git_open"));
        assert!(
            sc.text.starts_with("int git_open(void) {"),
            "однострочная сигнатура без дублирования: {}",
            sc.text
        );
        assert!(!sc.text.contains("prev"));
    }

    #[test]
    fn template_signature_visible() {
        // v0.86.0 (ГРАБЛЯ 45, Boost.Spirit): template-префикс больше не
        // прячет объявление от символьного слоя
        assert_eq!(
            signature_of("template <typename Iterator, typename Enable = void> struct phrase_parse_impl {"),
            Some(("struct".to_string(), "phrase_parse_impl".to_string()))
        );
        assert_eq!(
            signature_of("template <typename T> class Parser {"),
            Some(("class".to_string(), "Parser".to_string()))
        );
        // пустой параметрический блок (явная специализация)
        assert_eq!(
            signature_of("template <> struct token_printer_debug {"),
            Some(("struct".to_string(), "token_printer_debug".to_string()))
        );
        // префикс + fn
        assert_eq!(
            signature_of("template <class C> fn make() {"),
            Some(("fn".to_string(), "make".to_string()))
        );
        // незакрытый блок (многострочные параметры): строка template
        // НЕ матчится — имя ловится на своей строке отдельно
        assert_eq!(signature_of("template <typename Iterator,"), None);
        assert_eq!(
            signature_of("struct phrase_parse_impl {"),
            Some(("struct".to_string(), "phrase_parse_impl".to_string()))
        );
        // регрессия: сравнение без template не сломано
        assert_eq!(signature_of("int x = a < b;"), None);
        assert_eq!(
            signature_of("int sqlite3_step(sqlite3_stmt *pStmt){"),
            Some(("fn".to_string(), "sqlite3_step".to_string()))
        );
        // комментарий с template — не определение
        assert_eq!(signature_of("// template <typename T> struct Foo {"), None);
    }
}
