//! =======================================================================
//! РОТОРЫ: FFI-МОСТ RUST ⇄ No-Mul x86_64 МОНОЛИТЫ
//! =======================================================================
//!
//! Два .s-монолита (104 537 строк) собираются build.rs в libpoler_rotors.a
//! и линкуются статически (cfg `asm_rotors`). Модуль даёт безопасные обёртки:
//!
//! 1. `archetype_resonance_eval(cp, arch)` — языковой ротор букв мира:
//!    хеш Кнута cp*0x9E3779B9 (сдвиг-цепочкой, без imul) → строка матрицы
//!    12×12 → вес {-1,0,+1} в 16.16 fixed-point.
//! 2. `poler_chem_rotor_unrolled_resonance_50k(in, out, cnt)` — химический
//!    фазовый ротор 118 элементов: пролог обнуляет аккумуляторы, эпилог
//!    возвращает их в выходной буфер ([out+4088] фаза, [out+4092] Δχ-баланс).
//!
//! Универсальность важнее скорости: на не-x86_64 таргетах или без `as`
//! движок деградирует на чисто-Rust путь с идентичной семантикой ротора букв
//! (химротор — та же структура вычисления, см. doc у фолбэка).

use crate::universal_archetype_asm::{ArchetypeMatrix, NUM_ARCHETYPES};

/// Размер входного буфера химротора (читает [rdi + 0..2048))
pub const CHEM_INPUT_BYTES: usize = 2048;
/// Размер выходного буфера химротора (пишет [rsi + 0..4096))
pub const CHEM_OUTPUT_BYTES: usize = 4096;
/// Смещение фазовой суммы архетипов в выходном буфере
pub const CHEM_PHASE_OFFSET: usize = 4088;
/// Смещение Δχ-баланса пар в выходном буфере
pub const CHEM_DCHI_OFFSET: usize = 4092;

#[cfg(asm_rotors)]
mod ffi {
    extern "C" {
        /// rdi = codepoint, rsi = target archetype (0..11) → eax = (вес+1) в 16.16
        pub fn archetype_resonance_eval(codepoint: u32, target_arch: u64) -> i32;
        /// rdi = codepoint, rsi = target_arch, rdx = out_ptr → eax, [rdx] = (вес+1)
        pub fn archetype_unrolled_dispatch(
            codepoint: u32,
            target_arch: u64,
            out_ptr: *mut u32,
        ) -> i32;
        /// rdi = input[2048], rsi = output[4096], rdx = count (зарезервирован)
        pub fn poler_chem_rotor_unrolled_resonance_50k(
            input: *const u8,
            output: *mut u8,
            count: u64,
        );
    }
}

/// Связаны ли .s-роторы (сборка через build.rs + cfg asm_rotors)?
pub fn asm_rotors_linked() -> bool {
    cfg!(asm_rotors)
}

/// Ленивая матрица для Rust-фолбэка (строится один раз).
fn fallback_matrix() -> &'static ArchetypeMatrix {
    static M: std::sync::OnceLock<ArchetypeMatrix> = std::sync::OnceLock::new();
    M.get_or_init(|| ArchetypeMatrix::build())
}

/// Вес языкового ротора для пары (codepoint, target_arch): {-1, 0, +1}.
///
/// Семантика (единая для ASM и Rust): строка = хеш Кнута codepoint (mod 12),
/// столбец = target_arch, значение — антисимметричный трит матрицы Φ-фаз.
/// Для target_arch ≥ 12 или codepoint вне Unicode — нейтральный 0.
pub fn archetype_weight(codepoint: u32, target_arch: usize) -> i8 {
    if target_arch >= NUM_ARCHETYPES {
        return 0;
    }
    #[cfg(asm_rotors)]
    unsafe {
        let ret = ffi::archetype_resonance_eval(codepoint, target_arch as u64);
        // eax = 0x00010000 + (вес << 16) → вес = (ret - 0x10000) >> 16
        ((ret - 0x0001_0000) >> 16) as i8
    }
    #[cfg(not(asm_rotors))]
    {
        let row = ArchetypeMatrix::rotor_hash_arch(codepoint);
        fallback_matrix().weights[row][target_arch]
    }
}

