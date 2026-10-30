error[E0599]: no method named `exp` found for struct `Tensor<B, 1>` in the current scope
--> src/poler_core.rs:65:39
65  |         * sigma.clone().neg().exp();
|                               ^^^ method not found in `Tensor<B, 1>`

Причина: В Burn 0.14 метод exp для тензоров называется .exp(), но он существует только для тензоров с плавающей точкой. Однако здесь ошибка может быть связана с тем, что sigma — это Tensor<B, 1>, и .exp() должен работать. Но в коде есть лишний оператор * перед выражением? На самом деле строка:

rust
