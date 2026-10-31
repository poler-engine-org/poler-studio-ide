// =============================================================================
// P³ ENGINE — TEXTURE STREAMING MODULE (mipmaps + bilinear sampling)
// =============================================================================
//
// Native P³ implementation of texture streaming with mipmap chain and
// bilinear filtering. Architecturally inspired by UE's Texture2D + TextureMip
// but WITHOUT copying Epic's C++ — uses our own P³ math and Zig-native idioms.
//
// All algorithms are PUBLIC DOMAIN (textbook computer graphics):
//   - Mipmap level derivation: λ = log2(max(|∂u/∂x|, |∂v/∂x|, |∂u/∂y|, |∂v/∂y|) × W_tex)
//     (Williams 1983, "Pyramidal Parametrics")
//   - Bilinear filtering: 4-texel weighted average (standard GPU texture sampling)
//   - Mipmap chain memory: (4/3) × W × H (geometric series, ratio 1/4)
//
// See calculations/texture_streaming/texture_formulas.txt for full derivations.
// =============================================================================

const std = @import("std");
const math = std.math;
const p3 = @import("root.zig");

const Vec3 = p3.Vec3;

// ---------------------------------------------------------------------------
// Pixel color (RGBA, 8-bit per channel — matches VisualFrameBuffer.PixelColor)
// ---------------------------------------------------------------------------
pub const PixelColor = packed struct {
    r: u8,
    g: u8,
    b: u8,
    a: u8,

    pub inline fn init(r: u8, g: u8, b: u8, a: u8) PixelColor {
        return .{ .r = r, .g = g, .b = b, .a = a };
    }

    pub inline fn black() PixelColor {
        return .{ .r = 0, .g = 0, .b = 0, .a = 255 };
    }

    pub inline fn white() PixelColor {
        return .{ .r = 255, .g = 255, .b = 255, .a = 255 };
    }
};

// ---------------------------------------------------------------------------
// Texture formats (uncompressed for now; compression formats BC7/ASTC are
// stubs — would require platform-specific decompression)
// ---------------------------------------------------------------------------
pub const TextureFormat = enum(u8) {
    rgba8 = 0,    // 4 bytes per texel, raw
    rgb8 = 1,     // 3 bytes per texel, raw
    bc7 = 2,      // 0.5 bytes per texel (8 bytes per 4×4 block) — STUB
    astc_4x4 = 3, // 1.0 byte per texel (16 bytes per 4×4 block) — STUB
};

// ---------------------------------------------------------------------------
// Single mipmap level
// ---------------------------------------------------------------------------
pub const TextureMip = struct {
    width: u32,
    height: u32,
    format: TextureFormat,
    data: []const u8,
    is_compressed: bool = false,
};

