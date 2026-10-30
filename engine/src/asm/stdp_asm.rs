//! Трёхфакторный STDP + WTA сокровищницы в машинном коде x86_64 (AVX2+FMA).
//!
//! Порт языкового ядра LanguageCoreV2 (vault блок 0601, «spike-timing
//! без токенизатора») на чистый ассемблер:
//!
//! ```text
//! trace_i ← ρ·trace_i + pre_i            (eligibility, vfmadd213)
//! Δw_i    = η · reward · trace_i          (трёхфакторное правило)
//! Δw_i    ← −Δw_i  для 25% тормозных     (vpxor по маске lane 2,6)
//! w_i     ← clamp(w_i + Δw_i, ±w_max)    (vminps/vmaxps)
//! ```
//!
//! Плюс микроядро WTA-argmax (`comiss`-турнир) — «структура вместо
//! внимания»: победитель определяется одним проходом.

use std::arch::global_asm;

global_asm! {
    r#"
    .text
    .p2align 5
    # 25% тормозных синапсов: lane 2 и lane 6 из 8 (знаковый бит)
.Lst_inh:
    .long 0, 0, 0x80000000, 0, 0, 0, 0x80000000, 0
    .long 0, 0, 0, 0, 0, 0, 0, 0
    .p2align 5
.Lst_abs:
    .long 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF
    .long 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF
    .p2align 5
.Lst_sb:
    .long 0x80000000, 0x80000000, 0x80000000, 0x80000000
    .long 0x80000000, 0x80000000, 0x80000000, 0x80000000

    .text
    # ================================================================
    # float poler_stdp_step(f32* w_io, f32* trace_io, const f32* pre,
    #                       size_t n, float eta, float reward,
    #                       float rho, float wmax) -> Σ|Δw|
    #   rdi=w, rsi=trace, rdx=pre, rcx=n,
    #   xmm0=eta, xmm1=reward, xmm2=rho, xmm3=wmax
    # ================================================================
    .globl poler_stdp_step
    .p2align 4
poler_stdp_step:
    vbroadcastss ymm4, xmm0               # eta
    vbroadcastss ymm5, xmm1               # reward
    vbroadcastss ymm6, xmm2               # rho
    vbroadcastss ymm7, xmm3               # wmax
    vxorps ymm10, ymm7, [rip + .Lst_sb]    # -wmax
    vxorps ymm8, ymm8, ymm8                 # Σ|Δw| (вектор)
    vxorps xmm11, xmm11, xmm11              # Σ|Δw| (хвост)
    xor r9, r9
    test rcx, rcx
    jz .Lst_red
.Lst_l8:
    mov rax, rcx
    sub rax, r9
    cmp rax, 8
    jb .Lst_tail
    vmovups ymm0, [rdi + r9*4]              # w
    vmovups ymm1, [rsi + r9*4]              # trace
    vmovups ymm2, [rdx + r9*4]              # pre
    vfmadd213ps ymm1, ymm6, ymm2            # trace = trace*rho + pre
    vmovups [rsi + r9*4], ymm1
    vmulps ymm3, ymm1, ymm5                 # reward*trace
    vmulps ymm3, ymm3, ymm4                 # * eta = dw
    vxorps ymm3, ymm3, [rip + .Lst_inh]     # тормозные lane 2,6
    vaddps ymm0, ymm0, ymm3                 # w += dw
    vminps ymm0, ymm0, ymm7
    vmaxps ymm0, ymm0, ymm10
    vmovups [rdi + r9*4], ymm0
    vandps ymm9, ymm3, [rip + .Lst_abs]     # |dw|
    vaddps ymm8, ymm8, ymm9
    add r9, 8
    jmp .Lst_l8
.Lst_tail:
    cmp r9, rcx
    jae .Lst_red
    vmovss xmm0, [rdi + r9*4]
    vmovss xmm1, [rsi + r9*4]
    vmovss xmm2, [rdx + r9*4]
    vfmadd213ss xmm1, xmm6, xmm2
    vmovss [rsi + r9*4], xmm1
    vmulss xmm3, xmm1, xmm5
    vmulss xmm3, xmm3, xmm4                 # dw = eta*reward*trace
    # тормозный флип для скаляра: lane-позиция (i & 7) равна 2 или 6
    mov rax, r9
    and rax, 7
    cmp rax, 2
    je 6f
    cmp rax, 6
    je 6f
    jmp 7f
6:
    vxorps xmm3, xmm3, [rip + .Lst_sb]     # байт-маска lane0 = знак
7:
    vaddss xmm0, xmm0, xmm3
    vminss xmm0, xmm0, xmm7
    vmaxss xmm0, xmm0, xmm10
    vmovss [rdi + r9*4], xmm0
    vandps xmm9, xmm3, [rip + .Lst_abs]
    vaddss xmm11, xmm11, xmm9
    inc r9
    jmp .Lst_tail
.Lst_red:
    vextractf128 xmm1, ymm8, 1
    vaddps xmm8, xmm8, xmm1
    vhaddps xmm8, xmm8, xmm8
    vhaddps xmm8, xmm8, xmm8
    vaddss xmm0, xmm8, xmm11
    vzeroupper
    ret

    # ================================================================
    # u32 poler_wta_argmax(const f32* rates, size_t n) -> индекс максимума
    #   rdi=rates, rsi=n -> eax (первый максимум при равенстве)
    #   Ядро на голом SSE2 — работает без детекции.
    # ================================================================
    .globl poler_wta_argmax
    .p2align 4
poler_wta_argmax:
    xor edx, edx                           # best idx
    xor ecx, ecx
    test rsi, rsi
    jz .Lw_fin
    vmovss xmm0, [rdi]
.Lw_l:
    cmp rcx, rsi
    jae .Lw_fin
    vmovss xmm1, [rdi + rcx*4]
    vcomiss xmm1, xmm0
    jbe .Lw_n
    vmovaps xmm0, xmm1
    mov edx, ecx
.Lw_n:
    inc rcx
    jmp .Lw_l
.Lw_fin:
    mov eax, edx
    ret
    "#
}

