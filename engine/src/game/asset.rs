//! # Y «Эхо»: нейронное квантование готовых ассетов (цикл Y, v0.61.0)
//!
//! Задание владельца: «готовые звуки/ассеты квантуются в наш PQW через
//! нейросеть — нейроны, синапсы, всё уже в проекте. Готовое — не значит
//! [взять как есть]: это значит оно ГЕНЕРИРУЕТСЯ С НУЛЯ поверх
//! готового». Готовый ассет — не файл, который кладут рядом, а УЧИТЕЛЬ:
//! движок впитывает его структуру в трит-синапсы и потом поёт сам.
//!
//! ```text
//!   готовый ассет (WAV/PNG на диске)
//!        │  1. разбор контейнера — свой: WAV-PCM16, PNG+inflate, PPM
//!        ▼
//!   2. НЕЙРОНЫ: полосные нейроны спектра (звук) / прототипы тайлов
//!      (текстура) — один проход, без эпох градиентного спуска
//!        ▼
//!   3. СИНАПСЫ: STDP-граф лаг-корреляций полос (кто за кем fires —
//!      лаг-1 корреляция и есть асимметричное окно STDP) / граф
//!      смежности тайлов (Хебб по растру)
//!        ▼
//!   4. PQW-контейнер .poler: топология = дуги графа, фазы = трит + σ
//!      (ошибка квантования ≤ 1/126), гипер-параметры = глобалы
//!        ▼
//!   5. EMIT: движок генерирует С НУЛЯ — живой SSN-вихрь (600 нейронов,
//!      критичность 5%, лавины) рождает тайминг обрушений, синапсы
//!      задают спектральную связность, синтез — случайные фазы + OLA
//!        │
//!        ▼
//!   регенерированный ассет: 100% просчитан нашим движком
//! ```
//!
//! Честность метрик: регенерация — ДРУГАЯ реализация тех же статистик
//! (как вторая волна в том же море), поэтому сэмпл-точный PSNR для
//! звука не определён — меряются PSD-дистанция по полосам (дБ),
//! модуляционный спектр и детерминизм бит-в-бит; для текстуры — PSNR
//! против оригинала (та же решётка) и гистограммная дистанция.
//!
//! Дисциплина (всё как в T1/T2/V0): ноль внешних зависимостей — свой
//! WAV-ридер, свой inflate (RFC 1951: stored + fixed + dynamic Huffman),
//! свой PNG-анфильтр (Paeth включительно); FFT — общий с T1.

use std::path::Path;

// ---------------------------------------------------------------------------
// Контейнеры: WAV
// ---------------------------------------------------------------------------

/// Прочитанный WAV: PCM16, любое число каналов, сэмплы interleaved f32.
#[derive(Debug, Clone)]
pub struct WavData {
    pub fs: u32,
    pub channels: u16,
    /// Interleaved сэмплы ∈ [-1, 1], len = frames × channels.
    pub samples: Vec<f32>,
}

impl WavData {
    pub fn duration_s(&self) -> f64 {
        self.samples.len() as f64 / (self.fs as f64 * self.channels.max(1) as f64)
    }
    /// Моно-микс (среднее каналов).
    pub fn mono(&self) -> Vec<f64> {
        let ch = self.channels.max(1) as usize;
        let frames = self.samples.len() / ch;
        let mut out = vec![0.0f64; frames];
        for f in 0..frames {
            let mut acc = 0.0;
            for c in 0..ch {
                acc += self.samples[f * ch + c] as f64;
            }
            out[f] = acc / ch as f64;
        }
        out
    }
    /// FNV-1a по битам f32-сэмплов — контракт бит-в-бит (как audio_hash T1).
    pub fn audio_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for s in &self.samples {
            for b in s.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        h
    }
}

/// Чтение WAV (PCM16; формат 3/float32 тоже принимается). RIFF-чанки
/// перебираются до `data` — нестандартные пропускаются.
pub fn read_wav(path: &Path) -> Result<WavData, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("чтение {}: {e}", path.display()))?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("не RIFF/WAVE".into());
    }
    let mut fs = 0u32;
    let mut channels = 0u16;
    let mut bits = 0u16;
    let mut fmt_tag = 0u16;
    let mut data: Option<&[u8]> = None;
    let mut pos = 12usize;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32::from_le_bytes([bytes[pos + 4], bytes[pos + 5], bytes[pos + 6], bytes[pos + 7]]) as usize;
        let body_start = pos + 8;
        let body_end = body_start.checked_add(len).filter(|&e| e <= bytes.len()).ok_or("чанк WAV длиннее файла")?;
        if id == b"fmt " && len >= 16 {
            fmt_tag = u16::from_le_bytes([bytes[body_start], bytes[body_start + 1]]);
            channels = u16::from_le_bytes([bytes[body_start + 2], bytes[body_start + 3]]);
            fs = u32::from_le_bytes([bytes[body_start + 4], bytes[body_start + 5], bytes[body_start + 6], bytes[body_start + 7]]);
            bits = u16::from_le_bytes([bytes[body_start + 14], bytes[body_start + 15]]);
        } else if id == b"data" {
            data = Some(&bytes[body_start..body_end]);
        }
        pos = body_end + (len & 1); // чанки выровнены на 2
    }
    let data = data.ok_or("WAV без чанка data")?;
    if channels == 0 || fs == 0 {
        return Err("WAV без fmt или битый fmt".into());
    }
    let samples: Vec<f32> = match (fmt_tag, bits) {
        (1, 16) => data
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
            .collect(),
        (3, 32) => data
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect(),
        _ => return Err(format!("WAV: поддерживается PCM16/float32, получен tag={fmt_tag} bits={bits}")),
    };
    let frames = samples.len() / channels as usize;
    let samples: Vec<f32> = samples.into_iter().take(frames * channels as usize).collect();
    Ok(WavData { fs, channels, samples })
}

/// Запись WAV PCM16 (interleaved). Возвращает размер файла.
pub fn write_wav(path: &Path, fs: u32, channels: u16, samples: &[f32]) -> Result<u64, String> {
    use std::io::Write;
    let ch = channels.max(1) as u32;
    let frames = (samples.len() / ch as usize) as u32;
    let data_len = frames * ch * 2;
    let mut buf = Vec::with_capacity(44 + data_len as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_len).to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&(ch as u16).to_le_bytes());
    buf.extend_from_slice(&fs.to_le_bytes());
    buf.extend_from_slice(&(fs * ch * 2).to_le_bytes());
    buf.extend_from_slice(&((ch * 2) as u16).to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let q = (s.clamp(-1.0, 1.0) as f64 * 32767.0).round() as i16;
        buf.extend_from_slice(&q.to_le_bytes());
    }
    let mut f = std::fs::File::create(path).map_err(|e| format!("запись {}: {e}", path.display()))?;
    f.write_all(&buf).map_err(|e| e.to_string())?;
    Ok(buf.len() as u64)
}

// ---------------------------------------------------------------------------
// Контейнеры: inflate (RFC 1951) — декодер, публичное достояние алгоритма
// ---------------------------------------------------------------------------

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    cur: u32,
    nbits: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader { data, pos: 0, cur: 0, nbits: 0 }
    }
    /// LSB-first чтение n ≤ 16 бит.
    fn bits(&mut self, n: u32) -> Result<u32, String> {
        while self.nbits < n {
            let b = *self.data.get(self.pos).ok_or("inflate: обрыв потока")? as u32;
            self.pos += 1;
            self.cur |= b << self.nbits;
            self.nbits += 8;
        }
        let v = self.cur & ((1u32 << n) - 1);
        self.cur >>= n;
        self.nbits -= n;
        Ok(v)
    }
    fn align(&mut self) {
        self.cur = 0;
        self.nbits = 0;
    }
}

/// Канонический код Хаффмана (схема «puff»: counts/symbols).
struct Huffman {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

fn build_huffman(lengths: &[u8]) -> Result<Huffman, String> {
    let mut counts = [0u16; 16];
    for &l in lengths {
        counts[l as usize] += 1;
    }
    counts[0] = 0;
    // Проверка переполнения кодового пространства.
    let mut left: i32 = 1;
    for len in 1..16 {
        left <<= 1;
        left -= counts[len] as i32;
        if left < 0 {
            return Err("inflate: переполнение Хаффмана".into());
        }
    }
    let mut offs = [0u16; 16];
    for len in 1..15 {
        offs[len + 1] = offs[len] + counts[len];
    }
    let mut symbols = vec![0u16; lengths.iter().filter(|&&l| l != 0).count()];
    for (sym, &l) in lengths.iter().enumerate() {
        if l != 0 {
            symbols[offs[l as usize] as usize] = sym as u16;
            offs[l as usize] += 1;
        }
    }
    Ok(Huffman { counts, symbols })
}

fn decode_symbol(br: &mut BitReader, h: &Huffman) -> Result<u16, String> {
    let mut code: i32 = 0;
    let mut first: i32 = 0;
    let mut index: i32 = 0;
    for len in 1..16 {
        code |= br.bits(1)? as i32;
        let count = h.counts[len] as i32;
        if code - first < count {
            return Ok(h.symbols[(index + (code - first)) as usize]);
        }
        index += count;
        first = (first + count) << 1;
        code <<= 1;
    }
    Err("inflate: битый символ Хаффмана".into())
}

/// Длина/дистанция: база + доп. биты (таблицы RFC 1951 §3.2.5).
const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
    131, 163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
    13, 13,
];
const CLEN_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// Inflate: zlib-agnostic RAW deflate-поток (без 2-байтовой zlib-обёртки —
/// её снимает вызывающий PNG-парсер).
pub fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut br = BitReader::new(data);
    let mut out: Vec<u8> = Vec::with_capacity(data.len() * 4 + 64);
    loop {
        let bfinal = br.bits(1)?;
        let btype = br.bits(2)?;
        match btype {
            0 => {
                // stored: выравнивание + LEN/NLEN + сырые байты
                br.align();
                if br.pos + 4 > br.data.len() {
                    return Err("inflate: обрыв stored-заголовка".into());
                }
                let len = u16::from_le_bytes([br.data[br.pos], br.data[br.pos + 1]]) as usize;
                let nlen = u16::from_le_bytes([br.data[br.pos + 2], br.data[br.pos + 3]]) as usize;
                if len != (!nlen & 0xFFFF) {
                    return Err("inflate: NLEN не инвертирует LEN".into());
                }
                br.pos += 4;
                let end = br.pos + len;
                if end > br.data.len() {
                    return Err("inflate: обрыв stored-данных".into());
                }
                out.extend_from_slice(&br.data[br.pos..end]);
                br.pos = end;
            }
            1 => {
                // fixed Huffman
                let mut lit_len = [0u8; 288];
                for (i, l) in lit_len.iter_mut().enumerate() {
                    *l = if i < 144 {
                        8
                    } else if i < 256 {
                        9
                    } else if i < 280 {
                        7
                    } else {
                        8
                    };
                }
                let dist_len = [5u8; 30];
                let lit = build_huffman(&lit_len)?;
                let dist = build_huffman(&dist_len)?;
                inflate_block(&mut br, &lit, &dist, &mut out)?;
            }
            2 => {
                // dynamic Huffman
                let hlit = br.bits(5)? as usize + 257;
                let hdist = br.bits(5)? as usize + 1;
                let hclen = br.bits(4)? as usize + 4;
                let mut clen = [0u8; 19];
                for &o in CLEN_ORDER.iter().take(hclen) {
                    clen[o] = br.bits(3)? as u8;
                }
                let cl_h = build_huffman(&clen)?;
                let mut lengths = vec![0u8; hlit + hdist];
                let mut i = 0usize;
                while i < lengths.len() {
                    let sym = decode_symbol(&mut br, &cl_h)?;
                    match sym {
                        0..=15 => {
                            lengths[i] = sym as u8;
                            i += 1;
                        }
                        16 => {
                            if i == 0 {
                                return Err("inflate: repeat без предыдущего".into());
                            }
                            let prev = lengths[i - 1];
                            let rep = 3 + br.bits(2)? as usize;
                            for _ in 0..rep {
                                if i >= lengths.len() {
                                    return Err("inflate: repeat за границей".into());
                                }
                                lengths[i] = prev;
                                i += 1;
                            }
                        }
                        17 => i += 3 + br.bits(3)? as usize,
                        18 => i += 11 + br.bits(7)? as usize,
                        _ => return Err("inflate: битый код длин".into()),
                    }
                }
                if i > lengths.len() {
                    return Err("inflate: коды длин за границей".into());
                }
                let lit = build_huffman(&lengths[..hlit])?;
                let dist = build_huffman(&lengths[hlit..])?;
                inflate_block(&mut br, &lit, &dist, &mut out)?;
            }
            _ => return Err("inflate: BTYPE=3 запрещён".into()),
        }
        if bfinal == 1 {
            break;
        }
    }
    Ok(out)
}

fn inflate_block(
    br: &mut BitReader,
    lit: &Huffman,
    dist: &Huffman,
    out: &mut Vec<u8>,
) -> Result<(), String> {
    loop {
        let sym = decode_symbol(br, lit)?;
        match sym {
            0..=255 => out.push(sym as u8),
            256 => return Ok(()),
            257..=285 => {
                let li = (sym - 257) as usize;
                let len = LEN_BASE[li] as usize + br.bits(LEN_EXTRA[li] as u32)? as usize;
                let ds = decode_symbol(br, dist)? as usize;
                if ds >= 30 {
                    return Err("inflate: дистанция ≥ 30".into());
                }
                let d = DIST_BASE[ds] as usize + br.bits(DIST_EXTRA[ds] as u32)? as usize;
                if d > out.len() || d == 0 {
                    return Err("inflate: дистанция вне окна".into());
                }
                // Побайтно: LZ77-копия может перекрывать саму себя.
                let start = out.len() - d;
                for k in 0..len {
                    let b = out[start + k];
                    out.push(b);
                }
            }
            _ => return Err("inflate: литерал ≥ 286".into()),
        }
    }
}

// ---------------------------------------------------------------------------
// Контейнеры: PNG / PNM → GrayImage
// ---------------------------------------------------------------------------

/// Полутоновое изображение [0, 255].
#[derive(Debug, Clone)]
pub struct GrayImage {
    pub w: u32,
    pub h: u32,
    pub gray: Vec<u8>,
}

/// Читает изображение как полутон: PNG (8-bit gray/RGB, любые фильтры),
/// PGM P5 / PPM P6. Тип — по магике, не по расширению.
pub fn read_image(path: &Path) -> Result<GrayImage, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("чтение {}: {e}", path.display()))?;
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        png_luma(&bytes)
    } else if bytes.starts_with(b"P5") || bytes.starts_with(b"P6") {
        pnm_luma(&bytes)
    } else {
        Err("поддерживаются PNG (8-bit gray/RGB) и PGM/PPM (P5/P6)".into())
    }
}

