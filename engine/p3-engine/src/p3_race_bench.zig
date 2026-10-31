// =============================================================================
// P³ ENGINE — HEADLESS RACE BENCHMARK "NEON VECTOR"
// =============================================================================
// Same race scenario as Gemini's race_main.zig, but rendered through our
// own VisualFrameBuffer (CPU software rasterizer + triple buffer + CV
// analyzer). NO raylib, NO GPU, NO X11.
//
// This is the "correct" way to use the P3 engine for game development:
//   1. Define the scene (track + vehicles + cityscape) as world-space geometry
//   2. Each frame: project vertices, rasterize triangles to VisualFrameBuffer
//   3. Engine analyzes its own buffer (CV analyzer + tracker + scene graph)
//   4. Save observation_NNN.json (structured, ~3 KB) — NO PNG to LLM
//   5. LLM/agent can decide next action (accelerate, brake, steer, orbit cam)
//
// Gemini's race_main.zig used raylib (GPU + X11 + OpenGL). That bypasses our
// entire architecture — it's "another engine wearing our name". This file
// does it the proper way.
//
// Output: 10 frames showing the bolide going around one lap of the trefoil
//         circuit. Each frame has observation + (optional P3_DEBUG=1) PNG.
// =============================================================================

const std = @import("std");
const math = std.math;
const p3 = @import("root.zig");

const Vec3 = p3.Vec3;
const Vec4 = p3.Vec4;
const Mat4x4 = p3.Mat4x4;
const Quaternion = p3.Quaternion;

const vision = p3.p3.vision;
const VisualFrameBuffer = vision.VisualFrameBuffer;
const ProjectiveRasterizer = vision.ProjectiveRasterizer;
const ProjectedVertex = vision.ProjectedVertex;
const PixelColor = vision.PixelColor;

const RENDER_WIDTH: usize = 640;
const RENDER_HEIGHT: usize = 480;

// ---------------------------------------------------------------------------
// Entity IDs for segmentation
// ---------------------------------------------------------------------------
pub const EntityId = enum(u8) {
    void_ = 0,
    track = 1,
    track_railing_left = 2,
    track_railing_right = 3,
    boost_gate = 4,
    bolide_body = 5,
    bolide_wheel = 6,
    bolide_wing = 7,
    bolide_exhaust = 8,
    building = 9,
    star = 10,
};

// ---------------------------------------------------------------------------
// MeshVertex + IndexedMesh (same as p3_closed_loop.zig — local, to avoid
// the multi-module file conflict)
// ---------------------------------------------------------------------------
const MeshVertex = struct {
    pos: Vec3,
    normal: Vec3,
    color: [4]u8,
};

