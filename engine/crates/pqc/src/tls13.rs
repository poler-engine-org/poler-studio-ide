//! RQ19: TLS 1.3-клиент (RFC 8446) на чистом `std` — **zero-dep**.
//!
//! Транспорт интернета знаний: один комплект шифров
//! `TLS_CHACHA20_POLY1305_SHA256` (X25519 + ChaCha20-Poly1305 +
//! SHA-256 — все примитивы в [`crate::tlsprim`]), 1-RTT рукопожатие,
//! TCP через [`std::net::TcpStream`]. Ни одной внешней зависимости —
//! как у всего POLER.
//!
//! # Что поддерживается
//!
//! * ClientHello: SNI, supported_versions, key_share X25519,
//!   signature_algorithms, ALPN `http/1.1` (сервер не выберет h2);
//! * полный key schedule §7.1: early → handshake → master →
//!   traffic secrets + `HKDF-Expand-Label` для key/iv/finished;
//! * защищённый слой записей §5.2: nonce = iv ⊕ seq, AAD = 5-байтный
//!   заголовок, padding-разбор внутреннего plaintext;
//! * серверный Finished проверяется HMAC-ом по транскрипту;
//! * KeyUpdate (§4.6.3) и NewSessionTicket пропускаются корректно;
//! * close_notify — вежливое закрытие.
//!
//! # Честные границы
//!
//! * **Цепочка сертификатов не проверяется**: в zero-dep мире нет
//!   корневого хранилища. Канал шифруется против пассивного
//!   прослушивания, но сервер не аутентифицирован — активный MITM
//!   может подсунуть свой ключ. Для ингеста публичных знаний
//!   (Википедия) принимаем; для секретов — никогда.
//! * Один шифр-набор: если сервер не знает X25519+ChaCha20-Poly1305
//!   (в 2026-м так делают единицы), рукопожатие честно откажется.
//! * HelloRetryRequest не поддерживается: с X25519 в key_share он
//!   не встречается на практике.
//! * Сжатие, PSK, 0-RTT, session tickets на клиенте — нет (не нужны
//!   для точечных GET-запросов).
//!
//! # Проверка корректности
//!
//! Key schedule и finished-вычисления зафиксированы эталонной трассой
//! RFC 8448 §3 (полный 1-RTT): транскрипт-хэши, все секреты и
//! verify_data сверены байт-в-байт. Примитивы — векторами RFC 7748/8439
//! (см. [`crate::tlsprim`]). Живое рукопожатие — `#[ignore]`-тест
//! `live_handshake_wikipedia`.

use std::fmt;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::tlsprim::{
    aead_chacha20poly1305_open, aead_chacha20poly1305_seal, derive_secret, hkdf_expand_label,
    hkdf_extract, hmac_sha256, x25519, x25519_base, Sha256,
};

/// Идентификатор записи «application_data» (внешний opaque-тип).
const RT_APP_DATA: u8 = 23;
/// Идентификатор записи «handshake».
const RT_HANDSHAKE: u8 = 22;
/// Идентификатор записи «alert».
const RT_ALERT: u8 = 21;
/// ChangeCipherSpec (middlebox compat) — пропускается.
const RT_CCS: u8 = 20;

/// Типы handshake-сообщений.
const MT_CLIENT_HELLO: u8 = 1;
const MT_SERVER_HELLO: u8 = 2;
const MT_NEW_SESSION_TICKET: u8 = 4;
const MT_ENCRYPTED_EXTENSIONS: u8 = 8;
const MT_CERTIFICATE: u8 = 11;
const MT_CERTIFICATE_VERIFY: u8 = 15;
const MT_FINISHED: u8 = 20;
const MT_KEY_UPDATE: u8 = 24;

/// Единственный поддерживаемый шифр-набор.
const CIPHER_CHACHA: u16 = 0x1303;
/// Группа X25519.
const GROUP_X25519: u16 = 0x001d;

/// Магический random HelloRetryRequest (RFC 8446 §4.1.3).
const HRR_RANDOM: [u8; 32] = [
    0xcf, 0x21, 0xad, 0x74, 0xe5, 0x9a, 0x61, 0x11, 0xbe, 0x1d, 0x8c, 0x02, 0x1e, 0x65, 0xb8,
    0x91, 0xc2, 0xa2, 0x11, 0x16, 0x7a, 0xbb, 0x8c, 0x5e, 0x07, 0x9e, 0x09, 0xe2, 0xc8, 0xa8,
    0x33, 0x9c,
];

/// Верхний предел размера записи (plaintext ≤ 2^14 + tag).
const MAX_RECORD: usize = 1 << 16;

/// Ошибки TLS-транспорта.
#[derive(Debug)]
pub enum TlsError {
    /// Сетевая ошибка / таймаут.
    Io(std::io::Error),
    /// Сервер прислал alert (level, description).
    Alert { level: u8, desc: u8 },
    /// Тег AEAD не сошёлся или запись короче тега.
    DecryptFailed,
    /// Протокольное нарушение со стороны сервера.
    BadMessage(&'static str),
    /// Обрыв до close_notify (данные могли быть усечены).
    UnexpectedEof,
}

impl fmt::Display for TlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TlsError::Io(e) => write!(f, "сеть: {e}"),
            TlsError::Alert { level, desc } => {
                write!(f, "сервер прислал alert: {}", alert_text(*desc, *level))
            }
            TlsError::DecryptFailed => write!(f, "AEAD: тег записи не сошёлся"),
            TlsError::BadMessage(what) => write!(f, "протокол TLS: {what}"),
            TlsError::UnexpectedEof => write!(f, "соединение оборвано без close_notify"),
        }
    }
}

impl std::error::Error for TlsError {}

impl From<std::io::Error> for TlsError {
    fn from(e: std::io::Error) -> Self {
        TlsError::Io(e)
    }
}

/// Человекочитаемое имя alert-кода (RFC 8446 §6.2).
fn alert_text(desc: u8, level: u8) -> String {
    let name = match desc {
        0 => "close_notify",
        10 => "unexpected_message",
        40 => "handshake_failure",
        42 => "bad_certificate",
        43 => "unsupported_certificate",
        44 => "certificate_revoked",
        45 => "certificate_expired",
        46 => "certificate_unknown",
        47 => "illegal_parameter",
        48 => "unknown_ca",
        49 => "access_denied",
        50 => "decode_error",
        51 => "decrypt_error",
        70 => "protocol_version",
        80 => "internal_error",
        109 => "missing_extension",
        112 => "too_many_certs",
        120 => "no_application_protocol",
        _ => "unknown",
    };
    format!("{name} ({desc}, level {level})")
}

