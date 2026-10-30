//! RQ19: источник знаний — Wikipedia API (bot-friendly, anti-bot safe).
//!
//! Вместо скрейпинга HTML-страниц поиска (где живут анти-боты) —
//! **официальный API** `action=query` MediaWiki: JSON, без авторизации,
//! политикой разрешён для программных клиентов при честном User-Agent.
//! Извлечения приходят уже чистым текстом (`explaintext`) — ноль
//! HTML-парсинга.
//!
//! ## Вежливость
//!
//! * идентифицирующийся `User-Agent` с URL проекта (политика Wikimedia);
//! * пауза ≥ 300 мс между запросами;
//! * на `403/429` (лимит на общий IP песочницы/датацентра) — линейный
//!   backoff с повтором (2 попытки), затем честный отказ.
//!
//! ## Языки
//!
//! Кириллица в теме → `ru.wikipedia`, иначе `en.wikipedia`
//! ([`WikiSource::detect_language`]); явный выбор — параметром.

use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::json::Json;
use crate::netfetch::{percent_encode, HttpClient, NetError, Url};

/// Пауза между запросами (политика вежливости).
const POLITE_PAUSE: Duration = Duration::from_millis(300);
/// Пауза backoff при 403/429.
const BACKOFF: Duration = Duration::from_secs(2);
/// Максимум страниц в одном запросе extracts (лимит API для анонимов).
const EXLIMIT: usize = 20;

/// Источник знаний: Wikipedia API одного языкового раздела.
pub struct WikiSource {
    client: HttpClient,
    lang: String,
    last_request: Option<Instant>,
}

/// Результат поиска: заголовок + сниппет (сниппет чистится от разметки).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub title: String,
    pub snippet: String,
}

/// Извлечённая страница: заголовок + чистый текст.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageExtract {
    pub title: String,
    pub text: String,
}

impl WikiSource {
    /// Источник для языкового раздела (`ru`, `en`, …).
    pub fn new(lang: &str) -> WikiSource {
        WikiSource { client: HttpClient::default(), lang: lang.to_string(), last_request: None }
    }

