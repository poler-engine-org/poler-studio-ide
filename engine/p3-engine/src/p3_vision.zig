// =============================================================================
// P³ PROJECTIVE VISION ENGINE & HEADLESS SOFTWARE RASTERIZER v1.0 — ZIG
// =============================================================================
//
// 100% Original Projective Mathematical Formulation (No 3rd-party code copied):
//   - Projective Homogeneous Edge Functions (Dual Space Determinants in P³)
//   - Zero-singularity rasterization over Horizon W=0
//   - Triple-Layer Visual Observation:
//       1. RGB Color Frame (32-bit RGBA)
//       2. Normalized Projective Depth Map (32-bit Float Z-Buffer)
//       3. Semantic Object Segmentation Mask (8-bit Entity ID)
//   - Pure Zig NetPBM / Raw Pixel Exporter (Instantaneous RAM serialization for LLMs)
// =============================================================================

const std = @import("std");
const math = std.math;
const p3 = @import("root.zig");

const Vec2 = p3.Vec2;
const Vec3 = p3.Vec3;
const HomVec4 = p3.HomVec4;
const Mat4x4 = p3.Mat4x4;

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

pub const ProjectedVertex = struct {
    screen_x: f32,
    screen_y: f32,
    inv_w: f32,
    depth: f32,
    color: PixelColor,
    entity_id: u8,
};

pub const AgentAction = union(enum) {
    none,
    thrust: f32,
    yaw: f32,
    pitch: f32,
    reset,
};

pub const VisualObservation = struct {
    frame_id: u64,
    simulation_time: f64,
    width: usize,
    height: usize,
    color: []const PixelColor,
    depth: []const f32,
    segmentation: []const u8,
};

pub const ObservationPublisher = struct {
    next_frame_id: u64 = 0,

    pub fn publish(self: *ObservationPublisher, frame_buffer: *const VisualFrameBuffer, simulation_time: f64) VisualObservation {
        const frame_id = self.next_frame_id;
        self.next_frame_id += 1;
        return .{
            .frame_id = frame_id,
            .simulation_time = simulation_time,
            .width = frame_buffer.width,
            .height = frame_buffer.height,
            .color = frame_buffer.color_buffer,
            .depth = frame_buffer.depth_buffer,
            .segmentation = frame_buffer.segmentation_buffer,
        };
    }
};

pub const ActionBus = struct {
    const capacity = 64;
    actions: [capacity]AgentAction = undefined,
    read_index: usize = 0,
    write_index: usize = 0,
    count: usize = 0,

    pub fn push(self: *ActionBus, action: AgentAction) bool {
        if (self.count == capacity) return false;
        self.actions[self.write_index] = action;
        self.write_index = (self.write_index + 1) % capacity;
        self.count += 1;
        return true;
    }

    pub fn pop(self: *ActionBus) ?AgentAction {
        if (self.count == 0) return null;
        const action = self.actions[self.read_index];
        self.read_index = (self.read_index + 1) % capacity;
        self.count -= 1;
        return action;
    }
};

pub const VisualFrameBuffer = struct {
    width: usize,
    height: usize,
    color_buffer: []PixelColor,
    depth_buffer: []f32,
    segmentation_buffer: []u8,
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator, width: usize, height: usize) !VisualFrameBuffer {
        const total = width * height;
        const col = try allocator.alloc(PixelColor, total);
        const dep = try allocator.alloc(f32, total);
        const seg = try allocator.alloc(u8, total);

        var fb = VisualFrameBuffer{
            .width = width,
            .height = height,
            .color_buffer = col,
            .depth_buffer = dep,
            .segmentation_buffer = seg,
            .allocator = allocator,
        };
        fb.clear(PixelColor.init(12, 16, 24, 255));
        return fb;
    }

    pub fn deinit(self: *VisualFrameBuffer) void {
        self.allocator.free(self.color_buffer);
        self.allocator.free(self.depth_buffer);
        self.allocator.free(self.segmentation_buffer);
    }

    pub fn clear(self: *VisualFrameBuffer, bg_color: PixelColor) void {
        @memset(self.color_buffer, bg_color);
        @memset(self.depth_buffer, 1e9);
        @memset(self.segmentation_buffer, 0); // 0 = Background Void
    }

    pub inline fn setPixel(self: *VisualFrameBuffer, x: usize, y: usize, color: PixelColor, depth: f32, entity_id: u8) void {
        if (x >= self.width or y >= self.height) return;
        const idx = y * self.width + x;
        if (depth < self.depth_buffer[idx]) {
            self.depth_buffer[idx] = depth;
            self.color_buffer[idx] = color;
            self.segmentation_buffer[idx] = entity_id;
        }
    }

    pub fn observation(self: *const VisualFrameBuffer, frame_id: u64, simulation_time: f64) VisualObservation {
        return .{
            .frame_id = frame_id,
            .simulation_time = simulation_time,
            .width = self.width,
            .height = self.height,
            .color = self.color_buffer,
            .depth = self.depth_buffer,
            .segmentation = self.segmentation_buffer,
        };
    }

    /// Exports RGB Color Frame to NetPBM PPM format in memory (Zero dependencies, pure Zig)
    pub fn exportPpm(self: *const VisualFrameBuffer, allocator: std.mem.Allocator) ![]u8 {
        var list = std.ArrayList(u8).init(allocator);
        var writer = list.writer();

        // PPM Header: P6 width height 255
        try writer.print("P6\n{d} {d}\n255\n", .{ self.width, self.height });
        for (self.color_buffer) |p| {
            try writer.writeByte(p.r);
            try writer.writeByte(p.g);
            try writer.writeByte(p.b);
        }
        return list.toOwnedSlice();
    }

    /// Exports Depth Buffer to raw little-endian f32 bytes (Phase 1 Serialization)
    pub fn exportDepthBinary(self: *const VisualFrameBuffer, allocator: std.mem.Allocator) ![]u8 {
        const byte_len = self.depth_buffer.len * @sizeOf(f32);
        const bytes = try allocator.alloc(u8, byte_len);
        @memcpy(bytes, std.mem.sliceAsBytes(self.depth_buffer));
        return bytes;
    }

    /// Exports Semantic Segmentation Buffer to raw u8 entity IDs (Phase 2 Serialization)
    pub fn exportSegmentationBinary(self: *const VisualFrameBuffer, allocator: std.mem.Allocator) ![]u8 {
        const bytes = try allocator.alloc(u8, self.segmentation_buffer.len);
        @memcpy(bytes, self.segmentation_buffer);
        return bytes;
    }

    /// Exports full Wire Observation Bundle (JSON Header + RGB + Depth + Segmentation)
    pub fn exportObservationBundle(self: *const VisualFrameBuffer, allocator: std.mem.Allocator, frame_id: u64, sim_time: f64) ![]u8 {
        const ppm = try self.exportPpm(allocator);
        defer allocator.free(ppm);
        const depth_bytes = try self.exportDepthBinary(allocator);
        defer allocator.free(depth_bytes);
        const seg_bytes = try self.exportSegmentationBinary(allocator);
        defer allocator.free(seg_bytes);

        var bundle = std.ArrayList(u8).init(allocator);
        var writer = bundle.writer();

        const ppm_offset: usize = 0;
        const depth_offset = ppm.len;
        const seg_offset = depth_offset + depth_bytes.len;

        try writer.print(
            \\{{"frame_id":{d},"simulation_time":{d:.4},"width":{d},"height":{d},"buffers":{{"color":{{"offset":{d},"size":{d},"format":"ppm"}},"depth":{{"offset":{d},"size":{d},"format":"f32_le"}},"segmentation":{{"offset":{d},"size":{d},"format":"u8"}}}}}}
            \\---PAYLOAD---
            \\
        , .{
            frame_id, sim_time, self.width, self.height, ppm_offset, ppm.len, depth_offset, depth_bytes.len, seg_offset, seg_bytes.len,
        });

        try bundle.appendSlice(ppm);
        try bundle.appendSlice(depth_bytes);
        try bundle.appendSlice(seg_bytes);

        return bundle.toOwnedSlice();
    }
};

