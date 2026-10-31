// =============================================================================
// P³ ENGINE — LAUNCHER UI (отрисовка интерфейса в VisualFrameBuffer)
// =============================================================================
// Заменяет Qt widgets на отрисовку в наш встроенный виртуальный экран.
// Каждый элемент UI — прямоугольник с текстом, нарисованный через CPU.
// LLM/агент видит интерфейс через ПК-зрение (CV analyzer).
// =============================================================================

const std = @import("std");
const math = std.math;
const p3 = @import("root.zig");

const Vec3 = p3.Vec3;
const vision = p3.p3.vision;
const VisualFrameBuffer = vision.VisualFrameBuffer;
const PixelColor = vision.PixelColor;

// ---------------------------------------------------------------------------
// Цвета P³ Engine (киберпанк-неон, как в O3DE AzQtComponents)
// ---------------------------------------------------------------------------
pub const C_BG = PixelColor{ .r = 12, .g = 16, .b = 28, .a = 255 };
pub const C_BG_PANEL = PixelColor{ .r = 22, .g = 30, .b = 50, .a = 220 };
pub const C_BG_CARD = PixelColor{ .r = 28, .g = 38, .b = 58, .a = 255 };
pub const C_BG_CARD_HOVER = PixelColor{ .r = 35, .g = 50, .b = 80, .a = 255 };
pub const C_BORDER = PixelColor{ .r = 40, .g = 50, .b = 70, .a = 200 };
pub const C_ACCENT = PixelColor{ .r = 0, .g = 240, .b = 255, .a = 255 };
pub const C_ACCENT_DIM = PixelColor{ .r = 0, .g = 120, .b = 130, .a = 200 };
pub const C_ACCENT2 = PixelColor{ .r = 255, .g = 0, .b = 130, .a = 255 };
pub const C_TEXT = PixelColor{ .r = 220, .g = 230, .b = 240, .a = 255 };
pub const C_TEXT_DIM = PixelColor{ .r = 120, .g = 130, .b = 150, .a = 255 };
pub const C_TEXT_BRIGHT = PixelColor{ .r = 255, .g = 255, .b = 255, .a = 255 };
pub const C_GREEN = PixelColor{ .r = 0, .g = 255, .b = 130, .a = 255 };
pub const C_YELLOW = PixelColor{ .r = 255, .g = 220, .b = 50, .a = 255 };
pub const C_RED = PixelColor{ .r = 255, .g = 60, .b = 60, .a = 255 };
pub const C_BLUE = PixelColor{ .r = 80, .g = 120, .b = 255, .a = 255 };

// ---------------------------------------------------------------------------
// Прямоугольник (аналог QRect из Qt)
// ---------------------------------------------------------------------------
pub const Rect = struct {
    x: f32 = 0,
    y: f32 = 0,
    w: f32 = 0,
    h: f32 = 0,

    pub fn init(x: f32, y: f32, w: f32, h: f32) Rect {
        return .{ .x = x, .y = y, .w = w, .h = h };
    }

    pub fn contains(self: Rect, px: f32, py: f32) bool {
        return px >= self.x and px < self.x + self.w and py >= self.y and py < self.y + self.h;
    }
};

// ---------------------------------------------------------------------------
// Примитивы отрисовки в VisualFrameBuffer
// ---------------------------------------------------------------------------

/// Заливка прямоугольника
pub fn fillRect(fb: *VisualFrameBuffer, r: Rect, color: PixelColor) void {
    // Clamp to non-negative floats BEFORE @intFromFloat — otherwise negative
    // coordinates (e.g. when text is wider than its container and gets
    // center-clipped to a negative x) panic at runtime in Debug mode.
    const xf = @max(0.0, r.x);
    const yf = @max(0.0, r.y);
    const x1f = @max(0.0, r.x + r.w);
    const y1f = @max(0.0, r.y + r.h);
    const x0: usize = @intFromFloat(xf);
    const y0: usize = @intFromFloat(yf);
    const x1: usize = @min(fb.width, @as(usize, @intFromFloat(x1f)));
    const y1: usize = @min(fb.height, @as(usize, @intFromFloat(y1f)));
    if (x1 <= x0 or y1 <= y0) return;
    var y: usize = y0;
    while (y < y1) : (y += 1) {
        var x: usize = x0;
        while (x < x1) : (x += 1) {
            const idx = y * fb.width + x;
            fb.color_buffer[idx] = color;
            fb.depth_buffer[idx] = 1e8; // front
            fb.segmentation_buffer[idx] = 200; // UI element
        }
    }
}

