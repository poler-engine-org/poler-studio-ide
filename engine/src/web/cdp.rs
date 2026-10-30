//! Нативный CDP-клиент (Chrome DevTools Protocol) на чистом std.
//!
//! Замена «костыльной» связке Rust → Node.js → CLI → Chromium из
//! super-z-skills (skills/agent-browser): без npm, без spawn подпроцессов,
//! прямой WebSocket к отрендеренному Chromium.
//!
//! Реализовано:
//! * WebSocket-клиент RFC 6455 поверх `TcpStream` (handshake,
//!   маскированные клиентские фреймы, приём серверных);
//! * HTTP `GET /json` — discovery таргетов;
//! * CDP: `Page.enable`/`Network.enable`/`Page.navigate`/
//!   `Runtime.evaluate` (рендер-текст страницы, Shadow DOM включён —
//!   браузер исполняет весь JS);
//! * Перехват сетевых ответов JSON API (`Network.responseReceived` +
//!   `Network.getResponseBody`) — сырые данные до превращения в HTML.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// Максимальный размер одного WebSocket-сообщения (текст страницы).
const MAX_WS_MESSAGE: usize = 64 * 1024 * 1024;

/// Бюджет на выгрузку тел перехваченных JSON-ответов (внутри одной страницы):
/// long-polling/стриминг не должны съедать минуты.
const INTERCEPT_BUDGET: Duration = Duration::from_secs(8);
/// Сколько тел JSON-ответов выгружать максимум на страницу.
const INTERCEPT_MAX: usize = 32;

/// Таймаут чтения WS с учётом дедлайна страницы: не больше `cap`,
/// но и не дольше остатка дедлайна (пол ≥ 100 мс — без busy-loop).
fn clamp_timeout(remaining: Option<Duration>, cap: Duration) -> Duration {
    let floor = Duration::from_millis(100);
    match remaining {
        Some(r) => r.max(floor).min(cap),
        None => cap,
    }
}

