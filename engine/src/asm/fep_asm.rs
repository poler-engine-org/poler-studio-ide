//! FEP-контур сокровищницы в машинном коде x86_64 (AVX2 + FMA).
//!
//! Точный порт `docs/vault_drafts/poler_quantum/archive/poler-core/src/fep_loss.rs`
//! (принцип активного вывода Карла Фристона) на чистый ассемблер:
//!
//! ```text
//! F(p, o)  = Σᵢ wᵢ·(pᵢ − Ω(o)ᵢ)² + 0.5·λ·Σᵢ pᵢ²
//! ∇F(i)    = 2·wᵢ·(pᵢ − Ω(o)ᵢ) + λ·pᵢ
//! p_new    = p − η·∇F
//! ```
//!
//! `w` — диагональ метрики G; `gw = None` → единичная метрика (указатель
//! NULL подменяется на `.rodata`-массив из 1.0f — внутренний цикл не меняется).
//!
//! Три микроядра:
//! * `poler_fep_energy` — только свободная энергия F;
//! * `poler_fep_grad`   — градиент ∇F без обновления p;
//! * `poler_fep_step`   — полный шаг Фристона (F, ∇F, p −= η·∇F), требует gw.
//!
//! Векторная ширина — 8 lanes f32, хвост — скалярные xmm-итерации
//! (аккумулятор хвоста отдельный, чтобы VEX.128-записи не затирали
//! верхние 128 бит ymm-аккумулятора — грабля Intel: 128-битные записи
//! обнуляют старшую половину YMM).

use std::arch::global_asm;

