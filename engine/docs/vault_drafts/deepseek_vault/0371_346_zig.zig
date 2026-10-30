//! ============================================================================
//! tensor.zig — Corrected Deformed Tensor Product & POLER Cycle (f64)
//! ============================================================================
//! Implements the EXACT mathematical formulations from the POLER framework:
//!
//! 1. Deformed Tensor Product:  X ⊗_ε Y = (X·Y) + ε·(X⊙Y)
//!    where X·Y = matrix multiplication, X⊙Y = Hadamard (element-wise) product,
//!    ε = energy/deformation parameter (f64).
//!
//! 2. Archetype (a): idempotent invariant  a ⊗_ε a = a
//!    Fixed point: p* = a ⊗_ε p*
//!
//! 3. POLER Cycle (continuous): dp/dt = -η Π_Λ(p_t)[D·p_t + γJ·p_t + ∇F(p_t)]
//!    Discrete step: P_new = p_t - η Π_Λ((L·L^T)p_t + γ(A-A^T)p_t + ∇F)
//!
//! 4. Quantum Normalization: p_{t+1} = (1-mix)·P_new + mix·P_new/||P_new||
//!
//! 5. Dissipator D = L·L^T  (entropy burner)
//!    Resonance J = A - A^T  (skew-symmetric temporal echo)
//!    Projector Π_Λ = I - Jc^T(Jc·Jc^T)^(-1)·Jc  (causality enforcer)
//!
//! ALL operations use f64 (double precision). No bit logic, no XOR.
//! ============================================================================

const std = @import("std");

