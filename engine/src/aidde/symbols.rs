//! Глобальная таблица символов и call graph проекта.
//!
//! Лексический уровень (без полного парсера): определения извлекаются
//! построчной сигнатурной эвристикой, вызовы — паттерном `ident(`,
//! скоуп вызова определяется лексическим сканером [`crate::parser::ast_code`]
//! (строки/raw-строки/комментарии корректно пропускаются).

use rayon::prelude::*;
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;
use crate::parser::ast_code;
use crate::parser::triples::CALL_KEYWORDS;
use crate::parser::{detect_lang, CodeLang};

/// Определение символа (функция/структура/класс/…).
#[derive(Debug, Clone, Serialize)]
pub struct Definition {
    /// Имя символа (`validate`).
    pub symbol: String,
    /// Вид определения (`fn`, `struct`, `class`, `def`, …).
    pub kind: String,
    /// Файл определения.
    pub file: String,
    /// Строка определения (1-based).
    pub line: usize,
    /// Байтовое смещение начала сигнатуры.
    pub byte: usize,
}

/// Вызов: `caller` вызывает `callee` в файле `file` на строке `line`.
#[derive(Debug, Clone, Serialize)]
pub struct CallSite {
    pub caller: String,
    pub callee: String,
    pub file: String,
    pub line: usize,
}

/// Импорт модуля в файле.
#[derive(Debug, Clone, Serialize)]
pub struct ImportStmt {
    pub file: String,
    pub module: String,
}

static CALL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap());

static IMPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^\s*(?:pub(?:\s+\([^)]*\))?\s+)?use\s+([A-Za-z0-9_:]+)|^\s*from\s+([\w.]+)\s+import|^\s*import\s+([\w.]+)|^\s*#\s*include\s+[<"]([^>"]+)[>"]|^\s*import\s+[^;\n]*?from\s+['"]([^'"]+)['"]|=\s*require\(\s*['"]([^'"]+)['"]"#,
    )
    .unwrap()
});

/// v0.86.0 (ГРАБЛЯ 44, sqlite3.c-монолит): индекс переводов строки —
/// O(N) построение одним SIMD-memchr-проходом, O(log N) запрос
/// (partition_point). Прежняя `line_of_byte` сканировала файл от байта 0
/// на КАЖДЫЙ def и call-site — на монолите 250k строк это Σ(off) ≈
/// 4×10¹⁰ байт-операций (квадрат по CPU при линейном IO).
struct LineIndex {
    /// Байтовые позиции всех '\n' (отсортированы по построению).
    nl: Vec<usize>,
}

impl LineIndex {
    fn build(text: &str) -> Self {
        Self {
            nl: memchr::memchr_iter(b'\n', text.as_bytes()).collect(),
        }
    }

    /// Номер строки (1-based) байтового смещения — семантика прежней
    /// `line_of_byte`: число '\n' строго до `byte`, +1.
    fn line_of(&self, byte: usize) -> usize {
        self.nl.partition_point(|&p| p < byte) + 1
    }
}

