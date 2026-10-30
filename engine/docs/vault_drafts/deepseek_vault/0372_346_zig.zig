//! ============================================================================
//! main.zig — Entry Point for POLER FPGA Parallel Computation Engine (f64)
//! ============================================================================
//! Demonstrates the full mathematical pipeline:
//!   1. Deformed Tensor Product: X ⊗_ε Y = (X·Y) + ε·(X⊙Y)   [Hadamard, NOT XOR]
//!   2. Dissipator D = L·L^T  (entropy burner)
//!   3. Resonance J = A - A^T  (skew-symmetric temporal echo)
//!   4. Logical Projector Π_Λ = I - Jc^T(Jc·Jc^T)^(-1)·Jc  (causality enforcer)
//!   5. POLER Cycle: discrete step + quantum normalization
//!   6. Archetype idempotent verification: a ⊗_ε a = a
//! ============================================================================

const std = @import("std");
const tensor = @import("tensor.zig");

const Mat4 = tensor.Mat4;
const deformedTensorProduct = tensor.deformedTensorProduct;
const dissipator = tensor.dissipator;
const resonance = tensor.resonance;
const logicalProjector = tensor.logicalProjector;
const polerDiscreteStep = tensor.polerDiscreteStep;
const quantumNormalize = tensor.quantumNormalize;
const polerCycle = tensor.polerCycle;
const PolerConfig = tensor.PolerConfig;
const verifyArchetypeIdempotent = tensor.verifyArchetypeIdempotent;
const verifyFixedPoint = tensor.verifyFixedPoint;

pub fn main() !void {
const stdout = std.io.getStdOut().writer();

try stdout.print("=== POLER Cryptographic Engine (f64, Hadamard, Continuous Phase Space) ===\n", .{});
try stdout.print("Mathematical basis: X ⊗_ε Y = (X·Y) + ε·(X⊙Y)\n", .{});
try stdout.print("No XOR. No bit logic. Pure f64 linear algebra.\n\n", .{});

// ========================================================================
// 1. Deformed Tensor Product: X ⊗_ε Y = (X·Y) + ε·(X⊙Y)
// ========================================================================
try stdout.print("--- 1. Deformed Tensor Product ---\n", .{});
try stdout.print("  X ⊗_ε Y = (X·Y) + ε·(X⊙Y)\n", .{});
try stdout.print("  X·Y = matrix multiplication (linear interaction)\n", .{});
try stdout.print("  X⊙Y = Hadamard product (element-wise, NOT XOR)\n\n", .{});

const X = Mat4.init(.{
.{ 1.0, 2.0, 0.0, 0.0 },
.{ 3.0, 4.0, 0.0, 0.0 },
.{ 0.0, 0.0, 1.0, 2.0 },
.{ 0.0, 0.0, 3.0, 4.0 },
});

const Y = Mat4.init(.{
.{ 5.0, 6.0, 0.0, 0.0 },
.{ 7.0, 8.0, 0.0, 0.0 },
.{ 0.0, 0.0, 5.0, 6.0 },
.{ 0.0, 0.0, 7.0, 8.0 },
});

const epsilon: f64 = 0.1;

X.print("X");
Y.print("Y");
try stdout.print("  ε = {d}\n\n", .{epsilon});

const tensor_result = deformedTensorProduct(4, X, Y, epsilon);
tensor_result.print("X ⊗_ε Y");

// Show decomposition
const linear = Mat4.matmul(X, Y);
const nonlinear = Mat4.hadamard(X, Y);
const deform = Mat4.scale(nonlinear, epsilon);
linear.print("  X·Y (linear)");
nonlinear.print("  X⊙Y (Hadamard)");
deform.print("  ε·(X⊙Y) (deformation)");
try stdout.print("\n", .{});

// ========================================================================
// 2. Dissipator: D = L·L^T
// ========================================================================
try stdout.print("--- 2. Dissipator: D = L·L^T (entropy burner) ---\n", .{});

const L = Mat4.init(.{
.{ 0.5, 0.0, 0.0, 0.0 },
.{ 0.1, 0.4, 0.0, 0.0 },
.{ 0.0, 0.05, 0.3, 0.0 },
.{ 0.0, 0.0, 0.02, 0.2 },
});

const D = dissipator(4, L);
D.print("D = L·L^T");
try stdout.print("  D is symmetric positive semi-definite by construction\n\n", .{});

// ========================================================================
// 3. Resonance: J = A - A^T
// ========================================================================
try stdout.print("--- 3. Resonance: J = A - A^T (skew-symmetric temporal echo) ---\n", .{});

const A_res = Mat4.init(.{
.{ 0.0, 0.5, 0.0, 0.0 },
.{ -0.5, 0.0, 0.3, 0.0 },
.{ 0.0, -0.3, 0.0, 0.1 },
.{ 0.0, 0.0, -0.1, 0.0 },
});

const J = resonance(4, A_res);
J.print("J = A - A^T");
try stdout.print("  J is skew-symmetric: J^T = -J\n", .{});
try stdout.print("  Eigenvalues are purely imaginary → periodic orbits\n\n", .{});

// ========================================================================
// 4. Logical Projector: Π_Λ = I - Jc^T(Jc·Jc^T)^(-1)·Jc
// ========================================================================
try stdout.print("--- 4. Logical Projector: Π_Λ (causality enforcer) ---\n", .{});

const Jc = Mat4.init(.{
.{ 0.1, 0.0, 0.0, 0.0 },
.{ 0.0, 0.1, 0.0, 0.0 },
.{ 0.0, 0.0, 0.1, 0.0 },
.{ 0.0, 0.0, 0.0, 0.1 },
});

const Pi = logicalProjector(4, Jc);
Pi.print("Π_Λ");
try stdout.print("  Π_Λ is idempotent: Π_Λ² = Π_Λ\n\n", .{});

// ========================================================================
// 5. POLER Cycle: discrete step + quantum normalization
// ========================================================================
try stdout.print("--- 5. POLER Cycle (attractor search) ---\n", .{});
try stdout.print("  P_new = p_t - η·Π_Λ(D·p_t + γ·J·p_t + ∇F)\n", .{});
try stdout.print("  p_{{t+1}} = (1-mix) P_new + mix P_new/||P_new||\n\n", .{});

const G = Mat4.scale(Mat4.identity(), 0.01); // Diagonal gradient matrix

const p0 = Mat4.init(.{
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 0.5, 0.0, 0.0, 0.0 },
.{ 0.3, 0.0, 0.0, 0.0 },
.{ 0.1, 0.0, 0.0, 0.0 },
});

