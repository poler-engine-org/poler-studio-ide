//! Контракт вывода для AI-агента: самодостаточный Context Anchor.
//!
//! JSON-схема (спецификация §5):
//!
//! ```json
//! {
//!   "query": "нокс",
//!   "total_hits": 1,
//!   "anchors": [
//!     {
//!       "file": "/path/to/chapter_36.md",
//!       "token": "нокс",
//!       "epsilon": 7983.94,
//!       "resonance": 28714.73,
//!       "scene": {
//!         "chapter": "Глава 36. Инертный",
//!         "temporal_metric": "Метрика: Т-23",
//!         "location": "Локация: Разлом Каньона",
//!         "subjects": ["Субъекты: Мальчик (гибрид), Соболь (Нокс)"],
//!         "enclosing_scope": "Полный текст законченной сцены..."
//!       },
//!       "k_hop_relations": [["Нокс", "вонзила_когти", "Солнечное сплетение"]]
//!     }
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::parser::markdown_scenes::SceneContext;

/// v0.86.0 (ГРАБЛЯ 41, SQLite): сайт сущности в нексусе — точка, где
/// символ живёт (определение в конкретном файле/языке или место вызова).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NexusSite {
    /// Путь файла сайта (абсолютный — стабильный машинный контракт).
    pub file: String,
    /// Строка сайта, 1-based.
    pub line: usize,
    /// Роль сайта: `def` — определение/обёртка (хит в заголовке или теле
    /// определения символа), `call` — место вызова/использования,
    /// `ref` — упоминание без enclosing-определения (заголовок/макро).
    pub role: String,
    /// Вид символа из символьной таблицы (`fn`/`struct`/`class`/…).
    pub kind: String,
}

/// v0.86.0: причинно-следственный узел (Causal Nexus) — кросс-языковая
/// группировка top-N хитов по символу.
///
/// Директива владельца (SQLite-кейс): ИИ и человек обязаны видеть
/// ОДНОВРЕМЕННО и C-ядро (`vdbeapi.c:980`), и JNI-прослойку
/// (`CApi.java:2141`), и заголовок, и места вызовов — а не устраивать
/// драку языков за первое место. Ранжирование НЕ искажается: якоря
/// остаются в порядке (тир, −R) — нексус — надстройка-представление,
/// «граф, а не костыль» def-бонуса.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NexusNode {
    /// Имя сущности как в коде (case-sensitive, кросс-языковая
    /// идентичность по имени — `sqlite3_step` в C и в Java).
    pub symbol: String,
    /// Все сайты сущности среди top-N хитов: определения во всех
    /// языках + места вызовов.
    pub sites: Vec<NexusSite>,
    /// Рёбра графа символов в радиусе K-hop от сущности
    /// (`caller`, `"calls"`, `callee`), EntityGraph на символьной
    /// таблице hit-файлов.
    pub relations: Vec<(String, String, String)>,
    /// v0.87.0 VAULT-ASM (LENS): статистика No-Hits барьера —
    /// `(прошло, отсечено)`. Барьер сокровищницы: вес K-hop рёбер
    /// затухает 0.5^depth, связи с весом ≤ 0.05 отсекаются микроядром
    /// `poler_lens_filter` (SSE2). `None` — ничего не отсечено.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lens: Option<(usize, usize)>,
}

/// Один Context Anchor: самодостаточный контекст совпадения.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextAnchor {
    /// Абсолютный путь файла (стабильный контракт, для машины).
    pub file: String,
    /// Относительный путь от корня сканирования (v0.82.0, для человека/
    /// терминала). `None` — если путь вне корня (фолбэк на `file`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rel_file: Option<String>,
    /// Номер строки хита, 1-based; 0 — неизвестно (файл нечитаем).
    /// v0.82.0: переход к коду без лишнего действия (`gin.go:250`).
    #[serde(default)]
    pub line: usize,
    /// Превью, выровненное по строке хита (v0.83.0): строка совпадения —
    /// ПЕРВАЯ строка сниппета, а не «уехала вниз» за пределы превью.
    /// Прежний байт-сдвиг окна назад мог захватить закрывающий
    /// `extern "C"` вместо заголовка целевой функции (wireshark-кейс).
    /// `None` — файл нечитаем; рендер откатывается на enclosing_scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    pub token: String,
    pub epsilon: f64,
    pub resonance: f64,
    pub scene: SceneContext,
    pub k_hop_relations: Vec<(String, String, String)>,
    /// v0.86.0 (ГРАБЛЯ 41): символ нексуса, к которому отнесён этот хит
    /// (группировка по enclosing-определению из символьной таблицы
    /// hit-файлов). `None` — проза/хит без символа; сгруппированные
    /// якоря разворачиваются в `SearchResult::nexus`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nexus_symbol: Option<String>,
}

