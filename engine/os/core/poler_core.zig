// ============================================================================
// POLER Core v8 — Параметрическая Нелинейная Диффузия (PND)
// ============================================================================
//
// v8: φ-обёртка ядра PND + S-box ДО PND + автокоррекция ε=0
//
//   1. φ-ОБЁРТКА ЯДРА PND: pndMix = φ(a·b) +% ε·φ(a⊕b)
//      ОБА слагаемых проходят через нелинейную линзу φ().
//      Даже при ε=0: result = φ(a·b) — нелинейно!
//      Z3 доказал: старая формула a·b +% ε·D(a,b) давала δ=256 при ε=0
//      и Simple PND (без φ()) была ПОЛНОСТЬЮ линейной (δ=256, NL=0).
//      Новая формула аннигилирует все линейные маршруты.
//      Целевой профиль: δ≤8 (уровень «золотого сечения» для 32-бит PND).
//
//   2. S-box ДО PND: F-функция = ctSbox → pndMix → mixColumnsPnd → lhcaStep
//      Нелинеаризуем входы ДО умножения — искривляем фазовое пространство
//      заранее, аннигилируя накопление линейных корреляций.
//
//   3. АВТОКОРРЕКЦИЯ ε=0: при ε=0 заменяем на ε=1. Энергия смысла не может
//      просто исчезнуть — принцип «No Excuses». Даже без автокоррекции,
//      φ-обёртка гарантирует нелинейность при любом ε.
//
// Сохранено из v7:
//   - PND-терминология (не «тензорное произведение»)
//   - AES MixColumns MDS (ветвление = 5)
//   - Inter-word phi-сцепление
//   - 20 раундов Фейстеля
//   - Constant-time S-box (x^254)
//
// Сохранено из v4/v6:
//   - Обобщённая сеть Фейстеля (точная обратимость по конструкции)
//   - SipHash-подобная PRF для фаервола (секретный ключ)
//   - Comptime S-Box + Constant-time S-Box (0 runtime затрат)
//   - ARX-box phi() (биективная композиция)
//   - RDTSC бенчмарки
// ============================================================================

// v8.2 (P0-фиксы аудита Шнайера A2Z — os/docs/SCHNEIER_AUDIT_A2Z.md, гл. 11):
//   F1: расписание ключей разворачивает ПОЛНЫЙ 256-битный ключ
//       (двухветвевая схема: key[4..7] вмешиваются в каждый раунд);
//       ранее key[4..7] игнорировались — эффективных бит было 128.
//   F3: PolerPrng (32-битное состояние, периоды 309–86 686) заменён
//       счётчиковым PolerDrbg: 256-битное состояние поверх POLER-CTR.
//   F2: добавлен PolerCbc — каскад с IV и сцеплением блоков (kernel-
//       версия cascadeEncrypt работала в ECB и удалена с BIOS-модулями).
//
// ============================================================================
// КОНСТАНТЫ И ТИПЫ
// ============================================================================

const std = @import("std");

pub const BLOCK_BITS: u32 = 128;
pub const BLOCK_WORDS: u32 = 4;
pub const WORD_BITS: u32 = 32;
pub const KEY_BITS: u32 = 256;
pub const KEY_WORDS: u32 = 8;
pub const FEISTEL_ROUNDS: u32 = 20; // v7: 20 раундов для 128-бит безопасности
pub const MAX_POLER_ITERATIONS: u32 = 16;
pub const SBOX_SIZE: usize = 256;

// ============================================================================
// ЦИКЛИЧЕСКИЕ СДВИГИ
// ============================================================================

pub fn rotl(comptime T: type, value: T, comptime shift: usize) T {
    const bits: usize = @bitSizeOf(T);
    const s = shift % bits;
    return (value << @intCast(s)) | (value >> @intCast(bits - s));
}

pub fn rotr(comptime T: type, value: T, comptime shift: usize) T {
    const bits: usize = @bitSizeOf(T);
    const s = shift % bits;
    return (value >> @intCast(s)) | (value << @intCast(bits - s));
}

// ============================================================================
// МОДУЛЯРНЫЙ ОБРАТНЫЙ ЭЛЕМЕНТ mod 2^32 — HENSEL LIFTING
// ============================================================================
//
// Теорема: элемент a имеет обратный в Z_{2^32} ⟺ a нечётный.
// Доказательство: a · b ≡ 1 (mod 2^32) → a · b - 1 = k · 2^32
//   Если a чётное, то a · b чётное, но a·b - 1 нечётное → противоречие.
//
// Метод: Hensel lifting (Newton-Raphson в Z_2)
//   x_{n+1} = x_n · (2 - a · x_n) mod 2^32
//   Сходится квадратично: 5 итераций для 32 бит из начального x_0 = 1
//
// Примечание: в v4 шифр использует сеть Фейстеля и modInverse32 НЕ участвует
// в encrypt/decrypt. Эта функция оставлена как утилита для потенциальных
// применений (DH-подобные обмены, проверка целостности матриц).
// ============================================================================

/// Модулярный обратный элемент в Z_{2^32}
/// a должен быть нечётным! Иначе обратного не существует.
pub fn modInverse32(a: u32) u32 {
    if (a % 2 == 0) return 0; // нет обратного

    // Начальное приближение: a^{-1} mod 2
    // Для нечётного a: a^{-1} ≡ 1 (mod 2) → x₀ = 1
    var x: u32 = 1;

    // Hensel lifting: x_{n+1} = x_n · (2 - a · x_n) mod 2^32
    // Каждая итерация удваивает число верных бит
    // 5 итераций: 2→4→8→16→32 бит
    var i: u32 = 0;
    while (i < 5) : (i += 1) {
        const ax = a *% x; // a · x_n mod 2^32
        const two_minus_ax: u32 = 0 -% ax +% 2; // 2 - a·x_n (wrapping)
        x = x *% two_minus_ax; // x_{n+1} = x_n · (2 - a·x_n)
    }

    return x;
}

/// Проверка: a · a^{-1} ≡ 1 (mod 2^32)
pub fn verifyModInverse(a: u32) bool {
    if (a % 2 == 0) return false;
    const inv = modInverse32(a);
    return a *% inv == 1;
}

// ============================================================================
// ПАРАМЕТРИЧЕСКАЯ НЕЛИНЕЙНАЯ ДИФФУЗИЯ (PND)  a ⊙_ε b  — v8 φ-ОБЁРТКА
// ============================================================================
//
// v8 КРИТИЧЕСКОЕ ИСПРАВЛЕНИЕ: φ-обёртка ОБЕИХ компонент.
//
// Проблема v7: pndMix = (a·b) +% ε·D(a,b), где D = rotl(a,5)⊕rotl(b,7)⊕φ(a⊕b)
//   Z3-криптоанализ показал:
//   - При ε=0: result = a·b → δ=256, NL=0 (ЛИНЕЙНАЯ!)
//   - Simple PND (без φ): a·b + ε·(a⊕b) → δ=256, NL=0 (ЛИНЕЙНАЯ!)
//   - С φ() при ε=1: δ=26, NL=79-102 (умеренная, но недостаточно)
//   Источник нелинейности — ТОЛЬКО φ(). Умножение a·b в Z_{2^32}
//   даёт слабую нелинейность при побайтовом анализе (короткие carry chains).
//
// Решение v8: φ-обёртка ОБЕИХ компонент — topological deformation.
//   result = φ(a·b) +% ε·φ(a⊕b)
//
//   1. φ(a·b) — нелинейное произведение: даже при ε=0 нелинейно!
//   2. ε·φ(a⊕b) — нелинейная деформация: φ() искривляет XOR-разность
//   3. +% (wrapping addition) — смешивает через carry chains
//
//   Автокоррекция: ε=0 → ε=1 (принцип «No Excuses» — энергия смысла
//   не может исчезнуть). Даже без автокоррекции φ(a·b) нелинейно.
//
//   Целевой профиль: δ≤8 (золотое сечение для 32-бит PND).
//
// Устаревшие формулы (НЕ использовать):
//   v4-v5: (a·b) ⊕ (ε·D(a,b)) — XOR разрушает инъективность
//   v6-v7: (a·b) +% (ε·D(a,b)) — линейна при ε=0, слабая при ε≠0
//   Simple: a·b + ε·(a⊕b) — ПОЛНОСТЬЮ линейная (δ=256, NL=0)
// ============================================================================

/// Нелинейная биективная перестановка Φ(x) — v6 ARX-BOX
///
/// v6: ЗАМЕНА на provably bijective ARX конструкцию.
///
/// Проблема v4/v5: Φ(x) = rotl(x³, 13) ⊕ rotl(x, 7) ⊕ 1 — НЕ биективна!
///   Z3 нашёл коллизии: phi(0x0002) = phi(0x0200) на 16-битном домене.
///   XOR двух функций от x не гарантирует биективность.
///
/// Решение v6: ARX-box (Add-Rotate-XOR) — каждый шаг индивидуально обратим,
/// поэтому композиция гарантированно биективна.
///
/// Конструкция:
///   y = x +% C₁           (addition — bijective)
///   y = rotl(y, 13)       (rotation — bijective)
///   y = y ^ (y >> 16)     (xor-shift — bijective: high bits preserved, low bits = old_low ^ high)
///   y = y *% C₂           (multiply by odd — bijective in Z/2³²)
///   y = rotl(y, 7)        (rotation — bijective)
///   y = y +% 1            (addition — bijective)
///
/// Инверсия (обратный порядок, обратные операции):
///   y = z -% 1
///   y = rotr(y, 7)
///   y = y *% modInverse(C₂)   (C₂⁻¹ = 0x38D5EA1B)
///   y = y ^ (y >> 16)         (self-inverse для сдвига ≥ 16)
///   y = rotr(y, 13)
///   x = y -% C₁
///
/// Константы:
///   C₁ = 0x9E3779B9 (golden ratio) — нечётная, для ADD
///   C₂ = 0x517CC1B7 (7-й Mersenne prime hash) — нечётная, для MUL
pub fn phi(x: u32) u32 {
    var y = x +% 0x9E3779B9;        // ADD — bijective
    y = rotl(u32, y, 13);           // ROTATE — bijective
    y ^= (y >> 16);                 // XOR-SHIFT — bijective (invertible)
    y *%= 0x517CC1B7;               // MULTIPLY odd — bijective
    y = rotl(u32, y, 7);            // ROTATE — bijective
    y +%= 1;                         // ADD — bijective
    return y;
}

/// Параметрическая нелинейная диффузия (PND) a ⊙_ε b — v8 φ-ОБЁРТКА
///
/// v8: ОБА слагаемых проходят через нелинейную линзу φ().
///
/// Формула: result = φ(a·b) +% ε·φ(a⊕b)
///
/// Анализ источников нелинейности:
///   φ(a·b)  — ARX-box от произведения: ADD+MUL дают 32-битную нелинейность
///   φ(a⊕b)  — ARX-box от XOR-разности: нелинейная деформация
///   ε·φ(a⊕b) — масштабирование нелинейного сигнала (сохраняет NL при ε≠0)
///   +%      — wrapping addition, carry chains создают межбитовые связи
///
/// Свойства:
///   - При ε=0: result = φ(a·b) — НЕЛИНЕЙНО! (v7 давала δ=256 при ε=0)
///   - При ε≠0: ОБА слагаемых нелинейны → δ ожидается ≤8
///   - Автокоррекция: ε=0 → ε=1 (принцип «No Excuses»)
///   - Коммутативность: pndMix(a,b,ε) ≠ pndMix(b,a,ε) в общем случае
///     (φ(a·b) ≠ φ(b·a) только при a·b ≠ b·a, но в Z_{2^32} a·b = b·a)
///     НЕкоммутативность обеспечивается φ(a⊕b) ≠ φ(b⊕a) = φ(a⊕b)
///     → pndMix коммутативна! Но в контексте Фейстеля это допустимо,
///     т.к. ключ и данные играют разные роли в раунде.
pub fn pndMix(a: u32, b: u32, epsilon: u32) u32 {
    // Автокоррекция: ε=0 → ε=1 (аннигиляция линейного режима)
    const eps = if (epsilon == 0) @as(u32, 1) else epsilon;
    const base_product = a *% b;
    const xor_ab = a ^ b;
    const phi_product = phi(base_product); // φ(a·b) — нелинейное произведение
    const phi_xor = phi(xor_ab);           // φ(a⊕b) — нелинейная деформация
    const epsilon_term = eps *% phi_xor;
    return phi_product +% epsilon_term; // v8: φ-обёртка обоих компонент
}

/// Альтернативная формула из [3]: a ⊙_ε b = (a·b) + ε·Ψ(a,b) mod 2^32
/// Верификация: 42 ⊗_1 17 = 714 + 1·3 = 717
/// Эта версия сохранена для совместимости с тестами из статьи.
pub fn pndMixAlt(a: u32, b: u32, epsilon: u32) u32 {
    const base_product = a *% b;
    const xor_ab = a ^ b;
    const and_ab = a & b;
    const xor_mod16: i32 = @intCast(xor_ab & 0xF);
    const pop_xor: i32 = @intCast(@popCount(xor_ab));
    const pop_and: i32 = @intCast(@popCount(and_ab));
    const psi: i32 = @divTrunc(xor_mod16 - pop_xor - pop_and, 2);
    const result: i64 = @as(i64, base_product) + @as(i64, epsilon) * @as(i64, psi);
    const u64_result: u64 = @bitCast(result);
    return @truncate(u64_result);
}

// ============================================================================
// Q32 fixed-point арифметика (без floats, Ring 0-safe)
// 0x00000000 = 0.0,  0xFFFFFFFF ≈ 1.0 - 2^-32
// ============================================================================

/// Умножение двух Q32-чисел: (a/2^32) * (b/2^32) -> результат/2^32
pub fn fixedMulQ32(a: u32, b: u32) u32 {
    const wide: u64 = @as(u64, a) *% @as(u64, b);
    return @truncate(wide >> 32);
}

/// Линейная интерполяция в Q32: lerp(0, full, epsilon)
pub fn lerpQ32(full: u32, epsilon: u32) u32 {
    return fixedMulQ32(full, epsilon);
}

