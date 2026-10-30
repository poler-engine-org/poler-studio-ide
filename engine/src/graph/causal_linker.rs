//! CausalLinker — каузальный верификатор стыков модулей («голос» компилятора).
//! Ступень v0.76, контур A2.
//!
//! Директива владельца: «внутренний компилятор — это голос, который
//! математически просчитывает, как соединять модули правильно». Этот модуль —
//! арбитр сборки: перед компиляцией конвейера он проверяет каждый стык
//! (размерности, трит-глубина, семейство типов, инварианты нормы, аллокации,
//! W^X-дисциплина) и при провале НЕ паникует, а ставит диагноз строкой
//! на естественном русском языке и предлагает перестройку графа
//! (автовставку адаптеров там, где она математически корректна).
//!
//! Выход — проверенный [`DataflowGraph`], готовый к
//! [`crate::graph::graph_asm::DataflowCompiler::compile_x86_64`],
//! плюс расписание runtime-стадий (нормировка ‖Ω‖=1 — данные-зависимая,
//! линейным слоем не выражается).
//!
//! Все проверки детерминированы: один и тот же spec даёт тот же вердикт.

use crate::graph::graph_asm::{DataflowGraph, NodeAct, Reducer, WeightKind};

/// Тип данных на порте модуля.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortKind {
    /// Вектор тритов {−1, 0, +1} с трит-глубиной (тритов на символ).
    TritVector { depth: u8 },
    /// Вектор плотных f32.
    F32Vector,
}

/// Порт модуля.
#[derive(Debug, Clone)]
pub struct ModulePort {
    pub name: String,
    pub kind: PortKind,
    pub width: usize,
    /// Заявленный инвариант ‖выход‖ = 1 (для выходных портов).
    pub norm_unit: bool,
}

/// Описание модуля конвейера.
#[derive(Debug, Clone)]
pub struct ModuleSpec {
    pub name: String,
    pub inputs: Vec<ModulePort>,
    pub outputs: Vec<ModulePort>,
    /// Рёбра внутреннего графа: (вход, смещение) → (выход, смещение) × вес.
    pub edges: Vec<((usize, usize), (usize, usize), WeightKind)>,
    /// Аллокации в горячем пути за цикл (>0 — паразитные).
    pub hot_allocations: usize,
    /// Код-генерирующий модуль соблюдает W^X?
    pub wx_compliant: bool,
    /// Линейный слой (компилируется в kernel); false → runtime-стадия.
    pub jit_linear: bool,
}

impl ModuleSpec {
    /// Источник конвейера: входов нет, один выход.
    pub fn source(name: &str, width: usize) -> Self {
        Self {
            name: name.into(),
            inputs: Vec::new(),
            outputs: vec![ModulePort { name: "out".into(), kind: PortKind::F32Vector, width, norm_unit: false }],
            edges: Vec::new(),
            hot_allocations: 0,
            wx_compliant: true,
            jit_linear: true,
        }
    }

    /// Линейный слой: один вход, один выход.
    pub fn linear(name: &str, in_width: usize, out_width: usize, edges: Vec<((usize, usize), (usize, usize), WeightKind)>) -> Self {
        Self {
            name: name.into(),
            inputs: vec![ModulePort { name: "in".into(), kind: PortKind::F32Vector, width: in_width, norm_unit: false }],
            outputs: vec![ModulePort { name: "out".into(), kind: PortKind::F32Vector, width: out_width, norm_unit: false }],
            edges,
            hot_allocations: 0,
            wx_compliant: true,
            jit_linear: true,
        }
    }

    /// Runtime-стадия нормировки ‖x‖ → 1.
    pub fn normalizer(name: &str, width: usize) -> Self {
        Self {
            name: name.into(),
            inputs: vec![ModulePort { name: "in".into(), kind: PortKind::F32Vector, width, norm_unit: false }],
            outputs: vec![ModulePort { name: "out".into(), kind: PortKind::F32Vector, width, norm_unit: true }],
            edges: Vec::new(),
            hot_allocations: 0,
            wx_compliant: true,
            jit_linear: false,
        }
    }
}

/// Связь: выход модуля A → вход модуля B.
#[derive(Debug, Clone)]
pub struct PipelineLink {
    pub from_module: usize,
    pub from_port: usize,
    pub to_module: usize,
    pub to_port: usize,
}

/// Спецификация конвейера.
#[derive(Debug, Clone)]
pub struct PipelineSpec {
    pub modules: Vec<ModuleSpec>,
    pub links: Vec<PipelineLink>,
}

