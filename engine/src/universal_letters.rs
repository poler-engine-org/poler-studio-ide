//! =======================================================================
//! АЛГОРИТМ: «УНІВЕРСАЛЬНИЙ СИМВОЛЬНИЙ ФАЗОВИЙ РЕЗОНАТОР» (Universal Letter Engine)
//! Чистий рівень графем/літер без словників: аналітичні золоті фази CSE,
//! типізовані писемності (Unicode 16.0), антисиметричний ротор та No-Mul динаміка.
//! =======================================================================

use std::f64::consts::PI;

/// Золотий перетин: φ = (1 + √5) / 2
pub const PHI: f64 = 1.618033988749895;

/// Квантовий поріг тритизації для синуса фази
pub const THRESHOLD_SPIN: f64 = 0.25;

/// Типізовані писемності та системи письма світу (Unicode 16.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Script {
    Latin = 0,
    Cyrillic,
    Greek,
    Arabic,
    Hebrew,
    Devanagari,
    Bengali,
    Gurmukhi,
    Gujarati,
    Oriya,
    Tamil,
    Telugu,
    Kannada,
    Malayalam,
    Sinhala,
    Thai,
    Lao,
    Tibetan,
    Myanmar,
    Georgian,
    Armenian,
    Ethiopic,
    Cherokee,
    Ogham,
    Runic,
    Glagolitic,
    Coptic,
    Gothic,
    Syriac,
    Thaana,
    Khmer,
    Mongolian,
    Hangul,
    Hiragana,
    Katakana,
    Cjk,
    Phoenician,
    Unknown,
}

impl Script {
    /// Визначення писемності символу за діапазонами Unicode 16.0
    pub fn from_char(c: char) -> Self {
        let cp = c as u32;
        match cp {
            0x0041..=0x005A | 0x0061..=0x007A | 0x00C0..=0x024F | 0x1E00..=0x1EFF => Script::Latin,
            0x0400..=0x04FF | 0x0500..=0x052F | 0x2DE0..=0x2DFF | 0xA640..=0xA69F => Script::Cyrillic,
            0x0370..=0x03FF | 0x1F00..=0x1FFF => Script::Greek,
            0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF => Script::Arabic,
            0x0590..=0x05FF => Script::Hebrew,
            0x0900..=0x097F => Script::Devanagari,
            0x0980..=0x09FF => Script::Bengali,
            0x0A00..=0x0A7F => Script::Gurmukhi,
            0x0A80..=0x0AFF => Script::Gujarati,
            0x0B00..=0x0B7F => Script::Oriya,
            0x0B80..=0x0BFF => Script::Tamil,
            0x0C00..=0x0C7F => Script::Telugu,
            0x0C80..=0x0CFF => Script::Kannada,
            0x0D00..=0x0D7F => Script::Malayalam,
            0x0D80..=0x0DFF => Script::Sinhala,
            0x0E00..=0x0E7F => Script::Thai,
            0x0E80..=0x0EFF => Script::Lao,
            0x0F00..=0x0FFF => Script::Tibetan,
            0x1000..=0x109F => Script::Myanmar,
            0x10A0..=0x10FF | 0x2D00..=0x2D2F => Script::Georgian,
            0x0530..=0x058F => Script::Armenian,
            0x1200..=0x139F => Script::Ethiopic,
            0x13A0..=0x13FF => Script::Cherokee,
            0x1680..=0x169F => Script::Ogham,
            0x16A0..=0x16FF => Script::Runic,
            0x2C00..=0x2C5F | 0x1E000..=0x1E02F => Script::Glagolitic,
            0x2C80..=0x2CFF => Script::Coptic,
            0x10330..=0x1034F => Script::Gothic,
            0x0700..=0x074F => Script::Syriac,
            0x0780..=0x07BF => Script::Thaana,
            0x1780..=0x17FF => Script::Khmer,
            0x1800..=0x18AF => Script::Mongolian,
            0xAC00..=0xD7AF | 0x1100..=0x11FF | 0x3130..=0x318F => Script::Hangul,
            0x3040..=0x309F => Script::Hiragana,
            0x30A0..=0x30FF => Script::Katakana,
            0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF => Script::Cjk,
            0x10900..=0x1091F => Script::Phoenician,
            _ => Script::Unknown,
        }
    }

