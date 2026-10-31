// =============================================================================
// P³ UI ANIMATION v1.0 — ZIG
// =============================================================================
//
// Animation system (из O3DE UiAnimationSystem).
//
// В O3DE:
//   UiAnimationSystem  — менеджер анимаций
//   AnimSequence       — последовательность анимации (keyframes)
//   AnimNode           — узел в анимационном графе
//   AnimTrack          — трек анимации (float/Vector/Color/Bool)
//   CompoundSplineTrack — составной трек (Vec2/Vec3/Color из float треков)
//   2DSpline           — сплайн для интерполяции
//   BoolTrack          — трек для boolean значений
//   EventNode          — узел для событий
//   TrackEventTrack    — трек для событий
//
// В P³ Engine:
//   - Без CryMovie/Qt — чистый Zig
//   - Keyframe interpolation: step, linear, spline (Catmull-Rom)
//   - AnimTrack — tagged union вместо template
//   - P³ обобщение: проективная анимация (сохраняет cross-ratio)
//
// Портировано из O3DE/Gems/LyShine/Code/Source/Animation/*
// Адаптировано для Zig 0.14.0 + P³ Engine API.
//
// Принцип: убивать и жрать и рождать новое.
// =============================================================================

const std = @import("std");
const math = std.math;
const ui_transform = @import("p3_ui_transform.zig");
const ui_draw = @import("p3_ui_draw.zig");

pub const Vec2 = ui_transform.Vec2;
pub const Color = ui_draw.Color;

// =============================================================================
// 1. INTERPOLATION TYPE
// =============================================================================

/// Тип интерполяции между keyframes.
pub const InterpolationType = enum(u2) {
    /// Константное (snap)
    step = 0,
    /// Линейная интерполяция
    linear = 1,
    /// Сплайн (Catmull-Rom)
    spline = 2,
};

// =============================================================================
// 2. ANIMATION STATE
// =============================================================================

/// Состояние анимации.
pub const AnimState = enum(u2) {
    stopped = 0,
    playing = 1,
    paused = 2,
};

// =============================================================================
// 3. KEYFRAME (f32)
// =============================================================================

/// Ключевой кадр для float трека.
pub const FloatKey = struct {
    time: f32,
    value: f32,
    interp: InterpolationType,
    /// In/out tangent for spline
    in_tangent: f32,
    out_tangent: f32,
};

// =============================================================================
// 4. FLOAT TRACK
// =============================================================================

/// Трек анимации для float значений.
pub const FloatTrack = struct {
    keys: std.ArrayList(FloatKey),
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) FloatTrack {
        return .{
            .keys = std.ArrayList(FloatKey).init(allocator),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *FloatTrack) void {
        self.keys.deinit();
    }

    /// Добавить keyframe.
    pub fn addKey(self: *FloatTrack, time: f32, value: f32, interp: InterpolationType) !void {
        try self.keys.append(.{
            .time = time,
            .value = value,
            .interp = interp,
            .in_tangent = 0,
            .out_tangent = 0,
        });
    }

    /// Интерполировать значение в момент времени t.
    pub fn evaluate(self: FloatTrack, t: f32) f32 {
        if (self.keys.items.len == 0) return 0;
        if (self.keys.items.len == 1) return self.keys.items[0].value;

        // Find surrounding keys
        if (t <= self.keys.items[0].time) return self.keys.items[0].value;
        if (t >= self.keys.items[self.keys.items.len - 1].time) return self.keys.items[self.keys.items.len - 1].value;

        // Binary search for the right interval
        var lo: usize = 0;
        var hi: usize = self.keys.items.len - 1;
        while (hi - lo > 1) {
            const mid = lo + (hi - lo) / 2;
            if (self.keys.items[mid].time <= t) lo = mid else hi = mid;
        }

        const k0 = self.keys.items[lo];
        const k1 = self.keys.items[hi];
        const dt = k1.time - k0.time;
        if (dt <= 0) return k0.value;

        const s = (t - k0.time) / dt;

        return switch (k0.interp) {
            .step => k0.value,
            .linear => k0.value + s * (k1.value - k0.value),
            .spline => catmullRom(
                if (lo > 0) self.keys.items[lo - 1].value else k0.value,
                k0.value,
                k1.value,
                if (hi < self.keys.items.len - 1) self.keys.items[hi + 1].value else k1.value,
                s,
            ),
        };
    }

    /// Catmull-Rom spline interpolation.
    fn catmullRom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) f32 {
        const t2 = t * t;
        const t3 = t2 * t;
        return 0.5 * (
            (2 * p1) +
            (-p0 + p2) * t +
            (2 * p0 - 5 * p1 + 4 * p2 - p3) * t2 +
            (-p0 + 3 * p1 - 3 * p2 + p3) * t3
        );
    }
};

