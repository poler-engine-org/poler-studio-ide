#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Интеграционные тесты движка: полный пайплайн на фикстурах,
//! воспроизведение выходного контракта спецификации.
#![allow(clippy::field_reassign_with_default)]

use poler_engine::{
    scan_path, scan_path_with_stats, EngineConfig, PiiMode, ResonanceMode,
};
use std::fs;
use tempfile::TempDir;

const CH36: &str = include_str!("fixtures/chapter_36.md");
const CODE_RS: &str = include_str!("fixtures/example.rs");
const CODE_PY: &str = include_str!("fixtures/example.py");
const PII_TXT: &str = include_str!("fixtures/pii.txt");

fn write_fixture(name: &str, content: &str) -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    fs::write(dir.path().join(name), content).expect("write fixture");
    dir
}

// ---------------------------------------------------------------------------
// Выходной контракт спецификации
// ---------------------------------------------------------------------------

#[test]
fn spec_contract_on_chapter_36() {
    let dir = write_fixture("chapter_36.md", CH36);
    let cfg = EngineConfig::default();
    let res = scan_path(dir.path(), "нокс", &cfg);

    // 3 вхождения: метаданные, "по имени Нокс", "Нокс вонзила"
    assert!(res.total_hits >= 3, "total_hits={}", res.total_hits);
    assert!(!res.anchors.is_empty());

    let json = serde_json::to_value(&res).unwrap();
    assert_eq!(json["query"], "нокс");
    assert!(json["total_hits"].as_u64().unwrap() >= 3);

    let best = &res.anchors[0];
    assert!(best.epsilon > 0.0);
    assert!(best.resonance >= best.epsilon - 0.01, "R={} < ε={}", best.resonance, best.epsilon);
    assert!(best.file.contains("chapter_36.md"));
    assert_eq!(best.token, "нокс");

    // сцена: глава / метрика / локация / субъекты / полный скоуп
    assert_eq!(best.scene.chapter, "Глава 36. Инертный");
    assert_eq!(best.scene.temporal_metric.as_deref(), Some("Метрика: Т-23"));
    assert_eq!(best.scene.location.as_deref(), Some("Локация: Разлом Каньона"));
    assert!(best.scene.subjects[0].contains("Соболь (Нокс)"));
    assert!(best.scene.enclosing_scope.contains("шунт"));
    assert!(best.scene.enclosing_scope.len() > 200, "сцена должна быть полной");

    // K-hop: график обязан быть непустым
    assert!(!best.k_hop_relations.is_empty(), "k_hop_relations пуст");
    let rels: Vec<Vec<String>> = best
        .k_hop_relations
        .iter()
        .map(|(s, p, o)| vec![s.clone(), p.clone(), o.clone()])
        .collect();
    // обе тройки из примера контракта спецификации §5
    assert!(
        rels.contains(&vec![
            "Нокс".to_string(),
            "вонзила_когти".to_string(),
            "Солнечное сплетение".to_string()
        ]),
        "нет спец-тройки 1: {rels:?}"
    );
    assert!(
        rels.contains(&vec![
            "Шунт".to_string(),
            "сбрасывает_тепло".to_string(),
            "1300°C".to_string()
        ]),
        "нет спец-тройки 2: {rels:?}"
    );
}

#[test]
fn negation_phrase_is_findable() {
    // Устранение Negation Blindness: RAG смешал бы эти фразы,
    // точный лексический поиск обязан их различать.
    let dir = write_fixture("chapter_36.md", CH36);
    let res = scan_path(dir.path(), "не должны", &EngineConfig::default());
    assert!(res.total_hits >= 1, "total_hits={}", res.total_hits);

    let res_neg = scan_path(dir.path(), "не должен", &EngineConfig::default());
    assert!(res_neg.total_hits >= 1);

    // противоположная по смыслу фраза находится отдельно
    let res_oblig = scan_path(dir.path(), "обязана", &EngineConfig::default());
    assert!(res_oblig.total_hits >= 1);

    // Exact Lexical Anchors: фразы «не обязана» в корпусе НЕТ —
    // векторный RAG с cosine > 0.94 ошибочно нашёл бы её здесь.
    let res_absent = scan_path(dir.path(), "не обязана", &EngineConfig::default());
    assert_eq!(res_absent.total_hits, 0, "ложное совпадение отсутствующей фразы");
}

#[test]
fn temporal_filter_works() {
    let dir = write_fixture("chapter_36.md", CH36);
    let mut cfg = EngineConfig::default();
    cfg.temporal_filter = Some("Т-23".to_string());
    let res = scan_path(dir.path(), "нокс", &cfg);
    assert!(res.total_hits >= 1);
    for a in &res.anchors {
        assert!(a.scene.temporal_metric.is_some());
    }

    let mut cfg2 = EngineConfig::default();
    cfg2.temporal_filter = Some("Т-99".to_string());
    let res2 = scan_path(dir.path(), "нокс", &cfg2);
    // все сцены с метрикой Т-23 отфильтрованы
    assert_eq!(res2.total_hits, 0, "total_hits={}", res2.total_hits);
}

#[test]
fn top_n_truncates_but_total_hits_is_honest() {
    let dir = write_fixture("chapter_36.md", CH36);
    let mut cfg = EngineConfig::default();
    cfg.top_n = 1;
    let res = scan_path(dir.path(), "нокс", &cfg);
    assert_eq!(res.anchors.len(), 1);
    assert!(res.total_hits > 1);
}

#[test]
fn deterministic_output() {
    let dir = write_fixture("chapter_36.md", CH36);
    let cfg = EngineConfig::default();
    let a = scan_path(dir.path(), "нокс", &cfg);
    let b = scan_path(dir.path(), "нокс", &cfg);
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
}

// ---------------------------------------------------------------------------
// Код: enclosing scope + call graph
// ---------------------------------------------------------------------------

#[test]
fn rust_code_scope_and_call_graph() {
    let dir = write_fixture("example.rs", CODE_RS);
    let cfg = EngineConfig::default();
    let res = scan_path(dir.path(), "compute", &cfg);

    assert!(res.total_hits >= 1);
    let scope_names: Vec<&str> = res
        .anchors
        .iter()
        .map(|a| a.scene.enclosing_scope.as_str())
        .collect();
    // каждое совпадение живёт в полном теле функции, а не в строке
    assert!(
        scope_names
            .iter()
            .any(|s| s.contains("fn process_data")),
        "нет скоупа process_data: {scope_names:?}"
    );

    // call graph: process_data -> compute
    let flat: Vec<String> = res
        .anchors
        .iter()
        .flat_map(|a| a.k_hop_relations.iter())
        .map(|(s, p, o)| format!("{s}|{p}|{o}"))
        .collect();
    assert!(
        flat.iter().any(|s| s.contains("вызывает|compute")),
        "нет call graph: {flat:?}"
    );
}

#[test]
fn python_scope_by_indent() {
    let dir = write_fixture("example.py", CODE_PY);
    let res = scan_path(dir.path(), "transform", &EngineConfig::default());
    assert!(res.total_hits >= 1);
    assert!(
        res.anchors
            .iter()
            .any(|a| a.scene.enclosing_scope.contains("def run")
                || a.scene.enclosing_scope.contains("def transform"))
    );
}

// ---------------------------------------------------------------------------
// PII
// ---------------------------------------------------------------------------

#[test]
fn pii_masked_by_default() {
    let dir = write_fixture("pii.txt", PII_TXT);
    let res = scan_path(dir.path(), "отчёт", &EngineConfig::default());
    assert!(res.total_hits >= 1);
    let scope = &res.anchors[0].scene.enclosing_scope;
    assert!(scope.contains("[EMAIL]"), "scope={scope}");
    assert!(!scope.contains("user@example.com"));
}

#[test]
fn pii_off_keeps_original() {
    let dir = write_fixture("pii.txt", PII_TXT);
    let mut cfg = EngineConfig::default();
    cfg.pii_mode = PiiMode::Off;
    let res = scan_path(dir.path(), "отчёт", &cfg);
    assert!(res.anchors[0].scene.enclosing_scope.contains("user@example.com"));
}

#[test]
fn secrets_are_masked() {
    let dir = write_fixture("pii.txt", PII_TXT);
    let res = scan_path(dir.path(), "скомпрометирован", &EngineConfig::default());
    let scope = &res.anchors[0].scene.enclosing_scope;
    assert!(scope.contains("[SECRET]"), "scope={scope}");
    assert!(!scope.contains("sk-abcdef"));
}

// ---------------------------------------------------------------------------
// Резонанс и статистика
// ---------------------------------------------------------------------------

#[test]
fn field_resonance_mode_produces_results() {
    let dir = write_fixture("chapter_36.md", CH36);
    let mut cfg = EngineConfig::default();
    cfg.resonance_mode = ResonanceMode::Field;
    let res = scan_path(dir.path(), "нокс", &cfg);
    assert!(res.total_hits >= 3);
    for a in &res.anchors {
        assert!(a.resonance > 0.0);
        assert!(a.epsilon > 0.0);
    }
}

#[test]
fn resonance_accumulates_along_document() {
    // IIR накапливает R по документу: при 3+ совпадениях в одном файле
    // разброс резонансов строго положителен (поздние хиты резонируют сильнее),
    // а монотонность самого фильтра покрыта unit-тестами iir_filter.
    let dir = write_fixture("chapter_36.md", CH36);
    let cfg = EngineConfig::default();
    let res = scan_path(dir.path(), "нокс", &cfg);
    assert!(res.total_hits >= 3);
    let rs: Vec<f64> = res.anchors.iter().map(|a| a.resonance).collect();
    let max = rs.iter().cloned().fold(f64::MIN, f64::max);
    let min = rs.iter().cloned().fold(f64::MAX, f64::min);
    assert!(max > min, "резонансы одинаковы: {rs:?}");
    // результат отсортирован по убыванию R
    for w in res.anchors.windows(2) {
        assert!(w[0].resonance >= w[1].resonance);
    }
}