// ============================================================================
// Энтропия
// ============================================================================

/// 32 байта энтропии: `/dev/urandom` через [`crate::Rng::from_entropy`].
fn entropy32() -> [u8; 32] {
    let mut rng = crate::Rng::from_entropy();
    let mut buf = [0u8; 32];
    for chunk in buf.chunks_mut(8) {
        chunk.copy_from_slice(&rng.next_u64().to_le_bytes());
    }
    buf
}

// ============================================================================
// Построение ClientHello
// ============================================================================

/// Extension: server_name (0) — только hostname (без порта).
fn ext_server_name(host: &str) -> Vec<u8> {
    let mut body = Vec::with_capacity(host.len() + 9);
    body.extend_from_slice(&(host.len() as u16 + 3).to_be_bytes()); // server_name_list
    body.push(0); // type: host_name
    body.extend_from_slice(&(host.len() as u16).to_be_bytes());
    body.extend_from_slice(host.as_bytes());
    let mut ext = Vec::with_capacity(body.len() + 4);
    ext.extend_from_slice(&0u16.to_be_bytes());
    ext.extend_from_slice(&(body.len() as u16).to_be_bytes());
    ext.extend_from_slice(&body);
    ext
}

/// Extension: supported_groups (10) — только X25519.
fn ext_supported_groups() -> Vec<u8> {
    let mut ext = Vec::with_capacity(10);
    ext.extend_from_slice(&10u16.to_be_bytes());
    ext.extend_from_slice(&4u16.to_be_bytes()); // длина тела
    ext.extend_from_slice(&2u16.to_be_bytes()); // длина списка
    ext.extend_from_slice(&GROUP_X25519.to_be_bytes());
    ext
}

/// Extension: key_share (51) — публичный ключ X25519.
fn ext_key_share(pubkey: &[u8; 32]) -> Vec<u8> {
    let mut body = Vec::with_capacity(40);
    body.extend_from_slice(&36u16.to_be_bytes()); // client_shares list
    body.extend_from_slice(&GROUP_X25519.to_be_bytes());
    body.extend_from_slice(&32u16.to_be_bytes());
    body.extend_from_slice(pubkey);
    let mut ext = Vec::with_capacity(body.len() + 4);
    ext.extend_from_slice(&51u16.to_be_bytes());
    ext.extend_from_slice(&(body.len() as u16).to_be_bytes());
    ext.extend_from_slice(&body);
    ext
}

/// Extension: signature_algorithms (13) — стандартный минимум RSA/ECDSA.
fn ext_signature_algorithms() -> Vec<u8> {
    let sigs: [u16; 3] = [
        0x0804, // rsa_pss_rsae_sha256
        0x0403, // ecdsa_secp256r1_sha256
        0x0401, // rsa_pkcs1_sha256
    ];
    let mut body = Vec::with_capacity(2 + sigs.len() * 2);
    body.extend_from_slice(&((sigs.len() * 2) as u16).to_be_bytes());
    for s in sigs {
        body.extend_from_slice(&s.to_be_bytes());
    }
    let mut ext = Vec::with_capacity(body.len() + 4);
    ext.extend_from_slice(&13u16.to_be_bytes());
    ext.extend_from_slice(&(body.len() as u16).to_be_bytes());
    ext.extend_from_slice(&body);
    ext
}

/// Extension: supported_versions (43) — TLS 1.3.
fn ext_supported_versions() -> Vec<u8> {
    let mut ext = Vec::with_capacity(9);
    ext.extend_from_slice(&43u16.to_be_bytes());
    ext.extend_from_slice(&3u16.to_be_bytes());
    ext.push(2); // список версий длиной 2
    ext.extend_from_slice(&0x0304u16.to_be_bytes());
    ext
}

/// Extension: ALPN (16) — только http/1.1.
fn ext_alpn_http11() -> Vec<u8> {
    let proto = b"http/1.1";
    let mut body = Vec::with_capacity(4 + proto.len());
    body.extend_from_slice(&((proto.len() + 1) as u16).to_be_bytes()); // protocol list
    body.push(proto.len() as u8);
    body.extend_from_slice(proto);
    let mut ext = Vec::with_capacity(body.len() + 4);
    ext.extend_from_slice(&16u16.to_be_bytes());
    ext.extend_from_slice(&(body.len() as u16).to_be_bytes());
    ext.extend_from_slice(&body);
    ext
}

/// Сборка ClientHello (без заголовка записи). Возвращает байты сообщения
/// и случайные session_id/random (для воспроизводимости в тестах).
fn build_client_hello(host: &str, random: &[u8; 32], session_id: &[u8; 32], pubkey: &[u8; 32]) -> Vec<u8> {
    let mut exts = Vec::with_capacity(128);
    exts.extend_from_slice(&ext_server_name(host));
    exts.extend_from_slice(&ext_supported_groups());
    exts.extend_from_slice(&ext_key_share(pubkey));
    exts.extend_from_slice(&ext_signature_algorithms());
    exts.extend_from_slice(&ext_supported_versions());
    exts.extend_from_slice(&ext_alpn_http11());

    let mut m = Vec::with_capacity(64 + host.len() + exts.len());
    m.push(MT_CLIENT_HELLO);
    // длина тела посчитается ниже — вставим после сборки
    let body_start = m.len();
    m.extend_from_slice(&0u32.to_be_bytes()[1..]); // placeholder длины
    m.extend_from_slice(&0x0303u16.to_be_bytes()); // legacy_version
    m.extend_from_slice(random);
    m.push(session_id.len() as u8);
    m.extend_from_slice(session_id);
    m.extend_from_slice(&2u16.to_be_bytes()); // список шифров
    m.extend_from_slice(&CIPHER_CHACHA.to_be_bytes());
    m.push(1); // legacy_compression_methods
    m.push(0); // null
    m.extend_from_slice(&(exts.len() as u16).to_be_bytes());
    m.extend_from_slice(&exts);
    let body_len = m.len() - body_start - 3;
    m[body_start..body_start + 3].copy_from_slice(&(body_len as u32).to_be_bytes()[1..]);
    m
}