extern "sysv64" {
    fn poler_stdp_step(
        w: *mut f32,
        trace: *mut f32,
        pre: *const f32,
        n: usize,
        eta: f32,
        reward: f32,
        rho: f32,
        wmax: f32,
    ) -> f32;
    fn poler_wta_argmax(rates: *const f32, n: usize) -> u32;
}

#[inline]
fn ready() -> bool {
    let c = crate::asm::caps();
    c.avx2 && c.fma
}

/// Один шаг трёхфакторного STDP (LanguageCoreV2). Возвращает Σ|Δw|.
///
/// * `w` — веса (обновляются, клампятся в ±wmax);
/// * `trace` — eligibility-следы (обновляются: ρ·trace + pre);
/// * `pre` — пре-активности;
/// * 25% синапсов тормозные (рост веса инвертируется по маске lane 2,6).
pub fn step(
    w: &mut [f32],
    trace: &mut [f32],
    pre: &[f32],
    eta: f32,
    reward: f32,
    rho: f32,
    wmax: f32,
) -> f32 {
    let n = w.len().min(trace.len()).min(pre.len());
    if n == 0 {
        return 0.0;
    }
    if ready() {
        unsafe {
            poler_stdp_step(
                w.as_mut_ptr(),
                trace.as_mut_ptr(),
                pre.as_ptr(),
                n,
                eta,
                reward,
                rho,
                wmax,
            )
        }
    } else {
        step_scalar(&mut w[..n], &mut trace[..n], &pre[..n], eta, reward, rho, wmax)
    }
}

/// Скалярный эталон STDP (та же семантика, lane-позиция = i & 7).
pub fn step_scalar(
    w: &mut [f32],
    trace: &mut [f32],
    pre: &[f32],
    eta: f32,
    reward: f32,
    rho: f32,
    wmax: f32,
) -> f32 {
    let mut total = 0.0f32;
    for i in 0..w.len().min(trace.len()).min(pre.len()) {
        trace[i] = rho * trace[i] + pre[i];
        let mut dw = eta * reward * trace[i];
        let lane = i & 7;
        if lane == 2 || lane == 6 {
            dw = -dw; // 25% тормозных
        }
        w[i] = (w[i] + dw).clamp(-wmax, wmax);
        total += dw.abs();
    }
    total
}

/// WTA-argmax: индекс первого максимума (структура вместо внимания).
/// Ядро SSE2 — доступно всегда.
pub fn wta_argmax(rates: &[f32]) -> u32 {
    if rates.is_empty() {
        return 0;
    }
    unsafe { poler_wta_argmax(rates.as_ptr(), rates.len()) }
}

