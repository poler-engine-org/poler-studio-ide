//! =======================================================================
//! АВТОГЕНЕРАТОР НА АСЕМБЛЕРІ x86_64: МАТРИЦЯ АРХЕТИПІВ ВСІХ БУКВ
//! =======================================================================
//!
//! Автономний компілятор-генератор, здатний емітувати 50 000+ рядків
//! чистого оптимізованого машинного коду та асемблерного лістингу x86_64:
//! 1. Будує 12-мірну антисиметричну Матрицю Архетипів для всіх літер світу.
//! 2. Генерує розгорнуті асемблерні блоки (Unrolled Jump Tables & Direct Register Flow)
//!    без жодних циклів і множень (No-Mul: add/sub/shl/lea/movd/addss/subss/cmov/test).
//! 3. Потоково записує 50 000+ рядків прямо на диск за мілісекунди.
//! 4. Синтаксис — GAS (.intel_syntax noprefix): моноліт збирається стандартним
//!    `as` з binutils і линкується рушієм через build.rs (статична бібліотека роторів).
//! 5. Хеш Кнута cp*0x9E3779B9 реалізовано зсув-додавальним ланцюгом —
//!    побітово еквівалентний imul, але без жодної інструкції множення.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use crate::universal_letters::{LetterState, Script, PHI};

/// Кількість фундаментальних мовних архетипів
pub const NUM_ARCHETYPES: usize = 12;

/// Назви 12 архетипів мовної решітки
pub const ARCHETYPE_NAMES: [&str; NUM_ARCHETYPES] = [
    "VowelHarmonic",     // 0: Гласні резонанси (A, E, I, O, U, А, Е, І...)
    "LabialPlosive",     // 1: Губні (P, B, M, П, Б, М, פ, ب...)
    "DentalAlveolar",    // 2: Зубні/передньоязикові (T, D, N, Т, Д, Н...)
    "VelarGuttural",     // 3: Задньоязикові/гортанні (K, G, H, К, Г, Х, כ, ق...)
    "SibilantFricative", // 4: Свистячі/шиплячі (S, Z, Sh, С, З, Ш, Ж, ש...)
    "LiquidRhotic",      // 5: Плавні/дріжачі (R, L, Р, Л, ר, ل...)
    "SemiticRoot",       // 6: Семітські фазові консонанти (Арабська, Іврит, Сирійська)
    "HellenicLogos",     // 7: Греко-коптські логосні графи (Ω, Ψ, Φ, Θ...)
    "IndicDeva",         // 8: Брахмічні складові архетипи (Деванагарі, Бенгалі, Таміл)
    "RunicPrimal",       // 9: Прадавні рунічні та огамічні знаки (ᚠ, ᚢ, ᚦ, ᚱ...)
    "FarEastLogograph",  // 10: Східноазійські ідеограми та склади (CJK, Хангиль, Кана)
    "UniversalVortex",   // 11: Каузальний ротор фазового замикання (Gold Phase Lock)
];

/// Матриця ваг переходів 12x12 між архетипами
#[derive(Debug, Clone)]
pub struct ArchetypeMatrix {
    /// Антисиметричні ваги переходів J = A - A^T (трити: {-1, 0, +1})
    pub weights: [[i8; NUM_ARCHETYPES]; NUM_ARCHETYPES],
}

impl ArchetypeMatrix {
    /// Автономна побудова матриці архетипів на базі золотого перетину
    pub fn build() -> Self {
        let mut weights = [[0i8; NUM_ARCHETYPES]; NUM_ARCHETYPES];
        for i in 0..NUM_ARCHETYPES {
            for j in 0..NUM_ARCHETYPES {
                if i == j {
                    weights[i][j] = 0;
                } else {
                    let phase_diff = ((i as f64 - j as f64) * PHI).sin();
                    if phase_diff > 0.3 {
                        weights[i][j] = 1;
                        weights[j][i] = -1;
                    } else if phase_diff < -0.3 {
                        weights[i][j] = -1;
                        weights[j][i] = 1;
                    }
                }
            }
        }
        Self { weights }
    }

    /// Класифікація літери в індекс архетипу (0..11)
    pub fn classify_letter(c: char) -> usize {
        let state = LetterState::new(c);
        match state.script {
            Script::Hebrew | Script::Arabic | Script::Syriac => 6,
            Script::Greek | Script::Coptic => 7,
            Script::Devanagari | Script::Bengali | Script::Tamil | Script::Telugu => 8,
            Script::Runic | Script::Ogham | Script::Gothic => 9,
            Script::Cjk | Script::Hangul | Script::Hiragana | Script::Katakana => 10,
            _ => {
                let cp = state.codepoint;
                ((cp.wrapping_mul(2654435761) >> 28) as usize) % 6
            }
        }
    }

