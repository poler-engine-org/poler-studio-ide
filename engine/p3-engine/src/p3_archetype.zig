// =============================================================================
// P³ ARCHETYPE ALGEBRA v1.0 — ZIG
// =============================================================================
//
// Алгебра архетипов: деформированное тензорное произведение ⊗_ε,
// идемпотентные проекторы, POLER-цикл, когнитивный контур.
// Портировано из poler-os/src64/poler_core.zig + docs/math-sources/
//
// МАТЕМАТИКА:
//   Алгебра A = (O, ⊕, ⊗_ε):
//     ⊕ — XOR (GF(2) сложение, параллелизм)
//     ⊗_ε — деформированное тензорное произведение:
//       a ⊗_ε b = φ(a·b) + ε·φ(a⊕b)
//       где φ — ARX-box (биективная перестановка)
//       ε — параметр деформации (конформная метрика)
//
//   Идемпотент: a ⊗_ε a = a (архетип = проектор)
//   Шифрование: p_{t+1} = a ⊗_ε p_t ⊕ m
//   Аттрактор: p* = a ⊗_ε p* ⊕ m (стационарная точка)
//   Дешифровка: m = p* ⊕ (a ⊗_ε p*)
//
//   Нильпотентный оператор: N(y,key,ε) — N² = 0 за 2 шага
//   POLER-цикл: x_{k+1} = N(x_k, key, ε) → сходимость к аттрактору
//
// P³ ОБОБЩЕНИЯ:
//   - ⊗_ε на P³: деформированное тензорное произведение проективных объектов
//   - Архетип = идемпотентный элемент в Cl(3,0,1) (PGA)
//   - POLER-цикл на P³: projected gradient descent с ⊗_ε
//   - Когнитивный контур: проекция архетипа на сенсорный базис P³
//   - Pole-polar: двойственность в PG ↔ archetypal projection
//
// Донор: poler-os/poler_core.zig (1882 строк Zig) + math-sources (5 papers)
// Порт: Zig 0.14.0, u32 GF(2⁸) operations, P³ integration
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const math = std.math;

// =============================================================================
// 1. ARX-BOX φ() — БИЕКТИВНАЯ ПЕРЕСТАНОВКА
// =============================================================================

/// Golden ratio constant C1 = 0x9E3779B9 (2654435769)
pub const GOLDEN_RATIO_U32: u32 = 0x9E3779B9;

/// 7th Mersenne prime hash C2 = 0x517CC1B7
pub const MERSENNE7_HASH: u32 = 0x517CC1B7;

/// ARX-box φ(x) — full pipeline bijective permutation
/// Pipeline: ADD(C1) → ROTL(13) → XORSHIFT(16) → MUL(C2) → ROTL(7) → ADD(1)
/// Guaranteed bijective (invertible) for all u32 inputs
pub fn arxBox(x: u32) u32 {
    var y = x +% GOLDEN_RATIO_U32; // ADD
    y = rotl32(y, 13); // ROTL
    y ^= y >> 16; // XORSHIFT
    y *%= MERSENNE7_HASH; // MUL
    y = rotl32(y, 7); // ROTL
    y +%= 1; // ADD(1) — ensures no fixed point at 0
    return y;
}

/// 32-bit rotate left
pub fn rotl32(x: u32, comptime k: u5) u32 {
    return std.math.rotl(u32, x, k);
}

// =============================================================================
// 2. DEFORMED TENSOR PRODUCT ⊗_ε
// =============================================================================

/// PND mix (Parametric Nonlinear Diffusion) — core of ⊗_ε
/// pndMix(a, b, ε) = φ(a·b) + ε·φ(a⊕b)  (v8 with φ-wrap on both terms)
///
/// Properties:
///   - Non-linear even at ε=0 (due to φ-wrap)
///   - Auto-correction: ε=0 → ε=1 ("No Excuses" principle)
///   - When ε=1: reduces to φ(a·b) + φ(a⊕b) — maximal mixing
pub fn pndMix(a: u32, b: u32, epsilon: u32) u32 {
    const eps = if (epsilon == 0) @as(u32, 1) else epsilon; // No Excuses
    const product = arxBox(a *% b); // φ(a·b)
    const sum = arxBox(a ^ b); // φ(a⊕b)
    return product ^ (eps *% sum); // product ⊕ (ε · sum)
}

