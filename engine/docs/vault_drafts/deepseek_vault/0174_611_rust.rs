let s = (rotated - dissipated).mul_scalar(strength_val);

Проблема: strength_val — это f64, что правильно. Но в строке 65 используется strength_val, определённое как let strength_val: f64 = self.strength.val().into_scalar(); – это допустимо. Однако ошибка возникает в другом месте: в коде, который вы предоставили ранее, strength был Param<Tensor<B,1>>, и в forward использовалось strength_val, полученное через into_scalar(). Это преобразование синхронное, но компилятор его пропускает. Но в предоставленном коде synaptic_ops.rs (в запросе для кластера) строка выглядит так:

rust
