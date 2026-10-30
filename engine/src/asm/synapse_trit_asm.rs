//! SYNAPSE-TRIT — слияние тритов, кутритов и нейромодулей (v0.90.0).
//!
//! Классические счётчики дорогие — синус ~200 FLOPs (Тейлор/Чебышёв/libm),
//! нейрон ~1000 FLOPs (матрицы, активации). Троичная физика процессора
//! схлопывает оба счётчика:
//!
//! | Операция | Классика | SYNAPSE-TRIT | Механизм |
//! |----------|----------|--------------|----------|
//! | sin/cos  | ~200 FLOPs | ~1 такт (аморт.) | ℤ₁₂-ротор: LUT в регистрах, `vpermd`+`vblendvps` |
//! | синапс w·x | ~3 FLOPs | 0 умножений | `vpsignb` — умножение ЗНАКОМ |
//! | нейрон   | ~1000 FLOPs | ~10 вектор-инстр. на 32 синапса | unpack-LUT + vpsignb + vpmaddwd |
//!
//! ## ℤ₁₂-ротор (Примитив 10 SSN — гармонический синтез)
//!
//! Фаза — целое Q24: 2²⁴ = 1 шаг решётки ℤ₁₂ (30°). Поворот ω (120°)
//! = 4 шага: add + cmov — БЕЗ деления, БИТ-ТОЧНО mod 12 (инвариант
//! канонического k ∈ 0..11; ГРАБЛЯ: маска `&15` даёт mod 16 — период
//! 12 решётки требует коррекции, иначе ω³ ломается на k ≥ 4).
//! Чтение (sin, cos) — канонизация на чтении: записи LUT 12..15
//! повторяют 0..3. Полутаблицы живут В РЕГИСТРАХ (ymm) — синус =
//! `vpermd`-перестановка, не память. Точные узлы решётки:
//! sin ∈ {0, ±2²³ (1/2), ±2²⁴ (1)} — рациональные бит-точны;
//! 1+ω+ω² = 2²⁴−2²³−2²³ = 0 ТОЧНО (пары ±14529495 аннигилируют).
//!
//! ## «Это деление, а не умножение» (урок v0.89 АЗУ)
//!
//! Каждое умножение на обратное — замаскированное деление. Правила
//! SYNAPSE-TRIT: деление на степень двойки — ТОЧНЫЙ сдвиг (`sar`);
//! затухание пост-нейрона — Q8-дробь decay_q8/256: `(p·224) >> 8` =
//! p·7/8 БЕЗ округления. Целочисленный Q8-распад достигает АБСОЛЮТНОГО
//! НУЛЯ за конечные шаги (p ≤ 2 → 0), тогда как f32 × 0.875 застывает
//! в субнормали навсегда. Прочие деления — только через АЗУ
//! (`div_safe`, `cmp_canonical`, [N:D], ℝP¹).
//!
//! ## Плотность
//!
//! * dense (mac32): 1 Б/синапс — vpsignb-стрим, максимум скорости;
//! * packed5: **0.2 Б/синапс** — 50M синапсов FlyWire = 10 МБ, весь
//!   мозг в кэше L3. Распаковка — LUT-243 (байт → 5 трит-байт), не
//!   magic-÷3: таблица 243·8 Б живёт в L1.
//!
//! ## Константы — Калькулятор Всего (директива: ноль Python)
//!
//! ```text
//! poler-engine --exec 'calc sin(2*pi/6)*2^24'  → 14529495.26 → 14529495
//! poler-engine --exec 'calc sin(3*pi/6)*2^24'  → 16777216.0  → 2^24 (точно)
//! poler-engine --exec 'calc cos(1*pi/6)*2^24'  → 14529495.26 → 14529495
//! poler-engine --exec 'calc (e^(i*2*pi/3))^3'  → 1 − 2.4e-16i (фаз. ар. — точно)
//! ```

use std::arch::global_asm;