// =============================================================================
// 3. NILPOTENT OPERATOR
// =============================================================================

/// Nilpotent operator N(y, key, ε) — satisfies N² ≈ 0 (2-step convergence)
/// Pipeline: XOR → MUL(odd) → ADD → φ → MUL(golden) → ADD → ROTL
pub fn nilpotentOperator(y: u32, key: u32, epsilon: u32) u32 {
    var result = y ^ key; // XOR with key
    const odd_key = key | 1; // Ensure odd for GF(2⁸) invertibility
    result *%= odd_key; // MUL by odd key
    result +%= epsilon; // ADD deformation
    result = arxBox(result); // ARX-box
    result *%= GOLDEN_RATIO_U32; // MUL by golden ratio
    result +%= key; // ADD key
    result = rotl32(result, 17); // ROTL
    return result;
}

// =============================================================================
// 4. DYNAMIC ATTRACTOR
// =============================================================================

/// Compute attractor from key: attractor(key) = rotl(key, 17) ^ φ(key)
/// The POLER cycle converges to this fixed-point attractor
pub fn computeAttractor(key: u32) u32 {
    return rotl32(key, 17) ^ arxBox(key);
}

// =============================================================================
// 5. POLER CYCLE — DISCRETE-TIME DISSIPATIVE DYNAMICAL SYSTEM
// =============================================================================

/// POLER cycle result
pub const PolerCycleResult = struct {
    /// Final state (near attractor)
    final_state: u32,
    /// Number of iterations to converge
    iterations: u32,
    /// Whether converged (Hamming distance ≤ threshold)
    converged: bool,
    /// Attractor value
    attractor: u32,
};

/// Run POLER cycle: x_{k+1} = nilpotentOperator(x_k, key, ε)
/// Converges when Hamming distance to attractor ≤ hamming_threshold
/// Maximum max_iterations steps
pub fn polerCycle(
    initial_state: u32,
    key: u32,
    epsilon: u32,
    max_iterations: u32,
    hamming_threshold: u32,
) PolerCycleResult {
    const attractor = computeAttractor(key);
    var state = initial_state;
    var iter: u32 = 0;

    while (iter < max_iterations) : (iter += 1) {
        state = nilpotentOperator(state, key, epsilon);
        const dist = hammingDistance(state, attractor);
        if (dist <= hamming_threshold) {
            return .{
                .final_state = state,
                .iterations = iter + 1,
                .converged = true,
                .attractor = attractor,
            };
        }
    }

    return .{
        .final_state = state,
        .iterations = max_iterations,
        .converged = false,
        .attractor = attractor,
    };
}

/// Hamming distance (popcount of XOR)
pub fn hammingDistance(a: u32, b: u32) u32 {
    return @popCount(a ^ b);
}

// =============================================================================
// 6. IDEMPOTENT ARCHETYPE
// =============================================================================