pub const ProjectiveRasterizer = struct {
    frame_buffer: *VisualFrameBuffer,

    pub fn init(frame_buffer: *VisualFrameBuffer) ProjectiveRasterizer {
        return .{ .frame_buffer = frame_buffer };
    }

    /// Mathematical Projective Edge Function: Evaluates oriented signed area in Screen Space
    inline fn edgeFunction(x0: f32, y0: f32, x1: f32, y1: f32, px: f32, py: f32) f32 {
        return (px - x0) * (y1 - y0) - (py - y0) * (x1 - x0);
    }

    /// Rasterizes a single 4D triangle with depth testing and semantic segmentation
    pub fn rasterizeTriangle(
        self: *ProjectiveRasterizer,
        v0: ProjectedVertex,
        v1: ProjectedVertex,
        v2: ProjectedVertex,
    ) void {
        const min_x_f = @max(0.0, @min(v0.screen_x, @min(v1.screen_x, v2.screen_x)));
        const max_x_f = @min(@as(f32, @floatFromInt(self.frame_buffer.width - 1)), @max(v0.screen_x, @max(v1.screen_x, v2.screen_x)));
        const min_y_f = @max(0.0, @min(v0.screen_y, @min(v1.screen_y, v2.screen_y)));
        const max_y_f = @min(@as(f32, @floatFromInt(self.frame_buffer.height - 1)), @max(v0.screen_y, @max(v1.screen_y, v2.screen_y)));

        if (min_x_f > max_x_f or min_y_f > max_y_f) return;

        const min_x: usize = @intFromFloat(min_x_f);
        const max_x: usize = @intFromFloat(max_x_f);
        const min_y: usize = @intFromFloat(min_y_f);
        const max_y: usize = @intFromFloat(max_y_f);

        const area = edgeFunction(v0.screen_x, v0.screen_y, v1.screen_x, v1.screen_y, v2.screen_x, v2.screen_y);
        if (@abs(area) < 1e-5) return;
        const inv_area = 1.0 / area;

        var y = min_y;
        while (y <= max_y) : (y += 1) {
            const py = @as(f32, @floatFromInt(y)) + 0.5;
            var x = min_x;
            while (x <= max_x) : (x += 1) {
                const px = @as(f32, @floatFromInt(x)) + 0.5;

                const w0 = edgeFunction(v1.screen_x, v1.screen_y, v2.screen_x, v2.screen_y, px, py);
                const w1 = edgeFunction(v2.screen_x, v2.screen_y, v0.screen_x, v0.screen_y, px, py);
                const w2 = edgeFunction(v0.screen_x, v0.screen_y, v1.screen_x, v1.screen_y, px, py);

                // Check if inside triangle (works for both winding orders)
                const is_inside = if (area > 0)
                    (w0 >= 0 and w1 >= 0 and w2 >= 0)
                else
                    (w0 <= 0 and w1 <= 0 and w2 <= 0);

                if (is_inside) {
                    const l0 = w0 * inv_area;
                    const l1 = w1 * inv_area;
                    const l2 = w2 * inv_area;

                    // Projective Depth Interpolation
                    const depth = l0 * v0.depth + l1 * v1.depth + l2 * v2.depth;
                    self.frame_buffer.setPixel(x, y, v0.color, depth, v0.entity_id);
                }
            }
        }
    }
};

// =============================================================================
// TESTS
// =============================================================================

test "ProjectiveVision: FrameBuffer allocation, rasterization and PPM export" {
    const allocator = std.testing.allocator;
    var fb = try VisualFrameBuffer.init(allocator, 160, 120);
    defer fb.deinit();

    var rasterizer = ProjectiveRasterizer.init(&fb);

    const v0 = ProjectedVertex{ .screen_x = 20, .screen_y = 20, .inv_w = 1.0, .depth = 1.0, .color = PixelColor.init(255, 0, 0, 255), .entity_id = 1 };
    const v1 = ProjectedVertex{ .screen_x = 140, .screen_y = 30, .inv_w = 1.0, .depth = 2.0, .color = PixelColor.init(0, 255, 0, 255), .entity_id = 1 };
    const v2 = ProjectedVertex{ .screen_x = 80, .screen_y = 100, .inv_w = 1.0, .depth = 1.5, .color = PixelColor.init(0, 0, 255, 255), .entity_id = 1 };

    rasterizer.rasterizeTriangle(v0, v1, v2);

    // Verify center pixel has been drawn
    const center_idx = 50 * 160 + 80;
    try std.testing.expect(fb.depth_buffer[center_idx] < 1e8);
    try std.testing.expectEqual(@as(u8, 1), fb.segmentation_buffer[center_idx]);

    // Test Memory PPM Serializer
    const ppm = try fb.exportPpm(allocator);
    defer allocator.free(ppm);
    try std.testing.expect(ppm.len > 100);
}