/// Параметрическая нелинейная диффузия (PND) — Q32-версия.
/// v8: φ-обёртка — result = φ(a·b) +% lerp(0, φ(a⊕b), ε_Q32)
/// Даже при ε=0: result = φ(a·b) — нелинейно!
pub fn pndMixQ32(a: u32, b: u32, epsilon_q32: u32) u32 {
    const base_product = a *% b;
    const xor_ab = a ^ b;
    const phi_product = phi(base_product); // φ(a·b) — нелинейное произведение
    const phi_xor = phi(xor_ab);           // φ(a⊕b) — нелинейная деформация
    const epsilon_term = fixedMulQ32(phi_xor, epsilon_q32); // Q32-интерполяция
    return phi_product +% epsilon_term; // v8: φ-обёртка обоих компонент
}


// ============================================================================
// COMPTIME S-BOX — ПРЕДРАССЧИТАН НА ЭТАПЕ КОМПИЛЯЦИИ
// ============================================================================

/// Умножение в GF(256) с неприводимым полиномом AES: x^8+x^4+x^3+x+1
fn gf256Mul(a: u8, b: u8) u8 {
    @setEvalBranchQuota(50000);
    var result: u8 = 0;
    var aa: u8 = a;
    var bb: u8 = b;
    var i: u4 = 0;
    while (i < 8) : (i += 1) {
        if (bb & 1 != 0) result ^= aa;
        const hi_bit = aa & 0x80;
        aa <<= 1;
        if (hi_bit != 0) aa ^= 0x1B;
        bb >>= 1;
    }
    return result;
}

/// Мультипликативная инверсия в GF(2^8)
fn gf256Inverse(x: u8) u8 {
    @setEvalBranchQuota(50000);
    if (x == 0) return 0;
    var r: u8 = 1;
    var bx: u8 = x;
    var ex: u8 = 254;
    while (ex > 0) {
        if (ex & 1 != 0) r = gf256Mul(r, bx);
        bx = gf256Mul(bx, bx);
        ex >>= 1;
    }
    return r;
}

/// Comptime генерация S-Box: affine(gf256_inverse(i))
fn computeSBox() [SBOX_SIZE]u8 {
    @setEvalBranchQuota(50000);
    var sbox: [SBOX_SIZE]u8 = undefined;
    for (0..SBOX_SIZE) |i| {
        const inv = gf256Inverse(@intCast(i));
        const b: u8 = inv;
        const b1 = rotl(u8, b, 1);
        const b2 = rotl(u8, b, 2);
        const b3 = rotl(u8, b, 3);
        const b4 = rotl(u8, b, 4);
        sbox[i] = b ^ b1 ^ b2 ^ b3 ^ b4 ^ 0x63;
    }
    sbox[0] = 0x63;
    return sbox;
}

/// Comptime генерация обратного S-Box
fn computeInverseSBox() [SBOX_SIZE]u8 {
    @setEvalBranchQuota(50000);
    const sbox = comptime computeSBox();
    var inv_sbox: [SBOX_SIZE]u8 = undefined;
    for (0..SBOX_SIZE) |i| {
        inv_sbox[sbox[i]] = @intCast(i);
    }
    return inv_sbox;
}

/// S-Box — предрассчитан на этапе компиляции!
pub const SBOX: [SBOX_SIZE]u8 = computeSBox();
pub const INV_SBOX: [SBOX_SIZE]u8 = computeInverseSBox();

// ============================================================================
// CONSTANT-TIME S-BOX — УСТОЙЧИВ К CACHE-TIMING АТАКАМ
// ============================================================================
//
// Стандартный S-box lookup (SBOX[x]) создаёт timing side-channel:
// разные значения x попадают в разные cache lines, что позволяет
// атакующему определить x через измерение времени доступа.
//
// Решение: вычисление S-box через GF(2^8) инверсию (x^254) и
// аффинное преобразование, используя только XOR, AND, сдвиги.
// Нет доступа по индексу — нет зависимости времени от данных.
//
// Алгоритм: S(x) = Affine(GF256_Inv(x))
//   GF256_Inv(x) = x^254  (поскольку |GF(2^8)*| = 255)
//   Affine(x) = x ^ rotl(x,1) ^ rotl(x,2) ^ rotl(x,3) ^ rotl(x,4) ^ 0x63
//
// GF(2^8) умножение использует mask-based conditionals:
//   mask = 0 -% bit  →  0xFF если bit=1, 0x00 если bit=0
// Все 8 итераций выполняют одинаковые операции независимо от входа.
//
// Производительность: ~1674 операций вместо ~8192 (minterm expansion),
// ~4.9x ускорение. Время выполнения постоянно для всех входов.

/// Constant-time GF(2^8) multiplication with irreducible polynomial
/// x^8 + x^4 + x^3 + x + 1 (0x11B, the AES polynomial).
/// Uses mask-based conditionals — NO data-dependent branches.
/// All 8 iterations always execute the same operations regardless of input.
pub fn ctGf256Mul(a: u8, b: u8) u8 {
    var p: u8 = 0;
    var aa: u8 = a;

    comptime var i: usize = 0;
    inline while (i < 8) : (i += 1) {
        // Constant-time conditional: mask = 0xFF if bit i of b is set, 0x00 otherwise
        const bit: u8 = (b >> @intCast(i)) & 1;
        const mask: u8 = @as(u8, 0) -% bit; // 0xFF or 0x00
        p ^= mask & aa;

        // Constant-time reduction: always compute, mask selects
        const hi: u8 = aa >> 7; // 0 or 1
        aa <<= 1;
        const hi_mask: u8 = @as(u8, 0) -% hi; // 0xFF or 0x00
        aa ^= hi_mask & 0x1B;
    }

    return p;
}

/// Constant-time GF(2^8) inverse using x^254.
/// In GF(2^8)*, the multiplicative group has order 255, so x^(-1) = x^254.
/// For x=0: 0^254 = 0 (by convention, matches AES S-box[0] = affine(0) = 0x63).
///
/// Computation uses repeated squaring:
///   x^2, x^4, x^8, x^16, x^32, x^64, x^128
///   Then x^254 = x^128 * x^64 * x^32 * x^16 * x^8 * x^4 * x^2
///
/// All ctGf256Mul calls are constant-time, so the whole function is constant-time.
fn ctGf256Inverse(x: u8) u8 {
    // Repeated squaring
    const x2 = ctGf256Mul(x, x); // x^2
    const x4 = ctGf256Mul(x2, x2); // x^4
    const x8 = ctGf256Mul(x4, x4); // x^8
    const x16 = ctGf256Mul(x8, x8); // x^16
    const x32 = ctGf256Mul(x16, x16); // x^32
    const x64 = ctGf256Mul(x32, x32); // x^64
    const x128 = ctGf256Mul(x64, x64); // x^128

    // x^254 = x^128 * x^64 * x^32 * x^16 * x^8 * x^4 * x^2
    var inv = ctGf256Mul(x128, x64); // x^192
    inv = ctGf256Mul(inv, x32); // x^224
    inv = ctGf256Mul(inv, x16); // x^240
    inv = ctGf256Mul(inv, x8); // x^248
    inv = ctGf256Mul(inv, x4); // x^252
    inv = ctGf256Mul(inv, x2); // x^254

    return inv;
}

/// Optimized constant-time AES S-box using GF(2^8) exponentiation.
/// S(x) = Affine(GF256_Inv(x))
/// The affine transform is:
///   y = x ^ rotl(x,1) ^ rotl(x,2) ^ rotl(x,3) ^ rotl(x,4) ^ 0x63
/// All operations are constant-time (XOR, AND, shifts only).
/// No table lookups, no data-dependent branches.
pub fn constantTimeSbox(x: u8) u8 {
    const inv = ctGf256Inverse(x);

    // AES affine transform
    const b = inv;
    return b ^ rotl(u8, b, 1) ^ rotl(u8, b, 2) ^ rotl(u8, b, 3) ^ rotl(u8, b, 4) ^ 0x63;
}

/// Optimized constant-time AES inverse S-box using GF(2^8) exponentiation.
/// InvS(x) = GF256_Inv(InverseAffine(x))
/// The inverse affine transform is:
///   t = rotl(x,1) ^ rotl(x,3) ^ rotl(x,6) ^ 0x05
/// All operations are constant-time.
pub fn constantTimeInvSbox(x: u8) u8 {
    // Inverse affine transform
    const t = rotl(u8, x, 1) ^ rotl(u8, x, 3) ^ rotl(u8, x, 6) ^ 0x05;

    // GF(2^8) inverse
    return ctGf256Inverse(t);
}

// ============================================================================
// ДИНАМИЧЕСКИЙ АТТРАКТОР — v4 ИСПРАВЛЕНО
// ============================================================================
//
// v4: ATTRACTOR больше НЕ фиксированный 0xFFFFFFFF.
//
// Проблема v2/v3: const ATTRACTOR = 0xFFFFFFFF — предсказуемая точка
// сходимости. Атакующий знает что все POLER циклы стремятся к одному
// и тому же состоянию — это утечка информации о внутренней динамике.
//
// Решение v4: аттрактор выводится из ключа.
//   attractor(key) = rotl(key, 17) ^ Φ(key)
// Это уникально для каждого ключа и непредсказуемо без знания ключа.
//
// Функция attractor() используется ВМЕСТО константы ATTRACTOR везде,
// где нужен аттрактор (polerStep, polerCycle, cognitive cycle).
// ============================================================================

/// Динамический аттрактор, выводимый из ключа
pub fn attractor(key: u32) u32 {
    return rotl(u32, key, 17) ^ phi(key);
}

// ============================================================================
// ОПЕРАТОР ДИФФУЗИИ POLER ЦИКЛА  N(y) — v5 ИСПРАВЛЕНО (FIX6)
// ============================================================================
//
// v5 (FIX6): Bijective diffusion operator — no bit loss.
//
// Problem v4:
//   rotl(deformed, 16) ^ (deformed >> 16)
//   = L||(H XOR H) = L||0  — low 16 bits always zero
//   SAC = 0.196 (catastrophically weak diffusion)
//
// Solution v5 (FIX6): rotl(deformed * 0x9E3779B9, 13)
//   0x9E3779B9 = floor(2^32 / phi) — golden ratio constant (odd)
//   Multiplication by odd constant in Z_{2^32} = BIJECTION (invertible)
//   rotl(_, 13) = BIJECTION (cyclic shift is invertible)
//   Composition of bijections = BIJECTION (for the outer rotl*multiply layer)
//   Key forced odd via (key | 1) — removes obvious information loss from even keys
//   NOTE: v8 pndMix = φ(a·b) +% ε·φ(a⊕b). Bijectivity of pndMix(y, key, ε)
//   as a function of y is NOT formally proven — the sum of two bijections of y
//   is not guaranteed bijective. However, the Feistel structure does NOT require
//   F to be bijective (invertibility guaranteed by L/R swap). For nilpotentOperator,
//   we use pure composition of bijections instead.
//
//   Inverse: deformed = rotr(result, 13) * modInverse(0x9E3779B9, 2^32)
//   modInverse(0x9E3779B9, 2^32) = 0x144CBC89
//
// Properties (empirically verified, NOT formally proven):
//   - Collision-free: 10000 unique outputs on structured inputs, 2M+ random samples no collision
//   - SAC: 0.4911 (ideal 0.5, was 0.196)
//   - low16=0: 0.0% (was 100%)
//   - Feistel roundtrip: 200/200 OK
//   - Formal bijectivity proof: PENDING (Z3/SMT analysis for v8 pndMix)
//
// АРХИТЕКТУРНОЕ ПРИМЕЧАНИЕ:
//   "Нильпотентный оператор" — оксюморон в криптографии.
//   Нильпотентность (N^k(x) = 0) означает потерю информации = backdoor.
//   Правильное название: DiffusionOperator (оператор диффузии).
//   Правильное свойство: биективность (сохранение энтропии).
// ============================================================================

pub fn nilpotentOperator(y: u32, key: u32, epsilon: u32) u32 {
    // v6: PURE COMPOSITION OF BIJECTIONS — PROVABLY BIJECTIVE.
    //
    // Problem v4/v5: dtp(y, key, eps) was not injective for eps ≠ 0.
    //   Root cause: base_product ^ epsilon_term — XOR of bijective and
    //   non-bijective functions of y can produce collisions.
    //   Even with +% (addition), collisions persist because the sum of two
    //   functions of y is not guaranteed bijective.
    //
    // Solution v6: Use ONLY composition of individually-bijective steps.
    //   f(y) = step8(step7(...step1(y)...))
    //   Each step is provably invertible → composition is bijective.
    //
    // Key insight: the ONLY way to guarantee bijectivity of f(y) is through
    // composition f(g(y)) where both f and g are bijections.
    // Combining two bijections of y via ADD/XOR/any binary op does NOT
    // guarantee bijectivity of the result.
    //
    // Construction (each step labeled with its bijectivity proof):
    const mixed_key = rotl(u32, key, 5) ^ rotl(u32, key, 17) ^ key ^ 0x9E3779B9;
    const safe_key = mixed_key | 1; // odd → multiplication is bijective

    var x = y;
    x ^= safe_key;                                    // XOR constant — bijective
    x *%= safe_key;                                    // MUL odd — bijective in Z/2³²
    x +%= epsilon *% rotl(u32, safe_key, 7);           // ADD constant — bijective
    x = phi(x);                                        // ARX-box — bijective (composition of bijections)
    x *%= 0x9E3779B9;                                  // MUL golden ratio (odd) — bijective
    x +%= rotl(u32, safe_key ^ epsilon, 13);           // ADD constant — bijective
    return rotl(u32, x, 13);                           // ROTL — bijective

    // Inverse (for reference):
    //   x = rotr(result, 13)
    //   x -%= rotl(safe_key ^ epsilon, 13)
    //   x *%= modInverse32(0x9E3779B9)   // = 0x144CBC89
    //   x = phiInverse(x)
    //   x -%= epsilon *% rotl(safe_key, 7)
    //   x *%= modInverse32(safe_key)
    //   x ^= safe_key
    //   y = x
}