const IndexedMesh = struct {
    vertices: std.ArrayList(MeshVertex),
    indices: std.ArrayList(u16),
    allocator: std.mem.Allocator,

    fn init(allocator: std.mem.Allocator) IndexedMesh {
        return .{
            .vertices = std.ArrayList(MeshVertex).init(allocator),
            .indices = std.ArrayList(u16).init(allocator),
            .allocator = allocator,
        };
    }

    fn deinit(self: *IndexedMesh) void {
        self.vertices.deinit();
        self.indices.deinit();
    }

    fn addTriangle(self: *IndexedMesh, v0: MeshVertex, v1: MeshVertex, v2: MeshVertex) !void {
        const base: u16 = @intCast(self.vertices.items.len);
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

    fn addQuad(self: *IndexedMesh, v0: MeshVertex, v1: MeshVertex, v2: MeshVertex, v3: MeshVertex) !void {
        try self.addTriangle(v0, v1, v2);
        try self.addTriangle(v0, v2, v3);
    }

    fn addBox(self: *IndexedMesh, center: Vec3, half: Vec3, color: [4]u8) !void {
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
        try self.addTriangle(.{ .pos = c000, .normal = nx, .color = color }, .{ .pos = c010, .normal = nx, .color = color }, .{ .pos = c011, .normal = nx, .color = color });
        try self.addTriangle(.{ .pos = c000, .normal = nx, .color = color }, .{ .pos = c011, .normal = nx, .color = color }, .{ .pos = c001, .normal = nx, .color = color });
        try self.addTriangle(.{ .pos = c100, .normal = px, .color = color }, .{ .pos = c101, .normal = px, .color = color }, .{ .pos = c111, .normal = px, .color = color });
        try self.addTriangle(.{ .pos = c100, .normal = px, .color = color }, .{ .pos = c111, .normal = px, .color = color }, .{ .pos = c110, .normal = px, .color = color });
        try self.addTriangle(.{ .pos = c000, .normal = ny, .color = color }, .{ .pos = c001, .normal = ny, .color = color }, .{ .pos = c101, .normal = ny, .color = color });
        try self.addTriangle(.{ .pos = c000, .normal = ny, .color = color }, .{ .pos = c101, .normal = ny, .color = color }, .{ .pos = c100, .normal = ny, .color = color });
        try self.addTriangle(.{ .pos = c010, .normal = py, .color = color }, .{ .pos = c110, .normal = py, .color = color }, .{ .pos = c111, .normal = py, .color = color });
        try self.addTriangle(.{ .pos = c010, .normal = py, .color = color }, .{ .pos = c111, .normal = py, .color = color }, .{ .pos = c011, .normal = py, .color = color });
        try self.addTriangle(.{ .pos = c000, .normal = nz, .color = color }, .{ .pos = c100, .normal = nz, .color = color }, .{ .pos = c110, .normal = nz, .color = color });
        try self.addTriangle(.{ .pos = c000, .normal = nz, .color = color }, .{ .pos = c110, .normal = nz, .color = color }, .{ .pos = c010, .normal = nz, .color = color });
        try self.addTriangle(.{ .pos = c001, .normal = pz, .color = color }, .{ .pos = c011, .normal = pz, .color = color }, .{ .pos = c111, .normal = pz, .color = color });
        try self.addTriangle(.{ .pos = c001, .normal = pz, .color = color }, .{ .pos = c111, .normal = pz, .color = color }, .{ .pos = c101, .normal = pz, .color = color });
    }
};

// ---------------------------------------------------------------------------
// TRACK: closed trefoil-knot curve, with sampled waypoints
// ---------------------------------------------------------------------------
const TrackNode = struct {
    pos: Vec3,
    fwd: Vec3,
    right: Vec3,
    normal: Vec3,
    bank_angle: f32,
    has_gate: bool,
};

const NUM_TRACK_WAYPOINTS = 80;
const NUM_BOOST_GATES = 8;

fn generateTrackNodes(nodes: *[NUM_TRACK_WAYPOINTS]TrackNode) void {
    const major_r: f32 = 50.0;
    const minor_r: f32 = 8.0;

    var i: usize = 0;
    while (i < NUM_TRACK_WAYPOINTS) : (i += 1) {
        const u = (@as(f32, @floatFromInt(i)) / @as(f32, @floatFromInt(NUM_TRACK_WAYPOINTS))) * 2.0 * math.pi;

        const curve_x = (major_r + minor_r * @cos(3.0 * u)) * @cos(u);
        const curve_z = (major_r + minor_r * @cos(3.0 * u)) * @sin(u);
        const curve_y = 8.0 * @sin(2.0 * u) + 3.0 * @sin(5.0 * u);

        const du: f32 = 0.05;
        const next_u = u + du;
        const nx_x = (major_r + minor_r * @cos(3.0 * next_u)) * @cos(next_u);
        const nx_z = (major_r + minor_r * @cos(3.0 * next_u)) * @sin(next_u);
        const nx_y = 8.0 * @sin(2.0 * next_u) + 3.0 * @sin(5.0 * next_u);

        const fwd = Vec3.init(nx_x - curve_x, nx_y - curve_y, nx_z - curve_z).normalize();
        const up = Vec3.init(0, 1, 0);
        const right = fwd.cross(up).normalize();
        const normal = right.cross(fwd).normalize();

        nodes[i] = .{
            .pos = Vec3.init(curve_x, curve_y, curve_z),
            .fwd = fwd,
            .right = right,
            .normal = normal,
            .bank_angle = @sin(4.0 * u) * 0.3,
            .has_gate = (i % (NUM_TRACK_WAYPOINTS / NUM_BOOST_GATES) == 0),
        };
    }
}

// Sample position at fractional track_t (0..NUM_TRACK_WAYPOINTS) with linear interp
fn sampleTrack(nodes: *const [NUM_TRACK_WAYPOINTS]TrackNode, t: f32) TrackNode {
    const idx0 = @as(usize, @intFromFloat(@floor(t))) % NUM_TRACK_WAYPOINTS;
    const idx1 = (idx0 + 1) % NUM_TRACK_WAYPOINTS;
    const frac = t - @floor(t);
    const n0 = nodes[idx0];
    const n1 = nodes[idx1];
    return .{
        .pos = Vec3.init(
            n0.pos.x * (1 - frac) + n1.pos.x * frac,
            n0.pos.y * (1 - frac) + n1.pos.y * frac,
            n0.pos.z * (1 - frac) + n1.pos.z * frac,
        ),
        .fwd = Vec3.init(
            n0.fwd.x * (1 - frac) + n1.fwd.x * frac,
            n0.fwd.y * (1 - frac) + n1.fwd.y * frac,
            n0.fwd.z * (1 - frac) + n1.fwd.z * frac,
        ).normalize(),
        .right = Vec3.init(
            n0.right.x * (1 - frac) + n1.right.x * frac,
            n0.right.y * (1 - frac) + n1.right.y * frac,
            n0.right.z * (1 - frac) + n1.right.z * frac,
        ).normalize(),
        .normal = Vec3.init(
            n0.normal.x * (1 - frac) + n1.normal.x * frac,
            n0.normal.y * (1 - frac) + n1.normal.y * frac,
            n0.normal.z * (1 - frac) + n1.normal.z * frac,
        ).normalize(),
        .bank_angle = n0.bank_angle * (1 - frac) + n1.bank_angle * frac,
        .has_gate = n0.has_gate,
    };
}

// ---------------------------------------------------------------------------
// BUILDINGS: procedural cyberpunk skyscrapers
// ---------------------------------------------------------------------------
const NUM_BUILDINGS = 30;

const Building = struct {
    pos: Vec3,
    half: Vec3,
    color: [4]u8,
    neon_color: [4]u8,
    has_neon: bool,
};

fn generateBuildings(buildings: *[NUM_BUILDINGS]Building) void {
    var prng = std.Random.DefaultPrng.init(1337);
    var rng = prng.random();
    var i: usize = 0;
    while (i < NUM_BUILDINGS) : (i += 1) {
        const theta = rng.float(f32) * 2.0 * math.pi;
        const dist = 70.0 + rng.float(f32) * 40.0;
        const h = 35.0 + rng.float(f32) * 60.0;
        const w = 8.0 + rng.float(f32) * 14.0;
        const d = 8.0 + rng.float(f32) * 14.0;
        // Dark cyberpunk building color with subtle tint
        const r: u8 = 35 + rng.uintAtMost(u8, 25);
        const g: u8 = 40 + rng.uintAtMost(u8, 25);
        const b: u8 = 60 + rng.uintAtMost(u8, 35);
        const neon_r: u8 = rng.uintAtMost(u8, 100) + 155;
        const neon_g: u8 = rng.uintAtMost(u8, 80);
        const neon_b: u8 = rng.uintAtMost(u8, 100) + 155;
        buildings[i] = .{
            .pos = Vec3.init(dist * @cos(theta), h * 0.5 - 8.0, dist * @sin(theta)),
            .half = Vec3.init(w * 0.5, h * 0.5, d * 0.5),
            .color = .{ r, g, b, 255 },
            .neon_color = .{ neon_r, neon_g, neon_b, 255 },
            .has_neon = (rng.uintAtMost(u8, 100) < 60),
        };
    }
}

// ---------------------------------------------------------------------------
// LIGHTING (Lambertian + emissive override for neon entities)
// ---------------------------------------------------------------------------
fn lambertColor(base: PixelColor, normal_world: Vec3, light_dir: Vec3, emissive: bool) PixelColor {
    if (emissive) return base; // emissive surfaces don't get shaded
    const n = normal_world.normalize();
    const l = light_dir.normalize();
    var intensity = n.dot(l);
    if (intensity < 0.0) intensity = 0.0;
    const ambient: f32 = 0.25;
    const light = ambient + (1.0 - ambient) * intensity;
    return PixelColor.init(
        @intFromFloat(@min(255.0, @as(f32, @floatFromInt(base.r)) * light)),
        @intFromFloat(@min(255.0, @as(f32, @floatFromInt(base.g)) * light)),
        @intFromFloat(@min(255.0, @as(f32, @floatFromInt(base.b)) * light)),
        base.a,
    );
}

// ---------------------------------------------------------------------------
// VERTEX PROJECTION
// ---------------------------------------------------------------------------
fn projectVertex(world_pos: Vec3, vp: Mat4x4, width: f32, height: f32, base_color: PixelColor, entity_id: u8) ?ProjectedVertex {
    const clip = Mat4x4.mulVec(vp, Vec4.fromVec3Affine(world_pos));
    if (clip.w <= 0.0001) return null;
    const inv_w = 1.0 / clip.w;
    const ndc_x = clip.x * inv_w;
    const ndc_y = clip.y * inv_w;
    const ndc_z = clip.z * inv_w;
    if (ndc_x < -1.05 or ndc_x > 1.05) return null;
    if (ndc_y < -1.05 or ndc_y > 1.05) return null;
    if (ndc_z < -1.05 or ndc_z > 1.05) return null;
    const half_w = width * 0.5;
    const half_h = height * 0.5;
    return ProjectedVertex{
        .screen_x = half_w + ndc_x * half_w,
        .screen_y = half_h - ndc_y * half_h,
        .inv_w = inv_w,
        .depth = ndc_z,
        .color = base_color,
        .entity_id = entity_id,
    };
}

fn rasterizeMesh(
    rasterizer: *ProjectiveRasterizer,
    vertices: []const MeshVertex,
    indices: []const u16,
    model: Mat4x4,
    vp: Mat4x4,
    width: f32,
    height: f32,
    entity_id: u8,
    light_dir: Vec3,
    emissive: bool,
) void {
    var projected_buf: [8192]?ProjectedVertex = undefined;
    if (vertices.len > projected_buf.len) return;
    for (vertices, 0..) |v_local, i| {
        const world_pos = Mat4x4.transformPoint(model, v_local.pos);
        const world_normal = Mat4x4.transformVector(model, v_local.normal).normalize();
        const base_col = PixelColor.init(v_local.color[0], v_local.color[1], v_local.color[2], v_local.color[3]);
        const lit = lambertColor(base_col, world_normal, light_dir, emissive);
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
// BACKGROUND (deep space + stars + ground grid)
// ---------------------------------------------------------------------------
fn drawBackground(fb: *VisualFrameBuffer) void {
    var y: usize = 0;
    while (y < fb.height) : (y += 1) {
        const t = @as(f32, @floatFromInt(y)) / @as(f32, @floatFromInt(fb.height));
        // Deep purple-to-black cyberpunk gradient
        const r: u8 = @intFromFloat(15.0 + (1.0 - t) * 10.0);
        const g: u8 = @intFromFloat(5.0 + (1.0 - t) * 3.0);
        const b: u8 = @intFromFloat(30.0 + (1.0 - t) * 25.0);
        var x: usize = 0;
        while (x < fb.width) : (x += 1) {
            const idx = y * fb.width + x;
            fb.color_buffer[idx] = PixelColor.init(r, g, b, 255);
            fb.depth_buffer[idx] = 1e9;
            fb.segmentation_buffer[idx] = @intFromEnum(EntityId.void_);
        }
    }
}

fn drawStarfield(fb: *VisualFrameBuffer, seed: u64) void {
    var prng = std.Random.DefaultPrng.init(seed);
    var rng = prng.random();
    var i: usize = 0;
    while (i < 200) : (i += 1) {
        const x = rng.uintAtMost(usize, fb.width - 1);
        const y = rng.uintAtMost(usize, fb.height - 1);
        const brightness = rng.uintAtMost(u8, 200) + 55;
        const idx = y * fb.width + x;
        if (fb.depth_buffer[idx] > 1e8) {
            fb.color_buffer[idx] = PixelColor.init(brightness, brightness, brightness, 255);
            fb.depth_buffer[idx] = 1e8 - 1;
            fb.segmentation_buffer[idx] = @intFromEnum(EntityId.star);
        }
    }
}

// ---------------------------------------------------------------------------
// RACE STATE
// ---------------------------------------------------------------------------
const RaceState = struct {
    track_t: f32 = 0.0,           // position along track (0..NUM_TRACK_WAYPOINTS)
    speed_kmh: f32 = 0.0,         // current speed in km/h
    lap: u32 = 1,
    lateral_offset: f32 = 0.0,    // -5.5 to 5.5 (drift left/right of track center)
    cam_eye: Vec3 = Vec3.init(0, 8, -25), // smoothed chase camera position
    sim_time: f64 = 0.0,
    frame_id: u64 = 0,
};

// Build a 4-quad strip segment of track at a given TrackNode.
// Each segment is a flat ribbon with two side rails (railings).
fn buildTrackSegment(allocator: std.mem.Allocator, n: TrackNode, prev_n: TrackNode) !IndexedMesh {
    var mesh = IndexedMesh.init(allocator);
    errdefer mesh.deinit();
    const track_width: f32 = 8.0;
    // Track surface: 4 corners — two at this node, two at previous node
    const c_left = n.pos.add(n.right.scale(track_width * 0.5));
    const c_right = n.pos.add(n.right.scale(-track_width * 0.5));
    const p_left = prev_n.pos.add(prev_n.right.scale(track_width * 0.5));
    const p_right = prev_n.pos.add(prev_n.right.scale(-track_width * 0.5));

    // Top surface (facing up = along normal)
    const track_color = [4]u8{ 30, 32, 50, 255 }; // dark asphalt with subtle blue
    try mesh.addQuad(
        .{ .pos = p_left, .normal = n.normal, .color = track_color },
        .{ .pos = p_right, .normal = n.normal, .color = track_color },
        .{ .pos = c_right, .normal = n.normal, .color = track_color },
        .{ .pos = c_left, .normal = n.normal, .color = track_color },
    );
    // Left railing — neon cyan
    const rail_left_color = [4]u8{ 0, 240, 255, 255 };
    const c_left_top = c_left.add(n.normal.scale(1.0));
    const p_left_top = p_left.add(prev_n.normal.scale(1.0));
    try mesh.addQuad(
        .{ .pos = p_left, .normal = n.right.scale(-1), .color = rail_left_color },
        .{ .pos = p_left_top, .normal = n.right.scale(-1), .color = rail_left_color },
        .{ .pos = c_left_top, .normal = n.right.scale(-1), .color = rail_left_color },
        .{ .pos = c_left, .normal = n.right.scale(-1), .color = rail_left_color },
    );
    // Right railing — neon magenta
    const rail_right_color = [4]u8{ 255, 0, 130, 255 };
    const c_right_top = c_right.add(n.normal.scale(1.0));
    const p_right_top = p_right.add(prev_n.normal.scale(1.0));
    try mesh.addQuad(
        .{ .pos = p_right, .normal = n.right, .color = rail_right_color },
        .{ .pos = c_right, .normal = n.right, .color = rail_right_color },
        .{ .pos = c_right_top, .normal = n.right, .color = rail_right_color },
        .{ .pos = p_right_top, .normal = n.right, .color = rail_right_color },
    );
    return mesh;
}

// Build a Boost Gate (an arch over the track at a node)
fn buildBoostGate(allocator: std.mem.Allocator, n: TrackNode) !IndexedMesh {
    var mesh = IndexedMesh.init(allocator);
    errdefer mesh.deinit();
    const track_width: f32 = 8.0;
    const gate_height: f32 = 5.0;
    // Two vertical posts + horizontal top beam — all emissive green
    const gate_color = [4]u8{ 0, 255, 180, 255 };
    const post_r: f32 = 0.15;
    const c_left = n.pos.add(n.right.scale(track_width * 0.5));
    const c_right = n.pos.add(n.right.scale(-track_width * 0.5));
    // Left post
    try mesh.addBox(c_left.add(n.normal.scale(gate_height * 0.5)), Vec3.init(post_r, gate_height * 0.5, post_r), gate_color);
    // Right post
    try mesh.addBox(c_right.add(n.normal.scale(gate_height * 0.5)), Vec3.init(post_r, gate_height * 0.5, post_r), gate_color);
    // Top beam
    try mesh.addBox(
        n.pos.add(n.normal.scale(gate_height)).add(n.right.scale(0)),
        Vec3.init(track_width * 0.5 + post_r, post_r, post_r),
        gate_color,
    );
    return mesh;
}

// Build the cyber-bolide: chassis + 4 wheels + rear wing + 2 exhaust nozzles
fn buildBolide(allocator: std.mem.Allocator) !IndexedMesh {
    var mesh = IndexedMesh.init(allocator);
    errdefer mesh.deinit();
    // Main chassis — flat aerodynamic body
    try mesh.addBox(Vec3.init(0, 0.4, 0), Vec3.init(0.7, 0.18, 1.4), .{ 35, 35, 50, 255 });
    // Nose cone — slight wedge forward
    try mesh.addBox(Vec3.init(0, 0.35, 1.6), Vec3.init(0.6, 0.12, 0.3), .{ 60, 60, 80, 255 });
    // Front splitter (low lip at front-bottom)
    try mesh.addBox(Vec3.init(0, 0.12, 1.7), Vec3.init(0.8, 0.04, 0.15), .{ 255, 0, 130, 255 });
    // Rear wing — high-mounted
    try mesh.addBox(Vec3.init(0, 1.0, -1.3), Vec3.init(1.2, 0.05, 0.3), .{ 0, 240, 255, 255 });
    // Wing end plates
    try mesh.addBox(Vec3.init(1.2, 1.0, -1.3), Vec3.init(0.04, 0.25, 0.3), .{ 0, 240, 255, 255 });
    try mesh.addBox(Vec3.init(-1.2, 1.0, -1.3), Vec3.init(0.04, 0.25, 0.3), .{ 0, 240, 255, 255 });
    // 4 wheels — anti-grav discs
    const wheel_r: f32 = 0.3;
    const wheel_w: f32 = 0.15;
    try mesh.addBox(Vec3.init(0.85, 0.3, 0.9), Vec3.init(wheel_w, wheel_r, wheel_r), .{ 0, 200, 230, 255 });
    try mesh.addBox(Vec3.init(-0.85, 0.3, 0.9), Vec3.init(wheel_w, wheel_r, wheel_r), .{ 0, 200, 230, 255 });
    try mesh.addBox(Vec3.init(0.85, 0.3, -0.9), Vec3.init(wheel_w, wheel_r, wheel_r), .{ 0, 200, 230, 255 });
    try mesh.addBox(Vec3.init(-0.85, 0.3, -0.9), Vec3.init(wheel_w, wheel_r, wheel_r), .{ 0, 200, 230, 255 });
    // 2 exhaust nozzles at rear
    try mesh.addBox(Vec3.init(0.25, 0.4, -1.55), Vec3.init(0.1, 0.08, 0.15), .{ 255, 80, 30, 255 });
    try mesh.addBox(Vec3.init(-0.25, 0.4, -1.55), Vec3.init(0.1, 0.08, 0.15), .{ 255, 80, 30, 255 });
    return mesh;
}

// ---------------------------------------------------------------------------
// WRITE OUTPUT FILES
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

fn writeObservationJson(allocator: std.mem.Allocator, path: []const u8, fb: *const VisualFrameBuffer, state: *const RaceState, action_note: []const u8) !void {
    var obs = try vision.analyzeFrameBuffer(fb, allocator, state.frame_id, state.sim_time);
    defer obs.deinit();
    const obs_json = try vision.serializeObservationJson(&obs, allocator);
    defer allocator.free(obs_json);
    var f = try std.fs.cwd().createFile(path, .{});
    defer f.close();
    var bw = std.io.bufferedWriter(f.writer());
    var w = bw.writer();
    try w.writeAll("{\n  \"observation\": ");
    try w.writeAll(obs_json);
    try w.print(",\n  \"race_state\": {{ \"track_t\": {d:.4}, \"speed_kmh\": {d:.2}, \"lap\": {d}, \"lateral_offset\": {d:.2}, \"cam_eye\": [{d:.2}, {d:.2}, {d:.2}], \"action_note\": \"{s}\" }}\n}}\n", .{
        state.track_t, state.speed_kmh, state.lap, state.lateral_offset,
        state.cam_eye.x, state.cam_eye.y, state.cam_eye.z, action_note,
    });
    try bw.flush();
}

// ===========================================================================
// MAIN — render N frames of one lap
// ===========================================================================
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();
    const stdout = std.io.getStdOut().writer();

    try stdout.print("=======================================================\n", .{});
    try stdout.print("P3 ENGINE — HEADLESS RACE BENCHMARK 'NEON VECTOR'\n", .{});
    try stdout.print("Trefoil-knot circuit + cyber-bolide + cyberpunk city\n", .{});
    try stdout.print("Renderer: CPU software rasterizer (NO raylib, NO GPU)\n", .{});
    try stdout.print("Output: 10 frames per lap, structured observation JSON\n", .{});
    try stdout.print("=======================================================\n", .{});

    // Output directory
    const output_dir = "/home/z/renders/race";
    std.fs.cwd().makePath(output_dir) catch {};

    // --- Generate track + buildings ---
    var track_nodes: [NUM_TRACK_WAYPOINTS]TrackNode = undefined;
    generateTrackNodes(&track_nodes);
    var buildings: [NUM_BUILDINGS]Building = undefined;
    generateBuildings(&buildings);

    // --- Build static geometry: track segments + boost gates ---
    var track_meshes = std.ArrayList(IndexedMesh).init(allocator);
    defer {
        for (track_meshes.items) |*m| m.deinit();
        track_meshes.deinit();
    }
    var gate_meshes = std.ArrayList(IndexedMesh).init(allocator);
    defer {
        for (gate_meshes.items) |*m| m.deinit();
        gate_meshes.deinit();
    }
    var i: usize = 0;
    while (i < NUM_TRACK_WAYPOINTS) : (i += 1) {
        const n = track_nodes[i];
        const prev_n = track_nodes[(i + NUM_TRACK_WAYPOINTS - 1) % NUM_TRACK_WAYPOINTS];
        try track_meshes.append(try buildTrackSegment(allocator, n, prev_n));
        if (n.has_gate) {
            try gate_meshes.append(try buildBoostGate(allocator, n));
        }
    }

    // --- Build bolide mesh (will be transformed per frame) ---
    var bolide_mesh = try buildBolide(allocator);
    defer bolide_mesh.deinit();

    // --- Init frame buffer ---
    var fb = try VisualFrameBuffer.init(allocator, RENDER_WIDTH, RENDER_HEIGHT);
    defer fb.deinit();

    // --- Race state ---
    var state = RaceState{};
    const dt: f32 = 0.05; // 20 FPS for benchmark (one lap in ~10 frames)
    const light_dir = Vec3.init(0.5, 0.8, 0.3);

    // --- Render N frames ---
    const TOTAL_FRAMES = 10;
    var frame_idx: usize = 0;
    while (frame_idx < TOTAL_FRAMES) : (frame_idx += 1) {
        // Update race state — accelerate from 0 to ~250 km/h, lap around track
        state.speed_kmh = @min(280.0, state.speed_kmh + 35.0 * dt);
        const speed_in_nodes = (state.speed_kmh / 3.6) * 0.025; // tuned for visible motion
        state.track_t += speed_in_nodes * dt * 6.0; // accelerate lap progression
        if (state.track_t >= @as(f32, @floatFromInt(NUM_TRACK_WAYPOINTS))) {
            state.track_t -= @as(f32, @floatFromInt(NUM_TRACK_WAYPOINTS));
            state.lap += 1;
        }
        state.sim_time += dt;
        state.frame_id = frame_idx;

        // Sample current track node
        const cur_node = sampleTrack(&track_nodes, state.track_t);
        const bolide_pos = Vec3.init(
            cur_node.pos.x + cur_node.right.x * state.lateral_offset,
            cur_node.pos.y + cur_node.right.y * state.lateral_offset + 0.4,
            cur_node.pos.z + cur_node.right.z * state.lateral_offset,
        );

        // --- Chase camera (spring-arm) ---
        const cam_target = Vec3.init(
            bolide_pos.x - cur_node.fwd.x * 11.0 + cur_node.normal.x * 4.0,
            bolide_pos.y - cur_node.fwd.y * 11.0 + cur_node.normal.y * 4.0,
            bolide_pos.z - cur_node.fwd.z * 11.0 + cur_node.normal.z * 4.0,
        );
        // Smooth follow (low-pass)
        const follow_lerp: f32 = 0.3;
        state.cam_eye = Vec3.init(
            state.cam_eye.x + (cam_target.x - state.cam_eye.x) * follow_lerp,
            state.cam_eye.y + (cam_target.y - state.cam_eye.y) * follow_lerp,
            state.cam_eye.z + (cam_target.z - state.cam_eye.z) * follow_lerp,
        );

        // --- Camera matrices ---
        const cam_target_pos = Vec3.init(
            bolide_pos.x + cur_node.fwd.x * 4.0,
            bolide_pos.y + 0.8,
            bolide_pos.z + cur_node.fwd.z * 4.0,
        );
        const view = Mat4x4.createLookAt(state.cam_eye, cam_target_pos, cur_node.normal);
        const aspect: f32 = @as(f32, @floatFromInt(RENDER_WIDTH)) / @as(f32, @floatFromInt(RENDER_HEIGHT));
        const projection = Mat4x4.createProjectionFov(60.0 * math.pi / 180.0, aspect, 0.05, 500.0);
        const vp = Mat4x4.mul(projection, view);

        // --- Clear + render ---
        fb.clear(PixelColor.init(15, 5, 30, 255));
        drawBackground(&fb);
        drawStarfield(&fb, 0xCAFE0000 + frame_idx);

        var rasterizer = ProjectiveRasterizer.init(&fb);

        // 1. Render buildings (cyberpunk skyscrapers) — non-emissive (lambert)
        for (buildings) |b| {
            var building_mesh = IndexedMesh.init(allocator);
            defer building_mesh.deinit();
            try building_mesh.addBox(b.pos, b.half, b.color);
            const model = Mat4x4.identity();
            rasterizeMesh(
                &rasterizer,
                building_mesh.vertices.items,
                building_mesh.indices.items,
                model, vp,
                @as(f32, @floatFromInt(RENDER_WIDTH)),
                @as(f32, @floatFromInt(RENDER_HEIGHT)),
                @intFromEnum(EntityId.building),
                light_dir, false,
            );
            // Neon strip on top of building (emissive)
            if (b.has_neon) {
                var neon_mesh = IndexedMesh.init(allocator);
                defer neon_mesh.deinit();
                const neon_pos = Vec3.init(b.pos.x, b.pos.y + b.half.y + 0.3, b.pos.z);
                try neon_mesh.addBox(neon_pos, Vec3.init(b.half.x * 0.9, 0.15, b.half.z * 0.9), b.neon_color);
                rasterizeMesh(
                    &rasterizer,
                    neon_mesh.vertices.items,
                    neon_mesh.indices.items,
                    model, vp,
                    @as(f32, @floatFromInt(RENDER_WIDTH)),
                    @as(f32, @floatFromInt(RENDER_HEIGHT)),
                    @intFromEnum(EntityId.building),
                    light_dir, true,  // emissive!
                );
            }
        }

        // 2. Render track segments
        for (track_meshes.items) |*seg| {
            rasterizeMesh(
                &rasterizer,
                seg.vertices.items,
                seg.indices.items,
                Mat4x4.identity(), vp,
                @as(f32, @floatFromInt(RENDER_WIDTH)),
                @as(f32, @floatFromInt(RENDER_HEIGHT)),
                @intFromEnum(EntityId.track),
                light_dir, false,
            );
        }

        // 3. Render boost gates (emissive green)
        for (gate_meshes.items) |*g| {
            rasterizeMesh(
                &rasterizer,
                g.vertices.items,
                g.indices.items,
                Mat4x4.identity(), vp,
                @as(f32, @floatFromInt(RENDER_WIDTH)),
                @as(f32, @floatFromInt(RENDER_HEIGHT)),
                @intFromEnum(EntityId.boost_gate),
                light_dir, true,
            );
        }

        // 4. Render bolide (oriented to face forward along track)
        // Compose: yaw around normal (so bolide faces fwd), bank rotation, translation
        // We compute a rotation matrix from {fwd, right, normal} basis.
        const yaw_rot = Mat4x4.fromMat3x3(p3.Mat3x3.fromRows(
            cur_node.right,
            cur_node.normal,
            cur_node.fwd.scale(-1),
        ));
        const bank_rot = Mat4x4.createRotationX(cur_node.bank_angle);
        const orient = Mat4x4.mul(yaw_rot, bank_rot);
        const trans = Mat4x4.createTranslation(bolide_pos.x, bolide_pos.y, bolide_pos.z);
        const bolide_model = Mat4x4.mul(trans, orient);
        rasterizeMesh(
            &rasterizer,
            bolide_mesh.vertices.items,
            bolide_mesh.indices.items,
            bolide_model, vp,
            @as(f32, @floatFromInt(RENDER_WIDTH)),
            @as(f32, @floatFromInt(RENDER_HEIGHT)),
            @intFromEnum(EntityId.bolide_body),
            light_dir, false,
        );

        // --- Save observation_NNN.json (always) + frame_NNN.ppm (P3_DEBUG=1) ---
        const pad3 = std.fmt.allocPrint(allocator, "{d:0>3}", .{frame_idx}) catch "000";
        defer allocator.free(pad3);
        const obs_path = std.fmt.allocPrint(allocator, "{s}/observation_{s}.json", .{ output_dir, pad3 }) catch continue;
        defer allocator.free(obs_path);
        const note = std.fmt.allocPrint(allocator, "Frame {d}: lap {d}, speed {d:.1} km/h, track_t {d:.2}", .{
            frame_idx, state.lap, state.speed_kmh, state.track_t,
        }) catch "";
        defer allocator.free(note);
        try writeObservationJson(allocator, obs_path, &fb, &state, note);

        // PNG output only when P3_DEBUG=1 (for human visual audit)
        const debug_png = std.process.getEnvVarOwned(allocator, "P3_DEBUG") catch null;
        if (debug_png) |dp| {
            defer allocator.free(dp);
            const ppm_path = std.fmt.allocPrint(allocator, "{s}/frame_{s}.ppm", .{ output_dir, pad3 }) catch continue;
            defer allocator.free(ppm_path);
            try writePpm(ppm_path, &fb);
        }

        // Print summary
        var entity_counts: [16]usize = .{0} ** 16;
        for (fb.segmentation_buffer) |seg| {
            if (seg < 16) entity_counts[seg] += 1;
        }
        try stdout.print("Frame {d:>2}: lap={d} speed={d:>5.1} km/h  t={d:0>3.2}/{d}  cam=({d:>5.1},{d:>5.1},{d:>5.1})\n", .{
            frame_idx, state.lap, state.speed_kmh, state.track_t, NUM_TRACK_WAYPOINTS,
            state.cam_eye.x, state.cam_eye.y, state.cam_eye.z,
        });
        try stdout.print("  entities: track={d:>5}  rail={d:>5}  gate={d:>4}  bolide={d:>4}  bldg={d:>5}  star={d:>3}\n", .{
            entity_counts[1], entity_counts[2] + entity_counts[3], entity_counts[4],
            entity_counts[5] + entity_counts[6] + entity_counts[7] + entity_counts[8],
            entity_counts[9], entity_counts[10],
        });
    }

    try stdout.print("\n=========================================================\n", .{});
    try stdout.print("Race benchmark complete: {d} frames rendered\n", .{TOTAL_FRAMES});
    try stdout.print("Output: {s}/observation_*.json\n", .{output_dir});
    try stdout.print("Set P3_DEBUG=1 to also write PNG/PPM for human visual audit\n", .{});
    try stdout.print("=========================================================\n", .{});
}
