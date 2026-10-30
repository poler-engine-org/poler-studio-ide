//! S2/v0.36.0 «Триединая Архитектура» — муха + вихрь + кристалл → речь.
//!
//! Объединение трёх доказанных подсистем в одну говорящую цельность
//! (ответ на директиву: «мозг мухи объединить с синусоидным вихрем +
//! квантованная намертво зашитая в код модель — и на выходе говорящее
//! что-то»):
//!
//! | Опора | Модуль | Роль в речи |
//! |---|---|---|
//! | **Мозг мухи** (FLYCSR1, C2/v0.33.0) | [`flypulse`] | пульс и драйв: ротор J касты не даёт мысли застыть; дрейф по 12 архетипам разводит лексику |
//! | **Синаптический вихрь** (SSN, S1/v0.35.0) | [`crate::ssn`] | живой мозг: гомеостаз 5%, критичность; медиаторы DA/5HT/NE задают температуру сэмплирования |
//! | **Кристалл знаний** (.t5c, Trit5) | [`crystal`] | троичные веса мира: семантика через SIMD No-Mul, синтаксис биграммными тритами |
//!
//! Плюс две несущие конструкции:
//!
//! - [`stream_quant`] — потоковое квантование весов на лету (.t5q):
//!   путь «70B на диске 4 ГБ» — сырец не касается диска, RAM-пик —
//!   килобайты, результат не зависит от чанков чтения.
//! - [`ingest`] — потоковый интернет-инжектор памяти: папки/веб-краул →
//!   .t5c без удержания корпуса в RAM (RAM-дисциплина: слово ≤ 256 Б,
//!   счётчики под капами с детерминированной эвакуацией);
//! - [`motor`] — S2 моторный слой: речь → интенты poler_exec
//!   (только предложения; исполнение остаётся за агентом).
//!
//! ## Доказательства (до реализации — как требует F-принцип)
//!
//! - детерминизм речи (seed → те же слова);
//! - вихрь жив во время речи (активность в коридоре, S < 0.95);
//! - муха ломает циклы при γ > 0 (усреднение по 8 seed);
//! - температура зажата [0.6, 1.8] при любых медиаторах;
//! - NMDA-гейт пропускает совпадающие смыслы;
//! - моторный слой отклоняет деструктивную лексику;
//! - потоковое квантование инвариантно к размеру чанков;
//! - .t5c/.t5q защищены sha256.

pub mod compiler;
pub mod crystal;
pub mod flypulse;
pub mod ingest;
pub mod jit_loop;
pub mod motor;
pub mod motor_bridge;
pub mod neuron;
pub mod stream_quant;
pub mod triune;

pub use compiler::{
    CommitStats, MutationImpulse, PlasticityCompiler, PlasticityConfig, T5qMmapView, TritDirection,
    TritMutator,
};
pub use crystal::Crystal;
pub use flypulse::{FlyPulse, PulseOrigin};
pub use ingest::{IngestConfig, IngestStats, IngestingFetcher, StreamCrystalBuilder};
pub use jit_loop::{CycleReport, ExecutableKernel, JitLoop, WeightsInCode};
pub use motor::{MotorIntent, MotorOp};
pub use motor_bridge::{act_on_intents, execute, resolve, ActionLevel, BridgeConfig, ExecOutcome, ResolvedAction};
pub use neuron::{Inject, NeuralPopulation, NeuronConfig, NeuronTelemetry};
pub use stream_quant::{stream_quantize, verify_t5q, StreamQuantConfig, StreamQuantStats};
pub use triune::{TokenTrace, TriuneConfig, TriuneCore, Utterance};