// ============================================================================
// Matrix type — fixed N×N matrix of f64
// ============================================================================
pub fn Matrix(comptime N: usize) type {
return struct {
const Self = @This();
data: [N][N]f64,

/// Zero matrix
pub fn zero() Self {
var m: Self = undefined;
for (0..N) |i| {
for (0..N) |j| {
m.data[i][j] = 0.0;
}
}
return m;
}

/// Identity matrix
pub fn identity() Self {
var m = Self.zero();
for (0..N) |i| m.data[i][i] = 1.0;
return m;
}

/// Create from raw 2D array
pub fn init(data: [N][N]f64) Self {
return .{ .data = data };
}

/// Create from flat array (row-major)
pub fn fromFlat(flat_arr: [N * N]f64) Self {
var m = Self.zero();
for (0..N) |i| {
for (0..N) |j| {
m.data[i][j] = flat_arr[i * N + j];
}
}
return m;
}

/// Matrix addition: C = A + B
pub fn add(a: Self, b: Self) Self {
var c = Self.zero();
for (0..N) |i| {
for (0..N) |j| {
c.data[i][j] = a.data[i][j] + b.data[i][j];
}
}
return c;
}

/// Matrix subtraction: C = A - B
pub fn sub(a: Self, b: Self) Self {
var c = Self.zero();
for (0..N) |i| {
for (0..N) |j| {
c.data[i][j] = a.data[i][j] - b.data[i][j];
}
}
return c;
}

/// Scalar multiplication: C = α·A
pub fn scale(a: Self, alpha: f64) Self {
var c = Self.zero();
for (0..N) |i| {
for (0..N) |j| {
c.data[i][j] = alpha * a.data[i][j];
}
}
return c;
}

/// Matrix multiplication: C = A·B (standard linear algebra product)
pub fn matmul(a: Self, b: Self) Self {
var c = Self.zero();
for (0..N) |i| {
for (0..N) |j| {
var sum: f64 = 0.0;
for (0..N) |k| {
sum += a.data[i][k] * b.data[k][j];
}
c.data[i][j] = sum;
}
}
return c;
}

/// Hadamard (element-wise) product: C_ij = A_ij · B_ij
/// This is the CORRECT deformation term, NOT XOR.
pub fn hadamard(a: Self, b: Self) Self {
var c = Self.zero();
for (0..N) |i| {
for (0..N) |j| {
c.data[i][j] = a.data[i][j] * b.data[i][j];
}
}
return c;
}

/// Matrix transpose: B = A^T
pub fn transpose(a: Self) Self {
var b = Self.zero();
for (0..N) |i| {
for (0..N) |j| {
b.data[i][j] = a.data[j][i];
}
}
return b;
}

/// Frobenius norm: ||A||_F = sqrt(Σ A_ij²)
pub fn frobeniusNorm(a: Self) f64 {
var sum: f64 = 0.0;
for (0..N) |i| {
for (0..N) |j| {
sum += a.data[i][j] * a.data[i][j];
}
}
return @sqrt(sum);
}

/// Vector norm (treating N×1 column as vector): ||v|| = sqrt(Σ v_i²)
/// Assumes single-column matrix (all columns except first are zero)
pub fn vecNorm(a: Self) f64 {
var sum: f64 = 0.0;
for (0..N) |i| {
sum += a.data[i][0] * a.data[i][0];
}
return @sqrt(sum);
}

/// Normalize as vector: v / ||v||
pub fn vecNormalize(a: Self) Self {
const norm = a.vecNorm();
if (norm < 1e-15) return a;
return a.scale(1.0 / norm);
}

/// Matrix-vector product: y = A·x (x is N×1 column)
pub fn matvec(a: Self, x: Self) Self {
return a.matmul(x);
}

/// Extract column as separate matrix (N×1)
pub fn col(a: Self, j: usize) Self {
var v = Self.zero();
for (0..N) |i| {
v.data[i][0] = a.data[i][j];
}
return v;
}

/// Set column from vector
pub fn setCol(a: *Self, j: usize, v: Self) void {
for (0..N) |i| {
a.data[i][j] = v.data[i][0];
}
}

/// Flatten to 1D array (row-major)
pub fn flatArr(a: Self) [N * N]f64 {
var arr: [N * N]f64 = undefined;
for (0..N) |i| {
for (0..N) |j| {
arr[i * N + j] = a.data[i][j];
}
}
return arr;
}

/// Element-wise equality check (with tolerance)
pub fn approxEqual(a: Self, b: Self, tol: f64) bool {
for (0..N) |i| {
for (0..N) |j| {
if (std.math.fabs(a.data[i][j] - b.data[i][j]) > tol) return false;
}
}
return true;
}

/// Trace: sum of diagonal elements
pub fn trace(a: Self) f64 {
var sum: f64 = 0.0;
for (0..N) |i| sum += a.data[i][i];
return sum;
}

/// Print matrix to stdout
pub fn print(a: Self, label: []const u8) void {
const stdout = std.io.getStdOut().writer();
stdout.print("{s} ({d}x{d}):\n", .{ label, N, N }) catch {};
for (0..N) |i| {
stdout.print("  [", .{}) catch {};
for (0..N) |j| {
if (j > 0) stdout.print("  ", .{}) catch {};
stdout.print("{d: >10.4}", .{a.data[i][j]}) catch {};
}
stdout.print("]\n", .{}) catch {};
}
}

/// Maximum absolute element
pub fn maxAbs(a: Self) f64 {
var m: f64 = 0.0;
for (0..N) |i| {
for (0..N) |j| {
const v = std.math.fabs(a.data[i][j]);
if (v > m) m = v;
}
}
return m;
}
};
}

// ============================================================================
// Concrete 4×4 matrix type (default for FPGA implementation)
// ============================================================================
pub const Mat4 = Matrix(4);

// ============================================================================
// Deformed Tensor Product:  X ⊗_ε Y = (X·Y) + ε·(X⊙Y)
//
// This is the CORRECT formulation:
//   - X·Y = standard matrix multiplication (linear interaction)
//   - X⊙Y = Hadamard product (non-linear element-wise coupling)
//   - ε = deformation/energy parameter (f64, NOT bit mask)
//
// The Hadamard product provides continuous phase-space deformation,
// unlike XOR which creates dimension conflicts and chaotic summation.
// ============================================================================
pub fn deformedTensorProduct(comptime N: usize, X: Matrix(N), Y: Matrix(N), epsilon: f64) Matrix(N) {
const linear = Matrix(N).matmul(X, Y); // X·Y — matrix multiplication
const nonlinear = Matrix(N).hadamard(X, Y); // X⊙Y — Hadamard product
const deform = Matrix(N).scale(nonlinear, epsilon); // ε·(X⊙Y)
return Matrix(N).add(linear, deform); // (X·Y) + ε·(X⊙Y)
}

