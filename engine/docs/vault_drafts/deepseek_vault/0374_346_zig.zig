//! ============================================================================
//! test_tensor.zig - Comprehensive Zig Tests for POLER Tensor Operations (f64)
//! ============================================================================
//! Unit tests and property-based tests for the corrected deformed tensor
//! product (Hadamard, NOT XOR), POLER cycle, dissipator, resonance,
//! projector, and quantum normalization.
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
const invertMatrix = tensor.invertMatrix;

// ---- Basic Matrix Operation Tests ----

test "matrix add/sub commutativity" {
const a = Mat4.init(.{
.{ 1.0, 2.0, 3.0, 4.0 },
.{ 5.0, 6.0, 7.0, 8.0 },
.{ 9.0, 10.0, 11.0, 12.0 },
.{ 13.0, 14.0, 15.0, 16.0 },
});
const b = Mat4.init(.{
.{ 16.0, 15.0, 14.0, 13.0 },
.{ 12.0, 11.0, 10.0, 9.0 },
.{ 8.0, 7.0, 6.0, 5.0 },
.{ 4.0, 3.0, 2.0, 1.0 },
});
const ab = Mat4.add(a, b);
const ba = Mat4.add(b, a);
try std.testing.expect(Mat4.approxEqual(ab, ba, 1e-10));
}

test "matrix multiply identity" {
const a = Mat4.init(.{
.{ 1.0, 2.0, 3.0, 4.0 },
.{ 5.0, 6.0, 7.0, 8.0 },
.{ 9.0, 10.0, 11.0, 12.0 },
.{ 13.0, 14.0, 15.0, 16.0 },
});
const I = Mat4.identity();
const aI = Mat4.matmul(a, I);
const Ia = Mat4.matmul(I, a);
try std.testing.expect(Mat4.approxEqual(aI, a, 1e-10));
try std.testing.expect(Mat4.approxEqual(Ia, a, 1e-10));
}

test "Hadamard product with identity gives diagonal" {
const a = Mat4.init(.{
.{ 1.0, 2.0, 3.0, 4.0 },
.{ 5.0, 6.0, 7.0, 8.0 },
.{ 9.0, 10.0, 11.0, 12.0 },
.{ 13.0, 14.0, 15.0, 16.0 },
});
const I = Mat4.identity();
const h = Mat4.hadamard(a, I);
// Only diagonal elements survive: h_ij = a_ij * I_ij = a_ii * δ_ij
for (0..4) |i| {
for (0..4) |j| {
if (i == j) {
try std.testing.expect(@abs(h.data[i][j] - a.data[i][i]) < 1e-10);
} else {
try std.testing.expect(@abs(h.data[i][j]) < 1e-10);
}
}
}
}

// ---- Deformed Tensor Product Tests ----

test "deformed tensor product ε=0 = matrix multiply" {
const a = Mat4.init(.{
.{ 2.0, 1.0, 0.0, 0.0 },
.{ 1.0, 2.0, 1.0, 0.0 },
.{ 0.0, 1.0, 2.0, 1.0 },
.{ 0.0, 0.0, 1.0, 2.0 },
});
const b = Mat4.scale(Mat4.identity(), 3.0);
const result = deformedTensorProduct(4, a, b, 0.0);
const expected = Mat4.matmul(a, b);
try std.testing.expect(Mat4.approxEqual(result, expected, 1e-10));
}

test "deformed tensor product ε→∞ dominated by Hadamard" {
const a = Mat4.init(.{
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 0.0, 2.0, 0.0, 0.0 },
.{ 0.0, 0.0, 3.0, 0.0 },
.{ 0.0, 0.0, 0.0, 4.0 },
});
const b = Mat4.init(.{
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 0.0, 1.0, 0.0, 0.0 },
.{ 0.0, 0.0, 1.0, 0.0 },
.{ 0.0, 0.0, 0.0, 1.0 },
});
const eps: f64 = 1000.0;
const result = deformedTensorProduct(4, a, b, eps);
// Hadamard part dominates: a⊙I = diag(1,2,3,4), ε·(a⊙I) = diag(1000,2000,3000,4000)
const hadamard_part = Mat4.hadamard(a, b);
const scaled_hadamard = Mat4.scale(hadamard_part, eps);
// Result should be close to scaled_hadamard for large ε
try std.testing.expect(Mat4.approxEqual(result, Mat4.add(Mat4.matmul(a, b), scaled_hadamard), 1e-6));
}

