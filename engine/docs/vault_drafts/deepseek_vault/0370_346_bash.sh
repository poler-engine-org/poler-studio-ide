# Compile and simulate the tensor product testbench
iverilog -o sim/tensor_product verilog/tensor_product.v simulation/tb_tensor.v && vvp sim/tensor_product
