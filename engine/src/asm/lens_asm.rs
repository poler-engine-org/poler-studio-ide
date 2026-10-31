//! LENS No-Hits барьер сокровищницы в машинном коде x86_64.
//!
//! Порт `docs/vault_drafts/poler_quantum/archive/poler-lens/src/lens_index.rs`
//! (99.2% сжатия, блокировка галлюцинаций) на чистый ассемблер.
//!
//! Семантика барьера (документирована в сокровищнице):
//! ```text
//! keep(i)  ⟺  w[i] > min_w  ∧  (flags[i] & require) == require
//!              ∧  (flags[i] & forbid) == 0
//! ```
//! Канонический порог сокровищницы — `min_w = 0.05`.
//!
//! Микроядра:
//! * `poler_lens_filter` — компакция индексов выживших кандидатов
//!   (comiss + прямые ветвления — ядро на голом SSE2, т.е. базовом
//!   уровне x86_64: работает на ЛЮБОМ хосте без детекции);
//! * `poler_lens_popcount` — плотность масок `popcnt` (BFS-подсчёт
//!   хитов require-маски).

use std::arch::global_asm;

global_asm! {
    r#"
    .text
    # ================================================================
    # size_t poler_lens_filter(const f32* w, const u64* flags,
    #                          u32* out, size_t n,
    #                          u64 require, u64 forbid, float min_w)
    #   rdi=w, rsi=flags, rdx=out, rcx=n, r8=require, r9=forbid,
    #   xmm0=min_w -> rax = число выживших
    # ================================================================
    .globl poler_lens_filter
    .p2align 4
poler_lens_filter:
    push rbx
    xor r10, r10                      # count
    xor r11, r11                      # i
    test rcx, rcx
    jz .Llens_fin
.Llens_lp:
    cmp r11, rcx
    jae .Llens_fin
    mov rbx, [rsi + r11*8]
    and rbx, r8
    xor rbx, r8
    jnz .Llens_skip                   # маска require не совпала
    mov rbx, [rsi + r11*8]
    test rbx, r9
    jnz .Llens_skip                   # запрещённые биты присутствуют
    movss xmm1, [rdi + r11*4]
    comiss xmm1, xmm0                 # w vs min_w
    jbe .Llens_skip                   # w <= min_w — барьер!
    mov eax, r11d
    mov [rdx + r10*4], eax
    inc r10
.Llens_skip:
    inc r11
    jmp .Llens_lp
.Llens_fin:
    mov rax, r10
    pop rbx
    ret

    # ================================================================
    # u64 poler_lens_popcount(const u64* flags, size_t n, u64 require)
    #   rdi=flags, rsi=n, rdx=require -> rax = Σ popcnt(flags[i] & require)
    # ================================================================
    .globl poler_lens_popcount
    .p2align 4
poler_lens_popcount:
    xor rax, rax
    xor rcx, rcx
    test rsi, rsi
    jz .Llpc_fin
.Llpc_lp:
    mov r8, [rdi + rcx*8]
    and r8, rdx
    popcnt r8, r8
    add rax, r8
    inc rcx
    cmp rcx, rsi
    jb .Llpc_lp
.Llpc_fin:
    ret
    "#
}

extern "sysv64" {
    fn poler_lens_filter(
        w: *const f32,
        flags: *const u64,
        out: *mut u32,
        n: usize,
        require: u64,
        forbid: u64,
        min_w: f32,
    ) -> usize;
    fn poler_lens_popcount(flags: *const u64, n: usize, require: u64) -> u64;
}

// v0.93.0: LENS_MIN_W и скалярный эталон живут в crate::asm_compat
// (архитектурная независимость); пути сохранены реэкспортами.
pub use crate::asm_compat::{LENS_MIN_W, lens_filter_scalar as filter_scalar};

/// No-Hits барьер: компакция индексов выживших рёбер.
///
/// Возвращает число выживших; `out` вмещает `w.len()` индексов.
/// Ядро на голом SSE2 — доступно всегда, fallback не нужен.
pub fn filter(
    w: &[f32],
    flags: &[u64],
    out: &mut [u32],
    min_w: f32,
    require: u64,
    forbid: u64,
) -> usize {
    let n = w.len().min(flags.len()).min(out.len());
    if n == 0 {
        return 0;
    }
    unsafe {
        poler_lens_filter(
            w.as_ptr(),
            flags.as_ptr(),
            out.as_mut_ptr(),
            n,
            require,
            forbid,
            min_w,
        )
    }
}


/// Плотность require-маски: Σ popcnt(flags & require). Нужен POPCNT.
pub fn popcount(flags: &[u64], require: u64) -> u64 {
    if !crate::asm::caps().popcnt {
        return flags.iter().map(|f| (f & require).count_ones() as u64).sum();
    }
    if flags.is_empty() {
        return 0;
    }
    unsafe { poler_lens_popcount(flags.as_ptr(), flags.len(), require) }
}

// ------------------------------ тесты ------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lens_barrier_matches_vault_semantics() {
        // из lens_index.rs: ребро живёт ⟺ weight > 0.05
        let w: Vec<f32> = vec![0.06, 0.05, 0.049, 1.0, 0.0, 0.5, -0.3, 0.051];
        let flags: Vec<u64> = vec![0, 0, 0, 0, 0, 0, 0, 0];
        let mut out_a = vec![0u32; 8];
        let mut out_b = vec![0u32; 8];
        let ka = filter(&w, &flags, &mut out_a, LENS_MIN_W, 0, 0);
        let kb = filter_scalar(&w, &flags, &mut out_b, LENS_MIN_W, 0, 0);
        assert_eq!(ka, kb);
        // 0.06, 1.0, 0.5, 0.051 выживают; 0.05 (строгое >) — нет
        assert_eq!(ka, 4);
        assert_eq!(&out_a[..ka], &[0u32, 3, 5, 7]);
    }

    #[test]
    fn lens_flag_masks() {
        let w: Vec<f32> = vec![1.0; 6];
        let flags: Vec<u64> = vec![
            0b0011, 0b0010, 0b0001, 0b1111, 0b0100, 0b0011,
        ];
        let mut out_a = vec![0u32; 6];
        let mut out_b = vec![0u32; 6];
        // require = 0b0011, forbid = 0b0100
        let ka = filter(&w, &flags, &mut out_a, LENS_MIN_W, 0b0011, 0b0100);
        let kb = filter_scalar(&w, &flags, &mut out_b, LENS_MIN_W, 0b0011, 0b0100);
        assert_eq!(ka, kb);
        // выживают: 0b0011 (idx 0) и 0b0011 (idx 5); 0b0010 без 0b0001 — нет;
        // 0b0001 без 0b0010 — нет; 0b1111 содержит forbid — нет
        assert_eq!(ka, 2);
        assert_eq!(&out_a[..ka], &[0u32, 5]);
    }

    #[test]
    fn lens_popcount_counts_require_bits() {
        let flags: Vec<u64> = vec![0b1011, 0b0100, 0b1111];
        let total = popcount(&flags, 0b0011);
        assert_eq!(total, 2 + 0 + 2);
    }
}
