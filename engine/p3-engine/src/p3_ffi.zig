// =============================================================================
// p3_ffi.zig — C-ABI МОСТ P³ ENGINE → POLER ENGINE (цикл W, v0.59.0)
// =============================================================================
//
// Восстановлен с нуля после потери песочницы (оригинал цикла R не был
// закоммичен в P3_Engine). Семантика каждого символа снята зондом со
// старой libp3ffi.so (scripts/probe_old_p3ffi.py) и зеркалирует
// Rust-близнеца poler-engine/src/p3/native.rs:
//
//   p3ffi_abi_version        → u32 = 1
//   p3ffi_kernel_tag         → "p3-kernel/zig-0.14.0/abi-1"
//   p3ffi_fs_distance(a,b)   → d_FS = acos(|⟨a,b⟩|/(‖a‖·‖b‖)) ∈ [0, π/2]
//   p3ffi_pgl_*              → PGL(4,ℝ), column-major [16]f64
//   p3ffi_givens4(i,j,θ)     → G: M[i][j]=−s, M[j][i]=+s, det=+1
//   p3ffi_spectral_projector → P = vvᵀ/(vᵀv)
//   p3ffi_idempotent_rank    → след (ранг идемпотента)
//   p3ffi_render_frame       → тройной буфер RGB + d_FS-глубина + сегменты
//
// ГЛУБИНА РЕНДЕРА (расшифровано зондом, проверено численно):
//   depth = d_FS(O, P_view) = acos(1/‖(x', y', z₂, 1)‖) ∈ [0, π/2]
//   где O — начало координат пространства камеры, P_view — точка после
//   поворотов yaw/pitch (БЕЗ сдвига cam_dist). Фон = 1e30.
//
// СБОРКА: build.zig таргетит БАЗОВЫЙ x86-64 (SSE2, cpu_model = .baseline)
// — никаких AVX2, никакой SIGILL на Ivy Bridge и старше. Это лечение
// первопричины, а не обходной путь.
//
// Детерминизм: чистая функция без аллокаций и состояния; одинаковый
// вход → одинаковый тройной буфер бит-в-бит.
// =============================================================================

const std = @import("std");
const math = std.math;
const kernel = @import("p3_kernel.zig");

const HomVec4 = kernel.HomVec4;
const PGL4 = kernel.PGL4;

pub const ABI_VERSION: u32 = 1;
const KERNEL_TAG = "p3-kernel/zig-0.14.0/abi-1";

// =============================================================================
// 1. РУКОПОЖАТИЕ
// =============================================================================

export fn p3ffi_abi_version() u32 {
    return ABI_VERSION;
}

export fn p3ffi_kernel_tag() [*:0]const u8 {
    return KERNEL_TAG;
}

// =============================================================================
// 2. МЕТРИКА ФУБИНИ–ШТУДИ
// =============================================================================

inline fn hom4(p: [*]const f64) HomVec4 {
    return .{ .x = p[0], .y = p[1], .z = p[2], .w = p[3] };
}

export fn p3ffi_fs_distance(a: [*]const f64, b: [*]const f64) f64 {
    return kernel.fsDistance(hom4(a), hom4(b));
}

// =============================================================================
// 3. PGL(4,ℝ) — COLUMN-MAJOR [16]f64
// =============================================================================

export fn p3ffi_pgl_identity(out: [*]f64) void {
    const id = PGL4.identity();
    @memcpy(out[0..16], &id.data);
}

export fn p3ffi_pgl_mul(out: [*]f64, a: [*]const f64, b: [*]const f64) void {
    const ma = PGL4{ .data = a[0..16].* };
    const mb = PGL4{ .data = b[0..16].* };
    const r = ma.mul(mb);
    @memcpy(out[0..16], &r.data);
}

export fn p3ffi_pgl_transpose(out: [*]f64, m: [*]const f64) void {
    const mm = PGL4{ .data = m[0..16].* };
    const r = mm.transpose();
    @memcpy(out[0..16], &r.data);
}

export fn p3ffi_pgl_apply(out: [*]f64, m: [*]const f64, v: [*]const f64) void {
    const mm = PGL4{ .data = m[0..16].* };
    const r = mm.apply(hom4(v));
    out[0] = r.x;
    out[1] = r.y;
    out[2] = r.z;
    out[3] = r.w;
}

