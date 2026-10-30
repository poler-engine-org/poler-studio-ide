//! Тип ошибок крейта `pqw`.

use std::fmt;
use std::io;

/// Псевдоним результата, используемый по всему крейту.
pub type Result<T, E = PqwError> = std::result::Result<T, E>;

/// Все способы, которыми сериализация / десериализация может завершиться ошибкой.
#[derive(Debug)]
pub enum PqwError {
    /// Ошибка ввода-вывода.
    Io(io::Error),
    /// Магические 8 байт не равны `POLER_QW`.
    BadMagic([u8; 8]),
    /// Версия формата не поддерживается этой сборкой.
    UnsupportedVersion(u32),
    /// Несовпадение контрольной суммы заголовка (FNV-1a64 по `0x00..0x78`).
    CorruptHeader { expected: u64, actual: u64 },
    /// Несовпадение digest payload (SHA-256, усечённый до 24 байт).
    CorruptPayload,
    /// Файл короче, чем объявляет структура заголовка.
    Truncated { need: usize, have: usize },
    /// `d_pol == 0`.
    BadDimension(u32),
    /// Длина плотного состояния не равна `d_pol`.
    StateLen { expected: usize, actual: usize },
    /// Индекс топологии вне диапазона.
    BadIndex { index: u32, d_pol: u32 },
    /// Со стороны писателя: дуга добавлена повторно.
    DuplicateIndex(u32),
    /// Со стороны читателя: индексы топологии не строго возрастают.
    UnsortedTopology,
    /// Структурное нарушение раскладки (смещения, длины, лишние байты).
    Layout(&'static str),
    /// Поле длины противоречит `nnz` / ширине индекса.
    InconsistentTopology {
        field: &'static str,
        expected: u64,
        actual: u64,
    },
    /// Зарезервированные биты `flags` или слово `reserved` не нулевые.
    ReservedBits { value: u64 },
    /// Комбинация флагов не поддерживается форматом v1.
    UnsupportedFlags(u64),
    /// Два младших бита фазового байта образуют зарезервированный трит `0b11`.
    ReservedTrit(u8),
    /// Значение фазы — NaN / бесконечность / вне `[-1, 1]`.
    BadValue(f32),
    /// Операция определена только для контейнеров v2 (magic `POLER_Q2`).
    NotPacked,
}

impl fmt::Display for PqwError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PqwError::Io(e) => write!(f, "i/o error: {e}"),
            PqwError::BadMagic(m) => {
                write!(
                    f,
                    "bad magic: {m:02x?} (expected \"POLER_QW\" or \"POLER_Q2\")"
                )
            }
            PqwError::UnsupportedVersion(v) => write!(
                f,
                "unsupported format version: {v} (this build supports POLER_QW v1 and POLER_Q2 v2)"
            ),
            PqwError::CorruptHeader { expected, actual } => write!(
                f,
                "header checksum mismatch: stored {expected:#018x}, computed {actual:#018x}"
            ),
            PqwError::CorruptPayload => {
                write!(f, "payload digest mismatch (SHA-256 truncated to 24 bytes)")
            }
            PqwError::Truncated { need, have } => {
                write!(f, "truncated file: need {need} bytes, have {have}")
            }
            PqwError::BadDimension(d) => {
                write!(f, "invalid dimension d_pol = {d} (must be >= 1)")
            }
            PqwError::StateLen { expected, actual } => {
                write!(f, "state length {actual} != d_pol {expected}")
            }
            PqwError::BadIndex { index, d_pol } => {
                write!(
                    f,
                    "topology index {index} out of bounds for d_pol = {d_pol}"
                )
            }
            PqwError::DuplicateIndex(i) => write!(f, "duplicate arc index {i}"),
            PqwError::UnsortedTopology => {
                write!(f, "topology indices are not strictly increasing")
            }
            PqwError::Layout(msg) => write!(f, "layout violation: {msg}"),
            PqwError::InconsistentTopology {
                field,
                expected,
                actual,
            } => write!(
                f,
                "inconsistent {field}: expected {expected}, found {actual}"
            ),
            PqwError::ReservedBits { value } => {
                write!(f, "reserved bits must be zero, found {value:#x}")
            }
            PqwError::UnsupportedFlags(v) => write!(
                f,
                "unsupported flags combination: {v:#x} (v1 requires the curvature bit)"
            ),
            PqwError::ReservedTrit(b) => {
                write!(f, "phase byte {b:#04x} uses the reserved trit pattern 0b11")
            }
            PqwError::BadValue(p) => {
                write!(f, "phase value {p} is not a finite number in [-1, 1]")
            }
            PqwError::NotPacked => {
                write!(
                    f,
                    "operation requires a packed v2 container (magic POLER_Q2)"
                )
            }
        }
    }
}

impl std::error::Error for PqwError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PqwError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for PqwError {
    fn from(e: io::Error) -> Self {
        PqwError::Io(e)
    }
}