global_asm! {
    r#"
    .text
    .p2align 5
    # ℤ₁₂ синус-решётка Q24, 16 записей (12..15 = 0..3 — mod 12 бесплатно).
    # Точно: 0, ±8388608 (1/2), ±16777216 (1); округлено: ±14529495 (√3/2).
.Lz3_sin16:
    .long 0, 8388608, 14529495, 16777216, 14529495, 8388608, 0
    .long -8388608, -14529495, -16777216, -14529495, -8388608
    .long 0, 8388608, 14529495, 16777216
    .p2align 5
.Lz3_cos16:
    .long 16777216, 14529495, 8388608, 0, -8388608, -14529495, -16777216
    .long -14529495, -8388608, 0, 8388608, 14529495
    .long 16777216, 14529495, 8388608, 0
    .p2align 4
    # маска k&7 (8 lane) для vpermd-индексации полутаблиц
.Lz3_m7:
    .long 7, 7, 7, 7, 7, 7, 7, 7
    .p2align 4
    # 8×i16 единиц (vpmaddwd-свёртка)
.Lst_one16:
    .word 1, 1, 1, 1, 1, 1, 1, 1

    .text
    # ================================================================
    # void poler_z3_sincos_q24(const i32* phase, i32* sout, i32* cout,
    #                          size_t n)
    #   rdi=phase, rsi=sout, rdx=cout, rcx=n.
    #   ℤ₁₂-ротор: 8 фаз за итерацию, БЕЗ обращений к памяти за
    #   значениями — полутаблицы sin/cos в регистрах ymm.
    #     k   = phase >> 24          (шаг 30°)
    #     lo  = k & 7                (vpermd-индекс в полутаблице)
    #     sel = k << 28              (бит 3 → знаковый бит dword)
    #   Хвост <8 — скалярно. vzeroupper на выходе.
    # ================================================================
    .globl poler_z3_sincos_q24
    .p2align 4
poler_z3_sincos_q24:
    lea r9,  [rip + .Lz3_sin16]
    lea r10, [rip + .Lz3_cos16]
    vmovdqa ymm5, [r9]            # sin[0..7]
    vmovdqa ymm6, [r9 + 32]       # sin[8..15]
    vmovdqa ymm7, [r10]           # cos[0..7]
    vmovdqa ymm8, [r10 + 32]      # cos[8..15]
    xor r11, r11
    test rcx, rcx
    jz .Lz3_fin
.Lz3_lp:
    mov rax, rcx
    sub rax, r11
    cmp rax, 8
    jb .Lz3_tail
    vmovdqu ymm0, [rdi + r11*4]   # 8 фаз Q24
    vpsrld  ymm1, ymm0, 24        # k
    vpand   ymm2, ymm1, [rip + .Lz3_m7]
    vpslld  ymm3, ymm1, 28        # бит 3 k → знаковый бит dword
    vpermd  ymm9,  ymm2, ymm5     # sin[lo]
    vpermd  ymm10, ymm2, ymm6     # sin[8+lo]
    vblendvps ymm11, ymm9, ymm10, ymm3
    vmovdqu [rsi + r11*4], ymm11
    vpermd  ymm9,  ymm2, ymm7     # cos[lo]
    vpermd  ymm10, ymm2, ymm8     # cos[8+lo]
    vblendvps ymm12, ymm9, ymm10, ymm3
    vmovdqu [rdx + r11*4], ymm12
    add r11, 8
    jmp .Lz3_lp
.Lz3_tail:
    cmp r11, rcx
    jae .Lz3_fin
    mov eax, [rdi + r11*4]
    shr eax, 24                   # логич. сдвиг: фаза u32-модулярна
    and eax, 15
    mov r8d, [r9 + rax*4]
    mov [rsi + r11*4], r8d
    mov r8d, [r10 + rax*4]
    mov [rdx + r11*4], r8d
    inc r11
    jmp .Lz3_tail
.Lz3_fin:
    vzeroupper
    ret

    # ================================================================
    # i64 poler_trit_mac32(const i8* w, const i8* x, size_t n)
    #   rdi=w, rsi=x, rdx=n. Плотный трит-MAC БЕЗ умножителя:
    #   32 трит-произведения за итерацию. Байтовый аккумулятор:
    #   произведения ∈ {{−1,0,1}}, сумма lane за 3 чанка ∈ [−96, 96] —
    #   влезает в i8; свёртка vpmovsxbw+vpmaddwd раз в 3 чанка.
    #   Возвращает Σ wᵢxᵢ (ТОЧНО — целые, ни одного округления).
    # ================================================================
    .globl poler_trit_mac32
    .p2align 4
poler_trit_mac32:
    xor r8, r8                    # скалярный хвост-аккумулятор
    xor r11, r11                  # i
    xor r10d, r10d                # чанков до свёртки (0..2)
    vpxor xmm5, xmm5, xmm5        # 4×i32 векторный аккумулятор
    vpxor ymm6, ymm6, ymm6        # 32×i8 байтовый аккумулятор
    test rdx, rdx
    jz .Lm32_red
.Lm32_lp:
    mov rcx, rdx
    sub rcx, r11
    cmp rcx, 32
    jb .Lm32_tail
    vmovdqu ymm0, [rdi + r11]     # w (32 трита)
    vmovdqu ymm1, [rsi + r11]     # x (32 трита)
    vpsignb ymm2, ymm1, ymm0      # w·x — умножение ЗНАКОМ
    vpaddb  ymm6, ymm6, ymm2      # байтовая аккумуляция
    inc r10d
    cmp r10d, 3
    jb .Lm32_next
    xor r10d, r10d
    # --- свёртка байтового acc в i32 (верх ymm5 не трогаем: xmm5) ---
    vextracti128 xmm3, ymm6, 0
    vextracti128 xmm4, ymm6, 1
    vpmovsxbw xmm7, xmm3
    vpsrldq  xmm3, xmm3, 8
    vpmovsxbw xmm3, xmm3
    vpmaddwd xmm7, xmm7, [rip + .Lst_one16]
    vpmaddwd xmm3, xmm3, [rip + .Lst_one16]
    vpaddd  xmm5, xmm5, xmm7
    vpaddd  xmm5, xmm5, xmm3
    vpmovsxbw xmm7, xmm4
    vpsrldq  xmm4, xmm4, 8
    vpmovsxbw xmm4, xmm4
    vpmaddwd xmm7, xmm7, [rip + .Lst_one16]
    vpmaddwd xmm4, xmm4, [rip + .Lst_one16]
    vpaddd  xmm5, xmm5, xmm7
    vpaddd  xmm5, xmm5, xmm4
    vpxor   ymm6, ymm6, ymm6      # сброс байтового аккумулятора
.Lm32_next:
    add r11, 32
    jmp .Lm32_lp
.Lm32_tail:
    cmp r11, rdx
    jae .Lm32_flush
    movsx eax, byte ptr [rdi + r11]
    movsx ecx, byte ptr [rsi + r11]
    imul eax, ecx
    movsxd rax, eax
    add r8, rax
    inc r11
    jmp .Lm32_tail
.Lm32_flush:
    # ФИНАЛЬНАЯ свёртка байтового аккумулятора: счётчик чанков мог
    # остаться 1..2 (не кратен 3) — НЕсвёрнутые произведения в ymm6
    # обязаны попасть в xmm5, иначе Σ теряет до 64 трит-произведений
    vextracti128 xmm3, ymm6, 0
    vextracti128 xmm4, ymm6, 1
    vpmovsxbw xmm7, xmm3
    vpsrldq  xmm3, xmm3, 8
    vpmovsxbw xmm3, xmm3
    vpmaddwd xmm7, xmm7, [rip + .Lst_one16]
    vpmaddwd xmm3, xmm3, [rip + .Lst_one16]
    vpaddd  xmm5, xmm5, xmm7
    vpaddd  xmm5, xmm5, xmm3
    vpmovsxbw xmm7, xmm4
    vpsrldq  xmm4, xmm4, 8
    vpmovsxbw xmm4, xmm4
    vpmaddwd xmm7, xmm7, [rip + .Lst_one16]
    vpmaddwd xmm4, xmm4, [rip + .Lst_one16]
    vpaddd  xmm5, xmm5, xmm7
    vpaddd  xmm5, xmm5, xmm4
.Lm32_red:
    vpshufd xmm6, xmm5, 0x4E
    vpaddd xmm5, xmm5, xmm6
    vpshufd xmm6, xmm5, 0x01
    vpaddd xmm5, xmm5, xmm6
    vmovd eax, xmm5
    movsxd rax, eax
    add rax, r8
    vzeroupper
    ret

    # ================================================================
    # i64 poler_trit_step_packed(const TritStepParams* pp)
    #   Плотный троичный шаг SSN, веса pack5 (0.2 Б/синапс).
    #   Источник s читает ОКНО спайков x[s .. s+P·5) (временное окно,
    #   как у SSN v0.87); веса распаковываются LUT-243 в wbuf;
    #   MAC — vpsignb; пост — ТОЧНЫЙ i32 (ни одного округления);
    #   затухание — Q8: post = (post·decay_q8) >> 8 + Σ w·x.
    #   Возвращает энергию Σ|post'|.
    #   rdi = pp. Стек-слоты: [8]=mac_bound, [16]=acc, [24]=dst,
    #   [32]=P. Контракт: x.len() ≥ n_pre + P·5; wbuf ≥ P·5.
    # ================================================================
    .globl poler_trit_step_packed
    .p2align 4
poler_trit_step_packed:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 48
    mov rsi, [rdi + 0]           # w_packed
    mov rdx, [rdi + 8]           # x
    mov rcx, [rdi + 16]          # post
    mov r8,  [rdi + 24]          # lut
    mov r9,  [rdi + 32]          # wbuf
    mov r10, [rdi + 40]          # n_pre
    mov rax, [rdi + 48]          # P (байт pack5 на источник)
    mov [rsp + 32], rax
    mov r12d, [rdi + 56]         # post_mask
    mov r13d, [rdi + 60]         # post_stride
    mov r14d, [rdi + 64]         # decay_q8
    xor r15, r15                 # энергия
    xor ebx, ebx                 # s = 0
    test r10, r10
    jz .Lstp_done
.Lstp_outer:
    mov qword ptr [rsp + 16], 0   # acc = 0
    mov rax, [rsp + 32]          # P
    imul rax, rbx                # s·P
    lea rbp, [rsi + rax]         # бегущий указатель упакованных весов
    xor edi, edi                 # g = 0
.Lstp_un:
    cmp rdi, [rsp + 32]          # g < P?
    jae .Lstp_macprep
    lea r11, [rdi + rdi*4]      # g·5 (LEA ×5)
    add r11, r9                  # &wbuf[g·5]
    movzx eax, byte ptr [rbp + rdi]  # v = packed[s·P + g]
    shl rax, 3
    mov rax, [r8 + rax]          # LUT: u64 = 5 трит-байт
    mov [r11], eax               # младшие 4 трита
    shr rax, 32
    mov [r11 + 4], al            # 5-й трит
    inc rdi
    jmp .Lstp_un
.Lstp_macprep:
    mov rax, [rsp + 32]
    lea r11, [rax + rax*4]       # P·5 = мак-граница (LEA ×5)
    mov [rsp + 8], r11
    lea rbp, [rdx + rbx]         # окно спайков x[s ..]
    vpxor xmm5, xmm5, xmm5
    xor edi, edi                 # j = 0
.Lstp_mac:
    cmp rdi, [rsp + 8]
    jae .Lstp_post
    mov rax, [rsp + 8]
    sub rax, rdi
    cmp rax, 16
    jb .Lstp_mtail
    vmovdqu xmm0, [r9 + rdi]     # wbuf[j .. j+16)
    vmovdqu xmm1, [rbp + rdi]    # x[s + j ..]
    vpsignb xmm2, xmm1, xmm0     # w·x
    vpmovsxbw xmm3, xmm2
    vpsrldq xmm4, xmm2, 8
    vpmovsxbw xmm4, xmm4
    vpmaddwd xmm3, xmm3, [rip + .Lst_one16]
    vpmaddwd xmm4, xmm4, [rip + .Lst_one16]
    vpaddd xmm5, xmm5, xmm3
    vpaddd xmm5, xmm5, xmm4
    add rdi, 16
    jmp .Lstp_mac
.Lstp_mtail:
    cmp rdi, [rsp + 8]
    jae .Lstp_post
    movsx eax, byte ptr [r9 + rdi]
    movsx r11d, byte ptr [rbp + rdi]
    imul eax, r11d
    movsxd rax, eax
    add [rsp + 16], rax
    inc rdi
    jmp .Lstp_mtail
.Lstp_post:
    vpshufd xmm6, xmm5, 0x4E
    vpaddd xmm5, xmm5, xmm6
    vpshufd xmm6, xmm5, 0x01
    vpaddd xmm5, xmm5, xmm6
    vmovd eax, xmm5
    movsxd rax, eax
    add [rsp + 16], rax          # acc += векторная часть
    mov eax, ebx                 # dst = (s·stride) & mask
    imul eax, r13d
    and eax, r12d
    lea rbp, [rcx + rax*4]       # &post[dst]
    movsxd rax, dword ptr [rbp]  # старый пост (знакорасширение!)
    imul rax, r14                # × decay_q8
    sar rax, 8                    # ТОЧНОЕ деление на 2^8 (сдвиг)
    add rax, [rsp + 16]          # + acc (всё целое — ни одного rounding)
    mov [rbp], eax
    movsxd rax, eax
    test rax, rax
    jns 8f
    neg rax
8:
    add r15, rax                 # энергия += |post'|
    inc rbx
    cmp rbx, r10
    jb .Lstp_outer
.Lstp_done:
    mov rax, r15
    add rsp, 48
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret

    # ================================================================
    # i64 poler_trit_csr_step(const TritCsrParams* pp)
    #   CSR-режим: реальный коннектом (FlyWire). Пост-центричные строки
    #   in-рёбер: пост-нейрон u аккумулирует Σ w(u,v)·x[v] по своим
    #   пре-синапсам v. Веса — pack5 ПО СТРОКЕ (row-aligned, poffs);
    #   распаковка LUT-243; gather x[src] скалярный (x — L2-резидент);
    #   MAC — vpsignb по (wbuf × xbuf). Затухание Q8-сдвиг. Строки
    #   [row_begin, row_end) — диапазон для rayon-параллелизма
    #   (пост-нейроны ДИЗЪЮНКТНЫ — гонок нет).
    #   Возвращает Σ|post'| диапазона. rdi = pp.
    # ================================================================
    .globl poler_trit_csr_step
    .p2align 4
poler_trit_csr_step:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 56
    mov rsi, [rdi + 0]           # offsets
    mov rax, [rdi + 8]           # srcs → стек (rdx — универсальный скретч)
    mov [rsp + 48], rax
    mov rcx, [rdi + 16]          # w_packed
    mov r8,  [rdi + 24]          # poffs
    mov r9,  [rdi + 32]          # x
    mov r10, [rdi + 40]          # post
    mov r11, [rdi + 48]          # lut
    mov rbx, [rdi + 56]          # wbuf
    mov rbp, [rdi + 64]          # xbuf
    mov r12, [rdi + 72]          # row_begin
    mov r13, [rdi + 80]          # row_end
    mov r14d, [rdi + 88]         # decay_q8
    # локализация пост-базы под диапазон строк: вызывающий передаёт
    # срез, начинающийся РОВНО с row_begin — ядро пишет
    # post[u − row_begin]; срезы rayon ДИЗЪЮНКТНЫ, указатели никогда
    # не выходят за границы аллокаций
    mov [rsp + 24], r12          # row_begin (для пересчёта локального индекса)
    xor r15, r15                 # энергия
.Lcsr_row:
    cmp r12, r13
    jae .Lcsr_done
    mov qword ptr [rsp + 32], 0   # acc = 0
    mov eax, [rsi + r12*4]       # a = offsets[u]
    mov [rsp + 0], rax
    mov eax, [rsi + r12*4 + 4]   # b = offsets[u+1]
    sub eax, dword ptr [rsp + 0] # len = b − a
    mov [rsp + 8], rax
    mov eax, [r8 + r12*4]        # poffs[u]
    mov [rsp + 40], rax          # (затем → указатель строки)
    mov eax, [r8 + r12*4 + 4]    # poffs[u+1]
    sub eax, dword ptr [rsp + 40] # P = байт pack5 в строке
    mov [rsp + 16], rax
    mov rax, [rsp + 40]
    lea rax, [rcx + rax]         # бегущий указатель упакованной строки
    mov [rsp + 40], rax
    xor edi, edi                 # g = 0
.Lcsr_un:
    cmp rdi, [rsp + 16]          # g < P?
    jae .Lcsr_gatprep
    lea rdx, [rdi + rdi*4]      # g·5 (LEA ×5)
    add rdx, rbx                 # &wbuf[g·5]
    mov rax, [rsp + 40]
    movzx eax, byte ptr [rax + rdi]  # v
    shl rax, 3
    mov rax, [r11 + rax]         # LUT-243
    mov [rdx], eax
    shr rax, 32
    mov [rdx + 4], al
    inc rdi
    jmp .Lcsr_un
.Lcsr_gatprep:
    mov rax, [rsp + 48]          # srcs base
    mov rdx, [rsp + 0]           # a
    lea rdx, [rax + rdx*4]       # &srcs[a]
    xor edi, edi                 # i = 0
.Lcsr_gat:
    cmp rdi, [rsp + 8]           # i < len?
    jae .Lcsr_macprep
    mov eax, [rdx + rdi*4]       # src (u32)
    movzx eax, byte ptr [r9 + rax]   # x[src] — трит-спайк
    mov [rbp + rdi], al          # xbuf[i]
    inc rdi
    jmp .Lcsr_gat
.Lcsr_macprep:
    vpxor xmm5, xmm5, xmm5
    xor edi, edi                 # j = 0
.Lcsr_mac:
    cmp rdi, [rsp + 8]
    jae .Lcsr_post
    mov rax, [rsp + 8]
    sub rax, rdi
    cmp rax, 16
    jb .Lcsr_mtail
    vmovdqu xmm0, [rbx + rdi]    # wbuf[j .. j+16)
    vmovdqu xmm1, [rbp + rdi]    # xbuf[j .. j+16)
    vpsignb xmm2, xmm1, xmm0
    vpmovsxbw xmm3, xmm2
    vpsrldq xmm4, xmm2, 8
    vpmovsxbw xmm4, xmm4
    vpmaddwd xmm3, xmm3, [rip + .Lst_one16]
    vpmaddwd xmm4, xmm4, [rip + .Lst_one16]
    vpaddd xmm5, xmm5, xmm3
    vpaddd xmm5, xmm5, xmm4
    add rdi, 16
    jmp .Lcsr_mac
.Lcsr_mtail:
    cmp rdi, [rsp + 8]
    jae .Lcsr_post
    movsx eax, byte ptr [rbx + rdi]
    movsx edx, byte ptr [rbp + rdi]  # rdx свободен; rcx (w_packed) НЕ ТРОГАТЬ —
                                     # ГРАБЛЯ: затирка rcx роняла строку u+1
    imul eax, edx
    movsxd rax, eax
    add [rsp + 32], rax
    inc rdi
    jmp .Lcsr_mtail
.Lcsr_post:
    vpshufd xmm6, xmm5, 0x4E
    vpaddd xmm5, xmm5, xmm6
    vpshufd xmm6, xmm5, 0x01
    vpaddd xmm5, xmm5, xmm6
    vmovd eax, xmm5
    movsxd rax, eax
    add [rsp + 32], rax
    mov rax, r12
    sub rax, [rsp + 24]          # локальный индекс u − row_begin
    lea rdx, [r10 + rax*4]       # &post_local[u − row_begin]
    movsxd rax, dword ptr [rdx]
    imul rax, r14                # × decay_q8
    sar rax, 8                    # точное ÷2^8
    add rax, [rsp + 32]
    mov [rdx], eax
    movsxd rax, eax
    test rax, rax
    jns 9f
    neg rax
9:
    add r15, rax
    inc r12
    jmp .Lcsr_row
.Lcsr_done:
    mov rax, r15
    add rsp, 56
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
    "#
}

