//! PndHasher — суверенный 256-битный потоковый хеш над ядром PND v8.2.
//!
//! Назначение (крипто-слой данных, M4.5): криптография здесь — не только
//! «спрятать от хакера», но и **инструмент работы с данными**:
//!
//! - **Контентная адресация**: детерминированный дайджест любого чанка/
//!   страницы/файла — одинаковые данные дают одинаковый ключ кеша на
//!   любой машине (дедупликация, кеш-индексы, git-синхронизация памяти);
//! - **Целостность фактов**: однобитовое изменение входа меняет весь
//!   дайджест — подмена цитаты/лога/вектора обнаруживается мгновенно;
//! - **Ключевой MAC**: режим `new_keyed` превращает хешер в код
//!   аутентичности с секретным ключом (внутренняя проверка Vault).
//!
//! ## Конструкция
//!
//! Merkle-Damgård с двухлейновым сжатием (блок 16 байт = 4 слова):
//!
//! ```text
//! l = E_key(state_L ⊕ block)            — полный шифр: 20 Feistel-раундов
//! r_i = pndMix(state_R_i ⊕ l_i, block_j ⊕ l_k, ε)   — Φ-диффузия
//! state = [ r ‖ l ⊕ state_L ]            — лейны меняются ролями
//! ```
//!
//! Левая лейна даёт криптографическую стойкость (PND v8.2 с 256-битным
//! расписанием ключей), правая — суверенную математику pndMix/Φ и скорость.
//! Финализация — паддинг 0x80 + нули + длина в байтах (u64 LE), как у SHA-2.
//!
//! Память O(1): 16-байтовый буфер блока — потоковая обработка файлов
//! любого размера (от 500 КБ до сотен ГБ) без загрузки в RAM.

use super::pnd::{pnd_mix, phi, PolerCipher, PolerDrbg, KEY_WORDS};

/// Базовая ε для диффузии правой лейны (нечётная — обратимый pndMix).
const LANE_EPSILON: u32 = 0x9E3779B9;

/// Домен по умолчанию для контентной адресации (публичный, детерминированный).
pub const DOMAIN_CONTENT: &str = "POLER.PNDHASHER.CONTENT.v1";

/// Домен внутренней проверки целостности Vault (ключевой MAC листьев страниц).
pub const DOMAIN_VAULT_MAC: &str = "POLER.PNDHASHER.VAULT_MAC.v1";

/// Домен цепочки Vault (ключевой фолд листьев — порядок страниц).
pub const DOMAIN_VAULT_CHAIN: &str = "POLER.PNDHASHER.VAULT_CHAIN.v1";

/// M6 (VaultAppender): снапшот plain-состояния [`PndHasher`].
/// Переносим между вызовами/коммитами; шифр восстанавливается
/// из ключа детерминированно (`PndHasher::resume`).
#[derive(Clone, Debug)]
pub struct PndHasherParts {
    pub state: [u32; KEY_WORDS],
    pub buf: [u8; 16],
    pub buflen: usize,
    pub total: u64,
}

/// Потоковый 256-битный хешer над крипто-ядром PND.
pub struct PndHasher {
    cipher: PolerCipher,
    state: [u32; KEY_WORDS],
    buf: [u8; 16],
    buflen: usize,
    total: u64,
}

impl PndHasher {
    /// Публичный детерминированный хешер (контентная адресация):
    /// одинаковый вход → одинаковый дайджест на любой машине.
    pub fn new_content() -> Self {
        Self::new_keyed(&domain_seed(DOMAIN_CONTENT), DOMAIN_CONTENT)
    }