// =============================================================================
// 5. BOOL KEY
// =============================================================================

/// Ключевой кадр для boolean трека.
pub const BoolKey = struct {
    time: f32,
    value: bool,
};

// =============================================================================
// 6. BOOL TRACK
// =============================================================================

/// Трек для boolean значений (step interpolation only).
pub const BoolTrack = struct {
    keys: std.ArrayList(BoolKey),
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) BoolTrack {
        return .{
            .keys = std.ArrayList(BoolKey).init(allocator),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *BoolTrack) void {
        self.keys.deinit();
    }

    pub fn addKey(self: *BoolTrack, time: f32, value: bool) !void {
        try self.keys.append(.{ .time = time, .value = value });
    }

    pub fn evaluate(self: BoolTrack, t: f32) bool {
        if (self.keys.items.len == 0) return false;
        var result = self.keys.items[0].value;
        for (self.keys.items) |k| {
            if (k.time <= t) result = k.value else break;
        }
        return result;
    }
};

// =============================================================================
// 7. COMPOUND TRACK (Vec2, Vec3, Color)
// =============================================================================

/// Составной трек — объединяет несколько float треков.
///
/// Vec2 = 2 трека, Vec3/Color = 3 трека (+ alpha = 4 для Color).
pub const CompoundTrack = struct {
    tracks: [4]FloatTrack,
    component_count: u32,

    pub fn init(allocator: std.mem.Allocator, components: u32) CompoundTrack {
        var ct = CompoundTrack{
            .tracks = undefined,
            .component_count = @min(components, 4),
        };
        for (0..ct.component_count) |i| {
            ct.tracks[i] = FloatTrack.init(allocator);
        }
        return ct;
    }

    pub fn deinit(self: *CompoundTrack) void {
        for (0..self.component_count) |i| {
            self.tracks[i].deinit();
        }
    }

    /// Evaluate as Vec2.
    pub fn evaluateVec2(self: CompoundTrack, t: f32) Vec2 {
        return Vec2.init(
            self.tracks[0].evaluate(t),
            self.tracks[1].evaluate(t),
        );
    }

    /// Evaluate as Color.
    pub fn evaluateColor(self: CompoundTrack, t: f32) Color {
        return Color.init(
            self.tracks[0].evaluate(t),
            self.tracks[1].evaluate(t),
            if (self.component_count > 2) self.tracks[2].evaluate(t) else 1.0,
            if (self.component_count > 3) self.tracks[3].evaluate(t) else 1.0,
        );
    }
};

// =============================================================================
// 8. ANIM NODE
// =============================================================================

/// Узел в анимационном графе (entity reference).
pub const AnimNode = struct {
    name: []const u8,
    entity_id: u64,

    pub fn init(name: []const u8) AnimNode {
        return .{ .name = name, .entity_id = 0 };
    }
};

// =============================================================================
// 9. ANIM SEQUENCE
// =============================================================================