extern "sysv64" {
    fn poler_z3_sincos_q24(phase: *const i32, sout: *mut i32, cout: *mut i32, n: usize);
    fn poler_trit_mac32(w: *const i8, x: *const i8, n: usize) -> i64;
    fn poler_trit_step_packed(pp: *const TritStepParams) -> i64;
    fn poler_trit_csr_step(pp: *const TritCsrParams) -> i64;
}

// ====================================================================
// Rust-слой: LUT-243, ℤ₁₂-ротор, трит-MAC, плотное поле, CSR-коннектом
// ====================================================================

use std::sync::OnceLock;

/// ℤ₁₂-синус решётки Q24, 16 записей (12..15 ≡ 0..3).
pub const Z3_SIN16: [i32; 16] = [
    0, 8388608, 14529495, 16777216, 14529495, 8388608, 0, -8388608,
    -14529495, -16777216, -14529495, -8388608, 0, 8388608, 14529495, 16777216,
];

/// ℤ₁₂-косинус решётки Q24.
pub const Z3_COS16: [i32; 16] = [
    16777216, 14529495, 8388608, 0, -8388608, -14529495, -16777216, -14529495,
    -8388608, 0, 8388608, 14529495, 16777216, 14529495, 8388608, 0,
];

/// Шаг решётки в Q24: 2²⁴ = 30°.
pub const Z3_STEP_Q24: i32 = 1 << 24;