    /// Ключевой хешер (MAC): дайджест зависит от секретного ключа —
    /// без ключа подделать нельзя, даже зная весь открытый текст.
    pub fn new_keyed(key: &[u32; KEY_WORDS], domain: &str) -> Self {
        // ε домена: любое нечётное значение, производное от ключа+домена.
        let mut eps_acc = key[0] ^ key[4];
        for b in domain.bytes() {
            eps_acc = phi(eps_acc ^ (b as u32));
        }
        let epsilon = eps_acc | 1;

        let cipher = PolerCipher::new(key, epsilon).expect("PND cipher alloc");
        // Начальное состояние — DRBG-поток из (ключ ⊕ домен)-свёртки:
        // домен разводит потоки разных применений даже при одном ключе.
        let mut seed = *key;
        for (i, b) in domain.bytes().enumerate() {
            seed[i % KEY_WORDS] =
                phi(seed[i % KEY_WORDS] ^ (b as u32)).rotate_left((i % 31) as u32);
        }
        let mut drbg = PolerDrbg::new(&seed).expect("PND drbg alloc");
        let state = core::array::from_fn(|_| drbg.next());
        Self { cipher, state, buf: [0u8; 16], buflen: 0, total: 0 }
    }

    /// Подать очередной кусок данных (любой длины).
    pub fn update(&mut self, mut data: &[u8]) {
        self.total = self.total.wrapping_add(data.len() as u64);

        if self.buflen > 0 {
            let need = 16 - self.buflen;
            let take = need.min(data.len());
            self.buf[self.buflen..self.buflen + take].copy_from_slice(&data[..take]);
            self.buflen += take;
            data = &data[take..];
            if self.buflen == 16 {
                let block: [u8; 16] = self.buf;
                self.compress(block);
                self.buflen = 0;
            }
        }

        let mut chunks = data.chunks_exact(16);
        for block in &mut chunks {
            let mut b = [0u8; 16];
            b.copy_from_slice(block);
            self.compress(b);
        }
        let rem = chunks.remainder();
        if !rem.is_empty() {
            self.buf[..rem.len()].copy_from_slice(rem);
            self.buflen = rem.len();
        }
    }

    /// Завершить и снять 256-битный дайджест (8 слов, little-endian
    /// при сериализации). Self после вызова не используется.
    pub fn finalize(mut self) -> [u32; KEY_WORDS] {
        // Паддинг: 0x80, нули, длина в БАЙТАХ u64 LE (документировано;
        // отличается от битовой длины SHA-2 — это другой домен).
        let mut tail = [0u8; 32];
        tail[..self.buflen].copy_from_slice(&self.buf[..self.buflen]);
        tail[self.buflen] = 0x80;
        tail[24..32].copy_from_slice(&self.total.to_le_bytes());
        for block in tail.chunks_exact(16) {
            let mut b = [0u8; 16];
            b.copy_from_slice(block);
            self.compress(b);
        }
        self.state
    }

    /// One-shot дайджест буфера.
    pub fn digest(data: &[u8]) -> [u32; KEY_WORDS] {
        let mut h = Self::new_content();
        h.update(data);
        h.finalize()
    }

    /// M6 (VaultAppender): снапшот plain-состояния хешера. Шифр НЕ входит
    /// в снапшот (владеет Zig-handle) — он пересоздаётся детерминированно
    /// из того же ключа и ε домена через [`Self::resume`]. Снапшот + живой
    /// шифр эквивалентны клону: состояние резюмируется побайтово.
    pub fn parts(&self) -> PndHasherParts {
        PndHasherParts {
            state: self.state,
            buf: self.buf,
            buflen: self.buflen,
            total: self.total,
        }
    }

    /// M6: восстановить хешер из снапшота. `cipher` обязан быть создан
    /// тем же ключом и доменом (ε выводится из ключа+домена в
    /// `new_keyed` — пере-вывод здесь детерминирован).
    pub fn resume(parts: &PndHasherParts, key: &[u32; KEY_WORDS], domain: &str) -> Self {
        // Точная копия ε-вывода new_keyed: тот же аккмулятор.
        let mut eps_acc = key[0] ^ key[4];
        for b in domain.bytes() {
            eps_acc = phi(eps_acc ^ (b as u32));
        }
        let epsilon = eps_acc | 1;
        let cipher = PolerCipher::new(key, epsilon).expect("возобновление шифра: выделение контекста");
        Self {
            cipher,
            state: parts.state,
            buf: parts.buf,
            buflen: parts.buflen,
            total: parts.total,
        }
    }

