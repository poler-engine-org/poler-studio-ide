//! Синапс-атомарная SSN сокровищницы в машинном коде x86_64 (AVX2+FMA+F16C).
//!
//! Порт «Synapse-atomic SSN» (vault блоки 0986/0984 — «1.7 Б/синапс»)
//! на чистый ассемблер. Плотность:
//!
//! ```text
//! режим fp16:  w: u16 (binary16)            → 2.0 Б/синапс (payload)
//! режим i8:    w: i8 × глобальный scale     → 1.0 Б/синапс (payload)
//! func_id:     u4 на ИСТОЧНИК (не на синапс) → ~0 Б/синапс (амортизация)
//! bias:        f32 на пост-нейрон            → ~0 Б/синапс (амортизация)
//! ```
//! Цифра сокровищницы 1.7 Б/синапс перекрыта: fp16 ≈ 2.0 Б payload +
//! O(neurons) амортизация, i8 = 1.0 Б. 50M синапсов FlyWire → 50–100 МБ.
//!
//! ## Топология и шаг
//!
//! ```text
//! dst(s)   = (s·α) & (n_post−1)        — хэш-адресация (n_post = 2^k)
//! acc(s)   = Σ_k prim_{func[s]}( w[s,k] · pre[s+k] )
//! post[d]  = decay·post[d] + acc + bias[d]
//! ```
//!
//! ## 16 примитивов (u4 func_id → jump table)
//!
//! 0 identity, 1 relu, 2 sigmoid, 3 tanh, 4 expdecay, 5 spike, 6 gauss,
//! 7 abs, 8 sign, 9 softplus, 10 sin, 11 gate, 12 delay, 13 trace,
//! 14 clamp, 15 parity. Трансцендентные — мантисса-полиномы по битам
//! float (без libm): exp за 12 инструкций, tanh — рационал 27/9.
//!
//! Примитив 5 (`spike`) — ТРОИЧНЫЙ (v0.88.0): трит поляризации
//! {+1 деполяризация, 0 рефрактерный стазис, −1 гиперполяризация}
//! с мёртвой зоной [−0.5, 0.5] — не булево отсечение.
//!
//! ## Контракт паддинга (обязателен для прямых вызовов asm)
//!
//! * `w16` — до `n_pre·fanout` округлённого вверх кратного 8, +16 Б;
//! * `w8`  — то же в байтах;
//! * `pre` — `n_pre + fanout + 8` элементов (хвостовые lane маскируются
//!   keep-маской из `.rodata` — примитив видит мусорные lane, но их
//!   вклад в аккумулятор зануляется ПОСЛЕ примитива).

use std::arch::global_asm;

/// Идентификаторы примитивов ObservationCircuit (u4).
pub const FUNC_IDENTITY: u8 = 0;
pub const FUNC_RELU: u8 = 1;
pub const FUNC_SIGMOID: u8 = 2;
pub const FUNC_TANH: u8 = 3;
pub const FUNC_EXPDECAY: u8 = 4;
pub const FUNC_SPIKE: u8 = 5;
pub const FUNC_GAUSS: u8 = 6;
pub const FUNC_ABS: u8 = 7;
pub const FUNC_SIGN: u8 = 8;
pub const FUNC_SOFTPLUS: u8 = 9;
pub const FUNC_SIN: u8 = 10;
pub const FUNC_GATE: u8 = 11;
pub const FUNC_DELAY: u8 = 12;
pub const FUNC_TRACE: u8 = 13;
pub const FUNC_CLAMP: u8 = 14;
pub const FUNC_PARITY: u8 = 15;
pub const FUNC_COUNT: usize = 16;

/// Человекочитаемые имена примитивов (индекс = func_id).
pub const FUNC_NAMES: [&str; FUNC_COUNT] = [
    "identity", "relu", "sigmoid", "tanh", "expdecay", "spike", "gauss",
    "abs", "sign", "softplus", "sin", "gate", "delay", "trace", "clamp",
    "parity",
];

/// Параметры микроядра fp16 (SysV: один указатель в rdi).
#[repr(C)]
pub struct SsnParams {
    pub w: *const u16,        // +0:  веса fp16 [n_pre·fanout]
    pub pre: *const f32,      // +8:  состояния источников [n_pre+fanout+8]
    pub post: *mut f32,       // +16: состояния приёмников [n_post] (io)
    pub func: *const u8,      // +24: func_id на источник [n_pre]
    pub bias: *const f32,     // +32: смещения на приёмник [n_post]
    pub n_pre: usize,         // +40
    pub fanout: usize,        // +48
    pub post_mask: u32,       // +56: n_post−1 (степень двойки!)
    pub post_stride: u32,     // +60: α (нечётный множитель хэша)
    pub decay: f32,           // +64
    pub gate: f32,            // +68: значение pre[s] для примитива gate
}

/// Параметры микроядра i8 (то же + масштаб весов).
#[repr(C)]
pub struct SsnParamsI8 {
    pub w: *const i8,         // +0
    pub pre: *const f32,      // +8
    pub post: *mut f32,       // +16
    pub func: *const u8,      // +24
    pub bias: *const f32,     // +32
    pub n_pre: usize,         // +40
    pub fanout: usize,        // +48
    pub post_mask: u32,       // +56
    pub post_stride: u32,     // +60
    pub decay: f32,           // +64
    pub gate: f32,            // +68
    pub scale: f32,           // +72: f32 = i8 · scale
}