/// Розгорнутий диспетчер (52 035 строк прямих порівнянь) — той самий комфортний
/// API для кодпоінтів покритого діапазону (~0x20..0x230). Повертає (вага+1) в
/// 16.16 та записує його за out. Поза діапазоном — 0 без запису.
pub fn archetype_dispatch(codepoint: u32, target_arch: usize, out: &mut u32) -> i32 {
    if target_arch >= NUM_ARCHETYPES {
        *out = 0;
        return 0;
    }
    #[cfg(asm_rotors)]
    unsafe {
        ffi::archetype_unrolled_dispatch(codepoint, target_arch as u64, out)
    }
    #[cfg(not(asm_rotors))]
    {
        let w = archetype_weight(codepoint, target_arch);
        let v = 0x0001_0000u32.wrapping_add(((w as i32) << 16) as u32);
        *out = v;
        v as i32
    }
}

/// Резонанс химротора: пара аккумуляторов фазового ротора 118 элементов.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChemRotorResonance {
    /// Σ ±sin(Z·Φ + j/2) по архетипам активных элементов (аккумулятор xmm1)
    pub archetype_phase_sum: f32,
    /// Σ ±Δχ по парам элементов суперблоков (аккумулятор xmm3)
    pub delta_chi_balance: f32,
}

