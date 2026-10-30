//! Zero-Copy DOM-парсер и дерево элементов для POLER Browser.
//!
//! В отличие от классического Chromium (где каждый DOM-узел весит сотни байт
//! в heap), здесь узлы ссылаются на срезы исходного буфера и хранятся
//! в компактном плоском векторе с индексами u32.

use std::collections::HashMap;

/// Тип узла в дереве документа.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeType {
    Document,
    Element,
    Text,
    Comment,
}

/// Узел DOM-дерева в компактном представлении.
#[derive(Debug, Clone)]
pub struct DomNode {
    pub id: u32,
    pub parent: Option<u32>,
    pub children: Vec<u32>,
    pub node_type: NodeType,
    pub tag: String,
    pub attributes: HashMap<String, String>,
    pub text: String,
}

impl DomNode {
    pub fn new(id: u32, node_type: NodeType, tag: &str) -> Self {
        Self {
            id,
            parent: None,
            children: Vec::new(),
            node_type,
            tag: tag.to_ascii_lowercase(),
            attributes: HashMap::new(),
            text: String::new(),
        }
    }
}

/// Компактное представление веб-страницы.
#[derive(Debug, Clone)]
pub struct DomDocument {
    pub nodes: Vec<DomNode>,
    pub title: String,
    pub root_id: u32,
}

impl DomDocument {
    pub fn new() -> Self {
        let root = DomNode::new(0, NodeType::Document, "#document");
        Self {
            nodes: vec![root],
            title: String::new(),
            root_id: 0,
        }
    }

    /// Быстрый потоковый парсинг HTML.
    pub fn parse_html(html: &str) -> Self {
        let mut doc = Self::new();
        let mut current_parent = 0u32;
        let mut in_tag = false;
        let mut in_script_or_style = false;
        let mut script_tag = String::new();
        let mut tag_buf = String::new();
        let mut text_buf = String::new();

        let chars: Vec<char> = html.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];

            if in_script_or_style {
                if c == '<' && i + script_tag.len() + 2 < chars.len() {
                    let next_slice: String = chars[i..i + script_tag.len() + 3].iter().collect();
                    if next_slice.eq_ignore_ascii_case(&format!("</{}>", script_tag)) {
                        in_script_or_style = false;
                        i += script_tag.len() + 3;
                        continue;
                    }
                }
                i += 1;
                continue;
            }

            if c == '<' {
                if !text_buf.trim().is_empty() {
                    let id = doc.nodes.len() as u32;
                    let mut text_node = DomNode::new(id, NodeType::Text, "#text");
                    text_node.text = text_buf.trim().to_string();
                    text_node.parent = Some(current_parent);
                    doc.nodes[current_parent as usize].children.push(id);
                    doc.nodes.push(text_node);
                    text_buf.clear();
                } else {
                    text_buf.clear();
                }

                in_tag = true;
                tag_buf.clear();
            } else if c == '>' && in_tag {
                in_tag = false;
                let trimmed = tag_buf.trim();

                if trimmed.starts_with('/') {
                    // Закрывающий тег
                    if let Some(parent) = doc.nodes[current_parent as usize].parent {
                        current_parent = parent;
                    }
                } else if !trimmed.is_empty() && !trimmed.starts_with('!') {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    let tag_name = parts[0].to_ascii_lowercase();

                    if tag_name == "script" || tag_name == "style" {
                        in_script_or_style = true;
                        script_tag = tag_name;
                        i += 1;
                        continue;
                    }

                    let id = doc.nodes.len() as u32;
                    let mut elem_node = DomNode::new(id, NodeType::Element, &tag_name);
                    elem_node.parent = Some(current_parent);

                    // Парсинг базовых атрибутов (class, id, href, src)
                    for part in &parts[1..] {
                        if let Some((k, v)) = part.split_once('=') {
                            let clean_val = v.trim_matches(|c| c == '"' || c == '\'');
                            elem_node.attributes.insert(k.to_ascii_lowercase(), clean_val.to_string());
                        }
                    }

                    doc.nodes[current_parent as usize].children.push(id);
                    doc.nodes.push(elem_node);

                    // Если тег не самозакрывающийся
                    let is_self_closing = trimmed.ends_with('/') || matches!(tag_name.as_str(), "img" | "br" | "hr" | "input" | "meta" | "link");
                    if !is_self_closing {
                        current_parent = id;
                    }
                }
            } else if in_tag {
                tag_buf.push(c);
            } else {
                text_buf.push(c);
            }

            i += 1;
        }

        // Извлечение заголовка страницы (title)
        for node in &doc.nodes {
            if node.tag == "title" {
                if let Some(&child_id) = node.children.first() {
                    doc.title = doc.nodes[child_id as usize].text.clone();
                }
            }
        }

        doc
    }

    /// Извлечь весь чистый читаемый текст со страницы.
    pub fn extract_clean_text(&self) -> String {
        let mut out = String::new();
        self.collect_text(self.root_id, &mut out);
        out
    }

    fn collect_text(&self, node_id: u32, out: &mut String) {
        let node = &self.nodes[node_id as usize];

        // Игнорируем технические теги
        if matches!(node.tag.as_str(), "script" | "style" | "noscript" | "svg") {
            return;
        }

        if node.node_type == NodeType::Text && !node.text.is_empty() {
            out.push_str(&node.text);
            out.push(' ');
        }

        for &child_id in &node.children {
            self.collect_text(child_id, out);
        }

        if matches!(node.tag.as_str(), "p" | "div" | "h1" | "h2" | "h3" | "h4" | "li" | "article" | "section") {
            out.push('\n');
        }
    }
}

impl Default for DomDocument {
    fn default() -> Self {
        Self::new()
    }
}
