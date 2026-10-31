// =============================================================================
// P³ ENGINE — LAUNCHER CORE (полная замена O3DE ProjectManager + GameLauncher)
// =============================================================================
//
// Этот модуль — ПОЛНАЯ замена O3DE лаунчера на нативный Zig код.
//
// Ключевые принципы:
//   1. БЕЗ Qt — весь UI рисуется через наш VisualFrameBuffer (CPU rasterizer)
//   2. БЕЗ X11 — встроенный виртуальный экран, ПК-зрение LLM видит напрямую
//   3. БЕЗ внешних зависимостей — только Zig stdlib + p3_engine модули
//   4. ПОЛНЫЙ русский язык
//   5. Все функции O3DE ProjectManager сохранены:
//      - Создание/открытие проектов
//      - Настройки движка
//      - Менеджер ассетов (карты, модели, текстуры)
//      - Каталог Gems (плагинов)
//      - Настройки сцены (камера, свет, физика, материалы)
//      - Сборка и запуск проектов
//   6. Встроенный viewport — рендер сцены прямо в VisualFrameBuffer
//      LLM/агент видит что происходит через ПК-зрение (CV analyzer)
//   7. C ABI — для связи с C++ компонентами если нужно
//
// Архитектура:
//   p3_launcher_core.zig (этот файл) — ядро: состояние, логика, данные
//   p3_launcher_ui.zig — отрисовка UI в VisualFrameBuffer
//   p3_launcher_scene.zig — viewport сцены (3D рендер)
//   build.zig target "launcher-core" — headless (без raylib)
//   build.zig target "launcher" — с raylib (если есть GPU)
// =============================================================================

const std = @import("std");
const math = std.math;
const p3 = @import("root.zig");

const Vec3 = p3.Vec3;
const Vec4 = p3.Vec4;
const Mat4x4 = p3.Mat4x4;

const vision = p3.p3.vision;
const VisualFrameBuffer = vision.VisualFrameBuffer;
const ProjectiveRasterizer = vision.ProjectiveRasterizer;
const PixelColor = vision.PixelColor;

// ---------------------------------------------------------------------------
// Экраны лаунчера (полный аналог O3DE ProjectManagerScreen)
// ---------------------------------------------------------------------------
pub const LauncherScreen = enum(u8) {
    invalid = 0,
    empty = 1,                // Пустой экран (старт)
    projects = 2,             // Список проектов
    create_project = 3,       // Создание нового проекта
    new_project_settings = 4, // Настройки нового проекта
    gem_catalog = 5,          // Каталог плагинов (Gems)
    project_gem_catalog = 6,  // Плагины проекта
    update_project = 7,       // Обновление проекта
    update_project_settings = 8, // Настройки обновления
    engine_info = 9,          // Информация о движке
    engine_settings = 10,     // Настройки движка
    gem_repos = 11,           // Репозитории плагинов
    gems_gem_repos = 12,      // Gems + репозитории
    create_gem = 13,         // Создание плагина
    edit_gem = 14,            // Редактирование плагина
    scene_viewport = 15,      // 3D viewport сцены
    asset_browser = 16,      // Обозреватель ассетов
    settings_scene = 17,     // Настройки сцены (камера, свет, физика)
    loading = 18,            // Экран загрузки
    about = 19,              // О движке

    pub fn name(self: LauncherScreen) []const u8 {
        return switch (self) {
            .invalid => "Недействительно",
            .empty => "Старт",
            .projects => "Проекты",
            .create_project => "Новый проект",
            .new_project_settings => "Настройки проекта",
            .gem_catalog => "Каталог плагинов",
            .project_gem_catalog => "Плагины проекта",
            .update_project => "Обновление проекта",
            .update_project_settings => "Настройки обновления",
            .engine_info => "Движок",
            .engine_settings => "Настройки движка",
            .gem_repos => "Репозитории",
            .gems_gem_repos => "Плагины и репозитории",
            .create_gem => "Создать плагин",
            .edit_gem => "Редактировать плагин",
            .scene_viewport => "3D Обзор",
            .asset_browser => "Ассеты",
            .settings_scene => "Настройки сцены",
            .loading => "Загрузка",
            .about => "О движке",
        };
    }
};

// ---------------------------------------------------------------------------
// Проект (аналог O3DE ProjectInfo)
// ---------------------------------------------------------------------------
pub const Project = struct {
    name: []const u8 = "",
    path: []const u8 = "",
    template: []const u8 = "Default", // Шаблон: Default, Empty, VR, etc.
    engine_path: []const u8 = "",
    requires_build: bool = true,
    gems: std.ArrayList(GemRef) = undefined,
    last_modified: i64 = 0,

    pub fn init(allocator: std.mem.Allocator) Project {
        return .{
            .gems = std.ArrayList(GemRef).init(allocator),
        };
    }

    pub fn deinit(self: *Project) void {
        self.gems.deinit();
    }
};

