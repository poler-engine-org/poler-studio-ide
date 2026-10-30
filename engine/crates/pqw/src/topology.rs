//! Sparse LENS-топология: индексы ненулевых фазовых дуг.
//!
//! LENS-сжатие: фоновые дуги `|p| < ε` не хранятся вовсе — ни байта на дугу.
//! Хранимые индексы строго возрастают; ширина — u16 при `d_pol ≤ 65536`,
//! иначе u32.

use std::borrow::Cow;

use crate::error::{PqwError, Result};

/// Ширина индекса топологии в байтах.
#[inline]
pub fn index_width(index16: bool) -> usize {
    if index16 {
        2
    } else {
        4
    }
}

/// u32 → LE-байты. В u16-режиме индексы обязаны быть ≤ 65535
/// (гарантируется условием `d_pol ≤ 65536`).
pub fn encode_indices(indices: &[u32], index16: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(indices.len() * index_width(index16));
    for &i in indices {
        debug_assert!(!index16 || i <= u16::MAX as u32);
        if index16 {
            out.extend_from_slice(&(i as u16).to_le_bytes());
        } else {
            out.extend_from_slice(&i.to_le_bytes());
        }
    }
    out
}

/// LE-байты → u32. В 32-битном режиме на little-endian хосте — zero-copy
/// (заимствование без копирования при допустимом выравнивании).
pub fn decode_indices(bytes: &[u8], index16: bool) -> Result<Cow<'_, [u32]>> {
    let width = index_width(index16);
    if !bytes.len().is_multiple_of(width) {
        return Err(PqwError::Layout(
            "topology length is not a multiple of the index width",
        ));
    }
    if index16 {
        let v: Vec<u32> = bytes
            .as_chunks::<2>().0.iter()
            .map(|c| u32::from(u16::from_le_bytes([c[0], c[1]])))
            .collect();
        Ok(Cow::Owned(v))
    } else if cfg!(target_endian = "little")
        && (bytes.as_ptr() as usize).is_multiple_of(std::mem::align_of::<u32>())
    {
        let n = bytes.len() / 4;
        // SAFETY: длина кратна 4, выравнивание проверено выше; u32 — POD.
        let s = unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u32, n) };
        Ok(Cow::Borrowed(s))
    } else {
        let v: Vec<u32> = bytes
            .as_chunks::<4>().0.iter()
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        Ok(Cow::Owned(v))
    }
}

/// Строго возрастающие индексы внутри `[0, d_pol)`.
pub fn validate_indices(indices: &[u32], d_pol: u32) -> Result<()> {
    let mut prev: Option<u32> = None;
    for &i in indices {
        if i >= d_pol {
            return Err(PqwError::BadIndex { index: i, d_pol });
        }
        if let Some(p) = prev {
            if i <= p {
                return Err(PqwError::UnsortedTopology);
            }
        }
        prev = Some(i);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_u16() {
        let idx = [1u32, 3, 5, 60000];
        let b = encode_indices(&idx, true);
        assert_eq!(b.len(), 8);
        let d = decode_indices(&b, true).unwrap();
        assert_eq!(&d[..], &idx[..]);
    }

    #[test]
    fn roundtrip_u32() {
        let idx = [1u32, 70000, 3_000_000_000];
        let b = encode_indices(&idx, false);
        assert_eq!(b.len(), 12);
        let d = decode_indices(&b, false).unwrap();
        assert_eq!(&d[..], &idx[..]);
    }

    #[test]
    fn decode_rejects_bad_length() {
        assert!(decode_indices(&[1, 2, 3], true).is_err());
        assert!(decode_indices(&[1, 2, 3, 4, 5], false).is_err());
    }

    #[test]
    fn validate_rejects_bad_indices() {
        assert!(validate_indices(&[0, 2, 2, 3], 8).is_err()); // дубликат
        assert!(validate_indices(&[3, 1], 8).is_err()); // не отсортированы
        assert!(validate_indices(&[0, 8], 8).is_err()); // вне диапазона
        assert!(validate_indices(&[], 8).is_ok());
        assert!(validate_indices(&[0, 7], 8).is_ok());
    }
}
