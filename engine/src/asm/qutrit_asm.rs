//! QUTRIT-ASM — ℤ₃-фазовая физика процессора (v0.88.0).
//!
//! Кутрит живёт в ℂ³, а его фазовая симметрия — цикл ω = e^{i2π/3}
//! (шаг ровно 120°, кольцо вычетов ℤ₃). Умножение на ω:
//!
//! ```text
//! x' = −x/2 − (√3/2)·y        = −0.5·x − 0.8660254037844386·y
//! y' = (√3/2)·x − y/2         =  0.8660254037844386·x − 0.5·y
//! ```
//!
//! Деление на 2 — арифметический сдвиг; умножение на константу √3/2 —
//! одна `vmulps`/`vpmulhw` (никаких `div`/`sqrt`/libm). Три поворота
//! дают тождество: ω³ = 1 — квантование фазы тритом t·2π/3 замыкает
//! мост «сбалансированный трит → точка на S¹ ⊂ ℂ³» (генераторы Вейля
//! X: циклическая перестановка состояний — уже живёт в synapse_asm
//! как примитив `delay`/vpermilps; Z: ω-фаза — этот модуль).
//!
//! | Ядро | Формат | Базис |
//! |------|--------|-------|
//! | `poler_qutrit_omega_f32` | f32, 8 lane | AVX2+FMA (`vfnmadd231ps`) |
//! | `poler_qutrit_omega_q15` | Q14 i16, 8 lane | SSE2 (`pmulhw`+`psraw`) |
//! | `poler_qutrit_project` | трит → (re, im) на S¹ | скаляр, таблица 3 ячеек |
//!
//! ## Константы — Калькулятор Всего движка (ноль Python)
//!
//! ```text
//! poler-engine --exec 'calc sqrt(3)/2'      → 0.8660254037844386
//! poler-engine --exec 'calc (2-sqrt(3))*2^16' → 17560.318275166064 → 17560
//! poler-engine --exec 'calc (-1/2)^3 - 3*(-1/2)*(sqrt(3)/2)^2'
//!                                            → 0.9999999999999998  (ω³ = 1 ✓)
//! ```
//!
//! ## Почему M = (2−√3)·2¹⁶ = 17560, а не √3·2¹⁶
//!
//! `pmulhw` — ЗНАКОВОЕ умножение i16: √3·2¹⁶ = 113512 не влезает
//! в знаковый 16-битный диапазон (уходит в −48024). Тождество
//! √3 = 2 − (2−√3) даёт константу 17560 ∈ i16, а умножение на √3
//! превращается в `удвоение − pmulhw(2−√3)`; точность 2.8e-6 —
//! на уровне квантования Q14.
//!
//! Q14-контракт `omega_q15`: |x|, |y| ≤ 16000 (удвоение в i16 без
//! переполнения + запас под рост амплитуды).

use std::arch::global_asm;

/// √3/2 — компонент ω (Калькулятор Всего: 0.8660254037844386).
pub const QUTRIT_C: f64 = 0.8660254037844386;
/// Q14-масштаб фиксированной точки omega_q15.
pub const QUTRIT_Q14: i32 = 16384;

