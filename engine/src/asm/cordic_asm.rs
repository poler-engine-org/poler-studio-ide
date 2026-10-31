//! CORDIC-ротор сокровищницы в машинном коде x86_64 (фиксированная точка Q30).
//!
//! Итеративный сдвигово-складывающий ротор на S¹ — без `div`, без `sqrt`:
//! 30 микроитераций `x' = x ∓ (y>>i)`, `y' = y ± (x>>i)`, `z' = z ∓ atan(2⁻ⁱ)`
//! по таблице atan(2⁻ⁱ)·2³⁰ в `.rodata`. Свойства:
//!
//! * **битовая детерминированность** — целочисленные сдвиги/сложения дают
//!   одинаковый результат на любом x86_64-хосте (в отличие от libm);
//! * синус/косинус за ~30·10 тактов без таблиц элементарных функций;
//! * atan2 (векторинг) + ренормализация массива (re, im) на S¹.
//!
//! ## v0.88.0: Q24 → Q30
//!
//! Формат расширен с 24 до 30 дробных бит (30 итераций): квантование
//! 2⁻³⁰ ≈ 9.3e-10 против 2⁻²⁴ ≈ 6.0e-8 — точность ротора поднимается
//! с ~6.6e-7 до ~1e-8. Квадрант atan2 теперь через поворот на ±90°
//! (z-инициализация ±π/2·2³⁰ = 1686629713 — влезает в i32; π·2³⁰
//! уже не влезает), координаты — Q26 с запасом под рост K векторинга.
//!
//! ## Константы — Калькулятор Всего (ноль Python)
//!
//! ```text
//! poler-engine --exec 'calc atan(2^-i)*2^30'   i = 0..29
//! poler-engine --exec 'calc 1/sqrt((1+4^-0)*...*(1+4^-29))*2^30'
//!                                            → 652032874.0657896 (K30)
//! poler-engine --exec 'calc pi/2*2^30'       → 1686629713.0652523
//! ```
//! Генератор `scripts/gen_asm_consts.py` (Q24, Python) пенсионирован.
//!
//! Точности: sincos ≤ 2e-4 рад (Q30-квантование + 30 итераций),
//! atan2 ≤ 3e-5 (Q26-координаты + rcpss-Ньютон).

use std::arch::global_asm;

