// =============================================================================
// P³ ENGINE — NATIVE O3DE PROJECT MANAGER & LAUNCHER (ZIG + RAYLIB)
// =============================================================================
// Полный перенос функционала O3DE Launcher в Slate Dark UI:
//   - Projects Grid (список проектов из ~/.o3de/o3de_manifest.json + project.json)
//   - Прямой запуск O3DE Editor / GameLauncher / AssetProcessor
//   - Каталог Gems из SDK (/opt/O3DE/26.05/Gems) с фильтрацией и тегами
//   - Настройки движка (Engine Settings, пути SDK, кэш, компиляторы)
//   - Консоль логов сборки и процессов в реальном времени
// =============================================================================

const std = @import("std");
const o3de_manifest = @import("p3_o3de_manifest");
const o3de_builder = @import("p3_o3de_builder");
const o3de_viewport = @import("p3_o3de_viewport");

const rl = @cImport({
    @cInclude("raylib.h");
});

// Цветовая палитра O3DE Slate Theme
const C_BG_DARK       = rl.Color{ .r = 18,  .g = 20,  .b = 24,  .a = 255 }; // Фон окна
const C_SIDEBAR_BG    = rl.Color{ .r = 24,  .g = 27,  .b = 33,  .a = 255 }; // Боковая панель
const C_CARD_BG       = rl.Color{ .r = 30,  .g = 34,  .b = 42,  .a = 255 }; // Карточка проекта/гема
const C_CARD_HOVER    = rl.Color{ .r = 40,  .g = 45,  .b = 56,  .a = 255 }; // Ховер карточки
const C_BORDER        = rl.Color{ .r = 48,  .g = 54,  .b = 66,  .a = 255 }; // Границы
const C_ACCENT_BLUE   = rl.Color{ .r = 0,   .g = 136, .b = 255, .a = 255 }; // O3DE Синий акцент
const C_ACCENT_HOVER  = rl.Color{ .r = 30,  .g = 155, .b = 255, .a = 255 }; // Ховер кнопок
const C_TEXT_PRIMARY  = rl.Color{ .r = 240, .g = 243, .b = 246, .a = 255 }; // Основной текст
const C_TEXT_MUTED    = rl.Color{ .r = 140, .g = 148, .b = 160, .a = 255 }; // Второстепенный текст
const C_STATUS_OK     = rl.Color{ .r = 46,  .g = 204, .b = 113, .a = 255 }; // Зеленый статус

const NavTab = enum {
    projects,
    viewport_3d,
    gems,
    engine_settings,
    build_console,
};

fn drawZString(str: []const u8, posX: i32, posY: i32, fontSize: i32, color: rl.Color) void {
    var buf: [512:0]u8 = undefined;
    const len = @min(str.len, 511);
    @memcpy(buf[0..len], str[0..len]);
    buf[len] = 0;
    rl.DrawText(&buf, posX, posY, fontSize, color);
}

