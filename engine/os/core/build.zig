const std = @import("std");

// ============================================================================
// POLER Core — сборка как суверенная статическая библиотека (C-ABI слой)
// ============================================================================
//
// Исторически poler_core.zig жил внутри Ring-0 ядра poler-os
// (zig-kernel/src64/). После консолидации монорепозитория (M3) криптографи-
// ческое ядро выделено в os/core/ и собирается как обычная userspace
// статическая библиотека для Linux/macOS/Windows — без голого железа,
// без QEMU, без загрузочных секторов.
//
//   zig build              # → zig-out/lib/libpoler_core.a (root = abi.zig)
//   zig build test         # тесты poler_core.zig (30) + parity-тесты abi.zig
//   zig build -Doptimize=ReleaseFast
//
// M4 (docs/UNIFIED_ARCHITECTURE.md): Rust-сторона poler-engine линкует
// эту библиотеку через фичу `pnd-ffi` (см. build.rs в корне монорепо и
// src/crypto/pnd.rs). Ядро PND v8.2 включает P0-фиксы аудита Шнайера
// (F1: полный 256-битный ключ; F3: PolerDrbg; F2: PolerCbc).
// ============================================================================

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // Статическая библиотека с C-ABI экспортами: libpoler_core.a / .lib
    const lib = b.addStaticLibrary(.{
        .name = "poler_core",
        .root_source_file = b.path("abi.zig"),
        .target = target,
        .optimize = optimize,
    });
    // PIC: потребители линкуют библиотеку в PIE-бинарники (Rust-тесты,
    // CI) — без этого ld падает на R_X86_64_32S против локальных символов.
    // (Zig 0.14: флаг живёт на root_module, а не на Compile-степе.)
    lib.root_module.pic = true;
    // Встраиваем compiler-rt: иначе линкер потребителя не найдёт
    // __zig_probe_stack и прочие встроенные процедуры.
    lib.bundle_compiler_rt = true;
    b.installArtifact(lib);

    // Тесты крипто-ядра (30: 23 исходных + 7 P0 аудита Шнайера)
    const core_tests = b.addTest(.{
        .root_source_file = b.path("poler_core.zig"),
        .target = target,
        .optimize = optimize,
    });
    const run_core_tests = b.addRunArtifact(core_tests);

    // Parity-тесты C-ABI слоя (экспортные функции ≡ прямые вызовы ядра)
    const abi_tests = b.addTest(.{
        .root_source_file = b.path("abi.zig"),
        .target = target,
        .optimize = optimize,
    });
    const run_abi_tests = b.addRunArtifact(abi_tests);

    // E1/v0.31.0: тесты полер-исполнителя (raw-syscall слой + живой fork/exec)
    const exec_tests = b.addTest(.{
        .root_source_file = b.path("poler_exec.zig"),
        .target = target,
        .optimize = optimize,
    });
    const run_exec_tests = b.addRunArtifact(exec_tests);

    const test_step = b.step("test", "Run poler_core crypto self-tests + C-ABI parity tests + exec tests");
    test_step.dependOn(&run_core_tests.step);
    test_step.dependOn(&run_abi_tests.step);
    test_step.dependOn(&run_exec_tests.step);
}