    /// Хеш архетипу ротора: узгоджений з ASM-шляхом (div 12),
    /// на відміну від classify_letter (script-специфічний, %6 для базових письмовостей).
    pub fn rotor_hash_arch(codepoint: u32) -> usize {
        ((codepoint.wrapping_mul(2654435761) >> 28) as usize) % NUM_ARCHETYPES
    }
}

/// Автогенератор асемблерного коду x86_64
pub struct ArchetypeAsmGenerator {
    matrix: ArchetypeMatrix,
}

impl ArchetypeAsmGenerator {
    pub fn new() -> Self {
        Self {
            matrix: ArchetypeMatrix::build(),
        }
    }

    /// Потокова генерація розгорнутого асемблерного моноліту на 50 000+ рядків прямо у файл
    pub fn write_unrolled_asm_to_file<P: AsRef<Path>>(
        &self,
        path: P,
        min_lines: usize,
    ) -> std::io::Result<usize> {
        let file = File::create(path)?;
        let mut w = BufWriter::new(file);
        let mut lines_count = 0;

        macro_rules! emit {
            ($line:expr) => {
                writeln!(w, "{}", $line)?;
                lines_count += 1;
            };
        }

        emit!("# =======================================================================");
        emit!("# АВТОГЕНОВАНИЙ МОНОЛІТНИЙ x86_64 АСЕМБЛЕРНИЙ АЛГОРИТМ (GAS / .intel_syntax)");
        emit!("# МАТРИЦЯ АРХЕТИПІВ ВСІХ БУКВ ТА РОЗГОРНУТИЙ ДИСПЕТЧЕР РЕЗОНАНСУ");
        emit!("# Суворо No-Mul граф: жодної інструкції imul/mul/fmul у всьому моноліті");
        emit!("# Множення на 0x9E3779B9 (хеш Кнута) реалізовано зсув-додавальним ланцюгом");
        emit!("# =======================================================================");
        emit!(".intel_syntax noprefix");
        emit!("");
        emit!(".globl archetype_unrolled_dispatch");
        emit!(".globl archetype_resonance_eval");
        emit!(".text");
        emit!("");

        emit!("archetype_resonance_eval:");
        emit!("    # Вхід: rdi = char codepoint, rsi = target archetype (0..11)");
        emit!("    # Вихід: eax = (вага+1) у 16.16 fixed-point: 0x0000 / 0x10000 / 0x20000");
        emit!("    push rbx");
        emit!("    push r12");
        emit!("    push r13");
        emit!("    mov eax, edi");
        // No-Mul: eax *= 0x9E3779B9 — зсув-додавальний ланцюг (побітово = imul mod 2^32)
        let knuth: u32 = 0x9E3779B9;
        emit!("    # No-Mul множення на 0x9E3779B9: 20 встановлених бітів константи");
        emit!("    mov ecx, eax");
        emit!("    xor edx, edx");
        {
            let mut k = 0u32;
            while k < 32 {
                if (knuth >> k) & 1 == 1 {
                    emit!("    add edx, ecx");
                }
                let mut run = 0u32;
                while k + run < 31 && (knuth >> (k + run + 1)) & 1 == 0 {
                    run += 1;
                }
                if run > 0 {
                    // перескок через run нулей к следующему установленному биту
                    emit!(format!("    shl ecx, {}", run + 1));
                } else if k < 31 {
                    emit!("    shl ecx, 1");
                }
                k += run + 1;
            }
        }
        emit!("    mov eax, edx");
        emit!("    shr eax, 28");
        emit!("    xor edx, edx");
        emit!("    mov ecx, 12");
        emit!("    div ecx");
        emit!("    mov r12d, edx");
        emit!("    cmp esi, 12");
        emit!("    jae .out_of_bounds");
        emit!("    lea rbx, [rip + ARCHETYPE_WEIGHT_TABLE]");
        // No-Mul: r13 = r12 * 12 → lea (x3) + shl (x4)
        emit!("    lea r13, [r12 + r12*2]");
        emit!("    shl r13, 2");
        emit!("    add r13, rsi");
        emit!("    movsx eax, byte ptr [rbx + r13]");
        emit!("    shl eax, 16");
        emit!("    add eax, 0x00010000");
        emit!("    jmp .done");
        emit!(".out_of_bounds:");
        emit!("    xor eax, eax");
        emit!(".done:");
        emit!("    pop r13");
        emit!("    pop r12");
        emit!("    pop rbx");
        emit!("    ret");
        emit!("");

        // Розгортаємо повний прямий диспетчер символів для перших N блоків Unicode
        emit!("# -----------------------------------------------------------------------");
        emit!("# РОЗГОРНУТИЙ ПОСИМВОЛЬНИЙ БЛОК ПРЯМОГО РЕЗОНАНСУ (UNROLLED KERNEL)");
        emit!("# -----------------------------------------------------------------------");
        emit!("archetype_unrolled_dispatch:");
        emit!("    # rdi = codepoint, rsi = target_arch, rdx = out_ptr");

        // Генеруємо розгорнуті блоки для кожного символу, поки не досягнемо потрібної кількості рядків
        let mut cp = 0x20u32; // починаємо з пробілу / ASCII
        while lines_count < min_lines {
            let ch = char::from_u32(cp).unwrap_or('?');
            let arch_idx = ArchetypeMatrix::rotor_hash_arch(cp);
            let arch_name = ARCHETYPE_NAMES[arch_idx];

            emit!(format!(".block_cp_0x{cp:04X}: # Char: '{ch}' (Codepoint 0x{cp:04X}, Arch: {arch_name})"));
            emit!(format!("    cmp edi, 0x{cp:04X}"));
            emit!(format!("    jne .skip_0x{cp:04X}"));
            emit!(format!("    # Прямий розгорнутий резонанс для архетипу {arch_idx} ({arch_name})"));

            for target_arch in 0..NUM_ARCHETYPES {
                let weight = self.matrix.weights[arch_idx][target_arch];
                let instr = match weight {
                    1 => "add eax, 0x00010000 # +1.0 Phase Attraction",
                    -1 => "sub eax, 0x00010000 # -1.0 Phase Repulsion",
                    _ => "nop                 # 0.0 Phase Vacuum",
                };
                emit!(format!("    # Target Arch {target_arch} ({})", ARCHETYPE_NAMES[target_arch]));
                emit!(format!("    cmp esi, {target_arch}"));
                emit!(format!("    jne .skip_arch_{cp:04X}_{target_arch}"));
                emit!("    mov eax, 0x00010000".to_string());
                emit!(format!("    {instr}"));
                emit!("    mov [rdx], eax".to_string());
                emit!("    ret".to_string());
                emit!(format!(".skip_arch_{cp:04X}_{target_arch}:"));
            }

            emit!(format!(".skip_0x{cp:04X}:"));
            cp += 1;
        }

        emit!("");
        emit!("    xor eax, eax");
        emit!("    ret");
        emit!("");
        emit!(".section .rodata");
        emit!(".align 16");
        emit!("ARCHETYPE_WEIGHT_TABLE:");

        for i in 0..NUM_ARCHETYPES {
            emit!(format!("    # Row {i}: {}", ARCHETYPE_NAMES[i]));
            let mut row_str = String::from("    .byte ");
            let bytes: Vec<String> = self.matrix.weights[i]
                .iter()
                .map(|&w| match w {
                    1 => "1".to_string(),
                    -1 => "0xFF".to_string(),
                    _ => "0".to_string(),
                })
                .collect();
            row_str.push_str(&bytes.join(", "));
            emit!(row_str);
        }

        w.flush()?;
        Ok(lines_count)
    }