p0.print("p_0 (initial state)");

// Show single step
const P_new = polerDiscreteStep(4, L, A_res, Jc, G, p0, 0.05, 0.1);
P_new.print("P_new (1 step)");

const p1 = quantumNormalize(4, P_new, 0.1);
p1.print("p_1 (after quantum normalization)");

const config = PolerConfig{
.eta = 0.05,
.gamma = 0.1,
.mix = 0.05,
.max_iterations = 5000,
.tolerance = 1e-8,
};

const result = polerCycle(L, A_res, Jc, G, p0, config);
result.state.print("p* (attractor)");
try stdout.print("  Converged: {} in {d} iterations (delta={d:.6})\n\n", .{ result.converged, result.iterations, result.final_delta });

// ========================================================================
// 6. Archetype Idempotent Verification
// ========================================================================
try stdout.print("--- 6. Archetype Idempotent: a ⊗_ε a = a ---\n", .{});

// Projection matrix is idempotent under standard multiplication
// P·P = P, so with ε=0: P ⊗_0 P = P·P = P ✓
const archetype = Mat4.init(.{
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.0 },
.{ 0.0, 0.0, 1.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.0 },
});

const is_idempotent_0 = verifyArchetypeIdempotent(4, archetype, 0.0, 1e-10);
try stdout.print("  a tensor_0 a = a: {} (projection matrix, eps=0)\n", .{is_idempotent_0});

const is_idempotent_eps = verifyArchetypeIdempotent(4, archetype, 0.1, 1e-6);
try stdout.print("  a tensor_0.1 a = a: {} (with deformation)\n\n", .{is_idempotent_eps});

// ========================================================================
// 7. Batch Tensor Product Computation
// ========================================================================
try stdout.print("--- 7. Parallel Batch Computation ---\n", .{});

const eps_values = [_]f64{ 0.0, 0.01, 0.1, 0.5, 1.0 };
for (eps_values, 0..) |eps, i| {
const r = deformedTensorProduct(4, X, Y, eps);
const norm = Mat4.frobeniusNorm(r);
try stdout.print("  [{d}] X tensor_{d:.2} Y: ||result||_F = {d:.6}\n", .{ i, eps, norm });
}

try stdout.print("\nDone. All computations use f64 (double precision).\n", .{});
}
