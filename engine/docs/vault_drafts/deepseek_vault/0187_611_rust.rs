let eta = Tensor::from_floats([cfg.eta_base], &state.p.device())
* sigma.clone().neg().exp();
