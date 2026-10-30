//! MCP-сервер (Model Context Protocol) поверх stdio: poler-engine как
//! нативный инструмент LLM-агентов — без обёрток, без прокладок,
//! один бинарь `poler-engine --mcp`.
//!
//! Транспорт: JSON-RPC 2.0, одно сообщение — одна строка (stdio-транспорт
//! MCP). Протокольные сообщения ТОЛЬКО в stdout; вся диагностика — stderr
//! (иначе клиент парсит мусор).
//!
//! Инструменты (агент сам выбирает цели — ни сайт, ни тема не фиксированы):
//!
//! * `poler_web_search` — поиск по постоянному веб-индексу (WebRank:
//!   BM25 + PageRank + title + ε-плотность, кириллический стемминг);
//! * `poler_crawl`      — обход выбранного агентом сайта в постоянный
//!   индекс (robots.txt, sitemap, SimHash-дедуп, PageRank, инкрементально);
//! * `poler_fetch`      — открыть любой URL в реальном Chromium (CDP):
//!   JS/SPA/Shadow DOM рендерятся, скрытые JSON API перехватываются;
//! * `poler_search`     — локальный резонансный POLER-поиск (ε/R/сцены);
//! * `poler_box_exec` / `poler_box_status` — контейнерный jail (gateway).
//!
//! v2.0 (sovereign stack): poler_gmail / poler_drive / poler_nlm удалены
//! вместе с Google/NotebookLM-интеграциями.
//!
//! C2/v0.33.0 «Живая муха»: семейство poler_fly_* — резидентный
//! коннектом FLYCSR1 (мозг мухи FlyWire v783) как матрица A: паспорт
//! нейрона, ребро/ротор, K-hop, кратчайший путь, общие партнёры,
//! центральность/PageRank, глобальный топ циркуляции J, мотивы и
//! симуляция распространения сигнала. Артефакт грузится ОДИН раз
//! и живёт в RAM между вызовами (WarmFly); тяжёлые запросы уходят
//! в пул воркеров — агент может допросить муху параллельно.
//!
//! Chromium поднимается автоматически при первом poler_crawl/poler_fetch
//! (см. `web::ensure_chromium`).

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use crate::graph::connectome::{Connectome, ConnectomeNodes, InEdges, SignFilter};
use crate::graph::flyops::Direction;
use crate::literary::{
    flybridge::build_cast, LiteraryEngine, LiteraryParams, NarrativeReport, PsiMode,
    StepTelemetry,
};
use crate::web::{self, CdpFetcher, WebIndex};
use crate::{Engine, EngineConfig};

/// Cap рендер-текста в ответе poler_fetch: больше — бессмысленно для
/// контекстного окна агента (полный текст лежит в кэш-файле).
const FETCH_TEXT_CAP: usize = 24 * 1024;

// ---------------------------------------------------------------------------
// Аудит-фикс v0.21.0 (security hardening): ограждение MCP-инструментов.
// Угроза: токен Bearer (туннель/логи/конфиг) даёт УДАЛЁННОМУ держателю
// читать ЛЮБЫЕ локальные файлы (секреты, ключи, конфиги) и ходить на
// внутренние адреса (SSRF).
// ---------------------------------------------------------------------------

/// Чистая (без env) проверка: лежит ли канонический `path` внутри одного из
/// `roots`? Несуществующий путь — разрешён (ошибку отчётит сам инструмент,
/// канонизации нет). Символические ссылки резолвятся — escape через symlink
/// невозможен.
pub fn path_allowed_under(path: &str, roots: &[PathBuf]) -> bool {
    let Ok(target) = std::fs::canonicalize(path) else {
        return true; // нет файла — пусть инструмент честно скажет «не найден»
    };
    roots
        .iter()
        .filter_map(|r| std::fs::canonicalize(r).ok())
        .any(|r| target.starts_with(r))
}

/// Разрешён ли локальный путь для poler_grep / poler_chunk / poler_search
/// по умолчанию: рабочая директория, веб-кэш и POLER_MCP_EXTRA_ROOTS.
/// `POLER_MCP_ALLOW_ANY_PATH=1` возвращает прежнее поведение (доверенный
/// локальный агент).
pub fn mcp_path_allowed(path: &str) -> bool {
    if std::env::var("POLER_MCP_ALLOW_ANY_PATH")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        return true;
    }
    let mut roots: Vec<PathBuf> = vec![
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        web::web_cache_dir(),
    ];
    if let Ok(extra) = std::env::var("POLER_MCP_EXTRA_ROOTS") {
        for r in extra.split(':').filter(|s| !s.is_empty()) {
            roots.push(PathBuf::from(r));
        }
    }
    path_allowed_under(path, &roots)
}

/// Чистая (без env) проверка хоста: блокируемый ли это адрес?
/// Link-local 169.254.0.0/16 (вкл. cloud-metadata 169.254.169.254),
/// 0.0.0.0/8, IPv6 fe80::/10 (link-local), fc00::/7 (ULA), ::
/// и известные имена metadata-сервисов. RFC1918 и loopback НЕ блокируем:
/// WebLens и локальная разработка ходят на 127.0.0.1.
pub fn host_blocked_by_default(host: &str) -> bool {
    const META_HOSTS: [&str; 3] = ["metadata.google.internal", "metadata.goog", "instance-data"];
    let h = host.trim().trim_matches(|c| c == '[' || c == ']');
    if META_HOSTS.contains(&h.to_ascii_lowercase().as_str()) {
        return true;
    }
    if let Ok(ip) = h.parse::<std::net::Ipv4Addr>() {
        let o = ip.octets();
        return (o[0] == 169 && o[1] == 254) || o[0] == 0;
    }
    if let Ok(ip) = h.parse::<std::net::Ipv6Addr>() {
        let s = ip.segments();
        return (s[0] & 0xffc0) == 0xfe80 || (s[0] & 0xfe00) == 0xfc00 || ip.is_unspecified();
    }
    false // обычные домены — решение за Chromium (резолв и запрос)
}

/// Анти-SSRF-гейт для poler_fetch / poler_crawl. Отключение (наш собственный
/// агент на доверенной машине): `POLER_MCP_ALLOW_PRIVATE_NET=1`.
pub fn mcp_url_allowed(url: &str) -> Result<(), String> {
    if std::env::var("POLER_MCP_ALLOW_PRIVATE_NET")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        return Ok(());
    }
    let host = url
        .split_once("://")
        .map(|(_, r)| r)
        .unwrap_or(url)
        .split(['/', '?'])
        .next()
        .unwrap_or("")
        .to_string();
    // порт отрезаем только если он есть (rsplit_once без ':' даёт None)
    let host = host
        .rsplit_once(':')
        .map(|(h, _)| h.to_string())
        .unwrap_or(host);
    if host_blocked_by_default(&host) {
        return Err(format!(
            "SSRF-ограждение: хост {host} (link-local/metadata) заблокирован; \
             POLER_MCP_ALLOW_PRIVATE_NET=1 снимает ограничение"
        ));
    }
    Ok(())
}

/// Ограждение локального пути (общая ошибка для инструментов чтения файлов).
fn guard_path(path: &str) -> Result<(), String> {
    if mcp_path_allowed(path) {
        return Ok(());
    }
    Err(format!(
        "путь вне разрешённых корней MCP (cwd, веб-кэш, POLER_MCP_EXTRA_ROOTS); \
         POLER_MCP_ALLOW_ANY_PATH=1 снимает ограничение: {path}"
    ))
}

pub struct McpServer {
    cdp_port: u16,
    wait_ms: u64,
    db_path: PathBuf,
    /// БД знаний Суверенного Гиппокампа (v0.29): отдельная от веб-индекса,
    /// строится `--knowledge-ingest`.
    knowledge_db: PathBuf,
    // ── M6: резидентное (тёплое) состояние ──────────────────────────
    /// Открытый веб-индекс — переиспользуется между вызовами
    /// (холодный старт открывал SQLite на КАЖДЫЙ web_search/crawl).
    warm_web: Mutex<Option<WebIndex>>,
    /// Тёплый Гиппокамп: SQLite + RaBitQ + HNSW + эмбеддер.
    warm_knowledge: Mutex<Option<crate::sources::knowledge::WarmKnowledge>>,
    /// RAM-кэш файлов для poler_grep (mtime-инвалидация, LRU).
    file_cache: crate::retrieval::filecache::FileCache,
    /// Шифропоток журнала операций (стриминг логов в Vault, --vault-log).
    #[cfg(feature = "pnd-ffi")]
    vault_log: Option<Arc<Mutex<crate::crypto::vault::VaultAppender>>>,
    /// E2/v0.32.0: реестр фоновых задач poler_exec_async (отмена/опрос).
    #[cfg(feature = "pnd-ffi")]
    exec_tasks: Arc<crate::exec::TaskRegistry>,
    /// C2/v0.33.0: резидентная «живая муха» — коннектом + CSC в RAM,
    /// переиспользуется между вызовами poler_fly_* (холодный старт был
    /// ~60–300 мс загрузки + ~43 мс CSC на КАЖДЫЙ CLI-вызов агента).
    warm_fly: Mutex<Option<Arc<WarmFly>>>,
    /// L1/v0.34.0: резидентные сессии Литературного Двигателя POLER[Ψ]
    /// — латентное состояние p_t, эхо и мушиная калибровка живут между
    /// вызовами poler_literary_step (агент ведёт нарратив шаг за шагом).
    warm_psi: Mutex<PsiSessions>,
    /// S1/v0.35.0: резидентные живые мозги SSN (Синаптический Вихрь) —
    /// состояние сети, веса, медиаторы и тон живут между вызовами
    /// poler_ssn_step/inject/status (агент ведёт мозг непрерывно).
    warm_ssn: Mutex<SsnSessions>,
    /// S2/v0.36.0: резидентные Триединства (муха + вихрь + кристалл) —
    /// говорящие сессии: мозг, пульс мухи и контекст речи живут между
    /// вызовами poler_triune_speak/state.
    warm_triune: Mutex<TriuneSessions>,
    /// v0.48.0: резидентный Калькулятор Всего — переменные/ans/история
    /// живут между вызовами poler_calc (агент ведёт сквозные вычисления).
    warm_calc: Mutex<crate::calc::CalcState>,
}

impl McpServer {
    /// Конструктор для альтернативных транспортов (mcp_http: Streamable HTTP).
    pub fn new(cdp_port: u16, wait_ms: u64, db_path: PathBuf) -> Self {
        Self {
            cdp_port,
            wait_ms,
            db_path,
            knowledge_db: crate::sources::knowledge::default_db_path(),
            warm_web: Mutex::new(None),
            warm_knowledge: Mutex::new(None),
            file_cache: crate::retrieval::filecache::FileCache::new(DEFAULT_RAM_BUDGET),
            #[cfg(feature = "pnd-ffi")]
            vault_log: None,
            #[cfg(feature = "pnd-ffi")]
            exec_tasks: Arc::new(crate::exec::TaskRegistry::default()),
            warm_fly: Mutex::new(None),
            warm_psi: Mutex::new(PsiSessions::default()),
            warm_ssn: Mutex::new(SsnSessions::default()),
            warm_triune: Mutex::new(TriuneSessions::default()),
            warm_calc: Mutex::new(crate::calc::CalcState::new()),
        }
    }

    /// Переопределить БД знаний (Суверенный Гиппокамп, v0.29).
    pub fn with_knowledge_db(mut self, path: PathBuf) -> Self {
        self.knowledge_db = path;
        self
    }

    /// M6: RAM-бюджет файл-кэша grep (байт).
    pub fn with_ram_budget(mut self, bytes: usize) -> Self {
        self.file_cache = crate::retrieval::filecache::FileCache::new(bytes);
        self
    }

    /// M6: подключить шифропоток журнала операций (Vault .pvt).
    #[cfg(feature = "pnd-ffi")]
    pub fn with_vault_log(mut self, app: crate::crypto::vault::VaultAppender) -> Self {
        self.vault_log = Some(Arc::new(Mutex::new(app)));
        self
    }

    /// M6: снапшот статистики резидентного состояния (бенч/диагностика).
    pub fn resident_stats(&self) -> serde_json::Value {
        let cs = self.file_cache.stats();
        let warm_k = self.warm_knowledge.lock().map(|g| g.is_some()).unwrap_or(false);
        let warm_w = self.warm_web.lock().map(|g| g.is_some()).unwrap_or(false);
        serde_json::json!({
            "file_cache": {
                "files": cs.files,
                "bytes": cs.bytes,
                "hits": cs.hits,
                "misses": cs.misses,
                "invalidated": cs.invalidated,
                "evictions": cs.evictions,
                "budget_bytes": self.file_cache.cap_bytes(),
            },
            "warm_knowledge_open": warm_k,
            "warm_web_open": warm_w,
        })
    }

    /// M6: выполнить операцию над тёплым веб-индексом (get-or-open).
    fn with_web_index<T>(
        &self,
        f: impl FnOnce(&mut WebIndex) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut guard = self.warm_web.lock().expect("poler-mcp: отравленный веб-лок");
        if guard.is_none() {
            *guard = Some(WebIndex::open(&self.db_path).map_err(|e| e.to_string())?);
        }
        f(guard.as_mut().expect("только что открыли"))
    }

    /// M6: событие журнала — шифропоток Vault (+ мягкий коммит):
    /// {"ts":мс,"method":"tools/call","tool":…,"us":мкс,"ok":bool}.
    /// Ошибка журнала НЕ роняет запрос — лог не может ломать сервис.
    #[cfg(feature = "pnd-ffi")]
    fn log_event(&self, method: &str, tool: Option<&str>, us: u64, ok: bool) {
        let Some(vl) = &self.vault_log else { return };
        let Ok(mut g) = vl.lock() else { return };
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let tool_js = match tool {
            Some(t) => format!(",\"tool\":\"{t}\""),
            None => String::new(),
        };
        let _ = g.append_line(&format!(
            "{{\"ts\":{ts},\"method\":\"{method}\"{tool_js},\"us\":{us},\"ok\":{}}}",
            ok
        ));
        let _ = g.commit_soft();
    }

    #[cfg(not(feature = "pnd-ffi"))]
    fn log_event(&self, _method: &str, _tool: Option<&str>, _us: u64, _ok: bool) {}
}

/// RAM-бюджет файл-кэша по умолчанию (48 МиБ — в рамках лимита M6 ≤64 МиБ).
pub const DEFAULT_RAM_BUDGET: usize = 48 * 1024 * 1024;

/// M6: опции резидентного MCP-сервера (общие для stdio и HTTP).
#[derive(Default)]
pub struct McpServerOptions {
    /// RAM-бюджет файл-кэша grep (байт; 0 = default 48 МиБ).
    pub ram_budget: usize,
    /// Шифропоток журнала операций (Vault .pvt, --vault-log).
    #[cfg(feature = "pnd-ffi")]
    pub vault_log: Option<crate::crypto::vault::VaultAppender>,
}

impl McpServerOptions {
    /// Применить к серверу (builder-стиль).
    pub fn apply(self, srv: McpServer) -> McpServer {
        let mut srv = srv;
        if self.ram_budget > 0 {
            srv = srv.with_ram_budget(self.ram_budget);
        }
        #[cfg(feature = "pnd-ffi")]
        if let Some(app) = self.vault_log {
            srv = srv.with_vault_log(app);
        }
        srv
    }
}

/// E2/v0.32.0: инструменты исполнителя уходят в пул воркеров — агент может
/// запускать команды ПАРАЛЛЕЛЬНО (в E1 один poler_exec блокировал весь
/// stdio-цикл: 20 стресс-тестов вставали в очередь).
#[cfg(feature = "pnd-ffi")]
fn is_exec_tool(msg: &Value) -> bool {
    msg.get("method").and_then(|m| m.as_str()) == Some("tools/call")
        && matches!(
            msg.pointer("/params/name").and_then(|v| v.as_str()),
            Some(
                "poler_exec"
                    | "poler_exec_async"
                    | "poler_exec_task"
                    | "poler_exec_kill"
                    | "poler_exec_list"
            )
        )
}

/// C2/v0.33.0: семейство «живой мухи» уходит в пул воркеров — PageRank
/// полного коннектома и симуляция занимают сотни миллисекунд, а агент
/// часто допросает муху пачкой независимых запросов.
fn is_fly_tool(msg: &Value) -> bool {
    msg.get("method").and_then(|m| m.as_str()) == Some("tools/call")
        && matches!(
            msg.pointer("/params/name").and_then(|v| v.as_str()),
            Some(
                "poler_fly"
                    | "poler_fly_node"
                    | "poler_fly_edge"
                    | "poler_fly_khop"
                    | "poler_fly_path"
                    | "poler_fly_common"
                    | "poler_fly_centrality"
                    | "poler_fly_rotor"
                    | "poler_fly_motifs"
                    | "poler_fly_propagate"
            )
        )
}

/// L1/v0.34.0: семейство Литературного Двигателя — в пул воркеров:
/// полная генерация с мушиной калибровкой (загрузка коннектома +
/// каста + 48 шагов) занимает десятки/сотни миллисекунд, а запросы
/// step/generate независимы между сессиями.
fn is_psi_tool(msg: &Value) -> bool {
    msg.get("method").and_then(|m| m.as_str()) == Some("tools/call")
        && matches!(
            msg.pointer("/params/name").and_then(|v| v.as_str()),
            Some(
                "poler_literary_field"
                    | "poler_literary_step"
                    | "poler_literary_generate"
                    | "poler_literary_eject"
            )
        )
}

/// S1/v0.35.0: инструменты poler_ssn_* держат резидентные мозги SSN
/// (общее мутабельное состояние сессий) — как psi-семейство, маршрутизируются
/// на последовательный исполнитель, а не в параллельный пул.
fn is_ssn_tool(msg: &Value) -> bool {
    msg.get("method").and_then(|m| m.as_str()) == Some("tools/call")
        && matches!(
            msg.pointer("/params/name").and_then(|v| v.as_str()),
            Some(
                "poler_ssn_step"
                    | "poler_ssn_inject"
                    | "poler_ssn_status"
                    | "poler_ssn_eject"
            )
        )
}

/// v0.48.0: Калькулятор Всего — в пул воркеров (числовой solve может
/// сканировать диапазон ±100; состояние за Mutex — параллельно безопасно).
/// v0.51.0: poler_quantum — туда же (GHZ-1024 ≈ 367 мс; верификатор —
/// чистые вычисления без общего состояния).
fn is_calc_tool(msg: &Value) -> bool {
    msg.get("method").and_then(|m| m.as_str()) == Some("tools/call")
        && matches!(
            msg.pointer("/params/name").and_then(|v| v.as_str()),
            Some("poler_calc" | "poler_hw" | "poler_quantum" | "poler_scivoice")
        )
}

/// S2/v0.36.0: инструменты poler_triune_* держат резидентные говорящие
/// Триединства (муха + вихрь + кристалл) — последовательный исполнитель.
fn is_triune_tool(msg: &Value) -> bool {
    msg.get("method").and_then(|m| m.as_str()) == Some("tools/call")
        && matches!(
            msg.pointer("/params/name").and_then(|v| v.as_str()),
            Some("poler_triune_speak" | "poler_triune_state")
        )
}

/// Резидентная «живая муха»: коннектом FLYCSR1 + таблица root_id + CSC
/// (входящие рёбра) в RAM. CSC строится лениво один раз — первый
/// impact/центральность запрос платит ~43 мс, остальные — ноль.
pub struct WarmFly {
    /// Сам коннектом (CSR в RAM).
    con: Arc<Connectome>,
    /// Таблица 138 639 root_id (опциональна — для перевода индекс ↔ FlyWire).
    nodes: Option<ConnectomeNodes>,
    /// Путь загруженного артефакта (сверка при повторном csr).
    csr_path: PathBuf,
    /// Время загрузки артефакта, мс.
    load_ms: u128,
    /// CSC-транспонирование (ленивое, один раз).
    csc: std::sync::OnceLock<InEdges>,
}

impl WarmFly {
    /// CSC «кто управляет нейроном» — строится при первом обращении.
    fn csc(&self) -> &InEdges {
        self.csc.get_or_init(|| self.con.build_in_edges())
    }

    /// Построен ли уже CSC (диагностика теплоты).
    fn csc_ready(&self) -> bool {
        self.csc.get().is_some()
    }
}

/// Округление f32 до 6 знаков для JSON-телеметрии Ψ (в fly-семействе
/// аналогичный round6 — локальное замыкание).
fn psi_round(x: f32) -> f64 {
    ((x as f64) * 1e6).round() / 1e6
}

