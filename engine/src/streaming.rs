//! Потоковый конвейер с ограниченным потреблением памяти (v0.3).
//!
//! Исправление bug#1 (неконтролируемый рост RSS до 2.8 ГБ на корпусах
//! с десятками миллионов токенов): **никогда** не удерживать инвертированные
//! индексы всех файлов репозитория одновременно. Вместо этого:
//!
//! * **Литеральный предфильтр** (техника GNU grep `kwset` / ripgrep
//!   prefilter): SIMD-решёто Teddy (ASCII case-insensitive; для
//!   кириллицы — lowercase-contains) отбраковывает файлы **до** токенизации;
//! * **Проход 1**: потоковая статистика — временный [`FileTokens`]
//!   (заимствованные из mmap срезы, zero-copy) освобождается сразу после
//!   файла; из него берутся глобальные частоты и позиции совпадений;
//! * **Проход 2**: только hit-файлы токенизируются повторно (временно),
//!   считаются ε/R, локализуются сцены (лёгкие структуры без клонов текста),
//!   извлекаются тройки;
//! * **Проход 3**: полная материализация сцен только для top-N якорей.
//!
//! Файлы крупнее [`GIANT_FILE_BYTES`] обрабатываются строго последовательно:
//! в любой момент времени в памяти не более одного большого временного
//! индекса. Межпроходный кэш текста (`TEXT_CACHE_LIMIT` из v0.2) удалён
//! полностью — каждый проход читает файл заново через mmap.

use crate::compression::{StatsRef, TermFreqs};
use crate::retrieval::teddy::Teddy;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::parser::markdown_scenes::{light_meta, LightSceneMeta, SceneBounds};
use crate::parser::{detect_lang, extract_code_triples, extract_triples, CodeLang, SceneContext, Triple};
use crate::resonance::{apply_iir_resonance, calculate_epsilon, semantic_bonus};
use crate::search::intent::{SignatureQuery, TypeDeclQuery};
use crate::{with_text, EngineConfig, PiiCleaner, ResonanceMode};

/// Порог «гигантского» файла: обрабатывается строго последовательно.
pub const GIANT_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// Полуширина окна (в токенах) proximity-AND поиска: если точная фраза
/// не найдена, многословный запрос ищется как «все токены в одном окне».
/// 128 токенов ≈ типичная функция/абзац — покрывает агентский кейс
/// «найди функцию, где встречаются оба термина».
pub const PROXIMITY_WINDOW: usize = 128;

// ---------------------------------------------------------------------------
// Zero-copy токены файла
// ---------------------------------------------------------------------------

/// Набор токенов файла: заимствованные из mmap срезы + ленивая
/// lowercase-нормализация.
///
/// Токены, уже стоящие в нижнем регистре (большинство), хранятся как
/// zero-copy срезы (16 байт); заглавные нормализуются в `Box<str>`
/// только при необходимости. В отличие от [`crate::InvertedIndex`]
/// (публичный API с owned-строками на каждый токен) память на токен
/// в ~3–4 раза меньше.
pub struct FileTokens<'a> {
    /// Исходные срезы токенов.
    raw: Vec<&'a str>,
    /// lowercase-версия, если отличается от исходной.
    lower: Vec<Option<Box<str>>>,
    /// Байтовые позиции начал токенов.
    pos: Vec<u32>,
    /// Частоты токенов файла (lowercase ключи).
    pub counts: HashMap<Box<str>, u32>,
}

impl<'a> FileTokens<'a> {
    /// Один линейный проход O(N). Правила те же, что у `InvertedIndex`.
    pub fn build(text: &'a str) -> Self {
        let cap = text.len() / 6 + 8;
        let mut s = Self {
            raw: Vec::with_capacity(cap),
            lower: Vec::with_capacity(cap),
            pos: Vec::with_capacity(cap),
            counts: HashMap::new(),
        };
        let mut start = 0usize;
        let mut in_word = false;
        for (i, ch) in text.char_indices() {
            if ch.is_alphanumeric() || ch == '_' {
                if !in_word {
                    in_word = true;
                    start = i;
                }
            } else if in_word {
                in_word = false;
                s.push(&text[start..i], start);
            }
        }
        if in_word {
            s.push(&text[start..], start);
        }
        s
    }

    fn push(&mut self, tok: &'a str, at: usize) {
        let lowered = tok.to_lowercase();
        if lowered == tok {
            self.raw.push(tok);
            self.lower.push(None);
        } else {
            self.raw.push(tok);
            self.lower.push(Some(lowered.into_boxed_str()));
        }
        self.pos.push(at as u32);
        let idx = self.raw.len() - 1;
        let key: &str = match &self.lower[idx] {
            Some(s) => s,
            None => self.raw[idx],
        };
        let existing = self.counts.get_mut(key);
        match existing {
            Some(c) => *c += 1,
            None => {
                self.counts.insert(key.to_string().into_boxed_str(), 1);
            }
        }
    }

    /// Токен в нижнем регистре по индексу.
    pub fn tok(&self, i: usize) -> &str {
        match &self.lower[i] {
            Some(s) => s,
            None => self.raw[i],
        }
    }

    /// Число токенов.
    pub fn toks_len(&self) -> usize {
        self.raw.len()
    }

    /// Позиции (индексы токенов) вхождений фразы запроса.
    ///
    /// Семантика (agent-friendly):
    /// • 1 токен — все вхождения токена;
    /// • ≥2 токена — сначала ТОЧНАЯ ФРАЗА (подряд, в порядке запроса);
    ///   если фразы нет — proximity-AND: вхождения самого редкого токена,
    ///   в окне ±[`PROXIMITY_WINDOW`] токенов которого встречаются ВСЕ
    ///   токены запроса (в любом порядке). Раньше многословный запрос
    ///   без точной фразы давал 0 хитов, даже если все токены жили в
    ///   одной функции — главный агентский кейс («isatty confirm_mutating»).
    pub fn find_phrase(&self, query: &[String]) -> Vec<u32> {
        self.find_phrase_inner(query, false)
    }

    /// Строгий режим (v0.82.0, код-интент): ТОЛЬКО точная фраза,
    /// proximity-AND выключен.
    ///
    /// Код-грамматические запросы (`func Default`, `impl Display`,
    /// `class Engine`) — синтаксически упорядочены: struct-теги
    /// (`form:",default=1"`) и случайные вхождения в окне ±128 токенов —
    /// шум, а не сигнал. Проза/нейтральные запросы сохраняют proximity
    /// (кейс «isatty confirm_mutating» — идентификаторы без грамматики).
    pub fn find_phrase_strict(&self, query: &[String]) -> Vec<u32> {
        self.find_phrase_inner(query, true)
    }

    /// Позиции токенов, НАЧИНАЮЩИХСЯ с префикса (v0.83.0, мягкий fallback
    /// сигнатурного запроса).
    ///
    /// Кейс владельца (wireshark): `proto_register_field(` — точного токена
    /// нет нигде, но есть `proto_register_field_array(`. Токенайзер рвёт по
    /// не-alnum, `proto_register_field_array` — ОДИН токен; равенство его с
    /// `proto_register_field` ложно, зато префикс-семантика находит всё
    /// семейство идентификаторов.
    pub fn find_token_prefix(&self, prefix: &str) -> Vec<u32> {
        if prefix.is_empty() {
            return Vec::new();
        }
        (0..self.raw.len())
            .filter(|i| self.tok(*i).starts_with(prefix))
            .map(|i| i as u32)
            .collect()
    }

    /// v0.85.0 (ГРАБЛЯ 43, LLVM): позиции токенов-ключевых слов
    /// `class`/`struct`, за которыми через 1–4 макро-атрибута стоит имя
    /// типа из запроса.
    ///
    /// `class LLVM_ABI Function : public GlobalObject {` при запросе
    /// `class Function :` — строгая фраза не матчится (макрос между
    /// токенами), гэп-паттерн находит определение. Валидация гэпа — по
    /// сырому тексту между keyword и именем ([`is_macro_gap_region`]):
    /// только целые слова из заглавных/цифр/подчёркиваний (LLVM_ABI,
    /// Q_CORE_EXPORT, PLATFORM_EXPORT, __attribute__) и/или атрибутные
    /// скобочные формы ([[nodiscard]], alignas(16), __declspec(...),
    /// скобочные фрагменты). Ключевое слово обязано стоять в НАЧАЛЕ
    /// строки: определения/декларации типов не пишут внутрь выражений,
    /// а `friend class X;` и `enum class X` отсекаются этим же якорем.
    ///
    /// Хит — токен keyword (начало строки объявления): превью (v0.83.0)
    /// начинается со строки хита → пользователь видит объявление целиком.
    pub fn find_type_decl_gaps(&self, text: &str, td: &TypeDeclQuery) -> Vec<u32> {
        let bytes = text.as_bytes();
        let n = self.raw.len();
        let mut out = Vec::new();
        for i in 0..n {
            if self.tok(i) != td.keyword {
                continue;
            }
            if !at_line_start(bytes, self.pos[i] as usize) {
                continue; // friend/enum-префикс/упоминание в выражении
            }
            // имя типа — в пределах нескольких токенов за keyword
            let kw_end = self.pos[i] as usize + self.raw[i].len();
            for j in (i + 1)..(i + 5).min(n) {
                if self.raw[j] != td.name_word {
                    continue; // кандидат-макрос в гэпе
                }
                let gap = &text[kw_end..self.pos[j] as usize];
                if is_macro_gap_region(gap) {
                    out.push(i as u32);
                }
                break; // первое вхождение имени в окне — единственный кандидат
            }
        }
        out
    }

    /// v0.86.0 (ГРАБЛЯ 45, Boost.Spirit): позиции токенов `template`, за
    /// которыми стоит параметрический блок `<…>` (баланс угловых скобок)
    /// и class-key + имя типа из запроса.
    ///
    /// `template <typename Iterator, typename Enable = void>
    ///  struct phrase_parse_impl {` при запросе `template struct
    /// phrase_parse`: строгая фраза слепа (между `template` и `struct`
    /// ~60 байт параметров), гэп-паттерн находит объявление.
    ///
    /// Блок `<…>` сканируется по сырому тексту с подсчётом глубины
    /// (вложенные `vector<T>` законны, многострочность законна —
    /// Boost переносит параметры на отдельные строки); `->` не сбивает
    /// глубину; незакрытый за лимитом блок — не шаблон (сырой `<` в
    /// выражениях). Хит — токен `template` (начало строки объявления):
    /// превью начинается со строки хита → юзер видит объявление целиком.
    pub fn find_template_decl_gaps(&self, text: &str, td: &TypeDeclQuery) -> Vec<u32> {
        let bytes = text.as_bytes();
        let n = self.raw.len();
        let mut out = Vec::new();
        for i in 0..n {
            if self.tok(i) != "template" {
                continue;
            }
            if !at_line_start(bytes, self.pos[i] as usize) {
                continue; // template в выражении / упоминание в комментарии-токенах
            }
            let kw_end = self.pos[i] as usize + self.raw[i].len();
            let Some(gt_end) = skip_template_param_block(bytes, kw_end) else {
                continue; // незакрытый блок — не объявление
            };
            // class-key — первый токен за закрывающей `>`
            let mut j = i + 1;
            while j < n && (self.pos[j] as usize) < gt_end {
                j += 1;
            }
            if j >= n || self.tok(j) != td.keyword {
                continue; // за блоком не struct/class — не объявление типа
            }
            // имя типа — в пределах нескольких токенов за class-key
            // (макро-гэп LLVM_ABI между class-key и именем — законен и тут;
            // ПУСТОЙ гэп — норма: template-запрос никогда не матчится
            // точной фразой, «чужая семантика» v0.85 сюда не переносится)
            let ck_end = self.pos[j] as usize + self.raw[j].len();
            for k in (j + 1)..(j + 5).min(n) {
                if self.raw[k] != td.name_word {
                    continue; // кандидат-макрос в гэпе
                }
                let gap = &text[ck_end..self.pos[k] as usize];
                if is_decl_gap_region(gap) {
                    out.push(i as u32);
                }
                break; // первое вхождение имени в окне — единственный кандидат
            }
        }
        out
    }