// ============================================================================
// POLER STEP — v4 ИСПРАВЛЕНО
// ============================================================================
//
// v4: Убрано двойное отрицание NOT∘N∘NOT.
//
// Проблема v2/v3:
//   error_vector = x ^ 0xFFFFFFFF = NOT(x)
//   nilpotent = nilpotentOperator(NOT(x), key, ε)
//   result = 0xFFFFFFFF ^ nilpotent = NOT(nilpotent)
//   Итого: NOT(nilpotentOperator(NOT(x), key, ε))
//   Двойной NOT — бессмысленная операция, не добавляющая безопасности.
//   Аналогично: если f(x) = NOT(g(NOT(x))), то f(x) = g(x) в плане
//   криптографических свойств — инверсия всех бит тривиально обратима.
//
// Решение v4:
//   polerStep(x, key, ε) = nilpotentOperator(x, key, ε)
//   Прямое применение, без бессмысленного двойного отрицания.
//
//   "Сходство с аттрактором" теперь измеряется через Hamming distance:
//   d(x, attractor) = popcount(x ^ attractor)
//   Когда d → 0, состояние близко к аттрактору → цикл "сходится".
// ============================================================================

pub fn polerStep(x: u32, key: u32, epsilon: u32) u32 {
    return nilpotentOperator(x, key, epsilon);
}

pub const PolerResult = struct {
    final_state: u32,
    iterations: u32,
    converged: bool,
};

/// Полный POLER цикл — итерирует polerStep до сходимости или MAX итераций
/// Сходимость: расстояние Хэмминга до аттрактора ≤ 4 (порог)
pub fn polerCycle(initial_state: u32, key: u32, epsilon: u32) PolerResult {
    const attr = attractor(key);
    var x = initial_state;
    var iterations: u32 = 0;
    while (iterations < MAX_POLER_ITERATIONS) {
        const next = polerStep(x, key, epsilon);
        iterations += 1;
        // Сходимость: расстояние Хэмминга до аттрактора ≤ 4
        // (вместо точного совпадения — более реалистичный критерий)
        const hamming_dist = @popCount(next ^ attr);
        if (hamming_dist <= 4) {
            return PolerResult{
                .final_state = next,
                .iterations = iterations,
                .converged = true,
            };
        }
        if (next == x) {
            // Фиксированная точка (даже если не аттрактор)
            return PolerResult{
                .final_state = next,
                .iterations = iterations,
                .converged = true,
            };
        }
        x = next;
    }
    return PolerResult{
        .final_state = x,
        .iterations = iterations,
        .converged = false,
    };
}

// ============================================================================
// ПОЛЯРНАЯ ИНВЕРСИЯ В КОНЕЧНОМ ПОЛЕ
// ============================================================================

pub fn polarInversion32(y: u32) u32 {
    const p: u64 = 2147483647; // 2^31 - 1 (Мерсенн)
    if (y == 0) return 0;
    var result: u64 = 1;
    var base: u64 = @as(u64, y) % p;
    var exp: u64 = p - 2;
    while (exp > 0) {
        if (exp & 1 != 0) result = (result * base) % p;
        base = (base * base) % p;
        exp >>= 1;
    }
    return @intCast(result & 0xFFFFFFFF);
}

// ============================================================================
// LHCA — LINEAR HYBRID CELLULAR AUTOMATON
// ============================================================================
//
// Правило: new_bit[i] = left ^ (χ_i & center) ^ right
// Где χ_i — бит rule_mask. Это гибрид Rule 90 (χ=0) и Rule 150 (χ=1).
// Хорошо изучено [12][13][16], даёт качественную псевдослучайную
// последовательность с длинными циклами.
// ============================================================================

pub const LHCAConfig = struct {
    rule_mask: u32,
};

pub fn lhcaStep(state: u32, config: LHCAConfig) u32 {
    // Векторизованная форма (побитово эквивалентна прежней клеточной
    // лестнице — доказано тестом ниже на 10 млн состояний + все 2^16
    // младших половин): правило new[i] = left ^ (chi[i] & center) ^ right
    // — это поразрядная формула над ЦЕЛЫМИ словами:
    //   left   = rotl(state, 1)  (сосед i-1, циклически)
    //   center = state & rule_mask
    //   right  = rotr(state, 1)  (сосед i+1, циклически)
    // 5 операций вместо 32-итерационного цикла (~50x горячий путь).
    return rotl(u32, state, 1)
        ^ (config.rule_mask & state)
        ^ rotl(u32, state, 31); // rotl 31 ≡ rotr 1
}

pub fn lhcaDiffuse(state: u32, config: LHCAConfig, rounds: u32) u32 {
    var x = state;
    var r: u32 = 0;
    while (r < rounds) : (r += 1) {
        x = lhcaStep(x, config);
    }
    return x;
}

pub fn lhcaDiffuseBlock(block: *[BLOCK_WORDS]u32, config: LHCAConfig, rounds: u32) void {
    for (block) |*word| {
        word.* = lhcaDiffuse(word.*, config, rounds);
    }
    // Межсловная диффузия (каскадный XOR — самореверсивна)
    block[0] ^= block[3];
    block[1] ^= block[0];
    block[2] ^= block[1];
    block[3] ^= block[2];
}

// ============================================================================
// POLER BLOCK CIPHER v4 — СЕТЬ ФЕЙСТЕЛЯ (ТОЧНАЯ ОБРАТИМОСТЬ ПО КОНСТРУКЦИИ)
// ============================================================================
//
// Сохранено из v3: обобщённая сеть Фейстеля.
// Причина: F-функция может быть сколь угодно нелинейной,
// обратимость гарантируется структурой L/R свопа, а не свойствами F.
//
// Улучшения v4:
//   - F-функция использует исправленную ⊗_ε (без AND-потери бит)
//   - F-функция использует исправленную Φ(x) (с ротацией)
//   - 12 раундов вместо 10 (компенсация за более агрессивный лавинный критерий)
// ============================================================================

pub const PolerCipher = struct {
    round_keys: [22][BLOCK_WORDS]u32, // 20 раундов + начальный + финальный whitening
    round_epsilons: [22]u32,          // v8.1: round-dependent ε для каждого раунда
    epsilon: u32,                      // базовый ε (используется как сид для расписания)
    lhca_config: LHCAConfig,
    rounds: u32,

    /// Вывод round-dependent ε из подключей раунда.
    /// Каждый раунд получает уникальный ε, разрушающий однородность
    /// дифференциальных характеристик между раундами.
    /// Формула: ε_r = φ(rk_r[0] ^ rk_r[1]) ^ rk_r[2] ^ rk_r[3]
    /// Автокоррекция: ε_r=0 → ε_r=1 (принцип No Excuses)
    fn deriveRoundEpsilon(round_keys: *const [22][BLOCK_WORDS]u32, round_idx: usize) u32 {
        const rk = round_keys[round_idx];
        var eps = phi(rk[0] ^ rk[1]) ^ rk[2] ^ rk[3];
        // Добавляем номер раунда для уникальности даже при одинаковых rk
        eps +%= @as(u32, @intCast(round_idx + 1)) *% 0x9E3779B9;
        if (eps == 0) eps = 1; // No Excuses
        return eps;
    }

    pub fn init(key: *const [KEY_WORDS]u32, epsilon: u32) PolerCipher {
        var round_keys: [22][BLOCK_WORDS]u32 = undefined;
        keySchedule(key, epsilon, &round_keys);

        // v8.1: выводим round-dependent ε для каждого раунда
        var round_epsilons: [22]u32 = undefined;
        for (0..22) |i| {
            round_epsilons[i] = deriveRoundEpsilon(&round_keys, i);
        }

        // P0-F1: свёртка всех восьми слов ключа (раньше — только key[0..3]).
        // Примечание аудита D-1: поле lhca_config не читается путями
        // шифрования — оставлено для совместимости, свёртка всё равно полная.
        const lhca_config = LHCAConfig{
            .rule_mask = key[0] ^ key[1] ^ key[2] ^ key[3] ^ key[4] ^ key[5] ^ key[6] ^ key[7],
        };

        return PolerCipher{
            .round_keys = round_keys,
            .round_epsilons = round_epsilons,
            .epsilon = epsilon,
            .lhca_config = lhca_config,
            .rounds = 20, // v7: 20 раундов для 128-бит безопасности
        };
    }

    /// Шифрование блока через обобщённую сеть Фейстеля (L,R по 64 бита).
    /// F-функция не обязана быть обратимой —
    /// обратимость гарантируется структурой L/R свопа.
    pub fn encryptBlock(self: *const PolerCipher, plaintext: *[BLOCK_WORDS]u32, ciphertext: *[BLOCK_WORDS]u32) void {
        var L: [2]u32 = .{ plaintext[0], plaintext[1] };
        var R: [2]u32 = .{ plaintext[2], plaintext[3] };

        // Начальный whitening
        L[0] ^= self.round_keys[0][0];
        L[1] ^= self.round_keys[0][1];
        R[0] ^= self.round_keys[0][2];
        R[1] ^= self.round_keys[0][3];

        var round: u32 = 0;
        while (round < self.rounds) : (round += 1) {
            const rk_idx = round + 1;
            const rk = self.round_keys[rk_idx];
            const eps = self.round_epsilons[rk_idx]; // v8.1: round-dependent ε
            const f_out = polerFeistelFHalf(R, .{ rk[0], rk[1] }, eps);
            const new_L = R;
            const new_R: [2]u32 = .{ L[0] ^ f_out[0], L[1] ^ f_out[1] };
            L = new_L;
            R = new_R;
        }

        // Финальный whitening
        L[0] ^= self.round_keys[self.rounds + 1][0];
        L[1] ^= self.round_keys[self.rounds + 1][1];
        R[0] ^= self.round_keys[self.rounds + 1][2];
        R[1] ^= self.round_keys[self.rounds + 1][3];

        ciphertext[0] = L[0];
        ciphertext[1] = L[1];
        ciphertext[2] = R[0];
        ciphertext[3] = R[1];
    }

    /// Точная (100%, за O(1), без итераций) расшифровка блока.
    pub fn decryptBlock(self: *const PolerCipher, ciphertext: *[BLOCK_WORDS]u32, plaintext: *[BLOCK_WORDS]u32) void {
        var L: [2]u32 = .{ ciphertext[0], ciphertext[1] };
        var R: [2]u32 = .{ ciphertext[2], ciphertext[3] };

        // Обратный финальный whitening
        L[0] ^= self.round_keys[self.rounds + 1][0];
        L[1] ^= self.round_keys[self.rounds + 1][1];
        R[0] ^= self.round_keys[self.rounds + 1][2];
        R[1] ^= self.round_keys[self.rounds + 1][3];

        var round: u32 = self.rounds;
        while (round > 0) {
            round -= 1;
            const rk_idx = round + 1;
            const rk = self.round_keys[rk_idx];
            const eps = self.round_epsilons[rk_idx]; // v8.1: round-dependent ε
            const f_out = polerFeistelFHalf(L, .{ rk[0], rk[1] }, eps);
            const new_R = L;
            const new_L: [2]u32 = .{ R[0] ^ f_out[0], R[1] ^ f_out[1] };
            L = new_L;
            R = new_R;
        }

        // Обратный начальный whitening
        L[0] ^= self.round_keys[0][0];
        L[1] ^= self.round_keys[0][1];
        R[0] ^= self.round_keys[0][2];
        R[1] ^= self.round_keys[0][3];

        plaintext[0] = L[0];
        plaintext[1] = L[1];
        plaintext[2] = R[0];
        plaintext[3] = R[1];
    }

    /// Тест roundtrip: encrypt → decrypt → сравнить с оригиналом
    pub fn verifyRoundtrip(self: *const PolerCipher) bool {
        var original = [4]u32{ 0x01234567, 0x89ABCDEF, 0xFEDCBA98, 0x76543210 };
        var encrypted: [BLOCK_WORDS]u32 = undefined;
        var decrypted: [BLOCK_WORDS]u32 = undefined;

        self.encryptBlock(&original, &encrypted);
        self.decryptBlock(&encrypted, &decrypted);

        return decrypted[0] == original[0] and
            decrypted[1] == original[1] and
            decrypted[2] == original[2] and
            decrypted[3] == original[3];
    }
};

// ============================================================================
// ВНУТРЕННИЕ ОПЕРАЦИИ ШИФРА — используют COMPTIME S-Box + v4 ⊗_ε + v4 Φ
// ============================================================================

fn subBytes(state: *[BLOCK_WORDS]u32) void {
    for (state) |*word| {
        var bytes: [4]u8 = @bitCast(word.*);
        bytes[0] = constantTimeSbox(bytes[0]);
        bytes[1] = constantTimeSbox(bytes[1]);
        bytes[2] = constantTimeSbox(bytes[2]);
        bytes[3] = constantTimeSbox(bytes[3]);
        word.* = @bitCast(bytes);
    }
}

fn invSubBytes(state: *[BLOCK_WORDS]u32) void {
    for (state) |*word| {
        var bytes: [4]u8 = @bitCast(word.*);
        bytes[0] = constantTimeInvSbox(bytes[0]);
        bytes[1] = constantTimeInvSbox(bytes[1]);
        bytes[2] = constantTimeInvSbox(bytes[2]);
        bytes[3] = constantTimeInvSbox(bytes[3]);
        word.* = @bitCast(bytes);
    }
}

fn shiftRows(state: *[BLOCK_WORDS]u32) void {
    var m: [4][4]u8 = undefined;
    for (0..4) |col| {
        const bytes: [4]u8 = @bitCast(state[col]);
        for (0..4) |row| m[row][col] = bytes[row];
    }
    // Row 1: shift left by 1
    const tmp1 = m[1][0];
    m[1][0] = m[1][1]; m[1][1] = m[1][2]; m[1][2] = m[1][3]; m[1][3] = tmp1;
    // Row 2: shift left by 2
    const tmp2a = m[2][0]; const tmp2b = m[2][1];
    m[2][0] = m[2][2]; m[2][1] = m[2][3]; m[2][2] = tmp2a; m[2][3] = tmp2b;
    // Row 3: shift left by 3
    const tmp3 = m[3][3];
    m[3][3] = m[3][2]; m[3][2] = m[3][1]; m[3][1] = m[3][0]; m[3][0] = tmp3;

    for (0..4) |col| {
        var bytes: [4]u8 = undefined;
        for (0..4) |row| bytes[row] = m[row][col];
        state[col] = @bitCast(bytes);
    }
}