/// S1/v0.35.0: округление f64 до 6 знаков (SSN-телеметрия — двойная точность).
fn ssn_round(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// Резидентные сессии POLER[Ψ]: LRU-очередь + карта двигателей.
/// Лимит 8 — агенты редко ведут больше пары нарративов одновременно,
/// а каждая сессия держит фазовое состояние + эхо (десятки КБ).
#[derive(Default)]
struct PsiSessions {
    /// Живые сессии: id → двигатель.
    map: std::collections::HashMap<String, Arc<Mutex<LiteraryEngine>>>,
    /// Порядок создания (LRU-вытеснение самых старых).
    order: std::collections::VecDeque<String>,
    /// Счётчик для детерминированных id внутри процесса.
    counter: u64,
}

impl PsiSessions {
    const CAP: usize = 8;

    /// Зарегистрировать новую сессию (с вытеснением старых за лимитом).
    fn insert(&mut self, eng: LiteraryEngine) -> (String, Arc<Mutex<LiteraryEngine>>) {
        self.counter += 1;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let id = format!(
            "psi-{:x}-{:x}",
            self.counter,
            crate::literary::qualia::fnv1a64(&nanos.to_le_bytes()) & 0xffff
        );
        let arc = Arc::new(Mutex::new(eng));
        self.order.push_back(id.clone());
        self.map.insert(id.clone(), arc.clone());
        while self.map.len() > Self::CAP {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
        (id, arc)
    }

    /// Сессия по id (и её позиция в LRU — для диагностики).
    fn get(&self, id: &str) -> Option<Arc<Mutex<LiteraryEngine>>> {
        self.map.get(id).cloned()
    }

    /// Число живых сессий.
    fn len(&self) -> usize {
        self.map.len()
    }

    /// Выгрузить сессию (или все — id = "all"). Возвращает число выгруженных.
    fn eject(&mut self, id: &str) -> usize {
        if id == "all" || id == "все" {
            let n = self.map.len();
            self.map.clear();
            self.order.clear();
            return n;
        }
        if self.map.remove(id).is_some() {
            self.order.retain(|x| x != id);
            1
        } else {
            0
        }
    }
}

/// S1/v0.35.0: резидентные живые мозги SSN — LRU-очередь + карта.
/// Лимит 8: каждый мозг = N×F весов + фазы (600×16×2×8 Б ≈ 150 КБ).
#[derive(Default)]
struct SsnSessions {
    map: std::collections::HashMap<String, Arc<Mutex<crate::ssn::SsnEngine>>>,
    order: std::collections::VecDeque<String>,
    counter: u64,
}

impl SsnSessions {
    const CAP: usize = 8;

    fn insert(&mut self, eng: crate::ssn::SsnEngine) -> (String, Arc<Mutex<crate::ssn::SsnEngine>>) {
        self.counter += 1;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let id = format!(
            "ssn-{:x}-{:x}",
            self.counter,
            crate::literary::qualia::fnv1a64(&nanos.to_le_bytes()) & 0xffff
        );
        let arc = Arc::new(Mutex::new(eng));
        self.order.push_back(id.clone());
        self.map.insert(id.clone(), arc.clone());
        while self.map.len() > Self::CAP {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
        (id, arc)
    }

    fn get(&self, id: &str) -> Option<Arc<Mutex<crate::ssn::SsnEngine>>> {
        self.map.get(id).cloned()
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    fn eject(&mut self, id: &str) -> usize {
        if id == "all" || id == "все" {
            let n = self.map.len();
            self.map.clear();
            self.order.clear();
            return n;
        }
        if self.map.remove(id).is_some() {
            self.order.retain(|x| x != id);
            1
        } else {
            0
        }
    }
}

/// S2/v0.36.0: одна резидентная говорящая сессия Триединства.
struct TriuneSession {
    core: crate::triune::TriuneCore,
    /// Последняя сказанная фраза (для poler_triune_state).
    last_text: String,
    last_tokens: usize,
}

/// S2/v0.36.0: резидентные Триединства — LRU-очередь + карта.
/// Лимит 4: каждая сессия = мозг (~150 КБ) + пульс мухи (КБ) + кристалл
/// (~30 КБ) — говорящие организмы тяжеловаты, но их мало и надо.
#[derive(Default)]
struct TriuneSessions {
    map: std::collections::HashMap<String, Arc<Mutex<TriuneSession>>>,
    order: std::collections::VecDeque<String>,
    counter: u64,
}

impl TriuneSessions {
    const CAP: usize = 4;

    fn insert(&mut self, core: crate::triune::TriuneCore) -> (String, Arc<Mutex<TriuneSession>>) {
        self.counter += 1;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let id = format!(
            "triune-{:x}-{:x}",
            self.counter,
            crate::literary::qualia::fnv1a64(&nanos.to_le_bytes()) & 0xffff
        );
        let arc = Arc::new(Mutex::new(TriuneSession {
            core,
            last_text: String::new(),
            last_tokens: 0,
        }));
        self.order.push_back(id.clone());
        self.map.insert(id.clone(), arc.clone());
        while self.map.len() > Self::CAP {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
        (id, arc)
    }

    fn get(&self, id: &str) -> Option<Arc<Mutex<TriuneSession>>> {
        self.map.get(id).cloned()
    }

    fn len(&self) -> usize {
        self.map.len()
    }
}

/// Загрузка таблицы root_id с проверкой соответствия коннектому.
fn fly_load_nodes(path: &str, con: &Connectome) -> Result<ConnectomeNodes, String> {
    let tbl = ConnectomeNodes::load(std::path::Path::new(path))?;
    if tbl.len() != con.n_nodes() {
        return Err(format!(
            "{} содержит {} root_id, а коннектом ждёт {} узлов",
            path,
            tbl.len(),
            con.n_nodes()
        ));
    }
    Ok(tbl)
}

/// Запуск MCP-сервера (stdio). Возвращает код процесса (0 = чистый EOF stdin).
/// E2: tools/call poler_exec* исполняются в пуле воркеров параллельно;
/// ответы пишутся по мере готовности (JSON-RPC допускает внеочередность —
/// клиент сопоставляет по id).
pub fn run(
    cdp_port: u16,
    wait_ms: u64,
    db_path: PathBuf,
    knowledge_db: Option<PathBuf>,
    opts: McpServerOptions,
) -> i32 {
    let mut server = McpServer::new(cdp_port, wait_ms, db_path);
    if let Some(kdb) = knowledge_db {
        server = server.with_knowledge_db(kdb);
    }
    server = opts.apply(server);
    let server = Arc::new(server);

    // Пул воркеров: перекладывает долгие tools/call (exec-семейство) из
    // читающего потока. Лок на recv держится только на время ОЖИДАНИЯ —
    // полученные задачи исполняются параллельно.
    let (tx, rx) = std::sync::mpsc::channel::<Box<dyn FnOnce() + Send + 'static>>();
    let rx = Arc::new(Mutex::new(rx));
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .clamp(2, 8);
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let rx = rx.clone();
        handles.push(std::thread::spawn(move || loop {
            let job = {
                let guard = rx.lock().expect("poler-mcp: отравленный лок пула");
                guard.recv()
            };
            let Ok(job) = job else { break };
            job();
        }));
    }

    eprintln!(
        "poler-mcp: stdio JSON-RPC, db={:?}, knowledge={:?}, cdp_port={}, wait_ms={}, workers={} (параллельный poler_exec)",
        server.db_path, server.knowledge_db, server.cdp_port, server.wait_ms, workers
    );
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                let _ = write_line(
                    &mut std::io::stdout().lock(),
                    &json!({"jsonrpc":"2.0","id":null,"error":{
                        "code":-32700,"message":format!("parse error: {e}")
                    }}),
                );
                continue;
            }
        };
        #[cfg(feature = "pnd-ffi")]
        if is_exec_tool(&msg)
            || is_fly_tool(&msg)
            || is_psi_tool(&msg)
            || is_ssn_tool(&msg)
            || is_triune_tool(&msg)
            || is_calc_tool(&msg)
        {
            let server = server.clone();
            let _ = tx.send(Box::new(move || {
                if let Some(resp) = server.dispatch(&msg) {
                    let _ = write_line(&mut std::io::stdout().lock(), &resp);
                }
            }));
            continue;
        }
        #[cfg(not(feature = "pnd-ffi"))]
        if is_fly_tool(&msg)
            || is_psi_tool(&msg)
            || is_ssn_tool(&msg)
            || is_triune_tool(&msg)
            || is_calc_tool(&msg)
        {
            let server = server.clone();
            let _ = tx.send(Box::new(move || {
                if let Some(resp) = server.dispatch(&msg) {
                    let _ = write_line(&mut std::io::stdout().lock(), &resp);
                }
            }));
            continue;
        }
        if let Some(resp) = server.dispatch(&msg) {
            let _ = write_line(&mut std::io::stdout().lock(), &resp);
        }
    }
    drop(tx);
    for h in handles {
        let _ = h.join();
    }
    0
}

fn write_line(out: &mut impl Write, v: &Value) -> std::io::Result<()> {
    out.write_all(serde_json::to_string(v).unwrap_or_default().as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()
}

impl McpServer {
    /// Диспетчер одного JSON-RPC-сообщения: возвращает ответ (None —
    /// уведомление без id, ответ не нужен). Общий для stdio и HTTP.
    /// M6: каждое сообщение таймируется и (если подключён --vault-log)
    /// попадает в шифропоток журнала с латентностью в микросекундах.
    pub fn dispatch(&self, msg: &Value) -> Option<Value> {
        let t0 = std::time::Instant::now();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
        let id = msg.get("id").cloned();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        // уведомления (без id) не требуют ответа
        let Some(id) = id else { return None };

        let result: Result<Value, (i64, String)> = match method.as_str() {
            "initialize" => Ok(self.handle_initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tools_manifest()})),
            "tools/call" => self.handle_tools_call(&params),
            "resources/list" => Ok(json!({"resources": []})),
            "resources/templates/list" => Ok(json!({"resourceTemplates": []})),
            "prompts/list" => Ok(json!({"prompts": []})),
            _ => Err((-32601, format!("method not found: {method}"))),
        };

        let ok = result.is_ok();
        let tool = params.get("name").and_then(|n| n.as_str()).map(str::to_string);
        let us = t0.elapsed().as_micros() as u64;
        self.log_event(&method, tool.as_deref(), us, ok);

        Some(match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((code, message)) => json!({
                "jsonrpc": "2.0", "id": id,
                "error": {"code": code, "message": message}
            }),
        })
    }

    fn handle_initialize(&self, params: &Value) -> Value {
        // отвечаем версией клиента, если она известна, иначе свежей
        let client_pv = params
            .get("protocolVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("2024-11-05");
        json!({
            "protocolVersion": client_pv,
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "poler-engine",
                "version": env!("CARGO_PKG_VERSION"),
                "title": "POLER Engine — AI-Native Resonant Web & Local Search"
            }
        })
    }

    fn handle_tools_call(&self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or((-32602, "params.name обязателен".into()))?;
        let args = params.get("arguments").cloned().unwrap_or(json!({}));
        let call = match name {
            "poler_web_search" => self.tool_web_search(&args),
            "poler_crawl" => self.tool_crawl(&args),
            "poler_fetch" => self.tool_fetch(&args),
            "poler_search" => self.tool_local_search(&args),
            "poler_grep" => self.tool_grep(&args),
            "poler_chunk" => self.tool_chunk(&args),
            #[cfg(feature = "pnd-ffi")]
            "poler_exec" => self.tool_exec(&args),
            #[cfg(feature = "pnd-ffi")]
            "poler_exec_async" => self.tool_exec_async(&args),
            #[cfg(feature = "pnd-ffi")]
            "poler_exec_task" => self.tool_exec_task(&args),
            #[cfg(feature = "pnd-ffi")]
            "poler_exec_kill" => self.tool_exec_kill(&args),
            #[cfg(feature = "pnd-ffi")]
            "poler_exec_list" => self.tool_exec_list(&args),
            "poler_box_exec" => self.tool_box_exec(&args),
            "poler_box_status" => self.tool_box_status(&args),
            "query_poler_knowledge" => self.tool_knowledge_query(&args),
            "poler_fly" => self.tool_fly(&args),
            "poler_fly_node" => self.tool_fly_node(&args),
            "poler_fly_edge" => self.tool_fly_edge(&args),
            "poler_fly_khop" => self.tool_fly_khop(&args),
            "poler_fly_path" => self.tool_fly_path(&args),
            "poler_fly_common" => self.tool_fly_common(&args),
            "poler_fly_centrality" => self.tool_fly_centrality(&args),
            "poler_fly_rotor" => self.tool_fly_rotor(&args),
            "poler_fly_motifs" => self.tool_fly_motifs(&args),
            "poler_fly_propagate" => self.tool_fly_propagate(&args),
            "poler_literary_field" => self.tool_literary_field(&args),
            "poler_literary_step" => self.tool_literary_step(&args),
            "poler_literary_generate" => self.tool_literary_generate(&args),
            "poler_literary_eject" => self.tool_literary_eject(&args),
            "poler_ssn_step" => self.tool_ssn_step(&args),
            "poler_ssn_inject" => self.tool_ssn_inject(&args),
            "poler_ssn_status" => self.tool_ssn_status(&args),
            "poler_ssn_eject" => self.tool_ssn_eject(&args),
            "poler_triune_speak" => self.tool_triune_speak(&args),
            "poler_triune_state" => self.tool_triune_state(&args),
            "poler_calc" => self.tool_calc(&args),
            "poler_hw" => self.tool_hw(&args),
            "poler_quantum" => self.tool_quantum(&args),
            "poler_scivoice" => self.tool_scivoice(&args),
            "poler_jit" => self.tool_jit(&args),
            other => Err(format!("неизвестный инструмент: {other}")),
        };
        match call {
            Ok(text) => Ok(json!({
                "content": [{"type": "text", "text": text}],
                "isError": false
            })),
            Err(e) => Ok(json!({
                "content": [{"type": "text", "text": format!("ошибка: {e}")}],
                "isError": true
            })),
        }
    }

    // -----------------------------------------------------------------
    // poler_web_search: запрос → постоянный индекс → WebRank
    // -----------------------------------------------------------------
    fn tool_web_search(&self, args: &Value) -> Result<String, String> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or("аргумент query (строка) обязателен")?;
        let top = args.get("top").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        let top = top.clamp(1, 50);
        // M6: тёплый веб-индекс — SQLite открыт резидентно.
        self.with_web_index(|ix| {
        if ix.page_count() == 0 {
            return Ok(format!(
                "Веб-индекс пуст ({:?}). Сначала вызови poler_crawl с seed_url — \
                 агент сам выбирает, какой сайт обходить.",
                self.db_path
            ));
        }
        let bridge = crate::retrieval::SemanticBridge::offline();
        let (hits, expansion) = ix
            .search_with_bridge(query, top, &bridge)
            .map_err(|e| e.to_string())?;
        if hits.is_empty() {
            return Ok(format!(
                "Ничего не найдено по «{query}» (индекс: {} страниц). \
                 Попробуй другие термы или докрауль релевантный сайт.",
                ix.page_count()
            ));
        }
        let mut out = format!("poler web-search «{query}»: {} из {} страниц\n\n", hits.len(), ix.page_count());
        // Semantic Bridge WHY: агент обязан видеть, какие кандидаты сенсора
        // подмешаны в выдачу (объяснимость — часть контракта инструмента)
        if !expansion.is_empty() {
            out.push_str("Semantic Bridge (кросс-языковое расширение, WHY):\n");
            for l in expansion.why_lines() {
                out.push_str(&format!("  {l}\n"));
            }
            out.push('\n');
        }
        for (i, h) in hits.iter().enumerate() {
            out.push_str(&format!(
                "{}. [{:.4}] {} — {}\n   {}\n\n",
                i + 1,
                h.score,
                h.url,
                if h.title.is_empty() { "-" } else { &h.title },
                h.snippet
            ));
        }
        Ok(out)
        })
    }

    // -----------------------------------------------------------------
    // query_poler_knowledge: Суверенный Гиппокамп (v0.29)
    // -----------------------------------------------------------------
    /// Поиск по библиотеке POLER с эпистемической градацией доверия.
    ///
    /// Хиты несут: статус верификации (MVR-паспорт / первоисточник /
    /// нарратив), файл, номера строк, байтовый диапазон и прямую цитату —
    /// агент цитирует заземлённо и никогда не путает доказанное с нарративом.
    /// Библиотека не построена → инструкция (isError=false), не сбой.
    fn tool_knowledge_query(&self, args: &Value) -> Result<String, String> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or("аргумент query (строка) обязателен")?;
        if query.trim().is_empty() {
            return Err("query пустой — укажи поисковую фразу".into());
        }
        let top = args.get("top").and_then(|v| v.as_u64()).unwrap_or(8) as usize;
        let top = top.clamp(1, 30);
        let min_provenance = match args.get("min_provenance").and_then(|v| v.as_str()) {
            None => None,
            Some(s) => Some(crate::retrieval::Provenance::parse(s).ok_or_else(|| {
                format!("неизвестный min_provenance {s:?} (доступно: mvr | source | narrative)")
            })?),
        };
        // Векторное русло (если индекс построен с ним): .pqw-модель из
        // POLER_KNOWLEDGE_MODEL; без переменной — честная деградация до BM25.
        // M6: тёплый Гиппокамп — SQLite/RaBitQ/HNSW/эмбеддер открываются
        // один раз и живут в резидентном сервере (get-or-open ниже).
        let model = std::env::var("POLER_KNOWLEDGE_MODEL").ok().map(PathBuf::from);
        let opts = crate::sources::knowledge::QueryOptions {
            top,
            min_provenance,
            ..Default::default()
        };
        let mut guard = self
            .warm_knowledge
            .lock()
            .expect("poler-mcp: отравленный лок Гиппокампа");
        if guard.is_none() {
            let embedder =
                match crate::sources::knowledge::query_embedder(&self.knowledge_db, model.as_deref()) {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("poler-mcp: векторное русло пропущено: {e}");
                        None
                    }
                };
            match crate::sources::knowledge::WarmKnowledge::open(&self.knowledge_db, embedder) {
                Ok(wk) => *guard = Some(wk),
                // Библиотека не построена — состояние, не сбой инструмента
                Err(e) if e.contains("--knowledge-ingest") => {
                    return Ok(format!("Библиотека знаний ещё не проиндексирована: {e}"))
                }
                Err(e) => return Err(e),
            }
        }
        let wk = guard
            .as_mut()
            .expect("тёплый Гиппокамп только что открыт");
        match crate::sources::knowledge::query_warm(wk, query, &opts) {
            Ok(out) => Ok(crate::sources::knowledge::render_query_text(query, &out)),
            Err(e) if e.contains("--knowledge-ingest") => {
                Ok(format!("Библиотека знаний ещё не проиндексирована: {e}"))
            }
            Err(e) => Err(e),
        }
    }

    // -----------------------------------------------------------------
    // poler_crawl: seed URL → обход → постоянный индекс
    // -----------------------------------------------------------------
    fn tool_crawl(&self, args: &Value) -> Result<String, String> {
        let seed = args
            .get("seed_url")
            .and_then(|v| v.as_str())
            .ok_or("аргумент seed_url (http(s)://…) обязателен")?;
        if !seed.starts_with("http://") && !seed.starts_with("https://") {
            return Err(format!("seed_url должен быть http(s)://…, получено: {seed}"));
        }
        // Аудит-фикс: анти-SSRF (169.254.169.254 и пр.) до подъёма Chromium
        mcp_url_allowed(seed)?;
        let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
        let max_pages = args.get("max_pages").and_then(|v| v.as_u64()).unwrap_or(25) as usize;
        let delay_ms = args.get("delay_ms").and_then(|v| v.as_u64()).unwrap_or(1000);
        let cross_site = args
            .get("cross_site")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let wait_ms = args.get("wait_ms").and_then(|v| v.as_u64()).unwrap_or(self.wait_ms);
        // пер-страничный таймаут и robots — управляесы вызовом (WebLens
        // передаёт respect_robots=false для кнопки «индексировать ЭТУ страницу»)
        let page_timeout_ms = args
            .get("page_timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(45_000)
            .clamp(1_000, 300_000);
        let respect_robots = args
            .get("respect_robots")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        web::ensure_chromium(self.cdp_port)?;
        // M6: тёплый веб-индекс — краулинг дописывает в резидентное
        // соединение (виден последующим web_search без пере-открытия).
        // Лок индекса держится на весь обход: SQLite-соединение одно,
        // параллельный web_search честно ждёт (защита от SQLITE_BUSY).
        let db_path = self.db_path.clone();
        let cdp_port = self.cdp_port;
        let stats = self.with_web_index(|ix| {
            let cfg = web::CrawlConfig {
                max_pages: max_pages.clamp(1, 500),
                max_depth: depth.min(5),
                delay_ms,
                cross_site,
                wait_ms,
                page_timeout_ms,
                respect_robots,
            };
            eprintln!("poler-mcp: crawl seed={seed} depth={} max={}", cfg.max_depth, cfg.max_pages);
            let mut fetcher = CdpFetcher::new(cdp_port, wait_ms, page_timeout_ms)?;
            web::crawl::crawl(ix, &mut fetcher, seed, &cfg, false).map_err(|e| e.to_string())
        })?;
        Ok(format!(
            "Краулинг завершён: {} загружено, {} проиндексировано, {} без изменений, \
             {} дубликатов (SimHash), {} отклонено robots.txt, {} ошибок, {} мс. \
             Индекс: {:?}.",
            stats.fetched,
            stats.indexed,
            stats.unchanged,
            stats.duplicates,
            stats.skipped_robots,
            stats.errors,
            stats.elapsed_ms,
            db_path
        ))
    }

    // -----------------------------------------------------------------
    // poler_fetch: URL → реальный Chromium → рендер-текст + JSON API
    // -----------------------------------------------------------------
    fn tool_fetch(&self, args: &Value) -> Result<String, String> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or("аргумент url (http(s)://…) обязателен")?;
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(format!("url должен быть http(s)://…, получено: {url}"));
        }
        // Аудит-фикс: анти-SSRF (link-local/metadata) до подъёма Chromium
        mcp_url_allowed(url)?;
        let wait_ms = args.get("wait_ms").and_then(|v| v.as_u64()).unwrap_or(self.wait_ms);

        web::ensure_chromium(self.cdp_port)?;
        let mut session = web::CdpSession::connect(self.cdp_port)?;
        let page = session.load_page_full(url, wait_ms)?;
        if page.text.trim().is_empty() {
            return Err(format!(
                "пустой рендер-текст: {url} (JS-ошибка, таймаут или доступ требует аутентификации)"
            ));
        }
        let text = web::extract::clean_text(&page.text, 256 * 1024);

        // кэш: полный текст — как .md (доступен локальному POLER-поиску),
        // перехваченные JSON API — отдельными файлами
        let dir = web::web_cache_dir();
        let slug = web::url_slug(&page.final_url);
        let title: String = if page.title.trim().is_empty() {
            text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().chars().take(120).collect()
        } else {
            page.title.trim().chars().take(120).collect()
        };
        let text_file = dir.join(format!("{slug}.md"));
        let _ = std::fs::write(
            &text_file,
            format!("# {title}\n\n> source: {}\n\n{}\n", page.final_url, text),
        );
        let mut json_files = Vec::new();
        for (i, (jurl, body)) in page.json_responses.iter().enumerate() {
            let f = dir.join(format!("{}_api{}.json", web::url_slug(jurl), i));
            let pretty = serde_json::from_str::<Value>(body)
                .and_then(|v| serde_json::to_string_pretty(&v))
                .unwrap_or_else(|_| body.clone());
            let _ = std::fs::write(&f, pretty);
            json_files.push(f);
        }

        let preview: String = text.chars().take(FETCH_TEXT_CAP).collect();
        let truncated = text.len() > FETCH_TEXT_CAP;
        let mut out = format!(
            "poler-fetch: {url}\nфинальный URL: {} (редиректы учтены)\nзаголовок: {}\n\
             язык: {}, ссылок: {}, перехвачено JSON API: {}\nполный текст: {} байт{}\n\n",
            page.final_url,
            title,
            if page.lang.is_empty() { "-" } else { &page.lang },
            page.links.len(),
            json_files.len(),
            text.len(),
            if truncated { " (в ответе усечён, полный — в кэше)" } else { "" },
        );
        out.push_str(&preview);
        out.push_str(&format!("\n\n---\nкэш-текст: {}\n", text_file.display()));
        for f in &json_files {
            out.push_str(&format!("кэш-json: {}\n", f.display()));
        }
        Ok(out)
    }

    // -----------------------------------------------------------------
    // poler_search: локальный резонансный поиск (ε/R/сцены/K-hop)
    // -----------------------------------------------------------------
    fn tool_local_search(&self, args: &Value) -> Result<String, String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("аргумент path (файл или директория) обязателен")?;
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or("аргумент query обязателен")?;
        // Аудит-фикс: ограждение локальных путей (анти-экфильтрация)
        guard_path(path)?;
        let top = args.get("top").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
        let target = PathBuf::from(path);
        if !target.exists() {
            return Err(format!("путь не найден: {path}"));
        }
        let config = EngineConfig {
            top_n: top.clamp(1, 20),
            ..Default::default()
        };
        let mut engine = Engine::new(config, false);
        let (res, _stats) = engine.scan(&target, query);
        if res.anchors.is_empty() {
            return Ok(format!("0 хитов по «{query}» в {path}"));
        }
        let mut out = format!(
            "poler local-search «{query}» в {path}: {} хитов (показано {})\n\n",
            res.total_hits,
            res.anchors.len()
        );
        for (i, a) in res.anchors.iter().enumerate() {
            let head: String = a.scene.enclosing_scope.chars().take(200).collect();
            out.push_str(&format!(
                "{}. R={:.1} ε={:.1} | {} | {}\n",
                i + 1,
                a.resonance,
                a.epsilon,
                a.file,
                head.replace('\n', " ")
            ));
        }
        out.push_str("\n(полные сцены/K-hop граф — CLI: poler-engine <path> -q «…» --format ai-json)");
        Ok(out)
    }

    // -----------------------------------------------------------------
    // poler_exec (E2/v0.32.0): идеальный исполнитель + фоновые задачи
    // -----------------------------------------------------------------
    /// Разбор общих аргументов запуска. Умолчание capture для агентов —
    /// head_tail: и голова, и хвост огромного вывода (E1 терял голову).
    #[cfg(feature = "pnd-ffi")]
    fn exec_spec_from_args(args: &Value) -> Result<crate::exec::ExecSpec, String> {
        use crate::exec::{CaptureMode, ExecSpec};

        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or("аргумент command (строка) обязателен")?
            .to_string();
        if command.contains('\0') {
            return Err("command не может содержать NUL".into());
        }
        let cmd_args: Vec<String> = args
            .get("args")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default();
        if cmd_args.iter().any(|a| a.contains('\0')) {
            return Err("аргументы не могут содержать NUL".into());
        }
        let capture = match args.get("capture").and_then(|v| v.as_str()) {
            None => CaptureMode::HeadTail,
            Some(s) => CaptureMode::parse(s)
                .ok_or_else(|| format!("неизвестный capture {s:?} (доступно: tail | head_tail)"))?,
        };
        let env = args.get("env").and_then(|v| v.as_object()).map(|o| {
            o.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect::<Vec<_>>()
        });
        Ok(ExecSpec {
            program: command,
            args: cmd_args,
            env,
            timeout_ms: args.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(30_000),
            grace_ms: args.get("grace_ms").and_then(|v| v.as_u64()).unwrap_or(100),
            max_out_bytes: args
                .get("max_out_bytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(65_536) as usize,
            stdin_data: args.get("stdin").and_then(|v| v.as_str()).map(|s| s.as_bytes().to_vec()),
            cwd: args.get("cwd").and_then(|v| v.as_str()).map(String::from),
            pty: args.get("pty").and_then(|v| v.as_bool()).unwrap_or(false),
            capture,
            cancel: None,
        })
    }

    /// JSON-отчёт исполнения (общий для poler_exec и poler_exec_task).
    #[cfg(feature = "pnd-ffi")]
    fn exec_report_json(out: &crate::exec::ExecOutcome) -> Value {
        json!({
            "exit_code": out.exit_code,
            "signal": out.signal,
            "timed_out": out.timed_out,
            "cancelled": out.cancelled,
            "truncated": out.truncated,
            "pty": out.pty,
            "stdout": String::from_utf8_lossy(&out.stdout),
            "stderr": String::from_utf8_lossy(&out.stderr),
            "stdout_bytes": out.stdout.len(),
            "stderr_bytes": out.stderr.len(),
            "duration_us": out.duration_us,
            "pid": out.pid
        })
    }

    /// Запуск команды через Zig-ядро (raw-syscalls, os/core/poler_exec.zig).
    /// Блокирует до завершения/таймаута/отмены. Для фонового запуска —
    /// poler_exec_async; JSON-отчёт: exit_code/signal/stdout/stderr/
    /// timed_out/cancelled/truncated/duration_us/pid.
    #[cfg(feature = "pnd-ffi")]
    fn tool_exec(&self, args: &Value) -> Result<String, String> {
        use crate::exec::{self, ExecSpec};

        let spec: ExecSpec = Self::exec_spec_from_args(args)?;
        match exec::run(&spec) {
            Ok(out) => Ok(serde_json::to_string_pretty(&Self::exec_report_json(&out))
                .unwrap_or_else(|_| "{}".into())),
            Err(e) => Ok(format!(
                "{{\n  \"error\": \"{e}\",\n  \"exit_code\": {}\n}}",
                e.exit_code()
            )),
        }
    }

    /// Фоновый запуск: task_id сразу, результат — через poler_exec_task.
    /// Задача живёт в собственном потоке реестра; отмена — poler_exec_kill.
    #[cfg(feature = "pnd-ffi")]
    fn tool_exec_async(&self, args: &Value) -> Result<String, String> {
        use crate::exec::ExecSpec;

        let spec: ExecSpec = Self::exec_spec_from_args(args)?;
        let display = if spec.args.is_empty() {
            spec.program.clone()
        } else {
            format!("{} {}", spec.program, spec.args.join(" "))
        };
        let id = crate::exec::TaskRegistry::spawn(&self.exec_tasks, display, spec);
        let report = json!({
            "task_id": id,
            "status": "running",
            "hint": "опрос: poler_exec_task {task_id} (+wait_ms); отмена: poler_exec_kill {task_id}"
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// Опрос/ожидание фоновой задачи: wait_ms — сколько ждать завершения
    /// (0 — мгновенный снимок). Возвращает running-статус или полный отчёт.
    #[cfg(feature = "pnd-ffi")]
    fn tool_exec_task(&self, args: &Value) -> Result<String, String> {
        use crate::exec::TaskSnapshot;

        let id = args
            .get("task_id")
            .and_then(|v| v.as_u64())
            .ok_or("аргумент task_id (число) обязателен")?;
        let wait_ms = args.get("wait_ms").and_then(|v| v.as_u64()).unwrap_or(0).min(60_000);
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(wait_ms);
        loop {
            match self.exec_tasks.snapshot(id) {
                None => return Err(format!("задача {id} не найдена")),
                Some(TaskSnapshot::Running { command, elapsed_us }) => {
                    if std::time::Instant::now() >= deadline {
                        let report = json!({
                            "task_id": id, "status": "running",
                            "command": command, "elapsed_us": elapsed_us
                        });
                        return Ok(serde_json::to_string_pretty(&report)
                            .unwrap_or_else(|_| "{}".into()));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Some(TaskSnapshot::Done { command, elapsed_us, result }) => {
                    let report = match result {
                        Ok(out) => {
                            let mut r = Self::exec_report_json(&out);
                            r["task_id"] = json!(id);
                            r["command"] = json!(command);
                            r["elapsed_us"] = json!(elapsed_us);
                            r["status"] = json!("done");
                            r
                        }
                        Err(e) => json!({
                            "task_id": id, "status": "done", "command": command,
                            "elapsed_us": elapsed_us,
                            "error": e.to_string(), "exit_code": e.exit_code()
                        }),
                    };
                    return Ok(serde_json::to_string_pretty(&report)
                        .unwrap_or_else(|_| "{}".into()));
                }
            }
        }
    }

    /// Отмена фоновой задачи: атомарный флаг → TERM → grace → KILL
    /// (ядро замечает отмену за ≤25 мс). Результат — через poler_exec_task.
    #[cfg(feature = "pnd-ffi")]
    fn tool_exec_kill(&self, args: &Value) -> Result<String, String> {
        let id = args
            .get("task_id")
            .and_then(|v| v.as_u64())
            .ok_or("аргумент task_id (число) обязателен")?;
        match self.exec_tasks.kill(id) {
            Some(true) => Ok(format!(
                "{{\n  \"task_id\": {id},\n  \"killed\": true,\n  \"note\": \"TERM→KILL отправлен; результат — poler_exec_task {id}\"\n}}"
            )),
            Some(false) => Ok(format!(
                "{{\n  \"task_id\": {id},\n  \"killed\": false,\n  \"note\": \"задача уже завершена\"\n}}"
            )),
            None => Err(format!("задача {id} не найдена")),
        }
    }

    /// Список фоновых задач (живые + завершённые, до 128 в реестре).
    #[cfg(feature = "pnd-ffi")]
    fn tool_exec_list(&self, _args: &Value) -> Result<String, String> {
        use crate::exec::TaskSnapshot;

        let tasks: Vec<Value> = self
            .exec_tasks
            .list()
            .into_iter()
            .map(|(id, snap)| match snap {
                TaskSnapshot::Running { command, elapsed_us } => json!({
                    "task_id": id, "status": "running",
                    "command": command, "elapsed_us": elapsed_us
                }),
                TaskSnapshot::Done { command, elapsed_us, result } => match result {
                    Ok(out) => json!({
                        "task_id": id, "status": "done", "command": command,
                        "elapsed_us": elapsed_us, "exit_code": out.exit_code,
                        "cancelled": out.cancelled, "timed_out": out.timed_out
                    }),
                    Err(e) => json!({
                        "task_id": id, "status": "done", "command": command,
                        "elapsed_us": elapsed_us, "error": e.to_string()
                    }),
                },
            })
            .collect();
        let count = tasks.len();
        let report = json!({ "tasks": tasks, "count": count });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    // -----------------------------------------------------------------
    // C2/v0.33.0 «Живая муха»: коннектом FLYCSR1 как резидентный объект
    // допроса для агента. Get-or-load + 10 инструментов ниже.
    // -----------------------------------------------------------------

    /// Get-or-load тёплого коннектома: csr — путь к .csr.zst (первый
    /// вызов обязан его передать; далее артефакт живёт в RAM), nodes —
    /// путь к nodes.bin (опционально: перевод индекс ↔ root_id FlyWire).
    fn fly_state(&self, csr: Option<&str>, nodes: Option<&str>) -> Result<Arc<WarmFly>, String> {
        let mut guard = self
            .warm_fly
            .lock()
            .expect("poler-mcp: отравленный fly-лок");
        // Тёплый артефакт: тот же путь (или запрос без csr) → reuse.
        if let Some(warm) = guard.as_ref() {
            let same = csr.map(|p| *p == warm.csr_path).unwrap_or(true);
            if same {
                if let Some(p) = nodes {
                    if warm.nodes.is_none() {
                        let tbl = fly_load_nodes(p, &warm.con)?;
                        let upgraded = Arc::new(WarmFly {
                            con: warm.con.clone(),
                            nodes: Some(tbl),
                            csr_path: warm.csr_path.clone(),
                            load_ms: warm.load_ms,
                            csc: std::sync::OnceLock::new(),
                        });
                        *guard = Some(upgraded.clone());
                        return Ok(upgraded);
                    }
                }
                return Ok(warm.clone());
            }
        }
        let Some(csr) = csr else {
            return Err(
                "коннектом не загружен: передайте csr (путь к .csr.zst) хотя бы \
                 в одном вызове poler_fly*"
                    .to_string(),
            );
        };
        guard_path(csr)?;
        let t0 = std::time::Instant::now();
        let con = Connectome::load(std::path::Path::new(csr))?;
        let load_ms = t0.elapsed().as_millis();
        let tbl = nodes.map(|p| fly_load_nodes(p, &con)).transpose()?;
        let warm = Arc::new(WarmFly {
            con: Arc::new(con),
            nodes: tbl,
            csr_path: PathBuf::from(csr),
            load_ms,
            csc: std::sync::OnceLock::new(),
        });
        *guard = Some(warm.clone());
        Ok(warm)
    }

    /// Спецификация нейрона из JSON: индекс (число/строка) или root_id.
    fn fly_spec(v: &Value) -> Option<String> {
        v.as_str()
            .map(str::to_string)
            .or_else(|| v.as_u64().map(|n| n.to_string()))
    }

    /// Резолв спецификации: индекс CSR либо root_id (нужна таблица узлов).
    fn fly_resolve(w: &WarmFly, spec: &str) -> Result<usize, String> {
        if let Ok(idx) = spec.parse::<usize>() {
            if idx < w.con.n_nodes() {
                return Ok(idx);
            }
        }
        if let Some(ns) = &w.nodes {
            if let Ok(rid) = spec.parse::<u64>() {
                if let Some(i) = ns.idx_of(rid) {
                    return Ok(i);
                }
            }
        }
        Err(format!(
            "нейрон «{spec}» не найден (узлов: {}; формат: индекс 0..{} либо root_id с nodes)",
            w.con.n_nodes(),
            w.con.n_nodes().saturating_sub(1)
        ))
    }

    /// Обязательный аргумент-нейрон (ключ — имя аргумента).
    fn fly_neuron_arg(args: &Value, key: &str, w: &WarmFly) -> Result<usize, String> {
        let raw = args.get(key).ok_or(format!("аргумент {key} обязателен"))?;
        let spec = Self::fly_spec(raw)
            .ok_or(format!("аргумент {key}: индекс (число) или root_id (строка)"))?;
        Self::fly_resolve(w, &spec)
    }

    /// Массив нейронов (1..=lim) → индексы.
    fn fly_neurons_arg(args: &Value, key: &str, w: &WarmFly, lim: usize) -> Result<Vec<usize>, String> {
        let arr = args
            .get(key)
            .and_then(|v| v.as_array())
            .ok_or(format!("аргумент {key} (массив индексов/root_id) обязателен"))?;
        if arr.is_empty() {
            return Err(format!("{key}: пустой массив"));
        }
        if arr.len() > lim {
            return Err(format!("{key}: {} элементов — максимум {lim}", arr.len()));
        }
        arr.iter()
            .map(|v| {
                let spec = Self::fly_spec(v)
                    .ok_or(format!("{key}: индекс (число) или root_id (строка)"))?;
                Self::fly_resolve(w, &spec)
            })
            .collect()
    }

    /// Фильтр знака (умолчание all).
    fn fly_sign_arg(args: &Value) -> Result<SignFilter, String> {
        match args.get("sign") {
            None | Some(Value::Null) => Ok(SignFilter::All),
            Some(v) => SignFilter::parse(v.as_str().ok_or("sign: строка all | exc | inh")?),
        }
    }

    /// Направление (умолчание out; для common — down).
    fn fly_dir_arg(args: &Value) -> Result<Direction, String> {
        match args.get("direction") {
            None | Some(Value::Null) => Ok(Direction::Out),
            Some(v) => Direction::parse(v.as_str().ok_or("direction: строка out | in (down | up)")?),
        }
    }

    /// Корень узла (root_id FlyWire, если таблица загружена).
    fn fly_root(w: &WarmFly, idx: usize) -> Value {
        w.nodes.as_ref().and_then(|n| n.root_id(idx)).map(Value::from).unwrap_or(Value::Null)
    }

    /// JSON-объект ребра (other = другой конец).
    fn fly_edge_json(w: &WarmFly, e: &crate::graph::connectome::Edge) -> Value {
        json!({
            "other": e.target,
            "other_root_id": Self::fly_root(w, e.target as usize),
            "weight": e.weight,
            "nt": e.nt_name(),
            "sign": e.sign(),
            "signed_weight": e.signed_weight(),
        })
    }

    /// poler_fly: загрузка/сводка/выгрузка коннектома. Первый вызов
    /// грузит артефакт в RAM (csr + опционально nodes), дальше всё
    /// семейство работает без диска. action: summary (умолчание) | eject.
    fn tool_fly(&self, args: &Value) -> Result<String, String> {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("summary");
        if matches!(action, "eject" | "выгрузить") {
            let mut guard = self
                .warm_fly
                .lock()
                .expect("poler-mcp: отравленный fly-лок");
            let had = guard.take().is_some();
            let report = json!({
                "action": "eject",
                "unloaded": had,
                "note": "коннектом выгружен из RAM; следующий poler_fly загрузит заново"
            });
            return Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()));
        }
        if !matches!(action, "summary" | "сводка") {
            return Err(format!("неизвестное действие «{action}» (summary | eject)"));
        }
        let csr = args.get("csr").and_then(|v| v.as_str());
        let nodes = args.get("nodes").and_then(|v| v.as_str());
        let w = self.fly_state(csr, nodes)?;
        let con = &w.con;
        let m = con.mass_by_nt();
        let exc = m[1].0 + m[2].0;
        let inh = m[0].0;
        let modm = m[3].0 + m[4].0 + m[5].0;
        let tot = con.total_mass();
        let pct = |x: u64| format!("{:.1}%", x as f64 / tot as f64 * 100.0);
        let report = json!({
            "mode": "summary",
            "artifact": w.csr_path.display().to_string(),
            "core": con.is_core(),
            "n_nodes": con.n_nodes(),
            "n_edges": con.n_edges(),
            "total_mass": tot,
            "load_ms": w.load_ms,
            "warm": {"csc_built": w.csc_ready(), "has_nodes": w.nodes.is_some()},
            "nt": (0..6).map(|c| json!({
                "name": crate::graph::connectome::NT_NAMES[c],
                "edges": m[c].1,
                "mass": m[c].0,
                "sign": crate::graph::connectome::nt_sign(c as u8),
            })).collect::<Vec<_>>(),
            "mass_balance": {
                "excitatory": exc, "inhibitory": inh, "modulatory": modm,
                "excitatory_pct": pct(exc), "inhibitory_pct": pct(inh), "modulatory_pct": pct(modm),
            },
            "hint": "семейство: poler_fly_node/edge/khop/path/common/centrality/rotor/motifs/propagate (csr больше не нужен — муха в RAM)",
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_fly_node: паспорт нейрона — степени, массы, топ партнёров
    /// в обоих направлениях (с root_id, медиаторами и знаками).
    fn tool_fly_node(&self, args: &Value) -> Result<String, String> {
        let csr = args.get("csr").and_then(|v| v.as_str());
        let nodes = args.get("nodes").and_then(|v| v.as_str());
        let w = self.fly_state(csr, nodes)?;
        let idx = Self::fly_neuron_arg(args, "neuron", &w)?;
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(8)
            .clamp(1, 500) as usize;
        let con = &w.con;
        let csc = w.csc();
        let outs: Vec<crate::graph::connectome::Edge> =
            con.out_edges(idx).unwrap().collect();
        let out_mass: u64 = outs.iter().map(|e| e.weight as u64).sum();
        let ins: Vec<crate::graph::connectome::Edge> =
            csc.in_edges(con, idx).unwrap().collect();
        let in_mass: u64 = ins.iter().map(|e| e.weight as u64).sum();
        let mut top_out = outs.clone();
        top_out.sort_by_key(|e| std::cmp::Reverse(e.weight));
        top_out.truncate(limit);
        let mut top_in = ins.clone();
        top_in.sort_by_key(|e| std::cmp::Reverse(e.weight));
        top_in.truncate(limit);
        let report = json!({
            "neuron": idx,
            "root_id": Self::fly_root(&w, idx),
            "out_degree": outs.len(),
            "out_mass": out_mass,
            "in_degree": ins.len(),
            "in_mass": in_mass,
            "top_out": top_out.iter().map(|e| Self::fly_edge_json(&w, e)).collect::<Vec<_>>(),
            "top_in": top_in.iter().map(|e| Self::fly_edge_json(&w, e)).collect::<Vec<_>>(),
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_fly_edge: ребро u→v и ротор J = A − Aᵀ пары (циркуляция
    /// влияния: однонаправленный поток или реципрокная компенсация).
    fn tool_fly_edge(&self, args: &Value) -> Result<String, String> {
        let csr = args.get("csr").and_then(|v| v.as_str());
        let nodes = args.get("nodes").and_then(|v| v.as_str());
        let w = self.fly_state(csr, nodes)?;
        let u = Self::fly_neuron_arg(args, "u", &w)?;
        let v = Self::fly_neuron_arg(args, "v", &w)?;
        let con = &w.con;
        let fwd = con.edge(u, v);
        let bwd = con.edge(v, u);
        let rotor = con.rotor(u, v);
        if fwd.is_none() && bwd.is_none() {
            let report = json!({
                "u": u, "u_root_id": Self::fly_root(&w, u),
                "v": v, "v_root_id": Self::fly_root(&w, v),
                "found": false,
                "rotor_Juv": 0,
            });
            return Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()));
        }
        let report = json!({
            "u": u, "u_root_id": Self::fly_root(&w, u),
            "v": v, "v_root_id": Self::fly_root(&w, v),
            "found": true,
            "forward": fwd.map(|e| Self::fly_edge_json(&w, &e)),
            "backward": bwd.map(|e| Self::fly_edge_json(&w, &e)),
            "rotor_Juv": rotor,
            "rotor_Jvu": -rotor,
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_fly_khop: BFS потока сигнала от нейрона — размеры фронтов
    /// по хопам и всего достигнуто (фильтр знака сужает поток).
    fn tool_fly_khop(&self, args: &Value) -> Result<String, String> {
        let csr = args.get("csr").and_then(|v| v.as_str());
        let nodes = args.get("nodes").and_then(|v| v.as_str());
        let w = self.fly_state(csr, nodes)?;
        let idx = Self::fly_neuron_arg(args, "neuron", &w)?;
        let depth = args
            .get("depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(2)
            .clamp(1, 10) as usize;
        let filter = Self::fly_sign_arg(args)?;
        let r = w.con.k_hop(idx, depth, filter);
        let report = json!({
            "start": idx,
            "start_root_id": Self::fly_root(&w, idx),
            "depth": depth,
            "sign_filter": args.get("sign").and_then(|v| v.as_str()).unwrap_or("all"),
            "frontier_sizes": r.frontier_sizes,
            "visited": r.visited,
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_fly_path: кратчайший путь сигнала from→to с цепочкой
    /// прыжков (вес/медиатор/знак каждого синапса на маршруте).
    fn tool_fly_path(&self, args: &Value) -> Result<String, String> {
        let csr = args.get("csr").and_then(|v| v.as_str());
        let nodes = args.get("nodes").and_then(|v| v.as_str());
        let w = self.fly_state(csr, nodes)?;
        let from = Self::fly_neuron_arg(args, "from", &w)?;
        let to = Self::fly_neuron_arg(args, "to", &w)?;
        let filter = Self::fly_sign_arg(args)?;
        let r = w.con.shortest_path(from, to, filter);
        let report = json!({
            "from": from, "from_root_id": Self::fly_root(&w, from),
            "to": to, "to_root_id": Self::fly_root(&w, to),
            "sign_filter": args.get("sign").and_then(|v| v.as_str()).unwrap_or("all"),
            "found": r.found,
            "length": r.length(),
            "total_weight": r.total_weight(),
            "hops": r.hops.iter().map(|h| json!({
                "from": h.from, "from_root_id": Self::fly_root(&w, h.from),
                "to": h.to, "to_root_id": Self::fly_root(&w, h.to),
                "weight": h.edge.weight,
                "nt": h.edge.nt_name(),
                "sign": h.edge.sign(),
                "signed_weight": h.edge.signed_weight(),
            })).collect::<Vec<_>>(),
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_fly_common: пересечение окрестностей набора нейронов —
    /// общие мишени (down) или общие источники (up): конвергенция
    /// и дивергенция схемы мозга.
    fn tool_fly_common(&self, args: &Value) -> Result<String, String> {
        let csr = args.get("csr").and_then(|v| v.as_str());
        let nodes = args.get("nodes").and_then(|v| v.as_str());
        let w = self.fly_state(csr, nodes)?;
        let idxs = Self::fly_neurons_arg(args, "neurons", &w, 32)?;
        let dir = Self::fly_dir_arg(args)?;
        let filter = Self::fly_sign_arg(args)?;
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(20)
            .clamp(1, 500) as usize;
        let mut partners = w.con.common_partners(&idxs, dir, filter, w.csc())?;
        let count_full = partners.len();
        partners.truncate(limit);
        let report = json!({
            "neurons": idxs,
            "direction": match dir { Direction::Out => "down", Direction::In => "up" },
            "sign_filter": args.get("sign").and_then(|v| v.as_str()).unwrap_or("all"),
            "count": count_full,
            "partners": partners.iter().map(|p| json!({
                "node": p.node,
                "root_id": Self::fly_root(&w, p.node),
                "total_weight": p.total_weight,
                "links": p.members.iter().map(|(q, e)| json!({
                    "query": q,
                    "edge": Self::fly_edge_json(&w, e),
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_fly_centrality: хабы мозга — топ по степеням + взвешенный
    /// PageRank (модуляторы не проводят ранг; фильтр знака сужает граф).
    fn tool_fly_centrality(&self, args: &Value) -> Result<String, String> {
        let csr = args.get("csr").and_then(|v| v.as_str());
        let nodes = args.get("nodes").and_then(|v| v.as_str());
        let w = self.fly_state(csr, nodes)?;
        let top = args
            .get("top")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .clamp(1, 100) as usize;
        let filter = Self::fly_sign_arg(args)?;
        let damping = args
            .get("damping")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.85)
            .clamp(0.05, 0.99);
        let iterations = args
            .get("iterations")
            .and_then(|v| v.as_u64())
            .unwrap_or(30)
            .clamp(1, 200) as usize;
        let con = &w.con;
        let csc = w.csc();
        let top_out = con.degree_ranking(Direction::Out, top, csc);
        let top_in = con.degree_ranking(Direction::In, top, csc);
        let pr = con.pagerank(filter, damping, iterations, 1e-9, top);
        let report = json!({
            "sign_filter": args.get("sign").and_then(|v| v.as_str()).unwrap_or("all"),
            "top_out_degree": top_out.iter().map(|&(u, d)| json!({
                "neuron": u, "root_id": Self::fly_root(&w, u), "out_degree": d,
            })).collect::<Vec<_>>(),
            "top_in_degree": top_in.iter().map(|&(v, d)| json!({
                "neuron": v, "root_id": Self::fly_root(&w, v), "in_degree": d,
            })).collect::<Vec<_>>(),
            "pagerank": {
                "damping": damping,
                "iterations": pr.iterations,
                "converged": pr.converged,
                "top": pr.top.iter().map(|&(u, r)| json!({
                    "neuron": u, "root_id": Self::fly_root(&w, u), "rank": r,
                })).collect::<Vec<_>>(),
            },
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_fly_rotor: глобальный топ пар по циркуляции J = A − Aᵀ —
    /// самые однонаправленные влияния мозга (чистые потоки без
    /// встречной компенсации).
    fn tool_fly_rotor(&self, args: &Value) -> Result<String, String> {
        let csr = args.get("csr").and_then(|v| v.as_str());
        let nodes = args.get("nodes").and_then(|v| v.as_str());
        let w = self.fly_state(csr, nodes)?;
        let top = args
            .get("top")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .clamp(1, 1000) as usize;
        let min_abs = args
            .get("min_abs")
            .and_then(|v| v.as_i64())
            .unwrap_or(1)
            .clamp(1, i32::MAX as i64) as i32;
        let pairs = w.con.rotor_top(top, min_abs);
        let count = pairs.len();
        let report = json!({
            "top": top,
            "min_abs": min_abs,
            "count": count,
            "pairs": pairs.iter().map(|p| json!({
                "u": p.u, "u_root_id": Self::fly_root(&w, p.u),
                "v": p.v, "v_root_id": Self::fly_root(&w, p.v),
                "J_uv": p.j,
                "J_vu": -p.j,
                "forward_signed": p.forward,
                "backward_signed": p.backward,
            })).collect::<Vec<_>>(),
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_fly_motifs: перепись мотивов вокруг нейрона — реципрокные
    /// пары, feedforward-треугольники, feedback-циклы (местные схемы
    /// усиления и обратной связи).
    fn tool_fly_motifs(&self, args: &Value) -> Result<String, String> {
        let csr = args.get("csr").and_then(|v| v.as_str());
        let nodes = args.get("nodes").and_then(|v| v.as_str());
        let w = self.fly_state(csr, nodes)?;
        let idx = Self::fly_neuron_arg(args, "neuron", &w)?;
        let examples = args
            .get("examples")
            .and_then(|v| v.as_u64())
            .unwrap_or(5)
            .min(50) as usize;
        let m = w
            .con
            .motif_census(idx, w.csc(), examples)
            .ok_or(format!("нейрон {idx} вне диапазона (узлов: {})", w.con.n_nodes()))?;
        let report = json!({
            "neuron": idx,
            "root_id": Self::fly_root(&w, idx),
            "out_degree": m.out_degree,
            "in_degree": m.in_degree,
            "reciprocal_count": m.reciprocal.len(),
            "reciprocal": m.reciprocal.iter().map(|(fwd, bwd)| json!({
                "partner": fwd.target,
                "partner_root_id": Self::fly_root(&w, fwd.target as usize),
                "u_to_partner": {"weight": fwd.weight, "nt": fwd.nt_name(), "sign": fwd.sign()},
                "partner_to_u": {"weight": bwd.weight, "nt": bwd.nt_name(), "sign": bwd.sign()},
            })).collect::<Vec<_>>(),
            "feedforward": m.feedforward,
            "feedback3": m.feedback3,
            "ff_examples": m.ff_examples.iter().map(|&(v, t)| json!({
                "mid": v, "mid_root_id": Self::fly_root(&w, v),
                "target": t, "target_root_id": Self::fly_root(&w, t),
            })).collect::<Vec<_>>(),
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_fly_propagate: симуляция распространения сигнала —
    /// x(t+1) = leak·x(t) + γ·A·x(t) на знаковых весах: возбуждение
    /// разгоняет, торможение гасит. Муха «думает» прямо в RAM.
    fn tool_fly_propagate(&self, args: &Value) -> Result<String, String> {
        let csr = args.get("csr").and_then(|v| v.as_str());
        let nodes = args.get("nodes").and_then(|v| v.as_str());
        let w = self.fly_state(csr, nodes)?;
        let seeds = Self::fly_neurons_arg(args, "neurons", &w, 64)?;
        let steps = args
            .get("steps")
            .and_then(|v| v.as_u64())
            .unwrap_or(4)
            .clamp(1, 64) as usize;
        let gamma = args
            .get("gamma")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.05)
            .clamp(0.0, 10.0);
        let leak = args
            .get("leak")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.8)
            .clamp(0.0, 1.0);
        let theta = args
            .get("theta")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.01);
        let top = args
            .get("top")
            .and_then(|v| v.as_u64())
            .unwrap_or(15)
            .clamp(1, 200) as usize;
        let filter = Self::fly_sign_arg(args)?;
        let r = w
            .con
            .propagate(&seeds, steps, gamma, leak, filter, top, theta)?;
        let round6 = |x: f64| (x * 1e6).round() / 1e6;
        let report = json!({
            "seeds": r.seeds.iter().map(|&s| json!({
                "neuron": s, "root_id": Self::fly_root(&w, s),
            })).collect::<Vec<_>>(),
            "params": {
                "steps": steps, "gamma": gamma, "leak": leak,
                "sign_filter": args.get("sign").and_then(|v| v.as_str()).unwrap_or("all"),
                "theta": theta,
            },
            "timeline": r.steps.iter().map(|s| json!({
                "step": s.step,
                "active": s.active,
                "positive_mass": round6(s.positive_mass),
                "negative_mass": round6(s.negative_mass),
            })).collect::<Vec<_>>(),
            "top": r.top.iter().map(|&(v, x)| json!({
                "neuron": v, "root_id": Self::fly_root(&w, v), "potential": round6(x),
            })).collect::<Vec<_>>(),
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    // -----------------------------------------------------------------
    // L1/v0.34.0: Литературный Двигатель POLER[Ψ]
    // -----------------------------------------------------------------

    /// Гиперпараметры POLER[Ψ] из аргументов (умолчания — манифест).
    fn psi_params_from_args(args: &Value) -> LiteraryParams {
        let mut p = LiteraryParams::default();
        if let Some(v) = args.get("dims").and_then(|v| v.as_u64()) {
            p.dims = v as usize;
        }
        if let Some(v) = args.get("steps").and_then(|v| v.as_u64()) {
            p.max_steps = v as usize;
        }
        if let Some(v) = args.get("eta").and_then(|v| v.as_f64()) {
            p.eta = v as f32;
        }
        if let Some(v) = args.get("eta_r").and_then(|v| v.as_f64()) {
            p.eta_r = v as f32;
        }
        if let Some(v) = args.get("rho").and_then(|v| v.as_f64()) {
            p.rho = v as f32;
        }
        if let Some(v) = args.get("kappa").and_then(|v| v.as_f64()) {
            p.kappa = v as f32;
        }
        if let Some(v) = args.get("gamma").and_then(|v| v.as_f64()) {
            p.gamma = v as f32;
        }
        if let Some(v) = args.get("lambda").and_then(|v| v.as_f64()) {
            p.lambda_rl = v as f32;
        }
        if let Some(v) = args.get("no_mul").and_then(|v| v.as_bool()) {
            p.no_mul = v;
        }
        p
    }

    /// Ограничения причинности из аргументов: массив {i, j, c}
    /// (пустой/отсутствует — канонический замок p₀ − p₁ = 0).
    fn psi_constraints_from_args(args: &Value) -> Result<Vec<(usize, usize, f32)>, String> {
        let Some(arr) = args.get("constraints").and_then(|v| v.as_array()) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::with_capacity(arr.len());
        for (k, c) in arr.iter().enumerate() {
            let i = c
                .get("i")
                .and_then(|v| v.as_u64())
                .ok_or(format!("constraints[{k}].i: целое обязательно"))?;
            let j = c
                .get("j")
                .and_then(|v| v.as_u64())
                .ok_or(format!("constraints[{k}].j: целое обязательно"))?;
            let w = c.get("c").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
            out.push((i as usize, j as usize, w));
        }
        Ok(out)
    }

    /// Мушиная калибровка из аргументов: csr (+nodes) → WarmFly (тёплый
    /// артефакт!) → каста от seeds (умолчание [0]) на khop шагов.
    /// None — аргумента csr нет, чистая физика текста.
    fn psi_maybe_cast(&self, args: &Value) -> Result<Option<crate::literary::FlyCast>, String> {
        let csr = args.get("csr").and_then(|v| v.as_str());
        if csr.is_none() {
            return Ok(None);
        }
        let nodes = args.get("nodes").and_then(|v| v.as_str());
        let w = self.fly_state(csr, nodes)?;
        let seeds = match args.get("seeds").and_then(|v| v.as_array()) {
            Some(arr) if !arr.is_empty() => Self::fly_neurons_arg(args, "seeds", &w, 16)?,
            _ => vec![0],
        };
        let khop = args
            .get("khop")
            .and_then(|v| v.as_u64())
            .unwrap_or(2)
            .clamp(1, 4) as usize;
        let max_cast = args
            .get("max_cast")
            .and_then(|v| v.as_u64())
            .unwrap_or(48)
            .clamp(2, crate::literary::flybridge::MAX_CAST as u64) as usize;
        Ok(Some(build_cast(&w.con, &seeds, khop, max_cast)?))
    }

    /// poler_literary_field: анализ поля интенции — перцепция ℘,
    /// архетипическая топология O, оценка свободной энергии и
    /// опциональная мушиная калибровка (каста + конфликты).
    fn tool_literary_field(&self, args: &Value) -> Result<String, String> {
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or("аргумент text (строка-замысел) обязателен")?;
        let params = Self::psi_params_from_args(args);
        let constraints = Self::psi_constraints_from_args(args)?;
        let cast = self.psi_maybe_cast(args)?;
        let eng = LiteraryEngine::new(params, text, &constraints, cast)?;
        let top = crate::literary::qualia::cosine_topology(&eng.field);
        let mut report = json!({
            "phase": "℘→O",
            "dims": eng.params.dims,
            "tokens": eng.field.n_tokens,
            "terms": eng.field.n_terms,
            "intent_terms": eng.field.term_mass.iter().map(|(t, w)| json!({
                "term": t, "mass": psi_round(*w),
            })).collect::<Vec<_>>(),
            "archetypes": top.iter().take(6).map(|&(i, w)| json!({
                "name": crate::literary::qualia::archetype_name(i),
                "cosine": psi_round(w),
            })).collect::<Vec<_>>(),
            "constraints": constraints.len(),
            "f_initial": psi_round(eng.free_energy()),
            "causality_residual_initial": psi_round(eng.projector.residual(&eng.p)),
        });
        if let Some(c) = &eng.cast {
            report["fly"] = json!({
                "members": c.members.len(),
                "reached": c.stats.reached,
                "inner_edges": c.stats.inner_edges,
                "inner_abs_mass": c.stats.inner_abs_mass as i64,
                "top_rotor": c.stats.top_rotor.map(|(u, v, j)| json!({
                    "u": u, "v": v, "j": j,
                })),
                "archetype_load": c.anchors.iter().fold(
                    [0usize; 12],
                    |mut acc, &a| { acc[a] += 1; acc }
                ),
                "note": "ротор J = A − Aᵀ касты закрутит нарратив (γJ·p); метрика Ляпунова D = L·Lᵀ гасит пертурбации",
            });
        }
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_literary_step: создать сессию (text) или сделать один шаг
    /// фазового обновления p_t → p_{t+1} (session + опционально
    /// observation — эволюция замысла).
    fn tool_literary_step(&self, args: &Value) -> Result<String, String> {
        let session_id = args.get("session").and_then(|v| v.as_str());
        let text = args.get("text").and_then(|v| v.as_str());
        let observation = args.get("observation").and_then(|v| v.as_str());

        let (id, arc) = match (session_id, text) {
            (Some(id), _) => {
                let arc = self
                    .warm_psi
                    .lock()
                    .expect("poler-mcp: отравленный psi-лок")
                    .get(id)
                    .ok_or_else(|| {
                        format!("сессия «{id}» не найдена (создайте: text=..., или poler_literary_eject all для очистки)")
                    })?;
                (id.to_string(), arc)
            }
            (None, Some(t)) => {
                let params = Self::psi_params_from_args(args);
                let constraints = Self::psi_constraints_from_args(args)?;
                let cast = self.psi_maybe_cast(args)?;
                let eng = LiteraryEngine::new(params, t, &constraints, cast)?;
                let mut guard = self
                    .warm_psi
                    .lock()
                    .expect("poler-mcp: отравленный psi-лок");
                let (id, arc) = guard.insert(eng);
                let live = guard.len();
                let _ = live;
                (id, arc)
            }
            (None, None) => {
                return Err(
                    "укажите text (создать сессию) либо session (продолжить существующую)"
                        .into(),
                );
            }
        };

        let mut eng = arc
            .lock()
            .map_err(|_| "poler-mcp: сессия psi отравлена".to_string())?;
        let t = eng.step(observation);
        let report = json!({
            "session": id,
            "step": t.step,
            "f_free": psi_round(t.f_free),
            "epsilon": psi_round(t.epsilon),
            "sigma_t": psi_round(t.sigma_t),
            "resonance_norm": psi_round(t.resonance_norm),
            "phase_inertia": psi_round(t.phase_inertia),
            "causality_residual": psi_round(t.causality_residual),
            "plasticity": psi_round(t.plasticity),
            "h_psi": psi_round(t.h_psi),
            "mode": Self::psi_mode_name(t.mode),
            "refractions": t.refractions,
            "note": "поле шагнуло: p_t → p_t+1; H^Ψ → 0 = когнитивный покой",
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_literary_generate: полный прогон до сверхпроводимости —
    /// либо one-shot (text), либо на резидентной сессии (session).
    fn tool_literary_generate(&self, args: &Value) -> Result<String, String> {
        let session_id = args.get("session").and_then(|v| v.as_str());
        let text = args.get("text").and_then(|v| v.as_str());
        let report = match (session_id, text) {
            (Some(id), _) => {
                let arc = self
                    .warm_psi
                    .lock()
                    .expect("poler-mcp: отравленный psi-лок")
                    .get(id)
                    .ok_or_else(|| format!("сессия «{id}» не найдена"))?;
                let mut eng = arc
                    .lock()
                    .map_err(|_| "poler-mcp: сессия psi отравлена".to_string())?;
                eng.generate()
            }
            (None, Some(t)) => {
                let params = Self::psi_params_from_args(args);
                let constraints = Self::psi_constraints_from_args(args)?;
                let cast = self.psi_maybe_cast(args)?;
                LiteraryEngine::new(params, t, &constraints, cast)?.generate()
            }
            (None, None) => {
                return Err("укажите text (one-shot) либо session (резидентная)".into());
            }
        };
        let report = Self::psi_report_json(&report, session_id);
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_literary_eject: выгрузить сессию (id) или все ("all").
    fn tool_literary_eject(&self, args: &Value) -> Result<String, String> {
        let id = args
            .get("session")
            .and_then(|v| v.as_str())
            .unwrap_or("all");
        let n = self
            .warm_psi
            .lock()
            .expect("poler-mcp: отравленный psi-лок")
            .eject(id);
        let report = json!({
            "ejected": n,
            "requested": id,
            "note": "латентные состояния выгружены из RAM",
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    // ── S1/v0.35.0: Синаптический Вихрь SSN ───────────────────────

    /// Телеметрия вихря → JSON (общая для step/inject/status).
    fn ssn_telemetry_json(id: &str, t: &crate::ssn::VortexTelemetry) -> Value {
        json!({
            "session": id,
            "step": t.step,
            "activity": ssn_round(t.activity),
            "f_sys": ssn_round(t.f_sys),
            "b_tone": ssn_round(t.b_tone),
            "synchrony": ssn_round(t.synchrony),
            "criticality": ssn_round(t.criticality),
            "ei": ssn_round(t.ei),
            "gaba": t.gaba,
            "glut": t.glut,
            "da": t.da,
            "ht_5ht": t.ht,
            "ne": t.ne,
            "w_min": ssn_round(t.w_min),
            "w_max": ssn_round(t.w_max),
            "w_at_clip": ssn_round(t.w_at_clip),
            "active_count": t.active_count,
        })
    }

    /// poler_ssn_step: создать живой мозг (без text — новые параметры)
    /// или шагнуть существующий на N шагов. Параметры создания: seed,
    /// n, fields, dims. Возвращает телеметрию + readout топ-K.
    fn tool_ssn_step(&self, args: &Value) -> Result<String, String> {
        let session_id = args.get("session").and_then(|v| v.as_str());
        let steps = args
            .get("steps")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000)
            .clamp(1, 100_000) as usize;
        let readout_k = args
            .get("readout")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .clamp(1, 100) as usize;

        let (id, arc) = match session_id {
            Some(id) => {
                let arc = self
                    .warm_ssn
                    .lock()
                    .expect("poler-mcp: отравленный ssn-лок")
                    .get(id)
                    .ok_or_else(|| {
                        format!("мозг «{id}» не найден (создайте: poler_ssn_step без session, или poler_ssn_eject all)")
                    })?;
                (id.to_string(), arc)
            }
            None => {
                let cfg = crate::ssn::VortexConfig {
                    n: args.get("n").and_then(|v| v.as_u64()).unwrap_or(600).clamp(8, 1_000_000) as usize,
                    fields: args.get("fields").and_then(|v| v.as_u64()).unwrap_or(16).clamp(1, 64) as usize,
                    topology_seed: args.get("seed").and_then(|v| v.as_u64()).unwrap_or(777),
                    ..Default::default()
                };
                let seed = args.get("seed").and_then(|v| v.as_u64()).unwrap_or(777);
                let dims = args.get("dims").and_then(|v| v.as_u64()).unwrap_or(128).clamp(8, 4096) as usize;
                let eng = crate::ssn::SsnEngine::new(cfg, seed, dims);
                let mut guard = self
                    .warm_ssn
                    .lock()
                    .expect("poler-mcp: отравленный ssn-лок");
                let (id, arc) = guard.insert(eng);
                let _ = guard.len();
                (id, arc)
            }
        };

        let mut eng = arc
            .lock()
            .map_err(|_| "poler-mcp: мозг ssn отравлен".to_string())?;
        for _ in 0..steps {
            eng.step();
        }
        let t = eng.telemetry();
        let readout: Vec<Value> = eng
            .readout(readout_k)
            .iter()
            .map(|(i, v)| json!({"neuron": i, "activation": ssn_round(*v)}))
            .collect();
        let report = json!({
            "telemetry": Self::ssn_telemetry_json(&id, &t),
            "readout": readout,
            "steps_done": steps,
            "note": "живой мозг шагнул: гомеостаз-интегратор держит ~5%, лавины = критичность",
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_ssn_inject: сенсорная инъекция текста в живой мозг
    /// (CSE: золотая фаза → вектор → активации) + опциональные шаги.
    fn tool_ssn_inject(&self, args: &Value) -> Result<String, String> {
        let id = args
            .get("session")
            .and_then(|v| v.as_str())
            .ok_or("укажите session (id живого мозга, создан poler_ssn_step)")?;
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or("укажите text — сенсорный вход (что мозг должен воспринять)")?;
        let steps = args
            .get("steps")
            .and_then(|v| v.as_u64())
            .unwrap_or(500)
            .clamp(0, 100_000) as usize;
        let arc = self
            .warm_ssn
            .lock()
            .expect("poler-mcp: отравленный ssn-лок")
            .get(id)
            .ok_or_else(|| format!("мозг «{id}» не найден (создайте: poler_ssn_step без session)"))?;
        let mut eng = arc
            .lock()
            .map_err(|_| "poler-mcp: мозг ssn отравлен".to_string())?;
        let (chars, touched) = eng.inject_text(text);
        for _ in 0..steps {
            eng.step();
        }
        let t = eng.telemetry();
        let report = json!({
            "telemetry": Self::ssn_telemetry_json(id, &t),
            "injected": {"chars": chars, "neurons_touched": touched, "text": text},
            "steps_done": steps,
            "note": "текст воспринят: CSE-вектор (золотая фаза) влился в активации",
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_ssn_status: телеметрия + readout живого мозга (без шага).
    fn tool_ssn_status(&self, args: &Value) -> Result<String, String> {
        let id = args
            .get("session")
            .and_then(|v| v.as_str())
            .ok_or("укажите session (id живого мозга)")?;
        let readout_k = args
            .get("readout")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .clamp(1, 100) as usize;
        let live = self
            .warm_ssn
            .lock()
            .expect("poler-mcp: отравленный ssn-лок")
            .len();
        let arc = self
            .warm_ssn
            .lock()
            .expect("poler-mcp: отравленный ssn-лок")
            .get(id)
            .ok_or_else(|| {
                format!("мозг «{id}» не найден (живых мозгов: {live}; создайте: poler_ssn_step без session)")
            })?;
        let eng = arc
            .lock()
            .map_err(|_| "poler-mcp: мозг ssn отравлен".to_string())?;
        let t = eng.telemetry();
        let readout: Vec<Value> = eng
            .readout(readout_k)
            .iter()
            .map(|(i, v)| json!({"neuron": i, "activation": ssn_round(*v)}))
            .collect();
        let report = json!({
            "telemetry": Self::ssn_telemetry_json(id, &t),
            "readout": readout,
            "live_sessions": live,
            "note": "мозг спокоен (шага не было): снимок состояния",
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// poler_ssn_eject: выгрузить живые мозги из RAM (session=id | all).
    fn tool_ssn_eject(&self, args: &Value) -> Result<String, String> {
        let id = args
            .get("session")
            .and_then(|v| v.as_str())
            .unwrap_or("all");
        let n = self
            .warm_ssn
            .lock()
            .expect("poler-mcp: отравленный ssn-лок")
            .eject(id);
        let report = json!({
            "ejected": n,
            "requested": id,
            "note": "живые мозги выгружены из RAM",
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// S2/v0.36.0: poler_triune_speak — Триединство говорит от промпта.
    /// Без session — создаёт новую сессию (seed/gamma/tokens); с session —
    /// продолжает ту же (мозг, муха и контекст речи живут между вызовами).
    fn tool_triune_speak(&self, args: &Value) -> Result<String, String> {
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or("укажите text — сенсорный вход (о чём говорить)")?;
        let tokens = args
            .get("tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(24)
            .clamp(1, 256) as usize;
        let session_id = args.get("session").and_then(|v| v.as_str());

        let (id, arc) = match session_id {
            Some(id) => {
                let arc = self
                    .warm_triune
                    .lock()
                    .expect("poler-mcp: отравленный triune-лок")
                    .get(id)
                    .ok_or_else(|| {
                        format!("Триединство «{id}» не найдено (создайте: poler_triune_speak без session)")
                    })?;
                (id.to_string(), arc)
            }
            None => {
                let seed = args.get("seed").and_then(|v| v.as_u64()).unwrap_or(777);
                let gamma = args.get("gamma").and_then(|v| v.as_f64()).unwrap_or(0.8).clamp(0.0, 4.0);
                let crystal = crate::triune::Crystal::embedded()
                    .map_err(|e| format!("зашитый кристалл повреждён: {e}"))?;
                let fly = crate::triune::FlyPulse::synthetic(seed, gamma);
                let core = crate::triune::TriuneCore::new(
                    crystal,
                    crate::triune::TriuneConfig::default(),
                    fly,
                    seed,
                );
                let mut guard = self
                    .warm_triune
                    .lock()
                    .expect("poler-mcp: отравленный triune-лок");
                let (id, arc) = guard.insert(core);
                let _ = guard.len();
                (id, arc)
            }
        };

        let mut sess = arc
            .lock()
            .map_err(|_| "poler-mcp: Триединство отравлено".to_string())?;
        let u = sess.core.speak(text, tokens);
        sess.last_text = u.text.clone();
        sess.last_tokens = u.trace.len();

        // Провенанс: голова трейса (агенту хватает 5 токенов для вкуса).
        let r4 = |x: f64| (x * 1e4).round() / 1e4;
        let trace_head: Vec<Value> = u
            .trace
            .iter()
            .take(5)
            .map(|t| {
                json!({
                    "token": t.token, "sem": r4(t.sem), "gate": r4(t.gate),
                    "syn": t.syn, "fly": r4(t.fly), "score": r4(t.score), "tau": r4(t.tau),
                })
            })
            .collect();
        let intents: Vec<Value> = u
            .intents
            .iter()
            .map(|i| {
                json!({
                    "verb": i.verb, "op": i.op.as_str(), "object": i.object,
                    "proposal": i.proposal, "allowed": i.allowed,
                })
            })
            .collect();
        let t = &u.telemetry;
        let fly = match &u.fly_origin {
            crate::triune::PulseOrigin::Connectome { members, top_rotor } => json!({
                "kind": "connectome", "members": members, "top_rotor": top_rotor
            }),
            crate::triune::PulseOrigin::Synthetic { seed } => {
                json!({"kind": "synthetic", "seed": seed})
            }
        };
        let report = json!({
            "session": id,
            "prompt": text,
            "speech": u.text,
            "tokens": u.trace.len(),
            "crystal_vocab": u.crystal_vocab,
            "fly": fly,
            "brain": {
                "activity": ssn_round(t.activity), "synchrony": ssn_round(t.synchrony),
                "criticality": ssn_round(t.criticality), "ei": ssn_round(t.ei),
                "da": ssn_round(t.da), "ht_5": ssn_round(t.ht), "ne": ssn_round(t.ne),
            },
            "trace_head": trace_head,
            "motor_intents": intents,
            "note": "муха крутит, вихрь дышит, кристалл говорит; интенты — только \
предложения, исполнение остаётся за агентом (poler_exec)",
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// S2/v0.36.0: poler_triune_state — снимок говорящего Триединства.
    fn tool_triune_state(&self, args: &Value) -> Result<String, String> {
        let id = args
            .get("session")
            .and_then(|v| v.as_str())
            .ok_or("укажите session (id говорящего Триединства)")?;
        // Лок берём ОДИН раз: повторный lock внутри замыкания — дедлок.
        let (found, live) = {
            let guard = self
                .warm_triune
                .lock()
                .expect("poler-mcp: отравленный triune-лок");
            (guard.get(id), guard.len())
        };
        let arc = found.ok_or_else(|| {
            format!(
                "Триединство «{id}» не найдено (живых: {live}; создайте: poler_triune_speak без session)"
            )
        })?;
        let sess = arc
            .lock()
            .map_err(|_| "poler-mcp: Триединство отравлено".to_string())?;
        let (inj, spoken, ctx) = sess.core.state_summary();
        let t = sess.core.telemetry();
        let report = json!({
            "session": id,
            "injections": inj,
            "tokens_spoken": spoken,
            "ctx_window": ctx,
            "last_speech": sess.last_text,
            "last_tokens": sess.last_tokens,
            "brain": {
                "step": t.step, "activity": ssn_round(t.activity),
                "synchrony": ssn_round(t.synchrony), "criticality": ssn_round(t.criticality),
                "ei": ssn_round(t.ei), "da": ssn_round(t.da),
                "ht_5": ssn_round(t.ht), "ne": ssn_round(t.ne),
                "w_min": ssn_round(t.w_min), "w_max": ssn_round(t.w_max),
            },
            "note": "снимок без шага: мозг спит между фразами, но помнит всё",
        });
        Ok(serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()))
    }

    /// Имя режима Ψ для JSON.
/// v0.48.0: poler_calc — Калькулятор Всего для агентов (Antigravity).
    ///
    /// Состояние (переменные, ans) — резидентное: сквозные вычисления
    /// между вызовами. Возвращает структурированный JSON.
    fn tool_calc(&self, args: &Value) -> Result<String, String> {
        let expr = args
            .get("expression")
            .and_then(|v| v.as_str())
            .ok_or("аргумент expression обязателен: {\"expression\": \"2^10\"}")?;
        let mut calc = self
            .warm_calc
            .lock()
            .map_err(|_| "calc: состояние занято".to_string())?;
        Ok(calc.eval_structured(expr).to_string())
    }

    /// v0.73.0: poler_scivoice — «голос учёного» для внешних агентов.
    ///
    /// Тот же речевой контур, что слышит пользователь из `calc speak`:
    /// токенизатор → роторы 104k No-Mul ASM (языки мира + 118 элементов) →
    /// триединое ядро → трит-вердикт. Для формул из STO-3G-библиотеки
    /// (H2, He, LiH) голос добавляет секцию ab initio RHF/STO-3G —
    /// настоящий Хартри–Фок из POLER-ERI (E_total, ε_MO, SCF-итерации).
    /// mode="hf" — прямой расчёт с машиночитаемым JSON-отчётом.
    fn tool_scivoice(&self, args: &Value) -> Result<String, String> {
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or("аргумент text обязателен: {\"text\": \"C6H12O6\"}")?;
        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("read");
        match mode {
            "hf" => {
                let res = crate::quantum::eri::hartree_fock(text)?;
                Ok(json!({
                    "voice": "scivoice",
                    "mode": "hf",
                    "molecule": res.molecule,
                    "method": res.method,
                    "energy_hartree": res.energy_hartree,
                    "energy_ev": res.energy_hartree * 27.211386245988,
                    "e_nuc_hartree": res.e_nuc,
                    "n_basis": res.n_basis,
                    "n_electrons": res.n_electrons,
                    "occupied_orbital_energies_hartree": res.occupied_orbital_energies,
                    "scf_converged": res.converged,
                    "scf_iterations": res.iterations,
                    "reading": crate::quantum::eri::hf_report(&res)
                })
                .to_string())
            }
            "molecule" => {
                // v0.74.0: паспорт молекулы из SMILES/имени (лестница молекул)
                let g = crate::chem::resolve_input(text)?;
                let d = crate::chem::descriptors(&g);
                Ok(json!({
                    "voice": "scivoice",
                    "mode": "molecule",
                    "input": text,
                    "formula": g.hill_formula(),
                    "heavy_atoms": g.atoms.len(),
                    "total_atoms": d.total_atoms,
                    "charge": g.total_charge(),
                    "rings": d.rings,
                    "aromatic_rings": d.aromatic_rings,
                    "rotatable_bonds": d.rotatable,
                    "h_bond_donors": d.h_bond_donors,
                    "h_bond_acceptors": d.h_bond_acceptors,
                    "logp_estimate": crate::chem::logp_estimate(&g),
                    "ring_sizes": g.rings.iter().map(|r| r.len()).collect::<Vec<_>>(),
                    "reading": crate::calc::functions::smiles_passport(&g, text)
                })
                .to_string())
            }
            "bio" => {
                // v0.74.0: био-контур связывания лиганд ⇄ мишень
                let target = args
                    .get("target")
                    .and_then(|v| v.as_str())
                    .ok_or("режим bio требует аргумент target: {\"text\": \"дофамин\", \"target\": \"серотонин\"}")?;
                let res = crate::chem::binding_report(text, target)?;
                Ok(json!({
                    "voice": "scivoice",
                    "mode": "bio",
                    "ligand": res.ligand_formula,
                    "target": res.target_formula,
                    "delta_g_kj_mol": res.delta_g,
                    "e_vdw_kj_mol": res.e_vdw,
                    "e_hbond_kj_mol": res.e_hbond,
                    "e_elec_kj_mol": res.e_elec,
                    "entropy_kj_mol": res.e_entropy,
                    "ln_kd": res.ln_kd,
                    "log10_kd": res.log10_kd,
                    "trit_depth_kd": res.trit_depth,
                    "contacts": res.contacts,
                    "hbonds": res.hbonds,
                    "de9im_pattern": res.de9im_pattern,
                    "de9im_verdict": res.de9im_verdict,
                    "tree27": {"objects": res.tree27.0, "nodes": res.tree27.1, "height": res.tree27.2},
                    "trit_verdict": res.trit_verdict,
                    "reading": crate::chem::binding_text(&res)
                })
                .to_string())
            }
            _ => Ok(json!({
                "voice": "scivoice",
                "mode": "read",
                "text": text,
                "reading": crate::scivoice::scientist_read(text)
            })
            .to_string()),
        }
    }
    /// v0.76.0: poler_jit — «голос» внутреннего компилятора для внешних агентов.
    ///
    /// mode="bench": компилирует случайный matvec-граф (сид-детерминизм) в
    /// машинный код x86_64 и проверяет бит-в-бит против эталона — тот же
    /// инвариант, что в тестах A1. mode="linker": CausalLinker с подсаженным
    /// дефектом — внешние агенты слышат речь компилятора-арбитра.
    fn tool_jit(&self, args: &Value) -> Result<String, String> {
        use crate::graph::graph_asm::{
            DataflowCompiler, DataflowGraph, NodeAct, Reducer, WeightKind,
        };
        use crate::triune::jit_loop::ExecutableKernel;

        let mode = args.get("mode").and_then(|m| m.as_str()).unwrap_or("bench");
        match mode {
            "bench" => {
                let rows = args.get("rows").and_then(|v| v.as_u64()).unwrap_or(64).clamp(2, 256) as usize;
                let cols = args.get("cols").and_then(|v| v.as_u64()).unwrap_or(48).clamp(2, 256) as usize;
                let lcg = |s: &mut u64| {
                    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    ((*s >> 33) as i32 as f32 / i32::MAX as f32)
                };
                let mut g = DataflowGraph::new(cols, 0.02);
                g.add_layer(rows, NodeAct::Identity, Reducer::Sum);
                let mut s = 0xC0FFEEu64;
                let mut w = Vec::with_capacity(rows * cols);
                for _ in 0..rows * cols {
                    w.push(lcg(&mut s));
                }
                for i in 0..rows {
                    for j in 0..cols {
                        g.add_edge(0, j, i, WeightKind::F32(w[i * cols + j]));
                    }
                }
                let x: Vec<f32> = (0..cols).map(|_| lcg(&mut s)).collect();
                let compiled = DataflowCompiler::compile_x86_64(&g, "mcp_jit")?;
                let kern = ExecutableKernel::load(&compiled.machine_bytes)?;
                let ref_y = crate::graph::graph_asm::dataflow_reference_eval(&g, &x)?;
                let mut y = vec![0.0f32; rows];
                unsafe { kern.call_raw(x.as_ptr(), y.as_mut_ptr()) };
                let bitexact = ref_y.iter().zip(&y).all(|(a, b)| a.to_bits() == b.to_bits());
                let out = json!({
                    "mode": "bench",
                    "graph": {"inputs": cols, "outputs": rows, "edges": g.active_edges(),
                              "vacuum_pct": (g.sparsity_ratio() * 100.0)},
                    "machine_code": {"instructions": compiled.instruction_count,
                                     "bytes": compiled.machine_bytes.len(),
                                     "bytes_per_weight": compiled.machine_bytes.len() as f32 / g.active_edges().max(1) as f32},
                    "bitexact_ieee754": bitexact,
                    "wx": "PROT_READ|PROT_EXEC (W^X: запись и исполнение никогда вместе)",
                    "weights_in_code": "вес = immediate-операнд инструкции (mov eax,<f32>→movd→mulss); триты ±1 → addss/subss No-Mul"
                });
                Ok(out.to_string())
            }
            "linker" => {
                use crate::graph::causal_linker::{
                    link, ModuleSpec, PipelineLink, PipelineSpec,
                };
                let defect = args.get("defect").and_then(|d| d.as_str()).unwrap_or("none");
                // конвейер-образец: источник → слой → потребитель
                let mut layer = ModuleSpec::linear("dense", 9, 4, Vec::new());
                for j in 0..9 {
                    layer.edges.push(((0, j), (0, j % 4), WeightKind::F32(0.2)));
                }
                let mut consumer = ModuleSpec::linear("sink", 4, 2, Vec::new());
                for j in 0..4 {
                    consumer.edges.push(((0, j), (0, j % 2), WeightKind::Trit { val: 1, scale: 1.0 }));
                }
                let modules = vec![ModuleSpec::source("src", 9), layer, consumer];
                let links = vec![
                    PipelineLink { from_module: 0, from_port: 0, to_module: 1, to_port: 0 },
                    PipelineLink { from_module: 1, from_port: 0, to_module: 2, to_port: 0 },
                ];
                let mut spec = PipelineSpec { modules, links };
                match defect {
                    "width" => spec.modules[1].outputs[0].width = 6, // 9 vs 6 — сужение
                    "kind" => spec.modules[2].inputs[0].kind = crate::graph::causal_linker::PortKind::TritVector { depth: 3 },
                    "depth" => {
                        spec.modules[1].outputs[0].kind = crate::graph::causal_linker::PortKind::TritVector { depth: 2 };
                        spec.modules[2].inputs[0].kind = crate::graph::causal_linker::PortKind::TritVector { depth: 5 };
                    }
                    "norm" => spec.modules[2].inputs[0].norm_unit = true,
                    "alloc" => spec.modules[1].hot_allocations = 3,
                    "wx" => spec.modules[1].wx_compliant = false,
                    _ => {}
                }
                let out = match link(&spec) {
                    Ok(linked) => json!({
                        "mode": "linker",
                        "defect": defect,
                        "verdict": "linked",
                        "speech": linked.report.speech(),
                        "runtime_stages": linked.runtime_stages.iter().map(|s| json!({"module": s.module, "purpose": s.purpose, "width": s.width})).collect::<Vec<_>>(),
                        "graph_layers": linked.graph.layers.iter().map(|l| json!({"width": l.width, "act": format!("{:?}", l.act)})).collect::<Vec<_>>()
                    }),
                    Err(report) => json!({
                        "mode": "linker",
                        "defect": defect,
                        "verdict": "rejected",
                        "speech": report.speech()
                    }),
                };
                Ok(out.to_string())
            }
            other => Err(format!("неизвестный mode: {other} (bench | linker)")),
        }
    }


    /// v0.48.0: poler_hw — зонд скрытых параметров ПК (JSON).
    fn tool_hw(&self, _args: &Value) -> Result<String, String> {
        let report = crate::calc::hardware::probe();
        Ok(report.to_json())
    }

    /// v0.51.0 (цикл P): poler_quantum — квантовый мост для ИИ-агентов.
    ///
    /// Прямой доступ к идеальному симулятору POLER Quantum PC: запуск
    /// эталонных схем, телепортация, сфера Блоха и формальная верификация
    /// (точное кольцо ℤ[1/√2, i]). Возвращает структурированный JSON.
    fn tool_quantum(&self, args: &Value) -> Result<String, String> {
        use crate::quantum::core as pqc_core;
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("аргумент action обязателен: run | qcasm | qaoa | teleport | bloch | verify | list")?;
        let get_usize = |k: &str| args.get(k).and_then(|v| v.as_u64()).map(|v| v as usize);
        let get_u64 = |k: &str| args.get(k).and_then(|v| v.as_u64());
        let get_f64 = |k: &str| args.get(k).and_then(|v| v.as_f64());

        match action {
            "list" => {
                let catalog: Vec<Value> = pqc_core::algorithms::catalog()
                    .iter()
                    .map(|(n, d)| json!({"algorithm": n, "description": d}))
                    .collect();
                Ok(json!({
                    "engine": "poler-quantum-mcp",
                    "version": env!("CARGO_PKG_VERSION"),
                    "algorithms": catalog,
                    "actions": ["run", "qcasm", "qaoa", "teleport", "bloch", "verify", "list"],
                    "noise_presets": ["ideal", "ibm-heron", "google-willow", "noisy-90s"],
                    "physics_bridge": "poler_calc: schrodinger(H, psi0, t), pauli_x/y/z, kron, expm, eigen"
                })
                .to_string())
            }
            "qcasm" => {
                // Цикл Q: произвольная QCASM-схема — исходник прямо в аргументах
                // (или путь к файлу).
                let source = match args.get("source").and_then(|v| v.as_str()) {
                    Some(s) if !s.trim().is_empty() => s.to_string(),
                    _ => match args.get("path").and_then(|v| v.as_str()) {
                        Some(p) => std::fs::read_to_string(p)
                            .map_err(|e| format!("qcasm: не читается {p}: {e}"))?,
                        None => {
                            return Err(
                                "qcasm: аргумент source (текст схемы) или path (файл) обязателен"
                                    .into()
                            )
                        }
                    },
                };
                let circuit = pqc_core::qpc::Circuit::parse(&source)
                    .map_err(|e| format!("qcasm: ошибка разбора: {e:?}"))?;
                let shots = get_u64("shots").unwrap_or(1024);
                let seed = get_u64("seed").unwrap_or(42);
                // шум поверх идеала (цикл Q)
                if let Some(preset) = args.get("noise").and_then(|v| v.as_str()) {
                    let model = pqc_core::noise::NoiseModel::preset(preset)
                        .ok_or_else(|| format!("qcasm: неизвестный пресет {preset}"))?;
                    let nrep = pqc_core::noise::run_noisy(&circuit, &model, shots, seed)
                        .map_err(|e| format!("qcasm noise: {e:?}"))?;
                    let counts: Vec<Value> = nrep
                        .counts
                        .iter()
                        .map(|(o, c)| json!([o, c]))
                        .collect();
                    return Ok(json!({
                        "engine": "poler-quantum-mcp",
                        "action": "qcasm",
                        "mode": "noise-mcwf",
                        "n_qubits": nrep.n_qubits,
                        "preset": preset,
                        "shots": nrep.shots,
                        "tvd": nrep.tvd,
                        "classical_fidelity": nrep.classical_fidelity,
                        "chi2": nrep.chi2,
                        "chi2_dof": nrep.chi2_dof,
                        "ideal_peak": nrep.ideal_peak,
                        "noisy_peak": nrep.noisy_peak,
                        "ideal_probs": nrep.ideal_probs,
                        "counts": counts,
                    })
                    .to_string());
                }
                let rep = pqc_core::qpc::run(&circuit, shots, seed)
                    .map_err(|e| format!("qcasm: {e:?}"))?;
                let counts: Vec<Value> = rep
                    .counts
                    .iter()
                    .map(|(o, c)| json!([o, c]))
                    .collect();
                Ok(json!({
                    "engine": "poler-quantum-mcp",
                    "action": "qcasm",
                    "n_qubits": rep.n_qubits,
                    "gate_count": rep.gate_count,
                    "per_shot": rep.per_shot,
                    "shots": rep.shots,
                    "norm": rep.norm,
                    "entropy_bits": rep.entropy_bits,
                    "landauer_j": rep.landauer_j,
                    "probabilities": rep.probabilities,
                    "marginals": rep.marginals,
                    "counts": counts,
                })
                .to_string())
            }
            "qaoa" => {
                // Цикл Q: MaxCut-ансатц с классической оптимизацией.
                let edges: Vec<(usize, usize)> = if let Some(arr) =
                    args.get("edges").and_then(|v| v.as_array())
                {
                    let mut v = Vec::new();
                    for e in arr {
                        let pair = e
                            .as_array()
                            .ok_or("qaoa: edges — массив пар [i, j] или строка \"0-1,1-2\"" )?;
                        if pair.len() != 2 {
                            return Err("qaoa: ребро — ровно две вершины".into());
                        }
                        let i = pair[0].as_u64().ok_or("qaoa: вершины — целые")? as usize;
                        let j = pair[1].as_u64().ok_or("qaoa: вершины — целые")? as usize;
                        v.push((i, j));
                    }
                    v
                } else if let Some(s) = args.get("edges").and_then(|v| v.as_str()) {
                    pqc_core::qaoa::MaxCut::parse_edges(s)
                        .map_err(|e| format!("qaoa: {e:?}"))?
                } else {
                    // демо-граф: 4-цикл + диагональ
                    pqc_core::qaoa::MaxCut::demo().edges
                };
                let n = get_usize("n").unwrap_or_else(|| {
                    edges.iter().map(|&(i, j)| i.max(j)).max().unwrap_or(0) + 1
                });
                let problem = pqc_core::qaoa::MaxCut::new(n, edges.clone())
                    .map_err(|e| format!("qaoa: {e:?}"))?;
                let cfg = pqc_core::qaoa::QaoaConfig {
                    p: get_usize("p").unwrap_or(2).clamp(1, pqc_core::qaoa::MAX_P),
                    sweeps: get_usize("sweeps").unwrap_or(12),
                    restarts: get_usize("restarts").unwrap_or(3),
                    shots: get_u64("shots").unwrap_or(1024),
                    seed: get_u64("seed").unwrap_or(42),
                };
                let rep = pqc_core::qaoa::run_qaoa(&problem, &cfg)
                    .map_err(|e| format!("qaoa: {e:?}"))?;
                let bitstring = |o: u64| -> String {
                    let mut s = String::new();
                    for q in (0..rep.n_qubits).rev() {
                        s.push(if o >> q & 1 == 1 { '1' } else { '0' });
                    }
                    s
                };
                let counts: Vec<Value> = rep
                    .counts
                    .iter()
                    .map(|(o, c)| json!([o, c]))
                    .collect();
                Ok(json!({
                    "engine": "poler-quantum-mcp",
                    "action": "qaoa",
                    "problem": "maxcut",
                    "n_qubits": rep.n_qubits,
                    "edges": rep.edges.iter().map(|(i, j)| json!([i, j])).collect::<Vec<_>>(),
                    "p": rep.p,
                    "gammas": rep.params[..rep.p],
                    "betas": rep.params[rep.p..],
                    "expected_cut": rep.expected_cut,
                    "expected_cut_init": rep.expected_cut_init,
                    "best_bits": bitstring(rep.best_bits),
                    "best_cut": rep.best_cut,
                    "optimum": rep.optimum,
                    "approx_ratio": rep.approx_ratio,
                    "evals": rep.evals,
                    "gate_count": rep.gate_count,
                    "shots": rep.shots,
                    "norm": rep.norm,
                    "counts": counts,
                })
                .to_string())
            }
            "run" => {
                let name = args
                    .get("algorithm")
                    .and_then(|v| v.as_str())
                    .ok_or("run: аргумент algorithm обязателен (bell ghz qft iqft grover bv dj period teleport)")?;
                let params = pqc_core::algorithms::AlgoParams {
                    n: get_usize("n").unwrap_or(4),
                    marks: match args.get("marks").and_then(|v| v.as_array()) {
                        Some(arr) => arr
                            .iter()
                            .filter_map(|v| v.as_u64().map(|x| x as usize))
                            .collect(),
                        None => Vec::new(),
                    },
                    secret: get_u64("secret").unwrap_or(0b1011),
                    period: get_usize("period").unwrap_or(0),
                    offset: get_usize("offset").unwrap_or(0),
                    theta: get_f64("theta").unwrap_or(0.7),
                };
                let (circuit, iterations) = pqc_core::algorithms::build(name, &params)
                    .map_err(|e| format!("run {name}: {e:?}"))?;
                let shots = get_u64("shots").unwrap_or(1024);
                let seed = get_u64("seed").unwrap_or(42);
                let rep = pqc_core::qpc::run(&circuit, shots, seed)
                    .map_err(|e| format!("run {name}: {e:?}"))?;
                let counts: Vec<Value> = rep
                    .counts
                    .iter()
                    .map(|(o, c)| json!([o, c]))
                    .collect();
                Ok(json!({
                    "engine": "poler-quantum-mcp",
                    "action": "run",
                    "algorithm": name,
                    "n_qubits": rep.n_qubits,
                    "gate_count": rep.gate_count,
                    "iterations": iterations,
                    "shots": rep.shots,
                    "norm": rep.norm,
                    "entropy_bits": rep.entropy_bits,
                    "landauer_j": rep.landauer_j,
                    "probabilities": rep.probabilities,
                    "marginals": rep.marginals,
                    "counts": counts,
                })
                .to_string())
            }
            "teleport" => {
                let exact = args.get("exact").and_then(|v| v.as_bool()).unwrap_or(false);
                let theta = get_f64("theta").unwrap_or(0.7);
                let t = if exact {
                    pqc_core::algorithms::run_teleport_exact()
                } else {
                    pqc_core::algorithms::run_teleport(theta)
                }
                .map_err(|e| format!("teleport: {e:?}"))?;
                Ok(json!({
                    "engine": "poler-quantum-mcp",
                    "action": "teleport",
                    "mode": if t.exact { "exact:Z[1/sqrt(2),i]" } else { "numeric:f64" },
                    "theta": t.theta,
                    "psi_in": [t.psi_in[0].0, t.psi_in[0].1, t.psi_in[1].0, t.psi_in[1].1],
                    "psi_out": [t.psi_out[0].0, t.psi_out[0].1, t.psi_out[1].0, t.psi_out[1].1],
                    "fidelity": t.fidelity,
                    "branch_deviation": t.branch_deviation,
                    "exact": t.exact,
                    "bloch_in": t.bloch_in,
                    "bloch_out": t.bloch_out,
                    "gate_count": t.gate_count,
                })
                .to_string())
            }
            "bloch" => {
                // Амплитуды — выражения Калькулятора Всего (резидентное
                // состояние warm_calc: константы и переменные доступны).
                let a_src = args
                    .get("alpha")
                    .and_then(|v| v.as_str())
                    .ok_or("bloch: аргумент alpha обязателен (выражение или число)")?;
                let b_src = args.get("beta").and_then(|v| v.as_str()).unwrap_or("0");
                let mut calc = self
                    .warm_calc
                    .lock()
                    .map_err(|_| "bloch: состояние занято".to_string())?;
                let alpha = calc
                    .eval_complex(a_src)
                    .map_err(|e| format!("bloch: α — {e}"))?;
                let beta = calc
                    .eval_complex(b_src)
                    .map_err(|e| format!("bloch: β — {e}"))?;
                let norm_sq =
                    alpha.0 * alpha.0 + alpha.1 * alpha.1 + beta.0 * beta.0 + beta.1 * beta.1;
                if norm_sq < 1e-30 {
                    return Err("bloch: нулевое состояние".into());
                }
                let k = 1.0 / norm_sq.sqrt();
                let (alpha, beta) = ((alpha.0 * k, alpha.1 * k), (beta.0 * k, beta.1 * k));
                let b = pqc_core::algorithms::bloch_of(alpha, beta);
                let p0 = alpha.0 * alpha.0 + alpha.1 * alpha.1;
                Ok(json!({
                    "engine": "poler-quantum-mcp",
                    "action": "bloch",
                    "normalized": (norm_sq - 1.0).abs() > 1e-9,
                    "psi": {
                        "alpha": [alpha.0, alpha.1],
                        "beta": [beta.0, beta.1],
                        "p0": p0,
                        "p1": 1.0 - p0,
                        "relative_phase_rad": beta.1.atan2(beta.0),
                    },
                    "bloch": {"x": b[0], "y": b[1], "z": b[2]},
                })
                .to_string())
            }
            "verify" => {
                use pqc_core::verify::{Verdict, VerificationReport};
                let check = args
                    .get("check")
                    .and_then(|v| v.as_str())
                    .ok_or("verify: аргумент check обязателен (unitary | equivalence | teleport)")?;
                let n = get_usize("n").unwrap_or(4);
                let build_algo = |name: &str| -> Result<pqc_core::qpc::Circuit, String> {
                    let p = pqc_core::algorithms::AlgoParams {
                        n,
                        marks: vec![22 % (1usize << n.max(1))],
                        secret: 0b1011,
                        period: 0,
                        offset: 0,
                        theta: 0.7,
                    };
                    pqc_core::algorithms::build(name, &p)
                        .map(|(c, _)| c)
                        .map_err(|e| format!("{e:?}"))
                };
                let report: VerificationReport = match check {
                    "unitary" => {
                        let algo = args
                            .get("algorithm")
                            .and_then(|v| v.as_str())
                            .ok_or("verify unitary: аргумент algorithm обязателен")?;
                        let circuit = build_algo(algo)?;
                        pqc_core::verify::verify_unitary(&circuit)
                            .map_err(|e| format!("{e:?}"))?
                    }
                    "equivalence" => {
                        let names = args
                            .get("algorithms")
                            .and_then(|v| v.as_array())
                            .ok_or("verify equivalence: аргумент algorithms — массив из двух имён")?;
                        if names.len() != 2 {
                            return Err(
                                "verify equivalence: algorithms — ровно два имени".into()
                            );
                        }
                        let a_name = names[0].as_str().ok_or("algorithms[0] — строка")?;
                        let b_name = names[1].as_str().ok_or("algorithms[1] — строка")?;
                        let a = build_algo(a_name)?;
                        let b = build_algo(b_name)?;
                        let up_to_phase = args
                            .get("up_to_global_phase")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        pqc_core::verify::verify_equivalence(&a, &b, up_to_phase)
                            .map_err(|e| format!("{e:?}"))?
                    }
                    "teleport" => pqc_core::verify::verify_teleport_channel()
                        .map_err(|e| format!("{e:?}"))?,
                    other => {
                        return Err(format!(
                            "verify: неизвестная проверка `{other}` (unitary | equivalence | teleport)"
                        ))
                    }
                };
                let verdict = match report.verdict {
                    Verdict::ProvedExact => json!("proved_exact"),
                    Verdict::VerifiedNumeric(tol) => json!(["verified_numeric", tol]),
                    Verdict::VerifiedSampling(tol) => json!(["verified_sampling", tol]),
                    Verdict::Refuted(d) => json!(["refuted", d]),
                };
                // Цикл Q: шум поверх верификатора — вердикт + «идеал vs железо».
                let mut noise: Option<Value> = None;
                if let Some(preset) = args.get("noise").and_then(|v| v.as_str()) {
                    let noise_shots = get_u64("noise_shots").unwrap_or(4096);
                    let seed = get_u64("seed").unwrap_or(42);
                    let circuit_opt: Option<pqc_core::qpc::Circuit> = match check {
                        "unitary" => args
                            .get("algorithm")
                            .and_then(|v| v.as_str())
                            .and_then(|a| build_algo(a).ok()),
                        "teleport" => pqc_core::algorithms::teleport_prepared(0.7).ok(),
                        _ => None,
                    };
                    if let Some(circuit) = circuit_opt {
                        let model = pqc_core::noise::NoiseModel::preset(preset)
                            .ok_or_else(|| format!("verify: неизвестный пресет {preset}"))?;
                        let nrep = pqc_core::noise::run_noisy(&circuit, &model, noise_shots, seed)
                            .map_err(|e| format!("verify noise: {e:?}"))?;
                        noise = Some(json!({
                            "preset": preset,
                            "shots": nrep.shots,
                            "tvd": nrep.tvd,
                            "classical_fidelity": nrep.classical_fidelity,
                            "chi2": nrep.chi2,
                            "chi2_dof": nrep.chi2_dof,
                            "ideal_peak": nrep.ideal_peak,
                            "noisy_peak": nrep.noisy_peak,
                        }));
                    }
                }
                Ok(json!({
                    "engine": "poler-quantum-mcp",
                    "action": "verify",
                    "property": report.property,
                    "subject": report.subject,
                    "n_qubits": report.n_qubits,
                    "dim": report.dim,
                    "gate_count": report.gate_count,
                    "method": report.method,
                    "verdict": verdict,
                    "max_deviation": report.max_deviation,
                    "checks": report.checks.iter().map(|(n, ok, d)| json!([n, ok, d])).collect::<Vec<_>>(),
                    "notes": report.notes,
                    "noise": noise,
                })
                .to_string())
            }
            other => Err(format!(
                "неизвестное действие `{other}` (run | qcasm | qaoa | teleport | bloch | verify | list)"
            )),
        }
    }

    fn psi_mode_name(m: PsiMode) -> &'static str {
        match m {
            PsiMode::Attraction => "attraction",
            PsiMode::Refraction => "refraction",
            PsiMode::ObserverKill => "observer_kill",
            PsiMode::Superconductivity => "superconductivity",
        }
    }

    /// NarrativeReport → JSON (телеметрия сжата до ключевых точек).
    fn psi_report_json(r: &NarrativeReport, session: Option<&str>) -> Value {
        let tel_cap = 12;
        let stride = (r.telemetry.len() / tel_cap).max(1);
        let mut timeline: Vec<Value> = r
            .telemetry
            .iter()
            .step_by(stride)
            .map(Self::psi_step_json)
            .collect();
        // Финальный шаг — всегда в отчёте (даже если stride его пропустил).
        if let Some(t) = r.telemetry.last() {
            let last_step = t.step as u64;
            if !timeline
                .iter()
                .any(|x| x.get("step").and_then(|s| s.as_u64()) == Some(last_step))
            {
                timeline.push(Self::psi_step_json(t));
            }
        }
        json!({
            "session": session,
            "steps": r.steps,
            "converged": r.converged,
            "f_final": psi_round(r.final_f),
            "h_psi_final": psi_round(r.final_h_psi),
            "sigma_final": psi_round(r.final_sigma),
            "causality_clean": r.causality_clean,
            "refractions": r.refractions,
            "params": {
                "dims": r.params.dims,
                "eta": r.params.eta,
                "eta_r": r.params.eta_r,
                "rho": r.params.rho,
                "kappa": r.params.kappa,
                "gamma": r.params.gamma,
                "lambda": r.params.lambda_rl,
                "no_mul": r.params.no_mul,
            },
            "acts": r.acts.iter().map(|a| json!({
                "steps": [a.steps.0, a.steps.1],
                "f": [psi_round(a.f_start), psi_round(a.f_end)],
                "dominants": a.dominants.iter().map(|d| json!({
                    "archetype": d.name, "weight": psi_round(d.weight),
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "dramatic_pairs": r.dramatic_pairs.iter().map(|p| json!({
                "u": p.u, "v": p.v, "j": psi_round(p.j),
                "u_archetype": p.u_archetype, "v_archetype": p.v_archetype,
            })).collect::<Vec<_>>(),
            "timeline": timeline,
            "text": r.text,
        })
    }

    /// Шаг телеметрии → компактный JSON.
    fn psi_step_json(t: &StepTelemetry) -> Value {
        json!({
            "step": t.step,
            "f": psi_round(t.f_free),
            "epsilon": psi_round(t.epsilon),
            "sigma": psi_round(t.sigma_t),
            "h_psi": psi_round(t.h_psi),
            "mode": Self::psi_mode_name(t.mode),
        })
    }

    // -----------------------------------------------------------------
    // poler_grep: точный поиск в семантике grep (слой 0 Native Retrieval)
    // -----------------------------------------------------------------
    fn tool_grep(&self, args: &Value) -> Result<String, String> {
        use crate::retrieval as nr;
        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or("аргумент pattern обязателен")?
            .to_string();
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        // Аудит-фикс: ограждение локальных путей (анти-экфильтрация)
        guard_path(path)?;
        let mode = if args.get("regex").and_then(|v| v.as_bool()).unwrap_or(false) {
            nr::GrepMode::Regex
        } else {
            nr::GrepMode::Literal
        };
        let output = match args.get("output").and_then(|v| v.as_str()).unwrap_or("content") {
            "count" => nr::GrepOutput::Count,
            "list" => nr::GrepOutput::ListMatching,
            "list_nonmatching" => nr::GrepOutput::ListNonMatching,
            _ => nr::GrepOutput::Content,
        };
        let config = nr::GrepConfig {
            pattern,
            mode,
            case_insensitive: args
                .get("ignore_case")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            before: args.get("before").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            after: args.get("after").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            max_count: args
                .get("max_count")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize),
            output,
            include_hidden: false,
            respect_ignore: true,
            // v0.28.1: MCP-агент может включить скан архивов без распаковки
            // аргументами tools: {"archives": true, "archive_password": "…"}.
            scan_archives: args.get("archives").and_then(|v| v.as_bool()).unwrap_or(false),
            archive_password: args
                .get("archive_password")
                .and_then(|v| v.as_str())
                .map(String::from),
            archive_max_entry_bytes: 0,
        };
        let report = nr::grep_run_cached(&[PathBuf::from(path)], &config, &self.file_cache)
            .map_err(|e| format!("grep: {e}"))?;
        if args.get("json").and_then(|v| v.as_bool()).unwrap_or(false) {
            return serde_json::to_string_pretty(&report)
                .map_err(|e| format!("сериализация: {e}"));
        }
        let mut out = nr::render_text(&report, false);
        out.push_str(&format!(
            "\n[poler-grep] файлов: {}, с совпадениями: {}, строк: {}, время: {} мс\n",
            report.stats.files_scanned,
            report.stats.files_matched,
            report.stats.lines_matched,
            report.stats.elapsed_ms
        ));
        if !report.stats.errors.is_empty() {
            out.push_str(&format!("\nошибки обхода: {}\n", report.stats.errors.len()));
        }
        Ok(out)
    }

    // -----------------------------------------------------------------
    // poler_chunk: RAG-нарезка документа на чанки с якорями (слой B)
    // -----------------------------------------------------------------
    fn tool_chunk(&self, args: &Value) -> Result<String, String> {
        use crate::retrieval as nr;
        let (text, label, format) = if let Some(content) = args.get("content").and_then(|v| v.as_str()) {
            let label = args.get("title").and_then(|v| v.as_str())
                .or_else(|| args.get("url").and_then(|v| v.as_str()))
                .unwrap_or("weblens_capture.md");
            (content.to_string(), label.to_string(), nr::ChunkFormat::Markdown)
        } else if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
            // Аудит-фикс: ограждение локальных путей (анти-экфильтрация)
            guard_path(path)?;
            let target = PathBuf::from(path);
            if !target.is_file() {
                return Err(format!("не файл или не найден: {path}"));
            }
            let text = std::fs::read_to_string(&target)
                .map_err(|e| format!("прочитать {path}: {e}"))?;
            let fmt = nr::ChunkFormat::detect(&target);
            (text, path.to_string(), fmt)
        } else {
            return Err("аргумент path (файл) или content (текст) обязателен".to_string());
        };

        let config = nr::ChunkConfig {
            target_tokens: args
                .get("target_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(nr::DEFAULT_TARGET_TOKENS as u64) as usize,
            overlap_tokens: args
                .get("overlap_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(nr::DEFAULT_OVERLAP_TOKENS as u64) as usize,
            ..Default::default()
        };
        let report = nr::chunk_document(&text, format, &config);
        if args.get("json").and_then(|v| v.as_bool()).unwrap_or(false) {
            return serde_json::to_string_pretty(&report)
                .map_err(|e| format!("сериализация: {e}"));
        }
        Ok(nr::render_chunks_text(&report, &label))
    }

    // -----------------------------------------------------------------
    // poler_box_exec (v0.26.0): двухконтурный брокер — команда агента
    // судится sandbox-судьёй и исполняется ВНУТРИ изолированного
    // контейнера (runner → box), результат возвращается текстом.
    // -----------------------------------------------------------------
    fn tool_box_exec(&self, args: &Value) -> Result<String, String> {
        use crate::gateway::{containers, hostexec, pipeline, sandbox};

        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or("аргумент command обязателен (одиночная команда; кавычки — как в шлюзе)")?
            .to_string();
        if command.trim().is_empty() {
            return Err("command пуст".into());
        }
        let timeout_secs = args
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(120)
            .clamp(1, 600);
        let target = args
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("auto")
            .to_string();
        if !matches!(target.as_str(), "auto" | "runner" | "box") {
            return Err(format!("target={target}? (auto | runner | box)"));
        }

        // Разбор тем же лексером, что и REPL шлюза: кавычки раскрываются,
        // /bin/sh НЕ участвует (argv насквозь — нет класса shell-инъекций)
        let pl = pipeline::parse_line(&command)
            .map_err(|e| format!("не удалось разобрать команду: {e}"))?;

        // Вердикт ДО исполнения (контур 2 схемы) — по ВСЕМУ конвейеру:
        // exec-плоскость в контейнере судится без логической границы (её
        // держит контейнер — та же семантика, что у шлюза в jail), но
        // Block-инварианты (деструктив, привилегии, форк-бомбы, remote-exec)
        // действуют ВСЕГДА; Confirm из MCP не подтверждается в принципе
        // (Zero Silent Escalation — подтверждает только владелец).
        let all_host: Vec<usize> = (0..pl.segments.len()).collect();
        match sandbox::judge_pipeline_ws(&pl, &all_host, None) {
            sandbox::Policy::Allow => {}
            sandbox::Policy::Block(why) => {
                return Err(format!("⛔ блокировка sandbox: {why}"));
            }
            sandbox::Policy::Confirm(why) => {
                return Err(format!(
                    "⛔ {why} — подтверждение доступно только владельцу в шлюзе \
                     (allow <путь> / grant sudo); MCP-агент подтвердить не может"
                ));
            }
        }

        // Исполнение — только одиночная команда (без конвейера/редиректа):
        // брокер возвращает stdout/stderr целиком, конвейеры собираются
        // агентом из нескольких вызовов (bash -c '…' допустим — payload
        // судится рекурсивно тем же судьёй выше).
        if pl.segments.len() > 1 {
            return Err(
                "конвейеры не поддерживаются: вызывайте части отдельными вызовами poler_box_exec \
                 (bash -c '…' допустим — payload судится рекурсивно тем же судьёй)"
                    .into(),
            );
        }
        if pl.redirect.is_some() {
            return Err(
                "редиректы не поддерживаются: stdout/stderr и так возвращаются целиком".into(),
            );
        }
        let tokens = pl
            .segments
            .first()
            .map(|s| s.tokens.clone())
            .unwrap_or_default();
        if tokens.is_empty() {
            return Err("пустая команда".into());
        }

        // Discovery по workspace: сервис mcp НЕ разделяет память с gateway —
        // корень приходит через POLER_WORKSPACE (шлюз выставляет при
        // `service start mcp`), контейнеры находятся по docker-labels.
        let ws = std::env::var("POLER_WORKSPACE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            });
        containers::docker_probe().map_err(|e| {
            format!("docker недоступен: {e} (box runner on / box on — поднять контур исполнения в шлюзе)")
        })?;

        let runner = containers::runner_state_for_ws(&ws);
        let boxj = containers::box_state_for_ws(&ws);
        let (wrapped, target_name, target_kind) = match target.as_str() {
            "runner" => {
                let r = runner.ok_or(
                    "runner-контейнер не работает — box runner on (net=none, только /workspace)",
                )?;
                let name = r.name.clone();
                (containers::wrap_runner_exec(&r, &tokens, &ws), name, "runner")
            }
            "box" => {
                let j = boxj.ok_or("box-контейнер не работает — box on")?;
                let name = j.name.clone();
                (containers::wrap_host_exec(&j, &tokens, &ws), name, "box")
            }
            _ => {
                // auto: runner (жёстче: без сети, без home) → box → отказ
                if let Some(r) = runner {
                    let name = r.name.clone();
                    (containers::wrap_runner_exec(&r, &tokens, &ws), name, "runner")
                } else if let Some(j) = boxj {
                    let name = j.name.clone();
                    (containers::wrap_host_exec(&j, &tokens, &ws), name, "box")
                } else {
                    return Err(
                        "нет работающего контура исполнения: box runner on (предпочтительно) или box on"
                            .into(),
                    );
                }
            }
        };

        let limits = hostexec::HostLimits {
            timeout_secs,
            ..Default::default()
        };
        let outcome = hostexec::run(&wrapped, None, &limits, &ws, &[]);
        if let Some(e) = &outcome.spawn_error {
            return Err(format!("не удалось запустить docker-клиент: {e}"));
        }
        let code = outcome
            .code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "убит сигналом".into());
        let mut out = format!(
            "двухконтурный брокер: исполнено в {target_kind} {target_name} · exit {code}\n"
        );
        out.push_str(&format!("--- stdout ---\n{}", outcome.stdout));
        if !outcome.stderr.is_empty() {
            out.push_str(&format!("--- stderr ---\n{}", outcome.stderr));
        }
        if outcome.truncated {
            out.push_str("(вывод обрезан по капу)\n");
        }
        if outcome.timed_out {
            out.push_str(&format!("(таймаут {timeout_secs}s — процесс убит)\n"));
        }
        if outcome.interrupted {
            out.push_str("(прервано)\n");
        }
        Ok(out)
    }

    // -----------------------------------------------------------------
    // poler_box_status (v0.26.0): состояние jail-стека для workspace
    // -----------------------------------------------------------------
    fn tool_box_status(&self, _args: &Value) -> Result<String, String> {
        use crate::gateway::containers;
        let ws = std::env::var("POLER_WORKSPACE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            });
        let mut s = String::new();
        match containers::docker_probe() {
            Ok(v) => s.push_str(&format!("docker: сервер {v}\n")),
            Err(e) => {
                return Ok(format!(
                    "docker: недоступен ({e})\nконтуры исполнения не подняты: box on / box runner on — в шлюзе (poler_box_exec требует контура исполнения)\n"
                ))
            }
        }
        match containers::box_state_for_ws(&ws) {
            Some(j) => {
                let agents = if j.cfg.agent_names.is_empty() {
                    "—".to_string()
                } else {
                    j.cfg.agent_names.join(",")
                };
                s.push_str(&format!(
                    "box {}: работает · образ {} · агенты (ro-проброс): {agents}\n",
                    j.name, j.cfg.image
                ));
            }
            None => s.push_str(&format!(
                "box {}: не работает (box on в шлюзе — контуры 2/3 в контейнере)\n",
                containers::container_name(&ws)
            )),
        }
        match containers::runner_state_for_ws(&ws) {
            Some(r) => s.push_str(&format!(
                "runner {}: работает · образ {} · net {} · только /workspace\n",
                r.name, r.cfg.image, r.cfg.net
            )),
            None => s.push_str(&format!(
                "runner {}: не работает (box runner on — сюда poler_box_exec исполняет команды; net=none)\n",
                containers::runner_name(&ws)
            )),
        }
        s.push_str("poler_box_exec: target=auto → runner, при его отсутствии box; деструктив блокируется судьёй до исполнения\n");
        Ok(s)
    }
}

fn tools_manifest() -> Vec<Value> {
    let mut tools = vec![
        json!({
            "name": "poler_web_search",
            "description": "Веб-поиск по постоянному индексу poler-engine (собирается poler_crawl). \
Ранжирование WebRank: 0.55·BM25 + 0.15·PageRank + 0.20·title + 0.10·ε-плотность. \
Кириллица стеммингуется (укр/рос падежи унифицируются). Semantic Bridge (v0.21): \
кросс-языковые запросы (рус→англ корпус и обратно) расширяются офлайн-словарём, \
кандидаты подмешиваются с весом ≤0.85, ранжирование остаётся детерминированным, \
WHY-объяснение включается в ответ. Возвращает score, URL, заголовок, сниппет.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Поисковый запрос"},
                    "top": {"type": "integer", "default": 10, "minimum": 1, "maximum": 50}
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "poler_crawl",
            "description": "Обойти сайт (seed_url) и проиндексировать в постоянную БД: \
real-Chromium рендер (SPA/JS), robots.txt + sitemap, SimHash-дедуп, PageRank, \
инкрементальность (повторный обход переиндексирует только изменившееся). \
Агент сам выбирает цель. После краула используй poler_web_search.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "seed_url": {"type": "string", "description": "http(s)://… стартовый URL"},
                    "depth": {"type": "integer", "default": 2, "maximum": 5, "description": "Глубина обхода от seed"},
                    "max_pages": {"type": "integer", "default": 25, "minimum": 1, "maximum": 500},
                    "delay_ms": {"type": "integer", "default": 1000, "description": "Пауза между запросами к хосту"},
                    "cross_site": {"type": "boolean", "default": false, "description": "Переходить на другие хосты"},
                    "wait_ms": {"type": "integer", "default": 1200, "description": "Пауза на XHR после load"}
                },
                "required": ["seed_url"]
            }
        }),
        json!({
            "name": "poler_fetch",
            "description": "Открыть URL в реальном Chromium (CDP) и вернуть рендер-текст: \
JS исполнен, SPA/Shadow DOM отрендерены, скрытые JSON API перехвачены. \
Инструмент «прочитать любую страницу сейчас». Полный текст и JSON пишутся в кэш \
и доступны локальному POLER-поиску. Аутентификационные стены (логины, платный \
контент) движком не обходятся — возвращается то, что видит анонимный браузер.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "http(s)://… страница"},
                    "wait_ms": {"type": "integer", "default": 1200, "description": "Пауза на дочерние XHR"}
                },
                "required": ["url"]
            }
        }),
        json!({
            "name": "poler_search",
            "description": "Локальный резонансный POLER-поиск по файлу/директории: \
информационная плотность ε, резонанс R, полные логические сцены (не строки grep), \
K-hop связи сущностей. Работает и по веб-кэшу poler_fetch (/tmp/poler-web).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Файл или директория"},
                    "query": {"type": "string", "description": "Запрос: слово или фраза"},
                    "top": {"type": "integer", "default": 5, "minimum": 1, "maximum": 20}
                },
                "required": ["path", "query"]
            }
        }),
        json!({
            "name": "poler_grep",
            "description": "Точный поиск в семантике grep: ВСЕ совпадения по файлам, \
без индекса, гарантия полноты («ноль значит ноль»). Обход с .gitignore (ripgrep-класс), \
контекст -A/-B, счётчики, exit-статус в тексте. Замена внешнего grep/ripgrep: \
используй для точных строк, имён функций, конфигов — когда нужна ПОЛНОТА, \
а не релевантность (для релевантности бери poler_search/poler_web_search). \
С json=true возвращает машинный отчёт: byte_offset строк, байтовые диапазоны \
вхождений, статистика — для верификации и навигации по файлу.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Искомая строка или regex (см. флаг regex)"},
                    "path": {"type": "string", "default": ".", "description": "Файл или корень обхода"},
                    "regex": {"type": "boolean", "default": false, "description": "PATTERN — регулярное выражение (grep -E)"},
                    "ignore_case": {"type": "boolean", "default": false, "description": "Регистронезависимость, Unicode-fold"},
                    "before": {"type": "integer", "default": 0, "minimum": 0, "maximum": 50, "description": "Строк контекста ДО (grep -B)"},
                    "after": {"type": "integer", "default": 0, "minimum": 0, "maximum": 50, "description": "Строк контекста ПОСЛЕ (grep -A)"},
                    "max_count": {"type": "integer", "minimum": 1, "description": "Останов после N совпавших строк на файл (grep -m)"},
                    "output": {"type": "string", "enum": ["content", "count", "list", "list_nonmatching"], "default": "content"},
                    "json": {"type": "boolean", "default": false, "description": "Машинно-читаемый отчёт вместо текста"}
                },
                "required": ["pattern"]
            }
        }),
        json!({
            "name": "poler_calc",
            "description": "Калькулятор Всего: арифметика, единицы (to), уравнения (solve), \
матрицы expm/eigen/rot, триты POLER, астрономия (фазы Луны, планеты, восход), геодезия, \
теория чисел, специальная математика (gamma/erf/zeta). Константы CODATA/IAU/СИ-2019 \
встроены по именам (c, G, h, M_sun, R_earth, l_P…). Состояние резидентное: переменные \
и ans сохраняются между вызовами. Примеры: 2^10; 5 km to mi; solve x^2-4=0; \
expm([0,-1;1,0]*psi); moon_illum(2024,4,8,18.35); dist(50.45,30.52,49.84,24.03).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "Выражение калькулятора"}
                },
                "required": ["expression"]
            }
        }),
        json!({
            "name": "poler_hw",
            "description": "Скрытые параметры ПК: кеши L1d/L1i/L2/L3, ISA-флаги (AVX/AVX-512/AES), \
топология (сокеты/ядра/потоки/NUMA), bogomips, диски, GPU (nvidia-smi или PCI sysfs), \
гипервизор. Возвращает JSON.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "poler_quantum",
            "description": "Квантовый мост POLER (циклы P+Q): идеальный симулятор кубитов без \
декогеренции. Действия: run — эталонные схемы (bell, ghz, qft, iqft, grover, bv, dj, period, \
teleport) с распределением Борна и энтропией; teleport — телепортация q0→q2 с фиделити \
(theta — препарат Ry, exact — кольцо Z[1/sqrt(2),i]); bloch — сфера Блоха состояния \
(амплитуды — выражения poler_calc: 1/sqrt(2), i/2…); verify — формальная верификация: \
unitary (U†U = I), equivalence (две схемы, до глобальной фазы), teleport (канал на базисе) — \
вердикты proved_exact / verified_numeric / verified_sampling / refuted; noise в verify — \
шум железа поверх вердикта (TVD, фиделити, χ²); qcasm — произвольная схема QCASM-lite \
(source/path, noise); qaoa — MaxCut-ансатц с оптимизацией углов; list — каталог. \
Физика уровней/спектров — через poler_calc (schrodinger, pauli_*, kron, expm, eigen). \
Примеры: {\"action\":\"run\",\"algorithm\":\"ghz\",\"n\":5,\"shots\":1024}; \
{\"action\":\"teleport\",\"theta\":0.7,\"exact\":true}; \
{\"action\":\"bloch\",\"alpha\":\"1/sqrt(2)\",\"beta\":\"i/sqrt(2)\"}; \
{\"action\":\"verify\",\"check\":\"unitary\",\"algorithm\":\"qft\",\"n\":3}; \
{\"action\":\"verify\",\"check\":\"equivalence\",\"algorithms\":[\"qft\",\"iqft\"],\"n\":3,\"up_to_global_phase\":false}.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["run", "qcasm", "qaoa", "teleport", "bloch", "verify", "list"],
                               "description": "Действие моста"},
                    "algorithm": {"type": "string", "description": "bell|ghz|qft|iqft|grover|bv|dj|period|teleport"},
                    "n": {"type": "integer", "default": 4, "minimum": 1, "maximum": 26, "description": "Число кубитов"},
                    "shots": {"type": "integer", "default": 1024, "minimum": 0, "description": "Выстрелов Борна"},
                    "seed": {"type": "integer", "default": 42, "description": "Семя xoshiro256++"},
                    "marks": {"type": "array", "items": {"type": "integer"}, "description": "Пометки Гровера/ДЙ"},
                    "secret": {"type": "integer", "default": 11, "description": "Секрет Бернштейна–Вазирани"},
                    "period": {"type": "integer", "description": "Период гребёнки (period)"},
                    "offset": {"type": "integer", "default": 0, "description": "Смещение гребёнки"},
                    "theta": {"type": "number", "default": 0.7, "description": "Препарат телепортации Ry(theta)|0>"},
                    "exact": {"type": "boolean", "default": false, "description": "Точный режим телепортации"},
                    "alpha": {"type": "string", "description": "Амплитуда |0> (выражение calc)"},
                    "beta": {"type": "string", "description": "Амплитуда |1> (выражение calc)"},
                    "source": {"type": "string", "description": "Текст QCASM-схемы (для qcasm)"},
                    "path": {"type": "string", "description": "Файл QCASM-схемы (альтернатива source)"},
                    "noise": {"type": "string", "enum": ["ideal", "ibm-heron", "google-willow", "noisy-90s"], "description": "Пресет шума железа поверх идеала"},
                    "edges": {"description": "Рёбра MaxCut: [[0,1],[1,2]] или \"0-1,1-2\" (для qaoa)"},
                    "p": {"type": "integer", "default": 2, "minimum": 1, "maximum": 8, "description": "Глубина QAOA-ансатца"},
                    "restarts": {"type": "integer", "default": 3, "minimum": 0, "maximum": 16, "description": "Перезапусков оптимизатора QAOA"},
                    "sweeps": {"type": "integer", "default": 12, "minimum": 1, "maximum": 64, "description": "Проходов координатного спуска QAOA"},
                    "noise_shots": {"type": "integer", "default": 4096, "minimum": 0, "description": "Выстрелов для шума в verify"},
                    "check": {"type": "string", "enum": ["unitary", "equivalence", "teleport"], "description": "Проверка верификатора"},
                    "algorithms": {"type": "array", "items": {"type": "string"}, "maxItems": 2, "description": "Пара схем для equivalence"},
                    "up_to_global_phase": {"type": "boolean", "default": false, "description": "Эквивалентность до глобальной фазы"}
                },
                "required": ["action"]
            }
        }),
        json!({
            "name": "poler_scivoice",
            "description": "ГОЛОС УЧЁНОГО (v0.74.0): тот же научный речевой контур, что слышит \
пользователь из calc speak — токенизатор → роторы 104k строк No-Mul x86_64 ASM \
(языки мира + 118 химических элементов) → триединое ядро → трит-вердикт {-1,0,+1}. \
mode=\"read\" — научное чтение текста/формулы; mode=\"hf\" — прямой RHF/STO-3G \
расчёт (второй период B–Ne: H2, He, LiH, H2O, NH3, CH4, HF, CO; эталоны PySCF \
до 1e-6 Ha); mode=\"molecule\" — паспорт молекулы из SMILES/имени (кольца, \
дескрипторы Липински, заряды Гастайгера); mode=\"bio\" — био-контур связывания \
лиганд⇄мишень (ΔG, Kd в лог-домене с тритной глубиной, 27-дерево, DE-9IM). \
Примеры: {\"text\":\"C6H12O6\",\"mode\":\"read\"}; {\"text\":\"H2\",\"mode\":\"hf\"}; \
{\"text\":\"кофеин\",\"mode\":\"molecule\"}; {\"text\":\"дофамин\",\"target\":\"серотонин\",\"mode\":\"bio\"}.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "текст, формула, SMILES или имя молекулы (кофеин, аспирин, дофамин…)"},
                    "target": {"type": "string", "description": "мишень для режима bio (SMILES или имя)"},
                    "mode": {"type": "string", "enum": ["read", "hf", "molecule", "bio"], "default": "read", "description": "read — научное чтение; hf — RHF/STO-3G; molecule — паспорт SMILES; bio — связывание"}
                },
                "required": ["text"]
            }
        }),
        json!({
            "name": "poler_jit",
            "description": "ВНУТРЕННИЙ КОМПИЛЯТОР (v0.76.0, контур A): «голос» движка, \
математически просчитывающий соединение модулей. mode=\"bench\" — JIT-компиляция \
случайного matvec-графа в машинный код x86_64 (веса-иммедиаты WeightsInCode, \
триты No-Mul, W^X) с бит-в-бит проверкой против эталона и замером времени; \
mode=\"linker\" — CausalLinker: верификация стыков конвейера (ширина/тип/\
трит-глубина/норма/аллокации/W^X) с диагнозом на естественном русском и \
автопочинкой адаптерами. Примеры: {\"mode\":\"bench\",\"rows\":32}; \
{\"mode\":\"linker\",\"defect\":\"width\"}.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mode": {"type": "string", "enum": ["bench", "linker"], "default": "bench", "description": "bench — JIT-бенчмарк; linker — диагноз стыков"},
                    "rows": {"type": "integer", "default": 64, "minimum": 2, "maximum": 256, "description": "строк matvec"},
                    "cols": {"type": "integer", "default": 48, "minimum": 2, "maximum": 256, "description": "столбцов matvec"},
                    "defect": {"type": "string", "enum": ["none", "width", "kind", "depth", "norm", "alloc", "wx"], "default": "none", "description": "подсаженный дефект для демонстрации речи линковщика"}
                },
                "required": ["mode"]
            }
        }),
        json!({
            "name": "poler_chunk",
            "description": "RAG-нарезка документа на чанки с якорями: passage-уровень \
вместо чтения файла целиком. Возвращает фрагменты с byte range, номерами строк, \
breadcrumb заголовков и числом токенов. Формат определяется автоматически: \
markdown (секции заголовков, code-fence не режется), код (границы строк, \
никогда внутри строки), текст (абзацы → предложения → слова). Замена внешнего \
RAG-конвейера chunking→retrieve: нарежь документ, прочитай релевантные куски \
(найди их через poler_search/poler_grep), цитируй по byte range.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Файл документа (md/txt/код)"},
                    "target_tokens": {"type": "integer", "default": 384, "minimum": 16, "maximum": 8192, "description": "Целевой размер чанка в токенах POLER"},
                    "overlap_tokens": {"type": "integer", "default": 48, "minimum": 0, "maximum": 2048, "description": "Перекрытие соседних чанков"},
                    "json": {"type": "boolean", "default": false, "description": "Машинно-читаемый отчёт вместо текста"}
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "query_poler_knowledge",
            "description": "Поиск по библиотеке POLER «Суверенный Гиппокамп»: 300 PTS-спек, \
6-томный математический трактат, ядро шифра PND, Шнайер (укр), 194 транскрипта. \
Гибрид BM25/WebRank + векторы (RaBitQ), кросс-языковый Semantic Bridge (рус↔англ). \
Эпистемическая градация: каждый хит несёт статус достоверности — mvr_verified \
(машинно доказано, ×1.5) / source_document (первоисточник, ×1.0) / narrative \
(нарратив, ×0.7), файл, номера строк, байтовый диапазон и прямую цитату. \
min_provenance отсекает недостоверное ('mvr' — только машинно доказанное). \
Цитаты точны: text == файл[byte_start..byte_end] — проверяемо.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Поисковая фраза (рус/укр/англ)"},
                    "top": {"type": "integer", "default": 8, "minimum": 1, "maximum": 30},
                    "min_provenance": {
                        "type": "string",
                        "enum": ["mvr", "source", "narrative"],
                        "description": "Минимальный эпистемический статус хитов"
                    }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "poler_box_exec",
            "description": "Двухконтурный брокер (v0.26.0): исполняет команду В ИЗОЛИРОВАННОМ \
Docker-контейнере и возвращает stdout/stderr/exit-код. Схема: мозг агента \
(ты) → шлюз POLER (sandbox-судья: деструктив — Block ДО исполнения, \
подтверждения из MCP не принимаются в принципе) → контур исполнения \
(runner: net=none, только /workspace, без home; при его отсутствии — box). \
Одна команда без конвейеров и редиректов (кавычки — как в шлюзе; \
bash -c '…' допустим — payload судится рекурсивно). Путь к хост-системе \
НЕ существует физически: хост виден только как /workspace.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "Команда (например: cargo build или python3 script.py)"},
                    "timeout_secs": {"type": "integer", "default": 120, "minimum": 1, "maximum": 600},
                    "target": {
                        "type": "string",
                        "enum": ["auto", "runner", "box"],
                        "default": "auto",
                        "description": "auto: runner (жёстче) → box"
                    }
                },
                "required": ["command"]
            }
        }),
        json!({
            "name": "poler_box_status",
            "description": "Состояние Container Jail для текущего workspace: \
box-контейнер (контуры 2/3, проброшенные агенты) и runner (контур \
исполнения poler_box_exec: net=none, только /workspace). Вызывай перед \
poler_box_exec, чтобы понять, поднят ли контур исполнения.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
    ];

    // E1/v0.31.0 + E2/v0.32.0: идеальный исполнитель команд (ядро Zig,
    // raw-syscalls). Требует фичу pnd-ffi (libpoler_core.a).
    #[cfg(feature = "pnd-ffi")]
    tools.push(json!({
        "name": "poler_exec",
        "description": "ИДЕАЛЬНЫЙ ИСПОЛНИТЕЛЬ КОМАНД (ядро Zig, raw-syscall слой): запустить \
программу с жёстким таймаутом (SIGTERM → grace → SIGKILL группе), лимитом захвата вывода \
(O(1) памяти) и гарантией отсутствия зомби (pidfd + wait4 в ppoll-цикле). Рождён диагностикой \
GNU bash 5.2: классы bash-ошибок исключены конструктивно. argv передаётся массивом — шелл-инъекции \
невозможны. E2: capture=head_tail (голова B/2 + маркер dropped N + хвост B/2 — умолчание), \
cwd (рабочий каталог), env (явное окружение), pty (настоящий терминал 200x50 для sudo/fzf/htop), \
PATH разрешает сам ребёнок (ноль stat в родителе). Возвращает JSON: exit_code, signal, timed_out, \
cancelled, truncated, pty, stdout, stderr, duration_us, pid.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Программа: имя из PATH или путь с '/'"},
                "args": {"type": "array", "items": {"type": "string"}, "description": "Аргументы (массив, БЕЗ шелл-парсинга)"},
                "timeout_ms": {"type": "integer", "default": 30000, "minimum": 1},
                "grace_ms": {"type": "integer", "default": 100, "minimum": 0},
                "max_out_bytes": {"type": "integer", "default": 65536, "description": "Лимит захвата НА ПОТОК"},
                "stdin": {"type": "string", "description": "Данные в stdin (опционально)"},
                "cwd": {"type": "string", "description": "Рабочий каталог ребёнка (chdir в бутстрапе)"},
                "env": {"type": "object", "additionalProperties": {"type": "string"}, "description": "Явное окружение (иначе наследуется)"},
                "pty": {"type": "boolean", "default": false, "description": "Псевдотерминал 200x50; stdout/stderr слиты"},
                "capture": {"type": "string", "enum": ["tail", "head_tail"], "default": "head_tail", "description": "Режим при переполнении: хвост | голова+маркер+хвост"}
            },
            "required": ["command"]
        }
    }));

    #[cfg(feature = "pnd-ffi")]
    tools.push(json!({
        "name": "poler_exec_async",
        "description": "ФОНОВЫЙ запуск команды (E2): возвращает task_id мгновенно, команда живёт \
в собственном потоке. Опрос результата — poler_exec_task, отмена — poler_exec_kill, обзор — \
poler_exec_list. Аргументы совпадают с poler_exec (command/args/timeout_ms/…/cwd/env/pty/capture). \
Для долгих сборок/сканов: запустил — и продолжил работу.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "command": {"type": "string"},
                "args": {"type": "array", "items": {"type": "string"}},
                "timeout_ms": {"type": "integer", "default": 30000},
                "grace_ms": {"type": "integer", "default": 100},
                "max_out_bytes": {"type": "integer", "default": 65536},
                "stdin": {"type": "string"},
                "cwd": {"type": "string"},
                "env": {"type": "object", "additionalProperties": {"type": "string"}},
                "pty": {"type": "boolean", "default": false},
                "capture": {"type": "string", "enum": ["tail", "head_tail"], "default": "head_tail"}
            },
            "required": ["command"]
        }
    }));

    #[cfg(feature = "pnd-ffi")]
    tools.push(json!({
        "name": "poler_exec_task",
        "description": "ОПРОС/ОЖИДАНИЕ фоновой задачи poler_exec_async: task_id + необязательный \
wait_ms (сколько миллисекунд ждать завершения, до 60000). Отчёт: task_id, status (running|done), \
command, elapsed_us и — при done — полный JSON исполнения (exit_code, stdout, stderr, cancelled, …).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "task_id": {"type": "integer", "description": "id из poler_exec_async"},
                "wait_ms": {"type": "integer", "default": 0, "maximum": 60000}
            },
            "required": ["task_id"]
        }
    }));

    #[cfg(feature = "pnd-ffi")]
    tools.push(json!({
        "name": "poler_exec_kill",
        "description": "ОТМЕНА фоновой задачи: атомарный флаг → SIGTERM → grace → SIGKILL группе \
(ядро замечает отмену за ≤25 мс, зомби невозможны). Итог — через poler_exec_task \
(cancelled: true).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "task_id": {"type": "integer"}
            },
            "required": ["task_id"]
        }
    }));

    #[cfg(feature = "pnd-ffi")]
    tools.push(json!({
        "name": "poler_exec_list",
        "description": "ОБЗОР фоновых задач реестра: живые (running, elapsed_us) и завершённые \
(done, exit_code/cancelled/timed_out) — до 128 последних.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "required": []
        }
    }));

    // C2/v0.33.0 «Живая муха»: коннектом FLYCSR1 (мозг мухи FlyWire v783,
    // 138 639 нейронов) как резидентная матрица A. Артефакт грузится в RAM
    // ОДИН раз (csr в первом вызове), дальше всё семейство работает без
    // диска; тяжёлые запросы исполняются в пуле воркеров параллельно.
    tools.push(json!({
        "name": "poler_fly",
        "description": "ЖИВАЯ МУХА — загрузка/сводка/выгрузка коннектома FLYCSR1 (мозг мухи \
FlyWire v783: 138 639 нейронов, матрица A со знаками: ach/glut +1 возбуждающие, gaba −1 \
тормозные, oct/ser/da модуляторные). Первый вызов: csr = путь к .csr.zst (+ опционально \
nodes = nodes.bin для перевода индекс ↔ root_id FlyWire) — артефакт грузится в RAM и живёт \
между вызовами (тёплый: повторная сводка без диска). action: summary (умолчание) — сводка \
(узлы/рёбра/масса/баланс медиаторов), eject — выгрузить из RAM. Дальше вызывай \
poler_fly_node/edge/khop/path/common/centrality/rotor/motifs/propagate БЕЗ csr.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "csr": {"type": "string", "description": "Путь к .csr.zst (первый вызов; далее опционален)"},
                "nodes": {"type": "string", "description": "Путь к nodes.bin (root_id FlyWire, опционально)"},
                "action": {"type": "string", "enum": ["summary", "eject"], "default": "summary"}
            },
            "required": []
        }
    }));

    tools.push(json!({
        "name": "poler_fly_node",
        "description": "ПАСПОРТ НЕЙРОНА мухи: исходящие/входящие степени и синаптические массы, \
топ партнёров обоих направлений (other, root_id, weight, nt, sign, signed_weight). neuron = \
индекс CSR или root_id (строкой, если загружена nodes). limit усекает топы (умолчание 8).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "csr": {"type": "string"},
                "nodes": {"type": "string"},
                "neuron": {"type": ["string", "integer"], "description": "Индекс 0..138638 либо root_id FlyWire"},
                "limit": {"type": "integer", "default": 8, "maximum": 500}
            },
            "required": ["neuron"]
        }
    }));

    tools.push(json!({
        "name": "poler_fly_edge",
        "description": "РЕБРО u→v и РОТОР пары J = A − Aᵀ: циркуляция влияния. Однонаправленное \
ребро — чистый поток (J[u][v] = +w, J[v][u] = −w), реципрокная симметричная пара гасится в 0. \
forward/backward — знаковые веса обоих направлений.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "csr": {"type": "string"},
                "nodes": {"type": "string"},
                "u": {"type": ["string", "integer"]},
                "v": {"type": ["string", "integer"]}
            },
            "required": ["u", "v"]
        }
    }));

    tools.push(json!({
        "name": "poler_fly_khop",
        "description": "K-HOP BFS потока сигнала от нейрона: размеры фронтов по хопам + всего \
достигнуто (signal expansion). sign: all | exc (только возбуждающие) | inh (только тормозные).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "csr": {"type": "string"},
                "nodes": {"type": "string"},
                "neuron": {"type": ["string", "integer"]},
                "depth": {"type": "integer", "default": 2, "minimum": 1, "maximum": 10},
                "sign": {"type": "string", "enum": ["all", "exc", "inh"], "default": "all"}
            },
            "required": ["neuron"]
        }
    }));

    tools.push(json!({
        "name": "poler_fly_path",
        "description": "КРАТЧАЙШИЙ ПУТЬ СИГНАЛА from→to (BFS): цепочка прыжков — каждый синапс \
с весом, медиатором и знаком; total_weight — суммарная масса маршрута. Найди маршрут между \
любыми двумя нейронами мозга за миллисекунды.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "csr": {"type": "string"},
                "nodes": {"type": "string"},
                "from": {"type": ["string", "integer"]},
                "to": {"type": ["string", "integer"]},
                "sign": {"type": "string", "enum": ["all", "exc", "inh"], "default": "all"}
            },
            "required": ["from", "to"]
        }
    }));

    tools.push(json!({
        "name": "poler_fly_common",
        "description": "ОБЩИЕ ПАРТНЁРЫ набора нейронов (2..32): direction=down — общие мишени \
(на кого влияет весь набор), up — общие источники (кто влияет на весь набор). Конвергенция и \
дивергенция схем мозга; у каждого партнёра — суммарная масса и рёбра от каждого нейрона набора.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "csr": {"type": "string"},
                "nodes": {"type": "string"},
                "neurons": {"type": "array", "items": {"type": ["string", "integer"]}, "minItems": 1, "maxItems": 32},
                "direction": {"type": "string", "enum": ["down", "up"], "default": "down"},
                "sign": {"type": "string", "enum": ["all", "exc", "inh"], "default": "all"},
                "limit": {"type": "integer", "default": 20, "maximum": 500}
            },
            "required": ["neurons"]
        }
    }));

    tools.push(json!({
        "name": "poler_fly_centrality",
        "description": "ХАБЫ МОЗГА: топ нейронов по исходящей/входящей степени + взвешенный \
PageRank по |A| (модуляторы не проводят ранг; sign сужает граф до возбуждающих/тормозных путей). \
iterations/converged — диагностика сходимости.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "csr": {"type": "string"},
                "nodes": {"type": "string"},
                "top": {"type": "integer", "default": 10, "maximum": 100},
                "sign": {"type": "string", "enum": ["all", "exc", "inh"], "default": "all"},
                "damping": {"type": "number", "default": 0.85, "minimum": 0.05, "maximum": 0.99},
                "iterations": {"type": "integer", "default": 30, "maximum": 200}
            },
            "required": []
        }
    }));

    tools.push(json!({
        "name": "poler_fly_rotor",
        "description": "ГЛОБАЛЬНЫЙ ТОП ЦИРКУЛЯЦИИ J = A − Aᵀ: самые однонаправленные влияния \
мозга — чистые потоки без встречной компенсации (J_uv > 0, J_vu = −J_uv). min_abs отсекает \
слабые пары; forward/backback_signed — компоненты потока.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "csr": {"type": "string"},
                "nodes": {"type": "string"},
                "top": {"type": "integer", "default": 10, "maximum": 1000},
                "min_abs": {"type": "integer", "default": 1, "minimum": 1}
            },
            "required": []
        }
    }));

    tools.push(json!({
        "name": "poler_fly_motifs",
        "description": "МОТИВЫ ВОКРУГ НЕЙРОНА: реципрокные пары u⇄v (оба ребра с весами), \
feedforward-треугольники u→v→w + u→w (схемы усиления), feedback-циклы u→v→w→u (кольца \
обратной связи). examples ограничивает список примеров (счётчики всегда полные).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "csr": {"type": "string"},
                "nodes": {"type": "string"},
                "neuron": {"type": ["string", "integer"]},
                "examples": {"type": "integer", "default": 5, "maximum": 50}
            },
            "required": ["neuron"]
        }
    }));

    tools.push(json!({
        "name": "poler_fly_propagate",
        "description": "СИМУЛЯЦИЯ РАСПРОСТРАНЕНИЯ СИГНАЛА — муха думает в RAM: \
x(t+1) = leak·x(t) + γ·A·x(t) на знаковых весах (возбуждение разгоняет, торможение гасит, \
модуляторы молчат). neurons — семена (до 64), steps — шаги (до 64), gamma/leak — динамика, \
theta — порог активности. Отчёт: хронология (active, ±масса) + топ возбуждённых нейронов.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "csr": {"type": "string"},
                "nodes": {"type": "string"},
                "neurons": {"type": "array", "items": {"type": ["string", "integer"]}, "minItems": 1, "maxItems": 64},
                "steps": {"type": "integer", "default": 4, "maximum": 64},
                "gamma": {"type": "number", "default": 0.05},
                "leak": {"type": "number", "default": 0.8},
                "theta": {"type": "number", "default": 0.01},
                "top": {"type": "integer", "default": 15, "maximum": 200},
                "sign": {"type": "string", "enum": ["all", "exc", "inh"], "default": "all"}
            },
            "required": ["neurons"]
        }
    }));

    tools.push(json!({
        "name": "poler_literary_field",
        "description": "ЛИТЕРАТУРНЫЙ ДВИГАТЕЛЬ POLER[Ψ] — анализ поля интенции (фазы ℘→O): \
текст-замысел → инвариантный вектор Ω(o) (детерминированный FNV-хэш термов, ноль RNG) + \
косинусная топология 12 архетипов (Кэмпбелл + канон POLER). Опционально — мушиная \
калибровка (csr+seeds+khop): каста нейронов FLYCSR1 и её ротор J = A − Aᵀ. \
Отчёт: термы по массе, архетипы по косинусу, F/причинность на старте, статистика касты.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "text": {"type": "string", "description": "Замысел: текст любого языка"},
                "dims": {"type": "integer", "default": 64, "minimum": 16, "maximum": 256},
                "csr": {"type": "string", "description": "FLYCSR1 .csr.zst для мушиной калибровки"},
                "nodes": {"type": "string"},
                "seeds": {"type": "array", "items": {"type": ["string", "integer"]}, "maxItems": 16},
                "khop": {"type": "integer", "default": 2, "minimum": 1, "maximum": 4},
                "max_cast": {"type": "integer", "default": 48, "minimum": 2, "maximum": 64},
                "constraints": {"type": "array", "items": {"type": "object", "properties": {
                    "i": {"type": "integer"}, "j": {"type": "integer"}, "c": {"type": "number"}
                }}}
            },
            "required": ["text"]
        }
    }));

    tools.push(json!({
        "name": "poler_literary_step",
        "description": "ПОЛИГОН ФАЗЫ: один шаг канонического уравнения POLER[Ψ] \
p_{t+1} = p_t − η·Π_Λ(∇F + D·p + γJ·p) + η_r·Π_Λ(κ·(echo − p)). text — создать сессию \
(мушиная калибровка: csr/seeds/khop), session — шагнуть существующую (observation — \
эволюция замысла). Телеметрия шага: F, ε, Σ(t), H^Ψ, причинность, режим (attraction / \
refraction / observer_kill / superconductivity). Сессии резидентны в RAM (LRU 8).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "text": {"type": "string", "description": "Замысел — создаёт новую сессию"},
                "session": {"type": "string", "description": "id существующей сессии"},
                "observation": {"type": "string", "description": "Новый замысел — эволюция Ω(o)"},
                "dims": {"type": "integer", "default": 64, "minimum": 16, "maximum": 256},
                "steps": {"type": "integer", "default": 48},
                "eta": {"type": "number", "default": 0.1},
                "eta_r": {"type": "number", "default": 0.05},
                "rho": {"type": "number", "default": 0.9},
                "kappa": {"type": "number", "default": 1.2},
                "gamma": {"type": "number", "default": 1.0},
                "lambda": {"type": "number", "default": 0.01},
                "no_mul": {"type": "boolean", "default": false, "description": "Trit5-квантование p_t (No-Mul, SIMD)"},
                "csr": {"type": "string"},
                "nodes": {"type": "string"},
                "seeds": {"type": "array", "items": {"type": ["string", "integer"]}, "maxItems": 16},
                "khop": {"type": "integer", "default": 2, "minimum": 1, "maximum": 4},
                "max_cast": {"type": "integer", "default": 48, "minimum": 2, "maximum": 64},
                "constraints": {"type": "array", "items": {"type": "object", "properties": {
                    "i": {"type": "integer"}, "j": {"type": "integer"}, "c": {"type": "number"}
                }}}
            }
        }
    }));

    tools.push(json!({
        "name": "poler_literary_generate",
        "description": "ГЕНЕРАЦИЯ НАРРАТИВА: полный прогон POLER[Ψ] до сверхпроводимости \
смысла (H^Ψ = 0) либо до лимита шагов. text — one-shot, session — на резидентной сессии. \
Отчёт: три акта (доминанты-архетипы, кривая F), драматургические пары (роторные пары \
мухи: кто доминирует над кем), сжатая телеметрия и детерминированный текст-разметка. \
Без мухи — чистая физика текста (сходимость); с мухой (csr) — предельный цикл: живой \
мозг не даёт нарративу замереть.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "text": {"type": "string"},
                "session": {"type": "string"},
                "dims": {"type": "integer", "default": 64, "minimum": 16, "maximum": 256},
                "steps": {"type": "integer", "default": 48, "maximum": 512},
                "eta": {"type": "number", "default": 0.1},
                "gamma": {"type": "number", "default": 1.0},
                "no_mul": {"type": "boolean", "default": false},
                "csr": {"type": "string"},
                "nodes": {"type": "string"},
                "seeds": {"type": "array", "items": {"type": ["string", "integer"]}, "maxItems": 16},
                "khop": {"type": "integer", "default": 2, "minimum": 1, "maximum": 4},
                "max_cast": {"type": "integer", "default": 48, "minimum": 2, "maximum": 64},
                "constraints": {"type": "array", "items": {"type": "object", "properties": {
                    "i": {"type": "integer"}, "j": {"type": "integer"}, "c": {"type": "number"}
                }}}
            }
        }
    }));

    tools.push(json!({
        "name": "poler_literary_eject",
        "description": "Выгрузить резидентные сессии POLER[Ψ] из RAM: конкретную (session=id) \
или все (по умолчанию). Латентные состояния, эхо и мушиные касты освобождаются; следующий \
poler_literary_step с text создаст сессию заново.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "session": {"type": "string", "description": "id сессии либо all (умолчание)"}
            }
        }
    }));

    tools.push(json!({
        "name": "poler_ssn_step",
        "description": "ЖИВОЙ МОЗГ SSN (Синаптический Вихрь, доказанный стек): создать мозг \
(без session — параметры seed/n/fields/dims) или шагнуть существующий на steps шагов. \
Полный стек: виртуальная топология (tgt = (i·39293+f·29101+seed·73471) mod N — ноль RAM \
на граф), фазовые синапсы cos(), латеральное торможение, STDP+DA, гомеостаз-интегратор \
(f_sys), ретикулярный тон (анти-windup), E/I-контроллер, нейромодуляторы DA/5HT/NE, \
ритмы θ/γ. Инварианты (доказаны 67 проверок + 10 seed × 10k шагов): активность ~5%, \
S < 0.8, веса в клипах. Телеметрия + readout топ-K активных нейронов.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "session": {"type": "string", "description": "id живого мозга (продолжить)"},
                "seed": {"type": "integer", "default": 777},
                "n": {"type": "integer", "default": 600, "minimum": 8, "maximum": 1000000, "description": "нейронов"},
                "fields": {"type": "integer", "default": 16, "minimum": 1, "maximum": 64, "description": "виртуальных синаптических полей"},
                "dims": {"type": "integer", "default": 128, "description": "размерность CSE-сенсорики"},
                "steps": {"type": "integer", "default": 1000, "maximum": 100000},
                "readout": {"type": "integer", "default": 10, "description": "топ-K нейронов в readout"}
            }
        }
    }));

    tools.push(json!({
        "name": "poler_ssn_inject",
        "description": "СЕНСОРНАЯ ИНЪЕКЦИЯ в живой мозг SSN: текст → CSE-вектор \
(золотая фаза: c·φ mod 2π — разделимость 1.35) → активации нейронов. Затем \
steps шагов динамики. Мозг воспринимает текст как сенсорный сигнал: инварианты \
сохраняются (проверено inject_keeps_invariants).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "session": {"type": "string", "description": "id живого мозга"},
                "text": {"type": "string", "description": "сенсорный вход — что мозг воспринимает"},
                "steps": {"type": "integer", "default": 500, "maximum": 100000, "description": "шагов после инъекции (0 — только восприять)"}
            },
            "required": ["session", "text"]
        }
    }));

    tools.push(json!({
        "name": "poler_ssn_status",
        "description": "СНИМОК ЖИВОГО МОЗГА SSN без шага: телеметрия (активность, \
f_sys, ретикулярный тон, синхронность S, критичность C, E/I, GABA/глутамат, \
DA/5HT/NE, веса) + readout топ-K активных нейронов + число живых сессий.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "session": {"type": "string"},
                "readout": {"type": "integer", "default": 10}
            },
            "required": ["session"]
        }
    }));

    tools.push(json!({
        "name": "poler_ssn_eject",
        "description": "Выгрузить живые мозги SSN из RAM: конкретный (session=id) \
или все (all — умолчание). Состояния сетей, веса и медиаторы освобождаются.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "session": {"type": "string", "description": "id мозга либо all (умолчание)"}
            }
        }
    }));

    tools.push(json!({
        "name": "poler_triune_speak",
        "description": "ТРИЕДИНАЯ АРХИТЕКТУРА (S2/v0.36.0): муха + синусоидный вихрь + \
троичный кристалл → ЖИВАЯ РЕЧЬ. Без session — рождает новое Триединство (seed/gamma); \
с session — продолжает разговор (мозг, пульс мухи и контекст живут между вызовами). \
Промпт воспринимается CSE-сенсорикой (золотая фаза), вихрь дышит между токенами \
(медиаторы DA/5HT/NE задают температуру речи), ротор мухи разводит лексику по 12 \
архетипам (γ=0 — речь зацикливается), кристалл (.t5c, Trit5: 5 тритов/байт, \
No-Mul SIMD) даёт семантику и синтаксис, NMDA-гейт пропускает совпадающие смыслы. \
Возврат: speech + трейс провенанса (sem/gate/syn/fly/tau) + моторные интенты \
(ТОЛЬКО предложения poler_exec — исполнение остаётся за агентом).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "text": {"type": "string", "description": "сенсорный вход — о чём говорить"},
                "session": {"type": "string", "description": "id Триединства (продолжить разговор)"},
                "seed": {"type": "integer", "default": 777},
                "gamma": {"type": "number", "default": 0.8, "minimum": 0, "maximum": 4, "description": "усиление ротора мухи"},
                "tokens": {"type": "integer", "default": 24, "minimum": 1, "maximum": 256, "description": "длина речи"}
            },
            "required": ["text"]
        }
    }));

    tools.push(json!({
        "name": "poler_triune_state",
        "description": "СНИМОК говорящего Триединства без шага: последняя фраза, \
число сенсорных инъекций и произнесённых токенов, телеметрия живого мозга \
(активность ~5%, синхронность, критичность, медиаторы, веса).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "session": {"type": "string", "description": "id Триединства"}
            },
            "required": ["session"]
        }
    }));

    tools
}

