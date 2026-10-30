//! Минимальный SHA-256 (FIPS 180-4) без внешних зависимостей.
//!
//! Используется для digest payload (топология + фазовые блоки),
//! усечённого до 24 байт. Обработка потоковая — копий буфера нет.

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Один раунд сжатия над 64-байтовым блоком.
fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[4 * i],
            block[4 * i + 1],
            block[4 * i + 2],
            block[4 * i + 3],
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

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
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
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

/// SHA-256 от среза (потоковая обработка, без дополнительной аллокации).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize()
}

/// Потоковый SHA-256 (FIPS 180-4): update() произвольными кусками,
/// finalize() один раз. Память O(1) — ровно 64-байтовый буфер блока,
/// поэтому внешний digest контейнеров любого размера (байты … гигабайты)
/// считается без загрузки файла в RAM.
///
/// M6 (VaultAppender): `Clone` — снапшот состояния внешнего digest в
/// точке «последняя полная страница»; поля — plain-значения, копирование
/// тривиально безопасно (никаких указателей/ресурсов).
#[derive(Clone)]
pub struct Sha256 {
    state: [u32; 8],
    buf: [u8; 64],
    buflen: usize,
    total: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    /// Инициализировать с стандартными константами IV.
    pub fn new() -> Self {
        Sha256 { state: H0, buf: [0u8; 64], buflen: 0, total: 0 }
    }

    /// Подать очередной кусок данных (любой длины, включая 0).
    pub fn update(&mut self, mut data: &[u8]) {
        self.total = self.total.wrapping_add(data.len() as u64);
        if self.buflen > 0 {
            let need = 64 - self.buflen;
            let take = need.min(data.len());
            self.buf[self.buflen..self.buflen + take].copy_from_slice(&data[..take]);
            self.buflen += take;
            data = &data[take..];
            if self.buflen == 64 {
                let block: [u8; 64] = self.buf;
                compress(&mut self.state, &block);
                self.buflen = 0;
            }
        }
        let mut chunks = data.chunks_exact(64);
        for block in &mut chunks {
            compress(&mut self.state, block.try_into().unwrap());
        }
        let rem = chunks.remainder();
        if !rem.is_empty() {
            self.buf[..rem.len()].copy_from_slice(rem);
            self.buflen = rem.len();
        }
    }

    /// Завершить и снять 32-байтовый digest (self больше не используется).
    pub fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total.wrapping_mul(8);
        // Паддинг: 0x80, нули, длина в битах (big-endian).
        let mut tail = [0u8; 128];
        tail[..self.buflen].copy_from_slice(&self.buf[..self.buflen]);
        tail[self.buflen] = 0x80;
        let len_at = if self.buflen < 56 { 56 } else { 120 };
        tail[len_at..len_at + 8].copy_from_slice(&bit_len.to_be_bytes());
        for block in tail[..len_at + 8].as_chunks::<64>().0 {
            compress(&mut self.state, block.try_into().unwrap());
        }

        let mut out = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

/// Первые 24 байта SHA-256 — digest payload в формате `.pqw`.
pub fn sha256_trunc24(data: &[u8]) -> [u8; 24] {
    let full = sha256(data);
    let mut out = [0u8; 24];
    out.copy_from_slice(&full[..24]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn sha256_empty() {
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_abc() {
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_fox() {
        assert_eq!(
            hex(&sha256(b"The quick brown fox jumps over the lazy dog")),
            "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592"
        );
    }

    #[test]
    fn sha256_two_blocks() {
        // Классический NIST-вектор: сообщение 448 бит -> два блока сжатия.
        assert_eq!(
            hex(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn trunc24_is_prefix() {
        let d = sha256(b"poler");
        let t = sha256_trunc24(b"poler");
        assert_eq!(&d[..24], &t[..]);
    }

    #[test]
    fn streaming_equals_oneshot() {
        // Куски произвольной (не кратной 64) длины — digest идентичен one-shot.
        let data: Vec<u8> = (0..100_000u32).map(|i| (i * 7 + 13) as u8).collect();
        for &chunk_len in &[1usize, 7, 63, 64, 65, 4096, 65537] {
            let mut h = Sha256::new();
            for piece in data.chunks(chunk_len) {
                h.update(piece);
            }
            assert_eq!(h.finalize(), sha256(&data), "chunk_len={chunk_len}");
        }
    }

    #[test]
    fn streaming_empty_updates() {
        let mut h = Sha256::new();
        h.update(b"");
        h.update(b"abc");
        h.update(b"");
        assert_eq!(h.finalize(), sha256(b"abc"));
    }

    #[test]
    fn streaming_boundary_padding() {
        // Сообщения на границе правила паддинга (55/56/63/64 байта).
        for n in [55usize, 56, 63, 64, 119, 120, 127, 128] {
            let data = vec![0xA5u8; n];
            let mut h = Sha256::new();
            h.update(&data[..n / 2]);
            h.update(&data[n / 2..]);
            assert_eq!(h.finalize(), sha256(&data), "n={n}");
        }
    }
}
