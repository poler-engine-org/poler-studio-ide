//! Локальная трансформация сущности.
//!
//! Намеренно минимальная для v0.54.0: позиция + вращение вокруг оси Y
//! (спин) + масштаб. Полные кватернионы/PGL4-трансформации встанут в
//! G-цикле U, когда появится потребность (рендер P³ принимает точки —
//! каркасы колец строятся на месте, см. `render.rs`).

/// Локальная трансформация относительно родителя.
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    /// Локальная позиция относительно родителя (мир — если корень).
    pub pos: [f64; 3],
    /// Собственный угол вращения вокруг Y (накапливается, рад).
    pub spin: f64,
    /// Скорость самовращения (рад/с).
    pub spin_rate: f64,
    /// Масштаб (радиус каркаса в рендере умножается на него).
    pub scale: f64,
}

impl Default for Transform {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

impl Transform {
    /// Нейтральная трансформация: в нуле, без вращения, масштаб 1.
    pub const NEUTRAL: Transform = Transform {
        pos: [0.0, 0.0, 0.0],
        spin: 0.0,
        spin_rate: 0.0,
        scale: 1.0,
    };

    /// Трансформация только с позицией (остальное нейтрально).
    pub fn at(pos: [f64; 3]) -> Transform {
        Transform {
            pos,
            ..Self::NEUTRAL
        }
    }

    /// Самовращение вокруг Y с заданной скоростью.
    pub fn spinning(rate: f64) -> Transform {
        Transform {
            spin_rate: rate,
            ..Self::NEUTRAL
        }
    }
}

// ============================================================================
// ТЕСТЫ
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_is_identity() {
        let t = Transform::NEUTRAL;
        assert_eq!(t.pos, [0.0; 3]);
        assert_eq!(t.scale, 1.0);
        assert_eq!(t.spin, 0.0);
    }

    #[test]
    fn constructors_fill_fields() {
        let t = Transform::at([1.0, 2.0, 3.0]);
        assert_eq!(t.pos, [1.0, 2.0, 3.0]);
        assert_eq!(t.scale, 1.0);
        let s = Transform::spinning(0.5);
        assert_eq!(s.spin_rate, 0.5);
        assert_eq!(s.pos, [0.0; 3]);
    }
}
