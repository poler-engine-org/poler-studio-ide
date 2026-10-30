//! Минималистичный PNG-энкодер без внешних зависимостей.
//!
//! Формат: PNG 8-bit, color type 2 (RGB) или 0 (grayscale),
//! сжатие — zlib со stored-блоками deflate (без сжатия: каждый
//! блок — «сырые» байты; это легальный, читаемый любым декодером
//! поток). Для научных артефактов детерминированность и простота
//! важнее размера файла.
//!
//! Теги PNG: сигнатура → IHDR → IDAT (zlib: 0x78 0x01 + блоки +
//! adler32) → IEND; каждый чанк — CRC32.

use std::fs::File;
use std::io::Write;
use std::path::Path;

// -----------------------------------------------------------------------------
// CRC-32 (IEEE 802.3, полином 0xEDB88320) — табличная версия
// -----------------------------------------------------------------------------

fn crc32_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    for (n, slot) in t.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *slot = c;
    }
    t
}

pub fn crc32(data: &[u8]) -> u32 {
    let t = crc32_table();
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = t[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

// -----------------------------------------------------------------------------
// Adler-32 (конец zlib-потока)
// -----------------------------------------------------------------------------

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &byte in chunk {
            a += byte as u32;
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    (b << 16) | a
}

// -----------------------------------------------------------------------------
// Zlib-поток со stored-блоками
// -----------------------------------------------------------------------------

/// zlib = 2-байтовый заголовок + stored-deflate блоки + adler32.
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() + raw.len() / 65535 * 5 + 16);
    out.push(0x78); // CMF: deflate, 32K окно
    out.push(0x01); // FLG: без словаря, минимальный уровень
    let mut chunks = raw.chunks(65535).peekable();
    if raw.is_empty() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]); // один пустой финальный блок
    }
    while let Some(chunk) = chunks.next() {
        let final_block = chunks.peek().is_none();
        let header = if final_block { 0x01 } else { 0x00 };
        out.push(header);
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

// -----------------------------------------------------------------------------
// Чанки PNG
// -----------------------------------------------------------------------------

fn png_chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 12);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(payload);
    let mut crc_input = Vec::with_capacity(4 + payload.len());
    crc_input.extend_from_slice(tag);
    crc_input.extend_from_slice(payload);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    out
}

fn png_stream(width: u32, height: u32, color_type: u8, raw: &[u8]) -> Vec<u8> {
    // raw — развернутые строки пикселей БЕЗ фильтр-байтов
    let channels: usize = if color_type == 2 { 3 } else { 1 };
    let stride = width as usize * channels;
    // Каждая строка получает фильтр-байт 0 (None)
    let mut filtered = Vec::with_capacity((stride + 1) * height as usize);
    for row in raw.chunks(stride) {
        filtered.push(0u8);
        filtered.extend_from_slice(row);
    }

    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // бит на канал
    ihdr.push(color_type);
    ihdr.push(0); // сжатие deflate
    ihdr.push(0); // фильтр adaptive
    ihdr.push(0); // без interlace
    out.extend_from_slice(&png_chunk(b"IHDR", &ihdr));

    out.extend_from_slice(&png_chunk(b"IDAT", &zlib_stored(&filtered)));
    out.extend_from_slice(&png_chunk(b"IEND", &[]));
    out
}

/// Сохранить RGB-изображение (3 байта/пиксель, построчно сверху вниз).
pub fn encode_rgb(path: &Path, width: u32, height: u32, rgb: &[u8]) -> std::io::Result<()> {
    let expected = width as usize * height as usize * 3;
    if rgb.len() != expected {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("rgb buffer: нужно {expected} байт, есть {}", rgb.len()),
        ));
    }
    let bytes = png_stream(width, height, 2, rgb);
    File::create(path)?.write_all(&bytes)
}

/// Сохранить grayscale-изображение (1 байт/пиксель).
pub fn encode_gray(path: &Path, width: u32, height: u32, gray: &[u8]) -> std::io::Result<()> {
    let expected = width as usize * height as usize;
    if gray.len() != expected {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("gray buffer: нужно {expected} байт, есть {}", gray.len()),
        ));
    }
    let bytes = png_stream(width, height, 0, gray);
    File::create(path)?.write_all(&bytes)
}

// -----------------------------------------------------------------------------
// Парсер собственного формата (для round-trip тестов и внешних проверок)
// -----------------------------------------------------------------------------

