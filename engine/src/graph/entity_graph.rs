//! DiGraph на базе petgraph: сущности, предикатные рёбра, K-hop поиск.
//!
//! Формула обхода (спецификация §3.В):
//!
//! ```text
//! SubGraph(E0, k) = { (u, predicate, v) | dist(E0, u) < k ∧ (u →predicate v) ∈ G }
//! ```
//!
//! Особенности реализации:
//!
//! * обход BFS **в обе стороны** (outgoing + incoming) — связи «кто ссылается
//!   на сущность» не менее важны, чем «на кого ссылается сущность»;
//! * **v0.21: две политики идентичности** ([`IdentityPolicy`]):
//!   - `Text` — узлы сливаются case-insensitive, отображаемое имя
//!     повышается до варианта с заглавной буквы (заметки, нарративные
//!     сущности: «Нокс» == «НОКС»);
//!   - `CodeSymbol` — строгий case-sensitive ключ с квалификацией module
//!     path (`Foo` ≠ `foo` ≠ `FOO`; `parser::parse` ≠ `cli::parse`).
//!     В коде регистр — часть идентичности языка (Rust/Python/JS), а
//!     module path — часть пространства имён;
//! * каждый узел несёт temporal-слой (`Т-23`, …); при заданном фильтре
//!   рёбра между узлами чужих слоёв отбрасываются (Temporal Metric Tagging);
//! * тройки дедуплицируются, вывод ограничен `max_relations`.

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use crate::aidde::symbols::SymbolTable;

/// Политика идентичности узлов графа (v0.21.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum IdentityPolicy {
    /// Текстовые сущности: ключ узла — `lowercase(name)`, повторное
    /// добавление с заглавной повышает отображаемое имя. Для заметок,
    /// нарратива, общих знаний.
    #[default]
    Text,
    /// Символы кода: ключ узла — точное имя с учётом регистра,
    /// квалифицированное module path (`module::name`). Идентичность
    /// соответствует правилам языка: `Foo`, `foo` и `FOO` — три разных
    /// символа. Отображаемое имя НЕ повышается — первое вхождение
    /// (определение из AST-скана) авторитетно.
    CodeSymbol,
}


/// Узел знаний: сущность с temporal-слоем.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeNode {
    pub name: String,
    pub node_type: String,
    pub temporal_layer: Option<String>,
    /// Module path / namespace для `CodeSymbol`-узлов (`None` для текста).
    #[serde(default)]
    pub module_path: Option<String>,
}

/// Ребро знаний: предикат + вес.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationEdge {
    pub predicate: String,
    pub weight: f64,
}

/// Ссылка на символ кода для [`EntityGraph::add_code_triple`]:
/// точное имя (регистр важен) + опциональный module path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSymbolRef {
    /// Точное имя символа (`Foo` ≠ `foo`).
    pub name: String,
    /// Module path / namespace (`parser`, `crate::engine`, `utils.fs` …).
    pub module: Option<String>,
}

impl CodeSymbolRef {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), module: None }
    }

    pub fn qualified(module: &str, name: &str) -> Self {
        Self { name: name.to_string(), module: Some(module.to_string()) }
    }

    /// Ключ узла: `module::name` либо голое `name`.
    fn key(&self) -> String {
        match &self.module {
            Some(m) if !m.is_empty() => format!("{m}::{}", self.name),
            _ => self.name.clone(),
        }
    }
}

/// Граф сущностей со словарём узлов (ключ — по политике идентичности).
pub struct EntityGraph {
    graph: DiGraph<KnowledgeNode, RelationEdge>,
    lookup: HashMap<String, NodeIndex>,
    policy: IdentityPolicy,
}

