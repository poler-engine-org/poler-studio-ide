// =============================================================================
// P³ ENGINE — HEADLESS ARENA BENCHMARK: ASTEROID FIELD NAVIGATION
// =============================================================================
// Pure software rasterizer scenario (no GPU, no raylib, no OpenGL):
//   1. Procedural planet on a sphere (UV sphere)
//   2. 12 procedural asteroids (icosahedra at random positions)
//   3. Hover-tank (from p3_procedural_mesh) — viewpoint craft
//   4. Triple-buffer rendering: RGB + Depth + Segmentation
//   5. Output: PPM file + raw depth + segmentation + JSON sidecar
//
// Same scenario is replicated on Pygame (pure-CPU) for head-to-head
// comparison. The P³ implementation leverages the engine's projective
// rasterizer with zero division-by-zero (W=0 horizon) and built-in
// semantic segmentation — capabilities that Pygame cannot natively
// provide without re-implementing the entire software 3D pipeline.
// =============================================================================

const std = @import("std");
const math = std.math;
const p3 = @import("root.zig");

const Vec3 = p3.Vec3;
const Vec4 = p3.Vec4;
const Mat4x4 = p3.Mat4x4;

const vision = p3.p3.vision;
const VisualFrameBuffer = vision.VisualFrameBuffer;
const ProjectiveRasterizer = vision.ProjectiveRasterizer;
const ProjectedVertex = vision.ProjectedVertex;
const PixelColor = vision.PixelColor;

// Local MeshVertex used by our own procedural tank (NOT imported from
// p3_procedural_mesh.zig — that would create a Zig 0.14 multi-module conflict
// since root.zig already does a relative @import on the same file).
const MeshVertex = struct {
    pos: Vec3,
    normal: Vec3,
    color: [4]u8,
};

// ---------------------------------------------------------------------------
// Local procedural hover-tank: armored chassis + turret + barrel.
// Returns a flat triangle soup (no indices needed since we duplicate verts).
// ---------------------------------------------------------------------------
const TankMesh = struct {
    vertices: std.ArrayList(MeshVertex),
    indices: std.ArrayList(u16),
    allocator: std.mem.Allocator,

    fn init(allocator: std.mem.Allocator) TankMesh {
        return .{
            .vertices = std.ArrayList(MeshVertex).init(allocator),
            .indices = std.ArrayList(u16).init(allocator),
            .allocator = allocator,
        };
    }

    fn deinit(self: *TankMesh) void {
        self.vertices.deinit();
        self.indices.deinit();
    }

    fn addTriangle(self: *TankMesh, v0: MeshVertex, v1: MeshVertex, v2: MeshVertex) !void {
        const base: u16 = @intCast(self.vertices.items.len);
        // flat normal if normals are zero
        const e1 = v1.pos.sub(v0.pos);
        const e2 = v2.pos.sub(v0.pos);
        const n = e1.cross(e2).normalize();
        var a = v0; var b = v1; var c = v2;
        if (a.normal.lengthSq() < 0.01) a.normal = n;
        if (b.normal.lengthSq() < 0.01) b.normal = n;
        if (c.normal.lengthSq() < 0.01) c.normal = n;
        try self.vertices.append(a);
        try self.vertices.append(b);
        try self.vertices.append(c);
        try self.indices.append(base);
        try self.indices.append(base + 1);
        try self.indices.append(base + 2);
    }

    fn addBox(self: *TankMesh, center: Vec3, half: Vec3, color: [4]u8) !void {
        // 8 corners
        const x = half.x; const y = half.y; const z = half.z;
        const c000 = Vec3.init(center.x - x, center.y - y, center.z - z);
        const c001 = Vec3.init(center.x - x, center.y - y, center.z + z);
        const c010 = Vec3.init(center.x - x, center.y + y, center.z - z);
        const c011 = Vec3.init(center.x - x, center.y + y, center.z + z);
        const c100 = Vec3.init(center.x + x, center.y - y, center.z - z);
        const c101 = Vec3.init(center.x + x, center.y - y, center.z + z);
        const c110 = Vec3.init(center.x + x, center.y + y, center.z - z);
        const c111 = Vec3.init(center.x + x, center.y + y, center.z + z);
        const nx = Vec3.init(-1, 0, 0);
        const px = Vec3.init(1, 0, 0);
        const ny = Vec3.init(0, -1, 0);
        const py = Vec3.init(0, 1, 0);
        const nz = Vec3.init(0, 0, -1);
        const pz = Vec3.init(0, 0, 1);
        // -X face
        try self.addTriangle(.{ .pos = c000, .normal = nx, .color = color }, .{ .pos = c010, .normal = nx, .color = color }, .{ .pos = c011, .normal = nx, .color = color });
        try self.addTriangle(.{ .pos = c000, .normal = nx, .color = color }, .{ .pos = c011, .normal = nx, .color = color }, .{ .pos = c001, .normal = nx, .color = color });
        // +X face
        try self.addTriangle(.{ .pos = c100, .normal = px, .color = color }, .{ .pos = c101, .normal = px, .color = color }, .{ .pos = c111, .normal = px, .color = color });
        try self.addTriangle(.{ .pos = c100, .normal = px, .color = color }, .{ .pos = c111, .normal = px, .color = color }, .{ .pos = c110, .normal = px, .color = color });
        // -Y face
        try self.addTriangle(.{ .pos = c000, .normal = ny, .color = color }, .{ .pos = c001, .normal = ny, .color = color }, .{ .pos = c101, .normal = ny, .color = color });
        try self.addTriangle(.{ .pos = c000, .normal = ny, .color = color }, .{ .pos = c101, .normal = ny, .color = color }, .{ .pos = c100, .normal = ny, .color = color });
        // +Y face
        try self.addTriangle(.{ .pos = c010, .normal = py, .color = color }, .{ .pos = c110, .normal = py, .color = color }, .{ .pos = c111, .normal = py, .color = color });
        try self.addTriangle(.{ .pos = c010, .normal = py, .color = color }, .{ .pos = c111, .normal = py, .color = color }, .{ .pos = c011, .normal = py, .color = color });
        // -Z face
        try self.addTriangle(.{ .pos = c000, .normal = nz, .color = color }, .{ .pos = c100, .normal = nz, .color = color }, .{ .pos = c110, .normal = nz, .color = color });
        try self.addTriangle(.{ .pos = c000, .normal = nz, .color = color }, .{ .pos = c110, .normal = nz, .color = color }, .{ .pos = c010, .normal = nz, .color = color });
        // +Z face
        try self.addTriangle(.{ .pos = c001, .normal = pz, .color = color }, .{ .pos = c011, .normal = pz, .color = color }, .{ .pos = c111, .normal = pz, .color = color });
        try self.addTriangle(.{ .pos = c001, .normal = pz, .color = color }, .{ .pos = c111, .normal = pz, .color = color }, .{ .pos = c101, .normal = pz, .color = color });
    }
};