global_asm! {
    r#"
    .text
    .p2align 5
.Lssn_one:
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .p2align 5
.Lssn_half:
    .long 0x3F000000, 0x3F000000, 0x3F000000, 0x3F000000
    .long 0x3F000000, 0x3F000000, 0x3F000000, 0x3F000000
    .p2align 5
.Lssn_abs:
    .long 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF
    .long 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF
    .p2align 5
.Lssn_sb:
    .long 0x80000000, 0x80000000, 0x80000000, 0x80000000
    .long 0x80000000, 0x80000000, 0x80000000, 0x80000000
    .p2align 5
    # порог спайка 0.5 / −0.5 (мёртвая зона тритного стазиса)
.Lssn_thr:
    .long 0x3F000000, 0x3F000000, 0x3F000000, 0x3F000000
    .long 0x3F000000, 0x3F000000, 0x3F000000, 0x3F000000
    .p2align 5
.Lssn_nthr:
    .long 0xBF000000, 0xBF000000, 0xBF000000, 0xBF000000
    .long 0xBF000000, 0xBF000000, 0xBF000000, 0xBF000000
    .p2align 5
.Lssn_mone:
    .long 0xBF800000, 0xBF800000, 0xBF800000, 0xBF800000
    .long 0xBF800000, 0xBF800000, 0xBF800000, 0xBF800000
    .p2align 5
.Lssn_c27:
    .float 27.0, 27.0, 27.0, 27.0, 27.0, 27.0, 27.0, 27.0
    .p2align 5
.Lssn_c9:
    .float 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0
    .p2align 5
.Lssn_n4:
    .float -4.0, -4.0, -4.0, -4.0, -4.0, -4.0, -4.0, -4.0
    .p2align 5
.Lssn_p4:
    .float 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0
    .p2align 5
.Lssn_hpi:
    .float 1.5707963268, 1.5707963268, 1.5707963268, 1.5707963268
    .float 1.5707963268, 1.5707963268, 1.5707963268, 1.5707963268
    .p2align 5
.Lssn_nhpi:
    .float -1.5707963268, -1.5707963268, -1.5707963268, -1.5707963268
    .float -1.5707963268, -1.5707963268, -1.5707963268, -1.5707963268
    .p2align 5
.Lssn_log2e:
    .float 1.4426950409, 1.4426950409, 1.4426950409, 1.4426950409
    .float 1.4426950409, 1.4426950409, 1.4426950409, 1.4426950409
    .p2align 5
.Lssn_ec1:
    .float 0.6931471806, 0.6931471806, 0.6931471806, 0.6931471806
    .float 0.6931471806, 0.6931471806, 0.6931471806, 0.6931471806
    .p2align 5
.Lssn_ec2:
    .float 0.2402265070, 0.2402265070, 0.2402265070, 0.2402265070
    .float 0.2402265070, 0.2402265070, 0.2402265070, 0.2402265070
    .p2align 5
.Lssn_ec3:
    .float 0.0555041087, 0.0555041087, 0.0555041087, 0.0555041087
    .float 0.0555041087, 0.0555041087, 0.0555041087, 0.0555041087
    .p2align 5
.Lssn_2p23:
    .float 8388608.0, 8388608.0, 8388608.0, 8388608.0
    .float 8388608.0, 8388608.0, 8388608.0, 8388608.0
    .p2align 5
.Lssn_b127:
    .long 127, 127, 127, 127, 127, 127, 127, 127
    .p2align 5
.Lssn_tiny:
    .long 0x02081CEA, 0x02081CEA, 0x02081CEA, 0x02081CEA
    .long 0x02081CEA, 0x02081CEA, 0x02081CEA, 0x02081CEA
    .p2align 5
.Lssn_n88:
    .float -87.3, -87.3, -87.3, -87.3, -87.3, -87.3, -87.3, -87.3
    .p2align 5
.Lssn_nh:
    .float -0.5, -0.5, -0.5, -0.5, -0.5, -0.5, -0.5, -0.5
    .p2align 5
.Lssn_sin_c2:
    .float -0.16666667, -0.16666667, -0.16666667, -0.16666667
    .float -0.16666667, -0.16666667, -0.16666667, -0.16666667
    .p2align 5
.Lssn_sin_c4:
    .float 0.0083333333, 0.0083333333, 0.0083333333, 0.0083333333
    .float 0.0083333333, 0.0083333333, 0.0083333333, 0.0083333333
    .p2align 5
.Lssn_rho:
    .float 0.9, 0.9, 0.9, 0.9, 0.9, 0.9, 0.9, 0.9
    .p2align 5
    # parity: знаковый бит на нечётных lane (1,3,5,7)
.Lssn_par:
    .long 0, 0x80000000, 0, 0x80000000, 0, 0x80000000, 0, 0x80000000
    .long 0, 0, 0, 0, 0, 0, 0, 0
    .p2align 5
    # keep-маски хвоста fanout: маска j = j БИТОВЫХ единиц (0xFFFFFFFF!).
    # vandps — побитовое И: 0x3F800000 вычистил бы мантиссу и знак.
    # 9 масок × 32 Б; маска j на офсете j·32; mask[8] — полная (офсет 256).
.Lssn_keep:
    .long 0,0,0,0,0,0,0,0
    .long 0xFFFFFFFF,0,0,0,0,0,0,0
    .long 0xFFFFFFFF,0xFFFFFFFF,0,0,0,0,0,0
    .long 0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0,0,0,0,0
    .long 0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0,0,0,0
    .long 0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0,0,0
    .long 0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0,0
    .long 0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0
    .long 0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF
    .p2align 5
    # jump table 16 примитивов: ОТНОСИТЕЛЬНЫЕ офсеты (PIE-безопасно,
    # разрешаются на этапе ассемблирования — ноль релокаций)
.Lssn_jt:
    .quad .Lssn_p00_identity - .Lssn_jt, .Lssn_p01_relu - .Lssn_jt
    .quad .Lssn_p02_sigmoid - .Lssn_jt, .Lssn_p03_tanh - .Lssn_jt
    .quad .Lssn_p04_expdecay - .Lssn_jt, .Lssn_p05_spike - .Lssn_jt
    .quad .Lssn_p06_gauss - .Lssn_jt, .Lssn_p07_abs - .Lssn_jt
    .quad .Lssn_p08_sign - .Lssn_jt, .Lssn_p09_softplus - .Lssn_jt
    .quad .Lssn_p10_sin - .Lssn_jt, .Lssn_p11_gate - .Lssn_jt
    .quad .Lssn_p12_delay - .Lssn_jt, .Lssn_p13_trace - .Lssn_jt
    .quad .Lssn_p14_clamp - .Lssn_jt, .Lssn_p15_parity - .Lssn_jt

    .text
    # ================================================================
    # float poler_ssn_step_f16(const SsnParams* pp) -> энергия Σ|post'|
    #   rdi = pp
    # ================================================================
    .globl poler_ssn_step_f16
    .p2align 4
poler_ssn_step_f16:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 32                 # [rsp]=dst, [rsp+8]=пакеты, [rsp+16]=маска
                                # хвоста, [rsp+24]=полная маска
    mov rsi, [rdi + 0]          # w
    mov rdx, [rdi + 8]          # pre
    mov rbx, [rdi + 16]         # post
    mov r8,  [rdi + 24]         # func
    mov r9,  [rdi + 32]         # bias
    mov r10, [rdi + 40]         # n_pre
    mov r11, [rdi + 48]         # fanout
    mov r12d, [rdi + 56]        # post_mask
    mov r13d, [rdi + 60]        # post_stride
    vbroadcastss ymm14, [rdi + 64]   # decay
    vbroadcastss ymm13, [rdi + 68]   # gate
    # [rsp+24] = полная маска mask[8] (офсет 8·32 = 256)
    lea rcx, [rip + .Lssn_keep]
    add rcx, 256
    mov [rsp+24], rcx
    # [rsp+16] = маска хвоста: j = ((fanout−1)&7)+1 ∈ 1..8
    # (fanout кратен 8 → j=8 → маска полная — вырождается в no-op)
    lea rcx, [rip + .Lssn_keep]
    mov eax, r11d
    dec eax
    and eax, 7
    inc eax
    shl eax, 5                  # ×32 Б на маску
    add rcx, rax
    mov [rsp+16], rcx
    mov rax, [rsp+24]
    vmovaps ymm8, [rax]         # старт — полная маска
    lea rdi, [rip + .Lssn_jt]   # таблица примитивов
    lea r15, [rip + .Lssn_cont_f16]   # continuation этого ядра
    vxorps ymm15, ymm15, ymm15  # ноль
    vxorps ymm12, ymm12, ymm12  # энергия
    xor r14, r14                # s
    test r10, r10
    jz .Lssn_done_f16
.Lssn_outer_f16:
    cmp r14, r10
    jae .Lssn_done_f16
    # сброс полной маски (хвост предыдущего источника мог её подменить)
    mov rax, [rsp+24]
    vmovaps ymm8, [rax]
    # dst = (s·stride) & mask
    mov eax, r14d
    imul eax, r13d
    and eax, r12d
    mov [rsp], eax
    # адрес примитива источника (func_id на источник):
    # офсет из таблицы + база таблицы
    movzx eax, byte ptr [r8 + r14]
    mov rax, [rdi + rax*8]
    add rax, rdi
    movq xmm9, rax
    # w-пакеты источника (fp16: байты = элементы·2)
    mov rax, r14
    imul rax, r11
    lea rcx, [rsi + rax*2]
    lea rbp, [rdx + r14*4]      # окно pre
    mov rax, r11
    add rax, 7
    shr rax, 3                   # packs = ceil(fanout/8)
    mov [rsp+8], rax
    vxorps ymm2, ymm2, ymm2      # аккумулятор источника
.Lssn_inner_f16:
    mov rax, [rsp+8]
    test rax, rax
    jz .Lssn_iend_f16
    dec rax
    mov [rsp+8], rax
    vmovdqu xmm0, [rcx]          # 8×fp16 (16 байт)
    vcvtph2ps ymm0, xmm0         # -> 8×f32 одной инструкцией (F16C)
    vmovups ymm1, [rbp]          # окно pre
    vmulps ymm0, ymm0, ymm1      # w·pre
    # последняя упаковка источника? -> подмена на маску хвоста
    test rax, rax
    jnz 9f
    mov rax, [rsp+16]
    vmovaps ymm8, [rax]
9:
    movq rax, xmm9
    jmp rax                      # -> примитив -> .Lssn_ret
.Lssn_cont_f16:
    add rcx, 16
    add rbp, 32
    jmp .Lssn_inner_f16
.Lssn_iend_f16:
    vextractf128 xmm1, ymm2, 1
    vaddps xmm2, xmm2, xmm1
    vhaddps xmm2, xmm2, xmm2
    vhaddps xmm2, xmm2, xmm2     # xmm2[0] = Σ acc
    mov eax, [rsp]
    vmovss xmm4, [rbx + rax*4]   # post[dst]
    vaddss xmm2, xmm2, [r9 + rax*4]   # + bias[dst]
    vfmadd231ss xmm2, xmm4, xmm14     # + decay·post
    vmovss [rbx + rax*4], xmm2
    vandps xmm5, xmm2, [rip + .Lssn_abs]
    vaddss xmm12, xmm12, xmm5    # энергия += |post'|
    inc r14
    jmp .Lssn_outer_f16
.Lssn_done_f16:
    vmovaps xmm0, xmm12
    vzeroupper
    add rsp, 32
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret

    # ================================================================
    # float poler_ssn_step_i8(const SsnParamsI8* pp) -> энергия Σ|post'|
    # ================================================================
    .globl poler_ssn_step_i8
    .p2align 4
poler_ssn_step_i8:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 32
    mov rsi, [rdi + 0]          # w8
    mov rdx, [rdi + 8]          # pre
    mov rbx, [rdi + 16]         # post
    mov r8,  [rdi + 24]         # func
    mov r9,  [rdi + 32]         # bias
    mov r10, [rdi + 40]         # n_pre
    mov r11, [rdi + 48]         # fanout
    mov r12d, [rdi + 56]        # post_mask
    mov r13d, [rdi + 60]        # post_stride
    vbroadcastss ymm14, [rdi + 64]   # decay
    vbroadcastss ymm13, [rdi + 68]   # gate
    vbroadcastss ymm11, [rdi + 72]   # scale
    # маски: [rsp+24]=полная, [rsp+16]=хвост
    lea rcx, [rip + .Lssn_keep]
    add rcx, 256
    mov [rsp+24], rcx
    lea rcx, [rip + .Lssn_keep]
    mov eax, r11d
    dec eax
    and eax, 7
    inc eax
    shl eax, 5
    add rcx, rax
    mov [rsp+16], rcx
    mov rax, [rsp+24]
    vmovaps ymm8, [rax]
    lea rdi, [rip + .Lssn_jt]
    lea r15, [rip + .Lssn_cont_i8]
    vxorps ymm15, ymm15, ymm15
    vxorps ymm12, ymm12, ymm12
    xor r14, r14
    test r10, r10
    jz .Lssn_done_i8
.Lssn_outer_i8:
    cmp r14, r10
    jae .Lssn_done_i8
    mov rax, [rsp+24]
    vmovaps ymm8, [rax]          # сброс полной маски
    mov eax, r14d
    imul eax, r13d
    and eax, r12d
    mov [rsp], eax
    movzx eax, byte ptr [r8 + r14]
    mov rax, [rdi + rax*8]
    add rax, rdi
    movq xmm9, rax
    mov rax, r14
    imul rax, r11
    lea rcx, [rsi + rax]        # i8: 1 байт на синапс
    lea rbp, [rdx + r14*4]
    mov rax, r11
    add rax, 7
    shr rax, 3
    mov [rsp+8], rax
    vxorps ymm2, ymm2, ymm2
.Lssn_inner_i8:
    mov rax, [rsp+8]
    test rax, rax
    jz .Lssn_iend_i8
    dec rax
    mov [rsp+8], rax
    vmovq xmm0, [rcx]           # 8×i8 (8 байт)
    vpmovsxbd ymm0, xmm0        # -> 8×i32
    vcvtdq2ps ymm0, ymm0        # -> 8×f32
    vmulps ymm0, ymm0, ymm11    # ×scale
    vmovups ymm1, [rbp]
    vmulps ymm0, ymm0, ymm1
    # последняя упаковка? -> маска хвоста
    test rax, rax
    jnz 9f
    mov rax, [rsp+16]
    vmovaps ymm8, [rax]
9:
    movq rax, xmm9
    jmp rax
.Lssn_cont_i8:
    add rcx, 8
    add rbp, 32
    jmp .Lssn_inner_i8
.Lssn_iend_i8:
    vextractf128 xmm1, ymm2, 1
    vaddps xmm2, xmm2, xmm1
    vhaddps xmm2, xmm2, xmm2
    vhaddps xmm2, xmm2, xmm2
    mov eax, [rsp]
    vmovss xmm4, [rbx + rax*4]
    vaddss xmm2, xmm2, [r9 + rax*4]
    vfmadd231ss xmm2, xmm4, xmm14
    vmovss [rbx + rax*4], xmm2
    vandps xmm5, xmm2, [rip + .Lssn_abs]
    vaddss xmm12, xmm12, xmm5
    inc r14
    jmp .Lssn_outer_i8
.Lssn_done_i8:
    vmovaps xmm0, xmm12
    vzeroupper
    add rsp, 32
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret

    # ================================================================
    # 16 примитивов ObservationCircuit.
    # Контракт: вход ymm0 = 8 произведений w·pre; выход ymm0.
    # Скретч: ymm0, ymm1, ymm4..ymm7. ЗАПРЕЩЕНО трогать:
    # ymm2 (acc), ymm8 (keep), ymm9 (адрес), ymm11 (scale),
    # ymm12 (энергия), ymm13 (gate), ymm14 (decay), ymm15 (0), r15.
    # ================================================================
    .p2align 4
.Lssn_ret:
    vandps ymm0, ymm0, ymm8     # keep-маска хвоста fanout
    vaddps ymm2, ymm2, ymm0     # аккумулятор источника
    jmp r15                      # continuation ядра (f16/i8)

.Lssn_p00_identity:
    jmp .Lssn_ret
.Lssn_p01_relu:
    vmaxps ymm0, ymm0, ymm15
    jmp .Lssn_ret
.Lssn_p02_sigmoid:               # 0.5·tanh(0.5x) + 0.5
    vmulps ymm0, ymm0, [rip + .Lssn_half]
    vmaxps ymm0, ymm0, [rip + .Lssn_n4]
    vminps ymm0, ymm0, [rip + .Lssn_p4]
    vmulps ymm4, ymm0, ymm0       # x²
    vaddps ymm5, ymm4, [rip + .Lssn_c27]
    vmulps ymm5, ymm5, ymm0       # x(27+x²)
    vmulps ymm4, ymm4, [rip + .Lssn_c9]
    vaddps ymm4, ymm4, [rip + .Lssn_c27]   # 27+9x²
    vdivps ymm0, ymm5, ymm4       # tanh(0.5x)
    vmulps ymm0, ymm0, [rip + .Lssn_half]
    vaddps ymm0, ymm0, [rip + .Lssn_half]
    jmp .Lssn_ret
.Lssn_p03_tanh:                  # рационал x(27+x²)/(27+9x²), |x|<=4
    vmaxps ymm0, ymm0, [rip + .Lssn_n4]
    vminps ymm0, ymm0, [rip + .Lssn_p4]
    vmulps ymm4, ymm0, ymm0
    vaddps ymm5, ymm4, [rip + .Lssn_c27]
    vmulps ymm5, ymm5, ymm0
    vmulps ymm4, ymm4, [rip + .Lssn_c9]
    vaddps ymm4, ymm4, [rip + .Lssn_c27]
    vdivps ymm0, ymm5, ymm4
    jmp .Lssn_ret
.Lssn_p04_expdecay:              # e^x для x<=0
    vmaxps ymm0, ymm0, [rip + .Lssn_n88]
    vminps ymm0, ymm0, ymm15
    jmp .Lssn_exp_body
.Lssn_p05_spike:                 # ТРОИЧНЫЙ спайк поляризации (v0.88):
    # x > +0.5 → ⊕ +1 (деполяризация)
    # x < −0.5 → ⊖ −1 (гиперполяризация)
    # мёртвая зона → ⊙ 0 (рефрактерный стазис)
    # СКРЕТЧ-контракт: только ymm0/ymm1/ymm4..ymm7 (ymm2 = acc!)
    vcmpgtps ymm1, ymm0, [rip + .Lssn_thr]
    vandps ymm4, ymm1, [rip + .Lssn_one]
    vcmpltps ymm5, ymm0, [rip + .Lssn_nthr]
    vandps ymm6, ymm5, [rip + .Lssn_mone]
    vorps ymm0, ymm4, ymm6
    jmp .Lssn_ret
.Lssn_p06_gauss:                 # e^(-0.5x²)
    vmulps ymm0, ymm0, ymm0
    vmulps ymm0, ymm0, [rip + .Lssn_nh]
    vmaxps ymm0, ymm0, [rip + .Lssn_n88]
    jmp .Lssn_exp_body
.Lssn_p07_abs:
    vandps ymm0, ymm0, [rip + .Lssn_abs]
    jmp .Lssn_ret
.Lssn_p08_sign:                  # copysign(1, x)
    vandps ymm1, ymm0, [rip + .Lssn_sb]
    vorps ymm0, ymm1, [rip + .Lssn_one]
    jmp .Lssn_ret
.Lssn_p09_softplus:              # x·σ(x) — мягкий порог без log
    vmovaps ymm6, ymm0           # сохранить x
    vmulps ymm0, ymm0, [rip + .Lssn_half]
    vmaxps ymm0, ymm0, [rip + .Lssn_n4]
    vminps ymm0, ymm0, [rip + .Lssn_p4]
    vmulps ymm4, ymm0, ymm0
    vaddps ymm5, ymm4, [rip + .Lssn_c27]
    vmulps ymm5, ymm5, ymm0
    vmulps ymm4, ymm4, [rip + .Lssn_c9]
    vaddps ymm4, ymm4, [rip + .Lssn_c27]
    vdivps ymm0, ymm5, ymm4
    vmulps ymm0, ymm0, [rip + .Lssn_half]
    vaddps ymm0, ymm0, [rip + .Lssn_half]  # σ(x)
    vmulps ymm0, ymm0, ymm6      # x·σ(x)
    jmp .Lssn_ret
.Lssn_p10_sin:                   # x(1-x²/6+x⁴/120), |x|<=π/2
    vmaxps ymm0, ymm0, [rip + .Lssn_nhpi]
    vminps ymm0, ymm0, [rip + .Lssn_hpi]
    vmulps ymm4, ymm0, ymm0       # t=x²
    vmulps ymm5, ymm4, [rip + .Lssn_sin_c4]
    vaddps ymm5, ymm5, [rip + .Lssn_sin_c2]       # c2+t·c4
    vfmadd213ps ymm5, ymm4, [rip + .Lssn_one]     # 1+t·(c2+t·c4)
    vmulps ymm0, ymm0, ymm5
    jmp .Lssn_ret
.Lssn_p11_gate:                  # x·gate (gate = pre[s])
    vmulps ymm0, ymm0, ymm13
    jmp .Lssn_ret
.Lssn_p12_delay:                 # сдвиг фазы на 1 такт в окне из 8
    vpermilps ymm0, ymm0, 0x93
    jmp .Lssn_ret
.Lssn_p13_trace:                 # утечка eligibility ρ=0.9
    vmulps ymm0, ymm0, [rip + .Lssn_rho]
    jmp .Lssn_ret
.Lssn_p14_clamp:                 # насыщение ±4
    vmaxps ymm0, ymm0, [rip + .Lssn_n4]
    vminps ymm0, ymm0, [rip + .Lssn_p4]
    jmp .Lssn_ret
.Lssn_p15_parity:                # знаковая модуляция нечётных lane
    vpxor ymm0, ymm0, [rip + .Lssn_par]
    jmp .Lssn_ret

    # тело быстрой экспоненты: x ∈ [-87.3, 0] в ymm0
.Lssn_exp_body:
    vmulps ymm0, ymm0, [rip + .Lssn_log2e]   # y = x·log2e
    vroundps ymm1, ymm0, 9                    # i = floor(y)
    vsubps ymm0, ymm0, ymm1                   # f ∈ [0,1)
    vmulps ymm4, ymm0, [rip + .Lssn_ec3]
    vaddps ymm4, ymm4, [rip + .Lssn_ec2]
    vmulps ymm4, ymm4, ymm0
    vaddps ymm4, ymm4, [rip + .Lssn_ec1]      # c1+f·(c2+f·c3)
    vmulps ymm4, ymm4, ymm0                   # 2^f − 1
    vmulps ymm4, ymm4, [rip + .Lssn_2p23]     # мантисса
    vcvttps2dq ymm4, ymm4
    vcvttps2dq ymm1, ymm1
    vpaddd ymm1, ymm1, [rip + .Lssn_b127]     # i+127
    vpslld ymm1, ymm1, 23                     # экспонента
    vpaddd ymm0, ymm1, ymm4                   # биты e^x
    vmaxps ymm0, ymm0, [rip + .Lssn_tiny]
    jmp .Lssn_ret
    "#
}