export fn p3ffi_pgl_det(m: [*]const f64) f64 {
    const mm = PGL4{ .data = m[0..16].* };
    return mm.det();
}

export fn p3ffi_givens4(out: [*]f64, i: u32, j: u32, theta: f64) void {
    var g = PGL4.identity();
    if (i == j or i > 3 or j > 3) {
        @memcpy(out[0..16], &g.data);
        return;
    }
    const c = math.cos(theta);
    const s = math.sin(theta);
    const ii: usize = @intCast(i);
    const jj: usize = @intCast(j);
    g.set(ii, ii, c);
    g.set(jj, jj, c);
    g.set(ii, jj, -s); // M[i][j] = −sin
    g.set(jj, ii, s); // M[j][i] = +sin
    @memcpy(out[0..16], &g.data);
}

// =============================================================================
// 4. ИДЕМПОТЕНТЫ И ПРОЕКТОРЫ
// =============================================================================

export fn p3ffi_spectral_projector(out: [*]f64, v: [*]const f64) void {
    const hv = hom4(v);
    const vv = HomVec4.dot(hv, hv);
    var p = PGL4.identity();
    if (vv < 1e-300) {
        @memcpy(out[0..16], &p.data);
        return;
    }
    const comps = [4]f64{ hv.x, hv.y, hv.z, hv.w };
    var row: usize = 0;
    while (row < 4) : (row += 1) {
        var col: usize = 0;
        while (col < 4) : (col += 1) {
            p.set(row, col, comps[row] * comps[col] / vv);
        }
    }
    @memcpy(out[0..16], &p.data);
}

export fn p3ffi_idempotent_rank(m: [*]const f64) f64 {
    const mm = PGL4{ .data = m[0..16].* };
    return mm.get(0, 0) + mm.get(1, 1) + mm.get(2, 2) + mm.get(3, 3);
}

export fn p3ffi_idempotent_residual(m: [*]const f64) f64 {
    const mm = PGL4{ .data = m[0..16].* };
    const p2 = mm.mul(mm);
    var worst: f64 = 0;
    var k: usize = 0;
    while (k < 16) : (k += 1) {
        worst = @max(worst, @abs(p2.data[k] - mm.data[k]));
    }
    return worst;
}

export fn p3ffi_ortho_residual(m: [*]const f64) f64 {
    const mm = PGL4{ .data = m[0..16].* };
    const utu = mm.transpose().mul(mm);
    var worst: f64 = 0;
    var row: usize = 0;
    while (row < 4) : (row += 1) {
        var col: usize = 0;
        while (col < 4) : (col += 1) {
            const want: f64 = if (row == col) 1.0 else 0.0;
            worst = @max(worst, @abs(utu.get(row, col) - want));
        }
    }
    return worst;
}

// =============================================================================
// 5. РАСТЕРИЗАТОР: ТРОЙНОЙ БУФЕР RGB + d_FS-ГЛУБИНА + СЕГМЕНТАЦИЯ
// =============================================================================
//
// Конвенции (сняты зондом со старой .so, где проверяемо; остальное —
// чистая спецификация этого слоя, зеркалируемая в Rust native):
//   • фон: rgb=(10,12,22), depth=1e30, seg=0;
//   • камера: поворот yaw (вокруг Y), затем pitch (вокруг X), затем
//     сдвиг вдоль +Z на cam_dist; перспектива f (focal ≤ 0 → 1.15·h);
//   • глубина пикселя: d_FS(O, P_view), интерполируется НЕ между
//     глубинами концов, а пересчитывается из интерполированной 3D-точки
//     (честная глубина вдоль отрезка);
//   • цвет: бленд палитр сегментов вершин ребра по t + яркость от
//     глубины (0.55..1.00);
//   • seg-буфер: сегмент ПЕРВОЙ вершины ребра;
//   • толщина: ядро Брезенхема 1px + плюс-спрайты радиуса
//     max(1, thickness−1) на концах (смещение глубины +1e-4 — ядро
//     всегда выигрывает depth-тест у спрайта);
//   • depth-тест строгий (<) — при равенстве остаётся ранее нарисованное.

const FRAC_PI_2: f64 = 1.5707963267948966;