    /// v0.85.0: есть ли среди ТОЧНЫХ фразовых хитов (позиции keyword)
    /// определение типа — keyword в начале строки + def-сигнал
    /// ([`TypeDeclKind::Definition`]) после имени. Гасит гэп-fallback:
    /// определение без макроса найдено точным путём (Blender-кейс
    /// `struct BMVert {`).
    pub fn has_type_def_exact(&self, text: &str, hits: &[u32]) -> bool {
        let bytes = text.as_bytes();
        hits.iter().any(|&h| {
            let h = h as usize;
            if h + 1 >= self.raw.len() {
                return false;
            }
            // строгая фраза [keyword, name] нашлась в h — имя стоит в h+1
            if !at_line_start(bytes, self.pos[h] as usize) {
                return false; // friend-декларация/упоминание в выражении
            }
            let name_end = self.pos[h + 1] as usize + self.raw[h + 1].len();
            matches!(
                type_decl_kind_after(text, name_end),
                TypeDeclKind::Definition
            )
        })
    }

    fn find_phrase_inner(&self, query: &[String], strict: bool) -> Vec<u32> {
        if query.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let n = self.raw.len();
        if query.len() == 1 {
            let q = query[0].as_str();
            for i in 0..n {
                if self.tok(i) == q {
                    out.push(i as u32);
                }
            }
            return out;
        }
        let last_start = n.saturating_sub(query.len());
        'outer: for i in 0..=last_start {
            if self.tok(i) == query[0] {
                for (off, q) in query.iter().enumerate().skip(1) {
                    if self.raw.get(i + off).map_or(true, |_| self.tok(i + off) != q) {
                        continue 'outer;
                    }
                }
                out.push(i as u32);
            }
        }
        if !out.is_empty() {
            return out;
        }
        // Код-интент: proximity выключен — запрошена грамматика кода.
        if strict {
            return out;
        }
        // Дисциплина отрицания (Negation Blindness): частица «не»/«ни»
        // в запросе инвертирует смысл соседнего слова — «система обязана
        // отключиться» НЕ означает «система не обязана». Proximity-AND
        // подбирает случайную «не» из окна ±128 токенов (это абзацы!)
        // и возвращает ложное совпадение с ПРОТИВОПОЛОЖНЫМ смыслом.
        // Поэтому: в запросе с частицей отрицания засчитывается только
        // ТОЧНАЯ фраза (она и есть носитель смысла).
        if query.iter().any(|t| t == "не" || t == "ни") {
            return out;
        }
        self.find_proximity(query, PROXIMITY_WINDOW)
    }

    /// Proximity-AND: вхождения самого редкого токена запроса, в окне
    /// ±window токенов которого есть все остальные токены запроса.
    /// Позиции каждого уникального токена собираются одним проходом
    /// (отсортированы по построению), попадание в окно проверяется
    /// бинарным поиском — O(якоря × токены × log n).
    fn find_proximity(&self, query: &[String], window: usize) -> Vec<u32> {
        if self.raw.is_empty() || query.len() < 2 {
            return Vec::new();
        }
        // Уникальные токены запроса (повторы не проверяем дважды).
        let mut uniq: Vec<&str> = Vec::with_capacity(query.len());
        for t in query {
            if !uniq.contains(&t.as_str()) {
                uniq.push(t.as_str());
            }
        }
        if uniq.len() < 2 {
            // Все токены запроса одинаковы → семантика одиночного токена.
            let t = uniq[0];
            return (0..self.raw.len())
                .filter(|i| self.tok(*i) == t)
                .map(|i| i as u32)
                .collect();
        }
        // Один проход: позиции каждого уникального токена.
        let mut positions: Vec<Vec<u32>> = vec![Vec::new(); uniq.len()];
        for i in 0..self.raw.len() {
            let t = self.tok(i);
            for (ui, u) in uniq.iter().enumerate() {
                if t == *u {
                    positions[ui].push(i as u32);
                }
            }
        }
        // Хотя бы один токен в файле отсутствует — кандидатов нет.
        if positions.iter().any(|p| p.is_empty()) {
            return Vec::new();
        }
        // Якорь — самый редкий токен запроса (меньше проверок окна).
        let anchor = positions
            .iter()
            .enumerate()
            .min_by_key(|(_, p)| p.len())
            .map(|(i, _)| i)
            .unwrap_or(0);
        let hi_bound = self.raw.len() - 1;
        let mut out = Vec::new();
        'anchors: for &a in &positions[anchor] {
            let au = a as usize;
            let lo = au.saturating_sub(window);
            let hi = (au + window).min(hi_bound);
            for (ui, ps) in positions.iter().enumerate() {
                if ui == anchor {
                    continue;
                }
                // ps отсортирован: первый элемент >= lo.
                let first = ps.partition_point(|p| (*p as usize) < lo);
                if ps.get(first).map_or(true, |p| *p as usize > hi) {
                    continue 'anchors; // токен ui не попал в окно якоря
                }
            }
            out.push(a);
        }
        out
    }

    /// Байтовый диапазон текста, покрывающий окно токенов `[start, end)`.
    pub fn window_byte_range(&self, start: usize, end: usize) -> (usize, usize) {
        if start >= end || end == 0 || start >= self.raw.len() {
            return (0, 0);
        }
        let end = end.min(self.raw.len());
        let begin = self.pos[start] as usize;
        let finish = self.pos[end - 1] as usize + self.raw[end - 1].len();
        (begin, finish)
    }
}

/// Потоковый подсчёт частот токенов без построения массива токенов
/// (для файлов, отброшенных литеральным предфильтром).
///
/// v2.0 (Приоритет 3): ключи `Box<str>` — pass 1 больше не клонирует
/// строки при передаче словаря файла в sink (интернирование в
/// FSST-арену принимает `&str`).
pub fn streaming_counts(text: &str) -> (HashMap<Box<str>, u32>, u64) {
    let mut counts: HashMap<Box<str>, u32> = HashMap::new();
    let mut total = 0u64;
    let mut in_word = false;
    let mut start = 0usize;
    for (i, ch) in text.char_indices() {
        if ch.is_alphanumeric() || ch == '_' {
            if !in_word {
                in_word = true;
                start = i;
            }
        } else if in_word {
            in_word = false;
            total += 1;
            *counts.entry(text[start..i].to_string().into_boxed_str()).or_insert(0) += 1;
        }
    }
    if in_word {
        total += 1;
        *counts.entry(text[start..].to_string().into_boxed_str()).or_insert(0) += 1;
    }
    (counts, total)
}

// ---------------------------------------------------------------------------
// v0.85.0: гэп макро-атрибутов объявления типа (ГРАБЛЯ 43, LLVM/Qt)
// ---------------------------------------------------------------------------

/// Форма объявления типа после имени (v0.85.0).
///
/// `Definition` — за именем стоит `{` (тело) или `:` (базовый класс /
/// Python-класс); `Declaration` — `;` (forward-декларация) или иной
/// токен (elaborated-описатель переменной, упоминание в комментарии).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeDeclKind {
    Definition,
    Declaration,
}

/// Токен стоит в начале строки: от него назад — только пробелы/табы
/// до перевода строки (или начала файла).
fn at_line_start(bytes: &[u8], pos: usize) -> bool {
    let mut i = pos;
    while i > 0 {
        match bytes[i - 1] {
            b'\n' => return true,
            b' ' | b'\t' | b'\r' => i -= 1,
            _ => return false,
        }
    }
    true // начало файла
}

/// Def-сигнал после имени типа (v0.85.0).
///
/// 1) та же строка: ws → необязательное `final` → ws → `{`/`:` = Definition,
///    `;` = Declaration;
/// 2) стиль «class Foo\n{» — ограниченный проход по whitespace дальше
///    (≤2 переводов строки, ≤24 байт): `{` или `:` = Definition.
///
/// Python-классы (`class Foo:`) честно считаются определениями:
/// `:` сразу после имени — база (для C++) либо заголовок класса (Python).
pub(crate) fn type_decl_kind_after(text: &str, name_end: usize) -> TypeDeclKind {
    let bytes = text.as_bytes();
    let mut i = name_end;
    // 1) та же строка: пропускаем ws, необязательное `final`, снова ws.
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\r') {
        i += 1;
    }
    if text[i..].starts_with("final") {
        i += "final".len();
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\r') {
            i += 1;
        }
    }
    match bytes.get(i) {
        Some(b'{') | Some(b':') => return TypeDeclKind::Definition,
        Some(b';') => return TypeDeclKind::Declaration,
        _ => {}
    }
    // 2) `{`/базовый класс на следующей строке.
    let mut newlines = 0usize;
    while i < bytes.len() && newlines <= 2 && i - name_end <= 24 {
        match bytes[i] {
            b' ' | b'\t' | b'\r' => i += 1,
            b'\n' => {
                newlines += 1;
                i += 1;
            }
            b'{' | b':' => return TypeDeclKind::Definition,
            _ => return TypeDeclKind::Declaration,
        }
    }
    TypeDeclKind::Declaration
}

/// Гэп между keyword и именем типа — только макро-атрибуты (v0.85.0).
///
/// Пустой гэп — НЕ гэп (точная фраза, чужая семантика), перевод строки —
/// не гэп (объявление обязано жить в одной строке до имени), частей ≤ 4.
fn is_macro_gap_region(gap: &str) -> bool {
    if gap.is_empty() || gap.contains('\n') {
        return false;
    }
    let parts: Vec<&str> = gap.split_whitespace().collect();
    if parts.is_empty() || parts.len() > 4 {
        return false;
    }
    parts.iter().all(|p| is_macro_gap_part(p))
}

