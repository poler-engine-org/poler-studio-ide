// wasm-synaptics/src/lib.rs
use wasm_bindgen::prelude::*;
use nalgebra::{DVector, DMatrix};

#[wasm_bindgen]
pub struct CognitiveField {
// Фиксированные операторы
projection: DMatrix<f64>,
