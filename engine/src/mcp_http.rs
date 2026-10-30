//! MCP-сервер поверх HTTP (Streamable HTTP transport): тот же набор
//! инструментов, что и в stdio-режиме (`--mcp`), но доступный УДАЛЁННОМУ
//! агенту через туннель.
//!
//! `poler-engine --mcp-http 127.0.0.1:8765 --mcp-token <секрет>`
//!
//! Модель безопасности:
//!   * Bearer-токен обязателен для POST / и /mcp (без него — 401);
//!   * токен задаётся `--mcp-token`, env `POLER_MCP_TOKEN` или
//!     генерируется при старте (32 hex из /dev/urandom);
//!   * движок полностью локален (v2.0 sovereign stack): наружу идут
//!     только ответы на MCP-запросы агента;
//!   * наружу публикуется через туннель, например quick-tunnel
//!     без аккаунта: `cloudflared tunnel --url http://127.0.0.1:8765`.
//!
//! Транспорт: JSON-RPC 2.0, POST `/` или `/mcp`, тело — одно сообщение
//! или batch-массив; ответ — application/json (разрешено спецификацией
//! Streamable HTTP; серверные SSE-потоки не предлагаются — GET → 405).
//! Сервер stateless: сессий и `Mcp-Session-Id` нет. `GET /health` —
//! smoke-проба туннеля без токена (ничего не отдаёт, кроме «ok»).
//!
//! Сеть: ручной HTTP/1.1 поверх std::net — ноль новых зависимостей,
//! keep-alive + Expect: 100-continue (curl шлёт его на больших телах),
//! поток на соединение (долгие инструменты не блокируют акцептор).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::mcp::McpServer;

/// Максимум одновременно обрабатываемых соединений (анти-fork-bomb).
const MAX_CONNS: usize = 16;

/// Потолок блока заголовков (slow-loris / мусорные клиенты).
const HEADER_CAP: usize = 16 * 1024;

/// Потолок тела запроса: tools/call-аргументы больше — не бывает.
const BODY_CAP: usize = 8 * 1024 * 1024;

/// Idle-таймаут keep-alive-соединения (между запросами).
const IDLE: Duration = Duration::from_secs(65);

/// Аудит-фикс №C2: ОБЩИЙ бюджет одного запроса (заголовки+тело). Раньше
/// каждый read() сбрасывал 65-секундный таймаут — клиент-«капельница»
/// держал слот неограниченно долго (slow-loris на всех MAX_CONNS слотах).
const REQUEST_DEADLINE: Duration = Duration::from_secs(120);

// =====================================================================
// Запуск
// =====================================================================

/// Запуск MCP-сервера по HTTP. Блокируется до ошибки акцептора
/// (Ctrl+C убивает процесс штатно). Возвращает код процесса.
pub fn run_http(
    bind: &str,
    token: &str,
    cdp_port: u16,
    wait_ms: u64,
    db_path: PathBuf,
    knowledge_db: Option<PathBuf>,
    opts: crate::mcp::McpServerOptions,
) -> i32 {
    // Аудит-фикс №B1: пустой/короткий --mcp-token (или POLER_MCP_TOKEN="")
    // раньше означал, что ЛЮБОЙ запрос с пустым X-Poler-Token проходит
    // проверку (token_eq("", "") == true). Fail-closed: слабый токен
    // заменяется сгенерированным.
    let generated;
    let token = if token.trim().len() < 16 {
        generated = generate_token();
        eprintln!(
            "poler-mcp-http: заданный токен пуст/короток (<16) — заменён сгенерированным (fail-closed)"
        );
        &generated
    } else {
        token
    };
    // «8765» → «127.0.0.1:8765» (удобство: только порт без хоста)
    let bind = match bind.parse::<u16>() {
        Ok(port) => format!("127.0.0.1:{port}"),
        Err(_) => bind.to_string(),
    };
    let listener = match TcpListener::bind(&bind) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("poler-mcp-http: не удалось занять {bind}: {e}");
            return 2;
        }
    };
    eprintln!("poler-mcp-http: MCP over HTTP (Streamable HTTP), инструменты те же, что у --mcp");
    eprintln!("poler-mcp-http: слушаю http://{bind}   (POST /  или  POST /mcp)");
    eprintln!("poler-mcp-http: TOKEN: {token}");
    eprintln!();
    eprintln!("Удалённый доступ для агента (дай ему эти две строки):");
    eprintln!("  URL:       https://<твой-туннель>/mcp");
    eprintln!("  Заголовок: Authorization: Bearer {token}");
    eprintln!("Публичный туннель без аккаунта (URL напечатает сам):");
    eprintln!("  cloudflared tunnel --url http://{bind}");
    eprintln!("Токен: --mcp-token <T> | env POLER_MCP_TOKEN | сгенерирован выше. Ctrl+C — стоп.");
    let mut server = McpServer::new(cdp_port, wait_ms, db_path);
    if let Some(kdb) = knowledge_db {
        server = server.with_knowledge_db(kdb);
    }
    server = opts.apply(server);
    let server = Arc::new(server);
    serve(listener, server, token)
}

