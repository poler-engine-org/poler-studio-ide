//! POLER Event-Driven + Software Prefetch FlyWire Brain Profiler (Zig + x86_64 ASM)
//!
//! Порівняння:
//! 1. Базовий синхронний обхід (Dense Scan 2.7M рёбер)
//! 2. Синхронний обхід із Software Prefetch (`prefetchnta` на +16 / +32 ребра вперед)
//! 3. Справжня біологічна Event-Driven симуляція (Active Spikes Queue, 1-5% спайкова активність)

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

/// Апаратний Prefetch NTA (Non-Temporal Access: повз кеш у L1 без вимивання ліній)
inline fn prefetchNta(ptr: anytype) void {
    asm volatile ("prefetchnta (%[p])"
        :
        : [p] "r" (ptr),
        : "memory"
    );
}

pub fn main() !void {
    const stdout = std.io.getStdOut().writer();
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const allocator = gpa.allocator();

    try stdout.print("\n=================================================================\n", .{});
    try stdout.print("   POLER EVENT-DRIVEN & PREFETCH BENCHMARK: FLYWIRE-SCALE       \n", .{});
    try stdout.print("   Референс оригінального запуску: i7-3770 / DDR3-1600, 1 Thread \n", .{});
    try stdout.print("   Поточна машина: частота вимірюється нижче (див. [1])         \n", .{});
    try stdout.print("   УВАГА: топологія СИНТЕТИЧНА (граф масштабу FlyWire), не       \n", .{});
    try stdout.print("   реальний CSR коннектом — див. docs/flywire-connectome/.       \n", .{});
    try stdout.print("=================================================================\n\n", .{});

    // 1. Калібрування частоти
    const bench_duration_ms = 150;
    const t0 = std.time.nanoTimestamp();
    const c0 = rdtscStart();
    std.time.sleep(bench_duration_ms * std.time.ns_per_ms);
    const c1 = rdtscEnd();
    const t1 = std.time.nanoTimestamp();
    const elapsed_ns = @as(f64, @floatFromInt(t1 - t0));
    const elapsed_cycles = @as(f64, @floatFromInt(if (c1 > c0) c1 - c0 else 1));
    const cpu_ghz = (elapsed_cycles / (elapsed_ns / 1e9)) / 1e9;
    try stdout.print("[1] Частота CPU під час тесту: {d:.3} ГГц\n\n", .{cpu_ghz});

    // 2. Ініціалізація графа мозку мухи в CSR-структурі
    const n_neurons: usize = 138_639;
    const n_edges_core: usize = 2_700_513;

    try stdout.print("[2] Побудова CSR-індексованого графа (Forward Adjacency List)...\n", .{});

    // row_offsets для справжнього CSR (щоб event-driven йшов тільки по вихідних ребрах активного нейрона)
    const row_offsets = try allocator.alloc(u32, n_neurons + 1);
    defer allocator.free(row_offsets);
    const col_targets = try allocator.alloc(u32, n_edges_core);
    defer allocator.free(col_targets);
    const edge_weights = try allocator.alloc(i8, n_edges_core);
    defer allocator.free(edge_weights);

    // Вектор мембранних потенціалів V_i та порогів theta
    const membrane_v = try allocator.alloc(i16, n_neurons);
    defer allocator.free(membrane_v);
    @memset(membrane_v, 0);

    // Рівномірний розподіл ступенів для CSR
    var prng = std.Random.DefaultPrng.init(0xCAFE_783);
    const rand = prng.random();

    const avg_degree: usize = n_edges_core / n_neurons; // ~19-20 ребер на нейрон
    var current_edge: u32 = 0;
    for (0..n_neurons) |i| {
        row_offsets[i] = current_edge;
        const degree: u32 = @intCast(rand.intRangeAtMost(usize, avg_degree / 2, avg_degree * 3 / 2));
        var d: u32 = 0;
        while (d < degree and current_edge < n_edges_core) : (d += 1) {
            col_targets[current_edge] = rand.intRangeAtMost(u32, 0, @as(u32, @intCast(n_neurons - 1)));
            const w_trit = rand.intRangeAtMost(i8, -1, 1);
            edge_weights[current_edge] = if (w_trit == 0) 1 else w_trit;
            current_edge += 1;
        }
    }
    row_offsets[n_neurons] = current_edge;
    const actual_edges = current_edge;

    try stdout.print("    - Вершин: {d}, Зв'язків: {d}\n\n", .{ n_neurons, actual_edges });

    // ----------------------------------------------------------------------
    // ТЕСТ 1: Базовий синхронний обхід (Dense Scan без prefetch)
    // ----------------------------------------------------------------------
    try stdout.print("[3] ТЕСТ 1: Базовий синхронний обхід (Dense Scan, 2.7M рёбер)...\n", .{});
    const steps_dense: usize = 30;
    const t_dense_0 = rdtscStart();

    var s1: usize = 0;
    while (s1 < steps_dense) : (s1 += 1) {
        var e: usize = 0;
        while (e < actual_edges) : (e += 1) {
            const tgt = col_targets[e];
            const w = edge_weights[e];
            membrane_v[tgt] +%= w;
        }
    }
    const t_dense_1 = rdtscEnd();
    const cyc_dense = @as(f64, @floatFromInt(t_dense_1 - t_dense_0)) / @as(f64, @floatFromInt(steps_dense));
    const ms_dense = (cyc_dense / (cpu_ghz * 1e9)) * 1000.0;
    const hz_dense = 1000.0 / ms_dense;
    try stdout.print("    - Час кроку : {d:.3} мс ({d:.1} Гц)\n\n", .{ ms_dense, hz_dense });

    // ----------------------------------------------------------------------
    // ТЕСТ 2: Синхронний обхід + Software Prefetch (+16 ребер вперед)
    // ----------------------------------------------------------------------
    try stdout.print("[4] ТЕСТ 2: Синхронний обхід + Software PREFETCH (+16 ahead)...\n", .{});
    const t_pref_0 = rdtscStart();

    var s2: usize = 0;
    while (s2 < steps_dense) : (s2 += 1) {
        var e: usize = 0;
        while (e < actual_edges) : (e += 1) {
            // Апаратна вибірка адреси цільового нейрона на 16 кроків уперед
            if (e + 16 < actual_edges) {
                const next_tgt = col_targets[e + 16];
                prefetchNta(&membrane_v[next_tgt]);
            }

            const tgt = col_targets[e];
            const w = edge_weights[e];
            membrane_v[tgt] +%= w;
        }
    }
    const t_pref_1 = rdtscEnd();
    const cyc_pref = @as(f64, @floatFromInt(t_pref_1 - t_pref_0)) / @as(f64, @floatFromInt(steps_dense));
    const ms_pref = (cyc_pref / (cpu_ghz * 1e9)) * 1000.0;
    const hz_pref = 1000.0 / ms_pref;
    try stdout.print("    - Час кроку : {d:.3} мс ({d:.1} Гц)\n", .{ ms_pref, hz_pref });
    try stdout.print("    - Прискорення від Prefetch: {d:.2}x\n\n", .{ms_dense / ms_pref});

    // ----------------------------------------------------------------------
    // ТЕСТ 3: Справжня Біологічна Event-Driven Стрижнева модель (Active Spikes)
    // ----------------------------------------------------------------------
    try stdout.print("[5] ТЕСТ 3: EVENT-DRIVEN СИМУЛЯЦІЯ (Активність: 3.5% спайків)...\n", .{});

    // Очередь спайків: тільки активні нейрони
    const max_spikes = n_neurons;
    const spike_queue = try allocator.alloc(u32, max_spikes);
    defer allocator.free(spike_queue);

    // Ініціалізація активності ~3.5% (біологічний спайковий рейт мозку мухи)
    var spike_count: usize = 0;
    for (0..n_neurons) |u| {
        if (rand.intRangeAtMost(u8, 0, 100) < 4) {
            spike_queue[spike_count] = @intCast(u);
            spike_count += 1;
        }
    }

    const steps_event: usize = 200;
    const t_event_0 = rdtscStart();

    var s3: usize = 0;
    var total_processed_edges: usize = 0;
    while (s3 < steps_event) : (s3 += 1) {
        var next_spike_count: usize = 0;

        // Обробляємо ТІЛЬКИ аксони нейронів, які випустили спайк
        for (0..spike_count) |spk_idx| {
            const fired_neuron = spike_queue[spk_idx];
            const start_edge = row_offsets[fired_neuron];
            const end_edge = row_offsets[fired_neuron + 1];

            var edge_idx = start_edge;
            while (edge_idx < end_edge) : (edge_idx += 1) {
                // Prefetch наступного синапсу
                if (edge_idx + 8 < end_edge) {
                    const pref_tgt = col_targets[edge_idx + 8];
                    prefetchNta(&membrane_v[pref_tgt]);
                }

                const tgt = col_targets[edge_idx];
                const w = edge_weights[edge_idx];
                membrane_v[tgt] +%= w;

                // Якщо досягнуто поріг LIF (Theta = 15) -> генеруємо спайк на наступний такт
                if (membrane_v[tgt] > 15 and next_spike_count < max_spikes - 1) {
                    membrane_v[tgt] = 0; // Рефрактерний скид
                    spike_queue[next_spike_count] = tgt;
                    next_spike_count += 1;
                }
            }
            total_processed_edges += (end_edge - start_edge);
        }

        // Оновлюємо популяцію спайків для наступного кроку (якщо згасає — даємо біологічний фоновий шум 1%)
        if (next_spike_count < 1000) {
            for (0..3000) |rnd_s| {
                const rnd_neu = (rnd_s * 47) % n_neurons;
                spike_queue[next_spike_count] = @intCast(rnd_neu);
                next_spike_count += 1;
            }
        }
        spike_count = next_spike_count;
    }
    const t_event_1 = rdtscEnd();
    const cyc_event = @as(f64, @floatFromInt(t_event_1 - t_event_0)) / @as(f64, @floatFromInt(steps_event));
    const ms_event = (cyc_event / (cpu_ghz * 1e9)) * 1000.0;
    const hz_event = 1000.0 / ms_event;

    try stdout.print("    - Час кроку : {d:.3} мс ({d:.1} Гц / {d:.2} кГц)\n", .{ ms_event, hz_event, hz_event / 1000.0 });
    try stdout.print("    - Оброблено ребер/крок (фактично торкнуто): {d:.0} з {d} (розрідженість {d:.1}%)\n", .{ @as(f64, @floatFromInt(total_processed_edges)) / @as(f64, @floatFromInt(steps_event)), @as(f64, @floatFromInt(actual_edges)), 100.0 * @as(f64, @floatFromInt(total_processed_edges)) / (@as(f64, @floatFromInt(steps_event)) * @as(f64, @floatFromInt(actual_edges))) });
    try stdout.print("    - Прискорення проти базового Dense : {d:.2}x\n\n", .{ms_dense / ms_event});

    std.mem.doNotOptimizeAway(total_processed_edges);

    // ----------------------------------------------------------------------
    // ПІДСУМКОВА ТАБЛИЦЯ
    // ----------------------------------------------------------------------
    try stdout.print("======================= ПІДСУМКОВА ТАБЛИЦЯ ======================\n", .{});
    try stdout.print(" Режим симуляції               | Час кроку | Частота    | Прискорення \n", .{});
    try stdout.print("-----------------------------------------------------------------\n", .{});
    try stdout.print(" 1. Синхронний Dense Scan      | {d:6.3} мс | {d:6.1} Гц  |  1.00x      \n", .{ ms_dense, hz_dense });
    try stdout.print(" 2. Dense + Software PREFETCH  | {d:6.3} мс | {d:6.1} Гц  |  {d:4.2}x      \n", .{ ms_pref, hz_pref, ms_dense / ms_pref });
    try stdout.print(" 3. EVENT-DRIVEN + Prefetch    | {d:6.3} мс | {d:6.1} кГц | {d:4.1}x      \n", .{ ms_event, hz_event / 1000.0, ms_dense / ms_event });
    try stdout.print("=================================================================\n", .{});
    try stdout.print(" РЕАЛЬНІСТЬ: мозок масштабу FlyWire на цьому CPU працює на {d:.1} кГц\n", .{hz_event / 1000.0});
    try stdout.print(" (подієва симуляція при ~3.5% спайкової активності; синтетична топологія)\n", .{});
    if (hz_event >= 200.0) {
        try stdout.print(" Це у {d:.0}x швидше за живу біологічну муху (~200 Гц).\n", .{hz_event / 200.0});
    } else {
        try stdout.print(" Це складає {d:.0}% від швидкості живої мухи (~200 Гц).\n", .{hz_event / 200.0 * 100.0});
    }
    try stdout.print("=================================================================\n\n", .{});
}
