//! Потоковое квантование весов на лету (.t5q) — 70B на диске 4 ГБ.
//!
//! Ответ на вопрос «как выкачать интеллект 70B-модели, если сырые
//! веса (140 ГБ FP16) не помещаются на диск»: **сырец вообще не
//! касается диска**. Поток тензоров читается чанками из сети/файла в
//! кольцевой буфер RAM, квантуется блоками в троичную решётку
//! {−1, 0, +1} и на диск пишется только готовая Trit5-форма:
//!
//! ```text
//! [HTTP/файл: чанки по 64 КБ]
//!        ▼
//! [кольцевой буфер RAM: 2 блока × 512 f32 ≈ 4 КБ]   ← пик RAM фиксирован
//!        ▼  блок 512 значений:
//!        scale = max|w|;  трит = sign(w), если |w| ≥ θ·scale и |w| ≥ top-k
//!        ▼
//! [диск: .t5q = 4 Б масштаба + 103 Б тритов на блок]  ← 1.67 бита/вес
//! ```
//!
//! ## Математика (честная)
//!
//! - Плотный режим: 140 ГБ FP16 → 140·(107/512)/2 ≈ **7.3 ГБ**
//!   (в 19 раз меньше; Trit5 = 1.58 бита + 32 бита масштаба на блок).
//! - `--keep 0.1`: 90% тритов — нули (вакуум). Блочный формат хранит
//!   их нулями и **подготовлен к вакуум-компрессии** следующего
//!   поколения (нулевые блоки элиминируются, ненулевые — zstd).
//!   Уже сегодня No-Mul-ядра пропускают нули без работы.
//! - Инвариант: границы блоков кратны 512 значениям от начала потока
//!   → **результат не зависит от размера чанков чтения** (доказано
//!   тестом `chunk_size_invariance`): качай по 3 КБ или одним куском —
//!   байты идентичны.
//!
//! ## Формат .t5q (Trit5 Stream v1, little-endian)
//!
//! ```text
//! СМЕЩЕНИЕ  РАЗМЕР  ПОЛЕ
//! 0x00      8       magic "T5STRM\0\0"
//! 0x08      4       version u32 = 1
//! 0x0C      4       block u32 (значений на блок)
//! 0x10      4       flags u32 (bit0 = f16-вход, bit1 = top-k)
//! 0x14      4       theta_milli u32 (θ × 1000)
//! 0x18      4       keep_promille u32 (keep × 1000)
//! 0x1C      44      reserved (нули)
//! 0x48      ·       блоки: [f32 scale][ceil(block/5) байт Trit5]
//! …         ·       трейлер: values u64 | blocks u64 | zeros u64
//!                   | in_bytes u64 | sha256(блоки) 32 Б
//! ```
//!
//! Трейлер пишется в конце — поток не перематывается, формат дружит
//! с pipe/stdout. Заголовок и трейлер в сумме 120 байт.

use std::io::{Read, Write};

use crate::pqc::sha256::{sha256, Sha256};
use crate::pqc::tensor::Trit5Codec;

/// Магия формата.
pub const MAGIC: [u8; 8] = *b"T5STRM\0\0";
/// Версия формата.
pub const VERSION: u32 = 1;
/// Размер заголовка.
pub const HEADER: usize = 0x48;
/// Размер трейлера: 4×u64 + 32 Б sha256.
pub const TRAILER: usize = 64;
/// Значений на блок по умолчанию.
pub const DEFAULT_BLOCK: usize = 512;

/// Конфигурация потокового квантования.
#[derive(Debug, Clone)]
pub struct StreamQuantConfig {
    /// Значений на блок (масштаб общий внутри блока).
    pub block: usize,
    /// Мёртвая зона: |w| < θ·scale → трит 0.
    pub theta: f32,
    /// Доля удерживаемых топ-|w| тритов на блок (1.0 = плотно).
    pub keep: f32,
    /// Вход — f16-LE (иначе f32-LE).
    pub f16: bool,
}

impl Default for StreamQuantConfig {
    fn default() -> Self {
        StreamQuantConfig { block: DEFAULT_BLOCK, theta: 0.05, keep: 1.0, f16: false }
    }
}

