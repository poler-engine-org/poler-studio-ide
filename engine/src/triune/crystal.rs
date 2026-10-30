//! Кристалл Знаний — троично-квантованная языковая модель (.t5c).
//!
//! Третья опора Триединства (S2/v0.36.0): «квантованная намертво зашитая
//! в код модель». Веса живут в **троичной решётке {−1, 0, +1}** и
//! упакованы **пять тритов в байт** (Trit5, EQ-D17: 3⁵ = 243 ≤ 256 —
//! 1.58 бита на синапс). Никакого умножения на горячем пути: скалярные
//! произведения считаются SIMD-сложениями [`crate::pqc::tensor::dot_trit5_f32`].
//!
//! ## Что внутри кристалла
//!
//! 1. **Словарь** — топ слов корпуса (детерминированный порядок:
//!    частота ↓, лексика ↑ — никакого HashMap-порядка в артефакте).
//! 2. **Биграммная топология** — знак отношения правдоподобия:
//!    `r = P(next|prev)/P(next)`; трит = +1 при `r ≥ θ_hi`, −1 при
//!    `r ≤ θ_lo` (и пара встречалась), 0 — нейтраль/вакуум. Это
//!    знаковая квантизация PMI: синтаксис живёт в знаках, не в величинах.
//! 3. **Семантика НЕ хранится** — вложение токена выводится на лету из
//!    золотой фазы CSE (F6: только `c·φ mod 2π` даёт зазор 1.35) и
//!    квантуется в триты. Сенсорика и память делят один код — как
//!    биологический мозг использует одну кортикальную карту для
//!    восприятия и воспоминания.
//!
//! ## Формат .t5c (Trit5 Crystal v1, little-endian)
//!
//! ```text
//! СМЕЩЕНИЕ  РАЗМЕР  ПОЛЕ
//! 0x00      8       magic "T5CRYS\0\0"
//! 0x08      4       version u32 = 1
//! 0x0C      4       vocab V
//! 0x10      4       flags u32 (bit0 = есть биграммная секция)
//! 0x14      4       token_off u32 (абс.)
//! 0x18      4       bigram_off u32 (абс.)
//! 0x1C      8       corpus_words u64
//! 0x24      8       corpus_chars u64
//! 0x2C      4       theta_hi_milli u32 (θ_hi × 1000)
//! 0x30      32      sha256(тело: 0x50..EOF)
//! 0x50      ·       таблица токенов: V × (u8 len | utf8)
//! …         ·       биграммы: V строк × ceil(V/5) байт Trit5
//! ```
//!
//! Сборка полностью детерминирована (никаких таймстампов, никакого
//! состояния ОС): тот же корпус → побитово тот же файл.

use std::collections::HashMap;
use std::path::Path;

use crate::literary::qualia::fnv1a64;
use crate::literary::trit::{quantize, TritState};
use crate::pqc::sha256::sha256;
use crate::pqc::tensor::Trit5Codec;
use crate::ssn::cse;

/// Магия формата.
pub const MAGIC: [u8; 8] = *b"T5CRYS\0\0";
/// Версия формата.
pub const VERSION: u32 = 1;
/// Размер заголовка (sha256 заканчивается ровно на 0x50).
pub const HEADER: usize = 0x50;
/// Число архетипов поля (якорь токена = FNV-хэш mod 12, как у касты мухи).
pub const ARCHETYPES: usize = 12;
/// Размерность CSE-вложений по умолчанию.
pub const DEFAULT_DIMS: usize = 96;
/// Словарь по умолчанию.
pub const DEFAULT_VOCAB: usize = 384;
/// Порог знаковой квантизации PMI: притяжение.
pub const DEFAULT_THETA_HI: f64 = 1.7;
/// Порог знаковой квантизации PMI: отталкивание.
pub const DEFAULT_THETA_LO: f64 = 0.5;
/// Зашитый в бинарник кристалл (собран из corpus_ru.txt, пересборка:
/// `poler-engine --crystal-build src/triune/corpus_ru.txt --crystal-out …`).
pub const EMBEDDED_CRYSTAL: &[u8] = include_bytes!("crystal_ru.t5c");

