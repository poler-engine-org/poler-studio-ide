//! TRIT-ASM — сбалансированная троичная физика процессора (v0.88.0).
//!
//! Триты 𝕋 = {−1, 0, +1} — знако-симметричный базис «истинного Буля»:
//! умножение тритов — это знак (`vpsignb`, БЕЗ умножителя), инверсия —
//! вычитание из нуля, вентили Клини — min/max (`vpminsb`/`vpmaxsb`),
//! упаковка 5 трит/байт — троичный Horner на `lea` (×3 = r+r·2, БЕЗ imul),
//! распаковка — magic-деление на 3 (`mul 0x55555556`, БЕЗ div).
//!
//! | Ядро | Математика | Базис |
//! |------|------------|-------|
//! | `poler_trit_neg` | ¬t = 0 − t | AVX (VEX `vpsubb`) |
//! | `poler_trit_and` | Клини AND = min | AVX+SSE4.1 (`vpminsb`) |
//! | `poler_trit_or` | Клини OR = max | AVX+SSE4.1 (`vpmaxsb`) |
//! | `poler_trit_dot` | ⟨u,v⟩ = Σuᵢvᵢ без умножителя | AVX+SSSE3 (`vpsignb`) |
//! | `poler_trit_pack5` | V = Σ(tₖ+1)·3ᵏ, 5 трит/байт (3⁵=243<256) | скаляр (цепочки LEA ×3) |
//! | `poler_trit_unpack5` | divmod 3 через magic-произведение | скаляр (`mul 0x55555556`) |
//! | `poler_trit_spike_f32` | тернарный спайк {⊕,⊙,⊖} с мёртвой зоной θ | SSE2 (базовый!) |
//! | `poler_trit_spike_f32_avx512` | то же, 16 lane за проход | AVX-512F (k-маски) |
//! | `poler_trit_mux512` | out = sel ? a : b — VPTERNLOGD imm 0xE4 | AVX-512F |
//!
//! VPTERNLOGD — аппаратная ТРОИЧНАЯ логика процессора: одна инструкция
//! вычисляет любую битовую функцию трёх операндов. Мультиплексор
//! sel?a:b закодирован imm 0xE4 (таблица истинности в комментарии ядра).
//!
//! ## Константы — Калькулятор Всего (директива: ноль Python)
//!
//! Все числовые константы модуля посчитаны самим движком:
//!
//! ```text
//! poler-engine --exec 'calc sqrt(3)/2'        → 0.8660254037844386
//! poler-engine --exec 'calc pi/2*2^30'        → 1686629713.0652523
//! poler-engine --exec 'calc atan(2^-29)*2^30' → 2.0 (последняя итерация Q30)
//! ```
//! Генератор `scripts/gen_asm_consts.py` пенсионирован: таблицы v0.88
//! выводит нативный `calc` движка.

use std::arch::global_asm;