/// v0.86.0: гэп между class-key и именем в template-объявлении —
/// пустой (whitespace) допустим: template-запрос не матчится точной
/// фразой (параметрический блок между токенами), «чужая семантика
/// пустого гэпа» из v0.85 сюда не переносится.
fn is_decl_gap_region(gap: &str) -> bool {
    if gap.trim().is_empty() {
        return true;
    }
    is_macro_gap_region(gap)
}

/// Одна часть гэпа: макрос или атрибутная форма.
///
/// * целое слово из заглавных/цифр/подчёркиваний — LLVM_ABI,
///   Q_CORE_EXPORT, PLATFORM_EXPORT, V8_EXPORT, EXPORT;
/// * `[[...]]` — [[nodiscard]], [[deprecated("x")]];
/// * `alignas`/`alignas(16)`;
/// * GNU/MS-формы (строчные ключевые слова!): `__attribute__`,
///   `__declspec` — отдельно или со скобочной группой;
/// * скобочные фрагменты — `((visibility("default")))`, `(dllexport)`:
///   начинается с `(` либо заканчивается `)` (в легальном C++ между
///   class/struct и именем ничего иного стоять не может — грамматика
///   допускает только attribute-specifier-seq и макросы).
fn is_macro_gap_part(part: &str) -> bool {
    if part.is_empty() {
        return false;
    }
    let first = part.as_bytes()[0];
    let all_caps_word = (first.is_ascii_uppercase() || first == b'_')
        && part
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
    all_caps_word
        || part.starts_with("[[")
        || part.starts_with("alignas")
        || part.starts_with("__attribute__")
        || part.starts_with("__declspec")
        || part.starts_with('(')
        || part.ends_with(')')
}

