//! Комплексная амплитуда `Cx { re, im }` — минимальная арифметика без
//! внешних крейтов: statevector-ядрам нужен фиксированный набор операций,
//! все помечены `#[inline]` и авто-векторизуются вместе с окружающими циклами.

use core::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub};

/// Комплексное число — пара `f64`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cx {
    /// Вещественная часть.
    pub re: f64,
    /// Мнимая часть.
    pub im: f64,
}

impl Cx {
    /// Ноль.
    pub const ZERO: Cx = Cx { re: 0.0, im: 0.0 };
    /// Единица.
    pub const ONE: Cx = Cx { re: 1.0, im: 0.0 };
    /// Мнимая единица.
    pub const I: Cx = Cx { re: 0.0, im: 1.0 };

    /// Конструктор.
    #[inline]
    pub const fn new(re: f64, im: f64) -> Cx {
        Cx { re, im }
    }

    /// Комплексно-сопряжённое.
    #[inline]
    pub const fn conj(self) -> Cx {
        Cx {
            re: self.re,
            im: -self.im,
        }
    }

    /// |z|² = re² + im².
    #[inline]
    pub fn norm_sq(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    /// |z|.
    #[inline]
    pub fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }

    /// Умножение на вещественное.
    #[inline]
    pub fn scale(self, k: f64) -> Cx {
        Cx {
            re: self.re * k,
            im: self.im * k,
        }
    }

    /// e^{iθ} = cos θ + i·sin θ (фазовые гейты S/T/CP).
    #[inline]
    pub fn cis(theta: f64) -> Cx {
        Cx {
            re: theta.cos(),
            im: theta.sin(),
        }
    }
}

impl Add for Cx {
    type Output = Cx;
    #[inline]
    fn add(self, rhs: Cx) -> Cx {
        Cx {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }
}

impl AddAssign for Cx {
    #[inline]
    fn add_assign(&mut self, rhs: Cx) {
        *self = *self + rhs;
    }
}

impl Sub for Cx {
    type Output = Cx;
    #[inline]
    fn sub(self, rhs: Cx) -> Cx {
        Cx {
            re: self.re - rhs.re,
            im: self.im - rhs.im,
        }
    }
}

impl Mul for Cx {
    type Output = Cx;
    #[inline]
    fn mul(self, rhs: Cx) -> Cx {
        Cx {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl MulAssign for Cx {
    #[inline]
    fn mul_assign(&mut self, rhs: Cx) {
        *self = *self * rhs;
    }
}

impl Mul<f64> for Cx {
    type Output = Cx;
    #[inline]
    fn mul(self, k: f64) -> Cx {
        self.scale(k)
    }
}

impl Neg for Cx {
    type Output = Cx;
    #[inline]
    fn neg(self) -> Cx {
        Cx {
            re: -self.re,
            im: -self.im,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_and_conj() {
        assert_eq!(Cx::ONE * Cx::I, Cx::I);
        assert_eq!(Cx::I * Cx::I, -Cx::ONE);
        assert_eq!(Cx::new(3.0, -4.0).conj(), Cx::new(3.0, 4.0));
    }

    #[test]
    fn norm_sq_and_norm() {
        assert_eq!(Cx::new(3.0, 4.0).norm_sq(), 25.0);
        assert_eq!(Cx::new(3.0, 4.0).norm(), 5.0);
        assert_eq!(Cx::ZERO.norm(), 0.0);
    }

    #[test]
    fn mul_distributes_over_add() {
        let (a, b, c) = (Cx::new(1.5, -0.5), Cx::new(-2.0, 0.25), Cx::new(0.75, 1.25));
        assert!(((a * (b + c)) - (a * b + a * c)).norm() < 1e-15);
    }

    #[test]
    fn scale_and_neg() {
        assert_eq!(Cx::new(1.0, -2.0).scale(2.0), Cx::new(2.0, -4.0));
        assert_eq!(-Cx::new(1.0, -2.0), Cx::new(-1.0, 2.0));
        assert_eq!(Cx::new(1.0, 2.0) * 2.0, Cx::new(2.0, 4.0));
    }
}