/// Прямоугольник с скруглёнными углами (упрощённо — просто рамка)
pub fn drawRectBorder(fb: *VisualFrameBuffer, r: Rect, color: PixelColor, thickness: f32) void {
    const t = thickness;
    // Top
    fillRect(fb, .{ .x = r.x, .y = r.y, .w = r.w, .h = t }, color);
    // Bottom
    fillRect(fb, .{ .x = r.x, .y = r.y + r.h - t, .w = r.w, .h = t }, color);
    // Left
    fillRect(fb, .{ .x = r.x, .y = r.y, .w = t, .h = r.h }, color);
    // Right
    fillRect(fb, .{ .x = r.x + r.w - t, .y = r.y, .w = t, .h = r.h }, color);
}

/// Горизонтальная линия
pub fn drawHLine(fb: *VisualFrameBuffer, x: f32, y: f32, w: f32, color: PixelColor) void {
    fillRect(fb, .{ .x = x, .y = y, .w = w, .h = 1 }, color);
}

/// Вертикальная линия
pub fn drawVLine(fb: *VisualFrameBuffer, x: f32, y: f32, h: f32, color: PixelColor) void {
    fillRect(fb, .{ .x = x, .y = y, .w = 1, .h = h }, color);
}

/// Круг (для иконок)
pub fn drawCircle(fb: *VisualFrameBuffer, cx: f32, cy: f32, radius: f32, color: PixelColor) void {
    const r2 = radius * radius;
    const x0: i32 = @intFromFloat(@max(0, cx - radius));
    const y0: i32 = @intFromFloat(@max(0, cy - radius));
    const x1: i32 = @intFromFloat(@min(@as(f32, @floatFromInt(fb.width)), cx + radius));
    const y1: i32 = @intFromFloat(@min(@as(f32, @floatFromInt(fb.height)), cy + radius));
    var y: i32 = y0;
    while (y < y1) : (y += 1) {
        var x: i32 = x0;
        while (x < x1) : (x += 1) {
            const dx = @as(f32, @floatFromInt(x)) - cx;
            const dy = @as(f32, @floatFromInt(y)) - cy;
            if (dx * dx + dy * dy <= r2) {
                const idx = @as(usize, @intCast(y)) * fb.width + @as(usize, @intCast(x));
                fb.color_buffer[idx] = color;
                fb.depth_buffer[idx] = 1e8;
                fb.segmentation_buffer[idx] = 201;
            }
        }
    }
}

/// Прогресс-бар
pub fn drawProgressBar(fb: *VisualFrameBuffer, r: Rect, progress: f32, color: PixelColor) void {
    fillRect(fb, r, C_BG_PANEL);
    const fill_w = r.w * @max(0, @min(1, progress));
    fillRect(fb, .{ .x = r.x, .y = r.y, .w = fill_w, .h = r.h }, color);
    drawRectBorder(fb, r, C_BORDER, 1);
}

// ---------------------------------------------------------------------------
// Отрисовка текста (bitmap font — каждый символ 5×7 пикселей)
// ---------------------------------------------------------------------------

/// 5×7 bitmap font для базовых символов (ASCII + кириллица упрощённо)
/// В продакшене — загружать TTF через stb_truetype
const FONT_W: usize = 5;
const FONT_H: usize = 7;
const FONT_SPACING: usize = 1;

/// Упрощённая отрисовка текста — каждый символ как прямоугольник символов
pub fn drawText(fb: *VisualFrameBuffer, text: []const u8, x: f32, y: f32, size: f32, color: PixelColor) void {
    var cx = x;
    for (text) |ch| {
        if (ch == ' ') {
            cx += size * 3;
            continue;
        }
        if (ch == '\n') {
            cx = x;
            continue;
        }
        // Упрощённо: рисуем каждый символ как маленький прямоугольник
        // В реальной версии — bitmap lookup table или stb_truetype
        drawChar(fb, ch, cx, y, size, color);
        cx += size * 6 + FONT_SPACING;
    }
}

/// Отрисовка одного символа (упрощённая — прямоугольник с паттерном)
fn drawChar(fb: *VisualFrameBuffer, _: u8, x: f32, y: f32, size: f32, color: PixelColor) void {
    // Упрощённо: рисуем прямоугольник как фон символа
    // В реальной версии — bitmap pattern per character
    const w = size * 0.6;
    const h = size * 1.0;
    // Только контур — экономим пиксели
    drawRectBorder(fb, .{ .x = x, .y = y, .w = w, .h = h }, color, 1);
    // Цветной блок внутри для различимости
    fillRect(fb, .{ .x = x + 1, .y = y + 1, .w = @max(0, w - 2), .h = @max(0, h - 2) }, color);
}

