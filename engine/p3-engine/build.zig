// =============================================================================
// build.zig (vendored P³ Engine в poler-engine) — цикл W «Симбиоз»
// =============================================================================
// Минимальный сборочный скрипт интеграционного слоя poler-engine.
// Оригинальный build.zig P3_Engine (лаунчер O3DE) остаётся в апстриме:
// https://github.com/Kotokvit/P3_Engine
//
// Здесь собирается ОДИН артефакт — libp3ffi.so (C-ABI мост к POLER):
//
//   zig build p3-ffi -Doptimize=ReleaseFast
//   → zig-out/lib/libp3ffi.so
//
// ⚠️ КРИТИЧЕСКИ ВАЖНО: цель по умолчанию — БАЗОВЫЙ x86-64 (SSE2).
// Никакого native-CPU автодетекта: старая .so была собрана под AVX2
// и падала SIGILL на Ivy Bridge (i7-3770). cpu_model = .baseline
// гарантирует исполнение на любом x86-64 с 2003 года.
//
// Тесты FFI-слоя: zig build test-ffi
// =============================================================================

const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{
        .default_target = .{
            .cpu_arch = .x86_64,
            .os_tag = .linux,
            .abi = .gnu,
            // Лечение SIGILL: код только из базового набора x86-64 (SSE2).
            .cpu_model = .baseline,
        },
    });
    const optimize = b.standardOptimizeOption(.{});

    // --- libp3ffi.so: C-ABI мост P³ → POLER ENGINE ---
    const p3ffi = b.addSharedLibrary(.{
        .name = "p3ffi",
        .root_source_file = b.path("src/p3_ffi.zig"),
        .target = target,
        .optimize = optimize,
    });
    b.installArtifact(p3ffi);
    const p3ffi_step = b.step("p3-ffi", "Сборка libp3ffi.so (C-ABI мост P³ → POLER)");
    p3ffi_step.dependOn(b.getInstallStep());

    // --- тесты FFI-слоя ---
    const ffi_tests = b.addTest(.{
        .root_source_file = b.path("src/p3_ffi.zig"),
        .target = target,
        .optimize = optimize,
    });
    const run_ffi_tests = b.addRunArtifact(ffi_tests);
    const test_step = b.step("test-ffi", "Тесты C-ABI слоя p3ffi");
    test_step.dependOn(&run_ffi_tests.step);

    // --- дым всего vendored дерева (ядро P³) ---
    const kernel_tests = b.addTest(.{
        .root_source_file = b.path("src/root.zig"),
        .target = target,
        .optimize = optimize,
    });
    const run_kernel_tests = b.addRunArtifact(kernel_tests);
    const kernel_test_step = b.step("test-kernel", "Тесты ядра P³ (vendored)");
    kernel_test_step.dependOn(&run_kernel_tests.step);
}