// ---------------------------------------------------------------------------
// base64: encode для Sec-WebSocket-Key, decode для скриншотов и медиа
// ---------------------------------------------------------------------------

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Стандартный base64-декодер (без паддинга и с мусором вокруг — терпит).
/// Нужен для `Page.captureScreenshot` и загрузки медиа NotebookLM.
pub fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Result<u32, String> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'+' | b'-' => Ok(62),
            b'/' | b'_' => Ok(63),
            _ => Err(format!("не-base64 символ {:?}", c as char)),
        }
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for &c in s.as_bytes() {
        if c == b'=' || c == b'\n' || c == b'\r' || c == b' ' {
            continue; // паддинг и перевод строк игнорируем
        }
        acc = (acc << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

/// 16 случайных байт из /dev/urandom (или fallback на время).
/// НЕ для криптографии: маски WS-фреймов и прочая некритичная энтропия.
pub fn random_key_16() -> [u8; 16] {
    let mut buf = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    } else {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for (i, b) in buf.iter_mut().enumerate() {
            *b = ((t >> (i * 4)) & 0xFF) as u8 ^ (i as u8).wrapping_mul(31);
        }
    }
    buf
}

/// Аудит-фикс №C3: криптостойкая энтропия с fail-closed-семантикой.
/// Для OAuth-state и прочего критичного: предсказуемый fallback
/// (время+счётчик) неприемлем — лучше упасть громко, чем выдать
/// предсказуемый CSRF-токен. На Linux /dev/urandom есть всегда.
pub fn secure_key_16() -> [u8; 16] {
    let mut buf = [0u8; 16];
    let mut f = std::fs::File::open("/dev/urandom")
        .expect("poler-engine: /dev/urandom недоступен — источник энтропии обязателен");
    f.read_exact(&mut buf)
        .expect("poler-engine: не удалось прочитать /dev/urandom");
    buf
}

// ---------------------------------------------------------------------------
// WebSocket-клиент (минимальный, текстовые фреймы)
// ---------------------------------------------------------------------------

struct WsClient {
    stream: TcpStream,
}

impl WsClient {
    /// Подключение к `ws://host:port/path` с WS-handshake.
    fn connect(host: &str, port: u16, path: &str) -> Result<Self, String> {
        let mut stream = TcpStream::connect((host, port)).map_err(|e| format!("tcp: {e}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .ok();
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .ok();
        stream.set_nodelay(true).ok();

        let key = base64_encode(&random_key_16());
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\n\
             Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n\
             Sec-WebSocket-Version: 13\r\n\r\n"
        );
        stream
            .write_all(req.as_bytes())
            .map_err(|e| format!("handshake write: {e}"))?;

        // читать до \r\n\r\n
        let mut buf = Vec::with_capacity(512);
        let mut byte = [0u8; 1];
        loop {
            stream
                .read_exact(&mut byte)
                .map_err(|e| format!("handshake read: {e}"))?;
            buf.push(byte[0]);
            if buf.ends_with(b"\r\n\r\n") {
                break;
            }
            if buf.len() > 4096 {
                return Err("handshake: слишком длинный ответ".into());
            }
        }
        let head = String::from_utf8_lossy(&buf);
        if !head.starts_with("HTTP/1.1 101") {
            return Err(format!(
                "handshake отклонён: {}",
                head.lines().next().unwrap_or("")
            ));
        }
        Ok(Self { stream })
    }

    /// Отправка текстового фрейма (клиентские фреймы маскируются).
    fn send_text(&mut self, payload: &str) -> Result<(), String> {
        let data = payload.as_bytes();
        let mask: [u8; 4] = random_key_16()[..4].try_into().unwrap();
        let mut frame = Vec::with_capacity(data.len() + 14);
        frame.push(0x81); // FIN + text
        let len = data.len();
        if len < 126 {
            frame.push(0x80 | len as u8);
        } else if len <= 0xFFFF {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(len as u64).to_be_bytes());
        }
        frame.extend_from_slice(&mask);
        for (i, b) in data.iter().enumerate() {
            frame.push(b ^ mask[i % 4]);
        }
        self.stream
            .write_all(&frame)
            .map_err(|e| format!("ws send: {e}"))
    }

    /// Приём одного текстового сообщения (склейка фрагментов,
    /// ping -> pong, pong/close игнорируются).
    fn recv_text(&mut self) -> Result<String, String> {
        let mut message: Vec<u8> = Vec::new();
        loop {
            let (opcode, fin, payload) = self.recv_frame()?;
            match opcode {
                0x9 => {
                    // ping -> pong (только корректные пинги ≤ 125 байт:
                    // для более длинных длина не влезает в один байт фрейма)
                    if payload.len() <= 125 {
                        let mask: [u8; 4] = random_key_16()[..4].try_into().unwrap();
                        let mut pong = vec![0x8A, 0x80 | payload.len() as u8];
                        pong.extend_from_slice(&mask);
                        for (i, b) in payload.iter().enumerate() {
                            pong.push(b ^ mask[i % 4]);
                        }
                        let _ = self.stream.write_all(&pong);
                    }
                    continue;
                }
                0xA => continue, // pong
                0x8 => return Err("ws closed".into()),
                _ => {}
            }
            if message.len() + payload.len() > MAX_WS_MESSAGE {
                return Err("ws: сообщение превышает лимит".into());
            }
            message.extend_from_slice(&payload);
            if fin {
                return String::from_utf8(message)
                    .map_err(|_| "ws: невалидный UTF-8".to_string());
            }
        }
    }

    /// Приём одного фрейма: (opcode, fin, payload).
    fn recv_frame(&mut self) -> Result<(u8, bool, Vec<u8>), String> {
        let mut hdr = [0u8; 2];
        self.stream
            .read_exact(&mut hdr)
            .map_err(|e| format!("ws frame hdr: {e}"))?;
        let fin = hdr[0] & 0x80 != 0;
        let opcode = hdr[0] & 0x0F;
        let masked = hdr[1] & 0x80 != 0;
        let len = hdr[1] & 0x7F;
        let len = match len {
            126 => {
                let mut b = [0u8; 2];
                self.stream.read_exact(&mut b).map_err(|e| e.to_string())?;
                u16::from_be_bytes(b) as usize
            }
            127 => {
                let mut b = [0u8; 8];
                self.stream.read_exact(&mut b).map_err(|e| e.to_string())?;
                u64::from_be_bytes(b) as usize
            }
            n => n as usize,
        };
        let mask = if masked {
            let mut m = [0u8; 4];
            self.stream.read_exact(&mut m).map_err(|e| e.to_string())?;
            Some(m)
        } else {
            None
        };
        if len > MAX_WS_MESSAGE {
            return Err("ws: фрейм превышает лимит".into());
        }
        let mut payload = vec![0u8; len];
        self.stream
            .read_exact(&mut payload)
            .map_err(|e| format!("ws frame payload: {e}"))?;
        if let Some(m) = mask {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= m[i % 4];
            }
        }
        Ok((opcode, fin, payload))
    }
}