// ============================================================================
// Разбор ServerHello
// ============================================================================

/// Результат разбора ServerHello.
#[derive(Debug)]
struct ServerHello {
    server_pub: [u8; 32],
}

/// Разбор ServerHello (RFC 8446 §4.1.3). Строгий: лишнее — ошибка.
fn parse_server_hello(msg: &[u8]) -> Result<ServerHello, TlsError> {
    let bad = |w: &'static str| Err(TlsError::BadMessage(w));
    if msg.len() < 2 + 32 + 1 + 2 + 1 {
        return bad("ServerHello короче минимума");
    }
    let mut p = 0usize;
    p += 2; // legacy_version
    let mut random = [0u8; 32];
    random.copy_from_slice(&msg[p..p + 32]);
    p += 32;
    if random == HRR_RANDOM {
        return bad("HelloRetryRequest не поддерживается (X25519 должен приниматься сразу)");
    }
    let sid_len = msg[p] as usize;
    p += 1 + sid_len;
    if msg.len() < p + 2 + 1 + 2 {
        return bad("ServerHello: обрыв после session_id");
    }
    let cipher = u16::from_be_bytes([msg[p], msg[p + 1]]);
    p += 2;
    if cipher != CIPHER_CHACHA {
        return bad("сервер выбрал незнакомый шифр (нужен ChaCha20-Poly1305)");
    }
    if msg[p] != 0 {
        return bad("ServerHello: compression != 0");
    }
    p += 1;
    let exts_len = u16::from_be_bytes([msg[p], msg[p + 1]]) as usize;
    p += 2;
    if msg.len() != p + exts_len {
        return bad("ServerHello: несходимость длины расширений");
    }
    let mut server_pub = None;
    let mut version_ok = false;
    while p < msg.len() {
        if msg.len() < p + 4 {
            return bad("ServerHello: обрыв заголовка расширения");
        }
        let etype = u16::from_be_bytes([msg[p], msg[p + 1]]);
        let elen = u16::from_be_bytes([msg[p + 2], msg[p + 3]]) as usize;
        p += 4;
        if msg.len() < p + elen {
            return bad("ServerHello: расширение длиннее сообщения");
        }
        let body = &msg[p..p + elen];
        p += elen;
        match etype {
            43 => {
                // supported_versions в SH: только выбранная версия (2 байта,
                // без длины списка — она бывает только в CH).
                if body.len() == 2 && body[..] == [0x03, 0x04] {
                    version_ok = true;
                } else {
                    return bad("сервер не выбрал TLS 1.3");
                }
            }
            51 => {
                // key_share: группа + u16 длина + ключ
                if body.len() != 2 + 2 + 32 {
                    return bad("ServerHello: key_share неожиданной длины");
                }
                let group = u16::from_be_bytes([body[0], body[1]]);
                let klen = u16::from_be_bytes([body[2], body[3]]);
                if group != GROUP_X25519 || klen != 32 {
                    return bad("ServerHello: key_share не X25519");
                }
                let mut pk = [0u8; 32];
                pk.copy_from_slice(&body[4..]);
                server_pub = Some(pk);
            }
            _ => {} // прочие расширения игнорируем
        }
    }
    let server_pub = server_pub.ok_or(TlsError::BadMessage("ServerHello: нет key_share"))?;
    if !version_ok {
        return bad("ServerHello: нет supported_versions (TLS 1.3)");
    }
    Ok(ServerHello { server_pub })
}

// ============================================================================
// Key schedule (RFC 8446 §7.1)
// ============================================================================

/// Трафик-ключи направления: key (32) + iv (12) + счётчик записей.
#[derive(Clone)]
struct TrafficKeys {
    key: [u8; 32],
    iv: [u8; 12],
    seq: u64,
}

impl TrafficKeys {
    fn from_secret(secret: &[u8]) -> TrafficKeys {
        let key_vec = hkdf_expand_label(secret, "key", &[], 32);
        let iv_vec = hkdf_expand_label(secret, "iv", &[], 12);
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_vec);
        let mut iv = [0u8; 12];
        iv.copy_from_slice(&iv_vec);
        TrafficKeys { key, iv, seq: 0 }
    }

    /// Нонс записи: iv ⊕ seq (seq — BE64 в младших 8 байтах).
    fn nonce(&self) -> [u8; 12] {
        let mut n = self.iv;
        let seq = self.seq.to_be_bytes();
        for i in 0..8 {
            n[4 + i] ^= seq[i];
        }
        n
    }

}

/// Следующий s ap traffic secret после KeyUpdate (RFC 8446 §7.2):
/// HKDF-Expand-Label(старый, «traffic upd», «», 32).
fn keyupdate_secret(secret: &[u8; 32]) -> [u8; 32] {
    let next = hkdf_expand_label(secret, "traffic upd", &[], 32);
    let mut r = [0u8; 32];
    r.copy_from_slice(&next);
    r
}

/// Полный key schedule рукопожатия 1-RTT без PSK.
struct HandshakeSecrets {
    client_hs: [u8; 32],
    server_hs: [u8; 32],
    master: [u8; 32],
}

/// Расчёт секретов из общего секрета ECDHE и транскрипта CH..SH.
fn handshake_secrets(shared: &[u8; 32], transcript_ch_sh: &[&[u8]]) -> HandshakeSecrets {
    // early = Extract(0^32, 0^32)
    let early = hkdf_extract(&[0u8; 32], &[0u8; 32]);
    // derived = Derive-Secret(early, "derived", "")
    let derived = derive_secret(&early, "derived", &[]);
    // handshake = Extract(derived, shared)
    let handshake = hkdf_extract(&derived, shared);
    let client_hs = derive_secret(&handshake, "c hs traffic", transcript_ch_sh);
    let server_hs = derive_secret(&handshake, "s hs traffic", transcript_ch_sh);
    let derived2 = derive_secret(&handshake, "derived", &[]);
    let master = hkdf_extract(&derived2, &[0u8; 32]);
    HandshakeSecrets { client_hs, server_hs, master }
}

