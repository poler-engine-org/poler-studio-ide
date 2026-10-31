// =============================================================================
// P³ ENGINE — NATIVE REPLACEMENT FOR O3DE LauncherUnified (ZIG 0.14)
// =============================================================================

const std = @import("std");
const posix = std.posix;
const o3de_manifest = @import("p3_o3de_manifest.zig");
const o3de_spawnable = @import("p3_o3de_spawnable.zig");
const o3de_asset_system = @import("p3_o3de_asset_system.zig");

const c = @cImport({
    @cInclude("sys/resource.h");
});

pub const ReturnCode = enum(u8) {
    success = 0,
    err_exe_path = 1,
    err_command_line = 2,
    err_validation = 3,
    err_resource_limit = 4,
    err_app_descriptor = 5,
    err_cry_system_lib = 6,
    err_cry_system_interface = 7,
    err_cry_environment = 8,
    err_asset_processor = 9,
    err_unit_test_failure = 10,
    err_unit_test_not_supported = 11,

    pub fn toString(self: ReturnCode) []const u8 {
        return switch (self) {
            .success => "Success",
            .err_exe_path => "Failed to get executable path",
            .err_command_line => "Failed to copy command line",
            .err_validation => "Failed to validate secret",
            .err_resource_limit => "Failed to increase unix resource limits",
            .err_app_descriptor => "Failed to locate application descriptor",
            .err_cry_system_lib => "Failed to load CrySystem library",
            .err_cry_system_interface => "Failed to create CrySystem interface",
            .err_cry_environment => "Failed to initialize CryEngine environment",
            .err_asset_processor => "Failed to connect to asset processor",
            .err_unit_test_failure => "Unit test failed",
            .err_unit_test_not_supported => "Unit tests not supported",
        };
    }
};

/// Нативная замена O3DELauncher::IncreaseResourceLimits
pub fn increaseResourceLimits() bool {
    var rlim: c.rlimit = undefined;
    if (c.getrlimit(c.RLIMIT_NOFILE, &rlim) == 0) {
        rlim.rlim_cur = @max(rlim.rlim_cur, 8192);
        rlim.rlim_max = @max(rlim.rlim_max, 8192);
        _ = c.setrlimit(c.RLIMIT_NOFILE, &rlim);
    }

    if (c.getrlimit(c.RLIMIT_STACK, &rlim) == 0) {
        rlim.rlim_cur = @max(rlim.rlim_cur, 16 * 1024 * 1024);
        _ = c.setrlimit(c.RLIMIT_STACK, &rlim);
    }
    return true;
}

pub const LauncherConfig = struct {
    engine_path: []const u8 = "/opt/O3DE/26.05",
    project_path: []const u8 = "/home/vitalij/o3de_game_project",
    project_name: []const u8 = "o3de_game_project",
    level_name: []const u8 = "defaultlevel",
    is_server: bool = false,
    wait_for_asset_processor: bool = false,
};

pub const NativeO3DELauncher = struct {
    allocator: std.mem.Allocator,
    config: LauncherConfig,
    asset_system: ?o3de_asset_system.NativeAssetSystem = null,
    spawnable_scene: ?o3de_spawnable.Spawnable = null,

    pub fn init(allocator: std.mem.Allocator, config: LauncherConfig) NativeO3DELauncher {
        return .{
            .allocator = allocator,
            .config = config,
            .asset_system = null,
            .spawnable_scene = null,
        };
    }

    pub fn deinit(self: *NativeO3DELauncher) void {
        if (self.asset_system) |*as| as.deinit();
        if (self.spawnable_scene) |*ss| ss.deinit();
    }

    /// Нативный Run() аналог O3DELauncher::Run
    pub fn run(self: *NativeO3DELauncher) ReturnCode {
        const stderr = std.io.getStdErr().writer();

        // 1. Лимиты ОС
        stderr.print("   [1/4] Установка лимитов ОС (RLIMIT_NOFILE/STACK)... ", .{}) catch {};
        if (!increaseResourceLimits()) {
            stderr.print("FAIL\n", .{}) catch {};
            return .err_resource_limit;
        }
        stderr.print("OK\n", .{}) catch {};

        // 2. Инициализация VFS / Asset System
        stderr.print("   [2/4] NativeAssetSystem.init (project_root={s})... ", .{self.config.project_path}) catch {};
        self.asset_system = o3de_asset_system.NativeAssetSystem.init(self.allocator, self.config.project_path) catch |err| {
            stderr.print("FAIL ({s})\n", .{@errorName(err)}) catch {};
            return .err_asset_processor;
        };
        stderr.print("OK (cache_root={s})\n", .{self.asset_system.?.cache_root}) catch {};

        // 3. Загрузка стартового уровня сцены (.spawnable)
        var level_path_buf: [512]u8 = undefined;
        const level_path = std.fmt.bufPrint(&level_path_buf, "{s}/Cache/linux/levels/{s}/{s}.spawnable", .{
            self.config.project_path,
            self.config.level_name,
            self.config.level_name,
        }) catch return .err_app_descriptor;

        stderr.print("   [3/4] Spawnable.loadFromFile({s})... ", .{level_path}) catch {};
        var spawnable = o3de_spawnable.Spawnable.init(self.allocator);
        // Zig's `catch` is an expression — when loadFromFile succeeds, the
        // catch block is skipped silently. We use a flag to distinguish
        // success vs fallback so the log line reflects reality.
        var file_loaded = true;
        spawnable.loadFromFile(level_path) catch |err| {
            // Fallback when .spawnable file does not exist (no project
            // initialised yet). MUST allocator.dupe the literal — otherwise
            // SpawnableEntity.deinit would call c_allocator.free() on a
            // pointer into read-only .rodata, which is undefined behavior
            // and on glibc/Linux typically raises SIGSEGV (exit 139).
            file_loaded = false;
            stderr.print("miss ({s}) → fallback DefaultRootEntity\n", .{@errorName(err)}) catch {};
            const fallback_name = self.allocator.dupe(u8, "DefaultRootEntity") catch {
                self.spawnable_scene = spawnable;
                return .err_app_descriptor;
            };
            spawnable.entities.append(.{
                .id = 1,
                .name = fallback_name,
                .is_active = true,
            }) catch {
                self.allocator.free(fallback_name);
                self.spawnable_scene = spawnable;
                return .err_app_descriptor;
            };
        };
        if (file_loaded) {
            stderr.print("OK ({d} entities)\n", .{spawnable.entities.items.len}) catch {};
        }
        self.spawnable_scene = spawnable;

        // 4. Готово (фаза 5: открыть окно, рендер-луп — пока не реализовано)
        stderr.print("   [4/4] Готово. Рендер-луп Qt5/raylib — TODO (см. ROADMAP.md)\n", .{}) catch {};

        return .success;
    }
};

// =============================================================================
// C-ABI EXPORTS (Для полной бинарной совместимости с O3DE C++ Launcher)
// =============================================================================

export fn O3DELauncher_IncreaseResourceLimits() bool {
    return increaseResourceLimits();
}

export fn O3DELauncher_RunNative(engine_path: [*:0]const u8, project_path: [*:0]const u8) u8 {
    const allocator = std.heap.c_allocator;
    const ep = std.mem.span(engine_path);
    const pp = std.mem.span(project_path);

    var launcher = NativeO3DELauncher.init(allocator, .{
        .engine_path = ep,
        .project_path = pp,
    });
    defer launcher.deinit();

    return @intFromEnum(launcher.run());
}