#[cfg(test)]
mod audit_tests {
    use super::*;

    #[test]
    fn path_guard_allows_root_and_blocks_outside() {
        let dir = std::env::temp_dir().join("poler-audit-guard");
        let _ = std::fs::create_dir_all(&dir);
        let inside = dir.join("secret.txt");
        std::fs::write(&inside, b"x").unwrap();
        // внутри корня — можно
        assert!(path_allowed_under(
            inside.to_str().unwrap(),
            std::slice::from_ref(&dir)
        ));
        // /etc/passwd — нельзя
        assert!(!path_allowed_under("/etc/passwd", std::slice::from_ref(&dir)));
        // symlink-escape: ссылка внутри корня, цель снаружи — нельзя
        let link = dir.join("escape-link");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink("/etc/passwd", &link).unwrap();
        assert!(!path_allowed_under(link.to_str().unwrap(), &[dir]));
    }

    #[test]
    fn net_guard_blocks_metadata_and_link_local() {
        assert!(host_blocked_by_default("169.254.169.254"));
        assert!(host_blocked_by_default("metadata.google.internal"));
        assert!(host_blocked_by_default("metadata.goog"));
        assert!(host_blocked_by_default("0.0.0.0"));
        assert!(host_blocked_by_default("fe80::1"));
        assert!(host_blocked_by_default("fd00::1"));
        // легитимные цели — не блокируем
        assert!(!host_blocked_by_default("litnet.com"));
        assert!(!host_blocked_by_default("127.0.0.1")); // WebLens/локальная разработка
        assert!(!host_blocked_by_default("192.168.1.1")); // домашняя сеть
        assert!(!host_blocked_by_default("::1"));
        assert!(!host_blocked_by_default("docs.rs"));
    }

