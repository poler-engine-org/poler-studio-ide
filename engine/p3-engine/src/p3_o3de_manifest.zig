// =============================================================================
// P³ ENGINE — O3DE MANIFEST & PROJECT PARSER ENGINE
// =============================================================================
// Нативный парсер и генератор конфигураций:
//   - ~/.o3de/o3de_manifest.json (Глобальный реестр проектов, движков, repos)
//   - project.json (Конфигурация проекта, UUID, подключенные Gems)
//   - gem.json (Манифест плагина, зависимости, теги, метаданные)
//   - engine.json (Спецификация движка)
// =============================================================================

const std = @import("std");

pub const GemInfo = struct {
    gem_name: []const u8 = "",
    display_name: []const u8 = "",
    version: []const u8 = "",
    summary: []const u8 = "",
    origin: []const u8 = "",
    license: []const u8 = "",
    path: []const u8 = "",
    enabled: bool = false,
    dependencies: [][]const u8 = &[_][]const u8{},
    user_tags: [][]const u8 = &[_][]const u8{},

    pub fn deinit(self: *GemInfo, allocator: std.mem.Allocator) void {
        if (self.gem_name.len > 0) allocator.free(self.gem_name);
        if (self.display_name.len > 0) allocator.free(self.display_name);
        if (self.version.len > 0) allocator.free(self.version);
        if (self.summary.len > 0) allocator.free(self.summary);
        if (self.origin.len > 0) allocator.free(self.origin);
        if (self.license.len > 0) allocator.free(self.license);
        if (self.path.len > 0) allocator.free(self.path);
        for (self.dependencies) |dep| allocator.free(dep);
        if (self.dependencies.len > 0) allocator.free(self.dependencies);
        for (self.user_tags) |tag| allocator.free(tag);
        if (self.user_tags.len > 0) allocator.free(self.user_tags);
    }
};

pub const ProjectInfo = struct {
    project_name: []const u8 = "",
    display_name: []const u8 = "",
    project_id: []const u8 = "",
    path: []const u8 = "",
    summary: []const u8 = "",
    engine: []const u8 = "",
    engine_version: []const u8 = "",
    icon_path: []const u8 = "",
    gem_names: [][]const u8 = &[_][]const u8{},
    is_valid: bool = false,

    pub fn deinit(self: *ProjectInfo, allocator: std.mem.Allocator) void {
        if (self.project_name.len > 0) allocator.free(self.project_name);
        if (self.display_name.len > 0) allocator.free(self.display_name);
        if (self.project_id.len > 0) allocator.free(self.project_id);
        if (self.path.len > 0) allocator.free(self.path);
        if (self.summary.len > 0) allocator.free(self.summary);
        if (self.engine.len > 0) allocator.free(self.engine);
        if (self.engine_version.len > 0) allocator.free(self.engine_version);
        if (self.icon_path.len > 0) allocator.free(self.icon_path);
        for (self.gem_names) |gem| allocator.free(gem);
        if (self.gem_names.len > 0) allocator.free(self.gem_names);
    }
};

pub const O3deManifest = struct {
    o3de_manifest_name: []const u8 = "",
    default_projects_folder: []const u8 = "",
    default_gems_folder: []const u8 = "",
    default_templates_folder: []const u8 = "",
    engines: [][]const u8 = &[_][]const u8{},
    projects: [][]const u8 = &[_][]const u8{},
    repos: [][]const u8 = &[_][]const u8{},

    pub fn deinit(self: *O3deManifest, allocator: std.mem.Allocator) void {
        if (self.o3de_manifest_name.len > 0) allocator.free(self.o3de_manifest_name);
        if (self.default_projects_folder.len > 0) allocator.free(self.default_projects_folder);
        if (self.default_gems_folder.len > 0) allocator.free(self.default_gems_folder);
        if (self.default_templates_folder.len > 0) allocator.free(self.default_templates_folder);
        for (self.engines) |e| allocator.free(e);
        if (self.engines.len > 0) allocator.free(self.engines);
        for (self.projects) |p| allocator.free(p);
        if (self.projects.len > 0) allocator.free(self.projects);
        for (self.repos) |r| allocator.free(r);
        if (self.repos.len > 0) allocator.free(self.repos);
    }
};

/// Загрузка ~/.o3de/o3de_manifest.json
pub fn loadGlobalManifest(allocator: std.mem.Allocator) !O3deManifest {
    const home = std.posix.getenv("HOME") orelse return error.NoHomeDir;
    var path_buf: [1024]u8 = undefined;
    const manifest_path = try std.fmt.bufPrint(&path_buf, "{s}/.o3de/o3de_manifest.json", .{home});

    const file = std.fs.openFileAbsolute(manifest_path, .{}) catch |err| {
        if (err == error.FileNotFound) {
            return O3deManifest{};
        }
        return err;
    };
    defer file.close();

    const content = try file.readToEndAlloc(allocator, 1024 * 1024);
    defer allocator.free(content);

    var parsed = try std.json.parseFromSlice(std.json.Value, allocator, content, .{});
    defer parsed.deinit();

    var manifest = O3deManifest{};
    const root = parsed.value;

    if (root.object.get("o3de_manifest_name")) |val| {
        if (val == .string) manifest.o3de_manifest_name = try allocator.dupe(u8, val.string);
    }
    if (root.object.get("default_projects_folder")) |val| {
        if (val == .string) manifest.default_projects_folder = try allocator.dupe(u8, val.string);
    }
    if (root.object.get("default_gems_folder")) |val| {
        if (val == .string) manifest.default_gems_folder = try allocator.dupe(u8, val.string);
    }
    if (root.object.get("default_templates_folder")) |val| {
        if (val == .string) manifest.default_templates_folder = try allocator.dupe(u8, val.string);
    }

    if (root.object.get("projects")) |val| {
        if (val == .array) {
            var list = std.ArrayList([]const u8).init(allocator);
            for (val.array.items) |item| {
                if (item == .string) {
                    try list.append(try allocator.dupe(u8, item.string));
                }
            }
            manifest.projects = try list.toOwnedSlice();
        }
    }

    if (root.object.get("engines")) |val| {
        if (val == .array) {
            var list = std.ArrayList([]const u8).init(allocator);
            for (val.array.items) |item| {
                if (item == .string) {
                    try list.append(try allocator.dupe(u8, item.string));
                }
            }
            manifest.engines = try list.toOwnedSlice();
        }
    }

    return manifest;
}