global_asm! {
    r#"
    .text
    .p2align 4
    # magic-множитель беззнакового деления на 3: floor(v/3) = high32(v·M)
.Ltr_magic3:
    .long 0x55555556
    .p2align 4
    # 8×i16 единиц для vpmaddwd (сворачивание i16-пар в i32-суммы)
.Ltr_one16:
    .word 1, 1, 1, 1, 1, 1, 1, 1
    .p2align 4
    # тернарный спайк: ±1.0f и знаковая маска (4 lane SSE2-ядра)
.Ltr_one:
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
.Ltr_mone:
    .long 0xBF800000, 0xBF800000, 0xBF800000, 0xBF800000
.Ltr_sb:
    .long 0x80000000, 0x80000000, 0x80000000, 0x80000000
    .p2align 5
    # знаковая маска 16 lane для AVX-512 (vxorps → −θ)
.Ltr_sb512:
    .long 0x80000000, 0x80000000, 0x80000000, 0x80000000
    .long 0x80000000, 0x80000000, 0x80000000, 0x80000000
    .long 0x80000000, 0x80000000, 0x80000000, 0x80000000
    .long 0x80000000, 0x80000000, 0x80000000, 0x80000000

    .text
    # ================================================================
    # void poler_trit_neg(const i8* a, i8* out, size_t n)
    #   rdi=a, rsi=out, rdx=n.  Поразрядно: out = 0 − t (инверсия).
    #   16 трит за итерацию, VEX-кодировка (без порчи флагов).
    # ================================================================
    .globl poler_trit_neg
    .p2align 4
poler_trit_neg:
    xor rcx, rcx
    vpxor xmm1, xmm1, xmm1          # ноль
    test rdx, rdx
    jz .Ltn_fin
.Ltn_lp:
    mov rax, rdx
    sub rax, rcx
    cmp rax, 16
    jb .Ltn_fin
    vmovdqu xmm0, [rdi + rcx]
    vpsubb xmm2, xmm1, xmm0         # 0 − t
    vmovdqu [rsi + rcx], xmm2
    add rcx, 16
    jmp .Ltn_lp
.Ltn_fin:
    ret

    # ================================================================
    # void poler_trit_and(const i8* a, const i8* b, i8* out, size_t n)
    #   rdi=a, rsi=b, rdx=out, rcx=n.
    #   Клини AND = min(a, b) — трёхзначная логика.
    # ================================================================
    .globl poler_trit_and
    .p2align 4
poler_trit_and:
    xor r9, r9
    test rcx, rcx
    jz .Lta_fin
.Lta_lp:
    mov rax, rcx
    sub rax, r9
    cmp rax, 16
    jb .Lta_fin
    vmovdqu xmm0, [rdi + r9]
    vmovdqu xmm1, [rsi + r9]
    vpminsb xmm2, xmm0, xmm1        # min = AND Клини
    vmovdqu [rdx + r9], xmm2
    add r9, 16
    jmp .Lta_lp
.Lta_fin:
    ret

    # ================================================================
    # void poler_trit_or(const i8* a, const i8* b, i8* out, size_t n)
    #   rdi=a, rsi=b, rdx=out, rcx=n.
    #   Клини OR = max(a, b).
    # ================================================================
    .globl poler_trit_or
    .p2align 4
poler_trit_or:
    xor r9, r9
    test rcx, rcx
    jz .Lto_fin
.Lto_lp:
    mov rax, rcx
    sub rax, r9
    cmp rax, 16
    jb .Lto_fin
    vmovdqu xmm0, [rdi + r9]
    vmovdqu xmm1, [rsi + r9]
    vpmaxsb xmm2, xmm0, xmm1        # max = OR Клини
    vmovdqu [rdx + r9], xmm2
    add r9, 16
    jmp .Lto_lp
.Lto_fin:
    ret

    # ================================================================
    # i64 poler_trit_dot(const i8* a, const i8* b, size_t n)
    #   Скалярное произведение БЕЗ умножителя:
    #   vpsignb(b, a) = b·sign(a) = a·b  для a ∈ {{−1,0,1}}.
    #   Разворачивание i8→i16 (vpmovsxbw) + свёртка vpmaddwd.
    # ================================================================
    .globl poler_trit_dot
    .p2align 4
poler_trit_dot:
    xor rax, rax                     # аккумулятор
    xor rcx, rcx
    vpxor xmm5, xmm5, xmm5           # векторный аккумулятор 4×i32
    test rdx, rdx
    jz .Ltd_red
.Ltd_lp:
    mov r9, rdx
    sub r9, rcx
    cmp r9, 16
    jb .Ltd_tail
    vmovdqu xmm0, [rdi + rcx]        # a
    vmovdqu xmm1, [rsi + rcx]        # b
    vpsignb xmm2, xmm1, xmm0         # a·b (знаковое умножение тритов)
    vpmovsxbw xmm3, xmm2             # байты 0..7 → i16
    vpsrldq xmm4, xmm2, 8            # байты 8..15 → младшие
    vpmovsxbw xmm4, xmm4
    vpmaddwd xmm3, xmm3, [rip + .Ltr_one16]
    vpmaddwd xmm4, xmm4, [rip + .Ltr_one16]
    vpaddd xmm5, xmm5, xmm3
    vpaddd xmm5, xmm5, xmm4
    add rcx, 16
    jmp .Ltd_lp
.Ltd_tail:
    cmp rcx, rdx
    jae .Ltd_red
    movsx r8d, byte ptr [rdi + rcx]
    movsx r9d, byte ptr [rsi + rcx]
    imul r8d, r9d
    movsxd r9, r8d
    add rax, r9
    inc rcx
    jmp .Ltd_tail
.Ltd_red:
    vpshufd xmm6, xmm5, 0x4E         # [2,3,0,1]
    vpaddd xmm5, xmm5, xmm6
    vpshufd xmm6, xmm5, 0x01         # [1,0,2,3]
    vpaddd xmm5, xmm5, xmm6
    vmovd r10d, xmm5
    movsxd r10, r10d
    add rax, r10
    ret

    # ================================================================
    # size_t poler_trit_pack5(const i8* a, u8* out, size_t n)
    #   5 трит → байт: V = Σ(tₖ+1)·3ᵏ, старшая цифра — ПЕРВАЯ
    #   (big-endian, контракт Trits::digits). Троичный Horner:
    #   v = v·3 + (t+1) — умножение на 3 ТОЛЬКО lea [r+r*2].
    #   Возвращает число полных упакованных байт (= n/5).
    # ================================================================
    .globl poler_trit_pack5
    .p2align 4
poler_trit_pack5:
    xor rcx, rcx                     # i — индекс трита
    xor r8, r8                       # j — индекс байта
    test rdx, rdx
    jz .Ltp_fin
.Ltp_lp:
    lea r9, [rcx + 5]
    cmp r9, rdx
    ja .Ltp_fin                      # осталось < 5 трит
    xor r10d, r10d                   # v = 0
    movsx eax, byte ptr [rdi + rcx]
    lea r10, [r10 + r10*2]           # v·3 — троичный умножитель LEA
    lea r10, [r10 + rax + 1]         # v·3 + t + 1
    movsx eax, byte ptr [rdi + rcx + 1]
    lea r10, [r10 + r10*2]
    lea r10, [r10 + rax + 1]
    movsx eax, byte ptr [rdi + rcx + 2]
    lea r10, [r10 + r10*2]
    lea r10, [r10 + rax + 1]
    movsx eax, byte ptr [rdi + rcx + 3]
    lea r10, [r10 + r10*2]
    lea r10, [r10 + rax + 1]
    movsx eax, byte ptr [rdi + rcx + 4]
    lea r10, [r10 + r10*2]
    lea r10, [r10 + rax + 1]
    mov [rsi + r8], r10b
    lea rcx, [rcx + 5]
    inc r8
    jmp .Ltp_lp
.Ltp_fin:
    mov rax, r8
    ret

    # ================================================================
    # size_t poler_trit_unpack5(const u8* packed, i8* out, size_t n_bytes)
    #   Байт → 5 трит: q = floor(v/3) через high32(v·0x55555556)
    #   (magic-деление, БЕЗ div), r = v − 3q, трит = r − 1.
    #   ВНИМАНИЕ: mul r10d пишет старшую половину в edx — потому n
    #   живёт в r11, а v — в КАЛЛИ-СОХРАНЁННОМ rbx (переживает mul).
    #   Порядок big-endian: первым извлекается младшая цифра —
    #   пишется от [base+4] к [base]. Возвращает n_bytes·5 тритов.
    # ================================================================
    .globl poler_trit_unpack5
    .p2align 4
poler_trit_unpack5:
    push rbx
    mov r10d, 0x55555556             # magic-множитель /3 (hoist)
    xor rcx, rcx                     # j — индекс байта
    mov r9, rsi                      # r9 = бегущий указатель записи
    mov r11, rdx                     # n (mul бьёт rdx — сохранено!)
    test r11, r11
    jz .Ltu_fin
.Ltu_lp:
    cmp rcx, r11
    jae .Ltu_fin
    movzx ebx, byte ptr [rdi + rcx]  # v ∈ [0, 242] (rbx переживёт mul)
    mov eax, ebx
    mul r10d                         # edx = floor(v/3)
    lea r8, [rdx + rdx*2]            # 3q (LEA!)
    mov eax, ebx
    sub eax, r8d                     # r ∈ {{0,1,2}}
    dec eax                          # трит ∈ {{−1,0,1}}
    mov [r9 + 4], al
    mov ebx, edx                     # v = q
    mov eax, ebx
    mul r10d
    lea r8, [rdx + rdx*2]
    mov eax, ebx
    sub eax, r8d
    dec eax
    mov [r9 + 3], al
    mov ebx, edx
    mov eax, ebx
    mul r10d
    lea r8, [rdx + rdx*2]
    mov eax, ebx
    sub eax, r8d
    dec eax
    mov [r9 + 2], al
    mov ebx, edx
    mov eax, ebx
    mul r10d
    lea r8, [rdx + rdx*2]
    mov eax, ebx
    sub eax, r8d
    dec eax
    mov [r9 + 1], al
    mov ebx, edx
    mov eax, ebx
    mul r10d
    lea r8, [rdx + rdx*2]
    mov eax, ebx
    sub eax, r8d
    dec eax
    mov [r9], al
    inc rcx
    add r9, 5
    jmp .Ltu_lp
.Ltu_fin:
    lea rax, [rcx + rcx*4]           # n_bytes · 5 (LEA ×5)
    pop rbx
    ret

    # ================================================================
    # void poler_trit_spike_f32(const f32* x, f32* out, size_t n,
    #                            float theta)   [xmm0 = θ]
    #   ТРОИЧНЫЙ спайк поляризации с мёртвой зоной [−θ, θ]:
    #     x >  θ  →  +1.0  (⊕ деполяризация)
    #     x < −θ  →  −1.0  (⊖ гиперполяризация)
    #     иначе   →   0.0  (⊙ рефрактерный стазис)
    #   Голый SSE2 (cmpps imm 6 = NLE, imm 1 = LT), без ветвлений.
    # ================================================================
    .globl poler_trit_spike_f32
    .p2align 4
poler_trit_spike_f32:
    # rdi=x, rsi=out, rdx=n, xmm0=θ (scalar in lane 0 -> broadcast to all 4 lanes!)
    shufps xmm0, xmm0, 0x00          # broadcast θ to all 4 lanes
    movaps xmm7, xmm0                # θ (all 4 lanes)
    movaps xmm6, xmm0
    xorps xmm6, [rip + .Ltr_sb]      # −θ (all 4 lanes)
    xor rcx, rcx
    test rdx, rdx
    jz .Lts_fin
.Lts_lp:
    mov rax, rdx
    sub rax, rcx
    cmp rax, 4
    jb .Lts_tail
    movups xmm0, [rdi + rcx*4]
    movaps xmm1, xmm0
    cmpps xmm1, xmm7, 6              # pos: x > θ (NLE)
    movaps xmm2, xmm0
    cmpps xmm2, xmm6, 1              # neg: x < −θ (LT)
    movaps xmm3, xmm1
    andps xmm3, [rip + .Ltr_one]     # pos → +1.0
    movaps xmm4, xmm2
    andps xmm4, [rip + .Ltr_mone]    # neg → −1.0
    orps xmm3, xmm4                  # трит поляризации
    movups [rsi + rcx*4], xmm3
    add rcx, 4
    jmp .Lts_lp
.Lts_tail:
    cmp rcx, rdx
    jae .Lts_fin
    movss xmm0, [rdi + rcx*4]
    comiss xmm0, xmm7
    jbe 2f
    movss xmm1, [rip + .Ltr_one]
    jmp 3f
2:
    comiss xmm0, xmm6
    jae 4f
    movss xmm1, [rip + .Ltr_mone]
    jmp 3f
4:
    xorps xmm1, xmm1
3:
    movss [rsi + rcx*4], xmm1
    inc rcx
    jmp .Lts_tail
.Lts_fin:
    ret

    # ================================================================
    # void poler_trit_spike_f32_avx512(const f32* x, f32* out, size_t n,
    #                                  float theta)  [xmm0 = θ]
    #   То же, 16 lane за проход: vcmpps → k-маски, masked-blend.
    #   Обрабатывает только кратные 16 (хвост — обёртка Rust).
    # ================================================================
    .globl poler_trit_spike_f32_avx512
    .p2align 4
poler_trit_spike_f32_avx512:
    vbroadcastss zmm1, xmm0          # θ
    vmovdqu32 zmm3, [rip + .Ltr_sb512]   # НЕвыровненно: zmm требует 64Б,
                                          # .p2align 5 даёт лишь 32Б (#GP!)
    vbroadcastss zmm2, xmm0
    vxorps zmm2, zmm2, zmm3          # −θ
    vbroadcastss zmm4, [rip + .Ltr_one]
    vbroadcastss zmm5, [rip + .Ltr_mone]
    xor rcx, rcx
    test rdx, rdx
    jz .Lts5_fin
.Lts5_lp:
    mov rax, rdx
    sub rax, rcx
    cmp rax, 16
    jb .Lts5_fin
    vmovups zmm0, [rdi + rcx*4]
    vcmpps k1, zmm0, zmm1, 30        # GT_OQ: x > θ
    vcmpps k2, zmm0, zmm2, 17        # LT_OQ: x < −θ
    vpxord zmm6, zmm6, zmm6          # ⊙ 0
    vmovdqa32 zmm6 {{k1}}, zmm4        # ⊕ +1
    vmovdqa32 zmm6 {{k2}}, zmm5        # ⊖ −1
    vmovups [rsi + rcx*4], zmm6
    add rcx, 16
    jmp .Lts5_lp
.Lts5_fin:
    vzeroupper
    ret

    # ================================================================
    # void poler_trit_mux512(const u32* a, const u32* b,
    #                        const u32* sel, u32* out, size_t n)
    #   Аппаратная троичная логика — БИТОВЫЙ тернарный селектор:
    #     out_bit = sel_bit ? a_bit : b_bit   (побитово!)
    #   VPTERNLOGD imm 0xE4. Индекс бита imm8: i = (A<<2)|(B<<1)|C,
    #   где A = ПЕРВЫЙ операнд (dst) — СТАРШИЙ бит индекса, B = второй,
    #   C = третий (sel). Таблица истинности f = C ? A : B:
    #     C=1 → f=A (i5, i7); C=0 → f=B (i2, i6)
    #     → биты {{2,5,6,7}} → imm = 0b11100100 = 0xE4.
    #   (0xAC при этой конвенции = A?C:B — верифицировано тестом;
    #   знаменитый 0xCA = A?B:C.)
    #   Одна инструкция вместо (sel AND a) OR (NOT sel AND b).
    # ================================================================
    .globl poler_trit_mux512
    .p2align 4
poler_trit_mux512:
    # rdi=a, rsi=b, rdx=sel, rcx=out, r8=n
    xor r9, r9
    test r8, r8
    jz .Ltm_fin
.Ltm_lp:
    mov rax, r8
    sub rax, r9
    cmp rax, 16
    jb .Ltm_fin
    vmovdqu32 zmm0, [rdi + r9*4]    # a
    vmovdqu32 zmm1, [rsi + r9*4]    # b
    vmovdqu32 zmm2, [rdx + r9*4]    # sel
    vpternlogd zmm0, zmm1, zmm2, 0xE4
    vmovdqu32 [rcx + r9*4], zmm0
    add r9, 16
    jmp .Ltm_lp
.Ltm_fin:
    vzeroupper
    ret
    "#
}

