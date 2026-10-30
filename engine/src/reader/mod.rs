//! # «Рабочий стол» для LLM-читателя (LLM Reader Desktop / State Workspace)
//!
//! Даёт LLM-агенту сессионное состояние чтения документов вместо одноразового
//! неструктурированного контекста:
//! - Позиционный курсор и постраничная навигация
//! - Закладки (Bookmarks), оглавление (TOC) и точный прыжок (Goto)
//! - Аннотации (Annotations: Highlight, Note, Question, Summary, Link, Hypothesis)
//! - ReadSet (учёт прочитанных фрагментов, интервальное склеивание, расчёт Unread-гэпов)
//! - История переходов (Back / Forward)
//! - Черновик (Scratchpad) для записи промежуточных гипотез
//! - Сериализация / персистентность сессии (JSON)

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub type DocId = String;
pub type AnnotationId = String;

static DOC_SEQ: AtomicU64 = AtomicU64::new(1);
static ANN_SEQ: AtomicU64 = AtomicU64::new(1);

pub fn next_doc_id() -> DocId {
    format!("doc_{:04x}_{}", now() & 0xFFFF, DOC_SEQ.fetch_add(1, Ordering::Relaxed))
}

pub fn next_ann_id() -> AnnotationId {
    format!("ann_{:04x}_{}", now() & 0xFFFF, ANN_SEQ.fetch_add(1, Ordering::Relaxed))
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ───────────────────────────────────────────────────────────────────────────
// Базовые типы: Span, Section, Document
// ───────────────────────────────────────────────────────────────────────────

/// Байтовый отрезок текста `[start, end)`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            end: end.max(start),
        }
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// Заголовок/раздел документа (Markdown / структура).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Section {
    pub title: String,
    pub level: u8,
    pub span: Span,
}

/// Документ в сессии читателя.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: DocId,
    pub title: String,
    pub source: String,
    pub text: String,
    pub sections: Vec<Section>,
    pub pages: Vec<Span>,
}

impl Document {
    pub fn from_text(title: impl Into<String>, source: impl Into<String>, text: String) -> Self {
        let sections = extract_sections(&text);
        let pages = vec![Span::new(0, text.len())];
        Self {
            id: next_doc_id(),
            title: title.into(),
            source: source.into(),
            text,
            sections,
            pages,
        }
    }

    /// Срез текста по Span с безопасным выравниванием по границам.
    pub fn slice(&self, span: Span) -> &str {
        let s = span.start.min(self.text.len());
        let e = span.end.min(self.text.len());
        // Выравнивание по границам UTF-8
        let s_aligned = self.floor_char_boundary(s);
        let e_aligned = self.ceil_char_boundary(e);
        &self.text[s_aligned..e_aligned]
    }

    fn floor_char_boundary(&self, mut i: usize) -> usize {
        if i >= self.text.len() {
            return self.text.len();
        }
        while i > 0 && !self.text.is_char_boundary(i) {
            i -= 1;
        }
        i
    }

    fn ceil_char_boundary(&self, mut i: usize) -> usize {
        if i >= self.text.len() {
            return self.text.len();
        }
        while i < self.text.len() && !self.text.is_char_boundary(i) {
            i += 1;
        }
        i
    }

    /// Быстрый поиск вхождений подстроки.
    pub fn find_all(&self, needle: &str, limit: usize) -> Vec<Span> {
        let mut out = Vec::new();
        if needle.is_empty() {
            return out;
        }
        let mut from = 0;
        while out.len() < limit && from < self.text.len() {
            match self.text[from..].find(needle) {
                Some(rel) => {
                    let s = from + rel;
                    let e = s + needle.len();
                    out.push(Span::new(s, e));
                    from = e.max(s + 1);
                }
                None => break,
            }
        }
        out
    }
}

/// Построение оглавления по markdown-заголовкам (`#`, `##`, `###`).
fn extract_sections(text: &str) -> Vec<Section> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    let mut current: Option<(String, u8, usize)> = None;

    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            if let Some((title, level, start)) = current.take() {
                out.push(Section {
                    title,
                    level,
                    span: Span::new(start, offset),
                });
            }
            let hashes = trimmed.chars().take_while(|c| *c == '#').count();
            let title = trimmed.trim_start_matches('#').trim().to_string();
            current = Some((title, hashes.min(6) as u8, offset));
        }
        offset += line.len();
    }
    if let Some((title, level, start)) = current {
        out.push(Section {
            title,
            level,
            span: Span::new(start, offset),
        });
    }
    out
}