impl EntityGraph {
    /// Текстовый граф (политика по умолчанию — как во всех версиях до
    /// v0.21: case-insensitive слиятие узлов).
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            lookup: HashMap::new(),
            policy: IdentityPolicy::Text,
        }
    }

    /// Граф с явной политикой идентичности.
    pub fn with_policy(policy: IdentityPolicy) -> Self {
        Self {
            graph: DiGraph::new(),
            lookup: HashMap::new(),
            policy,
        }
    }

    /// Действующая политика идентичности.
    pub fn policy(&self) -> IdentityPolicy {
        self.policy
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// v0.86.0: ключи всех узлов графа (по политике идентичности).
    ///
    /// Для `CodeSymbol` ключи квалифицированы (`module::name`) — один и
    /// тот же bare-символ живёт в нескольких модулях/языках
    /// (`vdbeapi::sqlite3_step` и `CApi::sqlite3_step`): кросс-языковой
    /// нексус собирает соседство каждого из них.
    pub fn node_keys(&self) -> Vec<&str> {
        self.lookup.keys().map(|s| s.as_str()).collect()
    }

    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Ключ узла по политике идентичности.
    fn key_of(&self, name: &str) -> String {
        match self.policy {
            IdentityPolicy::Text => name.to_lowercase(),
            IdentityPolicy::CodeSymbol => name.to_string(),
        }
    }

    fn get_or_create(&mut self, name: &str, temporal: Option<&str>) -> NodeIndex {
        let key = self.key_of(name);
        if let Some(&idx) = self.lookup.get(&key) {
            // повышение отображаемого имени — ТОЛЬКО для текстовой политики:
            // в коде «foo» и «Foo» — разные символы, менять регистр нельзя
            if self.policy == IdentityPolicy::Text {
                let cur = self.graph[idx].name.clone();
                let new_better = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                    && cur.chars().next().map(|c| c.is_lowercase()).unwrap_or(false);
                if new_better {
                    self.graph[idx].name = name.to_string();
                }
            }
            if self.graph[idx].temporal_layer.is_none() {
                self.graph[idx].temporal_layer = temporal.map(String::from);
            }
            return idx;
        }
        let idx = self.graph.add_node(KnowledgeNode {
            name: name.to_string(),
            node_type: "Entity".to_string(),
            temporal_layer: temporal.map(String::from),
            module_path: None,
        });
        self.lookup.insert(key, idx);
        idx
    }

    /// Добавляет тройку `(subject, predicate, object)` с temporal-слоем.
    pub fn add_triple(
        &mut self,
        subject: &str,
        predicate: &str,
        object: &str,
        temporal: Option<&str>,
        weight: f64,
    ) {
        if subject.is_empty() || object.is_empty() || predicate.is_empty() {
            return;
        }
        let u = self.get_or_create(subject, temporal);
        let v = self.get_or_create(object, temporal);
        self.graph.add_edge(
            u,
            v,
            RelationEdge { predicate: predicate.to_string(), weight },
        );
    }

    /// Добавляет тройку над СИМВОЛАМИ КОДА: идентичность — точный регистр +
    /// module path. Узлы получают `node_type` из `kind` (fn/struct/class/…).
    pub fn add_code_triple(
        &mut self,
        subject: &CodeSymbolRef,
        subject_kind: &str,
        predicate: &str,
        object: &CodeSymbolRef,
        object_kind: &str,
        temporal: Option<&str>,
        weight: f64,
    ) {
        if predicate.is_empty() || subject.name.is_empty() || object.name.is_empty() {
            return;
        }
        let u = self.get_or_create_code(subject, subject_kind, temporal);
        let v = self.get_or_create_code(object, object_kind, temporal);
        // дедуп рёбер: между двумя символами один предикат нужен один раз
        // (call sites повторяются — рёбра множить незачем)
        let exists = self
            .graph
            .edges_connecting(u, v)
            .any(|e| e.weight().predicate == predicate);
        if !exists {
            self.graph.add_edge(
                u,
                v,
                RelationEdge { predicate: predicate.to_string(), weight },
            );
        }
    }

    fn get_or_create_code(
        &mut self,
        sym: &CodeSymbolRef,
        kind: &str,
        temporal: Option<&str>,
    ) -> NodeIndex {
        let key = sym.key();
        if let Some(&idx) = self.lookup.get(&key) {
            if self.graph[idx].temporal_layer.is_none() {
                self.graph[idx].temporal_layer = temporal.map(String::from);
            }
            return idx;
        }
        let idx = self.graph.add_node(KnowledgeNode {
            name: sym.name.clone(),
            node_type: if kind.is_empty() { "CodeSymbol".to_string() } else { kind.to_string() },
            temporal_layer: temporal.map(String::from),
            module_path: sym.module.clone(),
        });
        self.lookup.insert(key, idx);
        idx
    }

    /// Поиск узла (политика идентичности решает, как нормируется имя).
    pub fn find_node(&self, name: &str) -> Option<NodeIndex> {
        self.lookup.get(&self.key_of(name)).copied()
    }

    /// Поиск СИМВОЛА КОДА: точный регистр, module path опционален.
    ///
    /// * `module = Some("parser")` → ключ `parser::parse`;
    /// * `module = None` → голое имя `parse`; если голого узла нет, ищем
    ///   единственный ключ с суффиксом `::parse` (детерминированно —
    ///   по отсортированному списку ключей; неоднозначность → `None`).
    pub fn find_code_symbol(&self, module: Option<&str>, name: &str) -> Option<NodeIndex> {
        let direct = match module {
            Some(m) if !m.is_empty() => format!("{m}::{name}"),
            _ => name.to_string(),
        };
        if let Some(&idx) = self.lookup.get(&direct) {
            return Some(idx);
        }
        if module.is_some() {
            return None; // явная квалификация не нашлась — неоднозначность не разыскиваем
        }
        let suffix = format!("::{name}");
        let mut hits: Vec<&String> = self
            .lookup
            .keys()
            .filter(|k| k.ends_with(&suffix))
            .collect();
        hits.sort();
        match hits.len() {
            1 => self.lookup.get(hits[0]).copied(),
            _ => None, // 0 или неоднозначно (parser::parse и cli::parse)
        }
    }

    /// Отображаемое имя узла (для CodeSymbol — с квалификацией module).
    pub fn display_name(&self, idx: NodeIndex) -> String {
        let n = &self.graph[idx];
        match (&n.module_path, self.policy) {
            (Some(m), IdentityPolicy::CodeSymbol) if !m.is_empty() => format!("{m}::{}", n.name),
            _ => n.name.clone(),
        }
    }

    fn temporal_ok(&self, a: NodeIndex, b: NodeIndex, filter: Option<&str>) -> bool {
        match filter {
            None => true,
            Some(f) => {
                let la = &self.graph[a].temporal_layer;
                let lb = &self.graph[b].temporal_layer;
                let ok_a = la.as_deref().map_or(true, |x| x == f);
                let ok_b = lb.as_deref().map_or(true, |x| x == f);
                ok_a && ok_b
            }
        }
    }

    /// Ключ дедупликации троек: текст — lowercase (как до v0.21),
    /// код — точный регистр (`Foo|calls|bar` и `foo|calls|bar` — разные).
    fn triple_dedup_key(&self, s: &str, p: &str, o: &str) -> (String, String, String) {
        match self.policy {
            IdentityPolicy::Text => (s.to_lowercase(), p.to_string(), o.to_lowercase()),
            IdentityPolicy::CodeSymbol => (s.to_string(), p.to_string(), o.to_string()),
        }
    }

    /// Извлекает связный K-hop подграф вокруг сущности.
    ///
    /// Возвращает тройки `(subject, predicate, object)`, достижимые из `E0`
    /// за `< k` шагов BFS (обход в обе стороны), отфильтрованные по
    /// temporal-слою и ограниченные `max_relations`.
    pub fn extract_k_hop(
        &self,
        entity: &str,
        k: usize,
        temporal_filter: Option<&str>,
        max_relations: usize,
    ) -> Vec<(String, String, String)> {
        let mut out: Vec<(String, String, String)> = Vec::new();
        let start = match self.policy {
            IdentityPolicy::Text => match self.find_node(entity) {
                Some(i) => i,
                None => return out,
            },
            // кодовый старт: точное имя, при неудаче — уникальный суффикс
            IdentityPolicy::CodeSymbol => match self
                .find_code_symbol(None, entity)
                .or_else(|| self.find_node(entity))
            {
                Some(i) => i,
                None => return out,
            },
        };

        let mut visited: HashSet<NodeIndex> = HashSet::new();
        visited.insert(start);
        let mut queue: VecDeque<(NodeIndex, usize)> = VecDeque::new();
        queue.push_back((start, 0));
        let mut seen_triples: HashSet<(String, String, String)> = HashSet::new();

        while let Some((node, depth)) = queue.pop_front() {
            if depth >= k || out.len() >= max_relations {
                continue;
            }
            // исходящие рёбра: (node ->pred target)
            for edge in self.graph.edges_directed(node, Direction::Outgoing) {
                let target = edge.target();
                if self.temporal_ok(node, target, temporal_filter) {
                    let s = self.display_name(node);
                    let o = self.display_name(target);
                    let t = (s, edge.weight().predicate.clone(), o);
                    if seen_triples.insert(self.triple_dedup_key(&t.0, &t.1, &t.2)) {
                        out.push(t);
                        if out.len() >= max_relations {
                            break;
                        }
                    }
                }
                if !visited.contains(&target) {
                    visited.insert(target);
                    queue.push_back((target, depth + 1));
                }
            }
            // входящие рёбра: (source ->pred node)
            for edge in self.graph.edges_directed(node, Direction::Incoming) {
                let source = edge.source();
                if self.temporal_ok(source, node, temporal_filter) {
                    let s = self.display_name(source);
                    let o = self.display_name(node);
                    let t = (s, edge.weight().predicate.clone(), o);
                    if seen_triples.insert(self.triple_dedup_key(&t.0, &t.1, &t.2)) {
                        out.push(t);
                        if out.len() >= max_relations {
                            break;
                        }
                    }
                }
                if !visited.contains(&source) {
                    visited.insert(source);
                    queue.push_back((source, depth + 1));
                }
            }
        }
        out
    }

    /// v0.87.0 VAULT-ASM (LENS): K-hop рёбра с весами барьера.
    ///
    /// Тот же BFS, что [`extract_k_hop`], но возвращает четвёрки
    /// `(subject, predicate, object, lens_weight)`, где
    ///
    /// ```text
    /// lens_weight = edge.weight · 0.5^depth
    /// ```
    ///
    /// — вес рёбра затухает по K-hop дистанции (прямые соседи — полный
    /// вес, дистанция 2 — половина, …). LENS-барьер сокровищницы
    /// (`weight > 0.05`) отсекает связи глубже log2(w/0.05) хопов —
    /// «No-Hits» защита от галлюцинаций дальних обходов
    /// (archive/poler-lens/src/lens_index.rs).
    pub fn extract_k_hop_weighted(
        &self,
        entity: &str,
        k: usize,
        temporal_filter: Option<&str>,
        max_relations: usize,
    ) -> Vec<(String, String, String, f64)> {
        let mut out: Vec<(String, String, String, f64)> = Vec::new();
        let start = match self.policy {
            IdentityPolicy::Text => match self.find_node(entity) {
                Some(i) => i,
                None => return out,
            },
            IdentityPolicy::CodeSymbol => match self
                .find_code_symbol(None, entity)
                .or_else(|| self.find_node(entity))
            {
                Some(i) => i,
                None => return out,
            },
        };

        let mut visited: HashSet<NodeIndex> = HashSet::new();
        visited.insert(start);
        let mut queue: VecDeque<(NodeIndex, usize)> = VecDeque::new();
        queue.push_back((start, 0));
        let mut seen_triples: HashSet<(String, String, String)> = HashSet::new();

        while let Some((node, depth)) = queue.pop_front() {
            if depth >= k || out.len() >= max_relations {
                continue;
            }
            // затухание LENS: 0.5^depth (прямой сосед = 1.0)
            let decay = 0.5f64.powi(depth as i32);
            for dir in [Direction::Outgoing, Direction::Incoming] {
                for edge in self.graph.edges_directed(node, dir) {
                    let (src, dst) = match dir {
                        Direction::Outgoing => (node, edge.target()),
                        Direction::Incoming => (edge.source(), node),
                    };
                    if self.temporal_ok(src, dst, temporal_filter) {
                        let s = self.display_name(src);
                        let o = self.display_name(dst);
                        let t = (s, edge.weight().predicate.clone(), o);
                        if seen_triples.insert(self.triple_dedup_key(&t.0, &t.1, &t.2)) {
                            let lens_w = edge.weight().weight * decay;
                            out.push((t.0, t.1, t.2, lens_w));
                            if out.len() >= max_relations {
                                break;
                            }
                        }
                    }
                    let other = match dir {
                        Direction::Outgoing => edge.target(),
                        Direction::Incoming => edge.source(),
                    };
                    if !visited.contains(&other) {
                        visited.insert(other);
                        queue.push_back((other, depth + 1));
                    }
                }
            }
        }
        out
    }

    /// Граф символов кода из таблицы символов AST-скана (v0.21).
    ///
    /// * узлы — определения (module = stem файла, `node_type` = kind);
    /// * рёбра — вызовы `caller --calls--> callee` (дедуп по паре);
    /// * политика — [`IdentityPolicy::CodeSymbol`]: регистр и module path
    ///   строго сохраняются.
    ///
    /// Узел вызова, не имеющий определения, получает kind `extern/macro`.
    pub fn from_symbol_table(table: &SymbolTable) -> Self {
        let mut g = Self::with_policy(IdentityPolicy::CodeSymbol);
        let module_of = |file: &str| -> String {
            Path::new(file)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        // определения
        for def in &table.defs {
            let m = module_of(&def.file);
            let sym = if m.is_empty() {
                CodeSymbolRef::new(&def.symbol)
            } else {
                CodeSymbolRef::qualified(&m, &def.symbol)
            };
            g.get_or_create_code(&sym, &def.kind, None);
        }
        // вызовы: рёбра caller → callee
        for cs in &table.calls {
            let caller_mod = module_of(&cs.file);
            let caller = if caller_mod.is_empty() {
                CodeSymbolRef::new(&cs.caller)
            } else {
                CodeSymbolRef::qualified(&caller_mod, &cs.caller)
            };
            // разрешение callee: приоритет — определение В ТОМ ЖЕ ФАЙЛЕ
            // (лексический скоуп: file-local def затеняет одноимённые символы
            // других модулей), затем первое найденное; неизвестный callee —
            // внешний символ (kind `extern/macro`)
            let resolved = table.resolve(&cs.callee);
            let callee_def = resolved
                .iter()
                .find(|d| d.file == cs.file)
                .or_else(|| resolved.first());
            let mut callee = CodeSymbolRef::new(&cs.callee);
            if let Some(def) = callee_def {
                let m = module_of(&def.file);
                if !m.is_empty() {
                    callee = CodeSymbolRef::qualified(&m, &cs.callee);
                }
            }
            g.add_code_triple(&caller, "fn", "calls", &callee, "extern/macro", None, 1.0);
        }
        g
    }

    /// Экспорт графа в SQL (схема memory_graph из super-z-skills:
    /// entities/relations с UNIQUE-констрейнтами и temporal-слоем).
    ///
    /// Результат можно загрузить в SQLite: `sqlite3 graph.db < dump.sql`.
    pub fn export_sql(&self, out: &mut String) {
        fn esc(s: &str) -> String {
            s.replace('\'', "''")
        }
        out.push_str("-- POLER-Engine entity graph export\n");
        out.push_str("-- Схема заимствована из super-z-skills memory_graph.py\n");
        out.push_str(
            "CREATE TABLE IF NOT EXISTS entities (\n\
             \x20 id INTEGER PRIMARY KEY,\n\
             \x20 name TEXT NOT NULL,\n\
             \x20 type TEXT DEFAULT 'entity',\n\
             \x20 temporal_layer TEXT,\n\
             \x20 UNIQUE(name)\n);\n\n",
        );
        out.push_str(
            "CREATE TABLE IF NOT EXISTS relations (\n\
             \x20 id INTEGER PRIMARY KEY,\n\
             \x20 subject TEXT NOT NULL,\n\
             \x20 predicate TEXT NOT NULL,\n\
             \x20 object TEXT NOT NULL,\n\
             \x20 UNIQUE(subject, predicate, object)\n);\n\n",
        );

        for idx in self.graph.node_indices() {
            let n = &self.graph[idx];
            let tl = n
                .temporal_layer
                .as_deref()
                .map(|t| format!("'{}'", esc(t)))
                .unwrap_or_else(|| "NULL".to_string());
            out.push_str(&format!(
                "INSERT OR IGNORE INTO entities (name, type, temporal_layer) VALUES ('{}', '{}', {});\n",
                esc(&self.display_name(idx)),
                esc(&n.node_type),
                tl
            ));
        }
        out.push('\n');
        for e in self.graph.edge_references() {
            let s = self.display_name(e.source());
            let o = self.display_name(e.target());
            let p = &e.weight().predicate;
            out.push_str(&format!(
                "INSERT OR IGNORE INTO relations (subject, predicate, object) VALUES ('{}', '{}', '{}');\n",
                esc(&s),
                esc(p),
                esc(&o)
            ));
        }
    }
}

impl Default for EntityGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aidde::symbols::{CallSite, Definition, ImportStmt};

    fn def(symbol: &str, kind: &str, file: &str, line: usize) -> Definition {
        Definition {
            symbol: symbol.to_string(),
            kind: kind.to_string(),
            file: file.to_string(),
            line,
            byte: 0,
        }
    }

    fn call(caller: &str, callee: &str, file: &str, line: usize) -> CallSite {
        CallSite {
            caller: caller.to_string(),
            callee: callee.to_string(),
            file: file.to_string(),
            line,
        }
    }

    // ---------- Задача 1: строгая идентичность символов кода ----------

    #[test]
    fn code_symbols_case_sensitive_three_distinct() {
        let mut g = EntityGraph::with_policy(IdentityPolicy::CodeSymbol);
        g.add_code_triple(
            &CodeSymbolRef::new("Foo"), "fn", "calls",
            &CodeSymbolRef::new("bar"), "fn", None, 1.0,
        );
        g.add_code_triple(
            &CodeSymbolRef::new("foo"), "fn", "calls",
            &CodeSymbolRef::new("bar"), "fn", None, 1.0,
        );
        g.add_code_triple(
            &CodeSymbolRef::new("FOO"), "struct", "calls",
            &CodeSymbolRef::new("bar"), "fn", None, 1.0,
        );
        // ТРИ разных узла: регистр — часть идентичности
        assert_eq!(g.node_count(), 4, "Foo + foo + FOO + bar");
        assert!(g.find_code_symbol(None, "Foo").is_some());
        assert!(g.find_code_symbol(None, "foo").is_some());
        assert!(g.find_code_symbol(None, "FOO").is_some());
        // неточный регистр — НЕ находит (в отличие от текстовой политики)
        assert!(g.find_code_symbol(None, "fOO").is_none());
        assert!(g.find_code_symbol(None, "foo ").is_none());
    }

    #[test]
    fn text_policy_still_merges_case_insensitive() {
        // регресс: текстовое поведение v0.20 не изменилось
        let mut g = EntityGraph::new();
        g.add_triple("нокс", "действует", "шунт", None, 1.0);
        g.add_triple("Нокс", "взаимодействует_с", "Шунт", None, 1.0);
        assert_eq!(g.node_count(), 2); // нокс==Нокс, шунт==Шунт
        assert_eq!(g.find_node("НОКС"), g.find_node("нокс"));
    }

    #[test]
    fn module_path_qualifies_same_name() {
        let mut g = EntityGraph::with_policy(IdentityPolicy::CodeSymbol);
        g.add_code_triple(
            &CodeSymbolRef::qualified("parser", "parse"), "fn", "calls",
            &CodeSymbolRef::new("lex"), "fn", None, 1.0,
        );
        g.add_code_triple(
            &CodeSymbolRef::qualified("cli", "parse"), "fn", "calls",
            &CodeSymbolRef::new("lex"), "fn", None, 1.0,
        );
        // два одноимённых символа в разных модулях — разные узлы
        assert_eq!(g.node_count(), 3);
        assert!(g.find_code_symbol(Some("parser"), "parse").is_some());
        assert!(g.find_code_symbol(Some("cli"), "parse").is_some());
        assert_ne!(
            g.find_code_symbol(Some("parser"), "parse"),
            g.find_code_symbol(Some("cli"), "parse")
        );
        // голое имя неоднозначно → None (не угадываем модуль)
        assert_eq!(g.find_code_symbol(None, "parse"), None);
    }

    #[test]
    fn module_path_unique_suffix_resolves_bare_name() {
        let mut g = EntityGraph::with_policy(IdentityPolicy::CodeSymbol);
        g.add_code_triple(
            &CodeSymbolRef::qualified("engine", "alloc_buffer"), "fn", "calls",
            &CodeSymbolRef::new("memset"), "extern/macro", None, 1.0,
        );
        // единственный alloc_buffer::* → голое имя резолвится суффиксом
        assert!(g.find_code_symbol(None, "alloc_buffer").is_some());
        // K-hop от display-имени движка работает
        let rel = g.extract_k_hop("engine::alloc_buffer", 1, None, 64);
        assert!(
            rel.iter().any(|(s, p, o)| s == "engine::alloc_buffer" && p == "calls" && o == "memset"),
            "{rel:?}"
        );
    }

    #[test]
    fn code_display_name_never_upgraded() {
        let mut g = EntityGraph::with_policy(IdentityPolicy::CodeSymbol);
        g.add_code_triple(
            &CodeSymbolRef::new("foo"), "fn", "calls",
            &CodeSymbolRef::new("bar"), "fn", None, 1.0,
        );
        // «повышение регистра» из текстовой политики не применяется
        g.add_code_triple(
            &CodeSymbolRef::new("foo"), "fn", "reads",
            &CodeSymbolRef::new("bar"), "fn", None, 1.0,
        );
        let idx = g.find_code_symbol(None, "foo").unwrap();
        assert_eq!(g.display_name(idx), "foo");
    }

    #[test]
    fn code_triples_case_dont_collide_in_dedup() {
        let mut g = EntityGraph::with_policy(IdentityPolicy::CodeSymbol);
        g.add_code_triple(
            &CodeSymbolRef::new("Foo"), "fn", "calls",
            &CodeSymbolRef::new("bar"), "fn", None, 1.0,
        );
        g.add_code_triple(
            &CodeSymbolRef::new("foo"), "fn", "calls",
            &CodeSymbolRef::new("bar"), "fn", None, 1.0,
        );
        // обе тройки живут раздельно (текстовая политика бы слила)
        let rels = g.extract_k_hop("bar", 1, None, 64);
        assert!(rels.iter().any(|(s, _, _)| s == "Foo"), "{rels:?}");
        assert!(rels.iter().any(|(s, _, _)| s == "foo"), "{rels:?}");
    }

    #[test]
    fn duplicate_call_edges_deduped() {
        let mut g = EntityGraph::with_policy(IdentityPolicy::CodeSymbol);
        let caller = CodeSymbolRef::new("main");
        let callee = CodeSymbolRef::new("helper");
        g.add_code_triple(&caller, "fn", "calls", &callee, "fn", None, 1.0);
        g.add_code_triple(&caller, "fn", "calls", &callee, "fn", None, 1.0);
        g.add_code_triple(&caller, "fn", "calls", &callee, "fn", None, 1.0);
        assert_eq!(g.edge_count(), 1, "повторные call sites не множат ребро");
        // но другой предикат — отдельное ребро
        g.add_code_triple(&caller, "fn", "reads", &callee, "fn", None, 1.0);
        assert_eq!(g.edge_count(), 2);
    }

    #[test]
    fn from_symbol_table_rust_case_strict() {
        // Rust: fn Foo, fn foo и struct FOO в одном модуле — три символа
        let table = SymbolTable::from_parts(
            vec![
                def("Foo", "fn", "src/engine.rs", 10),
                def("foo", "fn", "src/engine.rs", 20),
                def("FOO", "struct", "src/engine.rs", 30),
            ],
            vec![
                call("Foo", "foo", "src/engine.rs", 12),
                call("foo", "FOO", "src/engine.rs", 22),
            ],
            vec![],
        );
        let g = EntityGraph::from_symbol_table(&table);
        assert_eq!(g.node_count(), 3);
        assert!(g.find_code_symbol(Some("engine"), "Foo").is_some());
        assert!(g.find_code_symbol(Some("engine"), "foo").is_some());
        assert!(g.find_code_symbol(Some("engine"), "FOO").is_some());
        // вызовы сохраняют регистр: Foo→foo, foo→FOO
        let rels = g.extract_k_hop("engine::Foo", 1, None, 64);
        assert!(
            rels.iter().any(|(s, p, o)| s == "engine::Foo" && p == "calls" && o == "engine::foo"),
            "{rels:?}"
        );
        let rels2 = g.extract_k_hop("engine::foo", 1, None, 64);
        assert!(
            rels2.iter().any(|(s, p, o)| s == "engine::foo" && p == "calls" && o == "engine::FOO"),
            "{rels2:?}"
        );
    }

    #[test]
    fn from_symbol_table_python_js_same_name_distinct() {
        // Python: class Foo + def foo; JS: function foo + class Foo —
        // идентичность не зависит от языка: регистр + модуль решают
        let table = SymbolTable::from_parts(
            vec![
                def("Foo", "class", "app/models.py", 1),
                def("foo", "def", "app/utils.py", 1),
                def("foo", "function", "src/app.js", 1),
                def("Foo", "class", "src/app.js", 40),
            ],
            vec![
                call("foo", "Foo", "app/utils.py", 3),
                call("foo", "Foo", "src/app.js", 3),
            ],
            vec![],
        );
        let g = EntityGraph::from_symbol_table(&table);
        // models::Foo, utils::foo, app::foo, app::Foo — 4 узла
        assert_eq!(g.node_count(), 4, "регистр и модуль разделяют все четыре");
        assert_ne!(
            g.find_code_symbol(Some("app"), "foo"),
            g.find_code_symbol(Some("app"), "Foo"),
            "JS: function foo и class Foo в одном файле — разные символы"
        );
        // рёбра из разных модулей идут в разные узлы Foo
        let r1 = g.extract_k_hop("utils::foo", 1, None, 64);
        assert!(r1.iter().any(|(_, p, o)| p == "calls" && o == "models::Foo"), "{r1:?}");
        let r2 = g.extract_k_hop("app::foo", 1, None, 64);
        assert!(r2.iter().any(|(_, p, o)| p == "calls" && o == "app::Foo"), "{r2:?}");
    }

    #[test]
    fn from_symbol_table_extern_callee_no_def() {
        // вызов внешнего символа без определения (println! и т.п.)
        let table = SymbolTable::from_parts(
            vec![def("main", "fn", "src/main.rs", 1)],
            vec![call("main", "println", "src/main.rs", 2)],
            vec![],
        );
        let g = EntityGraph::from_symbol_table(&table);
        assert_eq!(g.node_count(), 2);
        let rels = g.extract_k_hop("main::main", 1, None, 64);
        assert!(rels.iter().any(|(s, _, o)| s == "main::main" && o == "println"), "{rels:?}");
    }

    #[test]
    fn code_graph_export_sql_qualified_names() {
        let table = SymbolTable::from_parts(
            vec![
                def("parse", "fn", "src/parser.rs", 5),
                def("lex", "fn", "src/parser.rs", 9),
            ],
            vec![call("parse", "lex", "src/parser.rs", 7)],
            vec![],
        );
        let g = EntityGraph::from_symbol_table(&table);
        let mut sql = String::new();
        g.export_sql(&mut sql);
        assert!(sql.contains("'parser::parse'"), "{sql}");
        assert!(sql.contains("'parser::lex'"), "{sql}");
        assert!(sql.contains("INSERT OR IGNORE INTO relations (subject, predicate, object) VALUES ('parser::parse', 'calls', 'parser::lex');"), "{sql}");
    }

    #[test]
    fn symbol_table_parts_type_check() {
        // защита от дрейфа сигнатуры from_parts (используется в тестах выше)
        let _t: SymbolTable = SymbolTable::from_parts(
            vec![def("x", "fn", "a.rs", 1)],
            vec![],
            vec![ImportStmt { file: "a.rs".into(), module: "std".into() }],
        );
    }

    // ---------- регресс текстовой политики (v0.20) ----------

    #[test]
    fn k_hop_bfs_depth_two() {
        let mut g = EntityGraph::new();
        g.add_triple("Нокс", "вонзила_когти", "Солнечное сплетение", None, 1.0);
        g.add_triple("Нокс", "взаимодействует_с", "Шунт", None, 1.0);
        g.add_triple("Шунт", "сбрасывает_тепло", "1300°C", None, 1.0);
        g.add_triple("Шунт", "появляется_в", "Глава 36", None, 1.0);

        let rel = g.extract_k_hop("Нокс", 2, None, 64);
        let flat: Vec<String> = rel.iter().map(|(s, p, o)| format!("{s}|{p}|{o}")).collect();
        assert!(flat.iter().any(|s| s == "Нокс|вонзила_когти|Солнечное сплетение"), "{flat:?}");
        assert!(flat.iter().any(|s| s == "Шунт|сбрасывает_тепло|1300°C"), "{flat:?}");
    }

    #[test]
    fn k_hop_incoming_edges_traversed() {
        let mut g = EntityGraph::new();
        g.add_triple("Соболь", "по_имени", "Нокс", None, 1.0);
        let rel = g.extract_k_hop("Нокс", 1, None, 64);
        assert_eq!(rel.len(), 1);
        assert_eq!(rel[0], ("Соболь".into(), "по_имени".into(), "Нокс".into()));
    }

    #[test]
    fn case_insensitive_lookup_and_display_upgrade() {
        let mut g = EntityGraph::new();
        g.add_triple("нокс", "действует", "шунт", None, 1.0);
        g.add_triple("Нокс", "взаимодействует_с", "Шунт", None, 1.0);
        let rel = g.extract_k_hop("НОКС", 1, None, 64);
        assert!(rel.iter().any(|(s, _, _)| s == "Нокс"), "{rel:?}");
    }

    #[test]
    fn temporal_filter_drops_foreign_layers() {
        let mut g = EntityGraph::new();
        g.add_triple("Нокс", "появляется_в", "Глава 36", Some("Т-23"), 1.0);
        g.add_triple("Нокс", "появляется_в", "Глава 9", Some("Т-5"), 1.0);
        g.add_triple("Нокс", "живёт", "Каньон", None, 1.0);

        let all = g.extract_k_hop("Нокс", 1, None, 64);
        assert_eq!(all.len(), 3);

        let filtered = g.extract_k_hop("Нокс", 1, Some("Т-23"), 64);
        let flat: Vec<String> = filtered.iter().map(|(s, p, o)| format!("{s}|{p}|{o}")).collect();
        assert!(flat.iter().any(|s| s.contains("Глава 36")), "{flat:?}");
        assert!(flat.iter().any(|s| s.contains("Каньон")), "{flat:?}");
        assert!(!flat.iter().any(|s| s.contains("Глава 9")), "{flat:?}");
    }

    #[test]
    fn max_relations_cap() {
        let mut g = EntityGraph::new();
        for i in 0..20 {
            g.add_triple("Нокс", "рёбра", &format!("obj_{i}"), None, 1.0);
        }
        let rel = g.extract_k_hop("Нокс", 1, None, 5);
        assert_eq!(rel.len(), 5);
    }

    #[test]
    fn missing_entity_returns_empty() {
        let mut g = EntityGraph::new();
        g.add_triple("a", "b", "c", None, 1.0);
        assert!(g.extract_k_hop("нет такой", 2, None, 10).is_empty());
    }

    #[test]
    fn empty_triple_ignored() {
        let mut g = EntityGraph::new();
        g.add_triple("", "p", "o", None, 1.0);
        g.add_triple("s", "", "o", None, 1.0);
        assert_eq!(g.node_count(), 0);
    }
}