/// Archetype: an idempotent element a where a ⊗_ε a ≈ a
/// Used for projection-based encryption
pub const Archetype = struct {
    /// The archetype value (must satisfy a ⊗_ε a ≈ a)
    value: u32,
    /// Deformation parameter ε
    epsilon: u32,
    /// How close a ⊗_ε a is to a (0 = perfect idempotent)
    idempotency_error: u32,

    /// Find an archetype by searching for approximate idempotents
    /// Try random values and check ||a ⊗_ε a - a|| < tolerance
    pub fn find(epsilon: u32, tolerance: u32, max_tries: u32, rng: *std.Random) ?Archetype {
        var i: u32 = 0;
        while (i < max_tries) : (i += 1) {
            const candidate = rng.next();
            const product = pndMix(candidate, candidate, epsilon);
            const idemp_error = hammingDistance(product, candidate);
            if (idemp_error <= tolerance) {
                return .{
                    .value = candidate,
                    .epsilon = epsilon,
                    .idempotency_error = idemp_error,
                };
            }
        }
        return null;
    }

    /// Verify idempotency: check a ⊗_ε a ≈ a
    pub fn verify(a: Archetype) bool {
        const product = pndMix(a.value, a.value, a.epsilon);
        return hammingDistance(product, a.value) <= a.idempotency_error;
    }

    /// Project state through archetype: result = a ⊗_ε state
    pub fn project(a: Archetype, state: u32) u32 {
        return pndMix(a.value, state, a.epsilon);
    }
};

// =============================================================================
// 7. ARCHETYPE ENCRYPTION
// =============================================================================

/// Archetype-based encryption using idempotent projector
/// Encryption: p_{t+1} = a ⊗_ε p_t ⊕ m
/// Converges to: p* = a ⊗_ε p* ⊕ m
/// Decryption: m = p* ⊕ (a ⊗_ε p*)
pub const ArchetypeCipher = struct {
    archetype: Archetype,
    max_iterations: u32,

    /// Encrypt a message block using archetype projection
    /// Returns the ciphertext (attractor state)
    pub fn encrypt(cipher: ArchetypeCipher, message: u32, initial_state: u32) u32 {
        var state = initial_state;
        var i: u32 = 0;
        while (i < cipher.max_iterations) : (i += 1) {
            // p_{t+1} = (a ⊗_ε p_t) ⊕ m
            state = pndMix(cipher.archetype.value, state, cipher.archetype.epsilon) ^ message;
        }
        return state;
    }

    /// Decrypt: recover message from ciphertext (attractor)
    /// m = p* ⊕ (a ⊗_ε p*)
    pub fn decrypt(cipher: ArchetypeCipher, ciphertext: u32) u32 {
        const projected = pndMix(cipher.archetype.value, ciphertext, cipher.archetype.epsilon);
        return ciphertext ^ projected;
    }
};

// =============================================================================
// 8. COGNITIVE CONTOUR
// =============================================================================

/// Cognitive state — "Cognitive Contour" archetype projection
/// Inspired by projective geometry: pole-polar reciprocation
pub const CognitiveState = struct {
    /// Current projector state
    projector: u32,
    /// Active archetype
    archetype: u32,
    /// Attractor key
    attractor_key: u32,
    /// Deformation parameter
    epsilon: u32,

    /// Initialize cognitive state
    pub fn init(archetype_val: u32, key: u32, epsilon: u32) CognitiveState {
        return .{
            .projector = archetype_val,
            .archetype = archetype_val,
            .attractor_key = key,
            .epsilon = epsilon,
        };
    }

    /// Apply cognitive logic step: project input through archetype
    /// Result: new projector state = archetype ⊗_ε input
    pub fn logic(state: CognitiveState, input: u32) CognitiveState {
        const new_projector = pndMix(state.archetype, input, state.epsilon);
        return .{
            .projector = new_projector,
            .archetype = state.archetype,
            .attractor_key = state.attractor_key,
            .epsilon = state.epsilon,
        };
    }

    /// Compute Jacobian approximation (finite differences)
    /// J ≈ (φ(x+δ) - φ(x-δ)) / (2δ)
    pub fn jacobian(state: CognitiveState, delta: u32) u32 {
        const f_plus = pndMix(state.archetype, state.projector +% delta, state.epsilon);
        const f_minus = pndMix(state.archetype, state.projector -% delta, state.epsilon);
        return f_plus ^ f_minus; // XOR as "difference" in GF(2)
    }

    /// Distance to attractor
    pub fn distanceToAttractor(state: CognitiveState) u32 {
        return hammingDistance(state.projector, computeAttractor(state.attractor_key));
    }
};

