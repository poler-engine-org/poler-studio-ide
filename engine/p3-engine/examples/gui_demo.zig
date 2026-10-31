// =============================================================================
// P³ ENGINE — UNIFIED UI RUNTIME DEMO (O3DE LyShine & Slate Pipeline)
// =============================================================================
//
// Demonstrates the complete unified P³ UI Runtime:
//   Input Event (Raylib) ──→ UiCanvas.handleInput (Input Routing)
//                        ──→ Widget Component State (Hover/Pressed/Drag)
//                        ──→ UiCanvas Layout Update
//                        ──→ DrawQueue Batch Accumulation
//                        ──→ UiRaylibRenderer (Unicode UTF-8 Text + Scissor Clipping)
//
// Key Components Demonstrated:
//   - Interactive Buttons (Normal, Hover, Pressed states)
//   - Checkbox / Toggle Switch
//   - Linear Sliders with live value readouts
//   - Drag & Drop target area
//   - Unicode UTF-8 Typography with Cyrillic & Mathematical symbols
// =============================================================================

const std = @import("std");
const math = std.math;

const rl = @cImport({
    @cInclude("raylib.h");
});

const ui_transform = @import("p3_ui_transform.zig");
const ui_canvas = @import("p3_ui_canvas.zig");
const ui_draw = @import("p3_ui_draw.zig");
const ui_raylib = @import("p3_ui_raylib.zig");

const Vec2 = ui_transform.Vec2;
const Rect = ui_transform.Rect;
const Color = ui_draw.Color;
const UiCanvas = ui_canvas.UiCanvas;
const UiElement = ui_canvas.UiElement;
const InputEvent = ui_canvas.InputEvent;
const DrawQueue = ui_draw.DrawQueue;
const DrawCommand = ui_draw.DrawCommand;

// App State
var button_clicks: u32 = 0;
var checkbox_checked: bool = true;
var slider_value: f32 = 0.65;
var drag_pos: Vec2 = Vec2.init(540, 240);
var is_dragging_box: bool = false;

fn onCanvasInput(element: *UiElement, event: InputEvent) bool {
    if (std.mem.eql(u8, element.name, "action_button")) {
        if (event.event_type == .mouse_down) {
            button_clicks += 1;
            return true;
        }
    } else if (std.mem.eql(u8, element.name, "toggle_checkbox")) {
        if (event.event_type == .mouse_down) {
            checkbox_checked = !checkbox_checked;
            return true;
        }
    } else if (std.mem.eql(u8, element.name, "value_slider")) {
        if (event.event_type == .mouse_down or (event.event_type == .mouse_move and element.state == .pressed)) {
            const rel_x = event.position.x - element.computed_rect.left;
            const w = element.computed_rect.width();
            if (w > 0) {
                slider_value = @floatCast(std.math.clamp(rel_x / w, 0.0, 1.0));
            }
            return true;
        }
    } else if (std.mem.eql(u8, element.name, "drag_box")) {
        if (event.event_type == .mouse_down) {
            is_dragging_box = true;
            return true;
        } else if (event.event_type == .mouse_up) {
            is_dragging_box = false;
            return true;
        } else if (event.event_type == .mouse_move and is_dragging_box) {
            drag_pos = event.position;
            return true;
        }
    }
    return false;
}