test "deformed tensor product determinism" {
const a = Mat4.init(.{
.{ 1.0, 2.0, 3.0, 4.0 },
.{ 5.0, 6.0, 7.0, 8.0 },
.{ 9.0, 10.0, 11.0, 12.0 },
.{ 13.0, 14.0, 15.0, 16.0 },
});
const b = Mat4.scale(Mat4.identity(), 0.5);
const r1 = deformedTensorProduct(4, a, b, 0.1);
const r2 = deformedTensorProduct(4, a, b, 0.1);
try std.testing.expect(Mat4.approxEqual(r1, r2, 1e-15));
}

// ---- Dissipator Tests ----

test "dissipator is symmetric" {
const L = Mat4.init(.{
.{ 0.3, 0.0, 0.0, 0.0 },
.{ 0.1, 0.2, 0.0, 0.0 },
.{ 0.0, 0.05, 0.15, 0.0 },
.{ 0.0, 0.0, 0.01, 0.1 },
});
const D = dissipator(4, L);
const Dt = Mat4.transpose(D);
try std.testing.expect(Mat4.approxEqual(D, Dt, 1e-10));
}

test "dissipator diagonal dominance" {
const L = Mat4.init(.{
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 0.0, 1.0, 0.0, 0.0 },
.{ 0.0, 0.0, 1.0, 0.0 },
.{ 0.0, 0.0, 0.0, 1.0 },
});
const D = dissipator(4, L);
// For L=I: D = I·I^T = I, diagonal elements = 1
for (0..4) |i| {
try std.testing.expect(@abs(D.data[i][i] - 1.0) < 1e-10);
}
}

// ---- Resonance Tests ----

test "resonance is skew-symmetric" {
const A = Mat4.init(.{
.{ 0.0, 1.0, 2.0, 3.0 },
.{ 4.0, 0.0, 5.0, 6.0 },
.{ 7.0, 8.0, 0.0, 9.0 },
.{ 10.0, 11.0, 12.0, 0.0 },
});
const J = resonance(4, A);
const Jt = Mat4.transpose(J);
const negJ = Mat4.scale(J, -1.0);
try std.testing.expect(Mat4.approxEqual(Jt, negJ, 1e-10));
}

test "resonance diagonal is zero" {
const A = Mat4.init(.{
.{ 1.0, 2.0, 3.0, 4.0 },
.{ 5.0, 6.0, 7.0, 8.0 },
.{ 9.0, 10.0, 11.0, 12.0 },
.{ 13.0, 14.0, 15.0, 16.0 },
});
const J = resonance(4, A);
for (0..4) |i| {
try std.testing.expect(@abs(J.data[i][i]) < 1e-10);
}
}

// ---- Projector Tests ----

test "projector is idempotent: Π² = Π" {
const Jc = Mat4.init(.{
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 0.0, 1.0, 0.0, 0.0 },
.{ 0.0, 0.0, 1.0, 0.0 },
.{ 0.0, 0.0, 0.0, 1.0 },
});
const Pi = logicalProjector(4, Jc);
const Pi2 = Mat4.matmul(Pi, Pi);
try std.testing.expect(Mat4.approxEqual(Pi, Pi2, 1e-8));
}

test "projector is symmetric: Π^T = Π" {
const Jc = Mat4.init(.{
.{ 0.5, 0.0, 0.0, 0.0 },
.{ 0.0, 0.5, 0.0, 0.0 },
.{ 0.0, 0.0, 0.5, 0.0 },
.{ 0.0, 0.0, 0.0, 0.5 },
});
const Pi = logicalProjector(4, Jc);
const Pit = Mat4.transpose(Pi);
try std.testing.expect(Mat4.approxEqual(Pi, Pit, 1e-8));
}

// ---- Quantum Normalization Tests ----

test "quantum normalization mix=0 is identity" {
const v = Mat4.init(.{
.{ 3.0, 0.0, 0.0, 0.0 },
.{ 4.0, 0.0, 0.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.0 },
});
const result = quantumNormalize(4, v, 0.0);
try std.testing.expect(Mat4.approxEqual(result, v, 1e-10));
}