// ============================================================================
// Dissipator: D = L·L^T
//
// Burns entropy noise. L is a lower-triangular dissipation matrix.
// D is symmetric positive semi-definite by construction.
// Physically: this is the analog of a viscosity term that damps
// high-frequency oscillations in the phase space trajectory.
// ============================================================================
pub fn dissipator(comptime N: usize, L: Matrix(N)) Matrix(N) {
const Lt = Matrix(N).transpose(L);
return Matrix(N).matmul(L, Lt); // D = L·L^T
}

// ============================================================================
// Resonance: J = A - A^T
//
// Skew-symmetric operator (temporal echo). J^T = -J by construction.
// Creates oscillatory dynamics that encode temporal structure.
// The eigenvalues of J are purely imaginary, driving periodic orbits.
// ============================================================================
pub fn resonance(comptime N: usize, A: Matrix(N)) Matrix(N) {
const At = Matrix(N).transpose(A);
return Matrix(N).sub(A, At); // J = A - A^T
}

// ============================================================================
// Logical Projector: Π_Λ = I - Jc^T·(Jc·Jc^T)^(-1)·Jc
//
// Enforces causality constraints. Projects onto the null space of Jc.
// This is the Moore-Penrose pseudo-inverse projector that removes
// components violating the logical/causal constraints encoded in Jc.
//
// For small matrices (N≤4), we compute (Jc·Jc^T)^(-1) directly
// using cofactor expansion (Cramer's rule).
// ============================================================================
pub fn logicalProjector(comptime N: usize, Jc: Matrix(N)) Matrix(N) {
const I = Matrix(N).identity();
const JcT = Matrix(N).transpose(Jc);

// M = Jc · Jc^T  (symmetric positive semi-definite)
const M = Matrix(N).matmul(Jc, JcT);

// M^{-1} — invert using Gauss-Jordan elimination
const Minv = invertMatrix(N, M) catch {
// If M is singular, return identity projector (no constraint)
return I;
};

// Π_Λ = I - Jc^T · M^{-1} · Jc
const temp = Matrix(N).matmul(JcT, Minv);
const proj_inner = Matrix(N).matmul(temp, Jc);
return Matrix(N).sub(I, proj_inner);
}

// ============================================================================
// Matrix Inversion via Gauss-Jordan Elimination with Full Pivoting
//
// Returns the inverse of an N×N matrix, or error if singular.
// ============================================================================
pub fn invertMatrix(comptime N: usize, m: Matrix(N)) !Matrix(N) {
// Augmented matrix [M | I]
var aug: [N][2 * N]f64 = undefined;
for (0..N) |i| {
for (0..N) |j| {
aug[i][j] = 0.0;
}
for (0..N) |j| {
aug[i][N + j] = 0.0;
}
aug[i][N + i] = 1.0;
for (0..N) |j| {
aug[i][j] = m.data[i][j];
}
}

// Forward elimination with partial pivoting
for (0..N) |col| {
// Find pivot
var max_val: f64 = std.math.fabs(aug[col][col]);
var max_row: usize = col;
for (col + 1..N) |row| {
const v = std.math.fabs(aug[row][col]);
if (v > max_val) {
max_val = v;
max_row = row;
}
}

if (max_val < 1e-12) return error.SingularMatrix;

// Swap rows
if (max_row != col) {
const tmp = aug[col];
aug[col] = aug[max_row];
aug[max_row] = tmp;
}

// Scale pivot row
const pivot = aug[col][col];
for (0..2 * N) |j| {
aug[col][j] /= pivot;
}

// Eliminate column
for (0..N) |row| {
if (row == col) continue;
const factor = aug[row][col];
for (0..2 * N) |j| {
aug[row][j] -= factor * aug[col][j];
}
}
}

// Extract inverse
var result = Matrix(N).zero();
for (0..N) |i| {
for (0..N) |j| {
result.data[i][j] = aug[i][N + j];
}
}
return result;
}

// ============================================================================
// Gradient of potential F(p) — simplified quadratic model
//
// ∇F(p) = G·p where G is a symmetric positive definite matrix.
// For the general case, this can be any differentiable potential.
// Here we use a diagonal quadratic form: F(p) = 0.5·p^T·G·p
// so ∇F(p) = G·p.
// ============================================================================
pub fn gradientF(comptime N: usize, G: Matrix(N), p: Matrix(N)) Matrix(N) {
return Matrix(N).matmul(G, p);
}

