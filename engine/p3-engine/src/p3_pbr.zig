// =============================================================================
// P³ ENGINE — PBR MATERIALS MODULE (Cook-Torrance BRDF + IBL)
// =============================================================================
//
// Native P³ implementation of physically-based rendering materials.
// Architecturally inspired by Unreal Engine's MaterialX pbrlib (Cook-Torrance)
// but uses our own P³ math (Vec3) and Zig-native idioms — no Epic C++ copied.
//
// All algorithms are PUBLIC DOMAIN (textbook computer graphics):
//   - GGX/Trowbridge-Reitz NDF: Walter et al. 2007 (public research paper)
//   - Smith geometry (separable, GGX-matched): Heitz 2014 (public research)
//   - Fresnel Schlick: Schlick 1994 approximation (public, widely used)
//   - Cook-Torrance BRDF: Cook & Torrance 1982 (public domain, decades old)
//   - Disney BRDF notes: Burley 2012 (public Disney research notes)
//   - IBL LUT precompute: Karis 2013 "Real Shading in Unreal Engine 4" (public
//     SIGGRAPH course)
//
// Reference: calculations/pbr_materials/pbr_formulas.txt
// UE MaterialX shaders (public reference): Engine/Binaries/ThirdParty/MaterialX/
//   libraries/pbrlib/genglsl/lib/mx_microfacet_specular.glsl
// =============================================================================

const std = @import("std");
const math = std.math;
const p3 = @import("root.zig");

const Vec3 = p3.Vec3;
const Vec4 = p3.Vec4;
const Mat4x4 = p3.Mat4x4;

// ---------------------------------------------------------------------------
// Color (RGB, f32 in [0,1])
// ---------------------------------------------------------------------------
pub const Color = struct {
    r: f32,
    g: f32,
    b: f32,

    pub inline fn init(r: f32, g: f32, b: f32) Color {
        return .{ .r = r, .g = g, .b = b };
    }

    pub inline fn black() Color {
        return .{ .r = 0, .g = 0, .b = 0 };
    }

    pub inline fn white() Color {
        return .{ .r = 1, .g = 1, .b = 1 };
    }

    pub inline fn fromU8(r: u8, g: u8, b: u8) Color {
        return .{ .r = @as(f32, @floatFromInt(r)) / 255.0, .g = @as(f32, @floatFromInt(g)) / 255.0, .b = @as(f32, @floatFromInt(b)) / 255.0 };
    }

    pub inline fn add(a: Color, b: Color) Color {
        return .{ .r = a.r + b.r, .g = a.g + b.g, .b = a.b + b.b };
    }

    pub inline fn mul(a: Color, b: Color) Color {
        return .{ .r = a.r * b.r, .g = a.g * b.g, .b = a.b * b.b };
    }

    pub inline fn scale(a: Color, s: f32) Color {
        return .{ .r = a.r * s, .g = a.g * s, .b = a.b * s };
    }
};

// ---------------------------------------------------------------------------
// Material definition (matches UE/Unity Standard shader)
// ---------------------------------------------------------------------------
pub const Material = struct {
    /// Base color (albedo for dielectrics, F0 tint for metals)
    base_color: Color = Color.fromU8(180, 180, 180),

    /// Metallic factor (0 = dielectric, 1 = metal)
    /// Used to compute F0: dielectric F0 = 0.04; metal F0 = base_color
    metallic: f32 = 0.0,

    /// Roughness (0 = mirror, 1 = diffuse) — use roughness² as α for perceptual
    /// linear shading (Disney BRDF notes recommendation)
    roughness: f32 = 0.5,

    /// Emissive color (for self-lit materials like neon strips)
    emissive: Color = Color.black(),

    /// Compute F0 (Fresnel at normal incidence)
    /// For dielectric (metallic < 0.5): F0 = 0.04 (typical non-metal value)
    /// For metal (metallic >= 0.5): F0 = base_color tinted
    pub fn computeF0(self: Material) Color {
        if (self.metallic >= 0.5) {
            return self.base_color;
        }
        return Color.init(0.04, 0.04, 0.04);
    }

    /// Compute alpha (perceptual roughness²) for GGX
    pub fn computeAlpha(self: Material) f32 {
        const r = if (self.roughness < 0.001) 0.001 else self.roughness;
        return r * r;
    }
};