#[test]
fn verbose_stats_reported() {
    let dir = write_fixture("chapter_36.md", CH36);
    let (_, stats) = scan_path_with_stats(dir.path(), "нокс", &EngineConfig::default());
    assert_eq!(stats.files_scanned, 1);
    assert_eq!(stats.files_with_hits, 1);
    assert!(stats.total_tokens > 50);
    assert!(stats.graph_nodes > 3);
    assert!(stats.graph_edges > 3);
}

#[test]
fn multi_file_corpus_and_ranking() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("chapter_36.md"),
        CH36,
    )
    .unwrap();
    fs::write(
        dir.path().join("chapter_37.md"),
        "# Глава 37. Отражение\n\n**Метрика: Т-24**\n\nНокс снова появилась на горизонте.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("notes.txt"),
        "техническая записка без искомого слова\n",
    )
    .unwrap();

    let (res, stats) = scan_path_with_stats(dir.path(), "нокс", &EngineConfig::default());
    assert_eq!(stats.files_scanned, 3);
    assert_eq!(stats.files_with_hits, 2);
    assert!(res.total_hits >= 4);
    // резонансы отсортированы по убыванию
    for w in res.anchors.windows(2) {
        assert!(w[0].resonance >= w[1].resonance);
    }
}

#[test]
fn local_stats_mode_works() {
    let dir = write_fixture("chapter_36.md", CH36);
    let mut cfg = EngineConfig::default();
    cfg.local_stats = true;
    let res = scan_path(dir.path(), "нокс", &cfg);
    assert!(res.total_hits >= 3);
    assert!(res.anchors[0].epsilon > 0.0);
}

// ---------------------------------------------------------------------------
// Техники из исходников: ripgrep (ignore-обход), GNU grep (предфильтр),
// super-z-skills (SQL-схема memory_graph)
// ---------------------------------------------------------------------------

#[test]
fn gitignore_is_respected() {
    // WalkBuilder из крейта ignore (ripgrep): .gitignore работает даже
    // вне git-репозитория при require_git(false)
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("keep.md"),
        "# Глава 1\n\nЗдесь есть искомое слово ВЕГА.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("skipme.md"),
        "# Глава 2\n\nА тут тоже ВЕГА, но файл в .gitignore.\n",
    )
    .unwrap();
    fs::write(dir.path().join(".gitignore"), "skipme.md\n").unwrap();

    let (_, stats) = scan_path_with_stats(dir.path(), "ВЕГА", &EngineConfig::default());
    assert_eq!(stats.files_scanned, 1, "файл из .gitignore должен быть пропущен");
    assert_eq!(stats.files_with_hits, 1);
}

#[test]
fn literal_prefilter_always_on_preserves_corpus_stats() {
    // Предфильтр (GNU grep kwset-техника) активен всегда, но статистика
    // считается по ВСЕМУ корпусу: отброшенные файлы участвуют в N_total.
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("hit.md"),
        "# Глава\n\nФункция process_data обрабатывает поток.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("miss.md"),
        "# Другая глава\n\nЗдесь нет искомого литерала совсем. Совсем другое содержимое.\n",
    )
    .unwrap();

    let (res, stats) = scan_path_with_stats(dir.path(), "process_data", &EngineConfig::default());
    assert_eq!(stats.files_scanned, 2);
    assert_eq!(stats.files_with_hits, 1);
    // оба файла в статистике корпуса
    assert!(stats.total_tokens >= 15, "total_tokens={}", stats.total_tokens);
    // якоря только из hit-файла
    assert!(res.total_hits >= 1);
    assert!(res.anchors.iter().all(|a| a.file.contains("hit.md")));
}

#[test]
fn ascii_query_case_insensitive() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("code.rs"),
        "fn ALPHA_FUNC() {\n    let x = 1;\n}\n",
    )
    .unwrap();
    let res = scan_path(dir.path(), "alpha_func", &EngineConfig::default());
    assert!(res.total_hits >= 1, "CI-предфильтр не нашёл вхождение");
}

#[test]
fn non_ascii_query_full_path() {
    // кириллица: предфильтр через lowercase-contains, полный путь
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("ch.md"),
        "# Глава\n\nНокс действует решительно.\n",
    )
    .unwrap();
    let res = scan_path(dir.path(), "нокс", &EngineConfig::default());
    assert!(res.total_hits >= 1);
}

#[test]
fn graph_triples_budget_is_enforced() {
    let dir = write_fixture("chapter_36.md", CH36);
    let mut cfg = EngineConfig::default();
    cfg.max_graph_triples = 3;
    let (_, stats) = scan_path_with_stats(dir.path(), "нокс", &cfg);
    assert!(stats.graph_edges <= 3, "edges={}", stats.graph_edges);
}

// ---------------------------------------------------------------------------
// Watcher: инкрементальный рескан по mtime/size
// ---------------------------------------------------------------------------

#[test]
fn watcher_detects_modification_and_stability() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("scene.md"), "# Глава\n\nНокс тут.\n").unwrap();

    let mut engine = poler_engine::Engine::new(EngineConfig::default(), true);
    let (res1, _) = engine.scan(dir.path(), "нокс");
    assert_eq!(res1.total_hits, 1);

    // без изменений: пустое событие, стабильный результат
    let (ev0, res0, _) = engine.rescan(dir.path(), "нокс");
    assert!(ev0.is_empty(), "{ev0:?}");
    assert_eq!(res0.total_hits, 1);

    // модификация файла
    std::thread::sleep(std::time::Duration::from_millis(60));
    fs::write(
        dir.path().join("scene.md"),
        "# Глава\n\nНокс тут. И снова Нокс.\n",
    )
    .unwrap();
    let (ev1, res1, _) = engine.rescan(dir.path(), "нокс");
    assert_eq!(ev1.changed.len(), 1, "{ev1:?}");
    assert_eq!(res1.total_hits, 2);

    // повторный рескан без изменений — результат сохранён
    let (ev2, res2, _) = engine.rescan(dir.path(), "нокс");
    assert!(ev2.is_empty());
    assert_eq!(res2.total_hits, 2);
}

#[test]
fn watcher_add_and_remove_files() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "# A\n\nНокс первый.\n").unwrap();

    let mut engine = poler_engine::Engine::new(EngineConfig::default(), true);
    let (res, _) = engine.scan(dir.path(), "нокс");
    assert_eq!(res.total_hits, 1);

    // добавление файла
    std::thread::sleep(std::time::Duration::from_millis(60));
    fs::write(dir.path().join("b.md"), "# B\n\nНокс второй.\n").unwrap();
    let (ev, res, _) = engine.rescan(dir.path(), "нокс");
    assert_eq!(ev.added.len(), 1, "{ev:?}");
    assert_eq!(res.total_hits, 2);

    // удаление файла
    std::thread::sleep(std::time::Duration::from_millis(60));
    fs::remove_file(dir.path().join("a.md")).unwrap();
    let (ev, res, _) = engine.rescan(dir.path(), "нокс");
    assert_eq!(ev.removed.len(), 1, "{ev:?}");
    assert_eq!(res.total_hits, 1);
    assert!(res.anchors.iter().all(|a| a.file.contains("b.md")));
}

#[test]
fn watcher_stats_survive_removal() {
    // удалённый файл исключается из глобальной статистики
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "# A\n\nНокс и ещё немного слов для статистики.\n").unwrap();
    fs::write(dir.path().join("b.md"), "# B\n\nНокс и другие слова тут.\n").unwrap();

    let mut engine = poler_engine::Engine::new(EngineConfig::default(), true);
    let (_, s1) = engine.scan(dir.path(), "нокс");
    assert_eq!(s1.files_scanned, 2);

    std::thread::sleep(std::time::Duration::from_millis(60));
    fs::remove_file(dir.path().join("b.md")).unwrap();
    let (_, _, s2) = engine.rescan(dir.path(), "нокс");
    assert_eq!(s2.files_scanned, 1);
    assert!(s2.total_tokens < s1.total_tokens, "токены удалённого файла должны уйти");
}

// ---------------------------------------------------------------------------
// AIDDE: таблица символов + impact-паспорт
// ---------------------------------------------------------------------------

#[test]
fn aidde_impact_passport_upstream_downstream() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("core.rs"),
        "pub fn core_fn(x: i32) -> i32 {\n    x + 1\n}\n",
    )
    .unwrap();
    fs::write(dir.path().join("mid.rs"), "pub fn mid() {\n    core_fn(1);\n}\n").unwrap();
    fs::write(dir.path().join("top.rs"), "pub fn top() {\n    mid();\n}\n").unwrap();

    let files = vec![
        dir.path().join("core.rs"),
        dir.path().join("mid.rs"),
        dir.path().join("top.rs"),
    ];
    let table = poler_engine::aidde::SymbolTable::build(&files, 1024 * 1024);
    let report = poler_engine::aidde::impact_analysis(&table, "core_fn", 3, 100)
        .expect("impact для core_fn");

    assert!(report.file.ends_with("core.rs"));
    // upstream: mid (прямые) и top (транзитивно) — ДОКАЗАНО call graph
    let callers: Vec<&str> = report
        .structural_relations
        .upstream_dependents
        .iter()
        .map(|d| d.caller.as_str())
        .collect();
    assert!(callers.contains(&"mid::mid"), "{callers:?}");
    assert!(callers.contains(&"top::top"), "{callers:?}");
    // 2 файла затронуты
    assert!(report.danger_level_if_modified.contains("MEDIUM"), "{}", report.danger_level_if_modified);

    // downstream от top: mid и core_fn
    let down = poler_engine::aidde::impact_analysis(&table, "top", 3, 100).unwrap();
    let callees: Vec<&str> = down
        .structural_relations
        .downstream_dependencies
        .iter()
        .map(|d| d.callee.as_str())
        .collect();
    assert!(callees.contains(&"mid"), "{callees:?}");
    assert!(callees.contains(&"core_fn"), "{callees:?}");
}

