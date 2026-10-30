//! RQ19: HTTP/1.1-клиент поверх TLS 1.3 — zero-dep транспорт знаний.
//!
//! Минималистичный, но честный клиент для точечных GET-запросов:
//!
//! * [`Url`] — разбор `https://host[:port]/path?query` (+ percent-encoding
//!   для параметров Wikipedia API);
//! * [`HttpClient::get`] — GET c заголовками `Host`, `User-Agent`,
//!   `Accept-Encoding: identity` (никакого gzip — распаковщика в
//!   zero-dep мире нет), `Connection: close` (нет keep-alive машины);
//! * ответы с `Content-Length`, `Transfer-Encoding: chunked` и
//!   «до конца потока»;
//! * редиректы 301/302/303/307/308 с `Location` (до 5 прыжков),
//!   включая апгрейд http → https;
//! * [`NetError`] — человекочитаемые отказы.
//!
//! Вежливость к серверам (anti-bot safe — легальный API вместо скрейпинга):
//! честный `User-Agent` с URL проекта, таймауты на соединение и чтение,
//! один запрос на одно TCP-соединение.

use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::tls13::TlsConnection;

/// Максимальный размер ответа (10 МиБ) — защита от зацикленных ответов.
const MAX_BODY: usize = 10 * 1024 * 1024;
/// Максимум прыжков редиректа.
const MAX_REDIRECTS: usize = 5;

/// Ошибки сети.
#[derive(Debug)]
pub enum NetError {
    /// TLS-транспорт.
    Tls(crate::tls13::TlsError),
    /// Сокет.
    Io(std::io::Error),
    /// Неразборчивый URL.
    BadUrl(&'static str),
    /// Неразборчивый ответ.
    BadResponse(&'static str),
    /// Цепочка редиректов длиннее лимита.
    TooManyRedirects,
    /// HTTP-статус-ошибка с кодом.
    Status(u16),
    /// Слишком большой ответ.
    BodyTooLarge,
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetError::Tls(e) => write!(f, "tls: {e}"),
            NetError::Io(e) => write!(f, "сеть: {e}"),
            NetError::BadUrl(w) => write!(f, "url: {w}"),
            NetError::BadResponse(w) => write!(f, "http: {w}"),
            NetError::TooManyRedirects => {
                write!(f, "редиректы: больше {MAX_REDIRECTS} прыжков")
            }
            NetError::Status(code) => write!(f, "источник вернул HTTP {code}"),
            NetError::BodyTooLarge => write!(f, "ответ больше {MAX_BODY} Б"),
        }
    }
}

impl std::error::Error for NetError {}

impl From<crate::tls13::TlsError> for NetError {
    fn from(e: crate::tls13::TlsError) -> Self {
        NetError::Tls(e)
    }
}

impl From<std::io::Error> for NetError {
    fn from(e: std::io::Error) -> Self {
        NetError::Io(e)
    }
}

// ============================================================================
// URL
// ============================================================================

/// Разобранный URL: схема, хост, порт, путь с запросом.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    pub https: bool,
    pub host: String,
    pub port: u16,
    /// Путь с запросом: `/w/api.php?action=...`.
    pub path: String,
}

