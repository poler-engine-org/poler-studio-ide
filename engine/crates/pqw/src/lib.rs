#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! # pqw — POLER Quantum Weights
//!
//! Бинарный формат `.poler` / `.pqw`: сериализация фазовых состояний
//! POLER\[Ψ\] **без единой внешней зависимости** — ни serde, ни memmap2, ни libc.
//!
//! ## Слои формата (v1)
//!
//! ```text
//! 0x00   8   magic "POLER_QW"
//! 0x08   4   format_version (u32 LE, = 1)
//! 0x0C   4   d_pol (u32 LE)
//! 0x10  16   гиперпараметры: eta, gamma, rho, epsilon_threshold (f32 LE × 4)
//! 0x20   8   McWeeny: max |λ² − λ| по хранимым дугам (f64 LE)
//! 0x28  24   payload digest: SHA-256(topology ‖ phases), усечённый до 24 байт
//! 0x40  64   таблица смещений: topology/phase offset+len, nnz, flags,
//!            reserved (= 0), header checksum (FNV-1a64 по 0x00..0x78)
//! 0x80   ·   sparse LENS-топология: nnz × u16 (d_pol ≤ 65536) или nnz × u32 LE
//!        ·   фазовые блоки: 1 байт на дугу:
//!            биты 0..1 — трит {−1, 0, +1}; биты 2..7 — кривизна σ ∈ [0, 63]
//! EOF
//! ```
//!
//! Декодирование дуги: `p̂ = trit · (σ / 63)`, фазовый угол `θ̂ = arccos(p̂)`,
//! граница ошибки квантования `|p̂ − p| ≤ 1/126`.
//!
//! ## Пример
//!
//! ```
//! use pqw::{PqwReader, PqwWriter};
//!
//! let bytes = PqwWriter::new(8)?
//!     .hyperparams(0.5, 0.25, 0.75, 0.125)
//!     .add_phase(1, 0.5)?
//!     .add_phase(3, -1.0)?
//!     .to_bytes()?;
//!
//! let reader = PqwReader::from_bytes(&bytes)?;
//! assert_eq!(reader.nnz(), 2);
//! assert_eq!(reader.indices().to_vec(), vec![1, 3]);
//! # Ok::<(), pqw::PqwError>(())
//! ```

pub mod checksum;
pub mod error;
pub mod gyro;
pub mod header;
pub mod lexicon;
pub mod mcweeny;
#[cfg(unix)]
pub mod mmap;
pub mod phase;
pub mod reader;
pub mod reflex;
pub mod sha256;
pub mod stream;
pub mod topology;
pub mod trit_bloch;
pub mod writer;

pub use error::{PqwError, Result};
pub use gyro::{
    GyroData, GyroPair, GyroSection, GYRO_CODEC_GAP_RLE, GYRO_HEADER_SIZE, GYRO_MAGIC,
    GYRO_QUANT_MAX, GYRO_SECTION_VERSION, GYRO_SECTION_VERSION_RLE, gyro_slot, slot_to_pair,
};
pub use header::{
    Flags, Header, HyperParams, FORMAT_VERSION, FORMAT_VERSION_V2, FORMAT_VERSION_V3,
    FORMAT_VERSION_V4, FORMAT_VERSION_V5, HEADER_SIZE, MAGIC, MAGIC_V2, MAGIC_V3, MAGIC_V4,
    MAGIC_V5,
};
pub use lexicon::{Lexicon, LEXI_HEADER_SIZE, LEXI_MAGIC, LEXI_MAX_TOKEN, LEXI_SECTION_VERSION};
pub use phase::{
    nearest_trit, pack_quad, pack_trit2, unpack_quad, unpack_trit2, PhaseByte, Trit, TritEncoding,
    QUANT_EPS, SIGMA_MAX,
};
pub use reader::{Arc, Arcs, PackedTrits, PqwReader};
pub use reflex::{
    ReflexData, ReflexSection, REFL_HEADER_SIZE, REFL_MAGIC, REFL_MAX_EVENTS, REFL_MAX_NAME,
    REFL_SECTION_VERSION,
};
pub use stream::TextPhaseEncoder;
pub use writer::PqwWriter;

#[cfg(unix)]
pub use mmap::Mmap;

/// Оба расширения обозначают один и тот же контейнер v1:
/// `.poler` — состояние экосистемы POLER, `.pqw` — квантовые веса.
pub const EXTENSIONS: [&str; 2] = [".poler", ".pqw"];
