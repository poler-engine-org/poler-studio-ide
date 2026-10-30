//! Фазовые дуги: трит `{−1, 0, +1}` (биты 0..1) + 6-битная кривизна σ (биты 2..7).
//!
//! Декодирование дуги: `p̂ = trit · (σ / 63)`, фазовый угол `θ̂ = arccos(p̂)`.
//! Трит — предельная точка McWeeny-потока (см. [`crate::mcweeny`]), кривизна —
//! квантованная остаточная величина на дуге многообразия Грассмана.

use crate::error::{PqwError, Result};
use core::fmt;

/// Максимум 6-битной кривизны.
pub const SIGMA_MAX: u8 = 63;

/// Трит фазы.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trit {
    /// Фаза −1: собственное значение λ = 0, чистый проектор.
    Neg = 0b00,
    /// Явный нуль: λ = ½ — неподвижная точка McWeeny-потока.
    Zero = 0b01,
    /// Фаза +1: собственное значение λ = 1, чистый проектор.
    Pos = 0b10,
}

impl Trit {
    /// Биты 0..1 фазового байта; `0b11` зарезервирован.
    #[inline]
    pub fn from_bits(bits: u8) -> Result<Trit> {
        match bits & 0b11 {
            0 => Ok(Trit::Neg),
            1 => Ok(Trit::Zero),
            2 => Ok(Trit::Pos),
            _ => Err(PqwError::ReservedTrit(bits)),
        }
    }

    /// Битовое представление (0..=2).
    #[inline]
    pub fn as_bits(self) -> u8 {
        self as u8
    }

    /// Знак как множитель: −1.0 / 0.0 / +1.0.
    #[inline]
    pub fn sign(self) -> f64 {
        match self {
            Trit::Neg => -1.0,
            Trit::Zero => 0.0,
            Trit::Pos => 1.0,
        }
    }
}

impl fmt::Display for Trit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Trit::Neg => write!(f, "-1"),
            Trit::Zero => write!(f, "0"),
            Trit::Pos => write!(f, "+1"),
        }
    }
}

/// Один байт фазового блока: трит + кривизна.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PhaseByte(u8);

impl PhaseByte {
    /// Упаковка (трит, σ). σ маскируется до 6 бит.
    #[inline]
    pub fn encode(trit: Trit, sigma: u8) -> PhaseByte {
        debug_assert!(sigma <= SIGMA_MAX);
        PhaseByte(((sigma & SIGMA_MAX) << 2) | trit.as_bits())
    }

    /// Разбор произвольного байта; `0b11` в младших битах — ошибка.
    #[inline]
    pub fn from_raw(raw: u8) -> Result<PhaseByte> {
        Trit::from_bits(raw)?;
        Ok(PhaseByte(raw))
    }

    /// Конструктор для байтов, уже провалидированных читателем.
    #[inline]
    pub(crate) fn from_validated(raw: u8) -> PhaseByte {
        PhaseByte(raw)
    }

    /// Трит дуги.
    #[inline]
    pub fn trit(self) -> Trit {
        // 0b11 невозможно по построению: encode маскирует, from_raw отвергает.
        Trit::from_bits(self.0).unwrap_or(Trit::Zero)
    }

    /// Локальная деформация кривизны σ(t) ∈ [0, 63].
    #[inline]
    pub fn sigma(self) -> u8 {
        self.0 >> 2
    }

    /// Деквантованное значение p̂ ∈ [−1, 1]: `trit · σ/63`.
    #[inline]
    pub fn p(self) -> f64 {
        self.trit().sign() * f64::from(self.sigma()) / f64::from(SIGMA_MAX)
    }

    /// Фазовый угол θ̂ = arccos(p̂) ∈ [0, π] — готовый угол для Ry(arccos p).
    #[inline]
    pub fn theta(self) -> f64 {
        self.p().acos()
    }

    /// Сырой байт.
    #[inline]
    pub fn raw(self) -> u8 {
        self.0
    }
}

/// Граница ошибки квантования: |p̂ − p| ≤ 1/126 (полшага сетки).
pub const QUANT_EPS: f64 = 1.0 / 126.0;