// ───────────────────────────────────────────────────────────────────────────
// Аннотации и закладки
// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationKind {
    Highlight,
    Note,
    Question,
    Summary,
    Link,
    Hypothesis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: AnnotationId,
    pub doc: DocId,
    pub span: Span,
    pub kind: AnnotationKind,
    pub text: String,
    pub created_at: i64,
}

// ───────────────────────────────────────────────────────────────────────────
// Действия LLM (Tool Actions)
// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// Открыть документ из текста.
    Open {
        title: String,
        text: String,
        source: Option<String>,
    },
    /// Показать оглавление текущего документа.
    Toc,
    /// Прочитать окно вокруг курсора (или указанный span).
    Read {
        start: Option<usize>,
        len: Option<usize>,
    },
    /// Перейти в точку (процент, оффсет, раздел, закладка, span).
    Goto { target: Target },
    /// Следующее/предыдущее окно (листание).
    Page {
        direction: Dir,
        size: Option<usize>,
    },
    /// Поиск по текущему документу.
    Search {
        query: String,
        limit: Option<usize>,
    },
    /// Добавить аннотацию на выделенный span.
    Annotate {
        kind: AnnotationKind,
        start: usize,
        end: usize,
        text: String,
    },
    /// Поставить закладку на текущий курсор.
    Bookmark { name: String },
    /// Прыгнуть к закладке.
    JumpTo { name: String },
    /// Список аннотаций.
    ListNotes { doc: Option<DocId> },
    /// Дописать в черновик рассуждений.
    Scratch { text: String },
    /// Прочитать черновик.
    ReadScratch,
    /// Очистить черновик.
    ClearScratch,
    /// История назад.
    Back,
    /// История вперёд.
    Forward,
    /// Текущее состояние рабочего стола.
    Status,
    /// Список ещё не прочитанных фрагментов документа (гэпы в ReadSet).
    Unread,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Percent(f32),
    Offset(usize),
    Section(String),
    Bookmark(String),
    Span { start: usize, end: usize },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dir {
    Next,
    Prev,
}

// ───────────────────────────────────────────────────────────────────────────
// Наблюдения (Observations) для контекста LLM
// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Observation {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<DocId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    pub message: String,
}

impl Observation {
    pub fn ok(msg: impl Into<String>) -> Self {
        Self {
            ok: true,
            error: None,
            doc: None,
            cursor: None,
            text: None,
            data: None,
            message: msg.into(),
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            doc: None,
            cursor: None,
            text: None,
            data: None,
            message: String::new(),
        }
    }

    pub fn with_text(mut self, t: impl Into<String>) -> Self {
        self.text = Some(t.into());
        self
    }

    pub fn with_cursor(mut self, c: usize) -> Self {
        self.cursor = Some(c);
        self
    }

    pub fn with_data(mut self, d: serde_json::Value) -> Self {
        self.data = Some(d);
        self
    }

