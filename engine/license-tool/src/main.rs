//! poler-license-tool — офлайн-инструмент ВЛАДЕЛЬЦА POLER Engine.
//!
//! Живёт в приватном репо и НЕ попадает в поставку бинарника движка.
//! Две операции:
//!   keygen  — создать ключевую пару ed25519 (приватный seed → файл 0600,
//!             публичный ключ → stdout, его вшиваем в src/license/mod.rs)
//!   issue   — выпустить лицензию PO1 для покупателя
//!
//! ПРИВАТНЫЙ КЛЮЧ НИКОГДА не покидает машину владельца. Движок знает
//! только публичный ключ и умеет только ПРОВЕРЯТЬ подписи.
//!
//! Формат ключа лицензии (одна строка, удобно для письма покупателю):
//!   PO1.<base64url(payload JSON)>.<base64url(подпись ed25519 64 байта)>
//!
//! Подпись считается по ТОЧНЫМ байтам payload (serde_json выдаёт
//! стабильный порядок полей структуры), поэтому движку не важен
//! порядок полей — он проверяет подпись по сырым байтам.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("keygen") => cmd_keygen(&args[2..]),
        Some("issue") => cmd_issue(&args[2..]),
        _ => {
            eprintln!("poler-license-tool — офлайн-выпуск лицензий POLER Engine");
            eprintln!();
            eprintln!("  keygen --out <PRIVATE_KEY_FILE>");
            eprintln!("      Создать ключевую пару. Приватный seed пишется в файл (0600),");
            eprintln!("      публичный ключ (hex) печатается — вшить в src/license/mod.rs.");
            eprintln!();
            eprintln!("  issue --key <PRIVATE_KEY_FILE> --name <ИМЯ> --email <EMAIL>");
            eprintln!("         --tier <pro|enterprise|community> --days <N> [--features a,b,c]");
            eprintln!("      Выпустить лицензию. Печатает PO1-ключ и дату окончания.");
            eprintln!("      --days 0 = бессрочная (только для enterprise).");
            ExitCode::from(2)
        }
    }
}

// ---------- base64url (без паддинга, алфавит RFC 4648 §5) ----------

const B64URL_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn b64url_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64URL_ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(B64URL_ALPHABET[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(B64URL_ALPHABET[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(B64URL_ALPHABET[n as usize & 63] as char);
        }
    }
    out
}

pub fn b64url_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4 + 3);
    let mut buf: u32 = 0;
    let mut bits = 0u32;
    for (i, ch) in s.chars().enumerate() {
        let v = B64URL_ALPHABET
            .iter()
            .position(|&c| c as char == ch)
            .ok_or_else(|| format!("недопустимый символ base64url на позиции {i}: {ch:?}"))?
            as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

// ---------- утилиты ----------

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn read_seed(path: &Path) -> Result<[u8; 32], String> {
    let hex = fs::read_to_string(path)
        .map_err(|e| format!("не могу прочитать приватный ключ {}: {e}", path.display()))?;
    let hex = hex.trim();
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "файл {} не выглядит как hex-seed ed25519 (ожидалось 64 hex-символа)",
            path.display()
        ));
    }
    let mut seed = [0u8; 32];
    for i in 0..32 {
        seed[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|e| format!("hex-seed повреждён: {e}"))?;
    }
    Ok(seed)
}

/// Случайные 32 байта из ОС (/dev/urandom — криптографический источник Linux).
fn os_random_32() -> Result<[u8; 32], String> {
    let mut f = fs::File::open("/dev/urandom").map_err(|e| format!("/dev/urandom: {e}"))?;
    let mut buf = [0u8; 32];
    f.read_exact(&mut buf).map_err(|e| format!("чтение /dev/urandom: {e}"))?;
    Ok(buf)
}

fn json_escape(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"?\"".to_string())
}

// ---------- keygen ----------

fn cmd_keygen(args: &[String]) -> ExitCode {
    let out = match flag_str(args, "--out") {
        Some(v) => v,
        None => {
            eprintln!("keygen: обязателен --out <PRIVATE_KEY_FILE>");
            return ExitCode::from(2);
        }
    };
    let seed = match os_random_32() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("keygen: {e}");
            return ExitCode::from(2);
        }
    };
    let sk = SigningKey::from_bytes(&seed);
    let pub_hex = hex(&sk.verifying_key().to_bytes());
    let seed_hex = hex(&seed);

    if let Err(e) = write_private(&out, &seed_hex) {
        eprintln!("keygen: {e}");
        return ExitCode::from(2);
    }

    println!("Приватный seed сохранён: {out} (0600) — ХРАНИ ОФЛАЙН, НЕ КОММИТИТЬ");
    println!("Публичный ключ (hex, 32 байта):");
    println!("{pub_hex}");
    println!();
    println!("Вшить в движок: src/license/mod.rs → POLER_LICENSE_PUBLIC_KEY");
    ExitCode::SUCCESS
}