fn png_luma(bytes: &[u8]) -> Result<GrayImage, String> {
    if bytes.len() < 8 + 25 {
        return Err("PNG: короткий файл".into());
    }
    let mut pos = 8usize;
    let (mut w, mut h) = (0u32, 0u32);
    let mut bit_depth = 0u8;
    let mut color_type = 0u8;
    let mut interlace = 0u8;
    let mut idat: Vec<u8> = Vec::new();
    while pos + 8 <= bytes.len() {
        let len = u32::from_be_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]) as usize;
        let ctype = &bytes[pos + 4..pos + 8];
        let body = pos + 8;
        let end = body.checked_add(len).filter(|&e| e <= bytes.len()).ok_or("PNG: чанк длиннее файла")?;
        if end > bytes.len() {
            return Err("PNG: чанк длиннее файла".into());
        }
        match ctype {
            b"IHDR" => {
                if len < 13 {
                    return Err("PNG: короткий IHDR".into());
                }
                w = u32::from_be_bytes([bytes[body], bytes[body + 1], bytes[body + 2], bytes[body + 3]]);
                h = u32::from_be_bytes([bytes[body + 4], bytes[body + 5], bytes[body + 6], bytes[body + 7]]);
                bit_depth = bytes[body + 8];
                color_type = bytes[body + 9];
                interlace = bytes[body + 12];
            }
            b"IDAT" => idat.extend_from_slice(&bytes[body..end]),
            b"IEND" => break,
            _ => {}
        }
        pos = end + 4; // CRC
    }
    if w == 0 || h == 0 {
        return Err("PNG: нет IHDR".into());
    }
    if bit_depth != 8 || !matches!(color_type, 0 | 2) {
        return Err(format!("PNG: нужен 8-bit gray(0)/RGB(2), получено depth={bit_depth} color={color_type}"));
    }
    if interlace != 0 {
        return Err("PNG: interlace не поддерживается".into());
    }
    if idat.is_empty() {
        return Err("PNG: нет IDAT".into());
    }
    // zlib-обёртка: 2 байта заголовка + 4 байта adler32.
    if idat.len() < 6 {
        return Err("PNG: IDAT короче zlib-заголовка".into());
    }
    let raw = inflate(&idat[2..idat.len() - 4])?;
    let bpp: usize = if color_type == 0 { 1 } else { 3 };
    let stride = w as usize * bpp;
    let expect = (stride + 1) * h as usize;
    if raw.len() < expect {
        return Err(format!("PNG: распаковано {} байт, ожидалось {expect}", raw.len()));
    }
    // Анфильтр построчно.
    let mut img = vec![0u8; stride * h as usize];
    let mut prev = vec![0u8; stride];
    for y in 0..h as usize {
        let ft = raw[y * (stride + 1)];
        let line = &raw[y * (stride + 1) + 1..y * (stride + 1) + 1 + stride];
        for x in 0..stride {
            let a = if x >= bpp { img[y * stride + x - bpp] } else { 0 };
            let b = prev[x];
            let c = if x >= bpp { prev[x - bpp] } else { 0 };
            let v = match ft {
                0 => line[x],
                1 => line[x].wrapping_add(a),
                2 => line[x].wrapping_add(b),
                3 => line[x].wrapping_add(((a as u16 + b as u16) / 2) as u8),
                4 => line[x].wrapping_add(paeth(a, b, c)),
                _ => return Err(format!("PNG: фильтр {ft}")),
            };
            img[y * stride + x] = v;
        }
        prev.copy_from_slice(&img[y * stride..y * stride + stride]);
    }
    let gray = if color_type == 0 {
        img
    } else {
        img.chunks_exact(3)
            .map(|p| ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8)
            .collect()
    };
    Ok(GrayImage { w, h, gray })
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i32 + b as i32 - c as i32;
    let pa = (p - a as i32).abs();
    let pb = (p - b as i32).abs();
    let pc = (p - c as i32).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn pnm_luma(bytes: &[u8]) -> Result<GrayImage, String> {
    let six = bytes[1] == b'6';
    let mut pos = 2usize;
    let mut vals = [0u32; 3];
    let mut vi = 0usize;
    while vi < 3 {
        // пропуск пробелов/комментариев
        while pos < bytes.len() && (bytes[pos] as char).is_whitespace() {
            pos += 1;
        }
        if pos < bytes.len() && bytes[pos] == b'#' {
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        let mut v = 0u32;
        let mut got = false;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            v = v * 10 + (bytes[pos] - b'0') as u32;
            pos += 1;
            got = true;
        }
        if !got {
            return Err("PNM: битый заголовок".into());
        }
        vals[vi] = v;
        vi += 1;
    }
    pos += 1; // один разделитель после maxval
    let (w, h, maxval) = (vals[0], vals[1], vals[2]);
    if w == 0 || h == 0 || maxval == 0 || maxval > 255 {
        return Err("PNM: плохие размеры/maxval".into());
    }
    let ch = if six { 3 } else { 1 };
    let need = w as usize * h as usize * ch;
    let body = bytes.get(pos..pos + need).ok_or("PNM: тело короче w×h")?;
    let scale = |v: u8| (v as u32 * 255 / maxval).min(255) as u8;
    let gray = if six {
        body.chunks_exact(3)
            .map(|p| ((scale(p[0]) as u32 * 299 + scale(p[1]) as u32 * 587 + scale(p[2]) as u32 * 114) / 1000) as u8)
            .collect()
    } else {
        body.iter().map(|&v| scale(v)).collect()
    };
    Ok(GrayImage { w, h, gray })
}

// ---------------------------------------------------------------------------
// Y-аудио: полосные нейроны + STDP-граф → PQW → синтез с нуля
// ---------------------------------------------------------------------------

/// Число полосных нейронов (спектральных).
pub const B: usize = 24;
/// Окно STFT.
pub const STFT_N: usize = 1024;
/// Шаг STFT (50% перекрытие — COLA для Ханна).
pub const STFT_HOP: usize = 512;
/// Усреднение PSD для динамики/наклона: столько последних кадров.
const FRAMES_CAP: usize = 8192;

/// Бины полосы (lo..hi, half-spectrum без DC) для данного fs.
/// Полосы геометрические от max(50 Гц, 2 бина) до min(16 кГц, 0.45·fs);
/// пустые полосы (когда бина не хватило) — None и молчат.
pub fn band_ranges(fs: u32) -> Vec<Option<(usize, usize)>> {
    let n_bins = STFT_N / 2;
    let bin_hz = fs as f64 / STFT_N as f64;
    let f_lo = (50.0f64).max(2.0 * bin_hz);
    let f_hi = (0.45 * fs as f64).min(16_000.0);
    let mut out: Vec<Option<(usize, usize)>> = vec![None; B];
    if f_hi <= f_lo {
        return out;
    }
    let lg = (f_hi / f_lo).ln();
    for k in 1..=n_bins {
        let f = k as f64 * bin_hz;
        if f < f_lo || f >= f_hi {
            continue;
        }
        let b = ((f / f_lo).ln() / lg * B as f64) as usize;
        let b = b.min(B - 1);
        match &mut out[b] {
            Some((_, hi)) if *hi == k => *hi = k + 1,
            Some((lo, hi)) if k == *hi => *hi = k + 1,
            Some(_) => return out, // не должно случиться: полосы монотонны
            None => out[b] = Some((k, k + 1)),
        }
    }
    out
}

/// Центр полосы (Гц) — для наклона внутри полосы.
fn band_center(lo: usize, hi: usize, fs: u32) -> f64 {
    let bin_hz = fs as f64 / STFT_N as f64;
    ((lo as f64 * bin_hz) * (hi as f64 * bin_hz)).sqrt()
}

/// Впитанное эхо звука: нейроны (полосы) + синапсы (STDP-граф) + статистики.
#[derive(Debug, Clone)]
pub struct AudioEcho {
    /// STDP-граф B×B: G[i][j] = лаг-1 корреляция z_i(t)·z_j(t+1) ∈ [-1, 1].
    /// Асимметрия = направление времени: i СВЕТИТ раньше j ⇒ G[i][j] > G[j][i].
    pub graph: Vec<f32>,
    /// Средняя энергия полосы, дБ.
    pub mean_db: Vec<f32>,
    /// Динамический размах полосы (σ лог-энергии), дБ.
    pub dyn_db: Vec<f32>,
    /// Глубина модуляции полосы [0, 1].
    pub mod_depth: Vec<f32>,
    /// Доминирующая частота модуляции (каденс обрушений), Гц.
    pub mod_hz: Vec<f32>,
    /// Наклон PSD внутри полосы, дБ/октаву.
    pub tilt: Vec<f32>,
    pub fs: u32,
    /// Корреляция L/R оригинала.
    pub stereo_corr: f32,
    /// RMS оригинала, дБ.
    pub level_db: f32,
}

/// STFT-кадры: лог-энергии полос (дБ) и PSD-среднее по кадрам.
fn stft_band_log_energy(mono: &[f64], fs: u32) -> (Vec<Vec<f64>>, Vec<f64>) {
    let frames = 1 + mono.len().saturating_sub(STFT_N) / STFT_HOP;
    let mut hann = vec![0.0f64; STFT_N];
    for (n, w) in hann.iter_mut().enumerate() {
        *w = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * n as f64 / STFT_N as f64).cos());
    }
    let ranges = band_ranges(fs);
    let stride = (frames / FRAMES_CAP).max(1);
    let mut e_db: Vec<Vec<f64>> = Vec::new();
    let mut psd_sum = vec![0.0f64; STFT_N / 2 + 1];
    let mut psd_frames = 0usize;
    for fi in 0..frames {
        let mut re = vec![0.0f64; STFT_N];
        let mut im = vec![0.0f64; STFT_N];
        let base = fi * STFT_HOP;
        for n in 0..STFT_N {
            re[n] = mono[base + n] * hann[n];
        }
        crate::game::audio::fft_inplace(&mut re, &mut im);
        if fi % stride == 0 {
            let mut row = vec![f64::NEG_INFINITY; B];
            for (b, rng) in ranges.iter().enumerate() {
                if let Some((lo, hi)) = *rng {
                    let mut acc = 0.0;
                    for k in lo..hi {
                        let p = re[k] * re[k] + im[k] * im[k];
                        psd_sum[k] += p;
                        acc += p;
                    }
                    row[b] = 10.0 * (acc / (hi - lo) as f64 + 1e-20).log10();
                }
            }
            e_db.push(row);
            psd_frames += 1;
        }
    }
    for v in psd_sum.iter_mut() {
        *v /= psd_frames.max(1) as f64;
    }
    (e_db, psd_sum)
}

impl AudioEcho {
    /// Доминирующий синапс (пара полос) — для отчёта.
    pub fn strongest_synapse(&self) -> (usize, usize, f32) {
        let mut best = (0usize, 0usize, 0.0f32);
        for i in 0..B {
            for j in 0..B {
                let v = self.graph[i * B + j];
                if v.abs() > best.2.abs() {
                    best = (i, j, v);
                }
            }
        }
        best
    }
}

/// Впитать WAV: полосные нейроны + STDP-синапсы + статистики.
pub fn absorb_audio(wav: &WavData) -> Result<AudioEcho, String> {
    let mono = wav.mono();
    if mono.len() < STFT_N * 4 {
        return Err(format!("звук короче {} с — нечему учиться", (STFT_N * 4) as f64 / wav.fs as f64));
    }
    let (e_db, psd_sum) = stft_band_log_energy(&mono, wav.fs);
    let t = e_db.len();
    if t < 8 {
        return Err("кадров STFT меньше 8".into());
    }
    // z-нормализация лог-энергий по полосам.
    let mut mean_db = vec![0.0f32; B];
    let mut dyn_db = vec![0.0f32; B];
    let mut z = vec![vec![0.0f64; B]; t];
    for b in 0..B {
        let mut m = 0.0;
        for ti in 0..t {
            m += e_db[ti][b];
        }
        m /= t as f64;
        let mut s = 0.0;
        for ti in 0..t {
            s += (e_db[ti][b] - m) * (e_db[ti][b] - m);
        }
        let sd = (s / t as f64).sqrt().max(1e-6);
        for ti in 0..t {
            z[ti][b] = if e_db[ti][b] == f64::NEG_INFINITY { 0.0 } else { (e_db[ti][b] - m) / sd };
        }
        mean_db[b] = if m == f64::NEG_INFINITY { -60.0 } else { m as f32 };
        dyn_db[b] = sd as f32;
    }
    // STDP-граф: лаг-1 корреляция (асимметричное окно во времени).
    let mut graph = vec![0.0f32; B * B];
    for i in 0..B {
        for j in 0..B {
            let mut acc = 0.0;
            for ti in 0..t - 1 {
                acc += z[ti][i] * z[ti + 1][j];
            }
            graph[i * B + j] = (acc / (t - 1) as f64).clamp(-1.0, 1.0) as f32;
        }
    }
    // Модуляционный спектр каждой полосы: каденс обрушений.
    let frame_rate = wav.fs as f64 / STFT_HOP as f64;
    let mut mod_depth = vec![0.0f32; B];
    let mut mod_hz = vec![0.0f32; B];
    let mut spec = vec![0.0f64; B];
    for b in 0..B {
        // прореживание до ≤4096 точек
        let stride = (t / 4096).max(1);
        let mut series: Vec<f64> = (0..t).step_by(stride).map(|ti| z[ti][b]).collect();
        let rate = frame_rate / stride as f64;
        let mut pad = 16usize;
        while pad < series.len() {
            pad *= 2;
        }
        series.resize(pad, 0.0);
        let mut re = series;
        let mut im = vec![0.0f64; pad];
        crate::game::audio::fft_inplace(&mut re, &mut im);
        let mut total = 0.0f64;
        let mut in_band = 0.0f64;
        let mut peak_i = 0usize;
        let mut peak_v = 0.0f64;
        for k in 1..pad / 2 {
            let p = re[k] * re[k] + im[k] * im[k];
            let f = k as f64 * rate / pad as f64;
            total += p;
            if (0.05..=8.0).contains(&f) {
                in_band += p;
                if p > peak_v {
                    peak_v = p;
                    peak_i = k;
                }
            }
        }
        if total > 1e-12 {
            mod_depth[b] = (in_band / total).min(1.0) as f32;
            mod_hz[b] = (peak_i as f64 * rate / pad as f64) as f32;
        }
        spec[b] = peak_i as f64 * rate / pad as f64;
    }
    let _ = spec;
    // Наклон PSD внутри полосы (дБ/октаву).
    let ranges = band_ranges(wav.fs);
    let bin_hz = wav.fs as f64 / STFT_N as f64;
    let mut tilt = vec![0.0f32; B];
    for (b, rng) in ranges.iter().enumerate() {
        if let Some((lo, hi)) = *rng {
            let fc = band_center(lo, hi, wav.fs);
            let mut sx = 0.0;
            let mut sy = 0.0;
            let mut sxx = 0.0;
            let mut sxy = 0.0;
            let mut n = 0.0;
            for k in lo..hi {
                let x = (k as f64 * bin_hz / fc).log2();
                let y = 10.0 * (psd_sum[k] + 1e-20).log10();
                sx += x;
                sy += y;
                sxx += x * x;
                sxy += x * y;
                n += 1.0;
            }
            if n >= 2.0 {
                let det = n * sxx - sx * sx;
                if det.abs() > 1e-9 {
                    tilt[b] = ((n * sxy - sx * sy) / det).clamp(-12.0, 12.0) as f32;
                }
            }
        }
    }
    // Стерео-корреляция и уровень.
    let stereo_corr = if wav.channels >= 2 {
        let ch = wav.channels as usize;
        let frames = wav.samples.len() / ch;
        let mut sl = 0.0;
        let mut sr = 0.0;
        let mut slr = 0.0;
        let mut sll = 0.0;
        let mut srr = 0.0;
        for f in 0..frames {
            let l = wav.samples[f * ch] as f64;
            let r = wav.samples[f * ch + 1] as f64;
            sl += l;
            sr += r;
            slr += l * r;
            sll += l * l;
            srr += r * r;
        }
        let n = frames as f64;
        let cov = slr / n - (sl / n) * (sr / n);
        let sl_ = (sll / n - (sl / n).powi(2)).sqrt();
        let sr_ = (srr / n - (sr / n).powi(2)).sqrt();
        if sl_ > 1e-9 && sr_ > 1e-9 {
            (cov / (sl_ * sr_)).clamp(-1.0, 1.0) as f32
        } else {
            1.0
        }
    } else {
        1.0
    };
    let rms = (mono.iter().map(|s| s * s).sum::<f64>() / mono.len() as f64).sqrt();
    let level_db = (20.0 * (rms + 1e-9).log10()).clamp(-60.0, -3.0) as f32;
    Ok(AudioEcho {
        graph,
        mean_db,
        dyn_db,
        mod_depth,
        mod_hz,
        tilt,
        fs: wav.fs,
        stereo_corr,
        level_db,
    })
}

