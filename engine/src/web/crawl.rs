//! Веб-краулер: URL Frontier + вежливость (Mercator/Heritrix) + robots + sitemap.
//!
//! Поток краулинга (как Googlebot, только на одной машине):
//!
//! ```text
//! seed URL ──► robots.txt хоста (кэш в SQLite, 401/403 → полный запрет)
//!              │
//!              ▼
//!         URL Frontier (VecDeque, per-host politeness:
//!              │            delay = max(--crawl-delay-ms, Crawl-delay))
//!              ▼
//!         CDP-рендер (JS/SPA) → title/meta/links/текст
//!              │
//!              ├── SimHash: near-дубликат (≤3 бита) → dup_of, без индексации
//!              ├── content_hash не изменился → skip (Percolator-lite)
//!              └── изменился → upsert: postings + links
//!                     │
//!                     ▼
//!               новые URL (depth+1, тот же host) → Frontier
//!                     │
//!                     ▼
//!               PageRank по links-таблице (20 итераций)
//! ```

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use super::index::{content_hash, WebDoc, WebIndex};
use super::robots::Robots;
use super::urlnorm::Url;

/// Источник страниц (реальный: CDP; тестовый: мок).
pub trait PageFetcher {
    /// Полная выгрузка страницы.
    fn fetch(&mut self, url: &str) -> Result<FetchedPage, String>;
    /// Сырой ресурс: (HTTP-статус, текст).
    fn fetch_raw(&mut self, url: &str) -> Result<(u16, String), String>;
}

/// Выгрузка страницы краулером.
#[derive(Clone)]
pub struct FetchedPage {
    pub final_url: String,
    pub title: String,
    pub meta_description: String,
    pub lang: String,
    pub text: String,
    pub links: Vec<String>,
}

/// Конфигурация обхода.
#[derive(Debug, Clone)]
pub struct CrawlConfig {
    /// Максимум страниц (полезная нагрузка).
    pub max_pages: usize,
    /// Максимальная глубина от seed (seed = 0).
    pub max_depth: usize,
    /// Минимальная пауза между запросами к одному хосту, мс.
    pub delay_ms: u64,
    /// Переходить на другие хосты (по умолчанию — нет).
    pub cross_site: bool,
    /// Пауза после load на XHR, мс.
    pub wait_ms: u64,
    /// Пер-страничный таймаут загрузки, мс (0 = без лимита).
    /// Передаётся в CdpFetcher через cdp_fetcher_with_timeout.
    pub page_timeout_ms: u64,
    /// Соблюдать robots.txt (по умолчанию — да).
    /// false = явное распоряжение пользователя индексировать конкретную
    /// страницу (--browser-index): ручное сохранение, не автоматический обход.
    pub respect_robots: bool,
}

impl Default for CrawlConfig {
    fn default() -> Self {
        Self {
            max_pages: 25,
            max_depth: 2,
            delay_ms: 1000,
            cross_site: false,
            wait_ms: 800,
            page_timeout_ms: 45_000,
            respect_robots: true,
        }
    }
}

/// Статистика обхода.
#[derive(Debug, Default, serde::Serialize)]
pub struct CrawlStats {
    pub fetched: usize,
    pub indexed: usize,
    pub unchanged: usize,
    pub skipped_robots: usize,
    pub duplicates: usize,
    pub errors: usize,
    pub sitemap_urls: usize,
    pub frontier_left: usize,
    pub elapsed_ms: u64,
    /// Человекочитаемые события обхода (robots-запреты с хостами,
    /// таймауты страниц и т.п.) — не только счётчики: пользователь
    /// должен видеть ПОЧЕМУ страница не попала в индекс.
    #[serde(default)]
    pub notes: Vec<String>,
}

/// Сколько robots-сообщений печатать в stderr по ходу обхода
/// (остальные — только в stats.notes, чтобы sitemap-простыни не спамили).
const ROBOTS_PRINT_CAP: usize = 10;

