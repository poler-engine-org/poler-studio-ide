// =============================================================================
// P³ ENGINE — WAVEFRONT OBJ PARSER (Blender integration)
// =============================================================================
//
// Minimal .obj parser for importing geometry from Blender (or any tool that
// exports Wavefront OBJ). Supports:
//   - v  (vertex position)
//   - vn (vertex normal)
//   - f  (face — triangle or quad, with v/n/vt indices)
//
// Does NOT support: materials (.mtl), textures, NURBS, smoothing groups,
// animation. This is the minimal subset needed for static mesh import.
//
// Usage:
//   var mesh = try parseObjFile(allocator, "/path/to/model.obj");
//   defer mesh.deinit();
//   // mesh.vertices.items: []MeshVertex (pos + normal + color)
//   // mesh.indices.items: []u16
//
// Blender integration path:
//   1. In Blender: File > Export > Wavefront (.obj)
//   2. Settings: Apply Modifiers=ON, Normals=ON, Triangulate=ON
//   3. Use this parser to load the mesh into P³ Engine
//
// This shows that the engine supports BOTH procedural generation (Catmull-Rom
// splines, box primitives, sphere generators) AND external mesh import
// (Blender .obj). The user gets to choose: code-the-geometry or import-it.
// =============================================================================

const std = @import("std");

// Local Vec3 to avoid the multi-module file conflict that occurs when
// p3_obj_loader.zig (an engine module) does @import("root.zig") AND the
// consuming binary also adds root.zig as a named module.
// This Vec3 is layout-compatible with p3.Vec3 (same fields, same offsets).
pub const Vec3 = struct {
    x: f32,
    y: f32,
    z: f32,
    pub inline fn init(x: f32, y: f32, z: f32) Vec3 { return .{ .x = x, .y = y, .z = z }; }
    pub inline fn zero() Vec3 { return .{ .x = 0, .y = 0, .z = 0 }; }
};

pub const MeshVertex = struct {
    pos: Vec3,
    normal: Vec3,
    color: [4]u8,
};