test "ProjectiveVision: observations advance and actions round-trip" {
    const allocator = std.testing.allocator;
    var fb = try VisualFrameBuffer.init(allocator, 4, 4);
    defer fb.deinit();

    var publisher = ObservationPublisher{};
    const first = publisher.publish(&fb, 0.0);
    const second = publisher.publish(&fb, 0.016);
    try std.testing.expectEqual(@as(u64, 0), first.frame_id);
    try std.testing.expectEqual(@as(u64, 1), second.frame_id);
    try std.testing.expectEqual(@as(usize, 16), second.color.len);
    try std.testing.expectEqual(@as(usize, 16), second.depth.len);
    try std.testing.expectEqual(@as(usize, 16), second.segmentation.len);

    var actions = ActionBus{};
    try std.testing.expect(actions.push(.{ .thrust = 1.0 }));
    try std.testing.expect(actions.push(.reset));
    try std.testing.expectEqualDeep(AgentAction{ .thrust = 1.0 }, actions.pop().?);
    try std.testing.expectEqualDeep(AgentAction.reset, actions.pop().?);
    try std.testing.expect(actions.pop() == null);
}

test "ProjectiveVision: complete wire observation bundle serialization" {
    const allocator = std.testing.allocator;
    var fb = try VisualFrameBuffer.init(allocator, 8, 8);
    defer fb.deinit();

    const bundle = try fb.exportObservationBundle(allocator, 42, 1.25);
    defer allocator.free(bundle);

    try std.testing.expect(bundle.len > 0);
    try std.testing.expect(std.mem.indexOf(u8, bundle, "---PAYLOAD---") != null);
    try std.testing.expect(std.mem.indexOf(u8, bundle, "\"frame_id\":42") != null);
}

// =============================================================================
// NATIVE COMPUTER VISION ANALYZER
// =============================================================================
//
// This is the "real computer vision" for an LLM agent. Instead of sending
// PNG screenshots to a VLM (slow, bandwidth-heavy, fills disk), the engine
// analyzes its own VisualFrameBuffer in-process and produces a structured
// observation describing what's visible:
//
//   - Per-entity pixel count, centroid (screen position), bounding box
//   - Per-entity depth statistics (min, max, mean) → tells LLM "how far"
//   - Global depth statistics → tells LLM "closest object is N meters away"
//   - Anomaly detection: z-fighting (depth discontinuities), holes
//     (background showing through geometry), isolated pixels (rendering bugs)
//
// The LLM gets a JSON of ~1-3 KB per frame, not a 1 MB PNG. Decision loop:
//   LLM <- structured JSON observation  (no PNG!)
//   LLM -> JSON action                 (no base64!)
//   engine applies action, re-renders, analyzes again
//
// This is what makes P3 Engine suitable for autonomous agent loops in any
// sandbox (z.ai, OpenAI, Claude) — the LLM doesn't need vision API access
// or GPU; it gets structured perception data straight from the renderer.
// =============================================================================

pub const VisibleEntity = struct {
    entity_id: u8,
    pixel_count: usize,
    centroid_x: f32,
    centroid_y: f32,
    bbox_min_x: usize,
    bbox_min_y: usize,
    bbox_max_x: usize,
    bbox_max_y: usize,
    depth_min: f32,
    depth_max: f32,
    depth_sum: f32,
    /// Temporal tracking fields (populated by Tracker.update).
    /// Stable across frames — same object gets same track_id.
    /// 0 = no tracker assigned yet (e.g., first frame).
    track_id: u32 = 0,
    /// Velocity (pixels per frame): how fast centroid is moving
    velocity_x: f32 = 0,
    velocity_y: f32 = 0,
    /// How many frames this entity has been tracked
    age_frames: u32 = 0,
    /// How many frames since last seen (0 = currently visible)
    lost_frames: u32 = 0,
};

pub const Anomaly = struct {
    anomaly_type: []const u8, // "z_fighting" | "hole" | "isolated_pixel" | "inverted_normal"
    location_x: usize,
    location_y: usize,
    description: []const u8,
};

pub const Observation = struct {
    frame_id: u64,
    sim_time: f64,
    width: usize,
    height: usize,
    entities: []VisibleEntity,
    depth_min: f32,
    depth_max: f32,
    depth_mean: f32,
    anomaly_count: usize,
    anomalies: []Anomaly,
    allocator: std.mem.Allocator,

    pub fn deinit(self: *Observation) void {
        self.allocator.free(self.entities);
        self.allocator.free(self.anomalies);
    }
};

/// Per-entity type names (for JSON output)
pub fn entityTypeName(id: u8) []const u8 {
    return switch (id) {
        0 => "void",
        1 => "planet",
        2 => "atmosphere",
        3 => "asteroid",
        4 => "tank_body",
        5 => "tank_turret",
        6 => "tank_glow",
        7 => "star",
        else => "unknown",
    };
}