/// Акцептор: поток на соединение, лимит MAX_CONNS.
fn serve(listener: TcpListener, server: Arc<McpServer>, token: &str) -> i32 {
    let token = token.to_string();
    let active = Arc::new(AtomicUsize::new(0));
    for conn in listener.incoming() {
        let Ok(stream) = conn else {
            eprintln!("poler-mcp-http: accept: {}", conn.unwrap_err());
            continue;
        };
        if active.load(Ordering::Relaxed) >= MAX_CONNS {
            let mut w = stream;
            let _ = write_response(
                &mut w,
                503,
                "Service Unavailable",
                "text/plain",
                b"too many connections",
                false,
                &[],
            );
            // Аудит-фикс: доставить 503 до клиента. Раньше close() с
            // непрочитанным телом запроса превращался в RST и ответ
            // терялся. Теперь: полузакрытие записи (FIN) + добор буфера.
            let _ = w.shutdown(std::net::Shutdown::Write);
            let _ = w.set_read_timeout(Some(Duration::from_millis(50)));
            let mut sink = [0u8; 1024];
            while matches!(w.read(&mut sink), Ok(n) if n > 0) {}
            continue;
        }
        active.fetch_add(1, Ordering::Relaxed);
        let server = Arc::clone(&server);
        let token = token.clone();
        let active = Arc::clone(&active);
        thread::spawn(move || {
            handle_conn(stream, &server, &token);
            active.fetch_sub(1, Ordering::Relaxed);
        });
    }
    0
}

// =====================================================================
// Соединение: keep-alive-цикл «прочитай запрос → ответь»
// =====================================================================

fn handle_conn(stream: TcpStream, server: &McpServer, token: &str) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(IDLE));
    let mut reader = match stream.try_clone() {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut writer = stream;
    loop {
        // Аудит-фикс №C2: бюджет на ВЕСЬ запрос, а не на один read()
        let deadline = Instant::now() + REQUEST_DEADLINE;
        let req = match read_request(&mut reader, &mut writer, deadline) {
            Ok(Some(r)) => r,
            // EOF/таймаут/битый запрос — тихо закрываем
            _ => return,
        };
        let keep = req.keep_alive;
        respond(&mut writer, req, server, token);
        if !keep {
            return;
        }
    }
}

struct HttpRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    keep_alive: bool,
}

