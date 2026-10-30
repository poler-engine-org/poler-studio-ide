//! Орбитальная система: кеплер-лайт честность.
//!
//! Угловая скорость выводится из массы родителя (третий закон Кеплера):
//!
//! ```text
//!   ω = √(G · M_parent) / r^1.5
//! ```
//!
//! Никаких «подобранных на глаз» скоростей (главная ложь бутафорских
//! симуляций): удвоение радиуса орбиты замедляет тело в 2^1.5 ≈ 2.83
//! раза ровно как в физике. Наклон орбитальной плоскости — поворот
//! вокруг X на `inclination`.

/// Орбитальный компонент. Родитель берётся из иерархии World.
#[derive(Clone, Copy, Debug)]
pub struct Orbit {
    /// Радиус круговой орбиты (большая полуось, ед. сцены).
    pub radius: f64,
    /// Текущий фазовый угол (рад, [0, 2π)).
    pub angle: f64,
    /// Угловая скорость (рад/с) — кеплеровская, см. [`Orbit::kepler`].
    pub omega: f64,
    /// Наклон орбитальной плоскости к XZ (рад).
    pub inclination: f64,
}

/// Гравитационная постоянная сцены (подобрана для наглядности демо:
/// звезда M=4 → период орбиты r=1.6 равен ~6.3 с).
pub const G: f64 = 1.0;

impl Orbit {
    /// Кеплеровская орбита: ω = √(G·M)/r^1.5 вокруг родителя массы `m_parent`.
    pub fn kepler(m_parent: f64, radius: f64, angle: f64, inclination: f64) -> Orbit {
        let omega = (G * m_parent).sqrt() / radius.powf(1.5);
        Orbit {
            radius,
            angle,
            omega,
            inclination,
        }
    }

    /// Локальная позиция на орбите для текущего угла.
    pub fn local_pos(&self) -> [f64; 3] {
        let (s, c) = (self.angle.sin(), self.angle.cos());
        let (ci, si) = (self.inclination.cos(), self.inclination.sin());
        [self.radius * c, self.radius * s * si, self.radius * s * ci]
    }

    /// Орбитальный период (с).
    pub fn period(&self) -> f64 {
        2.0 * std::f64::consts::PI / self.omega
    }
}

// ============================================================================
// ТЕСТЫ
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kepler_third_law() {
        // ω = √(G·M)/r^1.5: удвоение радиуса замедляет в 2^1.5 раз
        let a = Orbit::kepler(4.0, 1.6, 0.0, 0.0);
        let b = Orbit::kepler(4.0, 3.2, 0.0, 0.0);
        let ratio = a.omega / b.omega;
        assert!(
            (ratio - 2.0f64.powf(1.5)).abs() < 1e-12,
            "ratio = {ratio}"
        );
        // численная проверка формулы
        let expect = (4.0f64).sqrt() / 1.6f64.powf(1.5);
        assert!((a.omega - expect).abs() < 1e-15);
    }

    #[test]
    fn local_pos_at_zero_angle() {
        let o = Orbit::kepler(4.0, 2.0, 0.0, 0.0);
        let p = o.local_pos();
        assert!((p[0] - 2.0).abs() < 1e-15, "x = r·cos0 = r");
        assert!(p[1].abs() < 1e-15 && p[2].abs() < 1e-15);
        // наклон 90°: орбита в плоскости XY
        let o90 = Orbit {
            inclination: std::f64::consts::FRAC_PI_2,
            angle: std::f64::consts::FRAC_PI_2,
            ..o
        };
        let q = o90.local_pos();
        assert!((q[0]).abs() < 1e-15);
        assert!((q[1] - 2.0).abs() < 1e-15, "y = r·sin(π/2)·sin(90°)");
        assert!((q[2]).abs() < 1e-15);
    }

    #[test]
    fn period_consistent_with_omega() {
        let o = Orbit::kepler(4.0, 1.6, 0.0, 0.0);
        assert!((o.period() * o.omega - 2.0 * std::f64::consts::PI).abs() < 1e-12);
    }

    #[test]
    fn orbit_tick_closes_after_full_period() {
        // интегрируем один период по шагам → фаза возвращается
        let mut o = Orbit::kepler(4.0, 1.6, 0.0, 0.3);
        let dt = o.period() / 360.0;
        for _ in 0..360 {
            o.angle += o.omega * dt;
        }
        // фаза ~2π: позиция совпадает с начальной
        let p = o.local_pos();
        let p0 = Orbit {
            angle: 0.0,
            ..o
        }
        .local_pos();
        for (a, b) in p.iter().zip(p0.iter()) {
            assert!((a - b).abs() < 1e-9, "орбита не замкнулась: {a} vs {b}");
        }
    }
}