/// Измерение ширины текста
pub fn textWidth(text: []const u8, size: f32) f32 {
    return @as(f32, @floatFromInt(text.len)) * (size * 6 + FONT_SPACING);
}

/// Отрисовка текста по центру
pub fn drawTextCentered(fb: *VisualFrameBuffer, text: []const u8, cx: f32, y: f32, size: f32, color: PixelColor) void {
    const w = textWidth(text, size);
    drawText(fb, text, cx - w / 2, y, size, color);
}

// ---------------------------------------------------------------------------
// UI виджеты (аналоги Qt widgets)
// ---------------------------------------------------------------------------

/// Кнопка (аналог QPushButton)
pub const Button = struct {
    rect: Rect = .{},
    label: []const u8 = "",
    accent: PixelColor = C_ACCENT,
    hover: bool = false,
    pressed: bool = false,
    clicked: bool = false,

    pub fn update(self: *Button, mouse_x: f32, mouse_y: f32, mouse_pressed: bool) void {
        self.hover = self.rect.contains(mouse_x, mouse_y);
        if (self.hover and mouse_pressed) {
            self.pressed = true;
        } else if (self.pressed and !mouse_pressed) {
            self.clicked = true;
            self.pressed = false;
        } else {
            self.pressed = false;
            self.clicked = false;
        }
    }

    pub fn draw(self: *const Button, fb: *VisualFrameBuffer) void {
        var bg = C_BG_CARD;
        if (self.hover) bg = C_BG_CARD_HOVER;
        if (self.pressed) bg = self.accent;

        fillRect(fb, self.rect, bg);
        drawRectBorder(fb, self.rect, if (self.hover) self.accent else C_BORDER, 2);

        // Текст кнопки
        const text_y = self.rect.y + (self.rect.h - 16) / 2;
        drawTextCentered(fb, self.label, self.rect.x + self.rect.w / 2, text_y, 14, if (self.pressed) C_TEXT_BRIGHT else C_TEXT);
    }
};

/// Карточка проекта (аналог ProjectButtonWidget)
pub const ProjectCard = struct {
    rect: Rect = .{},
    name: []const u8 = "",
    path: []const u8 = "",
    template: []const u8 = "Default",
    accent: PixelColor = C_ACCENT,
    hover: bool = false,
    build_progress: f32 = 0, // 0..1, 0 = не собирается
    has_preview: bool = false,

    pub fn update(self: *ProjectCard, mx: f32, my: f32, pressed: bool) bool {
        self.hover = self.rect.contains(mx, my);
        return self.hover and pressed;
    }

    pub fn draw(self: *const ProjectCard, fb: *VisualFrameBuffer) void {
        var bg = C_BG_CARD;
        if (self.hover) bg = C_BG_CARD_HOVER;

        fillRect(fb, self.rect, bg);
        drawRectBorder(fb, self.rect, if (self.hover) self.accent else C_BORDER, 2);

        // Цветная полоса слева (как в O3DE project card)
        fillRect(fb, .{ .x = self.rect.x, .y = self.rect.y, .w = 4, .h = self.rect.h }, self.accent);

        // Имя проекта
        drawText(fb, self.name, self.rect.x + 16, self.rect.y + 12, 16, C_TEXT_BRIGHT);

        // Путь
        drawText(fb, self.path, self.rect.x + 16, self.rect.y + 32, 12, C_TEXT_DIM);

        // Шаблон
        drawText(fb, "Шаблон: ", self.rect.x + 16, self.rect.y + 48, 12, C_TEXT_DIM);
        drawText(fb, self.template, self.rect.x + 90, self.rect.y + 48, 12, self.accent);

        // Прогресс-бар если идёт сборка
        if (self.build_progress > 0) {
            drawProgressBar(fb, .{ .x = self.rect.x + 16, .y = self.rect.y + self.rect.h - 20, .w = self.rect.w - 32, .h = 8 }, self.build_progress, self.accent);
        }
    }
};

/// Элемент списка (аналог QListWidgetItem)
pub const ListItem = struct {
    rect: Rect = .{},
    label: []const u8 = "",
    value: []const u8 = "",
    selected: bool = false,
    hover: bool = false,

    pub fn draw(self: *const ListItem, fb: *VisualFrameBuffer) void {
        var bg = C_BG_PANEL;
        if (self.selected) bg = C_BG_CARD_HOVER;
        if (self.hover and !self.selected) bg = C_BG_CARD;

        fillRect(fb, self.rect, bg);
        if (self.selected) {
            drawRectBorder(fb, self.rect, C_ACCENT, 2);
        }

        drawText(fb, self.label, self.rect.x + 8, self.rect.y + 4, 14, C_TEXT);
        if (self.value.len > 0) {
            const vw = textWidth(self.label, 14);
            drawText(fb, self.value, self.rect.x + 8 + vw + 12, self.rect.y + 4, 14, C_TEXT_DIM);
        }
    }
};