impl Url {
    /// Разбор абсолютного URL `http(s)://host[:port][/path][?query]`.
    pub fn parse(s: &str) -> Result<Url, NetError> {
        let (https, rest) = if let Some(r) = s.strip_prefix("https://") {
            (true, r)
        } else if let Some(r) = s.strip_prefix("http://") {
            (false, r)
        } else {
            return Err(NetError::BadUrl("нужен http:// или https://"));
        };
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        // хост без userinfo и без IPv6-скобок (домены API — достаточно)
        if authority.is_empty() {
            return Err(NetError::BadUrl("пустой хост"));
        }
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) => {
                if !p.bytes().all(|b| b.is_ascii_digit()) || p.is_empty() || h.is_empty() {
                    return Err(NetError::BadUrl("порт не числовой"));
                }
                (h.to_string(), p.parse::<u16>().map_err(|_| NetError::BadUrl("порт вне диапазона"))?)
            }
            None => (authority.to_string(), if https { 443 } else { 80 }),
        };
        if !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_')
        {
            return Err(NetError::BadUrl("хост: только ASCII-домены"));
        }
        if !path.starts_with('/') {
            return Err(NetError::BadResponse("путь без ведущего слэша"));
        }
        Ok(Url { https, host, port, path: path.to_string() })
    }

    /// Строка `Host:` заголовка (без порта для стандартных).
    fn host_header(&self) -> String {
        let default = (self.https && self.port == 443) || (!self.https && self.port == 80);
        if default {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

/// percent-encoding_query-параметра: безопасные символы `[A-Za-z0-9_.~-]`
/// и пробел → `+` (как в формах), остальное — `%XX` по байтам UTF-8.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// ============================================================================
// Ответ
// ============================================================================

/// Разобранный HTTP-ответ.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    /// Заголовки в нижнем регистре имён.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Значение заголовка (регистр имён не важен).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Разбор полного ответа (заголовки + тело) из буфера.
///
/// Тело определяется по `Content-Length` (обрезается, если сервер прислал
/// больше), иначе — всё до конца буфера. Chunked-кодирование разбирается
/// отдельно [`decode_chunked`].
pub fn parse_response(buf: &[u8]) -> Result<HttpResponse, NetError> {
    let header_end = find_subslice(buf, b"\r\n\r\n")
        .ok_or(NetError::BadResponse("нет конца заголовков"))?;
    let head = std::str::from_utf8(&buf[..header_end])
        .map_err(|_| NetError::BadResponse("заголовки не UTF-8"))?;
    let mut lines = head.split("\r\n");
    let status_line = lines.next().ok_or(NetError::BadResponse("пустой ответ"))?;
    let mut parts = status_line.splitn(3, ' ');
    let version = parts.next().unwrap_or("");
    if !version.starts_with("HTTP/1.") {
        return Err(NetError::BadResponse("не HTTP/1.x"));
    }
    let status: u16 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or(NetError::BadResponse("статус не числовой"))?;
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    let rest = &buf[header_end + 4..];
    let resp = HttpResponse { status, headers, body: rest.to_vec() };
    Ok(resp)
}

/// Декодирование `Transfer-Encoding: chunked` (RFC 9112 §7.1).
pub fn decode_chunked(body: &[u8]) -> Result<Vec<u8>, NetError> {
    let mut out = Vec::with_capacity(body.len());
    let mut p = 0usize;
    loop {
        // строка размера: hex [;ext]
        let line_end = find_subslice(&body[p..], b"\r\n")
            .ok_or(NetError::BadResponse("chunked: нет конца строки размера"))?
            + p;
        let line = std::str::from_utf8(&body[p..line_end])
            .map_err(|_| NetError::BadResponse("chunked: размер не ASCII"))?;
        let size_str = line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_str, 16)
            .map_err(|_| NetError::BadResponse("chunked: размер не hex"))?;
        p = line_end + 2;
        if size == 0 {
            return Ok(out); // последний чанк (трейлеры не нужны)
        }
        if p + size > body.len() {
            return Err(NetError::BadResponse("chunked: чанк обрезан"));
        }
        out.extend_from_slice(&body[p..p + size]);
        p += size;
        // после чанка обязана идти \r\n
        if body.len() < p + 2 || &body[p..p + 2] != b"\r\n" {
            return Err(NetError::BadResponse("chunked: нет CRLF после чанка"));
        }
        p += 2;
    }
}

/// Разбор `Location` редиректа относительно базового URL.
pub fn resolve_redirect(base: &Url, location: &str) -> Result<Url, NetError> {
    if location.starts_with("http://") || location.starts_with("https://") {
        return Url::parse(location);
    }
    if location.starts_with('/') {
        return Ok(Url { path: location.to_string(), ..base.clone() });
    }
    // относительный путь: заменить последний сегмент
    let dir = match base.path.rfind('/') {
        Some(i) => &base.path[..i + 1],
        None => "/",
    };
    Ok(Url { path: format!("{dir}{location}"), ..base.clone() })
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > hay.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

// ============================================================================
// Клиент
// ============================================================================

/// Транспорт соединения: TLS или чистый TCP.
enum Transport {
    Tls(Box<TlsConnection>),
    Plain(TcpStream),
}

impl Transport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, NetError> {
        match self {
            Transport::Tls(t) => Ok(t.read(buf)?),
            Transport::Plain(s) => {
                loop {
                    match s.read(buf) {
                        Ok(0) => return Ok(0),
                        Ok(n) => return Ok(n),
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(e) => return Err(NetError::Io(e)),
                    }
                }
            }
        }
    }

    fn write_all(&mut self, data: &[u8]) -> Result<(), NetError> {
        match self {
            Transport::Tls(t) => Ok(t.write(data)?),
            Transport::Plain(s) => Ok(s.write_all(data)?),
        }
    }
}

/// HTTP-клиент с настройками вежливости.
#[derive(Debug, Clone)]
pub struct HttpClient {
    pub timeout: Duration,
    pub user_agent: String,
    pub accept: String,
    pub max_redirects: usize,
}

impl Default for HttpClient {
    fn default() -> Self {
        HttpClient {
            timeout: Duration::from_secs(20),
            user_agent: format!(
                "POLER-Quantum/{} (+https://github.com/Kotokvit/POLER-Quantum-RS)",
                env!("CARGO_PKG_VERSION")
            ),
            accept: "application/json, text/plain;q=0.9, */*;q=0.5".to_string(),
            max_redirects: MAX_REDIRECTS,
        }
    }
}

impl HttpClient {
    /// GET с автоматическими редиректами. Один запрос — одно соединение
    /// (`Connection: close`): просто и без машины keep-alive.
    pub fn get(&self, url: &Url) -> Result<HttpResponse, NetError> {
        let mut current = url.clone();
        for _ in 0..=self.max_redirects {
            let resp = self.get_once(&current)?;
            if matches!(resp.status, 301 | 302 | 303 | 307 | 308) {
                let loc = resp
                    .header("location")
                    .ok_or(NetError::BadResponse("редирект без Location"))?
                    .to_string();
                current = resolve_redirect(&current, &loc)?;
                continue;
            }
            return Ok(resp);
        }
        Err(NetError::TooManyRedirects)
    }

    /// Один GET без редиректов.
    fn get_once(&self, url: &Url) -> Result<HttpResponse, NetError> {
        let mut transport = if url.https {
            Transport::Tls(Box::new(TlsConnection::connect(&url.host, url.port, self.timeout)?))
        } else {
            let addr = (url.host.as_str(), url.port)
                .to_socket_addrs()
                .map_err(NetError::Io)?
                .next()
                .ok_or(NetError::BadUrl("хост не разрешается"))?;
            let s = TcpStream::connect_timeout(&addr, self.timeout)?;
            s.set_read_timeout(Some(self.timeout))?;
            s.set_write_timeout(Some(self.timeout))?;
            Transport::Plain(s)
        };
        let req = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: {}\r\nAccept: {}\r\n\
             Accept-Encoding: identity\r\nConnection: close\r\n\r\n",
            url.path,
            url.host_header(),
            self.user_agent,
            self.accept,
        );
        transport.write_all(req.as_bytes())?;

        // Читаем до EOF (Connection: close) или Content-Length.
        let mut buf = Vec::with_capacity(16 * 1024);
        let mut chunk = [0u8; 16 * 1024];
        let content_len: Option<usize>;
        loop {
            // Если заголовки уже пришли и известен Content-Length —
            // прекращаем чтение по достижению размера.
            if let Some(hend) = find_subslice(&buf, b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&buf[..hend]).to_ascii_lowercase();
                if !head.contains("transfer-encoding:") {
                    if let Some(cl) = parse_content_length(&head) {
                        if buf.len() >= hend + 4 + cl {
                            content_len = Some(cl);
                            break;
                        }
                    }
                }
            }
            match transport.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    if buf.len() > MAX_BODY {
                        return Err(NetError::BodyTooLarge);
                    }
                }
                Err(e) => return Err(e),
            }
        }
        let _ = content_len;
        let mut resp = parse_response(&buf)?;
        if resp.header("transfer-encoding").map(|v| v.to_ascii_lowercase().contains("chunked")).unwrap_or(false) {
            resp.body = decode_chunked(&resp.body)?;
        } else if let Some(cl) = resp
            .header("content-length")
            .and_then(|v| v.parse::<usize>().ok())
        {
            if resp.body.len() > cl {
                resp.body.truncate(cl);
            }
        }
        Ok(resp)
    }
}