/// Серьёзность диагноза.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Severity {
    /// Фатально: компиляция отклонена.
    Error,
    /// Исправлено автовставкой адаптера.
    Healed,
    /// Наблюдение.
    Warn,
}

/// Один диагноз компилятора — его «речь».
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub fix: Option<String>,
}

/// Runtime-стадия, не попавшая в машинный код.
#[derive(Debug, Clone)]
pub struct RuntimeStage {
    pub module: String,
    pub purpose: String,
    pub width: usize,
}

/// Результат успешной линковки.
pub struct LinkedPipeline {
    pub graph: DataflowGraph,
    pub runtime_stages: Vec<RuntimeStage>,
    pub report: LinkerReport,
}

#[derive(Debug, Clone, Default)]
pub struct LinkerReport {
    pub diagnostics: Vec<Diagnostic>,
    pub checks: usize,
    pub healed: usize,
    pub errors: usize,
}

impl LinkerReport {
    /// «Речь» компилятора: естественноязычное резюме вердикта.
    pub fn speech(&self) -> String {
        let mut s = format!(
            "Проверено стыков: {}. Диагнозов: {} (исправлено адаптерами: {}, фатальных: {}).",
            self.checks, self.diagnostics.len(), self.healed, self.errors
        );
        for d in &self.diagnostics {
            let tag = match d.severity {
                Severity::Error => "ОШИБКА",
                Severity::Healed => "ИСПРАВЛЕНО",
                Severity::Warn => "ВНИМАНИЕ",
            };
            s.push_str(&format!("\n[{tag}] {}", d.message));
            if let Some(f) = &d.fix {
                s.push_str(&format!(" → починка: {f}"));
            }
        }
        if self.errors == 0 {
            s.push_str("\nВердикт: конвейер математически согласован, готов к компиляции в один kernel.");
        } else {
            s.push_str("\nВердикт: компиляция отклонена — см. ОШИБКИ выше.");
        }
        s
    }
}

// ─── Адаптеры (автопочинка стыков) ────────────────────────────────────────

/// TritExpander: ширина m → m·k (каждый вход тиражируется k раз).
fn adapter_expander(name: &str, in_width: usize, factor: usize, depth: u8) -> ModuleSpec {
    let out_width = in_width * factor;
    let mut edges = Vec::with_capacity(out_width);
    for j in 0..out_width {
        edges.push(((0, j / factor), (0, j), WeightKind::F32(1.0)));
    }
    ModuleSpec {
        name: name.into(),
        inputs: vec![ModulePort { name: "in".into(), kind: PortKind::TritVector { depth }, width: in_width, norm_unit: false }],
        outputs: vec![ModulePort { name: "out".into(), kind: PortKind::TritVector { depth }, width: out_width, norm_unit: false }],
        edges,
        hot_allocations: 0,
        wx_compliant: true,
        jit_linear: true,
    }
}

/// DensePad: m → n (m < n): тождественно + вакуумный хвост.
fn adapter_padder(name: &str, in_width: usize, out_width: usize) -> ModuleSpec {
    let mut edges = Vec::new();
    for j in 0..in_width.min(out_width) {
        edges.push(((0, j), (0, j), WeightKind::F32(1.0)));
    }
    ModuleSpec {
        name: name.into(),
        inputs: vec![ModulePort { name: "in".into(), kind: PortKind::F32Vector, width: in_width, norm_unit: false }],
        outputs: vec![ModulePort { name: "out".into(), kind: PortKind::F32Vector, width: out_width, norm_unit: false }],
        edges,
        hot_allocations: 0,
        wx_compliant: true,
        jit_linear: true,
    }
}

/// KindAdapter: трит-вектор → f32 (тождественный перенос ±1/0).
fn adapter_kind(name: &str, width: usize, depth: u8) -> ModuleSpec {
    let mut edges = Vec::with_capacity(width);
    for j in 0..width {
        edges.push(((0, j), (0, j), WeightKind::Trit { val: 1, scale: 1.0 }));
    }
    ModuleSpec {
        name: name.into(),
        inputs: vec![ModulePort { name: "in".into(), kind: PortKind::TritVector { depth }, width, norm_unit: false }],
        outputs: vec![ModulePort { name: "out".into(), kind: PortKind::F32Vector, width, norm_unit: false }],
        edges,
        hot_allocations: 0,
        wx_compliant: true,
        jit_linear: true,
    }
}

