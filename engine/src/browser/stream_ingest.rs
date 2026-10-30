//! Потоковый инжест веб-страниц (директива
//! docs/DIRECTIVE_STREAMING_INGESTION_PIPELINE.md, §2A).
//!
//! HTTP-чанки разжимаются и раздаются токенайзеру ПО МЕРЕ ПРИХОДА:
//! полная страница НИКОГДА не собирается в RAM. Токенайзер —
//! конечный автомат с UTF-8-хвостом (символ, разрезанный границей
//! чанка, переносится), `<script>/<style>` выбрасываются целиком,
//! сущности декодируются, ссылки и `<meta description>` собираются.
//!
//! ε-фильтр ([`EpsilonStreamFilter`]) оценивает информационную
//! плотность каждого текстового блока (тот же порог 1.2, что у
//! DOM-фильтра `browser::filter`): навигационный мусор, куки-баннеры
//! и ссылки-простыни уходят, семантика, код и таблицы остаются.
//!
//! [`StreamingFetcher`] реализует `web::crawl::PageFetcher` поверх
//! ureq — краул без Chromium (CDP остаётся для JS-тяжёлых сайтов).

use std::io::Read;

use crate::web::crawl::{FetchedPage, PageFetcher};

/// Статистика одной страницы.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct StreamPageStats {
    pub html_bytes: usize,
    pub blocks_seen: usize,
    pub blocks_kept: usize,
    pub clean_text_len: usize,
    pub links: usize,
    pub truncated: bool,
}

/// Результат потокового разбора страницы.
#[derive(Debug, Clone)]
pub struct StreamPage {
    pub title: String,
    pub meta_description: String,
    pub text: String,
    pub links: Vec<String>,
    pub stats: StreamPageStats,
}

/// Пороговое значение ε по умолчанию для ПОТОКОВОГО инжеста: 1.0
/// (recall-ориентир: краул ценит каждое смысловое предложение).
/// DOM-фильтр `browser::filter` агрессивнее (1.2) — он чистит одну
/// конкретную страницу, а не собирает корпус.
pub const DEFAULT_MIN_EPSILON: f32 = 1.0;

/// Максимальный размер страницы (защита RAM), 32 МиБ.
pub const DEFAULT_PAGE_MAX_BYTES: usize = 32 * 1024 * 1024;

// ───────────────────────── токенайзер ─────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ts {
    Data,
    TagOpen,      // после '<'
    Decl,         // после '<!': '-' → комментарий, иначе doctype
    TagName,      // имя тега
    AttrName,     // имя атрибута
    BeforeAttrVal,
    AttrValDq,    // "..."
    AttrValSq,    // '...'
    AttrValBare,  // до пробела/'>'/'/'
    RawText,      // script/style: до </tag
    Comment,      // <!-- -->
    Doctype,      // <!doctype ...> (ждём '>')
    AfterTagOpenSlash,
}

/// Потоковый сборщик страницы: кормим байтами, достаём семантику.
pub struct StreamPageBuilder {
    /// Хвост незавершённого UTF-8 символа (≤ 3 байт).
    utf8_tail: Vec<u8>,
    state: Ts,
    /// Накопитель текущего тега (имя), атрибутов.
    tag_buf: String,
    attr_name: String,
    attr_val: String,
    attrs: Vec<(String, String)>,
    /// Текущее имя rawtext-тега (script/style) для поиска закрытия.
    raw_tag: String,
    /// Готовая строка "</script" для сравнения (без аллокаций в цикле).
    raw_tag_close: String,
    /// Буфер проверки "</script" без аллокаций на каждый байт.
    close_probe: String,
    /// Текущий тег закрывающий (после '</').
    pending_close: bool,
    /// Глубина вложенности <a> (текст внутри ссылки — метка ссылки).
    a_depth: u32,
    /// Символов текста, набранных внутри <a> (для штрафа навигации).
    link_label_chars: usize,
    /// Накопитель текстового узла.
    text_buf: String,
    // семантика
    title: String,
    in_title: bool,
    meta_description: String,
    links: Vec<String>,
    /// Уровень вложенности выброшенных тегов (script/style/noscript/svg).
    skip_depth: u32,
    pre_depth: u32,
    table_depth: u32,
    heading: u8,
    filter: EpsilonStreamFilter,
    max_page_bytes: usize,
    html_bytes: usize,
}