/// Палитра 15 оттенков (сегмент 0 — фон, далее (seg−1) mod 15 + 1).
const PALETTE = [15][3]f64{
    .{ 90, 140, 230 }, // 1 — синий
    .{ 230, 90, 110 }, // 2 — красный
    .{ 90, 200, 120 }, // 3 — зелёный
    .{ 235, 190, 80 }, // 4 — золотой
    .{ 120, 200, 230 }, // 5 — циан
    .{ 210, 120, 230 }, // 6 — пурпур
    .{ 240, 150, 70 }, // 7 — оранж
    .{ 80, 210, 190 }, // 8 — бирюза
    .{ 200, 220, 90 }, // 9 — лайм
    .{ 150, 160, 255 }, // 10 — лаванда
    .{ 255, 120, 160 }, // 11 — розовый
    .{ 130, 230, 160 }, // 12 — мятный
    .{ 190, 140, 100 }, // 13 — песочный
    .{ 170, 180, 200 }, // 14 — стальной
    .{ 250, 220, 140 }, // 15 — шампань
};

inline fn palette_of(s: u8) [3]f64 {
    if (s == 0) return .{ 10, 12, 22 };
    return PALETTE[@as(usize, s - 1) % 15];
}

inline fn clamp_u8(v: f64) u8 {
    if (v <= 0) return 0;
    if (v >= 255) return 255;
    return @intFromFloat(@round(v));
}

/// Глубина Фубини–Штуди от начала координат камеры.
inline fn fs_depth(px: f64, py: f64, pz: f64) f64 {
    const q = 1.0 / @sqrt(px * px + py * py + pz * pz + 1.0);
    return math.acos(@min(1.0, q));
}

const Proj = struct {
    vx: f64,
    vy: f64,
    vz: f64, // координаты в пространстве камеры (до сдвига cam_dist)
    ix: i64,
    iy: i64,
    ok: bool,
};

/// Проекция точки: yaw → pitch → перспектива. Точки за камерой (zc ≤ 1e-9)
/// и бесконечные экранные координаты отсекаются.
fn project_point(
    x: f64,
    y: f64,
    z: f64,
    cy: f64,
    sy: f64,
    cp: f64,
    sp: f64,
    f: f64,
    cam_dist: f64,
    w2: f64,
    h2: f64,
) Proj {
    const x1 = cy * x + sy * z;
    const z1 = -sy * x + cy * z;
    const y2 = cp * y - sp * z1;
    const z2 = sp * y + cp * z1;
    const zc = z2 + cam_dist;
    if (zc <= 1e-9) {
        return .{ .vx = x1, .vy = y2, .vz = z2, .ix = 0, .iy = 0, .ok = false };
    }
    const u = f * x1 / zc + w2;
    const v = h2 - f * y2 / zc;
    if (!math.isFinite(u) or !math.isFinite(v) or @abs(u) > 9.0e15 or @abs(v) > 9.0e15) {
        return .{ .vx = x1, .vy = y2, .vz = z2, .ix = 0, .iy = 0, .ok = false };
    }
    return .{
        .vx = x1,
        .vy = y2,
        .vz = z2,
        .ix = @intFromFloat(@round(u)),
        .iy = @intFromFloat(@round(v)),
        .ok = true,
    };
}

const PlotCtx = struct {
    w: i64,
    h: i64,
    rgb: [*]u8,
    depth: [*]f32,
    seg: [*]u8,
};

inline fn plot(
    ctx: PlotCtx,
    x: i64,
    y: i64,
    d: f64,
    bias: f64,
    r: f64,
    g: f64,
    b: f64,
    s: u8,
) void {
    if (x < 0 or y < 0 or x >= ctx.w or y >= ctx.h) return;
    const idx: usize = @intCast(y * ctx.w + x);
    const dd: f32 = @floatCast(d + bias);
    if (dd < ctx.depth[idx]) {
        ctx.depth[idx] = dd;
        ctx.seg[idx] = s;
        ctx.rgb[idx * 3] = clamp_u8(r);
        ctx.rgb[idx * 3 + 1] = clamp_u8(g);
        ctx.rgb[idx * 3 + 2] = clamp_u8(b);
    }
}