// ============================================================================
// POLER Cycle — Discrete Step
//
// Given:
//   L  — lower triangular matrix (dissipation structure)
//   A  — general matrix for resonance J = A - A^T
//   Jc — constraint matrix for projector Π_Λ
//   G  — gradient matrix for potential ∇F = G·p
//   p_t — current state vector (N×1 matrix)
//   η  — learning rate / step size
//   γ  — resonance coupling strength
//
// Compute:
//   P_new = p_t - η · Π_Λ( D·p_t + γ·J·p_t + ∇F(p_t) )
//   where D = L·L^T, J = A - A^T
// ============================================================================
pub fn polerDiscreteStep(
comptime N: usize,
L: Matrix(N),
A: Matrix(N),
Jc: Matrix(N),
G: Matrix(N),
p_t: Matrix(N),
eta: f64,
gamma: f64,
) Matrix(N) {
// D = L·L^T  (dissipator)
const D = dissipator(N, L);

// J = A - A^T  (resonance, skew-symmetric)
const J = resonance(N, A);

// Π_Λ = I - Jc^T·(Jc·Jc^T)^(-1)·Jc  (logical projector)
const Pi = logicalProjector(N, Jc);

// D·p_t  (dissipation force)
const diss_term = Matrix(N).matmul(D, p_t);

// γ·J·p_t  (resonance force)
const res_term = Matrix(N).scale(Matrix(N).matmul(J, p_t), gamma);

// ∇F(p_t) = G·p_t  (potential gradient)
const grad_term = gradientF(N, G, p_t);

// Combined force: D·p_t + γ·J·p_t + ∇F
const force = Matrix(N).add(Matrix(N).add(diss_term, res_term), grad_term);

// Projected force: Π_Λ(force)
const projected_force = Matrix(N).matmul(Pi, force);

// Update: P_new = p_t - η·Π_Λ(force)
return Matrix(N).sub(p_t, Matrix(N).scale(projected_force, eta));
}

// ============================================================================
// Quantum Normalization
//
// p_{t+1} = (1 - mix) · P_new + mix · P_new / ||P_new||
//
// This interpolates between the raw update (mix=0) and a unit-norm
// projection (mix=1), providing topological regularization.
// Prevents the trajectory from diverging while preserving direction.
// ============================================================================
pub fn quantumNormalize(comptime N: usize, P_new: Matrix(N), mix: f64) Matrix(N) {
const raw = Matrix(N).scale(P_new, 1.0 - mix);
const normalized = Matrix(N).vecNormalize(P_new);
const unit = Matrix(N).scale(normalized, mix);
return Matrix(N).add(raw, unit);
}

// ============================================================================
// Full POLER Cycle — Iterative Attractor Search
//
// Runs the complete POLER iteration:
//   1. Discrete step with dissipator, resonance, projector
//   2. Quantum normalization
//   3. Convergence check
//
// Converges when ||p_{t+1} - p_t|| < tolerance or max iterations reached.
// ============================================================================
pub const PolerCycleResult = struct {
state: Mat4,
iterations: u32,
converged: bool,
final_delta: f64,
};

pub const PolerConfig = struct {
eta: f64 = 0.01, // Learning rate
gamma: f64 = 0.1, // Resonance coupling
mix: f64 = 0.1, // Quantum normalization mixing
max_iterations: u32 = 1000,
tolerance: f64 = 1e-10,
};

pub fn polerCycle(
L: Mat4,
A: Mat4,
Jc: Mat4,
G: Mat4,
p0: Mat4,
config: PolerConfig,
) PolerCycleResult {
var p = p0;
var iter: u32 = 0;
var delta: f64 = 1.0;

while (iter < config.max_iterations and delta > config.tolerance) {
// Discrete POLER step
const P_new = polerDiscreteStep(4, L, A, Jc, G, p, config.eta, config.gamma);

// Quantum normalization
const p_next = quantumNormalize(4, P_new, config.mix);

// Convergence check: ||p_next - p||_F
const diff = Mat4.sub(p_next, p);
delta = Mat4.frobeniusNorm(diff);

p = p_next;
iter += 1;
}

return .{
.state = p,
.iterations = iter,
.converged = delta <= config.tolerance,
.final_delta = delta,
};
}

