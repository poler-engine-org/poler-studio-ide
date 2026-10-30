use burn::tensor::{backend::Backend, Tensor};

pub struct EnergyParams {
pub kappa: f64,
pub rho: f64,
pub alpha: f64,
}

impl Default for EnergyParams {
fn default() -> Self {
Self {
kappa: 1.0,
rho: 0.9,
alpha: 0.1,
}
}
}

pub struct EnergyEngine {
pub params: EnergyParams,
}

impl EnergyEngine {
pub fn new(params: EnergyParams) -> Self {
Self { params }
}

pub fn compute_energy<B: Backend>(
&self,
current: &Tensor<B, 3>,
previous: &Tensor<B, 3>,
) -> Tensor<B, 1> {
let diff = current.clone() - previous.clone();
diff.powf_scalar(2.0)
.sum_dim([1, 2])
.squeeze([1, 2])
.mul_scalar(self.params.kappa)
}

pub fn update_resonance<B: Backend>(
&self,
r_prev: &Tensor<B, 3>,
thought: &Tensor<B, 3>,
energy: &Tensor<B, 1>,
) -> Tensor<B, 3> {
let energy_3d = energy.clone().unsqueeze().unsqueeze(); // [batch, 1, 1]
let gain = thought.clone().mul(energy_3d.add_scalar(1.0));
r_prev.clone().mul_scalar(self.params.rho)
+ gain.mul_scalar(self.params.alpha)
}
}
📄 src/fep_loss.rs
rust