/// Итог поиска.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub query: String,
    /// Общее число совпадений (до усечения по top_n).
    pub total_hits: usize,
    /// v0.83.0: точный сигнатурный паттерн (прикреплённая скобка) не
    /// найден нигде — выданы хиты мягкого fallback по префиксу
    /// идентификатора без скобки (`proto_register_field(` → семейство
    /// `proto_register_field_array`). Скрыт в JSON, когда fallback
    /// не срабатывал.
    #[serde(default, skip_serializing_if = "is_false")]
    pub soft_fallback: bool,
    /// v0.85.0: разновидность мягкого fallback — "prefix" (v0.83.0,
    /// скобка) или "type_decl_gap" (макро-атрибут между class/struct и
    /// именем типа, ГРАБЛЯ 43). Скрыт в JSON, когда fallback не
    /// срабатывал; агенты различают причину без парсинга stderr.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soft_fallback_kind: Option<String>,
    /// v0.86.0 (ГРАБЛЯ 41, SQLite): кросс-языковые узлы сущностей —
    /// группировка top-N код-хитов по символу через символьную таблицу
    /// и [`crate::graph::EntityGraph`] (`from_symbol_table`, K-hop).
    /// ИИ видит всю цепь вызова (C-ядро + JNI-прослойка + вызовы) одним
    /// взглядом. Пусто для прозы и хитов без символов — поле скрыто.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nexus: Vec<NexusNode>,
    pub anchors: Vec<ContextAnchor>,
}

#[inline]
fn is_false(b: &bool) -> bool {
    !*b
}

/// Markdown-рендер результата (формат `--format md`).
pub fn render_markdown(res: &SearchResult) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# POLER-Engine: «{}» (Найдено: {})\n\n",
        res.query, res.total_hits
    ));
    for (i, a) in res.anchors.iter().enumerate() {
        // v0.83.0: путь, резолвящийся из CWD (файл под CWD → относительно
        // CWD, иначе абсолютный) + строка хита
        let shown = display_path(a);
        let where_ref = if a.line > 0 {
            format!("{shown}:{}", a.line)
        } else {
            shown.to_string()
        };
        out.push_str(&format!(
            "## {}. Файл: `{}` (R={:.2}, ε={:.2})\n\n",
            i + 1,
            where_ref,
            a.resonance,
            a.epsilon
        ));
        if let Some(m) = &a.scene.temporal_metric {
            let val = m.strip_prefix("Метрика: ").unwrap_or(m);
            out.push_str(&format!("- **Метрика:** {val}\n"));
        }
        if let Some(l) = &a.scene.location {
            let val = l.strip_prefix("Локация: ").unwrap_or(l);
            out.push_str(&format!("- **Локация:** {val}\n"));
        }
        for s in &a.scene.subjects {
            let val = s.strip_prefix("Субъекты: ").unwrap_or(s);
            out.push_str(&format!("- **Субъекты:** {val}\n"));
        }
        if !a.k_hop_relations.is_empty() {
            out.push_str("- **K-Hop связи:**\n");
            for (s, p, o) in &a.k_hop_relations {
                out.push_str(&format!("  - `{s}` —{p}→ `{o}`\n"));
            }
        }
        out.push_str(&format!("\n```text\n{}\n```\n---\n\n", a.scene.enclosing_scope));
    }
    out.push_str(&render_nexus_markdown(res));
    out
}

