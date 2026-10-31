const std = @import("std");

pub const TRIT5_LUT_F32: [243][8]f32 = init_lut: {
    @setEvalBranchQuota(100000);
    var lut: [243][8]f32 = [_][8]f32{[_]f32{0.0} ** 8} ** 243;
    var b: usize = 0;
    while (b < 243) : (b += 1) {
        var curr = b;
        var i: usize = 0;
        while (i < 5) : (i += 1) {
            const u = curr % 3;
            const t: f32 = @floatFromInt(@as(i32, @intCast(u)) - 1);
            lut[b][i] = t;
            curr /= 3;
        }
    }
    break :init_lut lut;
};

// RDTSC hardware timer with CPUID serialization
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

// 1. Scalar Fallback
pub fn dot_trit5_scalar(packed_weights: []const u8, x: []const f32, n: usize) f32 {
    var accum: f32 = 0.0;
    var x_idx: usize = 0;
    for (packed_weights) |b| {
        if (b >= 243) continue;
        const trits = &TRIT5_LUT_F32[b];
        var i: usize = 0;
        while (i < 5 and x_idx < n) : ({
            i += 1;
            x_idx += 1;
        }) {
            const t = trits[i];
            const xi = x[x_idx];
            if (t == 1.0) {
                accum += xi;
            } else if (t == -1.0) {
                accum -= xi;
            }
        }
    }
    return accum;
}

