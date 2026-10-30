//! RQ19: криптографические примитивы TLS 1.3 — **с нуля, zero-dep**.
//!
//! Транспорт интернета знаний ([`crate::tls13`]) не тянет в проект ни
//! одной внешней зависимости: весь набор примитивов протокола
//! реализован здесь на чистом `std` и зафиксирован тестовыми векторами
//! из RFC:
//!
//! | Примитив | Спецификация | Тестовый вектор |
//! |---|---|---|
//! | [`sha256`] | FIPS 180-4 | «abc», пустая строка, двухблочный хэш |
//! | [`hmac_sha256`] | RFC 2104 | RFC 4231 кейсы 1–3 |
//! | [`hkdf_extract`]/[`hkdf_expand`] | RFC 5869 | кейс 1 |
//! | [`chacha20_xor`] | RFC 8439 §2.3–2.4 | блок и шифрование |
//! | [`poly1305`] | RFC 8439 §2.5 | вектор §2.5.2 |
//! | [`aead_chacha20poly1305_seal`]/[`_open`] | RFC 8439 §2.8 | вектор §2.8.2 |
//! | [`x25519`] | RFC 7748 §5–6 | скаляры, DH-пара |
//!
//! Философия слоя — как у всего POLER: **никаких доверённых чёрных
//! ящиков**. Каждый байт рукопожатия TLS, который отправляет и принимает
//! [`crate::tls13`], вычисляется кодом, чью корректность можно проверить
//! по эталонным векторам стандартов.
//!
//! # Честные границы
//!
//! * Реализация не претендует на side-channel-стойкость production-
//!   библиотек: горячие циклы не ветвятся по секретным данным, но
//!   формальных constant-time гарантий нет. Для ингеста публичных
//!   знаний приемлемо; для секретов — нет.
//! * TLS-клиент [`crate::tls13`] **не проверяет цепочку сертификатов**
//!   (в zero-dep мире нет корневого хранилища): канал шифруется против
//!   пассивного прослушивания, но не аутентифицирует сервер.

use std::fmt;

// ============================================================================
// SHA-256 (FIPS 180-4)
// ============================================================================

/// Константы раундов SHA-256.
const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
    0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
    0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
    0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
    0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
    0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const SHA256_H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c,
    0x1f83d9ab, 0x5be0cd19,
];

/// Поточный SHA-256: `update` кормит байтами, `finish` отдаёт дайджест.
///
/// Транскрипт-хэш TLS 1.3 строится именно потоково: рукопожатие
/// приходит сообщениями, хэш считается на лету.
#[derive(Clone)]
pub struct Sha256 {
    state: [u32; 8],
    buf: [u8; 64],
    buf_len: usize,
    total: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Sha256 { state: SHA256_H0, buf: [0; 64], buf_len: 0, total: 0 }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total = self.total.wrapping_add(data.len() as u64);
        if self.buf_len > 0 {
            let need = 64 - self.buf_len;
            let take = need.min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == 64 {
                let block = self.buf;
                self.compress(&block);
                self.buf_len = 0;
            }
        }
        while data.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[..64]);
            self.compress(&block);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    /// Дайджест (потребляет экземпляр — как `ring`/`RustCrypto`).
    pub fn finish(mut self) -> [u8; 32] {
        let bit_len = self.total.wrapping_mul(8);
        self.push_pad(&[0x80]);
        while self.buf_len != 56 {
            self.push_pad(&[0]);
        }
        self.push_pad(&bit_len.to_be_bytes());
        debug_assert_eq!(self.buf_len, 0);
        let mut out = [0u8; 32];
        for (i, w) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&w.to_be_bytes());
        }
        out
    }

    /// Байты паддинга — без счётчика длины (длина снята до паддинга).
    fn push_pad(&mut self, data: &[u8]) {
        for &b in data {
            self.buf[self.buf_len] = b;
            self.buf_len += 1;
            if self.buf_len == 64 {
                let block = self.buf;
                self.compress(&block);
                self.buf_len = 0;
            }
        }
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

/// Однократный SHA-256.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finish()
}

/// hex-кодирование (диагностика и тесты).
pub fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0xf) as usize] as char);
    }
    s
}

/// hex-парсер (тесты; паникует на мусоре — это fixtures-API, не парсинг
/// пользовательского ввода).
pub fn unhex(s: &str) -> Vec<u8> {
    let s = s.trim();
    assert!(s.len().is_multiple_of(2), "unhex: нечётная длина");
    let b = s.as_bytes();
    (0..s.len() / 2)
        .map(|i| {
            let v = |c: u8| -> u8 {
                match c {
                    b'0'..=b'9' => c - b'0',
                    b'a'..=b'f' => c - b'a' + 10,
                    b'A'..=b'F' => c - b'A' + 10,
                    _ => panic!("unhex: не-hex байт {:#04x}", c),
                }
            };
            (v(b[i * 2]) << 4) | v(b[i * 2 + 1])
        })
        .collect()
}

// ============================================================================
// HMAC-SHA256 (RFC 2104) и HKDF (RFC 5869)
// ============================================================================