#[test]
fn aidde_triage_alerts_reported() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("sys.rs"),
        "pub fn alloc_buffer(n: usize) -> Vec<u8> {\n    unsafe { GLOBAL_GAUGE += 1 };\n    ALLOC_MUTEX.lock();\n    std::fs::write(\"/tmp/x\", b\"y\").ok();\n    vec![0; n]\n}\n",
    )
    .unwrap();
    let files = vec![dir.path().join("sys.rs")];
    let table = poler_engine::aidde::SymbolTable::build(&files, 1024 * 1024);
    let report = poler_engine::aidde::impact_analysis(&table, "alloc_buffer", 2, 100).unwrap();
    // v0.21: triage-сигналы — с маркером и категорией (не «сайд-эффекты»)
    let alerts = &report.heuristic_triage_alerts;
    assert!(alerts.iter().any(|a| a.marker == "unsafe" && a.category.label().contains("память")), "{alerts:?}");
    assert!(alerts.iter().any(|a| a.marker == ".lock()" && a.category.label().contains("конкурент")), "{alerts:?}");
    // эвристики не поднимают danger level: доказанных ЗАВИСИМЫХ нет → LOW
    assert_eq!(report.danger_level_if_modified, "LOW (прямых зависимых не найдено)");
    // upstream пуст (никто не вызывает alloc_buffer); downstream от её тела
    // есть (lock/write) — доказательства графом, отдельно от триажа
    assert!(report.structural_relations.upstream_dependents.is_empty());
    assert!(!report.structural_relations.downstream_dependencies.is_empty());
}

#[test]
fn aidde_python_cross_file() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("proc.py"), "def process(data):\n    return data\n").unwrap();
    fs::write(dir.path().join("run.py"), "def run():\n    return process(1)\n").unwrap();
    let files = vec![dir.path().join("proc.py"), dir.path().join("run.py")];
    let table = poler_engine::aidde::SymbolTable::build(&files, 1024 * 1024);
    let report = poler_engine::aidde::impact_analysis(&table, "process", 2, 100).unwrap();
    assert!(
        report
            .structural_relations
            .upstream_dependents
            .iter()
            .any(|d| d.caller == "run::run"),
        "{:?}",
        report.structural_relations.upstream_dependents
    );
}

#[test]
fn aidde_missing_symbol() {
    let table = poler_engine::aidde::SymbolTable::build(&[], 1024 * 1024);
    assert!(poler_engine::aidde::impact_analysis(&table, "nope", 2, 10).is_none());
}

#[test]
fn hidden_files_skipped_by_default() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("visible.md"), "# Глава\n\nВЕГА видима.\n").unwrap();
    fs::create_dir(dir.path().join(".secret")).unwrap();
    fs::write(dir.path().join(".secret/hidden.md"), "# Глава\n\nВЕГА скрыта.\n").unwrap();

    let (_, stats) = scan_path_with_stats(dir.path(), "ВЕГА", &EngineConfig::default());
    assert_eq!(stats.files_scanned, 1, "скрытые файлы пропускаются по умолчанию");

    let mut cfg = EngineConfig::default();
    cfg.include_hidden = true;
    let (_, stats2) = scan_path_with_stats(dir.path(), "ВЕГА", &cfg);
    assert_eq!(stats2.files_scanned, 2, "--hidden включает скрытые");
}

// ---------------------------------------------------------------------------
// Регрессия bug: гигантская строка-простыня с «Субъекты:» внутри (Eteryya)
// ---------------------------------------------------------------------------

#[test]
fn megabyte_line_with_subjects_marker_does_not_explode() {
    // Реальный кейс Eteryya: строка 1.7 МБ содержит «- Персонажи:» в глубине
    // текста с тысячами запятых. Прежний light_meta забирал остаток строки
    // после двоеточия как значение → 9315 «субъектов» → OOM.
    let dir = TempDir::new().unwrap();
    let mut giant = String::with_capacity(1_200_000);
    giant.push_str("# Дамп\n\n## Начало\n\n");
    giant.push_str("Обычный текст начала сцены и нокс. ");
    // мегабайтная простыня без переносов строк
    for i in 0..15_000 {
        giant.push_str(&format!("Сегмент {i} повествования, "),
        );
    }
    giant.push_str(" Персонажи: а, б, в, г, д, е, ж, з, и, к, л, м, н, о, п, р, с, т, ");
    for i in 0..20_000 {
        giant.push_str(&format!("имя{i}, "));
    }
    giant.push_str("конец гигантской строки.\n");
    giant.push_str("\nФинальный абзац с нокс.\n");
    fs::write(dir.path().join("giant.md"), &giant).unwrap();

    let (res, stats) = scan_path_with_stats(dir.path(), "нокс", &EngineConfig::default());
    assert!(res.total_hits >= 1);
    // граф не взорвался: разумное число рёбер (кап 256 троек на сцену)
    assert!(stats.graph_edges < 300, "edges={}", stats.graph_edges);
    // субъекты не распухли: карточка ищется только в первых 8 КБ сцены
    let best = &res.anchors[0];
    assert!(best.scene.subjects.len() <= 1);
    if let Some(s) = best.scene.subjects.first() {
        assert!(s.len() < 600, "subjects len={}", s.len());
    }
}

#[test]
fn subjects_value_capped_even_at_scene_start() {
    // «Субъекты:» в ПЕРВОЙ строке сцены — значение обрезается до 256 байт
    let dir = TempDir::new().unwrap();
    let mut head = String::from("# Глава\n\n**Субъекты: ");
    for i in 0..500 {
        head.push_str(&format!("персонаж{i}, "));
    }
    head.push_str("**\n\nНокс действует.\n");
    fs::write(dir.path().join("sub.md"), &head).unwrap();

    let res = scan_path(dir.path(), "нокс", &EngineConfig::default());
    assert!(res.total_hits >= 1);
    let s = &res.anchors[0].scene.subjects;
    assert!(!s.is_empty());
    assert!(s[0].len() < 400, "len={}", s[0].len());
}

#[test]
fn watcher_diff_mode_returns_only_new_anchors() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "# A\n\nНокс первый.\n").unwrap();
    fs::write(dir.path().join("b.md"), "# B\n\nДругой текст.\n").unwrap();

    let mut engine = poler_engine::Engine::new(EngineConfig::default(), true).with_diff(true);
    let (res1, _) = engine.scan(dir.path(), "нокс");
    assert_eq!(res1.total_hits, 1);
    assert!(res1.anchors.iter().all(|a| a.file.contains("a.md")));

    // без изменений — пустой дифф
    let (_, res0, _) = engine.rescan(dir.path(), "нокс");
    assert_eq!(res0.anchors.len(), 0, "дифф без изменений должен быть пуст");

    // b.md модифицируется: появляется Нокс
    std::thread::sleep(std::time::Duration::from_millis(60));
    fs::write(dir.path().join("b.md"), "# B\n\nТеперь и Нокс здесь.\n").unwrap();
    let (_, res2, _) = engine.rescan(dir.path(), "нокс");
    // только новый якорь b.md; a.md не повторяется
    assert_eq!(res2.anchors.len(), 1, "{:?}", res2.anchors);
    assert!(res2.anchors[0].file.contains("b.md"));

    // повторный rescan без изменений — снова пусто (ключи запомнились)
    let (_, res3, _) = engine.rescan(dir.path(), "нокс");
    assert_eq!(res3.anchors.len(), 0);
}

#[test]
fn watcher_diff_mode_new_file() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "# A\n\nНокс.\n").unwrap();
    let mut engine = poler_engine::Engine::new(EngineConfig::default(), true).with_diff(true);
    let _ = engine.scan(dir.path(), "нокс");

    std::thread::sleep(std::time::Duration::from_millis(60));
    fs::write(dir.path().join("new.md"), "# N\n\nНокс новый.\n").unwrap();
    let (_, res, _) = engine.rescan(dir.path(), "нокс");
    assert_eq!(res.anchors.len(), 1);
    assert!(res.anchors[0].file.contains("new.md"));
}

// ---------------------------------------------------------------------------
// POLER[Ψ] и параллельные гиганты (v0.4)
// ---------------------------------------------------------------------------

#[test]
fn psi_mode_ranks_hits() {
    // POLER[Ψ]: каноническое уравнение внимания из POLER-Quantum
    let dir = write_fixture("chapter_36.md", CH36);
    let mut cfg = EngineConfig::default();
    cfg.resonance_mode = poler_engine::ResonanceMode::Psi;
    let res = scan_path(dir.path(), "нокс", &cfg);
    assert!(res.total_hits >= 3);
    for a in &res.anchors {
        // ψ-резонанс ограничен перцептивным пространством Ω = tanh ∈ (−1,1)
        assert!(a.resonance.abs() <= 1.0 + 1e-9, "R={}", a.resonance);
        assert!(a.epsilon > 0.0);
    }
    // сортировка по ψ сохраняется
    for w in res.anchors.windows(2) {
        assert!(w[0].resonance >= w[1].resonance);
    }
}