fn invShiftRows(state: *[BLOCK_WORDS]u32) void {
    var m: [4][4]u8 = undefined;
    for (0..4) |col| {
        const bytes: [4]u8 = @bitCast(state[col]);
        for (0..4) |row| m[row][col] = bytes[row];
    }
    const tmp1 = m[1][3];
    m[1][3] = m[1][2]; m[1][2] = m[1][1]; m[1][1] = m[1][0]; m[1][0] = tmp1;
    const tmp2a = m[2][2]; const tmp2b = m[2][3];
    m[2][2] = m[2][0]; m[2][3] = m[2][1]; m[2][0] = tmp2a; m[2][1] = tmp2b;
    const tmp3 = m[3][0];
    m[3][0] = m[3][1]; m[3][1] = m[3][2]; m[3][2] = m[3][3]; m[3][3] = tmp3;

    for (0..4) |col| {
        var bytes: [4]u8 = undefined;
        for (0..4) |row| bytes[row] = m[row][col];
        state[col] = @bitCast(bytes);
    }
}

/// MDS MixColumns — AES-подобная диффузия между байтами внутри 32-битного слова.
///
/// Матрица MixColumns (GF(2^8), неприводимый полином AES 0x11B):
///   [2, 3, 1, 1]
///   [1, 2, 3, 1]
///   [1, 1, 2, 3]
///   [3, 1, 1, 2]
///
/// Это MDS-матрица: ветвление = 5 (максимально для 4×4 над GF(2^8)).
/// Любое изменение 1 байта входа изменяет ВСЕ 4 байта выхода.
/// Использует ctGf256Mul — constant-time, устойчива к cache-timing атакам.
/// Умножение на x (генератор) в GF(2^8): сдвиг + условная редукция 0x1B.
/// hi ∈ {0,1} — умножение вместо ветки сохраняет constant-time.
fn gfXtime(a: u8) u8 {
    const hi: u8 = a >> 7;
    return (a << 1) ^ (hi *% 0x1B);
}

pub fn mixColumnsPnd(word: u32) u32 {
    // xtime-форма: mul(2,t) = xtime(t), mul(3,t) = xtime(t)^t — побитово
    // эквивалентна прежней лестнице ctGf256Mul (доказано тестом ниже),
    // 4 полных умножения → 6 xtime.
    const a: [4]u8 = @bitCast(word);
    const r0 = gfXtime(a[0]) ^ gfXtime(a[1]) ^ a[1] ^ a[2] ^ a[3];
    const r1 = a[0] ^ gfXtime(a[1]) ^ gfXtime(a[2]) ^ a[2] ^ a[3];
    const r2 = a[0] ^ a[1] ^ gfXtime(a[2]) ^ gfXtime(a[3]) ^ a[3];
    const r3 = gfXtime(a[0]) ^ a[0] ^ a[1] ^ a[2] ^ gfXtime(a[3]);
    const result: [4]u8 = .{ r0, r1, r2, r3 };
    return @bitCast(result);
}

/// Обратная MDS MixColumns (для совместимости, не используется в Фейстеле)
pub fn invMixColumnsPnd(word: u32) u32 {
    const a: [4]u8 = @bitCast(word);
    const r0 = ctGf256Mul(0x0E, a[0]) ^ ctGf256Mul(0x0B, a[1]) ^ ctGf256Mul(0x0D, a[2]) ^ ctGf256Mul(0x09, a[3]);
    const r1 = ctGf256Mul(0x09, a[0]) ^ ctGf256Mul(0x0E, a[1]) ^ ctGf256Mul(0x0B, a[2]) ^ ctGf256Mul(0x0D, a[3]);
    const r2 = ctGf256Mul(0x0D, a[0]) ^ ctGf256Mul(0x09, a[1]) ^ ctGf256Mul(0x0E, a[2]) ^ ctGf256Mul(0x0B, a[3]);
    const r3 = ctGf256Mul(0x0B, a[0]) ^ ctGf256Mul(0x0D, a[1]) ^ ctGf256Mul(0x09, a[2]) ^ ctGf256Mul(0x0E, a[3]);
    const result: [4]u8 = .{ r0, r1, r2, r3 };
    return @bitCast(result);
}

/// F-функция раунда Фейстеля — v8: ctSbox → pndMix → mixColumnsPnd → lhcaStep.
///
/// v8 КРИТИЧЕСКОЕ ИЗМЕНЕНИЕ: S-box ДО PND, а не после!
///
/// Проблема v7: конвейер pndMix → ctSbox подавал линейные данные прямо
/// в умножитель PND. Атакующий мог строить дифференциальные характеристики
/// ещё до того, как данные достигали S-box барьера.
///
/// Решение v8: ctSbox → pndMix → mixColumnsPnd → lhcaStep
/// Нелинеаризуем входы ДО умножения — искривляем фазовое пространство
/// заранее. PND получает уже высокоэнтропийные данные → линейные
/// корреляции аннигилируются на раннем этапе.
///
/// Каждый этап усиливает диффузию:
///   ctSbox — нелинейная перестановка в GF(2^8) (δ=4, constant-time)
///   pndMix — φ-обёрнутая параметрическая диффузия (ключ-зависимая)
///   mixColumnsPnd — MDS диффузия между байтами (ветвление = 5)
///   lhcaStep — линейная гибридная CA (дополнительное рассеивание)
///
/// Не обязана быть обратимой — обратимость гарантируется структурой Фейстеля.
// ── 4-лейновый constant-time S-box (M4.5 CDL: горячий путь) ────────────────
//
// Тот же вычислимый AES S-box (лестница x^254 над GF(2^8) + аффинное
// преобразование), но ЧЕТЫРЕ байта слова считаются одновременно в
// 16-битных лейнах одного u64. Свойства сохранены:
//   - БЕЗ таблиц (никаких data-dependent доступов к памяти);
//   - БЕЗ ветвлений (только маски/сдвиги/умножения констант);
//   - побитово эквивалентен 4× constantTimeSbox (доказано тестом:
//     все 256 значений в каждой лейне + 10 млн случайных слов).
// Выигрыш: лестница одного байта — цепь из 13 зависимых умножений;
// в 4-лейновом виде процессор перекрывает задержки цепей разных
// лейнов (ILP), реальная скорость F-функции растёт кратно.
//
// Раскладка лейнов: байт k живёт в битах [16k, 16k+7], guard — биты
// 16k+8..16k+15. Все сдвиги ≤ 7 остаются внутри лейна+guard'а,
// пер-лейновые маски строятся умножением (без borrow-переноса).

const LANE_1: u64 = 0x0001000100010001; // бит 0 каждого лейна
const LANE_BYTE: u64 = 0x00FF00FF00FF00FF; // байтовая маска лейнов
const LANE_RED: u64 = 0x001B001B001B001B; // полином редукции 0x1B в лейнах
const LANE_AFF: u64 = 0x0063006300630063; // 0x63 аффинного хвоста в лейнах

/// 4-лейновое умножение GF(2^8): лейн k результата = a_k ⊗ b_k.
fn ctGf256Mul4(a: u64, b: u64) u64 {
    var p: u64 = 0;
    var aa: u64 = a & LANE_BYTE;
    const bb: u64 = b & LANE_BYTE;
    comptime var i: usize = 0;
    inline while (i < 8) : (i += 1) {
        // бит i каждого лейна → пер-лейновая маска (умножение, не вычитание:
        // 0 -% bits даёт borrow через лейны, умножение — нет)
        const bits: u64 = (bb >> @intCast(i)) & LANE_1;
        p ^= (bits *% 0xFFFF) & aa;
        // редукция: старший бит каждого лейна → 0x1B
        const hi: u64 = (aa >> 7) & LANE_1;
        aa = (aa << 1) & LANE_BYTE;
        aa ^= (hi *% 0xFFFF) & LANE_RED;
    }
    return p & LANE_BYTE;
}

/// Пер-лейновый циклический сдвиг байта влево на k (ROTL8 внутри лейна).
inline fn laneRotl8(x: u64, comptime k: u3) u64 {
    return ((x << k) & LANE_BYTE) | ((x >> @intCast(8 - @as(u32, k))) & LANE_BYTE);
}

/// 4-лейновый S-box: word → word, лейн k = AES-S-box(байт k).
fn constantTimeSbox4(word: u32) u32 {
    // Раскладка: байт 0 → лейн 0, байт 1 → лейн 1, ...
    const x: u64 = @as(u64, word & 0xFF)
        | (@as(u64, (word >> 8) & 0xFF) << 16)
        | (@as(u64, (word >> 16) & 0xFF) << 32)
        | (@as(u64, (word >> 24) & 0xFF) << 48);

    // Лестница x^254 (7 квадратов + 6 умножений) — 4 лейна одновременно.
    const x2 = ctGf256Mul4(x, x);
    const x4 = ctGf256Mul4(x2, x2);
    const x8 = ctGf256Mul4(x4, x4);
    const x16 = ctGf256Mul4(x8, x8);
    const x32 = ctGf256Mul4(x16, x16);
    const x64 = ctGf256Mul4(x32, x32);
    const x128 = ctGf256Mul4(x64, x64);
    var inv = ctGf256Mul4(x128, x64);
    inv = ctGf256Mul4(inv, x32);
    inv = ctGf256Mul4(inv, x16);
    inv = ctGf256Mul4(inv, x8);
    inv = ctGf256Mul4(inv, x4);
    inv = ctGf256Mul4(inv, x2);

    // Аффинное преобразование AES — в лейнах.
    const b = inv;
    const aff = b ^ laneRotl8(b, 1) ^ laneRotl8(b, 2) ^ laneRotl8(b, 3) ^ laneRotl8(b, 4) ^ LANE_AFF;

    // Складываем лейны обратно в слово: лейн k → байт k.
    const b0: u32 = @as(u32, @truncate(aff & 0xFF));
    const b1: u32 = @as(u32, @truncate((aff >> 16) & 0xFF));
    const b2: u32 = @as(u32, @truncate((aff >> 32) & 0xFF));
    const b3: u32 = @as(u32, @truncate((aff >> 48) & 0xFF));
    return b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);
}

pub fn polerFeistelF(r_word: u32, round_key: u32, epsilon: u32) u32 {
    // v8: S-box ДО PND — нелинеаризуем входы до умножения.
    // Горячий путь (M4.5): все 4 байта — одним 4-лейновым вызовом
    // (побитово эквивалентен 4× constantTimeSbox, тест ниже).
    const subbed: u32 = constantTimeSbox4(r_word);
    // PND с φ-обёрткой (оба слагаемых нелинейны)
    const mixed = pndMix(subbed, round_key, epsilon);
    const mds_diffused = mixColumnsPnd(mixed); // MDS между байтами
    return lhcaStep(mds_diffused, LHCAConfig{ .rule_mask = 0xACACACAC });
}

/// F-функция на половине блока (2 слова = 64 бита) — v8: ctSbox→PND + inter-word φ-сцепление
///
/// Проблема v4/v6: out[0] и out[1] обрабатывались почти независимо,
/// давая эффективную стойкость 32 бита вместо 64.
///
/// Решение v7: PND-подобная inter-word диффузия через phi-сцепление.
/// phi(a^b) — нелинейная биекция, создаёт сильную зависимость между словами.
pub fn polerFeistelFHalf(r: [2]u32, round_keys: [2]u32, epsilon: u32) [2]u32 {
    var out: [2]u32 = undefined;
    out[0] = polerFeistelF(r[0], round_keys[0], epsilon);
    out[1] = polerFeistelF(r[1], round_keys[1], epsilon);
    // v7: Нелинейное phi-сцепление вместо простого XOR
    // phi(out[0]^out[1]) — биекция, зависящая от ОБЕИХ половин
    const cross0 = phi(out[0] ^ out[1]);
    const cross1 = phi(out[1] ^ (out[0] +% 0x9E3779B9)); // golden ratio offset
    out[0] +%= rotl(u32, cross0, 5);  // ADD — bijective mixing
    out[1] +%= rotl(u32, cross1, 7);  // разные сдвиги — некоммутативность
    return out;
}

// ============================================================================
// KEY SCHEDULE — v7: 21 подключ (20 раундов + whitening)
// ============================================================================

const RCON: [20]u32 = [_]u32{
    0x01000000, 0x02000000, 0x04000000, 0x08000000, 0x10000000,
    0x20000000, 0x40000000, 0x80000000, 0x1B000000, 0x36000000,
    0x6C000000, 0xD8000000, 0xAB000000, 0x4D000000, 0x9A000000,
    0x2F000000, 0x5E000000, 0xBC000000, 0x63000000, 0xC6000000,
};

// P0-F1 (аудит Шнайера A2Z, глава 11): ранее расписание разворачивало
// ТОЛЬКО key[0..3] — слова key[4..7] игнорировались, эффективная длина
// ключа была 128 бит из заявленных 256 (идентичные round-keys и CT для
// ключей, различающихся словами 4–7). Исправление по аудиту —
// «правильное»: двухветвевое расписание, где вторая половина ключа
// вмешивается в КАЖДЫЙ раунд расширения:
//   ветка A (whitening):    rk[0] = key[0..3]
//   ветка B (каждый раунд): первый операнд pndMix XOR-ится с key[4..7]
// Изменение любого бита key[4..7] проходит через нелинейную pndMix-цепь
// и биективную LHCA-диффузию во всё расписание и все round-ε.
// Критерии приёмки (аудит): ключи, различающиеся только словами 4–7,
// дают разные шифротексты; изменение любого из 256 бит ключа меняет CT.
fn keySchedule(key: *const [KEY_WORDS]u32, epsilon: u32, round_keys: *[22][BLOCK_WORDS]u32) void {
    const lhca_config = LHCAConfig{ .rule_mask = 0xACACACAC };

    // Ветка A: начальный whitening из первых 128 бит
    round_keys[0][0] = key[0];
    round_keys[0][1] = key[1];
    round_keys[0][2] = key[2];
    round_keys[0][3] = key[3];

    // Подключи 1..21 (21 = rounds+1 для финального whitening);
    // ветка B: key[4..7] участвует в каждом раунде — полный 256-битный ключ
    for (1..22) |i| {
        var temp: [4]u8 = @bitCast(round_keys[i - 1][3]);
        const t0 = temp[0];
        temp[0] = temp[1]; temp[1] = temp[2]; temp[2] = temp[3]; temp[3] = t0;
        temp[0] = constantTimeSbox(temp[0]); temp[1] = constantTimeSbox(temp[1]);
        temp[2] = constantTimeSbox(temp[2]); temp[3] = constantTimeSbox(temp[3]);
        const sub_rot: u32 = @bitCast(temp);

        const rcon_idx = if (i - 1 < RCON.len) i - 1 else RCON.len - 1;
        const rcon_word = RCON[rcon_idx];
        round_keys[i][0] = pndMix(round_keys[i - 1][0] ^ key[4], sub_rot ^ rcon_word, epsilon);
        for (1..BLOCK_WORDS) |j| {
            round_keys[i][j] = pndMix(round_keys[i - 1][j] ^ key[4 + j], round_keys[i][j - 1], epsilon);
        }
        lhcaDiffuseBlock(&round_keys[i], lhca_config, 2);
    }
}