/// Квантование p ∈ [−1, 1] в (трит, σ) — ближайший узел сетки `trit · k/63`.
///
/// Значения |p| ≤ 1/126 схлопываются в явный нуль. LENS-отсечение (не хранить
/// дугу вовсе) — отдельный, более грубый уровень: его выполняет писатель
/// по порогу `epsilon_threshold`.
///
/// Контракт: `p` конечно и лежит в [−1, 1] (проверяется на уровне писателя).
#[inline]
pub fn quantize(p: f32) -> PhaseByte {
    debug_assert!(
        p.is_finite() && (-1.0..=1.0).contains(&p),
        "caller must guarantee p in [-1, 1]"
    );
    let a = f64::from(p.abs());
    let sigma = (a * f64::from(SIGMA_MAX)).round() as u8;
    if sigma == 0 {
        PhaseByte::encode(Trit::Zero, 0)
    } else if p < 0.0 {
        PhaseByte::encode(Trit::Neg, sigma)
    } else {
        PhaseByte::encode(Trit::Pos, sigma)
    }
}

/// Кодировка фазовых блоков контейнера `.pqw`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TritEncoding {
    /// v1 (magic `POLER_QW`): 1 байт на дугу — трит в битах 0..1,
    /// 6-битная кривизна σ в битах 2..7. Точное значение `p̂ = trit·σ/63`.
    Curved,
    /// v2 (magic `POLER_Q2`): 4 трита на байт по 2 бита, кривизны нет —
    /// состояние живёт на самой тритовой решётке {−1, 0, +1}.
    ///
    /// Битовое отображение (спецификация v0.2): `0b00 → Zero`,
    /// `0b01 → Pos (+1)`, `0b10 → Neg (−1)`, `0b11 → Reserved`.
    /// Пары идут в LE-порядке: трит с номером `j` внутри байта занимает
    /// биты `2j..2j+1`; дуга `i` — пару `i mod 4` байта `i div 4`.
    ///
    /// Экономия RAM: `d = 65536 → 16 КиБ` (против 64 КиБ одних байтов v1).
    Packed4,
}

impl TritEncoding {
    /// Бит на дугу.
    pub fn bits_per_arc(self) -> usize {
        match self {
            TritEncoding::Curved => 8,
            TritEncoding::Packed4 => 2,
        }
    }

    /// Дуг на байт.
    pub fn arcs_per_byte(self) -> usize {
        match self {
            TritEncoding::Curved => 1,
            TritEncoding::Packed4 => 4,
        }
    }

    /// Имя кодировки для отчётов.
    pub fn name(self) -> &'static str {
        match self {
            TritEncoding::Curved => "curved",
            TritEncoding::Packed4 => "packed4",
        }
    }
}

/// 2-битный код трита в формате v2 (Packed4).
///
/// ВНИМАНИЕ: отображение отличается от дискриминантов [`Trit`]
/// (v1: `Neg = 0b00, Zero = 0b01, Pos = 0b10`) — здесь `Zero = 0b00`,
/// чтобы плотный массив фоновых монет кодировался нулевыми байтами.
#[inline]
pub fn pack_trit2(t: Trit) -> u8 {
    match t {
        Trit::Zero => 0b00,
        Trit::Pos => 0b01,
        Trit::Neg => 0b10,
    }
}

/// Обратное отображение v2; пара `0b11` зарезервирована.
#[inline]
pub fn unpack_trit2(bits: u8) -> Result<Trit> {
    match bits & 0b11 {
        0 => Ok(Trit::Zero),
        1 => Ok(Trit::Pos),
        2 => Ok(Trit::Neg),
        _ => Err(PqwError::ReservedTrit(bits)),
    }
}

/// Упаковка четырёх тритов в байт v2: `t[0]` — младшая пара.
#[inline]
pub fn pack_quad(ts: [Trit; 4]) -> u8 {
    pack_trit2(ts[0])
        | (pack_trit2(ts[1]) << 2)
        | (pack_trit2(ts[2]) << 4)
        | (pack_trit2(ts[3]) << 6)
}

/// Распаковка байта v2 в четыре трита; любая пара `0b11` — ошибка.
#[inline]
pub fn unpack_quad(b: u8) -> Result<[Trit; 4]> {
    Ok([
        unpack_trit2(b)?,
        unpack_trit2(b >> 2)?,
        unpack_trit2(b >> 4)?,
        unpack_trit2(b >> 6)?,
    ])
}