/// v0.86.0 (ГРАБЛЯ 45): конец параметрического блока шаблона `<…>` —
/// байт ЗА закрывающей `>`, начиная от `from` (пропуск whitespace).
///
/// Баланс угловых скобок с учётом вложенности (`template <typename T,
/// std::vector<int> V>` законно); многострочность законна (Boost
/// переносит длинные списки параметров); `->` не закрывает уровень
/// (тип-выражения с стрелкой в параметрах не сбивают глубину).
/// `None` — блок не начался (нет `<`) или не закрылся в пределах
/// лимита (сырой `<` в выражениях — не шаблон).
fn skip_template_param_block(bytes: &[u8], from: usize) -> Option<usize> {
    /// Потолок длины блока: реалистичные списки параметров (даже
    /// многострочные с дефолтными аргументами) короче; сырая `<` в
    /// коде без закрытия дальше не живёт.
    const TEMPLATE_GAP_MAX_BYTES: usize = 4096;
    let mut p = from;
    while p < bytes.len() && matches!(bytes[p], b' ' | b'\t' | b'\n' | b'\r') {
        p += 1;
    }
    if p >= bytes.len() || bytes[p] != b'<' {
        return None;
    }
    let mut depth = 0usize;
    let mut i = p;
    while i < bytes.len() && i - p < TEMPLATE_GAP_MAX_BYTES {
        match bytes[i] {
            b'<' => depth += 1,
            b'>' if i > 0 && bytes[i - 1] == b'-' => {} // `->` — не закрывает
            b'>' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Литеральный предфильтр (Teddy SIMD — v2.0 задача 2.5)
// ---------------------------------------------------------------------------

/// Литеральный предфильтр запроса: SIMD-решёто Teddy (v2.0 задача 2.5,
/// замена автомата Ахо-Корасик на горячем пути прохода 1).
///
/// * все токены ASCII — регистронезависимое решёто по сырому тексту
///   (семантика прежнего `ascii_case_insensitive` AC, ноль аллокаций);
/// * не-ASCII токены — точное решёто по быстрому фолду ASCII+кириллицы
///   (байт-в-байт ≈ to_lowercase; прочие письменности — полный фолд).
///
/// `None` (пустые/мусорные токены, сверх лимита паттернов) — вызывающий
/// код падает на прежний lowercase-contains фолбэк.
pub fn literal_prefilter(query_tokens: &[String]) -> Option<Teddy> {
    if query_tokens.is_empty() || query_tokens.iter().any(|t| t.is_empty()) {
        return None;
    }
    let pats: Vec<&[u8]> = query_tokens.iter().map(|s| s.as_bytes()).collect();
    if query_tokens.iter().all(|t| t.is_ascii()) {
        return Teddy::build_ascii_ci(&pats).ok();
    }
    Teddy::build(&pats).ok()
}

/// Есть ли в сыром тексте необходимое условие совпадения (литерал
/// какого-либо токена запроса)? Вызывается ДО PII-маскирования и
/// токенизации — на каждом файле корпуса в проходе 1.
pub fn literal_present(raw: &str, query_tokens: &[String], pre: &Option<Teddy>) -> bool {
    if query_tokens.is_empty() {
        return false;
    }
    match pre {
        // ASCII-запрос: SIMD-решёто по сырому тексту, ноль аллокаций.
        Some(t) if t.is_ascii_ci() => t.is_present(raw.as_bytes()),
        // Кириллица и прочий не-ASCII: регистр меняет UTF-8 байты —
        // быстрый фолд ASCII+кириллицы (≈to_lowercase, ~1 ГБ/с) и один
        // проход решётом; прочие письменности — прежний полный фолд.
        Some(t) => match crate::retrieval::teddy::fold_ascii_cyrillic(raw.as_bytes()) {
            Some(folded) => t.is_present(&folded),
            None => t.is_present(raw.to_lowercase().as_bytes()),
        },
        // Фолбэк (пустые/избыточные токены): прежнее поведение.
        None => {
            let lower = raw.to_lowercase();
            query_tokens.iter().any(|q| lower.contains(q.as_str()))
        }
    }
}

// ---------------------------------------------------------------------------
// Проход 1: статистика + обнаружение совпадений
// ---------------------------------------------------------------------------

/// Результат прохода 1 по одному файлу.
///
/// `counts` — словарь файла с `Box<str>`-ключами, перемещаемый из
/// [`FileTokens`] без клонов строк (v2.0, Приоритет 3).
///
/// `prefix_hits` (v0.83.0) — ленивый мягкий fallback сигнатурного запроса:
/// позиции токенов, начинающихся с якоря идентификатора. Считается ТОЛЬКО
/// когда точных хитов в файле нет — движок решает глобально, точные хиты
/// где-либо есть → fallback не включается и эти позиции отбрасываются.
///
/// `type_def_exact`/`type_gap_hits` (v0.85.0) — зеркальный протокол для
/// объявлений типов с макро-атрибутами (ГРАБЛЯ 43): def-сигнал точных
/// хитов (гасит fallback глобально) и ленивые гэп-хиты
/// `class МАКРО_АТРИБУТ Name` — только в файлах без точных хитов.
pub struct Pass1File {
    pub counts: HashMap<Box<str>, u32>,
    pub total: u64,
    pub hits: Vec<u32>,
    pub prefix_hits: Vec<u32>,
    /// v0.85.0: среди точных хитов есть определение типа (def-сигнал
    /// `{`/`:` после имени, keyword в начале строки). Только код-файлы.
    pub type_def_exact: bool,
    /// v0.85.0: хиты гэп-паттерна `class/struct МАКРО_АТРИБУТ Name`.
    pub type_gap_hits: Vec<u32>,
}

pub fn pass1_file(
    path: &Path,
    query_tokens: &[String],
    strict_phrase: bool,
    config: &EngineConfig,
    _cleaner: &PiiCleaner,
    pre: &Option<Teddy>,
    prefix_anchor: Option<&str>,
    type_decl: Option<&TypeDeclQuery>,
) -> Option<Pass1File> {
    with_text(path, config.max_file_bytes, |raw| {
        // Предфильтр по сырому тексту: файлы без литерала не токенизируются,
        // но всё равно участвуют в глобальной статистике (streaming counts).
        //
        // PII-маскирование НЕ применяется на уровне токенизации (v0.4):
        // регексы по всему корпусу стоили ~65% времени. Маскирование
        // выполняется только на материализации выходных сцен (engine.rs),
        // где текст получает AI-потребитель.
        if !literal_present(raw, query_tokens, pre) {
            let (counts, total) = streaming_counts(raw);
            return Pass1File {
                counts,
                total,
                hits: Vec::new(),
                prefix_hits: Vec::new(),
                type_def_exact: false,
                type_gap_hits: Vec::new(),
            };
        }
        let ft = FileTokens::build(raw);
        // v0.82.0: код-интент — строгая фраза (proximity-AND — шум
        // для грамматических запросов `func Default`).
        let hits = if strict_phrase {
            ft.find_phrase_strict(query_tokens)
        } else {
            ft.find_phrase(query_tokens)
        };
        // v0.83.0: мягкий fallback — префикс якоря только при пустых
        // точных хитах файла (литеральный префильтр Teddy уже пропустил
        // файл: префикс — подстрока, решето её видит).
        let prefix_hits = if hits.is_empty() {
            prefix_anchor.map_or(Vec::new(), |pa| ft.find_token_prefix(pa))
        } else {
            Vec::new()
        };
        // v0.85.0 (ГРАБЛЯ 43): протокол объявлений типов — зеркально
        // prefix_hits. Точные хиты: def-сигнал (гасит гэп-fallback
        // глобально — Blender-кейс `struct BMVert {`). Нет точных:
        // лениво ищем гэп `class МАКРО Name` (LLVM-кейс) — v0.86.0:
        // template-запросы идут своим паттерном (параметрический блок
        // `<…>` между template и class-key, Boost-кейс). Только
        // код-файлы: определения типов в документации не живут.
        let (type_def_exact, type_gap_hits) = match type_decl {
            Some(td) if is_code_file(path) => {
                if hits.is_empty() {
                    let gaps = if td.template {
                        ft.find_template_decl_gaps(raw, td)
                    } else {
                        ft.find_type_decl_gaps(raw, td)
                    };
                    (false, gaps)
                } else {
                    (ft.has_type_def_exact(raw, &hits), Vec::new())
                }
            }
            _ => (false, Vec::new()),
        };
        let total = ft.toks_len() as u64;
        // v2.0: словарь файла перемещается целиком (ноль клонов строк);
        // интернирование в FSST-арену происходит в sink под мьютексом.
        Pass1File {
            counts: ft.counts,
            total,
            hits,
            prefix_hits,
            type_def_exact,
            type_gap_hits,
        }
    })
}

// ---------------------------------------------------------------------------
// Проход 2: ε / резонанс / локализация сцен / тройки
// ---------------------------------------------------------------------------

/// Лёгкая запись о совпадении: тяжёлая материализация (текст сцены)
/// отложена в проход 3 и выполняется только для top-N.
#[derive(Debug, Clone)]
pub struct HitRecord {
    pub path: PathBuf,
    pub byte_pos: usize,
    pub epsilon: f64,
    pub resonance: f64,
    pub scene_key: (usize, usize),
    pub is_code: bool,
    pub metric_tag: Option<String>,
    /// Хит совпал с сигнатурным паттерном запроса (v0.82.0).
    /// false при неактивном сигнатурном слое — тир определяется intent'ом.
    pub signature_matched: bool,
}

/// Результат прохода 2 по одному файлу.
/// Третий элемент — все байтовые позиции хитов файла (честный total_hits, diff).
pub type Pass2Result = (
    Vec<HitRecord>,
    HashMap<(usize, usize), SceneInfo>,
    Vec<usize>,
);

/// Информация уникальной сцены: только тройки (без клонов текста сцены).
#[derive(Debug, Clone, Default)]
pub struct SceneInfo {
    pub triples: Vec<Triple>,
    pub metric_tag: Option<String>,
}

pub(crate) fn is_code_file(path: &Path) -> bool {
    matches!(detect_lang(path), CodeLang::Brace | CodeLang::Python)
}

/// Char-safe усечение среза без разрыва UTF-8.
pub(crate) fn truncate_slice(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Разделяет файлы на гигантские (последовательная обработка) и обычные.
pub fn split_giants(files: &[PathBuf]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    files.iter().cloned().partition(|p| {
        std::fs::metadata(p).map(|m| m.len()).unwrap_or(0) >= GIANT_FILE_BYTES
    })
}

/// Целевой размер чанка гиганта (файл режется на ~2 МБ кусков).
pub const CHUNK_TARGET_BYTES: usize = 2 * 1024 * 1024;

/// Делит текст гиганта на чанки по границам ПАРАГРАФОВ (\n\n) и
/// markdown-заголовков, выравнивая байтовые позиции к границам UTF-8 и
/// строк. Граница сцены — единственный безопасный разрез: enclosing
/// scope не рвётся посреди логического блока.
///
/// Каждый чанк — (start, end) в байтах оригинального текста.
pub fn chunk_boundaries(text: &str) -> Vec<(usize, usize)> {
    let len = text.len();
    if len <= CHUNK_TARGET_BYTES {
        return vec![(0, len)];
    }
    let mut cuts = Vec::new();
    let mut search_from = 0usize;
    while search_from < len {
        let target = (search_from + CHUNK_TARGET_BYTES).min(len);
        if target >= len {
            cuts.push((search_from, len));
            break;
        }
        // ищем ближайшую границу абзаца (пустая строка) или заголовка
        // в окне [target - 512K, target + 512K]; выравниваем границы
        // окна по границам символов UTF-8
        let mut win_lo = search_from + CHUNK_TARGET_BYTES / 2;
        while win_lo < len && !text.is_char_boundary(win_lo) {
            win_lo += 1;
        }
        let mut win_hi = (target + CHUNK_TARGET_BYTES / 2).min(len);
        while win_hi > win_lo && !text.is_char_boundary(win_hi) {
            win_hi += 1;
        }
        let win_hi = win_hi.min(len);
        let zone = &text[win_lo.min(len)..win_hi.min(len)];
        let mut best: Option<usize> = None;
        // предпочитаем границу заголовка, затем границу абзаца
        if let Some(rel) = zone.find("\n\n#") {
            best = Some(win_lo + rel + 1);
        } else if let Some(rel) = zone.find("\n\n") {
            best = Some(win_lo + rel + 2);
        }
        let cut = best.unwrap_or(target);
        // выравниваем на границу символа и строки
        let mut cut = cut.min(len).max(search_from + 1);
        while cut < len && !text.is_char_boundary(cut) {
            cut += 1;
        }
        if cut < len && text.as_bytes()[cut] != b'\n' && cut + 1 < len {
            // дотягиваем до конца строки
            if let Some(nl) = text[cut..].find('\n') {
                cut += nl;
            }
        }
        if cut <= search_from {
            cut = target; // защита от зацикливания
            while cut < len && !text.is_char_boundary(cut) {
                cut += 1;
            }
        }
        cuts.push((search_from, cut));
        search_from = cut;
    }
    cuts
}

/// Параллельная обработка гигантского файла: чанки по границам сцен
/// обрабатываются независимо в rayon-пуле; результат — слитые записи
/// (байтовые позиции глобальные: chunk.offset прибавляется к локальным).
///
/// Память: каждый поток держит только свой чанк (zero-copy срез mmap),
/// пиковое потребление ограничено ~CHUNK_TARGET_BYTES × threads.
#[allow(clippy::too_many_arguments)]
pub fn pass2_giant_parallel(
    path: &Path,
    query_tokens: &[String],
    strict_phrase: bool,
    config: &EngineConfig,
    _cleaner: &PiiCleaner,
    global_counts: &StatsRef,
    n_total: usize,
    hits: &[u32],
    signature: Option<&SignatureQuery>,
    soft_prefix: Option<&str>,
    type_gap: Option<&TypeDeclQuery>,
) -> Option<Pass2Result> {
    use rayon::prelude::*;

    with_text(path, config.max_file_bytes, |raw| {
        let text: &str = raw;

        // Разбиение на чанки и распределение хитов по чанкам.
        let chunks = chunk_boundaries(text);
        if chunks.len() <= 1 {
            // один чанк — обычный последовательный путь
            return pass2_file(
                path,
                query_tokens,
                config,
                _cleaner,
                global_counts,
                n_total,
                hits,
                signature,
                soft_prefix,
            );
        }

        // Распределение хитов по чанкам: байтовые позиции растут вместе
        // с позициями токенов, поэтому достаточно одного прохода
        // (FileTokens уже построен внутри per_chunk-замыкания? нет —
        // строим временный для маппинга токен→байт).
        let per_chunk: Vec<Vec<u32>> = {
            let mut v: Vec<Vec<u32>> = vec![Vec::new(); chunks.len()];
            let mut chunk_idx = 0usize;
            let ft_hint = FileTokens::build(text);
            for &h in hits {
                let byte = ft_hint.window_byte_range(h as usize, h as usize + 1).0;
                while chunk_idx + 1 < chunks.len() && chunks[chunk_idx].1 <= byte {
                    chunk_idx += 1;
                }
                v[chunk_idx].push(h);
            }
            v
        };

        let lang = detect_lang(path);
        let code_file = is_code_file(path);
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // Каждый чанк обрабатывается как независимый «файл»: локальные
        // границы сцен смещаются на chunk.start → глобальные ключи.
        let results: Vec<Option<Pass2Result>> = chunks
            .par_iter()
            .zip(per_chunk.into_par_iter())
            .map(|((start, end), chunk_hits)| {
                if chunk_hits.is_empty() {
                    return None;
                }
                let sub_text = &text[*start..*end];
                let sub_ft = FileTokens::build(sub_text);
                // v0.83.0: мягкий fallback — префикс-семантика якоря;
                // v0.85.0: гэп-семантика объявления типа (ГРАБЛЯ 43);
                // v0.86.0: template-гэп — параметрический блок `<…>`
                // (ГРАБЛЯ 45, Boost.Spirit);
                // в норме — прежняя фразовая (strict для код-интента).
                let sub_hits = if let Some(pa) = soft_prefix {
                    sub_ft.find_token_prefix(pa)
                } else if let Some(td) = type_gap {
                    if td.template {
                        sub_ft.find_template_decl_gaps(sub_text, td)
                    } else {
                        sub_ft.find_type_decl_gaps(sub_text, td)
                    }
                } else if strict_phrase {
                    sub_ft.find_phrase_strict(query_tokens)
                } else {
                    sub_ft.find_phrase(query_tokens)
                };
                if sub_hits.is_empty() {
                    return None;
                }
                // v0.83.0: эффективные токены запроса для ε — в fallback
                // это фактические токены префикс-семейства чанка (kw_count
                // честно растёт), в норме — исходные токены запроса.
                let eff_tokens: Vec<String> = soft_prefix.map_or_else(
                    || query_tokens.to_vec(),
                    |pa| {
                        let mut fam: std::collections::BTreeSet<String> =
                            std::collections::BTreeSet::new();
                        for &h in &sub_hits {
                            let t = sub_ft.tok(h as usize);
                            if t.starts_with(pa) {
                                fam.insert(t.to_string());
                            }
                        }
                        fam.into_iter().collect()
                    },
                );

                let sub_eps_res: Vec<(f64, f64)> = match config.resonance_mode {
                    ResonanceMode::Psi => {
                        let mut epsilons: Vec<f64> = Vec::with_capacity(sub_hits.len());
                        for &h in &sub_hits {
                            let c = h as usize;
                            let s0 = c.saturating_sub(config.window_radius);
                            let e0 = (c + config.window_radius + 1).min(sub_ft.toks_len());
                            let (b, e) = sub_ft.window_byte_range(s0, e0.max(s0 + 1));
                            let wtext = &sub_text[b.min(sub_text.len())..e.min(sub_text.len())];
                            let bonus = semantic_bonus(wtext);
                            let window: Vec<&str> = (s0..e0)
                                .map(|i| sub_ft.tok(i))
                                .collect();
                            epsilons.push(calculate_epsilon(
                                &window,
                                &eff_tokens,
                                global_counts,
                                n_total,
                                config.kappa,
                                bonus,
                            ));
                        }
                        let forbidden = vec![false; sub_hits.len()];
                        let psi =
                            crate::psi::psi_resonances(&epsilons, &forbidden, config.psi_params);
                        epsilons.into_iter().zip(psi).collect()
                    }
                    // Канонический POLER-цикл (P3_Engine)
                    ResonanceMode::Poler => {
                        let mut epsilons: Vec<f64> = Vec::with_capacity(sub_hits.len());
                        for &h in &sub_hits {
                            let c = h as usize;
                            let s0 = c.saturating_sub(config.window_radius);
                            let e0 = (c + config.window_radius + 1).min(sub_ft.toks_len());
                            let (b, e) = sub_ft.window_byte_range(s0, e0.max(s0 + 1));
                            let wtext = &sub_text[b.min(sub_text.len())..e.min(sub_text.len())];
                            let bonus = semantic_bonus(wtext);
                            let window: Vec<&str> = (s0..e0)
                                .map(|i| sub_ft.tok(i))
                                .collect();
                            epsilons.push(calculate_epsilon(
                                &window,
                                &eff_tokens,
                                global_counts,
                                n_total,
                                config.kappa,
                                bonus,
                            ));
                        }
                        let forbidden = vec![false; sub_hits.len()];
                        let poler = crate::poler::poler_resonances(
                            &epsilons,
                            &forbidden,
                            config.poler_params,
                        );
                        epsilons.into_iter().zip(poler).collect()
                    }
                    ResonanceMode::Hits => {
                        let mut epsilons: Vec<f64> = Vec::with_capacity(sub_hits.len());
                        for &h in &sub_hits {
                            let c = h as usize;
                            let s0 = c.saturating_sub(config.window_radius);
                            let e0 = (c + config.window_radius + 1).min(sub_ft.toks_len());
                            let (b, e) = sub_ft.window_byte_range(s0, e0.max(s0 + 1));
                            let wtext = &sub_text[b.min(sub_text.len())..e.min(sub_text.len())];
                            let bonus = semantic_bonus(wtext);
                            let window: Vec<&str> = (s0..e0)
                                .map(|i| sub_ft.tok(i))
                                .collect();
                            epsilons.push(calculate_epsilon(
                                &window,
                                &eff_tokens,
                                global_counts,
                                n_total,
                                config.kappa,
                                bonus,
                            ));
                        }
                        let res = apply_iir_resonance(&epsilons, config.phi_decay);
                        epsilons.into_iter().zip(res).collect()
                    }
                    ResonanceMode::Field => {
                        let hit_set: std::collections::HashSet<usize> =
                            sub_hits.iter().map(|&h| h as usize).collect();
                        let mut out = vec![(0.0, 0.0); sub_hits.len()];
                        let log_n = (n_total.max(1) as f64).ln();
                        let qset: HashSet<&str> =
                            eff_tokens.iter().map(|s| s.as_str()).collect();
                        let rarity2 = |tok: &str| -> f64 {
                            let freq = global_counts.freq(tok).unwrap_or(1) as f64;
                            let rr = (log_n - freq.ln()).max(0.0);
                            rr * rr
                        };
                        let mut win: HashMap<&str, u32> = HashMap::new();
                        let mut unique_sum = 0.0f64;
                        let mut kw = 0u32;
                        let mut head = 0usize;
                        let mut tail = 0usize;
                        let mut r = 0.0f64;
                        let radius = config.window_radius;
                        let n_toks = sub_ft.toks_len();
                        for center in 0..n_toks {
                            let s0 = center.saturating_sub(radius);
                            let e0 = (center + radius + 1).min(n_toks);
                            while head < e0 {
                                let t = sub_ft.tok(head);
                                if qset.contains(t) {
                                    kw += 1;
                                } else {
                                    let e = win.entry(t).or_insert(0);
                                    if *e == 0 {
                                        unique_sum += rarity2(t);
                                    }
                                    *e += 1;
                                }
                                head += 1;
                            }
                            while tail < s0 {
                                let t = sub_ft.tok(tail);
                                if qset.contains(t) {
                                    kw = kw.saturating_sub(1);
                                } else if let Some(e) = win.get_mut(t) {
                                    *e -= 1;
                                    if *e == 0 {
                                        win.remove(t);
                                        unique_sum -= rarity2(t);
                                    }
                                }
                                tail += 1;
                            }
                            let eps =
                                config.kappa * (1.0 + ((kw as f64) + 1.0).ln()) * unique_sum;
                            r = eps + config.phi_decay * r;
                            if hit_set.contains(&center) {
                                if let Some(pos) =
                                    sub_hits.iter().position(|&h| h as usize == center)
                                {
                                    out[pos] = (eps, r);
                                }
                            }
                        }
                        out
                    }
                };

                // ── Стриминговый Top-K в чанке (bug PYCCLE) ──
                // v0.82.0: ключ кучи — (тир сигнатуры, R): сигнатурные хиты
                // выживают в per-chunk top-N ДО финальной сортировки движка.
                use std::cmp::Reverse;
                let chunk_locator =
                    crate::parser::markdown_scenes::SceneLocator::new(sub_text, path);
                let bit_key = |r: f64| -> u64 {
                    let b = r.to_bits();
                    if b >> 63 == 0 {
                        b ^ 0x8000_0000_0000_0000
                    } else {
                        !b
                    }
                };
                let mut heap: std::collections::BinaryHeap<Reverse<(u8, u64, usize)>> =
                    std::collections::BinaryHeap::new();
                // (hit_idx, byte_pos, key, is_code, sig_matched) — лёгкие данные чанка
                let mut light: Vec<(usize, usize, (usize, usize), bool, bool)> =
                    Vec::with_capacity(sub_hits.len().min(1 << 20));

                for (i, &h) in sub_hits.iter().enumerate() {
                    let c = h as usize;
                    let Some(&local_byte_u32) = sub_ft.pos.get(c) else {
                        continue;
                    };
                    let local_byte = local_byte_u32 as usize;
                    let byte_pos = (start + local_byte).min(text.len());

                    let (key, is_code) = if code_file {
                        match crate::parser::ast_code::locate_scope(sub_text, local_byte, lang) {
                            Some((b, e)) => ((start + b, start + e), true),
                            None => ((*start, *end), true),
                        }
                    } else {
                        // локатор ЧАНКА: заголовки один раз на чанк
                        let b = chunk_locator.locate(sub_text, local_byte);
                        ((*start + b.start, *start + b.end), false)
                    };

                    let sig_matched =
                        signature.map_or(false, |sig| sig.matches_at(text, byte_pos));
                    light.push((i, byte_pos, key, is_code, sig_matched));
                    let tier: u8 = if signature.is_some() {
                        u8::from(!sig_matched)
                    } else {
                        0
                    };
                    heap.push(Reverse((tier, bit_key(sub_eps_res[i].1), light.len() - 1)));
                    if heap.len() > config.top_n {
                        heap.pop();
                    }
                }
                let chunk_hit_keys: Vec<usize> =
                    light.iter().map(|(_, b, _, _, _)| *b).collect();
                let survivors: Vec<usize> = {
                    let mut v: Vec<usize> = heap
                        .into_iter()
                        .map(|Reverse((_, _, li))| li)
                        .collect();
                    v.sort_unstable();
                    v
                };

                // Тяжёлые сцены — только для выживших чанка
                let mut scenes: HashMap<(usize, usize), SceneInfo> = HashMap::new();
                let mut records: Vec<HitRecord> = Vec::with_capacity(survivors.len());
                for li in survivors {
                    let (i, byte_pos, key, is_code, sig_matched) = light[li];
                    let info = scenes.entry(key).or_insert_with(|| {
                        if is_code {
                            let scope_text = &text[key.0..key.1.min(text.len())];
                            let name = crate::parser::ast_code::light_signature(text, key.0);
                            SceneInfo {
                                triples: extract_code_triples(scope_text, name.as_deref(), &stem),
                                metric_tag: None,
                            }
                        } else {
                            let b = crate::parser::markdown_scenes::SceneBounds {
                                chapter: stem.clone(),
                                start: key.0,
                                end: key.1,
                                structured: false,
                            };
                            let meta = crate::parser::markdown_scenes::light_meta(text, &b);
                            let scope_text = truncate_slice(
                                &text[key.0..key.1.min(text.len())],
                                config.max_scope_bytes,
                            );
                            let scene = SceneContext::from_meta(&meta);
                            SceneInfo {
                                triples: extract_triples(scope_text, &scene, &eff_tokens),
                                metric_tag: meta.metric_tag,
                            }
                        }
                    });
                    records.push(HitRecord {
                        path: path.to_path_buf(),
                        byte_pos,
                        epsilon: sub_eps_res[i].0,
                        resonance: sub_eps_res[i].1,
                        scene_key: key,
                        is_code,
                        metric_tag: info.metric_tag.clone(),
                        signature_matched: sig_matched,
                    });
                }
                Some((records, scenes, chunk_hit_keys))
            })
            .collect();

        // Слияние чанков: объединение per-chunk top-N ⊇ глобального top-N
        // (IIR в чанках независим) -> финальная обрезка по (тир, резонанс).
        let mut all_light: Vec<HitRecord> = Vec::new();
        let mut all_scenes: HashMap<(usize, usize), SceneInfo> = HashMap::new();
        let mut all_hit_keys: Vec<usize> = Vec::new();
        for (recs, scns, keys) in results.into_iter().flatten() {
            all_hit_keys.extend(keys);
            all_light.extend(recs);
            for (k, v) in scns {
                all_scenes.entry(k).or_insert(v);
            }
        }
        let sig_active = signature.is_some();
        all_light.sort_by(|a, b| {
            let ta = u8::from(sig_active && !a.signature_matched);
            let tb = u8::from(sig_active && !b.signature_matched);
            ta.cmp(&tb).then_with(|| {
                b.resonance
                    .partial_cmp(&a.resonance)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        all_light.truncate(config.top_n);
        // осиротевшие сцены после финальной обрезки
        let alive: std::collections::HashSet<(usize, usize)> =
            all_light.iter().map(|r| r.scene_key).collect();
        all_scenes.retain(|k, _| alive.contains(k));
        Some((all_light, all_scenes, all_hit_keys))
    })?
}

/// Поле резонанса строго за один проход O(N): инкрементальное скользящее
/// окно по `&str`-токенам + IIR; сэмплы в точках совпадений.
#[allow(clippy::too_many_arguments)]
fn field_eps_res(
    ft: &FileTokens,
    hits: &[u32],
    query: &[String],
    gcounts: &StatsRef,
    gtotal: usize,
    kappa: f64,
    phi: f64,
    radius: usize,
) -> Vec<(f64, f64)> {
    let mut out = vec![(0.0, 0.0); hits.len()];
    if ft.toks_len() == 0 {
        return out;
    }
    let log_n = (gtotal.max(1) as f64).ln();
    let qset: HashSet<&str> = query.iter().map(|s| s.as_str()).collect();
    let hit_idx: HashMap<u32, usize> = hits.iter().enumerate().map(|(i, &h)| (h, i)).collect();
    let rarity2 = |tok: &str| -> f64 {
        let freq = gcounts.freq(tok).unwrap_or(1) as f64;
        let rr = (log_n - freq.ln()).max(0.0);
        rr * rr
    };

    let mut win: HashMap<&str, u32> = HashMap::new();
    let mut unique_sum = 0.0f64;
    let mut kw = 0u32;
    let mut head = 0usize;
    let mut tail = 0usize;
    let mut r = 0.0f64;

    for center in 0..ft.toks_len() {
        let start = center.saturating_sub(radius);
        let end = (center + radius + 1).min(ft.toks_len());
        while head < end {
            let t = ft.tok(head);
            if qset.contains(t) {
                kw += 1;
            } else {
                let e = win.entry(t).or_insert(0);
                if *e == 0 {
                    unique_sum += rarity2(t);
                }
                *e += 1;
            }
            head += 1;
        }
        while tail < start {
            let t = ft.tok(tail);
            if qset.contains(t) {
                kw = kw.saturating_sub(1);
            } else if let Some(e) = win.get_mut(t) {
                *e -= 1;
                if *e == 0 {
                    win.remove(t);
                    unique_sum -= rarity2(t);
                }
            }
            tail += 1;
        }
        let eps = kappa * (1.0 + ((kw as f64) + 1.0).ln()) * unique_sum;
        r = eps + phi * r;
        if let Some(&i) = hit_idx.get(&(center as u32)) {
            out[i] = (eps, r);
        }
    }
    out
}

/// Строит SceneInfo (тройки + метрика) для уникальной сцены — без клонов
/// текста сцены: тройки извлекаются из среза mmap.
fn build_scene_info(
    text: &str,
    bounds: &SceneBounds,
    key: (usize, usize),
    is_code: bool,
    stem: &str,
    query_tokens: &[String],
    config: &EngineConfig,
) -> SceneInfo {
    let start = key.0.min(text.len());
    let end = key.1.min(text.len()).max(start);
    if is_code {
        let scope_text = &text[start..end];
        let name = crate::parser::ast_code::light_signature(text, start);
        SceneInfo {
            triples: extract_code_triples(scope_text, name.as_deref(), stem),
            metric_tag: None,
        }
    } else {
        let meta: LightSceneMeta = light_meta(text, bounds);
        let scope_text = truncate_slice(&text[start..end], config.max_scope_bytes);
        let scene = SceneContext::from_meta(&meta);
        SceneInfo {
            triples: extract_triples(scope_text, &scene, query_tokens),
            metric_tag: meta.metric_tag,
        }
    }
}

/// Проход 2 по hit-файлу: возвращает лёгкие записи и тройки сцен.
///
/// v0.82.0: `signature` — литеральный сигнатурный паттерн запроса
/// (активен, когда в запросе есть прикреплённая скобка `Default(`).
/// Сигнатурные хиты выживают в per-file top-N первыми (тир в ключе кучи).
///
/// v0.83.0: `soft_prefix` — активный мягкий fallback (движок уже выяснил:
/// точных хитов нет нигде). `hits` тогда содержат префиксные позиции из
/// прохода 1, а ε считается по фактическому префикс-семейству файла.
#[allow(clippy::too_many_arguments)]
pub fn pass2_file(
    path: &Path,
    query_tokens: &[String],
    config: &EngineConfig,
    _cleaner: &PiiCleaner,
    global_counts: &StatsRef,
    n_total: usize,
    hits: &[u32],
    signature: Option<&SignatureQuery>,
    soft_prefix: Option<&str>,
) -> Option<Pass2Result> {
    with_text(path, config.max_file_bytes, |raw| {
        let text: &str = raw;
        let ft = FileTokens::build(text);
        let lang = detect_lang(path);
        let code_file = is_code_file(path);
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        // Кэш заголовков: один проход по файлу вместо O(файл) на каждый хит
        let locator = crate::parser::markdown_scenes::SceneLocator::new(text, path);

        // Статистики для ε: глобальные по корпусу либо локальные по файлу
        // (v2.0: локальный словарь заимствуется у FileTokens — ноль клонов).
        let local_stats: StatsRef;
        let (gcounts, gtotal): (&StatsRef, usize) = if config.local_stats {
            local_stats = StatsRef::Local(&ft.counts);
            (&local_stats, ft.toks_len())
        } else {
            (global_counts, n_total)
        };

        // v0.83.0: эффективные токены запроса для ε. В норме — токены
        // запроса; в мягком fallback — фактическое префикс-семейство
        // файла (BTreeSet: детерминированный порядок), чтобы kw_count
        // честно отражал плотность совпадений, а не фиксировал ноль.
        let eff_tokens: Vec<String> = soft_prefix.map_or_else(
            || query_tokens.to_vec(),
            |pa| {
                let mut fam: std::collections::BTreeSet<String> =
                    std::collections::BTreeSet::new();
                for &h in hits {
                    let t = ft.tok(h as usize);
                    if t.starts_with(pa) {
                        fam.insert(t.to_string());
                    }
                }
                fam.into_iter().collect()
            },
        );

        // ε и IIR-резонанс по последовательности совпадений либо полю.
        let eps_res: Vec<(f64, f64)> = match config.resonance_mode {
            // POLER[Ψ]: наблюдения = ε окон, эволюция внимания по
            // каноническому уравнению p_{t+1} = p_t + ηΠ(−∇F + γ∇ε).
            // ψ-резонанс нормируется к масштабу ε (|p| ≤ 1 после tanh) —
            // умножаем на интенсивность последнего окна.
            ResonanceMode::Psi => {
                let mut epsilons: Vec<f64> = Vec::with_capacity(hits.len());
                for &h in hits {
                    let c = h as usize;
                    let start = c.saturating_sub(config.window_radius);
                    let end = (c + config.window_radius + 1).min(ft.toks_len());
                    let (b, e) = ft.window_byte_range(start, end.max(start + 1));
                    let wtext = &text[b.min(text.len())..e.min(text.len())];
                    let bonus = semantic_bonus(wtext);
                    let window: Vec<&str> = (start..end)
                        .map(|i| ft.tok(i))
                        .collect();
                    epsilons.push(calculate_epsilon(
                        &window,
                        &eff_tokens,
                        gcounts,
                        gtotal,
                        config.kappa,
                        bonus,
                    ));
                }
                let forbidden = vec![false; hits.len()];
                let psi = crate::psi::psi_resonances(&epsilons, &forbidden, config.psi_params);
                epsilons.into_iter().zip(psi).collect()
            }
            // Канонический POLER-цикл (P3_Engine): p −= ηΠ_Λ(D·p + γJ·p + ∇F)
            ResonanceMode::Poler => {
                let mut epsilons: Vec<f64> = Vec::with_capacity(hits.len());
                for &h in hits {
                    let c = h as usize;
                    let start = c.saturating_sub(config.window_radius);
                    let end = (c + config.window_radius + 1).min(ft.toks_len());
                    let (b, e) = ft.window_byte_range(start, end.max(start + 1));
                    let wtext = &text[b.min(text.len())..e.min(text.len())];
                    let bonus = semantic_bonus(wtext);
                    let window: Vec<&str> = (start..end)
                        .map(|i| ft.tok(i))
                        .collect();
                    epsilons.push(calculate_epsilon(
                        &window,
                        &eff_tokens,
                        gcounts,
                        gtotal,
                        config.kappa,
                        bonus,
                    ));
                }
                let forbidden = vec![false; hits.len()];
                let poler =
                    crate::poler::poler_resonances(&epsilons, &forbidden, config.poler_params);
                epsilons.into_iter().zip(poler).collect()
            }
            ResonanceMode::Hits => {
                let mut epsilons: Vec<f64> = Vec::with_capacity(hits.len());
                for &h in hits {
                    let c = h as usize;
                    let start = c.saturating_sub(config.window_radius);
                    let end = (c + config.window_radius + 1).min(ft.toks_len());
                    let (b, e) = ft.window_byte_range(start, end.max(start + 1));
                    let wtext = &text[b.min(text.len())..e.min(text.len())];
                    let bonus = semantic_bonus(wtext);
                    let window: Vec<&str> = (start..end)
                        .map(|i| ft.tok(i))
                        .collect();
                    epsilons.push(calculate_epsilon(
                        &window,
                        &eff_tokens,
                        gcounts,
                        gtotal,
                        config.kappa,
                        bonus,
                    ));
                }
                let res = apply_iir_resonance(&epsilons, config.phi_decay);
                epsilons.into_iter().zip(res).collect()
            }
            ResonanceMode::Field => field_eps_res(
                &ft,
                hits,
                &eff_tokens,
                gcounts,
                gtotal,
                config.kappa,
                config.phi_decay,
                config.window_radius,
            ),
        };

        // ── Стриминговый Top-K (bug PYCCLE: суперчастотные слова) ──
        // SceneInfo (тройки — дорого) НЕ строится на лету для каждого хита:
        // лёгкие записи собираются в ограниченную кучу top_n по резонансу,
        // тяжёлые сцены строятся только для выживших. Математическая
        // корректность: резонанс хита зависит только от предыдущих хитов
        // (IIR), поэтому отсев по ходу не меняет значений выживших.
        use std::cmp::Reverse;

        // Куча top-N: ключ — монотонное отображение f64 в u64
        // (каноничный total-order transform): не-/отрицательные ветки.
        // v0.82.0: префикс тира — сигнатурные хиты выживают первыми.
        let bit_key = |r: f64| -> u64 {
            let b = r.to_bits();
            if b >> 63 == 0 {
                b ^ 0x8000_0000_0000_0000
            } else {
                !b
            }
        };
        let mut heap: std::collections::BinaryHeap<Reverse<(u8, u64, usize)>> =
            std::collections::BinaryHeap::new();
        // Лёгкие данные хита: (hit_idx, byte_pos, key, is_code, sig_matched)
        let mut light: Vec<(usize, usize, (usize, usize), bool, bool)> =
            Vec::with_capacity(hits.len().min(1 << 20));
        let mut bounds_cache: HashMap<(usize, usize), SceneBounds> = HashMap::new();

        for (i, &h) in hits.iter().enumerate() {
            let c = h as usize;
            let Some(&byte_pos_u32) = ft.pos.get(c) else {
                continue;
            };
            let byte_pos = (byte_pos_u32 as usize).min(text.len());

            let (key, is_code) = if code_file {
                match crate::parser::ast_code::locate_scope(text, byte_pos, lang) {
                    Some((b, e)) => ((b, e), true),
                    None => ((0, text.len()), true),
                }
            } else {
                let b = locator.locate(text, byte_pos);
                let key = (b.start, b.end);
                bounds_cache.entry(key).or_insert_with(|| b);
                (key, false)
            };

            let sig_matched = signature.map_or(false, |sig| sig.matches_at(text, byte_pos));
            light.push((i, byte_pos, key, is_code, sig_matched));
            let tier: u8 = if signature.is_some() {
                u8::from(!sig_matched)
            } else {
                0
            };
            heap.push(Reverse((tier, bit_key(eps_res[i].1), light.len() - 1)));
            if heap.len() > config.top_n {
                heap.pop();
            }
        }

        // Полный набор байтовых позиций (компактно) — до отсева
        let hit_keys: Vec<usize> = light.iter().map(|(_, b, _, _, _)| *b).collect();

        // Выжившие индексы (top-N по (тир, резонанс)) — сцены строим только для них
        let survivors: Vec<usize> = {
            let mut v: Vec<usize> = heap
                .into_iter()
                .map(|Reverse((_, _, li))| li)
                .collect();
            v.sort_unstable();
            v
        };

        let mut scenes: HashMap<(usize, usize), SceneInfo> = HashMap::new();
        let mut records: Vec<HitRecord> = Vec::with_capacity(survivors.len());
        for li in survivors {
            let (i, byte_pos, key, is_code, sig_matched) = light[li];
            let bounds = if is_code {
                None
            } else {
                Some(SceneBounds {
                    chapter: stem.clone(),
                    start: key.0,
                    end: key.1,
                    structured: false,
                })
            };
            // bounds из кэша точнее (chapter/structured), fallback — синтетика
            let info = scenes.entry(key).or_insert_with(|| {
                let b = bounds_cache.get(&key).cloned().unwrap_or_else(|| {
                    bounds.clone().unwrap_or(SceneBounds {
                        chapter: stem.clone(),
                        start: key.0,
                        end: key.1,
                        structured: false,
                    })
                });
                build_scene_info(text, &b, key, is_code, &stem, &eff_tokens, config)
            });
            records.push(HitRecord {
                path: path.to_path_buf(),
                byte_pos,
                epsilon: eps_res[i].0,
                resonance: eps_res[i].1,
                scene_key: key,
                is_code,
                metric_tag: info.metric_tag.clone(),
                signature_matched: sig_matched,
            });
        }

        Some((records, scenes, hit_keys))
    })?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn file_tokens_matches_inverted_index() {
        let text = "Нокс вонзила когти в сплетение. Нокс и снова нокс";
        let ft = FileTokens::build(text);
        let idx = crate::InvertedIndex::build(text);
        let ft_owned: Vec<&str> = (0..ft.toks_len()).map(|i| ft.tok(i)).collect();
        assert_eq!(ft_owned, idx.tokens);
        assert_eq!(ft.pos.len(), idx.positions.len());
        for (a, b) in ft.pos.iter().zip(idx.positions.iter()) {
            assert_eq!(*a as usize, *b);
        }
        assert_eq!(ft.find_phrase(&q(&["нокс"])), vec![0, 5, 8]);
    }

    #[test]
    fn streaming_counts_matches_file_tokens() {
        let text = "alpha beta gamma alpha beta x";
        let (counts, total) = streaming_counts(text);
        let ft = FileTokens::build(text);
        assert_eq!(total, ft.toks_len() as u64);
        assert_eq!(counts.get("alpha"), Some(&2));
        assert_eq!(counts.get("beta"), Some(&2));
        assert_eq!(counts.get("gamma"), Some(&1));
        for (k, v) in &ft.counts {
            assert_eq!(counts.get(k.as_ref()), Some(&{ *v }));
        }
    }

    #[test]
    fn cyrillic_uppercase_normalized() {
        let ft = FileTokens::build("Нокс и НОКС и нокс");
        assert_eq!(ft.find_phrase(&q(&["нокс"])), vec![0, 2, 4]);
        assert_eq!(ft.counts.get("нокс").copied(), Some(3));
    }

    #[test]
    fn field_mode_matches_hits_mode_without_bonus() {
        // Инвариант: field-режим (строго O(N)) на позициях совпадений
        // даёт те же ε/R, что и пакетный расчёт окна без семантического бонуса.
        let text = "alpha beta gamma delta alpha epsilon zeta alpha beta theta iota \
                    kappa lambda alpha mu nu xi omicron pi alpha";
        let ft = FileTokens::build(text);
        let query = q(&["alpha"]);
        // v2.0: field_eps_res принимает StatsRef; локальная статистика —
        // заимствованный словарь FileTokens (как в реальном пути --local-stats).
        let gcounts = crate::compression::StatsRef::Local(&ft.counts);
        let hits = ft.find_phrase(&query);
        let radius = 3;
        let n = ft.toks_len();

        let field = field_eps_res(&ft, &hits, &query, &gcounts, n, 1.0, 0.85, radius);

        let mut epsilons = Vec::new();
        for &h in &hits {
            let c = h as usize;
            let start = c.saturating_sub(radius);
            let end = (c + radius + 1).min(n);
            let window: Vec<&str> = (start..end).map(|i| ft.tok(i)).collect();
            epsilons.push(calculate_epsilon(&window, &query, &gcounts, n, 1.0, 0.0));
        }
        // ε обязаны совпадать с пакетным расчётом окна; R в field-режиме
        // прогоняется по ВСЕМ позициям (а не только хитам), поэтому
        // сравнивается только положительностью и монотонностью аккумулятора.
        for (i, (e, r)) in field.iter().enumerate() {
            assert!((e - epsilons[i]).abs() < 1e-6, "eps mismatch at {i}");
            assert!(*r > 0.0, "resonance must be positive at {i}");
        }
        for w in field.windows(2) {
            assert!(w[1].1 >= w[0].1 * 0.85 - 1e-9);
        }
    }

    #[test]
    fn literal_prefilter_ascii_and_cyrillic() {
        // ASCII: CI-решёто Teddy по сырому тексту (как прежде AC-CI).
        let pre = literal_prefilter(&q(&["process"]));
        assert!(pre.is_some());
        assert!(pre.as_ref().unwrap().is_ascii_ci());
        assert!(literal_present("call PROCESS now", &q(&["process"]), &pre));
        assert!(!literal_present("nothing here", &q(&["process"]), &pre));

        // Кириллица: теперь тоже Teddy — точное решёто по lowercase-копии
        // (раньше — N × contains на ту же копию).
        let cyr = q(&["нокс"]);
        let pre = literal_prefilter(&cyr);
        assert!(pre.is_some());
        assert!(!pre.as_ref().unwrap().is_ascii_ci());
        assert!(literal_present("Здесь Нокс действует", &cyr, &pre));
        assert!(literal_present("НОКС и нокс", &cyr, &pre));
        assert!(!literal_present("Здесь Соболь", &cyr, &pre));

        // Фолбэк без решёта: прежний lowercase-contains.
        let none: Option<Teddy> = None;
        assert!(literal_present("Здесь Нокс действует", &cyr, &none));
        assert!(!literal_present("Здесь Соболь", &cyr, &none));
        // Много-токенный запрос: достаточно любого литерала.
        let both = q(&["alpha", "omega"]);
        let pre = literal_prefilter(&both);
        assert!(literal_present("только ALPHA тут", &both, &pre));
        assert!(literal_present("только omega тут", &both, &pre));
        assert!(!literal_present("ни одного литерала", &both, &pre));
    }

    #[test]
    fn find_phrase_exact_phrase_wins_over_proximity() {
        // Точная фраза найдена → proximity не должен подмешивать лишние якоря.
        let ft = FileTokens::build("alpha beta gamma alpha delta beta");
        let hits = ft.find_phrase(&q(&["alpha", "beta"]));
        assert_eq!(hits, vec![0]);
    }

    #[test]
    fn find_phrase_proximity_fallback() {
        // Реальный кейс из багрепорта: оба токена в одной функции,
        // но не подряд. Раньше — 0 хитов.
        let ft = FileTokens::build(
            "pub fn confirm_mutating ( action : & ResolvedAction ) -> bool { \
             let stdin_fd = std io stdin ( ) ; let is_stdin_tty = unsafe { \
             libc :: isatty ( stdin_fd ) == 1 } ; }",
        );
        let hits = ft.find_phrase(&q(&["isatty", "confirm_mutating"]));
        assert!(!hits.is_empty(), "proximity-AND должен найти функцию");
        // Якорь — позиция одного из токенов запроса.
        for h in &hits {
            let t = ft.tok(*h as usize);
            assert!(
                t == "isatty" || t == "confirm_mutating",
                "якорь должен быть токеном запроса, got {t}"
            );
        }
    }

    #[test]
    fn find_phrase_strict_drops_proximity_noise() {
        // v0.82.0 (код-интент): struct-тег `form:",default=1"` + `func`
        // в окне ±128 токенов — proximity-шум. Строгий режим — только
        // точная фраза: теги выпадают, определение остаётся.
        // Файл БЕЗ фразы `func default` (теги + func далеко):
        let noise = "var s struct { Age int `form:\",default=1\"` } \
                   func TestMapping ( t * TestingT ) { s = load ( ) }";
        let ft = FileTokens::build(noise);
        let loose = ft.find_phrase(&q(&["func", "default"]));
        assert!(!loose.is_empty(), "proximity должен ловить шум тегов");
        let strict = ft.find_phrase_strict(&q(&["func", "default"]));
        assert!(strict.is_empty(), "строгий режим обязан отбросить теги");
        // Файл С фразой `func Default (` — оба режима находят только её
        let def = "func Default ( ) * Engine { return New ( ) }";
        let ft2 = FileTokens::build(def);
        assert_eq!(
            ft2.find_phrase_strict(&q(&["func", "default"])),
            ft2.find_phrase(&q(&["func", "default"]))
        );
    }

    #[test]
    fn find_phrase_strict_single_token_unchanged() {
        // одиночный токен — семантика идентична в обоих режимах
        let ft = FileTokens::build("alpha beta alpha gamma");
        assert_eq!(
            ft.find_phrase_strict(&q(&["alpha"])),
            ft.find_phrase(&q(&["alpha"]))
        );
    }

    #[test]
    fn find_phrase_proximity_respects_window() {
        // 300 разделителей > окно 128 → совпадения быть не должно.
        let mut text = String::from("anchor");
        for i in 0..300 {
            text.push_str(&format!(" filler{i}"));
        }
        text.push_str(" needle");
        let ft = FileTokens::build(&text);
        assert!(ft.find_phrase(&q(&["anchor", "needle"])).is_empty());
    }

    #[test]
    fn find_phrase_negation_discipline() {
        // «система обязана ... не должны ...»: случайная «не» из окна
        // ±128 токенов НЕ делает «не обязана» найденной (регрессия
        // 7ee6b98: proximity-AND инвертировал смысл).
        let text = "Шунт сбрасывает избыточное тепло, которые не должны были \
             достаться телу. Нокс вонзила когти: давление падает, система \
             обязана отключиться. Мальчик не должен был выжить.";
        let ft = FileTokens::build(text);
        assert!(
            ft.find_phrase(&q(&["не", "обязана"])).is_empty(),
            "«не обязана» отсутствует — частица отключает proximity"
        );
        // точная фраза с «не» находится как обычно
        assert!(!ft.find_phrase(&q(&["не", "должны"])).is_empty());
        assert!(!ft.find_phrase(&q(&["не", "должен"])).is_empty());
        // без частицы proximity работает
        assert!(!ft.find_phrase(&q(&["система", "отключиться"])).is_empty());
    }

    #[test]
    fn find_phrase_proximity_missing_token_empty() {
        let ft = FileTokens::build("alpha beta gamma");
        assert!(ft.find_phrase(&q(&["alpha", "zzz"])).is_empty());
    }

    #[test]
    fn find_phrase_duplicated_token_single_semantics() {
        let ft = FileTokens::build("alpha beta alpha gamma");
        let hits = ft.find_phrase(&q(&["alpha", "alpha"]));
        assert_eq!(hits, vec![0, 2]);
    }

    #[test]
    fn window_byte_range_correct() {
        let ft = FileTokens::build("alpha beta gamma delta");
        let (b, e) = ft.window_byte_range(1, 3);
        assert_eq!(&"alpha beta gamma delta"[b..e], "beta gamma");
    }

    #[test]
    fn chunk_boundaries_small_file_single_chunk() {
        let text = "короткий текст";
        assert_eq!(chunk_boundaries(text), vec![(0, text.len())]);
    }

    #[test]
    fn chunk_boundaries_respect_paragraph_edges() {
        // большой текст: границы только по \n\n и UTF-8
        let mut text = String::new();
        // ~3 МБ: абзацы по ~400 байт
        for i in 0..12000 {
            text.push_str(&format!(
                "Абзац номер {i} содержит осмысленный текст средней длины \
                 с несколькими словами и точками для объёма. Ещё предложения.\n\n"
            ));
        }
        let chunks = chunk_boundaries(&text);
        assert!(chunks.len() > 1, "ожидается несколько чанков");
        // покрытие полное и без пересечений
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks.last().unwrap().1, text.len());
        for w in chunks.windows(2) {
            assert_eq!(w[0].1, w[1].0);
        }
        // каждая граница — на границе символа
        for &(s, e) in &chunks {
            assert!(text.is_char_boundary(s));
            assert!(text.is_char_boundary(e));
            // разрез проходит по границе абзаца или заголовка:
            // перед границей или сразу после неё есть перевод строки
            if e < text.len() {
                let lo = {
                    let mut i = e.saturating_sub(4);
                    while i < e && !text.is_char_boundary(i) {
                        i += 1;
                    }
                    i
                };
                let hi = {
                    let mut i = (e + 4).min(text.len());
                    while i > e && !text.is_char_boundary(i) {
                        i -= 1;
                    }
                    i
                };
                let around = &text[lo..hi];
                assert!(
                    around.contains('\n'),
                    "разрез не на границе строки: байт {e}"
                );
            }
        }
    }

    #[test]
    fn truncate_slice_char_safe() {
        let s = "кириллица".repeat(50);
        let t = truncate_slice(&s, 25);
        assert!(t.len() <= 25);
        assert!(s.starts_with(t));
    }

    // ---------- v0.83.0: префикс-семантика мягкого fallback ----------

    #[test]
    fn find_token_prefix_family() {
        // wireshark-кейс: точного токена нет, но есть семейство
        // идентификаторов-продолжений (`proto_register_field_array` —
        // ОДИН токен: разрыв только по не-alnum).
        let text = "void proto_register_field_array(void) {\n\
                    int proto_register_field_init = 0;\n\
                    proto_register_field_array();\n";
        let ft = FileTokens::build(text);
        let hits = ft.find_token_prefix("proto_register_field");
        // 3 вхождения семейства, ни одного ложного
        assert_eq!(hits.len(), 3, "hits={hits:?}");
        // все хиты стоят на токенах префикс-семейства
        for &h in &hits {
            assert!(
                ft.tok(h as usize).starts_with("proto_register_field"),
                "хит не из семейства: {}",
                ft.tok(h as usize)
            );
        }
        // точное равенство НЕ требуется, но короткий префикс ловит больше
        assert_eq!(ft.find_token_prefix("proto_register").len(), 3);
        // пустой префикс — ничего (защита от дегенерации)
        assert!(ft.find_token_prefix("").is_empty());
        // несуществующий префикс — пусто
        assert!(ft.find_token_prefix("zzz_absent").is_empty());
    }

    #[test]
    fn find_token_prefix_case_folded() {
        // токены хранятся фолдом: Wslua_PROTO_register( находит префикс
        let ft = FileTokens::build("int Wslua_PROTO_register(void);");
        let hits = ft.find_token_prefix("wslua_proto");
        assert_eq!(hits.len(), 1, "hits={hits:?}");
        assert_eq!(ft.tok(hits[0] as usize), "wslua_proto_register");
    }

    // ---------- v0.85.0: гэп макро-атрибутов объявления типа ----------

    #[test]
    fn find_type_decl_gaps_macro_families() {
        // LLVM / Qt / Chromium-морфология: макрос экспорта между
        // class/struct и именем типа
        let text = "class LLVM_ABI Function : public GlobalObject {\n\
                    class Q_CORE_EXPORT QObject {\n\
                    struct PLATFORM_EXPORT BMVert {\n\
                    class __attribute__ ((visibility(\"default\"))) Module {\n\
                    struct alignas(16) BMEdge {\n\
                    class [[deprecated]] LegacyAPI {\n";
        let ft = FileTokens::build(text);
        let td = TypeDeclQuery::parse("class Function :").unwrap();
        let hits = ft.find_type_decl_gaps(text, &td);
        assert_eq!(hits.len(), 1, "hits={hits:?}");
        // хит — токен keyword в начале строки объявления
        assert_eq!(ft.tok(hits[0] as usize), "class");

        for (query, name) in [
            ("class QObject", "Q_CORE_EXPORT"),
            ("struct BMVert {", "PLATFORM_EXPORT"),
            ("class Module", "__attribute__"),
            ("struct BMEdge {", "alignas"),
            ("class LegacyAPI", "[[deprecated]]"),
        ] {
            let td = TypeDeclQuery::parse(query).unwrap();
            let hits = ft.find_type_decl_gaps(text, &td);
            assert_eq!(hits.len(), 1, "{query}: hits={hits:?}");
            let _ = name;
        }
    }

    #[test]
    fn find_type_decl_gaps_rejects_non_macros() {
        // без гэпа (точная фраза), enum-префикс, комментарий,
        // не-в-начале-строки, квалифицированное имя, lowercase-гэп
        let text = "class Function {\n\
                    enum class Color : int {\n\
                    // class LLVM_ABI Function : public X {\n\
                    void f() { class Local Foo; }\n\
                    class llvm::Function;\n\
                    class my_macro Function;\n\
                    friend class Function;\n";
        let ft = FileTokens::build(text);
        let td = TypeDeclQuery::parse("class Function :").unwrap();
        let hits = ft.find_type_decl_gaps(text, &td);
        assert!(hits.is_empty(), "ложные гэп-хиты: {hits:?}");
    }

    #[test]
    fn find_type_decl_gaps_case_sensitive_name() {
        // имя чувствительно к регистру (как якорь сигнатуры)
        let text = "class LLVM_ABI function : public X {\n";
        let ft = FileTokens::build(text);
        let td = TypeDeclQuery::parse("class Function :").unwrap();
        assert!(ft.find_type_decl_gaps(text, &td).is_empty());
    }

    #[test]
    fn type_decl_kind_after_forms() {
        use super::TypeDeclKind::{Declaration, Definition};
        let cases: &[(&str, TypeDeclKind)] = &[
            ("class Foo {", Definition),          // тело на той же строке
            ("class Foo : public Bar {", Definition), // базовый класс
            ("class Foo final : public Bar {", Definition), // final между
            ("struct Foo\n{", Definition),         // тело на следующей строке
            ("struct Foo\n    : public Bar {", Definition), // база ниже
            ("class Foo;", Declaration),           // forward-декларация
            ("struct Foo x;", Declaration),        // elaborated-описатель
            ("class Foo is used", Declaration),    // проза
            ("class Foo:", Definition),            // Python-класс
        ];
        for (line, want) in cases {
            let name_end = line.find("Foo").unwrap() + "Foo".len();
            let got = super::type_decl_kind_after(line, name_end);
            assert_eq!(got, *want, "{line}: got={got:?}");
        }
    }

    #[test]
    fn has_type_def_exact_signal() {
        // точная фраза нашлась: def-сигнал различает определение
        // и forward-декларацию (гасит/не гасит гэп-fallback)
        let def = "class Function {\n  int x;\n};\n";
        let ft = FileTokens::build(def);
        let td_hits = ft.find_phrase_strict(&["class".into(), "function".into()]);
        assert!(!td_hits.is_empty());
        assert!(ft.has_type_def_exact(def, &td_hits));

        let fwd = "class Function;\nvoid use(Function *f);\n";
        let ft2 = FileTokens::build(fwd);
        let hits2 = ft2.find_phrase_strict(&["class".into(), "function".into()]);
        assert!(!hits2.is_empty());
        assert!(!ft2.has_type_def_exact(fwd, &hits2));

        // friend-декларация — не определение (не в начале строки)
        let friend = "friend class Function;\n";
        let ft3 = FileTokens::build(friend);
        let hits3 = ft3.find_phrase_strict(&["class".into(), "function".into()]);
        assert!(!ft3.has_type_def_exact(friend, &hits3));
    }

    // ---------- v0.86.0: template-гэп объявления типа (ГРАБЛЯ 45) ----------

    #[test]
    fn find_template_decl_gaps_boost_morphology() {
        // Boost.Spirit-морфология: параметрический блок <...> между
        // template и struct; строгая фраза `template struct phrase_parse`
        // к этому тексту слепа
        let text = "template <typename Iterator, typename Enable = void>\n\
                    struct phrase_parse_impl {\n\
                        void run() {}\n\
                    };\n\
                    template <typename T> struct token_printer_debug {\n\
                        T t;\n\
                    };\n";
        let ft = FileTokens::build(text);
        let td = TypeDeclQuery::parse("template struct phrase_parse_impl").unwrap();
        assert!(td.template);
        let hits = ft.find_template_decl_gaps(text, &td);
        assert_eq!(hits.len(), 1, "hits={hits:?}");
        // хит — токен `template` в начале строки объявления
        assert_eq!(ft.tok(hits[0] as usize), "template");
        // второе объявление — тоже находится
        let td2 = TypeDeclQuery::parse("template struct token_printer_debug").unwrap();
        let hits2 = ft.find_template_decl_gaps(text, &td2);
        assert_eq!(hits2.len(), 1, "hits={hits2:?}");
        // однострочная форма (Boost также пишет в одну строку)
        let single = "template <typename Iterator, typename Attribute> struct phrase_parse_impl { void run(); };\n";
        let fts = FileTokens::build(single);
        let hits3 = fts.find_template_decl_gaps(single, &td);
        assert_eq!(hits3.len(), 1, "hits3={hits3:?}");
    }

    #[test]
    fn find_template_decl_gaps_nested_params_and_rejects() {
        // вложенные угловые скобки в параметрах (контейнеры) + отказы:
        // имя из блока, class-key не тот, template без блока
        let text = "template <typename T, std::vector<int> V> struct Holder {\n\
                    int cmp(int a, int b) { return a < b; }\n\
                    template <typename T> struct Plain {\n\
                        T t;\n\
                    };\n";
        let ft = FileTokens::build(text);
        let td = TypeDeclQuery::parse("template struct Holder").unwrap();
        let hits = ft.find_template_decl_gaps(text, &td);
        assert_eq!(hits.len(), 1, "вложенные <> потеряны: {hits:?}");
        // имя из параметрического блока НЕ принимается за имя типа
        // (окно имени — только ЗА class-key)
        let td2 = TypeDeclQuery::parse("template struct vector").unwrap();
        assert!(ft.find_template_decl_gaps(text, &td2).is_empty());
        // class-key не тот: в файле struct, запросили class — мимо
        let td3 = TypeDeclQuery::parse("template class Plain").unwrap();
        assert!(ft.find_template_decl_gaps(text, &td3).is_empty());
        // тот же class-key — находка
        let td3b = TypeDeclQuery::parse("template struct Plain").unwrap();
        assert_eq!(ft.find_template_decl_gaps(text, &td3b).len(), 1);
        // template без параметров вообще (сырой < отсутствует) — мимо:
        let text2 = "template struct phrase_parse { };\n";
        let ft2 = FileTokens::build(text2);
        let td4 = TypeDeclQuery::parse("template struct phrase_parse").unwrap();
        // нет блока <...> — гэп-паттерн не срабатывает (точная фраза
        // сама найдёт: [template, struct, phrase_parse] подряд)
        assert!(ft2.find_template_decl_gaps(text2, &td4).is_empty());
    }
}
