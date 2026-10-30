//! poler-core — Центральная библиотека когнитивного движка POLER[Psi]
pub mod poler_core;
pub mod fep_loss;
pub mod residual_renderer;
pub mod entity_resolver;

pub use poler_core::{PolerCore, PolerCoreConfig};
pub use fep_loss::{FEPLoss, FEPLossConfig};
pub use residual_renderer::ResidualRenderer;
pub use entity_resolver::EntityResolver;
