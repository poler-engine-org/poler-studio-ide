// main.rs
use std::collections::HashMap;
use burn::{
config::Config,
module::Module,
nn,
tensor::{backend::AutodiffBackend, Tensor},
train::Adam,
};

// Параметры модели
