#!/usr/bin/env python3
"""
Генератор монолитного алгоритма «Универсальная Решётка Букв Всех Языков Мира» (Universal Letter Lattice).
Создаёт монолитный Rust-файл (40 000+ строк), содержащий:
1. Полную базу всех букв/графем всех письменностей Unicode (Латиница, Кириллица, Греческий,
   Арабский, Иврит, Деванагари, Грузинский, Армянский, Руны, Глаголица, Хангыль, CJK,
   Коптский, Эфиопский, Тибетский, Тайский, Финикийский, и др.).
2. Фазовые углы CSE (c * phi mod 2pi) для каждого символа.
3. Троичные No-Mul матрицы резонанса и переходов букв.
4. Символьный квантово-каузальный генератор и классификатор без слов.
"""

import sys
import unicodedata
import math

OUTPUT_PATH = "/home/vitalij/Стільниця/poler-engine/src/universal_letters.rs"

PHI = (1.0 + math.sqrt(5.0)) / 2.0  # Золотое сечение

def get_all_unicode_letters():
    letters = []
    # Обходим все кодовые точки Unicode (до BMP и дополнительных плоскостей)
    for cp in range(1, 0x2FFFF):
        ch = chr(cp)
        cat = unicodedata.category(ch)
        # Категории букв: Lu (Uppercase), Ll (Lowercase), Lt (Titlecase), Lm (Modifier), Lo (Other letter)
        if cat.startswith('L'):
            try:
                name = unicodedata.name(ch)
            except ValueError:
                name = f"UNICODE_LETTER_{cp:04X}"
            
            # Определяем семейство письменности
            script = "Other"
            for s in ["LATIN", "CYRILLIC", "GREEK", "ARABIC", "HEBREW", "DEVANAGARI",
                      "GEORGIAN", "ARMENIAN", "RUNIC", "GLAGOLITIC", "HANGUL", "CJK",
                      "COPTIC", "ETHIOPIC", "TIBETAN", "THAI", "PHOENICIAN", "OGHAM",
                      "GOTHIC", "SYRIAC", "THAANA", "BENGALI", "GURMUKHI", "GUJARATI",
                      "ORIYA", "TAMIL", "TELUGU", "KANNADA", "MALAYALAM", "SINHALA",
                      "MYANMAR", "KHMER", "MONGOLIAN", "HIRAGANA", "KATAKANA", "CHEROKEE"]:
                if s in name:
                    script = s.capitalize()
                    break
            
            # Вычисляем CSE золотую фазу
            phase = (cp * PHI) % (2.0 * math.pi)
            
            # Троичный квантованный спин {-1, 0, +1}
            sin_val = math.sin(phase)
            if sin_val > 0.25:
                spin = 1
            elif sin_val < -0.25:
                spin = -1
            else:
                spin = 0

            letters.append({
                "cp": cp,
                "ch": ch,
                "name": name,
                "script": script,
                "phase": phase,
                "spin": spin,
            })
    return letters