/// Расширения, которые не рендерятся в текст (медиа/архивы).
const SKIP_EXT: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "bmp", "mp3", "mp4", "avi", "mov",
    "webm", "wav", "ogg", "pdf", "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "exe", "dmg",
    "iso", "deb", "rpm", "apk", "css", "woff", "woff2", "ttf", "eot",
];

/// Состояние robots-кэшей на время обхода (память + SQLite через параметр).
struct RobotsCache {
    mem: HashMap<String, Robots>,
    last_hit: HashMap<String, Instant>,
    delay_ms: u64,
}

impl RobotsCache {
    /// robots.txt хоста: память → SQLite → fetch (с учётом статуса, RFC 9309).
    fn get(
        &mut self,
        index: &mut WebIndex,
        fetcher: &mut dyn PageFetcher,
        url: &Url,
    ) -> Result<Robots, String> {
        let host = url.host_key().to_string();
        if let Some(r) = self.mem.get(&host) {
            return Ok(r.clone());
        }
        // из SQLite (между запусками)
        if let Some(body) = index.robots_for(&host) {
            let r = Robots::parse(&body);
            self.mem.insert(host, r.clone());
            return Ok(r);
        }
        let robots_url = format!("{}://{}/robots.txt", url.scheme, url.host);
        self.polite_wait(&host);
        let (status, body) = fetcher.fetch_raw(&robots_url)?;
        // RFC 9309: 401/403 → полный запрет; 404/410 → всё разрешено.
        // В кэш SQLite кладём СИНТЕТИЧЕСКОЕ тело, воспроизводящее статус.
        let (robots, cache_body) = match status {
            401 | 403 => (Robots::disallow_all(), "User-agent: *\nDisallow: /\n".to_string()),
            404 | 410 => (Robots::default(), String::new()),
            _ => (Robots::parse(&body), body),
        };
        let _ = index.save_robots(&host, &cache_body);
        self.mem.insert(host, robots.clone());
        Ok(robots)
    }

    /// Per-host пауза перед запросом к хосту.
    fn polite_wait(&mut self, host: &str) {
        if let Some(t) = self.last_hit.get(host) {
            let wait = *t + Duration::from_millis(self.delay_ms);
            let now = Instant::now();
            if wait > now {
                std::thread::sleep(wait - now);
            }
        }
        self.last_hit.insert(host.to_string(), Instant::now());
    }
}

