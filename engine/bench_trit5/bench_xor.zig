const std = @import("std");

inline fn rdtsc_start() u64 {
    var cycles_low: u32 = 0;
    var cycles_high: u32 = 0;
    asm volatile (
        \\cpuid
        \\rdtsc
        : [low] "={eax}" (cycles_low),
          [high] "={edx}" (cycles_high),
        :
        : "rax", "rbx", "rcx", "rdx"
    );
    return (@as(u64, cycles_high) << 32) | @as(u64, cycles_low);
}

inline fn rdtsc_end() u64 {
    var cycles_low: u32 = 0;
    var cycles_high: u32 = 0;
    asm volatile (
        \\rdtscp
        \\mov %%eax, %[low]
        \\mov %%edx, %[high]
        \\cpuid
        : [low] "=r" (cycles_low),
          [high] "=r" (cycles_high),
        :
        : "rax", "rbx", "rcx", "rdx"
    );
    return (@as(u64, cycles_high) << 32) | @as(u64, cycles_low);
}

// Direct AVX1 8x Unrolled Dual-Pipe with 8 vector accumulators
pub fn dot_trit5_avx1_unrolled8(packed_weights: []const u8, x: []const f32, n: usize) f32 {
    const Vector8f = @Vector(8, f32);
    var a0: Vector8f = @splat(0.0);
    var a1: Vector8f = @splat(0.0);
    var a2: Vector8f = @splat(0.0);
    var a3: Vector8f = @splat(0.0);
    var a4: Vector8f = @splat(0.0);
    var a5: Vector8f = @splat(0.0);
    var a6: Vector8f = @splat(0.0);
    var a7: Vector8f = @splat(0.0);

    const one: Vector8f = @splat(1.0);
    const neg_one: Vector8f = @splat(-1.0);
    const zero: Vector8f = @splat(0.0);

    var byte_idx: usize = 0;
    var x_idx: usize = 0;
    const total_bytes = packed_weights.len;

    const TRIT5_LUT_F32 = @import("main.zig").TRIT5_LUT_F32;

    while (byte_idx + 8 <= total_bytes and x_idx + 43 <= n) : ({
        byte_idx += 8;
        x_idx += 40;
    }) {
        const t0: Vector8f = TRIT5_LUT_F32[packed_weights[byte_idx]];
        const xv0: Vector8f = x[x_idx..][0..8].*;
        a0 += @select(f32, t0 == one, xv0, zero) - @select(f32, t0 == neg_one, xv0, zero);

        const t1: Vector8f = TRIT5_LUT_F32[packed_weights[byte_idx + 1]];
        const xv1: Vector8f = x[x_idx + 5 ..][0..8].*;
        a1 += @select(f32, t1 == one, xv1, zero) - @select(f32, t1 == neg_one, xv1, zero);

        const t2: Vector8f = TRIT5_LUT_F32[packed_weights[byte_idx + 2]];
        const xv2: Vector8f = x[x_idx + 10 ..][0..8].*;
        a2 += @select(f32, t2 == one, xv2, zero) - @select(f32, t2 == neg_one, xv2, zero);

        const t3: Vector8f = TRIT5_LUT_F32[packed_weights[byte_idx + 3]];
        const xv3: Vector8f = x[x_idx + 15 ..][0..8].*;
        a3 += @select(f32, t3 == one, xv3, zero) - @select(f32, t3 == neg_one, xv3, zero);

        const t4: Vector8f = TRIT5_LUT_F32[packed_weights[byte_idx + 4]];
        const xv4: Vector8f = x[x_idx + 20 ..][0..8].*;
        a4 += @select(f32, t4 == one, xv4, zero) - @select(f32, t4 == neg_one, xv4, zero);

        const t5: Vector8f = TRIT5_LUT_F32[packed_weights[byte_idx + 5]];
        const xv5: Vector8f = x[x_idx + 25 ..][0..8].*;
        a5 += @select(f32, t5 == one, xv5, zero) - @select(f32, t5 == neg_one, xv5, zero);

        const t6: Vector8f = TRIT5_LUT_F32[packed_weights[byte_idx + 6]];
        const xv6: Vector8f = x[x_idx + 30 ..][0..8].*;
        a6 += @select(f32, t6 == one, xv6, zero) - @select(f32, t6 == neg_one, xv6, zero);

        const t7: Vector8f = TRIT5_LUT_F32[packed_weights[byte_idx + 7]];
        const xv7: Vector8f = x[x_idx + 35 ..][0..8].*;
        a7 += @select(f32, t7 == one, xv7, zero) - @select(f32, t7 == neg_one, xv7, zero);
    }

    var sum = @reduce(.Add, ((a0 + a1) + (a2 + a3)) + ((a4 + a5) + (a6 + a7)));

    // Tail
    while (byte_idx < total_bytes) : (byte_idx += 1) {
        const b = packed_weights[byte_idx];
        if (b < 243) {
            const trits = &TRIT5_LUT_F32[b];
            var i: usize = 0;
            while (i < 5 and x_idx < n) : ({
                i += 1;
                x_idx += 1;
            }) {
                const t = trits[i];
                const xi = x[x_idx];
                if (t == 1.0) {
                    sum += xi;
                } else if (t == -1.0) {
                    sum -= xi;
                }
            }
        }
    }

    return sum;
}

pub fn main() !void {
    const stdout = std.io.getStdOut().writer();
    const N_FLOATS = 4096;
    const N_BYTES = (N_FLOATS + 4) / 5;

    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const allocator = gpa.allocator();

    const packed_weights = try allocator.alloc(u8, N_BYTES);
    defer allocator.free(packed_weights);
    const x = try allocator.alloc(f32, N_FLOATS);
    defer allocator.free(x);

    for (packed_weights, 0..) |*p, i| p.* = @intCast(i % 243);
    for (x, 0..) |*v, i| v.* = @as(f32, @floatFromInt(i % 100)) * 0.01 - 0.5;

    const ITERS: usize = 20000;
    var min_cycles_8x: u64 = std.math.maxInt(u64);
    var res_8x: f32 = 0;
    var it: usize = 0;
    while (it < ITERS) : (it += 1) {
        const t0 = rdtsc_start();
        res_8x = dot_trit5_avx1_unrolled8(packed_weights, x, N_FLOATS);
        const t1 = rdtsc_end();
        const dt = t1 - t0;
        if (dt < min_cycles_8x) min_cycles_8x = dt;
    }

    try stdout.print("8x Unrolled Dual-Pipe: {:>6} cycles ({d:.3} cycles/float) [Result: {d:.4}]\n", .{ min_cycles_8x, @as(f64, @floatFromInt(min_cycles_8x)) / @as(f64, @floatFromInt(N_FLOATS)), res_8x });
}