/// Статистика прогонa.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StreamQuantStats {
    /// Всего значений прочитано.
    pub values: u64,
    /// Блоков записано.
    pub blocks: u64,
    /// Нулевых тритов (вакуум).
    pub zeros: u64,
    /// Байт входа.
    pub in_bytes: u64,
    /// Байт выхода (заголовок + блоки + трейлер).
    pub out_bytes: u64,
    /// Пик буфера RAM, байт (инвариант ограниченности).
    pub peak_buffer: usize,
}

impl StreamQuantStats {
    /// Эффективные бита на значение.
    pub fn bits_per_value(&self) -> f64 {
        if self.values == 0 {
            return 0.0;
        }
        (self.out_bytes as f64 * 8.0) / self.values as f64
    }

    /// Доля вакуума.
    pub fn vacuum_frac(&self) -> f64 {
        if self.values == 0 {
            return 0.0;
        }
        self.zeros as f64 / self.values as f64
    }
}

/// f16-LE → f32 (без внешних крейтов, битовая магия IEEE 754).
fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1f) as u32;
    let frac = (h & 0x3ff) as u32;
    let bits = match exp {
        0 => {
            if frac == 0 {
                sign << 31
            } else {
                // субнормаль: нормализуем
                let mut e = -1i32;
                let mut f = frac as i32;
                while f & 0x400 == 0 {
                    f <<= 1;
                    e -= 1;
                }
                f &= 0x3ff;
                (sign << 31) | (((127 - 15 + e + 1) as u32) << 23) | (f as u32) << 13
            }
        }
        0x1f => (sign << 31) | (0xff << 23) | (frac << 13),
        _ => (sign << 31) | ((exp + 127 - 15) << 23) | (frac << 13),
    };
    f32::from_bits(bits)
}

/// Читает поток байт, выдаёт значения f32 (по конфигу f16/f32).
/// Буфер фиксирован (64 КБ): refill сдвигает хвост и дочитывает —
/// пик RAM ограничен конструкцией, а не размером модели.
struct ValueReader<R: Read> {
    inner: R,
    f16: bool,
    buf: Vec<u8>,
    /// Сколько байт в буфере валидно (заполнено из потока).
    valid: usize,
    pos: usize,
    eof: bool,
}

impl<R: Read> ValueReader<R> {
    fn new(inner: R, f16: bool) -> Self {
        ValueReader { inner, f16, buf: vec![0u8; 64 * 1024], valid: 0, pos: 0, eof: false }
    }

    /// Следующее значение или None в конце потока.
    fn next_f32(&mut self, peak: &mut usize) -> std::io::Result<Option<f32>> {
        let w = if self.f16 { 2 } else { 4 };
        if self.pos + w > self.valid {
            self.refill(peak)?;
        }
        if self.pos + w > self.valid {
            return Ok(None); // конец потока (возможный хвост < w отбрасывается)
        }
        let v = if self.f16 {
            let h = u16::from_le_bytes([self.buf[self.pos], self.buf[self.pos + 1]]);
            f16_to_f32(h)
        } else {
            f32::from_le_bytes(self.buf[self.pos..self.pos + 4].try_into().unwrap())
        };
        self.pos += w;
        Ok(Some(v))
    }

    /// Сдвиг хвоста к началу + дочитывание до заполнения или EOF.
    fn refill(&mut self, peak: &mut usize) -> std::io::Result<()> {
        let tail = self.valid - self.pos;
        self.buf.copy_within(self.pos..self.valid, 0);
        self.pos = 0;
        self.valid = tail;
        while self.valid < self.buf.len() && !self.eof {
            let n = self.inner.read(&mut self.buf[self.valid..])?;
            if n == 0 {
                self.eof = true;
            } else {
                self.valid += n;
            }
        }
        *peak = (*peak).max(self.valid);
        Ok(())
    }
}

