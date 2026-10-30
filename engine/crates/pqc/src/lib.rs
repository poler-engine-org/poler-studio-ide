#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! # pqc — POLER Quantum Core
//!
//! Вычислительное ядро POLER\[Ψ\] на чистом Rust: statevector-движок с
//! анзацем `R_y(arccos p)` и Born-сэмплированием поверх контейнера
//! [`.poler` / `.pqw`](https://docs.rs/pqw) — **без единой внешней
//! зависимости** (ни faer, ни rand, ни rayon).
//!
//! ## Математика анзаца
//!
//! Трек `p ∈ [−1, 1]` кодируется кубитом через угол `θ = arccos(p)`:
//!
//! ```text
//! |ψ⟩ = ⊗_q R_y(θ_q)|0⟩,    R_y(θ)|0⟩ = cos(θ/2)|0⟩ + sin(θ/2)|1⟩
//! ```
//!
//! Born-семантика трека: `P(b_q = 1) = sin²(θ_q/2) = (1 − p_q)/2`, а
//! собственное значение McWeeny `λ_q = (1 + p_q)/2` — это в точности
//! `P(b_q = 0)`. Чистые триты: `p = +1 → |0⟩`, `p = −1 → |1⟩`,
//! `p = 0 → честная монета` (фон LENS).
//!
//! ## Движки
//!
//! | Движок | Условие | Память | Выстрел |
//! |---|---|---|---|
//! | [`Statevector`] | `d_pol ≤ 20` (настраивается) | `2^d_pol × 16 B` | `O(d_pol)` бинарный поиск |
//! | [`PhaseAnsatz`] | любое `d_pol` | `O(nnz)` | `O(nnz + d_pol/64)` |
//!
//! ## Пример: анзац из фазового вектора
//!
//! ```
//! use pqc::{Rng, Statevector};
//!
//! // Трек p → кубит с P(b=1) = (1−p)/2.
//! let sv = Statevector::from_phases(&[0.0, -1.0, 1.0]).unwrap();
//! let m = sv.marginals();               // P(b_q = 1)
//! assert!((m[0] - 0.5).abs() < 1e-12);  // p = 0  → честная монета
//! assert!((m[1] - 1.0).abs() < 1e-12);  // p = −1 → |1⟩
//! assert!(m[2] < 1e-12);                // p = +1 → |0⟩
//!
//! // Обратная проверка кодирования: ⟨Z_q⟩ = p_q.
//! assert!((sv.expect_z(1).unwrap() - (-1.0)).abs() < 1e-12);
//! let _ = Rng::seed_from_u64(42);
//! ```
//!
//! ## Пример: Born-сэмплирование
//!
//! ```
//! use pqc::{BornSampler, Rng, Statevector};
//!
//! let sv = Statevector::from_phases(&[0.6, -0.2]).unwrap();
//! let mut rng = Rng::seed_from_u64(7);
//! let sampler = BornSampler::new(&sv).unwrap();
//! let shots = sampler.sample_n(&mut rng, 1000);
//! assert_eq!(shots.len(), 1000);
//!
//! // Исход 0b10 (b0=0, b1=1): P = P(b0=0)·P(b1=1) = 0.8·0.6.
//! assert!((sampler.outcome_probability(0b10) - 0.8 * 0.6).abs() < 1e-12);
//! ```
//!
//! ## Пример: конвейер из контейнера .pqw
//!
//! ```
//! use pqc::{Ansatz, LoadOptions, Rng};
//! use pqw::{PqwReader, PqwWriter};
//!
//! let bytes = PqwWriter::new(4)
//!     .unwrap()
//!     .add_phase(1, -1.0)
//!     .unwrap()
//!     .add_phase(3, 0.0)
//!     .unwrap()
//!     .to_bytes()
//!     .unwrap();
//!
//! let reader = PqwReader::from_bytes(&bytes).unwrap();
//! let ansatz = Ansatz::from_reader(&reader, &LoadOptions::default()).unwrap();
//! let mut rng = Rng::seed_from_u64(1);
//! let report = ansatz.sample(&mut rng, 500, 4).unwrap();
//! assert_eq!(report.shots, 500);
//! assert_eq!(report.engine.name(), "statevector");
//!
//! // p = −1 → кубит 1 всегда в |1⟩: наблюдённая маргинала равна 1.
//! let q1 = report.marginals.iter().find(|m| m.0 == 1).unwrap();
//! assert!((q1.2 - 1.0).abs() < 1e-12);
//! ```