impl StreamPageBuilder {
    pub fn new(min_epsilon: f32, max_page_bytes: usize) -> Self {
        StreamPageBuilder {
            utf8_tail: Vec::new(),
            state: Ts::Data,
            tag_buf: String::new(),
            attr_name: String::new(),
            attr_val: String::new(),
            attrs: Vec::new(),
            raw_tag: String::new(),
            raw_tag_close: String::new(),
            close_probe: String::new(),
            pending_close: false,
            a_depth: 0,
            link_label_chars: 0,
            text_buf: String::new(),
            title: String::new(),
            in_title: false,
            meta_description: String::new(),
            links: Vec::new(),
            skip_depth: 0,
            pre_depth: 0,
            table_depth: 0,
            heading: 0,
            filter: EpsilonStreamFilter::new(min_epsilon),
            max_page_bytes,
            html_bytes: 0,
        }
    }

    /// Скармливаем очередные сырые байты HTTP-тела.
    pub fn push_chunk(&mut self, bytes: &[u8]) {
        let mut buf = std::mem::take(&mut self.utf8_tail);
        buf.extend_from_slice(bytes);
        loop {
            match std::str::from_utf8(&buf) {
                Ok(s) => {
                    self.consume(s);
                    buf.clear();
                    break;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    if valid > 0 {
                        // безопасно: валидный префикс по определению valid_up_to
                        let s = unsafe { std::str::from_utf8_unchecked(&buf[..valid]) };
                        self.consume(s);
                        buf.drain(..valid);
                    }
                    match e.error_len() {
                        Some(bad) => {
                            self.consume("\u{FFFD}");
                            let skip = bad.max(1).min(buf.len());
                            buf.drain(..skip);
                        }
                        None => {
                            // незавершённый символ: ждём продолжения,
                            // но не копим больше 3 байт мусора
                            if buf.len() > 3 {
                                self.consume("\u{FFFD}");
                                buf.clear();
                            }
                            break;
                        }
                    }
                }
            }
        }
        self.utf8_tail = buf;
    }

    /// Завершить разбор (EOF тела ответа).
    pub fn finish(mut self, base_url: &str) -> StreamPage {
        self.flush_text();
        let stats = StreamPageStats {
            html_bytes: self.html_bytes,
            blocks_seen: self.filter.blocks_seen,
            blocks_kept: self.filter.blocks_kept,
            clean_text_len: self.filter.out.len(),
            links: self.links.len(),
            truncated: self.html_bytes >= self.max_page_bytes,
        };
        let links = self.resolve_links(base_url);
        StreamPage {
            title: self.title,
            meta_description: self.meta_description,
            text: std::mem::take(&mut self.filter.out),
            links,
            stats,
        }
    }

