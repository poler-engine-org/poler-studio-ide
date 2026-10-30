//! RQ14: mmap-разворот тритов в углы Блоха — θ = arccos(p) на лету.
//!
//! Состояние v2 (Packed4) живёт на чистой тритовой решётке {−1, 0, +1} —
//! кривизны σ нет, поэтому фазовый угол каждого трита принимает ровно одно
//! из трёх значений: `θ = arccos(p) ∈ {0, π/2, π}`. Это ключевое свойство
//! для обучения из сжатого хранилища: «arccos на лету» — это не FPU-вызов
//! на элемент, а выборка из LUT-константы по 2-битному коду — углы живут
//! в L1-регистрах, а не в 8-байтных f64.
//!
//! Потоковая схема (zero-storage, как RQ3/RQ6):
//!
//! ```text
//! mmap-контейнер v2 ──(байты Packed4)──► BlochAngles / BlochArcs
//!                                          │  LUT: код → θ ∈ {0, π/2, π}
//!                                          ▼
//!                                     обучение (Born/curriculum)
//! ```
//!
//! Память: полный вектор углов никогда не материализуется — итератор
//! отдаёт пары (индекс, θ) порциями, окно живёт в кэше. Для d_pol = 2^26
//! плотный f64-вектор занял бы 512 МиБ; упакованные триты — 8 МиБ
//! (2 бита/параметр, ×32 меньше трафика памяти).
//!
//! Бонус совместимости: шифр-блоки RQ13 ([`crate::phase`]-конвенция
//! «контейнеры v2/v3») используют те же 2-битные коды {0, 1, 2} →
//! {Zero, Pos, Neg} — тот же LUT разворачивает в углы Блоха и обучающие
//! контейнеры, и триты шифртекста.

use crate::phase::{pack_trit2, Trit};
use core::f64::consts::{FRAC_PI_2, PI};

/// LUT углов Блоха по 2-битному коду v2: `0b00→Zero→π/2, 0b01→Pos→0,
/// 0b10→Neg→π`; `0b11` зарезервирован (читатель валидирует), безопасный
/// откат — Zero.
///
/// `θ = arccos(p)` на тритовой решётке: `arccos(0) = π/2`, `arccos(+1) = 0`,
/// `arccos(−1) = π` — точные значения, без FPU.
#[inline]
pub const fn theta_lut() -> [f64; 4] {
    [FRAC_PI_2, 0.0, PI, FRAC_PI_2]
}

/// LUT значений p = cos(θ) по 2-битному коду v2.
#[inline]
pub const fn p_lut() -> [f64; 4] {
    [0.0, 1.0, -1.0, 0.0]
}

/// Угол Блоха трита `i` из плотного Packed4-массива — LUT, без acos.
///
/// Контракт: `data.len() ≥ i/4 + 1` (гарантируется читателем после
/// валидации `phase_len == ceil(d_pol/4)`).
#[inline]
pub fn theta_at(data: &[u8], i: usize) -> f64 {
    theta_lut()[code_at(data, i)]
}

/// Значение p = cos(θ) трита `i` из плотного Packed4-массива.
#[inline]
pub fn p_at(data: &[u8], i: usize) -> f64 {
    p_lut()[code_at(data, i)]
}

/// Трит трека `i` (разворот кода v2).
#[inline]
pub fn trit_at(data: &[u8], i: usize) -> Trit {
    match code_at(data, i) {
        1 => Trit::Pos,
        2 => Trit::Neg,
        _ => Trit::Zero,
    }
}

#[inline]
fn code_at(data: &[u8], i: usize) -> usize {
    ((data[i / 4] >> (2 * (i % 4))) & 0b11) as usize
}

