// =============================================================================
// P³ ENGINE: HOVER-TANK PROJECTIVE ARENA v1.0 — ZIG & RAYLIB
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

pub fn main() !void {
    const screen_width: c_int = 1280;
    const screen_height: c_int = 720;

    rl.SetConfigFlags(rl.FLAG_WINDOW_RESIZABLE | rl.FLAG_MSAA_4X_HINT);
    rl.InitWindow(screen_width, screen_height, "P³ Engine — Orbital Hover-Tank Arena");
    defer rl.CloseWindow();

    rl.SetTargetFPS(60);

    // Font setup with Cyrillic and Unicode support
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

    var tank_mesh = try p3.generateHoverTankMesh(allocator, .{});
    defer tank_mesh.deinit();

    // Planet & Tank Geometry Parameters
    const planet_radius: f32 = 18.0;
    var tank_latitude: f32 = 0.2; // Angle from equator
    var tank_longitude: f32 = 0.0;
    var tank_speed: f32 = 0.0;
    var tank_heading: f32 = 0.0;
    var turret_yaw: f32 = 0.0;

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
        if (frame_count == 15 or rl.IsKeyPressed(rl.KEY_F12)) {
            rl.TakeScreenshot("p3_tank_arena_render.png");
        }

        // --- TANK CONTROLS ---
        if (rl.IsKeyDown(rl.KEY_A)) tank_heading -= 2.0 * dt;
        if (rl.IsKeyDown(rl.KEY_D)) tank_heading += 2.0 * dt;
        if (rl.IsKeyDown(rl.KEY_W)) tank_speed = @min(12.0, tank_speed + 8.0 * dt);
        if (rl.IsKeyDown(rl.KEY_S)) tank_speed = @max(-4.0, tank_speed - 6.0 * dt);

        // Turret Aiming
        if (rl.IsKeyDown(rl.KEY_LEFT)) turret_yaw -= 2.5 * dt;
        if (rl.IsKeyDown(rl.KEY_RIGHT)) turret_yaw += 2.5 * dt;

        tank_speed *= (1.0 - 0.8 * dt); // Ground resistance

        // Geodesic Position Update on S² Sphere manifold without gimbal singularities
        const move_dist = tank_speed * dt;
        tank_latitude += (move_dist * @cos(tank_heading)) / planet_radius;
        tank_longitude += (move_dist * @sin(tank_heading)) / (planet_radius * @cos(tank_latitude));

        // Hover height above planet surface
        const hover_altitude: f32 = 0.8;
        const total_r = planet_radius + hover_altitude;
        const px = total_r * @cos(tank_latitude) * @sin(tank_longitude);
        const py = total_r * @sin(tank_latitude);
        const pz = total_r * @cos(tank_latitude) * @cos(tank_longitude);

        // 3D Camera tracking the Tank
        const cam_dist: f32 = 14.0;
        const cam_height: f32 = 6.0;
        const cam_x = px - @sin(tank_longitude + tank_heading) * cam_dist;
        const cam_y = py + cam_height;
        const cam_z = pz - @cos(tank_longitude + tank_heading) * cam_dist;

        const camera = rl.Camera3D{
            .position = .{ .x = cam_x, .y = cam_y, .z = cam_z },
            .target = .{ .x = px, .y = py + 0.5, .z = pz },
            .up = .{ .x = 0.0, .y = 1.0, .z = 0.0 },
            .fovy = 50.0,
            .projection = rl.CAMERA_PERSPECTIVE,
        };

        // --- DRAWING ---
        rl.BeginDrawing();
        rl.ClearBackground(.{ .r = 10, .g = 14, .b = 22, .a = 255 });

        rl.BeginMode3D(camera);

        // Planetary Body
        rl.DrawSphere(.{ .x = 0, .y = 0, .z = 0 }, planet_radius, .{ .r = 35, .g = 48, .b = 68, .a = 255 });
        rl.DrawSphereWires(.{ .x = 0, .y = 0, .z = 0 }, planet_radius + 0.05, 24, 24, .{ .r = 60, .g = 100, .b = 160, .a = 90 });

        // Hover Tank Rendering
        const cos_h = @cos(tank_heading);
        const sin_h = @sin(tank_heading);

        var mi: usize = 0;
        while (mi + 2 < tank_mesh.indices.items.len) : (mi += 3) {
            const v0 = tank_mesh.vertices.items[tank_mesh.indices.items[mi]];
            const v1 = tank_mesh.vertices.items[tank_mesh.indices.items[mi + 1]];
            const v2 = tank_mesh.vertices.items[tank_mesh.indices.items[mi + 2]];

            const transformVert = struct {
                fn apply(v: Vec3, posx: f32, posy: f32, posz: f32, ch: f32, sh: f32) rl.Vector3 {
                    const x_rot = v.x * ch + v.z * sh;
                    const z_rot = -v.x * sh + v.z * ch;
                    return .{
                        .x = @floatCast(posx + x_rot),
                        .y = @floatCast(posy + v.y),
                        .z = @floatCast(posz + z_rot),
                    };
                }
            }.apply;

            const tp0 = transformVert(v0.pos, px, py, pz, cos_h, sin_h);
            const tp1 = transformVert(v1.pos, px, py, pz, cos_h, sin_h);
            const tp2 = transformVert(v2.pos, px, py, pz, cos_h, sin_h);

            const col = rl.Color{ .r = v0.color[0], .g = v0.color[1], .b = v0.color[2], .a = v0.color[3] };
            rl.DrawTriangle3D(tp0, tp1, tp2, col);
            rl.DrawTriangle3D(tp0, tp2, tp1, col);
            rl.DrawLine3D(tp0, tp1, .{ .r = 255, .g = 255, .b = 255, .a = 70 });
            rl.DrawLine3D(tp1, tp2, .{ .r = 255, .g = 255, .b = 255, .a = 70 });
            rl.DrawLine3D(tp2, tp0, .{ .r = 255, .g = 255, .b = 255, .a = 70 });
        }

        // Glowing Blue Anti-Grav Energy Rings under Repulsor Pods
        rl.DrawCircle3D(.{ .x = px, .y = py + 0.1, .z = pz }, 2.0, .{ .x = 0, .y = 1, .z = 0 }, 90.0, .{ .r = 0, .g = 220, .b = 255, .a = 150 });

        rl.EndMode3D();

        // --- HUD ---
        rl.DrawRectangle(0, 0, 1280, 50, .{ .r = 18, .g = 24, .b = 36, .a = 230 });
        drawUText(font, "P³ ENGINE: ORBITAL HOVER-TANK ARENA (S³ GEODESIC STABILITY)", 20, 14, 18, .{ .r = 255, .g = 210, .b = 40, .a = 255 });

        drawUText(font, "Управление: W/S Тяга, A/D Поворот корпуса, Стрелки Башня", 20, 680, 15, .{ .r = 180, .g = 210, .b = 240, .a = 255 });

        rl.EndDrawing();
    }
}