global_asm! {
    r#"
    .text
    .p2align 5
    # atan(2^-i) * 2^30, i = 0..29 — таблица выведена Калькулятором Всего
    # (poler-engine --exec 'calc atan(2^-i)*2^30'), округление к ближайшему.
.Lcordic_atan30:
    .long 843314857, 497837829, 263043837, 133525159
    .long 67021687, 33543516, 16775851, 8388437
    .long 4194283, 2097149, 1048576, 524288
    .long 262144, 131072, 65536, 32768
    .long 16384, 8192, 4096, 2048
    .long 1024, 512, 256, 128
    .long 64, 32, 16, 8
    .long 4, 2
    .p2align 4
.Lcordic_K30:                       # K = Π(1+4^-i)^(-1/2) = 0.6072529350
    .long 652032874                 # K·2^30 (calc: 652032874.0657896)
    .p2align 4
.Lc_i2pi:
    .float 0.1591549431             # 1/(2π)
.Lc_2pi:
    .float 6.2831853072
.Lc_pi:
    .float 3.1415926536
.Lc_hpi:
    .float 1.5707963268
.Lc_nhpi:
    .float -1.5707963268
.Lc_2p30:
    .float 1073741824.0             # 2^30
.Lc_2n30:
    .float 0.0000000009313225746    # 2^-30 (calc: 9.313225746154785e-10)
.Lc_q26:
    .float 67108864.0               # 2^26 (масштаб координат atan2)
.Lc_tiny:
    .long 0x02081CEA                # 1e-37
.Lc_two:
    .float 2.0
.Lc_abs:
    .long 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF

    .text
    # ================================================================
    # void poler_cordic_sincos(float angle, f32* cos_out, f32* sin_out)
    #   xmm0=angle, rdi=cos_out, rsi=sin_out
    # ================================================================
    .globl poler_cordic_sincos
    .p2align 4
poler_cordic_sincos:
    push rbx
    push rbp
    # --- редукция: angle -> [-π, π] ---
    vmulss xmm1, xmm0, [rip + .Lc_i2pi]
    vroundss xmm1, xmm1, xmm1, 8            # round-to-nearest
    vmulss xmm2, xmm1, [rip + .Lc_2pi]
    vsubss xmm0, xmm0, xmm2                  # angle -= k·2π
    xor r11d, r11d                           # флаг квадранта
    vcomiss xmm0, [rip + .Lc_hpi]
    jbe 2f
    vsubss xmm0, xmm0, [rip + .Lc_pi]        # angle > π/2: сложить
    mov r11d, 1
    jmp 3f
2:
    vcomiss xmm0, [rip + .Lc_nhpi]
    jae 3f
    vaddss xmm0, xmm0, [rip + .Lc_pi]        # angle < -π/2: развернуть
    mov r11d, 1
3:
    # --- в фиксированную точку Q30 ---
    vmulss xmm0, xmm0, [rip + .Lc_2p30]
    vcvttss2si r8d, xmm0                     # z
    mov ebx, [rip + .Lcordic_K30]            # x = K·2^30
    xor ebp, ebp                             # y = 0
    lea r9, [rip + .Lcordic_atan30]
    xor ecx, ecx                             # i
.Lc_loop:
    cmp ecx, 30
    jae .Lc_end
    mov eax, ebx
    sar eax, cl                              # x >> i
    mov edx, ebp
    sar edx, cl                              # y >> i
    mov r10d, [r9 + rcx*4]                   # atan[i] (Q30)
    test r8d, r8d
    js .Lc_neg
    sub ebx, edx                             # x -= y>>i
    add ebp, eax                             # y += x>>i
    sub r8d, r10d                            # z -= atan[i]
    jmp .Lc_next
.Lc_neg:
    add ebx, edx
    sub ebp, eax
    add r8d, r10d
.Lc_next:
    inc ecx
    jmp .Lc_loop
.Lc_end:
    test r11d, r11d                          # квадрантный флип: neg ОБЕИХ
    jz 4f
    neg ebx
    neg ebp
4:
    cvtsi2ss xmm1, ebx
    mulss xmm1, [rip + .Lc_2n30]
    cvtsi2ss xmm2, ebp
    mulss xmm2, [rip + .Lc_2n30]
    vmovss [rdi], xmm1                       # cos
    vmovss [rsi], xmm2                       # sin
    pop rbp
    pop rbx
    ret

    # ================================================================
    # float poler_cordic_atan2(float y, float x)  -> atan2(y, x)
    #   xmm0=y, xmm1=x -> xmm0
    # v0.88: квадрант — поворот вектора на ±90° (x<0), НО сдвиг фазы
    #   добавляется в FLOAT-домене в конце: z-регистр стартует с НУЛЯ
    #   (остаток ∈ [−90°, 90°] → |z| ≤ (π/2)·2³⁰ = 1.687e9 < 2³¹ —
    #   гарантированно без переполнения; инициализация z = ±(π/2)·2³⁰
    #   плюс остаток до 90° переполняла знаковый i32 на 2³² → ошибка
    #   ровно 4.0 рад). Координаты масштабируются в Q26.
    # ================================================================
    .globl poler_cordic_atan2
    .p2align 4
poler_cordic_atan2:
    push rbx
    push rbp
    # --- масштаб в Q26: s = 2^26 / max(|x|,|y|) (rcpss + Ньютон) ---
    vandps xmm2, xmm0, [rip + .Lc_abs]
    vandps xmm3, xmm1, [rip + .Lc_abs]
    vmaxss xmm2, xmm2, xmm3
    vmaxss xmm2, xmm2, [rip + .Lc_tiny]
    vrcpss xmm4, xmm4, xmm2                # r ≈ 1/m (src2 — заглушка)
    # Ньютон: r ← r·(2 − m·r)
    vmovss xmm5, [rip + .Lc_two]
    vmulss xmm6, xmm2, xmm4                 # m·r
    vsubss xmm5, xmm5, xmm6                 # 2 − m·r
    vmulss xmm4, xmm4, xmm5                 # r·(2 − m·r)
    vmulss xmm4, xmm4, [rip + .Lc_q26]
    vmulss xmm0, xmm0, xmm4                  # y·s
    vmulss xmm1, xmm1, xmm4                  # x·s
    vcvttss2si r10d, xmm0                    # yi (Q26)
    vcvttss2si r11d, xmm1                    # xi (Q26)
    # --- квадрант: x < 0 → поворот ±90°, z = 0, маркер сдвига в r11d ---
    xor r8d, r8d                             # z = 0
    test r11d, r11d
    jns .La_noquad
    test r10d, r10d                          # знак исходного y
    js .La_negq
    # y >= 0: (x, y) <- (y, −x); финал += π/2 (float)
    neg r11d
    xchg r10d, r11d
    mov ebx, r11d                            # x' = y
    mov ebp, r10d                            # y' = −x
    mov r11d, 1                              # маркер: +π/2
    jmp 6f
.La_negq:
    # y < 0: (x, y) <- (−y, x); финал −= π/2 (float)
    neg r10d
    xchg r10d, r11d
    mov ebx, r11d                            # x' = −y
    mov ebp, r10d                            # y' = x
    mov r11d, 2                              # маркер: −π/2
    jmp 6f
.La_noquad:
    mov ebx, r11d
    mov ebp, r10d
    xor r11d, r11d                           # маркер 0
6:
    # --- векторинг: y -> 0, z -> остаточный угол ---
    lea r9, [rip + .Lcordic_atan30]
    xor ecx, ecx
.La_loop:
    cmp ecx, 30
    jae .La_end
    mov eax, ebx
    sar eax, cl
    mov edx, ebp
    sar edx, cl
    mov r10d, [r9 + rcx*4]
    test ebp, ebp
    jns .La_pos
    sub ebx, edx                             # x -= y>>i
    add ebp, eax                             # y += x>>i
    sub r8d, r10d
    jmp .La_next
.La_pos:
    add ebx, edx
    sub ebp, eax
    add r8d, r10d
.La_next:
    inc ecx
    jmp .La_loop
.La_end:
    cvtsi2ss xmm0, r8d
    mulss xmm0, [rip + .Lc_2n30]
    # квадрантный сдвиг фазы — в FLOAT-домене (z-регистр не переполняется)
    cmp r11d, 1
    jne 7f
    vaddss xmm0, xmm0, [rip + .Lc_hpi]
    jmp 8f
7:
    cmp r11d, 2
    jne 8f
    vsubss xmm0, xmm0, [rip + .Lc_hpi]
8:
    pop rbp
    pop rbx
    ret
    "#
}