    pub fn with_doc(mut self, d: impl Into<String>) -> Self {
        self.doc = Some(d.into());
        self
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Рабочий стол (Workspace)
// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub docs: HashMap<DocId, Document>,
    pub order: Vec<DocId>,
    pub current: Option<DocId>,
    pub cursor: usize,
    pub history: Vec<(DocId, usize)>,
    pub history_pos: usize,
    pub annotations: Vec<Annotation>,
    pub bookmarks: HashMap<String, (DocId, usize)>,
    pub scratchpad: String,
    pub read_set: HashMap<DocId, Vec<Span>>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            docs: HashMap::new(),
            order: Vec::new(),
            current: None,
            cursor: 0,
            history: Vec::new(),
            history_pos: 0,
            annotations: Vec::new(),
            bookmarks: HashMap::new(),
            scratchpad: String::new(),
            read_set: HashMap::new(),
        }
    }

    pub fn current_doc(&self) -> Result<&Document, String> {
        let id = self.current.as_ref().ok_or_else(|| "Нет открытого документа".to_string())?;
        self.docs.get(id).ok_or_else(|| "Документ не найден".to_string())
    }

    fn push_history(&mut self) {
        if let Some(doc) = &self.current {
            self.history.truncate(self.history_pos + 1);
            self.history.push((doc.clone(), self.cursor));
            self.history_pos = self.history.len().saturating_sub(1);
        }
    }

    fn mark_read(&mut self, span: Span) {
        if let Some(doc) = &self.current {
            let entry = self.read_set.entry(doc.clone()).or_default();
            entry.push(span);
            entry.sort_by_key(|s| s.start);
            // Интервальное объединение
            let mut merged: Vec<Span> = Vec::with_capacity(entry.len());
            for s in entry.drain(..) {
                if let Some(last) = merged.last_mut() {
                    if s.start <= last.end {
                        last.end = last.end.max(s.end);
                        continue;
                    }
                }
                merged.push(s);
            }
            *entry = merged;
        }
    }

    /// Главная точка входа: исполняет действие и возвращает наблюдение.
    pub fn execute(&mut self, action: Action) -> Observation {
        match self.try_execute(action) {
            Ok(obs) => obs,
            Err(e) => Observation::err(e),
        }
    }

    fn try_execute(&mut self, action: Action) -> Result<Observation, String> {
        match action {
            Action::Open {
                title,
                text,
                source,
            } => {
                let src = source.unwrap_or_else(|| "<inline>".into());
                let doc = Document::from_text(title.clone(), src, text);
                let id = doc.id.clone();
                self.docs.insert(id.clone(), doc);
                self.order.push(id.clone());
                self.current = Some(id.clone());
                self.cursor = 0;
                self.push_history();
                Ok(Observation::ok(format!("Открыт документ «{}»", title)).with_doc(id))
            }

            Action::Toc => {
                let doc = self.current_doc()?;
                let id = doc.id.clone();
                let items: Vec<_> = doc
                    .sections
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "title": s.title,
                            "level": s.level,
                            "start": s.span.start,
                            "end": s.span.end,
                        })
                    })
                    .collect();
                Ok(Observation::ok("Оглавление")
                    .with_doc(id)
                    .with_data(serde_json::json!(items)))
            }

            Action::Read { start, len } => {
                let doc = self.current_doc()?;
                let s = start.unwrap_or(self.cursor);
                let l = len.unwrap_or(1500);
                let span = Span::new(s, (s + l).min(doc.text.len()));
                let text = doc.slice(span).to_string();
                let id = doc.id.clone();
                self.mark_read(span);
                self.cursor = span.end;
                self.push_history();
                Ok(Observation::ok(format!("Прочитано {} байт", span.len()))
                    .with_doc(id)
                    .with_cursor(span.end)
                    .with_text(text))
            }

            Action::Page { direction, size } => {
                let doc = self.current_doc()?;
                let l = size.unwrap_or(1500);
                let total = doc.text.len();
                let (s, e) = match direction {
                    Dir::Next => (self.cursor, (self.cursor + l).min(total)),
                    Dir::Prev => (self.cursor.saturating_sub(l), self.cursor),
                };
                let span = Span::new(s, e);
                let text = doc.slice(span).to_string();
                let id = doc.id.clone();
                self.mark_read(span);
                self.cursor = e;
                self.push_history();
                Ok(Observation::ok(format!("Стр. {:?}", direction))
                    .with_doc(id)
                    .with_cursor(e)
                    .with_text(text))
            }

            Action::Goto { target } => {
                let doc = self.current_doc()?;
                let pos = match target {
                    Target::Percent(p) => {
                        ((doc.text.len() as f32) * p.clamp(0.0, 1.0)) as usize
                    }
                    Target::Offset(o) => o.min(doc.text.len()),
                    Target::Section(name) => doc
                        .sections
                        .iter()
                        .find(|s| s.title.eq_ignore_ascii_case(&name))
                        .map(|s| s.span.start)
                        .ok_or_else(|| format!("Раздел не найден: {name}"))?,
                    Target::Bookmark(name) => {
                        let (_, pos) = self
                            .bookmarks
                            .get(&name)
                            .ok_or_else(|| format!("Закладка не найдена: {name}"))?;
                        *pos
                    }
                    Target::Span { start, .. } => start.min(doc.text.len()),
                };
                self.cursor = pos;
                self.push_history();
                Ok(Observation::ok("Курсор перемещён").with_cursor(pos))
            }

            Action::Search { query, limit } => {
                let doc = self.current_doc()?;
                let hits = doc.find_all(&query, limit.unwrap_or(20));
                let items: Vec<_> = hits
                    .iter()
                    .map(|h| {
                        let preview_span = Span::new(
                            h.start.saturating_sub(40),
                            (h.end + 40).min(doc.text.len()),
                        );
                        serde_json::json!({
                            "start": h.start,
                            "end": h.end,
                            "preview": doc.slice(preview_span),
                        })
                    })
                    .collect();
                Ok(Observation::ok(format!("Найдено {} совпадений", hits.len()))
                    .with_data(serde_json::json!(items)))
            }

            Action::Annotate {
                kind,
                start,
                end,
                text,
            } => {
                let doc = self.current_doc()?;
                let ann = Annotation {
                    id: next_ann_id(),
                    doc: doc.id.clone(),
                    span: Span::new(start, end),
                    kind,
                    text,
                    created_at: now(),
                };
                let id = ann.id.clone();
                self.annotations.push(ann);
                Ok(Observation::ok(format!("Аннотация добавлена: {id}"))
                    .with_data(serde_json::json!({ "id": id })))
            }

            Action::Bookmark { name } => {
                let doc = self.current_doc()?;
                self.bookmarks.insert(name.clone(), (doc.id.clone(), self.cursor));
                Ok(Observation::ok(format!(
                    "Закладка «{name}» на позиции {}",
                    self.cursor
                )))
            }

            Action::JumpTo { name } => {
                let (doc, pos) = self
                    .bookmarks
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| format!("Закладка не найдена: {name}"))?;
                self.current = Some(doc);
                self.cursor = pos;
                self.push_history();
                Ok(Observation::ok(format!("Прыжок к «{name}»")).with_cursor(pos))
            }

            Action::ListNotes { doc } => {
                let items: Vec<_> = self
                    .annotations
                    .iter()
                    .filter(|a| doc.as_ref().map_or(true, |d| &a.doc == d))
                    .map(|a| {
                        serde_json::json!({
                            "id": a.id,
                            "doc": a.doc,
                            "span": a.span,
                            "kind": a.kind,
                            "text": a.text,
                        })
                    })
                    .collect();
                Ok(Observation::ok(format!("{} аннотаций", items.len()))
                    .with_data(serde_json::json!(items)))
            }

            Action::Scratch { text } => {
                if !self.scratchpad.is_empty() {
                    self.scratchpad.push('\n');
                }
                self.scratchpad.push_str(&text);
                Ok(Observation::ok("Записано в черновик"))
            }

            Action::ReadScratch => {
                Ok(Observation::ok("Черновик").with_text(self.scratchpad.clone()))
            }

            Action::ClearScratch => {
                self.scratchpad.clear();
                Ok(Observation::ok("Черновик очищен"))
            }

            Action::Back => {
                if self.history_pos == 0 {
                    return Err("Некуда назад (начало истории)".into());
                }
                self.history_pos -= 1;
                let (doc, pos) = &self.history[self.history_pos];
                self.current = Some(doc.clone());
                self.cursor = *pos;
                Ok(Observation::ok("Назад").with_cursor(*pos))
            }

            Action::Forward => {
                if self.history_pos + 1 >= self.history.len() {
                    return Err("Некуда вперёд (конец истории)".into());
                }
                self.history_pos += 1;
                let (doc, pos) = &self.history[self.history_pos];
                self.current = Some(doc.clone());
                self.cursor = *pos;
                Ok(Observation::ok("Вперёд").with_cursor(*pos))
            }

            Action::Status => {
                let doc_info = self.current.as_ref().and_then(|id| {
                    self.docs.get(id).map(|d| {
                        serde_json::json!({
                            "id": d.id,
                            "title": d.title,
                            "len": d.text.len(),
                            "cursor": self.cursor,
                            "progress": self.cursor as f32 / d.text.len().max(1) as f32,
                        })
                    })
                });
                Ok(Observation::ok("Статус").with_data(serde_json::json!({
                    "current": doc_info,
                    "docs": self.order.len(),
                    "annotations": self.annotations.len(),
                    "bookmarks": self.bookmarks.keys().collect::<Vec<_>>(),
                    "history_len": self.history.len(),
                })))
            }

            Action::Unread => {
                let doc = self.current_doc()?;
                let read = self.read_set.get(&doc.id).cloned().unwrap_or_default();
                let mut gaps = Vec::new();
                let mut pos = 0usize;
                for r in &read {
                    if r.start > pos {
                        gaps.push(Span::new(pos, r.start));
                    }
                    pos = pos.max(r.end);
                }
                if pos < doc.text.len() {
                    gaps.push(Span::new(pos, doc.text.len()));
                }
                Ok(Observation::ok(format!("Непрочитанных фрагментов: {}", gaps.len()))
                    .with_data(serde_json::json!(gaps)))
            }
        }
    }

    // ─── Персистентность сессии ───

    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }

    pub fn load_from_file(path: &str) -> Result<Self, String> {
        let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| e.to_string())
    }
}

