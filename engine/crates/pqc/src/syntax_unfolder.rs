//! syntax_unfolder.rs — Детерминированный AOT-кристаллизатор прямого декодирования фазового аттрактора p*.
//!
//! Реализует безветвистую (branchless, zero-alloc) развертку фазового вектора в символьный поток
//! на основе логики метакомпилятора POLER-ERI (VRR/HRR AOT unrolling).

use crate::error::{PqcError, Result};

include!(concat!(env!("OUT_DIR"), "/morpheme_crystals.rs"));

/// Порог квантования тритов по умолчанию
pub const TRIT_THRESHOLD: f64 = 1.0 / 3.0;

/// Максимальный размер выходного буфера на блок
pub const MAX_OUT: usize = 128;

/// Проверка стационарности фазового вектора (‖dp/dt‖ < eps)
#[inline(always)]
pub fn is_stationary(p: &[f64], prev: &[f64], eps: f64) -> bool {
    p.iter().zip(prev).all(|(a, b)| (a - b).abs() < eps)
}

/// Прямая безветвистая развертка фазового вектора p в фиксированный стековый буфер
///
/// Гарантии:
/// - Ноль аллокаций в Heap
/// - Ноль паник в Release
/// - Детерминированная побайтовая воспроизводимость (идемпотентность)
#[inline(always)]
pub fn unfurl(p_state: &[f64], out_buf: &mut [u8; MAX_OUT]) -> usize {
    let mut out_len = 0;

    // Обработка четверками (K=4 трита -> 8 бит -> индекс 0..255)
    let chunks = p_state.chunks_exact(4);
    let remainder = chunks.remainder();

    for quad in chunks {
        let t0 =
            ((quad[0] > TRIT_THRESHOLD) as usize) | (((quad[0] < -TRIT_THRESHOLD) as usize) << 1);
        let t1 =
            ((quad[1] > TRIT_THRESHOLD) as usize) | (((quad[1] < -TRIT_THRESHOLD) as usize) << 1);
        let t2 =
            ((quad[2] > TRIT_THRESHOLD) as usize) | (((quad[2] < -TRIT_THRESHOLD) as usize) << 1);
        let t3 =
            ((quad[3] > TRIT_THRESHOLD) as usize) | (((quad[3] < -TRIT_THRESHOLD) as usize) << 1);

        let idx = (t0 & 0x03) | ((t1 & 0x03) << 2) | ((t2 & 0x03) << 4) | ((t3 & 0x03) << 6);
        let raw = MORPHEME_CRYSTALS[idx];

        if raw != 0 && out_len + 4 <= MAX_OUT {
            let bytes = raw.to_le_bytes();
            let len = 4 - (raw.leading_zeros() >> 3) as usize;

            let to_write = len.min(MAX_OUT - out_len);
            out_buf[out_len..out_len + to_write].copy_from_slice(&bytes[..to_write]);
            out_len += to_write;
        }
    }

    // Обработка остатка
    if !remainder.is_empty() && out_len + 4 <= MAX_OUT {
        let mut idx = 0usize;
        for (i, &val) in remainder.iter().enumerate() {
            let t = ((val > TRIT_THRESHOLD) as usize) | (((val < -TRIT_THRESHOLD) as usize) << 1);
            idx |= (t & 0x03) << (i * 2);
        }
        let raw = MORPHEME_CRYSTALS[idx];
        if raw != 0 {
            let bytes = raw.to_le_bytes();
            let len = 4 - (raw.leading_zeros() >> 3) as usize;
            let to_write = len.min(MAX_OUT - out_len);
            out_buf[out_len..out_len + to_write].copy_from_slice(&bytes[..to_write]);
            out_len += to_write;
        }
    }

    out_len
}

/// Удобная обертка для развертки в String
pub fn unfurl_to_string(p_state: &[f64]) -> Result<String> {
    let mut buf = [0u8; MAX_OUT];
    let len = unfurl(p_state, &mut buf);
    std::str::from_utf8(&buf[..len])
        .map(|s| s.to_string())
        .map_err(|_| PqcError::BadPhase(0.0))
}

/// Морфема AOT-кристалла по индексу `0..256` (детерминированный
/// фолбэк L5-генерации: координаты вне лексикона говорят синтаксисом).
///
/// Слот 0 пуст (нет кристалла) — `None`. Сырой u32 хранит ≤ 4 ASCII
/// байтов младшими разрядами вперёд.
pub fn morpheme_at(index: usize) -> Option<String> {
    let raw = *MORPHEME_CRYSTALS.get(index)?;
    if raw == 0 {
        return None;
    }
    let bytes = raw.to_le_bytes();
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(4);
    std::str::from_utf8(&bytes[..len])
        .ok()
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unfurl_deterministic_idempotence() {
        let p_state = [0.85, -0.72, 0.05, 0.91, -0.88, 0.12, -0.04, 0.77];
        let mut buf1 = [0u8; MAX_OUT];
        let mut buf2 = [0u8; MAX_OUT];

        let len1 = unfurl(&p_state, &mut buf1);
        let len2 = unfurl(&p_state, &mut buf2);

        assert_eq!(len1, len2);
        assert_eq!(&buf1[..len1], &buf2[..len2]);

        let s = std::str::from_utf8(&buf1[..len1]).unwrap();
        println!("Unfurled text: '{}' (len={})", s, len1);
    }

    #[test]
    fn test_zero_state_is_empty() {
        let p_zero = [0.0; 16];
        let mut buf = [0u8; MAX_OUT];
        let len = unfurl(&p_zero, &mut buf);
        assert_eq!(len, 0);
    }
}