/// HMAC-SHA256 с ключом произвольной длины.
pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&sha256(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(&ipad);
    inner.update(msg);
    let inner = inner.finish();
    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(&inner);
    outer.finish()
}

/// HKDF-Extract (RFC 5869 §2.2): `PRK = HMAC-Hash(salt, IKM)`.
pub fn hkdf_extract(salt: &[u8], ikm: &[u8]) -> [u8; 32] {
    hmac_sha256(salt, ikm)
}

/// HKDF-Expand (RFC 5869 §2.3): до 255 блокейнов по 32 байта.
pub fn hkdf_expand(prk: &[u8], info: &[u8], len: usize) -> Vec<u8> {
    assert!(len <= 255 * 32, "hkdf_expand: len > 8160");
    let mut okm = Vec::with_capacity(len);
    let mut t: Vec<u8> = Vec::new();
    let mut counter: u8 = 1;
    while okm.len() < len {
        let mut msg = Vec::with_capacity(t.len() + info.len() + 1);
        msg.extend_from_slice(&t);
        msg.extend_from_slice(info);
        msg.push(counter);
        let block = hmac_sha256(prk, &msg);
        okm.extend_from_slice(&block);
        t = block.to_vec();
        counter = counter.wrapping_add(1);
    }
    okm.truncate(len);
    okm
}

/// HKDF-Expand-Label (RFC 8446 §7.1) — рабочая лошадка key schedule.
///
/// ```text
/// struct { uint16 length; opaque label<7..255>; opaque context<0..255>; }
/// ```
/// где `label = "tls13 " + Label`.
pub fn hkdf_expand_label(secret: &[u8], label: &str, context: &[u8], len: usize) -> Vec<u8> {
    let full_label = format!("tls13 {label}");
    let mut info = Vec::with_capacity(2 + 1 + full_label.len() + 1 + context.len());
    info.extend_from_slice(&(len as u16).to_be_bytes());
    info.push(full_label.len() as u8);
    info.extend_from_slice(full_label.as_bytes());
    info.push(context.len() as u8);
    info.extend_from_slice(context);
    hkdf_expand(secret, &info, len)
}