pub mod ansatz;
pub mod algorithms;
pub mod archetype;
pub mod archetype_lattice;
pub mod bloch_stream;
pub mod born;
pub mod coherence;
pub mod complex;
pub mod crypto;
pub mod entangle;
pub mod error;
pub mod exact;
pub mod gates;
pub mod generate;
pub mod gyro;
pub mod gyro_lattice;
pub mod inspect;
pub mod json;
pub mod learn;
pub mod learn_net;
pub mod docsrc;
pub mod merge;
pub mod netfetch;
pub mod noise;
pub mod parallel;
pub mod parity;
pub mod qcurriculum;
pub mod qaoa;
pub mod qpc;
pub mod rng;
pub mod statevector;
pub mod stabilizer;
pub mod stream;
pub mod stream_engine;
pub mod substrate;
pub mod tls13;
pub mod tlsprim;
pub mod trite;
pub mod verify;
pub mod wikisrc;
pub mod gpu_kernel;

pub use ansatz::{
    Ansatz, Engine, LoadOptions, PhaseAnsatz, ProductStats, SampleReport, DEFAULT_MAX_SV_QUBITS,
};
pub use archetype::{precess_to_fixpoint, PrecessReport, TracePoint};
pub use born::BornSampler;
pub use coherence::{binary_entropy, coherence, CoherenceReport};
pub use complex::Cx;
// qpc — POLER Quantum PC (идеальные кубиты: точный режим + субстрат УДЕ).
pub use algorithms::{
    bell, bernstein_vazirani, deutsch_jozsa, ghz, grover, qft,
};
pub use exact::{run_exact, ExactCx, ExactDyadic, ExactReport, ExactStatevector};
pub use qpc::{
    run as run_circuit, Circuit, Op, QpcReport, K_B, LANDAUER_T,
};
pub use substrate::{
    mcweeney, run_flow, run_random, CMat, SubstrateConfig, SubstratePoint,
    SubstrateReport,
};
pub use crypto::{
    cipher_distance, decrypt, encrypt, CipherKey, DecryptReport, EncryptReport, MAX_MODES,
};
pub use entangle::{Entanglement, Entangler};
pub use error::{PqcError, Result};
pub use gates::Gate;
pub use generate::{
    BornStep, GenerationReport, GeneratorConfig, L5Generator, LexiconBuilder, TicketSource,
    LEXICON_TOKEN_MAX,
};
pub use gyro::{precess_step, resonant_modes_from_pairs, GyroMode, Gyroscope};
pub use gyro_lattice::{
    precess_step_packed4, EngineMemory, MomentumEvent, MomentumLattice, PrecessScratch,
    PrecessStats, QuantizedGyroCurriculum, QuantizedGyroReport, TransportMode, TritGyro,
    IGNITE_THRESHOLD, MAX_DIM_GYRO, MOMENTUM_UNIT, SIN_LUT, STALL_THRESHOLD,
};
pub use inspect::{
    arcs_csr_text, arcs_dot, ascii_matrix, ascii_strings, born_entropy, crypto_recon, detect_kind,
    graph_stats, header_rows, hex_dump, qcm_theory, raw_arcs, raw_packed4_arcs, reader_arcs,
    report_json, try_pqw_reader, CryptoRecon, DecodedArc, FieldRow, FileKind, GraphStats,
    ARCS_PREVIEW, MATRIX_D_MAX,
};
pub use json::{Json, JsonError};
pub use learn::{quadratic_target, ActiveInference, ActiveStepReport, BornOptimizer, StepReport};
pub use parity::{splitmix_phases, ParityAnsatz, ParityMode};
pub use rng::Rng;
pub use trite::{
    decrypt as trite_decrypt, encrypt as trite_encrypt, TritKey, TRITE_MAGIC,
};
pub use statevector::{phase_to_theta, Statevector, MAX_QUBITS};
pub use stream::{StreamReport, ZeroStoragePipeline};
pub use stream_engine::{
    fock_residual, strip_html, FockResidual, Forget, StreamChunkReport, StreamEngine,
};

pub use syntax_unfolder::{is_stationary, morpheme_at, unfurl, unfurl_to_string, MAX_OUT, TRIT_THRESHOLD};

pub mod syntax_unfolder;

// RQ21: грамматические мосты — синтаксическая связность речи L5.
pub mod syntax_bridge;
pub use syntax_bridge::{is_cyrillic_token, syntax_class, SyntaxChain, SyntaxClass, BRIDGE_RUN};

// RQ23: контекст-рефлекс W — автобиографическая память диалога.
pub mod reflex;
pub use reflex::{ContextReflex, REFLEX_NAME_CAP, REFLEX_TRAIL_CAP};

// RQ23: спиновая лавина GF(3) — нелинейный слой диффузии трит-шифра.
pub mod spin_avalanche;
pub use spin_avalanche::{measure_avalanche, msg_from_seed, AvalancheStats};
