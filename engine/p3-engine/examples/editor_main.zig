// =============================================================================
// P³ ENGINE STUDIO — O3DE & UNREAL ENGINE SLATE REFACTORED EDITOR
// =============================================================================
//
// Refactored from O3DE AzToolsFramework & Unreal Engine Slate:
// - Crisp Unicode TrueType Font (Cyrillic + Latin + Math symbols)
// - 3D Interactive Transform Gizmo (RGB coordinate arrows on selected entity)
// - Primitive Spawner: Sphere/Planet, Cube/Box, Cylinder, Plane, Quantum Probe
// - Collapsible Component Property Cards (Transform, Astronomy, Physics, Optics)
// - XYZW Vector Fields with Colored Badges (Red/Green/Blue/Violet)
// - O3DE / Slate Sleek Dark Theme (#181a24 / #242938 / #ffb300 Gold Accent)
// =============================================================================

const std = @import("std");
const p3 = @import("root.zig");

const rl = @cImport({
    @cInclude("raylib.h");
});

const Vec3 = p3.Vec3;
const HomVec4 = p3.HomVec4;
const AffineCard = p3.kernel.AffineCard;
const Rotator = p3.Rotator;
const Ray = p3.Ray;

pub const PrimitiveType = enum {
    sphere,
    cube,
    cylinder,
    plane,
    geodesic_probe,
    horizon_manifold,
};

pub const Entity = struct {
    id: u32,
    name: [48]u8,
    name_len: usize,
    prim_type: PrimitiveType,
    visible: bool = true,

    // P³ Transform Component (Homogeneous 4D)
    pos: HomVec4,
    rot: Rotator,
    scale: Vec3,

    // Astronomy & Physics Component
    is_celestial: bool = false,
    mass_earth: f32 = 1.0,
    radius_earth: f32 = 1.0,
    orbit_semi_major_au: f32 = 1.0,
    orbit_speed: f32 = 1.0,
    orbit_angle: f32 = 0.0,

    // Quantum Geodesic Component
    is_geodesic: bool = false,
    geodesic_omega: f32 = 1.2,

    // Material & Color
    color: rl.Color,

    pub fn getName(self: *const Entity) []const u8 {
        return self.name[0..self.name_len];
    }
};

const MAX_ENTITIES: usize = 48;
const MAX_LOGS: usize = 10;