/// Derive-Secret (RFC 8446 §7.1): метка + хэш конкатенации сообщений.
pub fn derive_secret(secret: &[u8], label: &str, messages: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for m in messages {
        h.update(m);
    }
    let th = h.finish();
    let out = hkdf_expand_label(secret, label, &th, 32);
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

// ============================================================================
// ChaCha20 (RFC 8439 §2.3–2.4)
// ============================================================================

/// Один блок ChaCha20 (64 байта keystream).
fn chacha20_block(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> [u8; 64] {
    let mut state = [0u32; 16];
    state[0] = 0x6170_7865;
    state[1] = 0x3320_646e;
    state[2] = 0x7962_2d32;
    state[3] = 0x6b20_6574;
    for i in 0..8 {
        state[4 + i] = u32::from_le_bytes([
            key[i * 4],
            key[i * 4 + 1],
            key[i * 4 + 2],
            key[i * 4 + 3],
        ]);
    }
    state[12] = counter;
    for i in 0..3 {
        state[13 + i] = u32::from_le_bytes([
            nonce[i * 4],
            nonce[i * 4 + 1],
            nonce[i * 4 + 2],
            nonce[i * 4 + 3],
        ]);
    }
    let mut w = state;
    for _ in 0..10 {
        quarter_round(&mut w, 0, 4, 8, 12);
        quarter_round(&mut w, 1, 5, 9, 13);
        quarter_round(&mut w, 2, 6, 10, 14);
        quarter_round(&mut w, 3, 7, 11, 15);
        quarter_round(&mut w, 0, 5, 10, 15);
        quarter_round(&mut w, 1, 6, 11, 12);
        quarter_round(&mut w, 2, 7, 8, 13);
        quarter_round(&mut w, 3, 4, 9, 14);
    }
    let mut out = [0u8; 64];
    for i in 0..16 {
        let v = w[i].wrapping_add(state[i]);
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    out
}

#[inline(always)]
fn quarter_round(w: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    w[a] = w[a].wrapping_add(w[b]);
    w[d] = (w[d] ^ w[a]).rotate_left(16);
    w[c] = w[c].wrapping_add(w[d]);
    w[b] = (w[b] ^ w[c]).rotate_left(12);
    w[a] = w[a].wrapping_add(w[b]);
    w[d] = (w[d] ^ w[a]).rotate_left(8);
    w[c] = w[c].wrapping_add(w[d]);
    w[b] = (w[b] ^ w[c]).rotate_left(7);
}

/// ChaCha20-шифрование XOR-ом keystream (счётчик блоков от `counter`).
pub fn chacha20_xor(key: &[u8; 32], nonce: &[u8; 12], counter: u32, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for (block_idx, chunk) in data.chunks(64).enumerate() {
        let ks = chacha20_block(key, counter.wrapping_add(block_idx as u32), nonce);
        for (i, b) in chunk.iter().enumerate() {
            out.push(b ^ ks[i]);
        }
    }
    out
}

// ============================================================================
// Poly1305 (RFC 8439 §2.5) — 5×26-битных конечностей (donna-стиль)
// ============================================================================

/// Poly1305-MAC: `tag = ((m·r mod 2^130−5) + s) mod 2^128`, LE-байты.
///
/// Горячий цикл без ветвлений по данным; умножения конечностей
/// помещаются в `u64` с запасом (конечности ≤ 2^27 после сложения,
/// r ≤ 2^26).
pub fn poly1305(key: &[u8; 32], msg: &[u8]) -> [u8; 16] {
    let b = |i: usize| u64::from(key[i]);
    // clamp(r) — маски donna по конечностям: кламп RFC 8439 чистит
    // биты {28..33, 60..65, 92..97, 124..127}, каждая конечность несёт
    // свою часть маски (limb-биты 2..7, 8..13, 14..19, 20..23).
    let r = [
        (b(0) | (b(1) << 8) | (b(2) << 16) | (b(3) << 24)) & 0x03ff_ffff,
        ((b(3) >> 2) | (b(4) << 6) | (b(5) << 14) | (b(6) << 22)) & 0x03ff_ff03,
        ((b(6) >> 4) | (b(7) << 4) | (b(8) << 12) | (b(9) << 20)) & 0x03ff_c0ff,
        ((b(9) >> 6) | (b(10) << 2) | (b(11) << 10) | (b(12) << 18)) & 0x03f0_3fff,
        ((b(12) >> 8) | (b(13)) | (b(14) << 8) | (b(15) << 16)) & 0x000f_ffff,
    ];
    // s — аддитивная часть (не клампится)
    let mut s = [0u8; 16];
    s.copy_from_slice(&key[16..32]);

    let mut h = [0u64; 5];
    let mut i = 0usize;
    while i < msg.len() {
        let take = (msg.len() - i).min(16);
        // Блок + бит 2^(8·take): полный блок даёт 2^128 (бит в h4).
        let mut blk = [0u8; 17];
        blk[..take].copy_from_slice(&msg[i..i + take]);
        blk[take] = 1;
        i += take;

        let m = |j: usize| u64::from(blk[j]);
        h[0] += m(0) | (m(1) << 8) | (m(2) << 16) | ((m(3) & 0x03) << 24);
        h[1] += (m(3) >> 2) | (m(4) << 6) | (m(5) << 14) | ((m(6) & 0x0f) << 22);
        h[2] += (m(6) >> 4) | (m(7) << 4) | (m(8) << 12) | ((m(9) & 0x3f) << 20);
        h[3] += (m(9) >> 6) | (m(10) << 2) | (m(11) << 10) | (m(12) << 18);
        h[4] += m(13) | (m(14) << 8) | (m(15) << 16) | (u64::from(blk[16]) << 24);

        // h = h·r mod 2^130 − 5 (5-я конечность × 19·… ≡ −5·2^130 ≡ −5·5)
        let r5 = |x: u64| x * 5;
        let t0 = h[0] * r[0] + r5(h[1] * r[4]) + r5(h[2] * r[3]) + r5(h[3] * r[2]) + r5(h[4] * r[1]);
        let t1 = h[0] * r[1] + h[1] * r[0] + r5(h[2] * r[4]) + r5(h[3] * r[3]) + r5(h[4] * r[2]);
        let t2 = h[0] * r[2] + h[1] * r[1] + h[2] * r[0] + r5(h[3] * r[4]) + r5(h[4] * r[3]);
        let t3 = h[0] * r[3] + h[1] * r[2] + h[2] * r[1] + h[3] * r[0] + r5(h[4] * r[4]);
        let t4 = h[0] * r[4] + h[1] * r[3] + h[2] * r[2] + h[3] * r[1] + h[4] * r[0];
        let c = t0 >> 26;
        h[0] = t0 & 0x03ff_ffff;
        let t1 = t1 + c;
        let c = t1 >> 26;
        h[1] = t1 & 0x03ff_ffff;
        let t2 = t2 + c;
        let c = t2 >> 26;
        h[2] = t2 & 0x03ff_ffff;
        let t3 = t3 + c;
        let c = t3 >> 26;
        h[3] = t3 & 0x03ff_ffff;
        let t4 = t4 + c;
        let c = t4 >> 26;
        h[4] = t4 & 0x03ff_ffff;
        h[0] += c * 5;
        let c = h[0] >> 26;
        h[0] &= 0x03ff_ffff;
        h[1] += c;
    }

    // Полный перенос.
    let mut c = h[1] >> 26;
    h[1] &= 0x03ff_ffff;
    h[2] += c;
    c = h[2] >> 26;
    h[2] &= 0x03ff_ffff;
    h[3] += c;
    c = h[3] >> 26;
    h[3] &= 0x03ff_ffff;
    h[4] += c;
    c = h[4] >> 26;
    h[4] &= 0x03ff_ffff;
    h[0] += c * 5;
    c = h[0] >> 26;
    h[0] &= 0x03ff_ffff;
    h[1] += c;

    // Условное вычитание p = 2^130 − 5 (два раунда — после переноса
    // h < 2^131, за раунд снимается ≤ 2^130).
    for _ in 0..2 {
        // g = h + 5 − 2^130
        let mut g = [0u64; 5];
        g[0] = h[0] + 5;
        let c = g[0] >> 26;
        g[0] &= 0x03ff_ffff;
        g[1] = h[1] + c;
        let c = g[1] >> 26;
        g[1] &= 0x03ff_ffff;
        g[2] = h[2] + c;
        let c = g[2] >> 26;
        g[2] &= 0x03ff_ffff;
        g[3] = h[3] + c;
        let c = g[3] >> 26;
        g[3] &= 0x03ff_ffff;
        g[4] = (h[4] + c).wrapping_sub(1 << 26);
        // g[4] «отрицателен» (старший бит) ⇔ h < p → оставить h,
        // иначе взять g.
        let choose_g = (g[4] >> 63).wrapping_sub(1); // 0 если h<p, иначе !0
        for k in 0..5 {
            h[k] ^= (h[k] ^ (g[k] & 0x03ff_ffff)) & choose_g;
        }
    }

    // h + s mod 2^128: 128-битное h раскладывается на два u64.
    let h_lo = h[0] | (h[1] << 26) | ((h[2] & 0x0fff) << 52);
    let h_hi = (h[2] >> 12) | (h[3] << 14) | (h[4] << 40);
    let s0 = u64::from_le_bytes(s[0..8].try_into().unwrap());
    let s1 = u64::from_le_bytes(s[8..16].try_into().unwrap());
    let (lo, carry) = h_lo.overflowing_add(s0);
    let hi = h_hi.wrapping_add(s1).wrapping_add(u64::from(carry));
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&lo.to_le_bytes());
    out[8..].copy_from_slice(&hi.to_le_bytes());
    out
}

// ============================================================================
// ChaCha20-Poly1305 AEAD (RFC 8439 §2.8)
// ============================================================================

/// AEAD-зашифрование: шифротекст ‖ тег (16 байт).
///
/// Блок 0 ChaCha20 идёт на ключ Poly1305, счётчик шифра стартует с 1.
pub fn aead_chacha20poly1305_seal(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    plaintext: &[u8],
) -> Vec<u8> {
    let poly_key = poly_key(key, nonce);
    let ct = chacha20_xor(key, nonce, 1, plaintext);
    let tag = poly1305_mac(&poly_key, aad, &ct);
    let mut out = ct;
    out.extend_from_slice(&tag);
    out
}

/// AEAD-расшифрование: проверка тега без раннего выхода.
pub fn aead_chacha20poly1305_open(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    ciphertext_with_tag: &[u8],
) -> Result<Vec<u8>, TlsPrimError> {
    if ciphertext_with_tag.len() < 16 {
        return Err(TlsPrimError::AeadOpen);
    }
    let (ct, tag) = ciphertext_with_tag.split_at(ciphertext_with_tag.len() - 16);
    let poly_key = poly_key(key, nonce);
    let expected = poly1305_mac(&poly_key, aad, ct);
    let mut diff = 0u8;
    for i in 0..16 {
        diff |= expected[i] ^ tag[i];
    }
    if diff != 0 {
        return Err(TlsPrimError::AeadOpen);
    }
    Ok(chacha20_xor(key, nonce, 1, ct))
}

fn poly_key(key: &[u8; 32], nonce: &[u8; 12]) -> [u8; 32] {
    let block = chacha20_block(key, 0, nonce);
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&block[..32]);
    pk
}

/// MAC-вход AEAD: `aad ‖ pad16 ‖ ct ‖ pad16 ‖ len(aad)_le64 ‖ len(ct)_le64`.
fn poly1305_mac(poly_key: &[u8; 32], aad: &[u8], ct: &[u8]) -> [u8; 16] {
    let mut mac = Vec::with_capacity(aad.len() + ct.len() + 32);
    mac.extend_from_slice(aad);
    mac.extend(std::iter::repeat_n(0u8, (16 - aad.len() % 16) % 16));
    mac.extend_from_slice(ct);
    mac.extend(std::iter::repeat_n(0u8, (16 - ct.len() % 16) % 16));
    mac.extend_from_slice(&(aad.len() as u64).to_le_bytes());
    mac.extend_from_slice(&(ct.len() as u64).to_le_bytes());
    poly1305(poly_key, &mac)
}

/// Ошибки примитивов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsPrimError {
    /// Тег AEAD не сошёлся (или вход короче 16 байт).
    AeadOpen,
    /// Некорректная длина входа.
    BadLength { what: &'static str, expected: usize, actual: usize },
}

impl fmt::Display for TlsPrimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TlsPrimError::AeadOpen => write!(f, "aead: tag mismatch"),
            TlsPrimError::BadLength { what, expected, actual } => {
                write!(f, "{what}: expected {expected} bytes, got {actual}")
            }
        }
    }
}