/// AnimSequence — последовательность анимации.
///
/// Содержит набор треков, привязанных к узлам (entities).
pub const AnimSequence = struct {
    name: []const u8,
    duration: f32,
    state: AnimState,
    current_time: f32,
    loop: bool,
    playback_speed: f32,

    // --- Tracks ---
    float_tracks: std.ArrayList(FloatTrack),
    bool_tracks: std.ArrayList(BoolTrack),
    compound_tracks: std.ArrayList(CompoundTrack),

    // --- Nodes ---
    nodes: std.ArrayList(AnimNode),

    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator, name: []const u8) AnimSequence {
        return .{
            .name = name,
            .duration = 1.0,
            .state = .stopped,
            .current_time = 0,
            .loop = false,
            .playback_speed = 1.0,
            .float_tracks = std.ArrayList(FloatTrack).init(allocator),
            .bool_tracks = std.ArrayList(BoolTrack).init(allocator),
            .compound_tracks = std.ArrayList(CompoundTrack).init(allocator),
            .nodes = std.ArrayList(AnimNode).init(allocator),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *AnimSequence) void {
        for (self.float_tracks.items) |*t| t.deinit();
        self.float_tracks.deinit();
        for (self.bool_tracks.items) |*t| t.deinit();
        self.bool_tracks.deinit();
        for (self.compound_tracks.items) |*t| t.deinit();
        self.compound_tracks.deinit();
        self.nodes.deinit();
    }

    /// Начать воспроизведение.
    pub fn play(self: *AnimSequence) void {
        self.state = .playing;
    }

    /// Пауза.
    pub fn pause(self: *AnimSequence) void {
        self.state = .paused;
    }

    /// Остановить.
    pub fn stop(self: *AnimSequence) void {
        self.state = .stopped;
        self.current_time = 0;
    }

    /// Установить время.
    pub fn setTime(self: *AnimSequence, t: f32) void {
        self.current_time = std.math.clamp(t, 0.0, self.duration);
    }

    /// Обновить анимацию.
    pub fn update(self: *AnimSequence, dt: f32) void {
        if (self.state != .playing) return;

        self.current_time += dt * self.playback_speed;

        if (self.current_time >= self.duration) {
            if (self.loop) {
                self.current_time = @mod(self.current_time, self.duration);
            } else {
                self.current_time = self.duration;
                self.state = .stopped;
            }
        }
    }

    /// Добавить float track.
    pub fn addFloatTrack(self: *AnimSequence) !usize {
        try self.float_tracks.append(FloatTrack.init(self.allocator));
        return self.float_tracks.items.len - 1;
    }

    /// Добавить bool track.
    pub fn addBoolTrack(self: *AnimSequence) !usize {
        try self.bool_tracks.append(BoolTrack.init(self.allocator));
        return self.bool_tracks.items.len - 1;
    }

    /// Добавить compound track.
    pub fn addCompoundTrack(self: *AnimSequence, components: u32) !usize {
        try self.compound_tracks.append(CompoundTrack.init(self.allocator, components));
        return self.compound_tracks.items.len - 1;
    }

    /// Добавить node.
    pub fn addNode(self: *AnimSequence, name: []const u8) !usize {
        try self.nodes.append(AnimNode.init(name));
        return self.nodes.items.len - 1;
    }
};

// =============================================================================
// 10. ANIMATION SYSTEM
// =============================================================================

/// UiAnimationSystem — менеджер всех анимаций.
///
/// В O3DE: UiAnimationSystem — singleton per canvas.
pub const AnimationSystem = struct {
    sequences: std.ArrayList(AnimSequence),
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) AnimationSystem {
        return .{
            .sequences = std.ArrayList(AnimSequence).init(allocator),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *AnimationSystem) void {
        for (self.sequences.items) |*seq| seq.deinit();
        self.sequences.deinit();
    }

    /// Создать новую анимацию.
    pub fn createSequence(self: *AnimationSystem, name: []const u8) !usize {
        try self.sequences.append(AnimSequence.init(self.allocator, name));
        return self.sequences.items.len - 1;
    }

    /// Обновить все активные анимации.
    pub fn update(self: *AnimationSystem, dt: f32) void {
        for (self.sequences.items) |*seq| {
            seq.update(dt);
        }
    }

    /// Количество активных анимаций.
    pub fn activeCount(self: AnimationSystem) u32 {
        var count: u32 = 0;
        for (self.sequences.items) |seq| {
            if (seq.state == .playing) count += 1;
        }
        return count;
    }
};