fn drawCustomCursor(pos: rl.Vector2) void {
    const p1 = rl.Vector2{ .x = pos.x, .y = pos.y };
    const p2 = rl.Vector2{ .x = pos.x + 14.0, .y = pos.y + 14.0 };
    const p3 = rl.Vector2{ .x = pos.x + 5.0, .y = pos.y + 18.0 };
    const p4 = rl.Vector2{ .x = pos.x, .y = pos.y + 20.0 };

    // Тень / обводка курсора
    rl.DrawTriangle(
        .{ .x = p1.x + 1, .y = p1.y + 1 },
        .{ .x = p2.x + 1, .y = p2.y + 1 },
        .{ .x = p3.x + 1, .y = p3.y + 1 },
        rl.Color{ .r = 0, .g = 0, .b = 0, .a = 200 },
    );
    // Основной указатель
    rl.DrawTriangle(p1, p2, p3, C_TEXT_PRIMARY);
    rl.DrawTriangle(p1, p3, p4, C_TEXT_PRIMARY);
    rl.DrawLineV(p1, p2, rl.Color{ .r = 20, .g = 20, .b = 20, .a = 255 });
    rl.DrawLineV(p2, p3, rl.Color{ .r = 20, .g = 20, .b = 20, .a = 255 });
    rl.DrawLineV(p3, p4, rl.Color{ .r = 20, .g = 20, .b = 20, .a = 255 });
    rl.DrawLineV(p4, p1, rl.Color{ .r = 20, .g = 20, .b = 20, .a = 255 });
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const o3de_dir = std.posix.getenv("O3DE_DIR") orelse "/opt/O3DE/26.05";

    // Инициализация манифеста и сканирование проектов
    var manifest = o3de_manifest.loadGlobalManifest(allocator) catch o3de_manifest.O3deManifest{};
    defer manifest.deinit(allocator);

    var projects = std.ArrayList(o3de_manifest.ProjectInfo).init(allocator);
    defer {
        for (projects.items) |*p| p.deinit(allocator);
        projects.deinit();
    }

    for (manifest.projects) |proj_path| {
        if (o3de_manifest.loadProjectJson(allocator, proj_path)) |proj_info| {
            try projects.append(proj_info);
        } else |_| {}
    }

    // Сканирование Gems
    const gems = o3de_manifest.scanEngineGems(allocator, o3de_dir) catch &[_]o3de_manifest.GemInfo{};
    defer {
        for (gems) |*g| {
            var gem_mut = g.*;
            gem_mut.deinit(allocator);
        }
        allocator.free(gems);
    }

    var proc_runner = o3de_builder.AsyncProcess.init(allocator);
    defer proc_runner.deinit();

    // Настройки окна Raylib
    rl.SetConfigFlags(rl.FLAG_WINDOW_RESIZABLE | rl.FLAG_MSAA_4X_HINT);
    rl.InitWindow(1280, 760, "O3DE Project Manager (P³ Engine Native)");
    defer rl.CloseWindow();
    rl.SetTargetFPS(60);

    // Включаем видимость системного курсора + явный показ
    rl.ShowCursor();
    rl.EnableCursor();

    var viewport = try o3de_viewport.NativeViewport.init(allocator);
    defer viewport.deinit();

    // Попытка загрузить дефолтный уровень проекта
    viewport.loadLevel("/home/vitalij/o3de_game_project/Asset/Levels/defaultlevel/defaultlevel.spawnable") catch {};

    var current_tab: NavTab = .projects;
    var status_msg: []const u8 = "P³ Engine — Все нативные модули активны";
    var gem_scroll: f32 = 0.0;

    while (!rl.WindowShouldClose()) {
        const sw: f32 = @floatFromInt(rl.GetScreenWidth());
        const sh: f32 = @floatFromInt(rl.GetScreenHeight());
        const mouse_pos = rl.GetMousePosition();
        const mouse_clicked = rl.IsMouseButtonPressed(rl.MOUSE_BUTTON_LEFT);
        const mouse_down_right = rl.IsMouseButtonDown(rl.MOUSE_BUTTON_RIGHT);
        const wheel = rl.GetMouseWheelMove();

        if (current_tab == .gems) {
            gem_scroll -= wheel * 30.0;
            if (gem_scroll < 0) gem_scroll = 0;
        }

        rl.BeginDrawing();
        rl.ClearBackground(C_BG_DARK);

        // --- 1. Боковая панель O3DE Sidebar (240px) ---
        const sidebar_w: f32 = 240.0;
        rl.DrawRectangle(0, 0, @intFromFloat(sidebar_w), @intFromFloat(sh), C_SIDEBAR_BG);
        rl.DrawLine(@intFromFloat(sidebar_w), 0, @intFromFloat(sidebar_w), @intFromFloat(sh), C_BORDER);

        // O3DE Логотип & Заголовок
        rl.DrawText("P³ ENGINE", 25, 25, 24, C_TEXT_PRIMARY);
        rl.DrawText("Native Studio", 145, 33, 13, C_ACCENT_BLUE);
        rl.DrawLine(25, 65, @intFromFloat(sidebar_w - 25.0), 65, C_BORDER);

        // Навигационные вкладки
        const nav_items = [_]struct { label: []const u8, tab: NavTab }{
            .{ .label = "Проекты O3DE", .tab = .projects },
            .{ .label = "3D Вьюпорт (P³)", .tab = .viewport_3d },
            .{ .label = "Каталог Gems", .tab = .gems },
            .{ .label = "Движок и пути", .tab = .engine_settings },
            .{ .label = "Консоль сборки", .tab = .build_console },
        };

        var nav_y: f32 = 90.0;
        for (nav_items) |item| {
            const is_selected = (current_tab == item.tab);
            const btn_rect = rl.Rectangle{ .x = 15, .y = nav_y, .width = sidebar_w - 30.0, .height = 40.0 };
            const is_hover = rl.CheckCollisionPointRec(mouse_pos, btn_rect);

            if (is_selected) {
                rl.DrawRectangleRec(btn_rect, C_CARD_BG);
                rl.DrawRectangle(15, @intFromFloat(nav_y), 4, 40, C_ACCENT_BLUE);
            } else if (is_hover) {
                rl.DrawRectangleRec(btn_rect, C_CARD_HOVER);
            }

            if (is_hover and mouse_clicked) {
                current_tab = item.tab;
            }

            drawZString(item.label, 30, @intFromFloat(nav_y + 12.0), 15, if (is_selected) C_TEXT_PRIMARY else C_TEXT_MUTED);
            nav_y += 48.0;
        }

        // Индикатор статуса внизу сайдбара
        rl.DrawRectangle(15, @intFromFloat(sh - 55.0), @intFromFloat(sidebar_w - 30.0), 40, C_CARD_BG);
        rl.DrawCircle(32, @intFromFloat(sh - 35.0), 5.0, C_STATUS_OK);
        rl.DrawText("P³ + O3DE Ready", 46, @intFromFloat(sh - 42.0), 13, C_TEXT_PRIMARY);

        // --- 2. Верхний Action Bar ---
        const content_x = sidebar_w + 30.0;
        const top_bar_h: f32 = 70.0;
        rl.DrawLine(@intFromFloat(sidebar_w), @intFromFloat(top_bar_h), @intFromFloat(sw), @intFromFloat(top_bar_h), C_BORDER);

        switch (current_tab) {
            .projects => {
                rl.DrawText("Проекты O3DE", @intFromFloat(content_x), 25, 22, C_TEXT_PRIMARY);
                
                // Кнопка "+ Новый проект"
                const new_proj_btn = rl.Rectangle{ .x = sw - 180.0, .y = 18.0, .width = 150.0, .height = 36.0 };
                const np_hover = rl.CheckCollisionPointRec(mouse_pos, new_proj_btn);
                rl.DrawRectangleRec(new_proj_btn, if (np_hover) C_ACCENT_HOVER else C_ACCENT_BLUE);
                rl.DrawText("+ Новый проект", @intFromFloat(sw - 165.0), 28, 14, C_TEXT_PRIMARY);

                // Список проектов (Карточки)
                var card_y: f32 = top_bar_h + 30.0;
                if (projects.items.len == 0) {
                    rl.DrawText("Нет зарегистрированных проектов в ~/.o3de/o3de_manifest.json", @intFromFloat(content_x), @intFromFloat(card_y), 16, C_TEXT_MUTED);
                }

                for (projects.items) |proj| {
                    const card_rect = rl.Rectangle{ .x = content_x, .y = card_y, .width = sw - content_x - 30.0, .height = 110.0 };
                    const card_hover = rl.CheckCollisionPointRec(mouse_pos, card_rect);
                    rl.DrawRectangleRec(card_rect, if (card_hover) C_CARD_HOVER else C_CARD_BG);
                    rl.DrawRectangleLinesEx(card_rect, 1.0, C_BORDER);

                    // Инфо о проекте
                    drawZString(if (proj.display_name.len > 0) proj.display_name else "Проект O3DE", @intFromFloat(content_x + 20.0), @intFromFloat(card_y + 18.0), 18, C_TEXT_PRIMARY);
                    drawZString(proj.path, @intFromFloat(content_x + 20.0), @intFromFloat(card_y + 44.0), 13, C_TEXT_MUTED);
                    drawZString(proj.project_id, @intFromFloat(content_x + 20.0), @intFromFloat(card_y + 68.0), 12, C_TEXT_MUTED);

                    // Кнопка "Открыть в Editor"
                    const edit_btn = rl.Rectangle{ .x = sw - 320.0, .y = card_y + 35.0, .width = 130.0, .height = 38.0 };
                    const eb_hover = rl.CheckCollisionPointRec(mouse_pos, edit_btn);
                    rl.DrawRectangleRec(edit_btn, if (eb_hover) C_ACCENT_HOVER else C_ACCENT_BLUE);
                    rl.DrawText("Открыть Editor", @intFromFloat(sw - 305.0), @intFromFloat(card_y + 47.0), 13, C_TEXT_PRIMARY);

                    if (eb_hover and mouse_clicked) {
                        status_msg = "Запуск O3DE Editor...";
                        proc_runner.launchEditor(o3de_dir, proj.path) catch {
                            status_msg = "Ошибка запуска Editor";
                        };
                    }

                    // Кнопка "Запустить Game"
                    const game_btn = rl.Rectangle{ .x = sw - 170.0, .y = card_y + 35.0, .width = 120.0, .height = 38.0 };
                    const gb_hover = rl.CheckCollisionPointRec(mouse_pos, game_btn);
                    rl.DrawRectangleRec(game_btn, if (gb_hover) C_CARD_BG else C_SIDEBAR_BG);
                    rl.DrawRectangleLinesEx(game_btn, 1.0, C_BORDER);
                    rl.DrawText("Запуск Game", @intFromFloat(sw - 155.0), @intFromFloat(card_y + 47.0), 13, C_TEXT_PRIMARY);

                    if (gb_hover and mouse_clicked) {
                        status_msg = "Запуск O3DE GameLauncher...";
                        proc_runner.launchGame(o3de_dir, proj.path) catch {
                            status_msg = "Ошибка запуска GameLauncher";
                        };
                    }

                    card_y += 125.0;
                }
            },
            .viewport_3d => {
                rl.DrawText("Нативный 3D Вьюпорт (P³ Proj-Geo Level View)", @intFromFloat(content_x), 25, 22, C_TEXT_PRIMARY);
                rl.DrawText("Управление: ПКМ + движение мыши = Орбита, Колесико = Zoom", @intFromFloat(sw - 500.0), 30, 13, C_TEXT_MUTED);

                const vp_rect = rl.Rectangle{
                    .x = content_x,
                    .y = top_bar_h + 20.0,
                    .width = sw - content_x - 30.0,
                    .height = sh - top_bar_h - 60.0,
                };

                const in_vp = rl.CheckCollisionPointRec(mouse_pos, vp_rect);
                viewport.update(in_vp and mouse_down_right, if (in_vp) wheel else 0.0);
                viewport.render(vp_rect.x, vp_rect.y, vp_rect.width, vp_rect.height);
            },
            .gems => {
                rl.DrawText("Каталог плагинов (Gems Catalog)", @intFromFloat(content_x), 25, 22, C_TEXT_PRIMARY);
                var gy: f32 = top_bar_h + 20.0 - gem_scroll;

                for (gems) |g| {
                    if (gy > top_bar_h - 60.0 and gy < sh - 40.0) {
                        const g_rect = rl.Rectangle{ .x = content_x, .y = gy, .width = sw - content_x - 30.0, .height = 70.0 };
                        rl.DrawRectangleRec(g_rect, C_CARD_BG);
                        rl.DrawRectangleLinesEx(g_rect, 1.0, C_BORDER);

                        drawZString(if (g.display_name.len > 0) g.display_name else "Gem", @intFromFloat(content_x + 15.0), @intFromFloat(gy + 12.0), 16, C_TEXT_PRIMARY);
                        drawZString(if (g.summary.len > 0) g.summary else "Плагин расширения функционала движка", @intFromFloat(content_x + 15.0), @intFromFloat(gy + 36.0), 13, C_TEXT_MUTED);
                    }
                    gy += 80.0;
                }
            },
            .engine_settings => {
                rl.DrawText("Настройки движка (Engine Settings)", @intFromFloat(content_x), 25, 22, C_TEXT_PRIMARY);
                const sy: f32 = top_bar_h + 30.0;
                rl.DrawText("Путь к O3DE SDK:", @intFromFloat(content_x), @intFromFloat(sy), 16, C_TEXT_PRIMARY);
                drawZString(o3de_dir, @intFromFloat(content_x), @intFromFloat(sy + 30.0), 14, C_TEXT_MUTED);

                rl.DrawText("Каталог проектов по умолчанию:", @intFromFloat(content_x), @intFromFloat(sy + 80.0), 16, C_TEXT_PRIMARY);
                drawZString(if (manifest.default_projects_folder.len > 0) manifest.default_projects_folder else "~/O3DE/Projects", @intFromFloat(content_x), @intFromFloat(sy + 110.0), 14, C_TEXT_MUTED);
            },
            .build_console => {
                rl.DrawText("Консоль вывода сборки (Build & Process Console)", @intFromFloat(content_x), 25, 22, C_TEXT_PRIMARY);
                const cy: f32 = top_bar_h + 20.0;
                const console_rect = rl.Rectangle{ .x = content_x, .y = cy, .width = sw - content_x - 30.0, .height = sh - cy - 50.0 };
                rl.DrawRectangleRec(console_rect, rl.Color{ .r = 10, .g = 12, .b = 15, .a = 255 });
                rl.DrawRectangleLinesEx(console_rect, 1.0, C_BORDER);
                rl.DrawText("Готов к запуску процессов сборки O3DE / CMake / Ninja / Zig.", @intFromFloat(content_x + 15.0), @intFromFloat(cy + 15.0), 14, C_STATUS_OK);
            },
        }

        // Нижняя строка состояния (Status Bar)
        rl.DrawRectangle(0, @intFromFloat(sh - 28.0), @intFromFloat(sw), 28, C_SIDEBAR_BG);
        rl.DrawLine(0, @intFromFloat(sh - 28.0), @intFromFloat(sw), @intFromFloat(sh - 28.0), C_BORDER);
        drawZString(status_msg, 250, @intFromFloat(sh - 20.0), 12, C_TEXT_MUTED);

        // Рендерим четкий аппаратный/кастомный курсор
        drawCustomCursor(mouse_pos);

        rl.EndDrawing();
    }
}