fn buildHoverTank(allocator: std.mem.Allocator) !TankMesh {
    var mesh = TankMesh.init(allocator);
    errdefer mesh.deinit();
    // Chassis (olive-green armored box)
    try mesh.addBox(Vec3.init(0, 0.5, 0), Vec3.init(1.2, 0.4, 1.8), .{ 65, 80, 70, 255 });
    // Turret (smaller darker box on top)
    try mesh.addBox(Vec3.init(0, 1.05, 0), Vec3.init(0.7, 0.25, 0.9), .{ 95, 115, 105, 255 });
    // Twin railgun barrels (cyan-glow thin boxes pointing forward +Z)
    try mesh.addBox(Vec3.init(-0.3, 1.05, 1.4), Vec3.init(0.05, 0.05, 0.7), .{ 0, 240, 255, 255 });
    try mesh.addBox(Vec3.init( 0.3, 1.05, 1.4), Vec3.init(0.05, 0.05, 0.7), .{ 0, 240, 255, 255 });
    // 4 anti-grav repulsor pads (cyan glow underneath)
    try mesh.addBox(Vec3.init(-0.9, 0.15,  0.9), Vec3.init(0.15, 0.05, 0.15), .{ 0, 240, 255, 255 });
    try mesh.addBox(Vec3.init( 0.9, 0.15,  0.9), Vec3.init(0.15, 0.05, 0.15), .{ 0, 240, 255, 255 });
    try mesh.addBox(Vec3.init(-0.9, 0.15, -0.9), Vec3.init(0.15, 0.05, 0.15), .{ 0, 240, 255, 255 });
    try mesh.addBox(Vec3.init( 0.9, 0.15, -0.9), Vec3.init(0.15, 0.05, 0.15), .{ 0, 240, 255, 255 });
    return mesh;
}

const RENDER_WIDTH: usize = 640;
const RENDER_HEIGHT: usize = 480;

