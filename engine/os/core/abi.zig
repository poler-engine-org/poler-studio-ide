// ============================================================================
// POLER Core — C-ABI обвязка (слой M4, docs/UNIFIED_ARCHITECTURE.md)
// ============================================================================
//
// Экспорт крипто-ядра PND v8.2 (poler_core.zig) для внешних потребителей
// через чистый C-ABI: Rust FFI poler-engine (src/crypto/pnd.rs, фича
// `pnd-ffi`), будущие C/Python-мосты. Нулевой оверхед, нуль внешних
// зависимостей — статическая библиотека libpoler_core.a.
//
// Конвенции:
//   - скаляры передаются по значению;
//   - составные объекты (шифр, DRBG) — opaque-handle: *anyopaque,
//     выделение через std.heap.page_allocator, освобождение *_free();
//   - массивы: указатель + неявная длина по контракту (ключ — всегда
//     KEY_WORDS слов, блок — всегда BLOCK_WORDS);
//   - ошибок нет: контракт lengths проверяется debug-assert'ом Zig.
//
// Сборка (Zig 0.14.0):
//   zig build            # → zig-out/lib/libpoler_core.a
//   zig build test       # тесты poler_core.zig + parity-тесты abi.zig
//   zig build -Doptimize=ReleaseFast
// ============================================================================

const std = @import("std");
const core = @import("poler_core.zig");

const alloc = std.heap.page_allocator;

/// Версия схемы ядра (8 = PND v8; .2 — P0-фиксы аудита Шнайера).
export fn poler_core_version() u32 {
    return 8;
}

// ── Скалярные примитивы ──────────────────────────────────────────────────────

/// Биекция Φ (ARX-box: add/rotl13/xorshift16/mul/rotl7/add).
export fn poler_phi(x: u32) u32 {
    return core.phi(x);
}

/// pndMix = φ(a·b) +% ε·φ(a⊕b) — v8: φ-обёртка обоих компонент.
export fn poler_pnd_mix(a: u32, b: u32, epsilon: u32) u32 {
    return core.pndMix(a, b, epsilon);
}

/// Шаг клеточного автомата LHCA.
export fn poler_lhca_step(x: u32, rule_mask: u32) u32 {
    return core.lhcaStep(x, .{ .rule_mask = rule_mask });
}

/// Constant-time S-box (x^254 над GF(2^8)).
export fn poler_ct_sbox(x: u8) u8 {
    return core.constantTimeSbox(x);
}

/// MDS-диффузия (MixColumns, ветвление = 5).
export fn poler_mix_columns(word: u32) u32 {
    return core.mixColumnsPnd(word);
}

/// Обращение по модулю 2^32 (Hensel; для чётных a — 0).
export fn poler_mod_inverse32(a: u32) u32 {
    return core.modInverse32(a);
}

// ── Полный шифр PolerCipher (opaque handle) ─────────────────────────────────

/// Создать контекст шифра из 256-битного ключа (8 слов) и ε.
/// Возвращает null только при отказе аллокатора.
export fn poler_cipher_new(key: [*]const u32, epsilon: u32) ?*anyopaque {
    var k: [core.KEY_WORDS]u32 = undefined;
    for (&k, 0..) |*w, i| w.* = key[i];
    const c = alloc.create(core.PolerCipher) catch return null;
    c.* = core.PolerCipher.init(&k, epsilon);
    return c;
}

/// Зашифровать блок (4 слова).
export fn poler_cipher_encrypt(handle: *anyopaque, plaintext: [*]const u32, ciphertext: [*]u32) void {
    const c: *core.PolerCipher = @ptrCast(@alignCast(handle));
    var pt: [core.BLOCK_WORDS]u32 = undefined;
    for (&pt, 0..) |*w, i| w.* = plaintext[i];
    var ct: [core.BLOCK_WORDS]u32 = undefined;
    c.encryptBlock(&pt, &ct);
    for (&ct, 0..) |w, i| ciphertext[i] = w;
}

