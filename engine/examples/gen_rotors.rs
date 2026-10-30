#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Канонический регенератор языкового ротора No-Mul x86_64 (GAS).
//!
//! Запуск из корня репозитория:
//!   cargo run --example gen_rotors --release
//!
//! Восстанавливает archetype_unrolled_resonance_50k.s байт-в-байт
//! (Zero-Payload Architecture: в git живёт генератор + канонический артефакт).
//!
//! Химический ротор chem_unrolled_resonance_50k.s канонически регенерируется
//! Питон-скриптом: python3 scripts/generate_universal_chem_algorithm.py
//! (там же — первоисточник данных RAW_ELEMENTS с современными значениями
//! электроотрицательностей; Rust-таблица universal_chem.rs использует
//! округлённый винтаж Полинга для быстрых трит-порогов).
use poler_engine::universal_archetype_asm::ArchetypeAsmGenerator;

fn main() {
    let arch = ArchetypeAsmGenerator::new();
    let n_arch = arch
        .write_unrolled_asm_to_file("archetype_unrolled_resonance_50k.s", 52_000)
        .expect("регенерация archetype_unrolled_resonance_50k.s");
    println!("[+] archetype_unrolled_resonance_50k.s: {n_arch} строк (GAS, No-Mul)");
    println!("[+] chem-ротор: python3 scripts/generate_universal_chem_algorithm.py (канонический путь)");
}