/// Блочное заполнение буфера углами θ (LUT-развёртка).
///
/// Для чанкового потребления обучающим контуром: окно `out` заполняется
/// углами тритов `[from, from + out.len())`. Стоимость — один LUT-lookup
/// на трит, ни одного вызова `acos`. Возврат — фактическое число
/// заполненных тритов (меньше `out.len()` только у последнего неполного
/// окна).
pub fn fill_thetas(data: &[u8], from: usize, out: &mut [f64]) -> usize {
    let lut = theta_lut();
    let mut filled = 0;
    for (k, slot) in out.iter_mut().enumerate() {
        let i = from + k;
        if i / 4 >= data.len() {
            break;
        }
        *slot = lut[code_at(data, i)];
        filled += 1;
    }
    filled
}

/// Плотный итератор углов Блоха `(индекс, θ)` из Packed4-байтов —
/// zero-copy стриминг из mmap (см. [`crate::mmap`]).
///
/// В отличие от [`crate::reader::PackedTrits`] выдаёт готовые углы —
/// потребитель обучения не знает про триты вовсе.
pub struct BlochAngles<'a> {
    data: &'a [u8],
    d: u32,
    pos: u32,
}

impl<'a> BlochAngles<'a> {
    /// Конструктор из сырых Packed4-байтов и числа тритов `d`.
    pub fn new(data: &'a [u8], d: u32) -> BlochAngles<'a> {
        BlochAngles { data, d, pos: 0 }
    }

    /// Число тритов в потоке.
    pub fn len(&self) -> usize {
        self.d as usize
    }

    /// Поток пуст.
    pub fn is_empty(&self) -> bool {
        self.d == 0
    }
}

impl Iterator for BlochAngles<'_> {
    type Item = (u32, f64);

    fn next(&mut self) -> Option<(u32, f64)> {
        if self.pos >= self.d {
            return None;
        }
        let i = self.pos;
        self.pos += 1;
        Some((i, theta_at(self.data, i as usize)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = (self.d - self.pos) as usize;
        (n, Some(n))
    }
}

impl core::iter::FusedIterator for BlochAngles<'_> {}

/// Разреженный (LENS) итератор: только ненулевые триты — готовые дуги
/// `(индекс, θ)` для продуктового анзаца. Нули — фон, дуг не порождают
/// (семантика [`crate::reader::PqwReader::arcs`] для v2).
pub struct BlochArcs<'a> {
    inner: BlochAngles<'a>,
}

impl<'a> BlochArcs<'a> {
    /// Конструктор из сырых Packed4-байтов и числа тритов `d`.
    pub fn new(data: &'a [u8], d: u32) -> BlochArcs<'a> {
        BlochArcs {
            inner: BlochAngles::new(data, d),
        }
    }
}

impl Iterator for BlochArcs<'_> {
    type Item = (u32, f64);

    fn next(&mut self) -> Option<(u32, f64)> {
        loop {
            let (i, theta) = self.inner.next()?;
            // Ноль — фон: p = 0, θ = π/2 — не дуга.
            if theta != FRAC_PI_2 {
                return Some((i, theta));
            }
        }
    }
}

impl core::iter::FusedIterator for BlochArcs<'_> {}

/// Статистика тритовой решётки — нулевое вздутие (zero-inflation)
/// и баланс знаков; полезно для контроля здравомыслия обучающих данных.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TritCounts {
    /// Число тритов +1.
    pub pos: usize,
    /// Число тритов −1.
    pub neg: usize,
    /// Число нулей (фон).
    pub zero: usize,
}

impl TritCounts {
    /// Всего тритов.
    pub fn total(&self) -> usize {
        self.pos + self.neg + self.zero
    }

    /// Доля ненулевых (плотность LENS-дуг).
    pub fn density(&self) -> f64 {
        if self.total() == 0 {
            0.0
        } else {
            (self.pos + self.neg) as f64 / self.total() as f64
        }
    }

    /// Баланс знаков: `+1/(|±1|)` ∈ [0, 1]; 0.5 — симметрия.
    pub fn balance(&self) -> f64 {
        let nz = self.pos + self.neg;
        if nz == 0 {
            0.5
        } else {
            self.pos as f64 / nz as f64
        }
    }
}