// ---------------------------------------------------------------------------
// Pre-computed BRDF LUT for image-based lighting (IBL)
// ---------------------------------------------------------------------------
pub const BrdfLut = struct {
    /// Sampled at indices [NdotV_idx][roughness_idx]
    /// scale: F0 multiplier for specular IBL
    /// bias: F0 additive for specular IBL
    /// F_resolved = F0 * scale + bias
    scale: []f32,
    bias: []f32,
    size: u32,

    /// Load from binary file (format: N×N f32 scale + N×N f32 bias, little-endian)
    /// Format produced by calculations/calc_pbr_materials.py: brdf_lut.bin
    pub fn loadFromFile(allocator: std.mem.Allocator, path: []const u8) !BrdfLut {
        var file = try std.fs.cwd().openFile(path, .{});
        defer file.close();
        const content = try file.readToEndAlloc(allocator, 1024 * 1024);
        defer allocator.free(content);

        // Detect size: total / 2 (scale + bias) / 4 (f32) → sqrt
        const total_floats = content.len / @sizeOf(f32);
        if (total_floats % 2 != 0) return error.InvalidBrdfLutFormat;
        const half = total_floats / 2;
        const size_u32: u32 = @intCast(math.sqrt(@as(f32, @floatFromInt(half))));
        if (size_u32 * size_u32 != half) return error.InvalidBrdfLutFormat;

        const scale = try allocator.alloc(f32, half);
        const bias = try allocator.alloc(f32, half);
        @memcpy(scale, std.mem.bytesAsSlice(f32, content[0..half * 4]));
        @memcpy(bias, std.mem.bytesAsSlice(f32, content[half * 4 ..]));

        return .{
            .scale = scale,
            .bias = bias,
            .size = size_u32,
        };
    }

    pub fn deinit(self: *BrdfLut, allocator: std.mem.Allocator) void {
        allocator.free(self.scale);
        allocator.free(self.bias);
    }

    /// Sample (scale, bias) at (NdotV, roughness) — both in [0, 1]
    pub fn sample(self: *const BrdfLut, NdotV: f32, roughness: f32) struct { scale: f32, bias: f32 } {
        const v = clampU32Index(NdotV, self.size);
        const r = clampU32Index(roughness, self.size);
        const idx = v * self.size + r;
        return .{ .scale = self.scale[idx], .bias = self.bias[idx] };
    }
};

fn clampU32Index(t: f32, size: u32) u32 {
    var v: f32 = t;
    if (v < 0) v = 0;
    if (v > 1) v = 1;
    return @intFromFloat(v * @as(f32, @floatFromInt(size - 1)));
}

// ---------------------------------------------------------------------------
// GGX (Trowbridge-Reitz) Normal Distribution Function
//   D(N, H, α) = α² / (π * ((N·H)² * (α² - 1) + 1)²)
// Reference: Walter et al. 2007, "Microfacet Models for Refraction"
// Also in UE MaterialX mx_microfacet_specular.glsl (Disney BRDF notes B.2 Eq 13)
// ---------------------------------------------------------------------------
pub fn ggxNdf(NdotH: f32, alpha: f32) f32 {
    if (NdotH <= 0) return 0;
    const a2 = alpha * alpha;
    const denom_sq = NdotH * NdotH * (a2 - 1) + 1;
    const denom = math.pi * denom_sq * denom_sq;
    if (denom < 1e-12) return 0;
    return a2 / denom;
}

// ---------------------------------------------------------------------------
// Smith Geometry Shadowing (separable, GGX-matched)
//   G1(N·X, α) = 2 * (N·X) / ((N·X) + sqrt(α² + (1 - α²) * (N·X)²))
//   G_Smith = G1(N·V, α) * G1(N·L, α)
// Reference: Heitz 2014, "Understanding the Masking-Shadowing Function in
// Microfacet-Based BRDFs" (public JCGT paper)
// ---------------------------------------------------------------------------
pub fn smithG1(NdotX: f32, alpha: f32) f32 {
    if (NdotX <= 0) return 0;
    const a2 = alpha * alpha;
    const denom = NdotX + @sqrt(a2 + (1 - a2) * NdotX * NdotX);
    if (denom < 1e-12) return 0;
    return 2 * NdotX / denom;
}

