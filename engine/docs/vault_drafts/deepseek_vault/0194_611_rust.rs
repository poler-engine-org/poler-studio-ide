use burn::tensor::{backend::Backend, Distribution, Tensor};
use burn::module::Module;
use crate::synaptic_ops::ConstraintLayer;
use crate::energy_engine::{EnergyEngine, EnergyParams};
use crate::fep_loss::FEPLoss;

pub struct PolerState<B: Backend> {
pub p: Tensor<B, 3>,
pub resonance: Tensor<B, 3>,
}

pub struct PolerConfig {
pub eta: f64,
pub gamma: f64,
pub dim: usize,
pub constraint_dim: usize,
}

impl Default for PolerConfig {
fn default() -> Self {
Self {
eta: 0.01,
gamma: 0.5,
dim: 64,
constraint_dim: 32,
}
}
}

#[derive(Module, Debug)]
pub struct PolerCore<B: Backend> {
pub sctp: ConstraintLayer<B>,
pub energy: EnergyEngine,
pub fep: FEPLoss,
pub config: PolerConfig,
}

impl<B: Backend> PolerCore<B> {
pub fn new(config: PolerConfig, device: &B::Device) -> Self {
Self {
sctp: ConstraintLayer::new(config.dim, config.constraint_dim, device),
energy: EnergyEngine::new(EnergyParams::default()),
fep: FEPLoss::new(0.1),
config,
}
}

pub fn init_state(&self, batch: usize, seq: usize, device: &B::Device) -> PolerState<B> {
PolerState {
p: Tensor::random(
[batch, seq, self.config.dim],
Distribution::Normal(0.0, 0.1),
device,
),
resonance: Tensor::zeros([batch, seq, self.config.dim], device),
}
}

pub fn evolve(&self, state: PolerState<B>, observation: Tensor<B, 3>) -> PolerState<B> {
let _f_val = self.fep.compute(&state.p, &observation);
let eps = self.energy.compute_energy(&state.p, &state.resonance);
let grad_f = (observation.clone() - state.p.clone()).mul_scalar(self.config.eta);
let grad_eps = (state.p.clone() - state.resonance.clone()).mul_scalar(self.config.gamma);
let force = grad_f + grad_eps;
let flow = self.sctp.forward(force);
let p_next = state.p.clone() + flow;
let new_resonance = self.energy.update_resonance(&state.resonance, &p_next, &eps);
PolerState {
p: p_next,
resonance: new_resonance,
}
}
}
📄 src/main.rs
rust
