//! WAV 16-bit PCM mono — запись и чтение (без зависимостей).

use crate::{ReaderError, Result, FS};

/// Заголовок RIFF/WAVE для моно 16 бит.
fn header(n_samples: usize, fs: u32) -> Vec<u8> {
    let data_len = n_samples * 2;
    let mut h = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&u32::to_le_bytes((36 + data_len) as u32));
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&u32::to_le_bytes(16));
    h.extend_from_slice(&u16::to_le_bytes(1)); // PCM
    h.extend_from_slice(&u16::to_le_bytes(1)); // mono
    h.extend_from_slice(&u32::to_le_bytes(fs));
    h.extend_from_slice(&u32::to_le_bytes(fs * 2)); // byte rate
    h.extend_from_slice(&u16::to_le_bytes(2)); // block align
    h.extend_from_slice(&u16::to_le_bytes(16)); // bits
    h.extend_from_slice(b"data");
    h.extend_from_slice(&u32::to_le_bytes(data_len as u32));
    h
}

/// Записать сэмплы [-1, 1] в WAV-файл.
pub fn save_wav(path: &std::path::Path, samples: &[f64], fs: u32) -> Result<()> {
    let mut buf = header(samples.len(), fs);
    buf.reserve(samples.len() * 2);
    for &s in samples {
        let v = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
        buf.extend_from_slice(&i16::to_le_bytes(v));
    }
    std::fs::write(path, buf)?;
    Ok(())
}

/// Прочитать WAV (моно 16 бит; стерео усредняется). Возвращает (сэмплы, fs).
pub fn load_wav(path: &std::path::Path) -> Result<(Vec<f64>, u32)> {
    let data = std::fs::read(path).map_err(ReaderError::Io)?;
    if data.len() < 44 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err(ReaderError::BadInput("не WAV-файл".into()));
    }
    // поиск чанка fmt и data (в общем порядке)
    let mut pos = 12;
    let mut fs = FS;
    let mut channels = 1u16;
    let mut bits = 16u16;
    let mut samples: Vec<f64> = Vec::new();
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let len = u32::from_le_bytes([
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]) as usize;
        let body = pos + 8;
        if id == b"fmt " && len >= 16 {
            channels = u16::from_le_bytes([data[body + 2], data[body + 3]]);
            fs = u32::from_le_bytes([
                data[body + 4],
                data[body + 5],
                data[body + 6],
                data[body + 7],
            ]);
            bits = u16::from_le_bytes([data[body + 14], data[body + 15]]);
        } else if id == b"data" {
            let end = (body + len).min(data.len());
            let raw = &data[body..end];
            if bits == 16 {
                for chunk in raw.chunks(2 * channels as usize) {
                    let mut acc = 0.0;
                    let mut n = 0.0;
                    for c in chunk.chunks(2) {
                        if c.len() == 2 {
                            let v = i16::from_le_bytes([c[0], c[1]]);
                            acc += v as f64 / 32768.0;
                            n += 1.0;
                        }
                    }
                    if n > 0.0 {
                        samples.push(acc / n);
                    }
                }
            } else {
                return Err(ReaderError::BadInput(format!(
                    "поддерживается только 16-бит PCM (найдено {bits})"
                )));
            }
        }
        pos = body + len + (len & 1); // выравнивание чанков на чёт
    }
    Ok((samples, fs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_wav() {
        let dir = std::env::temp_dir().join("poler_reader_wav");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("t.wav");
        let src: Vec<f64> = (0..1000)
            .map(|i| ((i as f64) * 0.05).sin() * 0.5)
            .collect();
        save_wav(&p, &src, FS).unwrap();
        let (back, fs) = load_wav(&p).unwrap();
        assert_eq!(fs, FS);
        assert_eq!(back.len(), 1000);
        for (a, b) in src.iter().zip(back.iter()) {
            assert!((a - b).abs() < 2.0 / 32768.0 + 1e-9);
        }
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn header_shape() {
        let h = header(10, 22_050);
        assert_eq!(&h[0..4], b"RIFF");
        assert_eq!(&h[8..12], b"WAVE");
        assert_eq!(&h[36..40], b"data");
        assert_eq!(h.len(), 44);
        let riff_len = u32::from_le_bytes([h[4], h[5], h[6], h[7]]);
        assert_eq!(riff_len, 36 + 20);
    }
}
