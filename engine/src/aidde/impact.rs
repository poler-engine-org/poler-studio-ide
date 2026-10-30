//! Двунаправленный impact-анализ и Impact Passport (v0.21: Triage Layer).
//!
//! **Upstream** (кто зависит от меня): BFS по обратным рёбрам call graph —
//! все функции и файлы, которые сломаются при изменении символа.
//! **Downstream** (от кого завишу я): BFS по прямым рёбрам — все скрытые
//! зависимости самого символа.
//!
//! v0.21 формализует два РАЗНЫХ класса знаний паспорта:
//!
//! * **[`StructuralRelations`]** — доказанные графом вызовы. Каждое ребро
//!   взято из AST-скана call graph (caller → callee на конкретной строке
//!   файла). Это доказательство, воспроизводимое и проверяемое.
//! * **[`TriageAlert`]** — Triage Layer: эвристический СИГНАЛ ВНИМАНИЯ,
//!   а не псевдо-доказательство. Маркеры (`unsafe`, `.lock()`, `spawn` …)
//!   ищутся подстрокой в теле определения; они говорят «сюда посмотреть»,
//!   но не «здесь есть эффект». Сигнал тревоги ≠ факт.
//!
//! Почему их нельзя смешивать (урок аудита v0.20): смешанный список
//! `side_effects` выглядел для агента одинаково достоверным — вызов из
//! call graph и совпадение подстроки имели один статус. Формализация
//! разделяет их на уровне типа: доказательства — в `structural_relations`,
//! гипотезы — в `heuristic_triage_alerts`.
//!
//! Выход — самодостаточный паспорт для AI-агента:
//!
//! ```json
//! {
//!   "target_function": "engine::alloc_buffer",
//!   "file": "src/engine/allocator.rs",
//!   "lines": "120-145",
//!   "structural_relations": {
//!     "upstream_dependents": [{"caller": "pipeline::decode", "file": "...", "line": 45}],
//!     "downstream_dependencies": [{"callee": "memmap2::MmapMut", "file": "..."}]
//!   },
//!   "heuristic_triage_alerts": [
//!     {"marker": "unsafe", "description": "Блок unsafe — снятые гарантии безопасности памяти",
//!      "category": "memory_safety"}
//!   ],
//!   "danger_level_if_modified": "CRITICAL (затронет 14 файлов)"
//! }
//! ```

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::aidde::symbols::{last_segment_is, Definition, SymbolTable};
use crate::parser::{detect_lang, extract_enclosing_scope};

/// Зависимый (upstream): кто вызывает цель.
#[derive(Debug, Clone, Serialize)]
pub struct Dependent {
    pub caller: String,
    pub file: String,
    pub line: usize,
}

/// Зависимость (downstream): кого вызывает цель.
#[derive(Debug, Clone, Serialize)]
pub struct Dependency {
    pub callee: String,
    pub file: String,
}

/// Категория эвристического сигнала Triage Layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TriageCategory {
    /// Снятие гарантий безопасности памяти (`unsafe`).
    MemorySafety,
    /// Мьютексы, блокировки, порождение потоков/задач.
    Concurrency,
    /// Глобальное/разделяемое состояние (static, GLOBAL, RefCell).
    GlobalState,
    /// Файловый и прочий I/O.
    Io,
    /// Сетевые соединения и сокеты.
    Network,
    /// Управление процессом (exit, сигналы).
    ProcessControl,
    /// Побочный вывод (stdout/stderr).
    Output,
    /// Паники и аварийные завершения.
    Panic,
}

impl TriageCategory {
    /// Короткий человекочитаемый ярлык для CLI-вывода.
    pub fn label(&self) -> &'static str {
        match self {
            TriageCategory::MemorySafety => "память/unsafe",
            TriageCategory::Concurrency => "конкурентность",
            TriageCategory::GlobalState => "глобальное состояние",
            TriageCategory::Io => "ввод-вывод",
            TriageCategory::Network => "сеть",
            TriageCategory::ProcessControl => "процесс",
            TriageCategory::Output => "вывод",
            TriageCategory::Panic => "паника",
        }
    }
}

/// Один эвристический сигнал Triage Layer: СИГНАЛ ВНИМАНИЯ, не доказательство.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TriageAlert {
    /// Маркер, найденный в теле (например `unsafe`, `.lock()`).
    pub marker: String,
    /// Человекочитаемое описание сигнала.
    pub description: String,
    /// Категория тревоги.
    pub category: TriageCategory,
}