    /// One-shot ключевой MAC буфера.
    pub fn mac(key: &[u32; KEY_WORDS], domain: &str, data: &[u8]) -> [u32; KEY_WORDS] {
        let mut h = Self::new_keyed(key, domain);
        h.update(data);
        h.finalize()
    }

    /// Дайджест в hex (для CLI/логов).
    pub fn hex(words: &[u32; KEY_WORDS]) -> String {
        let mut s = String::with_capacity(KEY_WORDS * 8);
        for w in words {
            s.push_str(&format!("{w:08x}"));
        }
        s
    }

    /// Сжатие одного 16-байтового блока (ядро конструкции, см. модуль).
    fn compress(&mut self, block: [u8; 16]) {
        let m = [
            u32::from_le_bytes([block[0], block[1], block[2], block[3]]),
            u32::from_le_bytes([block[4], block[5], block[6], block[7]]),
            u32::from_le_bytes([block[8], block[9], block[10], block[11]]),
            u32::from_le_bytes([block[12], block[13], block[14], block[15]]),
        ];

        // Левая лейна: полный шифр PND (Feistel ×20) над state_L ⊕ block.
        let l_in = [
            self.state[0] ^ m[0],
            self.state[1] ^ m[1],
            self.state[2] ^ m[2],
            self.state[3] ^ m[3],
        ];
        let l = self.cipher.encrypt_block(&l_in);

        // Правая лейна: pndMix/Φ-диффузия, втянутая в выход шифра.
        let r = [
            pnd_mix(self.state[4] ^ l[0], m[0] ^ l[1], LANE_EPSILON),
            pnd_mix(self.state[5] ^ l[1], m[1] ^ l[2], LANE_EPSILON),
            pnd_mix(self.state[6] ^ l[2], m[2] ^ l[3], LANE_EPSILON),
            pnd_mix(self.state[7] ^ l[3], m[3] ^ l[0], LANE_EPSILON),
        ];

        // Обмен ролями лейнов + Davies-Meyer-фидфорвард левой.
        let old_l = [self.state[0], self.state[1], self.state[2], self.state[3]];
        self.state[0] = r[0];
        self.state[1] = r[1];
        self.state[2] = r[2];
        self.state[3] = r[3];
        self.state[4] = l[0] ^ old_l[0];
        self.state[5] = l[1] ^ old_l[1];
        self.state[6] = l[2] ^ old_l[2];
        self.state[7] = l[3] ^ old_l[3];
    }
}