extern "sysv64" {
    fn poler_ssn_step_f16(pp: *const SsnParams) -> f32;
    fn poler_ssn_step_i8(pp: *const SsnParamsI8) -> f32;
}

/// Плотность хранения весов.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Density {
    /// fp16 (binary16): 2.0 Б/синапс payload.
    F16,
    /// i8 × scale: 1.0 Б/синапс payload — режим «85 МБ мозг».
    I8,
}

/// Поле синапсов: SoA-хранилище с хэш-адресацией пост-нейронов.
///
/// `dst(s) = (s·α) & (n_post−1)` — виртуальный коннектом (Кнутовский
/// мультипликативный хэш): плотность payload без CSR-массива dst.
/// Импорт реального FlyWire CSR — роадмап v0.88 (docs/VAULT_ASM.md).
pub struct SynapseField {
    pub n_pre: usize,
    /// Степень двойки! (битовая маска dst).
    pub n_post: usize,
    pub fanout: usize,
    /// Веса fp16 (с паддингом — см. контракт в доке модуля).
    pub w16: Vec<u16>,
    /// Веса i8 (с паддингом).
    pub w8: Vec<i8>,
    /// f32 = w8 · w8_scale.
    pub w8_scale: f32,
    /// func_id на источник (0..16).
    pub func: Vec<u8>,
    /// Смещения на пост-нейрон.
    pub bias: Vec<f32>,
    /// α хэша dst.
    pub post_stride: u32,
}