/// Разобрать PNG обратно в байты пикселей. Поддерживает только то,
/// что пишет этот энкодер (colortype 0/2, stored-deflate). Возвращает
/// (width, height, color_type, развернутые пиксели).
pub fn decode_own(bytes: &[u8]) -> Result<(u32, u32, u8, Vec<u8>), String> {
    let sig = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < 8 || bytes[..8] != sig {
        return Err("не PNG-сигнатура".into());
    }
    let mut pos = 8usize;
    let mut ihdr: Option<(u32, u32, u8)> = None;
    let mut idat: Vec<u8> = Vec::new();

    while pos + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        let tag: [u8; 4] = bytes[pos + 4..pos + 8].try_into().unwrap();
        let data_start = pos + 8;
        let data_end = data_start + len;
        if data_end + 4 > bytes.len() {
            return Err("обрезанный чанк".into());
        }
        let data = &bytes[data_start..data_end];
        // CRC
        let crc_stored = u32::from_be_bytes(bytes[data_end..data_end + 4].try_into().unwrap());
        let mut crc_input = Vec::with_capacity(4 + len);
        crc_input.extend_from_slice(&tag);
        crc_input.extend_from_slice(data);
        if crc32(&crc_input) != crc_stored {
            return Err(format!("CRC не сошёлся в чанке {}", String::from_utf8_lossy(&tag)));
        }
        match &tag {
            b"IHDR" => {
                let w = u32::from_be_bytes(data[0..4].try_into().unwrap());
                let h = u32::from_be_bytes(data[4..8].try_into().unwrap());
                let ct = data[9];
                ihdr = Some((w, h, ct));
            }
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        pos = data_end + 4;
    }

    let (w, h, ct) = ihdr.ok_or("нет IHDR")?;
    if idat.len() < 6 {
        return Err("нет IDAT".into());
    }
    // zlib-заголовок
    if idat[0] != 0x78 {
        return Err("не zlib-заголовок".into());
    }
    // stored-deflate распаковка
    let mut raw = Vec::new();
    let mut p = 2usize;
    loop {
        if p >= idat.len() - 4 {
            return Err("deflate: нет финального блока".into());
        }
        let header = idat[p];
        p += 1;
        let len = u16::from_le_bytes([idat[p], idat[p + 1]]) as usize;
        let nlen = u16::from_le_bytes([idat[p + 2], idat[p + 3]]) as usize;
        if len != (!nlen & 0xFFFF) {
            return Err("deflate: NLEN не инвертирует LEN".into());
        }
        p += 4;
        if p + len > idat.len() - 4 {
            return Err("deflate: блок обрезан".into());
        }
        raw.extend_from_slice(&idat[p..p + len]);
        p += len;
        if header & 0x01 != 0 {
            break;
        }
    }
    // adler
    let adler_stored = u32::from_be_bytes(
        idat[idat.len() - 4..].try_into().unwrap(),
    );
    if adler32(&raw) != adler_stored {
        return Err("adler32 не сошёлся".into());
    }

    // Убираем фильтр-байты
    let channels: usize = if ct == 2 { 3 } else { 1 };
    let stride = w as usize * channels;
    let mut px = Vec::with_capacity(stride * h as usize);
    let mut q = 0usize;
    for _ in 0..h {
        if q >= raw.len() || raw[q] != 0 {
            return Err("неожиданный фильтр-байт".into());
        }
        q += 1;
        if q + stride > raw.len() {
            return Err("строка обрезана".into());
        }
        px.extend_from_slice(&raw[q..q + stride]);
        q += stride;
    }
    Ok((w, h, ct, px))
}

// =============================================================================
// ТЕСТЫ
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_vectors() {
        // Эталонные значения CRC-32/ISO-HDLC
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
    }

    #[test]
    fn adler32_known_vector() {
        // "Wikipedia" → 0x11E60398
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(b""), 1);
    }

    #[test]
    fn png_rgb_roundtrip() {
        let (w, h) = (13u32, 7u32);
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for i in 0..(w * h) as usize {
            rgb.push((i % 256) as u8);
            rgb.push((i * 7 % 256) as u8);
            rgb.push((i * 13 % 256) as u8);
        }
        let bytes = png_stream(w, h, 2, &rgb);
        let (w2, h2, ct, px) = decode_own(&bytes).expect("decode");
        assert_eq!((w2, h2, ct), (w, h, 2));
        assert_eq!(px, rgb, "пиксели должны совпасть байт-в-байт");
    }

    #[test]
    fn png_gray_roundtrip_and_determinism() {
        let (w, h) = (64u32, 32u32);
        let gray: Vec<u8> = (0..w * h).map(|i| (i * 3 % 256) as u8).collect();
        let b1 = png_stream(w, h, 0, &gray);
        let b2 = png_stream(w, h, 0, &gray);
        assert_eq!(b1, b2, "энкодер детерминирован");
        let (_, _, ct, px) = decode_own(&b1).unwrap();
        assert_eq!(ct, 0);
        assert_eq!(px, gray);
    }

    #[test]
    fn png_rejects_corrupted_crc() {
        let bytes = png_stream(4, 4, 0, &[7u8; 16]);
        let mut bad = bytes.clone();
        let last = bad.len() - 1;
        bad[last] ^= 0xFF;
        assert!(decode_own(&bad).is_err(), "битый CRC должен ловиться");
    }

    #[test]
    fn png_encode_to_file() {
        let dir = std::env::temp_dir().join("poler_p3_png_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.png");
        encode_rgb(&path, 5, 4, &[200u8; 60]).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));
        let (_, _, _, px) = decode_own(&bytes).unwrap();
        assert_eq!(px, vec![200u8; 60]);
    }
}