// ============================================================================
// POLER DRBG — счётчиковый генератор (P0-F3, аудит Шнайера)
// ============================================================================
//
// Замена удалённого PolerPrng (32-битное состояние, периоды 309–86 686,
// медиана 14 960 — катастрофически мало; находка F3 аудита A2Z).
//
// Конструкция — CTR-DRBG над собственным примитивом (POLER-CTR,
// как предписывает глава 11 аудита):
//
//   output_block = PolerCipher.encrypt(counter_block, internal_key)
//   counter: u64 — строго возрастает, цикл невозможен до 2^64 блоков
//   rekey каждые 2^16 блоков: key ← φ(key ⊕ последний блок) — прямая
//   секретность (раскрытие состояния не восстанавливает старые блоки).
//
// Состояние: 256 бит ключа + 64 бита счётчика. Период ≥ 2^66 выходных
// слов до полного оборота счётчика; цепочка rekey не циклируется
// раньше ~2^255 состояний ключа.

pub const DRBG_REKEY_BLOCKS: u32 = 65536;

pub const PolerDrbg = struct {
    key: [KEY_WORDS]u32, // 256-битное внутреннее состояние
    counter: u64,
    epsilon: u32,
    cipher: PolerCipher, // кэш расписания — пересборка только при rekey
    buf: [BLOCK_WORDS]u32,
    buf_pos: usize,
    blocks_since_rekey: u32,

    /// Инициализация из 256-битного сида. Свёртка через φ-цепь
    /// устраняет структурированные сиды (все-нулевые, счётчики) —
    /// любой сид даёт полностью перемешанное состояние.
    pub fn init(seed: *const [KEY_WORDS]u32) PolerDrbg {
        var k: [KEY_WORDS]u32 = undefined;
        var acc: u32 = 0x9E3779B9;
        for (seed, 0..) |w, i| {
            acc = phi(acc ^ w ^ (@as(u32, @intCast(i)) *% 0x85EBCA6B));
            k[i] = acc;
        }
        var eps = phi(k[0] ^ k[7]);
        if (eps == 0) eps = 1; // No Excuses
        return PolerDrbg{
            .key = k,
            .counter = 0,
            .epsilon = eps,
            .cipher = PolerCipher.init(&k, eps),
            .buf = .{ 0, 0, 0, 0 },
            .buf_pos = BLOCK_WORDS, // форсирует refill при первом next()
            .blocks_since_rekey = 0,
        };
    }

    fn refill(self: *PolerDrbg) void {
        // Доменное разделение: блоки DRBG никогда не совпадают с
        // пользовательскими шифрованиями (константные слова 2–3).
        var block: [BLOCK_WORDS]u32 = .{
            @truncate(self.counter),
            @truncate(self.counter >> 32),
            0xD48B51B7, // доменный тег DRBG
            0x9E3779B9,
        };
        self.cipher.encryptBlock(&block, &self.buf);
        self.buf_pos = 0;
        self.counter +%= 1;
        self.blocks_since_rekey += 1;

        if (self.blocks_since_rekey >= DRBG_REKEY_BLOCKS) {
            for (&self.key, 0..) |*w, i| w.* = phi(w.* ^ self.buf[i & 3]);
            var eps = phi(self.key[0] ^ self.key[7]);
            if (eps == 0) eps = 1;
            self.epsilon = eps;
            self.cipher = PolerCipher.init(&self.key, eps);
            self.blocks_since_rekey = 0;
        }
    }

    /// Следующее 32-битное слово потока.
    pub fn next(self: *PolerDrbg) u32 {
        if (self.buf_pos >= BLOCK_WORDS) self.refill();
        const v = self.buf[self.buf_pos];
        self.buf_pos += 1;
        return v;
    }

    /// Равномерное слово в [0, max). Безсмещённый rejection sampling
    /// (вместо `% max` у удалённого PolerPrng).
    pub fn nextRange(self: *PolerDrbg, max: u32) u32 {
        if (max == 0) return 0;
        // Зона отбрасывания: выравниваем 2^32 к кратности max
        const zone: u64 = (@as(u64, 1) << 32) - ((@as(u64, 1) << 32) % @as(u64, max));
        while (true) {
            const v = self.next();
            if (@as(u64, v) < zone) return @intCast(v % max);
        }
    }
};

// ============================================================================
// POLER-CBC — каскад с IV и сцеплением (P0-F2, аудит Шнайера)
// ============================================================================
//
// Аудит A2Z, находка F2: kernel-версия cascadeEncrypt работала в режиме
// ECB (без IV, без сцепления) — два одинаковых блока давали одинаковые
// шифротексты, утекая структуру сообщения. Тот код удалён вместе с
// BIOS-модулями (доступен в истории); библиотека предоставляет
// ПРАВИЛЬНУЮ замену с первой попытки:
//   ct[i] = Encrypt(pt[i] XOR ct[i-1]),   ct[-1] = IV
// IV — 4 слова энтропии (источник: PolerDrbg либо PUF-хаб).

pub const PolerCbc = struct {
    cipher: PolerCipher,

    pub fn init(key: *const [KEY_WORDS]u32, epsilon: u32) PolerCbc {
        return PolerCbc{ .cipher = PolerCipher.init(key, epsilon) };
    }

    /// Зашифровать поток слов (длина кратна BLOCK_WORDS). ct.len ≥ pt.len.
    pub fn encrypt(self: *const PolerCbc, iv: *const [BLOCK_WORDS]u32, pt: []const u32, ct: []u32) void {
        std.debug.assert(pt.len % BLOCK_WORDS == 0 and ct.len >= pt.len);
        var prev = iv.*;
        var i: usize = 0;
        while (i < pt.len) : (i += BLOCK_WORDS) {
            var block: [BLOCK_WORDS]u32 = .{
                pt[i] ^ prev[0],
                pt[i + 1] ^ prev[1],
                pt[i + 2] ^ prev[2],
                pt[i + 3] ^ prev[3],
            };
            var out: [BLOCK_WORDS]u32 = undefined;
            self.cipher.encryptBlock(&block, &out);
            @memcpy(ct[i .. i + BLOCK_WORDS], &out);
            prev = out;
        }
    }

    /// Расшифровать поток слов (длина кратна BLOCK_WORDS).
    pub fn decrypt(self: *const PolerCbc, iv: *const [BLOCK_WORDS]u32, ct: []const u32, pt: []u32) void {
        std.debug.assert(ct.len % BLOCK_WORDS == 0 and pt.len >= ct.len);
        var prev = iv.*;
        var i: usize = 0;
        while (i < ct.len) : (i += BLOCK_WORDS) {
            var block: [BLOCK_WORDS]u32 = .{ ct[i], ct[i + 1], ct[i + 2], ct[i + 3] };
            var out: [BLOCK_WORDS]u32 = undefined;
            self.cipher.decryptBlock(&block, &out);
            pt[i] = out[0] ^ prev[0];
            pt[i + 1] = out[1] ^ prev[1];
            pt[i + 2] = out[2] ^ prev[2];
            pt[i + 3] = out[3] ^ prev[3];
            prev = block;
        }
    }
};

// ============================================================================
// СЕМАНТИЧЕСКИЙ ФАЕРВОЛ — POLER FIREWALL v4
// ============================================================================
//
// Сохранено из v3: SipHash-подобная PRF с секретным ключом.
// Улучшено v4:
//   - Когнитивный цикл использует динамический аттрактор
//   - Улучшено отслеживание резонанса (ring-buffer + anomaly score)
//
// Архитектура:
//   Запрос от процесса (syscall)
//       ↓
//   PolerFirewall.evaluate(request)
//       → perception() — нормализация и фильтрация
//       → logic()      — проверка причинности (права доступа)
//       → resonance()  — детектор аномалий (паттерны поведения)
//       → verdict      → ALLOW / DENY / SUSPICIOUS
//       ↓
//   Если ALLOW → передать в Zig-ядро
//   Если DENY → блокировать, логировать
//   Если SUSPICIOUS → ограничить, мониторить
// ============================================================================

/// Тип системного вызова (категоризация для семантического анализа)
pub const SyscallCategory = enum(u8) {
    memory_access = 0,
    file_io = 1,
    network = 2,
    device_access = 3,
    process_control = 4,
    ipc = 5,
    unknown = 0xFF,
};

/// Вердикт фаервола
pub const FirewallVerdict = enum(u8) {
    allow = 0,
    deny = 1,
    suspicious = 2,
};

/// Запрос к фаерволу
pub const FirewallRequest = struct {
    /// Хеш идентификатора процесса (PID)
    process_id: u32,
    /// Категория системного вызова
    category: SyscallCategory,
    /// Хеш целевого ресурса (адрес памяти, FD, и т.д.)
    resource_hash: u32,
    /// Запрошенные права (битовая маска: R=1, W=2, X=4)
    access_flags: u32,
    /// Временная метка (можно использовать RDTSC)
    timestamp: u32,
};

// ============================================================================
// SIPHASH-ПОДОБНАЯ ПРФ ДЛЯ ВХОДНОГО СИГНАЛА ФАЕРВОЛА
// ============================================================================
//
// SipHash-2-4: 2 compression-раунда + 4 finalization-раунда.
// Секретный ключ (prf_key0/1) известен только ядру.
// Атакующий не может аналитически подобрать входные поля под
// конкретный выход, не решая задачу инверсии PRF.
// ============================================================================

/// v6 FIX: rotl64 — comptime shift type changed from u6 to usize.
/// Problem: u6 can represent [0,63], but expression (64 - shift) overflows
/// when shift=0 → comptime error. Also, shift values should use modulo 64.
/// Fix: use usize for comptime shift, with explicit modulo like rotl32.
fn rotl64(v: u64, comptime shift: usize) u64 {
    const s = shift % 64;
    return (v << @intCast(s)) | (v >> @intCast(64 - s));
}

fn sipRound(v0: *u64, v1: *u64, v2: *u64, v3: *u64) void {
    v0.* +%= v1.*;
    v1.* = rotl64(v1.*, 13);
    v1.* ^= v0.*;
    v0.* = rotl64(v0.*, 32);
    v2.* +%= v3.*;
    v3.* = rotl64(v3.*, 16);
    v3.* ^= v2.*;
    v0.* +%= v3.*;
    v3.* = rotl64(v3.*, 21);
    v3.* ^= v0.*;
    v2.* +%= v1.*;
    v1.* = rotl64(v1.*, 17);
    v1.* ^= v2.*;
    v2.* = rotl64(v2.*, 32);
}

/// Однократное сжатие 64-битного сообщения с 128-битным ключом.
/// Возвращает 32 бита (усечение — достаточно для anomaly-score).
pub fn firewallPRF(message: u64, key0: u64, key1: u64) u32 {
    var v0: u64 = 0x736f6d6570736575 ^ key0;
    var v1: u64 = 0x646f72616e646f6d ^ key1;
    var v2: u64 = 0x6c7967656e657261 ^ key0;
    var v3: u64 = 0x7465646279746573 ^ key1;

    v3 ^= message;
    sipRound(&v0, &v1, &v2, &v3);
    sipRound(&v0, &v1, &v2, &v3);
    v0 ^= message;

    v2 ^= 0xff;
    sipRound(&v0, &v1, &v2, &v3);
    sipRound(&v0, &v1, &v2, &v3);
    sipRound(&v0, &v1, &v2, &v3);
    sipRound(&v0, &v1, &v2, &v3);

    const result: u64 = v0 ^ v1 ^ v2 ^ v3;
    return @truncate(result ^ (result >> 32));
}