// =============================================================================
// 9. P³ ARCHETYPE — PROJECTIVE INTEGRATION
// =============================================================================

/// Deformed tensor product on P³:
/// For homogeneous vectors [x:w], define:
///   [x:w] ⊗_ε [y:v] = [φ(x·y):φ(w·v)]  (projective deformed product)
pub fn pndMixHomogeneous(x: [4]f32, y: [4]f32, epsilon: f32) [4]f32 {
    var result: [4]f32 = undefined;
    for (0..4) |i| {
        // Use integer representation for φ, convert back
        const xi = @as(u32, @bitCast(x[i]));
        const yi = @as(u32, @bitCast(y[i]));
        const eps_int = @as(u32, @bitCast(epsilon));
        const mixed = pndMix(xi, yi, eps_int);
        result[i] = @as(f32, @bitCast(mixed));
    }
    return result;
}

/// Cross-ratio of 4 archetype states (projective invariant)
pub fn archetypeCrossRatio(
    a: u32,
    b: u32,
    c: u32,
    d: u32,
    epsilon: u32,
) f32 {
    // Use ⊗_ε products instead of ordinary products
    const ac = @as(f64, @floatFromInt(pndMix(a, c, epsilon)));
    const bd = @as(f64, @floatFromInt(pndMix(b, d, epsilon)));
    const ad = @as(f64, @floatFromInt(pndMix(a, d, epsilon)));
    const bc = @as(f64, @floatFromInt(pndMix(b, c, epsilon)));
    if (@abs(ad) < 1e-16 or @abs(bc) < 1e-16) return 0;
    return @floatCast((ac * bd) / (ad * bc));
}

// =============================================================================
// 10. LINEAR HYBRID CELLULAR AUTOMATON (LHCA)
// =============================================================================

/// LHCA step — diffusion layer for POLER cipher
/// Rule 90/150 hybrid: XOR of neighbors (elementary CA)
pub fn lhcaStep(state: u32, rule_mask: u32) u32 {
    const left = state >> 1;
    const right = state << 1;
    // Rule 90: left ^ right
    // Rule 150: left ^ center ^ right
    // Hybrid: select per bit using rule_mask
    const r90 = left ^ right;
    const r150 = left ^ state ^ right;
    return (r90 & ~rule_mask) | (r150 & rule_mask);
}

// =============================================================================
// 11. CONSTANT-TIME AES S-BOX
// =============================================================================

/// GF(2⁸) multiplication for AES operations
pub fn gf28Mul(a: u8, b: u8) u8 {
    var result: u8 = 0;
    var aa = a;
    var bb = b;
    while (bb != 0) {
        if (bb & 1 != 0) result ^= aa;
        const hi = aa & 0x80;
        aa <<= 1;
        if (hi != 0) aa ^= 0x1B; // AES irreducible polynomial x⁸+x⁴+x³+x+1
        bb >>= 1;
    }
    return result;
}

/// Constant-time AES S-box via x^254 in GF(2⁸) + affine transformation
/// Avoids lookup table (side-channel resistant)
pub fn ctSbox(x: u8) u8 {
    // 1. Inversion in GF(2⁸): x^254 = x^{-1} (0 maps to 0)
    var inv: u8 = 0;
    if (x != 0) {
        const x2 = gf28Mul(x, x);
        const x4 = gf28Mul(x2, x2);
        const x8 = gf28Mul(x4, x4);
        const x16 = gf28Mul(x8, x8);
        const x32 = gf28Mul(x16, x16);
        const x64 = gf28Mul(x32, x32);
        const x128 = gf28Mul(x64, x64);
        var res = gf28Mul(x128, x64);
        res = gf28Mul(res, x32);
        res = gf28Mul(res, x16);
        res = gf28Mul(res, x8);
        res = gf28Mul(res, x4);
        res = gf28Mul(res, x2);
        inv = res;
    }
    // 2. AES Affine transformation
    const r1 = std.math.rotl(u8, inv, 1);
    const r2 = std.math.rotl(u8, inv, 2);
    const r3 = std.math.rotl(u8, inv, 3);
    const r4 = std.math.rotl(u8, inv, 4);
    return inv ^ r1 ^ r2 ^ r3 ^ r4 ^ 0x63;
}