// ---------------------------------------------------------------------------
// Entity IDs for semantic segmentation
// ---------------------------------------------------------------------------
pub const EntityId = enum(u8) {
    void_ = 0,
    planet = 1,
    atmosphere = 2,
    asteroid = 3,
    tank_body = 4,
    tank_turret = 5,
    tank_glow = 6,
    star = 7,
    thruster = 8,
};

// ---------------------------------------------------------------------------
// Procedural UV sphere (planet)
// ---------------------------------------------------------------------------
const SphereMesh = struct {
    vertices: std.ArrayList(Vec3),
    indices: std.ArrayList([3]usize),
    normals: std.ArrayList(Vec3),
    entity_id: u8,

    pub fn init(allocator: std.mem.Allocator, entity_id: u8) SphereMesh {
        return .{
            .vertices = std.ArrayList(Vec3).init(allocator),
            .indices = std.ArrayList([3]usize).init(allocator),
            .normals = std.ArrayList(Vec3).init(allocator),
            .entity_id = entity_id,
        };
    }

    pub fn deinit(self: *SphereMesh) void {
        self.vertices.deinit();
        self.indices.deinit();
        self.normals.deinit();
    }
};

fn generateUVSphere(allocator: std.mem.Allocator, radius: f32, lat_segments: usize, lon_segments: usize, entity_id: u8) !SphereMesh {
    var mesh = SphereMesh.init(allocator, entity_id);
    errdefer mesh.deinit();

    var lat: usize = 0;
    while (lat <= lat_segments) : (lat += 1) {
        const theta = (@as(f32, @floatFromInt(lat)) / @as(f32, @floatFromInt(lat_segments))) * math.pi;
        var lon: usize = 0;
        while (lon <= lon_segments) : (lon += 1) {
            const phi = (@as(f32, @floatFromInt(lon)) / @as(f32, @floatFromInt(lon_segments))) * 2.0 * math.pi;
            const x = radius * @sin(theta) * @cos(phi);
            const y = radius * @cos(theta);
            const z = radius * @sin(theta) * @sin(phi);
            try mesh.vertices.append(Vec3.init(x, y, z));
            try mesh.normals.append(Vec3.init(x, y, z).normalize());
        }
    }

    var lat_i: usize = 0;
    while (lat_i < lat_segments) : (lat_i += 1) {
        var lon_i: usize = 0;
        while (lon_i < lon_segments) : (lon_i += 1) {
            const a = lat_i * (lon_segments + 1) + lon_i;
            const b = (lat_i + 1) * (lon_segments + 1) + lon_i;
            const c = (lat_i + 1) * (lon_segments + 1) + (lon_i + 1);
            const d = lat_i * (lon_segments + 1) + (lon_i + 1);
            try mesh.indices.append(.{ a, b, c });
            try mesh.indices.append(.{ a, c, d });
        }
    }
    return mesh;
}

// ---------------------------------------------------------------------------
// Procedural icosahedron asteroid
// ---------------------------------------------------------------------------
fn generateIcosahedron(allocator: std.mem.Allocator, radius: f32, entity_id: u8) !SphereMesh {
    var mesh = SphereMesh.init(allocator, entity_id);
    errdefer mesh.deinit();

    const t: f32 = (1.0 + @sqrt(5.0)) / 2.0;
    const scale = radius / @sqrt(t * t + 1.0);

    // 12 base vertices
    const base = [_]Vec3{
        Vec3.init(-1, t, 0), Vec3.init(1, t, 0), Vec3.init(-1, -t, 0), Vec3.init(1, -t, 0),
        Vec3.init(0, -1, t), Vec3.init(0, 1, t), Vec3.init(0, -1, -t), Vec3.init(0, 1, -t),
        Vec3.init(t, 0, -1), Vec3.init(t, 0, 1), Vec3.init(-t, 0, -1), Vec3.init(-t, 0, 1),
    };
    for (base) |v| {
        try mesh.vertices.append(Vec3.init(v.x * scale, v.y * scale, v.z * scale));
        try mesh.normals.append(v.normalize());
    }

    const faces = [_][3]usize{
        .{ 0, 11, 5 },   .{ 0, 5, 1 },    .{ 0, 1, 7 },   .{ 0, 7, 10 },  .{ 0, 10, 11 },
        .{ 1, 5, 9 },    .{ 5, 11, 4 },   .{ 11, 10, 2 }, .{ 10, 7, 6 },   .{ 7, 1, 8 },
        .{ 3, 9, 4 },    .{ 3, 4, 2 },    .{ 3, 2, 6 },   .{ 3, 6, 8 },    .{ 3, 8, 9 },
        .{ 4, 9, 5 },    .{ 2, 4, 11 },   .{ 6, 2, 10 },  .{ 8, 6, 7 },    .{ 9, 8, 1 },
    };
    for (faces) |f| {
        try mesh.indices.append(.{ f[0], f[1], f[2] });
    }
    return mesh;
}

