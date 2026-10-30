error[E0599]: no method named `unsqueeze` found for struct `Tensor<B, 1>` in the current scope
--> src/energy_engine.rs:24:43
24  |         let energy_3d = energy.clone().unsqueeze().unsqueeze();
|                                           ^^^^^^^^^ help: there is a method with a similar name: `squeeze`