fn write_private(path: &str, seed_hex: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    if Path::new(path).exists() {
        return Err(format!(
            "файл {path} уже существует — не перезаписываю (сперва удали вручную)"
        ));
    }
    let mut f = fs::File::create(path).map_err(|e| format!("создание {path}: {e}"))?;
    f.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("chmod 600 {path}: {e}"))?;
    writeln!(f, "{seed_hex}").map_err(|e| format!("запись {path}: {e}"))?;
    Ok(())
}

// ---------- issue ----------

fn cmd_issue(args: &[String]) -> ExitCode {
    let key_path = match flag_str(args, "--key") {
        Some(v) => v,
        None => {
            eprintln!("issue: обязателен --key <PRIVATE_KEY_FILE>");
            return ExitCode::from(2);
        }
    };
    let name = match flag_str(args, "--name") {
        Some(v) => v,
        None => {
            eprintln!("issue: обязателен --name <ИМЯ ПОКУПАТЕЛЯ>");
            return ExitCode::from(2);
        }
    };
    let email = match flag_str(args, "--email") {
        Some(v) => v,
        None => {
            eprintln!("issue: обязателен --email <EMAIL ПОКУПАТЕЛЯ>");
            return ExitCode::from(2);
        }
    };
    let tier = flag_str(args, "--tier").unwrap_or_else(|| "pro".to_string());
    if !matches!(tier.as_str(), "pro" | "enterprise" | "community") {
        eprintln!("issue: --tier ожидает pro | enterprise | community, получено: {tier}");
        return ExitCode::from(2);
    }
    let days: u64 = match flag_str(args, "--days") {
        Some(v) => match v.parse() {
            Ok(d) => d,
            Err(_) => {
                eprintln!("issue: --days ожидает число дней, получено: {v}");
                return ExitCode::from(2);
            }
        },
        None => 365,
    };
    if days == 0 && tier != "enterprise" {
        eprintln!("issue: бессрочная лицензия (--days 0) разрешена только для enterprise");
        return ExitCode::from(2);
    }
    let features = flag_str(args, "--features").unwrap_or_else(|| "gmail,drive,nlm".to_string());

    let seed = match read_seed(Path::new(&key_path)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("issue: {e}");
            return ExitCode::from(2);
        }
    };
    let sk = SigningKey::from_bytes(&seed);

    let issued = now_unix();
    let expires = if days == 0 { 0 } else { issued + days * 86400 };

    // Канонический payload: поля в фиксированном порядке (serde_json
    // сериализует структуру в порядке объявления полей).
    let payload = format!(
        "{{\"product\":\"poler-engine\",\"name\":{},\"email\":{},\"tier\":\"{}\",\"issued\":{},\"expires\":{},\"features\":[{}]}}",
        json_escape(&name),
        json_escape(&email),
        tier,
        issued,
        expires,
        features
            .split(',')
            .map(|f| json_escape(f.trim()))
            .collect::<Vec<_>>()
            .join(",")
    );

    let sig: Signature = sk.sign(payload.as_bytes());
    let license_key = format!(
        "PO1.{}.{}",
        b64url_encode(payload.as_bytes()),
        b64url_encode(&sig.to_bytes())
    );

    // Самопроверка перед выдачей (защита от кривого ключа/сериализации).
    let vk = VerifyingKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    if vk.verify(payload.as_bytes(), &sig).is_err() {
        eprintln!("issue: ВНУТРЕННЯЯ ОШИБКА — самоподпись не прошла проверку, ключ НЕ выдан");
        return ExitCode::from(2);
    }

    println!("ЛИЦЕНЗИЯ ВЫДАНА");
    println!("  кому:      {name} <{email}>");
    println!("  тир:       {tier}");
    if expires == 0 {
        println!("  срок:      бессрочно (enterprise)");
    } else {
        println!("  действует до: {} ({} дней)", civil_date(expires), days);
    }
    println!("  функции:   {features}");
    println!();
    println!("Ключ активации (передать покупателю одной строкой):");
    println!("{license_key}");
    ExitCode::SUCCESS
}

/// unix-секунды → YYYY-MM-DD (UTC), алгоритм Говарда Хиннанта.
fn civil_date(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn flag_str(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}