#[inline]
fn ssn_ready() -> bool {
    let c = crate::asm::caps();
    c.avx2 && c.fma && c.f16c
}

fn xs64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *state = x;
    x.wrapping_mul(0x2545F4914F6CDD1D)
}

impl SynapseField {
    /// Синтетическое поле (детерминированный xorshift64).
    ///
    /// `n_post` подбирается степенью двойки ≈ n_pre/8 (минимум 64).
    pub fn synthetic(n_pre: usize, fanout: usize, seed: u64) -> Self {
        let n_post = (n_pre / 8).next_power_of_two().max(64);
        let mut s = seed | 1;
        let total = n_pre * fanout;
        // паддинг: последняя 8-lane-упаковка последнего источника читает
        // до 7 элементов за границей — округляем вверх + запас 16 Б
        let pad16 = (total + 7) / 8 * 8 + 8;
        let mut w16 = Vec::with_capacity(pad16);
        let mut w8 = Vec::with_capacity(pad16);
        for _ in 0..pad16 {
            let r = (xs64(&mut s) >> 33) as i32;
            let w = (r % 200) as f32 / 100.0; // [-1, 1)
            w16.push(f32_to_f16(w));
            w8.push((w * 64.0).round().clamp(-127.0, 127.0) as i8);
        }
        let func = (0..n_pre)
            .map(|i| ((xs64(&mut s) >> 33) as usize % FUNC_COUNT) as u8)
            .collect();
        let bias = (0..n_post)
            .map(|_| ((xs64(&mut s) >> 33) as i32 % 20) as f32 / 100.0)
            .collect();
        Self {
            n_pre,
            n_post,
            fanout,
            w16,
            w8,
            w8_scale: 1.0 / 64.0,
            func,
            bias,
            post_stride: 2654435761, // Кнутовский мультипликативный хэш
        }
    }

