//! Web-Native Ingestion: рендер страницы через Chromium CDP → полный
//! POLER-пайплайн (ε / R / сцены / тройки / K-hop граф) + ВЕБ-ПОИСК
//! для AI: краулер (robots/sitemap/SimHash) → SQLite-индекс (BM25 +
//! PageRank + WebRank) → `--web-search`.
//!
//! Поток (веб-версия потокового конвейера движка):
//!
//! ```text
//! URL ──► Chromium (CDP): JS исполнен, DOM отрендерен
//!          ├── innerText (то, что видит человек: SPA/Shadow DOM решены)
//!          └── перехваченные JSON скрытых API (до превращения в HTML)
//!                 ▼
//!        Временный .md/.txt в веб-кэше (url → файл, детерминированно)
//!                 ▼
//!        poler_engine::scan_path: ε, R, сцены, тройки, граф
//!                 ▼
//!        ContextAnchor JSON + Cross-Universe рёбра (web→локальные файлы)
//! ```
//!
//! Фильтрация шума: реклама/меню/футеры отсеиваются сами — у них низкая
//! ε-плотность относительно запроса (редкие осмысленные токены побеждают
//! частотный шаблонный мусор).

pub mod cdp;
pub mod crawl;
pub mod extract;
pub mod index;
pub mod phrase;
pub mod robots;
pub mod simhash;
pub mod stem;
pub mod urlnorm;
pub mod weblens;

use std::path::{Path, PathBuf};

pub use cdp::{CdpSession, WebPage};
pub use crawl::{CrawlConfig, CrawlStats, PageFetcher};
pub use index::{WebHit, WebIndex};

/// Ленивая обёртка: CdpFetcher с автозапуском Chromium при необходимости.
/// Страница без явного таймаута (совместимость со старыми вызовами).
pub fn cdp_fetcher(cdp_port: u16, wait_ms: u64) -> Result<CdpFetcher, String> {
    cdp_fetcher_with_timeout(cdp_port, wait_ms, 0)
}

/// CdpFetcher с пер-страничным таймаутом (мс; 0 = без лимита).
/// Автозапуск + самовосстановление после полумёртвого CDP-браузера.
pub fn cdp_fetcher_with_timeout(
    cdp_port: u16,
    wait_ms: u64,
    page_timeout_ms: u64,
) -> Result<CdpFetcher, String> {
    ensure_chromium(cdp_port)?;
    match CdpFetcher::new(cdp_port, wait_ms, page_timeout_ms) {
        Ok(f) => Ok(f),
        // Браузер поднялся, но WS не handshake-ится (осиротевший после kill
        // родителя) — перезапускаем браузер и пробуем ещё раз.
        Err(first) => {
            eprintln!(
                "poler-cdp: сессия не открылась ({first}) — перезапускаю Chromium на порту {cdp_port}"
            );
            restart_chromium(cdp_port)?;
            ensure_chromium(cdp_port)?;
            CdpFetcher::new(cdp_port, wait_ms, page_timeout_ms)
                .map_err(|second| format!("{first}; после перезапуска: {second}"))
        }
    }
}

/// Реальный PageFetcher поверх Chromium CDP (одна живая сессия).
pub struct CdpFetcher {
    session: CdpSession,
    wait_ms: u64,
    /// Пер-страничный лимит на fetch (мс; 0 = без лимита).
    page_timeout_ms: u64,
}

impl CdpFetcher {
    pub fn new(cdp_port: u16, wait_ms: u64, page_timeout_ms: u64) -> Result<Self, String> {
        Ok(Self {
            session: CdpSession::connect(cdp_port)?,
            wait_ms,
            page_timeout_ms,
        })
    }
}

impl PageFetcher for CdpFetcher {
    fn fetch(&mut self, url: &str) -> Result<crawl::FetchedPage, String> {
        self.session
            .set_page_deadline(page_deadline(self.page_timeout_ms));
        let r = self.session.load_page_full(url, self.wait_ms);
        self.session.set_page_deadline(None);
        let p = r?;
        Ok(crawl::FetchedPage {
            final_url: p.final_url,
            title: p.title,
            meta_description: p.meta_description,
            lang: p.lang,
            text: p.text,
            links: p.links,
        })
    }

    fn fetch_raw(&mut self, url: &str) -> Result<(u16, String), String> {
        // robots/sitemap — маленькие ресурсы: таймаут вполовину страничного
        self.session
            .set_page_deadline(page_deadline(self.page_timeout_ms / 2));
        let r = self.session.fetch_raw(url);
        self.session.set_page_deadline(None);
        r
    }
}