/// Загрузка project.json по пути проекта
pub fn loadProjectJson(allocator: std.mem.Allocator, project_dir: []const u8) !ProjectInfo {
    var path_buf: [1024]u8 = undefined;
    const json_path = try std.fmt.bufPrint(&path_buf, "{s}/project.json", .{project_dir});

    const file = try std.fs.openFileAbsolute(json_path, .{});
    defer file.close();

    const content = try file.readToEndAlloc(allocator, 1024 * 1024);
    defer allocator.free(content);

    var parsed = try std.json.parseFromSlice(std.json.Value, allocator, content, .{});
    defer parsed.deinit();

    var info = ProjectInfo{
        .path = try allocator.dupe(u8, project_dir),
        .is_valid = true,
    };
    const root = parsed.value;

    if (root.object.get("project_name")) |val| {
        if (val == .string) info.project_name = try allocator.dupe(u8, val.string);
    }
    if (root.object.get("display_name")) |val| {
        if (val == .string) info.display_name = try allocator.dupe(u8, val.string);
    }
    if (root.object.get("project_id")) |val| {
        if (val == .string) info.project_id = try allocator.dupe(u8, val.string);
    }
    if (root.object.get("summary")) |val| {
        if (val == .string) info.summary = try allocator.dupe(u8, val.string);
    }
    if (root.object.get("engine")) |val| {
        if (val == .string) info.engine = try allocator.dupe(u8, val.string);
    }
    if (root.object.get("engine_version")) |val| {
        if (val == .string) info.engine_version = try allocator.dupe(u8, val.string);
    }
    if (root.object.get("icon_path")) |val| {
        if (val == .string) info.icon_path = try allocator.dupe(u8, val.string);
    }

    if (root.object.get("gem_names")) |val| {
        if (val == .array) {
            var list = std.ArrayList([]const u8).init(allocator);
            for (val.array.items) |item| {
                if (item == .string) {
                    try list.append(try allocator.dupe(u8, item.string));
                }
            }
            info.gem_names = try list.toOwnedSlice();
        }
    }

    return info;
}

/// Сканирование Gems движка (из $O3DE_DIR/Gems)
pub fn scanEngineGems(allocator: std.mem.Allocator, o3de_dir: []const u8) ![]GemInfo {
    var gems_path_buf: [1024]u8 = undefined;
    const gems_path = try std.fmt.bufPrint(&gems_path_buf, "{s}/Gems", .{o3de_dir});

    var dir = std.fs.openDirAbsolute(gems_path, .{ .iterate = true }) catch return &[_]GemInfo{};
    defer dir.close();

    var list = std.ArrayList(GemInfo).init(allocator);
    var iter = dir.iterate();

    while (try iter.next()) |entry| {
        if (entry.kind == .directory) {
            var sub_path_buf: [1024]u8 = undefined;
            const gem_json_path = try std.fmt.bufPrint(&sub_path_buf, "{s}/{s}/gem.json", .{ gems_path, entry.name });
            if (std.fs.openFileAbsolute(gem_json_path, .{})) |file| {
                defer file.close();
                const content = file.readToEndAlloc(allocator, 512 * 1024) catch continue;
                defer allocator.free(content);

                var parsed = std.json.parseFromSlice(std.json.Value, allocator, content, .{}) catch continue;
                defer parsed.deinit();

                var gem = GemInfo{
                    .gem_name = try allocator.dupe(u8, entry.name),
                    .display_name = try allocator.dupe(u8, entry.name),
                    .version = try allocator.dupe(u8, "1.0.0"),
                    .summary = try allocator.dupe(u8, ""),
                    .origin = try allocator.dupe(u8, ""),
                    .license = try allocator.dupe(u8, "Apache-2.0"),
                    .path = try std.fmt.allocPrint(allocator, "{s}/{s}", .{ gems_path, entry.name }),
                };

                const root = parsed.value;
                if (root == .object) {
                    if (root.object.get("display_name")) |val| {
                        if (val == .string) {
                            allocator.free(gem.display_name);
                            gem.display_name = try allocator.dupe(u8, val.string);
                        }
                    }
                    if (root.object.get("summary")) |val| {
                        if (val == .string) {
                            allocator.free(gem.summary);
                            gem.summary = try allocator.dupe(u8, val.string);
                        }
                    }
                    if (root.object.get("version")) |val| {
                        if (val == .string) {
                            allocator.free(gem.version);
                            gem.version = try allocator.dupe(u8, val.string);
                        }
                    }
                }
                try list.append(gem);
            } else |_| {}
        }
    }
    return list.toOwnedSlice();
}