/// Полный обход: seed → frontier → индекс → PageRank.
pub fn crawl(
    index: &mut WebIndex,
    fetcher: &mut dyn PageFetcher,
    seed: &str,
    cfg: &CrawlConfig,
    verbose: bool,
) -> Result<CrawlStats, String> {
    let started = Instant::now();
    let seed_url = Url::parse(seed).ok_or_else(|| format!("некорректный seed URL: {seed}"))?;

    let mut stats = CrawlStats::default();
    let mut frontier: VecDeque<(Url, usize)> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut robots_cache = RobotsCache {
        mem: HashMap::new(),
        last_hit: HashMap::new(),
        delay_ms: cfg.delay_ms,
    };

    // sitemap-дискавери с seed-хоста (до основного цикла);
    // при max_depth = 0 режим «одна страница» — sitemap не нужен
    let sitemap_urls = if cfg.max_depth > 0 {
        discover_sitemaps(&mut robots_cache, index, fetcher, &seed_url, verbose)
    } else {
        Vec::new()
    };
    stats.sitemap_urls = sitemap_urls.len();
    frontier.push_back((seed_url, 0));
    for u in sitemap_urls.into_iter().take(200) {
        frontier.push_back((u, 0));
    }

    while !frontier.is_empty() && stats.fetched < cfg.max_pages {
        let (url, depth) = frontier.pop_front().expect("frontier не пуст");
        let key = url.as_str();
        if visited.contains(&key) {
            continue;
        }
        visited.insert(key.clone());

        let robots = if cfg.respect_robots {
            match robots_cache.get(index, fetcher, &url) {
                Ok(r) => Some(r),
                Err(e) => {
                    stats.errors += 1;
                    stats
                        .notes
                        .push(format!("robots.txt недоступен {key}: {e} — страница пропущена"));
                    eprintln!("poler-crawl: robots {key}: {e}");
                    continue;
                }
            }
        } else {
            None
        };
        if let Some(robots) = &robots {
            if !robots.allowed(&url.robots_path()) {
                stats.skipped_robots += 1;
                let note = format!(
                    "robots.txt хоста {} запрещает {} — страница пропущена (RFC 9309)",
                    url.host_key(),
                    url.robots_path()
                );
                stats.notes.push(note.clone());
                // всегда сообщаем (не только verbose): молчаливый
                // skipped_robots — боль UX-аудита №3
                if stats.skipped_robots <= ROBOTS_PRINT_CAP {
                    eprintln!("poler-crawl: {note}");
                } else if stats.skipped_robots == ROBOTS_PRINT_CAP + 1 {
                    eprintln!("poler-crawl: … дальнейшие robots-запреты — в stats.notes");
                }
                continue;
            }
        } else if !cfg.respect_robots {
            // явное распоряжение пользователя: индексируем вопреки robots,
            // но честно фиксируем это в notes
            if let Ok(r) = robots_cache.get(index, fetcher, &url) {
                if !r.allowed(&url.robots_path()) {
                    stats.notes.push(format!(
                        "{} запрещает robots.txt — индексировано по явной команде пользователя",
                        key
                    ));
                }
            }
        }

        // per-host politeness: delay = max(настройка, Crawl-delay)
        let host = url.host_key().to_string();
        let delay_ms = robots
            .as_ref()
            .map(|r| cfg.delay_ms.max((r.crawl_delay_s * 1000.0) as u64))
            .unwrap_or(cfg.delay_ms);
        if let Some(t) = robots_cache.last_hit.get(&host).copied() {
            let wait = t + Duration::from_millis(delay_ms);
            let now = Instant::now();
            if wait > now {
                std::thread::sleep(wait - now);
            }
        }
        robots_cache.last_hit.insert(host, Instant::now());

        // рендер
        let page = match fetcher.fetch(&key) {
            Ok(p) => p,
            Err(e) => {
                stats.errors += 1;
                let is_timeout = e.contains("таймаут");
                stats.notes.push(format!(
                    "страница не загружена {key}: {e}{}",
                    if is_timeout {
                        " (пер-страничный лимит; увеличьте --crawl-page-timeout-ms, \
                         если сайт медленный)"
                    } else {
                        ""
                    }
                ));
                eprintln!("poler-crawl: fetch {key}: {e}");
                continue;
            }
        };
        stats.fetched += 1;

        // финальный URL (редиректы) — канонический
        let final_url = Url::parse(&page.final_url).unwrap_or_else(|| url.clone());
        let fkey = final_url.as_str();
        if fkey != key && visited.contains(&fkey) {
            continue; // редирект на уже посещённое
        }
        visited.insert(fkey.to_string());

        // текст → токены → дедуп SimHash. Токены — стемминг-путь: тот же,
        // что в upsert_page (иначе отпечатки в БД и сравниваемые разъедутся),
        // и падежный шум уходит из отпечатка — near-дубли ловятся надёжнее.
        let text = super::extract::clean_text(&page.text, 256 * 1024);
        let tokens = super::stem::tokenize_stem(&text);
        let lang = if page.lang.is_empty() {
            super::extract::detect_lang(&text).to_string()
        } else {
            page.lang.clone()
        };
        let links = normalize_links(&final_url, &page.links, cfg);

        let doc = WebDoc {
            url: fkey.to_string(),
            title: page.title.clone(),
            lang,
            meta_description: page.meta_description.clone(),
            text: text.clone(),
            links: links.clone(),
            content_hash: content_hash(&text),
        };

        // near-дубликат? → запись dup_of без участия в поиске
        if let Some(canonical) = index.find_duplicate(&fkey, &tokens) {
            if index.upsert_page(&doc).is_ok() {
                let _ = index.record_duplicate(&fkey, &canonical);
            }
            stats.duplicates += 1;
            if verbose {
                eprintln!("poler-crawl: дубликат {fkey} ≈ {canonical}");
            }
            continue;
        }

        match index.upsert_page(&doc) {
            Ok((_, reindexed)) => {
                if reindexed {
                    stats.indexed += 1;
                } else {
                    stats.unchanged += 1; // Percolator-lite
                }
                if verbose {
                    eprintln!(
                        "poler-crawl: [{}/{}] {fkey} — {} токенов, {} ссылок{}",
                        stats.fetched,
                        cfg.max_pages,
                        tokens.len(),
                        links.len(),
                        if reindexed { "" } else { " (без изменений)" }
                    );
                }
            }
            Err(e) => {
                stats.errors += 1;
                if verbose {
                    eprintln!("poler-crawl: индексация {fkey}: {e}");
                }
            }
        }

        // новые URL в frontier
        if depth < cfg.max_depth {
            for l in &links {
                if !visited.contains(l.as_str())
                    && !frontier.iter().any(|(u, _)| u.as_str() == l.as_str())
                {
                    if let Some(lu) = Url::parse(l) {
                        frontier.push_back((lu, depth + 1));
                    }
                }
            }
        }
    }

    // PageRank по накопленному графу ссылок
    if let Err(e) = index.recompute_pagerank(20) {
        if verbose {
            eprintln!("poler-crawl: pagerank: {e}");
        }
    }

    stats.frontier_left = frontier.len();
    stats.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(stats)
}

