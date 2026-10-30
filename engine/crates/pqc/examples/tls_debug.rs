#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Отладка TLS-рукопожатия против живой Википедии (не входит в релиз).

use std::time::Duration;
use pqc::tls13::TlsConnection;

fn main() {
    let host = std::env::args().nth(1).unwrap_or_else(|| "ru.wikipedia.org".into());
    eprintln!("[{host}] подключение…");
    let mut conn = match TlsConnection::connect(&host, 443, Duration::from_secs(15)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[{host}] рукопожатие ПРОВАЛЕНО: {e}");
            return;
        }
    };
    eprintln!("[{host}] рукопожатие OK — пишу GET");
    let req = format!(
        "GET /wiki/Rust HTTP/1.1\r\nHost: {host}\r\nUser-Agent: POLER-Quantum/0.9.0 (https://github.com/Kotokvit/POLER-Quantum-RS)\r\nAccept: application/json\r\nAccept-Encoding: identity\r\nConnection: close\r\n\r\n"
    );
    match conn.write(req.as_bytes()) {
        Ok(()) => eprintln!("[{host}] запись OK ({} Б)", req.len()),
        Err(e) => {
            eprintln!("[{host}] запись ПРОВАЛЕНА: {e}");
            return;
        }
    }
    let mut total = 0usize;
    let mut chunk = [0u8; 8192];
    loop {
        match conn.read(&mut chunk) {
            Ok(0) => {
                eprintln!("[{host}] EOF после {total} Б");
                break;
            }
            Ok(n) => {
                total += n;
                if total <= 200 {
                    eprintln!("[{host}] первые байты: {:?}", &chunk[..n.min(60)]);
                }
            }
            Err(e) => {
                eprintln!("[{host}] чтение ПРОВАЛЕНО после {total} Б: {e}");
                break;
            }
        }
    }
    if total > 0 {
        eprintln!("[{host}] всего {total} Б — ТРАНСПОРТ РАБОТАЕТ");
    }
}
