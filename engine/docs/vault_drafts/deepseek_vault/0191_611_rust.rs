use burn::nn::{Gelu, LayerNorm, LayerNormConfig, Linear, LinearConfig};
use burn::tensor::{backend::Backend, Distribution, Tensor};
use burn::module::{Module, Param};

#[derive(Module, Debug)]
pub struct ConstraintLayer<B: Backend> {
pub up_proj: Linear<B>,
pub down_proj: Linear<B>,
pub activation: Gelu,
pub raw_j: Param<Tensor<B, 2>>,
pub raw_d: Param<Tensor<B, 2>>,
pub norm: LayerNorm<B>,
pub strength: Param<Tensor<B, 1>>,
}

impl<B: Backend> ConstraintLayer<B> {
pub fn new(input_dim: usize, constraint_dim: usize, device: &B::Device) -> Self {
let up_proj = LinearConfig::new(input_dim, constraint_dim)
.with_bias(false)
.init(device);
let down_proj = LinearConfig::new(constraint_dim, input_dim)
.with_bias(false)
.init(device);
let activation = Gelu::new();
let norm = LayerNormConfig::new(input_dim).init(device);
let raw_j = Tensor::eye(constraint_dim, device).mul_scalar(0.1);
let raw_d = Tensor::zeros([constraint_dim, constraint_dim], device).add_scalar(0.01);
let strength = Tensor::from_floats([0.5], device);

Self {
up_proj,
down_proj,
activation,
raw_j: Param::from_tensor(raw_j),
raw_d: Param::from_tensor(raw_d),
norm,
strength: Param::from_tensor(strength),
}
}

pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
let p = self.activation.forward(self.up_proj.forward(x.clone()));
let w_j = self.raw_j.val();
let j_anti = (w_j.clone() - w_j.transpose()).mul_scalar(0.5);
let rotated = p.clone().matmul(j_anti);
let w_l = self.raw_d.val();
let d_pos = w_l.clone().matmul(w_l.transpose());
let dissipated = p.matmul(d_pos);
let strength_val: f64 = self.strength.val().into_scalar();
let s = (rotated - dissipated).mul_scalar(strength_val);
let flow = self.down_proj.forward(s);
self.norm.forward(x + flow)
}
}
📄 src/energy_engine.rs
rust