/// v0.86.0: рендер Causal Nexus в Markdown — кросс-языковое дерево
/// сущности (определения во всех языках + места вызовов + K-hop рёбра).
fn render_nexus_markdown(res: &SearchResult) -> String {
    let mut out = String::new();
    for node in &res.nexus {
        out.push_str(&format!(
            "## Causal Nexus: `{}` (сайтов: {}, связей: {})\n\n",
            node.symbol,
            node.sites.len(),
            node.relations.len()
        ));
        // v0.87.0 VAULT-ASM: LENS No-Hits барьер (порог 0.05 сокровищницы)
        if let Some((kept, blocked)) = node.lens {
            out.push_str(&format!(
                "> LENS-барьер 0.05: прошло {} из {} (тусклых K-hop связей отсечено: {})\n\n",
                kept,
                kept + blocked,
                blocked
            ));
        }
        for s in &node.sites {
            let role = match s.role.as_str() {
                "def" => "определение",
                "call" => "вызов",
                _ => "упоминание",
            };
            out.push_str(&format!("- **{role}** `{}:{}` ({})\n", s.file, s.line, s.kind));
        }
        if !node.relations.is_empty() {
            out.push_str("\n**Граф символов (K-hop):**\n\n");
            for (subj, pred, obj) in &node.relations {
                out.push_str(&format!("- `{subj}` —{pred}→ `{obj}`\n"));
            }
        }
        out.push('\n');
    }
    out
}

/// Путь для терминала/агента (v0.83.0).
///
/// Кейс владельца: агент стоит в `/home/vitalij`, скан запущен по
/// `/home/vitalij/Стільниця/ripgrep-src` — путь `crates/globset/lib.rs`
/// (от корня скана) НЕ резолвится инструментами агента. Правило:
/// файл под CWD → путь относительно CWD (короткий И резолвящийся),
/// иначе — абсолютный (резолвится всегда). Корень скана == CWD —
/// совпадает с прежним поведением v0.82.0.
fn display_path(a: &ContextAnchor) -> String {
    std::env::current_dir().map_or_else(
        |_| a.file.clone(),
        |cwd| display_path_with(a, &cwd),
    )
}

/// Чистая форма [`display_path`] с явным CWD — для тестов без chdir.
fn display_path_with(a: &ContextAnchor, cwd: &std::path::Path) -> String {
    let file = std::path::Path::new(&a.file);
    if let Ok(rel) = file.strip_prefix(cwd) {
        if !rel.as_os_str().is_empty() {
            return rel.to_string_lossy().to_string();
        }
    }
    a.file.clone()
}

/// Обрезка по СИМВОЛАМ с многоточием (границы UTF-8 не рвутся).
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

/// Компактный сниппет якоря (v0.83.0).
///
/// Строка хита (первая строка превью) — ЦЕЛИКОМ: сигнатура с
/// модификаторами (`pub fn schedule(...) callconv(.c) u64`) больше не
/// теряет хвост за многоточием. Последующие строки — компактно.
fn simple_snippet(a: &ContextAnchor) -> String {
    /// Первая строка (строка хита/сигнатура) — без обрезки до этого лимита.
    const HEAD_CHARS: usize = 200;
    /// Хвостовые строки — компактно для терминала.
    const TAIL_CHARS: usize = 64;
    /// Сколько хвостовых строк показывать.
    const TAIL_LINES: usize = 2;
    let text = a.preview.as_deref().unwrap_or(&a.scene.enclosing_scope);
    let mut lines = text.split('\n');
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut out = truncate_chars(first.trim(), HEAD_CHARS);
    for line in lines.take(TAIL_LINES) {
        let l = line.trim_end();
        if l.trim().is_empty() {
            continue;
        }
        out.push('\n');
        out.push_str(&truncate_chars(l, TAIL_CHARS));
    }
    out
}