#[test]
fn giant_chunked_equals_sequential_results() {
    // Эквивалентность: гигантский файл, обработанный чанками параллельно,
    // даёт те же сцены/хиты, что и малый файл с тем же содержимым
    // (порог GIANT_FILE_BYTES = 8 МБ).
    let dir = TempDir::new().unwrap();
    let mut text = String::with_capacity(9 * 1024 * 1024);
    text.push_str("# Документ\n\n");
    // ~8.5 МБ текста с абзацами и несколькими вхождениями
    for i in 0..90_000 {
        text.push_str(&format!(
            "Абзац {i} обычного текста с разными словами и смыслами. Слово ещё. Конец.\n\n"
        ));
    }
    // вхождения в разных частях гиганта
    text.push_str("## Раздел с нокс\n\nЗдесь Нокс упоминается впервые.\n\n");
    for i in 0..90_000 {
        text.push_str(&format!(
            "Второй блок абзацев {i} после раздела. Другие слова. Тоже конец.\n\n"
        ));
    }
    text.push_str("## Финал\n\nФинальный абзац с Нокс.\n\n");
    fs::write(dir.path().join("giant.md"), &text).unwrap();
    assert!(text.len() >= 9 * 1024 * 1024, "размер {}", text.len());

    let res = scan_path(dir.path(), "нокс", &EngineConfig::default());
    assert!(res.total_hits >= 3, "total_hits={}", res.total_hits);
    // сцены найдены и содержат вхождения из разных частей гиганта
    let scopes: Vec<&str> = res
        .anchors
        .iter()
        .map(|a| a.scene.enclosing_scope.as_str())
        .collect();
    assert!(
        scopes.iter().any(|s| s.contains("Нокс упоминается")),
        "нет средней сцены"
    );
    assert!(
        scopes.iter().any(|s| s.contains("Финальный абзац")),
        "нет финальной сцены: первые 200 символов {:?}",
        scopes.first().map(|s| s.chars().take(200).collect::<String>())
    );
}

#[test]
fn psi_params_from_cli_defaults_match_poler_quantum() {
    // Значения по умолчанию — точно из POLER_Psi_v3.py
    let p = poler_engine::psi::PsiParams::default();
    assert_eq!(p.eta, 0.05);
    assert_eq!(p.gamma, 0.5);
    assert_eq!(p.rho, 0.9);
    assert_eq!(p.memory_depth, 8);
}

// ---------------------------------------------------------------------------
// Канонический POLER-цикл из P3_Engine (p3_poler.zig, Kotokvit)
// ---------------------------------------------------------------------------

#[test]
fn poler_mode_ranks_hits() {
    // p_new = p − η·Π_Λ(D·p + γ·J·p + ∇F) с CORDIC-ренормализацией
    let dir = write_fixture("chapter_36.md", CH36);
    let mut cfg = EngineConfig::default();
    cfg.resonance_mode = poler_engine::ResonanceMode::Poler;
    let res = scan_path(dir.path(), "нокс", &cfg);
    assert!(res.total_hits >= 3);
    for a in &res.anchors {
        // POLER-амплитуда ограничена CORDIC-нормализацией на S¹
        assert!(a.resonance.is_finite() && a.resonance >= 0.0, "R={}", a.resonance);
        assert!(a.epsilon > 0.0);
    }
    for w in res.anchors.windows(2) {
        assert!(w[0].resonance >= w[1].resonance);
    }
}

#[test]
fn poler_dissipator_distinguishes_sparse_and_dense_hits() {
    // Диссипатор D=LLᵀ — энтропийный горел: при серии наблюдений
    // внимание накапливается, при паузе — сгорает. Плотная серия
    // наблюдений даёт больший POLER-резонанс, чем разовая вспышка.
    let dir = TempDir::new().unwrap();
    let mut dense = String::from("# Плотная сцена\n\n");
    for i in 0..12 {
        dense.push_str(&format!("Абзац {i}: нокс упоминается здесь.\n\n"));
    }
    fs::write(dir.path().join("dense.md"), &dense).unwrap();

    let mut sparse = String::from("# Разреженная сцена\n\n");
    sparse.push_str("Долгое вступление без искомого слова. ");
    sparse.push_str("Много других слов и предложений для объёма текста. ");
    sparse.push_str("И только один раз — нокс.\n\n");
    fs::write(dir.path().join("sparse.md"), &sparse).unwrap();

    let mut cfg = EngineConfig::default();
    cfg.resonance_mode = poler_engine::ResonanceMode::Poler;
    cfg.top_n = 20;
    let res = scan_path(dir.path(), "нокс", &cfg);

    let dense_max = res
        .anchors
        .iter()
        .filter(|a| a.file.contains("dense"))
        .map(|a| a.resonance)
        .fold(0.0, f64::max);
    let sparse_max = res
        .anchors
        .iter()
        .filter(|a| a.file.contains("sparse"))
        .map(|a| a.resonance)
        .fold(0.0, f64::max);
    assert!(
        dense_max > sparse_max,
        "диссипатор не различает плотность: dense={dense_max} sparse={sparse_max}"
    );
}

#[test]
fn sqlite_impact_reuse_skips_rebuild() {
    use poler_engine::aidde::{impact_analysis_sqlite, SymbolStore};
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("a.rs"),
        "pub fn core_fn(x: i32) -> i32 {\n    x + 1\n}\n",
    )
    .unwrap();
    fs::write(dir.path().join("b.rs"), "pub fn mid() {\n    core_fn(1);\n}\n").unwrap();
    let files = vec![dir.path().join("a.rs"), dir.path().join("b.rs")];
    let db = dir.path().join("sym.db");

    // первая сборка
    let mut s1 = SymbolStore::open(&db).unwrap();
    s1.build(&files, 1024 * 1024).unwrap();
    let (d1, c1) = s1.stats();
    drop(s1);

    // Изменим исходники: reuse обязан проигнорировать содержимое
    fs::write(dir.path().join("b.rs"), "pub fn other() {\n    nothing();\n}\n").unwrap();

    // reuse: схема заполнена -> перестройки нет, данные прежние
    let (s2, has) = SymbolStore::open_existing(&db).unwrap();
    assert!(has, "reuse должен увидеть заполненную схему");
    let (d2, c2) = s2.stats();
    assert_eq!((d1, c1), (d2, c2), "reuse не должен перестраивать");
    // старые вызовы доступны
    assert!(!s2.calls_of_callee("core_fn").is_empty());
    drop(s2);

    // impact по reuse-базе работает
    let (s3, _) = SymbolStore::open_existing(&db).unwrap();
    assert!(impact_analysis_sqlite(&s3, "core_fn", 2, 100).is_some());
}

// ---------------------------------------------------------------------------
// v2.0 Compression (Приоритет 3): дифференциальные тесты плотности памяти
// ---------------------------------------------------------------------------

#[test]
fn epsilon_identical_through_compressed_vocab() {
    // Дифференциал: ε, посчитанная через HashMap-частоты, обязана
    // побитово совпадать с ε через FSST-арену (VocabArena + GlobalStats)
    // — иначе замена представления словаря меняет ранжирование.
    use poler_engine::compression::{GlobalStats, TermFreqs, VocabArena};
    use poler_engine::resonance::calculate_epsilon;

    let corpus = [
        "нокс вонзила когти в сплетение теней и растворилась в сумерках",
        "система не должна отключаться при отказе питания реактора",
        "runtime panic unwrap deprecated hack todo fixme unsafe блок",
        "вектор резонанса протокола системы индексируется токенами запроса",
        "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda",
    ];
    let mut old: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut arena = VocabArena::new();
    let mut counts: Vec<u32> = Vec::new();
    let mut total = 0usize;
    for doc in corpus {
        for tok in doc.split_whitespace() {
            let t = tok.to_lowercase();
            *old.entry(t.clone()).or_insert(0) += 1;
            let id = arena.intern(&t) as usize;
            if id >= counts.len() {
                counts.resize(id + 1, 0);
            }
            counts[id] += 1;
            total += 1;
        }
    }
    arena.ensure_compact();
    assert!(arena.is_compact());
    let g = GlobalStats {
        vocab: &arena,
        counts: &counts,
    };

    let queries: Vec<Vec<String>> = vec![
        vec!["нокс".into()],
        vec!["не".into(), "должна".into()],
        vec!["runtime".into()],
        vec!["вектор".into(), "резонанса".into()],
        vec!["отсутствует".into()],
    ];
    for text in corpus {
        let window: Vec<&str> = text.split_whitespace().collect();
        let lowered: Vec<String> = window.iter().map(|w| w.to_lowercase()).collect();
        for q in &queries {
            let e_old = calculate_epsilon(&lowered.iter().map(|s| s.as_str()).collect::<Vec<_>>(), q, &old, total, 1.0, 0.0);
            let e_new = calculate_epsilon(&lowered.iter().map(|s| s.as_str()).collect::<Vec<_>>(), q, &g, total, 1.0, 0.0);
            assert_eq!(e_old, e_new, "ε разошлась на {text:?} q={q:?}");
        }
    }
    // и частоты напрямую совпадают
    for (term, freq) in &old {
        assert_eq!(g.freq(term).unwrap() as usize, *freq, "терм {term:?}");
    }
    assert_eq!(g.freq("нет_такого_терма"), None);
}