/// LUT-243: packed-байт → u64, где байт j = трит j (порядок pack5:
/// первый трит пятёрки — старшая цифра). Таблица 1944 Б живёт в L1.
fn lut243() -> &'static [u64; 243] {
    static LUT: OnceLock<[u64; 243]> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut t = [0u64; 243];
        for v in 0..243usize {
            let mut q = v;
            let mut u = 0u64;
            // цифры от МЛАДШЕЙ (d0) к старшей (d4); выходной порядок
            // unpack5: out[0] = d4 → байт 0 = d4−1, …, байт 4 = d0−1
            let mut digits = [0u32; 5];
            for k in (0..5).rev() {
                digits[k] = (q % 3) as u32;
                q /= 3;
            }
            for j in 0..5 {
                // digits[j] = цифра d(4−j) (цикл ниже заполняет так, что
                // digits[4]=d0…digits[0]=d4) — а байт j LUT = out[j]
                // unpack5 = d(4−j). Итого: байт j = digits[j] − 1.
                let trit = (digits[j] as i64) - 1;
                u |= (trit as u64 & 0xFF) << (8 * j);
            }
            t[v] = u;
        }
        t
    })
}

fn st_ready() -> bool {
    let c = crate::asm::caps();
    c.avx2 && c.ssse3 && c.sse41
}

/// ℤ₁₂-ротор: массив фаз Q24 → (sin, cos) Q24 за ~1 такт на значение.
///
/// Фаза — u32-модулярная: `k = (phase >> 24) & 15`. Поворот на 120°
/// (ω) — это [`z3_advance`] на 4 шага: одно сложение, бит-точно.
pub fn z3_sincos(phase: &[i32], sout: &mut [i32], cout: &mut [i32]) {
    let n = phase.len().min(sout.len()).min(cout.len());
    if n == 0 {
        return;
    }
    if crate::asm::caps().avx2 {
        let bulk = n / 8 * 8;
        if bulk > 0 {
            unsafe {
                poler_z3_sincos_q24(
                    phase.as_ptr(),
                    sout.as_mut_ptr(),
                    cout.as_mut_ptr(),
                    bulk,
                )
            };
        }
        for i in bulk..n {
            let k = ((phase[i] as u32) >> 24) as usize & 15;
            sout[i] = Z3_SIN16[k];
            cout[i] = Z3_COS16[k];
        }
    } else {
        for i in 0..n {
            let k = ((phase[i] as u32) >> 24) as usize & 15;
            sout[i] = Z3_SIN16[k];
            cout[i] = Z3_COS16[k];
        }
    }
}

/// Скалярный ℤ₁₂-ротор (эталон).
pub fn z3_sincos_scalar(phase: i32) -> (i32, i32) {
    let k = ((phase as u32) >> 24) as usize & 15;
    (Z3_SIN16[k], Z3_COS16[k])
}

/// Поворот фазы на `steps` шагов решётки (30°) — БИТ-ТОЧНО mod 12.
///
/// ω-поворот = 4 шага (120°); ω³ = тождество — точно, потому что
/// инвариант: канонический k ∈ 0..11 поддерживается на каждом
/// повороте (add + cmov, БЕЗ деления — вычитание 12 при переполнении).
/// Младшие 24 бита (дробная фаза внутри шага) не затрагиваются.
#[inline]
pub fn z3_advance(phase: i32, steps: i32) -> i32 {
    // нибл = биты 27..24 — ОБЯЗАТЕЛЬНА маска 15: у негативных фаз
    // биты 31..28 попадают в (u32 >> 24) и ломают mod-12 инвариант
    let k = (((phase as u32) >> 24) & 15) as i32;
    let k2 = (k + steps).rem_euclid(12);
    (phase & 0x00FF_FFFF) | (k2 << 24)
}

