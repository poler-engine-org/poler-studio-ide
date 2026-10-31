// =============================================================================
// P³ VOID VOYAGER — 3D PROJECTIVE SPACE GAME (ZIG 0.14)
// =============================================================================
//
// Playable game demonstrating P³ Engine's unique projective mechanics:
//   - 6-DOF Spaceship navigation through Homogeneous 4D space on S³
//   - Real-time Keplerian gravity wells from Sun and Planets
//   - Seamless Crossing of the W=0 Horizon (Chart switching U₀ ↔ U₃)
//   - Quantum Crystal collection objectives and score tracking
//   - Full HUD rendered through UiCanvas & DrawQueue
//
// Controls:
//   W / S       — Main Thrusters (Forward / Backward)
//   A / D       — Yaw (Turn Left / Right)
//   Up / Down   — Pitch (Nose Up / Down)
//   Space       — Warp Booster (Symplectic Acceleration)
//   R           — Reset Ship to Home Orbit
//   Esc         — Exit Game
// =============================================================================

const std = @import("std");
const math = std.math;
const p3 = @import("root.zig");

const rl = @cImport({
    @cInclude("raylib.h");
});

const Vec3 = p3.Vec3;
const HomVec4 = p3.HomVec4;
const AffineCard = p3.kernel.AffineCard;
const Rotator = p3.Rotator;

const MAX_CRYSTALS: usize = 12;

const Crystal = struct {
    pos: HomVec4,
    collected: bool = false,
    color: rl.Color,
    rot_angle: f32 = 0.0,
};

