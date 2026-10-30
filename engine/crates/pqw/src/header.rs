//! Фиксированный заголовок (0x00..0x80, 128 байт) — контракт формата v1.
//!
//! ```text
//! 0x00  8   magic "POLER_QW"            0x40  8   topology_offset
//! 0x08  4   format_version (u32)        0x48  8   topology_len
//! 0x0C  4   d_pol (u32)                 0x50  8   phase_offset
//! 0x10  4   eta (f32)                   0x58  8   phase_len
//! 0x14  4   gamma (f32)                 0x60  8   nnz
//! 0x18  4   rho (f32)                   0x68  8   flags
//! 0x1C  4   epsilon_threshold (f32)     0x70  8   reserved (0)
//! 0x20  8   mcweeny_residual (f64)      0x78  8   header checksum (FNV-1a64)
//! 0x28  24  payload digest (SHA-256→24)
//! ```
//!
//! Все числа — little-endian.

use crate::checksum::fnv1a64;
use crate::error::{PqwError, Result};
use crate::phase::TritEncoding;

/// Магические байты контейнера v1 (кривизна, 1 байт на дугу).
pub const MAGIC: [u8; 8] = *b"POLER_QW";
/// Магические байты контейнера v2 (упакованные триты, 4 дуги на байт).
pub const MAGIC_V2: [u8; 8] = *b"POLER_Q2";
/// Магические байты контейнера v3 (Packed4 + гироскопная топология
/// `J = A − Aᵀ` в топологической секции).
pub const MAGIC_V3: [u8; 8] = *b"POLER_Q3";
/// Магические байты контейнера v4 (v3 + секция лексикона `LEXI`:
/// координата → доминантный токен, обратная карта кодировщика).
pub const MAGIC_V4: [u8; 8] = *b"POLER_Q4";
/// Магические байты контейнера v5 (v4 + секция контекст-рефлекса
/// `REFL`: динамический след диалога — нить разговора переживает
/// рестарт, RQ23).
pub const MAGIC_V5: [u8; 8] = *b"POLER_Q5";
/// Версия формата v1, поддерживаемая этой сборкой.
pub const FORMAT_VERSION: u32 = 1;
/// Версия формата v2 (Packed4, magic `POLER_Q2`).
pub const FORMAT_VERSION_V2: u32 = 2;
/// Версия формата v3 (Packed4 + гироскоп, magic `POLER_Q3`).
pub const FORMAT_VERSION_V3: u32 = 3;
/// Версия формата v4 (v3 + лексикон, magic `POLER_Q4`).
pub const FORMAT_VERSION_V4: u32 = 4;
/// Версия формата v5 (v4 + контекст-рефлекс, magic `POLER_Q5`).
pub const FORMAT_VERSION_V5: u32 = 5;
/// Размер фиксированного заголовка.
pub const HEADER_SIZE: usize = 0x80;

// Точные смещения полей.
pub const OFF_VERSION: usize = 0x08;
pub const OFF_D_POL: usize = 0x0C;
pub const OFF_ETA: usize = 0x10;
pub const OFF_GAMMA: usize = 0x14;
pub const OFF_RHO: usize = 0x18;
pub const OFF_EPS: usize = 0x1C;
pub const OFF_MCWEENY_RESIDUAL: usize = 0x20;
pub const OFF_DIGEST: usize = 0x28;
pub const DIGEST_LEN: usize = 24;
pub const OFF_TOPOLOGY_OFFSET: usize = 0x40;
pub const OFF_TOPOLOGY_LEN: usize = 0x48;
pub const OFF_PHASE_OFFSET: usize = 0x50;
pub const OFF_PHASE_LEN: usize = 0x58;
pub const OFF_NNZ: usize = 0x60;
pub const OFF_FLAGS: usize = 0x68;
pub const OFF_RESERVED: usize = 0x70;
pub const OFF_CHECKSUM: usize = 0x78;

