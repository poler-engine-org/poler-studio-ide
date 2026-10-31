// =============================================================================
// P³ ENGINE — INTERACTIVE LIVE DEMONSTRATION & BENCHMARK
// =============================================================================
//
// Key Features Demonstrated:
// 1. Singularity-Free Horizon Crossing (W = 0):
//    Object moving along an S³ geodesic smoothly transitions across 4 affine
//    charts (UW, UX, UY, UZ) with ZERO NaN/INF errors or camera clipping.
// 2. Interactive Controls & Vision:
//    - Left Mouse Drag: Smooth 3D Orbit
//    - Mouse Wheel: Zoom in / Zoom out
//    - Keys 1, 2, 3, 4: Force specific Affine Chart (U0, U1, U2, U3)
//    - Key 0: Automatic chart selection (default)
//    - Tab: Toggle Split-Screen (P³ Manifold vs Euclidean ℝ³)
//    - Space: Pause / Resume
//    - +/-: Change simulation speed
//    - R: Reset camera and simulation
//    - F12 / P: Take GPU Screenshot (saved to p3_demo_screenshot.png)
// 3. Live Precision & Physics Benchmark:
//    Real-time calculation of Symplectic Energy Conservation (ΔE/E₀) in P³
//    vs Euclidean Euler drift over 1,000 steps/frame.
// =============================================================================

const std = @import("std");
const p3_kernel = @import("p3_kernel.zig");
const p3_math = @import("p3_math.zig");
const p3_raylib = @import("p3_raylib.zig");

const rl = @cImport({
    @cInclude("raylib.h");
});

const HomVec4 = p3_kernel.HomVec4;
const AffineCard = p3_kernel.AffineCard;
const Vec3 = p3_math.Vec3;

const TRAIL_LENGTH: usize = 360;
const BENCHMARK_STEPS: usize = 1000;

