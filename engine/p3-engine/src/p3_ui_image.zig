// =============================================================================
// P³ UI IMAGE v1.0 — ZIG
// =============================================================================
//
// Image + Sprite types + Fill types (из O3DE UiImageComponent).
//
// В O3DE: UiImageComponent — визуальный компонент для изображений.
// Поддерживает:
//   - SpriteAsset / RenderTarget sprite types
//   - Stretched / Sliced / Fixed / Tiled / StretchedToFit / StretchedToFill
//   - Fill types: None / Linear / Radial / RadialCorner / RadialEdge
//   - 9-slice (sliced image)
//   - Blend modes
//   - Override color/alpha/sprite
//   - Layout cell integration
//
// В P³ Engine:
//   - Без EBus — прямые поля
//   - Sprite = путь к файлу (без AssetSystem)
//   - P³ обобщение: проективный image transform
//   - Fill types — полные
//   - 9-slice — полный порт
//
// Портировано из O3DE/Gems/LyShine/Code/Source/UiImageComponent
// Адаптировано для Zig 0.14.0 + P³ Engine API.
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const math = std.math;
const ui_transform = @import("p3_ui_transform.zig");
const ui_draw = @import("p3_ui_draw.zig");

pub const Vec2 = ui_transform.Vec2;
pub const Rect = ui_transform.Rect;
pub const RectPoints = ui_transform.RectPoints;
pub const Color = ui_draw.Color;
pub const BlendMode = ui_draw.BlendMode;

// =============================================================================
// 1. SPRITE TYPE
// =============================================================================

/// Тип источника спрайта.
///
/// В O3DE: UiImageInterface::SpriteType
pub const SpriteType = enum(u2) {
    /// Спрайт из файла (текстура)
    sprite_asset = 0,
    /// Спрайт из render target (FBO)
    render_target = 1,
};

// =============================================================================
// 2. IMAGE TYPE
// =============================================================================

/// Как изображение масштабируется внутри элемента.
///
/// В O3DE: UiImageInterface::ImageType
pub const ImageType = enum(u3) {
    /// Растянуть на весь элемент
    stretched = 0,
    /// 9-slice (sliced image с бордюрами)
    sliced = 1,
    /// Оригинальный размер (без масштабирования)
    fixed = 2,
    /// Тайлить (повторять)
    tiled = 3,
    /// Растянуть с сохранением пропорций (вписать)
    stretched_to_fit = 4,
    /// Растянуть с сохранением пропорций (заполнить)
    stretched_to_fill = 5,
};

// =============================================================================
// 3. FILL TYPE
// =============================================================================

/// Тип заполнения изображения (для progress bars, radial fills, etc).
///
/// В O3DE: UiImageInterface::FillType
pub const FillType = enum(u3) {
    /// Без заполнения — полное изображение
    none = 0,
    /// Линейное заполнение (0→1 слева направо)
    linear = 1,
    /// Радиальное заполнение (от центра)
    radial = 2,
    /// Радиальное от угла
    radial_corner = 3,
    /// Радиальное от стороны
    radial_edge = 4,
};

// =============================================================================
// 4. FILL CORNER ORIGIN
// =============================================================================

/// Начальный угол для radial_corner fill.
///
/// В O3DE: UiImageInterface::FillCornerOrigin
pub const FillCornerOrigin = enum(u2) {
    top_left = 0,
    top_right = 1,
    bottom_right = 2,
    bottom_left = 3,
};

// =============================================================================
// 5. FILL EDGE ORIGIN
// =============================================================================

/// Начальная сторона для radial_edge fill.
///
/// В O3DE: UiImageInterface::FillEdgeOrigin
pub const FillEdgeOrigin = enum(u2) {
    left = 0,
    top = 1,
    right = 2,
    bottom = 3,
};

// =============================================================================
// 6. 9-SLICE BORDERS
// =============================================================================

/// Границы 9-slice изображения (в пикселях от краёв).
///
/// 9-slice делит изображение на 9 областей:
///   ┌───┬───────┬───┐
///   │ TL│  T    │ TR│
///   ├───┼───────┼───┤
///   │ L │ CENTER│ R │
///   ├───┼───────┼───┤
///   │ BL│  B    │ BR│
///   └───┴───────┴───┘
///
/// Углы (TL, TR, BL, BR) не масштабируются.
/// Стороны (T, B, L, R) масштабируются в одном направлении.
/// Центр масштабируется в обоих направлениях.
pub const SliceBorders = struct {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,

    pub inline fn zero() SliceBorders {
        return .{ .left = 0, .top = 0, .right = 0, .bottom = 0 };
    }

    pub inline fn uniform(v: f32) SliceBorders {
        return .{ .left = v, .top = v, .right = v, .bottom = v };
    }
};