    /// Автоопределение языка темы: кириллица → `ru`, иначе `en`.
    pub fn detect_language(topic: &str) -> &'static str {
        let cyr = topic.chars().any(|c| ('а'..='я').contains(&c) || ('А'..='Я').contains(&c));
        if cyr {
            "ru"
        } else {
            "en"
        }
    }

    /// Языковой раздел источника.
    pub fn lang(&self) -> &str {
        &self.lang
    }

    /// Поиск страниц по запросу (`list=search`).
    pub fn search(&mut self, query: &str, limit: usize) -> Result<Vec<SearchHit>, NetError> {
        let params = format!(
            "action=query&format=json&list=search&srsearch={}&srlimit={}",
            percent_encode(query),
            limit.min(50),
        );
        let json = self.fetch_json(&params)?;
        let empty = Vec::new();
        let hits = json
            .get("query")
            .and_then(|q| q.get("search"))
            .and_then(|s| s.as_arr())
            .unwrap_or(&empty);
        let mut out = Vec::with_capacity(hits.len());
        for h in hits {
            let title = h.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string();
            if title.is_empty() {
                continue;
            }
            // Сниппеты содержат <span class="searchmatch">…</span>.
            let snippet = crate::stream_engine::strip_html(
                h.get("snippet").and_then(|s| s.as_str()).unwrap_or("").as_bytes(),
            );
            out.push(SearchHit { title, snippet });
        }
        Ok(out)
    }

    /// Извлечение чистого текста страниц (`prop=extracts`, `explaintext`).
    ///
    /// `intro = true` — только вводные секции (плотный концентрат
    /// определений, ограниченный размер); `false` — полные статьи.
    /// Отсутствующие страницы отфильтровываются.
    pub fn extracts(&mut self, titles: &[String], intro: bool) -> Result<Vec<PageExtract>, NetError> {
        let mut out = Vec::with_capacity(titles.len());
        for batch in titles.chunks(EXLIMIT) {
            let joined = batch
                .iter()
                .map(|t| percent_encode(t.replace(' ', "_").as_str()))
                .collect::<Vec<_>>()
                .join("%7C");
            let intro_flag = if intro { "&exintro=1" } else { "" };
            let params = format!(
                "action=query&format=json&prop=extracts&explaintext=1&redirects=1&exlimit=max{intro_flag}&titles={joined}"
            );
            let json = self.fetch_json(&params)?;
            let empty = Vec::new();
            let pages = json
                .get("query")
                .and_then(|q| q.get("pages"))
                .unwrap_or(&Json::Null);
            let pairs = match pages {
                Json::Obj(pairs) => pairs,
                _ => &empty,
            };
            for (_, page) in pairs.iter() {
                let missing = page.get("missing").is_some();
                let title = page.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string();
                let text = page.get("extract").and_then(|t| t.as_str()).unwrap_or("").to_string();
                if !missing && !title.is_empty() && !text.trim().is_empty() {
                    out.push(PageExtract { title, text });
                }
            }
        }
        Ok(out)
    }

    /// URL вызова API.
    fn api_url(&self, params: &str) -> Url {
        // Язык и хост ограничены [a-z-] на сборке URL — безопасно.
        Url::parse(&format!(
            "https://{}.wikipedia.org/w/api.php?{params}",
            self.lang
        ))
        .expect("api url")
    }

    /// GET с вежливой паузой и backoff на rate-limit.
    fn fetch_json(&mut self, params: &str) -> Result<Json, NetError> {
        let url = self.api_url(params);
        let mut attempts = 0;
        loop {
            self.polite_wait();
            match self.client.get(&url) {
                Ok(resp) if resp.ok() => {
                    let text = String::from_utf8_lossy(&resp.body).into_owned();
                    return Json::parse(&text)
                        .map_err(|_| NetError::BadResponse("JSON API: некорректный ответ"));
                }
                Ok(resp) if (resp.status == 403 || resp.status == 429) && attempts < 2 => {
                    attempts += 1;
                    sleep(BACKOFF * attempts as u32);
                }
                Ok(resp) => return Err(NetError::Status(resp.status)),
                Err(NetError::Io(ref e))
                    if e.kind() == std::io::ErrorKind::WouldBlock && attempts < 2 =>
                {
                    attempts += 1;
                    sleep(BACKOFF);
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Пауза ≥ POLITE_PAUSE между реальными запросами.
    fn polite_wait(&mut self) {
        if let Some(t) = self.last_request {
            let elapsed = t.elapsed();
            if elapsed < POLITE_PAUSE {
                sleep(POLITE_PAUSE - elapsed);
            }
        }
        self.last_request = Some(Instant::now());
    }
}

/// Адаптер Wikipedia API под пайплайн [`crate::learn_net::learn`].
impl crate::learn_net::TextSource for WikiSource {
    fn search(&mut self, query: &str, limit: usize) -> Result<Vec<(String, String)>, String> {
        WikiSource::search(self, query, limit)
            .map(|hits| hits.into_iter().map(|h| (h.title, h.snippet)).collect())
            .map_err(|e| e.to_string())
    }

    fn extracts(&mut self, titles: &[String], intro: bool) -> Result<Vec<(String, String)>, String> {
        WikiSource::extracts(self, titles, intro)
            .map(|pages| pages.into_iter().map(|p| (p.title, p.text)).collect())
            .map_err(|e| e.to_string())
    }
}

// ============================================================================
// Тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_detection() {
        assert_eq!(WikiSource::detect_language("квантовая механика"), "ru");
        assert_eq!(WikiSource::detect_language("Rust tokio async"), "en");
        assert_eq!(WikiSource::detect_language("Γραφή"), "en"); // не-кириллица → en
        // Смешанная: кириллица где-то есть → ru
        assert_eq!(WikiSource::detect_language("asynchronous Rust в 2026"), "ru");
    }

    #[test]
    fn api_url_shape() {
        let w = WikiSource::new("ru");
        let u = w.api_url("action=query&format=json");
        assert_eq!(u.host, "ru.wikipedia.org");
        assert_eq!(u.path, "/w/api.php?action=query&format=json");
        assert_eq!(u.port, 443);
    }

    #[test]
    fn search_parse_fixture() {
        // Реальная форма ответа list=search (сниппет с разметкой).
        let fixture = r#"{"batchcomplete":"","continue":{"sroffset":2,"continue":"-||"},"query":{"searchinfo":{"totalhits":3},"search":[{"ns":0,"title":"Квантовая механика","pageid":12345,"size":65375,"wordcount":4284,"snippet":"<span class=\"searchmatch\">Квантовая</span> механика — фундаментальная физическая теория"},{"ns":0,"title":"Квантовая электродинамика","pageid":23456,"size":40111,"wordcount":2977,"snippet":"раздел <span class=\"searchmatch\">электродинамики</span>"}]}}"#;
        let json = Json::parse(fixture).unwrap();
        let empty = Vec::new();
        let hits = json
            .get("query")
            .and_then(|q| q.get("search"))
            .and_then(|s| s.as_arr())
            .unwrap_or(&empty);
        assert_eq!(hits.len(), 2);
        let t0 = hits[0].get("title").and_then(|t| t.as_str()).unwrap();
        assert_eq!(t0, "Квантовая механика");
        let raw_snip = hits[0].get("snippet").and_then(|s| s.as_str()).unwrap();
        let clean = crate::stream_engine::strip_html(raw_snip.as_bytes());
        assert!(!clean.contains('<'), "разметка не вырезана: {clean}");
        // strip_html вставляет пробелы между блоками — проверяем слова
        assert!(clean.contains("Квантовая"));
        assert!(clean.contains("механика"));
    }

    #[test]
    fn extracts_parse_fixture() {
        // Реальная форма ответа prop=extracts: pages как объект по pageid.
        let fixture = r#"{"batchcomplete":"","query":{"normalized":[{"from":"rust_lang","to":"Rust lang"}],"pages":{"53631":{"pageid":53631,"ns":0,"title":"Rust","extract":"Rust — мультипарадигмальный компилируемый язык программирования общего назначения."},"99999":{"pageid":99999,"ns":0,"title":"Нет такой","missing":""}}}}"#;
        let json = Json::parse(fixture).unwrap();
        let pages = json.get("query").and_then(|q| q.get("pages")).unwrap();
        let pairs = match pages {
            Json::Obj(pairs) => pairs,
            _ => panic!("pages не объект"),
        };
        assert_eq!(pairs.len(), 2);
        let mut found = 0;
        for (_, page) in pairs.iter() {
            let missing = page.get("missing").is_some();
            let title = page.get("title").and_then(|t| t.as_str()).unwrap_or("");
            let text = page.get("extract").and_then(|t| t.as_str()).unwrap_or("");
            if !missing && !text.trim().is_empty() {
                assert_eq!(title, "Rust");
                assert!(text.contains("мультипарадигмальный"));
                found += 1;
            }
        }
        assert_eq!(found, 1, "missing-страница должна отфильтроваться");
    }

    #[test]
    fn title_percent_encoding() {
        // Пробелы → _, весь батч → %XX; «|» сепаратор → %7C.
        let titles = vec!["Квантовая механика".to_string(), "Rust (язык)".to_string()];
        let joined = titles
            .iter()
            .map(|t| percent_encode(t.replace(' ', "_").as_str()))
            .collect::<Vec<_>>()
            .join("%7C");
        assert!(joined.contains("%D0%9A%D0%B2%D0%B0%D0%BD%D1%82%D0%BE%D0%B2%D0%B0%D1%8F"));
        // Скобки не входят в безопасный набор — кодируются %28/%29
        assert!(joined.contains("Rust_%28"));
        assert!(joined.contains("%D1%8F%D0%B7%D1%8B%D0%BA"));
        assert!(joined.contains("%7C"));
    }

    /// Живой поиск + извлечение (запускать явно:
    /// `cargo test -p pqc --lib wikisrc::tests::live_search_extract -- --ignored`).
    #[test]
    #[ignore = "живой интернет: смоук источника знаний"]
    fn live_search_extract() {
        let mut wiki = WikiSource::new(WikiSource::detect_language("квантовая механика"));
        assert_eq!(wiki.lang(), "ru");
        let hits = wiki.search("квантовая механика", 3).unwrap();
        assert!(!hits.is_empty(), "пустой поиск");
        assert!(hits.iter().any(|h| h.title.contains("механик")));
        let titles: Vec<String> = hits.iter().take(2).map(|h| h.title.clone()).collect();
        let pages = wiki.extracts(&titles, true).unwrap();
        assert!(!pages.is_empty());
        for p in &pages {
            assert!(p.text.len() > 200, "слишком короткий extract: {}", p.title);
            assert!(!p.text.contains("<"), "HTML в explaintext: {}", &p.text[..80.min(p.text.len())]);
        }
    }
}