    #[test]
    fn net_guard_url_extraction_with_and_without_port() {
        // регрессия: rsplit_once(':') без порта раньше давал пустой host
        assert!(mcp_url_allowed("http://169.254.169.254/latest/meta-data/").is_err());
        assert!(mcp_url_allowed("http://169.254.169.254:8080/x?y=1").is_err());
        assert!(mcp_url_allowed("https://metadata.google.internal/computeMetadata/").is_err());
        assert!(mcp_url_allowed("https://litnet.com/").is_ok());
        assert!(mcp_url_allowed("http://127.0.0.1:8765/").is_ok());
    }
}

#[cfg(test)]
mod box_broker_tests {
    use super::*;

    /// Сериализация env-мутаций (POLER_BOX_DOCKER / POLER_WORKSPACE) —
    /// общий лок с containers/dispatch-тестами.
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::gateway::containers::docker_env_test_lock()
    }

    fn server() -> McpServer {
        McpServer::new(9222, 10, PathBuf::from("/nonexistent-poler-test.db"))
    }

    /// Фейковый docker-клиент: пишет argv в лог-файл (env POLER_FAKE_LOG),
    /// отвечает на version/inspect/exec. Эмулирует РАБОТАЮЩИЙ daemon,
    /// у которого подняты и box, и runner (inspect → true).
    fn fake_docker(tag: &str) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("poler-mcp-fake-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let log = base.join("docker-calls.log");
        let script = base.join("fake-docker");
        std::fs::write(
            &script,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$POLER_FAKE_LOG\"\ncase \"$1\" in\n  version) echo '31.0.0-fake'; exit 0;;\n  inspect) echo 'true'; exit 0;;\n  exec) echo 'fake-exec-output'; exit 0;;\nesac\nexit 0\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        (script, log)
    }

    #[test]
    fn manifest_contains_box_broker_tools() {
        let m = tools_manifest();
        let names: Vec<&str> = m
            .iter()
            .filter_map(|t| t.get("name").and_then(|v| v.as_str()))
            .collect();
        assert!(names.contains(&"poler_box_exec"), "нет poler_box_exec: {names:?}");
        assert!(names.contains(&"poler_box_status"), "нет poler_box_status: {names:?}");
        // входы описаны
        let exec = m.iter().find(|t| t.get("name").and_then(|v| v.as_str()) == Some("poler_box_exec")).unwrap();
        let req = exec.pointer("/inputSchema/required").unwrap();
        assert!(req.to_string().contains("command"), "required: {req}");
    }

    #[test]
    fn tools_call_routing_unknown_and_known() {
        let srv = server();
        let r = srv.dispatch(&serde_json::json!({
            "jsonrpc": "2.0", "id": 7,
            "method": "tools/call",
            "params": {"name": "poler_box_status", "arguments": {}}
        }));
        let r = r.expect("ответ есть");
        let text = serde_json::to_string(&r).unwrap();
        assert!(text.contains("poler_box_exec"), "статус упоминает брокера: {text}");
    }

    #[test]
    fn box_exec_without_docker_honest_error() {
        let _g = env_lock();
        std::env::set_var("POLER_BOX_DOCKER", "/bin/false");
        let r = server().tool_box_exec(&json!({"command": "ls"}));
        std::env::remove_var("POLER_BOX_DOCKER");
        assert!(r.is_err(), "без docker — отказ: {r:?}");
        let e = r.unwrap_err();
        assert!(e.contains("docker"), "ошибка упоминает docker: {e}");
        assert!(e.contains("box"), "подсказка box runner on / box on: {e}");
    }

    #[test]
    fn box_exec_args_validation() {
        let srv = server();
        // нет command
        let e = srv.tool_box_exec(&json!({})).unwrap_err();
        assert!(e.contains("command"), "{e}");
        // пустой
        assert!(srv.tool_box_exec(&json!({"command": "   "})).is_err());
        // мусорный target
        let e = srv.tool_box_exec(&json!({"command": "ls", "target": "host"})).unwrap_err();
        assert!(e.contains("target"), "{e}");
        // конвейер не поддерживается
        let e = srv.tool_box_exec(&json!({"command": "ls | wc -l"})).unwrap_err();
        assert!(e.contains("конвейеры"), "{e}");
        // редирект не поддерживается
        let e = srv.tool_box_exec(&json!({"command": "ls > out.txt"})).unwrap_err();
        assert!(e.contains("редирект"), "{e}");
    }

    #[test]
    fn box_exec_destructive_blocked_before_any_docker_exec() {
        let _g = env_lock();
        let (docker, log) = fake_docker("destructive");
        let ws = log.parent().unwrap().join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        std::env::set_var("POLER_BOX_DOCKER", &docker);
        std::env::set_var("POLER_FAKE_LOG", &log);
        std::env::set_var("POLER_WORKSPACE", &ws);

        // деструктив: Block ДО исполнения — docker exec не вызывается вовсе
        for destructive in ["rm -rf /", "dd if=/dev/zero of=/dev/sda", ":(){ :|:& };:"] {
            let r = server().tool_box_exec(&json!({"command": destructive}));
            assert!(r.is_err(), "{destructive} — обязан быть Block: {r:?}");
            let e = r.unwrap_err();
            assert!(e.contains("блокировка"), "{destructive}: {e}");
        }
        let log_content = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(!log_content.contains("exec"), "docker exec не должен был вызываться: {log_content}");

        std::env::remove_var("POLER_BOX_DOCKER");
        std::env::remove_var("POLER_FAKE_LOG");
        std::env::remove_var("POLER_WORKSPACE");
        let _ = std::fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn box_exec_confirm_denied_zero_silent_escalation() {
        let _g = env_lock();
        let (docker, log) = fake_docker("confirm");
        let ws = log.parent().unwrap().join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        std::env::set_var("POLER_BOX_DOCKER", &docker);
        std::env::set_var("POLER_FAKE_LOG", &log);
        std::env::set_var("POLER_WORKSPACE", &ws);

        // sudo: Confirm — MCP-агент подтвердить не может → отказ (не исполнение!)
        let r = server().tool_box_exec(&json!({"command": "sudo apt update"}));
        assert!(r.is_err(), "sudo из MCP — отказ: {r:?}");
        let e = r.unwrap_err();
        assert!(e.contains("владельцу"), "подсказка о владельце: {e}");
        let log_content = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(!log_content.contains("exec"), "sudo не исполнялся: {log_content}");

        std::env::remove_var("POLER_BOX_DOCKER");
        std::env::remove_var("POLER_FAKE_LOG");
        std::env::remove_var("POLER_WORKSPACE");
        let _ = std::fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn box_exec_allowed_runs_in_runner_and_returns_output() {
        let _g = env_lock();
        let (docker, log) = fake_docker("allow");
        let ws = log.parent().unwrap().join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        std::env::set_var("POLER_BOX_DOCKER", &docker);
        std::env::set_var("POLER_FAKE_LOG", &log);
        std::env::set_var("POLER_WORKSPACE", &ws);

        // target=auto: runner поднят (inspect → true) → исполнение в runner
        let r = server().tool_box_exec(&json!({"command": "python3 build.py --flag"}));
        assert!(r.is_ok(), "разрешённая команда: {r:?}");
        let out = r.unwrap();
        assert!(out.contains("runner"), "цель — runner: {out}");
        assert!(out.contains("poler-runner-"), "имя runner: {out}");
        assert!(out.contains("fake-exec-output"), "stdout вернулся: {out}");
        assert!(out.contains("exit 0"), "exit-код: {out}");

        // argv дошёл насквозь (кавычки раскрыты лексером шлюза)
        let log_content = std::fs::read_to_string(&log).unwrap();
        let exec_line = log_content
            .lines()
            .find(|l| l.starts_with("exec"))
            .expect("docker exec вызван");
        assert!(exec_line.contains("-i"), "пайповый режим: {exec_line}");
        assert!(exec_line.contains("POLER_RUNNER=1"), "брокер-контекст: {exec_line}");
        assert!(exec_line.contains("python3"), "argv насквозь: {exec_line}");
        assert!(exec_line.contains("build.py"), "argv насквозь: {exec_line}");
        assert!(exec_line.contains("--flag"), "argv насквозь: {exec_line}");
        assert!(!exec_line.contains("-it"), "без TTY: {exec_line}");

        // явный target=box — тоже работает (inspect=true у фейка)
        let r = server().tool_box_exec(&json!({"command": "ls", "target": "box"}));
        assert!(r.is_ok());
        assert!(r.unwrap().contains("box"));

        std::env::remove_var("POLER_BOX_DOCKER");
        std::env::remove_var("POLER_FAKE_LOG");
        std::env::remove_var("POLER_WORKSPACE");
        let _ = std::fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn box_status_reports_containers_with_fake_docker() {
        let _g = env_lock();
        let (docker, log) = fake_docker("status");
        let ws = log.parent().unwrap().join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        std::env::set_var("POLER_BOX_DOCKER", &docker);
        std::env::set_var("POLER_WORKSPACE", &ws);

        let out = server().tool_box_status(&json!({})).unwrap();
        assert!(out.contains("docker"), "{out}");
        assert!(out.contains("box poler-box-"), "box-строка: {out}");
        assert!(out.contains("работает"), "inspect=true → работает: {out}");
        assert!(out.contains("runner poler-runner-"), "runner-строка: {out}");
        assert!(out.contains("poler_box_exec"), "подсказка брокера: {out}");

        std::env::remove_var("POLER_BOX_DOCKER");
        std::env::remove_var("POLER_WORKSPACE");
        let _ = std::fs::remove_dir_all(log.parent().unwrap());
    }
}

#[cfg(test)]
mod knowledge_tool_tests {
    use super::*;

    /// Сквозной тест Суверенного Гиппокампа: мини-библиотека → инжест →
    /// tools/list → tools/call query_poler_knowledge (провенанс, цитата,
    /// фильтр min_provenance, инструкция при отсутствии БД).
    #[test]
    fn knowledge_tool_end_to_end() {
        let dir = tempfile::tempdir().unwrap();
        let d1 = dir.path().join("01_SPECS");
        std::fs::create_dir_all(&d1).unwrap();
        std::fs::write(
            d1.join("PTS-042.md"),
            "# PTS-042: Спека\n\n## 1. ОБЗОР\nнарративный обзор механики.\n\n\
             ## 4. ПОЛНЫЙ ТЕХНИЧЕСКИЙ ТЕКСТ\noriginal source text про диффузионный слой pndMix.\n",
        )
        .unwrap();
        let d3 = dir.path().join("03_TREATISE");
        std::fs::create_dir_all(&d3).unwrap();
        std::fs::write(
            d3.join("VOLUME_V_PND.md"),
            "# Том V\n\n## Теорема\nдиффузионный слой pndMix доказан золотым вектором.\n",
        )
        .unwrap();

        let kdb = dir.path().join("knowledge.db");
        let mut emb = crate::sources::knowledge::KnowledgeEmbedder::None;
        crate::sources::knowledge::ingest(
            dir.path(),
            &kdb,
            &mut emb,
            &crate::sources::knowledge::IngestOptions::default(),
        )
        .unwrap();

        let srv = McpServer::new(9222, 10, PathBuf::from("/nonexistent-web.db"))
            .with_knowledge_db(kdb.clone());

        // tools/list содержит новый инструмент
        let r = srv
            .dispatch(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
            .unwrap();
        assert!(serde_json::to_string(&r).unwrap().contains("query_poler_knowledge"));

        // tools/call: хиты с провенансом и цитатой
        let r = srv
            .dispatch(&json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": {"name": "query_poler_knowledge",
                           "arguments": {"query": "pndMix диффузионный"}}
            }))
            .unwrap();
        let text = serde_json::to_string(&r).unwrap();
        assert!(text.contains("MVR-VERIFIED"), "бейдж MVR: {text}");
        assert!(text.contains("TREATISE-V"), "док-ключ: {text}");
        assert!(text.contains("pndMix"), "цитата: {text}");

        // min_provenance=mvr: нарративные PTS-секции отсечены
        let r = srv
            .dispatch(&json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": {"name": "query_poler_knowledge",
                           "arguments": {"query": "обзор механики", "min_provenance": "mvr"}}
            }))
            .unwrap();
        let text = serde_json::to_string(&r).unwrap();
        assert!(
            text.contains("0 хитов") || text.contains("ничего не найдено"),
            "mvr-фильтр жёсткий: {text}"
        );

        // без БД — инструкция, не isError-сбой
        let srv2 = McpServer::new(9222, 10, PathBuf::from("/nonexistent-web.db"))
            .with_knowledge_db(dir.path().join("missing.db"));
        let r = srv2
            .dispatch(&json!({
                "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                "params": {"name": "query_poler_knowledge", "arguments": {"query": "x"}}
            }))
            .unwrap();
        let text = serde_json::to_string(&r).unwrap();
        assert!(text.contains("--knowledge-ingest"), "инструкция инжеста: {text}");
        assert!(!text.contains("\"isError\":true"), "состояние, не сбой: {text}");
    }
    // ── M6: резидентное состояние + стриминг логов ────────────────────

    /// Тёплый Гиппокамп: вторая dispatch переиспользует открытый хэндл
    /// (resident_stats) и даёт идентичный результат.
    #[test]
    fn warm_knowledge_reused_across_dispatches() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("01_SPECS");
        std::fs::create_dir_all(&spec).unwrap();
        std::fs::write(
            spec.join("PTS-777.md"),
            "# PTS-777\n\n## 4. ПОЛНЫЙ ТЕХНИЧЕСКИЙ ТЕКСТ\nрезонансный аттрактор H Psi нулевой.\n",
        )
        .unwrap();
        let kdb = dir.path().join("knowledge.db");
        let mut emb = crate::sources::knowledge::KnowledgeEmbedder::None;
        crate::sources::knowledge::ingest(
            dir.path(),
            &kdb,
            &mut emb,
            &crate::sources::knowledge::IngestOptions::default(),
        )
        .unwrap();

        let srv = McpServer::new(9222, 10, PathBuf::from("/nonexistent-web.db"))
            .with_knowledge_db(kdb.clone());
        let call = |srv: &McpServer, id: i64| {
            srv.dispatch(&json!({
                "jsonrpc": "2.0", "id": id, "method": "tools/call",
                "params": {"name": "query_poler_knowledge",
                           "arguments": {"query": "резонансный аттрактор"}}
            }))
            .unwrap()
        };
        let text_of = |v: serde_json::Value| {
            v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
        };
        let r1 = text_of(call(&srv, 1));
        assert!(
            serde_json::to_value(srv.resident_stats()).unwrap()["warm_knowledge_open"]
                == serde_json::json!(true),
            "хэндл Гиппокампа обязан быть тёплым после первого вызова"
        );
        let r2 = text_of(call(&srv, 2));
        // первая строка содержит рендер латентности («N мс») — она не
        // часть контракта эквивалентности; сравниваем тело выдачи.
        let body = |t: &str| t.split('\n').skip(1).collect::<Vec<_>>().join("\n");
        assert_eq!(body(&r1), body(&r2), "повторный тёплый вызов идентичен (кроме id и мс)");
        assert!(!r1.is_empty(), "выдача не пуста: {r1}");
    }

    /// poler_grep через резидентный кэш: второй вызов без чтения диска.
    #[test]
    fn grep_dispatch_uses_warm_file_cache() {
        let _env = crate::gateway::containers::docker_env_test_lock();
        std::env::set_var("POLER_MCP_ALLOW_ANY_PATH", "1");
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("canon.md"),
            "канон: резонанс семантический повторяется\n".repeat(64),
        )
        .unwrap();
        let srv = McpServer::new(9222, 10, PathBuf::from("/nonexistent-web.db"));
        let call = || {
            srv.dispatch(&json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "poler_grep",
                           "arguments": {"pattern": "резонанс", "path": dir.path().to_str().unwrap()}}
            }))
            .unwrap()
        };
        call();
        let s1 = srv.resident_stats();
        assert!(s1["file_cache"]["misses"].as_u64().unwrap() >= 1, "первый вызов читает диск");
        call();
        let s2 = srv.resident_stats();
        assert!(
            s2["file_cache"]["hits"].as_u64().unwrap() >= 1,
            "второй вызов обязан идти из RAM: {s2}"
        );
        assert_eq!(
            s2["file_cache"]["misses"], s1["file_cache"]["misses"],
            "повторных чтений диска нет"
        );
        std::env::remove_var("POLER_MCP_ALLOW_ANY_PATH");
    }

    /// Стриминг логов в Vault: dispatch-события попадают в шифропоток,
    /// файл читается как обычный .pvt после Drop (финальный коммит).
    #[cfg(feature = "pnd-ffi")]
    #[test]
    fn vault_log_tee_records_dispatch() {
        use crate::crypto::vault::{open as vault_open, SealOptions, VaultAppender};

        let _env = crate::gateway::containers::docker_env_test_lock();
        std::env::set_var("POLER_MCP_ALLOW_ANY_PATH", "1");
        let dir = tempfile::tempdir().unwrap();
        let stream = dir.path().join("mcp-session.pvt");
        let app = VaultAppender::create(
            &stream,
            "сессионная-фраза",
            &SealOptions { iterations: crate::crypto::kdf::MIN_ITERATIONS, ..Default::default() },
        )
        .unwrap();
        let srv = McpServer::new(9222, 10, PathBuf::from("/nonexistent-web.db"))
            .with_vault_log(app);

        srv.dispatch(&json!({"jsonrpc": "2.0", "id": 1, "method": "ping"})).unwrap();
        srv.dispatch(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})).unwrap();
        let r = srv
            .dispatch(&json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": {"name": "poler_grep",
                           "arguments": {"pattern": "x", "path": dir.path().to_str().unwrap()}}
            }))
            .unwrap();
        assert!(serde_json::to_string(&r).unwrap().contains("poler-grep"));
        drop(srv); // Drop аппендера = финальный коммит

        let out = dir.path().join("session.log");
        vault_open(&stream, &out, "сессионная-фраза").unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.contains("\"method\":\"ping\""), "ping в журнале: {text}");
        assert!(text.contains("\"method\":\"tools/list\""), "tools/list в журнале");
        assert!(text.contains("\"tool\":\"poler_grep\""), "tool-имя в журнале");
        assert!(text.contains("\"us\":"), "латентность в микросекундах в журнале");
        assert!(text.contains("\"ok\":true"), "статус в журнале");
        std::env::remove_var("POLER_MCP_ALLOW_ANY_PATH");
    }

}