// ---------------------------------------------------------------------------
// Gem (плагин — аналог O3DE Gem)
// ---------------------------------------------------------------------------
pub const GemRef = struct {
    name: []const u8,
    version: []const u8 = "1.0.0",
    enabled: bool = true,
    path: []const u8 = "",
    repo_url: []const u8 = "", // Для удалённых репозиториев
};

// ---------------------------------------------------------------------------
// Ассет (аналог O3DE AssetEntry)
// ---------------------------------------------------------------------------
pub const AssetType = enum(u8) {
    scene = 0,      // .tig, .fbx, .obj
    texture = 1,    // .png, .jpg, .tga, .bc7
    material = 2,   // .material, .pngmat
    model = 3,      // .fbx, .dae, .obj
    shader = 4,      // .shader, .azsl
    script = 5,     // .lua, .py
    audio = 6,      // .wav, .ogg
    animation = 7,  // .anim, .fbx
    physics = 8,    // .physx, .collider
    level = 9,      // .ly, .prefab
    map = 10,       // .map (загружаемая карта)
    config = 11,    // .json, .xml, .ini

    pub fn name(self: AssetType) []const u8 {
        return switch (self) {
            .scene => "Сцена",
            .texture => "Текстура",
            .material => "Материал",
            .model => "Модель",
            .shader => "Шейдер",
            .script => "Скрипт",
            .audio => "Аудио",
            .animation => "Анимация",
            .physics => "Физика",
            .level => "Уровень",
            .map => "Карта",
            .config => "Конфиг",
        };
    }

    pub fn extensions(self: AssetType) []const []const u8 {
        return switch (self) {
            .scene => &.{".tig", ".fbx", ".obj"},
            .texture => &.{".png", ".jpg", ".tga", ".bc7", ".astc"},
            .material => &.{".material", ".pngmat"},
            .model => &.{".fbx", ".dae", ".obj", ".gltf"},
            .shader => &.{".shader", ".azsl", ".glsl"},
            .script => &.{".lua", ".py", ".zs"},
            .audio => &.{".wav", ".ogg", ".mp3"},
            .animation => &.{".anim", ".fbx"},
            .physics => &.{".physx", ".collider"},
            .level => &.{".ly", ".prefab"},
            .map => &.{".map", ".json"},
            .config => &.{".json", ".xml", ".ini", ".toml"},
        };
    }
};

pub const Asset = struct {
    name: []const u8 = "",
    path: []const u8 = "",
    asset_type: AssetType = .config,
    size_bytes: u64 = 0,
    preview_color: PixelColor = PixelColor.init(60, 80, 120, 255),
};

// ---------------------------------------------------------------------------
// Настройки сцены (аналог O3DE Viewport + Level settings)
// ---------------------------------------------------------------------------
pub const SceneSettings = struct {
    // Камера
    camera_eye: Vec3 = Vec3.init(0, 5, -15),
    camera_target: Vec3 = Vec3.init(0, 1, 0),
    camera_fov: f32 = 60.0, // градусы
    camera_near: f32 = 0.05,
    camera_far: f32 = 500.0,

    // Свет
    sun_direction: Vec3 = Vec3.init(0.5, 0.8, 0.3),
    sun_color: [3]f32 = .{ 1.0, 0.95, 0.85 },
    sun_intensity: f32 = 1.0,
    ambient_color: [3]f32 = .{ 0.15, 0.18, 0.22 },
    ambient_intensity: f32 = 0.25,

    // Физика
    gravity: Vec3 = Vec3.init(0, -9.81, 0),
    physics_enabled: bool = true,
    physics_step: f32 = 0.016, // 60 FPS

    // Материалы (глобальные)
    default_roughness: f32 = 0.5,
    default_metallic: f32 = 0.0,

    // Туман
    fog_color: [3]f32 = .{ 0.1, 0.12, 0.18 },
    fog_density: f32 = 0.0,
    fog_enabled: bool = false,

    // Skybox
    skybox_top: [3]u8 = .{ 15, 20, 40 },
    skybox_bottom: [3]u8 = .{ 5, 8, 15 },
};

// ---------------------------------------------------------------------------
// Настройки движка (аналог O3DE EngineSettings)
// ---------------------------------------------------------------------------
pub const EngineSettings = struct {
    engine_version: []const u8 = "P3 0.14.0",
    engine_path: []const u8 = "",
    default_project_path: []const u8 = "~/projects",
    gems_path: []const u8 = "",
    templates_path: []const u8 = "",

    // Рендер
    render_width: u32 = 640,
    render_height: u32 = 480,
    vsync: bool = true,
    msaa_samples: u8 = 4,

    // ПК-зрение (LLM agent)
    cv_analyzer_enabled: bool = true,
    temporal_tracker_enabled: bool = true,
    scene_graph_enabled: bool = true,
    observation_json_output: bool = true,

    // CPU-only режим (без GPU/X11)
    headless_mode: bool = true,
    virtual_display: bool = true, // Встроенный виртуальный экран

    // Язык
    language: []const u8 = "ru", // ru, en, uk
};