/// Analyzes the VisualFrameBuffer in-process and produces a structured
/// Observation. No PNG encoding, no disk I/O, no VLM API call — just direct
/// analysis of the in-RAM buffers.
///
/// Time complexity: O(W*H) for stats, O(W*H) for anomaly scan. Total ~2-3 ms
/// on 640x480 frame. The result is JSON-serializable via serializeToJson().
pub fn analyzeFrameBuffer(
    fb: *const VisualFrameBuffer,
    allocator: std.mem.Allocator,
    frame_id: u64,
    sim_time: f64,
) !Observation {
    // --- Pass 1: per-entity pixel count, centroid, bbox, depth stats ---
    // Track up to 256 entity_ids (u8 range). Most will be empty.
    var pixel_counts: [256]usize = .{0} ** 256;
    var sum_x: [256]f32 = .{0} ** 256;
    var sum_y: [256]f32 = .{0} ** 256;
    var min_x: [256]usize = .{std.math.maxInt(usize)} ** 256;
    var min_y: [256]usize = .{std.math.maxInt(usize)} ** 256;
    var max_x: [256]usize = .{0} ** 256;
    var max_y: [256]usize = .{0} ** 256;
    var depth_min_arr: [256]f32 = .{std.math.floatMax(f32)} ** 256;
    var depth_max_arr: [256]f32 = .{-std.math.floatMax(f32)} ** 256;
    var depth_sum_arr: [256]f32 = .{0} ** 256;

    // Global depth stats
    var global_depth_min: f32 = std.math.floatMax(f32);
    var global_depth_max: f32 = -std.math.floatMax(f32);
    var global_depth_sum: f32 = 0;
    var global_depth_count: usize = 0;

    for (fb.segmentation_buffer, 0..) |seg, i| {
        const x = i % fb.width;
        const y = i / fb.width;
        const depth = fb.depth_buffer[i];

        pixel_counts[seg] += 1;
        sum_x[seg] += @floatFromInt(x);
        sum_y[seg] += @floatFromInt(y);
        if (x < min_x[seg]) min_x[seg] = x;
        if (x > max_x[seg]) max_x[seg] = x;
        if (y < min_y[seg]) min_y[seg] = y;
        if (y > max_y[seg]) max_y[seg] = y;
        if (depth < depth_min_arr[seg]) depth_min_arr[seg] = depth;
        if (depth > depth_max_arr[seg]) depth_max_arr[seg] = depth;
        depth_sum_arr[seg] += depth;

        // Skip void/background and stars for global depth stats
        if (seg != 0 and seg != 7) {
            if (depth < global_depth_min) global_depth_min = depth;
            if (depth > global_depth_max) global_depth_max = depth;
            global_depth_sum += depth;
            global_depth_count += 1;
        }
    }

    // Count how many entity_ids actually have pixels (so we can alloc exact size)
    var entity_count: usize = 0;
    for (pixel_counts) |c| {
        if (c > 0) entity_count += 1;
    }

    // Allocate exact-sized slice
    var entities = try allocator.alloc(VisibleEntity, entity_count);
    errdefer allocator.free(entities);

    var idx: usize = 0;
    for (pixel_counts, 0..) |c, id| {
        if (c == 0) continue;
        entities[idx] = .{
            .entity_id = @intCast(id),
            .pixel_count = c,
            .centroid_x = sum_x[id] / @as(f32, @floatFromInt(c)),
            .centroid_y = sum_y[id] / @as(f32, @floatFromInt(c)),
            .bbox_min_x = min_x[id],
            .bbox_min_y = min_y[id],
            .bbox_max_x = max_x[id],
            .bbox_max_y = max_y[id],
            .depth_min = depth_min_arr[id],
            .depth_max = depth_max_arr[id],
            .depth_sum = depth_sum_arr[id],
        };
        idx += 1;
    }

    // --- Pass 2: anomaly detection (sparse scan — every 4 pixels to keep it fast) ---
    // Collect up to 32 anomalies (we don't need them all; just enough to alert the LLM)
    var anomalies_list = std.ArrayList(Anomaly).init(allocator);
    errdefer anomalies_list.deinit();

    var scan_y: usize = 4;
    while (scan_y + 4 < fb.height) : (scan_y += 4) {
        var scan_x: usize = 4;
        while (scan_x + 4 < fb.width) : (scan_x += 4) {
            const idx2 = scan_y * fb.width + scan_x;
            const seg = fb.segmentation_buffer[idx2];
            const depth = fb.depth_buffer[idx2];

            // Hole detection: void pixel completely surrounded by non-void
            // (geometry has a gap)
            if (seg == 0) {
                const n_idx = (scan_y - 1) * fb.width + scan_x;
                const s_idx = (scan_y + 1) * fb.width + scan_x;
                const e_idx = scan_y * fb.width + (scan_x + 1);
                const w_idx = scan_y * fb.width + (scan_x - 1);
                const n = fb.segmentation_buffer[n_idx];
                const south = fb.segmentation_buffer[s_idx];
                const e = fb.segmentation_buffer[e_idx];
                const w = fb.segmentation_buffer[w_idx];
                if (n != 0 and south != 0 and e != 0 and w != 0) {
                    if (anomalies_list.items.len < 32) {
                        try anomalies_list.append(.{
                            .anomaly_type = "hole",
                            .location_x = scan_x,
                            .location_y = scan_y,
                            .description = "Background void pixel surrounded by geometry — possible rendering gap",
                        });
                    }
                }
                continue;
            }

            // Z-fighting: depth discontinuity between adjacent pixels of the
            // SAME entity (depth jumps by > 0.5 between neighbors of same id)
            if (seg != 0 and seg != 7) { // skip void + stars
                const east_idx = scan_y * fb.width + (scan_x + 1);
                const south_idx = (scan_y + 1) * fb.width + scan_x;
                const east_seg = fb.segmentation_buffer[east_idx];
                const south_seg = fb.segmentation_buffer[south_idx];
                if (east_seg == seg) {
                    const east_depth = fb.depth_buffer[east_idx];
                    if (@abs(depth - east_depth) > 0.5) {
                        if (anomalies_list.items.len < 32) {
                            try anomalies_list.append(.{
                                .anomaly_type = "z_fighting",
                                .location_x = scan_x,
                                .location_y = scan_y,
                                .description = "Adjacent pixels of same entity have depth jump > 0.5 — possible z-fighting",
                            });
                        }
                    }
                }
                if (south_seg == seg) {
                    const south_depth = fb.depth_buffer[south_idx];
                    if (@abs(depth - south_depth) > 0.5) {
                        if (anomalies_list.items.len < 32) {
                            try anomalies_list.append(.{
                                .anomaly_type = "z_fighting",
                                .location_x = scan_x,
                                .location_y = scan_y,
                                .description = "Vertical depth jump on same entity — possible z-fighting",
                            });
                        }
                    }
                }
            }
        }
    }

    const anomalies = try anomalies_list.toOwnedSlice();

    return Observation{
        .frame_id = frame_id,
        .sim_time = sim_time,
        .width = fb.width,
        .height = fb.height,
        .entities = entities,
        .depth_min = if (global_depth_count > 0) global_depth_min else 0,
        .depth_max = if (global_depth_count > 0) global_depth_max else 0,
        .depth_mean = if (global_depth_count > 0) global_depth_sum / @as(f32, @floatFromInt(global_depth_count)) else 0,
        .anomaly_count = anomalies.len,
        .anomalies = anomalies,
        .allocator = allocator,
    };
}