impl HttpRequest {
    /// Поиск заголовка (имена уже в lowercase).
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Прочитать один HTTP-запрос. Ok(None) — чистое закрытие/битое — оборвать.
fn read_request(
    reader: &mut TcpStream,
    writer: &mut TcpStream,
    deadline: Instant,
) -> std::io::Result<Option<HttpRequest>> {
    let mut buf: Vec<u8> = Vec::with_capacity(2048);
    let mut chunk = [0u8; 4096];
    // ---- заголовки: читаем до \r\n\r\n (или \n\n) ----
    let header_end = loop {
        if let Some(pos) = find_header_end(&buf) {
            break pos;
        }
        if buf.len() > HEADER_CAP {
            let _ = write_response(writer, 431, "Request Header Fields Too Large", "text/plain", b"", false, &[]);
            return Ok(None);
        }
        // Аудит-фикс №C2: общий дедлайн — «капельница» больше не продлевает жизнь запросу
        if Instant::now() >= deadline {
            let _ = write_response(writer, 408, "Request Timeout", "text/plain", b"request deadline exceeded", false, &[]);
            return Ok(None);
        }
        let n = reader.read(&mut chunk)?;
        if n == 0 {
            return Ok(None); // EOF до конца заголовков (между запросами — норма)
        }
        buf.extend_from_slice(&chunk[..n]);
    };

    // ---- разбор стартовой строки + заголовков ----
    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = head.lines();
    let request_line = lines.next().unwrap_or_default().trim().to_string();
    if request_line.is_empty() {
        return Ok(None);
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_ascii_uppercase();
    let target = parts.next().unwrap_or_default().to_string();
    let version = parts.next().unwrap_or("HTTP/1.1").to_string();
    let path = target.split('?').next().unwrap_or("/").to_string();
    let mut headers: Vec<(String, String)> = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    let get = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    // keep-alive: HTTP/1.1 — по умолчанию да, Connection важнее
    let conn = get("connection").map(|v| v.to_ascii_lowercase());
    let keep_alive = match conn.as_deref() {
        Some(c) if c.contains("close") => false,
        Some(c) if c.contains("keep-alive") => true,
        _ => version == "HTTP/1.1",
    };

    // ---- тело ----
    if get("transfer-encoding").is_some() {
        let _ = write_response(writer, 411, "Length Required", "text/plain", b"chunked not supported", keep_alive, &[]);
        return Ok(None);
    }
    let content_length: usize = get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    if content_length > BODY_CAP {
        let _ = write_response(writer, 413, "Payload Too Large", "text/plain", b"body too large", keep_alive, &[]);
        return Ok(None);
    }
    // curl на телах >1 КБ ждёт «100 Continue», без него — висит
    if get("expect")
        .map(|v| v.to_ascii_lowercase().contains("100-continue"))
        .unwrap_or(false)
    {
        writer.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
        writer.flush()?;
    }
    let mut body: Vec<u8> = buf[header_end..].to_vec();
    if body.len() > content_length {
        body.truncate(content_length);
    }
    while body.len() < content_length {
        // Аудит-фикс №C2: дедлайн и на тело (dribble-атака)
        if Instant::now() >= deadline {
            let _ = write_response(writer, 408, "Request Timeout", "text/plain", b"request deadline exceeded", false, &[]);
            return Ok(None);
        }
        let n = reader.read(&mut chunk)?;
        if n == 0 {
            return Ok(None);
        }
        body.extend_from_slice(&chunk[..n]);
    }

    Ok(Some(HttpRequest {
        method,
        path,
        headers,
        body,
        keep_alive,
    }))
}

/// Конец блока заголовков: «\r\n\r\n» или «\n\n».
fn find_header_end(buf: &[u8]) -> Option<usize> {
    if buf.len() >= 4 {
        for i in 0..=buf.len() - 4 {
            if &buf[i..i + 4] == b"\r\n\r\n" {
                return Some(i + 4);
            }
        }
    }
    if buf.len() >= 2 {
        for i in 0..=buf.len() - 2 {
            if &buf[i..i + 2] == b"\n\n" {
                return Some(i + 2);
            }
        }
    }
    None
}

// =====================================================================
// Ответы
// =====================================================================

fn write_response(
    w: &mut impl Write,
    status: u16,
    reason: &str,
    ct: &str,
    body: &[u8],
    keep_alive: bool,
    extra: &[(&str, String)],
) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: {}\r\n",
        body.len(),
        if keep_alive { "keep-alive" } else { "close" },
    );
    // Аудит-фикс №A2: Access-Control-Allow-Origin по умолчанию *, но при
    // заданном POLER_MCP_ALLOWED_ORIGINS эхом отдаётся только совпавший Origin
    // (значение передаётся через `extra` тем же именем заголовка).
    match extra.iter().find(|(k, _)| *k == "Access-Control-Allow-Origin") {
        Some((_, v)) => head.push_str(&format!("Access-Control-Allow-Origin: {v}\r\n")),
        None => head.push_str("Access-Control-Allow-Origin: *\r\n"),
    }
    for (k, v) in extra.iter().filter(|(k, _)| *k != "Access-Control-Allow-Origin") {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    w.write_all(head.as_bytes())?;
    w.write_all(body)?;
    w.flush()
}

/// Аудит-фикс №A2: разрешённый CORS-Origin для этого запроса.
/// None — список не задан (режим по умолчанию: * — туннель/WebLens).
/// Some(origin) — Origin запроса входит в POLER_MCP_ALLOWED_ORIGINS.
/// Some("null") не бывает: несовпавший Origin не получает ACAO вовсе.
fn cors_allow_origin(req: &HttpRequest) -> Option<String> {
    let allow = std::env::var("POLER_MCP_ALLOWED_ORIGINS").ok()?;
    let origin = req.header("origin")?.to_string();
    let list: Vec<String> = allow
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if list.contains(&origin) {
        Some(origin)
    } else {
        None // без ACAO: браузер сам заблокирует чтение ответа
    }
}

/// Маршрутизация запроса → ответ.
fn respond(w: &mut TcpStream, req: HttpRequest, server: &McpServer, token: &str) {
    let is_mcp = req.path == "/" || req.path == "/mcp";
    match req.method.as_str() {
        // CORS-preflight (браузерные MCP-клиенты: WebLens, туннельные агенты).
        // Allow-Headers обязателен: fetch с Authorization/Content-Type
        // проходит preflight только при явном разрешении заголовков.
        "OPTIONS" => {
            // Аудит-фикс №A2: при заданном allowlist — эхо только доверенного Origin
            let mut extra = vec![
                ("Access-Control-Allow-Methods", "POST, GET, OPTIONS".to_string()),
                (
                    "Access-Control-Allow-Headers",
                    "Authorization, Content-Type, X-Poler-Token".to_string(),
                ),
                ("Access-Control-Max-Age", "86400".to_string()),
            ];
            if let Some(o) = cors_allow_origin(&req) {
                extra.push(("Access-Control-Allow-Origin", o));
            }
            let _ = write_response(w, 204, "No Content", "text/plain", b"", req.keep_alive, &extra);
        }
        // smoke-проба туннеля: без токена, без данных
        "GET" if req.path == "/health" => {
            let _ = write_response(w, 200, "OK", "text/plain; charset=utf-8", b"ok", req.keep_alive, &[]);
        }
        // SSE-поток сервера не предлагается (stateless-сервер)
        "GET" if is_mcp => {
            let msg = r#"{"error":"GET не поддерживается; JSON-RPC отправляется POST-ом"}"#;
            let _ = write_response(
                w,
                405,
                "Method Not Allowed",
                "application/json",
                msg.as_bytes(),
                req.keep_alive,
                &[("Allow", "POST, OPTIONS".to_string())],
            );
        }
        "POST" if is_mcp => {
            if !check_auth(&req, token) {
                let body = json!({
                    "jsonrpc": "2.0", "id": null,
                    "error": {"code": -32001,
                        "message": "unauthorized: нужен заголовок Authorization: Bearer <token> (или X-Poler-Token)"}
                })
                .to_string();
                let mut extra = vec![
                    ("WWW-Authenticate", "Bearer realm=\"poler-engine\"".to_string()),
                ];
                if let Some(o) = cors_allow_origin(&req) {
                    extra.push(("Access-Control-Allow-Origin", o));
                }
                let _ = write_response(
                    w,
                    401,
                    "Unauthorized",
                    "application/json",
                    body.as_bytes(),
                    req.keep_alive,
                    &extra,
                );
                return;
            }
            match rpc_handle(server, &req.body) {
                Some(resp) => {
                    let body = serde_json::to_string(&resp).unwrap_or_default();
                    let mut extra: Vec<(&str, String)> = Vec::new();
                    if let Some(o) = cors_allow_origin(&req) {
                        extra.push(("Access-Control-Allow-Origin", o));
                    }
                    let _ = write_response(w, 200, "OK", "application/json", body.as_bytes(), req.keep_alive, &extra);
                }
                // только уведомления — по JSON-RPC-конвенции ответа нет
                None => {
                    let _ = write_response(w, 202, "Accepted", "application/json", b"", req.keep_alive, &[]);
                }
            }
        }
        _ => {
            let _ = write_response(w, 404, "Not Found", "text/plain; charset=utf-8", b"not found", req.keep_alive, &[]);
        }
    }
}

/// Один HTTP-POST → JSON-RPC-ответ (None = только уведомления).
fn rpc_handle(server: &McpServer, body: &[u8]) -> Option<Value> {
    match serde_json::from_slice::<Value>(body) {
        Ok(Value::Array(msgs)) => {
            let rs: Vec<Value> = msgs.iter().filter_map(|m| server.dispatch(m)).collect();
            (!rs.is_empty()).then_some(Value::Array(rs))
        }
        Ok(v) if !v.is_object() => Some(json!({
            "jsonrpc": "2.0", "id": null,
            "error": {"code": -32600, "message": "invalid request: ожидается объект или batch-массив"}
        })),
        Ok(v) => server.dispatch(&v),
        Err(e) => Some(json!({
            "jsonrpc": "2.0", "id": null,
            "error": {"code": -32700, "message": format!("parse error: {e}")}
        })),
    }
}

// =====================================================================
// Токен
// =====================================================================

/// Проверка токена: Authorization: Bearer <T> или X-Poler-Token: <T>.
fn check_auth(req: &HttpRequest, token: &str) -> bool {
    if let Some(a) = req.header("authorization") {
        let t = a
            .strip_prefix("Bearer ")
            .or_else(|| a.strip_prefix("bearer "))
            .map(str::trim);
        if let Some(t) = t {
            return token_eq(t, token);
        }
    }
    if let Some(t) = req.header("x-poler-token") {
        return token_eq(t.trim(), token);
    }
    false
}

/// Сравнение за постоянное время. Аудит-фикс №A3: длина тоже сворачивается
/// в аккумулятор — нет раннего выхода по длине (утечка длины токена).
fn token_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let n = a.len().max(b.len());
    let mut acc = (a.len() ^ b.len()) as u8;
    for i in 0..n {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        acc |= x ^ y;
    }
    acc == 0
}