pub fn smithG(NdotV: f32, NdotL: f32, alpha: f32) f32 {
    return smithG1(NdotV, alpha) * smithG1(NdotL, alpha);
}

// ---------------------------------------------------------------------------
// Fresnel Schlick Approximation
//   F(V, H, F0) = F0 + (1 - F0) * (1 - V·H)⁵
// Reference: Schlick 1994, "An Inexpensive BRDF Model for Physically-based
// Rendering" (public domain, decades old)
// ---------------------------------------------------------------------------
pub fn fresnelSchlick(VdotH: f32, F0: Color) Color {
    // Schlick: F = F0 + (1 - F0) * (1 - V·H)^5
    // At V·H = 0 (grazing angle): factor = 1^5 = 1, so F = F0 + (1 - F0) = 1.0 (total reflection)
    // At V·H = 1 (normal incidence): factor = 0, so F = F0
    const v = @max(VdotH, 0.0);
    const factor = math.pow(f32, 1.0 - v, 5);
    return Color.init(
        F0.r + (1.0 - F0.r) * factor,
        F0.g + (1.0 - F0.g) * factor,
        F0.b + (1.0 - F0.b) * factor,
    );
}

// ---------------------------------------------------------------------------
// Cook-Torrance BRDF
//   f_r = (1 - F) * albedo/π + (D * G * F) / (4 * (N·V) * (N·L))
// Reference: Cook & Torrance 1982, "A Reflectance Model for Computer Graphics"
// ---------------------------------------------------------------------------
pub fn cookTorranceBrdf(
    N: Vec3,
    V: Vec3,
    L: Vec3,
    material: Material,
) Color {
    const H = V.add(L).normalize(); // half-vector
    const NdotV = @max(N.dot(V), 0.001);
    const NdotL_raw = N.dot(L);
    if (NdotL_raw <= 0) return Color.black();
    const NdotL = @max(NdotL_raw, 0.001);
    const NdotH = @max(N.dot(H), 0.0);
    const VdotH = @max(V.dot(H), 0.0);

    const alpha = material.computeAlpha();
    const F0 = material.computeF0();

    const D = ggxNdf(NdotH, alpha);
    const G = smithG(NdotV, NdotL, alpha);
    const F = fresnelSchlick(VdotH, F0);

    // Specular: (D * G * F) / (4 * NdotV * NdotL)
    const denom = 4 * NdotV * NdotL;
    var specular = Color.black();
    if (denom > 1e-12) {
        const factor = D * G / denom;
        specular = F.scale(factor);
    }

    // Diffuse: (1 - F) * albedo / π  (energy conservation)
    const kD = Color.init(1.0 - F.r, 1.0 - F.g, 1.0 - F.b);
    const one_over_pi = 1.0 / math.pi;
    const diffuse = kD.mul(material.base_color).scale(one_over_pi * (1.0 - material.metallic));

    return diffuse.add(specular);
}

// ---------------------------------------------------------------------------
// Image-Based Lighting (IBL) — simplified, uses precomputed BRDF LUT
//   indirect_specular = prefiltered_env * (F0 * scale + bias) * (N·V)
//   indirect_diffuse = irradiance_cubemap * albedo * (1 - metallic)
// Reference: Karis 2013 "Real Shading in Unreal Engine 4" (SIGGRAPH course)
// ---------------------------------------------------------------------------
pub fn imageBasedLighting(
    N: Vec3,
    V: Vec3,
    material: Material,
    brdf_lut: BrdfLut,
    irradiance_color: Color, // diffuse IBL (irradiance cubemap sample)
    prefiltered_color: Color, // specular IBL (prefiltered env sample at roughness)
) Color {
    const NdotV = @max(N.dot(V), 0.001);
    const roughness = if (material.roughness < 0.001) 0.001 else material.roughness;

    // Specular IBL: prefiltered_env * (F0 * scale + bias)
    const lut_sample = brdf_lut.sample(NdotV, roughness);
    const F0 = material.computeF0();
    const f_resolved = Color.init(
        F0.r * lut_sample.scale + lut_sample.bias,
        F0.g * lut_sample.scale + lut_sample.bias,
        F0.b * lut_sample.scale + lut_sample.bias,
    );
    const indirect_specular = prefiltered_color.mul(f_resolved);

    // Diffuse IBL: irradiance * albedo * (1 - metallic)
    const kS = F0; // approximation: metallic reflects, dielectric absorbs
    const kD = Color.init(1.0 - kS.r, 1.0 - kS.g, 1.0 - kS.b).scale(1.0 - material.metallic);
    const indirect_diffuse = irradiance_color.mul(material.base_color).mul(kD).scale(1.0 / math.pi);

    return indirect_diffuse.add(indirect_specular);
}