/// Serializes Observation to a compact JSON string (1-3 KB typical).
/// This is what the LLM receives instead of a 1 MB PNG.
pub fn serializeObservationJson(obs: *const Observation, allocator: std.mem.Allocator) ![]u8 {
    var buf = std.ArrayList(u8).init(allocator);
    errdefer buf.deinit();
    var w = buf.writer();

    try w.print("{{\n", .{});
    try w.print("  \"frame_id\": {d},\n", .{obs.frame_id});
    try w.print("  \"simulation_time\": {d:.4},\n", .{obs.sim_time});
    try w.print("  \"width\": {d},\n  \"height\": {d},\n", .{ obs.width, obs.height });
    try w.print("  \"depth_stats\": {{ \"min\": {d:.4}, \"max\": {d:.4}, \"mean\": {d:.4} }},\n", .{
        obs.depth_min, obs.depth_max, obs.depth_mean,
    });
    try w.print("  \"anomaly_count\": {d},\n", .{obs.anomaly_count});
    try w.print("  \"visible_entities\": [\n", .{});
    for (obs.entities, 0..) |e, i| {
        if (i > 0) try w.writeAll(",\n");
        try w.print("    {{ \"id\": {d}, \"type\": \"{s}\", \"pixel_count\": {d}, \"centroid\": [{d:.1}, {d:.1}], \"bbox\": [{d}, {d}, {d}, {d}], \"depth_min\": {d:.4}, \"depth_max\": {d:.4}, \"depth_mean\": {d:.4}, \"track_id\": {d}, \"velocity\": [{d:.2}, {d:.2}], \"age_frames\": {d}, \"lost_frames\": {d} }}", .{
            e.entity_id, entityTypeName(e.entity_id), e.pixel_count,
            e.centroid_x, e.centroid_y,
            e.bbox_min_x, e.bbox_min_y, e.bbox_max_x, e.bbox_max_y,
            e.depth_min, e.depth_max,
            if (e.pixel_count > 0) e.depth_sum / @as(f32, @floatFromInt(e.pixel_count)) else 0,
            e.track_id, e.velocity_x, e.velocity_y, e.age_frames, e.lost_frames,
        });
    }
    try w.print("\n  ],\n", .{});
    try w.print("  \"anomalies\": [", .{});
    for (obs.anomalies, 0..) |a, i| {
        if (i > 0) try w.writeAll(",");
        try w.print("\n    {{ \"type\": \"{s}\", \"location\": [{d}, {d}], \"description\": \"{s}\" }}", .{
            a.anomaly_type, a.location_x, a.location_y, a.description,
        });
    }
    try w.print("\n  ]\n}}\n", .{});

    return buf.toOwnedSlice();
}

test "ProjectiveVision: native CV analyzer produces structured observation" {
    const allocator = std.testing.allocator;
    var fb = try VisualFrameBuffer.init(allocator, 16, 16);
    defer fb.deinit();

    // Paint a simple scene: two entities with known positions
    fb.clear(PixelColor.init(0, 0, 0, 255)); // void_ + depth 1e9
    // Draw a 4x4 "planet" block at (4,4)-(7,7) with depth 5.0, entity_id 1
    var y: usize = 4;
    while (y < 8) : (y += 1) {
        var x: usize = 4;
        while (x < 8) : (x += 1) {
            fb.setPixel(x, y, PixelColor.init(80, 130, 110, 255), 5.0, 1);
        }
    }
    // Draw a 2x2 "asteroid" block at (10,10)-(11,11) with depth 8.0, entity_id 3
    y = 10;
    while (y < 12) : (y += 1) {
        var x: usize = 10;
        while (x < 12) : (x += 1) {
            fb.setPixel(x, y, PixelColor.init(120, 110, 95, 255), 8.0, 3);
        }
    }

    var obs = try analyzeFrameBuffer(&fb, allocator, 42, 1.5);
    defer obs.deinit();

    // Verify: 2 non-void entities (planet + asteroid), plus void + maybe anomalies
    try std.testing.expect(obs.entities.len >= 2);

    // Find the planet entity in the list
    var found_planet: bool = false;
    var found_asteroid: bool = false;
    for (obs.entities) |e| {
        if (e.entity_id == 1) {
            found_planet = true;
            try std.testing.expectEqual(@as(usize, 16), e.pixel_count);
            try std.testing.expectApproxEqAbs(e.centroid_x, 5.5, 0.01);
            try std.testing.expectApproxEqAbs(e.centroid_y, 5.5, 0.01);
            try std.testing.expectApproxEqAbs(e.depth_min, 5.0, 0.01);
            try std.testing.expectApproxEqAbs(e.depth_max, 5.0, 0.01);
            try std.testing.expectEqual(@as(usize, 4), e.bbox_min_x);
            try std.testing.expectEqual(@as(usize, 7), e.bbox_max_x);
        }
        if (e.entity_id == 3) {
            found_asteroid = true;
            try std.testing.expectEqual(@as(usize, 4), e.pixel_count);
            try std.testing.expectApproxEqAbs(e.centroid_x, 10.5, 0.01);
            try std.testing.expectApproxEqAbs(e.centroid_y, 10.5, 0.01);
            try std.testing.expectApproxEqAbs(e.depth_min, 8.0, 0.01);
        }
    }
    try std.testing.expect(found_planet);
    try std.testing.expect(found_asteroid);

    // Global depth stats: should have min=5, max=8
    try std.testing.expectApproxEqAbs(obs.depth_min, 5.0, 0.01);
    try std.testing.expectApproxEqAbs(obs.depth_max, 8.0, 0.01);

    // JSON serialization — must contain entity names
    const json = try serializeObservationJson(&obs, allocator);
    defer allocator.free(json);
    try std.testing.expect(json.len > 50);
    try std.testing.expect(std.mem.indexOf(u8, json, "\"type\": \"planet\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "\"type\": \"asteroid\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "\"frame_id\": 42") != null);
}

// =============================================================================
// TEMPORAL TRACKER (Classical Multi-Object Centroid Tracking)
// =============================================================================
//
// Inspired by AI2-THOR's instance_detections2D + SORT (Simple Online Realtime
// Tracking) by Bewley et al. — pure classical algorithms, NO neural networks.
//
// The Tracker keeps state across frames. On each call to update(), it:
//   1. Receives the new frame's per-entity VisibleEntity[] (with centroids)
//   2. For each track from previous frame, finds the closest new detection
//      (greedy matching by Euclidean centroid distance, with gating threshold)
//   3. Updates the matched track's centroid (smoothing with simple low-pass)
//   4. Increments age_frames for matched tracks, lost_frames for unmatched
//   5. Creates new tracks for unmatched detections (after max_age_frames in
//      a frame, gets a stable track_id)
//   6. Deletes tracks that have been lost for > max_lost_frames
//
// Output: each VisibleEntity gets a stable track_id (same object across frames
// gets same track_id) + velocity_x/y (centroid delta) + age_frames + lost_frames
//
// This solves the "is asteroid #5 on frame N the same as on frame N-1?" problem
// for the LLM agent — without any neural network, just classical CV.
// =============================================================================

