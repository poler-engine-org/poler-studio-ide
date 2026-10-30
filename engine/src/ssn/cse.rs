//! CSE — Contextual Semantic Encoder: текст → вектор через золотую фазу.
//!
//! Доказано (V8, proofs/ssn_verify3.py): посимвольная фаза, выведенная из
//! кода символа через золотое сечение φ = 0.618…, даёт колоссальную
//! разделимость: для канонического триплета cos(a,b) = 0.999 против
//! cos(a,c) = −0.355 — зазор 1.353 (без золотой фазы: 0.02).
//!
//! Схема на символ `ch` (позиция i, код c = ord(ch)):
//! - вес позиции wp = 1/(1+i);
//! - фаза cphase = (c·φ) mod 2π — у каждого символа СВОЯ фаза;
//! - 16 бит-частот: freq_j = 1 + 0.3·j, phase = 0.01·i + cphase;
//! - бит=1 → sin(freq·k + phase)·wp; бит=0 → 0.5·cos(freq·k + phase)·wp;
//! - центрирование по среднему, нормировка ‖v‖ = 1.
//!
//! Ноль ГПСЧ: один текст → один вектор, побитово воспроизводимо
//! (совместимо с Python-доказательством в пределах f64-округления).

/// Золотое сечение (минус 1): 1/φ = φ − 1 = 0.618…
pub const PHI: f64 = 0.618_033_988_749_894_9;

/// Число бит-частот на символ.
pub const BITS: usize = 16;

/// Кодировать текст в D-мерный единичный вектор.
pub fn encode(text: &str, d: usize) -> Vec<f64> {
    let mut vec = vec![0.0f64; d];
    if d == 0 {
        return vec;
    }
    let tau = 2.0 * std::f64::consts::PI;
    for (i, ch) in text.chars().enumerate() {
        let code = ch as u32;
        let wp = 1.0 / (1.0 + i as f64);
        let cphase = (code as f64 * PHI) % tau;
        let nbits = BITS.min(d);
        for j in 0..nbits {
            let bit = (code >> j) & 1;
            let freq = 1.0 + j as f64 * 0.3;
            let phase = i as f64 * 0.01 + cphase;
            if bit == 1 {
                for (k, v) in vec.iter_mut().enumerate() {
                    *v += (freq * k as f64 + phase).sin() * wp;
                }
            } else {
                for (k, v) in vec.iter_mut().enumerate() {
                    *v += (freq * k as f64 + phase).cos() * 0.5 * wp;
                }
            }
        }
    }
    // центрирование + нормировка
    let mean = vec.iter().sum::<f64>() / d as f64;
    for v in vec.iter_mut() {
        *v -= mean;
    }
    let n = norm(&vec);
    if n > 0.0 {
        for v in vec.iter_mut() {
            *v /= n;
        }
    }
    vec
}

/// Евклидова норма.
pub fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// Скалярное произведение.
pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Косинусное сходство (устойчивое к нулевым векторам).
pub fn cos_sim(a: &[f64], b: &[f64]) -> f64 {
    let denom = norm(a) * norm(b) + 1e-12;
    dot(a, b) / denom
}

/// Синусная коррекция сходства (FLAG из доказательств): cos → sin(cos·π/2).
/// Растягивает окрестность |cos|≈1 и сжимает неопределённую середину.
pub fn sin_corrected(a: &[f64], b: &[f64]) -> f64 {
    (cos_sim(a, b) * std::f64::consts::FRAC_PI_2).sin()
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "герой идёт в поход против тьмы";
    const B: &str = "герой идёт в поход против бездны";
    const C: &str = "кофеварка сломалась вчера";

    #[test]
    fn v8a_determinism() {
        assert_eq!(encode(A, 128), encode(A, 128));
    }

    #[test]
    fn v8b_norm_is_one() {
        let v = encode(A, 128);
        assert!((norm(&v) - 1.0).abs() < 1e-9, "norm={}", norm(&v));
    }

    #[test]
    fn v8c_separation_gap() {
        let (va, vb, vc) = (encode(A, 128), encode(B, 128), encode(C, 128));
        let cab = cos_sim(&va, &vb);
        let cac = cos_sim(&va, &vc);
        let gap = cab - cac;
        assert!(gap > 0.5, "cos(a,b)={cab:.3} cos(a,c)={cac:.3} gap={gap:.3}");
        assert!(cac < 0.0, "непохожий текст должен дать отрицательный cos, got {cac:.3}");
    }

    #[test]
    fn v8d_bounded() {
        let (va, vb, vc) = (encode(A, 128), encode(B, 128), encode(C, 128));
        assert!(cos_sim(&va, &vb).abs() <= 1.0);
        assert!(cos_sim(&va, &vc).abs() <= 1.0);
    }

    #[test]
    fn v8e_norm_any_length() {
        let vs = encode("а", 128);
        let vl = encode(&"а".repeat(1000), 128);
        assert!((norm(&vs) - 1.0).abs() < 1e-9);
        assert!((norm(&vl) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn v8f_sin_correction_monotone_bounded() {
        let (va, vb, vc) = (encode(A, 128), encode(B, 128), encode(C, 128));
        let s_ab = sin_corrected(&va, &vb);
        let s_ac = sin_corrected(&va, &vc);
        assert!(s_ab >= s_ac);
        assert!(s_ab.abs() <= 1.0 && s_ac.abs() <= 1.0);
    }

    /// Паритет с Python-доказательством: те же числовые значения в f64.
    /// Python: cos(a,b)=0.9987, cos(a,c)=-0.3547 (V8-прогон сьюта).
    #[test]
    fn parity_with_python_proof() {
        let (va, vb, vc) = (encode(A, 128), encode(B, 128), encode(C, 128));
        let cab = cos_sim(&va, &vb);
        let cac = cos_sim(&va, &vc);
        assert!((cab - 0.9987).abs() < 5e-4, "cos(a,b)={cab:.6}");
        assert!((cac - (-0.3547)).abs() < 5e-4, "cos(a,c)={cac:.6}");
    }
}