// ====================================================================
// v0.90.0 SYNAPSE-TRIT: АЗУ в конвейере ранжирования рёбер
// ====================================================================

/// Точное разложение f64-веса с затуханием в каноническую дробь [N:D].
///
/// `lens = w · 2^(−depth)` — БЕЗ единого округления: f64 = ±m·2^(E−1075)
/// (m — 53-битная мантисса с неявным битом), значит lens — диадическая
/// рациональность. Общие степени двойки сокращаются (сдвигами m и
/// знаменателя), произведения в [`crate::asm::zero_asm::cmp_canonical`]
/// ограничены 2⁶²·2⁶² = 2¹²⁴ < 2¹²⁷ — сравнение ТОЧНО там, где f64
/// цепочка «w·0.5^depth → f32-каст → SIMD-сравнение» давала ДВЕ
/// погрешности на каждом ребре.
///
/// Диапазон точности: |value| ∈ [2⁻⁶², 2⁶²] — весь реальный диапазон
/// семантических весов; за пределами — насыщение (документировано).
pub fn lens_weight_proj(w: f64, depth: usize) -> crate::asm::zero_asm::Proj {
    use crate::asm::zero_asm::Proj;
    if w == 0.0 || !w.is_finite() {
        return Proj::ZERO;
    }
    let bits = w.to_bits();
    let sign = if bits >> 63 == 1 { -1i64 } else { 1i64 };
    let mant = ((bits & 0x000F_FFFF_FFFF_FFFF) | 0x0010_0000_0000_0000) as i64; // 53 бита
    let e = ((bits >> 52) & 0x7FF) as i32 - 1075; // w = ±mant·2^e
    // lens = ±mant·2^(e−depth): каноническая [N:D], D > 0
    let mut s = e - depth as i32; // показатель после затухания
    let mut m = mant;
    // сокращение общих степеней двойки (ТОЧНО — сдвиги):
    let tz = m.trailing_zeros() as i32;
    if s < 0 && tz > 0 {
        let t = tz.min(-s);
        m >>= t;
        s += t;
    }
    if s >= 0 {
        if s <= 9 {
            // N = ±m·2^s ≤ 2^53·2^9 = 2^62 — произведения cmp ≤ 2^124 < 2^127
            Proj {
                n: sign * (m << s),
                d: 1,
            }
        } else {
            // |value| ≥ 2^62 — за пределами точного диапазона: насыщение
            // максимумом (документировано; семантических весов таких нет)
            Proj {
                n: sign.saturating_mul(i64::MAX / 2),
                d: 1,
            }
        }
    } else {
        // знаменатель 2^(−s), ограничение 2^62:
        let dd = -s;
        if dd > 62 {
            // значение < 2^-62 — насыщение минимальной представимой точкой
            Proj { n: sign, d: 1 << 62 }
        } else {
            Proj {
                n: sign * m,
                d: 1i64 << dd,
            }
        }
    }
}