// 2. AVX1 Vectorized Single Accumulator (256-bit)
pub fn dot_trit5_avx1_single(packed_weights: []const u8, x: []const f32, n: usize) f32 {
    const Vector8f = @Vector(8, f32);
    var accum: Vector8f = @splat(0.0);
    const one: Vector8f = @splat(1.0);
    const neg_one: Vector8f = @splat(-1.0);

    var byte_idx: usize = 0;
    var x_idx: usize = 0;
    const total_bytes = packed_weights.len;

    while (byte_idx < total_bytes and x_idx + 8 <= n) : ({
        byte_idx += 1;
        x_idx += 5;
    }) {
        const b = packed_weights[byte_idx];
        if (b < 243) {
            const trits: Vector8f = TRIT5_LUT_F32[b];
            const xv: Vector8f = x[x_idx..][0..8].*;

            const pos_mask = trits == one;
            const neg_mask = trits == neg_one;

            const pos_vals = @select(f32, pos_mask, xv, @as(Vector8f, @splat(0.0)));
            const neg_vals = @select(f32, neg_mask, xv, @as(Vector8f, @splat(0.0)));

            accum += (pos_vals - neg_vals);
        }
    }

    var sum = @reduce(.Add, accum);

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

// 3. AVX1 Vectorized 4x Unrolled (Dual-Port Pipe for Ivy Bridge)
pub fn dot_trit5_avx1_unrolled4(packed_weights: []const u8, x: []const f32, n: usize) f32 {
    const Vector8f = @Vector(8, f32);
    var acc0: Vector8f = @splat(0.0);
    var acc1: Vector8f = @splat(0.0);
    var acc2: Vector8f = @splat(0.0);
    var acc3: Vector8f = @splat(0.0);
    const one: Vector8f = @splat(1.0);
    const neg_one: Vector8f = @splat(-1.0);
    const zero: Vector8f = @splat(0.0);

    var byte_idx: usize = 0;
    var x_idx: usize = 0;
    const total_bytes = packed_weights.len;

    // Process 4 bytes (20 trits) per unrolled iteration
    while (byte_idx + 4 <= total_bytes and x_idx + 23 <= n) : ({
        byte_idx += 4;
        x_idx += 20;
    }) {
        const b0 = packed_weights[byte_idx];
        const b1 = packed_weights[byte_idx + 1];
        const b2 = packed_weights[byte_idx + 2];
        const b3 = packed_weights[byte_idx + 3];

        const t0: Vector8f = TRIT5_LUT_F32[b0];
        const xv0: Vector8f = x[x_idx..][0..8].*;
        const p0 = @select(f32, t0 == one, xv0, zero);
        const n0 = @select(f32, t0 == neg_one, xv0, zero);
        acc0 += (p0 - n0);

        const t1: Vector8f = TRIT5_LUT_F32[b1];
        const xv1: Vector8f = x[x_idx + 5 ..][0..8].*;
        const p1 = @select(f32, t1 == one, xv1, zero);
        const n1 = @select(f32, t1 == neg_one, xv1, zero);
        acc1 += (p1 - n1);

        const t2: Vector8f = TRIT5_LUT_F32[b2];
        const xv2: Vector8f = x[x_idx + 10 ..][0..8].*;
        const p2 = @select(f32, t2 == one, xv2, zero);
        const n2 = @select(f32, t2 == neg_one, xv2, zero);
        acc2 += (p2 - n2);

        const t3: Vector8f = TRIT5_LUT_F32[b3];
        const xv3: Vector8f = x[x_idx + 15 ..][0..8].*;
        const p3 = @select(f32, t3 == one, xv3, zero);
        const n3 = @select(f32, t3 == neg_one, xv3, zero);
        acc3 += (p3 - n3);
    }

    var sum = @reduce(.Add, (acc0 + acc1) + (acc2 + acc3));

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
    try stdout.print("\n=== POLER Trit5 Silicon Micro-Benchmark (Intel Core i7-3770 Ivy Bridge) ===\n\n", .{});

    // Hidden dim 4096 (standard LLM dimension) -> 4096 floats = 820 Trit5 bytes
    const N_FLOATS = 4096;
    const N_BYTES = (N_FLOATS + 4) / 5;

    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const allocator = gpa.allocator();

    const packed_weights = try allocator.alloc(u8, N_BYTES);
    defer allocator.free(packed_weights);
    const x = try allocator.alloc(f32, N_FLOATS);
    defer allocator.free(x);

    // Deterministic test data
    for (packed_weights, 0..) |*p, i| {
        p.* = @intCast(i % 243);
    }
    for (x, 0..) |*v, i| {
        v.* = @as(f32, @floatFromInt(i % 100)) * 0.01 - 0.5;
    }

    // Warmup caches (L1D = 32KB, test size = ~16KB fits completely in L1D)
    _ = dot_trit5_scalar(packed_weights, x, N_FLOATS);
    _ = dot_trit5_avx1_single(packed_weights, x, N_FLOATS);
    _ = dot_trit5_avx1_unrolled4(packed_weights, x, N_FLOATS);

    const ITERS: usize = 20000;

    // Benchmark 1: Scalar Fallback
    var min_cycles_scalar: u64 = std.math.maxInt(u64);
    var res_scalar: f32 = 0;
    {
        var it: usize = 0;
        while (it < ITERS) : (it += 1) {
            const t0 = rdtsc_start();
            res_scalar = dot_trit5_scalar(packed_weights, x, N_FLOATS);
            const t1 = rdtsc_end();
            const dt = t1 - t0;
            if (dt < min_cycles_scalar) min_cycles_scalar = dt;
        }
    }

    // Benchmark 2: AVX1 Single Accumulator
    var min_cycles_avx1_single: u64 = std.math.maxInt(u64);
    var res_avx1_single: f32 = 0;
    {
        var it: usize = 0;
        while (it < ITERS) : (it += 1) {
            const t0 = rdtsc_start();
            res_avx1_single = dot_trit5_avx1_single(packed_weights, x, N_FLOATS);
            const t1 = rdtsc_end();
            const dt = t1 - t0;
            if (dt < min_cycles_avx1_single) min_cycles_avx1_single = dt;
        }
    }

    // Benchmark 3: AVX1 4x Unrolled Dual-Pipe
    var min_cycles_avx1_unroll: u64 = std.math.maxInt(u64);
    var res_avx1_unroll: f32 = 0;
    {
        var it: usize = 0;
        while (it < ITERS) : (it += 1) {
            const t0 = rdtsc_start();
            res_avx1_unroll = dot_trit5_avx1_unrolled4(packed_weights, x, N_FLOATS);
            const t1 = rdtsc_end();
            const dt = t1 - t0;
            if (dt < min_cycles_avx1_unroll) min_cycles_avx1_unroll = dt;
        }
    }

    try stdout.print("Vector Size: {} floats ({} Trit5 bytes)\n", .{ N_FLOATS, N_BYTES });
    try stdout.print("Validation Check: Scalar={d:.4}, AVX1_Single={d:.4}, AVX1_4x={d:.4}\n", .{ res_scalar, res_avx1_single, res_avx1_unroll });
    try stdout.print("\n--- Silicon Performance (RDTSC Min Cycles over {} runs) ---\n", .{ITERS});
    try stdout.print("1. Scalar Fallback:             {:>6} cycles ({d:.3} cycles/float)\n", .{ min_cycles_scalar, @as(f64, @floatFromInt(min_cycles_scalar)) / @as(f64, @floatFromInt(N_FLOATS)) });
    try stdout.print("2. AVX1 Single Accumulator:     {:>6} cycles ({d:.3} cycles/float) -> {d:.2}x speedup\n", .{ min_cycles_avx1_single, @as(f64, @floatFromInt(min_cycles_avx1_single)) / @as(f64, @floatFromInt(N_FLOATS)), @as(f64, @floatFromInt(min_cycles_scalar)) / @as(f64, @floatFromInt(min_cycles_avx1_single)) });
    try stdout.print("3. AVX1 4x Unrolled Dual-Pipe:  {:>6} cycles ({d:.3} cycles/float) -> {d:.2}x speedup\n", .{ min_cycles_avx1_unroll, @as(f64, @floatFromInt(min_cycles_avx1_unroll)) / @as(f64, @floatFromInt(N_FLOATS)), @as(f64, @floatFromInt(min_cycles_scalar)) / @as(f64, @floatFromInt(min_cycles_avx1_unroll)) });

    try stdout.print("\n=== Benchmark Completed Successfully ===\n\n", .{});
}