// ------------------------------ тесты ------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: u64) -> u64 {
        seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)
    }

    #[test]
    fn stdp_step_matches_scalar() {
        let n = 133; // нечётный -> хвост exercised
        let mut s = 42u64;
        let mut w_a: Vec<f32> = (0..n).map(|_| { s = lcg(s); ((s >> 33) as i32 % 200) as f32 / 100.0 }).collect();
        let mut w_r = w_a.clone();
        let mut t_a: Vec<f32> = (0..n).map(|_| { s = lcg(s); ((s >> 33) as i32 % 100) as f32 / 100.0 }).collect();
        let mut t_r = t_a.clone();
        let pre: Vec<f32> = (0..n).map(|_| { s = lcg(s); ((s >> 33) as i32 % 100) as f32 / 100.0 }).collect();
        let sa = step(&mut w_a, &mut t_a, &pre, 0.01, 0.8, 0.9, 2.0);
        let sr = step_scalar(&mut w_r, &mut t_r, &pre, 0.01, 0.8, 0.9, 2.0);
        assert!((sa - sr).abs() / sr.abs().max(1e-6) < 1e-4, "Σ|Δw| {sa} vs {sr}");
        for i in 0..n {
            assert!((w_a[i] - w_r[i]).abs() < 1e-5, "w[{i}] {} vs {}", w_a[i], w_r[i]);
            assert!((t_a[i] - t_r[i]).abs() < 1e-6, "trace[{i}] {} vs {}", t_a[i], t_r[i]);
        }
    }

    #[test]
    fn stdp_inhibitory_synapses_shrink() {
        // lane 2 (индекс 2): тормозный — при положительном reward вес падает
        let mut w = vec![1.0f32; 8];
        let mut tr = vec![0.0f32; 8];
        let pre = vec![1.0f32; 8];
        step(&mut w, &mut tr, &pre, 0.1, 1.0, 0.0, 10.0);
        assert!(w[2] < 1.0, "тормозный синапс вырос: {}", w[2]);
        assert!(w[6] < 1.0, "тормозный синапс вырос: {}", w[6]);
        assert!(w[0] > 1.0 && w[1] > 1.0, "возбуждающие не выросли");
    }

    #[test]
    fn stdp_clamps_to_wmax() {
        let mut w = vec![1.9f32; 8];
        let mut tr = vec![1.0f32; 8];
        let pre = vec![1.0f32; 8];
        for _ in 0..50 {
            step(&mut w, &mut tr, &pre, 0.5, 1.0, 0.0, 2.0);
        }
        assert!(w.iter().all(|&x| x <= 2.0 + 1e-6), "кламп сломан: {w:?}");
    }

    #[test]
    fn wta_finds_first_max() {
        let rates = vec![0.1, 0.5, 0.3, 0.9, 0.9, 0.2];
        assert_eq!(wta_argmax(&rates), 3);
        assert_eq!(wta_argmax(&[5.0f32]), 0);
        assert_eq!(wta_argmax(&[]), 0);
        assert_eq!(wta_argmax(&[-1.0, -0.5, -2.0]), 1);
    }
}

/// Softmax-награда LanguageCoreV2 с АЗУ-гарантом (v0.90.0):
/// `r_i = e^{x_i} / Σ e^{x_j}` — классически делит на сумму; здесь
/// знаменатель идёт через [`crate::asm::zero_asm::div_safe_f32`]:
/// * Σ > 0 — обычная нормировка (сумма = 1);
/// * Σ = 0 (все x → −∞ вырождены) — РАВНОМЕРНЫЕ награды 1/n: нигде
///   ни NaN, ни деления на ноль — полюс заменён осмысленным пределом.
pub fn softmax_reward_safe(rates: &[f32]) -> Vec<f32> {
    let n = rates.len();
    if n == 0 {
        return Vec::new();
    }
    // численная стабильность: вычитание максимума (классика);
    // гарантирует sum ≥ 1 для конечных входов — но вырожденные
    // входы (−∞) всё же проходят через АЗУ-полюс:
    let m = rates.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f32> = rates.iter().map(|&x| (x - m).exp()).collect();
    let sum: f32 = exps.iter().sum();
    if sum == 0.0 || !sum.is_finite() {
        // полюс Σ=0 → равномерный предел (АЗУ: полюс — точка, не сбой)
        return vec![1.0 / n as f32; n];
    }
    exps.into_iter()
        .map(|e| crate::asm::zero_asm::div_safe_f32(e, sum))
        .collect()
}

#[cfg(test)]
mod softmax_tests {
    use super::softmax_reward_safe;

    #[test]
    fn softmax_reward_sums_to_one() {
        let r = [0.1, 0.5, 0.3, 0.9];
        let s = softmax_reward_safe(&r);
        let sum: f32 = s.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6, "Σ = {sum}");
        assert!(s[3] > s[0], "максимум не доминирует");
    }

    #[test]
    fn softmax_reward_degenerate_is_uniform() {
        // все −∞: классика дала бы NaN; АЗУ — равномерный предел
        let r = [f32::NEG_INFINITY; 4];
        let s = softmax_reward_safe(&r);
        assert!(s.iter().all(|&x| (x - 0.25).abs() < 1e-6), "{s:?}");
        assert!(softmax_reward_safe(&[]).is_empty());
        // вырожденно-равные — тоже равномерные
        let s2 = softmax_reward_safe(&[2.0, 2.0, 2.0, 2.0]);
        assert!(s2.iter().all(|&x| (x - 0.25).abs() < 1e-6));
    }
}