impl EntityGraph {
    /// K-hop рёбра с ТОЧНЫМИ проективными весами ([N:D], без f64-кастов).
    ///
    /// v0.90 АЗУ: то же семантическое дерево обхода, что
    /// [`extract_k_hop_weighted`], но lens-вес ребра — точная диадическая
    /// дробь `w·2^−depth` (см. [`lens_weight_proj`]). Ранжирование и
    /// барьер LENS на таких весах — БЕЗ погрешности.
    pub fn extract_k_hop_proj(
        &self,
        entity: &str,
        k: usize,
        temporal_filter: Option<&str>,
        max_relations: usize,
    ) -> Vec<(String, String, String, crate::asm::zero_asm::Proj)> {
        use crate::asm::zero_asm::Proj;
        let mut out: Vec<(String, String, String, Proj)> = Vec::new();
        let start = match self.policy {
            IdentityPolicy::Text => match self.find_node(entity) {
                Some(i) => i,
                None => return out,
            },
            IdentityPolicy::CodeSymbol => match self
                .find_code_symbol(None, entity)
                .or_else(|| self.find_node(entity))
            {
                Some(i) => i,
                None => return out,
            },
        };
        let mut visited: HashSet<NodeIndex> = HashSet::new();
        visited.insert(start);
        let mut queue: VecDeque<(NodeIndex, usize)> = VecDeque::new();
        queue.push_back((start, 0));
        let mut seen_triples: HashSet<(String, String, String)> = HashSet::new();
        while let Some((node, depth)) = queue.pop_front() {
            if depth >= k || out.len() >= max_relations {
                continue;
            }
            for dir in [Direction::Outgoing, Direction::Incoming] {
                for edge in self.graph.edges_directed(node, dir) {
                    let (src, dst) = match dir {
                        Direction::Outgoing => (node, edge.target()),
                        Direction::Incoming => (edge.source(), node),
                    };
                    if self.temporal_ok(src, dst, temporal_filter) {
                        let s = self.display_name(src);
                        let o = self.display_name(dst);
                        let t = (s, edge.weight().predicate.clone(), o);
                        if seen_triples.insert(self.triple_dedup_key(&t.0, &t.1, &t.2)) {
                            out.push((
                                t.0,
                                t.1,
                                t.2,
                                lens_weight_proj(edge.weight().weight, depth),
                            ));
                            if out.len() >= max_relations {
                                break;
                            }
                        }
                    }
                    let other = match dir {
                        Direction::Outgoing => edge.target(),
                        Direction::Incoming => edge.source(),
                    };
                    if !visited.contains(&other) {
                        visited.insert(other);
                        queue.push_back((other, depth + 1));
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod lens_proj_tests {
    use super::lens_weight_proj;
    use crate::asm::zero_asm::{cmp_canonical, Proj};

    #[test]
    fn lens_proj_dyadic_exactness() {
        // 0.5·2^-1 = 0.25 = [1:4] — ТОЧНО
        let p = lens_weight_proj(0.5, 1);
        assert_eq!((p.n, p.d), (1, 4));
        // 1.0 без затухания = [1:1]
        let p = lens_weight_proj(1.0, 0);
        assert_eq!((p.n, p.d), (1, 1));
        // 0.75·2^-2 = 3/16 = [3:16] (0.75 = 3·2^-2 — сокращение!)
        let p = lens_weight_proj(0.75, 2);
        assert_eq!((p.n, p.d), (3, 16));
        // ноль — абсолютный ноль [0:1]
        assert_eq!(lens_weight_proj(0.0, 3).n, 0);
    }

    #[test]
    fn lens_proj_value_roundtrip_exact() {
        // value_f64 == w·2^-depth БИТ-В-БИТ (деление на степень двойки
        // в f64 точно; f64-умножение на 0.5^k тоже точно)
        let mut s = 0xABCDu64 | 1;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        for _ in 0..1000 {
            let w = f64::from_bits((next() >> 11) as u64);
            if !w.is_finite() || w == 0.0 {
                continue;
            }
            for depth in 0..5usize {
                let expect = w * 0.5f64.powi(depth as i32);
                // точный диапазон АЗУ-разложения: [2^-62, 2^62] —
                // за пределами НАСЫЩЕНИЕ (документировано: защита
                // i128-произведений cmp_canonical ≤ 2^124)
                if expect.is_normal() && expect.abs() >= 2f64.powi(-62) && expect.abs() <= 2f64.powi(62) {
                    let got = lens_weight_proj(w, depth).value_f64();
                    assert_eq!(got.to_bits(), expect.to_bits(), "w={w} depth={depth}");
                }
            }
        }
    }

    #[test]
    fn lens_proj_exact_vs_f64_boundary() {
        // точный барьер LENS: w > 0.05 — проективно, без f32-каста
        let barrier = lens_weight_proj(0.05f32 as f64, 0);
        // вес ровно НАД барьером (ближайший f32-сосед сверху):
        let above = f32::from_bits(0.05f32.to_bits() + 1);
        let wa = lens_weight_proj(above as f64, 0);
        assert_eq!(cmp_canonical(&wa, &barrier), std::cmp::Ordering::Greater);
        // вес ровно ПОД барьером:
        let below = f32::from_bits(0.05f32.to_bits() - 1);
        let wb = lens_weight_proj(below as f64, 0);
        assert_eq!(cmp_canonical(&wb, &barrier), std::cmp::Ordering::Less);
        // сам барьер — равен (не строго больше): НЕ проходит
        assert_eq!(cmp_canonical(&barrier, &barrier), std::cmp::Ordering::Equal);
        // каноническая форма: d > 0 всегда
        for &w in &[0.05f64, 1e-10, 1e10, 0.3] {
            assert!(lens_weight_proj(w, 3).d > 0);
        }
    }
}