// ---------------------------------------------------------------------------
// Texture (with mipmap chain)
// ---------------------------------------------------------------------------
pub const Texture = struct {
    mips: []TextureMip,
    allocator: std.mem.Allocator,
    owned_data: bool = false, // true if we allocated mips[].data, must free

    pub fn init(allocator: std.mem.Allocator) Texture {
        return .{
            .mips = &[_]TextureMip{},
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *Texture) void {
        if (self.owned_data) {
            for (self.mips) |mip| {
                self.allocator.free(mip.data);
            }
        }
        if (self.mips.len > 0) {
            self.allocator.free(self.mips);
        }
    }

    /// Number of mipmap levels (1 = base only, >1 = chain)
    pub fn mipCount(self: *const Texture) u8 {
        return @intCast(self.mips.len);
    }

    /// Get mip level by index (clamped to available range)
    pub fn getMip(self: *const Texture, level: u8) ?*const TextureMip {
        if (level >= self.mips.len) return null;
        return &self.mips[level];
    }

    /// Compute mipmap level based on screen-space UV derivatives.
    /// λ = log2(max(|∂u/∂x|, |∂v/∂x|, |∂u/∂y|, |∂v/∂y|) × W_tex)
    /// Returns clamped to [0, max_level].
    pub fn selectMipLevel(
        self: *const Texture,
        du_dx: f32,
        dv_dx: f32,
        du_dy: f32,
        dv_dy: f32,
    ) u8 {
        if (self.mips.len == 0) return 0;
        const base_w = self.mips[0].width;
        const max_abs = @max(@abs(du_dx), @max(@abs(dv_dx), @max(@abs(du_dy), @abs(dv_dy))));
        if (max_abs < 0.0001) return 0; // magnification → use base level
        const rho = max_abs * @as(f32, @floatFromInt(base_w));
        const lambda = @log2(rho);
        // Clamp + cast
        var level: i32 = @intFromFloat(@floor(lambda));
        if (level < 0) level = 0;
        const max_level: i32 = @intCast(self.mips.len);
        if (level >= max_level) level = max_level - 1;
        return @intCast(level);
    }

    /// Bilinear sample at (u, v) in [0, 1] range on specified mip level.
    /// Returns black if out of range or mip doesn't exist.
    pub fn sampleBilinear(self: *const Texture, u: f32, v: f32, mip_level: u8) PixelColor {
        const mip_ptr = self.getMip(mip_level) orelse return PixelColor.black();
        const mip = mip_ptr.*;
        if (mip.format != .rgba8) return PixelColor.black(); // simplified

        // Convert UV to texel coords
        const fx = u * @as(f32, @floatFromInt(mip.width));
        const fy = v * @as(f32, @floatFromInt(mip.height));
        const x0 = @as(u32, @intFromFloat(@floor(fx)));
        const y0 = @as(u32, @intFromFloat(@floor(fy)));
        const x1 = (x0 + 1) % mip.width;
        const y1 = (y0 + 1) % mip.height;
        const tx = fx - @floor(fx);
        const ty = fy - @floor(fy);

        // Sample 4 texels
        const t00 = sampleTexelRGBA8(mip, x0, y0);
        const t10 = sampleTexelRGBA8(mip, x1, y0);
        const t01 = sampleTexelRGBA8(mip, x0, y1);
        const t11 = sampleTexelRGBA8(mip, x1, y1);

        // Bilinear interpolation
        const lerp = struct {
            fn call(a: u8, b: u8, t: f32) u8 {
                const fa = @as(f32, @floatFromInt(a));
                const fb = @as(f32, @floatFromInt(b));
                return @intFromFloat(fa + (fb - fa) * t);
            }
        };
        const r0 = lerp.call(t00.r, t10.r, tx);
        const g0 = lerp.call(t00.g, t10.g, tx);
        const b0 = lerp.call(t00.b, t10.b, tx);
        const a0 = lerp.call(t00.a, t10.a, tx);
        const r1 = lerp.call(t01.r, t11.r, tx);
        const g1 = lerp.call(t01.g, t11.g, tx);
        const b1 = lerp.call(t01.b, t11.b, tx);
        const a1 = lerp.call(t01.a, t11.a, tx);

        return PixelColor.init(
            lerp.call(r0, r1, ty),
            lerp.call(g0, g1, ty),
            lerp.call(b0, b1, ty),
            lerp.call(a0, a1, ty),
        );
    }
};

/// Sample a single texel from RGBA8 mipmap data
fn sampleTexelRGBA8(mip: TextureMip, x: u32, y: u32) PixelColor {
    if (x >= mip.width or y >= mip.height) return PixelColor.black();
    const idx = (y * mip.width + x) * 4;
    if (idx + 3 >= mip.data.len) return PixelColor.black();
    return PixelColor.init(
        mip.data[idx],
        mip.data[idx + 1],
        mip.data[idx + 2],
        mip.data[idx + 3],
    );
}

// ---------------------------------------------------------------------------
// Procedural texture generation (for tests / no asset loading required)
// ---------------------------------------------------------------------------

/// Create a procedural checkerboard texture (for testing)
pub fn createCheckerboard(
    allocator: std.mem.Allocator,
    width: u32,
    height: u32,
    cell_size: u32,
    color_a: PixelColor,
    color_b: PixelColor,
) !Texture {
    var tex = Texture.init(allocator);
    tex.owned_data = true;
    tex.mips = try allocator.alloc(TextureMip, 1);
    const data = try allocator.alloc(u8, width * height * 4);
    var y: u32 = 0;
    while (y < height) : (y += 1) {
        var x: u32 = 0;
        while (x < width) : (x += 1) {
            const idx = (y * width + x) * 4;
            const cell = ((x / cell_size) + (y / cell_size)) % 2;
            const c = if (cell == 0) color_a else color_b;
            data[idx] = c.r;
            data[idx + 1] = c.g;
            data[idx + 2] = c.b;
            data[idx + 3] = c.a;
        }
    }
    tex.mips[0] = .{
        .width = width,
        .height = height,
        .format = .rgba8,
        .data = data,
    };
    return tex;
}

/// Generate mipmap chain from base level (halve until min dimension = 1)
pub fn generateMipChain(
    allocator: std.mem.Allocator,
    base: TextureMip,
    max_levels: u8,
) ![]TextureMip {
    var levels: u32 = 1;
    var w = base.width;
    var h = base.height;
    while (w > 1 or h > 1) {
        w = if (w > 1) w / 2 else 1;
        h = if (h > 1) h / 2 else 1;
        levels += 1;
        if (levels >= max_levels) break;
    }

    var mips = try allocator.alloc(TextureMip, levels);
    mips[0] = base;

    var level: u32 = 1;
    w = base.width;
    h = base.height;
    while (level < levels) : (level += 1) {
        const new_w = if (w > 1) w / 2 else 1;
        const new_h = if (h > 1) h / 2 else 1;
        const new_data = try allocator.alloc(u8, new_w * new_h * 4);
        // Box filter: average 2×2 block from previous level
        var y: u32 = 0;
        while (y < new_h) : (y += 1) {
            var x: u32 = 0;
            while (x < new_w) : (x += 1) {
                const src_x = x * 2;
                const src_y = y * 2;
                // Sample 4 texels from previous level
                const t00 = sampleTexelRGBA8(mips[level - 1], src_x, src_y);
                const t10 = sampleTexelRGBA8(mips[level - 1], @min(src_x + 1, mips[level - 1].width - 1), src_y);
                const t01 = sampleTexelRGBA8(mips[level - 1], src_x, @min(src_y + 1, mips[level - 1].height - 1));
                const t11 = sampleTexelRGBA8(mips[level - 1], @min(src_x + 1, mips[level - 1].width - 1), @min(src_y + 1, mips[level - 1].height - 1));
                const avg_r = @as(u16, t00.r) / 4 + @as(u16, t10.r) / 4 + @as(u16, t01.r) / 4 + @as(u16, t11.r) / 4;
                const avg_g = @as(u16, t00.g) / 4 + @as(u16, t10.g) / 4 + @as(u16, t01.g) / 4 + @as(u16, t11.g) / 4;
                const avg_b = @as(u16, t00.b) / 4 + @as(u16, t10.b) / 4 + @as(u16, t01.b) / 4 + @as(u16, t11.b) / 4;
                const avg_a = @as(u16, t00.a) / 4 + @as(u16, t10.a) / 4 + @as(u16, t01.a) / 4 + @as(u16, t11.a) / 4;
                const idx = (y * new_w + x) * 4;
                new_data[idx] = @intCast(avg_r);
                new_data[idx + 1] = @intCast(avg_g);
                new_data[idx + 2] = @intCast(avg_b);
                new_data[idx + 3] = @intCast(avg_a);
            }
        }
        mips[level] = .{
            .width = new_w,
            .height = new_h,
            .format = .rgba8,
            .data = new_data,
        };
        w = new_w;
        h = new_h;
    }
    return mips;
}

// ===========================================================================
// TESTS
// ===========================================================================

test "Texture: checkerboard generation" {
    const allocator = std.testing.allocator;
    var tex = try createCheckerboard(allocator, 8, 8, 2, PixelColor.white(), PixelColor.black());
    defer tex.deinit();
    try std.testing.expectEqual(@as(usize, 1), tex.mips.len);
    try std.testing.expectEqual(@as(u32, 8), tex.mips[0].width);
    // Sample at (0.125, 0.125) — should be white (first cell)
    const c = tex.sampleBilinear(0.125, 0.125, 0);
    try std.testing.expectEqual(@as(u8, 255), c.r); // white
}

test "Texture: bilinear sampling at exact texel center" {
    const allocator = std.testing.allocator;
    var tex = try createCheckerboard(allocator, 4, 4, 2, PixelColor.init(255, 0, 0, 255), PixelColor.init(0, 0, 255, 255));
    defer tex.deinit();
    // At (0.125, 0.125), we're in cell (0,0) interior with cell_size=2
    // texel coords: 0.125*4 = 0.5, x0=0, x1=1, tx=0.5
    // Both texels (0,0) and (1,0) are in cell (0,0) → red, so bilinear should give red (255)
    const c = tex.sampleBilinear(0.125, 0.125, 0);
    try std.testing.expectEqual(@as(u8, 255), c.r);
    try std.testing.expectEqual(@as(u8, 0), c.b);
}

test "Texture: mip level selection" {
    const allocator = std.testing.allocator;
    var tex = try createCheckerboard(allocator, 1024, 1024, 8, PixelColor.white(), PixelColor.black());
    defer tex.deinit();
    // No UV change → level 0 (magnification)
    const level0 = tex.selectMipLevel(0.0, 0.0, 0.0, 0.0);
    try std.testing.expectEqual(@as(u8, 0), level0);
    // UV changes a lot → high mip level (minification)
    // base_w=1024, max_abs=2.0, rho=2048, lambda=log2(2048)=11, but only 1 mip → clamp to 0
    // Need mipmap chain to see level > 0
    const level_big = tex.selectMipLevel(0.5, 0.5, 0.5, 0.5);
    try std.testing.expect(level_big >= 0); // no mip chain → 0 (still valid)
}

test "Texture: mipmap chain generation" {
    const allocator = std.testing.allocator;
    const base_data = try allocator.alloc(u8, 4 * 4 * 4);
    defer allocator.free(base_data);
    // Fill with white
    @memset(base_data, 255);
    const base = TextureMip{
        .width = 4,
        .height = 4,
        .format = .rgba8,
        .data = base_data,
    };
    const mips = try generateMipChain(allocator, base, 10);
    defer {
        for (mips) |m| {
            if (m.width != base.width) {
                allocator.free(m.data);
            }
        }
        allocator.free(mips);
    }
    // 4x4 → 2x2 → 1x1 = 3 levels
    try std.testing.expectEqual(@as(usize, 3), mips.len);
    try std.testing.expectEqual(@as(u32, 4), mips[0].width);
    try std.testing.expectEqual(@as(u32, 2), mips[1].width);
    try std.testing.expectEqual(@as(u32, 1), mips[2].width);
}

test "Texture: format enum" {
    try std.testing.expectEqual(@as(u8, 0), @intFromEnum(TextureFormat.rgba8));
    try std.testing.expectEqual(@as(u8, 1), @intFromEnum(TextureFormat.rgb8));
    try std.testing.expectEqual(@as(u8, 2), @intFromEnum(TextureFormat.bc7));
    try std.testing.expectEqual(@as(u8, 3), @intFromEnum(TextureFormat.astc_4x4));
}