/// Официальная таблица Triage Layer (v0.21: бывшие SIDE_EFFECT_MARKERS).
///
/// Регулярные источники скрытых зависимостей, невидимых ни grep, ни
/// векторному RAG. Совпадение маркера — повод посмотреть на код, но не
/// утверждение о наличии эффекта (`.lock()` бывает в комментарии,
/// `connect` — названием локальной переменной).
const TRIAGE_MARKERS: &[(&str, &str, TriageCategory)] = &[
    // ---- память / безопасность ----
    ("unsafe", "Блок unsafe — снятые гарантии безопасности памяти", TriageCategory::MemorySafety),
    // ---- конкурентность ----
    ("Mutex", "Блокировка мьютекса", TriageCategory::Concurrency),
    (".lock()", "Блокировка мьютекса", TriageCategory::Concurrency),
    ("RwLock", "Блокировка чтения-записи", TriageCategory::Concurrency),
    ("spawn", "Порождение потока/задачи", TriageCategory::Concurrency),
    // ---- глобальное состояние ----
    ("static mut", "Мутация глобального состояния", TriageCategory::GlobalState),
    ("static ", "Доступ к глобальной статике", TriageCategory::GlobalState),
    ("lazy_static", "Инициализация глобального состояния", TriageCategory::GlobalState),
    ("GLOBAL", "Доступ к глобальному счётчику/состоянию", TriageCategory::GlobalState),
    ("RefCell", "Внутренняя мутабельность (RefCell)", TriageCategory::GlobalState),
    // ---- ввод-вывод ----
    ("std::fs::", "Файловый I/O", TriageCategory::Io),
    ("File::create", "Создание файла", TriageCategory::Io),
    ("OpenOptions", "Открытие файла", TriageCategory::Io),
    (".write(", "Запись в разделяемый ресурс", TriageCategory::Io),
    // ---- сеть ----
    ("socket", "Сетевой сокет", TriageCategory::Network),
    ("TcpStream", "Сетевое соединение", TriageCategory::Network),
    ("UdpSocket", "Сетевой сокет", TriageCategory::Network),
    ("connect", "Сетевое соединение", TriageCategory::Network),
    // ---- процесс / вывод / паника ----
    ("process::exit", "Завершение процесса", TriageCategory::ProcessControl),
    ("println!", "Вывод в stdout", TriageCategory::Output),
    ("eprintln!", "Вывод в stderr", TriageCategory::Output),
    ("panic!", "Паника", TriageCategory::Panic),
];

/// Triage-скан тела определения: все маркеры, найденные подстрокой.
///
/// Дедуп — по маркеру (один `unsafe` встречается в теле пять раз —
/// сигнал всё равно один). Порядок — порядок таблицы `TRIAGE_MARKERS`
/// (детерминизм для golden-тестов).
pub fn triage_scan(body: &str) -> Vec<TriageAlert> {
    TRIAGE_MARKERS
        .iter()
        .filter(|(m, _, _)| body.contains(m))
        .map(|(m, d, c)| TriageAlert {
            marker: m.to_string(),
            description: d.to_string(),
            category: *c,
        })
        .collect()
}

/// Доказанные графом отношения цели: upstream (кто зависит от меня)
/// и downstream (от кого завишу я). Каждая запись — ребро call graph.
#[derive(Debug, Clone, Serialize)]
pub struct StructuralRelations {
    /// Кто вызывает цель (прямо или транзитивно в пределах глубины BFS).
    pub upstream_dependents: Vec<Dependent>,
    /// Кого вызывает цель (скрытые зависимости).
    pub downstream_dependencies: Vec<Dependency>,
}

impl StructuralRelations {
    pub fn is_empty(&self) -> bool {
        self.upstream_dependents.is_empty() && self.downstream_dependencies.is_empty()
    }
}

/// Impact Passport символа.
#[derive(Debug, Clone, Serialize)]
pub struct ImpactReport {
    pub target_function: String,
    pub file: String,
    pub lines: String,
    /// ДОКАЗАННЫЕ графом вызовы (call graph, воспроизводимо).
    pub structural_relations: StructuralRelations,
    /// ЭВРИСТИЧЕСКИЕ сигналы Triage Layer (внимание, не доказательство).
    pub heuristic_triage_alerts: Vec<TriageAlert>,
    pub danger_level_if_modified: String,
}