/// Свёртка доменной строки в 8 слов (публичный «ключ» для контентного режима).
/// M6: публична — VaultAppender возобновляет content_id-цепь с тем же ключом.
pub fn domain_seed(domain: &str) -> [u32; KEY_WORDS] {
    let mut seed = [0u32; KEY_WORDS];
    for (i, b) in domain.bytes().enumerate() {
        let j = i % KEY_WORDS;
        seed[j] = phi(seed[j].wrapping_add((b as u32) << ((i % 5) * 6)));
    }
    seed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Детерминизм: одинаковый вход → одинаковый дайджест.
    #[test]
    fn deterministic() {
        let a = PndHasher::digest(b"poler-engine crypto data layer");
        let b = PndHasher::digest(b"poler-engine crypto data layer");
        assert_eq!(a, b);
    }

    /// Разные входы → разные дайджесты (в т.ч. разной длины).
    #[test]
    fn distinct_inputs() {
        let mut digests = std::collections::HashSet::new();
        let inputs: [&[u8]; 12] = [
            b"", b"a", b"b", b"ab", b"ba", b"poler", b"POLER",
            b"\x00", b"\x00\x00", &[0u8; 16], &[0xFFu8; 16],
            b"The quick brown fox jumps over the lazy dog",
        ];
        for inp in inputs {
            assert!(digests.insert(PndHasher::hex(&PndHasher::digest(inp))),
                "коллизия на входе {:?} (len={})", String::from_utf8_lossy(&inp[..inp.len().min(16)]), inp.len());
        }
    }

    /// Лавина: однобитовый флип меняет ВСЕ слова дайджеста.
    #[test]
    fn avalanche_one_bit() {
        let base = b"sovereign hippocampus memory cartridge v1";
        let d0 = PndHasher::digest(base);
        for bit in [0usize, 1, 7, 8, 63, 128, 255, 311] {
            let mut flipped = base.to_vec();
            flipped[bit / 8] ^= 1 << (bit % 8);
            let d1 = PndHasher::digest(&flipped);
            for w in 0..KEY_WORDS {
                assert_ne!(d0[w], d1[w], "бит {bit}: слово {w} не изменилось");
            }
        }
    }

    /// Стриминг кусками любой длины == one-shot.
    #[test]
    fn streaming_equals_oneshot() {
        let data: Vec<u8> = (0..50_000u32).map(|i| (i * 13 + 5) as u8).collect();
        let oneshot = PndHasher::digest(&data);
        for &chunk in &[1usize, 3, 15, 16, 17, 4096, 65537] {
            let mut h = PndHasher::new_content();
            for piece in data.chunks(chunk) {
                h.update(piece);
            }
            assert_eq!(h.finalize(), oneshot, "chunk={chunk}");
        }
    }

    /// Ключевой MAC: другой ключ → другой дайджест; тот же ключ → тот же.
    #[test]
    fn keyed_mac_domain_separation() {
        let data = b"vault page payload";
        let k1 = [1u32; KEY_WORDS];
        let k2 = [2u32; KEY_WORDS];
        let m1 = PndHasher::mac(&k1, DOMAIN_VAULT_MAC, data);
        assert_eq!(m1, PndHasher::mac(&k1, DOMAIN_VAULT_MAC, data));
        assert_ne!(m1, PndHasher::mac(&k2, DOMAIN_VAULT_MAC, data));
        // Разные домены при одном ключе → разные потоки.
        assert_ne!(m1, PndHasher::mac(&k1, DOMAIN_CONTENT, data));
        // Контентный (публичный) не равен ключевому.
        assert_ne!(PndHasher::digest(data), m1);
    }

    /// Пустой вход валиден (важно для edge-case Vault).
    #[test]
    fn empty_input() {
        let d = PndHasher::digest(b"");
        assert_ne!(d, [0u32; KEY_WORDS]);
        assert_ne!(d, PndHasher::digest(b"\x80"));
    }

    /// Дайджест длинного входа стабилен между вызовами (регрессия
    /// на перестройку лейнов после 2^32 байт невозможна — total u64).
    #[test]
    fn long_input_stability() {
        let data: Vec<u8> = (0..1_000_000u32).map(|i| (i >> 8) as u8).collect();
        let a = PndHasher::digest(&data);
        let b = PndHasher::digest(&data);
        assert_eq!(a, b);
    }

    /// M6: resume(parts) побайтово эквивалентен продолжению живого
    /// хешера — фундамент снапшотов MAC-цепи VaultAppender.
    #[test]
    fn resume_equivalence() {
        let key = [1u32, 2, 3, 4, 5, 6, 7, 8];
        let domain = DOMAIN_VAULT_CHAIN;
        let mut live = PndHasher::new_keyed(&key, domain);
        live.update("POLER-LOG-STREAM: первая пачка байт страницы 0..N".as_bytes());

        // снапшот в середине потока
        let snap = live.parts();

        // путь 1: живой хешер продолжает
        live.update(" + вторая пачка (события лога, коммит 2)".as_bytes());
        let direct = live.finalize();

        // путь 2: resume из снапшота + та же вторая пачка
        let mut resumed = PndHasher::resume(&snap, &key, domain);
        resumed.update(" + вторая пачка (события лога, коммит 2)".as_bytes());
        let resumed_digest = resumed.finalize();

        assert_eq!(direct, resumed_digest, "resume обязан побайтово совпадать с живым продолжением");

        // снапшот не мутировал: повторный resume даёт тот же результат
        let mut again = PndHasher::resume(&snap, &key, domain);
        again.update(" + вторая пачка (события лога, коммит 2)".as_bytes());
        assert_eq!(resumed_digest, again.finalize(), "снапшот переиспользуем");
    }
}