// ---------------------------------------------------------------------------
// Directional light source
// ---------------------------------------------------------------------------
pub const DirectionalLight = struct {
    direction: Vec3, // from surface toward light
    color: Color,
    intensity: f32 = 1.0,
};

// ---------------------------------------------------------------------------
// Final shaded pixel color (direct + indirect + emissive)
// ---------------------------------------------------------------------------
pub fn shadePixel(
    N: Vec3,
    V: Vec3,
    material: Material,
    lights: []const DirectionalLight,
    brdf_lut: BrdfLut,
    irradiance: Color,
    prefiltered_env: Color,
) Color {
    var direct = Color.black();
    for (lights) |light| {
        const L = light.direction.normalize();
        const light_color = light.color.scale(light.intensity);
        const brdf = cookTorranceBrdf(N, V, L, material);
        const NdotL = @max(N.dot(L), 0);
        direct = direct.add(light_color.mul(brdf).scale(NdotL));
    }
    const indirect = imageBasedLighting(N, V, material, brdf_lut, irradiance, prefiltered_env);
    return direct.add(indirect).add(material.emissive);
}

// ===========================================================================
// TESTS
// ===========================================================================

test "PBR: GGX NDF at zero N·H returns zero" {
    const d = ggxNdf(0.0, 0.5);
    try std.testing.expectApproxEqAbs(d, 0.0, 1e-6);
}

test "PBR: GGX NDF peaks at N·H=1" {
    const alpha = 0.25;
    const d_peak = ggxNdf(1.0, alpha);
    const d_off = ggxNdf(0.5, alpha);
    try std.testing.expect(d_peak > d_off);
    // At N·H=1: denom_sq = 1*(a²-1)+1 = a², denom = π*a⁴, so D = a² / (π*a⁴) = 1/(π*a²)
    const expected = 1.0 / (math.pi * alpha * alpha);
    try std.testing.expectApproxEqAbs(d_peak, expected, 0.001);
}

test "PBR: Smith G1 at zero N·V returns zero" {
    const g = smithG1(0.0, 0.5);
    try std.testing.expectApproxEqAbs(g, 0.0, 1e-6);
}

test "PBR: Smith G at zero roughness (perfect mirror) returns 1" {
    // At α → 0, G1(X, 0) = 2X / (X + sqrt(X²)) = 2X / (2X) = 1
    const g = smithG1(0.8, 0.001);
    try std.testing.expectApproxEqAbs(g, 1.0, 0.01);
}

test "PBR: Fresnel Schlick at V·H=1 returns F0" {
    const F0 = Color.init(0.04, 0.04, 0.04);
    const F = fresnelSchlick(1.0, F0);
    try std.testing.expectApproxEqAbs(F.r, F0.r, 1e-6);
    try std.testing.expectApproxEqAbs(F.g, F0.g, 1e-6);
    try std.testing.expectApproxEqAbs(F.b, F0.b, 1e-6);
}

test "PBR: Fresnel Schlick at V·H=0 (grazing) returns 1.0" {
    // At V·H=0, F = F0 + (1 - F0) * 1 = 1.0 (total reflection)
    const F0 = Color.init(0.04, 0.04, 0.04);
    const F = fresnelSchlick(0.0, F0);
    try std.testing.expectApproxEqAbs(F.r, 1.0, 1e-6);
    try std.testing.expectApproxEqAbs(F.g, 1.0, 1e-6);
}