/// Гиперпараметры фазового потока `dp/dt = −η·Π_Λ(∇F − γ∇ε)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HyperParams {
    /// Шаг потока (learning rate).
    pub eta: f32,
    /// Трение потока.
    pub gamma: f32,
    /// Затухание IIR-резонатора `R_t = ε_t + ρ·R_{t−1}`.
    pub rho: f32,
    /// Порог ε-значимости: LENS отсекает дуги с |p| < ε.
    pub epsilon_threshold: f32,
}

impl Default for HyperParams {
    fn default() -> Self {
        HyperParams {
            eta: 0.01,
            gamma: 0.1,
            rho: 0.99,
            epsilon_threshold: 0.05,
        }
    }
}

impl HyperParams {
    /// Все четыре значения конечны.
    pub fn is_finite(&self) -> bool {
        self.eta.is_finite()
            && self.gamma.is_finite()
            && self.rho.is_finite()
            && self.epsilon_threshold.is_finite()
    }
}

/// Флаги формата: v1 определяет два младших бита, остальные обязаны быть нулями.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flags(u64);

impl Flags {
    /// Пустые флаги — обязательное значение для контейнеров v2:
    /// ни топологии (INDEX16 бессмыслен), ни кривизны (CURVATURE запрещён)
    /// в упакованном режиме нет.
    pub const fn none() -> Flags {
        Flags(0)
    }

    /// Топология хранит u16-индексы (допустимо при `d_pol ≤ 65536`).
    /// v3: то же для индексов пар гироскопа.
    pub const INDEX16: u64 = 1 << 0;
    /// Фазовые блоки содержат 6-битную кривизну. v1: бит обязан быть установлен.
    pub const CURVATURE: u64 = 1 << 1;
    /// v3: топологическая секция несёт гироскоп `J = A − Aᵀ` (RQ10).
    pub const GYRO: u64 = 1 << 2;
    /// v4: за гироскопной секцией следует лексикон `LEXI` — обратная
    /// карта кодировщика (координата → доминантный токен, RQ17).
    pub const LEXICON: u64 = 1 << 3;
    /// v5: за лексиконом следует контекст-рефлекс `REFL` — динамический
    /// след диалога (кольцо гироскопа + имя собеседника, RQ23).
    pub const REFL: u64 = 1 << 4;

    /// Флаги для записи: CURVATURE всегда; INDEX16 по выбору писателя.
    pub fn new(index16: bool) -> Flags {
        Flags(Self::CURVATURE | if index16 { Self::INDEX16 } else { 0 })
    }

    /// u16-топология?
    pub fn index16(self) -> bool {
        self.0 & Self::INDEX16 != 0
    }

    /// Кривизна присутствует?
    pub fn curvature(self) -> bool {
        self.0 & Self::CURVATURE != 0
    }

    /// Ширина индекса топологии в байтах (2 или 4).
    pub fn index_width(self) -> usize {
        if self.index16() {
            2
        } else {
            4
        }
    }

    /// Проверка зарезервированных битов и обязательного бита CURVATURE (v1).
    pub fn validate(v: u64) -> Result<Flags> {
        if v & !(Self::INDEX16 | Self::CURVATURE) != 0 {
            return Err(PqwError::ReservedBits { value: v });
        }
        if v & Self::CURVATURE == 0 {
            return Err(PqwError::UnsupportedFlags(v));
        }
        Ok(Flags(v))
    }

    /// Проверка флагов контейнера v2: все биты обязаны быть нулями —
    /// топологии нет (INDEX16 бессмыслен), кривизны нет (CURVATURE запрещён).
    pub fn validate_v2(v: u64) -> Result<Flags> {
        if v != 0 {
            return Err(PqwError::ReservedBits { value: v });
        }
        Ok(Flags(0))
    }

