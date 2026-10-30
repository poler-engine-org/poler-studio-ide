//! poler_dynamis_v5 — POLER DYNAMIS v5.0: Supreme Law Engine
pub mod subquantum_bridge;
pub mod energy_engine;
pub mod eri_engine;
pub mod gto_utils;
pub mod basis_parser;
pub mod elements;

pub use elements::{ElementInfo, PERIODIC_TABLE, get_element_by_z, get_element_by_symbol};
pub use energy_engine::*;
pub use eri_engine::EriEngine;
pub use gto_utils::{double_factorial, gto_norm_constant};
pub use basis_parser::BasisParser;