/// Плюс-спрайт радиуса R вокруг конечной точки (ядро линии всегда
/// выигрывает: спрайт несёт штраф глубины 1e-4·(|ox|+|oy|+1)).
fn draw_endpoint_sprite(
    ctx: PlotCtx,
    p: Proj,
    col: [3]f64,
    s: u8,
    radius: i64,
) void {
    const d = fs_depth(p.vx, p.vy, p.vz);
    const bright = 0.55 + 0.45 * (1.0 - d / FRAC_PI_2);
    const r = col[0] * bright;
    const g = col[1] * bright;
    const b = col[2] * bright;
    var oy: i64 = -radius;
    while (oy <= radius) : (oy += 1) {
        var ox: i64 = -radius;
        while (ox <= radius) : (ox += 1) {
            const ax: i64 = if (ox > 0) ox else -ox;
            const ay: i64 = if (oy > 0) oy else -oy;
            const manh: i64 = ax + ay;
            if (manh > radius) continue; // форма «плюс/ромб»
            const bias = 1e-4 * @as(f64, @floatFromInt(manh + 1));
            plot(ctx, p.ix + ox, p.iy + oy, d, bias, r, g, b, s);
        }
    }
}

export fn p3ffi_render_frame(
    pts: [*]const f64,
    n_pts: u32,
    seg_ids: [*]const u8,
    pairs: [*]const u32,
    n_pairs: u32,
    width: u32,
    height: u32,
    focal: f64,
    cam_dist: f64,
    yaw: f64,
    pitch: f64,
    thickness: u32,
    rgb: [*]u8,
    depth: [*]f32,
    seg: [*]u8,
) void {
    const w: i64 = @intCast(width);
    const h: i64 = @intCast(height);
    const ctx = PlotCtx{ .w = w, .h = h, .rgb = rgb, .depth = depth, .seg = seg };

    // --- 1. Фон ---
    var i: usize = 0;
    while (i < @as(usize, @intCast(w * h))) : (i += 1) {
        rgb[i * 3] = 10;
        rgb[i * 3 + 1] = 12;
        rgb[i * 3 + 2] = 22;
        depth[i] = 1e30;
        seg[i] = 0;
    }
    if (n_pts == 0 or n_pairs == 0) return;

    const hf: f64 = @floatFromInt(height);
    const f = if (focal > 0) focal else 1.15 * hf;
    const w2: f64 = @as(f64, @floatFromInt(width)) / 2.0;
    const h2: f64 = hf / 2.0;
    const cy = math.cos(yaw);
    const sy = math.sin(yaw);
    const cp = math.cos(pitch);
    const sp = math.sin(pitch);
    const radius: i64 = @max(1, @as(i64, @intCast(thickness)) - 1);

    // --- 2. Рёбра ---
    var e: usize = 0;
    while (e < n_pairs) : (e += 1) {
        const ia: usize = pairs[e * 2];
        const jb: usize = pairs[e * 2 + 1];
        if (ia >= n_pts or jb >= n_pts) continue;
        const pa = pts + ia * 4;
        const pb = pts + jb * 4;
        const s0 = seg_ids[ia];
        const s1 = seg_ids[jb];

        const A = project_point(pa[0], pa[1], pa[2], cy, sy, cp, sp, f, cam_dist, w2, h2);
        const B = project_point(pb[0], pb[1], pb[2], cy, sy, cp, sp, f, cam_dist, w2, h2);
        if (!A.ok or !B.ok) continue;

        const col0 = palette_of(s0);
        const col1 = palette_of(s1);
        const cseg: u8 = s0; // сегмент ребра — первая вершина (конвенция ABI)

        // --- Брезенхем (целочисленный, классический) ---
        var x = A.ix;
        var y = A.iy;
        const x1 = B.ix;
        const y1 = B.iy;
        const dx = x1 - x;
        const dy = y1 - y;
        const adx: i64 = if (dx > 0) dx else -dx;
        const ady: i64 = if (dy > 0) dy else -dy;
        const n: i64 = @max(adx, ady);
        const sx: i64 = if (dx > 0) 1 else -1;
        const sy2: i64 = if (dy > 0) 1 else -1;
        var err: i64 = adx - ady;

        var k: i64 = 0;
        while (k <= n) : (k += 1) {
            const t: f64 = if (n > 0)
                @as(f64, @floatFromInt(k)) / @as(f64, @floatFromInt(n))
            else
                1.0; // вырожденное ребро → вторая вершина (семантика зонда)
            const px = A.vx + (B.vx - A.vx) * t;
            const py = A.vy + (B.vy - A.vy) * t;
            const pz = A.vz + (B.vz - A.vz) * t;
            const d = fs_depth(px, py, pz);
            const bright = 0.55 + 0.45 * (1.0 - d / FRAC_PI_2);
            const r = (col0[0] + (col1[0] - col0[0]) * t) * bright;
            const g = (col0[1] + (col1[1] - col0[1]) * t) * bright;
            const b = (col0[2] + (col1[2] - col0[2]) * t) * bright;
            plot(ctx, x, y, d, 0.0, r, g, b, cseg);

            // шаг Брезенхема
            if (2 * err > -ady) {
                err -= ady;
                x += sx;
            }
            if (2 * err < adx) {
                err += adx;
                y += sy2;
            }
        }

        // --- Спрайты концов (толщина) ---
        draw_endpoint_sprite(ctx, A, col0, cseg, radius);
        draw_endpoint_sprite(ctx, B, col1, cseg, radius);
    }
}