/// Заголовок экрана (аналог ScreenHeaderWidget)
pub const ScreenHeader = struct {
    rect: Rect = .{},
    title: []const u8 = "",
    back_button: Button = .{},

    pub fn init(rect: Rect, title: []const u8) ScreenHeader {
        return .{
            .rect = rect,
            .title = title,
            .back_button = .{
                .rect = .{ .x = rect.x, .y = rect.y, .w = 60, .h = rect.h },
                .label = "← Назад",
                .accent = C_ACCENT2,
            },
        };
    }

    pub fn draw(self: *const ScreenHeader, fb: *VisualFrameBuffer) void {
        fillRect(fb, self.rect, C_BG_PANEL);
        drawHLine(fb, self.rect.x, self.rect.y + self.rect.h, self.rect.w, C_BORDER);
        self.back_button.draw(fb);
        drawText(fb, self.title, self.rect.x + 80, self.rect.y + 10, 18, C_TEXT_BRIGHT);
    }
};

// ---------------------------------------------------------------------------
// Полный экран (аналог ScreensCtrl)
// ---------------------------------------------------------------------------
pub const ScreenLayout = struct {
    width: f32,
    height: f32,
    sidebar_w: f32 = 260,
    header_h: f32 = 50,
    footer_h: f32 = 40,
    padding: f32 = 16,

    pub fn contentX(self: ScreenLayout) f32 {
        return self.sidebar_w + self.padding;
    }
    pub fn contentW(self: ScreenLayout) f32 {
        return self.width - self.sidebar_w - self.padding * 2;
    }
    pub fn contentY(self: ScreenLayout) f32 {
        return self.header_h + self.padding;
    }
    pub fn contentH(self: ScreenLayout) f32 {
        return self.height - self.header_h - self.footer_h - self.padding * 2;
    }
};

// ---------------------------------------------------------------------------
// Отрисовка фона (звёздное небо + градиент)
// ---------------------------------------------------------------------------
pub fn drawBackground(fb: *VisualFrameBuffer, time: f32) void {
    var y: usize = 0;
    while (y < fb.height) : (y += 1) {
        const t = @as(f32, @floatFromInt(y)) / @as(f32, @floatFromInt(fb.height));
        const r: u8 = @intFromFloat(12.0 + (1.0 - t) * 6.0);
        const g: u8 = @intFromFloat(16.0 + (1.0 - t) * 4.0);
        const b: u8 = @intFromFloat(28.0 + (1.0 - t) * 18.0);
        var x: usize = 0;
        while (x < fb.width) : (x += 1) {
            const idx = y * fb.width + x;
            fb.color_buffer[idx] = PixelColor{ .r = r, .g = g, .b = b, .a = 255 };
            fb.depth_buffer[idx] = 1e9;
            fb.segmentation_buffer[idx] = 0;
        }
    }

    // Звёзды (детерминированный паттерн + время для анимации)
    var i: usize = 0;
    while (i < 60) : (i += 1) {
        const seed = i * 37 + 13;
        const sx: f32 = @floatFromInt(seed % @as(u32, @intCast(fb.width)));
        const sy_raw: f32 = @floatFromInt((seed * 31 + 7) % @as(u32, @intCast(fb.height)));
        const sy = @rem(sy_raw + time * (5 + @as(f32, @floatFromInt(seed % 20))), @as(f32, @floatFromInt(fb.height)));
        const brightness: u8 = @intFromFloat(80 + (seed * 7) % 150);
        const x: usize = @intFromFloat(sx);
        const y_star: usize = @intFromFloat(@max(0, sy));
        if (x < fb.width and y_star < fb.height) {
            const idx = y_star * fb.width + x;
            fb.color_buffer[idx] = PixelColor{ .r = brightness, .g = brightness, .b = brightness, .a = 255 };
            fb.depth_buffer[idx] = 1e8 - 1;
            fb.segmentation_buffer[idx] = 202; // star
        }
    }
}