    /// Требуемая длина pre-массива (контракт паддинга).
    pub fn pad_pre_len(&self) -> usize {
        self.n_pre + self.fanout + 8
    }

    /// Payload-байт на синапс в выбранной плотности.
    pub fn bytes_per_synapse(&self, d: Density) -> f32 {
        match d {
            Density::F16 => 2.0,
            Density::I8 => 1.0,
        }
    }

    /// Один шаг SSN в режиме fp16. Возвращает энергию Σ|post'|.
    pub fn step_f16(&self, pre: &[f32], post: &mut [f32], decay: f32, gate: f32) -> f32 {
        debug_assert!(self.w16.len() >= self.n_pre * self.fanout + 8);
        debug_assert!(pre.len() >= self.pad_pre_len());
        debug_assert!(post.len() >= self.n_post);
        if ssn_ready() {
            let pp = SsnParams {
                w: self.w16.as_ptr(),
                pre: pre.as_ptr(),
                post: post.as_mut_ptr(),
                func: self.func.as_ptr(),
                bias: self.bias.as_ptr(),
                n_pre: self.n_pre,
                fanout: self.fanout,
                post_mask: (self.n_post - 1) as u32,
                post_stride: self.post_stride,
                decay,
                gate,
            };
            unsafe { poler_ssn_step_f16(&pp) }
        } else {
            self.step_scalar(Density::F16, pre, post, decay, gate)
        }
    }