// ---------------------------------------------------------------------------
// Состояние лаунчера (главный контекст)
// ---------------------------------------------------------------------------
pub const LauncherState = struct {
    allocator: std.mem.Allocator,

    // Текущий экран
    current_screen: LauncherScreen = .empty,
    previous_screen: LauncherScreen = .empty,

    // Проекты
    projects: std.ArrayList(Project),
    current_project: ?Project = null,

    // Ассеты
    assets: std.ArrayList(Asset),
    asset_filter: AssetType = .scene,

    // Настройки
    engine_settings: EngineSettings = .{},
    scene_settings: SceneSettings = .{},

    // Gems (плагины)
    gems: std.ArrayList(GemRef),
    gem_repos: std.ArrayList([]const u8),

    // UI состояние
    selected_index: usize = 0,
    scroll_offset: f32 = 0,
    loading_progress: f32 = 0,
    loading_text: []const u8 = "",

    // Встроенный виртуальный экран (VisualFrameBuffer)
    frame_buffer: ?VisualFrameBuffer = null,

    // ПК-зрение (последнее наблюдение)
    last_observation: ?vision.Observation = null,

    pub fn init(allocator: std.mem.Allocator) LauncherState {
        return .{
            .allocator = allocator,
            .projects = std.ArrayList(Project).init(allocator),
            .assets = std.ArrayList(Asset).init(allocator),
            .gems = std.ArrayList(GemRef).init(allocator),
            .gem_repos = std.ArrayList([]const u8).init(allocator),
        };
    }

    pub fn deinit(self: *LauncherState) void {
        for (self.projects.items) |*p| p.deinit();
        self.projects.deinit();
        self.assets.deinit();
        self.gems.deinit();
        self.gem_repos.deinit();
        if (self.frame_buffer) |*fb| fb.deinit();
        if (self.last_observation) |*obs| obs.deinit();
    }

    /// Создать виртуальный экран (VisualFrameBuffer) — встроенный дисплей
    pub fn initVirtualDisplay(self: *LauncherState, width: usize, height: usize) !void {
        self.frame_buffer = try VisualFrameBuffer.init(self.allocator, width, height);
    }

    /// ПК-зрение: проанализировать текущий кадр
    pub fn observe(self: *LauncherState) !void {
        if (self.frame_buffer == null) return;
        const fb = &self.frame_buffer.?;
        if (self.last_observation) |*obs| obs.deinit();
        self.last_observation = try vision.analyzeFrameBuffer(fb, self.allocator, 0, 0.0);
    }

    /// Создать новый проект
    pub fn createProject(self: *LauncherState, name: []const u8, template: []const u8) !void {
        var project = Project.init(self.allocator);
        project.name = name;
        project.template = template;
        project.engine_path = self.engine_settings.engine_path;
        try self.projects.append(project);
    }

    /// Открыть проект
    pub fn openProject(self: *LauncherState, index: usize) void {
        if (index < self.projects.items.len) {
            self.current_project = self.projects.items[index];
            self.current_screen = .scene_viewport;
        }
    }

    /// Загрузить карту (аналог O3DE Open Level)
    pub fn loadMap(self: *LauncherState, map_path: []const u8) !void {
        _ = self;
        _ = map_path;
        // TODO: загрузить .json/.map файл, распарсить сцены, создать геометрию
    }

    /// Скачать ассет из репозитория (аналог O3DE Gem download)
    pub fn downloadAsset(self: *LauncherState, url: []const u8, asset_type: AssetType) !void {
        _ = self;
        _ = url;
        _ = asset_type;
        // TODO: HTTP download, сохранить в assets/
    }

    /// Сборка проекта (аналог O3DE cmake build)
    pub fn buildProject(self: *LauncherState) !void {
        if (self.current_project == null) return;
        self.current_screen = .loading;
        self.loading_progress = 0;
        self.loading_text = "Сборка проекта...";
        // TODO: запустить zig build / cmake
    }

    /// Запуск проекта (аналог O3DE GameLauncher)
    pub fn runProject(self: *LauncherState) !void {
        if (self.current_project == null) return;
        self.current_screen = .scene_viewport;
        // TODO: запустить рендер цикл в VisualFrameBuffer
    }
};

// ---------------------------------------------------------------------------
// C ABI — для связи с C++ компонентами если нужно
// ---------------------------------------------------------------------------