/// TritLifter: f32 → трит-вектор (квантизация знаком — активация SignTrit).
fn adapter_trit_lifter(name: &str, width: usize, depth: u8) -> (ModuleSpec, NodeAct) {
    let mut edges = Vec::with_capacity(width);
    for j in 0..width {
        edges.push(((0, j), (0, j), WeightKind::F32(1.0)));
    }
    (
        ModuleSpec {
            name: name.into(),
            inputs: vec![ModulePort { name: "in".into(), kind: PortKind::F32Vector, width, norm_unit: false }],
            outputs: vec![ModulePort { name: "out".into(), kind: PortKind::TritVector { depth }, width, norm_unit: false }],
            edges,
            hot_allocations: 0,
            wx_compliant: true,
            jit_linear: true,
        },
        NodeAct::SignTrit,
    )
}

/// Вставленный адаптер + его активация.
struct HealedAdapter {
    spec: ModuleSpec,
    act: NodeAct,
}

// ─── Линковщик ────────────────────────────────────────────────────────────

/// Проверить и слинковать конвейер.
pub fn link(spec: &PipelineSpec) -> Result<LinkedPipeline, LinkerReport> {
    let mut report = LinkerReport::default();
    let n = spec.modules.len();

    // 1. Топология: циклы
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for l in &spec.links {
        adj[l.from_module].push(l.to_module);
    }
    let mut state = vec![0u8; n];
    let mut has_cycle = false;
    fn dfs(v: usize, adj: &Vec<Vec<usize>>, st: &mut Vec<u8>, cyc: &mut bool) {
        if st[v] == 1 {
            *cyc = true;
            return;
        }
        if st[v] == 2 {
            return;
        }
        st[v] = 1;
        for &w in &adj[v] {
            dfs(w, adj, st, cyc);
        }
        st[v] = 2;
    }
    for v in 0..n {
        dfs(v, &adj, &mut state, &mut has_cycle);
    }
    report.checks += 1;
    if has_cycle {
        report.errors += 1;
        report.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: "граф конвейера содержит цикл — dataflow-компилятор компилирует только DAG".into(),
            fix: Some("разорви цикл задержкой (register/tap-модуль) и проведи обратную связь отдельным контуром".into()),
        });
        return Err(report);
    }

    // 2. Один продюсер на вход
    let mut producers: std::collections::HashMap<(usize, usize), usize> = std::collections::HashMap::new();
    for (li, l) in spec.links.iter().enumerate() {
        report.checks += 1;
        if l.from_module >= n || l.to_module >= n {
            report.errors += 1;
            report.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!("связь #{li}: индекс модуля вне диапазона ({}→{} при {n})", l.from_module, l.to_module),
                fix: None,
            });
            continue;
        }
        if producers.contains_key(&(l.to_module, l.to_port)) {
            report.errors += 1;
            report.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!("вход {}:{}.{} получает второго продюсера (связь #{li})", spec.modules[l.to_module].name, l.to_port, li),
                fix: Some("введи mixer-модуль (сумматор) вместо второго подключения".into()),
            });
            continue;
        }
        producers.insert((l.to_module, l.to_port), l.from_module);
    }
    if report.errors > 0 {
        return Err(report);
    }

    // 3. Висячие входы
    for (mi, m) in spec.modules.iter().enumerate() {
        for (pi, p) in m.inputs.iter().enumerate() {
            if !producers.contains_key(&(mi, pi)) {
                report.errors += 1;
                report.diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("вход {}.{} «{}» не подключён — данные не придут", m.name, pi, p.name),
                    fix: Some(format!("подключи источник ширины {}", p.width)),
                });
            }
        }
    }
    if report.errors > 0 {
        return Err(report);
    }

    // 4. Поканальная верификация + автопочинка
    let mut healed: Vec<(usize, HealedAdapter, usize, usize)> = Vec::new();
    let mut norm_stage_before: Vec<(usize, usize)> = Vec::new();
    for (li, l) in spec.links.iter().enumerate() {
        let from = &spec.modules[l.from_module];
        let to = &spec.modules[l.to_module];
        let fp = &from.outputs[l.from_port.min(from.outputs.len().saturating_sub(1))];
        let tp = &to.inputs[l.to_port.min(to.inputs.len().saturating_sub(1))];
        report.checks += 2;

        // 4a. ширина
        if fp.width != tp.width {
            if fp.width < tp.width {
                let (adapter, act) = if let PortKind::TritVector { depth } = tp.kind {
                    if tp.width % fp.width.max(1) == 0 {
                        (adapter_expander(&format!("expander_{li}"), fp.width, tp.width / fp.width.max(1), depth), NodeAct::Identity)
                    } else {
                        (adapter_padder(&format!("padder_{li}"), fp.width, tp.width), NodeAct::Identity)
                    }
                } else {
                    (adapter_padder(&format!("padder_{li}"), fp.width, tp.width), NodeAct::Identity)
                };
                report.healed += 1;
                report.diagnostics.push(Diagnostic {
                    severity: Severity::Healed,
                    message: format!("стык {}→{} ({}:{} → {}:{}): пришло {} значений, ждёт {}", l.from_module, l.to_module, from.name, fp.name, to.name, tp.name, fp.width, tp.width),
                    fix: Some(format!("вставлен адаптер «{}» ({})", adapter.name, if adapter.name.starts_with("expander") { "репликация каждого трита" } else { "хвост — вакуум" })),
                });
                healed.push((li, HealedAdapter { spec: adapter, act }, l.to_module, l.to_port));
                continue;
            } else {
                report.errors += 1;
                report.diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("стык {}→{} ({}:{} → {}:{}): продюсер даёт {} значений, потребитель ждёт меньше ({}) — сужение теряет данные", l.from_module, l.to_module, from.name, fp.name, to.name, tp.name, fp.width, tp.width),
                    fix: Some("поставь редьюсер (сумматор/максиматор) или прореди выход".into()),
                });
                continue;
            }
        }

        // 4b. семейство типов (триты vs f32; глубина — отдельной проверкой)
        let f_trit = matches!(fp.kind, PortKind::TritVector { .. });
        let t_trit = matches!(tp.kind, PortKind::TritVector { .. });
        if f_trit != t_trit {
            if let (PortKind::TritVector { depth }, PortKind::F32Vector) = (fp.kind, tp.kind) {
                let a = adapter_kind(&format!("kind_{li}"), fp.width, depth);
                report.healed += 1;
                report.diagnostics.push(Diagnostic {
                    severity: Severity::Healed,
                    message: format!("стык {}→{}: трит-вектор (глубина {depth}) подключён к f32-входу без конвертации", l.from_module, l.to_module),
                    fix: Some(format!("вставлен KindAdapter «{}» (тождественный перенос ±1/0)", a.name)),
                });
                healed.push((li, HealedAdapter { spec: a, act: NodeAct::Identity }, l.to_module, l.to_port));
                continue;
            }
            if let (PortKind::F32Vector, PortKind::TritVector { depth }) = (fp.kind, tp.kind) {
                let (a, act) = adapter_trit_lifter(&format!("lifter_{li}"), fp.width, depth);
                report.healed += 1;
                report.diagnostics.push(Diagnostic {
                    severity: Severity::Healed,
                    message: format!("стык {}→{}: f32-вектор подключён к трит-входу (глубина {depth}) без квантизации", l.from_module, l.to_module),
                    fix: Some(format!("вставлен TritLifter «{}» (квантизация знаком SignTrit: −1/0/+1)", a.name)),
                });
                healed.push((li, HealedAdapter { spec: a, act }, l.to_module, l.to_port));
                continue;
            }
            report.errors += 1;
            report.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!("стык {}→{}: несовместимые семейства типов {:?} → {:?} без адаптера", l.from_module, l.to_module, fp.kind, tp.kind),
                fix: None,
            });
            continue;
        }

        // 4c. трит-глубина не теряется (обе стороны — триты)
        if let (PortKind::TritVector { depth: d_in }, PortKind::TritVector { depth: d_out }) = (fp.kind, tp.kind) {
            if d_out > d_in {
                report.errors += 1;
                report.diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("стык {}→{} ({}:{} → {}:{}): трит-глубина падает с {d_in} до {d_out} — потребитель читает несуществующие триты", l.from_module, l.to_module, from.name, fp.name, to.name, tp.name),
                    fix: Some("вставь TritDepthAdapter (достройка вакуумом) на стороне продюсера".into()),
                });
                continue;
            }
            if d_out < d_in {
                report.diagnostics.push(Diagnostic {
                    severity: Severity::Warn,
                    message: format!("стык {}→{}: трит-глубина {d_in} → {d_out}: старшие триты отбрасываются (осознанное сжатие?)", l.from_module, l.to_module),
                    fix: None,
                });
            }
        }

        // 4d. норма: потребитель требует ‖Ω‖=1, продюсер не гарантирует
        if tp.norm_unit && !fp.norm_unit {
            report.healed += 1;
            report.diagnostics.push(Diagnostic {
                severity: Severity::Healed,
                message: format!("стык {}→{} ({}:{} → {}:{}): потребитель требует ‖Ω‖=1, продюсер инвариант не заявляет", l.from_module, l.to_module, from.name, fp.name, to.name, tp.name),
                fix: Some("вставлена runtime-стадия нормировки (‖x‖→1) — данные-зависимая, в kernel не компилируется".into()),
            });
            norm_stage_before.push((l.to_module, fp.width));
        }
    }

    // 5. Дисциплина горячего пути
    for m in &spec.modules {
        report.checks += 2;
        if m.hot_allocations > 0 {
            report.diagnostics.push(Diagnostic {
                severity: Severity::Warn,
                message: format!("модуль «{}»: {} аллокаций в горячем пути за цикл — губернатор памяти O(1) под угрозой", m.name, m.hot_allocations),
                fix: Some("перенеси буферы в стековый кадр / SoA-поле, строящееся один раз".into()),
            });
        }
        if !m.wx_compliant {
            report.errors += 1;
            report.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!("модуль «{}» генерирует код, но не соблюдает W^X (страница W+X) — канон запрещает", m.name),
                fix: Some("раздели запись и исполнение: mmap → копия → mprotect(PROT_READ|PROT_EXEC)".into()),
            });
        }
    }
    if report.errors > 0 {
        return Err(report);
    }

    // 6. Сборка DataflowGraph (топологический порядок)
    let mut indeg = vec![0usize; n];
    for l in &spec.links {
        indeg[l.to_module] += 1;
    }
    let mut queue: std::collections::VecDeque<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    let mut order: Vec<usize> = Vec::new();
    while let Some(v) = queue.pop_front() {
        order.push(v);
        for &w in &adj[v] {
            indeg[w] -= 1;
            if indeg[w] == 0 {
                queue.push_back(w);
            }
        }
    }

    let mut runtime_stages: Vec<RuntimeStage> = Vec::new();
    let mut graph = DataflowGraph::new(0, 0.0);
    let mut layer_of: Vec<Option<usize>> = vec![None; n];

    let mut adapter_before: std::collections::HashMap<(usize, usize), &HealedAdapter> = std::collections::HashMap::new();
    for (_, ad, tm, tp) in healed.iter() {
        adapter_before.insert((*tm, *tp), ad);
    }

    let producer_module = |links: &[PipelineLink], to: usize| -> usize {
        for l in links {
            if l.to_module == to {
                return l.from_module;
            }
        }
        usize::MAX
    };

    for &mi in &order {
        let m = &spec.modules[mi];
        if m.inputs.is_empty() {
            let w = m.outputs.first().map(|p| p.width).unwrap_or(0);
            if graph.num_inputs == 0 {
                graph.num_inputs = w;
            }
            layer_of[mi] = Some(0);
            continue;
        }
        if !m.jit_linear {
            runtime_stages.push(RuntimeStage {
                module: m.name.clone(),
                purpose: "нормировка ‖x‖→1 (данные-зависимая)".into(),
                width: m.inputs.first().map(|p| p.width).unwrap_or(0),
            });
            layer_of[mi] = layer_of[producer_module(&spec.links, mi)];
            continue;
        }
        if let Some(&(_, w)) = norm_stage_before.iter().find(|&&(tm, _)| tm == mi) {
            runtime_stages.push(RuntimeStage {
                module: format!("norm_before_{}", m.name),
                purpose: "нормировка ‖x‖→1 (данные-зависимая)".into(),
                width: w,
            });
        }
        let in_width = m.inputs.first().map(|p| p.width).unwrap_or(0);
        let mut cur_layer = layer_of[producer_module(&spec.links, mi)];
        let mut cur_width = in_width;
        if let Some(ad) = adapter_before.get(&(mi, 0)) {
            let out_w = ad.spec.outputs[0].width;
            let layer = graph.add_layer(out_w, ad.act, Reducer::Sum);
            for ((_, io), (_, oo), w) in &ad.spec.edges {
                match cur_layer {
                    Some(0) => graph.add_edge(0, *io, *oo, w.clone()),
                    Some(l) => graph.add_edge(l - 1, *io, *oo, w.clone()),
                    None => {}
                }
            }
            cur_layer = Some(layer);
            cur_width = out_w;
        }
        let out_w = m.outputs.first().map(|p| p.width).unwrap_or(0);
        let layer = graph.add_layer(out_w, NodeAct::Identity, Reducer::Sum);
        for ((_, io), (_, oo), w) in &m.edges {
            match cur_layer {
                Some(0) => graph.add_edge(0, *io, *oo, w.clone()),
                Some(l) => graph.add_edge(l - 1, *io, *oo, w.clone()),
                None => {}
            }
        }
        if graph.edges.iter().all(|e| e.src_layer + 1 != layer) && out_w == cur_width && out_w > 0 {
            for j in 0..out_w {
                match cur_layer {
                    Some(0) => graph.add_edge(0, j, j, WeightKind::F32(1.0)),
                    Some(l) => graph.add_edge(l - 1, j, j, WeightKind::F32(1.0)),
                    None => {}
                }
            }
        }
        layer_of[mi] = Some(layer);
        let _ = cur_width;
    }

    Ok(LinkedPipeline { graph, runtime_stages, report })
}