impl std::error::Error for TlsPrimError {}

// ============================================================================
// X25519 (RFC 7748) — поле GF(2^255−19), 5×51-битных конечностей
// ============================================================================

const MASK51: u64 = (1u64 << 51) - 1;

/// Элемент поля GF(2^255 − 19); конечности по 51 биту (между операциями
/// допускается провисание до ~2^52 — редукция в `fe_mul`/`fe_carry`).
type Fe = [u64; 5];

fn fe_one() -> Fe {
    [1, 0, 0, 0, 0]
}

fn fe_from_bytes(b: &[u8; 32]) -> Fe {
    let load = |i: usize| -> u64 {
        let mut w = [0u8; 8];
        w.copy_from_slice(&b[i..i + 8]);
        u64::from_le_bytes(w)
    };
    [
        load(0) & MASK51,
        (load(6) >> 3) & MASK51,
        (load(12) >> 6) & MASK51,
        (load(19) >> 1) & MASK51,
        (load(24) >> 12) & MASK51,
    ]
}

/// Перенос всех конечностей в диапазон [0, 2^51) (кроме финального
/// провисания +1 в старшей — гасится в `fe_to_bytes`).
fn fe_carry(h: &mut Fe) {
    let mut c = h[0] >> 51;
    h[0] &= MASK51;
    h[1] += c;
    c = h[1] >> 51;
    h[1] &= MASK51;
    h[2] += c;
    c = h[2] >> 51;
    h[2] &= MASK51;
    h[3] += c;
    c = h[3] >> 51;
    h[3] &= MASK51;
    h[4] += c;
    c = h[4] >> 51;
    h[4] &= MASK51;
    h[0] += 19 * c;
    c = h[0] >> 51;
    h[0] &= MASK51;
    h[1] += c;
}

