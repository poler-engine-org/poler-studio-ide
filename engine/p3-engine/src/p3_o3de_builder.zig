// =============================================================================
// P³ ENGINE — O3DE PROCESS & BUILD ORCHESTRATOR
// =============================================================================

const std = @import("std");

pub const BuildProfile = enum {
    debug,
    profile,
    release,

    pub fn asString(self: BuildProfile) []const u8 {
        return switch (self) {
            .debug => "Debug",
            .profile => "profile",
            .release => "Release",
        };
    }
};

pub const ProcessState = enum {
    idle,
    running,
    finished_success,
    finished_error,
};

pub const AsyncProcess = struct {
    allocator: std.mem.Allocator,
    child: ?std.process.Child = null,
    state: ProcessState = .idle,
    log_buffer: std.ArrayList(u8),
    exit_code: ?u8 = null,

    pub fn init(allocator: std.mem.Allocator) AsyncProcess {
        return .{
            .allocator = allocator,
            .child = null,
            .state = .idle,
            .log_buffer = std.ArrayList(u8).init(allocator),
            .exit_code = null,
        };
    }

    pub fn deinit(self: *AsyncProcess) void {
        self.log_buffer.deinit();
    }

    pub fn launchEditor(self: *AsyncProcess, o3de_dir: []const u8, project_dir: []const u8) !void {
        var editor_path_buf: [1024]u8 = undefined;
        const editor_path = try std.fmt.bufPrint(&editor_path_buf, "{s}/bin/Linux/profile/Default/Editor", .{o3de_dir});

        var engine_arg_buf: [1024]u8 = undefined;
        const engine_arg = try std.fmt.bufPrint(&engine_arg_buf, "--engine-path={s}", .{o3de_dir});

        var project_arg_buf: [1024]u8 = undefined;
        const project_arg = try std.fmt.bufPrint(&project_arg_buf, "--project-path={s}", .{project_dir});

        const argv = [_][]const u8{
            editor_path,
            engine_arg,
            project_arg,
        };

        var child = std.process.Child.init(&argv, self.allocator);
        
        // Настройка переменных окружения для дочернего процесса Editor
        var env_map = try std.process.getEnvMap(self.allocator);
        defer env_map.deinit();

        try env_map.put("QT_QPA_PLATFORM", "xcb");
        try env_map.put("XCURSOR_THEME", "breeze_cursors");
        try env_map.put("XCURSOR_SIZE", "24");
        try env_map.put("LD_PRELOAD", "/home/vitalij/Стільниця/P3_Engine_repo/zig-out/lib/libP3_O3DE_Bridge.so");

        child.env_map = &env_map;

        try child.spawn();
        self.child = child;
        self.state = .running;
    }

    pub fn launchGame(self: *AsyncProcess, o3de_dir: []const u8, project_dir: []const u8) !void {
        var launcher_path_buf: [1024]u8 = undefined;
        // Проверяем локально собранный бинарник проекта
        const local_launcher = try std.fmt.bufPrint(&launcher_path_buf, "{s}/build/linux/bin/profile/o3de_game_project.GameLauncher", .{project_dir});
        const launcher_path = if (std.fs.openFileAbsolute(local_launcher, .{})) |file| blk: {
            file.close();
            break :blk local_launcher;
        } else |_| try std.fmt.bufPrint(&launcher_path_buf, "{s}/bin/Linux/profile/Default/O3DE.GameLauncher", .{o3de_dir});

        var engine_arg_buf: [1024]u8 = undefined;
        const engine_arg = try std.fmt.bufPrint(&engine_arg_buf, "--engine-path={s}", .{o3de_dir});

        var project_arg_buf: [1024]u8 = undefined;
        const project_arg = try std.fmt.bufPrint(&project_arg_buf, "--project-path={s}", .{project_dir});

        const argv = [_][]const u8{
            launcher_path,
            engine_arg,
            project_arg,
        };

        var child = std.process.Child.init(&argv, self.allocator);

        var env_map = try std.process.getEnvMap(self.allocator);
        defer env_map.deinit();

        try env_map.put("XCURSOR_THEME", "breeze_cursors");
        try env_map.put("XCURSOR_SIZE", "24");
        try env_map.put("LD_PRELOAD", "/home/vitalij/Стільниця/P3_Engine_repo/zig-out/lib/libP3_O3DE_Bridge.so");

        child.env_map = &env_map;

        try child.spawn();
        self.child = child;
        self.state = .running;
    }

    pub fn launchAssetProcessor(self: *AsyncProcess, o3de_dir: []const u8, project_dir: []const u8) !void {
        var ap_path_buf: [1024]u8 = undefined;
        const ap_path = try std.fmt.bufPrint(&ap_path_buf, "{s}/bin/Linux/profile/Default/AssetProcessor", .{o3de_dir});

        var engine_arg_buf: [1024]u8 = undefined;
        const engine_arg = try std.fmt.bufPrint(&engine_arg_buf, "--engine-path={s}", .{o3de_dir});

        var project_arg_buf: [1024]u8 = undefined;
        const project_arg = try std.fmt.bufPrint(&project_arg_buf, "--project-path={s}", .{project_dir});

        const argv = [_][]const u8{
            ap_path,
            engine_arg,
            project_arg,
        };

        var child = std.process.Child.init(&argv, self.allocator);
        try child.spawn();
        self.child = child;
        self.state = .running;
    }
};
