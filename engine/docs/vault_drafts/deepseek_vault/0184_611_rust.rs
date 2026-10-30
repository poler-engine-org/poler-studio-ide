error[E0599]: no method named `mean` found for struct `Tensor<B, 3>` in the current scope
--> src/poler_core.rs:58:38
58  |         let sigma = delta.powf_scalar(2.0).mean()
|                                              ^^^^ method not found