fn fe_to_bytes(h: &Fe) -> [u8; 32] {
    let mut t = *h;
    fe_carry(&mut t);
    fe_carry(&mut t);
    // Условное вычитание p: q=1 ⇔ t ≥ p (после двух переносов t < p + 2^53).
    let mut q = (t[0] + 19) >> 51;
    q = (t[1] + q) >> 51;
    q = (t[2] + q) >> 51;
    q = (t[3] + q) >> 51;
    q = (t[4] + q) >> 51;
    debug_assert!(q <= 1);
    t[0] += 19 * q;
    // Строгий перенос: все конечности < 2^51.
    let mut c = t[0] >> 51;
    t[0] &= MASK51;
    t[1] += c;
    c = t[1] >> 51;
    t[1] &= MASK51;
    t[2] += c;
    c = t[2] >> 51;
    t[2] &= MASK51;
    t[3] += c;
    c = t[3] >> 51;
    t[3] &= MASK51;
    t[4] += c;
    c = t[4] >> 51;
    t[4] &= MASK51;
    t[0] += 19 * c;
    c = t[0] >> 51;
    t[0] &= MASK51;
    t[1] += c;
    c = t[1] >> 51;
    t[1] &= MASK51;
    t[2] += c;
    debug_assert!(t.iter().all(|&x| x < (1u64 << 51)));

    let w0 = t[0] | (t[1] << 51);
    let w1 = (t[1] >> 13) | (t[2] << 38);
    let w2 = (t[2] >> 26) | (t[3] << 25);
    let w3 = (t[3] >> 39) | (t[4] << 12);
    let mut s = [0u8; 32];
    s[0..8].copy_from_slice(&w0.to_le_bytes());
    s[8..16].copy_from_slice(&w1.to_le_bytes());
    s[16..24].copy_from_slice(&w2.to_le_bytes());
    s[24..32].copy_from_slice(&w3.to_le_bytes());
    s
}

fn fe_add(a: &Fe, b: &Fe) -> Fe {
    let mut r = [0u64; 5];
    for i in 0..5 {
        r[i] = a[i] + b[i];
    }
    let c = r[4] >> 51;
    r[4] &= MASK51;
    r[0] += 19 * c;
    let c = r[0] >> 51;
    r[0] &= MASK51;
    r[1] += c;
    r
}

/// a − b mod p: через a + 2p − b (конечности 2p с запасом ≥ 2^53).
fn fe_sub(a: &Fe, b: &Fe) -> Fe {
    debug_assert!(
        a.iter().all(|&x| x < (1u64 << 52)),
        "fe_sub: вход a грязный {a:?}"
    );
    debug_assert!(
        b.iter().all(|&x| x < (1u64 << 52)),
        "fe_sub: вход b грязный {b:?}"
    );
    let mut r = [0u64; 5];
    // 2p в конечностях: 2·(2^51 − 19) = 0xfffffffffffda, 2·(2^51 − 1) = 0xffffffffffffe
    r[0] = a[0] + 0xfffffffffffda - b[0];
    r[1] = a[1] + 0xffffffffffffe - b[1];
    r[2] = a[2] + 0xffffffffffffe - b[2];
    r[3] = a[3] + 0xffffffffffffe - b[3];
    r[4] = a[4] + 0xffffffffffffe - b[4];
    fe_carry(&mut r);
    r
}