extern "sysv64" {
    fn poler_trit_neg(a: *const i8, out: *mut i8, n: usize);
    fn poler_trit_and(a: *const i8, b: *const i8, out: *mut i8, n: usize);
    fn poler_trit_or(a: *const i8, b: *const i8, out: *mut i8, n: usize);
    fn poler_trit_dot(a: *const i8, b: *const i8, n: usize) -> i64;
    fn poler_trit_pack5(a: *const i8, out: *mut u8, n: usize) -> usize;
    fn poler_trit_unpack5(packed: *const u8, out: *mut i8, n_bytes: usize) -> usize;
    fn poler_trit_spike_f32(x: *const f32, out: *mut f32, n: usize, theta: f32);
    fn poler_trit_spike_f32_avx512(x: *const f32, out: *mut f32, n: usize, theta: f32);
    fn poler_trit_mux512(a: *const u32, b: *const u32, sel: *const u32, out: *mut u32, n: usize);
}

/// Порог готовности VEX/SSSE3/SSE4.1-ядер.
fn trit_simd_ready() -> bool {
    let c = crate::asm::caps();
    c.avx && c.ssse3 && c.sse41
}

/// Инверсия трит-вектора: out = −a (поразрядно). Хвост — скалярно.
pub fn neg(a: &[i8], out: &mut [i8]) {
    let n = a.len().min(out.len());
    let bulk = if trit_simd_ready() { n / 16 * 16 } else { 0 };
    if bulk > 0 {
        unsafe { poler_trit_neg(a.as_ptr(), out.as_mut_ptr(), bulk) };
    }
    for i in bulk..n {
        out[i] = -a[i];
    }
}

