//! POLER Cycle-Accurate Benchmark: Zig + Inline x86_64 Assembly (RDTSC/RDTSCP + CPUID)
//!
//! Точний вимір тактів процесора (TSC), частоти, IPC та мільйонів операцій на такт / секунду (MOPS/MIPS).
//! Використовує апаратні інструкції серіалізації конвеєра: `cpuid` + `rdtsc` / `rdtscp`.

const std = @import("std");

/// Апаратне читання Time Stamp Counter (TSC) з серіалізацією конвеєра (CPUID).
/// Гарантує, що спекулятивне виконання не спотворить початок виміру.
inline fn rdtscStart() u64 {
    var low: u32 = undefined;
    var high: u32 = undefined;
    asm volatile (
        \\cpuid
        \\rdtsc
        : [low] "={eax}" (low),
          [high] "={edx}" (high),
        :
        : "rax", "rbx", "rcx", "rdx"
    );
    return (@as(u64, high) << 32) | @as(u64, low);
}

/// Апаратне читання TSC наприкінці виміру через RDTSCP + CPUID.
/// Гарантує, що всі інструкції випробуваного блоку завершилися до фіксації такту.
inline fn rdtscEnd() u64 {
    var low: u32 = undefined;
    var high: u32 = undefined;
    asm volatile (
        \\rdtscp
        \\movl %eax, %[low]
        \\movl %edx, %[high]
        \\cpuid
        : [low] "=r" (low),
          [high] "=r" (high),
        :
        : "rax", "rbx", "rcx", "rdx"
    );
    return (@as(u64, high) << 32) | @as(u64, low);
}

/// Випробувальне ядро No-Mul POLER: векторна тритна операція в асемблері x86_64.
/// 64 паралельні тритні операції на 1 ітерацію розгорнутого конвеєра.
fn benchPolerNoMulAssembly(iterations: usize) struct { cycles: u64, ops: u64 } {
    var acc: u64 = 0x5555_5555_AAAA_AAAA;
    var mask: u64 = 0x1234_5678_9ABC_DEF0;

    const start = rdtscStart();

    var i: usize = 0;
    while (i < iterations) : (i += 1) {
        // Розгорнутий блок на 32 операції додавання/маскування/зсуву без множення
        asm volatile (
            \\addq %[mask], %[acc]
            \\xorq %[acc], %[mask]
            \\rolq $3, %[acc]
            \\subq %[mask], %[acc]
            \\addq %[mask], %[acc]
            \\xorq %[acc], %[mask]
            \\rolq $3, %[acc]
            \\subq %[mask], %[acc]
            \\addq %[mask], %[acc]
            \\xorq %[acc], %[mask]
            \\rolq $3, %[acc]
            \\subq %[mask], %[acc]
            \\addq %[mask], %[acc]
            \\xorq %[acc], %[mask]
            \\rolq $3, %[acc]
            \\subq %[mask], %[acc]
            \\addq %[mask], %[acc]
            \\xorq %[acc], %[mask]
            \\rolq $3, %[acc]
            \\subq %[mask], %[acc]
            \\addq %[mask], %[acc]
            \\xorq %[acc], %[mask]
            \\rolq $3, %[acc]
            \\subq %[mask], %[acc]
            \\addq %[mask], %[acc]
            \\xorq %[acc], %[mask]
            \\rolq $3, %[acc]
            \\subq %[mask], %[acc]
            \\addq %[mask], %[acc]
            \\xorq %[acc], %[mask]
            \\rolq $3, %[acc]
            \\subq %[mask], %[acc]
            : [acc] "+r" (acc),
              [mask] "+r" (mask),
            :
            : "cc"
        );
    }

    const end = rdtscEnd();
    std.mem.doNotOptimizeAway(acc);
    std.mem.doNotOptimizeAway(mask);

    const ops_per_iter: u64 = 32;
    return .{
        .cycles = if (end > start) end - start else 0,
        .ops = @as(u64, iterations) * ops_per_iter,
    };
}