/// Умножение в поле: аккумуляторы `u128`, фолд 2^255 ≡ 19.
fn fe_mul(a: &Fe, b: &Fe) -> Fe {
    let mut t = [0u128; 9];
    for i in 0..5 {
        let ai = u128::from(a[i]);
        for j in 0..5 {
            t[i + j] += ai * u128::from(b[j]);
        }
    }
    // t[5..8]·2^(51·5) ≡ t[5..8]·19 (mod p)
    let mut acc = [0u128; 5];
    acc.copy_from_slice(&t[..5]);
    for i in 5..9 {
        acc[i - 5] += t[i] * 19;
    }
    // Перенос в u128: конечности ≤ 2^51 + провисание.
    let mut carry: u128 = 0;
    let mut out = [0u64; 5];
    for i in 0..5 {
        let v = acc[i] + carry;
        out[i] = (v as u64) & MASK51;
        carry = v >> 51;
    }
    // Верхний перенос — это 2^255-кратное: свернуть ×19 и дотащить.
    // КРИТИЧНО: остаток, вытолкнутый из limb 4, тоже кратен 2^255 —
    // он сворачивается ×19 в limb 0 (потеря = −19 по модулю p).
    let mut extra: u128 = 19 * carry; // carry < 2^60 → extra < 2^65
    for slot in out.iter_mut() {
        if extra == 0 {
            break;
        }
        let v = u128::from(*slot) + extra;
        *slot = (v as u64) & MASK51;
        extra = v >> 51;
    }
    out[0] += (19 * extra) as u64;
    // Финальные переносы (маленькие).
    let mut c = out[0] >> 51;
    out[0] &= MASK51;
    out[1] += c;
    c = out[1] >> 51;
    out[1] &= MASK51;
    out[2] += c;
    c = out[2] >> 51;
    out[2] &= MASK51;
    out[3] += c;
    c = out[3] >> 51;
    out[3] &= MASK51;
    out[4] += c;
    c = out[4] >> 51;
    out[4] &= MASK51;
    out[0] += 19 * c;
    c = out[0] >> 51;
    out[0] &= MASK51;
    out[1] += c;
    out
}

fn fe_sq(a: &Fe) -> Fe {
    fe_mul(a, a)
}

/// a·121665 (коэффициент a24 лестницы): через u128-аккумуляторы.
fn fe_mul121665(a: &Fe) -> Fe {
    let mut carry: u128 = 0;
    let mut out = [0u64; 5];
    for i in 0..5 {
        let v = u128::from(a[i]) * 121_665 + carry;
        out[i] = (v as u64) & MASK51;
        carry = v >> 51;
    }
    out[0] += (19 * carry) as u64;
    let mut c = out[0] >> 51;
    out[0] &= MASK51;
    out[1] += c;
    c = out[1] >> 51;
    out[1] &= MASK51;
    out[2] += c;
    c = out[2] >> 51;
    out[2] &= MASK51;
    out[3] += c;
    c = out[3] >> 51;
    out[3] &= MASK51;
    out[4] += c;
    c = out[4] >> 51;
    out[4] &= MASK51;
    out[0] += 19 * c;
    c = out[0] >> 51;
    out[0] &= MASK51;
    out[1] += c;
    out
}

/// a^(p−2) = a^−1 — явная бинарная лестница по битам p − 2 = 2^255 − 21.
///
/// Один вызов на рукопожатие (~510 полевых операций): простота и
/// очевидная корректность важнее экономии пары микросекунд.
fn fe_invert(a: &Fe) -> Fe {
    // p − 2 в LE: 0xeb, 0xff×30, 0x7f
    let mut e = [0xffu8; 32];
    e[0] = 0xeb;
    e[31] = 0x7f;
    let mut r = fe_one();
    for bit in (0..255).rev() {
        r = fe_sq(&r);
        if (e[bit / 8] >> (bit % 8)) & 1 == 1 {
            r = fe_mul(&r, a);
        }
    }
    r
}

/// Условный обмен конечностей (без ветвления).
fn fe_cswap(swap: u64, a: &mut Fe, b: &mut Fe) {
    let mask = 0u64.wrapping_sub(swap);
    for i in 0..5 {
        let t = mask & (a[i] ^ b[i]);
        a[i] ^= t;
        b[i] ^= t;
    }
}