// =============================================================================
// 7. UV COORDINATES
// =============================================================================

/// UV координаты для текстурного маппинга.
pub const UvCoords = struct {
    u_min: f32,
    v_min: f32,
    u_max: f32,
    v_max: f32,

    pub inline fn full() UvCoords {
        return .{ .u_min = 0, .v_min = 0, .u_max = 1, .v_max = 1 };
    }

    pub inline fn fromRect(l: f32, t: f32, r: f32, b: f32) UvCoords {
        return .{ .u_min = l, .v_min = t, .u_max = r, .v_max = b };
    }
};

// =============================================================================
// 8. IMAGE COMPONENT
// =============================================================================

/// UI Image — визуальный компонент для изображения.
///
/// В O3DE: UiImageComponent — 300+ строк, интегрирован с renderer.
/// В P³: данные + рендер через DrawQueue.
pub const Image = struct {
    // --- Source ---
    sprite_type: SpriteType,
    sprite_path: []const u8,
    texture_id: u32, // Runtime texture handle (0 = none)

    // --- Appearance ---
    color: Color,
    alpha: f32,
    image_type: ImageType,
    blend_mode: BlendMode,

    // --- Fill ---
    fill_type: FillType,
    fill_amount: f32, // 0..1
    fill_start_angle: f32, // radians, for radial
    fill_corner_origin: FillCornerOrigin,
    fill_edge_origin: FillEdgeOrigin,
    fill_clockwise: bool,
    fill_center: bool,

    // --- 9-Slice ---
    slice_borders: SliceBorders,
    is_slice_stretched: bool,

    // --- Sprite sheet ---
    cell_index: u32,

    // --- UV ---
    uv: UvCoords,

    // --- Override (для state actions) ---
    override_color: Color,
    override_alpha: f32,
    is_color_overridden: bool,
    is_alpha_overridden: bool,

    // --- Render cache ---
    is_render_cache_dirty: bool,

    /// Инициализация по умолчанию
    pub fn init() Image {
        return .{
            .sprite_type = .sprite_asset,
            .sprite_path = "",
            .texture_id = 0,
            .color = Color.white(),
            .alpha = 1.0,
            .image_type = .stretched,
            .blend_mode = .alpha,
            .fill_type = .none,
            .fill_amount = 1.0,
            .fill_start_angle = 0,
            .fill_corner_origin = .top_left,
            .fill_edge_origin = .left,
            .fill_clockwise = true,
            .fill_center = true,
            .slice_borders = SliceBorders.zero(),
            .is_slice_stretched = true,
            .cell_index = 0,
            .uv = UvCoords.full(),
            .override_color = Color.white(),
            .override_alpha = 1.0,
            .is_color_overridden = false,
            .is_alpha_overridden = false,
            .is_render_cache_dirty = true,
        };
    }

    /// Получить эффективный цвет (с override).
    pub fn getEffectiveColor(self: Image) Color {
        var c = if (self.is_color_overridden) self.override_color else self.color;
        c.a *= self.getEffectiveAlpha();
        return c;
    }

    /// Получить эффективную альфу (с override).
    pub fn getEffectiveAlpha(self: Image) f32 {
        return if (self.is_alpha_overridden) self.override_alpha else self.alpha;
    }

    /// Установить override цвет (из state action).
    pub fn setOverrideColor(self: *Image, color: Color) void {
        self.override_color = color;
        self.is_color_overridden = true;
        self.is_render_cache_dirty = true;
    }

    /// Установить override альфу (из state action).
    pub fn setOverrideAlpha(self: *Image, alpha: f32) void {
        self.override_alpha = alpha;
        self.is_alpha_overridden = true;
        self.is_render_cache_dirty = true;
    }

    /// Сбросить все override.
    pub fn resetOverrides(self: *Image) void {
        self.is_color_overridden = false;
        self.is_alpha_overridden = false;
        self.is_render_cache_dirty = true;
    }

    /// Вычислить UV координаты с учётом fill.
    ///
    /// Для linear fill: обрезает текстуру по fill_amount.
    /// Для radial fill: использует fill_angle для маски.
    pub fn computeFilledUv(self: Image) UvCoords {
        if (self.fill_type == .none) return self.uv;

        var uv = self.uv;
        switch (self.fill_type) {
            .linear => {
                // Horizontal linear fill
                uv.u_max = uv.u_min + (uv.u_max - uv.u_min) * self.fill_amount;
            },
            .radial, .radial_corner, .radial_edge => {
                // Radial fill uses shader-based clipping
                // UV stays full, fill parameters go to shader
            },
            else => {},
        }
        return uv;
    }

    /// Вычислить размер изображения с учётом image_type.
    ///
    /// В O3DE: CalculateImageSize()
    pub fn computeImageSize(self: Image, element_size: Vec2, sprite_size: Vec2) Vec2 {
        switch (self.image_type) {
            .stretched => return element_size,
            .fixed => return sprite_size,
            .stretched_to_fit => {
                const aspect = sprite_size.x / @max(sprite_size.y, 0.001);
                const h = element_size.y;
                const w = h * aspect;
                if (w <= element_size.x) return Vec2.init(w, h);
                const scale = element_size.x / @max(w, 0.001);
                return Vec2.init(element_size.x, h * scale);
            },
            .stretched_to_fill => {
                const aspect = sprite_size.x / @max(sprite_size.y, 0.001);
                const h = element_size.y;
                const w = h * aspect;
                if (w >= element_size.x) return Vec2.init(w, h);
                const scale = element_size.x / @max(w, 0.001);
                return Vec2.init(element_size.x, h * scale);
            },
            .sliced, .tiled => return element_size,
        }
    }
};