extern "sysv64" {
    fn poler_cordic_sincos(angle: f32, cos_out: *mut f32, sin_out: *mut f32);
    fn poler_cordic_atan2(y: f32, x: f32) -> f32;
}

/// (cos, sin) угла в радианах — детерминированный CORDIC Q30.
pub fn sincos(angle: f32) -> (f32, f32) {
    let mut c = 0.0f32;
    let mut s = 0.0f32;
    unsafe { poler_cordic_sincos(angle, &mut c, &mut s) };
    (c, s)
}

/// atan2(y, x) — векторинг-режим CORDIC Q26/квадранты ±90° (Q30-углы).
pub fn atan2(y: f32, x: f32) -> f32 {
    unsafe { poler_cordic_atan2(y, x) }
}

/// Ренормализация массива фаз (re, im) на единичную окружность S¹:
/// atan2 → sincos → перезапись. Ноль-вектор остаётся нулём
/// (atan2(0,0) = 0 → (1, 0); документированная семантика ротора).
pub fn renorm(re: &mut [f32], im: &mut [f32]) {
    for i in 0..re.len().min(im.len()) {
        let a = atan2(im[i], re[i]);
        let (c, s) = sincos(a);
        re[i] = c;
        im[i] = s;
    }
}

// ------------------------------ тесты ------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cordic_sincos_matches_libm() {
        let mut worst = 0.0f32;
        let mut worst_a = 0.0f32;
        let mut k = 0u64;
        // сетка + случайные большие углы (проверка редукции mod 2π)
        let mut angles: Vec<f32> = (0..400)
            .map(|i| -7.0 + i as f32 * 0.035)
            .collect();
        angles.extend_from_slice(&[100.0, -100.0, 1234.5, -999.25, 6.2831853, -6.2831853]);
        for &a in &angles {
            let (c, s) = sincos(a);
            let ec = (c - a.cos()).abs();
            let es = (s - a.sin()).abs();
            if ec.max(es) > worst {
                worst = ec.max(es);
                worst_a = a;
            }
            k += 1;
        }
        // Q30: погрешность квантования ~2e-4 (было 2e-3 на Q24)
        assert!(worst < 5e-4, "CORDIC sincos err {worst} at {worst_a} ({k} углов)");
    }

    #[test]
    fn cordic_atan2_matches_libm() {
        let cases: Vec<(f32, f32)> = vec![
            (1.0, 1.0),
            (1.0, -1.0),
            (-1.0, 1.0),
            (-1.0, -1.0),
            (0.3, 5.0),
            (-0.3, 5.0),
            (5.0, 0.3),
            (-5.0, -0.3),
            (0.0, 1.0),
            (0.0, -1.0),
            (1e-6, 1e-6),
            (1e5, 1e5),
        ];
        for &(y, x) in &cases {
            let a = atan2(y, x);
            let r = y.atan2(x);
            let d = (a - r).abs().min((a - r + std::f32::consts::TAU).abs());
            // Q26/Q30: погрешность ~3e-5 (было 3e-3 на Q24/Q22)
            assert!(d < 1e-3, "atan2({y},{x}): asm={a} vs libm={r}");
        }
    }

    #[test]
    fn cordic_renorm_puts_phases_on_unit_circle() {
        let mut re = vec![3.0f32, 0.5, -2.0, 1e-6];
        let mut im = vec![4.0f32, -0.5, 0.1, 1e-6];
        renorm(&mut re, &mut im);
        for i in 0..re.len() {
            let m = (re[i] * re[i] + im[i] * im[i]).sqrt();
            assert!((m - 1.0).abs() < 2e-4, "norm[{i}]={m}");
            // направление сохранено
            let orig = (3.0f32, 4.0f32);
            if i == 0 {
                let d = (im[i] * orig.0 - re[i] * orig.1).abs();
                assert!(d < 0.01, "фаза потеряна: d={d}");
            }
        }
    }

    #[test]
    fn cordic_is_bit_deterministic() {
        // одинаковое состояние -> одинаковые биты (никаких libm-разногласий)
        let a = 1.2345f32;
        let r1 = sincos(a);
        let r2 = sincos(a);
        assert_eq!(r1.0.to_bits(), r2.0.to_bits());
        assert_eq!(r1.1.to_bits(), r2.1.to_bits());
    }
}