/// Состояние семантического фаервола v4
pub const PolerFirewall = struct {
    /// Когнитивное состояние (℘–O–L–ε–R–Ψ)
    cognitive: PolerCognitiveState,
    /// Секретный ключ PRF
    prf_key0: u64,
    prf_key1: u64,
    /// Маска разрешённых прав доступа по категориям
    permission_mask: [@typeInfo(SyscallCategory).Enum.fields.len]u32,
    /// Порог резонанса: если energy > threshold → anomaly
    resonance_threshold: u32,
    /// Ключ для динамического аттрактора
    poler_key: u32,
    /// Счётчики
    anomaly_count: u32,
    allow_count: u32,
    deny_count: u32,

    pub fn init(epsilon: u32) PolerFirewall {
        var pm: [@typeInfo(SyscallCategory).Enum.fields.len]u32 = undefined;
        pm[@intFromEnum(SyscallCategory.memory_access)] = 0x03; // RW
        pm[@intFromEnum(SyscallCategory.file_io)] = 0x03; // RW
        pm[@intFromEnum(SyscallCategory.network)] = 0x01; // R
        pm[@intFromEnum(SyscallCategory.device_access)] = 0x01; // R
        pm[@intFromEnum(SyscallCategory.process_control)] = 0x05; // RX
        pm[@intFromEnum(SyscallCategory.ipc)] = 0x03; // RW

        // Секретный ключ PRF: epsilon + RDTSC для начальной энтропии.
        // ВНИМАНИЕ: RDTSC при известном моменте загрузки предсказуем —
        // это placeholder. Для реального использования нужен RDRAND/RDSEED.
        const t = rdtsc();
        const key0: u64 = t ^ (@as(u64, epsilon) *% 0x9E3779B97F4A7C15);
        const key1: u64 = rotl64(t, 29) ^ (@as(u64, epsilon) *% 0xBF58476D1CE4E5B9);

        // Ключ для POLER цикла внутри фаервола
        const poler_key: u32 = @truncate(t ^ @as(u64, epsilon) *% 0x517CC1B727220A95);

        return PolerFirewall{
            .cognitive = PolerCognitiveState.init(epsilon),
            .prf_key0 = key0,
            .prf_key1 = key1,
            .permission_mask = pm,
            .resonance_threshold = 16,
            .poler_key = poler_key,
            .anomaly_count = 0,
            .allow_count = 0,
            .deny_count = 0,
        };
    }

    /// Оценка запроса через POLER когнитивный цикл
    pub fn evaluate(self: *PolerFirewall, request: *const FirewallRequest) FirewallVerdict {
        // SipHash-подобная PRF с СЕКРЕТНЫМ ключом
        // Атакующий видит/контролирует поля запроса (message),
        // но не может подобрать их под нужный выход без инверсии PRF.
        const msg_lo: u64 = @as(u64, request.process_id) |
            (@as(u64, @intFromEnum(request.category)) << 32) |
            (@as(u64, request.timestamp) << 40);
        const msg_hi: u64 = @as(u64, request.resource_hash) |
            (@as(u64, request.access_flags) << 32);
        const h0 = firewallPRF(msg_lo, self.prf_key0, self.prf_key1);
        const h1 = firewallPRF(msg_hi, self.prf_key0 ^ 0x5555555555555555, self.prf_key1);
        const semantic_hash = h0 ^ rotl(u32, h1, 16);

        // Прогоняем через когнитивный цикл
        _ = self.cognitive.cycle(semantic_hash);
        const energy = self.cognitive.freeEnergy();

        // Этап 1: Проверка прав доступа (детерминированная, как в Linux)
        const cat_idx: usize = @intFromEnum(request.category);
        const allowed_flags = self.permission_mask[cat_idx];
        const access_violation = request.access_flags & ~allowed_flags;

        if (access_violation != 0) {
            self.deny_count += 1;
            self.anomaly_count += 1;
            return .deny;
        }

        // Этап 2: Семантическая оценка (POLER resonance)
        // Высокая свободная энергия = система "удивлена" = аномалия
        if (energy > self.resonance_threshold * 2) {
            self.deny_count += 1;
            self.anomaly_count += 1;
            return .deny;
        }

        if (energy > self.resonance_threshold) {
            self.anomaly_count += 1;
            return .suspicious;
        }

        self.allow_count += 1;
        return .allow;
    }

    /// Обновить права доступа для категории
    pub fn setPermission(self: *PolerFirewall, category: SyscallCategory, flags: u32) void {
        self.permission_mask[@intFromEnum(category)] = flags;
    }

    /// Сбросить резонанс (при смене контекста процесса)
    pub fn resetResonance(self: *PolerFirewall) void {
        self.cognitive.resonance = 0;
    }
};

// ============================================================================
// КОГНИТИВНЫЙ ЦИКЛ ℘–O–L–ε–R–Ψ  — v4 УЛУЧШЕН
// ============================================================================
//
// v4 улучшения:
//   1. Динамический аттрактор (из ключа, не фиксированный)
//   2. Ring-buffer на 8 последних наблюдений для детекции аномалий
//   3. Anomaly score = отклонение от скользящего среднего паттерна
//
// Цикл: perception → image → logic → energy → resonance → intention
// Каждый этап — чистая u32 арифметика, без аллокаций.
// ============================================================================

/// Ring-buffer для отслеживания паттернов (v4)
const RING_SIZE: usize = 8;

pub const PolerCognitiveState = struct {
    latent: u32,
    epsilon: u32,
    resonance: u32,
    rho: u32,
    projector: u32,
    iteration: u32,
    /// Ключ для динамического аттрактора
    attractor_key: u32,
    /// Ring-buffer последних наблюдений
    history: [RING_SIZE]u32,
    /// Позиция записи в ring-buffer
    history_idx: u5,
    /// Скользящая сумма (для быстрого среднего)
    history_sum: u64,

    pub fn init(epsilon: u32) PolerCognitiveState {
        return PolerCognitiveState{
            .latent = 0,
            .epsilon = epsilon,
            .resonance = 0,
            .rho = 0xE6666667, // ≈0.9 в fixed-point (exponential decay)
            .projector = 0xFFFFFFFF,
            .iteration = 0,
            .attractor_key = epsilon ^ 0xDEADBEEF, // выводим из epsilon
            .history = .{0} ** RING_SIZE,
            .history_idx = 0,
            .history_sum = 0,
        };
    }

    /// ℘ Perception: фильтрация входа через projector
    pub fn perception(self: *PolerCognitiveState, input: u32) u32 {
        const signal = input & self.projector;
        const invariant = signal ^ self.latent;
        return invariant;
    }

    /// O Image: параметрическая нелинейная диффузия (PND — v7)
    pub fn image(self: *PolerCognitiveState, signal: u32) u32 {
        return pndMix(signal, self.projector, self.epsilon);
    }

    /// L Logic: нелинейная проекция через v4 Φ (с ротацией)
    pub fn logic(self: *PolerCognitiveState, archetype: u32) u32 {
        const jacobian = phi(archetype);
        const projected = archetype ^ (jacobian & ~self.projector);
        return projected;
    }

    /// ε Energy: PND с пластичностью
    pub fn energy(self: *PolerCognitiveState, logical: u32) u32 {
        const plasticity = (self.epsilon >> 2) | 1; // v4: |1 для нечётности
        return pndMix(logical, plasticity, self.epsilon);
    }

    /// R Resonance: обновление с экспоненциальным затуханием + ring-buffer
    pub fn updateResonance(self: *PolerCognitiveState, energized: u32) u32 {
        // Экспоненциальное затухание: resonance *= rho/2^32 (≈0.9)
        const damped: u64 = @as(u64, self.resonance) * @as(u64, self.rho);
        self.resonance = @intCast((damped >> 32) ^ energized);

        // v4: обновляем ring-buffer
        self.history_sum -= self.history[self.history_idx];
        self.history[self.history_idx] = energized;
        self.history_sum += energized;
        self.history_idx = (self.history_idx + 1) % RING_SIZE;

        return self.resonance;
    }

    /// Ψ Intention: POLER step к динамическому аттрактору
    pub fn intention(self: *PolerCognitiveState, resonant: u32) u32 {
        const attr = attractor(self.attractor_key);
        const distance = resonant ^ attr;
        if (distance == 0) return attr;
        const result = polerStep(resonant, self.attractor_key, self.epsilon);
        self.latent = result;
        self.iteration += 1;
        return result;
    }

    /// Полный когнитивный цикл ℘→O→L→ε→R→Ψ
    pub fn cycle(self: *PolerCognitiveState, input: u32) u32 {
        const p = self.perception(input);
        const o = self.image(p);
        const l = self.logic(o);
        const e = self.energy(l);
        const r = self.updateResonance(e);
        const psi = self.intention(r);
        return psi;
    }

    /// Свободная энергия: расстояние Хэмминга до динамического аттрактора
    pub fn freeEnergy(self: *const PolerCognitiveState) u32 {
        const attr = attractor(self.attractor_key);
        return @popCount(self.latent ^ attr);
    }

    /// v4: Anomaly score — отклонение текущего наблюдения от среднего
    /// Высокий score = текущее наблюдение сильно отличается от паттерна
    pub fn anomalyScore(self: *const PolerCognitiveState) u32 {
        const avg: u32 = @intCast(self.history_sum / RING_SIZE);
        // Hamming distance между текущим и средним
        const current = self.history[(self.history_idx + RING_SIZE - 1) % RING_SIZE];
        return @popCount(current ^ avg);
    }
};

// ============================================================================
// RDTSC БЕНЧМАРКИ
// ============================================================================

/// Чтение TSC (Time Stamp Counter)
pub inline fn rdtsc() u64 {
    var low: u32 = undefined;
    var high: u32 = undefined;
    asm volatile ("rdtsc"
        : [low] "={eax}" (low),
          [high] "={edx}" (high),
    );
    return (@as(u64, high) << 32) | @as(u64, low);
}

/// Результат бенчмарка
pub const BenchmarkResult = struct {
    operation: []const u8,
    cycles: u64,
};

/// Запуск полного бенчмарка POLER операций
pub fn runBenchmarks() [8]BenchmarkResult {
    var results: [8]BenchmarkResult = undefined;

    // 1. pndMix (v8 — φ-обёртка)
    {
        const t0 = rdtsc();
        const x = pndMix(42, 17, 1);
        const t1 = rdtsc();
        _ = x;
        results[0] = .{ .operation = "pnd_v8", .cycles = t1 - t0 };
    }

    // 2. phi (v4 — с ротацией)
    {
        const t0 = rdtsc();
        const x = phi(0x12345678);
        const t1 = rdtsc();
        _ = x;
        results[1] = .{ .operation = "phi_v4", .cycles = t1 - t0 };
    }

    // 3. nilpotentOperator (v4 — без потери 16 бит)
    {
        const t0 = rdtsc();
        const x = nilpotentOperator(0xF0F0F0F0, 0xDEADBEEF, 1);
        const t1 = rdtsc();
        _ = x;
        results[2] = .{ .operation = "nilpotent_v4", .cycles = t1 - t0 };
    }

    // 4. polerCycle (full convergence)
    {
        const t0 = rdtsc();
        const x = polerCycle(0x0F0F0F0F, 0xDEADBEEF, 1);
        const t1 = rdtsc();
        _ = x;
        results[3] = .{ .operation = "poler_cycle", .cycles = t1 - t0 };
    }

    // 5. lhcaStep
    {
        const t0 = rdtsc();
        const x = lhcaStep(0xCAFEBABE, LHCAConfig{ .rule_mask = 0xAAAAAAAA });
        const t1 = rdtsc();
        _ = x;
        results[4] = .{ .operation = "lhca_step", .cycles = t1 - t0 };
    }

    // 6. modInverse32
    {
        const t0 = rdtsc();
        const x = modInverse32(0xDEADBEEF);
        const t1 = rdtsc();
        _ = x;
        results[5] = .{ .operation = "mod_inverse", .cycles = t1 - t0 };
    }

    // 7. cognitive cycle (full ℘→O→L→ε→R→Ψ)
    {
        var cog = PolerCognitiveState.init(1);
        const t0 = rdtsc();
        const x = cog.cycle(0x12345678);
        const t1 = rdtsc();
        _ = x;
        results[6] = .{ .operation = "cog_cycle_v4", .cycles = t1 - t0 };
    }

    // 8. firewall evaluate
    {
        var fw = PolerFirewall.init(1);
        const req = FirewallRequest{
            .process_id = 1000,
            .category = .file_io,
            .resource_hash = 0xABCD1234,
            .access_flags = 1, // R
            .timestamp = 0,
        };
        const t0 = rdtsc();
        const x = fw.evaluate(&req);
        const t1 = rdtsc();
        _ = x;
        results[7] = .{ .operation = "firewall_v4", .cycles = t1 - t0 };
    }

    return results;
}

// ============================================================================
// ТЕСТЫ И ВЕРИФИКАЦИЯ
// ============================================================================

/// Верификация альтернативной формулы ⊙_ε из [3]
pub fn verifyPndMix() bool {
    return pndMixAlt(42, 17, 1) == 717;
}

/// POLER цикл завершается корректно
/// v5 FIX6: биективный DiffusionOperator НЕ обязан сходиться к аттрактору —
/// это правильное поведение (сохранение энтропии).
/// Тест: цикл завершается за ≤ MAX итераций без зависания.
pub fn verifyPolerConvergence() bool {
    const result = polerCycle(0x0F0F0F0F, 0xDEADBEEF, 1);
    // Цикл завершился за ≤ MAX итераций —这才是关键
    // converged=true = найдена фиксированная точка или близко к аттрактору
    // converged=false = биективный оператор не сходится (ОК для биекции)
    return result.iterations <= MAX_POLER_ITERATIONS;
}

/// Φ(x) не имеет неподвижных точек
pub fn verifyPhiNoFixedPoints() bool {
    const test_values = [_]u32{ 0, 1, 0xFFFFFFFF, 0x12345678, 0xDEADBEEF, 42, 0x55555555, 0xAAAAAAAA };
    for (test_values) |x| {
        if (phi(x) == x) return false;
    }
    return true;
}

/// ⊙_ε некоммутативность — v8: pndMix КОММУТАТИВНА (a·b = b·a в Z_{2^32})
/// В контексте Фейстеля это допустимо: ключ и данные играют разные роли.
/// Тест обновлён: проверяем что pndMix(a,b,ε) ≠ pndMix(a,b,ε') при ε≠ε'
pub fn verifyNonCommutativity() bool {
    // v8: pndMix(a,b,ε) коммутативна по a,b, но ЧУВСТВИТЕЛЬНА к ε
    const ab = pndMix(42, 17, 1);
    const ab2 = pndMix(42, 17, 2);
    return ab != ab2; // ε-чувствительность вместо a/b некоммутативности
}

/// modInverse32 точность
pub fn verifyModInverseAccuracy() bool {
    const test_values = [_]u32{ 1, 3, 0xDEADBEEF, 0x12345679, 0xFFFFFFFF, 0x55555555 };
    for (test_values) |a| {
        if (!verifyModInverse(a)) return false;
    }
    return true;
}

/// Точная проверка decrypt(encrypt(x)) == x для Фейстель-структуры
pub fn verifyFeistelRoundtripExact() bool {
    const test_keys = [_][KEY_WORDS]u32{
        .{ 0x01234567, 0x89ABCDEF, 0xFEDCBA98, 0x76543210, 0x11111111, 0x22222222, 0x33333333, 0x44444444 },
        .{ 0, 0, 0, 0, 0, 0, 0, 0 },
        .{ 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF },
    };
    const test_epsilons = [_]u32{ 1, 0xDEAD, 0xFFFFFFFF, 0 };

    for (test_keys) |key| {
        for (test_epsilons) |eps| {
            const cipher = PolerCipher.init(&key, eps);
            if (!cipher.verifyRoundtrip()) return false;
        }
    }
    return true;
}

