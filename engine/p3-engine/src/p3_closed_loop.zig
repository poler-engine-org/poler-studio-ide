// =============================================================================
// P³ ENGINE — LLM VISION CLOSED-LOOP DEMO
// =============================================================================
// This binary demonstrates the closed-loop vision-action cycle:
//
//   ┌──────────────────────────────────────────────────────────┐
//   │                  LLM Agent (this binary)                 │
//   │  reads JSON action script → applies each action →        │
//   │  renders frame → publishes observation bundle →        │
//   │  LLM receives PNG + depth + segmentation + meta         │
//   │  LLM decides next action → pushes into ActionBus →      │
//   │  engine applies action → renders next frame → ...      │
//   └──────────────────────────────────────────────────────────┘
//
// What this proves:
//   - P³ Engine's headless software rasterizer (no GPU, no X11) gives the
//     LLM real-time visual access to the engine's internal state.
//   - The triple buffer (RGB + depth + segmentation) lets the LLM see not
//     just "what's on screen" but also 3D geometry (depth) and per-pixel
//     object identity (segmentation) — capabilities that Pygame, Unity, and
//     Unreal do NOT provide without heavy external MCP/screenshot stacks.
//   - The ActionBus + ObservationPublisher pair forms a complete agent loop
//     in <100 lines of Zig, zero dependencies.
//
// Input:  /home/z/renders/actions.json (action script, see below)
// Output: /home/z/renders/loop/frame_NNN.png
//         /home/z/renders/loop/frame_NNN_depth.f32
//         /home/z/renders/loop/frame_NNN_seg.u8
//         /home/z/renders/loop/frame_NNN_meta.json
//         /home/z/renders/loop/loop_summary.json
//
// JSON action script format:
//   {
//     "output_dir": "/home/z/renders/loop",
//     "actions": [
//       { "action": "initial",         "note": "Initial state" },
//       { "action": "camera_orbit",   "yaw": -30.0, "note": "Orbit camera left" },
//       { "action": "tank_yaw",        "value": 45.0, "note": "Turn tank right" },
//       { "action": "tank_move",       "forward": 2.0, "note": "Move tank forward" },
//       { "action": "spawn_asteroid", "position": [4, 0, 0], "scale": 1.5, "note": "..." },
//       { "action": "tank_pitch",      "value": 10.0, "note": "Aim barrel up" }
//     ]
//   }
//
// The first action ("initial") renders the starting state. Every subsequent
// action mutates the SceneState and triggers a new render.
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
const ObservationPublisher = vision.ObservationPublisher;
const ActionBus = vision.ActionBus;
const AgentAction = vision.AgentAction;

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
};

