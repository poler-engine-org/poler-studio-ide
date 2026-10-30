//! gto_utils.rs — Аналитическая нормализация сферических GTO-орбиталей
//! Содержит аналитический расчет двойного факториала (2l-1)!! без FFI-переполнений.

pub fn double_factorial(n: i32) -> f64 {
    if n <= 0 {
        return 1.0;
    }
    let mut prod = 1.0;
    let mut k = n as f64;
    while k > 0.0 {
        prod *= k;
        k -= 2.0;
    }
    prod
}

/// Вычисление константы нормализации N(alpha, l)
pub fn gto_norm_constant(alpha: f64, l: usize) -> f64 {
    use std::f64::consts::PI;
    let l_i32 = l as i32;
    let s_norm = (2.0 * alpha / PI).powf(0.75);
    let l_factor = (4.0 * alpha).powf(l as f64 / 2.0);
    let dfact = double_factorial(2 * l_i32 - 1).sqrt();
    s_norm * l_factor / dfact
}
