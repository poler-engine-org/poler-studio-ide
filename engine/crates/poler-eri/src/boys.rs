//! Boys function F_m(T) evaluation.
//!
//! The Boys function is the core special function in Gaussian integral theory:
//!
//! ```text
//! F_m(T) = ∫₀¹ u^(2m) * exp(-T * u²) du
//! ```
//!
//! It appears as the radial part of every two-electron integral. The angular
//! momentum `m` determines the power of `u`, and `T = rho * R_PQ²` is the
//! argument that depends on the reduced exponent and inter-center distance.
//!
//! # Implementation
//!
//! We use a numerical approximation based on the upward recurrence:
//! - F_0(T) = sqrt(π/T) * erf(sqrt(T)) / 2  for T > 0
//! - F_0(0) = 1.0
//! - F_{m+1}(T) = [(2m+1) * F_m(T) - exp(-T)] / (2T)
//!
//! For small T (< 1e-12), we use the Taylor expansion to avoid division by zero.

/// Evaluate the Boys function F_m(T) for a single (m, T) pair.
///
/// # Arguments
///
/// * `m` — angular momentum order (0, 1, 2, ...)
/// * `t` — argument T = rho * R_PQ², must be non-negative
///
/// # Returns
///
/// The value F_m(T). Always non-negative for valid inputs.
///
/// # Examples
///
/// ```
/// use poler_eri::boys_f;
///
/// // F_0(0) = 1.0 (by definition)
/// assert!((boys_f(0, 0.0) - 1.0).abs() < 1e-10);
///
/// // F_0(1) ≈ 0.7468 (known value)
/// assert!((boys_f(0, 1.0) - 0.7468).abs() < 0.001);
/// ```
pub fn boys_f(m: usize, t: f64) -> f64 {
    if t < 1e-12 {
        // Taylor expansion for T → 0:
        // F_m(0) = 1/(2m+1)
        return 1.0 / (2.0 * m as f64 + 1.0);
    }

    // Compute F_0(T) = sqrt(π/T) * erf(sqrt(T)) / 2
    let sqrt_t = libm::sqrt(t);
    let f0 = libm::sqrt(std::f64::consts::PI / t) * libm::erf(sqrt_t) / 2.0;

    // Upward recurrence: F_{m+1}(T) = [(2m+1)*F_m(T) - exp(-T)] / (2T)
    let exp_t = libm::exp(-t);
    let mut fm = f0;
    for n in 0..m {
        fm = ((2.0 * n as f64 + 1.0) * fm - exp_t) / (2.0 * t);
    }
    fm
}

/// Evaluate Boys function for all m = 0..=m_max at once.
///
/// More efficient than calling `boys_f` repeatedly because F_0(T) is only
/// computed once and the upward recurrence is used for subsequent values.
///
/// # Returns
///
/// A vector of length `m_max + 1` where `result[m] = F_m(T)`.
pub fn boys_f_vec(m_max: usize, t: f64) -> Vec<f64> {
    let mut result = Vec::with_capacity(m_max + 1);
    if t < 1e-12 {
        for m in 0..=m_max {
            result.push(1.0 / (2.0 * m as f64 + 1.0));
        }
        return result;
    }

    let sqrt_t = libm::sqrt(t);
    let f0 = libm::sqrt(std::f64::consts::PI / t) * libm::erf(sqrt_t) / 2.0;
    let exp_t = libm::exp(-t);

    result.push(f0);
    for m in 0..m_max {
        let fm = result[m];
        result.push(((2.0 * m as f64 + 1.0) * fm - exp_t) / (2.0 * t));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boys_f0_zero() {
        let val = boys_f(0, 0.0);
        assert!((val - 1.0).abs() < 1e-10, "F_0(0) should be 1.0, got {}", val);
    }

    #[test]
    fn test_boys_f0_one() {
        let val = boys_f(0, 1.0);
        assert!((val - 0.7468).abs() < 0.001, "F_0(1) should be ~0.7468, got {}", val);
    }

    #[test]
    fn test_boys_f1_zero() {
        let val = boys_f(1, 0.0);
        // F_1(0) = 1/3
        assert!((val - 1.0/3.0).abs() < 1e-10, "F_1(0) should be 1/3, got {}", val);
    }

    #[test]
    fn test_boys_f_vec() {
        let vals = boys_f_vec(3, 1.0);
        assert_eq!(vals.len(), 4);
        for m in 0..=3 {
            assert!((vals[m] - boys_f(m, 1.0)).abs() < 1e-12,
                "boys_f_vec mismatch at m={}", m);
        }
    }

    #[test]
    fn test_boys_positivity() {
        for m in 0..5 {
            for t in [0.0, 0.1, 1.0, 10.0, 100.0].iter() {
                assert!(boys_f(m, *t) > 0.0,
                    "F_{}({}) should be positive", m, t);
            }
        }
    }
}