/// Sitemap-дискавери: robots.txt → теги `<loc>` → нормализованные URL своего хоста.
fn discover_sitemaps(
    cache: &mut RobotsCache,
    index: &mut WebIndex,
    fetcher: &mut dyn PageFetcher,
    seed: &Url,
    verbose: bool,
) -> Vec<Url> {
    let robots = match cache.get(index, fetcher, seed) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let host_key = seed.host_key().to_string();
    let mut out = Vec::new();
    for sm in robots.sitemaps.iter().take(4) {
        match fetcher.fetch_raw(sm) {
            Ok((_st, xml)) => {
                if xml.len() < 16 * 1024 * 1024 {
                    for loc in extract_locs(&xml).into_iter().take(200) {
                        if let Some(u) = Url::parse(&loc) {
                            // только наш хост (иначе sitemap утянет на чужие сайты)
                            if u.host_key() == host_key {
                                out.push(u);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                if verbose {
                    eprintln!("poler-crawl: sitemap {sm}: {e}");
                }
            }
        }
        cache.polite_wait(&host_key);
    }
    out
}

/// `<loc>…</loc>` без regex-зависимости (простой скан).
fn extract_locs(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(start) = xml[i..].find("<loc>") {
        let s = i + start + 5;
        let Some(endrel) = xml[s..].find("</loc>") else { break };
        let e = s + endrel;
        let url = xml[s..e].trim().trim_start_matches("<![CDATA[").trim_end_matches("]]>");
        if url.starts_with("http") && url.len() < 512 {
            out.push(url.to_string());
        }
        i = e + 6;
    }
    out
}

/// Нормализация и фильтрация ссылок страницы.
fn normalize_links(base: &Url, raw: &[String], cfg: &CrawlConfig) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for href in raw {
        if href.len() > 512 {
            continue;
        }
        let Some(abs) = base.join(href) else { continue };
        let key = abs.as_str();
        if !seen.insert(key.to_string()) {
            continue;
        }
        // тот же хост (или cross_site)
        if !cfg.cross_site && abs.host_key() != base.host_key() {
            continue;
        }
        // медиа/архивы — пропускаем
        let ext = abs
            .path
            .rsplit('.')
            .next()
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if SKIP_EXT.contains(&ext.as_str()) {
            continue;
        }
        out.push(key.to_string());
    }
    out.truncate(400);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Мок-фитчер: HTTP-мир в HashMap.
    struct MockFetcher {
        pages: HashMap<String, FetchedPage>,
        raw: HashMap<String, (u16, String)>,
        log: RefCell<Vec<String>>,
    }

    impl PageFetcher for MockFetcher {
        fn fetch(&mut self, url: &str) -> Result<FetchedPage, String> {
            self.log.borrow_mut().push(format!("GET {url}"));
            self.pages.get(url).cloned().ok_or_else(|| format!("404 {url}"))
        }
        fn fetch_raw(&mut self, url: &str) -> Result<(u16, String), String> {
            self.log.borrow_mut().push(format!("RAW {url}"));
            self.raw
                .get(url)
                .cloned()
                .ok_or_else(|| format!("404 {url}"))
        }
    }

    fn page(url: &str, title: &str, text: &str, links: &[&str]) -> FetchedPage {
        FetchedPage {
            final_url: url.to_string(),
            title: title.to_string(),
            meta_description: String::new(),
            lang: String::new(),
            text: text.to_string(),
            links: links.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn mock() -> MockFetcher {
        let mut pages = HashMap::new();
        let mut raw = HashMap::new();
        raw.insert(
            "https://site.com/robots.txt".to_string(),
            (
                200,
                "User-agent: *\nDisallow: /private\nCrawl-delay: 0\nSitemap: https://site.com/sitemap.xml"
                    .to_string(),
            ),
        );
        raw.insert(
            "https://site.com/sitemap.xml".to_string(),
            (
                200,
                "<?xml version=\"1.0\"?><urlset><url><loc>https://site.com/from-sitemap</loc></url></urlset>"
                    .to_string(),
            ),
        );
        pages.insert(
            "https://site.com/".to_string(),
            page(
                "https://site.com/",
                "Home",
                "главная страница сайта о поисковых движках и ранжировании",
                &["https://site.com/docs", "https://site.com/private/x", "https://other.com/away"],
            ),
        );
        pages.insert(
            "https://site.com/docs".to_string(),
            page(
                "https://site.com/docs",
                "Docs",
                "документация поискового движка: индексация краулер ранжирование пейджранк",
                &["https://site.com/", "https://site.com/docs/deep"],
            ),
        );
        pages.insert(
            "https://site.com/private/x".to_string(),
            page(
                "https://site.com/private/x",
                "Private",
                "закрытая robots страница для теста явной команды пользователя",
                &[],
            ),
        );
        pages.insert(
            "https://site.com/from-sitemap".to_string(),
            page(
                "https://site.com/from-sitemap",
                "Sitemap page",
                "страница найденная через sitemap xml протокол",
                &[],
            ),
        );
        MockFetcher {
            pages,
            raw,
            log: RefCell::new(Vec::new()),
        }
    }

    fn cfg() -> CrawlConfig {
        CrawlConfig {
            max_pages: 10,
            max_depth: 2,
            delay_ms: 0, // тесты без пауз
            cross_site: false,
            wait_ms: 0,
            page_timeout_ms: 0,
            respect_robots: true,
        }
    }

    #[test]
    fn crawl_indexes_and_respects_robots() {
        let mut ix = WebIndex::open_memory().unwrap();
        let mut f = mock();
        let stats = crawl(&mut ix, &mut f, "https://site.com/", &cfg(), false).unwrap();
        assert!(stats.indexed >= 2, "indexed={}", stats.indexed);
        // robots запретил /private/*
        assert!(stats.skipped_robots >= 1);
        // sitemap-страница проиндексирована
        let hits = ix.search("sitemap", 10).unwrap();
        assert!(hits.iter().any(|h| h.url == "https://site.com/from-sitemap"));
        // внешний хост не пошёл
        assert!(!f.log.borrow().iter().any(|l| l.contains("other.com/away")));
    }

    // ---- фикс №3 UX-аудита: robots-запрет объясняется, а не молчит ----

    #[test]
    fn crawl_robots_skip_is_explained_in_notes() {
        let mut ix = WebIndex::open_memory().unwrap();
        let mut f = mock();
        let stats = crawl(&mut ix, &mut f, "https://site.com/", &cfg(), false).unwrap();
        // не только счётчик, но и человекочитаемое ПОЧЕМУ с хостом и путём
        assert!(stats.notes.iter().any(|n| {
            n.contains("robots.txt")
                && n.contains("site.com")
                && n.contains("/private")
        }));
    }

    // ---- --browser-index: явная команда пользователя обходит robots ----

    #[test]
    fn crawl_respect_robots_false_indexes_disallowed_page() {
        let mut ix = WebIndex::open_memory().unwrap();
        let mut f = mock();
        let mut c = cfg();
        c.max_depth = 0; // одиночная страница — режим --browser-index
        c.max_pages = 1;
        c.respect_robots = false;
        let stats = crawl(&mut ix, &mut f, "https://site.com/private/x", &c, false).unwrap();
        assert_eq!(stats.skipped_robots, 0, "явная команда не считается запретом");
        assert_eq!(stats.indexed, 1, "страница должна попасть в индекс");
        // честность: notes фиксируют, что robots был нарушен по команде пользователя
        assert!(stats.notes.iter().any(|n| n.contains("явной команде")));
        // переиндексация той же страницы — без изменений (Percolator-lite)
        let stats2 = crawl(&mut ix, &mut f, "https://site.com/private/x", &c, false).unwrap();
        assert_eq!(stats2.unchanged, 1);
    }

    #[test]
    fn crawl_fetch_error_lands_in_notes() {
        let mut ix = WebIndex::open_memory().unwrap();
        let mut f = mock();
        let mut c = cfg();
        c.max_depth = 0;
        // страницы нет в моке → 404-ошибка попадает в notes
        let stats = crawl(&mut ix, &mut f, "https://site.com/nope", &c, false).unwrap();
        assert_eq!(stats.errors, 1);
        assert!(stats.notes.iter().any(|n| n.contains("nope")));
    }

    #[test]
    fn crawl_depth_limit() {
        let mut ix = WebIndex::open_memory().unwrap();
        let mut f = mock();
        let mut c = cfg();
        c.max_depth = 0; // только seed
        crawl(&mut ix, &mut f, "https://site.com/", &c, false).unwrap();
        assert_eq!(ix.page_count(), 1);
    }

    #[test]
    fn crawl_max_pages() {
        let mut ix = WebIndex::open_memory().unwrap();
        let mut f = mock();
        let mut c = cfg();
        c.max_pages = 1;
        let stats = crawl(&mut ix, &mut f, "https://site.com/", &c, false).unwrap();
        assert_eq!(stats.fetched, 1);
    }

    #[test]
    fn pagerank_computed_after_crawl() {
        let mut ix = WebIndex::open_memory().unwrap();
        let mut f = mock();
        crawl(&mut ix, &mut f, "https://site.com/", &cfg(), false).unwrap();
        // home ← docs (docs ссылается на home)
        let home_rank = ix.rank_of("https://site.com/");
        assert!(home_rank > 0.0);
    }

    #[test]
    fn duplicate_not_reindexed() {
        let mut ix = WebIndex::open_memory().unwrap();
        let mut f = mock();
        let body = "уникальный длинный текст статьи про поисковые движки ".repeat(4);
        f.pages.insert(
            "https://site.com/".to_string(),
            page("https://site.com/", "Home", &body, &["https://site.com/dup"]),
        );
        f.pages.insert(
            "https://site.com/dup".to_string(),
            page("https://site.com/dup", "Dup", &body, &[]),
        );
        f.raw.remove("https://site.com/sitemap.xml"); // без sitemap
        let stats = crawl(&mut ix, &mut f, "https://site.com/", &cfg(), false).unwrap();
        assert!(stats.duplicates >= 1, "dups={}", stats.duplicates);
        // поиск не возвращает дубликат
        let hits = ix.search("уникальный", 10).unwrap();
        assert!(hits.iter().all(|h| h.url != "https://site.com/dup"));
    }

    #[test]
    fn redirects_to_visited_skipped() {
        let mut ix = WebIndex::open_memory().unwrap();
        let mut f = mock();
        // /alias редиректит на /
        f.pages.insert(
            "https://site.com/alias".to_string(),
            page("https://site.com/", "Home", "alias redirect target", &[]),
        );
        f.pages.insert(
            "https://site.com/".to_string(),
            page("https://site.com/", "Home", "home page content", &["https://site.com/alias"]),
        );
        f.raw.remove("https://site.com/sitemap.xml");
        let stats = crawl(&mut ix, &mut f, "https://site.com/", &cfg(), false).unwrap();
        assert_eq!(ix.page_count(), 1);
        assert!(stats.fetched >= 1);
    }

    #[test]
    fn locs_extraction() {
        let xml = "<urlset><url><loc>https://a/1</loc></url><url><loc>https://a/2</loc></url></urlset>";
        assert_eq!(extract_locs(xml).len(), 2);
        assert!(extract_locs("no locs here").is_empty());
    }

    #[test]
    fn normalize_links_filters() {
        let base = Url::parse("https://a.io/x/y").unwrap();
        let links = vec![
            "https://a.io/z".to_string(),
            "relative.html".to_string(),
            "/root".to_string(),
            "https://b.io/external".to_string(),
            "https://a.io/pic.png".to_string(),
            "https://a.io/z?utm_source=x".to_string(), // дубликат z после нормализации
            "javascript:void(0)".to_string(),
        ];
        let out = normalize_links(&base, &links, &CrawlConfig::default());
        assert!(out.contains(&"https://a.io/z".to_string()));
        assert!(out.contains(&"https://a.io/x/relative.html".to_string()));
        assert!(out.contains(&"https://a.io/root".to_string()));
        assert!(!out.iter().any(|u| u.contains("b.io")));
        assert!(!out.iter().any(|u| u.ends_with(".png")));
        assert!(!out.iter().any(|u| u.contains("javascript")));
        assert_eq!(out.iter().filter(|u| *u == "https://a.io/z").count(), 1);
    }

    #[test]
    fn robots_401_full_disallow() {
        let mut ix = WebIndex::open_memory().unwrap();
        let mut f = mock();
        f.raw.insert(
            "https://closed.io/robots.txt".to_string(),
            (401, "auth required".to_string()),
        );
        f.pages.insert(
            "https://closed.io/".to_string(),
            page("https://closed.io/", "Closed", "secret content", &[]),
        );
        let stats = crawl(&mut ix, &mut f, "https://closed.io/", &cfg(), false).unwrap();
        assert!(stats.skipped_robots >= 1);
        assert_eq!(ix.page_count(), 0);
    }

    #[test]
    fn recrawl_unchanged_is_cheap() {
        let mut ix = WebIndex::open_memory().unwrap();
        let mut f = mock();
        f.raw.remove("https://site.com/sitemap.xml");
        crawl(&mut ix, &mut f, "https://site.com/", &cfg(), false).unwrap();
        // второй проход: robots из кэша SQLite, контент не изменился
        let mut f2 = mock();
        f2.raw.remove("https://site.com/sitemap.xml");
        let stats = crawl(&mut ix, &mut f2, "https://site.com/", &cfg(), false).unwrap();
        assert!(stats.unchanged >= 1);
        assert_eq!(stats.indexed, 0);
    }
}