// --- кодирование в PQW -----------------------------------------------------

/// Отображения параметров в фазу [-1, 1] и обратно.
fn map_mean_db(v: f32) -> f32 {
    ((v + 60.0) / 80.0 * 2.0 - 1.0).clamp(-1.0, 1.0)
}
fn unmap_mean_db(p: f64) -> f32 {
    ((p + 1.0) / 2.0 * 80.0 - 60.0) as f32
}
fn map_dyn_db(v: f32) -> f32 {
    (v / 36.0 * 2.0 - 1.0).clamp(-1.0, 1.0)
}
fn unmap_dyn_db(p: f64) -> f32 {
    ((p + 1.0) / 2.0 * 36.0) as f32
}
fn map_hz(v: f32) -> f32 {
    let lo: f32 = 0.05;
    let hi: f32 = 8.0;
    (((v.max(lo) / lo).ln() / (hi / lo).ln()) * 2.0 - 1.0).clamp(-1.0, 1.0)
}
fn unmap_hz(p: f64) -> f32 {
    (0.05f64 * (8.0f64 / 0.05f64).powf((p + 1.0) / 2.0)) as f32
}
fn map_level_db(v: f32) -> f32 {
    ((v + 60.0) / 60.0 * 2.0 - 1.0).clamp(-1.0, 1.0)
}
fn unmap_level_db(p: f64) -> f32 {
    ((p + 1.0) / 2.0 * 60.0 - 60.0) as f32
}

/// Сборка PQW-контейнера v1 из эха. Топология — дуги STDP-графа и
/// параметров полосных нейронов; гипер-параметры несут глобалы
/// (eta = fs > 0 — тег «аудио»).
pub fn audio_to_pqw(echo: &AudioEcho) -> Result<Vec<u8>, String> {
    use pqw_core::PqwWriter;
    let d_pol = (B * B + 5 * B) as u32;
    let mut w = PqwWriter::new(d_pol).map_err(|e| e.to_string())?
        .hyperparams(
            echo.fs as f32,
            echo.stereo_corr,
            map_level_db(echo.level_db),
            0.02,
        );
    let add = |w: &mut PqwWriter, i: u32, p: f32| -> Result<(), String> {
        w.add_phase(i, p).map_err(|e| e.to_string())?;
        Ok(())
    };
    for i in 0..B {
        for j in 0..B {
            let g = echo.graph[i * B + j];
            if g.abs() >= 0.04 {
                add(&mut w, (i * B + j) as u32, g)?;
            }
        }
    }
    for b in 0..B {
        let base = (B * B + b * 5) as u32;
        add(&mut w, base, map_mean_db(echo.mean_db[b]))?;
        add(&mut w, base + 1, map_dyn_db(echo.dyn_db[b]))?;
        add(&mut w, base + 2, echo.mod_depth[b] * 2.0 - 1.0)?;
        add(&mut w, base + 3, map_hz(echo.mod_hz[b]))?;
        add(&mut w, base + 4, echo.tilt[b] / 12.0)?;
    }
    w.to_bytes().map_err(|e| e.to_string())
}

/// Декодирование эха из PQW-байтов.
pub fn audio_from_pqw(bytes: &[u8]) -> Result<AudioEcho, String> {
    use pqw_core::PqwReader;
    let r = PqwReader::from_bytes(bytes).map_err(|e| e.to_string())?;
    let hyper = r.hyperparams();
    let fs = hyper.eta;
    if !fs.is_finite() || fs <= 0.0 {
        return Err("PQW не аудио-эхо (eta ≤ 0)".into());
    }
    let mut map = std::collections::HashMap::new();
    for (i, p) in r.decoded() {
        map.insert(i, p);
    }
    let mut graph = vec![0.0f32; B * B];
    let mut mean_db = vec![-80.0f32; B];
    let mut dyn_db = vec![4.0f32; B];
    let mut mod_depth = vec![0.0f32; B];
    let mut mod_hz = vec![0.2f32; B];
    let mut tilt = vec![0.0f32; B];
    for i in 0..B {
        for j in 0..B {
            if let Some(&p) = map.get(&((i * B + j) as u32)) {
                graph[i * B + j] = p as f32;
            }
        }
    }
    for b in 0..B {
        let base = (B * B + b * 5) as u32;
        if let Some(&p) = map.get(&base) {
            mean_db[b] = unmap_mean_db(p);
        }
        if let Some(&p) = map.get(&(base + 1)) {
            dyn_db[b] = unmap_dyn_db(p);
        }
        if let Some(&p) = map.get(&(base + 2)) {
            mod_depth[b] = ((p + 1.0) / 2.0) as f32;
        }
        if let Some(&p) = map.get(&(base + 3)) {
            mod_hz[b] = unmap_hz(p);
        }
        if let Some(&p) = map.get(&(base + 4)) {
            tilt[b] = (p * 12.0) as f32;
        }
    }
    Ok(AudioEcho {
        graph,
        mean_db,
        dyn_db,
        mod_depth,
        mod_hz,
        tilt,
        fs: fs as u32,
        stereo_corr: hyper.gamma.clamp(-1.0, 1.0),
        level_db: unmap_level_db(hyper.rho as f64),
    })
}

/// Seed из содержимого контейнера (content-addressed детерминизм):
/// одинаковые байты PQW → одинаковый океан, всегда.
fn seed_from_bytes(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Отчёт синтеза аудио.
#[derive(Debug, Clone)]
pub struct AudioEmitReport {
    pub seed: u64,
    pub frames: usize,
    pub vortex_steps: usize,
    pub vortex_activity: f64,
    pub vortex_criticality: f64,
    pub bursts: usize,
}

/// Синтез одной реализации (мид или сайд): случайные фазы + OLA.
#[allow(clippy::too_many_arguments)]
fn synth_stream(
    echo: &AudioEcho,
    frames: usize,
    vortex: &mut crate::ssn::vortex::SynapticVortex,
    rng: &mut crate::ssn::rng::Rng,
    burst_gain: f64,
    bursts: &mut usize,
) -> Vec<f64> {
    let ranges = band_ranges(echo.fs);
    let bin_hz = echo.fs as f64 / STFT_N as f64;
    let frame_rate = echo.fs as f64 / STFT_HOP as f64;
    let mut acc = vec![0.0f64; frames * STFT_HOP + STFT_N];
    let mut wsum = vec![0.0f64; frames * STFT_HOP + STFT_N];
    let mut hann = vec![0.0f64; STFT_N];
    for (n, w) in hann.iter_mut().enumerate() {
        *w = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * n as f64 / STFT_N as f64).cos());
    }
    let mut mod_phase = [0.0f64; B];
    for p in mod_phase.iter_mut() {
        *p = rng.f64() * 2.0 * std::f64::consts::PI;
    }
    // Позитивные части строк графа (нормировки когерентности).
    let mut gpos = vec![0.0f64; B * B];
    let mut gsum = [1e-9f64; B];
    for i in 0..B {
        for j in 0..B {
            let g = echo.graph[i * B + j].max(0.0) as f64;
            gpos[i * B + j] = g;
            gsum[i] += g;
        }
    }
    for fi in 0..frames {
        // Живой мозг: шаг вихря + топ-48 нейронов → драйв полос.
        // Абсолютная шкала: Σ a² по нейронам полосы / (48·0.4²) — лавина
        // (полный readout выше порога) даёт драйв → 1.
        vortex.step();
        let read = vortex.readout(48);
        *bursts += (read.len() >= 24) as usize;
        let mut drive = [0.0f64; B];
        for &(idx, act) in &read {
            let b = idx % B;
            let a = act.max(0.0);
            drive[b] += a * a;
        }
        const DRIVE_FULL: f64 = 48.0 * 0.4 * 0.4;
        for d in drive.iter_mut() {
            *d = (*d / DRIVE_FULL).min(1.0);
        }
        let t_s = fi as f64 / frame_rate;
        let mut re = vec![0.0f64; STFT_N];
        let mut im = vec![0.0f64; STFT_N];
        for (b, rngb) in ranges.iter().enumerate() {
            let Some((lo, hi)) = *rngb else { continue };
            // Когерентный всплеск: синапсы решают, какие полосы ревут вместе.
            let mut coh = 0.0f64;
            for j in 0..B {
                coh += gpos[b * B + j] * drive[j];
            }
            coh /= gsum[b];
            let gain = 1.0 + burst_gain * coh * coh;
            let z = rng.normal(0.0, 1.0);
            let m = echo.mod_depth[b] as f64;
            let e_db = echo.mean_db[b] as f64
                + echo.dyn_db[b] as f64 * z
                + 6.02 * m * (2.0 * std::f64::consts::PI * echo.mod_hz[b] as f64 * t_s + mod_phase[b]).sin()
                + 10.0 * gain.log10();
            let amp = 10.0f64.powf(e_db / 20.0);
            let fc = band_center(lo, hi, echo.fs);
            let tl = echo.tilt[b] as f64;
            for k in lo..hi {
                let shape = 2.0f64.powf(tl * (k as f64 * bin_hz / fc).log2() / 20.0);
                // ×2: окно анализа Σhann = N/2 — бин амплитуды A даёт
                // измеренную |X[k]| = A/2; делим на nbins НЕЛЬЗЯ —
                // mean_db уже есть энергия НА БИН.
                let mag = 2.0 * amp * shape;
                let ph = 2.0 * std::f64::consts::PI * rng.f64();
                let (s, c) = ph.sin_cos();
                re[k] += mag * c;
                im[k] += mag * s;
            }
        }
        // Эрмитово продолжение + обратный FFT (conjugate-trick).
        for k in 1..STFT_N / 2 {
            re[STFT_N - k] = re[k];
            im[STFT_N - k] = -im[k];
        }
        im[0] = 0.0;
        im[STFT_N / 2] = 0.0;
        for v in im.iter_mut() {
            *v = -*v;
        }
        crate::game::audio::fft_inplace(&mut re, &mut im);
        let base = fi * STFT_HOP;
        for n in 0..STFT_N {
            let x = re[n] / STFT_N as f64;
            acc[base + n] += x * hann[n];
            wsum[base + n] += hann[n];
        }
    }
    for i in 0..acc.len() {
        if wsum[i] > 1e-9 {
            acc[i] /= wsum[i];
        }
    }
    acc
}

/// Генерация звука с нуля из PQW-эха: живой SSN-вихрь рождает тайминг,
/// синапсы — связность, случайные фазы — реализацию. Любая длительность.
pub fn emit_audio(
    pqw: &[u8],
    seconds: f64,
    seed: Option<u64>,
    burst_gain: f64,
) -> Result<(WavData, AudioEmitReport), String> {
    let echo = audio_from_pqw(pqw)?;
    if !(0.5..=600.0).contains(&seconds) {
        return Err("seconds 0.5..=600".into());
    }
    let seed = seed.unwrap_or_else(|| seed_from_bytes(pqw));
    let frames = (seconds * echo.fs as f64 / STFT_HOP as f64).ceil() as usize;
    let mut vortex = crate::ssn::vortex::SynapticVortex::new(
        crate::ssn::vortex::VortexConfig::default(),
        seed,
    );
    let mut bursts = 0usize;
    let mid = {
        let mut rng = crate::ssn::rng::Rng::new(seed ^ 0x9E37_79B9_7F4A_7C15);
        synth_stream(&echo, frames, &mut vortex, &mut rng, burst_gain, &mut bursts)
    };
    let side = {
        let mut rng = crate::ssn::rng::Rng::new(seed ^ 0xC2B2_AE3D_27D4_EB4F);
        synth_stream(&echo, frames, &mut vortex, &mut rng, burst_gain, &mut bursts)
    };
    // Стерео с точной корреляцией ρ: L = a·M + b·S, R = a·M − b·S.
    let rho = echo.stereo_corr as f64;
    let a = ((1.0 + rho) / 2.0).sqrt();
    let b = ((1.0 - rho) / 2.0).sqrt();
    let mut samples = Vec::with_capacity(mid.len() * 2);
    let mut peak = 0.0f64;
    for i in 0..mid.len() {
        let l = a * mid[i] + b * side[i];
        let r = a * mid[i] - b * side[i];
        samples.push(l as f32);
        samples.push(r as f32);
        peak = peak.max(l.abs()).max(r.abs());
    }
    // Нормализация к впитанному уровню с защитой пика.
    let rms = (samples.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / samples.len() as f64).sqrt();
    let target = 10.0f64.powf(echo.level_db as f64 / 20.0);
    let mut scale = if rms > 1e-12 { target / rms } else { 1.0 };
    if scale * peak > 0.98 {
        scale = 0.98 / peak;
    }
    for s in samples.iter_mut() {
        *s = (*s as f64 * scale) as f32;
    }
    let tel = vortex.telemetry();
    let report = AudioEmitReport {
        seed,
        frames,
        vortex_steps: vortex.steps(),
        vortex_activity: tel.activity,
        vortex_criticality: tel.criticality,
        bursts,
    };
    Ok((
        WavData { fs: echo.fs, channels: 2, samples },
        report,
    ))
}

/// PSD-дистанция двух звуков (дБ, среднее по полосам). fs обязан совпадать.
pub fn psd_band_distance(a: &WavData, b: &WavData) -> Result<f64, String> {
    if a.fs != b.fs {
        return Err(format!("fs различаются: {} vs {}", a.fs, b.fs));
    }
    let (ea, _) = stft_band_log_energy(&a.mono(), a.fs);
    let (eb, _) = stft_band_log_energy(&b.mono(), b.fs);
    let mut acc = 0.0;
    let mut n = 0;
    for b_i in 0..B {
        let ma = mean_finite(&ea, b_i);
        let mb = mean_finite(&eb, b_i);
        if let (Some(x), Some(y)) = (ma, mb) {
            acc += (x - y).abs();
            n += 1;
        }
    }
    if n == 0 {
        return Err("нет живых полос для сравнения".into());
    }
    Ok(acc / n as f64)
}

fn mean_finite(rows: &[Vec<f64>], b: usize) -> Option<f64> {
    let mut acc = 0.0;
    let mut n = 0;
    for r in rows {
        if r[b] != f64::NEG_INFINITY {
            acc += r[b];
            n += 1;
        }
    }
    if n == 0 {
        None
    } else {
        Some(acc / n as f64)
    }
}

// ---------------------------------------------------------------------------
// Y-текстура: тайлы → WTA-кодбук → граф смежности → PQW → с нуля
// ---------------------------------------------------------------------------

/// Тайл кодбука.
pub const TILE: usize = 16;
/// Число прототипов (нейронов кодбука).
pub const M_PROTOS: usize = 32;
/// Ранг SVD кодбука.
pub const SVD_RANK: usize = 12;

/// Впитанное эхо текстуры: прототипы + синапсы смежности + статистики.
#[derive(Debug, Clone)]
pub struct TextureEcho {
    /// Прототипы [0, 255], M×(TILE²).
    pub codebook: Vec<Vec<f32>>,
    /// Граф «вправо»: P_r(j|i) — вероятности перехода.
    pub graph_right: Vec<Vec<f32>>,
    /// Граф «вниз»: P_d(j|i).
    pub graph_down: Vec<Vec<f32>>,
    /// Доля использования прототипов [0, 1].
    pub usage: Vec<f32>,
    /// Остаточный разброс кластера [0, 1] (RMS/255).
    pub residual: Vec<f32>,
    pub w: u32,
    pub h: u32,
}