/// Расшифровать блок (4 слова).
export fn poler_cipher_decrypt(handle: *anyopaque, ciphertext: [*]const u32, plaintext: [*]u32) void {
    const c: *core.PolerCipher = @ptrCast(@alignCast(handle));
    var ct: [core.BLOCK_WORDS]u32 = undefined;
    for (&ct, 0..) |*w, i| w.* = ciphertext[i];
    var pt: [core.BLOCK_WORDS]u32 = undefined;
    c.decryptBlock(&ct, &pt);
    for (&pt, 0..) |w, i| plaintext[i] = w;
}

/// Освободить контекст шифра.
export fn poler_cipher_free(handle: *anyopaque) void {
    const c: *core.PolerCipher = @ptrCast(@alignCast(handle));
    alloc.destroy(c);
}

// ── Счётчиковый DRBG (P0-F3) ────────────────────────────────────────────────

/// Создать генератор из 256-битного сида (8 слов).
export fn poler_drbg_new(seed: [*]const u32) ?*anyopaque {
    var s: [core.KEY_WORDS]u32 = undefined;
    for (&s, 0..) |*w, i| w.* = seed[i];
    const g = alloc.create(core.PolerDrbg) catch return null;
    g.* = core.PolerDrbg.init(&s);
    return g;
}

/// Следующее 32-битное слово потока.
export fn poler_drbg_next(handle: *anyopaque) u32 {
    const g: *core.PolerDrbg = @ptrCast(@alignCast(handle));
    return g.next();
}

/// Равномерное слово в [0, max) — безсмещённый rejection sampling.
export fn poler_drbg_next_range(handle: *anyopaque, max: u32) u32 {
    const g: *core.PolerDrbg = @ptrCast(@alignCast(handle));
    return g.nextRange(max);
}

/// Освободить генератор.
export fn poler_drbg_free(handle: *anyopaque) void {
    const g: *core.PolerDrbg = @ptrCast(@alignCast(handle));
    alloc.destroy(g);
}

// ── POLER-CBC (P0-F2) ───────────────────────────────────────────────────────

/// Зашифровать каскадом с IV: pt_words кратно 4; iv — 4 слова;
/// ct-буфер длины pt_words. Сцепление блоков — ECB-утечка исключена.
export fn poler_cbc_encrypt(handle: *anyopaque, iv: [*]const u32, pt: [*]const u32, pt_words: usize, ct: [*]u32) void {
    const c: *core.PolerCipher = @ptrCast(@alignCast(handle));
    std.debug.assert(pt_words % core.BLOCK_WORDS == 0);
    var ivb: [core.BLOCK_WORDS]u32 = undefined;
    for (&ivb, 0..) |*w, i| w.* = iv[i];
    const cbc = core.PolerCbc{ .cipher = c.* };
    cbc.encrypt(&ivb, pt[0..pt_words], ct[0..pt_words]);
}

/// Расшифровать каскад (см. poler_cbc_encrypt).
export fn poler_cbc_decrypt(handle: *anyopaque, iv: [*]const u32, ct: [*]const u32, ct_words: usize, pt: [*]u32) void {
    const c: *core.PolerCipher = @ptrCast(@alignCast(handle));
    std.debug.assert(ct_words % core.BLOCK_WORDS == 0);
    var ivb: [core.BLOCK_WORDS]u32 = undefined;
    for (&ivb, 0..) |*w, i| w.* = iv[i];
    const cbc = core.PolerCbc{ .cipher = c.* };
    cbc.decrypt(&ivb, ct[0..ct_words], pt[0..ct_words]);
}

// ── Parity-тесты: C-ABI слой ≡ прямые вызовы ядра ──────────────────────────