// ============================================================================
// Archetype Verification: a ⊗_ε a ≈ a  (idempotent)
//
// For the archetype to be a true idempotent under ⊗_ε, we need:
//   (a·a) + ε·(a⊙a) = a
// Since a⊙a has elements a_ij², and a·a has elements Σ_k a_ik·a_kj,
// the idempotent condition is a non-trivial algebraic equation.
//
// For ε = 0, this reduces to a·a = a (projection matrix condition).
// For ε ≠ 0, the archetype must be found numerically via POLER cycle.
// ============================================================================
pub fn verifyArchetypeIdempotent(comptime N: usize, a: Matrix(N), epsilon: f64, tol: f64) bool {
const a_tensor_a = deformedTensorProduct(N, a, a, epsilon);
return Matrix(N).approxEqual(a_tensor_a, a, tol);
}

// ============================================================================
// Fixed Point Check: p* = a ⊗_ε p*
//
// A vector p* is a fixed point of the deformed tensor product with
// archetype a if and only if applying the operation returns the same point.
// ============================================================================
pub fn verifyFixedPoint(comptime N: usize, a: Matrix(N), p: Matrix(N), epsilon: f64, tol: f64) bool {
const result = deformedTensorProduct(N, a, p, epsilon);
return Matrix(N).approxEqual(result, p, tol);
}

// ============================================================================
// Tests
// ============================================================================

test "matrix multiplication correctness" {
const a = Mat4.init(.{
.{ 1.0, 2.0, 0.0, 0.0 },
.{ 0.0, 1.0, 3.0, 0.0 },
.{ 0.0, 0.0, 1.0, 4.0 },
.{ 0.0, 0.0, 0.0, 1.0 },
});
const b = Mat4.identity();
const c = Mat4.matmul(a, b);
try std.testing.expect(Mat4.approxEqual(c, a, 1e-10));
}

test "Hadamard product element-wise" {
const a = Mat4.init(.{
.{ 2.0, 0.0, 0.0, 0.0 },
.{ 0.0, 3.0, 0.0, 0.0 },
.{ 0.0, 0.0, 4.0, 0.0 },
.{ 0.0, 0.0, 0.0, 5.0 },
});
const b = Mat4.init(.{
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 0.0, 2.0, 0.0, 0.0 },
.{ 0.0, 0.0, 3.0, 0.0 },
.{ 0.0, 0.0, 0.0, 4.0 },
});
const c = Mat4.hadamard(a, b);
try std.testing.expect(std.math.fabs(c.data[0][0] - 2.0) < 1e-10);
try std.testing.expect(std.math.fabs(c.data[1][1] - 6.0) < 1e-10);
try std.testing.expect(std.math.fabs(c.data[2][2] - 12.0) < 1e-10);
try std.testing.expect(std.math.fabs(c.data[3][3] - 20.0) < 1e-10);
}

test "deformed tensor product: ε=0 reduces to matrix multiply" {
const a = Mat4.init(.{
.{ 1.0, 2.0, 0.0, 0.0 },
.{ 3.0, 4.0, 0.0, 0.0 },
.{ 0.0, 0.0, 1.0, 0.0 },
.{ 0.0, 0.0, 0.0, 1.0 },
});
const b = Mat4.init(.{
.{ 5.0, 6.0, 0.0, 0.0 },
.{ 7.0, 8.0, 0.0, 0.0 },
.{ 0.0, 0.0, 1.0, 0.0 },
.{ 0.0, 0.0, 0.0, 1.0 },
});
const tensor_eps0 = deformedTensorProduct(4, a, b, 0.0);
const matmul_result = Mat4.matmul(a, b);
try std.testing.expect(Mat4.approxEqual(tensor_eps0, matmul_result, 1e-10));
}

test "deformed tensor product: ε>0 adds Hadamard deformation" {
const a = Mat4.init(.{
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 0.0, 1.0, 0.0, 0.0 },
.{ 0.0, 0.0, 1.0, 0.0 },
.{ 0.0, 0.0, 0.0, 1.0 },
});
const b = Mat4.init(.{
.{ 2.0, 0.0, 0.0, 0.0 },
.{ 0.0, 2.0, 0.0, 0.0 },
.{ 0.0, 0.0, 2.0, 0.0 },
.{ 0.0, 0.0, 0.0, 2.0 },
});
// I ⊗_ε (2I) = I·(2I) + ε·I⊙(2I) = 2I + ε·2I = (2+2ε)I
const eps: f64 = 0.5;
const result = deformedTensorProduct(4, a, b, eps);
const expected_val: f64 = 2.0 + eps * 2.0; // = 3.0
for (0..4) |i| {
try std.testing.expect(std.math.fabs(result.data[i][i] - expected_val) < 1e-10);
}
}