    /// Спорідненість між писемностями: [0.0, 1.0]
    #[inline]
    pub fn affinity(self, other: Self) -> f64 {
        if self == other {
            return 1.0;
        }
        match (self, other) {
            // Спільний греко-фінікійський корінь
            (Script::Latin, Script::Cyrillic) | (Script::Cyrillic, Script::Latin) => 0.75,
            (Script::Greek, Script::Latin) | (Script::Latin, Script::Greek) => 0.80,
            (Script::Greek, Script::Cyrillic) | (Script::Cyrillic, Script::Greek) => 0.85,
            (Script::Greek, Script::Coptic) | (Script::Coptic, Script::Greek) => 0.90,
            (Script::Cyrillic, Script::Glagolitic) | (Script::Glagolitic, Script::Cyrillic) => 0.95,
            // Семітська гілка
            (Script::Hebrew, Script::Arabic) | (Script::Arabic, Script::Hebrew) => 0.80,
            (Script::Arabic, Script::Syriac) | (Script::Syriac, Script::Arabic) => 0.85,
            // Брахмічні індійські писемності
            (Script::Devanagari, Script::Bengali) | (Script::Bengali, Script::Devanagari) => 0.85,
            (Script::Devanagari, Script::Gujarati) | (Script::Gujarati, Script::Devanagari) => 0.90,
            (Script::Tamil, Script::Telugu) | (Script::Telugu, Script::Tamil) => 0.75,
            // Далекосхідна група
            (Script::Hiragana, Script::Katakana) | (Script::Katakana, Script::Hiragana) => 0.95,
            (Script::Hiragana, Script::Cjk) | (Script::Cjk, Script::Hiragana) => 0.70,
            (Script::Hangul, Script::Cjk) | (Script::Cjk, Script::Hangul) => 0.60,
            _ => 0.10,
        }
    }
}

/// Динамічна квантова точка символу (нуль виділень пам'яті).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LetterState {
    pub character: char,
    pub codepoint: u32,
    pub script: Script,
    pub phase: f64,
    pub spin: i8,
}

impl LetterState {
    /// Миттєвий аналітичний синтез стану для будь-якої літери
    #[inline(always)]
    pub fn new(c: char) -> Self {
        let codepoint = c as u32;
        let phase = ((codepoint as f64) * PHI) % (2.0 * PI);
        let s = phase.sin();
        let spin = if s > THRESHOLD_SPIN {
            1
        } else if s < -THRESHOLD_SPIN {
            -1
        } else {
            0
        };
        let script = Script::from_char(c);

        Self {
            character: c,
            codepoint,
            script,
            phase,
            spin,
        }
    }

    /// Обчислення резонансу між двома символами:
    /// R = 0.50·(cos(Δφ) + 1)/2 + 0.25·SpinMatch + 0.25·ScriptAffinity
    #[inline(always)]
    pub fn resonance(&self, other: &LetterState) -> f64 {
        let d_phase = (self.phase - other.phase).abs();
        let phase_sim = (d_phase.cos() + 1.0) * 0.5;

        let spin_sim = if self.spin == other.spin {
            1.0
        } else if self.spin * other.spin == -1 {
            0.0
        } else {
            0.5
        };

        let script_sim = self.script.affinity(other.script);

        (phase_sim * 0.50 + spin_sim * 0.25 + script_sim * 0.25).clamp(0.0, 1.0)
    }

    /// Антисиметричний ротор циркуляції між символами:
    /// J(a, b) = sin(φ_a - φ_b) · affinity(script_a, script_b)
    #[inline(always)]
    pub fn rotor_flow(&self, other: &LetterState) -> f64 {
        (self.phase - other.phase).sin() * self.script.affinity(other.script)
    }
}