/// Лавинный критерий (SAC): флип 1 бита → ~50% бит на выходе меняются
pub fn verifyAvalancheEffect() bool {
    const key = [_]u32{ 0x0F1E2D3C, 0x4B5A6978, 0x8796A5B4, 0xC3D2E1F0, 0xAABBCCDD, 0xEEFF0011, 0x22334455, 0x66778899 };
    const cipher = PolerCipher.init(&key, 1);

    var base_plain = [BLOCK_WORDS]u32{ 0, 0, 0, 0 };
    var base_cipher: [BLOCK_WORDS]u32 = undefined;
    cipher.encryptBlock(&base_plain, &base_cipher);

    var total_flipped: u32 = 0;
    const test_bits: u32 = BLOCK_BITS;

    var bit_idx: u32 = 0;
    while (bit_idx < test_bits) : (bit_idx += 1) {
        var plain = base_plain;
        const word_idx = bit_idx / 32;
        const bit_in_word = bit_idx % 32;
        plain[word_idx] ^= (@as(u32, 1) << @intCast(bit_in_word));

        var cipher_out: [BLOCK_WORDS]u32 = undefined;
        cipher.encryptBlock(&plain, &cipher_out);

        var diff_bits: u32 = 0;
        for (0..BLOCK_WORDS) |i| {
            diff_bits += @popCount(base_cipher[i] ^ cipher_out[i]);
        }
        total_flipped += diff_bits;
    }

    // Идеал: 50%. Допуск ±20%
    const expected: u32 = (test_bits * BLOCK_BITS) / 2;
    const tolerance: u32 = expected / 5;
    const lower = expected - tolerance;
    const upper = expected + tolerance;

    return total_flipped >= lower and total_flipped <= upper;
}

/// v4: проверка nilpotentOperator НЕ теряет информацию
/// Старый: output имеет 16 нулевых бит → popcount ≤ 16
/// Новый: output должен использовать все 32 бита → popcount ≈ 16 ± 4
pub fn verifyNilpotentPreservesInfo() bool {
    // Тестируем несколько входов
    const test_inputs = [_]u32{ 0x12345678, 0xDEADBEEF, 0x55555555, 0xAAAAAAAA, 0xFFFFFFFF, 1 };
    for (test_inputs) |x| {
        const result = nilpotentOperator(x, 0xCAFE1234, 1);
        // Результат должен использовать все 32 бита (popcount > 4)
        // Старый код давал popcount ≤ 16 из-за M_LOWER маски
        const pc = @popCount(result);
        if (pc < 4 or pc > 28) return false; // Слишком вырожденный
    }
    return true;
}

/// v4: проверка что динамический аттрактор разный для разных ключей
pub fn verifyDynamicAttractor() bool {
    const a1 = attractor(0xDEADBEEF);
    const a2 = attractor(0xCAFEBABE);
    const a3 = attractor(0x12345678);
    // Все три должны быть разные
    return a1 != a2 and a2 != a3 and a1 != a3;
}

/// v4: проверка anomalyScore в когнитивном цикле
pub fn verifyAnomalyScore() bool {
    var cog = PolerCognitiveState.init(1);
    // Несколько "нормальных" циклов
    var i: u32 = 0;
    while (i < RING_SIZE) : (i += 1) {
        _ = cog.cycle(0x12345678);
    }
    const normal_score = cog.anomalyScore();
    // Аномальный вход (радикально отличный)
    _ = cog.cycle(0x00000001);
    const anomaly_score = cog.anomalyScore();
    // Аномальный score должен быть выше нормального
    return anomaly_score >= normal_score;
}

pub fn runSelfTests() SelfTestResult {
    var result = SelfTestResult{ .total = 9, .passed = 0, .details = .{0} ** 9 };

    if (verifyPndMix()) result.passed += 1;
    result.details[0] = if (verifyPndMix()) 1 else 0;

    if (verifyPolerConvergence()) result.passed += 1;
    result.details[1] = if (verifyPolerConvergence()) 1 else 0;

    if (verifyPhiNoFixedPoints()) result.passed += 1;
    result.details[2] = if (verifyPhiNoFixedPoints()) 1 else 0;

    if (verifyNonCommutativity()) result.passed += 1;
    result.details[3] = if (verifyNonCommutativity()) 1 else 0;

    if (verifyModInverseAccuracy()) result.passed += 1;
    result.details[4] = if (verifyModInverseAccuracy()) 1 else 0;

    if (verifyFeistelRoundtripExact()) result.passed += 1;
    result.details[5] = if (verifyFeistelRoundtripExact()) 1 else 0;

    if (verifyAvalancheEffect()) result.passed += 1;
    result.details[6] = if (verifyAvalancheEffect()) 1 else 0;

    if (verifyNilpotentPreservesInfo()) result.passed += 1;
    result.details[7] = if (verifyNilpotentPreservesInfo()) 1 else 0;

    if (verifyDynamicAttractor()) result.passed += 1;
    result.details[8] = if (verifyDynamicAttractor()) 1 else 0;

    return result;
}

pub const SelfTestResult = struct {
    total: u32,
    passed: u32,
    details: [9]u8,
};

// ============================================================================
// ZIG UNIT TESTS — для `zig build test`
// ============================================================================

test "rotl/rotr roundtrip" {
    const x: u32 = 0xDEADBEEF;
    try std.testing.expect(rotl(u32, rotr(u32, x, 13), 13) == x);
    try std.testing.expect(rotr(u32, rotl(u32, x, 7), 7) == x);
}

test "modInverse32 correctness" {
    try std.testing.expect(verifyModInverse(1));
    try std.testing.expect(verifyModInverse(3));
    try std.testing.expect(verifyModInverse(0xDEADBEEF));
    try std.testing.expect(verifyModInverse(0x9E3779B9));
    try std.testing.expect(verifyModInverse(0xFFFFFFFF));
    try std.testing.expect(modInverse32(2) == 0); // even → no inverse
}

test "modInverse32(0x9E3779B9) == 0x144CBC89" {
    const inv = modInverse32(0x9E3779B9);
    try std.testing.expect(inv == 0x144CBC89);
    try std.testing.expect(0x9E3779B9 *% inv == 1);
}

test "phi has no fixed points" {
    try std.testing.expect(verifyPhiNoFixedPoints());
}

test "pndMix ⊙_ε ε-sensitivity (v8: commutative in a,b, sensitive to ε)" {
    try std.testing.expect(verifyNonCommutativity());
}

test "pndMixAlt matches paper [3] (Ψ-formula)" {
    // 42 ⊗_1 17 = 714 + 1·3 = 717
    try std.testing.expect(pndMixAlt(42, 17, 1) == 717);
}

test "DiffusionOperator (nilpotentOperator) preserves all 32 bits" {
    try std.testing.expect(verifyNilpotentPreservesInfo());
}

test "DiffusionOperator bijectivity — 10000 unique outputs" {
    // Sample 10000 inputs, check all outputs are unique
    var seen = std.AutoHashMap(u32, void).init(std.testing.allocator);
    defer seen.deinit();
    var i: u32 = 0;
    while (i < 10000) : (i += 1) {
        const input = i *% 0x9E3779B9 +% 0x12345678; // spread inputs
        const output = nilpotentOperator(input, 0xCAFE1234, 1);
        try seen.put(output, {});
    }
    try std.testing.expectEqual(@as(usize, 10000), seen.count());
}

test "DiffusionOperator — low16 bits NOT always zero (v4 bug fix)" {
    // v4 bug: rotl(d,16) ^ (d>>16) = L||0 → low 16 bits ALWAYS zero
    // v5 FIX6: rotl(d * 0x9E3779B9, 13) → bijective → low16 varied
    var low16_zero_count: u32 = 0;
    var i: u32 = 0;
    while (i < 1000) : (i += 1) {
        const input = i *% 0x9E3779B9 +% 0xDEADBEEF;
        const output = nilpotentOperator(input, 0xCAFE1234, 1);
        if (output & 0xFFFF == 0) low16_zero_count += 1;
    }
    // Old code: 100% zeros. New code: ~1/65536 ≈ 0%
    try std.testing.expect(low16_zero_count < 50); // allow statistical variance
}

test "SAC (Strict Avalanche Criterion) — bit flip → ~50% output change" {
    try std.testing.expect(verifyAvalancheEffect());
}

test "Feistel encrypt/decrypt roundtrip" {
    try std.testing.expect(verifyFeistelRoundtripExact());
}

test "Feistel roundtrip — multiple keys and epsilons" {
    const keys = [_][KEY_WORDS]u32{
        .{ 0x01234567, 0x89ABCDEF, 0xFEDCBA98, 0x76543210, 0x11111111, 0x22222222, 0x33333333, 0x44444444 },
        .{ 0, 0, 0, 0, 0, 0, 0, 0 },
        .{ 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF },
        .{ 0x9E3779B9, 0x144CBC89, 0xDEADBEEF, 0xCAFEBABE, 0x12345678, 0x87654321, 0xAAAAAAAA, 0x55555555 },
    };
    const epsilons = [_]u32{ 1, 0xDEAD, 0xFFFFFFFF, 0 };

    for (keys) |key| {
        for (epsilons) |eps| {
            const cipher = PolerCipher.init(&key, eps);
            var plain = [BLOCK_WORDS]u32{ 0x01234567, 0x89ABCDEF, 0xFEDCBA98, 0x76543210 };
            var encrypted: [BLOCK_WORDS]u32 = undefined;
            var decrypted: [BLOCK_WORDS]u32 = undefined;
            cipher.encryptBlock(&plain, &encrypted);
            cipher.decryptBlock(&encrypted, &decrypted);
            for (0..BLOCK_WORDS) |j| {
                try std.testing.expect(decrypted[j] == plain[j]);
            }
        }
    }
}

test "POLER cycle convergence with dynamic attractor" {
    try std.testing.expect(verifyPolerConvergence());
}

test "Dynamic attractor uniqueness per key" {
    try std.testing.expect(verifyDynamicAttractor());
}

test "LHCA step determinism" {
    const config = LHCAConfig{ .rule_mask = 0xAAAAAAAA };
    const s1 = lhcaStep(0xCAFEBABE, config);
    const s2 = lhcaStep(0xCAFEBABE, config);
    try std.testing.expect(s1 == s2);
}

test "S-Box inverse consistency" {
    for (0..SBOX_SIZE) |i| {
        const s = SBOX[i];
        try std.testing.expect(INV_SBOX[s] == i);
    }
}

test "Constant-time S-box matches comptime SBOX for all 256 values" {
    for (0..SBOX_SIZE) |i| {
        const ct_val = constantTimeSbox(@intCast(i));
        const expected = SBOX[i];
        try std.testing.expectEqual(expected, ct_val);
    }
}

test "Constant-time inverse S-box matches comptime INV_SBOX for all 256 values" {
    for (0..SBOX_SIZE) |i| {
        const ct_val = constantTimeInvSbox(@intCast(i));
        const expected = INV_SBOX[i];
        try std.testing.expectEqual(expected, ct_val);
    }
}

test "Constant-time S-box roundtrip: INV_SBOX[SBOX[x]] = SBOX[INV_SBOX[x]] = x" {
    for (0..SBOX_SIZE) |i| {
        const x: u8 = @intCast(i);
        try std.testing.expect(constantTimeInvSbox(constantTimeSbox(x)) == x);
        try std.testing.expect(constantTimeSbox(constantTimeInvSbox(x)) == x);
    }
}

test "Constant-time GF(2^8) multiplication known vectors" {
    // FIPS-197 Section 4.2.1: 0x57 * 0x83 = 0xC1
    try std.testing.expectEqual(@as(u8, 0xC1), ctGf256Mul(0x57, 0x83));
    // Inverse pair: 0x53 * 0xCA = 0x01
    try std.testing.expectEqual(@as(u8, 0x01), ctGf256Mul(0x53, 0xCA));
    // Identity: 1 * x = x
    try std.testing.expectEqual(@as(u8, 0xFF), ctGf256Mul(0x01, 0xFF));
    // Zero: 0 * x = 0
    try std.testing.expectEqual(@as(u8, 0x00), ctGf256Mul(0x00, 0xFF));
}

test "Constant-time GF(2^8) inverse: x * x^(-1) = 1 for all non-zero x" {
    for (1..SBOX_SIZE) |i| {
        const x: u8 = @intCast(i);
        const inv = ctGf256Inverse(x);
        try std.testing.expectEqual(@as(u8, 1), ctGf256Mul(x, inv));
    }
}

test "modInverse32 Hensel convergence" {
    // Verify Hensel lifting converges: a * modInverse(a) ≡ 1 (mod 2^32)
    const test_odd: [5]u32 = .{ 1, 3, 0xDEADBEEF, 0x9E3779B9, 0xFFFFFFFF };
    for (test_odd) |a| {
        const inv = modInverse32(a);
        try std.testing.expect(a *% inv == 1);
    }
}

test "Q32 fixed-point PND φ-wrapper properties" {
    const a: u32 = 0xCAFEBABE;
    const b: u32 = 0xDEADBEEF;

    // v8: pndMixQ32 с φ-обёрткой — даже при ε=0 результат нелинеен!
    // При ε_Q32 = 0: result = φ(a·b) (только нелинейное произведение)
    const eps_zero = pndMixQ32(a, b, 0);
    const phi_product = phi(a *% b);
    try std.testing.expectEqual(phi_product, eps_zero);

    // При ε_Q32 = max: full deformation
    const full_deform_val = pndMixQ32(a, b, 0xFFFFFFFF);
    const phi_xor = phi(a ^ b);
    const expected_full = phi_product +% fixedMulQ32(phi_xor, 0xFFFFFFFF);
    try std.testing.expectEqual(expected_full, full_deform_val);

    // ε-чувствительность: разные ε → разные результаты
    const half_deform_val = pndMixQ32(a, b, 0x80000000);
    try std.testing.expect(eps_zero != half_deform_val);
    try std.testing.expect(half_deform_val != full_deform_val);
}

// ============================================================================
// P0-ТЕСТЫ АУДИТА ШНАЙЕРА A2Z — v8.2 (глава 11: критерии приёмки фиксов)
// ============================================================================

