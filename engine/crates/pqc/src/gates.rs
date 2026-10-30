//! Каталог гейтов: именованные однокубитные вращения, Паули, Адамар и
//! двухкубитные CNOT/CZ. Конвенции матриц стандартные (младший бит —
//! кубит 0), что закладывает паритет с qiskit в RQ4.

use core::fmt;

use crate::complex::Cx;

/// Один гейт схемы.
#[derive(Clone, Copy, Debug)]
pub enum Gate {
    /// Вращение R_y(θ) — фазовый анзац POLER.
    Ry {
        /// Кубит.
        q: usize,
        /// Угол.
        theta: f64,
    },
    /// Вращение R_x(θ).
    Rx {
        /// Кубит.
        q: usize,
        /// Угол.
        theta: f64,
    },
    /// Вращение R_z(θ).
    Rz {
        /// Кубит.
        q: usize,
        /// Угол.
        theta: f64,
    },
    /// Адамар.
    H {
        /// Кубит.
        q: usize,
    },
    /// Паули X.
    X {
        /// Кубит.
        q: usize,
    },
    /// Паули Y.
    Y {
        /// Кубит.
        q: usize,
    },
    /// Паули Z.
    Z {
        /// Кубит.
        q: usize,
    },
    /// CNOT.
    Cx {
        /// Контроль.
        control: usize,
        /// Цель.
        target: usize,
    },
    /// CZ.
    Cz {
        /// Контроль.
        control: usize,
        /// Цель.
        target: usize,
    },
    /// S = diag(1, i) — фазовый гейт π/2.
    S {
        /// Кубит.
        q: usize,
    },
    /// T = diag(1, e^{iπ/4}) — фазовый гейт π/4 (неточность фазы — одна
    /// из главных ошибок физических машин; здесь она равна нулю).
    T {
        /// Кубит.
        q: usize,
    },
    /// S† = diag(1, −i).
    Sdg {
        /// Кубит.
        q: usize,
    },
    /// T† = diag(1, e^{−iπ/4}).
    Tdg {
        /// Кубит.
        q: usize,
    },
    /// SWAP двух кубитов.
    Swap {
        /// Первый кубит.
        a: usize,
        /// Второй кубит.
        b: usize,
    },
    /// CCX (Тоффоли): два контроля, один target.
    Ccx {
        /// Контроль 1.
        c1: usize,
        /// Контроль 2.
        c2: usize,
        /// Цель.
        target: usize,
    },
    /// Контролируемая фаза CP(θ) = diag(1, 1, 1, e^{iθ}).
    Cp {
        /// Контроль.
        control: usize,
        /// Цель.
        target: usize,
        /// Угол фазы.
        theta: f64,
    },
    /// Произвольная однокубитная 2×2; унитарность на совести вызывающего.
    U2 {
        /// Кубит.
        q: usize,
        /// Матрица.
        m: [[Cx; 2]; 2],
    },
}

impl Gate {
    /// R_y(θ) на кубите `q`.
    pub fn ry(q: usize, theta: f64) -> Gate {
        Gate::Ry { q, theta }
    }

    /// R_x(θ) на кубите `q`.
    pub fn rx(q: usize, theta: f64) -> Gate {
        Gate::Rx { q, theta }
    }

    /// R_z(θ) на кубите `q`.
    pub fn rz(q: usize, theta: f64) -> Gate {
        Gate::Rz { q, theta }
    }

    /// Адамар на кубите `q`.
    pub fn h(q: usize) -> Gate {
        Gate::H { q }
    }

    /// X на кубите `q`.
    pub fn x(q: usize) -> Gate {
        Gate::X { q }
    }

    /// Y на кубите `q`.
    pub fn y(q: usize) -> Gate {
        Gate::Y { q }
    }

    /// Z на кубите `q`.
    pub fn z(q: usize) -> Gate {
        Gate::Z { q }
    }

    /// CNOT control → target.
    pub fn cx(control: usize, target: usize) -> Gate {
        Gate::Cx { control, target }
    }

    /// CZ на паре кубитов.
    pub fn cz(control: usize, target: usize) -> Gate {
        Gate::Cz { control, target }
    }

    /// S на кубите `q`.
    pub fn s(q: usize) -> Gate {
        Gate::S { q }
    }

    /// T на кубите `q`.
    pub fn t(q: usize) -> Gate {
        Gate::T { q }
    }

    /// S† на кубите `q`.
    pub fn sdg(q: usize) -> Gate {
        Gate::Sdg { q }
    }

    /// T† на кубите `q`.
    pub fn tdg(q: usize) -> Gate {
        Gate::Tdg { q }
    }

    /// SWAP пары кубитов.
    pub fn swap(a: usize, b: usize) -> Gate {
        Gate::Swap { a, b }
    }

    /// CCX (Тоффоли).
    pub fn ccx(c1: usize, c2: usize, target: usize) -> Gate {
        Gate::Ccx { c1, c2, target }
    }

    /// Контролируемая фаза CP(θ).
    pub fn cp(control: usize, target: usize, theta: f64) -> Gate {
        Gate::Cp {
            control,
            target,
            theta,
        }
    }
}

impl fmt::Display for Gate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Gate::Ry { q, theta } => write!(f, "Ry(q{q}, {theta:.4})"),
            Gate::Rx { q, theta } => write!(f, "Rx(q{q}, {theta:.4})"),
            Gate::Rz { q, theta } => write!(f, "Rz(q{q}, {theta:.4})"),
            Gate::H { q } => write!(f, "H(q{q})"),
            Gate::X { q } => write!(f, "X(q{q})"),
            Gate::Y { q } => write!(f, "Y(q{q})"),
            Gate::Z { q } => write!(f, "Z(q{q})"),
            Gate::Cx { control, target } => write!(f, "CX(q{control} -> q{target})"),
            Gate::Cz { control, target } => write!(f, "CZ(q{control}, q{target})"),
            Gate::S { q } => write!(f, "S(q{q})"),
            Gate::T { q } => write!(f, "T(q{q})"),
            Gate::Sdg { q } => write!(f, "Sdg(q{q})"),
            Gate::Tdg { q } => write!(f, "Tdg(q{q})"),
            Gate::Swap { a, b } => write!(f, "SWAP(q{a}, q{b})"),
            Gate::Ccx { c1, c2, target } => {
                write!(f, "CCX(q{c1}, q{c2} -> q{target})")
            }
            Gate::Cp {
                control,
                target,
                theta,
            } => write!(f, "CP(q{control}, q{target}, {theta:.4})"),
            Gate::U2 { q, .. } => write!(f, "U2(q{q})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // 1.5708 — намеренный литерал Display-теста (строка "Ry(q3, 1.5708)"),
    // не приближение π/2 для математики; poler-side фикс под CI-гейт
    // clippy::correctness после всасывания в workspace (M2).
    #[allow(clippy::approx_constant)]
    fn display_is_informative() {
        assert_eq!(Gate::ry(3, 1.5708).to_string(), "Ry(q3, 1.5708)");
        assert_eq!(Gate::cx(0, 1).to_string(), "CX(q0 -> q1)");
    }

    #[test]
    fn constructors_match_variants() {
        assert!(matches!(Gate::ry(1, 0.5), Gate::Ry { q: 1, theta: 0.5 }));
        assert!(matches!(Gate::h(2), Gate::H { q: 2 }));
        assert!(matches!(
            Gate::cz(3, 4),
            Gate::Cz {
                control: 3,
                target: 4
            }
        ));
    }
}