pub fn main() !void {
    rl.InitWindow(1280, 720, "P3 Engine — Unified UI Runtime Demo (O3DE LyShine & Slate)");
    rl.SetTargetFPS(60);
    rl.SetWindowState(rl.FLAG_WINDOW_RESIZABLE);
    defer rl.CloseWindow();

    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

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
    codepoints[cp_count] = 0x03C0; cp_count += 1;
    codepoints[cp_count] = 0x03A9; cp_count += 1;
    codepoints[cp_count] = 0x221E; cp_count += 1;

    const font = rl.LoadFontEx("/usr/share/fonts/TTF/DejaVuSans.ttf", 22, &codepoints, @intCast(cp_count));
    rl.SetTextureFilter(font.texture, rl.TEXTURE_FILTER_BILINEAR);
    defer rl.UnloadFont(font);

    // 2. Setup Canvas
    var canvas = UiCanvas.init(allocator, Rect.fromSize(0, 0, 1280, 720));
    defer canvas.deinit();
    canvas.setInputHandler(onCanvasInput);

    // Register UI Elements
    const btn_id = try canvas.createElement("action_button");
    const chk_id = try canvas.createElement("toggle_checkbox");
    const sld_id = try canvas.createElement("value_slider");
    const drg_id = try canvas.createElement("drag_box");

    var draw_queue = DrawQueue.init(allocator);
    defer draw_queue.deinit();

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
        const mouse = rl.GetMousePosition();
        const mouse_v = Vec2.init(mouse.x, mouse.y);

        // Update Canvas Element Rectangles
        if (canvas.findElement(btn_id)) |e| {
            e.computed_rect = Rect.fromSize(60, 160, 240, 48);
            e.interactable = true;
        }
        if (canvas.findElement(chk_id)) |e| {
            e.computed_rect = Rect.fromSize(60, 240, 240, 36);
            e.interactable = true;
        }
        if (canvas.findElement(sld_id)) |e| {
            e.computed_rect = Rect.fromSize(60, 320, 280, 24);
            e.interactable = true;
        }
        if (canvas.findElement(drg_id)) |e| {
            e.computed_rect = Rect.fromSize(drag_pos.x - 40, drag_pos.y - 40, 80, 80);
            e.interactable = true;
        }

        // Dispatch Raylib Events into UiCanvas
        _ = canvas.handleInput(.{
            .event_type = .mouse_move,
            .position = mouse_v,
            .button = 0,
            .key = 0,
            .delta = 0,
        });

        if (rl.IsMouseButtonPressed(rl.MOUSE_BUTTON_LEFT)) {
            _ = canvas.handleInput(.{
                .event_type = .mouse_down,
                .position = mouse_v,
                .button = 0,
                .key = 0,
                .delta = 0,
            });
        }
        if (rl.IsMouseButtonReleased(rl.MOUSE_BUTTON_LEFT)) {
            _ = canvas.handleInput(.{
                .event_type = .mouse_up,
                .position = mouse_v,
                .button = 0,
                .key = 0,
                .delta = 0,
            });
            is_dragging_box = false;
        }

        // --------------------------------------------------------------------
        // RENDER PASS
        // --------------------------------------------------------------------
        rl.BeginDrawing();
        rl.ClearBackground(.{ .r = 18, .g = 22, .b = 32, .a = 255 });

        // Header Panel
        rl.DrawRectangle(0, 0, 1280, 60, .{ .r = 25, .g = 30, .b = 45, .a = 255 });
        rl.DrawLine(0, 60, 1280, 60, .{ .r = 50, .g = 62, .b = 90, .a = 255 });
        drawUText(font, "P³ ENGINE UI RUNTIME (O3DE LyShine + Unreal Slate Architecture)", 30, 18, 18, .{ .r = 255, .g = 210, .b = 60, .a = 255 });

        // Left Container Panel (Widgets)
        rl.DrawRectangle(30, 90, 420, 580, .{ .r = 24, .g = 28, .b = 40, .a = 255 });
        rl.DrawRectangleLines(30, 90, 420, 580, .{ .r = 45, .g = 55, .b = 80, .a = 255 });
        drawUText(font, "Интерактивные Компоненты UI", 50, 110, 15, .{ .r = 170, .g = 205, .b = 255, .a = 255 });

        // 1. Button Render
        if (canvas.findElement(btn_id)) |e| {
            const r = rl.Rectangle{ .x = @floatCast(e.computed_rect.left), .y = @floatCast(e.computed_rect.top), .width = @floatCast(e.computed_rect.width()), .height = @floatCast(e.computed_rect.height()) };
            const col = switch (e.state) {
                .normal => rl.Color{ .r = 40, .g = 70, .b = 120, .a = 255 },
                .hover => rl.Color{ .r = 55, .g = 95, .b = 160, .a = 255 },
                .pressed, .active => rl.Color{ .r = 30, .g = 55, .b = 95, .a = 255 },
                .disabled => rl.Color{ .r = 40, .g = 40, .b = 50, .a = 255 },
            };
            rl.DrawRectangleRec(r, col);
            rl.DrawRectangleLinesEx(r, 1.5, .{ .r = 80, .g = 140, .b = 220, .a = 255 });

            var buf: [64]u8 = undefined;
            const b_txt = std.fmt.bufPrint(&buf, "Нажатий кнопки: {d}", .{button_clicks}) catch "";
            drawUText(font, b_txt, r.x + 30, r.y + 14, 14, .{ .r = 255, .g = 255, .b = 255, .a = 255 });
        }

        // 2. Checkbox Render
        if (canvas.findElement(chk_id)) |e| {
            const box_r = rl.Rectangle{ .x = @floatCast(e.computed_rect.left), .y = @floatCast(e.computed_rect.top), .width = 24, .height = 24 };
            rl.DrawRectangleRec(box_r, .{ .r = 35, .g = 42, .b = 60, .a = 255 });
            rl.DrawRectangleLinesEx(box_r, 1.5, .{ .r = 100, .g = 140, .b = 200, .a = 255 });
            if (checkbox_checked) {
                rl.DrawRectangle(@intFromFloat(box_r.x + 4), @intFromFloat(box_r.y + 4), 16, 16, .{ .r = 80, .g = 220, .b = 120, .a = 255 });
            }
            drawUText(font, "Включить проективный горизонт (W=0)", box_r.x + 36, box_r.y + 3, 13, .{ .r = 210, .g = 220, .b = 235, .a = 255 });
        }

        // 3. Slider Render
        if (canvas.findElement(sld_id)) |e| {
            const bar_r = rl.Rectangle{ .x = @floatCast(e.computed_rect.left), .y = @floatCast(e.computed_rect.top + 8), .width = @floatCast(e.computed_rect.width()), .height = 8 };
            rl.DrawRectangleRec(bar_r, .{ .r = 40, .g = 48, .b = 68, .a = 255 });

            const fill_w = bar_r.width * slider_value;
            rl.DrawRectangle(@intFromFloat(bar_r.x), @intFromFloat(bar_r.y), @intFromFloat(fill_w), @intFromFloat(bar_r.height), .{ .r = 60, .g = 150, .b = 255, .a = 255 });

            // Handle Knob
            const knob_x = bar_r.x + fill_w;
            rl.DrawCircle(@intFromFloat(knob_x), @intFromFloat(bar_r.y + 4), 8, .{ .r = 255, .g = 215, .b = 60, .a = 255 });

            var buf: [64]u8 = undefined;
            const s_txt = std.fmt.bufPrint(&buf, "Скорость геодезической: {d:.0}% (w={d:.2})", .{ slider_value * 100.0, slider_value * 2.0 }) catch "";
            drawUText(font, s_txt, bar_r.x, bar_r.y - 22, 12, .{ .r = 180, .g = 195, .b = 215, .a = 255 });
        }

        // Right Container Panel (Drag & Drop Canvas)
        rl.DrawRectangle(480, 90, 760, 580, .{ .r = 22, .g = 26, .b = 36, .a = 255 });
        rl.DrawRectangleLines(480, 90, 760, 580, .{ .r = 45, .g = 55, .b = 80, .a = 255 });
        drawUText(font, "Область Drag & Drop и Проективной Интерактивности", 510, 110, 15, .{ .r = 170, .g = 205, .b = 255, .a = 255 });
        drawUText(font, "Зажмите левую кнопку мыши на объекте и перетащите его:", 510, 138, 12, .{ .r = 140, .g = 150, .b = 175, .a = 255 });

        // 4. Draggable Box Render
        if (canvas.findElement(drg_id)) |e| {
            const dr = rl.Rectangle{ .x = @floatCast(e.computed_rect.left), .y = @floatCast(e.computed_rect.top), .width = @floatCast(e.computed_rect.width()), .height = @floatCast(e.computed_rect.height()) };
            rl.DrawRectangleRec(dr, if (is_dragging_box) .{ .r = 255, .g = 140, .b = 50, .a = 220 } else .{ .r = 120, .g = 60, .b = 200, .a = 200 });
            rl.DrawRectangleLinesEx(dr, 2.0, .{ .r = 255, .g = 220, .b = 100, .a = 255 });
            drawUText(font, "S³ Box", dr.x + 18, dr.y + 28, 14, .{ .r = 255, .g = 255, .b = 255, .a = 255 });
        }

        rl.EndDrawing();
    }
}