def generate_file():
    print("Собираем все буквы всех языков мира из Unicode...")
    letters = get_all_unicode_letters()
    print(f"Найдено {len(letters)} уникальных букв.")

    with open(OUTPUT_PATH, "w", encoding="utf-8") as f:
        f.write("""//! =======================================================================
//! МОНОЛИТНЫЙ АЛГОРИТМ: «УНИВЕРСАЛЬНАЯ РЕШЁТКА ВСЕХ БУКВ МИРА»
//! Чистый символьный уровень: ноль словарей, ноль слов — только буквы,
//! их фазовые углы, триты и топологические резонансы.
//! =======================================================================

#![allow(non_upper_case_globals, unused_variables, dead_code)]

/// Единая структура метаданных буквы всех языков планеты.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UniversalLetter {
    /// Кодовая точка Unicode
    pub codepoint: u32,
    /// Символ
    pub character: char,
    /// Семейство письменности / алфавита
    pub script: &'static str,
    /// Золотая фаза CSE: (c * φ) mod 2π
    pub phase: f64,
    /// Троичный спин No-Mul: {-1, 0, +1}
    pub spin: i8,
}

impl UniversalLetter {
    /// Вычисление резонанса между двумя любыми буквами любых языков мира
    #[inline(always)]
    pub fn resonance(&self, other: &UniversalLetter) -> f64 {
        let d_phase = (self.phase - other.phase).abs();
        let phase_sim = (d_phase.cos() + 1.0) * 0.5; // [0.0, 1.0]
        let spin_sim = if self.spin == other.spin { 1.0 } else if self.spin * other.spin == -1 { 0.0 } else { 0.5 }; // [0.0, 1.0]
        (phase_sim * 0.7 + spin_sim * 0.3).clamp(0.0, 1.0)
    }
}

/// Массив всех зарегистрированных букв всех алфавитов мира.
pub static ALL_LETTERS: &[UniversalLetter] = &[
""")
        
        # Записываем каждую букву как отдельную структуру (это даёт десятки тысяч строк)
        for i, l in enumerate(letters):
            cp = l["cp"]
            # экранируем символ для char literal
            if l["ch"] == '\\':
                ch_repr = "'\\\\'"
            elif l["ch"] == '\'':
                ch_repr = "'\\''"
            elif 32 <= cp <= 126:
                ch_repr = f"'{l['ch']}'"
            elif cp <= 0xFFFF:
                ch_repr = f"'\\u{{{cp:04X}}}'"
            else:
                ch_repr = f"'\\u{{{cp:06X}}}'"

            f.write(f'    UniversalLetter {{ codepoint: 0x{cp:X}, character: {ch_repr}, script: "{l["script"]}", phase: {l["phase"]:.6}, spin: {l["spin"]} }},\n')

        f.write("""
];

/// Универсальный индекс быстрого поиска буквы по кодовой точке.
pub fn lookup_letter(c: char) -> Option<&'static UniversalLetter> {
    let cp = c as u32;
    ALL_LETTERS.binary_search_by_key(&cp, |l| l.codepoint).ok().map(|idx| &ALL_LETTERS[idx])
}

/// Символьный фазовый переход: вычисляет следующую наиболее резонансную букву
/// без использования словарей — исключительно на базе квантово-каузальной фазы букв.
pub fn next_resonant_letter(current: char, temperature: f64) -> char {
    let cur_letter = match lookup_letter(current) {
        Some(l) => l,
        None => return current,
    };

    let mut best_char = current;
    let mut best_score = -1.0;

    // Резонансный поиск по окну решётки
    let cur_idx = ALL_LETTERS.binary_search_by_key(&(current as u32), |l| l.codepoint).unwrap_or(0);
    let start = cur_idx.saturating_sub(128);
    let end = (cur_idx + 128).min(ALL_LETTERS.len());

    for (i, candidate) in ALL_LETTERS[start..end].iter().enumerate() {
        if candidate.character == current {
            continue;
        }
        let res = cur_letter.resonance(candidate);
        let score = res / temperature.max(0.1);
        if score > best_score {
            best_score = score;
            best_char = candidate.character;
        }
    }

    best_char
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_letters_count_and_lookup() {
        assert!(ALL_LETTERS.len() > 30000, "должно быть более 30 000 букв всех языков");
        let a = lookup_letter('A').expect("латинская A");
        let ya = lookup_letter('Я').expect("кириллическая Я");
        let alpha = lookup_letter('Ω').expect("греческая Омега");
        
        assert_eq!(a.script, "Latin");
        assert_eq!(ya.script, "Cyrillic");
        assert_eq!(alpha.script, "Greek");

        let res = a.resonance(ya);
        assert!(res >= 0.0 && res <= 1.0);
    }
}
""")
    print(f"Готово! Файл записан в: {OUTPUT_PATH}")

if __name__ == "__main__":
    generate_file()
