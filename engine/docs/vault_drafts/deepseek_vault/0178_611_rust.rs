error[E0308]: mismatched types
--> src/poler_core.rs:72:36
72  |         let flow = self.sctp.forward(force);
|                    ----------------- ^^^^^ expected `Tensor<B, 3>`, found `Tensor<B, 3>`
