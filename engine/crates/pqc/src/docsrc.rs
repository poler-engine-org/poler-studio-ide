//! RQ22: источник документаций — markdown/текст через HTTPS.
//!
//! Второй чистый источник знаний для `pqc learn` (первый — Wikipedia
//! API, RQ19): файлы документаций и markdown-репозиториев —
//! `raw.githubusercontent.com`, doc-сайты, просто `.md`/`.txt` по
//! прямой ссылке. Без HTML-скрейпинга: файл уже текстовый — нужна
//! только чистка **разметки markdown**, чтобы в мозг не попадал
//! синтаксический шум (решётки заголовков, звёздочки акцентов,
//! URL ссылок, код в блоках-ограждениях).
//!
//! ## Чистка markdown → текст
//!
//! [`clean_markdown`] — чистая функция (детерминизм, ноль ГПСЧ):
//!
//! ```text
//!   ``` … ```        блоки кода — ВЫРЕЗАЮТСЯ (код не речь)
//!   [текст](url)     → текст            (ссылки оставляют якорь)
//!   ![alt](url)      → «»               (картинки уходят)
//!   <https://…>      → «»               (автоссылки уходят)
//!   http(s)://…      → «»               (голые URL — шум)
//!   # ## ###         маркеры заголовков → «»
//!   * _ ~ ** __ ~~   акценты → «» (кроме _внутри_слов: snake_case)
//!   > цитаты, списки - * + 1. → маркеры уходят
//!   | a | b |        строки таблиц → ячейки пробелами, разделители — вон
//!   <теги>           → strip_html (внутри markdown живёт HTML)
//!   \* \_            эскейпы → символ
//!   BOM/CRLF         нормализуются, пустые серии схлопываются
//! ```
//!
//! ## Вежливость
//!
//! Та же политика, что у Wikipedia-источника: идентифицирующийся
//! `User-Agent`, пауза ≥ 300 мс между файлами, backoff на 403/429.
//!
//! ## Пример
//!
//! ```
//! use pqc::docsrc::clean_markdown;
//!
//! let md = "# Заголовок\n\nКвантовая **механика** — [физика](https://x.y) микромира.\n";
//! let text = clean_markdown(md);
//! assert!(text.contains("Квантовая механика"));
//! assert!(!text.contains('['));
//! assert!(!text.contains('#'));
//! assert!(!text.contains("**"));
//! ```

use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::netfetch::{HttpClient, NetError, Url};

/// Пауза между загрузками файлов (вежливость к doc-хостингу).
const POLITE_PAUSE: Duration = Duration::from_millis(300);
/// Пауза backoff при 403/429.
const BACKOFF: Duration = Duration::from_secs(2);
/// Максимум байт текста одного документа (10 МиБ — как MAX_BODY).
const MAX_DOC_BYTES: usize = 10 * 1024 * 1024;

/// Источник документаций: очередь HTTPS-ссылок на файлы
/// (`.md`, `.txt`, любой текст).
///
/// `search` выдаёт ещё не загруженные документы (заголовок — базовое
/// имя файла), `extracts` забирает их по HTTP, чистит markdown и
/// отдаёт чистый текст. Раунды самоуправляемого поиска RQ19 не
/// применяются: список файлов дан явно — раунд 2 пуст, `learn`
/// завершается честно.
pub struct DocsSource {
    client: HttpClient,
    /// Очередь `(заголовок, URL)` — ещё не выданные документы.
    pending: Vec<(String, String)>,
    /// Кэш `(заголовок → URL)` для extracts.
    by_title: Vec<(String, String)>,
    last_request: Option<Instant>,
}

impl DocsSource {
    /// Источник из списка HTTPS-ссылок. Ссылки без схемы получают
    /// префикс `https://` (сырые пути — опечатка, не атака).
    pub fn new(urls: &[String]) -> DocsSource {
        let mut pending = Vec::with_capacity(urls.len());
        for u in urls {
            let full = if u.starts_with("https://") || u.starts_with("http://") {
                u.clone()
            } else {
                format!("https://{u}")
            };
            pending.push((doc_title(&full), full));
        }
        DocsSource {
            client: HttpClient::default(),
            pending,
            by_title: Vec::new(),
            last_request: None,
        }
    }