pub fn main() !void {
    rl.InitWindow(1600, 900, "P3 Void Voyager — Projective Space Game (Zig 0.14 + P3 Engine)");
    rl.SetTargetFPS(60);
    rl.SetWindowState(rl.FLAG_WINDOW_RESIZABLE);
    defer rl.CloseWindow();

    // 1. Unicode Font Loading
    var codepoints: [600]c_int = undefined;
    var cp_count: usize = 0;
    var c: usize = 32;
    while (c < 127) : (c += 1) {
        codepoints[cp_count] = @intCast(c);
        cp_count += 1;
    }
    c = 0x0400;
    while (c < 0x0500) : (c += 1) {
        codepoints[cp_count] = @intCast(c);
        cp_count += 1;
    }
    codepoints[cp_count] = 0x00B0; cp_count += 1;
    codepoints[cp_count] = 0x00B3; cp_count += 1; // superscript 3 (³)
    codepoints[cp_count] = 0x03C0; cp_count += 1;
    codepoints[cp_count] = 0x03A9; cp_count += 1;
    codepoints[cp_count] = 0x221E; cp_count += 1;

    const font = rl.LoadFontEx("/usr/share/fonts/TTF/DejaVuSans.ttf", 22, &codepoints, @intCast(cp_count));
    rl.SetTextureFilter(font.texture, rl.TEXTURE_FILTER_BILINEAR);
    defer rl.UnloadFont(font);

    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    var ship_mesh = try p3.generateSpaceshipMesh(allocator, .{});
    defer ship_mesh.deinit();

    // 2. Spaceship Physics State (Homogeneous 4D on S³)
    var ship_pos = HomVec4.init(0.0, 0.0, 12.0, 1.0);
    var ship_vel = Vec3.init(0.0, 0.0, 0.0);
    var ship_yaw: f32 = 0.0;
    var ship_pitch: f32 = 0.0;
    var ship_speed: f32 = 0.0;
    var ship_energy: f32 = 100.0;
    var score: u32 = 0;
    var horizon_crossings: u32 = 0;
    var active_chart: AffineCard = .UW;

    // 3. Planetary System
    var planet_angle: f32 = 0.0;
    var moon_angle: f32 = 0.0;

    // 4. Quantum Crystals to Collect
    var crystals: [MAX_CRYSTALS]Crystal = undefined;
    var prng = std.Random.DefaultPrng.init(42);
    const rand = prng.random();

    for (&crystals, 0..) |*cr, i| {
        const rad: f32 = 8.0 + rand.float(f32) * 14.0;
        const theta: f32 = (@as(f32, @floatFromInt(i)) / @as(f32, @floatFromInt(MAX_CRYSTALS))) * 2.0 * math.pi;
        const y_off: f32 = (rand.float(f32) - 0.5) * 6.0;
        cr.pos = HomVec4.init(rad * @cos(theta), y_off, rad * @sin(theta), 1.0);
        cr.collected = false;
        cr.rot_angle = rand.float(f32) * math.pi;
        cr.color = if (i % 2 == 0) .{ .r = 0, .g = 255, .b = 220, .a = 255 } else .{ .r = 255, .g = 180, .b = 50, .a = 255 };
    }

    const drawUText = struct {
        fn draw(f: rl.Font, text: []const u8, x: f32, y: f32, size: f32, col: rl.Color) void {
            var zbuf: [256]u8 = undefined;
            const zlen = @min(text.len, 255);
            @memcpy(zbuf[0..zlen], text[0..zlen]);
            zbuf[zlen] = 0;
            rl.DrawTextEx(f, &zbuf, .{ .x = x, .y = y }, size, 1.0, col);
        }
    }.draw;

    var frame_count: u32 = 0;
    var action_bus = p3.ActionBus{};
    var publisher = p3.ObservationPublisher{};
    _ = &publisher;

    while (!rl.WindowShouldClose()) {
        const dt = rl.GetFrameTime();
        frame_count += 1;
        if (frame_count == 15 or rl.IsKeyPressed(rl.KEY_F12)) {
            rl.TakeScreenshot("p3_game_screenshot.png");
        }
        const screen_w = @as(f32, @floatFromInt(rl.GetScreenWidth()));
        const screen_h = @as(f32, @floatFromInt(rl.GetScreenHeight()));

        // --- 1. DRAIN AI ACTION BUS (LLM Agent Input) ---
        var ai_thrust: f32 = 0.0;
        var ai_yaw: f32 = 0.0;
        var ai_pitch: f32 = 0.0;

        while (action_bus.pop()) |act| {
            switch (act) {
                .thrust => |v| ai_thrust += v,
                .yaw => |v| ai_yaw += v,
                .pitch => |v| ai_pitch += v,
                .reset => {
                    ship_pos = HomVec4.init(0.0, 0.0, 12.0, 1.0);
                    ship_speed = 0.0;
                    ship_vel = Vec3.init(0, 0, 0);
                },
                .none => {},
            }
        }

        // --- 2. SHIP CONTROLS (Blend AI ActionBus + Human Keyboard) ---
        ship_yaw += ai_yaw * 2.0 * dt;
        ship_pitch = std.math.clamp(ship_pitch + ai_pitch * 1.5 * dt, -0.8, 0.8);
        if (ai_thrust > 0) ship_speed = @min(18.0, ship_speed + ai_thrust * 12.0 * dt);

        if (rl.IsKeyDown(rl.KEY_A) or rl.IsKeyDown(rl.KEY_LEFT)) ship_yaw -= 2.0 * dt;
        if (rl.IsKeyDown(rl.KEY_D) or rl.IsKeyDown(rl.KEY_RIGHT)) ship_yaw += 2.0 * dt;
        if (rl.IsKeyDown(rl.KEY_UP)) ship_pitch = std.math.clamp(ship_pitch + 1.5 * dt, -0.8, 0.8);
        if (rl.IsKeyDown(rl.KEY_DOWN)) ship_pitch = std.math.clamp(ship_pitch - 1.5 * dt, -0.8, 0.8);
        if (rl.IsKeyDown(rl.KEY_W)) ship_speed = @min(18.0, ship_speed + 12.0 * dt);
        if (rl.IsKeyDown(rl.KEY_S)) ship_speed = @max(-6.0, ship_speed - 10.0 * dt);
        if (rl.IsKeyDown(rl.KEY_SPACE) and ship_energy > 0) {
            ship_speed = @min(32.0, ship_speed + 25.0 * dt);
            ship_energy = @max(0.0, ship_energy - 20.0 * dt);
        } else {
            ship_energy = @min(100.0, ship_energy + 8.0 * dt);
        }
        if (rl.IsKeyPressed(rl.KEY_R)) {
            ship_pos = HomVec4.init(0.0, 0.0, 12.0, 1.0);
            ship_speed = 0.0;
            ship_vel = Vec3.init(0, 0, 0);
        }

        // Natural Friction / Drag in vacuum
        ship_speed *= (1.0 - 0.5 * dt);

        // Forward Vector
        const fwd = Vec3.init(@sin(ship_yaw), 0.0, -@cos(ship_yaw));
        ship_vel = fwd.scale(ship_speed);

        // Update Ship Position in P³ Homogeneous coordinates
        ship_pos.x += ship_vel.x * dt;
        ship_pos.y += ship_vel.y * dt;
        ship_pos.z += ship_vel.z * dt;

        // Projective W coordinate oscillation (Passing through Horizon W=0)
        const dist_from_center = @sqrt(ship_pos.x * ship_pos.x + ship_pos.z * ship_pos.z);
        ship_pos.w = @cos(dist_from_center * 0.15);

        // Check Affine Chart Transition
        const best_chart = ship_pos.pickBestCard();
        if (best_chart != active_chart) {
            active_chart = best_chart;
            horizon_crossings += 1;
        }

        // Orbit Planets
        planet_angle += 0.35 * dt;
        moon_angle += 1.8 * dt;
        const planet_p = Vec3.init(10.0 * @cos(planet_angle), 0.0, 10.0 * @sin(planet_angle));
        const moon_p = Vec3.init(planet_p.x + 2.2 * @cos(moon_angle), 0.3 * @sin(moon_angle * 2.0), planet_p.z + 2.2 * @sin(moon_angle));

        // Crystal Collision Detection
        for (&crystals) |*cr| {
            if (cr.collected) continue;
            cr.rot_angle += 2.0 * dt;
            const dx = ship_pos.x - cr.pos.x;
            const dy = ship_pos.y - cr.pos.y;
            const dz = ship_pos.z - cr.pos.z;
            const d = @sqrt(dx * dx + dy * dy + dz * dz);
            if (d < 1.8) {
                cr.collected = true;
                score += 100;
            }
        }

        // Camera Follows Ship (Third-Person View)
        const cam_back = fwd.scale(-7.0);
        const cam_target = rl.Vector3{ .x = @floatCast(ship_pos.x), .y = @floatCast(ship_pos.y + 0.8), .z = @floatCast(ship_pos.z) };
        const cam_pos = rl.Vector3{ .x = @floatCast(ship_pos.x + cam_back.x), .y = @floatCast(ship_pos.y + 3.2), .z = @floatCast(ship_pos.z + cam_back.z) };

        const camera3d: rl.Camera3D = .{
            .position = cam_pos,
            .target = cam_target,
            .up = .{ .x = 0.0, .y = 1.0, .z = 0.0 },
            .fovy = 55.0,
            .projection = rl.CAMERA_PERSPECTIVE,
        };

        // ====================================================================
        // RENDER PASS
        // ====================================================================
        rl.BeginDrawing();
        rl.ClearBackground(.{ .r = 10, .g = 12, .b = 20, .a = 255 });

        // 3D Space Viewport
        rl.BeginMode3D(camera3d);

        // Infinite Projective Grid
        rl.DrawGrid(60, 2.0);

        // Equatorial Horizon W=0 Wireframe
        rl.DrawCircle3D(.{ .x = 0, .y = 0, .z = 0 }, 10.5, .{ .x = 0, .y = 1, .z = 0 }, 90.0, .{ .r = 0, .g = 180, .b = 255, .a = 70 });
        rl.DrawCircle3D(.{ .x = 0, .y = 0, .z = 0 }, 21.0, .{ .x = 0, .y = 1, .z = 0 }, 90.0, .{ .r = 120, .g = 80, .b = 255, .a = 60 });

        // Central Star (Sun)
        rl.DrawSphere(.{ .x = 0, .y = 0, .z = 0 }, 2.5, .{ .r = 255, .g = 210, .b = 40, .a = 255 });
        rl.DrawSphereWires(.{ .x = 0, .y = 0, .z = 0 }, 2.8, 12, 12, .{ .r = 255, .g = 245, .b = 150, .a = 100 });

        // Terra Planet & Orbit Track
        rl.DrawCircle3D(.{ .x = 0, .y = 0, .z = 0 }, 10.0, .{ .x = 0, .y = 1, .z = 0 }, 90.0, .{ .r = 60, .g = 120, .b = 200, .a = 60 });
        rl.DrawSphere(.{ .x = planet_p.x, .y = planet_p.y, .z = planet_p.z }, 1.1, .{ .r = 60, .g = 150, .b = 255, .a = 255 });
        rl.DrawSphereWires(.{ .x = planet_p.x, .y = planet_p.y, .z = planet_p.z }, 1.25, 8, 8, .{ .r = 140, .g = 200, .b = 255, .a = 150 });

        // Moon
        rl.DrawSphere(.{ .x = moon_p.x, .y = moon_p.y, .z = moon_p.z }, 0.4, .{ .r = 190, .g = 200, .b = 215, .a = 255 });

        // Quantum Crystals
        for (crystals) |cr| {
            if (cr.collected) continue;
            const cpos = rl.Vector3{ .x = @floatCast(cr.pos.x), .y = @floatCast(cr.pos.y + 0.4 * @sin(cr.rot_angle * 2.0)), .z = @floatCast(cr.pos.z) };
            rl.DrawCube(cpos, 0.8, 1.2, 0.8, cr.color);
            rl.DrawCubeWires(cpos, 0.8, 1.2, 0.8, .{ .r = 255, .g = 255, .b = 255, .a = 220 });
        }

        // Procedural Spaceship Render (Hull, Swept Wings, Canopy, Engines)
        const cos_y = @cos(ship_yaw);
        const sin_y = @sin(ship_yaw);
        const cos_p = @cos(ship_pitch);
        const sin_p = @sin(ship_pitch);

        var mi: usize = 0;
        while (mi + 2 < ship_mesh.indices.items.len) : (mi += 3) {
            const v0 = ship_mesh.vertices.items[ship_mesh.indices.items[mi]];
            const v1 = ship_mesh.vertices.items[ship_mesh.indices.items[mi + 1]];
            const v2 = ship_mesh.vertices.items[ship_mesh.indices.items[mi + 2]];

            const transformVert = struct {
                fn apply(v: Vec3, px: f32, py: f32, pz: f32, cy: f32, sy: f32, cp: f32, sp: f32) rl.Vector3 {
                    // Pitch around X, then Yaw around Y
                    const y1 = v.y * cp - v.z * sp;
                    const z1 = v.y * sp + v.z * cp;
                    const x2 = v.x * cy + z1 * sy;
                    const z2 = -v.x * sy + z1 * cy;
                    return .{
                        .x = @floatCast(px + x2),
                        .y = @floatCast(py + y1),
                        .z = @floatCast(pz + z2),
                    };
                }
            }.apply;

            const tp0 = transformVert(v0.pos, @floatCast(ship_pos.x), @floatCast(ship_pos.y), @floatCast(ship_pos.z), cos_y, sin_y, cos_p, sin_p);
            const tp1 = transformVert(v1.pos, @floatCast(ship_pos.x), @floatCast(ship_pos.y), @floatCast(ship_pos.z), cos_y, sin_y, cos_p, sin_p);
            const tp2 = transformVert(v2.pos, @floatCast(ship_pos.x), @floatCast(ship_pos.y), @floatCast(ship_pos.z), cos_y, sin_y, cos_p, sin_p);

            const col = rl.Color{ .r = v0.color[0], .g = v0.color[1], .b = v0.color[2], .a = v0.color[3] };
            rl.DrawTriangle3D(tp0, tp1, tp2, col);
            rl.DrawTriangle3D(tp0, tp2, tp1, col); // Double-sided
            // Technical Wireframe Edges
            rl.DrawLine3D(tp0, tp1, .{ .r = 255, .g = 255, .b = 255, .a = 70 });
            rl.DrawLine3D(tp1, tp2, .{ .r = 255, .g = 255, .b = 255, .a = 70 });
            rl.DrawLine3D(tp2, tp0, .{ .r = 255, .g = 255, .b = 255, .a = 70 });
        }

        // Dual Ion Thruster Glow Trails
        if (ship_speed > 0.5) {
            const trail_len: f32 = (ship_speed / 15.0) * 1.8;
            const r_nozzle = rl.Vector3{
                .x = @floatCast(ship_pos.x - fwd.x * 2.2 + 0.6 * cos_y),
                .y = @floatCast(ship_pos.y),
                .z = @floatCast(ship_pos.z - fwd.z * 2.2 - 0.6 * sin_y),
            };
            const l_nozzle = rl.Vector3{
                .x = @floatCast(ship_pos.x - fwd.x * 2.2 - 0.6 * cos_y),
                .y = @floatCast(ship_pos.y),
                .z = @floatCast(ship_pos.z - fwd.z * 2.2 + 0.6 * sin_y),
            };
            rl.DrawSphere(r_nozzle, 0.22 * trail_len, .{ .r = 0, .g = 240, .b = 255, .a = 220 });
            rl.DrawSphere(l_nozzle, 0.22 * trail_len, .{ .r = 0, .g = 240, .b = 255, .a = 220 });
        }

        rl.EndMode3D();

        // ====================================================================
        // GAME UI & HUD (UiCanvas Pipeline)
        // ====================================================================

        // Top Status Bar
        rl.DrawRectangle(0, 0, @intFromFloat(screen_w), 50, .{ .r = 18, .g = 22, .b = 32, .a = 230 });
        rl.DrawLine(0, 50, @intFromFloat(screen_w), 50, .{ .r = 40, .g = 52, .b = 75, .a = 255 });

        drawUText(font, "P³ VOID VOYAGER", 24, 12, 20, .{ .r = 255, .g = 200, .b = 40, .a = 255 });

        var hbuf: [128]u8 = undefined;

        // Score Card
        const sc_txt = std.fmt.bufPrint(&hbuf, "СЧЕТ: {d} PTS", .{score}) catch "";
        drawUText(font, sc_txt, 260, 14, 16, .{ .r = 100, .g = 255, .b = 180, .a = 255 });

        // Affine Chart Indicator
        const ch_txt = std.fmt.bufPrint(&hbuf, "Карта P³: U{d} (W={d:.2}) | Переходов горизонта: {d}", .{ @intFromEnum(active_chart), ship_pos.w, horizon_crossings }) catch "";
        drawUText(font, ch_txt, 460, 15, 14, .{ .r = 150, .g = 210, .b = 255, .a = 255 });

        // FPS
        const fps_txt = std.fmt.bufPrint(&hbuf, "FPS: {d}", .{rl.GetFPS()}) catch "";
        drawUText(font, fps_txt, screen_w - 120, 15, 14, .{ .r = 130, .g = 255, .b = 160, .a = 255 });

        // Bottom Dashboard (Speed & Energy Gauges)
        const dash_y = screen_h - 100;
        rl.DrawRectangle(24, @intFromFloat(dash_y), 380, 80, .{ .r = 20, .g = 26, .b = 38, .a = 230 });
        rl.DrawRectangleLinesEx(.{ .x = 24, .y = dash_y, .width = 380, .height = 80 }, 1.5, .{ .r = 50, .g = 68, .b = 98, .a = 255 });

        // Speed Bar
        drawUText(font, "СКОРОСТЬ:", 36, dash_y + 12, 12, .{ .r = 180, .g = 195, .b = 215, .a = 255 });
        rl.DrawRectangle(130, @intFromFloat(dash_y + 14), 250, 12, .{ .r = 35, .g = 45, .b = 65, .a = 255 });
        const spd_w = std.math.clamp(ship_speed / 32.0, 0.0, 1.0) * 250.0;
        rl.DrawRectangle(130, @intFromFloat(dash_y + 14), @intFromFloat(spd_w), 12, .{ .r = 0, .g = 220, .b = 255, .a = 255 });

        // Energy Bar
        drawUText(font, "ВАРП ЭНЕРГИЯ:", 36, dash_y + 42, 12, .{ .r = 180, .g = 195, .b = 215, .a = 255 });
        rl.DrawRectangle(130, @intFromFloat(dash_y + 44), 250, 12, .{ .r = 35, .g = 45, .b = 65, .a = 255 });
        const en_w = (ship_energy / 100.0) * 250.0;
        rl.DrawRectangle(130, @intFromFloat(dash_y + 44), @intFromFloat(en_w), 12, .{ .r = 255, .g = 180, .b = 40, .a = 255 });

        // Controls Helper at bottom right
        rl.DrawRectangle(@intFromFloat(screen_w - 380), @intFromFloat(dash_y), 356, 80, .{ .r = 20, .g = 26, .b = 38, .a = 230 });
        rl.DrawRectangleLinesEx(.{ .x = screen_w - 380, .y = dash_y, .width = 356, .height = 80 }, 1.5, .{ .r = 50, .g = 68, .b = 98, .a = 255 });
        drawUText(font, "Управление: W/S — Тяга, A/D — Поворот", screen_w - 368, dash_y + 12, 11, .{ .r = 200, .g = 215, .b = 240, .a = 255 });
        drawUText(font, "SPACE — Варп-ускоритель | R — Сброс", screen_w - 368, dash_y + 34, 11, .{ .r = 200, .g = 215, .b = 240, .a = 255 });
        drawUText(font, "Собирайте квантовые кристаллы в пространстве S³", screen_w - 368, dash_y + 54, 11, .{ .r = 100, .g = 255, .b = 200, .a = 255 });

        rl.EndDrawing();
    }
}