    fn resolve_links(&self, base_url: &str) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(base) = crate::web::urlnorm::Url::parse(base_url) {
            for href in &self.links {
                let h = href.trim();
                if h.is_empty()
                    || h.starts_with('#')
                    || h.starts_with("javascript:")
                    || h.starts_with("mailto:")
                    || h.starts_with("tel:")
                {
                    continue;
                }
                if let Some(j) = base.join(h) {
                    let s = j.as_str();
                    if (s.starts_with("http://") || s.starts_with("https://"))
                        && !out.contains(&s)
                    {
                        out.push(s);
                    }
                }
            }
        }
        out
    }

    fn consume(&mut self, s: &str) {
        self.html_bytes += s.len();
        for c in s.chars() {
            self.step(c);
        }
    }

    fn step(&mut self, c: char) {
        match self.state {
            Ts::Data => match c {
                '<' => {
                    self.state = Ts::TagOpen;
                    self.pending_close = false;
                    self.tag_buf.clear();
                    self.attrs.clear();
                }
                '&' => {
                    self.state = Ts::Data; // сущности собираются в push_text
                    self.push_text('&');
                }
                _ => self.push_text(c),
            },
            Ts::TagOpen => match c {
                '!' => {
                    self.state = Ts::Decl; // '<!--' или '<!doctype'
                    self.close_probe.clear();
                }
                '?' => self.state = Ts::Doctype,
                '/' => self.state = Ts::AfterTagOpenSlash,
                c if c.is_ascii_alphabetic() => {
                    self.tag_buf.clear();
                    self.tag_buf.push(c.to_ascii_lowercase());
                    self.state = Ts::TagName;
                }
                _ => {
                    // '<' + мусор — это текст
                    self.push_text('<');
                    self.push_text(c);
                    self.state = Ts::Data;
                }
            },
            Ts::Decl => match c {
                '-' => {
                    self.state = Ts::Comment;
                }
                '>' => self.state = Ts::Data,
                _ => self.state = Ts::Doctype,
            },
            Ts::AfterTagOpenSlash => {
                if c.is_ascii_alphabetic() {
                    self.pending_close = true;
                    self.tag_buf.clear();
                    self.tag_buf.push(c.to_ascii_lowercase());
                    self.state = Ts::TagName;
                } else if c == '>' {
                    self.state = Ts::Data;
                } else {
                    self.state = Ts::TagName;
                }
            }
            Ts::TagName => match c {
                '>' => {
                    self.on_tag(self.pending_close);
                    // on_open(script/style) сам переключает в RawText
                    if !matches!(self.state, Ts::RawText) {
                        self.state = Ts::Data;
                    }
                }
                '/' => {
                    self.on_tag(self.pending_close);
                    if matches!(self.state, Ts::RawText) {
                        // <script/> — самозакрытие, raw-тела нет
                        self.raw_tag.clear();
                        self.raw_tag_close.clear();
                    }
                    self.state = Ts::Data;
                }
                c if c.is_whitespace() => self.state = Ts::AttrName,
                c => {
                    if self.tag_buf.len() < 24 {
                        self.tag_buf.push(c.to_ascii_lowercase());
                    }
                }
            },
            Ts::AttrName => match c {
                '>' => {
                    self.on_tag(self.pending_close);
                    if !matches!(self.state, Ts::RawText) {
                        self.state = Ts::Data;
                    }
                }
                '=' => self.state = Ts::BeforeAttrVal,
                c if c.is_whitespace() => {}
                '/' => {}
                c => {
                    if self.attr_name.is_empty() && self.attr_val.is_empty() {
                        self.attr_name.clear();
                    }
                    if self.attr_name.len() < 64 {
                        self.attr_name.push(c.to_ascii_lowercase());
                    }
                }
            },
            Ts::BeforeAttrVal => match c {
                '"' => self.state = Ts::AttrValDq,
                '\'' => self.state = Ts::AttrValSq,
                c if c.is_whitespace() => {}
                c => {
                    self.attr_val.clear();
                    self.attr_val.push(c);
                    self.state = Ts::AttrValBare;
                }
            },
            Ts::AttrValDq => {
                if c == '"' {
                    self.attr_done();
                    self.state = Ts::AttrName;
                } else if self.attr_val.len() < 2048 {
                    self.attr_val.push(c);
                }
            }
            Ts::AttrValSq => {
                if c == '\'' {
                    self.attr_done();
                    self.state = Ts::AttrName;
                } else if self.attr_val.len() < 2048 {
                    self.attr_val.push(c);
                }
            }
            Ts::AttrValBare => {
                if c == '>' {
                    self.attr_done();
                    self.on_tag(self.pending_close);
                    if !matches!(self.state, Ts::RawText) {
                        self.state = Ts::Data;
                    }
                } else if c.is_whitespace() {
                    self.attr_done();
                    self.state = Ts::AttrName;
                } else if self.attr_val.len() < 2048 {
                    self.attr_val.push(c);
                }
            }
            Ts::RawText => {
                // ищем "</raw_tag" (probe держит хвост нужной длины)
                self.close_probe.push(c.to_ascii_lowercase());
                if self.close_probe.len() > self.raw_tag_close.len() {
                    let n = self.close_probe.len();
                    let cut = n - self.raw_tag_close.len();
                    self.close_probe.drain(..cut);
                }
                if self.close_probe == self.raw_tag_close {
                    // до '}' доберёмся в Doctype-подобном ожидании '>'
                    self.skip_depth = self.skip_depth.saturating_sub(1);
                    self.state = Ts::Doctype; // ждём '>' и выходим в Data
                    self.close_probe.clear();
                    self.raw_tag.clear();
                    self.raw_tag_close.clear();
                }
            }
            Ts::Comment => {
                // ждём "-->" (для doctype — '>')
                self.close_probe.push(c);
                if self.close_probe.ends_with("-->") {
                    self.state = Ts::Data;
                    self.close_probe.clear();
                } else if self.close_probe.len() > 8 {
                    let n = self.close_probe.len();
                    self.close_probe.drain(..n - 8);
                }
            }
            Ts::Doctype => {
                if c == '>' {
                    self.state = Ts::Data;
                }
            }
        }
    }

    fn attr_done(&mut self) {
        let name = std::mem::take(&mut self.attr_name);
        let val = std::mem::take(&mut self.attr_val);
        if !name.is_empty() && self.attrs.len() < 16 {
            self.attrs.push((name, val));
        }
    }

    /// Разбор завершённого открывающего/закрывающего тега.
    fn on_tag(&mut self, closing: bool) {
        let tag = self.tag_buf.clone();
        if tag.is_empty() {
            return;
        }
        if closing {
            self.on_close(&tag);
        } else {
            self.on_open(&tag);
        }
    }

    fn on_open(&mut self, tag: &str) {
        match tag {
            "script" | "style" | "noscript" | "svg" | "template" => {
                if tag == "script" || tag == "style" || tag == "template" {
                    self.raw_tag = tag.to_string();
                    self.raw_tag_close = format!("</{tag}");
                    self.state = Ts::RawText;
                } else {
                    self.skip_depth += 1;
                }
            }
            "pre" | "code" => self.pre_depth += 1,
            "table" => self.table_depth += 1,
            "a" => {
                self.a_depth += 1;
                if let Some(href) = self
                    .attrs
                    .iter()
                    .find(|(k, _)| k == "href")
                    .map(|(_, v)| v.clone())
                {
                    if self.links.len() < 4096 {
                        self.links.push(href);
                    }
                }
            }
            "title" => self.in_title = true,
            "meta" => {
                let name = self
                    .attrs
                    .iter()
                    .find(|(k, _)| k == "name")
                    .map(|(_, v)| v.to_ascii_lowercase());
                let content = self
                    .attrs
                    .iter()
                    .find(|(k, _)| k == "content")
                    .map(|(_, v)| v.clone());
                if name.as_deref() == Some("description") {
                    if let Some(c) = content {
                        if c.len() < 1024 {
                            self.meta_description = c;
                        }
                    }
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.flush_text(); // предыдущий блок — БЕЗ heading-бонуса
                self.heading = tag[1..].parse().unwrap_or(0);
            }
            "p" | "div" | "section" | "article" | "li" | "blockquote" | "tr" | "br" | "ul"
            | "ol" | "dl" | "dd" | "dt" | "header" | "footer" | "nav" | "aside" | "main"
            | "figure" | "figcaption" => {
                self.flush_text();
            }
            _ => {}
        }
    }

    fn on_close(&mut self, tag: &str) {
        match tag {
            "a" => self.a_depth = self.a_depth.saturating_sub(1),
            "pre" | "code" => self.pre_depth = self.pre_depth.saturating_sub(1),
            "table" => self.table_depth = self.table_depth.saturating_sub(1),
            "title" => self.in_title = false,
            "p" | "div" | "section" | "article" | "li" | "blockquote" | "tr" | "ul" | "ol"
            | "dl" | "dd" | "dt" | "header" | "footer" | "nav" | "aside" | "main" | "figure"
            | "figcaption" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.flush_text();
                self.heading = 0;
            }
            "script" | "style" | "noscript" | "svg" | "template" => {
                // страховка (RawText обычно сам закрылся)
                self.skip_depth = self.skip_depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    /// Текстовый узел: в title / буфер блока.
    fn push_text(&mut self, c: char) {
        if self.skip_depth > 0 {
            return; // noscript/svg: контент выброшен
        }
        if self.state == Ts::RawText {
            return; // script/style
        }
        if self.in_title {
            if self.title.len() < 512 {
                self.title.push(c);
            }
            return;
        }
        if self.text_buf.len() < 256 * 1024 {
            // пре-блоки сохраняют переводы строк, остальное — схлопываем
            if self.pre_depth > 0 {
                self.text_buf.push(c);
            } else if c == '\n' || c == '\r' || c == '\t' {
                self.text_buf.push(' ');
            } else {
                self.text_buf.push(c);
            }
            if self.a_depth > 0 {
                self.link_label_chars += 1;
            }
        }
    }

    /// Граница блока: текст -> ε-фильтр.
    fn flush_text(&mut self) {
        let mut text = std::mem::take(&mut self.text_buf);
        let link_label_chars = std::mem::take(&mut self.link_label_chars);
        if text.trim().is_empty() {
            // heading НЕ сбрасываем: on_open(h*) выставил его ПОСЛЕ
            // этого flush'а — пустой флаш не должен его затирать
            return;
        }
        // сущности (декодируем здесь, а не посимвольно: реже)
        if text.contains('&') {
            text = decode_entities(&text);
        }
        // заголовок: маркер для фильтра
        if self.heading > 0 {
            let mut marked = String::with_capacity(text.len() + 4);
            marked.push_str("## ");
            marked.push_str(&text);
            text = marked;
        }
        self.filter.push_block(
            &text,
            BlockHints {
                heading: self.heading > 0,
                code: self.pre_depth > 0,
                table: self.table_depth > 0,
                link_label_ratio: if text.chars().count() > 0 {
                    link_label_chars as f32 / text.chars().count() as f32
                } else {
                    0.0
                },
            },
        );
        self.heading = 0;
    }
}

/// Декодирование основных HTML-сущностей.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let end = rest[..rest.len().min(pos + 12)]
            .find(';')
            .map(|e| e + 1)
            .unwrap_or(0);
        if end == 0 {
            out.push('&');
            rest = &rest[1..];
            continue;
        }
        let ent = &rest[..end];
        rest = &rest[end..];
        match ent {
            "&amp;" => out.push('&'),
            "&lt;" => out.push('<'),
            "&gt;" => out.push('>'),
            "&quot;" => out.push('"'),
            "&#39;" | "&apos;" => out.push('\''),
            "&nbsp;" => out.push(' '),
            "&mdash;" => out.push('—'),
            "&laquo;" => out.push('«'),
            "&raquo;" => out.push('»'),
            "&hellip;" => out.push('…'),
            e if e.starts_with("&#") => {
                let num = &e[2..e.len() - 1];
                let cp = if let Some(hex) = num.strip_prefix('x').or_else(|| num.strip_prefix('X'))
                {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    num.parse::<u32>().ok()
                };
                if let Some(ch) = cp.and_then(char::from_u32) {
                    out.push(ch);
                } else {
                    out.push('\u{FFFD}');
                }
            }
            other => out.push_str(other),
        }
    }
    out.push_str(rest);
    out
}