// ---------------------------------------------------------------------------
// CDP-клиент
// ---------------------------------------------------------------------------

/// HTTP GET к DevTools HTTP-эндпоинту (например, `/json`).
/// Читает тело по Content-Length (read_to_end ломается на keep-alive
/// соединениях Chromium с EAGAIN).
pub fn http_get(host: &str, port: u16, path: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect((host, port)).map_err(|e| format!("tcp: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok();
    let req =
        format!("GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("http write: {e}"))?;

    // заголовки до \r\n\r\n
    let mut head = Vec::with_capacity(256);
    let mut b = [0u8; 1];
    loop {
        stream
            .read_exact(&mut b)
            .map_err(|e| format!("http head: {e}"))?;
        head.push(b[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
        if head.len() > 8192 {
            return Err("http: гигантские заголовки".into());
        }
    }
    let head_str = String::from_utf8_lossy(&head).to_string();
    let content_len: usize = head_str
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    // Аудит-фикс №E1: потолок на заявленный Content-Length. Раньше
    // vec![0u8; content_len] аллоцировался по заявлению сервера без
    // ограничений — подставной процесс на свободном CDP-порту мог
    // заставить движок аллоцировать гигабайты.
    const HTTP_BODY_CAP: usize = 16 * 1024 * 1024;
    if content_len > HTTP_BODY_CAP {
        return Err("http: Content-Length превышает 16 МБ".into());
    }

    // тело: Content-Length байт (или до EOF при chunked/отсутствии)
    let mut body = vec![0u8; content_len];
    if content_len > 0 {
        stream
            .read_exact(&mut body)
            .map_err(|e| format!("http body: {e}"))?;
    } else {
        let mut buf = Vec::new();
        // короткое окно: DevTools /json всегда отдаёт Content-Length
        let _ = stream.read_to_end(&mut buf);
        body = buf;
    }
    Ok(String::from_utf8_lossy(&body).to_string())
}

/// Живое соединение к Chromium CDP.
pub struct CdpSession {
    ws: WsClient,
    next_id: u64,
    /// Кооперативный дедлайн текущей загрузки страницы (пер-страничный
    /// таймаут краулера). None = без лимита.
    page_deadline: Option<Instant>,
}

/// Результат загрузки страницы: рендер-текст + перехваченный JSON.
pub struct WebPage {
    pub url: String,
    pub text: String,
    pub json_responses: Vec<(String, String)>, // (url, body)
}

/// Полная выгрузка страницы для краулера (load_page_full).
pub struct WebPageFull {
    /// Финальный URL после редиректов (location.href).
    pub final_url: String,
    pub title: String,
    pub meta_description: String,
    pub lang: String,
    pub text: String,
    /// Абсолютные ссылки со страницы (уже резолвлены браузером).
    pub links: Vec<String>,
    pub json_responses: Vec<(String, String)>,
}

impl CdpSession {
    /// Подключение к первому page-таргету Chromium на порту CDP.
    pub fn connect(port: u16) -> Result<Self, String> {
        let list = http_get("127.0.0.1", port, "/json")?;
        let targets: serde_json::Value =
            serde_json::from_str(&list).map_err(|e| format!("json /json: {e}"))?;
        let ws_url = targets
            .as_array()
            .and_then(|arr| {
                arr.iter()
                    .find(|t| t.get("type").and_then(|v| v.as_str()) == Some("page"))
                    .and_then(|t| t.get("webSocketDebuggerUrl"))
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .ok_or("не найден page-таргет (Chromium запущен с --remote-debugging-port?)")?;

        // ws://127.0.0.1:PORT/devtools/page/ID
        let path = ws_url
            .split_once("ws://")
            .and_then(|(_, rest)| rest.find('/').map(|i| rest[i..].to_string()))
            .unwrap_or_else(|| "/devtools/page/0".to_string());
        let ws = WsClient::connect("127.0.0.1", port, &path)?;
        let mut s = Self {
            ws,
            next_id: 1,
            page_deadline: None,
        };
        s.command("Page.enable", "{}")?;
        s.command("Network.enable", "{}")?;
        s.apply_stealth()?;
        Ok(s)
    }

    /// Установить/снять дедлайн текущей страницы (пер-страничный таймаут).
    pub fn set_page_deadline(&mut self, deadline: Option<Instant>) {
        self.page_deadline = deadline;
    }

    /// Остаток до дедлайна страницы (None = лимита нет).
    fn deadline_remaining(&self) -> Option<Duration> {
        self.page_deadline.map(|d| d.saturating_duration_since(Instant::now()))
    }

    /// Дедлайн страницы истёк?
    fn deadline_hit(&self) -> bool {
        self.page_deadline
            .map(|d| Instant::now() >= d)
            .unwrap_or(false)
    }

    /// Подстроить read-таймаут WS под дедлайн (cap 30 с).
    fn tune_recv_timeout(&mut self) {
        let t = clamp_timeout(self.deadline_remaining(), Duration::from_secs(30));
        self.ws.stream.set_read_timeout(Some(t)).ok();
    }

    /// Десктопный UA вместо «HeadlessChrome/…» (выдаёт автоматизацию
    /// простым фильтрам) + navigator.webdriver → undefined. Реальный
    /// рендер остаётся честным: это всё ещё полный Chromium 152.
    fn apply_stealth(&mut self) -> Result<(), String> {
        let ua = std::env::var("POLER_USER_AGENT").unwrap_or_else(|_| {
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/152.0.7977.54 Safari/537.36"
                .to_string()
        });
        self.command(
            "Network.setUserAgentOverride",
            &serde_json::json!({
                "userAgent": ua,
                "acceptLanguage": "uk-UA,uk;q=0.9,ru;q=0.8,en-US;q=0.7,en;q=0.6",
            })
            .to_string(),
        )?;
        self.command(
            "Page.addScriptToEvaluateOnNewDocument",
            &serde_json::json!({
                "source": "Object.defineProperty(navigator,'webdriver',{get:()=>undefined});"
            })
            .to_string(),
        )?;
        Ok(())
    }

    /// Отправка CDP-команды; события по пути складываются в `events`
    /// (нужны для сетевого перехвата).
    fn command_collect(
        &mut self,
        method: &str,
        params: &str,
        events: &mut Vec<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let msg = format!(r#"{{"id":{id},"method":"{method}","params":{params}}}"#);
        self.ws.send_text(&msg)?;
        loop {
            if self.deadline_hit() {
                return Err("пер-страничный таймаут (CdpSession::command_collect)".into());
            }
            self.tune_recv_timeout();
            let text = match self.ws.recv_text() {
                Ok(t) => t,
                // read-timeout при почти истёкшем дедлайне — это таймаут
                // страницы, а не сетевая ошибка: называем вещи своими именами
                Err(e) => {
                    let near = self
                        .deadline_remaining()
                        .map(|r| r < Duration::from_secs(2))
                        .unwrap_or(false);
                    if near {
                        return Err(format!("пер-страничный таймаут: страница не отдалась за лимит ({e})"));
                    }
                    return Err(e);
                }
            };
            let v: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| format!("cdp json: {e}"))?;
            if v.get("id").and_then(|i| i.as_u64()) == Some(id) {
                if let Some(err) = v.get("error") {
                    return Err(format!("cdp error: {err}"));
                }
                return Ok(v.get("result").cloned().unwrap_or(serde_json::Value::Null));
            }
            if v.get("method").is_some() {
                events.push(v);
            }
        }
    }

    fn command(&mut self, method: &str, params: &str) -> Result<serde_json::Value, String> {
        let mut ev = Vec::new();
        self.command_collect(method, params, &mut ev)
    }

    /// Runtime.evaluate с awaitPromise: асинхронные выражения (fetch)
    /// из контекста страницы. Браузер выступает HTTPS-клиентом движка —
    /// TLS отдаём Chromium, ноль TLS-зависимостей в Rust.
    pub fn eval_async_string(&mut self, expr: &str) -> Result<String, String> {
        let params = serde_json::json!({
            "expression": expr,
            "returnByValue": true,
            "awaitPromise": true,
        })
        .to_string();
        let res = self.command("Runtime.evaluate", &params)?;
        if let Some(exc) = res.get("exceptionDetails") {
            let desc = exc
                .get("exception")
                .and_then(|e| e.get("description"))
                .and_then(|d| d.as_str())
                .or_else(|| exc.get("text").and_then(|t| t.as_str()))
                .unwrap_or("неизвестное исключение");
            return Err(format!("js: {desc}"));
        }
        Ok(res
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string())
    }

    /// Скриншот текущей страницы: PNG-байты через `Page.captureScreenshot`.
    /// Медиа-канал для сервисов без API: видим страницу ровно как юзер
    /// (фото, слайды, графики — всё, что не отдаётся текстом).
    pub fn capture_screenshot(&mut self) -> Result<Vec<u8>, String> {
        let params = r#"{"format":"png","captureBeyondViewport":true}"#;
        let res = self.command("Page.captureScreenshot", params)?;
        let b64 = res
            .get("data")
            .and_then(|d| d.as_str())
            .ok_or("Page.captureScreenshot не вернул data")?;
        crate::web::cdp::base64_decode(b64)
    }

    /// Runtime.evaluate → строка (returnByValue).
    fn eval_string(&mut self, expr: &str) -> Result<String, String> {
        let params = serde_json::json!({"expression": expr, "returnByValue": true}).to_string();
        let res = self.command("Runtime.evaluate", &params)?;
        Ok(res
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string())
    }

    /// Навигация + ожидание load + дренирование событий сети.
    /// Возвращает собранные CDP-события (перехват JSON, статусы ответов).
    /// Ожидание load ограничено min(20 с, остаток дедлайна страницы).
    fn navigate_and_collect(
        &mut self,
        url: &str,
        wait_ms: u64,
    ) -> Result<Vec<serde_json::Value>, String> {
        let mut events: Vec<serde_json::Value> = Vec::new();
        self.command_collect(
            "Page.navigate",
            &serde_json::json!({"url": url}).to_string(),
            &mut events,
        )?;

        let nav_cap = self
            .deadline_remaining()
            .map(|r| r.min(Duration::from_secs(20)))
            .unwrap_or(Duration::from_secs(20));
        let deadline = std::time::Instant::now() + nav_cap;
        while std::time::Instant::now() < deadline {
            if self.deadline_hit() {
                return Err("пер-страничный таймаут: load не наступил".into());
            }
            self.ws
                .stream
                .set_read_timeout(Some(Duration::from_millis(250)))
                .ok();
            match self.ws.recv_text() {
                Ok(text) => {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                        if v.get("method").and_then(|m| m.as_str())
                            == Some("Page.loadEventFired")
                        {
                            events.push(v);
                            break;
                        }
                        if v.get("method").is_some() {
                            events.push(v);
                        }
                    }
                }
                Err(_) => continue,
            }
        }
        if wait_ms > 0 {
            // пауза на XHR не дальше дедлайна страницы
            let pause = self
                .deadline_remaining()
                .map(|r| r.min(Duration::from_millis(wait_ms)))
                .unwrap_or_else(|| Duration::from_millis(wait_ms));
            std::thread::sleep(pause);
            self.ws
                .stream
                .set_read_timeout(Some(Duration::from_millis(100)))
                .ok();
            while let Ok(text) = self.ws.recv_text() {
                if self.deadline_hit() {
                    break;
                }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if v.get("method").is_some() {
                        events.push(v);
                    }
                }
            }
        }
        self.ws
            .stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .ok();
        Ok(events)
    }

    /// Перехват JSON-ответов скрытых API из накопленных событий.
    /// Только ЗАВЕРШЁННЫСЯ ответы (Network.loadingFinished): тело незавершённого
    /// (long-polling/стрим) запроса не выгружается — раньше это висло минутами.
    /// Бюджет `INTERCEPT_BUDGET` и лимит `INTERCEPT_MAX` — жёсткие.
    fn intercept_json(
        &mut self,
        events: &[serde_json::Value],
    ) -> Vec<(String, String)> {
        let json_requests = finished_json_requests(events);
        let budget = std::time::Instant::now() + INTERCEPT_BUDGET;
        let mut intercepted: Vec<(String, String)> = Vec::new();
        for (rid, url) in json_requests.into_iter().take(INTERCEPT_MAX) {
            if std::time::Instant::now() >= budget || self.deadline_hit() {
                break;
            }
            // короткий read-таймаут: тело либо отдаётся быстро, либо не ждём
            let t = clamp_timeout(
                Some(budget.saturating_duration_since(Instant::now())),
                Duration::from_secs(3),
            );
            self.ws.stream.set_read_timeout(Some(t)).ok();
            if let Ok(res) = self.command(
                "Network.getResponseBody",
                &format!(r#"{{"requestId":"{rid}"}}"#),
            ) {
                if let Some(body) = res.get("body").and_then(|b| b.as_str()) {
                    if body.len() > 16 && body.len() < 8 * 1024 * 1024 {
                        intercepted.push((url.clone(), body.to_string()));
                    }
                }
            } else {
                break; // бюджет исчерпан — остальные тела пропускаем
            }
        }
        self.ws
            .stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .ok();
        intercepted
    }

    /// Graceful-остановка браузера (CDP `Browser.close`).
    pub fn close_browser(&mut self) -> Result<(), String> {
        self.command("Browser.close", "{}").map(|_| ())
    }

    /// Загрузка страницы и извлечение рендер-текста.
    ///
    /// * `wait_ms` — пауза после load на дочерние fetch/XHR
    ///   (React/Vue догружают контент после onload).
    pub fn load_page(&mut self, url: &str, wait_ms: u64) -> Result<WebPage, String> {
        let events = self.navigate_and_collect(url, wait_ms)?;
        let intercepted = self.intercept_json(&events);
        let text = self.eval_string(
            "document.body ? document.body.innerText : ''",
        )?;

        Ok(WebPage {
            url: url.to_string(),
            text,
            json_responses: intercepted,
        })
    }

    /// Полная выгрузка страницы для веб-краулера: финальный URL после
    /// редиректов, заголовок, meta description, язык, рендер-текст и
    /// абсолютные ссылки (`a.href` — браузер сам резолвит относительные).
    pub fn load_page_full(&mut self, url: &str, wait_ms: u64) -> Result<WebPageFull, String> {
        let events = self.navigate_and_collect(url, wait_ms)?;
        let intercepted = self.intercept_json(&events);
        let text = self
            .eval_string("document.body ? document.body.innerText : ''")?;

        // один вызов: title + meta + lang + ссылки (JSON — экранирование честное)
        let meta_json = self.eval_string(
            "JSON.stringify({\
              u: location.href,\
              t: document.title || '',\
              d: (document.querySelector('meta[name=\"description\"]') || {content: ''}).content || '',\
              l: document.documentElement.lang || '',\
              a: Array.from(document.querySelectorAll('a[href]'))\
                   .map(function(a){ return a.href; })\
                   .filter(function(h){ return h.indexOf('http') === 0; })\
                   .slice(0, 800)\
            })",
        )?;
        let meta: serde_json::Value = serde_json::from_str(&meta_json)
            .unwrap_or(serde_json::Value::Null);
        let g = |k: &str| {
            meta.get(k)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let links = meta
            .get("a")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(WebPageFull {
            final_url: if g("u").is_empty() {
                url.to_string()
            } else {
                g("u")
            },
            title: g("t"),
            meta_description: g("d"),
            lang: g("l"),
            text,
            links,
            json_responses: intercepted,
        })
    }

    /// Загрузка «сырого» ресурса (robots.txt, sitemap.xml):
    /// (HTTP-статус, текст). Статус — последнего Document-ответа.
    pub fn fetch_raw(&mut self, url: &str) -> Result<(u16, String), String> {
        let events = self.navigate_and_collect(url, 0)?;
        // статус последнего Document-ответа (учитывает редиректы)
        let status = events
            .iter()
            .rev()
            .filter(|e| {
                e.get("method").and_then(|m| m.as_str()) == Some("Network.responseReceived")
            })
            .find_map(|e| {
                let p = e.get("params")?;
                if p.get("type").and_then(|t| t.as_str()) != Some("Document") {
                    return None;
                }
                p.get("response")?.get("status")?.as_u64().map(|s| s as u16)
            })
            .unwrap_or(200);
        let text = self.eval_string(
            "document.documentElement ? document.documentElement.textContent : ''",
        )?;
        Ok((status, text))
    }
}

/// Из событий CDP выбрать ЗАВЕРШЁННЫЕ JSON-ответы: (requestId, url).
/// Завершённость = было `Network.loadingFinished` для requestId
/// (незавершённый long-polling/стрим тело не отдаёт — источник висений).
fn finished_json_requests(events: &[serde_json::Value]) -> Vec<(String, String)> {
    use std::collections::HashSet;
    let finished: HashSet<&str> = events
        .iter()
        .filter(|e| {
            e.get("method").and_then(|m| m.as_str()) == Some("Network.loadingFinished")
        })
        .filter_map(|e| e.get("params")?.get("requestId")?.as_str())
        .collect();
    events
        .iter()
        .filter_map(|e| {
            let p = e.get("params")?;
            if e.get("method").and_then(|m| m.as_str()) != Some("Network.responseReceived") {
                return None;
            }
            let mime = p.get("response")?.get("mimeType")?.as_str()?;
            if !mime.contains("json") {
                return None;
            }
            let rid = p.get("requestId")?.as_str()?;
            if !finished.contains(rid) {
                return None; // ещё стримится — тело не готово
            }
            let url = p.get("response")?.get("url")?.as_str()?.to_string();
            Some((rid.to_string(), url))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    // ---- пер-страничные дедлайны (фикс №2 UX-аудита) ----

    #[test]
    fn clamp_timeout_respects_deadline_and_floor() {
        let cap = Duration::from_secs(30);
        // без дедлайна — полный cap
        assert_eq!(clamp_timeout(None, cap), cap);
        // дедлайн дальше cap — cap
        assert_eq!(clamp_timeout(Some(Duration::from_secs(120)), cap), cap);
        // дедлайн ближе cap — дедлайн
        assert_eq!(
            clamp_timeout(Some(Duration::from_secs(2)), cap),
            Duration::from_secs(2)
        );
        // истёкший дедлайн — пол 100 мс (без busy-loop)
        assert_eq!(
            clamp_timeout(Some(Duration::ZERO), cap),
            Duration::from_millis(100)
        );
    }

    #[test]
    fn finished_json_requests_skips_unfinished_streams() {
        let events = vec![
            // JSON-ответ пришёл и ЗАВЕРШИЛСЯ — тело выгружаем
            serde_json::json!({"method": "Network.responseReceived", "params": {
                "requestId": "r1", "response": {"url": "https://a/api/1", "mimeType": "application/json"}}}),
            serde_json::json!({"method": "Network.loadingFinished", "params": {"requestId": "r1"}}),
            // JSON-ответ пришёл, но стрим ещё жив (нет loadingFinished) — пропускаем:
            // getResponseBody по нему висел минутами (кейс docs.rs из UX-аудита)
            serde_json::json!({"method": "Network.responseReceived", "params": {
                "requestId": "r2", "response": {"url": "https://a/poll", "mimeType": "application/json"}}}),
            // не-JSON — не интересует
            serde_json::json!({"method": "Network.responseReceived", "params": {
                "requestId": "r3", "response": {"url": "https://a/img", "mimeType": "image/png"}}}),
            serde_json::json!({"method": "Network.loadingFinished", "params": {"requestId": "r3"}}),
        ];
        let got = finished_json_requests(&events);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0], ("r1".to_string(), "https://a/api/1".to_string()));
    }

    #[test]
    fn finished_json_requests_empty() {
        assert!(finished_json_requests(&[]).is_empty());
    }
}