#[cfg(all(test, feature = "pnd-ffi"))]
mod exec_family_tests {
    use super::*;

    fn server() -> McpServer {
        McpServer::new(9222, 10, PathBuf::from("/nonexistent-poler-exec-test.db"))
    }

    fn call(srv: &McpServer, name: &str, arguments: Value) -> Value {
        srv.dispatch(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": name, "arguments": arguments}
        }))
        .expect("dispatch не падает")
    }

    fn text_of(resp: &Value) -> String {
        resp.pointer("/result/content/0/text")
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_string()
    }

    #[test]
    fn manifest_contains_exec_family() {
        let m = tools_manifest();
        let names: Vec<&str> =
            m.iter().filter_map(|t| t.get("name").and_then(|v| v.as_str())).collect();
        for expected in
            ["poler_exec", "poler_exec_async", "poler_exec_task", "poler_exec_kill", "poler_exec_list"]
        {
            assert!(names.contains(&expected), "нет {expected}: {names:?}");
        }
    }

    #[test]
    fn is_exec_tool_routes_exec_family_only() {
        assert!(is_exec_tool(&json!({
            "method": "tools/call", "params": {"name": "poler_exec", "arguments": {}}
        })));
        assert!(is_exec_tool(&json!({
            "method": "tools/call", "params": {"name": "poler_exec_kill", "arguments": {}}
        })));
        assert!(!is_exec_tool(&json!({
            "method": "tools/call", "params": {"name": "poler_grep", "arguments": {}}
        })));
        assert!(!is_exec_tool(&json!({"method": "tools/list"})));
    }

    #[test]
    fn tool_exec_head_tail_cwd_env() {
        let srv = server();
        let resp = call(
            &srv,
            "poler_exec",
            json!({
                "command": "sh",
                "args": ["-c", "printf HEAD7; dd if=/dev/zero bs=1024 count=64 2>/dev/null; printf TAIL7"],
                "cwd": "/tmp",
                "env": {"PATH": "/bin:/usr/bin"},
                "max_out_bytes": 1024,
                "timeout_ms": 10_000
            }),
        );
        let text = text_of(&resp);
        let report: Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("не JSON ({e}): {text}"));
        assert_eq!(report["exit_code"], json!(0));
        assert_eq!(report["truncated"], json!(true));
        let stdout = report["stdout"].as_str().unwrap();
        assert!(stdout.starts_with("HEAD7"), "голова потеряна: {stdout:?}");
        assert!(stdout.ends_with("TAIL7"), "хвост потерян: {stdout:?}");
        assert!(stdout.contains("dropped"), "нет маркера: {stdout:?}");
    }

    #[test]
    fn tool_exec_rejects_unknown_capture() {
        let srv = server();
        let resp = call(
            &srv,
            "poler_exec",
            json!({"command": "echo", "args": ["x"], "capture": "sideways"}),
        );
        // ошибка аргумента → isError: true, но протокол отвечает 200-образно
        assert!(
            resp.pointer("/result/isError").and_then(|v| v.as_bool()).unwrap_or(false),
            "ожидался isError: {resp}"
        );
    }

    #[test]
    fn async_lifecycle_spawn_poll_kill() {
        let srv = server();
        // 1) фоновый запуск долгой задачи
        let resp = call(
            &srv,
            "poler_exec_async",
            json!({"command": "sleep", "args": ["30"], "timeout_ms": 60_000}),
        );
        let text = text_of(&resp);
        let report: Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("не JSON ({e}): {text}"));
        let id = report["task_id"].as_u64().expect("task_id в отчёте");
        assert_eq!(report["status"], json!("running"));

        // 2) мгновенный опрос — running
        let resp = call(&srv, "poler_exec_task", json!({"task_id": id}));
        assert!(text_of(&resp).contains("running"), "ожидался running");

        // 3) kill → killed: true
        let resp = call(&srv, "poler_exec_kill", json!({"task_id": id}));
        assert!(text_of(&resp).contains("\"killed\": true"), "ожидался killed: true");

        // 4) ожидание завершения с cancelled: true
        let resp = call(&srv, "poler_exec_task", json!({"task_id": id, "wait_ms": 5000}));
        let text = text_of(&resp);
        let report: Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("не JSON ({e}): {text}"));
        assert_eq!(report["status"], json!("done"), "задача не завершилась: {text}");
        assert_eq!(report["cancelled"], json!(true), "ожидалась отмена: {text}");
        assert_eq!(report["timed_out"], json!(false), "отмена ≠ таймаут");

        // 5) повторный kill по завершённой — killed: false
        let resp = call(&srv, "poler_exec_kill", json!({"task_id": id}));
        assert!(text_of(&resp).contains("\"killed\": false"));

        // 6) list видит задачу
        let resp = call(&srv, "poler_exec_list", json!({}));
        let text = text_of(&resp);
        let report: Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("не JSON ({e}): {text}"));
        let listed = report["tasks"].as_array().unwrap();
        assert!(listed.iter().any(|t| t["task_id"] == json!(id)), "list не видит задачу");

        // 7) несуществующая задача — ошибка протокола (isError)
        let resp = call(&srv, "poler_exec_kill", json!({"task_id": 999_999}));
        assert!(
            resp.pointer("/result/isError").and_then(|v| v.as_bool()).unwrap_or(false),
            "ожидался isError для неизвестной задачи"
        );
    }

    #[test]
    fn async_completes_normally() {
        let srv = server();
        let resp = call(
            &srv,
            "poler_exec_async",
            json!({"command": "echo", "args": ["полер-фон"], "timeout_ms": 10_000}),
        );
        let report: Value = serde_json::from_str(&text_of(&resp)).expect("JSON");
        let id = report["task_id"].as_u64().unwrap();

        let resp = call(&srv, "poler_exec_task", json!({"task_id": id, "wait_ms": 10_000}));
        let text = text_of(&resp);
        let report: Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("не JSON ({e}): {text}"));
        assert_eq!(report["status"], json!("done"), "{text}");
        assert_eq!(report["exit_code"], json!(0), "{text}");
        assert!(report["stdout"].as_str().unwrap().contains("полер-фон"), "{text}");
    }
}