/// Плотный трит-MAC (развернутые веса, 1 Б/синапс): Σ wᵢxᵢ, i64.
pub fn mac32(w: &[i8], x: &[i8]) -> i64 {
    let n = w.len().min(x.len());
    if n == 0 {
        return 0;
    }
    if st_ready() {
        unsafe { poler_trit_mac32(w.as_ptr(), x.as_ptr(), n) }
    } else {
        w.iter().zip(x).map(|(&a, &b)| a as i64 * b as i64).sum()
    }
}

/// Параметры плотного троичного шага SSN (pack5-веса).
#[repr(C)]
pub struct TritStepParams {
    pub w_packed: *const u8,
    pub x: *const i8,
    pub post: *mut i32,
    pub lut: *const u64,
    pub wbuf: *mut i8,
    pub n_pre: usize,
    /// Байт pack5 на источник = (fanout + 4) / 5.
    pub p: usize,
    pub post_mask: u32,
    pub post_stride: u32,
    pub decay_q8: u32,
}

/// Плотное троичное синаптическое поле: pack5-веса (0.2 Б/синапс).
///
/// Виртуальный коннектом как у SSN v0.87: `dst(s) = (s·α) & (n_post−1)`.
/// Пост-состояние — ТОЧНЫЕ i32: ни одного округления на всём пути
/// (трит-MAC целочислен, затухание — Q8-сдвиг).
pub struct TritSsnField {
    pub n_pre: usize,
    /// Степень двойки!
    pub n_post: usize,
    pub fanout: usize,
    /// pack5-веса: n_pre × P байт.
    pub w_packed: Vec<u8>,
    /// P = (fanout+4)/5.
    pub p: usize,
    pub post_stride: u32,
}

impl TritSsnField {
    /// Квантизация f32-веса в трит: |w| > thr → знак, иначе ⊙ (вакуум).
    pub fn quantize(w: f32, thr: f32) -> i8 {
        if w > thr {
            1
        } else if w < -thr {
            -1
        } else {
            0
        }
    }

    /// Синтетическое поле (xorshift64, детерминизм).
    pub fn synthetic(n_pre: usize, fanout: usize, seed: u64) -> Self {
        let n_post = (n_pre / 8).next_power_of_two().max(64);
        let p = (fanout + 4) / 5;
        let mut s = seed | 1;
        let mut xs = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        // развёрнутые триты → pack5 (паддинг ⊙ до P·5 на источник)
        let mut trits = vec![0i8; n_pre * p * 5];
        for i in 0..n_pre * fanout {
            let w = ((xs() >> 33) as i32 % 200) as f32 / 100.0;
            trits[i] = Self::quantize(w, 0.25);
        }
        let mut w_packed = vec![0u8; n_pre * p];
        crate::asm::trit_asm::pack5(&trits, &mut w_packed);
        Self {
            n_pre,
            n_post,
            fanout,
            w_packed,
            p,
            post_stride: 2654435761,
        }
    }

    /// Байт на синапс (payload): ровно 0.2.
    pub fn bytes_per_synapse(&self) -> f32 {
        0.2
    }

    /// Число синапсов поля.
    pub fn synapses(&self) -> usize {
        self.n_pre * self.fanout
    }

    /// Контракт длины спайков: окно x[s .. s+P·5) последнего источника.
    pub fn pad_x_len(&self) -> usize {
        self.n_pre + self.p * 5
    }

    /// Один троичный шаг. `x` — трит-спайки (например, из
    /// [`crate::asm::trit_asm::spike_f32`] → i8), `post` — точные i32.
    /// Возвращает энергию Σ|post'|.
    pub fn step(&self, x: &[i8], post: &mut [i32], decay_q8: u32) -> i64 {
        let bound = self.p * 5;
        debug_assert!(x.len() >= self.pad_x_len());
        debug_assert!(post.len() >= self.n_post);
        let mut wbuf = vec![0i8; bound + 16];
        if st_ready() {
            let pp = TritStepParams {
                w_packed: self.w_packed.as_ptr(),
                x: x.as_ptr(),
                post: post.as_mut_ptr(),
                lut: lut243().as_ptr(),
                wbuf: wbuf.as_mut_ptr(),
                n_pre: self.n_pre,
                p: self.p,
                post_mask: (self.n_post - 1) as u32,
                post_stride: self.post_stride,
                decay_q8,
            };
            unsafe { poler_trit_step_packed(&pp) }
        } else {
            self.step_scalar(x, post, decay_q8)
        }
    }

    /// Скалярный эталон (та же семантика, что у ядра).
    pub fn step_scalar(&self, x: &[i8], post: &mut [i32], decay_q8: u32) -> i64 {
        let mut energy = 0i64;
        let bound = self.p * 5;
        let lut = lut243();
        for s in 0..self.n_pre {
            let dst =
                ((s as u32).wrapping_mul(self.post_stride) as usize) & (self.n_post - 1);
            let mut acc = 0i64;
            for g in 0..self.p {
                let v = self.w_packed[s * self.p + g] as usize;
                let u = lut[v];
                for j in 0..5 {
                    let t = ((u >> (8 * j)) & 0xFF) as i8;
                    let idx = g * 5 + j;
                    if idx < bound {
                        acc += t as i64 * x[s + idx] as i64;
                    }
                }
            }
            let old = post[dst] as i64;
            let new = ((old * decay_q8 as i64) >> 8) + acc;
            post[dst] = new as i32;
            energy += new.abs();
        }
        energy
    }
}

/// Параметры CSR-шага (реальный коннектом, пост-центричные строки).
#[repr(C)]
pub struct TritCsrParams {
    pub offsets: *const u32,
    pub srcs: *const u32,
    pub w_packed: *const u8,
    /// Смещения packed-байт по строкам (n_post+1) — row-aligned pack5.
    pub poffs: *const u32,
    pub x: *const i8,
    pub post: *mut i32,
    pub lut: *const u64,
    pub wbuf: *mut i8,
    pub xbuf: *mut i8,
    pub row_begin: usize,
    pub row_end: usize,
    pub decay_q8: u32,
}

/// Троичный CSR-коннектом: FlyWire → триты → pack5 по строкам.
///
/// Квантование: `|signed_weight| ≥ thr` → трит знака, иначе ⊙ (вакуум).
/// Плотность payload: 0.2 Б/синапс — 50M синапсов = 10 МБ (кэш L3).
pub struct TritCsrConnectome {
    pub n_nodes: usize,
    pub n_edges: usize,
    /// In-рёбра: строка u = пре-синапсы пост-нейрона u.
    pub offsets: Vec<u32>,
    pub srcs: Vec<u32>,
    /// Row-aligned pack5-веса.
    pub w_packed: Vec<u8>,
    pub poffs: Vec<u32>,
    /// Число ненулевых тритов (не-вакуум).
    pub n_active: usize,
}