// ---------------------------------------------------------------------------
// Simple Lambertian lighting model
// ---------------------------------------------------------------------------
fn lambertColor(base: PixelColor, normal_world: Vec3, light_dir: Vec3) PixelColor {
    const n = normal_world.normalize();
    const l = light_dir.normalize();
    var intensity = n.dot(l);
    if (intensity < 0.0) intensity = 0.0;
    // Add ambient so dark side isn't fully black
    const ambient: f32 = 0.15;
    const light = ambient + (1.0 - ambient) * intensity;
    return PixelColor.init(
        @intFromFloat(@min(255.0, @as(f32, @floatFromInt(base.r)) * light)),
        @intFromFloat(@min(255.0, @as(f32, @floatFromInt(base.g)) * light)),
        @intFromFloat(@min(255.0, @as(f32, @floatFromInt(base.b)) * light)),
        base.a,
    );
}

// ---------------------------------------------------------------------------
// Transform world vertex → clip → NDC → screen with perspective divide
// vp is the combined view-projection matrix (proj * view). We multiply once,
// keep the w component, then dehomogenize to NDC and remap to screen pixels.
// ---------------------------------------------------------------------------
fn projectVertex(world_pos: Vec3, vp: Mat4x4, width: f32, height: f32, base_color: PixelColor, entity_id: u8) ?ProjectedVertex {
    // World -> clip space (still homogeneous; w may be != 1)
    const clip = Mat4x4.mulVec(vp, Vec4.fromVec3Affine(world_pos));
    if (clip.w <= 0.0001) return null; // horizon / w<=0 — projective-safe skip

    const inv_w = 1.0 / clip.w;
    const ndc_x = clip.x * inv_w;
    const ndc_y = clip.y * inv_w;
    const ndc_z = clip.z * inv_w;

    // View-space z (for near/far clipping): from the view matrix alone, z is the
    // third row of (vp * world). Without decomposing vp, we use NDC z which
    // is already in [-1,1] inside the frustum for the standard projection.
    if (ndc_x < -1.05 or ndc_x > 1.05) return null;
    if (ndc_y < -1.05 or ndc_y > 1.05) return null;
    if (ndc_z < -1.05 or ndc_z > 1.05) return null;

    const half_w = width * 0.5;
    const half_h = height * 0.5;
    const screen_x = half_w + ndc_x * half_w;
    const screen_y = half_h - ndc_y * half_h; // flip Y for screen

    return ProjectedVertex{
        .screen_x = screen_x,
        .screen_y = screen_y,
        .inv_w = inv_w,
        .depth = ndc_z, // NDC depth in [-1, 1]; closer to -1 = near camera
        .color = base_color,
        .entity_id = entity_id,
    };
}

// ---------------------------------------------------------------------------
// Rasterize a single indexed triangle from a SphereMesh
// ---------------------------------------------------------------------------
fn rasterizeMesh(
    rasterizer: *ProjectiveRasterizer,
    mesh: *const SphereMesh,
    model: Mat4x4,
    vp: Mat4x4,
    proj: Mat4x4,
    width: f32,
    height: f32,
    base_color: PixelColor,
    light_dir: Vec3,
) void {
    _ = proj;
    // Pre-compute projected vertices
    var projected = std.heap.page_allocator.alloc(?ProjectedVertex, mesh.vertices.items.len) catch return;
    defer std.heap.page_allocator.free(projected);

    for (mesh.vertices.items, 0..) |v_local, i| {
        const world_pos = Mat4x4.transformPoint(model, v_local);
        const world_normal = Mat4x4.transformVector(model, mesh.normals.items[i]).normalize();
        const lit = lambertColor(base_color, world_normal, light_dir);
        projected[i] = projectVertex(world_pos, vp, width, height, lit, mesh.entity_id);
    }

    for (mesh.indices.items) |idx| {
        const v0 = projected[idx[0]] orelse continue;
        const v1 = projected[idx[1]] orelse continue;
        const v2 = projected[idx[2]] orelse continue;
        rasterizer.rasterizeTriangle(v0, v1, v2);
    }
}