/// Вентиль Клини AND = min (трёхзначная логика). Хвост — скалярно.
pub fn kleene_and(a: &[i8], b: &[i8], out: &mut [i8]) {
    let n = a.len().min(b.len()).min(out.len());
    let bulk = if trit_simd_ready() { n / 16 * 16 } else { 0 };
    if bulk > 0 {
        unsafe { poler_trit_and(a.as_ptr(), b.as_ptr(), out.as_mut_ptr(), bulk) };
    }
    for i in bulk..n {
        out[i] = a[i].min(b[i]);
    }
}

/// Вентиль Клини OR = max. Хвост — скалярно.
pub fn kleene_or(a: &[i8], b: &[i8], out: &mut [i8]) {
    let n = a.len().min(b.len()).min(out.len());
    let bulk = if trit_simd_ready() { n / 16 * 16 } else { 0 };
    if bulk > 0 {
        unsafe { poler_trit_or(a.as_ptr(), b.as_ptr(), out.as_mut_ptr(), bulk) };
    }
    for i in bulk..n {
        out[i] = a[i].max(b[i]);
    }
}

/// Скалярное произведение трит-векторов БЕЗ умножителя:
/// `vpsignb` умножает знаком — ⟨u, v⟩ ∈ [−n, +n].
pub fn dot(a: &[i8], b: &[i8]) -> i64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0;
    }
    if trit_simd_ready() {
        unsafe { poler_trit_dot(a.as_ptr(), b.as_ptr(), n) }
    } else {
        a.iter().zip(b).map(|(&x, &y)| x as i64 * y as i64).sum()
    }
}