/// Трит по индексу дуги из плотного упакованного массива v2.
///
/// Контракт: `data.len() ≥ (i / 4) + 1` — гарантируется читателем
/// после валидации `phase_len == ceil(d_pol / 4)`.
#[inline]
pub(crate) fn packed_trit_at(data: &[u8], i: usize) -> Trit {
    let b = data[i / 4];
    let bits = (b >> (2 * (i % 4))) & 0b11;
    match bits {
        0 => Trit::Zero,
        1 => Trit::Pos,
        2 => Trit::Neg,
        _ => Trit::Zero, // unreachable: 0b11 отвергается при валидации
    }
}

/// Ближайший трит по `p` (правило упаковки v2): `|p| ≥ 0.5 → sign(p)`,
/// иначе `Zero`. Середина тритовой решётки — ровно 0.5
/// (v1-эквивалент: σ = 31.5/63).
#[inline]
pub fn nearest_trit(p: f32) -> Trit {
    if p >= 0.5 {
        Trit::Pos
    } else if p <= -0.5 {
        Trit::Neg
    } else {
        Trit::Zero
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_exact_bounds() {
        assert_eq!(quantize(0.0), PhaseByte::encode(Trit::Zero, 0));
        assert_eq!(quantize(-0.0), PhaseByte::encode(Trit::Zero, 0));
        assert_eq!(quantize(1.0), PhaseByte::encode(Trit::Pos, 63));
        assert_eq!(quantize(-1.0), PhaseByte::encode(Trit::Neg, 63));
        assert_eq!(quantize(1.0).p(), 1.0);
        assert_eq!(quantize(-1.0).p(), -1.0);
    }

    #[test]
    fn quantize_rounding() {
        // 0.5 * 63 = 31.5 -> round half away from zero -> 32
        assert_eq!(quantize(0.5).sigma(), 32);
        assert_eq!(quantize(-0.5).sigma(), 32);
        assert_eq!(quantize(-0.5).trit(), Trit::Neg);
        // |p| * 63 < 0.5 -> явный нуль
        assert_eq!(quantize(0.007).trit(), Trit::Zero);
        assert_eq!(quantize(0.007).sigma(), 0);
        // 0.008 * 63 = 0.504 -> sigma = 1
        assert_eq!(quantize(0.008).sigma(), 1);
        assert_eq!(quantize(0.008).trit(), Trit::Pos);
    }

    #[test]
    fn raw_bit_layout() {
        // Спецификация: биты 0..1 = трит {-1, 0, +1}, биты 2..7 = σ.
        assert_eq!(PhaseByte::encode(Trit::Pos, 32).raw(), 130); // 10_0000_10
        assert_eq!(PhaseByte::encode(Trit::Neg, 63).raw(), 252); // 1111_1100
        assert_eq!(PhaseByte::encode(Trit::Zero, 0).raw(), 1); //  0000_0001
        assert_eq!(PhaseByte::encode(Trit::Neg, 1).raw(), 0b0000_0100);
        assert_eq!(PhaseByte::encode(Trit::Pos, 63).raw(), 0b1111_1110);
    }

    #[test]
    fn from_raw_rejects_reserved() {
        assert!(PhaseByte::from_raw(0b0000_0011).is_err());
        assert!(PhaseByte::from_raw(0b1111_1111).is_err());
        assert!(PhaseByte::from_raw(0b0000_0111).is_err()); // σ=1 + зарезервированный трит
        for raw in [0u8, 1, 2, 4, 130, 252, 254] {
            assert!(PhaseByte::from_raw(raw).is_ok());
        }
    }

    #[test]
    fn p_and_theta() {
        let b = PhaseByte::encode(Trit::Pos, 63);
        assert_eq!(b.p(), 1.0);
        assert!(b.theta().abs() < 1e-12);

        let b = PhaseByte::encode(Trit::Neg, 63);
        assert_eq!(b.p(), -1.0);
        assert!((b.theta() - std::f64::consts::PI).abs() < 1e-12);

        let b = PhaseByte::encode(Trit::Zero, 0);
        assert_eq!(b.p(), 0.0);
        assert!((b.theta() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);

        let b = PhaseByte::encode(Trit::Pos, 32);
        assert!((b.p() - 32.0 / 63.0).abs() < 1e-15);

        // Трит Zero доминирует над σ: p = 0 даже при σ > 0.
        assert_eq!(PhaseByte::encode(Trit::Zero, 40).p(), 0.0);
    }

    #[test]
    fn theta_is_arccos_of_p() {
        for raw in [0u8, 2, 130, 252, 254, 16, 200] {
            let b = PhaseByte::from_raw(raw).unwrap();
            assert!((b.theta() - b.p().acos()).abs() < 1e-15);
        }
    }

    #[test]
    fn v2_bit_mapping_matches_spec() {
        // Спецификация v0.2: 0b00 → Zero, 0b01 → +1, 0b10 → −1, 0b11 → Reserved.
        assert_eq!(pack_trit2(Trit::Zero), 0b00);
        assert_eq!(pack_trit2(Trit::Pos), 0b01);
        assert_eq!(pack_trit2(Trit::Neg), 0b10);
        assert_eq!(unpack_trit2(0b00).unwrap(), Trit::Zero);
        assert_eq!(unpack_trit2(0b01).unwrap(), Trit::Pos);
        assert_eq!(unpack_trit2(0b10).unwrap(), Trit::Neg);
        assert!(matches!(unpack_trit2(0b11), Err(PqwError::ReservedTrit(_))));
        // Старшие биты маскируются: значение определяется парой 0..1.
        assert_eq!(unpack_trit2(0b1111_0110).unwrap(), Trit::Neg);
    }

    #[test]
    fn v2_quad_roundtrip() {
        let quads = [
            [Trit::Zero, Trit::Zero, Trit::Zero, Trit::Zero],
            [Trit::Pos, Trit::Neg, Trit::Zero, Trit::Pos],
            [Trit::Neg, Trit::Neg, Trit::Pos, Trit::Pos],
        ];
        for q in quads {
            let b = pack_quad(q);
            assert_eq!(unpack_quad(b).unwrap(), q);
        }
        // Все нули — нулевой байт: плотный фон кодируется нулями.
        assert_eq!(pack_quad([Trit::Zero; 4]), 0);
        // t[0] — младшая пара: [Pos,0,0,0] = 0b00000001.
        assert_eq!(
            pack_quad([Trit::Pos, Trit::Zero, Trit::Zero, Trit::Zero]),
            1
        );
        // t[3] — старшая пара: [0,0,0,Neg] = 0b10_000000 = 0x80.
        assert_eq!(
            pack_quad([Trit::Zero, Trit::Zero, Trit::Zero, Trit::Neg]),
            0x80
        );
    }

    #[test]
    fn v2_packed_trit_at_le_pair_order() {
        // Плотный массив: дуга i лежит в паре i mod 4 байта i div 4.
        let data = [pack_quad([Trit::Pos, Trit::Neg, Trit::Zero, Trit::Pos])];
        assert_eq!(packed_trit_at(&data, 0), Trit::Pos);
        assert_eq!(packed_trit_at(&data, 1), Trit::Neg);
        assert_eq!(packed_trit_at(&data, 2), Trit::Zero);
        assert_eq!(packed_trit_at(&data, 3), Trit::Pos);
        let data2 = [
            0u8,
            pack_quad([Trit::Neg, Trit::Zero, Trit::Zero, Trit::Zero]),
        ];
        assert_eq!(packed_trit_at(&data2, 4), Trit::Neg);
        assert_eq!(packed_trit_at(&data2, 5), Trit::Zero);
    }

    #[test]
    fn nearest_trit_thresholds() {
        assert_eq!(nearest_trit(0.5), Trit::Pos);
        assert_eq!(nearest_trit(0.49), Trit::Zero);
        assert_eq!(nearest_trit(-0.5), Trit::Neg);
        assert_eq!(nearest_trit(-0.49), Trit::Zero);
        assert_eq!(nearest_trit(0.0), Trit::Zero);
        assert_eq!(nearest_trit(1.0), Trit::Pos);
        assert_eq!(nearest_trit(-1.0), Trit::Neg);
        assert_eq!(nearest_trit(0.7), Trit::Pos);
        assert_eq!(nearest_trit(-0.9), Trit::Neg);
    }

    #[test]
    fn encoding_dimensions() {
        assert_eq!(TritEncoding::Curved.bits_per_arc(), 8);
        assert_eq!(TritEncoding::Curved.arcs_per_byte(), 1);
        assert_eq!(TritEncoding::Packed4.bits_per_arc(), 2);
        assert_eq!(TritEncoding::Packed4.arcs_per_byte(), 4);
        assert_eq!(TritEncoding::Curved.name(), "curved");
        assert_eq!(TritEncoding::Packed4.name(), "packed4");
    }
}
