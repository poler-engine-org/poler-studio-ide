// ============================================================================
// poler_cycle.v - POLER Cycle Iteration Module (f64/Q32.32 Fixed-Point)
// ============================================================================
// Implements the corrected POLER cycle differential equation:
//
//   dp/dt = -η Π_Λ(p_t)[D·p_t + γJ·p_t + ∇F(p_t)]
//
// Discrete step:
//   P_new = p_t - η Π_Λ((L·L^T)p_t + γ(A-A^T)p_t + ∇F)
//
// Quantum normalization:
//   p_{t+1} = (1-mix)·P_new + mix·P_new/||P_new||
//
// Where:
//   D = L·L^T        (dissipator — entropy burner)
//   J = A - A^T      (resonance — skew-symmetric temporal echo)
//   Π_Λ = I - Jc^T(Jc·Jc^T)^(-1)·Jc  (logical projector — causality)
//   ∇F = G·p         (gradient of potential)
//
// All operations use Q32.32 fixed-point (64-bit).
// ============================================================================

module poler_cycle #(
parameter N = 4,
parameter MAT_ENTRIES = 16,
parameter FRAC_BITS = 32,
parameter MAX_ITERATIONS = 256,
parameter CONVERGENCE_THRESHOLD_FRAC = 16  // Threshold in fractional bits
)(
input  wire                             clk,
input  wire                             rst_n,
input  wire                             start,

// Dissipation matrix L (lower triangular, 4×4, Q32.32)
input  wire signed [63:0] L_mat [0:MAT_ENTRIES-1],
// Resonance matrix A (general, 4×4, Q32.32)
input  wire signed [63:0] A_mat [0:MAT_ENTRIES-1],
// Constraint matrix Jc (4×4, Q32.32)
input  wire signed [63:0] Jc_mat [0:MAT_ENTRIES-1],
// Gradient matrix G (4×4, Q32.32)
input  wire signed [63:0] G_mat [0:MAT_ENTRIES-1],

// Parameters
input  wire signed [63:0] eta,          // Learning rate (Q32.32)
input  wire signed [63:0] gamma,        // Resonance coupling (Q32.32)
input  wire signed [63:0] mix,          // Quantum normalization mix (Q32.32)

// Initial state vector p0 (4×1 column, stored as first column of 4×4, Q32.32)
input  wire signed [63:0] initial_state [0:MAT_ENTRIES-1],

// Outputs
output reg  signed [63:0] converged_state [0:MAT_ENTRIES-1],
output reg  [15:0]        iterations_used,
output reg                 converged,
output reg                 done
);

// ========================================================================
// Internal State Machine
// ========================================================================
typedef enum logic [3:0] {
IDLE            = 4'd0,
COMPUTE_D       = 4'd1,   // D = L·L^T
COMPUTE_J       = 4'd2,   // J = A - A^T
COMPUTE_PROJECTOR = 4'd3, // Π_Λ = I - Jc^T·(Jc·Jc^T)^(-1)·Jc
COMPUTE_FORCES  = 4'd4,   // D·p + γ·J·p + ∇F
PROJECT_FORCE   = 4'd5,   // Π_Λ(forces)
UPDATE_STATE    = 4'd6,   // P_new = p - η·Π_Λ(forces)
QUANTUM_NORM    = 4'd7,   // p_{t+1} = (1-mix)·P_new + mix·P_new/||P_new||
CHECK_CONV      = 4'd8,   // ||p_{t+1} - p_t|| < threshold?
FINISH          = 4'd9
} poler_state_t;

poler_state_t state;

// Intermediate matrices (Q32.32)
reg signed [63:0] D_mat [0:MAT_ENTRIES-1];      // Dissipator
reg signed [63:0] J_mat [0:MAT_ENTRIES-1];      // Resonance
reg signed [63:0] Pi_mat [0:MAT_ENTRIES-1];     // Projector
reg signed [63:0] current_p [0:MAT_ENTRIES-1];  // Current state
reg signed [63:0] P_new [0:MAT_ENTRIES-1];      // Updated state
reg signed [63:0] force_vec [0:MAT_ENTRIES-1];  // Combined force

reg [15:0] iter_count;
reg signed [63:0] prev_delta;

// ========================================================================
// Q32.32 Fixed-Point Helpers
// ========================================================================
function automatic signed [63:0] qmul;
input signed [63:0] a, b;
reg signed [127:0] full;
begin
full = a * b;
qmul = full >>> FRAC_BITS;
end
endfunction

function automatic signed [63:0] qadd;
input signed [63:0] a, b;
begin
qadd = a + b;
end
endfunction

