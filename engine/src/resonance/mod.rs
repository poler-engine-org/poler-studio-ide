//! Резонансный анализ: локальная информационная плотность ε и IIR-фильтр R(t).

pub mod epsilon;
pub mod iir_filter;

pub use epsilon::{calculate_epsilon, semantic_bonus, SlidingEpsilon};
pub use iir_filter::{apply_iir_resonance, clamp_phi, IirFilter};
