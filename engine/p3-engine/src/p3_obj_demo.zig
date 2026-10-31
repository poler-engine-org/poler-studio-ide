// =============================================================================
// P³ ENGINE — OBJ LOADER DEMO
// =============================================================================
// Loads a .obj file (e.g. exported from Blender), rasterizes it through our
// VisualFrameBuffer, saves PNG. Demonstrates "Blender integration" — the
// engine supports both procedural geometry AND external mesh import.
//
// Usage:
//   ./zig-out/bin/p3-obj-demo   # loads /home/z/renders/pyramid.obj by default
//   P3_OBJ_PATH=/path/to/mesh.obj ./zig-out/bin/p3-obj-demo
//
// Try this Blender workflow:
//   1. In Blender: File > Export > Wavefront (.obj)
//   2. Settings: Apply Modifiers=ON, Normals=ON, Triangulate=ON
//   3. P3_OBJ_PATH=/path/to/exported.obj ./p3-obj-demo
// =============================================================================

const std = @import("std");
const math = std.math;
const p3 = @import("root.zig");
const obj_loader = @import("p3_obj_loader");

const Vec3 = p3.Vec3;
const Vec4 = p3.Vec4;
const Mat4x4 = p3.Mat4x4;

const vision = p3.p3.vision;
const VisualFrameBuffer = vision.VisualFrameBuffer;
const ProjectiveRasterizer = vision.ProjectiveRasterizer;
const ProjectedVertex = vision.ProjectedVertex;
const PixelColor = vision.PixelColor;