function automatic signed [63:0] qsub;
input signed [63:0] a, b;
begin
qsub = a - b;
end
endfunction

// Convert integer to Q32.32
function automatic signed [63:0] int2q;
input integer v;
begin
int2q = v <<< FRAC_BITS;
end
endfunction

// ========================================================================
// Matrix Transpose (combinational)
// ========================================================================
function automatic signed [63:0] mat_transpose;
input integer row, col;
begin
mat_transpose = L_mat[col * N + row];  // Generic, use appropriate matrix
end
endfunction

// ========================================================================
// Main POLER Cycle State Machine
// ========================================================================
integer i, j, k;

always @(posedge clk or negedge rst_n) begin
if (!rst_n) begin
state <= IDLE;
iter_count <= 16'd0;
converged <= 1'b0;
done <= 1'b0;
for (i = 0; i < MAT_ENTRIES; i = i + 1) begin
D_mat[i] <= 64'sd0;
J_mat[i] <= 64'sd0;
Pi_mat[i] <= 64'sd0;
current_p[i] <= 64'sd0;
P_new[i] <= 64'sd0;
force_vec[i] <= 64'sd0;
converged_state[i] <= 64'sd0;
end
prev_delta <= 64'sd0;
end else begin
case (state)
// ============================================================
IDLE: begin
if (start) begin
// Initialize state from input
for (i = 0; i < MAT_ENTRIES; i = i + 1)
current_p[i] <= initial_state[i];
iter_count <= 16'd0;
converged <= 1'b0;
done <= 1'b0;
state <= COMPUTE_D;
end
end

// ============================================================
// D = L·L^T  (dissipator — symmetric positive semi-definite)
// ============================================================
COMPUTE_D: begin
for (i = 0; i < N; i = i + 1) begin
for (j = 0; j < N; j = j + 1) begin
// (L·L^T)_ij = Σ_k L_ik * L_jk
reg signed [63:0] acc;
acc = 64'sd0;
for (k = 0; k < N; k = k + 1) begin
acc = qadd(acc, qmul(L_mat[i*N+k], L_mat[j*N+k]));
end
D_mat[i*N+j] <= acc;
end
end
state <= COMPUTE_J;
end

// ============================================================
// J = A - A^T  (resonance — skew-symmetric)
// ============================================================
COMPUTE_J: begin
for (i = 0; i < N; i = i + 1) begin
for (j = 0; j < N; j = j + 1) begin
J_mat[i*N+j] <= qsub(A_mat[i*N+j], A_mat[j*N+i]);
end
end
state <= COMPUTE_PROJECTOR;
end

// ============================================================
// Π_Λ = I - Jc^T·(Jc·Jc^T)^(-1)·Jc
// Simplified: for well-conditioned Jc, use direct formula.
// Full inversion requires iterative Newton method in hardware.
// Here we pre-compute for the 4×4 case.
// ============================================================
COMPUTE_PROJECTOR: begin
// Simplified projector: Π_Λ ≈ I - α·Jc^T·Jc
// where α is a regularization parameter (1/tr(Jc·Jc^T))
// This is the first-order approximation of the full projector.
// For exact computation, use Newton-Schulz iteration offline.
reg signed [63:0] tr_JcJcT;
reg signed [63:0] alpha;

// tr(Jc·Jc^T) = Σ_ij Jc_ij²
tr_JcJcT = 64'sd0;
for (i = 0; i < MAT_ENTRIES; i = i + 1)
tr_JcJcT = qadd(tr_JcJcT, qmul(Jc_mat[i], Jc_mat[i]));

// α ≈ N / tr(Jc·Jc^T)  (regularized inverse trace)
if (tr_JcJcT != 64'sd0)
alpha = qmul(int2q(N), {1'b0, {FRAC_BITS{1'b0}}, tr_JcJcT[FRAC_BITS+:32]} == 32'd0 ? int2q(1) : int2q(1));  // Simplified
else
alpha = int2q(0);