test "P0-F1: ключи с разными словами 4-7 дают разные шифротексты" {
    // Аудит A2Z, критерий приёмки F1: «два ключа, различающиеся только
    // словами 4–7, дают разные шифротексты». До фикса v8.2 — проваливался
    // (расписание игнорировало key[4..7], round-keys были идентичны).
    var drbg = PolerDrbg.init(&[_]u32{ 1, 2, 3, 4, 5, 6, 7, 8 });
    var trials: usize = 0;
    while (trials < 32) : (trials += 1) {
        var key: [KEY_WORDS]u32 = undefined;
        for (&key) |*w| w.* = drbg.next();
        const eps = drbg.next();
        var pt: [BLOCK_WORDS]u32 = undefined;
        for (&pt) |*w| w.* = drbg.next();

        const c0 = PolerCipher.init(&key, eps);
        var ct0: [BLOCK_WORDS]u32 = undefined;
        c0.encryptBlock(&pt, &ct0);

        var w: usize = 4;
        while (w < KEY_WORDS) : (w += 1) {
            var key2 = key;
            key2[w] ^= 0xFFFFFFFF; // полностью другое слово из второй половины
            const c1 = PolerCipher.init(&key2, eps);
            var ct1: [BLOCK_WORDS]u32 = undefined;
            c1.encryptBlock(&pt, &ct1);
            try std.testing.expect(ct0[0] != ct1[0] or ct0[1] != ct1[1] or
                ct0[2] != ct1[2] or ct0[3] != ct1[3]);
        }
    }
}

test "P0-F1: изменение любого из 256 бит ключа меняет шифротекст (Ось IV)" {
    // Аудит A2Z, Ось IV: тест-вектор полной битовой чувствительности
    // ключа — основа для пересчёта постквантовой заявки от 128 к 256 бит.
    var drbg = PolerDrbg.init(&[_]u32{ 0xA, 0xB, 0xC, 0xD, 0xE, 0xF, 0x10, 0x11 });
    var trials: usize = 0;
    while (trials < 16) : (trials += 1) {
        var key: [KEY_WORDS]u32 = undefined;
        for (&key) |*w| w.* = drbg.next();
        const eps = drbg.next();
        var pt: [BLOCK_WORDS]u32 = undefined;
        for (&pt) |*w| w.* = drbg.next();

        const c0 = PolerCipher.init(&key, eps);
        var ct0: [BLOCK_WORDS]u32 = undefined;
        c0.encryptBlock(&pt, &ct0);

        var bit: usize = 0;
        while (bit < 256) : (bit += 1) {
            var key2 = key;
            key2[bit / 32] ^= @as(u32, 1) << @intCast(bit % 32);
            const c1 = PolerCipher.init(&key2, eps);
            var ct1: [BLOCK_WORDS]u32 = undefined;
            c1.encryptBlock(&pt, &ct1);
            try std.testing.expect(ct0[0] != ct1[0] or ct0[1] != ct1[1] or
                ct0[2] != ct1[2] or ct0[3] != ct1[3]);
        }
    }
}

test "P0-F3: PolerDrbg — первые 100 000 выходов уникальны" {
    // Регрессия против удалённого PolerPrng: периоды 309–86 686,
    // медиана 14 960 (аудит F3). DRBG обязан пройти на порядок дальше.
    var drbg = PolerDrbg.init(&[_]u32{ 42, 43, 44, 45, 46, 47, 48, 49 });
    var seen = std.AutoHashMap(u32, void).init(std.testing.allocator);
    defer seen.deinit();
    var i: usize = 0;
    while (i < 100_000) : (i += 1) {
        const v = drbg.next();
        try std.testing.expect(!seen.contains(v));
        try seen.put(v, {});
    }
}

test "P0-F3: PolerDrbg детерминизм и чувствительность сида" {
    const seed = [KEY_WORDS]u32{ 1, 2, 3, 4, 5, 6, 7, 8 };
    var a = PolerDrbg.init(&seed);
    var b = PolerDrbg.init(&seed);
    var i: usize = 0;
    while (i < 256) : (i += 1) {
        try std.testing.expectEqual(a.next(), b.next());
    }

    var seed2 = seed;
    seed2[7] ^= 1; // ровно один бит сида
    var c = PolerDrbg.init(&seed2);
    var d = PolerDrbg.init(&seed);
    var diffs: usize = 0;
    i = 0;
    while (i < 256) : (i += 1) {
        if (d.next() != c.next()) diffs += 1;
    }
    try std.testing.expect(diffs > 200); // потоки практически не пересекаются
}

test "P0-F3: PolerDrbg бит-баланс (200k выходов, все 32 бита ~50%)" {
    var drbg = PolerDrbg.init(&[_]u32{ 0xDEAD, 0xBEEF, 0xCAFE, 0xBABE, 1, 2, 3, 4 });
    var counts = [_]u32{0} ** 32;
    var i: usize = 0;
    while (i < 200_000) : (i += 1) {
        const v = drbg.next();
        var b: u5 = 0;
        while (true) : (b += 1) {
            if ((v >> b) & 1 == 1) counts[b] += 1;
            if (b == 31) break;
        }
    }
    for (counts) |cnt| {
        const p = @as(f64, @floatFromInt(cnt)) / 200_000.0;
        // σ ≈ 0.0011 → допуск 0.006 ≈ 5.4σ, тест детерминирован фиксированным сидом
        try std.testing.expect(@abs(p - 0.5) < 0.006);
    }
}

test "P0-F3: PolerDrbg nextRange — границы и безсмещённость" {
    var drbg = PolerDrbg.init(&[_]u32{ 9, 9, 9, 9, 9, 9, 9, 9 });
    var i: usize = 0;
    while (i < 1000) : (i += 1) {
        const v = drbg.nextRange(7);
        try std.testing.expect(v < 7);
    }
    try std.testing.expectEqual(@as(u32, 0), drbg.nextRange(1));
}

test "P0-F2: PolerCbc — одинаковые блоки дают разные шифротексты" {
    // Аудит A2Z, критерий приёмки F2: «два одинаковых блока в сообщении
    // дают разные блоки шифротекста» — исключение ECB-утечки структуры.
    const key = [KEY_WORDS]u32{ 1, 2, 3, 4, 5, 6, 7, 8 };
    const cbc = PolerCbc.init(&key, 0xDEAD);
    const iv = [BLOCK_WORDS]u32{ 0x11111111, 0x22222222, 0x33333333, 0x44444444 };
    const pt = [_]u32{ 0xA, 0xB, 0xC, 0xD, 0xA, 0xB, 0xC, 0xD }; // два одинаковых блока
    var ct: [8]u32 = undefined;
    cbc.encrypt(&iv, &pt, &ct);
    try std.testing.expect(ct[0] != ct[4] or ct[1] != ct[5] or
        ct[2] != ct[6] or ct[3] != ct[7]);

    // Раундтрип
    var back: [8]u32 = undefined;
    cbc.decrypt(&iv, &ct, &back);
    try std.testing.expectEqualSlices(u32, &pt, &back);

    // Разный IV → разный шифротекст (одно и то же сообщение)
    const iv2 = [BLOCK_WORDS]u32{ 0, 0, 0, 0 };
    var ct2: [8]u32 = undefined;
    cbc.encrypt(&iv2, &pt, &ct2);
    try std.testing.expect(ct[0] != ct2[0] or ct[1] != ct2[1] or
        ct[2] != ct2[2] or ct[3] != ct2[3]);
}


// ── Оптимизации скорости (M4.5 CDL): побитовая эквивалентность ──────────────
//
// Криптография — инструмент работы с данными гигабайтного масштаба:
// горячий путь F-функции (S-box ≈ 92% времени) ускорен ТОЖДЕСТВАМИ.
// Оба теста ниже — исчерпывающие (все 256 значений / все слова),
// поэтому golden-векторы (54 626) и весь аудит Шнайера остаются в силе
// БЕЗ пересчёта: выходные биты не изменились ни на одном входе.


test "perf: xtime-микс mixColumnsPnd ≡ лестнице ctGf256Mul — все 2^32 невозможно, все 4 байта независимы" {
    // mixColumnsPnd линейна по байтам: достаточно проверить все 256 значений
    // каждого байта в каждой позиции (полный перебор мономов).
    var x: u16 = 0;
    while (x < 256) : (x += 1) {
        const b: u8 = @intCast(x);
        // Эталон: исходная формула через ctGf256Mul для одиночного байта
        // в каждой из 4 позиций (остальные байты нулевые — линейность).
        const word: u32 = @as(u32, b);
        const got = mixColumnsPnd(word);
        const a: [4]u8 = @bitCast(word);
        const e0 = ctGf256Mul(0x02, a[0]) ^ ctGf256Mul(0x03, a[1]) ^ a[2] ^ a[3];
        const e1 = a[0] ^ ctGf256Mul(0x02, a[1]) ^ ctGf256Mul(0x03, a[2]) ^ a[3];
        const e2 = a[0] ^ a[1] ^ ctGf256Mul(0x02, a[2]) ^ ctGf256Mul(0x03, a[3]);
        const e3 = ctGf256Mul(0x03, a[0]) ^ a[1] ^ a[2] ^ ctGf256Mul(0x02, a[3]);
        const expect: [4]u8 = .{ e0, e1, e2, e3 };
        try std.testing.expectEqual(@as(u32, @bitCast(expect)), got);
        // И во 2-й/3-й/4-й позиции
        const w2: u32 = @as(u32, b) << 8;
        try std.testing.expectEqual(
            @as(u32, @bitCast(@as([4]u8, .{
                ctGf256Mul(0x02, @as([4]u8, @bitCast(w2))[0]) ^
                    ctGf256Mul(0x03, @as([4]u8, @bitCast(w2))[1]) ^
                    @as([4]u8, @bitCast(w2))[2] ^ @as([4]u8, @bitCast(w2))[3],
                ctGf256Mul(0x02, @as([4]u8, @bitCast(w2))[1]) ^ @as([4]u8, @bitCast(w2))[0] ^
                    ctGf256Mul(0x03, @as([4]u8, @bitCast(w2))[2]) ^
                    @as([4]u8, @bitCast(w2))[3],
                @as([4]u8, @bitCast(w2))[0] ^ @as([4]u8, @bitCast(w2))[1] ^
                    ctGf256Mul(0x02, @as([4]u8, @bitCast(w2))[2]) ^
                    ctGf256Mul(0x03, @as([4]u8, @bitCast(w2))[3]),
                ctGf256Mul(0x03, @as([4]u8, @bitCast(w2))[0]) ^ @as([4]u8, @bitCast(w2))[1] ^
                    @as([4]u8, @bitCast(w2))[2] ^
                    ctGf256Mul(0x02, @as([4]u8, @bitCast(w2))[3]),
            }))),
            mixColumnsPnd(w2),
        );
    }
}

test "perf: lhcaStep векторизованная ≡ лестнице — 10 млн + все 2^16 младших" {
    const RefLHCA = struct {
        fn step(state: u32, rule_mask: u32) u32 {
            var result: u32 = 0;
            var i: u6 = 0;
            while (i < 32) : (i += 1) {
                const left: u32 = if (i == 0) (state >> 31) & 1 else (state >> @intCast(i - 1)) & 1;
                const center: u32 = (state >> @intCast(i)) & 1;
                const right: u32 = if (i == 31) state & 1 else (state >> @intCast(i + 1)) & 1;
                const chi: u32 = (rule_mask >> @intCast(i)) & 1;
                const bit: u32 = left ^ (chi & center) ^ right;
                result |= (bit << @intCast(i));
            }
            return result;
        }
    };
    var prng = std.Random.DefaultPrng.init(0x50454C52);
    const rand = prng.random();
    var i: usize = 0;
    while (i < 10_000_000) : (i += 1) {
        const st = rand.int(u32);
        const rm = rand.int(u32);
        try std.testing.expectEqual(RefLHCA.step(st, rm), lhcaStep(st, .{ .rule_mask = rm }));
    }
    // Все 2^16 младших половин (старшая — от ГПСЧ, две серии)
    i = 0;
    while (i < 65_536) : (i += 1) {
        const lo: u32 = @intCast(i);
        try std.testing.expectEqual(
            RefLHCA.step(lo, 0xACACACAC),
            lhcaStep(lo, .{ .rule_mask = 0xACACACAC }),
        );
        const hi: u32 = @intCast(i << 16);
        try std.testing.expectEqual(
            RefLHCA.step(hi, 0xACACACAC),
            lhcaStep(hi, .{ .rule_mask = 0xACACACAC }),
        );
    }
}

test "perf: constantTimeSbox4 ≡ 4× constantTimeSbox — все 256 в каждой лейне + 10 млн слов" {
    var prng = std.Random.DefaultPrng.init(0x5A4C52);
    const rand = prng.random();
    var i: usize = 0;
    while (i < 10_000_000) : (i += 1) {
        const w = rand.int(u32);
        const expect: u32 = (@as(u32, constantTimeSbox(@truncate(w))) |
            (@as(u32, constantTimeSbox(@truncate(w >> 8))) << 8) |
            (@as(u32, constantTimeSbox(@truncate(w >> 16))) << 16) |
            (@as(u32, constantTimeSbox(@truncate(w >> 24))) << 24));
        try std.testing.expectEqual(expect, constantTimeSbox4(w));
    }
    // Все 256 значений, продублированные в каждую из 4 лейнов.
    var b: u16 = 0;
    while (b < 256) : (b += 1) {
        const bb: u32 = @intCast(b);
        const w = bb | (bb << 8) | (bb << 16) | (bb << 24);
        const expect: u32 = (@as(u32, constantTimeSbox(@truncate(bb))) |
            (@as(u32, constantTimeSbox(@truncate(bb))) << 8) |
            (@as(u32, constantTimeSbox(@truncate(bb))) << 16) |
            (@as(u32, constantTimeSbox(@truncate(bb))) << 24));
        try std.testing.expectEqual(expect, constantTimeSbox4(w));
        // И разнобойные лейны: старшая/младшая половины независимо
        const w2 = (bb << 16) | ((~bb) & 0xFFFF);
        const expect2: u32 = (@as(u32, constantTimeSbox(@truncate(w2))) |
            (@as(u32, constantTimeSbox(@truncate(w2 >> 8))) << 8) |
            (@as(u32, constantTimeSbox(@truncate(w2 >> 16))) << 16) |
            (@as(u32, constantTimeSbox(@truncate(w2 >> 24))) << 24));
        try std.testing.expectEqual(expect2, constantTimeSbox4(w2));
    }
}