fn stem_of(file: &str) -> String {
    Path::new(file)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Строит Impact Passport для символа.
///
/// * `depth` — глубина BFS в обе стороны (2–3 уровня абстракции);
/// * `max_items` — потолок списков зависимостей.
pub fn impact_analysis(
    table: &SymbolTable,
    target: &str,
    depth: usize,
    max_items: usize,
) -> Option<ImpactReport> {
    // Fallback для макросов/extern-символов (printk → _printk в ядре Linux):
    // определения нет, но сотни вызовов по имени — строим паспорт по ним.
    let external_def;
    let def: &Definition = match table.resolve(target).first() {
        Some(d) => d,
        None => {
            let n_callers = table
                .calls
                .iter()
                .filter(|c| c.callee == target)
                .count();
            if n_callers == 0 {
                return None;
            }
            external_def = Definition {
                symbol: target.to_string(),
                kind: "extern/macro".to_string(),
                file: String::new(),
                line: 0,
                byte: 0,
            };
            &external_def
        }
    };

    // Тело определения (enclosing scope) для строк и triage-сигналов.
    let (lines, triage_alerts) = if def.file.is_empty() {
        ("extern".to_string(), Vec::new())
    } else {
        let text = std::fs::read_to_string(&def.file).ok()?;
        let lang = detect_lang(Path::new(&def.file));
        let scope = extract_enclosing_scope(&text, def.byte, lang);
        (
            format!("{}-{}", scope.start_line, scope.end_line),
            triage_scan(&scope.text),
        )
    };

    // ---------- Индексы вызовов (v0.6: O(1) lookup вместо O(N) скана) ----------
    // По callee (для upstream): callee -> список вызовов.
    let mut by_callee: HashMap<&str, Vec<&crate::aidde::symbols::CallSite>> = HashMap::new();
    // По caller (для downstream): caller (last segment) -> список вызовов.
    let mut by_caller: HashMap<&str, Vec<&crate::aidde::symbols::CallSite>> = HashMap::new();
    for cs in &table.calls {
        by_callee.entry(cs.callee.as_str()).or_default().push(cs);
        by_caller.entry(last_segment_is(&cs.caller)).or_default().push(cs);
    }

    // ---------- Upstream: кто зависит от меня (обратные рёбра) ----------
    let mut upstream: Vec<Dependent> = Vec::new();
    let mut seen_calls: HashSet<(String, String, usize)> = HashSet::new();
    let mut level: HashSet<String> = HashSet::from([last_segment_is(target).to_string()]);
    let mut visited: HashSet<String> = level.clone();

    for _ in 0..depth.max(1) {
        let mut next: HashSet<String> = HashSet::new();
        // Кандидаты уровня: все вызовы, чей callee совпадает с одним из level.
        for cs in level.iter().flat_map(|l| {
            by_callee
                .get(l.as_str())
                .map(|v| v.iter().copied())
                .unwrap_or_default()
        }) {
            let key = (cs.caller.clone(), cs.file.clone(), cs.line);
            if seen_calls.insert(key) {
                upstream.push(Dependent {
                    caller: cs.caller.clone(),
                    file: cs.file.clone(),
                    line: cs.line,
                });
            }
            next.insert(last_segment_is(&cs.caller).to_string());
        }
        next.retain(|n| !visited.contains(n));
        if next.is_empty() || upstream.len() >= max_items {
            break;
        }
        visited.extend(next.iter().cloned());
        level = next;
    }
    upstream.truncate(max_items);

    // ---------- Downstream: от кого завишу я (прямые рёбра) ----------
    let mut downstream: Vec<Dependency> = Vec::new();
    let mut seen_dep: HashSet<String> = HashSet::new();
    let mut level: HashSet<String> = HashSet::from([last_segment_is(target).to_string()]);
    let mut visited: HashSet<String> = level.clone();

    for _ in 0..depth.max(1) {
        let mut next: HashSet<String> = HashSet::new();
        for cs in level.iter().flat_map(|l| {
            by_caller
                .get(l.as_str())
                .map(|v| v.iter().copied())
                .unwrap_or_default()
        }) {
            if seen_dep.insert(cs.callee.clone()) {
                let file = table
                    .resolve(&cs.callee)
                    .first()
                    .map(|d| d.file.clone())
                    .unwrap_or_else(|| cs.file.clone());
                downstream.push(Dependency {
                    callee: cs.callee.clone(),
                    file,
                });
            }
            next.insert(last_segment_is(&cs.callee).to_string());
        }
        next.retain(|n| !visited.contains(n));
        if next.is_empty() || downstream.len() >= max_items {
            break;
        }
        visited.extend(next.iter().cloned());
        level = next;
    }
    downstream.truncate(max_items);

    // ---------- Danger level ----------
    // Считается ТОЛЬКО по доказанным структурным отношениям: эвристические
    // triage-сигналы не участвуют (сигнал тревоги не может поднять градус
    // опасности без доказательства зависимостей).
    let files: HashSet<&String> = upstream.iter().map(|d| &d.file).collect();
    let n = files.len();
    let danger = match n {
        0 => "LOW (прямых зависимых не найдено)".to_string(),
        1..=3 => format!("MEDIUM (затронет {n} файл)"),
        4..=10 => format!("HIGH (затронет {n} файлов)"),
        _ => format!("CRITICAL (затронет {n} файлов)"),
    };

    Some(ImpactReport {
        target_function: format!("{}::{}", stem_of(&def.file), def.symbol),
        file: def.file.clone(),
        lines,
        structural_relations: StructuralRelations {
            upstream_dependents: upstream,
            downstream_dependencies: downstream,
        },
        heuristic_triage_alerts: triage_alerts,
        danger_level_if_modified: danger,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triage_markers_categorized() {
        let body = "unsafe { ALLOC_MUTEX.lock() }\nGLOBAL_GAUGE += 1;\nstd::fs::write(p, b)?;\nTcpStream::connect(a)?;\nprocess::exit(1);\n";
        let fx = triage_scan(body);
        let cat_of = |m: &str| {
            fx.iter()
                .find(|a| a.marker == m)
                .unwrap_or_else(|| panic!("маркер {m} не найден в {fx:?}"))
                .category
        };
        assert_eq!(cat_of("unsafe"), TriageCategory::MemorySafety);
        assert_eq!(cat_of(".lock()"), TriageCategory::Concurrency);
        assert_eq!(cat_of("GLOBAL"), TriageCategory::GlobalState);
        assert_eq!(cat_of("std::fs::"), TriageCategory::Io);
        assert_eq!(cat_of("TcpStream"), TriageCategory::Network);
        assert_eq!(cat_of("connect"), TriageCategory::Network);
        assert_eq!(cat_of("process::exit"), TriageCategory::ProcessControl);
    }

    #[test]
    fn triage_dedup_by_marker() {
        // маркер встречается пять раз — сигнал один
        let body = "unsafe {}\nunsafe {}\nunsafe {}\nunsafe {}\nunsafe {}\n";
        let fx = triage_scan(body);
        assert_eq!(fx.len(), 1);
        assert_eq!(fx[0].marker, "unsafe");
    }

    #[test]
    fn triage_order_deterministic() {
        let body = "spawn(b);\nunsafe {}\nMutex::new(x);\n";
        let fx = triage_scan(body);
        // порядок = порядок таблицы TRIAGE_MARKERS, не порядок вхождения
        let markers: Vec<&str> = fx.iter().map(|a| a.marker.as_str()).collect();
        assert_eq!(markers, vec!["unsafe", "Mutex", "spawn"]);
    }

    #[test]
    fn clean_body_no_alerts() {
        assert!(triage_scan("let x = a + b;").is_empty());
    }

    #[test]
    fn category_labels_non_empty() {
        let body = "unsafe {} spawn(x); println!(\"a\"); panic!(\"b\"); eprintln!(\"c\");";
        for a in triage_scan(body) {
            assert!(!a.category.label().is_empty(), "{a:?}");
        }
    }

    #[test]
    fn passport_json_separates_proof_from_heuristics() {
        // serde-форма паспорта: структурные отношения и triage-сигналы —
        // РАЗНЫЕ ключи JSON (защита от регресса к смешанному side_effects)
        let probe = ImpactReport {
            target_function: "a::b".into(),
            file: "a.rs".into(),
            lines: "1-2".into(),
            structural_relations: StructuralRelations {
                upstream_dependents: vec![Dependent {
                    caller: "caller".into(),
                    file: "f.rs".into(),
                    line: 3,
                }],
                downstream_dependencies: vec![Dependency { callee: "callee".into(), file: "g.rs".into() }],
            },
            heuristic_triage_alerts: vec![TriageAlert {
                marker: "unsafe".into(),
                description: "Блок unsafe".into(),
                category: TriageCategory::MemorySafety,
            }],
            danger_level_if_modified: "LOW".into(),
        };
        let json = serde_json::to_string(&probe).unwrap();
        assert!(json.contains("\"structural_relations\""), "{json}");
        assert!(json.contains("\"upstream_dependents\""), "{json}");
        assert!(json.contains("\"downstream_dependencies\""), "{json}");
        assert!(json.contains("\"heuristic_triage_alerts\""), "{json}");
        assert!(json.contains("\"memory_safety\""), "{json}");
        assert!(!json.contains("side_effects"), "{json}");
        assert!(!json.contains("\"category\":\"Concurrency\""), "{json}");
    }
}