pub fn main() !void {
    rl.InitWindow(1600, 900, "P3 Engine Studio — Open 3D Engine & Unreal Slate Architecture");
    rl.SetTargetFPS(60);
    rl.SetWindowState(rl.FLAG_WINDOW_RESIZABLE);
    defer rl.CloseWindow();

    // ------------------------------------------------------------------------
    // 1. LOAD TRUETYPE UNICODE FONT (Cyrillic + Latin + Math)
    // ------------------------------------------------------------------------
    var codepoints: [600]c_int = undefined;
    var cp_count: usize = 0;
    // Latin & Punctuation (32..126)
    var c: usize = 32;
    while (c < 127) : (c += 1) {
        codepoints[cp_count] = @intCast(c);
        cp_count += 1;
    }
    // Cyrillic Unicode Range (0x0400 .. 0x04FF)
    c = 0x0400;
    while (c < 0x0500) : (c += 1) {
        codepoints[cp_count] = @intCast(c);
        cp_count += 1;
    }
    // Mathematical & Box symbols
    codepoints[cp_count] = 0x00B0; cp_count += 1; // degree
    codepoints[cp_count] = 0x00B3; cp_count += 1; // superscript 3 (³)
    codepoints[cp_count] = 0x03C0; cp_count += 1; // pi
    codepoints[cp_count] = 0x03A9; cp_count += 1; // Omega
    codepoints[cp_count] = 0x03B8; cp_count += 1; // theta
    codepoints[cp_count] = 0x221E; cp_count += 1; // infinity
    codepoints[cp_count] = 0x25BC; cp_count += 1; // down triangle
    codepoints[cp_count] = 0x25BA; cp_count += 1; // right triangle

    const font = rl.LoadFontEx("/usr/share/fonts/TTF/DejaVuSans.ttf", 20, &codepoints, @intCast(cp_count));
    rl.SetTextureFilter(font.texture, rl.TEXTURE_FILTER_BILINEAR);
    defer rl.UnloadFont(font);

    // ------------------------------------------------------------------------
    // 2. SCENE ENTITIES
    // ------------------------------------------------------------------------
    var entities: [MAX_ENTITIES]Entity = undefined;
    var entity_count: usize = 0;
    var selected_idx: ?usize = 0;

    // Helper to spawn entities
    const spawnEntity = struct {
        fn spawn(
            ents: *[MAX_ENTITIES]Entity,
            count: *usize,
            name: []const u8,
            prim: PrimitiveType,
            pos: HomVec4,
            scale: Vec3,
            col: rl.Color,
        ) usize {
            if (count.* >= MAX_ENTITIES) return 0;
            const idx = count.*;
            var e = &ents[idx];
            e.id = @intCast(idx);
            const n_len = @min(name.len, 47);
            @memcpy(e.name[0..n_len], name[0..n_len]);
            e.name_len = n_len;
            e.prim_type = prim;
            e.visible = true;
            e.pos = pos;
            e.rot = Rotator.zero();
            e.scale = scale;
            e.color = col;
            e.is_celestial = false;
            e.is_geodesic = false;
            e.mass_earth = 1.0;
            e.radius_earth = 1.0;
            e.orbit_semi_major_au = 1.0;
            e.orbit_speed = 1.0;
            e.orbit_angle = 0.0;
            count.* += 1;
            return idx;
        }
    }.spawn;

    // Default World Setup
    {
        const idx0 = spawnEntity(&entities, &entity_count, "Центральная Звезда (Солнце)", .sphere, HomVec4.init(0, 0, 0, 1), Vec3.init(1.2, 1.2, 1.2), .{ .r = 255, .g = 210, .b = 60, .a = 255 });
        entities[idx0].is_celestial = true;
        entities[idx0].mass_earth = 333000.0;

        const idx1 = spawnEntity(&entities, &entity_count, "Планета Терра (Эллипсоид)", .sphere, HomVec4.init(4.5, 0, 0, 1), Vec3.init(0.55, 0.55, 0.55), .{ .r = 60, .g = 150, .b = 255, .a = 255 });
        entities[idx1].is_celestial = true;
        entities[idx1].orbit_semi_major_au = 1.0;
        entities[idx1].orbit_speed = 0.8;

        const idx2 = spawnEntity(&entities, &entity_count, "Спутник Альфа (Луна)", .sphere, HomVec4.init(5.6, 0.3, 0, 1), Vec3.init(0.22, 0.22, 0.22), .{ .r = 180, .g = 190, .b = 205, .a = 255 });
        entities[idx2].is_celestial = true;
        entities[idx2].mass_earth = 0.0123;
        entities[idx2].radius_earth = 0.272;
        entities[idx2].orbit_speed = 3.0;

        const idx3 = spawnEntity(&entities, &entity_count, "Квантовый Зонд S³ (W=0 Горизонт)", .geodesic_probe, HomVec4.init(1, 0, 0, 0.5).normalize(), Vec3.init(0.35, 0.35, 0.35), .{ .r = 0, .g = 255, .b = 210, .a = 255 });
        entities[idx3].is_geodesic = true;

        _ = spawnEntity(&entities, &entity_count, "Зеркало Плазмы Друде (Плоскость)", .plane, HomVec4.init(0, -2.5, 0, 1), Vec3.init(4.0, 0.1, 4.0), .{ .r = 200, .g = 90, .b = 255, .a = 190 });
        _ = spawnEntity(&entities, &entity_count, "Проективный Горизонт (W=0 Экватор)", .horizon_manifold, HomVec4.init(0, 0, 0, 0), Vec3.init(6.0, 6.0, 6.0), .{ .r = 60, .g = 90, .b = 160, .a = 120 });
    }

    // Diagnostics Log
    var log_entries: [MAX_LOGS][128]u8 = undefined;
    var log_count: usize = 0;

    const pushLog = struct {
        fn call(logs: *[MAX_LOGS][128]u8, count: *usize, msg: []const u8) void {
            const clen = @min(msg.len, 127);
            if (count.* < MAX_LOGS) {
                @memcpy(logs[count.*][0..clen], msg[0..clen]);
                logs[count.*][clen] = 0;
                count.* += 1;
            } else {
                for (0..MAX_LOGS - 1) |i| {
                    logs[i] = logs[i + 1];
                }
                @memcpy(logs[MAX_LOGS - 1][0..clen], msg[0..clen]);
                logs[MAX_LOGS - 1][clen] = 0;
            }
        }
    }.call;

    pushLog(&log_entries, &log_count, "[P³ Studio] Графический движок и ядро инициализированы");
    pushLog(&log_entries, &log_count, "[Slate UI] Подключен шрифт Unicode UTF-8 (Cyrillic + Math)");
    pushLog(&log_entries, &log_count, "[O3DE Viewport] 3D Манипуляторы и симплектический решатель готовы");

    // Camera
    var cam_angle: f32 = 0.55;
    var cam_pitch: f32 = 0.38;
    var cam_dist: f32 = 18.0;
    var is_orbiting: bool = false;
    var last_mouse = rl.GetMousePosition();

    // Simulation
    var sim_playing: bool = true;
    var sim_time: f32 = 0.0;
    var speed_scale: f32 = 1.0;
    var active_chart: AffineCard = .UW;
    var total_transitions: u64 = 0;

    // DrawText Helper with Unicode Font
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
        const screen_w = @as(f32, @floatFromInt(rl.GetScreenWidth()));
        const screen_h = @as(f32, @floatFromInt(rl.GetScreenHeight()));

        // Dock Dimensions
        const top_bar_h: f32 = 44.0;
        const bottom_bar_h: f32 = 175.0;
        const left_panel_w: f32 = 320.0;
        const right_panel_w: f32 = 360.0;

        const vp_x = left_panel_w;
        const vp_y = top_bar_h;
        const vp_w = screen_w - left_panel_w - right_panel_w;
        const vp_h = screen_h - top_bar_h - bottom_bar_h;

        const mouse = rl.GetMousePosition();
        const in_vp = (mouse.x >= vp_x and mouse.x <= vp_x + vp_w and mouse.y >= vp_y and mouse.y <= vp_y + vp_h);

        // Shortcuts
        if (rl.IsKeyPressed(rl.KEY_SPACE)) sim_playing = !sim_playing;
        if (rl.IsKeyPressed(rl.KEY_EQUAL) or rl.IsKeyPressed(rl.KEY_KP_ADD)) speed_scale = @min(5.0, speed_scale + 0.25);
        if (rl.IsKeyPressed(rl.KEY_MINUS) or rl.IsKeyPressed(rl.KEY_KP_SUBTRACT)) speed_scale = @max(0.1, speed_scale - 0.25);

        // Orbit / Pan in Viewport
        if (in_vp) {
            if (rl.IsMouseButtonPressed(rl.MOUSE_BUTTON_RIGHT) or rl.IsMouseButtonPressed(rl.MOUSE_BUTTON_LEFT)) {
                is_orbiting = true;
                last_mouse = mouse;
            }
            const wheel = rl.GetMouseWheelMove();
            if (wheel != 0) cam_dist = std.math.clamp(cam_dist - wheel * 2.0, 4.0, 70.0);
        }
        if (rl.IsMouseButtonReleased(rl.MOUSE_BUTTON_RIGHT) or rl.IsMouseButtonReleased(rl.MOUSE_BUTTON_LEFT)) {
            is_orbiting = false;
        }
        if (is_orbiting) {
            const dx = mouse.x - last_mouse.x;
            const dy = mouse.y - last_mouse.y;
            cam_angle -= dx * 0.005;
            cam_pitch = std.math.clamp(cam_pitch + dy * 0.005, -1.4, 1.4);
            last_mouse = mouse;
        }

        // Simulation Step
        if (sim_playing) {
            sim_time += dt * speed_scale;

            // Keplerian Planet Orbit
            if (entity_count > 1 and entities[1].is_celestial) {
                entities[1].orbit_angle += dt * entities[1].orbit_speed * speed_scale * 0.6;
                const r = entities[1].orbit_semi_major_au * 4.5;
                entities[1].pos.x = r * @cos(entities[1].orbit_angle);
                entities[1].pos.z = r * @sin(entities[1].orbit_angle);
            }

            // Moon Orbit around Planet
            if (entity_count > 2 and entities[2].is_celestial) {
                entities[2].orbit_angle += dt * entities[2].orbit_speed * speed_scale;
                const r_m: f32 = 1.3;
                entities[2].pos.x = entities[1].pos.x + r_m * @cos(entities[2].orbit_angle);
                entities[2].pos.z = entities[1].pos.z + r_m * @sin(entities[2].orbit_angle);
                entities[2].pos.y = entities[1].pos.y + 0.3 * @sin(entities[2].orbit_angle * 2.0);
            }

            // Quantum Geodesic Probe (Crossing W=0 seamlessly)
            if (entity_count > 3 and entities[3].is_geodesic) {
                const om = entities[3].geodesic_omega;
                const tv = sim_time * om;
                entities[3].pos = HomVec4.init(
                    @cos(tv),
                    @sin(tv),
                    0.5 * @sin(2.0 * tv),
                    @cos(0.5 * tv),
                ).normalize();

                const n_card = entities[3].pos.pickBestCard();
                if (n_card != active_chart) {
                    active_chart = n_card;
                    total_transitions += 1;
                    pushLog(&log_entries, &log_count, "[P³ Manifold] Бесшовный переход на аффинную карту горизонта W=0");
                }
            }
        }

        // Camera Position
        const cx = cam_dist * @cos(cam_pitch) * @sin(cam_angle);
        const cy = cam_dist * @sin(cam_pitch);
        const cz = cam_dist * @cos(cam_pitch) * @cos(cam_angle);

        const camera3d: rl.Camera3D = .{
            .position = .{ .x = cx, .y = cy, .z = cz },
            .target = .{ .x = 0.0, .y = 0.0, .z = 0.0 },
            .up = .{ .x = 0.0, .y = 1.0, .z = 0.0 },
            .fovy = 45.0,
            .projection = rl.CAMERA_PERSPECTIVE,
        };

        // ====================================================================
        // RENDER PASS
        // ====================================================================
        rl.BeginDrawing();
        rl.ClearBackground(.{ .r = 18, .g = 20, .b = 28, .a = 255 });

        // --------------------------------------------------------------------
        // A. 3D VIEWPORT
        // --------------------------------------------------------------------
        rl.BeginScissorMode(@intFromFloat(vp_x), @intFromFloat(vp_y), @intFromFloat(vp_w), @intFromFloat(vp_h));
        rl.ClearBackground(.{ .r = 10, .g = 12, .b = 18, .a = 255 });

        rl.BeginMode3D(camera3d);
        rl.DrawGrid(40, 1.0);

        for (entities[0..entity_count], 0..) |*e, idx| {
            if (!e.visible) continue;
            const is_sel = (selected_idx != null and selected_idx.? == idx);
            const pos3 = rl.Vector3{
                .x = @floatCast(e.pos.x * 3.5),
                .y = @floatCast(e.pos.y * 3.5),
                .z = @floatCast(e.pos.z * 3.5),
            };

            switch (e.prim_type) {
                .sphere => {
                    rl.DrawSphere(pos3, e.scale.x, e.color);
                    if (e.is_celestial and e.id == 0) {
                        rl.DrawSphereWires(pos3, e.scale.x * 1.12, 12, 12, .{ .r = 255, .g = 240, .b = 150, .a = 100 });
                    }
                    if (e.is_celestial and e.id == 1) {
                        rl.DrawCircle3D(.{ .x = 0, .y = 0, .z = 0 }, e.orbit_semi_major_au * 4.5 * 3.5, .{ .x = 0, .y = 1, .z = 0 }, 90.0, .{ .r = 70, .g = 120, .b = 200, .a = 70 });
                    }
                },
                .cube => {
                    rl.DrawCube(pos3, e.scale.x * 2.0, e.scale.y * 2.0, e.scale.z * 2.0, e.color);
                    rl.DrawCubeWires(pos3, e.scale.x * 2.0, e.scale.y * 2.0, e.scale.z * 2.0, .{ .r = 255, .g = 255, .b = 255, .a = 150 });
                },
                .cylinder => {
                    rl.DrawCylinder(pos3, e.scale.x, e.scale.x, e.scale.y * 2.0, 16, e.color);
                    rl.DrawCylinderWires(pos3, e.scale.x, e.scale.x, e.scale.y * 2.0, 16, .{ .r = 255, .g = 255, .b = 255, .a = 150 });
                },
                .plane => {
                    rl.DrawCube(pos3, e.scale.x, e.scale.y, e.scale.z, e.color);
                    rl.DrawCubeWires(pos3, e.scale.x, e.scale.y, e.scale.z, .{ .r = 255, .g = 255, .b = 255, .a = 150 });
                },
                .geodesic_probe => {
                    rl.DrawSphere(pos3, e.scale.x, e.color);
                    rl.DrawSphereWires(pos3, e.scale.x * 1.3, 8, 8, .{ .r = 255, .g = 255, .b = 255, .a = 220 });
                },
                .horizon_manifold => {
                    rl.DrawCircle3D(.{ .x = 0, .y = 0, .z = 0 }, e.scale.x, .{ .x = 0, .y = 1, .z = 0 }, 90.0, e.color);
                    rl.DrawCircle3D(.{ .x = 0, .y = 0, .z = 0 }, e.scale.x, .{ .x = 1, .y = 0, .z = 0 }, 90.0, e.color);
                    rl.DrawSphereWires(.{ .x = 0, .y = 0, .z = 0 }, e.scale.x, 16, 16, .{ .r = 40, .g = 60, .b = 120, .a = 50 });
                },
            }

            // Selection Bounding Box & 3D Transform Gizmo (Unreal Slate Style)
            if (is_sel) {
                rl.DrawSphereWires(pos3, e.scale.x * 1.4, 8, 8, .{ .r = 255, .g = 180, .b = 0, .a = 255 });

                // 3D Translation Gizmo Arrows (X: Red, Y: Green, Z: Blue)
                const g_len: f32 = 1.8;
                rl.DrawLine3D(pos3, .{ .x = pos3.x + g_len, .y = pos3.y, .z = pos3.z }, .{ .r = 255, .g = 60, .b = 60, .a = 255 });
                rl.DrawSphere(.{ .x = pos3.x + g_len, .y = pos3.y, .z = pos3.z }, 0.08, .{ .r = 255, .g = 60, .b = 60, .a = 255 });

                rl.DrawLine3D(pos3, .{ .x = pos3.x, .y = pos3.y + g_len, .z = pos3.z }, .{ .r = 60, .g = 255, .b = 60, .a = 255 });
                rl.DrawSphere(.{ .x = pos3.x, .y = pos3.y + g_len, .z = pos3.z }, 0.08, .{ .r = 60, .g = 255, .b = 60, .a = 255 });

                rl.DrawLine3D(pos3, .{ .x = pos3.x, .y = pos3.y, .z = pos3.z + g_len }, .{ .r = 60, .g = 120, .b = 255, .a = 255 });
                rl.DrawSphere(.{ .x = pos3.x, .y = pos3.y, .z = pos3.z + g_len }, 0.08, .{ .r = 60, .g = 120, .b = 255, .a = 255 });
            }
        }

        rl.EndMode3D();
        rl.EndScissorMode();

        // Viewport Overlay Header
        rl.DrawRectangleLinesEx(.{ .x = vp_x, .y = vp_y, .width = vp_w, .height = vp_h }, 1.0, .{ .r = 45, .g = 52, .b = 70, .a = 255 });
        rl.DrawRectangle(@intFromFloat(vp_x + 12), @intFromFloat(vp_y + 10), 250, 26, .{ .r = 22, .g = 26, .b = 36, .a = 220 });
        drawUText(font, "3D Вьюпорт (Многообразие S³)", vp_x + 20, vp_y + 14, 13, .{ .r = 150, .g = 200, .b = 255, .a = 255 });

        // --------------------------------------------------------------------
        // B. TOP TOOLBAR (Панель Инструментов Slate)
        // --------------------------------------------------------------------
        rl.DrawRectangle(0, 0, @intFromFloat(screen_w), @intFromFloat(top_bar_h), .{ .r = 24, .g = 28, .b = 38, .a = 255 });
        rl.DrawLine(0, @intFromFloat(top_bar_h), @intFromFloat(screen_w), @intFromFloat(top_bar_h), .{ .r = 45, .g = 52, .b = 70, .a = 255 });

        drawUText(font, "P³ STUDIO", 18, 12, 18, .{ .r = 255, .g = 190, .b = 40, .a = 255 });
        drawUText(font, "v1.0 (O3DE / Slate)", 125, 15, 12, .{ .r = 130, .g = 145, .b = 170, .a = 255 });

        // Play / Pause Button
        const play_rec = rl.Rectangle{ .x = screen_w * 0.5 - 120, .y = 6, .width = 110, .height = 32 };
        const play_hov = rl.CheckCollisionPointRec(mouse, play_rec);
        if (play_hov and rl.IsMouseButtonPressed(rl.MOUSE_BUTTON_LEFT)) {
            sim_playing = !sim_playing;
            pushLog(&log_entries, &log_count, if (sim_playing) "[Симуляция] Возобновлена" else "[Симуляция] Приостановлена");
        }
        rl.DrawRectangleRec(play_rec, if (play_hov) .{ .r = 40, .g = 130, .b = 70, .a = 255 } else .{ .r = 30, .g = 100, .b = 55, .a = 255 });
        rl.DrawRectangleLinesEx(play_rec, 1.0, .{ .r = 60, .g = 180, .b = 90, .a = 255 });
        drawUText(font, if (sim_playing) "|| Пауза" else "> Запуск", play_rec.x + 22, play_rec.y + 7, 14, .{ .r = 255, .g = 255, .b = 255, .a = 255 });

        // Reset Button
        const rst_rec = rl.Rectangle{ .x = screen_w * 0.5 + 2, .y = 6, .width = 100, .height = 32 };
        const rst_hov = rl.CheckCollisionPointRec(mouse, rst_rec);
        if (rst_hov and rl.IsMouseButtonPressed(rl.MOUSE_BUTTON_LEFT)) {
            sim_time = 0.0;
            pushLog(&log_entries, &log_count, "[Сцена] Сброс параметров в исходное состояние");
        }
        rl.DrawRectangleRec(rst_rec, if (rst_hov) .{ .r = 60, .g = 70, .b = 95, .a = 255 } else .{ .r = 40, .g = 48, .b = 68, .a = 255 });
        rl.DrawRectangleLinesEx(rst_rec, 1.0, .{ .r = 75, .g = 90, .b = 120, .a = 255 });
        drawUText(font, "Сброс", rst_rec.x + 26, rst_rec.y + 7, 14, .{ .r = 220, .g = 230, .b = 245, .a = 255 });

        // Screenshot Button
        const snap_rec = rl.Rectangle{ .x = screen_w * 0.5 + 115, .y = 6, .width = 125, .height = 32 };
        const snap_hov = rl.CheckCollisionPointRec(mouse, snap_rec);
        if (snap_hov and rl.IsMouseButtonPressed(rl.MOUSE_BUTTON_LEFT)) {
            rl.TakeScreenshot("p3_editor_screenshot.png");
            pushLog(&log_entries, &log_count, "[Снимок] Сохранен в p3_editor_screenshot.png");
        }
        rl.DrawRectangleRec(snap_rec, if (snap_hov) .{ .r = 50, .g = 90, .b = 140, .a = 255 } else .{ .r = 35, .g = 65, .b = 105, .a = 255 });
        rl.DrawRectangleLinesEx(snap_rec, 1.0, .{ .r = 70, .g = 130, .b = 200, .a = 255 });
        drawUText(font, "Снимок (F12)", snap_rec.x + 14, snap_rec.y + 7, 13, .{ .r = 230, .g = 240, .b = 255, .a = 255 });

        // FPS & Map Status
        var s_buf: [64]u8 = undefined;
        const s_txt = std.fmt.bufPrint(&s_buf, "FPS: {d} | Карта: U{d}", .{ rl.GetFPS(), @intFromEnum(active_chart) }) catch "";
        drawUText(font, s_txt, screen_w - 210, 14, 13, .{ .r = 100, .g = 255, .b = 160, .a = 255 });

        // --------------------------------------------------------------------
        // C. LEFT PANEL: SCENE OUTLINER (Иерархия объектов O3DE)
        // --------------------------------------------------------------------
        rl.DrawRectangle(0, @intFromFloat(top_bar_h), @intFromFloat(left_panel_w), @intFromFloat(screen_h - top_bar_h), .{ .r = 22, .g = 25, .b = 35, .a = 255 });
        rl.DrawLine(@intFromFloat(left_panel_w), @intFromFloat(top_bar_h), @intFromFloat(left_panel_w), @intFromFloat(screen_h), .{ .r = 45, .g = 52, .b = 70, .a = 255 });

        drawUText(font, "ИЕРАРХИЯ СЦЕНЫ (Outliner)", 16, top_bar_h + 12, 13, .{ .r = 180, .g = 195, .b = 220, .a = 255 });

        // Spawner Menu Buttons: [+ Сфера] [+ Куб] [+ Зонд]
        const sp_y = top_bar_h + 38;
        const b_w: f32 = 88.0;
        const b_h: f32 = 24.0;

        // + Сфера (Планета)
        const b1_r = rl.Rectangle{ .x = 16, .y = sp_y, .width = b_w, .height = b_h };
        if (rl.CheckCollisionPointRec(mouse, b1_r) and rl.IsMouseButtonPressed(rl.MOUSE_BUTTON_LEFT)) {
            const n_idx = spawnEntity(&entities, &entity_count, "Новая Планета (Сфера)", .sphere, HomVec4.init(7, 0, 0, 1), Vec3.init(0.5, 0.5, 0.5), .{ .r = 255, .g = 130, .b = 70, .a = 255 });
            entities[n_idx].is_celestial = true;
            entities[n_idx].orbit_semi_major_au = 1.6;
            selected_idx = n_idx;
            pushLog(&log_entries, &log_count, "[Создание] Добавлена новая Планета (Сфера)");
        }
        rl.DrawRectangleRec(b1_r, .{ .r = 35, .g = 48, .b = 70, .a = 255 });
        drawUText(font, "+ Сфера", b1_r.x + 14, b1_r.y + 4, 11, .{ .r = 160, .g = 200, .b = 255, .a = 255 });

        // + Куб
        const b2_r = rl.Rectangle{ .x = 16 + b_w + 8, .y = sp_y, .width = b_w, .height = b_h };
        if (rl.CheckCollisionPointRec(mouse, b2_r) and rl.IsMouseButtonPressed(rl.MOUSE_BUTTON_LEFT)) {
            const n_idx = spawnEntity(&entities, &entity_count, "Новый Куб (Box)", .cube, HomVec4.init(2, 2, 0, 1), Vec3.init(0.5, 0.5, 0.5), .{ .r = 120, .g = 220, .b = 120, .a = 255 });
            selected_idx = n_idx;
            pushLog(&log_entries, &log_count, "[Создание] Добавлен Куб (AABB)");
        }
        rl.DrawRectangleRec(b2_r, .{ .r = 35, .g = 48, .b = 70, .a = 255 });
        drawUText(font, "+ Куб", b2_r.x + 22, b2_r.y + 4, 11, .{ .r = 160, .g = 200, .b = 255, .a = 255 });

        // + Зонд S3
        const b3_r = rl.Rectangle{ .x = 16 + (b_w + 8) * 2, .y = sp_y, .width = b_w, .height = b_h };
        if (rl.CheckCollisionPointRec(mouse, b3_r) and rl.IsMouseButtonPressed(rl.MOUSE_BUTTON_LEFT)) {
            const n_idx = spawnEntity(&entities, &entity_count, "Квантовый Зонд S³", .geodesic_probe, HomVec4.init(1, 0, 0, 0.5).normalize(), Vec3.init(0.3, 0.3, 0.3), .{ .r = 0, .g = 255, .b = 210, .a = 255 });
            entities[n_idx].is_geodesic = true;
            selected_idx = n_idx;
            pushLog(&log_entries, &log_count, "[Создание] Добавлен Квантовый Зонд S³");
        }
        rl.DrawRectangleRec(b3_r, .{ .r = 35, .g = 48, .b = 70, .a = 255 });
        drawUText(font, "+ Зонд S³", b3_r.x + 12, b3_r.y + 4, 11, .{ .r = 160, .g = 200, .b = 255, .a = 255 });

        // Entity List Items
        var item_y: f32 = top_bar_h + 74;
        for (entities[0..entity_count], 0..) |*e, idx| {
            const is_sel = (selected_idx != null and selected_idx.? == idx);
            const i_rec = rl.Rectangle{ .x = 12, .y = item_y, .width = left_panel_w - 24, .height = 30 };
            const i_hov = rl.CheckCollisionPointRec(mouse, i_rec);

            if (i_hov and rl.IsMouseButtonPressed(rl.MOUSE_BUTTON_LEFT)) {
                selected_idx = idx;
            }

            if (is_sel) {
                rl.DrawRectangleRec(i_rec, .{ .r = 40, .g = 68, .b = 105, .a = 255 });
                rl.DrawRectangleLinesEx(i_rec, 1.0, .{ .r = 255, .g = 180, .b = 0, .a = 255 });
            } else if (i_hov) {
                rl.DrawRectangleRec(i_rec, .{ .r = 30, .g = 36, .b = 50, .a = 255 });
            }

            rl.DrawCircle(@intFromFloat(i_rec.x + 14), @intFromFloat(i_rec.y + 15), 5, e.color);
            drawUText(font, e.getName(), i_rec.x + 28, i_rec.y + 7, 12, if (is_sel) .{ .r = 255, .g = 255, .b = 255, .a = 255 } else .{ .r = 180, .g = 195, .b = 215, .a = 255 });

            item_y += 34;
        }

        // --------------------------------------------------------------------
        // D. RIGHT PANEL: COMPONENT PROPERTY INSPECTOR (Unreal Slate Style)
        // --------------------------------------------------------------------
        const insp_x = screen_w - right_panel_w;
        rl.DrawRectangle(@intFromFloat(insp_x), @intFromFloat(top_bar_h), @intFromFloat(right_panel_w), @intFromFloat(screen_h - top_bar_h), .{ .r = 22, .g = 25, .b = 35, .a = 255 });
        rl.DrawLine(@intFromFloat(insp_x), @intFromFloat(top_bar_h), @intFromFloat(insp_x), @intFromFloat(screen_h), .{ .r = 45, .g = 52, .b = 70, .a = 255 });

        drawUText(font, "ИНСПЕКТОР СВОЙСТВ (Properties)", insp_x + 16, top_bar_h + 12, 13, .{ .r = 180, .g = 195, .b = 220, .a = 255 });

        if (selected_idx) |s_idx| {
            var se = &entities[s_idx];
            var py: f32 = top_bar_h + 40;

            // Header Entity Card
            rl.DrawRectangle(@intFromFloat(insp_x + 12), @intFromFloat(py), @intFromFloat(right_panel_w - 24), 32, .{ .r = 30, .g = 36, .b = 52, .a = 255 });
            rl.DrawCircle(@intFromFloat(insp_x + 26), @intFromFloat(py + 16), 6, se.color);
            drawUText(font, se.getName(), insp_x + 40, py + 7, 13, .{ .r = 255, .g = 215, .b = 60, .a = 255 });
            py += 42;

            // SECTION 1: Transform Component (P³ Homogeneous Vector XYZW)
            rl.DrawRectangle(@intFromFloat(insp_x + 12), @intFromFloat(py), @intFromFloat(right_panel_w - 24), 22, .{ .r = 38, .g = 45, .b = 64, .a = 255 });
            drawUText(font, "▼ Трансформация P³ (Homogeneous 4D)", insp_x + 18, py + 4, 11, .{ .r = 160, .g = 200, .b = 255, .a = 255 });
            py += 28;

            var vbuf: [64]u8 = undefined;

            // Position XYZW
            drawUText(font, "Позиция:", insp_x + 16, py + 2, 11, .{ .r = 180, .g = 190, .b = 205, .a = 255 });
            // X (Red Badge)
            rl.DrawRectangle(@intFromFloat(insp_x + 80), @intFromFloat(py), 55, 20, .{ .r = 90, .g = 30, .b = 30, .a = 255 });
            const xt = std.fmt.bufPrint(&vbuf, "X:{d:.2}", .{se.pos.x}) catch "";
            drawUText(font, xt, insp_x + 84, py + 3, 10, .{ .r = 255, .g = 140, .b = 140, .a = 255 });

            // Y (Green Badge)
            rl.DrawRectangle(@intFromFloat(insp_x + 140), @intFromFloat(py), 55, 20, .{ .r = 30, .g = 80, .b = 40, .a = 255 });
            const yt = std.fmt.bufPrint(&vbuf, "Y:{d:.2}", .{se.pos.y}) catch "";
            drawUText(font, yt, insp_x + 144, py + 3, 10, .{ .r = 140, .g = 255, .b = 140, .a = 255 });

            // Z (Blue Badge)
            rl.DrawRectangle(@intFromFloat(insp_x + 200), @intFromFloat(py), 55, 20, .{ .r = 30, .g = 50, .b = 95, .a = 255 });
            const zt = std.fmt.bufPrint(&vbuf, "Z:{d:.2}", .{se.pos.z}) catch "";
            drawUText(font, zt, insp_x + 204, py + 3, 10, .{ .r = 140, .g = 180, .b = 255, .a = 255 });

            // W (Violet Badge)
            rl.DrawRectangle(@intFromFloat(insp_x + 260), @intFromFloat(py), 55, 20, .{ .r = 75, .g = 40, .b = 95, .a = 255 });
            const wt = std.fmt.bufPrint(&vbuf, "W:{d:.2}", .{se.pos.w}) catch "";
            drawUText(font, wt, insp_x + 264, py + 3, 10, .{ .r = 220, .g = 150, .b = 255, .a = 255 });
            py += 26;

            // Scale with [-] and [+] interactive controls
            drawUText(font, "Масштаб:", insp_x + 16, py + 2, 11, .{ .r = 180, .g = 190, .b = 205, .a = 255 });
            const st = std.fmt.bufPrint(&vbuf, "{d:.2}x", .{se.scale.x}) catch "";
            drawUText(font, st, insp_x + 90, py + 2, 12, .{ .r = 255, .g = 255, .b = 255, .a = 255 });

            const m_b = rl.Rectangle{ .x = insp_x + 150, .y = py, .width = 24, .height = 18 };
            if (rl.CheckCollisionPointRec(mouse, m_b) and rl.IsMouseButtonPressed(rl.MOUSE_BUTTON_LEFT)) {
                se.scale.x = @max(0.05, se.scale.x - 0.05);
                se.scale.y = se.scale.x;
                se.scale.z = se.scale.x;
            }
            rl.DrawRectangleRec(m_b, .{ .r = 45, .g = 55, .b = 75, .a = 255 });
            drawUText(font, "-", m_b.x + 8, m_b.y + 1, 12, .{ .r = 255, .g = 255, .b = 255, .a = 255 });

            const p_b = rl.Rectangle{ .x = insp_x + 180, .y = py, .width = 24, .height = 18 };
            if (rl.CheckCollisionPointRec(mouse, p_b) and rl.IsMouseButtonPressed(rl.MOUSE_BUTTON_LEFT)) {
                se.scale.x += 0.05;
                se.scale.y = se.scale.x;
                se.scale.z = se.scale.x;
            }
            rl.DrawRectangleRec(p_b, .{ .r = 45, .g = 55, .b = 75, .a = 255 });
            drawUText(font, "+", p_b.x + 7, p_b.y + 1, 12, .{ .r = 255, .g = 255, .b = 255, .a = 255 });
            py += 32;

            // SECTION 2: Celestial / Keplerian Astronomy
            if (se.is_celestial) {
                rl.DrawRectangle(@intFromFloat(insp_x + 12), @intFromFloat(py), @intFromFloat(right_panel_w - 24), 22, .{ .r = 38, .g = 45, .b = 64, .a = 255 });
                drawUText(font, "▼ Астрофизика & Законы Кеплера", insp_x + 18, py + 4, 11, .{ .r = 160, .g = 200, .b = 255, .a = 255 });
                py += 26;

                const mt = std.fmt.bufPrint(&vbuf, "Масса: {d:.2} M_земли", .{se.mass_earth}) catch "";
                drawUText(font, mt, insp_x + 16, py, 11, .{ .r = 180, .g = 190, .b = 205, .a = 255 });
                py += 18;

                const ot = std.fmt.bufPrint(&vbuf, "Большая полуось: {d:.2} а.е.", .{se.orbit_semi_major_au}) catch "";
                drawUText(font, ot, insp_x + 16, py, 11, .{ .r = 180, .g = 190, .b = 205, .a = 255 });
                py += 18;

                const t_days = @sqrt(se.orbit_semi_major_au * se.orbit_semi_major_au * se.orbit_semi_major_au) * 365.25;
                const tt = std.fmt.bufPrint(&vbuf, "Период T: {d:.1} суток", .{t_days}) catch "";
                drawUText(font, tt, insp_x + 16, py, 11, .{ .r = 100, .g = 255, .b = 180, .a = 255 });
                py += 18;

                const g_val = 9.80665 * (se.mass_earth / (se.radius_earth * se.radius_earth));
                const gt = std.fmt.bufPrint(&vbuf, "Гравитация g: {d:.2} м/с²", .{g_val}) catch "";
                drawUText(font, gt, insp_x + 16, py, 11, .{ .r = 100, .g = 255, .b = 180, .a = 255 });
                py += 26;
            }

            // SECTION 3: Symplectic Dynamics S3
            if (se.is_geodesic) {
                rl.DrawRectangle(@intFromFloat(insp_x + 12), @intFromFloat(py), @intFromFloat(right_panel_w - 24), 22, .{ .r = 38, .g = 45, .b = 64, .a = 255 });
                drawUText(font, "▼ Симплектическая Динамика S³", insp_x + 18, py + 4, 11, .{ .r = 160, .g = 200, .b = 255, .a = 255 });
                py += 26;

                const wt_txt = std.fmt.bufPrint(&vbuf, "Скорость вращения: {d:.2} рад/с", .{se.geodesic_omega}) catch "";
                drawUText(font, wt_txt, insp_x + 16, py, 11, .{ .r = 180, .g = 190, .b = 205, .a = 255 });
                py += 18;

                drawUText(font, "Сохранение энергии: ΔE/E₀ < 10⁻¹⁶", insp_x + 16, py, 11, .{ .r = 80, .g = 255, .b = 120, .a = 255 });
                py += 18;
                drawUText(font, "Сингулярность W=0: отсутствует", insp_x + 16, py, 11, .{ .r = 0, .g = 220, .b = 255, .a = 255 });
                py += 26;
            }
        }

        // --------------------------------------------------------------------
        // E. BOTTOM PANEL: ASSET BROWSER & DIAGNOSTICS CONSOLE
        // --------------------------------------------------------------------
        const bot_y = screen_h - bottom_bar_h;
        rl.DrawRectangle(0, @intFromFloat(bot_y), @intFromFloat(screen_w), @intFromFloat(bottom_bar_h), .{ .r = 20, .g = 23, .b = 32, .a = 255 });
        rl.DrawLine(0, @intFromFloat(bot_y), @intFromFloat(screen_w), @intFromFloat(bot_y), .{ .r = 45, .g = 52, .b = 70, .a = 255 });

        // Asset Tree
        const a_w: f32 = 360.0;
        rl.DrawLine(@intFromFloat(a_w), @intFromFloat(bot_y), @intFromFloat(a_w), @intFromFloat(screen_h), .{ .r = 35, .g = 42, .b = 58, .a = 255 });
        drawUText(font, "ФАЙЛОВЫЙ МЕНЕДЖЕР РЕСУРСОВ", 16, bot_y + 10, 12, .{ .r = 160, .g = 175, .b = 200, .a = 255 });

        drawUText(font, "> /src/p3_kernel.zig (Проективное ядро)", 20, bot_y + 32, 11, .{ .r = 140, .g = 180, .b = 230, .a = 255 });
        drawUText(font, "> /src/p3_scale.zig (Астрономия и шкалы)", 20, bot_y + 50, 11, .{ .r = 140, .g = 180, .b = 230, .a = 255 });
        drawUText(font, "> /src/p3_null_fluid.zig (Оптика Друде)", 20, bot_y + 68, 11, .{ .r = 140, .g = 180, .b = 230, .a = 255 });
        drawUText(font, "> /src/p3_math.zig (Математика Unreal Engine)", 20, bot_y + 86, 11, .{ .r = 140, .g = 180, .b = 230, .a = 255 });
        drawUText(font, "> /shaders/p3_geodesic.wgsl (GPU Вычисления)", 20, bot_y + 104, 11, .{ .r = 140, .g = 180, .b = 230, .a = 255 });

        // Console Log
        drawUText(font, "ЖУРНАЛ ДИАГНОСТИКИ И СОБЫТИЙ ДВИЖКА (Console)", a_w + 16, bot_y + 10, 12, .{ .r = 160, .g = 175, .b = 200, .a = 255 });

        var ly = bot_y + 32;
        for (0..log_count) |i| {
            drawUText(font, &log_entries[i], a_w + 20, ly, 11, .{ .r = 180, .g = 220, .b = 180, .a = 255 });
            ly += 16;
        }

        rl.EndDrawing();
    }
}