// ---------------------------------------------------------------------------
// Background starfield (procedural)
// ---------------------------------------------------------------------------
fn drawStarfield(fb: *VisualFrameBuffer, seed: u64) void {
    var prng = std.Random.DefaultPrng.init(seed);
    var rng = prng.random();
    var i: usize = 0;
    while (i < 250) : (i += 1) {
        const x = rng.uintAtMost(usize, fb.width - 1);
        const y = rng.uintAtMost(usize, fb.height - 1);
        const brightness = rng.uintAtMost(u8, 200) + 55;
        // Don't overwrite stars on top of foreground — just set if depth is still clear
        const idx = y * fb.width + x;
        if (fb.depth_buffer[idx] > 1e8) {
            fb.color_buffer[idx] = PixelColor.init(brightness, brightness, brightness, 255);
            fb.depth_buffer[idx] = 1e8 - 1; // very far
            fb.segmentation_buffer[idx] = @intFromEnum(EntityId.star);
        }
    }
}

// ---------------------------------------------------------------------------
// Background gradient (deep space to upper horizon)
// ---------------------------------------------------------------------------
fn drawBackground(fb: *VisualFrameBuffer) void {
    var y: usize = 0;
    while (y < fb.height) : (y += 1) {
        const t = @as(f32, @floatFromInt(y)) / @as(f32, @floatFromInt(fb.height));
        // Gradient from deep indigo at top to nearly black at bottom
        const r: u8 = @intFromFloat(8.0 + (1.0 - t) * 4.0);
        const g: u8 = @intFromFloat(10.0 + (1.0 - t) * 2.0);
        const b: u8 = @intFromFloat(18.0 + (1.0 - t) * 22.0);
        var x: usize = 0;
        while (x < fb.width) : (x += 1) {
            const idx = y * fb.width + x;
            fb.color_buffer[idx] = PixelColor.init(r, g, b, 255);
            fb.depth_buffer[idx] = 1e9;
            fb.segmentation_buffer[idx] = @intFromEnum(EntityId.void_);
        }
    }
}

// ---------------------------------------------------------------------------
// Asteroid field scene
// ---------------------------------------------------------------------------
const Asteroid = struct {
    position: Vec3,
    rotation_axis: Vec3,
    rotation_angle: f32,
    scale: f32,
    color: PixelColor,
};

fn generateAsteroidField(allocator: std.mem.Allocator, seed: u64) ![]Asteroid {
    var prng = std.Random.DefaultPrng.init(seed);
    var rng = prng.random();
    var list = std.ArrayList(Asteroid).init(allocator);
    errdefer list.deinit();

    var i: usize = 0;
    while (i < 12) : (i += 1) {
        const angle = rng.float(f32) * 2.0 * math.pi;
        const radius = 14.0 + rng.float(f32) * 10.0;
        const y_offset = (rng.float(f32) - 0.5) * 8.0;
        const scale = 0.6 + rng.float(f32) * 1.4;
        const asteroid = Asteroid{
            .position = Vec3.init(@cos(angle) * radius, y_offset, @sin(angle) * radius),
            .rotation_axis = Vec3.init(
                rng.float(f32) - 0.5,
                rng.float(f32) - 0.5,
                rng.float(f32) - 0.5,
            ).normalize(),
            .rotation_angle = rng.float(f32) * 2.0 * math.pi,
            .scale = scale,
            // Grayish-brown rocky asteroids with subtle color variation
            .color = PixelColor.init(
                120 + rng.uintAtMost(u8, 50),
                110 + rng.uintAtMost(u8, 50),
                95 + rng.uintAtMost(u8, 45),
                255,
            ),
        };
        try list.append(asteroid);
    }
    return list.toOwnedSlice();
}

// ---------------------------------------------------------------------------
// Tank model matrix: scale + translate to its position in the scene
// ---------------------------------------------------------------------------
fn buildTankModel(model_center: Vec3) Mat4x4 {
    const translation = Mat4x4.createTranslation(model_center.x, model_center.y, model_center.z);
    const scale_m = Mat4x4.createScale(1.0, 1.0, 1.0);
    return Mat4x4.mul(translation, scale_m);
}

