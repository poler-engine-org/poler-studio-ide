// MVR-v3, фаза побитовой сверки: дамп golden vectors из РЕАЛЬНОГО ядра.
// M4: ядро живёт в монорепо (os/core/poler_core.zig, PND v8.2 с P0-фиксами
// аудита Шнайера) и экспортирует нужные функции как pub — probe-копия
// больше не нужна, дампер импортирует ядро напрямую.
// Выход: текстовые строки "tag hex..." для последующей сверки с Python.
const std = @import("std");
const core = @import("poler_core");

var out_buf: std.ArrayList(u8) = undefined;
var out_arena: std.heap.ArenaAllocator = undefined;

fn emit(comptime fmt: []const u8, args: anytype) void {
    out_buf.writer().print(fmt, args) catch unreachable;
}

var lcg_state: u32 = 0x243F6A88;
fn lcg() u32 {
    lcg_state = lcg_state *% 0x9E3779B9 +% 0x1234567;
    return lcg_state;
}

pub fn main() !void {
    out_arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer out_arena.deinit();
    out_buf = std.ArrayList(u8).init(out_arena.allocator());

    // ── 1. phi: краевые + LCG ─────────────────────────────────────────────
    const edge: [12]u32 = .{ 0, 1, 2, 0x7FFFFFFF, 0x80000000, 0xFFFFFFFF,
        0x9E3779B9, 0x517CC1B7, 0xDEADBEEF, 0xCAFEBABE, 0x12345678, 0x55555555 };
    for (edge) |x| emit("phi {x:0>8} {x:0>8}\n", .{ x, core.phi(x) });
    for (0..10000) |_| {
        const x = lcg();
        emit("phi {x:0>8} {x:0>8}\n", .{ x, core.phi(x) });
    }

    // ── 2. pndMix: тестовые тройки Zig + краевые + LCG ────────────────────
    emit("pndmix {x} {x} {x} {x:0>8}\n", .{ @as(u32, 42), @as(u32, 17), @as(u32, 1), core.pndMix(42, 17, 1) });
    emit("pndmix {x} {x} {x} {x:0>8}\n", .{ @as(u32, 42), @as(u32, 17), @as(u32, 2), core.pndMix(42, 17, 2) });
    emit("pndmix {x} {x} {x} {x:0>8}\n", .{ @as(u32, 42), @as(u32, 17), @as(u32, 0), core.pndMix(42, 17, 0) }); // ε=0 → автокоррекция
    for (edge) |x| {
        emit("pndmix {x:0>8} {x:0>8} {x:0>8} {x:0>8}\n", .{ x, x, @as(u32, 1), core.pndMix(x, x, 1) });
        emit("pndmix {x:0>8} {x:0>8} {x:0>8} {x:0>8}\n", .{ x, ~x, @as(u32, 0xFFFFFFFF), core.pndMix(x, ~x, 0xFFFFFFFF) });
    }
    for (0..10000) |_| {
        const a = lcg(); const b = lcg(); const e = lcg();
        emit("pndmix {x:0>8} {x:0>8} {x:0>8} {x:0>8}\n", .{ a, b, e, core.pndMix(a, b, e) });
    }

    // ── 3. S-box / InvS-box: все 256 значений ─────────────────────────────
    for (0..256) |i| {
        const x: u8 = @intCast(i);
        emit("sbox {x:0>2} {x:0>2}\n", .{ x, core.constantTimeSbox(x) });
        emit("invsbox {x:0>2} {x:0>2}\n", .{ x, core.constantTimeInvSbox(x) });
    }

    // ── 4. mixColumnsPnd / inv: базисы, краевые, LCG ──────────────────────
    const basis: [5]u32 = .{ 0x00000000, 0x00000001, 0x00000100, 0x00010000, 0x01000000 };
    for (basis) |w| {
        emit("mds {x:0>8} {x:0>8}\n", .{ w, core.mixColumnsPnd(w) });
        emit("invmds {x:0>8} {x:0>8}\n", .{ w, core.invMixColumnsPnd(w) });
    }
    emit("mds {x:0>8} {x:0>8}\n", .{ @as(u32, 0xFFFFFFFF), core.mixColumnsPnd(0xFFFFFFFF) });
    for (0..4096) |_| {
        const w = lcg();
        emit("mds {x:0>8} {x:0>8}\n", .{ w, core.mixColumnsPnd(w) });
    }

    // ── 5. lhcaStep: маски Фейстеля/PRNG/краевые на LCG-состояниях ────────
    const masks: [5]u32 = .{ 0xACACACAC, 0xAAAAAAAA, 0xFFFFFFFF, 0x00000000, 0x55555555 };
    for (masks) |m| {
        for (edge) |s| emit("lhca {x:0>8} {x:0>8} {x:0>8}\n", .{ s, m, core.lhcaStep(s, .{ .rule_mask = m }) });
        for (0..2048) |_| {
            const s = lcg();
            emit("lhca {x:0>8} {x:0>8} {x:0>8}\n", .{ s, m, core.lhcaStep(s, .{ .rule_mask = m }) });
        }
    }

    // ── 6. F-функции раунда ────────────────────────────────────────────────
    for (0..4096) |_| {
        const r = lcg(); const rk = lcg(); const e = lcg();
        emit("fround {x:0>8} {x:0>8} {x:0>8} {x:0>8}\n", .{ r, rk, e, core.polerFeistelF(r, rk, e) });
    }
    for (0..4096) |_| {
        const r0 = lcg(); const r1 = lcg(); const k0 = lcg(); const k1 = lcg(); const e = lcg();
        const res = core.polerFeistelFHalf(.{ r0, r1 }, .{ k0, k1 }, e);
        emit("fhalf {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8}\n",
            .{ r0, r1, k0, k1, e, res[0], res[1] });
    }

    // ── 7. Полный шифр: векторы из СОБСТВЕННЫХ тестов Zig + LCG ───────────
    const zig_keys: [4][8]u32 = .{
        .{ 0x01234567, 0x89ABCDEF, 0xFEDCBA98, 0x76543210, 0x11111111, 0x22222222, 0x33333333, 0x44444444 },
        .{ 0, 0, 0, 0, 0, 0, 0, 0 },
        .{ 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF },
        .{ 0x9E3779B9, 0x144CBC89, 0xDEADBEEF, 0xCAFEBABE, 0x12345678, 0x87654321, 0xAAAAAAAA, 0x55555555 },
    };
    const zig_eps: [4]u32 = .{ 1, 0xDEAD, 0xFFFFFFFF, 0 };
    const zig_pt: [4]u32 = .{ 0x01234567, 0x89ABCDEF, 0xFEDCBA98, 0x76543210 };
    for (zig_keys) |k| {
        for (zig_eps) |e| {
            const c = core.PolerCipher.init(&k, e);
            var ct: [4]u32 = undefined;
            var pt2: [4]u32 = undefined;
            var zig_pt_mut = zig_pt;
            c.encryptBlock(&zig_pt_mut, &ct);
            c.decryptBlock(&ct, &pt2);
            const rt_ok = pt2[0] == zig_pt[0] and pt2[1] == zig_pt[1] and pt2[2] == zig_pt[2] and pt2[3] == zig_pt[3];
            emit("cipher {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x} {x} {x} {x} {x}\n",
                .{ k[0], k[1], k[2], k[3], k[4], k[5], k[6], k[7], e,
                   zig_pt[0], zig_pt[1], zig_pt[2], zig_pt[3],
                   ct[0], ct[1], ct[2], ct[3], @intFromBool(rt_ok) });
        }
    }
    // LCG-ключи/plaintext — ещё 256 полных шифрований
    for (0..256) |_| {
        var k: [8]u32 = undefined;
        for (&k) |*w| w.* = lcg();
        const e = lcg();
        var pt: [4]u32 = undefined;
        for (&pt) |*w| w.* = lcg();
        const c = core.PolerCipher.init(&k, e);
        var ct: [4]u32 = undefined;
        var pt2: [4]u32 = undefined;
        c.encryptBlock(&pt, &ct);
        c.decryptBlock(&ct, &pt2);
        const rt_ok = pt2[0] == pt[0] and pt2[1] == pt[1] and pt2[2] == pt[2] and pt2[3] == pt[3];
        emit("cipher {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x:0>8} {x} {x} {x} {x} {x}\n",
            .{ k[0], k[1], k[2], k[3], k[4], k[5], k[6], k[7], e,
               pt[0], pt[1], pt[2], pt[3],
               ct[0], ct[1], ct[2], ct[3], @intFromBool(rt_ok) });
    }

    // ── 8. DRBG (P0-F3: PolerPrng удалён) и утилиты ──────────────────────
    const drbg_seeds: [3][8]u32 = .{
        .{ 0, 0, 0, 0, 0, 0, 0, 0 },
        .{ 1, 2, 3, 4, 5, 6, 7, 8 },
        .{ 0xDEADBEEF, 0xCAFEBABE, 0x12345678, 0x9E3779B9, 0x517CC1B7, 0xFFFFFFFF, 0x00000001, 0x80000000 },
    };
    for (drbg_seeds) |s| {
        var drbg = core.PolerDrbg.init(&s);
        // hex-подпись сида (8 слов) — ключ потока в верификаторе
        for (0..1000) |_| emit("drbg {x:0>8}{x:0>8}{x:0>8}{x:0>8}{x:0>8}{x:0>8}{x:0>8}{x:0>8} {x:0>8}\n",
            .{ s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7], drbg.next() });
    }
    for (0..4096) |_| {
        const a = lcg() | 1;
        emit("modinv {x:0>8} {x:0>8}\n", .{ a, core.modInverse32(a) });
    }
    for (edge) |k| emit("attractor {x:0>8} {x:0>8}\n", .{ k, core.attractor(k) });
    for (0..4096) |_| {
        const x: u8 = @truncate(lcg());
        const y: u8 = @truncate(lcg());
        emit("gfmul {x:0>2} {x:0>2} {x:0>2}\n", .{ x, y, core.ctGf256Mul(x, y) });
    }

    try std.io.getStdOut().writeAll(out_buf.items);
}