/// `Content-Length` из уже опущенных в нижний регистр заголовков.
fn parse_content_length(head_lower: &str) -> Option<usize> {
    for line in head_lower.split("\r\n") {
        if let Some(v) = line.strip_prefix("content-length:") {
            return v.trim().parse().ok();
        }
    }
    None
}

use std::net::ToSocketAddrs;

// ============================================================================
// Тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_parse_variants() {
        let u = Url::parse("https://ru.wikipedia.org/w/api.php?action=query").unwrap();
        assert!(u.https);
        assert_eq!(u.host, "ru.wikipedia.org");
        assert_eq!(u.port, 443);
        assert_eq!(u.path, "/w/api.php?action=query");
        assert_eq!(u.host_header(), "ru.wikipedia.org");

        let u = Url::parse("http://example.com:8080/x").unwrap();
        assert!(!u.https);
        assert_eq!(u.port, 8080);
        assert_eq!(u.host_header(), "example.com:8080");

        // Без пути → «/»
        assert_eq!(Url::parse("https://a.b").unwrap().path, "/");

        assert!(Url::parse("ftp://x").is_err());
        assert!(Url::parse("https://").is_err());
        assert!(Url::parse("https://ho st/").is_err());
        assert!(Url::parse("notevenurl").is_err());
    }

    #[test]
    fn percent_encode_utf8_and_reserved() {
        assert_eq!(percent_encode("tokio"), "tokio");
        assert_eq!(percent_encode("a b"), "a+b");
        assert_eq!(percent_encode("квант"), "%D0%BA%D0%B2%D0%B0%D0%BD%D1%82");
        assert_eq!(percent_encode("a&b=c|d"), "a%26b%3Dc%7Cd");
    }

    #[test]
    fn response_parse_and_headers() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Empty:\r\n\r\n{\"a\":1}";
        let r = parse_response(raw).unwrap();
        assert_eq!(r.status, 200);
        assert!(r.ok());
        assert_eq!(r.header("content-type"), Some("application/json"));
        assert_eq!(r.body, b"{\"a\":1}".to_vec());

        // Content-Length больше фактического тела — тело до конца буфера
        let raw = b"HTTP/1.1 404 Not Found\r\nContent-Length: 999\r\n\r\nshort";
        let r = parse_response(raw).unwrap();
        assert_eq!(r.status, 404);
        assert!(!r.ok());
        assert_eq!(r.body, b"short".to_vec());

        assert!(parse_response(b"garbage").is_err());
        assert!(parse_response(b"HTTP/2 200\r\n\r\n").is_err());
    }

    #[test]
    fn chunked_decode() {
        // Классика из RFC 9112 §7.1
        let body = b"4\r\nWiki\r\n5\r\npedia\r\nE\r\n in\r\n\r\nchunks.\r\n0\r\n\r\n";
        let out = decode_chunked(body).unwrap();
        assert_eq!(out, b"Wikipedia in\r\n\r\nchunks.".to_vec());
        // Пустое тело
        assert_eq!(decode_chunked(b"0\r\n\r\n").unwrap(), Vec::<u8>::new());
        // Обрезанный чанк — отказ
        assert!(decode_chunked(b"5\r\nabc").is_err());
        // Не-hex размер — отказ
        assert!(decode_chunked(b"zz\r\n").is_err());
        // Нет CRLF после чанка — отказ
        assert!(decode_chunked(b"3\r\nabcXX").is_err());
    }

    #[test]
    fn redirect_resolution() {
        let base = Url::parse("https://ru.wikipedia.org/wiki/A").unwrap();
        let abs = resolve_redirect(&base, "https://en.wikipedia.org/wiki/B").unwrap();
        assert_eq!(abs.host, "en.wikipedia.org");
        assert_eq!(abs.path, "/wiki/B");
        let root = resolve_redirect(&base, "/w/api.php?x=1").unwrap();
        assert_eq!(root.host, "ru.wikipedia.org");
        assert_eq!(root.path, "/w/api.php?x=1");
        let rel = resolve_redirect(&base, "C").unwrap();
        assert_eq!(rel.path, "/wiki/C");
    }

    #[test]
    fn content_length_scan() {
        let head = "HTTP/1.1 200\r\ncontent-type: text/plain\r\ncontent-length: 42\r\n";
        assert_eq!(parse_content_length(head), Some(42));
        assert_eq!(parse_content_length("HTTP/1.1 200\r\n\r\n"), None);
    }

    /// Живой GET к Wikipedia API (запускать явно:
    /// `cargo test -p pqc --lib netfetch::tests::live_wikipedia_api -- --ignored`).
    #[test]
    #[ignore = "живой интернет: смоук HTTP-слоя"]
    fn live_wikipedia_api() {
        let q = percent_encode("квантовая механика");
        let url = Url::parse(&format!(
            "https://ru.wikipedia.org/w/api.php?action=query&list=search&srsearch={q}&srlimit=1&format=json"
        ))
        .unwrap();
        let client = HttpClient::default();
        let resp = client.get(&url).unwrap();
        assert!(resp.ok(), "статус {}", resp.status);
        let text = String::from_utf8_lossy(&resp.body);
        assert!(text.contains("\"query\""), "нет JSON: {}", &text[..120.min(text.len())]);
    }
}