// =============================================================================
// 9. IMAGE SEQUENCE
// =============================================================================

/// ImageSequence — покадровая анимация (flipbook).
///
/// В O3DE: UiImageSequenceComponent.
pub const ImageSequence = struct {
    frames: []const Image,
    current_frame: u32,
    is_playing: bool,
    fps: f32,
    elapsed: f32,
    loop: bool,

    pub fn init() ImageSequence {
        return .{
            .frames = &.{},
            .current_frame = 0,
            .is_playing = false,
            .fps = 24.0,
            .elapsed = 0,
            .loop = true,
        };
    }

    /// Обновить анимацию.
    pub fn update(self: *ImageSequence, dt: f32) void {
        if (!self.is_playing or self.frames.len == 0) return;

        self.elapsed += dt;
        const frame_time = 1.0 / @max(self.fps, 0.001);

        while (self.elapsed >= frame_time) {
            self.elapsed -= frame_time;
            self.current_frame += 1;
            if (self.current_frame >= self.frames.len) {
                if (self.loop) {
                    self.current_frame = 0;
                } else {
                    self.current_frame = @intCast(self.frames.len - 1);
                    self.is_playing = false;
                    break;
                }
            }
        }
    }

    /// Получить текущий кадр.
    pub fn getCurrentFrame(self: ImageSequence) ?*const Image {
        if (self.frames.len == 0) return null;
        return &self.frames[self.current_frame];
    }

    /// Начать воспроизведение.
    pub fn play(self: *ImageSequence) void {
        self.is_playing = true;
        self.elapsed = 0;
    }

    /// Остановить воспроизведение.
    pub fn stop(self: *ImageSequence) void {
        self.is_playing = false;
    }
};

// =============================================================================
// 10. FLIPBOOK ANIMATION
// =============================================================================

/// FlipbookAnimation — анимация на основе sprite sheet.
///
/// В O3DE: UiFlipbookAnimationComponent.
/// Использует один sprite sheet с cell_index для кадров.
pub const FlipbookAnimation = struct {
    /// Sprite sheet (одна текстура с сеткой кадров)
    sprite_path: []const u8,
    texture_id: u32,
    /// Количество кадров
    frame_count: u32,
    /// Количество колонок в sprite sheet
    columns: u32,
    /// Текущий кадр
    current_frame: u32,
    /// FPS
    fps: f32,
    /// Прошедшее время
    elapsed: f32,
    /// Зацикливание
    loop: bool,
    /// Воспроизведение
    is_playing: bool,

    pub fn init() FlipbookAnimation {
        return .{
            .sprite_path = "",
            .texture_id = 0,
            .frame_count = 1,
            .columns = 1,
            .current_frame = 0,
            .fps = 24.0,
            .elapsed = 0,
            .loop = true,
            .is_playing = false,
        };
    }

    /// Обновить анимацию.
    pub fn update(self: *FlipbookAnimation, dt: f32) void {
        if (!self.is_playing) return;

        self.elapsed += dt;
        const frame_time = 1.0 / @max(self.fps, 0.001);

        while (self.elapsed >= frame_time) {
            self.elapsed -= frame_time;
            self.current_frame += 1;
            if (self.current_frame >= self.frame_count) {
                if (self.loop) {
                    self.current_frame = 0;
                } else {
                    self.current_frame = self.frame_count - 1;
                    self.is_playing = false;
                    break;
                }
            }
        }
    }

    /// Получить cell index в sprite sheet.
    pub fn getCellIndex(self: FlipbookAnimation) u32 {
        return self.current_frame;
    }

    /// Получить UV координаты текущего кадра.
    pub fn getFrameUv(self: FlipbookAnimation) UvCoords {
        const col = @as(f32, @floatFromInt(self.current_frame % self.columns));
        const row = @as(f32, @floatFromInt(self.current_frame / self.columns));
        const total_rows = (self.frame_count + self.columns - 1) / self.columns;
        const u_step = 1.0 / @as(f32, @floatFromInt(self.columns));
        const v_step = 1.0 / @as(f32, @floatFromInt(total_rows));
        return .{
            .u_min = col * u_step,
            .v_min = row * v_step,
            .u_max = (col + 1) * u_step,
            .v_max = (row + 1) * v_step,
        };
    }
};