// ───────────────────────── ε-фильтр ─────────────────────────

/// Подсказки контекста блока (от токенайзера).
#[derive(Debug, Clone, Copy, Default)]
pub struct BlockHints {
    pub heading: bool,
    pub code: bool,
    pub table: bool,
    /// Доля символов, набранных внутри <a> (метки ссылок).
    pub link_label_ratio: f32,
}

/// Потоковый ε-фильтр: блок → оценка плотности → сохранить/выбросить.
///
/// ε(блок) = 0.6·min(1, n/24) + 0.9·уникальность + 0.5·кодоподобие
///         + 0.3·min(1, len/200) + бонусы контекста − штрафы
///         (мусорные фразы, ссылочные метки, обрывки, тарабарщина).
pub struct EpsilonStreamFilter {
    pub min_epsilon: f32,
    pub out: String,
    pub blocks_seen: usize,
    pub blocks_kept: usize,
}

const CODE_KEYWORDS: &[&str] = &[
    "fn", "let", "const", "mut", "def", "class", "import", "return", "struct", "impl", "pub",
    "use", "var", "if", "else", "for", "while", "match", "async", "await", "func", "package",
    "static", "void", "int", "char", "bool",
];

const JUNK_PATTERNS: &[&str] = &[
    "cookie", "accept all", "sign in", "sign up", "subscribe", "newsletter", "share on",
    "follow us", "log in", "register", "advertis", "sponsored", "skip to content", "toggle",
    "menu", "copyright ©", "all rights reserved", "javascript is disabled", "enable javascript",
];