const RENDER_WIDTH: usize = 640;
const RENDER_HEIGHT: usize = 480;

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();
    const stdout = std.io.getStdOut().writer();

    try stdout.print("=======================================================\n", .{});
    try stdout.print("P3 ENGINE — OBJ LOADER DEMO (Blender integration)\n", .{});
    try stdout.print("Loads .obj file, rasterizes through VisualFrameBuffer\n", .{});
    try stdout.print("=======================================================\n", .{});

    // Load OBJ file path from env var or use default
    const obj_path = std.process.getEnvVarOwned(allocator, "P3_OBJ_PATH") catch try allocator.dupe(u8, "/home/z/renders/pyramid.obj");
    defer allocator.free(obj_path);

    try stdout.print("Loading OBJ: {s}\n", .{obj_path});
    var mesh = obj_loader.parseObjFile(allocator, obj_path) catch |err| {
        try stdout.print("ERROR loading OBJ: {any}\n", .{err});
        return;
    };
    defer mesh.deinit();
    try stdout.print("Loaded: {d} original vertices, {d} triangles, {d} mesh vertices\n", .{
        mesh.vertex_count_orig, mesh.face_count, mesh.vertices.items.len,
    });

    // --- Init framebuffer ---
    var fb = try VisualFrameBuffer.init(allocator, RENDER_WIDTH, RENDER_HEIGHT);
    defer fb.deinit();

    // --- Camera (centered on mesh, looking from above-front) ---
    const eye = Vec3.init(3.5, 3.5, -4.5);
    const target = Vec3.init(0.5, 0.3, 0.5);
    const up = Vec3.init(0, 1, 0);
    const view = Mat4x4.createLookAt(eye, target, up);
    const aspect: f32 = @as(f32, @floatFromInt(RENDER_WIDTH)) / @as(f32, @floatFromInt(RENDER_HEIGHT));
    const projection = Mat4x4.createProjectionFov(50.0 * math.pi / 180.0, aspect, 0.05, 100.0);
    const vp = Mat4x4.mul(projection, view);

    // --- Clear + background ---
    var y: usize = 0;
    while (y < fb.height) : (y += 1) {
        const t = @as(f32, @floatFromInt(y)) / @as(f32, @floatFromInt(fb.height));
        const r: u8 = @intFromFloat(20.0 + (1.0 - t) * 8.0);
        const g: u8 = @intFromFloat(25.0 + (1.0 - t) * 8.0);
        const b: u8 = @intFromFloat(40.0 + (1.0 - t) * 12.0);
        var x: usize = 0;
        while (x < fb.width) : (x += 1) {
            const idx = y * fb.width + x;
            fb.color_buffer[idx] = PixelColor.init(r, g, b, 255);
            fb.depth_buffer[idx] = 1e9;
            fb.segmentation_buffer[idx] = 0;
        }
    }

    // --- Rasterize mesh with Lambertian lighting ---
    var rasterizer = ProjectiveRasterizer.init(&fb);
    const light_dir = Vec3.init(0.6, 0.8, 0.4);
    const model = Mat4x4.identity();

    var projected_buf: [16384]?ProjectedVertex = undefined;
    if (mesh.vertices.items.len > projected_buf.len) {
        try stdout.print("Mesh too large (>{d} vertices)\n", .{projected_buf.len});
        return;
    }
    for (mesh.vertices.items, 0..) |v, i| {
        // Convert obj_loader.Vec3 -> p3.Vec3 (layout-compatible but Zig treats them
        // as distinct types — manual field copy is the safe path)
        const p3_pos = Vec3.init(v.pos.x, v.pos.y, v.pos.z);
        const p3_normal = Vec3.init(v.normal.x, v.normal.y, v.normal.z);
        const world_pos = Mat4x4.transformPoint(model, p3_pos);
        const world_normal = Mat4x4.transformVector(model, p3_normal).normalize();
        var intensity = world_normal.dot(light_dir.normalize());
        if (intensity < 0.0) intensity = 0.0;
        const ambient: f32 = 0.25;
        const light = ambient + (1.0 - ambient) * intensity;
        const tinted = PixelColor.init(
            @intFromFloat(@min(255.0, @as(f32, @floatFromInt(v.color[0])) * light * 1.4)),
            @intFromFloat(@min(255.0, @as(f32, @floatFromInt(v.color[1])) * light * 1.0)),
            @intFromFloat(@min(255.0, @as(f32, @floatFromInt(v.color[2])) * light * 0.7)),
            v.color[3],
        );
        projected_buf[i] = projectVertex(world_pos, vp, tinted, 1);
    }

    var i: usize = 0;
    while (i + 2 < mesh.indices.items.len) : (i += 3) {
        const v0 = projected_buf[mesh.indices.items[i]] orelse continue;
        const v1 = projected_buf[mesh.indices.items[i + 1]] orelse continue;
        const v2 = projected_buf[mesh.indices.items[i + 2]] orelse continue;
        rasterizer.rasterizeTriangle(v0, v1, v2);
    }

    // --- Save PPM ---
    const out_path = "/home/z/renders/obj_demo.ppm";
    var f = try std.fs.cwd().createFile(out_path, .{});
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
    try stdout.print("\nSaved: {s}\n", .{out_path});

    // --- Run CV analyzer on the loaded mesh ---
    var obs = try vision.analyzeFrameBuffer(&fb, allocator, 1, 0.0);
    defer obs.deinit();
    try stdout.print("\nCV analysis of rendered mesh:\n", .{});
    try stdout.print("  visible entities: {d}\n", .{obs.entities.len});
    for (obs.entities) |e| {
        try stdout.print("    id={} type={s} pixels={d} centroid=({d:.0},{d:.0}) depth_min={d:.3}\n", .{
            e.entity_id, vision.entityTypeName(e.entity_id), e.pixel_count,
            e.centroid_x, e.centroid_y, e.depth_min,
        });
    }
    try stdout.print("  anomalies: {d}\n", .{obs.anomaly_count});
}

// Local projectVertex (mirrors the one in p3_race_bench.zig but inlined here)
fn projectVertex(world_pos: Vec3, vp: Mat4x4, base_color: PixelColor, entity_id: u8) ?ProjectedVertex {
    const clip = Mat4x4.mulVec(vp, Vec4.fromVec3Affine(world_pos));
    if (clip.w <= 0.0001) return null;
    const inv_w = 1.0 / clip.w;
    const ndc_x = clip.x * inv_w;
    const ndc_y = clip.y * inv_w;
    const ndc_z = clip.z * inv_w;
    if (ndc_x < -1.05 or ndc_x > 1.05) return null;
    if (ndc_y < -1.05 or ndc_y > 1.05) return null;
    if (ndc_z < -1.05 or ndc_z > 1.05) return null;
    const half_w = @as(f32, @floatFromInt(RENDER_WIDTH)) * 0.5;
    const half_h = @as(f32, @floatFromInt(RENDER_HEIGHT)) * 0.5;
    return ProjectedVertex{
        .screen_x = half_w + ndc_x * half_w,
        .screen_y = half_h - ndc_y * half_h,
        .inv_w = inv_w,
        .depth = ndc_z,
        .color = base_color,
        .entity_id = entity_id,
    };
}