/// Упаковка 5 сбалансированных тритов в байт: V = Σ(tₖ+1)·3ᵏ ≤ 242.
///
/// Порядок big-endian — контракт [`crate::calc::trits::Trits`]:
/// первый трит среза — старшая цифра. Возвращает число байт.
pub fn pack5(trits: &[i8], out: &mut [u8]) -> usize {
    let n = trits.len();
    let n_bytes = n / 5;
    if n_bytes == 0 || out.len() < n_bytes {
        return 0;
    }
    unsafe { poler_trit_pack5(trits.as_ptr(), out.as_mut_ptr(), n) }
}

/// Распаковка байтов в 5 тритов (magic-деление на 3, без `div`).
/// Возвращает число записанных тритов.
pub fn unpack5(packed: &[u8], out: &mut [i8]) -> usize {
    let n_bytes = packed.len();
    if n_bytes == 0 || out.len() < n_bytes * 5 {
        return 0;
    }
    unsafe { poler_trit_unpack5(packed.as_ptr(), out.as_mut_ptr(), n_bytes) }
}

/// Тернарный спайк: x → {+1.0, 0.0, −1.0} с мёртвой зоной [−θ, θ].
///
/// Физика: ⊕ деполяризация / ⊙ рефрактерный стазис / ⊖ гиперполяризация.
pub fn spike_f32(x: &[f32], out: &mut [f32], theta: f32) {
    let n = x.len().min(out.len());
    if n == 0 {
        return;
    }
    let c = crate::asm::caps();
    if c.avx512f {
        let bulk = n / 16 * 16;
        if bulk > 0 {
            unsafe { poler_trit_spike_f32_avx512(x.as_ptr(), out.as_mut_ptr(), bulk, theta) };
        }
        for i in bulk..n {
            out[i] = if x[i] > theta {
                1.0
            } else if x[i] < -theta {
                -1.0
            } else {
                0.0
            };
        }
    } else {
        // SSE2-ядро обрабатывает всё, включая хвост
        unsafe { poler_trit_spike_f32(x.as_ptr(), out.as_mut_ptr(), n, theta) };
    }
}