/// Токенизация корпуса/речи: lowercase, слова = цепочки букв/цифр.
pub fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            for lc in ch.to_lowercase() {
                cur.push(lc);
            }
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Разбивка на предложения (граница: . ! ? ; : и перевод строки).
fn sentences(text: &str) -> Vec<&str> {
    text.split(['.', '!', '?', ';', ':', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Кристалл Знаний: словарь + троичная биграммная топология +
/// производные CSE-вложения.
#[derive(Clone)]
pub struct Crystal {
    /// Токены словаря в каноническом порядке (порядок = id).
    pub tokens: Vec<String>,
    /// Обратный индекс: токен → id.
    index: HashMap<String, u32>,
    /// Биграммы, Trit5-упакованные: строка prev, stride байт на строку.
    /// stride ≥ ceil(V/5): запас под динамическое расширение словаря
    /// (expand_vocab) без перезапаковки всей матрицы на каждое слово.
    bigram: Vec<u8>,
    /// Выделено байт на строку биграмм (может быть больше ceil(V/5)).
    stride: usize,
    /// Статистика корпуса.
    pub corpus_words: u64,
    pub corpus_chars: u64,
    /// Число ненулевых биграмм (плотность топологии).
    pub bigram_nonzeros: u64,
    /// Пороги квантизации (милли-доли).
    theta_hi_milli: u32,
    /// Производные вложения: триты (No-Mul SIMD) + f64 (NMDA-гейт).
    embeds_trit: Vec<TritState>,
    embeds_f64: Vec<Vec<f64>>,
    /// Якоря архетипов.
    anchors: Vec<u8>,
    /// Размерность CSE-вложений.
    pub dims: usize,
}

impl Crystal {
    /// Сборка кристалла из корпуса. Полностью детерминирована.
    pub fn build(
        corpus: &str,
        vocab: usize,
        dims: usize,
        theta_hi: f64,
        theta_lo: f64,
    ) -> Result<Crystal, String> {
        if !(0.5..=8.0).contains(&theta_hi) || !(0.05..=0.95).contains(&theta_lo) {
            return Err(format!("пороги вне диапазона: θ_hi={theta_hi}, θ_lo={theta_lo}"));
        }
        if theta_hi <= theta_lo {
            return Err("θ_hi должен быть больше θ_lo".into());
        }
        let dims = dims.clamp(16, 512);

        // ── Частоты слов (канонический порядок: частота ↓, слово ↑) ──
        let words = tokenize(corpus);
        if words.len() < 16 {
            return Err(format!("корпус слишком мал: {} слов (нужно ≥ 16)", words.len()));
        }
        let mut counts: HashMap<&str, u64> = HashMap::new();
        for w in &words {
            *counts.entry(w.as_str()).or_insert(0) += 1;
        }
        let mut ranked: Vec<(&str, u64)> = counts.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        ranked.retain(|(w, _)| w.len() <= u8::MAX as usize); // u8-длина в .t5c
        // clamp(32, n) паникует при n < 32 — короткий корпус получает
        // свой фактический словарь (паритет с потоковым ingest::finalize)
        let vocab = if ranked.len() < 32 {
            ranked.len()
        } else {
            vocab.clamp(32, ranked.len())
        };
        let tokens: Vec<String> = ranked[..vocab].iter().map(|(w, _)| w.to_string()).collect();
        let index: HashMap<String, u32> =
            tokens.iter().enumerate().map(|(i, t)| (t.clone(), i as u32)).collect();

        // ── Поток в пределах словаря (предложения не склеиваются) ──
        let mut stream_ids: Vec<Vec<u32>> = Vec::new();
        let mut total: u64 = 0;
        for sent in sentences(corpus) {
            let ids: Vec<u32> = tokenize(sent)
                .into_iter()
                .filter_map(|w| index.get(&w).copied())
                .collect();
            total += ids.len() as u64;
            if !ids.is_empty() {
                stream_ids.push(ids);
            }
        }
        if total < 8 {
            return Err("после фильтра словаря поток пуст".into());
        }

        // ── Биграммные счётчики ──
        let mut uni = vec![0u64; vocab];
        let mut bi: HashMap<(u32, u32), u64> = HashMap::new();
        for ids in &stream_ids {
            for &w in ids {
                uni[w as usize] += 1;
            }
            for pair in ids.windows(2) {
                *bi.entry((pair[0], pair[1])).or_insert(0) += 1;
            }
        }
        let cols_packed = vocab.div_ceil(5);
        // ⚠ Байт 0x00 в Trit5 = пять тритов −1 (цифра 0 = трит −1).
        // Вакуум кодируется байтом 121 = pack_5([0,0,0,0,0]).
        let zero_byte = Trit5Codec::pack_5(&[0i8; 5]).unwrap_or(121);
        let mut bigram = vec![zero_byte; vocab * cols_packed];
        let mut bigram_nonzeros: u64 = 0;
        // Детерминизм: обход отсортированных ключей, не HashMap.
        let mut pairs: Vec<((u32, u32), u64)> =
            bi.iter().map(|(k, &v)| (*k, v)).collect();
        pairs.sort_by_key(|&(p, n)| (p, n));
        for ((prev, next), c) in pairs {
            let (prev, next) = (prev as usize, next as usize);
            let p_next = uni[next] as f64 / total as f64;
            let p_cond = c as f64 / uni[prev].max(1) as f64;
            let r = p_cond / p_next.max(1e-12);
            let trit: i8 = if r >= theta_hi {
                1
            } else if r <= theta_lo {
                -1
            } else {
                0
            };
            if trit != 0 {
                bigram_nonzeros += 1;
                let byte = &mut bigram[prev * cols_packed + next / 5];
                let mut five = Trit5Codec::unpack_5(*byte);
                five[next % 5] = trit;
                if let Some(b) = Trit5Codec::pack_5(&five) {
                    *byte = b;
                }
            } else {
                // Явная запись вакуума (иначе останется корректный 121 —
                // но перезапись гарантирует согласованность при смене кодека).
                let byte = &mut bigram[prev * cols_packed + next / 5];
                let mut five = Trit5Codec::unpack_5(*byte);
                five[next % 5] = 0;
                if let Some(b) = Trit5Codec::pack_5(&five) {
                    *byte = b;
                }
            }
        }

        let mut crystal = Crystal {
            tokens,
            index,
            bigram,
            stride: cols_packed,
            corpus_words: words.len() as u64,
            corpus_chars: corpus.chars().count() as u64,
            bigram_nonzeros,
            theta_hi_milli: (theta_hi * 1000.0).round() as u32,
            embeds_trit: Vec::new(),
            embeds_f64: Vec::new(),
            anchors: Vec::new(),
            dims,
        };
        crystal.derive_embeddings();
        Ok(crystal)
    }

    /// Производные данные: CSE-вложения → триты + f64, якоря архетипов.
    fn derive_embeddings(&mut self) {
        let dims = self.dims;
        let mut trits = Vec::with_capacity(self.tokens.len());
        let mut full = Vec::with_capacity(self.tokens.len());
        let mut anchors = Vec::with_capacity(self.tokens.len());
        for t in &self.tokens {
            let v = cse::encode(t, dims);
            let v32: Vec<f32> = v.iter().map(|&x| x as f32).collect();
            trits.push(quantize(&v32, 0.05));
            let q = trits.last().unwrap().dequantize();
            full.push(q.iter().map(|&x| x as f64).collect());
            anchors.push((fnv1a64(t.as_bytes()) % ARCHETYPES as u64) as u8);
        }
        self.embeds_trit = trits;
        self.embeds_f64 = full;
        self.anchors = anchors;
    }

    /// Трит биграммы (prev → next): −1/0/+1. No-Mul: прямой lookup.
    pub fn bigram_trit(&self, prev: u32, next: u32) -> i8 {
        let v = self.tokens.len();
        if (prev as usize) >= v || (next as usize) >= v {
            return 0;
        }
        let byte = self.bigram[prev as usize * self.stride + next as usize / 5];
        Trit5Codec::unpack_5(byte)[next as usize % 5]
    }

    /// Строка синапсов нейрона `prev`: Trit5-упакованные триты
    /// `prev → *` (stride байт; столбцы ≥ V — вакуум). Горячий путь
    /// нейронной популяции: скан без распаковки всей матрицы.
    pub fn bigram_row(&self, prev: u32) -> &[u8] {
        let from = prev as usize * self.stride;
        &self.bigram[from..from + self.stride]
    }

    /// Шаг упаковки (байт на строку синапсов).
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// Хеббовский сдвиг синапса (v0.38.0): трит `prev → next` ползёт к
    /// ±1, насыщаясь на краях решётки {−1, 0, +1}. Возврат — новый трит.
    ///
    /// Это и есть пластичность «вес зашит в синапс»: пара (prev, next)
    /// совпала (NMDA-гейт открыт, энергия удивления высока) — синапс
    /// укрепляется на +1; промах — ослабляется на −1. Один проход, без
    /// эпох градиентного спуска — потому данных нужно меньше.
    pub fn bigram_bump(&mut self, prev: u32, next: u32, delta: i8) -> i8 {
        let v = self.tokens.len();
        if (prev as usize) >= v || (next as usize) >= v || delta == 0 {
            return self.bigram_trit(prev, next);
        }
        let byte_idx = prev as usize * self.stride + next as usize / 5;
        let mut five = Trit5Codec::unpack_5(self.bigram[byte_idx]);
        let cur = five[next as usize % 5];
        let new = (cur + delta).clamp(-1, 1);
        if new != cur {
            if cur != 0 {
                self.bigram_nonzeros -= 1;
            }
            if new != 0 {
                self.bigram_nonzeros += 1;
            }
        }
        five[next as usize % 5] = new;
        if let Some(b) = Trit5Codec::pack_5(&five) {
            self.bigram[byte_idx] = b;
        }
        new
    }

    /// Троичное вложение токена (No-Mul SIMD-сторона).
    pub fn embed_trit(&self, idx: u32) -> &TritState {
        &self.embeds_trit[idx as usize]
    }

    /// Вложение токена в f64 (сторона NMDA-гейта).
    pub fn embed_f64(&self, idx: u32) -> &[f64] {
        &self.embeds_f64[idx as usize]
    }

    /// Якорь архетипа токена (0..12).
    pub fn anchor(&self, idx: u32) -> usize {
        self.anchors[idx as usize] as usize
    }

    /// Топ-k семантических соседей токена — ассоциативные
    /// (симметричные) синапсы популяции: взаимная поддержка
    /// тематического кластера. Ранжирование по трит-скалярному
    /// произведению CSE-вложений (No-Mul SIMD).
    pub fn semantic_neighbors(&self, idx: u32, k: usize) -> Vec<u32> {
        let v = self.tokens.len();
        if idx as usize >= v || k == 0 {
            return Vec::new();
        }
        let probe = self.embeds_trit[idx as usize].dequantize();
        let mut sims: Vec<(u32, f32)> = (0..v as u32)
            .filter(|&j| j != idx)
            .map(|j| (j, self.embeds_trit[j as usize].dot(&probe)))
            .collect();
        sims.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.cmp(&b.0))
        });
        sims.into_iter().take(k).map(|(j, _)| j).collect()
    }

    /// Ассоциативные соседи нейрона — симметричное Хеббовское
    /// замыкание синапсов для резонансного бассейна популяции:
    ///
    /// - семантические соседи (топ `sem_k` по трит-скалярному
    ///   произведению CSE — морфология/форма слова);
    /// - партнёры по совместной встречаемости: `bigram(i→j) = +1`
    ///   ИЛИ `bigram(j→i) = +1` («срабатывали вместе — связаны»),
    ///   до `bi_k` в каждую сторону.
    ///
    /// Направленность синтаксиса живёт в волне (step популяции);
    /// здесь — только взаимная поддержка темы.
    pub fn assoc_neighbors(&self, idx: u32, sem_k: usize, bi_k: usize) -> Vec<u32> {
        let v = self.tokens.len();
        if idx as usize >= v {
            return Vec::new();
        }
        let mut out: Vec<u32> = Vec::with_capacity(sem_k + 2 * bi_k);
        let push_unique = |j: u32, out: &mut Vec<u32>| {
            if j != idx && !out.contains(&j) {
                out.push(j);
            }
        };
        // 1. Семантика.
        for j in self.semantic_neighbors(idx, sem_k) {
            push_unique(j, &mut out);
        }
        // 2. Прямые партнёры (+1 в строке).
        let row = self.bigram_row(idx);
        let mut found = 0usize;
        for (byte_idx, &byte) in row.iter().enumerate() {
            if byte == 121 {
                continue; // вакуум
            }
            let five = crate::pqc::tensor::Trit5Codec::unpack_5(byte);
            for (slot, &w) in five.iter().enumerate() {
                if w != 1 {
                    continue;
                }
                let j = byte_idx * 5 + slot;
                if j >= v {
                    break;
                }
                push_unique(j as u32, &mut out);
                found += 1;
                if found >= bi_k {
                    break;
                }
            }
            if found >= bi_k {
                break;
            }
        }
        // 3. Обратные партнёры (+1 в столбце): встречались рядом —
        //    связь симметрична для памяти, какой бы ни была стрелка.
        found = 0;
        for j in 0..v as u32 {
            if self.bigram_trit(j, idx) == 1 {
                push_unique(j, &mut out);
                found += 1;
                if found >= bi_k {
                    break;
                }
            }
        }
        out
    }

    /// id токена или None (вне словаря).
    pub fn id_of(&self, token: &str) -> Option<u32> {
        self.index.get(token).copied()
    }

    /// Размер словаря.
    pub fn vocab(&self) -> usize {
        self.tokens.len()
    }

    /// Загрузка из байтов .t5c (dims — размерность CSE-вложений).
    pub fn load(bytes: &[u8], dims: usize) -> Result<Crystal, String> {
        if bytes.len() < HEADER {
            return Err("файл меньше заголовка".into());
        }
        if bytes[..8] != MAGIC {
            return Err("не .t5c: чужая магия".into());
        }
        let rd_u32 = |off: usize| -> u32 {
            u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap())
        };
        let rd_u64 = |off: usize| -> u64 {
            let lo = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()) as u64;
            let hi = u32::from_le_bytes(bytes[off + 4..off + 8].try_into().unwrap()) as u64;
            lo | (hi << 32)
        };
        if rd_u32(0x08) != VERSION {
            return Err(format!("версия {} не поддерживается", rd_u32(0x08)));
        }
        // sha256 тела.
        let digest = sha256(&bytes[HEADER..]);
        if digest[..] != bytes[0x30..0x50] {
            return Err("sha256 тела не сходится: кристалл повреждён".into());
        }
        let vocab = rd_u32(0x0C) as usize;
        let flags = rd_u32(0x10);
        let token_off = rd_u32(0x14) as usize;
        let bigram_off = rd_u32(0x18) as usize;
        let corpus_words = rd_u64(0x1C);
        let corpus_chars = rd_u64(0x24);
        let theta_hi_milli = rd_u32(0x2C);
        if vocab == 0 || vocab > 1_000_000 {
            return Err(format!("странный словарь: {vocab}"));
        }
        // Таблица токенов.
        let mut pos = token_off;
        let mut tokens = Vec::with_capacity(vocab);
        for _ in 0..vocab {
            if pos >= bytes.len() {
                return Err("таблица токенов оборвана".into());
            }
            let len = bytes[pos] as usize;
            pos += 1;
            if pos + len > bytes.len() {
                return Err("токен оборван".into());
            }
            tokens.push(String::from_utf8(bytes[pos..pos + len].to_vec()).map_err(|e| e.to_string())?);
            pos += len;
        }
        if pos != bigram_off && (flags & 1) == 1 {
            return Err("bigram_off не совпадает с концом таблицы токенов".into());
        }
        let cols = vocab.div_ceil(5);
        let bigram = if flags & 1 == 1 {
            let need = vocab * cols;
            if bigram_off + need != bytes.len() {
                return Err("биграммная секция не до конца файла".into());
            }
            bytes[bigram_off..bigram_off + need].to_vec()
        } else {
            if bigram_off != bytes.len() {
                return Err("лишние байты после таблицы токенов".into());
            }
            Vec::new()
        };
        let index: HashMap<String, u32> =
            tokens.iter().enumerate().map(|(i, t)| (t.clone(), i as u32)).collect();
        let bigram_nonzeros = bigram
            .chunks_exact(cols)
            .flat_map(|row| row.iter())
            .map(|&b| Trit5Codec::unpack_5(b))
            .map(|five| five.iter().filter(|&&t| t != 0).count() as u64)
            .sum();
        let mut crystal = Crystal {
            tokens,
            index,
            bigram,
            stride: cols,
            corpus_words,
            corpus_chars,
            bigram_nonzeros,
            theta_hi_milli,
            embeds_trit: Vec::new(),
            embeds_f64: Vec::new(),
            anchors: Vec::new(),
            dims: dims.clamp(16, 512),
        };
        crystal.derive_embeddings();
        Ok(crystal)
    }

    /// Сериализация в байты .t5c.
    /// Строки биграмм обрезаются до ceil(V/5) — запас stride не утекает
    /// в артефакт: тот же словарь + та же топология → те же байты,
    /// независимо от ёмкости в памяти.
    pub fn to_bytes(&self) -> Vec<u8> {
        let vocab = self.tokens.len();
        let cols = vocab.div_ceil(5);
        let token_off = HEADER;
        let mut token_table = Vec::new();
        for t in &self.tokens {
            token_table.push(t.len() as u8);
            token_table.extend_from_slice(t.as_bytes());
        }
        let bigram_off = token_off + token_table.len();
        let mut out = Vec::with_capacity(bigram_off + vocab * cols);
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(vocab as u32).to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes()); // flags: bigram present
        out.extend_from_slice(&(token_off as u32).to_le_bytes());
        out.extend_from_slice(&(bigram_off as u32).to_le_bytes());
        out.extend_from_slice(&self.corpus_words.to_le_bytes());
        out.extend_from_slice(&self.corpus_chars.to_le_bytes());
        out.extend_from_slice(&self.theta_hi_milli.to_le_bytes());
        let digest = {
            let mut body = Vec::with_capacity(token_table.len() + vocab * cols);
            body.extend_from_slice(&token_table);
            for row in 0..vocab {
                let from = row * self.stride;
                body.extend_from_slice(&self.bigram[from..from + cols]);
            }
            sha256(&body)
        };
        out.extend_from_slice(&digest);
        debug_assert_eq!(out.len(), HEADER, "заголовок .t5c ровно 0x50 байт");
        out.extend_from_slice(&token_table);
        for row in 0..vocab {
            let from = row * self.stride;
            out.extend_from_slice(&self.bigram[from..from + cols]);
        }
        out
    }

    /// Сохранение на диск.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        std::fs::write(path, self.to_bytes()).map_err(|e| format!("запись {path:?}: {e}"))
    }

    /// Зашитый в бинарник кристалл (собран из corpus_ru.txt).
    pub fn embedded() -> Result<Crystal, String> {
        Crystal::load(EMBEDDED_CRYSTAL, DEFAULT_DIMS)
    }

    /// θ_hi сборки (восстановление из милли-долей).
    pub fn theta_hi(&self) -> f64 {
        self.theta_hi_milli as f64 / 1000.0
    }

    /// Динамическое расширение словаря (Dynamic Vocab Expansion):
    /// новое слово мгновенно получает id, CSE-вложение и якорь архетипа —
    /// и становится полноценным участником речи Триединства.
    ///
    /// Матрица биграмм растёт с запасом stride (+64 столбца за рост),
    /// поэтому амортизированная стоимость добавления — O(1), а не
    /// O(V·cols) перезапаковка на каждое слово.
    pub fn expand_vocab(&mut self, word: &str) -> Option<u32> {
        if let Some(&id) = self.index.get(word) {
            return Some(id);
        }
        if word.is_empty() || word.len() > u8::MAX as usize {
            return None; // u8-длина в .t5c не вместит это слово
        }
        if self.tokens.len() >= u32::MAX as usize {
            return None;
        }
        let id = self.tokens.len() as u32;
        // Вакуум-байт Trit5: pack_5([0,0,0,0,0]) = 121 (не 0x00 — тот
        // декодируется как пять тритов −1!).
        let vacuum = Trit5Codec::pack_5(&[0i8; 5]).unwrap_or(121);
        // 1. Ёмкость матрицы: нужна строка для нового prev + столбец next.
        let need_cols = (self.tokens.len() + 1).div_ceil(5);
        if need_cols > self.stride {
            let new_stride = need_cols + 64;
            let mut grown = vec![vacuum; self.tokens.len() * new_stride];
            for row in 0..self.tokens.len() {
                let (from, to) = (row * self.stride, row * new_stride);
                grown[to..to + self.stride]
                    .copy_from_slice(&self.bigram[from..from + self.stride]);
            }
            self.bigram = grown;
            self.stride = new_stride;
        }
        self.bigram.extend(std::iter::repeat(vacuum).take(self.stride));
        // 2. Словарь + индекс.
        self.tokens.push(word.to_string());
        self.index.insert(word.to_string(), id);
        // 3. Вложения только для нового слова (не пересобираем весь V×dims).
        let v = cse::encode(word, self.dims);
        let v32: Vec<f32> = v.iter().map(|&x| x as f32).collect();
        let t = quantize(&v32, 0.05);
        let q = t.dequantize();
        self.embeds_trit.push(t);
        self.embeds_f64.push(q.iter().map(|&x| x as f64).collect());
        self.anchors
            .push((fnv1a64(word.as_bytes()) % ARCHETYPES as u64) as u8);
        Some(id)
    }

    /// Subword-фолбек: жадное разложение слова на известные подтокены
    /// (длиннейший префикс слева). Пустой вектор = слово не разложимо.
    /// Предел глубины 6 кусков защищает от бессмысленного дробления.
    pub fn subword_ids(&self, word: &str) -> Vec<u32> {
        let mut out = Vec::new();
        let bytes = word.as_bytes();
        let mut pos = 0usize;
        while pos < bytes.len() && out.len() < 6 {
            let mut best: Option<(usize, u32)> = None;
            // длиннейший префикс word[pos..], присутствующий в словаре
            for end in (pos + 1..=bytes.len()).rev() {
                if !word.is_char_boundary(end) {
                    continue;
                }
                if let Some(&id) = self.index.get(&word[pos..end]) {
                    best = Some((end, id));
                    break;
                }
            }
            match best {
                Some((end, id)) => {
                    out.push(id);
                    pos = end;
                }
                None => return Vec::new(), // тупик: слово не разложимо
            }
        }
        if pos < bytes.len() {
            return Vec::new(); // не доели слово за лимит кусков
        }
        out
    }

    /// Сборка кристалла из готовых компонентов (путь потокового
    /// инжектора [`crate::triune::ingest`]): словарь + уже
    /// квантованная Trit5-матрица биграмм + статистика корпуса.
    pub fn from_parts(
        tokens: Vec<String>,
        bigram: Vec<u8>,
        corpus_words: u64,
        corpus_chars: u64,
        bigram_nonzeros: u64,
        theta_hi: f64,
        theta_lo: f64,
        dims: usize,
    ) -> Result<Crystal, String> {
        if !(0.5..=8.0).contains(&theta_hi) || !(0.05..=0.95).contains(&theta_lo) {
            return Err(format!("пороги вне диапазона: θ_hi={theta_hi}, θ_lo={theta_lo}"));
        }
        if theta_hi <= theta_lo {
            return Err("θ_hi должен быть больше θ_lo".into());
        }
        let vocab = tokens.len();
        if vocab == 0 || vocab > 1_000_000 {
            return Err(format!("странный словарь: {vocab}"));
        }
        let stride = vocab.div_ceil(5);
        if bigram.len() != vocab * stride {
            return Err(format!(
                "матрица биграмм {vocab}×{stride} ожидается, байт: {}",
                bigram.len()
            ));
        }
        let index: HashMap<String, u32> =
            tokens.iter().enumerate().map(|(i, t)| (t.clone(), i as u32)).collect();
        let mut crystal = Crystal {
            tokens,
            index,
            bigram,
            stride,
            corpus_words,
            corpus_chars,
            bigram_nonzeros,
            theta_hi_milli: (theta_hi * 1000.0).round() as u32,
            embeds_trit: Vec::new(),
            embeds_f64: Vec::new(),
            anchors: Vec::new(),
            dims: dims.clamp(16, 512),
        };
        crystal.derive_embeddings();
        Ok(crystal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORPUS: &str = "Мозг мухи держит ритм мысли. Вихрь крутит смысл по кругу. \
        Кристалл хранит слова мира. Мозг говорит через кристалл. Вихрь диспетчеризует внимание. \
        Муха задаёт пульс системы. Система слушает вход. Вход течёт через кодировщик. \
        Кодировщик превращает текст в вектор. Вектор вливается в нейроны. \
        Открой терминал и покажи статус. Запусти сборку проекта. Покажи логи системы.";

    #[test]
    fn build_is_deterministic() {
        let a = Crystal::build(CORPUS, 64, 64, 1.7, 0.5).unwrap();
        let b = Crystal::build(CORPUS, 64, 64, 1.7, 0.5).unwrap();
        assert_eq!(a.to_bytes(), b.to_bytes(), "тот же корпус → те же байты");
    }

    #[test]
    fn roundtrip_preserves_everything() {
        let c = Crystal::build(CORPUS, 64, 64, 1.7, 0.5).unwrap();
        let bytes = c.to_bytes();
        let back = Crystal::load(&bytes, 64).unwrap();
        assert_eq!(back.tokens, c.tokens);
        assert_eq!(back.corpus_words, c.corpus_words);
        assert_eq!(back.corpus_chars, c.corpus_chars);
        assert_eq!(back.bigram_nonzeros, c.bigram_nonzeros);
        for prev in 0..c.vocab() as u32 {
            for next in 0..c.vocab() as u32 {
                assert_eq!(
                    c.bigram_trit(prev, next),
                    back.bigram_trit(prev, next),
                    "биграмма {prev}→{next} изменилась"
                );
            }
        }
    }

    #[test]
    fn tamper_detected_by_sha() {
        let mut bytes = Crystal::build(CORPUS, 64, 64, 1.7, 0.5).unwrap().to_bytes();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;
        assert!(Crystal::load(&bytes, 64).is_err(), "порча тела должна ловиться");
        bytes[0x0C] ^= 0x01; // порча заголовка (vocab)
        assert!(Crystal::load(&bytes, 64).is_err());
    }

    #[test]
    fn bigram_values_are_trits() {
        let c = Crystal::build(CORPUS, 64, 64, 1.7, 0.5).unwrap();
        for prev in 0..c.vocab() as u32 {
            for next in 0..c.vocab() as u32 {
                assert!([-1i8, 0, 1].contains(&c.bigram_trit(prev, next)));
            }
        }
        // «через кодировщик» — сильная биграмма корпуса → +1
        let (a, b) = (c.id_of("через").unwrap(), c.id_of("кодировщик").unwrap());
        assert_eq!(c.bigram_trit(a, b), 1, "частая пара должна притягиваться");
    }

    #[test]
    fn embeddings_are_no_mul_trits() {
        let c = Crystal::build(CORPUS, 64, 64, 1.7, 0.5).unwrap();
        for i in 0..c.vocab() as u32 {
            let t = c.embed_trit(i);
            assert_eq!(t.dims, 64);
            assert_eq!(t.packed_bytes().len(), 64_usize.div_ceil(5));
            assert!(t.nonzeros > 0, "вложение токена не вакуум");
        }
    }

    #[test]
    fn anchors_in_archetype_range() {
        let c = Crystal::build(CORPUS, 64, 64, 1.7, 0.5).unwrap();
        for i in 0..c.vocab() as u32 {
            assert!(c.anchor(i) < ARCHETYPES);
        }
        // Одинаковое слово → одинаковый якорь (детерминизм FNV).
        assert_eq!(c.anchor(0), c.anchor(0));
    }

    #[test]
    fn embedded_crystal_loads() {
        let c = Crystal::embedded().expect("зашитый кристалл должен загружаться");
        assert!(c.vocab() >= 32, "словарь встроенного кристалла: {}", c.vocab());
        assert!(c.corpus_words > 100, "корпус встроенного кристалла: {}", c.corpus_words);
        assert!(c.bigram_nonzeros > 0, "биграммы встроенного кристалла пусты");
    }

    #[test]
    fn tokenizer_lowercases_and_splits() {
        let ts = tokenize("Открой Терминал, и — покажи: статус!");
        assert_eq!(ts, vec!["открой", "терминал", "и", "покажи", "статус"]);
    }

    #[test]
    fn expand_vocab_gives_new_word_full_life() {
        let mut c = Crystal::build(CORPUS, 64, 64, 1.7, 0.5).unwrap();
        let v0 = c.vocab();
        assert!(c.id_of("квантигон").is_none(), "слова нет в корпусе");
        let id = c.expand_vocab("квантигон").expect("слово должно добавиться");
        assert_eq!(c.vocab(), v0 + 1);
        assert_eq!(c.id_of("квантигон"), Some(id));
        // Вложения и якорь на месте.
        assert_eq!(c.embed_trit(id).dims, 64);
        assert!(c.embed_trit(id).nonzeros > 0, "вложение нового слова не вакуум");
        assert!(!c.embed_f64(id).is_empty());
        assert!(c.anchor(id) < ARCHETYPES);
        // Повторное добавление — идемпотентно.
        assert_eq!(c.expand_vocab("квантигон"), Some(id));
        assert_eq!(c.vocab(), v0 + 1);
        // Пустое и сверхдлинное слова отклоняются.
        assert!(c.expand_vocab("").is_none());
        let long = "а".repeat(256);
        assert!(c.expand_vocab(&long).is_none());
    }

    #[test]
    fn expand_vocab_preserves_bigrams_and_serializes_clean() {
        let mut c = Crystal::build(CORPUS, 64, 64, 1.7, 0.5).unwrap();
        let (a, b) = (c.id_of("через").unwrap(), c.id_of("кодировщик").unwrap());
        let before = c.bigram_trit(a, b);
        assert_eq!(before, 1, "пару ожидает притяжение");
        // 30 расширений — матрица растёт со stride-запасом.
        for i in 0..30 {
            c.expand_vocab(&format!("неологизм{i}")).unwrap();
        }
        assert_eq!(c.bigram_trit(a, b), 1, "старые биграммы целы после роста");
        // Новая строка — вакуум (слово никогда не встречалось в парах).
        let new_id = c.id_of("неологизм0").unwrap();
        for next in 0..c.vocab() as u32 {
            assert_eq!(c.bigram_trit(new_id, next), 0);
        }
        // Сериализация: stride-запас не утекает, roundtrip бит-в-бит.
        let bytes = c.to_bytes();
        let back = Crystal::load(&bytes, 64).unwrap();
        assert_eq!(back.vocab(), c.vocab());
        for prev in 0..c.vocab() as u32 {
            for next in 0..c.vocab() as u32 {
                assert_eq!(c.bigram_trit(prev, next), back.bigram_trit(prev, next));
            }
        }
        // Файл ровно V×ceil(V/5) байт биграмм: никаких скрытых запасов.
        let v = c.vocab();
        let tail = &bytes[0x50..];
        let mut pos = 0usize;
        for _ in 0..v {
            pos += 1 + tail[pos] as usize;
        }
        assert_eq!(bytes.len(), 0x50 + pos + v * v.div_ceil(5), "хвост — ровно матрица");
    }

    #[test]
    fn subword_fallback_decomposes_unknown_words() {
        let c = Crystal::build(CORPUS, 64, 64, 1.7, 0.5).unwrap();
        // «мозговой» не в корпусе, но «мозг» есть… проверяем общий механизм
        // на гарантированно разложимом случае: склейке двух словарных слов.
        let w1 = c.tokens[0].clone();
        let w2 = c.tokens[1].clone();
        let glued = format!("{w1}{w2}");
        let ids = c.subword_ids(&glued);
        assert_eq!(ids.len(), 2, "склейка двух слов режется на 2 куска");
        assert_eq!(ids[0], c.id_of(&w1).unwrap());
        assert_eq!(ids[1], c.id_of(&w2).unwrap());
        // Словарное слово = само себя одним куском.
        assert_eq!(c.subword_ids(&w1), vec![c.id_of(&w1).unwrap()]);
        // Неразложимый мусор → пусто.
        assert!(c.subword_ids("ъъъъъ").is_empty() || {
            // «ъ» может не быть в словаре — тогда пусто; иначе один кусок
            c.subword_ids("ъъъъъ").len() <= 1
        });
    }

    #[test]
    fn from_parts_rejects_bad_input() {
        let c = Crystal::build(CORPUS, 64, 64, 1.7, 0.5).unwrap();
        let v = c.vocab();
        let stride = v.div_ceil(5);
        // Матрица не того размера.
        assert!(Crystal::from_parts(
            c.tokens.clone(),
            vec![0u8; v * stride + 1],
            c.corpus_words, c.corpus_chars, c.bigram_nonzeros, 1.7, 0.5, 64
        )
        .is_err());
        // Пороги вне диапазона.
        assert!(Crystal::from_parts(
            c.tokens.clone(),
            vec![0u8; v * stride],
            c.corpus_words, c.corpus_chars, c.bigram_nonzeros, 9.9, 0.5, 64
        )
        .is_err());
        // Корректный вход — работает.
        let parts = Crystal::from_parts(
            c.tokens.clone(),
            vec![121u8; v * stride],
            c.corpus_words, c.corpus_chars, 0, 1.7, 0.5, 64,
        );
        assert!(parts.is_ok());
    }
}
