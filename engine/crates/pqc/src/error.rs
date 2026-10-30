//! Ошибки вычислительного ядра `pqc`.

use core::fmt;

/// Результат операций ядра.
pub type Result<T> = core::result::Result<T, PqcError>;

/// Ошибки statevector-движка, анзаца и моста к контейнеру `.pqw`.
#[derive(Debug)]
pub enum PqcError {
    /// Пустое состояние: ноль кубитов / ноль дуг / d_pol = 0.
    EmptyState,
    /// Больше кубитов, чем допускает движок (2^n амплитуд).
    TooManyQubits { requested: usize, max: usize },
    /// Индекс кубита за пределами [0, n_qubits).
    BadQubit { q: usize, n_qubits: usize },
    /// Контроль и цель двухкубитного гейта совпадают.
    SameQubit { control: usize, target: usize },
    /// Коллизия индексов Тоффоли (ccx): два из трёх кубитов совпали.
    BadCcxCollision,
    /// Ошибка разбора QCASM-текста (строка и причина).
    QcParse { line: usize, what: String },
    /// Гейт вне точного кольца ℤ[1/√2, i] (режим --exact).
    NotExactGate { gate: String },
    /// Переполнение i128 в точной арифметике — схема слишком глубока.
    ExactOverflow,
    /// Некорректный аргумент алгоритма/субстрата (CLI и library).
    BadArgument { what: String },
    /// Фаза вне [−1, 1] или NaN.
    BadPhase(f64),
    /// Born-правило определено только для нормированного состояния.
    NotNormalized { norm: f64 },
    /// Индекс дуги вне [0, d_pol).
    BadArc { index: u32, d_pol: u32 },
    /// Длина вектора фаз не равна числу кубитов анзаца.
    LengthMismatch { expected: usize, actual: usize },
    /// Дуги не отсортированы по индексу (или дублируются).
    UnsortedArcs,
    /// Операция не поддерживается для данного формата/конфигурации
    /// (например, квантованный curriculum требует контейнер v2/v3 Packed4).
    Unsupported { what: &'static str },
    /// Ошибка контейнера `.poler` / `.pqw`.
    Pqw(pqw::PqwError),
}

impl fmt::Display for PqcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PqcError::EmptyState => write!(f, "empty state: at least one qubit required"),
            PqcError::TooManyQubits { requested, max } => {
                write!(f, "too many qubits: {requested} > {max} (2^n amplitudes)")
            }
            PqcError::BadQubit { q, n_qubits } => {
                write!(f, "qubit index {q} out of range [0, {n_qubits})")
            }
            PqcError::SameQubit { control, target } => {
                write!(f, "control and target coincide: {control} == {target}")
            }
            PqcError::BadCcxCollision => {
                write!(f, "ccx qubit indices must be pairwise distinct")
            }
            PqcError::QcParse { line, what } => {
                write!(f, "circuit parse error at line {line}: {what}")
            }
            PqcError::NotExactGate { gate } => write!(
                f,
                "gate {gate} is outside the exact ring Z[1/sqrt(2), i] (Clifford+T)"
            ),
            PqcError::ExactOverflow => write!(
                f,
                "exact ring overflow (i128): circuit too deep for bit-exact mode"
            ),
            PqcError::BadArgument { what } => write!(f, "bad argument: {what}"),
            PqcError::BadPhase(p) => write!(f, "phase out of [-1, 1] or NaN: {p}"),
            PqcError::NotNormalized { norm } => {
                write!(
                    f,
                    "state is not normalized: |psi| = {norm}, Born rule requires 1"
                )
            }
            PqcError::BadArc { index, d_pol } => {
                write!(f, "arc index {index} out of range [0, {d_pol})")
            }
            PqcError::LengthMismatch { expected, actual } => {
                write!(
                    f,
                    "length mismatch: expected {expected} phases, got {actual}"
                )
            }
            PqcError::UnsortedArcs => write!(f, "arcs must be strictly sorted by index"),
            PqcError::Unsupported { what } => write!(f, "unsupported: {what}"),
            PqcError::Pqw(e) => write!(f, "pqw container: {e}"),
        }
    }
}

impl std::error::Error for PqcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PqcError::Pqw(e) => Some(e),
            _ => None,
        }
    }
}

impl From<pqw::PqwError> for PqcError {
    fn from(e: pqw::PqwError) -> PqcError {
        PqcError::Pqw(e)
    }
}