/// Ключ Finished для направления: HKDF-Expand-Label(secret, "finished").
fn finished_key(secret: &[u8]) -> [u8; 32] {
    let fk = hkdf_expand_label(secret, "finished", &[], 32);
    let mut r = [0u8; 32];
    r.copy_from_slice(&fk);
    r
}

/// verify_data Finished: HMAC(finished_key, Hash(transcript)).
fn finished_verify_data(fk: &[u8; 32], transcript: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for m in transcript {
        h.update(m);
    }
    hmac_sha256(fk, &h.finish())
}

// ============================================================================
// Защищённый слой записей
// ============================================================================

/// Защита записи: `content || type` → шифротекст с тегом.
fn protect(keys: &mut TrafficKeys, content_type: u8, content: &[u8]) -> Vec<u8> {
    let mut inner = Vec::with_capacity(content.len() + 1);
    inner.extend_from_slice(content);
    inner.push(content_type);
    let len = inner.len() + 16;
    let aad = [RT_APP_DATA, 0x03, 0x03, (len >> 8) as u8, len as u8];
    let ct = aead_chacha20poly1305_seal(&keys.key, &keys.nonce(), &aad, &inner);
    keys.seq = keys.seq.wrapping_add(1);
    let mut record = Vec::with_capacity(5 + ct.len());
    record.extend_from_slice(&[RT_APP_DATA, 0x03, 0x03, (len >> 8) as u8, len as u8]);
    record.extend_from_slice(&ct);
    record
}

/// Снятие защиты: шифротекст → `(content, content_type)`.
///
/// Внутренний plaintext = `content ‖ type ‖ нули-паддинг` (RFC 8446 §5.4):
/// тип — последний НЕнулевой байт. Нули в конце контента (например,
/// u16-длины расширений сертификата) сохраняются.
fn unprotect(keys: &mut TrafficKeys, header: &[u8; 5], ciphertext: &[u8]) -> Result<(Vec<u8>, u8), TlsError> {
    let inner = aead_chacha20poly1305_open(&keys.key, &keys.nonce(), header, ciphertext)
        .map_err(|_| TlsError::DecryptFailed)?;
    keys.seq = keys.seq.wrapping_add(1);
    let mut end = inner.len();
    while end > 0 && inner[end - 1] == 0 {
        end -= 1; // паддинг после байта типа
    }
    if end == 0 {
        return Err(TlsError::BadMessage("нулевая защищённая запись"));
    }
    let content_type = inner[end - 1];
    let content = inner[..end - 1].to_vec();
    Ok((content, content_type))
}

// ============================================================================
// Соединение
// ============================================================================

/// Установленное TLS 1.3-соединение.
///
/// Создаётся [`TlsConnection::connect`]; дальше — синхронные
/// `read`/`write` байтов приложения.
pub struct TlsConnection {
    stream: TcpStream,
    #[allow(dead_code)]
    host: String,
    read_keys: Option<TrafficKeys>,
    write_keys: Option<TrafficKeys>,
    /// Расшифрованные, но ещё не отданные байты приложения.
    pending: Vec<u8>,
    /// Текущий s ap traffic secret (для KeyUpdate).
    server_ap_secret: [u8; 32],
    closed: bool,
}

impl TlsConnection {
    /// Рукопожатие 1-RTT с `host:port`.
    pub fn connect(host: &str, port: u16, timeout: Duration) -> Result<TlsConnection, TlsError> {
        if host.is_empty()
            || !host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        {
            return Err(TlsError::BadMessage("имя хоста: только ASCII-домены"));
        }
        let addr = (host, port)
            .to_socket_addrs()
            .map_err(TlsError::Io)?
            .next()
            .ok_or(TlsError::BadMessage("хост не разрешается в адрес"))?;
        let stream = TcpStream::connect_timeout(&addr, timeout)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        stream.set_nodelay(true).ok();
        Self::handshake(stream, host)
    }