// =============================================================================
// ТЕСТЫ FFI-СЛОЯ (zig build test-ffi)
// =============================================================================

const expect = std.testing.expect;
const expectApproxEqAbs = std.testing.expectApproxEqAbs;

test "abi и тег" {
    try expect(p3ffi_abi_version() == 1);
    const tag = std.mem.span(p3ffi_kernel_tag());
    try expect(std.mem.indexOf(u8, tag, "p3-kernel") != null);
}

test "fs_distance якоря" {
    var a = [4]f64{ 1, 0, 0, 0 };
    var b = [4]f64{ 0, 0, 0, 1 };
    try expectApproxEqAbs(FRAC_PI_2, p3ffi_fs_distance(&a, &b), 1e-14);
    b = .{ -2, 0, 0, 0 };
    try expectApproxEqAbs(0.0, p3ffi_fs_distance(&a, &b), 1e-14);
    b = .{ 0, 0, 0, 0 };
    try expect(p3ffi_fs_distance(&a, &b) == 0.0); // guard нулевого вектора
}

test "givens4 ортогональность" {
    var out: [16]f64 = undefined;
    p3ffi_givens4(&out, 0, 1, 0.3);
    const m = PGL4{ .data = out };
    try expectApproxEqAbs(@as(f64, 1.0), m.det(), 1e-12);
    try expect(p3ffi_ortho_residual(&out) < 1e-14);
    // вырожденный случай
    p3ffi_givens4(&out, 2, 2, 1.0);
    var i: usize = 0;
    while (i < 16) : (i += 1) {
        const want: f64 = if (i % 5 == 0) 1.0 else 0.0;
        try expect(out[i] == want);
    }
}

test "спектральный проектор ранга 1" {
    var v = [4]f64{ 1, 2, 3, 4 };
    var p: [16]f64 = undefined;
    p3ffi_spectral_projector(&p, &v);
    try expectApproxEqAbs(@as(f64, 1.0), p3ffi_idempotent_rank(&p), 1e-14);
    try expect(p3ffi_idempotent_residual(&p) < 1e-14);
}

test "render_frame дым: квадрат" {
    const pts = [16]f64{
        -0.5, -0.5, 0, 1,
        0.5,  -0.5, 0, 1,
        0.5,  0.5,  0, 1,
        -0.5, 0.5,  0, 1,
    };
    const segs = [4]u8{ 1, 2, 3, 4 };
    const pairs = [8]u32{ 0, 1, 1, 2, 2, 3, 3, 0 };
    const w = 96;
    const h = 72;
    var rgb: [w * h * 3]u8 = undefined;
    var depth: [w * h]f32 = undefined;
    var seg: [w * h]u8 = undefined;
    p3ffi_render_frame(
        &pts,
        4,
        &segs,
        &pairs,
        4,
        w,
        h,
        1.15 * @as(f64, h),
        3.4,
        -0.62,
        0.34,
        2,
        &rgb,
        &depth,
        &seg,
    );
    var painted: usize = 0;
    var maxd: f32 = 0;
    for (0..w * h) |k| {
        if (seg[k] != 0) {
            painted += 1;
            maxd = @max(maxd, depth[k]);
        }
    }
    try expect(painted > 20);
    try expect(maxd <= @as(f32, @floatCast(FRAC_PI_2)) + 1e-4);
    // фон
    try expect(depth[0] == 1e30);
    try expect(rgb[0] == 10 and rgb[1] == 12 and rgb[2] == 22);
}