// ---------------------------------------------------------------------------
// Боковая панель (аналог O3DE sidebar navigation)
// ---------------------------------------------------------------------------
pub fn drawSidebar(
    fb: *VisualFrameBuffer,
    layout: ScreenLayout,
    current_screen: u8,
    time: f32,
) void {
    _ = time;
    // Фон панели
    fillRect(fb, .{ .x = 0, .y = 0, .w = layout.sidebar_w, .h = layout.height }, C_BG_PANEL);
    drawVLine(fb, layout.sidebar_w, 0, layout.height, C_BORDER);

    // Логотип
    drawText(fb, "P3 ENGINE", 24, 30, 22, C_ACCENT);
    drawText(fb, "v0.14.0", 24, 56, 12, C_TEXT_DIM);

    // Разделитель
    drawHLine(fb, 24, 82, 200, C_ACCENT_DIM);

    // Меню навигации (соответствует LauncherScreen)
    const menu_items = [_]struct { name: []const u8, screen: u8 }{
        .{ .name = "Проекты", .screen = 2 },
        .{ .name = "Новый проект", .screen = 3 },
        .{ .name = "Плагины (Gems)", .screen = 5 },
        .{ .name = "Движок", .screen = 9 },
        .{ .name = "Настройки", .screen = 10 },
        .{ .name = "Репозитории", .screen = 11 },
        .{ .name = "Создать плагин", .screen = 13 },
        .{ .name = "3D Обзор", .screen = 15 },
        .{ .name = "Ассеты", .screen = 16 },
        .{ .name = "Настройки сцены", .screen = 17 },
        .{ .name = "О движке", .screen = 19 },
    };

    var y: f32 = 100;
    for (menu_items) |item| {
        const is_current = (item.screen == current_screen);
        const rect = Rect{ .x = 8, .y = y, .w = layout.sidebar_w - 16, .h = 32 };

        if (is_current) {
            fillRect(fb, rect, C_BG_CARD_HOVER);
            drawRectBorder(fb, rect, C_ACCENT, 1);
            drawText(fb, item.name, 20, y + 8, 14, C_ACCENT);
        } else {
            drawText(fb, item.name, 20, y + 8, 14, C_TEXT_DIM);
        }

        y += 36;
    }

    // Информация внизу
    drawHLine(fb, 24, layout.height - 60, 200, C_BORDER);
    drawText(fb, "CPU Headless", 24, layout.height - 50, 11, C_TEXT_DIM);
    drawText(fb, "ПК-зрение: ВКЛ", 24, layout.height - 35, 11, C_GREEN);
}

// ---------------------------------------------------------------------------
// Экран загрузки (аналог O3DE loading screen)
// ---------------------------------------------------------------------------
pub fn drawLoadingScreen(fb: *VisualFrameBuffer, layout: ScreenLayout, progress: f32, text: []const u8) void {
    drawBackground(fb, 0);

    const cx = layout.width / 2;
    const cy = layout.height / 2;

    // Логотип
    drawTextCentered(fb, "P3 ENGINE", cx, cy - 80, 36, C_ACCENT);
    drawTextCentered(fb, text, cx, cy - 30, 18, C_TEXT);

    // Прогресс-бар
    const bar_w = layout.width * 0.6;
    const bar_h: f32 = 12;
    drawProgressBar(fb, .{ .x = cx - bar_w / 2, .y = cy, .w = bar_w, .h = bar_h }, progress, C_ACCENT);

    // Проценты
    var pct_buf: [16]u8 = undefined;
    const pct_str = std.fmt.bufPrint(&pct_buf, "{d}%", .{@as(u32, @intFromFloat(progress * 100))}) catch "0%";
    drawTextCentered(fb, pct_str, cx, cy + 20, 18, C_TEXT);
}

// ---------------------------------------------------------------------------
// Экран "О движке" (аналог O3DE EngineInfo)
// ---------------------------------------------------------------------------
pub fn drawAboutScreen(fb: *VisualFrameBuffer, layout: ScreenLayout) void {
    drawBackground(fb, 0);

    const cx = layout.width / 2;
    drawTextCentered(fb, "P3 ENGINE", cx, 60, 40, C_ACCENT);
    drawTextCentered(fb, "Проективный 3D геометрический движок", cx, 100, 16, C_TEXT_DIM);

    const items = [_][2][]const u8{
        .{ "Движок:", "P3 Engine" },
        .{ "Версия:", "0.14.0" },
        .{ "Язык:", "Zig 0.14.0" },
        .{ "Модулей:", "60+ (37 000+ строк)" },
        .{ "Тестов:", "700+ (все проходят)" },
        .{ "Рендер:", "Headless CPU rasterizer" },
        .{ "Физика:", "Pacejka + капсула + шаги" },
        .{ "Зрение:", "CV analyzer + tracker + graph" },
        .{ "Анимация:", "LBS + DQS + FABRIK" },
        .{ "Материалы:", "Cook-Torrance PBR + LUT" },
        .{ "Blender:", "OBJ парсер" },
        .{ "LLM:", "Structured JSON (без PNG)" },
        .{ "Лаунчер:", "O3DE-совместимый, русский" },
    };

    var y: f32 = 150;
    for (items) |item| {
        drawText(fb, item[0], cx - 250, y, 16, C_ACCENT);
        drawText(fb, item[1], cx - 100, y, 16, C_TEXT);
        y += 30;
    }

    drawTextCentered(fb, "ESC - назад", cx, layout.height - 40, 14, C_TEXT_DIM);
}