impl EpsilonStreamFilter {
    pub fn new(min_epsilon: f32) -> Self {
        EpsilonStreamFilter { min_epsilon, out: String::new(), blocks_seen: 0, blocks_kept: 0 }
    }

    pub fn push_block(&mut self, text: &str, hints: BlockHints) -> bool {
        self.blocks_seen += 1;
        let eps = self.score(text, hints);
        if eps >= self.min_epsilon {
            if !self.out.is_empty() {
                self.out.push_str("\n\n");
            }
            self.out.push_str(text.trim());
            self.blocks_kept += 1;
            true
        } else {
            false
        }
    }

    /// Информационная плотность блока (документированная метрика).
    pub fn score(&self, text: &str, hints: BlockHints) -> f32 {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return 0.0;
        }
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        let n = tokens.len();
        let len = trimmed.chars().count();

        // уникальность (type-token ratio)
        let mut uniq = std::collections::BTreeSet::new();
        let mut alpha = 0usize;
        let mut link_tokens = 0usize;
        let mut kw_hits = 0usize;
        let mut lower_buf;
        for t in &tokens {
            lower_buf = t.to_lowercase();
            uniq.insert(lower_buf.clone());
            let a = t.chars().filter(|c| c.is_alphanumeric()).count();
            if a * 10 >= t.chars().count() * 6 {
                alpha += 1;
            }
            if t.starts_with("http://") || t.starts_with("https://") {
                link_tokens += 1;
            }
            if CODE_KEYWORDS.contains(&lower_buf.as_str()) {
                kw_hits += 1;
            }
        }
        let unique_ratio = if n > 0 { uniq.len() as f32 / n as f32 } else { 0.0 };
        let alpha_ratio = if n > 0 { alpha as f32 / n as f32 } else { 0.0 };