impl TritCsrConnectome {
    /// Синтетический CSR-коннектом (FlyWire-масштабируемый, детерминизм).
    pub fn synthetic(n_post: usize, avg_fanout: usize, seed: u64) -> Self {
        let mut s = seed | 1;
        let mut xs = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        // построение in-рёбер: для каждого пост u — fanout ~ Пуассон-подобный
        let mut counts = vec![0u32; n_post];
        let est = n_post * avg_fanout;
        let mut pairs: Vec<(u32, u32, i8)> = Vec::with_capacity(est + 16);
        for u in 0..n_post {
            let f = (avg_fanout + ((xs() >> 33) as usize % 5)).saturating_sub(2).max(1);
            for _ in 0..f {
                let src = (xs() >> 33) as u32 % n_post as u32;
                let r = (xs() >> 33) as i32 % 3;
                let t = if r == 0 { 0 } else if r == 1 { 1 } else { -1 };
                pairs.push((src, u as u32, t));
                counts[u] += 1;
            }
        }
        pairs.sort_by_key(|p| (p.1, p.0)); // по строкам, src возрастает
        Self::build(n_post, &pairs, &counts)
    }

    /// Из пар (src, post, trit) — общий строитель (сортировка на вызывающем).
    fn build(n_nodes: usize, pairs: &[(u32, u32, i8)], counts: &[u32]) -> Self {
        let mut offsets = Vec::with_capacity(n_nodes + 1);
        let mut off = 0u32;
        offsets.push(0u32);
        for &c in counts {
            off += c;
            offsets.push(off);
        }
        let n_edges = pairs.len();
        let mut srcs = Vec::with_capacity(n_edges);
        let mut trits: Vec<i8> = Vec::with_capacity(n_edges);
        let mut n_active = 0usize;
        for &(src, _, t) in pairs {
            srcs.push(src);
            trits.push(t);
            if t != 0 {
                n_active += 1;
            }
        }
        // row-aligned pack5: строка u упаковывается независимо (паддинг ⊙)
        let mut w_packed = Vec::new();
        let mut poffs = Vec::with_capacity(n_nodes + 1);
        poffs.push(0u32);
        for u in 0..n_nodes {
            let (a, b) = (offsets[u] as usize, offsets[u + 1] as usize);
            let mut row: Vec<i8> = trits[a..b].to_vec();
            while row.len() % 5 != 0 {
                row.push(0);
            }
            let mut pb = vec![0u8; row.len() / 5];
            crate::asm::trit_asm::pack5(&row, &mut pb);
            w_packed.extend_from_slice(&pb);
            poffs.push(w_packed.len() as u32);
        }
        Self {
            n_nodes,
            n_edges,
            offsets,
            srcs,
            w_packed,
            poffs,
            n_active,
        }
    }

    /// Импорт из [`crate::graph::connectome::Connectome`] (FlyWire CSR):
    /// in-рёбра + квантизация знаковых весов в триты.
    ///
    /// `thr` — порог числа синапсов: `|w·sign(nt)| ≥ thr` → трит знака.
    pub fn from_connectome(c: &crate::graph::connectome::Connectome, thr: i32) -> Self {
        let n = c.n_nodes();
        let mut counts = vec![0u32; n];
        let mut pairs: Vec<(u32, u32, i8)> = Vec::with_capacity(c.n_edges());
        for u in 0..n {
            if let Some(edges) = c.out_edges(u) {
                for e in edges {
                    let sw = e.signed_weight();
                    let t = if sw >= thr {
                        1
                    } else if sw <= -thr {
                        -1
                    } else {
                        0
                    };
                    // in-ребро пост-нейрона target:
                    pairs.push((u as u32, e.target, t));
                    counts[e.target as usize] += 1;
                }
            }
        }
        pairs.sort_by_key(|p| (p.1, p.0));
        Self::build(n, &pairs, &counts)
    }

    /// Payload-байт на синапс.
    pub fn bytes_per_synapse(&self) -> f32 {
        0.2
    }

    /// Полный шаг (все строки).
    pub fn step(&self, x: &[i8], post: &mut [i32], decay_q8: u32) -> i64 {
        let max_row = self
            .offsets
            .windows(2)
            .map(|w| (w[1] - w[0]) as usize)
            .max()
            .unwrap_or(0);
        let bound = ((max_row + 4) / 5) * 5 + 16;
        let mut scratch = vec![0i8; 2 * bound];
        self.step_range(x, post, 0, self.n_nodes, decay_q8, &mut scratch)
    }

    /// Шаг на диапазоне строк [row_begin, row_end): `post_range` —
    /// срез, покрывающий РОВНО эти строки (ядро локализует базу:
    /// post_local[u − row_begin]). Диапазоны дизъюнктны — безопасно
    /// для rayon-параллелизма, указатели в границах аллокаций.
    pub fn step_range(
        &self,
        x: &[i8],
        post_range: &mut [i32],
        row_begin: usize,
        row_end: usize,
        decay_q8: u32,
        scratch: &mut [i8],
    ) -> i64 {
        let max_row = self
            .offsets
            .windows(2)
            .map(|w| (w[1] - w[0]) as usize)
            .max()
            .unwrap_or(0);
        let bound = ((max_row + 4) / 5) * 5 + 16;
        debug_assert!(scratch.len() >= 2 * bound);
        debug_assert!(x.len() >= self.n_nodes);
        debug_assert!(post_range.len() >= row_end - row_begin);
        if !st_ready() {
            let lut = lut243();
            let mut e = 0i64;
            for u in row_begin..row_end {
                let (a, b) = (self.offsets[u] as usize, self.offsets[u + 1] as usize);
                let mut acc = 0i64;
                for i in a..b {
                    let g = (i - a) % 5;
                    let byte = self.w_packed[self.poffs[u] as usize + (i - a) / 5];
                    let t = ((lut[byte as usize] >> (8 * g)) & 0xFF) as i8;
                    acc += t as i64 * x[self.srcs[i] as usize] as i64;
                }
                let ps = &mut post_range[u - row_begin];
                let old = *ps as i64;
                let new = ((old * decay_q8 as i64) >> 8) + acc;
                *ps = new as i32;
                e += new.abs();
            }
            return e;
        }
        let (wb, xb) = scratch.split_at_mut(bound);
        let pp = TritCsrParams {
            offsets: self.offsets.as_ptr(),
            srcs: self.srcs.as_ptr(),
            w_packed: self.w_packed.as_ptr(),
            poffs: self.poffs.as_ptr(),
            x: x.as_ptr(),
            post: post_range.as_mut_ptr(),
            lut: lut243().as_ptr(),
            wbuf: wb.as_mut_ptr(),
            xbuf: xb.as_mut_ptr(),
            row_begin,
            row_end,
            decay_q8,
        };
        unsafe { poler_trit_csr_step(&pp) }
    }

    /// Параллельный шаг: пост делится `split_at_mut` на ДИЗЪЮНКТНЫЕ
    /// срезы по диапазонам строк, rayon-чанки гонок не имеют;
    /// энергия суммируется.
    pub fn step_parallel(&self, x: &[i8], post: &mut [i32], decay_q8: u32) -> i64 {
        use rayon::prelude::*;
        let max_row = self
            .offsets
            .windows(2)
            .map(|w| (w[1] - w[0]) as usize)
            .max()
            .unwrap_or(0);
        let bound = ((max_row + 4) / 5) * 5 + 16;
        let nchunks = rayon::current_num_threads().min(self.n_nodes).max(1);
        let chunk = (self.n_nodes + nchunks - 1) / nchunks;
        // дизъюнктные срезы поста:
        let mut slices: Vec<&mut [i32]> = Vec::with_capacity(nchunks);
        let mut ranges: Vec<(usize, usize)> = Vec::with_capacity(nchunks);
        let mut rest: &mut [i32] = post;
        let mut rb = 0usize;
        while rb < self.n_nodes {
            let re = (rb + chunk).min(self.n_nodes);
            let (head, tail) = rest.split_at_mut(re - rb);
            slices.push(head);
            ranges.push((rb, re));
            rest = tail;
            rb = re;
        }
        let results: Vec<i64> = slices
            .into_par_iter()
            .zip(ranges)
            .map(|(ps, (rb, re))| {
                let mut scratch = vec![0i8; 2 * bound];
                self.step_range(x, ps, rb, re, decay_q8, &mut scratch)
            })
            .collect();
        results.into_iter().sum()
    }