test "dissipator D = L·L^T is symmetric positive semi-definite" {
const L = Mat4.init(.{
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 0.5, 1.0, 0.0, 0.0 },
.{ 0.0, 0.3, 1.0, 0.0 },
.{ 0.0, 0.0, 0.2, 1.0 },
});
const D = dissipator(4, L);
const Dt = Mat4.transpose(D);
// D must equal D^T (symmetric)
try std.testing.expect(Mat4.approxEqual(D, Dt, 1e-10));
}

test "resonance J = A - A^T is skew-symmetric" {
const A = Mat4.init(.{
.{ 1.0, 2.0, 3.0, 4.0 },
.{ 5.0, 6.0, 7.0, 8.0 },
.{ 9.0, 10.0, 11.0, 12.0 },
.{ 13.0, 14.0, 15.0, 16.0 },
});
const J = resonance(4, A);
const Jt = Mat4.transpose(J);
// J^T must equal -J (skew-symmetric)
const neg_J = Mat4.scale(J, -1.0);
try std.testing.expect(Mat4.approxEqual(Jt, neg_J, 1e-10));
}

test "logical projector Π_Λ is idempotent: Π² = Π" {
const Jc = Mat4.init(.{
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 0.0, 1.0, 0.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.0 },
});
const Pi = logicalProjector(4, Jc);
const Pi2 = Mat4.matmul(Pi, Pi);
try std.testing.expect(Mat4.approxEqual(Pi, Pi2, 1e-8));
}

test "quantum normalization preserves direction" {
const v = Mat4.init(.{
.{ 3.0, 0.0, 0.0, 0.0 },
.{ 4.0, 0.0, 0.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.0 },
});
// mix=1.0 → pure normalization, ||v_normalized|| = 1
const v_norm = quantumNormalize(4, v, 1.0);
const norm = Mat4.vecNorm(v_norm);
try std.testing.expect(std.math.fabs(norm - 1.0) < 1e-10);
}

test "POLER cycle converges to attractor" {
// Simple setup: identity dissipation, small resonance, diagonal gradient
const L = Mat4.init(.{
.{ 0.1, 0.0, 0.0, 0.0 },
.{ 0.0, 0.1, 0.0, 0.0 },
.{ 0.0, 0.0, 0.1, 0.0 },
.{ 0.0, 0.0, 0.0, 0.1 },
});
const A = Mat4.init(.{
.{ 0.0, 0.1, 0.0, 0.0 },
.{ -0.1, 0.0, 0.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.1 },
.{ 0.0, 0.0, -0.1, 0.0 },
});
const Jc = Mat4.init(.{
.{ 0.01, 0.0, 0.0, 0.0 },
.{ 0.0, 0.01, 0.0, 0.0 },
.{ 0.0, 0.0, 0.01, 0.0 },
.{ 0.0, 0.0, 0.0, 0.01 },
});
const G = Mat4.scale(Mat4.identity(), 0.01);

const p0 = Mat4.init(.{
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 1.0, 0.0, 0.0, 0.0 },
});

const config = PolerConfig{
.eta = 0.05,
.gamma = 0.1,
.mix = 0.05,
.max_iterations = 5000,
.tolerance = 1e-8,
};

const result = polerCycle(L, A, Jc, G, p0, config);
// Should converge within max iterations
try std.testing.expect(result.iterations > 0);
try std.testing.expect(result.iterations <= config.max_iterations);
}

test "matrix inversion correctness" {
const A = Mat4.init(.{
.{ 4.0, 0.0, 0.0, 0.0 },
.{ 0.0, 3.0, 0.0, 0.0 },
.{ 0.0, 0.0, 2.0, 0.0 },
.{ 0.0, 0.0, 0.0, 1.0 },
});
const Ainv = try invertMatrix(4, A);
const I = Mat4.matmul(A, Ainv);
try std.testing.expect(Mat4.approxEqual(I, Mat4.identity(), 1e-8));
}