global_asm! {
    r#"
    .text
    .p2align 5
    # 64×1.0f — единичная метрика G для gw=NULL (лимит: n ≤ 64,
    # больше — обёртка уходит в скалярный эталон; literary dims=64)
.Lfep_one:
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .long 0x3F800000, 0x3F800000, 0x3F800000, 0x3F800000
    .p2align 5
    # 8×0.5f — для 0.5·λ
.Lfep_half:
    .long 0x3F000000, 0x3F000000, 0x3F000000, 0x3F000000
    .long 0x3F000000, 0x3F000000, 0x3F000000, 0x3F000000

    .text
    # ================================================================
    # float poler_fep_energy(const f32* p, const f32* o,
    #                        const f32* gw|null, size_t n, float lambda)
    #   rdi=p, rsi=o, rdx=gw, rcx=n, xmm0=lambda -> xmm0 = F
    # ================================================================
    .globl poler_fep_energy
    .p2align 4
poler_fep_energy:
    # gw=NULL -> указатель на массив единиц (G = I)
    test rdx, rdx
    jnz 1f
    lea rdx, [rip + .Lfep_one]
1:
    vbroadcastss ymm9, xmm0            # lambda
    vmulps ymm9, ymm9, [rip + .Lfep_half]  # 0.5*lambda
    vxorps ymm2, ymm2, ymm2              # F (векторный аккумулятор)
    vxorps xmm8, xmm8, xmm8              # F (скалярный хвост)
    xor r9, r9                           # i = 0
    test rcx, rcx
    jz .Lfep_energy_red
.Lfep_energy_l8:
    mov rax, rcx
    sub rax, r9
    cmp rax, 8
    jb .Lfep_energy_tail
    vmovups ymm0, [rdi + r9*4]           # p
    vmovups ymm1, [rsi + r9*4]           # o
    vmovups ymm3, [rdx + r9*4]           # gw
    vsubps ymm6, ymm0, ymm1              # r = p - o
    vmulps ymm7, ymm6, ymm6              # r^2
    vfmadd231ps ymm2, ymm7, ymm3         # F += r^2 * gw
    vmulps ymm7, ymm0, ymm0              # p^2
    vfmadd231ps ymm2, ymm7, ymm9         # F += 0.5*lambda*p^2
    add r9, 8
    jmp .Lfep_energy_l8
.Lfep_energy_tail:
    cmp r9, rcx
    jae .Lfep_energy_red
    vmovss xmm0, [rdi + r9*4]
    vmovss xmm1, [rsi + r9*4]
    vmovss xmm3, [rdx + r9*4]
    vsubss xmm6, xmm0, xmm1
    vmulss xmm7, xmm6, xmm6
    vfmadd231ss xmm8, xmm7, xmm3         # хвост в отдельный аккумулятор!
    vmulss xmm7, xmm0, xmm0
    vfmadd231ss xmm8, xmm7, xmm9
    inc r9
    jmp .Lfep_energy_tail
.Lfep_energy_red:
    vextractf128 xmm1, ymm2, 1
    vaddps xmm2, xmm2, xmm1
    vhaddps xmm2, xmm2, xmm2
    vhaddps xmm2, xmm2, xmm2
    vaddss xmm0, xmm2, xmm8
    vzeroupper
    ret

    # ================================================================
    # void poler_fep_grad(const f32* p, const f32* o, const f32* gw|null,
    #                     f32* grad, size_t n, float lambda)
    #   rdi=p, rsi=o, rdx=gw, rcx=grad, r8=n, xmm0=lambda
    # ================================================================
    .globl poler_fep_grad
    .p2align 4
poler_fep_grad:
    test rdx, rdx
    jnz 1f
    lea rdx, [rip + .Lfep_one]
1:
    vbroadcastss ymm4, xmm0            # lambda
    xor r9, r9
    test r8, r8
    jz .Lfep_grad_done
.Lfep_grad_l8:
    mov rax, r8
    sub rax, r9
    cmp rax, 8
    jb .Lfep_grad_tail
    vmovups ymm0, [rdi + r9*4]
    vmovups ymm1, [rsi + r9*4]
    vmovups ymm3, [rdx + r9*4]
    vsubps ymm6, ymm0, ymm1              # r
    vaddps ymm7, ymm6, ymm6              # 2r
    vmulps ymm7, ymm7, ymm3              # 2r*gw
    vfmadd231ps ymm7, ymm0, ymm4         # + lambda*p
    vmovups [rcx + r9*4], ymm7
    add r9, 8
    jmp .Lfep_grad_l8
.Lfep_grad_tail:
    cmp r9, r8
    jae .Lfep_grad_done
    vmovss xmm0, [rdi + r9*4]
    vmovss xmm1, [rsi + r9*4]
    vmovss xmm3, [rdx + r9*4]
    vsubss xmm6, xmm0, xmm1
    vaddss xmm7, xmm6, xmm6
    vmulss xmm7, xmm7, xmm3
    vfmadd231ss xmm7, xmm0, xmm4
    vmovss [rcx + r9*4], xmm7
    inc r9
    jmp .Lfep_grad_tail
.Lfep_grad_done:
    ret

    # ================================================================
    # float poler_fep_step(f32* p_io, const f32* o, const f32* gw,
    #                      f32* grad, size_t n, float lambda, float eta)
    #   rdi=p, rsi=o, rdx=gw, rcx=grad, r8=n, xmm0=lambda, xmm1=eta
    #   -> xmm0 = F (до шага); p обновлён на месте
    # ================================================================
    .globl poler_fep_step
    .p2align 4
poler_fep_step:
    vbroadcastss ymm4, xmm0            # lambda
    vbroadcastss ymm5, xmm1            # eta
    vmovaps ymm9, ymm4
    vmulps ymm9, ymm9, [rip + .Lfep_half]  # 0.5*lambda
    vxorps ymm2, ymm2, ymm2              # F (вектор)
    vxorps xmm8, xmm8, xmm8              # F (хвост)
    xor r9, r9
    test r8, r8
    jz .Lfep_step_red
.Lfep_step_l8:
    mov rax, r8
    sub rax, r9
    cmp rax, 8
    jb .Lfep_step_tail
    vmovups ymm0, [rdi + r9*4]           # p
    vmovups ymm1, [rsi + r9*4]           # o
    vmovups ymm3, [rdx + r9*4]           # gw
    vsubps ymm6, ymm0, ymm1              # r
    vmulps ymm7, ymm6, ymm6              # r^2
    vfmadd231ps ymm2, ymm7, ymm3         # F += r^2*gw
    vmulps ymm7, ymm0, ymm0              # p^2
    vfmadd231ps ymm2, ymm7, ymm9         # F += 0.5*lambda*p^2
    # grad = 2*gw*r + lambda*p
    vaddps ymm7, ymm6, ymm6
    vmulps ymm7, ymm7, ymm3
    vfmadd231ps ymm7, ymm0, ymm4
    vmovups [rcx + r9*4], ymm7
    # p -= eta*grad  (vfnmadd231: dst = -(src2*src3) + dst)
    vfnmadd231ps ymm0, ymm5, ymm7
    vmovups [rdi + r9*4], ymm0
    add r9, 8
    jmp .Lfep_step_l8
.Lfep_step_tail:
    cmp r9, r8
    jae .Lfep_step_red
    vmovss xmm0, [rdi + r9*4]
    vmovss xmm1, [rsi + r9*4]
    vmovss xmm3, [rdx + r9*4]
    vsubss xmm6, xmm0, xmm1
    vmulss xmm7, xmm6, xmm6
    vfmadd231ss xmm8, xmm7, xmm3
    vmulss xmm7, xmm0, xmm0
    vfmadd231ss xmm8, xmm7, xmm9
    vaddss xmm7, xmm6, xmm6
    vmulss xmm7, xmm7, xmm3
    vfmadd231ss xmm7, xmm0, xmm4
    vmovss [rcx + r9*4], xmm7
    vfnmadd231ss xmm0, xmm5, xmm7
    vmovss [rdi + r9*4], xmm0
    inc r9
    jmp .Lfep_step_tail
.Lfep_step_red:
    vextractf128 xmm1, ymm2, 1
    vaddps xmm2, xmm2, xmm1
    vhaddps xmm2, xmm2, xmm2
    vhaddps xmm2, xmm2, xmm2
    vaddss xmm0, xmm2, xmm8
    vzeroupper
    ret
    "#
}