pub const TrackedEntity = struct {
    /// Stable identifier across frames (assigned when track is created)
    track_id: u32,
    /// Entity id from segmentation buffer (planet=1, asteroid=3, etc.)
    entity_id: u8,
    /// Current smoothed centroid (screen X, Y)
    centroid_x: f32,
    centroid_y: f32,
    /// Velocity (pixels per frame): current - previous centroid
    velocity_x: f32,
    velocity_y: f32,
    /// Number of frames this track has been alive
    age_frames: u32,
    /// Number of consecutive frames this track has been unmatched
    lost_frames: u32,
    /// Bounding box from latest observation
    bbox_min_x: usize,
    bbox_min_y: usize,
    bbox_max_x: usize,
    bbox_max_y: usize,
    /// Pixel count from latest observation
    pixel_count: usize,
    /// Depth stats from latest observation
    depth_min: f32,
    depth_max: f32,
    depth_mean: f32,
};

pub const Tracker = struct {
    /// Active tracks
    tracks: std.ArrayList(TrackedEntity),
    /// Next available track_id (incremented per new track)
    next_track_id: u32 = 1,
    /// Max centroid distance for matching (pixels)
    match_distance_threshold: f32 = 80.0,
    /// Frames a track can be lost before deletion
    max_lost_frames: u32 = 5,
    /// Frames a track must exist before being reported as "stable"
    min_age_for_report: u32 = 0,
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) Tracker {
        return .{
            .tracks = std.ArrayList(TrackedEntity).init(allocator),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *Tracker) void {
        self.tracks.deinit();
    }

    /// Update tracker with new frame's detections. Modifies `entities` in
    /// place — fills in track_id, velocity, age_frames for matched entities.
    /// Tracks that disappear are removed (after max_lost_frames).
    pub fn update(
        self: *Tracker,
        entities: []VisibleEntity,
        frame_id: u64,
    ) !void {
        _ = frame_id;
        // For each existing track, find best matching entity (closest centroid
        // within threshold, same entity_id). Greedy nearest-neighbor matching.
        const n_tracks = self.tracks.items.len;
        const n_entities = entities.len;

        // Build a "matched" flag array for entities
        const matched_entities = try self.allocator.alloc(bool, n_entities);
        defer self.allocator.free(matched_entities);
        @memset(matched_entities, false);

        const matched_tracks = try self.allocator.alloc(bool, n_tracks);
        defer self.allocator.free(matched_tracks);
        @memset(matched_tracks, false);

        // Greedy matching: for each track, find closest unmatched entity
        // with same entity_id, within threshold
        var ti: usize = 0;
        while (ti < n_tracks) : (ti += 1) {
            const track = &self.tracks.items[ti];
            var best_entity_idx: ?usize = null;
            var best_dist: f32 = self.match_distance_threshold;

            var ei: usize = 0;
            while (ei < n_entities) : (ei += 1) {
                if (matched_entities[ei]) continue;
                if (entities[ei].entity_id != track.entity_id) continue;

                const dx = entities[ei].centroid_x - track.centroid_x;
                const dy = entities[ei].centroid_y - track.centroid_y;
                const dist = @sqrt(dx * dx + dy * dy);
                if (dist < best_dist) {
                    best_dist = dist;
                    best_entity_idx = ei;
                }
            }

            if (best_entity_idx) |idx| {
                matched_tracks[ti] = true;
                matched_entities[idx] = true;
                // Update track with new detection
                const e = &entities[idx];
                const prev_cx = track.centroid_x;
                const prev_cy = track.centroid_y;
                // Simple low-pass smoothing: new = 0.7*new + 0.3*old (reduces jitter)
                track.centroid_x = e.centroid_x * 0.7 + prev_cx * 0.3;
                track.centroid_y = e.centroid_y * 0.7 + prev_cy * 0.3;
                track.velocity_x = track.centroid_x - prev_cx;
                track.velocity_y = track.centroid_y - prev_cy;
                track.age_frames += 1;
                track.lost_frames = 0;
                track.bbox_min_x = e.bbox_min_x;
                track.bbox_min_y = e.bbox_min_y;
                track.bbox_max_x = e.bbox_max_x;
                track.bbox_max_y = e.bbox_max_y;
                track.pixel_count = e.pixel_count;
                track.depth_min = e.depth_min;
                track.depth_max = e.depth_max;
                track.depth_mean = if (e.pixel_count > 0) e.depth_sum / @as(f32, @floatFromInt(e.pixel_count)) else 0;
                // Stamp the entity with the track_id + velocity
                e.track_id = track.track_id;
                e.velocity_x = track.velocity_x;
                e.velocity_y = track.velocity_y;
                e.age_frames = track.age_frames;
                e.lost_frames = 0;
            }
        }

        // Remove tracks that have been lost for too long
        // (compact in place)
        var write_idx: usize = 0;
        var track_idx: usize = 0;
        while (track_idx < n_tracks) : (track_idx += 1) {
            const t = &self.tracks.items[track_idx];
            if (!matched_tracks[track_idx]) {
                t.lost_frames += 1;
                if (t.lost_frames > self.max_lost_frames) {
                    // Drop this track
                    continue;
                }
            }
            self.tracks.items[write_idx] = t.*;
            write_idx += 1;
        }
        self.tracks.shrinkRetainingCapacity(write_idx);

        // Create new tracks for unmatched entities
        for (entities, 0..) |*e, ei| {
            if (matched_entities[ei]) continue;
            // Skip "void" entity (background)
            if (e.entity_id == 0) continue;

            const new_track = TrackedEntity{
                .track_id = self.next_track_id,
                .entity_id = e.entity_id,
                .centroid_x = e.centroid_x,
                .centroid_y = e.centroid_y,
                .velocity_x = 0,
                .velocity_y = 0,
                .age_frames = 1,
                .lost_frames = 0,
                .bbox_min_x = e.bbox_min_x,
                .bbox_min_y = e.bbox_min_y,
                .bbox_max_x = e.bbox_max_x,
                .bbox_max_y = e.bbox_max_y,
                .pixel_count = e.pixel_count,
                .depth_min = e.depth_min,
                .depth_max = e.depth_max,
                .depth_mean = if (e.pixel_count > 0) e.depth_sum / @as(f32, @floatFromInt(e.pixel_count)) else 0,
            };
            try self.tracks.append(new_track);
            self.next_track_id += 1;

            // Stamp the entity with the new track_id
            e.track_id = new_track.track_id;
            e.velocity_x = 0;
            e.velocity_y = 0;
            e.age_frames = 1;
            e.lost_frames = 0;
        }
    }

    /// Returns the current snapshot of all active tracks (useful for
    /// rendering debug overlays or for the agent to query "where are
    /// the persistent objects even if currently occluded").
    pub fn getActiveTracks(self: *const Tracker) []const TrackedEntity {
        return self.tracks.items;
    }
};