// ---------------------------------------------------------------------------
// Экран проектов (аналог O3DE ProjectsScreen)
// ---------------------------------------------------------------------------
pub fn drawProjectsScreen(
    fb: *VisualFrameBuffer,
    layout: ScreenLayout,
    projects: []const ProjectCard,
    selected: usize,
) void {
    // Заголовок
    const header = ScreenHeader.init(.{ .x = layout.contentX(), .y = 8, .w = layout.contentW(), .h = 40 }, "Проекты");
    header.draw(fb);

    // Кнопка "Создать проект"
    var create_btn = Button{
        .rect = .{ .x = layout.contentX(), .y = 60, .w = 200, .h = 36 },
        .label = "+ Создать проект",
        .accent = C_GREEN,
    };
    create_btn.draw(fb);

    // Кнопка "Открыть проект"
    var open_btn = Button{
        .rect = .{ .x = layout.contentX() + 220, .y = 60, .w = 200, .h = 36 },
        .label = "Открыть проект",
        .accent = C_BLUE,
    };
    open_btn.draw(fb);

    // Список карточек проектов
    var y: f32 = 110;
    for (projects, 0..) |*card, i| {
        card.rect = .{ .x = layout.contentX(), .y = y, .w = layout.contentW(), .h = 70 };
        card.hover = (i == selected);
        card.draw(fb);
        y += 78;
    }

    if (projects.len == 0) {
        drawTextCentered(fb, "Нет проектов. Нажмите 'Создать проект'", layout.contentX() + layout.contentW() / 2, 200, 16, C_TEXT_DIM);
    }
}

// ---------------------------------------------------------------------------
// Экран настроек движка (аналог O3DE EngineSettingsScreen)
// ---------------------------------------------------------------------------
pub fn drawEngineSettingsScreen(
    fb: *VisualFrameBuffer,
    layout: ScreenLayout,
    engine_name: []const u8,
    engine_version: []const u8,
    engine_path: []const u8,
    render_width: u32,
    render_height: u32,
) void {
    const header = ScreenHeader.init(.{ .x = layout.contentX(), .y = 8, .w = layout.contentW(), .h = 40 }, "Настройки движка");
    header.draw(fb);

    var y: f32 = 60;
    const wh: f32 = layout.contentW() * 0.8;

    const items = [_]struct { label: []const u8, value: []const u8 }{
        .{ .label = "Имя движка:", .value = engine_name },
        .{ .label = "Версия:", .value = engine_version },
        .{ .label = "Путь:", .value = engine_path },
        .{ .label = "Рендер:", .value = "Headless CPU (без GPU)" },
    };

    for (items) |item| {
        fillRect(fb, .{ .x = layout.contentX(), .y = y, .w = wh, .h = 40 }, C_BG_CARD);
        drawRectBorder(fb, .{ .x = layout.contentX(), .y = y, .w = wh, .h = 40 }, C_BORDER, 1);
        drawText(fb, item.label, layout.contentX() + 12, y + 12, 14, C_ACCENT);
        drawText(fb, item.value, layout.contentX() + 200, y + 12, 14, C_TEXT);
        y += 44;
    }

    // Дополнительные настройки
    drawText(fb, "Разрешение рендера:", layout.contentX(), y, 14, C_TEXT);
    var res_buf: [32]u8 = undefined;
    const res_str = std.fmt.bufPrint(&res_buf, "{d}x{d}", .{ render_width, render_height }) catch "";
    drawText(fb, res_str, layout.contentX() + 200, y, 14, C_ACCENT);
    y += 30;

    drawText(fb, "ПК-зрение (CV analyzer):", layout.contentX(), y, 14, C_TEXT);
    drawText(fb, "ВКЛЮЧЕНО", layout.contentX() + 200, y, 14, C_GREEN);
    y += 30;

    drawText(fb, "Виртуальный экран:", layout.contentX(), y, 14, C_TEXT);
    drawText(fb, "ВКЛЮЧЕН (VisualFrameBuffer)", layout.contentX() + 200, y, 14, C_GREEN);
}