    /// Полное рукопожатие поверх готового стрима.
    fn handshake(mut stream: TcpStream, host: &str) -> Result<TlsConnection, TlsError> {
        // 1. Эфемерная пара X25519.
        let priv_key = entropy32();
        let pub_key = x25519_base(&priv_key);
        let random = entropy32();
        let session_id = entropy32();
        let client_hello = build_client_hello(host, &random, &session_id, &pub_key);

        // 2. Отправка ClientHello (незащищённая запись, legacy 0x0301).
        let mut rec = Vec::with_capacity(client_hello.len() + 5);
        rec.extend_from_slice(&[
            RT_HANDSHAKE,
            0x03,
            0x01,
            (client_hello.len() >> 8) as u8,
            client_hello.len() as u8,
        ]);
        rec.extend_from_slice(&client_hello);
        stream.write_all(&rec)?;
        stream.flush()?;

        // 3. Чтение до ServerHello: первая plaintext handshake-запись.
        let sh_record: Vec<u8>;
        let server_hello = loop {
            let (rtype, body) = read_plain_record(&mut stream)?;
            match rtype {
                RT_CCS => continue, // middlebox compat
                RT_ALERT => return Err(parse_alert(&body)),
                RT_HANDSHAKE => {
                    if body.is_empty() || body[0] != MT_SERVER_HELLO {
                        return Err(TlsError::BadMessage("ожидался ServerHello"));
                    }
                    // тело сообщения после 4-байтного заголовка
                    let parsed = parse_server_hello(&body[4..])?;
                    sh_record = body;
                    break parsed;
                }
                _ => return Err(TlsError::BadMessage("запись до ServerHello не handshake")),
            }
        };

        // 4. Общий секрет ECDHE (нулевой — маленькая подгруппа/баг).
        let shared = x25519(&priv_key, &server_hello.server_pub);
        if shared == [0u8; 32] {
            return Err(TlsError::BadMessage("ECDHE дал нулевой секрет (маленькая подгруппа)"));
        }

        // 5. Секреты рукопожатия; транскрипт = CH || SH.
        let secrets = handshake_secrets(&shared, &[&client_hello, &sh_record]);
        let mut server_keys = TrafficKeys::from_secret(&secrets.server_hs);
        let mut client_keys = TrafficKeys::from_secret(&secrets.client_hs);

        // 6. Защищённые записи рукопожатия: EE → Cert → CertVerify → Finished.
        //    Одна запись может нести несколько сообщений (а сообщение —
        //    продолжаться в следующей записи), поэтому копим буфер.
        let mut transcript: Vec<Vec<u8>> = vec![client_hello.clone(), sh_record];
        let mut hs_buf: Vec<u8> = Vec::new();
        let mut server_fin: Option<[u8; 32]> = None;
        while server_fin.is_none() {
            let mut header = [0u8; 5];
            read_exact_eof(&mut stream, &mut header)?;
            let rtype = header[0];
            let len = u16::from_be_bytes([header[3], header[4]]) as usize;
            if len > MAX_RECORD {
                return Err(TlsError::BadMessage("запись рукопожатия длиннее 2^16"));
            }
            let mut body = vec![0u8; len];
            read_exact_eof(&mut stream, &mut body)?;
            match rtype {
                RT_CCS => continue,
                RT_ALERT => return Err(parse_alert(&body)),
                RT_APP_DATA => {
                    let (content, inner_type) = unprotect(&mut server_keys, &header, &body)?;
                    if inner_type != RT_HANDSHAKE {
                        return Err(TlsError::BadMessage(
                            "защищённая запись рукопожатия не handshake",
                        ));
                    }
                    hs_buf.extend_from_slice(&content);
                }
                _ => return Err(TlsError::BadMessage("неожиданный тип записи в рукопожатии")),
            }
            // Разбор всех полных сообщений в буфере.
            while let Some((msg, rest)) = split_message(&hs_buf) {
                hs_buf = rest;
                let mtype = msg[0];
                match mtype {
                    MT_ENCRYPTED_EXTENSIONS | MT_CERTIFICATE | MT_CERTIFICATE_VERIFY => {
                        transcript.push(msg);
                    }
                    MT_FINISHED => {
                        // Тело = 32 байта verify_data; проверка по
                        // транскрипту БЕЗ самого Finished.
                        if msg.len() != 4 + 32 {
                            return Err(TlsError::BadMessage("Finished не 32 байта"));
                        }
                        let refs: Vec<&[u8]> = transcript.iter().map(|m| m.as_slice()).collect();
                        let fk = finished_key(&secrets.server_hs);
                        let expect = finished_verify_data(&fk, &refs);
                        let mut got = [0u8; 32];
                        got.copy_from_slice(&msg[4..]);
                        if expect != got {
                            return Err(TlsError::BadMessage(
                                "Finished сервера не прошёл проверку HMAC",
                            ));
                        }
                        server_fin = Some(got);
                        transcript.push(msg);
                    }
                    _ => {
                        return Err(TlsError::BadMessage(
                            "неожиданное сообщение в рукопожатии (ожидались EE/Cert/CV/Fin)",
                        ))
                    }
                }
            }
        }

        // 7. App-секреты: транскрипт CH..ServerFinished.
        let refs: Vec<&[u8]> = transcript.iter().map(|m| m.as_slice()).collect();
        let client_ap = derive_secret(&secrets.master, "c ap traffic", &refs);
        let server_ap = derive_secret(&secrets.master, "s ap traffic", &refs);

        // 8. Client Finished под ключами рукопожатия (seq продолжает c_hs).
        let fk_c = finished_key(&secrets.client_hs);
        let vd = finished_verify_data(&fk_c, &refs);
        let mut fin_msg = Vec::with_capacity(36);
        fin_msg.push(MT_FINISHED);
        fin_msg.extend_from_slice(&[0, 0, 32]);
        fin_msg.extend_from_slice(&vd);
        let fin_record = protect(&mut client_keys, RT_HANDSHAKE, &fin_msg);
        stream.write_all(&fin_record)?;
        stream.flush()?;

        // 9. Переключение на app-ключи (счётчики записей с нуля).
        Ok(TlsConnection {
            stream,
            host: host.to_string(),
            read_keys: Some(TrafficKeys::from_secret(&server_ap)),
            write_keys: Some(TrafficKeys::from_secret(&client_ap)),
            pending: Vec::new(),
            server_ap_secret: server_ap,
            closed: false,
        })
    }

    /// Чтение байтов приложения (блокирующее, хотя бы один байт).
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, TlsError> {
        while self.pending.is_empty() {
            if self.closed {
                return Ok(0);
            }
            self.fill_pending()?;
        }
        let n = buf.len().min(self.pending.len());
        buf[..n].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        Ok(n)
    }

    /// Запись байтов приложения (порциями ≤ 2^14).
    pub fn write(&mut self, data: &[u8]) -> Result<(), TlsError> {
        let keys = self
            .write_keys
            .as_mut()
            .ok_or(TlsError::BadMessage("соединение не установлено"))?;
        for chunk in data.chunks(1 << 14) {
            let record = protect(keys, RT_APP_DATA, chunk);
            self.stream.write_all(&record)?;
        }
        self.stream.flush()?;
        Ok(())
    }

    /// Вежливое закрытие: close_notify + shutdown TCP.
    pub fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            if let Some(keys) = self.write_keys.as_mut() {
                let alert = [1, 0]; // warning, close_notify
                let record = protect(keys, RT_ALERT, &alert);
                let _ = self.stream.write_all(&record);
                let _ = self.stream.flush();
            }
            let _ = self.stream.shutdown(std::net::Shutdown::Both);
        }
    }

    /// Чтение одной записи и наполнение `pending` байтами приложения.
    fn fill_pending(&mut self) -> Result<(), TlsError> {
        let mut header = [0u8; 5];
        read_exact_eof(&mut self.stream, &mut header)?;
        let rtype = header[0];
        let len = u16::from_be_bytes([header[3], header[4]]) as usize;
        if len > MAX_RECORD {
            return Err(TlsError::BadMessage("запись длиннее 2^16"));
        }
        let mut body = vec![0u8; len];
        read_exact_eof(&mut self.stream, &mut body)?;
        match rtype {
            RT_CCS => Ok(()), // пропустить (не должно быть после hs, но терпим)
            RT_ALERT => Err(parse_alert(&body)),
            RT_APP_DATA => {
                let keys = self
                    .read_keys
                    .as_mut()
                    .ok_or(TlsError::BadMessage("защищённая запись до готовности ключей"))?;
                let (content, inner_type) = unprotect(keys, &header, &body)?;
                match inner_type {
                    RT_APP_DATA => {
                        self.pending.extend_from_slice(&content);
                        Ok(())
                    }
                    RT_ALERT => {
                        if content.len() >= 2 && content[1] == 0 {
                            self.closed = true; // close_notify
                            Ok(())
                        } else if content.len() >= 2 {
                            Err(TlsError::Alert { level: content[0], desc: content[1] })
                        } else {
                            Err(TlsError::BadMessage("пустой защищённый alert"))
                        }
                    }
                    MT_NEW_SESSION_TICKET | MT_KEY_UPDATE => {
                        if inner_type == MT_KEY_UPDATE && !content.is_empty() {
                            // KeyUpdate(request=0|1): обновляем ключи чтения.
                            // request_posted=1 просит обновить и наши ключи
                            // записи — для коротких GET игнорируем (сервер
                            // обязан терпеть отсутствие ответного KeyUpdate).
                            self.server_ap_secret = keyupdate_secret(&self.server_ap_secret);
                            self.read_keys = Some(TrafficKeys::from_secret(&self.server_ap_secret));
                        }
                        Ok(())
                    }
                    RT_HANDSHAKE => Ok(()), // прочие post-hs handshake — пропустить
                    _ => Ok(()),
                }
            }
            _ => Err(TlsError::BadMessage("неизвестный тип записи")),
        }
    }
}