test "PBR: Material F0 computation (dielectric vs metal)" {
    // Dielectric (metallic=0): F0 = 0.04
    const dielectric = Material{ .base_color = Color.fromU8(180, 180, 180), .metallic = 0.0 };
    const f0_d = dielectric.computeF0();
    try std.testing.expectApproxEqAbs(f0_d.r, 0.04, 1e-3);

    // Metal (metallic=1): F0 = base_color
    const gold = Material{
        .base_color = Color.init(1.0, 0.71, 0.29),
        .metallic = 1.0,
    };
    const f0_m = gold.computeF0();
    try std.testing.expectApproxEqAbs(f0_m.r, 1.0, 1e-3);
    try std.testing.expectApproxEqAbs(f0_m.g, 0.71, 1e-3);
    try std.testing.expectApproxEqAbs(f0_m.b, 0.29, 1e-3);
}

test "PBR: Material alpha = roughness² (perceptual linear)" {
    const mat = Material{ .roughness = 0.5 };
    const alpha = mat.computeAlpha();
    try std.testing.expectApproxEqAbs(alpha, 0.25, 1e-6);
}

test "PBR: Color arithmetic" {
    const a = Color.init(0.5, 0.5, 0.5);
    const b = Color.init(0.25, 0.25, 0.25);
    const sum = a.add(b);
    try std.testing.expectApproxEqAbs(sum.r, 0.75, 1e-6);
    const prod = a.mul(b);
    try std.testing.expectApproxEqAbs(prod.r, 0.125, 1e-6);
    const scaled = a.scale(2.0);
    try std.testing.expectApproxEqAbs(scaled.r, 1.0, 1e-6);
}

test "PBR: Color from u8" {
    const c = Color.fromU8(255, 0, 128);
    try std.testing.expectApproxEqAbs(c.r, 1.0, 1e-6);
    try std.testing.expectApproxEqAbs(c.g, 0.0, 1e-6);
    try std.testing.expectApproxEqAbs(c.b, 128.0 / 255.0, 1e-6);
}

test "PBR: Cook-Torrance BRDF returns non-zero for lit surface" {
    const N = Vec3.init(0, 1, 0);
    const V = Vec3.init(0, 1, 0); // looking straight down at normal
    const L = Vec3.init(0.5, 0.7, 0.5).normalize();
    const mat = Material{ .base_color = Color.fromU8(180, 180, 180), .roughness = 0.3 };
    const brdf = cookTorranceBrdf(N, V, L, mat);
    try std.testing.expect(brdf.r > 0);
    try std.testing.expect(brdf.g > 0);
    try std.testing.expect(brdf.b > 0);
}

test "PBR: Cook-Torrance BRDF returns black when surface faces away from light" {
    const N = Vec3.init(0, 1, 0);
    const V = Vec3.init(0, 1, 0);
    const L = Vec3.init(0, -1, 0); // light from below
    const mat = Material{};
    const brdf = cookTorranceBrdf(N, V, L, mat);
    try std.testing.expectApproxEqAbs(brdf.r, 0.0, 1e-6);
}

test "PBR: BrdfLut sample boundary indices" {
    // Create a fake LUT in memory (4x4)
    const allocator = std.testing.allocator;
    var lut = BrdfLut{
        .scale = try allocator.alloc(f32, 16),
        .bias = try allocator.alloc(f32, 16),
        .size = 4,
    };
    defer {
        allocator.free(lut.scale);
        allocator.free(lut.bias);
    }
    @memset(lut.scale, 0.5);
    @memset(lut.bias, 0.1);
    const s1 = lut.sample(0.0, 0.0); // indices [0, 0]
    try std.testing.expectApproxEqAbs(s1.scale, 0.5, 1e-6);
    try std.testing.expectApproxEqAbs(s1.bias, 0.1, 1e-6);
    const s2 = lut.sample(1.0, 1.0); // indices [3, 3]
    try std.testing.expectApproxEqAbs(s2.scale, 0.5, 1e-6);
}

test "PBR: DirectionalLight struct" {
    const light = DirectionalLight{
        .direction = Vec3.init(0, 1, 0),
        .color = Color.white(),
        .intensity = 2.0,
    };
    try std.testing.expectEqual(@as(f32, 2.0), light.intensity);
}
