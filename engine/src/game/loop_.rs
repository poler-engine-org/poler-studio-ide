//! GameClock: фиксированный шаг + аккумулятор (детерминизм-first).
//!
//! Ответ на боль «сколько мс прошло — столько и просимулировали»:
//! симуляция живёт только целыми тиками по `FIXED_DT`. Реальное время
//! копится в аккумуляторе; хвост никогда не симулируется — он переносится.
//! Поэтому 0.5 с реального времени при dt=1/60 это ровно 30 тиков на
//! любой машине, любой частоте кадров, любом джиттере — бит-в-бит.

/// Фиксированный шаг симуляции: 60 Гц.
pub const FIXED_DT: f64 = 1.0 / 60.0;

/// Часы игрового цикла.
#[derive(Clone, Copy, Debug)]
pub struct GameClock {
    /// Шаг симуляции (с).
    pub dt: f64,
    /// Накопитель реального времени (с), остаток < dt.
    accumulator: f64,
    /// Счётчик выданных тиков.
    pub tick: u64,
}

impl Default for GameClock {
    fn default() -> Self {
        GameClock::new(FIXED_DT)
    }
}

impl GameClock {
    pub fn new(dt: f64) -> Self {
        assert!(dt > 0.0 && dt.is_finite(), "dt должен быть конечным > 0");
        GameClock {
            dt,
            accumulator: 0.0,
            tick: 0,
        }
    }

    /// Принять прошедшее реальное время; вернуть число целых тиков.
    /// Ограничение кадра: не более `max_ticks` за вызов (спираль смерти
    /// не разгоняется — хвост отбрасывается, как в честных движках).
    pub fn advance(&mut self, real_seconds: f64, max_ticks: usize) -> usize {
        if real_seconds <= 0.0 || !real_seconds.is_finite() {
            return 0;
        }
        self.accumulator += real_seconds;
        let mut n = 0usize;
        while self.accumulator >= self.dt && n < max_ticks {
            self.accumulator -= self.dt;
            self.tick += 1;
            n += 1;
        }
        if n == max_ticks && self.accumulator > self.dt {
            // догонять не будем: сброс хвоста (frame-drop политика)
            self.accumulator = 0.0;
        }
        n
    }

    /// Остаток аккумулятора (доля шага, 0..dt) — для интерполяции рендера.
    pub fn alpha(&self) -> f64 {
        self.accumulator / self.dt
    }
}

// ============================================================================
// ТЕСТЫ
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_second_is_thirty_ticks() {
        let mut c = GameClock::default();
        assert_eq!(c.advance(0.5, 100), 30, "0.5 с / (1/60) = 30");
        assert_eq!(c.tick, 30);
        assert!(c.alpha() < 1e-9 || (c.alpha() > 1.0 - 1e-9), "хвост почти нулевой: {}", c.alpha());
    }

    #[test]
    fn leftover_carries_over() {
        let mut c = GameClock::default();
        assert_eq!(c.advance(0.016, 100), 0, "16 мс < 1/60 = 16.67 мс");
        assert_eq!(c.advance(0.016, 100), 1, "32 мс > 1 тик");
        // 0.032 суммарно: ровно 1 тик + хвост ~1.33 мс
        let a = c.alpha();
        assert!((0.0..1.0).contains(&a));
    }

    #[test]
    fn death_spiral_capped() {
        let mut c = GameClock::default();
        // «зависли» на 10 секунд — но не более 5 тиков за вызов
        let n = c.advance(10.0, 5);
        assert_eq!(n, 5);
        assert_eq!(c.accumulator, 0.0, "хвост сброшен после capped-догоняния");
    }

    #[test]
    fn zero_and_negative_time_is_free() {
        let mut c = GameClock::default();
        assert_eq!(c.advance(0.0, 10), 0);
        assert_eq!(c.advance(-1.0, 10), 0, "отрицательное время игнорируется");
        assert_eq!(c.tick, 0);
    }

    #[test]
    fn custom_dt() {
        let mut c = GameClock::new(0.05);
        assert_eq!(c.advance(0.11, 10), 2);
        assert!((c.alpha() - 0.2).abs() < 1e-9, "остаток 0.01/0.05");
    }
}