// ---------------------------------------------------------------------------
// Helper: rasterize a triangle soup (vertices + indices) directly
// ---------------------------------------------------------------------------
fn rasterizeTriangleSoup(
    rasterizer: *ProjectiveRasterizer,
    vertices: []const MeshVertex,
    indices: []const u16,
    model: Mat4x4,
    vp: Mat4x4,
    proj: Mat4x4,
    width: f32,
    height: f32,
    entity_id: u8,
    light_dir: Vec3,
) void {
    _ = proj;
    // Project all vertices
    var projected_buf: [4096]?ProjectedVertex = undefined;
    if (vertices.len > projected_buf.len) return;

    for (vertices, 0..) |v_local, i| {
        const world_pos = Mat4x4.transformPoint(model, v_local.pos);
        const world_normal = Mat4x4.transformVector(model, v_local.normal).normalize();
        const base_col = PixelColor.init(
            v_local.color[0],
            v_local.color[1],
            v_local.color[2],
            v_local.color[3],
        );
        const lit = lambertColor(base_col, world_normal, light_dir);
        projected_buf[i] = projectVertex(world_pos, vp, width, height, lit, entity_id);
    }

    var i: usize = 0;
    while (i + 2 < indices.len) : (i += 3) {
        const v0 = projected_buf[indices[i]] orelse continue;
        const v1 = projected_buf[indices[i + 1]] orelse continue;
        const v2 = projected_buf[indices[i + 2]] orelse continue;
        rasterizer.rasterizeTriangle(v0, v1, v2);
    }
}

// ---------------------------------------------------------------------------
// Write PPM file
// ---------------------------------------------------------------------------
fn writePpm(path: []const u8, fb: *const VisualFrameBuffer) !void {
    var f = try std.fs.cwd().createFile(path, .{});
    defer f.close();
    var buf = std.io.bufferedWriter(f.writer());
    var w = buf.writer();
    try w.print("P6\n{d} {d}\n255\n", .{ fb.width, fb.height });
    for (fb.color_buffer) |p| {
        try w.writeByte(p.r);
        try w.writeByte(p.g);
        try w.writeByte(p.b);
    }
    try buf.flush();
}

// ---------------------------------------------------------------------------
// Write raw depth (f32 LE) + segmentation (u8) for VLM/LLM wire protocol
// ---------------------------------------------------------------------------
fn writeDepthAndSeg(path_depth: []const u8, path_seg: []const u8, fb: *const VisualFrameBuffer) !void {
    var fd = try std.fs.cwd().createFile(path_depth, .{});
    defer fd.close();
    try fd.writeAll(std.mem.sliceAsBytes(fb.depth_buffer));

    var fs = try std.fs.cwd().createFile(path_seg, .{});
    defer fs.close();
    try fs.writeAll(fb.segmentation_buffer);
}

// ---------------------------------------------------------------------------
// JSON sidecar: per-pixel-class histogram + camera metadata
// ---------------------------------------------------------------------------
fn writeJsonSidecar(path: []const u8, fb: *const VisualFrameBuffer, sim_time: f64) !void {
    var f = try std.fs.cwd().createFile(path, .{});
    defer f.close();
    var buf = std.io.bufferedWriter(f.writer());
    var w = buf.writer();

    var hist: [256]usize = .{0} ** 256;
    for (fb.segmentation_buffer) |s| hist[s] += 1;

    try w.print("{{\n  \"frame_id\": 1,\n  \"simulation_time\": {d:.4},\n", .{sim_time});
    try w.print("  \"width\": {d},\n  \"height\": {d},\n", .{ fb.width, fb.height });
    try w.print("  \"engine\": \"P3-Projective-Headless-1.0\",\n", .{});
    try w.print("  \"entity_class_counts\": {{\n", .{});
    var first: bool = true;
    for (hist, 0..) |c, i| {
        if (c == 0) continue;
        if (!first) try w.writeAll(",\n");
        first = false;
        try w.print("    \"{d}\": {d}", .{ i, c });
    }
    try w.print("\n  }}\n}}\n", .{});
    try buf.flush();
}