global_asm! {
    r#"
    .text
    .p2align 5
.Lq_c:
    .float 0.8660254037844386, 0.8660254037844386, 0.8660254037844386, 0.8660254037844386
    .float 0.8660254037844386, 0.8660254037844386, 0.8660254037844386, 0.8660254037844386
    .p2align 5
.Lq_negc:
    .float -0.8660254037844386, -0.8660254037844386, -0.8660254037844386, -0.8660254037844386
    .float -0.8660254037844386, -0.8660254037844386, -0.8660254037844386, -0.8660254037844386
    .p2align 5
.Lq_half:
    .float 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5
    .p2align 4
    # M = (2−√3)·2^16 = 17560 (calc (2-sqrt(3))*2^16) — влезает в ЗНАКОВЫЙ
    # i16 (√3·2^16 = 113512 — не влезает!); √3 = 2 − (2−√3)
.Lq3_2msqrt3:
    .word 17560, 17560, 17560, 17560, 17560, 17560, 17560, 17560
    .p2align 3
    # Фазовая таблица моста трит→S¹: (re, im) для t = −1, 0, +1
    # θ_t = t·2π/3:  t=−1 → (−0.5, −√3/2); t=0 → (1, 0); t=+1 → (−0.5, +√3/2)
.Lq_ph:
    .float -0.5, -0.8660254037844386
    .float 1.0, 0.0
    .float -0.5, 0.8660254037844386

    .text
    # ================================================================
    # void poler_qutrit_omega_f32(f32* re_io, f32* im_io, size_t n)
    #   rdi=re, rsi=im, rdx=n.  In-place поворот на ω (120°):
    #   x' = −c·im − 0.5·re;  y' = c·re − 0.5·im.  Bulk кратно 8.
    # ================================================================
    .globl poler_qutrit_omega_f32
    .p2align 4
poler_qutrit_omega_f32:
    xor rcx, rcx
    test rdx, rdx
    jz .Lqw_fin
.Lqw_lp:
    mov rax, rdx
    sub rax, rcx
    cmp rax, 8
    jb .Lqw_fin
    vmovups ymm0, [rdi + rcx*4]            # re
    vmovups ymm1, [rsi + rcx*4]            # im
    vmulps ymm2, ymm1, [rip + .Lq_negc]    # −c·im
    vfnmadd231ps ymm2, ymm0, [rip + .Lq_half]  # x' = −c·im − 0.5·re
    vmulps ymm3, ymm0, [rip + .Lq_c]       # c·re
    vfnmadd231ps ymm3, ymm1, [rip + .Lq_half]  # y' = c·re − 0.5·im
    vmovups [rdi + rcx*4], ymm2
    vmovups [rsi + rcx*4], ymm3
    add rcx, 8
    jmp .Lqw_lp
.Lqw_fin:
    vzeroupper
    ret

    # ================================================================
    # void poler_qutrit_omega_q15(i16* x_io, i16* y_io, size_t n)
    #   rdi=x, rsi=y, rdx=n.  Q14, in-place, БЕЗ умножителя/делений:
    #   √3 = 2 − (2−√3): v·√3 = 2v − pmulhw(v, 17560).
    #   Семантика (арифм. сдвиги — усечение к −∞, задокументировано):
    #     t1/2 = ((2y − pmulhw(y,M)) >> 1)
    #     t0/2 = ((2x − pmulhw(x,M)) >> 1)
    #     x'   = −((x>>1) + t1/2);   y' = t0/2 − (y>>1)
    #   Голый SSE2. Bulk кратно 8. КОНТРАКТ: |x|,|y| ≤ 16000.
    # ================================================================
    .globl poler_qutrit_omega_q15
    .p2align 4
poler_qutrit_omega_q15:
    movdqa xmm7, [rip + .Lq3_2msqrt3]
    xor rcx, rcx
    test rdx, rdx
    jz .Lqw15_fin
.Lqw15_lp:
    mov rax, rdx
    sub rax, rcx
    cmp rax, 8
    jb .Lqw15_fin
    movdqu xmm0, [rdi + rcx*2]     # x (Q14)
    movdqu xmm1, [rsi + rcx*2]     # y (Q14)
    # t1 = y·√3 = 2y − pmulhw(y, M); затем t1/2
    movdqa xmm2, xmm1
    pmulhw xmm2, xmm7              # y·(2−√3)
    movdqa xmm3, xmm1
    paddw xmm3, xmm3               # 2y
    psubw xmm3, xmm2               # t1 = y·√3
    psraw xmm3, 1                  # t1/2
    # t0 = x·√3 = 2x − pmulhw(x, M); затем t0/2
    movdqa xmm4, xmm0
    pmulhw xmm4, xmm7              # x·(2−√3)
    movdqa xmm5, xmm0
    paddw xmm5, xmm5               # 2x
    psubw xmm5, xmm4               # t0 = x·√3
    psraw xmm5, 1                  # t0/2
    # x' = −(x/2 + t1/2)
    movdqa xmm6, xmm0
    psraw xmm6, 1                  # x/2
    paddw xmm6, xmm3               # x/2 + t1/2
    pxor xmm2, xmm2
    psubw xmm2, xmm6               # x'
    # y' = t0/2 − y/2
    movdqa xmm6, xmm1
    psraw xmm6, 1                  # y/2
    psubw xmm5, xmm6               # y'
    movdqu [rdi + rcx*2], xmm2
    movdqu [rsi + rcx*2], xmm5
    add rcx, 8
    jmp .Lqw15_lp
.Lqw15_fin:
    ret

    # ================================================================
    # void poler_qutrit_project(const i8* trits, f32* re, f32* im,
    #                           size_t n)
    #   rdi=trits, rsi=re, rdx=im, rcx=n.
    #   Мост трит→кутрит: θ_t = t·2π/3 → точка на S¹. Табличная
    #   адресация (t+1)·8 — без ветвлений и тригонометрии.
    #   КОНТРАКТ: trits[i] ∈ {{−1, 0, +1}}.
    # ================================================================
    .globl poler_qutrit_project
    .p2align 4
poler_qutrit_project:
    xor r8, r8
    test rcx, rcx
    jz .Lqp_fin
.Lqp_lp:
    cmp r8, rcx
    jae .Lqp_fin
    movsx eax, byte ptr [rdi + r8]
    inc eax                       # idx = t + 1 ∈ {{0, 1, 2}}
    lea r9, [rip + .Lq_ph]
    mov rax, [r9 + rax*8]         # 64 бита: (re, im)
    mov r9d, eax
    mov [rsi + r8*4], r9d
    shr rax, 32
    mov [rdx + r8*4], eax
    inc r8
    jmp .Lqp_lp
.Lqp_fin:
    ret
    "#
}