    /// Один шаг SSN в режиме i8 (1 Б/синапс).
    pub fn step_i8(&self, pre: &[f32], post: &mut [f32], decay: f32, gate: f32) -> f32 {
        debug_assert!(self.w8.len() >= self.n_pre * self.fanout + 8);
        debug_assert!(pre.len() >= self.pad_pre_len());
        debug_assert!(post.len() >= self.n_post);
        if ssn_ready() {
            let pp = SsnParamsI8 {
                w: self.w8.as_ptr(),
                pre: pre.as_ptr(),
                post: post.as_mut_ptr(),
                func: self.func.as_ptr(),
                bias: self.bias.as_ptr(),
                n_pre: self.n_pre,
                fanout: self.fanout,
                post_mask: (self.n_post - 1) as u32,
                post_stride: self.post_stride,
                decay,
                gate,
                scale: self.w8_scale,
            };
            unsafe { poler_ssn_step_i8(&pp) }
        } else {
            self.step_scalar(Density::I8, pre, post, decay, gate)
        }
    }

    /// Скалярный эталон (та же семантика, что у микроядер).
    ///
    /// Зеркалит пачечную структуру asm: примитив применяется к 8-lane
    /// упаковке, маска хвоста — ПОСЛЕ примитива. Для `delay` это
    /// принципиально: ротация окна выталкивает новейшие сэмплы
    /// хвостовой упаковки за границу маски (каузальная задержка
    /// с потерей — документированная семантика vpermilps 0x93).
    pub fn step_scalar(
        &self,
        density: Density,
        pre: &[f32],
        post: &mut [f32],
        decay: f32,
        gate: f32,
    ) -> f32 {
        let mut energy = 0.0f32;
        let packs = (self.fanout + 7) / 8;
        for s in 0..self.n_pre {
            let dst = ((s as u32).wrapping_mul(self.post_stride) as usize) & (self.n_post - 1);
            let mut acc = 0.0f32;
            for p in 0..packs {
                let base = s * self.fanout + p * 8;
                let n_real = (self.fanout - p * 8).min(8);
                // 8 lane упаковки (пад-байты за fanout — РЕАЛЬНЫЕ данные
                // следующего сегмента, как читает и микроядро)
                let mut lanes = [0.0f32; 8];
                for k in 0..8 {
                    let w = match density {
                        Density::F16 => f16_to_f32(self.w16[base + k]),
                        Density::I8 => self.w8[base + k] as f32 * self.w8_scale,
                    };
                    lanes[k] = w * pre[s + p * 8 + k];
                }
                if self.func[s] == FUNC_DELAY {
                    // vpermilps 0x93: ротация вправо на 1 в каждой
                    // 128-бит половине ymm — два независимых окна по 4
                    let rot = [
                        lanes[3], lanes[0], lanes[1], lanes[2],
                        lanes[7], lanes[4], lanes[5], lanes[6],
                    ];
                    for k in 0..n_real {
                        acc += rot[k];
                    }
                } else {
                    for k in 0..n_real {
                        acc += apply_func_scalar(lanes[k], self.func[s], gate, k);
                    }
                }
            }
            let p0 = post[dst];
            post[dst] = decay * p0 + acc + self.bias[dst];
            energy += post[dst].abs();
        }
        energy
    }
}