    /// Число документов в очереди.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Загрузка и чистка одного файла (backoff на rate-limit).
    fn fetch_doc(&mut self, url: &str) -> Result<String, NetError> {
        let parsed = Url::parse(url)?;
        let mut attempts = 0u32;
        loop {
            self.polite_wait();
            match self.client.get(&parsed) {
                Ok(resp) if resp.ok() => {
                    if resp.body.len() > MAX_DOC_BYTES {
                        return Err(NetError::BadResponse(
                            "документ больше 10 МиБ — разрежите источник",
                        ));
                    }
                    let raw = String::from_utf8_lossy(&resp.body).into_owned();
                    return Ok(clean_markdown(&raw));
                }
                Ok(resp) if (resp.status == 403 || resp.status == 429) && attempts < 2 => {
                    attempts += 1;
                    sleep(BACKOFF * attempts);
                }
                Ok(resp) => return Err(NetError::Status(resp.status)),
                Err(NetError::Io(ref e))
                    if e.kind() == std::io::ErrorKind::WouldBlock && attempts < 2 =>
                {
                    attempts += 1;
                    sleep(BACKOFF);
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Пауза ≥ POLITE_PAUSE между реальными запросами.
    fn polite_wait(&mut self) {
        if let Some(t) = self.last_request {
            let elapsed = t.elapsed();
            if elapsed < POLITE_PAUSE {
                sleep(POLITE_PAUSE - elapsed);
            }
        }
        self.last_request = Some(Instant::now());
    }
}

/// Заголовок документа из URL: последнее звено пути, `%XX`
/// декодируется, расширение отбрасывается, дефисы/подчёркивания —
/// пробелы. Пустой путь (только хост) — хост целиком.
fn doc_title(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    // Хост-онли (схема://хост без пути): ≥ 3 слэшей = есть путь.
    let has_path = path.matches('/').count() >= 3;
    let last = path.rsplit('/').find(|s| !s.is_empty()).unwrap_or("");
    if last.is_empty() || !has_path {
        return path
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_string();
    }
    let no_ext = match last.rfind('.') {
        // Расширение отбрасываем, только если после точки ≤ 5 символов
        // (файл): «v1.2.3-release» — версия, не расширение.
        Some(i) if last.len() - i <= 6 && i > 0 => &last[..i],
        _ => last,
    };
    percent_decode_basic(no_ext).replace(['-', '_'], " ")
}

/// Минимальный percent-decode: `%XX` → байт (UTF-8 соберётся сам).
fn percent_decode_basic(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| -> Option<u8> {
                match b {
                    b'0'..=b'9' => Some(b - b'0'),
                    b'a'..=b'f' => Some(b - b'a' + 10),
                    b'A'..=b'F' => Some(b - b'A' + 10),
                    _ => None,
                }
            };
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Предпроход таблиц: строка-разделитель `|---|---|` уходит целиком,
/// строки-ячейки `| a | b |` получают пайпы → пробелы. Кодовые заборы
/// не трогаются (внутри кода пайпы легальны).
fn preprocess_tables(src: &str) -> String {
    let mut out_lines: Vec<String> = Vec::new();
    let mut in_fence = false;
    for line in src.split('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out_lines.push(line.to_string());
            continue;
        }
        if in_fence {
            out_lines.push(line.to_string());
            continue;
        }
        if trimmed.starts_with('|') {
            if trimmed.chars().all(|c| matches!(c, '-' | '|' | ':' | ' ' | '\t')) {
                continue; // разделитель таблицы — строка уходит
            }
            // Строка таблицы: пайпы → пробелы (данные остаются).
            out_lines.push(line.replace('|', " "));
            continue;
        }
        out_lines.push(line.to_string());
    }
    out_lines.join("\n")
}

/// Чистка markdown до естественного текста (чистая функция).
///
/// Порядок проходов важен: таблицы (построчно) → блоки кода →
/// inline-конструкции → HTML-остатки → схлопывание пустых серий.
pub fn clean_markdown(src: &str) -> String {
    // BOM и CRLF нормализуются до обработки; таблицы — предпроходом.
    let src = src.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let src = preprocess_tables(&src);

    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut i = 0usize;
    let n = chars.len();

    while i < n {
        let c = chars[i];

        // ---- Блоки кода: ``` … ``` (и ~~~ … ~~~) — вырезаются целиком.
        if (c == '`' || c == '~') && fence_len_at(&chars, i) >= 3 {
            let fence_char = c;
            let fence_len = fence_len_at(&chars, i);
            i += fence_len;
            // До закрывающего забора той же длины (или конца файла).
            while i < n {
                if chars[i] == fence_char && fence_len_at(&chars, i) >= fence_len {
                    i += fence_len_at(&chars, i);
                    break;
                }
                i += 1;
            }
            push_space(&mut out);
            continue;
        }

        // ---- Картинки: ![alt](url) — уходят целиком.
        if c == '!' && i + 1 < n && chars[i + 1] == '[' {
            if let Some((_, _, end)) = bracket_link(&chars, i + 1) {
                i = end;
                push_space(&mut out);
                continue;
            }
        }

        // ---- Ссылки: [текст](url) → текст.
        if c == '[' {
            if let Some((ts, te, end)) = bracket_link(&chars, i) {
                for &ch in &chars[ts..te] {
                    out.push(ch);
                }
                i = end;
                continue;
            }
        }

        // ---- Автоссылки: <https://…> — уходят.
        if c == '<' && is_autolink_at(&chars, i) {
            i = skip_until_gt(&chars, i);
            push_space(&mut out);
            continue;
        }

        // ---- Голые URL: http(s)://… до пробела — уходят.
        if (c == 'h' || c == 'H') && url_starts_at(&chars, i) {
            i = skip_url(&chars, i);
            push_space(&mut out);
            continue;
        }

        // ---- Заголовки: строка, начинающаяся с #…# — маркеры уходят.
        if c == '#' && (i == 0 || chars[i - 1] == '\n') {
            while i < n && chars[i] == '#' {
                i += 1;
            }
            if i < n && chars[i] == ' ' {
                i += 1;
            }
            continue;
        }

        // ---- Маркеры списков/цитат в начале строки: «- », «* »,
        // «+ », «1. », «>».
        if (i == 0 || chars[i - 1] == '\n') && is_line_marker(c) {
            if c.is_ascii_digit() {
                let mut j = i;
                while j < n && chars[j].is_ascii_digit() {
                    j += 1;
                }
                if j < n && chars[j] == '.' && j + 1 < n && chars[j + 1] == ' ' {
                    i = j + 2;
                    continue;
                }
            } else if c == '>' {
                // «>» (в т.ч. каскад »>>>) — маркер цитаты уходит.
                while i < n && chars[i] == '>' {
                    i += 1;
                }
                if i < n && chars[i] == ' ' {
                    i += 1;
                }
                continue;
            } else if i + 1 < n && chars[i + 1] == ' ' {
                i += 2;
                continue;
            }
        }

        // ---- Горизонтальные линии: --- / *** / ___ в начале строки.
        if (c == '-' || c == '*' || c == '_') && (i == 0 || chars[i - 1] == '\n') {
            let mut j = i;
            while j < n && chars[j] == c {
                j += 1;
            }
            if j - i >= 3 && (j >= n || chars[j] == '\n') {
                i = j;
                continue;
            }
        }

        // ---- Акценты: * ~ ` — уходят; _ — только не внутри слова
        // (snake_case — идентификатор, не акцент).
        if matches!(c, '*' | '~' | '`') {
            i += 1;
            push_space(&mut out);
            continue;
        }
        if c == '_'
            && !(i > 0
                && i + 1 < n
                && chars[i - 1].is_alphanumeric()
                && chars[i + 1].is_alphanumeric())
        {
            i += 1;
            push_space(&mut out);
            continue;
        }

        // ---- Эскейпы: \* → *.
        if c == '\\'
            && i + 1 < n
            && matches!(
                chars[i + 1],
                '*' | '_' | '~' | '`' | '#' | '[' | ']' | '\\' | '!' | '(' | ')'
            )
        {
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }

        out.push(c);
        i += 1;
    }

    // ---- HTML-остатки (markdown живёт с встроенным HTML) — вычищаем
    // переиспользуемым стриппером; финальная нормализация: пробельные
    // серии внутри строк схлопываются, пустые строки — не более одной
    // подряд (абзацный ритм), хвосты строк триммятся.
    let stripped = crate::stream_engine::strip_html(out.as_bytes());
    finalize_text(&stripped)
}

/// Схлопывание пробелов внутри строки + трим хвостов.
fn collapse_line_ws(line: &str) -> String {
    let mut norm = String::with_capacity(line.len());
    let mut pending_space = false;
    let mut started = false;
    for ch in line.chars() {
        if ch == ' ' || ch == '\t' {
            if started {
                pending_space = true;
            }
        } else {
            if pending_space {
                norm.push(' ');
                pending_space = false;
            }
            norm.push(ch);
            started = true;
        }
    }
    norm
}

/// Финальная нормализация: строки триммятся, пробельные серии — в один
/// пробел, пустые строки — не более одной подряд (абзацный ритм).
fn finalize_text(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut blank = 0usize;
    for line in src.split('\n') {
        let norm = collapse_line_ws(line);
        if norm.is_empty() {
            blank += 1;
        } else {
            blank = 0;
        }
        if blank <= 1 {
            out.push_str(&norm);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

/// Длина кодового забора в позиции (3+ одинаковых ` или ~).
fn fence_len_at(chars: &[char], i: usize) -> usize {
    let c = chars[i];
    let mut j = i;
    while j < chars.len() && chars[j] == c {
        j += 1;
    }
    j - i
}

/// Ссылка `[текст](url)` от открывающей `[`:
/// `(начало текста, конец текста, позиция за «)»)` или `None`,
/// если конструкция не ссылка.
fn bracket_link(chars: &[char], i: usize) -> Option<(usize, usize, usize)> {
    let n = chars.len();
    // Парная ] без учёта вложенности (вложенные [] в тексте ссылки —
    // крайняя редкость, [1] сноски не дают `(…)` после).
    let mut depth = 0usize;
    let mut j = i;
    let mut close = None;
    while j < n {
        match chars[j] {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(j);
                    break;
                }
            }
            _ => {}
        }
        j += 1;
    }
    let close = close?;
    // Сразу после ] обязан идти ( … ) без пустой строки.
    if close + 1 >= n || chars[close + 1] != '(' {
        return None;
    }
    let mut k = close + 2;
    while k < n && chars[k] != ')' {
        if chars[k] == '\n' && k + 1 < n && chars[k + 1] == '\n' {
            return None; // разрыв абзаца — не ссылка
        }
        k += 1;
    }
    if k >= n {
        return None;
    }
    Some((i + 1, close, k + 1))
}

/// `<https://…>` — автоссылка в углах.
fn is_autolink_at(chars: &[char], i: usize) -> bool {
    let rest: String = chars[i..(i + 9).min(chars.len())].iter().collect();
    let lower = rest.to_ascii_lowercase();
    lower.starts_with("<http://") || lower.starts_with("<https://") || lower.starts_with("<ftp://")
}

/// Пропустить до `>` (включительно).
fn skip_until_gt(chars: &[char], i: usize) -> usize {
    let mut j = i;
    while j < chars.len() && chars[j] != '>' {
        j += 1;
    }
    (j + 1).min(chars.len())
}

/// `http://` или `https://` в позиции.
fn url_starts_at(chars: &[char], i: usize) -> bool {
    let probe: String = chars[i..(i + 9).min(chars.len())].iter().collect();
    let lower = probe.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Пропустить URL до первого пробельного символа / `>` / `)`
/// (хвостовые `.` и `,` — пунктуация, не часть URL).
fn skip_url(chars: &[char], i: usize) -> usize {
    let mut j = i;
    while j < chars.len() && !chars[j].is_whitespace() && chars[j] != '>' && chars[j] != ')' {
        if j + 1 < chars.len()
            && (chars[j] == '.' || chars[j] == ',')
            && chars[j + 1].is_whitespace()
        {
            break;
        }
        j += 1;
    }
    j
}

/// Маркер ли начала строки: список или цитата.
fn is_line_marker(c: char) -> bool {
    matches!(c, '-' | '*' | '+' | '>' | '0'..='9')
}

/// Пробел-заполнитель (не даёт словам склеиться на месте разрезов).
fn push_space(out: &mut String) {
    if !out.ends_with(' ') && !out.ends_with('\n') {
        out.push(' ');
    }
}

/// Адаптер документаций под пайплайн [`crate::learn_net::learn`].
impl crate::learn_net::TextSource for DocsSource {
    fn search(&mut self, query: &str, limit: usize) -> Result<Vec<(String, String)>, String> {
        // Запрос раунда — тема learn; выдаём ещё не отданные документы
        // (до limit). Раунды 2+ получают пустоту — весь список уже
        // выдан, самоуправляемый поиск не нужен.
        let _ = query;
        let mut out = Vec::new();
        while out.len() < limit && !self.pending.is_empty() {
            let (title, url) = self.pending.remove(0);
            self.by_title.push((title.clone(), url.clone()));
            out.push((title, url));
        }
        Ok(out)
    }

    fn extracts(&mut self, titles: &[String], _intro: bool) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::with_capacity(titles.len());
        for title in titles {
            let Some(url) = self
                .by_title
                .iter()
                .find(|(t, _)| t == title)
                .map(|(_, u)| u.clone())
            else {
                continue;
            };
            let text = self.fetch_doc(&url).map_err(|e| format!("{url}: {e}"))?;
            if !text.trim().is_empty() {
                out.push((title.clone(), text));
            }
        }
        Ok(out)
    }
}

// ============================================================================
// Тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learn_net::TextSource;

    #[test]
    fn title_from_url() {
        assert_eq!(
            doc_title("https://raw.githubusercontent.com/o/r/main/docs/guide.md"),
            "guide"
        );
        assert_eq!(doc_title("https://example.com/Quantum-Physics.txt"), "Quantum Physics");
        assert_eq!(doc_title("https://example.com/a/b/notes_v2.md"), "notes v2");
        // Версия с точками — не расширение (после точки > 5 символов).
        assert_eq!(doc_title("https://example.com/release/v1.2.3-release"), "v1.2.3 release");
        // Пустой путь — хост.
        assert_eq!(doc_title("https://example.com"), "example.com");
        // Кириллица в percent-кодировке.
        assert_eq!(doc_title("https://x.y/%D0%BA%D0%B2%D0%B0%D0%BD%D1%82.md"), "квант");
    }

    #[test]
    fn clean_headers_and_emphasis() {
        let md = "# Квантовая механика\n\n## Основы\n\nФизика **микромира** изучает *волны* и ~~частицы~~.";
        let t = clean_markdown(md);
        assert!(t.contains("Квантовая механика"));
        assert!(t.contains("Основы"));
        assert!(t.contains("Физика микромира изучает волны и частицы"));
        assert!(!t.contains('#'));
        assert!(!t.contains('*'));
        assert!(!t.contains("~~"));
        assert!(!t.contains("**"));
    }

    #[test]
    fn clean_links_and_images() {
        let md = "Читайте [документацию](https://x.y/a) и смотрите ![схема](https://x.y/i.png) тут.";
        let t = clean_markdown(md);
        assert!(t.contains("Читайте документацию"), "текст: {t}");
        assert!(!t.contains("https://"), "URL вырезан: {t}");
        assert!(!t.contains('['));
        assert!(!t.contains("схема"));
    }

    #[test]
    fn clean_bare_urls_and_autolinks() {
        let md = "Источник: https://example.com/page, и <https://other.ru> тоже.";
        let t = clean_markdown(md);
        assert!(!t.contains("example.com"));
        assert!(!t.contains("other.ru"));
        assert!(!t.contains('<'));
        assert!(t.contains("Источник"));
        assert!(t.contains("тоже"));
    }

    #[test]
    fn clean_code_fences_removed() {
        let md = "Текст до.\n\n```rust\nlet x = 1; // *не* **чистится**\nfn main() {}\n```\n\nТекст после.";
        let t = clean_markdown(md);
        assert!(t.contains("Текст до"));
        assert!(t.contains("Текст после"));
        assert!(!t.contains("fn main"));
        assert!(!t.contains("let x"));
    }

    #[test]
    fn clean_lists_quotes_tables() {
        let md = "- первый пункт\n- второй пункт\n\n> Цитата мудреца\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n1. нумерованный\n2. список";
        let t = clean_markdown(md);
        assert!(t.contains("первый пункт"));
        assert!(t.contains("второй пункт"));
        assert!(t.contains("Цитата мудреца"));
        assert!(t.contains("нумерованный"));
        assert!(t.contains("список"));
        // Разделитель таблиц ушёл, ячейки остались словами.
        assert!(!t.contains("---"));
        assert!(t.contains("a"));
        assert!(t.contains("b"));
    }

    #[test]
    fn clean_escapes_and_hr() {
        let md = "Звёздочка \\* не акцент.\n\n---\n\nКонец.";
        let t = clean_markdown(md);
        assert!(t.contains("Звёздочка * не акцент"));
        assert!(!t.contains("---"));
        assert!(t.contains("Конец"));
    }

    #[test]
    fn clean_snake_case_underscore_kept() {
        let md = "Полярность p_at и функция born_step_packed4 — идентификаторы кода.";
        let t = clean_markdown(md);
        assert!(t.contains("p_at"), "внутрисловные _ сохранены: {t}");
        assert!(t.contains("born_step_packed4"), "текст: {t}");
    }

    #[test]
    fn clean_bom_crlf_and_blank_runs() {
        let md = "\u{feff}Первая строка\r\n\r\n\r\n\r\n\r\nВторая строка\r\n";
        let t = clean_markdown(md);
        assert!(t.starts_with("Первая строка"));
        assert!(t.contains("Вторая строка"));
        // Пустые серии схлопнуты до абзацного ритма.
        assert!(!t.contains("\n\n\n"));
    }

    #[test]
    fn clean_inline_html_stripped() {
        let md = "Мозг <b>думает</b> фазами <i>решётки</i>.";
        let t = clean_markdown(md);
        assert!(t.contains("Мозг думает фазами решётки"));
        assert!(!t.contains('<'));
    }

    #[test]
    fn clean_plain_text_passes_through() {
        let plain = "Обычный текст без разметки.\nВторая строка.";
        assert_eq!(clean_markdown(plain), "Обычный текст без разметки.\nВторая строка.");
    }

    #[test]
    fn clean_reference_style_link_falls_back() {
        // Ссылки в стиле [текст][ref] — не inline: скобки остаются
        // просто текстом (не ломаем содержимое).
        let md = "См. [руководство][ref1] в приложении.";
        let t = clean_markdown(md);
        // bracket_link не сработал (нет `(…)` сразу после ]) — текст
        // прошёл как есть; акцентов нет.
        assert!(t.contains("руководство"));
    }

    #[test]
    fn docs_source_search_rounds() {
        // Раунд 1 выдаёт все документы, раунд 2 — пустоту (честный
        // конец: список дан явно).
        let mut src = DocsSource::new(&[
            "https://example.com/a.md".into(),
            "https://example.com/b.md".into(),
        ]);
        assert_eq!(src.pending_len(), 2);
        let r1 = src.search("тема", 5).unwrap();
        assert_eq!(r1.len(), 2);
        assert_eq!(r1[0].0, "a");
        assert_eq!(r1[0].1, "https://example.com/a.md");
        let r2 = src.search("тема", 5).unwrap();
        assert!(r2.is_empty(), "раунд 2 пуст");
    }

    #[test]
    fn docs_source_normalizes_scheme() {
        let mut src = DocsSource::new(&["raw.githubusercontent.com/o/r/main/README.md".into()]);
        let hits = src.search("x", 5).unwrap();
        assert_eq!(
            hits[0].1,
            "https://raw.githubusercontent.com/o/r/main/README.md"
        );
        assert_eq!(hits[0].0, "README");
    }

    /// Живая загрузка markdown (запускать явно:
    /// `cargo test -p pqc --lib docsrc::tests::live_docs_fetch -- --ignored`).
    #[test]
    #[ignore = "живой интернет: смоук источника документаций"]
    fn live_docs_fetch() {
        let mut src = DocsSource::new(&[
            "https://raw.githubusercontent.com/Kotokvit/POLER-Quantum-RS/main/README.md".into(),
        ]);
        let hits = src.search("POLER", 5).unwrap();
        assert_eq!(hits.len(), 1);
        let pages = src.extracts(&["README".to_string()], false).unwrap();
        assert_eq!(pages.len(), 1);
        let (title, text) = &pages[0];
        assert_eq!(title, "README");
        assert!(text.len() > 500, "README слишком короткий после чистки: {}", text.len());
        assert!(!text.contains("]("), "markdown-ссылки не вычищены");
        assert!(!text.contains("```"), "кодовые заборы не вырезаны");
    }
}