        // кодоподобие: плотность символов синтаксиса + ключевые слова
        // (ключевые слова — мягкий сигнал: нужно ≥6, одно «use» не делает
        // блок кодом — иначе куки-баннеры проходят за код)
        let syms = trimmed
            .chars()
            .filter(|c| matches!(c, '{' | '}' | '(' | ')' | ';' | '=' | '<' | '>' | '[' | ']' | ':' | '|'))
            .count();
        let sym_density = syms as f32 / len.max(1) as f32;
        let code_likeness = (sym_density * 6.0 + (kw_hits as f32 / 6.0).min(1.0)).min(1.0);

        // штрафы: мусорные фразы, ссылочная простыня, обрывки
        let lower = trimmed.to_lowercase();
        let mut junk = 0.0f32;
        for p in JUNK_PATTERNS {
            if lower.contains(p) {
                junk += 0.25;
            }
        }
        let link_density = if n > 0 { link_tokens as f32 / n as f32 } else { 0.0 };
        let short_penalty = if n < 6 && !hints.heading && !hints.table { 0.5 } else { 0.0 };
        let gibberish_penalty = if alpha_ratio < 0.3 && !hints.code && !hints.table { 0.6 } else { 0.0 };
        // навигация: блок почти целиком из меток ссылок
        let nav_penalty = if hints.link_label_ratio > 0.5 {
            hints.link_label_ratio * 1.2
        } else {
            0.0
        };

        let mut eps = 0.6 * (n as f32 / 24.0).min(1.0)
            + 0.9 * unique_ratio
            + 0.5 * code_likeness
            + 0.3 * ((len as f32) / 200.0).min(1.0);
        if hints.heading {
            eps += 0.4;
        }
        if hints.code {
            eps += 0.3;
        }
        if hints.table {
            eps += 0.3;
        }
        eps - junk - link_density * 1.5 - short_penalty - gibberish_penalty - nav_penalty
    }
}

// ───────────────────────── fetcher ─────────────────────────

/// Стриминговый HTTP-фечер без Chromium: тело читается 64-КиБ чанками,
/// страница разбирается на лету, RAM ограничена `page_max_bytes`
/// (токенайзер выбрасывает мусор до того, как он накопится).
pub struct StreamingFetcher {
    pub agent: ureq::Agent,
    pub page_max_bytes: usize,
    pub min_epsilon: f32,
    pub page_timeout_ms: u64,
}