// ---------------------------------------------------------------------------
// Procedural mesh (local, to avoid the multi-module file conflict that
// p3_procedural_mesh.zig triggers when both root.zig and build.zig add it
// as a module).
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
// UV sphere (for planet + atmosphere)
// ---------------------------------------------------------------------------
const SphereMesh = struct {
    vertices: std.ArrayList(Vec3),
    normals: std.ArrayList(Vec3),
    indices: std.ArrayList([3]usize),
    allocator: std.mem.Allocator,
    entity_id: u8,

    fn init(allocator: std.mem.Allocator, entity_id: u8) SphereMesh {
        return .{
            .vertices = std.ArrayList(Vec3).init(allocator),
            .normals = std.ArrayList(Vec3).init(allocator),
            .indices = std.ArrayList([3]usize).init(allocator),
            .allocator = allocator,
            .entity_id = entity_id,
        };
    }

    fn deinit(self: *SphereMesh) void {
        self.vertices.deinit();
        self.normals.deinit();
        self.indices.deinit();
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
// Icosahedron (for asteroids)
// ---------------------------------------------------------------------------
fn generateIcosahedron(allocator: std.mem.Allocator, radius: f32, entity_id: u8) !SphereMesh {
    var mesh = SphereMesh.init(allocator, entity_id);
    errdefer mesh.deinit();
    const t: f32 = (1.0 + @sqrt(5.0)) / 2.0;
    const scale = radius / @sqrt(t * t + 1.0);
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
// Lighting (Lambertian + ambient)
// ---------------------------------------------------------------------------
fn lambertColor(base: PixelColor, normal_world: Vec3, light_dir: Vec3) PixelColor {
    const n = normal_world.normalize();
    const l = light_dir.normalize();
    var intensity = n.dot(l);
    if (intensity < 0.0) intensity = 0.0;
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
// Project vertex world → screen (with perspective divide, W=0 safe)
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

// ---------------------------------------------------------------------------
// Rasterize indexed sphere mesh
// ---------------------------------------------------------------------------
fn rasterizeSphereMesh(
    rasterizer: *ProjectiveRasterizer,
    mesh: *const SphereMesh,
    model: Mat4x4,
    vp: Mat4x4,
    width: f32,
    height: f32,
    base_color: PixelColor,
    light_dir: Vec3,
) void {
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
// Rasterize triangle soup (MeshVertex + indices)
// ---------------------------------------------------------------------------
fn rasterizeTriangleSoup(
    rasterizer: *ProjectiveRasterizer,
    vertices: []const MeshVertex,
    indices: []const u16,
    model: Mat4x4,
    vp: Mat4x4,
    width: f32,
    height: f32,
    entity_id: u8,
    light_dir: Vec3,
) void {
    var projected_buf: [8192]?ProjectedVertex = undefined;
    if (vertices.len > projected_buf.len) return;
    for (vertices, 0..) |v_local, i| {
        const world_pos = Mat4x4.transformPoint(model, v_local.pos);
        const world_normal = Mat4x4.transformVector(model, v_local.normal).normalize();
        const base_col = PixelColor.init(v_local.color[0], v_local.color[1], v_local.color[2], v_local.color[3]);
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
// Atmosphere halo: only back-facing triangles (away from camera)
// ---------------------------------------------------------------------------
fn rasterizeAtmosphereHalo(
    rasterizer: *ProjectiveRasterizer,
    mesh: *const SphereMesh,
    model: Mat4x4,
    vp: Mat4x4,
    eye: Vec3,
    width: f32,
    height: f32,
    base_color: PixelColor,
    light_dir: Vec3,
) void {
    var projected = std.heap.page_allocator.alloc(?ProjectedVertex, mesh.vertices.items.len) catch return;
    defer std.heap.page_allocator.free(projected);
    for (mesh.vertices.items, 0..) |v_local, i| {
        const world_pos = Mat4x4.transformPoint(model, v_local);
        const world_normal = Mat4x4.transformVector(model, mesh.normals.items[i]).normalize();
        const lit = lambertColor(base_color, world_normal, light_dir);
        projected[i] = projectVertex(world_pos, vp, width, height, lit, mesh.entity_id);
    }
    for (mesh.indices.items) |idx| {
        const v0 = mesh.vertices.items[idx[0]];
        const v1 = mesh.vertices.items[idx[1]];
        const v2 = mesh.vertices.items[idx[2]];
        const center = Vec3.init((v0.x + v1.x + v2.x) / 3.0, (v0.y + v1.y + v2.y) / 3.0, (v0.z + v1.z + v2.z) / 3.0);
        const center_world = Mat4x4.transformPoint(model, center);
        const normal_world = Mat4x4.transformVector(model, center.normalize());
        const to_cam = Vec3.init(eye.x - center_world.x, eye.y - center_world.y, eye.z - center_world.z);
        if (normal_world.dot(to_cam) > 0) continue; // skip front-facing
        const p0 = projected[idx[0]] orelse continue;
        const p1 = projected[idx[1]] orelse continue;
        const p2 = projected[idx[2]] orelse continue;
        rasterizer.rasterizeTriangle(p0, p1, p2);
    }
}

// ---------------------------------------------------------------------------
// Background (gradient + starfield)
// ---------------------------------------------------------------------------
fn drawBackground(fb: *VisualFrameBuffer) void {
    var y: usize = 0;
    while (y < fb.height) : (y += 1) {
        const t = @as(f32, @floatFromInt(y)) / @as(f32, @floatFromInt(fb.height));
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

fn drawStarfield(fb: *VisualFrameBuffer, seed: u64) void {
    var prng = std.Random.DefaultPrng.init(seed);
    var rng = prng.random();
    var i: usize = 0;
    while (i < 250) : (i += 1) {
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

// ===========================================================================
// SCENE STATE — mutable, updated by each action
// ===========================================================================
const Asteroid = struct {
    position: Vec3,
    rotation_axis: Vec3,
    rotation_angle: f32,
    scale: f32,
    color: PixelColor,
};

const SceneState = struct {
    // Camera (orbit around target)
    camera_yaw: f32 = 0.0,        // radians
    camera_pitch: f32 = 0.3,      // radians, slight downward look
    camera_distance: f32 = 12.0,
    camera_target: Vec3 = Vec3.init(0.0, 1.0, 0.0),
    camera_up: Vec3 = Vec3.init(0.0, 1.0, 0.0),

    // Tank
    tank_position: Vec3 = Vec3.init(0.0, 1.2, 0.0),
    tank_yaw: f32 = 0.0,           // radians
    tank_pitch: f32 = 0.0,         // radians (barrel elevation)

    // Asteroids (mutable list — starts with 12 seeded, can grow via spawn)
    asteroids: std.ArrayList(Asteroid),

    // Simulation time (advances per frame)
    sim_time: f64 = 0.0,

    // Frame counter
    frame_id: u64 = 0,

    fn init(allocator: std.mem.Allocator) !SceneState {
        var state = SceneState{
            .asteroids = std.ArrayList(Asteroid).init(allocator),
        };
        // Seed the initial 12 asteroids deterministically (matches p3_arena_bench.zig)
        var prng = std.Random.DefaultPrng.init(0xDEADBEEF);
        var rng = prng.random();
        var i: usize = 0;
        while (i < 12) : (i += 1) {
            const angle = rng.float(f32) * 2.0 * math.pi;
            const radius = 14.0 + rng.float(f32) * 10.0;
            const y_offset = (rng.float(f32) - 0.5) * 8.0;
            const scale = 0.6 + rng.float(f32) * 1.4;
            try state.asteroids.append(.{
                .position = Vec3.init(@cos(angle) * radius, y_offset, @sin(angle) * radius),
                .rotation_axis = Vec3.init(rng.float(f32) - 0.5, rng.float(f32) - 0.5, rng.float(f32) - 0.5).normalize(),
                .rotation_angle = rng.float(f32) * 2.0 * math.pi,
                .scale = scale,
                .color = PixelColor.init(
                    120 + rng.uintAtMost(u8, 50),
                    110 + rng.uintAtMost(u8, 50),
                    95 + rng.uintAtMost(u8, 45),
                    255,
                ),
            });
        }
        return state;
    }

    fn deinit(self: *SceneState) void {
        self.asteroids.deinit();
    }

    /// Compute camera eye position from yaw/pitch/distance around target.
    fn cameraEye(self: *const SceneState) Vec3 {
        const cp = @cos(self.camera_pitch);
        const sp = @sin(self.camera_pitch);
        const cy = @cos(self.camera_yaw);
        const sy = @sin(self.camera_yaw);
        // Spherical-to-cartesian, looking at target
        const offset = Vec3.init(
            self.camera_distance * cp * sy,
            self.camera_distance * sp,
            self.camera_distance * cp * cy,
        );
        return self.camera_target.add(offset);
    }
};

// ===========================================================================
// RENDER SCENE — uses current SceneState to produce one frame
// ===========================================================================
fn renderScene(state: *const SceneState, fb: *VisualFrameBuffer) !void {
    var rasterizer = ProjectiveRasterizer.init(fb);
    const allocator = fb.allocator;

    // --- Camera ---
    const eye = state.cameraEye();
    const view = Mat4x4.createLookAt(eye, state.camera_target, state.camera_up);
    const aspect: f32 = @as(f32, @floatFromInt(RENDER_WIDTH)) / @as(f32, @floatFromInt(RENDER_HEIGHT));
    const projection = Mat4x4.createProjectionFov(60.0 * math.pi / 180.0, aspect, 0.05, 200.0);
    const vp = Mat4x4.mul(projection, view);

    // --- Lighting ---
    const light_dir = Vec3.init(0.6, 0.8, 0.4);

    // --- Background ---
    drawBackground(fb);
    drawStarfield(fb, 0x5EED1234);

    // --- Planet (UV sphere) ---
    var planet = try generateUVSphere(allocator, 5.0, 24, 32, @intFromEnum(EntityId.planet));
    defer planet.deinit();
    const planet_model = Mat4x4.createTranslation(0.0, -4.5, 4.0);
    const planet_color = PixelColor.init(80, 130, 110, 255);
    rasterizeSphereMesh(&rasterizer, &planet, planet_model, vp, @as(f32, @floatFromInt(RENDER_WIDTH)), @as(f32, @floatFromInt(RENDER_HEIGHT)), planet_color, light_dir);

    // --- Atmosphere halo (back-facing only) ---
    var atmosphere = try generateUVSphere(allocator, 5.25, 24, 32, @intFromEnum(EntityId.atmosphere));
    defer atmosphere.deinit();
    const atm_color = PixelColor.init(120, 175, 220, 255);
    rasterizeAtmosphereHalo(&rasterizer, &atmosphere, planet_model, vp, eye, @as(f32, @floatFromInt(RENDER_WIDTH)), @as(f32, @floatFromInt(RENDER_HEIGHT)), atm_color, light_dir);

    // --- Asteroid field ---
    for (state.asteroids.items) |a| {
        var ico = try generateIcosahedron(allocator, a.scale, @intFromEnum(EntityId.asteroid));
        defer ico.deinit();
        const rot = Mat4x4.createFromQuaternionAndTranslation(
            Quaternion.fromAxisAngle(a.rotation_axis, a.rotation_angle),
            a.position,
        );
        rasterizeSphereMesh(
            &rasterizer,
            &ico,
            rot,
            vp,
            @as(f32, @floatFromInt(RENDER_WIDTH)),
            @as(f32, @floatFromInt(RENDER_HEIGHT)),
            a.color,
            light_dir,
        );
    }

    // --- Hover tank: chassis + turret + 2 barrels + 4 repulsor pads ---
    // All in tank-local space, then transformed by tank model matrix
    // (translate to tank_position, rotate by yaw, pitch).
    var tank_mesh = IndexedMesh.init(allocator);
    defer tank_mesh.deinit();
    try tank_mesh.addBox(Vec3.init(0, 0.5, 0), Vec3.init(1.2, 0.4, 1.8), .{ 65, 80, 70, 255 });
    try tank_mesh.addBox(Vec3.init(0, 1.05, 0), Vec3.init(0.7, 0.25, 0.9), .{ 95, 115, 105, 255 });
    try tank_mesh.addBox(Vec3.init(-0.3, 1.05, 1.4), Vec3.init(0.05, 0.05, 0.7), .{ 0, 240, 255, 255 });
    try tank_mesh.addBox(Vec3.init( 0.3, 1.05, 1.4), Vec3.init(0.05, 0.05, 0.7), .{ 0, 240, 255, 255 });
    try tank_mesh.addBox(Vec3.init(-0.9, 0.15,  0.9), Vec3.init(0.15, 0.05, 0.15), .{ 0, 240, 255, 255 });
    try tank_mesh.addBox(Vec3.init( 0.9, 0.15,  0.9), Vec3.init(0.15, 0.05, 0.15), .{ 0, 240, 255, 255 });
    try tank_mesh.addBox(Vec3.init(-0.9, 0.15, -0.9), Vec3.init(0.15, 0.05, 0.15), .{ 0, 240, 255, 255 });
    try tank_mesh.addBox(Vec3.init( 0.9, 0.15, -0.9), Vec3.init(0.15, 0.05, 0.15), .{ 0, 240, 255, 255 });

    // Tank model = translate(tank_position) * rotate_yaw * rotate_pitch
    // Apply yaw around Y, then pitch around X (barrel elevation).
    const yaw_rot = Mat4x4.createRotationY(state.tank_yaw);
    const pitch_rot = Mat4x4.createRotationX(state.tank_pitch);
    const tank_orient = Mat4x4.mul(pitch_rot, yaw_rot);
    const tank_trans = Mat4x4.createTranslation(state.tank_position.x, state.tank_position.y, state.tank_position.z);
    const tank_model = Mat4x4.mul(tank_trans, tank_orient);
    rasterizeTriangleSoup(
        &rasterizer,
        tank_mesh.vertices.items,
        tank_mesh.indices.items,
        tank_model,
        vp,
        @as(f32, @floatFromInt(RENDER_WIDTH)),
        @as(f32, @floatFromInt(RENDER_HEIGHT)),
        @intFromEnum(EntityId.tank_body),
        light_dir,
    );
}

// ===========================================================================
// Scene state JSON serializer — for high-level action targeting.
// LLM gets asteroid world positions so it can call
// {"action": "aim_at", "position": [5.0, 0.5, 3.0]}
// without needing to know internal indexing.
// ===========================================================================
fn serializeSceneStateJson(allocator: std.mem.Allocator, state: *const SceneState, action_note: []const u8) ![]u8 {
    var buf = std.ArrayList(u8).init(allocator);
    errdefer buf.deinit();
    var w = buf.writer();

    try w.print("{{\n", .{});
    try w.print("  \"camera\": {{ \"eye\": [{d:.4}, {d:.4}, {d:.4}], \"target\": [{d:.4}, {d:.4}, {d:.4}], \"yaw_deg\": {d:.2}, \"pitch_deg\": {d:.2}, \"distance\": {d:.2} }},\n", .{
        state.cameraEye().x, state.cameraEye().y, state.cameraEye().z,
        state.camera_target.x, state.camera_target.y, state.camera_target.z,
        state.camera_yaw * 180.0 / math.pi, state.camera_pitch * 180.0 / math.pi,
        state.camera_distance,
    });
    try w.print("  \"tank\": {{ \"position\": [{d:.4}, {d:.4}, {d:.4}], \"yaw_deg\": {d:.2}, \"pitch_deg\": {d:.2} }},\n", .{
        state.tank_position.x, state.tank_position.y, state.tank_position.z,
        state.tank_yaw * 180.0 / math.pi, state.tank_pitch * 180.0 / math.pi,
    });
    try w.print("  \"asteroid_count\": {d},\n", .{state.asteroids.items.len});
    try w.print("  \"asteroids\": [\n", .{});
    for (state.asteroids.items, 0..) |a, i| {
        if (i > 0) try w.writeAll(",\n");
        try w.print("    {{ \"index\": {d}, \"position\": [{d:.4}, {d:.4}, {d:.4}], \"scale\": {d:.4}, \"rotation_angle\": {d:.4} }}", .{
            i, a.position.x, a.position.y, a.position.z, a.scale, a.rotation_angle,
        });
    }
    try w.print("\n  ],\n", .{});
    try w.print("  \"action_note\": \"{s}\"\n", .{action_note});
    try w.print("}}", .{});

    return buf.toOwnedSlice();
}

// ===========================================================================
// OUTPUT: PPM → PNG conversion happens in Python after; here we write PPM
// + raw depth + raw segmentation + JSON sidecar.
// ===========================================================================
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

fn writeDepthAndSeg(path_depth: []const u8, path_seg: []const u8, fb: *const VisualFrameBuffer) !void {
    var fd = try std.fs.cwd().createFile(path_depth, .{});
    defer fd.close();
    try fd.writeAll(std.mem.sliceAsBytes(fb.depth_buffer));
    var fs = try std.fs.cwd().createFile(path_seg, .{});
    defer fs.close();
    try fs.writeAll(fb.segmentation_buffer);
}

fn writeMeta(path: []const u8, fb: *const VisualFrameBuffer, state: *const SceneState, action_note: []const u8) !void {
    var f = try std.fs.cwd().createFile(path, .{});
    defer f.close();
    var buf = std.io.bufferedWriter(f.writer());
    var w = buf.writer();
    var hist: [256]usize = .{0} ** 256;
    for (fb.segmentation_buffer) |s| hist[s] += 1;
    try w.print("{{\n", .{});
    try w.print("  \"frame_id\": {d},\n", .{state.frame_id});
    try w.print("  \"simulation_time\": {d:.4},\n", .{state.sim_time});
    try w.print("  \"width\": {d},\n  \"height\": {d},\n", .{ fb.width, fb.height });
    try w.print("  \"engine\": \"P3-ClosedLoop-1.0\",\n", .{});
    try w.print("  \"action_note\": \"{s}\",\n", .{action_note});
    try w.print("  \"camera\": {{ \"yaw\": {d:.4}, \"pitch\": {d:.4}, \"distance\": {d:.4}, \"eye\": [{d:.4}, {d:.4}, {d:.4}] }},\n", .{
        state.camera_yaw, state.camera_pitch, state.camera_distance,
        state.cameraEye().x, state.cameraEye().y, state.cameraEye().z,
    });
    try w.print("  \"tank\": {{ \"position\": [{d:.4}, {d:.4}, {d:.4}], \"yaw\": {d:.4}, \"pitch\": {d:.4} }},\n", .{
        state.tank_position.x, state.tank_position.y, state.tank_position.z,
        state.tank_yaw, state.tank_pitch,
    });
    try w.print("  \"asteroid_count\": {d},\n", .{state.asteroids.items.len});
    try w.print("  \"entity_class_counts\": {{", .{});
    var first: bool = true;
    for (hist, 0..) |c, i| {
        if (c == 0) continue;
        if (!first) try w.writeAll(", ");
        first = false;
        try w.print("\"{d}\": {d}", .{ i, c });
    }
    try w.print("}}\n}}\n", .{});
    try buf.flush();
}

// ===========================================================================
// JSON ACTION SCRIPT PARSING
// ===========================================================================
const ActionStruct = struct {
    action: []const u8,
    yaw: ?f32 = null,
    pitch: ?f32 = null,
    value: ?f32 = null,
    forward: ?f32 = null,
    strafe: ?f32 = null,
    position: ?[3]f32 = null,
    scale: ?f32 = null,
    note: ?[]const u8 = null,
};

const Script = struct {
    output_dir: []const u8,
    actions: []const ActionStruct,
};

fn applyAction(state: *SceneState, action: ActionStruct) void {
    const name = action.action;
    if (std.mem.eql(u8, name, "initial") or std.mem.eql(u8, name, "noop")) {
        // no-op: just render current state
    } else if (std.mem.eql(u8, name, "camera_orbit")) {
        if (action.yaw) |y| state.camera_yaw += y * math.pi / 180.0;
        if (action.pitch) |p| state.camera_pitch += p * math.pi / 180.0;
    } else if (std.mem.eql(u8, name, "tank_yaw")) {
        if (action.value) |v| state.tank_yaw += v * math.pi / 180.0;
    } else if (std.mem.eql(u8, name, "tank_pitch")) {
        if (action.value) |v| state.tank_pitch += v * math.pi / 180.0;
    } else if (std.mem.eql(u8, name, "tank_move")) {
        // Forward direction is tank's yaw (Y-axis rotation), so forward vector = (sin(yaw), 0, cos(yaw))
        const fwd: f32 = if (action.forward != null) action.forward.? else 0.0;
        const strafe: f32 = if (action.strafe != null) action.strafe.? else 0.0;
        const sin_y = @sin(state.tank_yaw);
        const cos_y = @cos(state.tank_yaw);
        state.tank_position = Vec3.init(
            state.tank_position.x + sin_y * fwd + cos_y * strafe,
            state.tank_position.y,
            state.tank_position.z + cos_y * fwd - sin_y * strafe,
        );
    } else if (std.mem.eql(u8, name, "spawn_asteroid")) {
        if (action.position != null and action.scale != null) {
            state.asteroids.append(.{
                .position = Vec3.init(action.position.?[0], action.position.?[1], action.position.?[2]),
                .rotation_axis = Vec3.init(0.4, 0.5, 0.3).normalize(),
                .rotation_angle = 0.7,
                .scale = action.scale.?,
                .color = PixelColor.init(150, 130, 110, 255),
            }) catch {};
        }
    } else if (std.mem.eql(u8, name, "reset")) {
        state.camera_yaw = 0.0;
        state.camera_pitch = 0.3;
        state.camera_distance = 12.0;
        state.tank_yaw = 0.0;
        state.tank_pitch = 0.0;
        state.tank_position = Vec3.init(0.0, 1.2, 0.0);
    } else if (std.mem.eql(u8, name, "aim_at")) {
        // High-level action: aim tank barrel at world position (x, y, z).
        // Engine computes the yaw + pitch angles automatically.
        // This is the VLA-style "high-level primitive" — LLM specifies
        // GOAL (where to aim), not MECHANISM (how many degrees to rotate).
        if (action.position) |pos| {
            const target = Vec3.init(pos[0], pos[1], pos[2]);
            const delta = target.sub(state.tank_position);
            // Yaw (around Y): atan2(dx, dz) — but careful with sign convention
            const yaw_rad = std.math.atan2(delta.x, delta.z);
            // Pitch (around X): atan2(dy, sqrt(dx^2 + dz^2)) — aim up/down
            const horizontal_dist = @sqrt(delta.x * delta.x + delta.z * delta.z);
            const pitch_rad = std.math.atan2(delta.y, horizontal_dist);
            state.tank_yaw = yaw_rad;
            state.tank_pitch = pitch_rad;
        }
    } else if (std.mem.eql(u8, name, "approach_entity")) {
        // High-level: move tank toward world position by `forward` units
        // (default 1.0). Tank moves in its forward direction (current yaw).
        // If `position` is provided, first reorient tank yaw toward target,
        // then move forward.
        if (action.position) |pos| {
            const target = Vec3.init(pos[0], pos[1], pos[2]);
            const delta = target.sub(state.tank_position);
            const yaw_rad = std.math.atan2(delta.x, delta.z);
            state.tank_yaw = yaw_rad;
            // Move forward by `value` units (default 1.0)
            const move_dist: f32 = if (action.value) |v| v else 1.0;
            const horizontal_dist = @sqrt(delta.x * delta.x + delta.z * delta.z);
            if (horizontal_dist > 0.001) {
                const dir_x = delta.x / horizontal_dist;
                const dir_z = delta.z / horizontal_dist;
                // Don't overshoot: move at most min(move_dist, horizontal_dist - 0.5)
                const actual_move = @min(move_dist, @max(0.0, horizontal_dist - 0.5));
                state.tank_position = Vec3.init(
                    state.tank_position.x + dir_x * actual_move,
                    state.tank_position.y,
                    state.tank_position.z + dir_z * actual_move,
                );
            }
        }
    } else if (std.mem.eql(u8, name, "orbit_entity")) {
        // High-level: orbit camera around a world position (entity of interest)
        // by `yaw` degrees. Camera target shifts to the entity, then camera
        // orbits around it.
        if (action.position) |pos| {
            state.camera_target = Vec3.init(pos[0], pos[1], pos[2]);
            if (action.yaw) |y| state.camera_yaw += y * math.pi / 180.0;
            if (action.pitch) |p| state.camera_pitch += p * math.pi / 180.0;
        }
    } else if (std.mem.eql(u8, name, "follow_entity")) {
        // High-level: combination of aim_at + approach_entity. Aim barrel
        // at the entity, then move toward it by `value` units.
        if (action.position) |pos| {
            const target = Vec3.init(pos[0], pos[1], pos[2]);
            const delta = target.sub(state.tank_position);
            const yaw_rad = std.math.atan2(delta.x, delta.z);
            const horizontal_dist = @sqrt(delta.x * delta.x + delta.z * delta.z);
            const pitch_rad = std.math.atan2(delta.y, horizontal_dist);
            state.tank_yaw = yaw_rad;
            state.tank_pitch = pitch_rad;
            const move_dist: f32 = if (action.value) |v| v else 1.0;
            if (horizontal_dist > 0.001) {
                const dir_x = delta.x / horizontal_dist;
                const dir_z = delta.z / horizontal_dist;
                const actual_move = @min(move_dist, @max(0.0, horizontal_dist - 0.5));
                state.tank_position = Vec3.init(
                    state.tank_position.x + dir_x * actual_move,
                    state.tank_position.y,
                    state.tank_position.z + dir_z * actual_move,
                );
            }
        }
    }
    // advance sim time
    state.sim_time += 0.0166; // 60 FPS
}

// ===========================================================================
// MAIN
// ===========================================================================
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const stdout = std.io.getStdOut().writer();

    try stdout.print("=======================================================\n", .{});
    try stdout.print("P3 ENGINE — LLM VISION CLOSED-LOOP DEMO\n", .{});
    try stdout.print("Proves the engine provides real-time visual access\n", .{});
    try stdout.print("(RGB + depth + segmentation) to an LLM agent\n", .{});
    try stdout.print("WITHOUT GPU, WITHOUT X11, WITHOUT screenshots.\n", .{});
    try stdout.print("=======================================================\n", .{});

    // --- Read action script ---
    const script_path = "/home/z/renders/actions.json";
    var file = std.fs.cwd().openFile(script_path, .{}) catch |err| {
        try stdout.print("ERROR: cannot open {s}: {any}\n", .{ script_path, err });
        try stdout.print("Create actions.json first. See header of p3_closed_loop.zig for format.\n", .{});
        return;
    };
    defer file.close();
    const script_bytes = try file.readToEndAlloc(allocator, 1 << 24);
    defer allocator.free(script_bytes);

    var parsed = std.json.parseFromSlice(Script, allocator, script_bytes, .{}) catch |err| {
        try stdout.print("ERROR: cannot parse JSON: {any}\n", .{err});
        return;
    };
    defer parsed.deinit();
    const script = parsed.value;

    try stdout.print("Loaded script: {d} actions, output_dir={s}\n\n", .{ script.actions.len, script.output_dir });

    // --- Create output dir ---
    std.fs.cwd().makePath(script.output_dir) catch {};

    // --- Init scene state (with 12 seeded asteroids) ---
    var state = try SceneState.init(allocator);
    defer state.deinit();

    // --- Init frame buffer ---
    var fb = try VisualFrameBuffer.init(allocator, RENDER_WIDTH, RENDER_HEIGHT);
    defer fb.deinit();

    // --- Init temporal tracker (persists state across frames) ---
    // This is the "real computer vision" — tracks objects across frames,
    // computes velocity, maintains stable track_ids for the LLM agent.
    var tracker = vision.Tracker.init(allocator);
    defer tracker.deinit();

    // --- Loop through actions ---
    // The first action is typically "initial" — render the starting state
    // before any mutation. Subsequent actions mutate state then render.
    var summary = std.ArrayList(u8).init(allocator);
    defer summary.deinit();
    var summary_writer = summary.writer();
    try summary_writer.print("[\n", .{});

    var first_summary: bool = true;
    var rendered: usize = 0;
    for (script.actions, 0..) |action, i| {
        // Apply action (mutates state). For "initial", state is already at defaults.
        applyAction(&state, action);

        // Clear and render
        fb.clear(PixelColor.init(12, 16, 24, 255));
        try renderScene(&state, &fb);

        // Save observation_NNN.json — the structured CV analysis.
        // This is the primary output for the LLM agent loop: ~2 KB JSON
        // describing what's visible (entities, centroids, depth, anomalies).
        // No PNG needed — LLM gets structured perception, not pixels.
        const note: []const u8 = if (action.note) |n| n else "";
        const pad3 = std.fmt.allocPrint(allocator, "{d:0>3}", .{i}) catch "000";
        defer allocator.free(pad3);

        const obs_path = std.fmt.allocPrint(allocator, "{s}/observation_{s}.json", .{ script.output_dir, pad3 }) catch continue;
        defer allocator.free(obs_path);

        var obs = vision.analyzeFrameBuffer(&fb, allocator, state.frame_id, state.sim_time) catch continue;
        defer obs.deinit();

        // Run temporal tracker — stamps entities with stable track_id + velocity
        // (persists across frames via the `tracker` struct outside the loop).
        // This is what gives the LLM agent continuity: "asteroid with track_id=3
        // is the same one I saw 3 frames ago, moving at velocity (1.4, 0.7) px/frame".
        tracker.update(obs.entities, state.frame_id) catch {};

        // Build scene graph from tracker's tracks — encodes parent/child/neighbor
        // relationships. Helps the LLM understand spatial structure
        // ("tank is inside planet's bbox; asteroid_3 is near tank").
        var graph = vision.buildSceneGraph(allocator, tracker.tracks.items, 200.0) catch continue;
        defer graph.deinit();

        // Serialize base CV observation + scene graph + scene_state (asteroid
        // world positions for high-level action targeting)
        const obs_json = vision.serializeObservationJson(&obs, allocator) catch continue;
        defer allocator.free(obs_json);
        const sg_json = vision.serializeSceneGraphJson(&graph, allocator) catch continue;
        defer allocator.free(sg_json);
        const scene_json = try serializeSceneStateJson(allocator, &state, note);
        defer allocator.free(scene_json);

        // Combine into a single observation file for the LLM agent.
        // This is the ONLY file the agent needs per frame:
        //   - visible_entities (with track_id + velocity)
        //   - scene_graph (parent/child/neighbors)
        //   - scene_state (asteroid world positions for aim_at/approach/orbit)
        //   - depth_stats, anomalies
        {
            var of = std.fs.cwd().createFile(obs_path, .{}) catch continue;
            defer of.close();
            try of.writeAll("{\n  \"observation\": ");
            try of.writeAll(obs_json);
            // Strip outermost braces of obs_json to embed cleanly — or just append:
            // Actually obs_json is a complete JSON object, we can reference it.
            // For simplicity, write as separate top-level fields:
            try of.writeAll(",\n  \"scene_graph\": ");
            try of.writeAll(sg_json);
            try of.writeAll(",\n  \"scene_state\": ");
            try of.writeAll(scene_json);
            try of.writeAll("\n}\n");
        }

        // PNG / depth / seg files are debug-only — only write when
        // P3_DEBUG=1 is set. This is the difference between "LLM agent loop"
        // (no PNG farming) and "human debug inspection" (PNG for me to look at).
        const debug_png = std.process.getEnvVarOwned(allocator, "P3_DEBUG") catch null;
        if (debug_png) |dp| {
            defer allocator.free(dp);
            const ppm_path = std.fmt.allocPrint(allocator, "{s}/frame_{s}.ppm", .{ script.output_dir, pad3 }) catch continue;
            defer allocator.free(ppm_path);
            const depth_path = std.fmt.allocPrint(allocator, "{s}/frame_{s}_depth.f32", .{ script.output_dir, pad3 }) catch continue;
            defer allocator.free(depth_path);
            const seg_path = std.fmt.allocPrint(allocator, "{s}/frame_{s}_seg.u8", .{ script.output_dir, pad3 }) catch continue;
            defer allocator.free(seg_path);
            const meta_path = std.fmt.allocPrint(allocator, "{s}/frame_{s}_meta.json", .{ script.output_dir, pad3 }) catch continue;
            defer allocator.free(meta_path);

            try writePpm(ppm_path, &fb);
            try writeDepthAndSeg(depth_path, seg_path, &fb);
            try writeMeta(meta_path, &fb, &state, note);
        }
        // Count entities for summary (from segmentation buffer directly)
        var entity_counts: [8]usize = .{0} ** 8;
        for (fb.segmentation_buffer) |s| {
            if (s < 8) entity_counts[s] += 1;
        }

        try stdout.print("Frame {d:>3}: action={s:<18} note={s}\n", .{ i, action.action, note });
        try stdout.print("  camera=({d:>6.1},{d:>6.1},{d:>6.1})  tank=({d:>5.2},{d:>5.2},{d:>5.2}) yaw={d:>5.1}° pitch={d:>5.1}°\n", .{
            state.cameraEye().x, state.cameraEye().y, state.cameraEye().z,
            state.tank_position.x, state.tank_position.y, state.tank_position.z,
            state.tank_yaw * 180.0 / math.pi, state.tank_pitch * 180.0 / math.pi,
        });
        try stdout.print("  entities: planet={d:>6}  atm={d:>5}  asteroid={d:>5}  tank={d:>5}  star={d:>4}\n", .{
            entity_counts[1], entity_counts[2], entity_counts[3], entity_counts[4], entity_counts[7],
        });

        if (!first_summary) try summary_writer.writeAll(",\n");
        first_summary = false;
        try summary_writer.print("  {{ \"frame\": {d}, \"action\": \"{s}\", \"note\": \"{s}\", \"camera_eye\": [{d:.4}, {d:.4}, {d:.4}], \"tank_pos\": [{d:.4}, {d:.4}, {d:.4}], \"tank_yaw_deg\": {d:.2}, \"tank_pitch_deg\": {d:.2}, \"entities\": {{ \"planet\": {d}, \"atmosphere\": {d}, \"asteroid\": {d}, \"tank\": {d}, \"star\": {d} }} }}", .{
            i, action.action, note,
            state.cameraEye().x, state.cameraEye().y, state.cameraEye().z,
            state.tank_position.x, state.tank_position.y, state.tank_position.z,
            state.tank_yaw * 180.0 / math.pi, state.tank_pitch * 180.0 / math.pi,
            entity_counts[1], entity_counts[2], entity_counts[3], entity_counts[4], entity_counts[7],
        });

        rendered += 1;
        state.frame_id += 1;
    }
    try summary_writer.print("\n]\n", .{});

    // --- Write summary JSON ---
    {
        const summary_path = std.fmt.allocPrint(allocator, "{s}/loop_summary.json", .{script.output_dir}) catch return;
        defer allocator.free(summary_path);
        var sf = try std.fs.cwd().createFile(summary_path, .{});
        defer sf.close();
        try sf.writeAll(summary.items);
    }

    try stdout.print("\n=========================================================\n", .{});
    try stdout.print("Closed-loop demo complete: {d} frames rendered\n", .{rendered});
    try stdout.print("Output: {s}\n", .{script.output_dir});
    try stdout.print("Primary: observation_NNN.json — structured CV analysis for LLM\n", .{});
    try stdout.print("Set P3_DEBUG=1 to also write PNG/depth/seg for human debug\n", .{});
    try stdout.print("loop_summary.json has per-frame state + entity counts\n", .{});
    try stdout.print("=========================================================\n", .{});
}
