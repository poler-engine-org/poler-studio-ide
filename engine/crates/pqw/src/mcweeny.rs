//! McWeeny-очистка (1960): `P_new = 3P² − 2P³` — восстановление идемпотентности
//! `P² = P` на многообразии Грассмана без переобучения.
//!
//! Для по-дугового (диагонального) представления каждый трек `p ∈ [−1, 1]`
//! задаёт собственное значение `λ = (1 + p)/2` однокубитного проектора;
//! один шаг потока: `λ' = 3λ² − 2λ³`. Чистые фазы ±1 и явный нуль —
//! неподвижные точки; сильные треки сходятся к тритам за 1–2 такта.

/// Один шаг потока на собственном значении λ: `λ' = 3λ² − 2λ³`.
#[inline]
pub fn purify_lambda(l: f64) -> f64 {
    3.0 * l * l - 2.0 * l * l * l
}

/// Один шаг потока на треке p: `p' = 2·purify_lambda((1+p)/2) − 1`.
#[inline]
pub fn purify_p(p: f64) -> f64 {
    2.0 * purify_lambda((1.0 + p) / 2.0) - 1.0
}

/// Максимум `|λ² − λ|` по всем трекам — отклонение от `P² = P`.
///
/// Именно это число записывается в заголовок файла (инвариант McWeeny).
pub fn idempotency_residual(ps: impl IntoIterator<Item = f64>) -> f64 {
    ps.into_iter()
        .map(|p| {
            let l = (1.0 + p) / 2.0;
            l * (1.0 - l)
        })
        .fold(0.0_f64, f64::max)
}

/// In-place очистка состояния, `steps` итераций. Возвращает финальный residual.
pub fn purify_state(ps: &mut [f64], steps: usize) -> f64 {
    for _ in 0..steps {
        for p in ps.iter_mut() {
            *p = purify_p(*p);
        }
    }
    idempotency_residual(ps.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_points() {
        assert_eq!(purify_p(1.0), 1.0);
        assert_eq!(purify_p(-1.0), -1.0);
        assert_eq!(purify_p(0.0), 0.0);
    }

    #[test]
    fn flow_direction() {
        assert!(purify_p(0.9) > 0.9);
        assert!(purify_p(-0.9) < -0.9);
        assert!(purify_p(0.3) > 0.3);
        assert!(purify_p(-0.3) < -0.3);
    }

    #[test]
    fn lambda_stays_in_unit_interval() {
        for i in 0..=100 {
            let l = f64::from(i) / 100.0;
            let lp = purify_lambda(l);
            assert!((0.0..=1.0).contains(&lp), "lambda {l} -> {lp}");
        }
    }

    #[test]
    fn polynomial_matches_spec() {
        // 3x^2 - 2x^3 == x + x(1-x)(2x-1)
        for i in 0..=50 {
            let x = f64::from(i) / 50.0;
            let lhs = purify_lambda(x);
            let rhs = x + x * (1.0 - x) * (2.0 * x - 1.0);
            assert!((lhs - rhs).abs() < 1e-15);
        }
    }

    #[test]
    fn residual_of_pure_state_is_zero() {
        assert_eq!(idempotency_residual([1.0, -1.0, 1.0]), 0.0);
    }

    #[test]
    fn residual_maximum_at_half() {
        assert_eq!(idempotency_residual([0.0]), 0.25);
        assert!((idempotency_residual([0.5]) - 0.1875).abs() < 1e-15);
        assert!(idempotency_residual([-0.9, 0.9]) < 0.25);
    }

    #[test]
    fn two_steps_purify_strong_tracks() {
        let p = (0..2).fold(0.9_f64, |acc, _| purify_p(acc));
        assert!(p > 0.999, "p = {p}");
        let p = (0..2).fold(-0.9_f64, |acc, _| purify_p(acc));
        assert!(p < -0.999, "p = {p}");
    }

    #[test]
    fn purify_state_monotone_residual() {
        let mut ps = vec![0.9, -0.7, 0.6];
        let r0 = idempotency_residual(ps.iter().copied());
        let r1 = purify_state(&mut ps, 1);
        let r2 = purify_state(&mut ps, 1);
        assert!(r1 < r0);
        assert!(r2 < r1);
        assert_eq!(ps.len(), 3);
    }
}