#[test]
fn watcher_state_survives_compaction_cycle() {
    // Полный цикл watcher: scan → rescan (компактная арена + ID-пары) →
    // результаты обязаны совпадать со свежим сканом тех же файлов.
    let dir = TempDir::new().unwrap();
    for i in 0..4 {
        fs::write(
            dir.path().join(format!("f{i}.md")),
            format!("# Глава {i}\n\nНокс и система резонанса. Повторение: нокс, нокс.\n"),
        )
        .unwrap();
    }
    let mut engine = poler_engine::Engine::new(EngineConfig::default(), true);
    let (res1, _) = engine.scan(dir.path(), "нокс");
    assert!(res1.total_hits >= 12);

    // «свежий» движок без watcher-состояния — эталон
    let mut fresh = poler_engine::Engine::new(EngineConfig::default(), false);
    let (res_fresh, _) = fresh.scan(dir.path(), "нокс");
    assert_eq!(res1.total_hits, res_fresh.total_hits);

    // rescan без изменений: статистика из сжатого состояния (ID-пары)
    let (ev, res2, _) = engine.rescan(dir.path(), "нокс");
    assert!(ev.is_empty(), "{ev:?}");
    assert_eq!(res2.total_hits, res_fresh.total_hits);
    // якоря идентичны свежему прогону (ε/R посчитаны по тем же частотам)
    let a1: Vec<(String, f64, f64)> = res_fresh
        .anchors
        .iter()
        .map(|a| (a.file.clone(), a.epsilon, a.resonance))
        .collect();
    let a2: Vec<(String, f64, f64)> = res2
        .anchors
        .iter()
        .map(|a| (a.file.clone(), a.epsilon, a.resonance))
        .collect();
    assert_eq!(a1, a2, "якоря rescan должны совпадать со свежим сканом");

    // изменение файла: вычитание старого словаря + новый pass 1/2
    std::thread::sleep(std::time::Duration::from_millis(60));
    fs::write(dir.path().join("f0.md"), "# Глава 0\n\nПолностью новый текст без искомого слова.\n").unwrap();
    let (ev3, res3, _) = engine.rescan(dir.path(), "нокс");
    assert_eq!(ev3.changed.len(), 1, "{ev3:?}");
    assert!(res3.total_hits < res_fresh.total_hits);
    let mut fresh2 = poler_engine::Engine::new(EngineConfig::default(), false);
    let (res_fresh2, _) = fresh2.scan(dir.path(), "нокс");
    assert_eq!(res3.total_hits, res_fresh2.total_hits);
}

#[test]
fn web_docstore_compression_roundtrip() {
    // zstd doc store: 40 страниц → словарь обучается → поиск находит
    // страницы и читает их сжатые тексты; старые строки читаются тоже.
    use poler_engine::web::index::{WebIndex, WebDoc};

    let dir = TempDir::new().unwrap();
    let db = dir.path().join("web.db");
    let mut ix = WebIndex::open(&db).unwrap();
    let page = |n: usize| WebDoc {
        url: format!("https://site.io/{n}"),
        title: format!("Страница {n}"),
        text: format!(
            "Общая шапка сайта: навигация, поиск, обратная связь. \
             Уникальный контент номер {n} про нокс и систему резонанса. \
             Служебные обороты: содержание, версия для печати, редактировать."
        ),
        lang: "ru".into(),
        meta_description: String::new(),
        content_hash: format!("hash{n}"),
        links: vec![],
    };
    for n in 0..40 {
        ix.upsert_page(&page(n)).unwrap();
    }
    // словарь обучен (>= 16 страниц), мета-запись существует
    assert!(ix.conn()
        .query_row("SELECT COUNT(*) FROM meta WHERE k = 'zstd_dict'", [],
        |r| r.get::<_, i64>(0)).unwrap() > 0);
    // сжатые тексты реально в text_c
    let compressed: i64 = ix.conn()
        .query_row("SELECT COUNT(*) FROM pages WHERE text_c IS NOT NULL AND length(text_c) > 0", [],
        |r| r.get(0)).unwrap();
    assert!(compressed >= 39, "почти все строки обязаны быть сжаты: {compressed}");
    let db_bytes = std::fs::metadata(&db).map(|m| m.len()).unwrap_or(0);

    // поиск по сжатому doc store
    let hits = ix.search("нокс резонанса", 10).unwrap();
    assert!(!hits.is_empty());
    assert!(hits[0].snippet.contains("нокс") || hits[0].snippet.contains("контент"));

    // переоткрытие: словарь из meta, поиск работает
    drop(ix);
    let mut ix2 = WebIndex::open(&db).unwrap();
    let hits2 = ix2.search("уникальный контент", 10).unwrap();
    assert!(!hits2.is_empty());

    // старая строка (плоский текст + термы, как в БД до v2.0) читается
    // наравне со сжатыми
    ix2.conn()
        .execute(
            "INSERT INTO pages(url, title, lang, meta_desc, text, content_hash, simhash, doclen, fetched_at)
             VALUES('https://old.io/1', 'Old', 'ru', '', 'legacy plain text про нокс', 'h1', 0, 6, 1)",
            [],
        )
        .unwrap();
    ix2.conn()
        .execute(
            "INSERT INTO terms(term, page_id, tf, title_tf, positions) VALUES
             ('legacy', (SELECT id FROM pages WHERE url='https://old.io/1'), 1, 0, NULL),
             ('plain', (SELECT id FROM pages WHERE url='https://old.io/1'), 1, 0, NULL)",
            [],
        )
        .unwrap();
    let hits3 = ix2.search("legacy plain", 10).unwrap();
    assert_eq!(hits3.len(), 1);
    assert!(db_bytes > 0);
}

// ---------------------------------------------------------------------------
// v0.82.0: intent-ранжирование код ↔ проза + сигнатурный слой
// (стресс-кейс владельца на gin-gonic: docs/doc.md R=55000 вытеснял
//  gin.go R=4240 из топ-10 запроса `func Default`)
// ---------------------------------------------------------------------------

use poler_engine::{render_simple, IntentMode};

/// Ключевой Go-файл: настоящая точка определения.
const GIN_GO: &str = r#"package gin

import (
        "net/http"
)

// Default returns an Engine instance with the Logger
// and Recovery middleware already attached.
func Default() *Engine {
        engine := New()
        engine.Use(Logger(), Recovery())
        return engine
}

func New() *Engine {
        debugPrintWARNINGNew()
        engine := &Engine{
                RouterGroup: RouterGroup{
                        Handlers: nil,
                        basePath: "/",
                        root:     true,
                },
        }
        return engine
}
"#;

/// Шумовой файл: struct-теги с `default=1` + func поблизости
/// (proximity-AND ловит их на запрос `func Default()`).
const BINDING_GO: &str = r#"package binding

type formMapping struct {
        formatter formatter
}

type exampleStruct struct {
        Name    string `form:"name" binding:"required"`
        Age     int    `form:",default=1"`
        Enabled bool   `form:"enabled,default=true"`
        Slot    int    `form:"slot,default=0"`
}

func mapForm(ptr any, tag string) error {
        return decode(ptr, tag)
}
"#;

/// Документация-обидчик: «default» сотни раз, IIR-резонанс раздувается.
fn gin_doc_md() -> String {
    let mut out = String::from("# gin documentation\n\n");
    for i in 0..40 {
        out.push_str(&format!(
            "## Section {i}\n\nThe default router uses the default middleware \
with default configuration. By default the default engine applies default \
logging and default recovery. Default values fall back to the default \
handler when the default flag is unset. The func default convention is \
documented here.\n\n"
        ));
    }
    out
}

fn gin_corpus() -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    let docs = dir.path().join("docs");
    fs::create_dir_all(&docs).unwrap();
    fs::write(dir.path().join("gin.go"), GIN_GO).unwrap();
    fs::write(dir.path().join("form_mapping.go"), BINDING_GO).unwrap();
    fs::write(docs.join("doc.md"), gin_doc_md()).unwrap();
    dir
}

fn def_line() -> usize {
    GIN_GO.lines().position(|l| l.contains("func Default()")).unwrap() + 1
}

#[test]
fn intent_code_query_demotes_docs() {
    // Кейс владельца: `func Default` без ручного --extensions go.
    // Документация (тир 1) обязана опуститься НИЖЕ кода (тир 0)
    // независимо от магнитуды IIR-резонанса.
    let dir = gin_corpus();
    let res = scan_path(dir.path(), "func Default", &EngineConfig::default());
    assert!(res.total_hits > 0, "хиты должны быть");

    let pos_go = res.anchors.iter().position(|a| a.file.ends_with("gin.go"));
    let pos_md = res
        .anchors
        .iter()
        .position(|a| a.file.ends_with("doc.md"));
    assert!(pos_go.is_some(), "gin.go должен быть в топ-N");
    assert!(pos_md.is_some(), "doc.md должен быть в топ-N (тир 1, но видим)");
    assert!(
        pos_go.unwrap() < pos_md.unwrap(),
        "код обязан стоять выше документации: go={:?} md={:?}",
        pos_go, pos_md
    );
}

#[test]
fn intent_off_restores_pure_resonance_order() {
    // --intent off: тиры выключены, порядок — чисто по R (контракт v0.81).
    let dir = gin_corpus();
    let mut cfg = EngineConfig::default();
    cfg.intent_mode = IntentMode::Off;
    let res = scan_path(dir.path(), "func Default", &cfg);
    // порядок монотонен по резонансу
    for w in res.anchors.windows(2) {
        assert!(
            w[0].resonance >= w[1].resonance,
            "R должен убывать: {} -> {}",
            w[0].resonance,
            w[1].resonance
        );
    }
}