fn import_of(line: &str) -> Option<String> {
    let caps = IMPORT_RE.captures(line)?;
    for g in 1..=6 {
        if let Some(m) = caps.get(g) {
            let s = m.as_str().trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// Последний сегмент qualified-имени (`a::b::c` / `a.b.c` -> `c`).
pub fn last_segment_is(s: &str) -> &str {
    last_segment(s)
}

fn last_segment(s: &str) -> &str {
    s.rsplit("::")
        .next()
        .unwrap_or(s)
        .rsplit('.')
        .next()
        .unwrap_or(s)
}

/// Глобальная таблица символов проекта.
pub struct SymbolTable {
    /// Все определения.
    pub defs: Vec<Definition>,
    /// Все вызовы (call graph).
    pub calls: Vec<CallSite>,
    /// Все импорты.
    pub imports: Vec<ImportStmt>,
    by_name: HashMap<String, Vec<usize>>,
}

impl SymbolTable {
    /// Сканирует кодовые файлы (Rust/Python/C/JS/…) и строит таблицу.
    ///
    /// v0.6: параллельная сборка (rayon par_iter) — файлы независимы,
    /// результаты сливаются в порядке исходного списка (детерминизм).
    pub fn build(files: &[PathBuf], max_file_bytes: u64) -> Self {
        let per_file: Vec<(Vec<Definition>, Vec<CallSite>, Vec<ImportStmt>)> = files
            .par_iter()
            .map(|path| scan_one_file(path, max_file_bytes))
            .collect();

        let mut defs: Vec<Definition> = Vec::new();
        let mut calls: Vec<CallSite> = Vec::new();
        let mut imports: Vec<ImportStmt> = Vec::new();
        for (d, c, i) in per_file {
            defs.extend(d);
            calls.extend(c);
            imports.extend(i);
        }
        Self::from_parts(defs, calls, imports)
    }

    /// Сборка таблицы из готовых частей (используется SQLite-бэкендом).
    pub fn from_parts(
        defs: Vec<Definition>,
        calls: Vec<CallSite>,
        imports: Vec<ImportStmt>,
    ) -> Self {
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, d) in defs.iter().enumerate() {
            by_name.entry(d.symbol.clone()).or_default().push(i);
        }
        Self {
            defs,
            calls,
            imports,
            by_name,
        }
    }
}

/// Разбор одного файла в (defs, calls, imports).
pub(crate) fn scan_one_file(
    path: &PathBuf,
    max_file_bytes: u64,
) -> (Vec<Definition>, Vec<CallSite>, Vec<ImportStmt>) {
    let mut defs: Vec<Definition> = Vec::new();
    let mut calls: Vec<CallSite> = Vec::new();
    let mut imports: Vec<ImportStmt> = Vec::new();

    if detect_lang(path) == CodeLang::Plain {
        return (defs, calls, imports);
    }
    let Ok(meta) = std::fs::metadata(path) else {
        return (defs, calls, imports);
    };
    if meta.len() > max_file_bytes {
        return (defs, calls, imports);
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return (defs, calls, imports);
    };
    let lang = detect_lang(path);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    #[allow(unused_variables)]
    let file_s = path.to_string_lossy().to_string();

let file_s = path.to_string_lossy().to_string();

    // --- определения + импорты (построчно) ---
    // v0.86.0 (ГРАБЛЯ 44): номер строки — инкрементальный счётчик цикла
    // (бесплатно), байтовое смещение продвигается с учётом \r\n
    // (`lines()` срезает \r — прежний `off += len + 1` дрейфовал на
    // CRLF-файлах). Прежний вызов line_of_byte пересканировал текст
    // от байта 0 на каждый def — O(N²) на монолитах.
    let text_bytes = text.as_bytes();
    let mut off = 0usize;
    let mut line_no = 1usize;
    for line in text.lines() {
        if let Some((kind, name)) = ast_code::signature_of(line) {
            defs.push(Definition {
                symbol: name,
                kind,
                file: file_s.clone(),
                line: line_no,
                byte: off,
            });
        }
        if let Some(module) = import_of(line) {
            imports.push(ImportStmt {
                file: file_s.clone(),
                module,
            });
        }
        let mut next = off + line.len();
        if text_bytes.get(next) == Some(&b'\r') {
            next += 1;
        }
        if text_bytes.get(next) == Some(&b'\n') {
            next += 1;
        }
        off = next;
        line_no += 1;
    }

    // --- вызовы (call graph) ---
    // Спаны генерируются последовательно => уже отсортированы по
    // началу. Бинарный поиск: O(log K) на вызов вместо O(K).
    // (v0.6: на Wireshark-файлах с 5000+ спанов убирает O(M×N).)
    let noise_spans = ast_code::string_comment_spans(&text);
    let in_noise = |off: usize| -> bool {
        match noise_spans.binary_search_by(|(s, e)| {
            if off < *s {
                std::cmp::Ordering::Greater
            } else if off >= *e {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        }) {
            Ok(_) => true,
            // Err(insert_pos): спан слева от insert_pos может
            // накрывать off (перекрывающихся спанов нет — они
            // дизъюнктны по построению)
            Err(0) => false,
            Err(ip) => {
                let (s, e) = noise_spans[ip - 1];
                off >= s && off < e
            }
        }
    };

    // v0.86.0 (ГРАБЛЯ 44): один O(N) SIMD-индекс '\n' на файл,
    // O(log N) запрос на вызов — вместо O(byte) рескана от начала файла
    // на каждый call-site (монолит sqlite3.c: ~100k вызовов × ~4 МБ
    // среднего смещения ≈ 4×10¹⁰ байт-сканов → один проход 8.5 МБ).
    let line_index = LineIndex::build(&text);

    // v0.86.0 (ГРАБЛЯ 44, вторая точка): инкрементальный локатор скоупов —
    // brace_bounds прежде сканировал файл от байта 0 на КАЖДЫЙ вызов
    // (O(N²) по CPU на монолитах). captures_iter даёт вызовы по возрастанию
    // смещений → лексер проходит файл ровно один раз (O(N) суммарно).
    let mut locator = ast_code::ScopeLocator::new(&text);

    for caps in CALL_RE.captures_iter(&text) {
        let callee = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
        if callee.is_empty() || CALL_KEYWORDS.contains(&callee.as_str()) {
            continue;
        }
        let call_off = caps.get(0).map(|m| m.start()).unwrap_or(0);

        // вызовы внутри строковых литералов и комментариев — шум
        if in_noise(call_off) {
            continue;
        }

        let scope = match lang {
            CodeLang::Brace => locator.locate(call_off),
            _ => ast_code::locate_scope(&text, call_off, lang),
        };

        // Определение вне любого блока (например, `fn inner() {}`
        // на верхнем уровне): сигнатура строки совпадает с callee.
        if scope.is_none() {
            let line_sig = ast_code::light_signature(&text, call_off);
            if line_sig.as_deref() == Some(callee.as_str()) {
                continue;
            }
        }

        let (scope_b, scope_name) = match scope {
            Some((b, _)) => (b, ast_code::light_signature(&text, b)),
            None => (0, None),
        };
        let caller = match &scope_name {
            Some(n) => format!("{stem}::{n}"),
            None => stem.clone(),
        };

        // Совпадение в строке сигнатуры — это определение, не вызов:
        // пропуск, если имя совпадает со скоупом и смещение до '{'
        // (brace-языки) / в пределах строки заголовка (Python).
        if let Some(name) = &scope_name {
            if *name == callee {
                let header_end = match lang {
                    CodeLang::Brace => text[scope_b..]
                        .find('{')
                        .map(|i| scope_b + i)
                        .unwrap_or(scope_b),
                    _ => text[scope_b..]
                        .find('\n')
                        .map(|i| scope_b + i)
                        .unwrap_or(text.len()),
                };
                if call_off <= header_end {
                    continue;
                }
            }
        }

        calls.push(CallSite {
            caller,
            callee,
            file: file_s.clone(),
            line: line_index.line_of(call_off),
        });
    }

    (defs, calls, imports)
}

impl SymbolTable {
    /// Разрешает имя символа (с поддержкой qualified `a::b::c`).
    pub fn resolve(&self, name: &str) -> Vec<&Definition> {
        let key = last_segment(name);
        self.by_name
            .get(key)
            .map(|idxs| idxs.iter().map(|&i| &self.defs[i]).collect())
            .unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn write(dir: &Path, name: &str, content: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn definitions_and_calls_extracted() {
        let dir = tempfile::TempDir::new().unwrap();
        write(
            dir.path(),
            "core.rs",
            "pub fn core_fn(x: i32) -> i32 {\n    x + 1\n}\n",
        );
        write(dir.path(), "mid.rs", "pub fn mid() {\n    core_fn(1);\n}\n");
        let files = vec![dir.path().join("core.rs"), dir.path().join("mid.rs")];
        let t = SymbolTable::build(&files, 1024 * 1024);
        assert_eq!(t.defs.len(), 2);
        assert_eq!(t.resolve("core_fn").len(), 1);
        assert_eq!(t.resolve("mid").len(), 1);
        // определение не считается вызовом самого себя
        let self_calls: Vec<&CallSite> = t
            .calls
            .iter()
            .filter(|c| c.caller.ends_with("core_fn"))
            .collect();
        assert!(self_calls.is_empty(), "{:?}", t.calls);
        // реальный вызов зарегистрирован
        assert!(
            t.calls
                .iter()
                .any(|c| c.callee == "core_fn" && c.caller == "mid::mid"),
            "{:?}",
            t.calls
        );
    }

    #[test]
    fn definition_in_signature_not_a_call() {
        let dir = tempfile::TempDir::new().unwrap();
        write(
            dir.path(),
            "a.rs",
            "fn outer() {\n    inner();\n}\nfn inner() {}\n",
        );
        let t = SymbolTable::build(&[dir.path().join("a.rs")], 1024 * 1024);
        // inner() на строке 1 после '{' — реальный вызов
        assert!(t
            .calls
            .iter()
            .any(|c| c.callee == "inner" && c.caller == "a::outer"));
        // определение inner (строка 3) не порождает вызова
        assert_eq!(t.calls.iter().filter(|c| c.callee == "inner").count(), 1);
    }

    #[test]
    fn python_definitions_and_imports() {
        let dir = tempfile::TempDir::new().unwrap();
        write(
            dir.path(),
            "svc.py",
            "import json\n\n\nclass Worker:\n    def run(self):\n        return self.load()\n\n    def load(self):\n        return 1\n",
        );
        let t = SymbolTable::build(&[dir.path().join("svc.py")], 1024 * 1024);
        assert_eq!(t.resolve("Worker").len(), 1);
        assert_eq!(t.resolve("run").len(), 1);
        assert!(t.imports.iter().any(|i| i.module == "json"));
        assert!(t
            .calls
            .iter()
            .any(|c| c.callee == "load" && c.caller == "svc::run"));
    }

    #[test]
    fn calls_inside_strings_ignored() {
        let dir = tempfile::TempDir::new().unwrap();
        write(
            dir.path(),
            "s.rs",
            "fn f() {\n    let s = \"not_a_call(1)\";\n    real();\n}\n",
        );
        let t = SymbolTable::build(&[dir.path().join("s.rs")], 1024 * 1024);
        assert!(t.calls.iter().any(|c| c.callee == "real"));
        assert!(!t.calls.iter().any(|c| c.callee == "not_a_call"));
    }

    #[test]
    fn line_numbers_incremental_and_crlf() {
        // v0.86.0 (ГРАБЛЯ 44): номера строк defs/calls — инкрементальный
        // счётчик и newline-индекс; CRLF больше не дрейфует
        let dir = tempfile::TempDir::new().unwrap();
        // LF-файл: def на строке 3, вызов на строке 5
        let lf = "fn a() {\n    let x = 1;\n}\nfn b() {\n    a();\n}\n";
        write(dir.path(), "lf.rs", lf);
        let t = SymbolTable::build(&[dir.path().join("lf.rs")], 1024 * 1024);
        let def_a = t.defs.iter().find(|d| d.symbol == "a").unwrap();
        let def_b = t.defs.iter().find(|d| d.symbol == "b").unwrap();
        assert_eq!(def_a.line, 1);
        assert_eq!(def_b.line, 4);
        let call = t.calls.iter().find(|c| c.callee == "a").unwrap();
        assert_eq!(call.line, 5);
        // байтовые смещения точны
        assert_eq!(def_b.byte, lf.find("fn b()").unwrap());
        // CRLF-файл: та же морфология — строки и байты не дрейфуют
        let crlf = "fn a() {\r\n    let x = 1;\r\n}\r\nfn b() {\r\n    a();\r\n}\r\n";
        write(dir.path(), "crlf.rs", crlf);
        let t2 = SymbolTable::build(&[dir.path().join("crlf.rs")], 1024 * 1024);
        let def_b2 = t2.defs.iter().find(|d| d.symbol == "b").unwrap();
        assert_eq!(def_b2.line, 4, "CRLF: строка def дрейфует");
        assert_eq!(def_b2.byte, crlf.find("fn b()").unwrap(), "CRLF: байт def дрейфует");
        let call2 = t2.calls.iter().find(|c| c.callee == "a").unwrap();
        assert_eq!(call2.line, 5, "CRLF: строка call дрейфует");
    }
}
