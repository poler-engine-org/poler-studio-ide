//! LLM-инференс — локальная генерация текста (PLAN_POLER_V2, Part F).
//!
//! GLM-декодер поверх `.pqw`-весов: RoPE, MQA/GQA, SwiGLU, MoE-роутер,
//! KV-арена, сэмплирование. Ни Python, ни PyTorch, ни API — один
//! статический бинарь.

pub mod glm_engine;

pub use glm_engine::{
    sample_token, synth_glm, KvArena, Sampling, GlmModel,
};