extern "sysv64" {
    fn poler_qutrit_omega_f32(re: *mut f32, im: *mut f32, n: usize);
    fn poler_qutrit_omega_q15(x: *mut i16, y: *mut i16, n: usize);
    fn poler_qutrit_project(trits: *const i8, re: *mut f32, im: *mut f32, n: usize);
}

/// Поворот фазового массива на ω = e^{i2π/3} (float, AVX2+FMA).
pub fn omega_f32(re: &mut [f32], im: &mut [f32]) {
    let n = re.len().min(im.len());
    let c = crate::asm::caps();
    let bulk = if c.avx2 && c.fma { n / 8 * 8 } else { 0 };
    if bulk > 0 {
        unsafe { poler_qutrit_omega_f32(re.as_mut_ptr(), im.as_mut_ptr(), bulk) };
    }
    for i in bulk..n {
        let (x, y) = (re[i], im[i]);
        re[i] = (-0.5f32) * x - (QUTRIT_C as f32) * y;
        im[i] = (QUTRIT_C as f32) * x - 0.5f32 * y;
    }
}

/// Скалярный эталон ω-поворота (f32).
pub fn omega_f32_scalar(re: &mut [f32], im: &mut [f32]) {
    for i in 0..re.len().min(im.len()) {
        let (x, y) = (re[i], im[i]);
        re[i] = -0.5 * x - QUTRIT_C as f32 * y;
        im[i] = QUTRIT_C as f32 * x - 0.5 * y;
    }
}

/// Q14-шаг pmulhw: старшие 16 бит знакового произведения.
#[inline]
fn mulhw(a: i16, b: i16) -> i16 {
    (((a as i32) * (b as i32)) >> 16) as i16
}

/// Поворот на ω в Q14 (голый SSE2 — работает на любом x86_64).
///
/// Контракт: |x|, |y| ≤ 16000. Зеркалит сдвиги ядра точно
/// (усечение к −∞ на каждом psraw/pmulhw).
pub fn omega_q15(x: &mut [i16], y: &mut [i16]) {
    let n = x.len().min(y.len());
    let bulk = n / 8 * 8;
    if bulk > 0 {
        unsafe { poler_qutrit_omega_q15(x.as_mut_ptr(), y.as_mut_ptr(), bulk) };
    }
    for i in bulk..n {
        let (xi, yi) = (x[i], y[i]);
        let m = |v: i16| mulhw(v, 17560);
        // t1/2 = (2y − pmulhw(y,M)) >> 1 — y·√3/2
        let t1h = yi.wrapping_add(yi).wrapping_sub(m(yi)) >> 1;
        // t0/2 = (2x − pmulhw(x,M)) >> 1 — x·√3/2
        let t0h = xi.wrapping_add(xi).wrapping_sub(m(xi)) >> 1;
        x[i] = (xi >> 1).wrapping_add(t1h).wrapping_neg();
        y[i] = t0h.wrapping_sub(yi >> 1);
    }
}

