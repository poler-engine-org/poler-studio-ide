//! POLER FlyWire Brain Cycle & Memory Profiler: Zig + x86_64 Assembly
//! Моделювання повного кроку LIF (Leaky Integrate-and-Fire) на реальному масштабі графа мозку мухи (FlyWire v783).
//! N = 138,639 нейронів, E = 2,700,513 рёбер (core) та E = 15,091,983 рёбер (full).
//! Порівняння:
//!   1. Чистий L1/Регістровий потік (ALU Peak, No-Mul)
//!   2. Послідовний CSR доступ (Streaming Memory)
//!   3. Реальний розсіяний доступ до пам'яті (Random Gather State Memory Latency DDR3)

const std = @import("std");

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

pub fn main() !void {
    const stdout = std.io.getStdOut().writer();
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const allocator = gpa.allocator();

    try stdout.print("\n=================================================================\n", .{});
    try stdout.print("   FLYWIRE v783 REAL-SCALE BENCHMARK: ALU vs MEMORY (ZIG)      \n", .{});
    try stdout.print("   Референс оригінального запуску: i7-3770 / DDR3-1600 / L3=8МБ\n", .{});
    try stdout.print("   Поточна машина: частота вимірюється нижче (див. [1])         \n", .{});
    try stdout.print("   УВАГА: топологія СИНТЕТИЧНА (випадковий граф масштабу       \n", .{});
    try stdout.print("   FlyWire: 138,639 нейронів / 2.7М ребер), не реальний CSR     \n", .{});
    try stdout.print("   коннектом. Реальний граф — docs/flywire-connectome/.         \n", .{});
    try stdout.print("=================================================================\n\n", .{});

    // 1. Калібрування частоти
    const bench_duration_ms = 200;
    const t0 = std.time.nanoTimestamp();
    const c0 = rdtscStart();
    std.time.sleep(bench_duration_ms * std.time.ns_per_ms);
    const c1 = rdtscEnd();
    const t1 = std.time.nanoTimestamp();
    const elapsed_ns = @as(f64, @floatFromInt(t1 - t0));
    const elapsed_cycles = @as(f64, @floatFromInt(if (c1 > c0) c1 - c0 else 1));
    const cpu_ghz = (elapsed_cycles / (elapsed_ns / 1e9)) / 1e9;
    try stdout.print("[1] Частота CPU під час тесту: {d:.3} ГГц\n\n", .{cpu_ghz});

    // 2. Ініціалізація графа мозку мухи
    const n_neurons: usize = 138_639;
    const n_edges_core: usize = 2_700_513;

    try stdout.print("[2] Генерація CSR топології мозку мухи (138,639 нейронів)...\n", .{});

    // Стан нейронів (вектор потенціалів мембрани V_i)
    const neuron_states = try allocator.alloc(i16, n_neurons);
    defer allocator.free(neuron_states);
    @memset(neuron_states, 0);
    // Ініціалізація активних нейронів
    for (0..10_000) |idx| {
        neuron_states[idx * 13] = 1;
    }

    // Ребра CSR (джерела та тритні ваги)
    const sources_core = try allocator.alloc(u32, n_edges_core);
    defer allocator.free(sources_core);
    const targets_core = try allocator.alloc(u32, n_edges_core);
    defer allocator.free(targets_core);
    const weights_core = try allocator.alloc(i8, n_edges_core);
    defer allocator.free(weights_core);

    // Псевдовипадковий генератор топології FlyWire
    var prng = std.Random.DefaultPrng.init(0x1337_783);
    const rand = prng.random();

    for (0..n_edges_core) |e| {
        sources_core[e] = rand.intRangeAtMost(u32, 0, @as(u32, @intCast(n_neurons - 1)));
        targets_core[e] = rand.intRangeAtMost(u32, 0, @as(u32, @intCast(n_neurons - 1)));
        const w_trit = rand.intRangeAtMost(i8, -1, 1);
        weights_core[e] = if (w_trit == 0) 1 else w_trit;
    }

    try stdout.print("    - FlyWire CORE: {d} ребер ({d:.2} МБ пам'яті)\n", .{ n_edges_core, @as(f64, @floatFromInt(n_edges_core * 9)) / 1024.0 / 1024.0 });
    try stdout.print("    - Вектор стану: {d} нейронів ({d:.2} КБ RAM)\n\n", .{ n_neurons, @as(f64, @floatFromInt(n_neurons * 2)) / 1024.0 });

    // -------------------------------------------------------------
    // ТЕСТ 1: Повний крок нейродинаміки LIF мозку мухи (Core 2.7M ребер)
    // -------------------------------------------------------------
    try stdout.print("[3] ВИПРОБУВАННЯ 1: Повний крок нейродинаміки FlyWire Core (2.7M рёбер)...\n", .{});
    const steps_core: usize = 100;
    const start_core = rdtscStart();

    var step: usize = 0;
    while (step < steps_core) : (step += 1) {
        for (0..n_edges_core) |e| {
            const src = sources_core[e];
            const tgt = targets_core[e];
            const w = weights_core[e];

            // No-Mul LIF акумуляція вхідного синаптичного струму
            const s = neuron_states[src];
            if (s > 0) {
                if (w == 1) {
                    neuron_states[tgt] +%= 1;
                } else if (w == -1) {
                    neuron_states[tgt] -%= 1;
                }
            }
        }
    }
    const end_core = rdtscEnd();
    const cycles_core = if (end_core > start_core) end_core - start_core else 1;
    const cycles_per_step_core = @as(f64, @floatFromInt(cycles_core)) / @as(f64, @floatFromInt(steps_core));
    const time_ms_per_step_core = (cycles_per_step_core / (cpu_ghz * 1e9)) * 1000.0;
    const hz_core = 1000.0 / time_ms_per_step_core;
    const cycles_per_edge_core = cycles_per_step_core / @as(f64, @floatFromInt(n_edges_core));

    try stdout.print("    ⚡ Час одного повного кроку мозку : {d:.3} мс ({d:.0} Гц / кроків/сек)\n", .{ time_ms_per_step_core, hz_core });
    try stdout.print("    ⏱️  Тактів на один крок мозку     : {d:.0} тактів\n", .{cycles_per_step_core});
    try stdout.print("    🎯 Тактів на 1 синаптичне ребро  : {d:.2} тактів/ребро\n\n", .{cycles_per_edge_core});

    // -------------------------------------------------------------
    // ТЕСТ 2: Кеш-ієрархія vs справжня DRAM-латентність (Gather)
    // -------------------------------------------------------------
    try stdout.print("[4] ВИПРОБУВАННЯ 2: Кеш-ієрархія (L2/L3) vs DRAM (великий working set)...\n", .{});
    const bench_items: usize = 10_000_000;

    // A. Послідовний доступ по компактному масиву станів (277 КБ — L2/L3-resident + prefetch)
    const t_seq_0 = rdtscStart();
    var acc_seq: i64 = 0;
    for (0..bench_items) |idx| {
        const fake_idx = idx % n_neurons;
        acc_seq +%= neuron_states[fake_idx];
    }
    const t_seq_1 = rdtscEnd();
    const cyc_seq = @as(f64, @floatFromInt(if (t_seq_1 > t_seq_0) t_seq_1 - t_seq_0 else 1)) / @as(f64, @floatFromInt(bench_items));

    // B. Випадковий доступ по КОМПАКТНОМУ масиву (277 КБ — працює з L2/L3, НЕ з DRAM)
    const t_rand_0 = rdtscStart();
    var acc_rand: i64 = 0;
    for (0..bench_items) |e_idx| {
        const rand_idx = sources_core[e_idx % n_edges_core];
        acc_rand +%= neuron_states[rand_idx];
    }
    const t_rand_1 = rdtscEnd();
    const cyc_rand = @as(f64, @floatFromInt(if (t_rand_1 > t_rand_0) t_rand_1 - t_rand_0 else 1)) / @as(f64, @floatFromInt(bench_items));

    // C. Справжня DRAM-латентність: ВЕЛИКИЙ working set (48 МБ > L3)
    //    Масив станів масштабу повного графа + запас: 24М тритів i16 = 48 МБ.
    const n_big: usize = 24_000_000;
    const big_states = try allocator.alloc(i16, n_big);
    defer allocator.free(big_states);
    for (0..n_big) |i| {
        big_states[i] = @intCast(@as(i32, @intCast(i % 7)) - 3);
    }
    var acc_big: i64 = 0;
    var prng_big = std.Random.DefaultPrng.init(0xDDDD_0003);
    const rand_big = prng_big.random();
    // Передгенеруємо індекси, щоб RNG не забруднював вимір пам'яті
    const big_idx = try allocator.alloc(u32, bench_items);
    defer allocator.free(big_idx);
    for (0..bench_items) |i| {
        big_idx[i] = rand_big.intRangeAtMost(u32, 0, @as(u32, @intCast(n_big - 1)));
    }
    const t_big_1 = rdtscStart();
    for (0..bench_items) |i| {
        acc_big +%= big_states[big_idx[i]];
    }
    const t_big_2 = rdtscEnd();
    const cyc_dram = @as(f64, @floatFromInt(if (t_big_2 > t_big_1) t_big_2 - t_big_1 else 1)) / @as(f64, @floatFromInt(bench_items));

    std.mem.doNotOptimizeAway(acc_seq);
    std.mem.doNotOptimizeAway(acc_rand);
    std.mem.doNotOptimizeAway(acc_big);

    try stdout.print("    - Послідовний доступ (277КБ, prefetch+L2/L3) : {d:.2} тактів/читання\n", .{cyc_seq});
    try stdout.print("    - Випадковий вибір (277КБ — L2/L3-resident) : {d:.2} тактів/читання\n", .{cyc_rand});
    try stdout.print("    - Випадковий вибір (48МБ > L3 — справжня DRAM): {d:.2} тактів/читання\n", .{cyc_dram});
    try stdout.print("    - Коефіцієнт DRAM vs послідовний               : {d:.2}x\n", .{cyc_dram / cyc_seq});
    try stdout.print("    - Коефіцієнт DRAM vs L2/L3-resident            : {d:.2}x\n\n", .{cyc_dram / cyc_rand});

    // -------------------------------------------------------------
    // ПІДСУМКОВИЙ ВЕРДИКТ
    // -------------------------------------------------------------
    try stdout.print("========================= ВЕРДИКТ ===============================\n", .{});
    try stdout.print(" 1. Мозок мухи (синтетика масштабу FlyWire, 2.7М ребер) на 1 ядрі:\n", .{});
    if (hz_core >= 200.0) {
        try stdout.print("    Швидкість {d:.1} Гц — у {d:.1}x швидше за біологічну муху (~200 Гц)\n", .{ hz_core, hz_core / 200.0 });
    } else {
        try stdout.print("    Швидкість {d:.1} Гц — складає {d:.0}% від біологічної мухи (~200 Гц), тобто у {d:.1}x повільніше\n", .{ hz_core, hz_core / 200.0 * 100.0, 200.0 / hz_core });
    }
    try stdout.print(" 2. Боттлнек масштабування (15М ребер, миша):\n", .{});
    try stdout.print("    НЕ ALU (No-Mul працює за <1 такт), а випадковий доступ до DRAM\n", .{});
    try stdout.print("    (див. вимір 48МБ working set у ТЕСТІ 2C вище).\n", .{});
    try stdout.print("=================================================================\n\n", .{});
}