/// Скалярное зеркало примитивов (формулы идентичны ассемблерным).
pub fn apply_func_scalar(x: f32, func: u8, gate: f32, lane: usize) -> f32 {
    let tanh_r = |v: f32| {
        let v = v.clamp(-4.0, 4.0);
        v * (27.0 + v * v) / (27.0 + 9.0 * v * v)
    };
    let sigmoid_r = |v: f32| 0.5 * tanh_r(0.5 * v) + 0.5;
    let fast_exp = |v: f32| {
        // v ∈ [-87.3, 0] — мантисса-полином по битам float
        let y = v * 1.4426950409;
        let i = y.floor();
        let f = y - i;
        let m = f * (0.6931471806 + f * (0.2402265070 + f * 0.0555041087));
        let bits = (((i as i64 + 127) as u64) << 23).wrapping_add((m * 8388608.0) as u64);
        f32::from_bits(bits as u32).max(1e-37)
    };
    match func {
        FUNC_RELU => x.max(0.0),
        FUNC_SIGMOID => sigmoid_r(x),
        FUNC_TANH => tanh_r(x),
        FUNC_EXPDECAY => fast_exp(x.min(0.0).max(-87.3)),
        FUNC_SPIKE => {
            // трит поляризации: ⊕/⊙/⊖ (зеркало .Lssn_p05_spike)
            if x > 0.5 {
                1.0
            } else if x < -0.5 {
                -1.0
            } else {
                0.0
            }
        }
        FUNC_GAUSS => fast_exp((-0.5 * x * x).max(-87.3)),
        FUNC_ABS => x.abs(),
        FUNC_SIGN => 1.0f32.copysign(x),
        FUNC_SOFTPLUS => x * sigmoid_r(x),
        FUNC_SIN => {
            let v = x.clamp(-1.5707963268, 1.5707963268);
            v * (1.0 + v * v * (-0.16666667 + v * v * 0.0083333333))
        }
        FUNC_GATE => x * gate,
        // векторный rotate lane сохраняет СУММУ окна — скалярно тождество
        FUNC_DELAY => x,
        FUNC_TRACE => 0.9 * x,
        FUNC_CLAMP => x.clamp(-4.0, 4.0),
        FUNC_PARITY => {
            if lane % 2 == 1 {
                -x
            } else {
                x
            }
        }
        _ => x,
    }
}

/// f32 → fp16 (round-to-nearest-even), чистые биты.
pub fn f32_to_f16(v: f32) -> u16 {
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp_f32 = ((bits >> 23) & 0xFF) as i32;
    let man_f32 = bits & 0x007F_FFFF;
    if exp_f32 == 0xFF {
        let mant = if man_f32 != 0 { 0x0200 } else { 0 };
        return sign | 0x7C00 | mant;
    }
    let e = exp_f32 - 127;
    if e > 15 {
        return sign | 0x7C00; // → Inf
    }
    if e >= -14 {
        // нормальный fp16
        let m = man_f32 >> 13;
        let round = (man_f32 >> 12) & 1;
        let sticky = man_f32 & 0xFFF;
        let mut h = (((e + 15) as u32) << 10) | m;
        if round == 1 && (sticky != 0 || (h & 1) == 1) {
            h += 1;
        }
        return sign | h as u16;
    }
    if e < -25 {
        return sign; // → ±0
    }
    // денормаль fp16
    let imp = man_f32 | 0x0080_0000;
    let shift = (-(e + 1)) as u32; // 14..24
    let m16 = imp >> shift;
    let rem = imp & ((1u32 << shift) - 1);
    let half = 1u32 << (shift - 1);
    let mut m = m16;
    if rem > half || (rem == half && (m16 & 1) == 1) {
        m += 1; // раунд вверх может дать 1024 = минимальную нормаль
    }
    sign | m as u16
}

/// fp16 → f32 (денормали нормализуются).
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h & 0x8000) as u32) << 16;
    let exp = ((h >> 10) & 0x1F) as u32;
    let man = (h & 0x03FF) as u32;
    let bits = match exp {
        0 => {
            if man == 0 {
                sign
            } else {
                let mut e = -14i32;
                let mut m = man;
                while m & 0x0400 == 0 {
                    m <<= 1;
                    e -= 1;
                }
                m &= 0x03FF;
                sign | (((e + 127) as u32) << 23) | (m << 13)
            }
        }
        31 => sign | 0x7F80_0000 | (man << 13),
        _ => sign | ((exp + 112) << 23) | (man << 13),
    };
    f32::from_bits(bits)
}