test "C-ABI parity: скаляры" {
    const phi_cases = [_]u32{ 0, 1, 0x9E3779B9, 0xDEADBEEF, 0xFFFFFFFF };
    for (phi_cases) |x| {
        try std.testing.expectEqual(core.phi(x), poler_phi(x));
    }
    const mix_cases = [_][3]u32{
        .{ 42, 17, 1 },    .{ 42, 17, 0 },     .{ 0, 0, 0 },
        .{ 0xDEADBEEF, 0xCAFEBABE, 0x9E3779B9 },
    };
    for (mix_cases) |c| {
        try std.testing.expectEqual(core.pndMix(c[0], c[1], c[2]), poler_pnd_mix(c[0], c[1], c[2]));
    }
    try std.testing.expectEqual(
        core.lhcaStep(0x12345678, .{ .rule_mask = 0xACACACAC }),
        poler_lhca_step(0x12345678, 0xACACACAC),
    );
    try std.testing.expectEqual(core.mixColumnsPnd(0xFFFFFFFF), poler_mix_columns(0xFFFFFFFF));
}

test "C-ABI parity: шифр через handle ≡ прямой вызов" {
    const key = [core.KEY_WORDS]u32{ 0x01234567, 0x89ABCDEF, 0xFEDCBA98, 0x76543210, 0x11111111, 0x22222222, 0x33333333, 0x44444444 };
    const epsilon: u32 = 0xDEAD;
    const pt = [core.BLOCK_WORDS]u32{ 0x01234567, 0x89ABCDEF, 0xFEDCBA98, 0x76543210 };

    const handle = poler_cipher_new(&key, epsilon) orelse return error.OutOfMemory;
    defer poler_cipher_free(handle);

    var ct: [core.BLOCK_WORDS]u32 = undefined;
    poler_cipher_encrypt(handle, &pt, &ct);

    var direct = core.PolerCipher.init(&key, epsilon);
    var ct2: [core.BLOCK_WORDS]u32 = undefined;
    direct.encryptBlock(@constCast(&pt), &ct2);
    try std.testing.expectEqualSlices(u32, &ct2, &ct);

    var back: [core.BLOCK_WORDS]u32 = undefined;
    poler_cipher_decrypt(handle, &ct, &back);
    try std.testing.expectEqualSlices(u32, &pt, &back);
}

test "C-ABI parity: DRBG через handle ≡ прямой вызов" {
    const seed = [core.KEY_WORDS]u32{ 1, 2, 3, 4, 5, 6, 7, 8 };
    const handle = poler_drbg_new(&seed) orelse return error.OutOfMemory;
    defer poler_drbg_free(handle);

    var direct = core.PolerDrbg.init(&seed);
    var i: usize = 0;
    while (i < 128) : (i += 1) {
        try std.testing.expectEqual(direct.next(), poler_drbg_next(handle));
    }
    for (0..256) |_| {
        try std.testing.expect(poler_drbg_next_range(handle, 7) < 7);
    }
}

test "C-ABI parity: CBC через handle" {
    const key = [core.KEY_WORDS]u32{ 1, 2, 3, 4, 5, 6, 7, 8 };
    const handle = poler_cipher_new(&key, 0xDEAD) orelse return error.OutOfMemory;
    defer poler_cipher_free(handle);

    const iv = [core.BLOCK_WORDS]u32{ 0x11111111, 0x22222222, 0x33333333, 0x44444444 };
    const pt = [_]u32{ 0xA, 0xB, 0xC, 0xD, 0xA, 0xB, 0xC, 0xD };
    var ct: [8]u32 = undefined;
    poler_cbc_encrypt(handle, &iv, &pt, pt.len, &ct);
    // ECB-утечка исключена: одинаковые pt-блоки → разные ct-блоки
    try std.testing.expect(ct[0] != ct[4] or ct[1] != ct[5] or ct[2] != ct[6] or ct[3] != ct[7]);

    var back: [8]u32 = undefined;
    poler_cbc_decrypt(handle, &iv, &ct, ct.len, &back);
    try std.testing.expectEqualSlices(u32, &pt, &back);
}

// ============================================================================
// E1/v0.31.0: полер-исполнитель команд — компиляция в библиотеку.
// Сам C-ABI и тесты живут в poler_exec.zig (единый файл: raw-syscall слой,
// ребёнок-бутстрап, ppoll-цикл, кольцевой захват). Здесь только реэкспорт,
// чтобы zig build включил модуль в libpoler_core.a.
// ============================================================================
comptime {
    _ = @import("poler_exec.zig");
}