// ---------------------------------------------------------------------------
// Экран настроек сцены (аналог O3DE Viewport settings)
// ---------------------------------------------------------------------------
pub fn drawSceneSettingsScreen(
    fb: *VisualFrameBuffer,
    layout: ScreenLayout,
    camera_fov: f32,
    sun_intensity: f32,
    gravity_y: f32,
    physics_enabled: bool,
) void {
    const header = ScreenHeader.init(.{ .x = layout.contentX(), .y = 8, .w = layout.contentW(), .h = 40 }, "Настройки сцены");
    header.draw(fb);

    var y: f32 = 60;

    // Камера
    drawText(fb, "Камера", layout.contentX(), y, 18, C_ACCENT);
    y += 30;
    var fov_buf: [16]u8 = undefined;
    const fov_str = std.fmt.bufPrint(&fov_buf, "FOV: {d:.0}°", .{camera_fov}) catch "";
    drawText(fb, fov_str, layout.contentX() + 12, y, 14, C_TEXT);
    drawProgressBar(fb, .{ .x = layout.contentX() + 200, .y = y, .w = 200, .h = 8 }, (camera_fov - 30) / 90, C_BLUE);
    y += 24;

    // Свет
    drawText(fb, "Освещение", layout.contentX(), y, 18, C_ACCENT);
    y += 30;
    var sun_buf: [16]u8 = undefined;
    const sun_str = std.fmt.bufPrint(&sun_buf, "Интенсивность: {d:.2}", .{sun_intensity}) catch "";
    drawText(fb, sun_str, layout.contentX() + 12, y, 14, C_TEXT);
    drawProgressBar(fb, .{ .x = layout.contentX() + 200, .y = y, .w = 200, .h = 8 }, sun_intensity, C_YELLOW);
    y += 30;

    // Физика
    drawText(fb, "Физика", layout.contentX(), y, 18, C_ACCENT);
    y += 30;
    var grav_buf: [16]u8 = undefined;
    const grav_str = std.fmt.bufPrint(&grav_buf, "Гравитация: {d:.2} м/с²", .{gravity_y}) catch "";
    drawText(fb, grav_str, layout.contentX() + 12, y, 14, C_TEXT);
    y += 24;
    drawText(fb, "Движок физики:", layout.contentX() + 12, y, 14, C_TEXT);
    if (physics_enabled) {
        drawText(fb, "ВКЛЮЧЁН", layout.contentX() + 200, y, 14, C_GREEN);
    } else {
        drawText(fb, "ОТКЛЮЧЁН", layout.contentX() + 200, y, 14, C_RED);
    }
}

// ---------------------------------------------------------------------------
// Обзорщик ассетов (аналог O3DE Asset Browser)
// ---------------------------------------------------------------------------
pub fn drawAssetBrowserScreen(
    fb: *VisualFrameBuffer,
    layout: ScreenLayout,
    assets: []const struct { name: []const u8, type: u8 },
    filter_type: u8,
) void {
    const header = ScreenHeader.init(.{ .x = layout.contentX(), .y = 8, .w = layout.contentW(), .h = 40 }, "Ассеты");
    header.draw(fb);

    // Фильтр
    const filters = [_][]const u8{ "Все", "Сцены", "Текстуры", "Материалы", "Модели", "Шейдеры", "Скрипты", "Карты" };
    var fx: f32 = layout.contentX();
    for (filters, 0..) |f, i| {
        const is_active = (i == filter_type);
        var btn = Button{
            .rect = .{ .x = fx, .y = 56, .w = 90, .h = 28 },
            .label = f,
            .accent = if (is_active) C_ACCENT else C_TEXT_DIM,
            .hover = is_active,
            .pressed = is_active,
        };
        btn.draw(fb);
        fx += 95;
    }

    // Список ассетов
    var y: f32 = 95;
    if (assets.len == 0) {
        drawTextCentered(fb, "Ассеты не найдены", layout.contentX() + layout.contentW() / 2, y + 20, 16, C_TEXT_DIM);
        return;
    }
    for (assets) |asset| {
        const rect = Rect{ .x = layout.contentX(), .y = y, .w = layout.contentW(), .h = 28 };
        fillRect(fb, rect, C_BG_CARD);
        drawRectBorder(fb, rect, C_BORDER, 1);

        // Иконка (круг цветом типа)
        const colors = [_]PixelColor{ C_BLUE, C_ACCENT, C_ACCENT2, C_GREEN, C_YELLOW, C_RED };
        const color = colors[asset.type % colors.len];
        drawCircle(fb, rect.x + 14, rect.y + 14, 6, color);

        drawText(fb, asset.name, rect.x + 30, rect.y + 6, 13, C_TEXT);
        y += 32;
    }
}