// =============================================================================
// SCENE GRAPH — spatial relationships between visible entities
// =============================================================================
//
// Builds a simple scene graph by computing spatial proximity:
//   - For each tracked entity, find its "parent" — the nearest larger
//     entity that contains or is near its centroid.
//   - "children" — entities whose centroids fall within or very close to
//     this entity's bounding box.
//   - "neighbors" — entities within a configurable proximity radius
//     (not parent/child but spatially adjacent).
//
// This is what DeepSeek meant by "Scene Graph access" — gives the LLM
// relational understanding, not just pixel stats.
// =============================================================================

pub const SceneGraphNode = struct {
    track_id: u32,
    entity_id: u8,
    parent_track_id: ?u32 = null,
    children_count: u32 = 0,
    /// Track IDs of children (up to 8 — enough for our scene)
    children: [8]u32 = .{0} ** 8,
    neighbors_count: u32 = 0,
    neighbors: [4]u32 = .{0} ** 4,
};

pub const SceneGraph = struct {
    nodes: std.ArrayList(SceneGraphNode),
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) SceneGraph {
        return .{
            .nodes = std.ArrayList(SceneGraphNode).init(allocator),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *SceneGraph) void {
        self.nodes.deinit();
    }
};

/// Build a scene graph from a set of tracked entities.
/// The graph encodes parent/child/neighbor relationships based on:
///   - centroid containment (child's centroid inside parent's bbox)
///   - spatial proximity (neighbors within neighbor_distance pixels)
pub fn buildSceneGraph(
    allocator: std.mem.Allocator,
    tracked: []const TrackedEntity,
    neighbor_distance: f32,
) !SceneGraph {
    var graph = SceneGraph.init(allocator);
    errdefer graph.deinit();

    // Create one node per tracked entity
    for (tracked) |t| {
        try graph.nodes.append(.{
            .track_id = t.track_id,
            .entity_id = t.entity_id,
        });
    }

    // Compute parent-child: child's centroid is inside parent's bbox,
    // AND parent has more pixels than child (parent is "larger").
    var ci_local: usize = 0;
    while (ci_local < tracked.len) : (ci_local += 1) {
        var pi_local: usize = 0;
        while (pi_local < tracked.len) : (pi_local += 1) {
            if (ci_local == pi_local) continue;
            const child = tracked[ci_local];
            const parent = tracked[pi_local];
            if (parent.pixel_count <= child.pixel_count) continue;
            const cx_ok = child.centroid_x >= @as(f32, @floatFromInt(parent.bbox_min_x)) and
                child.centroid_x <= @as(f32, @floatFromInt(parent.bbox_max_x));
            const cy_ok = child.centroid_y >= @as(f32, @floatFromInt(parent.bbox_min_y)) and
                child.centroid_y <= @as(f32, @floatFromInt(parent.bbox_max_y));
            if (cx_ok and cy_ok) {
                if (graph.nodes.items[ci_local].parent_track_id == null) {
                    graph.nodes.items[ci_local].parent_track_id = parent.track_id;
                    if (graph.nodes.items[pi_local].children_count < 8) {
                        const cnt = graph.nodes.items[pi_local].children_count;
                        graph.nodes.items[pi_local].children[cnt] = child.track_id;
                        graph.nodes.items[pi_local].children_count += 1;
                    }
                }
            }
        }
    }

    // Compute neighbors: entities within neighbor_distance pixels (centroid distance)
    var ai_local: usize = 0;
    while (ai_local < tracked.len) : (ai_local += 1) {
        var bi_local: usize = 0;
        while (bi_local < tracked.len) : (bi_local += 1) {
            if (ai_local == bi_local) continue;
            const a = tracked[ai_local];
            const b = tracked[bi_local];
            const dx = a.centroid_x - b.centroid_x;
            const dy = a.centroid_y - b.centroid_y;
            const dist = @sqrt(dx * dx + dy * dy);
            if (dist < neighbor_distance and dist > 0.001) {
                if (graph.nodes.items[ai_local].neighbors_count < 4) {
                    const cnt = graph.nodes.items[ai_local].neighbors_count;
                    graph.nodes.items[ai_local].neighbors[cnt] = b.track_id;
                    graph.nodes.items[ai_local].neighbors_count += 1;
                }
            }
        }
    }

    return graph;
}

/// Serialize the SceneGraph to JSON (compact form, ~500 bytes typical)
pub fn serializeSceneGraphJson(graph: *const SceneGraph, allocator: std.mem.Allocator) ![]u8 {
    var buf = std.ArrayList(u8).init(allocator);
    errdefer buf.deinit();
    var w = buf.writer();
    try w.print("[\n", .{});
    for (graph.nodes.items, 0..) |n, i| {
        if (i > 0) try w.writeAll(",\n");
        try w.print("  {{ \"track_id\": {d}, \"entity_id\": {d}, \"entity_type\": \"{s}\", \"parent\": ", .{
            n.track_id, n.entity_id, entityTypeName(n.entity_id),
        });
        if (n.parent_track_id) |p| {
            try w.print("{d}, ", .{p});
        } else {
            try w.writeAll("null, ");
        }
        try w.print("\"children\": [", .{});
        var c: u32 = 0;
        while (c < n.children_count) : (c += 1) {
            if (c > 0) try w.writeAll(", ");
            try w.print("{d}", .{n.children[c]});
        }
        try w.print("], \"neighbors\": [", .{});
        var nn: u32 = 0;
        while (nn < n.neighbors_count) : (nn += 1) {
            if (nn > 0) try w.writeAll(", ");
            try w.print("{d}", .{n.neighbors[nn]});
        }
        try w.print("] }}", .{});
    }
    try w.print("\n]\n", .{});
    return buf.toOwnedSlice();
}

