//! ГПСЧ xoshiro256++ с сеянием SplitMix64 — статистически строгий
//! генератор без внешних зависимостей (`rand` не нужен).
//!
//! Период 2²⁵⁶ − 1, экваториальное распределение, `jump`/`long_jump`
//! дают непересекающиеся потоки (2¹²⁸ / 2¹⁹² тактов) — параллельные
//! выборки Born не коррелируют.

/// Генератор xoshiro256++ (Blackman, Vigna, 2019).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    s: [u64; 4],
}

/// SplitMix64 — генератор сеяния (Vigna, 2015).
#[inline]
pub(crate) fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Один шаг xoshiro256++ над сырым состоянием (без возврата значения).
#[inline]
fn next_raw(s: &mut [u64; 4]) {
    let t = s[1] << 17;
    s[2] ^= s[0];
    s[3] ^= s[1];
    s[1] ^= s[2];
    s[0] ^= s[3];
    s[2] ^= t;
    s[3] = s[3].rotate_left(45);
}

impl Rng {
    /// Детерминированное сеяние из одного `u64` через SplitMix64.
    pub fn seed_from_u64(seed: u64) -> Rng {
        let mut sm = seed;
        let mut s = [0u64; 4];
        for slot in &mut s {
            *slot = splitmix64(&mut sm);
        }
        // Вырожденное нулевое состояние практически недостижимо, но контракт
        // xoshiro требует отличного от нуля — подстраховываемся.
        if s == [0u64; 4] {
            s[0] = 0x9E37_79B9_7F4A_7C15;
        }
        Rng { s }
    }

    /// Энтропийное сеяние: `/dev/urandom` (unix), иначе время + PID.
    pub fn from_entropy() -> Rng {
        #[cfg(unix)]
        {
            use std::io::Read;
            if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
                let mut buf = [0u8; 32];
                if f.read_exact(&mut buf).is_ok() {
                    let mut s = [0u64; 4];
                    for (i, slot) in s.iter_mut().enumerate() {
                        let mut w = [0u8; 8];
                        w.copy_from_slice(&buf[i * 8..i * 8 + 8]);
                        *slot = u64::from_le_bytes(w);
                    }
                    if s != [0u64; 4] {
                        return Rng { s };
                    }
                }
            }
        }
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let pid = u64::from(std::process::id());
        Rng::seed_from_u64(nanos ^ pid.rotate_left(32))
    }

    /// Следующее `u64`.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let r = self.s[0]
            .wrapping_add(self.s[3])
            .rotate_left(23)
            .wrapping_add(self.s[0]);
        next_raw(&mut self.s);
        r
    }

    /// Равномерное `f64` в [0, 1): 53 случайных бита мантиссы.
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Прыжок на 2¹²⁸ вызовов вперёд — независимый поток.
    pub fn jump(&mut self) {
        jump_with(&mut self.s, &JUMP);
    }

    /// Длинный прыжок на 2¹⁹² вызовов.
    pub fn long_jump(&mut self) {
        jump_with(&mut self.s, &LONG_JUMP);
    }
}

const JUMP: [u64; 4] = [
    0x180e_c6f3_3cb6_496b,
    0xd5a6_1266_f0c9_392c,
    0xa958_2618_e03f_c9aa,
    0x39ab_dc45_29b1_661c,
];

const LONG_JUMP: [u64; 4] = [
    0xbeac_0467_eba5_facb,
    0xd86b_048b_86aa_9922,
    0x290c_9637_7e11_b5bd,
    0x738d_0d2c_6ad3_ed7a,
];

fn jump_with(s: &mut [u64; 4], jump: &[u64; 4]) {
    let mut work = [0u64; 4];
    for &j in jump {
        for b in 0..64 {
            if (j >> b) & 1 == 1 {
                for i in 0..4 {
                    work[i] ^= s[i];
                }
            }
            next_raw(s);
        }
    }
    *s = work;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::seed_from_u64(42);
        let mut b = Rng::seed_from_u64(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::seed_from_u64(1);
        let mut b = Rng::seed_from_u64(2);
        let diff = (0..64).filter(|_| a.next_u64() != b.next_u64()).count();
        assert!(diff > 32, "streams overlap in {diff}/64 draws");
    }

    #[test]
    fn zero_seed_is_valid() {
        let mut rng = Rng::seed_from_u64(0);
        assert_ne!(rng.next_u64(), 0);
    }

    #[test]
    fn next_f64_bounds_and_mean() {
        let mut rng = Rng::seed_from_u64(7);
        let n = 100_000;
        let mut sum = 0.0;
        let mut min = 1.0_f64;
        let mut max = 0.0_f64;
        for _ in 0..n {
            let u = rng.next_f64();
            assert!((0.0..1.0).contains(&u));
            sum += u;
            min = min.min(u);
            max = max.max(u);
        }
        let mean = sum / n as f64;
        assert!((mean - 0.5).abs() < 0.01, "mean = {mean}");
        assert!(min < 0.01 && max > 0.99, "range [{min}, {max}] too narrow");
    }

    #[test]
    fn all_bits_fire() {
        let mut rng = Rng::seed_from_u64(3);
        let mut seen = [false; 64];
        for _ in 0..2_000 {
            let x = rng.next_u64();
            for b in 0..64 {
                if (x >> b) & 1 == 1 {
                    seen[b] = true;
                }
            }
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn jump_creates_independent_stream() {
        let mut a = Rng::seed_from_u64(42);
        let mut b = Rng::seed_from_u64(42);
        b.jump();
        let diff = (0..64).filter(|_| a.next_u64() != b.next_u64()).count();
        assert!(diff > 32);
        // Прыжок необратим в пределах разумного числа шагов:
        let mut c = Rng::seed_from_u64(42);
        for _ in 0..1000 {
            c.next_u64();
        }
        let mut b2 = Rng::seed_from_u64(42);
        b2.jump();
        assert_ne!(c.next_u64(), b2.next_u64());
    }

    #[test]
    fn clone_reproduces_stream() {
        let mut a = Rng::seed_from_u64(9);
        let mut b = a.clone();
        for _ in 0..16 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }
}