fn tiles_of(img: &GrayImage) -> Vec<Vec<f32>> {
    let cols = img.w as usize / TILE;
    let rows = img.h as usize / TILE;
    let mut out = Vec::with_capacity(cols * rows);
    for r in 0..rows {
        for c in 0..cols {
            let mut t = Vec::with_capacity(TILE * TILE);
            for y in 0..TILE {
                let base = (r * TILE + y) * img.w as usize + c * TILE;
                for x in 0..TILE {
                    t.push(img.gray[base + x] as f32);
                }
            }
            out.push(t);
        }
    }
    out
}

/// Онлайн WTA-кластеризация (нейроны-прототипы): победитель двигается к
/// тайлу скользящим средним, один проход без эпох — как Хебб в кристалле.
fn wta_codebook(tiles: &[Vec<f32>]) -> (Vec<Vec<f32>>, Vec<usize>, Vec<f32>, Vec<f32>) {
    // k-means++-стиль инициализации на подвыборке (детерминированной).
    let stride = (tiles.len() / 256).max(1);
    let cand: Vec<&Vec<f32>> = (0..tiles.len()).step_by(stride).map(|i| &tiles[i]).collect();
    let mut protos: Vec<Vec<f32>> = Vec::with_capacity(M_PROTOS);
    if cand.is_empty() {
        return (
            vec![vec![128.0; TILE * TILE]; M_PROTOS],
            vec![0; tiles.len()],
            vec![1.0 / M_PROTOS as f32; M_PROTOS],
            vec![0.0; M_PROTOS],
        );
    }
    protos.push(cand[0].clone());
    while protos.len() < M_PROTOS {
        let mut best_i = 0usize;
        let mut best_d = -1.0f64;
        for (ci, t) in cand.iter().enumerate() {
            let mut dmin = f64::INFINITY;
            for p in &protos {
                let mut ssd = 0.0;
                for k in 0..TILE * TILE {
                    let d = t[k] as f64 - p[k] as f64;
                    ssd += d * d;
                }
                dmin = dmin.min(ssd);
            }
            if dmin > best_d {
                best_d = dmin;
                best_i = ci;
            }
        }
        protos.push(cand[best_i].clone());
    }
    // Две эпохи онлайн-обучения: присвоение + скользящее среднее победителя.
    let mut assign = vec![0usize; tiles.len()];
    let mut counts = vec![0usize; M_PROTOS];
    for _epoch in 0..2 {
        let mut sums = vec![vec![0.0f64; TILE * TILE]; M_PROTOS];
        let mut cnts = vec![0usize; M_PROTOS];
        for (ti, t) in tiles.iter().enumerate() {
            let mut best = 0usize;
            let mut best_d = f64::INFINITY;
            for (pi, p) in protos.iter().enumerate() {
                let mut ssd = 0.0;
                for k in 0..TILE * TILE {
                    let d = t[k] as f64 - p[k] as f64;
                    ssd += d * d;
                }
                if ssd < best_d {
                    best_d = ssd;
                    best = pi;
                }
            }
            assign[ti] = best;
            cnts[best] += 1;
            for k in 0..TILE * TILE {
                sums[best][k] += t[k] as f64;
            }
        }
        for pi in 0..M_PROTOS {
            if cnts[pi] > 0 {
                for k in 0..TILE * TILE {
                    protos[pi][k] = (sums[pi][k] / cnts[pi] as f64) as f32;
                }
            }
        }
        counts = cnts;
    }
    // Остаточный разброс кластеров.
    let mut residual = vec![0.0f32; M_PROTOS];
    let mut acc = vec![0.0f64; M_PROTOS];
    for (ti, t) in tiles.iter().enumerate() {
        let p = &protos[assign[ti]];
        let mut ssd = 0.0;
        for k in 0..TILE * TILE {
            let d = t[k] as f64 - p[k] as f64;
            ssd += d * d;
        }
        acc[assign[ti]] += ssd;
    }
    let total_px = TILE * TILE;
    for pi in 0..M_PROTOS {
        let n = counts[pi].max(1);
        residual[pi] = ((acc[pi] / n as f64 / total_px as f64).sqrt() / 255.0).min(1.0) as f32;
    }
    let usage: Vec<f32> = counts
        .iter()
        .map(|&c| (c as f32 / tiles.len().max(1) as f32).min(1.0))
        .collect();
    (protos, assign, usage, residual)
}

/// Впитать текстуру: тайлы → WTA-нейроны → графы смежности (Хебб по растру).
pub fn absorb_texture(img: &GrayImage) -> Result<TextureEcho, String> {
    if (img.w as usize) < TILE * 2 || (img.h as usize) < TILE * 2 {
        return Err(format!("изображение меньше {}×{} — нечему учиться", TILE * 2, TILE * 2));
    }
    let tiles = tiles_of(img);
    let (protos, assign, usage, residual) = wta_codebook(&tiles);
    let cols = img.w as usize / TILE;
    let rows = img.h as usize / TILE;
    let idx = |c: usize, r: usize| r * cols + c;
    let mut cr = vec![0u32; M_PROTOS * M_PROTOS];
    let mut cd = vec![0u32; M_PROTOS * M_PROTOS];
    for r in 0..rows {
        for c in 0..cols {
            let a = assign[idx(c, r)];
            if c + 1 < cols {
                let b = assign[idx(c + 1, r)];
                cr[a * M_PROTOS + b] += 1;
            }
            if r + 1 < rows {
                let b = assign[idx(c, r + 1)];
                cd[a * M_PROTOS + b] += 1;
            }
        }
    }
    let norm = |cnt: &[u32]| -> Vec<Vec<f32>> {
        (0..M_PROTOS)
            .map(|i| {
                let row = &cnt[i * M_PROTOS..(i + 1) * M_PROTOS];
                let s: u32 = row.iter().sum();
                if s == 0 {
                    vec![0.0; M_PROTOS]
                } else {
                    row.iter().map(|&v| v as f32 / s as f32).collect()
                }
            })
            .collect()
    };
    Ok(TextureEcho {
        codebook: protos,
        graph_right: norm(&cr),
        graph_down: norm(&cd),
        usage,
        residual,
        w: img.w,
        h: img.h,
    })
}

// --- PQW текстуры ----------------------------------------------------------

/// Индексные базы PQW-текстуры.
fn tex_bases() -> (usize, usize, usize, usize, usize, usize, usize, usize) {
    let u = 0; // U: M*R
    let s = M_PROTOS * SVD_RANK; // σ̂: R
    let s0 = s + SVD_RANK; // σ0: 1
    let v = s0 + 1; // V: 256*R
    let gr = v + 256 * SVD_RANK; // right graph: M*M
    let gd = gr + M_PROTOS * M_PROTOS; // down graph: M*M
    let ug = gd + M_PROTOS * M_PROTOS; // usage: M
    let rs = ug + M_PROTOS; // residual: M
    (u, s, s0, v, gr, gd, ug, rs)
}

fn map_sigma0(v: f64) -> f32 {
    (2.0 * ((v.max(0.05f64) / 0.05f64).ln() / (91.0f64 / 0.05f64).ln()) - 1.0).clamp(-1.0, 1.0) as f32
}
fn unmap_sigma0(p: f64) -> f64 {
    0.05f64 * (91.0f64 / 0.05f64).powf((p + 1.0) / 2.0)
}

/// Сборка PQW-контейнера: кодбук — SVD ранга R (нейроны в базисе
/// сингулярных векторов), графы — разреженные дуги, гипер-параметры
/// несут размеры (eta = −1 — тег «текстура»).
pub fn texture_to_pqw(e: &TextureEcho) -> Result<Vec<u8>, String> {
    use pqw_core::PqwWriter;
    let (u_b, s_b, s0_b, v_b, gr_b, gd_b, ug_b, rs_b) = tex_bases();
    let d_pol = (rs_b + M_PROTOS) as u32;
    let mut w = PqwWriter::new(d_pol).map_err(|e| e.to_string())?
        .hyperparams(-1.0, e.w as f32, e.h as f32, 0.02);
    // SVD кодбука в [-1, 1].
    let mut a = vec![0.0f64; M_PROTOS * 256];
    for i in 0..M_PROTOS {
        for k in 0..256 {
            a[i * 256 + k] = e.codebook[i][k] as f64 / 127.5 - 1.0;
        }
    }
    let (sv_u, sv_s, sv_v) = crate::game::texture::jacobi_svd(&a, M_PROTOS, 256);
    let sigma0 = sv_s[0].max(1e-9);
    let add = |w: &mut PqwWriter, i: u32, p: f32| -> Result<(), String> {
        if !(-1.0..=1.0).contains(&p) || !p.is_finite() {
            return Err(format!("фаза вне [-1,1]: {p}"));
        }
        w.add_phase(i, p).map_err(|e| e.to_string())?;
        Ok(())
    };
    for i in 0..M_PROTOS {
        for r in 0..SVD_RANK {
            add(&mut w, (u_b + i * SVD_RANK + r) as u32, sv_u[i * 256 + r] as f32)?;
        }
    }
    for r in 0..SVD_RANK {
        add(&mut w, (s_b + r) as u32, (sv_s[r] / sigma0) as f32)?;
    }
    add(&mut w, s0_b as u32, map_sigma0(sigma0))?;
    for k in 0..256 {
        for r in 0..SVD_RANK {
            add(&mut w, (v_b + k * SVD_RANK + r) as u32, sv_v[k * 256 + r] as f32)?;
        }
    }
    // Графы: топ-8 дуг на строку (p = 2P−1 ≥ 0).
    for i in 0..M_PROTOS {
        let mut order: Vec<usize> = (0..M_PROTOS).collect();
        let row = &e.graph_right[i];
        order.sort_by(|&x, &y| row[y].partial_cmp(&row[x]).unwrap_or(std::cmp::Ordering::Equal));
        for &j in order.iter().take(8) {
            let p = row[j];
            if p > 0.02 {
                add(&mut w, (gr_b + i * M_PROTOS + j) as u32, p * 2.0 - 1.0)?;
            }
        }
    }
    for i in 0..M_PROTOS {
        let mut order: Vec<usize> = (0..M_PROTOS).collect();
        let row = &e.graph_down[i];
        order.sort_by(|&x, &y| row[y].partial_cmp(&row[x]).unwrap_or(std::cmp::Ordering::Equal));
        for &j in order.iter().take(8) {
            let p = row[j];
            if p > 0.02 {
                add(&mut w, (gd_b + i * M_PROTOS + j) as u32, p * 2.0 - 1.0)?;
            }
        }
    }
    for i in 0..M_PROTOS {
        add(&mut w, (ug_b + i) as u32, e.usage[i] * 2.0 - 1.0)?;
        add(&mut w, (rs_b + i) as u32, e.residual[i] * 2.0 - 1.0)?;
    }
    w.to_bytes().map_err(|e| e.to_string())
}

/// Декодирование текстуры-эха из PQW.
pub fn texture_from_pqw(bytes: &[u8]) -> Result<TextureEcho, String> {
    use pqw_core::PqwReader;
    let r = PqwReader::from_bytes(bytes).map_err(|e| e.to_string())?;
    let hyper = r.hyperparams();
    if hyper.eta >= 0.0 {
        return Err("PQW не текстура-эхо (eta ≥ 0)".into());
    }
    let mut map = std::collections::HashMap::new();
    for (i, p) in r.decoded() {
        map.insert(i, p);
    }
    let (u_b, s_b, s0_b, v_b, gr_b, gd_b, ug_b, rs_b) = tex_bases();
    let sigma0 = unmap_sigma0(map.get(&(s0_b as u32)).copied().unwrap_or(0.0));
    let mut codebook = vec![vec![128.0f32; 256]; M_PROTOS];
    for i in 0..M_PROTOS {
        for k in 0..256 {
            let mut acc = 0.0f64;
            for rr in 0..SVD_RANK {
                let u = map.get(&((u_b + i * SVD_RANK + rr) as u32)).copied().unwrap_or(0.0);
                let s = map.get(&((s_b + rr) as u32)).copied().unwrap_or(0.0) * sigma0;
                let v = map.get(&((v_b + k * SVD_RANK + rr) as u32)).copied().unwrap_or(0.0);
                acc += u * s * v;
            }
            codebook[i][k] = ((acc + 1.0) / 2.0 * 255.0).clamp(0.0, 255.0) as f32;
        }
    }
    let graph = |base: usize| -> Vec<Vec<f32>> {
        (0..M_PROTOS)
            .map(|i| {
                (0..M_PROTOS)
                    .map(|j| {
                        let p = map.get(&((base + i * M_PROTOS + j) as u32)).copied().unwrap_or(-1.0);
                        ((p + 1.0) / 2.0).max(0.0) as f32
                    })
                    .collect()
            })
            .collect()
    };
    let usage = (0..M_PROTOS)
        .map(|i| ((map.get(&((ug_b + i) as u32)).copied().unwrap_or(-1.0) + 1.0) / 2.0) as f32)
        .collect();
    let residual = (0..M_PROTOS)
        .map(|i| ((map.get(&((rs_b + i) as u32)).copied().unwrap_or(-1.0) + 1.0) / 2.0) as f32)
        .collect();
    Ok(TextureEcho {
        codebook,
        graph_right: graph(gr_b),
        graph_down: graph(gd_b),
        usage,
        residual,
        w: hyper.gamma.max(0.0) as u32,
        h: hyper.rho.max(0.0) as u32,
    })
}

/// Отчёт синтеза текстуры.
#[derive(Debug, Clone)]
pub struct TextureEmitReport {
    pub seed: u64,
    pub tiles: (usize, usize),
    pub start_proto: usize,
}

/// Генерация текстуры с нуля: блуждание по графу синапсов + остаточное
/// зерно — бесконечная неповторяющаяся реализация той же статистики.
pub fn emit_texture(
    pqw: &[u8],
    w: u32,
    h: u32,
    seed: Option<u64>,
) -> Result<(GrayImage, TextureEmitReport), String> {
    let e = texture_from_pqw(pqw)?;
    let w = if w == 0 { e.w.max(TILE as u32 * 2) } else { w };
    let h = if h == 0 { e.h.max(TILE as u32 * 2) } else { h };
    if w > 8192 || h > 8192 {
        return Err("размер до 8192".into());
    }
    let seed = seed.unwrap_or_else(|| seed_from_bytes(pqw));
    let mut rng = crate::ssn::rng::Rng::new(seed);
    let cols = (w as usize).div_ceil(TILE);
    let rows = (h as usize).div_ceil(TILE);
    let start = e
        .usage
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);
    let sample_from = |row: &[f32], usage: &[f32], rng: &mut crate::ssn::rng::Rng| -> usize {
        // Кандидаты: дуги графа (p > 0.02), веса p²; пусто — по usage.
        let mut acc = 0.0f64;
        for &p in row.iter() {
            if p > 0.02 {
                acc += (p * p) as f64;
            }
        }
        if acc > 1e-9 {
            let mut x = rng.f64() * acc;
            for (j, &p) in row.iter().enumerate() {
                if p > 0.02 {
                    x -= (p * p) as f64;
                    if x <= 0.0 {
                        return j;
                    }
                }
            }
            return M_PROTOS - 1;
        }
        let mut uacc = 0.0f64;
        for &u in usage.iter() {
            uacc += u as f64;
        }
        if uacc > 1e-9 {
            let mut x = rng.f64() * uacc;
            for (j, &u) in usage.iter().enumerate() {
                x -= u as f64;
                if x <= 0.0 {
                    return j;
                }
            }
        }
        rng.next_u64() as usize % M_PROTOS
    };
    // Столбец стартов (вниз), затем строки (вправо).
    let mut starts = vec![0usize; rows];
    starts[0] = start;
    for y in 1..rows {
        starts[y] = sample_from(&e.graph_down[starts[y - 1]], &e.usage, &mut rng);
    }
    let mut img = vec![0u8; (w as usize) * (h as usize)];
    for y in 0..rows {
        let mut cur = starts[y];
        for x in 0..cols {
            let proto = &e.codebook[cur];
            let resid = e.residual[cur];
            let tseed = seed
                ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
            for ty in 0..TILE {
                let py = y * TILE + ty;
                if py >= h as usize {
                    break;
                }
                for tx in 0..TILE {
                    let px = x * TILE + tx;
                    if px >= w as usize {
                        break;
                    }
                    let noise = crate::game::texture::hash_lattice(px as i64, py as i64, tseed) - 0.5;
                    let v = proto[ty * TILE + tx] + noise as f32 * resid * 255.0 * 2.0;
                    img[py * w as usize + px] = v.clamp(0.0, 255.0) as u8;
                }
            }
            if x + 1 < cols {
                cur = sample_from(&e.graph_right[cur], &e.usage, &mut rng);
            }
        }
    }
    Ok((
        GrayImage { w, h, gray: img },
        TextureEmitReport { seed, tiles: (cols, rows), start_proto: start },
    ))
}