#[cfg(test)]
mod fly_family_tests {
    use super::*;

    fn server() -> McpServer {
        McpServer::new(9222, 10, PathBuf::from("/nonexistent-poler-fly-test.db"))
    }

    fn call(srv: &McpServer, name: &str, arguments: Value) -> Value {
        srv.dispatch(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": name, "arguments": arguments}
        }))
        .expect("dispatch не падает")
    }

    fn text_of(resp: &Value) -> String {
        resp.pointer("/result/content/0/text")
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_string()
    }

    fn json_of(resp: &Value) -> Value {
        let text = text_of(resp);
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("не JSON ({e}): {text}"))
    }

    fn core_csr() -> String {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/flywire-connectome/flywire_v783_core.csr.zst");
        assert!(p.exists(), "артефакт {} не найден", p.display());
        p.display().to_string()
    }

    #[test]
    fn manifest_contains_fly_family() {
        let m = tools_manifest();
        let names: Vec<&str> =
            m.iter().filter_map(|t| t.get("name").and_then(|v| v.as_str())).collect();
        for expected in [
            "poler_fly",
            "poler_fly_node",
            "poler_fly_edge",
            "poler_fly_khop",
            "poler_fly_path",
            "poler_fly_common",
            "poler_fly_centrality",
            "poler_fly_rotor",
            "poler_fly_motifs",
            "poler_fly_propagate",
        ] {
            assert!(names.contains(&expected), "нет {expected}: {names:?}");
        }
    }

    #[test]
    fn is_fly_tool_routes_family_only() {
        for name in ["poler_fly", "poler_fly_node", "poler_fly_propagate"] {
            assert!(is_fly_tool(&json!({
                "method": "tools/call", "params": {"name": name, "arguments": {}}
            })));
        }
        assert!(!is_fly_tool(&json!({
            "method": "tools/call", "params": {"name": "poler_grep", "arguments": {}}
        })));
        assert!(!is_fly_tool(&json!({"method": "tools/list"})));
    }

    // Полный жизненный цикл теплоты: холодная загрузка → тёплый вызов
    // без csr → eject → отказ без csr.
    #[test]
    fn fly_warm_lifecycle() {
        let srv = server();
        // 1) холодная загрузка + сводка (золотые числа C1)
        let r = json_of(&call(&srv, "poler_fly", json!({"csr": core_csr()})));
        assert_eq!(r["n_nodes"], json!(138_639));
        assert_eq!(r["n_edges"], json!(2_700_513));
        assert_eq!(r["total_mass"], json!(34_153_566));
        assert_eq!(r["core"], json!(true));
        assert!(r["load_ms"].as_u64().unwrap() > 0, "load_ms: {r}");
        // 2) тёплый вызов без csr — муха уже в RAM
        let r2 = json_of(&call(&srv, "poler_fly", json!({})));
        assert_eq!(r2["n_nodes"], json!(138_639));
        assert_eq!(r2["warm"]["csc_built"], json!(false), "сводка не строит CSC");
        assert_eq!(r2["warm"]["has_nodes"], json!(false));
        // 3) eject
        let r3 = json_of(&call(&srv, "poler_fly", json!({"action": "eject"})));
        assert_eq!(r3["unloaded"], json!(true));
        let r4 = json_of(&call(&srv, "poler_fly", json!({"action": "eject"})));
        assert_eq!(r4["unloaded"], json!(false), "повторный eject — пусто");
        // 4) после eject без csr — протокольная ошибка
        let resp = call(&srv, "poler_fly_node", json!({"neuron": 0}));
        assert!(
            resp.pointer("/result/isError").and_then(|v| v.as_bool()).unwrap_or(false),
            "ожидался isError без загруженного коннектома: {resp}"
        );
        assert!(text_of(&resp).contains("коннектом не загружен"));
    }

    // Паспорт/ребро/K-hop на золотых числах узла 0.
    #[test]
    fn fly_node_edge_khop_golden() {
        let srv = server();
        let csr = core_csr();
        let r = json_of(&call(&srv, "poler_fly_node", json!({"csr": csr, "neuron": 0})));
        assert_eq!(r["out_degree"], json!(13));
        assert_eq!(r["in_degree"], json!(13));
        let top_in = r["top_in"].as_array().unwrap();
        assert!(!top_in.is_empty());
        assert_eq!(top_in[0]["other"], json!(79_529));
        assert_eq!(top_in[0]["weight"], json!(17));
        assert_eq!(top_in[0]["nt"], json!("gaba"));
        assert_eq!(top_in[0]["sign"], json!(-1));
        // root_id без таблицы узлов — null (честно)
        assert_eq!(top_in[0]["other_root_id"], json!(null));
        // ребро 0→6135 + ротор
        let e = json_of(&call(&srv, "poler_fly_edge", json!({"u": 0, "v": 6135})));
        assert_eq!(e["found"], json!(true));
        assert_eq!(e["forward"]["weight"], json!(5));
        assert_eq!(e["forward"]["nt"], json!("ach"));
        assert_eq!(e["backward"], json!(null));
        assert_eq!(e["rotor_Juv"], json!(5));
        assert_eq!(e["rotor_Jvu"], json!(-5));
        // K-hop: фронты [13, 443], достигнуто 457
        let k = json_of(&call(&srv, "poler_fly_khop", json!({"neuron": 0, "depth": 2})));
        assert_eq!(k["frontier_sizes"], json!([13, 443]));
        assert_eq!(k["visited"], json!(457));
        // фильтр возбуждающих сужает поток
        let k2 = json_of(&call(&srv, "poler_fly_khop", json!({"neuron": 0, "depth": 2, "sign": "exc"})));
        let exc_visited = k2["visited"].as_u64().unwrap();
        assert!(exc_visited <= 457, "exc-поток не шире полного: {exc_visited}");
    }

    // Путь/общие партнёры/ротор-топ/центральность на реальном ядре.
    #[test]
    fn fly_path_common_rotor_centrality() {
        let srv = server();
        let csr = core_csr();
        // прямой путь 0→6135 (ребро w5 ach)
        let p = json_of(&call(&srv, "poler_fly_path", json!({"csr": csr, "from": 0, "to": 6135})));
        assert_eq!(p["found"], json!(true));
        assert_eq!(p["length"], json!(1));
        assert_eq!(p["total_weight"], json!(5));
        let hops = p["hops"].as_array().unwrap();
        assert_eq!(hops.len(), 1);
        assert_eq!(hops[0]["from"], json!(0));
        assert_eq!(hops[0]["to"], json!(6135));
        assert_eq!(hops[0]["nt"], json!("ach"));
        // пустой путь 0→0
        let p0 = json_of(&call(&srv, "poler_fly_path", json!({"from": 0, "to": 0})));
        assert_eq!(p0["found"], json!(true));
        assert_eq!(p0["length"], json!(0));
        // общие мишени соседей 0 и 6135 — обязаны быть (кольца мозга)
        let c = json_of(&call(&srv, "poler_fly_common", json!({"neurons": [0, 6135]})));
        assert!(
            c["count"].as_u64().unwrap() > 0,
            "у соседей обязаны быть общие мишени: {c}"
        );
        let partners = c["partners"].as_array().unwrap();
        assert!(!partners.is_empty());
        assert!(partners[0]["total_weight"].as_u64().unwrap() > 0);
        // топ ротора: антисимметрия + убывание
        let rot = json_of(&call(&srv, "poler_fly_rotor", json!({"top": 5, "min_abs": 10})));
        assert_eq!(rot["count"], json!(5));
        let pairs = rot["pairs"].as_array().unwrap();
        let j0 = pairs[0]["J_uv"].as_i64().unwrap();
        assert!(j0 >= 10, "топ-циркуляция < порога: {j0}");
        assert_eq!(pairs[0]["J_vu"].as_i64().unwrap(), -j0);
        for w in pairs.windows(2) {
            assert!(
                w[0]["J_uv"].as_i64().unwrap() >= w[1]["J_uv"].as_i64().unwrap(),
                "ротор не убывает: {w:?}"
            );
        }
        // центральность: хабы + PageRank
        let cen = json_of(&call(&srv, "poler_fly_centrality", json!({"top": 5})));
        assert_eq!(cen["top_out_degree"].as_array().unwrap().len(), 5);
        assert_eq!(cen["top_in_degree"].as_array().unwrap().len(), 5);
        let pr = cen["pagerank"]["top"].as_array().unwrap();
        assert_eq!(pr.len(), 5);
        for w in pr.windows(2) {
            assert!(
                w[0]["rank"].as_f64().unwrap() >= w[1]["rank"].as_f64().unwrap(),
                "ранг не убывает: {w:?}"
            );
        }
        assert!(pr.iter().all(|x| x["rank"].as_f64().unwrap() > 0.0));
    }

    // Мотивы и симуляция на реальном ядре.
    #[test]
    fn fly_motifs_propagate() {
        let srv = server();
        let csr = core_csr();
        let m = json_of(&call(&srv, "poler_fly_motifs", json!({"csr": csr, "neuron": 0})));
        assert_eq!(m["out_degree"], json!(13));
        assert_eq!(m["in_degree"], json!(13));
        assert!(m["reciprocal_count"].is_u64());
        assert!(m["feedforward"].is_u64());
        assert!(m["feedback3"].is_u64());
        // симуляция от узла 0: сигнал расходится по 13 исходящим
        let p = json_of(&call(
            &srv,
            "poler_fly_propagate",
            json!({"neurons": [0], "steps": 3, "gamma": 0.05, "leak": 0.8}),
        ));
        let tl = p["timeline"].as_array().unwrap();
        assert_eq!(tl.len(), 3);
        assert_eq!(tl[0]["step"], json!(1));
        assert!(
            tl[0]["active"].as_u64().unwrap() >= 2,
            "сигнал не вышел за семя: {p}"
        );
        assert!(tl[0]["positive_mass"].as_f64().unwrap() > 0.0);
        assert!(p["top"].as_array().unwrap().len() >= 2);
        // неверный нейрон — протокольная ошибка
        let resp = call(&srv, "poler_fly_propagate", json!({"neurons": [999_999_999]}));
        assert!(
            resp.pointer("/result/isError").and_then(|v| v.as_bool()).unwrap_or(false),
            "ожидался isError для нейрона вне диапазона"
        );
        // отдельный сервер без загрузки — отказ до валидации аргументов
        let srv2 = server();
        let resp = call(&srv2, "poler_fly_propagate", json!({"neurons": [0]}));
        assert!(text_of(&resp).contains("коннектом не загружен"));
    }
}