#[test]
fn intent_prose_query_demotes_code() {
    // Симметрия: прозаический запрос → документация выше кода.
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    fs::write(
        dir.path().join("docs").join("readme_ru.md"),
        "# Резонанс\n\nКак работает резонанс: поле накапливает энергию. \
Как работает резонанс в движке — рассказано ниже.\n\nКак работает \
резонанс памяти: IIR-фильтр затухает.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("engine_ru.go"),
        "package engine\n\n// резонанс: как это работает в цикле\nfunc loop() {\n\treturn\n}\n",
    )
    .unwrap();

    let res = scan_path(dir.path(), "как работает резонанс", &EngineConfig::default());
    assert!(res.total_hits > 0);
    let first = &res.anchors[0];
    assert!(
        first.file.ends_with(".md"),
        "проза-запрос: топ-1 обязан быть документацией, а не кодом: {}",
        first.file
    );
}

#[test]
fn signature_query_puts_exact_definition_first() {
    // `func Default()` — прикреплённая скобка включает сигнатурный слой:
    // точка определения (тир 0) выше proximity-шума struct-тегов (тир 2).
    let dir = gin_corpus();
    let res = scan_path(dir.path(), "func Default()", &EngineConfig::default());
    assert!(res.total_hits > 0);

    let top = &res.anchors[0];
    assert!(
        top.file.ends_with("gin.go"),
        "топ-1 обязан быть точкой определения, а не тегом/доком: {}",
        top.file
    );
    // v0.82.0: номер строки хита + относительный путь
    assert_eq!(top.line, def_line(), "строка определения");
    assert_eq!(top.rel_file.as_deref(), Some("gin.go"));
    assert!(top.scene.enclosing_scope.contains("func Default() *Engine"));
}

#[test]
fn signature_keeps_token_hits_when_case_mismatches() {
    // `func default()` (нижний регистр): сигнатуры в gin.go НЕТ (Go
    // экспортирует с заглавной) — но токены по-прежнему находятся,
    // сигнатурный слой не теряет хиты (тир 2 у всех, порядок по R).
    let dir = gin_corpus();
    let res = scan_path(dir.path(), "func default()", &EngineConfig::default());
    assert!(res.total_hits > 0, "токенные хиты не должны теряться");
    // рендер не падает на новых полях
    let s = render_simple(&res);
    assert!(s.contains("gin.go:") || s.contains("doc.md:") || s.contains("form_mapping.go:"));
}

#[test]
fn anchor_line_and_rel_path_in_subdirs() {
    // Глубокая вложенность: rel_file от корня сканирования, line 1-based.
    let dir = gin_corpus();
    let res = scan_path(dir.path(), "func Default()", &EngineConfig::default());
    for a in &res.anchors {
        // rel_file всегда без абсолютного префикса директории
        if let Some(rel) = &a.rel_file {
            assert!(!rel.starts_with('/'), "rel_file абсолютен: {rel}");
        }
    }
    let doc_anchor = res
        .anchors
        .iter()
        .find(|a| a.file.ends_with("doc.md"))
        .expect("doc.md в результатах");
    assert_eq!(
        doc_anchor.rel_file.as_deref(),
        Some("docs/doc.md"),
        "относительный путь от корня"
    );
    assert!(doc_anchor.line >= 1);
}

#[test]
fn single_file_target_uses_file_name() {
    // Цель — отдельный файл: rel_file = имя файла, не полный путь.
    let dir = write_fixture("gin.go", GIN_GO);
    let target = dir.path().join("gin.go");
    let res = scan_path(&target, "func Default()", &EngineConfig::default());
    let top = &res.anchors[0];
    assert_eq!(top.rel_file.as_deref(), Some("gin.go"));
    assert_eq!(top.line, def_line());
}

// ---------------------------------------------------------------------------
// v0.83.0: мягкий fallback скобочного запроса + превью по строке хита +
// дефолтные расширения низкоуровневых языков (стресс-тест владельца на
// wireshark 1.2 ГБ и ядре poler-os на Zig)
// ---------------------------------------------------------------------------

/// Wireshark-морфология: точного токена `proto_register_field` нет нигде,
/// есть только семейство продолжений (`proto_register_field_array` — один
/// токен, разрыв только по не-alnum).
const WS_PROTO_C: &str = r#"#include "packet.h"

WS_DLL_PUBLIC int proto_register_field_array(void) {
    proto_register_field_array();
    return 1;
}
"#;

/// Документация с тем же префикс-семейством в прозе: fallback находит и её,
/// но intent (код-запрос) обязан держать исходник выше.
const WS_DOC_MD: &str = "# Wireshark internals\n\nThe proto_register_field_array helper\nregisters many fields at once. See also\nproto_register_field_init notes below.\n";

#[test]
fn signature_soft_fallback_finds_prefix_family() {
    // Кейс владельца: `proto_register_field(` → exit code 1 (0 результатов),
    // хотя `proto_register_field_array(...)` в коде точно есть. v0.83.0:
    // автоматический мягкий fallback на префикс идентификатора.
    let dir = TempDir::new().unwrap();
    let docs = dir.path().join("docs");
    fs::create_dir_all(&docs).unwrap();
    fs::write(dir.path().join("wslua_proto.c"), WS_PROTO_C).unwrap();
    fs::write(docs.join("internals.md"), WS_DOC_MD).unwrap();

    let res = scan_path(dir.path(), "proto_register_field(", &EngineConfig::default());
    assert!(
        res.total_hits > 0,
        "мягкий fallback обязан находить семейство идентификаторов"
    );
    assert!(res.soft_fallback, "флаг soft_fallback не поднят");
    // intent-тиры в fallback: код (тир 0) выше документации (тир 1)
    let top = &res.anchors[0];
    assert!(
        top.file.ends_with("wslua_proto.c"),
        "топ-1 обязан быть исходником, а не докой: {}",
        top.file
    );
    // превью топ-якоря начинается СО СТРОКИ ХИТА (определение или
    // call-site семейства — оба валидны, порядок решает IIR-резонанс)
    let top_preview = top.preview.as_deref().expect("превью материализовано");
    let first_line = top_preview.lines().next().unwrap_or("");
    assert!(
        first_line.contains("proto_register_field_array"),
        "строка хита не первая в превью: {top_preview:?}"
    );
    // определение семейства в топ-N: сигнатура целиком со строки хита
    let def_line = WS_PROTO_C
        .lines()
        .position(|l| l.contains("proto_register_field_array"))
        .unwrap()
        + 1;
    let def = res
        .anchors
        .iter()
        .find(|a| a.line == def_line)
        .expect("определение семейства в топ-N");
    let def_preview = def.preview.as_deref().unwrap();
    assert!(
        def_preview.starts_with("WS_DLL_PUBLIC int proto_register_field_array(void)"),
        "превью определения не со строки хита: {def_preview:?}"
    );
    // рендер не теряет сигнатуру
    let s = render_simple(&res);
    assert!(
        s.contains("proto_register_field_array(void)"),
        "simple не содержит сигнатуру семейства: {s}"
    );
}

#[test]
fn signature_exact_hits_disable_fallback() {
    // Точные токенные хиты есть (`func Default()` в gin-корпусе) —
    // fallback не должен включаться и подменять семантику.
    let dir = gin_corpus();
    let res = scan_path(dir.path(), "func Default()", &EngineConfig::default());
    assert!(res.total_hits > 0);
    assert!(!res.soft_fallback, "fallback не должен срабатывать");
    assert!(res.anchors[0].file.ends_with("gin.go"));
}

#[test]
fn preview_aligned_to_hit_line_not_scope_head() {
    // Wireshark-кейс: хит на forward-объявлении ВНЕ функции — скоуп
    // вырождается в целый файл (`#ifdef…extern "C" {…`), а превью
    // обязано начинаться СО СТРОКИ ХИТА, а не с головы скоупа.
    let packet_h = "#ifdef __cplusplus\nextern \"C\" {\n#endif /* __cplusplus */\n\nstruct wtap_block;\n\nWS_DLL_PUBLIC void proto_register_field_array(void) {\n    struct wtap_block *blk = 0;\n    (void)blk;\n}\n";
    let dir = write_fixture("packet.h", packet_h);
    let res = scan_path(dir.path(), "wtap_block", &EngineConfig::default());
    assert!(res.total_hits > 0);

    // якорь на строке forward-объявления (вне функции)
    let decl = res
        .anchors
        .iter()
        .find(|a| a.line == 5)
        .expect("хит на forward-объявлении (строка 5)");
    let preview = decl.preview.as_deref().expect("превью есть");
    assert!(
        preview.starts_with("struct wtap_block;"),
        "превью обязано начинаться со строки хита, а не с головы скоупа: {preview:?}"
    );
    assert!(
        !preview.starts_with("#ifdef") && !preview.starts_with("extern"),
        "превью захватил закрывающий контекст перед объявлением: {preview:?}"
    );
    // каждый якорь: первая строка превью содержит токен хита
    for a in &res.anchors {
        let first = a.preview.as_deref().unwrap_or("").lines().next().unwrap_or("");
        assert!(
            first.contains("wtap_block"),
            "строка хита не первая в превью {}: {first:?}",
            a.rel_file.as_deref().unwrap_or(&a.file)
        );
    }
}