extern "sysv64" {
    fn poler_fep_energy(
        p: *const f32,
        o: *const f32,
        gw: *const f32,
        n: usize,
        lambda: f32,
    ) -> f32;
    fn poler_fep_grad(
        p: *const f32,
        o: *const f32,
        gw: *const f32,
        grad: *mut f32,
        n: usize,
        lambda: f32,
    );
    fn poler_fep_step(
        p: *mut f32,
        o: *const f32,
        gw: *const f32,
        grad: *mut f32,
        n: usize,
        lambda: f32,
        eta: f32,
    ) -> f32;
}

/// Готовность AVX2+FMA (детекция в [`crate::asm::caps`]).
#[inline]
fn ready() -> bool {
    let c = crate::asm::caps();
    c.avx2 && c.fma
}

/// Лимит identity-таблицы в .rodata (64 float).
const IDENTITY_G_MAX: usize = 64;

/// Свободная энергия F (порт FEPLoss::compute_loss_and_grad, энергия).
///
/// `gw = None` → единичная метрика G (n ≤ 64, больше — скалярный
/// эталон: .rodata-таблица конечна). Формула сокровищницы:
/// `F = Σ w·(p−o)² + 0.5·λ·Σ p²`.
pub fn energy(p: &[f32], o: &[f32], gw: Option<&[f32]>, lambda: f32) -> f32 {
    let n = p.len().min(o.len());
    if n == 0 {
        return 0.0;
    }
    let asm_ok = gw.is_some() || n <= IDENTITY_G_MAX;
    if asm_ok && ready() {
        unsafe {
            poler_fep_energy(
                p.as_ptr(),
                o.as_ptr(),
                gw.map_or(std::ptr::null(), |g| g.as_ptr()),
                n,
                lambda,
            )
        }
    } else {
        energy_scalar(&p[..n], &o[..n], gw, lambda)
    }
}

/// Градиент ∇F без обновления состояния (порт FEPLoss, градиент).
///
/// `gw = None` → G = I (n ≤ 64, больше — скалярный эталон): `∇F = 2(p−o) + λ·p`.
pub fn grad(p: &[f32], o: &[f32], gw: Option<&[f32]>, lambda: f32, grad_out: &mut [f32]) {
    let n = p.len().min(o.len()).min(grad_out.len());
    if n == 0 {
        return;
    }
    let asm_ok = gw.is_some() || n <= IDENTITY_G_MAX;
    if asm_ok && ready() {
        unsafe {
            poler_fep_grad(
                p.as_ptr(),
                o.as_ptr(),
                gw.map_or(std::ptr::null(), |g| g.as_ptr()),
                grad_out.as_mut_ptr(),
                n,
                lambda,
            )
        }
    } else {
        grad_scalar(&p[..n], &o[..n], gw, lambda, &mut grad_out[..n]);
    }
}

/// Полный шаг Фристона: возвращает F (до шага), обновляет `p -= eta*∇F`,
/// выписывает ∇F в `grad_out`. Требует метрику `gw` (бенчмарк/канон).
pub fn step(
    p: &mut [f32],
    o: &[f32],
    gw: &[f32],
    grad_out: &mut [f32],
    lambda: f32,
    eta: f32,
) -> f32 {
    let n = p.len().min(o.len()).min(gw.len()).min(grad_out.len());
    if n == 0 {
        return 0.0;
    }
    if ready() {
        unsafe {
            poler_fep_step(
                p.as_mut_ptr(),
                o.as_ptr(),
                gw.as_ptr(),
                grad_out.as_mut_ptr(),
                n,
                lambda,
                eta,
            )
        }
    } else {
        step_scalar(&mut p[..n], &o[..n], gw, &mut grad_out[..n], lambda, eta)
    }
}

// ------------------------- скалярные эталоны -------------------------

/// Скалярный эталон энергии (тот же математический контракт).
pub fn energy_scalar(p: &[f32], o: &[f32], gw: Option<&[f32]>, lambda: f32) -> f32 {
    let mut f = 0.0f32;
    for i in 0..p.len().min(o.len()) {
        let r = p[i] - o[i];
        let w = gw.map_or(1.0f32, |g| g.get(i).copied().unwrap_or(1.0));
        f += w * r * r + 0.5 * lambda * p[i] * p[i];
    }
    f
}