// ---------------------------------------------------------------------------
// Тесты — «голос» ловит подсаженные дефекты и лечит адаптерами
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn pipeline(modules: Vec<ModuleSpec>, links: Vec<PipelineLink>) -> PipelineSpec {
        PipelineSpec { modules, links }
    }

    fn link_of(a: usize, b: usize) -> PipelineLink {
        PipelineLink { from_module: a, from_port: 0, to_module: b, to_port: 0 }
    }

    #[test]
    fn test_good_pipeline_compiles_and_runs() {
        let mut layer = ModuleSpec::linear("dense1", 4, 3, Vec::new());
        layer.edges = vec![
            ((0, 0), (0, 0), WeightKind::F32(0.5)),
            ((0, 1), (0, 0), WeightKind::F32(-0.25)),
            ((0, 2), (0, 1), WeightKind::Trit { val: 1, scale: 1.0 }),
            ((0, 3), (0, 2), WeightKind::Trit { val: -1, scale: 1.0 }),
        ];
        let spec = pipeline(vec![ModuleSpec::source("src", 4), layer], vec![link_of(0, 1)]);
        let linked = link(&spec).expect("линковка");
        assert_eq!(linked.report.errors, 0);
        assert_eq!(linked.report.healed, 0);
        assert_eq!(linked.graph.num_inputs, 4);
        assert_eq!(linked.graph.layers.len(), 1);
        let c = crate::graph::graph_asm::DataflowCompiler::compile_x86_64(&linked.graph, "causal_ok").unwrap();
        let k = crate::triune::jit_loop::ExecutableKernel::load(&c.machine_bytes).unwrap();
        let x = [0.7f32, -1.2, 0.9, 0.3];
        let mut y = [0.0f32; 3];
        unsafe { k.call_raw(x.as_ptr(), y.as_mut_ptr()) };
        let r = crate::graph::graph_asm::dataflow_reference_eval(&linked.graph, &x).unwrap();
        for (a, b) in r.iter().zip(&y) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    #[test]
    fn test_defect_width_healed_by_expander() {
        // Прямая цитата директивы: «стык 3→4: ожидалось 27 тритов, пришло 9»
        let mut nine = ModuleSpec::linear("trit_src", 9, 9, Vec::new());
        for j in 0..9 {
            nine.edges.push(((0, j), (0, j), WeightKind::Trit { val: if j % 2 == 0 { 1 } else { -1 }, scale: 1.0 }));
        }
        nine.outputs[0].kind = PortKind::TritVector { depth: 3 };
        nine.inputs[0].kind = PortKind::TritVector { depth: 3 };
        let mut twenty7 = ModuleSpec::linear("consumer27", 27, 4, Vec::new());
        for j in 0..27 {
            twenty7.edges.push(((0, j), (0, j % 4), WeightKind::F32(0.1)));
        }
        let spec = pipeline(
            vec![ModuleSpec::source("src", 9), nine, twenty7],
            vec![link_of(0, 1), link_of(1, 2)],
        );
        let linked = link(&spec).expect("автопочинка расширителем");
        assert_eq!(linked.report.errors, 0);
        assert!(linked.report.healed >= 1);
        assert!(linked.report.diagnostics.iter().any(|d| d.message.contains("пришло 9 значений, ждёт 27")));
        // слои: трит-слой(9) → расширитель(27) → потребитель(4)
        assert_eq!(linked.graph.layers.iter().map(|l| l.width).collect::<Vec<_>>(), vec![9, 9, 27, 4]);
        let c = crate::graph::graph_asm::DataflowCompiler::compile_x86_64(&linked.graph, "exp").unwrap();
        let k = crate::triune::jit_loop::ExecutableKernel::load(&c.machine_bytes).unwrap();
        let x: Vec<f32> = (1..=9).map(|i| i as f32 * 0.1).collect();
        let mut y = [0.0f32; 4];
        unsafe { k.call_raw(x.as_ptr(), y.as_mut_ptr()) };
        let r = crate::graph::graph_asm::dataflow_reference_eval(&linked.graph, &x).unwrap();
        for (a, b) in r.iter().zip(&y) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    #[test]
    fn test_defect_width_padded_by_vacuum() {
        let mut five = ModuleSpec::linear("five", 5, 5, Vec::new());
        for j in 0..5 {
            five.edges.push(((0, j), (0, j), WeightKind::F32(1.0)));
        }
        let mut eight = ModuleSpec::linear("eight", 8, 2, Vec::new());
        for j in 0..8 {
            eight.edges.push(((0, j), (0, j % 2), WeightKind::F32(0.2)));
        }
        let spec = pipeline(
            vec![ModuleSpec::source("src", 5), five, eight],
            vec![link_of(0, 1), link_of(1, 2)],
        );
        let linked = link(&spec).expect("паддинг");
        assert_eq!(linked.report.errors, 0);
        assert!(linked.report.healed >= 1);
        assert_eq!(linked.graph.layers.iter().map(|l| l.width).collect::<Vec<_>>(), vec![5, 8, 2]);
    }

    #[test]
    fn test_defect_kind_healed_by_kind_adapter() {
        let mut trit_out = ModuleSpec::linear("trit_layer", 6, 6, Vec::new());
        for j in 0..6 {
            trit_out.edges.push(((0, j), (0, j), WeightKind::Trit { val: 1, scale: 1.0 }));
        }
        trit_out.outputs[0].kind = PortKind::TritVector { depth: 5 };
        trit_out.inputs[0].kind = PortKind::TritVector { depth: 5 };
        let mut consumer = ModuleSpec::linear("f32_consumer", 6, 2, Vec::new());
        for j in 0..6 {
            consumer.edges.push(((0, j), (0, j % 2), WeightKind::F32(0.3)));
        }
        let spec = pipeline(
            vec![ModuleSpec::source("src", 6), trit_out, consumer],
            vec![link_of(0, 1), link_of(1, 2)],
        );
        let linked = link(&spec).expect("kind-адаптер");
        assert_eq!(linked.report.errors, 0);
        assert!(linked.report.diagnostics.iter().any(|d| d.fix.as_deref().unwrap_or("").contains("KindAdapter")));
    }

    #[test]
    fn test_defect_f32_to_trit_healed_by_lifter() {
        let mut consumer = ModuleSpec::linear("trit_consumer", 6, 2, Vec::new());
        consumer.inputs[0].kind = PortKind::TritVector { depth: 3 };
        for j in 0..6 {
            consumer.edges.push(((0, j), (0, j % 2), WeightKind::F32(0.3)));
        }
        let spec = pipeline(
            vec![ModuleSpec::source("src", 6), consumer],
            vec![link_of(0, 1)],
        );
        let linked = link(&spec).expect("lifter");
        assert_eq!(linked.report.errors, 0);
        assert!(linked.report.diagnostics.iter().any(|d| d.fix.as_deref().unwrap_or("").contains("TritLifter")));
        // активация SignTrit реальна в графе
        assert!(linked.graph.layers[0].act == NodeAct::SignTrit);
    }

    #[test]
    fn test_defect_norm_healed_by_runtime_stage() {
        let mut src_layer = ModuleSpec::linear("src_layer", 4, 4, Vec::new());
        for j in 0..4 {
            src_layer.edges.push(((0, j), (0, j), WeightKind::F32(0.5)));
        }
        let mut consumer = ModuleSpec::linear("needs_unit_norm", 4, 2, Vec::new());
        consumer.inputs[0].norm_unit = true;
        for j in 0..4 {
            consumer.edges.push(((0, j), (0, j % 2), WeightKind::F32(0.25)));
        }
        let spec = pipeline(
            vec![ModuleSpec::source("src", 4), src_layer, consumer],
            vec![link_of(0, 1), link_of(1, 2)],
        );
        let linked = link(&spec).expect("нормировка");
        assert_eq!(linked.report.errors, 0);
        assert!(linked.report.diagnostics.iter().any(|d| d.fix.as_deref().unwrap_or("").contains("нормировки")));
        assert!(!linked.runtime_stages.is_empty());
    }

    #[test]
    fn test_defect_trit_depth_loss_fails() {
        let mut deep = ModuleSpec::linear("deep5", 6, 6, Vec::new());
        for j in 0..6 {
            deep.edges.push(((0, j), (0, j), WeightKind::F32(1.0)));
        }
        deep.outputs[0].kind = PortKind::TritVector { depth: 3 };
        deep.inputs[0].kind = PortKind::TritVector { depth: 3 };
        let mut shallow = ModuleSpec::linear("wants5", 6, 2, Vec::new());
        shallow.inputs[0].kind = PortKind::TritVector { depth: 5 };
        for j in 0..6 {
            shallow.edges.push(((0, j), (0, j % 2), WeightKind::F32(0.1)));
        }
        let spec = pipeline(
            vec![ModuleSpec::source("src", 6), deep, shallow],
            vec![link_of(0, 1), link_of(1, 2)],
        );
        let err = link(&spec).err().expect("должен отклонить: глубина падает");
        assert!(err.diagnostics.iter().any(|d| d.message.contains("трит-глубина")));
    }

    #[test]
    fn test_defect_wx_violation_fails() {
        let mut bad = ModuleSpec::linear("wx_offender", 4, 4, Vec::new());
        for j in 0..4 {
            bad.edges.push(((0, j), (0, j), WeightKind::F32(1.0)));
        }
        bad.wx_compliant = false;
        let spec = pipeline(vec![ModuleSpec::source("src", 4), bad], vec![link_of(0, 1)]);
        let err = link(&spec).err().expect("W^X обязателен");
        assert!(err.diagnostics.iter().any(|d| d.message.contains("W^X")));
    }

    #[test]
    fn test_defect_parasitic_allocation_warns() {
        let mut alloc = ModuleSpec::linear("allocator", 4, 4, Vec::new());
        for j in 0..4 {
            alloc.edges.push(((0, j), (0, j), WeightKind::F32(1.0)));
        }
        alloc.hot_allocations = 3;
        let spec = pipeline(vec![ModuleSpec::source("src", 4), alloc], vec![link_of(0, 1)]);
        let linked = link(&spec).expect("аллокации — не фатал");
        assert!(linked.report.diagnostics.iter().any(|d| d.message.contains("аллокаций в горячем пути")));
    }

    #[test]
    fn test_defect_cycle_fails() {
        let a = ModuleSpec::linear("a", 4, 4, vec![((0, 0), (0, 0), WeightKind::F32(1.0))]);
        let b = ModuleSpec::linear("b", 4, 4, vec![((0, 0), (0, 0), WeightKind::F32(1.0))]);
        let spec = pipeline(vec![a, b], vec![link_of(0, 1), link_of(1, 0)]);
        let err = link(&spec).err().expect("цикл недопустим");
        assert!(err.diagnostics.iter().any(|d| d.message.contains("цикл")));
    }

    #[test]
    fn test_defect_dangling_input_fails() {
        let a = ModuleSpec::linear("a", 4, 4, vec![((0, 0), (0, 0), WeightKind::F32(1.0))]);
        let b = ModuleSpec::linear("b", 4, 4, vec![((0, 0), (0, 0), WeightKind::F32(1.0))]);
        let spec = pipeline(vec![a, b], vec![]);
        let err = link(&spec).err().expect("висячие входы");
        assert!(err.diagnostics.iter().any(|d| d.message.contains("не подключён")));
    }

    #[test]
    fn test_defect_duplicate_producer_fails() {
        let s1 = ModuleSpec::source("s1", 4);
        let s2 = ModuleSpec::source("s2", 4);
        let c = ModuleSpec::linear("c", 4, 4, vec![((0, 0), (0, 0), WeightKind::F32(1.0))]);
        let spec = pipeline(vec![s1, s2, c], vec![link_of(0, 2), link_of(1, 2)]);
        let err = link(&spec).err().expect("один продюсер на вход");
        assert!(err.diagnostics.iter().any(|d| d.message.contains("второго продюсера")));
    }

    #[test]
    fn test_speech_is_natural_language() {
        let mut alloc = ModuleSpec::linear("allocator", 4, 4, Vec::new());
        for j in 0..4 {
            alloc.edges.push(((0, j), (0, j), WeightKind::F32(1.0)));
        }
        alloc.hot_allocations = 2;
        let spec = pipeline(vec![ModuleSpec::source("src", 4), alloc], vec![link_of(0, 1)]);
        let linked = link(&spec).unwrap();
        let s = linked.report.speech();
        assert!(s.contains("Проверено стыков"));
        assert!(s.contains("Вердикт"));
        assert!(s.contains("аллокаций в горячем пути"));
    }
}