// ===========================================================================
// MAIN
// ===========================================================================
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const stdout = std.io.getStdOut().writer();

    try stdout.print("=====================================================\n", .{});
    try stdout.print("P3 ENGINE — HEADLESS ARENA BENCHMARK\n", .{});
    try stdout.print("Scenario: Asteroid Field + Hover Tank + Planet\n", .{});
    try stdout.print("Renderer: Pure Software Projective Rasterizer\n", .{});
    try stdout.print("Output:   {d}x{d} PPM + depth + segmentation + JSON\n", .{ RENDER_WIDTH, RENDER_HEIGHT });
    try stdout.print("=====================================================\n", .{});

    // --- Frame buffer & rasterizer ---
    var fb = try VisualFrameBuffer.init(allocator, RENDER_WIDTH, RENDER_HEIGHT);
    defer fb.deinit();

    var rasterizer = ProjectiveRasterizer.init(&fb);

    // --- Camera ---
    const eye = Vec3.init(0.0, 2.5, -12.0);
    const target = Vec3.init(0.0, 1.0, 0.0);
    const up = Vec3.init(0.0, 1.0, 0.0);
    const view = Mat4x4.createLookAt(eye, target, up);
    const aspect: f32 = @as(f32, @floatFromInt(RENDER_WIDTH)) / @as(f32, @floatFromInt(RENDER_HEIGHT));
    const projection = Mat4x4.createProjectionFov(60.0 * math.pi / 180.0, aspect, 0.05, 200.0);
    const vp = Mat4x4.mul(projection, view);

    // --- Lighting ---
    const light_dir = Vec3.init(0.6, 0.8, 0.4);

    // --- Background ---
    drawBackground(&fb);
    drawStarfield(&fb, 0x5EED1234);

    // --- Planet (pushed back, lower, so tank is in foreground) ---
    var planet = try generateUVSphere(allocator, 5.0, 24, 32, @intFromEnum(EntityId.planet));
    defer planet.deinit();
    const planet_model = Mat4x4.createTranslation(0.0, -4.5, 4.0);
    const planet_color = PixelColor.init(80, 130, 110, 255); // teal-green earthlike
    rasterizeMesh(&rasterizer, &planet, planet_model, vp, projection, @as(f32, @floatFromInt(RENDER_WIDTH)), @as(f32, @floatFromInt(RENDER_HEIGHT)), planet_color, light_dir);

    // --- Atmosphere: back-facing shell only (rim halo around planet) ---
    // We render the atmosphere as a slightly larger sphere but ONLY its
    // triangles whose normal points AWAY from the camera (back-facing in
    // view space). This gives a thin halo ring around the planet disk
    // without occluding the planet itself. No alpha blending required.
    {
        var atmosphere = try generateUVSphere(allocator, 5.25, 24, 32, @intFromEnum(EntityId.atmosphere));
        defer atmosphere.deinit();
        // Compute view direction for back-face culling. For each triangle,
        // the geometric normal is the average of vertex normals (they're
        // radial for a unit sphere centered at origin, so they equal the
        // normalized vertex position). A triangle is back-facing if
        // dot(normal_world, eye_to_triangle) > 0  (normal pointing toward camera
        // from far side → since we want only FAR/back side of atmosphere).
        // Equivalent: only keep triangles whose center's view-space z is
        // BEHIND the planet disk (z_planet in view space).
        // Simpler heuristic: triangle center distance from planet origin > planet radius.
        const atm_color = PixelColor.init(120, 175, 220, 255);
        for (atmosphere.indices.items) |idx| {
            const v0 = atmosphere.vertices.items[idx[0]];
            const v1 = atmosphere.vertices.items[idx[1]];
            const v2 = atmosphere.vertices.items[idx[2]];
            const center = Vec3.init(
                (v0.x + v1.x + v2.x) / 3.0,
                (v0.y + v1.y + v2.y) / 3.0,
                (v0.z + v1.z + v2.z) / 3.0,
            );
            // World position of triangle center
            const world_center = Mat4x4.transformPoint(planet_model, center);
            // Vector from camera to triangle center
            const to_cam = Vec3.init(eye.x - world_center.x, eye.y - world_center.y, eye.z - world_center.z);
            // World-space normal (radial from planet center)
            const world_normal = Mat4x4.transformVector(planet_model, center.normalize());
            // Keep only FAR side of the atmosphere shell (normal pointing AWAY from
            // the camera). to_cam = (eye - world_center) points from triangle to
            // camera. The triangle is on the FAR side of the shell when its outward
            // normal points AWAY from the camera → dot(normal, to_cam) < 0.
            // We CULL front-facing (visible-side) triangles where dot > 0.
            if (world_normal.dot(to_cam) > 0) continue;
            // Manually project this triangle and rasterize
            const world_v0 = Mat4x4.transformPoint(planet_model, v0);
            const world_v1 = Mat4x4.transformPoint(planet_model, v1);
            const world_v2 = Mat4x4.transformPoint(planet_model, v2);
            const lit0 = lambertColor(atm_color, Mat4x4.transformVector(planet_model, atmosphere.normals.items[idx[0]]).normalize(), light_dir);
            const lit1 = lambertColor(atm_color, Mat4x4.transformVector(planet_model, atmosphere.normals.items[idx[1]]).normalize(), light_dir);
            const lit2 = lambertColor(atm_color, Mat4x4.transformVector(planet_model, atmosphere.normals.items[idx[2]]).normalize(), light_dir);
            const p0 = projectVertex(world_v0, vp, @as(f32, @floatFromInt(RENDER_WIDTH)), @as(f32, @floatFromInt(RENDER_HEIGHT)), lit0, @intFromEnum(EntityId.atmosphere)) orelse continue;
            const p1 = projectVertex(world_v1, vp, @as(f32, @floatFromInt(RENDER_WIDTH)), @as(f32, @floatFromInt(RENDER_HEIGHT)), lit1, @intFromEnum(EntityId.atmosphere)) orelse continue;
            const p2 = projectVertex(world_v2, vp, @as(f32, @floatFromInt(RENDER_WIDTH)), @as(f32, @floatFromInt(RENDER_HEIGHT)), lit2, @intFromEnum(EntityId.atmosphere)) orelse continue;
            rasterizer.rasterizeTriangle(p0, p1, p2);
        }
    }

    // --- Asteroid field ---
    const asteroids = try generateAsteroidField(allocator, 0xDEADBEEF);
    defer allocator.free(asteroids);

    for (asteroids) |a| {
        var ico = try generateIcosahedron(allocator, a.scale, @intFromEnum(EntityId.asteroid));
        defer ico.deinit();

        // Compose model matrix: translate * rotate * scale (already in ico since radius)
        const rot = Mat4x4.createFromQuaternionAndTranslation(
            p3.Quaternion.fromAxisAngle(a.rotation_axis, a.rotation_angle),
            a.position,
        );
        rasterizeMesh(
            &rasterizer,
            &ico,
            rot,
            vp,
            projection,
            @as(f32, @floatFromInt(RENDER_WIDTH)),
            @as(f32, @floatFromInt(RENDER_HEIGHT)),
            a.color,
            light_dir,
        );
    }

    // --- Hover tank (locally-built: chassis + turret + barrels + repulsors) ---
    var tank_mesh = try buildHoverTank(allocator);
    defer tank_mesh.deinit();

    const tank_center = Vec3.init(0.0, 1.2, 0.0);
    const tank_model = buildTankModel(tank_center);
    rasterizeTriangleSoup(
        &rasterizer,
        tank_mesh.vertices.items,
        tank_mesh.indices.items,
        tank_model,
        vp,
        projection,
        @as(f32, @floatFromInt(RENDER_WIDTH)),
        @as(f32, @floatFromInt(RENDER_HEIGHT)),
        @intFromEnum(EntityId.tank_body),
        light_dir,
    );

    // --- Output files ---
    try writePpm("/home/z/renders/p3_engine_render.ppm", &fb);
    try writeDepthAndSeg("/home/z/renders/p3_engine_depth.f32", "/home/z/renders/p3_engine_seg.u8", &fb);
    try writeJsonSidecar("/home/z/renders/p3_engine_meta.json", &fb, 0.0);

    // --- Summary ---
    try stdout.print("\nRender complete:\n", .{});
    try stdout.print("  - PPM         : {d} bytes\n", .{ 64 + RENDER_WIDTH * RENDER_HEIGHT * 3 });
    try stdout.print("  - Depth (f32) : {d} bytes\n", .{ RENDER_WIDTH * RENDER_HEIGHT * 4 });
    try stdout.print("  - Segmentation: {d} bytes\n", .{ RENDER_WIDTH * RENDER_HEIGHT });
    try stdout.print("  - JSON sidecar: written\n", .{});
    try stdout.print("\nFiles at: /home/z/renders/\n", .{});

    // --- Entity counts ---
    var entity_counts: [8]usize = .{0} ** 8;
    for (fb.segmentation_buffer) |s| {
        if (s < 8) entity_counts[s] += 1;
    }
    try stdout.print("\nEntity pixel counts (semantic segmentation):\n", .{});
    try stdout.print("  Void       : {d:>6}\n", .{entity_counts[0]});
    try stdout.print("  Planet     : {d:>6}\n", .{entity_counts[1]});
    try stdout.print("  Atmosphere : {d:>6}\n", .{entity_counts[2]});
    try stdout.print("  Asteroid   : {d:>6}\n", .{entity_counts[3]});
    try stdout.print("  Tank Body  : {d:>6}\n", .{entity_counts[4]});
    try stdout.print("  Tank Turret: {d:>6}\n", .{entity_counts[5]});
    try stdout.print("  Tank Glow  : {d:>6}\n", .{entity_counts[6]});
    try stdout.print("  Star       : {d:>6}\n", .{entity_counts[7]});

    try stdout.print("\n[P3 Engine benchmark] DONE\n", .{});
}