/// Скалярный эталон градиента.
pub fn grad_scalar(
    p: &[f32],
    o: &[f32],
    gw: Option<&[f32]>,
    lambda: f32,
    grad_out: &mut [f32],
) {
    for i in 0..p.len().min(o.len()).min(grad_out.len()) {
        let r = p[i] - o[i];
        let w = gw.map_or(1.0f32, |g| g.get(i).copied().unwrap_or(1.0));
        grad_out[i] = 2.0 * w * r + lambda * p[i];
    }
}

/// Скалярный эталон полного шага.
pub fn step_scalar(
    p: &mut [f32],
    o: &[f32],
    gw: &[f32],
    grad_out: &mut [f32],
    lambda: f32,
    eta: f32,
) -> f32 {
    let mut f = 0.0f32;
    for i in 0..p.len().min(o.len()).min(gw.len()).min(grad_out.len()) {
        let r = p[i] - o[i];
        f += gw[i] * r * r + 0.5 * lambda * p[i] * p[i];
        grad_out[i] = 2.0 * gw[i] * r + lambda * p[i];
        p[i] -= eta * grad_out[i];
    }
    f
}

// ------------------------------ тесты ------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: u64) -> u64 {
        seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)
    }

    fn rand_vec(n: usize, seed: u64) -> Vec<f32> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = lcg(s);
                ((s >> 33) as i32 % 1000) as f32 / 500.0
            })
            .collect()
    }

    #[test]
    fn fep_energy_matches_vault_scalar() {
        // формула сокровищницы fep_loss.rs: F = Σ(p-o)^2 + 0.5λΣp^2
        let p = rand_vec(37, 1); // нечётная длина -> exercising хвост
        let o = rand_vec(37, 2);
        let gw: Vec<f32> = (0..37).map(|i| 1.0 + i as f32 * 0.01).collect();
        let asm = energy(&p, &o, Some(&gw), 0.75);
        let ref_ = energy_scalar(&p, &o, Some(&gw), 0.75);
        assert!(
            (asm - ref_).abs() / ref_.abs().max(1e-6) < 1e-5,
            "F asm={asm} vs scalar={ref_}"
        );
        // единичная метрика через NULL-указатель
        let asm_i = energy(&p, &o, None, 0.75);
        let ref_i = energy_scalar(&p, &o, None, 0.75);
        assert!(
            (asm_i - ref_i).abs() / ref_i.abs().max(1e-6) < 1e-5,
            "F(identity) asm={asm_i} vs scalar={ref_i}"
        );
    }

    #[test]
    fn fep_grad_matches_scalar() {
        let p = rand_vec(131, 7);
        let o = rand_vec(131, 9);
        let mut g_asm = vec![0.0f32; 131];
        let mut g_ref = vec![0.0f32; 131];
        grad(&p, &o, None, 0.05, &mut g_asm);
        grad_scalar(&p, &o, None, 0.05, &mut g_ref);
        for (a, b) in g_asm.iter().zip(&g_ref) {
            assert!((a - b).abs() < 1e-6, "grad {a} vs {b}");
        }
        // λ=0, G=I -> ровно 2(p-o) — контракт literary::engine::grad_f
        grad(&p, &o, None, 0.0, &mut g_asm);
        for i in 0..131 {
            let want = 2.0 * (p[i] - o[i]);
            assert!((g_asm[i] - want).abs() < 1e-6);
        }
    }

    #[test]
    fn fep_step_matches_scalar_and_descends() {
        let o = rand_vec(256, 21);
        let gw: Vec<f32> = (0..256).map(|i| 0.5 + (i % 7) as f32 * 0.1).collect();
        let mut p_asm = rand_vec(256, 5);
        let mut p_ref = p_asm.clone();
        let mut g_asm = vec![0.0f32; 256];
        let mut g_ref = vec![0.0f32; 256];
        let f_asm = step(&mut p_asm, &o, &gw, &mut g_asm, 1e-4, 0.05);
        let f_ref = step_scalar(&mut p_ref, &o, &gw, &mut g_ref, 1e-4, 0.05);
        assert!((f_asm - f_ref).abs() / f_ref.abs().max(1e-6) < 1e-5);
        for i in 0..256 {
            assert!((p_asm[i] - p_ref[i]).abs() < 1e-5, "p[{i}]: {} vs {}", p_asm[i], p_ref[i]);
            assert!((g_asm[i] - g_ref[i]).abs() < 1e-5);
        }
        // градиентный спуск обязан снижать энергию
        let f2 = energy(&p_asm, &o, Some(&gw), 1e-4);
        assert!(f2 < f_asm, "спуск не работает: {f2} !< {f_asm}");
    }
}