/// Битовый тернарный селектор на VPTERNLOGD (imm 0xE4):
/// out = (sel & a) | (¬sel & b) — побитовый уровень u32,
/// 16 lane за инструкцию. AVX-512F или скаляр.
pub fn mux512(a: &[u32], b: &[u32], sel: &[u32], out: &mut [u32]) {
    let n = a.len().min(b.len()).min(sel.len()).min(out.len());
    if n == 0 {
        return;
    }
    let c = crate::asm::caps();
    let bulk = if c.avx512f { n / 16 * 16 } else { 0 };
    if bulk > 0 {
        unsafe { poler_trit_mux512(a.as_ptr(), b.as_ptr(), sel.as_ptr(), out.as_mut_ptr(), bulk) };
    }
    for i in bulk..n {
        out[i] = (sel[i] & a[i]) | (!sel[i] & b[i]);
    }
}

// ------------------------------ тесты ------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn xs64(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        *state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn trit_vec(n: usize, seed: u64) -> Vec<i8> {
        let mut s = seed | 1;
        (0..n).map(|_| ((xs64(&mut s) >> 33) % 3) as i64 as i8 - 1).collect()
    }

    #[test]
    fn trit_neg_kleene_parity() {
        let a = trit_vec(1000, 11);
        let b = trit_vec(1000, 22);
        let mut n = vec![0i8; 1000];
        let mut k = vec![0i8; 1000];
        let mut o = vec![0i8; 1000];
        neg(&a, &mut n);
        kleene_and(&a, &b, &mut k);
        kleene_or(&a, &b, &mut o);
        for i in 0..1000 {
            assert_eq!(n[i], -a[i], "neg[{i}]");
            assert_eq!(k[i], a[i].min(b[i]), "and[{i}]");
            assert_eq!(o[i], a[i].max(b[i]), "or[{i}]");
        }
    }

    #[test]
    fn trit_dot_matches_scalar() {
        let a = trit_vec(997, 33);
        let b = trit_vec(997, 44);
        let asm = dot(&a, &b);
        let sc: i64 = a.iter().zip(&b).map(|(&x, &y)| x as i64 * y as i64).sum();
        assert_eq!(asm, sc, "dot asm={asm} scalar={sc}");
    }

    #[test]
    fn trit_pack_unpack_roundtrip() {
        let a = trit_vec(5000, 55); // 1000 полных пятёрок
        let mut packed = vec![0u8; 1000];
        let mut unpacked = vec![0i8; 5000];
        let nb = pack5(&a, &mut packed);
        assert_eq!(nb, 1000);
        let nt = unpack5(&packed, &mut unpacked);
        assert_eq!(nt, 5000);
        assert_eq!(&a[..5000], &unpacked[..5000]);
        // диапазон байта: 0..242
        assert!(packed.iter().all(|&v| v <= 242));
    }

    #[test]
    fn trit_pack5_agrees_with_engine_trits() {
        // паритет с канонической сбалансированной троичной арифметикой движка
        use crate::calc::trits::Trits;
        for v in [-100i64, -42, -13, -1, 0, 1, 7, 42, 100, 121] {
            let t = Trits::from_i64(v).unwrap();
            let mut digits = t.digits.clone();
            while digits.len() % 5 != 0 {
                digits.insert(0, 0); // старшие нули до кратности 5
            }
            let mut packed = vec![0u8; digits.len() / 5];
            let mut unpacked = vec![0i8; digits.len()];
            pack5(&digits, &mut packed);
            unpack5(&packed, &mut unpacked);
            assert_eq!(digits, unpacked, "v={v}");
            assert_eq!(Trits { digits: unpacked }.to_i64(), v);
        }
    }

    #[test]
    fn trit_spike_ternary_semantics() {
        let x: Vec<f32> = vec![2.0, 0.6, 0.5, 0.1, 0.0, -0.5, -0.6, -2.0];
        let mut out = vec![0.0f32; x.len()];
        spike_f32(&x, &mut out, 0.5);
        let expect = [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, -1.0];
        for i in 0..x.len() {
            assert_eq!(out[i], expect[i], "spike[{i}] x={}", x[i]);
        }
    }

    #[test]
    fn trit_spike_random_parity() {
        let mut s = 777u64 | 1;
        let x: Vec<f32> = (0..4096)
            .map(|_| ((xs64(&mut s) >> 33) % 200) as f32 / 100.0 - 1.0)
            .collect();
        let mut out = vec![0.0f32; x.len()];
        spike_f32(&x, &mut out, 0.35);
        for i in 0..x.len() {
            let e = if x[i] > 0.35 {
                1.0
            } else if x[i] < -0.35 {
                -1.0
            } else {
                0.0
            };
            assert_eq!(out[i], e, "spike[{i}] x={}", x[i]);
        }
    }

    #[test]
    fn trit_mux512_ternary_selector() {
        // битовый тернарный мультиплексор: out = (sel & a) | (¬sel & b)
        let a: Vec<u32> = (0..64).map(|i| i as u32 * 7 + 1).collect();
        let b: Vec<u32> = (0..64).map(|i| i as u32 * 13 + 5).collect();
        let sel: Vec<u32> = (0..64).map(|i| (0x33333333u32 >> (i % 5)) + i as u32).collect();
        let mut out = vec![0u32; 64];
        mux512(&a, &b, &sel, &mut out);
        for i in 0..64 {
            let e = (sel[i] & a[i]) | (!sel[i] & b[i]);
            assert_eq!(out[i], e, "mux[{i}]");
        }
    }
}