// ------------------------------ тесты ------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pre(f: &SynapseField, seed: u64) -> Vec<f32> {
        let mut s = seed | 1;
        (0..f.pad_pre_len())
            .map(|_| ((xs64(&mut s) >> 33) as i32 % 100) as f32 / 100.0)
            .collect()
    }

    fn compare(f: &SynapseField, density: Density, tol: f32) {
        let pre = make_pre(f, 77);
        let mut post_a = vec![0.0f32; f.n_post];
        let mut post_r = vec![0.0f32; f.n_post];
        let (ea, er, label) = match density {
            Density::F16 => (
                f.step_f16(&pre, &mut post_a, 0.9, 0.5),
                f.step_scalar(Density::F16, &pre, &mut post_r, 0.9, 0.5),
                "f16",
            ),
            Density::I8 => (
                f.step_i8(&pre, &mut post_a, 0.9, 0.5),
                f.step_scalar(Density::I8, &pre, &mut post_r, 0.9, 0.5),
                "i8",
            ),
        };
        assert!(
            (ea - er).abs() / er.abs().max(1e-6) < tol,
            "{label}: энергия asm={ea} vs scalar={er}"
        );
        let mut worst = 0.0f32;
        for i in 0..f.n_post {
            worst = worst.max((post_a[i] - post_r[i]).abs());
        }
        assert!(worst < tol, "{label}: худший пост {worst}");
    }

    #[test]
    fn ssn_f16_matches_scalar() {
        // fanout=10: хвост из 2 lane маскируется; 8: ровно; 13: хвост 5
        for fanout in [8usize, 10, 13] {
            let f = SynapseField::synthetic(200, fanout, fanout as u64 * 31 + 5);
            compare(&f, Density::F16, 2e-3);
        }
    }

    #[test]
    fn ssn_i8_matches_scalar() {
        for fanout in [8usize, 10, 13] {
            let f = SynapseField::synthetic(200, fanout, fanout as u64 * 57 + 11);
            compare(&f, Density::I8, 2e-3);
        }
    }

    #[test]
    fn ssn_all_16_primitives_exercised() {
        // поле, где каждый источник — свой func_id (n_pre ≥ 16)
        let mut f = SynapseField::synthetic(64, 8, 999);
        for (i, fu) in f.func.iter_mut().enumerate() {
            *fu = (i % FUNC_COUNT) as u8;
        }
        let pre = make_pre(&f, 4242);
        let mut post_a = vec![0.0f32; f.n_post];
        let mut post_r = vec![0.0f32; f.n_post];
        let ea = f.step_f16(&pre, &mut post_a, 0.95, 0.7);
        let er = f.step_scalar(Density::F16, &pre, &mut post_r, 0.95, 0.7);
        assert!(
            (ea - er).abs() / er.abs().max(1e-6) < 2e-3,
            "все 16 примитивов: {ea} vs {er}"
        );
    }

    #[test]
    fn ssn_energy_is_finite_and_dense() {
        let f = SynapseField::synthetic(10_000, 16, 2026);
        let pre = make_pre(&f, 13);
        let mut post = vec![0.0f32; f.n_post];
        let mut e = 0.0f32;
        for _ in 0..8 {
            e = f.step_f16(&pre, &mut post, 0.9, 0.5);
        }
        assert!(e.is_finite() && e > 0.0, "энергия {e}");
        assert!(post.iter().all(|x| x.is_finite()));
        // плотность payload
        assert_eq!(f.bytes_per_synapse(Density::F16), 2.0);
        assert_eq!(f.bytes_per_synapse(Density::I8), 1.0);
    }

    #[test]
    fn ssn_ternary_spike_semantics() {
        // трит поляризации: мёртвая зона [−0.5, 0.5] ⊙, пороги ⊕/⊖
        let cases = [
            (2.0f32, 1.0f32), (0.6, 1.0), (0.5, 0.0), (0.1, 0.0), (0.0, 0.0),
            (-0.1, 0.0), (-0.5, 0.0), (-0.6, -1.0), (-2.0, -1.0),
        ];
        for &(x, e) in &cases {
            assert_eq!(apply_func_scalar(x, FUNC_SPIKE, 0.5, 0), e, "spike({x})");
        }
        // и на ассемблерном пути: поле только из spike-источников
        let mut f = SynapseField::synthetic(64, 8, 31337);
        for fu in f.func.iter_mut() {
            *fu = FUNC_SPIKE;
        }
        let pre: Vec<f32> = (0..f.pad_pre_len())
            .map(|i| ((i % 7) as f32) * 0.3 - 0.9) // −0.9..0.9 через зону
            .collect();
        let mut post_a = vec![0.0f32; f.n_post];
        let mut post_r = vec![0.0f32; f.n_post];
        f.step_f16(&pre, &mut post_a, 0.9, 0.5);
        f.step_scalar(Density::F16, &pre, &mut post_r, 0.9, 0.5);
        let mut worst = 0.0f32;
        for i in 0..f.n_post {
            worst = worst.max((post_a[i] - post_r[i]).abs());
        }
        assert!(worst < 1e-5, "asm vs scalar spike: {worst}");
    }

    #[test]
    fn fp16_roundtrip_specials() {
        // раунд-трип точных значений + спец-случаи
        for v in [0.0f32, 1.0, -1.0, 0.5, -0.5, 2.0, 65504.0, 0.00006103515625] {
            assert_eq!(f16_to_f32(f32_to_f16(v)), v, "раундтрип {v}");
        }
        assert_eq!(f16_to_f32(0x0000), 0.0);
        assert_eq!(f16_to_f32(0x8000), -0.0);
        assert!(f16_to_f32(0x7C00).is_infinite());
        assert!(f16_to_f32(0x7E00).is_nan());
        assert_eq!(f16_to_f32(0x0400), 6.103515625e-05); // мин. нормаль
        assert_eq!(f16_to_f32(0x03FF), 6.097555160522461e-05); // макс. денормаль
        assert_eq!(f32_to_f16(1e30), 0x7C00); // переполнение -> inf
    }
}