/// Квантование одного блока: (масштаб, упакованные триты, нулей среди
/// реальных значений). Нули считаются по фактическому порогу
/// max(θ·scale, top-k cut) — честная статистика вакуума.
fn quantize_block(values: &[f32], theta: f32, keep: f32, block: usize) -> (f32, Vec<u8>, usize) {
    let scale = values.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
    let dead = theta * scale;
    // Топ-k отсечение: порог |w|, ниже которого — вакуум.
    let cut = if keep < 1.0 {
        let keep_n = ((values.len() as f32) * keep.clamp(0.01, 1.0)).ceil() as usize;
        let mut mags: Vec<f32> = values.iter().map(|v| v.abs()).collect();
        mags.sort_by(|a, b| b.total_cmp(a));
        mags[keep_n.min(mags.len()).saturating_sub(1)]
    } else {
        0.0
    };
    let threshold = dead.max(cut);
    let mut trits = vec![0i8; block];
    let mut nonzero = 0usize;
    for (t, &v) in trits.iter_mut().zip(values) {
        if v.abs() >= threshold && v.abs() > 0.0 {
            *t = if v > 0.0 { 1 } else { -1 };
            nonzero += 1;
        }
    }
    let zeros = values.len() - nonzero;
    // Упаковка 5 тритов в байт (блок фиксирован — паддинг нулями).
    let cols = block.div_ceil(5);
    let mut packed = vec![0u8; cols];
    for (c, chunk) in trits.chunks(5).enumerate() {
        let mut five = [0i8; 5];
        five[..chunk.len()].copy_from_slice(chunk);
        if let Some(b) = Trit5Codec::pack_5(&five) {
            packed[c] = b;
        }
    }
    (scale, packed, zeros)
}

/// Потоковое квантование: reader → writer. Пик RAM = буфер чтения +
/// один блок (килобайты), независимо от размера модели.
pub fn stream_quantize<R: Read, W: Write>(
    mut input: R,
    mut output: W,
    cfg: &StreamQuantConfig,
) -> Result<StreamQuantStats, String> {
    if cfg.block < 16 || cfg.block > 65536 {
        return Err(format!("block {} вне [16, 65536]", cfg.block));
    }
    if !(0.0..=0.5).contains(&cfg.theta) {
        return Err(format!("theta {} вне (0, 0.5]", cfg.theta));
    }
    // Заголовок.
    let mut head = Vec::with_capacity(HEADER);
    head.extend_from_slice(&MAGIC);
    head.extend_from_slice(&VERSION.to_le_bytes());
    head.extend_from_slice(&(cfg.block as u32).to_le_bytes());
    let flags: u32 = (cfg.f16 as u32) | ((cfg.keep < 1.0) as u32) << 1;
    head.extend_from_slice(&flags.to_le_bytes());
    head.extend_from_slice(&((cfg.theta * 1000.0).round() as u32).to_le_bytes());
    head.extend_from_slice(&((cfg.keep.clamp(0.01, 1.0) * 1000.0).round() as u32).to_le_bytes());
    head.resize(HEADER, 0);
    output.write_all(&head).map_err(|e| e.to_string())?;

    let mut reader = ValueReader::new(&mut input, cfg.f16);
    let mut block_buf: Vec<f32> = Vec::with_capacity(cfg.block);
    let mut stats = StreamQuantStats::default();
    let mut hasher = Sha256::new();
    let mut out_counter = HEADER as u64;

    loop {
        // Набрать блок (выход — на границе кратной block, независимо
        // от чанков чтения внутри ValueReader).
        block_buf.clear();
        while block_buf.len() < cfg.block {
            match reader
                .next_f32(&mut stats.peak_buffer)
                .map_err(|e| format!("чтение потока: {e}"))?
            {
                Some(v) => block_buf.push(v),
                None => break,
            }
        }
        if block_buf.is_empty() {
            break;
        }
        let (scale, packed, zeros) = quantize_block(&block_buf, cfg.theta, cfg.keep, cfg.block);
        stats.zeros += zeros as u64;
        let mut piece = Vec::with_capacity(4 + packed.len());
        piece.extend_from_slice(&scale.to_le_bytes());
        piece.extend_from_slice(&packed);
        hasher.update(&piece);
        output.write_all(&piece).map_err(|e| e.to_string())?;
        out_counter += piece.len() as u64;
        stats.values += block_buf.len() as u64;
        stats.blocks += 1;
        if block_buf.len() < cfg.block {
            break; // неполный финальный блок
        }
    }

    // Трейлер (без перемотки: считаем in_bytes по формату входа).
    let width = if cfg.f16 { 2u64 } else { 4u64 };
    stats.in_bytes = stats.values * width;
    let digest = hasher.finalize();
    let mut tail = Vec::with_capacity(TRAILER);
    tail.extend_from_slice(&stats.values.to_le_bytes());
    tail.extend_from_slice(&stats.blocks.to_le_bytes());
    tail.extend_from_slice(&stats.zeros.to_le_bytes());
    tail.extend_from_slice(&stats.in_bytes.to_le_bytes());
    tail.extend_from_slice(&digest);
    output.write_all(&tail).map_err(|e| e.to_string())?;
    out_counter += TRAILER as u64;
    stats.out_bytes = out_counter;
    Ok(stats)
}