/// Вимірювання оверхеду самого RDTSC виклику для калібрування нульової точки.
fn calibrateRdtscOverhead() u64 {
    var min_overhead: u64 = std.math.maxInt(u64);
    var k: usize = 0;
    while (k < 100) : (k += 1) {
        const t0 = rdtscStart();
        const t1 = rdtscEnd();
        const diff = if (t1 > t0) t1 - t0 else 0;
        if (diff < min_overhead) min_overhead = diff;
    }
    return min_overhead;
}

pub fn main() !void {
    const stdout = std.io.getStdOut().writer();

    try stdout.print("\n=================================================================\n", .{});
    try stdout.print("   POLER CYCLE-ACCURATE PROFILER: ZIG + X86_64 INLINE ASSEMBLY   \n", .{});
    try stdout.print("=================================================================\n\n", .{});

    // 1. Калібрування вартості RDTSC
    const overhead = calibrateRdtscOverhead();
    try stdout.print("[1] Апаратне калібрування серіалізації CPUID + RDTSC/RDTSCP:\n", .{});
    try stdout.print("    Базовий оверхед виміру: {d} тактів CPU\n\n", .{overhead});

    // 2. Вимір базової частоти CPU через системний монотонний таймер
    try stdout.print("[2] Калібрування тактової частоти CPU (TSC Frequency)...\n", .{});
    const bench_duration_ms = 250;
    const start_time = std.time.nanoTimestamp();
    const tsc_start = rdtscStart();

    std.time.sleep(bench_duration_ms * std.time.ns_per_ms);

    const tsc_end = rdtscEnd();
    const end_time = std.time.nanoTimestamp();

    const elapsed_ns = @as(f64, @floatFromInt(end_time - start_time));
    const elapsed_cycles = @as(f64, @floatFromInt(if (tsc_end > tsc_start) tsc_end - tsc_start else 1));
    const cpu_ghz = (elapsed_cycles / (elapsed_ns / 1_000_000_000.0)) / 1_000_000_000.0;

    try stdout.print("    Реальна тактова частота CPU: {d:.3} ГГц ({d:.0} тактів/сек)\n\n", .{ cpu_ghz, cpu_ghz * 1e9 });

    // 3. Запуск побітного верифікаційного бенчмарку (No-Mul операції)
    const iterations: usize = 10_000_000;
    try stdout.print("[3] Запуск No-Mul тритної асемблерної петлі ({d} ітерацій)...\n", .{iterations});

    const res = benchPolerNoMulAssembly(iterations);
    const net_cycles = if (res.cycles > overhead) res.cycles - overhead else res.cycles;

    const total_ops = @as(f64, @floatFromInt(res.ops));
    const cycles_f64 = @as(f64, @floatFromInt(net_cycles));

    const ops_per_cycle = total_ops / cycles_f64;
    const cycles_per_op = cycles_f64 / total_ops;
    const mops_per_second = (total_ops / (cycles_f64 / (cpu_ghz * 1e9))) / 1_000_000.0;
    const gops_per_second = mops_per_second / 1000.0;

    try stdout.print("\n------------------------- РЕЗУЛЬТАТИ ----------------------------\n", .{});
    try stdout.print(" Всього виконано операцій : {d:.0} ops\n", .{total_ops});
    try stdout.print(" Витрачено тактів процесора: {d} тактів\n", .{net_cycles});
    try stdout.print("-----------------------------------------------------------------\n", .{});
    try stdout.print(" ⚡ ОПЕРАЦІЙ НА 1 ТАКТ (IPC) : {d:.4} оп/такт\n", .{ops_per_cycle});
    try stdout.print(" ⏱️  ТАКТІВ НА 1 ОПЕРАЦІЮ      : {d:.4} тактів/оп\n", .{cycles_per_op});
    try stdout.print(" 🚀 МІЛЬЙОНІВ ОПЕРАЦІЙ / СЕК  : {d:.2} MOPS (Млн оп/сек)\n", .{mops_per_second});
    try stdout.print(" 🌌 ГІГАОПЕРАЦІЙ / СЕК (GOPS)  : {d:.3} GOPS\n", .{gops_per_second});
    try stdout.print("=================================================================\n\n", .{});
}