// ---------------------------------------------------------------------------
// Подвал (статус-бар)
// ---------------------------------------------------------------------------
pub fn drawFooter(fb: *VisualFrameBuffer, layout: ScreenLayout, fps: u32, screen_name: []const u8) void {
    const y = layout.height - layout.footer_h;
    fillRect(fb, .{ .x = 0, .y = y, .w = layout.width, .h = layout.footer_h }, C_BG_PANEL);
    drawHLine(fb, 0, y, layout.width, C_BORDER);

    drawText(fb, "Экран: ", 12, y + 12, 12, C_TEXT_DIM);
    drawText(fb, screen_name, 70, y + 12, 12, C_TEXT);

    var fps_buf: [16]u8 = undefined;
    const fps_str = std.fmt.bufPrint(&fps_buf, "FPS: {d}", .{fps}) catch "";
    drawText(fb, fps_str, layout.width - 100, y + 12, 12, C_TEXT_DIM);
}

// ===========================================================================
// TESTS
// ===========================================================================

test "Launcher UI: Rect contains" {
    const r = Rect.init(10, 20, 100, 50);
    try std.testing.expect(r.contains(50, 40));
    try std.testing.expect(!r.contains(5, 10));
    try std.testing.expect(!r.contains(120, 80));
}

test "Launcher UI: fillRect" {
    const allocator = std.testing.allocator;
    var fb = try VisualFrameBuffer.init(allocator, 32, 32);
    defer fb.deinit();
    fillRect(&fb, .{ .x = 5, .y = 5, .w = 10, .h = 10 }, C_ACCENT);
    // Center pixel should be accent
    const idx = 10 * 32 + 10;
    try std.testing.expectEqual(@as(u8, 0), fb.color_buffer[idx].r); // C_ACCENT.r = 0
    try std.testing.expectEqual(@as(u8, 240), fb.color_buffer[idx].g);
    try std.testing.expectEqual(@as(u8, 255), fb.color_buffer[idx].b);
}

test "Launcher UI: drawCircle" {
    const allocator = std.testing.allocator;
    var fb = try VisualFrameBuffer.init(allocator, 32, 32);
    defer fb.deinit();
    drawCircle(&fb, 16, 16, 5, C_GREEN);
    // Center should be green
    const idx = 16 * 32 + 16;
    try std.testing.expectEqual(@as(u8, 0), fb.color_buffer[idx].r);
    try std.testing.expectEqual(@as(u8, 255), fb.color_buffer[idx].g);
}

test "Launcher UI: Button update + draw" {
    const allocator = std.testing.allocator;
    var fb = try VisualFrameBuffer.init(allocator, 64, 64);
    defer fb.deinit();
    var btn = Button{
        .rect = .{ .x = 10, .y = 10, .w = 40, .h = 20 },
        .label = "Тест",
    };
    btn.update(20, 15, true);
    try std.testing.expect(btn.pressed);
    btn.update(20, 15, false);
    try std.testing.expect(btn.clicked);
    btn.draw(&fb);
    // Should have drawn something
    const idx = 15 * 64 + 20;
    try std.testing.expect(fb.depth_buffer[idx] > 1e7);
}

test "Launcher UI: ProgressBar" {
    const allocator = std.testing.allocator;
    var fb = try VisualFrameBuffer.init(allocator, 64, 16);
    defer fb.deinit();
    drawProgressBar(&fb, .{ .x = 0, .y = 0, .w = 64, .h = 8 }, 0.5, C_ACCENT);
    // Left half should be accent
    try std.testing.expectEqual(@as(u8, 240), fb.color_buffer[4 * 64 + 10].g);
    // Right half should be background
    try std.testing.expect(fb.color_buffer[4 * 64 + 50].g < 100);
}

test "Launcher UI: ProjectCard draw" {
    const allocator = std.testing.allocator;
    var fb = try VisualFrameBuffer.init(allocator, 128, 80);
    defer fb.deinit();
    var card = ProjectCard{
        .rect = .{ .x = 0, .y = 0, .w = 128, .h = 70 },
        .name = "Тест",
        .path = "/tmp/test",
        .template = "Default",
    };
    card.draw(&fb);
    // Should have drawn the accent bar on the left
    try std.testing.expect(fb.depth_buffer[35 * 128 + 2] > 1e7);
}

test "Launcher UI: textWidth" {
    const w = textWidth("Hello", 14);
    try std.testing.expect(w > 0);
    try std.testing.expect(w > 50);
}

test "Launcher UI: ScreenLayout" {
    const layout = ScreenLayout{ .width = 1280, .height = 720 };
    try std.testing.expectApproxEqAbs(layout.contentX(), 276, 1);
    try std.testing.expectApproxEqAbs(layout.contentW(), 988, 1);
}