/// Гистограммная дистанция (64 бина, L1/2) ∈ [0, 1].
pub fn histogram_distance(a: &GrayImage, b: &GrayImage) -> f64 {
    let mut ha = [0u64; 64];
    let mut hb = [0u64; 64];
    for &v in &a.gray {
        ha[(v >> 2) as usize] += 1;
    }
    for &v in &b.gray {
        hb[(v >> 2) as usize] += 1;
    }
    let na = a.gray.len() as f64;
    let nb = b.gray.len() as f64;
    let mut d = 0.0;
    for k in 0..64 {
        d += (ha[k] as f64 / na - hb[k] as f64 / nb).abs();
    }
    d / 2.0
}

// ---------------------------------------------------------------------------
// Тесты цикла Y
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("poler_y_{name}_{}", std::process::id()));
        p
    }

    /// Тестовый океан: три аналоговых полосы (однополюсные фильтры) +
    /// каденс обрушений; lead_lag — низкая полоса вспыхивает ЗА 1 кадр
    /// до высокой (для проверки направления STDP).
    fn synth_test_ocean(fs: u32, seconds: f64, seed: u64, lead_lag: bool, stereo: bool) -> WavData {
        let n = (seconds * fs as f64) as usize;
        let mut rng = crate::ssn::rng::Rng::new(seed);
        let mut rng2 = crate::ssn::rng::Rng::new(seed ^ 0xDEAD_BEEF_C0FE_F00D);
        let mut l = vec![0.0f64; n];
        let mut r = vec![0.0f64; n];
        let mut y1 = 0.0;
        let mut y2 = 0.0;
        let mut y3 = 0.0;
        let mut s1 = 0.0;
        let mut s2 = 0.0;
        let mut s3 = 0.0;
        let _frame_rate = fs as f64 / STFT_HOP as f64;
        let period = if lead_lag { 24.0 } else { 40.0 }; // кадров между обрушениями
        for i in 0..n {
            let t = i as f64 / fs as f64;
            let fi = (i / STFT_HOP) as f64;
            let ph = (fi % period) / period;
            // Гауссовы всплески полос (каденс ~0.9 c).
            let (g_low, g_high) = if lead_lag {
                // Носители в полосах 8 (300 Гц) и 18 (3 кГц): низкая
                // полоса вспыхивает [0..8) кадров, высокая — ВПЛОТНУЮ [8..16).
                let lo = if ph < (8.0 / 24.0) { 1.0 } else { 0.0 };
                let hi = if (8.0 / 24.0..16.0 / 24.0).contains(&ph) { 1.0 } else { 0.0 };
                (lo, hi)
            } else {
                let hi = 1.0 + 7.0 * (-((ph - 0.2) / 0.08).powi(2)).exp();
                let mid = 1.0 + 4.0 * (-((ph - 0.35) / 0.1).powi(2)).exp();
                (mid, hi)
            };
            let x = rng.f64() * 2.0 - 1.0;
            let x2 = rng2.f64() * 2.0 - 1.0;
            y1 = 0.995 * y1 + 0.005 * x; // рокот
            y2 = 0.92 * y2 + 0.08 * x; // шум прибоя
            y3 = 0.5 * y3 + 0.5 * x; // спрей
            s1 = 0.995 * s1 + 0.005 * x2;
            s2 = 0.92 * s2 + 0.08 * x2;
            s3 = 0.5 * s3 + 0.5 * x2;
            let (mono, side) = if lead_lag {
                let a = 0.45 * g_low * (2.0 * std::f64::consts::PI * 300.0 * t).sin()
                    + 0.45 * g_high * (2.0 * std::f64::consts::PI * 3000.0 * t).sin()
                    + 0.02 * x;
                let b = 0.45 * g_low * (2.0 * std::f64::consts::PI * 300.0 * t).sin()
                    + 0.45 * g_high * (2.0 * std::f64::consts::PI * 3000.0 * t).sin()
                    + 0.02 * x2;
                (a, b)
            } else {
                (
                    0.6 * y1 + 0.5 * y2 * g_low + 0.35 * y3 * g_high * 0.5,
                    0.6 * s1 + 0.5 * s2 * g_low + 0.35 * s3 * g_high * 0.5,
                )
            };
            l[i] = mono;
            r[i] = if stereo { 0.5 * mono + 0.75f64.sqrt() * side } else { mono };
        }
        let mut peak = 0.0f64;
        for i in 0..n {
            peak = peak.max(l[i].abs()).max(r[i].abs());
        }
        let sc = 0.7 / peak.max(1e-9);
        let mut samples = Vec::with_capacity(n * 2);
        for i in 0..n {
            samples.push((l[i] * sc) as f32);
            samples.push((r[i] * sc) as f32);
        }
        WavData { fs, channels: 2, samples }
    }

    /// Индекс полосы, содержащей частоту (для STDP-теста).
    fn band_of_hz(fs: u32, hz: f64) -> usize {
        let bin = (hz * STFT_N as f64 / fs as f64).round() as usize;
        let ranges = band_ranges(fs);
        for (b, r) in ranges.iter().enumerate() {
            if let Some((lo, hi)) = r {
                if bin >= *lo && bin < *hi {
                    return b;
                }
            }
        }
        B - 1
    }


    #[test]
    fn band_ranges_valid() {
        for fs in [8000u32, 22050, 44100, 48000] {
            let ranges = band_ranges(fs);
            assert_eq!(ranges.len(), B);
            let mut last_hi = 0usize;
            for r in ranges.iter().flatten() {
                assert!(r.0 >= 1 && r.1 <= STFT_N / 2 + 1 && r.0 < r.1);
                assert!(r.0 >= last_hi, "полосы пересекаются при fs={fs}");
                last_hi = r.1;
            }
        }
    }

    #[test]
    fn wav_roundtrip() {
        let mut samples = Vec::new();
        for i in 0..4410 {
            let v = (i as f64 * 0.01).sin() as f32;
            samples.push(v);
            samples.push(-v);
        }
        let path = tmp("wav.wav");
        let n = write_wav(&path, 22050, 2, &samples).unwrap();
        assert_eq!(n, 44 + 4410 * 4);
        let back = read_wav(&path).unwrap();
        assert_eq!(back.fs, 22050);
        assert_eq!(back.channels, 2);
        assert_eq!(back.samples.len(), samples.len());
        for (a, b) in samples.iter().zip(&back.samples) {
            assert!((a - b).abs() < 1.0 / 32767.0 * 1.5);
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn inflate_stored_handcrafted() {
        // Ручной stored-поток: два блока, второй финальный.
        let mut data = vec![0x00u8]; // bfinal=0, btype=00
        data.extend_from_slice(&3u16.to_le_bytes());
        data.extend_from_slice(&(!3u16).to_le_bytes());
        data.extend_from_slice(b"abc");
        data.push(0x01); // bfinal=1, btype=00
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&(!2u16).to_le_bytes());
        data.extend_from_slice(b"de");
        assert_eq!(inflate(&data).unwrap(), b"abcde");
    }

// fixed-Huffman (Z_FIXED), 188 байт -> 45 байт
    const FIXED_STREAM: &[u8] = &[
    0x73, 0x75, 0xF6, 0xF0, 0x57, 0x70, 0x85, 0x13, 0x25, 0x19, 0xA9, 0x0A, 0xF9, 0xC9, 0xA9, 0x89,
    0x79, 0x0A, 0x45, 0xA9, 0xB9, 0xA9, 0xB9, 0x49, 0xA9, 0x45, 0xC5, 0x0A, 0xA9, 0x65, 0xA9, 0x45,
    0x95, 0x0A, 0xE5, 0x89, 0x65, 0xA9, 0x8A, 0x48, 0x2A, 0x07, 0x81, 0x72, 0x00,
    ];
    const FIXED_SRC: &[u8] = &[
    0x45, 0x43, 0x48, 0x4F, 0x20, 0x45, 0x43, 0x48, 0x4F, 0x20, 0x45, 0x43, 0x48, 0x4F, 0x20, 0x74,
    0x68, 0x65, 0x20, 0x6F, 0x63, 0x65, 0x61, 0x6E, 0x20, 0x72, 0x65, 0x6D, 0x65, 0x6D, 0x62, 0x65,
    0x72, 0x73, 0x20, 0x65, 0x76, 0x65, 0x72, 0x79, 0x20, 0x77, 0x61, 0x76, 0x65, 0x21, 0x20, 0x45,
    0x43, 0x48, 0x4F, 0x20, 0x45, 0x43, 0x48, 0x4F, 0x20, 0x45, 0x43, 0x48, 0x4F, 0x20, 0x74, 0x68,
    0x65, 0x20, 0x6F, 0x63, 0x65, 0x61, 0x6E, 0x20, 0x72, 0x65, 0x6D, 0x65, 0x6D, 0x62, 0x65, 0x72,
    0x73, 0x20, 0x65, 0x76, 0x65, 0x72, 0x79, 0x20, 0x77, 0x61, 0x76, 0x65, 0x21, 0x20, 0x45, 0x43,
    0x48, 0x4F, 0x20, 0x45, 0x43, 0x48, 0x4F, 0x20, 0x45, 0x43, 0x48, 0x4F, 0x20, 0x74, 0x68, 0x65,
    0x20, 0x6F, 0x63, 0x65, 0x61, 0x6E, 0x20, 0x72, 0x65, 0x6D, 0x65, 0x6D, 0x62, 0x65, 0x72, 0x73,
    0x20, 0x65, 0x76, 0x65, 0x72, 0x79, 0x20, 0x77, 0x61, 0x76, 0x65, 0x21, 0x20, 0x45, 0x43, 0x48,
    0x4F, 0x20, 0x45, 0x43, 0x48, 0x4F, 0x20, 0x45, 0x43, 0x48, 0x4F, 0x20, 0x74, 0x68, 0x65, 0x20,
    0x6F, 0x63, 0x65, 0x61, 0x6E, 0x20, 0x72, 0x65, 0x6D, 0x65, 0x6D, 0x62, 0x65, 0x72, 0x73, 0x20,
    0x65, 0x76, 0x65, 0x72, 0x79, 0x20, 0x77, 0x61, 0x76, 0x65, 0x21, 0x20,
    ];
// dynamic-Huffman (level 9), 700 байт -> 705 байт
    const DYN_STREAM: &[u8] = &[
    0x01, 0xBC, 0x02, 0x43, 0xFD, 0x00, 0x25, 0x4B, 0x72, 0x99, 0xC1, 0xEA, 0x13, 0x3D, 0x68, 0x93,
    0xBF, 0xEC, 0x19, 0x47, 0x76, 0xA5, 0xD5, 0x06, 0x37, 0x69, 0x9C, 0xCF, 0x03, 0x38, 0x6D, 0xA3,
    0xDA, 0x11, 0x49, 0x82, 0xBB, 0xF5, 0x30, 0x6B, 0xA7, 0xE4, 0x21, 0x5F, 0x9E, 0xDD, 0x1D, 0x5E,
    0x9F, 0xE1, 0x24, 0x67, 0xAB, 0xF0, 0x35, 0x7B, 0xC2, 0x09, 0x51, 0x9A, 0xE3, 0x2D, 0x78, 0xC3,
    0x0F, 0x5C, 0xA9, 0xF7, 0x46, 0x95, 0xE5, 0x36, 0x87, 0xD9, 0x2C, 0x7F, 0xD3, 0x28, 0x7D, 0xD3,
    0x2A, 0x81, 0xD9, 0x32, 0x8B, 0xE5, 0x40, 0x9B, 0xF7, 0x54, 0xB1, 0x0F, 0x6E, 0xCD, 0x2D, 0x8E,
    0xEF, 0x51, 0xB4, 0x17, 0x7B, 0xE0, 0x45, 0xAB, 0x12, 0x79, 0xE1, 0x4A, 0xB3, 0x1D, 0x88, 0xF3,
    0x5F, 0xCC, 0x39, 0xA7, 0x16, 0x85, 0xF5, 0x66, 0xD7, 0x49, 0xBC, 0x2F, 0xA3, 0x18, 0x8D, 0x03,
    0x7A, 0xF1, 0x69, 0xE2, 0x5B, 0xD5, 0x50, 0xCB, 0x47, 0xC4, 0x41, 0xBF, 0x3E, 0xBD, 0x3D, 0xBE,
    0x3F, 0xC1, 0x44, 0xC7, 0x4B, 0xD0, 0x55, 0xDB, 0x62, 0xE9, 0x71, 0xFA, 0x83, 0x0D, 0x98, 0x23,
    0xAF, 0x3C, 0xC9, 0x57, 0xE6, 0x75, 0x05, 0x96, 0x27, 0xB9, 0x4C, 0xDF, 0x73, 0x08, 0x9D, 0x33,
    0xCA, 0x61, 0xF9, 0x92, 0x2B, 0xC5, 0x60, 0xFB, 0x97, 0x34, 0xD1, 0x6F, 0x0E, 0xAD, 0x4D, 0xEE,
    0x8F, 0x31, 0xD4, 0x77, 0x1B, 0xC0, 0x65, 0x0B, 0xB2, 0x59, 0x01, 0xAA, 0x53, 0xFD, 0xA8, 0x53,
    0xFF, 0xAC, 0x59, 0x07, 0xB6, 0x65, 0x15, 0xC6, 0x77, 0x29, 0xDC, 0x8F, 0x43, 0xF8, 0xAD, 0x63,
    0x1A, 0xD1, 0x89, 0x42, 0xFB, 0xB5, 0x70, 0x2B, 0xE7, 0xA4, 0x61, 0x1F, 0xDE, 0x9D, 0x5D, 0x1E,
    0xDF, 0xA1, 0x64, 0x27, 0xEB, 0xB0, 0x75, 0x3B, 0x02, 0xC9, 0x91, 0x5A, 0x23, 0xED, 0xB8, 0x83,
    0x4F, 0x1C, 0xE9, 0xB7, 0x86, 0x55, 0x25, 0xF6, 0xC7, 0x99, 0x6C, 0x3F, 0x13, 0xE8, 0xBD, 0x93,
    0x6A, 0x41, 0x19, 0xF2, 0xCB, 0xA5, 0x80, 0x5B, 0x37, 0x14, 0xF1, 0xCF, 0xAE, 0x8D, 0x6D, 0x4E,
    0x2F, 0x11, 0xF4, 0xD7, 0xBB, 0xA0, 0x85, 0x6B, 0x52, 0x39, 0x21, 0x0A, 0xF3, 0xDD, 0xC8, 0xB3,
    0x9F, 0x8C, 0x79, 0x67, 0x56, 0x45, 0x35, 0x26, 0x17, 0x09, 0xFC, 0xEF, 0xE3, 0xD8, 0xCD, 0xC3,
    0xBA, 0xB1, 0xA9, 0xA2, 0x9B, 0x95, 0x90, 0x8B, 0x87, 0x84, 0x81, 0x7F, 0x7E, 0x7D, 0x7D, 0x7E,
    0x7F, 0x81, 0x84, 0x87, 0x8B, 0x90, 0x95, 0x9B, 0xA2, 0xA9, 0xB1, 0xBA, 0xC3, 0xCD, 0xD8, 0xE3,
    0xEF, 0xFC, 0x09, 0x17, 0x26, 0x35, 0x45, 0x56, 0x67, 0x79, 0x8C, 0x9F, 0xB3, 0xC8, 0xDD, 0xF3,
    0x0A, 0x21, 0x39, 0x52, 0x6B, 0x85, 0xA0, 0xBB, 0xD7, 0xF4, 0x11, 0x2F, 0x4E, 0x6D, 0x8D, 0xAE,
    0xCF, 0xF1, 0x14, 0x37, 0x5B, 0x80, 0xA5, 0xCB, 0xF2, 0x19, 0x41, 0x6A, 0x93, 0xBD, 0xE8, 0x13,
    0x3F, 0x6C, 0x99, 0xC7, 0xF6, 0x25, 0x55, 0x86, 0xB7, 0xE9, 0x1C, 0x4F, 0x83, 0xB8, 0xED, 0x23,
    0x5A, 0x91, 0xC9, 0x02, 0x3B, 0x75, 0xB0, 0xEB, 0x27, 0x64, 0xA1, 0xDF, 0x1E, 0x5D, 0x9D, 0xDE,
    0x1F, 0x61, 0xA4, 0xE7, 0x2B, 0x70, 0xB5, 0xFB, 0x42, 0x89, 0xD1, 0x1A, 0x63, 0xAD, 0xF8, 0x43,
    0x8F, 0xDC, 0x29, 0x77, 0xC6, 0x15, 0x65, 0xB6, 0x07, 0x59, 0xAC, 0xFF, 0x53, 0xA8, 0xFD, 0x53,
    0xAA, 0x01, 0x59, 0xB2, 0x0B, 0x65, 0xC0, 0x1B, 0x77, 0xD4, 0x31, 0x8F, 0xEE, 0x4D, 0xAD, 0x0E,
    0x6F, 0xD1, 0x34, 0x97, 0xFB, 0x60, 0xC5, 0x2B, 0x92, 0xF9, 0x61, 0xCA, 0x33, 0x9D, 0x08, 0x73,
    0xDF, 0x4C, 0xB9, 0x27, 0x96, 0x05, 0x75, 0xE6, 0x57, 0xC9, 0x3C, 0xAF, 0x23, 0x98, 0x0D, 0x83,
    0xFA, 0x71, 0xE9, 0x62, 0xDB, 0x55, 0xD0, 0x4B, 0xC7, 0x44, 0xC1, 0x3F, 0xBE, 0x3D, 0xBD, 0x3E,
    0xBF, 0x41, 0xC4, 0x47, 0xCB, 0x50, 0xD5, 0x5B, 0xE2, 0x69, 0xF1, 0x7A, 0x03, 0x8D, 0x18, 0xA3,
    0x2F, 0xBC, 0x49, 0xD7, 0x66, 0xF5, 0x85, 0x16, 0xA7, 0x39, 0xCC, 0x5F, 0xF3, 0x88, 0x1D, 0xB3,
    0x4A, 0xE1, 0x79, 0x12, 0xAB, 0x45, 0xE0, 0x7B, 0x17, 0xB4, 0x51, 0xEF, 0x8E, 0x2D, 0xCD, 0x6E,
    0x0F, 0xB1, 0x54, 0xF7, 0x9B, 0x40, 0xE5, 0x8B, 0x32, 0xD9, 0x81, 0x2A, 0xD3, 0x7D, 0x28, 0xD3,
    0x7F, 0x2C, 0xD9, 0x87, 0x36, 0xE5, 0x95, 0x46, 0xF7, 0xA9, 0x5C, 0x0F, 0xC3, 0x78, 0x2D, 0xE3,
    0x9A, 0x51, 0x09, 0xC2, 0x7B, 0x35, 0xF0, 0xAB, 0x67, 0x24, 0xE1, 0x9F, 0x5E, 0x1D, 0xDD, 0x9E,
    0x5F, 0x21, 0xE4, 0xA7, 0x6B, 0x30, 0xF5, 0xBB, 0x82, 0x49, 0x11, 0xDA, 0xA3, 0x6D, 0x38, 0x03,
    0xCF, 0x9C, 0x69, 0x37, 0x06, 0xD5, 0xA5, 0x76, 0x47, 0x19, 0xEC, 0xBF, 0x93, 0x68, 0x3D, 0x13,
    0xEA, 0xC1, 0x99, 0x72, 0x4B, 0x25, 0x00, 0xDB, 0xB7, 0x94, 0x71, 0x4F, 0x2E, 0x0D, 0xED, 0xCE,
    0xAF, 0x91, 0x74, 0x57, 0x3B, 0x20, 0x05, 0xEB, 0xD2, 0xB9, 0xA1, 0x8A, 0x73, 0x5D, 0x48, 0x33,
    0x1F, 0x0C, 0xF9, 0xE7, 0xD6, 0xC5, 0xB5, 0xA6, 0x97, 0x89, 0x7C, 0x6F, 0x63, 0x58, 0x4D, 0x43,
    0x3A,
    ];
    const DYN_SRC: &[u8] = &[
    0x00, 0x25, 0x4B, 0x72, 0x99, 0xC1, 0xEA, 0x13, 0x3D, 0x68, 0x93, 0xBF, 0xEC, 0x19, 0x47, 0x76,
    0xA5, 0xD5, 0x06, 0x37, 0x69, 0x9C, 0xCF, 0x03, 0x38, 0x6D, 0xA3, 0xDA, 0x11, 0x49, 0x82, 0xBB,
    0xF5, 0x30, 0x6B, 0xA7, 0xE4, 0x21, 0x5F, 0x9E, 0xDD, 0x1D, 0x5E, 0x9F, 0xE1, 0x24, 0x67, 0xAB,
    0xF0, 0x35, 0x7B, 0xC2, 0x09, 0x51, 0x9A, 0xE3, 0x2D, 0x78, 0xC3, 0x0F, 0x5C, 0xA9, 0xF7, 0x46,
    0x95, 0xE5, 0x36, 0x87, 0xD9, 0x2C, 0x7F, 0xD3, 0x28, 0x7D, 0xD3, 0x2A, 0x81, 0xD9, 0x32, 0x8B,
    0xE5, 0x40, 0x9B, 0xF7, 0x54, 0xB1, 0x0F, 0x6E, 0xCD, 0x2D, 0x8E, 0xEF, 0x51, 0xB4, 0x17, 0x7B,
    0xE0, 0x45, 0xAB, 0x12, 0x79, 0xE1, 0x4A, 0xB3, 0x1D, 0x88, 0xF3, 0x5F, 0xCC, 0x39, 0xA7, 0x16,
    0x85, 0xF5, 0x66, 0xD7, 0x49, 0xBC, 0x2F, 0xA3, 0x18, 0x8D, 0x03, 0x7A, 0xF1, 0x69, 0xE2, 0x5B,
    0xD5, 0x50, 0xCB, 0x47, 0xC4, 0x41, 0xBF, 0x3E, 0xBD, 0x3D, 0xBE, 0x3F, 0xC1, 0x44, 0xC7, 0x4B,
    0xD0, 0x55, 0xDB, 0x62, 0xE9, 0x71, 0xFA, 0x83, 0x0D, 0x98, 0x23, 0xAF, 0x3C, 0xC9, 0x57, 0xE6,
    0x75, 0x05, 0x96, 0x27, 0xB9, 0x4C, 0xDF, 0x73, 0x08, 0x9D, 0x33, 0xCA, 0x61, 0xF9, 0x92, 0x2B,
    0xC5, 0x60, 0xFB, 0x97, 0x34, 0xD1, 0x6F, 0x0E, 0xAD, 0x4D, 0xEE, 0x8F, 0x31, 0xD4, 0x77, 0x1B,
    0xC0, 0x65, 0x0B, 0xB2, 0x59, 0x01, 0xAA, 0x53, 0xFD, 0xA8, 0x53, 0xFF, 0xAC, 0x59, 0x07, 0xB6,
    0x65, 0x15, 0xC6, 0x77, 0x29, 0xDC, 0x8F, 0x43, 0xF8, 0xAD, 0x63, 0x1A, 0xD1, 0x89, 0x42, 0xFB,
    0xB5, 0x70, 0x2B, 0xE7, 0xA4, 0x61, 0x1F, 0xDE, 0x9D, 0x5D, 0x1E, 0xDF, 0xA1, 0x64, 0x27, 0xEB,
    0xB0, 0x75, 0x3B, 0x02, 0xC9, 0x91, 0x5A, 0x23, 0xED, 0xB8, 0x83, 0x4F, 0x1C, 0xE9, 0xB7, 0x86,
    0x55, 0x25, 0xF6, 0xC7, 0x99, 0x6C, 0x3F, 0x13, 0xE8, 0xBD, 0x93, 0x6A, 0x41, 0x19, 0xF2, 0xCB,
    0xA5, 0x80, 0x5B, 0x37, 0x14, 0xF1, 0xCF, 0xAE, 0x8D, 0x6D, 0x4E, 0x2F, 0x11, 0xF4, 0xD7, 0xBB,
    0xA0, 0x85, 0x6B, 0x52, 0x39, 0x21, 0x0A, 0xF3, 0xDD, 0xC8, 0xB3, 0x9F, 0x8C, 0x79, 0x67, 0x56,
    0x45, 0x35, 0x26, 0x17, 0x09, 0xFC, 0xEF, 0xE3, 0xD8, 0xCD, 0xC3, 0xBA, 0xB1, 0xA9, 0xA2, 0x9B,
    0x95, 0x90, 0x8B, 0x87, 0x84, 0x81, 0x7F, 0x7E, 0x7D, 0x7D, 0x7E, 0x7F, 0x81, 0x84, 0x87, 0x8B,
    0x90, 0x95, 0x9B, 0xA2, 0xA9, 0xB1, 0xBA, 0xC3, 0xCD, 0xD8, 0xE3, 0xEF, 0xFC, 0x09, 0x17, 0x26,
    0x35, 0x45, 0x56, 0x67, 0x79, 0x8C, 0x9F, 0xB3, 0xC8, 0xDD, 0xF3, 0x0A, 0x21, 0x39, 0x52, 0x6B,
    0x85, 0xA0, 0xBB, 0xD7, 0xF4, 0x11, 0x2F, 0x4E, 0x6D, 0x8D, 0xAE, 0xCF, 0xF1, 0x14, 0x37, 0x5B,
    0x80, 0xA5, 0xCB, 0xF2, 0x19, 0x41, 0x6A, 0x93, 0xBD, 0xE8, 0x13, 0x3F, 0x6C, 0x99, 0xC7, 0xF6,
    0x25, 0x55, 0x86, 0xB7, 0xE9, 0x1C, 0x4F, 0x83, 0xB8, 0xED, 0x23, 0x5A, 0x91, 0xC9, 0x02, 0x3B,
    0x75, 0xB0, 0xEB, 0x27, 0x64, 0xA1, 0xDF, 0x1E, 0x5D, 0x9D, 0xDE, 0x1F, 0x61, 0xA4, 0xE7, 0x2B,
    0x70, 0xB5, 0xFB, 0x42, 0x89, 0xD1, 0x1A, 0x63, 0xAD, 0xF8, 0x43, 0x8F, 0xDC, 0x29, 0x77, 0xC6,
    0x15, 0x65, 0xB6, 0x07, 0x59, 0xAC, 0xFF, 0x53, 0xA8, 0xFD, 0x53, 0xAA, 0x01, 0x59, 0xB2, 0x0B,
    0x65, 0xC0, 0x1B, 0x77, 0xD4, 0x31, 0x8F, 0xEE, 0x4D, 0xAD, 0x0E, 0x6F, 0xD1, 0x34, 0x97, 0xFB,
    0x60, 0xC5, 0x2B, 0x92, 0xF9, 0x61, 0xCA, 0x33, 0x9D, 0x08, 0x73, 0xDF, 0x4C, 0xB9, 0x27, 0x96,
    0x05, 0x75, 0xE6, 0x57, 0xC9, 0x3C, 0xAF, 0x23, 0x98, 0x0D, 0x83, 0xFA, 0x71, 0xE9, 0x62, 0xDB,
    0x55, 0xD0, 0x4B, 0xC7, 0x44, 0xC1, 0x3F, 0xBE, 0x3D, 0xBD, 0x3E, 0xBF, 0x41, 0xC4, 0x47, 0xCB,
    0x50, 0xD5, 0x5B, 0xE2, 0x69, 0xF1, 0x7A, 0x03, 0x8D, 0x18, 0xA3, 0x2F, 0xBC, 0x49, 0xD7, 0x66,
    0xF5, 0x85, 0x16, 0xA7, 0x39, 0xCC, 0x5F, 0xF3, 0x88, 0x1D, 0xB3, 0x4A, 0xE1, 0x79, 0x12, 0xAB,
    0x45, 0xE0, 0x7B, 0x17, 0xB4, 0x51, 0xEF, 0x8E, 0x2D, 0xCD, 0x6E, 0x0F, 0xB1, 0x54, 0xF7, 0x9B,
    0x40, 0xE5, 0x8B, 0x32, 0xD9, 0x81, 0x2A, 0xD3, 0x7D, 0x28, 0xD3, 0x7F, 0x2C, 0xD9, 0x87, 0x36,
    0xE5, 0x95, 0x46, 0xF7, 0xA9, 0x5C, 0x0F, 0xC3, 0x78, 0x2D, 0xE3, 0x9A, 0x51, 0x09, 0xC2, 0x7B,
    0x35, 0xF0, 0xAB, 0x67, 0x24, 0xE1, 0x9F, 0x5E, 0x1D, 0xDD, 0x9E, 0x5F, 0x21, 0xE4, 0xA7, 0x6B,
    0x30, 0xF5, 0xBB, 0x82, 0x49, 0x11, 0xDA, 0xA3, 0x6D, 0x38, 0x03, 0xCF, 0x9C, 0x69, 0x37, 0x06,
    0xD5, 0xA5, 0x76, 0x47, 0x19, 0xEC, 0xBF, 0x93, 0x68, 0x3D, 0x13, 0xEA, 0xC1, 0x99, 0x72, 0x4B,
    0x25, 0x00, 0xDB, 0xB7, 0x94, 0x71, 0x4F, 0x2E, 0x0D, 0xED, 0xCE, 0xAF, 0x91, 0x74, 0x57, 0x3B,
    0x20, 0x05, 0xEB, 0xD2, 0xB9, 0xA1, 0x8A, 0x73, 0x5D, 0x48, 0x33, 0x1F, 0x0C, 0xF9, 0xE7, 0xD6,
    0xC5, 0xB5, 0xA6, 0x97, 0x89, 0x7C, 0x6F, 0x63, 0x58, 0x4D, 0x43, 0x3A,
    ];
// PNG 64x48 gray, фильтры 0..4, zlib level 6
    const PNG_BYTES: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x28, 0x00, 0x00, 0x00, 0x1E, 0x08, 0x00, 0x00, 0x00, 0x00, 0x7B, 0xB6, 0x03,
    0x01, 0x00, 0x00, 0x02, 0x4B, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x60, 0x60, 0xE1, 0xE0,
    0x11, 0x10, 0x91, 0x90, 0x51, 0x50, 0xD1, 0xD0, 0x31, 0x30, 0xB1, 0xB0, 0x71, 0x70, 0xF1, 0xF0,
    0x09, 0x08, 0x89, 0x88, 0x49, 0x48, 0xC9, 0xC8, 0x29, 0x28, 0xA9, 0xA8, 0x69, 0x68, 0xE9, 0xE8,
    0x99, 0x30, 0x65, 0xC6, 0x1C, 0x46, 0x66, 0x16, 0x30, 0xE0, 0x24, 0x44, 0x31, 0x31, 0x03, 0x01,
    0x07, 0x04, 0xF0, 0x42, 0x80, 0x10, 0x04, 0x88, 0x43, 0x80, 0x0C, 0x04, 0x28, 0x32, 0xB3, 0x81,
    0x74, 0x70, 0xB2, 0x71, 0xB2, 0xB1, 0x71, 0xB3, 0x71, 0x73, 0x72, 0x73, 0x72, 0xF2, 0x71, 0xF2,
    0x71, 0xF3, 0x71, 0x73, 0x0B, 0x70, 0x0B, 0xF0, 0x09, 0xF0, 0xF1, 0x09, 0xF3, 0x09, 0x0B, 0x08,
    0x0B, 0x08, 0x88, 0x0A, 0xB0, 0x00, 0xCD, 0x03, 0x41, 0x0E, 0x90, 0x72, 0x16, 0x4E, 0x4E, 0x9C,
    0x14, 0x03, 0xBF, 0xB0, 0x8C, 0xAA, 0xA6, 0x91, 0xB5, 0x8B, 0x47, 0x60, 0x54, 0x5C, 0x7A, 0x41,
    0x65, 0x6D, 0x5B, 0xFF, 0xE4, 0x39, 0x4B, 0xD7, 0x6D, 0xDA, 0x7D, 0xE4, 0xC4, 0xC5, 0x5B, 0x8F,
    0x9F, 0x7F, 0xF8, 0xF9, 0x97, 0x8D, 0x5F, 0x42, 0x46, 0x55, 0xCF, 0x88, 0x51, 0x08, 0xA8, 0x03,
    0x04, 0x08, 0x51, 0x4C, 0x44, 0xF9, 0x04, 0x08, 0x98, 0xF9, 0xD8, 0xC0, 0x80, 0x1B, 0xA2, 0x17,
    0xE8, 0x0B, 0x10, 0x00, 0xFA, 0x82, 0x8F, 0xAF, 0x0F, 0xE8, 0x0F, 0x30, 0x10, 0x15, 0x06, 0x02,
    0x16, 0xE4, 0xE0, 0xE1, 0xE4, 0xE4, 0x05, 0x29, 0x06, 0x23, 0x4E, 0x71, 0x08, 0x05, 0xE5, 0xF1,
    0x31, 0xC8, 0xA9, 0x1B, 0xD8, 0xB9, 0x87, 0xC6, 0xE5, 0x94, 0xD6, 0xF5, 0x4C, 0x5D, 0xBC, 0x66,
    0xD7, 0xE1, 0x33, 0xB7, 0x1E, 0x7F, 0xFC, 0xC5, 0x21, 0x28, 0xA5, 0x61, 0x68, 0xEF, 0x11, 0x16,
    0x9F, 0x51, 0x56, 0xDF, 0x3B, 0x6D, 0xC9, 0xDA, 0x6D, 0x47, 0xCE, 0x32, 0x2A, 0x82, 0x02, 0x8E,
    0x93, 0x8F, 0x8F, 0x10, 0x05, 0x8A, 0x19, 0x0E, 0x88, 0xDD, 0xBC, 0x10, 0x0F, 0x09, 0x41, 0x3C,
    0x24, 0x0E, 0xF1, 0x90, 0x0C, 0xC4, 0x43, 0x8A, 0x32, 0xCC, 0xA2, 0x6C, 0x9C, 0x9C, 0x20, 0xF7,
    0x03, 0x09, 0x90, 0x0F, 0x80, 0x04, 0xC8, 0xFD, 0x40, 0x02, 0xE4, 0x03, 0x20, 0x21, 0x0A, 0x04,
    0xC2, 0x53, 0x45, 0x25, 0x58, 0xD0, 0x82, 0x87, 0x8F, 0x4F, 0x88, 0x0F, 0x03, 0x28, 0x02, 0x31,
    0x83, 0xAE, 0xB5, 0x67, 0x78, 0x6A, 0x71, 0xE3, 0x94, 0x45, 0x1B, 0xF6, 0x9D, 0xB9, 0xF5, 0xE2,
    0x37, 0xA7, 0xB8, 0xAA, 0xB1, 0xA3, 0x7F, 0x52, 0x41, 0x5D, 0xCF, 0xAC, 0x15, 0xDB, 0x4E, 0x5E,
    0x7F, 0xFA, 0x99, 0x91, 0x5F, 0xD6, 0xC0, 0xCE, 0x27, 0x2A, 0x83, 0xD1, 0x80, 0x0F, 0x14, 0xF3,
    0x20, 0x84, 0x9F, 0x22, 0xDE, 0x33, 0xB2, 0xD0, 0x20, 0x00, 0xA5, 0x2B, 0xEE, 0x09, 0x02, 0xA0,
    0x74, 0xC5, 0x07, 0xF4, 0x85, 0x28, 0x08, 0x8A, 0x0A, 0x4B, 0x80, 0xA0, 0x84, 0xA8, 0x94, 0xA8,
    0x14, 0x0B, 0x8A, 0x89, 0xC2, 0x30, 0x13, 0xF9, 0x84, 0x40, 0xCA, 0xC5, 0x81, 0x18, 0x88, 0x40,
    0x98, 0x8F, 0xC1, 0xC6, 0x2B, 0xB6, 0xA0, 0x79, 0xDA, 0xCA, 0x3D, 0xA7, 0xEE, 0x7E, 0x60, 0x16,
    0xD3, 0xC4, 0xCD, 0x63, 0xB4, 0x17, 0x26, 0x0E, 0x80, 0xF3, 0x0C, 0x31, 0xD9, 0x86, 0x59, 0x05,
    0x92, 0xAA, 0x80, 0xA8, 0x0F, 0x98, 0xAA, 0x84, 0x21, 0xA9, 0x0A, 0x94, 0xB0, 0x24, 0x44, 0x45,
    0x81, 0x48, 0x54, 0x4A, 0x42, 0x02, 0x88, 0x24, 0x64, 0x91, 0x3C, 0x23, 0x81, 0x14, 0x3C, 0xC2,
    0xA0, 0xE0, 0x11, 0x16, 0x17, 0x06, 0x7B, 0x5B, 0x06, 0x44, 0x31, 0x78, 0xC7, 0x95, 0x75, 0x2E,
    0xDC, 0x72, 0xE6, 0xC9, 0x77, 0x7E, 0x25, 0x2B, 0xDF, 0xD4, 0xDA, 0x09, 0x2B, 0x76, 0x5F, 0x7E,
    0xF6, 0x4F, 0x4C, 0xD3, 0x31, 0x24, 0xA7, 0x7E, 0xFA, 0xFA, 0x43, 0xB7, 0xDE, 0xB2, 0x4A, 0x18,
    0x78, 0x44, 0x17, 0xB7, 0xCD, 0x63, 0xF4, 0x03, 0x86, 0x13, 0x10, 0x49, 0x10, 0xA2, 0x88, 0x2F,
    0x00, 0x74, 0x20, 0xF9, 0x42, 0x60, 0x32, 0x24, 0x14, 0x44, 0x21, 0x40, 0x02, 0x02, 0xA4, 0x20,
    0x40, 0x16, 0x08, 0x58, 0x60, 0xC1, 0x23, 0x81, 0xCD, 0x50, 0x09, 0x88, 0xA1, 0x20, 0x3D, 0x00,
    0x53, 0x76, 0xA6, 0xDA, 0xE8, 0xB0, 0xCD, 0x47, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44,
    0xAE, 0x42, 0x60, 0x82,
    ];
    const PNG_GRAY: &[u8] = &[
    0x00, 0x04, 0x08, 0x0C, 0x10, 0x14, 0x18, 0x1C, 0x20, 0x24, 0x28, 0x2C, 0x30, 0x34, 0x38, 0x3C,
    0x40, 0x44, 0x48, 0x4C, 0x50, 0x54, 0x58, 0x5C, 0x60, 0x64, 0x68, 0x6C, 0x70, 0x74, 0x78, 0x7C,
    0x80, 0x84, 0x88, 0x8C, 0x90, 0x94, 0x98, 0x9C, 0x03, 0x07, 0x0B, 0x0F, 0x13, 0x17, 0x1B, 0x24,
    0x28, 0x2C, 0x30, 0x34, 0x38, 0x3C, 0x45, 0x49, 0x4D, 0x51, 0x55, 0x59, 0x5D, 0x66, 0x6A, 0x6E,
    0x72, 0x76, 0x7A, 0x7E, 0x87, 0x8B, 0x8F, 0x93, 0x97, 0x9B, 0x9F, 0xA8, 0xAC, 0xB0, 0xB4, 0xB8,
    0x06, 0x0A, 0x0E, 0x12, 0x1B, 0x1F, 0x23, 0x2C, 0x30, 0x34, 0x38, 0x41, 0x45, 0x49, 0x52, 0x56,
    0x5A, 0x5E, 0x67, 0x6B, 0x6F, 0x78, 0x7C, 0x80, 0x84, 0x8D, 0x91, 0x95, 0x9E, 0xA2, 0xA6, 0xAA,
    0xB3, 0xB7, 0xBB, 0xC4, 0xC8, 0xCC, 0xD0, 0xD9, 0x09, 0x0D, 0x11, 0x1A, 0x1E, 0x27, 0x2B, 0x34,
    0x38, 0x3C, 0x45, 0x49, 0x52, 0x56, 0x5F, 0x63, 0x67, 0x70, 0x74, 0x7D, 0x81, 0x8A, 0x8E, 0x92,
    0x9B, 0x9F, 0xA8, 0xAC, 0xB5, 0xB9, 0xBD, 0xC6, 0xCA, 0xD3, 0xD7, 0xE0, 0xE4, 0xE8, 0xF1, 0xF5,
    0x0C, 0x10, 0x19, 0x1D, 0x26, 0x2A, 0x33, 0x3C, 0x40, 0x49, 0x4D, 0x56, 0x5A, 0x63, 0x6C, 0x70,
    0x79, 0x7D, 0x86, 0x8A, 0x93, 0x9C, 0xA0, 0xA9, 0xAD, 0xB6, 0xBA, 0xC3, 0xCC, 0xD0, 0xD9, 0xDD,
    0xE6, 0xEA, 0xF3, 0xFC, 0x00, 0x09, 0x0D, 0x16, 0x0F, 0x13, 0x1C, 0x25, 0x29, 0x32, 0x3B, 0x44,
    0x48, 0x51, 0x5A, 0x5E, 0x67, 0x70, 0x79, 0x7D, 0x86, 0x8F, 0x93, 0x9C, 0xA5, 0xAE, 0xB2, 0xBB,
    0xC4, 0xC8, 0xD1, 0xDA, 0xE3, 0xE7, 0xF0, 0xF9, 0xFD, 0x06, 0x0F, 0x18, 0x1C, 0x25, 0x2E, 0x32,
    0x12, 0x16, 0x1F, 0x28, 0x31, 0x3A, 0x43, 0x4C, 0x50, 0x59, 0x62, 0x6B, 0x74, 0x7D, 0x86, 0x8A,
    0x93, 0x9C, 0xA5, 0xAE, 0xB7, 0xC0, 0xC4, 0xCD, 0xD6, 0xDF, 0xE8, 0xF1, 0xFA, 0xFE, 0x07, 0x10,
    0x19, 0x22, 0x2B, 0x34, 0x38, 0x41, 0x4A, 0x53, 0x15, 0x1E, 0x27, 0x30, 0x39, 0x42, 0x4B, 0x54,
    0x5D, 0x66, 0x6F, 0x78, 0x81, 0x8A, 0x93, 0x9C, 0xA5, 0xAE, 0xB7, 0xC0, 0xC9, 0xD2, 0xDB, 0xE4,
    0xED, 0xF6, 0xFF, 0x08, 0x11, 0x1A, 0x23, 0x2C, 0x35, 0x3E, 0x47, 0x50, 0x59, 0x62, 0x6B, 0x74,
    0x18, 0x21, 0x2A, 0x33, 0x3C, 0x45, 0x4E, 0x5C, 0x65, 0x6E, 0x77, 0x80, 0x89, 0x92, 0xA0, 0xA9,
    0xB2, 0xBB, 0xC4, 0xCD, 0xD6, 0xE4, 0xED, 0xF6, 0xFF, 0x08, 0x11, 0x1A, 0x28, 0x31, 0x3A, 0x43,
    0x4C, 0x55, 0x5E, 0x6C, 0x75, 0x7E, 0x87, 0x90, 0x1B, 0x24, 0x2D, 0x36, 0x44, 0x4D, 0x56, 0x64,
    0x6D, 0x76, 0x7F, 0x8D, 0x96, 0x9F, 0xAD, 0xB6, 0xBF, 0xC8, 0xD6, 0xDF, 0xE8, 0xF6, 0xFF, 0x08,
    0x11, 0x1F, 0x28, 0x31, 0x3F, 0x48, 0x51, 0x5A, 0x68, 0x71, 0x7A, 0x88, 0x91, 0x9A, 0xA3, 0xB1,
    0x1E, 0x27, 0x30, 0x3E, 0x47, 0x55, 0x5E, 0x6C, 0x75, 0x7E, 0x8C, 0x95, 0xA3, 0xAC, 0xBA, 0xC3,
    0xCC, 0xDA, 0xE3, 0xF1, 0xFA, 0x08, 0x11, 0x1A, 0x28, 0x31, 0x3F, 0x48, 0x56, 0x5F, 0x68, 0x76,
    0x7F, 0x8D, 0x96, 0xA4, 0xAD, 0xB6, 0xC4, 0xCD, 0x21, 0x2A, 0x38, 0x41, 0x4F, 0x58, 0x66, 0x74,
    0x7D, 0x8B, 0x94, 0xA2, 0xAB, 0xB9, 0xC7, 0xD0, 0xDE, 0xE7, 0xF5, 0xFE, 0x0C, 0x1A, 0x23, 0x31,
    0x3A, 0x48, 0x51, 0x5F, 0x6D, 0x76, 0x84, 0x8D, 0x9B, 0xA4, 0xB2, 0xC0, 0xC9, 0xD7, 0xE0, 0xEE,
    0x24, 0x2D, 0x3B, 0x49, 0x52, 0x60, 0x6E, 0x7C, 0x85, 0x93, 0xA1, 0xAA, 0xB8, 0xC6, 0xD4, 0xDD,
    0xEB, 0xF9, 0x02, 0x10, 0x1E, 0x2C, 0x35, 0x43, 0x51, 0x5A, 0x68, 0x76, 0x84, 0x8D, 0x9B, 0xA9,
    0xB2, 0xC0, 0xCE, 0xDC, 0xE5, 0xF3, 0x01, 0x0A, 0x27, 0x30, 0x3E, 0x4C, 0x5A, 0x68, 0x76, 0x84,
    0x8D, 0x9B, 0xA9, 0xB7, 0xC5, 0xD3, 0xE1, 0xEA, 0xF8, 0x06, 0x14, 0x22, 0x30, 0x3E, 0x47, 0x55,
    0x63, 0x71, 0x7F, 0x8D, 0x9B, 0xA4, 0xB2, 0xC0, 0xCE, 0xDC, 0xEA, 0xF8, 0x01, 0x0F, 0x1D, 0x2B,
    0x2A, 0x38, 0x46, 0x54, 0x62, 0x70, 0x7E, 0x8C, 0x9A, 0xA8, 0xB6, 0xC4, 0xD2, 0xE0, 0xEE, 0xFC,
    0x0A, 0x18, 0x26, 0x34, 0x42, 0x50, 0x5E, 0x6C, 0x7A, 0x88, 0x96, 0xA4, 0xB2, 0xC0, 0xCE, 0xDC,
    0xEA, 0xF8, 0x06, 0x14, 0x22, 0x30, 0x3E, 0x4C, 0x2D, 0x3B, 0x49, 0x57, 0x65, 0x73, 0x81, 0x94,
    0xA2, 0xB0, 0xBE, 0xCC, 0xDA, 0xE8, 0xFB, 0x09, 0x17, 0x25, 0x33, 0x41, 0x4F, 0x62, 0x70, 0x7E,
    0x8C, 0x9A, 0xA8, 0xB6, 0xC9, 0xD7, 0xE5, 0xF3, 0x01, 0x0F, 0x1D, 0x30, 0x3E, 0x4C, 0x5A, 0x68,
    0x30, 0x3E, 0x4C, 0x5A, 0x6D, 0x7B, 0x89, 0x9C, 0xAA, 0xB8, 0xC6, 0xD9, 0xE7, 0xF5, 0x08, 0x16,
    0x24, 0x32, 0x45, 0x53, 0x61, 0x74, 0x82, 0x90, 0x9E, 0xB1, 0xBF, 0xCD, 0xE0, 0xEE, 0xFC, 0x0A,
    0x1D, 0x2B, 0x39, 0x4C, 0x5A, 0x68, 0x76, 0x89, 0x33, 0x41, 0x4F, 0x62, 0x70, 0x83, 0x91, 0xA4,
    0xB2, 0xC0, 0xD3, 0xE1, 0xF4, 0x02, 0x15, 0x23, 0x31, 0x44, 0x52, 0x65, 0x73, 0x86, 0x94, 0xA2,
    0xB5, 0xC3, 0xD6, 0xE4, 0xF7, 0x05, 0x13, 0x26, 0x34, 0x47, 0x55, 0x68, 0x76, 0x84, 0x97, 0xA5,
    0x36, 0x44, 0x57, 0x65, 0x78, 0x86, 0x99, 0xAC, 0xBA, 0xCD, 0xDB, 0xEE, 0xFC, 0x0F, 0x22, 0x30,
    0x43, 0x51, 0x64, 0x72, 0x85, 0x98, 0xA6, 0xB9, 0xC7, 0xDA, 0xE8, 0xFB, 0x0E, 0x1C, 0x2F, 0x3D,
    0x50, 0x5E, 0x71, 0x84, 0x92, 0xA5, 0xB3, 0xC6, 0x39, 0x47, 0x5A, 0x6D, 0x7B, 0x8E, 0xA1, 0xB4,
    0xC2, 0xD5, 0xE8, 0xF6, 0x09, 0x1C, 0x2F, 0x3D, 0x50, 0x63, 0x71, 0x84, 0x97, 0xAA, 0xB8, 0xCB,
    0xDE, 0xEC, 0xFF, 0x12, 0x25, 0x33, 0x46, 0x59, 0x67, 0x7A, 0x8D, 0xA0, 0xAE, 0xC1, 0xD4, 0xE2,
    0x3C, 0x4A, 0x5D, 0x70, 0x83, 0x96, 0xA9, 0xBC, 0xCA, 0xDD, 0xF0, 0x03, 0x16, 0x29, 0x3C, 0x4A,
    0x5D, 0x70, 0x83, 0x96, 0xA9, 0xBC, 0xCA, 0xDD, 0xF0, 0x03, 0x16, 0x29, 0x3C, 0x4A, 0x5D, 0x70,
    0x83, 0x96, 0xA9, 0xBC, 0xCA, 0xDD, 0xF0, 0x03, 0x3F, 0x52, 0x65, 0x78, 0x8B, 0x9E, 0xB1, 0xC4,
    0xD7, 0xEA, 0xFD, 0x10, 0x23, 0x36, 0x49, 0x5C, 0x6F, 0x82, 0x95, 0xA8, 0xBB, 0xCE, 0xE1, 0xF4,
    0x07, 0x1A, 0x2D, 0x40, 0x53, 0x66, 0x79, 0x8C, 0x9F, 0xB2, 0xC5, 0xD8, 0xEB, 0xFE, 0x11, 0x24,
    0x42, 0x55, 0x68, 0x7B, 0x8E, 0xA1, 0xB4, 0xCC, 0xDF, 0xF2, 0x05, 0x18, 0x2B, 0x3E, 0x56, 0x69,
    0x7C, 0x8F, 0xA2, 0xB5, 0xC8, 0xE0, 0xF3, 0x06, 0x19, 0x2C, 0x3F, 0x52, 0x6A, 0x7D, 0x90, 0xA3,
    0xB6, 0xC9, 0xDC, 0xF4, 0x07, 0x1A, 0x2D, 0x40, 0x45, 0x58, 0x6B, 0x7E, 0x96, 0xA9, 0xBC, 0xD4,
    0xE7, 0xFA, 0x0D, 0x25, 0x38, 0x4B, 0x63, 0x76, 0x89, 0x9C, 0xB4, 0xC7, 0xDA, 0xF2, 0x05, 0x18,
    0x2B, 0x43, 0x56, 0x69, 0x81, 0x94, 0xA7, 0xBA, 0xD2, 0xE5, 0xF8, 0x10, 0x23, 0x36, 0x49, 0x61,
    0x48, 0x5B, 0x6E, 0x86, 0x99, 0xB1, 0xC4, 0xDC, 0xEF, 0x02, 0x1A, 0x2D, 0x45, 0x58, 0x70, 0x83,
    0x96, 0xAE, 0xC1, 0xD9, 0xEC, 0x04, 0x17, 0x2A, 0x42, 0x55, 0x6D, 0x80, 0x98, 0xAB, 0xBE, 0xD6,
    0xE9, 0x01, 0x14, 0x2C, 0x3F, 0x52, 0x6A, 0x7D, 0x4B, 0x5E, 0x76, 0x89, 0xA1, 0xB4, 0xCC, 0xE4,
    0xF7, 0x0F, 0x22, 0x3A, 0x4D, 0x65, 0x7D, 0x90, 0xA8, 0xBB, 0xD3, 0xE6, 0xFE, 0x16, 0x29, 0x41,
    0x54, 0x6C, 0x7F, 0x97, 0xAF, 0xC2, 0xDA, 0xED, 0x05, 0x18, 0x30, 0x48, 0x5B, 0x73, 0x86, 0x9E,
    0x4E, 0x61, 0x79, 0x91, 0xA4, 0xBC, 0xD4, 0xEC, 0xFF, 0x17, 0x2F, 0x42, 0x5A, 0x72, 0x8A, 0x9D,
    0xB5, 0xCD, 0xE0, 0xF8, 0x10, 0x28, 0x3B, 0x53, 0x6B, 0x7E, 0x96, 0xAE, 0xC6, 0xD9, 0xF1, 0x09,
    0x1C, 0x34, 0x4C, 0x64, 0x77, 0x8F, 0xA7, 0xBA, 0x51, 0x64, 0x7C, 0x94, 0xAC, 0xC4, 0xDC, 0xF4,
    0x07, 0x1F, 0x37, 0x4F, 0x67, 0x7F, 0x97, 0xAA, 0xC2, 0xDA, 0xF2, 0x0A, 0x22, 0x3A, 0x4D, 0x65,
    0x7D, 0x95, 0xAD, 0xC5, 0xDD, 0xF0, 0x08, 0x20, 0x38, 0x50, 0x68, 0x80, 0x93, 0xAB, 0xC3, 0xDB,
    0x54, 0x6C, 0x84, 0x9C, 0xB4, 0xCC, 0xE4, 0xFC, 0x14, 0x2C, 0x44, 0x5C, 0x74, 0x8C, 0xA4, 0xBC,
    0xD4, 0xEC, 0x04, 0x1C, 0x34, 0x4C, 0x64, 0x7C, 0x94, 0xAC, 0xC4, 0xDC, 0xF4, 0x0C, 0x24, 0x3C,
    0x54, 0x6C, 0x84, 0x9C, 0xB4, 0xCC, 0xE4, 0xFC, 0x57, 0x6F, 0x87, 0x9F, 0xB7, 0xCF, 0xE7, 0x04,
    0x1C, 0x34, 0x4C, 0x64, 0x7C, 0x94, 0xB1, 0xC9, 0xE1, 0xF9, 0x11, 0x29, 0x41, 0x5E, 0x76, 0x8E,
    0xA6, 0xBE, 0xD6, 0xEE, 0x0B, 0x23, 0x3B, 0x53, 0x6B, 0x83, 0x9B, 0xB8, 0xD0, 0xE8, 0x00, 0x18,
    ];
    #[test]
    fn inflate_fixed_huffman() {
        let out = inflate(FIXED_STREAM).unwrap();
        assert_eq!(out, FIXED_SRC);
    }

    #[test]
    fn inflate_dynamic_huffman() {
        let out = inflate(DYN_STREAM).unwrap();
        assert_eq!(out, DYN_SRC);
    }

    #[test]
    fn png_roundtrip_own_encoder() {
        let mut gray = vec![0u8; 40 * 30];
        for (i, v) in gray.iter_mut().enumerate() {
            *v = (i * 7 % 251) as u8;
        }
        let path = tmp("own.png");
        crate::p3::png::encode_gray(&path, 40, 30, &gray).unwrap();
        let img = read_image(&path).unwrap();
        assert_eq!((img.w, img.h), (40, 30));
        assert_eq!(img.gray, gray);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn png_deflate_all_filters() {
        let path = tmp("fixture.png");
        std::fs::write(&path, PNG_BYTES).unwrap();
        let img = read_image(&path).unwrap();
        assert_eq!((img.w, img.h), (40, 30));
        assert_eq!(img.gray, PNG_GRAY);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn pnm_read() {
        let path = tmp("t.pgm");
        std::fs::write(&path, b"P5\n# comment\n4 3\n255\n").unwrap();
        std::fs::write(&path, b"P5\n# comment\n4 3\n255\n\x00\x40\x80\xFF\x10\x20\x30\x40\xF0\xE0\xD0\xC0").unwrap();
        let img = read_image(&path).unwrap();
        assert_eq!((img.w, img.h), (4, 3));
        assert_eq!(img.gray, vec![0x00, 0x40, 0x80, 0xFF, 0x10, 0x20, 0x30, 0x40, 0xF0, 0xE0, 0xD0, 0xC0]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn pqw_audio_roundtrip_precision() {
        let wav = synth_test_ocean(22050, 5.0, 42, false, true);
        let echo = absorb_audio(&wav).unwrap();
        let bytes = audio_to_pqw(&echo).unwrap();
        let back = audio_from_pqw(&bytes).unwrap();
        assert_eq!(back.fs, echo.fs);
        assert!((back.stereo_corr - echo.stereo_corr).abs() < 0.02, "стерео {:+.3}", back.stereo_corr - echo.stereo_corr);
        for b in 0..B {
            assert!(
                (back.mean_db[b] - echo.mean_db[b]).abs() < 0.75,
                "mean[{b}]: {:.2} vs {:.2}",
                back.mean_db[b],
                echo.mean_db[b]
            );
            assert!((back.tilt[b] - echo.tilt[b]).abs() < 0.25, "tilt[{b}]");
            let dh = (back.mod_hz[b] - echo.mod_hz[b]).abs();
            assert!(dh < echo.mod_hz[b].max(0.1) * 0.35 + 0.1, "mod_hz[{b}]: {dh}");
            for g in 0..B {
                assert!(
                    (back.graph[b * B + g] - echo.graph[b * B + g]).abs() < 0.03,
                    "graph[{b}][{g}]"
                );
            }
        }
    }

    #[test]
    fn stdp_graph_direction() {
        // Носители: 300 Гц вспыхивает ЗА кадр до 3 кГц — синапс
        // низ→высок должен быть сильнее высок→низ (асимметрия STDP).
        let wav = synth_test_ocean(22050, 6.0, 7, true, false);
        let echo = absorb_audio(&wav).unwrap();
        let low = band_of_hz(22050, 300.0);
        let high = band_of_hz(22050, 3000.0);
        let fwd = echo.graph[low * B + high];
        let rev = echo.graph[high * B + low];
        assert!(
            fwd - rev > 0.15,
            "STDP-асимметрия: G[низ][высок]={fwd:.3} vs G[высок][низ]={rev:.3} (полосы {low}/{high})"
        );
    }

    #[test]
    fn stereo_corr_learned() {
        let wav = synth_test_ocean(22050, 5.0, 42, false, true);
        let echo = absorb_audio(&wav).unwrap();
        assert!(
            (echo.stereo_corr - 0.5).abs() < 0.15,
            "корреляция L/R: {:.3} (ожидалась 0.5)",
            echo.stereo_corr
        );
    }

    #[test]
    fn audio_echo_determinism_and_psd() {
        let wav = synth_test_ocean(22050, 6.0, 42, false, true);
        let path = tmp("ocean.wav");
        write_wav(&path, wav.fs, wav.channels, &wav.samples).unwrap();
        let read_back = read_wav(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let echo = absorb_audio(&read_back).unwrap();
        let pqw = audio_to_pqw(&echo).unwrap();
        assert!(pqw.len() < 4096, "контейнер {} байт", pqw.len());
        let (gen1, rep1) = emit_audio(&pqw, 4.0, None, 2.0).unwrap();
        let (gen2, rep2) = emit_audio(&pqw, 4.0, None, 2.0).unwrap();
        // Детерминизм бит-в-бит: seed из содержимого контейнера.
        assert_eq!(rep1.seed, rep2.seed);
        assert_eq!(gen1.audio_hash(), gen2.audio_hash());
        assert!(rep1.bursts > 0, "вихрь ни разу не дал лавины");
        assert!(rep1.vortex_steps > 100);
        // PSD-дистанция — та же статистика, другая реализация.
        let d = psd_band_distance(&read_back, &gen1).unwrap();
        assert!(d < 6.0, "PSD-дистанция {d:.2} дБ");
        // Дольше входа — океан не кончается.
        let (long, _) = emit_audio(&pqw, 12.0, None, 2.0).unwrap();
        assert!(long.duration_s() > 11.0);
    }

    fn fbm_image(w: u32, h: u32, seed: u64) -> GrayImage {
        let mut gray = vec![0u8; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let v = crate::game::texture::fbm(x as f64 / 11.0, y as f64 / 11.0, seed, 4, 0.55);
                gray[(y * w + x) as usize] = (v * 255.0).clamp(0.0, 255.0) as u8;
            }
        }
        GrayImage { w, h, gray }
    }

    #[test]
    fn texture_echo_stats_and_determinism() {
        let orig = fbm_image(128, 128, 42);
        let echo = absorb_texture(&orig).unwrap();
        let pqw = texture_to_pqw(&echo).unwrap();
        assert!(pqw.len() < 16384, "контейнер {} байт", pqw.len());
        let (gen1, rep) = emit_texture(&pqw, 128, 128, None).unwrap();
        let (gen2, _) = emit_texture(&pqw, 128, 128, None).unwrap();
        assert_eq!(rep.tiles, (8, 8));
        // Детерминизм бит-в-бит.
        assert_eq!(gen1.gray, gen2.gray);
        // Статистика та же: гистограммы близки.
        let hd = histogram_distance(&orig, &gen1);
        assert!(hd < 0.2, "гистограммная дистанция {hd:.3}");
        // PSNR против оригинала — регенерация с нуля, не копия.
        let recon: Vec<f64> = gen1.gray.iter().map(|&v| v as f64).collect();
        let p = crate::game::texture::psnr(&orig.gray, &recon);
        assert!(p > 9.0, "PSNR {p:.2} дБ");
        // Апскейл ×2 — та же статистика, бесконечная текстура.
        let (big, _) = emit_texture(&pqw, 256, 256, None).unwrap();
        assert_eq!((big.w, big.h), (256, 256));
        let hd2 = histogram_distance(&orig, &big);
        assert!(hd2 < 0.25, "гистограммная дистанция апскейла {hd2:.3}");
    }
}