/// Запуск химротора для набора активных элементов.
///
/// `active[z-1] != 0` (z = 1..118) активирует элементальный блок Z.
/// Фолбэк (без asm_rotors) воспроизводит семантику аккумуляторов на данных
/// universal_chem (винтаж Полинга) — те же формулы, что эмитит генератор;
/// падынговые суперблоки считаются одним проходом пар (u*7+1)%118.
pub fn chem_rotor_resonance(active_elements: &[u8; 118]) -> ChemRotorResonance {
    #[cfg(asm_rotors)]
    unsafe {
        let mut input = vec![0u8; CHEM_INPUT_BYTES];
        for (idx, &active) in active_elements.iter().enumerate() {
            if active != 0 {
                let off = idx * 16;
                input[off..off + 4].copy_from_slice(&1u32.to_le_bytes());
            }
        }
        let mut output = vec![0u8; CHEM_OUTPUT_BYTES];
        ffi::poler_chem_rotor_unrolled_resonance_50k(
            input.as_ptr(),
            output.as_mut_ptr(),
            0,
        );
        let read_f32 = |off: usize| {
            f32::from_le_bytes([output[off], output[off + 1], output[off + 2], output[off + 3]])
        };
        ChemRotorResonance {
            archetype_phase_sum: read_f32(CHEM_PHASE_OFFSET),
            delta_chi_balance: read_f32(CHEM_DCHI_OFFSET),
        }
    }
    #[cfg(not(asm_rotors))]
    {
        use crate::universal_chem::{ChemArchetypeMatrix, PERIODIC_TABLE, PHI};
        let matrix = ChemArchetypeMatrix::build().weights;
        let mut phase = 0f32;
        let mut dchi = 0f32;
        // Элементные блоки: активные элементы гонят фазовый аккумулятор
        for (idx, elem) in PERIODIC_TABLE.iter().enumerate() {
            if active_elements.get(idx).copied().unwrap_or(0) == 0 {
                continue;
            }
            let arch = elem.archetype as usize;
            for j in 0..crate::universal_chem::NUM_CHEM_ARCHETYPES {
                let w = matrix[arch][j];
                let p = (((elem.z as f64) * PHI + (j as f64) * 0.5).sin()) as f32;
                if w > 0 {
                    phase += p;
                } else if w < 0 {
                    phase -= p;
                }
            }
        }
        // Суперблоки (один проход): пары (u*7+1) mod 118, Δχ-трит решает знак
        for pair_idx in 0..118 {
            let e1 = &PERIODIC_TABLE[pair_idx];
            let e2 = &PERIODIC_TABLE[(pair_idx * 7 + 1) % 118];
            let delta = (e1.electronegativity - e2.electronegativity).abs() as f32;
            let trit = if delta < 0.4 {
                0
            } else if delta <= 2.0 {
                1
            } else {
                -1
            };
            if trit > 0 {
                dchi += delta;
            } else if trit < 0 {
                dchi -= delta;
            }
        }
        ChemRotorResonance {
            archetype_phase_sum: phase,
            delta_chi_balance: dchi,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ASM-ротор букв должен совпадать с Rust-матрицей на представительной выборке
    /// (латиница, кириллица, греческий, CJK, за пределами BMP-диапазона диспетчера).
    #[test]
    fn archetype_rotor_matches_rust_matrix() {
        let samples: &[u32] = &[
            0x41,    // 'A'
            0x61,    // 'a'
            0x44F,   // 'я'
            0x3B8,   // 'θ'
            0x4E2D,  // '中'
            0x20,    // пробел
            0x1F600, // эмодзи (вне таблиц скриптов)
            0x0223,  // 'ȣ' — исторический блок
        ];
        let matrix = ArchetypeMatrix::build();
        for &cp in samples {
            for j in 0..NUM_ARCHETYPES {
                let expect = matrix.weights[ArchetypeMatrix::rotor_hash_arch(cp)][j];
                let got = archetype_weight(cp, j);
                assert_eq!(got, expect, "cp=0x{cp:X}, arch={j}: ASM={got}, Rust={expect}");
            }
        }
    }

    /// Диспетчер (развернутые блоки) согласован с табличным путём в покрытом диапазоне.
    #[test]
    fn dispatch_agrees_with_eval_in_covered_range() {
        for &cp in &[0x41u32, 0x42, 0x7A, 0xE9] {
            for j in 0..NUM_ARCHETYPES {
                let mut out = u32::MAX;
                let disp = archetype_dispatch(cp, j, &mut out);
                let ev = archetype_weight(cp, j);
                let expect_v = 0x0001_0000i32 + ((ev as i32) << 16);
                // в покрытом диапазоне блок обязан сработать и дать то же значение
                assert_eq!(disp, expect_v, "cp=0x{cp:X}, arch={j}");
                assert_eq!(out as i32, expect_v);
            }
        }
    }

    /// Вне границ — нейтральный ноль без паники.
    #[test]
    fn out_of_bounds_is_neutral() {
        assert_eq!(archetype_weight(0x41, 12), 0);
        assert_eq!(archetype_weight(0x41, 999), 0);
        let mut out = u32::MAX;
        let r = archetype_dispatch(0x41, 12, &mut out);
        assert_eq!(r, 0);
        assert_eq!(out, 0);
    }

    /// Химротор: пустая активация → нулевая фаза; Δχ-баланс глобален (суперблоки
    /// пар безусловны — не зависят от активации); активные элементы дают фазу.
    #[test]
    fn chem_rotor_basic_invariants() {
        let none = [0u8; 118];
        let r = chem_rotor_resonance(&none);
        assert_eq!(r.archetype_phase_sum, 0.0, "без активных элементов фаза = 0");
        assert!(r.delta_chi_balance.is_finite(), "Δχ-баланс обязан быть конечным");
        // Δχ-баланс не зависит от активации (суперблоки глобальны)
        let dchi_none = r.delta_chi_balance;

        let mut h2o = [0u8; 118];
        h2o[0] = 1; // H
        h2o[7] = 1; // O
        let r2 = chem_rotor_resonance(&h2o);
        assert!(r2.archetype_phase_sum.is_finite());
        assert!(r2.delta_chi_balance.is_finite());
        assert_eq!(r2.delta_chi_balance, dchi_none, "Δχ-баланс глобален по парам");
        // H (arch 7) и O (arch 7): одна строка матрицы × 12 колонок — фаза ненулевая
        assert!(r2.archetype_phase_sum != 0.0, "активные элементы обязаны дать фазу");
    }

    #[test]
    fn asm_linkage_reported() {
        // Информационный тест: показывает, каким путём идут роторы в этой сборке
        println!("asm_rotors_linked = {}", asm_rotors_linked());
    }
}