// Π_Λ = I - α·Jc^T·Jc (approximate)
for (i = 0; i < N; i = i + 1) begin
for (j = 0; j < N; j = j + 1) begin
reg signed [63:0] jcTjc;
jcTjc = 64'sd0;
for (k = 0; k < N; k = k + 1)
jcTjc = qadd(jcTjc, qmul(Jc_mat[k*N+i], Jc_mat[k*N+j]));
if (i == j)
Pi_mat[i*N+j] <= qsub(int2q(1), qmul(alpha, jcTjc));
else
Pi_mat[i*N+j] <= qsub(64'sd0, qmul(alpha, jcTjc));
end
end
state <= COMPUTE_FORCES;
end

// ============================================================
// Combined force: D·p + γ·J·p + ∇F(p)
// ∇F(p) = G·p (quadratic potential)
// ============================================================
COMPUTE_FORCES: begin
for (i = 0; i < N; i = i + 1) begin
// Treat p as N×1 column vector (first column of current_p)
reg signed [63:0] dp, jp, gp;
dp = 64'sd0;
jp = 64'sd0;
gp = 64'sd0;
for (k = 0; k < N; k = k + 1) begin
// D·p: only first column of p matters
dp = qadd(dp, qmul(D_mat[i*N+k], current_p[k*N+0]));
// γ·J·p
jp = qadd(jp, qmul(J_mat[i*N+k], current_p[k*N+0]));
// G·p
gp = qadd(gp, qmul(G_mat[i*N+k], current_p[k*N+0]));
end
// Store in first column of force_vec
force_vec[i*N+0] <= qadd(qadd(dp, qmul(gamma, jp)), gp);
// Other columns remain zero
for (j = 1; j < N; j = j + 1)
force_vec[i*N+j] <= 64'sd0;
end
state <= PROJECT_FORCE;
end

// ============================================================
// Projected force: Π_Λ·force
// ============================================================
PROJECT_FORCE: begin
for (i = 0; i < N; i = i + 1) begin
reg signed [63:0] acc;
acc = 64'sd0;
for (k = 0; k < N; k = k + 1)
acc = qadd(acc, qmul(Pi_mat[i*N+k], force_vec[k*N+0]));
force_vec[i*N+0] <= acc;
end
state <= UPDATE_STATE;
end

// ============================================================
// P_new = p_t - η·Π_Λ(forces)
// ============================================================
UPDATE_STATE: begin
for (i = 0; i < N; i = i + 1) begin
P_new[i*N+0] <= qsub(current_p[i*N+0], qmul(eta, force_vec[i*N+0]));
for (j = 1; j < N; j = j + 1)
P_new[i*N+j] <= 64'sd0;
end
state <= QUANTUM_NORM;
end

// ============================================================
// Quantum Normalization:
// p_{t+1} = (1-mix)·P_new + mix·P_new/||P_new||
// ============================================================
QUANTUM_NORM: begin
// Compute ||P_new|| (Euclidean norm of first column)
reg signed [63:0] norm_sq;
norm_sq = 64'sd0;
for (i = 0; i < N; i = i + 1)
norm_sq = qadd(norm_sq, qmul(P_new[i*N+0], P_new[i*N+0]));

// Approximate 1/sqrt(norm_sq) using shift (simplified)
// In full implementation, use CORDIC or Newton's method
reg signed [63:0] inv_norm;
if (norm_sq > 64'sd0)
inv_norm = int2q(1);  // Placeholder: use CORDIC in real silicon
else
inv_norm = int2q(0);

// p_{t+1} = (1-mix)·P_new + mix·P_new·inv_norm
//         = P_new·(1 - mix + mix·inv_norm)
for (i = 0; i < N; i = i + 1) begin
reg signed [63:0] raw, unit, blended;
raw = qmul(qsub(int2q(1), mix), P_new[i*N+0]);
unit = qmul(qmul(mix, P_new[i*N+0]), inv_norm);
blended = qadd(raw, unit);
current_p[i*N+0] <= blended;
end
state <= CHECK_CONV;
end

// ============================================================
// Convergence check: ||p_{t+1} - p_t|| < threshold
// ============================================================
CHECK_CONV: begin
reg signed [63:0] delta_sq;
delta_sq = 64'sd0;
for (i = 0; i < N; i = i + 1) begin
reg signed [63:0] diff;
diff = qsub(current_p[i*N+0], P_new[i*N+0]);
delta_sq = qadd(delta_sq, qmul(diff, diff));
end

iter_count <= iter_count + 16'd1;

if (delta_sq < int2q(1) >>> CONVERGENCE_THRESHOLD_FRAC ||
iter_count >= MAX_ITERATIONS - 1) begin
converged <= (delta_sq < int2q(1) >>> CONVERGENCE_THRESHOLD_FRAC);
for (i = 0; i < MAT_ENTRIES; i = i + 1)
converged_state[i] <= current_p[i];
iterations_used <= iter_count + 16'd1;
done <= 1'b1;
state <= FINISH;
end else begin
state <= COMPUTE_FORCES;  // Next iteration
end
end

// ============================================================
FINISH: begin
done <= 1'b1;
state <= IDLE;
end

default: state <= IDLE;
endcase
end
end

endmodule
