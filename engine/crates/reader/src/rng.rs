//! xorshift64 — детерминированный поток насіння (бит-в-бит как в цикле K).
//!
//! Тот же алгоритм, что в `living_voice_synthesizer.py`: все выводы
//! (дрожь формант, связи, маски щели, jitter/shimmer) — чистые функции
//! 64-битного семени. Один seed → одна личность диктора.

/// Генератор xorshift64*. Ноль — запрещённое состояние.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xorshift64(pub u64);

impl Xorshift64 {
    /// Создать из семени (0 заменяется на каноническую константу).
    pub fn new(seed: u64) -> Self {
        Xorshift64(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    /// Следующее значение (13, 7, 17 — классическая тройка Марсальи).
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut s = self.0;
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        self.0 = s;
        s
    }

    /// Равномерное [0, n) — как `next(g) % n` в Python (n > 0).
    #[inline]
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    /// Равномерное [−1, 1) с шагом 1/1000 — как `(x % 2000)/1000 − 1`.
    #[inline]
    pub fn sym_milli(&mut self) -> f64 {
        (self.next_u64() % 2000) as f64 / 1000.0 - 1.0
    }

    /// Равномерное [0, 1) с шагом 1/1000.
    #[inline]
    pub fn unit_milli(&mut self) -> f64 {
        (self.next_u64() % 1000) as f64 / 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_sequence() {
        let mut a = Xorshift64::new(0xC0FFEE_1234ABCD);
        let mut b = Xorshift64::new(0xC0FFEE_1234ABCD);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn known_first_value() {
        // Ручная проверка тройки 13/7/17 на эталонном семени.
        let mut g = Xorshift64::new(1);
        let s0 = 1u64;
        let mut s = s0;
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        assert_eq!(g.next_u64(), s);
    }

    #[test]
    fn zero_seed_is_replaced_not_panicking() {
        let mut g = Xorshift64::new(0);
        assert!(g.next_u64() != 0);
    }

    #[test]
    fn sym_milli_range() {
        let mut g = Xorshift64::new(42);
        for _ in 0..1000 {
            let v = g.sym_milli();
            assert!((-1.0..1.0).contains(&v));
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Xorshift64::new(7);
        let mut b = Xorshift64::new(8);
        let differ = (0..10).any(|_| a.next_u64() != b.next_u64());
        assert!(differ);
    }
}