/// Мост трит→кутрит: проекция трита на S¹ (таблица θ_t = t·2π/3).
pub fn project(trits: &[i8], re: &mut [f32], im: &mut [f32]) {
    let n = trits.len().min(re.len()).min(im.len());
    if n == 0 {
        return;
    }
    unsafe { poler_qutrit_project(trits.as_ptr(), re.as_mut_ptr(), im.as_mut_ptr(), n) };
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

    #[test]
    fn omega_f32_cubed_is_identity() {
        // ω³ = 1: три поворота возвращают вектор (ℤ₃-цикл)
        let mut s = 42u64 | 1;
        let mut re: Vec<f32> = (0..1000)
            .map(|_| ((xs64(&mut s) >> 33) % 200) as f32 / 100.0 - 1.0)
            .collect();
        let mut im: Vec<f32> = (0..1000)
            .map(|_| ((xs64(&mut s) >> 33) % 200) as f32 / 100.0 - 1.0)
            .collect();
        let (re0, im0) = (re.clone(), im.clone());
        for _ in 0..3 {
            omega_f32(&mut re, &mut im);
        }
        let mut worst = 0.0f32;
        for i in 0..1000 {
            worst = worst.max((re[i] - re0[i]).abs()).max((im[i] - im0[i]).abs());
        }
        assert!(worst < 1e-5, "ω³≠I: worst={worst}");
    }

    #[test]
    fn omega_f32_matches_scalar() {
        let mut s = 7u64 | 1;
        let mut re: Vec<f32> = (0..777)
            .map(|_| ((xs64(&mut s) >> 33) % 200) as f32 / 100.0 - 1.0)
            .collect();
        let mut im: Vec<f32> = (0..777)
            .map(|_| ((xs64(&mut s) >> 33) % 200) as f32 / 100.0 - 1.0)
            .collect();
        let mut re2 = re.clone();
        let mut im2 = im.clone();
        omega_f32(&mut re, &mut im);
        omega_f32_scalar(&mut re2, &mut im2);
        let mut worst = 0.0f32;
        for i in 0..777 {
            worst = worst.max((re[i] - re2[i]).abs()).max((im[i] - im2[i]).abs());
        }
        assert!(worst < 1e-6, "asm vs scalar: {worst}");
    }

    #[test]
    fn omega_f32_preserves_norm() {
        // |ω·z| = |z| — унитарность поворота
        let mut s = 99u64 | 1;
        let mut re: Vec<f32> = (0..500)
            .map(|_| ((xs64(&mut s) >> 33) % 200) as f32 / 100.0 - 1.0)
            .collect();
        let mut im: Vec<f32> = (0..500)
            .map(|_| ((xs64(&mut s) >> 33) % 200) as f32 / 100.0 - 1.0)
            .collect();
        let n0: Vec<f32> = re
            .iter()
            .zip(&im)
            .map(|(&x, &y)| (x * x + y * y).sqrt())
            .collect();
        omega_f32(&mut re, &mut im);
        for i in 0..500 {
            let n1 = (re[i] * re[i] + im[i] * im[i]).sqrt();
            assert!((n1 - n0[i]).abs() < 1e-5, "норма не сохранена [{i}]");
        }
    }

    #[test]
    fn omega_q15_cubed_is_identity() {
        // Q14-ротор: три шага → возврат с погрешностью усечения (≤ 6 LSB)
        let mut s = 13u64 | 1;
        let mut x: Vec<i16> = (0..512)
            .map(|_| (((xs64(&mut s) >> 33) % 200) as i32 - 100) * 160 / 1000)
            .map(|v| v.clamp(-16000, 16000) as i16)
            .collect();
        let mut y: Vec<i16> = (0..512)
            .map(|_| (((xs64(&mut s) >> 33) % 200) as i32 - 100) * 160 / 1000)
            .map(|v| v.clamp(-16000, 16000) as i16)
            .collect();
        let (x0, y0) = (x.clone(), y.clone());
        for _ in 0..3 {
            omega_q15(&mut x, &mut y);
        }
        let mut worst = 0i32;
        for i in 0..512 {
            worst = worst.max((x[i] - x0[i]).abs() as i32).max((y[i] - y0[i]).abs() as i32);
        }
        assert!(worst <= 8, "ω³≠I (Q14): worst={worst} LSB");
    }

    #[test]
    fn omega_q15_matches_f32() {
        // паритет форматов: один поворот, допуск квантования Q14
        let mut s = 21u64 | 1;
        let mut x: Vec<i16> = (0..256)
            .map(|_| (((xs64(&mut s) >> 33) % 200) as i32 - 100) * 120 / 1000)
            .map(|v| v.clamp(-12000, 12000) as i16)
            .collect();
        let mut y: Vec<i16> = (0..256)
            .map(|_| (((xs64(&mut s) >> 33) % 200) as i32 - 100) * 120 / 1000)
            .map(|v| v.clamp(-12000, 12000) as i16)
            .collect();
        let mut re: Vec<f32> = x.iter().map(|&v| v as f32 / QUTRIT_Q14 as f32).collect();
        let mut im: Vec<f32> = y.iter().map(|&v| v as f32 / QUTRIT_Q14 as f32).collect();
        omega_q15(&mut x, &mut y);
        omega_f32(&mut re, &mut im);
        for i in 0..256 {
            let dx = (x[i] as f32 / QUTRIT_Q14 as f32 - re[i]).abs();
            let dy = (y[i] as f32 / QUTRIT_Q14 as f32 - im[i]).abs();
            assert!(dx < 2e-3 && dy < 2e-3, "Q14 vs f32 [{i}]: {dx} {dy}");
        }
    }

    #[test]
    fn project_maps_trits_on_unit_circle() {
        let trits: Vec<i8> = vec![-1, 0, 1, -1, 0, 1];
        let mut re = vec![0.0f32; 6];
        let mut im = vec![0.0f32; 6];
        project(&trits, &mut re, &mut im);
        for (i, &t) in trits.iter().enumerate() {
            let m = (re[i] * re[i] + im[i] * im[i]).sqrt();
            assert!((m - 1.0).abs() < 1e-6, "|p[{i}]|={m}");
            match t {
                -1 => {
                    assert!((re[i] + 0.5).abs() < 1e-6 && (im[i] + QUTRIT_C as f32).abs() < 1e-5)
                }
                0 => assert!((re[i] - 1.0).abs() < 1e-6 && im[i].abs() < 1e-6),
                _ => {
                    assert!((re[i] + 0.5).abs() < 1e-6 && (im[i] - QUTRIT_C as f32).abs() < 1e-5)
                }
            }
        }
    }

    #[test]
    fn project_plus_omega_is_z3_cycle() {
        // МАТЕМАТИКА ℤ₃ НА ЯДРАХ: ω·project(t) = project(t + 1 mod 3)
        // θ_{t+1} = θ_t + 2π/3 — поворот ω переводит трит в следующий.
        for t in -1i8..=1 {
            let trits = vec![t];
            let mut re = vec![0.0f32];
            let mut im = vec![0.0f32];
            project(&trits, &mut re, &mut im);
            omega_f32(&mut re, &mut im);
            let t_next = if t == 1 { -1 } else { t + 1 };
            let mut re2 = vec![0.0f32];
            let mut im2 = vec![0.0f32];
            project(&[t_next], &mut re2, &mut im2);
            assert!(
                (re[0] - re2[0]).abs() < 1e-5 && (im[0] - im2[0]).abs() < 1e-5,
                "ω·θ{t} ≠ θ{t_next}: ({}, {}) vs ({}, {})",
                re[0], im[0], re2[0], im2[0]
            );
        }
    }
}