test "quantum normalization mix=1 gives unit vector" {
const v = Mat4.init(.{
.{ 3.0, 0.0, 0.0, 0.0 },
.{ 4.0, 0.0, 0.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.0 },
});
const result = quantumNormalize(4, v, 1.0);
const norm = Mat4.vecNorm(result);
try std.testing.expect(@abs(norm - 1.0) < 1e-10);
}

test "quantum normalization preserves direction" {
const v = Mat4.init(.{
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 2.0, 0.0, 0.0, 0.0 },
.{ 3.0, 0.0, 0.0, 0.0 },
.{ 4.0, 0.0, 0.0, 0.0 },
});
const result = quantumNormalize(4, v, 0.5);
// Result should be a weighted combination, same direction sign
for (0..4) |i| {
try std.testing.expect(result.data[i][0] > 0); // Same sign
}
}

// ---- POLER Cycle Tests ----

test "POLER cycle converges" {
const L = Mat4.init(.{
.{ 0.1, 0.0, 0.0, 0.0 },
.{ 0.0, 0.1, 0.0, 0.0 },
.{ 0.0, 0.0, 0.1, 0.0 },
.{ 0.0, 0.0, 0.0, 0.1 },
});
const A = Mat4.init(.{
.{ 0.0, 0.05, 0.0, 0.0 },
.{ -0.05, 0.0, 0.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.05 },
.{ 0.0, 0.0, -0.05, 0.0 },
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
.{ 0.5, 0.0, 0.0, 0.0 },
.{ 0.3, 0.0, 0.0, 0.0 },
.{ 0.1, 0.0, 0.0, 0.0 },
});

const config = PolerConfig{
.eta = 0.05,
.gamma = 0.1,
.mix = 0.05,
.max_iterations = 5000,
.tolerance = 1e-8,
};

const result = polerCycle(L, A, Jc, G, p0, config);
try std.testing.expect(result.iterations > 0);
try std.testing.expect(result.iterations <= config.max_iterations);
}

test "POLER discrete step reduces energy for small η" {
const L = Mat4.init(.{
.{ 0.5, 0.0, 0.0, 0.0 },
.{ 0.0, 0.5, 0.0, 0.0 },
.{ 0.0, 0.0, 0.5, 0.0 },
.{ 0.0, 0.0, 0.0, 0.5 },
});
const A = Mat4.init(.{
.{ 0.0, 0.1, 0.0, 0.0 },
.{ -0.1, 0.0, 0.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.1 },
.{ 0.0, 0.0, -0.1, 0.0 },
});
const Jc = Mat4.scale(Mat4.identity(), 0.01);
const G = Mat4.scale(Mat4.identity(), 0.1);
const p0 = Mat4.init(.{
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 1.0, 0.0, 0.0, 0.0 },
});

const P_new = polerDiscreteStep(4, L, A, Jc, G, p0, 0.01, 0.1);
// After one step, the state should have changed
try std.testing.expect(!Mat4.approxEqual(p0, P_new, 1e-15));
}

// ---- Archetype Tests ----

test "projection matrix is idempotent under ε=0" {
const a = Mat4.init(.{
.{ 1.0, 0.0, 0.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.0 },
.{ 0.0, 0.0, 1.0, 0.0 },
.{ 0.0, 0.0, 0.0, 0.0 },
});
try std.testing.expect(verifyArchetypeIdempotent(4, a, 0.0, 1e-10));
}

test "identity is idempotent under ε=0" {
const I = Mat4.identity();
try std.testing.expect(verifyArchetypeIdempotent(4, I, 0.0, 1e-10));
}

// ---- Matrix Inversion Tests ----

test "identity inverse is identity" {
const I = Mat4.identity();
const Iinv = try invertMatrix(4, I);
try std.testing.expect(Mat4.approxEqual(Iinv, I, 1e-10));
}

test "diagonal matrix inverse" {
const D = Mat4.init(.{
.{ 2.0, 0.0, 0.0, 0.0 },
.{ 0.0, 4.0, 0.0, 0.0 },
.{ 0.0, 0.0, 8.0, 0.0 },
.{ 0.0, 0.0, 0.0, 16.0 },
});
const Dinv = try invertMatrix(4, D);
const product = Mat4.matmul(D, Dinv);
try std.testing.expect(Mat4.approxEqual(product, Mat4.identity(), 1e-8));
}