#[cfg(test)]
mod psi_family_tests {
    use super::*;

    fn server() -> McpServer {
        McpServer::new(9222, 10, PathBuf::from("/nonexistent-poler-psi-test.db"))
    }

    fn call(srv: &McpServer, name: &str, arguments: Value) -> Value {
        srv.dispatch(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": name, "arguments": arguments}
        }))
        .expect("dispatch не падает")
    }

    fn text_of(resp: &Value) -> String {
        resp.pointer("/result/content/0/text")
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_string()
    }

    fn json_of(resp: &Value) -> Value {
        let text = text_of(resp);
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("не JSON ({e}): {text}"))
    }

    fn core_csr() -> String {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/flywire-connectome/flywire_v783_core.csr.zst");
        assert!(p.exists(), "артефакт {} не найден", p.display());
        p.display().to_string()
    }

    const INTENT: &str =
        "герой идёт в поход против тьмы и бездны, наставник даёт совет и знание";

    #[test]
    fn manifest_contains_psi_family() {
        let m = tools_manifest();
        let names: Vec<&str> =
            m.iter().filter_map(|t| t.get("name").and_then(|v| v.as_str())).collect();
        for expected in [
            "poler_literary_field",
            "poler_literary_step",
            "poler_literary_generate",
            "poler_literary_eject",
        ] {
            assert!(names.contains(&expected), "нет {expected}: {names:?}");
        }
    }

    #[test]
    fn manifest_contains_ssn_family() {
        let m = tools_manifest();
        let names: Vec<&str> =
            m.iter().filter_map(|t| t.get("name").and_then(|v| v.as_str())).collect();
        for expected in [
            "poler_ssn_step",
            "poler_ssn_inject",
            "poler_ssn_status",
            "poler_ssn_eject",
        ] {
            assert!(names.contains(&expected), "нет {expected}: {names:?}");
        }
    }

    #[test]
    fn is_ssn_tool_routes_family_only() {
        for name in ["poler_ssn_step", "poler_ssn_inject", "poler_ssn_status", "poler_ssn_eject"] {
            assert!(is_ssn_tool(&json!({
                "method": "tools/call",
                "params": {"name": name, "arguments": {}}
            })));
        }
        assert!(!is_ssn_tool(&json!({
            "method": "tools/list"
        })));
        assert!(!is_ssn_tool(&json!({
            "method": "tools/call",
            "params": {"name": "poler_literary_step", "arguments": {}}
        })));
    }

    #[test]
    fn manifest_contains_triune_family() {
        let m = tools_manifest();
        let names: Vec<&str> =
            m.iter().filter_map(|t| t.get("name").and_then(|v| v.as_str())).collect();
        for expected in ["poler_triune_speak", "poler_triune_state"] {
            assert!(names.contains(&expected), "нет {expected}: {names:?}");
        }
    }

    #[test]
    fn is_triune_tool_routes_family_only() {
        for name in ["poler_triune_speak", "poler_triune_state"] {
            assert!(is_triune_tool(&json!({
                "method": "tools/call",
                "params": {"name": name, "arguments": {}}
            })));
        }
        assert!(!is_triune_tool(&json!({"method": "tools/list"})));
        assert!(!is_triune_tool(&json!({
            "method": "tools/call",
            "params": {"name": "poler_ssn_step", "arguments": {}}
        })));
    }

    #[test]
    fn triune_tools_smoke_via_dispatch() {
        // полный цикл: родить Триединство → речь → снимок → продолжить разговор
        let db = std::env::temp_dir().join("poler_mcp_triune_smoke.db");
        let srv = McpServer::new(0, 0, db);
        let r = srv
            .dispatch(&json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "poler_triune_speak",
                           "arguments": {"text": "живой мозг говорит", "tokens": 8, "seed": 42}}
            }))
            .expect("речь");
        let text = r
            .pointer("/result/content/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let v: Value = serde_json::from_str(text).expect("речь — валидный JSON");
        let id = v.get("session").and_then(|s| s.as_str()).expect("session id").to_string();
        let speech = v.get("speech").and_then(|s| s.as_str()).unwrap_or("");
        assert!(!speech.is_empty(), "Триединство должно говорить: {text}");
        let activity = v.pointer("/brain/activity").and_then(|x| x.as_f64()).unwrap_or(1.0);
        assert!(activity < 0.5, "активность {activity} — эпилепсия в smoke-тесте");
        assert!(v.get("trace_head").is_some(), "провенанс токенов обязателен");

        let r = srv
            .dispatch(&json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": {"name": "poler_triune_state", "arguments": {"session": id}}
            }))
            .expect("снимок");
        let text = r
            .pointer("/result/content/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let st: Value = serde_json::from_str(text).expect("снимок — валидный JSON");
        assert_eq!(
            st.get("last_speech").and_then(|s| s.as_str()).unwrap_or(""),
            speech,
            "снимок помнит последнюю фразу"
        );
        assert!(st.get("tokens_spoken").and_then(|t| t.as_u64()).unwrap_or(0) >= 8);

        // Продолжение того же разговора: мозг и муха живы между вызовами.
        let r = srv
            .dispatch(&json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": {"name": "poler_triune_speak",
                           "arguments": {"session": id, "text": "продолжай мысль", "tokens": 6}}
            }))
            .expect("продолжение");
        assert!(serde_json::to_string(&r).unwrap().contains("speech"));
    }

    #[test]
    fn ssn_tools_smoke_via_dispatch() {
        // полный цикл: создать мозг → шагнуть → статус → выгрузить
        let db = std::env::temp_dir().join("poler_mcp_ssn_smoke.db");
        let srv = McpServer::new(0, 0, db);
        let r = srv
            .dispatch(&json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "poler_ssn_step", "arguments": {"steps": 1500, "seed": 42, "n": 300}}
            }))
            .expect("создание мозга");
        let text = r
            .pointer("/result/content/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let t_val: Value = serde_json::from_str(text).expect("телеметрия — валидный JSON");
        let id = t_val
            .pointer("/telemetry/session")
            .and_then(|s| s.as_str())
            .expect("session id в ответе")
            .to_string();
        let activity = t_val.pointer("/telemetry/activity").and_then(|v| v.as_f64()).unwrap_or(1.0);
        assert!(activity < 0.5, "активность {activity} — эпилепсия в smoke-тесте");

        let r = srv
            .dispatch(&json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": {"name": "poler_ssn_inject", "arguments": {"session": id, "text": "проверка связи", "steps": 200}}
            }))
            .expect("инъекция");
        assert!(serde_json::to_string(&r).unwrap().contains("neurons_touched"));

        let r = srv
            .dispatch(&json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": {"name": "poler_ssn_status", "arguments": {"session": id}}
            }))
            .expect("статус");
        assert!(serde_json::to_string(&r).unwrap().contains("criticality"));

        let r = srv
            .dispatch(&json!({
                "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                "params": {"name": "poler_ssn_eject", "arguments": {}}
            }))
            .expect("выгрузка");
        let text = r
            .pointer("/result/content/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let ev: Value = serde_json::from_str(text).expect("eject — валидный JSON");
        let ejected = ev.get("ejected").and_then(|v| v.as_u64()).unwrap_or(0);
        assert_eq!(ejected, 1, "должен выгрузиться один мозг: {text}");
    }

    #[test]
    fn is_psi_tool_routes_family_only() {
        for name in ["poler_literary_step", "poler_literary_generate"] {
            assert!(is_psi_tool(&json!({
                "method": "tools/call", "params": {"name": name, "arguments": {}}
            })));
        }
        assert!(!is_psi_tool(&json!({
            "method": "tools/call", "params": {"name": "poler_grep", "arguments": {}}
        })));
        assert!(!is_psi_tool(&json!({"method": "tools/list"})));
    }

    // Полный жизненный цикл сессии: создать → шагнуть (эволюция
    // замысла) → сгенерировать → eject → шагнуть в пустоту (отказ).
    #[test]
    fn psi_session_lifecycle() {
        let srv = server();
        // 1) анализ поля: архетипы ранжированы, термы на месте
        let f = json_of(&call(&srv, "poler_literary_field", json!({"text": INTENT})));
        assert_eq!(f["dims"], json!(64));
        assert!(f["tokens"].as_u64().unwrap() >= 10);
        assert!(f["archetypes"].as_array().unwrap().len() == 6);
        assert!(f["f_initial"].as_f64().unwrap() > 0.9, "F₀ ≈ ‖o‖² = 1");

        // 2) создать сессию и шагнуть (запас шагов: после смены
        // замысла траектории нужно время на пере-притяжение)
        let s1 = json_of(&call(
            &srv,
            "poler_literary_step",
            json!({"text": INTENT, "steps": 128}),
        ));
        let id = s1["session"].as_str().expect("session id").to_string();
        assert_eq!(s1["step"], json!(1));
        assert!(s1["f_free"].as_f64().unwrap() < 1.0, "первый шаг уже снижает F");

        // 3) эволюция замысла: observation меняет Ω(o)
        let s2 = json_of(&call(
            &srv,
            "poler_literary_step",
            json!({"session": id, "observation": "тишина зимнего сада и покой"}),
        ));
        assert_eq!(s2["step"], json!(2));
        assert_eq!(s2["session"], json!(id));

        // 4) генерация на резидентной сессии — сходится
        let g = json_of(&call(&srv, "poler_literary_generate", json!({"session": id})));
        assert_eq!(g["converged"], json!(true), "чистая физика текста сходится");
        assert!(g["f_final"].as_f64().unwrap() < 1e-3);
        assert_eq!(g["causality_clean"], json!(true));
        assert!(g["acts"].as_array().unwrap().len() == 3);
        assert!(g["text"].as_str().unwrap().contains("СВЕРХПРОВОДИМОСТЬ"));

        // 5) eject конкретной сессии
        let e = json_of(&call(&srv, "poler_literary_eject", json!({"session": id})));
        assert_eq!(e["ejected"], json!(1));
        // шаг в выгруженную сессию — протокольная ошибка
        let resp = call(&srv, "poler_literary_step", json!({"session": id}));
        assert!(
            resp.pointer("/result/isError").and_then(|v| v.as_bool()).unwrap_or(false),
            "сессия обязана исчезнуть"
        );
    }

    // Мушиная калибровка через один и тот же тёплый коннектом:
    // field → generate с csr (предельный цикл!) → eject all.
    #[test]
    fn psi_fly_calibrated_golden() {
        let srv = server();
        // 1) field с мушиной калибровкой: каста от нейрона 0
        let f = json_of(&call(
            &srv,
            "poler_literary_field",
            json!({"text": INTENT, "csr": core_csr()}),
        ));
        assert_eq!(f["fly"]["members"], json!(48), "каста золото L1");
        assert_eq!(f["fly"]["reached"], json!(457));
        assert_eq!(f["fly"]["inner_edges"], json!(252));

        // 2) generate с мушиной калибровкой: предельный цикл
        let g = json_of(&call(
            &srv,
            "poler_literary_generate",
            json!({"text": INTENT, "csr": core_csr()}),
        ));
        assert_eq!(g["converged"], json!(false), "муха не даёт замереть");
        let ff = g["f_final"].as_f64().unwrap();
        assert!((ff - 0.2572).abs() < 5e-3, "плато цикла: {ff}");
        assert_eq!(g["causality_clean"], json!(true));
        // драматургические пары мухи
        let pairs = g["dramatic_pairs"].as_array().expect("пары");
        assert!(!pairs.is_empty());
        assert_eq!(pairs[0]["u"], json!(13_609));
        assert_eq!(pairs[0]["v"], json!(41_414));
        // коннектом остался тёплым (переиспользование WarmFly)
        let summary = json_of(&call(&srv, "poler_fly", json!({})));
        assert_eq!(summary["n_nodes"], json!(138_639), "муха всё ещё в RAM");

        // 3) чистка: сессий нет, но коннектом выгружаем отдельно
        let e = json_of(&call(&srv, "poler_literary_eject", json!({})));
        assert_eq!(e["ejected"], json!(0), "one-shot сессий не было");
        let fe = json_of(&call(&srv, "poler_fly", json!({"action": "eject"})));
        assert_eq!(fe["unloaded"], json!(true));
    }

    // No-Mul сессия: Trit5-квантование на резидентном полигоне.
    #[test]
    fn psi_no_mul_session_and_guards() {
        let srv = server();
        let s = json_of(&call(
            &srv,
            "poler_literary_step",
            json!({"text": "бунт против шторма и свобода огня", "no_mul": true}),
        ));
        let id = s["session"].as_str().unwrap().to_string();
        assert_eq!(s["step"], json!(1));
        // гварды: без text и session — ошибка
        let resp = call(&srv, "poler_literary_step", json!({}));
        assert!(resp.pointer("/result/isError").and_then(|v| v.as_bool()).unwrap_or(false));
        // ограничение вне диапазона — ошибка
        let resp = call(
            &srv,
            "poler_literary_generate",
            json!({"text": "текст", "constraints": [{"i": 0, "j": 999}]}),
        );
        assert!(resp.pointer("/result/isError").and_then(|v| v.as_bool()).unwrap_or(false));
        // session и text вместе — session приоритетен, не падаем
        let s2 = json_of(&call(
            &srv,
            "poler_literary_step",
            json!({"session": id, "text": "ignored"}),
        ));
        assert_eq!(s2["session"], json!(id));
        assert_eq!(s2["step"], json!(2));
        let _ = call(&srv, "poler_literary_eject", json!({"session": "all"}));
    }

    // LRU: 9+ сессий вытесняют старейшую.
    #[test]
    fn psi_sessions_lru_cap() {
        let srv = server();
        let mut first_id = String::new();
        for i in 0..10 {
            let s = json_of(&call(
                &srv,
                "poler_literary_step",
                json!({"text": format!("замысел номер {i}")}),
            ));
            if i == 0 {
                first_id = s["session"].as_str().unwrap().to_string();
            }
        }
        // первая сессия вытеснена (лимит 8)
        let resp = call(&srv, "poler_literary_step", json!({"session": first_id}));
        assert!(
            resp.pointer("/result/isError").and_then(|v| v.as_bool()).unwrap_or(false),
            "LRU обязан вытеснить старейшую"
        );
        // но 9-я и 10-я живы
        let e = json_of(&call(&srv, "poler_literary_eject", json!({"session": "all"})));
        assert_eq!(e["ejected"], json!(8));
    }
}