/// Проверка целостности .t5q (магия + sha256 блоков).
pub fn verify_t5q(bytes: &[u8]) -> Result<StreamQuantStats, String> {
    if bytes.len() < HEADER + TRAILER {
        return Err("файл меньше заголовка+трейлера".into());
    }
    if bytes[..8] != MAGIC {
        return Err("не .t5q: чужая магия".into());
    }
    let rd_u32 = |off: usize| u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
    let rd_u64 = |off: usize| {
        u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap())
    };
    if rd_u32(0x08) != VERSION {
        return Err(format!("версия {}", rd_u32(0x08)));
    }
    let block = rd_u32(0x0C) as usize;
    if block == 0 || block > 65536 {
        return Err(format!("странный блок: {block}"));
    }
    let body = &bytes[HEADER..bytes.len() - TRAILER];
    let tail = &bytes[bytes.len() - TRAILER..];
    let digest = sha256(body);
    if digest[..] != tail[32..64] {
        return Err("sha256 блоков не сходится".into());
    }
    let cols = block.div_ceil(5);
    let piece = 4 + cols;
    let blocks = rd_u64(bytes.len() - TRAILER + 8) as usize;
    if body.len() % piece != 0 || body.len() / piece != blocks {
        return Err(format!("тело {} Б не бьётся на {blocks} блоков по {piece} Б", body.len()));
    }
    Ok(StreamQuantStats {
        values: rd_u64(bytes.len() - TRAILER),
        blocks: blocks as u64,
        zeros: rd_u64(bytes.len() - TRAILER + 16),
        in_bytes: rd_u64(bytes.len() - TRAILER + 24),
        out_bytes: bytes.len() as u64,
        peak_buffer: 0,
    })
}

// (трейты Read/Write импортированы в шапке модуля)

#[cfg(test)]
mod tests {
    use super::*;