impl Drop for TlsConnection {
    fn drop(&mut self) {
        self.close();
    }
}

/// Чтение незащищённой записи (до ServerHello).
fn read_plain_record(stream: &mut TcpStream) -> Result<(u8, Vec<u8>), TlsError> {
    let mut header = [0u8; 5];
    read_exact_eof(stream, &mut header)?;
    let rtype = header[0];
    let len = u16::from_be_bytes([header[3], header[4]]) as usize;
    if len > MAX_RECORD {
        return Err(TlsError::BadMessage("запись длиннее 2^16"));
    }
    let mut body = vec![0u8; len];
    read_exact_eof(stream, &mut body)?;
    Ok((rtype, body))
}

/// `read_exact`, отличающий EOF от короткого чтения.
fn read_exact_eof(stream: &mut TcpStream, buf: &mut [u8]) -> Result<(), TlsError> {
    let mut off = 0;
    while off < buf.len() {
        match stream.read(&mut buf[off..]) {
            Ok(0) => return Err(TlsError::UnexpectedEof),
            Ok(n) => off += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(TlsError::Io(e)),
        }
    }
    Ok(())
}

/// Разбор alert-записи (level, desc).
fn parse_alert(body: &[u8]) -> TlsError {
    if body.len() >= 2 {
        if body[1] == 0 {
            // close_notify до/во время рукопожатия — это отказ сервера
            TlsError::BadMessage("сервер закрыл соединение (close_notify на рукопожатии)")
        } else {
            TlsError::Alert { level: body[0], desc: body[1] }
        }
    } else {
        TlsError::BadMessage("alert короче 2 байт")
    }
}

/// Выделение полного handshake-сообщения из буфера фрагментов:
/// `(сообщение, остаток)` или `None`, если сообщение ещё не дополучено.
fn split_message(buf: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    if buf.len() < 4 {
        return None;
    }
    let body_len = ((buf[1] as usize) << 16) | ((buf[2] as usize) << 8) | buf[3] as usize;
    let total = 4 + body_len;
    if buf.len() < total || total > MAX_RECORD {
        return None;
    }
    Some((buf[..total].to_vec(), buf[total..].to_vec()))
}

