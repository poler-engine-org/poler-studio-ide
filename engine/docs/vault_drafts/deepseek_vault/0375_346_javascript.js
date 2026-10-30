// ============================================================================
// tensor_product.v - Deformed Tensor Product Module (f64/Q32.32 Fixed-Point)
// ============================================================================
// Implements: X ⊗_ε Y = (X·Y) + ε·(X⊙Y)
//   X·Y = matrix multiplication (linear interaction)
//   X⊙Y = Hadamard product (element-wise, NOT XOR)
//   ε   = energy/deformation parameter (f64 in software, Q32.32 in hardware)
//
// FPGA Implementation Strategy:
//   - Uses Q32.32 fixed-point (64-bit: 32 integer + 32 fractional bits)
//   - DSP48E1 slices for 25×18 multipliers, cascaded for 32×32
//   - Pipeline: 5 stages for matrix multiply + Hadamard + accumulation
//   - 4×4 matrix: 64 Hadamard elements + 64 matmul accumulations
// ============================================================================

module tensor_product #(
parameter N = 4,                    // Matrix dimension (4×4)
parameter FRAC_BITS = 32,           // Fractional bits in Q32.32
parameter MAT_ENTRIES = 16          // N*N
)(
input  wire                             clk,
input  wire                             rst_n,
input  wire                             valid_in,

// Matrix X (4×4, row-major, 64-bit per element Q32.32)
input  wire [63:0]  x [0:MAT_ENTRIES-1],
// Matrix Y (4×4, row-major, 64-bit per element Q32.32)
input  wire [63:0]  y [0:MAT_ENTRIES-1],
// Deformation parameter ε (Q32.32)
input  wire [63:0]  epsilon,

// Result: X ⊗_ε Y (4×4, row-major, Q32.32)
output reg  [63:0]  result [0:MAT_ENTRIES-1],
output reg          valid_out
);

// ========================================================================
// Stage 1: Hadamard Product (element-wise multiplication)
// X⊙Y: result_ij = X_ij * Y_ij
// ========================================================================
reg signed [63:0] hadamard [0:MAT_ENTRIES-1];
reg signed [63:0] stage1_eps;
reg              stage1_valid;

// Q32.32 × Q32.32 → Q64.64, shift right by 32 to get back Q32.32
function automatic signed [63:0] qmul;
input signed [63:0] a, b;
reg signed [127:0] full;
begin
full = a * b;
qmul = full >>> FRAC_BITS;  // Arithmetic shift right
end
endfunction

integer i;
always @(posedge clk or negedge rst_n) begin
if (!rst_n) begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
hadamard[i] <= 64'sd0;
stage1_eps   <= 64'sd0;
stage1_valid <= 1'b0;
end else begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
hadamard[i] <= qmul(x[i], y[i]);  // X_ij ⊙ Y_ij
stage1_eps   <= epsilon;
stage1_valid <= valid_in;
end
end

// ========================================================================
// Stage 2: Scaled Hadamard = ε·(X⊙Y)
// ========================================================================
reg signed [63:0] scaled_hadamard [0:MAT_ENTRIES-1];
reg signed [63:0] stage2_eps;
reg              stage2_valid;

always @(posedge clk or negedge rst_n) begin
if (!rst_n) begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
scaled_hadamard[i] <= 64'sd0;
stage2_eps   <= 64'sd0;
stage2_valid <= 1'b0;
end else begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
scaled_hadamard[i] <= qmul(stage1_eps, hadamard[i]);  // ε·(X⊙Y)
stage2_eps   <= stage1_eps;
stage2_valid <= stage1_valid;
end
end

// ========================================================================
// Stage 3: Matrix Multiplication X·Y (row of X × column of Y)
// (X·Y)_ij = Σ_k X_ik * Y_kj
// For 4×4: each element requires 4 multiplications + 3 additions
// ========================================================================
reg signed [63:0] matmul [0:MAT_ENTRIES-1];
reg              stage3_valid;

// Compute matrix multiplication using MAC (Multiply-Accumulate)
always @(posedge clk or negedge rst_n) begin
if (!rst_n) begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
matmul[i] <= 64'sd0;
stage3_valid <= 1'b0;
end else begin
for (i = 0; i < MAT_ENTRIES; i = i + 1) begin : matmul_compute
// Row = i / N, Col = i % N
// (X·Y)_ij = Σ_k X_{i,k} * Y_{k,j}
reg signed [63:0] acc;
integer k;
integer row, col;
row = i / N;
col = i % N;
acc = 64'sd0;
for (k = 0; k < N; k = k + 1) begin
acc = acc + qmul(x[row * N + k], y[k * N + col]);
end
matmul[i] <= acc;
end
stage3_valid <= valid_in;  // Parallel with Stage 1
end
end

// ========================================================================
// Stage 4: Combine: result = (X·Y) + ε·(X⊙Y)
// ========================================================================
reg signed [63:0] combined [0:MAT_ENTRIES-1];
reg              stage4_valid;

always @(posedge clk or negedge rst_n) begin
if (!rst_n) begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
combined[i] <= 64'sd0;
stage4_valid <= 1'b0;
end else begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
combined[i] <= matmul[i] + scaled_hadamard[i];  // (X·Y) + ε·(X⊙Y)
stage4_valid <= stage2_valid & stage3_valid;
end
end

// ========================================================================
// Stage 5: Output Register
// ========================================================================
always @(posedge clk or negedge rst_n) begin
if (!rst_n) begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
result[i] <= 64'sd0;
valid_out <= 1'b0;
end else begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
result[i] <= combined[i];
valid_out <= stage4_valid;
end
end

endmodule