/// 32 hex-символа из /dev/urandom; fallback — splitmix64 (время+pid+счётчик).
pub fn generate_token() -> String {
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let mut b = [0u8; 16];
        if f.read_exact(&mut b).is_ok() {
            return b.iter().map(|x| format!("{x:02x}")).collect();
        }
    }
    static COUNTER: AtomicU64 = AtomicU64::new(0x5eed_1234);
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let c = COUNTER.fetch_add(0x9E37_79B9, Ordering::Relaxed);
    let mut x = t ^ c.rotate_left(32) ^ (std::process::id() as u64).rotate_left(48);
    let mut out = String::with_capacity(32);
    for _ in 0..2 {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        out.push_str(&format!("{z:016x}"));
    }
    out
}

// =====================================================================
// Тесты: настоящий TCP на 127.0.0.1:0, клиент — std::net::TcpStream
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn start_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let server = Arc::new(McpServer::new(9222, 10, PathBuf::from("/nonexistent-poler-test.db")));
        let token = "unit-test-token".to_string();
        thread::spawn(move || serve(listener, server, &token));
        addr
    }

    /// Один запрос → весь ответ (Connection: close, сервер закрывает).
    fn http(addr: &str, raw: &str) -> String {
        let mut s = TcpStream::connect(addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        s.write_all(raw.as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    const AUTH: &str = "Authorization: Bearer unit-test-token";

    #[test]
    fn health_without_auth() {
        let addr = start_server();
        let resp = http(&addr, "GET /health HTTP/1.1\r\nHost: t\r\nConnection: close\r\n\r\n");
        assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
        assert!(resp.ends_with("ok"), "{resp}");
    }

    #[test]
    fn post_without_token_is_401() {
        let addr = start_server();
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let resp = http(
            &addr,
            &format!("POST /mcp HTTP/1.1\r\nHost: t\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()),
        );
        assert!(resp.starts_with("HTTP/1.1 401"), "{resp}");
        assert!(resp.contains("unauthorized"), "{resp}");
        assert!(resp.contains("WWW-Authenticate"), "{resp}");
    }

    #[test]
    fn post_with_wrong_token_is_401() {
        let addr = start_server();
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let resp = http(
            &addr,
            &format!("POST / HTTP/1.1\r\nHost: t\r\nAuthorization: Bearer wrong\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()),
        );
        assert!(resp.starts_with("HTTP/1.1 401"), "{resp}");
    }

    #[test]
    fn auth_via_x_poler_token_header() {
        let addr = start_server();
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let resp = http(
            &addr,
            &format!("POST /mcp HTTP/1.1\r\nHost: t\r\nX-Poler-Token: unit-test-token\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()),
        );
        assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
        assert!(resp.contains(r#""result":{}"#), "{resp}");
    }

    #[test]
    fn initialize_with_token() {
        let addr = start_server();
        let body = r#"{"jsonrpc":"2.0","id":7,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#;
        let resp = http(
            &addr,
            &format!("POST /mcp HTTP/1.1\r\nHost: t\r\n{AUTH}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()),
        );
        assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
        assert!(resp.contains("poler-engine"), "{resp}");
        assert!(resp.contains("serverInfo"), "{resp}");
        assert!(resp.contains("2025-03-26"), "{resp}");
    }

    #[test]
    fn tools_list_contains_grep_and_fetch() {
        let addr = start_server();
        let body = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
        let resp = http(
            &addr,
            &format!("POST / HTTP/1.1\r\nHost: t\r\n{AUTH}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()),
        );
        assert!(resp.contains("poler_grep"), "{resp}");
        assert!(resp.contains("poler_fetch"), "{resp}");
        assert!(resp.contains("poler_web_search"), "{resp}");
        assert!(!resp.contains("poler_nlm"), "v2.0: NLM-инструмент удалён");
        assert!(!resp.contains("poler_gmail"), "v2.0: Gmail-инструмент удалён");
        assert!(!resp.contains("poler_drive"), "v2.0: Drive-инструмент удалён");
    }

    #[test]
    fn tools_call_roundtrip() {
        let addr = start_server();
        let body = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"poler_grep","arguments":{"pattern":"nonexistent-pattern-xyz","path":"."}}}"#;
        let resp = http(
            &addr,
            &format!("POST /mcp HTTP/1.1\r\nHost: t\r\n{AUTH}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()),
        );
        // локальный grep выполняется без внешних зависимостей — канал работает
        assert!(resp.contains(r#""isError":false"#), "{resp}");
        assert!(resp.contains(r#""content":"#), "{resp}");
    }

    #[test]
    fn batch_with_notification_returns_one_response() {
        let addr = start_server();
        let body = r#"[{"jsonrpc":"2.0","method":"notifications/initialized"},{"jsonrpc":"2.0","id":9,"method":"ping"}]"#;
        let resp = http(
            &addr,
            &format!("POST /mcp HTTP/1.1\r\nHost: t\r\n{AUTH}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()),
        );
        assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
        assert!(resp.contains(r#""id":9"#), "{resp}");
        assert!(resp.contains(r#""result":{}"#), "{resp}");
        // уведомление осталось без ответа: ровно один объект в теле
        let body_part = resp.split("\r\n\r\n").nth(1).unwrap_or("");
        assert_eq!(body_part.matches(r#""jsonrpc":"2.0""#).count(), 1, "{resp}");
    }

    #[test]
    fn pure_notification_is_202() {
        let addr = start_server();
        let body = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let resp = http(
            &addr,
            &format!("POST /mcp HTTP/1.1\r\nHost: t\r\n{AUTH}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()),
        );
        assert!(resp.starts_with("HTTP/1.1 202"), "{resp}");
    }

    #[test]
    fn parse_error_is_32700() {
        let addr = start_server();
        let resp = http(
            &addr,
            &format!("POST /mcp HTTP/1.1\r\nHost: t\r\n{AUTH}\r\nContent-Length: 7\r\nConnection: close\r\n\r\nnot-jso"),
        );
        assert!(resp.contains("-32700"), "{resp}");
    }

    #[test]
    fn non_object_request_is_32600() {
        let addr = start_server();
        let resp = http(
            &addr,
            &format!("POST /mcp HTTP/1.1\r\nHost: t\r\n{AUTH}\r\nContent-Length: 1\r\nConnection: close\r\n\r\n5"),
        );
        assert!(resp.contains("-32600"), "{resp}");
    }

    #[test]
    fn unknown_method_is_32601() {
        let addr = start_server();
        let body = r#"{"jsonrpc":"2.0","id":4,"method":"no/such"}"#;
        let resp = http(
            &addr,
            &format!("POST /mcp HTTP/1.1\r\nHost: t\r\n{AUTH}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()),
        );
        assert!(resp.contains("-32601"), "{resp}");
    }

    #[test]
    fn unknown_path_is_404() {
        let addr = start_server();
        let resp = http(
            &addr,
            &format!("POST /other HTTP/1.1\r\nHost: t\r\n{AUTH}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"),
        );
        assert!(resp.starts_with("HTTP/1.1 404"), "{resp}");
    }

    #[test]
    fn get_mcp_is_405() {
        let addr = start_server();
        let resp = http(&addr, "GET /mcp HTTP/1.1\r\nHost: t\r\nConnection: close\r\n\r\n");
        assert!(resp.starts_with("HTTP/1.1 405"), "{resp}");
        assert!(resp.contains("Allow: POST"), "{resp}");
    }

    #[test]
    fn options_preflight_is_204() {
        let addr = start_server();
        let resp = http(&addr, "OPTIONS /mcp HTTP/1.1\r\nHost: t\r\nConnection: close\r\n\r\n");
        assert!(resp.starts_with("HTTP/1.1 204"), "{resp}");
    }

    #[test]
    fn keep_alive_two_requests_one_connection() {
        let addr = start_server();
        let mut s = TcpStream::connect(&addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        let one = |s: &mut TcpStream, id: i32| {
            let body = format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"ping"}}"#);
            let req = format!("POST /mcp HTTP/1.1\r\nHost: t\r\n{AUTH}\r\nContent-Length: {}\r\n\r\n{body}", body.len());
            s.write_all(req.as_bytes()).unwrap();
            // читаем ровно один ответ: по Content-Length
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            loop {
                s.read_exact(&mut byte).unwrap();
                head.extend_from_slice(&byte);
                if head.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let head_s = String::from_utf8_lossy(&head).into_owned();
            let len: usize = head_s
                .lines()
                .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
                .and_then(|l| l.split(':').nth(1))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            let mut body_buf = vec![0u8; len];
            s.read_exact(&mut body_buf).unwrap();
            (head_s, String::from_utf8_lossy(&body_buf).into_owned())
        };
        let (h1, b1) = one(&mut s, 1);
        assert!(h1.starts_with("HTTP/1.1 200"), "{h1}");
        assert!(b1.contains(r#""id":1"#), "{b1}");
        let (h2, b2) = one(&mut s, 2);
        assert!(h2.starts_with("HTTP/1.1 200"), "{h2}");
        assert!(b2.contains(r#""id":2"#), "{b2}");
    }

    // ---- юнит-уровень ----

    #[test]
    fn token_eq_cases() {
        assert!(token_eq("abc", "abc"));
        assert!(!token_eq("abc", "abd"));
        assert!(!token_eq("abc", "abcd"));
        assert!(!token_eq("", "x"));
        assert!(token_eq("", ""));
    }

    #[test]
    fn generate_token_format() {
        let t = generate_token();
        assert_eq!(t.len(), 32, "{t}");
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()), "{t}");
        assert_ne!(generate_token(), generate_token());
    }

    #[test]
    fn find_header_end_crlf_and_lf() {
        // «GET / HTTP/1.1» (14) + CRLF (2) + «A: 1» (4) + CRLF (2) + CRLF (2) = 24
        assert_eq!(find_header_end(b"GET / HTTP/1.1\r\nA: 1\r\n\r\nbody"), Some(24));
        // LF-вариант: 14 + 1 + 4 + 1 + 1 = 21
        assert_eq!(find_header_end(b"GET / HTTP/1.1\nA: 1\n\nbody"), Some(21));
        assert_eq!(find_header_end(b"GET / HTTP/1.1\r\nA: 1"), None);
    }

    #[test]
    fn bind_shorthand_port_only() {
        // «8765» разворачивается в «127.0.0.1:8765» — проверяем разбор
        // (bind сам по себе тестируется E2E-скриптом запуска бинарника)
        let port: u16 = "8765".parse().unwrap();
        assert_eq!(format!("127.0.0.1:{port}"), "127.0.0.1:8765");
    }
}