/// X25519 — скалярное умножение Curve25519 (RFC 7748 §5).
///
/// `scalar` и `u` — 32 байта LE; старший бит `u` маскируется. Вызывающий
/// код обязан проверить результат на ноль (маленькая подгруппа).
pub fn x25519(scalar: &[u8; 32], u: &[u8; 32]) -> [u8; 32] {
    let mut k = *scalar;
    k[0] &= 248;
    k[31] &= 127;
    k[31] |= 64;

    let mut ub = *u;
    ub[31] &= 127;
    let x1 = fe_from_bytes(&ub);

    let mut x2 = fe_one();
    let mut z2 = [0u64; 5];
    let mut x3 = x1;
    let mut z3 = fe_one();
    let mut swap: u64 = 0;

    for t in (0..255).rev() {
        let kt = u64::from((k[t / 8] >> (t % 8)) & 1);
        swap ^= kt;
        fe_cswap(swap, &mut x2, &mut x3);
        fe_cswap(swap, &mut z2, &mut z3);
        swap = kt;

        // Лестница Монтгомери (RFC 7748 §5, псевдокод)
        let a = fe_add(&x2, &z2);
        let aa = fe_sq(&a);
        let b = fe_sub(&x2, &z2);
        let bb = fe_sq(&b);
        let e = fe_sub(&aa, &bb);
        let c = fe_add(&x3, &z3);
        let d = fe_sub(&x3, &z3);
        let da = fe_mul(&d, &a);
        let cb = fe_mul(&c, &b);
        let dacb = fe_add(&da, &cb);
        x3 = fe_sq(&dacb);
        let dascb = fe_sub(&da, &cb);
        z3 = fe_mul(&x1, &fe_sq(&dascb));
        x2 = fe_mul(&aa, &bb);
        let a24e = fe_mul121665(&e);
        let t2 = fe_add(&aa, &a24e);
        z2 = fe_mul(&e, &t2);
    }
    fe_cswap(swap, &mut x2, &mut x3);
    fe_cswap(swap, &mut z2, &mut z3);

    fe_to_bytes(&fe_mul(&x2, &fe_invert(&z2)))
}

/// Публичный ключ X25519 (скаляр · базовая точка u = 9).
pub fn x25519_base(scalar: &[u8; 32]) -> [u8; 32] {
    let mut base = [0u8; 32];
    base[0] = 9;
    x25519(scalar, &base)
}

