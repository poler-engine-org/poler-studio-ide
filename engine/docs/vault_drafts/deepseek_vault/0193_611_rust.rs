use burn::tensor::{backend::Backend, Tensor};

pub struct FEPLoss {
pub beta: f64,
}

impl FEPLoss {
pub fn new(beta: f64) -> Self {
Self { beta }
}

pub fn compute<B: Backend>(
&self,
prediction: &Tensor<B, 3>,
observation: &Tensor<B, 3>,
) -> Tensor<B, 1> {
let diff = prediction.clone() - observation.clone();
let accuracy = diff.powf_scalar(2.0)
.sum_dim([1, 2])
.squeeze([1, 2]);

let complexity = prediction.clone()
.powf_scalar(2.0)
.mean()
.mul_scalar(self.beta);

accuracy + complexity
}
}
📄 src/poler_core.rs
rust
