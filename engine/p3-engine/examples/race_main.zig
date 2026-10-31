// =============================================================================
// P³ NEON VECTOR: CYBERPUNK ORBITAL GRAND PRIX v1.0 — ZIG & RAYLIB
// =============================================================================

const std = @import("std");
const math = std.math;
const p3 = @import("root.zig");

const rl = @cImport({
    @cInclude("raylib.h");
    @cInclude("raymath.h");
});

const Vec3 = p3.Vec3;
const HomVec4 = p3.HomVec4;

const NUM_TRACK_WAYPOINTS = 160;
const NUM_BUILDINGS = 45;
const NUM_BOOST_GATES = 12;

pub const TrackNode = struct {
    pos: Vec3,
    fwd: Vec3,
    normal: Vec3,
    right: Vec3,
    bank_angle: f32,
    has_gate: bool,
};

pub const Building = struct {
    pos: Vec3,
    width: f32,
    height: f32,
    depth: f32,
    color: rl.Color,
    neon_color: rl.Color,
};

pub fn main() !void {
    const screen_width: c_int = 1280;
    const screen_height: c_int = 720;

    rl.SetConfigFlags(rl.FLAG_WINDOW_RESIZABLE | rl.FLAG_MSAA_4X_HINT);
    rl.InitWindow(screen_width, screen_height, "P³ Neon Vector — Cyberpunk Orbital Grand Prix");
    defer rl.CloseWindow();

    rl.SetTargetFPS(60);

    // Cyrillic & Unicode Glyph Setup
    var codepoints: [512]c_int = undefined;
    var cp_count: usize = 0;
    var c: u32 = 0x0020;
    while (c <= 0x007E) : (c += 1) { codepoints[cp_count] = @intCast(c); cp_count += 1; }
    c = 0x0400;
    while (c < 0x0500) : (c += 1) { codepoints[cp_count] = @intCast(c); cp_count += 1; }
    codepoints[cp_count] = 0x00B0; cp_count += 1;
    codepoints[cp_count] = 0x00B3; cp_count += 1;
    codepoints[cp_count] = 0x03C0; cp_count += 1;
    codepoints[cp_count] = 0x03A9; cp_count += 1;
    codepoints[cp_count] = 0x221E; cp_count += 1;

    const font = rl.LoadFontEx("/usr/share/fonts/TTF/DejaVuSans.ttf", 22, &codepoints, @intCast(cp_count));
    rl.SetTextureFilter(font.texture, rl.TEXTURE_FILTER_BILINEAR);
    defer rl.UnloadFont(font);

    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    var bolide_mesh = try p3.generateCyberBolideMesh(allocator, .{});
    defer bolide_mesh.deinit();

    // 1. Generate Procedural Toroidal Closed Circuit
    var track_nodes: [NUM_TRACK_WAYPOINTS]TrackNode = undefined;
    const major_r: f32 = 65.0;
    const minor_r: f32 = 25.0;

    for (&track_nodes, 0..) |*node, i| {
        const u: f32 = (@as(f32, @floatFromInt(i)) / @as(f32, @floatFromInt(NUM_TRACK_WAYPOINTS))) * 2.0 * math.pi;
        const curve_x = (major_r + minor_r * @cos(3.0 * u)) * @cos(u);
        const curve_z = (major_r + minor_r * @cos(3.0 * u)) * @sin(u);
        const curve_y = 12.0 * @sin(2.0 * u) + 4.0 * @sin(5.0 * u);

        const du = 0.01;
        const next_u = u + du;
        const nx_x = (major_r + minor_r * @cos(3.0 * next_u)) * @cos(next_u);
        const nx_z = (major_r + minor_r * @cos(3.0 * next_u)) * @sin(next_u);
        const nx_y = 12.0 * @sin(2.0 * next_u) + 4.0 * @sin(5.0 * next_u);

        const fwd = Vec3.init(nx_x - curve_x, nx_y - curve_y, nx_z - curve_z).normalize();
        const up = Vec3.init(0, 1, 0);
        const right = fwd.cross(up).normalize();
        const normal = right.cross(fwd).normalize();

        node.pos = Vec3.init(curve_x, curve_y, curve_z);
        node.fwd = fwd;
        node.right = right;
        node.normal = normal;
        node.bank_angle = @sin(4.0 * u) * 0.35;
        node.has_gate = (i % (NUM_TRACK_WAYPOINTS / NUM_BOOST_GATES) == 0);
    }

    // 2. Generate Procedural Cyberpunk Skyscrapers
    var buildings: [NUM_BUILDINGS]Building = undefined;
    var prng = std.Random.DefaultPrng.init(1337);
    const rand = prng.random();

    for (&buildings) |*b| {
        const theta = rand.float(f32) * 2.0 * math.pi;
        const dist = major_r * 0.4 + rand.float(f32) * major_r * 0.9;
        const h = 25.0 + rand.float(f32) * 75.0;
        b.pos = Vec3.init(dist * @cos(theta), h * 0.5 - 15.0, dist * @sin(theta));
        b.width = 12.0 + rand.float(f32) * 18.0;
        b.depth = 12.0 + rand.float(f32) * 18.0;
        b.height = h;
        b.color = .{ .r = 15, .g = 20, .b = 32, .a = 255 };
        b.neon_color = if (rand.boolean()) .{ .r = 0, .g = 240, .b = 255, .a = 255 } else .{ .r = 255, .g = 0, .b = 130, .a = 255 };
    }

    // 3. Bolide Physics & Race State
    var track_t: f32 = 0.0;
    var bolide_speed: f32 = 0.0;
    var lateral_offset: f32 = 0.0;
    var nitro_energy: f32 = 100.0;
    var current_lap_time: f32 = 0.0;
    var best_lap_time: f32 = 0.0;
    var current_lap: u32 = 1;
    var cam_eye = Vec3.init(0, 10, -20);

    var frame_count: u32 = 0;

    const drawUText = struct {
        fn draw(f: rl.Font, text: []const u8, x: f32, y: f32, size: f32, col: rl.Color) void {
            var zbuf: [256]u8 = undefined;
            const zlen = @min(text.len, 255);
            @memcpy(zbuf[0..zlen], text[0..zlen]);
            zbuf[zlen] = 0;
            rl.DrawTextEx(f, &zbuf, .{ .x = x, .y = y }, size, 1.0, col);
        }
    }.draw;

    while (!rl.WindowShouldClose()) {
        const dt = rl.GetFrameTime();
        frame_count += 1;
        if (frame_count == 25 or rl.IsKeyPressed(rl.KEY_F12)) {
            rl.TakeScreenshot("p3_race_screenshot.png");
        }

        current_lap_time += dt;

        // --- BOLIDE CONTROLS ---
        if (rl.IsKeyDown(rl.KEY_W) or rl.IsKeyDown(rl.KEY_UP)) {
            bolide_speed = @min(240.0, bolide_speed + 95.0 * dt);
        } else if (rl.IsKeyDown(rl.KEY_S) or rl.IsKeyDown(rl.KEY_DOWN)) {
            bolide_speed = @max(0.0, bolide_speed - 120.0 * dt);
        } else {
            bolide_speed = @max(0.0, bolide_speed - 25.0 * dt); // Coasting friction
        }

        // Nitro Boost
        var nitro_active = false;
        if (rl.IsKeyDown(rl.KEY_SPACE) and nitro_energy > 5.0) {
            bolide_speed = @min(380.0, bolide_speed + 220.0 * dt);
            nitro_energy = @max(0.0, nitro_energy - 35.0 * dt);
            nitro_active = true;
        } else {
            nitro_energy = @min(100.0, nitro_energy + 12.0 * dt);
        }

        // Steering / Drift
        if (rl.IsKeyDown(rl.KEY_A) or rl.IsKeyDown(rl.KEY_LEFT)) lateral_offset = @max(-5.5, lateral_offset - 9.0 * dt);
        if (rl.IsKeyDown(rl.KEY_D) or rl.IsKeyDown(rl.KEY_RIGHT)) lateral_offset = @min(5.5, lateral_offset + 9.0 * dt);

        // Advance along spline
        const speed_in_nodes = (bolide_speed / 3.6) * 0.04;
        track_t += speed_in_nodes * dt;
        if (track_t >= @as(f32, @floatFromInt(NUM_TRACK_WAYPOINTS))) {
            track_t -= @as(f32, @floatFromInt(NUM_TRACK_WAYPOINTS));
            current_lap += 1;
            if (best_lap_time == 0.0 or current_lap_time < best_lap_time) {
                best_lap_time = current_lap_time;
            }
            current_lap_time = 0.0;
        }

        // Current Bolide Position on Circuit
        const idx0 = @as(usize, @intFromFloat(@floor(track_t))) % NUM_TRACK_WAYPOINTS;
        const idx1 = (idx0 + 1) % NUM_TRACK_WAYPOINTS;
        const frac = track_t - @floor(track_t);

        const n0 = track_nodes[idx0];
        const n1 = track_nodes[idx1];

        const center_pos = Vec3.init(
            n0.pos.x * (1.0 - frac) + n1.pos.x * frac,
            n0.pos.y * (1.0 - frac) + n1.pos.y * frac,
            n0.pos.z * (1.0 - frac) + n1.pos.z * frac,
        );
        const fwd_dir = Vec3.init(
            n0.fwd.x * (1.0 - frac) + n1.fwd.x * frac,
            n0.fwd.y * (1.0 - frac) + n1.fwd.y * frac,
            n0.fwd.z * (1.0 - frac) + n1.fwd.z * frac,
        ).normalize();
        const r_dir = Vec3.init(
            n0.right.x * (1.0 - frac) + n1.right.x * frac,
            n0.right.y * (1.0 - frac) + n1.right.y * frac,
            n0.right.z * (1.0 - frac) + n1.right.z * frac,
        ).normalize();

        const bolide_pos = Vec3.init(
            center_pos.x + r_dir.x * lateral_offset,
            center_pos.y + r_dir.y * lateral_offset + 0.35,
            center_pos.z + r_dir.z * lateral_offset,
        );

        // Smooth Spring-Arm Chase Camera
        const target_cam = Vec3.init(
            bolide_pos.x - fwd_dir.x * (11.0 + bolide_speed * 0.02) + n0.normal.x * 3.8,
            bolide_pos.y - fwd_dir.y * (11.0 + bolide_speed * 0.02) + n0.normal.y * 3.8,
            bolide_pos.z - fwd_dir.z * (11.0 + bolide_speed * 0.02) + n0.normal.z * 3.8,
        );
        cam_eye = Vec3.init(
            cam_eye.x + (target_cam.x - cam_eye.x) * 12.0 * dt,
            cam_eye.y + (target_cam.y - cam_eye.y) * 12.0 * dt,
            cam_eye.z + (target_cam.z - cam_eye.z) * 12.0 * dt,
        );

        const camera = rl.Camera3D{
            .position = .{ .x = cam_eye.x, .y = cam_eye.y, .z = cam_eye.z },
            .target = .{ .x = bolide_pos.x + fwd_dir.x * 4.0, .y = bolide_pos.y + 0.8, .z = bolide_pos.z + fwd_dir.z * 4.0 },
            .up = .{ .x = n0.normal.x, .y = n0.normal.y, .z = n0.normal.z },
            .fovy = 58.0 + (if (nitro_active) @as(f32, 14.0) else @as(f32, 0.0)),
            .projection = rl.CAMERA_PERSPECTIVE,
        };

        // --- RENDER SCENE ---
        rl.BeginDrawing();
        rl.ClearBackground(.{ .r = 8, .g = 10, .b = 18, .a = 255 });

        rl.BeginMode3D(camera);

        // 1. Cyberpunk Skyscrapers with Neon Light Strips
        for (buildings) |b| {
            const bpos = rl.Vector3{ .x = b.pos.x, .y = b.pos.y, .z = b.pos.z };
            rl.DrawCube(bpos, b.width, b.height, b.depth, b.color);
            rl.DrawCubeWires(bpos, b.width, b.height, b.depth, b.neon_color);
        }

        // 2. Track Surface and Dual Neon Guardrails
        var ti: usize = 0;
        while (ti < NUM_TRACK_WAYPOINTS) : (ti += 1) {
            const t0 = track_nodes[ti];
            const t1 = track_nodes[(ti + 1) % NUM_TRACK_WAYPOINTS];
            const hw: f32 = 6.5;

            const p0_l = rl.Vector3{ .x = t0.pos.x - t0.right.x * hw, .y = t0.pos.y, .z = t0.pos.z - t0.right.z * hw };
            const p0_r = rl.Vector3{ .x = t0.pos.x + t0.right.x * hw, .y = t0.pos.y, .z = t0.pos.z + t0.right.z * hw };
            const p1_l = rl.Vector3{ .x = t1.pos.x - t1.right.x * hw, .y = t1.pos.y, .z = t1.pos.z - t1.right.z * hw };
            const p1_r = rl.Vector3{ .x = t1.pos.x + t1.right.x * hw, .y = t1.pos.y, .z = t1.pos.z + t1.right.z * hw };

            const asphalt_col = if (ti % 2 == 0) rl.Color{ .r = 24, .g = 28, .b = 40, .a = 255 } else rl.Color{ .r = 20, .g = 24, .b = 34, .a = 255 };
            rl.DrawTriangle3D(p0_l, p1_l, p0_r, asphalt_col);
            rl.DrawTriangle3D(p0_r, p1_l, p1_r, asphalt_col);

            // Neon Glowing Guardrails (Cyan Left, Magenta Right)
            rl.DrawLine3D(p0_l, p1_l, .{ .r = 0, .g = 240, .b = 255, .a = 255 });
            rl.DrawLine3D(p0_r, p1_r, .{ .r = 255, .g = 0, .b = 130, .a = 255 });

            // Boost Gate Holographic Arches
            if (t0.has_gate) {
                const gate_top_l = rl.Vector3{ .x = p0_l.x, .y = p0_l.y + 6.0, .z = p0_l.z };
                const gate_top_r = rl.Vector3{ .x = p0_r.x, .y = p0_r.y + 6.0, .z = p0_r.z };
                rl.DrawLine3D(p0_l, gate_top_l, .{ .r = 0, .g = 255, .b = 180, .a = 255 });
                rl.DrawLine3D(p0_r, gate_top_r, .{ .r = 0, .g = 255, .b = 180, .a = 255 });
                rl.DrawLine3D(gate_top_l, gate_top_r, .{ .r = 0, .g = 255, .b = 180, .a = 255 });
            }
        }

        // 3. Cyber-Bolide Render
        const yaw_angle = std.math.atan2(fwd_dir.x, fwd_dir.z);
        const cos_y = @cos(yaw_angle);
        const sin_y = @sin(yaw_angle);

        var mi: usize = 0;
        while (mi + 2 < bolide_mesh.indices.items.len) : (mi += 3) {
            const v0 = bolide_mesh.vertices.items[bolide_mesh.indices.items[mi]];
            const v1 = bolide_mesh.vertices.items[bolide_mesh.indices.items[mi + 1]];
            const v2 = bolide_mesh.vertices.items[bolide_mesh.indices.items[mi + 2]];

            const transformVert = struct {
                fn apply(v: Vec3, px: f32, py: f32, pz: f32, cy: f32, sy: f32) rl.Vector3 {
                    const x_rot = v.x * cy + v.z * sy;
                    const z_rot = -v.x * sy + v.z * cy;
                    return .{ .x = @floatCast(px + x_rot), .y = @floatCast(py + v.y), .z = @floatCast(pz + z_rot) };
                }
            }.apply;

            const tp0 = transformVert(v0.pos, bolide_pos.x, bolide_pos.y, bolide_pos.z, cos_y, sin_y);
            const tp1 = transformVert(v1.pos, bolide_pos.x, bolide_pos.y, bolide_pos.z, cos_y, sin_y);
            const tp2 = transformVert(v2.pos, bolide_pos.x, bolide_pos.y, bolide_pos.z, cos_y, sin_y);

            const col = rl.Color{ .r = v0.color[0], .g = v0.color[1], .b = v0.color[2], .a = v0.color[3] };
            rl.DrawTriangle3D(tp0, tp1, tp2, col);
            rl.DrawTriangle3D(tp0, tp2, tp1, col);
            rl.DrawLine3D(tp0, tp1, .{ .r = 0, .g = 240, .b = 255, .a = 80 });
        }

        // Nitro Plasma Jet Trail
        if (bolide_speed > 20.0) {
            const jet_pos = rl.Vector3{ .x = bolide_pos.x - fwd_dir.x * 2.2, .y = bolide_pos.y + 0.3, .z = bolide_pos.z - fwd_dir.z * 2.2 };
            rl.DrawSphere(jet_pos, 0.35 + (if (nitro_active) @as(f32, 0.4) else @as(f32, 0.0)), .{ .r = 0, .g = 220, .b = 255, .a = 220 });
        }

        rl.EndMode3D();

        // --- CYBERPUNK HUD ---
        rl.DrawRectangle(0, 0, 1280, 55, .{ .r = 14, .g = 18, .b = 28, .a = 235 });
        drawUText(font, "P³ NEON VECTOR: CYBERPUNK ORBITAL GRAND PRIX", 20, 16, 20, .{ .r = 0, .g = 240, .b = 255, .a = 255 });

        // Speedometer
        var sbuf: [64]u8 = undefined;
        const slen = std.fmt.bufPrint(&sbuf, "{d:3.0} KM/H", .{bolide_speed}) catch "0 KM/H";
        drawUText(font, slen, 1080, 14, 24, .{ .r = 255, .g = 0, .b = 130, .a = 255 });

        // Nitro Gauge
        rl.DrawRectangle(920, 640, 320, 24, .{ .r = 20, .g = 25, .b = 38, .a = 220 });
        rl.DrawRectangle(920, 640, @intFromFloat(320.0 * (nitro_energy / 100.0)), 24, .{ .r = 0, .g = 240, .b = 255, .a = 255 });
        drawUText(font, "NITRO BOOST [SPACE]", 925, 615, 16, .{ .r = 0, .g = 240, .b = 255, .a = 255 });

        // Lap Times & Position
        var lbuf: [64]u8 = undefined;
        const llen = std.fmt.bufPrint(&lbuf, "LAP: {d} | TIME: {d:.2}s", .{ current_lap, current_lap_time }) catch "";
        drawUText(font, llen, 20, 70, 17, .{ .r = 255, .g = 220, .b = 60, .a = 255 });

        // Radar Minimap (Bottom Left)
        rl.DrawRectangle(20, 520, 160, 160, .{ .r = 12, .g = 16, .b = 24, .a = 200 });
        rl.DrawRectangleLines(20, 520, 160, 160, .{ .r = 0, .g = 240, .b = 255, .a = 255 });
        const map_cx: f32 = 100.0;
        const map_cy: f32 = 600.0;
        const map_scale: f32 = 0.7;

        var m_i: usize = 0;
        while (m_i < NUM_TRACK_WAYPOINTS) : (m_i += 2) {
            const mp = track_nodes[m_i].pos;
            rl.DrawPixel(@intFromFloat(map_cx + mp.x * map_scale), @intFromFloat(map_cy + mp.z * map_scale), .{ .r = 100, .g = 140, .b = 200, .a = 255 });
        }
        rl.DrawCircle(@intFromFloat(map_cx + bolide_pos.x * map_scale), @intFromFloat(map_cy + bolide_pos.z * map_scale), 3.5, .{ .r = 255, .g = 0, .b = 130, .a = 255 });

        rl.EndDrawing();
    }
}