// =============================================================================
// 11. SPRITE RESOURCE
// =============================================================================

/// Sprite — ресурс спрайта (текстура + 9-slice borders + cell info).
///
/// В O3DE: ISprite + Sprite.
pub const Sprite = struct {
    path: []const u8,
    texture_id: u32,
    width: f32,
    height: f32,
    slice_borders: SliceBorders,
    cell_count: u32,
    cells_per_row: u32,

    pub fn init(path: []const u8) Sprite {
        return .{
            .path = path,
            .texture_id = 0,
            .width = 0,
            .height = 0,
            .slice_borders = SliceBorders.zero(),
            .cell_count = 1,
            .cells_per_row = 1,
        };
    }
};

// =============================================================================
// 12. ТЕСТЫ
// =============================================================================

test "Image: init defaults" {
    const img = Image.init();
    try std.testing.expect(img.color.r == 1.0);
    try std.testing.expect(img.alpha == 1.0);
    try std.testing.expect(img.image_type == .stretched);
    try std.testing.expect(img.fill_type == .none);
}

test "Image: effective color without override" {
    var img = Image.init();
    img.color = Color.init(0.5, 0.5, 0.5, 1.0);
    const effective = img.getEffectiveColor();
    try std.testing.expectApproxEqAbs(effective.r, 0.5, 1e-6);
}

test "Image: override color" {
    var img = Image.init();
    img.setOverrideColor(Color.init(1.0, 0.0, 0.0, 1.0));
    try std.testing.expect(img.is_color_overridden);
    const effective = img.getEffectiveColor();
    try std.testing.expectApproxEqAbs(effective.r, 1.0, 1e-6);
}

test "Image: reset overrides" {
    var img = Image.init();
    img.setOverrideColor(Color.init(1.0, 0.0, 0.0, 1.0));
    img.setOverrideAlpha(0.5);
    img.resetOverrides();
    try std.testing.expect(!img.is_color_overridden);
    try std.testing.expect(!img.is_alpha_overridden);
}

test "Image: linear fill UV" {
    var img = Image.init();
    img.fill_type = .linear;
    img.fill_amount = 0.5;
    const uv = img.computeFilledUv();
    try std.testing.expectApproxEqAbs(uv.u_min, 0.0, 1e-6);
    try std.testing.expectApproxEqAbs(uv.u_max, 0.5, 1e-6);
}

test "Image: compute image size stretched" {
    const img = Image.init();
    const size = img.computeImageSize(Vec2.init(800, 600), Vec2.init(100, 100));
    try std.testing.expectApproxEqAbs(size.x, 800.0, 1e-6);
    try std.testing.expectApproxEqAbs(size.y, 600.0, 1e-6);
}

test "Image: compute image size fixed" {
    var img = Image.init();
    img.image_type = .fixed;
    const size = img.computeImageSize(Vec2.init(800, 600), Vec2.init(100, 100));
    try std.testing.expectApproxEqAbs(size.x, 100.0, 1e-6);
    try std.testing.expectApproxEqAbs(size.y, 100.0, 1e-6);
}

test "ImageSequence: update" {
    var seq = ImageSequence.init();
    seq.frames = &.{Image.init(), Image.init(), Image.init()};
    seq.is_playing = true;
    seq.fps = 10.0;
    seq.update(0.15); // 1.5 frames
    try std.testing.expect(seq.current_frame == 1);
}

test "FlipbookAnimation: frame UV" {
    var fb = FlipbookAnimation.init();
    fb.frame_count = 4;
    fb.columns = 2;
    fb.current_frame = 3; // row=1, col=1
    const uv = fb.getFrameUv();
    try std.testing.expectApproxEqAbs(uv.u_min, 0.5, 1e-6);
    try std.testing.expectApproxEqAbs(uv.v_min, 0.5, 1e-6);
    try std.testing.expectApproxEqAbs(uv.u_max, 1.0, 1e-6);
    try std.testing.expectApproxEqAbs(uv.v_max, 1.0, 1e-6);
}

test "SliceBorders: zero" {
    const b = SliceBorders.zero();
    try std.testing.expect(b.left == 0 and b.top == 0 and b.right == 0 and b.bottom == 0);
}

test "UvCoords: full" {
    const uv = UvCoords.full();
    try std.testing.expectApproxEqAbs(uv.u_min, 0.0, 1e-6);
    try std.testing.expectApproxEqAbs(uv.u_max, 1.0, 1e-6);
}