#[cfg(test)]
mod quantum_tool_tests {
    use super::*;

    fn server() -> McpServer {
        McpServer::new(9222, 10, PathBuf::from("/nonexistent-poler-quantum-test.db"))
    }

    fn call(srv: &McpServer, arguments: Value) -> Value {
        let r = srv
            .dispatch(&json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "poler_quantum", "arguments": arguments}
            }))
            .expect("dispatch не падает");
        let text = r
            .pointer("/result/content/0/text")
            .and_then(|v| v.as_str())
            .expect("text есть");
        serde_json::from_str(text).expect("валидный JSON инструмента")
    }

    #[test]
    fn manifest_declares_poler_quantum() {
        let m = tools_manifest();
        let t = m
            .iter()
            .find(|t| t.get("name").and_then(|v| v.as_str()) == Some("poler_quantum"))
            .expect("poler_quantum в манифесте");
        let props = t.pointer("/inputSchema/properties").unwrap();
        assert!(props.to_string().contains("up_to_global_phase"));
        let required = t.pointer("/inputSchema/required").unwrap();
        assert!(required.to_string().contains("action"));
    }

    #[test]
    fn list_returns_catalog_with_teleport() {
        let v = call(&server(), json!({"action": "list"}));
        let algos: Vec<&str> = v["algorithms"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|a| a["algorithm"].as_str())
            .collect();
        assert!(algos.contains(&"bell"));
        assert!(algos.contains(&"teleport"));
        assert!(algos.contains(&"period"));
    }

    #[test]
    fn run_ghz_born_distribution() {
        let v = call(&server(), json!({"action": "run", "algorithm": "ghz", "n": 4, "shots": 512}));
        assert_eq!(v["algorithm"], "ghz");
        assert_eq!(v["n_qubits"], 4);
        // GHZ-4: ровно две ненулевые вероятности по 0.5
        let probs = v["probabilities"].as_array().unwrap();
        let nonzero: Vec<&Value> = probs.iter().filter(|p| p.as_f64().unwrap() > 1e-9).collect();
        assert_eq!(nonzero.len(), 2, "GHZ: {probs:?}");
        for p in nonzero {
            assert!((p.as_f64().unwrap() - 0.5).abs() < 1e-9);
        }
    }

    #[test]
    fn teleport_exact_has_unit_fidelity() {
        let v = call(&server(), json!({"action": "teleport", "exact": true}));
        assert_eq!(v["exact"], true);
        assert_eq!(v["fidelity"], 1.0);
        assert_eq!(v["branch_deviation"], 0.0);
        assert_eq!(v["gate_count"], 6);
    }

    #[test]
    fn bloch_accepts_calc_expressions() {
        let v = call(
            &server(),
            json!({"action": "bloch", "alpha": "1/sqrt(2)", "beta": "1/sqrt(2)"}),
        );
        assert!((v["bloch"]["x"].as_f64().unwrap() - 1.0).abs() < 1e-12);
        assert!(v["bloch"]["z"].as_f64().unwrap().abs() < 1e-12);
    }

    #[test]
    fn verify_teleport_channel_proved_exact() {
        let v = call(&server(), json!({"action": "verify", "check": "teleport"}));
        assert_eq!(v["property"], "teleport_channel");
        assert_eq!(v["verdict"], "proved_exact");
        assert_eq!(v["max_deviation"], 0.0);
    }

    #[test]
    fn verify_unitary_qft3_exact_and_qft4_numeric() {
        let v = call(
            &server(),
            json!({"action": "verify", "check": "unitary", "algorithm": "qft", "n": 3}),
        );
        assert_eq!(v["verdict"], "proved_exact", "{v}");
        let v = call(
            &server(),
            json!({"action": "verify", "check": "unitary", "algorithm": "qft", "n": 4}),
        );
        assert_eq!(v["method"], "numeric:f64");
        assert_eq!(v["verdict"][0], "verified_numeric");
    }

    #[test]
    fn verify_equivalence_refutes_qft_vs_iqft() {
        // U_qft ≠ U_iqft (это взаимно обратные, не равные) — честное
        // опровержение слоем фальсификации; композиция qft∘iqft = I доказана
        // юнит-тестами крейта pqc.
        let v = call(
            &server(),
            json!({"action": "verify", "check": "equivalence", "algorithms": ["qft", "iqft"], "n": 3}),
        );
        assert_eq!(v["verdict"][0], "refuted", "{v}");
    }

    #[test]
    fn quantum_requires_action() {
        let r = server().tool_quantum(&json!({}));
        assert!(r.is_err());
        let e = r.unwrap_err();
        assert!(e.contains("action"), "{e}");
    }

    // -----------------------------------------------------------------
    // v0.52.0 (цикл Q): qcasm, qaoa, шум поверх верификатора
    // -----------------------------------------------------------------

    #[test]
    fn qcasm_runs_bell_from_source() {
        let v = call(
            &server(),
            json!({"action": "qcasm", "source": "qubits 2\nh 0\ncx 0 1\nmeasure all", "shots": 256}),
        );
        assert_eq!(v["action"], "qcasm");
        assert_eq!(v["n_qubits"], 2);
        let probs = v["probabilities"].as_array().unwrap();
        assert!((probs[0].as_f64().unwrap() - 0.5).abs() < 1e-12);
        assert!((probs[3].as_f64().unwrap() - 0.5).abs() < 1e-12);
        assert!(probs[1].as_f64().unwrap() < 1e-12);
        // ошибка разбора — честная
        let r = server().tool_quantum(&json!({"action": "qcasm", "source": "h 0"}));
        assert!(r.is_err());
    }

    #[test]
    fn qcasm_noise_degrades_ghz() {
        let v = call(
            &server(),
            json!({
                "action": "qcasm",
                "source": "qubits 3\nh 0\ncx 0 1\ncx 0 2\nmeasure all",
                "noise": "noisy-90s",
                "shots": 4000,
                "seed": 1
            }),
        );
        assert_eq!(v["mode"], "noise-mcwf");
        assert!(v["tvd"].as_f64().unwrap() > 0.10, "TVD = {}", v["tvd"]);
        assert!(v["noisy_peak"].as_f64().unwrap() < v["ideal_peak"].as_f64().unwrap());
        assert!(v["chi2"].as_f64().unwrap() > 30.0);
    }

    #[test]
    fn qaoa_optimizes_triangle() {
        let v = call(
            &server(),
            json!({"action": "qaoa", "edges": [[0, 1], [1, 2], [0, 2]], "p": 2, "restarts": 2, "sweeps": 10}),
        );
        assert_eq!(v["action"], "qaoa");
        assert_eq!(v["problem"], "maxcut");
        assert_eq!(v["n_qubits"], 3);
        assert!(v["expected_cut"].as_f64().unwrap() > 1.8, "{v}");
        assert_eq!(v["best_cut"], 2);
        assert_eq!(v["optimum"], 2);
        assert!(v["approx_ratio"].as_f64().unwrap() >= 0.999);
        // edges строкой
        let v2 = call(
            &server(),
            json!({"action": "qaoa", "edges": "0-1,1-2,0-2", "p": 1, "restarts": 1, "sweeps": 6}),
        );
        assert_eq!(v2["n_qubits"], 3);
        assert!(v2["expected_cut"].as_f64().unwrap() > 1.7);
    }

    #[test]
    fn verify_with_noise_block() {
        let v = call(
            &server(),
            json!({
                "action": "verify",
                "check": "unitary",
                "algorithm": "ghz",
                "n": 4,
                "noise": "ideal",
                "noise_shots": 2000,
                "seed": 1
            }),
        );
        assert_eq!(v["verdict"], "proved_exact");
        let noise = v["noise"].as_object().expect("блок шума");
        assert_eq!(noise["preset"], "ideal");
        assert!(noise["tvd"].as_f64().unwrap() < 0.08);
        // без noise поле null
        let v2 = call(
            &server(),
            json!({"action": "verify", "check": "teleport"}),
        );
        assert!(v2["noise"].is_null());
    }
}

#[cfg(test)]
mod scivoice_tool_tests {
    use super::*;

    fn server() -> McpServer {
        McpServer::new(9223, 10, PathBuf::from("/nonexistent-poler-scivoice-test.db"))
    }

    fn call(srv: &McpServer, arguments: Value) -> Value {
        let r = srv
            .dispatch(&json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "poler_scivoice", "arguments": arguments}
            }))
            .expect("dispatch не падает");
        let text = r
            .pointer("/result/content/0/text")
            .and_then(|v| v.as_str())
            .expect("text есть");
        serde_json::from_str(text).expect("валидный JSON инструмента")
    }

    #[test]
    fn manifest_declares_poler_scivoice() {
        let m = tools_manifest();
        let t = m
            .iter()
            .find(|t| t.get("name").and_then(|v| v.as_str()) == Some("poler_scivoice"))
            .expect("poler_scivoice в манифесте");
        let props = t.pointer("/inputSchema/properties").unwrap();
        assert!(props.to_string().contains("mode"));
        let required = t.pointer("/inputSchema/required").unwrap();
        assert!(required.to_string().contains("text"));
    }

    #[test]
    fn molecule_mode_passport() {
        let v = call(&server(), json!({"text": "кофеин", "mode": "molecule"}));
        assert_eq!(v["formula"].as_str().unwrap(), "C8H10N4O2");
        assert_eq!(v["heavy_atoms"].as_i64().unwrap(), 14);
        assert_eq!(v["aromatic_rings"].as_i64().unwrap(), 0, "кекulé-форма: кольца не помечены");
        assert_eq!(v["rings"].as_i64().unwrap(), 2);
        assert_eq!(v["h_bond_acceptors"].as_i64().unwrap(), 6);
        assert_eq!(v["h_bond_donors"].as_i64().unwrap(), 0);
        let reading = v["reading"].as_str().unwrap();
        assert!(reading.contains("C8H10N4O2"));
    }

    #[test]
    fn molecule_mode_smiles_direct() {
        let v = call(
            &server(),
            json!({"text": "CC(=O)Oc1ccccc1C(=O)O", "mode": "molecule"}),
        );
        assert_eq!(v["formula"].as_str().unwrap(), "C9H8O4");
        assert_eq!(v["rings"].as_i64().unwrap(), 1);
    }

    #[test]
    fn bio_mode_binding_report() {
        let v = call(
            &server(),
            json!({"text": "вода", "target": "вода", "mode": "bio"}),
        );
        assert!(v["delta_g_kj_mol"].is_f64());
        assert!(v["ln_kd"].is_f64());
        assert!(v["log10_kd"].is_f64());
        assert!(v["trit_depth_kd"].is_f64());
        assert!(v["tree27"]["objects"].is_i64());
        assert!(v["de9im_pattern"].is_string());
        assert_eq!(v["trit_verdict"].as_i64().unwrap_or(99) == 99, false);
        let reading = v["reading"].as_str().unwrap();
        assert!(reading.contains("ΔG"));
        assert!(reading.contains("Трит-вердикт"));
    }

    #[test]
    fn bio_mode_requires_target() {
        // Ошибка инструмента — JSON-RPC result с isError (текст не JSON)
        let r = server()
            .dispatch(&json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "poler_scivoice", "arguments": {"text": "вода", "mode": "bio"}}
            }))
            .expect("dispatch");
        let text = r
            .pointer("/result/content/0/text")
            .and_then(|v| v.as_str())
            .expect("text есть");
        assert!(text.contains("ошибка"), "без target — ошибка: {text}");
        assert_eq!(r.pointer("/result/isError").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn read_mode_speaks_like_scientist() {
        let v = call(&server(), json!({"text": "C6H12O6", "mode": "read"}));
        let reading = v["reading"].as_str().expect("reading есть");
        assert!(reading.contains("Научное чтение"));
        assert!(reading.contains("Химротор"));
        assert!(reading.contains("Трит-вердикт"));
    }

    #[test]
    fn read_mode_ab_initio_section_for_h2() {
        let v = call(&server(), json!({"text": "H2"}));
        let reading = v["reading"].as_str().expect("reading есть");
        assert!(
            reading.contains("Ab initio RHF/STO-3G"),
            "H2 обязан говорить Хартри–Фоком: {reading}"
        );
        assert!(reading.contains("-1.116714"), "E(H2) после PySCF-сверки: {reading}");
    }

    #[test]
    fn hf_mode_returns_machine_readable_json() {
        let v = call(&server(), json!({"text": "H2", "mode": "hf"}));
        let e = v["energy_hartree"].as_f64().expect("energy_hartree есть");
        assert!((e - (-1.116714)).abs() < 3e-4, "E(H2) = {e}");
        assert_eq!(v["n_basis"], json!(2));
        assert_eq!(v["scf_converged"], json!(true));
        let occ = v["occupied_orbital_energies_hartree"]
            .as_array()
            .expect("энергии МО — массив");
        assert_eq!(occ.len(), 1);
    }

    #[test]
    fn hf_mode_lih_full() {
        let v = call(&server(), json!({"text": "LiH@3.0", "mode": "hf"}));
        let e = v["energy_hartree"].as_f64().expect("energy_hartree есть");
        assert!((e - (-7.862246)).abs() < 2e-3, "E(LiH) = {e}");
        assert_eq!(v["n_basis"], json!(6));
    }

    #[test]
    fn text_argument_is_required() {
        let r = server().dispatch(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": "poler_scivoice", "arguments": {}}
        }));
        // dispatch возвращает Option<Value>; ошибка инструмента отражается
        // полем isError в ответе
        match r {
            Some(resp) => {
                let is_err = resp
                    .pointer("/result/isError")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                assert!(is_err, "без text инструмент обязан вернуть isError: {resp}");
            }
            None => panic!("dispatch не должен молча исчезать"),
        }
    }
}