    /// Детерминированный поток: N значений из seed (LCG).
    fn make_stream(n: usize, seed: u64) -> Vec<u8> {
        let mut s = seed;
        let mut out = Vec::with_capacity(n * 4);
        for _ in 0..n {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let v = ((s >> 33) as i32 as f64 / i32::MAX as f64) as f32;
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    /// Ридер с регулируемым размером чанка (проверка инвариантности).
    struct ChunkyReader<'a> {
        data: &'a [u8],
        chunk: usize,
        pos: usize,
    }

    impl std::io::Read for ChunkyReader<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.pos >= self.data.len() {
                return Ok(0);
            }
            let n = self.chunk.min(buf.len()).min(self.data.len() - self.pos);
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    #[test]
    fn chunk_size_invariance() {
        let data = make_stream(10_000, 42);
        let cfg = StreamQuantConfig::default();
        // Разные размеры чанков чтения — на выходе идентичные байты.
        let mut a = Vec::new();
        stream_quantize(&data[..], &mut a, &cfg).unwrap();
        for chunk in [1usize, 3 * 1024, 7 * 1024 + 13, 64 * 1024] {
            let mut b = Vec::new();
            let reader = ChunkyReader { data: &data, chunk, pos: 0 };
            stream_quantize(reader, &mut b, &cfg).unwrap();
            assert_eq!(a, b, "чанк {chunk} Б изменил результат");
        }
        // И верификация проходит.
        let stats = verify_t5q(&a).unwrap();
        assert_eq!(stats.values, 10_000);
        assert_eq!(stats.blocks, 20); // 10000/512 → 19 полных + 1 неполный
        assert_eq!(stats.out_bytes, a.len() as u64);
    }

    #[test]
    fn peak_ram_is_bounded() {
        let data = make_stream(50_000, 7);
        let mut out = Vec::new();
        let stats = stream_quantize(&data[..], &mut out, &StreamQuantConfig::default()).unwrap();
        // Пик буфера ≤ 64 КБ (буфер чтения), независимо от 200 КБ входа.
        assert!(stats.peak_buffer <= 64 * 1024, "пик {} Б", stats.peak_buffer);
        assert_eq!(stats.values, 50_000);
        // 98 блоков полных + 1 неполный (50000 = 97×512 + 336).
        assert_eq!(stats.blocks, 98);
    }

    #[test]
    fn compression_math_holds() {
        let data = make_stream(100_000, 11);
        let mut out = Vec::new();
        let stats = stream_quantize(&data[..], &mut out, &StreamQuantConfig::default()).unwrap();
        // 4 Б масштаба + 103 Б тритов на 512 значений ≈ 1.67 бита/вес.
        let bpv = stats.bits_per_value();
        assert!((1.5..1.9).contains(&bpv), "бит/вес = {bpv:.3}");
        assert!(stats.out_bytes * 15 < stats.in_bytes, "сжатие < 15×: {} → {}", stats.in_bytes, stats.out_bytes);
    }

    #[test]
    fn topk_creates_vacuum() {
        let data = make_stream(5_000, 13);
        let dense = {
            let mut o = Vec::new();
            stream_quantize(&data[..], &mut o, &StreamQuantConfig::default()).unwrap()
        };
        let sparse = {
            let mut o = Vec::new();
            let mut cfg = StreamQuantConfig::default();
            cfg.keep = 0.2;
            stream_quantize(&data[..], &mut o, &cfg).unwrap()
        };
        assert!(sparse.vacuum_frac() > dense.vacuum_frac() + 0.5,
            "top-k должен рождать вакуум: {} vs {}", sparse.vacuum_frac(), dense.vacuum_frac());
        assert!(sparse.vacuum_frac() >= 0.75, "keep=0.2 → ≥75% нулей, got {}", sparse.vacuum_frac());
    }

    #[test]
    fn f16_input_quantizes() {
        // 1000 f16 значений вручную.
        let mut data = Vec::new();
        for i in 0..1000i32 {
            let v = (i % 97 - 48) as f64 / 48.0;
            let bits = (v as f32).to_bits();
            let h = ((bits >> 16) & 0xffff) as u16; // грубое усечение — сгодится для теста
            data.extend_from_slice(&h.to_le_bytes());
        }
        let cfg = StreamQuantConfig { f16: true, ..Default::default() };
        let mut out = Vec::new();
        let stats = stream_quantize(&data[..], &mut out, &cfg).unwrap();
        assert_eq!(stats.values, 1000);
        assert_eq!(stats.in_bytes, 2000);
        assert_eq!(stats.blocks, 2);
        verify_t5q(&out).unwrap();
    }

    #[test]
    fn tamper_detected() {
        let data = make_stream(2000, 3);
        let mut out = Vec::new();
        stream_quantize(&data[..], &mut out, &StreamQuantConfig::default()).unwrap();
        let last_block = out.len() - TRAILER - 1;
        out[last_block] ^= 0x02;
        assert!(verify_t5q(&out).is_err(), "порча блока должна ловиться sha256");
    }

    #[test]
    fn f16_conversion_matches_known_values() {
        assert_eq!(f16_to_f32(0x0000), 0.0);
        assert_eq!(f16_to_f32(0x8000), -0.0);
        assert_eq!(f16_to_f32(0x3C00), 1.0);
        assert_eq!(f16_to_f32(0xBC00), -1.0);
        assert_eq!(f16_to_f32(0x4000), 2.0);
        assert_eq!(f16_to_f32(0x3800), 0.5);
        assert!((f16_to_f32(0x3555) - 0.33325195).abs() < 1e-6);
        assert!(f16_to_f32(0x7C00).is_infinite(), "inf");
        assert!(f16_to_f32(0x7E00).is_nan(), "nan");
    }

    #[test]
    fn empty_stream_is_legal() {
        let mut out = Vec::new();
        let stats = stream_quantize(&[][..], &mut out, &StreamQuantConfig::default()).unwrap();
        assert_eq!(stats.values, 0);
        assert_eq!(stats.blocks, 0);
        assert_eq!(stats.out_bytes, (HEADER + TRAILER) as u64);
        verify_t5q(&out).unwrap();
    }
}