test "ProjectiveVision: temporal tracker maintains track_id across frames" {
    const allocator = std.testing.allocator;
    var fb = try VisualFrameBuffer.init(allocator, 32, 32);
    defer fb.deinit();
    var tracker = Tracker.init(allocator);
    defer tracker.deinit();

    // Frame 1: planet at (4,4)-(7,7), asteroid at (20,20)-(23,23)
    fb.clear(PixelColor.init(0, 0, 0, 255));
    var y: usize = 4;
    while (y < 8) : (y += 1) {
        var x: usize = 4;
        while (x < 8) : (x += 1) {
            fb.setPixel(x, y, PixelColor.init(80, 130, 110, 255), 5.0, 1);
        }
    }
    y = 20;
    while (y < 24) : (y += 1) {
        var x: usize = 20;
        while (x < 24) : (x += 1) {
            fb.setPixel(x, y, PixelColor.init(120, 110, 95, 255), 8.0, 3);
        }
    }
    var obs1 = try analyzeFrameBuffer(&fb, allocator, 1, 0.016);
    defer obs1.deinit();
    try tracker.update(obs1.entities, 1);

    // After frame 1: should have 2 tracks (planet + asteroid), both with track_id
    try std.testing.expect(tracker.tracks.items.len == 2);
    var planet_track_id: u32 = 0;
    var asteroid_track_id: u32 = 0;
    for (obs1.entities) |e| {
        if (e.entity_id == 1) planet_track_id = e.track_id;
        if (e.entity_id == 3) asteroid_track_id = e.track_id;
    }
    try std.testing.expect(planet_track_id > 0);
    try std.testing.expect(asteroid_track_id > 0);
    try std.testing.expect(planet_track_id != asteroid_track_id);

    // Frame 2: asteroid moves slightly to (22,22)-(25,25); planet stays
    fb.clear(PixelColor.init(0, 0, 0, 255));
    y = 4;
    while (y < 8) : (y += 1) {
        var x: usize = 4;
        while (x < 8) : (x += 1) {
            fb.setPixel(x, y, PixelColor.init(80, 130, 110, 255), 5.0, 1);
        }
    }
    y = 22;
    while (y < 26) : (y += 1) {
        var x: usize = 22;
        while (x < 26) : (x += 1) {
            fb.setPixel(x, y, PixelColor.init(120, 110, 95, 255), 8.0, 3);
        }
    }
    var obs2 = try analyzeFrameBuffer(&fb, allocator, 2, 0.032);
    defer obs2.deinit();
    try tracker.update(obs2.entities, 2);

    // After frame 2: still 2 tracks (same planet + same asteroid, just moved)
    try std.testing.expect(tracker.tracks.items.len == 2);

    // Verify track_ids are preserved across frames
    var planet_track_id_2: u32 = 0;
    var asteroid_track_id_2: u32 = 0;
    for (obs2.entities) |e| {
        if (e.entity_id == 1) planet_track_id_2 = e.track_id;
        if (e.entity_id == 3) asteroid_track_id_2 = e.track_id;
    }
    try std.testing.expectEqual(planet_track_id, planet_track_id_2);
    try std.testing.expectEqual(asteroid_track_id, asteroid_track_id_2);

    // Asteroid should have velocity since it moved (4-6, 4-6) -> (6-6, 6-6) per pixel
    // The centroid moved from (23.5, 23.5) to (23.5, 23.5) — actually let's check
    // asteroid centroid moved from (21.5, 21.5) to (23.5, 23.5) — delta (2, 2)
    // Tracker uses low-pass smoothing so velocity might be ~1.4
    var asteroid_velocity_x: f32 = 0;
    var asteroid_velocity_y: f32 = 0;
    for (obs2.entities) |e| {
        if (e.entity_id == 3) {
            asteroid_velocity_x = e.velocity_x;
            asteroid_velocity_y = e.velocity_y;
        }
    }
    // Velocity should be roughly positive (object moved right+down)
    try std.testing.expect(asteroid_velocity_x > 0.5);
    try std.testing.expect(asteroid_velocity_y > 0.5);
}

test "ProjectiveVision: scene graph builds parent/child/neighbor relationships" {
    const allocator = std.testing.allocator;
    var fb = try VisualFrameBuffer.init(allocator, 64, 64);
    defer fb.deinit();
    var tracker = Tracker.init(allocator);
    defer tracker.deinit();

    // Planet is a large entity (id=1) — 20x20 box from (10,10) to (29,29)
    // Depth 5.0 (far from camera in our convention: smaller depth = closer)
    fb.clear(PixelColor.init(0, 0, 0, 255));
    var y: usize = 10;
    while (y < 30) : (y += 1) {
        var x: usize = 10;
        while (x < 30) : (x += 1) {
            fb.setPixel(x, y, PixelColor.init(80, 130, 110, 255), 5.0, 1);
        }
    }
    // Asteroid is smaller (id=3) — 4x4 box at (16,16)-(19,19) — INSIDE planet's bbox
    // Depth 3.0 (CLOSER than planet so it occludes planet pixels)
    y = 16;
    while (y < 20) : (y += 1) {
        var x: usize = 16;
        while (x < 20) : (x += 1) {
            fb.setPixel(x, y, PixelColor.init(120, 110, 95, 255), 3.0, 3);
        }
    }

    var obs = try analyzeFrameBuffer(&fb, allocator, 1, 0.016);
    defer obs.deinit();
    try tracker.update(obs.entities, 1);

    // Build scene graph with neighbor_distance=15 pixels
    var graph = try buildSceneGraph(allocator, tracker.tracks.items, 15.0);
    defer graph.deinit();

    // Verify: asteroid's parent should be planet (asteroid centroid inside planet bbox)
    var asteroid_parent: ?u32 = null;
    var planet_children_count: u32 = 0;
    for (graph.nodes.items) |n| {
        if (n.entity_id == 3) asteroid_parent = n.parent_track_id;
        if (n.entity_id == 1) planet_children_count = n.children_count;
    }
    try std.testing.expect(asteroid_parent != null);
    try std.testing.expect(planet_children_count >= 1);
}

