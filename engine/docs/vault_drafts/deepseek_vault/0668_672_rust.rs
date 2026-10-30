// lib.rs - Python модуль
use pyo3::prelude::*;
use ndarray::Array2;

#[pyclass]
struct SynapticEngine {
generator: DynamicWeightGenerator,
}

#[pymethods]
impl SynapticEngine {
#[new]
fn new(window_size: usize, embedding_dim: usize, constraint_dim: usize) -> Self {
SynapticEngine {
generator: DynamicWeightGenerator::new(
window_size, embedding_dim, constraint_dim
),
}
}

fn process_window(&self, data: Vec<Vec<f32>>) -> PyResult<Vec<Vec<f32>>> {
let array = Array2::from_shape_vec(
(data.len(), data[0].len()),
data.into_iter().flatten().collect()
).unwrap();

let operators = self.generator.generate_weights(&array.view());
let result = self.apply_operators(&array, &operators);

Ok(result.into_raw_vec().chunks(embedding_dim)
.map(|chunk| chunk.to_vec())
.collect())
}
}

#[pymodule]
fn synaptics(_py: Python, m: &PyModule) -> PyResult<()> {
m.add_class::<SynapticEngine>()?;
Ok(())
}
python