#[test]
fn default_extensions_cover_zig_asm_lua() {
    // Кейс poler-os: ядро на Zig + ASM-рассечение — без ручного
    // `--extensions zig,rs,c,h,py,md` файлы отсекались на обходе дерева.
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("kernel.zig"), "pub fn probe_9f3a() void {}\n").unwrap();
    fs::write(dir.path().join("boot.s"), "# probe_9f3a cold boot\n").unwrap();
    fs::write(dir.path().join("init.S"), "// probe_9f3a upper\n").unwrap();
    fs::write(dir.path().join("loader.asm"), "; probe_9f3a real mode\n").unwrap();
    fs::write(dir.path().join("tick.lua"), "-- probe_9f3a callback\n").unwrap();

    let res = scan_path(dir.path(), "probe_9f3a", &EngineConfig::default());
    let files: Vec<String> = res
        .anchors
        .iter()
        .filter_map(|a| a.rel_file.clone())
        .collect();
    assert_eq!(
        res.total_hits,
        5,
        "все 5 расширений должны находиться по умолчанию: {files:?}"
    );
    for ext in ["zig", "s", "S", "asm", "lua"] {
        assert!(
            files.iter().any(|f| f.ends_with(&format!(".{ext}"))),
            "нет .{ext} в выдаче: {files:?}"
        );
    }
}

#[test]
fn default_extensions_cover_modern_cpp() {
    // Грабля 42 (Blender/LLVM/Google): определение `struct BMVert` в
    // `bmesh_class.hh` не находилось без ручного `--extensions hh`;
    // современный C++ держит заголовки в .hh/.hxx, inline — в .inc/.inl.
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("bmesh_class.hh"),
        "struct BMVert {\n  struct BMHeader head;\n  float co[3];\n};\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("dna_gen.hxx"),
        "// struct BMVert generated by DNA\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("bmesh_ops.cxx"),
        "bool bm_vert_any(const struct BMVert *v) {\n  return v->head.index >= 0;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("inline_ops.inc"),
        "inline void bm_vert_clear(struct BMVert *v) {\n  v->head.index = -1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("vec_inl.inl"),
        "inline float len3(const struct BMVert *v) {\n  return v->co[0];\n}\n",
    )
    .unwrap();
    // доки с теми же упоминаниями — код-интент должен держать их ниже
    fs::write(
        dir.path().join("notes.md"),
        "Docs mention struct BMVert in prose.\n",
    )
    .unwrap();

    let res = scan_path(dir.path(), "struct BMVert", &EngineConfig::default());
    let ranked: Vec<String> = res
        .anchors
        .iter()
        .map(|a| a.rel_file.clone().unwrap_or_else(|| a.file.clone()))
        .collect();

    // все 5 новых расширений сканируются по умолчанию
    for ext in ["hh", "hxx", "cxx", "inc", "inl"] {
        assert!(
            ranked.iter().any(|f| f.ends_with(&format!(".{ext}"))),
            "нет .{ext} в выдаче: {ranked:?}"
        );
    }

    // код-интент (`struct` — ключевое слово): определение в .hh (тир 0)
    // выше .md-упоминаний (тир 1)
    let hh_pos = ranked.iter().position(|f| f.ends_with("bmesh_class.hh"));
    let md_pos = ranked.iter().position(|f| f.ends_with("notes.md"));
    assert!(hh_pos.is_some(), "определение BMVert не найдено: {ranked:?}");
    assert!(md_pos.is_some(), "доки не найдены: {ranked:?}");
    assert!(
        hh_pos.unwrap() < md_pos.unwrap(),
        "код-интент: .hh-определение должно стоять выше .md: {ranked:?}"
    );

    // .cxx теперь Brace-язык: enclosing_scope — функция целиком
    let cxx = res
        .anchors
        .iter()
        .find(|a| a.rel_file.as_deref().is_some_and(|f| f.ends_with("bmesh_ops.cxx")))
        .expect("нет якоря .cxx");
    assert!(
        cxx.scene.enclosing_scope.contains("bool bm_vert_any"),
        "Brace-скоуп .cxx должен содержать сигнатуру: {}",
        cxx.scene.enclosing_scope
    );
    assert!(
        cxx.scene.enclosing_scope.contains("return v->head.index"),
        "Brace-скоуп .cxx должен содержать тело функции: {}",
        cxx.scene.enclosing_scope
    );
}

#[test]
fn soft_fallback_flag_serialized_only_when_true() {
    // JSON-контракт: soft_fallback появляется только в fallback-прогоне
    // (обратная совместимость с потребителями v0.82.0).
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("wslua_proto.c"), WS_PROTO_C).unwrap();
    let res = scan_path(dir.path(), "proto_register_field(", &EngineConfig::default());
    let v = serde_json::to_value(&res).unwrap();
    assert_eq!(v["soft_fallback"], serde_json::json!(true));

    let res2 = scan_path(dir.path(), "proto_register_field_array", &EngineConfig::default());
    let v2 = serde_json::to_value(&res2).unwrap();
    assert!(
        v2.get("soft_fallback").is_none(),
        "флаг не должен сериализоваться при обычном поиске"
    );
}

// ---------------------------------------------------------------------------
// v0.85.0: макро-атрибуты между class/struct и именем типа (ГРАБЛЯ 43,
// стресс-тест владельца на LLVM: `class LLVM_ABI Function :` невидим для
// запроса `class Function :`, выдачу забивают forward-декларации)
// ---------------------------------------------------------------------------

/// LLVM-морфология: определение за макросом экспорта видимости.
const LLVM_FN_H: &str = r#"#pragma once
namespace llvm {
class LLVM_ABI Function : public GlobalObject {
public:
  Function(Type *Ty, LinkageTypes Linkage);
  const BasicBlock &getEntryBlock() const;
};
} // end namespace llvm
"#;

/// Forward-декларации — они и забивали выдачу в v0.84.0.
const LLVM_FWD_H: &str = r#"#pragma once
namespace llvm {
class Function;
class Module;
class BasicBlock;
class GlobalObject;
friend class Function;
void printFunctionName(const Function *F);
} // end namespace llvm
"#;

#[test]
fn type_decl_gap_fallback_finds_macro_definition() {
    // Кейс владельца (LLVM): `class Function :` выдал 10 forward-деклараций,
    // определение спрятано за LLVM_ABI. v0.85.0: гэп-fallback поднимает
    // определение на [1], forward-декларации больше не вытесняют его.
    let dir = TempDir::new().unwrap();
    let inc = dir.path().join("include/llvm/IR");
    fs::create_dir_all(&inc).unwrap();
    fs::write(inc.join("Function.h"), LLVM_FN_H).unwrap();
    fs::write(inc.join("FunctionFwd.h"), LLVM_FWD_H).unwrap();

    let res = scan_path(dir.path(), "class Function :", &EngineConfig::default());
    assert!(
        res.total_hits > 0,
        "гэп-fallback обязан находить определение за макросом"
    );
    assert!(res.soft_fallback, "флаг soft_fallback не поднят");
    assert_eq!(
        res.soft_fallback_kind.as_deref(),
        Some("type_decl_gap"),
        "разновидность fallback обязана быть type_decl_gap"
    );
    // топ-1 — определение (Function.h), не forward-декларации
    let top = &res.anchors[0];
    assert!(
        top.file.ends_with("Function.h"),
        "топ-1 обязан быть определением, не forward: {}",
        top.file
    );
    assert_eq!(top.line, 3, "строка определения class LLVM_ABI Function");
    let preview = top.preview.as_deref().expect("превью материализовано");
    let first = preview.lines().next().unwrap_or("");
    assert!(
        first.contains("class LLVM_ABI Function"),
        "превью обязано начинаться со строки определения: {first:?}"
    );
    // simple-рендер несёт определение
    let s = render_simple(&res);
    assert!(
        s.contains("class LLVM_ABI Function"),
        "simple не содержит определение: {s}"
    );
    // JSON-контракт: разновидность сериализуется только в fallback
    let v = serde_json::to_value(&res).unwrap();
    assert_eq!(v["soft_fallback_kind"], serde_json::json!("type_decl_gap"));
}

#[test]
fn type_decl_exact_definition_disables_gap_fallback() {
    // РЕГРЕССИЯ Blender (v0.84.0): `struct BMVert {` — определение без
    // макроса, def-сигнал точного хита гасит гэп-fallback: точный путь.
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("bmesh_class.hh"),
        "#pragma once\nstruct BMVert {\n  struct BMHeader head;\n  int index;\n};\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("fwd.hh"),
        "#pragma once\nstruct BMVert;\nstruct BMEdge;\n",
    )
    .unwrap();

    let res = scan_path(dir.path(), "struct BMVert {", &EngineConfig::default());
    assert!(res.total_hits > 0);
    assert!(
        !res.soft_fallback,
        "точное определение гасит гэп-fallback"
    );
    assert!(res.soft_fallback_kind.is_none());
    assert!(
        res.anchors[0].file.ends_with("bmesh_class.hh"),
        "топ-1 — точное определение: {}",
        res.anchors[0].file
    );
}

#[test]
fn type_decl_explicit_macro_query_stays_exact() {
    // Юзер сам вписал макрос — 3 слова, TypeDeclQuery не парсится,
    // точная строгая фраза без всякого fallback (поведение v0.84.0).
    let dir = TempDir::new().unwrap();
    let inc = dir.path().join("include/llvm/IR");
    fs::create_dir_all(&inc).unwrap();
    fs::write(inc.join("Function.h"), LLVM_FN_H).unwrap();
    fs::write(inc.join("FunctionFwd.h"), LLVM_FWD_H).unwrap();

    let res = scan_path(dir.path(), "class LLVM_ABI Function", &EngineConfig::default());
    assert!(res.total_hits > 0, "точная фраза обязана находить определение");
    assert!(!res.soft_fallback, "точный путь — без fallback");
    assert!(res.soft_fallback_kind.is_none());
    assert!(res.anchors[0].file.ends_with("Function.h"));
}