/// Компактный рендер (формат `--format simple`).
///
/// v0.82.0: `путь:строка` вместо абсолютного пути, съедающего половину
/// терминала.
///
/// v0.83.0: путь резолвится из CWD (файл под CWD → относительно CWD,
/// иначе абсолютный); сниппет начинается СО СТРОКИ ХИТА — сигнатура
/// целиком, хвост — компактно.
///
/// v0.86.0: после списка якорей — компактный блок Causal Nexus
/// (кросс-языковые узлы сущностей).
pub fn render_simple(res: &SearchResult) -> String {
    let mut out = String::new();
    let total = res.anchors.len();
    for (i, a) in res.anchors.iter().enumerate() {
        let shown = display_path(a);
        let where_ref = if a.line > 0 {
            format!("{shown}:{}", a.line)
        } else {
            shown
        };
        out.push_str(&format!(
            "[{}/{}] R={:.1} ε={:.1} | {} | {}\n",
            i + 1,
            total,
            a.resonance,
            a.epsilon,
            where_ref,
            simple_snippet(a)
        ));
    }
    out.push_str(&render_nexus_simple(res));
    out
}

/// v0.86.0: компактный рендер нексуса для `--format simple`.
fn render_nexus_simple(res: &SearchResult) -> String {
    let mut out = String::new();
    for node in &res.nexus {
        out.push_str(&format!(
            "◇ Нексус «{}»: {} сайтов, {} связей\n",
            node.symbol,
            node.sites.len(),
            node.relations.len()
        ));
        for s in &node.sites {
            out.push_str(&format!(
                "    {:<4} {}:{} ({})\n",
                s.role,
                s.file,
                s.line,
                s.kind
            ));
        }
        for (subj, pred, obj) in node.relations.iter().take(8) {
            out.push_str(&format!("    ⤷ {subj} —{pred}→ {obj}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::markdown_scenes::SceneContext;

    fn sample() -> SearchResult {
        let scene = SceneContext {
            chapter: "Глава 36. Инертный".into(),
            temporal_metric: Some("Метрика: Т-23".into()),
            location: Some("Локация: Разлом Каньона".into()),
            subjects: vec!["Субъекты: Мальчик (гибрид), Соболь (Нокс)".into()],
            enclosing_scope: "Нокс вонзила когти...".into(),
            metric_tag: Some("Т-23".into()),
            subject_names: vec![],
            subject_pairs: vec![],
        };
        SearchResult {
            query: "нокс".into(),
            total_hits: 3,
            soft_fallback: false,
            soft_fallback_kind: None,
            nexus: Vec::new(),
            anchors: vec![ContextAnchor {
                file: "/book/chapter_36.md".into(),
                rel_file: Some("chapter_36.md".into()),
                line: 250,
                preview: Some("Нокс вонзила когти...".into()),
                token: "нокс".into(),
                epsilon: 7983.94,
                resonance: 28714.73,
                scene,
                k_hop_relations: vec![(
                    "Нокс".into(),
                    "вонзила_когти".into(),
                    "Солнечное сплетение".into(),
                )],
                nexus_symbol: None,
            }],
        }
    }

    #[test]
    fn json_contract_shape() {
        let v = serde_json::to_value(sample()).unwrap();
        assert_eq!(v["query"], "нокс");
        assert_eq!(v["total_hits"], 3);
        let a = &v["anchors"][0];
        assert_eq!(a["file"], "/book/chapter_36.md");
        assert_eq!(a["rel_file"], "chapter_36.md");
        assert_eq!(a["line"], 250);
        assert_eq!(a["token"], "нокс");
        assert_eq!(a["preview"], "Нокс вонзила когти...");
        assert!(a["epsilon"].is_f64());
        assert!(a["resonance"].is_f64());
        assert_eq!(a["scene"]["chapter"], "Глава 36. Инертный");
        assert_eq!(a["scene"]["temporal_metric"], "Метрика: Т-23");
        assert_eq!(a["scene"]["location"], "Локация: Разлом Каньона");
        assert!(a["scene"]["subjects"].is_array());
        assert!(a["scene"]["enclosing_scope"].is_string());
        // скрытые поля не сериализуются
        assert!(a["scene"].get("metric_tag").is_none());
        assert!(a["scene"].get("subject_names").is_none());
        // soft_fallback=false скрыт (v0.83.0: флаг появляется только в fallback)
        assert!(v.get("soft_fallback").is_none());
        // v0.85.0: разновидность тоже скрыта без fallback
        assert!(v.get("soft_fallback_kind").is_none());
        // v0.86.0: пустой нексус скрыт
        assert!(v.get("nexus").is_none());
        // k_hop_relations — массивы из трёх строк
        let rel = &a["k_hop_relations"][0];
        assert_eq!(rel.as_array().unwrap().len(), 3);
        assert_eq!(rel[0], "Нокс");
        assert_eq!(rel[1], "вонзила_когти");
        assert_eq!(rel[2], "Солнечное сплетение");
    }

    #[test]
    fn json_backward_compat_old_fields_absent() {
        // десериализация JSON БЕЗ rel_file/line (контракт v0.81) —
        // serde(default) держит обратную совместимость
        let old = serde_json::json!({
            "file": "/book/chapter_36.md",
            "token": "нокс",
            "epsilon": 1.0,
            "resonance": 2.0,
            "scene": {
                "chapter": "глава",
                "temporal_metric": null,
                "location": null,
                "subjects": [],
                "enclosing_scope": "текст"
            },
            "k_hop_relations": []
        });
        let a: ContextAnchor = serde_json::from_value(old).unwrap();
        assert_eq!(a.line, 0);
        assert!(a.rel_file.is_none());
        assert!(a.preview.is_none());
        assert_eq!(a.file, "/book/chapter_36.md");
    }

    #[test]
    fn roundtrip_deserialize() {
        let json = serde_json::to_string(&sample()).unwrap();
        let back: SearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.query, "нокс");
        assert_eq!(back.anchors[0].k_hop_relations.len(), 1);
    }

    #[test]
    fn markdown_render_contains_sections() {
        let md = render_markdown(&sample());
        assert!(md.contains("POLER-Engine"));
        assert!(md.contains("**Метрика:** Т-23"));
        assert!(md.contains("**Локация:** Разлом Каньона"));
        assert!(md.contains("вонзила_когти"));
        assert!(md.contains("```text"));
    }

    #[test]
    fn simple_render_one_line_per_anchor() {
        let s = render_simple(&sample());
        // превью однострочное → одна строка вывода на якорь
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("R="));
        // v0.83.0: путь:строка; файл вне CWD → абсолютный (всегда резолвится)
        assert!(s.contains("chapter_36.md:250"), "нет пути со строкой: {s}");
        // сниппет — из превью, выровненного по строке хита
        assert!(s.contains("Нокс вонзила когти"), "нет сниппета: {s}");
    }

    #[test]
    fn display_path_prefers_cwd_relative() {
        // v0.83.0: путь в терминале обязан резолвиться из CWD агента
        // (кейс: агент в /home/vitalij, скан по .../Стільниця/ripgrep-src).
        let mut res = sample();
        let a = &mut res.anchors[0];
        a.file = "/home/u/repo/src/lib.rs".into();
        // файл под CWD → относительно CWD
        assert_eq!(
            display_path_with(a, std::path::Path::new("/home/u/repo")),
            "src/lib.rs"
        );
        // директория файла == CWD → имя файла
        assert_eq!(
            display_path_with(a, std::path::Path::new("/home/u/repo/src")),
            "lib.rs"
        );
        // файл вне CWD → абсолютный
        assert_eq!(
            display_path_with(a, std::path::Path::new("/tmp")),
            "/home/u/repo/src/lib.rs"
        );
    }

    #[test]
    fn simple_render_signature_line_not_truncated() {
        // v0.83.0 (кейс poler-os/Zig): сигнатура с модификаторами длиннее
        // хвостового лимита — обязана присутствовать ЦЕЛИКОМ, обрезка —
        // только для хвостовых строк тела.
        let mut res = sample();
        let a = &mut res.anchors[0];
        a.preview = Some(
            "pub fn schedule(current_rsp: u64) callconv(.c) u64 { // v0.13.0 arch\n\
             if (task_count <= 1) return current_rsp; // Only idle/kernel task exists\n\
             const saved_rsp = current_rsp;\n\
             const unreachable_tail_line = 0;\n"
                .into(),
        );
        let s = render_simple(&res);
        assert!(
            s.contains("pub fn schedule(current_rsp: u64) callconv(.c) u64 { // v0.13.0 arch"),
            "сигнатура обрезана: {s}"
        );
        // хвостовые строки — компактно: хвост за 64 символами срезан
        assert!(s.contains('…'), "нет маркера обрезки хвоста: {s}");
        assert!(!s.contains("task exists"), "хвостовая строка не обрезана: {s}");
        assert!(
            !s.contains("unreachable_tail_line"),
            "третья хвостовая строка не должна показываться: {s}"
        );
        // заголовок с сигнатурой (одна строка) + 2 хвостовых = 3 строки
        assert_eq!(s.lines().count(), 3, "строки вывода: {s}");
    }

    #[test]
    fn simple_render_fallback_without_line() {
        let mut res = sample();
        let a = &mut res.anchors[0];
        a.line = 0;
        a.rel_file = None;
        let s = render_simple(&res);
        assert!(s.contains("/book/chapter_36.md |"), "фолбэк на абсолютный путь: {s}");
    }

    #[test]
    fn nexus_render_and_json_contract() {
        // v0.86.0 (ГРАБЛЯ 41): кросс-языковой нексус — рендер + JSON-контракт
        let mut res = sample();
        res.query = "int sqlite3_step(".into();
        res.anchors[0].nexus_symbol = Some("sqlite3_step".into());
        res.nexus = vec![NexusNode {
            symbol: "sqlite3_step".into(),
            sites: vec![
                NexusSite {
                    file: "src/vdbeapi.c".into(),
                    line: 980,
                    role: "def".into(),
                    kind: "fn".into(),
                },
                NexusSite {
                    file: "ext/jni/CApi.java".into(),
                    line: 2141,
                    role: "def".into(),
                    kind: "fn".into(),
                },
                NexusSite {
                    file: "shell.c".into(),
                    line: 8123,
                    role: "call".into(),
                    kind: "fn".into(),
                },
            ],
            relations: vec![("main".into(), "calls".into(), "sqlite3_step".into())],
            // v0.87.0: LENS-статистика — None сериализуется как отсутствие поля
            lens: None,
        }];
        // JSON-контракт: nexus сериализуется, якорь несёт nexus_symbol
        let v = serde_json::to_value(&res).unwrap();
        let nx = &v["nexus"][0];
        assert_eq!(nx["symbol"], "sqlite3_step");
        assert_eq!(nx["sites"].as_array().unwrap().len(), 3);
        assert_eq!(nx["sites"][1]["file"], "ext/jni/CApi.java");
        assert_eq!(nx["sites"][2]["role"], "call");
        assert_eq!(v["anchors"][0]["nexus_symbol"], "sqlite3_step");
        // v0.87.0: lens=None → поле скрыто (обратная совместимость JSON)
        assert!(nx.get("lens").is_none());
        // простой-рендер: компактный блок нексуса
        let s = render_simple(&res);
        assert!(s.contains("◇ Нексус «sqlite3_step»"), "нет заголовка нексуса: {s}");
        assert!(s.contains("src/vdbeapi.c:980"), "нет C-ядра: {s}");
        assert!(s.contains("ext/jni/CApi.java:2141"), "нет JNI-моста: {s}");
        // md-рендер: секция Causal Nexus
        let md = render_markdown(&res);
        assert!(md.contains("## Causal Nexus: `sqlite3_step`"), "нет секции md: {md}");
        assert!(md.contains("**вызов** `shell.c:8123`"), "нет call-сайта md: {md}");
        // v0.87.0: LENS-строка появляется только при отсечениях
        res.nexus[0].lens = Some((7, 3));
        let md2 = render_markdown(&res);
        assert!(md2.contains("LENS-барьер 0.05: прошло 7 из 10"), "нет LENS-строки: {md2}");
    }
}