impl StreamingFetcher {
    pub fn new(page_timeout_ms: u64, min_epsilon: f32) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_millis(page_timeout_ms.min(30_000)))
            .user_agent("poler-engine/0.39 (sovereign streaming ingest)")
            .build();
        StreamingFetcher {
            agent,
            page_max_bytes: DEFAULT_PAGE_MAX_BYTES,
            min_epsilon,
            page_timeout_ms,
        }
    }

    /// Разобрать поток тела ответа в страницу.
    pub fn parse_body(&self, body: impl Read, base_url: &str) -> std::io::Result<StreamPage> {
        let mut builder = StreamPageBuilder::new(self.min_epsilon, self.page_max_bytes);
        let mut reader = body;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            builder.push_chunk(&buf[..n]);
            if builder.html_bytes >= self.page_max_bytes {
                break; // RAM-дисциплина: дальше не читаем
            }
        }
        Ok(builder.finish(base_url))
    }
}

impl PageFetcher for StreamingFetcher {
    fn fetch(&mut self, url: &str) -> Result<FetchedPage, String> {
        let resp = self
            .agent
            .get(url)
            .timeout(std::time::Duration::from_millis(self.page_timeout_ms))
            .call()
            .map_err(|e| format!("{url}: {e}"))?;
        let final_url = resp.get_url().to_string();
        let content_type = resp
            .header("content-type")
            .unwrap_or("text/html")
            .to_lowercase();
        let ct_ok = content_type.starts_with("text/")
            || content_type.contains("html")
            || content_type.contains("json")
            || content_type.contains("xml")
            || content_type.contains("markdown");
        let status = resp.status();
        if !ct_ok {
            return Err(format!("{url}: content-type {content_type} не текстовый"));
        }
        let _ = status;
        let page = self
            .parse_body(resp.into_reader(), &final_url)
            .map_err(|e| format!("{url}: {e}"))?;
        Ok(FetchedPage {
            final_url,
            title: page.title,
            meta_description: page.meta_description,
            lang: String::new(),
            text: page.text,
            links: page.links,
        })
    }

    fn fetch_raw(&mut self, url: &str) -> Result<(u16, String), String> {
        let resp = match self.agent.get(url).call() {
            Ok(r) => r,
            Err(ureq::Error::Status(code, _)) => return Ok((code, String::new())),
            Err(e) => return Err(format!("{url}: {e}")),
        };
        let status = resp.status();
        let mut body = String::new();
        let mut limited = resp.into_reader().take(1024 * 1024); // robots мал
        let _ = limited.read_to_string(&mut body);
        Ok((status, body))
    }
}