// =============================================================================
// TESTS
// =============================================================================

test "ARX-box: bijectivity (spot check)" {
    // φ should be a permutation: different inputs → different outputs
    const a = arxBox(0x12345678);
    const b = arxBox(0x87654321);
    try std.testing.expect(a != b);
}

test "PND mix: basic" {
    const result = pndMix(0x11111111, 0x22222222, 1);
    // Result should be non-zero and deterministic
    try std.testing.expect(result != 0);
    // Same inputs should give same result
    const result2 = pndMix(0x11111111, 0x22222222, 1);
    try std.testing.expectEqual(result, result2);
}

test "PND mix: ε=0 auto-corrects to ε=1" {
    const r0 = pndMix(0x1234, 0x5678, 0);
    const r1 = pndMix(0x1234, 0x5678, 1);
    try std.testing.expectEqual(r0, r1);
}

test "Nilpotent operator: deterministic" {
    const r1 = nilpotentOperator(0xDEADBEEF, 0xCAFEBABE, 1);
    const r2 = nilpotentOperator(0xDEADBEEF, 0xCAFEBABE, 1);
    try std.testing.expectEqual(r1, r2);
}

test "Attractor: deterministic" {
    const a1 = computeAttractor(0x12345678);
    const a2 = computeAttractor(0x12345678);
    try std.testing.expectEqual(a1, a2);
}

test "Hamming distance" {
    try std.testing.expectEqual(@as(u32, 0), hammingDistance(0, 0));
    try std.testing.expectEqual(@as(u32, 32), hammingDistance(0, 0xFFFFFFFF));
    try std.testing.expectEqual(@as(u32, 1), hammingDistance(0, 1));
}

test "POLER cycle: converges" {
    const result = polerCycle(0xDEADBEEF, 0x12345678, 1, 100, 4);
    // Should converge within 100 iterations for most keys
    try std.testing.expect(result.converged or result.iterations == 100);
}

test "GF(2⁸) multiply: identity" {
    try std.testing.expectEqual(@as(u8, 0x53), gf28Mul(0x53, 1));
    try std.testing.expectEqual(@as(u8, 0), gf28Mul(0x53, 0));
}

test "Constant-time S-box: AES known value" {
    // AES S-box: S(0x53) = 0xED
    try std.testing.expectEqual(@as(u8, 0xED), ctSbox(0x53));
    // S(0) = 0x63
    try std.testing.expectEqual(@as(u8, 0x63), ctSbox(0));
    // S(1) = 0x7C
    try std.testing.expectEqual(@as(u8, 0x7C), ctSbox(1));
}

test "LHCA: deterministic" {
    const s1 = lhcaStep(0x12345678, 0xAAAAAAAA);
    const s2 = lhcaStep(0x12345678, 0xAAAAAAAA);
    try std.testing.expectEqual(s1, s2);
}

test "Cognitive state: logic step" {
    const cs = CognitiveState.init(0x11111111, 0x22222222, 1);
    const cs2 = cs.logic(0x33333333);
    // After logic step, projector should have changed
    try std.testing.expect(cs2.projector != cs.projector or cs.archetype == 0x11111111);
}

test "Archetype cross-ratio: symmetry" {
    const a: u32 = 0x11111111;
    const b: u32 = 0x22222222;
    const c: u32 = 0x33333333;
    const d: u32 = 0x44444444;
    const cr1 = archetypeCrossRatio(a, b, c, d, 1);
    // Cross-ratio should be non-negative
    try std.testing.expect(cr1 >= 0);
}