// ============================================================================
// Тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tlsprim::{hex, unhex};

    /// RFC 8448 §3: ClientHello (196 октетов) — трасса «Simple 1-RTT».
    const RFC8448_CH: &str = "010000c00303cb34ecb1e78163ba1c38c6dacb196a6dffa21a8d9912ec18a2ef6283024dece7000006130113031302010000910000000b0009000006736572766572ff01000100000a00140012001d0017001800190100010101020103010400230000003300260024001d002099381de560e4bd43d23d8e435a7dbafeb3c06e51c13cae4d5413691e529aaf2c002b0003020304000d0020001e040305030603020308040805080604010501060102010402050206020202002d00020101001c00024001";
    /// RFC 8448 §3: ServerHello (90 октетов).
    const RFC8448_SH: &str = "020000560303a6af06a4121860dc5e6e60249cd34c95930c8ac5cb1434dac155772ed3e2692800130100002e00330024001d0020c9828876112095fe66762bdbf7c672e156d6cc253b833df1dd69b1b04e751f0f002b00020304";
    /// ECDHE-общий секрет трассы.
    const RFC8448_SHARED: &str = "8bd4054fb55b9d63fdfbacf9f04b9f0d35e6d63f537563efd46272900f89492d";
    /// Все секреты трассы.
    const RFC8448_EARLY: &str = "33ad0a1c607ec03b09e6cd9893680ce210adf300aa1f2660e1b22e10f170f92a";
    const RFC8448_DERIVED_EARLY: &str = "6f2615a108c702c5678f54fc9dbab69716c076189c48250cebeac3576c3611ba";
    const RFC8448_HANDSHAKE: &str = "1dc826e93606aa6fdc0aadc12f741b01046aa6b99f691ed221a9f0ca043fbeac";
    const RFC8448_C_HS: &str = "b3eddb126e067f35a780b3abf45e2d8f3b1a950738f52e9600746a0e27a55a21";
    const RFC8448_S_HS: &str = "b67b7d690cc16c4e75e54213cb2d37b4e9c912bcded9105d42befd59d391ad38";
    const RFC8448_DERIVED_HS: &str = "43de77e0c77713859a944db9db2590b53190a65b3ee2e4f12dd7a0bb7ce254b4";
    const RFC8448_MASTER: &str = "18df06843d13a08bf2a449844c5f8a478001bc4d4c627984d5a41da8d0402919";
    /// Хэш транскрипта CH..SH.
    const RFC8448_HASH_CH_SH: &str = "860c06edc07858ee8e78f0e7428c58edd6b43f2ca3e6e95f02ed063cf0e1cad8";
    /// Хэш транскрипта CH..ServerFinished (для app-секретов).
    const RFC8448_HASH_CH_SFIN: &str = "9608102a0f1ccc6db6250b7b7e417b1a000eaada3daae4777a7686c9ff83df13";
    const RFC8448_C_AP: &str = "9e40646ce79a7f9dc05af8889bce6552875afa0b06df0087f792ebb7c17504a5";
    const RFC8448_S_AP: &str = "a11af9f05531f856ad47116b45a950328204b4f44bfb6b3a4b4f1f3fcb631643";
    /// finished_key клиента и verify_data Finished клиента.
    const RFC8448_C_FIN_KEY: &str = "b80ad01015fb2f0bd65ff7d4da5d6bf83f84821d1f87fdc7d3c75b5a7b42d9c4";
    const RFC8448_C_FIN: &str = "a8ec436d677634ae525ac1fcebe11a039ec17694fac6e98527b642f2edd5ce61";
    /// Полный хвост транскрипта: EE(40) + Cert(445) + CV(136) + SVFin(36).
    const RFC8448_TAIL_FULL: &str = concat!(
        "080000240022000a00140012001d00170018001901000101010201030104001c",
        "00024001000000000b0001b9000001b50001b0308201ac30820115a003020102",
        "020102300d06092a864886f70d01010b0500300e310c300a0603550403130372",
        "7361301e170d3136303733303031323335395a170d3236303733303031323335",
        "395a300e310c300a0603550403130372736130819f300d06092a864886f70d01",
        "0101050003818d0030818902818100b4bb498f8279303d980836399b36c6988c",
        "0c68de55e1bdb826d3901a2461eafd2de49a91d015abbc9a95137ace6c1af19e",
        "aa6af98c7ced43120998e187a80ee0ccb0524b1b018c3e0b63264d449a6d38e2",
        "2a5fda430846748030530ef0461c8ca9d9efbfae8ea6d1d03e2bd193eff0ab9a",
        "8002c47428a6d35a8d88d79f7f1e3f0203010001a31a301830090603551d1304",
        "023000300b0603551d0f0404030205a0300d06092a864886f70d01010b050003",
        "81810085aad2a0e5b9276b908c65f73a7267170618a54c5f8a7b337d2df7a594",
        "365417f2eae8f8a58c8f8172f9319cf36b7fd6c55b80f21a03015156726096fd",
        "335e5e67f2dbf102702e608ccae6bec1fc63a42a99be5c3eb7107c3c54e9b9eb",
        "2bd5203b1c3b84e0a8b2f759409ba3eac9d91d402dcc0cc8f8961229ac9187b4",
        "2b4de100000f000084080400805a747c5d88fa9bd2e55ab085a61015b7211f82",
        "4cd484145ab3ff52f1fda8477b0b7abc90db78e2d33a5c141a078653fa6bef78",
        "0c5ea248eeaaa785c4f394cab6d30bbe8d4859ee511f602957b15411ac027671",
        "459e46445c9ea58c181e818e95b8c3fb0bf3278409d3be152a3da5043e063dda",
        "65cdf5aea20d53dfacd42f74f3140000209b9b141d906337fbd2cbdce71df4de",
        "da4ab42c309572cb7fffee5454b78f0718"
    );

    #[test]
    fn key_schedule_rfc8448() {
        // Ранняя стадия: early secret фиксирован (нет PSK).
        let early = hkdf_extract(&[0u8; 32], &[0u8; 32]);
        assert_eq!(hex(&early), RFC8448_EARLY);
        let derived = derive_secret(&early, "derived", &[]);
        assert_eq!(hex(&derived), RFC8448_DERIVED_EARLY);

        // Хэш транскрипта CH..SH.
        let ch = unhex(RFC8448_CH);
        let sh = unhex(RFC8448_SH);
        let mut h = Sha256::new();
        h.update(&ch);
        h.update(&sh);
        assert_eq!(hex(&h.finish()), RFC8448_HASH_CH_SH);

        // Стадия рукопожатия из общего секрета ECDHE.
        let shared_vec = unhex(RFC8448_SHARED);
        let mut shared = [0u8; 32];
        shared.copy_from_slice(&shared_vec);
        let handshake = hkdf_extract(&derived, &shared);
        assert_eq!(hex(&handshake), RFC8448_HANDSHAKE);
        let c_hs = derive_secret(&handshake, "c hs traffic", &[&ch, &sh]);
        assert_eq!(hex(&c_hs), RFC8448_C_HS);
        let s_hs = derive_secret(&handshake, "s hs traffic", &[&ch, &sh]);
        assert_eq!(hex(&s_hs), RFC8448_S_HS);
        let derived2 = derive_secret(&handshake, "derived", &[]);
        assert_eq!(hex(&derived2), RFC8448_DERIVED_HS);
        let master = hkdf_extract(&derived2, &[0u8; 32]);
        assert_eq!(hex(&master), RFC8448_MASTER);

        // App-секреты: транскрипт CH..ServerFinished.
        let tail = unhex(RFC8448_TAIL_FULL);
        let mut h = Sha256::new();
        h.update(&ch);
        h.update(&sh);
        h.update(&tail);
        assert_eq!(hex(&h.finish()), RFC8448_HASH_CH_SFIN);
        let transcript = [ch.as_slice(), sh.as_slice(), tail.as_slice()];
        let c_ap = derive_secret(&master, "c ap traffic", &transcript);
        assert_eq!(hex(&c_ap), RFC8448_C_AP);
        let s_ap = derive_secret(&master, "s ap traffic", &transcript);
        assert_eq!(hex(&s_ap), RFC8448_S_AP);

        // Finished: ключ и verify_data клиента.
        let fk = finished_key(&c_hs);
        assert_eq!(hex(&fk), RFC8448_C_FIN_KEY);
        let vd = finished_verify_data(&fk, &transcript);
        assert_eq!(hex(&vd), RFC8448_C_FIN);
    }

    #[test]
    fn client_hello_layout() {
        // Детерминированная сборка: проверяем структуру обратным разбором.
        let random = [1u8; 32];
        let session_id = [2u8; 32];
        let pubkey = [9u8; 32];
        let ch = build_client_hello("ru.wikipedia.org", &random, &session_id, &pubkey);
        assert_eq!(ch[0], MT_CLIENT_HELLO);
        // длина тела в заголовке
        let body_len = ((ch[1] as usize) << 16) | ((ch[2] as usize) << 8) | ch[3] as usize;
        assert_eq!(ch.len(), body_len + 4);
        // SNI присутствует
        let host = b"ru.wikipedia.org";
        let hay = &ch[..];
        assert!(hay.windows(host.len()).any(|w| w == host));
        // шифр 0x1303 в списке
        assert!(hay.windows(2).any(|w| w == [0x13, 0x03]));
        // публичный ключ в key_share
        assert!(hay.windows(32).any(|w| w == pubkey));
    }

    #[test]
    fn server_hello_parse_synthetic() {
        // Сборка валидного SH: random, cipher 0x1303, ext supported_versions + key_share.
        let mut body = Vec::new();
        body.extend_from_slice(&0x0303u16.to_be_bytes());
        body.extend_from_slice(&[7u8; 32]); // random ≠ HRR
        body.push(0); // session_id пуст
        body.extend_from_slice(&CIPHER_CHACHA.to_be_bytes());
        body.push(0); // compression
        let mut exts = Vec::new();
        exts.extend_from_slice(&43u16.to_be_bytes());
        exts.extend_from_slice(&2u16.to_be_bytes());
        exts.extend_from_slice(&[0x03, 0x04]);
        exts.extend_from_slice(&51u16.to_be_bytes());
        exts.extend_from_slice(&36u16.to_be_bytes());
        exts.extend_from_slice(&GROUP_X25519.to_be_bytes());
        exts.extend_from_slice(&32u16.to_be_bytes());
        exts.extend_from_slice(&[5u8; 32]);
        body.extend_from_slice(&(exts.len() as u16).to_be_bytes());
        body.extend_from_slice(&exts);
        let sh = parse_server_hello(&body).unwrap();
        assert_eq!(sh.server_pub, [5u8; 32]);

        // Чужой шифр — отказ.
        let mut bad_cipher = body.clone();
        let cipher_off = 2 + 32 + 1;
        bad_cipher[cipher_off..cipher_off + 2].copy_from_slice(&0x1301u16.to_be_bytes());
        assert!(parse_server_hello(&bad_cipher).is_err());

        // HRR-random — вежливый отказ.
        let mut hrr = body.clone();
        hrr[2..34].copy_from_slice(&HRR_RANDOM);
        assert!(parse_server_hello(&hrr).is_err());

        // Обрыв сообщения — отказ.
        assert!(parse_server_hello(&body[..20]).is_err());
    }

    #[test]
    fn record_seal_open_roundtrip() {
        // Полный цикл защиты: ключи из секрета, AAD = заголовок, seq растёт.
        let mut keys = TrafficKeys::from_secret(&[3u8; 32]);
        let payload = b"GET /wiki/Rust HTTP/1.1\r\nHost: ru.wikipedia.org\r\n\r\n";
        let record = protect(&mut keys, RT_APP_DATA, payload);
        assert_eq!(record[0], RT_APP_DATA);
        let len = u16::from_be_bytes([record[3], record[4]]) as usize;
        assert_eq!(record.len(), 5 + len);
        assert_eq!(keys.seq, 1);

        let mut header = [0u8; 5];
        header.copy_from_slice(&record[..5]);
        // Читатель — свой экземпляр ключей: своя последовательность записей.
        let mut reader = TrafficKeys::from_secret(&[3u8; 32]);
        let (content, ctype) = unprotect(&mut reader, &header, &record[5..]).unwrap();
        assert_eq!(content, payload.to_vec());
        assert_eq!(ctype, RT_APP_DATA);

        // Порча одного байта → отказ.
        let mut broken = record.clone();
        let mid = broken.len() / 2;
        broken[mid] ^= 1;
        let mut k2 = TrafficKeys::from_secret(&[3u8; 32]);
        let mut h2 = [0u8; 5];
        h2.copy_from_slice(&broken[..5]);
        assert!(matches!(
            unprotect(&mut k2, &h2, &broken[5..]),
            Err(TlsError::DecryptFailed)
        ));

        // Нонс = iv ⊕ seq: соседние записи получают разные нонсы.
        let n0 = TrafficKeys::from_secret(&[3u8; 32]).nonce();
        let mut k1 = TrafficKeys::from_secret(&[3u8; 32]);
        k1.seq = 1;
        let n1 = k1.nonce();
        assert_ne!(n0, n1);
        assert_eq!(&n0[..4], &n1[..4]); // старшие 4 байта iv не тронуты
    }

    #[test]
    fn keyupdate_roll() {
        // KeyUpdate: «traffic upd» даёт новые ключи, seq обнуляется.
        let s_ap = unhex(RFC8448_S_AP);
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&s_ap);
        let k1 = TrafficKeys::from_secret(&secret);
        let next = keyupdate_secret(&secret);
        assert_ne!(next, secret);
        let k2 = TrafficKeys::from_secret(&next);
        assert_ne!(k1.key, k2.key);
        assert_ne!(k1.iv, k2.iv);
        assert_eq!(k2.seq, 0);
        // Идемпотентность домен-сепарации: та же метка — тот же результат.
        assert_eq!(keyupdate_secret(&secret), next);
    }

    #[test]
    fn alert_text_map() {
        assert!(alert_text(40, 2).contains("handshake_failure"));
        assert!(alert_text(0, 1).contains("close_notify"));
        assert!(alert_text(200, 2).contains("unknown"));
    }

    /// Живое рукопожатие с Википедией (запускать явно:
    /// `cargo test -p pqc --lib tls13::tests::live_handshake_wikipedia -- --ignored`).
    #[test]
    #[ignore = "живой интернет: смоук-тест транспорта"]
    fn live_handshake_wikipedia() {
        let mut conn =
            TlsConnection::connect("ru.wikipedia.org", 443, Duration::from_secs(15)).unwrap();
        conn.write(b"GET /wiki/Rust HTTP/1.1\r\nHost: ru.wikipedia.org\r\nUser-Agent: POLER-Quantum/0.9.0 (https://github.com/Kotokvit/POLER-Quantum-RS)\r\nAccept: text/html\r\nAccept-Encoding: identity\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match conn.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                Err(e) => panic!("чтение: {e}"),
            }
        }
        assert!(buf.starts_with(b"HTTP/1.1 2"), "нет HTTP-ответа: {:?}", &buf[..40.min(buf.len())]);
        let text = String::from_utf8_lossy(&buf);
        assert!(text.contains("<html"), "нет HTML в ответе");
    }
}
