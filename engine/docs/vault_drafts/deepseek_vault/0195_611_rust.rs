use poler_core::{PolerCore, PolerConfig, PolerState};
use burn::backend::NdArray;
use burn::tensor::{Distribution, Tensor};

type Backend = NdArray;

fn main() {
println!("POLER Core v0.2.0");
let device = burn::backend::ndarray::NdArrayDevice::Cpu;
let config = PolerConfig::default();
let core = PolerCore::<Backend>::new(config, &device);
let state = core.init_state(2, 10, &device);
println!("State: {:?}", state.p.dims());
let observation = Tensor::random([2, 10, 64], Distribution::Normal(0.0, 1.0), &device);
let new_state = core.evolve(state, observation);
println!("Evolved: {:?}", new_state.p.dims());
}