    /// Скалярный эталон (строки [rb, re)).
    pub fn step_scalar(
        &self,
        x: &[i8],
        post: &mut [i32],
        row_begin: usize,
        row_end: usize,
        decay_q8: u32,
    ) -> i64 {
        let lut = lut243();
        let mut energy = 0i64;
        for u in row_begin..row_end {
            let (a, b) = (self.offsets[u] as usize, self.offsets[u + 1] as usize);
            let mut acc = 0i64;
            for i in a..b {
                let g = (i - a) % 5;
                let byte = self.w_packed[self.poffs[u] as usize + (i - a) / 5];
                let t = ((lut[byte as usize] >> (8 * g)) & 0xFF) as i8;
                acc += t as i64 * x[self.srcs[i] as usize] as i64;
            }
            let old = post[u] as i64;
            let new = ((old * decay_q8 as i64) >> 8) + acc;
            post[u] = new as i32;
            energy += new.abs();
        }
        energy
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

    const Q24: f64 = 16_777_216.0; // 2^24

    /// Калькулятор-оракул: LUT против libm sin/cos. Рациональные узлы
    /// (0, ±1/2, ±1) — ТОЧНО (0 LSB); иррациональные √3/2 — ≤ 1 LSB.
    #[test]
    fn z3_lut_vs_calculator_oracle() {
        for k in 0..16usize {
            let ang = (k as f64) * std::f64::consts::FRAC_PI_6;
            let s_ref = (ang.sin() * Q24).round() as i64;
            let c_ref = (ang.cos() * Q24).round() as i64;
            let (s, c) = (Z3_SIN16[k] as i64, Z3_COS16[k] as i64);
            assert!((s - s_ref).abs() <= 1, "sin[{k}]: {s} vs {s_ref}");
            assert!((c - c_ref).abs() <= 1, "cos[{k}]: {c} vs {c_ref}");
            // рациональные узлы — бит-в-бит: k mod 6 ∈ {0,1,3,5}
            match k % 6 {
                0 => assert_eq!(Z3_SIN16[k], 0, "sin узла {k} не точен"),
                1 | 5 => assert_eq!(Z3_SIN16[k].abs(), 1 << 23, "1/2 не точна (k={k})"),
                3 => assert_eq!(Z3_SIN16[k].abs(), 1 << 24, "1 не точен (k={k})"),
                _ => {} // ±√3/2 — округление ≤ 1 LSB проверено выше
            }
        }
    }

    /// ω³ = I БИТ-ТОЧНО в фазовой арифметике: три поворота на 120°
    /// возвращают исходные (sin, cos) — для 2²⁰ случайных фаз.
    #[test]
    fn z3_omega3_identity_bitexact() {
        let mut s = 0xC0FFEEu64 | 1;
        for _ in 0..(1 << 20) {
            let p = (xs64(&mut s) >> 32) as i32;
            let mut q = p;
            for _ in 0..3 {
                q = z3_advance(q, 4); // ω = 120° = 4 шага ℤ₁₂
            }
            let (sa, ca) = z3_sincos_scalar(p);
            let (sb, cb) = z3_sincos_scalar(q);
            assert_eq!((sa, ca), (sb, cb), "ω³ ≠ I для фазы {p}");
        }
    }

    /// 1 + ω + ω² = 0 — ТОЧНО в Q24 (ошибки √3/2 аннигилируют парами).
    #[test]
    fn z3_one_plus_omega_plus_omega2_zero() {
        let (s0, c0) = z3_sincos_scalar(0); // 1 = cos 0 + i·sin 0
        let (s1, c1) = z3_sincos_scalar(4 << 24); // ω (120°)
        let (s2, c2) = z3_sincos_scalar(8 << 24); // ω² (240°)
        let re: i64 = c0 as i64 + c1 as i64 + c2 as i64;
        let im: i64 = s0 as i64 + s1 as i64 + s2 as i64;
        assert_eq!(re, 0, "Re(1+ω+ω²) = {re} ≠ 0");
        assert_eq!(im, 0, "Im(1+ω+ω²) = {im} ≠ 0");
    }

    /// Векторное ядро ℤ₁₂-ротора ≡ скалярному эталону (хвосты включены).
    #[test]
    fn z3_sincos_parity_asm_scalar() {
        let mut s = 1234567u64 | 1;
        let n = 4093; // нечётный — хвост exercised
        let phase: Vec<i32> = (0..n).map(|_| (xs64(&mut s) >> 32) as i32).collect();
        let mut sout = vec![0i32; n];
        let mut cout = vec![0i32; n];
        z3_sincos(&phase, &mut sout, &mut cout);
        for i in 0..n {
            let (es, ec) = z3_sincos_scalar(phase[i]);
            assert_eq!(sout[i], es, "sin[{i}]");
            assert_eq!(cout[i], ec, "cos[{i}]");
        }
    }

    /// Плотный трит-MAC ≡ скалярному (включая нечётный хвост И остаток
    /// счётчика чанков не кратный 3 — ГРАБЛЯ финальной свёртки ymm6).
    #[test]
    fn mac32_matches_scalar() {
        // 9997: 312 чанков = 104 тройки ровно (хвост скалярный)
        // 1000: 31 чанк = 10 троек + 1 ОСТАТОК в байтовом аккумуляторе
        // 64:   2 чанка — оба в остатке
        // 32:   1 чанк — чистый остаток
        for &n in &[9997usize, 1000, 64, 32, 96, 128] {
            let mut s = 777u64 | 1;
            let w: Vec<i8> = (0..n).map(|_| ((xs64(&mut s) >> 33) % 3) as i64 as i8 - 1).collect();
            let x: Vec<i8> = (0..n).map(|_| ((xs64(&mut s) >> 33) % 3) as i64 as i8 - 1).collect();
            let asm = mac32(&w, &x);
            let sc: i64 = w.iter().zip(&x).map(|(&a, &b)| a as i64 * b as i64).sum();
            assert_eq!(asm, sc, "n={n}: mac32 asm={asm} scalar={sc}");
        }
    }

    /// Плотный шаг pack5-поля: asm ≡ скалярному эталону, энергия та же.
    #[test]
    fn trit_step_packed_parity() {
        let field = TritSsnField::synthetic(512, 40, 42); // fanout 40 → p=8, точно 0.2 Б/син
        let mut s = 555u64 | 1;
        let x: Vec<i8> = (0..field.pad_x_len())
            .map(|_| ((xs64(&mut s) >> 33) % 3) as i64 as i8 - 1)
            .collect();
        let mut post_a = vec![7i32; field.n_post];
        let mut post_b = vec![7i32; field.n_post];
        let ea = field.step(&x, &mut post_a, 224);
        let eb = field.step_scalar(&x, &mut post_b, 224);
        assert_eq!(ea, eb, "энергия {ea} vs {eb}");
        assert_eq!(post_a, post_b);
        // 0.2 Б/синапс: n_pre·P байт на n_pre·fanout синапсов (fanout кратно 5)
        let bps = field.w_packed.len() as f32 / field.synapses() as f32;
        assert!(bps <= 0.2001, "плотность {bps} > 0.2 Б/синапс");
        // fanout НЕ кратен 5 — паритет сохраняется, плотность ≤ 0.2 + паддинг
        let field2 = TritSsnField::synthetic(256, 37, 43);
        let mut pa = vec![1i32; field2.n_post];
        let mut pb = vec![1i32; field2.n_post];
        let x2: Vec<i8> = vec![1i8; field2.pad_x_len()];
        let e1 = field2.step(&x2, &mut pa, 224);
        let e2 = field2.step_scalar(&x2, &mut pb, 224);
        assert_eq!((e1, pa), (e2, pb));
    }

    /// Q8-распад достигает АБСОЛЮТНОГО НУЛЯ за конечные шаги (p ≤ 2 → 0),
    /// тогда как f32 × 0.875 застывает в субнормали.
    #[test]
    fn q8_decay_reaches_exact_zero() {
        let mut p_int = 100i64;
        let mut steps = 0;
        while p_int != 0 && steps < 64 {
            p_int = (p_int * 224) >> 8; // 7/8 Q8
            steps += 1;
        }
        assert_eq!(p_int, 0, "целочисленный распад не достиг нуля за {steps} шагов");
        // контраст: f32-путь на минимальной субнормали — НИКОГДА:
        let mut f = f32::from_bits(1); // 1.4e-45
        for _ in 0..64 {
            f *= 0.875;
        }
        assert!(f > 0.0, "f32-распад достиг нуля — неожиданно");
    }

    /// CSR-шаг: asm ≡ скалярному эталону (пост-центричные строки).
    #[test]
    fn csr_step_parity() {
        let csr = TritCsrConnectome::synthetic(256, 13, 99);
        let mut s = 31337u64 | 1;
        let x: Vec<i8> = (0..csr.n_nodes)
            .map(|_| ((xs64(&mut s) >> 33) % 3) as i64 as i8 - 1)
            .collect();
        let mut post_a = vec![3i32; csr.n_nodes];
        let mut post_b = vec![3i32; csr.n_nodes];
        let ea = csr.step(&x, &mut post_a, 224);
        let eb = csr.step_scalar(&x, &mut post_b, 0, csr.n_nodes, 224);
        assert_eq!(ea, eb, "энергия {ea} vs {eb}");
        assert_eq!(post_a, post_b);
    }

    /// Параллельный CSR-шаг ≡ последовательному (дизъюнктные срезы).
    #[test]
    fn csr_step_parallel_matches_serial() {
        let csr = TritCsrConnectome::synthetic(512, 9, 7);
        let mut s = 4242u64 | 1;
        let x: Vec<i8> = (0..csr.n_nodes)
            .map(|_| ((xs64(&mut s) >> 33) % 3) as i64 as i8 - 1)
            .collect();
        let mut post_ser = vec![5i32; csr.n_nodes];
        let mut post_par = vec![5i32; csr.n_nodes];
        let e_ser = csr.step(&x, &mut post_ser, 224);
        let e_par = csr.step_parallel(&x, &mut post_par, 224);
        assert_eq!(e_ser, e_par, "энергии {e_ser} vs {e_par}");
        assert_eq!(post_ser, post_par);
    }

    /// LUT-243 ≡ канонической unpack5 движка (паритет с v0.88).
    #[test]
    fn lut243_agrees_with_unpack5() {
        let lut = lut243();
        for v in 0..243u8 {
            let mut out = [0i8; 5];
            crate::asm::trit_asm::unpack5(&[v], &mut out);
            let u = lut[v as usize];
            for j in 0..5 {
                let t = ((u >> (8 * j)) & 0xFF) as i8;
                assert_eq!(t, out[j], "lut[{v}][{j}] = {t} ≠ unpack5 {out:?}");
            }
        }
    }

    /// Импорт FlyWire CSR: FLYCSR1-буфер → трит-квантование → pack5.
    #[test]
    fn connectome_flycsr1_import() {
        use crate::graph::connectome::Connectome;
        // ручной FLYCSR1: 4 узла, 5 рёбер (target строго возрастают)
        let mut raw: Vec<u8> = Vec::new();
        raw.extend_from_slice(b"FLYCSR1");
        raw.push(0); // core_flag
        raw.extend_from_slice(&4u32.to_le_bytes()); // n_nodes
        raw.extend_from_slice(&5u32.to_le_bytes()); // n_edges
        // offsets: u0: 2 ребра, u1: 0, u2: 1, u3: 2
        for o in [0u32, 2, 2, 3, 5] {
            raw.extend_from_slice(&o.to_le_bytes());
        }
        // targets (строго возрастают в строке):
        for t in [1u32, 3, 1, 0, 2] {
            raw.extend_from_slice(&t.to_le_bytes());
        }
        // weights (число синапсов):
        for w in [10u16, 3, 40, 2, 7] {
            raw.extend_from_slice(&w.to_le_bytes());
        }
        // nt: gaba(0), ach(1), glut(2), oct(3), da(5)
        for nt in [0u8, 1, 2, 3, 5] {
            raw.push(nt);
        }
        let c = Connectome::from_raw(&raw).expect("FLYCSR1 валиден");
        let csr = TritCsrConnectome::from_connectome(&c, 5);
        assert_eq!(csr.n_nodes, 4);
        assert_eq!(csr.n_edges, 5);
        // знаковые веса: u0→{1:+10, 3:−3}, u2→{1:+40}, u3→{2:0 (da), 0:+2}
        // трит порога 5: +10→+1, −3→0, +40→+1, 0→0, +2→0
        // in-рёбра: пост1 ← {u0:+1, u2:+1}, пост3 ← {u0:0}, пост2 ← {u3:0}, пост0 ← {u3:0}
        let mut expected = vec![vec![]; 4];
        for u in 0..4u32 {
            if let Some(es) = c.out_edges(u as usize) {
                for e in es {
                    let t = {
                        let sw = e.signed_weight();
                        if sw >= 5 { 1 } else if sw <= -5 { -1 } else { 0 }
                    };
                    expected[e.target as usize].push((u, t));
                }
            }
        }
        for row in 0..4 {
            let (a, b) = (csr.offsets[row] as usize, csr.offsets[row + 1] as usize);
            let got: Vec<(u32, i8)> = (a..b)
                .map(|i| {
                    let g = (i - a) % 5;
                    let byte = csr.w_packed[csr.poffs[row] as usize + (i - a) / 5];
                    let t = ((lut243()[byte as usize] >> (8 * g)) & 0xFF) as i8;
                    (csr.srcs[i], t)
                })
                .collect();
            assert_eq!(got, expected[row], "строка {row}");
        }
        // активные триты: ровно 2 (+1 от 10 синапсов, +1 от 40)
        assert_eq!(csr.n_active, 2);
        // плотность: 5 рёбер → 4 байта pack5 (0.8 Б/син на микро-масштабе)
        assert!(!csr.w_packed.is_empty());
    }
}