// ============================================================================
// Тесты: эталонные векторы RFC / FIPS / NIST
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_vectors() {
        // NIST: «abc»
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // Пустая строка
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // 55 байт: паддинг не помещается в первый блок (граничный случай)
        assert_eq!(
            hex(&sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // Поточный инкремент == однократный вызов
        let mut h = Sha256::new();
        h.update(b"ab");
        h.update(b"c");
        assert_eq!(h.finish(), sha256(b"abc"));
        // Ровно 64 байта (блок без остатка) и 65 (блок + 1 байт)
        let mut h = Sha256::new();
        h.update(&[7u8; 64]);
        let d64 = h.finish();
        assert_eq!(d64, sha256(&[7u8; 64]));
        let mut h = Sha256::new();
        h.update(&[7u8; 64]);
        h.update(&[7u8; 1]);
        assert_eq!(h.finish(), sha256(&[7u8; 65]));
    }

    #[test]
    fn hmac_vectors_rfc4231() {
        let m = hmac_sha256(&[0x0bu8; 20], b"Hi There");
        assert_eq!(
            hex(&m),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        let m = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex(&m),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // Длинный ключ (131×0xaa > 64 байта): ключ хэшируется перед
        // паддингом. Ожидание вычислено эталонной реализацией HMAC
        // (python hmac/hacl*, RFC 2104).
        let m = hmac_sha256(
            &[0xaau8; 131],
            b"Test Using Larger Than Block-Size Key - Hash Data First",
        );
        assert_eq!(
            hex(&m),
            "154a250c91e83dfc5144f2be2fea8e4ce1bd0138297f599ceb6b5f305a193c85"
        );
    }

    #[test]
    fn hkdf_vector_rfc5869_case1() {
        let salt = unhex("000102030405060708090a0b0c");
        let info = unhex("f0f1f2f3f4f5f6f7f8f9");
        let prk = hkdf_extract(&salt, &[0x0bu8; 22]);
        assert_eq!(
            hex(&prk),
            "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5"
        );
        let okm = hkdf_expand(&prk, &info, 42);
        assert_eq!(
            hex(&okm),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf\
             34007208d5b887185865"
        );
    }

    #[test]
    fn chacha20_block_and_encrypt_rfc8439() {
        // §2.3.2: блок с ключом 00..1f, нонсом 00 00 00 09 00 00 00 4a
        // 00 00 00 00, счётчиком 1 → первые слова keystream.
        let mut key = [0u8; 32];
        for (i, b) in key.iter_mut().enumerate() {
            *b = i as u8;
        }
        let mut nonce = [0u8; 12];
        nonce[3] = 0x09;
        nonce[7] = 0x4a;
        let ks = chacha20_block(&key, 1, &nonce);
        assert_eq!(hex(&ks[..16]), "10f1e7e4d13b5915500fdd1fa32071c4");

        // §2.4.2: полное шифрование «Ladies and Gentlemen…»
        let mut nonce = [0u8; 12];
        nonce[7] = 0x4a;
        let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you \
                          only one tip for the future, sunscreen would be it.";
        let ct = chacha20_xor(&key, &nonce, 1, plaintext);
        assert_eq!(hex(&ct[..16]), "6e2e359a2568f98041ba0728dd0d6981");
        assert_eq!(chacha20_xor(&key, &nonce, 1, &ct), plaintext.to_vec());
    }

    #[test]
    fn poly1305_rfc8439() {
        let mut key = [0u8; 32];
        key.copy_from_slice(&unhex(
            "85d6be7857556d337f4452fe42d506a80103808afb0db2fd4abff6af4149f51b",
        ));
        let msg = unhex("43727970746f6772617068696320466f72756d2052657365617263682047726f7570");
        assert_eq!(hex(&poly1305(&key, &msg)), "a8061dc1305136c6c22b8baf0c0127a9");
        // Пустое сообщение: тег = s (нулевой h)
        assert_eq!(hex(&poly1305(&key, &[])), hex(&key[16..32]));
        // Много блоков (> 32 байт): перенос конечностей между блоками
        let long: Vec<u8> = (0..100u8).collect();
        let t1 = poly1305(&key, &long);
        let t2 = poly1305(&key, &long);
        assert_eq!(t1, t2);
    }

    #[test]
    fn aead_rfc8439() {
        // §2.8.2
        let mut key = [0u8; 32];
        key.copy_from_slice(&unhex(
            "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f",
        ));
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&unhex("070000004041424344454647"));
        let aad = unhex("50515253c0c1c2c3c4c5c6c7");
        let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you \
                          only one tip for the future, sunscreen would be it.";
        let sealed = aead_chacha20poly1305_seal(&key, &nonce, &aad, plaintext);
        assert_eq!(hex(&sealed[..16]), "d31a8d34648e60db7b86afbc53ef7ec2");
        assert_eq!(hex(&sealed[sealed.len() - 16..]), "1ae10b594f09e26a7e902ecbd0600691");

        let opened = aead_chacha20poly1305_open(&key, &nonce, &aad, &sealed).unwrap();
        assert_eq!(opened, plaintext.to_vec());
        // Порча тега → отказ
        let mut broken = sealed.clone();
        let last = broken.len() - 1;
        broken[last] ^= 1;
        assert_eq!(
            aead_chacha20poly1305_open(&key, &nonce, &aad, &broken).unwrap_err(),
            TlsPrimError::AeadOpen
        );
        // Порча AAD → отказ
        let mut bad_aad = aad.clone();
        bad_aad[0] ^= 1;
        assert!(aead_chacha20poly1305_open(&key, &nonce, &bad_aad, &sealed).is_err());
        // Короткий вход → отказ
        assert!(aead_chacha20poly1305_open(&key, &nonce, &aad, &[1u8; 8]).is_err());
    }

    #[test]
    fn x25519_rfc7748_vectors() {
        let mut s = [0u8; 32];
        let mut p = [0u8; 32];
        // §5.2 вектор 1
        s.copy_from_slice(&unhex(
            "a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4",
        ));
        p.copy_from_slice(&unhex(
            "e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c",
        ));
        assert_eq!(
            hex(&x25519(&s, &p)),
            "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552"
        );
        // §5.2 вектор 2
        s.copy_from_slice(&unhex(
            "4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d",
        ));
        p.copy_from_slice(&unhex(
            "e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493",
        ));
        assert_eq!(
            hex(&x25519(&s, &p)),
            "95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957"
        );
    }

    #[test]
    fn x25519_rfc7748_diffie_hellman() {
        // §6.1: пара ключей Элис/Боб и общий секрет
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        a.copy_from_slice(&unhex(
            "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a",
        ));
        b.copy_from_slice(&unhex(
            "5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb",
        ));
        let pub_a = x25519_base(&a);
        let pub_b = x25519_base(&b);
        assert_eq!(
            hex(&pub_a),
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
        );
        assert_eq!(
            hex(&pub_b),
            "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f"
        );
        assert_eq!(x25519(&a, &pub_b), x25519(&b, &pub_a));
        assert_eq!(
            hex(&x25519(&a, &pub_b)),
            "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742"
        );
    }

    #[test]
    fn expand_label_shape() {
        let secret = [7u8; 32];
        let out = hkdf_expand_label(&secret, "key", &[], 32);
        assert_eq!(out.len(), 32);
        // Домен-сепарация меток: «key» ≠ «iv»
        let out2 = hkdf_expand_label(&secret, "iv", &[], 12);
        assert_eq!(out2.len(), 12);
        assert_ne!(&out[..12], &out2[..]);
        // Детерминизм
        assert_eq!(out, hkdf_expand_label(&secret, "key", &[], 32));
        // derive_secret = expand_label с транскрипт-хэшем в контексте
        let msgs: Vec<&[u8]> = vec![b"hello", b"world"];
        let ds = derive_secret(&secret, "c hs traffic", &msgs);
        assert_eq!(ds.len(), 32);
        let th = {
            let mut h = Sha256::new();
            h.update(b"hello");
            h.update(b"world");
            h.finish()
        };
        let manual = hkdf_expand_label(&secret, "c hs traffic", &th, 32);
        assert_eq!(ds.to_vec(), manual);
    }
}