// ───────────────────────────────────────────────────────────────────────────
// JSON Schema для LLM Tool-Calling (MCP / OpenAI / Anthropic / Local GLM)
// ───────────────────────────────────────────────────────────────────────────

/// Возвращает JSON-схему инструмента «reader» для системного промпта LLM.
pub fn tool_schema() -> serde_json::Value {
    serde_json::json!({
        "name": "reader",
        "description": "Управляет состоянием чтения документов: навигация, оглавление, поиск, аннотации, закладки, черновик и ReadSet.",
        "parameters": {
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "open", "toc", "read", "goto", "page", "search",
                        "annotate", "bookmark", "jump_to", "list_notes",
                        "scratch", "read_scratch", "clear_scratch",
                        "back", "forward", "status", "unread"
                    ]
                },
                "title": {"type": "string", "description": "Название документа (для open)"},
                "text": {"type": "string", "description": "Текст документа или заметки"},
                "source": {"type": "string", "description": "Источник документа (URL или путь к файлу)"},
                "start": {"type": "integer", "description": "Начальный байтовый оффсет"},
                "len": {"type": "integer", "description": "Длина фрагмента для чтения"},
                "end": {"type": "integer", "description": "Конечный байтовый оффсет для аннотации"},
                "query": {"type": "string", "description": "Поисковый запрос"},
                "kind": {
                    "type": "string",
                    "enum": ["highlight", "note", "question", "summary", "link", "hypothesis"],
                    "description": "Тип аннотации"
                },
                "target": {
                    "type": "object",
                    "description": "Цель для goto (percent, offset, section, bookmark, span)"
                },
                "direction": {
                    "type": "string",
                    "enum": ["next", "prev"],
                    "description": "Направление листания"
                },
                "name": {"type": "string", "description": "Имя закладки"}
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_reader_workspace_lifecycle() {
        let mut ws = Workspace::new();

        let text = "\
# Введение
POLER-Engine — суверенный AI-native движок.

# Квантовая семантика
Мастер-уравнение описывает динамику вероятностей.

# Субнейронная решетка
Синусоиды и гармоники заменяют слепой матричный оверхед.
";

        // 1. Open
        let obs = ws.execute(Action::Open {
            title: "POLER Architecture".into(),
            text: text.into(),
            source: Some("poler.md".into()),
        });
        assert!(obs.ok);
        assert!(ws.current.is_some());

        // 2. TOC
        let obs = ws.execute(Action::Toc);
        assert!(obs.ok);
        let data = obs.data.expect("data exists");
        assert_eq!(data.as_array().unwrap().len(), 3);

        // 3. Search
        let obs = ws.execute(Action::Search {
            query: "уравнение".into(),
            limit: Some(5),
        });
        assert!(obs.ok);
        let hits = obs.data.unwrap();
        assert_eq!(hits.as_array().unwrap().len(), 1);

        // 4. Goto section & Read
        let obs = ws.execute(Action::Goto {
            target: Target::Section("Субнейронная решетка".into()),
        });
        assert!(obs.ok);

        let obs = ws.execute(Action::Read {
            start: None,
            len: Some(100),
        });
        assert!(obs.ok);
        assert!(obs.text.unwrap().contains("Синусоиды"));

        // 5. Annotate & Bookmark
        let obs = ws.execute(Action::Bookmark {
            name: "lattice_start".into(),
        });
        assert!(obs.ok);

        let obs = ws.execute(Action::Annotate {
            kind: AnnotationKind::Hypothesis,
            start: 120,
            end: 180,
            text: "Связать синусоиды с частотами в архиве 299 источников".into(),
        });
        assert!(obs.ok);

        // 6. Scratchpad
        let obs = ws.execute(Action::Scratch {
            text: "Факт: субнейронный резонанс сходится за O(log N)".into(),
        });
        assert!(obs.ok);

        let obs = ws.execute(Action::ReadScratch);
        assert!(obs.text.unwrap().contains("O(log N)"));

        // 7. Unread check
        let obs = ws.execute(Action::Unread);
        assert!(obs.ok);

        // 8. Back & Forward navigation
        let cur_pos = ws.cursor;
        let obs = ws.execute(Action::Back);
        assert!(obs.ok);
        assert_ne!(ws.cursor, cur_pos);

        let obs = ws.execute(Action::Forward);
        assert!(obs.ok);
        assert_eq!(ws.cursor, cur_pos);
    }
}