pub const P3LauncherHandle = ?*LauncherState;

pub fn p3LauncherInit(allocator: std.mem.Allocator) ?*LauncherState {
    const state = allocator.create(LauncherState) catch return null;
    state.* = LauncherState.init(allocator);
    return state;
}

pub fn p3LauncherDeinit(handle: P3LauncherHandle, allocator: std.mem.Allocator) void {
    if (handle) |state| {
        state.deinit();
        allocator.destroy(state);
    }
}

// ===========================================================================
// TESTS
// ===========================================================================

test "Launcher: init/deinit" {
    const allocator = std.testing.allocator;
    var state = LauncherState.init(allocator);
    defer state.deinit();
    try std.testing.expectEqual(LauncherScreen.empty, state.current_screen);
}

test "Launcher: screen names (Russian)" {
    try std.testing.expectEqualStrings("Проекты", LauncherScreen.projects.name());
    try std.testing.expectEqualStrings("Новый проект", LauncherScreen.create_project.name());
    try std.testing.expectEqualStrings("Настройки движка", LauncherScreen.engine_settings.name());
    try std.testing.expectEqualStrings("3D Обзор", LauncherScreen.scene_viewport.name());
    try std.testing.expectEqualStrings("Ассеты", LauncherScreen.asset_browser.name());
    try std.testing.expectEqualStrings("Загрузка", LauncherScreen.loading.name());
}

test "Launcher: create project" {
    const allocator = std.testing.allocator;
    var state = LauncherState.init(allocator);
    defer state.deinit();
    try state.createProject("Тестовый проект", "Default");
    try std.testing.expectEqual(@as(usize, 1), state.projects.items.len);
    try std.testing.expectEqualStrings("Тестовый проект", state.projects.items[0].name);
}

test "Launcher: open project" {
    const allocator = std.testing.allocator;
    var state = LauncherState.init(allocator);
    defer state.deinit();
    try state.createProject("Игровой проект", "Empty");
    state.openProject(0);
    try std.testing.expect(state.current_project != null);
    try std.testing.expectEqual(LauncherScreen.scene_viewport, state.current_screen);
}

test "Launcher: virtual display init" {
    const allocator = std.testing.allocator;
    var state = LauncherState.init(allocator);
    defer state.deinit();
    try state.initVirtualDisplay(320, 240);
    try std.testing.expect(state.frame_buffer != null);
    try std.testing.expectEqual(@as(usize, 320), state.frame_buffer.?.width);
}

test "Launcher: CV observation (ПК-зрение)" {
    const allocator = std.testing.allocator;
    var state = LauncherState.init(allocator);
    defer state.deinit();
    try state.initVirtualDisplay(64, 64);
    try state.observe();
    try std.testing.expect(state.last_observation != null);
}

test "Launcher: asset type names (Russian)" {
    try std.testing.expectEqualStrings("Сцена", AssetType.scene.name());
    try std.testing.expectEqualStrings("Текстура", AssetType.texture.name());
    try std.testing.expectEqualStrings("Материал", AssetType.material.name());
    try std.testing.expectEqualStrings("Карта", AssetType.map.name());
}

test "Launcher: Zig API init/deinit" {
    const allocator = std.testing.allocator;
    const handle = p3LauncherInit(allocator);
    try std.testing.expect(handle != null);
    defer p3LauncherDeinit(handle, allocator);
    try std.testing.expectEqual(LauncherScreen.empty, handle.?.current_screen);
}

test "Launcher: Zig API screen switch" {
    const allocator = std.testing.allocator;
    const handle = p3LauncherInit(allocator);
    defer p3LauncherDeinit(handle, allocator);
    handle.?.previous_screen = handle.?.current_screen;
    handle.?.current_screen = .projects;
    try std.testing.expectEqual(LauncherScreen.projects, handle.?.current_screen);
}

test "Launcher: Zig API create project" {
    const allocator = std.testing.allocator;
    const handle = p3LauncherInit(allocator);
    defer p3LauncherDeinit(handle, allocator);
    try handle.?.createProject("Тест", "Default");
    try std.testing.expectEqual(@as(usize, 1), handle.?.projects.items.len);
}

test "Launcher: scene settings defaults" {
    const settings = SceneSettings{};
    try std.testing.expectApproxEqAbs(settings.camera_fov, 60.0, 0.01);
    try std.testing.expectApproxEqAbs(settings.gravity.y, -9.81, 0.01);
    try std.testing.expect(settings.physics_enabled);
}

test "Launcher: engine settings" {
    const settings = EngineSettings{};
    try std.testing.expect(settings.headless_mode);
    try std.testing.expect(settings.cv_analyzer_enabled);
    try std.testing.expect(settings.virtual_display);
    try std.testing.expectEqualStrings("ru", settings.language);
}