#[test]
fn type_decl_gap_only_decls_honest_fallback() {
    // Гэп-хиты есть, но все — декларации (`class MY_EXPORT Widget;`):
    // fallback честно возвращает их (детерминированно), не выдумывая
    // определение, и помечает разновидность.
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("widget.h"),
        "#pragma once\nclass MY_EXPORT Widget;\nvoid make(Widget *w);\n",
    )
    .unwrap();

    let res = scan_path(dir.path(), "class Widget", &EngineConfig::default());
    assert!(res.total_hits > 0, "гэп-fallback находит макро-декларацию");
    assert!(res.soft_fallback);
    assert_eq!(res.soft_fallback_kind.as_deref(), Some("type_decl_gap"));
    let top = &res.anchors[0];
    let preview = top.preview.as_deref().unwrap_or("");
    assert!(
        preview.contains("class MY_EXPORT Widget"),
        "превью обязано показать макро-декларацию: {preview:?}"
    );
}

#[test]
fn type_decl_gap_respects_qt_morphology() {
    // Qt: `class Q_CORE_EXPORT QObject` при запросе `class QObject :`
    // (вторая мировая конвенция экспорта после LLVM).
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("qobject.h"),
        "#pragma once\nclass Q_CORE_EXPORT QObject : public QObjectBase {\n\
         public:\n  QObject(QObject *parent = nullptr);\n};\n",
    )
    .unwrap();

    let res = scan_path(dir.path(), "class QObject :", &EngineConfig::default());
    assert!(res.total_hits > 0);
    assert!(res.soft_fallback);
    assert_eq!(res.soft_fallback_kind.as_deref(), Some("type_decl_gap"));
    let preview = res.anchors[0].preview.as_deref().unwrap_or("");
    assert!(
        preview.contains("class Q_CORE_EXPORT QObject"),
        "превью обязано показать Qt-определение: {preview:?}"
    );
}

// ---------------------------------------------------------------------------
// v0.86.0: три контура — O(N²)-фикс, template-гэп, Causal Nexus
// ---------------------------------------------------------------------------

/// Boost.Spirit-морфология (ГРАБЛЯ 45): параметрический блок <...> между
/// template и struct — строгая фраза слепа, template-гэп находит.
const BOOST_PARSE_HPP: &str = r#"#pragma once
namespace boost { namespace spirit { namespace qi {
template <typename Iterator, typename Skipper, typename Attr>
inline bool phrase_parse_impl(Iterator& first, Iterator last, Attr& attr){
    return true;
}
template <typename Iterator, typename Enable = void>
struct phrase_parse_impl {
    typedef Iterator type;
    void run() { }
};
template <typename T>
struct token_printer_debug {
    T t;
};
}}}
"#;

#[test]
fn template_decl_gap_finds_boost_declaration() {
    // ГРАБЛЯ 45: `template struct phrase_parse_impl` на v0.85.0 — exit 1
    // (строгая фраза слепа к параметрическому блоку). v0.86.0: гэп-паттерн
    // пропускает блок <...> и находит объявление.
    let dir = TempDir::new().unwrap();
    let inc = dir.path().join("include/boost/spirit/home/qi");
    fs::create_dir_all(&inc).unwrap();
    fs::write(inc.join("parse.hpp"), BOOST_PARSE_HPP).unwrap();

    let res = scan_path(
        dir.path(),
        "template struct phrase_parse_impl",
        &EngineConfig::default(),
    );
    assert!(
        res.total_hits > 0,
        "template-гэп обязан находить объявление за блоком <...>"
    );
    assert!(res.soft_fallback, "флаг soft_fallback не поднят");
    assert_eq!(
        res.soft_fallback_kind.as_deref(),
        Some("template_decl_gap"),
        "разновидность fallback обязана быть template_decl_gap"
    );
    // хит — строка `template <...>`: превью показывает объявление целиком
    let top = &res.anchors[0];
    assert!(
        top.file.ends_with("parse.hpp"),
        "топ-1 обязан быть объявлением: {}",
        top.file
    );
    let preview = top.preview.as_deref().expect("превью материализовано");
    let first = preview.lines().next().unwrap_or("");
    assert!(
        first.contains("template <typename Iterator, typename Enable = void>"),
        "превью обязано начинаться со строки template: {first:?}"
    );
    // однострочная форма тоже находится
    let res2 = scan_path(
        dir.path(),
        "template struct token_printer_debug",
        &EngineConfig::default(),
    );
    assert!(
        res2.total_hits > 0,
        "однострочная template-форма тоже обязана находиться"
    );
    // шаблонная ФУНКЦИЯ (inline bool) не путается со структурой:
    // её токен phrase_parse_impl внутри тела — не объявление в начале
    // строки с struct — не расширяет хиты без причины
}

#[test]
fn template_decl_query_rejects_nonmatching() {
    // class-key не тот / имя из блока — честный 0 без fallback-обмана
    let dir = TempDir::new().unwrap();
    let inc = dir.path().join("include/boost/spirit/home/qi");
    fs::create_dir_all(&inc).unwrap();
    fs::write(inc.join("parse.hpp"), BOOST_PARSE_HPP).unwrap();

    // в файле struct, запросили class — не нашлось
    let res = scan_path(
        dir.path(),
        "template class phrase_parse_impl",
        &EngineConfig::default(),
    );
    assert_eq!(res.total_hits, 0, "class-key mismatch обязан дать 0");
}

/// SQLite-морфология (ГРАБЛЯ 41): C-ядро + JNI-прослойка + заголовок —
/// все три сайта в ОДНОМ узле Causal Nexus.
const SQLITE_VDBEAPI_C: &str = r#"#include "sqlite3.h"
int sqlite3_step(sqlite3_stmt *pStmt){
    int rc = 0;
    Vdbe *v = (Vdbe*)pStmt;
    if (v->magic != 0x21bdc316) return 25;
    rc = sqlite3Step(v, 1);
    return rc & (0xff | 0x100);
}
"#;

const SQLITE_CAPI_JAVA: &str = r#"package org.sqlite.jni.capi;
public class CApi {
  public static int sqlite3_step(@NotNull sqlite3_stmt stmt){
    return null==stmt ? 1 : sqlite3_step(stmt.getNativePointer());
  }
}
"#;

const SQLITE_H: &str = r#"typedef struct sqlite3_stmt sqlite3_stmt;
int sqlite3_step(sqlite3_stmt*);
"#;

#[test]
fn causal_nexus_groups_cross_language_sites() {
    // Директива владельца: ИИ и человек видят ОДНОВРЕМЕННО C-ядро и
    // JNI-прослойку (граф, а не def-бонус). Ранжирование не искажается —
    // нексус чистая надстройка.
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("src");
    let jni = dir
        .path()
        .join("ext/jni/src/org/sqlite/jni/capi");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&jni).unwrap();
    fs::write(src.join("vdbeapi.c"), SQLITE_VDBEAPI_C).unwrap();
    fs::write(jni.join("CApi.java"), SQLITE_CAPI_JAVA).unwrap();
    fs::write(dir.path().join("sqlite3.h"), SQLITE_H).unwrap();

    let res = scan_path(dir.path(), "int sqlite3_step(", &EngineConfig::default());
    assert!(res.total_hits >= 2, "хиты обязаны найтись: {}", res.total_hits);

    // нексус: узел sqlite3_step с def-сайтами в ДВУХ языках
    let node = res
        .nexus
        .iter()
        .find(|n| n.symbol == "sqlite3_step")
        .expect("узел нексуса sqlite3_step обязан существовать");
    let files: Vec<&str> = node.sites.iter().map(|s| s.file.as_str()).collect();
    assert!(
        files.iter().any(|f| f.ends_with("vdbeapi.c")),
        "нет C-ядра в нексусе: {files:?}"
    );
    assert!(
        files.iter().any(|f| f.ends_with("CApi.java")),
        "нет JNI-прослойки в нексусе: {files:?}"
    );
    // оба def-сайта
    let defs: Vec<_> = node
        .sites
        .iter()
        .filter(|s| s.role == "def")
        .collect();
    assert!(
        defs.len() >= 2,
        "C-ядро и JNI обязаны быть def-сайтами: {:?}",
        node.sites
    );
    // якорь несёт метку узла
    let a0 = &res.anchors[0];
    assert_eq!(
        a0.nexus_symbol.as_deref(),
        Some("sqlite3_step"),
        "якорь обязан быть помечен nexus_symbol"
    );
    // K-hop рёбра графа символов присутствуют (call-graph hit-файлов)
    assert!(
        !node.relations.is_empty(),
        "нексус обязан нести рёбра графа символов"
    );
    // JSON-контракт: nexus сериализуется
    let v = serde_json::to_value(&res).unwrap();
    assert!(v["nexus"].is_array(), "nexus обязан сериализоваться");
    // simple-рендер несёт блок нексуса
    let s = poler_engine::render_simple(&res);
    assert!(
        s.contains("◇ Нексус «sqlite3_step»"),
        "simple не содержит нексус: {s}"
    );
}

#[test]
fn nexus_absent_for_prose_queries() {
    // проза не группируется в нексус: поле скрыто, узлов нет
    let dir = write_fixture("chapter_36.md", CH36);
    let res = scan_path(dir.path(), "нокс", &EngineConfig::default());
    assert!(res.total_hits > 0);
    assert!(res.nexus.is_empty(), "проза не даёт нексуса");
    assert!(res.anchors[0].nexus_symbol.is_none());
}