    /// Флаги для записи v3: GYRO всегда; INDEX16 — по выбору писателя
    /// (u16-индексы пар гироскопа при `d_pol ≤ 65536`).
    pub fn v3(index16: bool) -> Flags {
        Flags(Self::GYRO | if index16 { Self::INDEX16 } else { 0 })
    }

    /// Флаги для записи v4: GYRO + LEXICON; INDEX16 — по выбору писателя.
    pub fn v4(index16: bool) -> Flags {
        Flags(Self::v3(index16).0 | Self::LEXICON)
    }

    /// Флаги для записи v5: GYRO + LEXICON + REFL; INDEX16 — по выбору
    /// писателя (ширина координат пар гироскопа, лексикона и следа).
    pub fn v5(index16: bool) -> Flags {
        Flags(Self::v4(index16).0 | Self::REFL)
    }

    /// Гироскопная топология присутствует? (v3)
    pub fn gyro(self) -> bool {
        self.0 & Self::GYRO != 0
    }

    /// Секция лексикона присутствует? (v4)
    pub fn lexicon(self) -> bool {
        self.0 & Self::LEXICON != 0
    }

    /// Секция контекст-рефлекса присутствует? (v5)
    pub fn reflex(self) -> bool {
        self.0 & Self::REFL != 0
    }

    /// Проверка флагов контейнера v3: GYRO обязан быть установлен,
    /// допускается INDEX16 (ширина индексов пар гироскопа), кривизны нет.
    pub fn validate_v3(v: u64) -> Result<Flags> {
        if v & !(Self::INDEX16 | Self::GYRO) != 0 {
            return Err(PqwError::ReservedBits { value: v });
        }
        if v & Self::GYRO == 0 {
            return Err(PqwError::UnsupportedFlags(v));
        }
        Ok(Flags(v))
    }

    /// Проверка флагов контейнера v4: GYRO и LEXICON обязаны быть
    /// установлены, допускается INDEX16 (ширина индексов пар гироскопа
    /// и координат лексикона). CURVATURE и прочие биты запрещены.
    pub fn validate_v4(v: u64) -> Result<Flags> {
        if v & !(Self::INDEX16 | Self::GYRO | Self::LEXICON) != 0 {
            return Err(PqwError::ReservedBits { value: v });
        }
        if v & (Self::GYRO | Self::LEXICON) != Self::GYRO | Self::LEXICON {
            return Err(PqwError::UnsupportedFlags(v));
        }
        Ok(Flags(v))
    }

    /// Проверка флагов контейнера v5: GYRO, LEXICON и REFL обязаны быть
    /// установлены, допускается INDEX16 (ширина координат пар гироскопа,
    /// лексикона и следа рефлекса). Прочие биты запрещены.
    pub fn validate_v5(v: u64) -> Result<Flags> {
        if v & !(Self::INDEX16 | Self::GYRO | Self::LEXICON | Self::REFL) != 0 {
            return Err(PqwError::ReservedBits { value: v });
        }
        let need = Self::GYRO | Self::LEXICON | Self::REFL;
        if v & need != need {
            return Err(PqwError::UnsupportedFlags(v));
        }
        Ok(Flags(v))
    }

    /// Сырые биты.
    pub fn bits(self) -> u64 {
        self.0
    }
}

/// Полностью разобранный фиксированный заголовок.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub format_version: u32,
    pub d_pol: u32,
    pub hyper: HyperParams,
    /// max `|λ² − λ|` по хранимым дугам — отклонение от P² = P на момент записи.
    pub mcweeny_residual: f64,
    /// Первые 24 байта SHA-256 от payload (топология + фазовые блоки).
    pub payload_digest: [u8; DIGEST_LEN],
    pub topology_offset: u64,
    pub topology_len: u64,
    pub phase_offset: u64,
    pub phase_len: u64,
    pub nnz: u64,
    pub flags: Flags,
}

#[inline]
fn put_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

#[inline]
fn get_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
}