pub const ObjMesh = struct {
    vertices: std.ArrayList(MeshVertex),
    indices: std.ArrayList(u16),
    allocator: std.mem.Allocator,
    vertex_count_orig: usize = 0,
    face_count: usize = 0,

    pub fn init(allocator: std.mem.Allocator) ObjMesh {
        return .{
            .vertices = std.ArrayList(MeshVertex).init(allocator),
            .indices = std.ArrayList(u16).init(allocator),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *ObjMesh) void {
        self.vertices.deinit();
        self.indices.deinit();
    }
};

/// Parses a Wavefront .obj file. Returns ObjMesh with vertices + indices.
/// Default color is white (user can recolor after import).
pub fn parseObjFile(allocator: std.mem.Allocator, file_path: []const u8) !ObjMesh {
    var file = try std.fs.cwd().openFile(file_path, .{});
    defer file.close();
    const content = try file.readToEndAlloc(allocator, 50 * 1024 * 1024); // 50 MB max
    defer allocator.free(content);
    return try parseObjSlice(allocator, content);
}

/// Parses a .obj from an in-memory byte slice.
pub fn parseObjSlice(allocator: std.mem.Allocator, content: []const u8) !ObjMesh {
    var mesh = ObjMesh.init(allocator);
    errdefer mesh.deinit();

    var positions = std.ArrayList(Vec3).init(allocator);
    defer positions.deinit();
    var normals = std.ArrayList(Vec3).init(allocator);
    defer normals.deinit();

    var line_iter = std.mem.splitScalar(u8, content, '\n');
    while (line_iter.next()) |line_raw| {
        // Strip trailing \r and leading/trailing whitespace
        const line = std.mem.trim(u8, line_raw, " \r\t");
        if (line.len == 0) continue;
        if (line[0] == '#') continue; // comment

        // Tokenize by spaces
        var tokens = std.mem.tokenizeAny(u8, line, " \t");
        const cmd = tokens.next() orelse continue;

        if (std.mem.eql(u8, cmd, "v")) {
            // Vertex position: v x y z [w]
            const x_str = tokens.next() orelse continue;
            const y_str = tokens.next() orelse continue;
            const z_str = tokens.next() orelse continue;
            const x = std.fmt.parseFloat(f32, x_str) catch continue;
            const y = std.fmt.parseFloat(f32, y_str) catch continue;
            const z = std.fmt.parseFloat(f32, z_str) catch continue;
            try positions.append(Vec3.init(x, y, z));
            mesh.vertex_count_orig += 1;
        } else if (std.mem.eql(u8, cmd, "vn")) {
            // Vertex normal: vn x y z
            const x_str = tokens.next() orelse continue;
            const y_str = tokens.next() orelse continue;
            const z_str = tokens.next() orelse continue;
            const x = std.fmt.parseFloat(f32, x_str) catch continue;
            const y = std.fmt.parseFloat(f32, y_str) catch continue;
            const z = std.fmt.parseFloat(f32, z_str) catch continue;
            try normals.append(Vec3.init(x, y, z));
        } else if (std.mem.eql(u8, cmd, "f")) {
            // Face: f v1/vt1/vn1 v2/vt2/vn2 v3/vt3/vn3 [v4/vt4/vn4]
            // Triangulate: if 4 vertices, split into 2 triangles (0,1,2) and (0,2,3)
            var face_verts = std.ArrayList(struct { v_idx: usize, n_idx: ?usize }).init(allocator);
            defer face_verts.deinit();
            while (tokens.next()) |tok| {
                var part_iter = std.mem.splitScalar(u8, tok, '/');
                const v_str = part_iter.next() orelse continue;
                _ = part_iter.next(); // skip vt (texture coord)
                const n_str = part_iter.next(); // optional vn
                const v_idx = std.fmt.parseInt(usize, v_str, 10) catch continue;
                const n_idx = if (n_str) |s| std.fmt.parseInt(usize, s, 10) catch null else null;
                try face_verts.append(.{
                    .v_idx = if (v_idx > 0) v_idx - 1 else 0, // .obj is 1-indexed
                    .n_idx = if (n_idx) |n| (if (n > 0) n - 1 else 0) else null,
                });
            }

            if (face_verts.items.len < 3) continue;
            // Triangulate as fan: (0, 1, 2), (0, 2, 3), (0, 3, 4), ...
            var k: usize = 1;
            while (k + 1 < face_verts.items.len) : (k += 1) {
                const v0 = face_verts.items[0];
                const v1 = face_verts.items[k];
                const v2 = face_verts.items[k + 1];
                // Get positions
                if (v0.v_idx >= positions.items.len) continue;
                if (v1.v_idx >= positions.items.len) continue;
                if (v2.v_idx >= positions.items.len) continue;
                const p0 = positions.items[v0.v_idx];
                const p1 = positions.items[v1.v_idx];
                const p2 = positions.items[v2.v_idx];
                // Get normals (or compute flat normal)
                const n0 = if (v0.n_idx) |idx| (if (idx < normals.items.len) normals.items[idx] else Vec3.zero()) else Vec3.zero();
                const n1 = if (v1.n_idx) |idx| (if (idx < normals.items.len) normals.items[idx] else Vec3.zero()) else Vec3.zero();
                const n2 = if (v2.n_idx) |idx| (if (idx < normals.items.len) normals.items[idx] else Vec3.zero()) else Vec3.zero();
                // Append 3 vertices + 3 indices
                const base: u16 = @intCast(mesh.vertices.items.len);
                try mesh.vertices.append(.{ .pos = p0, .normal = n0, .color = .{ 200, 200, 200, 255 } });
                try mesh.vertices.append(.{ .pos = p1, .normal = n1, .color = .{ 200, 200, 200, 255 } });
                try mesh.vertices.append(.{ .pos = p2, .normal = n2, .color = .{ 200, 200, 200, 255 } });
                try mesh.indices.append(base);
                try mesh.indices.append(base + 1);
                try mesh.indices.append(base + 2);
                mesh.face_count += 1;
            }
        }
        // Ignore: vt, mtllib, usemtl, o, g, s — minimal parser
    }

    return mesh;
}

test "OBJ parser: parses minimal triangle" {
    const allocator = std.testing.allocator;
    const content =
        \\# simple triangle
        \\v 0 0 0
        \\v 1 0 0
        \\v 0 1 0
        \\vn 0 0 1
        \\vn 0 0 1
        \\vn 0 0 1
        \\f 1//1 2//2 3//3
    ;
    var mesh = try parseObjSlice(allocator, content);
    defer mesh.deinit();
    try std.testing.expectEqual(@as(usize, 3), mesh.vertex_count_orig);
    try std.testing.expectEqual(@as(usize, 1), mesh.face_count);
    try std.testing.expectEqual(@as(usize, 3), mesh.vertices.items.len);
    try std.testing.expectEqual(@as(usize, 3), mesh.indices.items.len);
    try std.testing.expectApproxEqAbs(mesh.vertices.items[0].pos.x, 0.0, 1e-4);
    try std.testing.expectApproxEqAbs(mesh.vertices.items[1].pos.x, 1.0, 1e-4);
    try std.testing.expectApproxEqAbs(mesh.vertices.items[2].pos.y, 1.0, 1e-4);
}

test "OBJ parser: triangulates quad into 2 triangles" {
    const allocator = std.testing.allocator;
    const content =
        \\v 0 0 0
        \\v 1 0 0
        \\v 1 1 0
        \\v 0 1 0
        \\f 1 2 3 4
    ;
    var mesh = try parseObjSlice(allocator, content);
    defer mesh.deinit();
    try std.testing.expectEqual(@as(usize, 4), mesh.vertex_count_orig);
    try std.testing.expectEqual(@as(usize, 2), mesh.face_count);
    try std.testing.expectEqual(@as(usize, 6), mesh.vertices.items.len);
    try std.testing.expectEqual(@as(usize, 6), mesh.indices.items.len);
}
