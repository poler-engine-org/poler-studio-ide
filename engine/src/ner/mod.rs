//! NER — извлечение сущностей (PLAN_POLER_V2, Фаза 7 через pqc).

pub mod native_gliner;

pub use native_gliner::{synth_gliner, Entity, GlinerModel, RealGlinerModel};
