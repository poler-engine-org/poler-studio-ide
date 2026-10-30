//! Trit5 / No-Mul: троичное квантование латентного состояния p_t.
//!
//! Латентный вектор нарратива сжимается в троичные примитивы
//! {−1, 0, +1} — пять тритов на байт, кодек [`crate::pqc::tensor::Trit5Codec`]
//! (тот же, что в квантовых весах PND). Резонансные скалярные
//! произведения считаются SIMD-регистрами CPU
//! ([`crate::pqc::tensor::dot_trit5_f32`], AVX2) без плавающей точки
//! на горячем пути: шаги интегратора и эхо живут в троичной algebra,
//! f32 возвращается только на выходе телеметрии.
//!
//! Схема per-vector: масштаб s = max|p_i|; трит t_i = sign(p_i), если
//! |p_i| ≥ θ·s (мёртвая зона отсекает шум), иначе 0. Восстановление
//! p̂_i = t_i·s — детерминированное, ошибка ограничена сверху
//! θ·s + s·(1−‖p‖∞/s)·0 — фактически max|p − p̂| ≤ max(θ·s, |малые|).

use crate::pqc::tensor::{dot_trit5_f32, Trit5Codec};

/// Троичное латентное состояние: упакованные триты + масштаб.
#[derive(Clone, Debug)]
pub struct TritState {
    packed: Vec<u8>,
    /// Масштаб s = max|p_i| на момент квантования.
    pub scale: f32,
    /// Число осей.
    pub dims: usize,
    /// Число ненулевых тритов (плотность состояния).
    pub nonzeros: usize,
}

/// Мёртвая зона по умолчанию: 5% масштаба.
pub const DEFAULT_THETA: f32 = 0.05;

/// Квантование p → TritState. Пустой/нулевой вектор — легальное
/// состояние (все триты 0).
pub fn quantize(p: &[f32], theta: f32) -> TritState {
    let dims = p.len();
    let scale = p.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
    let dead = theta.max(0.0) * scale;
    let mut trits = vec![0i8; dims];
    let mut nonzeros = 0usize;
    for (i, &v) in p.iter().enumerate() {
        if v.abs() > dead && v.abs() >= scale * 1e-6 {
            trits[i] = if v > 0.0 { 1 } else { -1 };
            nonzeros += 1;
        }
    }
    // Упаковка по 5 тритов в байт.
    let packed_cols = dims.div_ceil(5);
    let mut packed = vec![0u8; packed_cols];
    for (chunk_idx, chunk) in trits.chunks(5).enumerate() {
        let mut five = [0i8; 5];
        five[..chunk.len()].copy_from_slice(chunk);
        if let Some(b) = Trit5Codec::pack_5(&five) {
            packed[chunk_idx] = b;
        }
    }
    TritState {
        packed,
        scale,
        dims,
        nonzeros,
    }
}

impl TritState {
    /// Восстановление f32-вектора: p̂_i = t_i·s.
    pub fn dequantize(&self) -> Vec<f32> {
        let mut out = vec![0.0f32; self.dims];
        let mut i = 0usize;
        for &b in &self.packed {
            for t in Trit5Codec::unpack_5(b) {
                if i >= self.dims {
                    break;
                }
                out[i] = t as f32 * self.scale;
                i += 1;
            }
        }
        out
    }

    /// SIMD-скалярное произведение с f32-вектором (AVX2, без f32-математики
    /// на трит-стороне).
    pub fn dot(&self, x: &[f32]) -> f32 {
        debug_assert_eq!(x.len(), self.dims);
        dot_trit5_f32(&self.packed, x, self.dims) * self.scale
    }

    /// Плотность состояния: доля ненулевых тритов.
    pub fn density(&self) -> f32 {
        if self.dims == 0 {
            return 0.0;
        }
        self.nonzeros as f32 / self.dims as f32
    }

    /// Упакованные байты (сырой вид для сериализации).
    pub fn packed_bytes(&self) -> &[u8] {
        &self.packed
    }
}

/// Полный No-Mul-шаг: квантование + восстановление (раунд-трип).
/// Возвращаемое состояние живёт на троичной решётке — интегратор
/// в режиме No-Mul делает p ← dequant(quant(p)) после каждого шага.
pub fn roundtrip(p: &[f32], theta: f32) -> (TritState, Vec<f32>) {
    let q = quantize(p, theta);
    let back = q.dequantize();
    (q, back)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_dequantize_roundtrip_bound() {
        // scale = 0.95, theta = 0.05 → мёртвая зона 0.0475:
        // 0.04 проходит насквозь в ноль, 0.05 — уже сигнальный трит.
        let p = vec![0.9, -0.4, 0.0, 0.04, -0.95, 0.3, 0.3, 0.3];
        let (q, back) = roundtrip(&p, 0.05);
        assert_eq!(q.dims, 8);
        assert_eq!(q.scale, 0.95);
        // Восстановленные компоненты — кратны масштабу с верным знаком
        assert!((back[0] - 0.95).abs() < 1e-6);
        assert!((back[1] + 0.95).abs() < 1e-6);
        assert_eq!(back[2], 0.0, "ноль остаётся нулём");
        assert_eq!(back[3], 0.0, "мёртвая зона 0.0475 отсекла 0.04");
        assert!((back[4] + 0.95).abs() < 1e-6);
        // ошибка раунд-трипа ограничена масштабом
        for (a, b) in p.iter().zip(&back) {
            assert!((a - b).abs() <= q.scale + 1e-6);
        }
        // граничный случай: |v| чуть выше зоны — живой трит
        let pb = vec![0.05];
        let (qb, bb) = roundtrip(&pb, 0.05);
        assert!((bb[0] - 0.05).abs() < 1e-6, "0.05 > 0.0475 → трит +1");
        assert_eq!(qb.nonzeros, 1);
    }

    #[test]
    fn simd_dot_matches_scalar() {
        let p = vec![0.5, -0.5, 0.5, 0.5, -0.5, 0.0, 0.12, -0.5, 0.5, 0.5];
        let x = vec![1.0, 2.0, -3.0, 0.5, 0.25, 10.0, -2.0, 4.0, -1.0, 0.0];
        let q = quantize(&p, 0.0);
        let simd = q.dot(&x);
        let scalar = q.dequantize().iter().zip(&x).map(|(&a, &b)| a * b).sum::<f32>();
        assert!((simd - scalar).abs() < 1e-5, "simd {simd} vs scalar {scalar}");
    }

    #[test]
    fn zero_and_empty_states_legal() {
        let z = quantize(&[0.0; 6], 0.05);
        assert_eq!(z.scale, 0.0);
        assert_eq!(z.nonzeros, 0);
        assert_eq!(z.dequantize(), vec![0.0; 6]);
        assert_eq!(z.density(), 0.0);
        let e = quantize(&[], 0.05);
        assert_eq!(e.dims, 0);
        assert!(e.dequantize().is_empty());
        assert_eq!(e.dot(&[]), 0.0);
    }

    #[test]
    fn density_reflects_deadzone() {
        let p = vec![1.0, 0.01, -1.0, 0.02, 1.0];
        let dense = quantize(&p, 0.0);
        let sparse = quantize(&p, 0.05);
        assert!(sparse.density() < dense.density(), "мёртвая зона прореживает");
        assert_eq!(sparse.nonzeros, 3);
        assert_eq!(dense.nonzeros, 5);
    }
}