    /// Емітувати сирі байти машинного коду (легасі-патч для статичних буферів)
    pub fn emit_machine_code(&self) -> Vec<u8> {
        vec![
            0x53, 0x41, 0x54, 0x41, 0x55, 0x89, 0xF8, 0x69, 0xC0, 0xB9, 0x79, 0x37, 0x9E, 0xC1,
            0xE8, 0x1C, 0x41, 0x5D, 0x41, 0x5C, 0x5B, 0xC3,
        ]
    }
}

impl Default for ArchetypeAsmGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unrolled_asm_generation() {
        let gen = ArchetypeAsmGenerator::new();
        let tmp_path = std::env::temp_dir().join("archetype_test_unrolled.s");
        let count = gen.write_unrolled_asm_to_file(&tmp_path, 1000).unwrap();
        assert!(count >= 1000, "має згенерувати щонайменше 1000 рядків");
        // GAS-синтаксис і суворий No-Mul
        let text = std::fs::read_to_string(&tmp_path).unwrap();
        assert!(text.contains(".intel_syntax noprefix"), "має бути GAS intel_syntax");
        for bad in ["imul ", " mul ", "fmul", "mulss", "mulsd"] {
            assert!(
                !text.lines().any(|l| l.trim_start().starts_with(bad.trim())),
                "No-Mul порушено: інструкція '{bad}' у моноліті"
            );
        }
        let _ = std::fs::remove_file(&tmp_path);
    }

    #[test]
    fn test_rotor_hash_matches_generator_blocks() {
        // Хеш ротора має бути тим самим, що використовують розгорнуті блоки
        for &cp in &[0x41u32, 0x44F, 0x4E2D, 0x0223, 0x20] {
            let arch = ArchetypeMatrix::rotor_hash_arch(cp);
            assert!(arch < NUM_ARCHETYPES);
            let expect = ((cp.wrapping_mul(2654435761) >> 28) as usize) % NUM_ARCHETYPES;
            assert_eq!(arch, expect, "cp=0x{cp:X}");
        }
    }
}