/// Дедлайн пер-страничного лимита (None при timeout == 0).
fn page_deadline(timeout_ms: u64) -> Option<std::time::Instant> {
    (timeout_ms > 0)
        .then(|| std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms))
}

/// Путь к базе веб-индекса по умолчанию:
/// `$POLER_WEB_DB` → иначе `~/.local/share/poler-engine/web-index.db`.
pub fn default_db_path() -> PathBuf {
    if let Ok(p) = std::env::var("POLER_WEB_DB") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local/share/poler-engine/web-index.db")
}

/// Директория веб-кэша (текст страниц + перехваченный JSON).
pub fn web_cache_dir() -> PathBuf {
    let d = std::env::temp_dir().join("poler-web");
    let _ = std::fs::create_dir_all(&d);
    d
}

// ---------------------------------------------------------------------------
// Автозапуск Chromium: AI-агент не должен думать о браузере — движок сам
// поднимает headless-инстанс, если CDP-порт молчит.
// ---------------------------------------------------------------------------

/// Жив ли CDP на порту (tcp-коннект достаточно: DevTools слушает только его).
pub fn cdp_alive(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

/// Глубокая проверка: TCP + HTTP GET /json/version отдаёт валидный JSON.
/// Ловит «полумёртвые» браузеры (порт слушает, DevTools не отвечает).
pub fn cdp_healthy(port: u16) -> bool {
    let Ok(list) = cdp::http_get("127.0.0.1", port, "/json/version") else {
        return false;
    };
    serde_json::from_str::<serde_json::Value>(&list).is_ok()
}

/// Кандидаты браузера из кеша playwright (~/.cache/ms-playwright):
/// полный chromium И chrome-headless-shell, свежайшая версия побеждает.
/// Чистая функция от корня кеша — тестируется без env-мутаций.
pub fn playwright_candidates(cache_root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<(u64, bool, PathBuf)> = Vec::new(); // (версия, headless-shell?, путь)
    let entries = match std::fs::read_dir(cache_root) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    for ent in entries.flatten() {
        let dir = ent.path();
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        // chromium-1234 | chromium_headless_shell-1234
        let is_browser_dir = name.starts_with("chromium") && name.contains('-');
        if !is_browser_dir {
            continue;
        }
        let version: u64 = name
            .rsplit('-')
            .next()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let is_shell = name.contains("headless_shell");
        // полные сборки: chromium-*/chrome-linux*/chrome
        // headless-shell: chromium_headless_shell-*/chrome-headless-shell-*/chrome-headless-shell
        let bins: Vec<PathBuf> = if is_shell {
            // каждая дочерняя директория (chrome-headless-shell-linux64,
            // chrome-headless-shell-linux, mac-arm…) содержит бинарь
            std::fs::read_dir(&dir)
                .map(|rd| {
                    rd.flatten()
                        .map(|e| e.path().join("chrome-headless-shell"))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        } else {
            vec!["chrome-linux", "chrome-linux64"]
                .into_iter()
                .map(|d| dir.join(d).join("chrome"))
                .collect()
        };
        for b in bins {
            if b.is_file() {
                out.push((version, is_shell, b));
            }
        }
    }
    // свежая версия — первая; при равенстве headless-shell легче — первым
    out.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    out.into_iter().map(|(_, _, p)| p).collect()
}

/// Кеш playwright по платформе (linux: ~/.cache, mac: ~/Library/Caches).
fn playwright_cache_root() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let linux = PathBuf::from(&home).join(".cache/ms-playwright");
    if linux.is_dir() {
        return Some(linux);
    }
    let mac = PathBuf::from(home).join("Library/Caches/ms-playwright");
    (mac.is_dir()).then_some(mac)
}

/// Поиск бинаря браузера: $POLER_CHROME_BIN → PATH → кеш playwright
/// (свежая версия) → известные абсолютные пути.
pub fn find_browser() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("POLER_CHROME_BIN") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    for name in [
        "chrome-headless-shell",
        "chromium",
        "chromium-browser",
        "google-chrome",
        "google-chrome-stable",
        "chrome",
    ] {
        if let Some(path) = std::env::var("PATH")
            .ok()?
            .split(':')
            .map(|dir| Path::new(dir).join(name))
            .find(|p| p.is_file())
        {
            return Some(path);
        }
    }
    // кеш playwright: свежайшая сборка (headless-shell или полный chromium)
    if let Some(root) = playwright_cache_root() {
        if let Some(p) = playwright_candidates(&root).into_iter().next() {
            return Some(p);
        }
    }
    // известные точки размещения (в т.ч. bundle движка)
    [
        "/home/z/my-project/browser/chrome-headless-shell-linux64/chrome-headless-shell",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/usr/bin/google-chrome",
        "/snap/bin/chromium",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
}

/// Убить полумёртвый Chromium на порту (осиротевший после kill родителя).
/// pkill по точному флагу порта: чужие браузеры не трогаем.
/// `--` обязателен: паттерн начинается с дефисов, иначе pkill примет его
/// за собственный флаг и напечатает help вместо убийства.
fn kill_stale_chromium(port: u16) {
    let marker = format!("--remote-debugging-port={port}");
    let _ = std::process::Command::new("pkill")
        .arg("-f")
        .arg("--")
        .arg(&marker)
        .status();
    // порт освобождается не мгновенно — до 5 с
    for _ in 0..20 {
        if !cdp_alive(port) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

/// Перезапуск CDP-браузера на порту: kill осиротевшего → чистый подъём.
pub fn restart_chromium(port: u16) -> Result<(), String> {
    kill_stale_chromium(port);
    spawn_chromium(port)
}

/// Подъём headless-браузера на порту (без проверок — низкоуровневая часть).
fn spawn_chromium(port: u16) -> Result<(), String> {
    let bin = find_browser().ok_or_else(|| {
        "Chromium не найден. Установите POLER_CHROME_BIN=/путь/к/chrome-headless-shell \
         или положите бинарь в PATH"
            .to_string()
    })?;
    let log = std::env::temp_dir().join("poler-chromium.log");
    let log_f = std::fs::File::options()
        .create(true)
        .append(true)
        .open(&log)
        .map_err(|e| format!("лог {log:?}: {e}"))?;
    let mut child = std::process::Command::new(&bin)
        .args([
            "--headless",
            "--no-sandbox",
            "--disable-gpu",
            "--disable-dev-shm-usage",
            "--no-first-run",
            "--disable-blink-features=AutomationControlled",
            &format!("--remote-debugging-port={port}"),
            "about:blank",
        ])
        .stdout(log_f.try_clone().map_err(|e| e.to_string())?)
        .stderr(log_f)
        .stdin(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("запуск {:?}: {e}", bin))?;
    // готовность: до ~15 с (холодный старт на слабых машинах)
    for _ in 0..60 {
        if cdp_healthy(port) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    let _ = child.kill();
    Err(format!(
        "Chromium не поднялся на порт {port} за 15 с (лог: {log:?})"
    ))
}

/// Гарантирует живой Chromium с CDP на `port`: молча возвращается, если уже
/// здоров (проверка /json/version, не только TCP); иначе находит бинарь,
/// поднимает headless и ждёт готовности ~15 с.
/// Дочерний процесс живёт, пока жив родитель (CLI-ран или MCP-сервер).
pub fn ensure_chromium(port: u16) -> Result<(), String> {
    if cdp_healthy(port) {
        return Ok(());
    }
    // порт может держать полумёртвый осиротевший браузер — освобождаем
    if cdp_alive(port) {
        kill_stale_chromium(port);
    }
    spawn_chromium(port)
}

/// Детерминированное имя файла для URL (fnv-подобный хеш).
pub fn url_slug(url: &str) -> String {
    // простой стабильный хеш (FNV-1a 64)
    let mut h: u64 = 0xcbf29ce484222325;
    for b in url.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    // + человекочитаемый хвост
    let tail: String = url
        .chars()
        .rev()
        .take(24)
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '.')
        .collect();
    format!("{h:016x}_{tail}")
}

/// Результат ingestion-сессии.
pub struct IngestResult {
    /// Файл с рендер-текстом страницы (в веб-кэше).
    pub text_file: PathBuf,
    /// Файлы перехваченных JSON API.
    pub json_files: Vec<PathBuf>,
    /// Заголовок страницы (из рендер-текста, первая непустая строка).
    pub title: String,
    /// Число символов рендер-текста.
    pub text_len: usize,
}

/// Загрузка URL через CDP и материализация в веб-кэш.
///
/// * `cdp_port` — порт Chromium с --remote-debugging-port;
/// * `wait_ms` — пауза на дочерние fetch/XHR после load.
pub fn ingest_url(url: &str, cdp_port: u16, wait_ms: u64) -> Result<IngestResult, String> {
    let mut session = CdpSession::connect(cdp_port)?;
    let page = session.load_page(url, wait_ms)?;

    if page.text.trim().is_empty() {
        return Err(format!(
            "пустой рендер-текст (страница не загрузилась или JS-ошибка): {url}"
        ));
    }

    let dir = web_cache_dir();
    let slug = url_slug(url);

    // рендер-текст: сохраняем с заголовком-URL сверху (сцена/глава)
    let title = page
        .text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(url)
        .trim()
        .chars()
        .take(120)
        .collect::<String>();
    let md = format!(
        "# {title}\n\n> source: {url}\n\n{}\n",
        page.text.trim()
    );
    let text_file = dir.join(format!("{slug}.md"));
    std::fs::write(&text_file, md).map_err(|e| format!("запись кэша: {e}"))?;

    // перехваченные JSON: каждый как отдельный файл кэша
    let mut json_files = Vec::new();
    for (i, (jurl, body)) in page.json_responses.iter().enumerate() {
        let jslug = format!("{}_api{}.json", url_slug(jurl), i);
        let f = dir.join(jslug);
        // pretty-print JSON, если парсится
        let pretty = serde_json::from_str::<serde_json::Value>(body)
            .and_then(|v| serde_json::to_string_pretty(&v))
            .unwrap_or_else(|_| body.clone());
        let _ = std::fs::write(&f, pretty);
        json_files.push(f);
    }

    Ok(IngestResult {
        text_file,
        json_files,
        title,
        text_len: page.text.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_slug_stable_and_readable() {
        let a = url_slug("https://example.com/docs/mmap");
        let b = url_slug("https://example.com/docs/mmap");
        assert_eq!(a, b);
        assert!(a.ends_with("docs.mmap") || a.len() > 16);
        assert!(url_slug("https://a.com") != url_slug("https://b.com"));
    }

    #[test]
    fn cache_dir_creates() {
        let d = web_cache_dir();
        assert!(d.exists());
    }

    #[test]
    fn default_db_path_env_override() {
        // не мутируем env параллельных тестов — только проверка формы
        let p = default_db_path();
        assert!(p.to_string_lossy().contains("poler-engine"));
    }

    // ---- playwright-кеш: автодетект браузера (фикс №1 UX-аудита) ----

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"#!/bin/sh\n").unwrap();
    }

    #[test]
    fn playwright_candidates_prefers_fresh_version() {
        let root = std::env::temp_dir().join(format!("pw-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        touch(&root.join("chromium-1200/chrome-linux/chrome"));
        touch(&root.join("chromium-1234/chrome-linux64/chrome"));
        touch(&root.join("chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell"));
        let got = playwright_candidates(&root);
        // свежая версия (1234) впереди; при равенстве headless-shell первым
        assert_eq!(got.len(), 3);
        assert!(got[0].to_string_lossy().contains("headless-shell"));
        assert!(got[1].to_string_lossy().contains("chrome-linux64"));
        assert!(got[2].to_string_lossy().contains("chromium-1200"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn playwright_candidates_ignores_non_chromium_dirs() {
        let root = std::env::temp_dir().join(format!("pw-junk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        touch(&root.join("ffmpeg-1011/bin/ffmpeg"));
        touch(&root.join("chromium-999/chrome-linux64/chrome"));
        let got = playwright_candidates(&root);
        assert_eq!(got.len(), 1);
        assert!(got[0].to_string_lossy().contains("chromium-999"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn playwright_candidates_empty_root() {
        let root = std::env::temp_dir().join("pw-no-such-root-42");
        let _ = std::fs::remove_dir_all(&root);
        assert!(playwright_candidates(&root).is_empty());
    }

    // ---- cdp_healthy: /json/version вместо голого TCP (фикс №4) ----

    #[test]
    fn cdp_healthy_rejects_garbage_http() {
        // сервер, отвечающий мусором вместо JSON
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nNOTJS");
            }
        });
        assert!(!cdp_healthy(port));
    }

    #[test]
    fn cdp_healthy_accepts_devtools_json() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                let body = r#"{"Browser":"Chrome/152","ProtocolVersion":"1.3"}"#;
                let _ = s.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            }
        });
        assert!(cdp_healthy(port));
    }

    #[test]
    fn page_deadline_zero_is_none() {
        assert!(page_deadline(0).is_none());
        assert!(page_deadline(45000).is_some());
    }
}
