//! Тритная щель {-1, 0, +1} — No-Mul слой живого голоса.
//!
//! Слой: циклический сдвиг + инверсия по маске + своп по маске.
//! Ноль умножений (только перестановки и отрицания) — биекция,
//! Ландауэр ΔS = 0. Количество нулей — инвариант слоя.

pub const TRITS: [i8; 3] = [-1, 0, 1];

/// Размер состояния щели (3×(+1), 3×(−1), 2×0 — сбалансировано).
pub const SLIT_K: usize = 8;

/// Один шаг слоя: rotate → negate → swap. Биекция на (i8)^8.
#[inline]
pub fn no_mul_trit_layer(state: &[i8; SLIT_K], neg_mask: u8, swap_mask: u8) -> [i8; SLIT_K] {
    let mut s = *state;
    // циклический сдвиг влево на 1
    s.rotate_left(1);
    for i in 0..SLIT_K {
        if (neg_mask >> i) & 1 == 1 {
            s[i] = -s[i];
        }
    }
    let mut i = 0;
    while i + 1 < SLIT_K {
        if (swap_mask >> i) & 1 == 1 {
            s.swap(i, i + 1);
        }
        i += 2;
    }
    s
}

/// Сбалансированное начальное состояние из семени (Fisher–Yates):
/// {+1,+1,+1,−1,−1,−1,0,0} перемешано. Число нулей — инвариант,
/// поэтому p(0) остаётся физиологичным навсегда.
pub fn balanced_trit_state(g: &mut crate::rng::Xorshift64) -> [i8; SLIT_K] {
    let mut pool: [i8; SLIT_K] = [1, 1, 1, -1, -1, -1, 0, 0];
    for i in (1..SLIT_K).rev() {
        let j = (g.below((i + 1) as u64) as usize).min(i);
        pool.swap(i, j);
    }
    pool
}

/// Открытая квота щели по триту (физиология связок):
/// +1 — открытая (0.62), 0 — ламинарная (0.38), −1 — закрытая (0.12).
#[inline]
pub fn open_quotient(trit: i8) -> f64 {
    match trit {
        1 => 0.62,
        0 => 0.38,
        _ => 0.12,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Xorshift64;

    #[test]
    fn layer_is_bijection_on_sample() {
        // на случайном состоянии разные маски дают разные результаты,
        // а повторное применение обратимо при фиксированных масках
        // (rotate/neg/swap обратимы) — проверяем обратимость композицией
        let st = [1, -1, 0, 1, 0, -1, 1, -1];
        let nm = 0b1010_1010u8;
        let sm = 0b0100_0001u8;
        let fwd = no_mul_trit_layer(&st, nm, sm);
        // обратный проход: обратный своп → обратная инверсия → обратный сдвиг
        let mut r = fwd;
        let mut i = 0;
        while i + 1 < SLIT_K {
            if (sm >> i) & 1 == 1 {
                r.swap(i, i + 1);
            }
            i += 2;
        }
        for i in 0..SLIT_K {
            if (nm >> i) & 1 == 1 {
                r[i] = -r[i];
            }
        }
        r.rotate_right(1);
        assert_eq!(r, st, "слой обратим (биекция)");
    }

    #[test]
    fn zero_count_is_invariant() {
        let mut g = Xorshift64::new(0xBEEF);
        let mut st = balanced_trit_state(&mut g);
        let z0 = st.iter().filter(|&&t| t == 0).count();
        assert_eq!(z0, 2);
        for _ in 0..1000 {
            st = no_mul_trit_layer(&st, 0xA5, 0x5A);
            let z = st.iter().filter(|&&t| t == 0).count();
            assert_eq!(z, 2, "число нулей — инвариант слоя");
        }
    }

    #[test]
    fn trit_values_only_three() {
        let mut g = Xorshift64::new(7);
        let mut st = balanced_trit_state(&mut g);
        for _ in 0..500 {
            st = no_mul_trit_layer(&st, 0xFF, 0x00);
            assert!(st.iter().all(|&t| t == -1 || t == 0 || t == 1));
        }
    }

    #[test]
    fn open_quotient_ordering() {
        assert!(open_quotient(1) > open_quotient(0));
        assert!(open_quotient(0) > open_quotient(-1));
    }

    #[test]
    fn balanced_state_deterministic() {
        let mut a = Xorshift64::new(123);
        let mut b = Xorshift64::new(123);
        assert_eq!(balanced_trit_state(&mut a), balanced_trit_state(&mut b));
    }
}