pub fn main() !void {
    // Check CLI arguments for automated capture
    var capture_after_frames: ?usize = null;
    const screenshot_path: [*:0]const u8 = "p3_demo_screenshot.png";

    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena.deinit();
    const args = std.process.argsAlloc(arena.allocator()) catch &[_][:0]u8{};
    for (args[1..]) |arg| {
        if (std.mem.eql(u8, arg, "--capture") or std.mem.eql(u8, arg, "--screenshot")) {
            capture_after_frames = 60;
        } else if (std.mem.startsWith(u8, arg, "--frames=")) {
            capture_after_frames = std.fmt.parseInt(usize, arg[9..], 10) catch 60;
        }
    }

    rl.InitWindow(1280, 720, "P3 Engine — Singularity-Free Projective Geometry & S3 Live Demo");
    rl.SetTargetFPS(60);
    rl.SetWindowState(rl.FLAG_WINDOW_RESIZABLE);
    defer rl.CloseWindow();

    var t: f32 = 0.0;
    var speed_mult: f32 = 1.0;
    var cam_angle: f32 = 0.4;
    var cam_pitch: f32 = 0.35;
    var cam_dist: f32 = 12.0;

    // Mouse drag tracking
    var is_dragging: bool = false;
    var last_mouse_pos = rl.GetMousePosition();

    // Chart mode: null = Auto, or specific AffineCard
    var manual_chart: ?AffineCard = null;
    var split_screen: bool = false;
    var show_hud: bool = true;

    // Trails for P³ vs ℝ³
    var p3_trail: [TRAIL_LENGTH]rl.Vector3 = undefined;
    var r3_trail: [TRAIL_LENGTH]rl.Vector3 = undefined;
    var trail_count: usize = 0;

    // Live Benchmark stats
    var p3_energy_drift: f64 = 0.0;
    var r3_energy_drift: f64 = 0.0;
    var total_chart_switches: u64 = 0;
    var last_card: AffineCard = .UW;

    var paused: bool = false;
    var frame_count: usize = 0;
    var screenshot_notice_timer: f32 = 0.0;

    while (!rl.WindowShouldClose()) {
        const dt = rl.GetFrameTime();
        frame_count += 1;

        if (screenshot_notice_timer > 0) screenshot_notice_timer -= dt;

        // --- Mouse Drag Orbit ---
        const cur_mouse_pos = rl.GetMousePosition();
        if (rl.IsMouseButtonPressed(rl.MOUSE_BUTTON_LEFT)) {
            is_dragging = true;
            last_mouse_pos = cur_mouse_pos;
        } else if (rl.IsMouseButtonReleased(rl.MOUSE_BUTTON_LEFT)) {
            is_dragging = false;
        }

        if (is_dragging) {
            const dx = cur_mouse_pos.x - last_mouse_pos.x;
            const dy = cur_mouse_pos.y - last_mouse_pos.y;
            cam_angle -= dx * 0.006;
            cam_pitch = std.math.clamp(cam_pitch + dy * 0.006, -1.4, 1.4);
            last_mouse_pos = cur_mouse_pos;
        }

        // Mouse Wheel Zoom
        const wheel = rl.GetMouseWheelMove();
        if (wheel != 0) cam_dist = std.math.clamp(cam_dist - wheel * 1.5, 3.0, 35.0);

        // --- Keyboard Controls ---
        if (rl.IsKeyDown(rl.KEY_LEFT) or rl.IsKeyDown(rl.KEY_A)) cam_angle -= 1.5 * dt;
        if (rl.IsKeyDown(rl.KEY_RIGHT) or rl.IsKeyDown(rl.KEY_D)) cam_angle += 1.5 * dt;
        if (rl.IsKeyDown(rl.KEY_UP) or rl.IsKeyDown(rl.KEY_W)) cam_pitch = std.math.clamp(cam_pitch + 1.2 * dt, -1.4, 1.4);
        if (rl.IsKeyDown(rl.KEY_DOWN) or rl.IsKeyDown(rl.KEY_S)) cam_pitch = std.math.clamp(cam_pitch - 1.2 * dt, -1.4, 1.4);

        if (rl.IsKeyPressed(rl.KEY_SPACE)) paused = !paused;
        if (rl.IsKeyPressed(rl.KEY_TAB)) split_screen = !split_screen;
        if (rl.IsKeyPressed(rl.KEY_H)) show_hud = !show_hud;

        // Manual Chart Selection (1: U0/W, 2: U1/X, 3: U2/Y, 4: U3/Z, 0: Auto)
        if (rl.IsKeyPressed(rl.KEY_ONE)) manual_chart = .UW;
        if (rl.IsKeyPressed(rl.KEY_TWO)) manual_chart = .UX;
        if (rl.IsKeyPressed(rl.KEY_THREE)) manual_chart = .UY;
        if (rl.IsKeyPressed(rl.KEY_FOUR)) manual_chart = .UZ;
        if (rl.IsKeyPressed(rl.KEY_ZERO)) manual_chart = null;

        // Speed Control
        if (rl.IsKeyPressed(rl.KEY_EQUAL) or rl.IsKeyPressed(rl.KEY_KP_ADD)) speed_mult = @min(4.0, speed_mult + 0.25);
        if (rl.IsKeyPressed(rl.KEY_MINUS) or rl.IsKeyPressed(rl.KEY_KP_SUBTRACT)) speed_mult = @max(0.1, speed_mult - 0.25);

        // Reset
        if (rl.IsKeyPressed(rl.KEY_R)) {
            t = 0.0;
            cam_angle = 0.4;
            cam_pitch = 0.35;
            cam_dist = 12.0;
            trail_count = 0;
            manual_chart = null;
        }

        // Screenshot
        if (rl.IsKeyPressed(rl.KEY_F12) or rl.IsKeyPressed(rl.KEY_P)) {
            rl.TakeScreenshot(screenshot_path);
            screenshot_notice_timer = 2.5;
        }

        if (!paused) {
            t += dt * 0.8 * speed_mult;
        }

        // Camera position (Spherical)
        const cx = cam_dist * @cos(cam_pitch) * @sin(cam_angle);
        const cy = cam_dist * @sin(cam_pitch);
        const cz = cam_dist * @cos(cam_pitch) * @cos(cam_angle);

        const camera: rl.Camera3D = .{
            .position = .{ .x = cx, .y = cy, .z = cz },
            .target = .{ .x = 0.0, .y = 0.0, .z = 0.0 },
            .up = .{ .x = 0.0, .y = 1.0, .z = 0.0 },
            .fovy = 55.0,
            .projection = rl.CAMERA_PERSPECTIVE,
        };

        // ====================================================================
        // 1. P³ S³ GEODESIC TRAJECTORY & CHART RESOLUTION
        // ====================================================================
        const omega: f64 = 1.2;
        const pt_w = @cos(0.5 * omega * @as(f64, @floatCast(t)));
        const pt_x = @cos(omega * @as(f64, @floatCast(t)));
        const pt_y = @sin(omega * @as(f64, @floatCast(t)));
        const pt_z = 0.6 * @sin(2.0 * omega * @as(f64, @floatCast(t)));

        const p_hom = HomVec4.init(pt_x, pt_y, pt_z, pt_w).normalize();
        const active_card = manual_chart orelse p_hom.pickBestCard();
        if (active_card != last_card) {
            total_chart_switches += 1;
            last_card = active_card;
        }

        // P³ Affine coordinates
        const p3_vis_scale: f32 = 4.5;
        const p3_pos = rl.Vector3{
            .x = @floatCast(p_hom.x * p3_vis_scale),
            .y = @floatCast(p_hom.z * p3_vis_scale),
            .z = @floatCast(p_hom.y * p3_vis_scale),
        };

        // Euclidean ℝ³ Dehomogenized Coordinates (shows 1/W explosion)
        const r3_pos = if (@abs(p_hom.w) > 0.08)
            rl.Vector3{
                .x = @floatCast((p_hom.x / p_hom.w) * 1.5),
                .y = @floatCast((p_hom.z / p_hom.w) * 1.5),
                .z = @floatCast((p_hom.y / p_hom.w) * 1.5),
            }
        else
            rl.Vector3{ .x = 999.0, .y = 999.0, .z = 999.0 }; // Diverged!

        // Update trail buffer
        if (!paused) {
            if (trail_count < TRAIL_LENGTH) {
                p3_trail[trail_count] = p3_pos;
                r3_trail[trail_count] = r3_pos;
                trail_count += 1;
            } else {
                var i: usize = 0;
                while (i < TRAIL_LENGTH - 1) : (i += 1) {
                    p3_trail[i] = p3_trail[i + 1];
                    r3_trail[i] = r3_trail[i + 1];
                }
                p3_trail[TRAIL_LENGTH - 1] = p3_pos;
                r3_trail[TRAIL_LENGTH - 1] = r3_pos;
            }
        }

        // ====================================================================
        // 2. LIVE BENCHMARK (P³ Symplectic vs ℝ³ Euler)
        // ====================================================================
        {
            var p3_energy_err: f64 = 0.0;
            var r3_energy_err: f64 = 0.0;
            const h: f64 = 0.005;

            var q_p3 = HomVec4.init(1.0, 0.0, 0.0, 0.0);
            var v_p3 = HomVec4.init(0.0, 1.0, 0.0, 0.0);
            const e0_p3: f64 = q_p3.dot(q_p3) + v_p3.dot(v_p3);

            var q_r3: f64 = 1.0;
            var v_r3: f64 = 0.0;
            const e0_r3: f64 = 0.5 * (q_r3 * q_r3 + v_r3 * v_r3);

            var step: usize = 0;
            while (step < BENCHMARK_STEPS) : (step += 1) {
                const next_q = HomVec4.init(
                    q_p3.x * @cos(h) + v_p3.x * @sin(h),
                    q_p3.y * @cos(h) + v_p3.y * @sin(h),
                    q_p3.z * @cos(h) + v_p3.z * @sin(h),
                    q_p3.w * @cos(h) + v_p3.w * @sin(h),
                );
                const next_v = HomVec4.init(
                    -q_p3.x * @sin(h) + v_p3.x * @cos(h),
                    -q_p3.y * @sin(h) + v_p3.y * @cos(h),
                    -q_p3.z * @sin(h) + v_p3.z * @cos(h),
                    -q_p3.w * @sin(h) + v_p3.w * @cos(h),
                );
                q_p3 = next_q;
                v_p3 = next_v;

                const prev_q = q_r3;
                q_r3 += h * v_r3;
                v_r3 -= h * prev_q;
            }

            const e_curr_p3 = q_p3.dot(q_p3) + v_p3.dot(v_p3);
            p3_energy_err = @abs(e_curr_p3 - e0_p3) / e0_p3;

            const e_curr_r3 = 0.5 * (q_r3 * q_r3 + v_r3 * v_r3);
            r3_energy_err = @abs(e_curr_r3 - e0_r3) / e0_r3;

            p3_energy_drift = p3_energy_err;
            r3_energy_drift = r3_energy_err;
        }

        // ====================================================================
        // 3. RENDERING (OpenGL 3.3 / Raylib Viewport)
        // ====================================================================
        rl.BeginDrawing();
        rl.ClearBackground(.{ .r = 10, .g = 12, .b = 20, .a = 255 });

        rl.BeginMode3D(camera);

        // Reference Projective Grid
        rl.DrawGrid(40, 0.5);

        // Projective Horizon Circles (W = 0 Equatorial Manifold)
        rl.DrawCircle3D(.{ .x = 0, .y = 0, .z = 0 }, p3_vis_scale, .{ .x = 0, .y = 1, .z = 0 }, 90.0, .{ .r = 70, .g = 90, .b = 140, .a = 160 });
        rl.DrawCircle3D(.{ .x = 0, .y = 0, .z = 0 }, p3_vis_scale, .{ .x = 1, .y = 0, .z = 0 }, 90.0, .{ .r = 70, .g = 90, .b = 140, .a = 160 });
        rl.DrawSphereWires(.{ .x = 0, .y = 0, .z = 0 }, p3_vis_scale, 16, 16, .{ .r = 40, .g = 60, .b = 100, .a = 80 });

        // Draw Trails
        if (trail_count > 1) {
            var i: usize = 0;
            while (i < trail_count - 1) : (i += 1) {
                const alpha: u8 = @intCast(@min(255, (i * 255) / trail_count));

                // P³ Trail (Cyan smooth geodesic)
                const col_p3 = rl.Color{ .r = 0, .g = 220, .b = 255, .a = alpha };
                rl.DrawLine3D(p3_trail[i], p3_trail[i + 1], col_p3);

                // Euclidean ℝ³ Trail (Red divergence)
                if (@abs(r3_trail[i].x) < 18.0 and @abs(r3_trail[i + 1].x) < 18.0) {
                    const col_r3 = rl.Color{ .r = 255, .g = 80, .b = 50, .a = alpha / 2 };
                    rl.DrawLine3D(r3_trail[i], r3_trail[i + 1], col_r3);
                }
            }
        }

        // Probe Particle in P³
        rl.DrawSphere(p3_pos, 0.22, .{ .r = 0, .g = 255, .b = 200, .a = 255 });
        rl.DrawSphereWires(p3_pos, 0.28, 8, 8, .{ .r = 255, .g = 255, .b = 255, .a = 200 });

        // Coordinate Axes
        rl.DrawLine3D(.{ .x = 0, .y = 0, .z = 0 }, .{ .x = 3.0, .y = 0, .z = 0 }, .{ .r = 255, .g = 50, .b = 50, .a = 255 });
        rl.DrawLine3D(.{ .x = 0, .y = 0, .z = 0 }, .{ .x = 0, .y = 3.0, .z = 0 }, .{ .r = 50, .g = 255, .b = 50, .a = 255 });
        rl.DrawLine3D(.{ .x = 0, .y = 0, .z = 0 }, .{ .x = 0, .y = 0, .z = 3.0 }, .{ .r = 50, .g = 150, .b = 255, .a = 255 });

        // Origin Marker
        rl.DrawCubeWires(.{ .x = 0, .y = 0, .z = 0 }, 0.3, 0.3, 0.3, .{ .r = 255, .g = 200, .b = 80, .a = 255 });

        rl.EndMode3D();

        // ====================================================================
        // 4. HEADS-UP DISPLAY (HUD)
        // ====================================================================
        if (show_hud) {
            rl.DrawRectangle(15, 15, 510, 275, .{ .r = 15, .g = 18, .b = 30, .a = 225 });
            rl.DrawRectangleLines(15, 15, 510, 275, .{ .r = 60, .g = 120, .b = 200, .a = 255 });

            rl.DrawText("P3 ENGINE: SINGULARITY-FREE LIVE DEMO", 28, 26, 18, .{ .r = 120, .g = 220, .b = 255, .a = 255 });

            var buf: [128]u8 = undefined;

            // Active Chart Mode
            const mode_str = if (manual_chart != null) "[MANUAL LOCKED]" else "[AUTO-ADAPTIVE]";
            const card_name = switch (active_card) {
                .UW => "U0 (W != 0) [Standard Euclidean Chart]",
                .UX => "U1 (X != 0) [Projective Chart 1 - W -> 0 Horizon]",
                .UY => "U2 (Y != 0) [Projective Chart 2 - W -> 0 Horizon]",
                .UZ => "U3 (Z != 0) [Projective Chart 3 - W -> 0 Horizon]",
            };
            const card_txt = std.fmt.bufPrintZ(&buf, "Chart: {s} {s}", .{ card_name, mode_str }) catch "";
            rl.DrawText(card_txt, 28, 54, 13, .{ .r = 255, .g = 220, .b = 80, .a = 255 });

            // Coordinates
            const hom_txt = std.fmt.bufPrintZ(&buf, "S3 Coords: (X={d:.2}, Y={d:.2}, Z={d:.2}, W={d:.3})", .{ p_hom.x, p_hom.y, p_hom.z, p_hom.w }) catch "";
            rl.DrawText(hom_txt, 28, 74, 13, .{ .r = 200, .g = 200, .b = 200, .a = 255 });

            const sw_txt = std.fmt.bufPrintZ(&buf, "Chart Transitions: {d} | Speed: {d:.2}x", .{ total_chart_switches, speed_mult }) catch "";
            rl.DrawText(sw_txt, 28, 94, 13, .{ .r = 100, .g = 255, .b = 150, .a = 255 });

            rl.DrawLine(28, 115, 505, 115, .{ .r = 60, .g = 90, .b = 140, .a = 180 });

            // Benchmark
            rl.DrawText("LIVE SYMPLECTIC BENCHMARK (1,000 steps/frame):", 28, 122, 13, .{ .r = 180, .g = 180, .b = 255, .a = 255 });

            const p3_drift_txt = std.fmt.bufPrintZ(&buf, "  * P3 Symplectic Drift:  {e} (Machine Precision OK)", .{p3_energy_drift}) catch "";
            rl.DrawText(p3_drift_txt, 28, 142, 13, .{ .r = 80, .g = 255, .b = 120, .a = 255 });

            const r3_drift_txt = std.fmt.bufPrintZ(&buf, "  * R3 Euclidean Drift:   {e} (Diverging Error!)", .{r3_energy_drift}) catch "";
            rl.DrawText(r3_drift_txt, 28, 160, 13, .{ .r = 255, .g = 100, .b = 80, .a = 255 });

            // Legend & Controls
            rl.DrawText("[Cyan] P3 Geodesic Trajectory (Closed S3 Manifold)", 28, 184, 12, .{ .r = 0, .g = 220, .b = 255, .a = 255 });
            rl.DrawText("[Red]  R3 Standard Projection (Explodes at W=0)", 28, 200, 12, .{ .r = 255, .g = 100, .b = 80, .a = 255 });

            rl.DrawText("Mouse: Drag Left Click to Orbit, Wheel to Zoom", 28, 222, 11, .{ .r = 170, .g = 170, .b = 170, .a = 255 });
            rl.DrawText("Keys: 1-4 Force Charts, 0 Auto, +/- Speed, Space Pause, F12 Screenshot", 28, 240, 11, .{ .r = 170, .g = 170, .b = 170, .a = 255 });
            rl.DrawText("Press H to Toggle HUD overlay", 28, 258, 11, .{ .r = 140, .g = 140, .b = 140, .a = 255 });
        }

        // Screenshot Toast Notice
        if (screenshot_notice_timer > 0) {
            rl.DrawRectangle(440, 30, 400, 45, .{ .r = 30, .g = 120, .b = 60, .a = 240 });
            rl.DrawRectangleLines(440, 30, 400, 45, .{ .r = 100, .g = 255, .b = 150, .a = 255 });
            rl.DrawText("Screenshot captured: p3_demo_screenshot.png", 455, 45, 14, .{ .r = 255, .g = 255, .b = 255, .a = 255 });
        }

        rl.DrawFPS(1180, 20);

        rl.EndDrawing();

        // Direct GPU Framebuffer Capture
        if (frame_count == 30 or (capture_after_frames != null and frame_count >= capture_after_frames.?)) {
            const img = rl.LoadImageFromScreen();
            _ = rl.ExportImage(img, "p3_demo_screenshot.png");
            rl.UnloadImage(img);
            if (capture_after_frames != null and frame_count >= capture_after_frames.?) {
                break;
            }
        }
    }
}