// =============================================================================
// 11. ТЕСТЫ
// =============================================================================

test "FloatTrack: linear interpolation" {
    const allocator = std.testing.allocator;
    var track = FloatTrack.init(allocator);
    defer track.deinit();

    try track.addKey(0.0, 0.0, .linear);
    try track.addKey(1.0, 10.0, .linear);

    try std.testing.expectApproxEqAbs(track.evaluate(0.0), 0.0, 1e-6);
    try std.testing.expectApproxEqAbs(track.evaluate(0.5), 5.0, 1e-6);
    try std.testing.expectApproxEqAbs(track.evaluate(1.0), 10.0, 1e-6);
}

test "FloatTrack: step interpolation" {
    const allocator = std.testing.allocator;
    var track = FloatTrack.init(allocator);
    defer track.deinit();

    try track.addKey(0.0, 0.0, .step);
    try track.addKey(0.5, 10.0, .step);

    try std.testing.expectApproxEqAbs(track.evaluate(0.0), 0.0, 1e-6);
    try std.testing.expectApproxEqAbs(track.evaluate(0.4), 0.0, 1e-6);
    try std.testing.expectApproxEqAbs(track.evaluate(0.5), 10.0, 1e-6);
}

test "FloatTrack: clamping at boundaries" {
    const allocator = std.testing.allocator;
    var track = FloatTrack.init(allocator);
    defer track.deinit();

    try track.addKey(0.0, 0.0, .linear);
    try track.addKey(1.0, 10.0, .linear);

    try std.testing.expectApproxEqAbs(track.evaluate(-1.0), 0.0, 1e-6);
    try std.testing.expectApproxEqAbs(track.evaluate(2.0), 10.0, 1e-6);
}

test "BoolTrack: evaluate" {
    const allocator = std.testing.allocator;
    var track = BoolTrack.init(allocator);
    defer track.deinit();

    try track.addKey(0.0, false);
    try track.addKey(0.5, true);
    try track.addKey(0.8, false);

    try std.testing.expect(!track.evaluate(0.0));
    try std.testing.expect(track.evaluate(0.5));
    try std.testing.expect(!track.evaluate(0.8));
}

test "AnimSequence: play and update" {
    const allocator = std.testing.allocator;
    var seq = AnimSequence.init(allocator, "test");
    defer seq.deinit();

    seq.duration = 2.0;
    seq.play();
    try std.testing.expect(seq.state == .playing);

    seq.update(1.0);
    try std.testing.expectApproxEqAbs(seq.current_time, 1.0, 1e-6);

    seq.update(1.0);
    try std.testing.expect(seq.state == .stopped);
}

test "AnimSequence: loop" {
    const allocator = std.testing.allocator;
    var seq = AnimSequence.init(allocator, "test");
    defer seq.deinit();

    seq.duration = 1.0;
    seq.loop = true;
    seq.play();
    seq.update(1.5);
    try std.testing.expectApproxEqAbs(seq.current_time, 0.5, 1e-6);
}

test "AnimationSystem: update all" {
    const allocator = std.testing.allocator;
    var sys = AnimationSystem.init(allocator);
    defer sys.deinit();

    const idx = try sys.createSequence("anim1");
    sys.sequences.items[idx].duration = 1.0;
    sys.sequences.items[idx].play();

    sys.update(0.5);
    try std.testing.expect(sys.activeCount() == 1);
    try std.testing.expectApproxEqAbs(sys.sequences.items[idx].current_time, 0.5, 1e-6);
}