#[inline]
fn put_u64(buf: &mut [u8], off: usize, v: u64) {
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

#[inline]
fn get_u64(buf: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(buf[off..off + 8].try_into().unwrap())
}

#[inline]
fn put_f32(buf: &mut [u8], off: usize, v: f32) {
    put_u32(buf, off, v.to_bits());
}

#[inline]
fn get_f32(buf: &[u8], off: usize) -> f32 {
    f32::from_bits(get_u32(buf, off))
}

#[inline]
fn put_f64(buf: &mut [u8], off: usize, v: f64) {
    put_u64(buf, off, v.to_bits());
}

#[inline]
fn get_f64(buf: &[u8], off: usize) -> f64 {
    f64::from_bits(get_u64(buf, off))
}

impl Header {
    /// Кодировка фазовых блоков этого контейнера
    /// (v1 → [`TritEncoding::Curved`], v2/v3 → [`TritEncoding::Packed4`]).
    pub fn encoding(&self) -> TritEncoding {
        if self.format_version >= FORMAT_VERSION_V2 {
            TritEncoding::Packed4
        } else {
            TritEncoding::Curved
        }
    }

    /// Это контейнер с упакованными тритами (v2 или v3)?
    pub fn is_packed(&self) -> bool {
        self.format_version >= FORMAT_VERSION_V2
    }

    /// Это контейнер v3 с гироскопной топологией `J = A − Aᵀ`?
    /// (v4/v5 также содержат гироскоп — ответ `true` для них тоже.)
    pub fn is_gyro(&self) -> bool {
        matches!(
            self.format_version,
            FORMAT_VERSION_V3 | FORMAT_VERSION_V4 | FORMAT_VERSION_V5
        )
    }

    /// Это контейнер v4+ с секцией лексикона `LEXI`?
    pub fn is_lexicon(&self) -> bool {
        self.format_version >= FORMAT_VERSION_V4
    }

    /// Это контейнер v5 с секцией контекст-рефлекса `REFL`?
    pub fn is_reflex(&self) -> bool {
        self.format_version == FORMAT_VERSION_V5
    }

    /// Сериализация в 128 байт; checksum вычисляется последним.
    /// Magic выбирается по версии: v1 → `POLER_QW`, v2 → `POLER_Q2`,
    /// v3 → `POLER_Q3`, v4 → `POLER_Q4`, v5 → `POLER_Q5`.
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut b = [0u8; HEADER_SIZE];
        let magic = match self.format_version {
            FORMAT_VERSION_V2 => MAGIC_V2,
            FORMAT_VERSION_V3 => MAGIC_V3,
            FORMAT_VERSION_V4 => MAGIC_V4,
            FORMAT_VERSION_V5 => MAGIC_V5,
            _ => MAGIC,
        };
        b[..8].copy_from_slice(&magic);
        put_u32(&mut b, OFF_VERSION, self.format_version);
        put_u32(&mut b, OFF_D_POL, self.d_pol);
        put_f32(&mut b, OFF_ETA, self.hyper.eta);
        put_f32(&mut b, OFF_GAMMA, self.hyper.gamma);
        put_f32(&mut b, OFF_RHO, self.hyper.rho);
        put_f32(&mut b, OFF_EPS, self.hyper.epsilon_threshold);
        put_f64(&mut b, OFF_MCWEENY_RESIDUAL, self.mcweeny_residual);
        b[OFF_DIGEST..OFF_DIGEST + DIGEST_LEN].copy_from_slice(&self.payload_digest);
        put_u64(&mut b, OFF_TOPOLOGY_OFFSET, self.topology_offset);
        put_u64(&mut b, OFF_TOPOLOGY_LEN, self.topology_len);
        put_u64(&mut b, OFF_PHASE_OFFSET, self.phase_offset);
        put_u64(&mut b, OFF_PHASE_LEN, self.phase_len);
        put_u64(&mut b, OFF_NNZ, self.nnz);
        put_u64(&mut b, OFF_FLAGS, self.flags.bits());
        // OFF_RESERVED остаётся нулевым
        let checksum = fnv1a64(&b[..OFF_CHECKSUM]);
        put_u64(&mut b, OFF_CHECKSUM, checksum);
        b
    }

    /// Разбор и проверка: magic → version → checksum → flags/reserved → d_pol.
    ///
    /// Поддержаны все пять поколений: `POLER_QW` (v1, кривизна),
    /// `POLER_Q2` (v2, упакованные триты), `POLER_Q3` (v3, гироскоп),
    /// `POLER_Q4` (v4, гироскоп + лексикон) и `POLER_Q5` (v5, гироскоп
    /// + лексикон + контекст-рефлекс); пара magic ↔ версия
    /// перекрёстно проверяется. В v3/v4/v5 reserved-слово (0x70) хранит
    /// счётчик тактов гироскопа — проверка «reserved = 0» для них
    /// отключена.
    pub fn from_bytes(data: &[u8]) -> Result<Header> {
        if data.len() < HEADER_SIZE {
            return Err(PqwError::Truncated {
                need: HEADER_SIZE,
                have: data.len(),
            });
        }
        let mut magic = [0u8; 8];
        magic.copy_from_slice(&data[..8]);
        if magic != MAGIC
            && magic != MAGIC_V2
            && magic != MAGIC_V3
            && magic != MAGIC_V4
            && magic != MAGIC_V5
        {
            return Err(PqwError::BadMagic(magic));
        }
        let version = get_u32(data, OFF_VERSION);
        let expected = match magic {
            MAGIC => FORMAT_VERSION,
            MAGIC_V2 => FORMAT_VERSION_V2,
            MAGIC_V3 => FORMAT_VERSION_V3,
            MAGIC_V4 => FORMAT_VERSION_V4,
            _ => FORMAT_VERSION_V5,
        };
        if version != expected {
            return Err(PqwError::UnsupportedVersion(version));
        }
        let expected = get_u64(data, OFF_CHECKSUM);
        let actual = fnv1a64(&data[..OFF_CHECKSUM]);
        if expected != actual {
            return Err(PqwError::CorruptHeader { expected, actual });
        }
        let flags = match version {
            FORMAT_VERSION_V2 => Flags::validate_v2(get_u64(data, OFF_FLAGS))?,
            FORMAT_VERSION_V3 => Flags::validate_v3(get_u64(data, OFF_FLAGS))?,
            FORMAT_VERSION_V4 => Flags::validate_v4(get_u64(data, OFF_FLAGS))?,
            FORMAT_VERSION_V5 => Flags::validate_v5(get_u64(data, OFF_FLAGS))?,
            _ => Flags::validate(get_u64(data, OFF_FLAGS))?,
        };
        let reserved = get_u64(data, OFF_RESERVED);
        if version != FORMAT_VERSION_V3
            && version != FORMAT_VERSION_V4
            && version != FORMAT_VERSION_V5
            && reserved != 0
        {
            return Err(PqwError::ReservedBits { value: reserved });
        }
        let d_pol = get_u32(data, OFF_D_POL);
        if d_pol == 0 {
            return Err(PqwError::BadDimension(d_pol));
        }
        let mut payload_digest = [0u8; DIGEST_LEN];
        payload_digest.copy_from_slice(&data[OFF_DIGEST..OFF_DIGEST + DIGEST_LEN]);
        Ok(Header {
            format_version: version,
            d_pol,
            hyper: HyperParams {
                eta: get_f32(data, OFF_ETA),
                gamma: get_f32(data, OFF_GAMMA),
                rho: get_f32(data, OFF_RHO),
                epsilon_threshold: get_f32(data, OFF_EPS),
            },
            mcweeny_residual: get_f64(data, OFF_MCWEENY_RESIDUAL),
            payload_digest,
            topology_offset: get_u64(data, OFF_TOPOLOGY_OFFSET),
            topology_len: get_u64(data, OFF_TOPOLOGY_LEN),
            phase_offset: get_u64(data, OFF_PHASE_OFFSET),
            phase_len: get_u64(data, OFF_PHASE_LEN),
            nnz: get_u64(data, OFF_NNZ),
            flags,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Header {
        Header {
            format_version: FORMAT_VERSION,
            d_pol: 8,
            hyper: HyperParams {
                eta: 0.5,
                gamma: 0.25,
                rho: 0.75,
                epsilon_threshold: 0.125,
            },
            mcweeny_residual: 0.25,
            payload_digest: [7u8; 24],
            topology_offset: 0x80,
            topology_len: 6,
            phase_offset: 0x86,
            phase_len: 3,
            nnz: 3,
            flags: Flags::new(true),
        }
    }

    #[test]
    fn header_roundtrip() {
        let h = sample();
        let b = h.to_bytes();
        assert_eq!(b.len(), HEADER_SIZE);
        let h2 = Header::from_bytes(&b).unwrap();
        assert_eq!(h2, h);
    }

    #[test]
    fn checksum_detects_mutation() {
        let mut b = sample().to_bytes();
        b[0x0C] ^= 0x01; // d_pol без пересчёта checksum
        assert!(matches!(
            Header::from_bytes(&b),
            Err(PqwError::CorruptHeader { .. })
        ));
    }

    #[test]
    fn bad_magic_rejected() {
        let mut b = sample().to_bytes();
        b[0] = b'X';
        assert!(matches!(Header::from_bytes(&b), Err(PqwError::BadMagic(_))));
    }

    #[test]
    fn short_buffer_rejected() {
        let b = sample().to_bytes();
        assert!(matches!(
            Header::from_bytes(&b[..127]),
            Err(PqwError::Truncated { .. })
        ));
    }

    fn sample_v2() -> Header {
        Header {
            format_version: FORMAT_VERSION_V2,
            d_pol: 9,
            hyper: HyperParams {
                eta: 0.25,
                gamma: 0.5,
                rho: 0.99,
                epsilon_threshold: 0.05,
            },
            mcweeny_residual: 0.0,
            payload_digest: [3u8; 24],
            topology_offset: HEADER_SIZE as u64,
            topology_len: 0,
            phase_offset: HEADER_SIZE as u64,
            phase_len: 3, // ceil(9 / 4) = 3
            nnz: 4,       // число ненулевых тритов
            flags: Flags::none(),
        }
    }

    #[test]
    fn v2_header_roundtrip_and_magic() {
        let h = sample_v2();
        let b = h.to_bytes();
        // Magic POLER_Q2 = 0x504F4C45525F5132.
        assert_eq!(&b[..8], b"POLER_Q2");
        let h2 = Header::from_bytes(&b).unwrap();
        assert_eq!(h2, h);
        assert!(h2.is_packed());
        assert_eq!(h2.encoding(), TritEncoding::Packed4);
    }

    #[test]
    fn v2_rejects_any_flags() {
        let mut b = sample_v2().to_bytes();
        // flags + 1 → ReservedBits; пересчитаем checksum, чтобы дошла проверка flags.
        b[OFF_FLAGS] = 0x01;
        let checksum = fnv1a64(&b[..OFF_CHECKSUM]);
        b[OFF_CHECKSUM..OFF_CHECKSUM + 8].copy_from_slice(&checksum.to_le_bytes());
        assert!(matches!(
            Header::from_bytes(&b),
            Err(PqwError::ReservedBits { .. })
        ));
    }

    #[test]
    fn magic_version_cross_check() {
        // POLER_Q2 с version = 1 — рассинхрон пары.
        let mut b = sample_v2().to_bytes();
        b[OFF_VERSION..OFF_VERSION + 4].copy_from_slice(&1u32.to_le_bytes());
        let checksum = fnv1a64(&b[..OFF_CHECKSUM]);
        b[OFF_CHECKSUM..OFF_CHECKSUM + 8].copy_from_slice(&checksum.to_le_bytes());
        assert!(matches!(
            Header::from_bytes(&b),
            Err(PqwError::UnsupportedVersion(1))
        ));
    }

    fn sample_v3() -> Header {
        Header {
            format_version: FORMAT_VERSION_V3,
            d_pol: 512,
            hyper: HyperParams {
                eta: 0.25,
                gamma: 0.5,
                rho: 0.99,
                epsilon_threshold: 0.05,
            },
            mcweeny_residual: 0.0,
            payload_digest: [9u8; 24],
            topology_offset: (HEADER_SIZE + 128) as u64,
            topology_len: 47, // 32 служебных + 3 пары × 5 B
            phase_offset: HEADER_SIZE as u64,
            phase_len: 128,
            nnz: 64,
            flags: Flags::v3(true),
        }
    }

    #[test]
    fn v3_header_roundtrip_and_magic() {
        let h = sample_v3();
        let b = h.to_bytes();
        assert_eq!(&b[..8], b"POLER_Q3");
        let h2 = Header::from_bytes(&b).unwrap();
        assert_eq!(h2, h);
        assert!(h2.is_gyro());
        assert!(h2.is_packed());
        assert_eq!(h2.encoding(), TritEncoding::Packed4);
        assert!(h2.flags.gyro());
        assert!(h2.flags.index16());
    }

    #[test]
    fn v3_reserved_holds_ticks() {
        // В v3 reserved-слово = счётчик тактов: ненулевое значение легально.
        let mut b = sample_v3().to_bytes();
        b[OFF_RESERVED..OFF_RESERVED + 8].copy_from_slice(&123_456u64.to_le_bytes());
        let checksum = fnv1a64(&b[..OFF_CHECKSUM]);
        b[OFF_CHECKSUM..OFF_CHECKSUM + 8].copy_from_slice(&checksum.to_le_bytes());
        assert!(Header::from_bytes(&b).is_ok());
    }

    #[test]
    fn v3_requires_gyro_flag() {
        // GYRO сброшен → UnsupportedFlags (после пересчёта checksum).
        let mut b = sample_v3().to_bytes();
        b[OFF_FLAGS..OFF_FLAGS + 8].copy_from_slice(&0u64.to_le_bytes());
        let checksum = fnv1a64(&b[..OFF_CHECKSUM]);
        b[OFF_CHECKSUM..OFF_CHECKSUM + 8].copy_from_slice(&checksum.to_le_bytes());
        assert!(matches!(
            Header::from_bytes(&b),
            Err(PqwError::UnsupportedFlags(_))
        ));
    }

    #[test]
    fn v3_rejects_curvature_and_stray_bits() {
        for bad in [Flags::CURVATURE, 1 << 3, Flags::GYRO | (1 << 9)] {
            let mut b = sample_v3().to_bytes();
            b[OFF_FLAGS..OFF_FLAGS + 8].copy_from_slice(&bad.to_le_bytes());
            let checksum = fnv1a64(&b[..OFF_CHECKSUM]);
            b[OFF_CHECKSUM..OFF_CHECKSUM + 8].copy_from_slice(&checksum.to_le_bytes());
            assert!(Header::from_bytes(&b).is_err(), "flags {bad}");
        }
    }

    #[test]
    fn v3_magic_version_cross_check() {
        // POLER_Q3 с version = 2 — рассинхрон пары.
        let mut b = sample_v3().to_bytes();
        b[OFF_VERSION..OFF_VERSION + 4].copy_from_slice(&2u32.to_le_bytes());
        let checksum = fnv1a64(&b[..OFF_CHECKSUM]);
        b[OFF_CHECKSUM..OFF_CHECKSUM + 8].copy_from_slice(&checksum.to_le_bytes());
        assert!(matches!(
            Header::from_bytes(&b),
            Err(PqwError::UnsupportedVersion(2))
        ));
    }

    fn sample_v5() -> Header {
        Header {
            format_version: FORMAT_VERSION_V5,
            d_pol: 512,
            hyper: HyperParams {
                eta: 0.25,
                gamma: 0.5,
                rho: 0.99,
                epsilon_threshold: 0.05,
            },
            mcweeny_residual: 0.0,
            payload_digest: [11u8; 24],
            topology_offset: (HEADER_SIZE + 128) as u64,
            topology_len: 47,
            phase_offset: HEADER_SIZE as u64,
            phase_len: 128,
            nnz: 64,
            flags: Flags::v5(true),
        }
    }

    #[test]
    fn v5_header_roundtrip_and_magic() {
        let h = sample_v5();
        let b = h.to_bytes();
        assert_eq!(&b[..8], b"POLER_Q5");
        let h2 = Header::from_bytes(&b).unwrap();
        assert_eq!(h2, h);
        assert!(h2.is_reflex());
        assert!(h2.is_lexicon());
        assert!(h2.is_gyro());
        assert!(h2.flags.reflex());
        assert!(h2.flags.lexicon());
        assert!(h2.flags.gyro());
        assert!(h2.flags.index16());
    }

    #[test]
    fn v5_reserved_holds_ticks() {
        // В v5 reserved-слово = счётчик тактов: ненулевое значение легально.
        let mut b = sample_v5().to_bytes();
        b[OFF_RESERVED..OFF_RESERVED + 8].copy_from_slice(&987_654u64.to_le_bytes());
        let checksum = fnv1a64(&b[..OFF_CHECKSUM]);
        b[OFF_CHECKSUM..OFF_CHECKSUM + 8].copy_from_slice(&checksum.to_le_bytes());
        assert!(Header::from_bytes(&b).is_ok());
    }

    #[test]
    fn v5_requires_refl_flag() {
        // REFL сброшен → UnsupportedFlags (после пересчёта checksum).
        let mut b = sample_v5().to_bytes();
        let without_refl = Flags::v4(true).bits();
        b[OFF_FLAGS..OFF_FLAGS + 8].copy_from_slice(&without_refl.to_le_bytes());
        let checksum = fnv1a64(&b[..OFF_CHECKSUM]);
        b[OFF_CHECKSUM..OFF_CHECKSUM + 8].copy_from_slice(&checksum.to_le_bytes());
        assert!(matches!(
            Header::from_bytes(&b),
            Err(PqwError::UnsupportedFlags(_))
        ));
    }

    #[test]
    fn v5_rejects_stray_bits() {
        for bad in [Flags::CURVATURE, 1 << 5, Flags::v5(false).bits() | (1 << 9)] {
            let mut b = sample_v5().to_bytes();
            b[OFF_FLAGS..OFF_FLAGS + 8].copy_from_slice(&bad.to_le_bytes());
            let checksum = fnv1a64(&b[..OFF_CHECKSUM]);
            b[OFF_CHECKSUM..OFF_CHECKSUM + 8].copy_from_slice(&checksum.to_le_bytes());
            assert!(Header::from_bytes(&b).is_err(), "flags {bad}");
        }
    }

    #[test]
    fn v5_magic_version_cross_check() {
        // POLER_Q5 с version = 4 — рассинхрон пары.
        let mut b = sample_v5().to_bytes();
        b[OFF_VERSION..OFF_VERSION + 4].copy_from_slice(&4u32.to_le_bytes());
        let checksum = fnv1a64(&b[..OFF_CHECKSUM]);
        b[OFF_CHECKSUM..OFF_CHECKSUM + 8].copy_from_slice(&checksum.to_le_bytes());
        assert!(matches!(
            Header::from_bytes(&b),
            Err(PqwError::UnsupportedVersion(4))
        ));
    }
}
