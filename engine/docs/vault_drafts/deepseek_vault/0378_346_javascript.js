// ============================================================================
// tb_tensor.v - Testbench for Tensor Product Module (Q32.32 Fixed-Point)
// ============================================================================
// Verifies: X ⊗_ε Y = (X·Y) + ε·(X⊙Y)
// Uses Q32.32 fixed-point representation for all matrix elements.
// ============================================================================

`timescale 1ns/1ps

module tb_tensor;

parameter N = 4;
parameter MAT_ENTRIES = 16;
parameter FRAC_BITS = 32;
parameter CLK_PERIOD = 10;

// Signals
reg                  clk;
reg                  rst_n;
reg                  valid_in;
reg signed [63:0]    x [0:MAT_ENTRIES-1];
reg signed [63:0]    y [0:MAT_ENTRIES-1];
reg signed [63:0]    epsilon;
wire signed [63:0]   result [0:MAT_ENTRIES-1];
wire                 valid_out;

// Test tracking
integer test_count;
integer pass_count;
integer fail_count;

// Convert double to Q32.32
function automatic signed [63:0] dbl2q;
input real v;
reg signed [63:0] q;
begin
q = v * (2.0 ** FRAC_BITS);
dbl2q = q;
end
endfunction

// Convert Q32.32 to double
function automatic real q2dbl;
input signed [63:0] q;
begin
q2dbl = real'(q) / (2.0 ** FRAC_BITS);
end
endfunction

// Q32.32 multiplication
function automatic signed [63:0] qmul;
input signed [63:0] a, b;
reg signed [127:0] full;
begin
full = a * b;
qmul = full >>> FRAC_BITS;
end
endfunction

// Instantiate DUT
tensor_product #(
.N(N),
.MAT_ENTRIES(MAT_ENTRIES),
.FRAC_BITS(FRAC_BITS)
) dut (
.clk(clk),
.rst_n(rst_n),
.valid_in(valid_in),
.x(x),
.y(y),
.epsilon(epsilon),
.result(result),
.valid_out(valid_out)
);

// Clock generation
initial begin
clk = 0;
forever #(CLK_PERIOD/2) clk = ~clk;
end

// Main test sequence
initial begin
rst_n = 0;
valid_in = 0;
epsilon = 64'sd0;
test_count = 0;
pass_count = 0;
fail_count = 0;

// Initialize matrices
init_zero_x();
init_zero_y();

// Reset
#(CLK_PERIOD * 5);
rst_n = 1;
#(CLK_PERIOD * 2);

$display("=== Tensor Product Testbench (Q32.32, Hadamard) ===");
$display("Formula: X ⊗_ε Y = (X·Y) + ε·(X⊙Y)");
$display("");

// Test 1: Identity × Scalar (ε = 0)
// I ⊗_0 (2I) = I·(2I) = 2I
load_identity_x();
load_scalar_y(dbl2q(2.0));
epsilon = dbl2q(0.0);
apply_and_check_diag("I ⊗_0 (2I) = 2I", 2.0, 0.1);

// Test 2: Identity × Identity with ε
// I ⊗_ε I = I·I + ε·(I⊙I) = I + ε·I = (1+ε)I
load_identity_x();
load_identity_y();
epsilon = dbl2q(0.5);
apply_and_check_diag("I ⊗_0.5 I = 1.5I", 1.5, 0.2);

// Test 3: Diagonal matrices
// diag(1,2,3,4) ⊗_0 diag(2,3,4,5) = diag(2,6,12,20)
load_diag_x();
load_diag_y();
epsilon = dbl2q(0.0);
apply_and_check_diag_values("diag ⊗_0 diag", 0.3);

// Test 4: Zero epsilon = plain matrix multiply
load_identity_x();
load_scalar_y(dbl2q(3.0));
epsilon = dbl2q(0.0);
apply_and_check_diag("I ⊗_0 (3I) = 3I", 3.0, 0.1);

// Test 5: Large epsilon (Hadamard dominant)
// I ⊗_10 I = I + 10·I = 11I
load_identity_x();
load_identity_y();
epsilon = dbl2q(10.0);
apply_and_check_diag("I ⊗_10 I = 11I", 11.0, 1.0);

// Test 6: Zero matrix
load_zero_x();
load_identity_y();
epsilon = dbl2q(1.0);
apply_and_check_all_zero("0 ⊗_1 I = 0", 0.01);

// Summary
#(CLK_PERIOD * 10);
$display("");
$display("=== Test Summary ===");
$display("Total: %0d, Passed: %0d, Failed: %0d",
test_count, pass_count, fail_count);

if (fail_count == 0)
$display("ALL TESTS PASSED");
else
$display("SOME TESTS FAILED");

$finish;
end

// ========================================================================
// Helper tasks
// ========================================================================

task init_zero_x;
integer i;
begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
x[i] = 64'sd0;
end
endtask

task init_zero_y;
integer i;
begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
y[i] = 64'sd0;
end
endtask

task load_identity_x;
integer i;
begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
x[i] = 64'sd0;
x[0]  = dbl2q(1.0); x[5]  = dbl2q(1.0);
x[10] = dbl2q(1.0); x[15] = dbl2q(1.0);
end
endtask

task load_identity_y;
integer i;
begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
y[i] = 64'sd0;
y[0]  = dbl2q(1.0); y[5]  = dbl2q(1.0);
y[10] = dbl2q(1.0); y[15] = dbl2q(1.0);
end
endtask

task load_scalar_y;
input signed [63:0] s;
integer i;
begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
y[i] = 64'sd0;
y[0] = s; y[5] = s; y[10] = s; y[15] = s;
end
endtask

task load_diag_x;
begin
init_zero_x();
x[0] = dbl2q(1.0); x[5] = dbl2q(2.0);
x[10] = dbl2q(3.0); x[15] = dbl2q(4.0);
end
endtask

task load_diag_y;
begin
init_zero_y();
y[0] = dbl2q(2.0); y[5] = dbl2q(3.0);
y[10] = dbl2q(4.0); y[15] = dbl2q(5.0);
end
endtask

task load_zero_x;
integer i;
begin
for (i = 0; i < MAT_ENTRIES; i = i + 1)
x[i] = 64'sd0;
end
endtask

// Check diagonal elements match expected value
task apply_and_check_diag;
input [256*8-1:0] test_name;
input real expected_val;
input real tolerance;
integer i;
integer all_ok;
begin
all_ok = 1;
@(posedge clk);
valid_in = 1;
@(posedge clk);
valid_in = 0;

// Wait for result
repeat (20) begin
@(posedge clk);
if (valid_out) begin
test_count = test_count + 1;
for (i = 0; i < N; i = i + 1) begin
if (i == i) begin  // Check diagonal only
real r_val;
r_val = q2dbl(result[i*N+i]);
if (r_val < expected_val - tolerance || r_val > expected_val + tolerance) begin
all_ok = 0;
$display("  FAIL: %0s diag[%0d]=%f expected=%f",
test_name, i, r_val, expected_val);
end
end
end
if (all_ok) begin
pass_count = pass_count + 1;
$display("  PASS: %0s", test_name);
end else begin
fail_count = fail_count + 1;
end
disable apply_and_check_diag;
end
end
test_count = test_count + 1;
fail_count = fail_count + 1;
$display("  FAIL: %0s (timeout)", test_name);
end
endtask

// Check diagonal elements for diag×diag matrix multiply
task apply_and_check_diag_values;
input [256*8-1:0] test_name;
input real tolerance;
real expected [0:3];
integer i;
integer all_ok;
begin
// diag(1,2,3,4) · diag(2,3,4,5) = diag(2,6,12,20)
expected[0] = 2.0;
expected[1] = 6.0;
expected[2] = 12.0;
expected[3] = 20.0;
all_ok = 1;

@(posedge clk);
valid_in = 1;
@(posedge clk);
valid_in = 0;

repeat (20) begin
@(posedge clk);
if (valid_out) begin
test_count = test_count + 1;
for (i = 0; i < N; i = i + 1) begin
real r_val;
r_val = q2dbl(result[i*N+i]);
if (r_val < expected[i] - tolerance || r_val > expected[i] + tolerance) begin
all_ok = 0;
$display("  FAIL: %0s diag[%0d]=%f expected=%f",
test_name, i, r_val, expected[i]);
end
end
if (all_ok) begin
pass_count = pass_count + 1;
$display("  PASS: %0s", test_name);
end else begin
fail_count = fail_count + 1;
end
disable apply_and_check_diag_values;
end
end
test_count = test_count + 1;
fail_count = fail_count + 1;
$display("  FAIL: %0s (timeout)", test_name);
end
endtask

// Check all elements are approximately zero
task apply_and_check_all_zero;
input [256*8-1:0] test_name;
input real tolerance;
integer i;
integer all_ok;
begin
all_ok = 1;
@(posedge clk);
valid_in = 1;
@(posedge clk);
valid_in = 0;

repeat (20) begin
@(posedge clk);
if (valid_out) begin
test_count = test_count + 1;
for (i = 0; i < MAT_ENTRIES; i = i + 1) begin
real r_val;
r_val = q2dbl(result[i]);
if (r_val < -tolerance || r_val > tolerance) begin
all_ok = 0;
end
end
if (all_ok) begin
pass_count = pass_count + 1;
$display("  PASS: %0s", test_name);
end else begin
fail_count = fail_count + 1;
$display("  FAIL: %0s (non-zero elements found)", test_name);
end
disable apply_and_check_all_zero;
end
end
test_count = test_count + 1;
fail_count = fail_count + 1;
$display("  FAIL: %0s (timeout)", test_name);
end
endtask

// VCD dump
initial begin
$dumpfile("tb_tensor.vcd");
$dumpvars(0, tb_tensor);
end

endmodule