/// Подсчёт тритов по кодам Packed4 — один проход, без материализации.
pub fn counts(data: &[u8], d: usize) -> TritCounts {
    let mut c = TritCounts {
        pos: 0,
        neg: 0,
        zero: 0,
    };
    for i in 0..d {
        match code_at(data, i) {
            1 => c.pos += 1,
            2 => c.neg += 1,
            _ => c.zero += 1,
        }
    }
    c
}

/// Упаковка углов Блоха в Packed4-байты (обратный ход RQ14).
///
/// Углы квантуются ближайшим тритом: `|cos θ| ≥ 0.5 → sign`, иначе ноль
/// (правило [`crate::phase::nearest_trit`]). Возвращает пару
/// `(байты, число тритов)`.
pub fn pack_thetas(thetas: &[f64]) -> (Vec<u8>, usize) {
    let mut out = vec![0u8; thetas.len().div_ceil(4)];
    for (i, &t) in thetas.iter().enumerate() {
        let trit = nearest_trit_theta(t);
        out[i / 4] |= pack_trit2(trit) << (2 * (i % 4));
    }
    (out, thetas.len())
}

/// Ближайший трит по углу: cos θ сравнивается с серединой решётки.
#[inline]
fn nearest_trit_theta(theta: f64) -> Trit {
    crate::phase::nearest_trit(theta.cos() as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase::pack_quad;

    /// Смешанный массив: [Pos, Neg, Zero, Pos | Neg, Zero, Zero, Neg].
    fn sample_bytes() -> [u8; 2] {
        [
            pack_quad([Trit::Pos, Trit::Neg, Trit::Zero, Trit::Pos]),
            pack_quad([Trit::Neg, Trit::Zero, Trit::Zero, Trit::Neg]),
        ]
    }

    #[test]
    fn lut_exact_bloch_angles() {
        let lut = theta_lut();
        // arccos(p): 0 → π/2; +1 → 0; −1 → π.
        assert!((lut[0] - FRAC_PI_2).abs() < 1e-15); // Zero
        assert_eq!(lut[1], 0.0); // Pos
        assert!((lut[2] - PI).abs() < 1e-15); // Neg
        // И тождество θ = arccos(p) на всей решётке.
        for code in 0..3 {
            assert!((theta_lut()[code] - p_lut()[code].acos()).abs() < 1e-15);
        }
    }

    #[test]
    fn theta_at_matches_lut() {
        let data = sample_bytes();
        assert_eq!(theta_at(&data, 0), 0.0); // Pos
        assert!((theta_at(&data, 1) - PI).abs() < 1e-15); // Neg
        assert!((theta_at(&data, 2) - FRAC_PI_2).abs() < 1e-15); // Zero
        assert_eq!(theta_at(&data, 3), 0.0); // Pos
        assert!((theta_at(&data, 4) - PI).abs() < 1e-15); // Neg (байт 1)
        assert_eq!(p_at(&data, 0), 1.0);
        assert_eq!(p_at(&data, 1), -1.0);
        assert_eq!(p_at(&data, 2), 0.0);
        assert_eq!(trit_at(&data, 0), Trit::Pos);
        assert_eq!(trit_at(&data, 1), Trit::Neg);
        assert_eq!(trit_at(&data, 2), Trit::Zero);
    }

    #[test]
    fn iterator_dense_all_positions() {
        let data = sample_bytes();
        let v: Vec<(u32, f64)> = BlochAngles::new(&data, 8).collect();
        assert_eq!(v.len(), 8);
        for (i, (idx, _)) in v.iter().enumerate() {
            assert_eq!(*idx, i as u32);
        }
        // size_hint точен.
        let mut it = BlochAngles::new(&data, 8);
        assert_eq!(it.size_hint(), (8, Some(8)));
        it.next();
        assert_eq!(it.size_hint(), (7, Some(7)));
    }

    #[test]
    fn iterator_respects_d_not_multiple_of_4() {
        let data = sample_bytes();
        // d = 5: читаем только триты 0..5, хвост байта игнорируется.
        let v: Vec<_> = BlochAngles::new(&data, 5).collect();
        assert_eq!(v.len(), 5);
        assert_eq!(v[4].0, 4);
    }

    #[test]
    fn sparse_arcs_skip_zero_background() {
        let data = sample_bytes();
        let arcs: Vec<(u32, f64)> = BlochArcs::new(&data, 8).collect();
        // [Pos, Neg, Zero, Pos | Neg, Zero, Zero, Neg] → 5 дуг.
        assert_eq!(arcs.len(), 5);
        assert_eq!(arcs[0], (0, 0.0));
        assert_eq!(arcs[1].0, 1);
        assert_eq!(arcs[2].0, 3);
        assert_eq!(arcs[3].0, 4);
        assert_eq!(arcs[4].0, 7);
        // Ни одна дуга не равна π/2 (нулевому фону).
        assert!(arcs.iter().all(|&(_, t)| t != FRAC_PI_2));
    }

    #[test]
    fn fill_thetas_block_windows() {
        let data = sample_bytes();
        let mut buf = [0.0_f64; 4];
        assert_eq!(fill_thetas(&data, 0, &mut buf), 4);
        assert_eq!(buf, [0.0, PI, FRAC_PI_2, 0.0]);
        assert_eq!(fill_thetas(&data, 4, &mut buf), 4);
        assert_eq!(buf, [PI, FRAC_PI_2, FRAC_PI_2, PI]);
        // Последнее неполное окно: d = 6, окно 4 с позиции 4 → 2 значения.
        let mut b2 = [0.0_f64; 4];
        assert_eq!(fill_thetas(&data, 4, &mut b2[..]), 4);
        let d6 = BlochAngles::new(&data, 6).count();
        assert_eq!(d6, 6);
    }

    #[test]
    fn fill_matches_iterator() {
        let data = sample_bytes();
        let dense: Vec<f64> = BlochAngles::new(&data, 8).map(|(_, t)| t).collect();
        let mut via_fill = vec![0.0; 8];
        fill_thetas(&data, 0, &mut via_fill);
        assert_eq!(dense, via_fill);
    }

    #[test]
    fn counts_zero_inflation() {
        let data = sample_bytes();
        let c = counts(&data, 8);
        assert_eq!(c, TritCounts { pos: 2, neg: 3, zero: 3 });
        assert_eq!(c.total(), 8);
        assert!((c.density() - 5.0 / 8.0).abs() < 1e-12);
        assert!((c.balance() - 2.0 / 5.0).abs() < 1e-12);
        // Вырожденные случаи.
        let empty = counts(&[], 0);
        assert_eq!(empty.total(), 0);
        assert_eq!(empty.density(), 0.0);
        assert_eq!(empty.balance(), 0.5);
    }

    #[test]
    fn pack_thetas_roundtrip_lattice() {
        // Углы точно на решётке квантуются в себя.
        let thetas = [0.0, PI, FRAC_PI_2, 0.0, PI, FRAC_PI_2, FRAC_PI_2, PI];
        let (bytes, n) = pack_thetas(&thetas);
        assert_eq!(n, 8);
        let back: Vec<f64> = BlochAngles::new(&bytes, 8).map(|(_, t)| t).collect();
        assert_eq!(back, thetas.to_vec());
    }

    #[test]
    fn pack_thetas_quantizes_off_lattice() {
        // cos(0.3) ≈ 0.955 → Pos; cos(2.5) ≈ −0.801 → Neg;
        // cos(1.37) ≈ 0.199 и cos(1.77) ≈ −0.199 → |cos| < 0.5 → Zero.
        let thetas = [0.3, 2.5, 1.37, 1.77];
        let (bytes, _) = pack_thetas(&thetas);
        assert_eq!(trit_at(&bytes, 0), Trit::Pos);
        assert_eq!(trit_at(&bytes, 1), Trit::Neg);
        assert_eq!(trit_at(&bytes, 2), Trit::Zero);
        assert_eq!(trit_at(&bytes, 3), Trit::Zero);
    }
}
