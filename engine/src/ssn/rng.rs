//! Детерминированный ГПСЧ xoshiro256** для SSN — ноль внешних зависимостей.
//!
//! Python-доказательства (proofs/ssn_verify3.py) использовали numpy PCG64;
//! Rust-порт не требует траекторной совместимости с Python — тесты проверяют
//! ИНВАРИАНТЫ (активность 1–50%, S < 0.8, коридоры), а не конкретные траектории.
//! Детерминизм же здесь абсолютный: один seed → одна траектория, навсегда.
//!
//! CSE-кодировщик ([`crate::ssn::cse`]) ГПСЧ не использует вовсе —
//! детерминизм по построению, побитовая совместимость с Python.

/// xoshiro256** с посевом через splitmix64 (каноническая схема Блэкмана–Велы).
pub struct Rng {
    s: [u64; 4],
    spare: Option<f64>,
}

#[inline]
fn splitmix64(z: &mut u64) -> u64 {
    *z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut x = *z;
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

impl Rng {
    /// Посев: seed → splitmix64 × 4 → состояние xoshiro.
    /// seed = 0 допустим (splitmix разводит нулевое состояние).
    pub fn new(seed: u64) -> Self {
        let mut z = seed;
        let s = [splitmix64(&mut z), splitmix64(&mut z), splitmix64(&mut z), splitmix64(&mut z)];
        Rng { s, spare: None }
    }

    /// xoshiro256** — основной генератор.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Равномерное [0, 1) — 53 бита мантиссы, как numpy random().
    #[inline]
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Равномерное [lo, hi).
    pub fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f64()
    }

    /// Бернулли: true с вероятностью p.
    #[inline]
    pub fn bernoulli(&mut self, p: f64) -> bool {
        self.f64() < p
    }

    /// Нормальное N(mu, sigma) — Box–Muller с кэшем запасного значения.
    pub fn normal(&mut self, mu: f64, sigma: f64) -> f64 {
        if let Some(z) = self.spare.take() {
            return mu + sigma * z;
        }
        let mut u1 = self.f64();
        if u1 <= f64::EPSILON {
            u1 = f64::EPSILON;
        }
        let u2 = self.f64();
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f64::consts::PI * u2;
        let (z0, z1) = (r * theta.cos(), r * theta.sin());
        self.spare = Some(z1);
        mu + sigma * z0
    }

    /// Экспоненциальное со средним `scale` — обратный CDF.
    pub fn exponential(&mut self, scale: f64) -> f64 {
        let mut u = self.f64();
        if u >= 1.0 - 1e-16 {
            u = 1.0 - 1e-16;
        }
        -((1.0 - u).ln()) * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_same_seed_same_stream() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let da: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let db: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        assert_ne!(da, db);
    }

    #[test]
    fn uniform_in_range() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let x = r.uniform(0.0, 2.0 * std::f64::consts::PI);
            assert!((0.0..2.0 * std::f64::consts::PI).contains(&x));
        }
    }

    #[test]
    fn normal_has_reasonable_moments() {
        let mut r = Rng::new(99);
        let n = 200_000;
        let sum: f64 = (0..n).map(|_| r.normal(0.0, 1.0)).sum();
        let mean = sum / n as f64;
        assert!(mean.abs() < 0.01, "mean={mean}");
    }

    #[test]
    fn exponential_positive_and_scaled() {
        let mut r = Rng::new(123);
        for _ in 0..1000 {
            let x = r.exponential(0.3);
            assert!((0.0..50.0).contains(&x));
        }
    }
}