/// Каузальний символьний блукач (Causal Grapheme Walker):
/// Знаходить наступну літеру в динамічному потоці без словників.
pub struct CausalLetterEngine {
    current: LetterState,
    momentum: f64,
    temperature: f64,
}

impl CausalLetterEngine {
    pub fn new(initial_char: char, temperature: f64) -> Self {
        Self {
            current: LetterState::new(initial_char),
            momentum: 0.0,
            temperature: temperature.clamp(0.1, 2.0),
        }
    }

    /// Поточний символ
    pub fn current_char(&self) -> char {
        self.current.character
    }

    /// Зробити крок квантово-фазової еволюції: вибір наступної резонансної літери
    pub fn step(&mut self, alphabet_pool: &[char]) -> char {
        if alphabet_pool.is_empty() {
            return self.current.character;
        }

        let mut best_char = self.current.character;
        let mut best_score = -1e9;

        for &candidate_ch in alphabet_pool {
            if candidate_ch == self.current.character {
                continue;
            }

            let cand_state = LetterState::new(candidate_ch);
            let res = self.current.resonance(&cand_state);
            let flow = self.current.rotor_flow(&cand_state);

            // Критерій вибору: резонанс + роторний потік + момент
            let score = (res + flow * 0.3 + self.momentum * 0.1) / self.temperature;

            if score > best_score {
                best_score = score;
                best_char = candidate_ch;
            }
        }

        // Оновлення стану та моменту
        let next_state = LetterState::new(best_char);
        self.momentum = self.current.rotor_flow(&next_state);
        self.current = next_state;

        best_char
    }

    /// Згенерувати резонансний символьний ланцюжок заданої довжини
    pub fn generate_sequence(&mut self, alphabet_pool: &[char], length: usize) -> String {
        let mut out = String::with_capacity(length);
        out.push(self.current.character);
        for _ in 1..length {
            out.push(self.step(alphabet_pool));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_letter_state_and_phase() {
        let a = LetterState::new('A');
        let ya = LetterState::new('Я');
        let omega = LetterState::new('Ω');
        let alef = LetterState::new('א');

        assert_eq!(a.script, Script::Latin);
        assert_eq!(ya.script, Script::Cyrillic);
        assert_eq!(omega.script, Script::Greek);
        assert_eq!(alef.script, Script::Hebrew);

        // Фаза строго в [0, 2π)
        assert!(a.phase >= 0.0 && a.phase < 2.0 * PI);
        assert!(ya.phase >= 0.0 && ya.phase < 2.0 * PI);

        // Спін строго {-1, 0, 1}
        assert!(matches!(a.spin, -1..=1));
        assert!(matches!(ya.spin, -1..=1));
    }

    #[test]
    fn test_resonance_bounds_and_symmetry() {
        let a = LetterState::new('A');
        let b = LetterState::new('B');

        let r_ab = a.resonance(&b);
        let r_ba = b.resonance(&a);

        assert!((r_ab - r_ba).abs() < 1e-12, "Резонанс має бути симетричним");
        assert!((0.0..=1.0).contains(&r_ab), "Резонанс у межах [0, 1]");

        // Авторезонанс символу з самим собою дорівнює 1.0
        let r_self = a.resonance(&a);
        assert!((r_self - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_rotor_antisymmetry() {
        let a = LetterState::new('A');
        let ya = LetterState::new('Я');

        let flow_ab = a.rotor_flow(&ya);
        let flow_ba = ya.rotor_flow(&a);

        // J(a,b) = -J(b,a) (антисиметрія ротора)
        assert!((flow_ab + flow_ba).abs() < 1e-12, "Ротор має бути строго антисиметричним");
    }

    #[test]
    fn test_causal_letter_engine_walk() {
        let pool = ['A', 'B', 'C', 'D', 'А', 'Б', 'В', 'Г', 'α', 'β', 'γ'];
        let mut engine = CausalLetterEngine::new('A', 0.8);

        let seq = engine.generate_sequence(&pool, 8);
        assert_eq!(seq.chars().count(), 8);
        assert!(seq.starts_with('A'));
    }
}