// ───────────────────────── тесты ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<!doctype html>
<html><head><title>Kernel Scheduler Guide</title>
<meta name="description" content="How the CFS scheduler works">
<style>.body { color: red; }</style>
<script>window.tracker = function() { sendAnalytics("secret-token"); };</script>
</head><body>
<nav><a href="/">Home</a> <a href="/docs">Docs</a> <a href="/api">API</a></nav>
<div class="cookie-banner">We use cookies to improve your experience. Accept all Reject</div>
<main>
<h1>Completely Fair Scheduler</h1>
<p>The completely fair scheduler is the scheduling class of the Linux kernel
which implements fair queuing. It allows the kernel to assign processor time
to tasks in a predictable and equitable manner across the run queues of
every CPU in the system.</p>
<p>See the <a href="https://www.kernel.org/doc/html/latest/scheduler/">scheduler documentation</a>
and the <a href="cfs-design.html">design document</a> for details about latency and fairness.</p>
<pre><code>struct sched_entity {
    u64 vruntime;
    u64 exec_start;
};
</code></pre>
<table><tr><td>param</td><td>meaning</td></tr><tr><td>vruntime</td><td>virtual runtime of the task</td></tr></table>
</main>
<footer>Copyright 2026. All rights reserved. Subscribe to our newsletter!</footer>
</body></html>"#;

    fn page_text(html: &str, base: &str) -> StreamPage {
        let f = StreamingFetcher::new(5000, DEFAULT_MIN_EPSILON);
        let mut b = StreamPageBuilder::new(f.min_epsilon, 64 * 1024 * 1024);
        // подаём нечётными порциями: UTF-8 и границы состояний
        let bytes = html.as_bytes();
        let mut off = 0;
        for step in [7usize, 13, 3, 101, 29, 512] {
            let end = (off + step).min(bytes.len());
            b.push_chunk(&bytes[off..end]);
            off = end;
            if off >= bytes.len() {
                break;
            }
        }
        while off < bytes.len() {
            let end = (off + 64).min(bytes.len());
            b.push_chunk(&bytes[off..end]);
            off = end;
        }
        b.finish(base)
    }

    #[test]
    fn extracts_title_meta_links_and_semantic_text() {
        let p = page_text(PAGE, "https://kernel.example/guide.html");
        assert_eq!(p.title, "Kernel Scheduler Guide");
        assert_eq!(p.meta_description, "How the CFS scheduler works");
        let t = &p.text;
        assert!(t.contains("fair queuing"), "семантический абзац на месте");
        assert!(t.contains("vruntime"), "таблица/код на месте");
        assert!(t.contains("struct sched_entity"), "код сохранён");
        // ссылки: абсолютная + относительная
        assert!(p.links.contains(&"https://www.kernel.org/doc/html/latest/scheduler/".to_string()));
        assert!(p.links.contains(&"https://kernel.example/cfs-design.html".to_string()));
    }

    #[test]
    fn junk_is_filtered() {
        let p = page_text(PAGE, "https://kernel.example/guide.html");
        let t = &p.text;
        assert!(!t.contains("sendAnalytics"), "script выброшен");
        assert!(!t.contains("color: red"), "style выброшен");
        assert!(!t.to_lowercase().contains("cookie"), "баннер выброшен");
        assert!(!t.to_lowercase().contains("newsletter"), "футер-шум выброшен");
        assert!(p.stats.blocks_seen >= 4, "блоков: {:?}", p.stats.blocks_seen);
        assert!(p.stats.blocks_kept >= 2, "семантика сохранена");
    }

    #[test]
    fn entities_decode() {
        assert_eq!(decode_entities("a &amp; b &lt;x&gt; &#1040;&#x0431;"), "a & b <x> Аб");
        assert_eq!(decode_entities("нет сущностей"), "нет сущностей");
        assert_eq!(decode_entities("битая &amp бар"), "битая &amp бар");
    }

    #[test]
    fn epsilon_scores_separate_meaning_from_noise() {
        let f = EpsilonStreamFilter::new(DEFAULT_MIN_EPSILON);
        let semantic = "The completely fair scheduler implements fair queuing across run queues with predictable latency and equitable processor time distribution for every task in the system.";
        let nav = "Home Docs API Blog";
        let cookie = "We use cookies to improve your experience. Accept all Reject";
        let code = "fn main() { let x = vec![1, 2, 3]; for i in x { println!(\"{}\", i); } }";
        let heading = "Scheduler Internals";
        assert!(f.score(semantic, BlockHints::default()) >= DEFAULT_MIN_EPSILON);
        assert!(f.score(nav, BlockHints::default()) < DEFAULT_MIN_EPSILON);
        assert!(f.score(cookie, BlockHints::default()) < DEFAULT_MIN_EPSILON);
        assert!(f.score(code, BlockHints { code: true, ..Default::default() }) >= DEFAULT_MIN_EPSILON);
        assert!(f.score(heading, BlockHints { heading: true, ..Default::default() }) >= DEFAULT_MIN_EPSILON);
    }

    #[test]
    fn utf8_split_across_chunks() {
        let html = "<h1>Квантовий кристал</h1><p>мозг мухи держит ритм мысли живого вихря смысла через решётку тритов квантовой памяти</p>";
        let mut b = StreamPageBuilder::new(DEFAULT_MIN_EPSILON, 1 << 20);
        for byte in html.as_bytes() {
            b.push_chunk(&[*byte]);
        }
        let p = b.finish("https://x.example/");
        assert!(p.text.contains("кристал"), "кириллица пережила побайтовую подачу");
        assert!(p.text.contains("вихря"));
    }
}


